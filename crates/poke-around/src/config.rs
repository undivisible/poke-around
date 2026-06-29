use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub permission_mode: Option<String>,
}

#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
static ENV_MUTEX: Mutex<()> = Mutex::new(());

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

pub fn read_config() -> Result<Config> {
    let path = config_path()?;
    match std::fs::read_to_string(path) {
        Ok(data) => Ok(serde_json::from_str(&data)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(err) => Err(err.into()),
    }
}

fn restrict_private_file(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        if let Ok(username) = std::env::var("USERNAME")
            && !username.is_empty()
        {
            let _ = Command::new("icacls")
                .arg(path)
                .args(["/inheritance:r", "/grant:r", &format!("{username}:F")])
                .status();
        }
    }
    Ok(())
}

pub fn save_permission_mode(mode: &str) -> Result<()> {
    let mut config = read_config()?;
    config.permission_mode = Some(mode.to_string());
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&config)?)?;
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

            // Reading it back should show "full"
            let updated_config = read_config().unwrap();
            assert_eq!(updated_config.permission_mode, Some("full".to_string()));

            // Also verify the file actually exists and contains the data
            let cfg_path = config_path().unwrap();
            assert!(cfg_path.exists());

            let content = std::fs::read_to_string(cfg_path).unwrap();
            assert!(content.contains(r#""permission_mode": "full""#));
        });
    }

    #[test]
    #[serial]
    fn test_save_permission_mode_permissions() {
        with_temp_env(|_| {
            let mode = "limited";
            save_permission_mode(mode).unwrap();

            let cfg_path = config_path().unwrap();
            assert!(cfg_path.exists());
            assert!(cfg_path.parent().unwrap().exists());

            let config = read_config().unwrap();
            assert_eq!(config.permission_mode, Some(mode.to_string()));

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = std::fs::metadata(&cfg_path).unwrap();
                let permissions = metadata.permissions();
                assert_eq!(permissions.mode() & 0o777, 0o600);
            }
        });
    }
}
