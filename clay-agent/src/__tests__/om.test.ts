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
} from "@arnilo/prism";
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
): AIProvider {
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
