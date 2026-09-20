/**
 * Shared contract for the ClayAgentHost concern modules (plan 130 task 2):
 * the types, constants, pure helpers, and module-scoped state that two or
 * more `host/*.ts` modules (or `host.ts` itself) need. Concern-specific
 * code lives in its owning module; only cross-concern basics live here.
 *
 * The host class (`host.ts`) is the facade: it owns the mutable state and
 * the RPC dispatch; modules hold the logic as functions taking the host
 * instance explicitly. `import type { ClayAgentHost }` in the modules is
 * erased at compile time, so there is no runtime import cycle.
 */
import type { AIProvider, ModelConfig, SessionEntry, SettingsProvider, ToolDefinition, ExtensionKernel, Skill, AgentSession, Agent, SystemPromptConfig, AttentionTruncationTrigger } from "@arnilo/prism";
import type { WorkScopeController } from "@arnilo/prism-memory/compaction/observational-memory";
import type { CompactionStrategyName } from "../compaction.js";
import { CODING_TOOL_NAMES, type RepositoryToolCaps } from "../coding-tools.js";
import type { SqlitePersistence } from "@arnilo/prism-core/sessions/sqlite";
import type { KeychainCredentialStore } from "@arnilo/prism-core/credentials/node";
import type { SkillsConfig } from "./skills.js";

export type EmitFn = (method: string, params: unknown) => void;

/** Policy cap: finite positive integer or `null` (Prism 0.7.0: disable that axis). */
export type PolicyCap = number | null;

export interface RunConfig {
  maxTurns: PolicyCap;
  maxToolRounds: PolicyCap;
  maxToolCalls: PolicyCap;
  maxWallTimeMs: PolicyCap;
  maxInputTokens: PolicyCap;
  maxOutputTokens: PolicyCap;
  compactAfterTokens: number;
  compaction: CompactionStrategyName;
}

/** Auto-compaction ceiling (`run.setOptions.compactAfterTokens`): one of the
 *  three gates of the trigger `autoCompaction()` arms on compiler sessions. */
export const DEFAULT_COMPACT_TRIGGER_TOKENS = 800_000;

/** Coding envelope (Prism 0.7.0): fence axes disabled (`null`); the only
 *  hard caps left are Prism's per-frame request/response bytes. A host that
 *  wants a fence sets finite values via `run.setOptions`. */
export const DEFAULT_RUN_CONFIG: RunConfig = {
  maxTurns: null,
  maxToolRounds: null,
  maxToolCalls: null,
  maxWallTimeMs: null,
  maxInputTokens: null,
  maxOutputTokens: null,
  compactAfterTokens: DEFAULT_COMPACT_TRIGGER_TOKENS,
  compaction: "llm",
};

/** Per-session OM auto-compaction threshold override (2158 default 80000).
 *  Only meaningful for OM-attached sessions.
 *  ponytail: process-global keyed by sessionId (matches pre-split behavior);
 *  move onto the host instance if multi-host processes ever matter. */
export const omCompactAfterTokens = new Map<string, number>();

/** Plan 109 I8: per-session OM worker models (may differ from the session
 *  model; retained per session in record metadata, per workspace in the
 *  server book). Absent fields fall back to the host-global config.
 *  ponytail: process-global keyed by sessionId (matches pre-split behavior);
 *  move onto the host instance if multi-host processes ever matter. */
export interface OmWorkerSelection {
  readonly provider: string;
  readonly model: string;
}
export interface OmWorkerModels {
  observation?: OmWorkerSelection;
  reflection?: OmWorkerSelection;
}
export const omWorkerModels = new Map<string, OmWorkerModels>();

export const MAX_QUEUED_EVENTS = 256;
export const MAX_LIST = 50;
export const MAX_LOAD_ENTRIES = 200;
export const MAX_SESSION_SEARCH_LIMIT = 100;
/** Plan 109 R1: bounded completion surface (commands and description
 *  lengths) — names + descriptions only, never handlers or arg shapes. */
