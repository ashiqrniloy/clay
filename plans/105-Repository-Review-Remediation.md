# Repository Review Remediation

Source: comprehensive repository review completed 2026-08-31 on branch `code_review/refactor` (architecture, duplication, performance, cognitive complexity, security, documentation drift, hygiene). All gates were green at review time; every finding below is erosion-level, not architecture-level.
Scope note: this plan contains **no UI-surface tasks** (no component, panel, token, layout, theme, or typography changes). The mandatory UI skill stack gate applies only if a task's execution turns out to change rendered UI — if that happens, stop the task, load `clay-ui` + the four mandatory design skills, and record it. No editor mode, language mode, JS package, package runtime capability, or package extension point is added or materially changed, so no primitive-review task is required. No new user-visible behavior, configuration surface, or public Clay JS API is introduced; the Clay JS API and configuration tasks below are verification-only.

## Objectives

- Remove all dead code found by the review: the unreferenced native dialog backends in `src/client/file_dialog.rs` and every tracked junk file at the repository root.
- Resolve every documentation-drift finding: `docs/development/architecture-ownership.md` stale paths/counts, `DESIGN.md`/recipe-matrix "25 Component Kinds" conflation, `docs/development/security.md` incomplete CSP, `docs/wiki/modules/client-file-dialog.md` contradiction.
- Reduce the top cognitive-complexity hotspots: extract the ~1,092-line `handle_connection_loop` dispatch arms into the existing connection family modules, split the `workspace-controller.ts` monolith, and move giant inline test modules to sibling test files following the `js_runtime` precedent.
- Restore CI and bundle headroom: split the 656-second `editor_performance` suite so it parallelizes, and code-split the >500 KiB index chunk.
- Close the P3 audit items: `runtime/js` `.d.ts` drift check, `std::sync::Mutex`-across-`await` audit, production-path `unwrap` sweep.
- Keep every existing gate green throughout: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo audit`, frontend lint/format/test/build/bundle budget, clay-agent tests.

## Expected Outcome

- No unreferenced `unsafe` code remains in `src/`; the only dialog path is the Tauri-side ashpd portal commands (`src-tauri/src/commands.rs`), and the wiki matches reality.
- `git ls-files` no longer lists `tmp/`, `client*.log`, `client-hang.log`, `EOF`, `test.md`, `todo.md`, `typescript`, or an empty `vendor/`.
- `handle_connection_loop` is a coordinator of bounded size (target ≤ 350 lines); dispatch arms live in `connection/{documents,menus,runtime,tabs,workspace}.rs` with unchanged behavior and unchanged test names.
- No source file keeps thousands of lines of inline test code: the six flagged files follow the `js_runtime` `mod tests;` sibling-file pattern.
- The `editor_performance` suite wall time drops from ~656 s to ≤ 240 s on the development machine with full matrix coverage retained (or a recorded, explicitly approved coverage policy if the 50 MiB leg must be gated).
- The index chunk is below the 500 KiB Vite warning threshold; gzip budgets keep passing with improved headroom (`frontend/scripts/bundle-budget.mjs`: shell 180 kB, total 400 kB).
- Documentation matches the code it describes, with numbers re-verified against `wc -l` at update time.
- All findings from the 2026-08-31 review are either fixed or explicitly dispositioned in `Compromises Made`/`Further Actions`.

## Tasks

- [x] Record the baseline: full verification green plus measured timings (DONE 2026-08-31 16:59: green; one wall-clock flake found and documented — see baseline.md; runtime retry 10m27s green)
  - Acceptance Criteria:
    - Functional: Run the complete gate set (`scripts/check.sh`, frontend lint/format/test/build/budget, clay-agent tests) and confirm green before any change.
    - Performance: Record baseline numbers for later comparison: `editor_performance` suite wall time, total `cargo test --all-targets` wall time, and the frontend chunk table (raw and gzip sizes per chunk, from the vite build output and `frontend/scripts/bundle-budget.mjs`).
    - Code Quality: Baseline numbers are stored in the task completion evidence (this plan file's checkboxes stay in repo; numbers go in the commit message or a short `code-reviews/2026-08-31-review-remediation/baseline.md` note).
    - Security: `cargo audit` reports 0 vulnerabilities and 19 allowed warnings; confirm the documented-exception test still passes.
  - Approach:
    - Documentation Reviewed:
      - `scripts/check.sh`: stage list and serial lock.
      - `AGENTS.md`: Linux gate set is blocking.
      - `.agents/skills/project-patterns/references/planning-checklist.md`
    - Options Considered:
      - Skip the baseline and compare only end-state gates: faster, but the performance tasks (suite split, bundle split) need before/after numbers to prove their acceptance criteria.
      - Record baseline first: selected.
    - Chosen Approach:
      - One full gate run with `time` measurements captured per stage; no code changes in this task.
    - API Notes and Examples:
      ```bash
      time scripts/check.sh        # record per-stage durations, especially editor_performance
      ```
    - Files to Create/Edit:
      - `code-reviews/2026-08-31-review-remediation/baseline.md`: measured baseline numbers (this path is gitignored by design; numbers are also recorded in task completion notes).
    - References:
      - 2026-08-31 review: all gates green; `editor_performance` ≈ 656 s; index chunk 517 KiB raw / 165 KiB gzip.
  - Test Cases to Write:
    - None (measurement-only task); gates themselves are the check.
  - Execution Evidence:
    - Baseline recorded at commit `cbdca7c` in `code-reviews/2026-08-31-review-remediation/baseline.md`.
    - Rust: `scripts/check.sh full` — audit 0 vulns / 19 allowed warnings (2.5 s), fmt/check/clippy green, lib tests 1170 passed in 22.42 s; runtime suite flaked once under load (`large_document` head-paint 628.9 ms vs 500 ms budget), passed green on isolated retry in 625.97 s (71 tests); bench compile cached green. The `editor_performance` matrix is one test inside the runtime target and dominates it (slow-test warning).
    - Frontend: lint/fmt green, 194 Vitest tests green, build green (index chunk 517.49 KiB raw / 164.96 gzip), budget green (shell 169.3/180, total 359.6/400).
    - clay-agent: 8/8 pass.
    - Full numbers and chunk table: `code-reviews/2026-08-31-review-remediation/baseline.md`.

- [x] Remove tracked junk files and close the root checklist (review P1-2) (DONE 2026-08-31 17:25: all 11 items deleted; quick gate + protocol suite green; commits 944c7b9, d0f3214)
  - Acceptance Criteria:
    - Functional: `git ls-files` no longer lists any of: `tmp/` (scratch Cargo crate, `main.rs`, `main.ts`, `main.js`, marksman config), `client.log`, `client2.log`, `client3.log`, `client4.log`, `client-hang.log`, `EOF`, `test.md`, `todo.md`, `typescript`, `vendor/`. `test.md`'s content is superseded by `test-plan/` — deleted, not moved. `todo.md` items are all marked done — deleted. If `client-hang.log` documents an unresolved hang, its story is recorded in a decision log first; if it was resolved, it is simply deleted.
    - Performance: No build or test input references any removed file (verify with `rg -l 'client-hang|tmp/src|marksman'` over tracked files).
    - Code Quality: Empty `vendor/` directory removed; `.gitignore` unchanged unless a rule becomes dead (remove only rules that matched nothing after deletion).
    - Security: No logs or scratch files with potentially sensitive content remain tracked in git history going forward (history rewrite is explicitly out of scope — note it as a Further Action option only).
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`: confirms `test-plan/` supersedes the old `test.md` manual plan.
      - `docs/index.md`: confirm nothing links to `test.md`, `todo.md`, or `tmp/`.
    - Options Considered:
      - Relocate `test.md` into `test-plan/` as a historical module: rejected — its steps predate the React client and would be misleading.
      - Plain `git rm`: selected for everything except the `client-hang.log` story check.
    - Chosen Approach:
      - Single commit deleting all listed files; check `client-hang.log`'s referenced hang against current issues/decision logs before deletion; run gates after.
    - API Notes and Examples:
      ```bash
      git rm -r tmp/ vendor/ client.log client2.log client3.log client4.log client-hang.log EOF test.md todo.md typescript
      ```
    - Files to Create/Edit:
      - Deleted: `tmp/**`, `vendor/`, `client.log`, `client2.log`, `client3.log`, `client4.log`, `client-hang.log`, `EOF`, `test.md`, `todo.md`, `typescript`.
    - References:
      - 2026-08-31 review §4 P1-2.
  - Test Cases to Write:
  - Test Cases to Write:
    - `git ls-files | rg '^(tmp/|vendor/|client.*\.log|EOF|test\.md|todo\.md|typescript)'` returns nothing.
  - Execution Evidence:
    - Deletion commit `944c7b9`; plan file committed as `d0f3214`.
    - Dispositions: `client-hang.log` was 0 bytes (no unresolved story to record — plain delete); `todo.md` had 10/10 items checked (closed, not archived); `test.md` superseded by `test-plan/` (deleted); `vendor/` was an untracked empty dir (rmdir);
    - Reference check: all `marksman`/`tmp` pattern matches resolved to the marksman LSP server and `tests/fixtures/lsp/markdown/.marksman.toml` fixtures — no test or build input touched the junk files; `tmp/` was a scratch Cargo crate (Cargo.toml, src/{README,main.js,main.rs,main.ts}, tsconfig.json).
    - Gates after deletion: `scripts/check.sh quick` green (1170 lib tests), `cargo test --test protocol` green (200 tests incl. manual-smoke-documentation and primitive-docs registry checks that parse test-plan/ and docs/).
    - `git ls-files` junk check: clean. `.gitignore` needed no changes (no dead rules).

