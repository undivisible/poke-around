use crate::{Error, Result, config};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run_agent_by_name(name: &str) -> Result<()> {
    let path = find_agent(name)?;
    let runtime = find_js_runtime();
    let status = Command::new(runtime)
        .arg(path)
        .stdin(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::msg(format!("agent '{name}' exited with {status}")))
    }
}

pub fn find_agent(name: &str) -> Result<PathBuf> {
    let dir = config::agents_dir()?;
    let direct = dir.join(name);
    if direct.exists() {
        return Ok(direct);
    }
    let js_path = dir.join(format!("{}.js", name));
    if js_path.exists() {
        return Ok(js_path);
    }
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("js") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if file_name == name || file_name.starts_with(&format!("{name}.")) {
            return Ok(path);
        }
    }
    Err(Error::msg(format!(
        "agent '{name}' not found in {}",
        dir.display()
    )))
}

pub fn create_agent(prompt: Option<&str>) -> Result<PathBuf> {
    let dir = config::agents_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("custom.30m.js");
    let message = prompt.unwrap_or("Hello from Poke Around");
    let body = format!(
        "import {{ Poke, getToken }} from \"poke\";\nconst poke = new Poke({{ apiKey: getToken() }});\nawait poke.sendMessage({message:?});\n"
    );
    std::fs::write(&path, body)?;
    Ok(path)
}

pub fn download_agent(name: &str) -> Result<PathBuf> {
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Error::msg(
            "invalid agent name: only alphanumeric, dash, and underscore are allowed",
        ));
    }
    let dir = config::agents_dir()?;
    std::fs::create_dir_all(&dir)?;
    let url =
        format!("https://raw.githubusercontent.com/f/poke-gate/main/examples/agents/{name}.js");
    let output = Command::new("curl").arg("-fsSL").arg(&url).output()?;
    if !output.status.success() {
        return Err(Error::msg(format!(
            "failed to download agent '{name}': {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let path = dir.join(format!("{name}.js"));
    std::fs::write(&path, output.stdout)?;
    Ok(path)
}

fn find_js_runtime() -> &'static str {
    for candidate in ["/opt/homebrew/bin/bun", "/usr/local/bin/bun", "bun", "node"] {
        if candidate.contains('/') {
            if Path::new(candidate).exists() {
                return candidate;
            }
        } else {
            return candidate;
        }
    }
    "node"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        original_xdg: Option<std::ffi::OsString>,
        _temp_dir: tempfile::TempDir,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                if let Some(ref val) = self.original_xdg {
                    std::env::set_var("XDG_CONFIG_HOME", val);
                } else {
                    std::env::remove_var("XDG_CONFIG_HOME");
                }
            }
        }
    }

    fn setup_test_env() -> EnvGuard {
        let lock = ENV_MUTEX.lock().unwrap();
        let original_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let temp_dir = tempdir().unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }
        EnvGuard {
            _lock: lock,
            original_xdg,
            _temp_dir: temp_dir,
        }
    }

    #[test]
    fn test_find_agent_exact_match() {
        let _guard = setup_test_env();
        let agents_dir = config::agents_dir().unwrap();
        std::fs::create_dir_all(&agents_dir).unwrap();

        let agent_path = agents_dir.join("my_agent");
        std::fs::write(&agent_path, "test content").unwrap();

        let found = find_agent("my_agent").unwrap();
        assert_eq!(found, agent_path);
    }

    #[test]
    fn test_find_agent_js_extension() {
        let _guard = setup_test_env();
        let agents_dir = config::agents_dir().unwrap();
        std::fs::create_dir_all(&agents_dir).unwrap();

        let agent_path = agents_dir.join("my_agent.js");
        std::fs::write(&agent_path, "test content").unwrap();

        let found = find_agent("my_agent").unwrap();
        assert_eq!(found, agent_path);
    }

    #[test]
    fn test_find_agent_not_found() {
        let _guard = setup_test_env();
        let agents_dir = config::agents_dir().unwrap();
        std::fs::create_dir_all(&agents_dir).unwrap();

        let result = find_agent("non_existent_agent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_create_agent() {
        let _guard = setup_test_env();

        // Test with default prompt
        let path1 = create_agent(None).unwrap();
        assert!(path1.exists());
        let content1 = std::fs::read_to_string(&path1).unwrap();
        assert!(content1.contains("Hello from Poke Around"));

        std::fs::remove_file(&path1).unwrap();

        // Test with custom prompt
        let path2 = create_agent(Some("Custom test prompt")).unwrap();
        assert!(path2.exists());
        let content2 = std::fs::read_to_string(&path2).unwrap();
        assert!(content2.contains("Custom test prompt"));
    }

    #[test]
    fn test_find_js_runtime_returns_string() {
        let runtime = find_js_runtime();
        assert!(!runtime.is_empty());
    }
}
