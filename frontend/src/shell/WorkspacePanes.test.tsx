// @vitest-environment jsdom
// Live-path regression tests for the tab's two views (plan 118 task 33):
// entering the agent view must actually render the agent surface (the chain
// runtimeSnapshot envelope → packageUi.surfaces → view switch → AgentView),
// the inactive view must stay mounted, and the launcher must be the landing of
// an *uncommitted* tab only. Plan 124 added the tab's persistent agent lane
// below both views — the same instance in either view, mounted once per tab,
// reading the one store the host resolves for the tab.

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
import { agentLane } from "./layout-state";
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

describe("the tab's agent lane (plan 124)", () => {
  it("mounts one lane for both views and never remounts it on a switch", async () => {
    const ws = await mountedWorkspace(true);
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    const lane = screen.getByRole("contentinfo", { name: "Agent lane" });
    // Agent view up: the lane is still there (the composer belongs to the tab).
    expect(lane.contains(screen.getByLabelText("Message"))).toBe(true);
    await act(async () => {
      ws.setView("workspace");
    });
    // Same DOM node, not a second lane: switching views moves neither the
    // draft nor the pickers (performance AC).
    expect(screen.getByRole("contentinfo", { name: "Agent lane" })).toBe(lane);
  });

  it("hides the lane when the tab's lane state is off and keeps its draft", async () => {
    const ws = await mountedWorkspace(true);
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    const lane = document.querySelector(
      "footer[aria-label='Agent lane']",
    ) as HTMLElement;
    const field = screen.getByLabelText("Message") as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "half a thought" } });

    // `Ctrl+X Ctrl+P` flips this state (the chord is the shell matcher's —
    // plan 124 task 5); hiding is a state, so the draft survives.
    await act(async () => {
      agentLane.setVisible(false);
    });
    // Hidden is out of the accessibility tree as well as out of the flow.
    expect(
      screen.queryByRole("contentinfo", { name: "Agent lane" }),
    ).toBeNull();
    expect(lane.hidden).toBe(true);
    await act(async () => {
      agentLane.setVisible(true);
    });
    expect(lane.hidden).toBe(false);
    expect(
      (screen.getByLabelText("Message") as HTMLTextAreaElement).value,
    ).toBe("half a thought");
  });

  it("draws the palette in the lane and veils the working area only", async () => {
    // Plan 124/DESIGN §6: the composer's menus use the modal scrim recipe, but
    // over the working area — the lane keeps the field that *is* the palette's
    // query above the veil, and the sheet rises from inside it.
    const sent: string[] = [];
    const ws = await mountedWorkspace(true, undefined, {
      send: async (payload: string) => {
        sent.push(payload);
      },
    });
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    const host = screen.getByTestId("workspace-panes");
    const veil = host.querySelector("[data-clay-slot='scrim']") as HTMLElement;
    expect(veil.dataset.open).toBe("false");
    // The field opens the session (the shell sends `controlCenter.open`); the
    // snapshot that follows is what draws the sheet.
    const field = screen.getByLabelText("Message") as HTMLTextAreaElement;
    fireEvent.change(field, { target: { value: "/" } });
    await act(async () => {
      ws.handleEnvelope({
        kind: "routed",
        data: {
          clientId: 1,
          tabId: 1,
          event: {
            kind: "transientMenuSnapshot",
            data: {
              sessionId: "9" as never,
              prompt: "Commands",
              query: "",
              items: [
                {
                  id: "shell.toggleAgentLane",
                  label: "Toggle Agent Lane",
                  detail: "client — built-in",
                  scope: "shell",
                  bindings: ["Ctrl+X Ctrl+P"],
                  accessibilityLabel: "Toggle Agent Lane built-in",
                },
              ],
              selectedIndex: 0,
              status: "active",
              focusPolicy: "modal",
              origin: "commandPalette",
            },
          },
        },
      } as never);
    });
    const sheet = await screen.findByTestId("command-palette");
    // The sheet is inside the lane's composer form (its anchor), so it spans
    // the composer box it answers to.
    expect(
      screen.getByRole("contentinfo", { name: "Agent lane" }).contains(sheet),
    ).toBe(true);
    expect(field.closest("form")?.contains(sheet)).toBe(true);
    // One veil, over the view area — never over the lane.
    const veiled = host.querySelector(
      "[data-clay-slot='scrim']",
    ) as HTMLElement;
    expect(veiled).toBe(veil);
    expect(veiled.dataset.open).toBe("true");
    expect(
      veiled.closest("footer[aria-label='Agent lane']"),
      "the lane stays above the veil: its field is the palette's query",
    ).toBeNull();
    // It is the modal scrim *recipe* (the attribute is what carries the
    // reduced-transparency / no-backdrop-filter fallbacks in global.css) on a
    // host element, not a modal dialog.
    expect(veiled.getAttribute("data-clay-component")).toBe("modal");
    expect(veiled.getAttribute("data-clay-slot")).toBe("scrim");
    expect(veiled.getAttribute("aria-hidden")).toBe("true");
    // Plan 124: the sheet's scope chip reaches the session as a filter — the
    // host is the one wiring between the field's menu and the server, so the
    // payload (not just the component's intent) is what this asserts.
    const updates = sent
      .map((payload) => JSON.parse(payload))
      .filter((message) => message.family === "menuQueryUpdate");
    expect(updates.at(-1)?.payload).toMatchObject({ query: "", scope: null });
    // Plan 124 launch test: hiding the lane takes its menus with it, so the
    // veil closes instead of stranding a scrim over the working area with the
    // palette's query input gone. The session survives; showing the lane
    // again restores the sheet and the veil.
    await act(async () => {
      agentLane.setVisible(false);
    });
    expect(veiled.dataset.open).toBe("false");
    expect(
      (document.querySelector("footer[aria-label='Agent lane']") as HTMLElement)
        .hidden,
    ).toBe(true);
    await act(async () => {
      agentLane.setVisible(true);
    });
    expect(veiled.dataset.open).toBe("true");
    expect(await screen.findByTestId("command-palette")).toBe(sheet);
    fireEvent.click(screen.getByRole("button", { name: "Shell" }));
    await act(async () => undefined);
    const chipUpdate = sent
      .map((payload) => JSON.parse(payload))
      .filter((message) => message.family === "menuQueryUpdate")
      .at(-1);
    expect(chipUpdate?.payload).toMatchObject({ query: "", scope: "shell" });
  });

  it("boots the tab's store itself and shares it with the agent view", async () => {
    // The lane mounts for tabs whose agent view was never shown, so the host —
    // not the view — resolves and bootstraps the store (plan 124). The old
    // "panel-created store" assertion became the host's.
    const send = vi.fn(async (payload: string) => {
      void payload;
      return undefined;
    });
    const ws = await mountedWorkspace(true, null, { send });
    render(<WorkspacePanes workspace={ws} />);
    const runtime = ws.active();
    if (!runtime) throw new Error("no active tab");
    await waitFor(() => {
      expect(runtime.agent).not.toBeNull();
    });
    const store = runtime.agent;
    const asks = send.mock.calls.map(([payload]) => String(payload));
    expect(asks.some((payload) => payload.includes("listSessions"))).toBe(true);
    expect(asks.some((payload) => payload.includes("tabState"))).toBe(true);
    // The agent view was never mounted; the lane exists anyway, and the view
    // reads the same store once it is.
    expect(
      screen.getByRole("contentinfo", { name: "Agent lane" }),
    ).toBeInTheDocument();
    await act(async () => {
      ws.setView("agent");
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    expect(runtime.agent).toBe(store);
    expect(sendRequest).not.toHaveBeenCalled();
  });

  it("shows the agent-less state from the approved artifact", async () => {
    const ws = await mountedWorkspace(true);
    render(<WorkspacePanes workspace={ws} />);
    const field = await screen.findByLabelText("Message");
    expect(field).toBeDisabled();
    expect(field).toHaveProperty(
      "placeholder",
      "Attach an agent to this tab to send a prompt",
    );
    const trigger = document.querySelector("[data-agent-pick] button");
    expect(trigger).toHaveTextContent("Attach an agent");
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
  });

  it("marks the window while this tab works, and only while it works", async () => {
    const ws = await mountedWorkspace(true);
    const view = render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    const runtime = ws.active();
    if (!runtime) throw new Error("no active tab");
    await waitFor(() => expect(runtime.agent).not.toBeNull());
    // The lane reports the tab's run state: the tab record carries it (the
    // titlebar's mark reads it — the tab strip's own marker is colour only).
    await act(async () => {
      harness.emit({
        type: "RUN_STARTED",
        threadId: "s",
        runId: "r",
        clientId: runtime.clientId,
      });
    });
    await waitFor(() => {
      const tab = ws
        .getSnapshot()
        .tabs.find((entry) => entry.clientId === runtime.clientId);
      expect(tab?.agentBusy).toBe(true);
    });
    await act(async () => {
      harness.emit({
        type: "RUN_FINISHED",
        threadId: "s",
        runId: "r",
        clientId: runtime.clientId,
      });
    });
    await waitFor(() => {
      const tab = ws
        .getSnapshot()
        .tabs.find((entry) => entry.clientId === runtime.clientId);
      expect(tab?.agentBusy).toBe(false);
    });
    view.unmount();
  });

  it("keeps one store per tab across a tab switch", async () => {
    const ws = await mountedWorkspace(true);
    render(<WorkspacePanes workspace={ws} />);
    await act(async () => {
      ws.attachAgent({ type: "coding-agent", configRoot: "/tmp/agents/ca" });
    });
    await screen.findByRole("region", { name: "Coding Agent" });
    const first = ws.active();
    if (!first) throw new Error("no active tab");
    await waitFor(() => expect(first.agent).not.toBeNull());

    // A second tab gets its own store; the first tab's survives the switch.
    await act(async () => {
      ws.installBootstrap(bootstrap({ clientId: 2, tabId: 20 }));
      await ws.activate(2);
    });
    await waitFor(() => {
      expect(ws.active()?.clientId).toBe(2);
    });
    const second = ws.active();
    if (!second) throw new Error("no second tab");
    await waitFor(() => expect(second.agent).not.toBeNull());
    expect(second.agent).not.toBe(first.agent);
    await act(async () => {
      await ws.activate(1);
    });
    await waitFor(() => expect(ws.active()?.clientId).toBe(1));
    expect(ws.active()?.agent).toBe(first.agent);
  });
});
