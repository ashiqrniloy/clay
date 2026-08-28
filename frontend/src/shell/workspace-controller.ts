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
import {
  applySduiUpdate,
  emptyUiProjection,
  installSduiTree,
  type UiProjectionState,
} from "../sdui/state";
import type { SduiTree, SduiTreeUpdate } from "../sdui/types";
import {
  createDocumentSession,
  type DocumentSession,
  type SendFn,
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
import {
  applyRegistry,
  createTabStore,
  emptyTabs,
  markDirty,
  removeTab,
  tabLabel,
  upsertTab,
  type TabSnapshot,
  type TabStore,
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
  openTabDialog?: () => Promise<BootstrapDto | null>;
}

export interface PaneRecord {
  paneId: number;
  session: DocumentSession;
}

export interface TabRuntime {
  clientId: number;
  tabId: number | null;
  workspaceRoot: string;
  /** Server workspace root id from the tab registry; null until known. */
  workspaceRootId: number | null;
  tree: SplitTree;
  panes: Map<number, PaneRecord>;
  ui: UiProjectionState;
  menu: TransientMenuSnapshotDto | null;
  diagnostic: RuntimeDiagnosticDto | null;
  settingsOpen: boolean;
}

export interface PendingClose {
  clientId: number;
  dirtyPaths: string[];
}

function sendFor(adapters: WorkspaceAdapters, tabId: number | null): SendFn {
  return (payload) => adapters.send(payload, tabId ?? undefined);
}

export function createWorkspace(adapters: WorkspaceAdapters) {
  const tabs: TabStore = createTabStore();
  const runtimes = new Map<number, TabRuntime>();
  let pendingClose: PendingClose | null = null;
  // The server broadcasts the tab registry during handshake, before the
  // bootstrap command creates the runtime, so a fresh-boot restore would
  // otherwise queue document opens forever waiting for a root id that
  // already arrived. Remember the latest roots per client.
  const registryRootsByClient = new Map<number, number>();
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
      void adapters.saveLayout?.(serialize());
    }, 250);
  };

  const bindSession = (runtime: TabRuntime): DocumentSession => {
    const session = createDocumentSession({
      send: sendFor(adapters, runtime.tabId),
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
  ): TabRuntime => {
    const runtime: TabRuntime = {
      clientId: bootstrap.clientId,
      tabId: bootstrap.tabId ?? null,
      workspaceRoot: bootstrap.initialDocument.workspaceRoot,
      workspaceRootId: null,
      tree,
      panes: new Map(),
      ui: emptyUiProjection(),
      menu: null,
      diagnostic: null,
      settingsOpen: false,
    };
    const first = ensurePane(runtime, tree.activePaneId);
    first.session.installInitial(bootstrap);
    runtimes.set(bootstrap.clientId, runtime);
    tabs.set(
      upsertTab(tabs.get(), {
        tabId: runtime.tabId,
        clientId: runtime.clientId,
        workspaceRoot: runtime.workspaceRoot,
        label: tabLabel(runtime.workspaceRoot),
        dirty: false,
        disconnected: false,
      }),
    );
    return runtime;
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

  const sendTabCommand = (runtime: TabRuntime, command: unknown) =>
    adapters.send(
      JSON.stringify({
        family: "tabCommand",
        payload: { clientId: runtime.clientId, command },
      }),
      runtime.tabId ?? undefined,
    );

  const dispatchClientCommand = (runtime: TabRuntime, commandId: string) => {
    const activePane = runtime.panes.get(runtime.tree.activePaneId);
    if (activePane?.session.runClientCommand(commandId)) return;
    const direct: Record<string, () => void> = {
      "shell.clientSplitPaneVertical": () =>
        setTree(runtime, splitPane(runtime.tree, "horizontal")),
      "shell.clientSplitPaneHorizontal": () =>
        setTree(runtime, splitPane(runtime.tree, "vertical")),
      "shell.clientSplitPaneRight": () =>
        setTree(runtime, splitPane(runtime.tree, "horizontal")),
      "shell.clientSplitPaneDown": () =>
        setTree(runtime, splitPane(runtime.tree, "vertical")),
      "shell.clientAddEqualPane": () =>
        setTree(runtime, addEqualPane(runtime.tree)),
      "shell.clientClosePane": () => {
        activePane?.session.close(false);
        setTree(runtime, closePane(runtime.tree));
      },
      "shell.clientFocusPaneNext": () =>
        setTree(runtime, focusPane(runtime.tree, nextPane(runtime.tree))),
      "shell.clientFocusPanePrev": () =>
        setTree(runtime, focusPane(runtime.tree, prevPane(runtime.tree))),
      "shell.clientResizePaneLeft": () =>
        setTree(runtime, resizeActive(runtime.tree, "left")),
      "shell.clientResizePaneRight": () =>
        setTree(runtime, resizeActive(runtime.tree, "right")),
      "shell.clientResizePaneUp": () =>
        setTree(runtime, resizeActive(runtime.tree, "up")),
      "shell.clientResizePaneDown": () =>
        setTree(runtime, resizeActive(runtime.tree, "down")),
      "shell.clientMovePaneNext": () =>
        setTree(runtime, movePane(runtime.tree, "second")),
      "shell.clientMovePanePrev": () =>
        setTree(runtime, movePane(runtime.tree, "first")),
      "documents.clientOpenFileDialog": () => {
        adapters
          .openFileDialog?.(runtime.tabId ?? undefined)
          ?.catch((error: unknown) => {
            // Dialog failures must be visible: a busy portal lock or a failed
            // native dialog otherwise looks like a dead button.
            runtime.diagnostic = {
              severity: "error",
              code: "dialog.failed",
              message:
                error instanceof Error
                  ? error.message
                  : "File dialog could not open",
            };
            notify();
          });
      },
      "workspace.clientOpenFolderDialog": () => {
        adapters
          .openFolderDialog?.(runtime.tabId ?? undefined)
          ?.catch((error: unknown) => {
            runtime.diagnostic = {
              severity: "error",
              code: "dialog.failed",
              message:
                error instanceof Error
                  ? error.message
                  : "Folder dialog could not open",
            };
            notify();
          });
      },
      "settings.open": () => {
        runtime.settingsOpen = true;
        notify();
      },
      "settings.close": () => {
        runtime.settingsOpen = false;
        notify();
      },
    };
    if (direct[commandId]) {
      direct[commandId]();
      return;
    }
    const snapshot = tabs.get();
    const index = snapshot.tabs.findIndex(
      (tab) => tab.clientId === runtime.clientId,
    );
    const activateOffset = (offset: number) => {
      const target = snapshot.tabs.at(
        (index + offset + snapshot.tabs.length) % snapshot.tabs.length,
      );
      if (target?.tabId != null) void adapters.activateTab?.(target.tabId);
    };
    if (commandId === "shell.clientTabNext") activateOffset(1);
    else if (commandId === "shell.clientTabPrev") activateOffset(-1);
    else if (commandId === "shell.clientTabClose" && runtime.tabId != null)
      void adapters.closeTab?.(runtime.tabId);
    else if (commandId === "shell.clientTabMoveLeft" && runtime.tabId != null)
      void sendTabCommand(runtime, { moveLeft: { tabId: runtime.tabId } });
    else if (commandId === "shell.clientTabMoveRight" && runtime.tabId != null)
      void sendTabCommand(runtime, { moveRight: { tabId: runtime.tabId } });
    else if (commandId === "shell.clientTabNew")
      void (async () => {
        const bootstrap = await adapters.openTabDialog?.();
        if (bootstrap) mountRuntime(bootstrap);
        notify();
      })();
    else {
      const position = Number(commandId.split(".").at(-1));
      if (Number.isInteger(position) && position >= 1 && position <= 9) {
        if (commandId.startsWith("shell.clientTabActivate.")) {
          const target = snapshot.tabs[position - 1];
          if (target?.tabId != null) void adapters.activateTab?.(target.tabId);
        } else if (
          commandId.startsWith("shell.clientTabMoveTo.") &&
          runtime.tabId != null
        ) {
          void sendTabCommand(runtime, {
            moveTo: { tabId: runtime.tabId, position },
          });
        }
      }
    }
  };

  function serialize() {
    const snapshot = tabs.get();
    const layouts: TabLayout[] = snapshot.tabs.map((tab) => {
      const runtime = runtimes.get(tab.clientId);
      return {
        workspaceRoot: tab.workspaceRoot,
        tree: runtime?.tree ?? singlePane(),
        documents: runtimeDocuments(runtime),
      };
    });
    const activeIndex = snapshot.tabs.findIndex(
      (tab) => tab.clientId === snapshot.activeClientId,
    );
    return windowFromTabs(layouts, activeIndex >= 0 ? activeIndex : null);
  }

  return {
    tabs,
    subscribe: (listener: () => void) => {
      persistListeners.add(listener);
      return () => persistListeners.delete(listener);
    },
    getSnapshot: (): TabSnapshot => tabs.get(),
    runtime: (clientId: number) => runtimes.get(clientId) ?? null,
    active: activeRuntime,
    pendingClose: () => pendingClose,
    reset() {
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
      if (first && parsed.tabs[0]) {
        first.tree = parsed.tabs[0].tree;
        for (const id of paneIds(first.tree.root)) ensurePane(first, id);
        for (const [paneId, path] of parsed.tabs[0].documents) {
          if (path) first.panes.get(paneId)?.session.open(path);
        }
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
          const runtime = mountRuntime(bootstrap, extra.tree);
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
      if (parsed.activeIndex != null) {
        const target = tabs.get().tabs[parsed.activeIndex];
        if (target) await this.activate(target.clientId);
      }
      notify();
    },
    handleEnvelope(envelope: BridgeEnvelope) {
      if (envelope.kind === "runtimeSnapshot") {
        const runtime = runtimes.get(envelope.data.clientId);
        const snapshot = envelope.data.snapshot;
        if (
          !runtime ||
          snapshot.runtimeGenerationId <= runtime.ui.runtimeGeneration
        )
          return;
        runtime.ui = {
          runtimeGeneration: snapshot.runtimeGenerationId,
          sdui: installSduiTree(snapshot.sduiTree),
          packageUi: snapshot.packageUi,
        };
        runtime.diagnostic = snapshot.diagnostics.at(-1) ?? null;
        if (
          !snapshot.packageUi.panels.some(
            (panel) => panel.provenance.packageName === "@clay/settings",
          )
        )
          runtime.settingsOpen = false;
        for (const pane of runtime.panes.values()) {
          const document = snapshot.documents.find(
            (candidate) =>
              Number(candidate.documentId) ===
              pane.session.store.get()?.documentId,
          );
          const manifest =
            (document?.behaviorManifest as Record<string, unknown> | null) ??
            snapshot.behaviorManifest;
          pane.session.handleEnvelope({
            kind: "event",
            data: {
              kind: "behaviorManifestInstalled",
              data: {
                behaviorVersion: Number(manifest.behaviorVersion ?? 0),
                manifest,
              },
            },
          });
          if (document?.initialDecorations) {
            pane.session.handleEnvelope({
              kind: "event",
              data: {
                kind: "decorationSet",
                data: document.initialDecorations,
              },
            });
          }
          if (document?.initialDiagnostics) {
            pane.session.handleEnvelope({
              kind: "event",
              data: {
                kind: "diagnosticSet",
                data: document.initialDiagnostics,
              },
            });
          }
        }
        void adapters.send(
          JSON.stringify({
            family: "runtimeGenerationInstalled",
            payload: {
              clientId: 0,
              runtimeGenerationId: snapshot.runtimeGenerationId,
            },
          }),
          runtime.tabId ?? undefined,
        );
        notify();
        return;
      }
      if (envelope.kind === "disconnected") {
        const clientId = envelope.data.clientId;
        if (clientId != null && runtimes.has(clientId)) {
          const next = {
            ...tabs.get(),
            tabs: tabs
              .get()
              .tabs.map((tab) =>
                tab.clientId === clientId
                  ? { ...tab, disconnected: true }
                  : tab,
              ),
          };
          tabs.set(next);
          notify();
        }
        return;
      }
      if (envelope.kind !== "event" && envelope.kind !== "routed") return;
      const routed =
        envelope.kind === "routed"
          ? envelope.data
          : { clientId: null, tabId: null, event: envelope.data };
      if (
        routed.event &&
        (routed.event as { kind?: string }).kind === "tabRegistry"
      ) {
        const data = (
          routed.event as { data: Parameters<typeof applyRegistry>[1] }
        ).data;
        tabs.set(applyRegistry(tabs.get(), data));
        // The registry carries the server root id for each tab. The initial
        // document's own status lookup can fail ("unknownDocument") before
        // the tab binding registers it, so this is the authoritative moment
        // pane sessions learn the root id and any restore-queued open fires.
        const registryRoots = new Map(
          (
            data.tabs as Array<{
              clientId: number;
              workspaceRootId?: number;
            }>
          ).map((entry) => [entry.clientId, entry.workspaceRootId]),
        );
        for (const [clientId, rootId] of registryRoots) {
          if (rootId != null) registryRootsByClient.set(clientId, rootId);
        }
        for (const tab of tabs.get().tabs) {
          const runtime = runtimes.get(tab.clientId);
          if (runtime) {
            runtime.tabId = tab.tabId;
            runtime.workspaceRoot = tab.workspaceRoot;
            const rootId = registryRoots.get(tab.clientId);
            if (rootId != null) {
              runtime.workspaceRootId = rootId;
              deliverRootId(runtime, rootId);
            }
          }
        }
        notify();
        return;
      }
      const eventEnvelope =
        envelope.kind === "routed"
          ? { kind: "event" as const, data: routed.event }
          : envelope;
      const owners =
        routed.clientId != null
          ? [runtimes.get(routed.clientId)].filter(Boolean)
          : [...runtimes.values()];
      const event = routed.event as {
        kind?: string;
        data?: Record<string, unknown>;
      };
      for (const runtime of owners) {
        if (!runtime) continue;
        if (event.kind === "sduiSnapshot") {
          const tree = event.data?.tree as SduiTree | undefined;
          if (tree) runtime.ui = { ...runtime.ui, sdui: installSduiTree(tree) };
          notify();
          continue;
        }
        if (event.kind === "sduiUpdate") {
          const update = event.data as unknown as SduiTreeUpdate;
          runtime.ui = {
            ...runtime.ui,
            sdui: applySduiUpdate(runtime.ui.sdui, update),
          };
          notify();
          continue;
        }
        if (event.kind === "transientMenuSnapshot") {
          runtime.menu = event.data as unknown as TransientMenuSnapshotDto;
          notify();
          continue;
        }
        if (event.kind === "transientMenuClosed") {
          const sessionId = String(event.data?.sessionId ?? "");
          if (runtime.menu?.sessionId === sessionId) runtime.menu = null;
          notify();
          continue;
        }
        if (event.kind === "runtimeDiagnostic") {
          runtime.diagnostic = event.data as unknown as RuntimeDiagnosticDto;
          notify();
          continue;
        }
        if (event.kind === "fileOperationFailed") {
          // Server-rejected file operations (too large, unauthorized, missing)
          // must surface in shell status; the pane session store alone renders
          // nothing for an empty pane.
          runtime.diagnostic = {
            severity: "error",
            code: String(event.data?.code ?? "file.error"),
            message: String(event.data?.message ?? "File operation failed"),
          };
          notify();
          continue;
        }
        if (event.kind === "documentOpened") {
          // A successful open clears stale file-operation errors (the
          // bootstrap placeholder's "unknown workspace document" status
          // lookup and any failed open attempt), then falls through to the
          // normal pane routing below. dialog./server. errors stay.
          const code = runtime.diagnostic?.code ?? "";
          if (!code.startsWith("dialog.") && !code.startsWith("server.")) {
            runtime.diagnostic = null;
          }
          notify();
        }
        if (event.kind === "serverError") {
          runtime.diagnostic = {
            severity: "error",
            code: String(event.data?.code ?? "server.error"),
            message: String(event.data?.message ?? "Server request failed"),
          };
          notify();
          continue;
        }
        if (event.kind === "shellClientCommandRequest") {
          const commandId = event.data?.commandId;
          if (typeof commandId === "string")
            dispatchClientCommand(runtime, commandId);
          continue;
        }
        const documentId = eventDocumentId(event);
        const matching =
          documentId == null
            ? []
            : [...runtime.panes.values()].filter(
                (pane) => pane.session.store.get()?.documentId === documentId,
              );
        // An unclaimed open reply belongs to whichever pane's OpenDocument is
        // still in flight (restores open several panes at once, and restored
        // placeholders can share a document id with early real ids); the
        // documentId match is next, then the active pane fallback.
        const openReplyPath =
          event.kind === "documentOpened"
            ? String(
                (event.data as { metadata?: { path?: unknown } } | undefined)
                  ?.metadata?.path ?? "",
              )
            : null;
        const awaiting =
          openReplyPath == null || openReplyPath === ""
            ? undefined
            : [...runtime.panes.values()].find(
                (pane) => pane.session.inFlightOpenPath() === openReplyPath,
              );
        const targets =
          event.kind === "documentOpened"
            ? [
                awaiting ??
                  matching[0] ??
                  ensurePane(runtime, runtime.tree.activePaneId),
              ]
            : matching.length > 0
              ? matching
              : [...runtime.panes.values()];
        for (const pane of targets) pane.session.handleEnvelope(eventEnvelope);
      }
    },
    activate(clientId: number) {
      const runtime = runtimes.get(clientId);
      if (!runtime) return Promise.resolve();
      tabs.set({ ...tabs.get(), activeClientId: clientId });
      notify();
      if (runtime.tabId != null && adapters.activateTab) {
        return adapters.activateTab(runtime.tabId);
      }
      return Promise.resolve();
    },
    async openTab(workspaceRoot: string) {
      if (!adapters.openTab) return;
      const bootstrap = await adapters.openTab(workspaceRoot);
      mountRuntime(bootstrap);
      tabs.set({ ...tabs.get(), activeClientId: bootstrap.clientId });
      schedulePersist();
    },
    async openTabDialog() {
      const bootstrap = await adapters.openTabDialog?.();
      if (!bootstrap) return;
      mountRuntime(bootstrap);
      tabs.set({ ...tabs.get(), activeClientId: bootstrap.clientId });
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
      void adapters.send(
        JSON.stringify({
          family: "menuQueryUpdate",
          payload: {
            clientId: runtime.clientId,
            sessionId: menu.sessionId,
            query,
          },
        }),
        runtime.tabId ?? undefined,
      );
    },
    menuBackspace() {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      void adapters.send(
        JSON.stringify({
          family: "menuBackspace",
          payload: { clientId: runtime.clientId, sessionId: menu.sessionId },
        }),
        runtime.tabId ?? undefined,
      );
    },
    menuMove(delta: number) {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      void adapters.send(
        JSON.stringify({
          family: "menuSelectionMove",
          payload: {
            clientId: runtime.clientId,
            sessionId: menu.sessionId,
            delta,
          },
        }),
        runtime.tabId ?? undefined,
      );
    },
    menuActivate(secondary = false) {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      void adapters.send(
        JSON.stringify({
          family: "menuActivate",
          payload: {
            clientId: runtime.clientId,
            sessionId: menu.sessionId,
            kind: secondary ? "secondary" : "primary",
          },
        }),
        runtime.tabId ?? undefined,
      );
    },
    menuCancel() {
      const runtime = activeRuntime();
      const menu = runtime?.menu;
      if (!runtime || !menu) return;
      void adapters.send(
        JSON.stringify({
          family: "menuCancel",
          payload: { clientId: runtime.clientId, sessionId: menu.sessionId },
        }),
        runtime.tabId ?? undefined,
      );
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
        dispatchClientCommand(runtime, "documents.clientOpenFileDialog");
    },
    openFolderDialog() {
      const runtime = activeRuntime();
      if (runtime)
        dispatchClientCommand(runtime, "workspace.clientOpenFolderDialog");
    },
    serialize,
    emptyLayout,
  };
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

function eventDocumentId(event: {
  kind?: string;
  data?: Record<string, unknown>;
}): number | null {
  const data = event.data;
  if (!data) return null;
  if (typeof data.documentId === "number") return data.documentId;
  const metadata = data.metadata;
  if (
    metadata &&
    typeof metadata === "object" &&
    "documentId" in metadata &&
    typeof metadata.documentId === "number"
  ) {
    return metadata.documentId;
  }
  if (event.kind === "decorationBatch" && Array.isArray(data)) {
    const first = data[0] as { documentId?: unknown } | undefined;
    return typeof first?.documentId === "number" ? first.documentId : null;
  }
  return null;
}

export type WorkspaceController = ReturnType<typeof createWorkspace>;
