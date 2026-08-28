// UTF-16 (CodeMirror / JS string) ↔ UTF-8 (Clay rope / protocol) offsets.
// Mirrors `src/editor/position_map.rs`. Offsets inside a multi-unit scalar
// snap to that scalar's start so the result is always a UTF-8 boundary.

import {
  locateLine16,
  locateLine8,
  type BytePositionIndex,
} from "./position-index";

function utf16Width(codePoint: number): number {
  return codePoint > 0xffff ? 2 : 1;
}

function utf8Width(codePoint: number): number {
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

// ---------------------------------------------------------------------------
// Indexed conversion (hot path).
//
// Conversions take the shared incremental `BytePositionIndex` (see
// `position-index.ts`): one tree descent to the containing line, then a scan
// of that single line's text, read straight from the immutable document —
// no retained line-string copies.

/** UTF-16 → UTF-8 via the incremental index (O(log lines + line length)). */
export function utf16ToUtf8Indexed(
  index: BytePositionIndex,
  utf16: number,
): number {
  const offset = Math.max(0, Math.min(utf16, index.totalUtf16));
  if (offset >= index.totalUtf16) return index.totalUtf8;
  const located = locateLine16(index, offset);
  if (located.intra16 === 0) return located.start8;
  const text = index.doc.line(located.line + 1).text;
  let seen8 = 0;
  let seen16 = 0;
  for (const character of text) {
    const code = character.codePointAt(0) ?? 0;
    const width = utf16Width(code);
    // Snap down to the containing scalar's start, mirroring the linear
    // reference: offsets inside a multi-unit scalar are boundary-unsafe.
    if (located.intra16 < seen16 + width) break;
    seen16 += width;
    seen8 += utf8Width(code);
  }
  return located.start8 + seen8;
}

/** UTF-8 → UTF-16 via the incremental index (O(log lines + line length)). */
export function utf8ToUtf16Indexed(
  index: BytePositionIndex,
  utf8: number,
): number {
  return utf8ToUtf16Batch(index, [utf8])[0] ?? 0;
}

/**
 * Batch UTF-8 → UTF-16 conversion for dense patches. Offsets are processed
 * in sorted order with a resumable per-line cursor, so a whole span list
 * costs one scan per line instead of one scan per span — the difference
 * between milliseconds and seconds on long lines. Results are identical to
 * `utf8ToUtf16Indexed` for every offset, including snap-down inside
 * multi-byte scalars.
 */
export function utf8ToUtf16Batch(
  index: BytePositionIndex,
  offsets: readonly number[],
): number[] {
  const results = new Array<number>(offsets.length);
  const order = offsets
    .map((value, position) => ({ value, position }))
    .sort((left, right) => left.value - right.value);
  let line = -1;
  let text = "";
  let charIndex = 0;
  let seen8 = 0;
  let seen16 = 0;
  for (const { value, position } of order) {
    const offset = Math.max(0, Math.min(value, index.totalUtf8));
    if (offset >= index.totalUtf8) {
      results[position] = index.totalUtf16;
      continue;
    }
    const located = locateLine8(index, offset);
    if (located.line !== line) {
      line = located.line;
      text = index.doc.line(line + 1).text;
      charIndex = 0;
      seen8 = 0;
      seen16 = 0;
    }
    while (charIndex < text.length) {
      if (seen8 >= located.intra8) break;
      const code = text.codePointAt(charIndex) ?? 0;
      const width = utf8Width(code);
      // Mirror the linear snap-down to the containing scalar.
      if (located.intra8 < seen8 + width) break;
      seen8 += width;
      seen16 += utf16Width(code);
      charIndex += code > 0xffff ? 2 : 1;
    }
    results[position] = located.start16 + seen16;
  }
  return results;
}

/** Convert a UTF-16 code-unit offset into a UTF-8 byte offset. */
export function utf16ToUtf8(text: string, utf16: number): number {
  let seenUtf16 = 0;
  let seenUtf8 = 0;
  for (const character of text) {
    const code = character.codePointAt(0) ?? 0;
    const width = utf16Width(code);
    if (utf16 < seenUtf16 + width) return seenUtf8;
    seenUtf16 += width;
    seenUtf8 += utf8Width(code);
    if (utf16 === seenUtf16) return seenUtf8;
  }
  return encoderByteLength(text);
}

/** Convert a UTF-8 byte offset into a UTF-16 code-unit offset. */
export function utf8ToUtf16(text: string, utf8: number): number {
  let seenUtf8 = 0;
  let seenUtf16 = 0;
  for (const character of text) {
    const code = character.codePointAt(0) ?? 0;
    const width = utf8Width(code);
    if (utf8 < seenUtf8 + width) return seenUtf16;
    seenUtf8 += width;
    seenUtf16 += utf16Width(code);
    if (utf8 === seenUtf8) return seenUtf16;
  }
  return seenUtf16;
}

function encoderByteLength(text: string): number {
  let bytes = 0;
  for (const character of text) {
    bytes += utf8Width(character.codePointAt(0) ?? 0);
  }
  return bytes;
}

/** UTF-8 byte length of `text`. */
export function utf8Length(text: string): number {
  return encoderByteLength(text);
}

/** Shared with `src/editor/position_map.rs`. */
export const POSITION_MAP_VECTORS: readonly [string, number, number][] = [
  ["", 0, 0],
  ["abc", 1, 1],
  ["abc", 3, 3],
  ["héllo", 2, 3],
  ["a😀b", 1, 1],
  ["a😀b", 3, 5],
  ["a😀b", 4, 6],
  ["e\u{0301}", 1, 1],
  ["e\u{0301}", 2, 3],
  ["a\r\nb", 2, 2],
  ["a\r\nb", 3, 3],
  ["𐍈", 0, 0],
  ["𐍈", 2, 4],
];
