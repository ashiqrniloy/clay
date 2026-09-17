// Agent-surface derivations (plan 124 task 6): one reading of the tab's agent
// store, shared by the two surfaces that show it — the persistent agent lane
// (composer, agent controls, session-environment foot) and the agent view
// (transcript, state strip, inspector).
//
// Everything here is derived from the store's snapshot; nothing is owned. The
// lane owns the interactive state (draft, pending effort), the panel owns its
// selection, and the tab owns the store.

import { useMemo, useSyncExternalStore } from "react";

import type { AgentSessionModule, AgentSnapshot } from "../agent/state";
import type { DropdownOption } from "../components";
import { sessionFiles, type SessionFileRecord } from "../agent/session-files";
import {
  isContextItemDetail,
  isContextView,
  type ContextItemDetail,
  type ContextView,
} from "./ContextTab";
import type { OmView } from "./MemoryTab";
import {
  agentOf,
  kindOf,
  toolMeta,
  type TranscriptBox,
} from "./transcript-model";

export interface ModelInfo {
  provider?: unknown;
  model?: unknown;
  displayName?: unknown;
  /** Bounded context ceiling in tokens when the registry reports one
   *  (plan 117 token meter). */
  contextWindow?: unknown;
  /** Declared portable thinking levels, ascending (plan 109 I4). Empty =
   *  no effort control for this model. */
  thinkingLevels?: unknown;
}

export interface ProviderInfo {
  id?: unknown;
  configured?: unknown;
}

/** One MCP server's connect outcome from the daemon environment (plan 117). */
export interface McpServer {
  serverId: string;
  connected: boolean;
  tools: number;
  error: string;
}

/** Slash surface, mirroring the bundled package's daemon registrations
 *  plus the core built-ins (/model opens the daemon model picker). */
/** Chars-per-token by model-family tokenizer class (plan 117): coarse
 *  prose calibration picked from the model id; unlisted families read as
 *  4. Ceiling: swap for a real tokenizer count if Clay ships one.
 *  # ponytail: family table is calibration, not measurement
 */
const CHARS_PER_TOKEN_BY_FAMILY: ReadonlyArray<readonly [RegExp, number]> = [
  [/claude/i, 3.6],
  [/gpt|\bo[1345]/i, 4],
  [/gemini/i, 4],
  [/llama|mistral|qwen|deepseek/i, 3.8],
];

function charsPerToken(model: string): number {
  return (
    CHARS_PER_TOKEN_BY_FAMILY.find(([pattern]) => pattern.test(model))?.[1] ?? 4
  );
}

/** Occupancy estimate when no provider usage has been reported: total
 *  transcript characters divided by the family's chars-per-token rate. */
export function estimateContextTokens(
  messages: ReadonlyArray<{ content?: unknown }>,
  model: string,
): number {
  let chars = 0;
  for (const message of messages) {
    if (typeof message.content === "string") chars += message.content.length;
  }
  return Math.round(chars / charsPerToken(model));
}

/** Compact token count: 220000 → `220k`, 1250000 → `1.2m`, else digits. */
export function compactTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    const millions = (tokens / 1_000_000).toFixed(1).replace(/\.0$/, "");
    return `${millions}m`;
  }
  if (tokens >= 1_000) return `${Math.round(tokens / 1_000)}k`;
  return String(tokens);
}

/** Group configured providers' models for the picker/dropdown surfaces
 *  (plan 109 I3): one derivation, same items both surfaces consume. Models
 *  of unconfigured providers are dropped (matches the daemon picker filter);
 *  group order follows the inventory (provider) order. */
export function groupModelsByProvider(
  models: ModelInfo[],
  providers: ProviderInfo[],
): Array<{ label: string; options: DropdownOption[] }> {
  const configured = new Set(
    providers
      .filter((provider) => provider.configured === true)
      .map((provider) => String(provider.id)),
  );
  const groups: Array<{ label: string; options: DropdownOption[] }> = [];
  for (const model of models) {
    const providerId = String(model.provider ?? "");
    if (!configured.has(providerId)) continue;
    const modelId = String(model.model ?? "");
    const label = String(model.displayName ?? "") || modelId;
    let group = groups.find((candidate) => candidate.label === providerId);
    if (!group) {
      group = { label: providerId, options: [] };
      groups.push(group);
    }
    group.options.push({ id: `model:${providerId}/${modelId}`, label });
  }
  return groups;
}

/** The token meter's bounded readout, or `null` when no ceiling resolves. */
export interface ContextMeter {
  text: string;
  tone: "error" | "warning" | "normal";
  percent: number;
}

