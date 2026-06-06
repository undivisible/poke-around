use crate::policy::{self, PermissionMode};
use crate::{Error, Result, agents, config, tools};
use base64::Engine;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use rs_peekaboo::automation::{Target, parse_point};
use rs_peekaboo::{Bounds, Direction, ImageMode, Peekaboo, Snapshot};

#[derive(Clone)]
pub struct AppState {
    inner: Arc<StateInner>,
}

struct StateInner {
    mode: PermissionMode,
    home: PathBuf,
    verbose: bool,
    approvals: Mutex<HashMap<String, Approval>>,
    auto_approve: Mutex<HashSet<String>>,
}

#[derive(Clone)]
struct Approval {
    token: String,
    tool_name: String,
    clean_args: Value,
    expires_at: Instant,
}

impl AppState {
    pub fn new(mode: PermissionMode, verbose: bool) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(StateInner {
                mode,
                home: config::home_dir()?,
                verbose,
                approvals: Mutex::new(HashMap::new()),
                auto_approve: Mutex::new(HashSet::new()),
            }),
        })
    }

    pub fn mode(&self) -> PermissionMode {
        self.inner.mode
    }
}

pub fn start_server(state: AppState) -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = state.clone();
            thread::spawn(move || {
                let _ = handle_connection(stream, state);
            });
        }
    });
    Ok(port)
}

fn handle_connection(mut stream: TcpStream, state: AppState) -> Result<()> {
    let request = read_http_request(&mut stream)?;
    let path = normalized_path(&request.path);
    if state.inner.verbose {
        eprintln!("http: {} {} -> {}", request.method, request.path, path);
    }
    let session_id = request
        .headers
        .get("mcp-session-id")
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let response = if request.method == "OPTIONS" {
        None
    } else if request.method == "GET" && matches!(path.as_str(), "/" | "/health" | "/mcp") {
        Some(json!({ "ok": true }))
    } else if request.method == "POST" && matches!(path.as_str(), "/" | "/mcp") {
        handle_json_rpc(&request.body, &session_id, state.clone())?
    } else {
        write_http_response(
            &mut stream,
            404,
            &json!({ "error": "not found" }).to_string(),
        )?;
        return Ok(());
    };
    match response {
        Some(body) => {
            let body = body.to_string();
            if state.inner.verbose {
                eprintln!("http: response 200 bytes={}", body.len());
            }
            write_http_response(&mut stream, 200, &body)?;
        }
        None => {
            if state.inner.verbose {
                eprintln!("http: response 204 bytes=0");
            }
            write_http_response(&mut stream, 204, "")?;
        }
    }
    Ok(())
}

fn normalized_path(raw: &str) -> String {
    let path = if raw.starts_with('/') {
        raw.to_string()
    } else if let Some((_, rest)) = raw.split_once("://")
        && let Some(path_start) = rest.find('/')
    {
        rest[path_start..].to_string()
    } else {
        raw.to_string()
    };
    if path.ends_with("/mcp") {
        "/mcp".to_string()
    } else {
        path
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if bytes.len() > 1024 * 1024 {
            return Err(Error::msg("request headers too large"));
        }
    }
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| Error::msg("invalid http request"))?;
    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| Error::msg("missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    while bytes.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let body = String::from_utf8_lossy(&bytes[body_start..body_start + content_length]).to_string();
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn write_http_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, Mcp-Session-Id, Accept\r\nAccess-Control-Expose-Headers: Mcp-Session-Id\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )?;
    Ok(())
}

fn handle_json_rpc(body: &str, session_id: &str, state: AppState) -> Result<Option<Value>> {
    let request: Value = serde_json::from_str(body)?;
    if let Some(items) = request.as_array() {
        let mut responses = Vec::new();
        for item in items {
            if let Some(response) = handle_json_rpc_message(item, session_id, state.clone())? {
                responses.push(response);
            }
        }
        return Ok(Some(Value::Array(responses)));
    }
    handle_json_rpc_message(&request, session_id, state)
}

