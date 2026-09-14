import { access, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { readFileSync, realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, sep } from "node:path";
import { randomUUID } from "node:crypto";
import {
  type Agent,
  type AgentDefinition,
  type AgentEvent,
  type AgentRunInterruption,
  type AgentRunResume,
  type AgentRunStateOptions,
  type AgentSession,
  type AIProvider,
  type AuthMethod,
  type CommandDefinition,
  type CommandDrivers,
  type CommandExecutionContext,
  type CommandResult,
  type ExtensionKernel,
  type JsonObject,
  type LoadedExtension,
  type Message,
  type ContentBlock,
  type ModelConfig,
  type OAuthAuthMethod,
  type OAuthCredentials,
  type RunOptions,
  type RunDecision,
  type SecretRedactor,
  type SessionEntry,
  type Skill,
  type ToolDefinition,
  createAgent,
  createExplicitCredentialResolver,
  createExtensionKernel,
  createLoadSkillTool,
  createMemorySessionStore,
  createMockProvider,
  createProviderResolver,
  createSecretRedactor,
  createSessionEntry,
  createSkillRegistry,
  DEFAULT_SESSION_SEARCH_LIMIT,
  HARD_RUN_LIMITS,
  listSessionBranches,
  SESSION_SEARCH_WORKSPACE_METADATA_KEY,
  providerDone,
  providerTextDelta,
  redactAgentEvent,
  resolveActiveSkills,
  resumeAgentRunStream,
  parseThinkingLevel,
  thinkingLevelsForModel,
} from "@arnilo/prism";
import {
  createKeychainCredentialStore,
  createStoredCredentialResolver,
  openEncryptedCredentialStore,
  type EncryptedCredentialStore,
  type KeychainCredentialStore,
  CredentialStoreLockedError,
} from "@arnilo/prism-core/credentials/node";
import { discoverContributions } from "@arnilo/prism/node/contribution-discovery";
import { parseSkillFile } from "@arnilo/prism";
import { createSqlitePersistence, type SqlitePersistence } from "@arnilo/prism-core/sessions/sqlite";
import { createJsonSchemaToolArgumentValidator } from "@arnilo/prism-core/validation/json-schema";
import type { AskUserDecisionAnswer } from "@arnilo/prism-coding-tools/agent";
import {
  createObservationalMemory,
  createRecallMemoryTool,
} from "@arnilo/prism-memory/compaction/observational-memory";
import type { SettingsProvider, SystemPromptConfig } from "@arnilo/prism";
import type { AgentIdentity } from "@arnilo/prism";
import { ownershipFromIdentity } from "@arnilo/prism";
import { CODING_TOOL_NAMES, buildCodingTools, normalizeToolCaps, type RepositoryToolCaps } from "./coding-tools.js";
import {
  connectAllowListedMcpServers,
  MAX_MCP_SERVERS,
  type ConnectedMcpServers,
  type McpServerOutcome,
} from "./mcp.js";
import { resolveObscuraBinary, spawnObscuraHarness, type ObscuraHarness } from "./obscura.js";
import {
  DEFAULT_COMPACT_AFTER_TOKENS,
  createNamedCompactionStrategy,
  isCompactionStrategyName,
  registerCompactionStrategies,
  type CompactionStrategyName,
} from "./compaction.js";
import {
  WIKI_READ_PAGE_TOOL_NAME,
  WIKI_RECORD_INSIGHT_TOOL_NAME,
  WIKI_SEARCH_TOOL_NAME,
  createWikiExtension,
  wikiMaintainerSkill,
  wikiSearcherSkill,
  type WikiExtensionOptions,
} from "@arnilo/prism-memory/wiki";
import {
  createGraftExtension,
  resolveGraftCli,
  type GraftExtensionOptions,
  type GraftMode,
} from "@arnilo/prism-memory/graft";

/** Auto-compact trigger. Stored by run.setOptions; unused until wired. */
const DEFAULT_COMPACT_TRIGGER_TOKENS = 800_000;

/** Policy cap: finite positive integer or `null` (Prism 0.5.5: disable that axis). */
type PolicyCap = number | null;

interface RunConfig {
  maxTurns: PolicyCap;
  maxToolRounds: PolicyCap;
  maxToolCalls: PolicyCap;
  maxWallTimeMs: PolicyCap;
  maxInputTokens: PolicyCap;
  maxOutputTokens: PolicyCap;
  compactAfterTokens: number;
  compaction: CompactionStrategyName;
}

/** Coding envelope (Prism 0.5.5): fence axes disabled (`null`); the only
 *  hard caps left are Prism's per-frame request/response bytes. A host that
 *  wants a fence sets finite values via `run.setOptions`. */
const DEFAULT_RUN_CONFIG: RunConfig = {
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
 *  Only meaningful for OM-attached sessions. */
const omCompactAfterTokens = new Map<string, number>();
/** Plan 109 I8: per-session OM worker models (may differ from the session
 *  model; retained per session in record metadata, per workspace in the
 *  server book). Absent fields fall back to the host-global config. */
interface OmWorkerSelection {
  readonly provider: string;
  readonly model: string;
}
interface OmWorkerModels {
  observation?: OmWorkerSelection;
  reflection?: OmWorkerSelection;
}
const omWorkerModels = new Map<string, OmWorkerModels>();

/** Graft pull-tool names registered by the @arnilo/prism-memory/graft
 *  extension (tools.js); the package does not export the list. */
const GRAFT_TOOL_NAMES = [
  "graft_ask",
  "graft_grep",
  "graft_callers",
  "graft_skeleton",
  "graft_map",
  "graft_blast",
] as const;

/** Host-side graft skill copy (the package's own skill body is not part of
 *  its export map). Registered while the graft option is bound; the
 *  skillList catalog filters it out when the option is off. */
const graftSkill = {
  name: "graft",
  description:
    "Use the graft context graph to locate code by architecture, callers, and coupling before spelunking.",
  instructions: [
    "Query the repo's graft context graph instead of grepping blindly.",
    "- `graft_map`: orient first — dir clusters, hubs, hotspots (no API key).",
    "- `graft_ask` (question, --source for code spans) and `graft_grep` (literal identifier): find where code lives and how it works.",
    "- `graft_callers` (exact symbol) gives exact call edges; `--depth N` walks transitively for blast radius.",
    "- `graft_skeleton <file>` skims a file's API surface ~10x cheaper than reading it.",
    "- `graft_blast <path>` after an edit lists dependents worth re-checking.",
    "Graph output is agent-aid, not authority: verify against the source at the cited file:line before editing. Run `graft build` (or /graft-build) after big changes to refresh the graph.",
  ].join("\n"),
};

/** Agent-delivered skills are file-backed (plan 117): the in-code objects
 *  above are generators — `<agentConfigRoot>/skills/<name>/SKILL.md` is the
 *  delivered payload. Seeded when absent (deleted ⇒ regenerated at every
 *  start), parsed at every start (user edits apply on next daemon start),
 *  and falling back to the built-in content on malformed/oversized files —
 *  never a broken skill. `name` and `toolNames` stay daemon-owned: the file
 *  cannot rename a skill, grant tools, or break the fail-closed toolName
 *  check. Disk discovery skips these reserved names, so seeding + gated
 *  activation is the only delivery path. */
const MAX_AGENT_SKILL_FILE_BYTES = 256 * 1024;

/** Plan 118 task 35: agent types are directory names, never paths. Bounded
 *  and separator-free, so a name can only ever address a direct child of the
 *  `agents/` root the default agent lives in. */
const AGENT_TYPE_RE = /^[A-Za-z0-9_-]{1,64}$/;

/** Loaded agent roots kept in memory (config is small; the cap is abuse
 *  protection for a hand-made `agents/` folder). */
const MAX_AGENT_ROOTS = 16;
/** Plan 117 @-mentions: attached files cap at the same 256 KiB discipline as
 *  agent skill files; the workspace listing for the mention dropdown is
 *  bounded in entries and walk depth (client-side filter, debounced fetch). */
const MAX_MENTION_FILE_BYTES = 256 * 1024;
const MAX_LISTED_FILES = 200;
const MAX_LIST_WALK_DEPTH = 8;
const LIST_SKIPPED_DIRS = new Set([".git", "node_modules", "target", "dist", "build"]);
const IMAGE_MIME_BY_EXT = new Map([
  [".png", "image/png"],
  [".jpg", "image/jpeg"],
  [".jpeg", "image/jpeg"],
  [".gif", "image/gif"],
  [".webp", "image/webp"],
  [".bmp", "image/bmp"],
]);
const AGENT_DELIVERED_SKILLS: readonly { dir: string; skill: Skill }[] = [
  { dir: "wiki-searcher", skill: wikiSearcherSkill },
  { dir: "wiki-maintainer", skill: wikiMaintainerSkill },
  { dir: "graft", skill: graftSkill },
];

/** Render a built-in skill as its seeded SKILL.md (frontmatter + body). */
function skillToSeedMarkdown(skill: { name: string; description?: string; instructions?: string }): string {
  return `---\nname: ${skill.name}\ndescription: ${skill.description ?? ""}\n---\n\n${skill.instructions ?? ""}\n`;
}

/** Seed provenance stamp (plan 117): after writing a seed file, record its
 *  size + mtime in `<agentConfigRoot>/.seed-manifest.json` so the Rust
 *  settings listing can badge built-in-vs-edited without knowing the built-in
 *  content. A missing/stale stamp marks the file edited — honest fallback. */
async function recordSeedStamp(agentConfigRoot: string, relName: string): Promise<void> {
  const manifestFile = join(agentConfigRoot, ".seed-manifest.json");
  let manifest: Record<string, { sizeBytes: number; mtimeMs: number }> = {};
  try {
    manifest = JSON.parse(await readFile(manifestFile, "utf8")) as typeof manifest;
  } catch {
    // Absent or malformed: start a fresh manifest (seeds are being rebuilt).
  }
  try {
    const stats = await stat(join(agentConfigRoot, relName));
    // Truncate, not round: the Rust listing derives the file's mtime with
    // `as_millis()` (also truncating), so a rounded stamp is off by up to
    // +1 ms and every untouched seed badges as "edited".
    manifest[relName] = { sizeBytes: stats.size, mtimeMs: Math.trunc(stats.mtimeMs) };
    await writeFile(manifestFile, JSON.stringify(manifest, null, 2), "utf8");
  } catch {
    // Unstamped seed: the settings page shows it as edited — acceptable.
  }
}

/** Seed absent agent-delivered SKILL.md files; present files stay untouched
 *  (the file is the source of truth — user edits are preserved). */
async function seedAgentSkillFiles(agentConfigRoot: string): Promise<void> {
  const skillsDir = join(agentConfigRoot, "skills");
  for (const { dir, skill } of AGENT_DELIVERED_SKILLS) {
    const file = join(skillsDir, dir, "SKILL.md");
    try {
      await access(file);
      continue;
    } catch {
      // Absent: seed it (first launch or user deletion).
    }
    await mkdir(join(skillsDir, dir), { recursive: true });
    await writeFile(file, skillToSeedMarkdown(skill), "utf8");
    await recordSeedStamp(agentConfigRoot, `skills/${dir}/SKILL.md`);
  }
}

/** Load the delivered skill content from disk. Every registration site
 *  (wiki enable, graft bind) registers from this map, never the in-code
 *  objects directly. Seeding runs first, so a deleted file is delivered
 *  from its fresh seed in the same start. */
async function loadAgentSkillFiles(agentConfigRoot: string): Promise<ReadonlyMap<string, Skill>> {
  const skillsDir = join(agentConfigRoot, "skills");
  try {
    await seedAgentSkillFiles(agentConfigRoot);
  } catch (error) {
    process.stderr.write(
      `[skills] seeding agent skill files failed: ${error instanceof Error ? error.message : String(error)}\n`,
    );
  }
  const files = new Map<string, Skill>();
  for (const { dir, skill } of AGENT_DELIVERED_SKILLS) {
    const file = join(skillsDir, dir, "SKILL.md");
    let text: string;
    try {
      text = await readFile(file, "utf8");
    } catch {
      // Seeding failed (unwritable config root) — deliver the built-in content.
      files.set(dir, skill);
      continue;
    }
    try {
      if (Buffer.byteLength(text, "utf8") > MAX_AGENT_SKILL_FILE_BYTES) {
        throw new Error(`file exceeds ${MAX_AGENT_SKILL_FILE_BYTES} bytes`);
      }
      const parsed = parseSkillFile(text, file);
      // Name and toolNames are daemon-owned; the file carries description + body.
      files.set(dir, { ...parsed, name: dir, toolNames: skill.toolNames });
    } catch (error) {
      process.stderr.write(
        `[skills] ${file} unusable (${error instanceof Error ? error.message : String(error)}); using built-in seed content\n`,
      );
      files.set(dir, skill);
    }
  }
  return files;
}

/** User-owned global system-prompt layer (plan 117): seeded EMPTY at host
 *  creation (a default body would pollute every session's prompt — the file
 *  exists so the user has a stable home for global instructions), read per
 *  session build (sync, bounded — edits apply on the next session), and
 *  injected as a `user`-source system-prompt contribution so Prism's source
 *  rank orders it right after the profile base instructions and before the
 *  workspace AGENTS.md app layer. Byte-stable per session, so it rides the
 *  cached prefix. Oversized/unreadable ⇒ skipped with a warning, never a
 *  broken session. */
const MAX_PROMPT_LAYER_BYTES = 64 * 1024;
const USER_SYSTEM_PROMPT_FILE = "SYSTEM.md";
const WORKSPACE_AGENTS_FILE = "AGENTS.md";

async function seedUserSystemPrompt(agentConfigRoot: string): Promise<void> {
  const file = join(agentConfigRoot, USER_SYSTEM_PROMPT_FILE);
  try {
    await access(file);
    return;
  } catch {
    // Absent: seed it (first launch or user deletion).
  }
  await writeFile(file, "", "utf8");
  await recordSeedStamp(agentConfigRoot, USER_SYSTEM_PROMPT_FILE);
}

/** Shared bounded prompt-layer reader (plan 117): trimmed text, "" when
 *  absent, throws on unreadable/oversized so each caller applies its own
 *  trust policy (user-owned SYSTEM.md warns; repo AGENTS.md skips). */
function readPromptLayer(file: string, maxBytes: number): string {
  let text: string;
  try {
    text = readFileSync(file, "utf8");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return "";
    throw error;
  }
  if (Buffer.byteLength(text, "utf8") > maxBytes) {
    throw new Error(`file exceeds ${maxBytes} bytes`);
  }
  return text.trim();
}

function loadUserSystemPrompt(agentConfigRoot: string): string {
  const file = join(agentConfigRoot, USER_SYSTEM_PROMPT_FILE);
  try {
    return readPromptLayer(file, MAX_PROMPT_LAYER_BYTES);
  } catch (error) {
    process.stderr.write(
      `[system] ${file} unusable (${error instanceof Error ? error.message : String(error)}); skipping user layer\n`,
    );
    return "";
  }
}

/** Workspace `AGENTS.md` app layer (plan 117): repo content is untrusted
 *  prompt text — bounded bytes, symlink-escape excluded (the file's
 *  realpath must stay inside the workspace root), and a silent skip when
 *  absent (most repos have none; ENOENT is not a warning). Byte-stable per
 *  session; the `app` source rank composes it after the user SYSTEM.md
 *  layer. */
function loadWorkspaceAgentsPrompt(workspaceRoot: string): string {
  const file = join(workspaceRoot, WORKSPACE_AGENTS_FILE);
  try {
    const realFile = realpathSync(file);
    const realRoot = realpathSync(workspaceRoot);
    if (!realFile.startsWith(realRoot + sep)) return "";
    return readPromptLayer(realFile, MAX_PROMPT_LAYER_BYTES);
  } catch {
    return ""; // absent, unreadable, oversized, or escaped — all skip
  }
}

function omSettingsProvider(sessionId: string): SettingsProvider {
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
import { redactText } from "./redact.js";

export const MAX_QUEUED_EVENTS = 256;
const MAX_LIST = 50;
const MAX_LOAD_ENTRIES = 200;
const MAX_SESSION_SEARCH_LIMIT = 100;
/** Plan 117 follow-up: `/resume` rows are identified by the session's opening
 *  prompt, so the store gets one label per session — the first 5 words,
 *  bounded. Prism's search query reads the newest non-null label, so a single
 *  stamp on the opening entry keeps the session's identity stable for its
 *  whole life. */
const SESSION_LABEL_WORDS = 5;
const SESSION_LABEL_MAX_BYTES = 120;
const REVERSE_TIMEOUT_MS = 30_000;
const TENANT = "clay";
const KEYCHAIN_SERVICE = "clay-agent";
/** Host-authored durable-run revision (must match across run + resume). */
const RUN_STATE_REVISION = "clay-agent.1";
/** Bounded branch-summary text (plan 108 task 10) and in-place tree render. */
const MAX_BRANCH_SUMMARY_BYTES = 2_048;
const MAX_TREE_RENDER_BYTES = 8_192;
// Plan 109 I7 context inspector: bounded previews, bounded items per
// category, and item content capped at the transcript entry budget.
const MAX_CONTEXT_ITEMS = 200;
const MAX_CONTEXT_PREVIEW_CHARS = 160;
const MAX_CONTEXT_ITEM_BYTES = 32 * 1024;
const MAX_TREE_RENDER_ENTRIES = 60;
// Plan 109 I8 Observational Memory tab: bounded activity rows and summaries.
const MAX_OM_ACTIVITY_ROWS = 200;
/** Plan 109 R1: bounded completion surface (commands and description
 *  lengths) — names + descriptions only, never handlers or arg shapes. */
const MAX_COMPLETION_COMMANDS = 64;
const MAX_COMPLETION_NAME_CHARS = 48;
const MAX_COMPLETION_DESCRIPTION_CHARS = 96;
const MAX_OM_SUMMARY_CHARS = 160;
/** Device-code request is one POST. Stay under agent RPC_TIMEOUT (30s). */
const OAUTH_START_TIMEOUT_MS = 20_000;

export type EmitFn = (method: string, params: unknown) => void;

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

/** Plan 118 task 35: one agent type's config, loaded once per root.
 *  Everything the daemon reads per agent — tool caps, skills config, the
 *  seeded skill files, and the disk-discovered skill names of that root's
 *  `skills/` — lives here, so a session is built entirely from its own
 *  agent's files and a switch cannot widen one agent's grants with
 *  another's. */
interface AgentRootConfig {
  readonly root: string;
  readonly toolCaps: { caps: RepositoryToolCaps; file: string };
  readonly skillsConfig: SkillsConfig;
  readonly configSkillNames: readonly string[];
  readonly skillFiles: ReadonlyMap<string, Skill>;
}

interface LiveSession {
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
  /** Host-settable: toggled via session.setAutonomy (decision 2157). */
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
}

/** In-flight LLM branch-summary refinements: sessionId → branch point id. */

const KEYCHAIN_PROBE_REQUEST = { name: "clay-probe", provider: "clay-probe" } as const;

/** Prism 0.5 keychain health check: `list()` is unsupported on keychain
 *  stores, so probe via `get` of a sentinel credential — missing = fine
 *  (`undefined`), locked/denied = typed `CredentialStoreLockedError` (fail
 *  closed: a locked keychain is not an empty vault), unavailable backend =
 *  degrade to vault-only. Injectable candidate factory for tests. */
export async function resolveKeychain(
  createCandidate: () => KeychainCredentialStore = () =>
    createKeychainCredentialStore({ service: KEYCHAIN_SERVICE }),
): Promise<KeychainCredentialStore | undefined> {
  const candidate = createCandidate();
  try {
    await candidate.get(KEYCHAIN_PROBE_REQUEST);
    return candidate;
  } catch (error) {
    if (error instanceof CredentialStoreLockedError) throw error;
    return undefined;
  }
}

/** Persist the live book's provider/model onto the session record (plan 108
 *  task 9): a picker switch mid-session survives daemon restart and resume.
 *  Best-effort — a store without appendSession skips persistence. */
/** Load user-configured repository scan caps from tool-caps.json. Since
 *  decision 2026-09-10-1526 the file is per-agent CONFIGURATION living in
 *  the agent config root (beside skills.json/mcp.json); the data-dir
 *  location is the pre-decision legacy path, still honored when the
 *  config-root file is absent (a migrated data dir carries the old file).
 *  Missing everywhere = defaults; malformed = warn and fall back to
 *  defaults (never blocks daemon boot). */
async function loadToolCaps(
  agentConfigRoot: string,
  dataDir: string,
): Promise<{ caps: RepositoryToolCaps; file: string }> {
  for (const dir of [agentConfigRoot, dataDir]) {
    const file = join(dir, "tool-caps.json");
    try {
      const raw = JSON.parse(await readFile(file, "utf8"));
      return { caps: normalizeToolCaps(raw), file };
    } catch (error) {
      if ((error as NodeJS.ErrnoException)?.code === "ENOENT") continue;
      console.error(`[tool-caps] ignoring malformed ${file}: ${String(error)}`);
      return { caps: {}, file };
    }
  }
  return { caps: {}, file: join(agentConfigRoot, "tool-caps.json") };
}

/** Per-agent config root (decision 2026-09-09-1420):
 *  `~/.clay/agents/<agentId>/`. The coding agent is the first
 *  occupant. Skills config + seeded skill files + SYSTEM.md + mcp.json
 *  live under this root, keeping future agents isolated. */
const CODING_AGENT_CONFIG_DIR = join("agents", "coding-agent");

/** Skills discovered from disk never activate these names: they are
 *  agent-delivered and gated (wiki behind `/wiki-init`, graft behind
 *  binding). They seed into the agent skills dir (plan 117) but register
 *  only through their own activation paths. */
const RESERVED_AGENT_SKILL_NAMES: ReadonlySet<string> = new Set([
  "graft",
  "wiki-searcher",
  "wiki-maintainer",
]);

/** Locked cap: at most 64 skills scanned per discovery root. */
const MAX_SKILLS_PER_ROOT = 64;

interface SkillsConfig {
  workspaceEnabled: boolean;
  configRootEnabled: boolean;
  homeEnabled: boolean;
  homePath: string;
  agentSkills: { wikiSearcher: boolean; wikiMaintainer: boolean; graft: boolean };
}

const DEFAULT_SKILLS_CONFIG: SkillsConfig = {
  workspaceEnabled: true,
  configRootEnabled: true,
  homeEnabled: true,
  homePath: "",
  agentSkills: { wikiSearcher: true, wikiMaintainer: true, graft: true },
};

function skillsWarn(message: string): void {
  process.stderr.write(`[skills] ${message}\n`);
}

/** `~`/`~/` expansion; null when the path is relative (rejected
 *  fail-closed per the locked skills.json schema). */
function expandSkillsPath(value: string): string | null {
  if (value === "~") return homedir();
  if (value.startsWith("~/")) return join(homedir(), value.slice(2));
  if (value.startsWith("/")) return value;
  return null;
}

function skillsFlag(record: Record<string, unknown>, key: string, fallback: boolean): boolean {
  const value = record[key];
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") {
    skillsWarn(`ignoring non-boolean "${key}": ${JSON.stringify(value)}`);
    return fallback;
  }
  return value;
}

/** Load skills.json from the coding-agent config root. Absent/unreadable
 *  file = all roots on with defaults (stderr note, per the locked schema);
 *  unknown keys warned (tool-caps.json precedent); relative paths rejected
 *  fail-closed (root disabled, never a broken boot). */
async function loadSkillsConfig(
  agentConfigRoot: string,
  defaultHomePath: string,
): Promise<{ config: SkillsConfig; file: string }> {
  const file = join(agentConfigRoot, "skills.json");
  let raw: unknown;
  try {
    raw = JSON.parse(await readFile(file, "utf8"));
  } catch {
    skillsWarn(`${file} absent or unreadable; using defaults (all roots enabled)`);
    return {
      config: { ...DEFAULT_SKILLS_CONFIG, homePath: defaultHomePath, agentSkills: { ...DEFAULT_SKILLS_CONFIG.agentSkills } },
      file,
    };
  }
  const config: SkillsConfig = {
    ...DEFAULT_SKILLS_CONFIG,
    homePath: defaultHomePath,
    agentSkills: { ...DEFAULT_SKILLS_CONFIG.agentSkills },
  };
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    skillsWarn(`ignoring malformed ${file}: top level must be an object`);
    return { config, file };
  }
  const root = raw as Record<string, unknown>;
  for (const key of Object.keys(root)) {
    if (key !== "roots" && key !== "agentSkills") skillsWarn(`ignoring unknown key "${key}"`);
  }
  const roots = typeof root.roots === "object" && root.roots !== null && !Array.isArray(root.roots)
    ? (root.roots as Record<string, unknown>)
    : {};
  const workspace = typeof roots.workspace === "object" && roots.workspace !== null ? (roots.workspace as Record<string, unknown>) : {};
  const configRootRecord = typeof roots.configRoot === "object" && roots.configRoot !== null ? (roots.configRoot as Record<string, unknown>) : {};
  const home = typeof roots.home === "object" && roots.home !== null ? (roots.home as Record<string, unknown>) : {};
  config.workspaceEnabled = skillsFlag(workspace, "enabled", true);
  config.configRootEnabled = skillsFlag(configRootRecord, "enabled", true);
  config.homeEnabled = skillsFlag(home, "enabled", true);
  const homePathRaw = typeof home.path === "string" && home.path.length > 0 ? home.path : defaultHomePath;
  const expanded = expandSkillsPath(homePathRaw);
  if (expanded === null) {
    skillsWarn(`ignoring relative roots.home.path ${JSON.stringify(homePathRaw)}; home root disabled`);
    config.homeEnabled = false;
  } else {
    config.homePath = expanded;
  }
  const agentSkills = typeof root.agentSkills === "object" && root.agentSkills !== null && !Array.isArray(root.agentSkills)
    ? (root.agentSkills as Record<string, unknown>)
    : {};
  config.agentSkills.wikiSearcher = skillsFlag(agentSkills, "wikiSearcher", true);
  config.agentSkills.wikiMaintainer = skillsFlag(agentSkills, "wikiMaintainer", true);
  config.agentSkills.graft = skillsFlag(agentSkills, "graft", true);
  return { config, file };
}

async function persistProviderModel(
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

interface PendingOauth {
  readonly provider: string;
  promise: Promise<OAuthCredentials>;
  readonly done: Promise<void>;
  info?: { readonly userCode?: string; readonly verificationUri?: string; readonly authorizationUrl?: string };
}

function rpcError(code: number, message: string, data?: unknown): Error & { rpcCode: number; data?: unknown } {
  return Object.assign(new Error(message), { rpcCode: code, data });
}

function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw rpcError(-32602, "params must be an object");
  }
  return value as Record<string, unknown>;
}

