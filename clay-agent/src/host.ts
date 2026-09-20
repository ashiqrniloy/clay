import { access, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { readFileSync, realpathSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, sep } from "node:path";
import { createHash, randomUUID } from "node:crypto";
import {
  type AgentDefinition,
  type AgentRunStateOptions,
  type AttentionTruncationTrigger,
  type CommandDefinition,
  type CommandDrivers,
  type ExtensionKernel,
  type JsonObject,
  type LoadedExtension,
  type ContentBlock,
  type SecretRedactor,
  type Skill,
  type ToolDefinition,
  createExplicitCredentialResolver,
  createExtensionKernel,
  createMockProvider,
  createSecretRedactor,
  createSkillRegistry,
  providerDone,
  providerTextDelta,
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
import { observeSupervisorLifecycle, type AskUserDecisionAnswer } from "@arnilo/prism-coding-tools/agent";
import * as hostSessions from "./host/sessions.js";
import * as hostSessionRun from "./host/session-run.js";
import * as hostCommands from "./host/commands.js";
import {
  type SupervisorHooks,
} from "@arnilo/prism-core/runtime/supervisor";
import {
  isWorkScopeOpenedData,
  type WorkScopeController,
  type WorkScopeSpec,
} from "@arnilo/prism-memory/compaction/observational-memory";
import type { SettingsProvider, SystemPromptConfig } from "@arnilo/prism";
import type { AgentIdentity } from "@arnilo/prism";
import { ownershipFromIdentity } from "@arnilo/prism";
import { CODING_TOOL_NAMES, buildCodingTools, normalizeToolCaps, type RepositoryToolCaps } from "./coding-tools.js";
import {
  type ConnectedMcpServers,
  type McpServerOutcome,
} from "./mcp.js";
import { resolveObscuraBinary, spawnObscuraHarness, type ObscuraHarness } from "./obscura.js";
import { runObscuraCli, validateObscuraWebUrl } from "@arnilo/prism-web-tools/obscura";
import {
  registerCompactionStrategies,
} from "./compaction.js";
import {
} from "@arnilo/prism-memory/wiki";
import {
  type GraftMode,
} from "@arnilo/prism-memory/graft";





















import { redactText } from "./redact.js";

import {
  asRecord,
  type AgentRootConfig,
  type EmitFn,
  type HostOptions,
  DEFAULT_RUN_CONFIG,
  hasCodingTools,
  labelFirstPromptStore,
  MAX_QUEUED_EVENTS,
  type LiveSession,
  type OmWorkerModels,
  persistProviderModel,
  REVERSE_TIMEOUT_MS,
  rpcError,
  type RunConfig,
  TENANT,
} from "./host/internals.js";
import {
  loadAgentSkillFiles,
  loadSkillsConfig,
  seedUserSystemPrompt,
  type SkillsConfig,
} from "./host/skills.js";
import * as hostSkills from "./host/skills.js";
import * as hostContext from "./host/context-mentions.js";
import * as hostProviders from "./host/providers.js";
import type { PendingOauth } from "./host/providers.js";
import * as hostIntegrations from "./host/integrations.js";
import * as hostAgentRoots from "./host/agent-roots.js";
import { CODING_AGENT_CONFIG_DIR, loadToolCaps } from "./host/agent-roots.js";
import * as hostOm from "./host/om.js";
import * as hostSessionBuild from "./host/session-build.js";
import * as hostSessionTree from "./host/session-tree.js";
import { branchEntries } from "./host/session-tree.js";
export { MAX_QUEUED_EVENTS } from "./host/internals.js";
export type { EmitFn, HostOptions } from "./host/internals.js";

const KEYCHAIN_SERVICE = "clay-agent";
/** Host-authored durable-run revision (must match across run + resume). */
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
























/** Local `YYYY-MM-DD HH:MM` for the resume list's second line. The daemon runs
 *  on the user's machine, so its timezone is the user's; a raw ISO stamp in
 *  UTC reads as the wrong hour. */
export class ClayAgentHost {
  readonly secrets = new Set<string>();
  redactor: SecretRedactor;
  readonly live = new Map<string, LiveSession>();
  readonly branchSummarizing = new Map<string, string>();
  readonly oauth = new Map<string, PendingOauth>();
  private readonly reversePending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (error: Error) => void }
  >();
  private nextReverseId = 1;
  private closed = false;
  readonly omProfiles = new Set<string>();
  /** Kernel skill registry; duplicate skill names fail closed (plan 107 Task 6). */
  readonly skills = createSkillRegistry([], { duplicate: "error" });
  /** Disk-discovered skill names. Config-root + home roots scanned once;
   *  each workspace root cached on first session. `undefined` = not yet
   *  scanned. Config/home roots scan `<root>/skills/`; workspace keeps the
   *  npx skills `.agents/skills/` layout. */
  configSkillNames: readonly string[] | undefined;
  homeSkillNames: readonly string[] | undefined;
  readonly workspaceSkillNames = new Map<string, readonly string[]>();
  /** The daemon's default agent root (the shipped coding agent). Sessions
   *  created without an agent use it; named agents resolve as its siblings
   *  under the same `agents/` directory. */
  readonly agentConfigRoot: string;
  readonly homeSkillsRoot: string;
  readonly skillsConfig: SkillsConfig;
  /** Plan 118 task 35: per-agent config, loaded once per root (bounded). */
  readonly agentRoots = new Map<string, Promise<AgentRootConfig>>();
  /** Plan 118 task 35: MCP bridges + outcomes per agent root, so switching
   *  an agent connects that agent's servers and never inherits another's. */
  readonly mcpByRoot = new Map<
    string,
    { mcp?: ConnectedMcpServers; outcomes: readonly McpServerOutcome[] }
  >();
  /** Plan 118 task 35: the server-built allow-list last seen per agent root
   *  (sessions + the switch carry it; a restore re-uses it). */
  readonly mcpAllowListByRoot = new Map<string, readonly unknown[]>();
  /** Connected MCP bridges of the default agent root (empty allow-list =
   *  never populated). Kept for the default-agent paths (initialize
   *  handshake, tests) and mirrored by `mcpByRoot`. */
  mcp: ConnectedMcpServers | undefined;
  /** Per-server MCP connect outcomes (last connect), for the UI surfaces. */
  mcpOutcomes: readonly McpServerOutcome[] = [];
  /** Owned Obscura harness (undefined = binary absent, capability hidden). */
  obscura: ObscuraHarness | undefined;
  /** Capabilities activated lazily on first coding session; serialized. */
  capabilitiesPromise: Promise<void> | undefined;
  readonly resolveObscura: () => string | undefined;
  /** Opt-in wiki knowledge base (decision 2156): undefined = disabled
   *  (default, nothing loaded). Bound to one workspace at a time — the
   *  daemon serves the active workspace; re-enabling rebinds. */
  wiki: { workspaceRoot: string; loaded: LoadedExtension } | undefined;
  /** Opt-in graft knowledge base (decision 2156, plan 108 task 13): same
   *  binding discipline as the wiki — one workspace, fail-closed CLI
   *  resolution, zero residue when disabled. */
  graft:
    | { workspaceRoot: string; loaded: LoadedExtension; mode: GraftMode; deepModelIdentity: string }
    | undefined;
  /** Workspace roots whose default graft bind was attempted (plan 117):
   *  one attempt per root per daemon, success or fail-closed. */
  readonly graftBindAttempted = new Set<string>();
  /** Session of the run currently in flight — graft push state (patches,
   *  orientation freshness) persists against it. Ceiling: one binding; the
   *  extension API carries no session key on `getEntries`. Upgrade path:
   *  thread sessionId through the prism extension options when it grows one. */
  activeGraftSessionId: string | undefined;
  /** Prism ollama ships no catalog; host must call listOllamaModels. */
  ollamaDiscoveryAttempted = false;
  /** User-configured repository scan caps (tool-caps.json in the agent
   *  config root, legacy fallback beside book.json; decision 2026-09-05).
   *  Empty = Prism defaults. */
  readonly toolCaps: RepositoryToolCaps;
  /** Caps file path cited in truncation errors. */
  readonly capsFile: string;
  /** Coding-run ceilings + default compact strategy (init.js run.setOptions). */
  runConfig: RunConfig = { ...DEFAULT_RUN_CONFIG };
  /** Plan 117 follow-up: the session store Prism writes through, with the
   *  resume label stamped on each session's opening entry. Read paths
   *  (`session.list` / `session.load` / search) keep using `persistence`. */
  readonly labeledStore: SqlitePersistence;

  private constructor(
    readonly dataDir: string,
    readonly persistence: SqlitePersistence,
    readonly vault: EncryptedCredentialStore,
    readonly keychain: KeychainCredentialStore | undefined,
    readonly kernel: ExtensionKernel,
    readonly emit: EmitFn,
    readonly omConfig: HostOptions["observationalMemory"],
    readonly mcpAllowList: readonly unknown[],
    resolveObscura: () => string | undefined,
    toolCaps: { caps: RepositoryToolCaps; file: string },
    agentConfigRoot: string,
    homeSkillsRoot: string,
    skillsConfig: SkillsConfig,
    readonly agentSkillFiles: ReadonlyMap<string, Skill>,
    readonly graftCliPath: string | undefined,
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
    for (const live of this.live.values()) {
      live.session.abort("shutdown");
      live.stopSubagentLifecycle?.();
    }
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
  ensureCapabilities(agentRoot?: string, allowList?: readonly unknown[]): Promise<void> {
    return hostAgentRoots.ensureCapabilities(this, agentRoot, allowList);
  }

  /** Capability tools for a coding session; undefined shapes stay allowed. */
  capabilityTools(agentRoot?: string): ToolDefinition[] {
    return hostAgentRoots.capabilityTools(this, agentRoot);
  }

  /** Per-server MCP connect outcomes for the environment/UI (bounded,
   *  trimmed). Plan 118 task 35: scoped to an agent root — a switched
   *  session reports the servers it actually has. */
  mcpServerOutcomes(agentRoot?: string): JsonObject[] {
    return hostAgentRoots.mcpServerOutcomes(this, agentRoot);
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
      default:
        return this.handleAgentSurface(method, params);
    }
  }

  /** Provider/model/credential, agent-profile/skill/command/environment,
   *  and run-policy RPCs: the non-session half of the dispatch table. */
  private async handleAgentSurface(method: string, params: unknown): Promise<unknown> {
    switch (method) {
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
  resolveAgentRoot(agent: unknown): string {
    return hostAgentRoots.resolveAgentRoot(this, agent);
  }

  /** The allow-list known for one agent root: the last one the server sent
   *  for it, else the initialize-time list (default agent) or none. */
  mcpAllowListFor(root: string): readonly unknown[] {
    return hostAgentRoots.mcpAllowListFor(this, root);
  }

  /** The default agent's config as the host loaded it at create time (the
   *  synchronous recreate paths need no await). */
  defaultAgentConfig(): AgentRootConfig {
    return hostAgentRoots.defaultAgentConfig(this);
  }

  /** The type name for a root: the default agent is `undefined` (the server
   *  records no name for it). */
  agentTypeOf(root: string): string | undefined {
    return hostAgentRoots.agentTypeOf(this, root);
  }

  /**
   * Plan 118 task 35: load (once) the config of one agent root — seeding its
   * SYSTEM.md and delivered skills first, then tool caps, `skills.json`, the
   * delivered skill files, and the root's discovered skill names. Bounded by
   * `MAX_AGENT_ROOTS`: past the cap a root loads uncached (correct, just
   * re-read) rather than dropping the session.
   */
  agentRootConfig(root: string): Promise<AgentRootConfig> {
    return hostAgentRoots.agentRootConfig(this, root);
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

  rememberSecret(secret: string): void {
    if (!secret) return;
    this.secrets.add(secret);
    this.refreshRedactor();
  }

  private async sessionNew(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionNew(this, params);
  }
  createSession(
    id: string,
    profile: string,
    provider: string,
    modelId: string,
    options: {
      workspaceRoot: string;
      fullAutonomy: boolean;
      observationalMemory: boolean;
      agent: AgentRootConfig;
      mcpAllowList: readonly unknown[];
    },
  ) {
    return hostSessionBuild.createSession(this, id, profile, provider, modelId, options);
  }

  /** Coding-run ceilings shared by parent sessions and their supervisor
   *  children (plan 122): Prism 0.7.0 — omit maxProviderAttempts (lifts to
   * maxTurns); pass null tokens so DEFAULT 40k/10k/50k cannot apply. Bytes
   *  are per-frame HARD (null rejected); 64 MiB is the ceiling on any single
   *  provider frame, never a run-lifetime sum (0.5.5 fixed cumulative
   *  charging). Children additionally AND-compose the supervisor's own
   *  delegation limits (steps/tools/tokens/timeout), which can only lower. */

  /** Plan 121 follow-up: the attention compiler's auto-compact pair. The
   *  trigger ORs the truncation streak, the compiler's compactRatio, and the
   *  absolute token ceiling; the strategy stays Prism's local deterministic
   *  default (the same auto-compact shape Prism's own CLI host uses): an
   *  unattended gate must never turn a provider outage into a failed run at
   *  prompt assembly. `run.setOptions.compaction` keeps governing explicit
   *  `session.compact` / `/compact` and per-run `compaction`. One
   *  `AttentionTruncationTrigger` per agent — the event loop feeds it every
   *  `attention_compiled` event. */

  /** Rebuild the live session's agent with a new provider/model config
   *  (plan 108 task 9): durable runs fingerprint on AgentConfig and reject
   *  per-run model overrides, so a picker switch recreates the session.
   *  The leaf carries over so the next append chains onto the same branch. */
  recreateSessionModel(
    live: LiveSession,
    sessionId: string,
    provider: string,
    modelId: string,
    agent?: AgentRootConfig,
  ): LiveSession {
    return hostSessionBuild.recreateSessionModel(this, live, sessionId, provider, modelId, agent);
  }

  resolveOmFlag(profile: string, raw: unknown): boolean {
    return hostSessionBuild.resolveOmFlag(this, profile, raw);
  }

  /** True when the profile's declared tool list includes coding tool names. */
  profileWantsCodingTools(profile: string): boolean {
    return hostSessionBuild.profileWantsCodingTools(this, profile);
  }



  resolveCompaction(name: string, live: LiveSession) {
    return hostSessionBuild.resolveCompaction(this, name, live);
  }

  /**
   * Resolve a profile's tools. Names in CODING_TOOL_NAMES build fresh
   * per-session coding tools bound to the session workspace + autonomy flag;
   * everything else resolves from the kernel registry (Chat profiles with no
   * tools stay no-tools).
   */

  /** The session's coding tool set over Clay document reverse-RPC (plan 122
   *  extraction): shared verbatim by supervisor children, which reuse the
   *  PARENT sessionId + workspaceRoot so document ops stay on the daemon→
   *  server registry — the server resolves the workspace from the parent
   *  session id and a child id would fail closed (agent_documents). */

  /** Plan 122: host-owned spawn/wait/cancel over one Prism supervisor whose
   *  catalog is exactly `test` and `validation` (the Phase 6 loop shapes —
   * this is the primitive, not the orchestrator). The supervisor is
   *  in-process and per live coding session; async handles die on daemon
   *  restart (Prism contract — documented, never faked durable). Children
   *  narrow from the host run identity (Prism narrowIdentity +
   *  assertIdentityPropagation — the closed spawn schema cannot widen
   *  tenant/scopes/expiry) and their results pass the parent redactor. */

  /** Plan 122 child factory: an isolated Prism agent (own history — the
   *  supervisor derives the `${delegationId}-session` id) on the parent's
   *  provider/model, with the parent's coding tools rebuilt against the
   *  PARENT sessionId + workspaceRoot. No spawn/wait/cancel → children
   *  cannot recurse. No git worktrees: a child `cwd` outside the registered
   *  session workspace would starve document RPC (plan 122 compromise —
   *  upgrade via createWorktreeChildFactory only once a child can own a
   *  registered workspace root). Identity is supplied by the supervisor
   *  (narrowed run identity), never here. */

  /** Resolve a profile's skill names against the kernel registry, validating
   *  each skill's required tools are active (fail-closed before any turn).
   *  Wiki skills append for coding sessions in the enabled workspace — their
   *  toolNames require the wiki tools, which are active exactly then. The
   *  graft skill appends on the same workspace binding. */
  resolveSkills(
    def: AgentDefinition,
    tools: readonly ToolDefinition[],
    options?: { workspaceRoot?: string; agent?: AgentRootConfig },
  ): readonly Skill[] | undefined {
    return hostSkills.resolveSkills(this, def, tools, options);
  }

  /** Config-gated, per-root skill discovery. Config-root + home roots scan
   *  `<root>/skills/<name>/SKILL.md` (decision 2026-09-09-1420); the
   *  workspace keeps npx skills `<root>/.agents/skills/`. Each root is
   *  scanned once (cached). Already-registered names win on collision
   *  (registry is duplicate:"error"): built-ins and earlier roots shadow
   *  later ones. Reserved agent-delivered names never come from disk —
   *  they activate only through their own paths. A failing root logs and
   *  yields nothing — discovery never fails a session. */
  async ensureSkillDiscovery(workspaceRoot: string): Promise<void> {
    return hostSkills.ensureSkillDiscovery(this, workspaceRoot);
  }


  /** Whether an agent-delivered skill is enabled by skills.json
   *  `agentSkills` (default true). Consumed by the wiki/graft activation
   *  paths (plan 117). */
  agentSkillEnabled(name: "wikiSearcher" | "wikiMaintainer" | "graft"): boolean {
    return hostSkills.agentSkillEnabled(this, name);
  }

  private async sessionList(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionList(this, params);
  }
  private async sessionLoad(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionLoad(this, params);
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
  async sessionContext(params: Record<string, unknown>): Promise<unknown> {
    return hostContext.sessionContext(this, params);
  }

  /** Active-context slice: from the latest compaction entry onward. */

  /**
   * Categorized active context. One category model feeds counts and items
   * (the item list is capped; `count` stays the real number so the client
   * can show "…and N more").
   */

  /** One context item's full redacted content for the drawer detail. */

  /** Plan 109 I8: validate an OM worker selection payload from session.new
   *  / session.om.set. Each worker is `{ provider, model }` (both
   *  non-empty strings) or `null` to clear; unknown shapes fail closed. */
  parseOmWorkers(raw: unknown): OmWorkerModels | undefined {
    return hostOm.parseOmWorkers(raw);
  }

  scanSkillsDir(skillsDir: string): Promise<readonly string[]> {
    return hostSkills.scanSkillsDir(this, skillsDir);
  }

  /** Plan 109 I8: set per-session OM worker models. Selections are
   *  validated against the provider registry (unknown provider fails
   *  closed), persisted into the session record metadata, and applied by
   *  re-attaching OM (worker rebuild keeps the leaf). `requireExplicitModel`
   *  stays true: cleared/unset workers skip — never the session model. */
  async sessionOmSet(params: Record<string, unknown>): Promise<unknown> {
    return hostOm.sessionOmSet(this, params);
  }

  /** Best-effort persistence of per-session OM worker models (same
   *  advisory pattern as persistProviderModel). */

  /** Plan 109 I8: the Observational Memory tab's read model — current
   *  worker selection plus the bounded observer activity log (recorded
   *  observations with fact summaries, reflections, drops, compaction
   *  folds) derived from the session's branch entries at completion
   *  boundaries (fetched on tab open / transcript change, never polled). */
  async sessionOmActivity(params: Record<string, unknown>): Promise<unknown> {
    return hostOm.sessionOmActivity(this, params);
  }

  /** One-line fact summary: redacted, bounded, first line only. */

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
    return hostSessions.sessionSetAgent(this, params);
  }
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
    return hostSessions.sessionResume(this, params);
  }
  async ensureLive(sessionId: string): Promise<LiveSession> {
    return hostSessions.ensureLive(this, sessionId);
  }
  private async sessionDelete(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionDelete(this, params);
  }
  private async sessionPrompt(params: Record<string, unknown>): Promise<unknown> {
    return hostSessionRun.sessionPrompt(this, params);
  }
  private async omRunScope(sessionId: string, live: LiveSession): Promise<WorkScopeSpec | undefined> {
    if (!live.observationalMemory || !hasCodingTools(live.tools)) return undefined;
    live.omRunCount ??= await this.countOpenedScopes(live, `run:${sessionId}:`);
    live.omRunCount += 1;
    return { id: `run:${sessionId}:${live.omRunCount}`, kind: "run" };
  }

  private async countOpenedScopes(live: LiveSession, prefix: string): Promise<number> {
    return (await live.session.entries()).reduce(
      (count, entry) =>
        isWorkScopeOpenedData(entry.data) && entry.data.id.startsWith(prefix) ? count + 1 : count,
      0,
    );
  }

  /** One work-scope controller per OM-attached live session (plan 123): the
   *  run scope and its spawn children share the instance. The cache keys on
   *  the session object because a model switch swaps in a fresh OM proxy. */
  private omScopes(live: LiveSession): WorkScopeController {
    return hostSessionRun.omScopes(this, live);
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
  omSpawnScopes(sessionId: string): SupervisorHooks {
    return hostSessionRun.omSpawnScopes(this, sessionId);
  }
  /** Durable + tool-interrupt for coding sessions only; Chat stays non-durable. */
  private durableRunState(live: LiveSession): AgentRunStateOptions | undefined {
    return hostSessionRun.durableRunState(this, live);
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
  resolveMentions(text: string, live: LiveSession): Promise<{ prompt: string; attachments: ContentBlock[] }> {
    return hostContext.resolveMentions(this, text, live);
  }

  /** Read one @file mention inside the workspace root. Returns undefined
   *  (token stays plain text) for paths that escape the root, are missing,
   *  unreadable, or — for images — oversized. Images attach as base64
   *  image content; other files as a bounded fenced text block. */

  /** Plan 117: bounded workspace file list for the @-mention dropdown —
   *  workspace-relative paths, dotfiles and build dirs skipped, capped in
   *  entries and depth. Absent/unreachable root ⇒ empty list (never a
   *  throw; the dropdown degrades to skills-only). */
  async workspaceListFiles(params: Record<string, unknown>): Promise<unknown> {
    return hostContext.workspaceListFiles(this, params);
  }

  private async sessionCancel(params: Record<string, unknown>): Promise<unknown> {
    return hostSessionRun.sessionCancel(this, params);
  }
  private sessionSteer(params: Record<string, unknown>): unknown {
    return hostSessionRun.sessionSteer(this, params);
  }
  private async sessionCompact(params: Record<string, unknown>): Promise<unknown> {
    return hostSessionRun.sessionCompact(this, params);
  }
  /** Toggle full autonomy for a live session (decision 2026-09-20-2049):
   *  default true; false re-arms approval prompts for gated tool calls.
   *  Host-set only — no agent-facing tool can change this. */
  private sessionSetAutonomy(params: Record<string, unknown>): unknown {
    return hostSessionRun.sessionSetAutonomy(this, params);
  }
  /** Workspace-scoped session search (decision 2201): always filters by this
   *  session's workspaceRoot; hits are transcript metadata, never auto-injected
   *  into agent context. Cross-workspace queries return empty, not an error. */
  private async sessionSearch(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionSearch(this, params);
  }
  /** Plan 109 I9: workspace-scoped resumable session list backing the
   *  /resume picker — one surface shared by the slash command and the
   *  panel affordance. Fail-closed: requires an explicit workspaceRoot
   *  (this surface never lists across workspaces); the root arrives from
   *  the server's tab registry, never from webview input. Most-recent
   *  first (store ordering), bounded, safe display fields only (same
   *  redaction posture as session.search). */
  private async sessionResumable(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionResumable(this, params);
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
  async sessionCheckout(params: Record<string, unknown>): Promise<unknown> {
    return hostSessionTree.sessionCheckout(this, params);
  }

  /** Append a summary entry (kind "summary") for the abandoned path from
   *  `fromLeaf` back to `branchPointId` (exclusive). Deterministic bounded
   *  preview; the LLM refine lands as a sibling entry later. */

  /** Background LLM refine (mock-provider testable): one-shot no-tool run
   *  over the abandoned path; the refined entry re-roots the leaf when the
   *  session has not moved on. Never throws into the checkout path. */

  private sessionFork(params: Record<string, unknown>): unknown {
    return hostSessions.sessionFork(this, params);
  }
  private async sessionClone(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionClone(this, params);
  }
  /** Capture a document checkpoint at `entryId` (task-boundary or explicit
   *  user checkpoint). The server snapshots every open document buffer. */
  private async sessionCheckpoint(params: Record<string, unknown>): Promise<unknown> {
    return hostSessions.sessionCheckpoint(this, params);
  }
  /** Resume a suspended durable run. Validates decision shape before any checkpoint I/O. */
  private async runResume(params: Record<string, unknown>): Promise<unknown> {
    return hostSessionRun.runResume(this, params);
  }
  providerList(): Promise<unknown> {
    return hostProviders.providerList(this);
  }


  providerStatus(params: Record<string, unknown>): Promise<unknown> {
    return hostProviders.providerStatus(this, params);
  }

  modelList(): Promise<unknown> {
    return hostProviders.modelList(this);
  }

  modelSearch(params: Record<string, unknown>): unknown {
    return hostProviders.modelSearch(this, params);
  }

  credentialPut(params: Record<string, unknown>): Promise<unknown> {
    return hostProviders.credentialPut(this, params);
  }


  defaultCredentialName(provider: string): string {
    return hostProviders.defaultCredentialName(this, provider);
  }

  oauthStart(params: Record<string, unknown>): Promise<unknown> {
    return hostProviders.oauthStart(this, params);
  }

  oauthPoll(params: Record<string, unknown>): Promise<unknown> {
    return hostProviders.oauthPoll(this, params);
  }

  credentialDelete(params: Record<string, unknown>): Promise<unknown> {
    return hostProviders.credentialDelete(this, params);
  }

  private profileList(): unknown {
    return hostCommands.profileList(this);
  }
  private profileRegister(params: Record<string, unknown>): unknown {
    return hostCommands.profileRegister(this, params);
  }
  /** Register inert skill data on the kernel registry (re-registering a name replaces the definition). */
  skillRegister(params: Record<string, unknown>): unknown {
    return hostSkills.skillRegister(this, params);
  }

  /** Progressive catalog: name + description only. Full instructions load via `load_skill`.
   *  Wiki skills appear only while the wiki option is bound (no residue when disabled). */
  skillList(): unknown {
    return hostSkills.skillList(this);
  }

  /** Catalog-visible skills (wiki/graft skills hide while their option is
   *  unbound). Bounded to the environment's wire caps. */
  visibleSkills(): Array<{ name: string; description: string }> {
    return hostSkills.visibleSkills(this);
  }

  /** Register a command definition (data + host-side handler name). Drivers are
   *  never accepted over RPC; the host injects them at dispatch time. */
  private commandRegister(params: Record<string, unknown>): unknown {
    return hostCommands.commandRegister(this, params);
  }
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
    return hostCommands.environmentList(this, params);
  }
  private async commandDispatch(params: Record<string, unknown>): Promise<unknown> {
    return hostCommands.commandDispatch(this, params);
  }
  /** Coding-run policy caps + default compact strategy. Partial update;
   *  `null` disables that Prism axis. compactAfterTokens is the absolute
   *  ceiling of the auto-compaction trigger armed at `createSession` (see
   *  `autoCompaction`); new sessions pick it up, live ones keep their gate. */
  private runSetOptions(params: Record<string, unknown>): unknown {
    return hostCommands.runSetOptions(this, params);
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
  async knowledgeSetOptions(params: Record<string, unknown>): Promise<unknown> {
    return hostIntegrations.knowledgeSetOptions(this, params);
  }

  /** Resolve the explicit graft deep model (plan 121 follow-up): the model
   *  identity arrives over the trusted RPC, the API key does not have to — an
   *  omitted `apiKey` reads the provider credential the agent picker already
   *  stores, so `init.js` never carries the secret. Inline keys are accepted
   *  for providers with no stored credential (litellm/orcarouter) and join the
   *  redactor set. Unknown or incomplete shapes fail closed (-32602). */


  async enableWiki(workspaceRoot: string, qmdPath?: string): Promise<void> {
    return hostIntegrations.enableWiki(this, workspaceRoot, qmdPath);
  }

  /** Wiki knowledge tools for the bound workspace; empty when the option is
 *  disabled or the session's workspace is not the bound one. */
  wikiTools(workspaceRoot: string): ToolDefinition[] {
    return hostIntegrations.wikiTools(this, workspaceRoot);
  }

  /** Graft knowledge tools for the bound workspace (pull/both modes only —
   *  push-only registers no tools). Empty when disabled, unbound, or when
   *  the CLI resolved without registering a given tool. */
  graftTools(workspaceRoot: string): ToolDefinition[] {
    return hostIntegrations.graftTools(this, workspaceRoot);
  }

  /** Opt-in graft knowledge base (plan 108 task 13). Disabled by default.
   *  Enabling resolves the graft CLI fail-closed BEFORE load: an absent CLI
   *  (no cliPath, no host packageRoot, no @nanonets/graft peer) leaves the
   *  option off and the agent unperturbed — tools hidden, never a half
   *  binding. One binding per daemon; re-enabling rebinds. A changed
   *  `deepModel` also rebinds (the extension resolves its child env once at
   *  load), so the explicit `/graft-build-deep` opt-in can be added after a
   *  default-on bind without a restart. */


  /** Graft default-on trigger (plan 117): first coding session for a
   *  workspace attempts the pull-mode binding, once per root per daemon —
   *  no retry loops. Fail-closed: unresolvable CLI ⇒ off, tools hidden,
   *  agent unperturbed. knowledge.setOptions stays the explicit
   *  disable/mode-override surface and bypasses the attempt cache. */
  async ensureGraftBound(workspaceRoot: string): Promise<boolean> {
    return hostIntegrations.ensureGraftBound(this, workspaceRoot);
  }

  /** Host-verified run identity. The daemon is the trust boundary: it owns
   *  the workspace root and the acceptance policy, so it vouches for its own
   *  runs. Never sourced from RPC params. */
  runIdentity(): AgentIdentity {
    return {
      tenantId: TENANT,
      // Plan 122: supervisor ownership requires an account/user anchor;
      // the service principal's id doubles as the userId it vouches for.
      userId: "clay-agent",
      principal: { kind: "service", id: "clay-agent" },
      scopes: ["workspace"],
      issuedAt: new Date().toISOString(),
      verified: true,
    };
  }

  /** Ownership projected from the run identity — the same scope the run's
   *  durable effects and checkpoints are recorded under. */
  runOwnership(): ReturnType<typeof ownershipFromIdentity> {
    return ownershipFromIdentity(this.runIdentity());
  }

  /** Host driver capabilities for commands. Never sourced from RPC params. */
  private commandDrivers(live: LiveSession | undefined): CommandDrivers | undefined {
    return hostCommands.commandDrivers(this, live);
  }
  /** Branch summary for /tree (plan 108 task 10): every entry's id/parent/kind
   *  plus a short redacted preview, the branch list, stored branch summaries,
   *  checkpoint flags (server authority), and the in-flight summary markers.
   *  Bounded by the store's own entry list. */
  async sessionTreeSummary(sessionId: string): Promise<unknown> {
    return hostSessionTree.sessionTreeSummary(this, sessionId);
  }

  /** Render the /tree payload as bounded transcript text (pi-parity in-place
   *  view: entries with ids, leaf marker, checkpoints, branch summaries). */
  async renderTreeText(sessionId: string): Promise<string> {
    return hostSessionTree.renderTreeText(this, sessionId);
  }

}
