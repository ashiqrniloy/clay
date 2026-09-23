/**
 * Session construction for ClayAgentHost (plan 130 task 2): profile tool
 * assembly (coding tools, capabilities, wiki/graft riders), supervisor +
 * child agents, OM attachment, run limits/auto-compaction, and the
 * mid-session model rebuild.
 */
import {
  createAttentionTruncationTrigger,
  createAgent,
  createLoadSkillTool,
  createProviderResolver,
  DEFAULT_ATTENTION_COMPACT_RATIO,
  DEFAULT_ATTENTION_TRUNCATION_THRESHOLD,
  HARD_RUN_LIMITS,
  type Agent,
  type AgentDefinition,
  type AgentSession,
  type AIProvider,
  type AttentionTruncationTrigger,
  type CompactionOptions,
  type CompactionTrigger,
  type ModelConfig,
  type Skill,
  type SystemPromptContribution,
  type ToolDefinition,
} from "@arnilo/prism";
import { createJsonSchemaToolArgumentValidator } from "@arnilo/prism-core/validation/json-schema";
import { observeSupervisorLifecycle, type AskUserDecisionAnswer } from "@arnilo/prism-coding-tools/agent";
import {
  createCancelAgentTool,
  createSpawnAgentTool,
  createSupervisor,
  createWaitAgentTool,
} from "@arnilo/prism-core/runtime/supervisor";
import {
  createObservationalMemory,
  createRecallMemoryTool,
} from "@arnilo/prism-memory/compaction/observational-memory";
import { CODING_TOOL_NAMES, buildCodingTools } from "../coding-tools.js";
import {
  DEFAULT_COMPACT_AFTER_TOKENS,
  createNamedCompactionStrategy,
  isCompactionStrategyName,
} from "../compaction.js";
import type { ClayAgentHost } from "../host.js";
import {
  derivedTotalTokens,
  hasCodingTools,
  omSettingsProvider,
  omWorkerModels,
  rpcError,
  type AgentRootConfig,
  type LiveSession,
  type RunConfig,
} from "./internals.js";
import { loadUserSystemPrompt, loadWorkspaceAgentsPrompt } from "./skills.js";

export interface CreateSessionResult {
  session: AgentSession;
  agent: Agent;
  tools: ToolDefinition[];
  systemPrompt?: SystemPromptContribution[];
  attentionTruncation?: AttentionTruncationTrigger;
  stopSubagentLifecycle?: () => void;
}

export function createSession(
  host: ClayAgentHost,
  id: string,
  profile: string,
  provider: string,
  modelId: string,
  options: {
    workspaceRoot: string;
    fullAutonomy: boolean;
    observationalMemory: boolean;
    /** Plan 118 task 35: the session's agent config (root, tool caps,
     *  skills config + files) — never the host's default. */
    agent: AgentRootConfig;
    /** Server-built MCP allow-list for that agent (empty = none). */
    mcpAllowList: readonly unknown[];
  },
): CreateSessionResult {
  const def = host.kernel.registries.agents.resolve(profile);
  if (!host.kernel.registries.providers.get(provider)) throw rpcError(-32000, `Unknown provider: ${provider}`);
  const tools = sessionTools(host, def, id, options);
  const skills = host.resolveSkills(def, tools, options);
  const systemPrompt = resolveSystemPromptLayers(def, options);
  // Progressive disclosure: with active skills, host the `load_skill` tool so
  // the model can pull full instructions on demand (catalog-only by default).
  const runTools = skills && skills.length > 0 ? [...tools, createLoadSkillTool({ registry: host.skills, tools })] : tools;
  // Plan 122: coding sessions additionally get host-owned
  // spawn/wait/cancel over a supervisor whose catalog is exactly
  // `test` + `validation`; Chat/tool-free profiles stay without.
  const supervisor = hasCodingTools(tools)
    ? sessionSupervisor(host, id, profile, provider, modelId, {
        workspaceRoot: options.workspaceRoot,
        fullAutonomy: options.fullAutonomy,
      })
    : undefined;
  const sessionToolsList = [...runTools, ...(supervisor?.tools ?? [])];
  const model: ModelConfig = host.kernel.registries.models.get(provider, modelId) ?? { provider, model: modelId };
  const { attentionCompiler, compaction } = attentionSetup(host, tools, model);
  const agent = createAgent(
    agentConfig(host, { profile, def, model, sessionToolsList, skills, systemPrompt, attentionCompiler, compaction }),
  );
  let session = agent.createSession({ id });
  if (options.observationalMemory) session = attachOm(host, session, model);
  return {
    session,
    agent,
    tools: sessionToolsList,
    systemPrompt: systemPrompt.length > 0 ? systemPrompt : undefined,
    attentionTruncation: compaction?.truncation,
    stopSubagentLifecycle: supervisor?.stop,
  };
}

