// Client-local pane split tree. Mirrors `src/shell/layout.rs` semantics:
// 4-pane cap, ratio clamp 0.05–0.95, equal-area comb, close-merges-sibling,
// reading-order move. Content type is not stored here.

export const MAX_PANES_PER_TAB = 4;
export const MIN_SPLIT_RATIO = 0.05;
export const MAX_SPLIT_RATIO = 0.95;
export const KEYBOARD_RESIZE_STEP = 0.05;
export const DEFAULT_PANE_ID = 1;

export type SplitOrientation = "horizontal" | "vertical";
export type SplitChild = "first" | "second";

export type SplitNode =
  | { kind: "leaf"; paneId: number }
  | {
      kind: "split";
      orientation: SplitOrientation;
      ratio: number;
      first: SplitNode;
      second: SplitNode;
    };

export interface SplitTree {
  root: SplitNode;
  activePaneId: number;
}

export function clampRatio(value: number): number {
  if (!Number.isFinite(value)) return 0.5;
  return Math.min(MAX_SPLIT_RATIO, Math.max(MIN_SPLIT_RATIO, value));
}

export function singlePane(paneId = DEFAULT_PANE_ID): SplitTree {
  return { root: { kind: "leaf", paneId }, activePaneId: paneId };
}

export function paneIds(node: SplitNode, out: number[] = []): number[] {
  if (node.kind === "leaf") {
    out.push(node.paneId);
    return out;
  }
  paneIds(node.first, out);
  paneIds(node.second, out);
  return out;
}

export function paneCount(tree: SplitTree): number {
  return paneIds(tree.root).length;
}

export function containsPane(node: SplitNode, paneId: number): boolean {
  if (node.kind === "leaf") return node.paneId === paneId;
  return containsPane(node.first, paneId) || containsPane(node.second, paneId);
}

export function nextPaneId(tree: SplitTree): number {
  const ids = paneIds(tree.root);
  return (ids.length === 0 ? 0 : Math.max(...ids)) + 1;
}

export function focusPane(tree: SplitTree, paneId: number): SplitTree | null {
  if (!containsPane(tree.root, paneId)) return null;
  return { ...tree, activePaneId: paneId };
}

export function nextPane(tree: SplitTree): number {
  const ids = paneIds(tree.root);
  const idx = ids.indexOf(tree.activePaneId);
  return ids[(idx < 0 ? 0 : idx + 1) % ids.length] ?? tree.activePaneId;
}

export function prevPane(tree: SplitTree): number {
  const ids = paneIds(tree.root);
  const idx = ids.indexOf(tree.activePaneId);
  return (
    ids[(idx < 0 ? 0 : idx + ids.length - 1) % ids.length] ?? tree.activePaneId
  );
}

export function splitPane(
  tree: SplitTree,
  orientation: SplitOrientation,
  targetPane = tree.activePaneId,
  position: SplitChild = "second",
): SplitTree | null {
  if (paneCount(tree) >= MAX_PANES_PER_TAB) return null;
  if (!containsPane(tree.root, targetPane)) return null;
  const newPaneId = nextPaneId(tree);
  const root = splitNode(
    tree.root,
    targetPane,
    newPaneId,
    orientation,
    0.5,
    position,
  );
  if (!root) return null;
  return { root, activePaneId: tree.activePaneId };
}

export function closePane(
  tree: SplitTree,
  paneId = tree.activePaneId,
): SplitTree | null {
  if (paneCount(tree) <= 1) return null;
  if (!containsPane(tree.root, paneId)) return null;
  const closed = closeNode(tree.root, paneId);
  if (!closed) return null;
  const active =
    tree.activePaneId === paneId ? closed.handoff : tree.activePaneId;
  return { root: closed.root, activePaneId: active };
}

export function addEqualPane(tree: SplitTree): SplitTree | null {
  if (paneCount(tree) >= MAX_PANES_PER_TAB) return null;
  const orientation =
    tree.root.kind === "split" ? tree.root.orientation : "horizontal";
  const leaves = paneIds(tree.root);
  leaves.push(nextPaneId(tree));
  return {
    root: equalComb(leaves, orientation),
    activePaneId: tree.activePaneId,
  };
}

export function movePane(
  tree: SplitTree,
  direction: SplitChild,
  paneId = tree.activePaneId,
): SplitTree | null {
  const ids = paneIds(tree.root);
  const idx = ids.indexOf(paneId);
  if (idx < 0) return null;
  const neighbor = direction === "first" ? idx - 1 : idx + 1;
  const other = ids[neighbor];
  if (other == null) return null;
  return {
    root: swapIds(tree.root, paneId, other),
    activePaneId: tree.activePaneId,
  };
}

export function resizeActive(
  tree: SplitTree,
  direction: "left" | "right" | "up" | "down",
): SplitTree | null {
  const wanted: SplitOrientation =
    direction === "left" || direction === "right" ? "horizontal" : "vertical";
  const found = findSplitFor(tree.root, tree.activePaneId, wanted, []);
  if (!found) return null;
  const growFirst =
    (found.side === "first" &&
      (direction === "right" || direction === "down")) ||
    (found.side === "second" && (direction === "left" || direction === "up"));
  const delta = growFirst ? KEYBOARD_RESIZE_STEP : -KEYBOARD_RESIZE_STEP;
  const next = clampRatio(found.node.ratio + delta);
  if (next === found.node.ratio) return tree;
  return {
    root: setRatioAt(tree.root, found.path, next),
    activePaneId: tree.activePaneId,
  };
}

export function updateRatioAt(
  tree: SplitTree,
  path: SplitChild[],
  ratio: number,
): SplitTree {
  return {
    root: setRatioAt(tree.root, path, clampRatio(ratio)),
    activePaneId: tree.activePaneId,
  };
}

