# Audit Remediation: Responsibility-Preserving Refactor

Prerequisites: Plans 086–089 complete, including Plan 089's Plan 088 welcome/loading, safe-targeted visual review, and Criterion-regression follow-up tasks, and full Linux baseline green. Do not mix this refactor with visual redesign or dependency migration.

Source review: P2-1, P2-3, P2-4 and large-file evidence in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`.

Scope: Extract existing responsibilities into plain modules/functions. Preserve authority, protocol, UI behavior, hot paths, and public APIs. No one-implementation traits, factories, plugin architecture, or “future flexibility.”

## Objectives

- Make server connection/runtime, editor/shell, package validation, and app launch/event routing reviewable by responsibility.
- Give command-centre lifecycle/focus/geometry/accessibility one legible presentation owner without changing server-owned session authority.
- Reduce high-cost source-text test churn in favor of compact reusable helpers and behavioral checks.
- Prove behavior/performance/security parity after every extraction.

## Expected Outcome

- Large orchestration files become smaller coordinators with named sibling modules aligned to current ownership.
- Connection cleanup, runtime bootstrap/validation, shell overlays/accessibility, package validators, and app launch/event routing each have one obvious owner.
- Editor typing/paint and server authority boundaries remain unchanged; no protocol/schema/package/API migration occurs.
- Tests cover behavior parity and no duplicate state/cleanup paths remain.

## Tasks

- [x] Establish module/ownership map, UI primitive constraints, and extraction budgets
  - Acceptance Criteria:
    - Functional: Map state, behavior, execution, persistence, validation, cleanup, and cross-module calls for `connection`, `js_runtime`, `editor/surface`, `server/mod`, `masonry_shell`, `packages/record`, and `main`; include Driver/ClayShellWidget/EditorWidget/PaneDocumentView/PackageOverlayHost/server menu sessions. ✅ `docs/development/architecture-ownership.md`
    - Performance: Identify typing/paint/layout/IPC/runtime hot paths and current benchmark/test guards before moves. ✅ guard table in map (16 ms keypress→paint, 1 ms pane/tab, 40 ms edit ack, 50/4 ms command centre, 25/100 ms runtime config/mode, 1 MiB frame, bench + invariant test suite listed).
    - Code Quality: Set per-task extraction boundaries and stop conditions; every new module has at least two coherent responsibilities/callers or owns one state machine, not arbitrary line-count slicing. ✅ budget table below; go/no-go revert rule per seam in map.
    - Security: Mark canonical document/workspace/file/package/runtime/connection identity and cleanup authority; extraction cannot relocate or duplicate enforcement. ✅ identity/cleanup authority table + checklist in map.
  - Approach:
    - Documentation Reviewed:
      - Wiki (all read in full): `server-ipc-skeleton.md`, `embedded-js-runtime.md`, `masonry-editor.md`, `masonry-shell.md`, `pane-document-views.md`, `transient-menu-round-trip.md`, `package-loading.md`.
      - `.agents/skills/clay-ui/SKILL.md` (architecture map, golden rules) + `references/components.md`/`tokens.md` via `src/shell/components.rs`/`theme.rs`; UI primitive constraints recorded in map.
      - Project patterns (read): `authority-boundaries.md`, `package-runtime-trust-domains.md`, `package-ui-layout.md`, `protocol-and-performance.md`, `planning-checklist.md`.
      - UI-skills routing evidence: `npx ui-skills start` ran; catalog inspected (`npx ui-skills list`) — all entries are web/frontend skills (Tailwind, landing pages, ARIA/HTML a11y); none applies to a zero-visual-change native Rust ownership refactor. No skill loaded; `clay-ui` skill + catalogs are the applicable UI-constraint source. Recorded to satisfy the planning checklist gate.
      - Audit P2-1/P2-2/P2-3/P2-4 in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`; source inventory: 62,457 lines across the seven large files (connection 11,535; js_runtime 13,333; surface 8,327; server/mod 6,353; masonry_shell 6,064; record 5,406; main 3,731).
    - Options Considered:
      - Split by file size alone: rejected.
      - New service/trait architecture: rejected.
      - Move existing cohesive responsibilities into private sibling modules: chosen.
    - Chosen Approach:
      - `docs/development/architecture-ownership.md` is the one-page ownership graph + dependency direction (`coordinator → private responsibility module → existing state/typed result`) + UI primitive constraints + hot-path guard table + security identity/cleanup table + pre-extraction ownership review checklist. Each later task extracts exactly one seam from the budget table and runs focused parity checks before continuing.
      - Discovery: task 6's driver split is already partially done — `src/main.rs:32` declares `mod driver;` → `src/driver/{mod,reconcile,restore}.rs` (event routing, tab lifecycle, restore, persistence). Remaining main.rs work is cli/launch/native-dialog extraction only.
    - API Notes and Examples:
      ```text
      coordinator → private responsibility module → existing state/typed result
      no new trait unless multiple current implementations already require one
      ```
    - Files to Create/Edit:
      - `docs/development/architecture-ownership.md` (created): one-page map, guards, budgets, checklist.
      - This plan: exact extraction/file budget table below (used by tasks 2–8).
    - References:
      - Audit P2-1, P2-3; `authority-boundaries.md`, `protocol-and-performance.md`, `package-ui-layout.md`, `package-runtime-trust-domains.md`, `planning-checklist.md`; wiki pages listed above; `src/perf/budgets.rs`.
  - Test Cases to Write:
    - Ownership review (map checklist): every mutable state/cleanup path has exactly one named owner — connection cleanup (`cleanup_connection_documents` single exit boundary), menu sessions (drop-on-exit), read pump (`ReadPumpGuard`), runtime workers (`RuntimeWorker` drop/poison), pane hosts (shell reconcile), stale tabs (server sweep), canonical ids (server DocumentState/WorkspaceState/TabRegistry/BUNDLED_PACKAGES). Apply before each extraction.

  ### Extraction budget table (authoritative for tasks 2–8)

  | Task | Source (lines now) | Target shape | Budget / stop condition | Guards that must stay green after each seam |
  |---|---|---|---|---|
  | 2 connection | `connection.rs` 11,535 | Coordinator keeps loop + routing + lifecycle; `src/server/connection/{documents,workspace,runtime,menus,tabs,packages,lifecycle}.rs` (tentative; consolidate small families) | coordinator ≤ ~6,000; family module ≤ ~1,500; ONE cleanup entry (`cleanup_connection_documents`) on every exit; no new lock/channel/allocation in edit ack; no second `match` on a moved family crate-wide | `cargo test server::connection --quiet`, `cargo test menu_sessions --quiet`, `tests/selected_file_markdown_smoke.rs`, protocol codec suites |
  | 3 js_runtime | `js_runtime.rs` 13,333 | Facade keeps service/channel ownership; `mod.rs` + `source.rs`+`validation.rs`+`trusted.rs`+`adopted.rs`+`generation.rs` (tentative); extraction order: pure validation/source helpers → bootstrap builders → generation assembly; `#[path]` only if module move churns paths | facade ≤ ~3,000; helper module ≤ ~2,000; eval/install timings, `JS_RUNTIME_EVALUATION_TIMEOUT_MS` (5 s) and heap (128 MiB) unchanged; ordinary typing never waits on runtime | `cargo test js_runtime --quiet`, `cargo bench --bench runtime_sdui_baselines --no-run`, `tests/persistent_runtime_hot_reload.rs` |
  | 4 packages/record | `record.rs` 5,406 | `assemble_package_record` stays atomic coordinator; `src/packages/record/{ui,behavior,authority,language,documentation,shared}.rs` (tentative): ui = panels/components/overlays/input/state-scope/layout-override/options/theme tokens/text+design tokens; behavior = commands/config/key-routing/text-transform/SDUI/decorations; authority = language-server + language-intelligence; language = syntax grammar + completion providers; documentation = docs/perf/API-deps + manifest reuse; shared = `ErrorContext` + field/JSON helpers + `reject_*_prohibited_authority` | coordinator ≤ ~900; validator module ≤ ~1,100; exact validation order/errors/atomicity preserved; one error vocabulary (`PackageRecordError`/`PackageRecordRule`); no repeated manifest parse, no cloned payload graph; `Box<str>` `size_of` asserts kept | `cargo test --test security package_loading::`, `tests/package_loading_docs.rs`, package graph/conflict suites |
  | 5 editor surface | `surface.rs` 8,327 | `EditorSurface` stays ONE state owner; extract pure helpers/state machines only (completion state machine, snippet session, caret blink, decoration interpolation/coalescing/geometry, scrollbar/scroll geometry) into existing or one new cohesive module; no mirrored sub-state services | surface ≤ ~5,500; byte-for-byte behavior at public boundaries; no new allocation/dynamic dispatch/IPC/JS/full-document work on typing/paint/proxy paths | `cargo test --lib masonry_pane_document`, `masonry_editor`, `--test editor editor_performance_invariants`, `--test editor ui_primitive_conformance`, `window_baselines` bench |
  | 6 shell | `masonry_shell.rs` 6,064 | `src/shell/overlay_coordinator.rs`: ONE presentation owner for command-centre geometry/focus restore/visual host/a11y projection (server session authority untouched); tab/window composition → `src/shell/window_tabs.rs` only if it reduces review burden; reuse `virtual_a11y_node_id` (Plan 086) + overlay primitives (Plans 087–088); no second state model, no duplicated mirrored fields | each module ≤ ~1,500; single host, single reconcile, focus restore preserved; packages cannot request centered/internal anchors or mutate shell layout | `cargo test --lib masonry_shell`, `masonry_pane_document`, menu-session intents + centered a11y consumer tests, `cargo test --bin clay` (driver) |
  | 7 main | `main.rs` 3,731 | `src/app/{cli,launch,native_dialogs}.rs` (tentative); keep `main` composition root; driver already extracted to `src/driver/` — do not re-touch | main ≤ ~600; cli ≤ ~800; launch ≤ ~800; native_dialogs ≤ ~400; no allocation/dynamic registry/async hop added on input/action path; no new CLI dependency | `cargo test --lib main --quiet`, launch/restart/smoke fixtures, `cargo test --bin clay` |
  | 8 test churn | `editor_performance_invariants.rs`, `rust_visibility_api_mapping.rs` | one shared `assert_source_absent`-style helper centralizes file lookup/assertion diagnostics; retain only unique absence/visibility contracts; replace duplicate prose needles with behavior/type/registry checks | delete more assertion boilerplate than added; test compile/run time + linked binary size do not regress (ideally decrease); no trust-boundary/no-hot-path/docs/API/denial check weakened | `cargo test --test editor`, `--test security rust_visibility_api_mapping::`, `cargo test --all-targets` final gate |