fn handle_json_rpc_message(
    request: &Value,
    session_id: &str,
    state: AppState,
) -> Result<Option<Value>> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    if state.inner.verbose {
        eprintln!("rpc: {method} id={id}");
    }
    let result = match method {
        "notifications/initialized" => json!({}),
        "initialize" => json!({
            "protocolVersion": request
                .get("params")
                .and_then(|params| params.get("protocolVersion"))
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05"),
            "serverInfo": { "name": "poke-around", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "tools": { "listChanged": false } },
            "instructions": "This server gives you access to the user's machine. You can run shell commands, read/write files, list directories, use browser-style fetch tools, take screenshots, and get system info. Use these tools to help the user with OS-level tasks."
        }),
        "tools/list" => {
            let tools_value: Value = serde_json::from_str(&tools::tools_json())?;
            json!({ "tools": tools_value })
        }
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            handle_tool_call(name, &args, session_id, state)?
        }
        "ping" => json!({}),
        _ => {
            return Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Unknown method: {method}") }
            })));
        }
    };
    Ok(Some(
        json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    ))
}

fn handle_tool_call(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    state: AppState,
) -> Result<Value> {
    if state.inner.verbose {
        eprintln!("tool: {tool_name}");
    }
    if let Some(reason) = policy::evaluate_access_policy(tool_name, args, state.inner.mode) {
        return Ok(error_result(format!(
            "Blocked by access mode policy: {reason}"
        )));
    }
    if needs_approval(tool_name, args, state.inner.mode)
        && !is_approved(tool_name, args, session_id, &state)?
    {
        return request_approval(tool_name, args, session_id, &state);
    }
    execute_tool(tool_name, args, &state)
}

fn needs_approval(tool_name: &str, args: &Value, mode: PermissionMode) -> bool {
    match mode {
        PermissionMode::Full => match tool_name {
            "write_file" | "edit_file" | "take_screenshot" | "delete_file" | "image" | "see"
            | "click" | "press" | "type" | "paste" | "hotkey" | "scroll" | "swipe" | "drag"
            | "move" | "set_value" | "perform_action" | "window" | "app" | "open" | "menu"
            | "clipboard_write" | "run" | "clean" => true,
            "run_command" => args
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(policy::is_destructive_command),
            "git_operations" => args
                .get("operation")
                .and_then(Value::as_str)
                .is_some_and(|op| {
                    !matches!(
                        op,
                        "status" | "diff" | "log" | "show" | "branch" | "rev-parse"
                    )
                }),
            _ => false,
        },
        _ => false,
    }
}

fn clean_args(args: &Value) -> Value {
    let mut clean = args.clone();
    if let Some(object) = clean.as_object_mut() {
        object.remove("approval_token");
        object.remove("approve");
        object.remove("remember_in_session");
        object.remove("remember_all_risky");
    }
    clean
}

fn is_approved(tool_name: &str, args: &Value, session_id: &str, state: &AppState) -> Result<bool> {
    if state
        .inner
        .auto_approve
        .lock()
        .map_err(|_| Error::msg("auto approve lock poisoned"))?
        .contains(session_id)
    {
        return Ok(true);
    }
    if args.get("approve").and_then(Value::as_bool) != Some(true) {
        return Ok(false);
    }
    let token = args
        .get("approval_token")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut approvals = state
        .inner
        .approvals
        .lock()
        .map_err(|_| Error::msg("approval lock poisoned"))?;
    let Some(approval) = approvals.remove(token) else {
        return Ok(false);
    };
    let valid = approval.expires_at > Instant::now()
        && approval.tool_name == tool_name
        && approval.clean_args == clean_args(args);
    if valid && args.get("remember_all_risky").and_then(Value::as_bool) == Some(true) {
        state
            .inner
            .auto_approve
            .lock()
            .map_err(|_| Error::msg("auto approve lock poisoned"))?
            .insert(session_id.to_string());
    }
    Ok(valid)
}

