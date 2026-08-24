// @vitest-environment jsdom
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { describe, expect, it } from "vitest";

import { decorationExtension, replaceDecorations } from "./decorations";
import type { DecorationSet } from "./types";

const provenance = {
  packageName: "core",
  packageVersion: "builtin",
  packagePrefix: "core",
};

// Advisory JS-host budgets. Rust protocol/cache limits remain authoritative.
describe("editor projection performance budgets", () => {
  it("keeps a 1 MiB document local typing path responsive", () => {
    const text = "0123456789abcdef\n".repeat(61_681);
    const started = performance.now();
    const view = new EditorView({
      parent: document.body,
      state: EditorState.create({ doc: text }),
    });
    const mountedMs = performance.now() - started;
    const typed = performance.now();
    view.dispatch({ changes: { from: 0, insert: "x" } });
    const typingMs = performance.now() - typed;
    expect(mountedMs).toBeLessThan(2_000);
    expect(typingMs).toBeLessThan(100);
    view.destroy();
  });

  it("projects a bounded 1,000-span viewport without full-document work", () => {
    const text = "let value = 1;\n".repeat(2_000);
    const view = new EditorView({
      parent: document.body,
      state: EditorState.create({ doc: text, extensions: decorationExtension }),
    });
    const spans: DecorationSet["spans"] = Array.from(
      { length: 1_000 },
      (_, index) => ({
        byteStart: index * 15,
        byteEnd: index * 15 + 3,
        kind: "syntax" as const,
        tokenType: "keyword" as const,
        modifiers: 0,
        scope: null,
        fontRole: null,
        priority: 1,
        provenance,
        target: null,
        inlay: null,
      }),
    );
    const started = performance.now();
    view.dispatch({
      effects: replaceDecorations({
        documentId: 1,
        documentVersion: 1,
        packagePrefix: "core",
        kind: "syntax",
        viewportByteStart: 0,
        viewportByteEnd: text.length,
        spans,
      }),
    });
    expect(performance.now() - started).toBeLessThan(500);
    expect(view.state.doc.length).toBe(text.length);
    view.destroy();
  });
});
