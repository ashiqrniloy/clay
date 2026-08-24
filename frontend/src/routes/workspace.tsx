import { lazy, Suspense } from "react";

import { ClayButton, ClayText } from "../components";
import { useSessionConnection } from "../app/use-clay-session";
import { useShellChords } from "../shell/use-shell-chords";
import { workspace } from "../shell/workspace-singleton";
import type { ConnectionState } from "../state/connection-store";

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
 * Live workspace: per-tab split tree + editor panes.
 */
export function WorkspaceRoute({
  connection: injected,
  onReconnect,
}: WorkspaceRouteProps) {
  const live = useSessionConnection();
  const connection = injected ?? live;
  useShellChords(workspace, connection.phase === "ready");

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
    <div className={styles.editor}>
      <Suspense
        fallback={
          <div className={`${styles.workspace} ${styles.stack}`} role="status">
            <ClayText variant="title">Loading editor…</ClayText>
          </div>
        }
      >
        <WorkspacePanes workspace={workspace} />
      </Suspense>
    </div>
  );
}