fn request_approval(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    state: &AppState,
) -> Result<Value> {
    let token = format!("{:032x}", rand::random::<u128>());
    let approval = Approval {
        token: token.clone(),
        tool_name: tool_name.to_string(),
        clean_args: clean_args(args),
        expires_at: Instant::now() + Duration::from_secs(300),
    };
    state
        .inner
        .approvals
        .lock()
        .map_err(|_| Error::msg("approval lock poisoned"))?
        .insert(token.clone(), approval.clone());
    let summary = match tool_name {
        "run_command" => format!(
            "Run command: {}",
            args.get("command").and_then(Value::as_str).unwrap_or("")
        ),
        "write_file" => format!(
            "Write file: {}",
            args.get("path").and_then(Value::as_str).unwrap_or("?")
        ),
        "edit_file" => format!(
            "Edit file: {}",
            args.get("path").and_then(Value::as_str).unwrap_or("?")
        ),
        "delete_file" => format!(
            "Delete: {}",
            args.get("path").and_then(Value::as_str).unwrap_or("?")
        ),
        "image" | "see" => "Capture a screenshot".to_string(),
        "click" => "Click on-screen controls".to_string(),
        "press" => "Press a keyboard key".to_string(),
        "type" => "Type text into the active UI".to_string(),
        "paste" => "Paste text into the active UI".to_string(),
        "hotkey" => "Press a hotkey".to_string(),
        "scroll" => "Scroll the screen".to_string(),
        "swipe" | "drag" => "Drag or swipe across the screen".to_string(),
        "move" => "Move the pointer".to_string(),
        "set_value" => "Set a UI element value".to_string(),
        "perform_action" => "Perform a UI accessibility action".to_string(),
        "window" => "Change a window".to_string(),
        "app" => "Change app state".to_string(),
        "open" => "Open a path or URL".to_string(),
        "menu" => "Click a menu item".to_string(),
        "clipboard_write" => "Write text to the clipboard".to_string(),
        "run" => "Run a JSON automation script".to_string(),
        "clean" => "Remove cached snapshots".to_string(),
        _ => "Take screenshot".to_string(),
    };
    Ok(json!({
        "content": [{
            "type": "text",
            "text": "AWAITING_APPROVAL: Ask the user in chat to approve this action. Re-call the same tool with approve=true and approval_token from structuredContent."
        }],
        "structuredContent": {
            "status": "AWAITING_APPROVAL",
            "approvalRequestId": format!("{session_id}:{token}"),
            "approvalToken": approval.token,
            "toolName": tool_name,
            "summary": summary
        },
        "isError": true
    }))
}

fn execute_tool(tool_name: &str, args: &Value, state: &AppState) -> Result<Value> {
    match tool_name {
        "run_command" => run_command(args, state),
        "network_speed" => network_speed(args),
        "read_file" => read_file(args, state),
        "write_file" => write_file(args, state),
        "list_directory" => list_directory(args, state),
        "system_info" => system_info(state),
        "read_image" => read_image(args, state),
        "run_agent" => run_agent(args),
        "take_screenshot" => take_screenshot(args),
        "edit_file" => edit_file(args, state),
        "web_fetch" => web_fetch(args),
        "http_request" => http_request(args),
        "git_operations" => git_operations(args, state),
        "delete_file" => delete_file(args, state),
        "image" => image(args, state),
        "see" => see(args, state),
        "list_screens" => list_screens(args),
        "permissions" => permissions(),
        "click" => click(args, state),
        "press" => press(args, state),
        "type" => type_text(args, state),
        "paste" => paste(args, state),
        "hotkey" => hotkey(args, state),
        "scroll" => scroll(args, state),
        "swipe" => swipe(args, state),
        "drag" => drag(args, state),
        "move" => move_pointer(args, state),
        "set_value" => set_value(args, state),
        "perform_action" => perform_action(args, state),
        "window" => window(args, state),
        "app" => app(args, state),
        "open" => open_target(args, state),
        "menu" => menu(args, state),
        "clipboard_read" => clipboard_read(),
        "clipboard_write" => clipboard_write(args),
        "run" => run_file(args, state),
        "sleep" => sleep(args),
        "clean" => clean(args, state),
        _ => Ok(error_result(format!("Unknown tool: {tool_name}"))),
    }
}

