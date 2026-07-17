# Persistent Runtime Hot Reload

## Source

- `src/server/mod.rs`
- `src/server/js_runtime.rs`
- `src/server/connection.rs`
- `src/server/{command_execution,control_center,locks}.rs`
- `src/server/parse_coordinator.rs`
- `src/server/{completion,language_intelligence,document_analysis}.rs`
- `src/server/{behavior,sdui,ui,syntax,decorations,diagnostics}.rs`
- `src/server/workspace.rs`
- `src/protocol/runtime.rs`
- `src/client/{mod,runtime_state}.rs`
- `src/masonry_editor.rs`
- `src/masonry_sdui.rs`
- `src/shell/package_ui.rs`
- `src/editor/{surface,typography}.rs`
- `src/perf/budgets.rs`
- `runtime/js/packages.ts`
- `tests/persistent_runtime_hot_reload.rs`
- `tests/runtime_update_protocol.rs`
- `tests/parse_coordinator.rs`
- `tests/package_loading_docs.rs`

## Overview

Phase 19 hot reload replaces Clay's server-side JavaScript runtime as a generation, not by mutating a live V8 isolate. A successful reload builds a fresh `ClayJsRuntimeService`, reruns configured/default `init.js`, prepares and validates one `RuntimeGenerationCandidate` without mutating active state, commits its validated server state, refreshes open documents through generic mode activation, and cancels stale generation-owned work.

Evaluation, output validation, provider registration validation, package-UI/syntax validation, and open-document refresh inventory happen before commit. Any preparation failure keeps the previous runtime, behavior, SDUI, theme, typography, providers, workers, diagnostics caches, and client-visible state active and reports a sanitized diagnostic.

## Responsibilities

- Own active runtime generation state and serialized candidate commit through `RuntimeGenerationStore`.
- Retain the committed `ClayRuntimeEvaluation` behind `Arc` with the generation service so package records/modes/commands in the runtime and inert UI/syntax/provider snapshots share one generation identity.
- Keep `loadPackage` idempotent inside one generation and empty in the next generation.
- Stage behavior, SDUI, theme, typography, package UI, syntax grammars/preferences, decorations, diagnostics, parse/completion/intelligence providers, document analyzers, and open-document refresh metadata before mutation.
- Tag handlers/providers/tasks with runtime generation IDs; same-ID completion/intelligence providers replace only strictly older generations.
- After commit, cancel every older generation through shared `cancel_older_generations`/`cancel_package` primitives, drain queued parse/completion/analysis outputs, and shut down previous-generation language-server sessions.
- Refresh already-open documents after successful reload without sending full-document snapshots for unchanged documents.
- Route explicit user reloads through the built-in `clay.runtime.reloadConfiguration` command and the same `CommandExecutor` validation used by keybindings, SDUI actions, transient menus, and Control Center.
- Reject concurrent reload attempts immediately with `ReloadInProgress`; candidate evaluation remains outside the behavior lock.
- Acquire the generic server-owned `ScopedLockTarget::Behavior` only for candidate compare-and-swap commit and release it before document refresh.
- Provide deterministic non-GUI reload testing through `IpcServer::trigger_developer_hot_reload`, which delegates to the same command/service path.

Non-responsibilities:

- No public Clay JS reload API.
- No package-manager execution during reload.
- No non-`@clay/*` package loading.
- No JavaScript in keypress, paint, layout, scroll, edit acknowledgement, or text-event hot paths.

Current Phase 19 boundary (Plan 054 complete): candidate preparation/commit through two-phase validate-then-mutate, generic behavior-scoped command serialization via `ScopedLockManager`, post-commit generation-owned contribution replacement/cleanup (`cancel_older_runtime_generations`, `shutdown_generation_resources`), live `RuntimeStateSnapshot` fan-out (capacity-16 broadcast, latest-snapshot lag recovery), atomic client install via `ClientRuntimeStateCandidate` (validate-then-mutate, fail-close on invalid, ack after install), bounded previous-generation stale-edit grace (`BehaviorGraceState`, 2s/256-transaction ceilings, `InvalidBehaviorVersion` resync), preserved one-line `loadPackage` reload lifecycle docs, documented explicit built-in `clay.runtime.reloadConfiguration` command with compiled-budget table, rejected hidden keys, verified Clay JS API/command metadata boundaries (pub(crate) lifecycle helpers, no JS facade), and end-to-end verification tests (duplex barrier, multi-client atomic install, authority denial, LSP cleanup, budget locks). Full Linux gates passed.

## How It Works

