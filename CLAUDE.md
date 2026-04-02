# CLAUDE.md

This file provides guidance for working in this repository.

## Overview

Poke Around is a native Zig rewrite of the Poke tunnel/daemon. It starts a local MCP server, launches the bridge process, and forwards tool calls from Poke to the user’s machine.

## Current status

- feature/computer-use-integration is the active branch in this checkout.
- Validate with zig test src/main.zig before merging or pushing.

## Main flow

1. `src/main.zig` parses CLI flags and dispatches commands.
2. `src/app.zig` resolves the bridge, starts the MCP server, and manages reconnects.
3. `src/mcp_server.zig` handles JSON-RPC over HTTP and executes tools.
4. `src/permission.zig` validates approvals and per-session permissions.
5. `src/platform.zig` contains command parsing, platform helpers, and shell command filtering.
6. `src/agents.zig` discovers and runs scheduled agent scripts.

## Commands

```bash
zig build
zig build run
zig test src/main.zig
zig build-exe src/main.zig
bun run build:bridge
bun run release
```

## Zig 0.15 notes

This codebase is being ported to Zig 0.15, so keep an eye on API changes such as:

- `std.ArrayList` methods requiring an allocator argument
- `toOwnedSlice(allocator)` instead of zero-argument `toOwnedSlice()`
- `writer(allocator)` instead of zero-argument `writer()`
- `std.Thread.sleep()` instead of `std.time.sleep()`

If you touch older code, expect nearby call sites to need the same update.

## Runtime notes

- The bridge is bundled from `bridge/poke-bridge.ts` into `bridge/dist/poke-around-bridge.js`.
- Native binaries are installed into `zig-out/bin/`.
- The default config directory is `~/.config/poke-around/`.
- Scheduled agents live in `~/.config/poke-around/agents/` and use the `<name>.<interval>.js` naming convention.

## Stability expectations

- Preserve existing approvals and permission semantics.
- Prefer small, mechanical fixes when porting APIs.
- Avoid changing generated or release artifacts unless the build pipeline is regenerating them.
- Do not overwrite unrelated user changes in the working tree.

## Useful files

- `src/main.zig` — CLI entrypoint
- `src/app.zig` — daemon and bridge lifecycle
- `src/mcp_server.zig` — tool dispatch and HTTP handling
- `src/config.zig` — config/state paths and JSON helpers
- `src/platform.zig` — command parsing and command execution helpers
- `bridge/poke-bridge.ts` — bridge source before bundling

## Validation

Prefer validating changes with:

```bash
zig build-exe src/main.zig
zig test src/main.zig
```

If a change affects the bridge, rebuild it with `bun run build:bridge`.
