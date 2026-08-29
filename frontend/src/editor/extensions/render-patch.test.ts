// @vitest-environment jsdom
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { foldable } from "@codemirror/language";
import { describe, expect, it } from "vitest";

import {
  decorationExtension,
  decorationPatch,
  decorationStats,
  replaceDecorations,
} from "./decorations";
import {
  diagnosticExtension,
  diagnosticPatch,
  visibleDiagnostics,
} from "./diagnostics";
import { foldingExtension, installFolds } from "./folding";
import type { DiagnosticItem } from "./render-patch";
import type {
  DecorationSet,
  DiagnosticSet,
  FoldingRangeSet,
  TokenType,
} from "./types";

const provenance = {
  packageName: "@clay/test",
  packageVersion: "1",
  packagePrefix: "test",
};

function mounted(doc: string, extensions: Extension[] = []) {
  return new EditorView({
    state: EditorState.create({ doc, extensions }),
    parent: document.body,
  });
}

function decorationSet(
  spans: Array<{ byteStart: number; byteEnd: number; tokenType?: TokenType }>,
  covered: [number, number],
  authority = "test",
): DecorationSet {
  return {
    documentId: 1,
    documentVersion: 1,
    packagePrefix: authority,
    kind: "syntax",
    viewportByteStart: covered[0],
    viewportByteEnd: covered[1],
    spans: spans.map((span) => ({
      byteStart: span.byteStart,
      byteEnd: span.byteEnd,
      kind: "syntax",
      tokenType: span.tokenType ?? "keyword",
      modifiers: 0,
      scope: null,
      fontRole: null,
      priority: 1,
      provenance,
      target: null,
      inlay: null,
    })),
  };
}

