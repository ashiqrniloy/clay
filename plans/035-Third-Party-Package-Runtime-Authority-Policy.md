# Unified User-Authorized Package Authority

## Objectives

- Replace the strict third-party deny-first model with one package authority model for Clay-shipped and user-installed packages.
- Allow npm, GitHub/git, tarball, and local path packages to install and load through the same Clay package service/runtime path.
- Let users grant any Clay-defined package capability to any package source, including package-control over first-party and third-party packages.
- Preserve hot-path safety: package install, authorization, graph resolution, conflict resolution, load, reload, and rollback never run in typing/paint/layout/scroll/text-event/edit-ack hot paths.

## Expected Outcome

- `loadPackage` can resolve enabled, user-authorized packages from bundled Clay packages, npm, GitHub/git, tarball, and local paths.
- Clay records source provenance and user-approved capabilities for packages without treating third-party packages as permanently less capable.
- Packages can declare `dependsOn`, `extends`, `disables`, and `replaces`; Clay applies those relations when user-approved.
- Previous Plan 035 strict documentation/tests/comments are replaced with the approved unified authority model from `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`.

## Tasks

- [x] Review existing package/runtime primitives and remove strict third-party policy leftovers
  - Acceptance Criteria:
    - Functional: Inventory current install, enable, load, runtime execution, package-manager, sandbox/profile, client behavior, permission, and conflict primitives; remove or rewrite docs/tests/comments that require non-`@clay/*` packages to remain blocked by policy.
    - Performance: Inventory confirms all package policy work remains startup/install/enable/load/reload/background/user-command work, not editor hot-path work.
    - Code Quality: Reuse `PackageService`, `PackageManagerBackend`, manifest validation, conflict diagnostics, runtime generation, and module-loader allowlist primitives before adding new code.
    - Security: Keep validation, package-root confinement, provenance diagnostics, and explicit user authorization; do not keep categorical third-party bans.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`
      - `docs/reference/primitives/package-security.md`
      - `docs/reference/primitives/package-loading.md`
      - `docs/wiki/modules/third-party-runtime-authority.md`
      - `docs/wiki/modules/persistent-runtime-hardening.md`
    - Options Considered:
      - Keep strict Plan 035 as an intermediate gate: rejected; user rejected the model.
      - Rewrite policy docs first, then implement source-aware loading: chosen to keep tests aligned with the approved target.
    - Chosen Approach:
      - Treat first-party-only code as a current implementation limitation, not a policy requirement. Add docs-as-code coverage for the unified model.
    - API Notes and Examples:
      ```text
      package authority = source + manifest + enabled state + user-approved capabilities
      install != enable != load != runtime execution != package-manager execution != client behavior delivery
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/third-party-runtime-authority.md`: Rewrite as unified package runtime authority.
      - `docs/wiki/modules/persistent-runtime-hardening.md`: Replace third-party gate language with unified capability/profile language.
      - `docs/design/persistent-runtime-sandbox.md`: Make sandbox an optional runtime profile for any package source.
      - `docs/reference/primitives/package-security.md`: Replace denied third-party policy with unified capability authorization.
      - `docs/reference/primitives/package-loading.md`: Replace strict deferrals with source/provenance target model.
      - `tests/package_loading_docs.rs`: Replace strict gate tests with unified authority tests.
      - `src/server/ops/packages.rs`: Update strict comments to current-limitation language.
      - `src/packages/permissions.rs`: Accept target capability names instead of categorically prohibiting host capabilities.
    - References:
      - `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
  - Test Cases to Write:
    - `cargo test --test package_loading_docs unified_package_authority_model_is_documented`: Unified authority docs, decision log, permissions parser, and resolver comments are aligned.
  - Completed Notes:
    - Rewrote the Plan 035 strict policy surface into the approved unified user-authorized model across package authority, package security, package loading, Clay JS API loadPackage docs/inventory, package author docs, persistent runtime hardening, sandbox/profile, package-loading wiki, and Phase 18.5/18.7/19 runtime primitive docs.
    - Updated `src/packages/permissions.rs` so powerful package capabilities parse as grantable capabilities instead of categorically prohibited third-party authorities.
    - Updated `src/server/ops/packages.rs` comments so the current `@clay/*` resolver is documented as a temporary source-aware loading implementation limit, not policy.
    - Replaced strict docs-as-code gates with unified authority coverage in `tests/package_loading_docs.rs`.
    - Verified with `cargo test --test package_loading_docs`, `cargo test --test clay_js_api_inventory`, `cargo test --test primitives_docs`, `cargo test --test package_loading`, and `cargo fmt --check`.

