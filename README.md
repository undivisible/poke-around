# Poke Around

Let your [Poke](https://poke.com) AI assistant access your machine.

<sub>A community project — not affiliated with Poke or The Interaction Company.</sub>

[![Latest Release](https://img.shields.io/github/v/release/undivisible/poke-around?style=flat-square)](https://github.com/undivisible/poke-around/releases/latest)
[![License](https://img.shields.io/github/license/undivisible/poke-around?style=flat-square)](https://github.com/undivisible/poke-around/blob/main/LICENSE)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-blue?style=flat-square)

---

Run Poke Around on your machine, then message Poke from iMessage, Telegram, or SMS to run commands, read files, take screenshots, and more — all on your machine.

## Install

**Homebrew (macOS / Linux)**

```bash
brew tap undivisible/tap
brew install poke-around
```

**Build from source**

Requires [Zig 0.15](https://ziglang.org/download/) and [Bun](https://bun.sh):

```bash
git clone https://github.com/undivisible/poke-around.git
cd poke-around
bun run build:bridge
zig build -Doptimize=ReleaseSafe
./zig-out/bin/poke-around
```

**Manual download**

Download the latest binary for your platform from [Releases](https://github.com/undivisible/poke-around/releases).

On macOS, if the binary is blocked by Gatekeeper:

```bash
xattr -cr poke-around
```

## Usage

```bash
poke-around          # start the daemon (opens browser for OAuth on first run)
poke-around --verbose  # show tool calls in real time
poke-around --mode limited
poke-around --mode sandbox
```

Config is stored at `~/.config/poke-around/config.json`.

## Access modes

| Mode | Description |
|------|-------------|
| **full** (default) | All tools, no approval required. |
| **limited** | Read-only tools plus a curated set of safe commands (`ls`, `cat`, `grep`, `curl`, etc.). |
| **sandbox** | Broader command support, but writes restricted to `~/Downloads` and `/tmp`. |

```bash
poke-around --mode sandbox
# or
POKE_GATE_PERMISSION_MODE=limited poke-around
```

## System tray

- **macOS** — native menu bar icon (AppKit)
- **Linux** — AppIndicator tray icon via `menubar_linux.py` (requires `python3-gi` and `libayatana-appindicator3-0.1`)

## Running as a service

**macOS (Homebrew)**

```bash
brew services start poke-around
```

**Linux (systemd)**

```ini
# ~/.config/systemd/user/poke-around.service
[Unit]
Description=Poke Around Daemon
After=network.target

[Service]
ExecStart=%h/.local/bin/poke-around
Restart=always

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now poke-around
```

## Agents

Agents are scheduled JS scripts in `~/.config/poke-around/agents/` named `<name>.<interval>.js`:

| File | Runs |
|------|------|
| `beeper.1h.js` | every hour |
| `health.10m.js` | every 10 minutes |

Minimum interval is 10 minutes. Agents can import the `poke` SDK and send messages back via `poke.sendMessage(...)`.

```javascript
import { Poke, getToken } from "poke";
const poke = new Poke({ apiKey: getToken() });
await poke.sendMessage("Hello from my agent!");
```

Per-agent secrets go in `~/.config/poke-around/agents/.env.<name>`.

## Security

In **full mode**, Poke Around grants full shell access to your Poke agent. Only run it on machines and networks you trust. Use `limited` or `sandbox` mode for tighter restrictions.

See [SECURITY.md](SECURITY.md) for the vulnerability disclosure policy.

## Credits

- Native Zig rewrite of [f/poke-gate](https://github.com/f/poke-gate)
- [Poke](https://poke.com) by [The Interaction Company](https://interaction.co)

## License

MPL-2.0
