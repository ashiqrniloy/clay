/**
 * Allow-listed MCP bridge (decision 1758). The host (server) supplies the
 * complete allow-list; every entry must name a canonical (absolute) executable
 * with literal argv and explicit env names — nothing is searched, extended, or
 * resolved from package JavaScript. An empty allow-list connects nothing and
 * yields no tools (deny by default).
 */
import { connectMcpTools, type McpToolBridge } from "@arnilo/prism-mcp";
import type { ToolDefinition } from "@arnilo/prism";

/** Maximum allow-listed servers per connect (matches Prism ACP bounds spirit). */
export const MAX_MCP_SERVERS = 32;

/** Hard ceiling on literal argv per entry. */
const MAX_ARGV = 64;

/** Server-built allow-list entry: fixed id, canonical executable, literal argv. */
export interface McpAllowListEntry {
  readonly serverId: string;
  readonly command: string;
  readonly args?: readonly string[];
  readonly env?: Readonly<Record<string, string>>;
  readonly cwd?: string;
}

export interface ConnectedMcpServers {
  /** Prefixed Prism ToolDefinitions (`mcp:<serverId>:<name>`). */
  readonly tools: ToolDefinition[];
  close(): Promise<void>;
}

function fail(message: string): never {
  throw Object.assign(new Error(message), { rpcCode: -32602 });
}

function canonicalCommand(entry: McpAllowListEntry, index: number): string {
  const where = `mcpAllowList[${index}]`;
  if (typeof entry.command !== "string" || entry.command.length === 0) {
    fail(`${where}.command is required`);
  }
  if (entry.command.includes("\0")) fail(`${where}.command contains NUL`);
  // Canonical executable: absolute path only. No PATH search, no bare names.
  if (!entry.command.startsWith("/")) {
    fail(`${where}.command must be an absolute path (got "${entry.command}")`);
  }
  return entry.command;
}

function literalArgv(entry: McpAllowListEntry, index: number): readonly string[] {
  const where = `mcpAllowList[${index}]`;
  if (entry.args === undefined) return [];
  if (!Array.isArray(entry.args)) fail(`${where}.args must be an array`);
  if (entry.args.length > MAX_ARGV) fail(`${where}.args exceeds ${MAX_ARGV} entries`);
  for (const arg of entry.args) {
    if (typeof arg !== "string") fail(`${where}.args entries must be strings`);
    if (arg.includes("\0")) fail(`${where}.args contains NUL`);
  }
  return entry.args;
}

function explicitEnv(entry: McpAllowListEntry, index: number): Record<string, string> | undefined {
  const where = `mcpAllowList[${index}]`;
  if (entry.env === undefined) return undefined;
  if (typeof entry.env !== "object" || Array.isArray(entry.env)) fail(`${where}.env must be an object`);
  const env: Record<string, string> = {};
  for (const [name, value] of Object.entries(entry.env)) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) fail(`${where}.env has invalid name "${name}"`);
    if (typeof value !== "string") fail(`${where}.env values must be strings`);
    env[name] = value;
  }
  return Object.keys(env).length > 0 ? env : undefined;
}

function parseEntry(raw: unknown, index: number): McpAllowListEntry & { command: string; args: readonly string[] } {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) {
    fail(`mcpAllowList[${index}] must be an object`);
  }
  const entry = raw as Record<string, unknown> & Partial<McpAllowListEntry>;
  const known = new Set(["serverId", "command", "args", "env", "cwd"]);
  for (const key of Object.keys(entry)) {
    if (!known.has(key)) fail(`mcpAllowList[${index}].${key} is not a recognized field`);
  }
  const serverId = entry.serverId;
  if (typeof serverId !== "string" || serverId.length === 0) {
    fail(`mcpAllowList[${index}].serverId is required`);
  }
  // Prism serverId rules: keep the namespace safe for mcp:<serverId>:<name>.
  if (!/^[a-z0-9][a-z0-9._-]{0,63}$/.test(serverId)) {
    fail(`mcpAllowList[${index}].serverId must match [a-z0-9][a-z0-9._-]{0,63}`);
  }
  const cwd = entry.cwd;
  if (cwd !== undefined && (typeof cwd !== "string" || !cwd.startsWith("/"))) {
    fail(`mcpAllowList[${index}].cwd must be an absolute path`);
  }
  return {
    serverId,
    command: canonicalCommand(entry as McpAllowListEntry, index),
    args: literalArgv(entry as McpAllowListEntry, index),
    env: explicitEnv(entry as McpAllowListEntry, index),
    ...(typeof cwd === "string" ? { cwd } : {}),
  };
}

/**
 * Connect every allow-listed server; one failure fails the whole connect
 * closed (no partial tool surface). Empty allow-list resolves immediately
 * with zero tools and a no-op close.
 */
export async function connectAllowListedMcpServers(
  allowList: readonly unknown[],
): Promise<ConnectedMcpServers> {
  if (allowList.length > MAX_MCP_SERVERS) {
    fail(`mcpAllowList exceeds ${MAX_MCP_SERVERS} servers`);
  }
  const entries = allowList.map(parseEntry);
  const bridges: McpToolBridge[] = [];
  try {
    for (const entry of entries) {
      bridges.push(
        await connectMcpTools({
          serverId: entry.serverId,
          transport: {
            type: "stdio",
            command: entry.command,
            args: entry.args,
            ...(entry.env ? { env: entry.env } : {}),
            ...(entry.cwd ? { cwd: entry.cwd } : {}),
            // Bounded stderr capture; the host drains/limits it, not the model.
            stderr: "pipe",
          },
        }),
      );
    }
  } catch (error) {
    // Fail closed: close anything already connected before propagating.
    for (const bridge of bridges) await bridge.close().catch(() => {});
    throw error;
  }
  return {
    tools: bridges.flatMap((bridge) => [...bridge.tools]),
    close: async () => {
      for (const bridge of bridges) await bridge.close().catch(() => {});
    },
  };
}
