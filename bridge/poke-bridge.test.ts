import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("./poke-bridge.ts", import.meta.url), "utf8");

describe("bridge tunnel lifecycle", () => {
  test("does not tear down the bridge process on transient tunnel disconnects", () => {
    expect(source).not.toContain('tunnel.on("disconnected", () => {\n            reject');
    expect(source).toContain("scheduleTunnelRestart");
  });

  test("cleans stale connections without dropping webhook credentials", () => {
    expect(source).toContain("async function cleanupStaleConnections");
    expect(source).toContain("webhookUrl, webhookToken");
    expect(source).toContain('/mcp/connections/${id}');
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
});
