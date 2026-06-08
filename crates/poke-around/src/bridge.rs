use crate::{Error, Result, config};
use rs_poke::{
    CreateWebhook, CredentialsStore, LoginOptions, Poke, PokeOptions, TunnelEvent, TunnelOptions,
    TunnelRunner,
};
use serde_json::{Map, Value};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const RESTART_AFTER_DISCONNECT: Duration = Duration::from_secs(15);
const TUNNEL_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
// Poke's sync-tools endpoint cannot reach the local MCP through a new tunnel
// until roughly 25-30s after connect (same propagation delay the npm SDK hits).
const SYNC_TOOLS_INITIAL_DELAY: Duration = Duration::from_secs(20);
const SYNC_TOOLS_MAX_ATTEMPTS: usize = 4;
const SYNC_TOOLS_RETRY_DELAY: Duration = Duration::from_secs(3);
const MAX_CONN_HISTORY: usize = 10;

pub struct Bridge {
    tx: mpsc::UnboundedSender<BridgeCommand>,
    handle: Option<thread::JoinHandle<()>>,
}

enum BridgeCommand {
    SendWebhook(String),
    Stop,
}

impl Bridge {
    pub fn start(mcp_url: &str, mode: &str) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mcp_url = mcp_url.to_string();
        let mode = mode.to_string();
        let handle = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    log_status(&format!(
                        "Bridge error: failed to start async runtime: {err}"
                    ));
                    return;
                }
            };
            if let Err(err) = runtime.block_on(run_bridge(mcp_url, mode, rx)) {
                log_status(&format!("Bridge error: {err}"));
            }
        });
        Ok(Self {
            tx,
            handle: Some(handle),
        })
    }

    pub fn send_message(&self, message: &str) -> Result<()> {
        self.tx
            .send(BridgeCommand::SendWebhook(message.to_string()))
            .map_err(|_| Error::msg("bridge command channel closed"))
    }

    pub fn stop(&mut self) -> Result<()> {
        let _ = self.tx.send(BridgeCommand::Stop);
        let _ = self.handle.take();
        Ok(())
    }
}

