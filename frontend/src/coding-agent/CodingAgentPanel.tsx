// Coding Agent split surface (plan 108 task 8) — binding-spec presentation
// for the bundled `@clay/coding-agent` package.
//
// Provenance-exact host rendering, mirroring the SettingsPanel
// precedent: static copy comes from the package's declared component tree;
// every dynamic value rides the ONE core-owned AG-UI stream (`agentSession`
// — the tab's daemon session); every interaction emits declared inert
// command intents. Third-party replacements render through the unchanged
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
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import {
  ClayBadge,
  ClayButton,
  ClayDropdown,
  ClayIcon,
  ClayIconButton,
  ClayKbd,
  ClayTabStrip,
  ClayText,
  ClayTextField,
} from "../components";
import type { DropdownOption } from "../components";
import { agentSession } from "../agent/state";
import { sendRequest } from "../bridge/client";
import { sduiActionPayload } from "../sdui/actions";
import type { PackageComponentNode, PackageSurface } from "../sdui/types";
import { recipeAttributes } from "../components/recipe-attributes";
import { agentInspector } from "../shell/layout-state";
import type { DocumentSession } from "../editor/sync/session";
import {
  SESSION_FILE_MARK,
  sessionFiles,
  splitSessionFilePath,
  type SessionFileRecord,
} from "../agent/session-files";
import {
  AgentSettingsPanel,
  type AgentSettingsFileInfo,
} from "../agent-settings/AgentSettingsPanel";
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

type TranscriptKind =
  "user" | "assistant" | "reasoning" | "usage" | "error" | "tool" | "skill";

interface TranscriptBox {
  id: string;
  kind: TranscriptKind;
  label: string;
  content: string;
  /** Tool rows only: the tool name for Context-tab category counts. */
  toolName?: string;
  /** Plan 118 task 35: the agent type that produced the turn, when the
   *  server stamped one (rows carry their producer's name across a switch). */
  agent?: string;
}

