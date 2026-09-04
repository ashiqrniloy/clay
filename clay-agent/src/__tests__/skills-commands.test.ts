import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  type AIProvider,
  type ProviderRequest,
  type ToolExecutionContext,
  createLoadedSkillSet,
  createLoadSkillTool,
  createSkillRegistry,
  providerDone,
  providerTextDelta,
  providerToolCall,
  toolCallContent,
} from "@arnilo/prism";
import { ClayAgentHost } from "../host.js";

async function tempDir(): Promise<string> {
  return mkdtemp(join(tmpdir(), "clay-agent-skills-"));
}

function context(sessionId = "s1"): ToolExecutionContext {
  return {
    sessionId,
    runId: "r1",
    toolCallId: "c1",
    metadata: {},
  } as unknown as ToolExecutionContext;
}

test("skill.register stores on kernel; skill.list is catalog-only until load_skill", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  const registered = (await host.handle("skill.register", {
    name: "brief",
    description: "Answer tersely.",
    instructions: "Keep every answer under ten words.",
  })) as { name: string; registered: boolean };
  assert.equal(registered.registered, true);
  const catalog = (await host.handle("skill.list", {})) as { skills: Array<{ name: string; instructions?: string }> };
  assert.equal(catalog.skills.length, 1);
  assert.equal(catalog.skills[0]!.name, "brief");
  assert.equal(catalog.skills[0]!.instructions, undefined, "catalog must not leak full instructions");

  // load_skill progressive disclosure pulls the body into the loaded set.
  const registry = createSkillRegistry(
    [{ name: "brief", description: "Answer tersely.", instructions: "Keep every answer under ten words." }],
    { duplicate: "error" },
  );
  const loaded = createLoadedSkillSet();
  const tool = createLoadSkillTool({ registry, tools: [] });
  const result = await tool.execute(
    { name: "brief" },
    { ...context(), metadata: { loadedSkills: loaded, activeToolNames: [] } },
  );
  assert.equal((result.value as { ok?: boolean }).ok, true);
  assert.ok(loaded.list().includes("brief"));
  host.close();
});

test("skill.register duplicate name fails closed", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await host.handle("skill.register", { name: "brief", description: "Be brief." });
  await assert.rejects(() => host.handle("skill.register", { name: "brief", description: "Again" }), /duplicate/i);
  host.close();
});

test("skill referencing an inactive tool throws before a provider turn", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await host.handle("skill.register", {
    name: "needs-shell",
    description: "Runs commands.",
    instructions: "Use the shell tool.",
    toolNames: ["shell"],
  });
  await host.handle("agentProfile.register", { name: "codeless", skills: ["needs-shell"] });
  await assert.rejects(
    () =>
      host.handle("session.new", {
        profile: "codeless",
        provider: "mock",
        model: "demo",
      }),
    /requires inactive tool/i,
  );
  host.close();
});

test("profile with an active skill gets the load_skill tool; catalog stays progressive", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await host.handle("skill.register", { name: "brief", description: "Be brief.", instructions: "Ten words max." });
  await host.handle("agentProfile.register", { name: "skilled", skills: ["brief"] });
  const created = (await host.handle("session.new", {
    profile: "skilled",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string; tools: string[] };
  assert.ok(created.tools.includes("load_skill"), "active skills host the load_skill tool");
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "Hi" });
  host.close();
});

test("command.register + dispatch a /steer-like command calls host drivers.steer", async () => {
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
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
  });
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  await host.handle("command.register", {
    name: "steer",
    handler: "steer",
    description: "Steer the active run.",
    parameters: { input: { type: "string" } },
  });
  const prompt = host.handle("session.prompt", { sessionId: created.sessionId, text: "go" });
  await new Promise((r) => setTimeout(r, 50)); // let the run go active
  const dispatched = (await host.handle("command.dispatch", {
    name: "steer",
    sessionId: created.sessionId,
    args: { runId: "r1", input: "go slower" },
  })) as { value: { steered: boolean; input: string } };
  assert.equal(dispatched.value.steered, true);
  assert.equal(dispatched.value.input, "go slower");
  await host.handle("session.cancel", { sessionId: created.sessionId });
  await prompt.catch(() => undefined);
  host.close();
});

