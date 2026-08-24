import {
  EditorSelection,
  EditorState,
  type Extension,
} from "@codemirror/state";
import {
  addCursorAbove,
  addCursorBelow,
  selectLine,
  simplifySelection,
} from "@codemirror/commands";
import {
  selectNextOccurrence,
  selectSelectionMatches,
} from "@codemirror/search";
import {
  crosshairCursor,
  keymap,
  rectangularSelection,
} from "@codemirror/view";

export const interactionKeymaps: Extension = [
  EditorState.allowMultipleSelections.of(true),
  rectangularSelection(),
  crosshairCursor(),
  keymap.of([
    { key: "Ctrl-l", run: selectLine },
    { key: "Mod-d", run: selectNextOccurrence },
    { key: "Mod-Shift-l", run: selectSelectionMatches },
    { key: "Ctrl-Alt-ArrowUp", run: addCursorAbove },
    { key: "Ctrl-Alt-ArrowDown", run: addCursorBelow },
    { key: "Escape", run: simplifySelection },
  ]),
];

export function insertAtSelections(
  view: Parameters<typeof selectLine>[0],
  text: string,
): boolean {
  const changes = view.state.changeByRange((range) => ({
    changes: { from: range.from, to: range.to, insert: text },
    range: EditorSelection.cursor(range.from + text.length),
  }));
  view.dispatch(view.state.update(changes));
  return true;
}
