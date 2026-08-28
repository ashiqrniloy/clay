import type { Text } from "@codemirror/state";

import { buildPositionIndex, type BytePositionIndex } from "../position-index";
import { utf16ToUtf8Indexed, utf8Length } from "../position-map";

export type EditOperation =
  | { insert: { byteOffset: number; text: string } }
  | { delete: { start: number; end: number } }
  | { replace: { start: number; end: number; text: string } };

export interface TextChange {
  /** UTF-16 offset in the pre-change document. */
  from: number;
  /** UTF-16 offset in the pre-change document. */
  to: number;
  insert: string;
}

/**
 * Convert CodeMirror start-state changes into sequential byte-range
 * operations. Changes are assumed non-overlapping and ordered by `from`.
 *
 * Byte offsets come from the shared incremental position index — one tree
 * descent each, never a full-document scan or flattened string. Callers on
 * the live edit path pass the transaction start state's index; the document
 * fallback rebuilds one pass (tests and detached callers).
 */
export function changesToOperations(
  oldDoc: Text,
  changes: readonly TextChange[],
  index?: BytePositionIndex,
): EditOperation[] {
  const resolved = index ?? buildPositionIndex(oldDoc);
  const originals = changes.map((change) => ({
    start: utf16ToUtf8Indexed(resolved, change.from),
    end: utf16ToUtf8Indexed(resolved, change.to),
    insert: change.insert,
  }));
  const operations: EditOperation[] = [];
  let delta = 0;
  for (const range of originals) {
    const start = range.start + delta;
    const end = range.end + delta;
    operations.push(toOperation(start, end, range.insert));
    delta += utf8Length(range.insert) - (range.end - range.start);
  }
  return operations;
}

function toOperation(
  start: number,
  end: number,
  insert: string,
): EditOperation {
  if (start === end) return { insert: { byteOffset: start, text: insert } };
  if (insert.length === 0) return { delete: { start, end } };
  return { replace: { start, end, text: insert } };
}
