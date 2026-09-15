import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { RouterProvider } from "react-router";

import { workspace } from "../shell/workspace-singleton";

import { createAppRouter } from "../app/router";
import type { BootstrapDto } from "../bridge/types";
import type { ConnectionState } from "../state/connection-store";
import { behaviorManifestFixture } from "../test/contract-fixtures";

const readyBootstrap: BootstrapDto = {
  clientId: 1,
  tabId: 10,
  performanceProfile: false,
  protocolVersion: 28,
  endpoint: "/tmp/x.sock",
  generation: 1,
  initialDocument: {
    documentId: 1 as never,
    version: 1,
    head: { totalBytes: 0, firstChunk: "" },
    access: "readOnly",
    workspaceRoot: "/tmp/ws",
  },
  behaviorManifest: behaviorManifestFixture({ behaviorVersion: 1 }),
  activeTheme: {
    specifier: "",
    tokens: {},
    editorStyles: {},
    densityScale: 1,
  },
  activeTypography: {
    revision: 1,
    monospace: {
      families: ["m"],
      size: 13,
      ligatures: {
        enableStandard: true,
        enableContextual: true,
        discretionaryFeatures: [],
        rawFeatures: null,
        disableFeatures: [],
      },
    },
    proportional: {
      families: ["p"],
      size: 13,
      ligatures: {
        enableStandard: true,
        enableContextual: true,
        discretionaryFeatures: [],
        rawFeatures: null,
        disableFeatures: [],
      },
    },
    ui: {
      families: ["u"],
      size: 13,
      ligatures: {
        enableStandard: true,
        enableContextual: true,
        discretionaryFeatures: [],
        rawFeatures: null,
        disableFeatures: [],
      },
    },
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
};

const ready: ConnectionState = {
  phase: "ready",
  bootstrap: readyBootstrap,
};

afterEach(cleanup);

function renderAt(path: string, connection = ready) {
  const router = createAppRouter({ connection, onReconnect: () => {} }, path);
  return render(<RouterProvider router={router} />);
}

describe("tab chrome: the two views (plan 118 task 33)", () => {
  it("disables the switcher until a half is picked, then switches and marks", () => {
    workspace.reset();
    workspace.installBootstrap(readyBootstrap);
    const { container } = renderAt("/workspace");
    const switcher = container.querySelector(
      "[data-viewswitch]",
    ) as HTMLElement;
    const [workspaceItem, agentItem] = [
      ...switcher.querySelectorAll<HTMLButtonElement>("button"),
    ];

    // Uncommitted tab: both items are inert with the approved reasons
    // (start.html's `data-viewswitch="empty"`).
    expect(switcher.dataset.viewswitch).toBe("empty");
    expect(workspaceItem).toBeDisabled();
    expect(agentItem).toBeDisabled();
    expect(workspaceItem).toHaveAttribute("title", "Pick a workspace first");

    // The launcher's agent row attaches the agent half; both views become
    // reachable and the strip shows the approved mono marker.
    act(() =>
      workspace.attachAgent({
        type: "coding-agent",
        configRoot: "/tmp/agents",
      }),
    );
    expect(switcher.dataset.viewswitch).toBe("agent");
    expect(workspaceItem).toBeEnabled();
    expect(agentItem).toBeEnabled();
    expect(agentItem).toHaveAttribute("aria-selected", "true");
    expect(agentItem).toHaveAttribute("title", "Agent view (Ctrl+2)");
    expect(screen.getByText("agent")).toBeInTheDocument();
    expect(
      screen.getByTitle(/one tab, two views, agent: coding-agent/),
    ).toBeInTheDocument();

    // Busy pulses the marker; the view switch is chrome, so the agent half
    // stays attached when the workspace view comes back.
    act(() => workspace.setAgentBusy(1, true));
    expect(screen.getByText("agent")).toHaveAttribute("data-busy", "true");
    act(() => workspace.setView("workspace"));
    expect(switcher.dataset.viewswitch).toBe("workspace");
    expect(workspaceItem).toHaveAttribute("aria-selected", "true");
    expect(agentItem).toHaveAttribute("aria-selected", "false");
    expect(workspace.tabs.get().tabs[0]?.agent?.type).toBe("coding-agent");
    workspace.reset();
  });
});

describe("app shell landmarks", () => {
  it("renders exactly one main landmark with header/footer chrome", () => {
    renderAt("/workspace");
    expect(screen.getAllByRole("main")).toHaveLength(1);
    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(screen.getByRole("contentinfo")).toBeInTheDocument();
    expect(
      screen.getByRole("tablist", { name: "Window tabs" }),
    ).toBeInTheDocument();
  });

  it("shows the live session status in the footer", () => {
    renderAt("/workspace");
    const status = screen.getByTestId("shell-status").parentElement;
    expect(status).toHaveAttribute("role", "status");
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(status).toHaveTextContent("Connected");
  });

  it("carries mono status data and the keyboard hint row", () => {
    renderAt("/workspace");
    // Left: where the window is; middle: the connection; right: the real
    // commands the hints run (titlebar and status bar share one toggle).
    expect(screen.getByTestId("shell-location")).toBeInTheDocument();
    const bar = screen.getByTestId("shell-status").closest("footer");
    expect(bar).not.toBeNull();
    expect(bar?.getAttribute("data-clay-ds")).toBe("statusBar.root");
    const hints = [...(bar?.querySelectorAll("button") ?? [])].map((button) =>
      button.textContent?.trim(),
    );
    expect(hints.some((label) => label?.includes("palette"))).toBe(true);
    expect(hints.some((label) => label?.includes("files"))).toBe(true);
    expect(hints.some((label) => label?.includes("outline"))).toBe(true);
    // Every hint shows its keys as kbd chips, not as prose.
    for (const button of bar?.querySelectorAll("button") ?? []) {
      expect(button.querySelectorAll("kbd").length).toBeGreaterThan(0);
    }
  });

  it("renders deterministic fixture states in development builds only", async () => {
    const originalDev = import.meta.env.DEV;
    (import.meta.env as { DEV: boolean }).DEV = true;
    try {
      renderAt("/fixture/states");
      expect(await screen.findAllByText("Loading state")).not.toHaveLength(0);
      expect(screen.getByRole("alert")).toBeInTheDocument();
    } finally {
      (import.meta.env as { DEV: boolean }).DEV = originalDev;
    }
  });
});

describe("narrow/wide working area", () => {
  it("keeps a single main region when no left slot is visible", () => {
    const { container } = renderAt("/workspace");
    // No fixed-slot split: the rail's own divider is a content separator, not
    // a pane handle.
    expect(
      container.querySelector('[data-clay-ds="paneSplitTree.handle"]'),
    ).toBeNull();
  });

  it("projects the left slot with a keyboard-operable separator when present", async () => {
    const { WorkingArea } = await import("../app/layout/working-area");
    render(
      <WorkingArea left={<div>browser</div>}>
        <div>editor</div>
      </WorkingArea>,
    );
    expect(screen.getByRole("separator")).toBeInTheDocument();
  });
});

describe("design-system shell and surface stability", () => {
  it("preserves shell layout, tab strip, and status across recipe variable updates", () => {
    const { rerender } = renderAt("/workspace");
    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(
      screen.getByRole("tablist", { name: "Window tabs" }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("shell-status")).toHaveTextContent("Connected");

    // Simulate recipe variable overrides on document root
    document.documentElement.style.setProperty(
      "--clay-ds-shell-default-root-rest-background-color",
      "var(--clay-surface-main)",
    );
    document.documentElement.style.setProperty(
      "--clay-ds-pane-default-divider-rest-width",
      "4px",
    );

    rerender(
      <RouterProvider
        router={createAppRouter(
          { connection: ready, onReconnect: () => {} },
          "/workspace",
        )}
      />,
    );

    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(
      screen.getByRole("tablist", { name: "Window tabs" }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("shell-status")).toHaveTextContent("Connected");

    document.documentElement.style.removeProperty(
      "--clay-ds-shell-default-root-rest-background-color",
    );
    document.documentElement.style.removeProperty(
      "--clay-ds-pane-default-divider-rest-width",
    );
  });
});
