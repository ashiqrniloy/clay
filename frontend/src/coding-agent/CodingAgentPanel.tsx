// Coding Agent split surface (plan 108 task 8) — binding-spec presentation
// for the bundled `@clay/coding-agent` package.
//
// Provenance-exact host rendering, mirroring the ChatPanel/SettingsPanel
// precedent: static copy comes from the package's declared component tree;
// every dynamic value rides the ONE core-owned AG-UI stream (`chatAgent` —
// same daemon session as chat); every interaction emits declared inert
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
import { Group, Panel, Separator } from "react-resizable-panels";

import {
  ClayButton,
  ClayDropdown,
  ClayIcon,
  ClayIconButton,
  ClayTabStrip,
  ClayText,
  ClayTextField,
} from "../components";
import type { DropdownOption } from "../components";
import { chatAgent } from "../agent/state";
import { sendRequest } from "../bridge/client";
import { sduiActionPayload } from "../sdui/actions";
import type {
  PackageComponentNode,
  PackageSurface,
} from "../sdui/types";
import { ClayEditor } from "../editor/ClayEditor";
import type { DocumentSession } from "../editor/sync/session";

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
const CLIENT_SLASH_COMMANDS: ReadonlyArray<{ name: string; description: string }> = [
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
}

interface ProviderInfo {
  id?: unknown;
  configured?: unknown;
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
  | "user"
  | "assistant"
  | "reasoning"
  | "usage"
  | "error"
  | "tool"
  | "skill";

interface TranscriptBox {
  id: string;
  kind: TranscriptKind;
  label: string;
  content: string;
  /** Tool rows only: the tool name for Context-tab category counts. */
  toolName?: string;
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
 * Uniform-height truncated transcript boxes (fixed 3-line clamp — the
 * metadata-driven fixed-line-count truncation from the plan), colored by
 * content type from typed theme tokens. Selection shows the full content in
 * the right pane.
 */
const TranscriptBoxRow = memo(function TranscriptBoxRow({
  box,
  selected,
  onSelect,
}: {
  box: TranscriptBox;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <button
      type="button"
      className={`${styles.box} ${styles[`box_${box.kind}`]}`}
      aria-pressed={selected}
      aria-label={`${box.label} ${box.content}`}
      onClick={() => onSelect(selected ? "" : box.id)}
    >
      <span className={styles.boxLabel}>{box.label}</span>
      <span className={styles.boxContent}>{box.content}</span>
    </button>
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
  /** The pane's document session; the Files tab hosts its editor view
   *  (plan 109 I6: content follows workspace selection — the workspace
   *  browser tree lives only in the left workspace tab). */
  session?: DocumentSession | null;
  /** Intent sender for the agent session relay. */
  send?: (payload: string) => Promise<void>;
  /** Effective effort-cycle chord from the behavior manifest
   *  (`coding-agent.clientCycleEffort`); default `Shift+Tab`. */
  effortChord?: EffortChord | null;
}

export function CodingAgentPanel({
  surface,
  uiVersion,
  workspaceRoot,
  session = null,
  send,
  effortChord = null,
}: CodingAgentPanelProps) {
  const snapshot = useSyncExternalStore(
    chatAgent.subscribe,
    chatAgent.getSnapshot,
    chatAgent.getSnapshot,
  );

  useEffect(() => {
    chatAgent.agent.setUiVersion(uiVersion);
  }, [uiVersion]);
  useEffect(() => {
    if (send) chatAgent.agent.setSender(send);
  }, [send]);
  useEffect(() => chatAgent.start(), []);
  useEffect(() => {
    void sendRequest(agentCommandPayload({ listSessions: {} }));
  }, []);

  const declared = surface.component;
  const transcriptTitle = useMemo(
    () => findNode(declared, "coding-agent.transcriptTitle")?.text ?? "Agent",
    [declared],
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
  // Plan 109 I7: which detail tab is open — the Context fetch fires on tab
  // open and re-fires when the transcript length changes (event-driven
  // invalidation: every run finish / compaction / entry append lands a
  // transcript entry and bumps the server's context version).
  const [activeTab, setActiveTab] = useState("files");
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
  const mcpServers = Array.isArray(snapshot.state["mcpServers"])
    ? (snapshot.state["mcpServers"] as string[])
    : [];
  const effortLevels = Array.isArray(snapshot.state["effortLevels"])
    ? (snapshot.state["effortLevels"] as string[])
    : null;
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
  const activeModelId =
    provider && model ? `model:${provider}/${model}` : null;
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
  const contextWindow =
    typeof snapshot.state["contextWindow"] === "number"
      ? (snapshot.state["contextWindow"] as number)
      : null;

  const transcript = useMemo<TranscriptBox[]>(() => {
    const boxes: TranscriptBox[] = [];
    for (const message of snapshot.messages) {
      const kind = kindOf(message);
      if (!kind) continue;
      const content =
        typeof message.content === "string" ? message.content : "";
      const meta = kind === "tool" || kind === "skill" ? toolMeta(message) : null;
      boxes.push({
        id: message.id,
        kind,
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
        description: typeof command.description === "string" ? command.description : "",
      }));
    const merged = new Map<string, { name: string; description: string }>();
    for (const command of [...CLIENT_SLASH_COMMANDS, ...daemon]) {
      if (!merged.has(command.name)) merged.set(command.name, command);
    }
    return [...merged.values()];
  }, [snapshot.state]);

  const slashMatches = useMemo(() => {
    if (completionDismissed) return null;
    if (!draft.startsWith("/") || draft.includes(" ")) return null;
    return slashCommands
      .filter((command) => command.name.startsWith(draft))
      .slice(0, 8);
  }, [completionDismissed, draft, slashCommands]);

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
        chatAgent.agent.steer(trimmed);
        return;
      }
      // Plan 109 I4: the pending effort rides the prompt (daemon fail-closes
      // invalid strings at its boundary and applies the model-aware Prism
      // 0.5.0 mapping per run). Once sent, it is the session's active level.
      const effortForRun = pendingEffort ?? undefined;
      chatAgent.agent.sendPrompt(trimmed, effortForRun);
      // Plan 109 I5: the store flushes the deferred server snapshot once the
      // run pipeline is quiescent, so the server transcript wins at the
      // boundary.
      void chatAgent.runTurn().catch(() => {
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
      const chord = effortChord ?? { shift: true, ctrl: false, alt: false, meta: false, key: "Tab" };
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
          const index = current ? effortLevels.indexOf(current) : effortLevels.length - 1;
          setPendingEffort(effortLevels[(index + 1) % effortLevels.length] ?? effortLevels[0] ?? null);
        }
        return;
      }
      if (!slashMatches || slashMatches.length === 0) return;
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
    [effortChord, effortLevels, shownEffort, slashMatches],
  );

  const onComposerSubmit = useCallback(
    (value: string) => {
      if (slashMatches && slashMatches.length > 0) {
        const exact = slashMatches.find((command) => command.name === value);
        const highlighted: string | undefined = slashMatches[completionIndex]?.name;
        if (exact || highlighted) {
          submit(exact?.name ?? highlighted ?? "");
          return;
        }
      }
      submit(value);
    },
    [completionIndex, slashMatches, submit],
  );

  const complete = useCallback(
    (commandName: string) => {
      setDraft(commandName);
      setCompletionIndex(0);
    },
    [],
  );

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
      chatAgent.clearPendingApproval();
    },
    [pendingApproval],
  );
  // Session list/resume (plan 108 task 9): the same bounded listSessions
  // inventory Chat uses; Resume rebinds this tab's session server-side.
  const sessions = Array.isArray(snapshot.state["sessions"])
    ? (snapshot.state["sessions"] as Array<Record<string, unknown>>)
    : [];
  const lastUsageText =
    contextTokens !== null && contextWindow !== null
      ? `context ${contextTokens}/${contextWindow} tok`
      : lastUsage;
  // Plan 109 I7: the Context tab renders the server-authoritative view
  // (daemon `session.context`); the client-derived counters are gone.
  const sessionId =
    typeof snapshot.state["sessionId"] === "string"
      ? (snapshot.state["sessionId"] as string)
      : "";
  const contextView = isContextView(snapshot.state["contextView"])
    ? (snapshot.state["contextView"] as ContextView)
    : null;


