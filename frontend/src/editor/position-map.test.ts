import { describe, expect, it } from "vitest";

import {
  POSITION_MAP_VECTORS,
  textIndex,
  utf16ToUtf8,
  utf16ToUtf8Indexed,
  utf8Length,
  utf8ToUtf16,
  utf8ToUtf16Indexed,
} from "./position-map";

describe("utf16 ↔ utf8 position map", () => {
  it("matches the Rust golden vectors", () => {
    for (const [text, utf16, utf8] of POSITION_MAP_VECTORS) {
      expect(
        utf16ToUtf8(text, utf16),
        `${JSON.stringify(text)} @${utf16}`,
      ).toBe(utf8);
      expect(
        utf8ToUtf16(text, utf8),
        `${JSON.stringify(text)} utf8@${utf8}`,
      ).toBe(utf16);
    }
  });

  it("snaps mid-surrogate and mid-code-unit offsets to the scalar start", () => {
    expect(utf16ToUtf8("a😀b", 2)).toBe(1);
    expect(utf8ToUtf16("héllo", 2)).toBe(1);
  });

  it("clamps out of range to the document end", () => {
    expect(utf16ToUtf8("ab", 99)).toBe(2);
    expect(utf8ToUtf16("ab", 99)).toBe(2);
  });

  it("agrees with TextEncoder on random BMP + emoji + CRLF strings", () => {
    const samples = [
      "plain",
      "café",
      "a😀b😀c",
      "line\r\nbreak",
      "e\u0301x",
      "𐍈𐍈",
    ];
    for (const text of samples) {
      const encoded = new TextEncoder().encode(text);
      expect(utf8Length(text)).toBe(encoded.length);
      expect(utf16ToUtf8(text, text.length)).toBe(encoded.length);
      expect(utf8ToUtf16(text, encoded.length)).toBe(text.length);
    }
  });
});

// Indexed conversions must agree with the linear reference on every vector,
// including multi-line documents, emoji, and combining marks.
describe("text index", () => {
  const doc = (text: string) => {
    const lines = text.split("\n");
    let utf16 = 0;
    return {
      lines: lines.length,
      length: text.length,
      line(n: number) {
        const from = utf16;
        const body = lines[n - 1] ?? "";
        utf16 += body.length + 1;
        return { from, text: body };
      },
    };
  };

  it("matches the linear conversion on every shared vector", () => {
    for (const [text] of POSITION_MAP_VECTORS) {
      const index = textIndex(doc(text));
      for (let utf16 = 0; utf16 <= text.length; utf16 += 1)
        expect(utf16ToUtf8Indexed(index, utf16)).toBe(utf16ToUtf8(text, utf16));
      const totalUtf8 = utf16ToUtf8(text, text.length);
      for (let utf8 = 0; utf8 <= totalUtf8; utf8 += 1)
        expect(utf8ToUtf16Indexed(index, utf8)).toBe(utf8ToUtf16(text, utf8));
    }
  });

  it("handles multi-line documents with astral characters", () => {
    const text = "a😀\nbc\u0301d\n\ne\u{10FFFD}f";
    const index = textIndex(doc(text));
    expect(index.totalUtf8).toBe(utf16ToUtf8(text, text.length));
    expect(index.totalUtf16).toBe(text.length);
    for (let utf16 = 0; utf16 <= text.length; utf16 += 1) {
      const utf8 = utf16ToUtf8Indexed(index, utf16);
      expect(
        utf16ToUtf8Indexed(index, Math.min(utf16 + 1, text.length)),
      ).toBeGreaterThanOrEqual(utf8);
      expect(utf8ToUtf16Indexed(index, utf8)).toBeLessThanOrEqual(utf16);
    }
  });
});
