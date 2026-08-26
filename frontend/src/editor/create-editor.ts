import {
  EditorState,
  type Extension,
  type Text,
  type Transaction,
} from "@codemirror/state";
import {
  EditorView,
  keymap,
  placeholder as placeholderExt,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

import {
  behaviorCompartment,
  decorationCompartment,
  keymapCompartment,
  languageCompartment,
  readOnlyCompartment,
  themeCompartment,
} from "./compartments";
import { shouldEmitEdit } from "./transactions";
import type { TextChange } from "./sync/operations";

export interface CreateEditorOptions {
  doc?: string | Text;
  readOnly?: boolean;
  parent: HTMLElement;
  placeholder?: string;
  /** Receives the pre-change document (rope) — never a flattened string. */
  onUserChanges?: (oldDoc: Text, changes: TextChange[]) => void;
  onSave?: () => void;
  extra?: Extension[];
}

export const clayEditorTheme = EditorView.theme({
  "&": {
    height: "100%",
    backgroundColor: "var(--clay-surface-main)",
    color: "var(--clay-text-primary)",
    fontFamily: "var(--clay-font-monospace)",
    fontSize: "var(--clay-text-body-size)",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { overflow: "auto", fontFamily: "inherit" },
  ".cm-content": {
    caretColor: "var(--clay-accent-primary)",
    padding: "var(--clay-spacing-sm, 12px)",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--clay-accent-primary)",
  },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
    backgroundColor: "var(--clay-surface-selected) !important",
  },
  ".cm-activeLine": { backgroundColor: "transparent" },
});

export function createEditor(options: CreateEditorOptions): EditorView {
  const saveKey = keymap.of([
    {
      key: "Mod-s",
      preventDefault: true,
      run: () => {
        options.onSave?.();
        return true;
      },
    },
  ]);

  const listener = EditorView.updateListener.of((update) => {
    if (!update.docChanged) return;
    const emit = options.onUserChanges;
    if (!emit) return;
    for (const transaction of update.transactions) {
      if (!shouldEmitEdit(transaction)) continue;
      // Pass the rope itself; byte-offset conversion is indexed per document
      // version. Flattening here cost O(document) on every keystroke.
      emit(transaction.startState.doc, collectChanges(transaction));
    }
  });

  return new EditorView({
    parent: options.parent,
    state: EditorState.create({
      doc: options.doc ?? "",
      extensions: [
        history(),
        keymapCompartment.of(keymap.of([...defaultKeymap, ...historyKeymap])),
        saveKey,
        readOnlyCompartment.of(EditorState.readOnly.of(!!options.readOnly)),
        themeCompartment.of(clayEditorTheme),
        languageCompartment.of([]),
        behaviorCompartment.of([]),
        decorationCompartment.of([]),
        placeholderExt(options.placeholder ?? ""),
        listener,
        EditorView.lineWrapping,
        ...(options.extra ?? []),
      ],
    }),
  });
}

export function collectChanges(transaction: Transaction): TextChange[] {
  const changes: TextChange[] = [];
  transaction.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    changes.push({ from: fromA, to: toA, insert: inserted.toString() });
  });
  return changes;
}

export function setReadOnly(view: EditorView, readOnly: boolean): void {
  view.dispatch({
    effects: readOnlyCompartment.reconfigure(EditorState.readOnly.of(readOnly)),
  });
}

export function setTheme(view: EditorView, extension: Extension): void {
  view.dispatch({
    effects: themeCompartment.reconfigure(extension),
  });
}
