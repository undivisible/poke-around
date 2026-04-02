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
import * as os from "node:os";
import { randomUUID } from "node:crypto";

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
const HEARTBEAT_INTERVAL_MS = 30_000;
const RECONNECT_MIN_MS = 5_000;
const RECONNECT_MAX_MS = 60_000;
const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

// ── tunnel mode ─────────────────────────────────────────────────────────────

async function runTunnel(): Promise<void> {
  const mcpUrl = getArg("--mcp-url");
  if (!mcpUrl) {
    emit({ type: "error", message: "No --mcp-url provided to bridge." });
    process.exit(1);
  }

  const token = await ensureAuth();
  const poke = new Poke({ token });

  // Use hostname plus a session suffix to avoid stale-name collisions on reconnects.
  const tunnelName = `poke-around-${os.hostname().toLowerCase().replace(/[^a-z0-9]/g, "-")}-${randomUUID().slice(0, 8)}`;



  let stopRequested = false;
  let activeTunnel: PokeTunnel | null = null;
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  let activeWebhookUrl: string | null = null;
  let activeWebhookToken: string | null = null;
  let retryDelay = RECONNECT_MIN_MS;

  const clearHeartbeat = () => {
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
      heartbeatTimer = null;
    }
  };

  const cleanupTunnel = async () => {
    clearHeartbeat();
    if (activeTunnel) {
      try {
        await activeTunnel.stop();
      } catch {}
      activeTunnel = null;
    }
    activeWebhookUrl = null;
    activeWebhookToken = null;
  };

  const maintainTunnel = async () => {
    while (!stopRequested) {
      let tunnel: PokeTunnel | null = null;
      try {
        const wh = await poke.createWebhook({ condition: tunnelName, action: tunnelName });
        activeWebhookUrl = wh.webhookUrl;
        activeWebhookToken = wh.webhookToken;
        emit({ type: "webhook_ready", webhookUrl: activeWebhookUrl, webhookToken: activeWebhookToken });

        tunnel = new PokeTunnel({
          url: mcpUrl,
          name: tunnelName,
          token,
          cleanupOnStop: true,
        });
        activeTunnel = tunnel;

        const sessionLost = new Promise<void>((_resolve, reject) => {
          tunnel!.on("connected", (info) => {
            emit({ type: "connected", connectionId: info.connectionId });
            retryDelay = RECONNECT_MIN_MS;
            clearHeartbeat();
            heartbeatTimer = setInterval(() => {
              emit({ type: "heartbeat", tunnelName, ts: Date.now() });
            }, HEARTBEAT_INTERVAL_MS);
          });
          tunnel!.on("disconnected", () => {
            reject(new Error("tunnel disconnected"));
          });
          tunnel!.on("error", (err) => {
            reject(err instanceof Error ? err : new Error(String(err)));
          });
          tunnel!.on("toolsSynced", ({ toolCount }) => {
            emit({ type: "tools_synced", count: toolCount });
          });
        });
        // Prevent unhandled rejection if tunnel.start() throws before we reach
        // `await sessionLost` — the rejection is still surfaced via await below.
        sessionLost.catch(() => {});

        await tunnel.start();
        log(`Tunnel started → ${mcpUrl}`);
        await sessionLost;
      } catch (err) {
        emit({ type: "error", message: String(err) });
      } finally {
        await cleanupTunnel();
      }

      if (stopRequested) break;
      await sleep(retryDelay);
      retryDelay = Math.min(retryDelay * 2, RECONNECT_MAX_MS);
    }
  };

  void maintainTunnel();

  // Read commands from parent (Zig) on stdin
  const rl = readline.createInterface({ input: process.stdin, terminal: false });

  rl.on("line", async (line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    try {
      const cmd = JSON.parse(trimmed) as Record<string, unknown>;

      if (cmd.type === "send_webhook") {
        if (!activeWebhookUrl || !activeWebhookToken) {
          emit({ type: "webhook_error", message: "No webhook configured." });
          return;
        }
        try {
          await poke.sendWebhook({
            webhookUrl: activeWebhookUrl,
            webhookToken: activeWebhookToken,
            data: { message: cmd.message as string },
          });
          emit({ type: "webhook_sent" });
        } catch (err) {
          emit({ type: "webhook_error", message: String(err) });
        }

      } else if (cmd.type === "stop") {
        log("Stop requested.");
        stopRequested = true;
        await cleanupTunnel();
        process.exit(0);
      }
    } catch {
      // ignore malformed lines
    }
  });

  rl.on("close", () => {
    // parent closed stdin → shut down
    stopRequested = true;
    cleanupTunnel().finally(() => process.exit(0));
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

// ── global safety net ────────────────────────────────────────────────────────
// The Poke SDK runs an internal async loop whose Promise can reject unhandled
// (e.g. on disconnect), which would crash Bun. Emit the error so the Zig
// parent can reconnect, but keep the process alive so maintainTunnel retries.
process.on("unhandledRejection", (reason) => {
  emit({ type: "error", message: `unhandled rejection: ${String(reason)}` });
});

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
