/**
 * Session run loop for ClayAgentHost (plan 130 task 2): the prompt/stream
 * cycle (mentions, skills union, OM work scopes, durable-run suspension),
 * cancel/steer/compact, autonomy toggle, run.resume, and the durable-run
 * state derivation.
 */
import {
  parseThinkingLevel,
  redactAgentEvent,
  resumeAgentRunStream,
  type AgentEvent,
  type AgentRunInterruption,
  type AgentRunResume,
  type AgentRunStateOptions,
  type Message,
  type RunDecision,
  type ContentBlock,
  type RunOptions,
  type Skill,
} from "@arnilo/prism";
import {
  createWorkScopeController,
  isWorkScopeOpenedData,
  SESSION_WORK_SCOPE_ID,
  withWorkScope,
  type WorkScopeController,
  type WorkScopeSpec,
} from "@arnilo/prism-memory/compaction/observational-memory";
import type { SupervisorHooks } from "@arnilo/prism-core/runtime/supervisor";
import type { ClayAgentHost } from "../host.js";
import {
  hasCodingTools,
  MAX_QUEUED_EVENTS,
  omCompactAfterTokens,
  optString,
  persistProviderModel,
  reqString,
  rpcError,
  type LiveSession,
} from "./internals.js";
import { commandDispatch, commandInvocation, emitCommandFeedback, registeredCommand, slashArgs } from "./commands.js";

/** Host-authored durable-run revision (must match across run + resume). */
const RUN_STATE_REVISION = "clay-agent.1";

export async function sessionPrompt(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const text = reqString(params, "text");
  // Graft push state (patches, orientation freshness) persists against the
  // run in flight (see activeGraftSessionId).
  host.activeGraftSessionId = sessionId;
  const slash = await dispatchSlashCommand(host, sessionId, text);
  if (slash.handled) return slash.value;
  let live = await host.ensureLive(sessionId);
  live = await applyRunOverrides(host, sessionId, live, params);
  // Validate the per-run options before any mention I/O (the original
  // ordering): an invalid thinkingLevel/toolNames must not read files or
  // mutate the session's loaded skills on the way to failing.
  const run = {
    ...resolveRunOptions(host, live, params),
    runState: durableRunState(host, live),
  };
  const promptInput = buildPromptInput(await host.resolveMentions(text, live));
  const outcome = await streamPromptRun(host, sessionId, live, promptInput, {
    ...run,
    runSkills: skillsWithMentions(host, live),
    // Plan 123: OM-on coding prompts run inside a host-owned work scope
    // (Prism 0.7 scope index) so this run's observations fold into a
    // per-run working set; exact-id recall still reads the whole branch.
    // Chat / OM-off prompts never build a controller.
    runScope: await omRunScope(host, sessionId, live),
  });
  return promptReply(host, sessionId, outcome);
}

/** Slash-command intercept (pi parity): a prompt whose first token exactly
 *  matches a REGISTERED command name dispatches that command instead of
 *  prompting the model. Unknown /x stays a normal prompt (chat-safe); the
 *  dispatch error for a known-name failure is the caller's feedback.
 *  `/wiki-init` is the sole wiki initiator (plan 117): with the agentSkills
 *  gate on, an unmatched /wiki-init enables the binding (the exact enableWiki
 *  path knowledge.setOptions uses — idempotent per workspace) and then
 *  dispatches the now-registered extension command. Gate off ⇒ falls through
 *  to a normal prompt (chat-safe, zero residue). wiki-searcher gates the
 *  feature; wiki-maintainer's own flag filters its registration inside
 *  enableWiki. */
