import { describe, expect, it } from "vitest";

import type { BootstrapDto } from "../bridge/types";
import type { RuntimeSnapshot } from "../sdui/types";
import { tabsFromWindow } from "./persist";
import { createWorkspace } from "./workspace-controller";

function bootstrap(
  over: Partial<BootstrapDto> & { clientId: number },
): BootstrapDto {
  return {
    protocolVersion: 26,
    endpoint: "test",
    generation: 1,
    initialDocument: {
      documentId: over.clientId as never,
      version: 1,
      text: "seed",
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
    ...over,
  } as BootstrapDto;
}

describe("workspace controller", () => {
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
            text: "aaa",
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

  it("executes only routed client workflow commands", async () => {
    const dialogs: string[] = [];
    const ws = createWorkspace({
      send: async () => undefined,
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