export const MAX_COMPLETION_COMMANDS = 64;
export const MAX_COMPLETION_NAME_CHARS = 48;
export const MAX_COMPLETION_DESCRIPTION_CHARS = 96;
const SESSION_LABEL_WORDS = 5;
const SESSION_LABEL_MAX_BYTES = 120;
export const TENANT = "clay";
export const REVERSE_TIMEOUT_MS = 30_000;

export interface HostOptions {
  readonly dataDir: string;
  readonly passphrase: string;
  readonly mock?: boolean;
  readonly mockProvider?: AIProvider;
  /** Test/extension seam (plan 109 I4): register extra model configs
   *  (capabilities/compat) on the kernel's model registry. */
  readonly registerModels?: (registries: ExtensionKernel["registries"]) => void;
  /** Test seam (plan 109 I7): register extra agent definitions (profiles
   *  with system prompts) after the built-in Chat profile. */
  readonly registerAgents?: (registries: ExtensionKernel["registries"]) => void;
  readonly emit?: EmitFn;
  /**
   * Server-built MCP allow-list (decision 1758). Absent/empty connects
   * nothing. Entries are validated fail-closed (canonical executable,
   * literal argv, explicit env names).
   */
  readonly mcpAllowList?: readonly unknown[];
  /** Test seam: replace keychain store construction (default Prism node keychain). */
  readonly createKeychain?: () => KeychainCredentialStore;
  /** Test seam: replace Obscura binary resolution. */
  readonly resolveObscuraBinary?: () => string | undefined;
  /** Coding-agent config root (decision 2026-09-09-1420):
   *  `~/.clay/agents/coding-agent`. Holds `skills.json`, and the
   *  `skills/` directory (seeded agent-delivered + user-added skills).
   *  Test seam: tests point this at a temp dir. */
  readonly agentConfigRoot?: string;
  /** Home skill-discovery root (npx skills format). Default `~/.agents`;
   *  scanned as `<root>/skills/`. Test seam. */
  readonly homeSkillsRoot?: string;
  /** Default graft CLI path for the daemon-initiated graft binding (plan
   *  117: graft available by default). Absent ⇒ peer-package resolution
   *  only, fail-closed. Test seam: point at a stub to exercise the
   *  default bind deterministically. */
  readonly graftCliPath?: string;
  /** Worker models stay off the session model (2158). Omit to skip workers. */
  readonly observationalMemory?: {
    readonly observation?: {
      readonly provider: AIProvider;
      readonly model?: ModelConfig;
      readonly messageTokens?: number;
    };
    readonly reflection?: {
      readonly provider: AIProvider;
      readonly model?: ModelConfig;
      /** Reflection trigger: uncovered-observation token budget. */
      readonly observationTokens?: number;
    };
    /** Dropper config passthrough (plan 109 I8): `policy: "lowest-relevance"`
     *  drops deterministically without a model call; the default "model"
     *  policy resolves like the other workers (per-session selection may
     *  name it, else it skips). */
    readonly dropper?: {
      readonly provider?: AIProvider;
      readonly model?: ModelConfig;
      readonly targetTokens?: number;
      readonly policy?: "model" | "lowest-relevance";
    };
    readonly compactAfterTokens?: number;
  };
}

/** The live-session book entry: everything a created or restored session
 *  carries. State stays on the host (facade); concern modules operate on
 *  it through the host instance. */