async fn run_bridge(
    mcp_url: String,
    permission_mode: String,
    mut rx: mpsc::UnboundedReceiver<BridgeCommand>,
) -> Result<()> {
    let token = ensure_auth().await?;
    let mut poke = make_poke(&token)?;
    let tunnel_name = integration_name("poke-around");
    let (webhook_url, webhook_token) = ensure_webhook(&poke, &tunnel_name).await?;
    log_status("Webhook ready.");
    cleanup_stale_connections(&poke, &webhook_url, &webhook_token).await?;

    let mut stop_requested = false;
    while !stop_requested {
        let mut runner = TunnelRunner::new(
            poke.clone(),
            TunnelOptions {
                url: mcp_url.clone(),
                name: tunnel_name.clone(),
                cleanup_on_stop: false,
                sync_interval: Duration::from_secs(300),
                startup_timeout: TUNNEL_STARTUP_TIMEOUT,
            },
        );
        let mut events = runner.subscribe();
        let mut stop_during_start = false;
        let start_result = {
            let start = runner.start();
            tokio::pin!(start);
            loop {
                tokio::select! {
                    result = &mut start => break result,
                    command = rx.recv() => {
                        match command {
                            Some(BridgeCommand::SendWebhook(message)) => {
                                send_webhook_message(&poke, &webhook_url, &webhook_token, &message).await;
                            }
                            Some(BridgeCommand::Stop) | None => {
                                stop_during_start = true;
                                break Err(rs_poke::Error::Protocol("stop requested".into()));
                            }
                        }
                    }
                }
            }
        };
        if stop_during_start {
            if let Some(info) = runner.info() {
                let _ = runner.delete_connection(&info.connection_id).await;
            }
            let _ = runner.stop().await;
            return Ok(());
        }
        match start_result {
            Ok(info) => {
                record_connection(&info.connection_id)?;
                log_status(&format!(
                    "Tunnel connected ({}) -> {}",
                    info.connection_id, info.tunnel_url
                ));
                log_status("Waiting for tunnel propagation before syncing tools...");
                tokio::time::sleep(SYNC_TOOLS_INITIAL_DELAY).await;
                let synced = sync_and_report_tools(&runner, &mcp_url).await;
                if synced == 0 {
                    log_status(
                        "Tools not synced yet; Poke may not be able to use this machine.",
                    );
                } else {
                    log_status("Ready - your Poke agent can now access this machine.");
                }
                notify_poke(
                    &poke,
                    &webhook_url,
                    &webhook_token,
                    &permission_mode,
                    &tunnel_name,
                    &info.connection_id,
                    Some(&info.tunnel_url),
                )
                .await;
            }
            Err(err) => {
                log_status(&format!("Bridge error: {err}"));
                if let Some(info) = runner.info() {
                    let _ = runner.delete_connection(&info.connection_id).await;
                }
                let _ = runner.stop().await;
                sleep_or_stop(&mut rx, RESTART_AFTER_DISCONNECT, &mut stop_requested).await;
                continue;
            }
        }

        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        heartbeat.tick().await;
        let mut sync = tokio::time::interval(Duration::from_secs(300));
        sync.tick().await;
        loop {
            tokio::select! {
                _ = heartbeat.tick() => {}
                _ = sync.tick() => {
                    sync_and_report_tools(&runner, &mcp_url).await;
                }
                event = events.recv() => {
                    match event {
                        Ok(TunnelEvent::Disconnected) => {
                            log_status("Tunnel disconnected.");
                            break;
                        }
                        Ok(TunnelEvent::ToolsSynced { tool_count }) => {
                            if tool_count > 0 {
                                log_status(&format!("Tools synced: {tool_count}"));
                            }
                        }
                        Ok(TunnelEvent::OAuthRequired { auth_url }) => {
                            log_status(&format!(
                                "Poke token expired - re-authenticating ({auth_url})..."
                            ));
                            match ensure_auth().await {
                                Ok(token) => {
                                    if let Ok(client) = make_poke(&token) {
                                        poke = client;
                                    }
                                }
                                Err(err) => log_status(&format!("Re-auth failed: {err}")),
                            }
                            break;
                        }
                        Ok(TunnelEvent::Error(message)) => {
                            log_status(&format!("Bridge error: {message}"));
                            break;
                        }
                        Ok(TunnelEvent::Created(_)) | Ok(TunnelEvent::Connected(_)) => {}
                        Err(_) => break,
                    }
                }
                command = rx.recv() => {
                    match command {
                        Some(BridgeCommand::SendWebhook(message)) => {
                            send_webhook_message(&poke, &webhook_url, &webhook_token, &message).await;
                        }
                        Some(BridgeCommand::Stop) | None => {
                            if let Some(info) = runner.info() {
                                let _ = runner.delete_connection(&info.connection_id).await;
                            }
                            let _ = runner.stop().await;
                            return Ok(());
                        }
                    }
                }
            }
        }
        if let Some(info) = runner.info() {
            let _ = runner.delete_connection(&info.connection_id).await;
        }
        let _ = runner.stop().await;
        sleep_or_stop(&mut rx, RESTART_AFTER_DISCONNECT, &mut stop_requested).await;
    }
    Ok(())
}