fn image(args: &Value, _state: &AppState) -> Result<Value> {
    let mode = ImageMode::parse(str_arg(args, "mode").unwrap_or("screen"));
    let path = str_arg(args, "path").map(PathBuf::from);
    let retina = args.get("retina").and_then(Value::as_bool).unwrap_or(true);
    let capture = Peekaboo::new().image(mode, path, retina)?;
    let metadata = json!({
        "path": capture.path,
        "mode": capture.mode,
        "bytes": capture.bytes,
        "mime_type": capture.mime_type,
    });
    ok_json_with_image(metadata, &capture.path, &capture.mime_type)
}

fn see(args: &Value, _state: &AppState) -> Result<Value> {
    let app = str_arg(args, "app");
    let mode = ImageMode::parse(str_arg(args, "mode").unwrap_or("screen"));
    let path = str_arg(args, "path").map(PathBuf::from);
    let retina = args.get("retina").and_then(Value::as_bool).unwrap_or(true);
    let peekaboo = Peekaboo::new();
    let capture = peekaboo.image(mode, path, retina)?;
    let snapshot_id = rs_peekaboo::cache::new_snapshot_id();
    let snapshot = Snapshot {
        snapshot_id,
        elements: peekaboo.ui_elements(app)?,
    };
    rs_peekaboo::cache::save_snapshot(&snapshot)?;
    let metadata = json!({
        "snapshot_id": snapshot.snapshot_id,
        "elements": snapshot.elements,
        "image": {
            "path": capture.path,
            "mode": capture.mode,
            "bytes": capture.bytes,
            "mime_type": capture.mime_type,
        }
    });
    ok_json_with_image(metadata, &capture.path, &capture.mime_type)
}

fn list_screens(_args: &Value) -> Result<Value> {
    Ok(ok_json(Peekaboo::new().list_screens()?))
}

fn permissions() -> Result<Value> {
    Ok(ok_json(Peekaboo::new().permissions()))
}

fn click(args: &Value, _state: &AppState) -> Result<Value> {
    let target = if let Some(element_id) = str_arg(args, "element_id") {
        Target::Query {
            query: element_id.to_string(),
            snapshot: str_arg(args, "snapshot").map(str::to_string),
        }
    } else {
        Target::Point(rs_peekaboo::Point {
            x: int_arg(args, "x").unwrap_or(0),
            y: int_arg(args, "y").unwrap_or(0),
        })
    };
    let button = str_arg(args, "button").unwrap_or("left");
    let count = int_arg(args, "count").unwrap_or(1).max(1) as u32;
    Ok(ok_json(Peekaboo::new().click(target, button, count)?))
}

fn press(args: &Value, _state: &AppState) -> Result<Value> {
    let key = str_arg(args, "key").unwrap_or("");
    let count = int_arg(args, "count").unwrap_or(1).max(1) as u32;
    let delay_ms = args.get("delay_ms").and_then(Value::as_u64);
    Ok(ok_json(Peekaboo::new().press(key, count, delay_ms)?))
}

fn type_text(args: &Value, _state: &AppState) -> Result<Value> {
    let text = str_arg(args, "text").unwrap_or("");
    let clear = args.get("clear").and_then(Value::as_bool).unwrap_or(false);
    let press_return = args.get("return").and_then(Value::as_bool).unwrap_or(false);
    let delay_ms = args.get("delay_ms").and_then(Value::as_u64);
    Ok(ok_json(Peekaboo::new().type_text(
        text,
        clear,
        press_return,
        delay_ms,
    )?))
}

fn paste(args: &Value, _state: &AppState) -> Result<Value> {
    let text = str_arg(args, "text").unwrap_or("");
    Ok(ok_json(Peekaboo::new().paste(text)?))
}

