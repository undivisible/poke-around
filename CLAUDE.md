# CLAUDE.md

This file provides guidance for working in this repository.

## Overview

Poke Around is a native Zig daemon that starts a local MCP server, launches the bridge process, and forwards tool calls from Poke to the user's machine.

## Architecture

1. `src/main.zig` — CLI entry point, parses flags, dispatches commands
2. `src/app.zig` — daemon and bridge lifecycle, reconnect loop
3. `src/mcp_server.zig` — JSON-RPC over HTTP, tool dispatch and execution
4. `src/permission.zig` — approval tokens and per-session permissions
5. `src/platform.zig` — shell helpers, sandbox wrapping, command filtering
6. `src/agents.zig` — discovers and runs scheduled agent scripts
7. `src/menubar.zig` — native tray icon (macOS: AppKit via zig-objc; Linux: menubar_linux.py)
8. `bridge/poke-bridge.ts` — TypeScript bridge bundled into `bridge/dist/poke-around-bridge.js`

## Commands

```bash
zig build                  # build
zig build run              # build and run
zig test src/main.zig      # run tests
bun run build:bridge       # bundle bridge/poke-bridge.ts → bridge/dist/poke-around-bridge.js
bun run release            # build bridge + zig release-all
```

## Zig version

The codebase targets **Zig 0.15.2**. Key API patterns in use:

- `std.ArrayList` methods require an allocator argument
- `toOwnedSlice(allocator)` not zero-argument
- `std.Thread.sleep()` not `std.time.sleep()`
- `std.atomic.Value(T)` for atomics
- `b.createModule(...)` in build.zig

## Runtime notes

- Bridge is bundled from `bridge/poke-bridge.ts`; `bridge/dist/` is gitignored (built by CI).
- Binaries go to `zig-out/bin/`. The bridge JS must sit alongside the binary at runtime.
- Config/state: `~/.config/poke-around/`
- Agents: `~/.config/poke-around/agents/<name>.<interval>.js`
- Webhook credentials are cached in `~/.config/poke-around/state.json` — not recreated on reconnect.

## Release

Pushing a `v*.*.*` tag triggers `.github/workflows/release.yml`, which builds binaries for
macOS (arm64, x86_64) and Linux (x86_64) and uploads them to a GitHub release.
The homebrew-tap formula is updated automatically by its own workflow within ~1 hour.

## Validation

```bash
zig build
zig test src/main.zig
```

`zig build-exe src/main.zig` does **not** work standalone (requires `build_options` from build.zig).

If a change affects the bridge, rebuild with `bun run build:bridge`.

## Stability expectations

- Preserve approval and permission semantics.
- Prefer small, mechanical fixes.
- Do not overwrite unrelated user changes in the working tree.
