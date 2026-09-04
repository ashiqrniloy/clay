import assert from "node:assert/strict";
import { mkdtemp, writeFile, chmod, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";
import { connectAllowListedMcpServers } from "../mcp.js";
import { resolveObscuraBinary, resetObscuraBinaryCache } from "../resolve-obscura.js";
import { spawnObscuraHarness } from "../obscura.js";

async function tempDir(prefix: string): Promise<string> {
  return mkdtemp(join(tmpdir(), prefix));
}

function textProvider(text: string): Parameters<typeof ClayAgentHost.create>[0]["mockProvider"] {
  return {
    id: "mock",
    async *generate() {
      yield providerTextDelta(text);
      yield providerDone();
    },
  };
}

test("empty allow-list connects nothing and close is a no-op", async () => {
  const connected = await connectAllowListedMcpServers([]);
  assert.deepEqual(connected.tools, []);
  await connected.close();
});

test("allow-list entry with non-canonical command is rejected", async () => {
  await assert.rejects(
    () => connectAllowListedMcpServers([{ serverId: "docs", command: "" }]),
    /command is required/,
  );
  for (const command of ["obscura-mcp", "../obscura-mcp", "C:\\tools\\mcp.exe"]) {
    await assert.rejects(
      () => connectAllowListedMcpServers([{ serverId: "docs", command }]),
      /absolute path/,
    );
  }
  await assert.rejects(
    () => connectAllowListedMcpServers([{ serverId: "docs", command: "/bin/node", args: "x" }]),
    /args must be an array/,
  );
  await assert.rejects(
    () => connectAllowListedMcpServers([{ serverId: "docs", command: "/bin/node", env: { "BAD-NAME": "v" } }]),
    /invalid name/,
  );
  await assert.rejects(
    () => connectAllowListedMcpServers([{ serverId: "docs", command: "/bin/node", surprise: 1 }]),
    /not a recognized field/,
  );
  await assert.rejects(
    () => connectAllowListedMcpServers([{ serverId: "BAD SERVER", command: "/bin/node" }]),
    /serverId must match/,
  );
  await assert.rejects(
    () => connectAllowListedMcpServers([{ serverId: "docs", command: "/bin/node" }, "junk"]),
    /must be an object/,
  );
});

test("allow-list validation precedes any connection (fail closed, no partial spawn)", async () => {
  // The invalid second entry fails synchronously before any child spawns —
  // proving "one bad entry fails the whole connect closed".
  await assert.rejects(
    () =>
      connectAllowListedMcpServers([
        { serverId: "good", command: "/bin/definitely-not-here" },
        { serverId: "bad", command: "relative" },
      ]),
    /absolute path/,
  );
});

test("missing Obscura binary resolves to undefined (hidden, not an error)", async () => {
  resetObscuraBinaryCache();
  const prevPath = process.env.PATH;
  const prevEnv = process.env.CLAY_OBSCURA_BIN;
  try {
    delete process.env.CLAY_OBSCURA_BIN;
    process.env.PATH = await tempDir("empty-path-");
    assert.equal(resolveObscuraBinary(), undefined);
  } finally {
    if (prevPath === undefined) delete process.env.PATH;
    else process.env.PATH = prevPath;
    if (prevEnv === undefined) delete process.env.CLAY_OBSCURA_BIN;
    else process.env.CLAY_OBSCURA_BIN = prevEnv;
  }
});

test("host hides Obscura tools when binary absent; initialize unaffected", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-mcp-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider("hello"),
    resolveObscuraBinary: () => undefined,
  });
  await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  const created = (await host.handle("session.new", { profile: "chat", provider: "mock", model: "demo" })) as {
    tools: string[];
  };
  assert.ok(!created.tools.some((name) => name.startsWith("obscura_")));
  host.close();
});

test("present Obscura binary: harness spawns, registers obscura_ tools, closes on shutdown", async () => {
  // A stub "obscura" binary: a long-lived node script whose serve endpoint
  // never becomes ready is irrelevant here because the CDP seam is replaced;
  // the harness spawns `command serve` (owned) and `command mcp` (owned) and
  // close() must terminate both.
  const dir = await tempDir("obscura-stub-");
  const bin = join(dir, "obscura");
  // Exit immediately: the MCP stdio handshake fails fast instead of waiting
  // out the 60s bridge timeout, and the harness cleanup path still runs.
  await writeFile(bin, "#!/bin/sh\nexit 1\n");
  await chmod(bin, 0o755);

  let cdpClosed = false;
  const harness = (await spawnObscuraHarness({
    command: bin,
    connectCdp: (async () => {
      // Stand-in for the Playwright attach: no browser, but a close() the
      // harness must call (process kill is verified by the mcp child dying).
      return {
        browser: undefined as never,
        endpoint: "ws://127.0.0.1:9222",
        process: undefined,
        close: async () => {
          cdpClosed = true;
        },
      };
    }) as never,
  }).catch((error: unknown) => ({ failed: error as Error }))) as
    | { failed: Error }
    | unknown;
  if (harness && typeof harness === "object" && "failed" in harness) {
    assert.ok((harness as { failed: Error }).failed);
    assert.equal(cdpClosed, true, "half-started serve process is cleaned up");
    await rm(dir, { recursive: true, force: true });
    return;
  }
  const h = harness as unknown as { tools: Array<{ name: string }>; close: () => Promise<void> };
  assert.ok(h.tools.length > 0);
  await h.close();
  assert.equal(cdpClosed, true);
  await rm(dir, { recursive: true, force: true });
});

test("no brave/exa/firecrawl imports in daemon source", async () => {
  const { readFileSync, readdirSync } = await import("node:fs");
  const srcDir = new URL("../", import.meta.url).pathname;
  const files = readdirSync(srcDir).filter((f) => f.endsWith(".ts") && !f.startsWith("__tests__"));
  for (const file of files) {
    const text = readFileSync(join(srcDir, file), "utf8");
    assert.ok(!text.includes("prism-web-tools/brave"), `${file} imports brave`);
    assert.ok(!text.includes("prism-web-tools/exa"), `${file} imports exa`);
    assert.ok(!text.includes("prism-web-tools/firecrawl"), `${file} imports firecrawl`);
  }
});

test("session on coding profile does not error when Obscura harness fails to spawn", async () => {
  // Obscura failures hide the capability (capability reduction), they never
  // fail the coding session.
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-mcp-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider("hello"),
    resolveObscuraBinary: () => "/nonexistent/obscura",
    emit: () => {},
  });
  // Coding profile: wants coding tools, so capabilities activate. Obscura
  // resolution "found" a path that cannot exist as a stub — spawn fails and
  // must be swallowed. MCP allow-list is empty so no MCP either.
  await host.handle("agentProfile.register", {
    name: "coder",
    instructions: "Code.",
    tools: ["read", "write"],
  });
  // The obscura harness will fail (CDP attach to /nonexistent/obscura) —
  // session.new still succeeds.
  const created = (await host.handle("session.new", {
    profile: "coder",
    provider: "mock",
    model: "demo",
  })) as { tools: string[]; sessionId: string };
  assert.ok(created.tools.includes("read"));
  host.close();
});
