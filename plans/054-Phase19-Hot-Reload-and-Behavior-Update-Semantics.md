# Phase 19 Hot Reload and Behavior Update Semantics

## Objectives

- Complete the current roadmap Phase 19 on top of the generation-swap baseline completed in `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md`.
- Reload configuration and loaded package contributions through one staged, server-authoritative runtime generation transaction.
- Deliver behavior, theme, typography, SDUI/package UI, decoration, diagnostic, mode, command, completion, syntax, language-intelligence, and analysis changes to affected clients without mixed-generation state.
- Preserve immediate local typing and bounded background rendering while defining explicit stale-edit, lock, rollback, cancellation, and recovery semantics.
- Expose one deterministic user/agent reload command through existing command execution and Control Center primitives; do not add a file-watcher dependency unless explicit reload proves insufficient.

## Expected Outcome

- Editing `~/.config/clay/init.js` or a loaded first-party package and invoking the reload command builds and validates a fresh runtime generation before any active state changes.
- A valid generation is committed once, broadcast to connected clients, and installed atomically with monotonically advancing behavior/rendering metadata; old asynchronous work is cancelled or stale-dropped.
- A failed generation leaves runtime, package contributions, behavior, rendering, subprocess grants/sessions, open documents, and client state on the previous generation while publishing sanitized diagnostics.
- Edits already produced under the immediately previous behavior generation have bounded grace semantics; later stale edits are rejected with deterministic resync instead of silently mutating canonical state.
- Package authors, users, app help, and AI agents can discover reload lifecycle, command, limitations, permissions, diagnostics, and one-line `loadPackage(...)` behavior from maintained docs and registry surfaces.

## Tasks

- [x] Verify the Phase 19 entry gate and review existing reload/package primitives before implementation
  - Acceptance Criteria:
    - Functional: Confirm active Plans 036-044 and 046-053 are complete, confirm Plan 045 is explicitly superseded by completed Plan 047 and its approved tiered-engine decision, treat Plan 033 as the implemented generation-swap baseline, and inventory current runtime, package, mode, command, behavior, syntax, completion, UI, decoration, diagnostic, analysis-worker, language-server, protocol, and client-install primitives against the current roadmap Phase 19 focus areas.
    - Performance: Identify every reload operation that must remain server-first/background and record existing typed payload, timeout, heap, queue, parse, decoration, diagnostic, completion, and worker budgets; ordinary typing, local paint, layout, scroll, pointer, and edit acknowledgement remain independent of reload JavaScript.
    - Code Quality: Document what existing generic primitives already provide, identify current gaps precisely, and prohibit mode/language/package-specific Rust reload branches.
    - Security: Record current deny-by-default module loading, package provenance/capability checks, configuration-root rules, trusted-subprocess disclosure, executable callback rejection, selected-file/workspace authority, and generation cleanup requirements before proposing changes.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` — current Phase 19 entry gate, focus areas, and expected outcome.
      - `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md` — completed partial Phase 19 baseline.
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/reference/primitives/package-security.md`, `docs/reference/primitives/parse-update-strategy.md`.
      - `docs/wiki/modules/{persistent-runtime-hot-reload,embedded-js-runtime,behavior-manifests,decoration-transport,slot-aware-package-ui,completion-snippet-expansion,language-server-process-service}.md`.
      - `.agents/skills/project-patterns/references/{authority-boundaries,behavior-manifests,extensions-and-ai,mode-primitive-first,protocol-and-performance,package-ui-layout,planning-checklist}.md`.
      - `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md` and `decision-logs/2026-07-15-1750-lsp-document-sync-and-package-worker-authority.md`.
    - Options Considered:
      - Reuse the old Phase 19 primitive review unchanged: shortest, but it predates command execution, package UI, syntax engines, completion, diagnostics, analysis workers, and LSP contributions.
      - Extend the existing review page in place: risks erasing the historical Plan 033 baseline.
      - Add a current-scope successor review linked to the old review. Chosen; preserves history and makes the new gaps testable.
    - Chosen Approach:
      - Create a successor primitive review with an existing-capability/gap matrix. Pin the root gaps already visible in source: `apply_runtime_evaluation` mutates shared state before `RuntimeGenerationStore::swap`, reload refresh results are returned to a developer caller rather than broadcast to live clients, package UI snapshots are not applied across IPC, and behavior/rendering updates are separate messages without one generation commit.
    - API Notes and Examples:
      ```text
      Plan 033 baseline: fresh JsRuntime generation + parse cancellation
      Phase 19 completion: stage all contributions -> validate -> commit -> broadcast -> atomic client install
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase19-hot-reload-behavior-update-primitive-review.md`: current primitive inventory, gaps, ownership, budgets, and rejected alternatives.
      - `docs/wiki/index.md`: link the successor primitive review.
      - `tests/primitives_docs.rs`: deterministic coverage for the Phase 19 entry gate and generic gap inventory.
    - References:
      - `.agents/skills/create-plan/references/clay.md`; `.agents/skills/project-patterns/references/mode-primitive-first.md`.
  - Test Cases to Write:
    - `cargo test --test primitives_docs phase19_hot_reload_behavior_update_primitive_review`: passed; requires entry-gate evidence, Plan 033 baseline linkage, all contribution categories, atomicity gaps, no-hot-path rule, and security boundaries.
    - `cargo test --test primitives_docs`: passed all 122 primitive documentation checks.
    - `cargo fmt --check`: passed.

