// Coding Agent split surface (plan 108 task 8) — binding-spec presentation
// for the bundled `@clay/coding-agent` package.
//
// Provenance-exact host rendering, mirroring the SettingsPanel
// precedent: static copy comes from the package's declared component tree;
// every dynamic value rides the ONE core-owned AG-UI stream (the tab's own
// agent store — plan 119 SC-6); every interaction emits declared inert
// command intents through that tab's connection. Third-party replacements render through the unchanged
// generic SDUI renderer (PaneTree), never this module.
//
// Layout: 50/50 vertical split (react-resizable-panels, ratio-clamped,
// keyboard-operable separator). Left: transcript + composer + status row +
// extension strip. Right: Files / Memory / Context tabs, plus the full
// content of a selected truncated transcript box.
//
// Effort control (plan 109 I4): the daemon owns the portable thinking
// level — STATE carries effortLevels (declared) + effort (active); the
// panel offers a cataloged ClayDropdown and a manifest-bound cycle chord
// (default Shift+Tab) and sends the level with each prompt.

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { ClayDropdown, ClayIconButton, ClayText } from "../components";
import type { DropdownOption } from "../components";
import {
  agentCommandPayload,
  createAgentSession,
  type AgentSessionModule,
} from "../agent/state";
import { ApprovalStrip } from "./ApprovalStrip";
import { Composer, type EffortChord } from "./Composer";
import { InspectorTabs } from "./InspectorTabs";
import {
  isContextItemDetail,
  isContextView,
  type ContextItemDetail,
  type ContextView,
} from "./ContextTab";
import type { OmView } from "./MemoryTab";
import { TranscriptList } from "./TranscriptList";
import {
  agentLabel,
  agentOf,
  kindOf,
  toolMeta,
  type TranscriptBox,
} from "./transcript-model";
import { sduiActionPayload } from "../sdui/actions";
import type { PackageComponentNode, PackageSurface } from "../sdui/types";
import { recipeAttributes } from "../components/recipe-attributes";
import { agentInspector } from "../shell/layout-state";
import type { DocumentSession } from "../editor/sync/session";
import { sessionFiles } from "../agent/session-files";
import {
  launcherEntriesFrom,
  type LauncherAgentEntry,
} from "../launcher/LauncherPanel";

import styles from "./coding-agent.module.css";

/** Slash surface, mirroring the bundled package's daemon registrations
 *  plus the core built-ins (/model opens the daemon model picker). */
/**
 * Plan 109 R1: the composer completion's single source is the
 * daemon-registered command list that rides session STATE
 * (`snapshot.state["commands"]`, plan-108 registry — no hand-synced
 * frontend constant). Only the client-intercept built-ins — routed
 * beside their intercepts in `submit`, never daemon-dispatched — are
 * declared here.
 */
const CLIENT_SLASH_COMMANDS: ReadonlyArray<{
  name: string;
  description: string;
}> = [
  {
    name: "/model",
    description: "Choose a model from all configured providers.",
  },
  {
    name: "/resume",
    description: "Resume a session from this workspace.",
  },
];

