use crate::{Error, Result};
use atomicwrites::{AllowOverwrite, AtomicFile};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub permission_mode: Option<String>,
    pub approval_mode: Option<String>,
}

#[cfg(test)]
static ENV_MUTEX: Mutex<()> = Mutex::new(());
static CONFIG_MUTEX: Mutex<()> = Mutex::new(());

pub fn config_dir() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .ok_or_else(|| Error::msg("home directory not found"))?;
    Ok(base.join("poke-around"))
}

pub fn agents_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("agents"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("state.json"))
}

pub(crate) fn ensure_private_config_dir() -> Result<PathBuf> {
    let path = config_dir()?;
    std::fs::create_dir_all(&path)?;
    restrict_private_dir(&path)?;
    Ok(path)
}

pub fn read_config() -> Result<Config> {
    with_config_lock(read_config_unlocked)
}

fn read_config_unlocked() -> Result<Config> {
    let path = config_path()?;
    match std::fs::read_to_string(path) {
        Ok(data) => Ok(serde_json::from_str(&data)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn restrict_private_file(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let username = std::env::var("USERNAME")
            .map_err(|_| Error::msg("USERNAME is required to restrict private files"))?;
        if username.is_empty() {
            return Err(Error::msg("USERNAME is required to restrict private files"));
        }
        let status = Command::new("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r", &format!("{username}:F")])
            .status()?;
        if !status.success() {
            return Err(Error::msg("failed to restrict private file ACL"));
        }
    }
    Ok(())
}

pub(crate) fn restrict_private_dir(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let username = std::env::var("USERNAME")
            .map_err(|_| Error::msg("USERNAME is required to restrict private directories"))?;
        if username.is_empty() {
            return Err(Error::msg(
                "USERNAME is required to restrict private directories",
            ));
        }
        let status = Command::new("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r", &format!("{username}:F")])
            .status()?;
        if !status.success() {
            return Err(Error::msg("failed to restrict private directory ACL"));
        }
    }
    Ok(())
}

pub(crate) fn harden_peekaboo_cache() -> Result<()> {
    let path = rs_peekaboo::cache::snapshot_dir()?;
    if !path.try_exists()? {
        return Ok(());
    }
    restrict_private_dir(&path)?;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            restrict_private_file(&entry.path())?;
        }
    }
    Ok(())
}

pub fn save_permission_mode(mode: &str) -> Result<()> {
    with_config_lock(|| {
        let mut config = read_config_unlocked()?;
        config.permission_mode = Some(mode.to_string());
        save_config_unlocked(&config)
    })
}

pub fn save_approval_mode(mode: &str) -> Result<()> {
    with_config_lock(|| {
        let mut config = read_config_unlocked()?;
        config.approval_mode = Some(mode.to_string());
        save_config_unlocked(&config)
    })
}

fn with_config_lock<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let _process_guard = CONFIG_MUTEX
        .lock()
        .map_err(|_| Error::msg("config lock poisoned"))?;
    let directory = ensure_private_config_dir()?;
    let lock_path = directory.join("config.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    restrict_private_file(&lock_path)?;
    lock.lock_exclusive()?;
    let result = operation();
    FileExt::unlock(&lock)?;
    result
}

fn save_config_unlocked(config: &Config) -> Result<()> {
    let path = config_path()?;
    ensure_private_config_dir()?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let bytes = serde_json::to_vec_pretty(config)?;
    AtomicFile::new(&path, AllowOverwrite)
        .write_with_options(|file| file.write_all(&bytes), options)
        .map_err(std::io::Error::from)?;
    restrict_private_file(&path)?;
    Ok(())
}

pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error::msg("home directory not found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    #[test]
    fn test_config_derives() {
        let mut config = Config::default();
        assert_eq!(config.permission_mode, None);
        assert_eq!(config.approval_mode, None);

        config.permission_mode = Some("limited".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.permission_mode, Some("limited".to_string()));
    }

    // Helper to safely run a test with modified XDG_CONFIG_HOME
    fn with_temp_env<F>(test: F)
    where
        F: FnOnce(&std::path::Path),
    {
        let _lock = ENV_MUTEX.lock().unwrap();
        let temp_dir = TempDir::new().unwrap();

        // Save original XDG_CONFIG_HOME
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
        // Ensure HOME is not used by config_dir() fallback by explicitly setting XDG_CONFIG_HOME
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            test(temp_dir.path());
        }));

        // Restore original environment
        unsafe {
            if let Some(val) = original_xdg {
                std::env::set_var("XDG_CONFIG_HOME", val);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }

        if let Err(err) = result {
            std::panic::resume_unwind(err);
        }
    }

    #[test]
    #[serial]
    fn test_paths() {
        with_temp_env(|_| {
            let base = config_dir().unwrap();
            assert!(base.ends_with("poke-around"));

            let agents = agents_dir().unwrap();
            assert_eq!(agents, base.join("agents"));

            let cfg = config_path().unwrap();
            assert_eq!(cfg, base.join("config.json"));

            let state = state_path().unwrap();
            assert_eq!(state, base.join("state.json"));
        });
    }

    #[test]
    #[serial]
    fn test_read_write_config() {
        with_temp_env(|_| {
            // Initially reading config should return default (NotFound -> Ok(Config::default()))
            let initial_config = read_config().unwrap();
            assert_eq!(initial_config.permission_mode, None);

            // Save a new permission mode
            save_permission_mode("full").unwrap();
            save_approval_mode("per-action").unwrap();

            // Reading it back should show "full"
            let updated_config = read_config().unwrap();
            assert_eq!(updated_config.permission_mode, Some("full".to_string()));
            assert_eq!(updated_config.approval_mode, Some("per-action".to_string()));

            // Also verify the file actually exists and contains the data
            let cfg_path = config_path().unwrap();
            assert!(cfg_path.exists());

            let content = std::fs::read_to_string(cfg_path).unwrap();
            assert!(content.contains(r#""permission_mode": "full""#));
        });
    }

    #[test]
    #[serial]
    fn test_read_config_scenarios() {
        with_temp_env(|_| {
            let cfg_path = config_path().unwrap();

            // Ensure directory exists for our manual writes
            if let Some(parent) = cfg_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }

            // Scenario 1: file not found returns default Config
            if cfg_path.exists() {
                std::fs::remove_file(&cfg_path).unwrap();
            }
            let config = read_config().unwrap();
            assert_eq!(config.permission_mode, None);

            // Scenario 2: read valid json
            let valid_json = r#"{"permission_mode": "test_mode"}"#;
            std::fs::write(&cfg_path, valid_json).unwrap();
            let config = read_config().unwrap();
            assert_eq!(config.permission_mode, Some("test_mode".to_string()));

            // Scenario 3: read invalid json returns error
            let invalid_json = "invalid json";
            std::fs::write(&cfg_path, invalid_json).unwrap();
            let result = read_config();
            assert!(result.is_err());

            // Scenario 4: path is a directory
            std::fs::remove_file(&cfg_path).unwrap();
            std::fs::create_dir_all(&cfg_path).unwrap();
            let result = read_config();
            assert!(result.is_err());
            // Cleanup the directory so it doesn't pollute
            std::fs::remove_dir(&cfg_path).unwrap();
        });
    }
}
