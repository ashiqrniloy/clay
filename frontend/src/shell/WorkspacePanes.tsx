import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { ClayButton, ClayModal, ClayText } from "../components";
import { recipeAttributes } from "../components/recipe-attributes";
import { AgentLane } from "./AgentLane";
import { AgentView } from "../coding-agent/AgentView";
import { createAgentSession, type AgentSessionModule } from "../agent/state";
import type { ComposerPalette } from "../coding-agent/Composer";
import {
  SduiRegion,
  WORKSPACE_SIDE_SIZE,
  hostRailRegionId,
} from "../sdui/renderer";
import type { IntentSender } from "../sdui/actions";
import { PaneTree, effortChordOf } from "./PaneTree";
import { tabUncommitted, type ShellTabState } from "./tab-store";
import { detached } from "../lib/detached";
import { agentLane } from "./layout-state";
import type { TabRuntime, WorkspaceController } from "./workspace-controller";

import styles from "./workspace-panes.module.css";

const PackageWorkspace = lazy(async () => {
  const module = await import("../packages/PackageWorkspace");
  return { default: module.PackageWorkspace };
});

/**
 * The tab's agent session store, resolved by the host (plan 124): the lane is
 * mounted for every tab, so the store can no longer be born with the agent
 * view. One store per tab runtime — the tab adopts it (lifetime owner) and
 * disposes it on close; both the lane and the agent view read the same one.
 */
function useTabAgentStore(
  runtime: TabRuntime,
  workspace: WorkspaceController,
): AgentSessionModule {
  // Keyed by the runtime object: switching tabs reads that tab's own store,
  // never the previous tab's (plan 119 SC-6 — no process-global session).
  const created = useRef<{
    runtime: TabRuntime;
    store: AgentSessionModule;
  } | null>(null);
  if (created.current?.runtime !== runtime) {
    created.current = {
      runtime,
      store:
        runtime.agent ??
        createAgentSession({ send: runtime.send, clientId: runtime.clientId }),
    };
  }
  const store = created.current.store;
  useEffect(() => {
    if (!runtime.agent) workspace.attachAgentStore(runtime.clientId, store);
  }, [runtime, store, workspace]);
  // The tab's store bootstrap: what the tab knows before anyone prompts — the
  // session inventory and the tab's session binding + STATE (plan 117
  // follow-up). It belongs to the store, not to whichever surface is mounted.
  useEffect(() => {
    store.command("listSessions");
    store.requestBinding();
  }, [store]);
  const uiVersion = runtime.ui.packageUi?.version ?? 0;
  useEffect(() => {
    store.agent.setUiVersion(uiVersion);
  }, [store, uiVersion]);
  return store;
}

/** One tab's views plus its lane: hooks that need a runtime live below the
 *  "no tab" gate, never conditionally. */
