// Plan 121: coding sessions run Prism's attention compiler at its defaults.
//
// The daemon sets `AgentConfig.attentionCompiler: true` only on coding
// sessions (coding tool names active) whose resolved model declares a
// context window — Prism resolves the gate cap at run start and fails closed
// when neither the model window nor `maxInputTokens` exists, so a limit-less
// model must skip the optimization instead of failing every run. Chat /
// tool-free profiles never set the field, and `contextBudget` is never set
// beside it (mutually exclusive). The over-ratio case proves the gate is live:
// the run fails with Prism's `AttentionBudgetError` before any provider turn
// instead of silently evicting history.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createMockProvider, providerDone, providerTextDelta, type AgentEvent } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-attention-"));
}

interface CapturedRequest {
  tools?: ReadonlyArray<{ name?: string }>;
}

interface Fixture {
  host: ClayAgentHost;
  requests: CapturedRequest[];
  events: AgentEvent[];
  sessionId: string;
}

async function fixture(profile: "coding" | "chat", model: "windowed" | "demo"): Promise<Fixture> {
  const requests: CapturedRequest[] = [];
  const events: AgentEvent[] = [];
  const provider = createMockProvider([providerTextDelta("Hello"), providerDone()], {
    onRequest: (request) => {
      requests.push(request as CapturedRequest);
    },
  });
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: (method, params) => {
      if (method !== "event") return;
      events.push((params as { event: AgentEvent }).event);
    },
    // Hermetic agent config: no ambient MCP/capability tools or skills.
    agentConfigRoot: await tempDir(),
    registerModels: (registries) => {
      // A usable cap: 4096 - 1024 - 1024 reserve = 2048 input tokens.
      registries.models.register({
        provider: "mock",
        model: "windowed",
        displayName: "Windowed",
        limits: { contextWindow: 4096, maxOutputTokens: 1024 },
      });
    },
  });
  await host.handle("agentProfile.register", { name: "coding", instructions: "Code.", tools: ["read", "write", "edit", "shell"] });
  await host.handle("agentProfile.register", { name: "chat", instructions: "Chat." });
  const created = (await host.handle("session.new", {
    profile,
    provider: "mock",
    model,
    workspaceRoot: await tempDir(),
  })) as { sessionId: string };
  return { host, requests, events, sessionId: created.sessionId };
}

function agentConfig(host: ClayAgentHost, sessionId: string): { attentionCompiler?: unknown } {
  const live = (host as unknown as {
    live: Map<string, { agent: { config: { attentionCompiler?: unknown } } }>;
  }).live.get(sessionId);
  assert.ok(live, "live session exists");
  return live.agent.config;
}

test("coding session with a windowed model enables the attention compiler", async () => {
  const { host, sessionId } = await fixture("coding", "windowed");
  try {
    assert.equal(agentConfig(host, sessionId).attentionCompiler, true);
  } finally {
    host.close();
  }
});

test("Chat / tool-free profile never sets attentionCompiler", async () => {
  const { host, sessionId } = await fixture("chat", "windowed");
  try {
    assert.equal(agentConfig(host, sessionId).attentionCompiler, undefined);
  } finally {
    host.close();
  }
});

test("coding session on a limit-less model skips the compiler instead of failing runs", async () => {
  const { host, sessionId } = await fixture("coding", "demo");
  try {
    assert.equal(agentConfig(host, sessionId).attentionCompiler, undefined);
    // The session still runs: the missing window must not fail the run.
    await host.handle("session.prompt", { sessionId, text: "Hi" });
  } finally {
    host.close();
  }
});

test("under-ratio turn runs untouched: no attention_compiled, request as before", async () => {
  const { host, requests, events, sessionId } = await fixture("coding", "windowed");
  try {
    await host.handle("session.prompt", { sessionId, text: "Hi" });
    assert.equal(requests.length, 1, "one provider turn");
    assert.equal(
      events.some((event) => event.type === "attention_compiled"),
      false,
      "no telemetry under the ratio",
    );
  } finally {
    host.close();
  }
});

test("over-ratio turn fails with AttentionBudgetError before any provider turn", async () => {
  const { host, requests, sessionId } = await fixture("coding", "windowed");
  try {
    // ~10k estimated tokens vs a 2048-token cap with no stubbable rows: the
    // gate is live and must fail closed rather than evicting. The armed
    // auto-compaction trigger fires first (see auto-compaction.test.ts) but
    // runs locally — a single oversized input is not something compaction can
    // absorb — so no provider request may fire and the run still fails closed.
    await assert.rejects(
      () => host.handle("session.prompt", { sessionId, text: "x".repeat(40_000) }),
      /attention budget exceeded/,
    );
    assert.equal(requests.length, 0, "no provider request may fire on an over-budget turn");
  } finally {
    host.close();
  }
});