1. `RuntimeGenerationStore` stores `{ id, ClayJsRuntimeService, Arc<ClayRuntimeEvaluation>, diagnostics }` plus a commit mutex.
2. `IpcServer::reload_runtime_generation` constructs the next service off to the side and evaluates configuration while the current generation remains active.
3. Configuration reruns `~/.config/clay/init.js`; package authors normally call `await loadPackage("@clay/markdown")` there. `runtime/js/packages.ts` keeps `globalThis.__clayLoadedPackages` as a per-generation idempotence cache.
4. `prepare_runtime_generation_candidate` clones expected active behavior/SDUI/theme/typography, stages replacements, validates package UI through `PackageUiRuntimeState`, validates the final syntax snapshot, validates decoration/diagnostic sets, registers all executable adapters into temporary coordinators, verifies document-analyzer package/process grants, builds and validates one complete `RuntimeStateSnapshot` (including encode against the 1 MiB frame ceiling), and captures bounded open-document refresh metadata.
5. `execute_reload_command` validates the Clay-owned command through `CommandExecutor` and uses an immediate `try_lock_owned` attempt guard. Concurrent requests return `CommandExecutionRule::ReloadInProgress` instead of queueing or starting a second evaluation.
6. `commit_runtime_generation` acquires `ScopedLockTarget::Behavior`, compares expected generation and active snapshots, installs already-validated generation-owned registrations/state, swaps the generation store once, calls `cancel_older_runtime_generations` across parse/completion/intelligence/analysis coordinators (abort older work and drain queued outputs), shuts down previous-generation language-server sessions, publishes the committed `RuntimeStateSnapshot` on the capacity-16 runtime-state broadcast, and drops the behavior lock before document refresh. Typography broadcasts only after commit; lagged runtime-state receivers recover from the latest complete snapshot rather than replaying intermediate generations.
7. Connected clients receive `ServerMessage::RuntimeStateSnapshot`. The receive loop applies only a protocol-level gate (matching `client_id` + `validate()`); invalid snapshots fail-close without acknowledgement. The editor validates a full `ClientRuntimeStateCandidate`, installs behavior/theme/typography/SDUI/package UI/render caches in one pass while preserving caret/selection/viewport, then acknowledges with `ClientMessage::RuntimeGenerationInstalled`. Stale/duplicate generations are silent no-ops; spoofed client IDs and future generations are ignored on the server.
8. After commit, `BehaviorGraceState` retains only the immediately previous inert behavior manifest. Unacknowledged connections may submit G1 `Edit`/`EditorIntent` stamps that pass normal lease/version/lock checks until the first of: G2 acknowledgement, two seconds, 256 accepted G1 transactions, another commit, or shutdown. Expired/older/future stamps receive `InvalidBehaviorVersion`, the latest runtime snapshot is republished, and the client requests canonical document resync. Commands never receive grace.
9. `refresh_open_documents_after_reload` consumes the prepared open-document inventory, reruns `connection::open_document_followup_messages`, and returns behavior manifests, decoration sets, and diagnostics without unchanged full-document snapshots.
10. Evaluation, preparation, or compare-and-swap conflict returns `RuntimeReloadOutcome { reloaded: false, ... }`, keeps the prior generation active, and records one sanitized `RuntimeDiagnostic`.

## Primitive Coverage

- Runtime generation primitive: `RuntimeGenerationStore` in `src/server/mod.rs`.
- package cache invalidation primitive: per-generation `loadPackage` cache in `runtime/js/packages.ts` plus `PackageLoadEntryAllowlist` in `src/server/js_runtime.rs`.
- parse-handler generation replacement primitive: `ParseCoordinator::register_handler_for_generation`, `cancel_older_generations`, `cancel_generation`, `cancel_package`, and task-generation validation in `src/server/parse_coordinator.rs`; package-scoped cancellation reuses the same primitive for revocation.
- Contribution cleanup primitive: `cancel_older_runtime_generations` plus `withdraw_package_contributions` in `src/server/mod.rs`, mirrored by `ModeRegistry::unregister_package_modes` and `CommandRegistry::remove_package_commands`.
- Language-server generation teardown: `LanguageServerProcessService::shutdown_all` via `ClayJsRuntimeService::shutdown_generation_resources`.
- Open-document refresh primitive: `WorkspaceState::open_document_snapshots` plus `refresh_open_documents_after_reload`.
- Scoped-lock primitive: `ScopedLockManager`, `ScopedLockTarget`, and RAII `ScopedLockGuard` in `src/server/locks.rs`; range overlap is shared with `DocumentState` region-lock checks.
- Reload command primitive: `clay.runtime.reloadConfiguration`, built-in `ServerFirstWithLock { Behavior }`, empty permissions/bindings, and Control Center discovery. `bindKey` may explicitly add a user binding; package JavaScript cannot call it through `serverExecuteCommand`.
- Runtime snapshot fan-out primitive: `ActiveRuntimeStateFanout` in `src/server/mod.rs` plus `src/protocol/runtime.rs` (`RuntimeStateSnapshot`, `RuntimeGenerationInstalled`). Connections subscribe, stamp `client_id` on send, and recover from `latest_runtime_snapshot_for` after lag.
- Atomic client install primitive: `ClientRuntimeStateCandidate::validate` in `src/client/runtime_state.rs` plus `EditorWidget::install_runtime_state_snapshot` in `src/masonry_editor.rs`. Typography force-installs through `EditorSurface::install_runtime_typography`; package UI replaces through `PackageUiRuntimeState::install_runtime_snapshot`.
- Stale-edit grace primitive: `BehaviorGraceState` in `src/server/behavior.rs` with `PREVIOUS_BEHAVIOR_GRACE_MS` / `PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS` ceilings; Edit/EditorIntent validation in `src/server/connection.rs`; client `InvalidBehaviorVersion` resync via `rejection_requests_resync`.
- Test/developer trigger: `IpcServer::trigger_developer_hot_reload`, marked `#[doc(hidden)]`, not exported through Clay JS facades, and delegated to command validation.

