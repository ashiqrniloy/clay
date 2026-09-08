import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-run-options-"));
}

test("run.setOptions defaults and partial update", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  const first = (await host.handle("run.setOptions", { maxInputTokens: 500_000 })) as {
    maxInputTokens: number | null;
    maxOutputTokens: number | null;
    maxTurns: number | null;
    maxToolCalls: number | null;
    maxWallTimeMs: number | null;
    compactAfterTokens: number;
    compaction: string;
  };
  assert.equal(first.maxInputTokens, 500_000);
  assert.equal(first.maxOutputTokens, null);
  assert.equal(first.maxTurns, null);
  assert.equal(first.maxToolCalls, null);
  assert.equal(first.maxWallTimeMs, null);
  assert.equal(first.compactAfterTokens, 800_000);
  assert.equal(first.compaction, "llm");
  const second = (await host.handle("run.setOptions", { compaction: "om" })) as {
    maxInputTokens: number | null;
    compaction: string;
  };
  assert.equal(second.maxInputTokens, 500_000);
  assert.equal(second.compaction, "om");
  host.close();
});

test("run.setOptions accepts null and values above former HARD", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  const cleared = (await host.handle("run.setOptions", { maxInputTokens: null })) as {
    maxInputTokens: number | null;
  };
  assert.equal(cleared.maxInputTokens, null);
  const raised = (await host.handle("run.setOptions", {
    maxInputTokens: 5_000_000,
    maxOutputTokens: 1_000_000,
    maxTurns: 128,
  })) as {
    maxInputTokens: number | null;
    maxOutputTokens: number | null;
    maxTurns: number | null;
  };
  assert.equal(raised.maxInputTokens, 5_000_000);
  assert.equal(raised.maxOutputTokens, 1_000_000);
  assert.equal(raised.maxTurns, 128);
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  assert.equal(typeof created.sessionId, "string");
  host.close();
});

test("run.setOptions rejects empty, non-positive, and unknown compaction", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await assert.rejects(() => host.handle("run.setOptions", {}), /requires a policy cap/);
  await assert.rejects(
    () => host.handle("run.setOptions", { maxInputTokens: 0 }),
    /maxInputTokens must be a positive safe integer or null/,
  );
  await assert.rejects(
    () => host.handle("run.setOptions", { maxOutputTokens: 1.5 }),
    /maxOutputTokens must be a positive safe integer or null/,
  );
  await assert.rejects(
    () => host.handle("run.setOptions", { compaction: "magic" }),
    /compaction must be one of/,
  );
  host.close();
});

test("session.compact without strategy uses run.setOptions compaction", async () => {
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
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  const compacted = (await host.handle("session.compact", {
    sessionId: created.sessionId,
  })) as { strategy: string };
  assert.equal(compacted.strategy, "llm");
  await host.handle("run.setOptions", { compaction: "default" });
  const again = (await host.handle("session.compact", {
    sessionId: created.sessionId,
  })) as { strategy: string };
  assert.equal(again.strategy, "default");
  host.close();
});