function reqString(params: Record<string, unknown>, key: string): string {
  const value = params[key];
  if (typeof value !== "string" || value.length === 0) throw rpcError(-32602, `params.${key} is required`);
  return value;
}

function optString(params: Record<string, unknown>, key: string): string | undefined {
  const value = params[key];
  if (value === undefined) return undefined;
  if (typeof value !== "string") throw rpcError(-32602, `params.${key} must be a string`);
  return value;
}

function optPolicyCap(params: Record<string, unknown>, key: string): PolicyCap | undefined {
  const value = params[key];
  if (value === undefined) return undefined;
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw rpcError(-32602, `${key} must be a positive safe integer or null`);
  }
  return value;
}

function optPositiveInt(params: Record<string, unknown>, key: string): number | undefined {
  const value = params[key];
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw rpcError(-32602, `${key} must be a positive safe integer`);
  }
  return value;
}

function derivedTotalTokens(input: PolicyCap, output: PolicyCap): PolicyCap {
  if (input === null || output === null) return null;
  const sum = input + output;
  return Number.isSafeInteger(sum) && sum >= 1 ? sum : null;
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
function labelFirstPromptEntry(entry: SessionEntry): SessionEntry {
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

/** The session store Prism writes through, with the resume label stamped on
 *  the opening entry. Read paths stay on the underlying store. */
function labelFirstPromptStore(
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

/**
 * Plan 118 task 35: stamp the agent type that produced an entry into its
 * metadata. One tab can change its agent, so a transcript spanning a switch
 * must label each turn with its producer; the Rust server reads
 * `metadata.agentType` per entry (and falls back to the session's recorded
 * agent for entries written before the stamp existed). Sessions with no agent
 * type (the daemon default) are left untouched.
 */
function agentStampedEntry(entry: SessionEntry, agentType: string | undefined): SessionEntry {
  if (!agentType || entry.metadata?.agentType === agentType) return entry;
  return { ...entry, metadata: { ...(entry.metadata ?? {}), agentType } };
}

/** Local `YYYY-MM-DD HH:MM` for the resume list's second line. The daemon runs
 *  on the user's machine, so its timezone is the user's; a raw ISO stamp in
 *  UTC reads as the wrong hour. */
function localStamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function isOauth(method: AuthMethod): method is OAuthAuthMethod {
  return method.kind === "oauth";
}

function authMethodsFor(auth: readonly AuthMethod[], provider: string) {
  return auth
    .filter((method) => method.provider === provider)
    .map((method) => ({
      kind: method.kind,
      name: method.name,
      credentialName: "credentialName" in method ? method.credentialName : undefined,
    }));
}

const URL_PROVIDERS = new Set(["openai", "ollama", "openrouter"]);

function withUrlMethod(
  provider: string,
  methods: Array<{ kind: string; name?: string; credentialName?: string }>,
) {
  if (!URL_PROVIDERS.has(provider) || methods.some((method) => method.kind === "url")) {
    return methods;
  }
  return [...methods, { kind: "url", name: "API base URL", credentialName: "baseUrl" }];
}

export class ClayAgentHost {
  private readonly secrets = new Set<string>();
  private redactor: SecretRedactor;
  private readonly live = new Map<string, LiveSession>();
  private readonly branchSummarizing = new Map<string, string>();
  private readonly oauth = new Map<string, PendingOauth>();
  private readonly reversePending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (error: Error) => void }
  >();
  private nextReverseId = 1;
  private closed = false;
  private readonly omProfiles = new Set<string>();
  /** Kernel skill registry; duplicate skill names fail closed (plan 107 Task 6). */
  private readonly skills = createSkillRegistry([], { duplicate: "error" });
  /** Disk-discovered skill names. Config-root + home roots scanned once;
   *  each workspace root cached on first session. `undefined` = not yet
   *  scanned. Config/home roots scan `<root>/skills/`; workspace keeps the
   *  npx skills `.agents/skills/` layout. */
  private configSkillNames: readonly string[] | undefined;
  private homeSkillNames: readonly string[] | undefined;
  private readonly workspaceSkillNames = new Map<string, readonly string[]>();
  /** The daemon's default agent root (the shipped coding agent). Sessions
   *  created without an agent use it; named agents resolve as its siblings
   *  under the same `agents/` directory. */
  private readonly agentConfigRoot: string;
  private readonly homeSkillsRoot: string;
  private readonly skillsConfig: SkillsConfig;
  /** Plan 118 task 35: per-agent config, loaded once per root (bounded). */
  private readonly agentRoots = new Map<string, Promise<AgentRootConfig>>();
  /** Plan 118 task 35: MCP bridges + outcomes per agent root, so switching
   *  an agent connects that agent's servers and never inherits another's. */
  private readonly mcpByRoot = new Map<
    string,
    { mcp?: ConnectedMcpServers; outcomes: readonly McpServerOutcome[] }
  >();
  /** Plan 118 task 35: the server-built allow-list last seen per agent root
   *  (sessions + the switch carry it; a restore re-uses it). */
  private readonly mcpAllowListByRoot = new Map<string, readonly unknown[]>();
  /** Connected MCP bridges of the default agent root (empty allow-list =
   *  never populated). Kept for the default-agent paths (initialize
   *  handshake, tests) and mirrored by `mcpByRoot`. */
  private mcp: ConnectedMcpServers | undefined;
  /** Per-server MCP connect outcomes (last connect), for the UI surfaces. */
  private mcpOutcomes: readonly McpServerOutcome[] = [];
  /** Owned Obscura harness (undefined = binary absent, capability hidden). */
  private obscura: ObscuraHarness | undefined;
  /** Capabilities activated lazily on first coding session; serialized. */
  private capabilitiesPromise: Promise<void> | undefined;
  private readonly resolveObscura: () => string | undefined;
  /** Opt-in wiki knowledge base (decision 2156): undefined = disabled
   *  (default, nothing loaded). Bound to one workspace at a time — the
   *  daemon serves the active workspace; re-enabling rebinds. */
  private wiki: { workspaceRoot: string; loaded: LoadedExtension } | undefined;
  /** Opt-in graft knowledge base (decision 2156, plan 108 task 13): same
   *  binding discipline as the wiki — one workspace, fail-closed CLI
   *  resolution, zero residue when disabled. */
  private graft: { workspaceRoot: string; loaded: LoadedExtension; mode: GraftMode } | undefined;
  /** Workspace roots whose default graft bind was attempted (plan 117):
   *  one attempt per root per daemon, success or fail-closed. */
  private readonly graftBindAttempted = new Set<string>();
  /** Session of the run currently in flight — graft push state (patches,
   *  orientation freshness) persists against it. Ceiling: one binding; the
   *  extension API carries no session key on `getEntries`. Upgrade path:
   *  thread sessionId through the prism extension options when it grows one. */
  private activeGraftSessionId: string | undefined;
  /** Prism ollama ships no catalog; host must call listOllamaModels. */
  private ollamaDiscoveryAttempted = false;
  /** User-configured repository scan caps (tool-caps.json in the agent
   *  config root, legacy fallback beside book.json; decision 2026-09-05).
   *  Empty = Prism defaults. */
  private readonly toolCaps: RepositoryToolCaps;
  /** Caps file path cited in truncation errors. */
  private readonly capsFile: string;
  /** Coding-run ceilings + default compact strategy (init.js run.setOptions). */
  private runConfig: RunConfig = { ...DEFAULT_RUN_CONFIG };
  /** Plan 117 follow-up: the session store Prism writes through, with the
   *  resume label stamped on each session's opening entry. Read paths
   *  (`session.list` / `session.load` / search) keep using `persistence`. */
  private readonly labeledStore: SqlitePersistence;

  private constructor(
    readonly dataDir: string,
    private readonly persistence: SqlitePersistence,
    private readonly vault: EncryptedCredentialStore,
    private readonly keychain: KeychainCredentialStore | undefined,
    private readonly kernel: ExtensionKernel,
    private readonly emit: EmitFn,
    private readonly omConfig: HostOptions["observationalMemory"],
    private readonly mcpAllowList: readonly unknown[],
    resolveObscura: () => string | undefined,
    toolCaps: { caps: RepositoryToolCaps; file: string },
    agentConfigRoot: string,
    homeSkillsRoot: string,
    skillsConfig: SkillsConfig,
    private readonly agentSkillFiles: ReadonlyMap<string, Skill>,
    private readonly graftCliPath: string | undefined,
  ) {
    this.redactor = createSecretRedactor([]);
    this.resolveObscura = resolveObscura;
    this.toolCaps = toolCaps.caps;
    this.capsFile = toolCaps.file;
    this.agentConfigRoot = agentConfigRoot;
    this.homeSkillsRoot = homeSkillsRoot;
    this.skillsConfig = skillsConfig;
    this.labeledStore = labelFirstPromptStore(
      persistence,
      (sessionId) => this.live.get(sessionId)?.agentType,
    );
  }

  static async create(options: HostOptions): Promise<ClayAgentHost> {
    await mkdir(options.dataDir, { recursive: true, mode: 0o700 });
    const vaultPath = join(options.dataDir, "credentials.vault");
    const vault = await openEncryptedCredentialStore({
      path: vaultPath,
      getPassphrase: () => options.passphrase,
    });
    // Locked/denied keychain fails closed at boot (resolveKeychain throws
    // CredentialStoreLockedError); unavailable degenerates to vault-only.
    const keychain = await resolveKeychain(options.createKeychain);
    const resolver = createExplicitCredentialResolver([
      { name: "vault", resolver: createStoredCredentialResolver(vault) },
      ...(keychain ? [{ name: "keychain" as const, resolver: createStoredCredentialResolver(keychain) }] : []),
    ]);
    const persistence = createSqlitePersistence({
      filename: join(options.dataDir, "sessions.sqlite"),
      fileMode: 0o600,
    });
    const kernel = createExtensionKernel({ errorPolicy: "throw" });
    // Built-in plain-chat profile: the server's `ensure_tab_session` defaults
    // to "Chat" when the book has no profile, so a definition must always
    // exist even before any package registers richer profiles.
    kernel.registries.agents.register("Chat", {
      name: "Chat",
      description: "Plain conversation without coding tools",
    });
    if (options.mock) {
      kernel.registries.providers.register(
        options.mockProvider ?? createMockProvider([providerTextDelta("Hello"), providerDone()]),
      );
      kernel.registries.models.register({ provider: "mock", model: "demo", displayName: "Mock demo" });
      kernel.registries.authMethods.register("mock\0api_key", {
        kind: "api_key",
        provider: "mock",
        credentialName: "apiKey",
      });
    } else {
      const { loadProviderPackages } = await import("./providers.js");
      await loadProviderPackages(kernel, resolver);
    }
    // Per-agent config root (decision 2026-09-09-1420, root moved to
    // `~/.clay` by decision 2026-09-10-1526):
    // `~/.clay/agents/coding-agent`.
    const agentConfigRoot =
      options.agentConfigRoot ?? join(homedir(), ".clay", CODING_AGENT_CONFIG_DIR);
    const homeSkillsRoot = options.homeSkillsRoot ?? join(homedir(), ".agents");
    // Seed the user SYSTEM.md (skills are seeded inside loadAgentSkillFiles)
    // before the first session build reads it.
    await seedUserSystemPrompt(agentConfigRoot).catch((error) => {
      process.stderr.write(
        `[system] seeding SYSTEM.md failed: ${error instanceof Error ? error.message : String(error)}\n`,
      );
    });
    const host = new ClayAgentHost(
      options.dataDir,
      persistence,
      vault,
      keychain,
      kernel,
      options.emit ?? (() => {}),
      options.observationalMemory,
      Array.isArray(options.mcpAllowList) ? options.mcpAllowList : [],
      options.resolveObscuraBinary ?? resolveObscuraBinary,
      await loadToolCaps(agentConfigRoot, options.dataDir),
      agentConfigRoot,
      homeSkillsRoot,
      (await loadSkillsConfig(agentConfigRoot, homeSkillsRoot)).config,
      await loadAgentSkillFiles(agentConfigRoot),
      options.graftCliPath,
    );
    const mockProvider = kernel.registries.providers.get("mock");
    options.registerModels?.(kernel.registries);
    options.registerAgents?.(kernel.registries);
    registerCompactionStrategies(kernel, {
      secrets: [],
      ...(mockProvider ? { llm: { provider: mockProvider, model: { provider: "mock", model: "demo" } } } : {}),
    });
    host.secrets.add(options.passphrase);
    host.refreshRedactor();
    return host;
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    for (const live of this.live.values()) live.session.abort("shutdown");
    this.live.clear();
    for (const pending of this.reversePending.values()) {
      pending.reject(new Error("host closed"));
    }
    this.reversePending.clear();
    // Owned capability children die with the daemon (revocation/shutdown kills).
    if (this.mcp) {
      const mcp = this.mcp;
      this.mcp = undefined;
      this.mcpOutcomes = [];
      void mcp.close().catch(() => {});
    }
    if (this.obscura) {
      const obscura = this.obscura;
      this.obscura = undefined;
      void obscura.close().catch(() => {}); // SIGTERM → SIGKILL, process-group kill on POSIX
    }
    this.persistence.close();
  }

  /**
   * Lazy capability activation on the first coding session (never on import
   * or Chat initialize). Validation errors fail closed (they mean the server
   * sent a malformed allow-list); connection failures hide the capability —
   * a server that will not connect simply provides no tools.
   */
  private ensureCapabilities(
    agentRoot: string = this.agentConfigRoot,
    allowList: readonly unknown[] = this.mcpAllowList,
  ): Promise<void> {
    if (agentRoot === this.agentConfigRoot && this.capabilitiesPromise) {
      // The default agent's capabilities keep their single lazy promise
      // (initialize-order semantics unchanged).
      return this.capabilitiesPromise;
    }
    if (!this.mcpByRoot.has(agentRoot)) this.mcpByRoot.set(agentRoot, { outcomes: [] });
    const pending = (async () => {
        if (allowList.length > 0 && !this.mcpByRoot.get(agentRoot)?.mcp) {
          try {
            const connected = await connectAllowListedMcpServers(allowList);
            this.mcpByRoot.set(agentRoot, { mcp: connected, outcomes: connected.outcomes });
            if (agentRoot === this.agentConfigRoot) {
              // The initialize-time list stays the default agent's bridge.
              this.mcp = connected;
              this.mcpOutcomes = connected.outcomes;
            }
          } catch (error) {
            if ((error as { rpcCode?: number }).rpcCode === -32602) throw error;
            this.mcpByRoot.set(agentRoot, { outcomes: [] });
          }
        }
        if (!this.obscura) {
          const bin = this.resolveObscura();
          if (bin) {
            try {
              this.obscura = await spawnObscuraHarness({ command: bin });
            } catch {
              this.obscura = undefined; // hidden, never an error (2159)
            }
          }
        }
      })();
    if (agentRoot === this.agentConfigRoot) {
      // Keep the documented single-promise default path (callers await it
      // again); per-agent roots await their own connect.
      this.capabilitiesPromise = pending;
    }
    return pending;
  }

  /** Capability tools for a coding session; undefined shapes stay allowed. */
  private capabilityTools(agentRoot: string = this.agentConfigRoot): ToolDefinition[] {
    const bridge = this.mcpByRoot.get(agentRoot)?.mcp ?? (agentRoot === this.agentConfigRoot ? this.mcp : undefined);
    return [...(bridge?.tools ?? []), ...(this.obscura?.tools ?? [])];
  }

  /** Per-server MCP connect outcomes for the environment/UI (bounded,
   *  trimmed). Plan 118 task 35: scoped to an agent root — a switched
   *  session reports the servers it actually has. */
  private mcpServerOutcomes(agentRoot: string = this.agentConfigRoot): JsonObject[] {
    const outcomes =
      this.mcpByRoot.get(agentRoot)?.outcomes ??
      (agentRoot === this.agentConfigRoot ? this.mcpOutcomes : []);
    return outcomes.slice(0, MAX_MCP_SERVERS).map((outcome) => ({
      serverId: outcome.serverId.slice(0, MAX_COMPLETION_NAME_CHARS),
      connected: outcome.connected,
      tools: outcome.tools,
      ...(outcome.error === undefined ? {} : { error: outcome.error.slice(0, MAX_COMPLETION_DESCRIPTION_CHARS) }),
    }));
  }

  /**
   * Daemon→server reverse RPC for coding-tool document backends. Requests
   * carry daemon-namespaced numeric ids; the server routes them to the
   * document registry.
   */
  request(method: string, params: Record<string, unknown>): Promise<unknown> {
    if (this.closed) return Promise.reject(Object.assign(new Error("host closed"), { rpcCode: -32000 }));
    const id = this.nextReverseId;
    this.nextReverseId += 1;
    return new Promise<unknown>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.reversePending.delete(id);
        reject(Object.assign(new Error(`reverse request timed out: ${method}`), { rpcCode: -32000 }));
      }, REVERSE_TIMEOUT_MS);
      this.reversePending.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      this.emit("reverse", { jsonrpc: "2.0", id, method, params });
    });
  }

  /** Complete a pending reverse request from a server response frame. */
  resolveReverse(id: unknown, payload: { result?: unknown; error?: { code?: number; message?: string } }): void {
    if (typeof id !== "number") return;
    const pending = this.reversePending.get(id);
    if (!pending) return;
    this.reversePending.delete(id);
    if (payload.error) {
      const message = redactText(payload.error.message ?? "reverse request failed", this.secrets);
      pending.reject(Object.assign(new Error(message), { rpcCode: Number(payload.error.code ?? -32000) }));
      return;
    }
    pending.resolve(payload.result);
  }

  async handle(method: string, params: unknown): Promise<unknown> {
    switch (method) {
      case "session.new":
        return this.sessionNew(asRecord(params));
      case "session.list":
        return this.sessionList(asRecord(params ?? {}));
      case "session.load":
        return this.sessionLoad(asRecord(params));
      // Plan 109 I7: live context inspector — categorized, bounded,
      // redacted. `{ sessionId }` lists categories; `{ sessionId, itemId }`
      // returns one item's full redacted content.
      case "session.context":
        return this.sessionContext(asRecord(params));
      // Plan 109 I8: OM worker-model selection + observer activity log.
      case "session.om.set":
        return this.sessionOmSet(asRecord(params));
      case "session.om.activity":
        return this.sessionOmActivity(asRecord(params));
      // Plan 117: bounded workspace file listing for the @-mention
      // dropdown (client-side filter, server-side walk inside the root).
      case "workspace.files":
        return this.workspaceListFiles(asRecord(params));
      case "session.setAgent":
        return this.sessionSetAgent(asRecord(params));
      case "session.resume":
        return this.sessionResume(asRecord(params));
      case "session.delete":
        return this.sessionDelete(asRecord(params));
      case "session.prompt":
        return this.sessionPrompt(asRecord(params));
      case "session.cancel":
        return this.sessionCancel(asRecord(params));
      case "session.steer":
        return this.sessionSteer(asRecord(params));
      case "session.compact":
        return this.sessionCompact(asRecord(params));
      case "session.setAutonomy":
        return this.sessionSetAutonomy(asRecord(params));
      case "session.search":
        return this.sessionSearch(asRecord(params));
      // Plan 109 I9: workspace-scoped resumable session list (/resume).
      case "session.resumable":
        return this.sessionResumable(asRecord(params));
      case "session.checkout":
        return this.sessionCheckout(asRecord(params));
      case "session.fork":
        return this.sessionFork(asRecord(params));
      case "session.clone":
        return this.sessionClone(asRecord(params));
      case "session.checkpoint":
        return this.sessionCheckpoint(asRecord(params));
      case "run.resume":
        return this.runResume(asRecord(params));
      case "provider.list":
        return this.providerList();
      case "provider.status":
        return this.providerStatus(asRecord(params));
      case "model.list":
        return this.modelList();
      case "model.search":
        return this.modelSearch(asRecord(params));
      case "credential.put":
        return this.credentialPut(asRecord(params));
      case "credential.oauthStart":
        return this.oauthStart(asRecord(params));
      case "credential.oauthPoll":
        return this.oauthPoll(asRecord(params));
      case "credential.delete":
        return this.credentialDelete(asRecord(params));
      case "agentProfile.list":
        return this.profileList();
      case "agentProfile.register":
        return this.profileRegister(asRecord(params));
      case "skill.register":
        return this.skillRegister(asRecord(params));
      case "skill.list":
        return this.skillList();
      case "command.register":
        return this.commandRegister(asRecord(params));
      case "command.dispatch":
        return this.commandDispatch(asRecord(params));
      case "environment.list":
        return this.environmentList(asRecord(params));
      case "knowledge.setOptions":
        return this.knowledgeSetOptions(asRecord(params));
      case "run.setOptions":
        return this.runSetOptions(asRecord(params));
      default:
        throw rpcError(-32601, `unknown method: ${method}`);
    }
  }

  /**
   * Plan 118 task 35: resolve one agent type to its config root.
   *
   * The name is a bare directory name (`AGENT_TYPE_RE`), so it cannot escape
   * the `agents/` root — the root is a direct child of the default root's
   * parent. An explicit name that does not resolve is rejected fail-closed
   * (never silently the default agent: that would run one agent's config
   * under another's name). An absent name means the default agent.
   */
  private resolveAgentRoot(agent: unknown): string {
    if (agent === undefined || agent === null) return this.agentConfigRoot;
    if (typeof agent !== "string") {
      throw rpcError(-32602, "agent must be a string when present");
    }
    const name = agent.trim();
    if (name === "") return this.agentConfigRoot;
    if (!AGENT_TYPE_RE.test(name)) {
      throw rpcError(-32602, `invalid agent type: ${JSON.stringify(agent)}`);
    }
    const root = join(dirname(this.agentConfigRoot), name);
    let resolved = false;
    try {
      resolved = statSync(root).isDirectory();
    } catch {
      resolved = false;
    }
    if (!resolved) {
      throw rpcError(-32602, `agent type ${name} is not configured`);
    }
    return root;
  }

  /** The allow-list known for one agent root: the last one the server sent
   *  for it, else the initialize-time list (default agent) or none. */
  private mcpAllowListFor(root: string): readonly unknown[] {
    return (
      this.mcpAllowListByRoot.get(root) ??
      (root === this.agentConfigRoot ? this.mcpAllowList : [])
    );
  }

  /** The default agent's config as the host loaded it at create time (the
   *  synchronous recreate paths need no await). */
  private defaultAgentConfig(): AgentRootConfig {
    return {
      root: this.agentConfigRoot,
      toolCaps: { caps: this.toolCaps, file: this.capsFile },
      skillsConfig: this.skillsConfig,
      configSkillNames: this.configSkillNames ?? [],
      skillFiles: this.agentSkillFiles,
    };
  }

  /** The type name for a root: the default agent is `undefined` (the server
   *  records no name for it). */
  private agentTypeOf(root: string): string | undefined {
    return root === this.agentConfigRoot ? undefined : basename(root);
  }

  /**
   * Plan 118 task 35: load (once) the config of one agent root — seeding its
   * SYSTEM.md and delivered skills first, then tool caps, `skills.json`, the
   * delivered skill files, and the root's discovered skill names. Bounded by
   * `MAX_AGENT_ROOTS`: past the cap a root loads uncached (correct, just
   * re-read) rather than dropping the session.
   */
  private agentRootConfig(root: string): Promise<AgentRootConfig> {
    const cached = this.agentRoots.get(root);
    if (cached) return cached;
    const pending = (async (): Promise<AgentRootConfig> => {
      await seedUserSystemPrompt(root).catch((error) => {
        process.stderr.write(
          `[system] seeding SYSTEM.md for ${root} failed: ${error instanceof Error ? error.message : String(error)}\n`,
        );
      });
      const toolCaps = await loadToolCaps(root, this.dataDir);
      const skillsConfig = (await loadSkillsConfig(root, this.homeSkillsRoot)).config;
      const skillFiles = await loadAgentSkillFiles(root);
      // User-added skills of this root register here (the delivered
      // wiki/graft files do not: their own activation paths own those gates).
      const configSkillNames = skillsConfig.configRootEnabled
        ? await this.scanSkillsDir(join(root, "skills"))
        : [];
      return { root, toolCaps, skillsConfig, configSkillNames, skillFiles };
    })();
    if (this.agentRoots.size >= MAX_AGENT_ROOTS) {
      process.stderr.write(
        `[agent] agent root cap of ${MAX_AGENT_ROOTS} reached; ${root} loads uncached\n`,
      );
      return pending;
    }
    this.agentRoots.set(root, pending);
    return pending;
  }

  /** Test/introspection accessor: current autonomy flag for a live session. */
  sessionAutonomy(sessionId: string): boolean {
    return this.live.get(sessionId)?.fullAutonomy ?? false;
  }

  redactError(error: unknown): { code: number; message: string; data?: unknown } {
    const code = typeof error === "object" && error && "rpcCode" in error ? Number(error.rpcCode) : -32000;
    const message = redactText(error instanceof Error ? error.message : String(error), this.secrets);
    return { code: Number.isFinite(code) ? code : -32000, message };
  }

  private refreshRedactor(): void {
    this.redactor = createSecretRedactor([...this.secrets]);
  }

  private rememberSecret(secret: string): void {
    if (!secret) return;
    this.secrets.add(secret);
    this.refreshRedactor();
  }

  private async sessionNew(params: Record<string, unknown>): Promise<unknown> {
    const profile = reqString(params, "profile");
    const provider = reqString(params, "provider");
    const modelId = reqString(params, "model");
    const id = optString(params, "id") ?? randomUUID();
    const workspaceRoot = optString(params, "workspaceRoot") ?? process.cwd();
    // Approvals are opt-out by default (user decision 2026-09-05): tool
    // calls auto-approve unless a caller explicitly disables autonomy.
    const fullAutonomy = params.fullAutonomy !== false;
    const observationalMemory = this.resolveOmFlag(profile, params.observationalMemory);
    // Plan 109 I8: per-session OM worker models (each may differ from the
    // session model; `null` clears a previously-set selection).
    const omWorkers = this.parseOmWorkers(params.observationalMemoryWorkers);
    if (omWorkers) omWorkerModels.set(id, omWorkers);
    // Capabilities activate on the first session that declares coding tools
    // (never on import, never for Chat-only initialize).
    // Plan 118 task 35: the session's agent (its config root + that agent's
    // MCP allow-list). Absent = the daemon's default agent; an unknown or
    // malformed name fails closed here.
    const agentRoot = this.resolveAgentRoot(params.agent);
    const agentType = this.agentTypeOf(agentRoot);
    const agent = await this.agentRootConfig(agentRoot);
    const allowList = Array.isArray(params.mcpAllowList)
      ? (params.mcpAllowList as readonly unknown[])
      : this.mcpAllowListFor(agentRoot);
    this.mcpAllowListByRoot.set(agentRoot, allowList);
    const wantsCoding = this.profileWantsCodingTools(profile);
    if (wantsCoding) {
      await this.ensureCapabilities(agentRoot, allowList);
      // Graft is available by default (plan 117): first coding session for
      // a workspace attempts the pull-mode binding once per root per
      // daemon. Chat sessions never trigger it.
      await this.ensureGraftBound(workspaceRoot);
    }
    await this.ensureSkillDiscovery(workspaceRoot);
    const created = this.createSession(id, profile, provider, modelId, {
      workspaceRoot,
      fullAutonomy,
      observationalMemory,
      agent,
      mcpAllowList: allowList,
    });
    const now = new Date().toISOString();
    if (typeof this.persistence.appendSession !== "function") {
      throw rpcError(-32000, "session store cannot persist session records");
    }
    await this.persistence.appendSession({
      id,
      tenantId: TENANT,
      agentDefinitionId: profile,
      createdAt: now,
      updatedAt: now,
      metadata: {
        profile,
        provider,
        model: modelId,
        observationalMemory,
        fullAutonomy,
        // Stable workspace identity for workspace-scoped search (decision 2201).
        [SESSION_SEARCH_WORKSPACE_METADATA_KEY]: workspaceRoot,
        ...(agentType ? { agentType } : {}),
        ...(omWorkers ? { omWorkers } : {}),
      },
    });
    this.live.set(id, {
      session: created.session,
      agent: created.agent,
      systemPrompt: created.systemPrompt,
      profile,
      provider,
      model: modelId,
      workspaceRoot,
      agentType,
      agentRoot,
      mcpAllowList: allowList,
      fullAutonomy,
      observationalMemory,
      tools: created.tools,
      mentionSkills: [],
    });
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

  private createSession(
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
  ): {
    session: AgentSession;
    agent: Agent;
    tools: ToolDefinition[];
    systemPrompt?: SystemPromptConfig;
  } {
    const def = this.kernel.registries.agents.resolve(profile);
    if (!this.kernel.registries.providers.get(provider)) throw rpcError(-32000, `Unknown provider: ${provider}`);
    const tools = this.sessionTools(def, id, options);
    const skills = this.resolveSkills(def, tools, options);
    // User-owned global system-prompt layer (plan 117): a `user`-source
    // contribution composes after the profile base instructions and before
    // the workspace AGENTS.md app layer (Prism source rank user < app).
    // def.systemPrompt === false disables contributions entirely — the
    // user layer is suppressed with them.
    const userSystemText = loadUserSystemPrompt(options.agent.root);
    const agentsText = loadWorkspaceAgentsPrompt(options.workspaceRoot);
    // Locked layer order (plan 117): base instructions → SYSTEM.md (user)
    // → AGENTS.md (app). Prism's composeSystemPrompt sorts contributions
    // by source rank (user=0, app=2), so array order here is not load-
    // bearing; def.systemPrompt === false suppresses both layers.
    const layers: Array<{ id: string; source: "user" | "app"; text: string }> = [];
    if (userSystemText) layers.push({ id: "user-system-md", source: "user", text: userSystemText });
    if (agentsText) layers.push({ id: "agents-md", source: "app", text: agentsText });
    const profileContributions =
      def.systemPrompt === undefined || def.systemPrompt === false
        ? []
        : Array.isArray(def.systemPrompt)
          ? [...def.systemPrompt]
          : [def.systemPrompt];
    const systemPrompt = [...layers, ...profileContributions];
    // Progressive disclosure: with active skills, host the `load_skill` tool so
    // the model can pull full instructions on demand (catalog-only by default).
    const runTools = skills && skills.length > 0 ? [...tools, createLoadSkillTool({ registry: this.skills, tools })] : tools;
    const model: ModelConfig = this.kernel.registries.models.get(provider, modelId) ?? { provider, model: modelId };
    const agent = createAgent({
      id: profile,
      model,
      providerSource: createProviderResolver(this.kernel.registries.providers),
      store: this.labeledStore,
      runLedger: this.persistence,
      redactor: this.redactor,
      validator: createJsonSchemaToolArgumentValidator(),
      // Coding-run ceilings from run.setOptions. Prism 0.5.5: omit
      // maxProviderAttempts (lifts to maxTurns); pass null tokens so
      // DEFAULT 40k/10k/50k cannot apply. Bytes are per-frame HARD
      // (null rejected); 64 MiB is the ceiling on any single provider
      // frame, never a run-lifetime sum (0.5.5 fixed cumulative charging).
      limits: {
        maxTurns: this.runConfig.maxTurns,
        maxToolRounds: this.runConfig.maxToolRounds,
        maxToolCalls: this.runConfig.maxToolCalls,
        maxWallTimeMs: this.runConfig.maxWallTimeMs,
        maxInputTokens: this.runConfig.maxInputTokens,
        maxOutputTokens: this.runConfig.maxOutputTokens,
        maxTotalTokens: derivedTotalTokens(
          this.runConfig.maxInputTokens,
          this.runConfig.maxOutputTokens,
        ),
        maxRequestBytes: HARD_RUN_LIMITS.maxRequestBytes,
        maxResponseBytes: HARD_RUN_LIMITS.maxResponseBytes,
      },
      // Host-verified identity default for every session and resumed run: the
      // daemon is the trust boundary that owns the workspace root and
      // acceptance policy, so it vouches for its own runs. Durable tool
      // effects (write/edit/delete/move) require it — without it every
      // mediated write fails closed with ERR_PRISM_TOOL_EFFECT_CONFLICT,
      // including post-resume turns where run options don't reach the context.
      identity: this.runIdentity(),
      // No providerRequestPolicies — Prism 0.5.1 kernel fills session/cache
      // keys (decision 2026-09-07-2149).
      ...(def.instructions !== undefined ? { instructions: def.instructions } : {}),
      ...(systemPrompt.length > 0 ? { systemPrompt } : {}),
      ...(runTools.length > 0 ? { tools: runTools } : {}),
      ...(skills !== undefined ? { skills } : {}),
    });
    let session = agent.createSession({ id });
    if (options.observationalMemory) session = this.attachOm(session, model);
    return { session, agent, tools: runTools, systemPrompt: systemPrompt.length > 0 ? systemPrompt : undefined };
  }

  /** Rebuild the live session's agent with a new provider/model config
   *  (plan 108 task 9): durable runs fingerprint on AgentConfig and reject
   *  per-run model overrides, so a picker switch recreates the session.
   *  The leaf carries over so the next append chains onto the same branch. */
  private recreateSessionModel(
    live: LiveSession,
    sessionId: string,
    provider: string,
    modelId: string,
    agent?: AgentRootConfig,
  ): LiveSession {
    const recreated = this.createSession(sessionId, live.profile, provider, modelId, {
      workspaceRoot: live.workspaceRoot,
      fullAutonomy: live.fullAutonomy,
      observationalMemory: live.observationalMemory,
      // Plan 118 task 35: the session's own agent config (loaded by the
      // caller); the default-agent path reuses the host's loaded values.
      agent: agent ?? this.defaultAgentConfig(),
      mcpAllowList: live.mcpAllowList,
    });
    // The created session is already bound to the branch's current leaf
    // (the leaf this recreate carries over), and it is the OM-attached
    // proxy when Observational Memory is on — the Observer's post-run
    // flush and context provider must survive model switches. The prior
    // raw `agent.createSession` here silently detached OM after any
    // mid-session model switch (plan 109 I8 fix).
    const session = recreated.session;
    live.session = session;
    live.agent = recreated.agent;
    live.provider = provider;
    live.model = modelId;
    return live;
  }

  private resolveOmFlag(profile: string, raw: unknown): boolean {
    if (raw === true) return true;
    if (raw === false) return false;
    return this.omProfiles.has(profile);
  }

  /** True when the profile's declared tool list includes coding tool names. */
  private profileWantsCodingTools(profile: string): boolean {
    try {
      const def = this.kernel.registries.agents.resolve(profile);
      return (def.tools ?? []).some((name) => (CODING_TOOL_NAMES as readonly string[]).includes(name));
    } catch {
      return false;
    }
  }

  private sessionTools(
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
    const built = this.buildSessionTools(def, sessionId, options) ?? [];
    // Capabilities ride along with coding sessions (they were activated for
    // the coding profile; Chat profiles with no tools stay no-tools). Plan
    // 118 task 35: the agent's own bridge, so a switch swaps its servers.
    const capabilities = built.length > 0 ? this.capabilityTools(options.agent.root) : [];
    // Opt-in wiki knowledge tools ride along with coding sessions in the
    // enabled workspace (decision 2156); absent entirely when disabled.
    const wiki = built.length > 0 ? this.wikiTools(options.workspaceRoot) : [];
    // Opt-in graft knowledge tools ride the same workspace binding.
    const graft = built.length > 0 ? this.graftTools(options.workspaceRoot) : [];
    if (!options.observationalMemory) return [...built, ...capabilities, ...wiki, ...graft];
    return [
      ...built,
      ...capabilities,
      ...wiki,
      ...graft,
      createRecallMemoryTool({
        getEntries: (sid) => this.persistence.list(sid),
        secrets: [...this.secrets],
      }),
    ];
  }

  /** Plan 109 I8: resolve a per-session OM worker config. The selection's
   *  provider instance must be registered — an unknown provider fails
   *  closed (worker omitted, `requireExplicitModel` skips it) and never
   *  silently falls back to the session model (decision 2158). */
  private omWorkerConfig(
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
        ? this.omConfig?.observation
        : this.omConfig?.reflection;
    if (selection) {
      const provider = this.kernel.registries.providers.get(selection.provider);
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

  private attachOm(session: AgentSession, model: ModelConfig): AgentSession {
    const observation = this.omWorkerConfig(session.id, "observation");
    const reflection = this.omWorkerConfig(session.id, "reflection");
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
      ...(this.omConfig?.dropper
        ? {
            dropper: {
              ...(this.omConfig.dropper.provider ? { provider: this.omConfig.dropper.provider } : {}),
              ...(this.omConfig.dropper.model ? { model: this.omConfig.dropper.model } : {}),
              ...(this.omConfig.dropper.targetTokens !== undefined
                ? { targetTokens: this.omConfig.dropper.targetTokens }
                : {}),
              ...(this.omConfig.dropper.policy ? { policy: this.omConfig.dropper.policy } : {}),
            },
          }
        : {}),
      context: { compactAfterTokens: this.omConfig?.compactAfterTokens ?? DEFAULT_COMPACT_AFTER_TOKENS },
      settings: omSettingsProvider(session.id),
      secrets: [...this.secrets],
      requireExplicitModel: true,
    });
    return om.attach(session, {
      appendEntry: (entry, appendOptions) => this.persistence.append(entry, appendOptions),
      sessionModel: model,
    }).session;
  }

  private resolveCompaction(name: string, live: LiveSession) {
    if (!isCompactionStrategyName(name)) throw rpcError(-32602, `unknown compaction strategy: ${name}`);
    const provider = this.kernel.registries.providers.get(live.provider);
    return createNamedCompactionStrategy(name, {
      secrets: [...this.secrets],
      ...(provider ? { llm: { provider, model: { provider: live.provider, model: live.model } } } : {}),
    });
  }

  /**
   * Resolve a profile's tools. Names in CODING_TOOL_NAMES build fresh
   * per-session coding tools bound to the session workspace + autonomy flag;
   * everything else resolves from the kernel registry (Chat profiles with no
   * tools stay no-tools).
   */
  private buildSessionTools(
    def: AgentDefinition,
    sessionId: string,
    options: { workspaceRoot: string; fullAutonomy: boolean },
  ): ToolDefinition[] | undefined {
    if (!def.tools) return undefined;
    const coding = def.tools.filter((name) => (CODING_TOOL_NAMES as readonly string[]).includes(name));
    const rest = def.tools.filter((name) => !(CODING_TOOL_NAMES as readonly string[]).includes(name));
    if (coding.length === 0) {
      return rest.length > 0 ? rest.map((name) => this.kernel.registries.tools.resolve(name)) : undefined;
    }
    const tools = buildCodingTools({
      workspaceRoot: options.workspaceRoot,
      request: (method, params) => this.request(method, params),
      fullAutonomy: () => this.live.get(sessionId)?.fullAutonomy ?? options.fullAutonomy,
      toolCaps: this.toolCaps,
      capsFile: this.capsFile,
      approve: (action) =>
        this.request("approval.request", {
          action: { kind: action.kind, operation: action.operation, paths: action.paths, command: action.command },
        }).then(
          () => true,
          () => false,
        ),
      ask: async (request) => {
        const answer = (await this.request("approval.askUserDecision", {
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
    return [...rest.map((name) => this.kernel.registries.tools.resolve(name)), ...tools];
  }

  /** Resolve a profile's skill names against the kernel registry, validating
   *  each skill's required tools are active (fail-closed before any turn).
   *  Wiki skills append for coding sessions in the enabled workspace — their
   *  toolNames require the wiki tools, which are active exactly then. The
   *  graft skill appends on the same workspace binding. */
  private resolveSkills(
    def: AgentDefinition,
    tools: readonly ToolDefinition[],
    options?: { workspaceRoot?: string; agent?: AgentRootConfig },
  ): readonly Skill[] | undefined {
    const names = def.skills ? [...def.skills] : [];
    if (options?.workspaceRoot !== undefined && this.wiki?.workspaceRoot === options.workspaceRoot) {
      names.push(wikiSearcherSkill.name, wikiMaintainerSkill.name);
    }
    if (options?.workspaceRoot !== undefined && this.graft?.workspaceRoot === options.workspaceRoot) {
      names.push(graftSkill.name);
    }
    // Disk-discovered skills ride along when every declared toolName is
    // active for this session. Profile-declared names keep fail-closed
    // resolution; discovered ones are skipped instead of thrown so one bad
    // SKILL.md cannot brick session start.
    if (options?.workspaceRoot !== undefined) {
      const activeToolNames = new Set(tools.map((tool) => tool.name));
      // Plan 118 task 35: the config-root names come from the session's own
      // agent (the home and workspace roots are host/workspace scoped).
      const discovered = [
        ...(options.agent?.configSkillNames ?? this.configSkillNames ?? []),
        ...(this.homeSkillNames ?? []),
        ...(this.workspaceSkillNames.get(options.workspaceRoot) ?? []),
      ];
      for (const name of discovered) {
        if (names.includes(name)) continue;
        const skill = this.skills.get(name);
        if (skill?.toolNames && !skill.toolNames.every((toolName) => activeToolNames.has(toolName))) continue;
        names.push(name);
      }
    }
    if (names.length === 0) return undefined;
    return resolveActiveSkills({ registry: this.skills, names, tools });
  }

  /** Config-gated, per-root skill discovery. Config-root + home roots scan
   *  `<root>/skills/<name>/SKILL.md` (decision 2026-09-09-1420); the
   *  workspace keeps npx skills `<root>/.agents/skills/`. Each root is
   *  scanned once (cached). Already-registered names win on collision
   *  (registry is duplicate:"error"): built-ins and earlier roots shadow
   *  later ones. Reserved agent-delivered names never come from disk —
   *  they activate only through their own paths. A failing root logs and
   *  yields nothing — discovery never fails a session. */
  private async ensureSkillDiscovery(workspaceRoot: string): Promise<void> {
    if (this.configSkillNames === undefined) {
      this.configSkillNames = this.skillsConfig.configRootEnabled
        ? await this.scanSkillsDir(join(this.agentConfigRoot, "skills"))
        : [];
    }
    if (this.homeSkillNames === undefined) {
      this.homeSkillNames = this.skillsConfig.homeEnabled
        ? await this.scanSkillsDir(join(this.skillsConfig.homePath, "skills"))
        : [];
    }
    if (!this.workspaceSkillNames.has(workspaceRoot)) {
      this.workspaceSkillNames.set(
        workspaceRoot,
        this.skillsConfig.workspaceEnabled ? await this.discoverWorkspaceSkills(workspaceRoot) : [],
      );
    }
  }

  /** Workspace scan via Prism's discoverContributions (symlink-escape
   *  safe, ENOENT-tolerant, npx skills layout). */
  private async discoverWorkspaceSkills(workspaceRoot: string): Promise<readonly string[]> {
    let discovered: readonly (Awaited<ReturnType<typeof discoverContributions>>)[number][];
    try {
      discovered = await discoverContributions({ kinds: ["skill"], workspaceRoot });
    } catch (error) {
      process.stderr.write(
        `skill discovery failed for ${workspaceRoot}: ${error instanceof Error ? error.message : String(error)}\n`,
      );
      return [];
    }
    const names: string[] = [];
    for (const entry of discovered) {
      const skill = entry.skill;
      if (!skill || this.skills.get(skill.name)) continue;
      this.skills.register(skill);
      names.push(skill.name);
    }
    return names;
  }

  /** Scan `<skillsDir>/<name>/SKILL.md` (one level) and register each
   *  parseable skill. Bounded; absent dir = no skills (not a warning);
   *  a bad SKILL.md is skipped with a stderr note. */
  private async scanSkillsDir(skillsDir: string): Promise<readonly string[]> {
    let entries: string[];
    try {
      entries = await readdir(skillsDir);
    } catch {
      return [];
    }
    const names: string[] = [];
    for (const entry of entries.slice(0, MAX_SKILLS_PER_ROOT)) {
      if (RESERVED_AGENT_SKILL_NAMES.has(entry)) continue;
      const file = join(skillsDir, entry, "SKILL.md");
      let text: string;
      try {
        text = await readFile(file, "utf8");
      } catch {
        continue;
      }
      try {
        const skill = parseSkillFile(text, file);
        if (this.skills.get(skill.name)) continue;
        this.skills.register(skill);
        names.push(skill.name);
      } catch (error) {
        process.stderr.write(
          `[skills] skipping ${file}: ${error instanceof Error ? error.message : String(error)}\n`,
        );
      }
    }
    return names;
  }

  /** Whether an agent-delivered skill is enabled by skills.json
   *  `agentSkills` (default true). Consumed by the wiki/graft activation
   *  paths (plan 117). */
  private agentSkillEnabled(name: "wikiSearcher" | "wikiMaintainer" | "graft"): boolean {
    return this.skillsConfig.agentSkills[name];
  }

  private async sessionList(params: Record<string, unknown>): Promise<unknown> {
    const limitRaw = params.limit;
    const limit = typeof limitRaw === "number" && Number.isFinite(limitRaw) ? Math.min(MAX_LIST, Math.max(1, limitRaw)) : MAX_LIST;
    const page = await this.persistence.querySessions({
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

  private async sessionLoad(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const entryId = typeof params.entryId === "string" ? params.entryId : undefined;
    const page = await this.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
    const record = page.items[0];
    if (!record) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const entries = await this.persistence.list(sessionId);
    const selected = entryId ? branchEntries(entries, entryId) : undefined;
    return {
      sessionId,
      profile: record.agentDefinitionId,
      metadata: record.metadata,
      entries: (selected ?? entries).slice(-MAX_LOAD_ENTRIES).map((entry) => this.redactor.redact(entry)),
    };
  }

  /**
   * Plan 109 I7: live context inspector over the active session branch.
   * Active context = the branch chain from the latest `kind: "compaction"`
   * entry onward (compacted-away items leave the list; the summary item
   * stays) — a branch checkout rebinds `entries()` to the checked-out
   * branch, so the same derivation restores the earlier context. Builds
   * on demand (never on keystroke/paint paths), redacted through the
   * session redactor, previews and content bounded.
   */
  private async sessionContext(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const entries = await live.session.entries();
    const itemId = optString(params, "itemId");
    if (itemId) {
      const item = this.contextItem(live, entries, itemId);
      if (!item) throw rpcError(-32000, `Unknown context item: ${itemId}`);
      return { sessionId, itemId, ...item };
    }
    return {
      sessionId,
      // Cheap invalidation version: every entry append (run finished,
      // compaction, steer, skill row) bumps it; the client refetches when
      // its transcript length changes.
      version: entries.length,
      categories: this.contextCategories(live, entries),
    };
  }

  /** Active-context slice: from the latest compaction entry onward. */
  private activeContextEntries(entries: readonly SessionEntry[]): readonly SessionEntry[] {
    let boundary = -1;
    for (let index = entries.length - 1; index >= 0; index -= 1) {
      if (entries[index]?.kind === "compaction") {
        boundary = index;
        break;
      }
    }
    return boundary === -1 ? entries : entries.slice(boundary);
  }

  /**
   * Categorized active context. One category model feeds counts and items
   * (the item list is capped; `count` stays the real number so the client
   * can show "…and N more").
   */
  private contextCategories(
    live: LiveSession,
    entries: readonly SessionEntry[],
  ): Array<{ kind: string; label: string; count: number; items: Array<ContextItemRef> }> {
    const system: ContextItemRef[] = [];
    const user: ContextItemRef[] = [];
    const skills: ContextItemRef[] = [];
    const tools: ContextItemRef[] = [];
    const thinking: ContextItemRef[] = [];
    const agent: ContextItemRef[] = [];
    const summaries: ContextItemRef[] = [];
    // The composed system prompt the model actually receives: base
    // instructions first, then rank-ordered contributions (plan 117 —
    // the group is no longer empty for profiles with only base
    // instructions).
    const baseInstructions = live.agent.config.instructions;
    if (typeof baseInstructions === "string" && baseInstructions.length > 0) {
      system.push({
        id: "system-prompt-base",
        title: "System prompt (base instructions)",
        preview: clipUtf8(baseInstructions, MAX_CONTEXT_PREVIEW_CHARS),
      });
    }
    for (const [index, contribution] of rankedSystemPromptContributions(live.systemPrompt).entries()) {
      system.push({
        id: `system-prompt-${index}`,
        title: `System prompt (${promptLayerLabel(contribution.id)})`,
        preview: clipUtf8(contribution.text, MAX_CONTEXT_PREVIEW_CHARS),
      });
    }
    for (const entry of this.activeContextEntries(entries)) {
      const redacted = this.redactor.redact(entry);
      if (redacted.kind === "summary" || redacted.kind === "compaction") {
        summaries.push({
          id: redacted.id,
          title: "Compaction summary",
          preview: clipUtf8(redacted.summary ?? "", MAX_CONTEXT_PREVIEW_CHARS),
        });
        continue;
      }
      if (redacted.kind === "message" && redacted.message) {
        const message = redacted.message;
        message.content.forEach((block, index) => {
          const blockId = `${redacted.id}#${index}`;
          if (block.type === "thinking") {
            thinking.push({
              id: blockId,
              title: "Thinking",
              preview: clipUtf8(block.text, MAX_CONTEXT_PREVIEW_CHARS),
            });
            return;
          }
          if (block.type === "tool_call") {
            // Loaded skills ride the same transcript as tool calls: a
            // `load_skill` call feeds both the skill and tool categories
            // (skill tool-execution events are not persisted as entries).
            if (block.name === "load_skill") {
              const skillName =
                typeof (block.arguments as Record<string, unknown> | undefined)?.name === "string"
                  ? ((block.arguments as Record<string, unknown>).name as string)
                  : "unknown";
              skills.push({
                id: blockId,
                title: `load_skill: ${skillName}`,
                preview: clipUtf8(JSON.stringify(block.arguments), MAX_CONTEXT_PREVIEW_CHARS),
              });
            }
            tools.push({
              id: blockId,
              title: `tool call: ${block.name}`,
              preview: clipUtf8(JSON.stringify(block.arguments), MAX_CONTEXT_PREVIEW_CHARS),
            });
            return;
          }
          if (block.type === "tool_result") {
            tools.push({
              id: blockId,
              title: `tool output: ${block.name}`,
              preview: clipUtf8(
                block.error ? `error: ${block.error.message ?? ""}` : JSON.stringify(block.result ?? null),
                MAX_CONTEXT_PREVIEW_CHARS,
              ),
            });
            return;
          }
          if (block.type !== "text" || !block.text) return;
          if (message.role === "user") {
            user.push({ id: blockId, title: "User prompt", preview: clipUtf8(block.text, MAX_CONTEXT_PREVIEW_CHARS) });
          } else if (message.role === "assistant") {
            agent.push({ id: blockId, title: "Agent message", preview: clipUtf8(block.text, MAX_CONTEXT_PREVIEW_CHARS) });
          }
        });
        continue;
      }
      // Loaded skills: load_skill tool calls are persisted as tool events.
      if (redacted.kind === "event" && redacted.event) {
        const event = redacted.event as Record<string, unknown>;
        if (
          event.type === "tool_execution_started" &&
          typeof event.call === "object" && event.call !== null &&
          (event.call as Record<string, unknown>).name === "load_skill"
        ) {
          const call = (event.call as Record<string, unknown>).arguments as Record<string, unknown> | undefined;
          const name = typeof call?.name === "string" ? call.name : "unknown";
          skills.push({
            id: redacted.id,
            title: `load_skill: ${name}`,
            preview: clipUtf8(JSON.stringify(call ?? {}), MAX_CONTEXT_PREVIEW_CHARS),
          });
        }
      }
    }
    const build = (kind: string, label: string, items: ContextItemRef[]) => ({
      kind,
      label,
      count: items.length,
      items: items.slice(-MAX_CONTEXT_ITEMS),
    });
    return [
      build("systemPrompt", "System prompt", system),
      build("userMessage", "User prompts", user),
      build("skill", "Skills loaded", skills),
      build("toolOutput", "Tool calls + outputs", tools),
      build("thinking", "Thinking", thinking),
      build("agentMessage", "Agent messages", agent),
      build("compactionSummary", "Compaction summaries", summaries),
    ];
  }

  /** One context item's full redacted content for the drawer detail. */
  private contextItem(
    live: LiveSession,
    entries: readonly SessionEntry[],
    itemId: string,
  ): { kind: string; title: string; content: string } | undefined {
    // System-prompt items live outside the transcript.
    if (itemId === "system-prompt-base") {
      const base = live.agent.config.instructions;
      if (typeof base !== "string" || base.length === 0) return undefined;
      return {
        kind: "systemPrompt",
        title: "System prompt (base instructions)",
        content: clipUtf8(base, MAX_CONTEXT_ITEM_BYTES),
      };
    }
    if (itemId.startsWith("system-prompt-")) {
      const index = Number(itemId.slice("system-prompt-".length));
      const contribution = rankedSystemPromptContributions(live.systemPrompt)[index];
      if (!contribution) return undefined;
      return {
        kind: "systemPrompt",
        title: `System prompt (${promptLayerLabel(contribution.id)})`,
        content: clipUtf8(contribution.text, MAX_CONTEXT_ITEM_BYTES),
      };
    }
    const [entryId, blockIndex] = itemId.split("#");
    const entry = this.activeContextEntries(entries).find((candidate) => candidate.id === entryId);
    if (!entry) return undefined;
    const redacted = this.redactor.redact(entry);
    const clip = (text: string) => clipUtf8(text, MAX_CONTEXT_ITEM_BYTES);
    if (redacted.kind === "summary" || redacted.kind === "compaction") {
      return { kind: "compactionSummary", title: "Compaction summary", content: clip(redacted.summary ?? "") };
    }
    if (redacted.kind === "event" && redacted.event) {
      const event = redacted.event as Record<string, unknown>;
      if (event.type === "tool_execution_started" && typeof event.call === "object" && event.call !== null) {
        return {
          kind: "skill",
          title: "load_skill",
          content: clip(JSON.stringify((event.call as Record<string, unknown>).arguments ?? {})),
        };
      }
      return undefined;
    }
    if (redacted.kind !== "message" || !redacted.message) return undefined;
    const index = Number(blockIndex);
    const block = Number.isInteger(index) ? redacted.message.content[index] : undefined;
    if (!block) return undefined;
    if (block.type === "thinking") {
      return { kind: "thinking", title: "Thinking", content: clip(block.text) };
    }
    if (block.type === "tool_call") {
      return {
        kind: "toolOutput",
        title: `tool call: ${block.name}`,
        content: clip(JSON.stringify({ name: block.name, arguments: block.arguments })),
      };
    }
    if (block.type === "tool_result") {
      return {
        kind: "toolOutput",
        title: `tool output: ${block.name}`,
        content: clip(
          block.error ? `error: ${block.error.message ?? JSON.stringify(block.error)}` : JSON.stringify(block.result ?? null),
        ),
      };
    }
    if (block.type === "text") {
      const kind = redacted.message.role === "user" ? "userMessage" : "agentMessage";
      const title = redacted.message.role === "user" ? "User prompt" : "Agent message";
      return { kind, title, content: clip(block.text) };
    }
    return undefined;
  }

  /** Plan 109 I8: validate an OM worker selection payload from session.new
   *  / session.om.set. Each worker is `{ provider, model }` (both
   *  non-empty strings) or `null` to clear; unknown shapes fail closed. */
  private parseOmWorkers(raw: unknown): OmWorkerModels | undefined {
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
  private async sessionOmSet(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const workers = this.parseOmWorkers(params.workers);
    if (!workers) throw rpcError(-32602, "workers must be an object");
    for (const worker of [workers.observation, workers.reflection]) {
      if (worker && !this.kernel.registries.providers.get(worker.provider)) {
        throw rpcError(-32000, `Unknown OM worker provider: ${worker.provider}`);
      }
    }
    const existing = omWorkerModels.get(sessionId) ?? {};
    omWorkerModels.set(sessionId, { ...existing, ...workers });
    await this.persistOmWorkers(sessionId);
    // Re-attach so the new workers apply immediately (next run); the
    // rebuild preserves the leaf and is the same mechanism as a
    // mid-session model switch.
    const live = this.live.get(sessionId);
    if (live?.observationalMemory) {
      const agent = await this.agentRootConfig(live.agentRoot);
      this.recreateSessionModel(live, sessionId, live.provider, live.model, agent);
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
  private async persistOmWorkers(sessionId: string): Promise<void> {
    if (typeof this.persistence.appendSession !== "function") return;
    try {
      const page = await this.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
      const record = page.items[0];
      if (!record) return;
      const stored = omWorkerModels.get(sessionId);
      const metadata = {
        ...((record.metadata ?? {}) as Record<string, unknown>),
        omWorkers: stored ?? {},
      };
      await this.persistence.appendSession({
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
  private async sessionOmActivity(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
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
              summary: this.omActivitySummary(observation.content),
            });
            if (activity.length >= MAX_OM_ACTIVITY_ROWS) break;
          }
        } else if (type === "om.reflections.recorded" && Array.isArray(data.reflections)) {
          for (const reflection of data.reflections as Array<Record<string, unknown>>) {
            if (typeof reflection.content !== "string") continue;
            activity.push({
              id: `${entry.id}#${String(reflection.id ?? activity.length)}`,
              kind: "reflection",
              summary: this.omActivitySummary(reflection.content),
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
  private omActivitySummary(text: string): string {
    const firstLine = text.split("\n", 1)[0] ?? text;
    return clipUtf8(this.redactor.redact(firstLine), MAX_OM_SUMMARY_CHARS);
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
  private async sessionSetAgent(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = await this.ensureLive(sessionId);
    const agentRoot = this.resolveAgentRoot(params.agent);
    const agentType = this.agentTypeOf(agentRoot);
    const agent = await this.agentRootConfig(agentRoot);
    const allowList = Array.isArray(params.mcpAllowList)
      ? (params.mcpAllowList as readonly unknown[])
      : this.mcpAllowListFor(agentRoot);
    this.mcpAllowListByRoot.set(agentRoot, allowList);
    const overProvider = optString(params, "provider");
    const overModel = optString(params, "model");
    const provider = overProvider ?? live.provider;
    const model = overModel ?? live.model;
    if (!this.kernel.registries.providers.get(provider)) {
      throw rpcError(-32000, `Unknown provider: ${provider}`);
    }
    // Bind the new agent *before* the rebuild: the rebuilt session's tools
    // and capability set come from these fields.
    live.agentRoot = agentRoot;
    live.agentType = agentType;
    live.mcpAllowList = allowList;
    this.recreateSessionModel(live, sessionId, provider, model, agent);
    const omWorkers = this.parseOmWorkers(params.observationalMemoryWorkers);
    if (omWorkers) {
      omWorkerModels.set(sessionId, omWorkers);
      await this.persistOmWorkers(sessionId);
    }
    if (this.profileWantsCodingTools(live.profile)) {
      await this.ensureCapabilities(agentRoot, allowList);
    }
    await this.persistAgentType(sessionId, agentType, provider, model);
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
  private async persistAgentType(
    sessionId: string,
    agentType: string | undefined,
    provider: string,
    model: string,
  ): Promise<void> {
    if (typeof this.persistence.appendSession !== "function") return;
    try {
      const page = await this.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
      const record = page.items[0];
      if (!record) return;
      const metadata: Record<string, unknown> = {
        ...((record.metadata ?? {}) as Record<string, unknown>),
        provider,
        model,
      };
      if (agentType) metadata.agentType = agentType;
      else delete metadata.agentType;
      await this.persistence.appendSession({
        ...record,
        metadata,
        updatedAt: new Date().toISOString(),
      });
    } catch {
      // Metadata persistence is advisory; the live switch already applied.
    }
  }

  private async sessionResume(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = await this.ensureLive(sessionId);
    return {
      sessionId,
      profile: live.profile,
      provider: live.provider,
      model: live.model,
      // Plan 118 task 35: the session's agent rides the resume reply so the
      // server keeps labelling turns with their producer after a restart.
      agent: live.agentType,
      agentRoot: live.agentRoot,
      leafId: live.session.leafId,
    };
  }

  private async ensureLive(sessionId: string): Promise<LiveSession> {
    const existing = this.live.get(sessionId);
    if (existing) return existing;
    const page = await this.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
    const record = page.items[0];
    if (!record) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const metadata = (record.metadata ?? {}) as Record<string, unknown>;
    const profile = typeof metadata.profile === "string" ? metadata.profile : record.agentDefinitionId;
    const provider = typeof metadata.provider === "string" ? metadata.provider : undefined;
    const model = typeof metadata.model === "string" ? metadata.model : undefined;
    if (!profile || !provider || !model) throw rpcError(-32000, `Session ${sessionId} is missing profile/provider/model`);
    const observationalMemory = this.resolveOmFlag(profile, metadata.observationalMemory);
    const restoredWorkers = this.parseOmWorkers(metadata.omWorkers);
    if (restoredWorkers) omWorkerModels.set(sessionId, restoredWorkers);
    // Plan 118 task 35: a restored session runs the agent recorded with it
    // (absent for sessions written before agent types — the default agent).
    const agentRoot = this.resolveAgentRoot(
      typeof metadata.agentType === "string" ? metadata.agentType : undefined,
    );
    const agentType = this.agentTypeOf(agentRoot);
    const agent = await this.agentRootConfig(agentRoot);
    await this.ensureSkillDiscovery(process.cwd());
    const created = this.createSession(sessionId, profile, provider, model, {
      workspaceRoot: process.cwd(),
      fullAutonomy: metadata.fullAutonomy !== false,
      observationalMemory,
      agent,
      mcpAllowList: this.mcpAllowListFor(agentRoot),
    });
    const live: LiveSession = {
      session: created.session,
      agent: created.agent,
      systemPrompt: created.systemPrompt,
      profile,
      provider,
      model,
      workspaceRoot: process.cwd(),
      agentType,
      agentRoot,
      mcpAllowList: this.mcpAllowListFor(agentRoot),
      fullAutonomy: false,
      observationalMemory,
      tools: created.tools,
      mentionSkills: [],
    };
    this.live.set(sessionId, live);
    return live;
  }

  private async sessionDelete(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
    if (live) {
      live.session.abort("deleted");
      this.live.delete(sessionId);
    }
    const result = await this.persistence.lifecycle.applyRetention({
      tenantId: TENANT,
      policy: { id: "clay-agent-delete", createdAt: new Date().toISOString(), tenantId: TENANT },
      candidates: [sessionId],
    });
    return { deleted: result.deleted.includes(sessionId) };
  }

  private async sessionPrompt(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const text = reqString(params, "text");
    // Graft push state (patches, orientation freshness) persists against the
    // run in flight (see activeGraftSessionId).
    this.activeGraftSessionId = sessionId;
    // Slash-command intercept (pi parity): a prompt whose first token exactly
    // matches a REGISTERED command name dispatches that command instead of
    // prompting the model. Unknown /x stays a normal prompt (chat-safe); the
    // dispatch error for a known-name failure is the caller's feedback.
    const slash = commandInvocation(text);
    if (slash) {
      const command = this.registeredCommand(slash.name);
      if (command) {
        const value = await this.commandDispatch({
          name: slash.name,
          sessionId,
          args: slashArgs(slash),
        });
        // Slash feedback rides the transcript lane (pi parity): a synthetic
        // started/delta/finished triple renders the result as an assistant
        // message and closes the client's AG-UI run (the dispatch reply alone
        // would leave the run observable open and the composer stuck).
        const feedbackText =
          slash.name === "/tree" ? await this.renderTreeText(sessionId).catch(() => "tree:") : undefined;
        this.emitCommandFeedback(sessionId, slash.name, value, feedbackText);
        return value;
      }
      // /wiki-init is the sole wiki initiator (plan 117): with the
      // agentSkills gate on, an unmatched /wiki-init enables the binding
      // (the exact enableWiki path knowledge.setOptions uses — idempotent
      // per workspace) and then dispatches the now-registered extension
      // command. Gate off ⇒ falls through to a normal prompt (chat-safe,
      // zero residue). wiki-searcher gates the feature; wiki-maintainer's
      // own flag filters its registration inside enableWiki.
      if (slash.name === "/wiki-init" && this.agentSkillEnabled("wikiSearcher")) {
        const live = await this.ensureLive(sessionId);
        await this.enableWiki(live.workspaceRoot);
        const value = await this.commandDispatch({
          name: slash.name,
          sessionId,
          args: slashArgs(slash),
        });
        this.emitCommandFeedback(sessionId, slash.name, value, undefined);
        return value;
      }
    }
    let live = await this.ensureLive(sessionId);
    // Provider/model switching mid-session (pi parity): rebuild the session's
    // agent with the new model config, because durable runs fingerprint on
    // AgentConfig and reject per-run model overrides. Only when the target
    // differs from the live book — the run-scoped passthrough from the server
    // echoes the current model, and a needless rebuild would reset the leaf.
    // Validated against the registries, remembered on the live book, and
    // persisted so resume keeps the switch.
    const overProvider = optString(params, "provider");
    const overModel = optString(params, "model");
    if (overProvider !== undefined || overModel !== undefined) {
      const provider = overProvider ?? live.provider;
      const model = overModel ?? live.model;
      if (!this.kernel.registries.providers.get(provider)) {
        throw rpcError(-32000, `Unknown provider: ${provider}`);
      }
      // Model ids pass through: the discovery catalog is convenience, not
      // authority — the provider rejects genuinely unknown ids at call time.
      // (The book may reference a model discovery has not listed yet.)
      if (provider !== live.provider || model !== live.model) {
        live = this.recreateSessionModel(
          live,
          sessionId,
          provider,
          model,
          await this.agentRootConfig(live.agentRoot),
        );
        await persistProviderModel(this.persistence, sessionId, provider, model);
      }
    }
    const compactionName = optString(params, "compaction");
    const compaction = compactionName
      ? { strategy: this.resolveCompaction(compactionName, live), secrets: [...this.secrets] }
      : undefined;
    // Plan 109 I4: per-run portable thinking level. Fail-closed at this
    // boundary — Prism `parseThinkingLevel` rejects empty/non-string input;
    // opaque non-empty strings pass through (forward-compat). Prism 0.5.1
    // resolves the compat family and snaps to the model's declared set
    // kernel-side; non-reasoning models get no compat invented.
    const rawLevel = params.thinkingLevel;
    let thinkingLevel: string | undefined;
    if (rawLevel !== undefined) {
      const level = parseThinkingLevel(rawLevel);
      if (level === undefined) {
        throw rpcError(-32602, "thinkingLevel must be a non-empty string");
      }
      // Opaque non-empty strings pass through for forward-compatible
      // provider fields; known levels carry as the portable union.
      thinkingLevel = typeof level === "string" ? level : level.opaque;
    }
    const runState = this.durableRunState(live);
    // Plan 117 @-mentions: `@skill:<name>` is a manual skill trigger — the
    // named skill joins the session's loaded set (the same state the
    // load_skill tool mutates, restoreLoadedSkills is the public path) and
    // rides every subsequent run's `skills` option, so its body renders
    // from the first round with no tool round-trip. `@file:<relative path>`
    // attaches a workspace file server-side (images as image content
    // blocks). Tokens that do not resolve stay plain text (chat-safe);
    // mention names validate against the registry with the same toolNames
    // discipline activation applies.
    const mentionedSkills = await this.resolveMentions(text, live);
    const promptInput: string | Message =
      mentionedSkills.attachments.length > 0
        ? { role: "user", content: [{ type: "text", text: mentionedSkills.prompt }, ...mentionedSkills.attachments] }
        : mentionedSkills.prompt;
    // Run skills union: the profile's resolved active set (config default)
    // plus manually triggered skills. Only overridden when a mention exists
    // — untouched runs keep the config-provided catalog byte-identical.
    let runSkills: readonly Skill[] | undefined;
    if (live.mentionSkills.length > 0) {
      const base = this.resolveSkills(
        this.kernel.registries.agents.resolve(live.profile),
        live.tools,
        { workspaceRoot: live.workspaceRoot },
      );
      runSkills = [...(base ?? []), ...live.mentionSkills];
    }
    // stream() (not subscribe()+run()) — durable runs keep the subscription
    // open past settlement, so only stream() both drains and resolves.
    const stream = live.session.stream(promptInput, {
      maxQueuedEvents: MAX_QUEUED_EVENTS,
      overflow: "drop_oldest",
      // Host-verified identity: the daemon is the trust boundary that owns the
      // workspace root and acceptance policy, so it vouches for its own runs.
      // Durable tool effects (write/edit/delete/move) require this — without
      // it every mediated write fails closed with ERR_PRISM_TOOL_EFFECT_CONFLICT.
      identity: this.runIdentity(),
      ...(compaction ? { compaction } : {}),
      ...(runState ? { runState } : {}),
      ...(thinkingLevel ? { thinkingLevel } : {}),
      ...(runSkills ? { skills: runSkills } : {}),
    });
    let lastType: string | undefined;
    let suspended:
      | { runId: string; interruption: AgentRunInterruption; version: number }
      | undefined;
    try {
      for await (const event of stream) {
        lastType = event.type;
        if (event.type === "agent_suspended") {
          suspended = {
            runId: event.runId,
            interruption: event.interruption,
            version: event.version,
          };
        }
        this.emit("event", { sessionId, event: redactAgentEvent(event, this.redactor) satisfies AgentEvent });
      }
    } catch (error) {
      throw rpcError(-32000, error instanceof Error ? error.message : String(error));
    }
    return {
      sessionId,
      lastEvent: lastType,
      ...(suspended
        ? (() => {
            // Stash the suspension version: run.resume callers (approval UI)
            // may omit expectedVersion and rely on this stash.
            const liveSession = this.live.get(sessionId);
            if (liveSession) liveSession.suspension = { runId: suspended.runId, version: suspended.version };
            return {
              status: "suspended" as const,
              runId: suspended.runId,
              version: suspended.version,
              interruption: suspended.interruption,
            };
          })()
        : { status: lastType === "agent_finished" ? ("succeeded" as const) : ("aborted" as const) }),
    };
  }

  /** Durable + tool-interrupt for coding sessions only; Chat stays non-durable. */
  private durableRunState(live: LiveSession): AgentRunStateOptions | undefined {
    const coding = live.tools.some((tool) => (CODING_TOOL_NAMES as readonly string[]).includes(tool.name));
    if (!coding) return undefined;
    // Tool-approval interrupts are opt-in via autonomy: full autonomy
    // (the default) auto-approves every tool call and streams through;
    // turning autonomy off (session.setAutonomy / NewSession param)
    // re-arms the suspend-on-tool-gate flow for callers that want it.
    return {
      checkpoints: this.persistence.checkpoints,
      definitionRevision: RUN_STATE_REVISION,
      interruptBeforeTool: !live.fullAutonomy,
    };
  }

  /** Plan 117 @-mentions: parse `@skill:` / `@file:` tokens out of the raw
   *  prompt. Valid skill mentions join the session's loaded set (the same
   *  LoadedSkillSet the load_skill tool mutates — bodies re-resolve from
   *  the registry, persistence rides the snapshot names-only), append to
   *  the session's mention set (rides every run's skills option), and the
   *  prompt gains one instruction line. Resolvable file mentions become
   *  content blocks (images as base64 image content, text files as a
   *  fenced text block) resolved server-side inside the workspace root.
   *  Everything unresolvable stays plain text: chat-safe, no throw. */
  private async resolveMentions(
    text: string,
    live: LiveSession,
  ): Promise<{ prompt: string; attachments: ContentBlock[] }> {
    const toolNames = new Set(live.tools.map((tool) => tool.name));
    const loaded: Skill[] = [];
    // Mention tokens are whitespace-delimited, so skill names with spaces
    // cannot be mentioned (the dropdown embeds dir-form names; ceiling:
    // add an escape syntax if space-y names ever need mentioning).
    // # ponytail: no-space mention names; escape syntax if ever needed
    let prompt = text.replace(/@skill:([A-Za-z0-9_-]+)/g, (token, raw) => {
      const name = String(raw).trim();
      const skill = this.skills.get(name);
      // Same activation discipline as resolveActiveSkills: a skill whose
      // tools are unavailable never loads (a hand-typed mention is still
      // gated; catalog names already passed this filter).
      if (skill === undefined) return token;
      if (skill.toolNames && !skill.toolNames.every((tool) => toolNames.has(tool))) return token;
      if (!loaded.some((entry) => entry.name === name)) loaded.push(skill);
      return `\`${name}\``;
    });
    const attachments: ContentBlock[] = [];
    const fileMatches = [...prompt.matchAll(/@file:([^\s@]+)/g)];
    if (fileMatches.length > 0) {
      const blocks = new Map<string, ContentBlock | undefined>();
      for (const match of fileMatches) {
        const relPath = String(match[1]);
        if (!blocks.has(relPath)) blocks.set(relPath, await this.readMentionFile(relPath, live.workspaceRoot));
      }
      prompt = prompt.replace(/@file:([^\s@]+)/g, (token, raw) => {
        const block = blocks.get(String(raw));
        if (block === undefined) return token;
        attachments.push(block);
        return "";
      });
    }
    if (loaded.length > 0) {
      // RuntimeAgentSession exposes restoreLoadedSkills (public since plan 015
      // task 4); the AgentSession interface predates it, hence the narrow cast.
      (live.session as unknown as { restoreLoadedSkills(names: readonly string[]): void }).restoreLoadedSkills(
        loaded.map((skill) => skill.name),
      );
      for (const skill of loaded) {
        if (!live.mentionSkills.some((entry) => entry.name === skill.name)) live.mentionSkills.push(skill);
      }
      prompt = `[skills loaded by mention: ${loaded.map((skill) => skill.name).join(", ")}]

${prompt}`;
    }
    return { prompt, attachments };
  }

  /** Read one @file mention inside the workspace root. Returns undefined
   *  (token stays plain text) for paths that escape the root, are missing,
   *  unreadable, or — for images — oversized. Images attach as base64
   *  image content; other files as a bounded fenced text block. */
  private async readMentionFile(relPath: string, workspaceRoot: string): Promise<ContentBlock | undefined> {
    const full = join(workspaceRoot, relPath);
    try {
      const rootReal = realpathSync(workspaceRoot);
      const fileReal = realpathSync(full);
      if (!fileReal.startsWith(rootReal + sep)) return undefined;
      const info = await stat(fileReal);
      if (!info.isFile() || info.size > MAX_MENTION_FILE_BYTES) return undefined;
      const ext = relPath.slice(relPath.lastIndexOf(".")).toLowerCase();
      const mime = IMAGE_MIME_BY_EXT.get(ext);
      if (mime) {
        const data = (await readFile(fileReal)).toString("base64");
        return { type: "image", mimeType: mime, data, name: relPath };
      }
      const body = (await readFile(fileReal, "utf8")).slice(0, MAX_MENTION_FILE_BYTES);
      return { type: "text", text: `\n\n[attached file: ${relPath}]\n\`\`\`\n${body}\n\`\`\`` };
    } catch {
      return undefined;
    }
  }

  /** Plan 117: bounded workspace file list for the @-mention dropdown —
   *  workspace-relative paths, dotfiles and build dirs skipped, capped in
   *  entries and depth. Absent/unreachable root ⇒ empty list (never a
   *  throw; the dropdown degrades to skills-only). */
  private async workspaceListFiles(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = await this.ensureLive(sessionId);
    const files: string[] = [];
    const walk = async (dir: string, depth: number): Promise<void> => {
      if (depth > MAX_LIST_WALK_DEPTH || files.length >= MAX_LISTED_FILES) return;
      let entries;
      try {
        entries = await readdir(dir, { withFileTypes: true });
      } catch {
        return;
      }
      for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
        if (files.length >= MAX_LISTED_FILES) return;
        const rel = join(dir, entry.name).slice(live.workspaceRoot.length + 1);
        if (entry.isDirectory()) {
          if (!LIST_SKIPPED_DIRS.has(entry.name) && !entry.name.startsWith(".")) await walk(join(dir, entry.name), depth + 1);
        } else if (entry.isFile() && !entry.name.startsWith(".")) {
          files.push(rel);
        }
      }
    };
    await walk(live.workspaceRoot, 1);
    return { files: files.slice(0, MAX_LISTED_FILES) };
  }

  private async sessionCancel(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    live.session.abort("cancel");
    return { sessionId, cancelled: true };
  }

  private sessionSteer(params: Record<string, unknown>): unknown {
    const sessionId = reqString(params, "sessionId");
    const text = reqString(params, "text");
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    live.session.steer(text, { softInterrupt: params.softInterrupt === true });
    return { sessionId, steered: true };
  }

  private async sessionCompact(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const strategyName = optString(params, "strategy") ?? this.runConfig.compaction;
    const threshold = params.compactAfterTokens;
    if (
      threshold !== undefined &&
      (typeof threshold !== "number" || !Number.isFinite(threshold) || threshold <= 0)
    ) {
      throw rpcError(-32602, "compactAfterTokens must be a positive finite number");
    }
    if (threshold !== undefined) omCompactAfterTokens.set(sessionId, threshold);
    const live = await this.ensureLive(sessionId);
    try {
      const result = await live.session.compact({
        strategy: this.resolveCompaction(strategyName, live),
        secrets: [...this.secrets],
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

  /** Toggle full autonomy for a live session (decision 2157): default false;
   *  true skips approval prompts for gated tool calls. Host-set only — no
   *  agent-facing tool can change this. */
  private sessionSetAutonomy(params: Record<string, unknown>): unknown {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    if (typeof params.enabled !== "boolean") {
      throw rpcError(-32602, "enabled must be a boolean");
    }
    live.fullAutonomy = params.enabled;
    return { sessionId, fullAutonomy: live.fullAutonomy };
  }

  /** Workspace-scoped session search (decision 2201): always filters by this
   *  session's workspaceRoot; hits are transcript metadata, never auto-injected
   *  into agent context. Cross-workspace queries return empty, not an error. */
  private async sessionSearch(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const query = typeof params.query === "string" ? params.query : undefined;
    const limitRaw = typeof params.limit === "number" && Number.isSafeInteger(params.limit) ? params.limit : undefined;
    const limit = limitRaw ? Math.min(MAX_SESSION_SEARCH_LIMIT, Math.max(1, limitRaw)) : DEFAULT_SESSION_SEARCH_LIMIT;
    if (!this.persistence.searchSessions) {
      throw rpcError(-32000, "session store does not support search");
    }
    const page = await this.persistence.searchSessions({
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
        ...(hit.snippet !== undefined ? { snippet: this.redactor.redact(hit.snippet) } : {}),
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
  private async sessionResumable(params: Record<string, unknown>): Promise<unknown> {
    const workspaceRoot = reqString(params, "workspaceRoot");
    const limitRaw = typeof params.limit === "number" && Number.isSafeInteger(params.limit) ? params.limit : undefined;
    const limit = limitRaw ? Math.min(MAX_SESSION_SEARCH_LIMIT, Math.max(1, limitRaw)) : DEFAULT_SESSION_SEARCH_LIMIT;
    if (!this.persistence.searchSessions) {
      throw rpcError(-32000, "session store does not support search");
    }
    const page = await this.persistence.searchSessions({
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
        ...(hit.snippet !== undefined ? { snippet: this.redactor.redact(hit.snippet) } : {}),
        metadata: hit.metadata,
      })),
      nextCursor: page.nextCursor,
    };
  }

  /** Checkout the conversation leaf AND restore the document checkpoint
   *  recorded at that entry (decision 2200): server restores document
   *  versions first, then the session history rebuilds. Fails closed if the
   *  server cannot restore (e.g. lease held elsewhere) — leaf stays put.
   *  Before the leaf moves, the abandoned path (current leaf back to the
   *  branch point) gets a summary entry parented at the branch point — on
   *  the NEW branch, pi's branch_summary model — immediately with a
   *  deterministic preview, refined in the background by an LLM run that
   *  replaces the leaf when the session has not moved on. Nothing is ever
   *  deleted: the abandoned side keeps its entries. */
  private async sessionCheckout(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const entryId = reqString(params, "entryId");
    const live = await this.ensureLive(sessionId);
    await this.request("checkpoint.restore", { sessionId, entryId });
    const fromLeaf = live.session.leafId;
    const preview = await this.appendBranchSummary(live, sessionId, entryId, fromLeaf);
    await live.session.checkout(preview.entryId);
    void this.refineBranchSummary(live, sessionId, entryId, fromLeaf, preview);
    return { sessionId, leafId: live.session.leafId, summaryEntryId: preview.entryId, summarizing: true };
  }

  /** Append a summary entry (kind "summary") for the abandoned path from
   *  `fromLeaf` back to `branchPointId` (exclusive). Deterministic bounded
   *  preview; the LLM refine lands as a sibling entry later. */
  private async appendBranchSummary(
    live: LiveSession,
    sessionId: string,
    branchPointId: string,
    fromLeaf: string | undefined,
  ): Promise<{ entryId: string }> {
    const entries = await live.session.entries();
    const text = branchText(entries, fromLeaf, branchPointId);
    const summaryEntry = createSessionEntry({
      sessionId,
      parentId: branchPointId,
      kind: "summary",
      summary: clipUtf8(text, MAX_BRANCH_SUMMARY_BYTES) || "(empty branch)",
      data: { fromId: fromLeaf, pending: true },
    });
    await this.persistence.append(summaryEntry);
    return { entryId: summaryEntry.id };
  }

  /** Background LLM refine (mock-provider testable): one-shot no-tool run
   *  over the abandoned path; the refined entry re-roots the leaf when the
   *  session has not moved on. Never throws into the checkout path. */
  private async refineBranchSummary(
    live: LiveSession,
    sessionId: string,
    branchPointId: string,
    fromLeaf: string | undefined,
    preview: { entryId: string },
  ): Promise<void> {
    this.branchSummarizing.set(sessionId, branchPointId);
    try {
      const entries = await live.session.entries();
      const branchText = branchTextForPrompt(entries, fromLeaf, branchPointId);
      const runModel = this.kernel.registries.models.get(live.provider, live.model) ?? {
        provider: live.provider,
        model: live.model,
      };
      const worker = createAgent({
        id: "branch-summary",
        model: runModel,
        providerSource: createProviderResolver(this.kernel.registries.providers),
        store: createMemorySessionStore(),
        redactor: this.redactor,
      });
      const workerSession = worker.createSession();
      const result = await workerSession.run(
        `Summarize the abandoned branch of this coding session in at most 6 sentences,\n`
          + `covering what was attempted and why it was left behind. Entries:\n${branchText}`,
      );
      const text = clipUtf8(result.text, MAX_BRANCH_SUMMARY_BYTES);
      if (!text) return;
      const refined = createSessionEntry({
        sessionId,
        parentId: branchPointId,
        kind: "summary",
        summary: text,
        data: { fromId: fromLeaf, pending: false },
      });
      await this.persistence.append(refined);
      // Re-root only when nothing else moved the leaf meanwhile.
      const current = this.live.get(sessionId);
      if (current && current.session.leafId === preview.entryId) {
        await current.session.checkout(refined.id);
      }
    } catch {
      // The deterministic preview stays; the summary is never lost.
    } finally {
      if (this.branchSummarizing.get(sessionId) === branchPointId) {
        this.branchSummarizing.delete(sessionId);
      }
    }
  }

  private sessionFork(params: Record<string, unknown>): unknown {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const entryId = optString(params, "entryId");
    const forked = live.session.fork({ ...(entryId ? { leafId: entryId } : {}) });
    // Same session id, same store, different branch. Abandoned side is kept.
    return { sessionId: forked.id, leafId: forked.leafId };
  }

  private async sessionClone(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const entryId = optString(params, "entryId");
    const id = optString(params, "id") ?? randomUUID();
    // Pre-create the record so Prism's clone entry-appends attach to a
    // properly tenanted session instead of auto-creating a tenant-less row.
    const now = new Date().toISOString();
    if (typeof this.persistence.appendSession !== "function") {
      throw rpcError(-32000, "session store cannot persist session records");
    }
    await this.persistence.appendSession({
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
  private async sessionCheckpoint(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const entryId = reqString(params, "entryId");
    if (!this.live.has(sessionId)) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    return (await this.request("checkpoint.capture", { sessionId, entryId })) as unknown;
  }

  /** Resume a suspended durable run. Validates decision shape before any checkpoint I/O. */
  private async runResume(params: Record<string, unknown>): Promise<unknown> {
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
        : this.live.get(sessionId)?.suspension?.version;
    if (expectedVersion === undefined) {
      throw rpcError(-32602, "params.expectedVersion must be a positive integer");
    }
    const decision = optString(params, "decision");
    const decisionsRaw = params.decisions;
    const decisions = Array.isArray(decisionsRaw) ? decisionsRaw.map((item, index) => this.resumeDecision(item, index)) : undefined;
    if (!decision && !decisions) throw rpcError(-32602, "params.decision or params.decisions is required");
    if (decision && decisions) throw rpcError(-32602, "params.decision and params.decisions are mutually exclusive");
    if (decision !== undefined && decision !== "approve" && decision !== "deny") {
      throw rpcError(-32602, `unknown decision: ${decision}`);
    }
    const live = await this.ensureLive(sessionId);
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
          checkpoints: this.persistence.checkpoints,
          definitionRevision: RUN_STATE_REVISION,
          ownership: this.runOwnership(),
        },
      )) {
        lastType = event.type;
        if (event.type === "agent_suspended") {
          // Re-arm the stash: chained approvals resume from here.
          suspended = { runId: event.runId, interruption: event.interruption, version: event.version };
          live.suspension = { runId, version: event.version };
        }
        this.emit("event", { sessionId, event: redactAgentEvent(event, this.redactor) satisfies AgentEvent });
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

  private resumeDecision(value: unknown, index: number): RunDecision {
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

  private async providerList(): Promise<unknown> {
    const auth = this.kernel.registries.authMethods.list();
    const providers = this.kernel.registries.providers.list().map((provider) => ({
      id: provider.id,
      auth: authMethodsFor(auth, provider.id),
    }));
    const extra = auth
      .filter((method) => !providers.some((item) => item.id === method.provider))
      .map((method) => ({
        id: method.provider,
        auth: authMethodsFor(auth, method.provider),
      }));
    const listed = [...providers, ...extra].map((provider) => ({
      ...provider,
      auth: withUrlMethod(provider.id, provider.auth),
    }));
    const out = [];
    for (const provider of listed) {
      out.push({
        ...provider,
        configured: await this.providerConfigured(provider.id, provider.auth),
      });
    }
    return { providers: out };
  }

  private async providerConfigured(
    id: string,
    methods: Array<{ kind: string; credentialName?: string }>,
  ): Promise<boolean> {
    for (const method of methods) {
      if (method.kind === "api_key" || method.kind === "url") {
        const name = method.credentialName ?? (method.kind === "url" ? "baseUrl" : "apiKey");
        if (await this.vault.get({ name, provider: id })) return true;
      } else if (method.kind === "oauth" && (await this.vault.getOAuth(id))) {
        return true;
      }
    }
    return false;
  }

  private async providerStatus(params: Record<string, unknown>): Promise<unknown> {
    const id = reqString(params, "id");
    const methods = this.kernel.registries.authMethods.list().filter((method) => method.provider === id);
    let configured = false;
    for (const method of methods) {
      if (method.kind === "api_key") {
        const name = "credentialName" in method ? (method.credentialName ?? "apiKey") : "apiKey";
        configured = Boolean(await this.vault.get({ name, provider: id }));
      } else if (method.kind === "oauth") {
        configured = Boolean(await this.vault.getOAuth(id));
      }
      if (configured) break;
    }
    return { id, configured, present: Boolean(this.kernel.registries.providers.get(id) || methods.length) };
  }

  private async modelList(): Promise<unknown> {
    if (!this.ollamaDiscoveryAttempted && this.kernel.registries.providers.get("ollama")) {
      await this.refreshOllamaModels();
    }
    return {
      models: this.kernel.registries.models.list().map((model) => ({
        provider: model.provider,
        model: model.model,
        displayName: model.displayName,
        // Generic bounded capability (plan 108 task 9): status-row
        // context-used-vs-window without a package-specific channel.
        ...(model.limits?.contextWindow !== undefined
          ? { contextWindow: model.limits.contextWindow }
          : {}),
        // Plan 109 I4: declared portable thinking levels (ascending), from
        // Prism registry metadata — no provider call. Omitted for
        // non-reasoning / undeclared models.
        ...(thinkingLevelsForModel(model)
          ? { thinkingLevels: thinkingLevelsForModel(model) }
          : {}),
      })),
    };
  }

  private modelSearch(params: Record<string, unknown>): unknown {
    const query = reqString(params, "query").toLowerCase();
    const models = this.kernel.registries.models
      .list()
      .filter((model) =>
        [model.provider, model.model, model.displayName ?? ""].some((part) => part.toLowerCase().includes(query)),
      )
      .slice(0, MAX_LIST)
      .map((model) => ({ provider: model.provider, model: model.model, displayName: model.displayName }));
    return { models };
  }

  private async credentialPut(params: Record<string, unknown>): Promise<unknown> {
    const provider = reqString(params, "provider");
    const secret = reqString(params, "secret");
    const name = optString(params, "name") ?? this.defaultCredentialName(provider);
    const type = optString(params, "type") === "bearer" ? "bearer" : "api_key";
    await this.vault.set({ name, provider, credential: { type, value: secret } });
    this.rememberSecret(secret);
    if (this.keychain) {
      try {
        await this.keychain.set({ name, provider, credential: { type, value: secret } });
      } catch {
        // Encrypted vault already persisted; keychain is best-effort.
      }
    }
    if (provider === "ollama") {
      this.ollamaDiscoveryAttempted = false;
      await this.refreshOllamaModels();
    }
    return { provider, name, stored: true };
  }

  private async refreshOllamaModels(): Promise<void> {
    if (this.ollamaDiscoveryAttempted) return;
    if (!this.kernel.registries.providers.get("ollama")) {
      this.ollamaDiscoveryAttempted = true;
      return;
    }
    const apiKey = await this.vault.get({ name: "apiKey", provider: "ollama" });
    const base = await this.vault.get({ name: "baseUrl", provider: "ollama" });
    if (!apiKey && !base) return;
    this.ollamaDiscoveryAttempted = true;
    try {
      const { listOllamaModels } = await import("@arnilo/prism-providers/ollama");
      const models = await listOllamaModels({
        apiKey: apiKey?.value,
        baseUrl: base?.value,
        signal: AbortSignal.timeout(3000),
      });
      for (const model of models) {
        if (this.kernel.registries.models.get(model.provider, model.model) === undefined) {
          this.kernel.registries.models.register(model);
        }
      }
    } catch {
      // Best-effort: local daemon down or cloud unreachable.
    }
  }

  private defaultCredentialName(provider: string): string {
    const method = this.kernel.registries.authMethods.list().find((item) => item.provider === provider && item.kind === "api_key");
    return method && "credentialName" in method ? (method.credentialName ?? "apiKey") : "apiKey";
  }

  private async oauthStart(params: Record<string, unknown>): Promise<unknown> {
    const provider = reqString(params, "provider");
    const method = this.kernel.registries.authMethods.list().find((item) => item.provider === provider && isOauth(item));
    if (!method || !isOauth(method) || !method.oauth) throw rpcError(-32000, `No OAuth method for ${provider}`);
    const loginId = randomUUID();
    let settle!: () => void;
    const done = new Promise<void>((resolve) => {
      settle = resolve;
    });
    let loginError: unknown;
    const abort = new AbortController();
    const pending: PendingOauth = { provider, done, promise: Promise.resolve({} as OAuthCredentials) };
    pending.promise = Promise.resolve(
      method.oauth.login({
        signal: abort.signal,
        onDeviceCode(code) {
          pending.info = { userCode: code.userCode, verificationUri: code.verificationUri };
          settle();
        },
        onAuth(url) {
          pending.info = { authorizationUrl: url };
          settle();
        },
      }),
    );
    void pending.promise.then(settle, (error) => {
      loginError = error;
      settle();
    });
    this.oauth.set(loginId, pending);
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<never>((_, reject) => {
      timer = setTimeout(() => {
        abort.abort();
        reject(rpcError(-32000, `OAuth start timed out for ${provider}`));
      }, OAUTH_START_TIMEOUT_MS);
    });
    try {
      await Promise.race([pending.done, timeout]);
    } finally {
      if (timer !== undefined) clearTimeout(timer);
    }
    if (pending.info) {
      return { loginId, provider, status: "started", ...pending.info };
    }
    this.oauth.delete(loginId);
    abort.abort();
    if (loginError) {
      const message = loginError instanceof Error ? loginError.message : String(loginError);
      throw rpcError(-32000, message);
    }
    throw rpcError(-32000, `OAuth start produced no device code or authorization URL for ${provider}`);
  }

  private async oauthPoll(params: Record<string, unknown>): Promise<unknown> {
    const loginId = reqString(params, "loginId");
    const pending = this.oauth.get(loginId);
    if (!pending) throw rpcError(-32000, `Unknown OAuth login: ${loginId}`);
    const raced = await Promise.race([
      pending.promise.then((credentials) => ({ credentials })),
      new Promise<{ pending: true }>((resolve) => setTimeout(() => resolve({ pending: true }), 25)),
    ]);
    if ("pending" in raced) return { loginId, status: "pending", ...(pending.info ?? {}) };
    await this.vault.setOAuth(pending.provider, raced.credentials);
    if (raced.credentials.access) this.rememberSecret(raced.credentials.access);
    if (raced.credentials.refresh) this.rememberSecret(raced.credentials.refresh);
    this.oauth.delete(loginId);
    return { loginId, status: "complete", provider: pending.provider, accountId: raced.credentials.accountId };
  }

  private async credentialDelete(params: Record<string, unknown>): Promise<unknown> {
    const provider = reqString(params, "provider");
    const name = optString(params, "name") ?? this.defaultCredentialName(provider);
    const deleted = await this.vault.delete({ name, provider });
    await this.vault.deleteOAuth(provider);
    if (this.keychain) {
      try {
        await this.keychain.delete({ name, provider });
        await this.keychain.deleteOAuth(provider);
      } catch {
        // ignore
      }
    }
    return { provider, name, deleted };
  }

  private profileList(): unknown {
    return {
      profiles: this.kernel.registries.agents.list().map((profile) => ({
        name: profile.name,
        description: profile.description,
        tools: profile.tools ?? [],
        skills: profile.skills ?? [],
      })),
    };
  }

  private profileRegister(params: Record<string, unknown>): unknown {
    const name = reqString(params, "name");
    const description = optString(params, "description");
    const instructions = optString(params, "instructions");
    const tools = Array.isArray(params.tools) ? params.tools.filter((item): item is string => typeof item === "string") : undefined;
    const skills = Array.isArray(params.skills)
      ? params.skills.filter((item): item is string => typeof item === "string")
      : undefined;
    const def: AgentDefinition = {
      name,
      ...(description ? { description } : {}),
      ...(instructions ? { instructions } : {}),
      ...(tools ? { tools } : {}),
      ...(skills ? { skills } : {}),
    };
    this.kernel.registries.agents.register(name, def);
    process.stderr.write(`agentProfile.register '${name}' applied\n`);
    if (params.observationalMemory === true) this.omProfiles.add(name);
    else if (params.observationalMemory === false) this.omProfiles.delete(name);
    return { name, registered: true, observationalMemory: this.omProfiles.has(name) };
  }

  /** Register inert skill data on the kernel registry (re-registering a name replaces the definition). */
  private skillRegister(params: Record<string, unknown>): unknown {
    const name = reqString(params, "name");
    const description = optString(params, "description");
    const instructions = optString(params, "instructions");
    const toolNames = Array.isArray(params.toolNames)
      ? params.toolNames.filter((item): item is string => typeof item === "string")
      : undefined;
    const metadata = params.metadata && typeof params.metadata === "object" && !Array.isArray(params.metadata)
      ? (params.metadata as Record<string, unknown>)
      : undefined;
    this.skills.register({
      name,
      ...(description !== undefined ? { description } : {}),
      ...(instructions !== undefined ? { instructions } : {}),
      ...(toolNames && toolNames.length > 0 ? { toolNames } : {}),
      ...(metadata !== undefined ? { metadata } : {}),
    });
    return { name, registered: true };
  }

  /** Progressive catalog: name + description only. Full instructions load via `load_skill`.
   *  Wiki skills appear only while the wiki option is bound (no residue when disabled). */
  private skillList(): unknown {
    return {
      skills: this.visibleSkills(),
    };
  }

  /** Catalog-visible skills (wiki/graft skills hide while their option is
   *  unbound). Bounded to the environment's wire caps. */
  private visibleSkills(): Array<{ name: string; description: string }> {
    return this.skills
      .list()
      .filter(
        (skill) =>
          (this.wiki !== undefined || !skill.name.startsWith("wiki-")) &&
          (this.graft !== undefined || skill.name !== graftSkill.name),
      )
      .slice(0, MAX_COMPLETION_COMMANDS)
      .map((skill) => ({
        name: skill.name.slice(0, MAX_COMPLETION_NAME_CHARS),
        description: (skill.description ?? "").slice(0, MAX_COMPLETION_DESCRIPTION_CHARS),
      }));
  }

  /** Register a command definition (data + host-side handler name). Drivers are
   *  never accepted over RPC; the host injects them at dispatch time. */
  private commandRegister(params: Record<string, unknown>): unknown {
    const name = reqString(params, "name");
    const handler = optString(params, "handler");
    if (handler !== undefined && !isCommandHandlerName(handler)) {
      throw rpcError(-32602, `unknown command handler: ${handler}`);
    }
    if (this.kernel.registries.commands.get(name)) throw rpcError(-32602, `duplicate command: ${name}`);
    const description = optString(params, "description");
    const parameters = params.parameters && typeof params.parameters === "object" && !Array.isArray(params.parameters)
      ? (params.parameters as JsonObject)
      : undefined;
    const metadata = params.metadata && typeof params.metadata === "object" && !Array.isArray(params.metadata)
      ? (params.metadata as Record<string, unknown>)
      : undefined;
    const command: CommandDefinition = {
      name,
      ...(description !== undefined ? { description } : {}),
      ...(parameters !== undefined ? { parameters } : {}),
      ...(metadata !== undefined ? { metadata } : {}),
      execute: (args, context) => this.runHostCommand(name, handler, args, context),
    };
    this.kernel.registries.commands.register(name, command);
    return { name, registered: true, ...(handler !== undefined ? { handler } : {}) };
  }

  /** Package commands register as "/name"; kernel-registered commands
   *  (e.g. the opt-in wiki commands) use bare names — try both so slash
   *  invocation reaches any registered command. */
  private registeredCommand(name: string): CommandDefinition | undefined {
    const registry = this.kernel.registries.commands;
    return registry.get(name) ?? registry.get(name.slice(1));
  }

  /** Plan 109 R1/R3: the composer completion's single source — the
   *  daemon's registered commands (names normalized to `/name`) — plus
   *  the loaded opt-in extensions for the status strip. Registered
   *  post-boot (init.js) commands appear on the next fetch; the Rust
   *  server caches per daemon generation and invalidates on
   *  command.register / knowledge.setOptions. */
  private environmentList(params: Record<string, unknown> = {}): JsonObject {
    const commands: JsonObject[] = [];
    for (const command of this.kernel.registries.commands.list()) {
      if (commands.length >= MAX_COMPLETION_COMMANDS) break;
      const raw = typeof command.name === "string" ? command.name : "";
      if (!raw) continue;
      const name = (raw.startsWith("/") ? raw : `/${raw}`).slice(
        0,
        MAX_COMPLETION_NAME_CHARS,
      );
      const description = (
        typeof command.description === "string" ? command.description : ""
      ).slice(0, MAX_COMPLETION_DESCRIPTION_CHARS);
      commands.push({ name, description });
    }
    const extensions: string[] = [];
    if (this.wiki) extensions.push(this.wiki.loaded.name.slice(0, 48));
    if (this.graft) extensions.push(this.graft.loaded.name.slice(0, 48));
    // Catalog skills ride the same snapshot (skills card on the coding
    // surface): bounded name/description pairs, same caps as commands.
    // Per-server MCP connect outcomes ride too (MCP card / composer
    // section): id, connected, tool count, and the hidden-because error.
    // Plan 118 task 35: the inventories follow the session's agent when one
    // is named (the Context tab's skills + MCP cards after a switch);
    // without a session the default agent's view is the honest answer.
    const sessionId = optString(params, "sessionId");
    const agentRoot = sessionId
      ? (this.live.get(sessionId)?.agentRoot ?? this.agentConfigRoot)
      : // The Rust server caches per agent type and asks by name; an absent
        // agent is the default one.
        this.resolveAgentRoot(params.agent);
    return {
      commands,
      extensions,
      // Ceiling (plan 118 task 35): the skill registry is host-wide, so the
      // catalog can list a skill another agent root discovered. A run's
      // *active* skills/tools come from the session's own agent config, which
      // is what the switch changes; per-root catalogs need a source tag on
      // the registry, which Prism's skill registry does not carry.
      skills: this.visibleSkills(),
      mcpServers: this.mcpServerOutcomes(agentRoot),
    };
  }

  private async commandDispatch(params: Record<string, unknown>): Promise<unknown> {
    const name = reqString(params, "name");
    const command = this.registeredCommand(name);
    if (!command) throw rpcError(-32602, `unknown command: ${name}`);
    const sessionId = optString(params, "sessionId");
    const live = sessionId ? this.live.get(sessionId) : undefined;
    // Host-injected drivers; absent when no live session so the context key is omitted.
    const drivers = this.commandDrivers(live);
    const context: CommandExecutionContext = drivers
      ? { ...(sessionId ? { sessionId } : {}), drivers }
      : sessionId
        ? { sessionId }
        : {};
    const args = params.args && typeof params.args === "object" && !Array.isArray(params.args)
      ? (params.args as JsonObject)
      : {};
    return command.execute(args, context);
  }

  /** Coding-run policy caps + default compact strategy. Partial update;
   *  `null` disables that Prism axis. compactAfterTokens is stored and
   *  unused until auto-compact is wired. */
  private runSetOptions(params: Record<string, unknown>): unknown {
    const maxTurns = optPolicyCap(params, "maxTurns");
    const maxToolRounds = optPolicyCap(params, "maxToolRounds");
    const maxToolCalls = optPolicyCap(params, "maxToolCalls");
    const maxWallTimeMs = optPolicyCap(params, "maxWallTimeMs");
    const maxInputTokens = optPolicyCap(params, "maxInputTokens");
    const maxOutputTokens = optPolicyCap(params, "maxOutputTokens");
    const compactAfterTokens = optPositiveInt(params, "compactAfterTokens");
    const compactionRaw = params.compaction;
    let compaction: CompactionStrategyName | undefined;
    if (compactionRaw !== undefined) {
      if (typeof compactionRaw !== "string" || !isCompactionStrategyName(compactionRaw)) {
        throw rpcError(-32602, "compaction must be one of default|llm|om");
      }
      compaction = compactionRaw;
    }
    if (
      maxTurns === undefined &&
      maxToolRounds === undefined &&
      maxToolCalls === undefined &&
      maxWallTimeMs === undefined &&
      maxInputTokens === undefined &&
      maxOutputTokens === undefined &&
      compactAfterTokens === undefined &&
      compaction === undefined
    ) {
      throw rpcError(
        -32602,
        "run.setOptions requires a policy cap, compactAfterTokens, and/or compaction",
      );
    }
    this.runConfig = {
      maxTurns: maxTurns !== undefined ? maxTurns : this.runConfig.maxTurns,
      maxToolRounds: maxToolRounds !== undefined ? maxToolRounds : this.runConfig.maxToolRounds,
      maxToolCalls: maxToolCalls !== undefined ? maxToolCalls : this.runConfig.maxToolCalls,
      maxWallTimeMs: maxWallTimeMs !== undefined ? maxWallTimeMs : this.runConfig.maxWallTimeMs,
      maxInputTokens: maxInputTokens !== undefined ? maxInputTokens : this.runConfig.maxInputTokens,
      maxOutputTokens: maxOutputTokens !== undefined ? maxOutputTokens : this.runConfig.maxOutputTokens,
      compactAfterTokens: compactAfterTokens ?? this.runConfig.compactAfterTokens,
      compaction: compaction ?? this.runConfig.compaction,
    };
    return { ...this.runConfig };
  }

  /** Opt-in wiki knowledge base (decision 2156, plan 108 task 12). Disabled
   *  by default: nothing loads, no commands/tools/skills registered, no
   *  residue. Enabling loads `@arnilo/prism-memory/wiki` via `kernel.load`,
   *  which registers the /wiki-init, /wiki-refresh, /wiki-lint commands,
   *  the wiki_search/wiki_read_page/wiki_record_insight tools, and the
   *  wiki-searcher/wiki-maintainer skills (progressive disclosure via
   *  load_skill). Wiki writes stay inside the workspace .wiki/ tree: the
   *  extension's skill-file deployment is disabled, skills come from the
   *  registry instead. */
  private async knowledgeSetOptions(params: Record<string, unknown>): Promise<unknown> {
    const workspaceRoot = reqString(params, "workspaceRoot");
    const wiki = typeof params.wiki === "boolean" ? params.wiki : undefined;
    const graft = typeof params.graft === "boolean" ? params.graft : undefined;
    if (wiki === undefined && graft === undefined) {
      throw rpcError(-32602, "knowledge.setOptions requires a boolean `wiki` and/or `graft` flag");
    }
    const graftMode = optString(params, "graftMode");
    if (graftMode !== undefined && graftMode !== "pull" && graftMode !== "push" && graftMode !== "both") {
      throw rpcError(-32602, "knowledge.setOptions `graftMode` must be one of pull|push|both");
    }
    if (wiki === false) this.disableWiki();
    else if (wiki === true) await this.enableWiki(workspaceRoot, optString(params, "qmdPath"));
    let graftResolved: boolean | undefined;
    if (graft === false) this.disableGraft();
    else if (graft === true) {
      graftResolved = await this.enableGraft(workspaceRoot, {
        ...(graftMode ? { mode: graftMode } : {}),
        ...(optString(params, "graftCliPath") ? { cliPath: optString(params, "graftCliPath") } : {}),
      });
    }
    return {
      workspaceRoot,
      ...(wiki !== undefined ? { wiki } : {}),
      ...(graft !== undefined ? { graft: graftResolved ?? graft } : {}),
    };
  }

  private disableWiki(): void {
    const current = this.wiki;
    if (!current) return;
    this.wiki = undefined;
    current.loaded.dispose();
  }

  private async enableWiki(workspaceRoot: string, qmdPath?: string): Promise<void> {
    if (this.wiki?.workspaceRoot === workspaceRoot) return;
    // One wiki binding per daemon: dispose the previous workspace's
    // extension (its registry contributions go with it) before loading the
    // new one — the wiki tools are single-named singletons. A failed load
    // leaves the option disabled (fail closed), never half-bound.
    const previous = this.wiki;
    this.wiki = undefined;
    previous?.loaded.dispose();
    const options: WikiExtensionOptions = {
      workspaceRoot,
      // Relative default: ".wiki" in the workspace root. The workspace
      // option carries the binding — an absolute wikiRoot here breaks the
      // tool-side path resolution.
      wikiRoot: ".wiki",
      // Skill files deploy into the workspace on init by default; skills
      // load from the kernel registry instead (wiki writes stay inside
      // .wiki/).
      autoDeploySkills: false,
      // qmd hybrid search is strictly opt-in (host-owned binary, deny by
      // default): without an explicit path the catalog fallback serves.
      ...(qmdPath ? { qmdPath } : {}),
    };
    let loaded: LoadedExtension;
    try {
      [loaded] = await this.kernel.load([createWikiExtension(options)]);
    } catch (error) {
      throw rpcError(-32000, `wiki activation failed: ${error instanceof Error ? error.message : String(error)}`);
    }
    this.wiki = { workspaceRoot, loaded };
    // Agent-delivered skills register from their file-backed content
    // (loadAgentSkillFiles at host creation), gated per skill (decision
    // 2026-09-09-1420). The feature gate (wiki-searcher) is checked by the
    // /wiki-init initiator.
    for (const [dir, key, fallback] of [
      ["wiki-searcher", "wikiSearcher", wikiSearcherSkill],
      ["wiki-maintainer", "wikiMaintainer", wikiMaintainerSkill],
    ] as const) {
      const delivered = this.agentSkillFiles.get(dir) ?? fallback;
      if (!this.skills.get(dir) && this.agentSkillEnabled(key)) {
        this.skills.register(delivered);
      }
    }
  }

  /** Wiki knowledge tools for the bound workspace; empty when the option is
 *  disabled or the session's workspace is not the bound one. */
  private wikiTools(workspaceRoot: string): ToolDefinition[] {
    if (this.wiki?.workspaceRoot !== workspaceRoot) return [];
    return [WIKI_SEARCH_TOOL_NAME, WIKI_READ_PAGE_TOOL_NAME, WIKI_RECORD_INSIGHT_TOOL_NAME].map((name) =>
      this.kernel.registries.tools.resolve(name),
    );
  }

  /** Graft knowledge tools for the bound workspace (pull/both modes only —
   *  push-only registers no tools). Empty when disabled, unbound, or when
   *  the CLI resolved without registering a given tool. */
  private graftTools(workspaceRoot: string): ToolDefinition[] {
    const binding = this.graft;
    if (binding?.workspaceRoot !== workspaceRoot || binding.mode === "push") return [];
    return GRAFT_TOOL_NAMES.map((name) => this.kernel.registries.tools.get(name)).filter(
      (tool): tool is ToolDefinition => tool !== undefined,
    );
  }

  /** Opt-in graft knowledge base (plan 108 task 13). Disabled by default.
   *  Enabling resolves the graft CLI fail-closed BEFORE load: an absent CLI
   *  (no cliPath, no host packageRoot, no @nanonets/graft peer) leaves the
   *  option off and the agent unperturbed — tools hidden, never a half
   *  binding. One binding per daemon; re-enabling rebinds. */
  private async enableGraft(
    workspaceRoot: string,
    options: { mode?: GraftMode; cliPath?: string },
  ): Promise<boolean> {
    // agentSkills.graft=false ⇒ no binding attempt ever, on any path
    // (default or explicit RPC) — the gate is authoritative (decision
    // 2026-09-09-1420).
    if (!this.agentSkillEnabled("graft")) return false;
    if (this.graft?.workspaceRoot === workspaceRoot) return true;
    const previous = this.graft;
    this.graft = undefined;
    previous?.loaded.dispose();
    try {
      resolveGraftCli(
        options.cliPath ? { cliPath: options.cliPath } : this.graftCliPath ? { cliPath: this.graftCliPath } : {},
      );
    } catch {
      // GraftResolveError: no host-owned way to run the graft CLI. Fail
      // closed — tools stay hidden, nothing loads.
      return false;
    }
    const extensionOptions: GraftExtensionOptions = {
      projectDir: workspaceRoot,
      mode: options.mode ?? "pull",
      ...(options.cliPath ? { cliPath: options.cliPath } : {}),
      // Push-surface persistence (OM attach pattern). State patches route
      // by the entry's own sessionId; getEntries rides the active run.
      appendEntry: (entry, appendOptions) => this.persistence.append(entry, appendOptions),
      getEntries: () =>
        this.activeGraftSessionId ? this.persistence.list(this.activeGraftSessionId) : Promise.resolve([]),
    };
    let loaded: LoadedExtension;
    try {
      [loaded] = await this.kernel.load([createGraftExtension(extensionOptions)]);
    } catch (error) {
      throw rpcError(-32000, `graft activation failed: ${error instanceof Error ? error.message : String(error)}`);
    }
    this.graft = { workspaceRoot, loaded, mode: extensionOptions.mode ?? "pull" };
    const graftDelivered = this.agentSkillFiles.get("graft") ?? graftSkill;
    if (!this.skills.get(graftDelivered.name)) this.skills.register(graftDelivered);
    return true;
  }

  private disableGraft(): void {
    const current = this.graft;
    if (!current) return;
    this.graft = undefined;
    this.activeGraftSessionId = undefined;
    current.loaded.dispose();
  }

  /** Graft default-on trigger (plan 117): first coding session for a
   *  workspace attempts the pull-mode binding, once per root per daemon —
   *  no retry loops. Fail-closed: unresolvable CLI ⇒ off, tools hidden,
   *  agent unperturbed. knowledge.setOptions stays the explicit
   *  disable/mode-override surface and bypasses the attempt cache. */
  private async ensureGraftBound(workspaceRoot: string): Promise<boolean> {
    if (!this.agentSkillEnabled("graft")) return false;
    if (this.graft?.workspaceRoot === workspaceRoot) return true;
    if (this.graftBindAttempted.has(workspaceRoot)) return false;
    this.graftBindAttempted.add(workspaceRoot);
    try {
      return await this.enableGraft(workspaceRoot, {
        mode: "pull",
        ...(this.graftCliPath ? { cliPath: this.graftCliPath } : {}),
      });
    } catch {
      // Default bind never surfaces errors (agent unperturbed): a load
      // failure (broken CLI binary, extension error) fails closed exactly
      // like an unresolvable CLI. The explicit RPC path still throws for
      // caller feedback.
      return false;
    }
  }

  /** Host-verified run identity. The daemon is the trust boundary: it owns
   *  the workspace root and the acceptance policy, so it vouches for its own
   *  runs. Never sourced from RPC params. */
  private runIdentity(): AgentIdentity {
    return {
      tenantId: TENANT,
      principal: { kind: "service", id: "clay-agent" },
      scopes: ["workspace"],
      issuedAt: new Date().toISOString(),
      verified: true,
    };
  }

  /** Ownership projected from the run identity — the same scope the run's
   *  durable effects and checkpoints are recorded under. */
  private runOwnership(): ReturnType<typeof ownershipFromIdentity> {
    return ownershipFromIdentity(this.runIdentity());
  }

  /** Host driver capabilities for commands. Never sourced from RPC params. */
  private commandDrivers(live: LiveSession | undefined): CommandDrivers | undefined {
    if (!live) return undefined;
    return {
      startRun: (input, options) => live.session.run(input, { ...(options ?? {}), identity: this.runIdentity() }),
      startWorkflow: async () => {
        throw rpcError(-32000, "workflows not enabled (Phase 5)");
      },
      steer: (_runId, input) => live.session.steer(input),
    };
  }

  /** Host-side execute for a registered command's named handler. */
  private runHostCommand(
    commandName: string,
    handler: string | undefined,
    args: JsonObject,
    context: CommandExecutionContext,
  ): Promise<CommandResult> | CommandResult {
    const noDriversError = () => rpcError(-32000, `${commandName} requires an active session (drivers absent)`);
    switch (handler) {
      case undefined:
        throw rpcError(-32000, `command ${commandName} has no host handler`);
      case "steer": {
        const drivers = context.drivers;
        if (!drivers) throw noDriversError();
        const input = reqString(args, "input");
        const runId = optString(args, "runId") ?? "";
        drivers.steer(runId, input);
        return { name: commandName, value: { steered: true, runId, input } };
      }
      case "startRun": {
        const drivers = context.drivers;
        if (!drivers) throw noDriversError();
        const input = reqString(args, "input");
        const options = args.options && typeof args.options === "object" ? (args.options as RunOptions) : undefined;
        return Promise.resolve(drivers.startRun(input, options)).then((result) => ({
          name: commandName,
          value: { runId: result.runId, status: result.status },
        }));
      }
      case "startWorkflow": {
        const drivers = context.drivers;
        if (!drivers) throw noDriversError();
        const definition = args.definition && typeof args.definition === "object" ? (args.definition as JsonObject) : {};
        return Promise.resolve(drivers.startWorkflow(definition, args.input)).then((result) => ({
          name: commandName,
          value: result,
        }));
      }
      case "compact": {
        const sessionId = this.requireCommandSession(commandName, context);
        return Promise.resolve(this.handle("session.compact", { sessionId })).then((value) => ({
          name: commandName,
          value,
        }));
      }
      case "newSession": {
        // Re-seed a fresh session from the current one's profile/provider/model.
        const current = this.requireLive(commandName, context);
        return Promise.resolve(
          this.handle("session.new", {
            profile: current.profile,
            provider: current.provider,
            model: current.model,
            workspaceRoot: current.workspaceRoot,
          }),
        ).then((value) => ({ name: commandName, value }));
      }
      case "checkout": {
        const sessionId = this.requireCommandSession(commandName, context);
        const entryId = optString(args, "entryId") ?? optString(args, "input");
        if (!entryId) throw rpcError(-32602, `${commandName} requires an entryId`);
        return Promise.resolve(this.handle("session.checkout", { sessionId, entryId })).then((value) => ({
          name: commandName,
          value,
        }));
      }
      case "discard": {
        // Post-hoc discard (decision 2200): checkout to the branch point —
        // conversation + document checkpoint roll back in one action, the
        // abandoned side keeps its entries and summary. Two-step confirm:
        // without `confirm` the reply previews and nothing changes.
        const sessionId = this.requireCommandSession(commandName, context);
        const entryId = optString(args, "entryId") ?? optString(args, "input");
        if (!entryId) throw rpcError(-32602, `${commandName} requires an entryId`);
        if (args.confirm !== true) {
          return Promise.resolve(this.sessionTreeSummary(sessionId)).then((tree) => ({
            name: commandName,
            value: { needsConfirm: true, entryId, reRun: `${commandName} ${entryId} confirm` },
          }));
        }
        return Promise.resolve(this.handle("session.checkout", { sessionId, entryId })).then((value) => ({
          name: commandName,
          value,
        }));
      }
      case "forkSession": {
        const sessionId = this.requireCommandSession(commandName, context);
        const entryId = optString(args, "entryId");
        return Promise.resolve(
          this.handle("session.fork", { sessionId, ...(entryId ? { entryId } : {}) }),
        ).then((value) => ({ name: commandName, value }));
      }
      case "cloneSession": {
        const sessionId = this.requireCommandSession(commandName, context);
        const entryId = optString(args, "entryId");
        return Promise.resolve(
          this.handle("session.clone", { sessionId, ...(entryId ? { entryId } : {}) }),
        ).then((value) => ({ name: commandName, value }));
      }
      case "tree": {
        const sessionId = this.requireCommandSession(commandName, context);
        return Promise.resolve(this.sessionTreeSummary(sessionId)).then((value) => ({
          name: commandName,
          value,
        }));
      }
      case "openSession": {
        // `--session`-equivalent: open a specific session by id (bounded page).
        const sessionId = reqString(args, "sessionId");
        return Promise.resolve(this.handle("session.load", { sessionId })).then((value) => ({
          name: commandName,
          value,
        }));
      }
      case "openSessionAsFork": {
        // `--fork`-equivalent: open a session and fork it at its leaf (or entryId).
        const sessionId = reqString(args, "sessionId");
        const entryId = optString(args, "entryId");
        return (async () => {
          await this.handle("session.resume", { sessionId });
          const value = await this.handle("session.fork", { sessionId, ...(entryId ? { entryId } : {}) });
          return { name: commandName, value };
        })();
      }
      default:
        throw rpcError(-32602, `unknown command handler: ${handler}`);
    }
  }

  /** The dispatch context's sessionId (never RPC-supplied drivers). */
  private requireCommandSession(commandName: string, context: CommandExecutionContext): string {
    const sessionId = context.sessionId;
    if (!sessionId) throw rpcError(-32000, `${commandName} requires an active session`);
    return sessionId;
  }

  private requireLive(
    commandName: string,
    context: CommandExecutionContext,
  ): { profile: string; provider: string; model: string; workspaceRoot: string } {
    const sessionId = this.requireCommandSession(commandName, context);
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `${commandName} requires an active session`);
    return live;
  }

  /** Branch summary for /tree (plan 108 task 10): every entry's id/parent/kind
   *  plus a short redacted preview, the branch list, stored branch summaries,
   *  checkpoint flags (server authority), and the in-flight summary markers.
   *  Bounded by the store's own entry list. */
  private async sessionTreeSummary(sessionId: string): Promise<unknown> {
    const live = this.live.get(sessionId);
    if (!live) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const entries = await live.session.entries();
    const summarized = new Map<string, unknown>();
    for (const entry of entries) {
      if (entry.kind === "summary" && entry.summary) {
        const data = entry.data && typeof entry.data === "object" ? (entry.data as Record<string, unknown>) : {};
        // Latest summary per branch point wins (the LLM refine appends later).
        summarized.set(entry.parentId ?? "", {
          entryId: entry.id,
          fromId: data.fromId,
          text: entry.summary,
          pending: data.pending === true,
        });
      }
    }
    let checkpointed: string[] = [];
    try {
      // Advisory data: never block the tree on a slow reverse consumer.
      const request = this.request("checkpoint.list", { sessionId });
      const budget = new Promise<{ entryIds?: unknown }>((resolve) => {
        setTimeout(() => resolve({}), 2_000);
      });
      const listed = (await Promise.race([request, budget])) as { entryIds?: unknown };
      if (Array.isArray(listed?.entryIds)) {
        checkpointed = listed.entryIds.filter((id): id is string => typeof id === "string");
      }
    } catch {
      // Checkpoint flags are advisory; a server without the method still gets a tree.
    }
    const checkpointSet = new Set(checkpointed);
    const summaries: Array<Record<string, unknown>> = [];
    const treeEntries = entries.slice(-MAX_LOAD_ENTRIES).map((entry) => {
      const redacted = this.redactor.redact(entry);
      const content: unknown = redacted.message?.content;
      const preview =
        redacted.label ??
        redacted.summary ??
        (typeof content === "string"
          ? content.slice(0, 80)
          : Array.isArray(content)
            ? content
                .map((part) =>
                  typeof part === "object" && part !== null && "text" in part
                    ? String((part as { text: unknown }).text)
                    : "",
                )
                .join("")
                .slice(0, 80)
            : undefined);
      const isSummary = redacted.kind === "summary";
      if (isSummary && redacted.summary) {
        const data = redacted.data && typeof redacted.data === "object" ? (redacted.data as Record<string, unknown>) : {};
        summaries.push({
          entryId: redacted.id,
          parentId: redacted.parentId,
          fromId: data.fromId,
          text: redacted.summary,
          pending: data.pending === true,
        });
      }
      return {
        id: redacted.id,
        ...(redacted.parentId ? { parentId: redacted.parentId } : {}),
        kind: redacted.kind,
        ...(checkpointSet.has(redacted.id) ? { checkpointed: true } : {}),
        ...(preview !== undefined && preview !== "" ? { preview } : {}),
      };
    });
    const branches = listSessionBranches(entries)
      .slice(-MAX_LOAD_ENTRIES)
      .map((branch) => ({ leafId: branch.leafId, entries: branch.entries.length }));
    return {
      sessionId,
      leafId: live.session.leafId,
      entries: treeEntries,
      summaries,
      branches,
      ...(this.branchSummarizing.has(sessionId)
        ? { summarizing: this.branchSummarizing.get(sessionId) }
        : {}),
    };
  }

  /** Render the /tree payload as bounded transcript text (pi-parity in-place
   *  view: entries with ids, leaf marker, checkpoints, branch summaries). */
  private async renderTreeText(sessionId: string): Promise<string> {
    const tree = (await this.sessionTreeSummary(sessionId)) as {
      leafId?: string;
      entries?: Array<Record<string, unknown>>;
      summaries?: Array<Record<string, unknown>>;
      summarizing?: string;
    };
    const lines: string[] = [`tree (leaf ${shortId(tree.leafId)}):`];
    for (const entry of (tree.entries ?? []).slice(-MAX_TREE_RENDER_ENTRIES)) {
      const marker = entry.id === tree.leafId ? " ←" : "";
      const checkpoint = entry.checkpointed === true ? " ⚑" : "";
      const preview = typeof entry.preview === "string" ? ` ${entry.preview}` : "";
      lines.push(`${entry.id === tree.leafId ? "●" : "○"} ${String(entry.id).slice(0, 8)} [${String(entry.kind)}]${preview}${checkpoint}${marker}`);
    }
    for (const summary of tree.summaries ?? []) {
      lines.push(
        `summary at ${String(summary.entryId).slice(0, 8)} (from ${String(summary.fromId ?? "?").slice(0, 8)}${summary.pending === true ? ", pending" : ""}): ${String(summary.text).slice(0, 160)}`,
      );
    }
    if (typeof tree.summarizing === "string") {
      lines.push(`(summarizing branch at ${tree.summarizing.slice(0, 8)}…)`);
    }
    return clipUtf8(lines.join("\n"), MAX_TREE_RENDER_BYTES);
  }

  /** Slash-command feedback rides the transcript lane: a synthetic
   *  started/delta/finished triple renders the result as an assistant message
   *  and closes the client's AG-UI run (the dispatch reply alone would leave
   *  the run observable open and the composer stuck streaming). Bounded. */
  private emitCommandFeedback(sessionId: string, commandName: string, value: unknown, text?: string): void {
    const record = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
    const body =
      text ??
      (record.needsConfirm === true
        ? `discard preview: branching from ${String(record.entryId).slice(0, 8)} restores that checkpoint and abandons the later path. Re-run with confirm to discard.`
        : typeof record.leafId === "string"
          ? `${commandName.slice(1)}: leaf ${record.leafId.slice(0, 8)}`
          : `${commandName.slice(1)}: done`);
    const runId = `cmd-${commandName.slice(1).slice(0, 32)}`;
    this.emit("event", { sessionId, event: { type: "agent_started", sessionId, runId } });
    this.emit("event", {
      sessionId,
      event: { type: "message_delta", sessionId, runId, content: { type: "text", text: body.slice(0, 8192) } },
    });
    this.emit("event", { sessionId, event: { type: "agent_finished", sessionId, runId } });
  }
}

const COMMAND_HANDLERS = [
  "startRun",
  "startWorkflow",
  "steer",
  // Slash-command handlers (pi-parity surface): each maps to the daemon's
  // own session.* RPCs. Session-scoped handlers require an active session
  // (drivers/context sessionId); they never elevate authority.
  "compact",
  "newSession",
  "checkout",
  "discard",
  "forkSession",
  "cloneSession",
  "tree",
  "openSession",
  "openSessionAsFork",
] as const;
type CommandHandlerName = (typeof COMMAND_HANDLERS)[number];

function isCommandHandlerName(value: string): value is CommandHandlerName {
  return (COMMAND_HANDLERS as readonly string[]).includes(value);
}

/** Path from the tree root to `entryId` (inclusive), oldest first. Used by
 *  `session.load { entryId }` for search-result opens (plan 108 task 11):
 *  read-only view of that branch — never mutates the tree. Unknown entry
 *  ids return undefined so the caller falls back to the default tail. */
function branchEntries(
  entries: readonly SessionEntry[],
  entryId: string,
): SessionEntry[] | undefined {
  const byId = new Map(entries.map((entry) => [entry.id, entry]));
  if (!byId.has(entryId)) return undefined;
  const path: SessionEntry[] = [];
  let current: string | undefined = entryId;
  while (current) {
    const entry = byId.get(current);
    if (!entry) break;
    path.unshift(entry);
    current = entry.parentId;
  }
  return path;
}

/** Entry text preview for branch summaries, bounded and redaction-safe
 *  (redaction happens on the caller side via entry redaction upstream). */
function branchText(entries: readonly SessionEntry[], fromLeaf: string | undefined, branchPointId: string): string {
  return branchTextForPrompt(entries, fromLeaf, branchPointId, 160);
}

/** Bounded prompt form: role/kind per entry, ≤ `previewChars` per line. */
function branchTextForPrompt(
  entries: readonly SessionEntry[],
  fromLeaf: string | undefined,
  branchPointId: string,
  previewChars = 200,
): string {
  const lines: string[] = [];
  let current: string | undefined = fromLeaf;
  let bytes = 0;
  while (current && current !== branchPointId) {
    const entry = entries.find((item) => item.id === current);
    if (!entry) break;
    const content: unknown = entry.message?.content;
    const body =
      entry.summary ??
      entry.label ??
      (typeof content === "string"
        ? content
        : Array.isArray(content)
          ? content
              .map((part) => (typeof part === "object" && part !== null && "text" in part ? String((part as { text: unknown }).text) : ""))
              .join("")
          : "");
    const line = `[${entry.kind}] ${body.slice(0, previewChars)}`;
    bytes += line.length + 1;
    if (bytes > 4096 || lines.length >= 24) {
      lines.unshift("…(older entries omitted)");
      break;
    }
    lines.unshift(line);
    current = entry.parentId;
  }
  return lines.join("\n");
}

/** Plan 109 I7: normalize the definition's system prompt config into
 *  contributions for the context inspector (`false` = disabled, absent =
 *  no explicit prompt). */
function systemPromptContributions(config: SystemPromptConfig | undefined): Array<{
  id: string;
  source?: string;
  text: string;
}> {
  if (!config) return [];
  const list = Array.isArray(config) ? config : [config];
  return list
    .filter((entry): entry is { id: string; source?: string; text: string } => typeof entry?.text === "string")
    .map((entry, index) => ({ id: entry.id || `prompt-${index}`, source: entry.source, text: entry.text }));
}

/** Mirror of Prism's `composeSystemPrompt` source ranks (system-prompts.js):
 *  the inspector lists system-prompt items in the order the model actually
 *  receives them. Unknown/custom sources sit after package, before app. */
const PROMPT_SOURCE_RANK: Readonly<Record<string, number>> = { user: 0, package: 1, app: 2, run: 3 };

function rankedSystemPromptContributions(config: SystemPromptConfig | undefined): Array<{
  id: string;
  source?: string;
  text: string;
}> {
  return systemPromptContributions(config)
    .map((contribution, index) => ({ contribution, index }))
    .sort(
      (a, b) =>
        (PROMPT_SOURCE_RANK[a.contribution.source ?? ""] ?? 1.5) -
          (PROMPT_SOURCE_RANK[b.contribution.source ?? ""] ?? 1.5) || a.index - b.index,
    )
    .map(({ contribution }) => contribution);
}

/** Friendly source labels for host-owned prompt layers (plan 117). */
const PROMPT_LAYER_LABELS: Readonly<Record<string, string>> = {
  "user-system-md": "user SYSTEM.md",
  "agents-md": "workspace AGENTS.md",
};

function promptLayerLabel(id: string): string {
  return PROMPT_LAYER_LABELS[id] ?? id;
}

/** Wire shape of one bounded context-inspector item reference. */
interface ContextItemRef {
  id: string;
  title: string;
  preview: string;
}

function clipUtf8(text: string, maxBytes: number): string {
  let end = Math.min(text.length, maxBytes);
  while (end > 0 && Buffer.byteLength(text.slice(0, end), "utf8") > maxBytes) end -= 1;
  return text.slice(0, end);
}

function shortId(id: string | undefined): string {
  return (id ?? "?").slice(0, 8);
}

/** Parse a slash-command invocation: `/name` alone or `/name {json args}` /
 *  `/name free text` (free text becomes `{ "input": text }`). Returns
 *  undefined for non-slash prompts. Bounded: name ≤ 64 chars, args ≤ 4 KiB. */
function commandInvocation(text: string): { name: string; argsText?: string } | undefined {
  const trimmed = text.trimStart();
  if (!trimmed.startsWith("/")) return undefined;
  const match = /^\/([A-Za-z0-9._-]{1,64})(?:[ \t]+([\s\S]*))?$/.exec(trimmed);
  if (!match) return undefined;
  const argsText = match[2]?.trim();
  if (argsText === undefined || argsText === "") return { name: `/${match[1]}` };
  if (argsText.length > 4096) return { name: `/${match[1]}` };
  return { name: `/${match[1]}`, argsText };
}

/** Slash args: a `{json}` object parses as the args; anything else (or
 *  non-object JSON) is free text carried as `{ input }`. Bounded upstream. */
function slashArgs(invocation: { argsText?: string }): JsonObject {
  if (invocation.argsText === undefined) return {};
  try {
    const parsed: unknown = JSON.parse(invocation.argsText);
    if (parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as JsonObject;
    }
  } catch {
    // Not JSON: free text.
  }
  return { input: invocation.argsText };
}

