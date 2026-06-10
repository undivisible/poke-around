use poke_around::tools::tools_json;
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
            "run_command",
            "network_speed",
            "read_file",
            "write_file",
            "list_directory",
            "system_info",
            "read_image",
            "run_agent",
            "take_screenshot",
            "edit_file",
            "web_fetch",
            "http_request",
            "delete_file",
            "image",
            "see",
            "list_screens",
            "permissions",
            "click",
            "press",
            "type",
            "paste",
            "hotkey",
            "scroll",
            "swipe",
            "drag",
            "move",
            "set_value",
            "perform_action",
            "window",
            "app",
            "open",
            "menu",
            "clipboard_read",
            "clipboard_write",
            "run",
            "sleep",
            "clean",
        ]
    );
}
