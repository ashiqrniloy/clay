import { lintGutter, setDiagnostics, type Diagnostic } from "@codemirror/lint";
import type { Extension, TransactionSpec } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

import { textIndex, utf8ToUtf16Indexed } from "../position-map";
import type { DiagnosticSet, DiagnosticSpan } from "./types";

export const diagnosticExtension: Extension = lintGutter();

export class DiagnosticProjection {
  private chunks = new Map<string, DiagnosticSet>();

  clear(view?: EditorView): void {
    this.chunks.clear();
    if (view) view.dispatch(setDiagnostics(view.state, []));
  }

  /**
   * Build the lint-state effect for a validated diagnostic set without
   * dispatching, so callers can batch several feature updates into one
   * editor transaction.
   */
  prepare(view: EditorView, set: DiagnosticSet): TransactionSpec | null {
    for (const [key, cached] of this.chunks) {
      if (cached.documentVersion !== set.documentVersion)
        this.chunks.delete(key);
    }
    this.chunks.set(
      `${set.source}:${set.provenance.packagePrefix}:${set.viewportByteStart}:${set.viewportByteEnd}`,
      set,
    );
    const index = textIndex(view.state.doc);
    const spans = [...this.chunks.values()].flatMap((chunk) => chunk.spans);
    const suppressors = spans.filter(
      (span) => span.source !== "tree-sitter" && span.severity !== "info",
    );
    const visible = spans.filter(
      (span) =>
        span.source !== "tree-sitter" ||
        !suppressors.some((other) => overlaps(span, other)),
    );
    const diagnostics: Diagnostic[] = visible.map((span) => ({
      from: utf8ToUtf16Indexed(index, span.byteStart),
      to: utf8ToUtf16Indexed(index, span.byteEnd),
      severity: span.severity,
      message: span.message,
      source: span.source,
      markClass: `cm-clay-diagnostic-${span.severity}`,
    }));
    return setDiagnostics(view.state, diagnostics);
  }

  install(view: EditorView, set: DiagnosticSet): void {
    const effect = this.prepare(view, set);
    if (effect) view.dispatch(effect);
  }
}

function overlaps(left: DiagnosticSpan, right: DiagnosticSpan): boolean {
  return left.byteStart < right.byteEnd && right.byteStart < left.byteEnd;
}
