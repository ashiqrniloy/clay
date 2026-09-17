// Agent lane (plan 124 task 6): the shell's one bottom section, mounted once
// per tab beside the two view slots. It is the same lane in both views, so a
// prompt belongs to the *tab*, not to the agent view — a document can prompt
// the tab's agent without leaving the workspace view (DESIGN.md §12).
//
// Composition, top to bottom: the approval strip when a tool call is
// suspended, the composer box (the field's row, then the tab's own agent
// controls as one toolbar row *inside* that box, then the hint row), and the
// session-environment foot. The lane never carries a run indicator: the run's
// motion is the window mark's dot (this tab's `agentBusy`) and the agent view's
// working bars.
//
// Interaction authority lives here: the client built-ins, steer-vs-prompt, the
// run lifecycle, and the approval decision — all through the tab's own store.

import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from "react";

import { ClayDropdown, ClayText } from "../components";
import { ApprovalStrip } from "../coding-agent/ApprovalStrip";
import { Composer, type EffortChord } from "../coding-agent/Composer";
import { useAgentSurfaceState } from "../coding-agent/surface-state";
import { agentLabel } from "../coding-agent/transcript-model";
import { agentCommandPayload, type AgentSessionModule } from "../agent/state";
import { sduiActionPayload } from "../sdui/actions";
import type { DocumentSession } from "../editor/sync/session";
import {
  launcherEntriesFrom,
  type LauncherAgentEntry,
} from "../launcher/LauncherPanel";
import { agentLane } from "./layout-state";
import type { ComposerPalette } from "../coding-agent/Composer";

import styles from "./agent-lane.module.css";

/** The agent-less picker's placeholder row: the trigger reads as the offer.
 *  Not a real agent id, so selecting it is a no-op. */
const ATTACH_AGENT = "Attach an agent";

export interface AgentLaneProps {
  /** The tab's own agent store (the host resolves and adopts it, so the lane
   *  and the agent view read the same one). */
  store: AgentSessionModule;
  /** Current runtime package UI version; validates every intent. */
  uiVersion: number;
  /** Folder the tab is about — the foot's root and the Files tab's subject. */
  workspaceRoot: string;
  /** Active pane session: the agent-type picker offers the same server
   *  enumeration the launcher lists (one directory scan, no package load). */
  session?: DocumentSession | null;
  /** Effective effort-cycle chord from the behavior manifest
   *  (`coding-agent.clientCycleEffort`); `Shift+Tab` when unbound. */
  effortChord?: EffortChord | null;
  /** The tab's agent type (`null` = none picked yet): the picker's value, and
   *  what makes the field inert until one is attached. */
  agentType?: string | null;
  /** Switch the tab's agent (tab chrome: the shell sends the
   *  server-validated command). `null` detaches the agent. */
  onPickAgent?: ((agent: string | null) => void) | null;
  /** The tab's run state for the window mark (`tab.agentBusy`). */
  onBusyChange?: (busy: boolean) => void;
  /** The command palette's session and intents (plan 124): the composer's
   *  field is the palette's query, so the lane is where it is driven from. */
  palette: ComposerPalette;
  /** Reports whether one of the field's menus is up: the shell veils the
   *  working area while one is. */
  onFieldMenuOpen?: (open: boolean) => void;
}