/** Locked layer order (plan 117): base instructions → SYSTEM.md (user) →
 *  AGENTS.md (app). Prism's composeSystemPrompt sorts contributions by
 *  source rank (user=0, app=2), so array order here is not load-bearing;
 *  def.systemPrompt === false suppresses both layers. */
function resolveSystemPromptLayers(
  def: AgentDefinition,
  options: { workspaceRoot: string; agent: AgentRootConfig },
): SystemPromptContribution[] {
  const userSystemText = loadUserSystemPrompt(options.agent.root);
  const agentsText = loadWorkspaceAgentsPrompt(options.workspaceRoot);
  const layers: Array<{ id: string; source: "user" | "app"; text: string }> = [];
  if (userSystemText) layers.push({ id: "user-system-md", source: "user", text: userSystemText });
  if (agentsText) layers.push({ id: "agents-md", source: "app", text: agentsText });
  const profileContributions =
    def.systemPrompt === undefined || def.systemPrompt === false
      ? []
      : Array.isArray(def.systemPrompt)
        ? [...def.systemPrompt]
        : [def.systemPrompt];
  return [...layers, ...profileContributions];
}

/** Plan 121: coding sessions run Prism's attention compiler at its defaults —
 *  a per-turn, clone-only gate that stubs old thinking blocks then old tool
 *  results once the assembled input crosses 75% of the model's input cap, and
 *  raises `AttentionBudgetError` instead of silently evicting. Prism resolves
 *  the gate from the model's declared window at run start and fails closed
 *  when neither `maxInputTokens` nor `limits.contextWindow` exists, so enable
 *  it only when the resolved model declares a usable window: a limit-less
 *  discovery model (Ollama) or a pass-through model id would otherwise fail
 *  every run instead of simply skipping the optimization. `contextBudget` is
 *  never set beside the compiler (mutually exclusive); Chat/tool-free profiles
 *  and the branch-summary worker stay off. */
function attentionSetup(
  host: ClayAgentHost,
  tools: readonly ToolDefinition[],
  model: ModelConfig,
): { attentionCompiler: boolean; compaction?: { options: CompactionOptions; truncation: AttentionTruncationTrigger } } {
  // Plan 121 follow-up: auto-compaction on the same sessions as the
  // compiler — the two are Prism's designed pair (shrink at triggerRatio,
  // compact at compactRatio). Prism runs `autoCompact` only when
  // `AgentConfig.compaction` carries a trigger, so the host arms one
  // composed custom trigger (see autoCompaction). The gate is evaluated
  // once per run, before provider turns, and a throwing trigger decides
  // `false` (Prism), so a missing window can never compact on a guess.
  // `compactAfterTokens` stays inert for non-coding/limit-less sessions:
  // they keep the previous fail-closed behavior with no implicit branch
  // rewrite.
  const window = model.limits?.contextWindow;
  const attentionCompiler =
    hasCodingTools(tools) && typeof window === "number" && Number.isSafeInteger(window) && window > 0;
  return { attentionCompiler, compaction: attentionCompiler ? autoCompaction(host) : undefined };
}