describe("atomic viewport render patches", () => {
  it("replaces exactly the covered range of one authority", () => {
    const editor = mounted("0123456789abcdefghij", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 4 }], [0, 10], "a"),
      ),
    });
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 12, byteEnd: 16 }], [10, 20], "a"),
      ),
    });
    // First covered range [0,10] kept its span; second added [12,16].
    expect(decorationStats(editor.state).marks).toBe(2);

    // A complete new patch for covered [0,10] replaces the old span there.
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 2, byteEnd: 5 }], [0, 10], "a"),
      ),
    });
    expect(decorationStats(editor.state).marks).toBe(2);
    const text = editor.state.doc.toString();
    const marked = editor.dom.querySelectorAll(".cm-clay-t-keyword");
    expect(
      [...marked].map((node) => text.indexOf(node.textContent ?? "")),
    ).toEqual([2, 12]);
    editor.destroy();
  });

  it("keeps every split fragment that claims only its own span range", () => {
    const editor = mounted("x".repeat(80), [decorationExtension]);
    editor.dispatch({
      effects: [
        decorationPatch(
          editor.state,
          decorationSet([{ byteStart: 0, byteEnd: 2 }], [0, 2]),
          false,
        ),
        decorationPatch(
          editor.state,
          decorationSet([{ byteStart: 60, byteEnd: 62 }], [60, 62]),
          false,
        ),
      ],
    });
    expect(decorationStats(editor.state).marks).toBe(2);
    editor.destroy();
  });

  it("clears only the covered range on an empty patch, keeping siblings", () => {
    const editor = mounted("x".repeat(64), [decorationExtension]);
    editor.dispatch({
      effects: [
        replaceDecorations(
          editor.state,
          decorationSet([{ byteStart: 0, byteEnd: 2 }], [0, 4], "a"),
        ),
        replaceDecorations(
          editor.state,
          decorationSet([{ byteStart: 30, byteEnd: 32 }], [30, 34], "a"),
        ),
        replaceDecorations(
          editor.state,
          decorationSet([{ byteStart: 0, byteEnd: 2 }], [0, 4], "b"),
        ),
      ],
    });
    expect(decorationStats(editor.state).marks).toBe(3);
    // Empty patch for authority a in [0,4]: clears a's span there only.
    editor.dispatch({
      effects: replaceDecorations(editor.state, decorationSet([], [0, 4], "a")),
    });
    expect(decorationStats(editor.state).marks).toBe(2);
    editor.destroy();
  });

  it("drops a mark that covered the token plus newline after backspace", () => {
    const editor = mounted("fn\nnext", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 3 }], [0, 8]),
      ),
    });
    expect(editor.dom.querySelector(".cm-clay-t-keyword")?.textContent).toBe(
      "fn",
    );
    editor.dispatch({ changes: { from: 1, to: 2 } });
    editor.dispatch({ changes: { from: 0, to: 1 } });
    expect(editor.state.doc.toString()).toBe("\nnext");
    expect(decorationStats(editor.state).marks).toBe(0);
    expect(editor.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    editor.destroy();
  });

  it("does not leave a highlighted space after backspacing a token", () => {
    const editor = mounted("fn \n", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 2 }], [0, 4]),
      ),
    });
    // Insert a space after the token the way typing does, then delete the token.
    editor.dispatch({
      changes: { from: 2, insert: "x" },
      selection: { anchor: 3 },
    });
    editor.dispatch({ changes: { from: 2, to: 3 } });
    editor.dispatch({ changes: { from: 1, to: 2 } });
    editor.dispatch({ changes: { from: 0, to: 1 } });
    expect(editor.state.doc.toString()).toBe(" \n");
    expect(decorationStats(editor.state).marks).toBe(0);
    expect(editor.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    editor.destroy();
  });

  it("does not grow a mark onto text inserted at its exclusive end", () => {
    const editor = mounted("fn\n", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 2 }], [0, 3]),
      ),
    });
    editor.dispatch({ changes: { from: 2, insert: " " } });
    expect(editor.dom.querySelector(".cm-clay-t-keyword")?.textContent).toBe(
      "fn",
    );
    editor.destroy();
  });

  it("drops an impl keyword after four backspaces, including the rest of the line", () => {
    const editor = mounted("impl Foo {\n", [decorationExtension]);
    editor.dispatch({
      selection: { anchor: 4 },
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 4 }], [0, 11]),
      ),
    });
    expect(editor.dom.querySelector(".cm-clay-t-keyword")?.textContent).toBe(
      "impl",
    );
    for (let i = 0; i < 4; i += 1)
      editor.dispatch({ changes: { from: 3 - i, to: 4 - i } });
    expect(editor.state.doc.toString()).toBe(" Foo {\n");
    expect(decorationStats(editor.state).marks).toBe(0);
    expect(editor.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    editor.destroy();
  });

  it("clears the final JavaScript keyword fragment when the document becomes empty", () => {
    const editor = mounted("function", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 8 }], [0, 8], "javascript"),
      ),
    });
    expect(editor.dom.querySelector(".cm-clay-t-keyword")?.textContent).toBe(
      "function",
    );
    for (let end = 8; end > 0; end -= 1)
      editor.dispatch({ changes: { from: end - 1, to: end } });
    expect(editor.state.doc.length).toBe(0);
    expect(decorationStats(editor.state).marks).toBe(0);
    expect(editor.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    editor.destroy();
  });

  it("drops a mark as soon as backspace overlaps it, without waiting for a patch", () => {
    const editor = mounted("hello\nnext", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 5 }], [0, 10]),
      ),
    });
    expect(editor.dom.querySelector(".cm-clay-t-keyword")?.textContent).toBe(
      "hello",
    );
    editor.dispatch({ changes: { from: 4, to: 5 } });
    expect(editor.state.doc.toString()).toBe("hell\nnext");
    expect(decorationStats(editor.state).marks).toBe(0);
    expect(editor.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    editor.destroy();
  });

  it("drops syntax marks that backspace deleted instead of sticking to the newline", () => {
    const editor = mounted("fn\nnext", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 0, byteEnd: 2 }], [0, 8]),
      ),
    });
    expect(decorationStats(editor.state).marks).toBe(1);
    editor.dispatch({ changes: { from: 1, to: 2 } });
    editor.dispatch({ changes: { from: 0, to: 1 } });
    expect(editor.state.doc.toString()).toBe("\nnext");
    expect(decorationStats(editor.state).marks).toBe(0);
    expect(editor.dom.querySelector(".cm-clay-t-keyword")).toBeNull();
    editor.destroy();
  });

  it("maps unaffected spans through local edits", () => {
    const editor = mounted("aaaa\nbbbb\ncccc", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorationSet([{ byteStart: 5, byteEnd: 9 }], [0, 20]),
      ),
    });
    editor.dispatch({ changes: { from: 0, insert: "xx\n" } });
    const marked = editor.dom.querySelector(".cm-clay-t-keyword");
    expect(marked?.textContent).toBe("bbbb");
    editor.destroy();
  });

  it("bounds retained spans across 100 consecutive viewport patches", () => {
    const blocks = 100;
    const block = "y".repeat(200);
    const text = Array.from({ length: blocks }, () => block).join("\n");
    const editor = mounted(text, [decorationExtension]);
    // One patch per viewport block: 20 spans each.
    for (let index = 0; index < blocks; index += 1) {
      const start = index * 201;
      const spans = Array.from({ length: 20 }, (_, i) => ({
        byteStart: start + i * 10,
        byteEnd: start + i * 10 + 5,
      }));
      editor.dispatch({
        effects: replaceDecorations(
          editor.state,
          decorationSet(spans, [start, start + 200]),
        ),
      });
    }
    // Guard = covered ± max(4096, covered): at most ~42 blocks of history.
    expect(decorationStats(editor.state).marks).toBeLessThanOrEqual(20 * 42);
    expect(decorationStats(editor.state).marks).toBeGreaterThanOrEqual(20);
    editor.destroy();
  });

  it("applies a dense 10,000-span patch in one transaction", () => {
    const text = "z".repeat(20_000);
    let transactions = 0;
    const state = EditorState.create({
      doc: text,
      extensions: [
        decorationExtension,
        EditorView.updateListener.of(() => {
          transactions += 1;
        }),
      ],
    });
    const spans = Array.from({ length: 10_000 }, (_, i) => ({
      byteStart: i * 2,
      byteEnd: i * 2 + 1,
    }));
    // State-update cost without DOM: the patch itself stays cheap.
    const started = performance.now();
    const effects = [
      replaceDecorations(state, decorationSet(spans, [0, 20_000])),
      replaceDecorations(
        state,
        decorationSet(
          [{ byteStart: 0, byteEnd: 1, tokenType: "string" as const }],
          [0, 20_000],
          "other",
        ),
      ),
    ];
    const patched = state.update({ effects });
    expect(performance.now() - started).toBeLessThan(500);
    const editor = new EditorView({
      parent: document.body,
      state: patched.state,
    });
    expect(decorationStats(editor.state).marks).toBe(10_001);
    expect(
      editor.dom.querySelectorAll(".cm-clay-t-keyword").length,
    ).toBeGreaterThan(0);
    expect(editor.dom.querySelector(".cm-clay-t-string")).not.toBeNull();
    // A follow-up patch lands as exactly one transaction.
    editor.dispatch({
      effects: [
        replaceDecorations(
          editor.state,
          decorationSet([{ byteStart: 0, byteEnd: 1 }], [0, 20_000], "other"),
        ),
      ],
    });
    expect(transactions).toBe(1);
    editor.destroy();
  });
});

