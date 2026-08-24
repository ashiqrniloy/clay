import { describe, expect, it } from "vitest";

import { applySduiUpdate, installSduiTree } from "./state";
import type { SduiTree } from "./types";

const tree: SduiTree = {
  uiVersion: 3,
  rootId: 1,
  nodes: [
    { id: 1, kind: { stack: { children: [2, 3] } } },
    { id: 2, kind: { label: { text: "old" } } },
    { id: 3, kind: { label: { text: "stable" } } },
  ],
};

describe("SDUI stable-id state", () => {
  it("replaces only targeted nodes and preserves surviving object identity", () => {
    const initial = installSduiTree(tree);
    const stable = initial.nodes.get(3);
    const next = applySduiUpdate(initial, {
      baseUiVersion: 3,
      newUiVersion: 4,
      operations: [
        { replaceNode: { node: { id: 2, kind: { label: { text: "new" } } } } },
      ],
    });
    expect(next?.nodes.get(2)?.kind).toEqual({ label: { text: "new" } });
    expect(next?.nodes.get(3)).toBe(stable);
  });

  it("drops stale updates without mutating current state", () => {
    const initial = installSduiTree(tree);
    const next = applySduiUpdate(initial, {
      baseUiVersion: 2,
      newUiVersion: 4,
      operations: [{ removeNode: { nodeId: 3 } }],
    });
    expect(next).toBe(initial);
  });
});
