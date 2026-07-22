// Clay package UI contribution facade.
//
// These helpers run inside Clay's constrained server-side JavaScript runtime and
// delegate inert slot-aware UI contribution validation to Clay-owned ops.  The
// public API accepts declarative contribution data only; package provenance is
// stamped host-side from the executing-package context. Raw op names, Masonry
// widgets, native handles, CSS strings, renderer callbacks, and client-side
// JavaScript hooks are not package-facing authorities.
function uiOps() {
    const ops = Deno?.core?.ops;
    if (typeof ops?.op_clay_ui_register_panel_contribution !== "function" ||
        typeof ops?.op_clay_ui_register_component_contribution !== "function" ||
        typeof ops?.op_clay_ui_register_transient_overlay_contribution !== "function" ||
        typeof ops?.op_clay_ui_register_theme_token !== "function" ||
        typeof ops?.op_clay_ui_register_input_contribution !== "function" ||
        typeof ops?.op_clay_ui_register_ui_state_scope !== "function" ||
        typeof ops?.op_clay_ui_set_layout_override !== "function") {
        throw new Error("clay.ui runtime ops are unavailable in this context");
    }
    return ops;
}
function encode(value) {
    return JSON.stringify(value ?? null);
}
export function serverRegisterPanelContribution(declaration) {
    return JSON.parse(uiOps().op_clay_ui_register_panel_contribution(encode(declaration)));
}
export function serverRegisterComponentContribution(declaration) {
    return JSON.parse(uiOps().op_clay_ui_register_component_contribution(encode(declaration)));
}
export function serverRegisterTransientOverlayContribution(declaration) {
    return JSON.parse(uiOps().op_clay_ui_register_transient_overlay_contribution(encode(declaration)));
}
export function serverRegisterInputContribution(declaration) {
    return JSON.parse(uiOps().op_clay_ui_register_input_contribution(encode(declaration)));
}
export function serverRegisterUiStateScope(declaration) {
    return JSON.parse(uiOps().op_clay_ui_register_ui_state_scope(encode(declaration)));
}
export function serverSetLayoutOverride(declaration) {
    return JSON.parse(uiOps().op_clay_ui_set_layout_override(encode(declaration)));
}
export function serverRegisterThemeToken(declaration) {
    return JSON.parse(uiOps().op_clay_ui_register_theme_token(encode(declaration)));
}
