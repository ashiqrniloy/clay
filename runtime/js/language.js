// Clay language-intelligence provider primitive facade.
//
// Registration is configuration/package-load time only. Short providers receive
// bounded windows; approved document analyzers receive bounded canonical events
// through package-owned module specifiers. No callback arguments, raw ops,
// client JS, filesystem, network, shell, or implicit process authority cross
// this facade. Process use separately requires `clay:language-server` grant.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("language.runtime_unavailable: Clay language APIs require the server runtime");
    }
    return ops;
}
function parseResult(json) {
    return JSON.parse(json);
}
export function serverRegisterDocumentAnalyzer(options) {
    return parseResult(requireOps()["op_clay_language_register_document_analyzer"](JSON.stringify(options ?? null)));
}
export function serverRegisterLanguageIntelligenceProvider(options) {
    for (const key of [
        "handler",
        "callback",
        "function",
        "clientJavaScript",
        "nativeHandle",
        "rawOps",
        "executable",
        "process",
        "languageServer",
    ]) {
        if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
            throw new Error(`language.invalid_provider: executable or process authority field ${key} is not accepted by the public registration contract`);
        }
    }
    const { module, exportName = "provideLanguageIntelligence", ...opOptions } = options ?? {};
    const registration = parseResult(requireOps()["op_clay_language_register_intelligence_provider"](JSON.stringify({
        ...(opOptions ?? {}),
        exportName,
        runtimeBridge: module !== undefined,
    })));
    if (module !== undefined) {
        const handler = module[exportName];
        if (typeof handler !== "function") {
            throw new Error(`language.invalid_provider: module export ${exportName} must be a function`);
        }
        globalThis.__clayLanguageIntelligenceHandlers ??= Object.create(null);
        globalThis.__clayLanguageIntelligenceHandlers[registration.token ?? ""] = handler;
    }
    return registration;
}
