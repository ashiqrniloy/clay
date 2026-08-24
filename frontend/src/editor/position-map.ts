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
