use poke_around::tools::{TOOL_NAMES, tools_json};
use serde_json::Value;

#[test]
fn tool_schema_includes_poke_gate_and_poke_around_tools() {
    let tools: Value = serde_json::from_str(tools_json()).expect("tools json parses");
    let names = tools
        .as_array()
        .expect("tools is array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool has name"))
        .collect::<Vec<_>>();
    assert_eq!(names, TOOL_NAMES);
}
