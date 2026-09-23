import { Annotation, Transaction } from "@codemirror/state";

/** Distinguishes who produced a transaction so sync can ignore non-user ops. */
export type ClayOrigin =
  "user" | "correction" | "resync" | "remote" | "undo" | "programmatic";

export const clayOrigin = Annotation.define<ClayOrigin>();

/** Annotations for server-authored content installs: never undoable. */
export function programmaticAnnotations(): Array<
  Annotation<ClayOrigin> | Annotation<boolean>
> {
  return [clayOrigin.of("programmatic"), Transaction.addToHistory.of(false)];
}

export function originOf(transaction: Transaction): ClayOrigin {
  const marked = transaction.annotation(clayOrigin);
  if (marked) return marked;
  if (transaction.isUserEvent("undo") || transaction.isUserEvent("redo")) {
    return "undo";
  }
  return "user";
}

/** True when the transaction should emit an optimistic edit. */
export function shouldEmitEdit(transaction: Transaction): boolean {
  if (!transaction.docChanged) return false;
  const origin = originOf(transaction);
  return origin === "user" || origin === "undo";
}
