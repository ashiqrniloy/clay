const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("theme.runtime_unavailable: Clay theme APIs require the server runtime");
    }
    return ops;
}
export function setTheme(options) {
    const specifier = typeof options === "string" ? options : options?.specifier;
    if (typeof specifier !== "string" || specifier.length === 0) {
        throw new Error("theme.invalid_request: setTheme requires a theme specifier");
    }
    return JSON.parse(requireOps().op_clay_theme_set_theme(JSON.stringify({ specifier })));
}
export function setTypography(options) {
    if (options === null || typeof options !== "object") {
        throw new Error("theme.invalid_typography: setTypography requires complete typography profiles");
    }
    return JSON.parse(requireOps().op_clay_theme_set_typography(JSON.stringify(options)));
}
export function setAppearance(options) {
    const appearance = typeof options === "string" ? options : options?.appearance;
    if (typeof appearance !== "string" || appearance.length === 0) {
        throw new Error("theme.invalid_request: setAppearance requires an appearance string");
    }
    return JSON.parse(requireOps().op_clay_theme_set_appearance(JSON.stringify({ appearance })));
}
export function setDesignSystem(options) {
    const specifier = typeof options === "string" ? options : options?.specifier;
    if (typeof specifier !== "string" || specifier.length === 0) {
        throw new Error("theme.invalid_request: setDesignSystem requires a design-system specifier");
    }
    return JSON.parse(requireOps().op_clay_theme_set_design_system(JSON.stringify({ specifier })));
}
export function setIconPack(options) {
    const specifier = typeof options === "string" ? options : options?.specifier;
    if (typeof specifier !== "string" || specifier.length === 0) {
        throw new Error("theme.invalid_request: setIconPack requires an icon-pack specifier");
    }
    return JSON.parse(requireOps().op_clay_theme_set_icon_pack(JSON.stringify({ specifier })));
}

