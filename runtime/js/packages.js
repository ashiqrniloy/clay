// Trusted Clay package facade. Third-party runtimes cannot import this module.
// Host ops validate package provenance, grants, durable adoption, graph consent,
// runtime domain, and root-confined load entries before package code executes.
const ops = globalThis.Deno?.core?.ops;
// Per-runtime-generation cache: repeated calls are idempotent inside one
// ClayJsRuntimeService. Hot reload invalidates it by swapping to a fresh
// runtime rather than mutating globals/module cache in place, adding a
// force flag, or invoking package-authored reload/migration hooks.
// Generation-local only: this object is empty in every candidate generation.
const loadedPackages = (globalThis.__clayLoadedPackages ??= Object.create(null));
function requireOps() {
    if (!ops) {
        throw new Error("clay.packages.runtime_unavailable: Clay package APIs require the server runtime");
    }
    return ops;
}
function parse(json) {
    return JSON.parse(json);
}
export function serverValidatePackageManifest(manifest) {
    return parse(requireOps().op_clay_packages_validate_manifest(JSON.stringify(manifest ?? null)));
}
export function serverValidatePackagePermissions(permissions) {
    return parse(requireOps().op_clay_packages_validate_permissions(JSON.stringify(permissions ?? null)));
}
export function serverLoadPackage(packageJson) {
    return parse(requireOps().op_clay_packages_load_package(JSON.stringify(packageJson ?? null)));
}
export function serverListFirstPartyPackageSpecifiers() {
    return parse(requireOps().op_clay_packages_list_first_party_specifiers()).specifiers;
}
function plannedPackageApi(name) {
    const unavailable = Deno?.core?.ops?.op_clay_runtime_unavailable;
    if (typeof unavailable === "function") {
        unavailable(name);
    }
    throw new Error(`${name} is planned; Clay package management op wiring is not implemented yet`);
}
/** Install a package from an npm-compatible specifier and record provenance.
 * Planned: not callable until the op wiring and authorization flow ship. */
export function install(_options) {
    return plannedPackageApi("clay.packages.install");
}
/** Enable an installed, authorized package and evaluate its package graph.
 * Planned: not callable until the op wiring ships. */
export function enable(_options) {
    return plannedPackageApi("clay.packages.enable");
}
/** Disable an enabled package and withdraw its contributions.
 * Planned: not callable until the op wiring ships. */
export function disable(_options) {
    return plannedPackageApi("clay.packages.disable");
}
/** Inspect package metadata, provenance, capabilities, and authorization state.
 * Planned: not callable until the op wiring ships. */
export function inspect(_options) {
    return plannedPackageApi("clay.packages.inspect");
}
/** List installed/bundled packages with provenance and authorization status.
 * Planned: not callable until the op wiring ships. */
export function list() {
    return plannedPackageApi("clay.packages.list");
}
/** Authorize capabilities and a runtime profile for a package.
 * Planned: not callable until the op wiring ships. */
export function authorize(_options) {
    return plannedPackageApi("clay.packages.authorize");
}
/** Set an explicit user-selected winner for a package contribution conflict.
 * Planned: not callable until the op wiring ships. */
export function setConflictOverride(_options) {
    return plannedPackageApi("clay.packages.setConflictOverride");
}
/** Load and activate an installed, user-authorized package by specifier.
 *
 * This is the one-line default end-user package loader (e.g.
 * `await loadPackage("@clay/markdown")`, `await loadPackage("@vendor/foo")`,
 * or `await loadPackage("github:user/repo")` from `~/.config/clay/init.js`). It
 * resolves + validates + authorizes + enables the package through the
 * authoritative PackageService path. Third-party packages require prior CLI
 * adoption and execute through the Rust bridge in the shared third-party
 * runtime; JavaScript cannot approve itself or promote into trusted runtime.
 * The load entry then registers inert contributions. Repeated calls within one runtime
 * generation return the cached summary; hot reload reruns `init.js` in a fresh
 * generation so the cache starts empty and every `loadEntry` rebuilds
 * declarations from durable configuration/package metadata. There is no
 * `loadPackage(spec, { force: true })` and no package reload callback API.
 * The module loader imports only canonical loadEntry paths recorded in the
 * validated package allowlist, and relative imports remain confined to the
 * package root. */
export async function loadPackage(specifier) {
    if (typeof specifier !== "string") {
        throw new Error("clay.packages.invalid_specifier: loadPackage requires a string specifier");
    }
    if (loadedPackages[specifier]) {
        return loadedPackages[specifier];
    }
    const result = parse(requireOps().op_clay_packages_load_package_by_specifier(JSON.stringify({ specifier })));
    if (result.domain === "third-party") {
        // Approved third-party packages never execute in this (trusted) runtime:
        // the host bridge evaluates their load entry in the third-party runtime
        // and absorbs the registration payload (Plan 061 task 12).
        parse(await requireOps().op_clay_packages_load_in_package_domain(JSON.stringify(result)));
        loadedPackages[specifier] = result;
        return result;
    }
    // Import the validated on-disk loadEntry, then invoke its default export so
    // the package activates (registers modes/commands/parse handlers) under Clay's
    // authority. The loadEntry contract is: a module whose default export is the
    // activation function. Curated `clay:` facade imports inside the loadEntry are
    // always allowed by the module loader; no new authority is granted here.
    // The host stamps package provenance and enters the package-activation
    // scope for the registration calls inside the activation; the `finally`
    // ends that scope so later user-configuration statements in the same
    // init.js run outside package code again (the provenance stamp persists
    // for attribution of later package-facing calls).
    try {
        const loadEntry = await import(result.loadEntrySpecifier);
        if (typeof loadEntry.default === "function") {
            await loadEntry.default();
        }
    } finally {
        requireOps().op_clay_packages_end_package_activation();
    }
    loadedPackages[specifier] = result;
    return result;
}