async fn sleep_or_stop(
    rx: &mut mpsc::UnboundedReceiver<BridgeCommand>,
    duration: Duration,
    stop_requested: &mut bool,
) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        match rx.try_recv() {
            Ok(BridgeCommand::Stop) | Err(mpsc::error::TryRecvError::Disconnected) => {
                *stop_requested = true;
                return;
            }
            Ok(BridgeCommand::SendWebhook(_)) | Err(mpsc::error::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

async fn ensure_auth() -> Result<String> {
    if let Some(token) = rs_poke::get_token()? {
        return Ok(token);
    }
    log_status("Opening browser for Poke login...");
    let store = CredentialsStore::default_store().map_err(|err| Error::msg(err.to_string()))?;
    let options = LoginOptions::new(store);
    rs_poke::login(options)
        .await
        .map_err(|err| Error::msg(err.to_string()))
}

async fn ensure_webhook(poke: &Poke, tunnel_name: &str) -> Result<(String, String)> {
    let state = read_state()?;
    let webhook_url = state.get("webhookUrl").and_then(Value::as_str);
    let webhook_token = state.get("webhookToken").and_then(Value::as_str);
    let webhook_name = state.get("webhookName").and_then(Value::as_str);
    if let (Some(url), Some(token), Some(name)) = (webhook_url, webhook_token, webhook_name)
        && name == tunnel_name
    {
        eprintln!("\x1b[2m[bridge] Reusing cached webhook.\x1b[0m");
        return Ok((url.to_string(), token.to_string()));
    }
    eprintln!("\x1b[2m[bridge] Creating webhook (first run)...\x1b[0m");
    let webhook = poke
        .create_webhook(CreateWebhook {
            condition: tunnel_name,
            action: tunnel_name,
        })
        .await
        .map_err(|err| Error::msg(err.to_string()))?;
    patch_state([
        ("webhookUrl", Value::String(webhook.webhook_url.clone())),
        ("webhookToken", Value::String(webhook.webhook_token.clone())),
        ("webhookName", Value::String(tunnel_name.to_string())),
    ])?;
    Ok((webhook.webhook_url, webhook.webhook_token))
}

async fn cleanup_stale_connections(
    poke: &Poke,
    webhook_url: &str,
    webhook_token: &str,
) -> Result<()> {
    let state = read_state()?;
    let mut ids = Vec::new();
    if let Some(id) = state.get("connectionId").and_then(Value::as_str) {
        ids.push(id.to_string());
    }
    if let Some(history) = state.get("connectionHistory").and_then(Value::as_array) {
        for id in history.iter().filter_map(Value::as_str) {
            if !ids.iter().any(|known| known == id) {
                ids.push(id.to_string());
            }
        }
    }
    if ids.is_empty() {
        return Ok(());
    }
    eprintln!(
        "\x1b[2m[bridge] Cleaning up {} old connection(s)...\x1b[0m",
        ids.len()
    );
    for id in ids {
        let _ = poke
            .raw_auth(
                reqwest::Method::DELETE,
                &format!("/mcp/connections/{id}"),
                None,
            )
            .await;
    }
    patch_state([
        ("webhookUrl", Value::String(webhook_url.to_string())),
        ("webhookToken", Value::String(webhook_token.to_string())),
        ("connectionHistory", Value::Array(Vec::new())),
    ])?;
    remove_state_key("connectionId")?;
    Ok(())
}

async fn notify_poke(
    poke: &Poke,
    webhook_url: &str,
    webhook_token: &str,
    permission_mode: &str,
    tunnel_name: &str,
    connection_id: &str,
    tunnel_url: Option<&str>,
) {
    let mode_message = match permission_mode {
        "limited" => {
            "Access mode: Limited. You can read files, list directories, and run safe read-only commands. You cannot write files, take screenshots, or run other commands."
        }
        "sandbox" => {
            "Access mode: Sandbox. You can read files, list directories, and run approved sandbox commands. Destructive or disallowed actions require approval or are blocked."
        }
        _ => {
            "Access mode: Full. You can run shell commands, read files, list directories, take screenshots, and use computer-control tools. Destructive actions still require approval."
        }
    };
    let message = format!(
        "Poke Around is connected to {tunnel_name} (tunnel: {connection_id}). {}{mode_message} Use the Poke Around MCP tools whenever I ask you to do something on this machine.",
        tunnel_url
            .map(|url| format!("Tunnel URL: {url}. "))
            .unwrap_or_default()
    );
    match poke
        .send_webhook(
            webhook_url,
            webhook_token,
            serde_json::json!({
                "message": message,
                "connectionId": connection_id,
                "tunnelUrl": tunnel_url,
                "mode": permission_mode,
                "integration": tunnel_name
            }),
        )
        .await
    {
        Ok(_) => log_status("Notified Poke agent about connection."),
        Err(err) => log_status(&format!("Bridge error: {err}")),
    }
}

async fn send_webhook_message(poke: &Poke, webhook_url: &str, webhook_token: &str, message: &str) {
    match poke
        .send_webhook(
            webhook_url,
            webhook_token,
            serde_json::json!({ "message": message }),
        )
        .await
    {
        Ok(_) => log_status("Notified Poke agent about connection."),
        Err(err) => log_status(&format!("Bridge error: {err}")),
    }
}

fn make_poke(token: &str) -> Result<Poke> {
    Poke::new(PokeOptions {
        api_key: Some(token.to_string()),
        ..PokeOptions::default()
    })
    .map_err(|err| Error::msg(err.to_string()))
}

async fn sync_and_report_tools(runner: &TunnelRunner, mcp_url: &str) -> usize {
    let mut last_synced = 0usize;
    for attempt in 1..=SYNC_TOOLS_MAX_ATTEMPTS {
        last_synced = match runner.sync_tools().await {
            Ok(count) => count,
            Err(err) => {
                log_status(&format!("Tool sync error: {err}"));
                0
            }
        };
        let local = local_tool_count(mcp_url).await;
        let reported = last_synced.max(local);
        if last_synced > 0 {
            log_status(&format!("Tools synced: {reported}"));
            return reported;
        }
        if attempt < SYNC_TOOLS_MAX_ATTEMPTS {
            if local > 0 {
                log_status(&format!(
                    "Tunnel not ready for sync-tools yet (attempt {attempt}/{SYNC_TOOLS_MAX_ATTEMPTS}); local MCP has {local} tools, retrying in {}s...",
                    SYNC_TOOLS_RETRY_DELAY.as_secs()
                ));
            } else {
                log_status(&format!(
                    "Tunnel not ready for sync-tools yet (attempt {attempt}/{SYNC_TOOLS_MAX_ATTEMPTS}); retrying in {}s...",
                    SYNC_TOOLS_RETRY_DELAY.as_secs()
                ));
            }
            tokio::time::sleep(SYNC_TOOLS_RETRY_DELAY).await;
        }
    }
    let local = local_tool_count(mcp_url).await;
    let reported = last_synced.max(local);
    if reported > 0 {
        log_status(&format!("Tools synced: {reported}"));
    } else {
        log_status("Tools synced: 0 (check MCP server and tunnel connectivity)");
    }
    reported
}

async fn local_tool_count(mcp_url: &str) -> usize {
    let Ok(response) = reqwest::Client::new()
        .post(mcp_url)
        .json(&serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
        .await
    else {
        return 0;
    };
    let Ok(body) = response.json::<Value>().await else {
        return 0;
    };
    body.get("result")
        .and_then(|result| result.get("tools"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn integration_name(base: &str) -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .unwrap_or_default()
        });
    let suffix = raw
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if suffix.is_empty() {
        base.to_string()
    } else {
        format!("{base}-{suffix}")
    }
}

fn read_state() -> Result<Map<String, Value>> {
    match std::fs::read_to_string(config::state_path()?) {
        Ok(data) => Ok(serde_json::from_str::<Value>(&data)?
            .as_object()
            .cloned()
            .unwrap_or_default()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(err) => Err(err.into()),
    }
}

fn patch_state<const N: usize>(updates: [(&str, Value); N]) -> Result<()> {
    let mut state = read_state()?;
    for (key, value) in updates {
        state.insert(key.to_string(), value);
    }
    write_state(&state)
}

fn remove_state_key(key: &str) -> Result<()> {
    let mut state = read_state()?;
    state.remove(key);
    write_state(&state)
}

fn write_state(state: &Map<String, Value>) -> Result<()> {
    let path = config::state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

fn record_connection(connection_id: &str) -> Result<()> {
    let state = read_state()?;
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
    patch_state([
        ("connectionId", Value::String(connection_id.to_string())),
        ("connectionHistory", Value::Array(history)),
    ])
}

fn log_status(message: &str) {
    let now = chrono::Local::now().format("%H:%M:%S");
    eprintln!("[{now}] {message}");
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub fn send_one_shot_message(message: &str) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| Error::msg(format!("failed to start async runtime: {err}")))?;
    runtime.block_on(async {
        let token = ensure_auth().await?;
        let poke = make_poke(&token)?;
        poke.send_message(message)
            .await
            .map_err(|err| Error::msg(err.to_string()))?;
        println!("sent");
        Ok(())
    })
}
