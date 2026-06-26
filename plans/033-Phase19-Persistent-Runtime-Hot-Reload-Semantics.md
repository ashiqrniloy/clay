# Phase 19 Persistent Runtime Hot Reload Semantics

## Objectives

- Define generic hot reload semantics for Clay's persistent server-side JavaScript runtime.
- Replace package/runtime state by generation instead of mutating stale handlers in place.
- Keep ordinary typing, rendering, edit acknowledgement, and local paint independent from JavaScript reload work.
- Preserve Phase 18.7 first-party-only, deny-by-default package authority until a separate third-party authority review.

## Expected Outcome

- A hot reload request creates a new runtime generation, reloads configured packages, swaps active package/mode/parse/behavior state atomically, and invalidates stale parse handlers/tasks.
- Open documents are refreshed through generic mode classification/activation and bounded parse scheduling with no Markdown-specific branches.
- Failed reloads publish sanitized diagnostics and keep the previous generation active.
- Documentation, Clay JS API/configuration review, and wiki pages explain reload lifecycle, authority boundaries, tests, and limitations.

## Tasks

- [x] Review existing runtime/package/mode primitives and define generic hot reload gaps
  - Acceptance Criteria:
    - Functional: The primitive review inventories existing persistent runtime, `loadPackage`, `clay:modes`, `clay:parse`, `ParseCoordinator`, behavior manifest, workspace/document, and selected-file activation primitives before proposing implementation work.
    - Performance: The review identifies reload work as background/server-first work and confirms no JavaScript is added to keypress, paint, layout, scroll, or edit-ack hot paths.
    - Code Quality: Proposed gaps are generic lifecycle primitives, not Markdown-specific reload branches or parser calls.
    - Security: The review preserves resolver-validated first-party packages, deny-by-default module loading, parse permission checks, executable callback rejection, and sanitized diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `docs/wiki/modules/embedded-js-runtime.md`
      - `docs/wiki/modules/package-loading.md`
      - `docs/wiki/modules/parse-coordinator.md`
      - `docs/wiki/modules/server-ipc-skeleton.md`
      - `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`
    - Options Considered:
      - Mutate the existing runtime's global registries in place. Rejected; stale closures/tokens and module cache entries are hard to prove invalid.
      - Recreate per-open runtimes. Rejected; violates Phase 18.7 and reintroduces open-time V8/disk churn.
      - Replace the whole runtime generation and re-register packages/modes/handlers. Chosen; simplest invariant: old generation drains/cancels, new generation owns new closures.
    - Chosen Approach:
      - Write a primitive review page that names existing primitives, gaps, invariants, rejected alternatives, and the minimal generation-swap model before code changes.
    - API Notes and Examples:
      ```text
      loadPackage('@clay/markdown')  // idempotent inside one generation; reload creates a new generation
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase19-persistent-runtime-hot-reload-primitive-review.md`: primitive review and rejected alternatives.
      - `docs/wiki/index.md`: link primitive review page.
      - `tests/package_loading_docs.rs`: documentation coverage test for generation swap, stale handler invalidation, and hot-path/security boundaries.
    - References:
      - Phase 18.7 authority decision log; `mode-primitive-first.md`; `embedded-js-runtime.md`.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps`: Documentation coverage test requiring the primitive review page and key phrases for generation swap, stale handler invalidation, and no hot-path JavaScript.

- [x] Introduce runtime generation ownership and atomic reload swap
  - Acceptance Criteria:
    - Functional: Server state tracks an active runtime generation ID; reload constructs a fresh `ClayJsRuntimeService`, evaluates configuration/package load entries, and swaps it into service state only after validation succeeds.
    - Performance: Runtime construction/evaluation happens off the typing/rendering path and uses existing timeout guards.
    - Code Quality: Generation state is explicit and owned by the server/runtime service boundary; no global mutable singleton or per-document runtime is added.
    - Security: Failed reload leaves the prior validated generation active and publishes only sanitized diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/embedded-js-runtime.md`
      - `src/server/js_runtime.rs`
      - `src/server/mod.rs`
    - Options Considered:
      - In-place `globalThis` registry clearing. Rejected; cached module namespace objects may still point at stale code.
      - Process restart. Rejected; too heavy and loses open-document/lease state.
      - Fresh runtime generation swap. Chosen; smallest reliable boundary for JS closures and module cache.
    - Chosen Approach:
      - Add a small server-owned runtime generation holder wrapping `ClayJsRuntimeService`, diagnostics, and generation ID. Build next generation separately, then atomically swap Arc/state after configuration and package loads pass.
    - API Notes and Examples:
      ```rust
      let next = RuntimeGeneration::load_from_config(root).await?;
      runtime_state.swap_if_valid(next).await;
      ```
    - Files to Create/Edit:
      - `src/server/mod.rs`: `RuntimeGenerationStore`, `RuntimeGeneration`, reload outcome, default load path through active generation, and reload entrypoint.
      - `src/server/connection.rs`: use active generation snapshot for open-time selected-file activation.
      - `docs/wiki/modules/embedded-js-runtime.md`: document generation ownership/reload semantics and verification.
      - `tests/package_loading_docs.rs`: documentation coverage for runtime generation implementation wiki markers.
    - References:
      - `ClayJsRuntimeService`; `IpcServer::load_default_configuration`; `apply_runtime_outputs`.
  - Test Cases to Write:
    - `cargo test runtime_generation --lib`: Successful reload increments generation ID and swaps to fresh runtime state; failed reload keeps previous generation active and emits sanitized diagnostic.
    - `cargo test --lib server::connection::tests::server_sends_runtime_diagnostics_after_bootstrap`: Existing connections still receive stored runtime diagnostics after signature change.
    - `cargo test --lib selected_markdown_file_publishes_manifest_and_decorations`: Selected-file activation still uses runtime-backed generic mode/parse flow.
    - `cargo test --test package_loading_docs phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps`: Docs coverage pins `RuntimeGenerationStore`/reload implementation notes.

