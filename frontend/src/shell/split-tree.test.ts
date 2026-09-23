import { describe, expect, it } from "vitest";

import {
  MAX_PANES_PER_TAB,
  addEqualPane,
  closePane,
  movePane,
  paneCount,
  paneIds,
  resizeActive,
  singlePane,
  splitPane,
  toPersisted,
  treeFromPersisted,
} from "./split-tree";

function must<T>(value: T | null): T {
  expect(value).not.toBeNull();
  return value as T;
}

describe("split tree", () => {
  it("caps at four panes", () => {
    let tree = singlePane();
    for (let i = 0; i < 6; i += 1) {
      tree = addEqualPane(tree) ?? tree;
    }
    expect(paneCount(tree)).toBe(MAX_PANES_PER_TAB);
    expect(splitPane(tree, "horizontal")).toBeNull();
    expect(addEqualPane(tree)).toBeNull();
  });

  it("splits the active pane equally and keeps its id", () => {
    const tree = must(splitPane(singlePane(), "horizontal"));
    expect(paneIds(tree.root)).toEqual([1, 2]);
    expect(tree.activePaneId).toBe(1);
    expect(tree.root.kind === "split" && tree.root.ratio).toBe(0.5);
  });

  it("closes a pane by promoting the sibling and handing focus over", () => {
    const two = must(splitPane(singlePane(), "vertical"));
    const closed = must(closePane(two, 1));
    expect(paneIds(closed.root)).toEqual([2]);
    expect(closed.activePaneId).toBe(2);
    expect(closePane(closed)).toBeNull();
  });

  it("redivides into equal areas and preserves reading order", () => {
    const three = must(addEqualPane(must(addEqualPane(singlePane()))));
    expect(paneIds(three.root)).toEqual([1, 2, 3]);
    expect(three.root.kind === "split" && three.root.ratio).toBeCloseTo(1 / 3);
  });

  it("moves a pane only within reading-order bounds", () => {
    const two = must(splitPane(singlePane(), "horizontal"));
    expect(movePane(two, "first", 1)).toBeNull();
    const swapped = must(movePane(two, "second", 1));
    expect(paneIds(swapped.root)).toEqual([2, 1]);
    expect(movePane(swapped, "second", 1)).toBeNull();
  });

  it("keyboard-resizes along a bordering divider and clamps", () => {
    const two = must(splitPane(singlePane(), "horizontal"));
    let next = must(resizeActive(two, "right"));
    expect(next.root.kind === "split" && next.root.ratio).toBeCloseTo(0.55);
    for (let i = 0; i < 20; i += 1) next = must(resizeActive(next, "right"));
    expect(next.root.kind === "split" && next.root.ratio).toBe(0.95);
    expect(resizeActive(two, "up")).toBeNull();
  });

  it("rejects hostile persisted trees and keeps pane 1", () => {
    const hostile = treeFromPersisted(
      {
        split: {
          orientation: "horizontal",
          ratio: 0.5,
          first: { leaf: { paneId: 9 } },
          second: { leaf: { paneId: 10 } },
        },
      },
      9,
    );
    expect(paneIds(hostile.root)).toEqual([1]);
    const ok = treeFromPersisted(
      toPersisted(must(splitPane(singlePane(), "vertical")).root),
      1,
    );
    expect(paneIds(ok.root)).toEqual([1, 2]);
  });
});