fn hotkey(args: &Value, _state: &AppState) -> Result<Value> {
    let keys = str_arg(args, "keys").unwrap_or("");
    let parts = keys
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    Ok(ok_json(Peekaboo::new().hotkey(&parts)?))
}

fn scroll(args: &Value, _state: &AppState) -> Result<Value> {
    let dx = int_arg(args, "dx").unwrap_or(0);
    let dy = int_arg(args, "dy").unwrap_or(0);
    let (direction, amount) = if dx != 0 {
        let direction = if dx < 0 {
            Direction::Left
        } else {
            Direction::Right
        };
        (direction, dx.unsigned_abs().max(1) as u32)
    } else {
        let direction = if dy < 0 {
            Direction::Down
        } else {
            Direction::Up
        };
        (direction, dy.unsigned_abs().max(1) as u32)
    };
    Ok(ok_json(Peekaboo::new().scroll(direction, amount)?))
}

fn swipe(args: &Value, _state: &AppState) -> Result<Value> {
    let from = parse_point(str_arg(args, "from").unwrap_or("0,0"))
        .unwrap_or(rs_peekaboo::Point { x: 0, y: 0 });
    let to = parse_point(str_arg(args, "to").unwrap_or("0,0"))
        .unwrap_or(rs_peekaboo::Point { x: 0, y: 0 });
    let duration_ms = args
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(250);
    Ok(ok_json(Peekaboo::new().swipe(
        Target::Point(from),
        Target::Point(to),
        duration_ms,
    )?))
}

fn drag(args: &Value, _state: &AppState) -> Result<Value> {
    let from = parse_point(str_arg(args, "from").unwrap_or("0,0"))
        .unwrap_or(rs_peekaboo::Point { x: 0, y: 0 });
    let to = parse_point(str_arg(args, "to").unwrap_or("0,0"))
        .unwrap_or(rs_peekaboo::Point { x: 0, y: 0 });
    let duration_ms = args
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(250);
    Ok(ok_json(Peekaboo::new().drag(
        Target::Point(from),
        Target::Point(to),
        duration_ms,
    )?))
}

fn move_pointer(args: &Value, _state: &AppState) -> Result<Value> {
    let target = if let Some(element_id) = str_arg(args, "element_id") {
        Target::Query {
            query: element_id.to_string(),
            snapshot: str_arg(args, "snapshot").map(str::to_string),
        }
    } else {
        Target::Point(rs_peekaboo::Point {
            x: int_arg(args, "x").unwrap_or(0),
            y: int_arg(args, "y").unwrap_or(0),
        })
    };
    Ok(ok_json(Peekaboo::new().move_cursor(target)?))
}

fn set_value(args: &Value, _state: &AppState) -> Result<Value> {
    let on = str_arg(args, "on").unwrap_or("");
    let value = str_arg(args, "value").unwrap_or("");
    let snapshot = str_arg(args, "snapshot").map(str::to_string);
    Ok(ok_json(Peekaboo::new().set_value(
        Target::Query {
            query: on.to_string(),
            snapshot,
        },
        value,
    )?))
}

fn perform_action(args: &Value, _state: &AppState) -> Result<Value> {
    let on = str_arg(args, "on").unwrap_or("");
    let action = str_arg(args, "action").unwrap_or("");
    let snapshot = str_arg(args, "snapshot").map(str::to_string);
    Ok(ok_json(Peekaboo::new().perform_action(
        Target::Query {
            query: on.to_string(),
            snapshot,
        },
        action,
    )?))
}

fn window(args: &Value, _state: &AppState) -> Result<Value> {
    let action = str_arg(args, "action").unwrap_or("");
    if action == "list" {
        return Ok(ok_json(Peekaboo::new().window("list", None, None)?));
    }
    let app = str_arg(args, "app").unwrap_or("");
    let bounds = match action {
        "move" => Some(Bounds {
            x: int_arg(args, "x").unwrap_or(0),
            y: int_arg(args, "y").unwrap_or(0),
            width: 0,
            height: 0,
        }),
        "resize" => Some(Bounds {
            x: 0,
            y: 0,
            width: int_arg(args, "width").unwrap_or(0),
            height: int_arg(args, "height").unwrap_or(0),
        }),
        "set-bounds" => Some(Bounds {
            x: int_arg(args, "x").unwrap_or(0),
            y: int_arg(args, "y").unwrap_or(0),
            width: int_arg(args, "width").unwrap_or(0),
            height: int_arg(args, "height").unwrap_or(0),
        }),
        _ => None,
    };
    Ok(ok_json(Peekaboo::new().window(
        action,
        Some(app),
        bounds,
    )?))
}

