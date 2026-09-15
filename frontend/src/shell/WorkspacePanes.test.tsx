// @vitest-environment jsdom
// Live-path regression tests for the tab's two views (plan 118 task 33):
// entering the agent view must actually render the agent surface (the chain
// runtimeSnapshot envelope → packageUi.surfaces → view switch → AgentView),
// the inactive view must stay mounted, and the launcher must be the landing of
// an *uncommitted* tab only.

import { describe, expect, it, vi, afterEach } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { behaviorManifestFixture } from "../test/contract-fixtures";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
}));

type StreamEvent = Record<string, unknown> & { type: string };
const harness = vi.hoisted(() => {
  const listeners = new Set<(event: StreamEvent) => void>();
  return {
    emit(event: StreamEvent): void {
      for (const listener of [...listeners]) listener(event);
    },
    subscribe(next: (event: StreamEvent) => void): void {
      listeners.add(next);
    },
  };
});

vi.mock("../agent/events", () => ({
  agentStream: {
    events: {
      subscribe: (observer: { next: (event: StreamEvent) => void }) => {
        harness.subscribe(observer.next);
        return { unsubscribe: () => undefined };
      },
    },
    retain: () => () => undefined,
  },
  pipeRelay: (observer: {
    next: (event: StreamEvent) => void;
    error: (error: unknown) => void;
  }) => {
    harness.subscribe(observer.next);
    return { unsubscribe: () => undefined };
  },
}));

vi.mock("../bridge/client", () => ({
  sendRequest: vi.fn(async () => undefined),
}));

import { createWorkspace } from "./workspace-controller";
import type { BootstrapDto } from "../bridge/types";
import { sendRequest } from "../bridge/client";
import { WorkspacePanes } from "./WorkspacePanes";
import type { PackageSurfaceDto, PackageUiSnapshotDto } from "../bridge/types";

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
      // Empty tab: no seed content, no edit lease — the landing renders.
      head: { totalBytes: 0, firstChunk: "" },
      access: { none: {} },
      workspaceRoot: `/tmp/ws${over.clientId}`,
    },
    behaviorManifest: behaviorManifestFixture({ behaviorVersion: 2 }),
    activeTheme: {
      specifier: "",
      tokens: {},
      editorStyles: {},
      densityScale: 1,
    },
    activeTypography: {
      revision: 1,
      monospace: { families: ["m"], size: 13, lineScale: 1 },
      proportional: { families: ["p"], size: 14, lineScale: 1 },
    },
    activeDesignSystem: {
      specifier: "",
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
    serverKeymaps: [],
    ...(over as Partial<BootstrapDto>),
  } as BootstrapDto;
}

const agentSurface = {
  id: "coding-agent.surface",
  actionTargets: ["coding-agent.profile", "coding-agent.close"],
  provenance: {
    packageName: "@clay/coding-agent",
    packageVersion: "0.1.0",
    apiPrefix: "coding-agent",
    trustDomain: "trusted" as const,
  },
  component: {
    kind: "panel" as const,
    id: "coding-agent.root",
    title: "Coding Agent",
    children: [
      {
        kind: "label" as const,
        id: "coding-agent.emptyHint",
        text: "No conversation yet.",
      },
    ],
  },
};

const launcherSurface: PackageSurfaceDto = {
  id: "launcher.start",
  actionTargets: ["workspace.clientOpenFolderDialog"],
  provenance: {
    packageName: "@clay/launcher",
    packageVersion: "0.1.0",
    apiPrefix: "launcher",
    trustDomain: "trusted" as const,
  },
  component: {
    kind: "panel" as const,
    id: "launcher.root",
    title: "Start",
    children: [
      {
        kind: "button" as const,
        id: "launcher.openFolder",
        label: "Open folder…",
        action: { commandId: "workspace.clientOpenFolderDialog" },
      },
    ],
  },
};

