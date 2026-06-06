use poke_around::mcp::{AppState, start_server};
use poke_around::policy::PermissionMode;
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
fn get_mcp_should_return_health_response() {
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

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#"{"ok":true}"#));
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
fn initialized_notification_without_id_should_return_json_rpc_response() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""id":null"#));
    assert!(response.contains(r#""result":{}"#));
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
fn batch_unknown_method_without_id_should_return_json_body() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"[{"jsonrpc":"2.0","method":"unknown/notification"}]"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""id":null"#));
    assert!(response.contains(r#""error""#));
}

#[test]
fn tool_call_without_id_should_return_json_rpc_response() {
    let state = AppState::new(PermissionMode::Full, false).expect("state should initialize");
    let port = start_server(state).expect("server should start");
    let response = post_json(
        port,
        r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"system_info","arguments":{}}}"#,
    );

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(r#""id":null"#));
    assert!(response.contains(r#""structuredContent""#));
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
