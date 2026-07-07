use crate::bridge_auth::{
    self, OAuthRecoveryOutcome, ensure_auth, make_poke, recover_from_oauth_required,
};
use crate::bridge_state::{
    log_status, patch_state, read_state, record_connection, remove_state_key,
};
use crate::{Error, Result};
use futures::future::join_all;
use rs_poke::{
    CreateWebhook, FetchWithAuthOptions, Poke, TunnelEvent, TunnelOptions, TunnelRunner,
    fetch_with_auth,
};
use serde_json::Value;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc as async_mpsc;

const BRIDGE_STOP_TIMEOUT: Duration = Duration::from_secs(30);
const RECONNECT_BASE_DELAY: Duration = Duration::from_secs(15);
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(60);
const TUNNEL_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const SYNC_TOOLS_MAX_ATTEMPTS: usize = 8;
const SYNC_TOOLS_RETRY_DELAY: Duration = Duration::from_secs(3);
const TUNNEL_HEALTH_CHECK: Duration = Duration::from_secs(30);
const DELETE_MAX_ATTEMPTS: usize = 3;
const DELETE_RETRY_DELAY: Duration = Duration::from_secs(2);

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
    let tunnel_name = bridge_auth::ensure_integration_name("poke-around")?;
    let (webhook_url, webhook_token) = ensure_webhook(&poke, &tunnel_name).await?;
    log_status("Webhook ready.");
    cleanup_stale_connections(&poke, &webhook_url, &webhook_token).await?;

    let mut stop_requested = false;
    let mut reconnect_attempt = 0u32;
    let mut notified_agent = false;
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
                stop_tunnel(&mut runner, true).await;
                return Ok(());
            }
            StartTunnelResult::Success(info) => {
                reconnect_attempt = 0;
                record_connection(&info.connection_id)?;
                log_status(&format!(
                    "Tunnel connected ({}) -> {}",
                    info.connection_id, info.tunnel_url
                ));
                if !notified_agent {
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
                    notified_agent = true;
                } else {
                    log_status("Tunnel reconnected; skipping duplicate agent notification.");
                }
                let synced = sync_and_report_tools(&runner, &mcp_url).await;
                if synced > 0 {
                    log_status("Ready - your Poke agent can now access this machine.");
                } else {
                    log_status("Tools not synced yet; Poke may not be able to use this machine.");
                }
            }
            StartTunnelResult::Error(err) => {
                log_status(&format!("Bridge error: {err}"));
                stop_tunnel(&mut runner, false).await;
                cleanup_stale_connections(&poke, &webhook_url, &webhook_token).await?;
                reconnect_attempt = reconnect_attempt.saturating_add(1);
                let delay = reconnect_backoff(reconnect_attempt);
                log_status(&format!(
                    "Retrying tunnel in {}s (attempt {reconnect_attempt})...",
                    delay.as_secs()
                ));
                sleep_or_stop(
                    &mut rx,
                    delay,
                    &mut stop_requested,
                    &poke,
                    &webhook_url,
                    &webhook_token,
                )
                .await;
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
                stop_tunnel(&mut runner, true).await;
                return Ok(());
            }
            MonitorTunnelResult::Restart { new_poke } => {
                poke = new_poke;
                restart_tunnel = true;
            }
            MonitorTunnelResult::Disconnected => {}
        }

        stop_tunnel(&mut runner, false).await;
        cleanup_stale_connections(&poke, &webhook_url, &webhook_token).await?;
        if restart_tunnel {
            reconnect_attempt = 0;
            log_status("Restarting tunnel with refreshed credentials...");
            sleep_or_stop(
                &mut rx,
                Duration::from_secs(1),
                &mut stop_requested,
                &poke,
                &webhook_url,
                &webhook_token,
            )
            .await;
            continue;
        }
        reconnect_attempt = reconnect_attempt.saturating_add(1);
        let delay = reconnect_backoff(reconnect_attempt);
        log_status(&format!(
            "Tunnel will retry in {}s (attempt {reconnect_attempt})...",
            delay.as_secs()
        ));
        sleep_or_stop(
            &mut rx,
            delay,
            &mut stop_requested,
            &poke,
            &webhook_url,
            &webhook_token,
        )
        .await;
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
                                log_status(&format!(
                                    "Re-auth failed: {message}{}",
                                    bridge_auth::oauth_failure_hint()
                                ));
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
    poke: &Poke,
    webhook_url: &str,
    webhook_token: &str,
) {
    let mut buffered = Vec::new();
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        match rx.try_recv() {
            Ok(BridgeCommand::Stop) | Err(async_mpsc::error::TryRecvError::Disconnected) => {
                *stop_requested = true;
                return;
            }
            Ok(BridgeCommand::SendWebhook(message)) => {
                buffered.push(message);
            }
            Err(async_mpsc::error::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
    for message in buffered {
        send_webhook_message(poke, webhook_url, webhook_token, &message).await;
    }
}

async fn ensure_webhook(poke: &Poke, tunnel_name: &str) -> Result<(String, String)> {
    let state = read_state()?;
    let webhook_url = state.get("webhookUrl").and_then(Value::as_str);
    let webhook_token = state.get("webhookToken").and_then(Value::as_str);
    let webhook_name = state.get("webhookName").and_then(Value::as_str);
    if let (Some(url), Some(token), Some(name)) = (webhook_url, webhook_token, webhook_name)
        && name == tunnel_name
    {
        log_status("Reusing cached webhook.");
        return Ok((url.to_string(), token.to_string()));
    }
    log_status("Creating webhook (first run)...");
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
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();

    if let Some(id) = state.get("connectionId").and_then(Value::as_str) {
        ids.push(id.to_string());
        seen.insert(id);
    }
    if let Some(history) = state.get("connectionHistory").and_then(Value::as_array) {
        for id in history.iter().filter_map(Value::as_str) {
            if seen.insert(id) {
                ids.push(id.to_string());
            }
        }
    }
    if ids.is_empty() {
        return Ok(());
    }
    log_status(&format!("Cleaning up {} old connection(s)...", ids.len()));
    let poke = poke.clone();
    let futures = ids.into_iter().map(|id| {
        let poke = poke.clone();
        async move { delete_remote_connection_with_retry(&poke, &id).await }
    });
    join_all(futures).await;
    patch_state([
        ("webhookUrl", Value::String(webhook_url.to_string())),
        ("webhookToken", Value::String(webhook_token.to_string())),
        ("connectionHistory", Value::Array(Vec::new())),
    ])?;
    remove_state_key("connectionId")?;
    Ok(())
}

async fn delete_remote_connection_with_retry(poke: &Poke, connection_id: &str) {
    for attempt in 1..=DELETE_MAX_ATTEMPTS {
        match delete_remote_connection(poke, connection_id).await {
            Ok(()) => return,
            Err(err) => {
                log_status(&format!(
                    "Failed to delete connection {connection_id} (attempt {attempt}/{DELETE_MAX_ATTEMPTS}): {err}"
                ));
                if attempt < DELETE_MAX_ATTEMPTS {
                    tokio::time::sleep(DELETE_RETRY_DELAY).await;
                }
            }
        }
    }
}

async fn delete_remote_connection(poke: &Poke, connection_id: &str) -> Result<()> {
    let response = fetch_with_auth(FetchWithAuthOptions {
        path: &format!("/mcp/connections/{connection_id}"),
        method: reqwest::Method::DELETE,
        body: None,
        token: Some(poke.api_key().to_string()),
        base_url: Some(poke.base_url().to_string()),
        client: None,
    })
    .await?;
    let status = response.status();
    if status.is_success() || status == reqwest::StatusCode::NOT_FOUND {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    Err(Error::msg(format!(
        "delete-connection HTTP {}: {body}",
        status.as_u16()
    )))
}

async fn delete_connection_with_retry(runner: &TunnelRunner, connection_id: &str) {
    for attempt in 1..=DELETE_MAX_ATTEMPTS {
        match runner.delete_connection(connection_id).await {
            Ok(()) => return,
            Err(err) => {
                log_status(&format!(
                    "Failed to delete connection {connection_id} (attempt {attempt}/{DELETE_MAX_ATTEMPTS}): {err}"
                ));
                if attempt < DELETE_MAX_ATTEMPTS {
                    tokio::time::sleep(DELETE_RETRY_DELAY).await;
                }
            }
        }
    }
}

async fn stop_tunnel(runner: &mut TunnelRunner, delete_remote: bool) {
    if delete_remote && let Some(info) = runner.info() {
        delete_connection_with_retry(runner, &info.connection_id).await;
    }
    let _ = runner.stop().await;
}

fn reconnect_backoff(attempt: u32) -> Duration {
    let multiplier = 2u64.saturating_pow(attempt.saturating_sub(1).min(4));
    let secs = RECONNECT_BASE_DELAY
        .as_secs()
        .saturating_mul(multiplier)
        .min(RECONNECT_MAX_DELAY.as_secs());
    Duration::from_secs(secs)
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
        Ok(_) => log_status("Notified Poke agent."),
        Err(err) => log_status(&format!("Bridge error: {err}")),
    }
}

async fn perform_sync_attempt(runner: &TunnelRunner, mcp_url: &str) -> (usize, usize) {
    let last_synced = match runner.sync_tools().await {
        Ok(count) => count,
        Err(err) => {
            log_status(&format!("Tool sync error: {err}"));
            0
        }
    };
    let local = local_tool_count(mcp_url).await;
    (last_synced, local)
}

async fn handle_sync_retry(attempt: usize, local: usize) {
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

async fn sync_and_report_tools(runner: &TunnelRunner, mcp_url: &str) -> usize {
    let mut last_synced = 0usize;
    for attempt in 1..=SYNC_TOOLS_MAX_ATTEMPTS {
        let (synced, local) = perform_sync_attempt(runner, mcp_url).await;
        last_synced = synced;

        let reported = last_synced.max(local);
        if reported > 0 {
            log_status(&format!("Tools synced: {reported}"));
            return reported;
        }

        handle_sync_retry(attempt, local).await;
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

fn is_non_fatal_bridge_error(message: &str) -> bool {
    message.contains("activate-tunnel") || message.contains("sync-tools")
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
    fn reconnect_backoff_grows_to_cap() {
        assert_eq!(reconnect_backoff(1), Duration::from_secs(15));
        assert_eq!(reconnect_backoff(2), Duration::from_secs(30));
        assert_eq!(reconnect_backoff(3), Duration::from_secs(60));
        assert_eq!(reconnect_backoff(10), Duration::from_secs(60));
    }

    #[test]
    fn non_fatal_errors_include_activate_and_sync_tools() {
        assert!(is_non_fatal_bridge_error("activate-tunnel failed"));
        assert!(is_non_fatal_bridge_error("sync-tools returned 0"));
        assert!(!is_non_fatal_bridge_error("connection timeout"));
    }

    use serial_test::serial;

    #[test]
    #[serial]
    fn ensure_integration_name_persists_first_computed_value() {
        let temp_dir = tempfile::tempdir().unwrap();
        let original_env = std::env::var_os("XDG_CONFIG_HOME");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let first = bridge_auth::ensure_integration_name("poke-around").expect("first name");
        let second = bridge_auth::ensure_integration_name("poke-around").expect("cached name");
        assert_eq!(first, second);

        let state_path = temp_dir.path().join("poke-around/state.json");
        let state: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&state_path)
                .unwrap_or_else(|err| panic!("read {}: {err}", state_path.display())),
        )
        .unwrap();
        assert_eq!(state["integrationName"], first);

        unsafe {
            if let Some(val) = original_env {
                std::env::set_var("XDG_CONFIG_HOME", val);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }

    #[test]
    fn oauth_recovery_restarts_when_cached_token_is_valid() {
        let outcome =
            crate::bridge_auth::plan_oauth_recovery(Ok("pk_cached".into()), Err("skipped".into()));
        assert_eq!(
            outcome,
            OAuthRecoveryOutcome::Restart {
                token: "pk_cached".into()
            }
        );
    }

    #[test]
    fn oauth_recovery_falls_back_to_fresh_login() {
        let outcome = crate::bridge_auth::plan_oauth_recovery(
            Err("cached invalid".into()),
            Ok("pk_fresh".into()),
        );
        assert_eq!(
            outcome,
            OAuthRecoveryOutcome::Restart {
                token: "pk_fresh".into()
            }
        );
    }

    #[test]
    fn oauth_recovery_reports_both_failures() {
        let outcome = crate::bridge_auth::plan_oauth_recovery(
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

    #[tokio::test]
    #[serial]
    async fn ensure_auth_returns_error_on_invalid_credentials_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let original_env = std::env::var_os("XDG_CONFIG_HOME");

        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }
        let poke_dir = temp_dir.path().join("poke");
        std::fs::create_dir(&poke_dir).unwrap();
        std::fs::write(poke_dir.join("credentials.json"), b"not json").unwrap();

        let result = crate::bridge_auth::ensure_auth(false).await;
        let is_err = result.is_err();

        unsafe {
            if let Some(val) = original_env {
                std::env::set_var("XDG_CONFIG_HOME", val);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }

        assert!(is_err);
    }
}
