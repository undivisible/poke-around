use poke_around::mcp::{AppState, start_server};
use poke_around::policy::PermissionMode;
use std::io::{Read, Write};
use std::net::TcpStream;

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
