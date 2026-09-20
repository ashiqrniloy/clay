/**
 * Profile/command surfaces for ClayAgentHost (plan 130 task 2): profile +
 * command registration, the environment/completion snapshot, slash-command
 * parsing and dispatch, run.setOptions, host command drivers, and the
 * host-side command handlers + transcript feedback.
 */
import type {
  AgentDefinition,
  CommandDefinition,
  CommandDrivers,
  CommandExecutionContext,
  CommandResult,
  JsonObject,
  RunOptions,
} from "@arnilo/prism";
import type { ClayAgentHost } from "../host.js";
import {
  MAX_COMPLETION_COMMANDS,
  MAX_COMPLETION_DESCRIPTION_CHARS,
  MAX_COMPLETION_NAME_CHARS,
  optPolicyCap,
  optPositiveInt,
  optString,
  reqString,
  rpcError,
  type RunConfig,
} from "./internals.js";
import { isCompactionStrategyName, type CompactionStrategyName } from "../compaction.js";

export function profileList(host: ClayAgentHost): unknown {
  return {
    profiles: host.kernel.registries.agents.list().map((profile) => ({
      name: profile.name,
      description: profile.description,
      tools: profile.tools ?? [],
      skills: profile.skills ?? [],
    })),
  };
}

export function profileRegister(host: ClayAgentHost, params: Record<string, unknown>): unknown {
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
  host.kernel.registries.agents.register(name, def);
  process.stderr.write(`agentProfile.register '${name}' applied\n`);
  if (params.observationalMemory === true) host.omProfiles.add(name);
  else if (params.observationalMemory === false) host.omProfiles.delete(name);
  return { name, registered: true, observationalMemory: host.omProfiles.has(name) };
}

/** Register a command definition (data + host-side handler name). Drivers are
 *  never accepted over RPC; the host injects them at dispatch time. */
export function commandRegister(host: ClayAgentHost, params: Record<string, unknown>): unknown {
  const name = reqString(params, "name");
  const handler = optString(params, "handler");
  if (handler !== undefined && !isCommandHandlerName(handler)) {
    throw rpcError(-32602, `unknown command handler: ${handler}`);
  }
  if (host.kernel.registries.commands.get(name)) throw rpcError(-32602, `duplicate command: ${name}`);
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
    execute: (args, context) => runHostCommand(host, name, handler, args, context),
  };
  host.kernel.registries.commands.register(name, command);
  return { name, registered: true, ...(handler !== undefined ? { handler } : {}) };
}

/** Package commands register as "/name"; kernel-registered commands
 *  (e.g. the opt-in wiki commands) use bare names — try both so slash
 *  invocation reaches any registered command. */
export function registeredCommand(host: ClayAgentHost, name: string): CommandDefinition | undefined {
  const registry = host.kernel.registries.commands;
  return registry.get(name) ?? registry.get(name.slice(1));
}

/** Plan 109 R1/R3: the composer completion's single source — the
 *  daemon's registered commands (names normalized to `/name`) — plus
 *  the loaded opt-in extensions for the status strip. Registered
 *  post-boot (init.js) commands appear on the next fetch; the Rust
 *  server caches per daemon generation and invalidates on
 *  command.register / knowledge.setOptions. */
