use crate::policy::{self, PermissionMode};
use crate::{Error, Result, config, mcp_tools};
use base64::engine::Engine;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use url::Url;

// used by optional_output_path
#[cfg(test)]
use rs_peekaboo::ImageMode;
use rs_peekaboo::automation::Target;
use rs_peekaboo::automation::validate_output_path;
use rs_peekaboo::{ImageCapture, Point};

#[derive(Clone)]
pub struct AppState {
    pub(crate) inner: Arc<StateInner>,
}

pub(crate) struct StateInner {
    pub(crate) mode: RwLock<PermissionMode>,
    pub(crate) home: PathBuf,
    pub(crate) approvals: Mutex<HashMap<String, Approval>>,
    pub(crate) auto_approve: Mutex<HashSet<String>>,
    pub(crate) session_approved_commands: Mutex<HashMap<String, HashSet<String>>>,
}

#[derive(Clone)]
pub(crate) struct Approval {
    token: String,
    tool_name: String,
    clean_args: Value,
    expires_at: Instant,
}

impl AppState {
    pub fn new(mode: PermissionMode, _verbose: bool) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(StateInner {
                mode: RwLock::new(mode),
                home: config::home_dir()?,
                approvals: Mutex::new(HashMap::new()),
                auto_approve: Mutex::new(HashSet::new()),
                session_approved_commands: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn mode(&self) -> PermissionMode {
        *self
            .inner
            .mode
            .read()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub fn set_mode(&self, mode: PermissionMode) {
        if let Ok(mut guard) = self.inner.mode.write() {
            *guard = mode;
        }
    }

    pub(crate) fn approvals_lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, Approval>>> {
        self.inner
            .approvals
            .lock()
            .map_err(|_| Error::msg("approvals lock poisoned"))
    }

    pub(crate) fn auto_approve_lock(&self) -> Result<std::sync::MutexGuard<'_, HashSet<String>>> {
        self.inner
            .auto_approve
            .lock()
            .map_err(|_| Error::msg("auto approve lock poisoned"))
    }

    pub(crate) fn session_commands_lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, HashSet<String>>>> {
        self.inner
            .session_approved_commands
            .lock()
            .map_err(|_| Error::msg("session command lock poisoned"))
    }
}

// MCP routing ---

pub(crate) fn handle_json_rpc(
    body: &str,
    session_id: &str,
    state: AppState,
) -> Result<Option<Value>> {
    let request: Value = serde_json::from_str(body)?;
    if let Some(items) = request.as_array() {
        let mut responses = Vec::new();
        for item in items {
            if let Some(response) = handle_json_rpc_message(item, session_id, &state)? {
                responses.push(response);
            }
        }
        return if responses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Value::Array(responses)))
        };
    }
    handle_json_rpc_message(&request, session_id, &state)
}

fn handle_json_rpc_message(
    request: &Value,
    session_id: &str,
    state: &AppState,
) -> Result<Option<Value>> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let is_notification = id.as_ref().is_none_or(Value::is_null);
    if is_notification {
        return Ok(None);
    }
    let id = id.unwrap_or(Value::Null);
    let result = match method {
        "notifications/initialized" => json!({}),
        "initialize" => handle_initialize(request),
        "tools/list" => handle_tools_list()?,
        "tools/call" => handle_tools_call_request(request, session_id, state)?,
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

fn handle_initialize(request: &Value) -> Value {
    json!({
        "protocolVersion": request
            .get("params")
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_str)
            .unwrap_or("2024-11-05"),
        "serverInfo": { "name": "poke-around", "version": env!("CARGO_PKG_VERSION") },
        "capabilities": { "tools": { "listChanged": false } },
        "instructions": "This server gives you access to the user's machine. You can run shell commands, read/write files, list directories, use browser-style fetch tools, take screenshots, and get system info. Use these tools to help the user with OS-level tasks."
    })
}

fn handle_tools_list() -> Result<Value> {
    Ok(json!({ "tools": serde_json::from_str::<Value>(&crate::mcp_tools::tools_json())? }))
}

