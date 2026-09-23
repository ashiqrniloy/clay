// Plan 121 follow-up: auto-compaction on the attention-compiler pair.
//
// The daemon arms `AgentConfig.compaction` (trigger only, local strategy) on
// exactly the sessions that run the attention compiler: coding profiles whose
// resolved model declares a context window. The composed trigger fires when
//   - two consecutive `truncated` attention reports arrive (stubs can no
//     longer hold the request — rebuild the prefix),
//   - the assembled input sits at the compiler's compactRatio (0.9 of the
//     resolved input cap), or
//   - the absolute `run.setOptions.compactAfterTokens` ceiling is reached.
// Prism runs `autoCompact` once per run, before provider turns; the strategy
// stays Prism's local deterministic default so an unattended gate can never
// turn a provider outage into a failed run at assembly.
import { test } from "bun:test";
import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  createMockProvider,
  providerDone,
  providerTextDelta,
  providerThinkingDelta,
  type AgentEvent,
  type AttentionTruncationTrigger,
} from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(prefix: string): Promise<string> {
  return mkdtemp(join(tmpdir(), prefix));
}

interface Fixture {
  host: ClayAgentHost;
  requests: unknown[];
  events: AgentEvent[];
  sessionId: string;
  truncation: () => AttentionTruncationTrigger;
}

async function fixture(): Promise<Fixture> {
  const requests: unknown[] = [];
  const events: AgentEvent[] = [];
  // One thinking block per turn: the compiler has aged rows to strip.
  const provider = createMockProvider(
    [providerThinkingDelta("t".repeat(800)), providerTextDelta("ok"), providerDone()],
    { onRequest: (request) => requests.push(request) },
  );
  const host = await ClayAgentHost.create({
    dataDir: await tempDir("clay-agent-autocompact-"),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: (method, params) => {
      if (method !== "event") return;
      events.push((params as { event: AgentEvent }).event);
    },
    agentConfigRoot: await tempDir("clay-agent-autocompact-config-"),
    registerModels: (registries) => {
      // 4096 - 1024 - 1024 reserve = 2048 input cap ⇒ trigger 1536,
      // compactRatio gate 1843.
      registries.models.register({
        provider: "mock",
        model: "windowed",
        displayName: "Windowed",
        limits: { contextWindow: 4096, maxOutputTokens: 1024 },
      });
    },
  });
  await host.handle("agentProfile.register", {
    name: "coding",
    instructions: "Code.",
    tools: ["read", "write", "edit", "shell"],
  });
  await host.handle("agentProfile.register", { name: "chat", instructions: "Chat." });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "windowed",
    workspaceRoot: await tempDir("clay-agent-autocompact-ws-"),
  })) as { sessionId: string };
  const truncation = (): AttentionTruncationTrigger => {
    const live = (host as unknown as { live: Map<string, { attentionTruncation?: AttentionTruncationTrigger }> }).live.get(
      created.sessionId,
    );
    assert.ok(live?.attentionTruncation, "truncation trigger armed");
    return live.attentionTruncation;
  };
  return { host, requests, events, sessionId: created.sessionId, truncation };
}

function compactionEvents(events: AgentEvent[]): string[] {
  return events.filter((event) => event.type === "compaction_started" || event.type === "compaction_finished").map((event) => event.type);
}

test("auto-compaction is armed exactly where the attention compiler is", async () => {
  const { host, sessionId } = await fixture();
  try {
    const live = (host as unknown as { live: Map<string, { agent: { config: Record<string, unknown> } }> }).live.get(
      sessionId,
    );
    const compaction = live?.agent.config.compaction as { strategy?: unknown; trigger?: { type?: string } } | undefined;
    assert.equal(compaction?.trigger?.type, "custom", "coding + windowed model arms the trigger");
    assert.equal(compaction?.strategy, undefined, "auto-compact stays on the local default strategy");

    // Chat: no compiler ⇒ no auto-compaction either.
    const chat = (await host.handle("session.new", {
      profile: "chat",
      provider: "mock",
      model: "windowed",
      workspaceRoot: await tempDir("clay-agent-autocompact-chat-"),
    })) as { sessionId: string };
    const chatLive = (host as unknown as { live: Map<string, { agent: { config: Record<string, unknown> } }> }).live.get(
      chat.sessionId,
    );
    assert.equal(chatLive?.agent.config.compaction, undefined, "chat keeps no implicit branch rewrite");

    // Coding on a limit-less model: no cap to compare against ⇒ no trigger
    // (the ceiling alone must not silently compact a branch).
    await host.handle("agentProfile.register", {
      name: "coding-demo",
      instructions: "Code.",
      tools: ["read", "write", "edit", "shell"],
    });
    const limited = (await host.handle("session.new", {
      profile: "coding-demo",
      provider: "mock",
      model: "demo",
      workspaceRoot: await tempDir("clay-agent-autocompact-demo-"),
    })) as { sessionId: string };
    const limitedLive = (
      host as unknown as { live: Map<string, { agent: { config: Record<string, unknown> } }> }
    ).live.get(limited.sessionId);
    assert.equal(limitedLive?.agent.config.compaction, undefined, "limit-less models keep the prior behavior");
  } finally {
    host.close();
  }
});

