# Beeper Agent

The Beeper agent fetches messages from the last hour via [Beeper Desktop](https://beeper.com)'s local API, groups them by sender, and sends a summary to your Poke agent.

## What it does

Every hour:

1. Calls Beeper's local API at `http://localhost:23373`
2. Searches for messages from the last 60 minutes
3. Filters out messages you sent (only shows incoming)
4. Groups messages by sender name
5. Formats a summary with sender name, message count, and last 3 messages
6. Sends the summary to Poke via `sendMessage`

## Prerequisites

- [Beeper Desktop](https://beeper.com) running on your machine
- Beeper API token (find it in Beeper Desktop > Settings > API)
- Signed in to Poke (`npx poke login`)

## Install

```bash
npx poke-around agent get beeper
```

When prompted, paste your Beeper token:

```
BEEPER_TOKEN (Find it in Beeper Desktop > Settings > API): <paste>
```

## Test

```bash
npx poke-around run-agent beeper
```

Expected output:

```
[agents] Running agent: beeper (beeper.1h.js)
[agents] [beeper] Fetching messages from the last hour...
[agents] [beeper] Found 42 messages
[agents] [beeper] Sending summary to Poke...
[agents] [beeper] Summary sent to Poke.
[agents] [beeper] completed
```

## What Poke receives

Your Poke agent gets a message like:

> Messages from the last hour (3 people):
>
> Alice (5 messages):
>   - Hey, are you free for lunch?
>   - The meeting got moved to 3pm
>   - Can you review my PR?
>
> Bob (2 messages):
>   - Deployed the fix
>   - All tests passing now
>
> Mom (1 messages):
>   - Don't forget dinner tonight!

## Configuration

### Env variables

| Variable | Required | Description |
|----------|----------|-------------|
| `BEEPER_TOKEN` | yes | Beeper Desktop API token |
| `BEEPER_BASE_URL` | no | Override default `http://localhost:23373` |

Edit: `~/.config/poke-around/agents/.env.beeper`

### Change the interval

Rename the file to change how often it runs:

```bash
# Every 30 minutes
mv ~/.config/poke-around/agents/beeper.1h.js ~/.config/poke-around/agents/beeper.30m.js
```

Or use the macOS Agents editor.

## Frontmatter

```javascript
/**
 * @agent beeper
 * @name Beeper Message Digest
 * @description Fetches messages from the last hour via Beeper Desktop and sends a summary to Poke.
 * @interval 1h
 * @env BEEPER_TOKEN - Beeper Desktop local API token (Settings > API)
 * @env BEEPER_BASE_URL - (optional) Override default http://localhost:23373
 * @author f
 */
```

## Source

[View on GitHub](https://github.com/f/poke-around/blob/main/examples/agents/beeper.1h.js)
