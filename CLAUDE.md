# CLAUDE.md

This file provides guidance for working in this repository.

## Overview

Poke Around is a Rust daemon that starts a local MCP server, runs an in-process Poke tunnel bridge, and forwards tool calls from Poke to the user's machine.

## Architecture

1. `crates/poke-around/src/main.rs` — CLI entry point, parses flags, dispatches commands
2. `crates/poke-around/src/daemon.rs` — daemon startup and bridge lifecycle
3. `crates/poke-around/src/mcp.rs` — JSON-RPC over HTTP, tool dispatch and execution
4. `crates/poke-around/src/policy.rs` — access modes, command filtering, approval classification
5. `crates/poke-around/src/agents.rs` — discovers agent scripts; runs them on demand via `poke-around run-agent` and the `run_agent` MCP tool (no built-in scheduler)
6. `crates/poke-around/src/bridge.rs` — in-process tunnel bridge via `rs_poke`

## Commands

```bash
cargo build --workspace    # build
cargo run --bin poke-around # build and run
cargo test --workspace     # run tests
bash scripts/test-install.sh
cargo build --workspace --release
```

## Rust version

The active daemon targets stable Rust with edition 2024.

## Runtime notes

- Binaries go to `target/release/`.
- Config/state: `~/.config/poke-around/`
- Agents: `~/.config/poke-around/agents/<name>.<interval>.js`
- Webhook credentials are cached in `~/.config/poke-around/state.json` — not recreated on reconnect.
- User auth uses `rs_poke` device login and `~/.config/poke/credentials.json`.

## Release

Pushing a `v*.*.*` tag triggers `.github/workflows/release.yml`, which builds binaries for
macOS (arm64, x86_64), Linux (x86_64, aarch64), and Windows (x86_64) and uploads them to a GitHub release.
The homebrew-tap formula is updated automatically by its own workflow within ~1 hour.

## Validation

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --release
bash scripts/test-install.sh
```

## Stability expectations

- Preserve approval and permission semantics.
- Prefer small, mechanical fixes.
- Do not overwrite unrelated user changes in the working tree.