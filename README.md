# Poke Around

Let your [Poke](https://poke.com) AI assistant access your machine.

<sub>A community project — not affiliated with Poke or The Interaction Company.</sub>

[![Latest Release](https://img.shields.io/github/v/release/undivisible/poke-around?style=flat-square)](https://github.com/undivisible/poke-around/releases/latest)
[![License](https://img.shields.io/github/license/undivisible/poke-around?style=flat-square)](https://github.com/undivisible/poke-around/blob/main/LICENSE)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-blue?style=flat-square)

---

Run Poke Around on your machine, then message Poke from iMessage, Telegram, or SMS to use the explicitly advertised local tools.

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
poke-around --approval-mode per-action
```

Config is stored at `~/.config/poke-around/config.json`.

## Access modes

| Mode | Description |
|------|-------------|
| **full** (default) | All advertised tools are available. The host authorizes permitted actions by launching Poke Around. |
| **limited** | Read-only tools with mutations blocked by policy. |
| **sandbox** | Read-only tools with unsafe mutations blocked by policy. |

Set the mode at startup or persist it in `~/.config/poke-around/config.json`:

```bash
poke-around --mode sandbox
poke-around set-mode limited
```

```json
{ "permission_mode": "limited", "approval_mode": "full" }
```

The `--mode` flag overrides `config.json` for that daemon run. `poke-around set-mode` updates the config file so the next daemon start (and live reload) picks it up.

Approval modes:

| Mode | Description |
|------|-------------|
| **full** (default) | Launching Poke Around authorizes every action permitted by the access mode. |
| **per-action** | Risky actions require confirmation in the local terminal. The MCP caller receives only an opaque request ID and cannot approve its own request. |

```bash
poke-around --approval-mode per-action
poke-around set-approval-mode per-action
```

Per-action mode requires a foreground interactive terminal and fails closed when standard input is not a terminal. The saved approval mode applies after restarting the daemon.

Active tool calls have a 30-second deadline and accept MCP `notifications/cancelled`; access-mode changes cancel tracked calls before taking effect. A TCP disconnect alone is not proof of cancellation or no effect.

In full mode, `observe_ui` returns bounded semantic elements with short-lived tags. `click` and `set_value` accept only a tag from that exact observation plus an `interactive` or `background_only` interaction mode; Poke Around resolves and revalidates the host-owned target before issuing one Praefectus authority grant. Background-only effects use target-addressed accessibility operations and fail closed unless the host executor can guard the shared desktop context; callers cannot select or claim host isolation. A successful accessibility click receipt is explicitly `ExecutedUnverified`, not a claim that the intended UI outcome occurred; ambiguous dispatch is `OutcomeUnknown`. Value setting requires post-effect value-hash verification. The `image` tool captures a non-interactive screen image into a private, size/count/age-bounded content-addressed host cache and returns only its hash locator, byte count, and media type. Caller-selected screenshot paths and screenshot bytes are never accepted or returned. Per-action mode additionally requires local terminal approval for observations, captures, and each effect. `OutcomeUnknown` is always returned with `retry_safe: false`.

Raw coordinates, arbitrary browser debugging, arbitrary image reads, caller-selected screenshot paths, shell commands, general network requests, agent execution, backend capability reports, application opening, and unbounded sleeps fail closed because their current backends cannot meet the required authority, privacy, target-identity, effect, and cancellation guarantees. Screenshot artifacts are observation-only and are never accepted as target authority for an effect.

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

Run an agent manually from the local CLI. Remote agent execution is not advertised:

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

In **full mode**, Poke Around permits every advertised tool. Only run it on machines and networks you trust. Use `limited` or `sandbox` mode for tighter restrictions. Shell execution is not advertised in any mode. The loopback MCP server requires a host-generated bearer capability. The tunnel reaches it through a separate unguessable capability path that injects the bearer locally; neither capability is written to configuration or logs.

See [SECURITY.md](SECURITY.md) for the vulnerability disclosure policy.

## Credits

- Rust rewrite of [f/poke-gate](https://github.com/f/poke-gate)
- macOS computer-use primitives through [rs_peekaboo](https://github.com/undivisible/rs_peekaboo)
- Poke tunnel client through [rs_poke](https://github.com/undivisible/rs_poke)
- [Poke](https://poke.com) by [The Interaction Company](https://interaction.co)

## License

MPL-2.0
