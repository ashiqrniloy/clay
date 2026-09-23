// Incremental UTF-16↔UTF-8 byte-position index for CodeMirror documents.
//
// Structure: one persistent (path-copied) order-statistic treap over line
// chunks. Leaves carry per-line UTF-16/UTF-8 widths as numbers only — never
// line strings — so history states share structure instead of retaining
// duplicated text tables. Ordinary edits rebuild only the touched lines and
// O(log lines) tree nodes; conversions are a tree descent plus one
// intra-line scan.
//
// A line longer than one scan block records its block starts as well, so that
// scan resumes inside the line instead of re-walking it: before this, a 1 MiB
// single line cost a 1 MiB scan per conversion.

import { EditorState, StateField, type Transaction } from "@codemirror/state";

import { utf8Length, utf8Width } from "./position-map";

/** Minimal document surface the index needs (CodeMirror `Text` satisfies it). */
export interface LineSource {
  readonly lines: number;
  readonly length: number;
  line(number: number): { from: number; text: string };
  /** Optional: reads a UTF-16 range without materializing the whole line. */
  sliceString?(from: number, to: number): string;
}

const CHUNK_LINES = 64;

/** UTF-16 units per scan block of a long line.
 *
 * The intra-line scan is inherent to the conversion, its length is not:
 * blocks bound one conversion's walk (and the text it reads) to 8 KiB units
 * — ~25 µs on ASCII — while the tables cost ~0.1% of the line's text. */
export const SEGMENT_UNITS = 8 * 1024;

/** Block starts of one long line: `char[k]`/`byte[k]` are the line-relative
 * UTF-16 and UTF-8 offsets of block `k`, both on a scalar boundary. */
export interface LineSegments {
  readonly char: Uint32Array;
  readonly byte: Uint32Array;
}