  return (
    <section
      className={styles.surface}
      aria-label="Coding Agent"
      data-coding-agent-surface
    >
      <Group
        orientation="horizontal"
        className={styles.split}
        aria-label="Coding Agent split"
      >
        <Panel
          defaultSize="50%"
          minSize="20%"
          maxSize="80%"
          className={styles.leftPane}
        >
          <div className={styles.left}>
            <header className={styles.header}>
              <ClayText variant="title">{transcriptTitle}</ClayText>
              <span className={styles.headerMeta}>
                <ClayText variant="caption" muted>
                  {configured
                    ? `${String(snapshot.state["profile"] ?? "coding")} ·`
                    : "Configure a provider to start."}
                </ClayText>
                {configured && modelGroups.length > 0 && (
                  <ClayDropdown
                    label="Model"
                    options={[]}
                    groups={modelGroups}
                    selectedId={activeModelId}
                    onSelect={onModelSelect}
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
              {transcript.length > 0 ? (
                transcript.map((box) => (
                  <TranscriptBoxRow
                    key={box.id}
                    box={box}
                    selected={box.id === selectedId}
                    onSelect={selectCard}
                  />
                ))
              ) : (
                <p className={styles.empty} role="status">
                  <ClayText variant="body" muted>
                    No conversation yet.
                  </ClayText>
                </p>
              )}
            </div>

            <footer className={styles.composerArea}>
              {pendingApproval && (
                <div className={styles.approvalStrip} role="alertdialog" aria-label="Tool approval">
                  <ClayText variant="caption">
                    Tool “{pendingApproval.toolName}” needs approval
                  </ClayText>
                  <ClayButton onPress={() => resolveApproval("allow_once")}>Allow</ClayButton>
                  <ClayButton onPress={() => resolveApproval("reject_once")}>Deny</ClayButton>
                </div>
              )}
              <p className={styles.statusLine} role="status">
                <ClayText variant="status">
                  {streaming ? "Streaming" : (snapshot.status.status ?? "Ready")}
                </ClayText>
              </p>
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
                        <ClayText variant="caption" muted>
                          {command.description}
                        </ClayText>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <form
                className={styles.composer}
                onSubmit={(event) => {
                  event.preventDefault();
                  onComposerSubmit(draft);
                }}
              >
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
                  placeholder={
                    configured
                      ? streaming
                        ? "Steer the agent, or wait"
                        : "Ask, or type /"
                      : "Configure a provider first"
                  }
                  disabled={!configured}
                  onKeyDown={onComposerKeyDown}
                  onSubmit={onComposerSubmit}
                />
                <span className={styles.composerActions}>
                  {streaming ? (
                    <ClayIconButton
                      icon="generation.stop"
                      label="Stop"
                      onPress={() => chatAgent.agent.abortRun()}
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
              </form>
            </footer>

            <div className={styles.statusRow} role="status">
              <span className={styles.statusLeft}>
                {/* Plan 109 R2: the workspace's real git branch (bounded
                 * server-side `.git` read riding session state); `—` when
                 * not a repo or not yet read. */}
                <ClayText variant="caption" muted>
                  {workspaceRoot} · git {gitBranch || "—"}
                </ClayText>
              </span>
              <span className={styles.statusRight}>
                <ClayText variant="caption" muted>
                  {configured ? `${provider}/${model}` : "no provider"}
                  {lastUsageText ? ` · ${lastUsageText}` : ""}
                </ClayText>
                {/* Plan 109 I4: cataloged ClayDropdown for declared levels;
                 * plain caption readout otherwise. The selected level rides
                 * the next prompt. */}
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
                      {` · effort ${activeEffort}`}
                    </ClayText>
                  )
                )}
              </span>
            </div>

            <div className={styles.extensionStrip} aria-label="Extensions">
              {/* Plan 109 R3: actual active extensions from daemon state;
               * the segment is omitted when none report. */}
              {extensions.length > 0 && (
                <ClayText variant="caption" muted>
                  Extensions: {extensions.join(", ")}
                </ClayText>
              )}
              <ClayText variant="caption" muted>
                MCP: {mcpServers.length > 0 ? mcpServers.join(", ") : "none"}
              </ClayText>
            </div>
          </div>
        </Panel>

        <Separator className={styles.separator} aria-label="Resize split" />

        <Panel minSize="20%" maxSize="80%" className={styles.rightPane}>
          <div className={styles.right}>
            <ClayTabStrip
              className={styles.tabs}
              activeId={activeTab}
              onActivate={setActiveTab}
              ariaLabel="Agent detail"
              tabs={[
                {
                  id: "files",
                  label: "Files",
                  content: (
                    <FilesTab
                      session={session}
                      sendIntent={sendIntent}
                      sessions={sessions}
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
                      detail={
                        isContextItemDetail(snapshot.state["contextItemDetail"])
                          ? (snapshot.state["contextItemDetail"] as ContextItemDetail)
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
              ]}
            />
          </div>
        </Panel>
      </Group>
    </section>
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
 * Files tab (plan 109 I6): hosts the same ClayEditor surface the shell
 * uses, bound to the pane's document session — content follows workspace
 * selection with no second document pipeline and no coding-agent editor
 * branch. The workspace browser tree lives only in the left workspace
 * tab. No file selected → the empty state with Open file actions.
 */
function FilesTab({
  session,
  sendIntent,
  sessions,
}: {
  session: DocumentSession | null;
  sendIntent: (commandId: string) => void;
  sessions: readonly Record<string, unknown>[];
}) {
  const meta = useSyncExternalStore(
    session ? session.store.subscribe : () => () => undefined,
    session ? session.store.get : () => null,
    () => null,
  );
  if (!session || (!meta?.path && session.snapshotDoc().length === 0)) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          Open a file to work on it alongside the agent.
        </ClayText>
        <ClayButton onPress={() => sendIntent("documents.clientOpenFileDialog")}>
          Open file
        </ClayButton>
        <ClayButton
          variant="muted"
          onPress={() => sendIntent("agent.clientOpenSessionPicker")}
        >
          Resume session
        </ClayButton>
        <ClayButton
          variant="muted"
          onPress={() => sendIntent("agent.clientOpenSessionSearchPicker")}
        >
          Search sessions…
        </ClayButton>
        {sessions.length > 0 && (
          <ul className={styles.sessionList} aria-label="Recent sessions">
            {sessions.slice(0, 5).map((record) => (
              <li key={String(record["id"])} className={styles.sessionRow}>
                <ClayText variant="detail" muted>
                  {String(record["id"]).slice(0, 12)}
                </ClayText>
                <ClayIconButton
                  icon="session.resume"
                  label={`Resume ${String(record["id"])}`}
                  variant="muted"
                  onPress={() =>
                    void sendRequest(
                      agentCommandPayload({
                        resumeSession: { sessionId: String(record["id"]) },
                      }),
                    )
                  }
                />
              </li>
            ))}
          </ul>
        )}
      </div>
    );
  }
  // Same document surface the shell mounts for the pane; the pane's
  // editor is unmounted while the agent surface is open, so this is the
  // session's only EditorView (one view per mount).
  return <ClayEditor session={session} />;
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
  const groupsWithClear =
    firstGroup
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
  const selectWorker = (worker: "observation" | "reflection") => (id: string) => {
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
  const stale = view && typeof view.sessionId === "string" && view.sessionId !== sessionId;
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
                className={`${styles.omKind} ${styles[
                  `omKind_${row.kind}` as keyof typeof styles
                ] ?? ""}`}
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
  detail,
}: {
  sessionId: string;
  open: boolean;
  transcriptLength: number;
  view: ContextView | null;
  detail: ContextItemDetail | null;
}) {
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
  const shownDetail =
    detail && requestedItem === detail.itemId ? detail : null;

  if (!sessionId) {
    return (
      <div className={styles.tabEmpty}>
        <ClayText variant="body" muted>
          No active agent session.
        </ClayText>
      </div>
    );
  }
  // First response not in yet (or stale view from a previous session).
  if (!view || (typeof view.sessionId === "string" && view.sessionId !== sessionId)) {
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
            <ClayButton variant="muted" onPress={() => setRequestedItem(null)}>
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
                  …and {drawerCategory.count - drawerCategory.items.length} more
                  (not listed)
                </ClayText>
              </li>
            )}
          </ul>
        )}
      </div>
    );
  }
  return (
    <dl className={styles.contextList}>
      {categories.map((category) => (
        <div key={category.kind} className={styles.contextRow}>
          <dt>
            <button
              type="button"
              className={styles.contextItem}
              onClick={() => setDrawer(category.kind)}
            >
              {category.label}
            </button>
          </dt>
          <dd>
            <ClayText variant="detail" muted>
              {category.count}
            </ClayText>
          </dd>
        </div>
      ))}
      {categories.length === 0 && (
        <div className={styles.contextRow}>
          <dt>
            <ClayText variant="detail" muted>
              Loading context…
            </ClayText>
          </dt>
        </div>
      )}
    </dl>
  );
}

/** Typed agent-family request through the validated bridge path. */
function agentCommandPayload(command: Record<string, unknown>): string {
  return JSON.stringify({
    family: "agent",
    payload: { clientId: 0, command },
  });
}
