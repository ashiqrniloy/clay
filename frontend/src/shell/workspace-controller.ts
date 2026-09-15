// Per-window workspace: N independent tab clients, each with its own
// split tree and pane→document sessions. Split topology is client-local;
// tab identity is server-authoritative.

import type {
  BootstrapDto,
  BridgeEnvelope,
  RuntimeDiagnosticDto,
  TransientMenuSnapshotDto,
} from "../bridge/types";
import {
  persistenceKeyProjection,
  shellStatusProjection,
} from "../state/document-store";
import { emptyUiProjection, type UiProjectionState } from "../sdui/state";
import {
  handleEnvelope as applyEnvelope,
  type EnvelopeContext,
} from "./workspace-envelope";
import {
  dispatchClientCommand,
  type CommandContext,
} from "./workspace-commands";
import {
  createDocumentSession,
  type DocumentSession,
} from "../editor/sync/session";
import {
  addEqualPane,
  closePane,
  focusPane,
  movePane,
  nextPane,
  paneIds,
  prevPane,
  resizeActive,
  singlePane,
  splitPane,
  updateRatioAt,
  type SplitChild,
  type SplitOrientation,
  type SplitTree,
} from "./split-tree";
import {
  emptyLayout,
  tabsFromWindow,
  windowFromTabs,
  type TabLayout,
} from "./persist";
import { agentInspector, workspaceRail } from "./layout-state";
import { detached } from "../lib/detached";
// Type-only on purpose: the store is created by the lazy agent view, so the
// eager shell must never take a runtime dependency on the agent lane (it drags
// the AG-UI stack into the startup preload).
import type { AgentSessionModule } from "../agent/state";
import {
  createTabStore,
  emptyTabs,
  markDirty,
  patchTab,
  removeTab,
  tabLabel,
  tabUncommitted,
  upsertTab,
  type ShellTabState,
  type TabAgent,
  type TabSnapshot,
  type TabStore,
  type TabView,
} from "./tab-store";

export interface WorkspaceAdapters {
  send: (payload: string, tabId?: number) => Promise<void>;
  openTab?: (workspaceRoot: string) => Promise<BootstrapDto>;
  closeTab?: (tabId: number) => Promise<void>;
  activateTab?: (tabId: number) => Promise<void>;
  loadLayout?: () => Promise<unknown>;
  saveLayout?: (state: unknown) => Promise<void>;
  openFileDialog?: (tabId?: number) => Promise<boolean>;
  openFolderDialog?: (tabId?: number) => Promise<boolean>;
  /** Agent settings page (plan 117): listen for the listing reply on the
   *  given session, or tear down with `null`. */
}

export interface PaneRecord {
  paneId: number;
  session: DocumentSession;
}

/** Serialized server `KeyStroke` (protocol `KeyCode` + modifiers). */
export interface ServerKeyStroke {
  key: string | { character: string };
  modifiers: {
    shift: boolean;
    control: boolean;
    alt: boolean;
    superKey: boolean;
  };
}

export interface TabRuntime {
  clientId: number;
  tabId: number | null;
  /** The server's root for this tab. Always a real folder: the server falls
   *  back to the configured root or cwd. The *picked* folder (what the strip
   *  shows and layout.json persists) lives on the tab record, because a tab
   *  may have picked nothing yet while its session is still rooted. */
  sessionRoot: string;
  /** Server workspace root id from the tab registry; null until known. */
  workspaceRootId: number | null;
  tree: SplitTree;
  panes: Map<number, PaneRecord>;
  ui: UiProjectionState;
  menu: TransientMenuSnapshotDto | null;
  diagnostic: RuntimeDiagnosticDto | null;
  settingsOpen: boolean;
  /** The agent view has been shown at least once. The panel stays mounted
   *  afterwards so switching views never restarts the session or loses the
   *  transcript's scroll position (plan 118 task 33, performance AC). */
  agentMounted: boolean;
  /** This tab's agent session store (plan 119 SC-6), once its agent view has
   *  mounted one. The tab — not the process — owns the session, its transcript,
   *  and its relay subscription. */
  agent: AgentSessionModule | null;
  /** Tab-stamped sender: every agent intent rides this tab's own connection. */
  send: (payload: string) => Promise<void>;
}

/** Identity a newly mounted tab starts with; absent fields default to
 *  "uncommitted, workspace view, no agent". */
