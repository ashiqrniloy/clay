// Clay mode primitive facade skeleton.
//
// Mode APIs are server-owned registration/classification/activation APIs. They
// run at package load, document open, or explicit activation time and never make
// ordinary typing or paint depend on JavaScript in the Rust client.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("modes.runtime_unavailable: Clay mode APIs require the server runtime");
    }
    return ops;
}
function parse(json) {
    return JSON.parse(json);
}
const activationRegistry = (globalThis.__clayModeActivations ??= Object.create(null));
const activationKey = (apiPrefix, modeId) => `${apiPrefix}:${modeId}`;
// Package provenance is stamped host-side; the op response carries the
// host-registered packagePrefix/modeId used to key inert activation payloads.
export function serverRegisterModePattern(declaration) {
    const result = parse(requireOps().op_clay_modes_register_pattern(JSON.stringify(declaration ?? null)));
    const mode = declaration;
    if (result?.packagePrefix && result?.modeId) {
        activationRegistry[activationKey(result.packagePrefix, result.modeId)] = {
            editorRules: mode?.editorRules,
            commands: mode?.commands,
            keymaps: mode?.keymaps,
        };
    }
    return result;
}
export function serverClassifyDocument(input) {
    return parse(requireOps().op_clay_modes_classify_document(JSON.stringify(input ?? null)));
}
export function serverActivateMajorMode(input) {
    return parse(requireOps().op_clay_modes_activate_major_mode(JSON.stringify(input ?? null)));
}
export function serverActivateClassifiedMode(classification, input = {}) {
    const classified = classification;
    const activation = activationRegistry[activationKey(String(classified?.apiPrefix), String(classified?.modeId))];
    if (!activation || classified?.documentId === undefined || !classified?.modeId) {
        throw new Error("modes.activation_failed: classified mode has no registered activation metadata");
    }
    return serverActivateMajorMode({
        ...input,
        documentId: classified.documentId,
        modeId: classified.modeId,
        editorRules: activation.editorRules,
        commands: activation.commands,
        keymaps: activation.keymaps,
    });
}
export function serverSelectDocumentManifest(options) {
    void options;
    return requireOps().op_clay_runtime_unavailable("modes.serverSelectDocumentManifest");
}
export function serverRegisterDecorationProvider(options) {
    void options;
    return requireOps().op_clay_runtime_unavailable("modes.serverRegisterDecorationProvider");
}
export function serverRegisterParseProvider(options) {
    void options;
    return requireOps().op_clay_runtime_unavailable("modes.serverRegisterParseProvider");
}
export function serverRegisterFoldingProvider(options) {
    void options;
    return requireOps().op_clay_runtime_unavailable("modes.serverRegisterFoldingProvider");
}