- [x] Define loaded-package cache invalidation and default `init.js` reload behavior
  - Acceptance Criteria:
    - Functional: `globalThis.__clayLoadedPackages` remains idempotent within one generation but is empty in the next generation, so `~/.config/clay/init.js` one-line loads re-run cleanly during reload.
    - Performance: Reload does not scan or import packages on ordinary edits; package reload is explicit reload work.
    - Code Quality: Package loading reuses `loadPackage`, `FirstPartyLoadEntryAllowlist`, and `PackageService` validation instead of adding package-specific reload paths.
    - Security: First-party `@clay/*` allowlist boundaries remain unchanged; non-`@clay/*` packages remain out of scope.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/package-loading.md`
      - `runtime/js/packages.ts`
      - `src/server/ops/packages.rs`
      - `packages/markdown/dist/load.js`
    - Options Considered:
      - Add a `force` flag to `loadPackage`. Rejected for Phase 19 initial implementation; generation replacement gives clean reload without public mutation semantics.
      - Clear JS globals manually. Rejected; incomplete for module cache.
      - Re-run user `init.js` in a fresh generation. Chosen; matches existing explicit-load contract.
    - Chosen Approach:
      - Treat hot reload as configuration generation replacement: rerun configured `init.js`, rebuild load-entry allowlist, rerun package load entries, and collect package summaries/diagnostics.
    - API Notes and Examples:
      ```javascript
      import { loadPackage } from 'clay:packages';
      await loadPackage('@clay/markdown');
      ```
    - Files to Create/Edit:
      - `src/server/mod.rs`: reload tests proving `init.js` package load reruns in a fresh generation and failed package reload keeps prior generation active.
      - `src/server/js_runtime.rs`: embedded `clay:packages` cache-lifetime comment.
      - `runtime/js/packages.ts`: facade cache-lifetime and hot-reload invalidation comments.
      - `docs/reference/primitives/package-loading.md`: runtime-generation package cache invalidation semantics.
      - `docs/reference/packages/creating-packages.md`: package author/user docs for `init.js` reload behavior.
      - `docs/wiki/modules/package-loading.md`: implementation wiki for generation-scoped `loadPackage` state.
      - `tests/package_loading_docs.rs`: documentation coverage for package cache invalidation markers.
    - References:
      - Phase 18.6/18.7 package loading docs and tests.
  - Test Cases to Write:
    - `cargo test --lib load_package_is_idempotent_per_persistent_runtime`: `loadPackage('@clay/markdown')` registers once inside one persistent runtime generation.
    - `cargo test runtime_generation --lib`: successful reload reruns `init.js` in a fresh generation and failed invalid package reload leaves prior generation/package cache active.
    - `cargo test --test package_loading_docs phase19_load_package_cache_docs_pin_generation_invalidation`: docs/facade comments pin generation-scoped package cache semantics.

- [x] Replace parse-handler registrations by generation and cancel stale parse work
  - Acceptance Criteria:
    - Functional: Parse handlers registered by old runtime generations are unregistered or ignored after reload; stale parse results cannot publish after generation swap.
    - Performance: Parse cancellation remains background/cancellable; ordinary edit acknowledgement does not wait for reload parsing.
    - Code Quality: `ParseCoordinator` owns handler generation metadata and stale-result checks generically for all modes.
    - Security: Handler timeout, budget validation, executable callback rejection, and failed-task instrumentation remain enforced.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/parse-coordinator.md`
      - `docs/wiki/modules/parse-task-lifecycle.md`
      - `src/server/parse_coordinator.rs`
      - `src/server/js_runtime.rs`
    - Options Considered:
      - Keep old handlers until process exit. Rejected; stale package code could keep parsing after reload.
      - Clear all parse state globally without generation checks. Rejected; racing tasks could still publish late.
      - Generation-tag handlers and parse tasks. Chosen; small explicit stale-result guard.
    - Chosen Approach:
      - Extend parse handler metadata and scheduled tasks with runtime generation IDs, unregister/replace handlers on successful reload, cancel outstanding tasks for old generations, and reject late results during validation.
    - API Notes and Examples:
      ```rust
      parse_coordinator.replace_handlers_for_generation(generation_id, handlers);
      parse_coordinator.cancel_generation(old_generation_id);
      ```
    - Files to Create/Edit:
      - `src/server/parse_coordinator.rs`: generation-owned handler registration, replacement, old-generation task cancellation, and late-result generation guard.
      - `src/server/js_runtime.rs`: pass runtime generation IDs when adapting JS parse-handler tokens into `ParseCoordinator` handlers.
      - `src/server/connection.rs`: selected-file open registers parse handlers under the current runtime generation snapshot.
      - `src/server/mod.rs`: startup/reload applies runtime evaluations with generation IDs and cancels prior-generation parse work on successful swap.
      - `tests/parse_coordinator.rs`: focused generation replacement/cancellation/failure instrumentation coverage.
      - `docs/wiki/modules/parse-coordinator.md`: implementation notes for generation-scoped handlers/tasks.
      - `docs/wiki/modules/parse-task-lifecycle.md`: lifecycle notes for generation cancellation and stale-result rejection.
      - `docs/wiki/modules/embedded-js-runtime.md`: runtime bridge notes for generation-owned JS parse tokens.
      - `tests/package_loading_docs.rs`: docs-as-code coverage for generation parse replacement wording.
    - References:
      - Phase 18.7 JS-backed handler bridge and parse failure instrumentation.
  - Test Cases to Write:
    - `cargo test --test parse_coordinator generation_replacement_uses_new_handler_for_subsequent_parse`: replacing a handler for a newer generation uses only the new handler on subsequent parses.
    - `cargo test --test parse_coordinator replacing_generation_cancels_old_in_flight_parse_work`: in-flight old-generation parse work is cancelled and cannot publish after replacement.
    - `cargo test --test parse_coordinator handler_failures_are_instrumented_after_generation_replacement`: handler failures still increment failed-task stats after generation replacement.
    - `cargo test --test package_loading_docs phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps`: docs cover generation-scoped parse replacement.
    - `cargo test --lib`: runtime/connection generation integration still passes.