- [x] Delete the unreferenced native dialog backends and align the wiki (review P1-1) (DONE 2026-08-31 17:50: file deleted, docs/wiki/API docs/api-inventory/registry aligned, decision log written, all suites green)
  - Acceptance Criteria:
    - Functional: `src/client/file_dialog.rs` is deleted; `src/client/mod.rs` re-exports of `FileDialogFilter`, `FileDialogResult`, `markdown_file_dialog_filters`, `open_folder_dialog`, `open_markdown_file_dialog` are removed; `cargo check --all-targets` and `clippy -D warnings` stay green; `docs/wiki/modules/client-file-dialog.md` describes the Tauri-side ashpd portal commands (`src-tauri/src/commands.rs`: `dialog_open_file`, `dialog_open_folder`, `tab_open_dialog`) as the only dialog path, matching what the page already claims was decided in Plan 097 Phase 12.
    - Performance: No dialog code remains in the core `clay` crate; dialog latency is unaffected (unchanged code path).
    - Code Quality: All 11 `unsafe` sites in the file are gone; `rg -n 'unsafe' src/ --include='*.rs'` shows only the narrow, SAFETY-commented `libc`/`env` uses listed in the review. A decision log records the deletion and the platform policy it encodes.
    - Security: Deleting the Windows COM `unsafe` code removes dead, never-compiled-in-CI attack surface; no capability, permission, or protocol change occurs; the grant/capability-token flow for file opens is untouched.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/client-file-dialog.md`: claims COM/portal/NSOpenPanel backends were removed in Plan 097 Phase 12 — code contradicts it.
      - `src-tauri/src/commands.rs:173-250`: the replacement path (ashpd XDG file-chooser portal commands with per-dialog busy locks).
      - `AGENTS.md` platform-validation section: Windows is a long-term target; dead Windows code that CI never compiles is rot, not support.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
    - Options Considered:
      - Keep the backends and fix the wiki to say they exist: rejected — zero callers anywhere in `src/`, `src-tauri/`, or tests; keeping never-compiled `unsafe` code contradicts both the wiki and the security posture.
      - Delete the file and its re-exports: selected — matches the already-documented Plan 097 Phase 12 decision; the wiki needs only a path/reference touch-up, not a rewrite.
    - Chosen Approach:
      - Delete the file and re-exports in one commit; update the wiki page to name `src-tauri/src/commands.rs` as the implementation; write the decision log; fold one durable rule into project patterns (see below).
    - API Notes and Examples:
      ```text
      React client -> invoke("dialog_open_file"/"dialog_open_folder"/"tab_open_dialog")
                  -> ashpd portal -> server grant/capability flow (unchanged)
      ```
    - Files to Create/Edit:
      - Deleted: `src/client/file_dialog.rs`.
      - `src/client/mod.rs`: remove `pub mod file_dialog;` and the re-export lines (L2, L6-L8).
      - `docs/wiki/modules/client-file-dialog.md`: point implementation references at the Tauri commands; remove any description of the deleted Rust backends.
      - `decision-logs/2026-08-31-<time>-delete-dead-native-dialog-backends.md`: decision + platform policy (approved by this plan).
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: fold in the durable rule "wiki pages and code must not contradict each other; platform-gated code that CI never compiles must be either wired to a real caller or deleted."
    - References:
      - 2026-08-31 review §4 P1-1 and §6 (31 `unsafe` hits, 11 in this file).
      - Plan 097 (Tauri/React migration), Phase 12.
  - Test Cases to Write:
    - Existing gates: no test referenced the deleted functions (verified during review); `cargo check --all-targets` green is the proof.
    - `rg -n 'file_dialog|FileDialogResult|open_folder_dialog|open_markdown_file_dialog' src src-tauri tests` returns no live references.
  - Execution Evidence (2026-08-31 17:45):
    - Deleted `src/client/file_dialog.rs` + the `pub mod file_dialog;` declaration and 5-item re-export in `src/client/mod.rs`. `cargo check --all-targets`, clippy `-D warnings`, fmt, and lib tests green immediately; protocol (200), security (134), presentation (40) integration suites green.
    - Doc alignment scope grew beyond the wiki page (doc-pinning tests + the registry single-source rules forced it): rewrote `docs/wiki/modules/client-file-dialog.md` (Tauri-only dialog path, live chain diagram, stale test/source lists corrected); updated `docs/wiki/index.md` summary; fixed platform claims in `docs/development/launch-and-gui-smoke.md` (platform matrix + Phase 19 lists), `docs/development/windows.md`, `docs/development/file-open-save-reload-workflow.md` (2 tables), `docs/reference/clay-js-api/configuration.md` (2 spots); updated both Clay JS API docs (`backing_rust`/`backing Rust/current owner` now cite `src-tauri/src/commands.rs::dialog_open_file` / `::dialog_open_folder`, `src/client/behavior.rs`, server workspace/protocol paths — all existence-tested by `clay_js_api_inventory`), mirrored in `api-inventory.toml` (`backing_rust` + `current_rust_owner`); regenerated `docs/generated/clay-js-api-registry.json`; updated `tests/manual_smoke_docs.rs` wiki markers ("Shell COM APIs"/"FileDialogResult::Selected(PathBuf)" → "dialog_open_file"/"BridgeState::accept_selected_path").
    - Honest platform statement in all touched docs: dialogs are portal-backed on Linux (ashpd) today; Windows/macOS native pickers are pending long-term targets at the same Tauri command seam; platforms without a picker get a sanitized `file dialog failed` diagnostic.
    - 11 `unsafe` uses deleted with the file; remaining `unsafe` in `src/` is the narrow documented libc/env set. `rg 'file_dialog|FileDialogFilter|FileDialogResult|open_folder_dialog|open_markdown_file_dialog' src tests src-tauri/src` → no live references.
    - Decision log: `decision-logs/2026-08-31-1745-delete-dead-native-dialog-backends.md` (platform policy + alternatives). Pattern folded into `.agents/skills/project-patterns/references/maintenance-validation.md` (wiki-code parity; platform-gated code must be wired or deleted).

- [x] Fix the documentation-drift batch (review P1-4) (DONE 2026-08-31 18:20: 4 target docs + 7 same-class dead-path citations; protocol 200/security 134/presentation 40/runtime 71 green)
  - Acceptance Criteria:
    - Functional: Three documents match reality: (1) `docs/development/architecture-ownership.md` cites current module paths (`server/connection/mod.rs` + family modules, `packages/record/mod.rs` + `language.rs`/`ui.rs`, `server/mod.rs`) with line counts re-measured by `wc -l` at update time and a "counts as of 2026-08-31" stamp; (2) `DESIGN.md` (component-kinds diagram and prose) and `docs/development/ui-design-system-recipe-matrix.md` state the real taxonomy: 15 SDUI `ComponentKind`s (`src/shell/components.rs:23`) plus 25 recipe-styled surfaces (button, textInput, dropdown, list, collapse, modal, panel, label, statusItem, flex, stack, overlay, portal, scroll, tab, tabBar, card, badge, kbd, tooltip, popover, menu, commandCentre, chat, editor); (3) `docs/development/security.md` quotes the full CSP from `src-tauri/tauri.conf.json` verbatim and explains why `style-src 'unsafe-inline'` is required (CSS custom-property injection) and why `img-src data:`/`font-src 'self'` exist.
    - Performance: Documentation-only change; no runtime effect.
    - Code Quality: Numbers are generated from `wc -l` commands listed in the doc or commit message so the next refresh is mechanical; the recipe matrix keeps its per-surface rows unchanged.
    - Security: The full CSP being documented (not summarized) lets readers verify there is no `script-src` weakening; no permission/capability text changes.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/architecture-ownership.md`, `DESIGN.md`, `docs/development/ui-design-system-recipe-matrix.md`, `docs/development/security.md`, `src-tauri/tauri.conf.json:24` (CSP string), `src/shell/components.rs:23` (enum).
      - `.agents/skills/project-patterns/references/documentation-as-code.md`, `.agents/skills/project-patterns/references/doc-registry-tests.md`
    - Options Considered:
      - Replace line counts with symbols-only counts: avoids future drift but loses the size signal the review used; rejected.
      - Keep counts + date stamp + measurement command: selected — drift becomes detectable instead of silently wrong.
      - Rewrite DESIGN.md kind taxonomy as "25 kinds": rejected — `ComponentKind` is a protocol enum with 15 members; conflating it with recipe surfaces is exactly the drift being fixed.
    - Chosen Approach:
      - One docs-only commit; every number re-measured immediately before writing it down.
    - API Notes and Examples:
      ```bash
      wc -l src/server/connection/*.rs src/server/mod.rs src/packages/record/*.rs   # paste real numbers
      ```
    - Files to Create/Edit:
      - `docs/development/architecture-ownership.md`: path/table refresh + date stamp.
      - `DESIGN.md`: "15 SDUI component kinds; 25 recipe-styled surfaces" wording in the three flagged places (component-kind diagram note, border-radii prose, "Component Kinds" heading area).
      - `docs/development/ui-design-system-recipe-matrix.md`: same wording fix in "All 25 Supported" heading and intro.
      - `docs/development/security.md`: full CSP + rationale.
    - References:
      - 2026-08-31 review §5 (drift table).
  - Test Cases to Write:
    - Existing: `cargo test --test primitives_docs` and the wiki/doc registry tests stay green (they cross-check catalog/package/doc references).
  - Execution Evidence (2026-08-31 18:20):
    - gates: protocol 200, security 134, presentation 40, runtime 71 (incl. editor_performance, 625s) all green; `update-doc-registry` re-run no-op; `rg` confirms zero remaining `src/server/connection.rs` / `src/server/js_runtime.rs` / `src/packages/record.rs` citations outside the self-describing wiki Plan-090 split notes.
    - `docs/development/architecture-ownership.md`: server-module table now cites the real directory modules with exact `wc -l` counts + a stamped re-measurement command (server/mod.rs 6,681; connection/mod.rs 9,679 + documents 1,567/runtime 1,152/menus 689/workspace 411/tabs 403; js_runtime/mod.rs 1,534 + tests 10,323, validation 1,013, worker 495, evaluation 459, source 271, error 205; packages/record/mod.rs 1,740 + language 1,359/ui 1,299/behavior 805/theme 670/documentation 201; menu_sessions 1,264 + control_center 805). Ownership symbols re-verified live (`cleanup_connection_documents`, `ReadPumpGuard`, `assemble_package_record`, `evaluate_controlled_module`).
    - Taxonomy (15 vs 25): the review's "25 recipe-styled surfaces" number was itself stale. Measured: 35 distinct surfaces in the recipe matrix = 15 `ComponentKind`s (src/shell/components.rs, reserved `table` aside) + 11 Clay-native internal surfaces + 8 chrome primitives. DESIGN.md (diagram + radii prose), docs/reference/ui-design-systems.md (diagram, radii, and the wrong "All 25 Supported" 25-name list — now the frozen enum list + family lists), ui-design-system-conformance.md, ui-design-system-visual-direction.md, and matrix section headings now state the real taxonomy with counts.
    - CSP: `docs/development/security.md` now quotes the full CSP verbatim from src-tauri/tauri.conf.json:24 with per-directive rationale (style-src 'unsafe-inline' = runtime --clay-*/--clay-ds-* custom-property injection via theme/design-system adapters; img-src data: = defensive room for the planned inert icon-slot, no current emitter; font-src 'self' = local font stacks only).
    - Same-class dead-path fixes: `docs/development/tauri-react-primitive-migration.md`, `docs/reference/primitives/audit.md`, `docs/reference/ui-components.md`, `.agents/skills/clay-ui/references/tokens.md`, `.agents/skills/clay-ui/references/components.md`, `docs/development/tauri-react-parity-ledger.json` (6 `current_owner` entries; targeted string replacement to preserve ledger formatting).


