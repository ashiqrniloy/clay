// Plan 113: Prism 0.5.1 kernel request construction. With no host request
// policy, the kernel fills options.sessionId/cacheKey from the session id
// and stamps default cache breakpoints for cache-control models (decision
// 2026-09-07-2149; the host createSessionCachePolicy stopgap from decision
// 1325 is deleted and denied in tests/agent_protocol.rs).
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createMockProvider, providerDone, providerTextDelta } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

interface CapturedRequest {
  options?: {
    sessionId?: string;
    cacheKey?: string;
    cacheRetention?: string;
    cache?: { breakpoints?: ReadonlyArray<{ location?: string }> };
  };
}

async function hostWithCapturingProvider(
  events: CapturedRequest[],
  modelCache?: { kind: "cache_control" },
): Promise<{ host: ClayAgentHost; sessionId: string }> {
  const dataDir = await mkdtemp(join(tmpdir(), "clay-agent-reqcon-"));
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
      registries.models.register({
        provider: "mock",
        model: "cached",
        displayName: "Cached",
        ...(modelCache ? { cache: modelCache } : {}),
      });
    },
  });
  const created = (await host.handle("session.new", {
    profile: "Chat",
    provider: "mock",
    model: "cached",
  })) as { sessionId: string };
  return { host, sessionId: created.sessionId };
}

test("session.prompt fills sessionId/cacheKey without a host policy", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await hostWithCapturingProvider(events);
  try {
    await host.handle("session.prompt", { sessionId, text: "Hi" });
    const captured = events.at(-1);
    assert.equal(captured?.options?.sessionId, sessionId);
    assert.equal(captured?.options?.cacheKey, sessionId);
  } finally {
    host.close();
  }
});

test("cache_control model gets default short retention + kernel breakpoints", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await hostWithCapturingProvider(events, { kind: "cache_control" });
  try {
    await host.handle("session.prompt", { sessionId, text: "Hi" });
    const captured = events.at(-1);
    assert.equal(captured?.options?.cacheRetention, "short");
    assert.deepEqual(captured?.options?.cache?.breakpoints, [
      { location: "system_prompt" },
      { location: "last_stable_message" },
    ]);
  } finally {
    host.close();
  }
});

test("models without cache capabilities get no cache patch", async () => {
  const events: CapturedRequest[] = [];
  const { host, sessionId } = await hostWithCapturingProvider(events);
  try {
    await host.handle("session.prompt", { sessionId, text: "Hi" });
    const captured = events.at(-1);
    assert.equal(captured?.options?.cacheRetention, undefined);
    assert.equal(captured?.options?.cache, undefined);
  } finally {
    host.close();
  }
});
