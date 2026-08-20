# Unified Package Authority Primitive Review (Historical)

> **Superseded:** this page records the pre-Plan-061 single-runtime/`RuntimeProfile` review and Plan 035 outcome. Current execution authority is documented in [Package Extension and Adoption Authority](third-party-runtime-authority.md) and [Embedded JavaScript Runtime](embedded-js-runtime.md): exact bundled inventory selects the trusted domain, all other adopted packages execute in the shared third-party domain, and normal approval cannot promote them. Statements below about `@clay/*` auto-trust or future profile routing are historical and must not be used as current security guidance.

## Source

- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/package-loading.md`
- `docs/wiki/modules/third-party-runtime-authority.md`
- `src/packages/manager.rs`
- `src/packages/service.rs`
- `src/packages/manifest.rs`
- `src/packages/record/mod.rs`
- `src/packages/conflict.rs`
- `src/packages/permissions.rs`
- `src/server/ops/packages.rs`
- `src/server/js_runtime/mod.rs`
- `src/server/parse_coordinator.rs`
- `src/server/decorations.rs`
- `src/server/ui.rs`
- `src/shell/layout.rs`

## Scope

This review inventories existing generic package/runtime/editor primitives before Plan 035 adds source-aware package work. The goal is to reuse existing primitives first, identify only generic gaps, and avoid package-source-specific Rust branches.

The target flow is:

```text
PackageSource -> PackageAuthorization -> PackageGraph -> RuntimeGeneration
```

The implementation must preserve Clay's authority split:

```text
install != enable != load != runtime execution != package-manager execution != client behavior delivery
```

## Existing Primitive Inventory

| Area | Existing primitive | What it already provides | Current gap for unified package authority |
| --- | --- | --- | --- |
| Package manager boundary | `PackageManagerBackend`, `PnpmBackend`, `PackageStore`, `InstallResult`, `DiscoveredPackage` in `src/packages/manager.rs` | Delegates fetching, dependency resolution, lockfiles, integrity, caching, GitHub/git/local handling, and process execution to npm-compatible tooling; suppresses lifecycle scripts by default. | Needs persistent `PackageSource` / provenance records for requested spec, resolved identity, source kind, package root, lockfile, integrity, and sanitized diagnostics. |
| Package service | `PackageService`, `InstalledPackage`, `PackageInspection`, `PackageRevocationRecord`, and `PackageContributionWithdrawalCounts` in `src/packages/service.rs` | Separates install from enable/load, caches inert package metadata and source-aware package roots, validates and checks authorization before enable, evaluates package graph relations, rolls back failed conflict/graph candidates, records package-scoped revocation generations/counts, lists/inspects enabled state, requested/approved capabilities, runtime profile, and provenance. | Needs durable persistent installed/enabled/authorization/revocation state and package-import boundary checks. |
| Manifest validation | `validate_manifest_value`, entry/loadEntry path validation, API prefix validation, payload budget checks in `src/packages/manifest.rs` | Validates Clay metadata, prefix, entries, modes, compatibility `permissions`, `capabilities`, package graph relations (`dependsOn`, `extends`, `disables`, `replaces`), forbidden runtime metadata, and bounded manifest payloads. | Needs runtime profile metadata schema. |
| Package records | `assemble_package_record` in `src/packages/record/mod.rs` | Produces typed package records with contribution provenance for commands, key routing, configuration, SDUI, decorations, package UI, input, state scopes, layout overrides, package options, docs, performance, and API dependencies. | Needs source identity and authorization identity in the record so diagnostics and revocation can find all package-owned contributions. |
| Capability parser | `PackagePermission` / `parse_permission` in `src/packages/permissions.rs` | Uses one vocabulary for existing package permissions plus grantable capabilities such as `package-control`, `package-import`, `filesystem`, `network`, `shell`, `wasm`, `ai-tools`, `workspace-mutation`, `native-ui`, `client-runtime`, and `raw-ops`; `PackageAuthorizationRecord` enforces that manifest requests are not grants. | Needs persistent authorization storage and per-boundary checks beyond enable. |
| Conflict detection/resolution | `check_enabled_packages`, `PackageConflictResolutionPolicy`, and `PackageConflictResolutionDiagnostic` in `src/packages/conflict.rs` | Deterministically detects duplicate prefixes, modes, commands, key bindings, configuration, SDUI, decorations, UI panels/components/overlays/theme tokens, input/state/layout/package options, and behavior entries; resolves only explicit user overrides, package-control `replaces`/`disables`, and distinct key-binding priority/routing metadata with winner/loser diagnostics. | Needs richer user configuration persistence/UI and future precedence schemas for package-specific primitive categories. |
| `loadPackage` resolver | `op_clay_packages_load_package_by_specifier` and `PackageLoadEntryAllowlist` in `src/server/ops/packages.rs` | Reuses `PackageService::enable`, resolves bundled and installed source-aware package specifiers, validates canonical `loadEntry`, confines package imports to the canonical package root, records resolver-validated load entries, and returns an opaque `clay://packages/...` specifier. | Needs durable runtime hydration of installed/authorized package state across process restarts. |
| Runtime module loading | `ClayModuleLoader` and `PackageLoadEntryAllowlist` in `src/server/js_runtime/source.rs` / `src/server/ops/packages.rs` | Loads curated `clay:*` facades, configuration-root relative modules, and recorded package load-entry modules; records package ownership for loadEntry/transitive imports; `PackageLoadEntryAllowlist::revoke_package` withdraws package-owned module entries; keeps V8 work on server workers and uses timeout/heap diagnostics. | Needs package-import graph checks; the hot path remains map lookup and file read only. |
| Runtime generations | `ClayJsRuntimeService`, runtime generation swap, hot reload docs/tests, package generation records | Rebuilds runtime generations, reruns `init.js`, drops old load-entry cache, keeps prior generation on failed reload, and records package-generation revocation for package disable/rollback. | Needs durable runtime profile storage and stale package-output rejection across future contribution caches beyond enabled records/parse/loadEntry. |
| Mode activation | `DocumentClassification`, `MajorModeActivation`, `serverRegisterModePattern`, `serverActivateMajorMode` | Server owns classification and activation; client receives installed behavior manifests and mode state. | Needs package graph ordering so extended/replaced modes compose or disable deterministically. |
| Commands/key routing/text transforms | command registry, keybinding APIs, behavior manifests | Commands and key routes are registered as inert metadata; client-first text transforms remain Rust-known manifest data. | Needs conflict resolution and package-control/import-aware precedence without package-specific client branches. |
| Parse/decorations/folding/completion | `ParseCoordinator`, `IncrementalParseUpdate`, `DecorationSet`, `DecorationSpan`, folding/completion registry rows | Background server-side work is cancellable, versioned, bounded, publication-validated before client delivery, and package-scoped cancellation via `ParseCoordinator::cancel_package` withdraws handlers and aborts in-flight tasks on revocation. | Needs authorization checks for requested package capability at registration/request/publication boundaries beyond current enable-time grants. |
| SDUI/package UI/layout/input/state/configuration | `src/server/ui.rs`, `src/shell/layout.rs`, `clay:ui`, `clay:configuration`, package UI validators | Packages declare inert UI/component/layout/input/state/configuration contributions; Clay owns Masonry/native layout and validates payloads. | Needs package graph precedence and package-scoped withdrawal for all contribution indexes. |
| Client delivery | protocol messages, behavior manifests, SDUI, decorations, diagnostics | Client receives validated inert data and server-routed intents; no package JavaScript runs in Masonry paint/layout/input/text handlers. | Future `client-runtime`/`native-ui` capability must be a separately documented API; Plan 035 should not smuggle it through package loading. |
| Sandbox/profile | persistent runtime sandbox design and hardening docs | Separate-process sandbox harness and runtime profile design exist as hardening primitives. | Needs `native-trust | sandboxed | restricted` profile selection tied to package authorization and production routing. |