- [x] Review existing editor primitives and plan generic primitive gaps before package work
  - Acceptance Criteria:
    - Functional: Inventory generic primitives needed for source-aware package loading, package authorization, package graph evaluation, conflict resolution, runtime profile selection, package-scoped revocation, and client delivery.
    - Performance: Identify which checks happen at install/enable/load/reload/background time and which cheap checks may occur at request/publication boundaries.
    - Code Quality: New primitives are generic and reusable across packages/modes; no package-source-specific Rust branches beyond source resolution.
    - Security: Primitive review preserves package-root confinement, explicit user grants, revocation, diagnostics, and no unapproved hot-path JavaScript.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`
      - `docs/reference/primitives/registry.md`
      - `docs/wiki/modules/primitive-architecture.md`
      - `docs/reference/primitives/package-security.md`
      - `docs/reference/primitives/package-loading.md`
    - Options Considered:
      - Add one-off third-party branches to loader: rejected.
      - Add generic package-source, package-authorization, and package-graph primitives: chosen.
    - Chosen Approach:
      - Document gaps before implementation and only add generic primitives.
    - API Notes and Examples:
      ```rust
      PackageSource -> PackageAuthorization -> PackageGraph -> RuntimeGeneration
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/unified-package-authority-primitive-review.md`: New primitive inventory/gap page.
      - `docs/wiki/index.md`: Link primitive review.
      - `tests/package_loading_docs.rs`: Add primitive-review docs guard.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
  - Test Cases to Write:
    - `cargo test --test package_loading_docs unified_package_authority_primitive_review_is_documented`: Requires primitive inventory and generic gap list.
  - Completed Notes:
    - Created `docs/wiki/modules/unified-package-authority-primitive-review.md` with a primitive inventory covering package-manager, package-service, manifest, record, capability parser, conflict detection, `loadPackage` resolver, module loader, runtime generation, mode/command/parse/decorations/UI/client delivery, and sandbox/profile primitives.
    - Documented generic gaps before package work: `PackageSource`, `PackageAuthorization`, `PackageGraph`, `ConflictResolution`, `PackageLoadEntryRegistry`, `PackageGenerationRevocation`, `RuntimeProfile`, and package inspection/diagnostics primitives.
    - Documented that install/provenance/authorization/enable/load/graph/conflict/reload/revocation work stays at startup/install/enable/load/reload/user-command/background time, while request/publication boundaries may only perform cheap loaded-state checks.
    - Preserved security requirements for package-root confinement, explicit revocable user grants, diagnostic-only package-manager metadata, server-side package JavaScript, inert client delivery, powerful capability APIs, and no source-specific Rust branches.
    - Linked the new primitive review from `docs/wiki/index.md` and refreshed stale index descriptions for unified package authority.
    - Added `cargo test --test package_loading_docs unified_package_authority_primitive_review_is_documented` as a docs-as-code guard.
    - Verified with `cargo test --test package_loading_docs unified_package_authority_primitive_review_is_documented`.

- [x] Add source-aware package install and provenance records
  - Acceptance Criteria:
    - Functional: Package service records requested spec, resolved package name/version, source kind, package root, lockfile/integrity when available, and diagnostics for npm, GitHub/git, tarball, and local path installs.
    - Performance: Source/provenance parsing happens during install/refresh/inspect only.
    - Code Quality: Continue delegating fetching/resolution to npm-compatible tooling; Clay does not implement a registry client.
    - Security: Package-manager output is bounded/sanitized for diagnostics and never implies enable/load authority.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/manager.rs`
      - `src/packages/service.rs`
      - `docs/reference/primitives/package-loading.md`
    - Options Considered:
      - Build a registry/GitHub client: rejected.
      - Extend existing backend discovery/result structs: chosen.
    - Chosen Approach:
      - Add a small source/provenance type carried by installed package state.
    - API Notes and Examples:
      ```bash
      clay package add @vendor/foo
      clay package add github:user/repo
      clay package add ./local-package
      ```
    - Files to Create/Edit:
      - `src/packages/manager.rs`: Add provenance fields where available.
      - `src/packages/service.rs`: Persist installed package provenance.
      - `tests/package_loading.rs`: Add npm/GitHub/local install metadata scenarios.
      - `docs/wiki/modules/package-loading.md`: Document implementation.
    - References:
      - `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`
  - Test Cases to Write:
    - Package install provenance test: npm, GitHub/git, and local specs are recorded without enabling runtime.
  - Completed Notes:
    - Added `PackageSourceKind` and `PackageProvenance` in `src/packages/manager.rs` to classify Clay-shipped, npm, GitHub shorthand, git URL, tarball, and local-path specs and record requested spec, resolved name/version, package root, optional lockfile/integrity fields, and bounded sanitized diagnostics.
    - Updated `DiscoveredPackage`, `InstalledPackage`, and `PackageInspection` so provenance is carried from backend discovery through `PackageService::install`, `refresh_installed`, `list`, and `inspect` without enabling or executing runtime behavior.
    - Kept package fetching/resolution delegated to `PackageManagerBackend`/`PnpmBackend`; Clay only classifies delegated specs and records provenance/diagnostics at install/refresh/inspect boundaries.
    - Added diagnostics sanitization that bounds copied package-manager output and redacts token/password/authorization-like lines before exposing provenance.
    - Added package install provenance coverage for npm, GitHub shorthand, git URL, tarball, and local-path specs plus bounded/redacted diagnostics coverage in `tests/package_loading.rs`.
    - Updated `docs/wiki/modules/package-loading.md`, `docs/reference/primitives/package-loading.md`, and package-loading docs guard coverage for the implemented provenance primitive.
    - Verified with `cargo test --test package_loading`, `cargo test --test package_loading_docs`, and `cargo fmt --check`.

