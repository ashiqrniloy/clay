// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { SduiRenderer } from "../sdui/renderer";
import { installSduiTree } from "../sdui/state";

afterEach(cleanupImport());

function cleanupImport() {
  return () => document.body.replaceChildren();
}

describe("SDUI region sizing (plan 118 E1)", () => {
  it("marks a token-sized region and passes a list filter to the catalog list", () => {
    const state = installSduiTree({
      uiVersion: 3,
      rootId: 1,
      nodes: [
        { id: 1, kind: { flex: { direction: "row", children: [2, 4] } } },
        {
          id: 2,
          kind: { stack: { children: [3] } },
          size: "dimension.sidebar.default",
        },
        {
          id: 3,
          kind: {
            list: {
              items: [
                { id: "a.md", label: "a.md", detail: "src", action: null },
              ],
              filter: { placeholder: "Filter files", shortcut: "/" },
            },
          },
        },
        {
          id: 4,
          kind: {
            editorView: { binding: { documentId: 1, expectedVersion: 1 } },
          },
        },
      ],
    });
    const { container } = render(
      <SduiRenderer state={state} send={async () => {}} editorSlot={<div />} />,
    );
    const region = container.querySelector(
      '[data-clay-size="dimension.sidebar.default"]',
    );
    expect(region).not.toBeNull();
    expect(region?.getAttribute("data-clay-component")).toBe("stack");
    // The descriptor became the approved field; a plain list renders none.
    expect(container.querySelector("[data-clay-list-filter]")).not.toBeNull();
    expect(container.querySelector("input")).not.toBeNull();
  });

  it("renders a plain list with no filter affordance", () => {
    const state = installSduiTree({
      uiVersion: 3,
      rootId: 1,
      nodes: [
        { id: 1, kind: { stack: { children: [2] } } },
        {
          id: 2,
          kind: {
            list: {
              items: [{ id: "a", label: "a", detail: "src", action: null }],
            },
          },
        },
      ],
    });
    const { container } = render(
      <SduiRenderer state={state} send={async () => {}} editorSlot={<div />} />,
    );
    expect(container.querySelector("[data-clay-list-filter]")).toBeNull();
    expect(container.querySelector("input")).toBeNull();
  });
});
