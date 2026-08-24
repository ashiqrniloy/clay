import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { RouterProvider } from "react-router";

import { createAppRouter } from "../app/router";
import type { ConnectionState } from "../state/connection-store";

const ready: ConnectionState = {
  phase: "ready",
  bootstrap: {
    clientId: 1,
    protocolVersion: 26,
    endpoint: "/tmp/x.sock",
    generation: 1,
    initialDocument: {
      documentId: 1 as never,
      version: 1,
      text: "",
      access: {},
      workspaceRoot: "/tmp/ws",
    },
    behaviorManifest: {
      manifestId: "m",
      behaviorVersion: 1,
      commands: [],
      keymaps: [],
    },
    activeTheme: {
      specifier: "",
      tokens: {},
      densityScale: 1,
    },
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
  },
};

afterEach(cleanup);

function renderAt(path: string, connection = ready) {
  const router = createAppRouter({ connection, onReconnect: () => {} }, path);
  return render(<RouterProvider router={router} />);
}

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
    renderAt("/workspace");
    // No left slot content in Phase 4 shell: no separator rendered.
    expect(screen.queryByRole("separator")).not.toBeInTheDocument();
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