- [x] Refresh open documents after successful reload through generic mode activation
  - Acceptance Criteria:
    - Functional: On successful reload, each open workspace document is reclassified/re-activated through `clay:modes`, receives the correct behavior manifest, and schedules bounded parse refresh where a handler exists.
    - Performance: Refresh is bounded per document/viewport and queued as background work; no full-document IPC for ordinary edits.
    - Code Quality: Refresh uses existing `DocumentClassification`, `MajorModeActivation`, `ParseCoordinator`, and behavior manifest primitives; no `if markdown` branch.
    - Security: Refresh only touches already-open server-owned documents and preserves workspace/file authority and selected-file capability boundaries.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/server-ipc-skeleton.md`
      - `docs/wiki/modules/server-file-workspace.md`
      - `docs/wiki/modules/embedded-js-runtime.md`
    - Options Considered:
      - Refresh only future opens. Rejected; open documents would show stale behavior after reload.
      - Push full document snapshots to clients. Rejected; unnecessary and violates performance direction.
      - Re-run generic activation for open documents and publish behavior/decorations only. Chosen.
    - Chosen Approach:
      - Add a server reload follow-up that iterates open document metadata/text through existing generic classification/activation and schedules viewport-bounded parse; publish behavior/decorations/diagnostics over connected clients using existing protocol messages.
    - API Notes and Examples:
      ```text
      reload -> classify open docs -> publish BehaviorManifest -> schedule bounded parse -> publish DecorationSet
      ```
    - Files to Create/Edit:
      - `src/server/workspace.rs`: added `open_document_snapshots` for reload-only enumeration of server-owned open documents/text.
      - `src/server/connection.rs`: exposed the existing selected-file follow-up primitive for reload refresh reuse.
      - `src/server/mod.rs`: `reload_runtime_generation` now calls `refresh_open_documents_after_reload` after successful swap and returns per-document follow-up messages.
      - `docs/wiki/modules/embedded-js-runtime.md`: documents successful reload open-document refresh.
      - `docs/wiki/modules/server-ipc-skeleton.md`: documents refresh messages and full-text snapshot exclusion.
      - `docs/wiki/modules/server-file-workspace.md`: documents reload-only open document snapshot enumeration.
      - `tests/package_loading_docs.rs`: docs-as-code coverage for reload refresh wording.
    - References:
      - Phase 18.7 selected-file generic activation flow.
  - Test Cases to Write:
    - `cargo test successful_reload_refreshes_open_documents_without_full_snapshots --lib`: open Markdown document receives behavior/decorations after reload, non-matching text file receives no parse decorations, and refresh emits no `DocumentOpened`/`DocumentReloaded` full-text snapshots.
    - `cargo test --test package_loading_docs phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps`: docs cover generic reload open-document refresh.
    - `cargo test --lib`: server/runtime/connection regression coverage.

- [x] Add a deterministic non-GUI reload trigger for tests/developer workflow
  - Acceptance Criteria:
    - Functional: A server-side test or developer command can trigger hot reload without GUI interaction and observe success/failure diagnostics.
    - Performance: Trigger is explicit and does not run during ordinary client event processing unless requested.
    - Code Quality: The trigger is thin and calls the shared reload primitive; no duplicate reload logic in CLI/test code.
    - Security: Trigger does not grant package-manager, filesystem, network, shell, or third-party package authority beyond existing configuration-root evaluation.
  - Approach:
    - Documentation Reviewed:
      - `src/main.rs` smoke command patterns.
      - `src/server/mod.rs` server lifecycle.
      - `docs/wiki/modules/maintenance-validation.md`
    - Options Considered:
      - GUI-only reload command. Rejected; hard to test in headless agents.
      - Public JS API first. Deferred until semantics are proven through internal reload primitive.
      - Non-GUI test helper/IPC command using shared primitive. Chosen.
    - Chosen Approach:
      - Provide the smallest deterministic trigger needed by tests, either an internal server method or protocol/dev command, then decide public user-facing command/API in the Clay JS API review task.
    - API Notes and Examples:
      ```bash
      cargo test --test persistent_runtime_hot_reload
      ```
    - Files to Create/Edit:
      - `tests/persistent_runtime_hot_reload.rs`: added headless trigger test for successful reload and sanitized failure retention.
      - `src/server/mod.rs`: added public `IpcServer::trigger_developer_hot_reload` thin wrapper around shared reload primitive and returned diagnostics in `RuntimeReloadOutcome`.
      - `docs/wiki/modules/embedded-js-runtime.md`: documented deterministic non-GUI reload trigger and test coverage.
      - `docs/wiki/modules/maintenance-validation.md`: documented headless hot reload validation command.
      - `tests/package_loading_docs.rs`: docs-as-code coverage for non-GUI trigger wording.
    - References:
      - `tests/selected_file_markdown_smoke.rs`; server smoke patterns.
  - Test Cases to Write:
    - `cargo test --test persistent_runtime_hot_reload`: headless reload succeeds through `trigger_developer_hot_reload`, then failure returns sanitized diagnostic and keeps prior generation active.
    - `cargo test --test package_loading_docs phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps`: docs cover the non-GUI trigger and shared reload primitive.
    - `cargo test --lib`: server/runtime regression coverage, including behavior refresh from the previous task.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Any user-visible reload setting or command is represented as a documented Clay JS API/configuration surface, or explicitly documented as internal/developer-only for Phase 19.
    - Performance: Configuration reload options do not add runtime work to hot paths.
    - Code Quality: No hidden JSON/TOML/ad hoc config key is introduced; `~/.config/clay/init.js` remains the entry point.
    - Security: Configuration does not implicitly grant filesystem, network, shell, AI, WASM, package-manager, client-JS, or third-party package authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay Configuration Task.
      - `docs/reference/clay-js-api/configuration.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
    - Options Considered:
      - Add hidden `hotReload=true` config. Rejected.
      - Expose a public reload setting before lifecycle is proven. Rejected unless implementation needs it.
      - Verify no new public configuration API is needed for internal reload semantics. Likely chosen unless user-facing reload controls are added.
    - Chosen Approach:
      - Inventory implementation changes; add docs/registry/tests only for real public behavior-changing configuration.
    - API Notes and Examples:
      ```javascript
      // No hidden key. Public reload controls, if added, must be Clay JS APIs.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: added Phase 19 persistent-runtime hot reload configuration review documenting no public reload setting/API, no hidden reload keys, and developer-trigger-only semantics.
      - `docs/reference/clay-js-api/api-inventory.toml`: unchanged; no public configuration API was added.
      - `tests/clay_js_api_inventory.rs`: added guard that rejects hidden hot reload configuration keys and `clay:configuration` reload APIs.
    - References:
      - Clay configuration project plan requirements.
  - Test Cases to Write:
    - `cargo test --test clay_js_api_inventory phase19_hot_reload_configuration_review_rejects_hidden_reload_keys`: inventory/docs assert no undocumented reload configuration keys or `clay:configuration` reload APIs exist.
    - `cargo test --test package_loading_docs phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps`: Phase 19 docs guard remains green.
    - `cargo test --lib`: server/runtime regression coverage.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Public reload commands/APIs are documented with stable IDs, user-facing names, key bindings/custom properties, examples, errors, permissions, backing Rust paths, ops/facades, and lookup tags; internal helpers remain private or `pub(crate)`.
    - Performance: Public APIs document that reload is background/server-first and not a typing/rendering hot-path operation.
    - Code Quality: Raw `Deno.core.ops` names are not the user-facing API; generated registry is fresh.
    - Security: API docs state first-party-only package reload, deny-by-default module loading, sanitized diagnostics, and no expanded filesystem/network/shell/package-manager authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `docs/index.md`
    - Options Considered:
      - Make every Rust reload helper public. Rejected; public Rust does not equal public Clay JS API.
      - Internal-only Phase 19 reload primitive. Acceptable if no user-facing command ships.
      - Public `clay.runtime.reloadPackages`/similar API. Only if implementation intentionally exposes user reload.
    - Chosen Approach:
      - After implementation, inventory Rust visibility and add/update docs only for real public programmatic surfaces.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: clarified that `trigger_developer_hot_reload` and reload outcome types are `#[doc(hidden)]` test/developer surfaces, not Clay JS APIs.
      - `src/server/mod.rs`: marked `ReloadedDocumentRefresh`, `RuntimeReloadOutcome`, and `trigger_developer_hot_reload` as `#[doc(hidden)]`; kept shared reload primitive `pub(crate)`.
      - `docs/reference/clay-js-api/api-inventory.toml`: unchanged; no public runtime reload API was added.
      - `docs/generated/clay-js-api-registry.json`: unchanged; no registry entry was needed.
      - `tests/clay_js_api_inventory.rs`: added Rust visibility/API-surface guard for Phase 19 hot reload.
      - `tests/clay_js_doc_registry.rs`: verified unchanged registry stays fresh.
    - References:
      - Clay JS API project plan requirements.
  - Test Cases to Write:
    - `cargo test --test clay_js_api_inventory phase19_hot_reload_has_no_public_clay_js_api_surface`: Rust visibility mapping verifies reload helpers are doc-hidden, shared primitive is `pub(crate)`, and no Clay JS facade/registry reload API exists.
    - `cargo test --test clay_js_api_inventory`: full inventory coverage passed.
    - `cargo test --test clay_js_doc_registry`: generated registry coverage passed.
    - `cargo test --test persistent_runtime_hot_reload`: developer-only Rust trigger still works.
    - `cargo test --lib`: server/runtime regression coverage.

