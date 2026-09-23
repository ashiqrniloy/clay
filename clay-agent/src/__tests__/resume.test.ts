// Plan 109 I9: /resume — workspace-scoped session list with full restore.
// The list surface (`session.resumable`) scopes by the server-supplied
// workspace root, never lists across workspaces, and stays bounded and
// most-recent first. The resume drill restarts the host so `ensureLive`
// restores from record metadata: transcript, live context, and the
// session's persisted provider/model (the Rust resume path writes those
// into the per-workspace book).
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "bun:test";
import assert from "node:assert/strict";
import { providerDone, providerTextDelta, providerToolCall, toolCallContent, type AIProvider } from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-resume-"));
}

interface ResumableSession {
  sessionId: string;
  updatedAt?: string;
  updatedAtLabel?: string;
  label?: string;
  summary?: string;
}

// Plan 117 follow-up: `/resume` rows were unidentifiable — the picker printed
// the profile (identical for every session in a workspace, empty for the
// resumable list) and a raw UTC ISO stamp. The row identity is the session's
// opening prompt (first five words, stamped on the first entry by the store
// seam) plus a local-time last-active stamp.
test("resumable rows carry the opening prompt label and a local last-active stamp", async () => {
  const dataDir = await tempDir();
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: () => {},
  });
  try {
    await host.handle("agentProfile.register", { name: "chat", instructions: "Be brief." });
    const first = (await host.handle("session.new", {
      profile: "chat",
      provider: "mock",
      model: "demo",
      workspaceRoot: "/ws/label",
    })) as { sessionId: string };
    await host.handle("session.prompt", {
      sessionId: first.sessionId,
      text: "How does the resume list identify  a  session?",
    });
    // A second prompt must not re-label the session: the opening words are the
    // session's identity, so the label stays put for its whole life.
    await host.handle("session.prompt", { sessionId: first.sessionId, text: "And the stamp?" });
    const untitled = (await host.handle("session.new", {
      profile: "chat",
      provider: "mock",
      model: "demo",
      workspaceRoot: "/ws/label",
    })) as { sessionId: string };

    const page = (await host.handle("session.resumable", {
      workspaceRoot: "/ws/label",
    })) as { sessions: ResumableSession[] };
    const labelled = page.sessions.find((row) => row.sessionId === first.sessionId);
    assert.equal(labelled?.label, "How does the resume list");
    const row = page.sessions.find((entry) => entry.sessionId === untitled.sessionId);
    assert.ok(!row?.label, "a never-prompted session has no label to show");
    // Local `YYYY-MM-DD HH:MM` beside the raw ISO, so the picker can print a
    // time that matches the user's clock instead of UTC.
    assert.match(labelled?.updatedAtLabel ?? "", /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/);
    assert.ok(labelled?.updatedAt?.endsWith("Z"), "the ISO stamp is unchanged");
  } finally {
    host.close();
    await rm(dataDir, { recursive: true, force: true });
  }
});

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
    // Prism orders by updated_at (millisecond resolution) with a random
    // session-id tie-break, so make sure the prompt lands in a later
    // millisecond than the creations it must outrank.
    await new Promise((resolve) => setTimeout(resolve, 5));
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

/** Provider: writes one relative path, then finishes. The path is relative on
 *  purpose — it is the session's tool cwd that decides where it lands. */
function writeOnceProvider(relativePath: string): AIProvider {
  let turns = 0;
  return {
    id: "mock",
    async *generate() {
      turns += 1;
      if (turns === 1) {
        yield providerToolCall(toolCallContent("w1", "write", { path: relativePath, content: "resumed\n" }));
        yield providerDone();
      } else {
        yield providerTextDelta("written");
        yield providerDone();
      }
    },
  };
}

function liveToolNames(host: ClayAgentHost, sessionId: string): string[] {
  const live = (host as unknown as { live: Map<string, { tools: Array<{ name: string }> }> }).live.get(sessionId);
  return (live?.tools ?? []).map((tool) => tool.name);
}

/** The live records' full-autonomy flag (what the run loop gates on). */
function liveAutonomy(host: ClayAgentHost, sessionId: string): boolean | undefined {
  const live = (host as unknown as { live: Map<string, { fullAutonomy?: boolean }> }).live.get(sessionId);
  return live?.fullAutonomy;
}

/**
 * Plan 130 A2: a resumed session's tools come from the workspace root the
 * session was created in — the key recorded at `session.new` — never the
 * restarted daemon's launch cwd. Tool cwd and acceptance roots are the same
 * value (one `workspaceRoot`), so a relative write proves the whole binding,
 * and the graft tools prove the workspace's binding was re-established for
 * that recorded root (a restarted daemon has none).
 */