- [x] Add user authorization records and unified capability parsing
  - Acceptance Criteria:
    - Functional: Clay stores user-approved capabilities and runtime profile for package identity/source; manifest `permissions`/`capabilities` are requests, not grants.
    - Performance: Grant lookup occurs at enable/load/registration/request boundaries only.
    - Code Quality: Use one capability vocabulary for all package sources; no first-party/third-party enum split.
    - Security: Powerful grants are explicit, visible, revocable, and fail closed when absent.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/permissions.rs`
      - `src/packages/manifest.rs`
      - `docs/reference/primitives/package-security.md`
    - Options Considered:
      - Single trusted switch: rejected.
      - Explicit capability list: chosen.
    - Chosen Approach:
      - Add authorization record types and update manifest parser to accept `clay.capabilities` while preserving `clay.permissions` compatibility.
    - API Notes and Examples:
      ```toml
      [package_authority."@vendor/foo"]
      capabilities = ["mode-registration", "package-control", "network"]
      runtime_profile = "native-trust"
      ```
    - Files to Create/Edit:
      - `src/packages/permissions.rs`: Finalize capability vocabulary.
      - `src/packages/manifest.rs`: Parse `capabilities` and compatibility path from `permissions`.
      - `src/packages/authorization.rs`: Add authorization records.
      - `tests/package_manifest.rs` or existing package tests: Capability parsing/grant tests.
    - References:
      - Package security docs.
  - Test Cases to Write:
    - Missing grant fails enable/load for requested capability.
    - Granted filesystem/network/shell/package-control capability parses and is visible in inspection.
  - Completed Notes:
    - Added `src/packages/authorization.rs` with `PackageAuthorizationRecord` and `RuntimeProfile` to store package identity/source, approved capabilities, runtime profile, and approver.
    - Updated manifest parsing so `clay.capabilities` and compatibility `clay.permissions` both feed the same `PackagePermission` vocabulary.
    - Updated `PackageService` with `authorize_package`, authorization storage, fail-closed grant checks during `enable`, and inspection fields for requested capabilities, approved capabilities, and runtime profile.
    - Kept grant checks at enable/load boundaries; package install and inspect still record/show metadata without executing runtime behavior.
    - Updated bundled `loadPackage` resolver to authorize bundled package requested capabilities before enabling through the existing service path.
    - Added tests for missing grants failing enable and filesystem/network/shell/package-control grants parsing and appearing in inspection.
    - Updated package security/package-loading docs and docs-as-code coverage for authorization records and `clay.capabilities` compatibility.
    - Verified with `cargo test --test package_loading`, `cargo test --test markdown_mode`, `cargo test --test package_loading_docs`, and `cargo fmt --check`.

- [x] Generalize `loadPackage` and module-loader allowlist to enabled package sources
  - Acceptance Criteria:
    - Functional: `loadPackage` resolves an enabled authorized package from the package store/source registry, validates metadata through `PackageService`, records canonical `loadEntry`, and imports it through the module loader for any supported source.
    - Performance: Resolution/canonicalization happens at load/reload time only; module hot path remains allowlist lookup and file read.
    - Code Quality: Reuse package-root confinement and `PackageService::enable`; no separate third-party loader.
    - Security: `loadEntry` and transitive imports remain confined to the canonical package root unless explicit package-import/API rules allow otherwise.
  - Approach:
    - Documentation Reviewed:
      - `src/server/ops/packages.rs`
      - `src/server/js_runtime.rs`
      - `runtime/js/packages.ts`
    - Options Considered:
      - Keep bundled and installed loaders separate: rejected.
      - Generalize the existing allowlist to all enabled package roots: chosen.
    - Chosen Approach:
      - Rename/extend first-party allowlist concepts to validated package load entries and add source-aware package root lookup.
    - API Notes and Examples:
      ```javascript
      import { loadPackage } from "clay:packages";
      await loadPackage("@vendor/foo");
      await loadPackage("github:user/repo");
      ```
    - Files to Create/Edit:
      - `src/server/ops/packages.rs`: Source-aware resolver.
      - `src/server/js_runtime.rs`: Generalized package allowlist naming/logic.
      - `runtime/js/packages.ts`: Facade docs/examples.
      - `tests/package_loading.rs`: End-to-end package source load tests.
    - References:
      - `docs/reference/primitives/package-loading.md`
  - Test Cases to Write:
    - `loadPackage` loads authorized npm-style fixture.
    - `loadPackage` loads authorized GitHub/local fixture through fake backend.
    - Escaping `loadEntry`/relative import remains rejected.
  - Completed Notes:
    - Renamed the runtime load-entry allowlist primitive to `PackageLoadEntryAllowlist` and removed first-party-only module-loader language.
    - Generalized `op_clay_packages_load_package_by_specifier` to resolve installed packages by package name or original requested source specifier from `PackageService` provenance, while still seeding bundled `@clay/*` packages from Clay's shipped package directory.
    - Reused `PackageService::enable` for metadata validation, authorization grant checks, conflict detection, and enabled-record state; no separate third-party loader was added.
    - Added explicit package-root install helpers in `PackageService` for source-aware roots and requested-spec provenance.
    - Preserved load/reload-time canonicalization and allowlist recording; module loading remains an allowlist lookup plus file read, and transitive relative imports stay confined to the validated package root, including scoped package names.
    - Updated `runtime/js/packages.ts`, Clay JS API docs/inventory, primitive docs, wiki docs, and docs-as-code tests to describe source-aware `loadPackage` behavior.
    - Added runtime tests for authorized npm-style loading, authorized GitHub requested-spec loading, authorized local requested-spec loading, uninstalled specifier rejection, invalid bundled specifier rejection, and escaping relative import rejection.
    - Verified with `cargo test --test package_loading`, `cargo test --test package_loading_docs`, `cargo test --test clay_js_api_inventory`, `cargo test --lib load_package_`, `cargo test --lib op_clay_packages_load_package_by_specifier`, and `cargo fmt --check`.

- [x] Add package graph relations and package-control behavior
  - Acceptance Criteria:
    - Functional: Manifests can declare `dependsOn`, `extends`, `disables`, and `replaces`; package graph evaluation loads dependencies, applies extensions, disables/replaces targets when user grants `package-control`, and reports cycles/missing targets.
    - Performance: Graph evaluation happens at enable/load/reload/package-control time only.
    - Code Quality: Generic graph evaluator; no hard-coded package names or first-party special cases.
    - Security: Package-control cannot run without explicit grant and preserves revocation/rollback diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/manifest.rs`
      - `src/packages/service.rs`
      - `src/packages/conflict.rs`
    - Options Considered:
      - Let packages disable others imperatively from JS: rejected as too hard to audit.
      - Declarative graph relations plus Clay APIs: chosen.
    - Chosen Approach:
      - Validate graph declarations in manifest and apply them in service/runtime generation logic.
    - API Notes and Examples:
      ```json
      { "clay": { "extends": ["@clay/markdown"], "disables": ["@clay/markdown"] } }
      ```
    - Files to Create/Edit:
      - `src/packages/manifest.rs`: Parse graph relation fields.
      - `src/packages/service.rs`: Evaluate graph during enable/disable.
      - `src/packages/graph.rs`: New graph helper if needed.
      - `tests/package_graph.rs`: Graph behavior tests.
    - References:
      - Unified authority decision log.
  - Test Cases to Write:
    - Third-party package disables first-party package with `package-control` grant.
    - Package extends another package while both remain active.
    - Cycle/missing dependency diagnostics are deterministic.
  - Completed Notes:
    - Added `PackageGraphRelations` manifest metadata for `dependsOn`, `extends`, `disables`, and `replaces`, including validation for array shape, non-empty specifier strings, uniqueness, and `InvalidPackageGraph` diagnostics.
    - Added `src/packages/graph.rs` with generic `PackageGraphPlan` helpers for activation targets, controlled targets, package-control detection, and deterministic cycle paths.
    - Updated `PackageService::enable` to evaluate package graphs at enable/load/reload time only: dependencies/extensions are enabled first, disables/replaces withdraw enabled targets, failed graph/conflict candidates restore the previous enabled set, and missing targets/cycles return deterministic diagnostics.
    - Enforced `package-control` authorization for `disables` and `replaces` with a fail-closed `MissingPackageControlGrant` error; no hard-coded package names or source/first-party special cases were added.
    - Added `tests/package_graph.rs` coverage for a user-authorized package disabling a Clay-shipped package, extension activation with both packages active, missing target diagnostics, dependency cycles, and missing package-control grants.
    - Updated package security/loading reference docs, package-loading wiki, primitive review, and docs-as-code guards for package graph behavior.
    - Verified with `cargo test --test package_graph`, `cargo test --test package_loading`, `cargo test --test package_loading_docs`, `cargo test --test clay_js_api_inventory`, `cargo test --test markdown_mode`, `cargo test --lib load_package_`, `cargo test --lib op_clay_packages_load_package_by_specifier`, and `cargo fmt --check`.

- [x] Replace hard-reject conflicts with explicit conflict resolution policy
  - Acceptance Criteria:
    - Functional: Conflict detection reports duplicate/overlapping contributions and resolves by user config, package `replaces`/`extends`, explicit priority, or deterministic diagnostic fallback.
    - Performance: Conflict resolution runs at enable/load/reload only.
    - Code Quality: Preserve provenance-rich diagnostics from `check_enabled_packages`; avoid silent load-order wins.
    - Security: Package cannot override another package without user-approved package-control or explicit user config.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/conflict.rs`
      - `docs/reference/primitives/package-security.md`
    - Options Considered:
      - Remove conflict checks: rejected.
      - Turn conflict checks into resolver with explicit policy: chosen.
    - Chosen Approach:
      - Keep deterministic conflict index; add resolution inputs and results.
    - API Notes and Examples:
      ```json
      { "conflicts": { "modes.markdown": "replace" } }
      ```
    - Files to Create/Edit:
      - `src/packages/conflict.rs`: Add resolution model.
      - `src/packages/service.rs`: Apply conflict resolution during enable.
      - `tests/package_conflicts.rs`: Resolution tests.
      - `docs/reference/primitives/package-security.md`: Document precedence.
    - References:
      - Package security docs.
  - Test Cases to Write:
    - Duplicate mode rejected without resolution.
    - Replacement wins with package-control grant and provenance diagnostic.
  - Completed Notes:
    - Added `PackageConflictResolutionPolicy`, `PackageConflictResolutionDiagnostic`, and `PackageConflictResolutionReason` to keep `check_enabled_packages` as the deterministic provenance-rich detector while resolving only explicit cases.
    - Added user conflict overrides through `PackageService::set_conflict_override`; unresolved conflicts still fail closed with `PackageConflictDiagnostic` and no load-order winner.
    - Wired package graph `replaces` / `disables` to record winner/loser diagnostics when a user-authorized `package-control` package withdraws a target.
    - Updated key-binding conflict indexing so distinct explicit priority/routing entries are non-conflicting, while identical priority/routing falls back to deterministic diagnostics.
    - Added `tests/package_conflicts.rs` covering unresolved duplicate rejection, package-control replacement with provenance diagnostic, user override resolution, explicit priority behavior, deterministic fallback, and replacement denial without package-control.
    - Updated package security/loading docs, package-loading wiki, primitive review, and docs-as-code guards for explicit conflict resolution policy.
    - Verified with `cargo test --test package_conflicts`, `cargo test --test package_graph`, `cargo test --test package_loading`, `cargo test --test package_loading_docs`, `cargo test --test clay_js_api_inventory`, `cargo test --test markdown_mode`, `cargo test --lib load_package_`, `cargo test --lib op_clay_packages_load_package_by_specifier`, and `cargo fmt --check`.

- [x] Implement package-scoped disable, rollback, and revocation
  - Acceptance Criteria:
    - Functional: Disable/revoke withdraws package-owned commands, behavior manifests, SDUI, parse handlers, decorations, completions, layout/input/state/theme contributions, and diagnostics; rollback keeps prior valid generation on failure.
    - Performance: Disable/rollback/revoke never blocks editor hot paths.
    - Code Quality: Reuse runtime generation and parse coordinator cancellation primitives.
    - Security: Revoked package cannot leave active handlers/contributions behind.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/persistent-runtime-hot-reload.md`
      - `docs/wiki/modules/parse-task-lifecycle.md`
      - `src/server/parse_coordinator.rs`
      - `src/server/js_runtime.rs`
    - Options Considered:
      - Disable only future loads: rejected.
      - Generation-scoped active withdrawal: chosen.
    - Chosen Approach:
      - Add package ownership indexes and generation revocation hooks.
    - API Notes and Examples:
      ```text
      revoke package -> cancel generation work -> rebuild manifest -> publish replacement state
      ```
    - Files to Create/Edit:
      - `src/packages/service.rs`: Revocation state.
      - `src/server/js_runtime.rs`: Runtime generation/package withdrawal wiring.
      - `src/server/parse_coordinator.rs`: Package-scoped cancellation if needed.
      - `tests/package_loading.rs`: Revocation/rollback tests.
      - `docs/wiki/modules/package-loading.md`: Document implementation.
    - References:
      - Phase 19 runtime generation docs.
  - Test Cases to Write:
    - Disable package withdraws owned contributions and cancels stale parse work.
    - Failed replacement keeps previous generation active.
  - Completed Notes:
    - Added package-scoped revocation audit primitives in `src/packages/service.rs`: `PackageRevocationRecord`, `PackageContributionWithdrawalCounts`, `PackageService::revocation_record(s)`, and `revoke_enabled_package`.
    - `PackageService::disable` now records active withdrawal counts for package-owned commands, behavior manifests, SDUI, parse handlers, decorations, completions, layout, input, state, theme, and diagnostics while removing the enabled record.
    - Enable/graph/conflict failure rollback now restores enabled records, conflict diagnostics, revocation records, and package generation so failed replacement attempts keep the prior valid package generation active.
    - Conflict/user-override loser withdrawal records revocation diagnostics instead of silently dropping enabled records.
    - Added package-owned runtime hooks: `ParseCoordinator::cancel_package` withdraws package parse handlers and aborts in-flight parse tasks via the same abort path as generation cancellation; `PackageLoadEntryAllowlist::record_for_package` and `revoke_package` withdraw package-owned loadEntry and transitive module entries.
    - Updated package loading/reference docs, parse lifecycle wiki, persistent runtime hot reload wiki, unified package authority primitive review, and docs-as-code coverage.
    - Verified with `cargo test --test package_loading`, `cargo test --test parse_coordinator`, `cargo test --lib package_load_entry_allowlist_revokes_owned_entries`, `cargo test --test package_loading_docs`, `cargo test --test package_graph`, `cargo test --test package_conflicts`, `cargo test --test clay_js_api_inventory`, `cargo test --test markdown_mode`, `cargo test --lib load_package_`, `cargo test --lib op_clay_packages_load_package_by_specifier`, and `cargo fmt --check`.

- [x] Define and verify the package default init.js loading experience
  - Acceptance Criteria:
    - Functional: `~/.config/clay/init.js` can explicitly load npm/GitHub/local packages after install/authorization with one line when defaults are enough.
    - Performance: init loading remains startup/reload work only.
    - Code Quality: No copied manifests or low-level primitive boilerplate required for ordinary package defaults.
    - Security: init.js cannot silently grant powerful capabilities without documented authorization APIs/config.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `runtime/js/packages.ts`
      - `docs/reference/packages/creating-packages.md`
    - Options Considered:
      - Keep package loading only CLI-driven: rejected.
      - One-line explicit `loadPackage` from init.js: chosen.
    - Chosen Approach:
      - Extend existing `loadPackage` docs and tests for user-installed sources.
    - API Notes and Examples:
      ```javascript
      import { loadPackage } from "clay:packages";
      await loadPackage("@vendor/foo");
      ```
    - Files to Create/Edit:
      - `runtime/js/packages.ts`: Usage docs.
      - `docs/reference/packages/creating-packages.md`: Author/user examples.
      - `tests/package_loading_docs.rs`: Docs guard.
    - References:
      - Explicit init.js loading plan requirement.
  - Test Cases to Write:
    - Docs/test prove one-line load path for user-installed package.
  - Completed Notes:
    - Added `evaluate_init_js_with_seeded_package` runtime test helper mirroring `evaluate_with_seeded_package` but loading a real `~/.config/clay/init.js`-shaped config root, so a user-installed (non-`@clay/*`) package exercises the full init.js → `loadPackage(specifier)` → resolver → enable → authorize → `loadEntry` import → default-export invocation path.
    - Added `load_package_user_installed_default_loads_from_init_js` test proving a `github:vendor/mode` installed+authorized package loads from a genuine one-line `init.js` (no inline manifest, no per-primitive registration, no facade plumbing) and its `loadEntry` default export runs.
    - Updated `runtime/js/packages.ts` and the embedded `CLAY_FACADE_PACKAGES` constant in `src/server/js_runtime.rs` to document the one-line init.js path covering bundled and user-installed packages (`@clay/markdown`, `@vendor/foo`, `github:user/repo`) and the rule that init.js grants no capabilities on its own.
    - Updated `docs/reference/packages/creating-packages.md`, `docs/reference/primitives/package-loading.md`, and `docs/wiki/modules/package-loading.md` with user-installed one-line examples, the `@clay/*`-means-shipped-not-more-capable rule, and the security constraint that init.js cannot silently grant powerful capabilities (filesystem/network/shell/AI/WASM/raw-ops/native-ui/client-runtime/package-control are separate user-approved grants).
    - Added `package_default_init_js_user_installed_one_line_path_is_documented_and_verified` docs-as-code guard verifying the user-installed one-line example coverage, the init.js-grants-no-capabilities statement, and the runtime test existence.
    - Verified with `cargo test --test package_loading_docs` (25), `cargo test --lib load_package_user_installed_default_loads_from_init_js`, `cargo test --test package_loading --test package_loading_docs --test package_graph --test package_conflicts --test parse_coordinator --test clay_js_api_inventory --test markdown_mode` (183), `cargo test --lib load_package_` (14), `cargo test --lib op_clay_packages_load_package_by_specifier` (4), and `cargo fmt --check`.

- [x] Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: Package guide documents that user-installed packages may request same UI/layout/native/client capabilities as Clay packages, with required grants and validation.
    - Performance: UI/layout declarations remain validated load/reload/config work, not paint/layout hot-path JS.
    - Code Quality: New UI/layout primitives remain generic and reusable.
    - Security: Native UI/client runtime is explicit capability/API work, not implicit through package source.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
      - `docs/reference/packages/creating-packages.md`
    - Options Considered:
      - Leave guide first-party-biased: rejected.
      - Document unified authoring contract: chosen.
    - Chosen Approach:
      - Update package author docs after implementation surfaces settle.
    - API Notes and Examples:
      ```text
      native-ui/client-runtime require explicit capability grants and documented APIs
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`: Unified authoring model.
      - `docs/reference/primitives/package-security.md`: UI/native capability notes.
    - References:
      - Package UI/layout pattern.
  - Test Cases to Write:
    - Docs guard for package UI/layout authoring contract.
  - Completed Notes:
    - Added "Unified UI/layout authoring contract across package sources" subsection to `docs/reference/packages/creating-packages.md` stating the contract is identical for `@clay/*` and user-installed packages (npm/GitHub/git/tarball/local), `@clay/*` means shipped by Clay not more capable, user-installed packages may request the same UI/layout/native/client capabilities through the unified vocabulary subject to explicit user authorization grants, native-ui/client-runtime are explicit capability/API work (no implicit source-based authority), UI/layout declarations stay validated load/reload/config work with no package JS in Masonry paint/layout hot paths, and no UI/layout primitive branches on package source.
    - Added "Native UI and Client Runtime Are Explicit Capability/API Work" section to `docs/reference/primitives/package-security.md` stating native UI and client-side runtime are never implicit through package source, `native-ui`/`client-runtime` are granted only through explicit user/admin authorization records tied to identity/source/provenance never inferred from source kind, a capability grant authorizes use of a surface but does not materialize it or bypass Masonry/client validation, and no UI/layout/security primitive branches on package source.
    - Fixed stale docs-as-code guard in `tests/primitives_docs.rs` (`creating_packages_docs_mark_examples_by_status`) that referenced the old `loadPackage("@clay/*")` one-line phrase; updated to the current "**Implemented end-user default:**" unified wording. `primitives_docs` went from 82 passed + 1 failing to 83 passed.
    - Added `package_ui_layout_authoring_contract_is_unified_across_package_sources` docs guard to `tests/package_loading_docs.rs` verifying the unified authoring contract content across creating-packages.md and package-security.md (functional, security, performance, and code-quality phrases).
    - Verified with `cargo test --test package_loading_docs` (26), `--test primitives_docs` (83), `--test package_loading --test package_graph --test package_conflicts --test parse_coordinator --test clay_js_api_inventory --test markdown_mode`, `cargo test --lib load_package_` (14), `cargo test --lib op_clay_packages_load_package_by_specifier` (4), and `cargo fmt --check`.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: User authorization, capability grants, runtime profile choices, package graph overrides, and conflict resolutions are exposed only through documented Clay JS/config APIs or explicitly documented CLI/UI state, not hidden keys.
    - Performance: Configuration evaluation is startup/reload/user-command work only.
    - Code Quality: Every behavior-changing config surface has docs, inventory entry, examples, and custom properties.
    - Security: Config can grant powerful capabilities only through explicit authorization flow with provenance and revocation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/clay-js-api/configuration.md`
    - Options Considered:
      - Ad hoc TOML/JSON grants: rejected unless documented as API surface.
      - Clay JS/CLI/UI authorization APIs: chosen.
    - Chosen Approach:
      - Inventory config surfaces late in implementation and document each one.
    - API Notes and Examples:
      ```javascript
      clay.packages.authorize({ package: "@vendor/foo", capabilities: ["network"] })
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: Config/authorization docs.
      - `docs/reference/clay-js-api/api-inventory.toml`: Registry entries.
      - `tests/clay_js_api_inventory.rs`: Documentation gates.
    - References:
      - Clay configuration task requirement.
  - Test Cases to Write:
    - API inventory/docs fail if authorization/config surfaces are undocumented.
  - Completed Notes:
    - Added planned inventory entries `clay.packages.authorize` and `clay.packages.setConflictOverride` to `docs/reference/clay-js-api/api-inventory.toml` with status = "planned", registry_public = false, custom properties, and security notes explaining explicit user/admin grants, provenance, revocation, fail-closed behavior, and the rule that a grant authorizes use of a surface but does not materialize it.
    - Added "Plan 035 unified package authority configuration review" section to `docs/reference/clay-js-api/configuration.md` documenting the configuration surfaces for authorization/capability grants, runtime profile selection, user conflict overrides, package graph relations, package-control authority, bundled package auto-authorization, and authorization inspection; includes the intended `clay.packages.authorize({ package, capabilities, runtimeProfile, approvedBy })` example and the explicit documented implementation gap that no callable end-user surface exists yet for user-installed package authorization/conflict override.
    - Updated stale "Plan 034 persistent-runtime hardening is intentionally not configurable" section: corrected the outdated third-party execution gate language to reflect the unified model (non-`@clay/*` packages load after install and user authorization through `clay.packages.authorize`, no `enableThirdPartyPackages`/`allowThirdPartyPackages` shortcut).
    - Updated stale Phase 18.5 Markdown audit table row: `clay.packages.loadPackage` is now documented as implemented by Plan 029/Phase 18.6 and generalized by Plan 035 to source-aware loading of bundled and installed user-authorized packages.
    - Added `plan_035_unified_package_authority_configuration_surfaces_are_documented` test to `tests/clay_js_api_inventory.rs` verifying the planned inventory entries, custom properties, security notes, and configuration review content.
    - Fixed two pre-existing docs-as-code guard phrase mismatches caused by the prior unified-authority edits: `plan_034_runtime_hardening_does_not_add_hidden_configuration_knobs` and `phase20_markdown_configuration_audit_documents_end_user_contract` now match the corrected Plan 034/Plan 035 language.
    - Verified with `cargo test --test clay_js_api_inventory` (50), `cargo test --test package_loading_docs` (26), `cargo test --test primitives_docs` (83), combined package/js-runtime/docs suites (268 across 8 suites), `cargo test --lib load_package_` (14), `cargo test --lib op_clay_packages_load_package_by_specifier` (4), and `cargo fmt --check`.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Public package install/inspect/authorize/enable/disable/revoke/conflict APIs are documented Clay JS APIs where exposed to JS.
    - Performance: API docs state package management work is install/enable/load/reload/background/user-command work, not hot-path work.
    - Code Quality: Raw `Deno.core.ops` names, package-manager internals, and sandbox protocol frames are not user-facing APIs.
    - Security: API docs explain capability grants, provenance, runtime profiles, revocation, and diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
    - Options Considered:
      - Expose Rust functions directly: rejected.
      - Stable Clay JS facade APIs: chosen.
    - Chosen Approach:
      - Review all new public Rust functions and add docs/facades only for intended JS API surfaces.
    - API Notes and Examples:
      ```javascript
      await clay.packages.enable("@vendor/foo");
      await clay.packages.disable("@clay/markdown");
      ```
    - Files to Create/Edit:
      - `runtime/js/packages.ts`: Facades.
      - `docs/reference/clay-js-api/**`: API docs.
      - `docs/reference/clay-js-api/api-inventory.toml`: Registry entries.
      - `tests/rust_visibility_api_mapping.rs`: Visibility/API mapping.
    - References:
      - Clay JS API task requirement.
  - Test Cases to Write:
    - Rust public item/API inventory tests pass.
    - Package JS API docs registry is current.
  - Completed Notes:
    - Allowlisted `src/server/parse_coordinator.rs::ParseCoordinator::cancel_package` in `tests/rust_visibility_api_mapping.rs` as internal server infrastructure (package-scoped revocation primitive, not a user-facing API).
    - Added planned Clay JS API inventory entries for package management surfaces: `clay.packages.install`, `clay.packages.enable`, `clay.packages.disable`, `clay.packages.inspect`, `clay.packages.list`, plus the task-10 entries `clay.packages.authorize` and `clay.packages.setConflictOverride`.
    - Each planned entry has status = "planned", registry_public = false, custom properties, security notes, hot-path policy, and backing Rust pointing to planned `src/server/ops/packages.rs::op_clay_packages_*` wrappers (not directly to `PackageService` internal methods, per the existing Phase 18.5 boundary test).
    - Added stub facade exports in `runtime/js/packages.ts` for `install`, `enable`, `disable`, `inspect`, `list`, `authorize`, and `setConflictOverride` using the `plannedPackageApi` helper (throws with `op_clay_runtime_unavailable` when available).
    - Added `plan_035_unified_package_authority_public_surfaces_are_mapped_or_internal` test to `tests/rust_visibility_api_mapping.rs` verifying the new op paths, facade paths, and conflict-resolution types are mapped in `api-inventory.toml`, and that internal revocation helpers (`ParseCoordinator::cancel_package`, `PackageLoadEntryAllowlist::revoke_package`) are not mapped as user-facing APIs.
    - Verified with `cargo test --test rust_visibility_api_mapping` (10), `cargo test --test clay_js_api_inventory` (50), combined relevant suites (278 across 9 suites), `cargo test --lib load_package_` (14), `cargo test --lib op_clay_packages_load_package_by_specifier` (4), and `cargo fmt --check`.
    - Package JS API docs registry is current.

- [x] Verify policy and implementation gates
  - Acceptance Criteria:
    - Functional: Tests prove unified authority docs, source-aware loading, authorization, package graph, conflict resolution, disable/revoke, and docs/API registry behavior.
    - Performance: Focused tests and docs prove no package management work moved into editor hot paths.
    - Code Quality: No stale strict Plan 035 tests require non-`@clay/*` deny-by-default behavior as policy.
    - Security: Powerful capabilities require explicit grants and revocation tests.
  - Approach:
    - Documentation Reviewed:
      - All touched docs and tests.
    - Options Considered:
      - Rely on full `cargo test`: useful but not enough for docs/API gates.
      - Add focused gates plus full relevant suites: chosen.
    - Chosen Approach:
      - Run focused tests after each implementation area and final aggregate checks.
    - API Notes and Examples:
      ```bash
      cargo test --test package_loading_docs
      cargo test --test package_loading
      cargo test --test package_graph
      cargo test --test package_conflicts
      cargo test --test parse_coordinator
      cargo test --test clay_js_api_inventory
      cargo test --test rust_visibility_api_mapping
      cargo test --test primitives_docs
      cargo test --test markdown_mode
      cargo test --lib load_package_
      cargo test --lib op_clay_packages_load_package_by_specifier
      cargo fmt --check
      ```
    - Files to Create/Edit:
      - `tests/package_loading_docs.rs`: Unified docs gates.
      - Relevant package/runtime/API tests.
    - References:
      - Documentation-as-code pattern.
  - Test Cases to Write:
    - Final focused test list covering each implemented surface.
  - Completed Notes:
    - Unified authority docs: `unified_package_authority_model_is_documented` passes in `tests/package_loading_docs.rs`.
    - Source-aware loading: `tests/package_loading.rs` covers npm/GitHub/local requested-specifier loading and package-root confinement (`load_package_rejects_escaping_relative_import_from_package_root`).
    - Authorization: `missing_authorization_grant_fails_enable_for_requested_capability` in `tests/package_loading.rs` and `PackageService::authorize_package` coverage verify explicit grants are required.
    - Package graph: `tests/package_graph.rs` (5 tests) covers dependsOn/extends/disables/replaces, cycle detection, missing targets, and package-control grant enforcement.
    - Conflict resolution: `tests/package_conflicts.rs` (6 tests) covers user overrides, package-control replacement/disable, keybinding priority fallback, and deterministic diagnostics.
    - Disable/revoke: `tests/package_loading.rs::package_service_disable_removes_active_contributions`, `tests/parse_coordinator.rs::package_cancel_withdraws_handlers_and_in_flight_parse_work`, and `src/server/js_runtime.rs::package_load_entry_allowlist_revokes_owned_entries` verify contribution withdrawal and revocation records.
    - Docs/API registry: `tests/clay_js_api_inventory.rs` (50 tests) and `tests/rust_visibility_api_mapping.rs` (10 tests) verify inventory entries, facade mappings, and JS API boundaries.
    - Hot-path exclusion: docs-as-code gates and inventory `hot_path_policy` entries state that install/enable/load/reload/authorization/conflict/graph/revoke work never runs from keypress/paint/layout/scroll/text-event/edit-ack/pointer/Masonry paths.
    - Stale strict policy audit: grep across `tests/`, `docs/`, and `plans/` found no remaining non-`@clay/*` deny-by-default policy language; remaining `@clay/*` references describe the temporary resolver limit or the unified model (`@clay/*` means shipped by Clay, not more capable).
    - Powerful capabilities: `src/packages/permissions.rs::is_prohibited_authority` returns `false` for all inputs (no categorical prohibition); 19 capabilities are grantable through explicit user authorization records; `MissingCapabilityGrant` fail-closed errors are tested.
    - Final focused verification passed: 278 tests across 9 suites, plus 14 `load_package_` lib tests, 4 `op_clay_packages_load_package_by_specifier` lib tests, and `cargo fmt --check`.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Completed Notes:
    - Read `.agents/skills/project-wiki/SKILL.md` and followed the post-implementation wiki update workflow.
    - Updated `docs/wiki/modules/third-party-runtime-authority.md` to reflect the implemented unified authority model: current primitive inventory now lists provenance, authorization, source-aware load, runtime module loading, and package-scoped revocation as implemented; authority model, conflict resolution, runtime profiles, and disable/revoke sections rewritten in present tense; added a "Remaining Implementation Work" section listing deferred durable persistence, end-user API wiring, production sandbox routing, package-import boundaries, and richer conflict precedence.
    - Updated `docs/wiki/modules/unified-package-authority-primitive-review.md` to mark all 8 generic primitive gaps as implemented by Plan 035, rewrote the "Implementation Guidance" section as "Implementation Outcome", and added a "Remaining Gaps" section aligned with the deferred work. Expanded the test command list to include package_graph, package_conflicts, parse_coordinator, clay_js_api_inventory, and rust_visibility_api_mapping.
    - Verified `docs/wiki/index.md` already links `Unified Package Authority Primitive Review`, `Third-Party Runtime Authority`, `Package Loading`, `Persistent Runtime Hot Reload`, `Persistent Runtime Hardening`, `Parse Task Lifecycle`, `Embedded JavaScript Runtime`, and `Package Primitive Gate`; no new index entries were needed.
    - Verified `docs/wiki/modules/package-loading.md` already covers source-aware loading, authorization, package graph, conflict resolution, revocation, and hot-path policy in the final implementation state; no substantive rewrite was required.
    - Wiki updates document security boundaries (explicit grants, provenance, revocation, hot-path exclusion, no source-specific branches) without exposing secrets.
    - Docs-as-code gates passed after wiki updates: `package_loading_docs` (26), `primitives_docs` (83), `clay_js_api_inventory` (50), `rust_visibility_api_mapping` (10), combined relevant suites (278 across 9 suites), `load_package_` lib tests (14), `op_clay_packages_load_package_by_specifier` lib tests (4), and `cargo fmt --check`.

## Compromises Made

- Durable persisted enable/authorization hydration across server restarts remains deferred; the runtime resolver can load bundled packages and installed user-authorized packages already present in `PackageService` for the current runtime generation.

## Further Actions

- Execute tasks in order, starting with strict-policy cleanup and primitive review.
