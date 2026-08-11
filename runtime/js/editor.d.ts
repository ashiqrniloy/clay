export type DocumentId = string;
export type ClientCopySelectionCommandId = "editor.clientCopySelection";
export type ClientCutSelectionCommandId = "editor.clientCutSelection";
export type ClientPasteClipboardCommandId = "editor.clientPasteClipboard";
export type ClientUndoCommandId = "editor.clientUndo";
export type ClientRedoCommandId = "editor.clientRedo";
export type ClientShowOpenDocumentsCommandId = "editor.clientShowOpenDocuments";
export type ClientRequestResyncCommandId = "editor.clientRequestResync";
export type ClientDismissRecoveryCommandId = "editor.clientDismissRecovery";
export type ClientMoveCursorWordStartForwardCommandId = "editor.clientMoveCursor.nextWordStart";
export type ClientMoveCursorWordStartBackwardCommandId = "editor.clientMoveCursor.prevWordStart";
export type ClientMoveCursorParagraphForwardCommandId = "editor.clientMoveCursor.nextParagraph";
export type ClientMoveCursorParagraphBackwardCommandId = "editor.clientMoveCursor.prevParagraph";
export type ClientSetSelectionSelectWordCommandId = "editor.clientSetSelection.selectWord";
export type ClientSetSelectionSelectLineCommandId = "editor.clientSetSelection.selectLine";
export type ClientAddCursorBelowCommandId = "editor.clientAddCursor.below";
export type ClientAddCursorAboveCommandId = "editor.clientAddCursor.above";
export type ClientColumnSelectDownCommandId = "editor.clientColumnSelect.down";
export type ClientColumnSelectUpCommandId = "editor.clientColumnSelect.up";
export type ClientColumnSelectLeftCommandId = "editor.clientColumnSelect.left";
export type ClientColumnSelectRightCommandId = "editor.clientColumnSelect.right";
export type ClientSelectNextMatchCommandId = "editor.clientSelectNextMatch";
export type ClientSelectPrevMatchCommandId = "editor.clientSelectPrevMatch";
export type ClientSelectAllMatchesCommandId = "editor.clientSelectAllMatches";
export type ClientCancelMultipleSelectionsCommandId = "editor.clientCancelMultipleSelections";
export type ClientKeepSelectionCommandId = "editor.clientKeepSelection";
export type ClientRemoveSelectionCommandId = "editor.clientRemoveSelection";
export type ClientUndoCursorMoveCommandId = "editor.clientUndoCursorMove";
export type ClientSmartSelectExpandCommandId = "editor.clientSmartSelect.expand";
export type ClientSmartSelectShrinkCommandId = "editor.clientSmartSelect.shrink";
export type TextobjectKind =
    | "function"
    | "class"
    | "argument"
    | "comment"
    | "loop"
    | "conditional"
    | "call"
    | "statement";