fn handle_tools_call_request(request: &Value, session_id: &str, state: &AppState) -> Result<Value> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    handle_tool_call(name, &args, session_id, state)
}

fn handle_tool_call(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    state: &AppState,
) -> Result<Value> {
    let mode = state.mode();
    if let Some(reason) = policy::evaluate_access_policy(tool_name, args, mode) {
        return Ok(error_result(format!(
            "Blocked by access mode policy: {reason}"
        )));
    }
    if needs_approval(tool_name, args, mode) && !is_approved(tool_name, args, session_id, state)? {
        let result = request_approval(tool_name, args, session_id, state)?;
        return Ok(result);
    }
    let result = mcp_tools::execute_tool(tool_name, args, state)?;
    Ok(result)
}

// Approval system ---

fn needs_approval(tool_name: &str, args: &Value, mode: PermissionMode) -> bool {
    match mode {
        PermissionMode::Full => match mcp_tools::tool_approval(tool_name) {
            Some(mcp_tools::ApprovalCategory::Always) => true,
            Some(mcp_tools::ApprovalCategory::DestructiveOnly) => args
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(policy::is_destructive_command),
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
        object.remove("remember_all_risky");
        object.remove("remember_in_session");
    }
    clean
}

fn is_approved(tool_name: &str, args: &Value, session_id: &str, state: &AppState) -> Result<bool> {
    if state.auto_approve_lock()?.contains(session_id) {
        return Ok(true);
    }
    if tool_name == "run_command"
        && let Some(command) = args.get("command").and_then(Value::as_str)
        && state
            .session_commands_lock()?
            .get(session_id)
            .is_some_and(|commands| commands.contains(command))
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
    let mut approvals = state.approvals_lock()?;
    let Some(approval) = approvals.get(token).cloned() else {
        return Ok(false);
    };
    let valid = approval.expires_at > Instant::now()
        && approval.tool_name == tool_name
        && approval.clean_args == clean_args(args);
    if valid {
        approvals.remove(token);
        if args.get("remember_all_risky").and_then(Value::as_bool) == Some(true) {
            state.auto_approve_lock()?.insert(session_id.to_string());
        }
        if tool_name == "run_command"
            && args.get("remember_in_session").and_then(Value::as_bool) == Some(true)
            && let Some(command) = args.get("command").and_then(Value::as_str)
        {
            state
                .session_commands_lock()?
                .entry(session_id.to_string())
                .or_default()
                .insert(command.to_string());
        }
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
        .approvals_lock()?
        .insert(token.clone(), approval.clone());

    let mut summary = mcp_tools::tool_summary(tool_name, args);
    if args.get("remember_all_risky").and_then(Value::as_bool) == Some(true) {
        summary.push_str(" and remember all risky actions for this session");
    }
    if tool_name == "run_command"
        && args.get("remember_in_session").and_then(Value::as_bool) == Some(true)
    {
        summary.push_str(" and remember this command for this session");
    }
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
            "summary": summary,
            "rememberAllRisky": args.get("remember_all_risky").and_then(Value::as_bool).unwrap_or(false)
        },
        "isError": true
    }))
}

// Shared helpers ---

pub(crate) fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)?.as_str()
}

pub(crate) fn int_arg(args: &Value, key: &str) -> Option<i64> {
    args.get(key)?
        .as_i64()
        .or_else(|| args.get(key)?.as_f64().map(|n| n as i64))
}

pub(crate) fn ok_text(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }] })
}

pub(crate) fn ok_json(value: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()) }],
        "structuredContent": value
    })
}

pub(crate) fn ok_json_with_image(value: Value, capture: &ImageCapture) -> Result<Value> {
    let data = fs::read(&capture.path)?;
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    let response = json!({
        "content": [
            { "type": "text", "text": text },
            {
                "type": "image",
                "data": base64::engine::general_purpose::STANDARD.encode(data),
                "mimeType": capture.mime_type
            }
        ],
        "structuredContent": value
    });
    if capture.ephemeral {
        let _ = fs::remove_file(&capture.path);
    }
    Ok(response)
}

