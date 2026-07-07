use crate::{Result, config};
use serde_json::{Map, Value};

const MAX_CONN_HISTORY: usize = 10;

pub(crate) async fn read_state() -> Result<Map<String, Value>> {
    let path = config::state_path()?;
    let result = tokio::task::spawn_blocking(move || std::fs::read_to_string(path))
        .await
        .unwrap_or_else(|err| Err(std::io::Error::other(err.to_string())));
    match result {
        Ok(data) => Ok(serde_json::from_str::<Value>(&data)?
            .as_object()
            .cloned()
            .unwrap_or_default()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(err) => Err(err.into()),
    }
}

pub(crate) async fn patch_state<const N: usize>(updates: [(&str, Value); N]) -> Result<()> {
    let mut state = read_state().await?;
    for (key, value) in updates {
        state.insert(key.to_string(), value);
    }
    write_state(&state).await
}

pub(crate) async fn remove_state_key(key: &str) -> Result<()> {
    let mut state = read_state().await?;
    state.remove(key);
    write_state(&state).await
}

async fn write_state(state: &Map<String, Value>) -> Result<()> {
    let path = config::state_path()?;
    let state_json = serde_json::to_string_pretty(state)?;

    tokio::task::spawn_blocking(move || -> Result<()> {
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
        std::fs::write(&tmp_path, state_json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    })
    .await
    .unwrap()?;
    Ok(())
}

pub(crate) async fn record_connection(connection_id: &str) -> Result<()> {
    let state = read_state().await?;
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
    .await
}

pub(crate) fn log_status(message: &str) {
    let now = chrono::Local::now().format("%H:%M:%S");
    eprintln!("[{now}] {message}");
}
