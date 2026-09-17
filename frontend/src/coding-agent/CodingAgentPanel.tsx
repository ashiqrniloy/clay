// Coding Agent split surface (plan 108 task 8) — binding-spec presentation
// for the bundled `@clay/coding-agent` package.
//
// Provenance-exact host rendering, mirroring the SettingsPanel
// precedent: every dynamic value rides the ONE core-owned AG-UI stream (the
// tab's own agent store — plan 119 SC-6); every interaction emits declared
// inert command intents through that tab's connection. Third-party
// replacements render through the unchanged generic SDUI renderer
// (PaneTree), never this module.
//
// Layout (plan 124 task 6): the tab's agent *view* is the transcript, the state
// strip, and the inspector. Its composer, agent controls, and session foot moved
// to the shell's persistent agent lane (`../shell/AgentLane`), which mounts once
// per tab for both views — so a prompt belongs to the tab, not to this view, and
// there is exactly one place the agent, model, and effort are changed.
//
// Effort control (plan 109 I4): the daemon owns the portable thinking level —
// STATE carries the model's declared levels + the active one; the lane owns the
// dropdown and the manifest-bound cycle chord.

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { createAgentSession, type AgentSessionModule } from "../agent/state";
import type { DocumentSession } from "../editor/sync/session";
import { sduiActionPayload } from "../sdui/actions";
import type { PackageSurface } from "../sdui/types";
import { recipeAttributes } from "../components/recipe-attributes";
import { agentInspector } from "../shell/layout-state";
import { InspectorTabs } from "./InspectorTabs";
import { useAgentSurfaceState } from "./surface-state";
import { TranscriptList } from "./TranscriptList";

import styles from "./coding-agent.module.css";

export type { EffortChord } from "./Composer";
export {
  compactTokens,
  estimateContextTokens,
  groupModelsByProvider,
} from "./surface-state";

export interface CodingAgentPanelProps {
  surface: PackageSurface;
  /** Current runtime package UI version; validates every intent. */
  uiVersion: number;
  /** The pane's document session: the Settings tab lists the agent's
   *  delivered files through it. */
  session?: DocumentSession | null;
  /** The tab's agent session store (plan 119 SC-6). The shell's host resolves
   *  and adopts it (plan 124), so the lane and this view read the same one; a
   *  standalone mount (fixtures, tests) keeps its own. */
  agent?: AgentSessionModule | null;
  /** Connection id of the tab the panel is rendered for (the store's delivery
   *  filter); `null` accepts every delivery (standalone mounts). */
  agentClientId?: number | null;
  /** Adopt the store a standalone mount created (no-op with a host store). */
  onAgentStore?: ((store: AgentSessionModule) => void) | null;
  /** The tab's own sender — construction-scoped into the store's agent so its
   *  prompts/cancel/steer ride this tab's connection. */
  send?: (payload: string) => Promise<void>;
  /** Plan 118 task 36: open a path in the tab's *workspace* view — the Files
   *  tab's row action (the shell switches the view and opens the document). */
  onOpenInWorkspace?: ((path: string) => void) | null;
}

export function CodingAgentPanel({
  surface,
  uiVersion,
  session = null,
  agent = null,
  agentClientId = null,
  onAgentStore = null,
  send,
  onOpenInWorkspace = null,
}: CodingAgentPanelProps) {
  // Stable across renders so the inspector's rows do not re-bind; a panel
  // without a shell (fixtures) keeps the rows but they lead nowhere.
  const openInWorkspace = useCallback(
    (path: string) => onOpenInWorkspace?.(path),
    [onOpenInWorkspace],
  );
  // The tab's store lives as long as the tab. In the app the host owns it
  // (plan 124: the lane is mounted for every tab, so it cannot depend on this
  // view having been shown); a standalone mount keeps its own, and keeps
  // asking the server for the tab's STATE itself.
  const created = useRef<AgentSessionModule | null>(null);
  const standalone = agent === null;
  const store =
    agent ??
    (created.current ??= createAgentSession({
      send: send ?? undefined,
      clientId: agentClientId,
    }));
  useEffect(() => {
    onAgentStore?.(store);
  }, [onAgentStore, store]);
  useEffect(() => {
    store.agent.setUiVersion(uiVersion);
  }, [store, uiVersion]);
  useEffect(() => {
    if (!standalone) return;
    store.command("listSessions");
    // Plan 117 follow-up: ask for the tab's STATE at mount. Nothing else
    // emits a snapshot before the first prompt, so without this the status
    // row showed `git —` and the skills/MCP cards stayed empty until the
    // first message (the daemon environment needs no session). The same
    // answer carries the tab's session binding (plan 119 SC-6), which is
    // what the store filters the relay with.
    store.requestBinding();
  }, [standalone, store]);

  const {
    snapshot,
    transcript,
    sessionFileList,
    configured,
    streaming,
    sessionId,
    modelGroups,
    skills,
    mcpServers,
    contextView,
    contextDetail,
    omView,
    pendingApproval,
  } = useAgentSurfaceState(store);
  const command = store.command;

  const [selectedId, setSelectedId] = useState("");
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
  const [activeTab, setActiveTab] = useState("files");
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
  const selected = transcript.find((box) => box.id === selectedId) ?? null;

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

  // State strip (DESIGN.md §12): the run's own status line — and, while a turn
  // is in flight, the three working bars in place of the tone dot. The text is
  // a fact about the run, never invented progress.
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
      // The declared surface's own label names the region (provenance-exact
      // host rendering: the copy comes from the package's tree).
      aria-label={surface.component.title?.trim() || "Coding Agent"}
      data-coding-agent-surface
    >
      <div
        className={styles.agent}
        data-inspector={inspectorVisible ? "expanded" : "collapsed"}
      >
        <main className={styles.agentColumn}>
          <TranscriptList
            boxes={transcript}
            selectedId={selectedId}
            onSelect={selectCard}
          />

          <div className={styles.stateStrip} role="status">
            {streaming ? (
              // The run's motion in words (§7): three accent bars at the
              // text's cap height, decoration over the strip's own text.
              <span
                className={styles.workingBars}
                aria-hidden
                data-working-bars
              >
                <span />
                <span />
                <span />
              </span>
            ) : (
              <span
                className={styles.statusDot}
                data-tone={stateTone}
                aria-hidden
                {...recipeAttributes("statusDot", "root")}
              />
            )}
            <span className={styles.stateText}>{stateText}</span>
            <span className={styles.spacer} />
            <span className={styles.stateNote}>{stateNote}</span>
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
          contextDetail={contextDetail}
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