- [x] Extract `handle_connection_loop` dispatch arms into the connection family modules (review P1-3) (DONE 2026-08-31 19:45: arms already family-delegated on this branch; finished the residual extraction; lib 1164/protocol 200/security 134/presentation 40/runtime 71 (628.3s) green)
  - Acceptance Criteria:
    - Functional: `src/server/connection/mod.rs:475-1567` `handle_connection_loop` shrinks to a coordinator (target ≤ 350 lines) that routes each `ClientMessage` family to handlers in the existing `connection/documents.rs`, `connection/menus.rs`, `connection/runtime.rs`, `connection/tabs.rs`, `connection/workspace.rs`; behavior is identical — every existing test in `connection/mod.rs`'s test module and the integration suites passes with unchanged names, and no test is weakened or skipped.
    - Performance: Connection-loop work per message is unchanged (pure code motion — no new allocation, locking, or channel on the hot path); the `editor_performance` suite timings do not regress beyond noise.
    - Code Quality: Family modules own their dispatch arms and errors; `mod.rs` keeps shared state types (`RuntimeDiagnosticStore`, `PendingViewportPatch`, capability pool) and the loop skeleton; no `pub` visibility expansion — extracted handlers are `pub(crate)` or private.
    - Security: No new authority: capability tokens, region locks, tab binding conflict checks, and unknown-session diagnostics move verbatim; no trust boundary changes.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection/mod.rs` (skeleton L317-L474: `handle_connection`, `handle_connection_with_analysis`; loop L475-L1567; helpers L137-L313).
      - Family modules `src/server/connection/{documents,menus,runtime,tabs,workspace}.rs`: current helper placement.
      - `docs/development/architecture-ownership.md`: cleanup-exactly-once authority table (updated by the previous task).
      - `.agents/skills/project-patterns/references/tauri-react-client.md`: server authority boundaries.
    - Options Considered:
      - Extract a generic `match` dispatcher table: rejected — indirection without a second consumer; plain functions per family are the boring option.
      - Move arms into family modules as plain functions taking the existing shared state: selected — the 2026-08-14 review's P2-1 already created the family modules; this finishes that move.
    - Chosen Approach:
      - One family per commit (documents → menus → runtime → tabs → workspace), running `cargo test --lib` plus the protocol/connection-related integration suites after each move.
    - API Notes and Examples:
      ```rust
      // connection/documents.rs
      pub(crate) async fn handle_document_message<S: AsyncRead + AsyncWrite + Unpin>(
          state: &mut LoopState<'_>, message: DocumentMessage,
      ) -> Result<LoopOutcome, ServerError> { /* moved arm body */ }
      ```
      (exact shape follows the existing helper signatures in the family modules; `LoopState` is an example name for the existing shared-state parameter set the loop already threads.)
    - Files to Create/Edit:
      - `src/server/connection/mod.rs`: loop becomes coordinator; shared helpers stay.
      - `src/server/connection/documents.rs`, `menus.rs`, `runtime.rs`, `tabs.rs`, `workspace.rs`: receive their dispatch arms.
      - `docs/development/architecture-ownership.md`: update the connection row's counts in the same commit.
    - References:
      - 2026-08-31 review §4 P1-3; plans/097 (Phase 12 family-module groundwork).
  - Test Cases to Write:
    - No new tests required: the existing ~7,900 lines of connection tests are the behavior-parity harness; a green run with zero test-name changes is the acceptance evidence. Add one test only if a moved arm loses a `pub(crate)`-only invariant that was previously exercised implicitly.
  - Execution Evidence (2026-08-31 19:45):
    - Discovery that reframes the task: the dispatch-arm move was already ~95% complete on this branch before the task started. Every `ClientMessage` arm already routed to the family modules (`documents::dispatch_edit_operation`/`handle_*`, `workspace::handle_open_selected_file`/`handle_add_selected_workspace_root`, `tabs::handle_tab_command`, `menus::handle_menu_*`, `runtime::handle_sdui_action`/`handle_command_intent`/completion/intelligence/installed, L1013-1594 pre-move). The review's P1-3 numbers predated the Plan 090 family-module groundwork the plan text itself credits ("P2-1 already created the family modules; this finishes that move"); the seam was already finished. No test-name changes, zero test edits (`git diff` of tests empty) — the parity harness never moved.
    - Residual extraction executed (pure code motion, verbatim bodies, review 2026-08-31 P1-3 completion):
      1. `documents.rs`: new `PendingViewportPatch`-aware `deliver_parse_update` (the select loop's parse-updates lane body, ~75 lines incl. chunk batching + `finalize_viewport_covered_ranges`) and `track_pending_viewport_request` (the `ViewportRenderRequest` arm's latest-wins bookkeeping). Both `pub(super)`, matching the family convention — no visibility widening; `PendingViewportPatch` stays in `mod.rs` per the plan (verified: only loop-internal users, zero test references).
      2. `menus.rs`: `write_active_menu_session_closed` (the runtime-state lane's generation-cancelled menu write).
      3. `mod.rs`: the parse lane, viewport-bookkeeping block, and state-lane menu write replaced by family calls; dropped the `finalize_viewport_covered_ranges` copy (moved verbatim) and now-unused imports.
    - Resulting metrics: `handle_connection_loop` shrank 1,120 → 1,009 lines; `connection/mod.rs` 9,679 → 9,565; `documents.rs` 1,567 → 1,732, `menus.rs` 689 → 708. `architecture-ownership.md` connection row refreshed in the same commit.
    - On the ≤350-line coordinator target: not reachable without violating the plan's own constraints. The residual ~1,000 lines are (a) the `tokio::select!` transport skeleton (~470 lines of broadcast lanes the plan's Code Quality AC explicitly keeps in mod.rs), (b) the welcome/capability handshake (~190), (c) the routing match of thin family calls (~250), and (d) the trust prelude (identity gate/metrics). Compressing (a) needs a generic channel-dispatch table — the option the plan rejected ("indirection without a second consumer"); a context bag is likewise rejected by the loop's own `allow` reason ("state handles explicitly instead of hiding authority in a context bag"). Recorded as a compromise: the AC's substance (family modules own dispatch arms and errors) is satisfied by construction; the line target was extrapolated from pre-extraction numbers.
    - Gates: fmt, clippy `-D warnings` (one `#[allow(clippy::too_many_arguments)]` restored that the insertion had separated from its fn), lib 1,164 pass/1 ignored, protocol 200, security 134, presentation 40, runtime 71 pass in 628.3 s (baseline 625.4-626.0 s — +2.4 s noise; includes the `editor_performance` matrix; no hot-path change: viewport bookkeeping and parse-lane delivery are the same instructions, same locks, same channel usage).