export interface AgentSurfaceState {
  snapshot: AgentSnapshot;
  /** The tab's own command lane (this tab's connection). */
  command: (command: Record<string, unknown> | string) => void;
  transcript: TranscriptBox[];
  sessionFileList: SessionFileRecord[];
  /** A provider/model is configured (gates sending, never typing). */
  configured: boolean;
  streaming: boolean;
  provider: string;
  model: string;
  sessionId: string;
  gitBranch: string;
  extensions: string[];
  mcpServers: McpServer[];
  connectedMcpServers: string[];
  modelGroups: Array<{ label: string; options: DropdownOption[] }>;
  activeModelId: string | null;
  effortLevels: string[] | null;
  activeEffort: string | null;
  meter: ContextMeter | null;
  /** Fallback readout when no ceiling resolves: the last usage box's text. */
  lastUsageText: string;
  skills: Array<{ name: string; description: string }>;
  files: string[];
  contextView: ContextView | null;
  contextDetail: ContextItemDetail | null;
  omView: OmView | undefined;
  pendingApproval: AgentSnapshot["pendingApproval"];
}

/**
 * Reads one tab's agent store into the values both agent surfaces render.
 * Subscribes with `useSyncExternalStore`, so a store mutation repaints both.
 */
export function useAgentSurfaceState(
  store: AgentSessionModule,
): AgentSurfaceState {
  const command = store.command;
  const snapshot = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
  const state = snapshot.state;

  const configured =
    typeof state["provider"] === "string" &&
    (state["provider"] as string).length > 0;
  const provider = String(state["provider"] ?? "");
  const model = String(state["model"] ?? "");
  const sessionId =
    typeof state["sessionId"] === "string"
      ? (state["sessionId"] as string)
      : "";

  const gitBranch =
    typeof state["branch"] === "string" ? (state["branch"] as string) : "";
  const extensions = Array.isArray(state["extensions"])
    ? (state["extensions"] as unknown[]).filter(
        (name): name is string => typeof name === "string" && name.length > 0,
      )
    : [];

  const mcpServers = useMemo(() => {
    const stateList = Array.isArray(state["mcpServers"])
      ? (state["mcpServers"] as Array<{
          serverId?: unknown;
          connected?: unknown;
          tools?: unknown;
          error?: unknown;
        }>)
      : [];
    return stateList
      .filter(
        (server) =>
          typeof server.serverId === "string" && server.serverId.length > 0,
      )
      .map((server) => ({
        serverId: server.serverId as string,
        connected: server.connected === true,
        tools:
          typeof server.tools === "number" &&
          Number.isInteger(server.tools) &&
          server.tools >= 0
            ? server.tools
            : 0,
        error: typeof server.error === "string" ? server.error : "",
      }));
  }, [state]);
  const connectedMcpServers = mcpServers
    .filter((server) => server.connected)
    .map((server) => server.serverId);

  // Effort levels resolve from the models inventory (per-model
  // thinkingLevels) the picker already holds — present from session start,
  // updates on model switch. The old STATE `effortLevels` key only existed
  // after the first prompt, which kept the dropdown hidden until then.
  const effortLevels = useMemo(() => {
    const models = Array.isArray(state["models"])
      ? (state["models"] as ModelInfo[])
      : [];
    const active = models.find(
      (candidate) =>
        candidate.provider === provider && candidate.model === model,
    );
    const levels = Array.isArray(active?.thinkingLevels)
      ? (active?.thinkingLevels as unknown[]).filter(
          (level): level is string =>
            typeof level === "string" && level.length > 0,
        )
      : [];
    return levels.length > 0 ? levels : null;
  }, [state, provider, model]);
  const activeEffort =
    typeof state["effort"] === "string" ? (state["effort"] as string) : null;

  // The two state slices are read into locals so the memo's deps are simple
  // values (a nested index expression defeats the hook's static check).
  const stateModels = state["models"];
  const stateProviders = state["providers"];
  const modelGroups = useMemo(
    () =>
      groupModelsByProvider(
        Array.isArray(stateModels) ? (stateModels as ModelInfo[]) : [],
        Array.isArray(stateProviders) ? (stateProviders as ProviderInfo[]) : [],
      ),
    [stateModels, stateProviders],
  );
  const activeModelId = provider && model ? `model:${provider}/${model}` : null;

  // Counter numerator (provider usage) vs the registry ceiling.
  const contextTokens =
    typeof state["contextTokens"] === "number"
      ? (state["contextTokens"] as number)
      : null;
  const contextWindow = useMemo(() => {
    const models = Array.isArray(state["models"])
      ? (state["models"] as ModelInfo[])
      : [];
    const active = models.find(
      (candidate) =>
        candidate.provider === provider && candidate.model === model,
    );
    return typeof active?.contextWindow === "number" && active.contextWindow > 0
      ? active.contextWindow
      : null;
  }, [state, provider, model]);
  // Usage unreported (no provider turn yet, or a provider that never
  // reports): estimate occupancy from transcript size (plan 117).
  const meterTokens =
    contextTokens ?? estimateContextTokens(snapshot.messages, model);
  // Theme thresholds only: warning above 60%, error above 80%.
  const meter = useMemo<ContextMeter | null>(() => {
    if (contextWindow === null) return null;
    const ratio = meterTokens / contextWindow;
    return {
      text: `${compactTokens(meterTokens)}/${compactTokens(contextWindow)}`,
      tone: ratio > 0.8 ? "error" : ratio > 0.6 ? "warning" : "normal",
      percent: Math.round(ratio * 100),
    };
  }, [meterTokens, contextWindow]);
  const lastUsage = useMemo(() => {
    for (let index = snapshot.messages.length - 1; index >= 0; index -= 1) {
      const message = snapshot.messages[index];
      if (
        message &&
        (message as { metadata?: { clayKind?: string } }).metadata?.clayKind ===
          "usage"
      ) {
        return typeof message.content === "string" ? message.content : "";
      }
    }
    return "";
  }, [snapshot.messages]);
  const lastUsageText = meter ? "" : lastUsage;

  const transcript = useMemo<TranscriptBox[]>(() => {
    const boxes: TranscriptBox[] = [];
    for (const message of snapshot.messages) {
      const kind = kindOf(message);
      if (!kind) continue;
      const content =
        typeof message.content === "string" ? message.content : "";
      const meta =
        kind === "tool" || kind === "skill" ? toolMeta(message) : null;
      const agent = agentOf(message);
      boxes.push({
        id: message.id,
        kind,
        ...(agent ? { agent } : {}),
        label:
          kind === "usage"
            ? "usage"
            : kind === "error"
              ? "error"
              : kind === "reasoning"
                ? "thinking"
                : kind === "skill"
                  ? meta?.skillName || "skill"
                  : kind === "tool"
                    ? meta?.toolName || "tool"
                    : kind,
        content,
        ...(meta ? { toolName: meta.toolName } : {}),
      });
    }
    return boxes;
  }, [snapshot.messages]);

  // Catalog skills (disk-discovered + registered): the pinned skills card and
  // the @-mention dropdown's skill section.
  const skills = useMemo(() => {
    const stateList = Array.isArray(state["skills"])
      ? (state["skills"] as Array<{ name?: unknown; description?: unknown }>)
      : [];
    return stateList
      .filter(
        (skill) => typeof skill.name === "string" && skill.name.length > 0,
      )
      .map((skill) => ({
        name: skill.name as string,
        description:
          typeof skill.description === "string" ? skill.description : "",
      }));
  }, [state]);

  // Plan 117 @-mentions: the workspace file cache the composer's merged
  // dropdown consumes (composer owns the token parse; the listing is the
  // bounded server walk).
  // ponytail: no display gate — a session switch refetches on the first @;
  // the bounded stale list renders for one RPC round-trip at most.
  const files = Array.isArray(state["workspaceFiles"])
    ? (state["workspaceFiles"] as unknown[]).filter(
        (file): file is string => typeof file === "string",
      )
    : [];

  // Plan 118 task 36: the Files tab's one data source — the session's file
  // records, projected from the transcript's tool rows.
  const sessionFileList = useMemo(
    () => sessionFiles(snapshot.messages),
    [snapshot.messages],
  );

  return {
    snapshot,
    command,
    transcript,
    sessionFileList,
    configured,
    streaming: snapshot.status.streaming,
    provider,
    model,
    sessionId,
    gitBranch,
    extensions,
    mcpServers,
    connectedMcpServers,
    modelGroups,
    activeModelId,
    effortLevels,
    activeEffort,
    meter,
    lastUsageText,
    skills,
    files,
    contextView: isContextView(state["contextView"])
      ? (state["contextView"] as ContextView)
      : null,
    contextDetail: isContextItemDetail(state["contextItemDetail"])
      ? (state["contextItemDetail"] as ContextItemDetail)
      : null,
    omView: state["omView"] as OmView | undefined,
    pendingApproval: snapshot.pendingApproval,
  };
}
