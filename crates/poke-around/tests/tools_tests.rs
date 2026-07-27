use poke_around::mcp_tools::tools_json;
use serde_json::{Value, json};

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
            "observe_ui",
            "click",
            "set_value",
            "image",
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
        "see",
        "doctor",
        "press",
        "type",
        "paste",
        "hotkey",
        "scroll",
        "move",
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
    for name in [
        "write_file",
        "clipboard_read",
        "observe_ui",
        "click",
        "set_value",
        "image",
    ] {
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

#[test]
fn computer_use_tool_schemas_are_strict_and_bounded() {
    let tools: Value = serde_json::from_str(&tools_json()).expect("tools json parses");
    for (name, expected) in [
        ("observe_ui", vec!["approval_request_id"]),
        (
            "click",
            vec![
                "approval_request_id",
                "generation",
                "interaction_mode",
                "observation_id",
                "tag",
            ],
        ),
        (
            "set_value",
            vec![
                "approval_request_id",
                "generation",
                "interaction_mode",
                "observation_id",
                "tag",
                "value",
            ],
        ),
        ("image", vec!["approval_request_id", "retina"]),
    ] {
        let schema = &tools
            .as_array()
            .expect("tools is array")
            .iter()
            .find(|tool| tool["name"] == name)
            .expect("semantic tool is advertised")["inputSchema"];
        assert_eq!(schema["additionalProperties"], false);
        let properties = schema["properties"]
            .as_object()
            .expect("semantic schema has properties");
        assert_eq!(
            properties.keys().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(properties["approval_request_id"]["maxLength"], 32);
        if matches!(name, "click" | "set_value") {
            assert_eq!(
                properties["generation"]["maximum"],
                9_007_199_254_740_991_u64
            );
            assert_eq!(
                properties["interaction_mode"]["enum"],
                json!(["interactive", "background_only"])
            );
        }
    }
}

#[test]
fn tool_schema_includes_descriptions() {
    let tools: Value = serde_json::from_str(&tools_json()).expect("tools json parses");
    let tools_array = tools.as_array().expect("tools is array");
    assert!(!tools_array.is_empty(), "tools array is empty");
    for tool in tools_array {
        let name = tool["name"].as_str().expect("tool has name");
        let description = tool["description"]
            .as_str()
            .unwrap_or_else(|| panic!("tool {} has description", name));
        assert!(
            !description.is_empty(),
            "tool {} description is empty",
            name
        );
    }
}
