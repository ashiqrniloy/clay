// Incremental UTF-16↔UTF-8 byte-position index for CodeMirror documents.
//
// Structure: one persistent (path-copied) order-statistic treap over line
// chunks. Leaves carry per-line UTF-16/UTF-8 widths as numbers only — never
// line strings — so history states share structure instead of retaining
// duplicated text tables. Ordinary edits rebuild only the touched lines and
// O(log lines) tree nodes; conversions are a tree descent plus one
// intra-line scan.

import { EditorState, StateField, type Transaction } from "@codemirror/state";

import { utf8Length } from "./position-map";

/** Minimal document surface the index needs (CodeMirror `Text` satisfies it). */
export interface LineSource {
  readonly lines: number;
  readonly length: number;
  line(number: number): { from: number; text: string };
}

const CHUNK_LINES = 64;

interface ChunkData {
  readonly l16: Uint32Array;
  readonly l8: Uint32Array;
}

interface LeafNode {
  readonly kind: "leaf";
  readonly lines: number;
  /** UTF-16 units including each line's phantom trailing newline. */
  readonly w16: number;
  /** UTF-8 bytes including each line's phantom trailing newline. */
  readonly w8: number;
  readonly prio: number;
  readonly chunk: ChunkData;
}

interface BranchNode {
  readonly kind: "branch";
  readonly lines: number;
  readonly w16: number;
  readonly w8: number;
  readonly prio: number;
  readonly left: IndexNode;
  readonly right: IndexNode;
}

type IndexNode = LeafNode | BranchNode;

export interface BytePositionIndex {
  readonly doc: LineSource;
  readonly root: IndexNode | null;
  readonly totalUtf16: number;
  readonly totalUtf8: number;
}

// Deterministic PRNG (xorshift32): treap priorities must be stable so the
// same edit sequence always produces the same tree shape.
let prngState = 0x9e3779b9;

function nextPriority(): number {
  let x = prngState;
  x ^= (x << 13) >>> 0;
  x ^= x >>> 17;
  x ^= (x << 5) >>> 0;
  prngState = x;
  return x || 1;
}

function makeChunk(l16: Uint32Array, l8: Uint32Array): LeafNode {
  let w16 = 0;
  let w8 = 0;
  for (let i = 0; i < l16.length; i += 1) {
    w16 += l16[i] ?? 0;
    w8 += l8[i] ?? 0;
  }
  return {
    kind: "leaf",
    lines: l16.length,
    w16,
    w8,
    prio: nextPriority(),
    chunk: { l16, l8 },
  };
}

function makeBranch(left: IndexNode, right: IndexNode): BranchNode {
  return {
    kind: "branch",
    lines: left.lines + right.lines,
    w16: left.w16 + right.w16,
    w8: left.w8 + right.w8,
    prio: left.prio >= right.prio ? left.prio : right.prio,
    left,
    right,
  };
}

/** Cartesian-tree (max-heap on priority) build over the chunk sequence.
 * Divide-and-conquer on the maximum priority: expected O(n log n), same
 * expected depth as the classic stack algorithm but trivially correct for
 * immutable nodes. */
function cartesian(
  leaves: IndexNode[],
  lo: number,
  hi: number,
): IndexNode | null {
  if (lo >= hi) return null;
  if (hi - lo === 1) return leaves[lo] ?? null;
  let maxI = lo;
  for (let i = lo + 1; i < hi; i += 1) {
    const leaf = leaves[i];
    const best = leaves[maxI];
    if (leaf && best && leaf.prio > best.prio) maxI = i;
  }
  const root = leaves[maxI];
  if (!root) return null;
  const right = cartesian(leaves, maxI + 1, hi);
  const left = cartesian(leaves, lo, maxI);
  // `root` carries the range's maximum priority, so both joins place it on
  // top and the heap invariant holds by construction.
  let node: IndexNode = right ? (join(root, right) ?? root) : root;
  if (left) node = join(left, node) ?? node;
  return node;
}

function buildTree(l16: number[], l8: number[]): IndexNode | null {
  if (l16.length === 0) return null;
  const leaves: IndexNode[] = [];
  for (let start = 0; start < l16.length; start += CHUNK_LINES) {
    const end = Math.min(start + CHUNK_LINES, l16.length);
    leaves.push(
      makeChunk(
        Uint32Array.from(l16.slice(start, end)),
        Uint32Array.from(l8.slice(start, end)),
      ),
    );
  }
  return cartesian(leaves, 0, leaves.length);
}

