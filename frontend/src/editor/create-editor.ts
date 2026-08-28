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
import { bytePositionField, type BytePositionIndex } from "./position-index";
import { shouldEmitEdit } from "./transactions";
import {
  editorPerformance,
  PERFORMANCE_STAGE,
  type PerformanceSpan,
} from "./performance";
import type { TextChange } from "./sync/operations";

export interface CreateEditorOptions {
  doc?: string | Text;
  readOnly?: boolean;
  parent: HTMLElement;
  placeholder?: string;
  documentId?: number;
  version?: number;
  /** Receives the pre-change document (rope) — never a flattened string. */
  onUserChanges?: (
    oldDoc: Text,
    changes: TextChange[],
    traceId?: number,
    /** Position index of the pre-change state (shared hot-path field). */
    index?: BytePositionIndex,
  ) => void;
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

  let pendingInput: { traceId: number; span: PerformanceSpan } | null = null;
  const inputEvents = EditorView.domEventHandlers({
    beforeinput: () => {
      const traceId = editorPerformance.trace();
      pendingInput = {
        traceId,
        span: editorPerformance.span(PERFORMANCE_STAGE.browserInput, traceId, {
          documentId: options.documentId,
          version: options.version,
        }),
      };
      return false;
    },
  });
  const listener = EditorView.updateListener.of((update) => {
    if (!update.docChanged) return;
    const input = pendingInput;
    pendingInput = null;
    const traceId = input?.traceId || editorPerformance.trace();
    if (!input) {
      editorPerformance.mark(PERFORMANCE_STAGE.browserInput, traceId, {
        documentId: options.documentId,
        version: options.version,
      });
    }
    input?.span.end();
    editorPerformance.mark(PERFORMANCE_STAGE.codemirrorUpdate, traceId, {
      documentId: options.documentId,
      version: options.version,
    });
    editorPerformance.frame(traceId, {
      documentId: options.documentId,
      version: options.version,
    });
    const emit = options.onUserChanges;
    if (!emit) return;
    for (const transaction of update.transactions) {
      if (!shouldEmitEdit(transaction)) continue;
      // Pass the rope itself; byte-offset conversion is indexed per document
      // version. Flattening here cost O(document) on every keystroke.
      emit(
        transaction.startState.doc,
        collectChanges(transaction),
        traceId,
        transaction.startState.field(bytePositionField, false) ?? undefined,
      );
    }
  });

  return new EditorView({
    parent: options.parent,
    state: EditorState.create({
      doc: options.doc ?? "",
      extensions: [
        history(),
        // First so every consumer reads one shared incremental index.
        bytePositionField,
        keymapCompartment.of(keymap.of([...defaultKeymap, ...historyKeymap])),
        saveKey,
        readOnlyCompartment.of(EditorState.readOnly.of(!!options.readOnly)),
        themeCompartment.of(clayEditorTheme),
        languageCompartment.of([]),
        behaviorCompartment.of([]),
        decorationCompartment.of([]),
        placeholderExt(options.placeholder ?? ""),
        inputEvents,
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
  editorPerformance.count(PERFORMANCE_STAGE.compartmentReconfigure, 0, {
    feature: "readOnly",
  });
  view.dispatch({
    effects: readOnlyCompartment.reconfigure(EditorState.readOnly.of(readOnly)),
  });
}

export function setTheme(view: EditorView, extension: Extension): void {
  editorPerformance.count(PERFORMANCE_STAGE.compartmentReconfigure, 0, {
    feature: "theme",
  });
  view.dispatch({
    effects: themeCompartment.reconfigure(extension),
  });
}