function agentConfig(
  host: ClayAgentHost,
  input: {
    profile: string;
    def: AgentDefinition;
    model: ModelConfig;
    sessionToolsList: ToolDefinition[];
    skills: readonly Skill[] | undefined;
    systemPrompt: readonly SystemPromptContribution[];
    attentionCompiler: boolean;
    compaction: { options: CompactionOptions } | undefined;
  },
): Parameters<typeof createAgent>[0] {
  const { profile, def, model, sessionToolsList, skills, systemPrompt, attentionCompiler, compaction } = input;
  return {
    id: profile,
    model,
    providerSource: createProviderResolver(host.kernel.registries.providers),
    store: host.labeledStore,
    runLedger: host.persistence,
    redactor: host.redactor,
    validator: createJsonSchemaToolArgumentValidator(),
    // Coding-run ceilings from run.setOptions (see runLimits()).
    limits: runLimits(host),
    // Host-verified identity default for every session and resumed run: the
    // daemon is the trust boundary that owns the workspace root and
    // acceptance policy, so it vouches for its own runs. Durable tool
    // effects (write/edit/delete/move) require it — without it every
    // mediated write fails closed with ERR_PRISM_TOOL_EFFECT_CONFLICT,
    // including post-resume turns where run options don't reach the context.
    identity: host.runIdentity(),
    // No providerRequestPolicies — Prism 0.5.1 kernel fills session/cache
    // keys (decision 2026-09-07-2149).
    ...(attentionCompiler ? { attentionCompiler: true } : {}),
    ...(compaction ? { compaction: compaction.options } : {}),
    ...(def.instructions !== undefined ? { instructions: def.instructions } : {}),
    ...(systemPrompt.length > 0 ? { systemPrompt } : {}),
    ...(sessionToolsList.length > 0 ? { tools: sessionToolsList } : {}),
    ...(skills !== undefined ? { skills } : {}),
  };
}

/** Coding-run ceilings shared by parent sessions and their supervisor
 *  children (plan 122): Prism 0.7.0 — omit maxProviderAttempts (lifts to
 * maxTurns); pass null tokens so DEFAULT 40k/10k/50k cannot apply. Bytes
 *  are per-frame HARD (null rejected); 64 MiB is the ceiling on any single
 *  provider frame, never a run-lifetime sum (0.5.5 fixed cumulative
 *  charging). Children additionally AND-compose the supervisor's own
 *  delegation limits (steps/tools/tokens/timeout), which can only lower. */
export function runLimits(host: ClayAgentHost) {
  const runConfig: RunConfig = host.runConfig;
  return {
    maxTurns: runConfig.maxTurns,
    maxToolRounds: runConfig.maxToolRounds,
    maxToolCalls: runConfig.maxToolCalls,
    maxWallTimeMs: runConfig.maxWallTimeMs,
    maxInputTokens: runConfig.maxInputTokens,
    maxOutputTokens: runConfig.maxOutputTokens,
    maxTotalTokens: derivedTotalTokens(
      runConfig.maxInputTokens,
      runConfig.maxOutputTokens,
    ),
    maxRequestBytes: HARD_RUN_LIMITS.maxRequestBytes,
    maxResponseBytes: HARD_RUN_LIMITS.maxResponseBytes,
  };
}

/** Plan 121 follow-up: the attention compiler's auto-compact pair. The
 *  trigger ORs the truncation streak, the compiler's compactRatio, and the
 *  absolute token ceiling; the strategy stays Prism's local deterministic
 *  default (the same auto-compact shape Prism's own CLI host uses): an
 *  unattended gate must never turn a provider outage into a failed run at
 *  prompt assembly. `run.setOptions.compaction` keeps governing explicit
 *  `session.compact` / `/compact` and per-run `compaction`. One
 *  `AttentionTruncationTrigger` per agent — the event loop feeds it every
 *  `attention_compiled` event. */
function autoCompaction(host: ClayAgentHost): { options: CompactionOptions; truncation: AttentionTruncationTrigger } {
  const truncation = createAttentionTruncationTrigger();
  const compactAfterTokens = host.runConfig.compactAfterTokens;
  const trigger: CompactionTrigger = {
    type: "custom",
    // Prism memoizes both getters; `inputCapTokens` resolves exactly like
    // the compiler's own cap (same `resolveInputCap` helper).
    shouldCompact: (context) => {
      const armed = truncation.streak() >= DEFAULT_ATTENTION_TRUNCATION_THRESHOLD;
      const ratio = context.estimatedInputTokens >= context.inputCapTokens * DEFAULT_ATTENTION_COMPACT_RATIO;
      const ceiling = context.estimatedInputTokens >= compactAfterTokens;
      if (!armed && !ratio && !ceiling) return false;
      // One fire per armed streak, and the branch is fresh whichever gate
      // decided — Prism asks hosts to reset after compacting for their own
      // reasons (attention-compiler docs).
      truncation.reset();
      return true;
    },
  };
  return { options: { secrets: [...host.secrets], trigger }, truncation };
}

/** Rebuild the live session's agent with a new provider/model config
 *  (plan 108 task 9): durable runs fingerprint on AgentConfig and reject
 *  per-run model overrides, so a picker switch recreates the session.
 *  The leaf carries over so the next append chains onto the same branch. */
