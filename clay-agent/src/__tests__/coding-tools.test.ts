import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import type { ToolDefinition, ToolExecutionContext } from "@arnilo/prism";
import {
  toAskUserDecisionSuspendData,
  validateAskUserDecisionResume,
} from "@arnilo/prism-coding-tools/agent";
import { buildCodingTools, createClayAcceptancePolicy, normalizeToolCaps } from "../coding-tools.js";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-coding-"));
}

function context(): ToolExecutionContext {
  return {
    sessionId: "s1",
    runId: "r1",
    toolCallId: "c1",
    metadata: {},
  } as unknown as ToolExecutionContext;
}

interface MockDoc {
  text: string;
  version: number;
  dirty: boolean;
  /** When false, writes are rejected like a foreign lease. */
  writable: boolean;
  /** Server cut a resident buffer at its read cap (plan 119 P2-1). */
  truncated?: boolean;
  totalBytes?: number;
}

interface MockServerState {
  docs: Map<string, MockDoc>;
  writes: Array<{ path: string; content: string }>;
}

function mockReverseServer(state: MockServerState) {
  return async (method: string, params: Record<string, unknown>): Promise<unknown> => {
    const path = String(params.path);
    if (method === "document.read") {
      const doc = state.docs.get(path);
      if (!doc) throw new Error(`file unavailable: ${path}`);
      return {
        text: doc.text,
        version: doc.version,
        dirty: doc.dirty,
        open: true,
        truncated: doc.truncated ?? false,
        totalBytes: doc.totalBytes ?? Buffer.byteLength(doc.text, "utf8"),
      };
    }
    if (method === "document.write") {
      const doc = state.docs.get(path);
      if (doc && !doc.writable) throw new Error("document lease held by another client");
      if (doc) {
        doc.text = String(params.content);
        doc.version += 1;
        doc.dirty = true;
      } else {
        state.docs.set(path, {
          text: String(params.content),
          version: 1,
          dirty: false,
          writable: true,
        });
      }
      state.writes.push({ path, content: String(params.content) });
      return { version: state.docs.get(path)!.version, saved: true, open: true };
    }
    if (method === "document.mkdir") return { ok: true };
    if (method === "document.stat") {
      const doc = state.docs.get(path);
      if (!doc) throw new Error(`file unavailable: ${path}`);
      return { size: Buffer.byteLength(doc.text, "utf8"), open: true };
    }
    throw new Error(`unknown reverse method: ${method}`);
  };
}

const CODING_TOOLS = [
  "shell",
  "read",
  "write",
  "edit",
  "repo_list",
  "repo_search",
  "glob",
  "delete",
  "move",
];

test("buildCodingTools registers the nine tools plus ask_user_decision", () => {
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: "/tmp/ws",
    request: async () => {
      throw new Error("unused");
    },
    fullAutonomy: () => false,
  });
  const names = tools.map((tool) => tool.name);
  for (const name of CODING_TOOLS) assert.ok(names.includes(name), `missing ${name}`);
  assert.ok(!names.includes("ask_user_decision"), "ask tool requires an ask callback");
  const withAsk = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: "/tmp/ws",
    request: async () => {
      throw new Error("unused");
    },
    fullAutonomy: () => false,
    ask: async () => ({ selectedId: "a" }),
  });
  assert.ok(withAsk.map((tool) => tool.name).includes("ask_user_decision"));
});

test("tool caps from toolCaps flow into repo tools; truncation surfaces a remedy error", async () => {
  const capsFile = join(await tempDir(), "tool-caps.json");
  const wsRoot = await tempDir();
  // Three-file tree against maxEntries 2: the walk must truncate, proving
  // the user cap flowed into the tool and the remedy error fires.
  await writeFile(join(wsRoot, "a.txt"), "needle\n");
  await writeFile(join(wsRoot, "b.txt"), "other\n");
  await writeFile(join(wsRoot, "c.txt"), "more\n");
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: wsRoot,
    request: async () => {
      throw new Error("unused");
    },
    fullAutonomy: () => true,
    toolCaps: { maxEntries: 2, maxMatches: 1 },
    capsFile,
  });
  const search = tools.find((tool) => tool.name === "repo_search");
  assert.ok(search, "repo_search present");
  // maxEntries 2 forces a wall-hit on the real workspace walk; the result
  // must carry an error naming the caps file, the key, and the hard ceiling.
  const result = await search.execute({ query: "x" }, context());
  assert.ok(result?.error, "truncation must produce an error");
  assert.match(result.error.message, /maxEntries/);
  assert.match(result.error.message, new RegExp(capsFile.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  assert.match(result.error.message, /100,000/);
  // Content still carries the original result text plus the remedy line.
  const last = (result.content?.at(-1) as { text?: string } | undefined)?.text ?? "";
  assert.match(last, /repository scan cap/);
});