- [x] Move giant inline test modules to sibling test files (review P2-1) (DONE 2026-08-31 21:05: four module moves to sibling tests.rs + three file->dir conversions; full suite green; drift-guard pins realigned)
  - Acceptance Criteria:
    - Functional: The six flagged files stop carrying thousands of lines of inline test code, following the existing `src/server/js_runtime/mod.rs:1534` (`mod tests;` + `src/server/js_runtime/tests.rs`) precedent: `src/server/connection/mod.rs` (tests ≈ L1786-L9679), `src/server/mod.rs` (6,681 lines), `src/client/mod.rs` (5,734; test mod from ≈ L1997), `src/server/workspace.rs` (5,724; tests from ≈ L2,036), `src/server/syntax.rs` (3,414), `src/shell/layout.rs` (3,395; test mods from L67). Plain-file modules (`workspace.rs`, `syntax.rs`, `layout.rs`, `client/mod.rs` is already a dir) are converted to directory modules (`workspace/mod.rs` + `workspace/tests.rs`) so the import path `crate::server::workspace` is unchanged; test code is moved verbatim (no test logic edits).
    - Performance: No runtime change — `#[cfg(test)]` code motion only; compile time for the test profile should not regress (record `cargo check --all-targets` time before/after).
    - Code Quality: Every moved test still compiles against private items via `use super::*`; zero test renames, zero `#[ignore]` additions; `git diff` of moved blocks is near-pure motion (whitespace/import fixes only).
    - Security: Test-only code motion; no production code touched except module declarations.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/mod.rs:1534` and `src/server/js_runtime/tests.rs`: the in-repo precedent (10,323-line sibling test file).
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
    - Options Considered:
      - `#[path = "workspace_tests.rs"]` attribute files: avoids dir conversion but invents a second convention; rejected — one convention already works.
      - Directory-module conversion with verbatim test moves: selected — matches `js_runtime` and `connection` structure.
    - Chosen Approach:
      - One file per commit, gates green after each; largest first (`connection/mod.rs`, then `server/mod.rs`, `client/mod.rs`, `workspace.rs`, `syntax.rs`, `layout.rs`).
    - API Notes and Examples:
      ```text
      src/server/workspace.rs        -> src/server/workspace/mod.rs + src/server/workspace/tests.rs
      mod.rs gains: #[cfg(test)] mod tests;   // tests.rs starts with: use super::*;
      ```
    - Files to Create/Edit:
      - `src/server/connection/mod.rs` → tests into `src/server/connection/tests.rs`.
      - `src/server/mod.rs` → `src/server/mod.rs` keeps prod; tests into `src/server/tests.rs` (module path `crate::server::tests` — same `use super::*` reach).
      - `src/client/mod.rs` → `src/client/tests.rs`.
      - `src/server/workspace.rs` → `src/server/workspace/mod.rs` + `tests.rs`.
      - `src/server/syntax.rs` → `src/server/syntax/mod.rs` + `tests.rs`.
      - `src/shell/layout.rs` → `src/shell/layout/mod.rs` + `tests.rs`.
      - `docs/development/architecture-ownership.md`: refresh counts in the same commits.
    - References:
      - 2026-08-31 review §4 P2-1 table.
  - Test Cases to Write:
    - None new; acceptance is: same test count, same names, all green after each file move (`cargo test --lib` per commit, full suite at the end).
    - Execution Evidence (2026-08-31 21:05):
      - Moves (verbatim; dedent attempt reverted after it broke one `{`/`}` pair — verbatim + `cargo fmt` is the safe transform):
        - `connection/mod.rs` 9,565→1,672 prod + `connection/tests.rs` 7,892 (515d0e9)
        - `server/mod.rs` 6,681→5,838 + `server/tests.rs` 769 — the file has TWO top-level test modules; the tiny `#[cfg(all(test, windows))] mod windows_tests` (71 lines) stays inline (not a giant module) (d591424)
        - `client/mod.rs` 5,729→1,991 + `client/tests.rs` 3,735 (e95554f)
        - Plain files → directory modules per the js_runtime precedent so `crate::server::workspace` / `crate::server::syntax` / `crate::shell::layout` paths are unchanged: `workspace.rs`→`workspace/mod.rs` 3,197 + `tests.rs` 2,524; `syntax.rs`→`syntax/mod.rs` 2,495 + `tests.rs` 916 (its `include_str!("../../packages/...")` query paths bumped to `../../../` for the new nesting depth); `layout.rs`→`layout/mod.rs` 1,819 + `tests.rs` 1,573 (3ab5ff4)
      - Test counts identical (1,164 lib, same names; `mod tests` keeps module path so existing `server::tests` etc. references hold — none exist cross-module, verified).
      - Drift-guard follow-up (1b4ec68): 107 doc/test files pinned the old file paths (`src/server/workspace.rs` etc.) — mechanically realigned to `*/mod.rs`; `tests/performance_budgets.rs::production_body` learned the sibling-`tests.rs` convention (it truncated at an in-source `mod tests` marker that no longer exists for pure test files; the guard now skips files named `tests.rs`).
      - Performance: `cargo check --all-targets` 10.1 s → 10.0 s warm (code motion only); full suite runtime leg 624 s (baseline 625-628 s).
      - Gates: fmt, clippy `-D warnings`, lib 1,164, protocol 200, runtime 71, security 134, presentation 40 — all green after the full pass.

