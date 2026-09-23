/**
 * Session lifecycle RPCs for ClayAgentHost (plan 130 task 2): create,
 * list/load, agent switch, resume/restore, delete, fork/clone/checkpoint,
 * workspace-scoped search + resumable list.
 */
import { randomUUID } from "node:crypto";
import { DEFAULT_SESSION_SEARCH_LIMIT, SESSION_SEARCH_WORKSPACE_METADATA_KEY } from "@arnilo/prism";
import type { ClayAgentHost } from "../host.js";
import {
  MAX_LIST,
  MAX_LOAD_ENTRIES,
  MAX_SESSION_SEARCH_LIMIT,
  optString,
  reqString,
  rpcError,
  TENANT,
  omWorkerModels,
  type AgentRootConfig,
  type LiveSession,
} from "./internals.js";
import type { CreateSessionResult } from "./session-build.js";
import { branchEntries } from "./session-tree.js";
import { persistOmWorkers } from "./om.js";

/** Local `YYYY-MM-DD HH:MM` for the resume list's second line. The daemon runs
 *  on the user's machine, so its timezone is the user's; a raw ISO stamp in
 *  UTC reads as the wrong hour. */
function localStamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/**
 * What a coding session needs live before its tools are built: capabilities
 * activate on the first session that declares coding tools (never on import,
 * never for Chat-only initialize), and graft is available by default (plan
 * 117) — the first coding session for a workspace attempts the pull-mode
 * binding once per root per daemon. Chat sessions trigger neither.
 *
 * Every path that can start a coding session calls this: create, a resume
 * after a daemon restart (plan 130 A2: the restored session's tools, graft
 * binding and acceptance roots must come from its recorded workspace root,
 * not the restarted daemon's cwd), and an in-place agent switch.
 */
async function activateCodingSession(
  host: ClayAgentHost,
  profile: string,
  agentRoot: string,
  allowList: readonly unknown[],
  workspaceRoot: string,
): Promise<void> {
  if (!host.profileWantsCodingTools(profile)) return;
  await host.ensureCapabilities(agentRoot, allowList);
  await host.ensureGraftBound(workspaceRoot);
}

export async function sessionNew(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const profile = reqString(params, "profile");
  const provider = reqString(params, "provider");
  const modelId = reqString(params, "model");
  const id = optString(params, "id") ?? randomUUID();
  const workspaceRoot = optString(params, "workspaceRoot") ?? process.cwd();
  // Approvals are opt-out by default (decision 2026-09-20-2049): tool
  // calls auto-approve unless a caller explicitly blocks autonomy.
  const fullAutonomy = params.fullAutonomy !== false;
  const observationalMemory = host.resolveOmFlag(profile, params.observationalMemory);
  // Plan 109 I8: per-session OM worker models (each may differ from the
  // session model; `null` clears a previously-set selection).
  const omWorkers = host.parseOmWorkers(params.observationalMemoryWorkers);
  if (omWorkers) omWorkerModels.set(id, omWorkers);
  // Plan 118 task 35: the session's agent (its config root + that agent's
  // MCP allow-list). Absent = the daemon's default agent; an unknown or
  // malformed name fails closed here.
  const { agentRoot, agentType, agent, allowList } = await resolveAgentBinding(host, params.agent, params.mcpAllowList);
  await activateCodingSession(host, profile, agentRoot, allowList, workspaceRoot);
  await host.ensureSkillDiscovery(workspaceRoot);
  const created = host.createSession(id, profile, provider, modelId, {
    workspaceRoot,
    fullAutonomy,
    observationalMemory,
    agent,
    mcpAllowList: allowList,
  });
  await persistSessionRecord(host, {
    id,
    profile,
    provider,
    model: modelId,
    workspaceRoot,
    fullAutonomy,
    observationalMemory,
    agentType,
    omWorkers,
  });
  host.live.set(id, liveSessionFor(created, {
    profile,
    provider,
    model: modelId,
    workspaceRoot,
    agentType,
    agentRoot,
    mcpAllowList: allowList,
    fullAutonomy,
    observationalMemory,
  }));
  return {
    sessionId: id,
    profile,
    provider,
    model: modelId,
    workspaceRoot,
    agent: agentType,
    agentRoot,
    fullAutonomy,
    observationalMemory,
    tools: created.tools.map((tool) => tool.name),
  };
}