test("dispatch a command that wants startWorkflow gets the Phase 5 error", async () => {
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
  await host.handle("command.register", { name: "workflow", handler: "startWorkflow" });
  await assert.rejects(
    () =>
      host.handle("command.dispatch", {
        name: "workflow",
        sessionId: created.sessionId,
        args: { definition: { kind: "x" }, input: "go" },
      }),
    /workflows not enabled|Phase 5/i,
  );
  host.close();
});

test("RPC cannot inject drivers; unknown command fails closed", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await assert.rejects(() => host.handle("command.dispatch", { name: "nope", args: {} }), /unknown command/i);
  // A `drivers` field in register params is inert data — ignored, not stored.
  const registered = (await host.handle("command.register", {
    name: "hax",
    handler: "steer",
    drivers: { steer: () => ({ hacked: true }) },
  })) as { registered: boolean };
  assert.equal(registered.registered, true);
  // Drivers never came from RPC: dispatch without a live session hits the fail-closed path.
  await assert.rejects(
    () => host.handle("command.dispatch", { name: "hax", args: { input: "x" } }),
    /requires an active session|drivers absent/i,
  );
  const catalog = (await host.handle("skill.list", {})) as { skills: unknown[] };
  assert.deepEqual(catalog.skills, []);
  host.close();
});

test("command dispatch without a live session omits drivers and fails closed", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await host.handle("command.register", { name: "steer", handler: "steer" });
  await assert.rejects(
    () => host.handle("command.dispatch", { name: "steer", args: { input: "x" } }),
    /requires an active session|drivers absent/i,
  );
  host.close();
});

// ---- Phase 2 task 7: slash-command surface (pi parity) ----

/** Standalone harness: stub the daemon→server reverse channel (checkpoint and
 *  document backends live in the Rust server, absent here). */
function stubReverse(
  host: ClayAgentHost,
  overrides: Record<string, (params: Record<string, unknown>) => unknown> = {},
): void {
  (host as unknown as {
    request: (method: string, params: Record<string, unknown>) => Promise<unknown>;
  }).request = async (method, params) => {
    const override = overrides[method];
    if (override) return override(params);
    if (method.startsWith("checkpoint.")) return {};
    if (method === "document.stat") return { size: 0 };
    throw new Error(`no reverse backend for ${method}`);
  };
}

/** Register the Coding Agent's slash surface exactly as its load entry does. */
async function registerSlashCommands(host: ClayAgentHost): Promise<void> {
  for (const command of [
    { name: "/compact", handler: "compact", description: "Compact the current session manually." },
    { name: "/new", handler: "newSession", description: "Start a fresh session." },
    { name: "/n", handler: "newSession", description: "Alias of /new." },
    { name: "/branch", handler: "checkout", description: "Checkout a branch point." },
    { name: "/tree", handler: "tree", description: "Show the branch tree." },
    { name: "/fork", handler: "forkSession", description: "Fork the session." },
    { name: "/clone", handler: "cloneSession", description: "Clone the session." },
    { name: "/open-session", handler: "openSession", description: "Open a session by id." },
    { name: "/open-session-as-fork", handler: "openSessionAsFork", description: "Open as fork." },
  ]) {
    await host.handle("command.register", command);
  }
}