/** `reviewer` → `Reviewer` (the launcher's label rule, client side). */
function agentLabel(type: string): string {
  return type
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

/** Per-turn agent attribution (plan 118 task 35): read from the message
 *  metadata the server projects per transcript row. */
function agentOf(message: unknown): string | undefined {
  const agent =
    typeof message === "object" && message !== null
      ? (message as { metadata?: { agent?: unknown } }).metadata?.agent
      : undefined;
  return typeof agent === "string" && agent.length > 0 ? agent : undefined;
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

function kindOf(message: {
  role: string;
  content?: unknown;
  metadata?: { clayKind?: string };
}): TranscriptKind | null {
  if (message.metadata?.clayKind === "usage") return "usage";
  if (message.metadata?.clayKind === "error") return "error";
  if (message.role === "user") return "user";
  if (message.role === "reasoning") return "reasoning";
  if (message.role === "assistant") return "assistant";
  // Plan 109 I5: bounded tool rows ride the standard AG-UI tool role;
  // load_skill rows present as skill boxes.
  if (message.role === "tool") {
    return message.metadata?.clayKind === "skill" ? "skill" : "tool";
  }
  return null;
}

/** Tool-row metadata for box labels and Context-tab category counts. */
function toolMeta(message: unknown): { toolName: string; skillName: string } {
  const meta =
    typeof message === "object" && message !== null
      ? (message as { metadata?: { toolName?: unknown; skillName?: unknown } })
          .metadata
      : undefined;
  return {
    toolName: typeof meta?.toolName === "string" ? meta.toolName : "",
    skillName: typeof meta?.skillName === "string" ? meta.skillName : "",
  };
}

/**
 * The transcript is turns, not cards (DESIGN.md §12): one hairline-separated
 * block per entry with a turn head — role micro-label left, kind right — and
 * either prose (the conversation) or a uniform truncated tool box (machine
 * output). The AG-UI message carries no timestamp, so none is shown rather
 * than invented. Selecting a turn opens its full content in Session Info.
 */
const TURN_HEADS: Record<TranscriptKind, { role: string; note: string }> = {
  user: { role: "you", note: "prompt" },
  assistant: { role: "agent", note: "turn" },
  reasoning: { role: "thinking", note: "reasoning" },
  tool: { role: "tool", note: "call" },
  skill: { role: "skill", note: "skill" },
  usage: { role: "usage", note: "tokens" },
  error: { role: "error", note: "failed" },
};

/** Machine output is clamped to a fixed line count so box height never depends
 *  on content; the conversation reads in full. */
const BOXED_KINDS = new Set<TranscriptKind>([
  "tool",
  "skill",
  "usage",
  "error",
]);

const TranscriptTurn = memo(function TranscriptTurn({
  box,
  selected,
  showAgent,
  agentChangedFrom,
  onSelect,
}: {
  box: TranscriptBox;
  selected: boolean;
  /** More than one agent produced this transcript: label every turn with its
   *  own agent so a switch is readable turn by turn (plan 118 task 35). */
  showAgent: boolean;
  /** The agent the previous turn ran as, when it differs (the switch note). */
  agentChangedFrom?: string;
  onSelect: (id: string) => void;
}) {
  const head = TURN_HEADS[box.kind];
  // Tool/skill rows name the thing that ran where the generic kind would be.
  const note =
    box.kind === "tool" || box.kind === "skill" ? box.label : head.note;
  return (
    <button
      type="button"
      className={styles.turn}
      data-role={box.kind}
      data-selected={selected}
      data-agent={box.agent ?? undefined}
      aria-pressed={selected}
      aria-label={`${box.label} ${box.content}`}
      onClick={() => onSelect(selected ? "" : box.id)}
    >
      {agentChangedFrom && box.agent ? (
        <span className={styles.turnSwitch} data-turn-switch>
          {`Switched to ${agentLabel(box.agent)}`}
        </span>
      ) : null}
      <span className={styles.turnHead}>
        <span className={styles.turnRole}>
          {showAgent && box.agent ? agentLabel(box.agent) : head.role}
        </span>
        <span className={styles.spacer} />
        <span className={styles.turnNote}>{note}</span>
      </span>
      {BOXED_KINDS.has(box.kind) ? (
        <span className={styles.turnBox}>
          <span className={styles.turnBoxText}>{box.content}</span>
        </span>
      ) : (
        <span className={styles.turnBody}>{box.content}</span>
      )}
    </button>
  );
});

/**
 * Capability inventories — the session's skill catalog and its MCP servers.
 * Reference data, so it lives in the inspector's Context tab and never in the
 * transcript (DESIGN.md §12). Display-only: no restart/connect actions.
 */
const CapabilitySections = memo(function CapabilitySections({
  skills,
  servers,
}: {
  skills: ReadonlyArray<{ name: string; description: string }>;
  servers: ReadonlyArray<{
    serverId: string;
    connected: boolean;
    tools: number;
    error: string;
  }>;
}) {
  if (skills.length === 0 && servers.length === 0) return null;
  return (
    <>
      {skills.length > 0 && (
        <section className={styles.section} aria-label="Skills loaded">
          <div className={styles.sectionHead}>
            <span className={styles.sectionLabel}>Skills</span>
            <ClayBadge tone="muted">{skills.length}</ClayBadge>
          </div>
          <ul className={styles.skillRows}>
            {skills.map((skill) => (
              <li key={skill.name} className={styles.skillRow}>
                <span className={styles.rowMain}>
                  <span className={styles.rowName}>{skill.name}</span>
                  {skill.description.length > 0 && (
                    <span className={styles.rowDetail}>
                      {skill.description}
                    </span>
                  )}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {servers.length > 0 && (
        <section className={styles.section} aria-label="MCP servers connected">
          <div className={styles.sectionHead}>
            <span className={styles.sectionLabel}>MCP servers</span>
            <ClayBadge tone="muted">{servers.length}</ClayBadge>
          </div>
          <ul className={styles.skillRows}>
            {servers.map((server) => (
              <li key={server.serverId} className={styles.serverRow}>
                <span className={styles.rowName}>{server.serverId}</span>
                <span
                  className={styles.rowDetail}
                  data-tone={server.connected ? "ok" : "err"}
                >
                  {server.connected
                    ? `${server.tools} ${server.tools === 1 ? "tool" : "tools"}`
                    : server.error.length > 0
                      ? `hidden: ${server.error}`
                      : "hidden"}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
});

/** The first-run state: the keyboard path, and nothing pretending to be a
 *  conversation (DESIGN.md §12 — real values only). */
const EmptyTranscript = memo(function EmptyTranscript() {
  return (
    <div className={styles.empty} role="status">
      <p className={styles.emptyTitle}>No conversation yet.</p>
      <p className={styles.emptyText}>
        Ask a question, or drive the session from the keyboard. Skills and tools
        live in the Context tab until they are needed.
      </p>
      <div className={styles.keyHints}>
        <span className={styles.keyHint}>
          <ClayKbd>/</ClayKbd> commands
        </span>
        <span className={styles.keyHint}>
          <ClayKbd>@</ClayKbd> files &amp; skills
        </span>
        <span className={styles.keyHint}>
          <ClayKbd>⇧</ClayKbd>
          <ClayKbd>↵</ClayKbd> newline
        </span>
      </div>
    </div>
  );
});

/** Plan 109 I4: effective effort-cycle chord (key + modifiers), read from
 *  the behavior manifest by the shell so `bindKey` overrides apply. */
export interface EffortChord {
  shift: boolean;
  ctrl: boolean;
  alt: boolean;
  meta: boolean;
  key: string;
}

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
  /** Intent sender for the agent session relay. */
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
  const snapshot = useSyncExternalStore(
    agentSession.subscribe,
    agentSession.getSnapshot,
    agentSession.getSnapshot,
  );

  useEffect(() => {
    agentSession.agent.setUiVersion(uiVersion);
  }, [uiVersion]);
  useEffect(() => {
    if (send) agentSession.agent.setSender(send);
  }, [send]);
  useEffect(() => agentSession.start(), []);
  useEffect(() => {
    void sendRequest(agentCommandPayload("listSessions"));
    // Plan 117 follow-up: ask for the tab's STATE at mount. Nothing else
    // emits a snapshot before the first prompt, so without this the status
    // row showed `git —` and the skills/MCP cards stayed empty until the
    // first message (the daemon environment needs no session).
    void sendRequest(agentCommandPayload("tabState"));
  }, []);

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

  const [draft, setDraft] = useState("");
  const [selectedId, setSelectedId] = useState("");
  // Plan 109 I4: the pending effort level the next prompt carries (cycles
  // via the manifest-bound chord, settable via the dropdown); the session's
  // active level comes from STATE. Levels: declared levels for the model,
  // empty/absent → no control, chord is a no-op.
  const [pendingEffort, setPendingEffort] = useState<string | null>(null);
  const [completionIndex, setCompletionIndex] = useState(0);
  const [completionDismissed, setCompletionDismissed] = useState(false);
  // Session id of the bound agent run (STATE-carried; read early because
  // the @-mention fetch and file-cache gate key off it).
  const sessionId =
    typeof snapshot.state["sessionId"] === "string"
      ? (snapshot.state["sessionId"] as string)
      : "";
  // Plan 117 @-mentions: workspace file cache for the dropdown, one fetch
  // per session (bounded server walk; client-side filtering). The parse is
  // gated on the fetch guard so a previous session's list never renders.
  const filesFetchedFor = useRef("");
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
  const modelGroups = useMemo(
    () =>
      groupModelsByProvider(
        Array.isArray(snapshot.state["models"])
          ? (snapshot.state["models"] as ModelInfo[])
          : [],
        Array.isArray(snapshot.state["providers"])
          ? (snapshot.state["providers"] as ProviderInfo[])
          : [],
      ),
    [snapshot.state["models"], snapshot.state["providers"]],
  );
  const activeModelId = provider && model ? `model:${provider}/${model}` : null;
  const onModelSelect = useCallback((id: string) => {
    // Same selection payload the Command Centre picker emits; the server
    // applies it through select_picker with this tab (I2 per-workspace
    // persistence), then the book snapshot refreshes the active pair.
    void sendRequest(agentCommandPayload({ select: { kind: "model", id } }));
  }, []);
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

  // Plan 118 task 35: the agent types present in this transcript. With more
  // than one, every turn shows its producer and the switch gets a note row
  // (derived from the stamped rows, so it survives a reload).
  const transcriptAgents = useMemo(() => {
    const seen = new Set<string>();
    for (const box of transcript) if (box.agent) seen.add(box.agent);
    return seen;
  }, [transcript]);
  const multiAgent = transcriptAgents.size > 1;

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

  const slashMatches = useMemo(() => {
    if (completionDismissed) return null;
    if (!draft.startsWith("/") || draft.includes(" ")) return null;
    return slashCommands
      .filter((command) => command.name.startsWith(draft))
      .slice(0, 8);
  }, [completionDismissed, draft, slashCommands]);

  // Plan 117 @-mentions: a trailing `@token` in the draft opens the merged
  // dropdown — the session's skill catalog plus the workspace file cache
  // (both server-known; filter is client-side). `@skill:x` / `@file:x`
  // narrow to one section; a bare token filters both.
  const filesRaw = Array.isArray(snapshot.state["workspaceFiles"])
    ? (snapshot.state["workspaceFiles"] as unknown[]).filter(
        (file): file is string => typeof file === "string",
      )
    : [];
  // ponytail: no display gate — a session switch refetches on the first @;
  // the bounded stale list renders for one RPC round-trip at most.
  const files = filesRaw;
  const mentionToken = useMemo(() => {
    if (completionDismissed) return null;
    const match = /(?:^|\s)@([^\s]*)$/.exec(draft);
    if (!match) return null;
    return {
      query: match[1] ?? "",
      start: match.index + match[0].lastIndexOf("@"),
    };
  }, [completionDismissed, draft]);
  const mentionMatches = useMemo(() => {
    if (!mentionToken) return null;
    const raw = mentionToken.query;
    const skillQuery = raw.startsWith("skill:")
      ? raw.slice(6)
      : raw.startsWith("file:")
        ? null
        : raw;
    const fileQuery = raw.startsWith("file:")
      ? raw.slice(5)
      : raw.startsWith("skill:")
        ? null
        : raw;
    const section = (
      label: string,
      items: string[],
      kind: "skill" | "file",
      query: string,
    ) => ({
      label,
      items: items
        .filter((item) => item.toLowerCase().includes(query.toLowerCase()))
        .slice(0, 8)
        .map((name) => ({ kind, name })),
    });
    const sections = [
      ...(skillQuery !== null
        ? [
            section(
              "Skills",
              skills.map((skill) => skill.name),
              "skill",
              skillQuery,
            ),
          ]
        : []),
      ...(fileQuery !== null && files.length > 0
        ? [section("Files", files, "file", fileQuery)]
        : []),
    ];
    const flat = sections.flatMap((entry) => entry.items);
    return flat.length > 0 ? { sections, flat } : null;
  }, [files, mentionToken, skills]);
  // First @-token of a session fetches the bounded workspace listing once;
  // the reply rides the clay.agentRpc custom event into agent state.
  useEffect(() => {
    if (!mentionToken || !sessionId || filesFetchedFor.current === sessionId)
      return;
    filesFetchedFor.current = sessionId;
    void sendRequest(agentCommandPayload({ workspaceFiles: { sessionId } }));
  }, [mentionToken, sessionId]);
  const embedMention = useCallback((kind: "skill" | "file", name: string) => {
    setDraft((current) => {
      const match = /(?:^|\s)@([^\s]*)$/.exec(current);
      if (!match) return current;
      const start = match.index + match[0].lastIndexOf("@");
      return `${current.slice(0, start)}@${kind}:${name} `;
    });
    setCompletionDismissed(true);
  }, []);

  const sendIntent = useCallback(
    (commandId: string) => {
      void sendRequest(
        sduiActionPayload(uiVersion, {
          commandId,
          source: { button: { nodeId: 1 } },
          arguments: [],
        }),
      ).catch(() => {
        // Disconnect flow owns recovery; nothing to do here.
      });
    },
    [uiVersion],
  );

  const submit = useCallback(
    (text: string) => {
      const trimmed = text.trim();
      if (!trimmed) return;
      // Core built-in (plan 109 I3): /model opens the daemon model picker
      // (Command Centre flow — configured models grouped by provider;
      // selection applies through select_picker with the bound tab, writing
      // the per-workspace selection per I2). The package deliberately does
      // not re-register it — see load.js SLASH_COMMANDS.
      if (trimmed === "/model") {
        setDraft("");
        setCompletionDismissed(true);
        sendIntent("agent.clientOpenModelPicker");
        return;
      }
      // Core built-in (plan 109 I9): /resume opens the workspace-scoped
      // session picker (Command Centre flow — server-scoped to the tab's
      // workspace; selection resumes through the existing resume path,
      // restoring transcript, live context, and the session's persisted
      // provider/model into the book).
      if (trimmed === "/resume") {
        setDraft("");
        setCompletionDismissed(true);
        sendIntent("agent.clientOpenSessionPicker");
        return;
      }
      if (!configured) return;
      setDraft("");
      setSelectedId("");
      if (streaming) {
        // Mid-run queue (pi-parity steer): the daemon folds the message into
        // the active run; the transcript keeps streaming. The pending effort
        // level stays pending — it applies to the next fresh prompt.
        agentSession.agent.steer(trimmed);
        return;
      }
      // Plan 109 I4: the pending effort rides the prompt (daemon fail-closes
      // invalid strings at its boundary and applies the model-aware Prism
      // 0.5.0 mapping per run). Once sent, it is the session's active level.
      const effortForRun = pendingEffort ?? undefined;
      agentSession.agent.sendPrompt(trimmed, effortForRun);
      // Plan 109 I5: the store flushes the deferred server snapshot once the
      // run pipeline is quiescent, so the server transcript wins at the
      // boundary.
      void agentSession.runTurn().catch(() => {
        // Failures already landed as RUN_ERROR status.
      });
    },
    [configured, sendIntent, streaming, pendingEffort],
  );

  const onComposerKeyDown = useCallback(
    (event: {
      key: string;
      shiftKey: boolean;
      ctrlKey?: boolean;
      altKey?: boolean;
      metaKey?: boolean;
      preventDefault: () => void;
    }) => {
      // Plan 109 I4: the manifest-bound effort chord (default Shift+Tab)
      // cycles declared levels — a no-op (no focus change) when the model
      // declares none or the chord is unbound.
      const chord = effortChord ?? {
        shift: true,
        ctrl: false,
        alt: false,
        meta: false,
        key: "Tab",
      };
      if (
        event.key === chord.key &&
        event.shiftKey === chord.shift &&
        (event.ctrlKey ?? false) === chord.ctrl &&
        (event.altKey ?? false) === chord.alt &&
        (event.metaKey ?? false) === chord.meta
      ) {
        if (effortLevels && effortLevels.length > 0) {
          event.preventDefault();
          const current =
            shownEffort !== null && effortLevels.includes(shownEffort)
              ? shownEffort
              : effortLevels[effortLevels.length - 1];
          const index = current
            ? effortLevels.indexOf(current)
            : effortLevels.length - 1;
          setPendingEffort(
            effortLevels[(index + 1) % effortLevels.length] ??
              effortLevels[0] ??
              null,
          );
        }
        return;
      }
      if (!slashMatches || slashMatches.length === 0) {
        // Plan 117 @-mention dropdown shares the completion index + dismiss
        // state (slash and mention are mutually exclusive by construction).
        if (mentionMatches && mentionMatches.flat.length > 0) {
          const count = mentionMatches.flat.length;
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setCompletionIndex((index) => (index + 1) % count);
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setCompletionIndex((index) => (index - 1 + count) % count);
          } else if (event.key === "Escape") {
            event.preventDefault();
            setCompletionDismissed(true);
          } else if (event.key === "Tab") {
            event.preventDefault();
            const picked = mentionMatches.flat[completionIndex % count];
            if (picked) embedMention(picked.kind, picked.name);
          }
        }
        return;
      }
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setCompletionIndex((index) => (index + 1) % slashMatches.length);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setCompletionIndex(
          (index) => (index - 1 + slashMatches.length) % slashMatches.length,
        );
      } else if (event.key === "Escape") {
        event.preventDefault();
        setCompletionDismissed(true);
      }
    },
    [
      effortChord,
      effortLevels,
      shownEffort,
      slashMatches,
      mentionMatches,
      completionIndex,
      embedMention,
    ],
  );

  const onComposerSubmit = useCallback(
    (value: string) => {
      if (slashMatches && slashMatches.length > 0) {
        const exact = slashMatches.find((command) => command.name === value);
        const highlighted: string | undefined =
          slashMatches[completionIndex]?.name;
        if (exact || highlighted) {
          submit(exact?.name ?? highlighted ?? "");
          return;
        }
      }
      submit(value);
    },
    [completionIndex, slashMatches, submit],
  );

  const complete = useCallback((commandName: string) => {
    setDraft(commandName);
    setCompletionIndex(0);
  }, []);

  // Tool approval (durable runs): Allow/Deny strip for suspended runs.
  const pendingApproval = snapshot.pendingApproval;
  const resolveApproval = useCallback(
    (outcome: "allow_once" | "reject_once") => {
      if (!pendingApproval) return;
      void sendRequest(
        agentCommandPayload({
          runResume: {
            sessionId: pendingApproval.sessionId,
            runId: pendingApproval.runId,
            decisionJson: JSON.stringify({
              decisions: [{ approvalId: pendingApproval.requestId, outcome }],
            }),
          },
        }),
      );
      // Optimistic clear; the resumed run re-announces via RUN_STARTED.
      agentSession.clearPendingApproval();
    },
    [pendingApproval],
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

          <div
            className={styles.transcript}
            role="log"
            aria-label="Transcript"
            aria-live="polite"
          >
            <div className={styles.transcriptInner}>
              {transcript.length > 0 ? (
                transcript.map((box, index) => {
                  const previous = transcript
                    .slice(0, index)
                    .reverse()
                    .find((candidate) => candidate.agent)?.agent;
                  return (
                    <TranscriptTurn
                      key={box.id}
                      box={box}
                      selected={box.id === selectedId}
                      showAgent={multiAgent}
                      agentChangedFrom={
                        multiAgent && box.agent && previous !== box.agent
                          ? previous
                          : undefined
                      }
                      onSelect={selectCard}
                    />
                  );
                })
              ) : (
                <EmptyTranscript />
              )}
            </div>
          </div>

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
              <div
                className={styles.approvalStrip}
                role="alertdialog"
                aria-label="Tool approval"
              >
                <ClayText variant="caption">
                  Tool “{pendingApproval.toolName}” needs approval
                </ClayText>
                <ClayButton onPress={() => resolveApproval("allow_once")}>
                  Allow
                </ClayButton>
                <ClayButton onPress={() => resolveApproval("reject_once")}>
                  Deny
                </ClayButton>
              </div>
            )}
            <form
              className={styles.composer}
              onSubmit={(event) => {
                event.preventDefault();
                onComposerSubmit(draft);
              }}
            >
              {slashMatches && slashMatches.length > 0 && (
                <ul
                  className={styles.completions}
                  aria-label="Slash commands"
                  role="listbox"
                >
                  {slashMatches.map((command, index) => (
                    <li key={command.name}>
                      <button
                        type="button"
                        role="option"
                        aria-selected={index === completionIndex}
                        className={styles.completionRow}
                        onClick={() => complete(command.name)}
                      >
                        <span className={styles.completionName}>
                          {command.name}
                        </span>
                        <span className={styles.completionDetail}>
                          {command.description}
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              {mentionMatches && (
                <ul
                  className={styles.completions}
                  aria-label="Mentions"
                  role="listbox"
                >
                  {mentionMatches.sections.map((section) => (
                    <li key={section.label} role="presentation">
                      <span
                        className={styles.completionSection}
                        role="presentation"
                      >
                        {section.label}
                      </span>
                      <ul role="group" aria-label={`${section.label} matches`}>
                        {section.items.map((item) => (
                          <li key={`${item.kind}:${item.name}`}>
                            <button
                              type="button"
                              role="option"
                              aria-selected={
                                mentionMatches.flat[
                                  completionIndex % mentionMatches.flat.length
                                ]?.name === item.name
                              }
                              className={styles.completionRow}
                              onClick={() => embedMention(item.kind, item.name)}
                            >
                              <span className={styles.completionName}>
                                @{item.kind}:{item.name}
                              </span>
                            </button>
                          </li>
                        ))}
                      </ul>
                    </li>
                  ))}
                </ul>
              )}
              <ClayTextField
                label="Message"
                value={draft}
                onChange={(value) => {
                  setDraft(value);
                  setCompletionIndex(0);
                  setCompletionDismissed(false);
                }}
                multiline
                autoGrow
                variant="composer"
                placeholder={
                  configured
                    ? streaming
                      ? "Steer the agent, or wait"
                      : "Ask, or type / or @"
                    : "Configure a provider first"
                }
                disabled={!configured}
                onKeyDown={onComposerKeyDown}
                onSubmit={onComposerSubmit}
                endContent={
                  <span className={styles.composerActions}>
                    {streaming ? (
                      <ClayIconButton
                        icon="generation.stop"
                        label="Stop"
                        onPress={() => agentSession.agent.abortRun()}
                      />
                    ) : (
                      <ClayIconButton
                        icon="message.send"
                        label="Send"
                        type="submit"
                        isDisabled={!configured || !draft.trim()}
                      />
                    )}
                    <ClayIconButton
                      icon="action.close"
                      label="Close"
                      variant="muted"
                      onPress={() => sendIntent("coding-agent.close")}
                    />
                  </span>
                }
              />
              <div className={styles.composerHints}>
                <span className={styles.keyHint}>
                  <ClayKbd>/</ClayKbd> commands
                </span>
                <span className={styles.keyHint}>
                  <ClayKbd>@</ClayKbd> mention file or skill
                </span>
                <span className={styles.keyHint}>
                  <ClayKbd>↵</ClayKbd> send
                </span>
                <span className={styles.keyHint}>
                  <ClayKbd>⇧</ClayKbd>
                  <ClayKbd>↵</ClayKbd> newline
                </span>
                <span className={styles.spacer} />
                {draft.trim().length > 0 && (
                  <span className={styles.hintCount}>
                    {draft.trim().length} chars
                  </span>
                )}
              </div>
            </form>
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

        <aside className={styles.inspector} aria-label="Agent inspector">
          <ClayTabStrip
            className={styles.tabs}
            activeId={activeTab}
            onActivate={setActiveTab}
            ariaLabel="Agent detail"
            actions={
              <ClayIconButton
                icon="preview.toggle"
                label="Hide inspector"
                variant="muted"
                onPress={() => setInspectorVisible(false)}
              />
            }
            tabs={[
              {
                id: "files",
                label: "Files",
                content: (
                  <FilesTab
                    records={sessionFileList}
                    open={activeTab === "files"}
                    onOpen={openInWorkspace}
                  />
                ),
              },
              {
                id: "memory",
                label: "Memory",
                content: (
                  <MemoryTab
                    sessionId={sessionId}
                    open={activeTab === "memory"}
                    transcriptLength={transcript.length}
                    view={omView}
                    modelGroups={modelGroups}
                  />
                ),
              },
              {
                id: "context",
                label: "Context",
                content: (
                  <ContextTab
                    sessionId={sessionId}
                    open={activeTab === "context"}
                    transcriptLength={transcript.length}
                    view={contextView}
                    skills={skills}
                    servers={mcpServers}
                    detail={
                      isContextItemDetail(snapshot.state["contextItemDetail"])
                        ? (snapshot.state[
                            "contextItemDetail"
                          ] as ContextItemDetail)
                        : null
                    }
                  />
                ),
              },
              {
                // Plan 109 I10: card-detail destination. Selecting a
                // transcript card auto-switches here and renders that
                // entry's full redacted content from the already-loaded
                // transcript — no refetch.
                id: "session-info",
                label: "Session Info",
                content: (
                  <SessionInfoTab
                    selected={selected}
                    onBack={() => selectCard(selectedId)}
                  />
                ),
              },
              {
                // Plan 117 follow-up: the agent's delivered config files.
                // Replaces the shell-owned side panel — the listing rides
                // this pane's own document session, so the shell keeps no
                // agent-settings state.
                id: "settings",
                label: "Settings",
                content: (
                  <SettingsTab
                    session={session}
                    open={activeTab === "settings"}
                    sendIntent={sendIntent}
                  />
                ),
              },
            ]}
          />
        </aside>
      </div>
    </section>
  );
}

/**
 * Settings tab (plan 117 follow-up): the agent's delivered config files
 * (SYSTEM.md + seeded SKILL.md), with built-in-vs-edited provenance. The
 * listing and the open both ride this pane's own document session, so the
 * tab needs no shell state and no extra protocol surface: the reply arrives
 * as an `agentSettingsFiles` feature event on that same session.
 */
function SettingsTab({
  session,
  open,
  sendIntent,
}: {
  session: DocumentSession | null;
  open: boolean;
  sendIntent: (commandId: string) => void;
}) {
  const [files, setFiles] = useState<AgentSettingsFileInfo[] | null>(null);
  useEffect(() => {
    if (!open || !session) return;
    const unsubscribe = session.subscribeFeatures((envelope) => {
      const event = envelope.data as {
        kind?: string;
        data?: { files?: AgentSettingsFileInfo[] };
      };
      if (event.kind !== "agentSettingsFiles" || !event.data) return;
      setFiles(event.data.files ?? []);
    });
    session.listAgentSettings();
    return unsubscribe;
  }, [open, session]);
  if (!session) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          No active agent session.
        </ClayText>
      </div>
    );
  }
  return (
    <AgentSettingsPanel
      files={files}
      loading={files == null}
      onOpen={(name) => {
        session.openAgentSettings(name);
        // The opened file is an ordinary document: release the agent surface
        // so the pane shows the editor instead of this panel (the same
        // release the composer's Close button performs).
        sendIntent("coding-agent.close");
      }}
    />
  );
}

/**
 * Plan 109 I10: shared bounded-text viewer — read-only, redacted,
 * wrapping. Used by the Session Info detail and the Context drawer
 * (the kinds overlap: both render transcript-derived bounded text).
 */
function BoundedText({ text }: { text: string }) {
  return <pre className={styles.detailContent}>{text}</pre>;
}

/**
 * Session Info tab (plan 109 I10): the card-detail destination. Renders
 * the selected transcript entry's kind, complete redacted content, and
 * type-specific metadata from the already-loaded transcript — no refetch,
 * no new data path. No selection → guidance. Back restores the tab the
 * selection came from.
 */
function SessionInfoTab({
  selected,
  onBack,
}: {
  selected: TranscriptBox | null;
  onBack: () => void;
}) {
  if (!selected) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          Select a transcript card to inspect its full detail here.
        </ClayText>
      </div>
    );
  }
  const rows: Array<[string, string]> = [["Kind", selected.kind]];
  if (selected.kind === "tool" || selected.kind === "skill") {
    rows.push([
      selected.kind === "skill" ? "Skill" : "Tool",
      selected.toolName || selected.label,
    ]);
  }
  return (
    <div className={styles.tabEmpty}>
      <div className={styles.detailHeader}>
        <ClayText variant="caption" muted>
          {selected.label}
        </ClayText>
        <ClayButton variant="muted" onPress={onBack}>
          <ClayIcon name="navigation.back" />
          Back
        </ClayButton>
      </div>
      <dl className={styles.infoMeta}>
        {rows.map(([term, value]) => (
          <div key={term} className={styles.infoRow}>
            <dt>{term}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <BoundedText text={selected.content} />
    </div>
  );
}

/**
 * Files tab (plan 118 task 36): the files *this session* has touched, newest
 * first — session history, never a file browser (the tree lives in the
 * workspace view, and so does the editor). A row opens in the tab's workspace
 * view, which is the dual-view payoff: the agent keeps its transcript and the
 * workspace takes the file. Rows come from the transcript's own tool records,
 * so the list survives a resume and clears with the session.
 */
function FilesTab({
  records,
  open,
  onOpen,
}: {
  records: readonly SessionFileRecord[];
  /** The inspector tab is showing (the panel mounts every tab's content). */
  open: boolean;
  /** Open a path in the tab's workspace view. */
  onOpen: (path: string) => void;
}) {
  const [query, setQuery] = useState("");
  const filterRef = useRef<HTMLInputElement | null>(null);
  // The filter's chord, scoped to the panel that shows it and inert while a
  // field has the keyboard (the artifact's `F` hint is a real binding).
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "f") return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      const tag = target?.tagName.toLowerCase();
      if (
        tag === "input" ||
        tag === "textarea" ||
        target?.isContentEditable === true
      ) {
        return;
      }
      event.preventDefault();
      filterRef.current?.focus();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open]);

  const needle = query.trim().toLowerCase();
  const visible =
    needle.length === 0
      ? records
      : records.filter((record) => record.path.toLowerCase().includes(needle));

  if (records.length === 0) {
    // The approved empty state: what this panel is, and the two keystrokes
    // that move the session forward instead.
    return (
      <div className={styles.sessionFilesEmpty}>
        <ClayText variant="title">No files in this session yet</ClayText>
        <ClayText variant="body" muted>
          Files the agent reads, writes or searches appear here, newest first.
          Opening one is a keystroke: it switches the tab to its workspace view
          at that file.
        </ClayText>
        <div className={styles.sessionFileKeys}>
          <span className={styles.keyHint}>
            <ClayKbd>@</ClayKbd> mention a file
          </span>
          <span className={styles.keyHint}>
            <ClayKbd>Ctrl</ClayKbd>
            <ClayKbd>1</ClayKbd> workspace view
          </span>
        </div>
      </div>
    );
  }

  return (
    <section className={styles.section} aria-label="Session files">
      <div className={styles.sectionHead}>
        <span className={styles.sectionLabel}>Session files</span>
        <ClayBadge tone="muted">{visible.length}</ClayBadge>
        <span className={styles.spacer} />
        <span className={styles.sessionFilter}>
          <input
            ref={filterRef}
            type="search"
            className={styles.sessionFilterInput}
            placeholder="Filter"
            aria-label="Filter session files"
            autoComplete="off"
            spellCheck={false}
            value={query}
            {...recipeAttributes("textInput", "input")}
            onChange={(event) => setQuery(event.target.value)}
          />
          <ClayKbd>F</ClayKbd>
        </span>
      </div>
      <ul
        className={styles.sessionList}
        aria-label="Files this session has touched"
      >
        {visible.map((record) => {
          const { name, dir } = splitSessionFilePath(record.path);
          return (
            <li key={record.path}>
              <button
                type="button"
                className={styles.sessionRow}
                data-role={record.role}
                aria-label={`Open ${record.path} in the workspace view`}
                onClick={() => onOpen(record.path)}
                {...recipeAttributes("sessionRow", "root")}
              >
                <span className={styles.sessionMark} aria-hidden>
                  {SESSION_FILE_MARK[record.role]}
                </span>
                <span className={styles.rowMain}>
                  <span className={styles.rowName}>{name}</span>
                  {dir.length > 0 && (
                    <span className={styles.rowDetail}>{dir}</span>
                  )}
                </span>
                <span className={styles.sessionRole}>{record.role}</span>
              </button>
            </li>
          );
        })}
      </ul>
      <p className={styles.rowDetail}>
        Files this session has read, written or searched. Opening one switches
        the tab to its workspace view at that file; this is session history, not
        a file browser — the tree lives in the workspace view.
      </p>
    </section>
  );
}

/** Plan 109 I7: server-authoritative context view (daemon `session.context`).
 *  Counts are real numbers; `items` are capped and may trail `count`. */
interface ContextItemRef {
  id: string;
  title: string;
  preview: string;
}
interface ContextCategory {
  kind: string;
  label: string;
  count: number;
  items: ContextItemRef[];
}
interface ContextView {
  sessionId?: string;
  version?: number;
  categories?: ContextCategory[];
}
/** One open item's full redacted content (`session.context { itemId }`). */
interface ContextItemDetail {
  itemId?: string;
  kind?: string;
  title?: string;
  content?: string;
}

function isContextView(value: unknown): value is ContextView {
  return (
    typeof value === "object" &&
    value !== null &&
    Array.isArray((value as ContextView).categories)
  );
}

/** Drawer detail shape guard (see the `clay.agentRpc` handler). */
function isContextItemDetail(value: unknown): value is ContextItemDetail {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as ContextItemDetail).itemId === "string"
  );
}

/**
 * Observational Memory tab (plan 109 I8): the Observer's activity log —
 * recorded observations with fact summaries, reflections, drops (visible,
 * never silently vanished), and compaction folds — plus OM worker-model
 * selection over the same grouped model list as the session model. The
 * view is server-authoritative (`session.om.activity`), fetched on tab
 * open / transcript change (completion boundaries, never polled); worker
 * selections ride `SelectWorker` (per-workspace book + per-session
 * daemon metadata) and may differ from the session model.
 */
interface OmActivityRow {
  id: string;
  kind: string;
  summary: string;
}

interface OmView {
  sessionId?: string;
  attached?: boolean;
  observation?: { provider?: string; model?: string } | null;
  reflection?: { provider?: string; model?: string } | null;
  activity?: OmActivityRow[];
}

const OM_WORKER_CLEAR = "om-worker:none";

function OmWorkerDropdown({
  label,
  selection,
  groups,
  onSelect,
}: {
  label: string;
  selection: { provider?: string; model?: string } | null;
  groups: Array<{ label: string; options: DropdownOption[] }>;
  onSelect: (id: string) => void;
}) {
  const selectedId = selection?.provider
    ? `model:${selection.provider}/${selection.model}`
    : OM_WORKER_CLEAR;
  const clearOption = { id: OM_WORKER_CLEAR, label: "Not set (workers off)" };
  // The clear option must live in the rendered set: ClayDropdown ignores
  // `options` when `groups` is present, so the clear entry is prepended to
  // the first group (or stands alone when no models are configured).
  const firstGroup = groups[0];
  const groupsWithClear = firstGroup
    ? [
        {
          label: firstGroup.label,
          options: [clearOption, ...firstGroup.options],
        },
        ...groups.slice(1),
      ]
    : undefined;
  return (
    <div className={styles.omWorkerRow}>
      <ClayText variant="caption" muted>
        {label}
      </ClayText>
      <ClayDropdown
        label={label}
        options={[clearOption]}
        groups={groupsWithClear}
        selectedId={selectedId}
        onSelect={onSelect}
      />
    </div>
  );
}

function MemoryTab({
  sessionId,
  open,
  transcriptLength,
  view,
  modelGroups,
}: {
  sessionId: string;
  open: boolean;
  transcriptLength: number;
  view: OmView | undefined;
  modelGroups: Array<{ label: string; options: DropdownOption[] }>;
}) {
  // The activity list follows run/OM-worker completion via the transcript
  // length (the same completion-boundary signal the transcript uses).
  useEffect(() => {
    if (!open || !sessionId) return;
    void sendRequest(agentCommandPayload({ omActivity: { sessionId } })).catch(
      () => {
        // Failures land as diagnostics; the tab keeps the last view.
      },
    );
  }, [open, sessionId, transcriptLength]);
  const selectWorker =
    (worker: "observation" | "reflection") => (id: string) => {
      if (!sessionId) return;
      const clear = id === OM_WORKER_CLEAR;
      void sendRequest(
        agentCommandPayload({
          selectWorker: {
            worker,
            id: clear ? "" : id,
            sessionId,
          },
        }),
      ).catch(() => {
        // Same failure surface as the read fetch.
      });
    };
  if (!sessionId) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          No active agent session.
        </ClayText>
      </div>
    );
  }
  const stale =
    view && typeof view.sessionId === "string" && view.sessionId !== sessionId;
  const activity = (!stale && view?.activity) || [];
  const attached = !stale && view?.attached === true;
  return (
    <div className={styles.omTab}>
      <div className={styles.omWorkers}>
        <OmWorkerDropdown
          label="Observation worker"
          selection={stale ? null : (view?.observation ?? null)}
          groups={modelGroups}
          onSelect={selectWorker("observation")}
        />
        <OmWorkerDropdown
          label="Reflection worker"
          selection={stale ? null : (view?.reflection ?? null)}
          groups={modelGroups}
          onSelect={selectWorker("reflection")}
        />
      </div>
      <ClayText variant="caption" muted>
        {attached
          ? "Observer activity this session"
          : "Observational Memory is off for this session."}
      </ClayText>
      {activity.length > 0 ? (
        <ul className={styles.omActivity} aria-label="Observer activity">
          {activity.map((row) => (
            <li key={row.id} className={styles.omActivityRow}>
              <span
                className={`${styles.omKind} ${
                  styles[`omKind_${row.kind}` as keyof typeof styles] ?? ""
                }`}
              >
                {row.kind}
              </span>
              <ClayText variant="detail">{row.summary}</ClayText>
            </li>
          ))}
        </ul>
      ) : (
        <div className={styles.tabEmpty}>
          <ClayText variant="caption" muted>
            {attached
              ? "No observer activity yet."
              : "Select worker models to let the Observer record activity."}
          </ClayText>
        </div>
      )}
    </div>
  );
}

/**
 * Context tab (plan 109 I7): live context inspector over the daemon's
 * server-authoritative `session.context` view. Category list → in-tab
 * drawer (item list) → item detail — all inside the tab panel (no
 * modal/overlay), catalog list/label composition, recipe vars only.
 * Fetch is on-demand (tab open + transcript-length invalidation); the
 * server view is cached per version in agent state.
 */
function ContextTab({
  sessionId,
  open,
  transcriptLength,
  view,
  skills,
  servers,
  detail,
}: {
  sessionId: string;
  open: boolean;
  transcriptLength: number;
  view: ContextView | null;
  /** Capability inventories: reference data, so it lives here and not in the
   *  transcript (DESIGN.md §12). */
  skills: ReadonlyArray<{ name: string; description: string }>;
  servers: ReadonlyArray<{
    serverId: string;
    connected: boolean;
    tools: number;
    error: string;
  }>;
  detail: ContextItemDetail | null;
}) {
  const capabilities = <CapabilitySections skills={skills} servers={servers} />;
  const [drawer, setDrawer] = useState<string | null>(null);
  const [requestedItem, setRequestedItem] = useState<string | null>(null);
  useEffect(() => {
    if (!open || !sessionId) return;
    void sendRequest(agentCommandPayload({ context: { sessionId } })).catch(
      () => {
        // Failures land as diagnostics; the tab keeps the last view.
      },
    );
  }, [open, sessionId, transcriptLength]);
  const openItem = (id: string) => {
    if (!sessionId) return;
    setRequestedItem(id);
    void sendRequest(
      agentCommandPayload({ context: { sessionId, itemId: id } }),
    ).catch(() => {
      // Same failure surface as the list fetch.
    });
  };
  const categories = view?.categories ?? [];
  const drawerCategory =
    drawer !== null ? categories.find((c) => c.kind === drawer) : undefined;
  // The drawer detail follows the last requested item id.
  const shownDetail = detail && requestedItem === detail.itemId ? detail : null;

  if (!sessionId) {
    return (
      <div className={styles.contextTab}>
        {capabilities}
        <div className={styles.tabEmpty}>
          <ClayText variant="body" muted>
            No active agent session.
          </ClayText>
        </div>
      </div>
    );
  }
  // First response not in yet (or stale view from a previous session).
  if (
    !view ||
    (typeof view.sessionId === "string" && view.sessionId !== sessionId)
  ) {
    return (
      <div className={styles.tabEmpty} role="status">
        <ClayText variant="body" muted>
          Loading context…
        </ClayText>
      </div>
    );
  }
  if (drawerCategory) {
    return (
      <div className={styles.contextTab}>
        {capabilities}
        <div className={styles.contextDrawer}>
          <div className={styles.detailHeader}>
            <ClayText variant="caption">{drawerCategory.label}</ClayText>
            <ClayButton
              variant="muted"
              onPress={() => {
                setDrawer(null);
                setRequestedItem(null);
              }}
            >
              <ClayIcon name="navigation.back" />
              Back
            </ClayButton>
          </div>
          {shownDetail ? (
            <div className={styles.contextDetail}>
              <ClayText variant="detail">{shownDetail.title}</ClayText>
              <BoundedText text={shownDetail.content ?? ""} />
              <ClayButton
                variant="muted"
                onPress={() => setRequestedItem(null)}
              >
                <ClayIcon name="navigation.back" />
                Back to list
              </ClayButton>
            </div>
          ) : (
            <ul
              className={styles.contextScroll}
              aria-label={`${drawerCategory.label} items`}
            >
              {drawerCategory.items.map((item) => (
                <li key={item.id}>
                  <button
                    type="button"
                    className={styles.contextItem}
                    onClick={() => openItem(item.id)}
                  >
                    <ClayText variant="detail">{item.title}</ClayText>
                    <ClayText variant="caption" muted>
                      {item.preview}
                    </ClayText>
                  </button>
                </li>
              ))}
              {drawerCategory.count > drawerCategory.items.length && (
                <li className={styles.contextMore}>
                  <ClayText variant="caption" muted>
                    …and {drawerCategory.count - drawerCategory.items.length}{" "}
                    more (not listed)
                  </ClayText>
                </li>
              )}
            </ul>
          )}
        </div>
      </div>
    );
  }
  // Count bars are relative to the largest real count on screen — a share of
  // the session's own context, never a fabricated ceiling.
  const maxCount = categories.reduce(
    (max, category) => Math.max(max, category.count),
    0,
  );
  return (
    <div className={styles.contextTab}>
      {capabilities}
      {categories.length > 0 && (
        <section className={styles.section} aria-label="Context items">
          <div className={styles.sectionHead}>
            <span className={styles.sectionLabel}>Context items</span>
          </div>
          <ul className={styles.statRows}>
            {categories.map((category) => (
              <li key={category.kind} className={styles.statRow}>
                <button
                  type="button"
                  className={styles.statButton}
                  onClick={() => setDrawer(category.kind)}
                >
                  <span className={styles.statLabel}>{category.label}</span>
                  <span className={styles.statValue}>{category.count}</span>
                </button>
                <span className={styles.statBar} aria-hidden>
                  <span
                    className={styles.statBarFill}
                    style={{
                      width: `${maxCount > 0 ? Math.round((category.count / maxCount) * 100) : 0}%`,
                    }}
                  />
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {categories.length === 0 && (
        <div className={styles.tabEmpty}>
          <ClayText variant="body" muted>
            Loading context…
          </ClayText>
        </div>
      )}
    </div>
  );
}

/** Typed agent-family request through the validated bridge path. Unit
 *  variants ride the bare-string form — `{ listSessions: {} }` fails serde
 *  deserialization (map content where a unit is expected). */
function agentCommandPayload(
  command: Record<string, unknown> | string,
): string {
  return JSON.stringify({
    family: "agent",
    payload: { clientId: 0, command },
  });
}