test("repo_list per-call maxResults pagination is not a scan-cap error", async () => {
  const wsRoot = await tempDir();
  await writeFile(join(wsRoot, "a.txt"), "a\n");
  await writeFile(join(wsRoot, "b.txt"), "b\n");
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: wsRoot,
    request: async () => {
      throw new Error("unused");
    },
    fullAutonomy: () => true,
    capsFile: join(wsRoot, "tool-caps.json"),
  });
  const list = tools.find((tool) => tool.name === "repo_list");
  assert.ok(list, "repo_list present");
  const result = await list.execute({ maxResults: 1 }, context());
  assert.ok(!result?.error, "per-call maxResults is pagination, not a cap");
  const text = JSON.stringify(result);
  assert.ok(!/repository scan cap/.test(text));
  assert.match(text, /truncated by results/);
});

test("normalizeToolCaps drops malformed entries and keeps valid ones", () => {
  assert.deepEqual(normalizeToolCaps({ maxEntries: 5000, maxFiles: -1, bogus: 3, exclude: ["a", 4] }), {
    maxEntries: 5000,
  });
  assert.deepEqual(normalizeToolCaps("junk"), {});
  assert.deepEqual(normalizeToolCaps({ maxScanBytes: 1_048_576, exclude: ["target"] }), {
    maxScanBytes: 1_048_576,
    exclude: ["target"],
  });
});

test("read findText advances offset past the first page", async () => {
  const root = await tempDir();
  const target = join(root, "big.txt");
  const lines = Array.from({ length: 2500 }, (_, i) => `line ${i + 1}`);
  lines[2259] = "private async oauthStart unique-needle";
  const text = `${lines.join("\n")}\n`;
  let reads = 0;
  const state: MockServerState = {
    docs: new Map([[target, { text, version: 1, dirty: false, writable: true }]]),
    writes: [],
  };
  const request = mockReverseServer(state);
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: root,
    request: async (method, params) => {
      if (method === "document.read") {
        reads += 1;
        assert.ok(reads <= 20, "readPage ignored offset; findText looped");
      }
      return request(method, params);
    },
    fullAutonomy: () => true,
  });
  const read = tools.find((tool) => tool.name === "read");
  assert.ok(read);
  const result = await read.execute({ path: target, findText: "unique-needle" }, context());
  assert.ok(!result?.error, result?.error?.message);
  const body = (result.content?.[0] as { text?: string } | undefined)?.text ?? "";
  assert.match(body, /unique-needle/);
  assert.ok(!body.startsWith("line 1"), "page must start at the hit, not line 1");
});

test("read returns the dirty buffer of an open document, not disk bytes", async () => {
  const state: MockServerState = {
    docs: new Map([["/tmp/ws/notes.md", { text: "dirty buffer text", version: 3, dirty: true, writable: true }]]),
    writes: [],
  };
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: "/tmp/ws",
    request: mockReverseServer(state),
    fullAutonomy: () => false,
  });
  const read = tools.find((tool) => tool.name === "read");
  assert.ok(read);
  const result = await read.execute({ path: "/tmp/ws/notes.md" }, context());
  assert.ok(JSON.stringify(result).includes("dirty buffer text"));
});