test("slash commands reach their daemon RPCs: compact, new, tree, fork, clone, checkout", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await registerSlashCommands(host);
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  const sessionId = created.sessionId;
  stubReverse(host);

  // Prompt text makes the session live (and appends a user entry for the tree).
  await host.handle("session.prompt", { sessionId, text: "hello" });

  // /tree: branch summary with ids and kinds.
  const tree = (await host.handle("command.dispatch", { name: "/tree", sessionId, args: {} })) as {
    value: { sessionId: string; entries: Array<{ id: string; kind: string }> };
  };
  assert.equal(tree.value.sessionId, sessionId);
  assert.ok(tree.value.entries.length >= 1, "tree lists entries");
  assert.ok(tree.value.entries.every((entry) => entry.id && entry.kind));

  // /compact: manual compaction runs on the live session.
  const compacted = (await host.handle("command.dispatch", { name: "/compact", sessionId, args: {} })) as {
    value: { sessionId: string; strategy: string };
  };
  assert.equal(compacted.value.sessionId, sessionId);

  // /branch: checkout an entry id (the tree's first entry). The leaf re-roots
  // on the appended branch summary (plan 108 task 10), not the raw entry.
  const entryId = tree.value.entries[0]!.id;
  const checkedOut = (await host.handle("command.dispatch", {
    name: "/branch",
    sessionId,
    args: { entryId },
  })) as { value: { sessionId: string; leafId: string; summaryEntryId?: string } };
  assert.equal(checkedOut.value.sessionId, sessionId);
  assert.ok(checkedOut.value.summaryEntryId, "checkout appends the branch summary");
  assert.equal(checkedOut.value.leafId, checkedOut.value.summaryEntryId);

  // /fork keeps the id and moves the leaf; /clone gets a new id.
  const forked = (await host.handle("command.dispatch", {
    name: "/fork",
    sessionId,
    args: { entryId },
  })) as { value: { sessionId: string; leafId: string } };
  assert.equal(forked.value.sessionId, sessionId);
  const cloned = (await host.handle("command.dispatch", {
    name: "/clone",
    sessionId,
    args: { entryId },
  })) as { value: { sessionId: string } };
  assert.notEqual(cloned.value.sessionId, sessionId);

  // /new: fresh session with the same profile/provider/model.
  const fresh = (await host.handle("command.dispatch", { name: "/new", sessionId, args: {} })) as {
    value: { sessionId: string; profile: string; provider: string; model: string };
  };
  assert.notEqual(fresh.value.sessionId, sessionId);
  assert.equal(fresh.value.profile, "chat");
  assert.equal(fresh.value.provider, "mock");

  // /open-session (the --session equivalent): loads entries by id.
  const opened = (await host.handle("command.dispatch", {
    name: "/open-session",
    args: { sessionId },
  })) as { value: { sessionId: string; entries: unknown[] } };
  assert.equal(opened.value.sessionId, sessionId);
  assert.ok(opened.value.entries.length >= 1);

  // /open-session-as-fork (the --fork equivalent): resume + fork.
  const openedFork = (await host.handle("command.dispatch", {
    name: "/open-session-as-fork",
    args: { sessionId },
  })) as { value: { sessionId: string; leafId?: string } };
  assert.equal(openedFork.value.sessionId, sessionId);

  host.close();
});

test("prompt text starting with a registered slash command dispatches instead of prompting", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await registerSlashCommands(host);
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  await host.handle("session.prompt", { sessionId: created.sessionId, text: "hello" });

  // /tree in prompt position returns the tree, not an LLM turn.
  const result = (await host.handle("session.prompt", {
    sessionId: created.sessionId,
    text: "/tree",
  })) as { name?: string; value?: { entries: unknown[] } };
  assert.equal(result.name, "/tree");
  assert.ok(Array.isArray(result.value?.entries));

  // Unknown /x is not intercepted: it prompts normally (chat-safe).
  const plain = await host.handle("session.prompt", {
    sessionId: created.sessionId,
    text: "/etc/hosts what is this",
  });
  assert.ok(plain !== undefined);

  host.close();
});

test("dispatching an unknown command is a bounded typed no-op", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await registerSlashCommands(host);
  await assert.rejects(
    () => host.handle("command.dispatch", { name: "/nope", args: {} }),
    /unknown command/i,
  );
  // The registry is unchanged: the known commands still dispatch.
  await host.handle("agentProfile.register", { name: "chat" });
  const created = (await host.handle("session.new", {
    profile: "chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  const tree = await host.handle("command.dispatch", { name: "/tree", sessionId: created.sessionId, args: {} });
  assert.ok(tree !== undefined);
  host.close();
});

test("session-scoped slash commands fail closed without a session", async () => {
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
  });
  await registerSlashCommands(host);
  await assert.rejects(
    () => host.handle("command.dispatch", { name: "/compact", args: {} }),
    /requires an active session/i,
  );
  host.close();
});

// ---- Phase 2 task 7: create-plan skill run (plan files + coding checkpoints) ----

