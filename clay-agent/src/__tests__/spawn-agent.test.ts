// Plan 122: host-owned spawn/wait/cancel over a Prism supervisor on coding
// profiles (catalog exactly `test` + `validation`).
//
// Covered:
// - coding sessions list spawn_agent/wait_agent/cancel_agent; Chat does not
// - sync spawn runs an isolated child (own provider turns) whose registry
//   lacks spawn_agent, and whose document ops reuse the PARENT sessionId
// - unknown childId fails closed as a tool error before any child turn
// - wait_agent on a foreign/stale delegationId is a tool error
// - parent abort (session.cancel) stops a running async child
// - document-registry authority: no createWorktreeChildFactory import
import { test } from "bun:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  type AIProvider,
  type JsonObject,
  type ProviderEvent,
  providerDone,
  providerTextDelta,
  providerToolCall,
  toolCallContent,
} from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

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

interface CapturedRequest {
  tools?: ReadonlyArray<{ name?: string }>;
}

interface Fixture {
  host: ClayAgentHost;
  sessionId: string;
  /** Daemon "event" emissions (run events + subagent lifecycle). */
  events: Array<Record<string, unknown>>;
  /** Reverse-RPC calls (document ops), for sessionId assertions. */
  reverse: Array<{ method: string; params: Record<string, unknown> }>;
  /** Every provider turn, in order (parent and child interleaved). */
  requests: CapturedRequest[];
}

type Turn = (request: { signal?: AbortSignal }) => AsyncGenerator<ProviderEvent>;

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-spawn-"));
}

/** Deterministic turn script: the Nth `generate()` call runs turns[N]. */
function scriptedProvider(turns: readonly Turn[], requests: CapturedRequest[]): AIProvider {
  let index = 0;
  return {
    id: "mock",
    async *generate(request) {
      requests.push(request as CapturedRequest);
      yield* turns[Math.min(index++, turns.length - 1)]!(request as CapturedRequest & { signal?: AbortSignal });
    },
  };
}

const finishTurn =
  (text: string): Turn =>
  async function* () {
    yield providerTextDelta(text);
    yield providerDone();
  };

const toolCallTurn =
  (id: string, name: string, args: JsonObject): Turn =>
  async function* () {
    yield providerToolCall(toolCallContent(id, name, args));
    yield providerDone();
  };

/** A turn that settles only on its abort signal (Prism leaves
 *  signal-oblivious providers hanging — see compaction.test.ts). */
const hangTurn: Turn = async function* (request) {
  yield providerTextDelta("busy");
  await new Promise<never>((_resolve, reject) => {
    request.signal?.addEventListener("abort", () => {
      reject(request.signal?.reason ?? new Error("aborted"));
    });
  });
};

async function spawnFixture(turns: readonly Turn[]): Promise<Fixture> {
  const events: Array<Record<string, unknown>> = [];
  const reverse: Array<{ method: string; params: Record<string, unknown> }> = [];
  const requests: CapturedRequest[] = [];
  let hostRef: ClayAgentHost | undefined;
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: scriptedProvider(turns, requests),
    emit: (method, params) => {
      if (method === "event") {
        events.push((params as { event: Record<string, unknown> }).event);
        return;
      }
      if (method !== "reverse") return;
      const frame = params as { id: number; method: string; params?: Record<string, unknown> };
      reverse.push({ method: frame.method, params: frame.params ?? {} });
      const answer = (): unknown => {
        switch (frame.method) {
          case "document.read":
            return { text: "", version: 1, dirty: false, open: false };
          case "document.stat":
            return { size: 0 };
          case "document.write":
            return { version: 2, saved: true, open: false };
          case "document.mkdir":
            return {};
          default:
            throw new Error(`unknown reverse method: ${frame.method}`);
        }
      };
      answerAs(frame, answer, hostRef!);
    },
    // Hermetic agent config: no ambient MCP/capability tools or skills.
    agentConfigRoot: await tempDir(),
  });
  hostRef = host;
  await host.handle("agentProfile.register", { name: "coding", instructions: "Code.", tools: CODING_TOOLS });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws",
  })) as { sessionId: string; tools: string[] };
  return { host, sessionId: created.sessionId, events, reverse, requests };
}