async function mountedWorkspace(
  withSurface: boolean,
  emptyTab: PackageUiSnapshotDto["emptyTab"] = null,
  options: {
    persistLayout?: boolean;
    /** Spy for the tab's stamped sender (plan 119 SC-6 lane assertions). */
    send?: (payload: string) => Promise<void>;
  } = {},
) {
  const ws = createWorkspace({
    send: options.send ?? (async () => undefined),
    // A persisted layout restores the tab's picked folder (a launcher tab is
    // committed from then on); the launcher tests start with no layout at all.
    loadLayout: async () =>
      options.persistLayout === false
        ? null
        : {
            version: 2,
            tabs: [
              {
                workspaceRoot: "/tmp/ws1",
                panes: { "0": null },
                activePane: 1,
              },
            ],
            activeTab: 0,
          },
  });
  ws.installBootstrap(bootstrap({ clientId: 1, tabId: 10 }));
  ws.handleEnvelope({
    kind: "runtimeSnapshot",
    data: {
      clientId: 1,
      tabId: 10,
      snapshot: {
        runtimeGenerationId: 2,
        behaviorManifest: behaviorManifestFixture({ behaviorVersion: 2 }),
        activeTheme: {
          specifier: "",
          tokens: {},
          editorStyles: {},
          densityScale: 1,
        },
        activeTypography: bootstrap({ clientId: 1 }).activeTypography,
        activeDesignSystem: bootstrap({ clientId: 1 }).activeDesignSystem,
        // Live shape: the default tree is a single editorView node — the
        // editor slot (and the landing inside it) rides the SDUI tree.
        sduiTree: {
          uiVersion: 2,
          rootId: 1,
          nodes: [
            {
              id: 1,
              kind: {
                editorView: {
                  binding: { documentId: 1, expectedVersion: null },
                },
              },
            },
          ],
        },
        packageUi: {
          version: 2,
          emptyTab,
          surfaces: withSurface ? [agentSurface] : [],
          panels: [],
          overlays: [],
          components: [],
          inputRoutes: [],
        },
        uiChoices: { themes: [], designSystems: [] },
        documents: [],
        diagnostics: [],
      },
    },
  });
  await ws.restore();
  return ws;
}