/** Plan 118 task 35: resolve one session's agent from the RPC's agent name +
 *  MCP allow-list — the config root, its recorded type name, the loaded
 *  config, and the allow-list to remember for that root (the last one the
 *  server sent, else this root's known list). Absent name = default agent;
 *  an unknown/malformed name fails closed inside `resolveAgentRoot`. */
async function resolveAgentBinding(
  host: ClayAgentHost,
  agentParam: unknown,
  allowListParam: unknown,
): Promise<{ agentRoot: string; agentType: string | undefined; agent: AgentRootConfig; allowList: readonly unknown[] }> {
  const agentRoot = host.resolveAgentRoot(agentParam);
  const agentType = host.agentTypeOf(agentRoot);
  const agent = await host.agentRootConfig(agentRoot);
  const allowList = Array.isArray(allowListParam)
    ? (allowListParam as readonly unknown[])
    : host.mcpAllowListFor(agentRoot);
  host.mcpAllowListByRoot.set(agentRoot, allowList);
  return { agentRoot, agentType, agent, allowList };
}

/** Append the session record that session.search/resumable and resume read
 *  back (workspace identity included — decision 2201). */
async function persistSessionRecord(
  host: ClayAgentHost,
  args: {
    id: string;
    profile: string;
    provider: string;
    model: string;
    workspaceRoot: string;
    fullAutonomy: boolean;
    observationalMemory: boolean;
    agentType: string | undefined;
    omWorkers: unknown;
  },
): Promise<void> {
  const now = new Date().toISOString();
  if (typeof host.persistence.appendSession !== "function") {
    throw rpcError(-32000, "session store cannot persist session records");
  }
  await host.persistence.appendSession({
    id: args.id,
    tenantId: TENANT,
    agentDefinitionId: args.profile,
    createdAt: now,
    updatedAt: now,
    metadata: {
      profile: args.profile,
      provider: args.provider,
      model: args.model,
      observationalMemory: args.observationalMemory,
      fullAutonomy: args.fullAutonomy,
      // Stable workspace identity for workspace-scoped search (decision 2201).
      [SESSION_SEARCH_WORKSPACE_METADATA_KEY]: args.workspaceRoot,
      ...(args.agentType ? { agentType: args.agentType } : {}),
      ...(args.omWorkers ? { omWorkers: args.omWorkers } : {}),
    },
  });
}

/** The live book for a freshly built session: one shape for session.new and
 *  ensureLive so the two paths cannot drift. */
function liveSessionFor(
  created: CreateSessionResult,
  book: {
    profile: string;
    provider: string;
    model: string;
    workspaceRoot: string;
    agentType: string | undefined;
    agentRoot: string;
    mcpAllowList: readonly unknown[];
    fullAutonomy: boolean;
    observationalMemory: boolean;
  },
): LiveSession {
  return {
    session: created.session,
    agent: created.agent,
    systemPrompt: created.systemPrompt,
    profile: book.profile,
    provider: book.provider,
    model: book.model,
    workspaceRoot: book.workspaceRoot,
    agentType: book.agentType,
    agentRoot: book.agentRoot,
    mcpAllowList: book.mcpAllowList,
    fullAutonomy: book.fullAutonomy,
    observationalMemory: book.observationalMemory,
    tools: created.tools,
    mentionSkills: [],
    ...(created.attentionTruncation ? { attentionTruncation: created.attentionTruncation } : {}),
    ...(created.stopSubagentLifecycle ? { stopSubagentLifecycle: created.stopSubagentLifecycle } : {}),
  };
}

export async function sessionList(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const limitRaw = params.limit;
  const limit = typeof limitRaw === "number" && Number.isFinite(limitRaw) ? Math.min(MAX_LIST, Math.max(1, limitRaw)) : MAX_LIST;
  const page = await host.persistence.querySessions({
    tenantId: TENANT,
    limit,
    ...(typeof params.cursor === "string" ? { cursor: params.cursor } : {}),
  });
  return {
    sessions: page.items.map((item) => ({
      id: item.id,
      profile: item.agentDefinitionId,
      updatedAt: item.updatedAt,
      metadata: item.metadata,
    })),
    nextCursor: page.nextCursor,
  };
}

