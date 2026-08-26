import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";

import { changesToOperations } from "./operations";

const doc = (text: string) => EditorState.create({ doc: text }).doc;

describe("changesToOperations", () => {
  it("maps a caret insert to a byte Insert", () => {
    expect(
      changesToOperations(doc("ab"), [{ from: 1, to: 1, insert: "x" }]),
    ).toEqual([{ insert: { byteOffset: 1, text: "x" } }]);
  });

  it("maps a deletion to a byte Delete", () => {
    expect(
      changesToOperations(doc("abcd"), [{ from: 1, to: 3, insert: "" }]),
    ).toEqual([{ delete: { start: 1, end: 3 } }]);
  });

  it("maps a replacement and uses UTF-8 offsets for emoji", () => {
    expect(
      changesToOperations(doc("a😀b"), [{ from: 1, to: 3, insert: "X" }]),
    ).toEqual([{ replace: { start: 1, end: 5, text: "X" } }]);
  });

  it("remaps later changes after an earlier insert", () => {
    expect(
      changesToOperations(doc("abcd"), [
        { from: 1, to: 1, insert: "XY" },
        { from: 3, to: 4, insert: "" },
      ]),
    ).toEqual([
      { insert: { byteOffset: 1, text: "XY" } },
      { delete: { start: 5, end: 6 } },
    ]);
  });
});
