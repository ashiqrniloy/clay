/**
 * Wiki/graft knowledge-base enablement for ClayAgentHost (plan 130 task 2):
 * the opt-in wiki and graft extension bindings (one workspace each,
 * fail-closed resolution, zero residue when disabled), their tool sets,
 * and the graft default-on bind.
 */
import { createHash } from "node:crypto";
import type { LoadedExtension, ToolDefinition } from "@arnilo/prism";
import {
  WIKI_INGEST_TOOL_NAME,
  WIKI_READ_PAGE_TOOL_NAME,
  WIKI_RECORD_INSIGHT_TOOL_NAME,
  WIKI_SEARCH_TOOL_NAME,
  createWikiExtension,
  wikiSearcherSkill,
  wikiMaintainerSkill,
  type WikiExtensionOptions,
} from "@arnilo/prism-memory/wiki";
import {
  createGraftExtension,
  resolveGraftCli,
  type GraftDeepModel,
  type GraftExtensionOptions,
  type GraftMode,
} from "@arnilo/prism-memory/graft";
import { runObscuraCli, validateObscuraWebUrl } from "@arnilo/prism-web-tools/obscura";
import type { ClayAgentHost } from "../host.js";
import { optString, reqString, rpcError } from "./internals.js";
import { graftSkill } from "./skills.js";

/** Graft's own `build --deep` provider ids (the package's `GraftDeepProvider`,
 *  not Prism's provider registry). Kept as a literal list so the host
 *  validates before the extension does. */
const GRAFT_DEEP_PROVIDERS = ["openai", "anthropic", "litellm", "orcarouter"] as const;

/** In-memory identity of a bound deep model, used only to decide whether a
 *  repeat `knowledge.setOptions` needs to rebind. The key contributes as a
 *  digest, so this string can never leak it into a log or dump. */
function deepModelIdentity(deep: GraftDeepModel | undefined): string {
  if (!deep) return "";
  const key = createHash("sha256").update(deep.apiKey).digest("hex").slice(0, 16);
  return `${deep.provider}\u0000${deep.model}\u0000${deep.baseUrl ?? ""}\u0000${key}`;
}

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

/** Opt-in wiki knowledge base (decision 2156, plan 108 task 12). Disabled
 *  by default: nothing loads, no commands/tools/skills registered, no
 *  residue. Enabling loads `@arnilo/prism-memory/wiki` via `kernel.load`,
 *  which registers the /wiki-init, /wiki-refresh, /wiki-lint commands,
 *  the wiki_search/wiki_read_page/wiki_record_insight tools, and the
 *  wiki-searcher/wiki-maintainer skills (progressive disclosure via
 *  load_skill). Wiki writes stay inside the workspace .wiki/ tree: the
 *  extension's skill-file deployment is disabled, skills come from the
 *  registry instead. */
export async function knowledgeSetOptions(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const workspaceRoot = reqString(params, "workspaceRoot");
  const wiki = typeof params.wiki === "boolean" ? params.wiki : undefined;
  const graft = typeof params.graft === "boolean" ? params.graft : undefined;
  const rawDeepModel = params.graftDeepModel;
  if (rawDeepModel !== undefined && graft !== true) {
    throw rpcError(
      -32602,
      "knowledge.setOptions `graftDeepModel` requires `graft: true` (it configures the graft extension)",
    );
  }
  if (wiki === undefined && graft === undefined) {
    throw rpcError(-32602, "knowledge.setOptions requires a boolean `wiki` and/or `graft` flag");
  }
  const graftMode = optString(params, "graftMode");
  if (graftMode !== undefined && graftMode !== "pull" && graftMode !== "push" && graftMode !== "both") {
    throw rpcError(-32602, "knowledge.setOptions `graftMode` must be one of pull|push|both");
  }
  // Plan 121 follow-up: the explicit `/graft-build-deep` model. Resolved
  // before any activation so a malformed shape or a missing key fails the
  // whole call (-32602) instead of half-binding graft without the knob.
  const graftDeepModel = rawDeepModel === undefined ? undefined : await resolveGraftDeepModel(host, rawDeepModel);
  if (wiki === false) disableWiki(host);
  else if (wiki === true) await enableWiki(host, workspaceRoot, optString(params, "qmdPath"));
  let graftResolved: boolean | undefined;
  if (graft === false) disableGraft(host);
  else if (graft === true) {
    graftResolved = await enableGraft(host, workspaceRoot, {
      ...(graftMode ? { mode: graftMode } : {}),
      ...(optString(params, "graftCliPath") ? { cliPath: optString(params, "graftCliPath") } : {}),
      ...(graftDeepModel ? { deepModel: graftDeepModel } : {}),
    });
  }
  return {
    workspaceRoot,
    ...(wiki !== undefined ? { wiki } : {}),
    ...(graft !== undefined ? { graft: graftResolved ?? graft } : {}),
    ...(graftDeepModel
      ? { graftDeepModel: { provider: graftDeepModel.provider, model: graftDeepModel.model } }
      : {}),
  };
}

