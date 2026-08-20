export type DecorationSpanInput = {
    byteStart: number;
    byteEnd: number;
    kind: "syntax" | "semantic" | "diagnostic" | "search-match" | "link" | "inlayHint";
    inlay?: {
        label: string;
        placement: "before" | "after";
    };
    target?: {
        kind: "workspacePath" | "documentRange" | "displayOnly";
        relativePath?: string;
        text?: string;
        byteStart?: number;
        byteEnd?: number;
    };
    /**
     * Closed TokenType variant name (e.g. "Function", "Variable", "Keyword").
     * Preferred for semantic/LSP and first-party two-axis publishers.
     * Provide either `tokenType` or legacy `styleToken`.
     */
    tokenType?: string;
    /**
     * Closed Modifiers names (e.g. ["Declaration", "Readonly", "Bold"]).
     * Only consulted when `tokenType` is set.
     */
    modifiers?: string[];
    /**
     * Legacy free-form style token (e.g. "keyword.control"). Classified into
     * TokenType + Modifiers server-side and retained as the optional scope escape.
     */
    styleToken?: string;
    fontRole?: "monospace" | "proportional";
    priority?: number;
};
export type ServerPublishDecorationsOptions = {
    documentId: number;
    documentVersion: number;
    currentDocumentVersion?: number;
    behaviorVersion?: number;
    viewport: {
        byteStart: number;
        byteEnd: number;
    };
    spans: DecorationSpanInput[];
};
export declare function serverPublishDecorations(options: ServerPublishDecorationsOptions): unknown;
