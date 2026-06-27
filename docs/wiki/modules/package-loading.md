# Package Loading

## Source

- `src/packages/record.rs`
- `src/packages/conflict.rs`
- `src/packages/service.rs`
- `src/packages/manager.rs`
- `src/packages/modes.rs`
- `src/main.rs`
- `src/server/ops/packages.rs`
- `src/server/js_runtime.rs`
- `runtime/js/packages.ts`
- `runtime/js/modes.ts`
- `runtime/js/decorations.ts`
- `runtime/js/parse.ts`
- `docs/reference/primitives/package-loading.md`
- `tests/package_loading.rs`
- `tests/package_loading_docs.rs`

## Overview

Phase 17 package loading turns a `package.json` Clay metadata block into a typed `PackageRecord`, enables it through `PackageService`, and rejects cross-package conflicts before contributions become active. Installation remains separate from enable/load: install records package metadata through a delegated package-manager boundary, while enable performs Clay-owned validation and conflict checks without executing package runtime JavaScript. The `clay package add|remove|list|enable|disable|inspect` CLI routes through the same service boundary intended for future in-app package UI. Runtime facade wiring now lets the controlled server-side JavaScript runtime import `clay:packages`, `clay:modes`, `clay:commands`, `clay:decorations`, and `clay:parse`; `serverLoadPackage` routes through the same package record assembler and is documented in the generated Clay JS API registry, while per-document manifest selection and decoration/parse public calls remain explicit planned handoff APIs. Phase 18.6 shipped the generic one-line loader: `loadPackage` is a runtime-backed `clay:packages` facade export that resolves a first-party `@clay/*` specifier, validates package metadata through `PackageService`, enables the package, and imports and executes its declared `loadEntry`. The module-loader extension (`ClayModuleLoader`) is deny-by-default for all specifiers and only accepts a resolver-validated first-party `loadEntry` recorded in a shared `FirstPartyLoadEntryAllowlist`. The `loadEntry` is confined to the validated package root for its own imports. The resolver reuses `PackageService::enable` (`assemble_package_record` + `check_enabled_packages`) so invalid metadata and conflicting packages are rejected before activation. Optional customization after the one-line load uses documented `setPackageOption` and `serverSetLayoutOverride` APIs; hidden JSON/TOML/ad hoc keys remain rejected.

## Responsibilities

