/**
 * poke-bridge.ts — thin Poke SDK bridge for poke-around (Zig daemon).
 *
 * Modes:
 *   poke-bridge tunnel --mcp-url http://localhost:PORT
 *       Starts PokeTunnel, creates webhook, and communicates with the Zig
 *       parent over stdin/stdout using newline-delimited JSON.
 *
 *   poke-bridge send-message --message "..."
 *       Sends a one-shot message to the Poke user and exits.
 */

import { PokeTunnel, isLoggedIn, login, getToken, Poke } from "poke";
import * as readline from "node:readline";

// ── helpers ────────────────────────────────────────────────────────────────

function emit(obj: Record<string, unknown>): void {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

function log(msg: string): void {
  process.stderr.write(`\x1b[2m[bridge] ${msg}\x1b[0m\n`);
}

async function ensureAuth(): Promise<string> {
  if (!isLoggedIn()) {
    emit({ type: "auth_required", message: "Opening browser for Poke login…" });
    await login({ openBrowser: true });
  }
  const token = getToken();
  if (!token) throw new Error("Authentication failed: no token after login.");
  return token;
}

// ── arg parsing ─────────────────────────────────────────────────────────────

const argv = process.argv.slice(2);

function getArg(flag: string): string | null {
  const i = argv.indexOf(flag);
  return i !== -1 && i + 1 < argv.length ? argv[i + 1] : null;
}

const mode = argv[0] ?? "tunnel";

// ── tunnel mode ─────────────────────────────────────────────────────────────

async function runTunnel(): Promise<void> {
  const mcpUrl = getArg("--mcp-url");
  if (!mcpUrl) {
    emit({ type: "error", message: "No --mcp-url provided to bridge." });
    process.exit(1);
  }

  const token = await ensureAuth();
  const poke = new Poke({ token });

  // Create or reuse webhook
  let webhookUrl: string | null = null;
  let webhookToken: string | null = null;
  try {
    const wh = await poke.createWebhook({ condition: "poke-around", action: "poke-around" });
    webhookUrl = wh.webhookUrl;
    webhookToken = wh.webhookToken;
    emit({ type: "webhook_ready", webhookUrl, webhookToken });
  } catch (err) {
    emit({ type: "webhook_error", message: String(err) });
  }

  const tunnel = new PokeTunnel({
    url: mcpUrl,
    name: "poke-around",
    token,
    cleanupOnStop: true,
  });

  tunnel.on("connected", (info) => {
    emit({ type: "connected", connectionId: info.connectionId });
  });
  tunnel.on("disconnected", () => {
    emit({ type: "disconnected" });
  });
  tunnel.on("error", (err) => {
    emit({ type: "error", message: err.message });
  });
  tunnel.on("toolsSynced", ({ toolCount }) => {
    emit({ type: "tools_synced", count: toolCount });
  });

  await tunnel.start();
  log(`Tunnel started → ${mcpUrl}`);

  // Read commands from parent (Zig) on stdin
  const rl = readline.createInterface({ input: process.stdin, terminal: false });

  rl.on("line", async (line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    try {
      const cmd = JSON.parse(trimmed) as Record<string, unknown>;

      if (cmd.type === "send_webhook") {
        if (!webhookUrl || !webhookToken) {
          emit({ type: "webhook_error", message: "No webhook configured." });
          return;
        }
        try {
          await poke.sendWebhook({
            webhookUrl,
            webhookToken,
            data: { message: cmd.message as string },
          });
          emit({ type: "webhook_sent" });
        } catch (err) {
          emit({ type: "webhook_error", message: String(err) });
        }

      } else if (cmd.type === "stop") {
        log("Stop requested.");
        await tunnel.stop();
        process.exit(0);
      }
    } catch {
      // ignore malformed lines
    }
  });

  rl.on("close", () => {
    // parent closed stdin → shut down
    tunnel.stop().finally(() => process.exit(0));
  });
}

// ── send-message mode ────────────────────────────────────────────────────────

async function runSendMessage(): Promise<void> {
  const message = getArg("--message") ?? argv.slice(1).join(" ");
  if (!message) {
    process.stderr.write("Usage: poke-bridge send-message --message \"...\"\n");
    process.exit(1);
  }
  const token = await ensureAuth();
  const poke = new Poke({ token });
  await poke.sendMessage(message);
  process.stdout.write("sent\n");
}

// ── dispatch ─────────────────────────────────────────────────────────────────

if (mode === "send-message") {
  runSendMessage().catch((err) => {
    process.stderr.write(`bridge error: ${err.message}\n`);
    process.exit(1);
  });
} else {
  runTunnel().catch((err) => {
    emit({ type: "error", message: err.message });
    process.exit(1);
  });
}
