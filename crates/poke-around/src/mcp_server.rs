use crate::mcp::handle_json_rpc;
use crate::{Error, Result};
use rand::Rng;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use crate::mcp::AppState;

const MAX_CONCURRENT_CONNECTIONS: usize = 32;
const MAX_HTTP_BODY_SIZE: usize = 10 * 1024 * 1024;

pub fn start_server(state: AppState, bearer: &str) -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let connections = Arc::new(AtomicUsize::new(0));
    let bearer = bearer.to_string();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = state.clone();
            let connections = connections.clone();
            let bearer = bearer.clone();
            if !try_acquire_connection(&connections) {
                eprintln!("mcp: rejecting connection, max concurrent connections reached");
                continue;
            }
            thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_connection(stream, state, &bearer)
                }));
                connections.fetch_sub(1, Ordering::AcqRel);
                match result {
                    Ok(Err(err)) => eprintln!("mcp connection error: {err}"),
                    Err(_) => eprintln!("mcp connection error: request handler panicked"),
                    Ok(Ok(())) => {}
                }
            });
        }
    });
    Ok(port)
}

fn try_acquire_connection(connections: &AtomicUsize) -> bool {
    connections
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
            (active < MAX_CONCURRENT_CONNECTIONS).then_some(active + 1)
        })
        .is_ok()
}

pub fn new_bearer_capability() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn start_tunnel_relay(mcp_url: &str, bearer: &str) -> Result<String> {
    let target = url::Url::parse(mcp_url)?;
    let port = target
        .port_or_known_default()
        .ok_or_else(|| Error::msg("MCP URL is missing a port"))?;
    if target.host_str() != Some("127.0.0.1") {
        return Err(Error::msg("MCP relay target must be loopback"));
    }
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let relay_port = listener.local_addr()?.port();
    let path_capability = new_bearer_capability();
    let relay_path = format!("/{path_capability}/mcp");
    let expected_path = relay_path.clone();
    let bearer = bearer.to_string();
    let connections = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let bearer = bearer.clone();
            let expected_path = expected_path.clone();
            let connections = connections.clone();
            if !try_acquire_connection(&connections) {
                continue;
            }
            thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_relay_connection(stream, port, &expected_path, &bearer)
                }));
                connections.fetch_sub(1, Ordering::AcqRel);
                match result {
                    Ok(Err(err)) => eprintln!("mcp relay error: {err}"),
                    Err(_) => eprintln!("mcp relay error: request handler panicked"),
                    Ok(Ok(())) => {}
                }
            });
        }
    });
    Ok(format!("http://127.0.0.1:{relay_port}{relay_path}"))
}

fn handle_relay_connection(
    mut stream: TcpStream,
    target_port: u16,
    expected_path: &str,
    bearer: &str,
) -> Result<()> {
    apply_socket_timeouts(&stream)?;
    let request = read_http_request(&mut stream)?;
    if request.path != expected_path {
        let response = HttpResponse::not_found();
        write_http_response(
            &mut stream,
            response.status,
            &response.body,
            &response.headers,
        )?;
        return Ok(());
    }
    let mut target = TcpStream::connect(("127.0.0.1", target_port))?;
    apply_socket_timeouts(&target)?;
    write!(target, "{} /mcp HTTP/1.1\r\n", request.method)?;
    write!(target, "Host: 127.0.0.1:{target_port}\r\n")?;
    for (name, value) in request.headers {
        if matches!(
            name.as_str(),
            "authorization" | "connection" | "content-length" | "host"
        ) || !is_safe_header_component(&name)
            || !is_safe_header_component(&value)
        {
            continue;
        }
        write!(target, "{name}: {value}\r\n")?;
    }
    write!(
        target,
        "Authorization: Bearer {bearer}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        request.body.len(),
        request.body
    )?;
    std::io::copy(&mut target, &mut stream)?;
    Ok(())
}

fn apply_socket_timeouts(stream: &TcpStream) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    Ok(())
}

