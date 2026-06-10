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
