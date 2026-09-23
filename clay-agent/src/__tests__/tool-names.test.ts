// Plan 121: the daemon's per-run tool allow-list (Prism 0.7
// `RunOptions.toolNames`).
//
// `session.prompt.toolNames` is shape-validated at this boundary (-32602 for
// non-array / non-string / empty entries) and forwarded to
// `session.stream(..., { toolNames })` when present. Omitted = full registry,
// `[]` = no tools, a list = that subset. Unknown names fail closed inside
// Prism's `selectRunTools` during run assembly — before any provider turn —
// so the provider-visible tool schemas are the behavioral evidence used here.
import { test } from "bun:test";
import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createMockProvider, providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-tool-names-"));
}

interface CapturedRequest {
  tools?: ReadonlyArray<{ name?: string }>;
}

async function codingHostWithCapturingProvider(events: CapturedRequest[]) {
  const provider = createMockProvider([providerTextDelta("Hello"), providerDone()], {
    onRequest: (request) => {
      events.push(request as CapturedRequest);
    },
  });
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: () => {},
    // Hermetic agent config: no ambient MCP/capability tools or skills
    // change the registry these assertions read.
    agentConfigRoot: await tempDir(),
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
    workspaceRoot: await tempDir(),
  })) as { sessionId: string };
  return { host, sessionId: created.sessionId };
}

function toolNamesOf(request: CapturedRequest | undefined): string[] {
  return (request?.tools ?? []).map((tool) => tool.name ?? "");
}

test("omitted toolNames leaves the full registry (0.6 behavior)", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await codingHostWithCapturingProvider(events);
  try {
    await host.handle("session.prompt", { sessionId, text: "Hi" });
    const names = toolNamesOf(events.at(-1));
    for (const expected of ["read", "write", "edit", "shell"]) {
      assert.ok(names.includes(expected), `omitted grant must keep ${expected}: ${names.join(",")}`);
    }
  } finally {
    host.close();
  }
});

test("empty toolNames is a no-tools grant, not 'all tools'", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await codingHostWithCapturingProvider(events);
  try {
    await host.handle("session.prompt", { sessionId, text: "Hi", toolNames: [] });
    assert.deepEqual(toolNamesOf(events.at(-1)), []);
  } finally {
    host.close();
  }
});

test("a known subset reaches the provider as exactly that subset", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await codingHostWithCapturingProvider(events);
  try {
    await host.handle("session.prompt", { sessionId, text: "Hi", toolNames: ["read"] });
    assert.deepEqual(toolNamesOf(events.at(-1)), ["read"]);
  } finally {
    host.close();
  }
});

test("unknown names fail closed before any provider turn", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await codingHostWithCapturingProvider(events);
  try {
    await assert.rejects(
      () => host.handle("session.prompt", { sessionId, text: "Hi", toolNames: ["read", "nope"] }),
      /Unknown run tool: nope/,
    );
    assert.equal(events.length, 0, "no provider request may fire for an unknown grant");
  } finally {
    host.close();
  }
});

test("non-array and malformed entries fail closed as -32602", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await codingHostWithCapturingProvider(events);
  try {
    for (const bad of ["read", 42, { names: ["read"] }, ["read", 42], ["read", ""]]) {
      await assert.rejects(
        () => host.handle("session.prompt", { sessionId, text: "Hi", toolNames: bad }),
        (error: unknown) => {
          assert.match(
            String((error as Error).message),
            /toolNames must be an array of non-empty strings/,
          );
          assert.equal((error as { rpcCode?: number }).rpcCode, -32602);
          return true;
        },
      );
    }
    assert.equal(events.length, 0, "no provider request may fire for malformed grants");
  } finally {
    host.close();
  }
});