export async function sessionLoad(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const entryId = typeof params.entryId === "string" ? params.entryId : undefined;
  const page = await host.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
  const record = page.items[0];
  if (!record) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  const entries = await host.persistence.list(sessionId);
  const selected = entryId ? branchEntries(entries, entryId) : undefined;
  return {
    sessionId,
    profile: record.agentDefinitionId,
    metadata: record.metadata,
    entries: (selected ?? entries).slice(-MAX_LOAD_ENTRIES).map((entry) => host.redactor.redact(entry)),
  };
}

/**
 * Plan 118 task 35: switch a live session's agent type in place.
 *
 * The agent decides the system prompt, the skill roots, the tool caps and
 * the MCP servers a run uses, so the live agent is rebuilt over the *same*
 * session branch (the mid-session model-switch mechanism): the session id,
 * its transcript and its leaf survive, and only the config the next run
 * reads changes. The caller's workspace is untouched. The agent name is
 * validated here as well (contained to the `agents/` root next to the
 * default agent), so a bad name fails closed instead of silently running
 * the default agent's config.
 */
export async function sessionSetAgent(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = await host.ensureLive(sessionId);
  const { agentRoot, agentType, agent, allowList } = await resolveAgentBinding(
    host,
    params.agent,
    params.mcpAllowList,
  );
  const overProvider = optString(params, "provider");
  const overModel = optString(params, "model");
  const provider = overProvider ?? live.provider;
  const model = overModel ?? live.model;
  if (!host.kernel.registries.providers.get(provider)) {
    throw rpcError(-32000, `Unknown provider: ${provider}`);
  }
  // Bind the new agent *before* the rebuild: the rebuilt session's tools
  // and capability set come from these fields.
  live.agentRoot = agentRoot;
  live.agentType = agentType;
  live.mcpAllowList = allowList;
  host.recreateSessionModel(live, sessionId, provider, model, agent);
  const omWorkers = host.parseOmWorkers(params.observationalMemoryWorkers);
  if (omWorkers) {
    omWorkerModels.set(sessionId, omWorkers);
    await persistOmWorkers(host, sessionId);
  }
  await activateCodingSession(host, live.profile, agentRoot, allowList, live.workspaceRoot);
  await persistAgentType(host, sessionId, agentType, provider, model);
  return {
    sessionId,
    agent: agentType,
    agentRoot,
    profile: live.profile,
    provider,
    model,
    tools: live.tools.map((tool) => tool.name),
  };
}

/** Best-effort persistence of the switch (same advisory pattern as
 *  `persistProviderModel`): the live session already runs the new agent,
 *  and the record keeps resume consistent. */
async function persistAgentType(
  host: ClayAgentHost,
  sessionId: string,
  agentType: string | undefined,
  provider: string,
  model: string,
): Promise<void> {
  if (typeof host.persistence.appendSession !== "function") return;
  try {
    const page = await host.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
    const record = page.items[0];
    if (!record) return;
    const metadata: Record<string, unknown> = {
      ...((record.metadata ?? {}) as Record<string, unknown>),
      provider,
      model,
    };
    if (agentType) metadata.agentType = agentType;
    else delete metadata.agentType;
    await host.persistence.appendSession({
      ...record,
      metadata,
      updatedAt: new Date().toISOString(),
    });
  } catch {
    // Metadata persistence is advisory; the live switch already applied.
  }
}

export async function sessionResume(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = await host.ensureLive(sessionId);
  return {
    sessionId,
    profile: live.profile,
    provider: live.provider,
    model: live.model,
    // Plan 118 task 35: the session's agent rides the resume reply so the
    // server keeps labelling turns with their producer after a restart.
    agent: live.agentType,
    agentRoot: live.agentRoot,
    // Plan 119 SC-6: the resumed session's workspace is the one it was
    // created in (metadata), so callers can see the binding the restored
    // tools and prompts actually use.
    workspaceRoot: live.workspaceRoot,
    leafId: live.session.leafId,
  };
}

