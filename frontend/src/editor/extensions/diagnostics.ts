import { lintGutter, setDiagnostics, type Diagnostic } from "@codemirror/lint";
import {
  StateField,
  type EditorState,
  type Extension,
  type StateEffect,
  type TransactionSpec,
} from "@codemirror/state";

import { positionIndex } from "../position-index";
import { utf8ToUtf16Batch } from "../position-map";
import type { DiagnosticSet } from "./types";
import {
  applyRenderPatch,
  coveredRangeOf,
  guardOf,
  mapItems,
  pruneOutside,
  replaceCovered,
  type DiagnosticItem,
} from "./render-patch";

/**
 * Render-data owner for diagnostics: projected UTF-16 items held by a
 * CodeMirror state field, mapped through local edits, replaced per covered
 * viewport range per authority. The lint extension keeps its own mapped
 * copy; both stay in sync because patches push `setDiagnostics` in the same
 * transaction.
 */
const diagnosticField = StateField.define<DiagnosticItem[]>({
  create: () => [],
  update(value, transaction) {
    let items = value;
    for (const effect of transaction.effects) {
      if (!effect.is(applyRenderPatch)) continue;
      const patch = effect.value;
      if (patch.kind === "reset") items = [];
      else if (patch.kind === "diagnostic")
        items = pruneOutside(
          replaceCovered(items, patch.authority, patch.covered, patch.items),
          guardOf(patch.covered),
        );
    }
    if (!transaction.docChanged) return items;
    return mapItems(items, transaction.changes);
  },
});

export const diagnosticExtension: Extension = [diagnosticField, lintGutter()];

/**
 * Build one transaction spec applying a validated diagnostic set: the
 * atomic patch effect plus the lint sync, computed from the field's current
 * (edit-mapped) items so the two never diverge.
 */
export function diagnosticPatch(
  state: EditorState,
  set: DiagnosticSet,
): TransactionSpec {
  const index = positionIndex(state);
  const covered = coveredRangeOf(
    index,
    set.viewportByteStart,
    set.viewportByteEnd,
  );
  const authority = `${set.source}:${set.provenance.packagePrefix}`;
  const converted = utf8ToUtf16Batch(
    index,
    set.spans.flatMap((span) => [span.byteStart, span.byteEnd]),
  );
  const items: DiagnosticItem[] = [];
  for (let i = 0; i < set.spans.length; i += 1) {
    const span = set.spans[i];
    if (!span) continue;
    const from = converted[i * 2] ?? 0;
    const to = converted[i * 2 + 1] ?? 0;
    if (from > to || to > index.totalUtf16) continue;
    items.push({
      from,
      to,
      authority,
      severity: span.severity,
      message: span.message,
      source: span.source,
    });
  }
  const merged = pruneOutside(
    replaceCovered(state.field(diagnosticField), authority, covered, items),
    guardOf(covered),
  );
  // setDiagnostics returns a spec; fold its effect in so the patch and the
  // lint sync land in one transaction.
  const lint = setDiagnostics(state, lintDiagnostics(merged));
  return {
    effects: [
      applyRenderPatch.of({ kind: "diagnostic", authority, covered, items }),
      ...(lint.effects as readonly StateEffect<unknown>[]),
    ],
  };
}

export const resetDiagnostics = (state: EditorState): TransactionSpec => {
  const lint = setDiagnostics(state, []);
  return {
    effects: [
      applyRenderPatch.of({ kind: "reset" }),
      ...(lint.effects as readonly StateEffect<unknown>[]),
    ],
  };
};

function lintDiagnostics(items: readonly DiagnosticItem[]): Diagnostic[] {
  return visibleDiagnostics(items).map((item) => ({
    from: item.from,
    to: item.to,
    severity: item.severity,
    message: item.message,
    source: item.source,
    markClass: `cm-clay-diagnostic-${item.severity}`,
  }));
}

function isSuppressor(item: DiagnosticItem): boolean {
  return item.source !== "tree-sitter" && item.severity !== "info";
}

/**
 * Tree-sitter diagnostics are suppressed under any non-tree-sitter
 * error/warning. Suppressors merge into disjoint intervals first, so each
 * span costs one binary search instead of a comparison against every other
 * span (the previous implementation was quadratic in patch size).
 */
export function visibleDiagnostics(
  items: readonly DiagnosticItem[],
): DiagnosticItem[] {
  const suppressors = items.filter(isSuppressor);
  if (!suppressors.length) return [...items];
  const merged: Array<{ from: number; to: number }> = [];
  for (const item of [...suppressors].sort(
    (left, right) => left.from - right.from,
  )) {
    const last = merged.at(-1);
    if (last && item.from <= last.to) last.to = Math.max(last.to, item.to);
    else merged.push({ from: item.from, to: item.to });
  }
  return items.filter(
    (item) =>
      item.source !== "tree-sitter" ||
      !overlapsMerged(merged, item.from, item.to),
  );
}

function overlapsMerged(
  merged: ReadonlyArray<{ from: number; to: number }>,
  from: number,
  to: number,
): boolean {
  // Greatest interval starting at or before `from`.
  let lo = 0;
  let hi = merged.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (merged[mid]?.from !== undefined && merged[mid].from <= from)
      lo = mid + 1;
    else hi = mid;
  }
  const before = lo > 0 ? (merged[lo - 1] ?? null) : null;
  if (before && before.to > from && before.from < to) return true;
  const after = lo < merged.length ? (merged[lo] ?? null) : null;
  return !!after && after.from < to;
}
