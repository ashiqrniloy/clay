/**
 * Observational-memory worker selection + activity surface for
 * ClayAgentHost (plan 130 task 2, plan 109 I8): per-session OM worker
 * model validation/persistence/apply and the bounded observer activity
 * log.
 */
import type { ClayAgentHost } from "../host.js";
import {
  clipUtf8,
  omWorkerModels,
  type OmWorkerModels,
  type OmWorkerSelection,
  reqString,
  rpcError,
  TENANT,
} from "./internals.js";

// Plan 109 I8 Observational Memory tab: bounded activity rows and summaries.
const MAX_OM_ACTIVITY_ROWS = 200;
const MAX_OM_SUMMARY_CHARS = 160;

/** Plan 109 I8: validate an OM worker selection payload from session.new
 *  / session.om.set. Each worker is `{ provider, model }` (both
 *  non-empty strings) or `null` to clear; unknown shapes fail closed. */
export function parseOmWorkers(raw: unknown): OmWorkerModels | undefined {
  if (raw === undefined) return undefined;
  if (raw === null) return {};
  if (typeof raw !== "object" || Array.isArray(raw)) {
    throw rpcError(-32602, "observationalMemoryWorkers must be an object");
  }
  const value = raw as Record<string, unknown>;
  const one = (input: unknown, name: string): OmWorkerSelection | null | undefined => {
    if (input === undefined) return undefined;
    if (input === null) return null;
    if (typeof input !== "object" || Array.isArray(input)) {
      throw rpcError(-32602, `${name} must be { provider, model } or null`);
    }
    const worker = input as Record<string, unknown>;
    if (typeof worker.provider !== "string" || worker.provider.trim() === "" ||
        typeof worker.model !== "string" || worker.model.trim() === "") {
      throw rpcError(-32602, `${name}.provider and ${name}.model must be non-empty strings`);
    }
    return { provider: worker.provider, model: worker.model };
  };
  const observation = one(value.observation, "observation");
  const reflection = one(value.reflection, "reflection");
  const out: OmWorkerModels = {};
  if (observation === null) out.observation = undefined;
  else if (observation !== undefined) out.observation = observation;
  if (reflection === null) out.reflection = undefined;
  else if (reflection !== undefined) out.reflection = reflection;
  return out;
}

/** Plan 109 I8: set per-session OM worker models. Selections are
 *  validated against the provider registry (unknown provider fails
 *  closed), persisted into the session record metadata, and applied by
 *  re-attaching OM (worker rebuild keeps the leaf). `requireExplicitModel`
 *  stays true: cleared/unset workers skip — never the session model. */
export async function sessionOmSet(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const workers = parseOmWorkers(params.workers);
  if (!workers) throw rpcError(-32602, "workers must be an object");
  for (const worker of [workers.observation, workers.reflection]) {
    if (worker && !host.kernel.registries.providers.get(worker.provider)) {
      throw rpcError(-32000, `Unknown OM worker provider: ${worker.provider}`);
    }
  }
  const existing = omWorkerModels.get(sessionId) ?? {};
  omWorkerModels.set(sessionId, { ...existing, ...workers });
  await persistOmWorkers(host, sessionId);
  // Re-attach so the new workers apply immediately (next run); the
  // rebuild preserves the leaf and is the same mechanism as a
  // mid-session model switch.
  const live = host.live.get(sessionId);
  if (live?.observationalMemory) {
    const agent = await host.agentRootConfig(live.agentRoot);
    host.recreateSessionModel(live, sessionId, live.provider, live.model, agent);
  }
  const stored = omWorkerModels.get(sessionId) ?? {};
  return {
    sessionId,
    observation: stored.observation ?? null,
    reflection: stored.reflection ?? null,
  };
}

/** Best-effort persistence of per-session OM worker models (same
 *  advisory pattern as persistProviderModel). */
export async function persistOmWorkers(host: ClayAgentHost, sessionId: string): Promise<void> {
  if (typeof host.persistence.appendSession !== "function") return;
  try {
    const page = await host.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
    const record = page.items[0];
    if (!record) return;
    const stored = omWorkerModels.get(sessionId);
    const metadata = {
      ...((record.metadata ?? {}) as Record<string, unknown>),
      omWorkers: stored ?? {},
    };
    await host.persistence.appendSession({
      ...record,
      metadata,
      updatedAt: new Date().toISOString(),
    });
  } catch {
    // Metadata persistence is advisory; the live selection still applies.
  }
}

/** Plan 109 I8: the Observational Memory tab's read model — current
 *  worker selection plus the bounded observer activity log (recorded
 *  observations with fact summaries, reflections, drops, compaction
 *  folds) derived from the session's branch entries at completion
 *  boundaries (fetched on tab open / transcript change, never polled). */
export async function sessionOmActivity(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  const stored = omWorkerModels.get(sessionId) ?? {};
  const activity: Array<{ id: string; kind: string; summary: string }> = [];
  if (live?.observationalMemory) {
    for (const entry of await live.session.entries()) {
      const data = entry.data as Record<string, unknown> | undefined;
      if (!data) continue;
      const type = typeof data.type === "string" ? data.type : undefined;
      if (type === "om.observations.recorded" && Array.isArray(data.observations)) {
        for (const observation of data.observations as Array<Record<string, unknown>>) {
          if (typeof observation.content !== "string") continue;
          activity.push({
            id: `${entry.id}#${String(observation.id ?? activity.length)}`,
            kind: "observation",
            summary: omActivitySummary(host, observation.content),
          });
          if (activity.length >= MAX_OM_ACTIVITY_ROWS) break;
        }
      } else if (type === "om.reflections.recorded" && Array.isArray(data.reflections)) {
        for (const reflection of data.reflections as Array<Record<string, unknown>>) {
          if (typeof reflection.content !== "string") continue;
          activity.push({
            id: `${entry.id}#${String(reflection.id ?? activity.length)}`,
            kind: "reflection",
            summary: omActivitySummary(host, reflection.content),
          });
          if (activity.length >= MAX_OM_ACTIVITY_ROWS) break;
        }
      } else if (type === "om.observations.dropped" && Array.isArray(data.observationIds)) {
        const ids = (data.observationIds as unknown[]).filter((id): id is string => typeof id === "string");
        if (ids.length > 0) {
          activity.push({
            id: `${entry.id}#dropped`,
            kind: "drop",
            summary: `Dropped ${ids.length} observation${ids.length === 1 ? "" : "s"}`,
          });
        }
      } else if (type === "om.folded") {
        activity.push({
          id: `${entry.id}#folded`,
          kind: "fold",
          summary: "Compaction folded memory into the summary",
        });
      }
      if (activity.length >= MAX_OM_ACTIVITY_ROWS) break;
    }
  }
  return {
    sessionId,
    attached: Boolean(live?.observationalMemory),
    observation: stored.observation ?? null,
    reflection: stored.reflection ?? null,
    activity,
  };
}

/** One-line fact summary: redacted, bounded, first line only. */
function omActivitySummary(host: ClayAgentHost, text: string): string {
  const firstLine = text.split("\n", 1)[0] ?? text;
  return clipUtf8(host.redactor.redact(firstLine), MAX_OM_SUMMARY_CHARS);
}
