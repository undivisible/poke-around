/**
 * Minimal client for connecting to unthinkclaw MCP HTTP server.
 * Allows poke-around to proxy computer use tools from unthinkclaw.
 */

export interface McpTool {
  name: string;
  description: string;
  inputSchema: any;
}

export interface McpToolResult {
  content?: Array<{ type: string; text?: string; data?: string; mimeType?: string }>;
  isError?: boolean;
}

export class UnthinkclawClient {
  private baseUrl: string;

  constructor(baseUrl: string = "http://127.0.0.1:5174") {
    this.baseUrl = baseUrl.replace(/\/$/, ""); // Remove trailing slash
  }

  /**
   * List available tools from unthinkclaw MCP server
   */
  async listTools(): Promise<McpTool[]> {
    const response = await fetch(`${this.baseUrl}/mcp`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "tools/list",
      }),
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const data = await response.json();
    if (data.error) {
      throw new Error(`MCP error: ${data.error.message}`);
    }

    return data.result?.tools || [];
  }

  /**
   * Call a tool on unthinkclaw MCP server
   */
  async callTool(name: string, args: Record<string, any>): Promise<McpToolResult> {
    const response = await fetch(`${this.baseUrl}/mcp`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 2,
        method: "tools/call",
        params: {
          name,
          arguments: args,
        },
      }),
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const data = await response.json();
    if (data.error) {
      throw new Error(`MCP error: ${data.error.message}`);
    }

    return data.result || {};
  }

  /**
   * Check if unthinkclaw server is healthy
   */
  async isHealthy(): Promise<boolean> {
    try {
      const response = await fetch(`${this.baseUrl}/health`, { 
        method: "GET",
        signal: AbortSignal.timeout(2000),
      });
      return response.ok;
    } catch {
      return false;
    }
  }
}