/** Resolve the explicit graft deep model (plan 121 follow-up): the model
 *  identity arrives over the trusted RPC, the API key does not have to — an
 *  omitted `apiKey` reads the provider credential the agent picker already
 *  stores, so `init.js` never carries the secret. Inline keys are accepted
 *  for providers with no stored credential (litellm/orcarouter) and join the
 *  redactor set. Unknown or incomplete shapes fail closed (-32602). */
async function resolveGraftDeepModel(host: ClayAgentHost, raw: unknown): Promise<GraftDeepModel> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    throw rpcError(-32602, "graftDeepModel must be an object");
  }
  const record = raw as Record<string, unknown>;
  const provider = reqString(record, "provider");
  if (!(GRAFT_DEEP_PROVIDERS as readonly string[]).includes(provider)) {
    throw rpcError(-32602, `graftDeepModel.provider must be one of ${GRAFT_DEEP_PROVIDERS.join("|")}`);
  }
  const model = reqString(record, "model");
  const baseUrl = optString(record, "baseUrl");
  const inlineKey = optString(record, "apiKey");
  if (inlineKey !== undefined && inlineKey.length === 0) {
    throw rpcError(-32602, "graftDeepModel.apiKey must be a non-empty string when present");
  }
  const stored = inlineKey ?? (await host.vault.get({ name: host.defaultCredentialName(provider), provider }))?.value;
  if (stored === undefined || stored.length === 0) {
    throw rpcError(-32602, `graftDeepModel requires an apiKey or a stored \`${provider}\` credential`);
  }
  if (inlineKey !== undefined) host.rememberSecret(inlineKey);
  return {
    provider: provider as GraftDeepModel["provider"],
    model,
    apiKey: stored,
    ...(baseUrl ? { baseUrl } : {}),
  };
}

export function disableWiki(host: ClayAgentHost): void {
  const current = host.wiki;
  if (!current) return;
  host.wiki = undefined;
  current.loaded.dispose();
}