export async function ensureLive(host: ClayAgentHost, sessionId: string): Promise<LiveSession> {
  const existing = host.live.get(sessionId);
  if (existing) return existing;
  const page = await host.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
  const record = page.items[0];
  if (!record) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  const metadata = (record.metadata ?? {}) as Record<string, unknown>;
  const profile = typeof metadata.profile === "string" ? metadata.profile : record.agentDefinitionId;
  const provider = typeof metadata.provider === "string" ? metadata.provider : undefined;
  const model = typeof metadata.model === "string" ? metadata.model : undefined;
  if (!profile || !provider || !model) throw rpcError(-32000, `Session ${sessionId} is missing profile/provider/model`);
  const observationalMemory = host.resolveOmFlag(profile, metadata.observationalMemory);
  const restoredWorkers = host.parseOmWorkers(metadata.omWorkers);
  if (restoredWorkers) omWorkerModels.set(sessionId, restoredWorkers);
  // Plan 118 task 35: a restored session runs the agent recorded with it
  // (absent for sessions written before agent types — the default agent).
  const agentRoot = host.resolveAgentRoot(
    typeof metadata.agentType === "string" ? metadata.agentType : undefined,
  );
  const agentType = host.agentTypeOf(agentRoot);
  const agent = await host.agentRootConfig(agentRoot);
  // Plan 119 SC-6: a resumed session runs against the workspace it was
  // created in — the root the server recorded at `session.new` — never the
  // daemon's launch cwd (`process.cwd()`), which would bind the restored
  // tool cwd, acceptance roots, and wiki/graft binding to the wrong folder.
  // Only pre-workspace records (no metadata) fall back to the cwd.
  const workspaceRoot =
    typeof metadata[SESSION_SEARCH_WORKSPACE_METADATA_KEY] === "string"
      ? (metadata[SESSION_SEARCH_WORKSPACE_METADATA_KEY] as string)
      : process.cwd();
  const allowList = host.mcpAllowListFor(agentRoot);
  // Autonomy is restored from the session's own record, not from a fresh
  // session default: `session.new` records what the caller asked for, and a
  // user who blocked autonomy must not have it re-armed by restarting the
  // daemon (decision 2026-09-20-2049). Records written before the field
  // existed fall back to the same opt-out default as creation.
  const fullAutonomy = metadata.fullAutonomy !== false;
  // Plan 130 A2: a restarted daemon has no live capabilities or graft
  // binding, so the resumed coding session re-activates them for its
  // recorded root — otherwise the resumed session silently loses its MCP,
  // browser, and graft tools even though its root is right.
  await activateCodingSession(host, profile, agentRoot, allowList, workspaceRoot);
  await host.ensureSkillDiscovery(workspaceRoot);
  const created = host.createSession(sessionId, profile, provider, model, {
    workspaceRoot,
    fullAutonomy,
    observationalMemory,
    agent,
    mcpAllowList: allowList,
  });
  const live = liveSessionFor(created, {
    profile,
    provider,
    model,
    workspaceRoot,
    agentType,
    agentRoot,
    mcpAllowList: allowList,
    fullAutonomy,
    observationalMemory,
  });
  host.live.set(sessionId, live);
  return live;
}

export async function sessionDelete(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  if (live) {
    live.session.abort("deleted");
    live.stopSubagentLifecycle?.();
    host.live.delete(sessionId);
  }
  const result = await host.persistence.lifecycle.applyRetention({
    tenantId: TENANT,
    policy: { id: "clay-agent-delete", createdAt: new Date().toISOString(), tenantId: TENANT },
    candidates: [sessionId],
  });
  return { deleted: result.deleted.includes(sessionId) };
}

export function sessionFork(host: ClayAgentHost, params: Record<string, unknown>): unknown {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  const entryId = optString(params, "entryId");
  const forked = live.session.fork({ ...(entryId ? { leafId: entryId } : {}) });
  // Same session id, same store, different branch. Abandoned side is kept.
  return { sessionId: forked.id, leafId: forked.leafId };
}

