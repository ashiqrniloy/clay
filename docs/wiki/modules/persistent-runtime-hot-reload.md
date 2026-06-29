# Persistent Runtime Hot Reload

## Source

- `src/server/mod.rs`
- `src/server/js_runtime.rs`
- `src/server/connection.rs`
- `src/server/parse_coordinator.rs`
- `src/server/workspace.rs`
- `runtime/js/packages.ts`
- `tests/persistent_runtime_hot_reload.rs`
- `tests/parse_coordinator.rs`
- `tests/package_loading_docs.rs`

## Overview

Phase 19 hot reload replaces Clay's server-side JavaScript runtime as a generation, not by mutating a live V8 isolate. A successful reload builds a fresh `ClayJsRuntimeService`, reruns configured/default `init.js`, reloads first-party packages through the existing `loadPackage` path, performs an atomic reload swap of the active generation, refreshes open documents through generic mode activation, and cancels stale parse work from the old generation.

Failed reloads keep the previous generation active and report sanitized diagnostics.

## Responsibilities

- Own active runtime generation state through `RuntimeGenerationStore`.
- Keep `loadPackage` idempotent inside one generation and empty in the next generation.
- Tag parse handlers and parse tasks with runtime generation IDs.
- Refresh already-open documents after successful reload without sending full-document snapshots for unchanged documents.
- Provide deterministic non-GUI reload testing through `IpcServer::trigger_developer_hot_reload`.

Non-responsibilities:

- No public Clay JS reload API.
- No package-manager execution during reload.
- No non-`@clay/*` package loading.
- No JavaScript in keypress, paint, layout, scroll, edit acknowledgement, or text-event hot paths.

## How It Works

1. `RuntimeGenerationStore` stores `{ id, ClayJsRuntimeService, diagnostics }` behind a mutex.
2. `IpcServer::reload_runtime_generation` constructs the next service off to the side and evaluates configuration before swap.
3. Configuration reruns `~/.config/clay/init.js`; package authors normally call `await loadPackage("@clay/markdown")` there.
4. `runtime/js/packages.ts` keeps `globalThis.__clayLoadedPackages` as a per-generation idempotence cache. A fresh service starts with an empty cache.
5. Successful evaluation applies runtime outputs, registers parse handlers with the new generation ID, cancels old-generation parse work, and swaps the store.
6. `refresh_open_documents_after_reload` enumerates `WorkspaceState::open_document_snapshots`, reruns `connection::selected_file_open_followup_messages`, and returns only behavior manifests, decoration sets, and diagnostics.
7. Failed evaluation returns `RuntimeReloadOutcome { reloaded: false, ... }`, keeps the old generation active, and records a sanitized `RuntimeDiagnostic`.

## Primitive Coverage

- Runtime generation primitive: `RuntimeGenerationStore` in `src/server/mod.rs`.
- package cache invalidation primitive: per-generation `loadPackage` cache in `runtime/js/packages.ts` plus `PackageLoadEntryAllowlist` in `src/server/js_runtime.rs`.
- parse-handler generation replacement primitive: `ParseCoordinator::register_handler_for_generation`, `cancel_generation`, `cancel_package`, and task-generation validation in `src/server/parse_coordinator.rs`; package-scoped cancellation reuses the same primitive for revocation.
- Open-document refresh primitive: `WorkspaceState::open_document_snapshots` plus `refresh_open_documents_after_reload`.
- Test/developer trigger: `IpcServer::trigger_developer_hot_reload`, marked `#[doc(hidden)]` and not exported through Clay JS facades.

Future packages should reuse `loadPackage`, `clay:modes`, and `clay:parse` registration. Do not add mode-specific Rust reload branches.

## Invariants and Constraints

- Atomic swap only after successful configuration evaluation.
- Failed reload keeps previous runtime generation and package state active.
- Reload preserves module loading through recorded package allowlist entries and resolver-validated package `loadEntry` imports; package disable/revoke can withdraw package-owned allowlist entries with `PackageLoadEntryAllowlist::revoke_package`.
- Diagnostics are sanitized: no absolute paths, secrets, source snippets, URLs, or raw tokens.
- Parse results publish only if document version and runtime generation still match active state.
- Open-document refresh emits no `DocumentOpened` or `DocumentReloaded` full-text snapshots for unchanged documents.

## Tests

- `cargo test --test persistent_runtime_hot_reload`: success, rollback, sanitized diagnostics, and authority-denial regression.
- `cargo test --test parse_coordinator`: generation replacement, cancellation, stale result rejection, and handler failure instrumentation.
- `cargo test --test package_loading_docs`: docs-as-code coverage for hot reload lifecycle, package author docs, and wiki links.
- `cargo test --lib`: server integration coverage for generation swap, package reload, open-document refresh, and connection behavior.

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Package Loading](package-loading.md)
- [Parse Coordinator](parse-coordinator.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Server File Workspace Model](server-file-workspace.md)
- [Phase 19 Persistent Runtime Hot Reload Primitive Review](phase19-persistent-runtime-hot-reload-primitive-review.md)
- `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md`
