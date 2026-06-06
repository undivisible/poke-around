# CLAUDE.md

This file provides guidance for working in this repository.

## Overview

Poke Around is a Rust daemon that starts a local MCP server, launches the bridge process, and forwards tool calls from Poke to the user's machine. The bridge remains TypeScript and uses the Poke SDK.

## Architecture

1. `crates/poke-around/src/main.rs` — CLI entry point, parses flags, dispatches commands
2. `crates/poke-around/src/daemon.rs` — daemon startup and bridge lifecycle
3. `crates/poke-around/src/mcp.rs` — JSON-RPC over HTTP, tool dispatch and execution
4. `crates/poke-around/src/policy.rs` — access modes, command filtering, approval classification
5. `crates/poke-around/src/agents.rs` — discovers and runs scheduled agent scripts
6. `crates/poke-around/src/bridge.rs` — launches the Poke SDK bridge process
7. `bridge/poke-bridge.ts` — TypeScript bridge bundled into `bridge/dist/poke-around-bridge.js`

## Commands

```bash
cargo build --workspace    # build
cargo run --bin poke-around # build and run
cargo test --workspace     # run tests
bun run build:bridge       # bundle bridge/poke-bridge.ts → bridge/dist/poke-around-bridge.js
bun run test:install       # validate scripts/install.sh
bun run release            # build bridge + Rust release binary
```

## Rust version

The active daemon targets stable Rust with edition 2024.

## Runtime notes

- Bridge is bundled from `bridge/poke-bridge.ts`; `bridge/dist/` is gitignored (built by CI).
- Binaries go to `target/release/`. The bridge JS must sit alongside the binary at runtime.
- Config/state: `~/.config/poke-around/`
- Agents: `~/.config/poke-around/agents/<name>.<interval>.js`
- Webhook credentials are cached in `~/.config/poke-around/state.json` — not recreated on reconnect.

## Release

Pushing a `v*.*.*` tag triggers `.github/workflows/release.yml`, which builds binaries for
macOS (arm64, x86_64) and Linux (x86_64), packages them with the bridge, and uploads them to a GitHub release.
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

If a change affects the bridge, rebuild with `bun run build:bridge`.

## Stability expectations

- Preserve approval and permission semantics.
- Prefer small, mechanical fixes.
- Do not overwrite unrelated user changes in the working tree.
