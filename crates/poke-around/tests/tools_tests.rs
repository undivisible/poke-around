use poke_around::mcp_tools::tools_json;
use serde_json::Value;

#[test]
fn tool_schema_includes_poke_gate_and_poke_around_tools() {
    let tools: Value = serde_json::from_str(&tools_json()).expect("tools json parses");
    let names = tools
        .as_array()
        .expect("tools is array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool has name"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "read_file",
            "write_file",
            "list_directory",
            "system_info",
            "edit_file",
            "delete_file",
            "list_screens",
            "permissions",
            "clipboard_read",
            "clipboard_write",
        ]
    );
}

#[test]
fn tool_schema_does_not_advertise_unavailable_target_effects() {
    let tools: Value = serde_json::from_str(&tools_json()).expect("tools json parses");
    for name in [
        "run_command",
        "network_speed",
        "read_image",
        "run_agent",
        "take_screenshot",
        "web_fetch",
        "http_request",
        "image",
        "see",
        "doctor",
        "click",
        "press",
        "type",
        "paste",
        "hotkey",
        "scroll",
        "move",
        "set_value",
        "perform_action",
        "window",
        "app",
        "open",
        "menu",
        "swipe",
        "drag",
        "run",
        "clean",
        "sleep",
    ] {
        assert!(
            tools
                .as_array()
                .unwrap()
                .iter()
                .all(|tool| tool["name"] != name)
        );
    }
    for name in ["write_file", "clipboard_read"] {
        let tool = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        assert!(
            tool["inputSchema"]["properties"]
                .as_object()
                .unwrap()
                .contains_key("approval_request_id")
        );
    }
}