interface ChunkData {
  readonly l16: Uint32Array;
  readonly l8: Uint32Array;
  /** Per-entry scan blocks; `undefined` for a line within one block. */
  readonly seg: (LineSegments | undefined)[];
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

function makeChunk(
  l16: Uint32Array,
  l8: Uint32Array,
  seg: (LineSegments | undefined)[],
): LeafNode {
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
    chunk: { l16, l8, seg },
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

interface LineEntries {
  readonly l16: number[];
  readonly l8: number[];
  readonly seg: (LineSegments | undefined)[];
}

/** Measures one line into `entries`, recording block starts once the line
 * outgrows one scan block. Lines at or below one block stay table-free. */
function pushLine(entries: LineEntries, text: string): void {
  const units = text.length + 1;
  entries.l16.push(units);
  if (units <= SEGMENT_UNITS) {
    entries.l8.push(utf8Length(text) + 1);
    entries.seg.push(undefined);
    return;
  }
  const char: number[] = [0];
  const byte: number[] = [0];
  let seen16 = 0;
  let seen8 = 0;
  let block16 = 0;
  for (let at = 0; at < text.length;) {
    const unit = text.charCodeAt(at);
    // ASCII fast path (the common case inside a long line): ~3x faster than
    // walking code points. Anything else re-reads the pair via codePointAt,
    // which keeps lone surrogates byte-identical to the linear reference.
    let width = 1;
    let bytes = 1;
    if (unit >= 0x80) {
      const code = text.codePointAt(at) ?? unit;
      width = code > 0xffff ? 2 : 1;
      bytes = utf8Width(code);
    }
    seen16 += width;
    seen8 += bytes;
    at += width;
    if (seen16 - block16 >= SEGMENT_UNITS) {
      block16 = seen16;
      char.push(seen16);
      byte.push(seen8);
    }
  }
  entries.l8.push(seen8 + 1);
  entries.seg.push({
    char: Uint32Array.from(char),
    byte: Uint32Array.from(byte),
  });
}

function buildTree(entries: LineEntries): IndexNode | null {
  const { l16, l8, seg } = entries;
  if (l16.length === 0) return null;
  const leaves: IndexNode[] = [];
  for (let start = 0; start < l16.length; start += CHUNK_LINES) {
    const end = Math.min(start + CHUNK_LINES, l16.length);
    leaves.push(
      makeChunk(
        Uint32Array.from(l16.slice(start, end)),
        Uint32Array.from(l8.slice(start, end)),
        seg.slice(start, end),
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
      makeChunk(c.l16.subarray(0, k), c.l8.subarray(0, k), c.seg.slice(0, k)),
      makeChunk(c.l16.subarray(k), c.l8.subarray(k), c.seg.slice(k)),
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

/** Replaces tree lines `[start, start + count)` with `entries`. */
function replaceRange(
  root: IndexNode | null,
  start: number,
  count: number,
  entries: LineEntries,
): IndexNode | null {
  const [head, rest] = split(root, start);
  const [, tail] = split(rest, count);
  return join(join(head, buildTree(entries)), tail);
}

export interface LineLocation {
  /** 0-based line index. */
  line: number;
  /** UTF-16 start of the line. */
  start16: number;
  /** UTF-8 start of the line. */
  start8: number;
  /** Line length in UTF-16 units, including the phantom newline. */
  lineUnits: number;
  /** Line-relative UTF-16 offset the intra-line scan may start from: the
   * containing block start on a long line, `0` otherwise. */
  scan16: number;
  /** Line-relative UTF-8 offset matching `scan16`, same scalar boundary. */
  scan8: number;
  /** Line-relative end of that block (next block start, or the line end):
   * always a scalar boundary, so a slice never cuts a surrogate pair. */
  scanEnd16: number;
}

/** Index of the last block starting at or before `target`. */
function lastBlockStart(coords: Uint32Array, target: number): number {
  let lo = 0;
  let hi = coords.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if ((coords[mid] ?? 0) <= target) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** Block start at or before line-relative `intra16`, as
 * `[scan16, scan8, scanEnd16]` — the block, both spaces agreeing on a scalar
 * boundary at either end. */
function segmentScan16(
  seg: LineSegments | undefined,
  intra16: number,
  lineUnits: number,
): [number, number, number] {
  const lineEnd = lineUnits - 1;
  if (!seg) return [0, 0, lineEnd];
  const block = lastBlockStart(seg.char, intra16);
  return [
    seg.char[block] ?? 0,
    seg.byte[block] ?? 0,
    seg.char[block + 1] ?? lineEnd,
  ];
}

/** Block start at or before line-relative `intra8`, as
 * `[scan16, scan8, scanEnd16]`. */
function segmentScan8(
  seg: LineSegments | undefined,
  intra8: number,
  lineUnits: number,
): [number, number, number] {
  const lineEnd = lineUnits - 1;
  if (!seg) return [0, 0, lineEnd];
  const block = lastBlockStart(seg.byte, intra8);
  return [
    seg.char[block] ?? 0,
    seg.byte[block] ?? 0,
    seg.char[block + 1] ?? lineEnd,
  ];
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
      return {
        line,
        start16: s16,
        start8: s8,
        lineUnits: 1,
        intra16: 0,
        scan16: 0,
        scan8: 0,
        scanEnd16: 0,
      };
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
      if (offset < w) {
        const [scan16, scan8, scanEnd16] = segmentScan16(c.seg[i], offset, w);
        return {
          line: line + i,
          start16: s16,
          start8: s8,
          lineUnits: w,
          intra16: offset,
          scan16,
          scan8,
          scanEnd16,
        };
      }
      offset -= w;
      s16 += w;
      s8 += c.l8[i] ?? 0;
    }
    // Defensive tail (offset === subtree width): clamp to last line end.
    const last = c.l16.length - 1;
    const units = c.l16[last] ?? 1;
    const intra16 = units - 1;
    const [scan16, scan8, scanEnd16] = segmentScan16(
      c.seg[last],
      intra16,
      units,
    );
    return {
      line: line + last,
      start16: s16 - units,
      start8: s8 - (c.l8[last] ?? 0),
      lineUnits: units,
      intra16,
      scan16,
      scan8,
      scanEnd16,
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
      return {
        line,
        start16: s16,
        start8: s8,
        lineUnits: 1,
        intra8: 0,
        scan16: 0,
        scan8: 0,
        scanEnd16: 0,
      };
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
      if (offset < w) {
        const units = c.l16[i] ?? 1;
        const [scan16, scan8, scanEnd16] = segmentScan8(
          c.seg[i],
          offset,
          units,
        );
        return {
          line: line + i,
          start16: s16,
          start8: s8,
          lineUnits: units,
          intra8: offset,
          scan16,
          scan8,
          scanEnd16,
        };
      }
      offset -= w;
      s16 += c.l16[i] ?? 0;
      s8 += w;
    }
    const last = c.l8.length - 1;
    const units = c.l16[last] ?? 1;
    const intra8 = (c.l8[last] ?? 1) - 1;
    const [scan16, scan8, scanEnd16] = segmentScan8(c.seg[last], intra8, units);
    return {
      line: line + last,
      start16: s16 - units,
      start8: s8 - (c.l8[last] ?? 0),
      lineUnits: units,
      intra8,
      scan16,
      scan8,
      scanEnd16,
    };
  }
}

/** The text one intra-line scan needs: the whole line when it fits in a
 * single block, otherwise just that block — a long line is never
 * materialized end to end per conversion. */
export function lineScanText(
  index: BytePositionIndex,
  located: LineLocation,
): string {
  const doc = index.doc;
  const from = located.start16 + located.scan16;
  const to = located.start16 + located.scanEnd16;
  if (located.scan16 === 0 && located.scanEnd16 === located.lineUnits - 1)
    return doc.line(located.line + 1).text;
  const sliceString = (
    doc as LineSource & { sliceString?: (from: number, to: number) => string }
  ).sliceString;
  if (typeof sliceString === "function") return sliceString.call(doc, from, to);
  const text = doc.line(located.line + 1).text;
  return text.slice(located.scan16, located.scan16 + (to - from));
}

/** One bounded O(document) install pass. */
export function buildPositionIndex(doc: LineSource): BytePositionIndex {
  const entries: LineEntries = { l16: [], l8: [], seg: [] };
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
      pushLine(entries, step.value);
    }
  } else {
    for (let n = 1; n <= doc.lines; n += 1) {
      pushLine(entries, doc.line(n).text);
    }
  }
  const root = buildTree(entries);
  return {
    doc,
    root,
    totalUtf16: doc.length,
    totalUtf8: Math.max(0, (root?.w8 ?? 1) - 1),
  };
}

/** Line entries for a region string (weights include phantom newlines). */
function regionEntries(region: string): LineEntries {
  const entries: LineEntries = { l16: [], l8: [], seg: [] };
  for (const text of region.split("\n")) pushLine(entries, text);
  return entries;
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
    root = replaceRange(root, oldStart + shift, oldCount, entries);
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