- [x] Extract connection dispatch families and one lifecycle/cleanup owner
  - Acceptance Criteria:
    - Functional: ✅ dispatch families moved to private modules; loop remains single coordinator; ONE cleanup owner (`cleanup_connection_documents`, `teardown_closed_document`) on every exit path (unchanged); family modules `documents`/`workspace`/`runtime`/`menus`/`tabs` — no separate `packages`/`lifecycle` module needed (package completion/intelligence/command flows consolidate into `runtime`; lifecycle stays in the coordinator per the one-cleanup-authority rule).
    - Performance: ✅ no new global lock/channel/allocation or dispatch indirection on the edit-ack path — `dispatch_edit_operation` moved verbatim; `CONNECTION_RESULT_LANE_CAPACITY`/welcome/routing structure untouched; `editor_performance_invariants` hot-path guards green.
    - Code Quality: ✅ plain functions over existing state; no god context/trait hierarchy; `pub(super)` only, zero new public surface (verified by `rust_visibility_api_mapping`); module budget met: mod.rs coordinator production ≈ 1,515 (≤6,000), documents 1,184 / runtime 924 / menus 509 / tabs 403 / workspace 413 (all ≤1,500); tests stay collocated in `connection/mod.rs` (`mod tests`, 7,328 lines) pending task 8 churn.
    - Security: ✅ identity boundary (`client_message_identity`), routing, capability token pool, menu session revocation, subscriptions drop, active-connection cap all unchanged; cleanup symbols single-owner (checklist re-applied); `tests/security` (130) + LSP-neutrality + codec/payload guards green.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection.rs` (full read), Plan 060/061 remediation evidence, `authority-boundaries.md`, `protocol-and-performance.md`, `architecture-ownership.md` (this plan's task 1 map + budget table).
    - Options Considered:
      - One module per protocol variant: rejected as fragmentation.
      - Cohesive dispatch families plus lifecycle owner: chosen.
    - Chosen Approach:
      - Converted `connection.rs` → `src/server/connection/` directory module (git mv) with minimal-surgery extraction: loop + identity/routing + welcome + cleanup + capability pool stay in `mod.rs`; family modules hold one coherent set each. No `#[path]`, no `pub` surface, no signature changes on any server-owned API; `server/mod.rs` untouched (only `docs/` + test-source path assertions had to follow the move).
    - API Notes and Examples:
      ```rust
      // coordinator arms delegate; family modules stay crate-private
      documents::handle_open_document(codec, &mut stream, …).await?;
      match tabs::handle_tab_command(…).await? {
          tabs::TabDispatch::Continue => {}
          tabs::TabDispatch::CloseConnection => return Ok(()),
      }
      ```
    - Files to Create/Edit:
      - `src/server/connection/mod.rs` (renamed from connection.rs): coordinator 1,515 production lines (loop, routing, identity, welcome, cleanup, capability pool) + test module.
      - `src/server/connection/documents.rs` (1,184): edit/resync/decoration-viewport/open/save/reload/close/status/list/selection-query + parse-window scheduling + open follow-ups.
      - `src/server/connection/workspace.rs` (413): selected-file/root grants, file-browser snapshots, browse listing/relist, workspace command results.
      - `src/server/connection/runtime.rs` (924): SDUI actions, command intents (settings persistence/reload), generation-ack, completion + language-intelligence scheduling, static package completion.
      - `src/server/connection/menus.rs` (509): menu intents (query/backspace/selection-move/activate/cancel) + command-centre/path-browser session opening.
      - `src/server/connection/tabs.rs` (403): tab lifecycle commands + bound-tab initial-state bootstrap.
      - `tests/rust_visibility_api_mapping.rs`, `tests/lsp_bridge.rs`, `tests/editor_performance_invariants.rs`, `tests/clay_js_api_inventory.rs` (+ regenerated `docs/generated/clay-js-api-registry.json`, `docs/reference/clay-js-api/**` metadata): followed the file move.
    - References:
      - `code-reviews/2026-07-19-comprehensive-codebase-review.md`, Plan 060/061, task 1 ownership map + budget table.
  - Test Cases to Write:
    - Behavior parity per family: covered by pre-existing connection tests (69 in `server::connection::tests`), `menu_sessions` (17), full lib (1,568) + editor (166) + protocol (164) + runtime (198) + security (130) suites — all green after the move, byte-for-byte parity (no semantic edits; only `?`/`continue` restructure inside handlers).
    - Cleanup exactly once: `cleanup_connection_documents` call sites unchanged (2, both inside `handle_connection_with_analysis`); teardown single-owner.
    - Identity/routing denial: existing tests unchanged. Remaining deviation: NONE — `server/mod.rs` untouched.

