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
