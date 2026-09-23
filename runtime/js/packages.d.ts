export declare function serverValidatePackageManifest(manifest: unknown): unknown;
export declare function serverValidatePackagePermissions(permissions: unknown): unknown;
export declare function serverLoadPackage(packageJson: unknown): unknown;
export declare function serverListFirstPartyPackageSpecifiers(): string[];
/** Install a package from an npm-compatible specifier and record provenance.
 * Planned: not callable until the op wiring and authorization flow ship. */
export declare function install(_options: {
    specifier: string;
} & Record<string, unknown>): never;
/** Enable an installed, authorized package and evaluate its package graph.
 * Planned: not callable until the op wiring ships. */
export declare function enable(_options: {
    packageName: string;
} & Record<string, unknown>): never;
/** Disable an enabled package and withdraw its contributions.
 * Planned: not callable until the op wiring ships. */
export declare function disable(_options: {
    packageName: string;
} & Record<string, unknown>): never;
/** Inspect package metadata, provenance, capabilities, and authorization state.
 * Planned: not callable until the op wiring ships. */
export declare function inspect(_options: {
    packageName: string;
} & Record<string, unknown>): never;
/** List installed/bundled packages with provenance and authorization status.
 * Planned: not callable until the op wiring ships. */
export declare function list(): never;
/** Options for {@link authorize}. */
export interface PackageAuthorizeOptions {
    /** Installed package name or original requested source specifier. */
    package: string;
    /** Capabilities to approve; each must be declared by the package manifest. */
    capabilities: string[];
    /** Runtime profile recorded with the grant (default `native-trust`). */
    runtimeProfile?: "native-trust" | "sandboxed" | "restricted";
    /** Optional provenance match: the original requested specifier or resolved name. */
    source?: string;
    /** Who approved the grant; package activation can never grant. */
    approvedBy: "user" | "cli" | "config";
}
/** Recorded capability grant returned by {@link authorize}. */
export interface PackageAuthorizationGrant {
    packageName: string;
    version: string;
    sourceKind: string;
    capabilities: string[];
    runtimeProfile: "native-trust" | "sandboxed" | "restricted";
    approvedBy: string;
    granted: true;
}
/** Authorize capabilities and a runtime profile for an installed package.
 *
 * Records an explicit user/CLI/config capability grant against the installed
 * package's provenance. Trusted-only and refused during package activation,
 * so package code can never grant capabilities to itself or another package.
 * Every granted capability must be declared by the package manifest, and the
 * grant is what lets `loadPackage` enable a package that requests powerful
 * capabilities. Grants are visible in `clay package inspect` and withdrawn by
 * `clay package revoke`. */
export declare function authorize(options: PackageAuthorizeOptions): PackageAuthorizationGrant;
/** Set an explicit user-selected winner for a package contribution conflict.
 * Planned: not callable until the op wiring ships. */
export declare function setConflictOverride(_options: {
    contributionId: string;
    winnerPackage: string;
} & Record<string, unknown>): never;
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
export declare function loadPackage(specifier: string): Promise<unknown>;
