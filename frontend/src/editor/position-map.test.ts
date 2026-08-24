import { describe, expect, it } from "vitest";

import {
  POSITION_MAP_VECTORS,
  utf16ToUtf8,
  utf8Length,
  utf8ToUtf16,
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