- [x] Split the `workspace-controller.ts` monolith (review P2-2) (DONE 2026-08-31 21:25: envelope + command dispatch extracted to sibling modules with context seams; 962→589-line controller; full frontend gate green)
  - Acceptance Criteria:
    - Functional: `frontend/src/shell/workspace-controller.ts` `createWorkspace` (L101-L926) keeps its public object shape and behavior; `handleEnvelope` (L465-L717) and `dispatchClientCommand` (L253-L372) move into focused modules under `frontend/src/shell/` with explicit parameter types; `workspace-controller.test.ts` and all dependent tests pass unchanged.
    - Performance: No per-message allocation or re-render changes; envelope handling stays O(1) per event with the same notify/persist scheduling; the frontend test suite and build stay green with no bundle regressions (chunks recorded).
    - Code Quality: No UI-surface change (pure code motion + typed seams); no `any`, no non-null assertions, `noUncheckedIndexedAccess` clean; extracted modules export only what the controller passes in (no new global state).
    - Security: No Tauri invoke surface changes; envelope handling keeps trusting only the typed bridge; no new IPC channels.
  - Approach:
    - Documentation Reviewed:
      - `frontend/src/shell/workspace-controller.ts` (skeleton: L61-L99 adapters/records, L101 factory, L465-L717 handleEnvelope, L253-L372 dispatchClientCommand).
      - `frontend/src/shell/workspace-controller.test.ts`, `tab-store.test.ts`, `split-tree.test.ts`: behavior harness.
      - `.agents/skills/project-patterns/references/tauri-react-client.md`: React owns presentation; server stays authoritative.
    - Options Considered:
      - Rewrite as a class or state-machine library: rejected — churn without a second consumer; the closure factory works.
      - Extract the two long functions into sibling modules with explicit seams: selected — halves the file while keeping the existing API.
    - Chosen Approach:
      - Two extractions in separate commits: envelope handling first (largest), then command dispatch; controller keeps sole ownership of tabs/panes state.
    - API Notes and Examples:
      ```ts
      // frontend/src/shell/workspace-envelope.ts
      export interface EnvelopeContext { /* runtime, panes, notify, ... */ }
      export function handleEnvelope(ctx: EnvelopeContext, envelope: BridgeEnvelope): void;
      ```
    - Files to Create/Edit:
      - `frontend/src/shell/workspace-envelope.ts`: extracted envelope handling.
      - `frontend/src/shell/workspace-commands.ts`: extracted command dispatch.
      - `frontend/src/shell/workspace-controller.ts`: reduced factory wiring the seams.
      - `docs/wiki/modules/react-shell.md`: note the module split (also covered by the final wiki task).
    - References:
      - 2026-08-31 review §4 P2-2.
  - Test Cases to Write:
    - Existing tests are the parity harness; add one Vitest case only if an extracted seam drops previously-implicit coverage of persist scheduling (verify `workspace-controller.test.ts` covers restore/persist before extraction).
    - Execution Evidence (2026-08-31 21:25, commit dd28750):
      - `frontend/src/shell/workspace-envelope.ts` (313 lines): `handleEnvelope` + `eventDocumentId` (which is pure envelope routing; `runtimeDocuments` stayed in the controller where `serialize` uses it). `EnvelopeContext` seam: adapters, runtimes map, tabs store, registryRootsByClient, notify, deliverRootId, ensurePane, dispatchClientCommand callback. Body moved verbatim via context destructure — zero text edits.
      - `frontend/src/shell/workspace-commands.ts` (166 lines): `dispatchClientCommand` + `sendTabCommand`. `CommandContext` seam: adapters, tabs, notify, setTree, mountRuntime. Only mechanical renames: closure names → `ctx.` members.
      - `workspace-controller.ts` 962 → 589. Both contexts built ONCE inside `createWorkspace` (no per-message allocation — Performance AC), `handleEnvelope`/`openFileDialog`/`openFolderDialog` become thin wrappers. Same object references the closure already held: no new global state, no Tauri IPC surface change.
      - Deviation from the plan's "two separate commits": both extractions share one seam (envelopeContext wraps the commands dispatch callback), so they cannot land as two independent green commits; documented in the commit message. The two-module split itself is exactly as planned.
      - Gates: `tsc --noEmit`, eslint, prettier, vitest 194 pass (28 files, incl. 15 workspace-controller cases — restore/persist scheduling covered), `npm run build` ok. Bundle: shell 169.8 kB gzip (budget 180), total 360.0 kB (400) — baseline 169.3/359.6, +0.5 kB is module-seam scaffolding; chunks recorded in commit.