async function dispatchSlashCommand(
  host: ClayAgentHost,
  sessionId: string,
  text: string,
): Promise<{ handled: false } | { handled: true; value: unknown }> {
  const slash = commandInvocation(text);
  if (!slash) return { handled: false };
  const dispatch = async (): Promise<{ handled: true; value: unknown }> => {
    const value = await commandDispatch(host, { name: slash.name, sessionId, args: slashArgs(slash) });
    // Slash feedback rides the transcript lane (pi parity): a synthetic
    // started/delta/finished triple renders the result as an assistant
    // message and closes the client's AG-UI run (the dispatch reply alone
    // would leave the run observable open and the composer stuck).
    const feedbackText =
      slash.name === "/tree" ? await host.renderTreeText(sessionId).catch(() => "tree:") : undefined;
    emitCommandFeedback(host, sessionId, slash.name, value, feedbackText);
    return { handled: true, value };
  };
  if (registeredCommand(host, slash.name)) return dispatch();
  if (slash.name === "/wiki-init" && host.agentSkillEnabled("wikiSearcher")) {
    const live = await host.ensureLive(sessionId);
    await host.enableWiki(live.workspaceRoot);
    return dispatch();
  }
  return { handled: false };
}

/** Provider/model switching mid-session (pi parity): rebuild the session's
 *  agent with the new model config, because durable runs fingerprint on
 *  AgentConfig and reject per-run model overrides. Only when the target
 *  differs from the live book — the run-scoped passthrough from the server
 *  echoes the current model, and a needless rebuild would reset the leaf.
 *  Validated against the registries, remembered on the live book, and
 *  persisted so resume keeps the switch. */
async function applyRunOverrides(
  host: ClayAgentHost,
  sessionId: string,
  live: LiveSession,
  params: Record<string, unknown>,
): Promise<LiveSession> {
  const overProvider = optString(params, "provider");
  const overModel = optString(params, "model");
  if (overProvider === undefined && overModel === undefined) return live;
  const provider = overProvider ?? live.provider;
  const model = overModel ?? live.model;
  if (!host.kernel.registries.providers.get(provider)) {
    throw rpcError(-32000, `Unknown provider: ${provider}`);
  }
  // Model ids pass through: the discovery catalog is convenience, not
  // authority — the provider rejects genuinely unknown ids at call time.
  // (The book may reference a model discovery has not listed yet.)
  if (provider === live.provider && model === live.model) return live;
  const rebuilt = host.recreateSessionModel(live, sessionId, provider, model, await host.agentRootConfig(live.agentRoot));
  await persistProviderModel(host.persistence, sessionId, provider, model);
  return rebuilt;
}

interface PromptRun {
  compaction?: RunOptions["compaction"];
  runState?: AgentRunStateOptions;
  thinkingLevel?: string;
  toolNames?: string[];
  runSkills?: readonly Skill[];
  runScope?: WorkScopeSpec;
}

interface PromptOutcome {
  lastType?: string;
  suspended?: { runId: string; interruption: AgentRunInterruption; version: number };
}

/** Per-run options the host validates at this boundary: compaction strategy,
 *  portable thinking level, and the tool allow-list. Shape only — unknown
 *  tool names fail closed in Prism's `selectRunTools` during run assembly,
 *  before any provider turn or tool dispatch. */
function resolveRunOptions(
  host: ClayAgentHost,
  live: LiveSession,
  params: Record<string, unknown>,
): Pick<PromptRun, "compaction" | "thinkingLevel" | "toolNames"> {
  const compactionName = optString(params, "compaction");
  const compaction = compactionName
    ? { strategy: host.resolveCompaction(compactionName, live), secrets: [...host.secrets] }
    : undefined;
  return { compaction, thinkingLevel: resolveThinkingLevel(params), toolNames: resolveToolNames(params) };
}

/** Plan 109 I4: per-run portable thinking level. Fail-closed — Prism
 *  `parseThinkingLevel` rejects empty/non-string input; opaque non-empty
 *  strings pass through (forward-compat). Prism resolves the compat family
 *  and snaps to the model's declared set kernel-side; non-reasoning models
 *  get no compat invented. */
