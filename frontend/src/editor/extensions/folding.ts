import {
  StateField,
  type EditorState,
  type Extension,
  type StateEffect as StateEffectValue,
} from "@codemirror/state";
import { foldGutter, foldKeymap, foldService } from "@codemirror/language";
import { keymap } from "@codemirror/view";

import { positionIndex } from "../position-index";
import { utf8ToUtf16Batch } from "../position-map";
import type { FoldingRangeSet } from "./types";
import {
  applyRenderPatch,
  mapItems,
  replaceCovered,
  type FoldItem,
  type RenderPatch,
} from "./render-patch";

/**
 * Render-data owner for folds: projected UTF-16 ranges held by a state
 * field, mapped through local edits, replaced wholesale per package
 * authority (fold sets are whole-document, so the covered range is the
 * entire document). Kept sorted by (from, to) for binary lookup.
 */
const foldField = StateField.define<FoldItem[]>({
  create: () => [],
  update(value, transaction) {
    let items = value;
    for (const effect of transaction.effects) {
      if (!effect.is(applyRenderPatch)) continue;
      const patch = effect.value;
      if (patch.kind === "reset") items = [];
      else if (patch.kind === "fold")
        items = replaceCovered(
          items,
          patch.authority,
          { from: 0, to: transaction.state.doc.length },
          patch.ranges,
        );
    }
    if (!transaction.docChanged) return items;
    return mapItems(items, transaction.changes).filter(
      (item) => item.from < item.to,
    );
  },
});

export const foldingExtension: Extension = [
  foldField,
  foldService.of((state, lineStart, lineEnd) => {
    const items = state.field(foldField);
    // Binary search to the first range starting at/after the line start,
    // then scan only the ranges that start on this line.
    // ponytail: window scan is O(ranges starting on the line); a per-line
    // segment tree is the upgrade path if pathological single-line density
    // ever shows up.
    let lo = 0;
    let hi = items.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (items[mid]?.from !== undefined && items[mid].from < lineStart)
        lo = mid + 1;
      else hi = mid;
    }
    let candidate: FoldItem | null = null;
    for (let index = lo; index < items.length; index += 1) {
      const item = items[index];
      if (!item) continue;
      if (item.from > lineEnd) break;
      if (item.to <= lineEnd) continue;
      if (!candidate || item.to < candidate.to) candidate = item;
    }
    return candidate ? { from: lineEnd, to: candidate.to } : null;
  }),
  foldGutter(),
  keymap.of(foldKeymap),
];

export function foldPatch(
  state: EditorState,
  set: FoldingRangeSet,
): StateEffectValue<RenderPatch> {
  const index = positionIndex(state);
  const converted = utf8ToUtf16Batch(
    index,
    set.ranges.flatMap((range) => [range.byteStart, range.byteEnd]),
  );
  const ranges: FoldItem[] = [];
  for (let i = 0; i < set.ranges.length; i += 1) {
    const range = set.ranges[i];
    if (!range) continue;
    const from = converted[i * 2] ?? 0;
    const to = converted[i * 2 + 1] ?? 0;
    if (from >= to || to > index.totalUtf16) continue;
    ranges.push({ from, to, authority: set.packagePrefix, label: range.label });
  }
  return applyRenderPatch.of({
    kind: "fold",
    authority: set.packagePrefix,
    ranges,
  });
}

export const installFolds = foldPatch;
export const resetFolds = () => applyRenderPatch.of({ kind: "reset" });
