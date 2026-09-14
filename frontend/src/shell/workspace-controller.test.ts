import { EditorState } from "@codemirror/state";
import { describe, expect, it, vi } from "vitest";

import type { BootstrapDto } from "../bridge/types";
import type { RuntimeSnapshot } from "../sdui/types";
import { tabsFromWindow } from "./persist";
import { agentInspector, workspaceRail } from "./layout-state";
import { createWorkspace } from "./workspace-controller";

function bootstrap(
  over: Partial<BootstrapDto> & { clientId: number },
): BootstrapDto {
  return {
    protocolVersion: 28,
    endpoint: "test",
    generation: 1,
    initialDocument: {
      documentId: over.clientId as never,
      version: 1,
      head: { totalBytes: 4, firstChunk: "seed" },
      access: { editable: { leaseId: 1 } },
      workspaceRoot: `/tmp/ws${over.clientId}`,
    },
    behaviorManifest: {
      manifestId: "m",
      behaviorVersion: 2,
      commands: [],
      keymaps: [],
    },
    activeTheme: { specifier: "", tokens: {}, densityScale: 1 },
    activeTypography: {
      revision: 1,
      monospace: {
        families: ["m"],
        size: 13,
        ligatures: { enableStandard: true },
      },
      proportional: {
        families: ["p"],
        size: 13,
        ligatures: { enableStandard: true },
      },
      ui: { families: ["u"], size: 13, ligatures: { enableStandard: true } },
      hierarchy: {
        display: 1.5,
        title: 1,
        section: 1,
        body: 1,
        status: 1,
        detail: 0.8,
        caption: 0.75,
      },
    },
    activeDesignSystem: {
      specifier: "@clay/core",
      schemaVersion: 1,
      generation: 1,
      provenance: {
        packageName: "core",
        packageVersion: "1.0.0",
        apiPrefix: "clay",
        trustDomain: "trusted",
      },
      recipes: {},
      variables: {},
    },
    ...over,
  } as BootstrapDto;
}