function resolveThinkingLevel(params: Record<string, unknown>): string | undefined {
  const rawLevel = params.thinkingLevel;
  if (rawLevel === undefined) return undefined;
  const level = parseThinkingLevel(rawLevel);
  if (level === undefined) throw rpcError(-32602, "thinkingLevel must be a non-empty string");
  // Opaque non-empty strings pass through for forward-compatible
  // provider fields; known levels carry as the portable union.
  return typeof level === "string" ? level : level.opaque;
}

/** Plan 121: per-run tool allow-list (Prism 0.7 `RunOptions.toolNames`).
 *  Omitted → full registry; `[]` → no tools this run (a real grant, not
 *  "all"); a name list is forwarded as-is. Names come from this RPC
 *  (host-trusted), never model JSON. */
function resolveToolNames(params: Record<string, unknown>): string[] | undefined {
  const rawToolNames = params.toolNames;
  if (rawToolNames === undefined) return undefined;
  if (
    !Array.isArray(rawToolNames)
    || rawToolNames.some((name) => typeof name !== "string" || name.length === 0)
  ) {
    throw rpcError(-32602, "toolNames must be an array of non-empty strings");
  }
  return rawToolNames as string[];
}

/** Plan 117 @-mentions: `@skill:<name>` is a manual skill trigger — the
 *  named skill joins the session's loaded set (the same state the load_skill
 *  tool mutates, restoreLoadedSkills is the public path) and rides every
 *  subsequent run's `skills` option, so its body renders from the first
 *  round with no tool round-trip. `@file:<relative path>` attaches a
 *  workspace file server-side (images as image content blocks). Tokens that
 *  do not resolve stay plain text (chat-safe); mention names validate
 *  against the registry with the same toolNames discipline activation
 *  applies. */
function buildPromptInput(mentioned: { prompt: string; attachments: ContentBlock[] }): string | Message {
  return mentioned.attachments.length > 0
    ? { role: "user", content: [{ type: "text", text: mentioned.prompt }, ...mentioned.attachments] }
    : mentioned.prompt;
}

/** Run skills union: the profile's resolved active set (config default) plus
 *  manually triggered skills. Only overridden when a mention exists —
 *  untouched runs keep the config-provided catalog byte-identical. */
function skillsWithMentions(host: ClayAgentHost, live: LiveSession): readonly Skill[] | undefined {
  if (live.mentionSkills.length === 0) return undefined;
  const base = host.resolveSkills(
    host.kernel.registries.agents.resolve(live.profile),
    live.tools,
    { workspaceRoot: live.workspaceRoot },
  );
  return [...(base ?? []), ...live.mentionSkills];
}

/** stream() (not subscribe()+run()) — durable runs keep the subscription open
 *  past settlement, so only stream() both drains and resolves. Created inside
 *  the scope (and only then called) so a rejected scope id fails the prompt
 *  before Prism's run-depth proxy is entered — a stream built outside the
 *  scope would strand run depth on a throw. */
async function streamPromptRun(
  host: ClayAgentHost,
  sessionId: string,
  live: LiveSession,
  promptInput: string | Message,
  run: PromptRun,
): Promise<PromptOutcome> {
  const outcome: PromptOutcome = {};
  const runStream = async (): Promise<void> => {
    const stream = live.session.stream(promptInput, {
      maxQueuedEvents: MAX_QUEUED_EVENTS,
      overflow: "drop_oldest",
      // Host-verified identity: the daemon is the trust boundary that owns the
      // workspace root and acceptance policy, so it vouches for its own runs.
      // Durable tool effects (write/edit/delete/move) require this — without
      // it every mediated write fails closed with ERR_PRISM_TOOL_EFFECT_CONFLICT.
      identity: host.runIdentity(),
      ...(run.compaction ? { compaction: run.compaction } : {}),
      ...(run.runState ? { runState: run.runState } : {}),
      ...(run.thinkingLevel ? { thinkingLevel: run.thinkingLevel } : {}),
      ...(run.toolNames !== undefined ? { toolNames: run.toolNames } : {}),
      ...(run.runSkills ? { skills: run.runSkills } : {}),
    });
    for await (const event of stream) {
      outcome.lastType = event.type;
      // Plan 121 follow-up: the truncation streak is the only auto-compact
      // signal that lives in run events (the ratio/ceiling gates re-measure
      // at the next prompt boundary).
      if (event.type === "attention_compiled") live.attentionTruncation?.observe(event);
      if (event.type === "agent_suspended") {
        outcome.suspended = {
          runId: event.runId,
          interruption: event.interruption,
          version: event.version,
        };
      }
      host.emit("event", { sessionId, event: redactAgentEvent(event, host.redactor) satisfies AgentEvent });
    }
  };
  try {
    if (run.runScope) {
      await withWorkScope(omScopes(host, live), run.runScope, runStream);
    } else {
      await runStream();
    }
  } catch (error) {
    throw rpcError(-32000, error instanceof Error ? error.message : String(error));
  }
  return outcome;
}

