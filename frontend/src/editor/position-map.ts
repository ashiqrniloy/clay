// UTF-16 (CodeMirror / JS string) ↔ UTF-8 (Clay rope / protocol) offsets.
// Mirrors `src/editor/position_map.rs`. Offsets inside a multi-unit scalar
// snap to that scalar's start so the result is always a UTF-8 boundary.

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
// The linear scans above are O(document) per call. The editor calls them per
// keystroke and per decoration span, which froze large files. `textIndex`
// builds one line-start table per document version (CodeMirror `Text` is
// immutable, so a WeakMap keyed on it memoizes safely) and every conversion
// becomes a binary search plus a scan of one line.

interface LineSource {
  readonly lines: number;
  readonly length: number;
  line(number: number): { from: number; text: string };
}

export interface TextIndex {
  readonly utf16Starts: readonly number[];
  readonly utf8Starts: readonly number[];
  readonly lineUtf8Lengths: readonly number[];
  readonly totalUtf16: number;
  readonly totalUtf8: number;
}

const indexCache = new WeakMap<LineSource, TextIndex>();

/** Memoized line-start index for a CodeMirror document (or test fake). */
export function textIndex(doc: LineSource): TextIndex {
  const cached = indexCache.get(doc);
  if (cached) return cached;
  const utf16Starts: number[] = [];
  const utf8Starts: number[] = [];
  const lineUtf8Lengths: number[] = [];
  const lineTexts: string[] = [];
  let utf8 = 0;
  for (let n = 1; n <= doc.lines; n += 1) {
    const line = doc.line(n);
    utf16Starts.push(line.from);
    utf8Starts.push(utf8);
    lineTexts.push(line.text);
    const width = encoderByteLength(line.text);
    lineUtf8Lengths.push(width);
    // One "\n" between consecutive lines (Clay never reconfigures
    // state.lineBreak): +1 UTF-16 unit and +1 UTF-8 byte.
    utf8 += width + 1;
  }
  const index: TextIndex = {
    utf16Starts,
    utf8Starts,
    lineUtf8Lengths,
    totalUtf16: doc.length,
    totalUtf8: Math.max(0, utf8 - 1),
  };
  indexCache.set(doc, index);
  indexLineTextCache.set(index, lineTexts);
  return index;
}

function lineForUtf16(index: TextIndex, utf16: number): number {
  let low = 0;
  let high = index.utf16Starts.length - 1;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if (at(index.utf16Starts, mid) <= utf16) low = mid;
    else high = mid - 1;
  }
  return low;
}

function lineForUtf8(index: TextIndex, utf8: number): number {
  let low = 0;
  let high = index.utf8Starts.length - 1;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if (at(index.utf8Starts, mid) <= utf8) low = mid;
    else high = mid - 1;
  }
  return low;
}

/** UTF-16 → UTF-8 via the memoized index (O(log lines + line length)). */
export function utf16ToUtf8Indexed(index: TextIndex, utf16: number): number {
  const offset = Math.max(0, Math.min(utf16, index.totalUtf16));
  const line = lineForUtf16(index, offset);
  const intra = offset - at(index.utf16Starts, line);
  if (intra === 0) return at(index.utf8Starts, line);
  let seen8 = 0;
  let seen16 = 0;
  for (const character of lineText(index, line)) {
    const code = character.codePointAt(0) ?? 0;
    const width = utf16Width(code);
    // Snap down to the containing scalar's start, mirroring the linear
    // reference: offsets inside a multi-unit scalar are boundary-unsafe.
    if (intra < seen16 + width) break;
    seen16 += width;
    seen8 += utf8Width(code);
  }
  const base = at(index.utf8Starts, line);
  return Math.min(base + seen8, base + at(index.lineUtf8Lengths, line));
}

/** UTF-8 → UTF-16 via the memoized index (O(log lines + line length)). */
export function utf8ToUtf16Indexed(index: TextIndex, utf8: number): number {
  const offset = Math.max(0, Math.min(utf8, index.totalUtf8));
  const line = lineForUtf8(index, offset);
  const intra = offset - at(index.utf8Starts, line);
  if (intra === 0) return at(index.utf16Starts, line);
  const text = lineText(index, line);
  let seen8 = 0;
  let seen16 = 0;
  for (const character of text) {
    const code = character.codePointAt(0) ?? 0;
    const width = utf8Width(code);
    // Mirror the linear snap-down to the containing scalar.
    if (intra < seen8 + width) break;
    seen8 += width;
    seen16 += utf16Width(code);
  }
  const base = at(index.utf16Starts, line);
  return Math.min(base + seen16, base + text.length);
}

const indexLineTextCache = new WeakMap<TextIndex, readonly string[]>();

function lineText(index: TextIndex, line: number): string {
  return indexLineTextCache.get(index)?.[line] ?? "";
}

function at(values: readonly number[], i: number): number {
  return values[i] ?? 0;
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
