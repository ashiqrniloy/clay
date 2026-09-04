import { mkdir } from "node:fs/promises";
import { join } from "node:path";
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
  listSessionBranches,
  SESSION_SEARCH_WORKSPACE_METADATA_KEY,
  providerDone,
  providerTextDelta,
  redactAgentEvent,
  resolveActiveSkills,
  resumeAgentRun,
} from "@arnilo/prism";
import {
  createKeychainCredentialStore,
  createStoredCredentialResolver,
  openEncryptedCredentialStore,
  type EncryptedCredentialStore,
  type KeychainCredentialStore,
} from "@arnilo/prism-core/credentials/node";
import { createSqlitePersistence, type SqlitePersistence } from "@arnilo/prism-core/sessions/sqlite";
import { createJsonSchemaToolArgumentValidator } from "@arnilo/prism-core/validation/json-schema";
import type { AskUserDecisionAnswer } from "@arnilo/prism-coding-tools/agent";
import {
  createObservationalMemory,
  createRecallMemoryTool,
} from "@arnilo/prism-memory/compaction/observational-memory";
import type { SettingsProvider } from "@arnilo/prism";
import type { AgentIdentity } from "@arnilo/prism";
import { ownershipFromIdentity } from "@arnilo/prism";
import { CODING_TOOL_NAMES, buildCodingTools } from "./coding-tools.js";
import { connectAllowListedMcpServers, MAX_MCP_SERVERS, type ConnectedMcpServers } from "./mcp.js";
import { resolveObscuraBinary, spawnObscuraHarness, type ObscuraHarness } from "./obscura.js";
import {
  DEFAULT_COMPACT_AFTER_TOKENS,
  createNamedCompactionStrategy,
  isCompactionStrategyName,
  registerCompactionStrategies,
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

/** Per-session OM auto-compaction threshold override (2158 default 80000).
 *  Only meaningful for OM-attached sessions. */
const omCompactAfterTokens = new Map<string, number>();

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
const REVERSE_TIMEOUT_MS = 30_000;
const TENANT = "clay";
const KEYCHAIN_SERVICE = "clay-agent";
/** Host-authored durable-run revision (must match across run + resume). */
const RUN_STATE_REVISION = "clay-agent.1";
/** Bounded branch-summary text (plan 108 task 10) and in-place tree render. */
const MAX_BRANCH_SUMMARY_BYTES = 2_048;
const MAX_TREE_RENDER_BYTES = 8_192;
const MAX_TREE_RENDER_ENTRIES = 60;

export type EmitFn = (method: string, params: unknown) => void;

export interface HostOptions {
  readonly dataDir: string;
  readonly passphrase: string;
  readonly mock?: boolean;
  readonly mockProvider?: AIProvider;
  readonly emit?: EmitFn;
  /**
   * Server-built MCP allow-list (decision 1758). Absent/empty connects
   * nothing. Entries are validated fail-closed (canonical executable,
   * literal argv, explicit env names).
   */
  readonly mcpAllowList?: readonly unknown[];
  /** Test seam: replace Obscura binary resolution. */
  readonly resolveObscuraBinary?: () => string | undefined;
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
    };
    readonly compactAfterTokens?: number;
  };
}

interface LiveSession {
  session: AgentSession;
  agent: Agent;
  readonly profile: string;
  /** Mutable: provider/model pickers switch mid-session (plan 108 task 9). */
  provider: string;
  model: string;
  readonly workspaceRoot: string;
  /** Host-settable: toggled via session.setAutonomy (decision 2157). */
  fullAutonomy: boolean;
  readonly observationalMemory: boolean;
  /** Live tools for this session (empty for Chat without OM). */
  readonly tools: ToolDefinition[];
}

/** In-flight LLM branch-summary refinements: sessionId → branch point id. */