fn app(args: &Value, _state: &AppState) -> Result<Value> {
    let action = str_arg(args, "action").unwrap_or("");
    if action == "list" {
        return Ok(ok_json(Peekaboo::new().app("list", None)?));
    }
    let app = str_arg(args, "app").unwrap_or("");
    Ok(ok_json(Peekaboo::new().app(action, Some(app))?))
}

fn open_target(args: &Value, _state: &AppState) -> Result<Value> {
    let target = str_arg(args, "target").unwrap_or("");
    let app = str_arg(args, "app");
    let no_focus = args
        .get("no_focus")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(ok_json(Peekaboo::new().open(target, app, no_focus)?))
}

fn menu(args: &Value, _state: &AppState) -> Result<Value> {
    let action = str_arg(args, "action").unwrap_or("");
    let app = str_arg(args, "app").unwrap_or("");
    if matches!(action, "list" | "list-all" | "inspect") {
        let action = if action == "inspect" { "list" } else { action };
        return Ok(ok_json(Peekaboo::new().menu(action, app, None, None)?));
    }
    let menu = str_arg(args, "menu").unwrap_or("");
    let item = str_arg(args, "item").unwrap_or("");
    Ok(ok_json(Peekaboo::new().menu(
        "click",
        app,
        Some(menu),
        Some(item),
    )?))
}

fn clipboard_read() -> Result<Value> {
    let text = Peekaboo::new().clipboard_read()?;
    Ok(ok_json(json!({ "text": text })))
}

fn clipboard_write(args: &Value) -> Result<Value> {
    let text = str_arg(args, "text").unwrap_or("");
    Ok(ok_json(Peekaboo::new().clipboard_write(text)?))
}

fn run_file(args: &Value, _state: &AppState) -> Result<Value> {
    let file = str_arg(args, "file").unwrap_or("");
    let results = Peekaboo::new().run_file(&PathBuf::from(file))?;
    Ok(ok_json(json!(results)))
}

fn sleep(args: &Value) -> Result<Value> {
    let seconds = args.get("seconds").and_then(Value::as_f64).unwrap_or(0.0);
    let millis = (seconds.max(0.0) * 1000.0) as u64;
    std::thread::sleep(Duration::from_millis(millis));
    Ok(ok_json(json!({ "slept_ms": millis })))
}

fn clean(args: &Value, _state: &AppState) -> Result<Value> {
    let all = args
        .get("all_snapshots")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let snapshot = str_arg(args, "snapshot");
    let removed = rs_peekaboo::cache::clean_snapshots(all, snapshot)?;
    Ok(ok_json(json!({ "removed": removed })))
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)?.as_str()
}

fn int_arg(args: &Value, key: &str) -> Option<i64> {
    args.get(key)?
        .as_i64()
        .or_else(|| args.get(key)?.as_f64().map(|n| n as i64))
}

fn ok_text(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }] })
}

fn ok_json(value: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()) }],
        "structuredContent": value
    })
}

fn ok_json_with_image(value: Value, image_path: &Path, mime_type: &str) -> Result<Value> {
    let data = fs::read(image_path)?;
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    Ok(json!({
        "content": [
            { "type": "text", "text": text },
            {
                "type": "image",
                "data": base64::engine::general_purpose::STANDARD.encode(data),
                "mimeType": mime_type
            }
        ],
        "structuredContent": value
    }))
}

fn error_result(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "isError": true })
}

