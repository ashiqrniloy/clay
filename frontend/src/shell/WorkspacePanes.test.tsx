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
} from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => undefined),
}));

type StreamEvent = Record<string, unknown> & { type: string };
const harness = vi.hoisted(() => {
  const listeners = new Set<(event: StreamEvent) => void>();
  return {
    emit(_event: StreamEvent): void {
      for (const listener of listeners) listeners.add(listener);
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
import { WorkspacePanes } from "./WorkspacePanes";
import type { PackageSurface } from "../sdui/types";
import { resetAgentSessionForTests } from "../agent/state";

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
    behaviorManifest: {
      manifestId: "m",
      behaviorVersion: 2,
      commands: [],
      keymaps: [],
    },
    activeTheme: { specifier: "", tokens: {}, densityScale: 1 },
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

const launcherSurface: PackageSurface = {
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
  emptyTab: PackageSurface | null = null,
  options: { persistLayout?: boolean } = {},
) {
  const ws = createWorkspace({
    send: async () => undefined,
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
        behaviorManifest: {
          manifestId: "m",
          behaviorVersion: 2,
          commands: [],
          keymaps: [],
        },
        activeTheme: { specifier: "", tokens: {}, densityScale: 1 },
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
    resetAgentSessionForTests();
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
    expect(
      await screen.findByRole("region", { name: "Coding Agent" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Agent inspector")).toBeInTheDocument();
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
    const store = (await import("../agent/state")).agentSession;
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
