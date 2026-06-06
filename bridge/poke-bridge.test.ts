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
});