test("create-plan skill run writes a plans/ doc with parseable todos and a checkpoint artifact", async () => {
  const { createCodingPlanMarkdown, codingPlanPathForTask, readCodingPlanFile, buildCodingCheckpointMetadata, fingerprintJson } =
    await import("@arnilo/prism-coding-tools/agent");
  const { rm } = await import("node:fs/promises");

  const workspaceRoot = await tempDir();
  let turn = 0;
  const provider: AIProvider = {
    id: "mock",
    async *generate() {
      turn += 1;
      if (turn === 1) {
        // Progressive disclosure pulls the skill body first.
        yield providerToolCall(toolCallContent("c1", "load_skill", { name: "coding-agent.createPlan" }));
      } else if (turn === 2) {
        const markdown = createCodingPlanMarkdown({
          title: "Phase 2: coding agent",
          taskId: "task-1",
          todos: [
            { id: "task-1", text: "Register the coding profile" },
            { id: "task-2", text: "Verify the split surface", done: true },
          ],
        });
        yield providerToolCall(toolCallContent("c2", "write", { path: codingPlanPathForTask("task-1"), content: markdown }));
      }
      yield providerDone();
    },
  };
  const host = await ClayAgentHost.create({
    dataDir: await tempDir(),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: provider,
  });
  // Document backend stub: writes land in the temp workspace.
  const { writeFile, mkdir } = await import("node:fs/promises");
  const { dirname } = await import("node:path");
  stubReverse(host, {
    // First write into a not-yet-existing subdirectory fails realpath
    // containment and routes to host approval; the harness approves it (the
    // production server shows the approval UI for the same case).
    "approval.request": () => ({ allowed: true }),
    "document.mkdir": async (params) => {
      await mkdir(String(params.path), { recursive: true });
      return {};
    },
    "document.write": async (params) => {
      const path = String(params.path);
      if (!path.startsWith(workspaceRoot)) throw new Error("write escaped the workspace");
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, String(params.content ?? ""), "utf8");
      return { version: 1 };
    },
  });
  await host.handle("skill.register", {
    name: "coding-agent.createPlan",
    description: "Plan-file convention.",
    instructions: "Plans live under plans/ ...",
    toolNames: ["read", "write"],
  });
  await host.handle("agentProfile.register", {
    name: "coding",
    tools: ["read", "write"],
    skills: ["coding-agent.createPlan"],
  });
  const created = (await host.handle("session.new", {
    profile: "coding",
    provider: "mock",
    model: "demo",
    workspaceRoot,
  })) as { sessionId: string; tools: string[] };
  assert.ok(created.tools.includes("load_skill"), "the skill hosts load_skill");

  // Coding tool side effects suspend for approval in the durable-run flow;
  // approve each and drive the run to completion.
  type SuspendedPrompt = {
    status?: string;
    runId?: string;
    version?: number;
    interruption?: { pendingDecisions: Array<{ approvalId: string; outcome?: string }> };
  };
  let prompt = (await host.handle("session.prompt", {
    sessionId: created.sessionId,
    text: "Plan this phase.",
  })) as SuspendedPrompt;
  for (let guard = 0; prompt.status === "suspended" && guard < 8; guard += 1) {
    const decision = prompt.interruption!.pendingDecisions[0]!;
    prompt = (await host.handle("run.resume", {
      sessionId: created.sessionId,
      runId: prompt.runId,
      expectedVersion: prompt.version,
      decisions: [{ approvalId: decision.approvalId, outcome: "allow_once" }],
    })) as SuspendedPrompt;
  }
  assert.equal(prompt.status, "succeeded", "the approved skill run completes");

  // The plan file follows the convention and parses back into todos.
  const planPath = codingPlanPathForTask("task-1");
  const read = await readCodingPlanFile({ workspaceRoot, planPath });
  assert.equal(read.todos.length, 2);
  assert.equal(read.todos[0]!.id, "task-1");
  assert.equal(read.todos[0]!.done, false);
  assert.equal(read.todos[1]!.done, true);

  // Checkpoint convention: the artifact ref + metadata carry the plan.
  const checkpoint = buildCodingCheckpointMetadata({
    taskId: "task-1",
    workspaceRoot,
    baseBranch: "main",
    branch: "agent/task-1",
    planPath,
    plan: read.artifact,
    artifacts: [],
    checks: [],
    status: "editing",
    fingerprints: {
      workflowRevision: "test",
      toolFingerprint: fingerprintJson(["read", "write"]),
      policyFingerprint: fingerprintJson({ roots: [workspaceRoot] }),
    },
    todos: read.todos,
  });
  assert.equal(checkpoint.schemaVersion, 1);
  assert.equal(checkpoint.planPath, planPath);
  assert.equal(checkpoint.plan.kind, "plan");
  assert.equal(checkpoint.todos.length, 2);

  await rm(workspaceRoot, { recursive: true, force: true });
  host.close();
});