fn path_arg(args: &Value, state: &AppState) -> Result<PathBuf> {
    let raw = args.get("path").and_then(Value::as_str).unwrap_or("~");
    Ok(expand_path(raw, &state.inner.home))
}

fn expand_path(raw: &str, home: &Path) -> PathBuf {
    if raw == "~" {
        home.to_path_buf()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(raw)
    }
}

fn run_command(args: &Value, state: &AppState) -> Result<Value> {
    let command = args.get("command").and_then(Value::as_str).unwrap_or("");
    let cwd = args
        .get("cwd")
        .and_then(Value::as_str)
        .map(|path| expand_path(path, &state.inner.home))
        .unwrap_or_else(|| state.inner.home.clone());
    let output = Command::new(shell())
        .arg(shell_flag())
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()?;
    Ok(ok_json(json!({
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "exit_code": output.status.code().unwrap_or(1),
        "success": output.status.success()
    })))
}

fn shell() -> &'static str {
    if cfg!(target_os = "windows") {
        "cmd.exe"
    } else if Path::new("/bin/zsh").exists() {
        "/bin/zsh"
    } else {
        "/bin/bash"
    }
}

fn shell_flag() -> &'static str {
    if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-lc"
    }
}

fn network_speed(args: &Value) -> Result<Value> {
    let tests = args.get("tests").and_then(Value::as_str).unwrap_or("both");
    let mut result = serde_json::Map::new();
    if matches!(tests, "download" | "both") {
        result.insert(
            "download_mbps".to_string(),
            json!(measure_curl_speed(&[
                "-o",
                "/dev/null",
                "-sS",
                "-w",
                "%{speed_download}",
                "https://speed.cloudflare.com/__down?bytes=10000000",
            ])?),
        );
    }
    if matches!(tests, "upload" | "both") {
        result.insert(
            "upload_mbps".to_string(),
            json!(measure_curl_speed(&[
                "-o",
                "/dev/null",
                "-sS",
                "-w",
                "%{speed_upload}",
                "-X",
                "POST",
                "--data-binary",
                "poke-around-speed-test",
                "https://speed.cloudflare.com/__up",
            ])?),
        );
    }
    Ok(ok_json(Value::Object(result)))
}

fn measure_curl_speed(args: &[&str]) -> Result<f64> {
    let output = Command::new("curl").args(args).output()?;
    if !output.status.success() {
        return Err(Error::msg(String::from_utf8_lossy(&output.stderr)));
    }
    let bytes_per_second = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .map_err(|err| Error::msg(format!("invalid curl speed output: {err}")))?;
    Ok((bytes_per_second * 8.0) / 1_000_000.0)
}

fn read_file(args: &Value, state: &AppState) -> Result<Value> {
    Ok(ok_text(fs::read_to_string(path_arg(args, state)?)?))
}

fn write_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let content = args.get("content").and_then(Value::as_str).unwrap_or("");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    Ok(ok_json(json!({ "path": path, "bytes": content.len() })))
}

fn list_directory(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        entries.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "path": entry.path(),
            "is_dir": metadata.is_dir(),
            "size": metadata.len()
        }));
    }
    Ok(ok_json(json!({ "path": path, "entries": entries })))
}

fn system_info(state: &AppState) -> Result<Value> {
    Ok(ok_json(json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "home": state.inner.home,
        "now": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs()
    })))
}

fn read_image(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let data = fs::read(&path)?;
    let mime = match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };
    Ok(ok_json(json!({
        "path": path,
        "mime_type": mime,
        "base64": base64::engine::general_purpose::STANDARD.encode(data)
    })))
}

fn run_agent(args: &Value) -> Result<Value> {
    let name = args.get("name").and_then(Value::as_str).unwrap_or("");
    agents::run_agent_by_name(name)?;
    Ok(ok_json(json!({ "name": name, "status": "ran" })))
}