describe("per-tab layout visibility (plan 118 task E2)", () => {
  it("keeps rail and inspector visibility per tab and persists both", async () => {
    vi.useFakeTimers();
    const saved: unknown[] = [];
    const ws = createWorkspace({
      send: async () => {},
      saveLayout: async (state) => {
        saved.push(state);
      },
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    expect(agentInspector.isVisible()).toBe(true);
    expect(workspaceRail.isVisible()).toBe(true);

    // Tab 1: hide both — a toggle alone schedules the layout write.
    workspaceRail.setVisible(false);
    agentInspector.setVisible(false);
    expect(workspaceRail.isVisible()).toBe(false);
    vi.advanceTimersByTime(300);
    await Promise.resolve();
    const afterToggle = saved.at(-1) as {
      tabs: Array<{ railVisible: boolean; inspectorVisible: boolean }>;
    };
    expect(afterToggle.tabs[0]).toMatchObject({
      railVisible: false,
      inspectorVisible: false,
    });

    // Tab 2: its own shape (both visible again) — switching tabs re-reads.
    ws.installBootstrap(bootstrap({ clientId: 2 }));
    await ws.activate(2);
    expect(workspaceRail.isVisible()).toBe(true);
    expect(agentInspector.isVisible()).toBe(true);
    workspaceRail.setVisible(true);

    // Back to tab 1: the hidden rail is still hidden for that tab.
    await ws.activate(1);
    expect(workspaceRail.isVisible()).toBe(false);
    expect(agentInspector.isVisible()).toBe(false);

    // Persist: one entry per tab, from the same store.
    vi.advanceTimersByTime(300);
    await Promise.resolve();
    const layout = saved.at(-1) as {
      tabs: Array<{ railVisible: boolean; inspectorVisible: boolean }>;
    };
    expect(layout.tabs).toHaveLength(2);
    expect(layout.tabs[0]).toMatchObject({
      railVisible: false,
      inspectorVisible: false,
    });
    expect(layout.tabs[1]).toMatchObject({
      railVisible: true,
      inspectorVisible: true,
    });
    vi.useRealTimers();
  });

  it("restores what the layout recorded, per tab", async () => {
    const ws = createWorkspace({
      send: async () => {},
      loadLayout: async () => ({
        version: 2,
        activeTab: 0,
        tabs: [
          {
            workspaceRoot: "/tmp/one",
            railVisible: false,
            inspectorVisible: false,
          },
        ],
      }),
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    await ws.restore();
    expect(workspaceRail.isVisible()).toBe(false);
    expect(agentInspector.isVisible()).toBe(false);
  });
});

describe("workspace controller", () => {
  it("adopts the tab id from a pre-bootstrap registry so server-first chords route to the right tab", async () => {
    const sent: Array<{ payload: string; tabId?: number }> = [];
    const ws = createWorkspace({
      send: async (payload, tabId) => {
        sent.push({ payload, tabId });
      },
      loadLayout: async () => ({
        version: 2,
        activeTab: 0,
        tabs: [{ clientId: 2, kind: "placeholder" }],
      }),
    });

    // The server tab registry broadcasts during the handshake before
    // mountRuntime sees the bootstrap — same race they already fixed for
    // root ids. The remembered tab id must be adopted at mount time.
    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "tabRegistry",
        data: {
          tabs: [
            {
              tabId: 20,
              clientId: 2,
              workspaceRoot: "/tmp/ws2",
              workspaceRootId: 4,
            },
          ],
          active: 2,
          revision: 1,
        },
      },
    });

    ws.installBootstrap(bootstrap({ clientId: 2 })); // bootstrap.tabId omitted
    await ws.activate(2);

    expect(ws.runtime(2)?.tabId).toBe(20);
    expect(ws.dispatchServerCommand("controlCenter.open")).toBe(true);
    await Promise.resolve();
    const last = sent.at(-1);
    expect(last).toBeDefined();
    expect(last?.tabId).toBe(20);
  });

  it("restore flushes a queued open when the handshake registry raced the bootstrap", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
      loadLayout: async () => ({
        version: 2,
        activeTab: 0,
        tabs: [
          {
            workspaceRoot: "/tmp/ws1",
            activePane: 1,
            splitTree: { leaf: { paneId: 1 } },
            slots: [],
            panes: { "1": "notes.md" },
          },
        ],
      }),
    });
    // Fresh boot: the server broadcasts the tab registry during the
    // handshake, before the bootstrap command installs the runtime.
    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "tabRegistry",
        data: {
          tabs: [
            {
              tabId: 1,
              clientId: 1,
              workspaceRoot: "/tmp/ws1",
              workspaceRootId: 3,
            },
          ],
          active: 1,
          revision: 1,
        },
      },
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    await ws.restore();
    expect(
      sent.some(
        (payload) =>
          payload.includes('"family":"openDocument"') &&
          payload.includes('"workspaceRootId":3'),
      ),
    ).toBe(true);
  });

  it("restore opens persisted documents once the root id arrives", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
      loadLayout: async () => ({
        version: 2,
        activeTab: 0,
        tabs: [
          {
            workspaceRoot: "/tmp/ws1",
            activePane: 1,
            splitTree: { leaf: { paneId: 1 } },
            slots: [],
            panes: { "1": "notes.md" },
          },
        ],
      }),
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    await ws.restore();
    // Open queued (no root id yet); no openDocument payload sent.
    expect(
      sent.some((payload) => payload.includes('"family":"openDocument"')),
    ).toBe(false);

    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "documentStatus",
        data: {
          documentId: 1,
          version: 1,
          dirty: false,
          access: { editable: { leaseId: 1 } },
          workspaceRootId: 3,
          path: "",
        },
      },
    });
    expect(
      sent.some((payload) => payload.includes('"family":"openDocument"')),
    ).toBe(true);
    expect(sent.some((payload) => payload.includes('"path":"notes.md"'))).toBe(
      true,
    );
  });

  it("restore opens persisted documents when the tab registry delivers the root id", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
      loadLayout: async () => ({
        version: 2,
        activeTab: 0,
        tabs: [
          {
            workspaceRoot: "/tmp/ws1",
            activePane: 1,
            splitTree: { leaf: { paneId: 1 } },
            slots: [],
            panes: { "1": "notes.md" },
          },
        ],
      }),
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    await ws.restore();
    expect(
      sent.some((payload) => payload.includes('"family":"openDocument"')),
    ).toBe(false);

    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "tabRegistry",
        data: {
          tabs: [
            {
              tabId: 1,
              clientId: 1,
              workspaceRoot: "/tmp/ws1",
              workspaceRootId: 3,
            },
          ],
          active: 1,
          revision: 1,
        },
      },
    });
    expect(
      sent.some((payload) => payload.includes('"family":"openDocument"')),
    ).toBe(true);
    expect(
      sent.some((payload) => payload.includes('"workspaceRootId":3')),
    ).toBe(true);
  });

  it("restore opens each persisted pane through OpenDocument, not a bootstrap snapshot", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
      loadLayout: async () => ({
        version: 2,
        activeTab: 0,
        tabs: [
          {
            workspaceRoot: "/tmp/ws1",
            activePane: 1,
            splitTree: {
              split: {
                orientation: "horizontal",
                ratio: 0.5,
                first: { leaf: { paneId: 1 } },
                second: {
                  split: {
                    orientation: "vertical",
                    ratio: 0.5,
                    first: { leaf: { paneId: 2 } },
                    second: { leaf: { paneId: 3 } },
                  },
                },
              },
            },
            slots: [],
            panes: {
              "1": "a.txt",
              "2": "b.txt",
              "3": "large.txt",
            },
          },
        ],
      }),
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    await ws.restore();
    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "tabRegistry",
        data: {
          tabs: [
            {
              tabId: 1,
              clientId: 1,
              workspaceRoot: "/tmp/ws1",
              workspaceRootId: 3,
            },
          ],
          active: 1,
          revision: 1,
        },
      },
    });
    const opens = sent.filter((payload) =>
      payload.includes('"family":"openDocument"'),
    );
    expect(opens).toHaveLength(3);
    expect(opens.some((payload) => payload.includes('"path":"a.txt"'))).toBe(
      true,
    );
    expect(opens.some((payload) => payload.includes('"path":"b.txt"'))).toBe(
      true,
    );
    expect(
      opens.some((payload) => payload.includes('"path":"large.txt"')),
    ).toBe(true);
    expect(sent.some((payload) => payload.includes("bootstrapSnapshot"))).toBe(
      false,
    );
  });

  it("routes each restored pane's open reply to the pane that requested it", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
      loadLayout: async () => ({
        version: 2,
        activeTab: 0,
        tabs: [
          {
            workspaceRoot: "/tmp/ws1",
            activePane: 1,
            splitTree: {
              split: {
                orientation: "horizontal",
                ratio: 0.5,
                first: { leaf: { paneId: 1 } },
                second: { leaf: { paneId: 2 } },
              },
            },
            slots: [],
            panes: { "1": "notes.md", "2": "review.rs" },
          },
        ],
      }),
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    await ws.restore();
    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "tabRegistry",
        data: {
          tabs: [
            {
              tabId: 1,
              clientId: 1,
              workspaceRoot: "/tmp/ws1",
              workspaceRootId: 3,
            },
          ],
          active: 1,
          revision: 1,
        },
      },
    });

    const documentOpened = (path: string, documentId: number) => ({
      kind: "event" as const,
      data: {
        kind: "documentOpened",
        data: {
          metadata: {
            documentId,
            version: 1,
            dirty: false,
            access: { editable: { leaseId: documentId } },
            workspaceRootId: 3,
            path,
          },
          head: { totalBytes: 4, firstChunk: `seed of ${path}` },
        },
      },
    });

    // Replies arrive out of order; each pane must receive its own document.
    ws.handleEnvelope(documentOpened("review.rs", 7));
    ws.handleEnvelope(documentOpened("notes.md", 6));

    const runtime = ws.runtime(1);
    expect(runtime).not.toBeNull();
    const paths = new Map(
      [...(runtime?.panes ?? [])].map(([paneId, pane]) => {
        const meta = pane.session.store.get();
        return [paneId, { path: meta?.path, documentId: meta?.documentId }];
      }),
    );
    expect(paths.get(1)).toEqual({ path: "notes.md", documentId: 6 });
    expect(paths.get(2)).toEqual({ path: "review.rs", documentId: 7 });
  });

  it("keeps split trees and documents isolated per tab", () => {
    const sent: Array<{ payload: string; tabId?: number }> = [];
    const ws = createWorkspace({
      send: async (payload, tabId) => {
        sent.push({ payload, tabId });
      },
    });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    ws.installBootstrap(bootstrap({ clientId: 2 }));
    void ws.activate(1);
    ws.split("horizontal");
    expect(ws.runtime(1)?.tree.root.kind).toBe("split");
    expect(ws.runtime(2)?.tree.root.kind).toBe("leaf");

    ws.handleEnvelope({
      kind: "routed",
      data: {
        clientId: 1,
        tabId: 10,
        event: {
          kind: "documentOpened",
          data: {
            metadata: {
              documentId: 7,
              version: 1,
              dirty: false,
              access: { editable: { leaseId: 1 } },
              path: "a.md",
              workspaceRootId: 1,
            },
            head: { totalBytes: 3, firstChunk: "aaa" },
          },
        },
      },
    });
    expect(ws.runtime(1)?.panes.get(1)?.session.store.get()?.path).toBe("a.md");
    expect(ws.runtime(2)?.panes.get(1)?.session.store.get()?.path).toBe("");
  });

  it("installs one routed runtime UI generation and acknowledges after state replacement", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    const initial = bootstrap({ clientId: 1, tabId: 10 });
    ws.installBootstrap(initial);
    const snapshot: RuntimeSnapshot = {
      runtimeGenerationId: 8,
      behaviorManifest: initial.behaviorManifest,
      activeTheme: initial.activeTheme,
      activeTypography: initial.activeTypography,
      activeDesignSystem: initial.activeDesignSystem,
      sduiTree: {
        uiVersion: 8,
        rootId: 1,
        nodes: [{ id: 1, kind: { label: { text: "Git" } } }],
      },
      packageUi: {
        version: 8,
        emptyTab: null,
        panels: [],
        overlays: [],
        components: [],
        inputRoutes: [],
      },
      documents: [],
      diagnostics: [],
    };
    ws.handleEnvelope({
      kind: "runtimeSnapshot",
      data: { clientId: 1, tabId: 10, snapshot },
    });
    expect(ws.runtime(1)?.ui.runtimeGeneration).toBe(8);
    expect(ws.runtime(1)?.ui.sdui?.nodes.get(1)?.kind).toEqual({
      label: { text: "Git" },
    });
    expect(JSON.parse(sent.at(-1) ?? "")).toMatchObject({
      family: "runtimeGenerationInstalled",
      payload: { runtimeGenerationId: 8 },
    });
  });

  it("keeps transient menu state server-authoritative and routes opaque intents", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    ws.installBootstrap(bootstrap({ clientId: 1, tabId: 10 }));
    ws.handleEnvelope({
      kind: "routed",
      data: {
        clientId: 1,
        tabId: 10,
        event: {
          kind: "transientMenuSnapshot",
          data: {
            sessionId: "9223372036854775809" as never,
            prompt: "Command Centre",
            query: "git",
            items: [
              {
                id: "git.refresh",
                label: "Refresh Git",
                detail: "@clay/git",
                accessibilityLabel: "Refresh Git",
              },
            ],
            selectedIndex: 0,
            status: "active",
            focusPolicy: "modal",
            origin: "centered",
          },
        },
      },
    });
    expect(ws.active()?.menu?.query).toBe("git");
    ws.menuQuery("reload");
    ws.menuMove(1);
    ws.menuActivate();
    await Promise.resolve();
    expect(sent.map((payload) => JSON.parse(payload).family)).toEqual([
      "getDocumentStatus",
      "menuQueryUpdate",
      "menuSelectionMove",
      "menuActivate",
    ]);
    ws.handleEnvelope({
      kind: "routed",
      data: {
        clientId: 1,
        tabId: 10,
        event: {
          kind: "transientMenuClosed",
          data: { sessionId: "9223372036854775809" as never },
        },
      },
    });
    expect(ws.active()?.menu).toBeNull();
  });

  it("closes the transient menu optimistically on cancel and launches the coding agent client-side", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    ws.installBootstrap(bootstrap({ clientId: 1, tabId: 10 }));
    ws.handleEnvelope({
      kind: "routed",
      data: {
        clientId: 1,
        tabId: 10,
        event: {
          kind: "transientMenuSnapshot",
          data: {
            sessionId: "9223372036854775809" as never,
            prompt: "Command Centre",
            query: "",
            items: [],
            selectedIndex: 0,
            status: "active",
            focusPolicy: "modal",
            origin: "centered",
          },
        },
      },
    });
    ws.menuCancel();
    // Local close is immediate — the UI must not wait on the server.
    expect(ws.active()?.menu).toBeNull();
    await Promise.resolve();
    const cancel = sent.find(
      (payload) => JSON.parse(payload).family === "menuCancel",
    );
    expect(cancel).toBeDefined();

    // Plan 118 task 33: the agent is a *view* of the tab, not a pane surface.
    // Attaching it keeps the workspace half; the view switches with it.
    ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    const attached = ws.tabs.get().tabs[0];
    expect(attached?.agent?.type).toBe("coding-agent");
    expect(attached?.view).toBe("agent");
    ws.setView("workspace");
    expect(ws.tabs.get().tabs[0]?.view).toBe("workspace");
    expect(ws.tabs.get().tabs[0]?.agent).not.toBeNull();
  });

  it("exposes global server-first keymaps and dispatches their command intents", async () => {
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    const manifest = {
      manifestId: "m",
      behaviorVersion: 7,
      commands: [],
      keymaps: [
        {
          commandId: "controlCenter.open",
          sequence: [
            {
              key: { character: "x" },
              modifiers: {
                shift: false,
                control: true,
                alt: false,
                superKey: false,
              },
            },
            {
              key: { character: "p" },
              modifiers: {
                shift: false,
                control: true,
                alt: false,
                superKey: false,
              },
            },
          ],
          context: "Global",
          routingPolicy: "ServerFirst",
        },
        {
          commandId: "shell.clientSplitPaneVertical",
          sequence: [
            {
              key: { character: "\\" },
              modifiers: {
                shift: false,
                control: true,
                alt: false,
                superKey: false,
              },
            },
          ],
          context: "Global",
          routingPolicy: "ClientUiCommand",
        },
      ],
    };
    ws.installBootstrap(bootstrap({ clientId: 1, behaviorManifest: manifest }));
    const keymaps = ws.serverKeymaps();
    // serde emits unit-variant enum names as written ("Global",
    // "ServerFirst") — the filter must match the wire spelling, and the
    // ClientUiCommand-routed pane chord must stay client-owned.
    expect(keymaps).toHaveLength(1);
    expect(keymaps[0]?.commandId).toBe("controlCenter.open");

    expect(ws.dispatchServerCommand("controlCenter.open")).toBe(true);
    await Promise.resolve();
    const raw = sent[sent.length - 1];
    expect(raw).toBeDefined();
    const payload = JSON.parse(raw as string);
    expect(payload.family).toBe("commandIntent");
    expect(payload.payload.commandId).toBe("controlCenter.open");
    expect(payload.payload.behaviorVersion).toBe(7);
    expect(typeof payload.payload.documentId).toBe("number");
  });

  it("executes only routed client workflow commands", async () => {
    const dialogs: string[] = [];
    const sent: string[] = [];
    const ws = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
      openFileDialog: async () => {
        dialogs.push("file");
        return true;
      },
    });
    ws.installBootstrap(bootstrap({ clientId: 1, tabId: 10 }));
    const command = (commandId: string) =>
      ws.handleEnvelope({
        kind: "routed",
        data: {
          clientId: 1,
          tabId: 10,
          event: { kind: "shellClientCommandRequest", data: { commandId } },
        },
      });
    command("settings.open");
    expect(ws.active()?.settingsOpen).toBe(true);
    command("documents.clientOpenFileDialog");
    await Promise.resolve();
    expect(dialogs).toEqual(["file"]);
    command("settings.close");
    expect(ws.active()?.settingsOpen).toBe(false);
    command("documents.clientOpenFileDialog.evil");
    await Promise.resolve();
    expect(dialogs).toEqual(["file"]);

    const documentId = ws
      .active()
      ?.panes.get(1)
      ?.session.store.get()?.documentId;
    ws.handleEnvelope({
      kind: "routed",
      data: {
        clientId: 1,
        tabId: 10,
        event: {
          kind: "documentClosed",
          data: { documentId, closed: true },
        },
      },
    });
    ws.openFileDialog();
    await Promise.resolve();
    expect(dialogs).toEqual(["file", "file"]);
  });

  it("keeps per-keystroke acks from rerendering the shell or persisting", () => {
    const ws = createWorkspace({ send: async () => undefined });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    let notifies = 0;
    ws.subscribe(() => {
      notifies += 1;
    });
    const runtime = ws.runtime(1);
    const pane =
      runtime && runtime.panes.get(runtime.tree.activePaneId)
        ? runtime.panes.get(runtime.tree.activePaneId)
        : null;
    if (!runtime || !pane) throw new Error("active pane missing");
    const baseline = notifies;

    // One user edit flips dirty: exactly one shell notification.
    pane.session.emitUserChanges(EditorState.create({ doc: "seed" }).doc, [
      { from: 4, to: 4, insert: "!" },
    ]);
    expect(notifies).toBe(baseline + 1);

    // A second edit while already dirty: no further notification.
    pane.session.emitUserChanges(EditorState.create({ doc: "seed!" }).doc, [
      { from: 5, to: 5, insert: "?" },
    ]);
    expect(notifies).toBe(baseline + 1);

    // The ack (version/pending only) must not notify the shell.
    ws.handleEnvelope({
      kind: "event",
      data: {
        kind: "editAck",
        data: { documentId: 1, version: 2, transactionId: 1 },
      },
    });
    expect(notifies).toBe(baseline + 1);
    expect(pane.session.store.get()?.version).toBe(2);
    expect(pane.session.store.get()?.pending).toBe(1);
  });

  it("notifies shell status for loading and diagnostics without ack churn", () => {
    const ws = createWorkspace({ send: async () => undefined });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    let notifies = 0;
    ws.subscribe(() => {
      notifies += 1;
    });
    const pane = ws.runtime(1)?.panes.get(1);
    expect(pane).toBeTruthy();
    const baseline = notifies;
    const snapshotBeforeLoading = ws.getSnapshot();

    pane?.session.store.update({ loading: true });
    expect(ws.getSnapshot()).not.toBe(snapshotBeforeLoading);
    expect(notifies).toBe(baseline + 1);
    pane?.session.store.update({ loading: true, pending: 1 });
    expect(notifies).toBe(baseline + 1);
    pane?.session.store.update({ loading: false });
    expect(notifies).toBe(baseline + 2);
    pane?.session.store.update({ diagnostic: "file failed" });
    expect(notifies).toBe(baseline + 3);
    pane?.session.store.update({ diagnostic: "file failed", version: 2 });
    expect(notifies).toBe(baseline + 3);
  });

  it("focuses the existing pane on a duplicate open", () => {
    const ws = createWorkspace({ send: async () => undefined });
    ws.installBootstrap(bootstrap({ clientId: 1 }));
    ws.split("horizontal");
    const left = ws.runtime(1)?.panes.get(1);
    expect(left).toBeTruthy();
    left?.session.store.update({ path: "notes.md", workspaceRootId: 1 });
    ws.focus(2);
    expect(ws.active()?.tree.activePaneId).toBe(2);
    ws.openPath("notes.md");
    expect(ws.active()?.tree.activePaneId).toBe(1);
  });

  it("blocks dirty tab close until confirmed", async () => {
    const closed: number[] = [];
    const ws = createWorkspace({
      send: async () => undefined,
      closeTab: async (tabId) => {
        closed.push(tabId);
      },
    });
    ws.installBootstrap(bootstrap({ clientId: 1, tabId: 10 }));
    ws.installBootstrap(bootstrap({ clientId: 2, tabId: 11 }));
    const dirtyPane = ws.runtime(2)?.panes.get(1);
    expect(dirtyPane).toBeTruthy();
    dirtyPane?.session.store.update({ dirty: true, path: "x.md" });
    ws.requestClose(2);
    expect(ws.pendingClose()?.dirtyPaths).toEqual(["x.md"]);
    expect(ws.getSnapshot().tabs).toHaveLength(2);
    await ws.confirmClose(2, false);
    expect(closed).toEqual([11]);
    expect(ws.getSnapshot().tabs.map((tab) => tab.clientId)).toEqual([1]);
  });

  it("refuses to close the last tab", () => {
    const ws = createWorkspace({ send: async () => undefined });
    ws.installBootstrap(bootstrap({ clientId: 1, tabId: 10 }));
    ws.requestClose(1);
    expect(ws.getSnapshot().tabs).toHaveLength(1);
    expect(ws.pendingClose()).toBeNull();
  });
});

describe("persisted layout fallback", () => {
  it("drops corrupt or hostile documents", () => {
    expect(tabsFromWindow(null)).toBeNull();
    expect(tabsFromWindow({ version: 1, tabs: [] })).toBeNull();
    expect(
      tabsFromWindow({ version: 2, tabs: [{ workspaceRoot: "" }] }),
    ).toBeNull();
    const ok = tabsFromWindow({
      version: 2,
      activeTab: 0,
      tabs: [
        {
          workspaceRoot: "/tmp/ws",
          activePane: 1,
          splitTree: { leaf: { paneId: 1 } },
          slots: [],
          panes: { "1": "a.md" },
        },
      ],
    });
    expect(ok?.tabs).toHaveLength(1);
    expect(ok?.tabs[0]?.documents.get(1)).toBe("a.md");
  });
});
