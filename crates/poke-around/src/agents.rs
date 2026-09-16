use crate::{Error, Result, config};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run_agent_by_name(name: &str) -> Result<()> {
    let path = find_agent(name)?;
    let runtime = find_js_runtime()?;
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

fn validate_agent_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('\0')
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains("..")
    {
        return Err(Error::msg(
            "invalid agent name: path separators and parent directories are not allowed",
        ));
    }
    Ok(())
}

pub fn find_agent(name: &str) -> Result<PathBuf> {
    validate_agent_name(name)?;
    let dir = config::agents_dir()?;
    let candidates = [dir.join(name), dir.join(format!("{name}.js"))];
    for candidate in candidates {
        if candidate.exists() {
            return ensure_agent_path(&dir, candidate);
        }
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
            return ensure_agent_path(&dir, entry.path());
        }
    }
    Err(Error::msg(format!(
        "agent '{name}' not found in {}",
        dir.display()
    )))
}

fn normalized_components(path: &Path) -> Option<Vec<std::ffi::OsString>> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => {
                parts.push(prefix.as_os_str().to_os_string());
            }
            std::path::Component::RootDir => {
                parts.push(std::ffi::OsString::from(
                    std::path::MAIN_SEPARATOR.to_string(),
                ));
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let root = std::ffi::OsString::from(std::path::MAIN_SEPARATOR.to_string());
                if parts.last() == Some(&root) || parts.pop().is_none() {
                    return None;
                }
            }
            std::path::Component::Normal(part) => parts.push(part.to_os_string()),
        }
    }
    Some(parts)
}

fn path_is_within(root: &Path, candidate: &Path) -> bool {
    match (
        normalized_components(root),
        normalized_components(candidate),
    ) {
        (Some(root_parts), Some(candidate_parts)) => candidate_parts.starts_with(&root_parts),
        _ => false,
    }
}

fn ensure_agent_path(dir: &Path, path: PathBuf) -> Result<PathBuf> {
    match (dir.canonicalize(), path.canonicalize()) {
        (Ok(root), Ok(canonical)) => {
            if path_is_within(&root, &canonical) {
                Ok(canonical)
            } else {
                Err(Error::msg("agent path escaped the agents directory"))
            }
        }
        _ => {
            // Windows temp dirs can deny canonicalize (os error 5). Fall back to
            // component containment so lookup still fails closed on `..`.
            if path_is_within(dir, &path) {
                Ok(path)
            } else {
                Err(Error::msg("agent path escaped the agents directory"))
            }
        }
    }
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
    validate_agent_name(name)?;
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

    let client = crate::mcp::public_http_client(&url)?;
    let response = client
        .get(&url)
        .send()
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

fn find_js_runtime() -> Result<&'static str> {
    for candidate in ["/opt/homebrew/bin/bun", "/usr/local/bin/bun"] {
        if Path::new(candidate).exists() {
            return Ok(candidate);
        }
    }
    if find_in_path("bun") {
        return Ok("bun");
    }
    Err(Error::msg("bun runtime not found"))
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

    fn assert_same_agent_path(found: &Path, expected: &Path) {
        match (found.canonicalize(), expected.canonicalize()) {
            (Ok(found), Ok(expected)) => assert_eq!(found, expected),
            _ => assert_eq!(found, expected),
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
        assert_same_agent_path(&found, &agent_path);
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
        assert_same_agent_path(&found, &agent_path);
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
        match find_js_runtime() {
            Ok(runtime) => assert!(!runtime.is_empty()),
            Err(error) => assert_eq!(error.to_string(), "bun runtime not found"),
        }
    }

    #[test]
    #[serial]
    fn test_download_agent_invalid_name() {
        let path_names = ["bad/name", r"bad\name", "..", "foo..bar", "C:name"];
        for name in path_names {
            let result = download_agent(name);
            assert!(result.is_err(), "Expected error for name '{name}'");
            assert_eq!(
                result.unwrap_err().to_string(),
                "invalid agent name: path separators and parent directories are not allowed"
            );
        }

        let invalid_names = ["name with spaces", "name&", ".name", "name#1"];
        for name in invalid_names {
            let result = download_agent(name);
            assert!(result.is_err(), "Expected error for name '{name}'");
            assert_eq!(
                result.unwrap_err().to_string(),
                "invalid agent name: only alphanumeric, dash, and underscore are allowed"
            );
        }
    }

    #[test]
    #[serial]
    fn test_find_agent_rejects_path_traversal_names() {
        let _guard = setup_test_env();
        let agents_dir = config::agents_dir().unwrap();
        std::fs::create_dir_all(&agents_dir).unwrap();

        for name in ["../secret", r"..\secret", "..", "foo/bar", "foo\\bar", ""] {
            let result = find_agent(name);
            assert!(result.is_err(), "Expected error for name '{name}'");
            assert_eq!(
                result.unwrap_err().to_string(),
                "invalid agent name: path separators and parent directories are not allowed"
            );
        }
    }

    #[test]
    fn ensure_agent_path_falls_back_to_component_containment() {
        let root = PathBuf::from("poke-around-agents");
        let inside = root.join("ok.js");
        let escaped = root.join("..").join("secret.js");
        assert!(path_is_within(&root, &inside));
        assert!(!path_is_within(&root, &escaped));
        assert!(ensure_agent_path(&root, escaped).is_err());
    }

    #[cfg(not(windows))]
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
