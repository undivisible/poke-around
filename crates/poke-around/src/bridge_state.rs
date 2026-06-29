use crate::{Result, config};
use serde_json::{Map, Value};

const MAX_CONN_HISTORY: usize = 10;

pub(crate) fn read_state() -> Result<Map<String, Value>> {
    match std::fs::read_to_string(config::state_path()?) {
        Ok(data) => Ok(serde_json::from_str::<Value>(&data)?
            .as_object()
            .cloned()
            .unwrap_or_default()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn patch_state<const N: usize>(updates: [(&str, Value); N]) -> Result<()> {
    let mut state = read_state()?;
    for (key, value) in updates {
        state.insert(key.to_string(), value);
    }
    write_state(&state)
}

pub(crate) fn remove_state_key(key: &str) -> Result<()> {
    let mut state = read_state()?;
    state.remove(key);
    write_state(&state)
}

fn write_state(state: &Map<String, Value>) -> Result<()> {
    let path = config::state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = {
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("state.json");
        path.parent()
            .map(|parent| parent.join(format!("{file_name}.tmp")))
            .unwrap_or_else(|| path.with_extension("tmp"))
    };
    std::fs::write(&tmp_path, serde_json::to_string_pretty(state)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

pub(crate) fn record_connection(connection_id: &str) -> Result<()> {
    let state = read_state()?;
    let mut history = state
        .get("connectionHistory")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !history
        .iter()
        .filter_map(Value::as_str)
        .any(|known| known == connection_id)
    {
        history.insert(0, Value::String(connection_id.to_string()));
    }
    history.truncate(MAX_CONN_HISTORY);
    patch_state([
        ("connectionId", Value::String(connection_id.to_string())),
        ("connectionHistory", Value::Array(history)),
    ])
}

pub(crate) fn log_status(message: &str) {
    let now = chrono::Local::now().format("%H:%M:%S");
    eprintln!("[{now}] {message}");
}
