import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import type { BootstrapDto } from "../bridge/types";
import { createWorkspace } from "../shell/workspace-controller";
import { CommandCentre } from "./CommandCentre";
import { behaviorManifestFixture } from "../test/contract-fixtures";

afterEach(cleanup);

function bootstrap(): BootstrapDto {
  return {
    clientId: 1,
    tabId: 10,
    performanceProfile: false,
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

function mountMenu(snapshot: {
  prompt: string;
  query?: string;
  items?: { id: string; label: string; detail: string | null }[];
  status?: "active" | { empty: { message: string } };
  origin?: "centered" | "commandPalette" | "contextMenu" | "menuBar";
}) {
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
          query: snapshot.query ?? "",
          selectedIndex: 0,
          status: snapshot.status ?? "active",
          focusPolicy: "modal",
          origin: snapshot.origin ?? "centered",
          prompt: snapshot.prompt,
          items: (snapshot.items ?? []).map((item) => ({
            ...item,
            accessibilityLabel: item.label,
          })),
        },
      },
    },
  });
  render(<CommandCentre workspace={workspace} />);
  return { workspace, sent };
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
    const search = screen.getByRole("textbox", { name: "Command Centre" });
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

  it("keeps a typed API key when the snapshot query is masked and flushes it on Enter", async () => {
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
      .map(
        (payload) =>
          JSON.parse(payload) as {
            family: string;
            payload: { query?: string };
          },
      )
      .filter((payload) => payload.family === "menuQueryUpdate")
      .map((payload) => payload.payload.query);
    expect(queries.at(-1)).toBe("sk-test-key");
    expect(sent.map((payload) => JSON.parse(payload).family)).toContain(
      "menuActivate",
    );
  });

  it("composes the centred sheet: head, results, foot hints, live count", () => {
    mountMenu({
      prompt: "Command Centre",
      items: [
        { id: "git.refresh", label: "Refresh Git status", detail: "@clay/git" },
        {
          id: "settings.open",
          label: "Open settings",
          detail: "@clay/settings",
        },
      ],
    });
    const sheet = screen.getByTestId("command-centre");
    // One sheet: the centred surface, never a second frame inside it. (The
    // composer's `/` palette is the lane's own sheet — plan 124 — and is not
    // drawn here; `WorkspacePanes` filters that origin out before mounting.)
    expect(sheet.className).toContain("surface");
    expect(sheet.className).not.toContain("menu");
    // The prompt is the head's one visible label and the field's name (the
    // sheet paints no second title row).
    expect(screen.getByText("Command Centre")).toBeVisible();
    expect(
      screen.getByRole("textbox", { name: "Command Centre" }),
    ).toBeVisible();
    expect(
      screen.getByRole("dialog", { name: "Command Centre" }),
    ).toBeVisible();
    // Rows carry the server's detail (which is where a chord is stated).
    expect(screen.getByText("@clay/git")).toBeVisible();
    // The foot states the keys and the live count.
    expect(screen.getByText("navigate")).toBeVisible();
    expect(screen.getByText("run")).toBeVisible();
    expect(screen.getByText("close")).toBeVisible();
    expect(screen.getByText("2 results")).toBeVisible();
  });

  it("renders an empty result set inside the sheet, not as a nested card", () => {
    mountMenu({
      prompt: "Command Centre",
      status: { empty: { message: "No commands match this query" } },
    });
    const empty = screen.getAllByRole("status")[0] as HTMLElement;
    expect(empty).toHaveTextContent("No commands match this query");
    expect(empty).toHaveTextContent("Esc");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("uses the popover surface and the prompt label for menu sessions", () => {
    mountMenu({
      prompt: "Session actions",
      origin: "contextMenu",
      items: [{ id: "fork", label: "Fork Session", detail: null }],
    });
    const sheet = screen.getByTestId("command-centre");
    expect(sheet.className).toContain("menu");
    // The prompt is a visible micro-label here and the field's name.
    expect(screen.getByText("Session actions")).toBeVisible();
    expect(
      screen.getByRole("textbox", { name: "Session actions" }),
    ).toBeVisible();
  });

  it("fabricates no scope controls the server does not send", () => {
    mountMenu({
      prompt: "Command Centre",
      items: [{ id: "git.refresh", label: "Refresh Git status", detail: null }],
    });
    // The prototype's all/Session/Skills/MCP scopes have no server data, so no
    // surface fabricates them (the item-field addition that would carry a scope
    // group is its own task).
    expect(screen.queryByRole("radiogroup")).toBeNull();
    expect(screen.queryByRole("tablist")).toBeNull();
  });
});