function promptReply(host: ClayAgentHost, sessionId: string, outcome: PromptOutcome): unknown {
  const { lastType, suspended } = outcome;
  if (!suspended) {
    return {
      sessionId,
      lastEvent: lastType,
      status: lastType === "agent_finished" ? ("succeeded" as const) : ("aborted" as const),
    };
  }
  // Stash the suspension version: run.resume callers (approval UI) may omit
  // expectedVersion and rely on this stash.
  const liveSession = host.live.get(sessionId);
  if (liveSession) liveSession.suspension = { runId: suspended.runId, version: suspended.version };
  return {
    sessionId,
    lastEvent: lastType,
    status: "suspended" as const,
    runId: suspended.runId,
    version: suspended.version,
    interruption: suspended.interruption,
  };
}

/** Plan 123: the per-prompt work-scope spec for an OM coding run, or
 *  undefined for Chat / OM-off sessions (no controller is ever built).
 *  Ids are `run:<sessionId>:<n>`, host-generated and monotonic per branch;
 *  labels/kinds are redacted by the controller's `secrets`. `hasCodingTools`
 *  is the same coding predicate that arms durable runs and the attention
 *  compiler. */
async function omRunScope(host: ClayAgentHost, sessionId: string, live: LiveSession): Promise<WorkScopeSpec | undefined> {
  if (!live.observationalMemory || !hasCodingTools(live.tools)) return undefined;
  live.omRunCount ??= await countOpenedScopes(live, `run:${sessionId}:`);
  live.omRunCount += 1;
  return { id: `run:${sessionId}:${live.omRunCount}`, kind: "run" };
}

/** Count this branch's already-opened host scopes with `prefix` (plan 123).
 *  Prism rejects a duplicate `open`, and `LiveSession` state is not durable,
 *  so a resumed session must derive the next id from the ledger. */
async function countOpenedScopes(live: LiveSession, prefix: string): Promise<number> {
  return (await live.session.entries()).reduce(
    (count, entry) =>
      isWorkScopeOpenedData(entry.data) && entry.data.id.startsWith(prefix) ? count + 1 : count,
    0,
  );
}

/** One work-scope controller per OM-attached live session (plan 123): the
 *  run scope and its spawn children share the instance. The cache keys on
 *  the session object because a model switch swaps in a fresh OM proxy. */
export function omScopes(host: ClayAgentHost, live: LiveSession): WorkScopeController {
  const cached = live.omScopes;
  if (cached?.session === live.session) return cached.controller;
  const controller = createWorkScopeController({
    session: live.session,
    appendEntry: (entry, options) => host.persistence.append(entry, options),
    secrets: [...host.secrets],
  });
  live.omScopes = { session: live.session, controller };
  return controller;
}

/** Plan 123 task 2: nest every `test`/`validation` delegation as a `child`
 *  scope under the run scope that owns it. Hooks, not a wrapped child run:
 *  the ledger's enter/leave stack is session-global, so a child entering a
 *  scope while the async parent is still open would make the parent's own
 *  flush bind to the child. Open/close only — one ledger pair per spawn,
 *  closed child scopes stay out of the default (leaf + ancestors)
 *  projection, and Prism enforces the depth cap and that `parentId` exists
 *  and is open. Without an active scoped run (Chat / OM off / a resumed
 *  run outside a prompt) there is no parent: no scope is written. */