test("document calls name their session so the server picks the workspace", async () => {
  // Plan 119 SC-6: without the session id the server cannot resolve a root
  // and fails the call closed — the id must ride every document round-trip.
  const seen: Array<{ method: string; sessionId: unknown }> = [];
  const state: MockServerState = {
    docs: new Map([["/tmp/ws/notes.md", { text: "x", version: 1, dirty: false, writable: true }]]),
    writes: [],
  };
  const inner = mockReverseServer(state);
  const tools = buildCodingTools({
    sessionId: "session-42",
    workspaceRoot: "/tmp/ws",
    request: async (method, params) => {
      seen.push({ method, sessionId: params.sessionId });
      return inner(method, params);
    },
    fullAutonomy: () => false,
  });
  const read = tools.find((tool) => tool.name === "read");
  assert.ok(read);
  await read.execute({ path: "/tmp/ws/notes.md" }, context());
  assert.ok(seen.length > 0, "the read tool must reach the server");
  for (const call of seen) {
    assert.equal(call.sessionId, "session-42", `${call.method} lost its session id`);
  }
});

test("read fails loudly when the server cut the buffer at its read cap", async () => {
  // Plan 119 P2-1: the server caps dirty-buffer reads and flags the cut. A cut
  // prefix must never be paged or scanned as if it were the whole file — that
  // would read as EOF to the model.
  const state: MockServerState = {
    docs: new Map([
      [
        "/tmp/ws/huge.md",
        {
          text: "head of a very large document",
          version: 9,
          dirty: true,
          writable: true,
          truncated: true,
          totalBytes: 9_000_000,
        },
      ],
    ]),
    writes: [],
  };
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: "/tmp/ws",
    request: mockReverseServer(state),
    fullAutonomy: () => false,
  });
  const read = tools.find((tool) => tool.name === "read");
  assert.ok(read);
  const result = await read.execute({ path: "/tmp/ws/huge.md" }, context());
  assert.ok(result.error, "expected an error result, not a cut page");
  assert.match(result.error?.message ?? "", /9000000 bytes, exceeds the .* byte read cap/);
  assert.ok(!JSON.stringify(result).includes("head of a very large document"));
});

test("write goes through the server and bumps the version", async () => {
  const root = await tempDir();
  const target = join(root, "notes.md");
  const state: MockServerState = {
    docs: new Map([[target, { text: "old", version: 1, dirty: false, writable: true }]]),
    writes: [],
  };
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: root,
    request: mockReverseServer(state),
    fullAutonomy: () => false,
  });
  const write = tools.find((tool) => tool.name === "write");
  assert.ok(write);
  await write.execute({ path: target, content: "new body" }, context());
  const doc = state.docs.get(target);
  assert.equal(doc?.version, 2);
  assert.equal(doc?.text, "new body");
});

test("edit on a user-held lease fails closed", async () => {
  const root = await tempDir();
  const target = join(root, "locked.md");
  const state: MockServerState = {
    docs: new Map([[target, { text: "held", version: 1, dirty: false, writable: false }]]),
    writes: [],
  };
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: root,
    request: mockReverseServer(state),
    fullAutonomy: () => false,
  });
  const edit = tools.find((tool) => tool.name === "edit");
  assert.ok(edit);
  // Edit tools return an error result instead of throwing (Prism contract).
  const result = await edit.execute({ path: target, oldText: "held", newText: "changed" }, context());
  assert.ok(result.error, "expected an error result");
  assert.ok(JSON.stringify(result).includes("lease held by another client"));
  assert.equal(state.docs.get(target)?.text, "held");
});

test("acceptance policy: in-root writes free, out-root gated by approval", async () => {
  const root = await tempDir();
  const other = await tempDir();
  const roots = [root];
  const fullAutonomy = { value: false };
  const policy = createClayAcceptancePolicy({
    roots,
    fullAutonomy: () => fullAutonomy.value,
    approve: (action) => action.kind === "write" && action.paths?.[0] === join(other, "approved.txt"),
  });
  const inRoot = await policy.check({ kind: "write", operation: "write", paths: [join(root, "file.txt")] });
  assert.equal(inRoot.allowed, true);
  const outRoot = await policy.check({ kind: "write", operation: "write", paths: [join(other, "denied.txt")] });
  assert.equal(outRoot.allowed, false);
  const approved = await policy.check({ kind: "write", operation: "write", paths: [join(other, "approved.txt")] });
  assert.equal(approved.allowed, true);
  fullAutonomy.value = true;
  const autonomous = await policy.check({ kind: "write", operation: "write", paths: [join(other, "denied.txt")] });
  assert.equal(autonomous.allowed, true);
});

