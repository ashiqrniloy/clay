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
    protocolVersion: 28,
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
    // Escape (React Aria) and the close button both land in menuCancel,
    // which clears the menu locally without waiting for the server.
    expect(workspace.active()?.menu).toBeNull();
  });

  it("closes from the modal close button", async () => {
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
            items: [],
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
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(workspace.active()?.menu).toBeNull();
    const families = sent.map((payload) => JSON.parse(payload).family);
    expect(families).toContain("menuCancel");
  });

  it("keeps a typed API key when the snapshot query is masked and flushes it on Enter",
    async () => {
      const sent: string[] = [];
      const workspace = createWorkspace({
        send: async (payload) => {
          sent.push(payload);
        },
      });
      workspace.installBootstrap(bootstrap());
      const snapshot = {
        sessionId: "9223372036854775809" as never,
        prompt: "API key (hidden)",
        query: "",
        items: [
          {
            id: "store_secret",
            label: "Store API key",
            detail: "Value is hidden. Enter stores it.",
            accessibilityLabel: "Store API key",
          },
        ],
        selectedIndex: 0,
        status: "active" as const,
        focusPolicy: "modal" as const,
        origin: "centered" as const,
      };
      workspace.handleEnvelope({
        kind: "routed",
        data: {
          clientId: 1,
          tabId: 10,
          event: { kind: "transientMenuSnapshot", data: snapshot },
        },
      });
      const user = userEvent.setup();
      render(<CommandCentre workspace={workspace} />);
      const field = screen.getByRole("textbox", { name: "API key (hidden)" });
      await user.type(field, "sk-test-key");
      workspace.handleEnvelope({
        kind: "routed",
        data: {
          clientId: 1,
          tabId: 10,
          event: {
            kind: "transientMenuSnapshot",
            data: { ...snapshot, query: "•".repeat(11) },
          },
        },
      });
      expect(field).toHaveValue("sk-test-key");
      await user.keyboard("{Enter}");
      const queries = sent
        .map((payload) => JSON.parse(payload) as { family: string; payload: { query?: string } })
        .filter((payload) => payload.family === "menuQueryUpdate")
        .map((payload) => payload.payload.query);
      expect(queries.at(-1)).toBe("sk-test-key");
      expect(sent.map((payload) => JSON.parse(payload).family)).toContain(
        "menuActivate",
      );
    },
  );
});