interface ModelInfo {
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

interface ProviderInfo {
  id?: unknown;
  configured?: unknown;
}

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

function findNode(
  node: PackageComponentNode,
  id: string,
): PackageComponentNode | null {
  if (node.id === id) return node;
  for (const child of node.children ?? []) {
    const found = findNode(child, id);
    if (found) return found;
  }
  return null;
}

export type { EffortChord };

export interface CodingAgentPanelProps {
  surface: PackageSurface;
  /** Current runtime package UI version; validates every intent. */
  uiVersion: number;
  /** Workspace path for the status row. */
  workspaceRoot: string;
  /** The pane's document session: the Settings tab lists the agent's
   *  delivered files through it. The Files tab no longer hosts an editor
   *  (plan 118 task 36 — it is the session's file history). */
  session?: DocumentSession | null;
  /** The tab's agent session store (plan 119 SC-6) once its runtime has one.
   *  The panel creates it on first mount and adopts it through `onAgentStore`;
   *  a standalone mount (fixtures, tests) simply keeps its own. */
  agent?: AgentSessionModule | null;
  /** Connection id of the tab the panel is rendered for (the store's delivery
   *  filter); `null` accepts every delivery (standalone mounts). */
  agentClientId?: number | null;
  /** Adopt the created store into the tab runtime (lifetime owner). */
  onAgentStore?: ((store: AgentSessionModule) => void) | null;
  /** The tab's own sender — construction-scoped into the store's agent so its
   *  prompts/cancel/steer ride this tab's connection. */
  send?: (payload: string) => Promise<void>;
  /** Effective effort-cycle chord from the behavior manifest
   *  (`coding-agent.clientCycleEffort`); default `Shift+Tab`. */
  effortChord?: EffortChord | null;
  /** The tab strip marks this tab's agent while it works (plan 118 task 33). */
  onBusyChange?: (busy: boolean) => void;
  /** Plan 118 task 35: the agent the tab runs (`null` = none picked yet).
   *  The header's picker reads it; the session's own STATE (`agent`) wins
   *  when the daemon reports one, so a resumed session shows its producer. */
  agentType?: string | null;
  /** Switch the tab's agent (tab chrome: the shell sends the server-validated
   *  command). `null` detaches the agent. */
  onPickAgent?: ((agent: string | null) => void) | null;
  /** Plan 118 task 36: open a path in the tab's *workspace* view — the Files
   *  tab's row action (the shell switches the view and opens the document). */
  onOpenInWorkspace?: ((path: string) => void) | null;
}

export function CodingAgentPanel({
  surface,
  uiVersion,
  workspaceRoot,
  session = null,
  agent = null,
  agentClientId = null,
  onAgentStore = null,
  send,
  effortChord = null,
  onBusyChange,
  agentType = null,
  onPickAgent = null,
  onOpenInWorkspace = null,
}: CodingAgentPanelProps) {
  // Stable across renders so the inspector's rows do not re-bind; a panel
  // without a shell (fixtures) keeps the rows but they lead nowhere.
  const openInWorkspace = useCallback(
    (path: string) => onOpenInWorkspace?.(path),
    [onOpenInWorkspace],
  );
  // The tab's store lives as long as the tab: the view creates it on first
  // mount and the tab runtime adopts it (so a view switch or a hidden tab
  // keeps its transcript). A standalone mount (fixtures, tests) keeps its own.
  const created = useRef<AgentSessionModule | null>(null);
  const store =
    agent ??
    (created.current ??= createAgentSession({
      send: send ?? undefined,
      clientId: agentClientId,
    }));
  useEffect(() => {
    onAgentStore?.(store);
  }, [onAgentStore, store]);
  // Stable command lane for the inspector tabs: the tab's own connection, not
  // the process-wide active client (which is the last-activated tab).
  const command = store.command;
  const snapshot = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );

  useEffect(() => {
    store.agent.setUiVersion(uiVersion);
  }, [store, uiVersion]);
  useEffect(() => {
    store.command("listSessions");
    // Plan 117 follow-up: ask for the tab's STATE at mount. Nothing else
    // emits a snapshot before the first prompt, so without this the status
    // row showed `git —` and the skills/MCP cards stayed empty until the
    // first message (the daemon environment needs no session). The same
    // answer carries the tab's session binding (plan 119 SC-6), which is
    // what the store filters the relay with.
    store.requestBinding();
  }, [store]);

  const declared = surface.component;
  const transcriptTitle = useMemo(
    () => findNode(declared, "coding-agent.transcriptTitle")?.text ?? "Agent",
    [declared],
  );

  // Plan 118 task 35: the configured agent types come from the same
  // server-resolved listing the launcher renders (one directory scan, no
  // package load). Fetched once here so the header's picker can offer them.
  const [agentEntries, setAgentEntries] = useState<LauncherAgentEntry[]>([]);
  useEffect(() => {
    if (!session) return;
    const apply = (event: { kind?: string; data?: unknown }) => {
      if (event.kind !== "launcherEntries") return;
      const entries = launcherEntriesFrom(event.data);
      if (entries) setAgentEntries(entries.agents);
    };
    for (const envelope of session.featureSnapshot())
      apply(envelope.data as { kind?: string; data?: unknown });
    const unsubscribe = session.subscribeFeatures((envelope) => {
      apply(envelope.data as { kind?: string; data?: unknown });
    });
    session.listLauncherEntries();
    return unsubscribe;
  }, [session]);

