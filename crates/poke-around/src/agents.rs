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
    let prefix = format!("{name}.");
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let file_name_os = entry.file_name();
        let Some(file_name_str) = file_name_os.to_str() else {
            continue;
        };

        if !file_name_str.ends_with(".js") {
            continue;
        }

        let stem = &file_name_str[..file_name_str.len() - 3];
        if stem == name || stem.starts_with(&prefix) {
            return Ok(entry.path());
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
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("custom-{timestamp}.30m.js"));
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

    let base_url = std::env::var("POKE_AROUND_AGENT_BASE_URL").unwrap_or_else(|_| {
        "https://raw.githubusercontent.com/f/poke-gate/main/examples/agents".to_string()
    });
    let url = format!("{base_url}/{name}.js");

    let response = reqwest::blocking::get(&url)
        .map_err(|e| Error::msg(format!("failed to fetch agent: {}", e)))?;
    if !response.status().is_success() {
        return Err(Error::msg(format!(
            "failed to download agent '{name}': HTTP status {}",
            response.status()
        )));
    }
    let path = dir.join(format!("{name}.js"));
    let bytes = response
        .bytes()
        .map_err(|e| Error::msg(format!("failed to read agent body: {}", e)))?;
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn find_in_path(program: &str) -> bool {
    #[cfg(windows)]
    let lookup = "where";
    #[cfg(not(windows))]
    let lookup = "which";

    Command::new(lookup)
        .arg(program)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn find_js_runtime() -> &'static str {
    for candidate in ["/opt/homebrew/bin/bun", "/usr/local/bin/bun"] {
        if Path::new(candidate).exists() {
            return candidate;
        }
    }
    for name in ["bun", "node"] {
        if find_in_path(name) {
            return name;
        }
    }
    "node"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
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
        let lock = ENV_MUTEX.lock().unwrap_or_else(|err| err.into_inner());
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
    #[serial]
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
    #[serial]
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
    #[serial]
    fn test_find_agent_not_found() {
        let _guard = setup_test_env();
        let agents_dir = config::agents_dir().unwrap();
        std::fs::create_dir_all(&agents_dir).unwrap();

        let result = find_agent("non_existent_agent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    #[serial]
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
    #[serial]
    fn test_find_js_runtime_returns_string() {
        let runtime = find_js_runtime();
        assert!(!runtime.is_empty());
    }

    #[test]
    #[serial]
    fn test_download_agent_invalid_name() {
        let invalid_names = ["bad/name", "name with spaces", "name&", ".name", "name#1"];
        for name in invalid_names {
            let result = download_agent(name);
            assert!(result.is_err(), "Expected error for name '{}'", name);
            assert_eq!(
                result.unwrap_err().to_string(),
                "invalid agent name: only alphanumeric, dash, and underscore are allowed"
            );
        }
    }

    use httptest::{Expectation, Server, matchers::*, responders::*};

    #[test]
    #[serial]
    #[cfg(not(windows))]
    fn test_download_agent_mocked_success() {
        let _env_guard = setup_test_env();

        // Use a mock HTTP server instead of a path override
        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path("GET", "/test-agent.js"))
                .respond_with(status_code(200).body("console.log(\"mocked agent\");\n")),
        );

        // Set the environment variable to point to our local mock server
        unsafe {
            std::env::set_var(
                "POKE_AROUND_AGENT_BASE_URL",
                server.url_str("/").trim_end_matches('/'),
            );
        }

        let agent_name = "test-agent";
        let path = download_agent(agent_name).unwrap();

        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "test-agent.js");
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "console.log(\"mocked agent\");\n");
    }

    #[test]
    #[serial]
    #[cfg(not(windows))]
    fn test_download_agent_mocked_failure() {
        let _env_guard = setup_test_env();

        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path("GET", "/fail-agent.js"))
                .respond_with(status_code(500)),
        );

        unsafe {
            std::env::set_var(
                "POKE_AROUND_AGENT_BASE_URL",
                server.url_str("/").trim_end_matches('/'),
            );
        }

        let agent_name = "fail-agent";
        let result = download_agent(agent_name);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("failed to download agent 'fail-agent'"));
        assert!(err_msg.contains("HTTP status 500"));
    }
}
