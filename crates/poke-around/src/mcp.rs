use crate::policy::{self, ApprovalMode, PermissionMode};
use crate::{Error, Result, config, mcp_tools};
use ed25519_dalek::SigningKey;
use praefectus::{CancellationToken, NativeExecutor};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use url::Url;

// used by optional_output_path
use rs_peekaboo::ImageCapture;
#[cfg(test)]
use rs_peekaboo::ImageMode;
use rs_peekaboo::automation::validate_output_path;

const MAX_PENDING_APPROVALS: usize = 32;
const MAX_PENDING_CANCELLATIONS: usize = 64;

#[derive(Clone)]
pub struct AppState {
    pub(crate) inner: Arc<StateInner>,
}

pub(crate) struct StateInner {
    pub(crate) mode: RwLock<PermissionMode>,
    pub(crate) approval_mode: RwLock<ApprovalMode>,
    pub(crate) home: PathBuf,
    pub(crate) approvals: Mutex<HashMap<String, Approval>>,
    active_requests: Mutex<ActiveRequests>,
    dispatch: RwLock<()>,
    mode_change: Mutex<()>,
    mode_transition: AtomicBool,
    pub(crate) approval_generation: AtomicU64,
    approval_prompt: Mutex<()>,
    pub(crate) praefectus_signing_key: SigningKey,
    pub(crate) praefectus_executor: Arc<NativeExecutor>,
    pub(crate) semantic_observations:
        Mutex<HashMap<String, crate::praefectus_adapter::BoundSemanticObservation>>,
}

#[derive(Clone)]
pub(crate) struct Approval {
    session_id: String,
    tool_name: String,
    clean_args: Value,
    generation: u64,
    expires_at: Instant,
    decision: ApprovalDecision,
}

#[derive(Default)]
struct ActiveRequests {
    active: HashMap<(String, String), CancellationToken>,
    pending_cancellations: HashMap<(String, String), Instant>,
    cancel_all_until: Option<Instant>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ApprovalDecision {
    Pending,
    Approved,
    Denied,
}

impl AppState {
    pub fn new(mode: PermissionMode, _verbose: bool) -> Result<Self> {
        Self::with_approval_mode(mode, ApprovalMode::Full)
    }

