use crate::bridge::Bridge;
use crate::mcp::{AppState, start_server};
use crate::policy::PermissionMode;
use crate::{Result, config};
use std::thread;
use std::time::Duration;

pub fn run(mode_arg: Option<&str>, verbose: bool) -> Result<()> {
    let saved = config::read_config()?.permission_mode;
    let mode = PermissionMode::parse(mode_arg.or(saved.as_deref()));
    let state = AppState::new(mode, verbose)?;
    let port = start_server(state)?;
    let mcp_url = format!("http://127.0.0.1:{port}/mcp");
    eprintln!("poke-around MCP server listening on {mcp_url}");
    let _bridge = Bridge::start(&mcp_url)?;
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
