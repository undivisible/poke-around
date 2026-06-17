use crate::bridge::Bridge;
use crate::mcp::{AppState, start_server};
use crate::policy::PermissionMode;
use crate::{Result, config};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub fn run(mode_arg: Option<&str>, verbose: bool) -> Result<()> {
    let saved = config::read_config()?.permission_mode;
    let mode = PermissionMode::parse(mode_arg.or(saved.as_deref()));
    let state = AppState::new(mode, verbose)?;
    if mode_arg.is_none() {
        spawn_config_poll(state.clone());
    }
    crate::platform::log_gui_session_readiness(verbose);
    let port = start_server(state)?;
    let mcp_url = format!("http://127.0.0.1:{port}/mcp");
    eprintln!("poke-around MCP server listening on {mcp_url}");
    let mut bridge = Bridge::start(&mcp_url, mode.as_str())?;
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
            if let Ok(config) = config::read_config() {
                state.set_mode(PermissionMode::parse(config.permission_mode.as_deref()));
            }
        }
    });
}
