import { EditorState, Text } from "@codemirror/state";
import { describe, expect, it } from "vitest";

import {
  POSITION_MAP_VECTORS,
  utf16ToUtf8,
  utf16ToUtf8Indexed,
  utf8Length,
  utf8ToUtf16,
  utf8ToUtf16Batch,
  utf8ToUtf16Indexed,
} from "./position-map";
import {
  bytePositionField,
  buildPositionIndex,
  positionIndex,
  positionIndexStats,
} from "./position-index";

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

// The incremental index must agree with the linear reference everywhere:
// every shared vector, every offset, after every edit of a random sequence.
describe("incremental position index", () => {
  const build = (text: string) => buildPositionIndex(Text.of(text.split("\n")));

  it("matches the linear conversion on every shared vector", () => {
    for (const [text] of POSITION_MAP_VECTORS) {
      const index = build(text);
      for (let utf16 = 0; utf16 <= text.length; utf16 += 1)
        expect(utf16ToUtf8Indexed(index, utf16)).toBe(utf16ToUtf8(text, utf16));
      const totalUtf8 = utf16ToUtf8(text, text.length);
      for (let utf8 = 0; utf8 <= totalUtf8; utf8 += 1)
        expect(utf8ToUtf16Indexed(index, utf8)).toBe(utf8ToUtf16(text, utf8));
      // The dense-patch batch converter must agree per offset, in any
      // input order (it sorts internally and resumes a per-line cursor).
      const offsets = Array.from(
        { length: totalUtf8 + 1 },
        (_, offset) => offset,
      ).reverse();
      expect(utf8ToUtf16Batch(index, offsets)).toEqual(
        offsets.map((offset) => utf8ToUtf16Indexed(index, offset)),
      );
    }
  });

  it("handles multi-line documents with astral characters", () => {
    const text = "a😀\nbc\u0301d\n\ne\u{10FFFD}f";
    const index = build(text);
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

  it("equals a fresh rebuild after random edit sequences", () => {
    let rng = 12345;
    const random = (max: number) => {
      rng = (rng * 1103515245 + 12345) & 0x7fffffff;
      return rng % max;
    };
    const pieces = [
      "a",
      "é",
      "😀",
      "e\u0301",
      "\n",
      "\r\n",
      "𐍈",
      "ab\n",
      "",
      "xyz",
    ];
    for (let round = 0; round < 8; round += 1) {
      let state = EditorState.create({
        doc: Text.of(
          // More than two chunks (64 lines each) so tree building and
          // multi-node splits/joins are exercised, not just single chunks.
          ("seed line 😀\n".repeat(150) + "tail\r\n").split("\n"),
        ),
        extensions: [bytePositionField],
      });
      for (let edit = 0; edit < 25; edit += 1) {
        const doc = state.doc;
        const from = random(doc.length + 1);
        const to = Math.min(doc.length, from + random(30));
        const insert =
          (pieces[random(pieces.length)] ?? "") +
          (pieces[random(pieces.length)] ?? "");
        state = state.update({ changes: { from, to, insert } }).state;
        const text = state.doc.toString();
        const totalUtf8 = utf16ToUtf8(text, text.length);
        const incremental = positionIndex(state);
        const fresh = buildPositionIndex(state.doc);
        expect(incremental.totalUtf16).toBe(text.length);
        expect(incremental.totalUtf8).toBe(totalUtf8);
        for (let utf16 = 0; utf16 <= Math.min(text.length, 400); utf16 += 1) {
          const expected = utf16ToUtf8(text, utf16);
          expect(utf16ToUtf8Indexed(incremental, utf16)).toBe(expected);
          expect(utf16ToUtf8Indexed(fresh, utf16)).toBe(expected);
        }
        for (let utf8 = 0; utf8 <= Math.min(totalUtf8, 400); utf8 += 1) {
          const expected = utf8ToUtf16(text, utf8);
          expect(utf8ToUtf16Indexed(incremental, utf8)).toBe(expected);
          expect(utf8ToUtf16Indexed(fresh, utf8)).toBe(expected);
        }
      }
    }
  });

  it("updates a one-character edit in bounded work independent of size", () => {
    const editWork = (size: number) => {
      const state = EditorState.create({
        doc: Text.of("short line\n".repeat(size).split("\n")),
        extensions: [bytePositionField],
      });
      const before = positionIndexStats(positionIndex(state));
      const next = state.update({
        changes: { from: 3, insert: "x" },
      }).state;
      const after = positionIndexStats(positionIndex(next));
      return after.nodes - before.nodes;
    };
    const small = editWork(2_000);
    const large = editWork(200_000);
    // A full rebuild would add nodes proportional to document size; an
    // incremental edit adds only the touched chunk plus a tree path.
    expect(large).toBeLessThanOrEqual(200);
    expect(large).toBe(small);
  });

  it("shares structure across history states instead of copying tables", () => {
    let state = EditorState.create({
      doc: Text.of("history line 😀\n".repeat(20_000).split("\n")),
      extensions: [bytePositionField],
    });
    const before = positionIndexStats(positionIndex(state));
    for (let i = 0; i < 256; i += 1) {
      state = state.update({
        changes: { from: (i * 31) % state.doc.length, insert: "é" },
      }).state;
    }
    const after = positionIndexStats(positionIndex(state));
    // 256 edits × (touched chunk + O(log) path) — not 256 document-sized
    // tables (~before.nodes each).
    expect(after.nodes).toBeLessThanOrEqual(before.nodes + 256 * 250);
    expect(after.chunks).toBeLessThanOrEqual(before.chunks + 256 * 4);
  });

  it("keeps a 1 MiB single line editable with reported conversion cost", () => {
    const line = "x".repeat(1024 * 1024);
    const state = EditorState.create({
      doc: Text.of([line]),
      extensions: [bytePositionField],
    });
    const index = positionIndex(state);
    const started = performance.now();
    for (let i = 0; i < 200; i += 1) {
      utf16ToUtf8Indexed(index, (i * 5000) % line.length);
      utf8ToUtf16Indexed(index, (i * 5000) % line.length);
    }
    const perConversion = (performance.now() - started) / 400;
    // ponytail: advisory ceiling; long-line intra-scan is O(line) by design
    // (same as the previous index), reported so a future line-segment split
    // has a measured baseline.
    expect(perConversion).toBeLessThan(5);
  });
});