- [x] Approve and record the runtime-generation transaction, stale-edit, trigger, and lock semantics
  - Acceptance Criteria:
    - Functional: Compared realistic transaction/install/stale-edit/trigger alternatives, obtained explicit user approval, and recorded exact semantics in an approved decision log before implementation.
    - Performance: Candidate evaluation keeps the old generation active, serializes reload attempts without queueing, acquires `LockScope::Behavior` only for final commit, uses capacity-16 snapshots bounded by the 1 MiB frame ceiling, and keeps typing/background parsing outside the lock.
    - Code Quality: The approved design defines one runtime-generation identity, one server rollback boundary, one atomic client-install acknowledgement, lag recovery from latest complete state, and measured snapshot-to-diff review thresholds of 768 KiB payload p95 or 16 ms install p95.
    - Security: Reload uses only configured/resolver-authorized sources and current grants; old executable authority is revoked logically at commit and workers/sessions terminate afterward under existing two-second graceful/five-second total ceilings.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md` and relevant project-pattern references.
      - `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`.
      - `decision-logs/2026-07-15-1750-lsp-document-sync-and-package-worker-authority.md`.
      - `src/server/{mod,behavior,connection,command_execution}.rs`, `src/client/{mod,behavior}.rs`, and protocol/budget definitions.
      - Context7 `/denoland/deno_core` plus version-exact local `deno_core 0.400.0` runtime/module lifecycle APIs.
    - Options Considered:
      - In-place mutation, server restart, multi-message client prepare/commit, immediate stale rejection, indefinite stale acceptance/old-runtime retention, full-evaluation global lock, reload-specific IPC, initial filesystem watcher, and initial diff/chunk protocol. Rejected for rollback, state-loss, complexity, authority, latency, duplication, or unmeasured-need costs recorded in the decision log.
      - Fresh staged candidate, one bounded complete client snapshot, atomic acknowledgement, and immediately previous inert-manifest grace. Approved.
    - Chosen Approach:
      - Prepare G2 while G1 remains active; commit once under the behavior lock; broadcast and atomically install per-connection snapshots; permit only normally valid G1 `Edit`/`EditorIntent` operations until that connection acknowledges G2 or the global two-second/256-accepted-transaction ceiling closes; reject/resync afterward.
      - Register `clay.runtime.reloadConfiguration` as a global `ServerFirstWithLock { Behavior }` built-in with no default binding. Keep watcher, dedicated IPC, and diffs deferred.
    - API Notes and Examples:
      ```text
      G1 active -> prepare/validate G2 -> behavior-lock CAS commit -> broadcast snapshot
      client validate/install G2 -> RuntimeGenerationInstalled(G2)
      eligible G1 edit before ack/ceiling -> validate; later G1 edit -> InvalidBehaviorVersion + latest runtime snapshot + document resync
      ```
    - Files Created/Edited:
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`: approved exact decision, evidence, alternatives, limits, consequences, and revisit conditions.
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`: staged transaction, lock, trigger, rollback, and authority-cleanup defaults.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: bounded previous-inert-manifest grace and correction rules.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: complete bounded runtime snapshot exception and measured diff threshold rule.
      - `docs/wiki/modules/phase19-hot-reload-behavior-update-primitive-review.md`: approved semantics and decision-log linkage.
      - `plans/054-Phase19-Hot-Reload-and-Behavior-Update-Semantics.md`: completion evidence and approved references.
    - References:
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
      - `.agents/skills/project-patterns/references/{extensions-and-ai,behavior-manifests,protocol-and-performance}.md`.
  - Test Cases to Write:
    - Manual decision-log review: passed; frontmatter records approved status and explicit approval, and the body records exact transaction, acknowledgement, two-second/256-transaction grace, command/lock, authority cleanup, alternatives, evidence, consequences, and revisit thresholds.
    - Manual project-pattern review: passed; stable hot-reload, behavior-version, and bounded-protocol guidance links to the approved decision without copying the full log.
    - `cargo test --test primitives_docs phase19_hot_reload_behavior_update_primitive_review`: passed; approved-semantics wiki update preserves deterministic primitive-review coverage.
    - `cargo fmt --check` and `git diff --check`: passed.

- [x] Stage and validate a complete runtime generation before mutating active server state
  - Acceptance Criteria:
    - Functional: Replaced “evaluate then mutate shared state then swap” with a server candidate containing the fresh runtime service (and its package/mode/command state), retained `ClayRuntimeEvaluation`, staged behavior/theme/typography/SDUI, package UI, syntax grammars/preferences, parse/completion/intelligence registrations, document analyzers, decorations/diagnostics, and prepared open-document refresh metadata. Live connection snapshot serialization remains deliberately owned by the later protocol/fan-out task; that task must extend this same candidate before enabling live reload fan-out.
    - Performance: Candidate construction stays on the runtime/background path, respects existing V8 timeout/heap limits, captures bounded open-document metadata, and holds neither active-generation nor active-state locks during JavaScript evaluation or validation.
    - Code Quality: Reused `ClayRuntimeEvaluation`, existing validators, temporary instances of existing coordinators, and one serialized compare-and-swap commit; added no package loader, mode/command registry, parser path, singleton, or dependency.
    - Security: Candidate validation rechecks behavior/SDUI/UI/syntax/render payloads, parse/completion/intelligence provenance and permissions, and exact document-analyzer package/process grants. Evaluation/preparation failure changes no active runtime, grants, workers, registrations, behavior, SDUI, theme, typography, or diagnostics caches.
  - Approach:
    - Documentation Reviewed:
      - `src/server/mod.rs::{RuntimeGenerationStore,reload_runtime_generation,apply_runtime_outputs}` and coordinator registration/cancellation paths.
      - `src/server/js_runtime.rs::{ClayJsRuntimeService,ClayRuntimeEvaluation}` plus version-exact local `deno_core 0.400.0` runtime lifecycle.
      - `src/server/{behavior,sdui,ui,syntax,decorations,diagnostics,completion,language_intelligence,document_analysis}.rs`.
      - `docs/wiki/modules/{embedded-js-runtime,persistent-runtime-hot-reload,package-loading}.md` and approved Phase 19 decision.
    - Options Considered:
      - Clone every live registry: rejected as broad, expensive, and prone to divergence.
      - Validate JavaScript only and apply Rust outputs best-effort: rejected because one failed output/provider could leave mixed active state.
      - Build a compact candidate, validate executable adapters in temporary existing coordinators, retain complete inert evaluation snapshots, then commit only prevalidated values. Chosen.
    - Chosen Approach:
      - `prepare_runtime_generation_candidate` stages cloned active state, validates all inert snapshots, validates registrations against temporary coordinators, verifies analyzer authority, and captures open-document refresh input without live mutation.
      - `commit_runtime_generation` serializes commits, rejects stale generation/active-state expectations, installs strictly newer provider generations, replaces staged state, swaps one retained generation, then cancels old generation work and refreshes prepared documents. All candidate diagnostics are bounded/sanitized.
    - API Notes and Examples:
      ```rust
      let candidate = server
          .prepare_runtime_generation_candidate(current_id, next_id, service, evaluation)
          .await?;
      let refreshed = server.commit_runtime_generation(candidate).await?;
      ```
    - Files Created/Edited:
      - `src/server/mod.rs`: retained evaluation state, candidate prepare/commit/CAS flow, temporary registration validation, rollback diagnostics, staged refresh input, and regression tests.
      - `src/server/behavior.rs`: side-effect-free behavior staging and explicit staged install.
      - `src/server/ui.rs`, `src/server/syntax.rs`: package-UI and final syntax-snapshot validators.
      - `src/server/js_runtime.rs`: defaultable retained evaluation, parse provenance recheck, analyzer grant recheck, and generation replacement registration.
      - `src/server/{completion,language_intelligence,document_analysis}.rs`: validate-before-replace helpers for strictly older provider generations.
      - `src/server/connection.rs`: test runtime-generation construction updated for retained evaluation/commit state.
      - `docs/wiki/modules/persistent-runtime-hot-reload.md`, `tests/package_loading_docs.rs`: implementation flow and deterministic documentation coverage.
    - References:
      - `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md`; `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
      - `.agents/skills/project-patterns/references/{extensions-and-ai,authority-boundaries,maintenance-validation}.md`.
  - Test Cases to Write:
    - `candidate_validation_failure_changes_no_active_state`: passed; invalid staged SDUI after a valid behavior candidate leaves all sampled active state unchanged.
    - `candidate_commit_advances_all_server_generation_state_once`: passed; valid behavior/SDUI/theme/typography/evaluation state commits under generation 2.
    - `runtime_timeout_drops_candidate_service_and_keeps_old_generation`: passed with a 10 ms test runtime ceiling; timed-out fresh worker never becomes active.
    - `cargo test runtime_generation_tests --lib`: passed all 9 candidate/reload tests.
    - `cargo test --lib`: passed all 765 library tests.
    - `cargo test --test persistent_runtime_hot_reload`: passed both success/rollback/authority integration tests.
    - `cargo test --test package_loading_docs`: passed all 50 documentation lifecycle checks.
    - `cargo test --test rust_visibility_api_mapping`: passed all 16 public/internal API-boundary checks.
    - `cargo test --test primitives_docs phase19_hot_reload_behavior_update_primitive_review`: passed.
    - `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `git diff --check`: passed.

- [x] Add generic scoped reload locks and route an explicit reload command through existing command execution
  - Acceptance Criteria:
    - Functional: Added immediate server-owned range/document/behavior/workspace lock conflict semantics with RAII release. `clay.runtime.reloadConfiguration` is a Clay-owned built-in named “Reload Configuration and Packages,” declared `ServerFirstWithLock { Behavior }`, permission-free, unbound by default, discoverable through Control Center, and executable from existing validated connection command/SDUI/menu routes. One reload attempt runs at a time; a concurrent trigger returns `ReloadInProgress` without queueing.
    - Performance: The attempt guard covers admission/evaluation, but candidate evaluation holds no scoped mutation lock. Behavior acquisition is immediate and held only across final compare-and-swap/cancellation publication, then dropped before document refresh; document edits and client paint/layout paths do not consult it.
    - Code Quality: Reused `RoutingPolicy`, `LockOwner`, built-in command metadata, `CommandExecutor`, connection intent normalization, `bindKey`, and `DocumentState` range-overlap semantics. Added no client message, dispatcher, watcher, timeout setting, or dependency.
    - Security: Reload retains existing resolver/grant authority. Package-side `serverExecuteCommand` explicitly rejects reload as `UnauthorizedTarget`; only user command intents reach `IpcServer::execute_reload_command`. Tokio/RAII guards release on success, validation/evaluation/commit failure, cancellation/drop, disconnect, and unwind.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/{command-registry,persistent-runtime-hot-reload}.md`; `docs/wiki/flows/document-leases-and-region-locks.md`.
      - `src/protocol/mod.rs::{RoutingPolicy,LockScope,LockOwner}` and `src/server/{command_execution,control_center,document,connection,mod}.rs`.
      - `.agents/skills/project-patterns/references/{authority-boundaries,behavior-manifests,extensions-and-ai}.md`.
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
    - Options Considered:
      - New `ClientMessage::ReloadRuntime`: rejected; bypasses command discovery, behavior routing, SDUI/menu validation, and Control Center.
      - Hold one global/scoped lock across JavaScript evaluation: rejected; blocks unrelated work.
      - Reuse the task-3 commit mutex beside a new lock manager: rejected as duplicate serialization.
      - Immediate attempt admission plus generic RAII scoped lock only at commit. Chosen.
    - Chosen Approach:
      - `ScopedLockManager::try_acquire` validates targets and deterministically rejects workspace-wide, behavior, document, and overlapping same-document range conflicts. `ScopedLockGuard::drop` releases ownership; `DocumentState` and scoped ranges share `ranges_overlap`.
      - `IpcServer::execute_reload_command` validates built-in metadata through `CommandExecutor`, uses `try_lock_owned` for one-at-a-time admission, evaluates/prepares outside scoped locks, and calls commit. Connection intents return one status/diagnostic; the doc-hidden developer helper delegates to this path.
    - API Notes and Examples:
      ```javascript
      import { bindKey } from "clay:keybindings";
      bindKey("Ctrl+Shift+R", "clay.runtime.reloadConfiguration", { scope: "global" });
      ```
    - Files Created/Edited:
      - `src/server/locks.rs`: generic target validation, conflict metadata/rules, immediate acquisition, and RAII release.
      - `src/server/{mod,command_execution,control_center,connection,document}.rs`: reload admission/commit lock, built-in metadata/discovery, shared intent execution, status diagnostics, and shared range overlap.
      - `src/server/ops/{mod,keybindings}.rs`, `src/server/js_runtime.rs`: explicit user binding with behavior-lock routing and direct package-JS execution denial.
      - `tests/command_execution.rs`: built-in metadata/shared executor coverage.
      - `docs/wiki/modules/{command-registry,persistent-runtime-hot-reload,phase19-hot-reload-behavior-update-primitive-review}.md`, `docs/wiki/flows/document-leases-and-region-locks.md`, `tests/primitives_docs.rs`: synchronized implementation documentation and deterministic coverage.
    - References:
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`; `docs/reference/primitives/registry.md#CommandExecution`.
  - Test Cases to Write:
    - `reload_command_is_server_first_behavior_locked_and_discoverable`: passed; exact name, policy, empty binding/permissions, built-in listing, and shared executor validation.
    - `concurrent_reload_commands_commit_at_most_one_candidate_at_a_time`: passed; held admission guard returns `ReloadInProgress`, preserves generation, and next request succeeds after release.
    - `failed_reload_releases_attempt_lock` and `failed_candidate_commit_releases_behavior_lock`: passed; evaluation and post-acquisition commit failures leave no stale guard.
    - `range_document_workspace_lock_conflicts_reuse_generic_manager` and `behavior_lock_releases_on_guard_drop`: passed with deterministic conflict owner/target metadata.
    - `reload_command_intent_uses_shared_server_reload_service`: passed; normalized command intent reaches the same reload service and returns success diagnostic.
    - `configuration_can_explicitly_bind_reload_without_default_binding` and `package_javascript_cannot_directly_execute_reload_command`: passed.
    - `cargo test --lib runtime_generation_tests`: passed all 12 reload/candidate tests; `cargo test --test command_execution`: passed all 18 command tests.
    - `cargo test --test persistent_runtime_hot_reload`, `cargo test --test rust_visibility_api_mapping`, and focused primitive/package documentation tests: passed.
    - `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`: passed.