fn is_safe_header_component(value: &str) -> bool {
    !value
        .bytes()
        .any(|byte| byte == b'\r' || byte == b'\n' || byte == 0)
}

fn handle_connection(mut stream: TcpStream, state: AppState, bearer: &str) -> Result<()> {
    apply_socket_timeouts(&stream)?;
    let request = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err(err) => {
            let body = json!({ "error": err.to_string() }).to_string();
            write_http_response(&mut stream, 400, &body, &[])?;
            return Ok(());
        }
    };
    let path = normalized_path(&request.path);
    let session_id = request
        .headers
        .get("mcp-session-id")
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(new_mcp_session_id);
    let http_response = route_http_request(&request, &path, &session_id, state, bearer)?;
    write_http_response(
        &mut stream,
        http_response.status,
        &http_response.body,
        &http_response.headers,
    )?;
    Ok(())
}

fn route_http_request(
    request: &HttpRequest,
    path: &str,
    session_id: &str,
    state: AppState,
    bearer: &str,
) -> Result<HttpResponse> {
    if request.method == "GET" && matches!(path, "/" | "/health") {
        Ok(HttpResponse::json(200, json!({ "ok": true })))
    } else if !request_has_bearer(request, bearer) {
        Ok(HttpResponse::unauthorized())
    } else if request.method == "OPTIONS" {
        Ok(HttpResponse::no_content())
    } else if (request.method == "GET" && path == "/mcp")
        || (request.method == "DELETE" && matches!(path, "/" | "/mcp"))
    {
        Ok(HttpResponse::method_not_allowed())
    } else if request.method == "POST" && matches!(path, "/" | "/mcp") {
        match handle_json_rpc(&request.body, session_id, state)? {
            Some(body) => {
                let mut response = HttpResponse::json(200, body);
                if request_contains_initialize(&request.body) {
                    let new_session = new_mcp_session_id();
                    response
                        .headers
                        .push(("Mcp-Session-Id".to_string(), new_session));
                }
                Ok(response)
            }
            None => Ok(HttpResponse::accepted()),
        }
    } else {
        Ok(HttpResponse::not_found())
    }
}

