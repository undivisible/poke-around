use crate::{Error, Result, config};
use rs_poke::{
    CreateWebhook, CredentialsStore, FetchWithAuthOptions, LoginOptions, Poke, PokeOptions,
    TunnelEvent, TunnelOptions, TunnelRunner, fetch_with_auth,
};
use serde_json::{Map, Value};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc as async_mpsc;

const BRIDGE_STOP_TIMEOUT: Duration = Duration::from_secs(30);
const RESTART_AFTER_DISCONNECT: Duration = Duration::from_secs(15);
const TUNNEL_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const SYNC_TOOLS_MAX_ATTEMPTS: usize = 8;
const SYNC_TOOLS_RETRY_DELAY: Duration = Duration::from_secs(3);
const MAX_CONN_HISTORY: usize = 10;
const TUNNEL_HEALTH_CHECK: Duration = Duration::from_secs(30);

pub struct Bridge {
    tx: async_mpsc::UnboundedSender<BridgeCommand>,
    handle: Option<thread::JoinHandle<()>>,
    done: Option<mpsc::Receiver<()>>,
}

enum BridgeCommand {
    SendWebhook(String),
    Stop,
}

impl Bridge {
    pub fn start(mcp_url: &str, mode: &str) -> Result<Self> {
        let (tx, rx) = async_mpsc::unbounded_channel();
        let (done_tx, done_rx) = mpsc::channel();
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
                    let _ = done_tx.send(());
                    return;
                }
            };
            if let Err(err) = runtime.block_on(run_bridge(mcp_url, mode, rx)) {
                log_status(&format!("Bridge error: {err}"));
            }
            let _ = done_tx.send(());
        });
        Ok(Self {
            tx,
            handle: Some(handle),
            done: Some(done_rx),
        })
    }

    /// Send a notification to the Poke agent through the cached webhook.
    pub fn notify_via_webhook(&self, message: &str) -> Result<()> {
        self.tx
            .send(BridgeCommand::SendWebhook(message.to_string()))
            .map_err(|_| Error::msg("bridge command channel closed"))
    }

    pub fn stop(&mut self) -> Result<()> {
        let _ = self.tx.send(BridgeCommand::Stop);
        if let Some(done_rx) = self.done.take() {
            let _ = done_rx.recv_timeout(BRIDGE_STOP_TIMEOUT);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        Ok(())
    }
}

async fn run_bridge(
    mcp_url: String,
    permission_mode: String,
    mut rx: async_mpsc::UnboundedReceiver<BridgeCommand>,
) -> Result<()> {
    let token = ensure_auth(false).await?;
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
                client_id: std::env::var("POKE_CLIENT_ID").ok(),
                client_secret: std::env::var("POKE_CLIENT_SECRET").ok(),
                cleanup_on_stop: false,
                sync_interval: Duration::from_secs(300),
                startup_timeout: TUNNEL_STARTUP_TIMEOUT,
                ..TunnelOptions::default()
            },
        );
        let events = runner.subscribe();

        match start_tunnel_runner(&mut runner, &mut rx, &poke, &webhook_url, &webhook_token).await {
            StartTunnelResult::StopRequested => {
                if let Some(info) = runner.info() {
                    let _ = runner.delete_connection(&info.connection_id).await;
                }
                let _ = runner.stop().await;
                return Ok(());
            }
            StartTunnelResult::Success(info) => {
                record_connection(&info.connection_id)?;
                log_status(&format!(
                    "Tunnel connected ({}) -> {}",
                    info.connection_id, info.tunnel_url
                ));
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
                let synced = sync_and_report_tools(&runner, &mcp_url).await;
                if synced > 0 {
                    log_status("Ready - your Poke agent can now access this machine.");
                } else {
                    log_status("Tools not synced yet; Poke may not be able to use this machine.");
                }
            }
            StartTunnelResult::Error(err) => {
                log_status(&format!("Bridge error: {err}"));
                if let Some(info) = runner.info() {
                    let _ = runner.delete_connection(&info.connection_id).await;
                }
                let _ = runner.stop().await;
                sleep_or_stop(&mut rx, RESTART_AFTER_DISCONNECT, &mut stop_requested).await;
                continue;
            }
        }

        let mut restart_tunnel = false;
        match monitor_tunnel(
            &runner,
            events,
            &mut rx,
            &poke,
            &webhook_url,
            &webhook_token,
        )
        .await?
        {
            MonitorTunnelResult::StopRequested => {
                if let Some(info) = runner.info() {
                    let _ = runner.delete_connection(&info.connection_id).await;
                }
                let _ = runner.stop().await;
                return Ok(());
            }
            MonitorTunnelResult::Restart { new_poke } => {
                poke = new_poke;
                restart_tunnel = true;
            }
            MonitorTunnelResult::Disconnected => {
                // Continue to regular cleanup and optional sleep
            }
        }

        if let Some(info) = runner.info() {
            let _ = runner.delete_connection(&info.connection_id).await;
        }
        let _ = runner.stop().await;
        if restart_tunnel {
            log_status("Restarting tunnel with refreshed credentials...");
            sleep_or_stop(&mut rx, Duration::from_secs(1), &mut stop_requested).await;
            continue;
        }
        sleep_or_stop(&mut rx, RESTART_AFTER_DISCONNECT, &mut stop_requested).await;
    }
    Ok(())
}

