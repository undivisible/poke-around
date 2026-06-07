import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("./poke-bridge.ts", import.meta.url), "utf8");

describe("bridge tunnel lifecycle", () => {
  test("does not tear down the bridge process on transient tunnel disconnects", () => {
    expect(source).not.toContain('tunnel.on("disconnected", () => {\n            reject');
    expect(source).toContain("scheduleTunnelRestart");
  });

  test("does not delete remote connection on transient tunnel restart", () => {
    expect(source).toContain("cleanupOnStop: false");
    expect(source).toContain("await cleanupTunnel(false)");
    expect(source).toContain("await cleanupTunnel(true)");
  });

  test("cleans stale connections without dropping webhook credentials", () => {
    expect(source).toContain("async function cleanupStaleConnections");
    expect(source).toContain("webhookUrl, webhookToken");
    expect(source).toContain('/mcp/connections/${connectionId}');
  });

  test("recreates cached webhook when tunnel name changes", () => {
    expect(source).toContain('integrationName("poke-around")');
    expect(source).toContain("webhookName !== tunnelName");
    expect(source).toContain("webhookName: tunnelName");
  });

  test("notifies Poke after a fresh tunnel connection", () => {
    expect(source).toContain("const notifyPoke = async");
    expect(source).toContain("void notifyPoke(info.connectionId, info.tunnelUrl)");
    expect(source).toContain("Use the Poke Around MCP tools");
  });

  test("emits tunnel url with connection events", () => {
    expect(source).toContain('tunnelUrl: info.tunnelUrl');
  });

  test("syncs tools immediately after tunnel start", () => {
    expect(source).toContain('syncTools(): Promise<void>');
    expect(source).toContain(".syncTools();");
  });

  test("activates pending tunnel before first upstream connection", () => {
    expect(source).toContain("activatePendingTunnel(tunnel)");
    expect(source).toContain("internal.activateTunnel.call(internal)");
  });

  test("reports local tool count when sdk sync count is wrong", () => {
    expect(source).toContain("async function localToolCount");
    expect(source).toContain("Math.max(toolCount, await localToolCount(mcpUrl))");
  });

  test("includes nested causes in bridge error output", () => {
    expect(source).toContain("function formatError");
    expect(source).toContain("cause:");
  });
});
