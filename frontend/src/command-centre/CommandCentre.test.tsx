import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import type { BootstrapDto } from "../bridge/types";
import { createWorkspace } from "../shell/workspace-controller";
import { CommandCentre } from "./CommandCentre";

afterEach(cleanup);

function bootstrap(): BootstrapDto {
  return {
    clientId: 1,
    tabId: 10,
    protocolVersion: 27,
    endpoint: "test",
    generation: 1,
    initialDocument: {
      documentId: 1 as never,
      version: 1,
      head: { totalBytes: 0, firstChunk: "" },
      access: { editable: { leaseId: 1 } },
      workspaceRoot: "/tmp/ws",
    },
    behaviorManifest: {
      manifestId: "test",
      behaviorVersion: 1,
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
        title: 1.16,
        section: 1.08,
        body: 1,
        status: 1,
        detail: 0.83,
        caption: 0.75,
      },
    },
  };
}

describe("CommandCentre", () => {
  it("renders one modal list and forwards query, movement, activation, and cancel", async () => {
    const sent: string[] = [];
    const workspace = createWorkspace({
      send: async (payload) => {
        sent.push(payload);
      },
    });
    workspace.installBootstrap(bootstrap());
    workspace.handleEnvelope({
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
            items: [
              {
                id: "runtime.reloadConfiguration",
                label: "Reload Configuration",
                detail: "Clay",
                accessibilityLabel: "Reload Configuration",
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
    const user = userEvent.setup();
    render(<CommandCentre workspace={workspace} />);
    expect(
      screen.getByRole("dialog", { name: "Command Centre" }),
    ).toBeVisible();
    expect(
      screen.getByRole("listbox", { name: "Command Centre results" }),
    ).toBeVisible();
    const search = screen.getByRole("textbox", { name: "Search" });
    await user.type(search, "r");
    await user.keyboard("{ArrowDown}{Enter}");
    await user.keyboard("{Escape}");
    const families = sent.map((payload) => JSON.parse(payload).family);
    expect(families).toContain("menuQueryUpdate");
    expect(families).toContain("menuSelectionMove");
    expect(families).toContain("menuActivate");
    expect(families).toContain("menuCancel");
  });
});
