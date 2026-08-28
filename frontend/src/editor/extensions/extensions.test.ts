// @vitest-environment jsdom
import {
  EditorSelection,
  EditorState,
  type Extension,
} from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { foldable } from "@codemirror/language";
import { describe, expect, it } from "vitest";

import { applyEnterRule, behaviorExtensions } from "./behavior";
import {
  decorationExtension,
  replaceDecorations,
  showInlays,
} from "./decorations";
import { foldingExtension, installFolds } from "./folding";
import { interactionKeymaps, insertAtSelections } from "./keymaps";
import type { DecorationSet, FoldingRangeSet } from "./types";

function view(doc: string, extensions: Extension = []) {
  return new EditorView({
    state: EditorState.create({ doc, extensions }),
    parent: document.body,
  });
}

const provenance = {
  packageName: "@clay/test",
  packageVersion: "1",
  packagePrefix: "test",
};

function decorations(spans: DecorationSet["spans"]): DecorationSet {
  return {
    documentId: 1,
    documentVersion: 1,
    packagePrefix: "test",
    kind: spans[0]?.kind ?? "syntax",
    viewportByteStart: 0,
    viewportByteEnd: 100,
    spans,
  };
}

describe("editor parity adapters", () => {
  it("projects syntax, links, and toggleable inlays without changing text", () => {
    const editor = view("const value = 1", [decorationExtension]);
    editor.dispatch({
      effects: replaceDecorations(
        editor.state,
        decorations([
          {
            byteStart: 0,
            byteEnd: 5,
            kind: "syntax",
            tokenType: "keyword",
            modifiers: 0,
            scope: null,
            fontRole: null,
            priority: 1,
            provenance,
            target: null,
            inlay: null,
          },
          {
            byteStart: 6,
            byteEnd: 11,
            kind: "link",
            tokenType: "link",
            modifiers: 4096,
            scope: null,
            fontRole: null,
            priority: 1,
            provenance,
            target: { displayOnly: { text: "value" } },
            inlay: null,
          },
          {
            byteStart: 11,
            byteEnd: 11,
            kind: "inlayHint",
            tokenType: "type",
            modifiers: 0,
            scope: null,
            fontRole: null,
            priority: 1,
            provenance,
            target: null,
            inlay: { label: ": number", placement: "after" },
          },
        ]),
      ),
    });
    expect(editor.dom.querySelector(".cm-clay-link")?.textContent).toBe(
      "value",
    );
    expect(
      (editor.dom.querySelector(".cm-clay-inlay") as HTMLElement | null)
        ?.dataset.label,
    ).toBe(": number");
    editor.dispatch({ effects: showInlays(false) });
    expect(editor.dom.querySelector(".cm-clay-inlay")).toBeNull();
    expect(editor.state.doc.toString()).toBe("const value = 1");
    editor.destroy();
  });

  it("exposes validated server folds through CodeMirror fold service", () => {
    const editor = view("fn main() {\n  work();\n}\n", [foldingExtension]);
    const set: FoldingRangeSet = {
      documentId: 1,
      documentVersion: 1,
      packagePrefix: "core",
      ranges: [{ byteStart: 0, byteEnd: 23, label: "function", provenance }],
    };
    editor.dispatch({ effects: installFolds(editor.state, set) });
    const line = editor.state.doc.line(1);
    expect(foldable(editor.state, line.from, line.to)).toEqual({
      from: line.to,
      to: 23,
    });
    editor.destroy();
  });

  it("applies declarative list continuation and tab policy", () => {
    const editor = view("- item", [
      behaviorExtensions({
        behaviorVersion: 1,
        documentFontRole: "proportional",
        editorRules: {
          enter: {
            continueLineMarkers: {
              markers: ["-", "ordered-dot"],
              exitOnEmptyItem: true,
            },
          },
          tab: { mode: "insertSpaces", spacesPerTab: 2 },
        },
      }),
    ]);
    editor.dispatch({ selection: { anchor: 6 } });
    applyEnterRule(editor, {
      continueLineMarkers: {
        markers: ["-", "ordered-dot"],
        exitOnEmptyItem: true,
      },
    });
    insertAtSelections(editor, "  ");
    expect(editor.state.doc.toString()).toBe("- item\n-   ");
    editor.destroy();
  });

  it("routes manifest multi-stroke chords without inserting text", () => {
    const commands: string[] = [];
    const editor = view("safe", [
      behaviorExtensions(
        {
          behaviorVersion: 1,
          keymaps: [
            {
              commandId: "controlCenter.open",
              context: "global",
              routingPolicy: "serverFirst",
              sequence: [
                {
                  key: { character: "x" },
                  modifiers: {
                    control: true,
                    shift: false,
                    alt: false,
                    superKey: false,
                  },
                },
                {
                  key: { character: "p" },
                  modifiers: {
                    control: true,
                    shift: false,
                    alt: false,
                    superKey: false,
                  },
                },
              ],
            },
          ],
        },
        (command) => {
          commands.push(command);
          return true;
        },
      ),
    ]);
    editor.contentDOM.dispatchEvent(
      new KeyboardEvent("keydown", { key: "x", ctrlKey: true, bubbles: true }),
    );
    editor.contentDOM.dispatchEvent(
      new KeyboardEvent("keydown", { key: "p", ctrlKey: true, bubbles: true }),
    );
    expect(commands).toEqual(["controlCenter.open"]);
    expect(editor.state.doc.toString()).toBe("safe");
    editor.destroy();
  });

  it("keeps multi-selection edits atomic in CodeMirror", () => {
    const editor = view("one\ntwo", [interactionKeymaps]);
    editor.dispatch({
      selection: EditorSelection.create([
        EditorSelection.cursor(0),
        EditorSelection.cursor(4),
      ]),
    });
    insertAtSelections(editor, "x");
    expect(editor.state.doc.toString()).toBe("xone\nxtwo");
    expect(editor.state.selection.ranges).toHaveLength(2);
    editor.destroy();
  });
});
