// Plan 109 I4: the daemon's portable thinking-level boundary.
//
// The daemon accepts `session.prompt.thinkingLevel`, fail-closes empty /
// non-string values (Prism `parseThinkingLevel`), applies the model-aware
// Prism 0.5.0 mapping (`applyThinkingLevelForModel`: family + snap + merge
// in one call), and exposes declared levels via `model.list.thinkingLevels`
// (Prism `thinkingLevelsForModel`). Wire-field assertions use a capturing
// mock provider so the per-run `providerOptions.compat` patch is visible
// exactly as a provider package would read it.
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createMockProvider, providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-effort-"));
}

interface CapturedRequest {
  model?: { provider?: string; model?: string };
  options?: { compat?: Record<string, unknown> };
}

async function hostWithCapturingProvider(events: CapturedRequest[]) {
  const dataDir = await tempDir();
  const provider = createMockProvider([providerTextDelta("Hello"), providerDone()], {
    onRequest: (request) => {
      events.push(request as CapturedRequest);
    },
  });
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
    emit: () => {},
    registerModels: (registries) => {
      // Two reasoner models with different family stamps + a declared
      // ladder, and a non-reasoning control model.
      registries.models.register({
        provider: "mock",
        model: "effort-anthropic",
        displayName: "Effort Anthropic",
        compat: { thinkingFamily: "output_config_effort" },
        capabilities: { reasoning: true, thinkingLevels: ["low", "medium", "high"] },
      });
      registries.models.register({
        provider: "mock",
        model: "effort-xai",
        displayName: "Effort xAI",
        compat: { thinkingFamily: "reasoning_effort" },
        capabilities: { reasoning: true, thinkingLevels: ["low", "medium", "high"] },
      });
      registries.models.register({
        provider: "mock",
        model: "effort-google",
        displayName: "Effort Google",
        compat: { thinkingFamily: "google" },
        capabilities: { reasoning: true, thinkingLevels: ["low", "medium", "high"] },
      });
      registries.models.register({
        provider: "mock",
        model: "plain",
        displayName: "Plain",
      });
    },
  });
  const created = (await host.handle("session.new", {
    profile: "Chat",
    provider: "mock",
    model: "effort-anthropic",
  })) as { sessionId: string };
  return { host, sessionId: created.sessionId };
}

async function switchModel(
  host: ClayAgentHost,
  sessionId: string,
  model: string,
): Promise<void> {
  await host.handle("session.prompt", { sessionId, text: "switch", model });
}

test("selected level reaches the 0.5 wire field for each family", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await hostWithCapturingProvider(events);
  try {
    // Anthropic-family stamp: output_config.effort (no 0.4 silent drop).
    await host.handle("session.prompt", { sessionId, text: "Hi", thinkingLevel: "high" });
    const anthropic = events.at(-1);
    assert.equal(
      (anthropic?.options?.compat?.output_config as { effort?: string } | undefined)?.effort,
      "high",
    );
    // xAI-family stamp: reasoning_effort.
    await switchModel(host, sessionId, "effort-xai");
    await host.handle("session.prompt", { sessionId, text: "Hi", thinkingLevel: "low" });
    const xai = events.at(-1);
    assert.equal(xai?.options?.compat?.reasoning_effort, "low");
    // Google-family stamp: thinkingLevel.
    await switchModel(host, sessionId, "effort-google");
    await host.handle("session.prompt", { sessionId, text: "Hi", thinkingLevel: "medium" });
    const google = events.at(-1);
    assert.equal(google?.options?.compat?.thinkingLevel, "medium");
  } finally {
    host.close();
  }
});

test("out-of-set levels snap to the model's declared ladder", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await hostWithCapturingProvider(events);
  try {
    // max is above the declared ladder; the adapter snaps to the nearest
    // declared level (ties up) instead of dropping or passing through.
    await host.handle("session.prompt", { sessionId, text: "Hi", thinkingLevel: "max" });
    const captured = events.at(-1);
    assert.equal(
      (captured?.options?.compat?.output_config as { effort?: string } | undefined)?.effort,
      "high",
    );
  } finally {
    host.close();
  }
});

test("empty and non-string levels are rejected fail-closed at the boundary", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await hostWithCapturingProvider(events);
  try {
    for (const bad of ["", 42, null, { level: "high" }]) {
      await assert.rejects(
        () => host.handle("session.prompt", { sessionId, text: "Hi", thinkingLevel: bad }),
        /thinkingLevel must be a non-empty string/,
      );
    }
    assert.equal(events.length, 0, "no provider request may fire for invalid levels");
  } finally {
    host.close();
  }
});

test("non-reasoning model: no compat field is invented", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await hostWithCapturingProvider(events);
  try {
    await switchModel(host, sessionId, "plain");
    await host.handle("session.prompt", { sessionId, text: "Hi", thinkingLevel: "high" });
    const captured = events.at(-1);
    const compat = captured?.options?.compat ?? {};
    assert.equal(
      compat.output_config === undefined && compat.reasoning_effort === undefined
        && compat.thinkingLevel === undefined,
      true,
      `unexpected thinking compat on non-reasoning model: ${JSON.stringify(compat)}`,
    );
  } finally {
    host.close();
  }
});

test("model.list exposes declared thinkingLevels; undeclared models omit them", async () => {
  const events: CapturedRequest[] = [];
  const { host } = await hostWithCapturingProvider(events);
  try {
    const listed = (await host.handle("model.list", {})) as {
      models: Array<{ model: string; thinkingLevels?: string[] }>;
    };
    const byId = new Map(listed.models.map((model) => [model.model, model]));
    assert.deepEqual(byId.get("effort-anthropic")?.thinkingLevels, ["low", "medium", "high"]);
    assert.equal(byId.get("plain")?.thinkingLevels, undefined);
  } finally {
    host.close();
  }
});
