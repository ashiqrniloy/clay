// Clay folding primitive facade.
//
// Folding APIs are server-side, load/background-time surfaces for publishing
// inert, bounded folding ranges. Collapse state is client-local. Packages
// never run in paint, layout, or pointer paths.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("folding.runtime_unavailable: Clay folding APIs require the server runtime");
    }
    return ops;
}
function parseResult(json) {
    return JSON.parse(json);
}
export function serverPublishFoldingRanges(options) {
    return parseResult(requireOps()["op_clay_folding_publish_ranges"](JSON.stringify(options ?? null)));
}
