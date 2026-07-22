export type SyntaxDiagnosticKind = "error" | "missing";
export type SyntaxDiagnosticCapture = {
    byteStart: number;
    byteEnd: number;
    kind: SyntaxDiagnosticKind;
};
export type ParseDiagnosticSpan = {
    byteStart: number;
    byteEnd: number;
    severity: "error" | "warning" | "info";
    code: string;
    message: string;
};
export type ParseDiagnosticUpdate = {
    source: string;
    spans: ParseDiagnosticSpan[];
};
export type IncrementalParseResult = {
    documentId?: number;
    documentVersion?: number;
    behaviorVersion?: number;
    packagePrefix?: string;
    mode?: string;
    viewport?: {
        byteStart: number;
        byteEnd: number;
    };
    syntaxTreeDelta?: string;
    spans?: unknown[];
    diagnostics?: ParseDiagnosticUpdate;
};
export type ServerRegisterParseHandlerOptions = {
    module?: Record<string, unknown>;
    handler?: never;
    callback?: never;
    onParse?: never;
    function?: never;
    exportName?: string;
    mode: string;
    parseUnit?: "file" | "region" | "line-group";
    viewportPriority?: boolean;
    timeoutMs?: number;
    maxWindowBytes?: number;
    parseWindowBytes?: number;
    guardBytes?: number;
    memoryBudgetBytes?: number;
};
export declare function serverRegisterParseHandler(options: ServerRegisterParseHandlerOptions): unknown;