function TabPanes({
  runtime,
  tab,
  workspace,
}: {
  runtime: TabRuntime;
  tab: ShellTabState | null;
  workspace: WorkspaceController;
}) {
  const clientId = runtime.clientId;
  const onAgentBusy = useCallback(
    (busy: boolean) => workspace.setAgentBusy(clientId, busy),
    [clientId, workspace],
  );
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
  // The host owns the tab's store; the agent view no longer creates it (it
  // must not depend on that view having been shown — plan 124).
  const adoptAgentStore = useCallback(
    (store: AgentSessionModule) => workspace.attachAgentStore(clientId, store),
    [clientId, workspace],
  );
  const agentStore = useTabAgentStore(runtime, workspace);
  const uiVersion = runtime.ui.packageUi?.version ?? 0;
  const activePane = runtime.panes.get(runtime.tree.activePaneId) ?? null;
  // Plan 124: the composer's `/` palette is the tab's menu session, so the two
  // surfaces that carry it — the sheet in the lane and the veil over the
  // working area — read one owner. The veil also opens for the field's `@`
  // mentions, which is the composer's own state (it reports it up).
  const paletteMenu =
    runtime.menu?.origin === "commandPalette" ? runtime.menu : null;
  const laneVisible = useSyncExternalStore(
    agentLane.subscribe,
    agentLane.isVisible,
  );
  const [mentionsOpen, setMentionsOpen] = useState(false);
  const palette = useMemo<ComposerPalette>(
    () => ({
      menu: paletteMenu,
      request: () => workspace.dispatchServerCommand("controlCenter.open"),
      query: (filter, scope) => workspace.menuQuery(filter, scope),
      move: (delta) => workspace.menuMove(delta),
      activate: (secondary) => workspace.menuActivate(secondary),
      // One stage back, and the session is what answers whether there is one
      // behind it (a stage's first step dismisses the sheet).
      back: () => workspace.menuBackspace(),
      cancel: () => workspace.menuCancel(),
    }),
    [paletteMenu, workspace],
  );
  // Both of the tab's views stay mounted and the inactive one is hidden, so
  // switching never reloads a package, re-fetches the tree, or loses the other
  // view's scroll/selection state (plan 118 task 33, performance AC). The agent
  // half mounts the first time it is shown — its session must not start for a
  // tab the user only edits.
  const agentMounted = runtime.agentMounted || tab?.view === "agent";
  // The workspace sidebar is the tree's token-sized region, and the shell — not
  // the tree — owns the box it lands in (DESIGN.md §5/§12): it renders that
  // region as the working area's own left rail, so it keeps the full height
  // beside the lane, and the pane's tree is rendered without it.
  const sdui = runtime.ui.sdui;
  const sideRegionId = sdui
    ? hostRailRegionId(sdui, WORKSPACE_SIDE_SIZE)
    : null;
  const sendToPane = useCallback<IntentSender>(
    (payload) =>
      runtime.panes.get(runtime.tree.activePaneId)?.session.request(payload) ??
      Promise.resolve(),
    [runtime],
  );
  const panes = (
    <PaneTree
      runtime={runtime}
      node={runtime.tree.root}
      packageUi={runtime.ui.packageUi}
      uiVersion={uiVersion}
      onFocus={(paneId) => workspace.focus(paneId)}
      onRatio={(path, ratio) => workspace.setRatio(path, ratio)}
      onOpenPath={(path) => workspace.openPath(path)}
      onOpenFile={() => workspace.openFileDialog()}
      onOpenFolder={() => workspace.openFolderDialog()}
      onPickAgent={(agent) => workspace.attachAgent(agent)}
      onOpenWorkspace={(root) => detached(workspace.openWorkspace(root))}
      showLauncher={tab ? tabUncommitted(tab) : false}
    />
  );
  const agentView = agentMounted ? (
    <AgentView
      packageUi={runtime.ui.packageUi}
      uiVersion={uiVersion}
      session={activePane?.session ?? null}
      agent={runtime.agent}
      agentClientId={clientId}
      onAgentStore={adoptAgentStore}
      send={runtime.send}
      onOpenInWorkspace={openInWorkspace}
    />
  ) : null;
  const content =
    runtime.ui.sdui || runtime.ui.packageUi ? (
      <Suspense fallback={panes}>
        <PackageWorkspace
          sdui={runtime.ui.sdui}
          packageUi={runtime.ui.packageUi}
          send={sendToPane}
          editorSlot={panes}
          settingsOpen={runtime.settingsOpen}
          omittedRegions={sideRegionId != null ? [sideRegionId] : undefined}
        />
      </Suspense>
    ) : (
      panes
    );
  const workspaceActive = tab?.view !== "agent";
  // A hidden lane takes the field's menus with it (the sheet lives inside it),
  // so the veil must not outlive the query input it would otherwise cover for:
  // a lane-hidden palette session would strand a scrim with no way to dismiss
  // it from the working area (plan 124 launch test).
  const fieldMenuOpen = laneVisible && (paletteMenu !== null || mentionsOpen);
  return (
    <>
      {/* The workspace sidebar (the tree's `dimension.sidebar.default` region)
          is the working area's left rail: column 1, spanning the working area's
          rows, so it — like the inspector rail — keeps the full height and the
          lane's hairline stops at its edge (DESIGN.md §12). It belongs to the
          workspace view only: the agent view has no files rail, and the lane
          then spans the pane's whole width. */}
      <div
        className={styles.side}
        data-panes="side"
        hidden={!workspaceActive || sdui == null || sideRegionId == null}
      >
        {sdui != null && sideRegionId != null ? (
          <SduiRegion state={sdui} send={sendToPane} regionId={sideRegionId} />
        ) : null}
      </div>
      <div className={styles.viewArea} data-panes="view-area">
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
      </div>
      {/* The field's menus veil the working area they cover and leave the lane
          above them (DESIGN.md §6: the scrim's second caller). It is the tab's
          own layer, not part of the view area: the working-area grid places it
          over the panes *and* the rail, and the lane's higher z-index keeps its
          field — the palette's query — undimmed and unblocked. */}
      <div
        className={styles.veil}
        data-open={fieldMenuOpen ? "true" : "false"}
        data-panes="veil"
        aria-hidden="true"
        {...recipeAttributes("modal", "scrim")}
      />
      {/* The tab's one lane, mounted for both views (plan 124): the composer,
          the agent controls, and the session foot never remount on a view
          switch, and the tab's visibility state hides it (`Ctrl+X Ctrl+P`). */}
      <AgentLane
        store={agentStore}
        uiVersion={uiVersion}
        workspaceRoot={tab?.workspaceRoot || runtime.sessionRoot}
        session={activePane?.session ?? null}
        effortChord={activePane ? effortChordOf(activePane) : null}
        agentType={tab?.agent?.type ?? null}
        onPickAgent={(agent) => workspace.pickAgent(agent)}
        onBusyChange={onAgentBusy}
        palette={palette}
        onFieldMenuOpen={setMentionsOpen}
      />
    </>
  );
}

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
  if (!runtime) {
    return (
      <div className={styles.empty}>
        <ClayText variant="body" muted>
          No tab
        </ClayText>
      </div>
    );
  }
  const tab =
    snapshot.tabs.find((entry) => entry.clientId === runtime.clientId) ?? null;
  // Plan 125: this host draws exactly one transient surface — the lane's
  // composer palette. The retired `centered` origin has no projection here any
  // more, and the package origins (`contextMenu`/`menuBar`) are the package UI
  // renderer's own overlays, not the shell's.
  return (
    <div className={styles.host} data-testid="workspace-panes">
      <TabPanes runtime={runtime} tab={tab} workspace={workspace} />
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
                  detached(workspace.confirmClose(pending.clientId, false));
              }}
            >
              Discard and close
            </ClayButton>
            <ClayButton
              variant="primary"
              onPress={() => {
                if (pending)
                  detached(workspace.confirmClose(pending.clientId, true));
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
