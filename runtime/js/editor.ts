// Clay editor facade skeleton.
//
// This file defines planned user-facing APIs for the future server-side
// JavaScript runtime. These exports intentionally do not call raw
// `Deno.core.ops` functions; Phase 11 will wire them to explicit Clay op
// wrappers behind this stable facade.

export type DocumentId = string;

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
  selection?: { anchor: number; focus: number };
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

function plannedApi(name: string): never {
  throw new Error(`${name} is planned; Clay JS runtime op wiring is not implemented yet`);
}

export async function serverInsertText(options: ServerInsertTextOptions): Promise<EditResult> {
  void options;
  plannedApi("clay.editor.serverInsertText");
}

export async function serverDeleteRange(options: ServerDeleteRangeOptions): Promise<EditResult> {
  void options;
  plannedApi("clay.editor.serverDeleteRange");
}

export function clientMoveCursor(options: ClientMoveCursorOptions): CursorMoveResult {
  void options;
  plannedApi("clay.editor.clientMoveCursor");
}

export function clientSetSelection(options: ClientSetSelectionOptions): SelectionResult {
  void options;
  plannedApi("clay.editor.clientSetSelection");
}

export function clientScrollTo(options: ClientScrollToOptions): ScrollResult {
  void options;
  plannedApi("clay.editor.clientScrollTo");
}

export async function serverInsertNewline(options: ServerInsertNewlineOptions): Promise<EditResult> {
  void options;
  plannedApi("clay.editor.serverInsertNewline");
}

export function clientSetCursorStyle(options: ClientSetCursorStyleOptions): CursorStyleResult {
  void options;
  plannedApi("clay.editor.clientSetCursorStyle");
}

export function clientSetViewport(options: ClientSetViewportOptions): { documentId: DocumentId; visibleLineCount: number } {
  void options;
  plannedApi("clay.editor.clientSetViewport");
}