- Assemble validated package records with package name, version, API prefix, docs path, performance metadata, API dependencies, and inert contribution descriptors.
- Retain contribution provenance for commands, configuration keys, key-routing entries, text transforms, SDUI/status regions, decoration/render primitive descriptors, slot-aware package UI panels/components/overlays, and package theme tokens.
- Validate package SDUI/status descriptors as inert metadata, reject executable widget/native fields, and enforce `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` and `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` estimates at enable/load time.
- Validate Phase 18.4 package UI metadata (`clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, input declarations, UI state scopes, layout-override API dependencies, package-option API dependencies, and `themeTokens`) at package load/enable time: package-prefixed IDs/tokens, fixed slot claims, component catalog kinds, input/focus/action metadata, UI state schema/lifecycle metadata, typed style variables, declared command action targets, same-type Clay core token fallbacks, prohibited authority fields, and bounded payload estimates.
- Run one deterministic conflict pass across the enabled package set for prefixes, modes, commands, key bindings, configuration keys, SDUI regions, decoration primitives, and behavior-manifest entries.
- Delegate package-manager process work through `PackageManagerBackend`/`PnpmBackend` and keep captured stdout/stderr/exit-code handling at a typed boundary.
- Route `clay package ...` CLI operations through `PackageService` instead of duplicating install/enable/list/inspect logic.
- Roll back `PackageService::enable` when the candidate package would introduce a conflict.
- Select per-document behavior manifests from one active major mode plus compatible non-overriding minor modes, preserving package/mode/behavior-version provenance.
- Provide load/configuration-time runtime facades that call typed Rust validators without exposing raw op names as user-facing API exports.
- Document package loading scope, conflict handling, hot-path exclusion, and Phase 18 decoration/parse handoff in `docs/reference/primitives/package-loading.md`.

## How It Works

`assemble_package_record` first reuses the Phase 16.5 manifest validator, then parses contribution descriptors from `clay.contributions`. Descriptors are inert data only. Package-owned IDs must use the package `apiPrefix` namespace and may not claim `clay.*`. Command and configuration descriptors require their matching package permissions; SDUI/status and Phase 18.3 package UI descriptors do not grant authority, and embedded actions still target separately declared commands.

Phase 18.4 extends the record with `ui_panels`, `ui_components`, `ui_overlays`, `theme_tokens`, `input_contributions`, `ui_state_scopes`, `layout_overrides`, and `package_options`; the manifest keys are `input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions`. The validator accepts detailed metadata only when IDs/tokens are package-prefixed, `clay.ui.*` / `clay.configuration.*` API dependencies are known, required `package-configuration` permissions are declared for layout overrides and package option schemas, registered actions/action targets are declared command contributions, component trees use the Clay component catalog and typed style variables, input/focus/action declarations stay inert, UI state scopes declare schema/lifecycle metadata rather than state values, layout overrides reference registered input/action/theme-token metadata, package options use the supported option names (`layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`), package theme tokens resolve through same-type core token fallbacks, duplicate fixed slot claims are rejected within and across packages, and each declaration stays within its bounded payload / SDUI snapshot-update budget. Cross-package enable checks reject duplicate UI IDs, duplicate input IDs, duplicate input contribution IDs, duplicate UI state scope IDs, duplicate layout override target/property pairs, duplicate package option schemas, duplicate theme tokens, and duplicate fixed-slot claims with package provenance instead of load-order wins. In short, Phase 18.4 rejects duplicate input, duplicate UI state scope, duplicate layout override, and duplicate package option metadata before activation, while preserving hidden-key rejection and state-value rejection as load-time diagnostics.

`check_enabled_packages` receives enabled `PackageRecord` values, sorts them by prefix/name/version, and builds `BTreeMap` indices. Each insertion either records provenance or returns a `PackageConflictDiagnostic` containing the conflict kind, contribution ID, first package provenance, second package provenance, and an actionable message. The pass is deterministic because record order and index ordering are stable.

`PnpmBackend` implements `PackageManagerBackend` with `pnpm add`, `pnpm remove`, and `pnpm list --json --long`. It captures stdout/stderr and exit codes in typed results/errors and reads discovered `package.json` files back into Clay as inert JSON. `pnpm add` passes `--ignore-scripts` by default; lifecycle scripts are suppressed unless the caller opts in through `PackageInstallOptions::allow_lifecycle_scripts`. This prevents remote package code from executing before Clay validates package metadata. `PackageService::install` creates the store directory, delegates to the backend with the install options, and records the returned `package.json` without enabling or executing the package. `PackageService::enable` validates the raw installed manifest into a `PackageRecord`, temporarily inserts it into the enabled set, runs `check_enabled_packages`, and removes the candidate again if any conflict is reported. This keeps failed enables from partially activating prefixes, modes, commands, configuration keys, SDUI regions, or behavior entries.

Third-party registry and integrity policy stays at this package-manager boundary. Clay delegates registry access, resolution, lockfile writing, integrity verification, caching, and offline store behavior to pnpm/npm-compatible tooling instead of implementing a registry client. A future trusted third-party package must have a Clay-owned provenance record with requested spec, resolved version, registry/source URL, lockfile path, integrity digest, tarball or source path, package root, and offline/cache key. Enable/load must compare that record to the explicit trust identity record before runtime execution. Package-manager stdout, stderr, exit code, `package.json`, lockfile text, and registry metadata are diagnostic/provenance inputs only; they are not runtime authority. Diagnostics copied from package-manager output must be sanitized to remove environment secrets, auth tokens, filesystem roots outside the Clay package store, raw registry credentials, shell-expanded command text, and unbounded stdout/stderr. Offline/cache hits and updates do not widen authority: cached packages must still match the trusted resolved version and integrity digest, and changed version/registry/tarball/source/integrity values require a new trust record. Current gaps are generic provenance storage, lockfile integrity parsing, diagnostic sanitization, offline/cache key modeling, and update-as-new-identity enforcement.

Phase 19 hot reload treats package state as runtime-generation state. `loadPackage` keeps `globalThis.__clayLoadedPackages` idempotent inside one `ClayJsRuntimeService`; `IpcServer::reload_runtime_generation` invalidates that cache by constructing a fresh service, rerunning the configured/default `init.js`, rebuilding the first-party `loadEntry` allowlist (`FirstPartyLoadEntryAllowlist`), and rerunning package `loadEntry` modules. Package authors must rebuild mode registrations, commands, UI contributions, and parse registrations from `loadEntry` each generation; mutable JS globals are generation-local and are not a persistence mechanism. Parse handlers are generation-scoped: new registrations replace old handler tokens, old-generation active parse work is cancelled, and late old-runtime-generation parse results are rejected before publication. Failed reloads keep the prior generation's loaded-package cache and active service intact and emit sanitized diagnostics.

`src/main.rs` parses the `clay package` subcommands and constructs a service using the default Clay package store under the user's config directory. Each CLI invocation is a fresh process, so for `list`/`enable`/`disable`/`inspect`/`remove` the CLI calls `PackageService::refresh_installed()` immediately after construction to repopulate the in-memory `installed` map from the package-manager store (e.g. `pnpm list --json --long`); `add` skips this because `install` re-discovers the package itself and a missing pnpm binary should fail at `pnpm add`, not at the pre-list step. `refresh_installed` reads only `package.json` metadata and does not execute package code; enabled state remains in memory per process (packages must be re-enabled each session, matching the runtime loader's explicit-load contract). The CLI surface is intentionally thin: package state transitions and diagnostics stay in `PackageService`, while package-manager execution stays in `PackageManagerBackend`.

Per-document mode loading uses `ModeRegistry` to store one active major mode per document, optional compatible minor modes, and the selected inert `BehaviorManifest` keyed by document ID. Minor modes must declare the active major mode as compatible and may append behavior entries only when they do not replace major-mode entries. Each selection records major/minor package provenance and a behavior version for client manifest installation and later parse/decorations handoff.

`op_clay_packages_load_package` accepts a `package.json`-shaped value from the controlled runtime facade, calls `assemble_package_record`, and returns typed summary metadata. This gives package load fixtures a runtime path through the validator without installing packages or spawning package managers. `serverLoadPackage` remains a lower-level validation helper; it is not the end-user default. The end-user default is `loadPackage("@clay/*")`.

Phase 18.6 (`plans/029`) implemented the generic one-line loader with the `op_clay_packages_load_package_by_specifier` resolver, the `FirstPartyLoadEntryAllowlist` module-loader gate, and the `loadPackage` facade export. Phase 18.7 verifies the default init path end-to-end: one line from `~/.config/clay/init.js` (`await loadPackage("@clay/markdown");`) loads the package once on the persistent runtime, registers mode activation metadata and the parse handler, and selected-file open later reuses that state through generic classification + `ParseCoordinator` scheduling. User config does not copy package manifests, call low-level facades, perform manual primitive registration, publish representative decoration publication payloads, or create per-open runtime roots. `loadPackage` is idempotent per runtime generation; repeated calls return the cached validated summary until Phase 19 reload replaces the runtime generation and reruns `init.js` in a fresh service. The resolver is constrained to first-party `@clay/*` packages only. Package `entry` and `loadEntry` metadata must be explicit relative `./... .js` module paths with no `..`, empty path segments, backslashes, URLs, absolute paths, or raw-op strings. The resolver canonicalizes both the package root and `loadEntry` target, treats canonicalization failure as a load failure, and records the allowlist entry only when the canonical `loadEntry` stays inside the canonical package root. Non-`@clay/*` registry resolution (third-party, npm, custom registries) is deferred to Phase 23 ecosystem hardening. Runtime-generation hot reload now invalidates the in-memory `loadPackage` cache by replacing the service; persistent shared enable state across server restarts remains deferred. The Markdown package's `loadEntry` (`markdownLoadMode` in `packages/markdown/dist/load.js`) is the default activation export that `loadPackage` invokes; it imports the `clay:packages`, `clay:modes`, `clay:commands`, and `clay:parse` facades directly, contains no inline manifest, and delegates to `loadMarkdownPackage`. The package-owned fallback `markdownLoadMode()` remains a documented convenience alias for per-load options. See `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` for the authority rationale and security review.

Phase 18.6 implemented the one-line loader. The current `clay:packages` facade exports `serverValidatePackageManifest`, `serverValidatePackagePermissions`, `serverLoadPackage`, `serverListFirstPartyPackageSpecifiers`, and `loadPackage`. The `op_clay_packages_load_package_by_specifier` resolver op (`src/server/ops/packages.rs`) resolves `@clay/*` specifiers, validates metadata, enables the package, and imports the `loadEntry`; `op_clay_packages_list_first_party_specifiers` lists installed first-party specifiers for Phase 18.7 generic open-time activation when no mode has been registered yet. The module-loader extension (`ClayModuleLoader`) accepts only resolver-validated `loadEntry` specifiers from a shared allowlist; all other specifiers are denied. The missing non-`@clay/*` registry bridge and persistent enable state are carried-forward deferrals, not current gaps.

Customization after a future one-line load is now defined through documented Clay JS APIs and runtime-backed APIs rather than hidden JSON/TOML/ad hoc keys. `setPackageOption({ packagePrefix, option, value, source })` records typed package options such as `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`. `serverSetLayoutOverride({ targetId, property, value, source })` records validated layout/theme/input/action overrides with deterministic source precedence. Both run at startup/package-load/configuration-change/explicit setting-change time; Masonry paint/layout/pointer/key/text paths read already-installed inert state.

`clay:decorations` and `clay:parse` are importable runtime modules, but their public Phase 18 functions currently delegate to the planned-unavailable op. The typed Rust validator/coordinator code exists for the handoff; the API inventory maps those planned facades separately from the implemented package-loading API so only registry-ready surfaces are public.

## Invariants and Constraints

- Package-manager process calls, conflict checks, mode activation/manifest selection, Phase 18.4 UI/input/state/layout/configuration metadata checks, `serverLoadPackage` validation, `setPackageOption`, `serverSetLayoutOverride`, and hot reload refresh happen only at explicit install/load/enable/activation/reload/configuration-change time, never in keypress, paint, layout, scroll, edit acknowledgement, or text-event handlers.
- Conflicts never silently override behavior.
- Package SDUI/status and package UI contributions are inert descriptors; no package widget code, client JavaScript, raw CSS, renderer callbacks, direct Masonry widgets, draw callbacks, or native handles reach the client.
- SDUI/package UI action authority is inherited from target commands and their permissions; declaring a region, panel, component, or overlay alone grants no command authority.
- Installed packages do not gain filesystem, network, shell, AI, workspace, package-enable, package-manager execution, raw-op, native-widget, client-JS, or runtime-execution authority merely by being present in the package store. The runtime resolver still rejects non-`@clay/*` examples such as `left-pad`, `@scope/pkg`, URL, local path, and traversal specifiers before module loading until an approved third-party authority decision exists.
- Hot reload preserves resolver-validated first-party `@clay/*` loading only, deny-by-default module loading, package permission checks, executable callback payload rejection, and sanitized reload diagnostics.
- Duplicate mode IDs across different package prefixes are normally rejected earlier by package-owned ID rules, but the conflict pass still preserves deterministic mode collision diagnostics for enabled records.
- Error/diagnostic types returned in `Result` positions use `Box<str>` (not `String`) for their free-text and identity fields, and `PackageServiceError` boxes its two large payload variants (`InvalidClayMetadata(Box<PackageRecordError>)`, `ContributionConflict(Box<PackageConflictDiagnostic>)`), so the diagnostic structs (`PackageRecordError`, `ModeDiagnostic`, `CommandDiagnostic`, `PackageConflictDiagnostic`, `UiContributionDiagnostic`) and the service error enum stay under clippy's `result_large_err` 128-byte threshold. `Box<str>` is a 16-byte fat pointer vs `String`'s 24-byte (ptr+len+cap); these diagnostics are constructed once and read/displayed, never grown in place. Compile-time `size_of` asserts in the `packages::record` and `server::ui` test modules guard against a regression.

## Tests

Run focused coverage with:

```text
cargo test --test package_loading
```

Relevant tests:

- `enable_rejects_duplicate_prefix_and_mode_and_command`
- `ambiguous_keybinding_across_packages_rejected_without_priority`
- `package_sdui_contribution_carries_provenance_and_respects_budget`
- `package_manifest_accepts_phase18_4_input_state_config_metadata`
- `package_manifest_rejects_invalid_phase18_4_metadata`
- `enabled_package_conflicts_reject_duplicate_input_state_config_targets`
- `phase18_4_diagnostics_preserve_package_provenance`
- `package_cli_subcommands_route_through_shared_service`, `package_service_list_persists_across_service_instances`, and `package_service_enable_after_refresh_does_not_require_reinstall` cover the shared service API surface and CLI state persistence across fresh service instances.
- `third_party_install_metadata_does_not_imply_runtime_execution_authority` verifies package-manager/store metadata can be recorded without enabling or executing third-party package JavaScript.
- Existing package record, manager/service/CLI, conflict, SDUI provenance, and per-document manifest selection tests in `tests/package_loading.rs`
- `runtime_imports_modes_commands_and_packages_facades` and `phase18_primitive_facades_remain_explicitly_planned` in `src/server/js_runtime.rs`
- `package_loading_doc_linked_from_indexes_and_marks_phase17_ready`, `package_loading_keeps_validation_and_parsing_out_of_typing_hot_path`, and `phase18_only_apis_remain_planned_or_documented_without_raw_op_exposure` in `tests/package_loading_docs.rs`

## Related

- [Package Primitive Gate](package-primitive-gate.md)
- [Mode Registry](mode-registry.md)
- [Command Registry](command-registry.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- `docs/reference/primitives/package-security.md`
- `.agents/skills/project-patterns/references/package-distribution.md`
