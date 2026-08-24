import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

/** Native editable semantics stay on CodeMirror's contenteditable surface. */
export const accessibilityExtension: Extension = [
  EditorView.contentAttributes.of({
    "aria-label": "Document editor",
    "aria-multiline": "true",
    spellcheck: "false",
  }),
];