- [x] Replace and clean up every generation-owned package contribution after commit
  - Acceptance Criteria:
    - Functional: Successful commit installs candidate parse/completion/language-intelligence/document-analysis/syntax/mode/command/UI state, reclassifies open documents, schedules bounded replacement parse/decorations, and then cancels/removes old-generation handlers, tasks, workers, outputs, language-server sessions, and package-owned caches; failure performs none of those withdrawals.
    - Performance: Cancellation is non-blocking on edit acknowledgement, cleanup uses existing bounded shutdown/kill deadlines, reparsing is viewport-prioritized/cancellable, and no full-document IPC is introduced.
    - Code Quality: Every contribution category has explicit generation ownership and one replace/cancel path; package disable/revoke and runtime reload reuse the same package/generation cleanup primitives.
    - Security: Old grants cannot authorize new-generation work, late results cannot publish, subprocess/analysis-worker teardown follows approved trusted-subprocess lifecycle, and baseline modes/syntax/completion remain available if a package disappears.
  - Approach:
    - Documentation Reviewed:
      - `src/server/{parse_coordinator,completion,language_intelligence,document_analysis,language_server,syntax}.rs`.
      - `src/packages/{service,modes,commands}.rs`; `src/server/ui.rs`.
      - `docs/wiki/modules/{parse-task-lifecycle,completion-snippet-expansion,language-intelligence,language-server-process-service,syntax-grammar-registry,slot-aware-package-ui}.md`.
      - `decision-logs/2026-07-15-1750-lsp-document-sync-and-package-worker-authority.md`.
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
    - Options Considered:
      - Clear all registries before candidate load: rejected; failure destroys working state.
      - Let old work drain without generation checks: rejected; stale output can overwrite new state.
      - Commit new registries, switch active generation, then cancel older generation work and stale-drop late output. Chosen.
    - Chosen Approach:
      - Normalized `cancel_older_generations` across parse/completion/language-intelligence/document-analysis coordinators; post-commit orchestration through `cancel_older_runtime_generations` plus `ClayJsRuntimeService::shutdown_generation_resources`.
      - Package disable/revoke reuses `withdraw_package_contributions`, `ModeRegistry::unregister_package_modes`, and `CommandRegistry::remove_package_commands`.
      - Open-document refresh continues through generic mode activation; removed package modes fall back to `core.text`/`core.code`.
    - API Notes and Examples:
      ```rust
      register_runtime_contributions(candidate.id, &service, &evaluation, ...)?;
      runtime_generation.swap(candidate.generation.clone()).await;
      cancel_older_runtime_generations(candidate.id, &parse, &completion, &analysis, &intelligence);
      previous.service.shutdown_generation_resources().await;
      ```
    - Files Created/Edited:
      - `src/server/{parse_coordinator,completion,language_intelligence,document_analysis,language_server,js_runtime,ops/mod,mod}.rs`: generation replacement, drain, LS shutdown, and shared cleanup helpers.
      - `src/packages/{modes,commands}.rs`: package-scoped mode/command withdrawal.
      - `docs/wiki/modules/{persistent-runtime-hot-reload,phase19-hot-reload-behavior-update-primitive-review,parse-task-lifecycle,language-intelligence,language-server-process-service}.md`.
      - `tests/{rust_visibility_api_mapping,primitives_docs}.rs` and `src/server/mod.rs` runtime_generation_tests.
    - References:
      - `.agents/skills/project-patterns/references/{mode-primitive-first,authority-boundaries,protocol-and-performance}.md`.
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
  - Test Cases:
    - `successful_reload_replaces_all_provider_registries_and_cancels_old_work`: passed; coordinator snapshots retain only committed generations.
    - `failed_reload_keeps_workers_sessions_and_outputs_on_previous_generation`: passed; failed reload preserves gen-1 registries/sessions.
    - `late_old_generation_parse_completion_diagnostic_and_intelligence_output_is_dropped`: passed; cancel_older drains/drops late parse output.
    - `removed_language_package_reclassifies_to_core_fallback`: passed; `unregister_package_modes` falls back to `core.text`.
    - `withdraw_package_contributions_reuses_generation_cancel_primitives`: passed; shared package withdrawal path.
    - `cargo test --lib runtime_generation_tests`: passed (17).
    - `cargo test --test {parse_coordinator,completion_provider,persistent_runtime_hot_reload,rust_visibility_api_mapping}`: passed.
    - `cargo test --test primitives_docs phase19_hot_reload_behavior_update_primitive_review`: passed.
    - `cargo fmt --check` / `cargo check --all-targets`: passed.

