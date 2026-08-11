// Clay range-diagnostic primitive facade.
//
// Diagnostics APIs are server-side, load/background-time surfaces for publishing
// inert, bounded DiagnosticSet ranges. They do not expose renderer callbacks,
// client JavaScript, raw Deno ops, CSS, or native handles publicly.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("diagnostics.runtime_unavailable: Clay diagnostic APIs require the server runtime");
    }
    return ops;
}
function parseResult(json) {
    return JSON.parse(json);
}
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
];
export function serverPublishDiagnostics(options) {
    for (const key of FORBIDDEN_KEYS) {
        if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
            throw new Error(`diagnostics.invalid_publication: executable or raw authority field ${key} is not accepted`);
        }
    }
    return parseResult(requireOps()["op_clay_diagnostics_publish_diagnostics"](JSON.stringify(options ?? null)));
}
