# Unified Package Runtime Authority

## Source

- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
- `src/packages/manager.rs`
- `src/packages/service.rs`
- `src/packages/manifest.rs`
- `src/packages/record.rs`
- `src/packages/permissions.rs`
- `src/packages/conflict.rs`
- `src/server/ops/packages.rs`
- `src/server/js_runtime.rs`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/package-loading.md`

## Scope

Clay uses one package authority model for Clay-shipped and user-installed packages. Package source (`@clay/*`, npm, GitHub, git URL, tarball, or local path) affects default trust prompts and provenance display, but not the capabilities a user can grant.

Authority boundaries remain separate:

```text
install != enable != load != runtime execution != package-manager execution != client behavior delivery
```

## Current Primitive Inventory

- **Install:** `PackageService::install` delegates package download, registry access, dependency resolution, lockfile/integrity/caching, and package-store mutation to `PackageManagerBackend` / `PnpmBackend`. Install records package files and metadata; it does not enable or run `loadEntry`.
- **Enable:** `PackageService::enable` reads installed `package.json`, builds a `PackageRecord` through `assemble_package_record`, then runs `check_enabled_packages`. Today conflicts reject; the target model evolves this into explicit user/package override, extend, disable, and replace policy.
- **Load:** `op_clay_packages_load_package_by_specifier` currently resolves only `@clay/*` from the bundled first-party package root. Plan 035 replaces this limitation with a source-aware resolver that can load enabled npm/GitHub/git/local packages through the same validation path.
- **Runtime execution:** `ClayModuleLoader` currently admits only resolver-recorded load entries. Target runtime keeps root confinement and provenance, but generalizes allowlist entries to all enabled user-authorized packages.
- **Package-manager execution:** Package-manager stdout/stderr/exit code, lockfiles, and discovered `package.json` metadata are provenance/diagnostics, not automatic enablement. User authorization plus Clay metadata validation controls enable/load.
- **Client behavior delivery:** Clients receive validated manifests, SDUI/protocol updates, decorations, parse updates, diagnostics, and other approved state. Client-side package JavaScript/native UI becomes possible only through explicit future capabilities and APIs.
- **Capabilities:** Current code knows narrow package permissions plus newly documented grantable host capabilities. All packages use the same vocabulary; source does not impose a permanent ceiling.

## Target Authority Model

A Clay package identity is:

```text
source + package name + version/resolved identity + package root + clay.apiPrefix + enabled state + user-approved capabilities
```

Source examples:

```bash
clay package add @clay/markdown
clay package add @vendor/package
clay package add github:user/repo
clay package add https://github.com/user/repo.git
clay package add ./local-package
```

Clay should show source and requested capabilities before enable, then record user/admin approval. Package manifest declarations are requests; user authorization is the grant.

## Grantable Capabilities

Initial target capability vocabulary:

- `mode-registration`
- `mode-activation`
- `command-registration`
- `package-configuration`
- `parse-document`
- `render-decorations`
- `render-folding`
- `completion-provider`
- `package-control`
- `package-import`
- `filesystem`
- `network`
- `shell`
- `wasm`
- `ai-tools`
- `workspace-mutation`
- `native-ui`
- `client-runtime`
- `raw-ops`

These are powerful and must be visible, revocable, and diagnosable. They are not categorically forbidden for third-party packages.

## Package Graph and Package Control

Packages may declare graph relations:

```json
{
  "clay": {
    "apiPrefix": "example",
    "loadEntry": "./dist/load.js",
    "capabilities": ["mode-registration", "package-control"],
    "dependsOn": ["@clay/markdown"],
    "extends": ["@clay/markdown"],
    "disables": ["@clay/markdown"],
    "replaces": ["@clay/markdown"]
  }
}
```

Rules:

- `dependsOn`: target package loads first and may be imported/used internally when `package-import` is granted.
- `extends`: both packages remain active; extender can register additive behavior.
- `disables`: target package is disabled when user grants `package-control`.
- `replaces`: target package is disabled and replacement may claim its package slots/modes through explicit conflict policy.
- Same rules apply to Clay and non-Clay packages.

## Conflict Resolution Target

Current `check_enabled_packages` gives useful deterministic diagnostics but rejects all collisions. Target resolution order:

1. explicit user configuration;
2. package `replaces` / `extends` declarations;
3. package priority/precedence metadata;
4. deterministic diagnostic fallback.

No silent load-order wins. Conflicts should include package name, version, source, apiPrefix, contribution ID, requested relation, and selected winner/loser.

## Runtime Profiles

Runtime profile is a user/config choice, not a first-party/third-party distinction:

```text
native-trust | sandboxed | restricted
```

Any source may use any implemented profile when the user grants it. Sandboxing remains useful as an optional runtime profile, not as proof that third-party packages are second-class.

## Hot-Path Policy

Install, provenance lookup, authorization prompts, enable/load validation, package graph changes, conflict resolution, reload, rollback, and package-manager calls run at startup, user command, configuration, reload, or background time. They do not run in keypress, paint, layout, scroll, text-event, edit-ack, or Masonry hot paths.

## Package-Scoped Disable, Rollback, and Revocation

`PackageService::disable` is active withdrawal. It routes through `revoke_enabled_package`, removes the package from the enabled `PackageRecord` set, increments a monotonic package generation, stores a `PackageRevocationRecord` with `PackageContributionWithdrawalCounts` (commands, behavior manifests, SDUI, parse handlers, decorations, completions, layout, input, state, theme, diagnostics), and removes conflict resolutions involving the disabled package. Failed enable/graph/conflict attempts snapshot and restore enabled records, conflict diagnostics, revocation records, and package generation so rollback keeps the previous valid generation active.

Runtime hooks reuse existing generation primitives: `ParseCoordinator::cancel_package` removes package-owned parse handlers and aborts in-flight tasks through the same abort path as `cancel_generation`, while `PackageLoadEntryAllowlist::revoke_package` withdraws package-owned load entries and transitive module entries so no orphaned imports remain.

## Remaining Implementation Work

Plan 035 implemented the unified authority model, source-aware resolver, authorization records, package graph, conflict resolution, and package-scoped revocation. Work that remains for future plans includes:

- Durable persisted installed/enabled/authorization/revocation state across server process restarts.
- End-user callable Clay JS APIs and CLI commands for `authorize`, `enable`, `disable`, `inspect`, `list`, and `setConflictOverride` (Rust primitives exist; op wiring is planned).
- Production sandbox profile routing (the `RuntimeProfile` enum and authorization storage exist; separate-process sandbox harness wiring is deferred).
- Package-import boundary checks for `package-import` capability and internal cross-package module use.
- Richer user-facing conflict precedence configuration beyond user overrides, package-control graph actions, and key-binding priority.
- Publication-side wiring for withdrawing all package-owned contribution caches beyond enabled records, parse handlers, and module load entries.

## Tests

Run focused coverage with:

```text
cargo test --test package_loading_docs unified_package_authority_model_is_documented
cargo test --test package_loading_docs package_loading_keeps_validation_and_parsing_out_of_typing_hot_path
cargo test --test package_loading
 cargo test --test package_graph
 cargo test --test package_conflicts
 cargo test --test parse_coordinator
 cargo test --test clay_js_api_inventory
 cargo test --test rust_visibility_api_mapping
```

## Related

- [Package Loading](package-loading.md)
- [Persistent Runtime Hardening](persistent-runtime-hardening.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Parse Coordinator](parse-coordinator.md)
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/package-loading.md`
- `plans/035-Third-Party-Package-Runtime-Authority-Policy.md`