- [x] Add a bounded runtime-generation snapshot protocol and live connection fan-out
  - Acceptance Criteria:
    - Functional: Introduced `RuntimeGenerationId`, complete connection-scoped `RuntimeStateSnapshot` (behavior, theme, typography, SDUI, versioned package UI, per-document render reset/initial sets, diagnostics), `ServerMessage::RuntimeStateSnapshot`, and `ClientMessage::RuntimeGenerationInstalled`; successful commits fan out to every connected client; lagged receivers recover from the latest complete snapshot; clients stage validated snapshots and acknowledge; spoofed/future acks are ignored. Full atomic client install remains Plan 054 task 7.
    - Performance: Capacity-16 Tokio broadcast; snapshots constructed/validated and encode-checked against the 1 MiB frame ceiling before commit; diffs/chunking deferred to the approved 768 KiB / 16 ms thresholds; ordinary edit frames unchanged.
    - Code Quality: Protocol semantics live in `src/protocol/runtime.rs` with `rkyv` behind `Codec`; `PROTOCOL_VERSION` advanced to 4; no per-feature ad hoc reload messages.
    - Security: Local IPC updates/acks treated as fallible; client identity/generation validated; snapshots exclude source text, absolute paths, tokens, grants, callbacks, and secrets; oversized/invalid snapshots fail closed before commit/install ack.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/{mod,codec,sdui,decorations,diagnostics}.rs`.
      - `docs/wiki/modules/protocol-codec.md` and `docs/wiki/modules/server-ipc-skeleton.md`.
      - Version-exact `tokio 1.52.2` local source for `tokio::sync::broadcast::{channel,Sender::send,Sender::subscribe}`.
      - `.agents/skills/rust-async-patterns/SKILL.md` channel/cancellation guidance.
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.
    - Options Considered:
      - Send existing Behavior/Theme/Typography/SDUI/Decoration messages independently: rejected; clients can paint mixed generations.
      - Add per-feature prepare/commit messages: flexible but too much state-machine surface for current bounded payloads.
      - One validated snapshot plus later generation-tagged async render chunks and an install acknowledgement. Chosen.
    - Chosen Approach:
      - `ActiveRuntimeStateFanout` stores the latest committed snapshot and broadcasts generation IDs; connections stamp `client_id` and always fetch latest on receive/lag. Prepare builds and encode-validates the snapshot; commit publishes it. Package UI wire payloads remain empty/versioned until package UI publication crosses IPC.
    - API Notes and Examples:
      ```rust
      let mut updates = runtime_generation.subscribe_runtime_state();
      // Lagged => send runtime_generation.latest_runtime_snapshot_for(client_id)
      ```
    - Files Created/Edited:
      - `src/protocol/runtime.rs`: snapshot, document render state, package UI version stub, validation.
      - `src/protocol/{mod,codec}.rs`: reexports, message variants, `PROTOCOL_VERSION=4`, codec tests.
      - `src/perf/budgets.rs`: broadcast capacity and snapshot count/diff-review ceilings.
      - `src/server/{mod,connection,sdui}.rs`: fan-out ownership, prepare/commit publish, subscribe/lag recovery, ack validation.
      - `src/client/mod.rs`: decode, stage event, acknowledge after validation.
      - `tests/runtime_update_protocol.rs` plus `runtime_generation_tests` fan-out coverage.
      - `docs/wiki/modules/{protocol-codec,persistent-runtime-hot-reload,phase19-hot-reload-behavior-update-primitive-review}.md`.
    - References:
      - Local `tokio 1.52.2`; `ActiveTypographyState` broadcast precedent; decision log 2026-07-16-1825.
  - Test Cases:
    - `runtime_state_snapshot_round_trips_with_generation_and_bounded_payload`: passed (codec + integration).
    - `oversized_or_invalid_runtime_snapshot_is_rejected_before_install`: passed.
    - `successful_reload_reaches_two_connected_clients`: passed.
    - `lagged_connection_receives_latest_snapshot_not_intermediate_generations`: passed.
    - `spoofed_or_future_install_ack_is_ignored`: passed.
    - `successful_reload_publishes_runtime_state_snapshot_to_subscribers`: passed.
    - `cargo test --lib runtime_generation_tests`: passed (21).
    - `cargo test --test {runtime_update_protocol,persistent_runtime_hot_reload,primitives_docs}`: passed.
    - `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`: passed.

- [x] Atomically install behavior and rendering state in the Rust client
  - Acceptance Criteria:
    - Functional: Client validates the full snapshot off to the side via `ClientRuntimeStateCandidate::validate`, then installs behavior, active theme, typography, SDUI/package UI, render generation, and per-document decoration/diagnostic reset/data as one `EditorWidget::install_runtime_state_snapshot` event; any invalid component rejects the entire snapshot, sends no acknowledgement, keeps no partial candidate, and fail-closes (`Disconnected` / connection error) so the shell can rebootstrap into latest authoritative state. Stale/duplicate generations are silent no-ops without ack.
    - Performance: Validation/install stays on the connection-event path outside Masonry paint/text-event handlers (source-guarded by `runtime_generation_install_stays_outside_paint_and_text_event_hot_paths`); layout invalidates once per successful install; no package JavaScript or blocking IPC enters client hot paths.
    - Code Quality: Reuses `ClientBehaviorState` validation, `StyleRegistry`, `TypographyRegistry` force-install, SDUI/`PackageUiRuntimeState::install_runtime_snapshot`, and editor decoration/diagnostic caches; one generation check (`runtime_generation_id`) replaces scattered ordering assumptions. Ack is deferred until after install via `ClientEditQueue::enqueue_runtime_generation_installed`.
    - Security: Client accepts only inert validated protocol data, rejects raw/executable/native/CSS authority, preserves caret/selection/viewport/focus connection status where compatible, and fail-closes/reconnects without acknowledging a rejected snapshot.
  - Approach:
    - Documentation Reviewed:
      - `src/client/{mod,behavior,runtime_state}.rs`.
      - `src/masonry_editor.rs`, `src/masonry_sdui.rs`, `src/shell/package_ui.rs`.
      - `src/editor/{surface,theme,typography}.rs`.
      - `docs/wiki/modules/{behavior-manifests,editor-theme-registry,typography-registry-and-font-roles,slot-aware-package-ui,masonry-editor,persistent-runtime-hot-reload}.md`.
    - Options Considered:
      - Apply events in arrival order: current behavior; rejected for mixed-generation paint.
      - Clone the whole editor widget: loses transient state and duplicates native resources.
      - Build validated replacement sub-state and swap/install it through one connection event. Chosen.
    - Chosen Approach:
      - Added `ClientRuntimeStateCandidate` that validates all inert parts, then apply it in one `EditorWidget`/shell mutation pass. Existing standalone startup messages remain for bootstrap; live reload uses only the atomic snapshot path. Receive-loop protocol gate fail-closes on invalid frames; editor owns full candidate validation and post-install ack.
    - API Notes and Examples:
      ```rust
      let candidate = ClientRuntimeStateCandidate::validate(snapshot, client_id, current_generation)?;
      editor.install_runtime_state_snapshot(snapshot); // one invalidation/event + ack
      ```
    - Files Created/Edited:
      - `src/client/runtime_state.rs`: staged validation and monotonic generation checks.
      - `src/client/mod.rs`: deferred ack, fail-close on invalid snapshot, enqueue helper.
      - `src/masonry_editor.rs`: one atomic runtime-state event and one layout/render invalidation.
      - `src/masonry_sdui.rs`, `src/shell/package_ui.rs`: package UI snapshot replacement without partial slot/overlay state.
      - `src/editor/surface.rs`: `install_runtime_typography`, `clear_decorations`, `clear_diagnostics`.
      - `tests/runtime_update_protocol.rs`, `tests/editor_performance_invariants.rs`: client rejection/atomicity and hot-path guards.
    - References:
      - `.agents/skills/project-patterns/references/{behavior-manifests,package-ui-layout,protocol-and-performance}.md`.
  - Test Cases Written:
    - `client_installs_behavior_theme_typography_ui_and_render_generation_atomically`: observation sees complete G2 install + single ack.
    - `invalid_snapshot_installs_nothing_and_disconnects_without_ack`: no partial behavior/theme/UI/cache change or acknowledgement; disconnect status set.
    - `runtime_install_preserves_caret_selection_viewport_and_focus_status`: transient editor state survives compatible reload.
    - `runtime_install_invalidates_layout_once`: typography/theme/UI update does not trigger repeated rebuilds; duplicate generation is a no-op.
    - `runtime_state_snapshot_is_staged_without_immediate_ack` / `invalid_runtime_state_snapshot_fail_closes_without_ack_or_event`: receive-loop contract.
    - `runtime_generation_install_stays_outside_paint_and_text_event_hot_paths`: source guard.

- [x] Enforce bounded stale-edit grace, rendering-generation rejection, and deterministic recovery
  - Acceptance Criteria:
    - Functional: Server retains only the immediately previous committed inert behavior manifest/metadata while affected connections remain eligible; normally valid G1 `Edit`/`EditorIntent` operations already rendered locally are accepted only before that connection acknowledges G2 and before the global two-second/256-accepted-G1-transaction ceiling, while expired/older/future versions are rejected without canonical mutation and trigger latest-runtime plus document resync. Old commands are rejected; decoration/diagnostic/SDUI/provider requests/results from non-active generations are dropped.
    - Performance: Grace validation is constant-time metadata/operation validation before the document lock, never evaluates JavaScript, stores bounded per-connection acknowledgement state plus one shared previous inert manifest, and drops that metadata on all acknowledgements, deadline/cap, next commit, disconnect, or shutdown; asynchronous replacement parsing remains cancellable/background.
    - Code Quality: Semantics are centralized in behavior/runtime generation validation rather than duplicated across edit, editor-intent, command, completion, intelligence, and render call sites.
    - Security: Grace cannot bypass lease, document version, transaction ordering, range, region/scoped lock, operation capability, package permission, client identity, or payload validation; no old runtime, worker, session, grant, provider, or callback is retained, and malicious non-acknowledging clients hit hard typed ceilings.
  - Approach:
    - Documentation Reviewed:
      - `src/server/behavior.rs`, `src/server/connection.rs`, `src/server/document.rs`.
      - `src/client/mod.rs::rejection_requests_resync` and client pending-edit state.
      - `docs/wiki/flows/{client-server-edit-ack,versioned-text-synchronization}.md`.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`.
    - Options Considered:
      - Reject every old behavior edit immediately: simple, but current snapshot resync may discard an edit already painted locally.
      - Accept old behavior forever: non-janky but lets stale clients pin revoked behavior.
      - Bounded one-generation inert-manifest grace until per-connection install acknowledgement or global two-second/256-transaction cap, then reject and resync. Approved.
    - Chosen Approach:
      - Store current/per-connection acknowledgement state plus one shared previous inert manifest, require G1 to be immediately previous and to allow the submitted `Edit`/`EditorIntent`, and still run every normal document/lease/order/range/lock/payload validation. Include `InvalidBehaviorVersion` in latest-runtime publication plus deterministic document resync after grace. Tag all async renderer/provider publications with runtime generation and check on server and client.
    - API Notes and Examples:
      ```rust
      match behavior.validate_for_client(client_id, behavior_version, operation_kind, now) {
          Current | PreviousWithinGrace => apply_normal_document_validation(),
          Expired | Older | Future => reject_and_resync(),
      }
      ```
    - Files to Create/Edit:
      - `src/server/{behavior,connection,mod}.rs`: previous-generation retention, per-client ack/grace state, centralized validation, recovery messages.
      - `src/client/mod.rs`: acknowledgement and resync/correction handling for expired behavior versions.
      - `src/protocol/{runtime,decorations,diagnostics,sdui}.rs`: generation metadata where asynchronous output can race commit.
      - `src/server/{parse_coordinator,completion,language_intelligence,document_analysis}.rs`: active-generation publication checks.
      - `src/editor/surface.rs`: reject/clear non-active decoration and diagnostic generations.
      - `tests/{persistent_runtime_hot_reload,runtime_update_protocol,performance_protocol}.rs`: race, grace, expiry, and stale rendering coverage.
    - References:
      - `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`; roadmap synchronization recovery follow-up.
  - Test Cases Written:
    - `edit_sent_before_snapshot_install_is_accepted_once_under_previous_generation`: G1 edit accepted under grace before ack.
    - `previous_generation_edit_after_ack_or_expiry_is_rejected_and_snapshot_resent`: after ack/expiry, InvalidBehaviorVersion + RuntimeStateSnapshot republish; document unchanged.
    - `grace_never_bypasses_lease_validation`: bad lease still rejected under grace.
    - `grace_accepts_immediately_previous_version_before_ack_deadline_and_cap` / `grace_rejects_after_ack_deadline_or_transaction_ceiling`: unit coverage for ceilings.
    - `invalid_behavior_version_rejection_requests_resync`: client requests canonical resync.
    - `old_generation_render_and_provider_outputs_never_reappear_after_commit`: covered by task 5 cancel_older_generations tests; executable authority revoked at commit.
    - `non_acknowledging_client_cannot_pin_old_generation_resources`: expire_for_test / 256-transaction ceiling drops inert metadata; workers/sessions already shut down at commit.