export function AgentLane({
  store,
  uiVersion,
  workspaceRoot,
  session = null,
  effortChord = null,
  agentType = null,
  onPickAgent = null,
  onBusyChange,
  palette,
  onFieldMenuOpen,
}: AgentLaneProps) {
  const surface = useAgentSurfaceState(store);
  // Plan 124: `Ctrl+X Ctrl+P` flips this tab's lane visibility (the shell owns
  // the chord; this is the state it flips). Hidden keeps the draft and the
  // pickers alive — a state, not a deletion (DESIGN.md §12). A palette session
  // reveals the lane: the palette *is* the composer's menu, so a window without
  // the lane has nowhere to draw it (the artifact's chord does the same).
  const laneVisible = useSyncExternalStore(
    agentLane.subscribe,
    agentLane.isVisible,
  );
  const paletteOpen = palette.menu !== null;
  useEffect(() => {
    if (paletteOpen) agentLane.setVisible(true);
  }, [paletteOpen]);
  const {
    snapshot,
    command,
    configured,
    streaming,
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
    pendingApproval,
  } = surface;

  // Plan 109 I4: the pending effort level the next prompt carries (cycles via
  // the manifest-bound chord, settable via the dropdown); the session's active
  // level comes from STATE. Levels: declared levels for the model, empty →
  // no control, chord is a no-op.
  const [pendingEffort, setPendingEffort] = useState<string | null>(null);
  const shownEffort = pendingEffort ?? activeEffort;

  // Plan 118 task 35: the configured agent types come from the same
  // server-resolved listing the launcher renders.
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
  // it into an agent-less tab, so the tab's record and the lane agree. Only
  // from the *unknown* side: a pick the user just made is never reverted while
  // the session catches up.
  useEffect(() => {
    if (!onPickAgent || agentType !== null || !sessionAgent) return;
    onPickAgent(sessionAgent);
  }, [agentType, onPickAgent, sessionAgent]);

  // The menu never lists an agent the server did not list; a tab whose agent
  // is gone (deleted folder) still shows its own row so the state is visible.
  // With no agent at all the picker itself is the offer: the placeholder row
  // makes the trigger read `Attach an agent` (DESIGN.md §12) and selecting it
  // is a no-op.
  const agentOptions = useMemo(() => {
    const rows = agentEntries.map((entry) => ({
      id: entry.name,
      label: entry.label,
    }));
    if (currentAgent && !rows.some((row) => row.id === currentAgent)) {
      rows.unshift({ id: currentAgent, label: agentLabel(currentAgent) });
    }
    if (!currentAgent) rows.unshift({ id: ATTACH_AGENT, label: ATTACH_AGENT });
    return rows;
  }, [agentEntries, currentAgent]);
  const skillCount = useMemo(
    () =>
      agentEntries.find((entry) => entry.name === currentAgent)?.skillCount ??
      null,
    [agentEntries, currentAgent],
  );

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

  // The lane owns the send authority: client built-ins, steer-vs-prompt, and
  // the run lifecycle. The composer owns the text, so this answers whether the
  // input was consumed (`false` = ignored, keep the draft).
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
      // No provider (or no agent): the input is not consumed, so the draft
      // survives — the foot says why nothing sends (DESIGN.md §12).
      if (!configured) return false;
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
    [configured, pendingEffort, sendIntent, store, streaming],
  );

  const stopRun = useCallback(() => store.agent.abortRun(), [store]);

  // Tool approval (durable runs): Allow/Deny strip for suspended runs.
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

  // Plan 124: report the tab's run state to the shell — the window mark's dot
  // pulses while this tab works (the lane is the always-mounted reporter, so a
  // hidden agent view still marks its tab).
  useEffect(() => {
    onBusyChange?.(streaming === true);
  }, [onBusyChange, streaming]);

  // The foot states the *environment*, plus the one reason nothing would send.
  // Never a run cue (DESIGN.md §12).
  const laneNote = !currentAgent
    ? "no agent on this tab · the lane keeps its place"
    : !configured
      ? "no provider configured · Settings → Providers"
      : "";
  const mcpSummary =
    mcpServers.length > 0
      ? `MCP ${mcpServers
          .map((server) =>
            server.connected
              ? `${server.serverId} · ${server.tools} ${
                  server.tools === 1 ? "tool" : "tools"
                }`
              : `${server.serverId} · hidden`,
          )
          .join(" · ")}`
      : "MCP none";

  return (
    <footer
      className={styles.lane}
      data-clay-ds="shell.default.footer.rest"
      aria-label="Agent lane"
      hidden={!laneVisible}
    >
      {pendingApproval && (
        <ApprovalStrip approval={pendingApproval} onResolve={resolveApproval} />
      )}
      <Composer
        agentless={!currentAgent}
        streaming={streaming}
        sessionId={sessionId}
        command={command}
        palette={palette}
        onFieldMenuOpen={onFieldMenuOpen}
        skills={skills}
        files={files}
        effortLevels={effortLevels}
        shownEffort={shownEffort}
        onEffortChange={setPendingEffort}
        effortChord={effortChord}
        onStop={stopRun}
        onSubmit={submit}
        toolbar={
          <div className={styles.controls}>
            {/* The tab's agent: the picker *is* the control (plan 118 task 35),
                and the trigger is the `agentPicker` family — one place, both
                views (DESIGN.md §12). */}
            <span className={styles.pick} data-agent-pick>
              <ClayDropdown
                label="Agent type"
                triggerFamily="agentPicker"
                selectedHint="current"
                options={agentOptions}
                selectedId={currentAgent ?? ATTACH_AGENT}
                disabled={!onPickAgent || agentEntries.length === 0}
                onSelect={(id) => {
                  if (!onPickAgent || !id || id === ATTACH_AGENT) return;
                  if (id === currentAgent) return;
                  // Switching resets the effort control: the level belonged to
                  // the previous agent's model (the model itself follows the new
                  // agent's own default from the server).
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
            {/* The model/effort/meter trio describes the *agent's* run, so a
                tab with no agent shows none of them (approved agent-less
                state). With a provider: the grouped configured-model
                inventory; without one: the trigger states the reason (the
                field stays typeable — review 2026-09-17). */}
            {!currentAgent ? null : modelGroups.length > 0 && configured ? (
              <ClayDropdown
                label="Model"
                options={[]}
                groups={modelGroups}
                selectedId={activeModelId}
                onSelect={(id) =>
                  store.command({ select: { kind: "model", id } })
                }
              />
            ) : configured ? (
              // A provider is configured but the models inventory has not
              // answered yet: state the pair as data (never a control that
              // cannot act), muted, in the same row.
              <span className={styles.ctl} data-model-pair>
                <ClayText variant="caption" muted>
                  {provider && model
                    ? `${provider}/${model}`
                    : String(snapshot.state["profile"] ?? "coding")}
                </ClayText>
              </span>
            ) : (
              <span className={styles.ctl} data-model-missing>
                <ClayDropdown
                  label="Model"
                  options={[{ id: "", label: "Configure a provider" }]}
                  selectedId=""
                  disabled
                  onSelect={() => undefined}
                />
              </span>
            )}
            {/* Declared thinking levels for the active model: the selected
                level rides the next prompt (plan 109 I4). */}
            {currentAgent && effortLevels && effortLevels.length > 0 && (
              <ClayDropdown
                label="Effort"
                options={effortLevels.map((level) => ({
                  id: level,
                  label: level,
                }))}
                selectedId={shownEffort}
                onSelect={setPendingEffort}
              />
            )}
            <span className={styles.spacer} />
            {/* Context used vs the model's ceiling (plan 117): real values
                only; the meter travels with the model it measures. */}
            {currentAgent && meter ? (
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
            ) : currentAgent && lastUsageText.length > 0 ? (
              <span className={styles.meterValue}>{lastUsageText}</span>
            ) : null}
          </div>
        }
      />
      <div className={styles.foot} aria-label="Session environment">
        {laneNote.length > 0 && (
          <>
            <span className={styles.note}>{laneNote}</span>
            <span className={styles.faint}>·</span>
          </>
        )}
        <span>{workspaceRoot}</span>
        <span className={styles.faint}>·</span>
        <span>{`git ${gitBranch || "—"}`}</span>
        {extensions.length > 0 && (
          <>
            <span className={styles.faint}>·</span>
            <span>{`extensions ${extensions.join(", ")}`}</span>
          </>
        )}
        <span className={styles.spacer} />
        {/* One summary segment: the per-server detail is the Context tab's,
            and this line only says what is connected. */}
        <span data-tone={connectedMcpServers.length > 0 ? "ok" : "muted"}>
          {mcpSummary}
        </span>
      </div>
    </footer>
  );
}
