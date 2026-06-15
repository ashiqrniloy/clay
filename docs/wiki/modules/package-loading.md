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

Phase 17 package loading turns a `package.json` Clay metadata block into a typed `PackageRecord`, enables it through `PackageService`, and rejects cross-package conflicts before contributions become active. Installation remains separate from enable/load: install records package metadata through a delegated package-manager boundary, while enable performs Clay-owned validation and conflict checks without executing package runtime JavaScript. The `clay package add|remove|list|enable|disable|inspect` CLI routes through the same service boundary intended for future in-app package UI. Runtime facade wiring now lets the controlled server-side JavaScript runtime import `clay:packages`, `clay:modes`, `clay:commands`, `clay:decorations`, and `clay:parse`; `serverLoadPackage` routes through the same package record assembler and is documented in the generated Clay JS API registry, while per-document manifest selection and decoration/parse public calls remain explicit planned handoff APIs. Phase 18.4 verifies that `loadPackage("@clay/markdown")` is still a preferred `init.js` target rather than an implemented `clay:packages` export, and optional customization uses documented configuration/layout APIs instead of hidden keys.

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

`PnpmBackend` implements `PackageManagerBackend` with `pnpm add`, `pnpm remove`, and `pnpm list --json --long`. It captures stdout/stderr and exit codes in typed results/errors and reads discovered `package.json` files back into Clay as inert JSON. `PackageService::install` records that JSON without enabling or executing the package; `PackageService::enable` validates the raw installed manifest into a `PackageRecord`, temporarily inserts it into the enabled set, runs `check_enabled_packages`, and removes the candidate again if any conflict is reported. This keeps failed enables from partially activating prefixes, modes, commands, configuration keys, SDUI regions, or behavior entries.

`src/main.rs` parses the `clay package` subcommands and constructs a service using the default Clay package store under the user's config directory. The CLI surface is intentionally thin: package state transitions and diagnostics stay in `PackageService`, while package-manager execution stays in `PackageManagerBackend`.

Per-document mode loading uses `ModeRegistry` to store one active major mode per document, optional compatible minor modes, and the selected inert `BehaviorManifest` keyed by document ID. Minor modes must declare the active major mode as compatible and may append behavior entries only when they do not replace major-mode entries. Each selection records major/minor package provenance and a behavior version for client manifest installation and later parse/decorations handoff.

`op_clay_packages_load_package` accepts a `package.json`-shaped value from the controlled runtime facade, calls `assemble_package_record`, and returns typed summary metadata (identity, entries, docs, performance estimate, API dependencies, and contribution counts, including Phase 18.4 UI panel/component/overlay/theme-token/input/state-scope/layout-override/package-option counts where available). This gives package load fixtures a runtime path through the validator without installing packages, spawning package managers, or executing package entry points. It is not the end-user one-line package loader: the preferred `~/.config/clay/init.js` target remains `loadPackage("@clay/markdown")` once generic install/enable/load-entry authority exists; until then, fixture scripts that call `serverLoadPackage(packageJson)` plus package-owned load helpers are explicitly temporary and represent a temporary validation/loading gap.

Phase 18.5 (`plans/028` Task 4) investigated the generic one-line loader and deferred it with a decision-log-backed rationale (`decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`). The controlled server-side runtime is deny-by-default (`src/server/js_runtime.rs::ClayModuleLoader`) and confines loadable modules to the configuration root (`src/server/configuration.rs::canonical_local_file`), so a working `loadPackage("@clay/*")` requires a security-critical module-loader extension that lets the runtime import a resolver-validated first-party `loadEntry` from outside the config root, plus a `PackageService` resolve/enable/execute path and a new op. That authority expansion warrants its own focused phase. The Markdown package now ships the fallback entry shape the future resolver will invoke: `markdownLoadMode()` in `packages/markdown/dist/load.js` (re-exported from `./dist/index.js`) imports the `clay:packages`, `clay:modes`, `clay:commands`, and `clay:parse` facades directly, contains no inline manifest object, and reuses the existing `loadMarkdownPackage` logic. The documented temporary end-user fallback is therefore `import { markdownLoadMode } from "@clay/markdown"; await markdownLoadMode();`, which consumes only implemented generic primitives and requires no copied fixture manifest once the module-loader bridge ships.

Phase 18.4's loader audit keeps that gap explicit: the generic one-line loader is not implemented yet, and the generic loader/API gap is the missing package-service bridge. The current `clay:packages` facade exports `serverValidatePackageManifest`, `serverValidatePackagePermissions`, and `serverLoadPackage`; it does not export `loadPackage`, and `src/server/ops/packages.rs` has no op that can resolve an installed package specifier, enable the package, import the package `loadEntry`, and record activation from `~/.config/clay/init.js`. That missing bridge is why normal package defaults cannot yet be described as a shipped one-line setup.

Customization after a future one-line load is now defined through documented Clay JS APIs and runtime-backed APIs rather than hidden JSON/TOML/ad hoc keys. `setPackageOption({ packagePrefix, option, value, source })` records typed package options such as `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`. `serverSetLayoutOverride({ targetId, property, value, source })` records validated layout/theme/input/action overrides with deterministic source precedence. Both run at startup/package-load/configuration-change/explicit setting-change time; Masonry paint/layout/pointer/key/text paths read already-installed inert state.

`clay:decorations` and `clay:parse` are importable runtime modules, but their public Phase 18 functions currently delegate to the planned-unavailable op. The typed Rust validator/coordinator code exists for the handoff; the API inventory maps those planned facades separately from the implemented package-loading API so only registry-ready surfaces are public.

## Invariants and Constraints

- Package-manager process calls, conflict checks, mode activation/manifest selection, Phase 18.4 UI/input/state/layout/configuration metadata checks, `serverLoadPackage` validation, `setPackageOption`, and `serverSetLayoutOverride` happen only at explicit install/load/enable/activation/reload/configuration-change time, never in keypress, paint, layout, scroll, or text-event handlers.
- Conflicts never silently override behavior.
- Package SDUI/status and package UI contributions are inert descriptors; no package widget code, client JavaScript, raw CSS, renderer callbacks, direct Masonry widgets, draw callbacks, or native handles reach the client.
- SDUI/package UI action authority is inherited from target commands and their permissions; declaring a region, panel, component, or overlay alone grants no command authority.
- Installed packages do not gain filesystem, network, shell, AI, workspace, package-enable, package-manager execution, raw-op, native-widget, client-JS, or runtime-execution authority merely by being present in the package store.
- Duplicate mode IDs across different package prefixes are normally rejected earlier by package-owned ID rules, but the conflict pass still preserves deterministic mode collision diagnostics for enabled records.

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