fn request_has_bearer(request: &HttpRequest, bearer: &str) -> bool {
    request
        .headers
        .get("authorization")
        .and_then(|value| value.split_once(' '))
        .is_some_and(|(scheme, value)| {
            scheme.eq_ignore_ascii_case("bearer")
                && constant_time_eq(value.as_bytes(), bearer.as_bytes())
        })
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn request_contains_initialize(body: &str) -> bool {
    let Ok(request) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    if let Some(items) = request.as_array() {
        return items
            .iter()
            .any(|item| item.get("method").and_then(Value::as_str) == Some("initialize"));
    }
    request.get("method").and_then(Value::as_str) == Some("initialize")
}

fn new_mcp_session_id() -> String {
    let mut bytes = [0_u8; 16];
    rand::rng().fill(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    if path.starts_with("/.well-known/") {
        return path;
    }
    if tunnel_prefixed_mcp_path(&path) {
        "/mcp".to_string()
    } else {
        path
    }
}

fn tunnel_prefixed_mcp_path(path: &str) -> bool {
    let Some(prefix) = path.strip_suffix("/mcp") else {
        return false;
    };
    prefix.is_empty() || prefix.starts_with('/') && prefix.len() > 1
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    apply_socket_timeouts(stream)?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut search_from = 0usize;
    let mut header_end = None;
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        let start = search_from.saturating_sub(3);
        if let Some(relative) = bytes[start..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
        {
            header_end = Some(start + relative);
            break;
        }
        search_from = bytes.len();
        if bytes.len() > 1024 * 1024 {
            return Err(Error::msg("request headers too large"));
        }
    }
    let header_end = header_end.ok_or_else(|| Error::msg("invalid http request"))?;
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
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if is_safe_header_component(&name) && is_safe_header_component(&value) {
                headers.insert(name, value);
            }
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_HTTP_BODY_SIZE {
        return Err(Error::msg("request body too large"));
    }
    let body_start = header_end + 4;
    while bytes.len().saturating_sub(body_start) < content_length {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.len().saturating_sub(body_start) < content_length {
        return Err(Error::msg("incomplete request body"));
    }
    let body = String::from_utf8_lossy(&bytes[body_start..body_start + content_length]).to_string();
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
    extra_headers: &[(String, String)],
) -> Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        401 => "Unauthorized",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    write!(stream, "HTTP/1.1 {status} {reason}\r\n")?;
    for (name, value) in extra_headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    if body.is_empty() && status != 200 {
        write!(stream, "Content-Length: 0\r\nConnection: close\r\n\r\n")?;
    } else {
        write!(
            stream,
            "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )?;
    }
    Ok(())
}

struct HttpResponse {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
}

impl HttpResponse {
    fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: body.to_string(),
            headers: Vec::new(),
        }
    }

    fn no_content() -> Self {
        Self {
            status: 204,
            body: String::new(),
            headers: Vec::new(),
        }
    }

    fn accepted() -> Self {
        Self {
            status: 202,
            body: String::new(),
            headers: Vec::new(),
        }
    }

    fn method_not_allowed() -> Self {
        Self {
            status: 405,
            body: String::new(),
            headers: Vec::new(),
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: 401,
            body: json!({ "error": "unauthorized" }).to_string(),
            headers: vec![("WWW-Authenticate".to_string(), "Bearer".to_string())],
        }
    }

    fn not_found() -> Self {
        Self {
            status: 404,
            body: json!({ "error": "not found" }).to_string(),
            headers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::AppState;
    use crate::policy::PermissionMode;
    use std::time::Duration;

    #[test]
    fn bearer_compare_is_length_sensitive_and_value_sensitive() {
        assert!(constant_time_eq(b"abcd", b"abcd"));
        assert!(!constant_time_eq(b"abcd", b"abce"));
        assert!(!constant_time_eq(b"abcd", b"abc"));
    }

    #[test]
    fn header_components_reject_crlf_and_nul() {
        assert!(is_safe_header_component("x-request-id"));
        assert!(!is_safe_header_component("bad\rname"));
        assert!(!is_safe_header_component("bad\nvalue"));
        assert!(!is_safe_header_component("bad\0value"));
    }

    #[test]
    fn handle_connection_read_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server_stream, _) = listener.accept().unwrap();

        rustix::net::sockopt::set_socket_linger(&client, Some(Duration::ZERO)).unwrap();

        drop(client);
        std::thread::sleep(Duration::from_millis(100));

        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let result = handle_connection(server_stream, state, "test-bearer");

        assert!(
            result.is_err(),
            "Expected an error when handling a reset connection, got {:?}",
            result
        );
    }

    #[test]
    fn tunnel_relay_requires_secret_path_and_adds_bearer() {
        let state = AppState::new(PermissionMode::Full, false).unwrap();
        let bearer = "test-bearer";
        let port = start_server(state, bearer).unwrap();
        let relay_url =
            start_tunnel_relay(&format!("http://127.0.0.1:{port}/mcp"), bearer).unwrap();
        let relay = url::Url::parse(&relay_url).unwrap();
        let relay_port = relay.port().unwrap();

        let mut rejected = TcpStream::connect(("127.0.0.1", relay_port)).unwrap();
        rejected
            .write_all(b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        let mut rejected_response = String::new();
        rejected.read_to_string(&mut rejected_response).unwrap();
        assert!(rejected_response.starts_with("HTTP/1.1 404"));

        let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let mut accepted = TcpStream::connect(("127.0.0.1", relay_port)).unwrap();
        write!(
            accepted,
            "POST {} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\n\r\n{}",
            relay.path(),
            body.len(),
            body
        )
        .unwrap();
        let mut accepted_response = String::new();
        accepted.read_to_string(&mut accepted_response).unwrap();
        assert!(accepted_response.starts_with("HTTP/1.1 200 OK"));
        assert!(accepted_response.contains("read_file"));
    }
}
