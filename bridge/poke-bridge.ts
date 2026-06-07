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
import * as fs from "node:fs";
import * as path from "node:path";

// ── helpers ────────────────────────────────────────────────────────────────

function emit(obj: Record<string, unknown>): void {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

function log(msg: string): void {
  process.stderr.write(`\x1b[2m[bridge] ${msg}\x1b[0m\n`);
}

function formatError(err: unknown): string {
  if (!(err instanceof Error)) return String(err);
  const cause = "cause" in err ? (err as Error & { cause?: unknown }).cause : undefined;
  return cause ? `${err.name}: ${err.message}; cause: ${formatError(cause)}` : `${err.name}: ${err.message}`;
}

function integrationName(base: string): string {
  const suffix = os.hostname().trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return suffix ? `${base}-${suffix}` : base;
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

// ── state.json helpers ───────────────────────────────────────────────────────
// Mirror the Zig daemon's state.json so webhook credentials survive restarts.

function getStatePath(): string {
  const xdg = process.env.XDG_CONFIG_HOME;
  const base = xdg || path.join(os.homedir(), ".config");
  return path.join(base, "poke-around", "state.json");
}

function readState(): Record<string, unknown> {
  try {
    return JSON.parse(fs.readFileSync(getStatePath(), "utf8"));
  } catch {
    return {};
  }
}

function patchState(updates: Record<string, unknown>): void {
  try {
    const merged = { ...readState(), ...updates };
    const p = getStatePath();
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, JSON.stringify(merged, null, 2));
  } catch (err) {
    log(`state write failed: ${formatError(err)}`);
  }
}

async function cleanupStaleConnections(
  token: string,
  webhook: { webhookUrl?: string; webhookToken?: string },
): Promise<void> {
  const state = readState() as {
    connectionId?: string;
    connectionHistory?: string[];
  };
  const ids = new Set<string>();
  if (state.connectionId) ids.add(state.connectionId);
  if (Array.isArray(state.connectionHistory)) {
    for (const id of state.connectionHistory) ids.add(id);
  }
  if (ids.size === 0) return;

  log(`Cleaning up ${ids.size} old connection(s)…`);
  for (const id of ids) {
    await deleteConnection(token, id);
  }

  const { webhookUrl, webhookToken } = webhook;
  patchState({ webhookUrl, webhookToken, connectionId: undefined, connectionHistory: [] });
}

async function deleteConnection(token: string, connectionId: string): Promise<void> {
  const base = process.env.POKE_API ?? "https://poke.com/api/v1";
  try {
    await fetch(`${base}/mcp/connections/${connectionId}`, {
      method: "DELETE",
      headers: { Authorization: `Bearer ${token}` },
    });
  } catch {}
}

// ── arg parsing ─────────────────────────────────────────────────────────────

const argv = process.argv.slice(2);

function getArg(flag: string): string | null {
  const i = argv.indexOf(flag);
  return i !== -1 && i + 1 < argv.length ? argv[i + 1] : null;
}

const mode = argv[0] ?? "tunnel";
const permissionMode = getArg("--mode") ?? "full";
const HEARTBEAT_INTERVAL_MS = 30_000;
const RESTART_AFTER_DISCONNECT_MS = 15_000;
const MAX_CONN_HISTORY = 10;
const PENDING_TUNNEL_ACTIVATION_TIMEOUT_MS = 5_000;

type PendingTunnel = PokeTunnel & {
  connectionId?: string | null;
  activateTunnel?: () => Promise<void>;
};

async function activatePendingTunnel(tunnel: PokeTunnel): Promise<void> {
  const internal = tunnel as PendingTunnel;
  const deadline = Date.now() + PENDING_TUNNEL_ACTIVATION_TIMEOUT_MS;
  while (!internal.connectionId && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  if (!internal.connectionId || typeof internal.activateTunnel !== "function") return;
  try {
    await internal.activateTunnel.call(internal);
  } catch (err) {
    emit({ type: "error", message: `pending tunnel activation failed: ${formatError(err)}` });
  }
}

async function localToolCount(mcpUrl: string): Promise<number> {
  try {
    const response = await fetch(mcpUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });
    if (!response.ok) return 0;
    const body = await response.json() as { result?: { tools?: unknown[] } };
    return Array.isArray(body.result?.tools) ? body.result.tools.length : 0;
  } catch {
    return 0;
  }
}

// ── tunnel mode ─────────────────────────────────────────────────────────────

async function runTunnel(): Promise<void> {
  const mcpUrl = getArg("--mcp-url");
  if (!mcpUrl) {
    emit({ type: "error", message: "No --mcp-url provided to bridge." });
    process.exit(1);
  }

  const token = await ensureAuth();
  const poke = new Poke({ apiKey: token });

  const tunnelName = integrationName("poke-around");

  // ── Webhook: create once, cache forever ─────────────────────────────────
  // poke-gate pattern: read from state.json; only call createWebhook if missing.
  let { webhookUrl, webhookToken, webhookName } = readState() as {
    webhookUrl?: string;
    webhookToken?: string;
    webhookName?: string;
  };

  if (!webhookUrl || !webhookToken || webhookName !== tunnelName) {
    log("Creating webhook (first run)…");
    const wh = await poke.createWebhook({ condition: tunnelName, action: tunnelName });
    webhookUrl = wh.webhookUrl;
    webhookToken = wh.webhookToken;
    patchState({ webhookUrl, webhookToken, webhookName: tunnelName });
  } else {
    log("Reusing cached webhook.");
  }

  emit({ type: "webhook_ready", webhookUrl, webhookToken });
  await cleanupStaleConnections(token, { webhookUrl, webhookToken });

  let stopRequested = false;
  let activeTunnel: PokeTunnel | null = null;
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  let restartTimer: ReturnType<typeof setTimeout> | null = null;
  let startingTunnel = false;

  const clearHeartbeat = () => {
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
      heartbeatTimer = null;
    }
  };

  const clearTunnelRestart = () => {
    if (restartTimer) {
      clearTimeout(restartTimer);
      restartTimer = null;
    }
  };

  const buildAccessModeMessage = (): string => {
    switch (permissionMode) {
      case "limited":
        return "Access mode: Limited. You can read files, list directories, and run safe read-only commands. You cannot write files, take screenshots, or run other commands.";
      case "sandbox":
        return "Access mode: Sandbox. You can read files, list directories, and run approved sandbox commands. Destructive or disallowed actions require approval or are blocked.";
      default:
        return "Access mode: Full. You can run shell commands, read files, list directories, take screenshots, and use computer-control tools. Destructive actions still require approval.";
    }
  };

  const notifyPoke = async (connectionId: string, tunnelUrl?: string) => {
    if (!webhookUrl || !webhookToken) return;
    try {
      await poke.sendWebhook({
        webhookUrl,
        webhookToken,
        data: {
          message:
            `Poke Around is connected to ${tunnelName} (tunnel: ${connectionId}). ` +
            `${tunnelUrl ? `Tunnel URL: ${tunnelUrl}. ` : ""}` +
            `${buildAccessModeMessage()} ` +
            `Use the Poke Around MCP tools whenever I ask you to do something on this machine.`,
          connectionId,
          tunnelUrl,
          mode: permissionMode,
          integration: tunnelName,
        },
      });
      emit({ type: "webhook_sent" });
    } catch (err) {
      emit({ type: "webhook_error", message: formatError(err) });
    }
  };

  const cleanupTunnel = async (deleteRemote: boolean) => {
    const connectionId = activeTunnel?.info?.connectionId;
    clearHeartbeat();
    clearTunnelRestart();
    if (activeTunnel) {
      try { await activeTunnel.stop(); } catch {}
      activeTunnel = null;
    }
    if (deleteRemote && connectionId) {
      await deleteConnection(token, connectionId);
    }
  };

  // Handle SIGTERM/SIGINT from the Zig parent (or user) by cleaning up the
  // active PokeTunnel before exiting, so the integration is deregistered from
  // Poke's backend and doesn't accumulate stale instances across restarts.
  const sigHandler = () => {
    stopRequested = true;
    cleanupTunnel(true).finally(() => process.exit(0));
  };
  process.once("SIGTERM", sigHandler);
  process.once("SIGINT", sigHandler);

  const recordConnection = (connectionId: string) => {
    const state = readState() as { connectionHistory?: string[] };
    const history: string[] = state.connectionHistory ?? [];
    if (!history.includes(connectionId)) history.unshift(connectionId);
    patchState({
      connectionId,
      connectionHistory: history.slice(0, MAX_CONN_HISTORY),
    });
  };

  const scheduleTunnelRestart = () => {
    if (stopRequested || restartTimer) return;
    restartTimer = setTimeout(async () => {
      restartTimer = null;
      if (stopRequested) return;
      await cleanupTunnel(false);
      await startFreshTunnel();
    }, RESTART_AFTER_DISCONNECT_MS);
  };

  const startFreshTunnel = async () => {
    if (stopRequested || startingTunnel) return;
    startingTunnel = true;
    clearTunnelRestart();
    try {
      const tunnel = new PokeTunnel({
        url: mcpUrl,
        name: tunnelName,
        token,
        cleanupOnStop: false,
      });
      activeTunnel = tunnel;

      tunnel.on("connected", (info) => {
        emit({ type: "connected", connectionId: info.connectionId, tunnelUrl: info.tunnelUrl });
        recordConnection(info.connectionId);
        clearTunnelRestart();
        clearHeartbeat();
        heartbeatTimer = setInterval(() => {
          emit({ type: "heartbeat", tunnelName, ts: Date.now() });
        }, HEARTBEAT_INTERVAL_MS);
        void notifyPoke(info.connectionId, info.tunnelUrl);
      });
      tunnel.on("disconnected", () => {
        emit({ type: "disconnected" });
        clearHeartbeat();
        scheduleTunnelRestart();
      });
      tunnel.on("error", (err) => {
        emit({ type: "error", message: formatError(err) });
        scheduleTunnelRestart();
      });
      tunnel.on("oauthRequired", async () => {
        emit({ type: "auth_required", message: "Poke token expired — re-authenticating…" });
        try {
          await login({ openBrowser: true });
        } catch (authErr) {
          emit({ type: "error", message: `Re-auth failed: ${formatError(authErr)}` });
        }
        scheduleTunnelRestart();
      });
      tunnel.on("toolsSynced", ({ toolCount }) => {
        void (async () => {
          emit({ type: "tools_synced", count: Math.max(toolCount, await localToolCount(mcpUrl)) });
        })();
      });

      const startPromise = tunnel.start();
      void activatePendingTunnel(tunnel);
      await startPromise;
      await (tunnel as unknown as { syncTools(): Promise<void> }).syncTools();
      log(`Tunnel started → ${mcpUrl}`);
    } catch (err) {
      emit({ type: "error", message: formatError(err) });
      activeTunnel = null;
      scheduleTunnelRestart();
    } finally {
      startingTunnel = false;
    }
  };

  void startFreshTunnel();

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
          emit({ type: "webhook_error", message: formatError(err) });
        }

      } else if (cmd.type === "stop") {
        log("Stop requested.");
        stopRequested = true;
        await cleanupTunnel(true);
        process.exit(0);
      }
    } catch {
      // ignore malformed lines
    }
  });

  rl.on("close", () => {
    // parent closed stdin → shut down
    stopRequested = true;
    cleanupTunnel(true).finally(() => process.exit(0));
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
  const poke = new Poke({ apiKey: token });
  await poke.sendMessage(message);
  process.stdout.write("sent\n");
}

// ── global safety net ────────────────────────────────────────────────────────
// The Poke SDK runs an internal async loop whose Promise can reject unhandled
// (e.g. on disconnect), which would crash Bun. Emit the error so the Zig
// parent can reconnect, but keep the process alive so maintainTunnel retries.
process.on("unhandledRejection", (reason) => {
  emit({ type: "error", message: `unhandled rejection: ${formatError(reason)}` });
});

// ── dispatch ─────────────────────────────────────────────────────────────────

if (mode === "send-message") {
  runSendMessage().catch((err) => {
    process.stderr.write(`bridge error: ${formatError(err)}\n`);
    process.exit(1);
  });
} else {
  runTunnel().catch((err) => {
    emit({ type: "error", message: formatError(err) });
    process.exit(1);
  });
}
