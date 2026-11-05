#[cfg(target_os = "windows")]
pub fn log_gui_session_readiness(verbose: bool) {
    use std::process::{Command, Stdio};

    let script = r#"
$session = (Get-Process -Id $PID).SessionId
$interactive = [Environment]::UserInteractive
$sessionName = [Environment]::GetEnvironmentVariable('SESSIONNAME')
[pscustomobject]@{
  session_id = $session
  user_interactive = $interactive
  session_name = $sessionName
} | ConvertTo-Json -Compress
"#;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script.trim()])
        .stdin(Stdio::null())
        .output();

    let Ok(output) = output else {
        eprintln!("poke-around: could not inspect Windows session (powershell unavailable)");
        return;
    };

    if !output.status.success() {
        if verbose {
            eprintln!(
                "poke-around: session probe failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        return;
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if verbose {
        eprintln!("poke-around: Windows session {text}");
    }

    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return,
    };
    let session_id = value.get("session_id").and_then(serde_json::Value::as_u64);
    let interactive = value
        .get("user_interactive")
        .and_then(serde_json::Value::as_bool);
    let session_name = value
        .get("session_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    if session_id == Some(0) {
        eprintln!(
            "poke-around WARNING: running in session 0 (non-interactive). \
             GUI tools launch invisible windows that will not appear on your monitor. \
             Start poke-around from an interactive desktop terminal instead."
        );
        return;
    }

    if interactive == Some(false) {
        eprintln!(
            "poke-around WARNING: process is not user-interactive. \
             GUI automation may not reach your desktop."
        );
    }

    if session_name.eq_ignore_ascii_case("Services") {
        eprintln!(
            "poke-around WARNING: SESSIONNAME=Services indicates a non-interactive context. \
             GUI tools will not paint on your physical display."
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn log_gui_session_readiness(_verbose: bool) {}

pub fn diagnostic_permissions(mut value: serde_json::Value) -> serde_json::Value {
    #[cfg(target_os = "linux")]
    if let Some(object) = value.as_object_mut()
        && let Some(available) = object.remove("accessibility")
    {
        object.insert("xdotool_available".into(), available);
        object.insert("accessibility".into(), serde_json::Value::Null);
        object.insert(
            "accessibility_note".into(),
            serde_json::json!(
                "xdotool availability does not verify desktop accessibility or permission"
            ),
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = &mut value;
    value
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn linux_diagnostics_do_not_treat_xdotool_as_accessibility_permission() {
        for available in [true, false] {
            let permissions =
                diagnostic_permissions(json!({"accessibility": available, "clipboard": true}));
            assert_eq!(permissions["accessibility"], json!(null));
            assert_eq!(permissions["xdotool_available"], available);
            assert_eq!(permissions["clipboard"], true);
        }
    }
}