describe("WorkspacePanes two views", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders the agent surface in the tab's agent view", async () => {
    const ws = await mountedWorkspace(true);
    render(<WorkspacePanes workspace={ws} />);
    // The agent half is inert tab data (attached from the launcher); the view
    // switch is tab chrome (Ctrl+2 / the titlebar segmented control).
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    // The agent view is a fixed composition (transcript column + inspector
    // column), not a resizable split.
    // The agent panel is a lazy chunk; under full-suite parallelism the first
    // mount can exceed the default 1 s findBy timeout.
    expect(
      await screen.findByRole(
        "region",
        { name: "Coding Agent" },
        { timeout: 5000 },
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Agent inspector")).toBeInTheDocument();
  });

  it("a panel-created store's commands ride the tab's own sender (plan 119 SC-6)", async () => {
    // AgentView hands the panel the tab's stamped sender; the store the panel
    // creates on first mount must use it (not the process-wide active client,
    // which is whichever tab was activated last).
    const send = vi.fn(async (payload: string) => {
      void payload;
      return undefined;
    });
    const ws = await mountedWorkspace(true, null, { send });
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    await waitFor(() =>
      expect(
        vi
          .mocked(send)
          .mock.calls.map(([payload]) => String(payload))
          .some((payload) => payload.includes("listSessions")),
      ).toBe(true),
    );
    expect(sendRequest).not.toHaveBeenCalled();
  });

  it("keeps a tab's agent store on the relay after its view unmounts (plan 119 SC-6)", async () => {
    const ws = await mountedWorkspace(true);
    const view = render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    const runtime = ws.active();
    if (!runtime?.agent) throw new Error("no active tab store");
    const store = runtime.agent;
    // The surface goes away (another tab is up) but the tab keeps its store:
    // a run that outlives the view still streams into this transcript.
    view.unmount();
    act(() => {
      harness.emit({
        type: "CUSTOM",
        name: "clay.agentRpc",
        value: {
          code: "session.bound",
          result: {
            clientId: runtime.clientId,
            tabId: runtime.tabId,
            sessionId: "sess-hidden",
          },
        },
        clientId: runtime.clientId,
      });
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(store.sessionId()).toBe("sess-hidden");
  });

  it("opens a session file in the workspace view from the Files tab (plan 118 task 36)", async () => {
    const ws = await mountedWorkspace(true);
    const openPath = vi.spyOn(ws, "openPath");
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });

    // The session's own file records (a tool row the server derived from the
    // call's arguments) are what the Files tab lists.
    // The tab runtime owns the store (plan 119 SC-6): no process global.
    const runtime = ws.active();
    if (!runtime?.agent) throw new Error("no active tab store");
    const store = runtime.agent;
    const release = store.start();
    try {
      // The seed type is the panel's own row shape; the file record is what
      // the server puts on the message's metadata.
      store.seedForDev({
        messages: [
          {
            id: "clay-tool-t1",
            role: "tool",
            content: 'edit {"path":"notes/plan.md"}',
            metadata: {
              clayKind: "tool",
              toolName: "edit",
              sessionFile: { path: "notes/plan.md", op: "edit" },
            },
          },
        ] as never,
      });
      fireEvent.click(screen.getByRole("tab", { name: "Files" }));
      const row = await screen.findByRole("button", {
        name: "Open notes/plan.md in the workspace view",
      });
      fireEvent.click(row);

      // The document opens through the same path every other open uses, and
      // the tab switches to the view that shows it — the agent half stays
      // mounted so its transcript is untouched.
      expect(openPath).toHaveBeenCalledWith("notes/plan.md");
      const workspaceSlot = document.querySelector(
        '[data-view="workspace"]',
      ) as HTMLElement;
      const agentSlot = document.querySelector(
        '[data-view="agent"]',
      ) as HTMLElement;
      expect(workspaceSlot.hidden).toBe(false);
      expect(agentSlot.hidden).toBe(true);
      expect(agentSlot).not.toBeNull();
    } finally {
      release();
    }
  });

  it("keeps both views mounted and hides the inactive one", async () => {
    const ws = await mountedWorkspace(true);
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    // The workspace view is hidden, not unmounted: switching back must not
    // re-fetch the tree or lose editor state (plan 118 task 33, performance).
    const workspaceSlot = document.querySelector(
      '[data-view="workspace"]',
    ) as HTMLElement;
    const agentSlot = document.querySelector(
      '[data-view="agent"]',
    ) as HTMLElement;
    expect(workspaceSlot.hidden).toBe(true);
    expect(agentSlot.hidden).toBe(false);
    await act(async () => {
      ws.setView("workspace");
    });
    expect(workspaceSlot.hidden).toBe(false);
    expect(agentSlot.hidden).toBe(true);
    expect(document.querySelector('[data-view="agent"]')).toBe(agentSlot);
  });

  it("shows the agent view's own prompt when no agent surface is installed", async () => {
    const ws = await mountedWorkspace(false);
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.setView("agent");
    });
    expect(await screen.findByText("No agent in this tab")).toBeInTheDocument();
  });

  it("keeps the core Open File / Open Folder fallback with no empty-tab contribution", async () => {
    const ws = await mountedWorkspace(false, null, { persistLayout: false });
    render(<WorkspacePanes workspace={ws} />);
    expect(
      screen.getByRole("group", { name: "Empty tab" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Open file" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Open folder" }),
    ).toBeInTheDocument();
    // No product-named landing lives in core (plan 118 Part D).
    expect(screen.queryByRole("button", { name: "Coding Agent" })).toBeNull();
    expect(screen.queryByRole("group", { name: "Start" })).toBeNull();
  });

  it("does not show the launcher inside a committed tab", async () => {
    // The launcher is what a tab *opens on*: once the tab has a folder (here
    // restored from layout.json) its empty pane is the plain open prompt, not
    // a second, stale picker (plan 118 task 33's gating rule).
    const ws = await mountedWorkspace(false, launcherSurface);
    render(<WorkspacePanes workspace={ws} />);
    expect(screen.queryByRole("group", { name: "Start" })).toBeNull();
    expect(
      screen.getByRole("group", { name: "Empty tab" }),
    ).toBeInTheDocument();
  });

  it("renders the launcher panel for the trusted launcher contribution", async () => {
    const ws = await mountedWorkspace(false, launcherSurface, {
      persistLayout: false,
    });
    render(<WorkspacePanes workspace={ws} />);
    // The panel is the host's trusted renderer; with no server listing yet it
    // shows the specified first-run state rather than fabricated rows.
    expect(
      await screen.findByRole("group", { name: "Start" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/No workspace has been opened yet/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Agents are folders under ~\/\.clay\/agents\//),
    ).toBeInTheDocument();
  });

  it("renders a third-party empty-tab contribution through generic SDUI", async () => {
    const ws = await mountedWorkspace(
      false,
      {
        ...launcherSurface,
        provenance: {
          ...launcherSurface.provenance,
          packageName: "@vendor/start",
          trustDomain: "thirdParty",
        },
      },
      { persistLayout: false },
    );
    render(<WorkspacePanes workspace={ws} />);
    expect(screen.queryByRole("group", { name: "Start" })).toBeNull();
    // The declared tree renders instead: a panel titled Start with one button.
    expect(
      await screen.findByRole("button", { name: "Open folder…" }),
    ).toBeInTheDocument();
  });
});
