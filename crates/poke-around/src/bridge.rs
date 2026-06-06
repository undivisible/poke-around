use crate::{Error, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct Bridge {
    child: Child,
    writer: Arc<Mutex<Option<ChildStdin>>>,
}

impl Bridge {
    pub fn start(mcp_url: &str, mode: &str) -> Result<Self> {
        let bridge_path = resolve_bridge_path()?;
        let runtime = runtime_for(&bridge_path);
        let mut child = Command::new(runtime)
            .arg(&bridge_path)
            .arg("tunnel")
            .arg("--mcp-url")
            .arg(mcp_url)
            .arg("--mode")
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let writer = Arc::new(Mutex::new(child.stdin.take()));
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(std::result::Result::ok) {
                    print_bridge_event(&line);
                }
            });
        }
        Ok(Self { child, writer })
    }

    pub fn send_message(&self, message: &str) -> Result<()> {
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| Error::msg("bridge lock poisoned"))?;
        let Some(stdin) = guard.as_mut() else {
            return Err(Error::msg("bridge stdin closed"));
        };
        let payload = serde_json::json!({ "type": "send_webhook", "message": message });
        writeln!(stdin, "{payload}")?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Ok(mut guard) = self.writer.lock()
            && let Some(mut stdin) = guard.take()
        {
            let _ = writeln!(stdin, "{{\"type\":\"stop\"}}");
            drop(stdin);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

fn print_bridge_event(line: &str) {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        eprintln!("{line}");
        return;
    };
    let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    match event_type {
        "webhook_ready" => log_status("Webhook ready."),
        "connected" => {
            let connection_id = value
                .get("connectionId")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let tunnel_url = value.get("tunnelUrl").and_then(Value::as_str);
            match tunnel_url {
                Some(url) => log_status(&format!("Tunnel connected ({connection_id}) -> {url}")),
                None => log_status(&format!("Tunnel connected ({connection_id})")),
            }
            log_status("Ready - your Poke agent can now access this machine.");
        }
        "tools_synced" => {
            let count = value.get("count").and_then(Value::as_u64).unwrap_or(0);
            log_status(&format!("Tools synced: {count}"));
        }
        "disconnected" => log_status("Tunnel disconnected."),
        "auth_required" => {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Authentication required.");
            log_status(message);
        }
        "webhook_sent" => log_status("Notified Poke agent about connection."),
        "webhook_error" | "error" => {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("bridge error");
            log_status(&format!("Bridge error: {message}"));
        }
        _ => eprintln!("{line}"),
    }
}

fn log_status(message: &str) {
    let now = chrono_like_time();
    eprintln!("[{now}] {message}");
}

fn chrono_like_time() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() % 86_400)
        .unwrap_or(0);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{hour:02}:{minute:02}:{second:02}")
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub fn send_one_shot_message(message: &str) -> Result<()> {
    let bridge_path = resolve_bridge_path()?;
    let runtime = runtime_for(&bridge_path);
    let status = Command::new(runtime)
        .arg(bridge_path)
        .arg("send-message")
        .arg("--message")
        .arg(message)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::msg(format!("bridge exited with {status}")))
    }
}

pub fn resolve_bridge_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let candidates = [
        exe_dir.join("poke-around-bridge.js"),
        exe_dir.join("../bridge/dist/poke-around-bridge.js"),
        std::env::current_dir()?.join("bridge/dist/poke-around-bridge.js"),
        std::env::current_dir()?.join("bridge/poke-bridge.ts"),
    ];
    candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| Error::msg("poke-around bridge not found"))
}

fn runtime_for(path: &Path) -> &'static str {
    if path.extension().and_then(|value| value.to_str()) == Some("ts") {
        return "bun";
    }
    for candidate in ["/opt/homebrew/bin/bun", "/usr/local/bin/bun"] {
        if Path::new(candidate).exists() {
            return candidate;
        }
    }
    "node"
}