pub(crate) fn optional_output_path(args: &Value) -> Result<Option<PathBuf>> {
    match str_arg(args, "path") {
        Some(path) => Ok(Some(validate_output_path(Path::new(path))?)),
        None => Ok(None),
    }
}

pub(crate) fn error_result(text: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": text.into() }], "isError": true })
}

pub(crate) fn path_arg(args: &Value, state: &AppState) -> Result<PathBuf> {
    resolve_path_arg(args, "path", "~", state)
}

pub(crate) fn file_path_arg(args: &Value, state: &AppState) -> Result<PathBuf> {
    resolve_path_arg(args, "file", "", state)
}

pub(crate) fn resolve_path_arg(
    args: &Value,
    key: &str,
    default: &str,
    state: &AppState,
) -> Result<PathBuf> {
    let raw = args.get(key).and_then(Value::as_str).unwrap_or(default);
    if raw.is_empty() {
        return Err(Error::msg(format!("{key} path is required")));
    }
    let path = expand_path(raw, &state.inner.home);
    let canonical = canonicalize_path(&path)?;
    ensure_path_allowed(&canonical, state)?;
    Ok(canonical)
}

pub(crate) fn expand_path(raw: &str, home: &Path) -> PathBuf {
    if raw == "~" {
        home.to_path_buf()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(raw)
    }
}

pub(crate) fn canonicalize_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|err| Error::msg(format!("invalid path '{}': {err}", path.display())));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let canonical_parent = parent
            .canonicalize()
            .map_err(|err| Error::msg(format!("invalid path '{}': {err}", path.display())))?;
        Ok(canonical_parent.join(
            path.file_name()
                .ok_or_else(|| Error::msg(format!("invalid path '{}'", path.display())))?,
        ))
    } else {
        Ok(path.to_path_buf())
    }
}

pub(crate) fn ensure_path_allowed(path: &Path, state: &AppState) -> Result<()> {
    if state.mode() == PermissionMode::Full {
        return Ok(());
    }
    let home = canonicalize_path(&state.inner.home)?;
    if path.starts_with(&home) {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "Path '{}' is outside the home directory.",
            path.display()
        )))
    }
}

pub(crate) fn block_private_urls(url_str: &str) -> Result<()> {
    if url_str.trim().is_empty() {
        return Err(Error::msg("url is required"));
    }
    let url = Url::parse(url_str).map_err(|err| Error::msg(format!("invalid url: {err}")))?;
    let scheme = url.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(Error::msg(format!(
            "unsupported url scheme '{scheme}', only http and https are allowed"
        )));
    }
    let host = url
        .host_str()
        .ok_or_else(|| Error::msg("url missing host"))?;
    let host_lower = host.to_ascii_lowercase();
    if host_lower == "localhost" || host_lower.ends_with(".localhost") {
        return Err(Error::msg("requests to localhost are not allowed"));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err(Error::msg(format!(
                "requests to private IP {ip} are not allowed"
            )));
        }
        return Ok(());
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs: Vec<std::net::SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|err| Error::msg(format!("dns resolution failed for '{host}': {err}")))?
        .collect();
    for addr in &addrs {
        if is_private_ip(addr.ip()) {
            return Err(Error::msg(format!(
                "url resolves to private IP {}",
                addr.ip()
            )));
        }
    }
    Ok(())
}

pub(crate) fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

pub(crate) fn target_from_args(args: &Value) -> Result<Target> {
    if let Some(element_id) = str_arg(args, "element_id")
        .or_else(|| str_arg(args, "on"))
        .map(str::to_string)
    {
        return Ok(Target::Query {
            query: element_id,
            snapshot: str_arg(args, "snapshot").map(str::to_string),
        });
    }
    Ok(Target::Point(Point {
        x: int_arg(args, "x").unwrap_or(0),
        y: int_arg(args, "y").unwrap_or(0),
    }))
}

