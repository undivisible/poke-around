use poke_around::windows_automation;
use serde_json::json;

#[test]
fn parses_explicit_point_arguments() {
    let point = windows_automation::point_from_args(&json!({ "x": 42, "y": 24 }), None)
        .expect("point should parse");

    assert_eq!(point.x, 42);
    assert_eq!(point.y, 24);
}

#[test]
fn converts_hotkey_names_to_sendkeys_chords() {
    assert_eq!(
        windows_automation::send_keys_for_hotkey("ctrl+shift+p"),
        "^+p"
    );
    assert_eq!(windows_automation::send_keys_for_hotkey("alt+f4"), "%{F4}");
}

#[test]
fn normalizes_named_keys_for_sendkeys() {
    assert_eq!(windows_automation::send_keys_for_key("enter"), "{ENTER}");
    assert_eq!(windows_automation::send_keys_for_key("escape"), "{ESC}");
    assert_eq!(windows_automation::send_keys_for_key("a"), "a");
}