  // The session's own agent (server-reported) wins over the tab's identity:
  // resuming another agent's session shows the agent that produced it.
  const sessionAgent =
    typeof snapshot.state["agent"] === "string"
      ? (snapshot.state["agent"] as string)
      : null;
  const currentAgent = sessionAgent ?? agentType;
  // Plan 118 task 35: a session that already runs an agent (a resume) adopts
  // it into an agent-less tab, so the tab's record and the view agree. Only
  // from the *unknown* side: a pick the user just made is never reverted while
  // the session catches up.
  useEffect(() => {
    if (!onPickAgent || agentType !== null || !sessionAgent) return;
    onPickAgent(sessionAgent);
  }, [agentType, onPickAgent, sessionAgent]);

  // The menu never lists an agent the server did not list; a tab whose agent
  // is gone (deleted folder) still shows its own row so the state is visible.
  const agentOptions = useMemo(() => {
    const rows = agentEntries.map((entry) => ({
      id: entry.name,
      label: entry.label,
    }));
    if (currentAgent && !rows.some((row) => row.id === currentAgent)) {
      rows.unshift({ id: currentAgent, label: agentLabel(currentAgent) });
    }
    // No agent picked yet and nothing listed: the surface's own title is the
    // honest trigger text (the fallback the picker shows as its value).
    if (rows.length === 0) rows.push({ id: "", label: transcriptTitle });
    return rows;
  }, [agentEntries, currentAgent, transcriptTitle]);
  const skillCount = useMemo(
    () =>
      agentEntries.find((entry) => entry.name === currentAgent)?.skillCount ??
      null,
    [agentEntries, currentAgent],
  );

  const [selectedId, setSelectedId] = useState("");
  // Plan 109 I4: the pending effort level the next prompt carries (cycles
  // via the manifest-bound chord, settable via the dropdown); the session's
  // active level comes from STATE. Levels: declared levels for the model,
  // empty/absent → no control, chord is a no-op.
  const [pendingEffort, setPendingEffort] = useState<string | null>(null);
  // Session id of the bound agent run (STATE-carried; read early because
  // the @-mention fetch and file-cache gate key off it).
  const sessionId =
    typeof snapshot.state["sessionId"] === "string"
      ? (snapshot.state["sessionId"] as string)
      : "";
  // Plan 109 I7: which detail tab is open — the Context fetch fires on tab
  // open and re-fires when the transcript length changes (event-driven
  // invalidation: every run finish / compaction / entry append lands a
  // transcript entry and bumps the server's context version).
  const [activeTab, setActiveTab] = useState("files");
  // The inspector is the agent view's right column (DESIGN.md §12) and its
  // visibility is the tab's own layout state (plan 118 task E2): one owner
  // (`agentInspector`), persisted per tab with the rail, so two tabs can keep
  // different shapes across a restart.
  const inspectorVisible = useSyncExternalStore(
    agentInspector.subscribe,
    agentInspector.isVisible,
  );
  const setInspectorVisible = (next: boolean) =>
    agentInspector.setVisible(next);
  // Plan 109 I10: the tab a card selection came from, restored on
  // deselect (Back or re-click).
  const previousTabRef = useRef("files");
  const selectCard = useCallback(
    (id: string) => {
      const deselect = !id || id === selectedId;
      setSelectedId(deselect ? "" : id);
      if (deselect) {
        setActiveTab(previousTabRef.current);
      } else {
        if (activeTab !== "session-info") previousTabRef.current = activeTab;
        setActiveTab("session-info");
      }
    },
    [selectedId, activeTab],
  );

