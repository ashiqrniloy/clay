/**
 * Provider/model/credential surfaces for ClayAgentHost (plan 130 task 2):
 * provider listing + configured checks, model catalog/search (with the
 * one-shot Ollama discovery), vault/keychain credential put/delete, and
 * the device-code OAuth start/poll pair.
 */
import { randomUUID } from "node:crypto";
import { thinkingLevelsForModel, type AuthMethod, type JsonObject, type OAuthAuthMethod, type OAuthCredentials } from "@arnilo/prism";
import type { ClayAgentHost } from "../host.js";
import { MAX_LIST, optString, reqString, rpcError } from "./internals.js";

export interface PendingOauth {
  readonly provider: string;
  promise: Promise<OAuthCredentials>;
  readonly done: Promise<void>;
  info?: { readonly userCode?: string; readonly verificationUri?: string; readonly authorizationUrl?: string };
}

/** Device-code request is one POST. Stay under agent RPC_TIMEOUT (30s). */
const OAUTH_START_TIMEOUT_MS = 20_000;

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

export async function providerList(host: ClayAgentHost): Promise<unknown> {
  const auth = host.kernel.registries.authMethods.list();
  const providers = host.kernel.registries.providers.list().map((provider) => ({
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
      configured: await providerConfigured(host, provider.id, provider.auth),
    });
  }
  return { providers: out };
}

async function providerConfigured(
  host: ClayAgentHost,
  id: string,
  methods: Array<{ kind: string; credentialName?: string }>,
): Promise<boolean> {
  for (const method of methods) {
    if (method.kind === "api_key" || method.kind === "url") {
      const name = method.credentialName ?? (method.kind === "url" ? "baseUrl" : "apiKey");
      if (await host.vault.get({ name, provider: id })) return true;
    } else if (method.kind === "oauth" && (await host.vault.getOAuth(id))) {
      return true;
    }
  }
  return false;
}

export async function providerStatus(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const id = reqString(params, "id");
  const methods = host.kernel.registries.authMethods.list().filter((method) => method.provider === id);
  let configured = false;
  for (const method of methods) {
    if (method.kind === "api_key") {
      const name = "credentialName" in method ? (method.credentialName ?? "apiKey") : "apiKey";
      configured = Boolean(await host.vault.get({ name, provider: id }));
    } else if (method.kind === "oauth") {
      configured = Boolean(await host.vault.getOAuth(id));
    }
    if (configured) break;
  }
  return { id, configured, present: Boolean(host.kernel.registries.providers.get(id) || methods.length) };
}

export async function modelList(host: ClayAgentHost): Promise<unknown> {
  if (!host.ollamaDiscoveryAttempted && host.kernel.registries.providers.get("ollama")) {
    await refreshOllamaModels(host);
  }
  return {
    models: host.kernel.registries.models.list().map((model) => ({
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

export function modelSearch(host: ClayAgentHost, params: Record<string, unknown>): unknown {
  const query = reqString(params, "query").toLowerCase();
  const models = host.kernel.registries.models.list()
    .filter((model) =>
      [model.provider, model.model, model.displayName ?? ""].some((part) => part.toLowerCase().includes(query)),
    )
    .slice(0, MAX_LIST)
    .map((model) => ({ provider: model.provider, model: model.model, displayName: model.displayName }));
  return { models };
}

export async function credentialPut(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const provider = reqString(params, "provider");
  const secret = reqString(params, "secret");
  const name = optString(params, "name") ?? defaultCredentialName(host, provider);
  const type = optString(params, "type") === "bearer" ? "bearer" : "api_key";
  await host.vault.set({ name, provider, credential: { type, value: secret } });
  host.rememberSecret(secret);
  if (host.keychain) {
    try {
      await host.keychain.set({ name, provider, credential: { type, value: secret } });
    } catch {
      // Encrypted vault already persisted; keychain is best-effort.
    }
  }
  if (provider === "ollama") {
    host.ollamaDiscoveryAttempted = false;
    await refreshOllamaModels(host);
  }
  return { provider, name, stored: true };
}

async function refreshOllamaModels(host: ClayAgentHost): Promise<void> {
  if (host.ollamaDiscoveryAttempted) return;
  if (!host.kernel.registries.providers.get("ollama")) {
    host.ollamaDiscoveryAttempted = true;
    return;
  }
  const apiKey = await host.vault.get({ name: "apiKey", provider: "ollama" });
  const base = await host.vault.get({ name: "baseUrl", provider: "ollama" });
  if (!apiKey && !base) return;
  host.ollamaDiscoveryAttempted = true;
  try {
    const { listOllamaModels } = await import("@arnilo/prism-providers/ollama");
    const models = await listOllamaModels({
      apiKey: apiKey?.value,
      baseUrl: base?.value,
      signal: AbortSignal.timeout(3000),
    });
    for (const model of models) {
      if (host.kernel.registries.models.get(model.provider, model.model) === undefined) {
        host.kernel.registries.models.register(model);
      }
    }
  } catch {
    // Best-effort: local daemon down or cloud unreachable.
  }
}

export function defaultCredentialName(host: ClayAgentHost, provider: string): string {
  const method = host.kernel.registries.authMethods.list().find((item) => item.provider === provider && item.kind === "api_key");
  return method && "credentialName" in method ? (method.credentialName ?? "apiKey") : "apiKey";
}

export async function oauthStart(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const provider = reqString(params, "provider");
  const method = host.kernel.registries.authMethods.list().find((item) => item.provider === provider && isOauth(item));
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
  host.oauth.set(loginId, pending);
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
  host.oauth.delete(loginId);
  abort.abort();
  if (loginError) {
    const message = loginError instanceof Error ? loginError.message : String(loginError);
    throw rpcError(-32000, message);
  }
  throw rpcError(-32000, `OAuth start produced no device code or authorization URL for ${provider}`);
}

export async function oauthPoll(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const loginId = reqString(params, "loginId");
  const pending = host.oauth.get(loginId);
  if (!pending) throw rpcError(-32000, `Unknown OAuth login: ${loginId}`);
  const raced = await Promise.race([
    pending.promise.then((credentials) => ({ credentials })),
    new Promise<{ pending: true }>((resolve) => setTimeout(() => resolve({ pending: true }), 25)),
  ]);
  if ("pending" in raced) return { loginId, status: "pending", ...(pending.info ?? {}) };
  await host.vault.setOAuth(pending.provider, raced.credentials);
  if (raced.credentials.access) host.rememberSecret(raced.credentials.access);
  if (raced.credentials.refresh) host.rememberSecret(raced.credentials.refresh);
  host.oauth.delete(loginId);
  return { loginId, status: "complete", provider: pending.provider, accountId: raced.credentials.accountId };
}

export async function credentialDelete(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const provider = reqString(params, "provider");
  const name = optString(params, "name") ?? defaultCredentialName(host, provider);
  const deleted = await host.vault.delete({ name, provider });
  await host.vault.deleteOAuth(provider);
  if (host.keychain) {
    try {
      await host.keychain.delete({ name, provider });
      await host.keychain.deleteOAuth(provider);
    } catch {
      // ignore
    }
  }
  return { provider, name, deleted };
}