## What Existing Primitives Already Achieve

Clay can already validate package metadata, keep install separate from runtime execution, reject invalid prefixes and entry paths, preserve package provenance on contributions, reject deterministic conflicts, load a bundled package through `loadPackage`, confine validated `loadEntry` imports to a package root, register modes/commands/UI/configuration/parse primitives through Clay facades, publish only inert client state, cancel stale parse work, and preserve editor hot paths from package JavaScript.

Those primitives are enough to implement the first source-aware path without adding mode-specific or source-specific editor behavior. Plan 035 should extend package identity, authorization, graph, and resolver records around the existing validators instead of adding a separate third-party loader.

## Generic Primitive Gaps Filled by Plan 035

Plan 035 extended the existing primitives below rather than adding source-specific branches. Gaps that remain are durable persistence, end-user callable API wiring, and future contribution-cache withdrawal.

1. **PackageSource provenance primitive** — Implemented: `PackageSourceKind` (`ClayShipped`, `NpmRegistry`, `GitHub`, `GitUrl`, `Tarball`, `LocalPath`) and `PackageProvenance` record requested spec, source kind, resolved name/version, canonical package root, lockfile, integrity, and sanitized diagnostics bounded to 4 KB with token/password redaction. `DiscoveredPackage`, `InstalledPackage`, and `PackageInspection` carry provenance.
2. **PackageAuthorization primitive** — Implemented: `PackageAuthorizationRecord` stores user/admin-approved capabilities and `RuntimeProfile` for a package identity/source. `authorize_package` records grants; `enable` fails closed with `MissingCapabilityGrant` when a requested capability is not granted. `@clay/*` packages are auto-authorized with `NativeTrust` and `clay-bundled-default`.
3. **PackageGraph primitive** — Implemented: `PackageGraphRelations`, `PackageGraphPlan`, and `PackageService::enable` parse and validate `dependsOn`, `extends`, `disables`, and `replaces`, enable dependencies/extensions first, require `package-control` for disables/replaces, roll back failed graph candidates, and report missing targets/cycles deterministically.
4. **ConflictResolution primitive** — Implemented: `PackageConflictResolutionPolicy`, `PackageConflictResolutionDiagnostic`, and `PackageService::reconcile_enabled_conflicts` replace hard conflict rejection for explicit user overrides, package-control graph replacement/disable, and distinct key-binding priorities while preserving deterministic diagnostic fallback and no silent load-order wins.
5. **PackageLoadEntryRegistry primitive** — Implemented: `PackageLoadEntryAllowlist` is source-agnostic and records validated package entries keyed by opaque package module specifier, canonical root, and package owner; `revoke_package` evicts loadEntry/transitive module entries for one package.
6. **PackageGenerationRevocation primitive** — Implemented: `PackageRevocationRecord`, `PackageContributionWithdrawalCounts`, `PackageService::revoke_enabled_package`, `ParseCoordinator::cancel_package`, and `PackageLoadEntryAllowlist::revoke_package` track/withdraw package-owned contributions and runtime hooks so disable/revoke/update affects one package without corrupting others.
7. **RuntimeProfile primitive** — Implemented: `RuntimeProfile` enum (`NativeTrust`, `Sandboxed`, `Restricted`) is stored in `PackageAuthorizationRecord`. Production sandbox harness routing remains optional/deferred.
8. **PackageInspection/Diagnostics primitive** — Implemented: `PackageInspection` exposes source, requested capabilities, approved capabilities, runtime profile, graph relations, and enabled status. End-user callable inspect/list API wiring is planned; current inspection is used by tests and internal diagnostics.

