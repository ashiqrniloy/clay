// Clay editor facade skeleton.
//
// This file defines user-facing APIs for the server-side JavaScript runtime.
// Cursor/selection facades validate typed args via Clay deno ops; the argless
// command-id facades return stable registry IDs for key binding (Plan 071 task 5).
const editorOps = globalThis.Deno?.core?.ops;
function requireEditorOps() {
    if (!editorOps) {
        throw new Error("editor.runtime_unavailable: Clay editor APIs require the server runtime");
    }
    return editorOps;
}
function parseResult(json) {
    return JSON.parse(json);
}
export async function serverInsertText(options) {
    void options;
    throw new Error("editor.serverInsertText is planned; Clay JS runtime op wiring is not implemented yet");
}
export async function serverDeleteRange(options) {
    void options;
    throw new Error("editor.serverDeleteRange is planned; Clay JS runtime op wiring is not implemented yet");
}
export function clientMoveCursor(options) {
    return parseResult(requireEditorOps().op_clay_editor_move_cursor(JSON.stringify(options ?? {})));
}
export function clientSetSelection(options) {
    return parseResult(requireEditorOps().op_clay_editor_set_selection(JSON.stringify(options ?? {})));
}
export function clientScrollTo(options) {
    void options;
    throw new Error("editor.clientScrollTo is planned; Clay JS runtime op wiring is not implemented yet");
}
export async function serverInsertNewline(options) {
    void options;
    throw new Error("editor.serverInsertNewline is planned; Clay JS runtime op wiring is not implemented yet");
}
export function clientSetCursorStyle(options) {
    return parseResult(requireEditorOps().op_clay_editor_set_cursor_style(JSON.stringify(options ?? {})));
}
// Phase 26: user-owned editor wrap-policy override. Trusted-domain init.js
// only (packages cannot forge it); beats the per-mode manifest wrap policy.
export function clientSetEditorLayout(options) {
    return parseResult(requireEditorOps().op_clay_editor_set_editor_layout(JSON.stringify(options ?? {})));
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
    return "editor.clientSelectNextMatch";
}
export function clientSelectPrevMatch() {
    return "editor.clientSelectPrevMatch";
}
export function clientSelectAllMatches() {
    return "editor.clientSelectAllMatches";
}
export function clientCancelMultipleSelections() {
    return "editor.clientCancelMultipleSelections";
}
export function clientKeepSelection() {
    return "editor.clientKeepSelection";
}
export function clientRemoveSelection() {
    return "editor.clientRemoveSelection";
}
export function clientUndoCursorMove() {
    return "editor.clientUndoCursorMove";
}
export function toggleComment() {
    return "editor.toggleComment";
}
export function toggleListMarker() {
    return "editor.toggleListMarker";
}
export function rotateHeading() {
    return "editor.rotateHeading";
}
export function clientToggleFold() {
    return "editor.clientToggleFold";
}
export function toggleInlayHints() {
    return "editor.toggleInlayHints";
}
export function clientSetViewport(options) {
    void options;
    throw new Error("editor.clientSetViewport is planned; Clay JS runtime op wiring is not implemented yet");
}
export function clientCopySelection() {
    return "editor.clientCopySelection";
}
export function clientCutSelection() {
    return "editor.clientCutSelection";
}
export function clientPasteClipboard() {
    return "editor.clientPasteClipboard";
}
export function clientUndo() {
    return "editor.clientUndo";
}
export function clientRedo() {
    return "editor.clientRedo";
}
export function clientShowOpenDocuments() {
    return "editor.clientShowOpenDocuments";
}
export function clientRequestResync() {
    return "editor.clientRequestResync";
}
export function clientDismissRecovery() {
    return "editor.clientDismissRecovery";
}
