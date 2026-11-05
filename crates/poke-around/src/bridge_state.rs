use crate::{Result, config};
use atomicwrites::{AllowOverwrite, AtomicFile};
use fs2::FileExt;
use serde_json::{Map, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

static STATE_MUTEX: Mutex<()> = Mutex::new(());

fn join_blocking<T>(result: std::result::Result<Result<T>, tokio::task::JoinError>) -> Result<T> {
    result.map_err(|_| crate::Error::msg("state worker task failed"))?
}

pub(crate) async fn read_state() -> Result<Map<String, Value>> {
    join_blocking(tokio::task::spawn_blocking(|| with_state_lock(read_state_unlocked)).await)
}

fn read_state_unlocked() -> Result<Map<String, Value>> {
    match std::fs::read_to_string(config::state_path()?) {
        Ok(data) => Ok(serde_json::from_str::<Value>(&data)?
            .as_object()
            .cloned()
            .unwrap_or_default()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(err) => Err(err.into()),
    }
}

pub(crate) async fn patch_state(updates: Vec<(String, Value)>) -> Result<()> {
    join_blocking(
        tokio::task::spawn_blocking(move || {
            with_state_lock(|| {
                let mut state = read_state_unlocked()?;
                for (key, value) in updates {
                    state.insert(key, value);
                }
                write_state_unlocked(&state)
            })
        })
        .await,
    )
}

pub(crate) async fn forget_connections(ids: Vec<String>) -> Result<()> {
    join_blocking(
        tokio::task::spawn_blocking(move || {
            with_state_lock(|| {
                let mut state = read_state_unlocked()?;
                forget_connections_in_state(&mut state, &ids);
                write_state_unlocked(&state)
            })
        })
        .await,
    )
}

fn forget_connections_in_state(state: &mut Map<String, Value>, ids: &[String]) {
    if state
        .get("connectionId")
        .and_then(Value::as_str)
        .is_some_and(|id| ids.iter().any(|deleted| deleted == id))
    {
        state.remove("connectionId");
    }
    if let Some(history) = state
        .get_mut("connectionHistory")
        .and_then(Value::as_array_mut)
    {
        history.retain(|value| {
            !value
                .as_str()
                .is_some_and(|id| ids.iter().any(|deleted| deleted == id))
        });
    }
}

fn with_state_lock<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let _process_guard = STATE_MUTEX
        .lock()
        .map_err(|_| crate::Error::msg("state lock poisoned"))?;
    let directory = config::ensure_private_config_dir()?;
    let lock_path = directory.join("state.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    config::restrict_private_file(&lock_path)?;
    lock.lock_exclusive()?;
    let result = operation();
    FileExt::unlock(&lock)?;
    result
}

fn write_state_unlocked(state: &Map<String, Value>) -> Result<()> {
    let path = config::state_path()?;
    config::ensure_private_config_dir()?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let bytes = serde_json::to_vec_pretty(state)?;
    AtomicFile::new(&path, AllowOverwrite)
        .write_with_options(|file| file.write_all(&bytes), options)
        .map_err(std::io::Error::from)?;
    config::restrict_private_file(&path)?;
    Ok(())
}

pub(crate) async fn record_connection(connection_id: String) -> Result<()> {
    join_blocking(
        tokio::task::spawn_blocking(move || {
            with_state_lock(|| {
                let mut state = read_state_unlocked()?;
                record_connection_in_state(&mut state, connection_id);
                write_state_unlocked(&state)
            })
        })
        .await,
    )
}

fn record_connection_in_state(state: &mut Map<String, Value>, connection_id: String) {
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
        history.insert(0, Value::String(connection_id.clone()));
    }
    state.insert("connectionId".to_string(), Value::String(connection_id));
    state.insert("connectionHistory".to_string(), Value::Array(history));
}

pub(crate) fn log_status(message: &str) {
    let now = chrono::Local::now().format("%H:%M:%S");
    eprintln!("[{now}] {message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn failed_connections_survive_history_growth_and_successful_cleanup() {
        let mut state = Map::new();
        for i in 0..20 {
            record_connection_in_state(&mut state, format!("id-{i}"));
        }
        forget_connections_in_state(&mut state, &["id-1".into(), "id-19".into()]);
        let history = state["connectionHistory"].as_array().unwrap();
        assert_eq!(history.len(), 18);
        assert!(history.contains(&json!("id-0")));
        assert!(!history.contains(&json!("id-1")));
        assert!(!state.contains_key("connectionId"));
        record_connection_in_state(&mut state, "new".into());
        forget_connections_in_state(&mut state, &["id-0".into()]);
        assert_eq!(state["connectionId"], json!("new"));
    }
}