export function environmentList(host: ClayAgentHost, params: Record<string, unknown> = {}): JsonObject {
  const commands: JsonObject[] = [];
  for (const command of host.kernel.registries.commands.list()) {
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
  if (host.wiki) extensions.push(host.wiki.loaded.name.slice(0, 48));
  if (host.graft) extensions.push(host.graft.loaded.name.slice(0, 48));
  // Catalog skills ride the same snapshot (skills card on the coding
  // surface): bounded name/description pairs, same caps as commands.
  // Per-server MCP connect outcomes ride too (MCP card / composer
  // section): id, connected, tool count, and the hidden-because error.
  // Plan 118 task 35: the inventories follow the session's agent when one
  // is named (the Context tab's skills + MCP cards after a switch);
  // without a session the default agent's view is the honest answer.
  const sessionId = optString(params, "sessionId");
  const agentRoot = sessionId
    ? (host.live.get(sessionId)?.agentRoot ?? host.agentConfigRoot)
    : // The Rust server caches per agent type and asks by name; an absent
      // agent is the default one.
      host.resolveAgentRoot(params.agent);
  return {
    commands,
    extensions,
    // Ceiling (plan 118 task 35): the skill registry is host-wide, so the
    // catalog can list a skill another agent root discovered. A run's
    // *active* skills/tools come from the session's own agent config, which
    // is what the switch changes; per-root catalogs need a source tag on
    // the registry, which Prism's skill registry does not carry.
    skills: host.visibleSkills(),
    mcpServers: host.mcpServerOutcomes(agentRoot),
  };
}

export async function commandDispatch(host: ClayAgentHost, params: Record<string, unknown>): Promise<unknown> {
  const name = reqString(params, "name");
  const command = registeredCommand(host, name);
  if (!command) throw rpcError(-32602, `unknown command: ${name}`);
  const sessionId = optString(params, "sessionId");
  const live = sessionId ? host.live.get(sessionId) : undefined;
  // Host-injected drivers; absent when no live session so the context key is omitted.
  const drivers = commandDrivers(host, live);
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
 *  `null` disables that Prism axis. compactAfterTokens is the absolute
 *  ceiling of the auto-compaction trigger armed at `createSession` (see
 *  `autoCompaction`); new sessions pick it up, live ones keep their gate. */
export function runSetOptions(host: ClayAgentHost, params: Record<string, unknown>): unknown {
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
  const runConfig: RunConfig = host.runConfig;
  host.runConfig = {
    maxTurns: maxTurns !== undefined ? maxTurns : runConfig.maxTurns,
    maxToolRounds: maxToolRounds !== undefined ? maxToolRounds : runConfig.maxToolRounds,
    maxToolCalls: maxToolCalls !== undefined ? maxToolCalls : runConfig.maxToolCalls,
    maxWallTimeMs: maxWallTimeMs !== undefined ? maxWallTimeMs : runConfig.maxWallTimeMs,
    maxInputTokens: maxInputTokens !== undefined ? maxInputTokens : runConfig.maxInputTokens,
    maxOutputTokens: maxOutputTokens !== undefined ? maxOutputTokens : runConfig.maxOutputTokens,
    compactAfterTokens: compactAfterTokens ?? runConfig.compactAfterTokens,
    compaction: compaction ?? runConfig.compaction,
  };
  return { ...host.runConfig };
}

/** Host driver capabilities for commands. Never sourced from RPC params. */
export function commandDrivers(host: ClayAgentHost, live: import("./internals.js").LiveSession | undefined): CommandDrivers | undefined {
  if (!live) return undefined;
  return {
    startRun: (input, options) => live.session.run(input, { ...(options ?? {}), identity: host.runIdentity() }),
    startWorkflow: async () => {
      throw rpcError(-32000, "workflows not enabled (Phase 5)");
    },
    steer: (_runId, input) => live.session.steer(input),
  };
}

/** Host-side execute for a registered command's named handler. The handler
 *  set is closed (`COMMAND_HANDLERS`): driver handlers run against the
 *  dispatch context's drivers, the rest map 1:1 onto the daemon's own
 *  session.* RPCs. */
export function runHostCommand(
  host: ClayAgentHost,
  commandName: string,
  handler: string | undefined,
  args: JsonObject,
  context: CommandExecutionContext,
): Promise<CommandResult> | CommandResult {
  if (handler === undefined) throw rpcError(-32000, `command ${commandName} has no host handler`);
  if ((DRIVER_HANDLERS as readonly string[]).includes(handler)) {
    return runDriverCommand(host, commandName, handler, args, context);
  }
  return runSessionCommand(host, commandName, handler, args, context);
}

const DRIVER_HANDLERS = ["steer", "startRun", "startWorkflow"] as const;

/** Driver handlers fail closed when no live session provided drivers. */
function runDriverCommand(
  host: ClayAgentHost,
  commandName: string,
  handler: string,
  args: JsonObject,
  context: CommandExecutionContext,
): Promise<CommandResult> | CommandResult {
  const drivers = context.drivers;
  if (!drivers) throw rpcError(-32000, `${commandName} requires an active session (drivers absent)`);
  switch (handler) {
    case "steer": {
      const input = reqString(args, "input");
      const runId = optString(args, "runId") ?? "";
      drivers.steer(runId, input);
      return { name: commandName, value: { steered: true, runId, input } };
    }
    case "startRun": {
      const input = reqString(args, "input");
      const options = args.options && typeof args.options === "object" ? (args.options as RunOptions) : undefined;
      return Promise.resolve(drivers.startRun(input, options)).then((result) => ({
        name: commandName,
        value: { runId: result.runId, status: result.status },
      }));
    }
    case "startWorkflow": {
      const definition = args.definition && typeof args.definition === "object" ? (args.definition as JsonObject) : {};
      return Promise.resolve(drivers.startWorkflow(definition, args.input)).then((result) => ({
        name: commandName,
        value: result,
      }));
    }
    default:
      throw rpcError(-32602, `unknown command handler: ${handler}`);
  }
}

/** Session-scoped handlers: each maps onto the daemon's own session.* RPC
 *  (and /tree onto the host's tree summary). The context's sessionId is the
 *  only authority consulted — never RPC-supplied drivers. */
function runSessionCommand(
  host: ClayAgentHost,
  commandName: string,
  handler: string,
  args: JsonObject,
  context: CommandExecutionContext,
): Promise<CommandResult> | CommandResult {
  const sessionId = () => requireCommandSession(commandName, context);
  switch (handler) {
    case "compact":
      return callRpc(host, commandName, "session.compact", { sessionId: sessionId() });
    case "newSession": {
      // Re-seed a fresh session from the current one's profile/provider/model.
      const current = requireLive(host, commandName, context);
      return callRpc(host, commandName, "session.new", {
        profile: current.profile,
        provider: current.provider,
        model: current.model,
        workspaceRoot: current.workspaceRoot,
      });
    }
    case "checkout": {
      const entryId = requireEntryId(commandName, args);
      return callRpc(host, commandName, "session.checkout", { sessionId: sessionId(), entryId });
    }
    case "discard": {
      // Post-hoc discard (decision 2200): checkout to the branch point —
      // conversation + document checkpoint roll back in one action, the
      // abandoned side keeps its entries and summary. Two-step confirm:
      // without `confirm` the reply previews and nothing changes.
      const entryId = requireEntryId(commandName, args);
      if (args.confirm !== true) {
        return Promise.resolve(host.sessionTreeSummary(sessionId())).then((tree) => ({
          name: commandName,
          value: { needsConfirm: true, entryId, reRun: `${commandName} ${entryId} confirm` },
        }));
      }
      return callRpc(host, commandName, "session.checkout", { sessionId: sessionId(), entryId });
    }
    case "forkSession":
    case "cloneSession": {
      const entryId = optString(args, "entryId");
      const method = handler === "forkSession" ? "session.fork" : "session.clone";
      return callRpc(host, commandName, method, { sessionId: sessionId(), ...(entryId ? { entryId } : {}) });
    }
    case "tree":
      return Promise.resolve(host.sessionTreeSummary(sessionId())).then((value) => ({ name: commandName, value }));
    case "openSession":
      // `--session`-equivalent: open a specific session by id (bounded page).
      return callRpc(host, commandName, "session.load", { sessionId: reqString(args, "sessionId") });
    case "openSessionAsFork": {
      // `--fork`-equivalent: open a session and fork it at its leaf (or entryId).
      const id = reqString(args, "sessionId");
      const entryId = optString(args, "entryId");
      return (async () => {
        await host.handle("session.resume", { sessionId: id });
        return callRpc(host, commandName, "session.fork", { sessionId: id, ...(entryId ? { entryId } : {}) });
      })();
    }
    default:
      throw rpcError(-32602, `unknown command handler: ${handler}`);
  }
}

function callRpc(
  host: ClayAgentHost,
  commandName: string,
  method: string,
  params: Record<string, unknown>,
): Promise<CommandResult> {
  return Promise.resolve(host.handle(method, params)).then((value) => ({ name: commandName, value }));
}

function requireEntryId(commandName: string, args: JsonObject): string {
  const entryId = optString(args, "entryId") ?? optString(args, "input");
  if (!entryId) throw rpcError(-32602, `${commandName} requires an entryId`);
  return entryId;
}

/** The dispatch context's sessionId (never RPC-supplied drivers). */
function requireCommandSession(commandName: string, context: CommandExecutionContext): string {
  const sessionId = context.sessionId;
  if (!sessionId) throw rpcError(-32000, `${commandName} requires an active session`);
  return sessionId;
}

function requireLive(
  host: ClayAgentHost,
  commandName: string,
  context: CommandExecutionContext,
): { profile: string; provider: string; model: string; workspaceRoot: string } {
  const sessionId = requireCommandSession(commandName, context);
  const live = host.live.get(sessionId);
  if (!live) throw rpcError(-32000, `${commandName} requires an active session`);
  return live;
}

/** Slash-command feedback rides the transcript lane: a synthetic
 *  started/delta/finished triple renders the result as an assistant message
 *  and closes the client's AG-UI run (the dispatch reply alone would leave
 *  the run observable open and the composer stuck streaming). Bounded. */
export function emitCommandFeedback(
  host: ClayAgentHost,
  sessionId: string,
  commandName: string,
  value: unknown,
  text?: string,
): void {
  const record = value && typeof value === "object" ? (value as Record<string, unknown>) : {};
  const body =
    text ??
    (record.needsConfirm === true
      ? `discard preview: branching from ${String(record.entryId).slice(0, 8)} restores that checkpoint and abandons the later path. Re-run with confirm to discard.`
      : typeof record.leafId === "string"
        ? `${commandName.slice(1)}: leaf ${record.leafId.slice(0, 8)}`
        : `${commandName.slice(1)}: done`);
  const runId = `cmd-${commandName.slice(1).slice(0, 32)}`;
  host.emit("event", { sessionId, event: { type: "agent_started", sessionId, runId } });
  host.emit("event", {
    sessionId,
    event: { type: "message_delta", sessionId, runId, content: { type: "text", text: body.slice(0, 8192) } },
  });
  host.emit("event", { sessionId, event: { type: "agent_finished", sessionId, runId } });
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

/** Parse a slash-command invocation: `/name` alone or `/name {json args}` /
 *  `/name free text` (free text becomes `{ "input": text }`). Returns
 *  undefined for non-slash prompts. Bounded: name ≤ 64 chars, args ≤ 4 KiB. */
export function commandInvocation(text: string): { name: string; argsText?: string } | undefined {
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
export function slashArgs(invocation: { argsText?: string }): JsonObject {
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