export interface TabIdentity {
  workspaceRoot?: string;
  agent?: TabAgent | null;
  view?: TabView;
}

export interface PendingClose {
  clientId: number;
  dirtyPaths: string[];
}

export function createWorkspace(adapters: WorkspaceAdapters) {
  const tabs: TabStore = createTabStore();
  const runtimes = new Map<number, TabRuntime>();
  let pendingClose: PendingClose | null = null;
  /** Feature-event subscription while the agent settings page listens for
   *  its listing reply (plan 117); torn down on close or workspace drop. */
  // The server broadcasts the tab registry during handshake, before the
  // bootstrap command creates the runtime, so a fresh-boot restore would
  // otherwise queue document opens forever waiting for a root id that
  // already arrived. Remember the latest roots per client.
  const registryRootsByClient = new Map<number, number>();
  /** Same race for tab ids: the registry carries each client's server tab id
   *  but usually lands before its runtime mounts. Without adopting it here,
   *  runtime.tabId stays null and every pane request falls back to the
   *  bridge's active-client (the FIRST connection) — the Control Centre
   *  chord opened on the wrong, hidden tab with two tabs up. */
  const registryTabsByClient = new Map<number, number>();
  const persistListeners = new Set<() => void>();
  let persistTimer: ReturnType<typeof setTimeout> | null = null;

  const notify = () => {
    // useSyncExternalStore compares snapshots by identity; transient pane
    // status changes need a fresh shell snapshot without changing tab data.
    tabs.set({ ...tabs.get() });
    for (const listener of [...persistListeners]) listener();
  };

  const schedulePersist = () => {
    notify();
    if (!adapters.saveLayout) return;
    if (persistTimer) clearTimeout(persistTimer);
    persistTimer = setTimeout(() => {
      persistTimer = null;
      detached(adapters.saveLayout?.(serialize()));
    }, 250);
  };

  // Rail/inspector visibility is per-tab layout state: a toggle persists with
  // the rest of the layout, not only when something else schedules a write
  // (plan 118 task E2).
  workspaceRail.subscribe(schedulePersist);
  agentInspector.subscribe(schedulePersist);

  const bindSession = (runtime: TabRuntime): DocumentSession => {
    const session = createDocumentSession({
      send: runtime.send,
    });
    let persistKey: string | null = null;
    let statusKey = "";
    session.store.subscribe(() => {
      const meta = session.store.get();
      // Loading/diagnostic changes are shell-visible, but version/pending
      // churn stays pane-local and does not rerender the shell.
      const nextStatusKey = shellStatusProjection(meta);
      if (nextStatusKey !== statusKey) {
        statusKey = nextStatusKey;
        notify();
      }
      // Only document identity/path/dirty transitions schedule persistence;
      // per-keystroke acks stay pane-local.
      const key = persistenceKeyProjection(meta);
      if (key === persistKey) return;
      persistKey = key;
      const dirty = [...runtime.panes.values()].some(
        (pane) => pane.session.store.get()?.dirty,
      );
      tabs.set(markDirty(tabs.get(), runtime.clientId, dirty));
      schedulePersist();
    });
    return session;
  };

  /** Delivers the server root id to pane sessions still missing it; any
   * restore-queued document open flushes once the root id is known. */
  const deliverRootId = (runtime: TabRuntime, rootId: number) => {
    for (const pane of runtime.panes.values()) {
      const current = pane.session.store.get();
      if (current && current.workspaceRootId != null) continue;
      pane.session.handleEnvelope({
        kind: "event",
        data: {
          kind: "documentStatus",
          data: {
            documentId: current?.documentId ?? 0,
            workspaceRootId: rootId,
          },
        },
      });
    }
  };

  const ensurePane = (runtime: TabRuntime, paneId: number): PaneRecord => {
    const existing = runtime.panes.get(paneId);
    if (existing) return existing;
    const record = { paneId, session: bindSession(runtime) };
    runtime.panes.set(paneId, record);
    return record;
  };

  const mountRuntime = (
    bootstrap: BootstrapDto,
    tree = singlePane(),
    identity: TabIdentity = {},
  ): TabRuntime => {
    const runtime: TabRuntime = {
      clientId: bootstrap.clientId,
      tabId:
        bootstrap.tabId ?? registryTabsByClient.get(bootstrap.clientId) ?? null,
      agent: null,
      // Tab-stamped and lazy: the registry usually delivers the tab id after
      // the runtime mounts, so the sender reads it per call (plan 119 SC-6:
      // no mutable process-wide sender).
      send: (payload) => adapters.send(payload, runtime.tabId ?? undefined),
      sessionRoot: bootstrap.initialDocument.workspaceRoot,
      workspaceRootId: null,
      tree,
      panes: new Map(),
      ui: emptyUiProjection(),
      menu: null,
      diagnostic: null,
      settingsOpen: false,
      agentMounted: identity.view === "agent",
    };
    const first = ensurePane(runtime, tree.activePaneId);
    first.session.installInitial(bootstrap);
    runtimes.set(bootstrap.clientId, runtime);
    // Tab visibility follows the tab that is up; the first mounted tab is it
    // until `activate` says otherwise.
    if (tabs.get().activeClientId == null) {
      workspaceRail.setActiveTab(bootstrap.clientId);
      agentInspector.setActiveTab(bootstrap.clientId);
    }
    // Only an explicit pick commits a tab's folder. The server always roots a
    // session (configured root or cwd fallback), so adopting its root here
    // would mark a fresh window — and every ⌘T tab — as already committed and
    // hide the launcher that is their landing (plan 118 Part D).
    const picked = identity.workspaceRoot ?? "";
    tabs.set(
      upsertTab(tabs.get(), {
        tabId: runtime.tabId,
        clientId: runtime.clientId,
        workspaceRoot: picked,
        agent: identity.agent ?? null,
        view: identity.view ?? "workspace",
        agentBusy: false,
        label: tabLabel(picked, identity.agent ?? null),
        dirty: false,
        disconnected: false,
      }),
    );
    return runtime;
  };

  /** The tab record for a runtime (strip identity; the runtime carries the
   *  session's own server-side facts). */
  const tabFor = (clientId: number): ShellTabState | null =>
    tabs.get().tabs.find((tab) => tab.clientId === clientId) ?? null;

  /** Tab close / reconnect: the store's relay subscription goes with the tab.
   *  The store itself is created by the tab's agent view (bundle boundary) and
   *  adopted here, so a run that outlives a view switch keeps streaming. */
  const disposeAgent = (runtime: TabRuntime) => {
    runtime.agent?.dispose();
    runtime.agent = null;
  };

  const patchActive = (
    patch: Partial<Omit<ShellTabState, "clientId" | "label">>,
    clientId?: number,
  ) => {
    const runtime = clientId == null ? activeRuntime() : runtimes.get(clientId);
    if (!runtime) return;
    tabs.set(patchTab(tabs.get(), runtime.clientId, patch));
    schedulePersist();
  };

  const activeRuntime = (): TabRuntime | null => {
    const id = tabs.get().activeClientId;
    return id == null ? null : (runtimes.get(id) ?? null);
  };

  const findPaneWithPath = (
    runtime: TabRuntime,
    path: string,
  ): PaneRecord | null => {
    for (const pane of runtime.panes.values()) {
      if (pane.session.store.get()?.path === path) return pane;
    }
    return null;
  };

  const setTree = (runtime: TabRuntime, tree: SplitTree | null) => {
    if (!tree) return;
    runtime.tree = tree;
    for (const id of paneIds(tree.root)) ensurePane(runtime, id);
    for (const id of [...runtime.panes.keys()]) {
      if (!paneIds(tree.root).includes(id)) runtime.panes.delete(id);
    }
    schedulePersist();
    notify();
  };

  const commandAdapters = {
    ...adapters,
  };
  const commandContext: CommandContext = {
    adapters: commandAdapters,
    tabs,
    notify,
    setTree,
    mountRuntime,
  };
  const envelopeContext: EnvelopeContext = {
    adapters,
    runtimes,
    tabs,
    registryRootsByClient,
    registryTabsByClient,
    notify,
    deliverRootId,
    ensurePane,
    dispatchClientCommand: (runtime, commandId) =>
      dispatchClientCommand(commandContext, runtime, commandId),
  };

  function serialize() {
    const snapshot = tabs.get();
    const rails = workspaceRail.snapshot();
    const inspectors = agentInspector.snapshot();
    const layouts: TabLayout[] = snapshot.tabs.map((tab) => {
      const runtime = runtimes.get(tab.clientId);
      return {
        workspaceRoot: tab.workspaceRoot,
        agent: tab.agent,
        view: tab.view,
        // Absent means visible, which is also what the store answers.
        railVisible: rails.get(tab.clientId) ?? true,
        inspectorVisible: inspectors.get(tab.clientId) ?? true,
        tree: runtime?.tree ?? singlePane(),
        documents: runtimeDocuments(runtime),
      };
    });
    const activeIndex = snapshot.tabs.findIndex(
      (tab) => tab.clientId === snapshot.activeClientId,
    );
    return windowFromTabs(layouts, activeIndex >= 0 ? activeIndex : null);
  }

  const workspace = {
    tabs,
    subscribe: (listener: () => void) => {
      persistListeners.add(listener);
      return () => persistListeners.delete(listener);
    },
    getSnapshot: (): TabSnapshot => tabs.get(),
    runtime: (clientId: number) => runtimes.get(clientId) ?? null,
    /** Adopts the tab's agent store (plan 119 SC-6). Called by the agent view
     *  when it mounts: the tab runtime owns the store from then on, and the
     *  tab's close disposes it. Idempotent for a repeated mount. */
    attachAgentStore(clientId: number, store: AgentSessionModule) {
      const runtime = runtimes.get(clientId);
      if (!runtime || runtime.agent === store) return;
      disposeAgent(runtime);
      runtime.agent = store;
    },
    active: activeRuntime,
    pendingClose: () => pendingClose,
    reset() {
      for (const runtime of runtimes.values()) disposeAgent(runtime);
      runtimes.clear();
      tabs.set(emptyTabs());
      pendingClose = null;
      registryRootsByClient.clear();
      notify();
    },
    installBootstrap(bootstrap: BootstrapDto) {
      mountRuntime(bootstrap);
      notify();
    },
    async restore() {
      if (!adapters.loadLayout) return;
      const parsed = tabsFromWindow(await adapters.loadLayout());
      if (!parsed) return;
      const existing = [...runtimes.values()];
      const first = existing[0];
      const restoredVisibility = new Map<
        number,
        { rail: boolean; inspector: boolean }
      >();
      if (first && parsed.tabs[0]) {
        const restored = parsed.tabs[0];
        first.tree = restored.tree;
        first.agentMounted = restored.view === "agent";
        tabs.set(
          patchTab(tabs.get(), first.clientId, {
            workspaceRoot: restored.workspaceRoot,
            agent: restored.agent,
            view: restored.view,
          }),
        );
        for (const id of paneIds(first.tree.root)) ensurePane(first, id);
        for (const [paneId, path] of parsed.tabs[0].documents) {
          if (path) first.panes.get(paneId)?.session.open(path);
        }
        // Per-tab visibility (plan 118 task E2): the first tab's own values.
        restoredVisibility.set(first.clientId, {
          rail: restored.railVisible,
          inspector: restored.inspectorVisible,
        });
        // The registry may have delivered the root id before these panes
        // existed (fresh boot: the handshake broadcast races the bootstrap
        // command); deliver the remembered root so queued opens flush.
        if (first.workspaceRootId == null) {
          const remembered = registryRootsByClient.get(first.clientId);
          if (remembered != null) {
            first.workspaceRootId = remembered;
            deliverRootId(first, remembered);
          }
        }
        if (first.workspaceRootId != null)
          deliverRootId(first, first.workspaceRootId);
      }
      for (const extra of parsed.tabs.slice(1)) {
        if (!adapters.openTab) break;
        try {
          const bootstrap = await adapters.openTab(extra.workspaceRoot);
          const runtime = mountRuntime(bootstrap, extra.tree, {
            workspaceRoot: extra.workspaceRoot,
            agent: extra.agent,
            view: extra.view,
          });
          restoredVisibility.set(runtime.clientId, {
            rail: extra.railVisible,
            inspector: extra.inspectorVisible,
          });
          for (const id of paneIds(extra.tree.root)) ensurePane(runtime, id);
          for (const [paneId, path] of extra.documents) {
            if (path) runtime.panes.get(paneId)?.session.open(path);
          }
          if (runtime.workspaceRootId == null) {
            const remembered = registryRootsByClient.get(runtime.clientId);
            if (remembered != null) {
              runtime.workspaceRootId = remembered;
              deliverRootId(runtime, remembered);
            }
          }
          if (runtime.workspaceRootId != null) {
            deliverRootId(runtime, runtime.workspaceRootId);
          }
        } catch {
          // Hostile/unreachable persisted tab: skip; first tab stays up.
        }
      }
      // Hand the restored values to the visibility stores once every tab has
      // its clientId, then let `activate` put the right tab up.
      workspaceRail.restore(
        [...restoredVisibility].map(([clientId, value]) => [
          clientId,
          value.rail,
        ]),
      );
      agentInspector.restore(
        [...restoredVisibility].map(([clientId, value]) => [
          clientId,
          value.inspector,
        ]),
      );
      if (parsed.activeIndex != null) {
        const target = tabs.get().tabs[parsed.activeIndex];
        if (target) await this.activate(target.clientId);
      }
      notify();
    },
    handleEnvelope(envelope: BridgeEnvelope) {
      applyEnvelope(envelopeContext, envelope);
    },
    activate(clientId: number) {
      const runtime = runtimes.get(clientId);
      if (!runtime) return Promise.resolve();
      tabs.set({ ...tabs.get(), activeClientId: clientId });
      workspaceRail.setActiveTab(clientId);
      agentInspector.setActiveTab(clientId);
      notify();
      if (runtime.tabId != null && adapters.activateTab) {
        return adapters.activateTab(runtime.tabId);
      }
      return Promise.resolve();
    },
    async openTab(workspaceRoot: string, identity: TabIdentity = {}) {
      if (!adapters.openTab) return;
      const bootstrap = await adapters.openTab(workspaceRoot);
      mountRuntime(bootstrap, singlePane(), {
        workspaceRoot,
        ...identity,
      });
      tabs.set({ ...tabs.get(), activeClientId: bootstrap.clientId });
      workspaceRail.setActiveTab(bootstrap.clientId);
      agentInspector.setActiveTab(bootstrap.clientId);
      schedulePersist();
    },
    setSettingsOpen(open: boolean) {
      const runtime = activeRuntime();
      if (!runtime) return;
      runtime.settingsOpen = open;
      notify();
    },
    menuQuery(query: string) {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      detached(
        adapters.send(
          JSON.stringify({
            family: "menuQueryUpdate",
            payload: {
              clientId: runtime.clientId,
              sessionId: menu.sessionId,
              query,
            },
          }),
          runtime.tabId ?? undefined,
        ),
      );
    },
    menuBackspace() {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      detached(
        adapters.send(
          JSON.stringify({
            family: "menuBackspace",
            payload: { clientId: runtime.clientId, sessionId: menu.sessionId },
          }),
          runtime.tabId ?? undefined,
        ),
      );
    },
    menuMove(delta: number) {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      detached(
        adapters.send(
          JSON.stringify({
            family: "menuSelectionMove",
            payload: {
              clientId: runtime.clientId,
              sessionId: menu.sessionId,
              delta,
            },
          }),
          runtime.tabId ?? undefined,
        ),
      );
    },
    menuActivate(secondary = false) {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      detached(
        adapters.send(
          JSON.stringify({
            family: "menuActivate",
            payload: {
              clientId: runtime.clientId,
              sessionId: menu.sessionId,
              kind: secondary ? "secondary" : "primary",
            },
          }),
          runtime.tabId ?? undefined,
        ),
      );
    },
    menuCancel() {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      // Close locally first — the UI must never wait on the server round
      // trip (or its loss) to dismiss a modal the user asked to close.
      runtime.menu = null;
      notify();
      detached(
        adapters.send(
          JSON.stringify({
            family: "menuCancel",
            payload: { clientId: runtime.clientId, sessionId: menu.sessionId },
          }),
          runtime.tabId ?? undefined,
        ),
      );
    },
    /** Show one of the active tab's two views (tab chrome, ⌘1/⌘2). The other
     *  view stays mounted, so switching never re-fetches or restarts it. */
    setView(view: TabView) {
      const runtime = activeRuntime();
      if (!runtime) return;
      if (view === "agent") runtime.agentMounted = true;
      patchActive({ view });
    },
    /** Attach an agent to the active tab in place (plan 118 task 34's picker
     *  handoff; task 35 adds the in-tab registry). The workspace half and its
     *  documents are untouched. */
    attachAgent(agent: TabAgent) {
      const runtime = activeRuntime();
      if (!runtime) return;
      runtime.agentMounted = true;
      patchActive({ agent, view: "agent" });
      this.setAgentType(runtime, agent.type);
    },
    /**
     * Switch (or detach) the tab's agent type through tab chrome (plan 118
     * task 35): the server validates the name against the configured
     * `agents/` root, stores it on the registry, and re-reads that agent's
     * config for the tab's live session. The workspace half is untouched, so
     * the patch here is optimistic bookkeeping — the registry snapshot
     * reconciles it if the server refuses.
     */
    setAgentType(runtime: TabRuntime, agent: string | null) {
      if (runtime.tabId == null) return;
      patchActive(
        agent === null
          ? { agent: null }
          : { agent: { type: agent, configRoot: "" } },
        runtime.clientId,
      );
      detached(
        adapters.send(
          JSON.stringify({
            family: "tabCommand",
            payload: {
              clientId: runtime.clientId,
              command: { setAgent: { tabId: runtime.tabId, agent } },
            },
          }),
          runtime.tabId,
        ),
      );
    },
    /** The agent view's picker: the active tab's agent changes in place. */
    pickAgent(agent: string | null) {
      const runtime = activeRuntime();
      if (!runtime) return;
      this.setAgentType(runtime, agent);
    },
    /** The strip's marker pulses while this tab's agent is working. Patch only
     *  on a real change: the view re-renders often and an unconditional write
     *  would feed itself. */
    setAgentBusy(clientId: number, busy: boolean) {
      const tab = tabFor(clientId);
      if (!tab || tab.agentBusy === busy) return;
      patchActive({ agentBusy: busy }, clientId);
    },
    /** ⌘T / the strip's `+`: a new tab on the launcher. The tab is
     *  uncommitted — nothing picked — so its landing is the launcher; the
     *  server roots the session at its own fallback until a folder is picked. */
    async newTab() {
      await this.openTab("", { workspaceRoot: "" });
    },
    /** A launcher row pick. An uncommitted tab is filled in place by rebinding
     *  its workspace (the server rebroadcasts the registry with the new path,
     *  so the label and layout.json follow); a tab that already holds a
     *  workspace opens the picked one as its own tab. */
    async openWorkspace(root: string) {
      const runtime = activeRuntime();
      if (!runtime) return;
      const tab = tabFor(runtime.clientId);
      if (tab && tabUncommitted(tab) && runtime.tabId != null) {
        patchActive({ workspaceRoot: root, view: "workspace" });
        detached(
          adapters.send(
            JSON.stringify({
              family: "tabCommand",
              payload: {
                clientId: runtime.clientId,
                command: { openWorkspace: { tabId: runtime.tabId, root } },
              },
            }),
            runtime.tabId,
          ),
        );
        return;
      }
      await this.openTab(root);
    },
    requestClose(clientId: number) {
      if (tabs.get().tabs.length <= 1) return;
      const runtime = runtimes.get(clientId);
      if (!runtime) return;
      const dirtyPaths = [...runtime.panes.values()]
        .map((pane) => pane.session.store.get())
        .filter((meta) => meta?.dirty)
        .map((meta) => meta?.path || "untitled");
      if (dirtyPaths.length > 0) {
        pendingClose = { clientId, dirtyPaths };
        notify();
        return;
      }
      return this.confirmClose(clientId, false);
    },
    cancelClose() {
      pendingClose = null;
      notify();
    },
    async confirmClose(clientId: number, save: boolean) {
      const runtime = runtimes.get(clientId);
      pendingClose = null;
      if (!runtime) return;
      for (const pane of runtime.panes.values()) {
        if (save) pane.session.save();
        pane.session.close(true);
      }
      if (runtime.tabId != null) await adapters.closeTab?.(runtime.tabId);
      disposeAgent(runtime);
      runtimes.delete(clientId);
      tabs.set(removeTab(tabs.get(), clientId));
      schedulePersist();
    },
    split(orientation: SplitOrientation) {
      const runtime = activeRuntime();
      if (!runtime) return;
      setTree(runtime, splitPane(runtime.tree, orientation));
    },
    addEqual() {
      const runtime = activeRuntime();
      if (!runtime) return;
      setTree(runtime, addEqualPane(runtime.tree));
    },
    closeActivePane() {
      const runtime = activeRuntime();
      if (!runtime) return;
      const pane = runtime.panes.get(runtime.tree.activePaneId);
      if (pane?.session.store.get()?.dirty) {
        pendingClose = {
          clientId: runtime.clientId,
          dirtyPaths: [pane.session.store.get()?.path || "untitled"],
        };
        notify();
        return;
      }
      pane?.session.close(false);
      setTree(runtime, closePane(runtime.tree));
    },
    focus(which: "next" | "prev" | number) {
      const runtime = activeRuntime();
      if (!runtime) return;
      const paneId =
        which === "next"
          ? nextPane(runtime.tree)
          : which === "prev"
            ? prevPane(runtime.tree)
            : which;
      setTree(runtime, focusPane(runtime.tree, paneId));
    },
    resize(direction: "left" | "right" | "up" | "down") {
      const runtime = activeRuntime();
      if (!runtime) return;
      setTree(runtime, resizeActive(runtime.tree, direction));
    },
    move(direction: SplitChild) {
      const runtime = activeRuntime();
      if (!runtime) return;
      setTree(runtime, movePane(runtime.tree, direction));
    },
    setRatio(path: SplitChild[], ratio: number) {
      const runtime = activeRuntime();
      if (!runtime) return;
      setTree(runtime, updateRatioAt(runtime.tree, path, ratio));
    },
    openPath(path: string) {
      const runtime = activeRuntime();
      if (!runtime) return;
      const existing = findPaneWithPath(runtime, path);
      if (existing) {
        setTree(runtime, focusPane(runtime.tree, existing.paneId));
        return;
      }
      ensurePane(runtime, runtime.tree.activePaneId).session.open(path);
    },
    openFileDialog() {
      const runtime = activeRuntime();
      if (runtime)
        dispatchClientCommand(
          commandContext,
          runtime,
          "documents.clientOpenFileDialog",
        );
    },
    openFolderDialog() {
      const runtime = activeRuntime();
      if (runtime)
        dispatchClientCommand(
          commandContext,
          runtime,
          "workspace.clientOpenFolderDialog",
        );
    },
    /** Global server-first keymaps from the active session's behavior
     *  manifest — the shell chord matcher resolves these outside editor
     *  focus (the editor keymap owns them inside `.cm-editor`). */
    serverKeymaps() {
      const runtime = activeRuntime();
      const session = runtime?.panes.get(runtime.tree.activePaneId)?.session;
      if (!session) return [];
      const keymaps =
        (session.behaviorManifest().keymaps as
          | Array<{
              commandId: string;
              sequence: ServerKeyStroke[];
              context?: unknown;
              routingPolicy?: unknown;
            }>
          | undefined) ?? [];
      // serde emits unit-variant names as written ("Global",
      // "ServerFirst"); package strings may use kebab. Normalize all three
      // spellings before comparing.
      const normalize = (value: unknown) =>
        String(value).toLowerCase().replace(/[-_]/g, "");
      return keymaps.filter(
        (binding) =>
          normalize(binding.context) === "global" &&
          normalize(binding.routingPolicy) === "serverfirst",
      );
    },
    /** Send a server-first command intent through the active pane session
     *  (same lane the editor extension controller uses). Returns false when
     *  no session can carry it. */
    dispatchServerCommand(commandId: string) {
      const runtime = activeRuntime();
      const session = runtime?.panes.get(runtime.tree.activePaneId)?.session;
      if (!session) return false;
      const store = session.store.get();
      void session.request(
        JSON.stringify({
          family: "commandIntent",
          payload: {
            clientId: session.clientId(),
            documentId: store?.documentId ?? 0,
            behaviorVersion:
              store?.behaviorVersion ??
              session.behaviorManifest().behaviorVersion,
            commandId,
          },
        }),
      );
      return true;
    },
    serialize,
    emptyLayout,
  };
  return workspace;
}

function runtimeDocuments(
  runtime: TabRuntime | undefined,
): Map<number, string | null> {
  const documents = new Map<number, string | null>();
  if (!runtime) return documents;
  for (const id of paneIds(runtime.tree.root)) {
    documents.set(id, runtime.panes.get(id)?.session.store.get()?.path || null);
  }
  return documents;
}

export type WorkspaceController = ReturnType<typeof createWorkspace>;