export function omSpawnScopes(host: ClayAgentHost, sessionId: string): SupervisorHooks {
  const opened = new Map<string, string>();
  return {
    before: async ({ childId, delegationId }) => {
      const live = host.live.get(sessionId);
      // `before` re-runs when a suspended child is resumed (Prism contract:
      // hooks are idempotent) — the existing scope keeps serving it.
      if (!live?.observationalMemory || opened.has(delegationId)) return {};
      const scopes = omScopes(host, live);
      const parentId = await scopes.leaf();
      if (parentId === SESSION_WORK_SCOPE_ID) return {};
      live.omChildCount ??= await countOpenedScopes(live, `child:${sessionId}:`);
      live.omChildCount += 1;
      const id = `child:${sessionId}:${live.omChildCount}`;
      await scopes.open({ id, parentId, kind: "child", label: childId });
      opened.set(delegationId, id);
      return {};
    },
    after: async ({ delegationId }) => {
      const id = opened.get(delegationId);
      if (!id) return;
      opened.delete(delegationId);
      const live = host.live.get(sessionId);
      if (!live?.observationalMemory) return;
      // Advisory bookkeeping: a scope lost to a concurrent delete must not
      // fail the delegation's terminal hook (Prism reports hook errors as
      // `delegation_error` events).
      await omScopes(host, live).close(id).catch(() => undefined);
    },
  };
}

/** Durable + tool-interrupt for coding sessions only; Chat stays non-durable. */
export function durableRunState(host: ClayAgentHost, live: LiveSession): AgentRunStateOptions | undefined {
  if (!hasCodingTools(live.tools)) return undefined;
  // Tool-approval interrupts are opt-in via autonomy: full autonomy
  // (the default) auto-approves every tool call and streams through;
  // turning autonomy off (session.setAutonomy / NewSession param)
  // re-arms the suspend-on-tool-gate flow for callers that want it.
  return {
    checkpoints: host.persistence.checkpoints,
    definitionRevision: RUN_STATE_REVISION,
    interruptBeforeTool: !live.fullAutonomy,
  };
}

export async function sessionCancel(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  live.session.abort("cancel");
  return { sessionId, cancelled: true };
}

export function sessionSteer(host: ClayAgentHost, params: Record<string, unknown>): unknown {
  const sessionId = reqString(params, "sessionId");
  const text = reqString(params, "text");
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  live.session.steer(text, { softInterrupt: params.softInterrupt === true });
  return { sessionId, steered: true };
}

export async function sessionCompact(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const strategyName = optString(params, "strategy") ?? host.runConfig.compaction;
  const threshold = params.compactAfterTokens;
  if (
    threshold !== undefined &&
    (typeof threshold !== "number" || !Number.isFinite(threshold) || threshold <= 0)
  ) {
    throw rpcError(-32602, "compactAfterTokens must be a positive finite number");
  }
  if (threshold !== undefined) omCompactAfterTokens.set(sessionId, threshold);
  const live = await host.ensureLive(sessionId);
  try {
    const result = await live.session.compact({
      strategy: host.resolveCompaction(strategyName, live),
      secrets: [...host.secrets],
    });
    const entry = result.entries?.[0];
    return {
      sessionId,
      summary: result.summary,
      entryId: entry?.id,
      strategy: strategyName,
    };
  } catch (error) {
    throw rpcError(-32000, error instanceof Error ? error.message : String(error));
  }
}

/** Toggle full autonomy for a live session (decision 2026-09-20-2049):
 *  default true; false re-arms approval prompts for gated tool calls.
 *  Host-set only — no agent-facing tool can change this. */
export function sessionSetAutonomy(host: ClayAgentHost, params: Record<string, unknown>): unknown {
  const sessionId = reqString(params, "sessionId");
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
  if (typeof params.enabled !== "boolean") {
    throw rpcError(-32602, "enabled must be a boolean");
  }
  live.fullAutonomy = params.enabled;
  return { sessionId, fullAutonomy: live.fullAutonomy };
}

