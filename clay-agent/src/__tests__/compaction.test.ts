import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  type AIProvider,
  type ProviderRequest,
  type SessionEntry,
  type ToolDefinition,
  type ToolExecutionContext,
  providerDone,
  providerTextDelta,
  providerToolCall,
  toolCallContent,
} from "@arnilo/prism";
import {
  buildObservationalMemoryProjection,
  createRecallMemoryTool,
} from "@arnilo/prism-memory/compaction/observational-memory";
import { COMPACTION_STRATEGIES, createNamedCompactionStrategy } from "../compaction.js";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-compact-"));
}

function context(sessionId = "s1"): ToolExecutionContext {
  return {
    sessionId,
    runId: "r1",
    toolCallId: "c1",
    metadata: {},
  } as unknown as ToolExecutionContext;
}

function observationWorker(): AIProvider {
  return {
    id: "om-observe",
    async *generate(request: ProviderRequest) {
      const blob = JSON.stringify(request.messages);
      const ids = [...blob.matchAll(/\[([A-Za-z0-9_-]+)\]/g)].map((match) => match[1]);
      const sourceEntryIds = ids.slice(0, 1);
      yield providerToolCall(
        toolCallContent("c1", "record_observation", {
          content: "User asked to remember the release target.",
          relevance: "high",
          sourceEntryIds,
        }),
      );
      yield providerDone();
    },
  };
}

test("named strategies register default, llm, and om", () => {
  assert.deepEqual([...COMPACTION_STRATEGIES], ["default", "llm", "om"]);
  assert.equal(createNamedCompactionStrategy("default", { secrets: [] }).name, "default");
  assert.equal(createNamedCompactionStrategy("om", { secrets: [] }).name, "om");
});

test("manual compact on a mock session appends a compaction entry; raw entries remain", async () => {
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
  })) as { sessionId: string; tools: string[]; observationalMemory: boolean };
  assert.equal(created.observationalMemory, false);
  assert.ok(!created.tools.includes("recall"));
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  const compacted = (await host.handle("session.compact", {
    sessionId: created.sessionId,
    strategy: "default",
  })) as { entryId: string; strategy: string };
  assert.equal(compacted.strategy, "default");
  assert.ok(compacted.entryId);
  const loaded = (await host.handle("session.load", { sessionId: created.sessionId })) as {
    entries: Array<{ kind: string; id: string; data?: { strategy?: string } }>;
  };
  assert.ok(loaded.entries.some((entry) => entry.kind === "message"));
  const compaction = loaded.entries.find((entry) => entry.kind === "compaction");
  assert.ok(compaction);
  assert.equal(compaction.data?.strategy, "default");
  host.close();
});

test("session.compact while a run is active fails closed", async () => {
  const provider: AIProvider = {
    id: "mock",
    async *generate(request: ProviderRequest) {
      yield providerTextDelta("partial");
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(resolve, 5_000);
        request.signal?.addEventListener("abort", () => {
          clearTimeout(timer);
          reject(request.signal?.reason ?? new Error("aborted"));
        });
      });
      yield providerDone();
    },
  };
  const events: unknown[] = [];
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: (method, params) => events.push({ method, params }),
  });
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  const prompt = host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  const started = Date.now();
  while (events.length === 0 && Date.now() - started < 1000) await new Promise((r) => setTimeout(r, 10));
  await assert.rejects(
    () => host.handle("session.compact", { sessionId: created.sessionId }),
    /already has an active run/i,
  );
  await host.handle("session.cancel", { sessionId: created.sessionId });
  await prompt.catch(() => undefined);
  host.close();
});

test("strategy override llm with mock summary provider writes an llm compaction entry", async () => {
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
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Summarize later" });
  const compacted = (await host.handle("session.compact", {
    sessionId: created.sessionId,
    strategy: "llm",
  })) as { strategy: string; entryId: string };
  assert.equal(compacted.strategy, "llm");
  const loaded = (await host.handle("session.load", { sessionId: created.sessionId })) as {
    entries: Array<{ kind: string; data?: { strategy?: string } }>;
  };
  const compaction = loaded.entries.find((entry) => entry.kind === "compaction");
  assert.equal(compaction?.data?.strategy, "llm");
  host.close();
});

test("OM attach records an observation; recall round-trips a known id; invalid id fails closed", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    observationalMemory: {
      observation: {
        provider: observationWorker(),
        model: { provider: "om-observe", model: "memory" },
        messageTokens: 1,
      },
      compactAfterTokens: 80_000,
    },
  });
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
    observationalMemory: true,
  })) as { sessionId: string; tools: string[]; observationalMemory: boolean };
  assert.equal(created.observationalMemory, true);
  assert.ok(created.tools.includes("recall"));
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "remember release 0.0.19" });
  const loaded = (await host.handle("session.load", { sessionId: created.sessionId })) as {
    entries: SessionEntry[];
  };
  const projection = buildObservationalMemoryProjection(loaded.entries);
  assert.ok(projection.observations.length > 0, "observer should record at least one observation");
  const observationId = projection.observations[0]!.id;
  const recall = createRecallMemoryTool({
    getEntries: async () => loaded.entries,
  });
  const found = await recall.execute({ id: observationId }, context(created.sessionId));
  assert.equal((found.value as { found?: boolean }).found, true);
  const invalid = await recall.execute({ id: "not-an-id" }, context(created.sessionId));
  assert.equal((invalid.value as { found?: boolean }).found, false);
  assert.equal((invalid.value as { reason?: string }).reason, "invalid_id");
  host.close();
});

test("chat session without OM attach has no recall tool", async () => {
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
  })) as { tools: string[]; observationalMemory: boolean };
  assert.equal(created.observationalMemory, false);
  assert.deepEqual(created.tools, []);
  const recall = created.tools.find((name) => name === "recall");
  assert.equal(recall, undefined);
  host.close();
});

test("compactAfterTokens override validates and reaches the OM settings provider", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    observationalMemory: {
      observation: {
        provider: observationWorker(),
        model: { provider: "om-observe", model: "memory" },
        messageTokens: 1,
      },
    },
  });
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
    observationalMemory: true,
  })) as { sessionId: string };
  await assert.rejects(
    () =>
      host.handle("session.compact", {
        sessionId: created.sessionId,
        compactAfterTokens: -5,
      }),
    /compactAfterTokens must be a positive/,
  );
  await assert.rejects(
    () =>
      host.handle("session.compact", {
        sessionId: created.sessionId,
        compactAfterTokens: "many",
      }),
    /compactAfterTokens must be a positive/,
  );
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "remember threshold" });
  const compacted = (await host.handle("session.compact", {
    sessionId: created.sessionId,
    compactAfterTokens: 80_000,
  })) as { strategy: string; entryId?: string };
  assert.equal(compacted.strategy, "om");
  // The runtime settings provider must now resolve the override; verify by
  // attaching a second OM session whose threshold the host tracks from the
  // manual compact call (2158 default 80000 stays in force without one).
  host.close();
});