- [x] Preserve one-line package loading and document the package reload lifecycle
  - Acceptance Criteria:
    - Functional: First-party packages still load through ordinary `await loadPackage("@clay/...")`; reload reruns `init.js` in a fresh generation with an empty `globalThis.__clayLoadedPackages` cache, rebuilds modes/commands/syntax/completion/UI/parse (and LSP when granted), applies user overrides/grants in documented order, and requires no copied manifests, force flags, reload callbacks, fixture SDUI, or manual decoration publication.
    - Performance: Package author docs require load-time registration only, bounded/background viewport-prioritized reparsing, no synchronous package work before local paint, and no package state retained outside documented generation/persistence scopes.
    - Code Quality: `creating-packages.md` and `package-loading.md` distinguish generation-local state, explicitly persistent user/workspace state, and unsupported migration hooks; the same lifecycle applies to Markdown, Rust, TypeScript, JavaScript, Git, themes, and LSP bridges.
    - Security: Reload docs and facade comments state that reload does not broaden package source trust/permissions, reuses exact language-server grants re-declared in `init.js`, cleans old-generation workers/sessions after commit, and rejects executable client declarations/raw ops.
  - Approach:
    - Kept one-line `loadPackage` unchanged (no `{ force: true }` and no package `onReload`/`migrateState` hooks); strengthened generation-local cache comments in `runtime/js/packages.ts` and the embedded facade.
    - Documented the candidate/commit/cache-invalidation lifecycle in `docs/reference/packages/creating-packages.md` and `docs/reference/primitives/package-loading.md`, corrected the outdated Planned/target hot-reload note, and aligned Rust/TypeScript/JavaScript package pages plus the package-loading wiki.
  - Test Cases:
    - `reload_reruns_one_line_loads_and_rebuilds_representative_contributions` — passed (`src/server/mod.rs` runtime_generation_tests).
    - `load_package_remains_idempotent_inside_one_generation` — passed (`src/server/js_runtime.rs`).
    - `package_author_docs_cover_generation_local_state_and_rollback` — passed (`tests/package_loading_docs.rs`).

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: `clay.runtime.reloadConfiguration` is documented as the explicit built-in reload command with no default binding, empty permissions, and `ServerFirstWithLock { Behavior }` routing; optional `bindKey` exposure shown in configuration and bind-key docs; no auto-reload, file-watcher, debounce, polling, reload-on-save, or hidden configuration key is introduced.
    - Performance: Reload remains explicit server-first work with no hidden polling, watcher, or hot-path configuration check; compiled grace/snapshot/broadcast budgets are documented as not configurable from `init.js`.
    - Code Quality: `~/.config/clay/init.js` remains the only configuration entry point; every behavior-changing option is a documented Clay JS API or built-in command; no ad hoc JSON/TOML/environment key controls reload semantics or compiled ceilings.
    - Security: Configuration cannot self-expand package, process, filesystem, network, shell, workspace, AI, WASM, raw-op, native-widget, client-JS, or package-manager authority during reload; concurrent reload returns `ReloadInProgress`.
  - Approach:
    - Rewrote the `## Phase 19 persistent-runtime hot reload configuration review` section in `docs/reference/clay-js-api/configuration.md` from "developer-only/internal primitive" to accurate current state: built-in command, optional `bindKey`, compiled budgets table, rejected hidden keys list.
    - Updated `docs/reference/clay-js-api/keybindings/bind-key.md` with Phase 19 reload binding note and example.
    - Verified `docs/reference/clay-js-api/api-inventory.toml` has no `clay.runtime.*` JS facade entry (command-only, not a Clay JS API).
    - Updated existing `phase19_hot_reload_configuration_review_rejects_hidden_reload_keys` test to match new docs (removed `reloadConfiguration` from forbidden-keys list, added it as documented command ID).
  - Test Cases:
    - `phase19_reload_configuration_review_rejects_hidden_watcher_and_reload_keys` — passed (`tests/clay_js_api_inventory.rs`).
    - `reload_command_can_be_bound_through_existing_bind_key_api` — passed (`tests/clay_js_api_inventory.rs`).
    - `phase19_hot_reload_configuration_review_rejects_hidden_reload_keys` — updated, passed (`tests/clay_js_api_inventory.rs`).
    - `phase19_hot_reload_has_no_public_clay_js_api_surface` — unchanged, still passes.
    - `configuration_can_explicitly_bind_reload_without_default_binding` — unchanged, still passes (`src/server/js_runtime.rs`).

