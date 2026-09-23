// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

afterEach(cleanup);

import { SduiRenderer } from "./renderer";
import { installSduiTree } from "./state";

it("renders the file-browser hide indicator as an icon button beside the title", () => {
  const send = vi.fn<(payload: string) => Promise<void>>(async () => undefined);
  const state = installSduiTree({
    uiVersion: 6,
    rootId: 1,
    nodes: [
      { id: 1, kind: { flex: { direction: "row", children: [2, 4] } } },
      { id: 2, kind: { stack: { children: [7, 5] } } },
      { id: 3, kind: { label: { text: "Workspace · clay", icon: null } } },
      {
        id: 6,
        kind: {
          button: {
            label: "Hide file browser",
            icon: "disclosure.right",
            action: {
              commandId: "workspace.toggleFileBrowser",
              source: { button: { nodeId: 6 } },
              arguments: [],
            },
          },
        },
      },
      {
        id: 7,
        kind: { flex: { direction: "row", children: [3, 6] } },
      },
      {
        id: 5,
        kind: {
          list: {
            items: [],
            filter: { placeholder: "Filter files", shortcut: "/" },
          },
        },
      },
      {
        id: 4,
        kind: {
          editorView: { binding: { documentId: 1, expectedVersion: 2 } },
        },
      },
    ],
  });
  render(
    <SduiRenderer
      state={state}
      send={send}
      editorSlot={<div data-testid="editor-slot">editor</div>}
    />,
  );
  const toggle = screen.getByRole("button", { name: "Hide file browser" });
  expect(toggle.querySelector("svg")).toHaveAttribute(
    "data-icon-name",
    "disclosure.right",
  );
  expect(toggle.textContent).not.toContain("Hide file browser");
  fireEvent.click(toggle);
  expect(JSON.parse(String(send.mock.calls[0]?.[0]))).toMatchObject({
    family: "sduiAction",
    payload: {
      uiVersion: 6,
      intent: { commandId: "workspace.toggleFileBrowser" },
    },
  });
});

it("renders the hidden file-browser state as a show indicator", () => {
  const send = vi.fn<(payload: string) => Promise<void>>(async () => undefined);
  const state = installSduiTree({
    uiVersion: 6,
    rootId: 1,
    nodes: [
      { id: 1, kind: { flex: { direction: "row", children: [8, 2] } } },
      {
        id: 8,
        kind: {
          button: {
            label: "Show file browser",
            icon: "disclosure.right",
            action: {
              commandId: "workspace.toggleFileBrowser",
              source: { button: { nodeId: 8 } },
              arguments: [],
            },
          },
        },
      },
      {
        id: 2,
        kind: {
          editorView: { binding: { documentId: 1, expectedVersion: 2 } },
        },
      },
    ],
  });
  render(
    <SduiRenderer state={state} send={send} editorSlot={<div>editor</div>} />,
  );
  expect(
    screen.getByRole("button", { name: "Show file browser" }),
  ).toBeVisible();
});

it("renders a bounded SDUI tree with editor slot and typed actions", () => {
  const send = vi.fn<(payload: string) => Promise<void>>(async () => undefined);
  const state = installSduiTree({
    uiVersion: 6,
    rootId: 1,
    nodes: [
      { id: 1, kind: { flex: { direction: "row", children: [2, 4] } } },
      { id: 2, kind: { panel: { title: "Workspace", children: [3] } } },
      {
        id: 3,
        kind: {
          button: {
            label: "Refresh",
            icon: null,
            action: {
              commandId: "workspace.refresh",
              source: { button: { nodeId: 3 } },
              arguments: [],
            },
          },
        },
      },
      {
        id: 4,
        kind: {
          editorView: { binding: { documentId: 1, expectedVersion: 2 } },
        },
      },
    ],
  });
  render(
    <SduiRenderer
      state={state}
      send={send}
      editorSlot={<div data-testid="editor-slot">editor</div>}
    />,
  );
  expect(
    screen.getByRole("complementary", { name: "Workspace" }),
  ).toBeVisible();
  expect(screen.getByTestId("editor-slot")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
  expect(JSON.parse(String(send.mock.calls[0]?.[0]))).toMatchObject({
    family: "sduiAction",
    payload: { uiVersion: 6, intent: { commandId: "workspace.refresh" } },
  });
});

it("projects semantic icon references on labels, buttons, and list rows (plan 112 T9)", () => {
  const send = vi.fn<(payload: string) => Promise<void>>(async () => undefined);
  const state = installSduiTree({
    uiVersion: 6,
    rootId: 1,
    nodes: [
      { id: 1, kind: { flex: { direction: "column", children: [2, 4, 5] } } },
      {
        id: 2,
        kind: {
          list: {
            items: [
              {
                id: "src",
                label: "src/",
                detail: "3 items",
                icon: "file.folder",
                action: {
                  commandId: "workspace.openDirectory",
                  source: { listItem: { nodeId: 2, itemId: "src" } },
                  arguments: [],
                },
              },
            ],
          },
        },
      },
      { id: 4, kind: { label: { text: "Branch: main", icon: "git.branch" } } },
      { id: 5, kind: { label: { text: "No icon here", icon: null } } },
    ],
  });
  const view = render(
    <SduiRenderer state={state} send={send} editorSlot={<div>editor</div>} />,
  );
  expect(
    view.container.querySelector('svg[data-icon-name="file.folder"]'),
  ).not.toBeNull();
  expect(
    view.container.querySelector('svg[data-icon-name="git.branch"]'),
  ).not.toBeNull();
  // Labels without icon keep rendering plain text; no icon, no announcement.
  expect(screen.getByText("No icon here")).toBeVisible();
  const row = screen.getByRole("option", { name: /src\// });
  expect(row.querySelector("svg")).toHaveAttribute(
    "data-icon-name",
    "file.folder",
  );
});
