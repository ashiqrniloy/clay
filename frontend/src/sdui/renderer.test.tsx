// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";

import { SduiRenderer } from "./renderer";
import { installSduiTree } from "./state";

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
      { id: 5, kind: { label: { text: "No icon here" } } },
    ],
  });
  const view = render(
    <SduiRenderer
      state={state}
      send={send}
      editorSlot={<div>editor</div>}
    />,
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