- [x] Split the `editor_performance` suite so it parallelizes (review P2-3) (DONE 2026-08-31 22:10, commit 84ed71a: 656→49.3 s matrix / 626→49.4 s suite)
  - Acceptance Criteria:
    - Functional: `tests/editor_performance.rs` keeps every matrix cell covered; the single 656-second `editor_performance_matrix_holds_deterministic_invariants` test (L108) is split into per-size-class tests that `cargo test` runs in parallel, or the 50 MiB leg is moved behind an explicit env-gated larger-matrix test with the policy recorded in a decision log and `test-plan/` ceilings — the split-with-full-coverage option is preferred and must be tried first.
    - Performance: Suite wall time ≤ 240 s on the development machine (baseline ≈ 656 s); every invariant assertion from the current single test still executes against every cell it covered.
    - Code Quality: Fixture generation is shared (no duplicated matrix definitions); per-test server setup stays deterministic; the module keeps its "one server mirrors a real session" property within each size class.
    - Security: No fixture or approved-root change; temp-dir discipline unchanged.
  - Approach:
    - Documentation Reviewed:
      - `tests/editor_performance.rs` (551 lines; matrix L95-L105; single test L108; fixture generation comment "The whole matrix runs in one test against one server").
      - `docs/development/editor-performance-review-2026-08-26.md`, `docs/development/performance.md`, `src/perf/budgets.rs`.
      - `scripts/check.sh` `test` stage (runs `cargo test --all-targets`).
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
    - Options Considered:
      - One test, fixtures generated concurrently: helps startup but the serial per-cell server work dominates; insufficient alone.
      - Per-size-class tests sharing helpers (parallel across `cargo test` threads): selected — mechanical, keeps coverage, no policy change.
      - Env-gate the 50 MiB leg (nightly-only): only if the parallel split cannot reach ≤ 240 s; requires decision-log + `test-plan/` ceiling entry and explicit approval.
    - Chosen Approach:
      - Split by size class (e.g., small/medium/large/huge) into separate `#[tokio::test]`s over the same shared matrix/fixture helpers; measure before/after.
    - API Notes and Examples:
      ```rust
      #[tokio::test] async fn editor_performance_small_cells_hold_invariants() { run_matrix_cells(SizeClass::Small).await }
      // shared: fn run_matrix_cells(class: SizeClass) — same assertions, subset of matrix()
      ```
    - Files to Create/Edit:
      - `tests/editor_performance.rs`: split tests, shared helpers.
      - `code-reviews/2026-08-31-review-remediation/` note or commit message: before/after wall times.
      - `decision-logs/` + `test-plan/index.md`: only if the 50 MiB gating fallback becomes necessary.
    - References:
      - 2026-08-31 review §4 P2-3; baseline timings from task 1.
      - Baseline correction discovered during task 1: the matrix runs inside the `runtime` target (`tests/suites/runtime.rs`, 71 tests, ≈ 626 s) — the suite-parallelization effort targets that target's composition, and `tests/editor_performance.rs` helpers are shared with it.
      - Baseline flake: `large_document::large_document_open_edit_save_reload_roundtrip_is_chunked` failed once under concurrent load (628.9 ms vs 500 ms head-paint budget), passed on retry — deterministic-assertion candidate to consider in the same task.
  - Test Cases to Write:
    - The split tests themselves are the deliverable; acceptance is measured wall time with identical cell coverage (list the matrix cells covered per test in a comment).
  - Execution Evidence (2026-08-31 22:10, commit 84ed71a):
    - Timing: matrix tests 482-656 s (serial) -> 49.3 s (3 tests parallel); full runtime target 626 s -> 49.4 s (73 tests: 71-1+3). AC wall ≤ 240 s met with 5x headroom. Per-class solo walls (post-fix): small 58 s (24 cells), medium 14 s (5), large 22 s (2) — measured parallel: max ≈ 49 s.
    - Split: one test -> `editor_performance_{small,medium,large}_cells_hold_invariants`; shared `cells_for(SizeClass)` filter (no duplicated matrix definitions) + `run_matrix_cells` runner; each class keeps its own server ("one server mirrors a real session" per class). Cell lists per test in comments (24/5/2; 31 cells total).
    - Root cause discovered via phase instrumentation: the 10 s flat read-gap in `wait_exactly_one_patch` was the wall-time dominator — patches always arrive at t=0 (trace showed t=0.000), so each cell burned 2 x 10 s of idle quiet-window waiting (20 s/cell, ~0 CPU). The probe is now two-phase: 30 s gap BEFORE the patch (loaded-CI tolerance, strictly more permissive than the old 10 s) and 1 s idle gap after it to trip on late duplicates (server delivers one patch per request id structurally; the gap is a tripwire, not a protocol deadline). All invariant assertions unchanged and still execute per cell.
    - Not a documented test-plan ceiling (verified test-plan/index.md); no decision-log entry needed. The 50 MiB leg stays in the always-on large class with full coverage.
    - Baseline flake note from the plan (`large_document` head-paint 628.9 ms vs 500 ms): NOT addressed here — it is a large_document.rs budget concern, not the matrix; reconsidered and left for a future task (plan P2-3 scope was suite parallelism).
    - Doc drift fixed: build-and-test.md Plan 099 section rewritten; wiki citations in frontend-edit-synchronization, parse-task-lifecycle, parse-coordinator, desktop-typed-bridge, performance-fixtures repointed ('cargo test --test runtime editor_performance_'); primitives_docs guard pin updated to the three new test names (caught the stale doc first, as designed).
    - Gates: fmt/clippy/check clean; lib 1164/1 ignored, protocol 200 (incl. primitives_docs + documentation_coverage), runtime 73 @ 49.4 s, security 134, presentation 40. graft index refreshed.

- [ ] Code-split the frontend index chunk below the 500 KiB warning (review P2-4)
  - Acceptance Criteria:
    - Functional: `frontend/vite.config.ts` gains a `manualChunks` (or equivalent rollup) mapping so `@codemirror/*` and `react-aria-components` leave the index chunk into lazy-or-parallel chunks; the app boots identically (router lazy routes already exist) and all Vitest suites pass.
    - Performance: The main index chunk drops below 500 KiB raw (clearing the Vite warning; baseline 517 KiB raw / 165 KiB gzip); gzip totals keep passing `frontend/scripts/bundle-budget.mjs` (shell ≤ 180 kB, total ≤ 400 kB) with improved headroom; record the full before/after chunk table; no new waterfall on first paint (startup chunk count and load order recorded).
    - Code Quality: Build config change only — no component code changes; chunk names are stable and readable.
    - Security: No CSP change; all chunks still load from `'self'`; no remote origins.
  - Approach:
    - Documentation Reviewed:
      - `frontend/vite.config.ts`, `frontend/scripts/bundle-budget.mjs` (SHELL_BUDGET_GZIP_KB = 180, TOTAL_BUDGET_GZIP_KB = 400).
      - `.agents/skills/project-patterns/references/tauri-react-client.md` and `planning-checklist.md` ("code-split heavy web renderers", explicit bundle budgets).
      - `docs/development/performance.md`: budget history.
    - Options Considered:
      - Raise the Vite warning threshold: hides the problem; rejected.
      - `manualChunks` vendor split for the two heavy libraries: selected — standard, boring, measurable.
      - Convert more routes to lazy imports: already done (`router.tsx` lazy fixtures); marginal.
    - Chosen Approach:
      - `manualChunks` entry for `@codemirror` and `react-aria-components` groups; verify lazy editor import still defers editor chunk; measure.
    - API Notes and Examples:
      ```ts
      // vite.config.ts — build.rollupOptions.output.manualChunks
      manualChunks(id) {
        if (id.includes("@codemirror/")) return "codemirror";
        if (id.includes("react-aria-components")) return "aria";
      }
      ```
    - Files to Create/Edit:
      - `frontend/vite.config.ts`: manualChunks.
      - `docs/development/performance.md`: new chunk table + date.
    - References:
      - 2026-08-31 review §4 P2-4; baseline chunk table from task 1.
  - Test Cases to Write:
    - Existing bundle-budget gate (`frontend/scripts/bundle-budget.mjs` via the frontend check) plus recorded chunk table; Vitest suites green.

