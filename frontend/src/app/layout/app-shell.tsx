import { useSyncExternalStore, type ReactNode } from "react";
import { Outlet, useLocation } from "react-router";

import { ClayIconButton, ClayKbd } from "../../components";
import { workspace } from "../../shell/workspace-singleton";
import { tabTitle } from "../../shell/tab-store";
import { agentLane, workspaceRail } from "../../shell/layout-state";
import { useSessionConnection } from "../use-clay-session";
import styles from "./shell.module.css";
import { TabBar, type ShellTab } from "./tab-bar";
import { ViewSwitcher } from "./view-switcher";
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

/** Workspace label for the status bar: the folder's basename, never a path. */
function workspaceLabel(root: string | undefined): string {
  if (!root) return "";
  const parts = root.split(/[\\/]/).filter(Boolean);
  return parts.at(-1) ?? "";
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
  const railVisible = useSyncExternalStore(
    workspaceRail.subscribe,
    workspaceRail.isVisible,
  );
  const laneVisible = useSyncExternalStore(
    agentLane.subscribe,
    agentLane.isVisible,
  );
  const activeRuntime = workspace.active();
  const activeDiagnostic = activeRuntime?.diagnostic;
  const activePane = activeRuntime?.panes.get(activeRuntime.tree.activePaneId);
  const activeMeta = activePane?.session.store.get() ?? null;
  // Progressive chunk load in flight: a transient, server-backed phase.
  const documentLoading = [...(activeRuntime?.panes.values() ?? [])].some(
    (pane) => pane.session.store.get()?.loading,
  );
  // Transient shell diagnostics (dialog/file failures) outrank the steady
  // connection phase: a silent failure reads exactly like a dead button.
  const resolvedStatus =
    activeDiagnostic?.message ??
    (documentLoading
      ? "Loading document…"
      : (status ?? statusFromPhase(live.phase)));

  const activeTab =
    snapshot.tabs.find((tab) => tab.clientId === snapshot.activeClientId) ??
    null;
  const liveTabs: ShellTab[] = snapshot.tabs.map((tab) => ({
    id: String(tab.clientId),
    label: tab.label,
    title: tabTitle(tab.workspaceRoot, tab.agent),
    dirty: tab.dirty,
    closable: snapshot.tabs.length > 1,
    // Approved marker: the mono word inside the tab, not a second badge.
    agent: tab.agent ? { busy: tab.agentBusy } : null,
  }));
  const tabs =
    injectedTabs ??
    (liveTabs.length > 0 ? liveTabs : [{ id: "main", label: "Workspace" }]);
  const activeTabId =
    injectedActive ??
    (snapshot.activeClientId != null
      ? String(snapshot.activeClientId)
      : (tabs[0]?.id ?? null));

  const hints = [
    {
      id: "lane",
      label: laneVisible ? "hide lane" : "lane",
      keys: "Ctrl X P",
      // Same toggle the chord and the palette row run (plan 124).
      run: () => agentLane.toggle(),
    },
    {
      id: "palette",
      label: "palette",
      keys: "Ctrl X O",
      run: () => workspace.dispatchServerCommand("controlCenter.open"),
    },
    {
      id: "files",
      label: "files",
      keys: "Ctrl B",
      run: () => workspace.dispatchServerCommand("workspace.toggleFileBrowser"),
    },
    {
      id: "outline",
      label: railVisible ? "hide outline" : "outline",
      keys: "Ctrl I",
      run: () => workspaceRail.toggle(),
    },
    {
      id: "filter",
      label: "filter",
      keys: "/",
      run: () => {
        document
          .querySelector<HTMLInputElement>("[data-clay-list-filter] input")
          ?.focus();
      },
    },
  ];

  return (
    <div className={styles.shell}>
      <header className={styles.header} data-clay-ds="shell.header">
        {/* The window mark carries the window's run signal (plan 124): its dot
            pulses while this tab works — the one looping pulse a window has
            (DESIGN.md §7/§9). The tab's own marker is colour only. */}
        <span
          className={styles.brand}
          data-busy={activeTab?.agentBusy ? "true" : "false"}
        >
          Clay
        </span>
        <div className={styles.tabs}>
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
            onNew={injectedTabs ? undefined : () => void workspace.newTab()}
          />
        </div>
        <span className={styles.titlebarSpacer} />
        <nav className={styles.actions} aria-label="Application controls">
          <ClayIconButton
            icon="control-center.open"
            label="Control Center"
            shortcut="Ctrl+X Ctrl+O"
            variant="muted"
            onPress={() =>
              workspace.dispatchServerCommand("controlCenter.open")
            }
          />
          {injectedTabs ? null : (
            <ViewSwitcher
              view={activeTab?.view ?? "workspace"}
              hasWorkspace={Boolean(activeTab?.workspaceRoot)}
              hasAgent={Boolean(activeTab?.agent)}
              onSelect={(view) => workspace.setView(view)}
            />
          )}
        </nav>
      </header>
      <main className={styles.workingArea} aria-label="Clay workspace">
        <WorkingArea left={left}>
          <Outlet />
        </WorkingArea>
      </main>
      <footer className={styles.footer} data-clay-ds="statusBar.root">
        <span className={styles.statusGroup} data-testid="shell-location">
          {workspaceLabel(
            activeTab?.workspaceRoot || activeRuntime?.sessionRoot,
          ) || location.pathname}
        </span>
        {activeMeta?.path ? (
          <span className={styles.statusGroup}>
            <span className={styles.statusValue}>{activeMeta.path}</span>
            <span>{`v${activeMeta.version}`}</span>
            <span>{activeMeta.dirty ? "dirty" : "clean"}</span>
          </span>
        ) : null}
        <span role="status" aria-live="polite" className={styles.statusMessage}>
          <span data-testid="shell-status" className={styles.statusValue}>
            {resolvedStatus}
          </span>
        </span>
        <span className={styles.statusSpacer} />
        {hints.length > 0 ? (
          <span className={styles.statusHints}>
            {hints.map((hint) => (
              <button
                key={hint.id}
                type="button"
                className={styles.statusHint}
                onClick={hint.run}
              >
                <span>{hint.label}</span>
                {hint.keys.split(" ").map((key) => (
                  <ClayKbd key={`${hint.id}-${key}`}>{key}</ClayKbd>
                ))}
              </button>
            ))}
          </span>
        ) : null}
      </footer>
    </div>
  );
}