- [x] Verify performance, security, and regression behavior
  - Acceptance Criteria:
    - Functional: Hot reload tests cover success, failure rollback, package cache reset, handler invalidation, stale parse cancellation, open-document refresh, and diagnostics.
    - Performance: Existing protocol/edit hot-path tests still pass; reload benchmark or timing smoke shows bounded reload work with no edit-path dependency.
    - Code Quality: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` pass.
    - Security: Boundary tests confirm denied platform authorities, first-party-only module loading, executable callback rejection, timeout recovery, and sanitized diagnostics after reload.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/maintenance-validation.md`
      - `docs/development/performance.md`
      - Phase 18.7 security tests in `src/server/js_runtime.rs` and `tests/parse_coordinator.rs`.
    - Options Considered:
      - Only unit tests. Rejected; lifecycle swap needs integration coverage.
      - GUI smoke. Optional, but non-GUI deterministic coverage is required.
      - Full all-target gate. Chosen as final verification.
    - Chosen Approach:
      - Add focused tests for reload semantics, then run full repository gates.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo test --test persistent_runtime_hot_reload
      cargo test --test performance_protocol
      ```
    - Files to Create/Edit:
      - `tests/persistent_runtime_hot_reload.rs`: added authority-denial-after-success coverage and verified success/failure rollback diagnostics.
      - `tests/rust_visibility_api_mapping.rs`: allowlisted doc-hidden/internal reload and generation helpers as non-JS server infrastructure.
      - `docs/wiki/modules/maintenance-validation.md`: updated current gate result and noted hot reload/performance coverage.
      - Existing runtime/parse/connection tests reused for package cache reset, stale parse cancellation, open-document refresh, diagnostics, and security boundaries.
    - References:
      - `maintenance-validation.md`; `performance.md`.
  - Test Cases to Write:
    - `cargo test --test persistent_runtime_hot_reload`: reload success/failure rollback, sanitized diagnostics, and runtime authority denial after reload.
    - `cargo test --test performance_protocol`: protocol/edit hot-path budgets remain green.
    - `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets`: full regression gate passed (786 tests across 22 suites).
    - Existing `cargo test --lib` and `tests/parse_coordinator.rs` coverage verifies package cache reset, handler replacement, stale parse cancellation, open-document refresh, and diagnostics.

- [x] Update package authoring and runtime documentation
  - Acceptance Criteria:
    - Functional: Package authors understand idempotent-per-generation `loadPackage`, what reload does, what state must be rebuilt, and how parse handlers/behavior are replaced.
    - Performance: Docs state reload is not hot-path work and parse refresh is bounded/background.
    - Code Quality: Docs forbid per-open runtimes, Markdown-only reload branches, raw ops, executable callback payloads, and fake init.js decoration publication.
    - Security: Docs state first-party-only authority, package permission checks, deny-by-default module loading, and sanitized reload diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md`
      - `docs/reference/packages/markdown.md`
      - `docs/wiki/modules/package-loading.md`
      - `docs/wiki/modules/embedded-js-runtime.md`
    - Options Considered:
      - Document only public API pages. Rejected; implementation wiki and package author guide both need lifecycle semantics.
      - Leave docs until after API task. Rejected; docs-as-code tests should enforce lifecycle wording.
    - Chosen Approach:
      - Update author-facing package docs plus implementation wiki pages and add docs-as-code assertions where practical.
    - API Notes and Examples:
      ```javascript
      await loadPackage('@clay/markdown'); // reload reruns this in a fresh generation
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`: documented runtime-generation package author contract, `loadEntry` rebuild expectations, parse handler replacement/cancellation, and first-party allowlist preservation.
      - `docs/reference/packages/markdown.md`: documented Markdown reload generation behavior, no Markdown-specific Rust branch, bounded/background parse refresh, and stale parse-result rejection.
      - `docs/wiki/modules/embedded-js-runtime.md`: documented package author reload lifecycle for fresh generations.
      - `docs/wiki/modules/package-loading.md`: documented generation-local package state, handler invalidation, hot-path exclusion, and security boundary preservation.
      - `tests/package_loading_docs.rs`: added docs-as-code assertions for Phase 19 package author/runtime lifecycle wording.
    - References:
      - Phase 18.7 package author guide contract.
  - Test Cases to Write:
    - `cargo test --test package_loading_docs phase19_package_author_docs_cover_reload_runtime_lifecycle`: docs require reload generation, handler invalidation, no hot-path JS, sanitized diagnostics, executable callback rejection, and first-party-only boundary phrases.
    - `cargo test --test package_loading_docs`: full package-loading docs suite passed.

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
      - `docs/wiki/index.md`: Added navigation link for the Phase 19 persistent runtime hot reload implementation page.
      - `docs/wiki/modules/persistent-runtime-hot-reload.md`: Added final implementation wiki covering generation ownership, atomic reload swap, package cache invalidation, parse-handler generation replacement, open-document refresh, non-GUI trigger, invariants, security/performance boundaries, and tests.
      - `tests/package_loading_docs.rs`: Extended docs-as-code coverage to require the implementation wiki link and key hot reload implementation phrases.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - `cargo test --test package_loading_docs phase19_hot_reload_primitive_review_is_linked_and_pins_generic_gaps`: verifies the master index links the implementation wiki and the page documents generation ownership, atomic swap, package cache invalidation, parse-handler replacement, open-document refresh, hot-path exclusion, public API boundary, diagnostics, and tests.
    - Manual wiki review completed: master index links relevant pages and implementation wiki explains what changed code does and how it works.

## Compromises Made

- Hot reload remains an internal/developer server primitive, not a public Clay JS API or user command. This keeps authority and UX scope narrow for Phase 19.
- Reload support is first-party-only through resolver-validated `@clay/*` package load entries. Arbitrary npm/package-manager reload remains deferred to the package distribution phase.
- Open-document reload refresh publishes behavior/decorations/diagnostics only and intentionally avoids full-document snapshots for unchanged documents.

## Further Actions

- Phase 23/package distribution: revisit non-`@clay/*` package resolution and persistent shared package enable state.
- Future UX phase: decide whether to expose a user-facing reload command/keybinding after security, diagnostics, and workflow requirements are defined.
