export type DocumentId = string;
export type ClientCopySelectionCommandId = "clay.editor.clientCopySelection";
export type ClientCutSelectionCommandId = "clay.editor.clientCutSelection";
export type ClientPasteClipboardCommandId = "clay.editor.clientPasteClipboard";
export type ClientUndoCommandId = "clay.editor.clientUndo";
export type ClientRedoCommandId = "clay.editor.clientRedo";
export type ClientShowOpenDocumentsCommandId = "clay.editor.clientShowOpenDocuments";
export type ClientRequestResyncCommandId = "clay.editor.clientRequestResync";
export type ClientDismissRecoveryCommandId = "clay.editor.clientDismissRecovery";
export interface EditResult {
    accepted: boolean;
    documentVersion?: number;
}
export interface ServerInsertTextOptions {
    documentId: DocumentId;
    offset: number;
    text: string;
    normalizeLineEndings?: boolean;
}
export interface ServerDeleteRangeOptions {
    documentId: DocumentId;
    start: number;
    end: number;
}
export interface ServerInsertNewlineOptions {
    documentId: DocumentId;
    offset: number;
    enterRule?: "preserveLeadingWhitespace" | "none";
    commentContinuation?: string;
}
export interface ClientMoveCursorOptions {
    documentId: DocumentId;
    direction: "left" | "right" | "up" | "down" | "start" | "end";
    extendSelection?: boolean;
}
export interface ClientSetSelectionOptions {
    documentId: DocumentId;
    anchor: number;
    focus: number;
}
export interface ClientScrollToOptions {
    documentId: DocumentId;
    line?: number;
    column?: number;
    revealCursor?: boolean;
}
export interface ClientSetCursorStyleOptions {
    color?: string;
    blinking?: boolean;
    type?: "block" | "bar" | "underline";
}
export interface ClientSetViewportOptions {
    documentId: DocumentId;
    visibleLineCount: number;
    overscanLines?: number;
}
export interface CursorMoveResult {
    documentId: DocumentId;
    cursorOffset: number;
    selection?: {
        anchor: number;
        focus: number;
    };
}
export interface SelectionResult {
    documentId: DocumentId;
    anchor: number;
    focus: number;
}
export interface ScrollResult {
    documentId: DocumentId;
    line?: number;
    column?: number;
}
export interface CursorStyleResult {
    color?: string;
    blinking?: boolean;
    type?: "block" | "bar" | "underline";
}
export declare function serverInsertText(options: ServerInsertTextOptions): Promise<EditResult>;
export declare function serverDeleteRange(options: ServerDeleteRangeOptions): Promise<EditResult>;
export declare function clientMoveCursor(options: ClientMoveCursorOptions): CursorMoveResult;
export declare function clientSetSelection(options: ClientSetSelectionOptions): SelectionResult;
export declare function clientScrollTo(options: ClientScrollToOptions): ScrollResult;
export declare function serverInsertNewline(options: ServerInsertNewlineOptions): Promise<EditResult>;
export declare function clientSetCursorStyle(options: ClientSetCursorStyleOptions): CursorStyleResult;
export declare function clientSetViewport(options: ClientSetViewportOptions): {
    documentId: DocumentId;
    visibleLineCount: number;
};
export declare function clientCopySelection(): ClientCopySelectionCommandId;
export declare function clientCutSelection(): ClientCutSelectionCommandId;
export declare function clientPasteClipboard(): ClientPasteClipboardCommandId;
export declare function clientUndo(): ClientUndoCommandId;
export declare function clientRedo(): ClientRedoCommandId;
export declare function clientShowOpenDocuments(): ClientShowOpenDocumentsCommandId;
export declare function clientRequestResync(): ClientRequestResyncCommandId;
export declare function clientDismissRecovery(): ClientDismissRecoveryCommandId;
