# Getting Started

## Install

### Manual download

Download the latest binary for your platform from [GitHub Releases](https://github.com/undivisible/poke-around/releases/latest).

On macOS:

```bash
xattr -cr poke-around
./poke-around
```

### CLI usage

If you have Zig installed, you can build from source:

```bash
zig build -Doptimize=ReleaseSafe
./zig-out/bin/poke-around
```

Poke Around needs **Accessibility** permission on your Mac to automate keyboard/mouse and take screenshots.

### 1. Sign in
Poke Around uses Poke OAuth to authenticate. On first launch:

1. Open Poke Around from your menu bar.
2. The **Setup View** will appear to guide you through:
   - Selecting an access mode (Full, Limited, or Sandbox)
   - Granting the required macOS Accessibility permissions
3. If you're not signed in, a browser window opens for Poke OAuth.
4. After signing in, the connection is established.

You can also sign in manually:

```bash
poke login
```

## Verify it works

Once connected, you'll see a green dot in the menu bar. The popover shows:

> ● Connected to your Poke, your name

Now open iMessage or Telegram and message your Poke:

> "What's my hostname?"

Poke will use the `system_info` tool to answer from your machine.

## What's next?

- [How It Works](/how-it-works) — understand the architecture
- [Tools](/tools) — see all available tools
- [Agents](/agents/) — set up scheduled automation