/** Persist the live book's provider/model onto the session record (plan 108
 *  task 9): a picker switch mid-session survives daemon restart and resume.
 *  Best-effort — a store without appendSession skips persistence. */
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
  /** Connected MCP bridges (empty allow-list = never populated). */
  private mcp: ConnectedMcpServers | undefined;
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
  /** Session of the run currently in flight — graft push state (patches,
   *  orientation freshness) persists against it. Ceiling: one binding; the
   *  extension API carries no session key on `getEntries`. Upgrade path:
   *  thread sessionId through the prism extension options when it grows one. */
  private activeGraftSessionId: string | undefined;

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
  ) {
    this.redactor = createSecretRedactor([]);
    this.resolveObscura = resolveObscura;
  }

  static async create(options: HostOptions): Promise<ClayAgentHost> {
    await mkdir(options.dataDir, { recursive: true, mode: 0o700 });
    const vaultPath = join(options.dataDir, "credentials.vault");
    const vault = await openEncryptedCredentialStore({
      path: vaultPath,
      getPassphrase: () => options.passphrase,
    });
    let keychain: KeychainCredentialStore | undefined;
    try {
      const candidate = createKeychainCredentialStore({ service: KEYCHAIN_SERVICE });
      await candidate.list();
      keychain = candidate;
    } catch {
      keychain = undefined;
    }
    const resolver = createExplicitCredentialResolver([
      { name: "vault", resolver: createStoredCredentialResolver(vault) },
      ...(keychain ? [{ name: "keychain" as const, resolver: createStoredCredentialResolver(keychain) }] : []),
    ]);
    const persistence = createSqlitePersistence({
      filename: join(options.dataDir, "sessions.sqlite"),
      fileMode: 0o600,
    });
    const kernel = createExtensionKernel({ errorPolicy: "throw" });
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
    );
    const mockProvider = kernel.registries.providers.get("mock");
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
  private ensureCapabilities(): Promise<void> {
    if (!this.capabilitiesPromise) {
      this.capabilitiesPromise = (async () => {
        if (this.mcpAllowList.length > 0 && !this.mcp) {
          try {
            this.mcp = await connectAllowListedMcpServers(this.mcpAllowList);
          } catch (error) {
            if ((error as { rpcCode?: number }).rpcCode === -32602) throw error;
            this.mcp = undefined;
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
    }
    return this.capabilitiesPromise;
  }

  /** Capability tools for a coding session; undefined shapes stay allowed. */
  private capabilityTools(): ToolDefinition[] {
    return [...(this.mcp?.tools ?? []), ...(this.obscura?.tools ?? [])];
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
      case "knowledge.setOptions":
        return this.knowledgeSetOptions(asRecord(params));
      default:
        throw rpcError(-32601, `unknown method: ${method}`);
    }
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
    const fullAutonomy = params.fullAutonomy === true;
    const observationalMemory = this.resolveOmFlag(profile, params.observationalMemory);
    // Capabilities activate on the first session that declares coding tools
    // (never on import, never for Chat-only initialize).
    const wantsCoding = this.profileWantsCodingTools(profile);
    if (wantsCoding) await this.ensureCapabilities();
    const created = this.createSession(id, profile, provider, modelId, {
      workspaceRoot,
      fullAutonomy,
      observationalMemory,
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
        // Stable workspace identity for workspace-scoped search (decision 2201).
        [SESSION_SEARCH_WORKSPACE_METADATA_KEY]: workspaceRoot,
      },
    });
    this.live.set(id, {
      session: created.session,
      agent: created.agent,
      profile,
      provider,
      model: modelId,
      workspaceRoot,
      fullAutonomy,
      observationalMemory,
      tools: created.tools,
    });
    return {
      sessionId: id,
      profile,
      provider,
      model: modelId,
      workspaceRoot,
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
    options: { workspaceRoot: string; fullAutonomy: boolean; observationalMemory: boolean },
  ): { session: AgentSession; agent: Agent; tools: ToolDefinition[] } {
    const def = this.kernel.registries.agents.resolve(profile);
    if (!this.kernel.registries.providers.get(provider)) throw rpcError(-32000, `Unknown provider: ${provider}`);
    const tools = this.sessionTools(def, id, options);
    const skills = this.resolveSkills(def, tools, options);
    // Progressive disclosure: with active skills, host the `load_skill` tool so
    // the model can pull full instructions on demand (catalog-only by default).
    const runTools = skills && skills.length > 0 ? [...tools, createLoadSkillTool({ registry: this.skills, tools })] : tools;
    const model: ModelConfig = this.kernel.registries.models.get(provider, modelId) ?? { provider, model: modelId };
    const agent = createAgent({
      id: profile,
      model,
      providerSource: createProviderResolver(this.kernel.registries.providers),
      store: this.persistence,
      runLedger: this.persistence,
      redactor: this.redactor,
      validator: createJsonSchemaToolArgumentValidator(),
      // Host-verified identity default for every session and resumed run: the
      // daemon is the trust boundary that owns the workspace root and
      // acceptance policy, so it vouches for its own runs. Durable tool
      // effects (write/edit/delete/move) require it — without it every
      // mediated write fails closed with ERR_PRISM_TOOL_EFFECT_CONFLICT,
      // including post-resume turns where run options don't reach the context.
      identity: this.runIdentity(),
      ...(def.instructions !== undefined ? { instructions: def.instructions } : {}),
      ...(def.systemPrompt !== undefined ? { systemPrompt: def.systemPrompt } : {}),
      ...(runTools.length > 0 ? { tools: runTools } : {}),
      ...(skills !== undefined ? { skills } : {}),
    });
    let session = agent.createSession({ id });
    if (options.observationalMemory) session = this.attachOm(session, model);
    return { session, agent, tools: runTools };
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
  ): LiveSession {
    const leafId = live.session.leafId;
    const recreated = this.createSession(sessionId, live.profile, provider, modelId, {
      workspaceRoot: live.workspaceRoot,
      fullAutonomy: live.fullAutonomy,
      observationalMemory: live.observationalMemory,
    });
    const session = recreated.agent.createSession({ id: sessionId, ...(leafId ? { leafId } : {}) });
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
    options: { workspaceRoot: string; fullAutonomy: boolean; observationalMemory: boolean },
  ): ToolDefinition[] {
    const built = this.buildSessionTools(def, sessionId, options) ?? [];
    // Capabilities ride along with coding sessions (they were activated for
    // the coding profile; Chat profiles with no tools stay no-tools).
    const capabilities = built.length > 0 ? this.capabilityTools() : [];
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

  private attachOm(session: AgentSession, model: ModelConfig): AgentSession {
    const observation = this.omConfig?.observation;
    const reflection = this.omConfig?.reflection;
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
    options?: { workspaceRoot?: string },
  ): readonly Skill[] | undefined {
    const names = def.skills ? [...def.skills] : [];
    if (options?.workspaceRoot !== undefined && this.wiki?.workspaceRoot === options.workspaceRoot) {
      names.push(wikiSearcherSkill.name, wikiMaintainerSkill.name);
    }
    if (options?.workspaceRoot !== undefined && this.graft?.workspaceRoot === options.workspaceRoot) {
      names.push(graftSkill.name);
    }
    if (names.length === 0) return undefined;
    return resolveActiveSkills({ registry: this.skills, names, tools });
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

  private async sessionResume(params: Record<string, unknown>): Promise<unknown> {
    const sessionId = reqString(params, "sessionId");
    const live = await this.ensureLive(sessionId);
    return { sessionId, profile: live.profile, provider: live.provider, model: live.model, leafId: live.session.leafId };
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
    const created = this.createSession(sessionId, profile, provider, model, {
      workspaceRoot: process.cwd(),
      fullAutonomy: false,
      observationalMemory,
    });
    const live: LiveSession = {
      session: created.session,
      agent: created.agent,
      profile,
      provider,
      model,
      workspaceRoot: process.cwd(),
      fullAutonomy: false,
      observationalMemory,
      tools: created.tools,
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
      if (this.kernel.registries.models.get(provider, model) === undefined && overModel !== undefined) {
        throw rpcError(-32000, `Unknown model: ${provider}/${model}`);
      }
      if (provider !== live.provider || model !== live.model) {
        live = this.recreateSessionModel(live, sessionId, provider, model);
        await persistProviderModel(this.persistence, sessionId, provider, model);
      }
    }
    const compactionName = optString(params, "compaction");
    const compaction = compactionName
      ? { strategy: this.resolveCompaction(compactionName, live), secrets: [...this.secrets] }
      : undefined;
    const runState = this.durableRunState(live);
    // stream() (not subscribe()+run()) — durable runs keep the subscription
    // open past settlement, so only stream() both drains and resolves.
    const stream = live.session.stream(text, {
      maxQueuedEvents: MAX_QUEUED_EVENTS,
      overflow: "drop_oldest",
      // Host-verified identity: the daemon is the trust boundary that owns the
      // workspace root and acceptance policy, so it vouches for its own runs.
      // Durable tool effects (write/edit/delete/move) require this — without
      // it every mediated write fails closed with ERR_PRISM_TOOL_EFFECT_CONFLICT.
      identity: this.runIdentity(),
      ...(compaction ? { compaction } : {}),
      ...(runState ? { runState } : {}),
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
        ? {
            status: "suspended" as const,
            runId: suspended.runId,
            version: suspended.version,
            interruption: suspended.interruption,
          }
        : { status: lastType === "agent_finished" ? ("succeeded" as const) : ("aborted" as const) }),
    };
  }

  /** Durable + tool-interrupt for coding sessions only; Chat stays non-durable. */
  private durableRunState(live: LiveSession): AgentRunStateOptions | undefined {
    const coding = live.tools.some((tool) => (CODING_TOOL_NAMES as readonly string[]).includes(tool.name));
    if (!coding) return undefined;
    return {
      checkpoints: this.persistence.checkpoints,
      definitionRevision: RUN_STATE_REVISION,
      interruptBeforeTool: true,
    };
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
    const strategyName = optString(params, "strategy");
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
        ...(strategyName ? { strategy: this.resolveCompaction(strategyName, live) } : {}),
        secrets: [...this.secrets],
      });
      const entry = result.entries?.[0];
      return {
        sessionId,
        summary: result.summary,
        entryId: entry?.id,
        strategy: strategyName ?? (live.observationalMemory ? "om" : "default"),
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
    const expectedVersion = params.expectedVersion;
    if (typeof expectedVersion !== "number" || !Number.isSafeInteger(expectedVersion) || expectedVersion <= 0) {
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
    const resume: AgentRunResume = { expectedVersion, ...(decisions ? { decisions } : { decision }) };
    try {
      const result = await resumeAgentRun(live.agent, { runId, sessionId }, resume, {
        checkpoints: this.persistence.checkpoints,
        definitionRevision: RUN_STATE_REVISION,
        ownership: this.runOwnership(),
      });
      const suspended = result.status === "suspended" && result.runState !== undefined;
      return {
        sessionId,
        runId,
        ...(suspended && result.runState
          ? {
              status: "suspended",
              version: result.runState.version,
              interruption: result.runState.interruption,
            }
          : { status: result.status }),
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

  private modelList(): unknown {
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
    return { provider, name, stored: true };
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
    const pending: PendingOauth = { provider, done, promise: Promise.resolve({} as OAuthCredentials) };
    pending.promise = Promise.resolve(
      method.oauth.login({
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
    void pending.promise.then(settle, settle);
    this.oauth.set(loginId, pending);
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 50));
    await Promise.race([pending.done, timeout]);
    return {
      loginId,
      provider,
      status: "started",
      ...(pending.info ?? {}),
    };
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
      skills: this.skills
        .list()
        .filter(
          (skill) =>
            (this.wiki !== undefined || !skill.name.startsWith("wiki-")) &&
            (this.graft !== undefined || skill.name !== graftSkill.name),
        )
        .map((skill) => ({
        name: skill.name,
        ...(skill.description !== undefined ? { description: skill.description } : {}),
        ...(skill.toolNames !== undefined ? { toolNames: skill.toolNames } : {}),
      })),
    };
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
    for (const skill of [wikiSearcherSkill, wikiMaintainerSkill]) {
      if (!this.skills.get(skill.name)) this.skills.register(skill);
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
    if (this.graft?.workspaceRoot === workspaceRoot) return true;
    const previous = this.graft;
    this.graft = undefined;
    previous?.loaded.dispose();
    try {
      resolveGraftCli(options.cliPath ? { cliPath: options.cliPath } : {});
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
    if (!this.skills.get(graftSkill.name)) this.skills.register(graftSkill);
    return true;
  }

  private disableGraft(): void {
    const current = this.graft;
    if (!current) return;
    this.graft = undefined;
    this.activeGraftSessionId = undefined;
    current.loaded.dispose();
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

