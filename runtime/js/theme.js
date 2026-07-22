const ops = globalThis.Deno?.core?.ops;
function requireOps() {
    if (!ops) {
        throw new Error("clay.theme.runtime_unavailable: Clay theme APIs require the server runtime");
    }
    return ops;
}
export function setTheme(options) {
    const specifier = typeof options === "string" ? options : options?.specifier;
    if (typeof specifier !== "string" || specifier.length === 0) {
        throw new Error("clay.theme.invalid_request: setTheme requires a theme specifier");
    }
    return JSON.parse(requireOps().op_clay_theme_set_theme(JSON.stringify({ specifier })));
}
export function setTypography(options) {
    if (options === null || typeof options !== "object") {
        throw new Error("clay.theme.invalid_typography: setTypography requires complete typography profiles");
    }
    return JSON.parse(requireOps().op_clay_theme_set_typography(JSON.stringify(options)));
}
