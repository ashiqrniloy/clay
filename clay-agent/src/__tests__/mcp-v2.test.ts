import { test } from "bun:test";
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

test("per-server fault isolation: one failing server hides only its own tools", async () => {
  // Connection failures no longer reject the connect (decision 1758
  // amendment + plan 117): a crashed binary hides only its own tools.
  const connected = await connectAllowListedMcpServers([
    { serverId: "good", command: process.execPath, args: [FIXTURE] },
    { serverId: "bad", command: "/definitely/not/a/real/binary-9f3c2a" },
  ]);
  try {
    assert.ok(connected.tools.some((t) => t.name.startsWith("mcp:good:")), "healthy server's tools stay bridged");
    assert.ok(!connected.tools.some((t) => t.name.startsWith("mcp:bad:")), "failing server's tools are hidden");
    assert.equal(connected.outcomes.length, 2);
    const good = connected.outcomes.find((o) => o.serverId === "good");
    assert.ok(good?.connected, "healthy server outcome reports connected");
    assert.ok((good?.tools ?? 0) >= 1, "healthy outcome carries its tool count");
    const bad = connected.outcomes.find((o) => o.serverId === "bad");
    assert.equal(bad?.connected, false);
    assert.ok(bad?.error, "failing outcome carries why it is hidden");
  } finally {
    await connected.close();
  }
});

test("timeoutMs rides the connect options: slow tool call times out", async () => {
  const connected = await connectAllowListedMcpServers([
    { serverId: "slow", command: process.execPath, args: [FIXTURE], timeoutMs: 200 },
  ]);
  try {
    const tool = connected.tools.find((t) => t.name === "mcp:slow:sleep");
    assert.ok(tool, "fixture sleep tool bridged");
    // Prism semantics: a timed-out call resolves with an error ToolResult
    // (call errors surface as results, they never throw to the model).
    const result = (await tool.execute({}, ctx)) as { error?: { message?: string } };
    assert.match(result.error?.message ?? "", /timed out after 200ms/);
  } finally {
    await connected.close();
  }
});

test("timeoutMs bounds calls, not the connect handshake: a slow-booting server still connects", async () => {
  // 2026-09-14 review P1-3: Prism 0.5.5 has a single timeout knob
  // (`callTimeoutMs`) — it bounds the initialize handshake and the first
  // tools/list page as well as every call — so a small call-shaped timeoutMs
  // used to hide a server whose process needs ~1s to answer initialize.
  const connected = await connectAllowListedMcpServers([
    {
      serverId: "slowboot",
      command: process.execPath,
      args: [FIXTURE],
      env: { CLAY_MCP_BOOT_DELAY_MS: "1000" },
      timeoutMs: 200,
    },
  ]);
  try {
    const outcome = connected.outcomes[0];
    assert.ok(
      outcome?.connected,
      `slow-booting server must connect (got ${outcome?.error ?? "no outcome"})`,
    );
    const tool = connected.tools.find((t) => t.name === "mcp:slowboot:sleep");
    assert.ok(tool, "slow-booting server's tools must be bridged");
    // ...and the small timeoutMs still bounds the call itself.
    const result = (await tool.execute({}, ctx)) as { error?: { message?: string } };
    assert.match(result.error?.message ?? "", /timed out after 200ms/);
  } finally {
    await connected.close();
  }
});

test("a run cancellation still aborts an in-flight bounded call", async () => {
  // The Clay-owned call deadline must forward the run's own abort, not
  // replace it: a cancelled session must not leave the call running.
  const connected = await connectAllowListedMcpServers([
    { serverId: "cancel", command: process.execPath, args: [FIXTURE], timeoutMs: 200 },
  ]);
  try {
    const tool = connected.tools.find((t) => t.name === "mcp:cancel:sleep");
    assert.ok(tool, "fixture sleep tool bridged");
    const cancelled = new AbortController();
    cancelled.abort(new Error("session cancelled"));
    const result = (await tool.execute({}, { ...ctx, signal: cancelled.signal })) as {
      error?: { message?: string };
    };
    assert.match(result.error?.message ?? "", /session cancelled/);
  } finally {
    await connected.close();
  }
});

test("validation: null cwd/timeoutMs read as absent, not as invalid values", async () => {
  // Plan 117 regression: the Rust allow-list builder used to emit
  // `"cwd": null` / `"timeoutMs": null` for entries without them, which
  // failed the whole list and every session.new. Null now means "default".
  const connected = await connectAllowListedMcpServers([
    { serverId: "nullish", command: process.execPath, args: [FIXTURE], cwd: null, timeoutMs: null },
  ]);
  try {
    assert.ok(
      connected.tools.some((t) => t.name.startsWith("mcp:nullish:")),
      "a null cwd/timeoutMs entry still connects with defaults",
    );
  } finally {
    await connected.close();
  }
});

test("validation: timeoutMs must be a positive integer within the Prism ceiling", async () => {
  await assert.rejects(
    connectAllowListedMcpServers([{ serverId: "t", command: "/bin/true", timeoutMs: 0 }]),
    /positive integer/,
  );
  await assert.rejects(
    connectAllowListedMcpServers([{ serverId: "t", command: "/bin/true", timeoutMs: 1.5 }]),
    /positive integer/,
  );
  await assert.rejects(
    connectAllowListedMcpServers([{ serverId: "t", command: "/bin/true", timeoutMs: 30 * 60 * 1000 + 1 }]),
    /hard ceiling/,
  );
  await assert.rejects(
    connectAllowListedMcpServers([
      { serverId: "t", command: "/bin/true" },
      { serverId: "t", command: "/bin/true" },
    ]),
    /duplicate serverId/,
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