// @vitest-environment jsdom
// Live-path regression test (plan 108 task 8 defect): launching the Coding
// Agent must actually render the split surface. This exercises the exact
// production chain — runtimeSnapshot envelope → packageUi.surfaces →
// launchCodingAgent → PaneContent agent branch — that previously broke when
// a wire layer dropped `surfaces` and no test noticed.

import { describe, expect, it, vi, afterEach } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

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
import { resetChatAgentForTests } from "../agent/state";

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

async function mountedWorkspace(withSurface: boolean) {
  const ws = createWorkspace({
    send: async () => undefined,
    loadLayout: async () => ({
      version: 2,
      tabs: [
        { workspaceRoot: "/tmp/ws1", panes: { "0": null }, activePane: 1 },
      ],
      activeTab: 0,
    }),
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
          emptyTab: null,
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

describe("WorkspacePanes coding agent launch", () => {
  afterEach(() => {
    cleanup();
    resetChatAgentForTests();
  });

  it("launches the Coding Agent surface from the empty-tab landing", async () => {
    const ws = await mountedWorkspace(true);
    render(<WorkspacePanes workspace={ws} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Coding Agent" }));
    expect(
      await screen.findByLabelText("Coding Agent split"),
    ).toBeInTheDocument();
  });

  it("keeps the landing when the surface never reached the snapshot", async () => {
    const ws = await mountedWorkspace(false);
    render(<WorkspacePanes workspace={ws} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Coding Agent" }));
    expect(
      screen.queryByLabelText("Coding Agent split"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("group", { name: "Empty tab" }),
    ).toBeInTheDocument();
  });
});
