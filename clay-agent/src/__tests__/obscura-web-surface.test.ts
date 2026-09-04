import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import type { ToolExecutionContext } from "@arnilo/prism";
import { providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";
import { spawnObscuraHarness } from "../obscura.js";

async function tempDir(prefix: string): Promise<string> {
  return mkdtemp(join(tmpdir(), prefix));
}

function textProvider(): Parameters<typeof ClayAgentHost.create>[0]["mockProvider"] {
  return {
    id: "mock",
    async *generate() {
      yield providerTextDelta("ok");
      yield providerDone();
    },
  };
}

const WEB_SURFACE = ["web_search", "web_fetch", "obscura_fetch", "obscura_scrape"];
const BROWSER_SURFACE = ["browser_open", "browser_snapshot", "browser_act", "browser_evaluate", "browser_close"];

function execContext(): ToolExecutionContext {
  return { toolCallId: "call-1", runId: "run-1", sessionId: "s-1" } as ToolExecutionContext;
}

test("engine absent: web and browser tools hidden, sessions unperturbed", async () => {
  const root = await tempDir("clay-agent-web-absent-");
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-web-data-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: textProvider(),
    resolveObscuraBinary: () => undefined,
    emit: () => {},
  });
  await host.handle("agentProfile.register", {
    name: "coding",
    instructions: "Code.",
    tools: ["read", "write", "edit", "shell"],
  });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: root,
  })) as { tools: string[] };
  for (const tool of [...WEB_SURFACE, ...BROWSER_SURFACE]) {
    assert(!created.tools.includes(tool), `no ${tool} when the engine is absent`);
  }
  await rm(root, { recursive: true, force: true });
  host.close();
});

/** Stub `obscura` binary (node): `obscura mcp` speaks a minimal MCP stdio
 *  handshake (empty tool list); `obscura fetch …` answers `--eval` with
 *  search rows, `--dump markdown` with markdown, other dumps with text. */
async function writeObscuraStub(dir: string): Promise<string> {
  const bin = join(dir, "obscura");
  await writeFile(
    bin,
    [
      "#!/usr/bin/env node",
      "const NL = String.fromCharCode(10);",
      'const [cmd] = process.argv.slice(2);',
      'if (cmd === "mcp") {',
      '  const readline = require("node:readline");',
      '  const rl = readline.createInterface({ input: process.stdin });',
      '  rl.on("line", (line) => {',
      '    let msg; try { msg = JSON.parse(line); } catch { return; }',
      '    if (msg.method === "initialize") {',
      '      process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { protocolVersion: msg.params?.protocolVersion ?? "1.0", capabilities: { tools: {} }, serverInfo: { name: "stub-obscura", version: "0.0.0" } } }) + NL);',
      '    } else if (msg.method === "tools/list") {',
      '      process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: { tools: [] } }) + NL);',
      '    } else if (msg.id !== undefined) {',
      '      process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, result: {} }) + NL);',
      '    }',
      '  });',
      '  rl.on("close", () => process.exit(0));',
      '} else if (cmd === "fetch") {',
      '  const rest = process.argv.slice(3).join(" ");',
      '  if (rest.includes("--eval")) {',
      '    process.stdout.write(JSON.stringify([{ url: "https://example.com/a", title: "Fixture A", snippet: "alpha result" }]));',
      '  } else if (rest.includes("--dump markdown")) {',
      '    process.stdout.write(["# Fixture", "", "hello from the stub"].join(NL));',
      '  } else {',
      '    process.stdout.write("fixture text body");',
      '  }',
      '}',
      "",
    ].join("\n"),
  )
  await chmod(bin, 0o755)
  return bin
}

