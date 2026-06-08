use poke_around::policy::{PermissionMode, evaluate_access_policy, split_command_segments};
use serde_json::json;

#[test]
fn piped_read_only_commands_remain_allowed_in_limited_mode() {
    let reason = evaluate_access_policy(
        "run_command",
        &json!({ "command": "pwd && ls -la | head" }),
        PermissionMode::Limited,
    );
    assert_eq!(reason, None);
}

#[test]
fn piped_python_execution_stays_blocked_in_limited_mode() {
    let reason = evaluate_access_policy(
        "run_command",
        &json!({ "command": "python3 script.py | cat" }),
        PermissionMode::Limited,
    );
    assert_eq!(
        reason,
        Some("Command 'python3' is not permitted in this mode.".to_string())
    );
}

#[test]
fn split_command_segments_do_not_break_on_pipes() {
    let segments: Vec<_> = split_command_segments("ls -la | head").collect();
    assert_eq!(segments, vec!["ls -la | head"]);
}
