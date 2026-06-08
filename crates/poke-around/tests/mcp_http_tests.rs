use poke_around::mcp::{AppState, start_server};
use poke_around::policy::PermissionMode;
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;

fn post_json(port: u16, body: &str) -> String {
    post_json_path(port, "/mcp", body)
}

fn post_json_path(port: u16, path: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("server should accept");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(request.as_bytes())
        .expect("request should write");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("response should read");
    response
}

#[test]
fn get_mcp_should_return_method_not_allowed() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("server should accept");
    stream
        .write_all(b"GET /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .expect("request should write");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("response should read");

    assert!(response.starts_with("HTTP/1.1 405"));
    assert!(!response.contains(r#"{"ok":true}"#));
}

#[test]
fn initialize_should_return_mcp_session_id_header() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.to_ascii_lowercase().contains("mcp-session-id:"));
}

#[test]
fn mcp_batch_should_return_tools_after_initialized_notification() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"[{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}},{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","id":2,"method":"tools/list"}]"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""id":1"#));
    assert!(response.contains(r#""id":2"#));
    assert!(response.contains(r#""name":"run_command""#));
    assert!(!response.contains(r#"notifications/initialized"#));
}

#[test]
fn initialized_notification_without_id_should_return_no_content() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );

    assert!(response.starts_with("HTTP/1.1 202 Accepted"));
}

#[test]
fn initialized_request_with_id_should_return_json_rpc_response() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"{"jsonrpc":"2.0","id":3,"method":"notifications/initialized"}"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""id":3"#));
    assert!(response.contains(r#""result":{}"#));
}

#[test]
fn batch_unknown_method_without_id_should_return_accepted() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"[{"jsonrpc":"2.0","method":"unknown/notification"}]"#,
    );

    assert!(response.starts_with("HTTP/1.1 202 Accepted"));
}

#[test]
fn tool_call_without_id_should_return_no_content() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"system_info","arguments":{}}}"#,
    );

    assert!(response.starts_with("HTTP/1.1 202 Accepted"));
}

#[test]
fn incomplete_body_should_return_bad_request() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("server should accept");
    stream
        .write_all(
            b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 20\r\n\r\n{}",
        )
        .expect("request should write");
    stream
        .shutdown(std::net::Shutdown::Write)
        .expect("write side should close");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("response should read");

    assert!(response.starts_with("HTTP/1.1 400 Error"));
    assert!(response.contains("incomplete request body"));
}

#[test]
fn mcp_absolute_form_request_target_should_return_tools() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json_path(
        port,
        &format!("http://127.0.0.1:{port}/mcp"),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""name":"run_command""#));
}

#[test]
fn mcp_root_request_target_should_return_tools() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json_path(
        port,
        "/",
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""name":"run_command""#));
}

#[test]
fn oauth_well_known_paths_should_not_be_collapsed_to_health_check() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    for path in [
        "/.well-known/oauth-protected-resource/mcp",
        "/.well-known/oauth-authorization-server/mcp",
        "/.well-known/openid-configuration/mcp",
        "/mcp/.well-known/openid-configuration",
    ] {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("server should accept");
        let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .expect("request should write");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("response should read");

        assert!(
            response.starts_with("HTTP/1.1 404"),
            "expected 404 for {path}, got: {}",
            response.lines().next().unwrap_or_default()
        );
        assert!(
            !response.contains(r#""ok":true"#),
            "well-known path {path} must not return health check payload"
        );
    }
}

#[test]
fn mcp_prefixed_request_target_should_return_tools() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json_path(
        port,
        "/b92095cb-bf54/mcp",
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""name":"run_command""#));
}

#[test]
fn read_image_tool_should_return_mcp_image_content_over_http() {
    let path = std::env::temp_dir().join(format!(
        "poke-around-http-read-image-{}.png",
        std::process::id()
    ));
    fs::write(&path, [1_u8, 2, 3, 4]).expect("image fixture should write");
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let body = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "read_image",
            "arguments": { "path": path }
        }
    })
    .to_string();
    let response = post_json(port, &body);
    let (_, body) = response
        .split_once("\r\n\r\n")
        .expect("response should include body separator");
    let value: Value = serde_json::from_str(body).expect("response body should parse");
    let result = &value["result"];
    let content = result["content"]
        .as_array()
        .expect("content should be array");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["mimeType"], "image/png");
    assert_eq!(content[1]["data"], "AQIDBA==");
    assert!(result["structuredContent"].get("base64").is_none());

    let _ = fs::remove_file(path);
}

#[test]
fn approval_token_should_not_allow_hidden_auto_approve_escalation() {
    let path =
        std::env::temp_dir().join(format!("poke-around-approval-{}.txt", std::process::id()));
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let first = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": path,
                "content": "first"
            }
        }
    })
    .to_string();
    let response = post_json(port, &first);
    let (_, body) = response
        .split_once("\r\n\r\n")
        .expect("response should include body separator");
    let value: Value = serde_json::from_str(body).expect("response body should parse");
    let token = value["result"]["structuredContent"]["approvalToken"]
        .as_str()
        .expect("approval token should exist");
    let second = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "write_file",
            "arguments": {
                "path": path,
                "content": "first",
                "approve": true,
                "approval_token": token,
                "remember_all_risky": true
            }
        }
    })
    .to_string();

    let response = post_json(port, &second);

    assert!(response.contains("AWAITING_APPROVAL"));
    assert!(!path.exists());
}