enum StartTunnelResult {
    Success(rs_poke::TunnelInfo),
    Error(rs_poke::Error),
    StopRequested,
}

enum MonitorTunnelResult {
    Restart { new_poke: Poke },
    StopRequested,
    Disconnected,
}

async fn monitor_tunnel(
    runner: &TunnelRunner,
    mut events: tokio::sync::broadcast::Receiver<TunnelEvent>,
    rx: &mut async_mpsc::UnboundedReceiver<BridgeCommand>,
    poke: &Poke,
    webhook_url: &str,
    webhook_token: &str,
) -> Result<MonitorTunnelResult> {
    let mut health_check = tokio::time::interval(TUNNEL_HEALTH_CHECK);
    health_check.tick().await;
    loop {
        tokio::select! {
            _ = health_check.tick() => {
                if !runner.connected() {
                    log_status("Tunnel no longer connected.");
                    return Ok(MonitorTunnelResult::Disconnected);
                }
            }
            event = events.recv() => {
                match event {
                    Ok(TunnelEvent::Disconnected) => {
                        log_status("Tunnel disconnected.");
                        return Ok(MonitorTunnelResult::Disconnected);
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
                        match recover_from_oauth_required().await {
                            OAuthRecoveryOutcome::Restart { token } => {
                                let new_poke = make_poke(&token)?;
                                return Ok(MonitorTunnelResult::Restart { new_poke });
                            }
                            OAuthRecoveryOutcome::Failed(message) => {
                                log_status(&format!("Re-auth failed: {message}"));
                            }
                        }
                        return Ok(MonitorTunnelResult::Disconnected);
                    }
                    Ok(TunnelEvent::Error(message)) => {
                        if is_non_fatal_bridge_error(&message) {
                            log_status(&format!("Bridge warning: {message}"));
                        } else {
                            log_status(&format!("Bridge error: {message}"));
                            return Ok(MonitorTunnelResult::Disconnected);
                        }
                    }
                    Ok(TunnelEvent::Created(_)) | Ok(TunnelEvent::Connected(_)) => {}
                    Err(_) => return Ok(MonitorTunnelResult::Disconnected),
                }
            }
            command = rx.recv() => {
                match command {
                    Some(BridgeCommand::SendWebhook(message)) => {
                        send_webhook_message(poke, webhook_url, webhook_token, &message).await;
                    }
                    Some(BridgeCommand::Stop) | None => {
                        return Ok(MonitorTunnelResult::StopRequested);
                    }
                }
            }
        }
    }
}

async fn start_tunnel_runner(
    runner: &mut TunnelRunner,
    rx: &mut async_mpsc::UnboundedReceiver<BridgeCommand>,
    poke: &Poke,
    webhook_url: &str,
    webhook_token: &str,
) -> StartTunnelResult {
    let start = runner.start();
    tokio::pin!(start);
    loop {
        tokio::select! {
            result = &mut start => {
                return match result {
                    Ok(info) => StartTunnelResult::Success(info),
                    Err(err) => StartTunnelResult::Error(err),
                };
            }
            command = rx.recv() => {
                match command {
                    Some(BridgeCommand::SendWebhook(message)) => {
                        send_webhook_message(poke, webhook_url, webhook_token, &message).await;
                    }
                    Some(BridgeCommand::Stop) | None => {
                        return StartTunnelResult::StopRequested;
                    }
                }
            }
        }
    }
}

