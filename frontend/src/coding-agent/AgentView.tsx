// The agent view of a tab (plan 118 task 33, DESIGN.md §12): one tab holds one
// workspace and one agent, and this is the agent's half — the same tab the
// workspace view edits, reached from the titlebar switcher or Ctrl+2.
//
// It renders the trusted `@clay/coding-agent` surface through the same
// provenance-exact lookup the pane path used (an untrusted same-named package
// never reaches the host panel), or an SDUI surface for a third-party agent
// package. The panel keeps the tab's active pane session, so its Files tab and
// the editor still follow the workspace selection.

import { lazy, Suspense } from "react";

import { ClayText } from "../components";
import type { AgentSessionModule } from "../agent/state";
import type { PackageSurface, PackageUiSnapshot } from "../sdui/types";
import type { DocumentSession } from "../editor/sync/session";
import { hostRenderedSurface } from "../shell/PaneTree";
import type { EffortChord } from "./CodingAgentPanel";

import styles from "./agent-view.module.css";

const PackageSurfaceView = lazy(async () => {
  const module = await import("../sdui/registry");
  return { default: module.PackageSurfaceView };
});

const CodingAgentPanel = lazy(async () => {
  const module = await import("./CodingAgentPanel");
  return { default: module.CodingAgentPanel };
});

export interface AgentViewProps {
  packageUi: PackageUiSnapshot | null;
  uiVersion: number;
  /** Folder the tab is about (the session's own root when nothing is picked). */
  workspaceRoot: string;
  /** Active pane session: the Settings tab lists the agent's delivered files
   *  through it (plan 118 task 36 made the Files tab session history). */
  session: DocumentSession | null;
  /** The tab's agent session store (plan 119 SC-6), once its agent view has
   *  mounted one; the panel creates and adopts it on first mount. */
  agent: AgentSessionModule | null;
  /** The tab's connection id (the store's delivery filter) and the tab-stamped
   *  sender its agent intents ride. */
  agentClientId: number | null;
  /** Adopt the store the panel created into the tab runtime. */
  onAgentStore?: ((store: AgentSessionModule) => void) | null;
  /** SDUI package surfaces send through the pane session; the coding-agent
   *  panel uses its tab store instead. */
  send: ((payload: string) => Promise<void>) | null;
  effortChord: EffortChord | null;
  /** Agent working / idle: the tab strip's marker pulses while busy. */
  onBusyChange?: (busy: boolean) => void;
  /** Plan 118 task 35: the tab's agent type (`null` = none picked yet); the
   *  agent view's title is the picker for it. */
  agentType?: string | null;
  /** Switch the tab's agent through tab chrome (the shell sends the
   *  server-validated `tabCommand`; the tab's workspace is untouched). */
  onPickAgent?: ((agent: string | null) => void) | null;
  /** Plan 118 task 36: open a path in the tab's workspace view — the Files
   *  tab's row action. `null` keeps the rows inert (fixtures). */
  onOpenInWorkspace?: ((path: string) => void) | null;
}

/** The trusted coding-agent surface, if the package contributed one. */
export function codingAgentSurfaceOf(
  packageUi: PackageUiSnapshot | null,
): PackageSurface | null {
  return (
    packageUi?.surfaces?.find(
      (surface) => hostRenderedSurface(surface) === "coding-agent",
    ) ?? null
  );
}

export function AgentView({
  packageUi,
  uiVersion,
  workspaceRoot,
  session,
  agent,
  agentClientId,
  onAgentStore = null,
  send,
  effortChord,
  onBusyChange,
  agentType = null,
  onPickAgent = null,
  onOpenInWorkspace = null,
}: AgentViewProps) {
  const surface = codingAgentSurfaceOf(packageUi);
  if (!surface) {
    // No agent half yet (the in-tab picker is plan 118 task 35; today an agent
    // is attached from the launcher) — or the agent package is not installed.
    return (
      <div className={styles.empty} role="status">
        <ClayText variant="title">No agent in this tab</ClayText>
        <ClayText variant="body" muted>
          Open the launcher with Ctrl+T to attach one. The tab keeps its
          workspace either way.
        </ClayText>
      </div>
    );
  }
  return (
    <Suspense
      fallback={
        <div className={styles.empty} role="status">
          <ClayText variant="body" muted>
            Loading agent…
          </ClayText>
        </div>
      }
    >
      {hostRenderedSurface(surface) === "coding-agent" ? (
        <CodingAgentPanel
          surface={surface}
          uiVersion={uiVersion}
          workspaceRoot={workspaceRoot}
          session={session}
          agent={agent}
          agentClientId={agentClientId}
          onAgentStore={onAgentStore}
          send={send ?? undefined}
          effortChord={effortChord}
          onBusyChange={onBusyChange}
          agentType={agentType}
          onPickAgent={onPickAgent}
          onOpenInWorkspace={onOpenInWorkspace}
        />
      ) : send ? (
        <PackageSurfaceView
          surface={surface}
          uiVersion={uiVersion}
          send={send}
        />
      ) : (
        <div className={styles.empty} role="status">
          <ClayText variant="body" muted>
            This tab has no session to carry the agent intents.
          </ClayText>
        </div>
      )}
    </Suspense>
  );
}
