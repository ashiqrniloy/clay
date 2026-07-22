// Clay server-driven UI facade.
//
// These helpers run inside Clay's constrained server-side JavaScript runtime and
// delegate SDUI node validation/construction and explicit publication to
// Clay-owned ops. Published trees are validated by the server before they are
// sent through the existing Phase 12 SDUI snapshot/update protocol path.
function sduiOps() {
    const ops = Deno?.core?.ops;
    if (typeof ops?.op_clay_sdui_define_node !== "function" ||
        typeof ops?.op_clay_sdui_publish_tree !== "function") {
        throw new Error("clay.sdui runtime ops are unavailable in this context");
    }
    return ops;
}
function defineNode(kind, options) {
    return JSON.parse(sduiOps().op_clay_sdui_define_node(kind, JSON.stringify(options ?? {})));
}
export function definePanel(options) {
    return defineNode("panel", options);
}
export function defineLabel(options) {
    return defineNode("label", options);
}
export function defineButton(options) {
    return defineNode("button", options);
}
export function defineList(options) {
    return defineNode("list", options);
}
export function defineEditorView(options) {
    return defineNode("editorView", options);
}
export function defineFlex(options) {
    return defineNode("flex", options);
}
export function defineStack(options) {
    return defineNode("stack", options);
}
export async function publishTree(tree) {
    sduiOps().op_clay_sdui_publish_tree(JSON.stringify(tree));
}
