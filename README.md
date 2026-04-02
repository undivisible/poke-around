# Poke Around

Poke Around exposes your machine to your Poke assistant over an MCP tunnel so you can run commands, inspect files, take screenshots, and launch scheduled agents on your own computer.

## Current status

- The current checkout is on feature/computer-use-integration.
- zig test src/main.zig currently passes locally.

## Requirements

- Zig 0.15+
- Bun 1.0+
- Node.js (used for agent scripts and the bridge runtime when Bun is unavailable)

## Build

```bash
zig build
```

That builds the native Zig daemon and installs the bridge script into `zig-out/bin/`.

To build the bridge bundle directly:

```bash
bun run build:bridge
```

To create release binaries for all supported targets:

```bash
bun run release
```

## Run

```bash
zig build run
# or
./zig-out/bin/poke-around
```

On first launch, Poke OAuth opens in your browser and connects the tunnel.

Useful flags:

```bash
./zig-out/bin/poke-around --verbose
./zig-out/bin/poke-around --mode full
./zig-out/bin/poke-around --mode limited
./zig-out/bin/poke-around --mode sandbox
```

## Commands

```bash
poke-around status
poke-around notify
poke-around restart
poke-around take-screenshot
poke-around set-mode <full|limited|sandbox>
poke-around run-agent <name>
poke-around agent get <name>
poke-around agent create --prompt "..."
```

## Configuration

Config lives in:

```bash
~/.config/poke-around/
```

Key files:

- `config.json` — saved permission mode
- `state.json` — runtime state used by the tunnel and bridge
- `agents/` — installed and custom agent scripts

## Agents

Agents are JavaScript files stored in `~/.config/poke-around/agents/` and named:

```text
<name>.<interval>.js
```

Examples:

- `beeper.1h.js`
- `backup.2h.js`
- `cleanup.30m.js`

Intervals use minutes or hours, and the minimum supported interval is 10 minutes.

Each agent can have a matching env file:

```bash
~/.config/poke-around/agents/.env.<name>
```

Example frontmatter:

```javascript
/**
 * @agent beeper
 * @name Beeper Message Digest
 * @description Fetches messages and sends a summary to Poke.
 * @interval 1h
 * @author you
 */
```

## Access modes

| Mode | Description |
| --- | --- |
| full | Full shell access. Risky actions require approval. |
| limited | Read-only tools plus a small safe command allowlist. |
| sandbox | Command execution is broader, but writes are restricted to temporary locations. |

## Security

Poke Around grants your Poke agent shell access through a tunnel on your machine. Only run it on machines and networks you trust.

## Project layout

- `src/` — Zig daemon, HTTP/MCP server, permissions, platform helpers
- `bridge/` — Poke bridge bundle and source
- `docs/` — user-facing documentation
- `examples/agents/` — sample scheduled agents

## License

MIT
