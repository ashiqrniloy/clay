// Plan 109 I8: the Observational Memory tab's daemon surface.
//
// Activity drill: a session with OM attached runs the REAL worker
// pipeline — observe → reflect → drop — through one prompt-routed fake
// provider (session turns answer with text; observer/reflector turns,
// detected by their worker tools, answer with the corresponding tool
// calls). The activity log surfaces all three row kinds, drops included.
//
// Worker-model drill: per-session selections persist to the session
// record metadata (restored on resume by ensureLive), may differ from
// the session model, and fail closed — unknown providers are rejected
// and unset workers never reuse the session model
// (`requireExplicitModel: true` skips them).
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  providerDone,
  providerTextDelta,
  providerToolCall,
  type AIProvider,
  type ProviderEvent,
  type SessionEntry,
} from "@arnilo/prism";
import {
  foldObservationalMemoryLedger,
  foldWorkScopeMap,
  projectWorkMemory,
  recallObservationalMemory,
} from "@arnilo/prism-memory/compaction/observational-memory";
import { ClayAgentHost } from "../host.js";

interface OmActivityRow {
  id: string;
  kind: string;
  summary: string;
}

interface OmActivityView {
  sessionId: string;
  attached: boolean;
  observation: { provider: string; model: string } | null;
  reflection: { provider: string; model: string } | null;
  activity: OmActivityRow[];
}

/**
 * One prompt-routed fake provider for both the session and the OM
 * workers: plain turns answer with text; turns carrying the observer's
 * or reflector's worker tool emit the matching record call, extracting
 * the eligible entry id / observation id out of the prompt (the worker
 * prompts render as `[<id>] …` lines).
 */
function routingProvider(
  capture?: (request: { options?: { sessionId?: string } }) => void,
  options: { spawn?: boolean } = {},
): AIProvider {
  let spawnPending = options.spawn === true;
  return {
    id: "mock",
    async *generate(request: {
      messages?: unknown;
      tools?: ReadonlyArray<{ name?: string }>;
      options?: { sessionId?: string };
    }): AsyncGenerator<ProviderEvent> {
      capture?.(request);
      const toolNames = (request.tools ?? [])
        .map((tool) => tool.name)
        .join(",");
      if (toolNames.includes("record_observation")) {
        const entryId = JSON.stringify(request.messages).match(
          /\[(entry_[0-9a-f-]+)\]/,
        )?.[1];
        if (entryId) {
          yield providerToolCall({
            type: "tool_call",
            id: "o1",
            name: "record_observation",
            arguments: {
              content: "User prefers dark mode",
              sourceEntryIds: [entryId],
              relevance: "high",
            },
          });
        }
        yield providerDone();
        return;
      }
      if (toolNames.includes("record_reflection")) {
        const observationId = JSON.stringify(request.messages).match(
          /\[([0-9a-f]{12})\]/,
        )?.[1];
        if (observationId) {
          yield providerToolCall({
            type: "tool_call",
            id: "r1",
            name: "record_reflection",
            arguments: {
              content: "User values dark mode",
              supportingObservationIds: [observationId],
            },
          });
        }
        yield providerDone();
        return;
      }
      // Plan 123 task 2: one delegation on the first session turn; the child
      // (coding tools, no spawn tools) falls through to a plain text turn.
      if (spawnPending && toolNames.includes("spawn_agent")) {
        spawnPending = false;
        yield providerToolCall({
          type: "tool_call",
          id: "s1",
          name: "spawn_agent",
          arguments: { childId: "test", input: "child work" },
        });
        yield providerDone();
        return;
      }
      yield providerTextDelta("answer");
      yield providerDone();
    },
  };
}

