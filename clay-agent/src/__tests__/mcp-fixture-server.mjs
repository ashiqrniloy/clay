/**
 * Minimal stdio MCP fixture server for clay-agent MCP bridge tests
 * (Prism 0.5 / MCP SDK v2, 2026-07-28-era stdio). Registers one tool
 * that reports the child process's environment variable names so tests
 * can assert the SDK v2 allow-list env policy: omitted env must not leak
 * ambient `process.env`; explicit env must be forwarded.
 *
 * Spawned by tests via the allow-list: `node <this file>`.
 */
import { McpServer } from "@modelcontextprotocol/server";
import { serveStdio } from "@modelcontextprotocol/server/stdio";

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