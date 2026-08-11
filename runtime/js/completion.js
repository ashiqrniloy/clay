// Clay completion provider primitive facade.
//
// Completion APIs register inert metadata and optional imported module exports
// at load/reload time. Provider execution stays server-side, cancellable, and
// bounded; no raw ops, callback arguments, client JS, filesystem, network,
// shell, AI, WASM, native-widget, or package-manager authority is exposed.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("completion.runtime_unavailable: Clay completion APIs require the server runtime");
    }
    return ops;
}
function parseResult(json) {
    return JSON.parse(json);
}
export function serverRegisterCompletionProvider(options) {
    for (const key of ["handler", "callback", "complete", "function", "clientJavaScript", "nativeHandle", "rawOps"]) {
        if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
            throw new Error(`completion.invalid_provider: executable or raw authority field ${key} is not accepted by the public registration contract`);
        }
    }
    const { module, exportName = "provideCompletion", ...opOptions } = options ?? {};
    const registration = parseResult(requireOps()["op_clay_completion_register_completion_provider"](JSON.stringify({ ...opOptions, exportName, runtimeBridge: module !== undefined })));
    if (module !== undefined) {
        const handler = module[exportName];
        if (typeof handler !== "function") {
            throw new Error(`completion.invalid_provider: module export ${exportName} must be a function`);
        }
        globalThis
            .__clayCompletionHandlers ??= Object.create(null);
        for (const token of registration.tokens ?? []) {
            globalThis
                .__clayCompletionHandlers[token] = handler;
        }
    }
    return registration;
}
export function serverDisableCompletion(options) {
    for (const key of Object.keys(options ?? {})) {
        if (key !== "provider" && key !== "packagePrefix") {
            throw new Error("completion.invalid_disable: only provider or packagePrefix is accepted");
        }
    }
    const provider = (options ?? {}).provider;
    const packagePrefix = (options ?? {}).packagePrefix;
    const targets = [provider, packagePrefix].filter((value) => typeof value === "string" && value.trim().length > 0);
    if (targets.length !== 1) {
        throw new Error("completion.invalid_disable: provide exactly one non-empty provider or packagePrefix");
    }
    return parseResult(requireOps()["op_clay_completion_disable"](JSON.stringify(options)));
}
export function serverListCompletionProvidersForTrigger(options) {
    const trigger = (options ?? {}).trigger;
    if (typeof trigger !== "string" || trigger.length === 0) {
        throw new Error("completion.invalid_trigger: trigger must be a non-empty string");
    }
    return parseResult(requireOps()["op_clay_completion_providers_for_trigger"](trigger));
}
export function completionTriggerCharactersFromEditorRules(editorRules) {
    const triggers = editorRules?.autocompleteTriggers ?? [];
    const characters = [];
    for (const trigger of triggers) {
        const value = trigger?.trigger;
        if (typeof value === "string" && value.length > 0) {
            characters.push(value);
        }
    }
    return characters;
}
