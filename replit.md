# Poke Around

A native daemon that exposes your machine to the [Poke](https://poke.com) AI assistant via MCP tunnel.

## Architecture

- **Language:** Zig 0.15.2 (daemon binary) + TypeScript/Bun (bridge)
- **Binary:** `zig-out/bin/poke-around`
- **Bridge:** `bridge/dist/poke-around-bridge.js` (bundled from `bridge/poke-bridge.ts`)
- **Config:** `~/.config/poke-around/config.json`

### Source files
- `src/main.zig` — CLI entry point, parses flags, dispatches commands
- `src/app.zig` — daemon and bridge lifecycle, reconnect loop
- `src/mcp_server.zig` — JSON-RPC over HTTP, tool dispatch and execution
- `src/permission.zig` — approval tokens and per-session permissions
- `src/platform.zig` — shell helpers, sandbox wrapping, command filtering
- `src/agents.zig` — discovers and runs scheduled agent scripts
- `src/menubar.zig` — native tray icon (macOS: AppKit; Linux: menubar_linux.py)
- `bridge/poke-bridge.ts` — TypeScript bridge (bundled into `bridge/dist/poke-around-bridge.js`)

## Build

Zig 0.15.2 is installed at `/home/runner/zig/zig-x86_64-linux-0.15.2/zig`. Always add it to PATH:

```bash
export PATH="/home/runner/zig/zig-x86_64-linux-0.15.2:$PATH"
bun run build:bridge    # bundle TypeScript bridge
zig build               # compile Zig binary to zig-out/bin/poke-around
```

## Usage

```bash
./zig-out/bin/poke-around --version
./zig-out/bin/poke-around           # start daemon (OAuth browser on first run)
./zig-out/bin/poke-around --verbose
./zig-out/bin/poke-around --mode limited
./zig-out/bin/poke-around --mode sandbox
```

## Access modes

| Mode | Description |
|------|-------------|
| **full** (default) | All tools, no approval required |
| **limited** | Read-only tools + safe commands (ls, cat, grep, curl, …) |
| **sandbox** | Broader commands, writes restricted to ~/Downloads and /tmp |

## Workflow

The "Build poke-around" console workflow builds the bridge and binary, then verifies the version.

## Dependencies

- Zig 0.15.2 (installed to `/home/runner/zig/`)
- Bun 1.3.6 (pre-installed in Replit)
- Node.js 20 (pre-installed in Replit)
- Bridge npm dep: `poke@^0.4.2`