fn take_screenshot(args: &Value) -> Result<Value> {
    let path = args.get("path").and_then(Value::as_str).map(PathBuf::from);
    let capture = rs_peekaboo::Peekaboo::new().image(rs_peekaboo::ImageMode::Screen, path, true)?;
    let metadata = json!({
        "path": capture.path,
        "bytes": capture.bytes,
        "mime_type": capture.mime_type
    });
    ok_json_with_image(metadata, &capture.path, &capture.mime_type)
}

fn edit_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let old = args.get("old_string").and_then(Value::as_str).unwrap_or("");
    let new = args.get("new_string").and_then(Value::as_str).unwrap_or("");
    let data = fs::read_to_string(&path)?;
    let count = data.matches(old).count();
    if old.is_empty() || count != 1 {
        return Ok(error_result(format!("old_string matched {count} times")));
    }
    let updated = data.replacen(old, new, 1);
    fs::write(&path, updated)?;
    Ok(ok_json(json!({ "path": path, "replacements": 1 })))
}

fn web_fetch(args: &Value) -> Result<Value> {
    let url = args.get("url").and_then(Value::as_str).unwrap_or("");
    let max_chars = args
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(20000) as usize;
    let output = Command::new("curl").arg("-fsSL").arg(url).output()?;
    if !output.status.success() {
        return Ok(error_result(String::from_utf8_lossy(&output.stderr)));
    }
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.len() > max_chars {
        text.truncate(max_chars);
    }
    Ok(ok_text(text))
}

fn http_request(args: &Value) -> Result<Value> {
    let method = args.get("method").and_then(Value::as_str).unwrap_or("GET");
    let url = args.get("url").and_then(Value::as_str).unwrap_or("");
    let mut command = Command::new("curl");
    command.arg("-sS").arg("-X").arg(method);
    if let Some(headers) = args.get("headers").and_then(Value::as_object) {
        for (name, value) in headers {
            command.arg("-H").arg(format!(
                "{name}: {}",
                value.as_str().unwrap_or(&value.to_string())
            ));
        }
    }
    if let Some(body) = args.get("body").and_then(Value::as_str) {
        command.arg("--data").arg(body);
    }
    let output = command.arg(url).output()?;
    Ok(ok_json(json!({
        "success": output.status.success(),
        "status": output.status.code().unwrap_or(1),
        "body": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr)
    })))
}

fn git_operations(args: &Value, state: &AppState) -> Result<Value> {
    let operation = args.get("operation").and_then(Value::as_str).unwrap_or("");
    let cwd = args
        .get("cwd")
        .and_then(Value::as_str)
        .map(|path| expand_path(path, &state.inner.home))
        .unwrap_or_else(|| state.inner.home.clone());
    let mut command = Command::new("git");
    command.arg(operation);
    if let Some(extra) = args.get("args").and_then(Value::as_array) {
        for arg in extra.iter().filter_map(Value::as_str) {
            command.arg(arg);
        }
    }
    let output = command.current_dir(cwd).output()?;
    Ok(ok_json(json!({
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "exit_code": output.status.code().unwrap_or(1),
        "success": output.status.success()
    })))
}

fn delete_file(args: &Value, state: &AppState) -> Result<Value> {
    let path = path_arg(args, state)?;
    let recursive = args
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let metadata = fs::metadata(&path)?;
    if metadata.is_dir() {
        if recursive {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_dir(&path)?;
        }
    } else {
        fs::remove_file(&path)?;
    }
    Ok(ok_json(json!({ "path": path, "deleted": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_json_with_image_should_embed_mcp_image_content() {
        let path = std::env::temp_dir().join(format!(
            "poke-around-image-result-{}.png",
            std::process::id()
        ));
        fs::write(&path, [1_u8, 2, 3, 4]).unwrap();

        let response = ok_json_with_image(json!({ "path": path }), &path, "image/png").unwrap();
        let content = response["content"].as_array().unwrap();

        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["mimeType"], "image/png");
        assert_eq!(content[1]["data"], "AQIDBA==");
        assert_eq!(
            response["structuredContent"]["path"].as_str().unwrap(),
            path.to_string_lossy()
        );

        let _ = fs::remove_file(path);
    }
}