export type TextobjectDirection = "current" | "next" | "previous";
export type SmartSelectAction = "expand" | "shrink";
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
    documentId?: DocumentId;
    direction: "nextWordStart" | "prevWordStart" | "nextWordEnd" | "prevWordEnd" | "nextParagraph" | "prevParagraph" | "firstNonWhitespace" | "lastNonWhitespace" | "matchingPair" | "left" | "right" | "up" | "down" | "start" | "end";
    granularity?: "word" | "subword" | "paragraph" | "line" | "character";
    extend?: boolean;
    count?: number;
}
export interface ClientSetSelectionOptions {
    documentId?: DocumentId;
    action: "selectWord" | "selectLine" | "selectParagraph";
    extend?: boolean;
    direction?: "current" | "next" | "prev";
}
export interface ClientScrollToOptions {
    documentId: DocumentId;
    line?: number;
    column?: number;
    revealCursor?: boolean;
}
export interface ClientSetCursorStyleOptions {
    /** Caret glyph shape. Colour stays theme-owned (`base.caret`). */
    shape?: "bar" | "line" | "block" | "underline";
    /** Blink behaviour; `solid` never hides (reduced-motion friendly). */
    blink?: "solid" | "blink" | "phase" | "smooth";
    /** Stroke thickness for bar/line/underline, in pixels. */
    widthPx?: number;
    /** Caret height as a fraction of the line height (1 = full line). */
    heightPct?: number;
    /** Render `block` as an outline. */
    hollow?: boolean;
    /** Restart the blink to visible on typing (default true). */
    stopBlinkOnTyping?: boolean;
}
export interface ClientSetViewportOptions {
    documentId: DocumentId;
    visibleLineCount: number;
    overscanLines?: number;
}
export interface ClientAddCursorOptions {
    documentId?: DocumentId;
    /** Add a caret one line below/above the primary at the same column. */
    direction: "below" | "above";
}
export interface ClientColumnSelectOptions {
    documentId?: DocumentId;
    /** Down/up grow the column box one line; left/right move every caret. */
    direction: "down" | "up" | "left" | "right";
}
export interface ClientSelectTextobjectOptions {
    /** Which text object to select (grammar-provided captures). */
    object: TextobjectKind;
    /** true selects the `around` capture; false (default) the `inner`. */
    around?: boolean;
    /** current (default): innermost around the caret; next/previous: walk. */
    direction?: TextobjectDirection;
}
export interface ClientSmartSelectOptions {
    /** expand grows to the enclosing AST node range; shrink reverses. */
    action: SmartSelectAction;
}
export interface CursorMoveResult {
    commandId: "editor.clientMoveCursor";
    direction: string;
    granularity?: string;
    extend: boolean;
    count: number;
}
export interface SelectionResult {
    commandId: "editor.clientSetSelection";
    action: string;
    extend: boolean;
    direction?: string;
}
export interface ScrollResult {
    documentId: DocumentId;
    line?: number;
    column?: number;
}
export interface CursorStyleResult {
    commandId: "editor.clientSetCursorStyle";
    shape?: "bar" | "line" | "block" | "underline";
    blink?: "solid" | "blink" | "phase" | "smooth";
    widthPx?: number;
    heightPct?: number;
    hollow: boolean;
    stopBlinkOnTyping: boolean;
}
export interface AddCursorResult {
    commandId: ClientAddCursorBelowCommandId | ClientAddCursorAboveCommandId;
    direction: "below" | "above";
}
export interface ColumnSelectResult {
    commandId:
        | ClientColumnSelectDownCommandId
        | ClientColumnSelectUpCommandId
        | ClientColumnSelectLeftCommandId
        | ClientColumnSelectRightCommandId;
    direction: "down" | "up" | "left" | "right";
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
export declare function clientAddCursor(options: ClientAddCursorOptions): AddCursorResult;
export declare function clientColumnSelect(options: ClientColumnSelectOptions): ColumnSelectResult;
export interface SelectTextobjectResult {
    /** editor.clientSelectTextobject.<object>.<inner|around>[.next|.previous] */
    commandId: string;
    object: TextobjectKind;
    around: boolean;
    direction: TextobjectDirection;
}
export interface SmartSelectResult {
    /** editor.clientSmartSelect.<action> */
    commandId: string;
    action: SmartSelectAction;
}
/**
 * Options for `clientExecuteEditorCommand` (Plan 071 follow-up round,
 * `editor-control`). The command ID must be a known direction-specific
 * argless editor command ID; unknown IDs are rejected deny-by-default.
 */
export interface ClientExecuteEditorCommandOptions {
    /** e.g. `editor.clientMoveCursor.nextWordStart`. */
    commandId: string;
}
export interface ExecuteEditorCommandResult {
    requested: boolean;
    /** True when the connection publisher is wired (advisory delivery). */
    published: boolean;
    commandId: string;
}
export declare function clientSelectTextobject(options: ClientSelectTextobjectOptions): SelectTextobjectResult;
export declare function clientSmartSelect(options: ClientSmartSelectOptions): SmartSelectResult;
export declare function clientExecuteEditorCommand(options: ClientExecuteEditorCommandOptions): ExecuteEditorCommandResult;
export declare function clientSelectNextMatch(): ClientSelectNextMatchCommandId;
export declare function clientSelectPrevMatch(): ClientSelectPrevMatchCommandId;
export declare function clientSelectAllMatches(): ClientSelectAllMatchesCommandId;
export declare function clientCancelMultipleSelections(): ClientCancelMultipleSelectionsCommandId;
export declare function clientKeepSelection(): ClientKeepSelectionCommandId;
export declare function clientRemoveSelection(): ClientRemoveSelectionCommandId;
export declare function clientUndoCursorMove(): ClientUndoCursorMoveCommandId;