/** Resume a suspended durable run. Validates decision shape before any checkpoint I/O. */
export async function runResume(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const sessionId = reqString(params, "sessionId");
  const runId = reqString(params, "runId");
  // expectedVersion is optional: the approval UI may omit it, in which
  // case the version stashed at suspension time applies. A supplied
  // version still wins (fail-closed optimistic concurrency). The stash
  // lookup never creates a session — validation below still precedes I/O.
  const suppliedVersion = params.expectedVersion;
  const expectedVersion =
    typeof suppliedVersion === "number" && Number.isSafeInteger(suppliedVersion) && suppliedVersion > 0
      ? suppliedVersion
      : host.live.get(sessionId)?.suspension?.version;
  if (expectedVersion === undefined) {
    throw rpcError(-32602, "params.expectedVersion must be a positive integer");
  }
  const decision = optString(params, "decision");
  const decisionsRaw = params.decisions;
  const decisions = Array.isArray(decisionsRaw) ? decisionsRaw.map((item, index) => resumeDecision(item, index)) : undefined;
  if (!decision && !decisions) throw rpcError(-32602, "params.decision or params.decisions is required");
  if (decision && decisions) throw rpcError(-32602, "params.decision and params.decisions are mutually exclusive");
  if (decision !== undefined && decision !== "approve" && decision !== "deny") {
    throw rpcError(-32602, `unknown decision: ${decision}`);
  }
  const live = await host.ensureLive(sessionId);
  live.suspension = undefined;
  const resume: AgentRunResume = { expectedVersion, ...(decisions ? { decisions } : { decision }) };
  try {
    // Stream the resumed run like sessionPrompt: clients repaint the
    // transcript from these events; the RPC reply only reports final
    // status. Without this the continuation ran invisibly.
    let lastType: string | undefined;
    let suspended:
      | { runId: string; interruption: AgentRunInterruption; version: number }
      | undefined;
    for await (const event of resumeAgentRunStream(
      live.agent,
      { runId, sessionId },
      resume,
      {
        checkpoints: host.persistence.checkpoints,
        definitionRevision: RUN_STATE_REVISION,
        ownership: host.runOwnership(),
      },
    )) {
      lastType = event.type;
      if (event.type === "agent_suspended") {
        // Re-arm the stash: chained approvals resume from here.
        suspended = { runId: event.runId, interruption: event.interruption, version: event.version };
        live.suspension = { runId, version: event.version };
      }
      host.emit("event", { sessionId, event: redactAgentEvent(event, host.redactor) satisfies AgentEvent });
    }
    return suspended
      ? {
          sessionId,
          runId,
          status: "suspended" as const,
          version: suspended.version,
          interruption: suspended.interruption,
        }
      : {
          sessionId,
          runId,
          status: lastType === "agent_finished" ? ("succeeded" as const) : ("aborted" as const),
        };
  } catch (error) {
    throw rpcError(-32000, error instanceof Error ? error.message : String(error));
  }
}

function resumeDecision(value: unknown, index: number): RunDecision {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw rpcError(-32602, `params.decisions[${index}] must be an object`);
  }
  const record = value as Record<string, unknown>;
  const approvalId = record.approvalId;
  const outcome = record.outcome;
  if (typeof approvalId !== "string" || approvalId.length === 0) {
    throw rpcError(-32602, `params.decisions[${index}].approvalId is required`);
  }
  if (outcome !== "allow_once" && outcome !== "allow_for_run" && outcome !== "reject_once" && outcome !== "reject_for_run") {
    throw rpcError(-32602, `params.decisions[${index}].outcome must be allow_once, allow_for_run, reject_once, or reject_for_run`);
  }
  return {
    approvalId,
    outcome,
    ...(typeof record.reason === "string" ? { reason: record.reason } : {}),
  };
}
