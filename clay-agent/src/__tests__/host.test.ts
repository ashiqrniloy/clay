import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
  createMockProvider,
  providerDone,
  providerError,
  providerTextDelta,
  type AIProvider,
  type ProviderRequest,
} from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-"));
}

test("prompt streams mock events, persists, and resumes", async () => {
  const dataDir = await tempDir();
  const events: unknown[] = [];
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: (method, params) => events.push({ method, params }),
  });
  await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  const prompted = (await host.handle("session.prompt", {
    sessionId: created.sessionId,
    text: "Hi",
  })) as { lastEvent: string };
  assert.equal(prompted.lastEvent, "agent_finished");
  assert.ok(events.some((item) => (item as { method: string }).method === "event"));
  const listed = (await host.handle("session.list", {})) as { sessions: Array<{ id: string }> };
  assert.equal(listed.sessions.length, 1);
  host.close();

  const resumedHost = await ClayAgentHost.create({ dataDir, passphrase: "pass-phrase-ok", mock: true });
  await resumedHost.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  const loaded = (await resumedHost.handle("session.load", { sessionId: created.sessionId })) as {
    entries: unknown[];
    profile: string;
  };
  assert.ok(loaded.entries.length > 0);
  assert.equal(loaded.profile, "chat");
  const resumed = (await resumedHost.handle("session.resume", { sessionId: created.sessionId })) as { sessionId: string };
  assert.equal(resumed.sessionId, created.sessionId);
  resumedHost.close();
});

test("cancel aborts an in-flight mock generate", async () => {
  const dataDir = await tempDir();
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
    dataDir,
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
  const started = Date.now();
  const prompt = host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  while (events.length === 0 && Date.now() - started < 1000) await new Promise((r) => setTimeout(r, 10));
  await host.handle("session.cancel", { sessionId: created.sessionId });
  await prompt.catch(() => undefined);
  assert.ok(Date.now() - started < 2000, "cancel must abort the in-flight generate");
  host.close();
});

test("unknown tools fail closed and secret-shaped errors are redacted", async () => {
  const secret = "sk-testsecretvalue999";
  const dataDir = await tempDir();
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: createMockProvider([providerError(new Error(`boom ${secret}`))]),
  });
  await host.handle("agentProfile.register", { name: "broken", tools: ["nope"] });
  await assert.rejects(
    () => host.handle("session.new", { profile: "broken", provider: "mock", model: "demo" }),
    /Unknown tool: nope/,
  );
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  try {
    await host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
    assert.fail("expected prompt failure");
  } catch (error) {
    const mapped = host.redactError(error);
    assert.equal(mapped.message.includes(secret), false);
    assert.match(mapped.message, /REDACTED/);
  }
  const stored = (await host.handle("credential.put", {
    provider: "mock",
    secret,
  })) as { stored: boolean; secret?: string };
  assert.equal(stored.stored, true);
  assert.equal("secret" in stored, false);
  host.close();
});

test("unreadable vault fails closed", async () => {
  const dataDir = await tempDir();
  await writeFile(join(dataDir, "credentials.vault"), "{not-a-vault}", { mode: 0o600 });
  await assert.rejects(() => ClayAgentHost.create({ dataDir, passphrase: "pass-phrase-ok", mock: true }));
});

test("daemon process exits non-zero on unreadable vault", async () => {
  const dataDir = await tempDir();
  await writeFile(join(dataDir, "credentials.vault"), "{not-a-vault}", { mode: 0o600 });
  const main = join(dirname(fileURLToPath(import.meta.url)), "../main.js");
  const child = spawn(process.execPath, [main, "--data-dir", dataDir, "--mock"], { stdio: ["pipe", "pipe", "pipe"] });
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id: 1, method: "initialize", params: { passphrase: "pass-phrase-ok" } })}\n`);
  child.stdin.end();
  const code = await new Promise<number | null>((resolve) => child.on("exit", resolve));
  assert.equal(code, 1);
});