export interface LiveSession {
  session: AgentSession;
  agent: Agent;
  readonly profile: string;
  /** Mutable: provider/model pickers switch mid-session (plan 108 task 9). */
  provider: string;
  model: string;
  readonly workspaceRoot: string;
  /** Plan 118 task 35: the agent type this session runs as, and the config
   *  root it resolved to (`~/.clay/agents/<type>`). A switch rewrites both
   *  through `session.setAgent`, which keeps the session id and its branch
   *  and re-reads only that agent's config. `agentType` is undefined for the
   *  daemon's default agent (a session created without an agent). */
  agentType: string | undefined;
  agentRoot: string;
  /** Server-built MCP allow-list for this session's agent (plan 118 task
   *  35): each agent type declares its own servers. */
  mcpAllowList: readonly unknown[];
  /** Plan 117 @-mentions: skills manually triggered this session. They ride
   *  every run's `skills` option (profile config union) so their loaded
   *  bodies keep rendering across turns. */
  readonly mentionSkills: Skill[];
  /** Host-settable: on unless the caller blocks it (decision
   *  2026-09-20-2049); toggled via session.setAutonomy. */
  fullAutonomy: boolean;
  readonly observationalMemory: boolean;
  /** Live tools for this session (empty for Chat without OM). */
  readonly tools: ToolDefinition[];
  /** Definition-owned system prompt config (plan 109 I7 context
   *  inspector); `undefined` = the profile contributes no explicit prompt. */
  readonly systemPrompt?: SystemPromptConfig;
  /** Last durable-run suspension: expectedVersion source for run.resume
   *  when the client omits it, plus the pending approval ids. */
  suspension?: { runId: string; version: number };
  /** Plan 121 follow-up: armed only when the attention compiler is on;
   *  `session.prompt` feeds it every `attention_compiled` event so the next
   *  prompt-boundary auto-compact can act on a truncated streak. */
  attentionTruncation?: AttentionTruncationTrigger;
  /** Plan 122: stops this session's supervisor lifecycle pump (subagent
   *  Started/Finished rows). Stopped on delete, model rebuild, shutdown. */
  stopSubagentLifecycle?: () => void;
  /** Plan 123: prompts already scoped on this branch (seeded lazily from
   *  the branch's `om.scope.opened` entries so a resume never reuses an id).
   *  ponytail: the lazy seed races only if two callers allocate before the
   *  first seed resolves — unreachable while Clay keeps Prism's default
   *  `toolConcurrency` of 1 and one prompt per session; chain allocations if
   *  parallel dispatch ever lands. */
  omRunCount?: number;
  /** Plan 123: spawn-child scopes already opened on this branch. Seeded like
   *  `omRunCount` — Prism's own delegation ids restart with the supervisor
   *  (every model switch rebuilds it), so they cannot key the ledger. */
  omChildCount?: number;
  /** Plan 123: one work-scope controller per OM-attached session, shared by
   *  the run scope and its spawn children. Keyed on the session object it
   *  appends through: a model switch swaps in a fresh OM proxy. */
  omScopes?: { session: AgentSession; controller: WorkScopeController };
}

export function rpcError(code: number, message: string, data?: unknown): Error & { rpcCode: number; data?: unknown } {
  return Object.assign(new Error(message), { rpcCode: code, data });
}

/** Plan 118 task 35: one agent type's config, loaded once per root.
 *  Everything the daemon reads per agent — tool caps, skills config, the
 *  seeded skill files, and the disk-discovered skill names of that root's
 *  `skills/` — lives here, so a session is built entirely from its own
 *  agent's files and a switch cannot widen one agent's grants with
 *  another's. */
export interface AgentRootConfig {
  readonly root: string;
  readonly toolCaps: { caps: RepositoryToolCaps; file: string };
  readonly skillsConfig: SkillsConfig;
  readonly configSkillNames: readonly string[];
  readonly skillFiles: ReadonlyMap<string, Skill>;
}

/** Plan 121: a coding session is one whose active tools include any Prism
 *  coding tool — the same predicate that arms durable runs and the attention
 *  compiler. */
export function hasCodingTools(tools: readonly ToolDefinition[]): boolean {
  return tools.some((tool) => (CODING_TOOL_NAMES as readonly string[]).includes(tool.name));
}

export function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw rpcError(-32602, "params must be an object");
  }
  return value as Record<string, unknown>;
}

export function reqString(params: Record<string, unknown>, key: string): string {
  const value = params[key];
  if (typeof value !== "string" || value.length === 0) throw rpcError(-32602, `params.${key} is required`);
  return value;
}

export function optString(params: Record<string, unknown>, key: string): string | undefined {
  const value = params[key];
  if (value === undefined) return undefined;
  if (typeof value !== "string") throw rpcError(-32602, `params.${key} must be a string`);
  return value;
}

export function optPolicyCap(params: Record<string, unknown>, key: string): PolicyCap | undefined {
  const value = params[key];
  if (value === undefined) return undefined;
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw rpcError(-32602, `${key} must be a positive safe integer or null`);
  }
  return value;
}

