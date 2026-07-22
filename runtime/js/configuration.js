// Clay configuration facade.
//
// Configuration runs from `~/.config/clay/init.js` in Clay's constrained
// server-side JavaScript runtime. These APIs delegate to Clay-owned ops when
// the embedded runtime provides them; they do not grant network, shell,
// package, AI, workspace, WASM, or client-side JavaScript authority.
function configurationOps() {
    const ops = Deno?.core?.ops;
    if (typeof ops?.op_clay_configuration_load_module !== "function" ||
        typeof ops?.op_clay_configuration_get_state !== "function" ||
        typeof ops?.op_clay_configuration_set_package_option !== "function") {
        throw new Error("clay.configuration runtime ops are unavailable in this context");
    }
    return ops;
}
function plannedConfigurationApi(api) {
    const unavailable = Deno?.core?.ops?.op_clay_runtime_unavailable;
    if (typeof unavailable === "function") {
        unavailable(api);
    }
    throw new Error(`${api} is planned; configuration setting validation is not implemented yet`);
}
export async function loadConfigurationModule(options) {
    if (options === null || typeof options !== "object" || typeof options.path !== "string") {
        throw new Error("clay.configuration.invalid_module: loadConfigurationModule requires { path: string }");
    }
    const path = configurationOps().op_clay_configuration_load_module(options.path);
    await import(path);
}
export function getConfigurationState() {
    return JSON.parse(configurationOps().op_clay_configuration_get_state());
}
export function setPackageOption(options) {
    return JSON.parse(configurationOps().op_clay_configuration_set_package_option(JSON.stringify(options ?? null)));
}
export function setModePreference(options) {
    void options;
    return plannedConfigurationApi("clay.configuration.setModePreference");
}
export function setDecorationTheme(options) {
    void options;
    return plannedConfigurationApi("clay.configuration.setDecorationTheme");
}
export function setParsePolicy(options) {
    void options;
    return plannedConfigurationApi("clay.configuration.setParsePolicy");
}
