import { useSyncExternalStore, type ReactNode } from "react";
import { Outlet, useLocation } from "react-router";

import { ClayText } from "../../components";
import { workspace } from "../../shell/workspace-singleton";
import { useSessionConnection } from "../use-clay-session";
import styles from "./shell.module.css";
import { TabBar, type ShellTab } from "./tab-bar";
import { WorkingArea } from "./working-area";

export interface AppShellProps {
  tabs?: ShellTab[];
  activeTabId?: string | null;
  onActivateTab?: (id: string) => void;
  /** Optional test override; production reads the session store. */
  status?: string;
  left?: ReactNode;
}

function statusFromPhase(phase: string): string {
  if (phase === "ready") return "Connected";
  if (phase === "disconnected") return "Disconnected";
  return "Connecting…";
}

/**
 * Application shell landmarks: header (brand + tab strip), main working
 * area (routed), footer status bar. One `main` landmark per window; tabs,
 * panes, and overlays remain application state.
 */
export function AppShell({
  tabs: injectedTabs,
  activeTabId: injectedActive,
  onActivateTab,
  status,
  left,
}: AppShellProps) {
  const location = useLocation();
  const live = useSessionConnection();
  const snapshot = useSyncExternalStore(
    workspace.subscribe,
    workspace.getSnapshot,
  );
  const activeDiagnostic = workspace.active()?.diagnostic;
  const resolvedStatus =
    status ?? activeDiagnostic?.message ?? statusFromPhase(live.phase);

  const liveTabs: ShellTab[] = snapshot.tabs.map((tab) => ({
    id: String(tab.clientId),
    label: tab.label,
    dirty: tab.dirty,
    closable: snapshot.tabs.length > 1,
  }));
  const tabs =
    injectedTabs ??
    (liveTabs.length > 0 ? liveTabs : [{ id: "main", label: "Workspace" }]);
  const activeTabId =
    injectedActive ??
    (snapshot.activeClientId != null
      ? String(snapshot.activeClientId)
      : (tabs[0]?.id ?? null));

  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <span className={styles.brand}>CLAY</span>
        <TabBar
          tabs={tabs}
          activeId={activeTabId}
          onActivate={(id) => {
            if (onActivateTab) {
              onActivateTab(id);
              return;
            }
            const clientId = Number(id);
            if (Number.isFinite(clientId)) void workspace.activate(clientId);
          }}
          onClose={(id) => {
            const clientId = Number(id);
            if (Number.isFinite(clientId)) workspace.requestClose(clientId);
          }}
          onNew={
            injectedTabs ? undefined : () => void workspace.openTabDialog()
          }
        />
      </header>
      <main className={styles.workingArea} aria-label="Clay workspace">
        <WorkingArea left={left}>
          <Outlet />
        </WorkingArea>
      </main>
      <footer className={styles.footer}>
        <ClayText variant="status" muted>
          {location.pathname}
        </ClayText>
        <span role="status" aria-live="polite">
          <ClayText variant="status" muted data-testid="shell-status">
            {resolvedStatus}
          </ClayText>
        </span>
      </footer>
    </div>
  );
}