- [x] Create or verify Clay JS APIs and command metadata for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: `clay.runtime.reloadConfiguration` is the sole built-in reload command with stable ID, user-facing name ("Reload Configuration and Packages"), `ServerFirstWithLock { Behavior }` routing, empty default bindings, empty permissions, documented diagnostics, and lookup tags; no Clay JS facade or op is created (command-only surface); all reload lifecycle Rust helpers remain `pub(crate)` or private.
    - Performance: `server-register-command.md` classifies reload as `ServerFirstWithLock { Behavior }` and states no JavaScript/IPC/server round trip precedes ordinary local paint.
    - Code Quality: Raw `Deno.core.ops` and Rust helpers are not user-facing; generated registry is regenerated and fresh; markdown/index/generated registry artifacts pass `clay_js_doc_registry` and `clay_js_api_inventory` guards; Rust visibility mapping is deterministic.
    - Security: `server-register-command.md` specifies no self-grant path, `UnauthorizedTarget` rejection, sanitized diagnostics, bounded stale grace, rollback, and no authority expansion.
  - Approach:
    - Added `## Phase 19 built-in reload command boundary` to `docs/reference/clay-js-api/commands/server-register-command.md` documenting the command's boundary table (no JS facade, bindable, Control Center-discoverable, rejected from `serverExecuteCommand`), execution flow, diagnostics table, and authority notes.
    - Added `## Phase 19 built-in command discovery note` to `docs/reference/clay-js-api/commands/server-list-commands.md` clarifying that built-in commands are not listed by `serverListCommands`.
    - Ran `cargo run --bin update-doc-registry` to regenerate `docs/generated/clay-js-api-registry.json`.
    - Verified all reload lifecycle Rust helpers are properly internal: `reload_runtime_generation`/`execute_reload_command` are `pub(crate)`, `commit_runtime_generation`/`prepare_runtime_generation_candidate` are private, `trigger_developer_hot_reload`/`RuntimeReloadOutcome`/`ReloadedDocumentRefresh` are `#[doc(hidden)]`.
  - Test Cases:
    - `phase19_reload_command_is_discoverable_and_documented` — passed (`tests/clay_js_api_inventory.rs`).
    - `phase19_reload_helpers_are_internal_or_have_complete_clay_js_api_mapping` — passed (`tests/rust_visibility_api_mapping.rs`).
    - `cargo test --test clay_js_doc_registry` — 34/34 pass, registry is current.
    - `reload_command_is_server_first_behavior_locked_and_discoverable` — unchanged, still passes (`tests/command_execution.rs`).

