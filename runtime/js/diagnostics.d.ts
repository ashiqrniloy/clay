export type DiagnosticSeverityInput = "error" | "warning" | "info";
export type DiagnosticSpanInput = {
    byteStart: number;
    byteEnd: number;
    severity: DiagnosticSeverityInput;
    code: string;
    message: string;
    source?: string;
};
export type ServerPublishDiagnosticsOptions = {
    documentId: number;
    documentVersion: number;
    currentDocumentVersion?: number;
    viewport: {
        byteStart: number;
        byteEnd: number;
    };
    source: string;
    spans: DiagnosticSpanInput[];
};
export declare function serverPublishDiagnostics(options: ServerPublishDiagnosticsOptions): unknown;