  const configured =
    typeof snapshot.state["provider"] === "string" &&
    (snapshot.state["provider"] as string).length > 0;
  const streaming = snapshot.status.streaming;
  const provider = String(snapshot.state["provider"] ?? "");
  const model = String(snapshot.state["model"] ?? "");
  // Plan 109 R2/R3: git branch + active extensions ride session state.
  const gitBranch =
    typeof snapshot.state["branch"] === "string"
      ? (snapshot.state["branch"] as string)
      : "";
  const extensions = Array.isArray(snapshot.state["extensions"])
    ? (snapshot.state["extensions"] as unknown[]).filter(
        (name): name is string => typeof name === "string" && name.length > 0,
      )
    : [];
  // Per-server MCP connect outcomes (plan 117): {serverId, connected,
  // tools, error} from the daemon's environment. The strip shows connected
  // names only; the pinned card + composer section read the full list.
  const mcpServers = useMemo(() => {
    const stateList = Array.isArray(snapshot.state["mcpServers"])
      ? (snapshot.state["mcpServers"] as Array<{
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
  }, [snapshot.state]);
  const connectedMcpServers = mcpServers
    .filter((server) => server.connected)
    .map((server) => server.serverId);
  // Plan 117: effort levels resolve from the models inventory the picker
  // already holds (per-model thinkingLevels) — present from session start,
  // updates on model switch. The old STATE `effortLevels` key only existed
  // after the first prompt (book cache filled at session creation), which
  // kept the dropdown hidden until then.
  const effortLevels = useMemo(() => {
    const models = Array.isArray(snapshot.state["models"])
      ? (snapshot.state["models"] as ModelInfo[])
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
  }, [snapshot.state, provider, model]);
  const activeEffort =
    typeof snapshot.state["effort"] === "string"
      ? (snapshot.state["effort"] as string)
      : null;
  const shownEffort = pendingEffort ?? activeEffort;
  // Model dropdown (plan 109 I3): same grouped configured-models inventory
  // the Command Centre picker shows; ids match its `model:provider/name`.
  // The two state slices are read into locals so the memo's deps are simple
  // values (a nested index expression defeats the hook's static check).
  const stateModels = snapshot.state["models"];
  const stateProviders = snapshot.state["providers"];
  const modelGroups = useMemo(
    () =>
      groupModelsByProvider(
        Array.isArray(stateModels) ? (stateModels as ModelInfo[]) : [],
        Array.isArray(stateProviders) ? (stateProviders as ProviderInfo[]) : [],
      ),
    [stateModels, stateProviders],
  );
  const activeModelId = provider && model ? `model:${provider}/${model}` : null;
  const onModelSelect = useCallback(
    (id: string) => {
      // Same selection payload the Command Centre picker emits; the server
      // applies it through select_picker with this tab (I2 per-workspace
      // persistence), then the book snapshot refreshes the active pair.
      store.command({ select: { kind: "model", id } });
    },
    [store],
  );
  // Context-used-vs-window (plan 108 task 9): numerator rides every snapshot
  // STATE event, denominator the model registry's contextWindow. Both are
  // bounded counters/ints — counters only, never transcript content.
  // Plan 109 I8: the Observational Memory tab's server view (worker
  // selection + bounded activity log), cached in agent state.
  const omView = snapshot.state["omView"] as OmView | undefined;
  const contextTokens =
    typeof snapshot.state["contextTokens"] === "number"
      ? (snapshot.state["contextTokens"] as number)
      : null;
  // Ceiling (plan 117 token meter): the active model's registry context
  // window, resolved from the models inventory the picker already holds —
  // present from session start, updates on model switch.
  const contextWindow = useMemo(() => {
    const models = Array.isArray(snapshot.state["models"])
      ? (snapshot.state["models"] as ModelInfo[])
      : [];
    const active = models.find(
      (candidate) =>
        candidate.provider === provider && candidate.model === model,
    );
    return typeof active?.contextWindow === "number" && active.contextWindow > 0
      ? active.contextWindow
      : null;
  }, [snapshot.state, provider, model]);
  // Usage unreported (no provider turn yet, or a provider that never
  // reports): estimate occupancy from transcript size (plan 117).
  const meterTokens =
    contextTokens ?? estimateContextTokens(snapshot.messages, model);

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

  // Session token meter (plan 117): compact occupancy vs the model's
  // ceiling — `220k/270k`. Theme thresholds only: warning above 60%,
  // error above 80%, normal otherwise.
  const meter = useMemo(() => {
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

  const selected = transcript.find((box) => box.id === selectedId) ?? null;

  // Plan 109 R1: daemon-registered commands (state) + client built-ins.
  const slashCommands = useMemo(() => {
    const stateList = Array.isArray(snapshot.state["commands"])
      ? (snapshot.state["commands"] as Array<{
          name?: unknown;
          description?: unknown;
        }>)
      : [];
    const daemon = stateList
      .filter(
        (command) =>
          typeof command.name === "string" && command.name.startsWith("/"),
      )
      .map((command) => ({
        name: command.name as string,
        description:
          typeof command.description === "string" ? command.description : "",
      }));
    const merged = new Map<string, { name: string; description: string }>();
    for (const command of [...CLIENT_SLASH_COMMANDS, ...daemon]) {
      if (!merged.has(command.name)) merged.set(command.name, command);
    }
    return [...merged.values()];
  }, [snapshot.state]);

  // Catalog skills (disk-discovered + registered): pinned skills card on
  // the transcript. Bounded parse, same discipline as the command list.
  const skills = useMemo(() => {
    const stateList = Array.isArray(snapshot.state["skills"])
      ? (snapshot.state["skills"] as Array<{
          name?: unknown;
          description?: unknown;
        }>)
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
  }, [snapshot.state]);

  // Plan 117 @-mentions: the workspace file cache the composer's merged
  // dropdown consumes (the composer owns the token parse and filters
  // client-side; the listing itself is the bounded server walk).
  const filesRaw = Array.isArray(snapshot.state["workspaceFiles"])
    ? (snapshot.state["workspaceFiles"] as unknown[]).filter(
        (file): file is string => typeof file === "string",
      )
    : [];
  // ponytail: no display gate — a session switch refetches on the first @;
  // the bounded stale list renders for one RPC round-trip at most.
  const files = filesRaw;

  const sendIntent = useCallback(
    (commandId: string) => {
      void store
        .sendPayload(
          sduiActionPayload(uiVersion, {
            commandId,
            source: { button: { nodeId: 1 } },
            arguments: [],
          }),
        )
        .catch(() => {
          // Disconnect flow owns recovery; nothing to do here.
        });
    },
    [store, uiVersion],
  );

  // The panel owns the send authority: client built-ins, steer-vs-prompt,
  // and the run lifecycle. The composer owns the text, so this answers
  // whether the input was consumed (`false` = ignored, keep the draft).
  const submit = useCallback(
    (text: string): boolean => {
      const trimmed = text.trim();
      if (!trimmed) return false;
      // Core built-in (plan 109 I3): /model opens the daemon model picker
      // (Command Centre flow — configured models grouped by provider;
      // selection applies through select_picker with the bound tab, writing
      // the per-workspace selection per I2). The package deliberately does
      // not re-register it — see load.js SLASH_COMMANDS.
      if (trimmed === "/model") {
        sendIntent("agent.clientOpenModelPicker");
        return true;
      }
      // Core built-in (plan 109 I9): /resume opens the workspace-scoped
      // session picker (Command Centre flow — server-scoped to the tab's
      // workspace; selection resumes through the existing resume path,
      // restoring transcript, live context, and the session's persisted
      // provider/model into the book).
      if (trimmed === "/resume") {
        sendIntent("agent.clientOpenSessionPicker");
        return true;
      }
      if (!configured) return false;
      setSelectedId("");
      if (streaming) {
        // Mid-run queue (pi-parity steer): the daemon folds the message into
        // the active run; the transcript keeps streaming. The pending effort
        // level stays pending — it applies to the next fresh prompt.
        store.agent.steer(trimmed);
        return true;
      }
      // Plan 109 I4: the pending effort rides the prompt (daemon fail-closes
      // invalid strings at its boundary and applies the model-aware Prism
      // 0.5.0 mapping per run). Once sent, it is the session's active level.
      const effortForRun = pendingEffort ?? undefined;
      store.agent.sendPrompt(trimmed, effortForRun);
      // Plan 109 I5: the store flushes the deferred server snapshot once the
      // run pipeline is quiescent, so the server transcript wins at the
      // boundary.
      void store.runTurn().catch(() => {
        // Failures already landed as RUN_ERROR status.
      });
      return true;
    },
    [configured, sendIntent, store, streaming, pendingEffort],
  );

  // Stable action lanes for the composer's own buttons (Stop / Close).
  const stopRun = useCallback(() => store.agent.abortRun(), [store]);
  const closeSurface = useCallback(
    () => sendIntent("coding-agent.close"),
    [sendIntent],
  );

  // Tool approval (durable runs): Allow/Deny strip for suspended runs.
  const pendingApproval = snapshot.pendingApproval;
  const resolveApproval = useCallback(
    (outcome: "allow_once" | "reject_once") => {
      if (!pendingApproval) return;
      void store
        .sendPayload(
          agentCommandPayload({
            runResume: {
              sessionId: pendingApproval.sessionId,
              runId: pendingApproval.runId,
              decisionJson: JSON.stringify({
                decisions: [{ approvalId: pendingApproval.requestId, outcome }],
              }),
            },
          }),
        )
        .catch(() => {
          // Failures land as diagnostics; the strip keeps its last state.
        });
      // Optimistic clear; the resumed run re-announces via RUN_STARTED.
      store.clearPendingApproval();
    },
    [pendingApproval, store],
  );
  // Plan 118 task 36: the Files tab's one data source — the session's file
  // records, projected from the transcript's tool rows.
  const sessionFileList = useMemo(
    () => sessionFiles(snapshot.messages),
    [snapshot.messages],
  );
  // Fallback readout when no model ceiling resolves (inventory absent):
  // the raw usage box text, else nothing.
  const lastUsageText = meter ? "" : lastUsage;
  // Plan 109 I7: the Context tab renders the server-authoritative view
  // (daemon `session.context`); the client-derived counters are gone.
  const contextView = isContextView(snapshot.state["contextView"])
    ? (snapshot.state["contextView"] as ContextView)
    : null;

  // Plan 118 task 33: report the run state up to the shell so the tab strip's
  // agent marker can pulse. The panel is the only writer of this fact.
  useEffect(() => {
    onBusyChange?.(streaming === true);
  }, [onBusyChange, streaming]);

  // State strip (DESIGN.md §12): the dot, the session's own status line, and a
  // note that is a fact about the run — never invented progress.
  const stateTone = !configured
    ? "muted"
    : streaming
      ? "busy"
      : snapshot.status.status === null
        ? "success"
        : "warning";
  const stateText = streaming ? "Working" : (snapshot.status.status ?? "Ready");
  const stateNote = pendingApproval
    ? "tool call needs approval"
    : streaming
      ? "steer or stop from the composer"
      : "agent idle";

  return (
    <section
      className={styles.surface}
      aria-label="Coding Agent"
      data-coding-agent-surface
    >
      <div
        className={styles.agent}
        data-inspector={inspectorVisible ? "expanded" : "collapsed"}
      >
        <main className={styles.agentColumn}>
          <header className={styles.agentHead}>
            {/* Plan 118 task 35: the title *is* the agent-type picker — one
             *  tab holds one agent, and this is where it changes. The menu
             *  lists the configured types (the same server enumeration the
             *  launcher shows), marks the current one, and names where more
             *  come from. */}
            <span className={styles.agentPick} data-agent-pick>
              <ClayDropdown
                label="Agent type"
                triggerFamily="agentPicker"
                selectedHint="current"
                options={agentOptions}
                selectedId={currentAgent}
                disabled={!onPickAgent || agentOptions.length === 0}
                onSelect={(id) => {
                  if (!onPickAgent || id === currentAgent) return;
                  // Switching resets the effort control: the level belonged
                  // to the previous agent's model (the model itself follows
                  // the new agent's own default from the server).
                  setPendingEffort(null);
                  onPickAgent(id);
                }}
                footer={
                  currentAgent
                    ? `${currentAgent} · ${
                        skillCount === null
                          ? "skills unknown"
                          : `${skillCount} skill${skillCount === 1 ? "" : "s"}`
                      } · from ~/.clay/agents/`
                    : "Agent types are folders under ~/.clay/agents/"
                }
              />
            </span>
            <span className={styles.agentControls}>
              {configured && modelGroups.length > 0 ? (
                <ClayDropdown
                  label="Model"
                  options={[]}
                  groups={modelGroups}
                  selectedId={activeModelId}
                  onSelect={onModelSelect}
                />
              ) : (
                <ClayText variant="caption" muted>
                  {configured
                    ? provider && model
                      ? `${provider}/${model}`
                      : String(snapshot.state["profile"] ?? "coding")
                    : "Configure a provider to start."}
                </ClayText>
              )}
              {meter ? (
                <span className={styles.meter}>
                  <span className={styles.meterTrack} aria-hidden>
                    <span
                      className={styles.meterFill}
                      style={{ width: `${meter.percent}%` }}
                    />
                  </span>
                  <span
                    className={
                      meter.tone === "error"
                        ? styles.meterError
                        : meter.tone === "warning"
                          ? styles.meterWarning
                          : styles.meterValue
                    }
                    title={`Context usage: ${meter.percent}% of ceiling`}
                  >
                    {meter.text}
                  </span>
                </span>
              ) : (
                lastUsageText.length > 0 && (
                  <span className={styles.meterValue}>{lastUsageText}</span>
                )
              )}
              {/* Plan 109 I4: cataloged ClayDropdown for declared levels;
               * the selected level rides the next prompt. */}
              {effortLevels && effortLevels.length > 0 ? (
                <ClayDropdown
                  label="Effort"
                  options={effortLevels.map((level) => ({
                    id: level,
                    label: level,
                  }))}
                  selectedId={shownEffort}
                  onSelect={setPendingEffort}
                />
              ) : (
                activeEffort && (
                  <ClayText variant="caption" muted>
                    {`effort ${activeEffort}`}
                  </ClayText>
                )
              )}
              {!inspectorVisible && (
                <ClayIconButton
                  icon="preview.toggle"
                  label="Show inspector"
                  variant="muted"
                  onPress={() => setInspectorVisible(true)}
                />
              )}
            </span>
          </header>

          <TranscriptList
            boxes={transcript}
            selectedId={selectedId}
            onSelect={selectCard}
          />

          <div className={styles.stateStrip} role="status">
            <span
              className={styles.statusDot}
              data-tone={stateTone}
              aria-hidden
              {...recipeAttributes("statusDot", "root")}
            />
            <span className={styles.stateText}>{stateText}</span>
            <span className={styles.spacer} />
            <span className={styles.stateNote}>{stateNote}</span>
          </div>

          <footer className={styles.composerArea}>
            {pendingApproval && (
              <ApprovalStrip
                approval={pendingApproval}
                onResolve={resolveApproval}
              />
            )}
            <Composer
              configured={configured}
              streaming={streaming}
              sessionId={sessionId}
              command={command}
              slashCommands={slashCommands}
              skills={skills}
              files={files}
              effortLevels={effortLevels}
              shownEffort={shownEffort}
              onEffortChange={setPendingEffort}
              effortChord={effortChord}
              onStop={stopRun}
              onClose={closeSurface}
              onSubmit={submit}
            />
          </footer>

          <div
            className={styles.agentFoot}
            data-clay-ds="shell.footer"
            aria-label="Session environment"
          >
            <span>{workspaceRoot}</span>
            <span className={styles.footFaint}>·</span>
            <span>{`git ${gitBranch || "—"}`}</span>
            {extensions.length > 0 && (
              <>
                <span className={styles.footFaint}>·</span>
                <span>{`extensions ${extensions.join(", ")}`}</span>
              </>
            )}
            <span className={styles.spacer} />
            {/* One summary segment: the per-server detail is the Context
             *  tab's, and this line only says what is connected. */}
            <span data-tone={connectedMcpServers.length > 0 ? "ok" : "muted"}>
              {mcpServers.length > 0
                ? `MCP ${mcpServers
                    .map((server) =>
                      server.connected
                        ? `${server.serverId} · ${server.tools} ${
                            server.tools === 1 ? "tool" : "tools"
                          }`
                        : `${server.serverId} · hidden`,
                    )
                    .join(" · ")}`
                : "MCP none"}
            </span>
          </div>
        </main>

        <InspectorTabs
          activeTab={activeTab}
          onActivate={setActiveTab}
          onHide={() => setInspectorVisible(false)}
          files={sessionFileList}
          onOpenInWorkspace={openInWorkspace}
          sessionId={sessionId}
          transcriptLength={transcript.length}
          omView={omView}
          modelGroups={modelGroups}
          contextView={contextView}
          contextDetail={
            isContextItemDetail(snapshot.state["contextItemDetail"])
              ? (snapshot.state["contextItemDetail"] as ContextItemDetail)
              : null
          }
          skills={skills}
          servers={mcpServers}
          command={command}
          selected={selected}
          onBack={() => selectCard(selectedId)}
          session={session}
          sendIntent={sendIntent}
        />
      </div>
    </section>
  );
}
