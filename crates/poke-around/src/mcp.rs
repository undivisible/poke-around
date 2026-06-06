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
    let response = if request.method == "GET" && matches!(path.as_str(), "/health" | "/mcp") {
        Some(json!({ "ok": true }))
    } else if request.method == "POST" && path == "/mcp" {
        handle_json_rpc(&request.body, &session_id, state)?
    } else {
        write_http_response(
            &mut stream,
            404,
            &json!({ "error": "not found" }).to_string(),
        )?;
        return Ok(());
    };
    match response {
        Some(body) => write_http_response(&mut stream, 200, &body.to_string())?,
        None => write_http_response(&mut stream, 204, "")?,
    }
    Ok(())
}

fn normalized_path(raw: &str) -> String {
    if raw.starts_with('/') {
        return raw.to_string();
    }
    if let Some((_, rest)) = raw.split_once("://")
        && let Some(path_start) = rest.find('/')
    {
        return rest[path_start..].to_string();
    }
    raw.to_string()
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
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
    if method == "notifications/initialized" {
        return Ok(None);
    }
    let result = match method {
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
            let tools_value: Value = serde_json::from_str(tools::tools_json())?;
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
            if id.is_null() {
                return Ok(None);
            }
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
            "write_file" | "edit_file" | "take_screenshot" | "delete_file" => true,
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
        _ => Ok(error_result(format!("Unknown tool: {tool_name}"))),
    }
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
    Ok(ok_json(json!({
        "path": capture.path,
        "bytes": capture.bytes,
        "mime_type": capture.mime_type
    })))
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
