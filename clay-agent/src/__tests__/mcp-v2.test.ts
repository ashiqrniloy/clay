import { test } from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { connectAllowListedMcpServers, type ConnectedMcpServers } from "../mcp.js";
import type { ToolExecutionContext } from "@arnilo/prism";

/** Path to the stdio fixture server (resides in src, outside the tsc build). */
const FIXTURE = join(process.cwd(), "src", "__tests__", "mcp-fixture-server.mjs");

const ctx: ToolExecutionContext = {
  sessionId: "test",
  runId: "test",
  toolCallId: "test",
};

function toolResultText(result: unknown): string {
  const value = (result as { value?: { text?: string } })?.value;
  if (typeof value?.text === "string") return value.text;
  const content = (result as { content?: Array<{ type: string; text?: string }> })?.content;
  const text = content?.find((block) => block.type === "text")?.text;
  if (typeof text === "string") return text;
  throw new Error(`unexpected ToolResult shape: ${JSON.stringify(result).slice(0, 200)}`);
}

async function envKeysOf(bridges: ConnectedMcpServers, serverId: string): Promise<string[]> {
  const tool = bridges.tools.find((t) => t.name === `mcp:${serverId}:get_env_keys`);
  assert.ok(tool, `fixture tool mcp:${serverId}:get_env_keys not bridged`);
  const result = await tool.execute({}, ctx);
  return JSON.parse(toolResultText(result)) as string[];
}

test("stdio fixture server connects; tool is prefixed by serverId", async () => {
  const bridges = await connectAllowListedMcpServers([
    { serverId: "fixture", command: process.execPath, args: [FIXTURE] },
  ]);
  try {
    assert.ok(bridges.tools.length >= 1);
    assert.ok(bridges.tools.some((t) => t.name.startsWith("mcp:fixture:")));
  } finally {
    await bridges.close();
  }
});

test("omitted env does not inherit ambient process.env; explicit env is forwarded (SDK v2 allow-list)", async () => {
  process.env.CLAY_MCP_PLANTED_SECRET = "s3cr3t";
  try {
    const bridges = await connectAllowListedMcpServers([
      {
        serverId: "env-holder",
        command: process.execPath,
        args: [FIXTURE],
        env: { CLAY_MCP_EXPLICIT: "yes" },
      },
    ]);
    try {
      const keys = await envKeysOf(bridges, "env-holder");
      assert.ok(keys.includes("CLAY_MCP_EXPLICIT"), "explicit env entry must reach the child");
      assert.ok(
        !keys.includes("CLAY_MCP_PLANTED_SECRET"),
        "ambient process.env must not leak to the child (SDK v2 child-env allow-list)",
      );
    } finally {
      await bridges.close();
    }
  } finally {
    delete process.env.CLAY_MCP_PLANTED_SECRET;
  }
});

test("explicit env forwarding works even when ambient secret is absent", async () => {
  const bridges = await connectAllowListedMcpServers([
    { serverId: "env-clear", command: process.execPath, args: [FIXTURE], env: { CLAY_MCP_EXPLICIT: "1" } },
  ]);
  try {
    const keys = await envKeysOf(bridges, "env-clear");
    assert.ok(keys.includes("CLAY_MCP_EXPLICIT"));
  } finally {
    await bridges.close();
  }
});

test("bad command fails closed: whole connect rejects, nothing partially bridged", async () => {
  await assert.rejects(
    connectAllowListedMcpServers([
      { serverId: "good", command: process.execPath, args: [FIXTURE] },
      { serverId: "bad", command: "/definitely/not/a/real/binary-9f3c2a" },
    ]),
    /error/i,
  );
});

test("daemon source never imports MCP SDK modules directly", async () => {
  const { readFileSync, readdirSync } = await import("node:fs");
  const { join: pjoin } = await import("node:path");
  const walk = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
      e.isDirectory()
        ? e.name === "__tests__"
          ? []
          : walk(pjoin(dir, e.name))
        : e.name.endsWith(".ts")
          ? [pjoin(dir, e.name)]
          : [],
    );
  const sources = walk(pjoin(process.cwd(), "src"));
  for (const file of sources) {
    const text = readFileSync(file, "utf8");
    for (const needle of ["@modelcontextprotocol/client", "@modelcontextprotocol/server", "@modelcontextprotocol/sdk"]) {
      assert.ok(
        !text.includes(needle),
        `${file} must not import ${needle} — MCP travels only through @arnilo/prism-mcp`,
      );
    }
  }
});