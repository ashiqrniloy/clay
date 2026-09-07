// Plan 109 I9: /resume — workspace-scoped session list with full restore.
// The list surface (`session.resumable`) scopes by the server-supplied
// workspace root, never lists across workspaces, and stays bounded and
// most-recent first. The resume drill restarts the host so `ensureLive`
// restores from record metadata: transcript, live context, and the
// session's persisted provider/model (the Rust resume path writes those
// into the per-workspace book).
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-resume-"));
}

interface ResumableSession {
  sessionId: string;
  updatedAt?: string;
  label?: string;
  summary?: string;
}

test("resumable list is workspace-scoped, most-recent first, and bounded", async () => {
  const dataDir = await tempDir();
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: () => {},
  });
  try {
    await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
    const alpha1 = (await host.handle("session.new", {
      profile: "chat",
      provider: "mock",
      model: "demo",
      workspaceRoot: "/ws/alpha",
    })) as { sessionId: string };
    const alpha2 = (await host.handle("session.new", {
      profile: "chat",
      provider: "mock",
      model: "demo",
      workspaceRoot: "/ws/alpha",
    })) as { sessionId: string };
    const beta = (await host.handle("session.new", {
      profile: "chat",
      provider: "mock",
      model: "demo",
      workspaceRoot: "/ws/beta",
    })) as { sessionId: string };
    // A prompt bumps alpha1's updatedAt: it must sort first.
    await host.handle("session.prompt", { sessionId: alpha1.sessionId, text: "latest" });

    const page = (await host.handle("session.resumable", {
      workspaceRoot: "/ws/alpha",
    })) as { sessions: ResumableSession[]; nextCursor: string | null };
    const ids = page.sessions.map((session) => session.sessionId);
    assert.deepEqual(ids, [alpha1.sessionId, alpha2.sessionId]);
    assert.ok(!ids.includes(beta.sessionId), "cross-workspace session never appears");
    assert.ok(ids.every((id) => id.length > 0));

    // Bounded page.
    const bounded = (await host.handle("session.resumable", {
      workspaceRoot: "/ws/alpha",
      limit: 1,
    })) as { sessions: ResumableSession[] };
    assert.equal(bounded.sessions.length, 1);
    assert.equal(bounded.sessions[0]?.sessionId, alpha1.sessionId);

    // Empty workspace: empty list, not an error.
    const empty = (await host.handle("session.resumable", {
      workspaceRoot: "/ws/none",
    })) as { sessions: ResumableSession[] };
    assert.deepEqual(empty.sessions, []);
  } finally {
    host.close();
    await rm(dataDir, { recursive: true, force: true });
  }
});

test("resumable list requires a workspace root (fail closed)", async () => {
  const dataDir = await tempDir();
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: () => {},
  });
  try {
    await assert.rejects(
      host.handle("session.resumable", {}) as Promise<unknown>,
      /workspaceRoot/,
    );
    await assert.rejects(
      host.handle("session.resumable", { workspaceRoot: "" }) as Promise<unknown>,
      /workspaceRoot/,
    );
  } finally {
    host.close();
    await rm(dataDir, { recursive: true, force: true });
  }
});

test("resume drill: transcript, context, and persisted model restore across host restart", async () => {
  const dataDir = await tempDir();
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: () => {},
    registerModels: (registries) => {
      registries.models.register({ provider: "mock", model: "mini", displayName: "Mock mini" });
    },
  });
  await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws/alpha",
  })) as { sessionId: string };
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "first turn" });
  // Mid-session model switch persists into the record metadata.
  await host.handle("session.prompt", {
    sessionId: created.sessionId,
    text: "switch",
    provider: "mock",
    model: "mini",
  });
  host.close();

  const resumedHost = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: () => {},
    registerModels: (registries) => {
      registries.models.register({ provider: "mock", model: "mini", displayName: "Mock mini" });
    },
  });
  await resumedHost.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
  try {
    // The resumable list still sees the session in its workspace.
    const page = (await resumedHost.handle("session.resumable", {
      workspaceRoot: "/ws/alpha",
    })) as { sessions: ResumableSession[] };
    assert.equal(page.sessions.length, 1);
    assert.equal(page.sessions[0]?.sessionId, created.sessionId);

    // Full restore: transcript entries + the session's persisted model.
    const loaded = (await resumedHost.handle("session.load", {
      sessionId: created.sessionId,
    })) as { entries: Array<{ kind: string; message?: { role: string } }> };
    const userEntries = loaded.entries.filter(
      (entry) => entry.kind === "message" && entry.message?.role === "user",
    );
    assert.ok(userEntries.length >= 2, "both turns restored");
    const resumed = (await resumedHost.handle("session.resume", {
      sessionId: created.sessionId,
    })) as { sessionId: string; provider: string; model: string };
    assert.equal(resumed.provider, "mock");
    assert.equal(resumed.model, "mini", "persisted model applies on resume");

    // A follow-up prompt continues the resumed session with that model.
    const followUp = (await resumedHost.handle("session.prompt", {
      sessionId: created.sessionId,
      text: "continuing",
    })) as { lastEvent: string };
    assert.equal(followUp.lastEvent, "agent_finished");
    const reloaded = (await resumedHost.handle("session.load", {
      sessionId: created.sessionId,
    })) as { entries: Array<{ kind: string; message?: { role: string } }> };
    const texts = reloaded.entries.flatMap((entry) =>
      entry.kind === "message" && entry.message?.role === "user"
        ? [entry.message]
        : [],
    );
    assert.ok(texts.length >= 3, "follow-up turn appended to the restored session");
  } finally {
    resumedHost.close();
    await rm(dataDir, { recursive: true, force: true });
  }
});
