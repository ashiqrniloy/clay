// Clay range-diagnostic primitive facade.
//
// Diagnostics APIs are server-side, load/background-time surfaces for publishing
// inert, bounded DiagnosticSet ranges. They do not expose renderer callbacks,
// client JavaScript, raw Deno ops, CSS, or native handles publicly.

const ops = globalThis.Deno?.core?.ops;

function requireOps(): NonNullable<typeof ops> {
  if (!ops) {
    throw new Error("clay.diagnostics.runtime_unavailable: Clay diagnostic APIs require the server runtime");
  }
  return ops;
}

function parseResult(json: string): unknown {
  return JSON.parse(json);
}

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
  viewport: { byteStart: number; byteEnd: number };
  source: string;
  spans: DiagnosticSpanInput[];
};

const FORBIDDEN_KEYS = [
  "handler",
  "callback",
  "onDiagnostic",
  "function",
  "clientJavaScript",
  "nativeHandle",
  "rawOps",
  "draw",
  "css",
  "render",
] as const;

export function serverPublishDiagnostics(options: ServerPublishDiagnosticsOptions): unknown {
  for (const key of FORBIDDEN_KEYS) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(
        `clay.diagnostics.invalid_publication: executable or raw authority field ${key} is not accepted`,
      );
    }
  }
  return parseResult(
    requireOps()["op_clay_diagnostics_publish_diagnostics"](JSON.stringify(options ?? null)),
  );
}
