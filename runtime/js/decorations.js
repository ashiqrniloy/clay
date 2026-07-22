// Clay decoration primitive facade.
//
// Decoration APIs are server-side, load/background-time surfaces for publishing
// inert, bounded decoration ranges. They do not expose renderer callbacks,
// client JavaScript, or raw Deno ops publicly.
//
// Semantic intelligence (Phase 18.20) publishes DecorationKind::Semantic spans
// with direct closed TokenType + Modifiers vocabulary. Legacy styleToken input
// remains a third-party compatibility escape and is classified into the same
// two-axis model server-side.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("clay.decorations.runtime_unavailable: Clay decoration APIs require the server runtime");
    }
    return ops;
}
function parseResult(json) {
    return JSON.parse(json);
}
export function serverPublishDecorations(options) {
    return parseResult(requireOps()["op_clay_decorations_publish_decorations"](JSON.stringify(options ?? null)));
}