async fn sleep_or_stop(
    rx: &mut async_mpsc::UnboundedReceiver<BridgeCommand>,
    duration: Duration,
    stop_requested: &mut bool,
) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        match rx.try_recv() {
            Ok(BridgeCommand::Stop) | Err(async_mpsc::error::TryRecvError::Disconnected) => {
                *stop_requested = true;
                return;
            }
            Ok(BridgeCommand::SendWebhook(_)) | Err(async_mpsc::error::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OAuthRecoveryOutcome {
    Restart { token: String },
    Failed(String),
}

fn plan_oauth_recovery(
    cached_reauth: std::result::Result<String, String>,
    fresh_login: std::result::Result<String, String>,
) -> OAuthRecoveryOutcome {
    match cached_reauth {
        Ok(token) => OAuthRecoveryOutcome::Restart { token },
        Err(cached_err) => match fresh_login {
            Ok(token) => OAuthRecoveryOutcome::Restart { token },
            Err(fresh_err) => OAuthRecoveryOutcome::Failed(format!(
                "{cached_err}; fresh login failed: {fresh_err}"
            )),
        },
    }
}

async fn recover_from_oauth_required() -> OAuthRecoveryOutcome {
    let cached = ensure_auth(false).await.map_err(|err| err.to_string());
    if cached.is_ok() {
        return plan_oauth_recovery(cached, Err("skipped".into()));
    }
    log_status("Cached credentials invalid - opening browser for fresh Poke login...");
    let fresh = ensure_auth(true).await.map_err(|err| err.to_string());
    plan_oauth_recovery(cached, fresh)
}

async fn ensure_auth(force_fresh: bool) -> Result<String> {
    if !force_fresh {
        if let Some(token) = rs_poke::get_token()? {
            return Ok(token);
        }
    } else {
        rs_poke::logout()
            .await
            .map_err(|err| Error::msg(err.to_string()))?;
    }
    log_status("Opening browser for Poke login...");
    let store = CredentialsStore::default_store().map_err(|err| Error::msg(err.to_string()))?;
    let options = LoginOptions::new(store).on_code(|info| {
        log_status(&format!(
            "Enter code {} at {}",
            info.user_code, info.login_url
        ));
    });
    let login = if force_fresh {
        rs_poke::login_fresh(options).await
    } else {
        rs_poke::login(options).await
    };
    login
        .map(|result| result.token)
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
        ("triggerId", Value::String(webhook.trigger_id.clone())),
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
        let _ = fetch_with_auth(FetchWithAuthOptions {
            path: &format!("/mcp/connections/{id}"),
            method: reqwest::Method::DELETE,
            body: None,
            token: Some(poke.api_key().to_string()),
            base_url: Some(poke.base_url().to_string()),
            client: None,
        })
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
        if reported > 0 {
            log_status(&format!("Tools synced: {reported}"));
            return reported;
        }
        if attempt < SYNC_TOOLS_MAX_ATTEMPTS {
            if local > 0 {
                log_status(&format!(
                    "sync-tools API returned 0 (attempt {attempt}/{SYNC_TOOLS_MAX_ATTEMPTS}); local MCP has {local} tools, retrying in {}s...",
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
    std::fs::write(&path, serde_json::to_string_pretty(state)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
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

fn is_non_fatal_bridge_error(message: &str) -> bool {
    message.contains("activate-tunnel") || message.contains("sync-tools")
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
        let token = ensure_auth(false).await?;
        let poke = make_poke(&token)?;
        poke.send_message(message)
            .await
            .map_err(|err| Error::msg(err.to_string()))?;
        println!("sent");
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integration_name_appends_hostname_suffix() {
        let name = integration_name("poke-around");
        assert!(name.starts_with("poke-around"));
    }

    #[test]
    fn non_fatal_errors_include_activate_and_sync_tools() {
        assert!(is_non_fatal_bridge_error("activate-tunnel failed"));
        assert!(is_non_fatal_bridge_error("sync-tools returned 0"));
        assert!(!is_non_fatal_bridge_error("connection timeout"));
    }

    #[test]
    fn oauth_recovery_restarts_when_cached_token_is_valid() {
        let outcome = plan_oauth_recovery(Ok("pk_cached".into()), Err("skipped".into()));
        assert_eq!(
            outcome,
            OAuthRecoveryOutcome::Restart {
                token: "pk_cached".into()
            }
        );
    }

    #[test]
    fn oauth_recovery_falls_back_to_fresh_login() {
        let outcome = plan_oauth_recovery(Err("cached invalid".into()), Ok("pk_fresh".into()));
        assert_eq!(
            outcome,
            OAuthRecoveryOutcome::Restart {
                token: "pk_fresh".into()
            }
        );
    }

    #[test]
    fn oauth_recovery_reports_both_failures() {
        let outcome = plan_oauth_recovery(
            Err("cached invalid".into()),
            Err("browser login timed out".into()),
        );
        assert_eq!(
            outcome,
            OAuthRecoveryOutcome::Failed(
                "cached invalid; fresh login failed: browser login timed out".into()
            )
        );
    }
}