async function hostWithOm() {
  const dataDir = await mkdtemp(join(tmpdir(), "clay-agent-om-"));
  // Plan 113: workers resolve the registry provider — the top-level
  // mockProvider — so capture there to assert the kernel's om:
  // correlation ids.
  const workerRequests: Array<{ options?: { sessionId?: string } }> = [];
  const host = await ClayAgentHost.create({
    dataDir,
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: routingProvider((r) => workerRequests.push(r)),
    emit: () => {},
    // Tiny worker thresholds so one short run drives the full
    // observe → reflect → drop pipeline; the deterministic lowest-relevance
    // dropper drops without a model call.
    observationalMemory: {
      observation: { provider: routingProvider(), messageTokens: 1 },
      reflection: { provider: routingProvider(), observationTokens: 1 },
      dropper: { targetTokens: 1, policy: "lowest-relevance" },
    },
  });
  await host.handle("agentProfile.register", {
    name: "Om",
    description: "OM fixture",
    observationalMemory: true,
  });
  const created = (await host.handle("session.new", {
    profile: "Om",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  // Workers run only with an explicit worker model
  // (`requireExplicitModel: true` skips them otherwise) — the drill
  // selects the registered provider for both workers, which is exactly
  // the I8 selection surface.
  await host.handle("session.om.set", {
    sessionId: created.sessionId,
    workers: {
      observation: { provider: "mock", model: "demo" },
      reflection: { provider: "mock", model: "demo" },
    },
  });
  return { host, sessionId: created.sessionId, workerRequests };
}

test("om activity drill: observe → reflect → drop all surface (drops included)", async () => {
  const { host, sessionId, workerRequests } = await hostWithOm();
  try {
    await host.handle("session.prompt", { sessionId, text: "hello" });
    // Give the post-run worker flush a beat to settle.
    for (let attempt = 0; attempt < 40; attempt += 1) {
      const view = (await host.handle("session.om.activity", {
        sessionId,
      })) as OmActivityView;
      if (view.activity.some((row) => row.kind === "drop")) break;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    const view = (await host.handle("session.om.activity", {
      sessionId,
    })) as OmActivityView;
    assert.equal(view.attached, true);

    const observation = view.activity.find((row) => row.kind === "observation");
    assert.ok(observation, "recorded observation row missing");
    assert.match(observation.summary, /User prefers dark mode/);

    const reflection = view.activity.find((row) => row.kind === "reflection");
    assert.ok(reflection, "recorded reflection row missing");
    assert.match(reflection.summary, /User values dark mode/);

    const drop = view.activity.find((row) => row.kind === "drop");
    assert.ok(drop, "dropped-observation row missing (must be visible)");
    assert.match(drop.summary, /Dropped \d+ observation/);

    // Plan 113: kernel-derived OM worker correlation (decision
    // 2026-09-07-2149) — worker generate options carry om:{session.id}.
    // The capture also holds the bare-session main prompt, so assert
    // existence of an om:-prefixed request rather than uniformity.
    assert.ok(
      workerRequests.some((request) => /^om:/.test(request.options?.sessionId ?? "")),
      `expected an om:-prefixed worker request, got: ${JSON.stringify(
        workerRequests.map((request) => request.options?.sessionId),
      )}`,
    );
  } finally {
    await host.handle("session.delete", { sessionId });
  }
});

test("om model retention: per-session selection persists to record metadata and restores on resume", async () => {
  const { host, sessionId } = await hostWithOm();
  try {
    const applied = (await host.handle("session.om.set", {
      sessionId,
      workers: {
        observation: { provider: "mock", model: "worker-obs" },
        reflection: { provider: "mock", model: "worker-ref" },
      },
    })) as { observation: { provider: string; model: string } };
    assert.deepEqual(applied.observation, { provider: "mock", model: "worker-obs" });

    // The record metadata carries the selection (resume restores it via
    // ensureLive); the live selection is reported by the activity read.
    const listed = (await host.handle("session.list", {})) as {
      sessions: Array<{ id: string; metadata?: Record<string, unknown> }>;
    };
    const record = listed.sessions.find((session) => session.id === sessionId);
    assert.deepEqual(record?.metadata?.omWorkers, {
      observation: { provider: "mock", model: "worker-obs" },
      reflection: { provider: "mock", model: "worker-ref" },
    });
    const view = (await host.handle("session.om.activity", {
      sessionId,
    })) as OmActivityView;
    assert.deepEqual(view.observation, { provider: "mock", model: "worker-obs" });
    assert.deepEqual(view.reflection, { provider: "mock", model: "worker-ref" });
  } finally {
    await host.handle("session.delete", { sessionId });
  }
});

test("om activity: unattached sessions report attached false with an empty log", async () => {
  const { host } = await hostWithOm();
  const created = (await host.handle("session.new", {
    profile: "Chat",
    provider: "mock",
    model: "demo",
  })) as { sessionId: string };
  try {
    const view = (await host.handle("session.om.activity", {
      sessionId: created.sessionId,
    })) as OmActivityView;
    assert.equal(view.attached, false);
    assert.deepEqual(view.activity, []);
  } finally {
    await host.handle("session.delete", { sessionId: created.sessionId });
  }
});

test("om fail-closed: unknown worker provider rejected; malformed shapes rejected", async () => {
  const { host, sessionId } = await hostWithOm();
  try {
    await assert.rejects(
      host.handle("session.om.set", {
        sessionId,
        workers: { observation: { provider: "nope", model: "x" } },
      }),
      /Unknown OM worker provider/,
    );
    await assert.rejects(
      host.handle("session.om.set", {
        sessionId,
        workers: { observation: { provider: "mock", model: "" } },
      }),
      /must be non-empty strings/,
    );
    await assert.rejects(
      host.handle("session.om.set", { sessionId, workers: "nope" }),
      /must be an object/,
    );
  } finally {
    await host.handle("session.delete", { sessionId });
  }
});

// ---------------------------------------------------------------------------
// Plan 123: per-prompt work scopes on OM coding runs. Chat / OM-off sessions
// never build a controller; the run scope is host-generated, redacted by the
// controller's secrets, and exact-id recall stays branch-wide.

const CODING_TOOLS = [
  "shell",
  "read",
  "write",
  "edit",
  "repo_list",
  "repo_search",
  "glob",
  "delete",
  "move",
];

interface ScopeData {
  type?: string;
  id?: string;
  scopeId?: string;
  kind?: string;
  parentId?: string;
  label?: string;
}

function scopeData(entry: { data?: unknown }): ScopeData | undefined {
  return entry.data as ScopeData | undefined;
}

/** Coding profile + OM workers: the only shape that scopes a prompt.
 *  `spawn` makes the first session turn delegate to a `test` child. */
async function codingOmHost(options: { spawn?: boolean } = {}): Promise<ClayAgentHost> {
  const host = await ClayAgentHost.create({
    dataDir: await mkdtemp(join(tmpdir(), "clay-agent-om-scope-")),
    passphrase: "pass-phrase-ok",
    mock: true,
    mockProvider: routingProvider(undefined, options),
    emit: () => {},
    // Hermetic agent config: no ambient MCP/capability tools or skills.
    agentConfigRoot: await mkdtemp(join(tmpdir(), "clay-agent-om-scope-cfg-")),
    observationalMemory: {
      observation: { provider: routingProvider(), messageTokens: 1 },
      reflection: { provider: routingProvider(), observationTokens: 1 },
    },
  });
  await host.handle("agentProfile.register", {
    name: "OmCoding",
    description: "OM coding fixture",
    tools: CODING_TOOLS,
    observationalMemory: true,
  });
  return host;
}

async function codingOmSession(
  host: ClayAgentHost,
  options: { id?: string; observationalMemory?: boolean } = {},
): Promise<string> {
  const created = (await host.handle("session.new", {
    profile: "OmCoding",
    provider: "mock",
    model: "demo",
    workspaceRoot: "/ws",
    ...(options.id ? { id: options.id } : {}),
    ...(options.observationalMemory === false ? { observationalMemory: false } : {}),
  })) as { sessionId: string };
  if (options.observationalMemory !== false) {
    await host.handle("session.om.set", {
      sessionId: created.sessionId,
      workers: {
        observation: { provider: "mock", model: "demo" },
        reflection: { provider: "mock", model: "demo" },
      },
    });
  }
  return created.sessionId;
}

async function loadLedger(host: ClayAgentHost, sessionId: string): Promise<SessionEntry[]> {
  const loaded = (await host.handle("session.load", { sessionId })) as { entries: SessionEntry[] };
  return loaded.entries;
}

function scopeTypes(entries: readonly SessionEntry[]): string[] {
  return entries
    .map((entry) => scopeData(entry)?.type)
    .filter((type): type is string => typeof type === "string" && type.startsWith("om.scope."));
}

test("om work scope: OM-on coding prompt opens, enters, and leaves one run scope", async () => {
  const host = await codingOmHost();
  const sessionId = await codingOmSession(host);
  try {
    await host.handle("session.prompt", { sessionId, text: "hello" });
    const entries = await loadLedger(host, sessionId);
    const types = scopeTypes(entries);
    assert.equal(types[0], "om.scope.opened", `scope entries: ${types.join(",")}`);
    assert.ok(types.includes("om.scope.entered"), `entered missing: ${types.join(",")}`);
    // withWorkScope always leaves in finally and never closes.
    assert.equal(types.at(-1), "om.scope.left");
    assert.ok(!types.includes("om.scope.closed"), "scopes stay open for projection");
    const opened = entries.find((entry) => scopeData(entry)?.type === "om.scope.opened");
    assert.equal(opened && scopeData(opened)?.id, `run:${sessionId}:1`);
    assert.equal(opened && scopeData(opened)?.kind, "run");
    // The run's own observation auto-binds to the run scope, not the session
    // root — the per-run projection owns it.
    const bound = entries.find((entry) => scopeData(entry)?.type === "om.scope.bound");
    assert.equal(bound && scopeData(bound)?.scopeId, `run:${sessionId}:1`);
  } finally {
    await host.handle("session.delete", { sessionId });
    host.close();
  }
});

test("om work scope: OM-off coding prompt writes zero scope entries", async () => {
  const host = await codingOmHost();
  const sessionId = await codingOmSession(host, { observationalMemory: false });
  try {
    await host.handle("session.prompt", { sessionId, text: "hello" });
    const entries = await loadLedger(host, sessionId);
    assert.deepEqual(scopeTypes(entries), []);
    assert.ok(entries.some((entry) => entry.kind === "message"), "the run itself must still happen");
  } finally {
    await host.handle("session.delete", { sessionId });
    host.close();
  }
});

test("om work scope: an invalid scope id fails closed before the prompt starts", async () => {
  const host = await codingOmHost();
  const sessionId = await codingOmSession(host, { id: "a..b" });
  try {
    await assert.rejects(
      host.handle("session.prompt", { sessionId, text: "hello" }),
      /Invalid work scope/,
    );
    const entries = await loadLedger(host, sessionId);
    assert.deepEqual(scopeTypes(entries), []);
    assert.ok(!entries.some((entry) => entry.kind === "message"), "the run must not start");
  } finally {
    await host.handle("session.delete", { sessionId });
    host.close();
  }
});

test("om work scope: per-run projection stays separate while exact-id recall sees the whole branch", async () => {
  const host = await codingOmHost();
  const sessionId = await codingOmSession(host);
  try {
    await host.handle("session.prompt", { sessionId, text: "one" });
    await host.handle("session.prompt", { sessionId, text: "two" });
    const entries = await loadLedger(host, sessionId);
    const ledger = foldObservationalMemoryLedger(entries);
    const scopes = foldWorkScopeMap(entries);
    const runIds = [...scopes.scopes.keys()].filter((id) => id.startsWith(`run:${sessionId}:`));
    assert.deepEqual(runIds, [`run:${sessionId}:1`, `run:${sessionId}:2`]);
    const first = projectWorkMemory(ledger, scopes, { from: runIds[0]!, include: "self" });
    const second = projectWorkMemory(ledger, scopes, { from: runIds[1]!, include: "self+ancestors" });
    assert.equal(first.observations.length, 1, "run 1 owns its observation");
    assert.equal(second.observations.length, 1, "run 2 owns its observation");
    const earlierId = first.observations[0]!.id;
    assert.notEqual(earlierId, second.observations[0]!.id);
    assert.ok(
      !second.observations.some((observation) => observation.id === earlierId),
      "run 1's observation is not in run 2's working set",
    );
    // Exact-id recall is unfiltered: the full branch still serves run 1's id.
    const recalled = recallObservationalMemory(entries, earlierId);
    assert.equal(recalled.found, true);
    assert.match(recalled.text, /dark mode/);
  } finally {
    await host.handle("session.delete", { sessionId });
    host.close();
  }
});

// Plan 123 task 2: a `test`/`validation` delegation nests as a `child` scope
// under the run scope that owns it — one open/close ledger pair, never
// entered (the ledger's enter/leave stack is session-global, so a child
// holding it would capture the parent run's own flush), and closed so the
// default leaf+ancestors projection stays the run's own working set.
test("om work scope: a delegation nests a closed child scope under the run scope", async () => {
  const host = await codingOmHost({ spawn: true });
  const sessionId = await codingOmSession(host);
  try {
    await host.handle("session.prompt", { sessionId, text: "spawn a child" });
    const entries = await loadLedger(host, sessionId);
    const opened = entries.filter((entry) => scopeData(entry)?.type === "om.scope.opened");
    assert.equal(scopeData(opened[0]!)?.id, `run:${sessionId}:1`, "the prompt scopes the run first");
    const child = opened.find((entry) => scopeData(entry)?.kind === "child");
    assert.ok(child, `child scope missing: ${JSON.stringify(opened.map(scopeData))}`);
    const childScopeId = scopeData(child)?.id;
    assert.equal(childScopeId, `child:${sessionId}:1`);
    assert.equal(scopeData(child)?.parentId, `run:${sessionId}:1`, "the run scope owns the delegation");
    assert.equal(scopeData(child)?.label, "test", "the child id labels the scope");
    // Closed on settle: one pair per spawn, and never a projection leaf.
    const closes = entries.filter(
      (entry) => scopeData(entry)?.type === "om.scope.closed" && scopeData(entry)?.scopeId === childScopeId,
    );
    assert.equal(closes.length, 1, "exactly one close for the child scope");
    const entered = entries
      .filter((entry) => scopeData(entry)?.type === "om.scope.entered")
      .map((entry) => scopeData(entry)?.scopeId);
    assert.deepEqual(entered, [`run:${sessionId}:1`], "only the run scope is ever entered");
    // Depth from the session root: session → run → child (Prism caps at 8).
    const scopes = foldWorkScopeMap(entries);
    assert.equal(scopes.scopes.get(childScopeId!)?.status, "closed");
    let depth = 0;
    for (let cursor = childScopeId; cursor && cursor !== "session"; depth += 1) {
      cursor = scopes.scopes.get(cursor)?.parentId ?? "session";
    }
    assert.equal(depth, 2, "child depth is run scope + 1");
  } finally {
    await host.handle("session.delete", { sessionId });
    host.close();
  }
});