export function recreateSessionModel(
  host: ClayAgentHost,
  live: LiveSession,
  sessionId: string,
  provider: string,
  modelId: string,
  agent?: AgentRootConfig,
): LiveSession {
  const recreated = createSession(host, sessionId, live.profile, provider, modelId, {
    workspaceRoot: live.workspaceRoot,
    fullAutonomy: live.fullAutonomy,
    observationalMemory: live.observationalMemory,
    // Plan 118 task 35: the session's own agent config (loaded by the
    // caller); the default-agent path reuses the host's loaded values.
    agent: agent ?? host.defaultAgentConfig(),
    mcpAllowList: live.mcpAllowList,
  });
  // A model switch rebuilds the agent and with it the auto-compact trigger;
  // the truncation streak is per-agent state and starts empty (the ratio
  // and ceiling gates are re-derived from the new model's window).
  // The created session is already bound to the branch's current leaf
  // (the leaf this recreate carries over), and it is the OM-attached
  // proxy when Observational Memory is on — the Observer's post-run
  // flush and context provider must survive model switches. The prior
  // raw `agent.createSession` here silently detached OM after any
  // mid-session model switch (plan 109 I8 fix).
  const session = recreated.session;
  // A model switch rebuilds the agent and its supervisor (plan 122): stop
  // the old lifecycle pump so orphaned delegations cannot keep emitting
  // rows for the same session id.
  live.stopSubagentLifecycle?.();
  if (recreated.stopSubagentLifecycle) live.stopSubagentLifecycle = recreated.stopSubagentLifecycle;
  else delete live.stopSubagentLifecycle;
  live.session = session;
  live.agent = recreated.agent;
  live.provider = provider;
  live.model = modelId;
  live.attentionTruncation = recreated.attentionTruncation;
  return live;
}

export function resolveOmFlag(host: ClayAgentHost, profile: string, raw: unknown): boolean {
  if (raw === true) return true;
  if (raw === false) return false;
  return host.omProfiles.has(profile);
}

/** True when the profile's declared tool list includes coding tool names. */
export function profileWantsCodingTools(host: ClayAgentHost, profile: string): boolean {
  try {
    const def = host.kernel.registries.agents.resolve(profile);
    return (def.tools ?? []).some((name) => (CODING_TOOL_NAMES as readonly string[]).includes(name));
  } catch {
    return false;
  }
}

function sessionTools(
  host: ClayAgentHost,
  def: AgentDefinition,
  sessionId: string,
  options: {
    workspaceRoot: string;
    fullAutonomy: boolean;
    observationalMemory: boolean;
    agent: AgentRootConfig;
    mcpAllowList: readonly unknown[];
  },
): ToolDefinition[] {
  const built = buildSessionTools(host, def, sessionId, options) ?? [];
  // Capabilities ride along with coding sessions (they were activated for
  // the coding profile; Chat profiles with no tools stay no-tools). Plan
  // 118 task 35: the agent's own bridge, so a switch swaps its servers.
  const capabilities = built.length > 0 ? host.capabilityTools(options.agent.root) : [];
  // Opt-in wiki knowledge tools ride along with coding sessions in the
  // enabled workspace (decision 2156); absent entirely when disabled.
  const wiki = built.length > 0 ? host.wikiTools(options.workspaceRoot) : [];
  // Opt-in graft knowledge tools ride the same workspace binding.
  const graft = built.length > 0 ? host.graftTools(options.workspaceRoot) : [];
  if (!options.observationalMemory) return [...built, ...capabilities, ...wiki, ...graft];
  return [
    ...built,
    ...capabilities,
    ...wiki,
    ...graft,
    createRecallMemoryTool({
      getEntries: (sid) => host.persistence.list(sid),
      secrets: [...host.secrets],
    }),
  ];
}

/** Plan 109 I8: resolve a per-session OM worker config. The selection's
 *  provider instance must be registered — an unknown provider fails
 *  closed (worker omitted, `requireExplicitModel` skips it) and never
 *  silently falls back to the session model (decision 2158). */
