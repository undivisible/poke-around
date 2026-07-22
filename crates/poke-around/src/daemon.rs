use crate::bridge::Bridge;
use crate::mcp::AppState;
use crate::mcp_server::{new_bearer_capability, start_server};
use crate::policy::{ApprovalMode, PermissionMode};
use crate::{Result, config};
use std::io::{self, IsTerminal};
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
    let mut bridge = Bridge::start(&mcp_url, &mcp_bearer, mode.as_str(), approval_mode.as_str())?;
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = shutdown_tx.send(());
    })
    .map_err(|err| crate::Error::msg(format!("failed to set Ctrl-C handler: {err}")))?;
    let _ = shutdown_rx.recv();
    eprintln!("poke-around shutting down");
    bridge.stop()?;
    Ok(())
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