    pub fn with_approval_mode(mode: PermissionMode, approval_mode: ApprovalMode) -> Result<Self> {
        config::harden_peekaboo_cache()?;
        let home = config::home_dir()?;
        mcp_tools::harden_artifact_cache(&home)?;
        Ok(Self {
            inner: Arc::new(StateInner {
                mode: RwLock::new(mode),
                approval_mode: RwLock::new(approval_mode),
                home,
                approvals: Mutex::new(HashMap::new()),
                active_requests: Mutex::new(ActiveRequests::default()),
                dispatch: RwLock::new(()),
                mode_change: Mutex::new(()),
                mode_transition: AtomicBool::new(false),
                approval_generation: AtomicU64::new(rand::random()),
                approval_prompt: Mutex::new(()),
                praefectus_signing_key: SigningKey::from_bytes(&rand::random()),
                praefectus_executor: Arc::new(NativeExecutor::default()),
                semantic_observations: Mutex::new(HashMap::new()),
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
        let _mode_change = self
            .inner
            .mode_change
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.mode() == mode {
            return;
        }
        self.inner.mode_transition.store(true, Ordering::Release);
        {
            let active = self
                .inner
                .active_requests
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            for cancellation in active.active.values() {
                cancellation.cancel();
            }
        }
        {
            let _dispatch = self
                .inner
                .dispatch
                .write()
                .unwrap_or_else(|error| error.into_inner());
            let mut mode_guard = self
                .inner
                .mode
                .write()
                .unwrap_or_else(|error| error.into_inner());
            let mut approvals = self
                .inner
                .approvals
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut observations = self
                .inner
                .semantic_observations
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            self.inner
                .approval_generation
                .fetch_add(1, Ordering::AcqRel);
            approvals.clear();
            observations.clear();
            *mode_guard = mode;
        }
        self.inner.mode_transition.store(false, Ordering::Release);
    }

    pub fn approval_mode(&self) -> ApprovalMode {
        *self
            .inner
            .approval_mode
            .read()
            .unwrap_or_else(|err| err.into_inner())
    }

    pub(crate) fn approvals_lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, Approval>>> {
        self.inner
            .approvals
            .lock()
            .map_err(|_| Error::msg("approvals lock poisoned"))
    }

    fn decide_approval(&self, request_id: &str, decision: ApprovalDecision) -> Result<bool> {
        let generation = self.inner.approval_generation.load(Ordering::Acquire);
        let mut approvals = self.approvals_lock()?;
        let Some(approval) = approvals.get_mut(request_id) else {
            return Ok(false);
        };
        if approval.generation != generation || approval.expires_at <= Instant::now() {
            approvals.remove(request_id);
            return Ok(false);
        }
        approval.decision = decision;
        Ok(true)
    }

    fn cancel_request(&self, session_id: &str, request_id: &str) {
        let Ok(mut requests) = self.inner.active_requests.lock() else {
            return;
        };
        let key = (session_id.to_string(), request_id.to_string());
        if let Some(cancellation) = requests.active.get(&key) {
            cancellation.cancel();
            return;
        }
        let now = Instant::now();
        requests
            .pending_cancellations
            .retain(|_, expires_at| *expires_at > now);
        if requests.pending_cancellations.len() < MAX_PENDING_CANCELLATIONS {
            requests
                .pending_cancellations
                .insert(key, now + Duration::from_secs(30));
        } else {
            requests.cancel_all_until = Some(now + Duration::from_secs(30));
        }
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
        if method == "notifications/cancelled"
            && let Some(request_id) = request
                .get("params")
                .and_then(|params| params.get("requestId"))
            && let Ok(request_id) = serde_json::to_string(request_id)
        {
            state.cancel_request(session_id, &request_id);
        }
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
        "instructions": "This server exposes only the bounded tools returned by tools/list. Semantic UI observation, click, and value setting use short-lived host-fenced tags. Image capture returns only a private content-addressed artifact reference. Raw coordinates, caller-selected screenshot paths, shell, network, agent, and debugger access are unavailable."
    })
}

fn handle_tools_list() -> Result<Value> {
    Ok(json!({ "tools": crate::mcp_tools::tools_value().clone() }))
}

fn handle_tools_call_request(request: &Value, session_id: &str, state: &AppState) -> Result<Value> {
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let request_id = serde_json::to_string(request.get("id").unwrap_or(&Value::Null))?;
    handle_tool_call(name, &args, session_id, &request_id, state)
}

fn handle_tool_call(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    request_id: &str,
    state: &AppState,
) -> Result<Value> {
    handle_tool_call_with_adapter(
        tool_name,
        args,
        session_id,
        request_id,
        state,
        crate::praefectus_adapter::execute_tool,
    )
}

fn handle_tool_call_with_adapter(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    request_id: &str,
    state: &AppState,
    execute_adapter: impl FnOnce(
        &str,
        &Value,
        &str,
        &str,
        &AppState,
        &CancellationToken,
    ) -> Result<Option<Value>>,
) -> Result<Value> {
    let _dispatch = state
        .inner
        .dispatch
        .read()
        .unwrap_or_else(|error| error.into_inner());
    let cancellation = CancellationToken::default();
    let active_key = (session_id.to_string(), request_id.to_string());
    if let Err(message) = register_active_request(state, &active_key, &cancellation) {
        return Ok(error_result(message));
    }
    let result = (|| {
        if cancellation.is_cancelled() {
            return Ok(json!({
                "content": [{ "type": "text", "text": "CANCELLED_BEFORE_EFFECT" }],
                "structuredContent": {
                    "status": "CANCELLED_BEFORE_EFFECT",
                    "retry_safe": true
                },
                "isError": true
            }));
        }
        let mode = state.mode();
        let approval_generation = state.inner.approval_generation.load(Ordering::Acquire);
        if mcp_tools::is_unavailable_tool(tool_name) {
            return Ok(error_result(
                "tool unavailable: hardened authority, privacy, or cancellation guarantees are not supported",
            ));
        }
        if let Some(reason) = policy::evaluate_access_policy(tool_name, args, mode) {
            return Ok(error_result(format!(
                "Blocked by access mode policy: {reason}"
            )));
        }
        if needs_approval(tool_name, args, state.approval_mode()) {
            match consume_approval(tool_name, args, session_id, state)? {
                ApprovalStatus::Approved => {}
                ApprovalStatus::New => {
                    return request_approval(
                        tool_name,
                        args,
                        session_id,
                        approval_generation,
                        state,
                    );
                }
                ApprovalStatus::Pending => {
                    return Ok(approval_status_result("AWAITING_APPROVAL"));
                }
                ApprovalStatus::Denied => {
                    return Ok(approval_status_result("APPROVAL_DENIED"));
                }
                ApprovalStatus::Invalid => {
                    return Ok(approval_status_result("APPROVAL_INVALID"));
                }
            }
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        match execute_adapter(
            tool_name,
            args,
            session_id,
            request_id,
            state,
            &cancellation,
        ) {
            Ok(Some(result)) => Ok(result),
            Ok(None) => mcp_tools::execute_tool_with_control(
                tool_name,
                args,
                state,
                &cancellation,
                deadline,
            ),
            Err(error) => Err(error),
        }
    })();
    if let Ok(mut requests) = state.inner.active_requests.lock() {
        requests.active.remove(&active_key);
    }
    result
}

fn register_active_request(
    state: &AppState,
    key: &(String, String),
    cancellation: &CancellationToken,
) -> std::result::Result<(), &'static str> {
    let mut requests = state
        .inner
        .active_requests
        .lock()
        .map_err(|_| "active requests unavailable")?;
    if state.inner.mode_transition.load(Ordering::Acquire) {
        return Err("access mode transition in progress");
    }
    if requests.active.contains_key(key) {
        return Err("request is already in progress");
    }
    let now = Instant::now();
    requests
        .pending_cancellations
        .retain(|_, expires_at| *expires_at > now);
    if requests.pending_cancellations.remove(key).is_some()
        || requests.cancel_all_until.is_some_and(|until| until > now)
    {
        cancellation.cancel();
    }
    requests.active.insert(key.clone(), cancellation.clone());
    Ok(())
}

// Approval system ---

fn needs_approval(tool_name: &str, args: &Value, approval_mode: ApprovalMode) -> bool {
    if approval_mode == ApprovalMode::Full {
        return false;
    }
    match mcp_tools::tool_approval(tool_name) {
        Some(mcp_tools::ApprovalCategory::Always) => true,
        Some(mcp_tools::ApprovalCategory::MutatingHttp) => args
            .get("method")
            .and_then(Value::as_str)
            .is_some_and(|method| {
                !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD")
            }),
        Some(mcp_tools::ApprovalCategory::PermissionGrant) => {
            str_arg(args, "action") == Some("grant")
        }
        _ => false,
    }
}

fn clean_args(args: &Value) -> Value {
    let mut clean = args.clone();
    if let Some(object) = clean.as_object_mut() {
        object.remove("approval_token");
        object.remove("approve");
        object.remove("approval_request_id");
    }
    clean
}

enum ApprovalStatus {
    New,
    Pending,
    Approved,
    Denied,
    Invalid,
}

fn consume_approval(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    state: &AppState,
) -> Result<ApprovalStatus> {
    let Some(request_id) = args.get("approval_request_id").and_then(Value::as_str) else {
        return Ok(ApprovalStatus::New);
    };
    let generation = state.inner.approval_generation.load(Ordering::Acquire);
    let mut approvals = state.approvals_lock()?;
    let Some(approval) = approvals.get(request_id).cloned() else {
        return Ok(ApprovalStatus::Invalid);
    };
    if approval.expires_at <= Instant::now() || approval.generation != generation {
        approvals.remove(request_id);
        return Ok(ApprovalStatus::Invalid);
    }
    if approval.session_id != session_id
        || approval.tool_name != tool_name
        || approval.clean_args != clean_args(args)
    {
        return Ok(ApprovalStatus::Invalid);
    }
    match approval.decision {
        ApprovalDecision::Pending => Ok(ApprovalStatus::Pending),
        ApprovalDecision::Approved => {
            approvals.remove(request_id);
            Ok(ApprovalStatus::Approved)
        }
        ApprovalDecision::Denied => {
            approvals.remove(request_id);
            Ok(ApprovalStatus::Denied)
        }
    }
}

fn request_approval(
    tool_name: &str,
    args: &Value,
    session_id: &str,
    generation: u64,
    state: &AppState,
) -> Result<Value> {
    let (host_summary, caller_summary) =
        match crate::praefectus_adapter::approval_summary(tool_name, args, session_id, state) {
            Some(Ok(summary)) => (summary.host, summary.caller),
            Some(Err(error)) => return Ok(error_result(error.to_string())),
            None => {
                let summary = mcp_tools::tool_summary(tool_name, args);
                (summary.clone(), summary)
            }
        };
    let request_id = format!("{:032x}", rand::random::<u128>());
    let approval = Approval {
        session_id: session_id.to_string(),
        tool_name: tool_name.to_string(),
        clean_args: clean_args(args),
        generation,
        expires_at: Instant::now() + Duration::from_secs(300),
        decision: ApprovalDecision::Pending,
    };
    let mut approvals = state.approvals_lock()?;
    approvals.retain(|_, approval| {
        approval.expires_at > Instant::now() && approval.generation == generation
    });
    if approvals.len() >= MAX_PENDING_APPROVALS {
        return Ok(approval_status_result("APPROVAL_QUEUE_FULL"));
    }
    approvals.insert(request_id.clone(), approval);
    drop(approvals);

    spawn_host_prompt(state.clone(), request_id.clone(), host_summary);
    Ok(json!({
        "content": [{
            "type": "text",
            "text": "AWAITING_APPROVAL: The local host must approve this action. Re-call the same tool with approval_request_id after approval."
        }],
        "structuredContent": {
            "status": "AWAITING_APPROVAL",
            "approvalRequestId": request_id,
            "toolName": tool_name,
            "summary": caller_summary
        },
        "isError": true
    }))
}

fn approval_status_result(status: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": status }],
        "structuredContent": { "status": status },
        "isError": true
    })
}

fn spawn_host_prompt(state: AppState, request_id: String, summary: String) {
    if !io::stdin().is_terminal() {
        return;
    }
    std::thread::spawn(move || {
        let Ok(_prompt) = state.inner.approval_prompt.lock() else {
            return;
        };
        eprint!(
            "Poke Around requests: {}\nApprove? [y/N] ",
            sanitize_host_prompt(&summary)
        );
        let _ = io::stderr().flush();
        let mut answer = String::new();
        let approved = io::stdin()
            .read_line(&mut answer)
            .is_ok_and(|_| answer.trim().eq_ignore_ascii_case("y"));
        let _ = state.decide_approval(
            &request_id,
            if approved {
                ApprovalDecision::Approved
            } else {
                ApprovalDecision::Denied
            },
        );
    });
}

fn sanitize_host_prompt(value: &str) -> String {
    value
        .chars()
        .take(512)
        .map(|character| {
            if character == ' ' || character.is_ascii_graphic() {
                character
            } else {
                '?'
            }
        })
        .collect()
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

pub(crate) fn ok_json_with_image(mut value: Value, capture: &ImageCapture) -> Result<Value> {
    let data = fs::read(&capture.path)?;
    let digest = Sha256::digest(&data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let artifact = json!({
        "locator": format!("sha256:{digest}"),
        "sha256": digest,
        "bytes": data.len(),
        "mime_type": capture.mime_type,
    });
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::msg("image metadata must be an object"))?;
    object.insert("artifact".to_string(), artifact);
    if capture.ephemeral {
        let _ = fs::remove_file(&capture.path);
    }
    Ok(ok_json(value))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PermissionMode;
    use std::cell::Cell;

    #[test]
    fn ok_json_with_image_should_return_only_an_external_hash_locator() {
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
        let response = ok_json_with_image(json!({ "mode": "screen" }), &capture).unwrap();
        let content = response["content"].as_array().unwrap();

        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(response["structuredContent"]["artifact"]["bytes"], 4);
        assert_eq!(
            response["structuredContent"]["artifact"]["mime_type"],
            "image/png"
        );
        assert!(
            response["structuredContent"]["artifact"]["locator"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(response.to_string().find("AQIDBA==").is_none());
        assert!(
            response
                .to_string()
                .find(&path.to_string_lossy().to_string())
                .is_none()
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn handle_tool_call_should_not_invoke_adapter_when_access_policy_denies() {
        let state = AppState::new(PermissionMode::Limited, false).unwrap();
        let called = Cell::new(false);

        let result = handle_tool_call_with_adapter(
            "click",
            &json!({ "x": 10, "y": 20 }),
            "session",
            "request",
            &state,
            |_, _, _, _, _, _| {
                called.set(true);
                Ok(Some(ok_json(json!({ "adapter": true }))))
            },
        )
        .unwrap();

        assert_eq!(
            (called.get(), result["isError"].as_bool()),
            (false, Some(true))
        );
    }

    #[test]
    fn full_is_the_default_approval_mode() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();

        assert_eq!(state.approval_mode(), ApprovalMode::Full);
    }

    #[test]
    fn per_action_gates_arbitrary_executors_and_mutating_http() {
        for (tool, args) in [
            ("observe_ui", json!({})),
            (
                "click",
                json!({
                    "observation_id": "0".repeat(64),
                    "generation": 1,
                    "tag": "e0"
                }),
            ),
            (
                "set_value",
                json!({
                    "observation_id": "0".repeat(64),
                    "generation": 1,
                    "tag": "e0",
                    "value": "value"
                }),
            ),
            ("image", json!({})),
            ("write_file", json!({ "path": "file", "content": "value" })),
            (
                "edit_file",
                json!({ "path": "file", "old_string": "old", "new_string": "new" }),
            ),
            ("delete_file", json!({ "path": "file" })),
            ("clipboard_read", json!({})),
            ("permissions", json!({ "action": "grant" })),
            (
                "http_request",
                json!({ "method": "POST", "url": "https://example.com" }),
            ),
            (
                "http_request",
                json!({ "method": "delete", "url": "https://example.com" }),
            ),
        ] {
            assert!(needs_approval(tool, &args, ApprovalMode::PerAction));
        }
        for args in [
            json!({ "url": "https://example.com" }),
            json!({ "method": "GET", "url": "https://example.com" }),
            json!({ "method": "head", "url": "https://example.com" }),
        ] {
            assert!(!needs_approval(
                "http_request",
                &args,
                ApprovalMode::PerAction
            ));
        }
        assert!(!needs_approval(
            "permissions",
            &json!({}),
            ApprovalMode::PerAction
        ));
    }

    #[test]
    fn caller_echo_cannot_approve_a_pending_action() {
        let state =
            AppState::with_approval_mode(PermissionMode::Full, ApprovalMode::PerAction).unwrap();
        let calls = Cell::new(0_u32);
        let awaiting = handle_tool_call_with_adapter(
            "clipboard_write",
            &json!({ "text": "value" }),
            "session",
            "request-1",
            &state,
            |_, _, _, _, _, _| {
                calls.set(calls.get() + 1);
                Ok(Some(ok_json(json!({ "adapter": true }))))
            },
        )
        .unwrap();
        let request_id = awaiting["structuredContent"]["approvalRequestId"]
            .as_str()
            .unwrap();
        let echoed = handle_tool_call_with_adapter(
            "clipboard_write",
            &json!({
                "text": "value",
                "approval_request_id": request_id,
                "approve": true,
                "approval_token": "caller-issued"
            }),
            "session",
            "request-2",
            &state,
            |_, _, _, _, _, _| {
                calls.set(calls.get() + 1);
                Ok(Some(ok_json(json!({ "adapter": true }))))
            },
        )
        .unwrap();

        assert_eq!(
            (echoed["structuredContent"]["status"].as_str(), calls.get()),
            (Some("AWAITING_APPROVAL"), 0)
        );
    }

    #[test]
    fn trusted_approval_is_bound_and_consumed_once() {
        let state =
            AppState::with_approval_mode(PermissionMode::Full, ApprovalMode::PerAction).unwrap();
        let args = json!({ "text": "value" });
        let calls = Cell::new(0_u32);
        let awaiting = handle_tool_call_with_adapter(
            "clipboard_write",
            &args,
            "session",
            "request-1",
            &state,
            |_, _, _, _, _, _| {
                calls.set(calls.get() + 1);
                Ok(Some(ok_json(json!({ "adapter": true }))))
            },
        )
        .unwrap();
        let request_id = awaiting["structuredContent"]["approvalRequestId"]
            .as_str()
            .unwrap();
        assert!(
            state
                .decide_approval(request_id, ApprovalDecision::Approved)
                .unwrap()
        );
        let approved_args = json!({
            "text": "value",
            "approval_request_id": request_id,
        });
        let approved = handle_tool_call_with_adapter(
            "clipboard_write",
            &approved_args,
            "session",
            "request-2",
            &state,
            |_, _, _, _, _, _| {
                calls.set(calls.get() + 1);
                Ok(Some(ok_json(json!({ "adapter": true }))))
            },
        )
        .unwrap();
        let replay = handle_tool_call_with_adapter(
            "clipboard_write",
            &approved_args,
            "session",
            "request-3",
            &state,
            |_, _, _, _, _, _| {
                calls.set(calls.get() + 1);
                Ok(Some(ok_json(json!({ "adapter": true }))))
            },
        )
        .unwrap();

        assert_eq!(
            (
                awaiting["structuredContent"]["status"].as_str(),
                approved["structuredContent"]["adapter"].as_bool(),
                replay["structuredContent"]["status"].as_str(),
                calls.get(),
            ),
            (
                Some("AWAITING_APPROVAL"),
                Some(true),
                Some("APPROVAL_INVALID"),
                1,
            )
        );
    }

    #[test]
    fn trusted_approval_rejects_cross_session_and_different_payload() {
        let state =
            AppState::with_approval_mode(PermissionMode::Full, ApprovalMode::PerAction).unwrap();
        let awaiting = request_approval(
            "clipboard_write",
            &json!({ "text": "value" }),
            "session-a",
            state.inner.approval_generation.load(Ordering::Acquire),
            &state,
        )
        .unwrap();
        let request_id = awaiting["structuredContent"]["approvalRequestId"]
            .as_str()
            .unwrap();
        state
            .decide_approval(request_id, ApprovalDecision::Approved)
            .unwrap();

        for (session, text) in [("session-b", "value"), ("session-a", "different")] {
            let args = json!({ "text": text, "approval_request_id": request_id });
            assert!(matches!(
                consume_approval("clipboard_write", &args, session, &state).unwrap(),
                ApprovalStatus::Invalid
            ));
        }
        assert!(matches!(
            consume_approval(
                "clipboard_write",
                &json!({ "text": "value", "approval_request_id": request_id }),
                "session-a",
                &state,
            )
            .unwrap(),
            ApprovalStatus::Approved
        ));
    }

    #[test]
    fn approval_queue_is_bounded() {
        let state =
            AppState::with_approval_mode(PermissionMode::Full, ApprovalMode::PerAction).unwrap();
        let generation = state.inner.approval_generation.load(Ordering::Acquire);
        let approval = Approval {
            session_id: "session".to_string(),
            tool_name: "clipboard_write".to_string(),
            clean_args: json!({ "text": "value" }),
            generation,
            expires_at: Instant::now() + Duration::from_secs(60),
            decision: ApprovalDecision::Pending,
        };
        let mut approvals = state.approvals_lock().unwrap();
        for index in 0..MAX_PENDING_APPROVALS {
            approvals.insert(index.to_string(), approval.clone());
        }
        drop(approvals);

        let result = request_approval(
            "clipboard_write",
            &json!({ "text": "value" }),
            "session",
            generation,
            &state,
        )
        .unwrap();
        assert_eq!(
            result["structuredContent"]["status"].as_str(),
            Some("APPROVAL_QUEUE_FULL")
        );
    }

    #[test]
    fn permission_mode_changes_invalidate_pending_approvals() {
        let state =
            AppState::with_approval_mode(PermissionMode::Full, ApprovalMode::PerAction).unwrap();
        let generation = state.inner.approval_generation.load(Ordering::Acquire);
        let awaiting = request_approval(
            "clipboard_write",
            &json!({ "text": "value" }),
            "session",
            generation,
            &state,
        )
        .unwrap();
        let request_id = awaiting["structuredContent"]["approvalRequestId"]
            .as_str()
            .unwrap();

        state.set_mode(PermissionMode::Limited);

        assert_ne!(
            state.inner.approval_generation.load(Ordering::Acquire),
            generation
        );
        assert!(state.approvals_lock().unwrap().is_empty());
        assert!(matches!(
            consume_approval(
                "clipboard_write",
                &json!({ "text": "value", "approval_request_id": request_id }),
                "session",
                &state,
            )
            .unwrap(),
            ApprovalStatus::Invalid
        ));
    }

    #[test]
    fn permission_mode_changes_cancel_and_serialize_active_dispatch() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let worker_state = state.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (cancelled_tx, cancelled_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            handle_tool_call_with_adapter(
                "clipboard_write",
                &json!({ "text": "value" }),
                "session",
                "request",
                &worker_state,
                |_, _, _, _, adapter_state, cancellation| {
                    started_tx.send(()).unwrap();
                    while !cancellation.is_cancelled() {
                        std::thread::yield_now();
                    }
                    assert_eq!(adapter_state.mode(), PermissionMode::Full);
                    cancelled_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(Some(error_result("cancelled")))
                },
            )
            .unwrap()
        });
        started_rx.recv().unwrap();
        let changer_state = state.clone();
        let (changed_tx, changed_rx) = std::sync::mpsc::channel();
        let changer = std::thread::spawn(move || {
            changer_state.set_mode(PermissionMode::Limited);
            changed_tx.send(()).unwrap();
        });
        cancelled_rx.recv().unwrap();

        assert!(changed_rx.try_recv().is_err());
        release_tx.send(()).unwrap();
        let result = worker.join().unwrap();
        changer.join().unwrap();
        changed_rx.recv().unwrap();
        assert_eq!(state.mode(), PermissionMode::Limited);
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn permission_transition_rejects_registration_after_cancellation_sweep() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let dispatch = state.inner.dispatch.read().unwrap();
        let changer_state = state.clone();
        let changer = std::thread::spawn(move || {
            changer_state.set_mode(PermissionMode::Limited);
        });
        let timeout = Instant::now() + Duration::from_secs(1);
        while !state.inner.mode_transition.load(Ordering::Acquire) {
            assert!(Instant::now() < timeout);
            std::thread::yield_now();
        }
        let cancellation = CancellationToken::default();
        assert_eq!(
            register_active_request(
                &state,
                &("session".to_string(), "request".to_string()),
                &cancellation,
            ),
            Err("access mode transition in progress")
        );
        drop(dispatch);
        changer.join().unwrap();
        assert_eq!(state.mode(), PermissionMode::Limited);
    }

    #[test]
    fn cancellation_before_registration_is_preserved() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        state.cancel_request("session", "request");

        let result = handle_tool_call_with_adapter(
            "clipboard_write",
            &json!({ "text": "value" }),
            "session",
            "request",
            &state,
            |_, _, _, _, _, _| panic!("cancelled request reached the adapter"),
        )
        .unwrap();

        assert_eq!(
            result["structuredContent"]["status"],
            "CANCELLED_BEFORE_EFFECT"
        );
        assert_eq!(result["structuredContent"]["retry_safe"], true);
    }

    #[test]
    fn cancellation_notification_cancels_the_matching_request() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let cancellation = CancellationToken::default();
        state.inner.active_requests.lock().unwrap().active.insert(
            ("session".to_string(), "1".to_string()),
            cancellation.clone(),
        );

        let response = handle_json_rpc(
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": { "requestId": 1 }
            })
            .to_string(),
            "session",
            state,
        )
        .unwrap();

        assert!(response.is_none());
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn host_prompt_rejects_terminal_control_sequences() {
        assert_eq!(
            sanitize_host_prompt("command\n\u{1b}[2J\u{202e}secret"),
            "command??[2J?secret"
        );
        assert_eq!(sanitize_host_prompt(&"a".repeat(513)).len(), 512);
    }
}