function omWorkerConfig(
  host: ClayAgentHost,
  sessionId: string,
  worker: "observation" | "reflection",
):
  | {
      provider: AIProvider;
      model?: ModelConfig;
      messageTokens?: number;
      observationTokens?: number;
    }
  | undefined {
  const selection = omWorkerModels.get(sessionId)?.[worker];
  // Host-global worker config (thresholds + default provider instance);
  // the union narrows per worker below.
  const global: {
    provider: AIProvider;
    model?: ModelConfig;
    messageTokens?: number;
    observationTokens?: number;
  } | undefined =
    worker === "observation"
      ? host.omConfig?.observation
      : host.omConfig?.reflection;
  if (selection) {
    const provider = host.kernel.registries.providers.get(selection.provider);
    if (!provider) return undefined;
    // Per-session selection: the registered provider instance + the
    // selected model; thresholds still come from the host config.
    return {
      provider,
      model: { provider: selection.provider, model: selection.model },
      ...(global?.messageTokens !== undefined ? { messageTokens: global.messageTokens } : {}),
      ...(global?.observationTokens !== undefined
        ? { observationTokens: global.observationTokens }
        : {}),
    };
  }
  return global ? { ...global } : undefined;
}

function attachOm(host: ClayAgentHost, session: AgentSession, model: ModelConfig): AgentSession {
  const observation = omWorkerConfig(host, session.id, "observation");
  const reflection = omWorkerConfig(host, session.id, "reflection");
  const om = createObservationalMemory({
    ...(observation
      ? {
          observation: {
            provider: observation.provider,
            ...(observation.model ? { model: observation.model } : {}),
            ...(observation.messageTokens !== undefined ? { messageTokens: observation.messageTokens } : {}),
          },
        }
      : {}),
    ...(reflection
      ? {
          reflection: {
            provider: reflection.provider,
            ...(reflection.model ? { model: reflection.model } : {}),
            ...(reflection.observationTokens !== undefined
              ? { observationTokens: reflection.observationTokens }
              : {}),
          },
        }
      : {}),
    ...(host.omConfig?.dropper
      ? {
          dropper: {
            ...(host.omConfig.dropper.provider ? { provider: host.omConfig.dropper.provider } : {}),
            ...(host.omConfig.dropper.model ? { model: host.omConfig.dropper.model } : {}),
            ...(host.omConfig.dropper.targetTokens !== undefined
              ? { targetTokens: host.omConfig.dropper.targetTokens }
              : {}),
            ...(host.omConfig.dropper.policy ? { policy: host.omConfig.dropper.policy } : {}),
          },
        }
      : {}),
    context: { compactAfterTokens: host.omConfig?.compactAfterTokens ?? DEFAULT_COMPACT_AFTER_TOKENS },
    settings: omSettingsProvider(session.id),
    secrets: [...host.secrets],
    requireExplicitModel: true,
  });
  return om.attach(session, {
    appendEntry: (entry, appendOptions) => host.persistence.append(entry, appendOptions),
    sessionModel: model,
  }).session;
}

export function resolveCompaction(host: ClayAgentHost, name: string, live: LiveSession) {
  if (!isCompactionStrategyName(name)) throw rpcError(-32602, `unknown compaction strategy: ${name}`);
  const provider = host.kernel.registries.providers.get(live.provider);
  return createNamedCompactionStrategy(name, {
    secrets: [...host.secrets],
    ...(provider ? { llm: { provider, model: { provider: live.provider, model: live.model } } } : {}),
  });
}

/**
 * Resolve a profile's tools. Names in CODING_TOOL_NAMES build fresh
 * per-session coding tools bound to the session workspace + autonomy flag;
 * everything else resolves from the kernel registry (Chat profiles with no
 * tools stay no-tools).
 */
function buildSessionTools(
  host: ClayAgentHost,
  def: AgentDefinition,
  sessionId: string,
  options: { workspaceRoot: string; fullAutonomy: boolean },
): ToolDefinition[] | undefined {
  if (!def.tools) return undefined;
  const coding = def.tools.filter((name) => (CODING_TOOL_NAMES as readonly string[]).includes(name));
  const rest = def.tools.filter((name) => !(CODING_TOOL_NAMES as readonly string[]).includes(name));
  if (coding.length === 0) {
    return rest.length > 0 ? rest.map((name) => host.kernel.registries.tools.resolve(name)) : undefined;
  }
  const tools = sessionCodingTools(host, sessionId, options);
  return [...rest.map((name) => host.kernel.registries.tools.resolve(name)), ...tools];
}

/** The session's coding tool set over Clay document reverse-RPC (plan 122
 *  extraction): shared verbatim by supervisor children, which reuse the
 *  PARENT sessionId + workspaceRoot so document ops stay on the daemon→
 *  server registry — the server resolves the workspace from the parent
 *  session id and a child id would fail closed (agent_documents). */
