use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub permission_mode: Option<String>,
}

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
