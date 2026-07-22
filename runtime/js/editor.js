// Clay editor facade skeleton.
//
// This file defines planned user-facing APIs for the future server-side
// JavaScript runtime. These exports intentionally do not call raw
// `Deno.core.ops` functions; Phase 11 will wire them to explicit Clay op
// wrappers behind this stable facade.
function plannedApi(name) {
    throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}
export async function serverInsertText(options) {
    void options;
    plannedApi("clay.editor.serverInsertText");
}
export async function serverDeleteRange(options) {
    void options;
    plannedApi("clay.editor.serverDeleteRange");
}
export function clientMoveCursor(options) {
    void options;
    plannedApi("clay.editor.clientMoveCursor");
}
export function clientSetSelection(options) {
    void options;
    plannedApi("clay.editor.clientSetSelection");
}
export function clientScrollTo(options) {
    void options;
    plannedApi("clay.editor.clientScrollTo");
}
export async function serverInsertNewline(options) {
    void options;
    plannedApi("clay.editor.serverInsertNewline");
}
export function clientSetCursorStyle(options) {
    void options;
    plannedApi("clay.editor.clientSetCursorStyle");
}
export function clientSetViewport(options) {
    void options;
    plannedApi("clay.editor.clientSetViewport");
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