export function sessionCodingTools(
  host: ClayAgentHost,
  sessionId: string,
  options: { workspaceRoot: string; fullAutonomy: boolean },
): ToolDefinition[] {
  return buildCodingTools({
    sessionId,
    workspaceRoot: options.workspaceRoot,
    request: (method, params) => host.request(method, params),
    fullAutonomy: () => host.live.get(sessionId)?.fullAutonomy ?? options.fullAutonomy,
    toolCaps: host.toolCaps,
    capsFile: host.capsFile,
    approve: (action) =>
      host.request("approval.request", {
        action: { kind: action.kind, operation: action.operation, paths: action.paths, command: action.command },
      }).then(
        () => true,
        () => false,
      ),
    ask: async (request) => {
      const answer = (await host.request("approval.askUserDecision", {
        question: request.question,
        options: request.options,
        selectionMode: request.selectionMode,
        allowCustom: request.allowCustom,
        toolCallId: request.toolCallId,
        sessionId: request.sessionId,
        runId: request.runId,
      })) as AskUserDecisionAnswer;
      return answer;
    },
  });
}

/** Plan 122: host-owned spawn/wait/cancel over one Prism supervisor whose
 *  catalog is exactly `test` and `validation` (the Phase 6 loop shapes —
 *  this is the primitive, not the orchestrator). The supervisor is
 *  in-process and per live coding session; async handles die on daemon
 *  restart (Prism contract — documented, never faked durable). Children
 *  narrow from the host run identity (Prism narrowIdentity +
 *  assertIdentityPropagation — the closed spawn schema cannot widen
 *  tenant/scopes/expiry) and their results pass the parent redactor. */
function sessionSupervisor(
  host: ClayAgentHost,
  sessionId: string,
  profile: string,
  provider: string,
  modelId: string,
  options: { workspaceRoot: string; fullAutonomy: boolean },
): { tools: ToolDefinition[]; stop: () => void } {
  const child = () => createChildAgent(host, profile, provider, modelId, sessionId, options);
  const supervisor = createSupervisor({
    ownership: host.runOwnership(),
    identity: host.runIdentity(),
    redactor: host.redactor,
    children: {
      test: { createAgent: child },
      validation: { createAgent: child },
    },
    // Plan 123: each delegation nests as a `child` scope under the run
    // scope that owns it (no-op without an active scoped run).
    hooks: host.omSpawnScopes(sessionId),
  });
  // Supervisor milestones bridge onto the coding lifecycle the Rust
  // mapper already renders as ordinary Tool rows (`subagent_started` /
  // `subagent_stopped`) — no new panel or AG-UI event type.
  const stop = observeSupervisorLifecycle(supervisor, {
    onEvent: (event) => host.emit("event", { sessionId, event }),
  });
  return {
    tools: [
      createSpawnAgentTool({ supervisor }),
      createWaitAgentTool({ supervisor }),
      createCancelAgentTool({ supervisor }),
    ],
    stop,
  };
}

/** Plan 122 child factory: an isolated Prism agent (own history — the
 *  supervisor derives the `${delegationId}-session` id) on the parent's
 *  provider/model, with the parent's coding tools rebuilt against the
 *  PARENT sessionId + workspaceRoot. No spawn/wait/cancel → children
 *  cannot recurse. No git worktrees: a child `cwd` outside the registered
 *  session workspace would starve document RPC (plan 122 compromise —
 *  upgrade via createWorktreeChildFactory only once a child can own a
 *  registered workspace root). Identity is supplied by the supervisor
 *  (narrowed run identity), never here. */
function createChildAgent(
  host: ClayAgentHost,
  profile: string,
  provider: string,
  modelId: string,
  sessionId: string,
  options: { workspaceRoot: string; fullAutonomy: boolean },
): Agent {
  const model = host.kernel.registries.models.get(provider, modelId) ?? { provider, model: modelId };
  return createAgent({
    id: `${profile}:child`,
    model,
    providerSource: createProviderResolver(host.kernel.registries.providers),
    store: host.labeledStore,
    runLedger: host.persistence,
    redactor: host.redactor,
    validator: createJsonSchemaToolArgumentValidator(),
    limits: runLimits(host),
    tools: sessionCodingTools(host, sessionId, options),
  });
}
