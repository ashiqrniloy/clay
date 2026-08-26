import { lazy, Suspense, useSyncExternalStore } from "react";

import { ClayButton, ClayModal, ClayText } from "../components";
import { PaneTree } from "./PaneTree";
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
  useSyncExternalStore(workspace.subscribe, workspace.getSnapshot);
  const runtime = workspace.active();
  const pending = workspace.pendingClose();
  if (!runtime) {
    return (
      <div className={styles.empty}>
        <ClayText variant="body" muted>
          No tab
        </ClayText>
      </div>
    );
  }
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
    />
  );
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
  return (
    <div className={styles.host} data-testid="workspace-panes">
      {content}
      {runtime.menu && (
        <Suspense fallback={null}>
          <CommandCentre workspace={workspace} />
        </Suspense>
      )}
      <ClayModal
        title="Unsaved changes"
        open={pending != null}
        onClose={() => workspace.cancelClose()}
      >
        <ClayText variant="body">
          Save or discard {pending?.dirtyPaths.join(", ")} before closing.
        </ClayText>
        <div className={styles.actions}>
          <ClayButton
            variant="primary"
            onPress={() => {
              if (pending) void workspace.confirmClose(pending.clientId, true);
            }}
          >
            Save all and close
          </ClayButton>
          <ClayButton
            variant="danger"
            onPress={() => {
              if (pending) void workspace.confirmClose(pending.clientId, false);
            }}
          >
            Discard and close
          </ClayButton>
          <ClayButton variant="muted" onPress={() => workspace.cancelClose()}>
            Cancel
          </ClayButton>
        </div>
      </ClayModal>
    </div>
  );
}
