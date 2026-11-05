use crate::bridge::Bridge;
use crate::mcp::AppState;
use crate::mcp_server::{new_bearer_capability, start_server};
use crate::policy::{ApprovalMode, PermissionMode};
use crate::{Result, config};
use std::io::{self, IsTerminal, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn run(mode_arg: Option<&str>, approval_mode_arg: Option<&str>, verbose: bool) -> Result<()> {
    let saved = config::read_config()?;
    let mode = PermissionMode::parse(mode_arg.or(saved.permission_mode.as_deref()));
    let approval_mode = ApprovalMode::parse(approval_mode_arg.or(saved.approval_mode.as_deref()));
    if approval_mode == ApprovalMode::PerAction && !io::stdin().is_terminal() {
        return Err(crate::Error::msg(
            "per-action approval mode requires an interactive host terminal",
        ));
    }
    let state = AppState::with_approval_mode(mode, approval_mode)?;
    if mode_arg.is_none() {
        spawn_config_poll(state.clone());
    }
    crate::platform::log_gui_session_readiness(verbose);
    let mcp_bearer = new_bearer_capability();
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = shutdown_tx.send(());
    })
    .map_err(|err| crate::Error::msg(format!("failed to set Ctrl-C handler: {err}")))?;
    let port = start_server(state, &mcp_bearer)?;
    let mcp_url = format!("http://127.0.0.1:{port}/mcp");
    eprintln!("poke-around MCP server listening on {mcp_url}");
    eprintln!(
        "poke-around approval mode: {}{}",
        approval_mode.as_str(),
        if approval_mode == ApprovalMode::Full {
            " (authorized by host launch)"
        } else {
            ""
        }
    );
    let _ = io::stderr().flush();
    let mut bridge = Bridge::start(&mcp_url, &mcp_bearer, mode.as_str(), approval_mode.as_str())?;
    let result = wait_for_shutdown(&shutdown_rx, bridge.completion());
    eprintln!("poke-around shutting down");
    bridge.stop()?;
    result
}

fn wait_for_shutdown(
    shutdown: &mpsc::Receiver<()>,
    bridge: &mpsc::Receiver<std::result::Result<(), String>>,
) -> Result<()> {
    loop {
        match bridge.try_recv() {
            Ok(Err(err)) => return Err(crate::Error::msg(format!("bridge failed: {err}"))),
            Ok(Ok(())) => return Err(crate::Error::msg("bridge exited unexpectedly")),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(crate::Error::msg("bridge thread terminated unexpectedly"));
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        match shutdown.recv_timeout(Duration::from_millis(100)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn spawn_config_poll(state: AppState) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(30));
            match config::read_config() {
                Ok(config) => {
                    state.set_mode(PermissionMode::parse(config.permission_mode.as_deref()));
                }
                Err(_) => state.set_mode(PermissionMode::Sandbox),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_exit_ends_daemon_wait() {
        let (_shutdown_tx, shutdown_rx) = mpsc::channel();
        let (bridge_tx, bridge_rx) = mpsc::channel();
        bridge_tx.send(Err("authentication failed".into())).unwrap();
        assert!(
            wait_for_shutdown(&shutdown_rx, &bridge_rx)
                .unwrap_err()
                .to_string()
                .contains("authentication failed")
        );
        drop(bridge_tx);
        assert!(wait_for_shutdown(&shutdown_rx, &bridge_rx).is_err());
    }

    #[test]
    fn shutdown_ends_daemon_wait() {
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let (_bridge_tx, bridge_rx) = mpsc::channel();
        shutdown_tx.send(()).unwrap();
        assert!(wait_for_shutdown(&shutdown_rx, &bridge_rx).is_ok());
    }
}