test("resumed session binds its tools to the recorded workspace root, not the daemon cwd", async () => {
  const dataDir = await tempDir();
  const recordedRoot = await tempDir();
  const launchDir = await tempDir();
  const configRoot = await tempDir();
  const homeSkillsRoot = await tempDir();
  const mcpFixture = join(process.cwd(), "src", "__tests__", "mcp-fixture-server.mjs");
  const graftStub = join(recordedRoot, "graft-stub.sh");
  await writeFile(graftStub, "#!/usr/bin/env bash\nprintf '{}\\n'\n", { mode: 0o755 });
  await chmod(graftStub, 0o755);

  const hostOptions = {
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: () => {},
    agentConfigRoot: configRoot,
    homeSkillsRoot,
    graftCliPath: graftStub,
    resolveObscuraBinary: () => undefined,
    // Plan 130 A2: a coding session's MCP bridge is part of the same lazy
    // activation resume has to redo, so the fixture server is allow-listed
    // exactly as the Rust server allow-lists a real one.
    mcpAllowList: [{ serverId: "live", command: process.execPath, args: [mcpFixture] }],
  } as const;

  const first = await ClayAgentHost.create({ ...hostOptions });
  let sessionId: string;
  try {
    await first.handle("agentProfile.register", { name: "coding", instructions: "Code.", tools: ["read", "write"] });
    const created = (await first.handle("session.new", {
      profile: "coding",
      provider: "mock",
      model: "demo",
      workspaceRoot: recordedRoot,
    })) as { sessionId: string };
    sessionId = created.sessionId;
  } finally {
    first.close();
  }

  // The daemon that resumes runs in a different directory than the session's
  // root: without the recorded key, `process.cwd()` would be the tool cwd.
  const previousCwd = process.cwd();
  const writes: string[] = [];
  let hostRef: ClayAgentHost | undefined;
  let resumedHost: ClayAgentHost | undefined;
  try {
    process.chdir(launchDir);
    resumedHost = await ClayAgentHost.create({
      ...hostOptions,
      mockProvider: writeOnceProvider("resumed.txt"),
      emit: (method, params) => {
        if (method !== "reverse") return;
        const frame = params as { id: number; method: string; params?: Record<string, unknown> };
        const answer = (): unknown => {
          if (frame.method === "document.write") {
            writes.push(String(frame.params?.path));
            return { version: 1, saved: true, open: false };
          }
          if (frame.method === "document.mkdir") return {};
          throw new Error(`unexpected reverse method: ${frame.method}`);
        };
        try {
          hostRef!.resolveReverse(frame.id, { result: answer() });
        } catch (error) {
          hostRef!.resolveReverse(frame.id, { error: { code: -32000, message: String(error) } });
        }
      },
    });
    hostRef = resumedHost;
    await resumedHost.handle("agentProfile.register", { name: "coding", instructions: "Code.", tools: ["read", "write"] });
    const resumed = (await resumedHost.handle("session.resume", { sessionId })) as { workspaceRoot: string };
    assert.equal(resumed.workspaceRoot, recordedRoot, "resume reports the recorded root, not the cwd");

    // The restarted daemon re-activates the workspace's coding surfaces for
    // that root: the graft binding (with its tool) and the MCP bridge.
    const resumedTools = liveToolNames(resumedHost, sessionId);
    assert.ok(
      resumedTools.includes("graft_ask"),
      "the resumed coding session re-binds graft for its recorded workspace",
    );
    assert.ok(
      resumedTools.some((name) => name.startsWith("mcp:live:")),
      `the resumed coding session re-connects its MCP allow-list: ${resumedTools.join(", ")}`,
    );

    // Decision 2026-09-20-2049: the resumed session restores the autonomy
    // recorded with it — the live record the run loop gates on, not a
    // hardcoded value — and a session created without the flag records the
    // opt-out default (on). So the run streams through with no approval round
    // trip and no `session.setAutonomy` call after the restart.
    assert.equal(
      liveAutonomy(resumedHost, sessionId),
      true,
      "a resumed session keeps the recorded autonomy (on by default)",
    );
    const run = (await resumedHost.handle("session.prompt", { sessionId, text: "write it" })) as { lastEvent: string };
    assert.equal(run.lastEvent, "agent_finished");
    assert.deepEqual(
      writes,
      [join(recordedRoot, "resumed.txt")],
      "the relative write resolved against the recorded workspace root",
    );
  } finally {
    resumedHost?.close();
    process.chdir(previousCwd);
    await rm(dataDir, { recursive: true, force: true });
    await rm(recordedRoot, { recursive: true, force: true });
    await rm(launchDir, { recursive: true, force: true });
  }
});

/**
 * Decision 2026-09-20-2049, the other direction: a caller who blocks autonomy
 * is not overridden by a daemon restart. The session is created with
 * `fullAutonomy: false`, which both gates the run before the restart and must
 * survive it — a resumed session whose live record re-armed autonomy would
 * silently drop the user's block.
 */
test("resumed session keeps a recorded autonomy block (approvals stay armed)", async () => {
  const dataDir = await tempDir();
  const workspace = await tempDir();
  const hostOptions = {
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    emit: () => {},
    resolveObscuraBinary: () => undefined,
  } as const;

  const first = await ClayAgentHost.create({ ...hostOptions });
  let sessionId: string;
  try {
    await first.handle("agentProfile.register", { name: "coding", instructions: "Code.", tools: ["read", "write"] });
    const created = (await first.handle("session.new", {
      profile: "coding",
      provider: "mock",
      model: "demo",
      workspaceRoot: workspace,
      fullAutonomy: false,
    })) as { sessionId: string; fullAutonomy: boolean };
    assert.equal(created.fullAutonomy, false, "session.new honours the explicit block");
    sessionId = created.sessionId;
  } finally {
    first.close();
  }

  const resumedHost = await ClayAgentHost.create({ ...hostOptions });
  try {
    await resumedHost.handle("agentProfile.register", { name: "coding", instructions: "Code.", tools: ["read", "write"] });
    await resumedHost.handle("session.resume", { sessionId });
    assert.equal(
      liveAutonomy(resumedHost, sessionId),
      false,
      "the recorded block survives the restart",
    );
  } finally {
    resumedHost.close();
    await rm(dataDir, { recursive: true, force: true });
    await rm(workspace, { recursive: true, force: true });
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