Future packages should reuse `loadPackage`, `clay:modes`, and `clay:parse` registration. Do not add mode-specific Rust reload branches.

## Invariants and Constraints

- No live server state mutates until configuration evaluation and every candidate validator succeeds.
- One reload attempt runs at a time; concurrent triggers fail immediately rather than queueing.
- Candidate commit holds only the behavior-scoped RAII lock and rejects a stale expected generation or changed active behavior/SDUI/theme/typography snapshot before mutation.
- Failed evaluation/preparation keeps previous runtime generation and package state active.
- Reload preserves module loading through recorded package allowlist entries and resolver-validated package `loadEntry` imports; package disable/revoke can withdraw package-owned allowlist entries with `PackageLoadEntryAllowlist::revoke_package`.
- Diagnostics are sanitized: no absolute paths, secrets, source snippets, URLs, or raw tokens.
- Parse results publish only if document version and runtime generation still match active state.
- Open-document refresh emits no `DocumentOpened` or `DocumentReloaded` full-text snapshots for unchanged documents.

## Tests

- `cargo test candidate_ --lib`: invalid candidate changes no active state; valid candidate advances retained generation state once.
- `cargo test --test runtime_update_protocol`: snapshot/ack codec round trips, complete install surface, and oversized/invalid rejection.
- `cargo test --lib client::runtime_state masonry_editor::tests::client_installs masonry_editor::tests::invalid_snapshot masonry_editor::tests::runtime_install`: atomic client install, fail-close without ack, caret/viewport preservation, and single layout invalidation.
- `cargo test --lib behavior::tests runtime_generation_tests::edit_sent_before_snapshot runtime_generation_tests::previous_generation_edit runtime_generation_tests::grace_never`: grace accept/reject/lease and InvalidBehaviorVersion snapshot republish.
- `cargo test --lib runtime_generation_tests`: candidate commit, concurrent-attempt rejection, failure lock release, timeout rollback, typography publication, package cache replacement, open-document refresh, snapshot fan-out, lag recovery, spoofed-ack rejection, duplex-barrier edit-ack non-blocking, failed-reload diagnostic/snapshot absence, multi-client one-generation install, and LSP authority/cleanup regression.
- `cargo test --lib locks::tests`: generic range/document/behavior/workspace conflict and RAII release semantics.
- `cargo test --lib behavior::tests`: BehaviorGraceState accept/expire/cap and grace-boundary rejection.
- `cargo test --test command_execution reload_command_is_server_first_behavior_locked_and_discoverable`: command metadata and shared validation.
- `cargo test --test persistent_runtime_hot_reload`: success, rollback, sanitized diagnostics, authority-denial regression, and failed-reload generation preservation.
- `cargo test --test parse_coordinator`: generation replacement, cancellation, stale result rejection, and handler failure instrumentation.
- `cargo test --test package_loading_docs`: docs-as-code coverage for hot reload lifecycle, package author docs, and wiki links.
- `cargo test --test performance_protocol phase19_runtime_state_snapshot_and_grace_budgets_are_locked`: Phase 19 budget constant locks.
- `cargo test --test runtime_update_protocol`: snapshot/ack codec round trips, complete install surface, oversized/invalid rejection, and diff-review payload under frame ceiling.

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Package Loading](package-loading.md)
- [Parse Coordinator](parse-coordinator.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Server File Workspace Model](server-file-workspace.md)
- [Phase 19 Persistent Runtime Hot Reload Primitive Review](phase19-persistent-runtime-hot-reload-primitive-review.md)
- `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md`