/** Treap split: first `k` lines to the left result. */
function split(
  node: IndexNode | null,
  k: number,
): [IndexNode | null, IndexNode | null] {
  if (!node || k <= 0) return [null, node];
  if (k >= node.lines) return [node, null];
  if (node.kind === "leaf") {
    const c = node.chunk;
    // subarray shares the read-only buffer: persistence without copying.
    return [
      makeChunk(c.l16.subarray(0, k), c.l8.subarray(0, k)),
      makeChunk(c.l16.subarray(k), c.l8.subarray(k)),
    ];
  }
  if (k < node.left.lines) {
    const [head, tail] = split(node.left, k);
    return [head, tail ? join(tail, node.right) : node.right];
  }
  if (k > node.left.lines) {
    const [head, tail] = split(node.right, k - node.left.lines);
    return [head ? join(node.left, head) : node.left, tail];
  }
  return [node.left, node.right];
}

/** Treap merge; `a`'s lines all precede `b`'s. */
function join(a: IndexNode | null, b: IndexNode | null): IndexNode | null {
  if (!a) return b;
  if (!b) return a;
  if (a.prio >= b.prio) {
    if (a.kind === "leaf") return makeBranch(a, b);
    const merged = join(a.right, b);
    return merged ? makeBranch(a.left, merged) : a.left;
  }
  if (b.kind === "leaf") return makeBranch(a, b);
  const merged = join(a, b.left);
  return merged ? makeBranch(merged, b.right) : b.right;
}

/** Replaces tree lines `[start, start + count)` with `l16`/`l8` entries. */
function replaceRange(
  root: IndexNode | null,
  start: number,
  count: number,
  l16: number[],
  l8: number[],
): IndexNode | null {
  const [head, rest] = split(root, start);
  const [, tail] = split(rest, count);
  return join(join(head, buildTree(l16, l8)), tail);
}

export interface LineLocation {
  /** 0-based line index. */
  line: number;
  /** UTF-16 start of the line. */
  start16: number;
  /** UTF-8 start of the line. */
  start8: number;
}

/** Finds the line containing UTF-16 `offset` (must be < totalUtf16). */
export function locateLine16(
  index: BytePositionIndex,
  offset: number,
): LineLocation & { intra16: number } {
  let node: IndexNode | null = index.root;
  let line = 0;
  let s16 = 0;
  let s8 = 0;
  for (;;) {
    if (!node) {
      // Defensive: callers clamp below totalUtf16, so the root covers it.
      return { line, start16: s16, start8: s8, intra16: 0 };
    }
    if (node.kind === "branch") {
      const left = node.left;
      if (offset < left.w16) {
        node = left;
        continue;
      }
      offset -= left.w16;
      line += left.lines;
      s16 += left.w16;
      s8 += left.w8;
      node = node.right;
      continue;
    }
    const c = node.chunk;
    for (let i = 0; i < c.l16.length; i += 1) {
      const w = c.l16[i] ?? 0;
      if (offset < w)
        return { line: line + i, start16: s16, start8: s8, intra16: offset };
      offset -= w;
      s16 += w;
      s8 += c.l8[i] ?? 0;
    }
    // Defensive tail (offset === subtree width): clamp to last line end.
    const last = c.l16.length - 1;
    return {
      line: line + last,
      start16: s16 - (c.l16[last] ?? 0),
      start8: s8 - (c.l8[last] ?? 0),
      intra16: (c.l16[last] ?? 1) - 1,
    };
  }
}

/** Finds the line containing UTF-8 `offset` (must be < totalUtf8). */
export function locateLine8(
  index: BytePositionIndex,
  offset: number,
): LineLocation & { intra8: number } {
  let node: IndexNode | null = index.root;
  let line = 0;
  let s16 = 0;
  let s8 = 0;
  for (;;) {
    if (!node) {
      return { line, start16: s16, start8: s8, intra8: 0 };
    }
    if (node.kind === "branch") {
      const left = node.left;
      if (offset < left.w8) {
        node = left;
        continue;
      }
      offset -= left.w8;
      line += left.lines;
      s16 += left.w16;
      s8 += left.w8;
      node = node.right;
      continue;
    }
    const c = node.chunk;
    for (let i = 0; i < c.l8.length; i += 1) {
      const w = c.l8[i] ?? 0;
      if (offset < w)
        return { line: line + i, start16: s16, start8: s8, intra8: offset };
      offset -= w;
      s16 += c.l16[i] ?? 0;
      s8 += w;
    }
    const last = c.l8.length - 1;
    return {
      line: line + last,
      start16: s16 - (c.l16[last] ?? 0),
      start8: s8 - (c.l8[last] ?? 0),
      intra8: (c.l8[last] ?? 1) - 1,
    };
  }
}

