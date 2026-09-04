import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  type AIProvider,
  providerDone,
  providerTextDelta,
  providerToolCall,
  toolCallContent,
} from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-durable-"));
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

interface MockDoc {
  text: string;
  version: number;
  dirty: boolean;
}

interface Fixture {
  host: ClayAgentHost;
  reads: () => number;
  sessionId: string;
}

/** Provider: turn 1 calls `read /ws/a.txt`, later turns just finish. */
function readOnceProvider(): AIProvider {
  let turns = 0;
  return {
    id: "mock",
    async *generate() {
      turns += 1;
      if (turns === 1) {
        yield providerToolCall(toolCallContent("c1", "read", { path: "/ws/a.txt" }));
        yield providerDone();
      } else {
        yield providerTextDelta("read it");
        yield providerDone();
      }
    },
  };
}

async function codingFixture(): Promise<Fixture> {
  const docs = new Map<string, MockDoc>([[ "/ws/a.txt", { text: "hello", version: 1, dirty: false } ]]);
  let reads = 0;
  const answer = async (method: string, params: Record<string, unknown>): Promise<unknown> => {
    const path = String(params.path);
    if (method === "document.read") {
      const doc = docs.get(path);
      if (!doc) throw new Error(`file unavailable: ${path}`);
      reads += 1;
      return { text: doc.text, version: doc.version, dirty: doc.dirty, open: true };
    }
    if (method === "document.stat") {
      const doc = docs.get(path);
      if (!doc) throw new Error(`file unavailable: ${path}`);
      return { size: Buffer.byteLength(doc.text, "utf8"), open: true };
    }
    throw new Error(`unknown reverse method: ${method}`);
  };
  let hostRef: ClayAgentHost | undefined;
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: readOnceProvider(),
    emit: (method, params) => {
      if (method !== "reverse") return;
      const frame = params as { id: number; method: string; params?: Record<string, unknown> };
      answer(frame.method, frame.params ?? {}).then(
        (result) => hostRef!.resolveReverse(frame.id, { result }),
        (error: unknown) => hostRef!.resolveReverse(frame.id, { error: { code: -32000, message: String(error) } }),
      );
    },
  });
  hostRef = host;
  await host.handle("agentProfile.register", { name: "coding", tools: CODING_TOOLS });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws",
  })) as { sessionId: string };
  return { host, reads: () => reads, sessionId: created.sessionId };
}

interface Suspended {
  status: "suspended";
  runId: string;
  version: number;
  interruption: { pendingDecisions: Array<{ approvalId: string; scope: { toolName: string } }> };
}

test("coding run suspends before tool side effect; resume approve executes once; stale version fails closed", async () => {
  const fixture = await codingFixture();
  const { host, reads, sessionId } = fixture;
  const prompt = (await host.handle("session.prompt", { sessionId, text: "read a.txt" })) as Suspended;
  assert.equal(prompt.status, "suspended");
  assert.ok(prompt.version > 0);
  assert.equal(prompt.interruption.pendingDecisions.length, 1);
  const decision = prompt.interruption.pendingDecisions[0]!;
  assert.equal(decision.scope.toolName, "read");
  assert.equal(reads(), 0, "tool must not execute before approval");

  const resumed = (await host.handle("run.resume", {
    sessionId,
    runId: prompt.runId,
    expectedVersion: prompt.version,
    decisions: [{ approvalId: decision.approvalId, outcome: "allow_once" }],
  })) as { status: string };
  assert.equal(resumed.status, "succeeded");
  assert.equal(reads(), 1, "approved tool executes exactly once");

  await assert.rejects(
    () =>
      host.handle("run.resume", {
        sessionId,
        runId: prompt.runId,
        expectedVersion: prompt.version,
        decision: "approve",
      }),
    /stale|non-suspended/i,
  );
  assert.equal(reads(), 1, "stale resume has no side effect");
  host.close();
});

test("malformed resume (unknown decision or outcome) fails closed before any checkpoint I/O", async () => {
  const fixture = await codingFixture();
  const { host, sessionId } = fixture;
  const prompt = (await host.handle("session.prompt", { sessionId, text: "read a.txt" })) as Suspended;
  await assert.rejects(
    () =>
      host.handle("run.resume", {
        sessionId,
        runId: prompt.runId,
        expectedVersion: prompt.version,
        decision: "sideways",
      }),
    /unknown decision/i,
  );
  await assert.rejects(
    () =>
      host.handle("run.resume", {
        sessionId,
        runId: prompt.runId,
        expectedVersion: prompt.version,
        decisions: [{ approvalId: "nope", outcome: "maybe" }],
      }),
    /outcome must be/i,
  );
  // Checkpoint survived both malformed resumes untouched: valid resume still works.
  const decision = prompt.interruption.pendingDecisions[0]!;
  const resumed = (await host.handle("run.resume", {
    sessionId,
    runId: prompt.runId,
    expectedVersion: prompt.version,
    decisions: [{ approvalId: decision.approvalId, outcome: "allow_once" }],
  })) as { status: string };
  assert.equal(resumed.status, "succeeded");
  host.close();
});

test("chat runs stay non-durable: prompt never writes run state, resume fails closed", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  const prompt = (await host.handle("session.prompt", {
    sessionId: created.sessionId,
    text: "hi",
  })) as { status: string };
  assert.equal(prompt.status, "succeeded");
  await assert.rejects(
    () =>
      host.handle("run.resume", {
        sessionId: created.sessionId,
        runId: "r1",
        expectedVersion: 1,
        decision: "approve",
      }),
    /stale|non-suspended|run state|No durable/i,
  );
  host.close();
});