pub(crate) fn query_target_from_args(args: &Value) -> Result<Target> {
    let query = str_arg(args, "on")
        .or_else(|| str_arg(args, "element_id"))
        .unwrap_or("")
        .to_string();
    Ok(Target::Query {
        query,
        snapshot: str_arg(args, "snapshot").map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PermissionMode;
    use std::time::{Duration, Instant};

    #[test]
    fn ok_json_with_image_should_embed_mcp_image_content() {
        let path = std::env::temp_dir().join(format!(
            "poke-around-image-result-{}.png",
            std::process::id()
        ));
        fs::write(&path, [1_u8, 2, 3, 4]).unwrap();

        let capture = ImageCapture {
            path: path.clone(),
            mode: ImageMode::Screen,
            bytes: 4,
            mime_type: "image/png".to_string(),
            ephemeral: false,
        };
        let response = ok_json_with_image(json!({ "path": path }), &capture).unwrap();
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

    #[test]
    fn is_approved_should_validate_tool_name_and_args() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let token = "test_token_123".to_string();
        let tool_name = "test_tool";
        let clean_args = json!({ "path": "/test.txt" });

        let add_approval = |expires_in: i64| {
            let mut approvals = state.inner.approvals.lock().unwrap();
            let expires_at = if expires_in >= 0 {
                Instant::now() + Duration::from_secs(expires_in as u64)
            } else {
                Instant::now() - Duration::from_secs((-expires_in) as u64)
            };
            approvals.insert(
                token.clone(),
                Approval {
                    token: token.clone(),
                    tool_name: tool_name.to_string(),
                    clean_args: clean_args.clone(),
                    expires_at,
                },
            );
        };

        let session_id = "session_1";

        add_approval(60);
        let valid_args = json!({
            "approve": true,
            "approval_token": token,
            "path": "/test.txt"
        });
        assert!(is_approved(tool_name, &valid_args, session_id, &state).unwrap());

        add_approval(60);
        let mismatched_tool_args = json!({
            "approve": true,
            "approval_token": token,
            "path": "/test.txt"
        });
        assert!(!is_approved("wrong_tool", &mismatched_tool_args, session_id, &state).unwrap());

        add_approval(60);
        let mismatched_args = json!({
            "approve": true,
            "approval_token": token,
            "path": "/wrong.txt"
        });
        assert!(!is_approved(tool_name, &mismatched_args, session_id, &state).unwrap());

        add_approval(-60);
        let valid_args_expired = json!({
            "approve": true,
            "approval_token": token,
            "path": "/test.txt"
        });
        assert!(!is_approved(tool_name, &valid_args_expired, session_id, &state).unwrap());

        let missing_token_args = json!({
            "approve": true,
            "path": "/test.txt"
        });
        assert!(!is_approved(tool_name, &missing_token_args, session_id, &state).unwrap());
    }

    #[test]
    fn is_approved_should_not_consume_token_until_valid() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let token = "consume_test_token".to_string();
        let tool_name = "write_file";
        state.inner.approvals.lock().unwrap().insert(
            token.clone(),
            Approval {
                token: token.clone(),
                tool_name: tool_name.to_string(),
                clean_args: json!({ "path": "/test.txt", "content": "ok" }),
                expires_at: Instant::now() + Duration::from_secs(60),
            },
        );

        let invalid_args = json!({
            "approve": true,
            "approval_token": token,
            "path": "/wrong.txt",
            "content": "ok"
        });
        assert!(!is_approved(tool_name, &invalid_args, "session", &state).unwrap());
        assert!(state.inner.approvals.lock().unwrap().contains_key(&token));

        let valid_args = json!({
            "approve": true,
            "approval_token": token,
            "path": "/test.txt",
            "content": "ok"
        });
        assert!(is_approved(tool_name, &valid_args, "session", &state).unwrap());
        assert!(!state.inner.approvals.lock().unwrap().contains_key(&token));
    }
}