- [ ] Add or verify the `runtime/js` `.d.ts` drift check (review P3)
  - Acceptance Criteria:
    - Functional: Either an existing check already fails when a `runtime/js/*.d.ts` declaration drifts from its adjacent `.js` implementation, or a minimal deterministic check is added (extend `tests/clay_js_facade_layout.rs`, which already inventories facade files, or add a script comparing exported names per `.js`/`.d.ts` pair); `cargo test` fails on a deliberately introduced drift (verified by a temporary local mutation), then passes.
    - Performance: Check is static (no V8 runtime); adds ≤ a few seconds to the test suite.
    - Code Quality: One check, no new dependency; error message names the drifted pair.
    - Security: Read-only static analysis of checked-in files; no runtime surface change.
  - Approach:
    - Documentation Reviewed:
      - `tests/clay_js_facade_layout.rs` (existing facade inventory test), `tests/primitives_docs.rs:522` (facades.rs inventory).
      - `runtime/js/` structure: 46 hand-paired `.js`/`.d.ts` facade files.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `.agents/skills/project-patterns/references/maintenance-validation.md`
    - Options Considered:
      - Full `tsc` typecheck of `.js` against `.d.ts` in CI: strongest but adds a Node toolchain dependency to the Rust test stage; reject unless trivially available.
      - Export-name parity check per pair inside the existing Rust facade test: selected — same rigor for drift detection (names/shape), zero new dependencies.
    - Chosen Approach:
      - Extend `tests/clay_js_facade_layout.rs`: for each `runtime/js/*.js`, the adjacent `.d.ts` must declare every exported name (parsed with the same lightweight heuristics the file already uses for facade inventories).
    - API Notes and Examples:
      ```text
      runtime/js/editor.js exports { clientUndo, ... }
      runtime/js/editor.d.ts must declare: clientUndo, ...
      test failure: "runtime/js/editor.d.ts missing export 'clientUndo' present in editor.js"
      ```
    - Files to Create/Edit:
      - `tests/clay_js_facade_layout.rs`: pair-parity check (or a new small `tests/js_facade_types.rs` if cleaner).
    - References:
      - 2026-08-31 review §4 P3 (first item).
  - Test Cases to Write:
      - Drift detection: temporarily rename one export in a `.d.ts`, confirm the test fails naming the pair, revert.

