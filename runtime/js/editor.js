// Clay editor facade skeleton.
//
// This file defines user-facing APIs for the server-side JavaScript runtime.
// Cursor/selection facades validate typed args via Clay deno ops; the argless
// command-id facades return stable registry IDs for key binding (Plan 071 task 5).
const editorOps = globalThis.Deno?.core?.ops;
function requireEditorOps() {
    if (!editorOps) {
        throw new Error("clay.editor.runtime_unavailable: Clay editor APIs require the server runtime");
    }
    return editorOps;
}
function parseResult(json) {
    return JSON.parse(json);
}
export async function serverInsertText(options) {
    void options;
    throw new Error("clay.editor.serverInsertText is planned; Clay JS runtime op wiring is not implemented yet");
}
export async function serverDeleteRange(options) {
    void options;
    throw new Error("clay.editor.serverDeleteRange is planned; Clay JS runtime op wiring is not implemented yet");
}
export function clientMoveCursor(options) {
    return parseResult(requireEditorOps().op_clay_editor_move_cursor(JSON.stringify(options ?? {})));
}
export function clientSetSelection(options) {
    return parseResult(requireEditorOps().op_clay_editor_set_selection(JSON.stringify(options ?? {})));
}
export function clientScrollTo(options) {
    void options;
    throw new Error("clay.editor.clientScrollTo is planned; Clay JS runtime op wiring is not implemented yet");
}
export async function serverInsertNewline(options) {
    void options;
    throw new Error("clay.editor.serverInsertNewline is planned; Clay JS runtime op wiring is not implemented yet");
}
export function clientSetCursorStyle(options) {
    return parseResult(requireEditorOps().op_clay_editor_set_cursor_style(JSON.stringify(options ?? {})));
}
// Plan 071 task 9 multi-cursor APIs. Direction-bearing commands validate via
// Clay deno ops and return a direction-specific commandId; argless commands
// return their stable registry ID for key binding.
export function clientAddCursor(options) {
    return parseResult(requireEditorOps().op_clay_editor_add_cursor(JSON.stringify(options ?? {})));
}
export function clientColumnSelect(options) {
    return parseResult(requireEditorOps().op_clay_editor_column_select(JSON.stringify(options ?? {})));
}
export function clientSelectTextobject(options) {
    return parseResult(requireEditorOps().op_clay_editor_select_textobject(JSON.stringify(options ?? {})));
}
export function clientSmartSelect(options) {
    return parseResult(requireEditorOps().op_clay_editor_smart_select(JSON.stringify(options ?? {})));
}
export function clientExecuteEditorCommand(options) {
    return parseResult(requireEditorOps().op_clay_editor_execute_command(JSON.stringify(options ?? {})));
}
export function clientSelectNextMatch() {
    return "clay.editor.clientSelectNextMatch";
}
export function clientSelectPrevMatch() {
    return "clay.editor.clientSelectPrevMatch";
}
export function clientSelectAllMatches() {
    return "clay.editor.clientSelectAllMatches";
}
export function clientCancelMultipleSelections() {
    return "clay.editor.clientCancelMultipleSelections";
}
export function clientKeepSelection() {
    return "clay.editor.clientKeepSelection";
}
export function clientRemoveSelection() {
    return "clay.editor.clientRemoveSelection";
}
export function clientUndoCursorMove() {
    return "clay.editor.clientUndoCursorMove";
}
export function clientSetViewport(options) {
    void options;
    throw new Error("clay.editor.clientSetViewport is planned; Clay JS runtime op wiring is not implemented yet");
}
export function clientCopySelection() {
    return "clay.editor.clientCopySelection";
}
export function clientCutSelection() {
    return "clay.editor.clientCutSelection";
}
export function clientPasteClipboard() {
    return "clay.editor.clientPasteClipboard";
}
export function clientUndo() {
    return "clay.editor.clientUndo";
}
export function clientRedo() {
    return "clay.editor.clientRedo";
}
export function clientShowOpenDocuments() {
    return "clay.editor.clientShowOpenDocuments";
}
export function clientRequestResync() {
    return "clay.editor.clientRequestResync";
}
export function clientDismissRecovery() {
    return "clay.editor.clientDismissRecovery";
}