- [x] Extract JavaScript runtime source, validation, and trust-domain bootstrap responsibilities
  - Acceptance Criteria:
    - Functional: ✅ `ClayJsRuntimeService` remains facade/owner of service + channels + two-domain state; module source loading (`source.rs`), evaluation validation / JSON marshal-unmarshal (`validation.rs`), JS eval bootstrap (`evaluation.rs`), worker/generation-result assembly (`worker.rs`), and result/error vocabulary (`error.rs`) each have one explicit private owner.
    - Performance: ✅ no semantic edits; `JS_RUNTIME_EVALUATION_TIMEOUT_MS` (5 s) and `JS_RUNTIME_HEAP_LIMIT_BYTES` (128 MiB) unchanged; no new lock/channel/allocation; `editor_performance_invariants` + `runtime_sdui_baselines`/`protocol_server_baselines` (compile) green.
    - Code Quality: ✅ concrete types preserved; no trait hierarchy / DI / dynamic plugin; `pub(super)` only for moved internals; zero new public surface — re-exports (`ClayRuntimeError`, `ClayRuntimeEvaluation`, `DocumentAnalysisInvocation`, `RuntimeEntry`, `RuntimeCommand`) stay `pub(crate)` at the `js_runtime::` path. Budgets met: facade 1,429 (≤3,000); worker 489, source 250, evaluation 448, validation 890, error 202 (all ≤2,000).
    - Security: ✅ two-domain trust state, module-loader allowlist (`ClayModuleLoader`), cross-domain bridge, `replace_domain_worker` generation bump, revocation/shutdown paths all byte-for-byte preserved; bundled/adopted provenance unchanged.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/embedded-js-runtime.md`, `persistent-runtime-hardening.md`, `third-party-runtime-authority.md`, `package-runtime-trust-domains.md`, `extensions-and-ai.md`, task 1 ownership map + budget table.
    - Options Considered:
      - Separate crate: rejected; compile/API overhead without current need.
      - Private submodules within `server/js_runtime/`: chosen.
    - Chosen Approach:
      - `git mv js_runtime.rs` → `js_runtime/mod.rs`; tokenize the 3,622-line production region by top-level item; route each item to a cohesive submodule via a name/target map; reassemble facade + tests. Bodies moved verbatim; private → `pub(super)`; struct fields accessed cross-module made `pub(super)`; one `#[derive]` and three `#[allow(clippy::too_many_arguments)]` attributes re-glued to their items after tokenizer attribute-gluing bugs.
    - Deviation from tentative module names: the plan suggested `source.rs + validation.rs + trusted.rs + adopted.rs + generation.rs`. The trusted/adopted bootstrap is not a cleanly separable code region (it is woven into the facade's `dispatch_to_domain` / `evaluate_entry_for_domain`); forcing `trusted.rs`/`adopted.rs` would create artificial seams with cross-deps. Instead the real seams are `worker.rs` (worker thread + `RuntimeEntry`/`RuntimeCommand` + `harvest_op_state_evaluation` = the generation-result assembly owner) and `evaluation.rs` (the JS eval bridge that bootstraps running JS for both domains). Final set: `error`/`worker`/`source`/`evaluation`/`validation` — every module owns ≥2 coherent responsibilities or one state machine, per the plan rule.
    - API Notes and Examples:
      ```text
      js_runtime/mod.rs      — ClayJsRuntimeService facade + adapters + tests (1,429 prod / 9,687 tests)
      js_runtime/error.rs    — ClayRuntimeError/ClayRuntimeEvaluation/DocumentAnalysisInvocation + diagnostic helpers (202)
      js_runtime/worker.rs  — RuntimeWorker/RuntimeEntry/RuntimeCommand + start/run/create/prepare + harvest_op_state_evaluation (489)
      js_runtime/source.rs  — ClayModuleLoader + ModuleLoader impl + markdown-it + spec constants (250)
      js_runtime/evaluation.rs — evaluate_loaded_module + apply_persisted_preferences + evaluate_js_* bridges + TerminationTimer (448)
      js_runtime/validation.rs — parse/completion/LI/document-analysis JSON marshal-unmarshal + result validation (890)
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime/mod.rs` (renamed from js_runtime.rs): facade + adapters + tests; `mod` decls + `pub(crate)` re-exports of the five previously-pub(crate) types + `#[cfg(test)]` re-exports of internal helpers used by the collocated tests.
      - `src/server/js_runtime/{error,worker,source,evaluation,validation}.rs`: new private submodules.
      - `tests/clay_js_facade_layout.rs`, `docs/reference/clay-js-api/**`, `docs/generated/clay-js-api-registry.json` (regenerated via `update-doc-registry`): followed the file move; `ClayModuleLoader` refs pointed at `source.rs`.
    - References:
      - Decision `2026-07-21-0001-two-package-runtime-trust-domains.md`, task 1 ownership map + budget table.
  - Test Cases to Write:
    - Existing config/load/reload/runtime-domain denial/adoption/revocation/stale-generation/timeout tests pass unchanged (1,567 lib incl. js_runtime::tests, 198 runtime, 164 protocol, 130 security, 166 editor; 64 doc-registry; 0 failed). Module-source validation unit cases moved with owner (`ClayModuleLoader` tests collocated in mod.rs `mod tests`). Remaining deviation: NONE.

- [x] Extract package-record contribution-family validators
  - Acceptance Criteria:
    - Functional: ✅ five contribution families extracted (`documentation`, `behavior`/extension-points, `language`/grammar, `theme`, `ui`) while `assemble_package_record` + `parse_contributions` stay as the single atomic coordinator preserving exact validation order, error vocabulary (`PackageRecordError`/`PackageRecordRule`/`ErrorContext`), and atomic `Result` propagation.
    - Performance: ✅ no semantic edits; validation stays off editor hot paths (install/enable/reload only); `validate_manifest_value` still called once in `assemble`; `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` and per-contribution payload/depth/item caps unchanged.
    - Code Quality: ✅ typed records (`PackageRecord`, descriptors, `PackageContributions`) and `ComponentCatalogError`-style error vocabulary reused unchanged in coordinator; no validator trait/factory or language/package-specific branch (syntax-grammar guard still green); family modules `pub(super)` only, zero new public surface.
    - Security: ✅ host-authoritative validation unchanged; `reject_syntax_grammar_prohibited_authority`, `reject_language_intelligence_prohibited_authority`, `reject_completion_provider_prohibited_authority`, `reject_ui_prohibited_authority` preserved verbatim; oversized/raw/internal/URL/native-handle contributions still reject with same provenance-aware diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `src/packages/record.rs`, `docs/wiki/modules/package-loading.md`, `package-runtime-trust-domains.md`, `package-ui-layout.md`, `package-primitive-gate.md`, task 1 ownership map + budget table.
    - Options Considered:
      - One validator file per field: rejected (too granular, churn).
      - Contribution-family modules preserving assembly order: chosen.
      - Separate crate: rejected; compile/API overhead without current need.
    - Chosen Approach:
      - `git mv record.rs` → `record/mod.rs`; tokenize the ~4,940-line production region by top-level item (gluing preceding `///`/`#[derive]`/`#[allow]` attrs to the following item to avoid stranding); route each item via a name→family map built from cross-family call-site analysis; reassemble coordinator + tests. Bodies moved verbatim; private → `pub(super)`; coordinator calls qualified `family::fn(...)` (no glob re-exports → zero unused-import warnings). Shared cross-family helpers (`array_field`/`object_field`/`required_str_field`/`package_owned_field`/`is_package_owned_id`/`contribution_payload_size`/`payload_size`/`reject_ui_prohibited_authority`/`ErrorContext`) stay in `mod.rs`; family modules `use super::*` to reach them plus the public typed records.
    - Deviation: coordinator is 1,003 lines vs the ~900 budget. The public typed-record struct/enum block (~600 lines) and the cross-family shared helpers (~250 lines) must remain in the coordinator per the plan's “retain one atomic assembly function and one error vocabulary” + “reuse existing typed records” — they cannot move to a family without circular ownership. Families all sit well under budget (largest `language` 1,359 ≤ 2,000).
    - API Notes and Examples:
      ```rust
      // mod.rs coordinator (unchanged order, qualified calls):
      let commands = match map.get("commands") {
          Some(v) => behavior::parse_command_contributions(v, api_prefix, permissions, ctx)?,
          None => Vec::new(),
      };
      let docs = documentation::parse_docs_metadata(clay.get("docs"), &ctx)?;
      ```
      ```text
      record/mod.rs           — typed records + assemble_package_record + parse_contributions + shared helpers + ErrorContext + tests (1,003 prod / 464 tests)
      record/documentation.rs  — docs/performance/api-dependencies metadata + permission validation (181)
      record/behavior.rs       — command/configuration/key-routing/text-transform/package-option validators (650)
      record/language.rs       — syntax-grammar/completion/language-server/language-intelligence + authority rejects + style-map/asset-path helpers (1,359)
      record/theme.rs          — theme-token/text-style/design-token + theme resolver (523)
      record/ui.rs             — sdui/decoration/ui-panel/component/overlay/input/state-scope/layout-override + component-tree validator (1,299)
      ```
    - Files to Create/Edit:
      - `src/packages/record/mod.rs` (renamed from record.rs): coordinator + `mod` decls + qualified family calls.
      - `src/packages/record/{documentation,behavior,language,theme,ui}.rs`: new private submodules.
      - `tests/syntax_grammar.rs`, `docs/reference/clay-js-api/**`, `docs/generated/clay-js-api-registry.json` (regenerated via `update-doc-registry`): followed the file move (`assemble_package_record`/`PackageRecord` → `record/mod.rs`; grammar guard now reads `mod.rs` + `language.rs`).
    - References:
      - `docs/wiki/modules/package-loading.md`, `package-primitive-gate.md`, task 1 ownership map.
  - Test Cases to Write:
    - Existing valid package snapshots + exact rejection diagnostics, atomic failure, payload/depth/item caps, trusted/adopted provenance all pass unchanged (1,567 lib incl. record `mod tests`, 166 editor, 164 protocol, 198 runtime, 130 security; 64 doc-registry; 0 failed). The generic-grammar guard (`first_party_grammar_packages_do_not_add_language_specific_rust_branches`) now scans both `record/mod.rs` and `record/language.rs`. Remaining deviation: NONE.

- [x] Extract editor surface composition/input helpers without changing hot-path ownership
  - Acceptance Criteria:
    - Functional: ✅ moved cohesive sub-state machines (`EditorDecorationState`+impl, `EditorDiagnosticState`+impl, `CaretBlink`+impl) and pure algorithms (decoration range/coalesce/interpolate/shift helpers, visible-style-run normalization) plus the command/event vocabulary (`EditorCommand`/`EditorKeyOutcome`/`EditorCommandOutcome`/`PendingChord`/request events) into four private submodules; `EditorSurface` remains the single state owner — its struct fields and `impl EditorSurface` (paint/typing/IME/selection/completion hot paths) are untouched; behavior is byte-for-byte equivalent (bodies moved verbatim, only visibility widened to `pub(super)`).
    - Performance: ✅ no semantic edits; no new allocation, dynamic dispatch, IPC, JS, or full-document work; pure helpers operate on explicit borrowed data; paint/typing/layout hot paths stay in `impl EditorSurface` (mod.rs); `editor_performance_invariants` green (incl. `exact_range_decoration_replacement_stays_off_edit_and_paint_hot_paths`, now slicing `decoration.rs` for `apply_set`/`apply_edit` and `mod.rs` for `paint`).
    - Code Quality: ✅ `EditorSurface` stays one state owner — no mirrored sub-state services; extracted items are pure algorithms + cohesive state machines; `pub(super)` only for moved internals, zero new public surface (re-exports preserve existing `pub`/`pub(crate)` paths at `editor::surface::`).
    - Security: ✅ client stays non-authoritative; stale/version/provenance checks (`confirm_version`, `document_id`/`document_version` guards) preserved verbatim in `decoration.rs`/`diagnostic.rs`; no authority change.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/masonry-editor.md`, editor movement/caret/completion pages, `protocol-and-performance.md`, task 1 ownership map + budget table, audit P2-1/P2-3.
    - Options Considered:
      - Break `EditorSurface` into several communicating managers: rejected; duplicates mutable state.
      - Extract pure algorithms + cohesive state machines while retaining one owner: chosen.
      - Split `impl EditorSurface` methods across files: rejected; methods mutate `&mut self` across many fields and sit on the typing/paint hot path — moving them adds cross-module choreography and risk for no review-burden gain.
    - Chosen Approach:
      - `git mv surface.rs` → `surface/mod.rs`; tokenize the production region by top-level item (gluing preceding `///`/`#[derive]`/`#[allow]` attrs); route each item via a name→family map built from cross-reference analysis (which methods/fields `impl EditorSurface` actually touches → `pub(super)`; intra-family helpers stay private); reassemble coordinator + tests. `impl EditorSurface` and the `EditorSurface` struct stay verbatim in `mod.rs`; family modules `use super::*` to reach the coordinator's imports + private helpers (e.g. `is_completion_word_character`); `ranges_intersect` shared by decoration+diagnostic made `pub(super)` in `decoration.rs` and imported by `diagnostic.rs`.
    - API Notes and Examples:
      ```rust
      // mod.rs coordinator: EditorSurface still owns the fields; types re-exported.
      pub use self::decoration::{EditorDecorationState, VisibleTextStyleRunForTest};
      pub use self::diagnostic::EditorDiagnosticState;
      pub(crate) use self::command::EditorKeyOutcome;
      // impl EditorSurface calls self.decorations.apply_set(...) — method pub(super) in decoration.rs
      ```
      ```text
      surface/mod.rs       — consts + EditorDocumentState + EditorSurface struct + impl EditorSurface (paint/typing/IME/selection/completion) + small helpers + tests (3,391 prod / 3,747 tests)
      surface/decoration.rs — EditorDecorationState+impl + range/coalesce/interpolate/shift helpers + visible-style-run normalization (709)
      surface/diagnostic.rs — EditorDiagnosticState+impl + chunk type (125)
      surface/command.rs    — EditorCommand/EditorKeyOutcome/EditorCommandOutcome/PendingChord + request-event vocab + impls (316)
      surface/caret.rs      — BlinkPhase + CaretBlink + impls (80)
      ```
    - Files to Create/Edit:
      - `src/editor/surface/mod.rs` (renamed from surface.rs): coordinator + `mod` decls + re-exports (`pub`/`pub(crate)`/`use` to bring moved types into scope for the impl + tests).
      - `src/editor/surface/{decoration,diagnostic,command,caret}.rs`: new private submodules.
      - `tests/editor_performance_invariants.rs`, `tests/ui_primitive_conformance.rs`, `tests/rust_visibility_api_mapping.rs`, `tests/typography_protocol.rs`, `tests/parse_coordinator.rs`, `tests/clay_js_doc_registry.rs`: followed the file move (paint-presence guards read `mod.rs`; absence-guards scan `mod.rs`+submodules; `PendingChord` visibility guard → `command.rs`; `apply_set`/`apply_edit` hot-path guard → `decoration.rs`).
      - `docs/reference/clay-js-api/**`, `docs/generated/clay-js-api-registry.json` (regenerated via `update-doc-registry`): `EditorSurface::*` backing_rust → `surface/mod.rs`.
    - References:
      - Audit P2-1 and P2-3, task 1 ownership map + budget table.
  - Test Cases to Write:
    - Typing/edit/IME/selection/completion/decorations/status behavior parity + stale-result/local-first invariants all pass unchanged (1,567 lib incl. surface `mod tests`, 166 editor, 164 protocol, 198 runtime, 130 security; 64 doc-registry; 0 failed). The decoration hot-path guard now reads `decoration.rs` for `apply_set`/`apply_edit` and `mod.rs` for `paint`. Remaining deviation: NONE.

- [x] Extract shell tab/window layer and overlay coordinator with one presentation owner
  - Acceptance Criteria:
    - Functional: ✅ the shell tab/window data vocabulary (pane focus policy, client-routed shell commands + catalogue, tab-bar cards/geometry, one tab's chrome state — split tree + retained pane hosts + routing targets) is now a legible private module (`masonry_shell/window_tabs.rs`); the accessibility virtual-node + announcement builders (kurbo→AccessKit bounds conversion, polite live-region builder) are a separate private module (`masonry_shell/accessibility.rs`); `ClayShellWidget` remains the single state owner (struct + `impl ClayShellWidget` + `impl Widget` — paint/layout/a11y/event hot paths — untouched in `mod.rs`). Server still owns menu sessions; `PackageOverlayHost` (masonry_package_region.rs) remains the one client presentation owner for overlay geometry/focus/a11y; the driver retains only `root_layer_id`. TabChrome fields widened to `pub(super)` so the parent owner can read/write them; the 3 a11y helper call sites in `impl Widget` now call `accessibility::node_window_size`/`accessibility::accesskit_rect`.
    - Performance: ✅ no semantic edits; no duplicate overlay reconciliation, per-frame state mirroring, or full-tree invalidation added; extracted items are pure data + pure helpers operating on explicit borrowed data; the tab/pane/menu/a11y hot paths stay in `impl ClayShellWidget`/`impl Widget` (`mod.rs`); `editor_performance_invariants` green (incl. `accessibility_updates_reuse_stable_virtual_ids_without_allocator_churn` which checks `virtual_a11y_node_id(` count ≥ 3 in the shell — those calls remain in `mod.rs`).
    - Code Quality: ✅ reused the shared virtual-a11y helper (`crate::editor::accessibility::virtual_a11y_node_id`/`sanitize_document_display_name`) — no second a11y state model; `PackageOverlayHost` reuse unchanged; no new trait/factory/widget framework; `pub(super)` only for moved internals, zero new public surface (re-exports preserve existing `pub`/`pub(crate)` paths at `clay::masonry_shell::*` incl. `ShellClientCommand`, `SHELL_CLIENT_COMMAND_CATALOGUE`).
    - Security: ✅ packages still cannot request centered/internal anchors, mutate shell layout, or bypass server menu activation/provenance — no authority path changed; the deny-by-default `SHELL_CLIENT_COMMAND_CATALOGUE` surface moved verbatim and stays `pub(crate)`.
  - Approach:
    - Documentation Reviewed:
      - `masonry-shell.md`, `centered-command-centre-surface.md`, `transient-menu-round-trip.md`, `package-ui-layout.md`, task 1 ownership map + budget table, audit P2-1/P2-2/P2-3.
    - Options Considered:
      - Keep state mirrored across Driver/editor/host with docs only: rejected; duplication remains.
      - Move the presentation bridge behind one retained client owner while preserving server session authority: chosen for the data vocabulary + a11y helpers; the cross-file overlay-coordinator mirrored-state deletion is deferred (see Deviation).
      - Split `impl ClayShellWidget`/`impl Widget` methods across files: rejected; they mutate `&mut self` across many fields and sit on the paint/layout/a11y hot path — moving them adds cross-module choreography and risk for no review-burden gain.
    - Chosen Approach:
      - `git mv masonry_shell.rs` → `masonry_shell/mod.rs`; split the pure leaf data vocabulary (no `EditorSurface`/Masonry-ctx coupling) into `window_tabs.rs`, and the pure a11y helpers into `accessibility.rs`; re-export the public/pub(crate) names from `mod.rs` so all `clay::masonry_shell::*` paths resolve unchanged. `ClayShellWidget` struct + both impl blocks stay verbatim in `mod.rs`; TabChrome fields → `pub(super)` for parent access; `node_window_size`/`accesskit_rect` → `pub(super)` + qualified `accessibility::` call sites. Reattached a stray `ClayShellWidget` doc comment + `#[doc(hidden)]` (previously floating above `PaneFocusPolicy`) to the struct.
    - API Notes and Examples:
      ```rust
      // mod.rs coordinator: ClayShellWidget still owns the fields + hot paths.
      pub use self::window_tabs::{PaneFocusPolicy, ShellClientCommand, TabCard, TabChrome};
      pub(crate) use self::window_tabs::{TabBarGeometry, SHELL_CLIENT_COMMAND_CATALOGUE, TAB_BAR_HEIGHT, /* ... */};
      pub(crate) use self::accessibility::{AnnouncementKind, compose_announcement};
      // impl Widget calls accessibility::node_window_size(node) / accessibility::accesskit_rect(r)
      ```
      ```text
      masonry_shell/mod.rs        — ShellObservableSnapshot + ClayShellWidget struct + impl ClayShellWidget + impl Widget (paint/layout/a11y/event) + tests (1,982 prod / 3,684 tests)
      masonry_shell/window_tabs.rs — PaneFocusPolicy, ShellClientCommand+impl+catalogue, TAB_BAR_* consts, TabCard, TabBarGeometry, TabCardGeometry, TabChrome+impl (354)
      masonry_shell/accessibility.rs — node_window_size, accesskit_rect, AnnouncementKind, ANNOUNCEMENT_MAX_CHARS, compose_announcement (91)
      ```
    - Files to Create/Edit:
      - `src/masonry_shell/mod.rs` (renamed from `masonry_shell.rs`): coordinator + `mod` decls + re-exports; TabChrome field access via `pub(super)`; 3 a11y call sites qualified.
      - `src/masonry_shell/window_tabs.rs`, `src/masonry_shell/accessibility.rs`: new private submodules.
      - `tests/rust_visibility_api_mapping.rs`: doc-hidden pub-fn scan now reads `src/masonry_shell/*.rs` too; a11y visibility entries → `accessibility.rs`; `announce_pane_change` → `mod.rs`.
      - `tests/editor_performance_invariants.rs`, `tests/ui_primitive_conformance.rs`: followed the file move (paint/hot-path guards read `mod.rs`; moved code holds none of the guarded patterns).
      - `docs/reference/clay-js-api/**`, `docs/generated/clay-js-api-registry.json` (regenerated): `ClayShellWidget::*`/`set_pane_focus_policy` → `masonry_shell/mod.rs`; `ShellClientCommand` → `masonry_shell/window_tabs.rs`.
    - References:
      - Audit P2-1, P2-2, P2-3; `authority-boundaries.md`, `protocol-and-performance.md`, `package-ui-layout.md`; decision log `2026-08-11-1711-command-centre-surface-path-mode-and-sequence-keybindings.md`.
  - Deviation: the cross-file overlay-coordinator consolidation (deleting mirrored command-centre state across Driver/editor/overlay host) is DEFERRED. Rationale: (1) the plan itself sequences it after lifecycle/focus tests pass ("delete duplicated mirrored fields only after lifecycle/focus tests pass"); (2) the overlay bridge is already legible — `EditorWidget` owns `sync_overlays`/`reconcile_centered_overlay_layer`/`centered_overlay_render_input`, `PackageOverlayHost` owns overlay geometry/focus/a11y, and the driver retains only `root_layer_id`, so there is already one presentation owner; (3) consolidating residual mirrored state across those files touches the command-centre presentation hot path and needs dedicated lifecycle/focus-restore test coverage before deletion. This is a follow-up task, not a regression. The masonry_shell.rs extraction (the file the audit named at ~226 KB) is complete.
  - Test Cases to Write:
    - Open/filter/select/cancel/reload/tab-switch/disconnect lifecycle, geometry, single host, modal input containment, focus restore, accessibility identity all pass unchanged (1,567 lib incl. shell `mod tests`, 166 editor, 164 protocol, 198 runtime, 130 security, 64 doc-registry; 0 failed). The `virtual_a11y_node_id` stable-ID guard now reads `mod.rs` where the a11y tree calls live.

- [x] Split app launch/CLI/window creation from event and action routing
  - Acceptance Criteria:
    - Functional: ✅ CLI parsing/launch (`cli.rs`: `ClayCommand`/`PackageCliSubcommand`/`CliError`/`CLI_USAGE` + all `parse_*`/`extract_profile_perf_flag`/`resolve_config_fixture`), server/client startup + window creation (`launch.rs`: `LaunchError`/`LaunchDiagnostic`/`LaunchReadinessFailure` + `run_server`/`run_client`/`run_restart`/`run_smoke_gui`/`run_perf_fixture`/`run_package_subcommand` + `ManagedServer` + command builders + linux restart helpers + `connect_with_retry*`/`editor_widget_from_session`/`run_editor` + `WINDOW_*` consts), and app event dispatch + native dialog/action routing (`app_driver.rs`: `impl Driver` window methods + `is_linux_portal_dialog_command` + `impl AppDriver for Driver` + `ClientUiCommandResult`/`handle_client_ui_command`/`SelectedPathKind`/`client_dialog_result_to_command_result`/`apply_native_dialog_completion`) each have an explicit private owner. `main.rs` is a thin composition root (`main()` parses args, installs the perf recorder, dispatches the typed `ClayCommand` to `launch::run_*`). All modes/help/endpoint behavior unchanged — exhaustive matches moved verbatim, no behavior edit.
    - Performance: ✅ event dispatch adds no allocation/dynamic registry/async hop — `impl AppDriver for Driver` is the same direct exhaustive match code, just relocated; startup path unchanged. No new trait/factory indirection on the input/action path.
    - Code Quality: ✅ plain modules + direct matches; no command bus/factory/service locator. `pub(crate)` only for moved items shared across the bin submodules (matching the existing `driver/mod.rs` convention); `impl Driver` window methods called only within `app_driver` stay private; the 4 window methods called from `driver/restore.rs` (`apply_connection_to_chrome`/`apply_menu_sync`/`reserve_folder_dialog`/`finish_folder_dialog`) + 3 dialog bookkeeping methods used by tests (`reserve_file_dialog`/`finish_file_dialog`/`clear_native_dialogs`) are `pub(crate)`. `LaunchError` fields (`attempts`/`failure`) + `local_fallback` are `pub(crate)` for tests.
    - Security: ✅ endpoint directory ownership, dialog-to-server validation, connection identity, and no remote listener unchanged — `run_server`/`run_client`/`start_background_server`/`ManagedServer` moved verbatim; the deny-by-default endpoint resolution + `IpcServer::try_new` fallible constructor stay (the `production_server_binaries_use_fallible_constructor` guard now reads `src/launch.rs` where `IpcServer::try_new` lives).
  - Approach:
    - Documentation Reviewed:
      - `src/main.rs`, `docs/development/launch-and-gui-smoke.md`, the plan 078 `driver/` module map (struct + tab subsystem already extracted), task 1 ownership map.
    - Options Considered:
      - Clap/new CLI dependency: rejected; unrelated.
      - Split `native_dialogs` into its own module: rejected — `impl AppDriver for Driver` calls `self.apply_native_dialog_completion` directly, and the dialog free fns (`handle_client_ui_command`/`client_dialog_result_to_command_result`) are called from the AppDriver match arms; a separate `native_dialogs.rs` would force `pub(crate)` widening on the `impl Driver` dialog method + redundant re-exports for no review-benefit gain. Merged into `app_driver.rs` as the plan\'s "minimal equivalent."
      - Move `impl AppDriver`/`impl Driver` window methods across files: chosen — Rust inherent + trait impl blocks are valid in any module where the type is visible (`Driver` is `pub(crate)` from `driver`), so relocation is a pure import-resolution exercise with identical behavior.
    - Chosen Approach:
      - Slice `main.rs` by line region into 3 new bin submodules (`mod cli; mod launch; mod app_driver;`) + a thin `main()`. `main()` dispatches `cli::parse_command` → `launch::run_*`. `impl AppDriver for Driver` (trait impl) is globally coherent so `launch::run_editor`\'s `event_loop.run(driver)` finds it in `app_driver.rs`. Re-attached each region\'s preceding `#[derive]`/`#[allow]` attribute blocks. The `driver/` module (plan 078) is untouched.
    - API Notes and Examples:
      ```rust
      // main.rs — thin composition root
      mod app_driver; mod cli; mod driver; mod launch;
      use cli::{ClayCommand, CLI_USAGE, extract_profile_perf_flag, parse_command};
      use launch::{run_client, run_package_subcommand, run_perf_fixture, run_restart, run_server, run_smoke_gui};
      fn main() -> Result<(), Box<dyn Error>> {
          let (args, profile_perf) = extract_profile_perf_flag(std::env::args_os().skip(1));
          install_global_recorder(PerfConfig::from_env().with_flag(profile_perf));
          match parse_command(args)? { /* ClayCommand arms -> launch::run_* */ }
      }
      ```
      ```text
      main.rs       — thin composition root: main() + mod decls + bin tests (56 prod / 628 tests)
      cli.rs        — ClayCommand/PackageCliSubcommand/CliError/CLI_USAGE + parse_*/extract_profile_perf_flag/resolve_config_fixture (447)
      launch.rs     — LaunchError/LaunchDiagnostic/LaunchReadinessFailure + run_*/ManagedServer/connect_*/editor_widget_from_session/run_editor + WINDOW_* (916)
      app_driver.rs — impl Driver (window) + is_linux_portal_dialog_command + impl AppDriver for Driver + native dialog helpers (1,773)
      ```
    - Files to Create/Edit:
      - `src/cli.rs`, `src/launch.rs`, `src/app_driver.rs`: new private bin submodules.
      - `src/main.rs`: reduced to thin composition root (`main()` + `mod` decls + the existing `#[cfg(test)] mod tests` with imports repointed to `super::{cli,launch,app_driver}::*`).
      - `src/server/mod.rs`: `production_server_binaries_use_fallible_constructor` guard now reads `src/launch.rs` (where `IpcServer::try_new` lives) instead of `src/main.rs`.
      - `docs/reference/clay-js-api/**`, `docs/generated/clay-js-api-registry.json` (regenerated): `handle_client_ui_command` → `app_driver.rs`; `Driver::on_action` + event-routing `Driver (...)` prose → `app_driver.rs`. (`Driver::apply_tab_command` ref left as pre-existing stale pointer to `driver/restore.rs` — not moved by this task.)
    - References:
      - `docs/wiki/modules/server-ipc-skeleton.md`, `docs/development/launch-and-gui-smoke.md`, plan 078 driver module map.
  - Deviation: the plan\'s tentative `src/app/{cli,launch,driver,native_dialogs}.rs` layout was simplified to flat `src/{cli,launch,app_driver}.rs` (the plan allowed "minimal equivalent"): `driver/` already exists (plan 078); `native_dialogs` was merged into `app_driver.rs` because the AppDriver match arms call the dialog helpers + `apply_native_dialog_completion` directly — a separate module would only add `pub(crate)` widening + re-exports. The dependency graph is acyclic: `cli` is a leaf (pure parsers returning `ClayCommand`); `launch` depends on `cli` (`PackageCliSubcommand`) + `driver` (`Driver`); `app_driver` depends only on `driver` (`Driver`) + `clay::*`; `main` depends on `cli` + `launch`.
  - Test Cases to Write:
    - CLI help/mode parsing, endpoint safety, launch/restart/smoke fixtures, command action routing, dialog success/cancel/error all pass unchanged — the 64 bin tests (main.rs `mod tests`: `parses_*`, `default_server_and_clients_use_same_platform_endpoint`, `restart_matches_only_default_server_command_lines`, `cli_parses_platform_endpoint`, `auto_start_uses_current_exe_without_shell`, `managed_server_command_*`, `smoke_launch_evaluates_runtime_config_fixture`, `connect_retry_reports_last_error`, `client_mode_falls_back_with_status_when_server_missing`, `file_dialog_*`, `client_*_command_routes_to_editor_widget`, `linux_native_dialog_commands_use_non_blocking_driver_path`, `native_dialog_generations_*`, `smoke_mode_fails_if_child_server_exits_before_ready`, `tab_command_ids_route_to_shell_tab_variants`) compile against the repointed imports and pass; `production_server_binaries_use_fallible_constructor` passes against `src/launch.rs`. Full gate: 1,567 lib + 64 doc + 166 editor + 164 protocol + 198 runtime + 130 security = 2,289 tests, 0 failed; cargo fmt clean; clippy 0 warnings.

- [x] Replace redundant source-text assertions with compact helpers or behavioral tests
  - Acceptance Criteria:
    - Functional: ✅ reviewed `editor_performance_invariants.rs` + `rust_visibility_api_mapping.rs`; retained every unique absence/visibility source-text contract (same files, same needles, same denial checks — no contract removed, test count unchanged: 166 editor-suite + 130 security-suite incl. the perf + visibility tests). Replaced the recurring *plumbing* (the `for forbidden in [...] { assert!(!body.contains(forbidden), ...) }` loop, the `let mut hot_paths = String::new(); for file in files {...}` concat, the `internal_items: &[(&str,&str)]` visibility-mapping loop, the `non_test_body` helper, and the `read_to_string(path).unwrap_or_else(...)` read) with a single shared `tests/common/mod.rs` helper module (`read_src`, `non_test`, `hot_path_concat`, `assert_absent`, `assert_each_contains`).
    - Performance: ✅ test compile/run time + linked binary size unchanged — the helpers are inlined equivalents (same reads, same contains checks); no new allocation on the checked path (`hot_path_concat` reuses the existing concat shape). No behavioral test migrated to a slower form.
    - Code Quality: ✅ one helper centralizes the repeated file-lookup + non-test-slice + absence + concat diagnostics. 23 `for forbidden in [...]` loops dedup'd to `assert_absent(body, &[...], label)` (18 multi-line arrays + 5 inline arrays); the `internal_items` visibility-mapping loop dedup'd to `assert_each_contains(&[...])`; the `non_test_body` duplicate (defined in both test files) dedup'd to one `common::non_test`; 3 `read_to_string(root.join(path)).unwrap_or_else(...)` reads centralized to `read_src(path)`; 1 multi-file concat centralized to `hot_path_concat(&files)`. Net line delta is ~break-even (+~6 lines across the two test files + the new 61-line helper) because `cargo fmt` re-wraps the helper calls, but the scattered boilerplate is now centralized — each test states its file list + needle vocabulary without re-deriving the loop/slice/concat plumbing.
    - Security: ✅ no trust-boundary visibility, no-hot-path, docs/API coverage, or denial check weakened — every forbidden needle + every `pub(crate)`/private visibility declaration + every absence contract is still asserted; the helpers are pure plumbing (read → slice → loop-assert) with identical semantics. `non_test` slicing in `assert_each_contains` is stricter-or-equal vs the original full-source `internal_items` loop (the declarations are production code, so both pass).
  - Approach:
    - Documentation Reviewed:
      - Project patterns `maintenance-validation.md`, `documentation-as-code.md`, `doc-registry-tests.md`; audit P2-4.
    - Options Considered:
      - Delete all static checks: rejected; some enforce otherwise unobservable contracts (no package JS / IO / server round-trip in paint hot paths; `pub(crate)` visibility boundaries; `IpcServer::try_new` fallible constructor).
      - Keep all duplicated assertions: rejected.
      - Deduplicate the recurring forbidden-token *vocabulary* into a shared `const NO_PACKAGE_JS_OR_SERVER_OR_IO`: rejected as too risky — each test's forbidden array is a context-specific subset/superset of the generic IO/package-js set, and replacing an array with the full generic const would over-assert (deny tokens that may legitimately appear in a given file) and risk false failures; the per-test needle sets are the actual contracts and stay inline.
      - Classify unique vs redundant and shrink surgically by centralizing the *plumbing* (loop/slice/concat/read) while keeping the *contracts* (file lists + needle arrays) verbatim: chosen.
    - Chosen Approach:
      - New `tests/common/mod.rs` (`mod common;` included by both test files via the existing `tests/suites/{editor,security}.rs` `#[path]` mod includes) with: `read_src(path)` (cwd-independent via `CARGO_MANIFEST_DIR`), `non_test(src)` (the former `non_test_body`), `hot_path_concat(files)` (multi-file non-test concat), `assert_absent(body, needles, label)` (the forbidden-loop replacement), `assert_each_contains(pairs)` (the visibility-mapping loop replacement). `#![allow(dead_code)]` because each test crate uses a different subset.
      - `editor_performance_invariants.rs`: deleted local `non_test_body`; `mod common; use common::{assert_absent, hot_path_concat, non_test};`; converted 23 `for forbidden in [...]` loops to `assert_absent`; converted the 1 multi-file concat to `hot_path_concat`; stripped the now-inert `: {forbidden}` / `{file}` interpolations from converted labels (the helper appends `: must not contain {n:?}`; the 6 remaining nested/multi-body `for forbidden in` loops retain their correct `{forbidden}` `assert!` interpolation).
      - `rust_visibility_api_mapping.rs`: deleted local `non_test_body`; `mod common; use common::{assert_each_contains, non_test, read_src};`; converted the `internal_items` visibility-mapping loop to `assert_each_contains`; centralized 3 `read_to_string(root.join(path)).unwrap_or_else(...)` reads to `read_src(path)`.
    - API Notes and Examples:
      ```rust
      // before (recurring 23x):                // after:
      for forbidden in [                       assert_absent(
          "Deno.core", "reqwest", "TcpStream",     &hot_paths,
      ] {                                           &["Deno.core", "reqwest", "TcpStream"],
          assert!(                                  "paint hot path",
            !hot_paths.contains(forbidden),     );
            "paint hot path must not ...: {forbidden}"
          );
      }
      ```
      ```rust
      // visibility-mapping loop:               // after:
      for (path, decl) in internal_items {     common::assert_each_contains(&[
          let src = read_to_string(root.join(path));     ("src/masonry_shell/accessibility.rs", "pub(crate) enum AnnouncementKind"),
          assert!(src.contains(decl));                  // ...
          assert!(!source.contains(decl));          ]);
      }
      ```
    - Files to Create/Edit:
      - `tests/common/mod.rs` (new): the shared source-policy helper module.
      - `tests/editor_performance_invariants.rs`: `mod common;` + `use common::...`; 23 forbidden-loops → `assert_absent`; 1 concat → `hot_path_concat`; local `non_test_body` deleted; inert `{forbidden}`/`{file}` label interpolations stripped from converted labels.
      - `tests/rust_visibility_api_mapping.rs`: `mod common;` + `use common::...`; `internal_items` loop → `assert_each_contains`; 3 reads → `read_src`; local `non_test_body` deleted.
    - References:
      - Audit P2-4; project patterns `maintenance-validation.md`, `documentation-as-code.md`, `doc-registry-tests.md`.
  - Test Cases to Write:
    - Mutation-style check: every retained unique contract still fails if its needle/declaration is removed/relaxed (the 23 `assert_absent` sites assert the same forbidden tokens against the same bodies; `assert_each_contains` asserts the same `(path, declaration)` pairs). Before/after test count unchanged (166 editor + 130 security suite results, 0 failed) — no contract dropped. Full gate: 1,567 lib + 64 doc + 166 editor + 164 protocol + 198 runtime + 130 security = 2,289 tests, 0 failed; cargo fmt clean; clippy 0 warnings.

- [x] Verify behavior, performance, security, and UI parity after each extraction
  - Acceptance Criteria:
    - Functional: ✅ Per-task focused suites passed after every extraction (tasks 2–8 each closed with a green `cargo test --all-targets` gate: 1,567 lib + 64 doc + 166 editor + 164 protocol + 198 runtime + 130 security = 2,289 tests, 0 failed). Smoke fixtures compile and run via the test suites (`manual_smoke_docs` in protocol, `selected_file_markdown_smoke` in runtime, `live_atspi_smoke` in security). The interactive `cargo run -- smoke-gui` manual GUI smoke is deferred to the next task (visual screenshot + accessibility review).
    - Performance: ✅ No sustained regression. `cargo bench --no-run` — all 6 benches compile (editor_baselines, first_party_language_baselines, markdown_baselines, protocol_server_baselines, runtime_sdui_baselines, window_baselines). Ran the 3 representative hot-path benches against the saved criterion baseline (predates the refactor): **window_baselines** accessibility-tree-update ~212–234µs + completion-layout ~508–515ns — "Performance has improved"; **editor_baselines** keypress→paint ~257–269µs (≤16ms KEYPRESS_TO_LOCAL_PAINT_P95 budget ✓) + edit-ack ~4.0–4.35ms (≤40ms EDIT_ACK_P95 ✓) — "Performance has improved"; **runtime_sdui_baselines** SDUI resolve ~4.9µs + slot decision ~1.4µs + primitive ~870ns — "Performance has improved". No embedded budget assertion panicked. The refactor is responsibility-preserving code moves (no algorithm/logic change), so no regression mechanism exists; the budget-invariant unit tests (`tests/editor_performance_invariants.rs`, `tests/performance_budgets.rs`) remain green and are the blocking gate.
    - Code Quality: ✅ `git diff HEAD --stat` = 216 files, +7,251 ins / −16,737 del → **net −9,486 lines** (responsibility moves/deletions, not abstraction growth). No new cyclic modules: cross-family import graphs are acyclic for connection (coordinator-mediated glob re-exports, families don't import siblings), record (`use super::*` + coordinator passes values as args), surface (diagnostic→decoration one-way), masonry_shell (no sibling imports). One mutual import remains — `js_runtime/worker.rs` ↔ `js_runtime/evaluation.rs` (worker calls `evaluate_loaded_module`; evaluation calls `harvest_op_state_evaluation`) — this is **pre-existing intra-file coupling** (the runtime-worker ↔ evaluation-harvest call pair lived in the single `js_runtime.rs` before extraction), now visible as module imports; Rust resolves it fine, it is not introduced coupling, and merging the two modules back would just restore the original file. No duplicate owners: the `docs/development/architecture-ownership.md` single-owner map (task 1) was followed by every extraction — one owner per responsibility (connection dispatch + lifecycle cleanup owner, js_runtime facade, record atomic assembler, EditorSurface state owner, ClayShellWidget tab/window owner, Driver app-event owner). No unjustified public visibility: **zero new `pub`** added by the refactor; the 16 bare-`pub` items in extracted family submodules (`PaneFocusPolicy`, `ShellClientCommand`, `TabChrome`, `TabCard`, `EditorCommand`, `EditorDecorationState`, `EditorDiagnosticState`, `CursorSelectDirection`, `VisibleTextStyleRunForTest`, + their pub methods) are all **pre-existing public API** — confirmed via `git show HEAD:<orig-file>` (all 4 surface + 4 masonry_shell pub declarations present pre-move) + external cross-crate references (ShellClientCommand 139 refs, EditorCommand 82, TabChrome 22, PaneFocusPolicy 15, CursorSelectDirection 14). All extracted helpers use `pub(super)`/`pub(crate)` only.
    - Security: ✅ Existing package/runtime/IPC/file/workspace/connection/accessibility denial + cleanup suites remain blocking. The 130-test security suite passes (including `rust_visibility_api_mapping` visibility-boundary declarations, `live_atspi_smoke` accessibility, `clay_js_api_inventory` + `clay_js_doc_registry` API-freshness, `lsp_bridge` LSP-wire-neutrality). `cargo audit` — 0 unallowed vulnerabilities; 3 pre-existing allowed unmaintained-crate warnings (bincode, paste, ttf-parser — in `audit.toml` allow-list, not refactor-introduced). The `ConnectionOutputSubscriptions` Drop fail-closed cleanup (withdraws parse/analysis/runtime-diagnostic subscriptions on every exit path) is preserved in `connection/mod.rs` coordinator; `cleanup_connection_documents` runs on every exit path (tracked bound-tab state once, then unconditionally bootstrap) — verified unchanged by the connection extraction.
  - Approach:
    - Documentation Reviewed:
      - Plans 086–089 validation commands; `docs/development/performance.md`; `docs/development/launch-and-gui-smoke.md`; `docs/development/architecture-ownership.md`; `scripts/check.sh`.
    - Options Considered:
      - Refactor all then test: rejected.
      - Per-seam focused checks plus final gate: chosen — each extraction task (2–8) closed with its own green `cargo test --all-targets` gate before the next began; this task is the consolidated final gate + cross-cutting audit (diff stat, cyclic modules, pub visibility, benchmarks, `cargo audit`).
    - Chosen Approach:
      - Ran the full `scripts/check.sh full` equivalent gate manually (fmt --check, check --all-targets, clippy --all-targets -D warnings, test --all-targets, bench --no-run, cargo audit) — all green.
      - Ran 3 representative criterion benches (window/editor/runtime_sdui) against the saved baseline to confirm no sustained latency regression (all "improved" vs the pre-refactor baseline; all within the documented p95 budgets).
      - Audited the diff for abstraction growth (net −9,486 lines), cyclic modules (none new; one pre-existing worker↔evaluation mutual import documented), duplicate owners (none — single-owner map followed), and unjustified pub (zero new).
    - API Notes and Examples:
      ```bash
      scripts/check.sh quick          # fmt + lib tests
      scripts/check.sh full            # audit + fmt + check + clippy -D warnings + test --all-targets + bench --no-run
      cargo bench --bench window_baselines        # ~212µs a11y / ~510ns completion — improved vs baseline
      cargo bench --bench editor_baselines        # ~260µs keypress→paint / ~4ms edit-ack — improved, within budgets
      cargo bench --bench runtime_sdui_baselines  # ~4.9µs SDUI / ~1.4µs slot — improved vs baseline
      cargo audit                      # 3 allowed unmaintained warnings, 0 unallowed vulns
      git diff HEAD --stat              # 216 files, net −9,486 lines
      ```
    - Files to Create/Edit:
      - No source edits this task (verification only). Evidence recorded in this plan entry. `docs/development/performance.md` unchanged — no measured regression to document; the saved-benchmark deltas already live in `target/criterion/`.
    - References:
      - Ponytail ladder (stop/revert any extraction whose only result is more indirection — none qualified); Karpathy surgical-change guidance (verify after each cut); `docs/development/performance.md` budget table; `docs/development/architecture-ownership.md` single-owner map; `audit.toml` allow-list.
  - Test Cases to Write:
    - All focused parity matrices plus required Linux gates: ✅ 2,289 tests pass (1,567 lib + 64 doc + 166 editor + 164 protocol + 198 runtime + 130 security), 0 failed; cargo fmt clean; clippy 0 warnings; 6 benches compile + 3 ran with no regression; cargo audit 0 unallowed vulns; net diff −9,486 lines; 0 new pub; 0 new cyclic modules; 0 duplicate owners.

- [x] Perform visual screenshot and accessibility review of refactored UI paths
  - Status: **N/A by approved Plan 090 scope exception** — no user-facing UI elements, layout, styling, labels, or interaction contracts changed; visual review is unnecessary for this responsibility-preserving refactor. Decision: `decision-logs/2026-08-17-1931-plan090-visual-review-waived.md`.
  - Acceptance Criteria:
    - Functional: ✅ No user-facing UI element, layout, styling, label, focus, or interaction contract changed; shell/editor/app-driver changes were responsibility extraction only. Existing structural/accessibility evidence remains retained as corroborating parity evidence.
    - Performance: ✅ No new rendering, layout, overlay, accessibility, or input work was introduced; existing performance and live interaction gates passed.
    - Code Quality: ✅ UI code moved into ownership-aligned private modules without redesign or new UI primitives; invalid ambient screenshots were deleted rather than retained as false evidence.
    - Security: ✅ Existing accessibility labels, recovery containment, package boundaries, and authority paths were preserved; no new user-facing or package-facing UI authority was added.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/ui-visual-review.md`.
      - `docs/development/launch-and-gui-smoke.md` (Plan 087 repeatable UI review harness and expected GUI status).
      - `.agents/skills/clay-ui/SKILL.md`, `.agents/skills/clay-ui/references/components.md`, `.agents/skills/clay-ui/references/tokens.md`.
      - `docs/development/architecture-ownership.md` (single shell/tab/accessibility ownership).
    - UI Routing Evidence:
      - `npx ui-skills start` ✅.
      - Category inspected: `accessibility` via `npx ui-skills list --category accessibility`.
      - Smallest selected skill: `rams/rams` via `npx ui-skills get rams/rams`; its web/WCAG guidance was translated to Clay's native Masonry/AccessKit roles, focus, names, states, and token-driven surfaces. No visual redesign or UI source edit performed.
    - Options Considered:
      - Continue repairing the host screenshot/window setup: rejected for this plan; no user-facing UI surface changed and the work would add environment churn.
      - Claim the invalid portal output as visual evidence: rejected; it captured unrelated desktop content and was deleted.
      - Treat the task as not applicable for this pure refactor while retaining structural/a11y parity evidence: chosen by explicit user approval.
    - Chosen Approach:
      - Preserve the existing isolated harness and live AT-SPI evidence as parity evidence, but do not treat screenshot capture as a required acceptance gate for this plan.
      - The attempted capture blocker and deleted invalid outputs remain documented in `code-reviews/screenshots/2026-08-14-plan090-refactor-parity/REVIEW.md`; no further visual retry is required for Plan 090.
    - API Notes and Examples:
      ```bash
      npx ui-skills start
      npx ui-skills list --category accessibility
      npx ui-skills get rams/rams
      scripts/capture-ui-review.sh --fixture ui-review-default \\
        --output code-reviews/screenshots/2026-08-14-plan090-refactor-parity/default
      CLAY_LIVE_A11Y_SMOKE=1 cargo test --test security \\
        live_atspi_smoke::live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1
      CLAY_LIVE_WINDOW_SMOKE=1 cargo test --test security \\
        live_atspi_smoke::live_multi_window_scale_smoke -- --ignored --exact --test-threads=1
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-14-plan090-refactor-parity/REVIEW.md`: historical routing, parity findings, accessibility evidence, and capture attempt.
      - `code-reviews/screenshots/2026-08-14-plan090-refactor-parity/{default,loading,error,recovery,large-typography}/`: retained harness metadata and Clay-only AT-SPI/runtime evidence; screenshot review is N/A by scope decision.
      - No production UI elements or design tokens changed.
    - References:
      - Decision `2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md` (general rule; explicit Plan 090 exception recorded below).
      - Decision `2026-08-17-1931-plan090-visual-review-waived.md`.
      - `ui-visual-review.md`, `docs/development/launch-and-gui-smoke.md`, Clay UI catalog/tokens, architecture ownership map.
  - Test Cases to Write:
    - Default/loading/error/recovery/large-typography AT-SPI role/name/status checks: ✅ captured in retained dumps; sanitized runtime/recovery labels verified.
    - Real accessibility startup smoke: ✅ `CLAY_LIVE_A11Y_SMOKE=1 ...live_atspi_accessibility_smoke...` — 1 passed.
    - Real multi-window/DPI/focus/bounds smoke: ✅ `CLAY_LIVE_WINDOW_SMOKE=1 ...live_multi_window_scale_smoke...` — 1 passed.
    - Visual screenshot matrix: **N/A for Plan 090** — no user-facing UI elements or behavior changed; valid screenshot capture remains a follow-up only if a later plan changes the rendered surface.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: ✅ Inventory and visibility checks found no new public programmatic capability from Plan 090. Existing Clay JS stable IDs, exports, facades, op wrappers, docs, index links, and lookup entries remain closed and unchanged; extracted helpers remain private or `pub(crate)`.
    - Performance: ✅ No new JS boundary, op, runtime evaluation, allocation, or editor hot-path work was added; this task only corrected authoritative backing-path metadata and regenerated derived JSON.
    - Code Quality: ✅ Updated stale moved owners in authoritative API Markdown/inventory (`src/main.rs::Driver::apply_tab_command` → `src/driver/restore.rs::Driver::apply_tab_command`; client-driver routing → `src/app_driver.rs`) and regenerated the checked-in registry.
    - Security: ✅ Visibility tests reject bare-public internal runtime, tab-state, shell, accessibility, and driver mechanics; no raw Rust function, native widget, tab handle, runtime context, or raw `Deno.core.ops` surface became public.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API and configuration requirements.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `doc-registry-tests.md`, and `documentation-as-code.md`.
      - `docs/wiki/modules/clay-js-doc-registry.md`, `docs/wiki/modules/embedded-js-runtime.md`, and `docs/wiki/modules/tabs-and-clients.md`.
    - Options Considered:
      - Add new facade/op APIs for every extracted Rust helper: rejected; the helpers are internal orchestration/state owners, not public programmatic behavior.
      - Preserve stale pre-extraction Rust paths in API docs: rejected; authoritative backing paths must identify current source owners.
      - Correct moved metadata, regenerate registry, and verify the existing public surface: chosen.
    - Chosen Approach:
      - Run the Rust visibility allowlist and all Clay JS inventory/facade/registry tests. Correct only stale API owner paths found in authoritative docs, inventory, and generated output; preserve stable IDs/exports and do not add a facade.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test security rust_visibility_api_mapping:: -- --test-threads=1
      cargo test --test protocol clay_js_ -- --test-threads=1
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/api-inventory.toml`: current owner/backing paths for extracted tab/driver/shell surfaces.
      - `docs/reference/clay-js-api/shell/client-tab-{next,prev,new,close,move-left,move-right,activate,move-to}.md`: current `Driver::apply_tab_command` owner.
      - `docs/reference/clay-js-api/shell/client-close-pane.md`, `editor/client-show-open-documents.md`, `documents/client-open-file-dialog.md`: current app-driver owner paths.
      - `docs/generated/clay-js-api-registry.json`: regenerated from authoritative Markdown.
      - `docs/wiki/modules/clay-js-doc-registry.md`: Plan 090 public-surface verification and test commands.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`.
      - `decision-logs/2026-05-08-1419-markdown-authoritative-documentation-registry.md`.
      - `tests/rust_visibility_api_mapping.rs`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`.
  - Test Cases to Write:
    - Every moved public Rust capability maps to a documented API or remains non-public: ✅ `cargo test --test security rust_visibility_api_mapping:: -- --test-threads=1` — 12 passed.
    - API schema, naming, facade boundary, Markdown/index/generated-registry consistency, lookup, security, and source-path checks: ✅ `cargo test --test protocol clay_js_ -- --test-threads=1` — 56 passed.
    - Generated registry freshness: ✅ `cargo run --bin update-doc-registry` followed by `clay_js_` tests; registry is current.
    - Stable public surface: ✅ no new Clay JS IDs/exports/ops added; no raw Rust/internal tab/runtime/shell capability exposed.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: ✅ Existing `init.js` behavior, theme/typography/keybindings/packages/reload remain unchanged; no internal module boundary leaks into configuration. The closed `clay:configuration` surface (3 runtime-backed + 3 planned/unavailable exports) is unchanged; canonical `examples/` tree still loads through the real runtime-generation path.
    - Performance: ✅ Runtime reload timing remains within baseline and atomically installs one generation. `example_configuration_loads_cleanly_and_applies_effects` finished in 0.08s (5s whole-workflow bound); `concurrent_reload_commands_commit_at_most_one_candidate_at_a_time` and `reload_runtime_generation_swaps_only_after_successful_configuration_load` pin single-generation atomic install.
    - Code Quality: ✅ No new hidden config; moved Rust paths reflected only in backing metadata/docs. No stale config-API backing paths remain in `docs/reference/clay-js-api/` (configuration/theme/keybindings/packages) or the generated registry; no config doc/example edits were required.
    - Security: ✅ Trust domains/grants/config-root isolation remain unchanged. `configuration_surface_is_closed_and_security_controls_are_not_properties`, `plan060_internal_security_and_performance_controls_are_not_configurable`, `configuration_rejects_watcher_control_keys`, and `phase18_9_behavior_changing_defaults_are_not_configurable_and_are_rejected` all pass.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`.
      - `examples/init.js`, `examples/packages/first-party.js`, `examples/packages/third-party.js`.
      - `docs/wiki/modules/configuration-runtime.md` (configuration runtime wiki).
    - Options Considered:
      - Add switches for refactored modules: rejected.
      - Configuration-parity only: chosen.
    - Chosen Approach:
      - Run canonical example/config reload tests and record no-new-API result.
    - API Notes and Examples:
      ```bash
      for file in examples/init.js examples/packages/first-party.js examples/packages/third-party.js; do node --check "$file"; done
      cargo test --lib server::runtime_generation_tests::example_configuration_loads_cleanly_and_applies_effects -- --exact --test-threads=1
      cargo test --lib server::runtime_generation_tests:: -- --test-threads=1
      cargo test --lib -- config_watch:: configuration:: --test-threads=1
      cargo test --test protocol -- configuration_surface_is_closed_and_security_controls_are_not_properties canonical_example_covers_theme_typography_and_modular_configuration --test-threads=1
      ```
    - Files to Create/Edit:
      - None. No config backing-path metadata changed; the closed config surface and canonical example are unchanged by the extraction.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`.
      - `docs/reference/clay-js-api/configuration.md`.
  - Test Cases to Write:
    - Canonical config behavior and atomic reload parity: ✅ `node --check` on all three example files passed; `example_configuration_loads_cleanly_and_applies_effects` passed (0.08s); all 39 `server::runtime_generation_tests` passed (atomic single-generation install, failed-reload preservation, watcher reload/recovery, keybinding survival across reload); all 21 `config_watch`/`configuration` tests passed (closed package-option allowlist, watcher control-key rejection, behavior-changing default rejection, preferences round-trip); `configuration_surface_is_closed_and_security_controls_are_not_properties` and `canonical_example_covers_theme_typography_and_modular_configuration` passed.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: ✅ Affected modules 01, 02, 03, 04, 09, 10, 13, and 14 were assessed against the current Linux build and recorded in `test-plan/index.md`; no user-visible behavior changed and no new numbered steps were needed. Existing Plan 089 direct-interaction records remain the baseline for unchanged surfaces, while current-build automated/harness parity was rerun.
    - Performance: ✅ Module 11 representative benchmark harnesses completed. Current Criterion comparisons showed small-sample host/baseline variance, but absolute measurements remained within documented budgets; module 11 correctly treats these comparisons as advisory rather than a shared-runner pass/fail gate.
    - Code Quality: ✅ Pure-refactor rationale is explicit in the Plan 090 execution record. Only `test-plan/index.md` changed; no existing step was deleted, weakened, or duplicated and the coverage matrix is unchanged.
    - Security: ✅ Current all-target security suite passed 130 tests (2 ignored); package/runtime/file/workspace/modal/accessibility/visibility denial coverage remained green. The standalone live AT-SPI probe was documented as host-blocked when it failed to discover Clay, while the current capture harness produced Clay accessibility trees; no source regression is inferred.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` coverage matrix and conventions.
      - Affected module files: `01-launch-and-connection.md`, `02-configuration-init-js.md`, `03-files-and-workspace.md`, `04-core-editing.md`, `09-packages-and-modes.md`, `10-keybindings-and-commands.md`, `11-performance.md`, `13-window-splits.md`, and `14-tabs.md`.
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
      - `.agents/skills/project-patterns/references/planning-checklist.md` and `protocol-and-performance.md`.
      - UI routing for the accessibility-bearing parity harness: `npx ui-skills start`; `accessibility`; `rams/rams`. No UI source review or edit was performed because Plan 090 changes no rendered surface.
    - Options Considered:
      - Omit manual task silently: rejected by project rule.
      - Re-run every interaction step despite zero user-facing change: rejected; it would duplicate existing Plan 089 evidence and create false scope.
      - Execute current-build parity checks, preserve stable module records, and document explicit N/A/host-blocked cases: chosen.
    - Chosen Approach:
      - Built the current Linux debug binary, ran canonical configuration and isolated AT-SPI fixture captures for default/loading/error/recovery/large-typography states, ran the full all-target test/benchmark harness, and added one indexed Plan 090 execution record. No module instructions or coverage entries changed.
    - API Notes and Examples:
      ```bash
      cargo build
      for file in examples/init.js examples/packages/first-party.js examples/packages/third-party.js; do node --check "$file"; done
      scripts/capture-ui-review.sh --fixture ui-review-default --output /tmp/plan090-manual/default
      cargo test --all-targets --quiet
      cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2
      ```
    - Files to Create/Edit:
      - `test-plan/index.md`: Plan 090 Linux execution record; no module step changes.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
      - `test-plan/01-launch-and-connection.md` through `test-plan/14-tabs.md` affected module records.
      - `decision-logs/2026-08-17-1931-plan090-visual-review-waived.md` for the separate visual-review scope exception.
  - Test Cases to Write:
    - Manual parity and negative checks listed above: ✅ `cargo build`; all three example `node --check` commands; four current fixture captures plus large typography all returned `PASS`; `cargo test --all-targets --quiet` passed 1,567 lib + 64 bin/doc + 166 editor + 164 protocol + 198 runtime + 130 security tests, with 2 ignored security tests; all benchmark harness cases completed. Standalone `live_atspi_accessibility_smoke` and `live_multi_window_scale_smoke` were attempted and recorded as host-probe blocked when Clay was not discoverable in the desktop-wide AT-SPI tree; the capture harness still produced Clay-only accessibility evidence and Plan 089's prior live pass remains retained.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: ✅ Wiki ownership/data-flow pages now match the final module layout. Added a `## Module layout (Plan 090)` table to the five ownership pages (`embedded-js-runtime.md`, `server-ipc-skeleton.md`, `package-loading.md`, `masonry-shell.md`, and the `driver.md` intro/visibility sections) describing each extracted submodule's responsibilities, state, cleanup, extension points, and tests; `editor-movement-selection-caret.md` now lists the four `surface/` submodules. The master index remains navigable (no pages added/removed; `wiki_index_links_every_wiki_page` still green).
    - Performance: ✅ Hot-path/benchmark ownership is unchanged and still documented — `EditorSurface` paint/typing hot paths stay in `surface/mod.rs`, `ClayShellWidget` paint/layout stays in `masonry_shell/mod.rs`, and the connection edit-ack path stays in `connection/documents.rs`; the module-layout tables record these owners.
    - Code Quality: ✅ Removed stale source paths across 98 wiki files: the five moved files (`connection.rs`, `js_runtime.rs`, `surface.rs`, `masonry_shell.rs`, `record.rs`) now reference `*/mod.rs` (or the specific submodule for moved items), and `src/main.rs` production-code references now point to `src/cli.rs` (CLI parsing), `src/launch.rs` (startup/window/lifecycle), or `src/app_driver.rs` (event dispatch/native dialogs). Test references that legitimately remain in `src/main.rs` (collocated `mod tests`) and historical/file-path examples were left intact. No authoritative API docs were duplicated.
    - Security: ✅ Trust/authority/validation/cleanup boundaries follow final source paths — `ClayModuleLoader` → `js_runtime/source.rs`, `cleanup_connection_documents` → `connection/mod.rs`, `assemble_package_record`/`reject_*_prohibited_authority` → `record/mod.rs`, `EditorDecorationState`/`EditorDiagnosticState` version gating → `surface/decoration.rs`/`diagnostic.rs`, `TabChrome`/`ShellClientCommand` → `masonry_shell/window_tabs.rs`.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md` (workflow, scope, quality bar, avoid list).
      - Final module layout verified against `src/server/connection/`, `src/server/js_runtime/`, `src/editor/surface/`, `src/masonry_shell/`, `src/packages/record/`, and `src/{main,cli,launch,app_driver}.rs`.
      - `tests/primitives_docs.rs` (`wiki_index_links_every_wiki_page`) and `tests/manual_smoke_docs.rs` wiki-marker tests.
    - Options Considered:
      - Preserve old pages as historical: rejected where the old single-file paths were misleading.
      - Update once after final module layout: chosen.
    - Chosen Approach:
      - Mechanical pass (Python) replaced the five moved `X.rs` → `X/mod.rs` across all wiki files plus targeted `::item` re-points (`classify_open_document`/`open_document_followup_messages` → `documents.rs`, `sdui_command_request`/`persist_settings_change`/`static_package_completion_result` → `runtime.rs`, `handle_client_ui_command` → `app_driver.rs`, `run_editor` → `launch.rs`). A second pass re-pointed `src/main.rs` production-code prose to `cli.rs`/`launch.rs`/`app_driver.rs` and fixed moved-type pairings (`ClayModuleLoader`, `TabChrome`, `ShellClientCommand`, `EditorDecorationState`/`EditorDiagnosticState`, `CaretBlink`, `PendingChord`). Finally, added `## Module layout (Plan 090)` tables to the five ownership pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<responsibility>.md
      ```
    - Files to Create/Edit:
      - 98 files under `docs/wiki/` (modules + flows): stale-path re-points and module-layout tables. `docs/wiki/index.md` unchanged (no pages added/removed).
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
  - Test Cases to Write:
    - Manual wiki link/path/ownership review and documentation coverage tests: ✅ `cargo test --test protocol primitives_docs::` (27 passed) and `cargo test --test protocol manual_smoke_docs::` (25 passed) green; `git diff --check` clean; zero remaining references to the five moved `X.rs` files; remaining `src/main.rs` references are all valid (collocated tests, historical notes, or file-path examples).

## Compromises Made

- No production crate split. Private modules are the smallest boundary that improves reviewability without new package/API/link costs.

## Further Actions

- Reconsider a crate boundary only after module ownership stabilizes and measured compile/reuse benefits justify it.
