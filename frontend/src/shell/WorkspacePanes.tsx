import { lazy, Suspense, useCallback, useSyncExternalStore } from "react";

import { ClayButton, ClayModal, ClayText } from "../components";
import { AgentView } from "../coding-agent/AgentView";
import { PaneTree, effortChordOf } from "./PaneTree";
import { tabUncommitted } from "./tab-store";
import type { WorkspaceController } from "./workspace-controller";

import styles from "./workspace-panes.module.css";

const PackageWorkspace = lazy(async () => {
  const module = await import("../packages/PackageWorkspace");
  return { default: module.PackageWorkspace };
});

const CommandCentre = lazy(async () => {
  const module = await import("../command-centre/CommandCentre");
  return { default: module.CommandCentre };
});

export function WorkspacePanes({
  workspace,
}: {
  workspace: WorkspaceController;
}) {
  const snapshot = useSyncExternalStore(
    workspace.subscribe,
    workspace.getSnapshot,
  );
  const runtime = workspace.active();
  const pending = workspace.pendingClose();
  const tab =
    snapshot.tabs.find((entry) => entry.clientId === runtime?.clientId) ?? null;
  // Stable identity: the agent panel reports its run state on every render.
  const clientId = runtime?.clientId ?? null;
  const onAgentBusy = useCallback(
    (busy: boolean) => {
      if (clientId != null) workspace.setAgentBusy(clientId, busy);
    },
    [clientId, workspace],
  );
  if (!runtime) {
    return (
      <div className={styles.empty}>
        <ClayText variant="body" muted>
          No tab
        </ClayText>
      </div>
    );
  }
  // Both of the tab's views stay mounted and the inactive one is hidden, so
  // switching never reloads a package, re-fetches the tree, or loses the other
  // view's scroll/selection state (plan 118 task 33, performance AC). The agent
  // half mounts the first time it is shown — its session must not start for a
  // tab the user only edits.
  const activePane = runtime.panes.get(runtime.tree.activePaneId) ?? null;
  const agentMounted = runtime.agentMounted || tab?.view === "agent";
  // Plan 118 task 36: the Files tab's row action — the document opens through
  // the same path every other open uses (server-contained to the workspace
  // root), and the tab switches to the view that shows it. The agent view stays
  // mounted, so its transcript and scroll position survive the handoff.
  const openInWorkspace = useCallback(
    (path: string) => {
      workspace.openPath(path);
      workspace.setView("workspace");
    },
    [workspace],
  );
  const panes = (
    <PaneTree
      runtime={runtime}
      node={runtime.tree.root}
      packageUi={runtime.ui.packageUi}
      uiVersion={runtime.ui.packageUi?.version ?? 0}
      onFocus={(paneId) => workspace.focus(paneId)}
      onRatio={(path, ratio) => workspace.setRatio(path, ratio)}
      onOpenPath={(path) => workspace.openPath(path)}
      onOpenFile={() => workspace.openFileDialog()}
      onOpenFolder={() => workspace.openFolderDialog()}
      onPickAgent={(agent) => workspace.attachAgent(agent)}
      onOpenWorkspace={(root) => void workspace.openWorkspace(root)}
      showLauncher={tab ? tabUncommitted(tab) : false}
    />
  );
  const agentView = agentMounted ? (
    <AgentView
      packageUi={runtime.ui.packageUi}
      uiVersion={runtime.ui.packageUi?.version ?? 0}
      workspaceRoot={tab?.workspaceRoot || runtime.sessionRoot}
      session={activePane?.session ?? null}
      send={activePane?.session.request ?? null}
      effortChord={activePane ? effortChordOf(activePane) : null}
      onBusyChange={onAgentBusy}
      agentType={tab?.agent?.type ?? null}
      onPickAgent={(agent) => void workspace.pickAgent(agent)}
      onOpenInWorkspace={openInWorkspace}
    />
  ) : null;
  const content =
    runtime.ui.sdui || runtime.ui.packageUi ? (
      <Suspense fallback={panes}>
        <PackageWorkspace
          sdui={runtime.ui.sdui}
          packageUi={runtime.ui.packageUi}
          send={(payload) =>
            runtime.panes
              .get(runtime.tree.activePaneId)
              ?.session.request(payload) ?? Promise.resolve()
          }
          editorSlot={panes}
          settingsOpen={runtime.settingsOpen}
        />
      </Suspense>
    ) : (
      panes
    );
  const workspaceActive = tab?.view !== "agent";
  return (
    <div className={styles.host} data-testid="workspace-panes">
      <div
        className={styles.viewSlot}
        data-view="workspace"
        data-active={workspaceActive ? "true" : "false"}
        hidden={!workspaceActive}
      >
        {content}
      </div>
      {agentView ? (
        <div
          className={styles.viewSlot}
          data-view="agent"
          data-active={workspaceActive ? "false" : "true"}
          hidden={workspaceActive}
        >
          {agentView}
        </div>
      ) : null}
      {runtime.menu && (
        <Suspense fallback={null}>
          <CommandCentre workspace={workspace} />
        </Suspense>
      )}
      <ClayModal
        title="Unsaved changes"
        open={pending != null}
        onClose={() => workspace.cancelClose()}
        // The foot is the modal's own region (hairline top, wrapping), cancel
        // leading and the primary action last.
        footer={
          <>
            <ClayButton variant="muted" onPress={() => workspace.cancelClose()}>
              Cancel
            </ClayButton>
            <ClayButton
              variant="danger"
              onPress={() => {
                if (pending)
                  void workspace.confirmClose(pending.clientId, false);
              }}
            >
              Discard and close
            </ClayButton>
            <ClayButton
              variant="primary"
              onPress={() => {
                if (pending)
                  void workspace.confirmClose(pending.clientId, true);
              }}
            >
              Save all and close
            </ClayButton>
          </>
        }
      >
        <ClayText variant="body">
          Save or discard {pending?.dirtyPaths.join(", ")} before closing.
        </ClayText>
      </ClayModal>
    </div>
  );
}