test("engine present: web tools answer through the CLI with untrusted-labeled results", async () => {
  const dir = await tempDir("clay-agent-web-live-");
  const bin = await writeObscuraStub(dir);
  const harness = await spawnObscuraHarness({
    command: bin,
    connectCdp: (async () => ({
      // createBrowserTools only needs a truthy handle for registration;
      // browser tools are not executed in this drill.
      browser: { newContext: async () => ({}) } as never,
      endpoint: "ws://127.0.0.1:9222",
      process: undefined,
      close: async () => {},
    })) as never,
  });
  try {
    const names = harness.tools.map((tool) => tool.name);
    for (const tool of [...WEB_SURFACE, ...BROWSER_SURFACE]) {
      assert(names.includes(tool), `${tool} registered when the engine is present`);
    }
    const byName = new Map(harness.tools.map((tool) => [tool.name, tool]));

    // web_search: replaceable HTML search profile answers with citations.
    const search = await byName.get("web_search")!.execute({ query: "fixture" }, execContext());
    const searchValue = search.value as { results: Array<{ url: string }>; untrusted: boolean };
    assert.equal(searchValue.results[0].url, "https://example.com/a");
    assert.equal(searchValue.untrusted, true, "web results are labeled untrusted");

    // web_fetch: bounded markdown, untrusted.
    const fetched = await byName.get("web_fetch")!.execute({ url: "https://example.com/page" }, execContext());
    assert.equal((fetched.value as { markdown: string }).markdown, "# Fixture\n\nhello from the stub");

    // obscura_fetch: native bounded dump.
    const native = await byName.get("obscura_fetch")!.execute({ url: "https://example.com/page" }, execContext());
    assert.equal((native.value as { content: string }).content, "fixture text body");

    // Public-HTTP(S)-only validation: file:// and other schemes fail closed.
    await assert.rejects(
      async () => byName.get("obscura_fetch")!.execute({ url: "file:///etc/passwd" }, execContext()),
      /http/i,
    );
    await assert.rejects(
      async () => byName.get("web_fetch")!.execute({ url: "ftp://example.com/x" }, execContext()),
      /http/i,
    );

    // Bounded batch: obscura_scrape caps url counts (default limits: 25).
    const urls = Array.from({ length: 26 }, () => "https://example.com/x");
    await assert.rejects(
      async () => byName.get("obscura_scrape")!.execute({ urls }, execContext()),
      /batch exceeds|urls/i,
    );

    // allowEval is deny-by-default: custom expressions rejected.
    await assert.rejects(
      async () =>
        byName.get("obscura_scrape")!.execute({ urls: ["https://example.com/x"], expression: "1+1" }, execContext()),
      /allowEval/i,
    );
  } finally {
    await harness.close();
    await rm(dir, { recursive: true, force: true });
  }
});

test("e2e: CDP composition drives a fixture page with the browser tools", { skip: !process.env.CLAY_WEB_E2E }, async () => {
  // Real composition minus the obscura process: launch Chromium with a CDP
  // endpoint, attach with connectOverCDP (the exact Phase 1 seam target),
  // and drive a local fixture page through the prism browser tools.
  const { chromium } = await import("playwright-core");
  const { createServer } = await import("node:http");
  const cdpPort = 9339;
  const userDataDir = await tempDir("clay-agent-web-chrome-");
  const chrome = spawn(chromium.executablePath(), [
    "--headless=new",
    `--remote-debugging-port=${cdpPort}`,
    `--user-data-dir=${userDataDir}`,
    "--no-first-run",
    "about:blank",
  ]);
  let browser: import("playwright-core").Browser | undefined;
  const server = createServer((request, response) => {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(
      '<html><body><button id="go" onclick="document.getElementById(\'out\').textContent=\'clicked\'">Go</button><div id="out">idle</div></body></html>',
    );
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const fixtureUrl = `http://127.0.0.1:${(server.address() as { port: number }).port}/`;
  try {
    // Bounded readiness probe for the CDP endpoint.
    const deadline = Date.now() + 15_000;
    let wsEndpoint = "";
    while (Date.now() < deadline) {
      try {
        const response = await fetch(`http://127.0.0.1:${cdpPort}/json/version`);
        if (response.ok) {
          wsEndpoint = ((await response.json()) as { webSocketDebuggerUrl: string }).webSocketDebuggerUrl;
          if (wsEndpoint) break;
        }
      } catch {
        // not ready yet
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    assert.ok(wsEndpoint, "chromium CDP endpoint became ready");

    // The Phase 1 seam returns exactly this shape to the harness.
    browser = await chromium.connectOverCDP(wsEndpoint);
    const { createBrowserTools } = await import("@arnilo/prism-web-tools/browser");
    // The fixture page is served on 127.0.0.1 — loopback allowed, contained
    // proxy off for the local drill. Production egress goes through the
    // host contained-proxy attestation instead.
    const browserOptions = {
      browser: browser as unknown as never,
      networkPolicy: { allowLoopback: true, requireContainedProxy: false },
    } as Parameters<typeof createBrowserTools>[0];
    const browserTools = createBrowserTools(browserOptions);
    const asText = (result: { content?: Array<{ type: string; text?: string }>; value?: unknown }): string =>
      result.content?.map((block) => (block.type === "text" ? (block.text ?? "") : "")).join("\n") ??
      String(result.value ?? "");
    const byName = new Map(browserTools.map((tool) => [tool.name, tool]));
    const context = execContext();

    const opened = await byName.get("browser_open")!.execute({ url: fixtureUrl }, context);
    assert.equal(opened.metadata?.trust, "untrusted_external", "page content is untrusted external");

    const snapshot = await byName.get("browser_snapshot")!.execute({}, context);
    assert.match(asText(snapshot as { content?: Array<{ type: string; text?: string }> }), /Go/);

    const clicked = await byName.get("browser_act")!.execute(
      { action: "click", target: { css: "#go" } },
      context,
    );
    assert.ok(!clicked.error, `click succeeds: ${clicked.error?.message ?? ""}`);
    const after = await byName.get("browser_snapshot")!.execute({}, context);
    assert.match(asText(after as { content?: Array<{ type: string; text?: string }> }), /clicked/);
  } finally {
    await browser?.close().catch(() => {});
    chrome.kill("SIGKILL");
    server.close();
    await rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
});
