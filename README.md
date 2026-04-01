<p align="center">
  <img src="assets/logo.png" width="128" height="128" alt="Poke Around icon">
</p>

<h1 align="center">Poke Around</h1>

<p align="center">
  Let your <a href="https://poke.com">Poke</a> AI assistant access your machine.<br>
  <sub>A community project — not affiliated with Poke or The Interaction Company.</sub>
</p>

<p align="center">
  <a href="https://github.com/undivisible/poke-around/releases/latest"><img src="https://img.shields.io/github/v/release/undivisible/poke-around?style=flat-square" alt="Latest Release"></a>
  <a href="https://github.com/undivisible/poke-around/blob/main/LICENSE"><img src="https://img.shields.io/github/license/undivisible/poke-around?style=flat-square" alt="License"></a>
  <img src="https://img.shields.io/badge/platform-macOS%2015%2B-blue?style=flat-square" alt="Platform">
</p>

---

Run Poke Around on your Mac, then message Poke from iMessage, Telegram, or SMS to run commands, read files, take screenshots, and more — all on your machine.

## Install

**Homebrew** (recommended)

```bash
brew install f/tap/poke-around
```

**Install via Zig**

If you have Zig installed, you can build and install the binary from source:

```bash
git clone https://github.com/undivisible/poke-around.git
cd poke-around
zig build -Doptimize=ReleaseSafe
./zig-out/bin/poke-around
```

**Manual download**

Download the latest binary for your platform (Linux, macOS, Windows) from [Releases](https://github.com/undivisible/poke-around/releases).

Since the app is not notarized on macOS, you may need to run:

```bash
xattr -cr poke-around
```

## CLI usage

Start the gate:

```bash
./poke-around
```

On first run, Poke OAuth opens in your browser. Add `--verbose` to see tool calls in real time:

```bash
./poke-around --verbose
```

Set the access mode with `--mode`:

```bash
./poke-around --mode limited
./poke-around --mode sandbox
```

Config is stored at `~/.config/poke-around/config.json`.


## Agents

<p align="center">
  <img src="assets/screenshots/agents-editor.png" width="600" alt="Agents Editor">
</p>

Agents are scheduled scripts that run automatically in the background. They live in `~/.config/poke-around/agents/` and follow a simple naming convention:

```
<name>.<interval>.js
```

| File | Runs |
|------|------|
| `beeper.1h.js` | Every hour |
| `backup.2h.js` | Every 2 hours |
| `health.10m.js` | Every 10 minutes |
| `cleanup.30m.js` | Every 30 minutes |

Intervals: `Nm` (minutes) or `Nh` (hours). Minimum is 10 minutes.

### Install an agent

Download a community agent from the repository:

```bash
./poke-around agent get beeper
```

This downloads `beeper.1h.js` and `.env.beeper` to `~/.config/poke-around/agents/`. Edit the env file with your credentials and test it:

```bash
nano ~/.config/poke-around/agents/.env.beeper
./poke-around run-agent beeper
```

### Per-agent env files

Each agent can have a `.env.<name>` file for secrets:

```
~/.config/poke-around/agents/.env.beeper
```

```env
BEEPER_TOKEN=your_token_here
```

Variables are injected into the agent process automatically.

### Agent frontmatter

Each agent file starts with a JSDoc-style frontmatter block:

```javascript
/**
 * @agent beeper
 * @name Beeper Message Digest
 * @description Fetches messages from the last hour and sends a summary to Poke.
 * @interval 1h
 * @env BEEPER_TOKEN - Beeper Desktop local API token
 * @author f
 */
```

### Creating your own agent

An agent is just a JS file that runs with Node.js. It has access to:

- `process.env` — variables from `.env.<name>`
- `poke` package — `import { Poke, getToken } from "poke"`
- Any npm package installed globally or via npx

```javascript
/**
 * @agent my-agent
 * @name My Custom Agent
 * @description Does something useful every 30 minutes.
 * @interval 30m
 */

import { Poke, getToken } from "poke";

const poke = new Poke({ apiKey: getToken() });
await poke.sendMessage("Hello from my agent!");
```

Save as `~/.config/poke-around/agents/my-agent.30m.js` and it runs automatically when poke-around is connected.

Agents start running when poke-around connects and run once immediately on startup.

## Access modes

Poke Around supports three access modes that control what your agent can do:

| Mode | Description |
|------|-------------|
| **Full** (default) | All tools available with no approval required. The agent can run commands, write files, and take screenshots directly. |
| **Limited** | Read-only tools plus a curated set of safe commands (`ls`, `cat`, `grep`, `curl`, etc.). `write_file` and `take_screenshot` are disabled. |
| **Sandbox** | Broader command support than Limited, but writes are restricted to `~/Downloads` and `/tmp` via macOS `sandbox-exec`. |

Set the mode via CLI flag, environment variable, or the macOS app Settings:

```bash
./poke-around --mode sandbox
# or
POKE_GATE_PERMISSION_MODE=limited ./poke-around
```

## Security

**In full mode, Poke Around grants full shell access to your Poke agent.** This means:

- Any command can be run with your user's permissions
- Files can be read and written anywhere your user has access
- Risky tools require approval in chat before execution
- Only your Poke agent (authenticated via Poke OAuth) can reach the tunnel

Only run Poke Around on machines and networks you trust. Use `limited` or `sandbox` mode if you want tighter restrictions.

## Credits

- [Poke](https://poke.com) by [The Interaction Company of California](https://interaction.co)
- [Poke SDK](https://www.npmjs.com/package/poke)

## License

MIT
