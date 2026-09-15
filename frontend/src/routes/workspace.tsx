import { lazy, Suspense, useSyncExternalStore, type ReactNode } from "react";

import { ClayButton, ClayText } from "../components";
import { useSessionConnection } from "../app/use-clay-session";
import { workspaceRail } from "../shell/layout-state";
import { useShellChords } from "../shell/use-shell-chords";
import { workspace } from "../shell/workspace-singleton";
import type { DocumentSession } from "../editor/sync/session";
import type { ConnectionState } from "../state/connection-store";
import { WorkspaceRail } from "./WorkspaceRail";

const WorkspacePanes = lazy(async () => {
  const module = await import("../shell/WorkspacePanes");
  return { default: module.WorkspacePanes };
});

import styles from "./workspace.module.css";

export interface WorkspaceRouteProps {
  /** Optional test override; production reads the session store. */
  connection?: ConnectionState;
  onReconnect: () => void;
}

/**
 * The workspace view's composition: the routed surface plus the optional right
 * rail (DESIGN.md §12). Exported so the DEV review fixture renders the same
 * grid instead of its own copy.
 */
export function WorkspaceView({
  session,
  children,
  showRail = true,
}: {
  session: DocumentSession | null;
  children: ReactNode;
  showRail?: boolean;
}) {
  const railEnabled = useSyncExternalStore(
    workspaceRail.subscribe,
    workspaceRail.isVisible,
  );
  const railVisible = showRail && railEnabled;
  return (
    <div
      className={styles.view}
      data-rail={railVisible ? "expanded" : "collapsed"}
    >
      <div className={styles.viewMain}>{children}</div>
      {railVisible ? <WorkspaceRail session={session} /> : null}
    </div>
  );
}

/**
 * Live workspace: per-tab split tree + editor panes.
 */
export function WorkspaceRoute({
  connection: injected,
  onReconnect,
}: WorkspaceRouteProps) {
  const live = useSessionConnection();
  const connection = injected ?? live;
  // The rail describes the active workspace pane, never the agent view.
  const tabs = useSyncExternalStore(workspace.subscribe, workspace.getSnapshot);
  useShellChords(workspace, connection.phase === "ready");
  const activeRuntime = workspace.active();
  const activeTab = tabs.tabs.find(
    (tab) => tab.clientId === activeRuntime?.clientId,
  );
  const railSession =
    activeRuntime?.panes.get(activeRuntime.tree.activePaneId)?.session ?? null;

  if (connection.phase === "disconnected") {
    return (
      <div className={`${styles.workspace} ${styles.stack}`} role="alert">
        <ClayText variant="title">Session lost</ClayText>
        <ClayText variant="body" muted>
          {connection.reason}
        </ClayText>
        <ClayButton variant="primary" onPress={onReconnect}>
          Reconnect session
        </ClayButton>
      </div>
    );
  }
  if (connection.phase !== "ready") {
    return (
      <div className={`${styles.workspace} ${styles.stack}`} role="status">
        <ClayText variant="title">Connecting to Clay server…</ClayText>
      </div>
    );
  }

  return (
    <WorkspaceView
      session={railSession}
      showRail={activeTab?.view !== "agent"}
    >
      <Suspense
        fallback={
          <div className={`${styles.workspace} ${styles.stack}`} role="status">
            <ClayText variant="title">Loading editor…</ClayText>
          </div>
        }
      >
        <WorkspacePanes workspace={workspace} />
      </Suspense>
    </WorkspaceView>
  );
}