/** One bounded O(document) install pass. */
export function buildPositionIndex(doc: LineSource): BytePositionIndex {
  const l16: number[] = [];
  const l8: number[] = [];
  const iterLines = (
    doc as LineSource & {
      iterLines?: (from?: number, to?: number) => Iterator<string>;
    }
  ).iterLines;
  if (typeof iterLines === "function") {
    // Fast path for real CodeMirror Text: one linear pass, no per-line
    // rope descent.
    const cursor = iterLines.call(doc);
    for (let step = cursor.next(); !step.done; step = cursor.next()) {
      const text = step.value;
      l16.push(text.length + 1);
      l8.push(utf8Length(text) + 1);
    }
  } else {
    for (let n = 1; n <= doc.lines; n += 1) {
      const text = doc.line(n).text;
      l16.push(text.length + 1);
      l8.push(utf8Length(text) + 1);
    }
  }
  const root = buildTree(l16, l8);
  return {
    doc,
    root,
    totalUtf16: doc.length,
    totalUtf8: Math.max(0, (root?.w8 ?? 1) - 1),
  };
}

/** Line entries for a region string (weights include phantom newlines). */
function regionEntries(region: string): {
  l16: number[];
  l8: number[];
} {
  const lines = region.split("\n");
  const l16: number[] = new Array(lines.length);
  const l8: number[] = new Array(lines.length);
  for (let i = 0; i < lines.length; i += 1) {
    const text = lines[i] ?? "";
    l16[i] = text.length + 1;
    l8[i] = utf8Length(text) + 1;
  }
  return { l16, l8 };
}

/**
 * Incremental update: each changed range replaces only its whole-line span.
 * Later ranges shift by the line-count delta of earlier ones (changed
 * ranges are ordered and disjoint).
 */
export function updatePositionIndex(
  index: BytePositionIndex,
  transaction: Transaction,
): BytePositionIndex {
  if (!transaction.docChanged) return index;
  const oldDoc = index.doc as LineSource & {
    lineAt(pos: number): {
      number: number;
      from: number;
      to: number;
      text: string;
    };
  };
  const newDoc = transaction.newDoc;
  let root = index.root;
  let shift = 0;
  transaction.changes.iterChangedRanges((fromA, toA, fromB, toB) => {
    const first = oldDoc.lineAt(fromA);
    const last = oldDoc.lineAt(toA);
    const oldStart = first.number - 1;
    const oldCount = last.number - first.number + 1;
    const prefix = first.text.slice(0, fromA - first.from);
    const suffix = last.text.slice(toA - last.from);
    const region = prefix + newDoc.sliceString(fromB, toB) + suffix;
    const entries = regionEntries(region);
    root = replaceRange(
      root,
      oldStart + shift,
      oldCount,
      entries.l16,
      entries.l8,
    );
    shift += entries.l16.length - oldCount;
  });
  return {
    doc: newDoc,
    root,
    totalUtf16: newDoc.length,
    totalUtf8: Math.max(0, (root?.w8 ?? 1) - 1),
  };
}

/**
 * The one shared position field. Install it before any extension that reads
 * it; consumers outside field updates read via `positionIndex(state)`.
 */
export const bytePositionField = StateField.define<BytePositionIndex>({
  create: (state) => buildPositionIndex(state.doc),
  update: (index, transaction) => updatePositionIndex(index, transaction),
});

/** Reads the shared field, falling back to a one-pass build when absent. */
export function positionIndex(state: EditorState): BytePositionIndex {
  return state.field(bytePositionField, false) ?? buildPositionIndex(state.doc);
}

/** Debug/test-only statistics: node and chunk counts. */
export function positionIndexStats(index: BytePositionIndex): {
  lines: number;
  nodes: number;
  chunks: number;
} {
  let nodes = 0;
  let chunks = 0;
  const walk = (node: IndexNode | null): void => {
    if (!node) return;
    nodes += 1;
    if (node.kind === "leaf") chunks += 1;
    else {
      walk(node.left);
      walk(node.right);
    }
  };
  walk(index.root);
  return { lines: index.root?.lines ?? 0, nodes, chunks };
}