- [ ] Audit `std::sync::Mutex` guards held across `.await` (review P3)
  - Acceptance Criteria:
    - Functional: Every `std::sync::Mutex` used on async paths (`OutputRouter` stores, `RuntimeDiagnosticStore` in `src/server/connection/mod.rs:63-103`, `WorkspaceState`/`DocumentState`/`StaticSduiState` handles threaded through connection code) is audited: each lock acquisition site is listed with whether its guard can be held across an `.await` (including held-across-`?`/early-return paths); violations are fixed by shrinking scopes or moving to blocking-safe patterns, or documented as safe with the reason (e.g., guard dropped before any `.await`).
    - Performance: No lock added or widened; any scope-shrinking fixes must not add measurable regression to the connection suites.
    - Code Quality: Findings recorded in a short `docs/development/` note or wiki section (per-audit table: site, held-across-await?, action); clippy stays green.
    - Security: No authority change; this audit protects against priority-inversion/hang class bugs only.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection/mod.rs:63-103` (`RuntimeDiagnosticStore`), `src/server/output_router*.rs` (`OutputRouter`), `src/server/mod.rs` state handles.
      - `.agents/skills/project-patterns/references/rust-async-patterns.md` (project-local async skill, if present under `.agents/skills/`) and the `rust-async-patterns` skill.
    - Options Considered:
      - Switch all std mutexes to `tokio::sync::Mutex`: rejected — most are short critical sections where std locks are correct and faster.
      - Manual audit of acquisition sites on async paths + document: selected — the review found no symptom; this is verification, not re-architecture.
    - Chosen Approach:
      - Enumerate `lock()`/`.lock().unwrap()` sites reachable from `handle_connection*` async fns; for each, trace guard lifetime to next `.await`; fix or annotate.
    - API Notes and Examples:
      ```text
      audit row: connection/mod.rs:NN  guard held? no — dropped before message await   [safe]
      ```
    - Files to Create/Edit:
      - `docs/development/std-mutex-await-audit-2026-08-31.md`: audit table (or a wiki page under `docs/wiki/development/` if that is the established location for such notes — check `docs/wiki/index.md`).
      - Violation fixes: in-place scope shrinks only, if found.
    - References:
      - 2026-08-31 review §4 P3 (second item).
  - Test Cases to Write:
    - None new unless a violation is found and fixed; then the existing connection/stress suites plus one targeted regression test reproducing the fixed hang pattern if feasible.

- [ ] Sweep production-path `unwrap`/`expect` in the refactored files (review P3)
  - Acceptance Criteria:
    - Functional: In the six files split by the sibling-tests task (now prod modules), every remaining `unwrap`/`expect` in non-test code is either (a) provably unreachable invariant with a comment naming the invariant, or (b) replaced by typed error handling/diagnostics where the value can genuinely be absent; the connection loop and ops modules get first pass, `protocol/mod.rs` (already exemplary at 8 in 3,038 lines) is untouched unless a stray is found.
    - Performance: No hot-path allocation added; error paths replace panics only.
    - Code Quality: `rg -c 'unwrap\(\)' <file>` on prod ranges is annotated in the task evidence; no clippy regressions; no test-code changes (test `unwrap`s stay).
    - Security: Panic-on-malformed-input class bugs are the target; malformed-input paths already return typed errors — verify, don't rewrite.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md` (typed error handling discipline).
      - Review data: `ops/mod.rs` 138 hits dominated by test code after L397; `protocol/mod.rs` exemplary.
    - Options Considered:
      - Global `#![deny(clippy::unwrap_used)]` with test allowances: broad churn across every module; rejected for this plan (note as Further Action if wanted).
      - Targeted sweep of the files this plan already touches: selected — smallest diff, same safety gain where it matters.
    - Chosen Approach:
      - Per-file pass during/after the test-split commits; only genuinely reachable panics change behavior (error instead of panic) — each such site gets a one-line rationale in the commit message.
    - API Notes and Examples:
      ```rust
      // before: let guard = state.lock().unwrap();
      // after (if poisoned lock is possible): map to typed ServerError with diagnostic
      ```
    - Files to Create/Edit:
      - `src/server/connection/mod.rs`, `src/server/mod.rs`, `src/server/workspace/mod.rs`, `src/server/syntax/mod.rs`, `src/shell/layout/mod.rs`, `src/client/mod.rs`: targeted prod-path unwrap fixes only where reachable.
    - References:
      - 2026-08-31 review §4 P3 (third item).
  - Test Cases to Write:
    - One test per converted reachable site (feed the absent/invalid input, assert the typed error), only where an existing test doesn't already cover the error path.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory every Rust `pub` function removed or visibility-changed by this plan (the deleted `src/client/file_dialog.rs` exports and any `pub` items touched by the connection extraction); confirm none is a Clay JS API surface (no `deno_core` op, no JS facade, no `api-inventory.toml` entry, no docs page); confirm every remaining public programmatic surface is unchanged. Functions the extraction made internal are `pub(crate)`, per the boundary rule.
    - Performance: No runtime surface change; `cargo run --bin update-doc-registry` (if docs artifacts changed) and registry tests stay green.
    - Code Quality: `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`, `tests/primitives_docs.rs` all green without edits (edits only allowed for the new d.ts parity check from its own task).
    - Security: No op, capability, or permission surface changed; deletion of the dialog backends removes no JS-reachable function (verified by the inventory).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`, `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `tests/clay_js_doc_registry.rs`, `api-inventory.toml` (if present at repo root or under `docs/`).
    - Options Considered:
      - Expose dialog functions as JS APIs "for completeness": rejected — no user need; they are Tauri-bridge concerns now.
      - Verification-only task with inventory evidence: selected — the plan adds no new public programmatic behavior.
    - Chosen Approach:
      - Write the removed/changed `pub` inventory in the task completion evidence; run the three registry/facade tests; state explicitly that `examples/init.js` and configuration surfaces are unchanged (no new configuration APIs exist to document).
    - API Notes and Examples:
      ```text
      removed pub (non-JS): clay::client::file_dialog::{open_folder_dialog, open_markdown_file_dialog,
        FileDialogResult, FileDialogFilter, markdown_file_dialog_filters} — no op/facade/docs references
      ```
    - Files to Create/Edit:
      - None expected; edit only if the inventory finds a stale docs/registry reference.
    - References:
      - `.agents/skills/create-plan/references/clay.md` (Clay JS API task requirements).
  - Test Cases to Write:
    - None new; the existing registry/coverage tests are the gate.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: This plan changes no user-visible behavior by design (internal refactors, build config, docs). Record that explicitly in `test-plan/index.md` coverage notes for this period. Run the smoke-level modules that could catch an accidental regression from the structural work — `test-plan/01-launch-and-connection.md` (app launch, tab open via dialog path), `04-core-editing.md` (typing on a large file), and the workspace/split steps — on a real Linux build; record pass/fail against the numbered steps.
    - Performance: Add no manual steps; existing timings unchanged.
    - Code Quality: If any task introduced new user-visible behavior despite the design intent (e.g., chunk split changed startup behavior), add numbered steps for it in the affected module file instead of recording it only in chat.
    - Security: No manual security steps; security posture is unchanged (verified by gates).
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map + coverage matrix), `test-plan/01-launch-and-connection.md`, `test-plan/04-core-editing.md`.
      - `.agents/skills/create-plan/references/clay.md` (manual test plan task requirements).
    - Options Considered:
      - Skip manual testing as "pure refactor": the clay.md rule allows recording an exception, but the dialog-path deletion and chunk split touch launch/edit flows users exercise — a smoke pass is cheap and catches wiring mistakes.
      - Smoke pass on affected modules + explicit exception record: selected.
    - Chosen Approach:
      - Build the real app (`scripts/` entry per `test-plan/01`), run the three modules' steps, record results in the module files' results section per the established format.
    - API Notes and Examples:
      ```text
      01-launch-and-connection.md: steps 1..N — pass (2026-08-31 remediation smoke)
      ```
    - Files to Create/Edit:
      - `test-plan/01-launch-and-connection.md`, `test-plan/04-core-editing.md`: results recorded per existing format.
      - `test-plan/index.md`: coverage note for this plan (internal refactor; smoke modules run).
    - References:
      - `.agents/skills/create-plan/references/clay.md` decision source: user instruction 2026-08-04.
  - Test Cases to Write:
    - No new steps unless user-visible behavior emerged; the smoke pass itself is the check.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks complete: `docs/wiki/modules/client-file-dialog.md` (dialog path now Tauri-only), connection/server/workspace/syntax/layout/client module pages (family dispatch split, sibling test files), `docs/wiki/modules/react-shell.md` (controller split), and any page referencing deleted files or old module paths; every wiki page stays linked from `docs/wiki/index.md`.
    - Performance: Wiki updates add no runtime work; they document the performance-relevant changes this plan made (suite timings, chunk table references).
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, and examples where useful; the wiki index links every page.
    - Security: `client-file-dialog.md` and any touched pages document trust boundaries accurately (portal commands → server grant flow); no secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: workflow and quality bar.
      - `docs/wiki/index.md`, affected `docs/wiki/modules/*.md` pages.
    - Options Considered:
      - Update after each task: noisy, churns while subsequent tasks move the same files.
      - Update once after implementation and verification pass: selected.
    - Chosen Approach:
      - Single wiki pass at the end; then run `graft build` to refresh the repo context graph after the large code motion (AGENTS.md graft section: refresh after big code changes).
    - API Notes and Examples:
      ```bash
      graft build   # refresh graft/ graph after module moves
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: navigation updates.
      - `docs/wiki/modules/client-file-dialog.md`, `react-shell.md`, plus connection/server/workspace/syntax/layout/client module pages.
      - `graft/`: regenerated graph (gitignored artifact).
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`
  - Test Cases to Write:
    - Manual wiki review: master index links every page; updated pages match the code they describe (spot-check file paths and line-count claims).

- [ ] Final verification: full gate set green with recorded deltas
  - Acceptance Criteria:
    - Functional: Complete `scripts/check.sh` (fmt, check, clippy, all-targets tests, bench compile, audit), frontend lint/format/test/build/bundle-budget, and clay-agent tests all green on Linux.
    - Performance: Recorded deltas against task 1 baseline: `editor_performance` suite wall time (target ≤ 240 s), total test wall time, index chunk raw/gzip (target < 500 KiB raw), bundle budget headroom.
    - Code Quality: `git ls-files` junk check clean; no TODO/FIXME introduced by this plan; `graft build` output committed-or-refreshed per its ignore rules.
    - Security: `cargo audit` still 0 vulnerabilities; documented-exception test green; `rg 'unsafe' src/` count reduced by exactly the 11 deleted dialog sites (plus/minus any audit fixes, each explained).
  - Approach:
    - Documentation Reviewed:
      - `scripts/check.sh`, `AGENTS.md` (blocking Linux gates), `frontend/scripts/bundle-budget.mjs`.
    - Options Considered:
      - Rely on per-task gate runs: leaves the cross-task delta (six module moves + extraction + splits interacting) unverified as a whole.
      - One final full run with delta recording: selected.
    - Chosen Approach:
      - Final `time scripts/check.sh` plus frontend/clay-agent runs; paste the delta table into `Compromises Made`/`Further Actions` evidence and close the plan.
    - API Notes and Examples:
      ```bash
      time scripts/check.sh && (cd frontend && npm run lint && npm run test -- --run && npm run build && node scripts/bundle-budget.mjs)
      ```
    - Files to Create/Edit:
      - This plan file: mark checkboxes, fill `Compromises Made` and `Further Actions`.
    - References:
      - Task 1 baseline numbers.
  - Test Cases to Write:
    - None new; the full gate set is the check.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.