export function optPositiveInt(params: Record<string, unknown>, key: string): number | undefined {
  const value = params[key];
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw rpcError(-32602, `${key} must be a positive safe integer`);
  }
  return value;
}

export function derivedTotalTokens(input: PolicyCap, output: PolicyCap): PolicyCap {
  if (input === null || output === null) return null;
  const sum = input + output;
  return Number.isSafeInteger(sum) && sum >= 1 ? sum : null;
}

export function clipUtf8(text: string, maxBytes: number): string {
  let end = Math.min(text.length, maxBytes);
  while (end > 0 && Buffer.byteLength(text.slice(0, end), "utf8") > maxBytes) end -= 1;
  return text.slice(0, end);
}

/**
 * Plan 117 follow-up: stamp the session's opening prompt onto its first
 * entry's `label`, so `/resume` can identify a row by topic instead of by its
 * profile (identical for every session in a workspace).
 *
 * Only the first entry is eligible (`parentId` unset, first user message):
 * Prism's session search reads the newest non-null label, so one stamp gives
 * the session a stable identity for its whole life, and a store written by an
 * older daemon simply has no label (the picker falls back to "Untitled
 * session").
 */
export function labelFirstPromptEntry(entry: SessionEntry): SessionEntry {
  if (entry.label !== undefined || entry.parentId !== undefined) return entry;
  const message = entry.message;
  if (!message || message.role !== "user") return entry;
  const text = message.content
    .filter((block): block is { type: "text"; text: string } => block.type === "text")
    .map((block) => block.text)
    .join(" ");
  const words = text.split(/\s+/).filter(Boolean).slice(0, SESSION_LABEL_WORDS);
  if (words.length === 0) return entry;
  const label = words.join(" ").slice(0, SESSION_LABEL_MAX_BYTES);
  return { ...entry, label };
}

/**
 * Plan 118 task 35: stamp the agent type that produced an entry into its
 * metadata. One tab can change its agent, so a transcript spanning a switch
 * must label each turn with its producer; the Rust server reads
 * `metadata.agentType` per entry (and falls back to the session's recorded
 * agent for entries written before the stamp existed). Sessions with no agent
 * type (the daemon default) are left untouched.
 */
export function agentStampedEntry(entry: SessionEntry, agentType: string | undefined): SessionEntry {
  if (!agentType || entry.metadata?.agentType === agentType) return entry;
  return { ...entry, metadata: { ...(entry.metadata ?? {}), agentType } };
}

/** The session store Prism writes through, with the resume label stamped on
 *  the opening entry. Read paths stay on the underlying store. */
export function labelFirstPromptStore(
  store: SqlitePersistence,
  agentOf: (sessionId: string) => string | undefined,
): SqlitePersistence {
  return new Proxy(store, {
    get(target, property, receiver) {
      if (property !== "append") return Reflect.get(target, property, receiver);
      return (entry: SessionEntry, options?: unknown) =>
        target.append(agentStampedEntry(labelFirstPromptEntry(entry), agentOf(entry.sessionId)), options as never);
    },
  });
}

/** Persist the live book's provider/model onto the session record (plan 108
 *  task 9): a picker switch mid-session survives daemon restart and resume.
 *  Best-effort — a store without appendSession skips persistence. */
export async function persistProviderModel(
  persistence: SqlitePersistence,
  sessionId: string,
  provider: string,
  model: string,
): Promise<void> {
  if (typeof persistence.appendSession !== "function") return;
  try {
    const page = await persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
    const record = page.items[0];
    if (!record) return;
    const metadata = { ...(record.metadata ?? {}), provider, model };
    await persistence.appendSession({
      ...record,
      metadata,
      updatedAt: new Date().toISOString(),
    });
  } catch {
    // Metadata persistence is advisory; the run-scoped override still applies.
  }
}

export function omSettingsProvider(sessionId: string): SettingsProvider {
  return {
    get<T = unknown>(key: string): T | undefined {
      if (key !== "observational-memory") return undefined;
      const threshold = omCompactAfterTokens.get(sessionId);
      // Nested form; flat keys fail closed in resolveObservationalMemorySettings.
      return threshold === undefined
        ? undefined
        : ({ context: { compactAfterTokens: threshold } } as T);
    },
  };
}