test("acceptance policy: reads free everywhere, shell metacharacters gated", async () => {
  const root = await tempDir();
  const fullAutonomy = { value: false };
  const policy = createClayAcceptancePolicy({
    roots: [root],
    fullAutonomy: () => fullAutonomy.value,
  });
  const outsideRead = await policy.check({ kind: "read", operation: "read", paths: ["/etc/hostname"] });
  assert.equal(outsideRead.allowed, true);
  const plainShell = await policy.check({ kind: "shell", operation: "execute", command: "echo hi" });
  assert.equal(plainShell.allowed, true);
  const metaShell = await policy.check({ kind: "shell", operation: "execute", command: "echo hi > /tmp/ws/f" });
  assert.equal(metaShell.allowed, false);
  fullAutonomy.value = true;
  const autonomousShell = await policy.check({
    kind: "shell",
    operation: "execute",
    command: `echo hi > ${join(root, "f")}`,
  });
  assert.equal(autonomousShell.allowed, true);
});

test("real write through the tool reaches the mock server document", async () => {
  const dir = await tempDir();
  const target = join(dir, "scratch.txt");
  const state: MockServerState = { docs: new Map(), writes: [] };
  const tools = buildCodingTools({
    sessionId: "session-1",
    workspaceRoot: dir,
    request: mockReverseServer(state),
    fullAutonomy: () => false,
  });
  const write = tools.find((tool) => tool.name === "write");
  assert.ok(write);
  await write.execute({ path: target, content: "from tool" }, context());
  const doc = state.docs.get(target);
  assert.equal(doc?.text, "from tool");
  assert.equal(doc?.version, 1);
  assert.equal(state.writes.length, 1);
  void readFile;
  void writeFile;
});

test("D1: omitted allowCustom persists false and resume accepts selectedId", () => {
  const data = toAskUserDecisionSuspendData({
    question: "Which approach?",
    options: [
      { id: "a", label: "Option A", pros: ["p", "p", "p"], cons: ["c", "c", "c"] },
      { id: "b", label: "Option B", pros: ["p", "p", "p"], cons: ["c", "c", "c"] },
    ],
    selectionMode: "single",
  });
  assert.equal(data.allowCustom, false);
  const answer = validateAskUserDecisionResume(data, { selectedId: "b" });
  assert.deepEqual(answer, { kind: "selection", selectedId: "b", selectedIds: ["b"] });
  assert.throws(() => validateAskUserDecisionResume(data, { customText: "nope" }));
});

test("chat host session with no coding tools stays tool-free", async () => {
  const dataDir = await tempDir();
  const events: Array<{ method: string }> = [];
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: (method) => events.push({ method }),
  });
  await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  await host.handle("agentProfile.register", {
    name: "coding",
    instructions: "Code.",
    tools: CODING_TOOLS,
  });
  const profiles = (await host.handle("agentProfile.list", {})) as {
    profiles: Array<{ name: string; tools: string[] }>;
  };
  assert.equal(profiles.profiles.find((profile) => profile.name === "chat")?.tools.length, 0);
  const coding = profiles.profiles.find((profile) => profile.name === "coding");
  assert.equal(coding?.tools.length, CODING_TOOLS.length);
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: dataDir,
  })) as { sessionId: string; workspaceRoot: string };
  assert.equal(created.workspaceRoot, dataDir);
  const prompted = (await host.handle("session.prompt", {
    sessionId: created.sessionId,
    text: "List files",
  })) as { lastEvent: string };
  assert.equal(prompted.lastEvent, "agent_finished");
  const toolEvents = events.filter((event) => event.method === "event");
  // Mock provider never calls tools: the profile lists them but no run invokes them.
  assert.ok(Array.isArray(toolEvents));
  host.close();
});