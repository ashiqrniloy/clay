import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import {
  type AgentDefinition,
  type AgentEvent,
  type AgentSession,
  type AIProvider,
  type AuthMethod,
  type ExtensionKernel,
  type ModelConfig,
  type OAuthAuthMethod,
  type OAuthCredentials,
  type SecretRedactor,
  type ToolDefinition,
  createAgent,
  createExplicitCredentialResolver,
  createExtensionKernel,
  createMockProvider,
  createProviderResolver,
  createSecretRedactor,
  providerDone,
  providerTextDelta,
  redactAgentEvent,
} from "@arnilo/prism";
import {
  createKeychainCredentialStore,
  createStoredCredentialResolver,
  openEncryptedCredentialStore,
  type EncryptedCredentialStore,
  type KeychainCredentialStore,
} from "@arnilo/prism-credentials-node";
import { createSqlitePersistence, type SqlitePersistence } from "@arnilo/prism-session-store-sqlite";
import { createJsonSchemaToolArgumentValidator } from "@arnilo/prism-tool-validator-json-schema";
import { redactText } from "./redact.js";

export const MAX_QUEUED_EVENTS = 256;
const MAX_LIST = 50;
const MAX_LOAD_ENTRIES = 200;
const TENANT = "clay";
const KEYCHAIN_SERVICE = "clay-agent";

export type EmitFn = (method: string, params: unknown) => void;

export interface HostOptions {
  readonly dataDir: string;
  readonly passphrase: string;
  readonly mock?: boolean;
  readonly mockProvider?: AIProvider;
  readonly emit?: EmitFn;
}

interface LiveSession {
  readonly session: AgentSession;
  readonly profile: string;
  readonly provider: string;
  readonly model: string;
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
  private readonly oauth = new Map<string, PendingOauth>();
  private closed = false;

  private constructor(
    readonly dataDir: string,
    private readonly persistence: SqlitePersistence,
    private readonly vault: EncryptedCredentialStore,
    private readonly keychain: KeychainCredentialStore | undefined,
    private readonly kernel: ExtensionKernel,
    private readonly emit: EmitFn,
  ) {
    this.redactor = createSecretRedactor([]);
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
    );
    host.secrets.add(options.passphrase);
    host.refreshRedactor();
    return host;
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    for (const live of this.live.values()) live.session.abort("shutdown");
    this.live.clear();
    this.persistence.close();
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
      default:
        throw rpcError(-32601, `unknown method: ${method}`);
    }
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
    const session = this.createSession(id, profile, provider, modelId);
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
      metadata: { profile, provider, model: modelId },
    });
    this.live.set(id, { session, profile, provider, model: modelId });
    return { sessionId: id, profile, provider, model: modelId };
  }

  private createSession(id: string, profile: string, provider: string, modelId: string): AgentSession {
    const def = this.kernel.registries.agents.resolve(profile);
    if (!this.kernel.registries.providers.get(provider)) throw rpcError(-32000, `Unknown provider: ${provider}`);
    const tools = this.resolveTools(def);
    const skills = this.resolveSkills(def);
    const model: ModelConfig = this.kernel.registries.models.get(provider, modelId) ?? { provider, model: modelId };
    const agent = createAgent({
      model,
      providerSource: createProviderResolver(this.kernel.registries.providers),
      store: this.persistence,
      runLedger: this.persistence,
      redactor: this.redactor,
      validator: createJsonSchemaToolArgumentValidator(),
      ...(def.instructions !== undefined ? { instructions: def.instructions } : {}),
      ...(def.systemPrompt !== undefined ? { systemPrompt: def.systemPrompt } : {}),
      ...(tools !== undefined ? { tools } : {}),
      ...(skills !== undefined ? { skills } : {}),
    });
    return agent.createSession({ id });
  }

  private resolveTools(def: AgentDefinition): ToolDefinition[] | undefined {
    if (!def.tools) return undefined;
    return def.tools.map((name) => this.kernel.registries.tools.resolve(name));
  }

  private resolveSkills(def: AgentDefinition) {
    if (!def.skills) return undefined;
    return def.skills.map((name) => this.kernel.registries.skills.resolve(name));
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
    const page = await this.persistence.querySessions({ id: sessionId, tenantId: TENANT, limit: 1 });
    const record = page.items[0];
    if (!record) throw rpcError(-32000, `Unknown session: ${sessionId}`);
    const entries = await this.persistence.list(sessionId);
    return {
      sessionId,
      profile: record.agentDefinitionId,
      metadata: record.metadata,
      entries: entries.slice(-MAX_LOAD_ENTRIES).map((entry) => this.redactor.redact(entry)),
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
    const session = this.createSession(sessionId, profile, provider, model);
    const live = { session, profile, provider, model };
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
    const live = await this.ensureLive(sessionId);
    let lastType: string | undefined;
    try {
      for await (const event of live.session.stream(text, {
        maxQueuedEvents: MAX_QUEUED_EVENTS,
        overflow: "drop_oldest",
      })) {
        lastType = event.type;
        this.emit("event", { sessionId, event: redactAgentEvent(event, this.redactor) satisfies AgentEvent });
      }
    } catch (error) {
      throw rpcError(-32000, error instanceof Error ? error.message : String(error));
    }
    return { sessionId, lastEvent: lastType };
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
    return { name, registered: true };
  }
}

