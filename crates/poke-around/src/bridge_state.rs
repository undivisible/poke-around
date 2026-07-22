use crate::{Result, config};
use atomicwrites::{AllowOverwrite, AtomicFile};
use fs2::FileExt;
use serde_json::{Map, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

const MAX_CONN_HISTORY: usize = 10;
static STATE_MUTEX: Mutex<()> = Mutex::new(());

pub(crate) fn read_state() -> Result<Map<String, Value>> {
    with_state_lock(read_state_unlocked)
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

pub(crate) fn patch_state<const N: usize>(updates: [(&str, Value); N]) -> Result<()> {
    with_state_lock(|| {
        let mut state = read_state_unlocked()?;
        for (key, value) in updates {
            state.insert(key.to_string(), value);
        }
        write_state_unlocked(&state)
    })
}

pub(crate) fn remove_state_key(key: &str) -> Result<()> {
    with_state_lock(|| {
        let mut state = read_state_unlocked()?;
        state.remove(key);
        write_state_unlocked(&state)
    })
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

pub(crate) fn record_connection(connection_id: &str) -> Result<()> {
    with_state_lock(|| {
        let state = read_state_unlocked()?;
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
        let mut next = state;
        next.insert(
            "connectionId".to_string(),
            Value::String(connection_id.to_string()),
        );
        next.insert("connectionHistory".to_string(), Value::Array(history));
        write_state_unlocked(&next)
    })
}

pub(crate) fn log_status(message: &str) {
    let now = chrono::Local::now().format("%H:%M:%S");
    eprintln!("[{now}] {message}");
}