/** Persisted `splitTree` encoding used by layout.json v2. */
export type PersistedSplitNode =
  | { leaf: { paneId: number } }
  | {
      split: {
        orientation: SplitOrientation;
        ratio: number;
        first: PersistedSplitNode;
        second: PersistedSplitNode;
      };
    };

export function toPersisted(node: SplitNode): PersistedSplitNode {
  if (node.kind === "leaf") return { leaf: { paneId: node.paneId } };
  return {
    split: {
      orientation: node.orientation,
      ratio: node.ratio,
      first: toPersisted(node.first),
      second: toPersisted(node.second),
    },
  };
}

export function fromPersisted(
  node: PersistedSplitNode | null | undefined,
): SplitNode | null {
  if (!node) return null;
  if ("leaf" in node) {
    const paneId = node.leaf?.paneId;
    if (!paneId || paneId === 0) return null;
    return { kind: "leaf", paneId };
  }
  if ("split" in node) {
    const first = fromPersisted(node.split.first);
    const second = fromPersisted(node.split.second);
    if (!first || !second) return null;
    return {
      kind: "split",
      orientation: node.split.orientation,
      ratio: clampRatio(node.split.ratio),
      first,
      second,
    };
  }
  return null;
}

export function treeFromPersisted(
  node: PersistedSplitNode | null | undefined,
  activePane: number,
): SplitTree {
  const root = fromPersisted(node) ?? { kind: "leaf", paneId: DEFAULT_PANE_ID };
  const ids = paneIds(root);
  if (ids.length > MAX_PANES_PER_TAB || !ids.includes(DEFAULT_PANE_ID)) {
    return singlePane();
  }
  const activePaneId = ids.includes(activePane) ? activePane : DEFAULT_PANE_ID;
  return { root, activePaneId };
}

function splitNode(
  node: SplitNode,
  target: number,
  newPaneId: number,
  orientation: SplitOrientation,
  ratio: number,
  position: SplitChild,
): SplitNode | null {
  if (node.kind === "leaf") {
    if (node.paneId !== target) return null;
    const original: SplitNode = { kind: "leaf", paneId: target };
    const created: SplitNode = { kind: "leaf", paneId: newPaneId };
    const first = position === "first" ? created : original;
    const second = position === "first" ? original : created;
    return { kind: "split", orientation, ratio, first, second };
  }
  const left = splitNode(
    node.first,
    target,
    newPaneId,
    orientation,
    ratio,
    position,
  );
  const right = splitNode(
    node.second,
    target,
    newPaneId,
    orientation,
    ratio,
    position,
  );
  if (left && !right) return { ...node, first: left };
  if (right && !left) return { ...node, second: right };
  return null;
}

function closeNode(
  node: SplitNode,
  paneId: number,
): { root: SplitNode; handoff: number } | null {
  if (node.kind === "leaf") return null;
  if (node.first.kind === "leaf" && node.first.paneId === paneId) {
    return { root: node.second, handoff: firstLeaf(node.second) };
  }
  if (node.second.kind === "leaf" && node.second.paneId === paneId) {
    return { root: node.first, handoff: firstLeaf(node.first) };
  }
  const left = closeNode(node.first, paneId);
  if (left)
    return { root: { ...node, first: left.root }, handoff: left.handoff };
  const right = closeNode(node.second, paneId);
  if (right)
    return { root: { ...node, second: right.root }, handoff: right.handoff };
  return null;
}

function firstLeaf(node: SplitNode): number {
  return node.kind === "leaf" ? node.paneId : firstLeaf(node.first);
}

function equalComb(leaves: number[], orientation: SplitOrientation): SplitNode {
  const ratio = clampRatio(1 / leaves.length);
  const head = leaves[0] ?? DEFAULT_PANE_ID;
  const tail = leaves[1] ?? head;
  const first: SplitNode = { kind: "leaf", paneId: head };
  const second: SplitNode =
    leaves.length === 2
      ? { kind: "leaf", paneId: tail }
      : equalComb(leaves.slice(1), orientation);
  return { kind: "split", orientation, ratio, first, second };
}

function swapIds(node: SplitNode, a: number, b: number): SplitNode {
  if (node.kind === "leaf") {
    const paneId = node.paneId === a ? b : node.paneId === b ? a : node.paneId;
    return { kind: "leaf", paneId };
  }
  return {
    ...node,
    first: swapIds(node.first, a, b),
    second: swapIds(node.second, a, b),
  };
}

function findSplitFor(
  node: SplitNode,
  paneId: number,
  orientation: SplitOrientation,
  path: SplitChild[],
): {
  node: Extract<SplitNode, { kind: "split" }>;
  path: SplitChild[];
  side: SplitChild;
} | null {
  if (node.kind === "leaf") return null;
  if (containsPane(node.first, paneId)) {
    if (node.orientation === orientation) {
      return { node, path, side: "first" };
    }
    return findSplitFor(node.first, paneId, orientation, [...path, "first"]);
  }
  if (containsPane(node.second, paneId)) {
    if (node.orientation === orientation) {
      return { node, path, side: "second" };
    }
    return findSplitFor(node.second, paneId, orientation, [...path, "second"]);
  }
  return null;
}

function setRatioAt(
  node: SplitNode,
  path: SplitChild[],
  ratio: number,
): SplitNode {
  if (node.kind === "leaf") return node;
  if (path.length === 0) return { ...node, ratio };
  const [head, ...rest] = path;
  if (head === "first")
    return { ...node, first: setRatioAt(node.first, rest, ratio) };
  return { ...node, second: setRatioAt(node.second, rest, ratio) };
}
