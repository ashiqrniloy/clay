/**
 * Per-agent-root configuration and MCP/Obscura capability management for
 * ClayAgentHost (plan 130 task 2): agent-type resolution to config roots,
 * once-per-root config loading (tool caps, skills config, seeded files),
 * the server-built MCP allow-list bookkeeping, and lazy capability
 * activation (MCP bridges + Obscura harness).
 */
import { readFile } from "node:fs/promises";
import { statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import type { JsonObject, ToolDefinition } from "@arnilo/prism";
import { normalizeToolCaps, type RepositoryToolCaps } from "../coding-tools.js";
import { connectAllowListedMcpServers, MAX_MCP_SERVERS } from "../mcp.js";
import { spawnObscuraHarness } from "../obscura.js";
import type { ClayAgentHost } from "../host.js";
import {
  MAX_COMPLETION_DESCRIPTION_CHARS,
  MAX_COMPLETION_NAME_CHARS,
  rpcError,
  type AgentRootConfig,
} from "./internals.js";
import { loadAgentSkillFiles, loadSkillsConfig, seedUserSystemPrompt } from "./skills.js";

/** Plan 118 task 35: agent types are directory names, never paths. Bounded
 *  and separator-free, so a name can only ever address a direct child of
 *  the `agents/` root the default agent lives in. */
const AGENT_TYPE_RE = /^[A-Za-z0-9_-]{1,64}$/;

/** Loaded agent roots kept in memory (config is small; the cap is abuse
 *  protection for a hand-made `agents/` folder). */
const MAX_AGENT_ROOTS = 16;

/** Load user-configured repository scan caps from tool-caps.json. Since
 *  decision 2026-09-10-1526 the file is per-agent CONFIGURATION living in
 *  the agent config root (beside skills.json/mcp.json); the data-dir
 *  location is the pre-decision legacy path, still honored when the
 *  config-root file is absent (a migrated data dir carries the old file).
 *  Missing everywhere = defaults; malformed = warn and fall back to
 *  defaults (never blocks daemon boot). */
export async function loadToolCaps(
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

export { CODING_AGENT_CONFIG_DIR };

/**
 * Plan 118 task 35: resolve one agent type to its config root.
 *
 * The name is a bare directory name (`AGENT_TYPE_RE`), so it cannot escape
 * the `agents/` root — the root is a direct child of the default root's
 * parent. An explicit name that does not resolve is rejected fail-closed
 * (never silently the default agent: that would run one agent's config
 * under another's name). An absent name means the default agent.
 */
export function resolveAgentRoot(host: ClayAgentHost, agent: unknown): string {
  if (agent === undefined || agent === null) return host.agentConfigRoot;
  if (typeof agent !== "string") {
    throw rpcError(-32602, "agent must be a string when present");
  }
  const name = agent.trim();
  if (name === "") return host.agentConfigRoot;
  if (!AGENT_TYPE_RE.test(name)) {
    throw rpcError(-32602, `invalid agent type: ${JSON.stringify(agent)}`);
  }
  const root = join(dirname(host.agentConfigRoot), name);
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
export function mcpAllowListFor(host: ClayAgentHost, root: string): readonly unknown[] {
  return (
    host.mcpAllowListByRoot.get(root) ??
    (root === host.agentConfigRoot ? host.mcpAllowList : [])
  );
}

/** The default agent's config as the host loaded it at create time (the
 *  synchronous recreate paths need no await). */
export function defaultAgentConfig(host: ClayAgentHost): AgentRootConfig {
  return {
    root: host.agentConfigRoot,
    toolCaps: { caps: host.toolCaps, file: host.capsFile },
    skillsConfig: host.skillsConfig,
    configSkillNames: host.configSkillNames ?? [],
    skillFiles: host.agentSkillFiles,
  };
}

/** The type name for a root: the default agent is `undefined` (the server
 *  records no name for it). */
export function agentTypeOf(host: ClayAgentHost, root: string): string | undefined {
  return root === host.agentConfigRoot ? undefined : basename(root);
}

/**
 * Plan 118 task 35: load (once) the config of one agent root — seeding its
 * SYSTEM.md and delivered skills first, then tool caps, `skills.json`, the
 * delivered skill files, and the root's discovered skill names. Bounded by
 * `MAX_AGENT_ROOTS`: past the cap a root loads uncached (correct, just
 * re-read) rather than dropping the session.
 */
export function agentRootConfig(host: ClayAgentHost, root: string): Promise<AgentRootConfig> {
  const cached = host.agentRoots.get(root);
  if (cached) return cached;
  const pending = (async (): Promise<AgentRootConfig> => {
    await seedUserSystemPrompt(root).catch((error) => {
      process.stderr.write(
        `[system] seeding SYSTEM.md for ${root} failed: ${error instanceof Error ? error.message : String(error)}\n`,
      );
    });
    const toolCaps = await loadToolCaps(root, host.dataDir);
    const skillsConfig = (await loadSkillsConfig(root, host.homeSkillsRoot)).config;
    const skillFiles = await loadAgentSkillFiles(root);
    // User-added skills of this root register here (the delivered
    // wiki/graft files do not: their own activation paths own those gates).
    const configSkillNames = skillsConfig.configRootEnabled
      ? await host.scanSkillsDir(join(root, "skills"))
      : [];
    return { root, toolCaps, skillsConfig, configSkillNames, skillFiles };
  })();
  if (host.agentRoots.size >= MAX_AGENT_ROOTS) {
    process.stderr.write(
      `[agent] agent root cap of ${MAX_AGENT_ROOTS} reached; ${root} loads uncached\n`,
    );
    return pending;
  }
  host.agentRoots.set(root, pending);
  return pending;
}

/**
 * Lazy capability activation on the first coding session (never on import
 * or Chat initialize). Validation errors fail closed (they mean the server
 * sent a malformed allow-list); connection failures hide the capability —
 * a server that will not connect simply provides no tools.
 */
export function ensureCapabilities(
  host: ClayAgentHost,
  agentRoot: string = host.agentConfigRoot,
  allowList: readonly unknown[] = host.mcpAllowList,
): Promise<void> {
  if (agentRoot === host.agentConfigRoot && host.capabilitiesPromise) {
    // The default agent's capabilities keep their single lazy promise
    // (initialize-order semantics unchanged).
    return host.capabilitiesPromise;
  }
  if (!host.mcpByRoot.has(agentRoot)) host.mcpByRoot.set(agentRoot, { outcomes: [] });
  const pending = (async () => {
      if (allowList.length > 0 && !host.mcpByRoot.get(agentRoot)?.mcp) {
        try {
          const connected = await connectAllowListedMcpServers(allowList);
          host.mcpByRoot.set(agentRoot, { mcp: connected, outcomes: connected.outcomes });
          if (agentRoot === host.agentConfigRoot) {
            // The initialize-time list stays the default agent's bridge.
            host.mcp = connected;
            host.mcpOutcomes = connected.outcomes;
          }
        } catch (error) {
          if ((error as { rpcCode?: number }).rpcCode === -32602) throw error;
          host.mcpByRoot.set(agentRoot, { outcomes: [] });
        }
      }
      if (!host.obscura) {
        const bin = host.resolveObscura();
        if (bin) {
          try {
            host.obscura = await spawnObscuraHarness({ command: bin });
          } catch {
            host.obscura = undefined; // hidden, never an error (2159)
          }
        }
      }
    })();
  if (agentRoot === host.agentConfigRoot) {
    // Keep the documented single-promise default path (callers await it
    // again); per-agent roots await their own connect.
    host.capabilitiesPromise = pending;
  }
  return pending;
}

/** Capability tools for a coding session; undefined shapes stay allowed. */
export function capabilityTools(host: ClayAgentHost, agentRoot: string = host.agentConfigRoot): ToolDefinition[] {
  const bridge = host.mcpByRoot.get(agentRoot)?.mcp ?? (agentRoot === host.agentConfigRoot ? host.mcp : undefined);
  return [...(bridge?.tools ?? []), ...(host.obscura?.tools ?? [])];
}

/** Per-server MCP connect outcomes for the environment/UI (bounded,
 *  trimmed). Plan 118 task 35: scoped to an agent root — a switched
 *  session reports the servers it actually has. */
export function mcpServerOutcomes(host: ClayAgentHost, agentRoot: string = host.agentConfigRoot): JsonObject[] {
  const outcomes =
    host.mcpByRoot.get(agentRoot)?.outcomes ??
    (agentRoot === host.agentConfigRoot ? host.mcpOutcomes : []);
  return outcomes.slice(0, MAX_MCP_SERVERS).map((outcome) => ({
    serverId: outcome.serverId.slice(0, MAX_COMPLETION_NAME_CHARS),
    connected: outcome.connected,
    tools: outcome.tools,
    ...(outcome.error === undefined ? {} : { error: outcome.error.slice(0, MAX_COMPLETION_DESCRIPTION_CHARS) }),
  }));
}
