// Clay key binding facade skeleton.
//
// Key binding APIs are planned configuration-time server runtime APIs. They
// record user intent for Clay-owned commands by stable registry ID; they do not
// install JavaScript into the Rust client keypress hot path.
const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("clay.keybindings.runtime_unavailable: Clay key binding APIs require the server runtime");
    }
    return ops;
}
export function bindKey(key, command, options = {}) {
    return JSON.parse(requireOps().op_clay_keybindings_bind_key(key, command, JSON.stringify(options ?? {})));
}
export function unbindKey(key, options = {}) {
    requireOps().op_clay_keybindings_unbind_key(key, JSON.stringify(options ?? {}));
}
export function listKeyBindings(scope = "all") {
    return JSON.parse(requireOps().op_clay_keybindings_list_key_bindings(scope));
}
