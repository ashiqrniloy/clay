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

Phase 17 package loading turns a `package.json` Clay metadata block into a typed `PackageRecord`, enables it through `PackageService`, and rejects cross-package conflicts before contributions become active. Installation remains separate from enable/load: install records package metadata through a delegated package-manager boundary, while enable performs Clay-owned validation and conflict checks without executing package runtime JavaScript. The `clay package add|remove|list|enable|disable|inspect` CLI routes through the same service boundary intended for future in-app package UI. Runtime facade wiring now lets the controlled server-side JavaScript runtime import `clay:packages`, `clay:modes`, `clay:commands`, `clay:decorations`, and `clay:parse`; `serverLoadPackage` routes through the same package record assembler and is documented in the generated Clay JS API registry, while per-document manifest selection and decoration/parse public calls remain explicit planned handoff APIs.

## Responsibilities

- Assemble validated package records with package name, version, API prefix, docs path, performance metadata, API dependencies, and inert contribution descriptors.
- Retain contribution provenance for commands, configuration keys, key-routing entries, text transforms, SDUI/status regions, and decoration/render primitive descriptors.
- Validate package SDUI/status descriptors as inert metadata, reject executable widget/native fields, and enforce `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` and `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` estimates at enable/load time.
- Run one deterministic conflict pass across the enabled package set for prefixes, modes, commands, key bindings, configuration keys, SDUI regions, decoration primitives, and behavior-manifest entries.
- Delegate package-manager process work through `PackageManagerBackend`/`PnpmBackend` and keep captured stdout/stderr/exit-code handling at a typed boundary.
- Route `clay package ...` CLI operations through `PackageService` instead of duplicating install/enable/list/inspect logic.
- Roll back `PackageService::enable` when the candidate package would introduce a conflict.
- Select per-document behavior manifests from one active major mode plus compatible non-overriding minor modes, preserving package/mode/behavior-version provenance.
- Provide load/configuration-time runtime facades that call typed Rust validators without exposing raw op names as user-facing API exports.
- Document package loading scope, conflict handling, hot-path exclusion, and Phase 18 decoration/parse handoff in `docs/reference/primitives/package-loading.md`.

## How It Works

`assemble_package_record` first reuses the Phase 16.5 manifest validator, then parses contribution descriptors from `clay.contributions`. Descriptors are inert data only. Package-owned IDs must use the package `apiPrefix` namespace and may not claim `clay.*`. Command and configuration descriptors require their matching package permissions; SDUI/status descriptors do not grant authority and embedded actions still target separately declared commands.

`check_enabled_packages` receives enabled `PackageRecord` values, sorts them by prefix/name/version, and builds `BTreeMap` indices. Each insertion either records provenance or returns a `PackageConflictDiagnostic` containing the conflict kind, contribution ID, first package provenance, second package provenance, and an actionable message. The pass is deterministic because record order and index ordering are stable.

`PnpmBackend` implements `PackageManagerBackend` with `pnpm add`, `pnpm remove`, and `pnpm list --json --long`. It captures stdout/stderr and exit codes in typed results/errors and reads discovered `package.json` files back into Clay as inert JSON. `PackageService::install` records that JSON without enabling or executing the package; `PackageService::enable` validates the raw installed manifest into a `PackageRecord`, temporarily inserts it into the enabled set, runs `check_enabled_packages`, and removes the candidate again if any conflict is reported. This keeps failed enables from partially activating prefixes, modes, commands, configuration keys, SDUI regions, or behavior entries.

`src/main.rs` parses the `clay package` subcommands and constructs a service using the default Clay package store under the user's config directory. The CLI surface is intentionally thin: package state transitions and diagnostics stay in `PackageService`, while package-manager execution stays in `PackageManagerBackend`.

Per-document mode loading uses `ModeRegistry` to store one active major mode per document, optional compatible minor modes, and the selected inert `BehaviorManifest` keyed by document ID. Minor modes must declare the active major mode as compatible and may append behavior entries only when they do not replace major-mode entries. Each selection records major/minor package provenance and a behavior version for client manifest installation and later parse/decorations handoff.

`op_clay_packages_load_package` accepts a `package.json`-shaped value from the controlled runtime facade, calls `assemble_package_record`, and returns typed summary metadata (identity, entries, docs, performance estimate, API dependencies, and contribution counts). This gives package load fixtures a runtime path through the full Phase 17 validator without installing packages, spawning package managers, or executing package entry points.

`clay:decorations` and `clay:parse` are importable runtime modules, but their public Phase 18 functions currently delegate to the planned-unavailable op. The typed Rust validator/coordinator code exists for the handoff; the API inventory maps those planned facades separately from the implemented package-loading API so only registry-ready surfaces are public.

## Invariants and Constraints

- Package-manager process calls, conflict checks, mode activation/manifest selection, and `serverLoadPackage` validation happen only at explicit install/load/enable/activation/reload time, never in keypress, paint, layout, scroll, or text-event handlers.
- Conflicts never silently override behavior.
- Package SDUI/status contributions are inert descriptors; no package widget code, client JavaScript, draw callbacks, or native handles reach the client.
- SDUI action authority is inherited from target commands and their permissions; declaring an SDUI region alone grants no command authority.
- Installed packages do not gain filesystem, network, shell, AI, workspace, package-enable, or runtime-execution authority merely by being present in the package store.
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
- Existing package record, manager/service/CLI, conflict, SDUI provenance, and per-document manifest selection tests in `tests/package_loading.rs`
- `runtime_imports_modes_commands_and_packages_facades` and `phase18_primitive_facades_remain_explicitly_planned` in `src/server/js_runtime.rs`
- `package_loading_doc_linked_from_indexes_and_marks_phase17_ready`, `package_loading_keeps_validation_and_parsing_out_of_typing_hot_path`, and `phase18_only_apis_remain_planned_or_documented_without_raw_op_exposure` in `tests/package_loading_docs.rs`

## Related

- [Package Primitive Gate](package-primitive-gate.md)
- [Mode Registry](mode-registry.md)
- [Command Registry](command-registry.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- `docs/reference/primitives/package-security.md`
- `.agents/skills/project-patterns/references/package-distribution.md`