describe("diagnostic suppression sweep", () => {
  const reference = (items: DiagnosticItem[]): DiagnosticItem[] => {
    const suppressors = items.filter(
      (item) => item.source !== "tree-sitter" && item.severity !== "info",
    );
    return items.filter(
      (item) =>
        item.source !== "tree-sitter" ||
        !suppressors.some(
          (other) => item.from < other.to && other.from < item.to,
        ),
    );
  };

  it("matches the reference nested algorithm on random overlaps", () => {
    let seed = 42;
    const random = (max: number) => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed % max;
    };
    for (let round = 0; round < 50; round += 1) {
      const items: DiagnosticItem[] = Array.from({ length: 60 }, (_, index) => {
        const from = random(200);
        return {
          from,
          to: from + random(30),
          authority: `a${index}`,
          severity:
            (["error", "warning", "info"] as const)[random(3)] ?? "error",
          message: "m",
          source: random(3) === 0 ? "tree-sitter" : `linter-${random(3)}`,
        };
      });
      expect(visibleDiagnostics(items)).toEqual(reference(items));
    }
  });

  it("replaces the covered range and keeps lint in sync in one transaction", () => {
    let transactions = 0;
    const editor = new EditorView({
      parent: document.body,
      state: EditorState.create({
        doc: "let value = 1;\nlet other = 2;\n",
        extensions: [
          diagnosticExtension,
          EditorView.updateListener.of(() => {
            transactions += 1;
          }),
        ],
      }),
    });
    const base = {
      documentId: 1,
      documentVersion: 1,
      source: "rust",
      provenance,
    };
    const span = (
      byteStart: number,
      byteEnd: number,
      severity: "error" | "warning" | "info",
      source: string,
      message: string,
    ) => ({
      byteStart,
      byteEnd,
      severity,
      code: "X",
      message,
      source,
      provenance,
    });
    editor.dispatch(
      diagnosticPatch(editor.state, {
        ...base,
        viewportByteStart: 0,
        viewportByteEnd: 30,
        spans: [
          span(4, 9, "error", "rust", "bad value"),
          span(19, 24, "info", "tree-sitter", "tree thing"),
        ],
      } satisfies DiagnosticSet),
    );
    // One dispatch, one transaction, both marks rendered.
    expect(transactions).toBe(1);
    expect(
      editor.dom.querySelector(".cm-clay-diagnostic-error")?.textContent,
    ).toBe("value");
    expect(
      editor.dom.querySelector(".cm-clay-diagnostic-info")?.textContent,
    ).toBe("other");

    // A wide suppressor in the same patch hides the tree-sitter info.
    editor.dispatch(
      diagnosticPatch(editor.state, {
        ...base,
        viewportByteStart: 0,
        viewportByteEnd: 30,
        spans: [
          span(4, 24, "warning", "rust", "wide warning"),
          span(19, 24, "info", "tree-sitter", "tree thing"),
        ],
      } satisfies DiagnosticSet),
    );
    expect(editor.dom.querySelector(".cm-clay-diagnostic-info")).toBeNull();
    // Marks crossing a line break render per line.
    expect(
      [...editor.dom.querySelectorAll(".cm-clay-diagnostic-warning")].map(
        (node) => node.textContent,
      ),
    ).toEqual(["value = 1;", "let other"]);
    editor.destroy();
  });
});

describe("sorted fold index", () => {
  it("looks up folds over 10,000 sorted ranges quickly", () => {
    const text = "data\n".repeat(10_000);
    const editor = mounted(text, [foldingExtension]);
    const ranges = Array.from({ length: 10_000 }, (_, index) => ({
      byteStart: index * 5,
      byteEnd: 5 * 10_000,
      label: null,
      provenance,
    }));
    const set: FoldingRangeSet = {
      documentId: 1,
      documentVersion: 1,
      packagePrefix: "test",
      ranges,
    };
    editor.dispatch({ effects: installFolds(editor.state, set) });
    const started = performance.now();
    for (let number = 1; number <= 10_000; number += 7) {
      const lineAt = editor.state.doc.line(number);
      foldable(editor.state, lineAt.from, lineAt.to);
    }
    expect(performance.now() - started).toBeLessThan(250);
    // Correctness: the fold on line 1 spans to the end of the document.
    const first = editor.state.doc.line(1);
    expect(foldable(editor.state, first.from, first.to)).toEqual({
      from: first.to,
      to: text.length,
    });
    editor.destroy();
  });
});