export async function sessionClone(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  const entryId = optString(params, "entryId");
  const id = optString(params, "id") ?? randomUUID();
  // Pre-create the record so Prism's clone entry-appends attach to a
  // properly tenanted session instead of auto-creating a tenant-less row.
  const now = new Date().toISOString();
  if (typeof host.persistence.appendSession !== "function") {
    throw rpcError(-32000, "session store cannot persist session records");
  }
  await host.persistence.appendSession({
    id,
    tenantId: TENANT,
    agentDefinitionId: live.profile,
    createdAt: now,
    updatedAt: now,
    metadata: { profile: live.profile, provider: live.provider, model: live.model, workspaceRoot: live.workspaceRoot },
  });
  const cloned = await live.session.clone({ id, ...(entryId ? { leafId: entryId } : {}) });
  return { sessionId: cloned.id, leafId: cloned.leafId };
}

/** Capture a document checkpoint at `entryId` (task-boundary or explicit
 *  user checkpoint). The server snapshots every open document buffer. */
export async function sessionCheckpoint(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const entryId = reqString(params, "entryId");
  if (!host.live.has(sessionId)) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  return (await host.request("checkpoint.capture", { sessionId, entryId })) as unknown;
}

/** Workspace-scoped session search (decision 2201): always filters by this
 *  session's workspaceRoot; hits are transcript metadata, never auto-injected
 *  into agent context. Cross-workspace queries return empty, not an error. */
export async function sessionSearch(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  const query = typeof params.query === "string" ? params.query : undefined;
  const limitRaw = typeof params.limit === "number" && Number.isSafeInteger(params.limit) ? params.limit : undefined;
  const limit = limitRaw ? Math.min(MAX_SESSION_SEARCH_LIMIT, Math.max(1, limitRaw)) : DEFAULT_SESSION_SEARCH_LIMIT;
  if (!host.persistence.searchSessions) {
    throw rpcError(-32000, "session store does not support search");
  }
  const page = await host.persistence.searchSessions({
    tenantId: TENANT,
    workspaceRoot: live.workspaceRoot,
    ...(query ? { query } : {}),
    limit,
  });
  return {
    hits: page.items.map((hit) => ({
      sessionId: hit.sessionId,
      leafId: hit.leafId,
      updatedAt: hit.updatedAt,
      label: hit.label,
      summary: hit.summary,
      ...(hit.snippet !== undefined ? { snippet: host.redactor.redact(hit.snippet) } : {}),
      metadata: hit.metadata,
    })),
    nextCursor: page.nextCursor,
  };
}

/** Plan 109 I9: workspace-scoped resumable session list backing the
 *  /resume picker — one surface shared by the slash command and the
 *  panel affordance. Fail-closed: requires an explicit workspaceRoot
 *  (this surface never lists across workspaces); the root arrives from
 *  the server's tab registry, never from webview input. Most-recent
 *  first (store ordering), bounded, safe display fields only (same
 *  redaction posture as session.search). */
export async function sessionResumable(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const workspaceRoot = reqString(params, "workspaceRoot");
  const limitRaw = typeof params.limit === "number" && Number.isSafeInteger(params.limit) ? params.limit : undefined;
  const limit = limitRaw ? Math.min(MAX_SESSION_SEARCH_LIMIT, Math.max(1, limitRaw)) : DEFAULT_SESSION_SEARCH_LIMIT;
  if (!host.persistence.searchSessions) {
    throw rpcError(-32000, "session store does not support search");
  }
  const page = await host.persistence.searchSessions({
    tenantId: TENANT,
    workspaceRoot,
    limit,
  });
  return {
    sessions: page.items.map((hit) => ({
      sessionId: hit.sessionId,
      updatedAt: hit.updatedAt,
      // Plan 117 follow-up: the resume list shows the last-active time, so
      // it ships a local-time display stamp beside the raw ISO value.
      ...(hit.updatedAt ? { updatedAtLabel: localStamp(hit.updatedAt) } : {}),
      label: hit.label,
      summary: hit.summary,
      ...(hit.snippet !== undefined ? { snippet: host.redactor.redact(hit.snippet) } : {}),
      metadata: hit.metadata,
    })),
    nextCursor: page.nextCursor,
  };
}
