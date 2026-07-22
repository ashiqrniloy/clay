// Clay syntax grammar primitive facade.
//
// Syntax APIs register inert, first-party grammar metadata for server-side
// background highlighting. They do not expose raw Deno ops, executable parser
// callbacks, native grammar handles, arbitrary artifact loading, or client JS.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("clay.syntax.runtime_unavailable: Clay syntax APIs require the server runtime");
    }
    return ops;
}
function parseResult(json) {
    return JSON.parse(json);
}
export function setSyntaxEnginePreference(target, tier) {
    return parseResult(requireOps()["op_clay_syntax_set_engine_preference"](target, tier));
}
export function serverRegisterSyntaxGrammar(options) {
    for (const key of ["handler", "callback", "onParse", "function", "clientJavaScript", "nativeHandle", "rawOps"]) {
        if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
            throw new Error(`clay.syntax.invalid_grammar: executable or raw authority field ${key} is not accepted by the public registration contract`);
        }
    }
    return parseResult(requireOps()["op_clay_syntax_register_syntax_grammar"](JSON.stringify(options ?? null)));
}
