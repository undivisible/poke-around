# Poke Around

Let your [Poke](https://poke.com) AI assistant access your machine.

<sub>A community project — not affiliated with Poke or The Interaction Company.</sub>

[![Latest Release](https://img.shields.io/github/v/release/undivisible/poke-around?style=flat-square)](https://github.com/undivisible/poke-around/releases/latest)
[![License](https://img.shields.io/github/license/undivisible/poke-around?style=flat-square)](https://github.com/undivisible/poke-around/blob/main/LICENSE)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-blue?style=flat-square)

---

Run Poke Around on your machine, then message Poke from iMessage, Telegram, or SMS to run commands, read files, take screenshots, and more — all on your machine.

## Install

**Install script (macOS / Linux)**

```bash
curl -fsSL https://raw.githubusercontent.com/undivisible/poke-around/main/scripts/install.sh | bash
```

**Install script (Windows PowerShell)**

```powershell
irm https://raw.githubusercontent.com/undivisible/poke-around/main/scripts/install.ps1 | iex
```

Installs to `%LOCALAPPDATA%\Programs\poke-around\poke-around.exe` unless `POKE_AROUND_BIN` is set.

**Homebrew (macOS / Linux)**

```bash
brew tap undivisible/tap
brew install poke-around
```

If install fails at **`brew link`** for **`simdjson`** (a dependency of Homebrew **node**), your prefix still has an older simdjson linked. Unlink it, then retry:

```bash
brew unlink simdjson
brew install poke-around
```

If Brew already poured a newer simdjson but could not link it, either run `brew link --overwrite simdjson` and retry, or `brew reinstall simdjson` after `brew unlink simdjson`.

**Build from source**

Requires stable [Rust](https://www.rust-lang.org/tools/install):

```bash
git clone https://github.com/undivisible/poke-around.git
cd poke-around
./scripts/install.sh
```

```powershell
git clone https://github.com/undivisible/poke-around.git
cd poke-around
.\scripts\install.ps1
```

On macOS/Linux this installs to `~/.local/bin/poke-around` unless `POKE_AROUND_BIN` is set. On Windows it installs to `%LOCALAPPDATA%\Programs\poke-around\`.

**Manual download**

Download the latest archive for your platform from [Releases](https://github.com/undivisible/poke-around/releases), extract it, and place `poke-around` or `poke-around.exe` on your `PATH`.

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
| **full** (default) | All tools available; destructive shell commands, writes, deletes, screenshots, UI automation, and other risky operations require approval in chat. |
| **limited** | Read-only tools plus a curated set of safe commands (`ls`, `cat`, `grep`, `curl`, etc.). |
| **sandbox** | Broader command support, with destructive patterns blocked and write/delete/UI mutation tools requiring approval or blocked by policy. |

Set the mode at startup or persist it in `~/.config/poke-around/config.json`:

```bash
poke-around --mode sandbox
poke-around set-mode limited
```

```json
{ "permission_mode": "limited" }
```

The `--mode` flag overrides `config.json` for that daemon run. `poke-around set-mode` updates the config file so the next daemon start (and live reload) picks it up.

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

Agents are JS scripts in `~/.config/poke-around/agents/`. Files may be named `<name>.js` or `<name>.<interval>.js` (the interval suffix is a naming convention only — Poke Around does not run agents on a schedule).

Run an agent manually from the CLI or via the `run_agent` MCP tool:

```bash
poke-around run-agent beeper
```

Schedule agents externally (for example with `cron`, `launchd`, or `systemd` timers) if you want periodic execution.

```javascript
import { Poke, getToken } from "poke";
const poke = new Poke({ apiKey: getToken() });
await poke.sendMessage("Hello from my agent!");
```

Per-agent secrets go in `~/.config/poke-around/agents/.env.<name>`.

## Configuration

Poke Around uses [rs_poke](https://github.com/undivisible/rs_poke) for authentication and tunneling. Credentials are stored at `~/.config/poke/credentials.json`.

| Variable | Description |
|----------|-------------|
| `POKE_API` | Poke API base URL (default: `https://poke.com/api/v1`) |
| `POKE_API_KEY` | API key (optional if logged in via device flow) |
| `POKE_FRONTEND` | Frontend URL for device login (default: `https://poke.com`) |
| `POKE_CLIENT_ID` | OAuth client ID for MCP connections (optional) |
| `POKE_CLIENT_SECRET` | OAuth client secret for MCP connections (optional) |

## Security

In **full mode**, Poke Around grants full shell access to your Poke agent. Only run it on machines and networks you trust. Use `limited` or `sandbox` mode for tighter restrictions.

See [SECURITY.md](SECURITY.md) for the vulnerability disclosure policy.

## Credits

- Rust rewrite of [f/poke-gate](https://github.com/f/poke-gate)
- macOS computer-use primitives through [rs_peekaboo](https://github.com/undivisible/rs_peekaboo)
- Poke tunnel client through [rs_poke](https://github.com/undivisible/rs_poke)
- [Poke](https://poke.com) by [The Interaction Company](https://interaction.co)

## License

MPL-2.0