test("the compactRatio gate compacts locally before provider turns", async () => {
  const { host, requests, events, sessionId } = await fixture();
  try {
    // ~10k estimated tokens against a 2048-token cap: far past the 0.9 gate.
    await assert.rejects(
      () => host.handle("session.prompt", { sessionId, text: "x".repeat(40_000) }),
      /attention budget exceeded/,
    );
    assert.deepEqual(compactionEvents(events), ["compaction_started", "compaction_finished"]);
    const finished = events.find((event) => event.type === "compaction_finished");
    assert.match((finished as { summary?: string })?.summary ?? "", /./, "a local summary is written");
    assert.equal(requests.length, 0, "the local strategy needs no provider call");
  } finally {
    host.close();
  }
});

test("an absolute compactAfterTokens ceiling fires under the ratio gate", async () => {
  const { host, requests, events, sessionId } = await fixture();
  try {
    // 200 tokens is far below the 0.9 gate (1843) but must still compact:
    // this is the knob being live, not stored-and-unused. The prompt stays
    // small enough that the post-compaction input is under the trigger ratio
    // (tool schemas + instructions already cost ~1000 of the 2048 cap).
    await host.handle("run.setOptions", { compactAfterTokens: 200 });
    const created = (await host.handle("session.new", {
      profile: "coding",
      provider: "mock",
      model: "windowed",
      workspaceRoot: await tempDir("clay-agent-autocompact-ceiling-"),
    })) as { sessionId: string };
    const result = (await host.handle("session.prompt", { sessionId: created.sessionId, text: "y".repeat(1400) })) as {
      lastEvent?: string;
    };
    assert.equal(result.lastEvent, "agent_finished", "the run proceeds after compacting");
    assert.deepEqual(compactionEvents(events), ["compaction_started", "compaction_finished"]);
    assert.equal(requests.length, 1, "only the agent turn reaches the provider");
    void sessionId;
  } finally {
    host.close();
  }
});

test("attention_compiled events feed the streak; two truncated turns arm the next compaction", async () => {
  const { host, events, sessionId, truncation } = await fixture();
  try {
    const trigger = truncation();
    const observed: unknown[] = [];
    const original = trigger.observe.bind(trigger);
    trigger.observe = (report: { readonly truncated?: unknown }) => {
      observed.push(report);
      return original(report);
    };

    // Grow the branch to a real mutated turn: the event loop must feed the
    // trigger with Prism's own report (spy proves the wiring, the report is
    // the real one).
    for (let turn = 0; turn < 4; turn += 1) {
      await host.handle("session.prompt", { sessionId, text: "hello world ".repeat(4) });
    }
    const compiled = events.filter((event) => event.type === "attention_compiled");
    assert.ok(compiled.length > 0, "the fixture reaches a mutated turn");
    assert.ok(observed.length > 0, "session.prompt feeds every attention_compiled event to the trigger");
    assert.equal(trigger.streak(), 0, "a non-truncated report keeps the streak clear");

    // Two truncated turns arm the gate; the next prompt compacts whatever its
    // size (the streak is the signal, not the estimate).
    trigger.observe({ truncated: true });
    trigger.observe({ truncated: true });
    assert.equal(trigger.streak(), 2);
    const before = compactionEvents(events).length;
    await host.handle("session.prompt", { sessionId, text: "small" });
    assert.deepEqual(compactionEvents(events).slice(before), ["compaction_started", "compaction_finished"]);
    assert.equal(trigger.streak(), 0, "the trigger resets after firing");
  } finally {
    host.close();
  }
});
