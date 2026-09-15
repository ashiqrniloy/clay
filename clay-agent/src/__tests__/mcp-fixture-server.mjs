/**
 * Minimal stdio MCP fixture server for clay-agent MCP bridge tests
 * (Prism 0.5 / MCP SDK v2, 2026-07-28-era stdio). Registers one tool
 * that reports the child process's environment variable names so tests
 * can assert the SDK v2 allow-list env policy: omitted env must not leak
 * ambient `process.env`; explicit env must be forwarded.
 *
 * Spawned by tests via the allow-list: `node <this file>`. Set
 * `CLAY_MCP_BOOT_DELAY_MS` to make the process answer its first request only
 * after that delay (slow-boot regression fixture for the connect floor).
 */
import { McpServer } from "@modelcontextprotocol/server";
import { serveStdio } from "@modelcontextprotocol/server/stdio";

// Before the transport is attached, so the client's initialize request waits:
// that is the boot window a call-shaped timeout must not swallow.
const bootDelayMs = Number(process.env.CLAY_MCP_BOOT_DELAY_MS ?? 0);
if (Number.isFinite(bootDelayMs) && bootDelayMs > 0) {
  await new Promise((resolve) => setTimeout(resolve, bootDelayMs));
}

serveStdio(() => {
  const server = new McpServer(
    { name: "clay-mcp-fixture", version: "1.0.0" },
    { capabilities: { tools: {} } },
  );

  server.registerTool(
    "get_env_keys",
    {
      title: "Get environment variable names",
      description: "Return the names of environment variables visible to this process.",
      inputSchema: undefined,
    },
    async () => ({
      content: [{ type: "text", text: JSON.stringify(Object.keys(process.env).sort()) }],
    }),
  );

  server.registerTool(
    "sleep",
    {
      title: "Sleep",
      description: "Sleep for one second (timeout testing).",
      inputSchema: undefined,
    },
    async () => {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      return { content: [{ type: "text", text: "slept" }] };
    },
  );

  return server;
});