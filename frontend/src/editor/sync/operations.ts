import { utf16ToUtf8, utf8Length } from "../position-map";

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
 */
export function changesToOperations(
  oldText: string,
  changes: readonly TextChange[],
): EditOperation[] {
  const originals = changes.map((change) => ({
    start: utf16ToUtf8(oldText, change.from),
    end: utf16ToUtf8(oldText, change.to),
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