## Hot-Path Policy

Install, source/provenance discovery, authorization prompts, enable/load validation, package graph evaluation, conflict resolution, runtime profile selection, package generation swap, disable/revoke, and package-manager calls happen during startup, install, enable, load, reload, explicit user command, or background audit work.

Request/publication boundaries may perform cheap checks against already-loaded authorization and generation state, such as package ID, generation ID, handler token, document version, payload size, and contribution ownership.

No package source resolution, package-manager call, authorization prompt, graph traversal, JavaScript evaluation, or configuration evaluation may run from keypress, paint, layout, scroll, text-event, edit-ack, pointer, or Masonry hot paths.

## Security and Authority Boundaries

- Keep package-root confinement for `entry`, `loadEntry`, and transitive package imports.
- Keep explicit user grants visible, revocable, and tied to source/provenance.
- Keep package-manager metadata diagnostic-only; install does not imply enable/load/runtime execution.
- Keep runtime JavaScript server-side unless a future `client-runtime` capability and API are deliberately implemented.
- Keep Clay-owned clients receiving validated manifests, SDUI/protocol updates, decorations, parse updates, diagnostics, or other inert state.
- Keep powerful capabilities such as filesystem, network, shell, WASM, AI tools, workspace mutation, native UI, client runtime, package-control, and raw ops behind documented APIs, diagnostics, revocation behavior, and tests.
- Do not add source-specific Rust branches such as `if github_package`, `if npm_package`, or `if third_party`; source handling belongs in the generic `PackageSource` resolver/provenance primitive.

## Implementation Outcome

Plan 035 implemented the unified package authority model by extending the existing primitives rather than adding source-specific Rust branches:

1. Source/provenance records were added to the package manager/service boundary before resolver widening.
2. Authorization records and grant checks were added before treating requested capabilities as active.
3. Graph relation parsing and evaluation were added before allowing package-control over Clay-shipped or user-installed packages.
4. Load-entry allowlist naming and records were generalized before adding npm/GitHub/local package loading.
5. Package-scoped ownership indexes were added before implementing disable/revoke/update incident behavior.
6. Clay JS API/configuration docs were updated only for public surfaces; internal records remain server-owned until intentionally exposed via planned op wiring.

## Remaining Gaps

- Durable persisted installed/enabled/authorization/revocation state across server process restarts.
- End-user callable Clay JS APIs and CLI commands for `authorize`, `enable`, `disable`, `inspect`, `list`, and `setConflictOverride` (Rust primitives exist; op wiring is planned).
- Production sandbox profile routing (enum and authorization storage implemented; harness wiring deferred).
- Package-import boundary checks for cross-package internal module use.
- Richer user-facing conflict precedence and revocation persistence surfaces.

## Tests

Run focused coverage with:

```text
cargo test --test protocol package_loading_docs::
cargo test --test protocol package_loading_docs::
cargo test --test security package_loading::
 cargo test --test security package_graph::
 cargo test --test security package_conflicts::
 cargo test --test runtime parse_coordinator::
 cargo test --test protocol clay_js_api_inventory::
 cargo test --test security rust_visibility_api_mapping::
 cargo test --test protocol primitives_docs::
```

## Related

- [Unified Package Runtime Authority](third-party-runtime-authority.md)
- [Package Loading](package-loading.md)
- [Primitive Architecture](primitive-architecture.md)
- [Parse Coordinator](parse-coordinator.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/package-loading.md`
- `plans/035-Third-Party-Package-Runtime-Authority-Policy.md`