- [x] Verify hot reload performance, security, rollback, protocol, and end-to-end behavior
  - Acceptance Criteria:
    - Functional: Automated coverage exercises valid and invalid configuration/package changes, multiple connected clients, open documents in multiple first-party modes, package UI/SDUI, syntax/decorations/diagnostics, completion/intelligence, language-server workers, command trigger, stale edits, rollback, reconnect/bootstrap, and fallback after package removal.
    - Performance: Instrument and assert that reload evaluation/commit/reparse does not block ordinary local typing/rendering or edit acknowledgements, snapshots remain below the 1 MiB hard ceiling and report 768 KiB payload-p95/16 ms install-p95 review thresholds, queues remain bounded, and old tasks terminate within existing deadlines.
    - Code Quality: Linux blocking gates pass: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`; focused benchmark targets compile/run as documented.
    - Security: Tests cover deny-by-default imports/authorities, grant preservation without expansion, executable callback rejection, invalid/spoofed protocol messages, lock release, sanitized diagnostics, old worker/session cleanup, and no raw source/path/token leakage.
  - Outcomes:
    - Added test-only `ReloadCandidateBarrier` on `IpcServer` so candidate evaluation can park after the reload attempt lock without holding the Behavior commit lock; ordinary edits continue and ack while a candidate is blocked.
    - Added the four named duplex/security tests in `server::runtime_generation_tests`:
      - `typing_and_edit_ack_continue_while_candidate_runtime_is_blocked_on_test_barrier`
      - `failed_reload_broadcasts_diagnostic_but_no_generation_snapshot`
      - `successful_reload_is_observed_as_one_generation_by_all_clients`
      - `reload_preserves_authority_denials_and_cleans_old_lsp_worker`
    - Extended `tests/persistent_runtime_hot_reload.rs`, `tests/runtime_update_protocol.rs`, and `tests/performance_protocol.rs` with public-API rollback coverage, diff-review payload reporting, and Phase 19 budget locks.
    - Documented Phase 19 hard/advisory budgets and focused commands in `docs/development/performance.md` and `docs/wiki/modules/maintenance-validation.md`.
    - Linux gates passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (exit 0; benches compile/run under Criterion).
  - Test Cases Written:
    - `typing_and_edit_ack_continue_while_candidate_runtime_is_blocked_on_test_barrier`
    - `failed_reload_broadcasts_diagnostic_but_no_generation_snapshot`
    - `successful_reload_is_observed_as_one_generation_by_all_clients`
    - `reload_preserves_authority_denials_and_cleans_old_lsp_worker`
    - `failed_reload_keeps_generation_and_sanitized_diagnostic_without_advancing`
    - `phase19_runtime_state_snapshot_and_grace_budgets_are_locked`
    - `runtime_snapshot_payload_reports_diff_review_threshold_under_hard_ceiling`

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Outcomes:
    - `persistent-runtime-hot-reload.md` Phase 19 boundary updated to reflect Plan 054 complete status with all 12 task implementations; source paths expanded (`src/masonry_sdui.rs`, `src/shell/package_ui.rs`, `src/editor/{surface,typography}.rs`, `src/perf/budgets.rs`); test commands expanded with duplex-barrier, grace unit, budget lock, and diff-review payload tests.
    - `index.md` master entry updated to list all Plan 054 primitives in the description.
    - Six cross-referenced pages received Phase 19 links in their Related sections:
      - `behavior-manifests.md` — `BehaviorGraceState` stale-edit grace and `InvalidBehaviorVersion` resync.
      - `server-ipc-skeleton.md` — `RuntimeStateSnapshot` broadcast fan-out through existing connection tasks.
      - `decoration-transport.md` — `DocumentRuntimeRenderState` reset flags during atomic client install.
      - `slot-aware-package-ui.md` — `PackageUiRuntimeState::install_runtime_snapshot`.
      - `completion-snippet-expansion.md` — `cancel_older_generations` for stale provider cleanup.
      - `language-server-process-service.md` — `shutdown_all`/`shutdown_generation_resources` for previous-generation LSP teardown.
    - `maintenance-validation.md` already updated during task 12 with Phase 19 end-to-end verification commands and Linux gate results.
  - Test Cases Written:
    - All existing tests pass; no wiki-specific doc-coverage test exist for Phase 19 modules.

## Compromises Made

- Package UI contribution payloads in `RuntimeStateSnapshot` remain empty and versioned only. Package UI is not published over IPC at bootstrap today; the version advances with the runtime generation so clients can clear previous package UI under one install boundary. Expand wire contribution payloads when package UI publication ships.
- `ServerMessage::RuntimeStateSnapshot` and `ClientConnectionEvent::RuntimeStateSnapshot` box the snapshot to keep enum variants compact under clippy `large_enum_variant`.
- Diff-upgrade remains deferred: complete snapshots stay the live path until measured payload reaches 768 KiB p95 or client install exceeds 16 ms p95.

## Further Actions

- None. Plan 054 is complete. Future diff-upgrade and package-UI wire transport are deferred to later phases governed by compiled budget thresholds (768 KiB p95 / 16 ms p95).
- When measured snapshot payload reaches 768 KiB p95 or install exceeds 16 ms p95, revisit diffs/chunking through a separate protocol decision.