export async function enableWiki(host: ClayAgentHost, workspaceRoot: string, qmdPath?: string): Promise<void> {
  if (host.wiki?.workspaceRoot === workspaceRoot) return;
  // One wiki binding per daemon: dispose the previous workspace's
  // extension (its registry contributions go with it) before loading the
  // new one — the wiki tools are single-named singletons. A failed load
  // leaves the option disabled (fail closed), never half-bound.
  const previous = host.wiki;
  host.wiki = undefined;
  previous?.loaded.dispose();
  // URL ingest is host-fetched (the wiki package ships no HTTP client and
  // never fetches itself): wire Obscura only when its binary resolves, so
  // text/path ingest always works and `url` fails closed with a named
  // error when Obscura is absent. The hook re-validates the URL
  // (`validateObscuraWebUrl`: public http(s), no credentials — Prism ran
  // `assertSsrfAllowedUrl` before calling it) and runs the CLI under its
  // own owned path/flags; the child runs per ingest, never on the editor
  // hot path. The output is staged as markdown (`source.md`), so an
  // Obscura fetch of a PDF URL lands as its markdown dump, not as a
  // binary the wiki would try to parse.
  const obscuraPath = host.resolveObscura();
  const fetchUrl: WikiExtensionOptions["fetchUrl"] = obscuraPath
    ? async ({ url }) => {
        validateObscuraWebUrl(url);
        const run = await runObscuraCli({
          command: obscuraPath,
          args: ["fetch", url, "--dump", "markdown"],
        });
        return run.stdout.length > 0 ? { text: run.stdout, filename: "source.md" } : null;
      }
    : undefined;
  const options: WikiExtensionOptions = {
    workspaceRoot,
    // Relative default: ".wiki" in the workspace root. Prism 0.7
    // resolves it against `workspaceRoot` (`path.resolve`) for every
    // tool/command and reports the absolute path from `/wiki-init`;
    // an absolute value here also resolves to itself.
    wikiRoot: ".wiki",
    // Skill files deploy into the workspace on init by default; skills
    // load from the kernel registry instead (wiki writes stay inside
    // .wiki/).
    autoDeploySkills: false,
    // qmd hybrid search is strictly opt-in (host-owned binary, deny by
    // default): without an explicit path the catalog fallback serves.
    ...(qmdPath ? { qmdPath } : {}),
    ...(fetchUrl ? { fetchUrl } : {}),
  };
  let loaded: LoadedExtension;
  try {
    [loaded] = await host.kernel.load([createWikiExtension(options)]);
  } catch (error) {
    throw rpcError(-32000, `wiki activation failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  host.wiki = { workspaceRoot, loaded };
  // Agent-delivered skills register from their file-backed content
  // (loadAgentSkillFiles at host creation), gated per skill (decision
  // 2026-09-09-1420). The feature gate (wiki-searcher) is checked by the
  // /wiki-init initiator.
  for (const [dir, key, fallback] of [
    ["wiki-searcher", "wikiSearcher", wikiSearcherSkill],
    ["wiki-maintainer", "wikiMaintainer", wikiMaintainerSkill],
  ] as const) {
    const delivered = host.agentSkillFiles.get(dir) ?? fallback;
    if (!host.skills.get(dir) && host.agentSkillEnabled(key)) {
      host.skills.register(delivered);
    }
  }
}

/** Wiki knowledge tools for the bound workspace; empty when the option is
 *  disabled or the session's workspace is not the bound one. */
export function wikiTools(host: ClayAgentHost, workspaceRoot: string): ToolDefinition[] {
  if (host.wiki?.workspaceRoot !== workspaceRoot) return [];
  return [WIKI_SEARCH_TOOL_NAME, WIKI_READ_PAGE_TOOL_NAME, WIKI_RECORD_INSIGHT_TOOL_NAME, WIKI_INGEST_TOOL_NAME].map(
    (name) => host.kernel.registries.tools.resolve(name),
  );
}

/** Graft knowledge tools for the bound workspace (pull/both modes only —
 *  push-only registers no tools). Empty when disabled, unbound, or when
 *  the CLI resolved without registering a given tool. */
export function graftTools(host: ClayAgentHost, workspaceRoot: string): ToolDefinition[] {
  const binding = host.graft;
  if (binding?.workspaceRoot !== workspaceRoot || binding.mode === "push") return [];
  return GRAFT_TOOL_NAMES.map((name) => host.kernel.registries.tools.get(name)).filter(
    (tool): tool is ToolDefinition => tool !== undefined,
  );
}

/** Opt-in graft knowledge base (plan 108 task 13). Disabled by default.
 *  Enabling resolves the graft CLI fail-closed BEFORE load: an absent CLI
 *  (no cliPath, no host packageRoot, no @nanonets/graft peer) leaves the
 *  option off and the agent unperturbed — tools hidden, never a half
 *  binding. One binding per daemon; re-enabling rebinds. A changed
 *  `deepModel` also rebinds (the extension resolves its child env once at
 *  load), so the explicit `/graft-build-deep` opt-in can be added after a
 *  default-on bind without a restart. */
export async function enableGraft(
  host: ClayAgentHost,
  workspaceRoot: string,
  options: { mode?: GraftMode; cliPath?: string; deepModel?: GraftDeepModel },
): Promise<boolean> {
  // agentSkills.graft=false ⇒ no binding attempt ever, on any path
  // (default or explicit RPC) — the gate is authoritative (decision
  // 2026-09-09-1420).
  if (!host.agentSkillEnabled("graft")) return false;
  const deepIdentity = deepModelIdentity(options.deepModel);
  if (host.graft?.workspaceRoot === workspaceRoot && host.graft.deepModelIdentity === deepIdentity) return true;
  const previous = host.graft;
  host.graft = undefined;
  previous?.loaded.dispose();
  try {
    resolveGraftCli(
      options.cliPath ? { cliPath: options.cliPath } : host.graftCliPath ? { cliPath: host.graftCliPath } : {},
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
    // `/graft-init` is non-interactive by design (the child has no TTY):
    // Prism then passes `--yes` plus fixed `--no-global --no-mcp
    // --no-hooks --no-statusline` — never user-level state, and Clay keeps
    // Prism's own graft surfaces instead of MCP/hook wiring. `initWireMcp`
    // stays unset. Without a `deepModel` (below),
    // `/graft-build-deep` refuses before spawning (no hidden paid call).
    initYes: true,
    // Plan 121 follow-up: explicit deep-build model. Prism merges it into
    // the child env as GRAFT_PROVIDER/GRAFT_MODEL/GRAFT_API_KEY
    // (+GRAFT_BASE_URL), never argv; the key came from the vault unless the
    // trusted caller passed one inline.
    ...(options.deepModel ? { deepModel: options.deepModel } : {}),
    // Push-surface persistence (OM attach pattern). State patches route
    // by the entry's own sessionId; getEntries rides the active run.
    appendEntry: (entry, appendOptions) => host.persistence.append(entry, appendOptions),
    getEntries: () =>
      host.activeGraftSessionId ? host.persistence.list(host.activeGraftSessionId) : Promise.resolve([]),
  };
  let loaded: LoadedExtension;
  try {
    [loaded] = await host.kernel.load([createGraftExtension(extensionOptions)]);
  } catch (error) {
    throw rpcError(-32000, `graft activation failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  host.graft = { workspaceRoot, loaded, mode: extensionOptions.mode ?? "pull", deepModelIdentity: deepIdentity };
  const graftDelivered = host.agentSkillFiles.get("graft") ?? graftSkill;
  if (!host.skills.get(graftDelivered.name)) host.skills.register(graftDelivered);
  return true;
}

export function disableGraft(host: ClayAgentHost): void {
  const current = host.graft;
  if (!current) return;
  host.graft = undefined;
  host.activeGraftSessionId = undefined;
  current.loaded.dispose();
}

/** Graft default-on trigger (plan 117): first coding session for a
 *  workspace attempts the pull-mode binding, once per root per daemon —
 *  no retry loops. Fail-closed: unresolvable CLI ⇒ off, tools hidden,
 *  agent unperturbed. knowledge.setOptions stays the explicit
 *  disable/mode-override surface and bypasses the attempt cache. */
export async function ensureGraftBound(host: ClayAgentHost, workspaceRoot: string): Promise<boolean> {
  if (!host.agentSkillEnabled("graft")) return false;
  if (host.graft?.workspaceRoot === workspaceRoot) return true;
  if (host.graftBindAttempted.has(workspaceRoot)) return false;
  host.graftBindAttempted.add(workspaceRoot);
  try {
    return await enableGraft(host, workspaceRoot, {
      mode: "pull",
      ...(host.graftCliPath ? { cliPath: host.graftCliPath } : {}),
    });
  } catch {
    // Default bind never surfaces errors (agent unperturbed): a load
    // failure (broken CLI binary, extension error) fails closed exactly
    // like an unresolvable CLI. The explicit RPC path still throws for
    // caller feedback.
    return false;
  }
}