function answerAs(
  frame: { id: number },
  answer: () => unknown,
  host: ClayAgentHost,
): void {
  Promise.resolve()
    .then(answer)
    .then(
      (result) => host.resolveReverse(frame.id, { result }),
      (error: unknown) =>
        host.resolveReverse(frame.id, { error: { code: -32000, message: String(error) } }),
    );
}

function toolNames(request: CapturedRequest | undefined): string[] {
  return (request?.tools ?? []).map((tool) => tool.name ?? "");
}

function subagentEvents(events: Array<Record<string, unknown>>) {
  return events.filter((event) => String(event.type).startsWith("subagent_"));
}

async function waitFor(check: () => boolean, label: string, deadlineMs = 5000): Promise<void> {
  const deadline = Date.now() + deadlineMs;
  while (!check()) {
    if (Date.now() > deadline) assert.fail(`timed out waiting for ${label}`);
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
}

test("coding session tools include spawn/wait/cancel; Chat stays without", async () => {
  // Cheap shape check via session.new returns on both profiles.
  const fixture = await spawnFixture([finishTurn("ok")]);
  const { host } = fixture;
  try {
    const chat = (await host.handle("session.new", {
      profile: "Chat",
      provider: "mock",
      model: "demo",
      workspaceRoot: "/ws",
    })) as { sessionId: string; tools: string[] };
    for (const name of ["spawn_agent", "wait_agent", "cancel_agent"]) {
      assert.ok(!chat.tools.includes(name), `Chat must not list ${name}`);
    }
    const coding = (await host.handle("session.new", {
      profile: "coding",
      provider: "mock",
      model: "demo",
      workspaceRoot: "/ws",
    })) as { sessionId: string; tools: string[] };
    for (const name of ["spawn_agent", "wait_agent", "cancel_agent"]) {
      assert.ok(coding.tools.includes(name), `coding must list ${name}: ${coding.tools.join(",")}`);
    }
  } finally {
    host.close();
  }
});

test("sync spawn runs an isolated child: no spawn tools, parent sessionId on writes, lifecycle rows", async () => {
  // Global provider turn order: parent calls spawn_agent → child calls write
  // → child finishes → parent finishes.
  const fixture = await spawnFixture([
    toolCallTurn("c1", "spawn_agent", { childId: "test", input: "write the file" }),
    toolCallTurn("c2", "write", { path: "/ws/child.txt", content: "from child" }),
    finishTurn("wrote it"),
    finishTurn("done"),
  ]);
  const { host, sessionId, events, reverse, requests } = fixture;
  try {
    const result = (await host.handle("session.prompt", {
      sessionId,
      text: "spawn test child",
    })) as { status: string };
    assert.equal(result.status, "succeeded");
    // Child registry: coding tools yes, supervisor tools no (no recursion).
    const childTools = toolNames(requests[1]);
    assert.ok(childTools.includes("write"), `child must keep write: ${childTools.join(",")}`);
    assert.ok(!childTools.includes("spawn_agent"), "child must not carry spawn_agent");
    // Document authority: the child's write went through reverse RPC stamped
    // with the PARENT session id.
    const write = reverse.find((call) => call.method === "document.write");
    assert.ok(write, "child write must reach document.write");
    assert.equal(write.params.sessionId, sessionId, "child document ops use the parent sessionId");
    // Lifecycle bridge: started/stopped pair on the daemon event stream.
    const lifecycle = subagentEvents(events);
    assert.ok(
      lifecycle.some((event) => event.type === "subagent_started" && event.childId === "test"),
      `expected subagent_started(test): ${JSON.stringify(lifecycle)}`,
    );
    assert.ok(
      lifecycle.some((event) => event.type === "subagent_stopped" && event.status === "succeeded"),
      `expected subagent_stopped(succeeded): ${JSON.stringify(lifecycle)}`,
    );
  } finally {
    host.close();
  }
});

test("unknown childId fails closed as a tool error before any child turn", async () => {
  const fixture = await spawnFixture([
    toolCallTurn("c1", "spawn_agent", { childId: "evil", input: "nope" }),
    finishTurn("done"),
  ]);
  const { host, sessionId, events, requests } = fixture;
  try {
    const result = (await host.handle("session.prompt", {
      sessionId,
      text: "spawn evil child",
    })) as { status: string };
    assert.equal(result.status, "succeeded");
    // Two provider turns only (parent call + parent finish): the child
    // factory never ran.
    assert.equal(requests.length, 2, "no child provider turn may happen");
    // The closed spawn schema (childId enum = supervisor catalog) blocks
    // unknown ids at validation, before the tool body — and the tool body
    // re-checks against the frozen catalog.
    const blocked = events.find(
      (event) =>
        event.type === "tool_execution_blocked" && (event as { name?: string }).name === "spawn_agent",
    );
    assert.ok(blocked, "spawn_agent must fail closed on unknown childId");
    assert.equal((blocked as { reason?: string }).reason, "validation_failed");
  } finally {
    host.close();
  }
});

test("wait_agent on a foreign delegationId is a tool error", async () => {
  const fixture = await spawnFixture([
    toolCallTurn("c1", "wait_agent", { delegationId: "not-a-real-handle" }),
    finishTurn("done"),
  ]);
  const { host, sessionId, events } = fixture;
  try {
    const result = (await host.handle("session.prompt", {
      sessionId,
      text: "wait bogus",
    })) as { status: string };
    assert.equal(result.status, "succeeded");
    const wait = events.find(
      (event) =>
        event.type === "tool_execution_finished" &&
        (event as { result?: { name?: string } }).result?.name === "wait_agent",
    );
    const message = (wait as { result?: { error?: { message?: string } } })?.result?.error?.message;
    assert.ok(message, "wait_agent must surface a tool error for a foreign delegationId");
  } finally {
    host.close();
  }
});

test("parent abort stops a running async child", async () => {
  // Turn order: parent spawns async → child hangs → parent hangs.
  const fixture = await spawnFixture([
    toolCallTurn("c1", "spawn_agent", { childId: "test", input: "loop forever", mode: "async" }),
    hangTurn,
    hangTurn,
  ]);
  const { host, sessionId, events } = fixture;
  try {
    const prompt = host.handle("session.prompt", { sessionId, text: "spawn then hang" }).then(
      (value) => value,
      (error) => error,
    );
    await waitFor(
      () => subagentEvents(events).some((event) => event.type === "subagent_started"),
      "subagent_started",
    );
    await host.handle("session.cancel", { sessionId });
    await prompt;
    await waitFor(
      () => subagentEvents(events).some((event) => event.type === "subagent_stopped"),
      "subagent_stopped after abort",
    );
  } finally {
    host.close();
  }
});

test("document-registry authority holds: no worktree child factory import", async () => {
  // Runs compiled (dist/__tests__ → dist/host.js); fall back to src for
  // source-mode runs. Either artifact reflects the same import graph.
  for (const file of ["../host", "../coding-tools"]) {
    let source: string;
    try {
      source = await readFile(new URL(`${file}.js`, import.meta.url), "utf8");
    } catch {
      source = await readFile(new URL(`${file}.ts`, import.meta.url), "utf8");
    }
    assert.ok(
      !/createWorktreeChildFactory\s*\(/.test(source),
      `${file} must not call createWorktreeChildFactory (plan 122: worktrees deferred)`,
    );
  }
});
