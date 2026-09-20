# 129 — Connection Loop Decomposition and Future-Size Reduction

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
C1, P4 (review §4, §2). Continues plan 119's SC-2 (which extracted the delivery
policy) by extracting per-family message handling out of `handle_connection_loop`
(`src/server/connection/mod.rs` L514–L1587, 1,073 lines, 31 arms, ~68 awaits).
Pure refactor: observable protocol behavior unchanged.

## Objectives

- C1: `handle_connection_loop` becomes a thin select router; each of the 31
  `ClientMessage` arms calls a named, individually testable handler in its
  submodule (`documents.rs`, `menus.rs`, `tabs.rs`, `workspace.rs`, `runtime.rs`, …).
- P4: as a consequence (plus explicit `Box::pin` on rare arms if still needed),
  no async fn on the connection path has a future > ~8 KB; the clippy
  `large_futures` warnings (baseline task 1: 15 lib-code sites inside
  `src/server/connection/**` + `src/server/mod.rs`; sizes 40×20,624 B /
  21×21,800 B / 7×28,168 B …, plus 70 test-code sites) drop to zero on this
  path. The plan originally wrote the lint as `large_future` (singular); on
  clippy 0.1.96 / rustc 1.96.1 that name is an unknown lint (E0602).
- Zero behavior change: all connection suites
  (`src/server/connection/tests.rs`, 8,558 lines) pass unmodified.

## Expected Outcome

- `handle_connection_loop` is < ~250 lines: subscriptions + select + one match
  dispatching to `handle_<family>` functions; per-family handlers live in their
  existing submodules with unit-testable signatures.
- `cargo clippy --all-targets -- -W clippy::large_futures` (lint is plural on
  this toolchain) reports no connection-path lib-code warnings (baseline: 15,
  see task 1; test-code sites excluded).
- All Linux gates green. Baseline caveat (task 1): HEAD has one pre-existing
  parallel-mode flake, `js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`
  (passes in isolation and serially); connection suites are green.

## Tasks

- [x] Baseline gates, loop metrics, and future-size inventory
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full set passes untouched; record exit codes.
    - Performance: record current clippy `large_future` count/locations for `src/server/connection/**` and `src/server/mod.rs`; record loop length/arms as static metrics.
    - Code Quality: no code changes.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §4 (C1), §2 (P4).
      - `plans/119-Editor-and-Agent-Architecture-Remediation.md` SC-2 task (delivery-policy extraction precedent and its evidence format).
    - Options Considered:
      - Skip metrics: refactor's success criteria are these numbers. Recorded.
    - Chosen Approach:
      - Static counts + clippy inventory stored with plan evidence.
    - Files to Create/Edit:
      - None.
    - References:
      - `src/server/connection/mod.rs:514–1587`.
  - Test Cases to Write:
    - None (evidence-recording task).
  - Evidence (2026-09-20):
    - Tree: branch `review/1909`, HEAD `66648e3`; 41 uncommitted WIP files
      (`src/server/js_runtime/**`, docs/plans/test-plan). No WIP change touches
      `src/server/connection/**` or `src/server/mod.rs` — plan target untouched.
    - Loop metrics (static): `handle_connection_loop`
      `src/server/connection/mod.rs:514–1587` = 1,074 lines (incl. signature);
      dispatch `match message` at L881 with 31 `ClientMessage::*` arms and 68
      `.await` inside the loop. Plan's "1,073 lines / 31 arms / ~68 awaits"
      confirmed. `src/server/connection/tests.rs` = 8,765 lines (plan's 8,558 is
      stale); `cargo test --lib --quiet connection::` → 93 passed, 0 failed.
    - Clippy inventory — lint name correction: `cargo clippy --all-targets
      --message-format=json -- -W clippy::large_futures` → repo-wide 95 warning
      sites (111 raw JSON emissions), connection scope
      (`src/server/connection/**` + `src/server/mod.rs`) 85 sites:
      - lib code (15): `src/server/mod.rs` 6 — `spawn_configuration_watcher`
        L967/L970, `run` L980, `trigger_developer_hot_reload` L1124,
        `execute_reload_command` L1157, `reload_runtime_generation` L1161;
        `connection/runtime.rs` 5 — `execute_command_intent` L89/L161,
        `persist_settings_change` L309, `handle_sdui_action` L785,
        `handle_command_intent` L915; `connection/mod.rs` 3 — inside the loop at
        the `menus::handle_menu_activate` (L1309), `runtime::handle_sdui_action`
        (L1346), `runtime::handle_command_intent` (L1372) arms;
        `connection/menus.rs` 1 — `handle_menu_activate` L576.
      - test code (70): `connection/tests.rs` 21; `src/server/mod.rs` test
        modules 49.
      - Sizes (unique sites): 40×20,624 B; 21×21,800 B; 7×28,168 B; 7×20,528 B;
        2×20,736 B; singles 23,176 / 22,992 / 22,432 / 22,488+22,584 (one site)
        / 21,536 / 20,088 / 21,776 / 20,840 B. Review counts 40/21/7 match; its
        bytes are 192 B higher (different rustc/clippy).
      - `-W clippy::large_future` (singular, as the plan was written) is unknown
        on clippy 0.1.96/rustc 1.96.1: E0602 per compiled target, command still
        exits 0. Tasks 2–3 must use `clippy::large_futures`.
    - Baseline gates — true untouched tree (detached worktree of HEAD `66648e3`
      at `/tmp/clay-head-129`; untracked build outputs `clay-agent/dist` and
      `frontend/dist` linked in, otherwise `test` fails with `clay-agent host
      stopped`): `cargo audit` 0, `cargo fmt --check` 0, `cargo check
      --all-targets` 0, `cargo clippy --all-targets -- -D warnings` 0, `cargo
      bench --no-run` 0, `scripts/check-bindings.sh` 0; `cargo test --all-targets`
      exit 101 — one failure: `server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`
      ("coding profile declaration queues without a skills field", tests.rs:10986).
      Parallel-mode flake/pollution: passes in isolation and in a serial full-suite
      run (`cargo test --lib -- --test-threads=1` → 1,414 passed, 0 failed). No
      `connection::` test fails.
    - Baseline gates — working tree (41 dirty files): same single test failure at
      dirty line `src/server/js_runtime/tests.rs:11021`; `scripts/check.sh full`
      exits 101 at the `test` stage (audit/fmt/check/clippy passed; bench/bindings
      not reached — script aborts on first failure).
    - Consequence: "all gates green" is blocked by the pre-existing js_runtime
      flake, not by this refactor; connection-path suites are green. Task 3's
      lib-code target is 15 sites (at minimum, the three loop arm call sites).
    - Logs: `/tmp/check-head-129.log`, `/tmp/check-full-129.log`,
      `/tmp/clippy-large-futures.jsonl`, `/tmp/head-serial2-129.log`,
      `/tmp/main-connection-suite-129.log`.

- [x] Define the handler context object and extract the mechanical arms
  - Acceptance Criteria:
    - Functional: a `ConnectionCtx<'_>` (or small struct of the loop's shared state: stream handle, client_id, document/behavior/workspace Arcs, coordinators, codec, subscriptions as needed) is passed to extracted `handle_*` fns; at least the straightforward arms (ListDocuments, GetDocumentStatus, ListLauncherEntries, RemoveLauncherRecent, ListAgentSettingsFiles, OpenAgentSettingsFile, OpenSelectedFile, RequestResync, CloseDocument, SaveDocument, ReloadDocument, DocumentChunkRequest) are extracted.
    - Performance: none (refactor).
    - Code Quality: each handler ≤ ~120 lines; no `clone` introduced solely for extraction (Arc clones only where the handler outlives the borrow — prefer keeping handlers borrowing the ctx).
    - Security: authorization checks (`document_for_message`, tab binding checks, session binding) move with their arms unchanged — each extraction's diff shows the check preceding state access.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection/mod.rs` (loop + existing helpers `message_requires_tab_state`, `route_connection_tab_state`), submodules `documents.rs`/`menus.rs`/`tabs.rs`/`workspace.rs`/`runtime.rs` (existing extraction targets).
      - `.agents/skills/clay-execution/references/protocol-perf.md` (connection-path perf invariants).
    - Options Considered:
      - One giant `match` moved to a new file: moves the problem.
      - Context struct + per-family handlers in existing submodules. (Chosen.)
      - Message-handler registry (map from discriminant to fn): indirection without benefit at 31 arms — rejected.
    - Chosen Approach:
      - Incremental extraction, one family per commit-sized step, suites run between steps.
    - API Notes and Examples:
      ```rust
      // src/server/connection/documents.rs
      pub(super) async fn handle_save_document<S: AsyncWrite + Unpin>(
          ctx: &mut ConnectionCtx<'_, S>, msg: SaveDocument,
      ) -> Result<(), CodecError> { … }
      ```
    - Files to Create/Edit:
      - `src/server/connection/mod.rs` (ctx + router), `src/server/connection/documents.rs`, `menus.rs`, `tabs.rs`, `workspace.rs`, `runtime.rs`.
    - References:
      - Plan 119 SC-2 evidence (loop already thinned once; same method).
  - Test Cases to Write:
    - Existing suites unmodified and green after each extraction step (this is the regression net; no new tests for moved code).
  - Evidence (2026-09-20):
    - `ConnectionCtx<'a, S>` added in `src/server/connection/mod.rs`: codec,
      stream, client_id, document/workspace/behavior Arcs, runtime_generation,
      sdui, parse_coordinator, completion, language_intelligence,
      document_analysis, reload_server, file_open_capabilities — all borrows,
      no clones.
    - 14 arms converted: the 12 listed (RequestResync, DocumentChunkRequest,
      OpenSelectedFile, ListAgentSettingsFiles, ListLauncherEntries,
      RemoveLauncherRecent, OpenAgentSettingsFile, SaveDocument, ReloadDocument,
      CloseDocument, GetDocumentStatus, ListDocuments) plus `OpenDocument` and
      `AddSelectedWorkspaceRoot`, which shared the identical open/workspace
      handlers.
    - Handler changes: `documents.rs` — ctx-taking `handle_document_chunk_request`
      (client_id dropped from `DocumentChunkRequestParams`), `handle_request_resync`,
      `handle_open_document`, `handle_save_document`, `handle_reload_document`,
      `handle_close_document`, `handle_get_document_status`, `handle_list_documents`;
      `workspace.rs` — ctx-taking `handle_open_selected_file`,
      `handle_add_selected_workspace_root`, plus new `handle_list_launcher_entries`,
      `handle_remove_launcher_recent`, `handle_list_agent_settings_files`,
      `handle_open_agent_settings_file`, and the launcher helper
      `write_launcher_entries` moved from `mod.rs`.
    - A `ctx!()` local macro builds the context per converted arm. It is
      deliberate task-2 scaffolding: unconverted arms (`TabCommand` rebinds
      `document`/`workspace`; lane/select state) still use the loop's locals,
      which a loop-scoped context would forbid. Task 3 replaces the macro with
      the loop-scoped context.
    - Metrics: `handle_connection_loop` L499–L1435 = 937 lines (was 1,074;
      −137); 31 arms unchanged; 65 `.await` (was 68); 14 `ctx!()` uses. Handler
      sizes: max 59 lines (`handle_add_selected_workspace_root`), all ≤ 120.
      No new clones introduced (`git diff` grep for `Arc::clone`/`.clone()` in
      `src/server/connection/` → 0).
    - Security: authorization moved unchanged with the arms —
      `document_for_message` is still the first check in the resync/chunk
      handlers, capability `consume`/`issue` still precede the open in both
      workspace grant handlers, and `agent_settings::resolve_agent_settings_file`
      still validates the name server-side before the ordinary document
      pipeline.
    - Verification: `cargo fmt --check` clean; `cargo clippy --all-targets
      -- -D warnings` exit 0; `cargo test --lib connection::` → 93 passed, 0
      failed; `cargo test --tests` → 1,414 lib + 9 + 62 + 224 + 75 + 152 all
      passed, 0 failed (the task-1 parallel flake passed this run).
      `src/server/connection/tests.rs` unmodified.
    - `large_futures` lib sites unchanged at 15; the loop's three sites moved
      L1309/1346/1372 → L1157/1194/1220 — those are the still-inline
      `MenuActivate`/`SduiAction`/`CommandIntent` calls, i.e. task 3's stateful
      arms. Task 2 moved no large futures.
    - Files touched: `src/server/connection/mod.rs`, `documents.rs`,
      `workspace.rs` only; `menus.rs`/`tabs.rs`/`runtime.rs` untouched.
    - Graft graph refreshed (`graft build`, 11,458 nodes) so later tasks read
      current spans.

- [x] Extract the stateful arms (Edit, EditorIntent, CompletionRequest, LanguageIntelligenceRequest, SduiAction, ViewportRenderRequest, Agent, Hello, RuntimeGenerationInstalled, Menu* family)
  - Acceptance Criteria:
    - Functional: loop is a thin router (< ~250 lines); all 31 arms dispatch; subscription select arms stay in the loop.
    - Performance: clippy `large_futures` lib-code warnings on the connection path eliminated (baseline 15 sites, task 1; Box::pin only where a rare arm still oversized — justified in code comment); test-code sites excluded.
    - Code Quality: handlers individually callable from tests; at least two new unit tests exercise extracted handlers directly (previously only reachable through the loop).
    - Security: per-arm authorization and budget checks unchanged (diff-level review recorded in evidence).
  - Approach:
    - Documentation Reviewed:
      - Same as task 2; `src/server/connection/runtime.rs` (completion/LI arms to extract, following plan 126's changes if already landed — coordinate ordering).
    - Options Considered:
      - Extract everything at once: high-risk single step.
      - Two-stage (mechanical first, stateful second). (Chosen.)
    - Chosen Approach:
      - Stateful arms last; run connection suite after each. Task 2 landed 14
        arms on `ConnectionCtx`; the remaining 17 arms (Edit, EditorIntent,
        ViewportRenderRequest, SelectionQueryRequest, SduiAction, CommandIntent,
        CompletionRequest, LanguageIntelligenceRequest, RuntimeGenerationInstalled,
        Hello, Agent, TabCommand, and the five Menu* intents) need the ctx
        extended with the lane/select-owned state (`menu_sessions`,
        `bound_state`/`bound_tab_id`, `pending_viewport_patches`, result-lane
        senders, tab registry). `ctx!()` stays per-arm: a loop-scoped context
        cannot coexist with the acceptance rule that the subscription `select!`
        arms keep the connection's locals, and `TabCommand` rebinds
        `document`/`workspace`, which the context exposes as `&mut` handles.
    - Files to Create/Edit:
      - `src/server/connection/mod.rs` + submodules as above.
    - References:
      - Review C1, P4.
  - Test Cases to Write:
    - `handler_direct_unit_tests`: two extracted handlers invoked with crafted ctx + message asserting response stream content.
    - Existing 8,558-line connection suite green (regression net).
  - Evidence (2026-09-20):
    - `ConnectionCtx` (mod.rs) extended with the stateful handles:
      `menu_sessions`, `bound_tab_id`, `bound_state`, `tab_registry`,
      `tab_registry_tx`, `pending_viewport_patches`, `completion_tx`,
      `language_intelligence_tx`, `dropped_results`; `document`/`workspace` are
      now `&mut Arc` so `TabCommand` rebinding flows through the context.
    - All 17 remaining arms extracted; all 31 arms now dispatch through family
      handlers: `documents::{dispatch_edit_operation` (Edit + EditorIntent),
      `handle_viewport_render_request` + `track_pending_viewport_request`,
      `handle_selection_query_request}`, `menus::{handle_menu_query_update,
      handle_menu_backspace, handle_menu_selection_move, handle_menu_activate,
      handle_menu_cancel}`, `tabs::handle_tab_command`,
      `runtime::{handle_sdui_action, handle_command_intent,
      handle_completion_request, handle_language_intelligence_request,
      handle_runtime_generation_installed, handle_duplicate_hello,
      handle_agent_command}`.
    - Moved verbatim: the Agent arm became `runtime::handle_agent_command`
      (whitespace-normalized diff vs `HEAD` shows only rustfmt rewraps of three
      chained calls — zero logic change); the first-message `Hello` handshake
      became `complete_first_message_handshake` in mod.rs; the duplicate-`Hello`
      error became `runtime::handle_duplicate_hello`.
    - Handler style: bodies unchanged; each converted handler opens with aliases
      from `ctx` (`let codec = ctx.codec; let stream: &mut S = &mut *ctx.stream;
      …`), so its diff is signature + prelude only. Edit/EditorIntent/Viewport
      now pass `documents::{EditOperationParams, ViewportRequestParams}`.
    - Metrics: `handle_connection_loop` L507–L1109 = 603 lines (baseline 1,074;
      task 2: 937). Dispatch `match` region = 249 lines (≈ the thin-router
      target); 31/31 arms are one handler call (three marshal a params struct
      inline); loop `.await`s 49 (baseline 68); 31 `ctx!()` uses.
    - Acceptance deviation (explicit): the *function* is 603 lines, not < ~250.
      The residual is fixed connection scaffolding, not dispatch: signature 24,
      subscription/channel/capability prelude ~200, `select!` lane block ~70,
      decode/identity/routing ~60, dispatch match 249. The router region itself
      meets the target; shrinking further means moving subscription setup/select
      out of the function, which this task's own acceptance rule forbids
      (“subscription select arms stay in the loop”).
    - Large futures — eliminated: default-threshold
      `cargo clippy --all-targets -W clippy::large_futures` reports zero
      lib-code sites in `src/server/connection/**` and `src/server/mod.rs`
      (baseline 15). With a temporary `clippy.toml`
      (`future-size-threshold = 8192`): zero lib-code sites > 8 KB repo-wide
      (13 residual > 8 KB sites are all test modules, excluded by acceptance).
      Connection-loop future ≈ 4.3 KB (baseline ~21 KB); largest
      connection-path awaited futures are all < 8 KB —
      `menus::handle_menu_activate` 6,488 B, `runtime::handle_sdui_action`
      5,040 B, `runtime::handle_agent_command` 4,984 B, `tabs::handle_tab_command`
      5,232 B, `runtime::handle_command_intent` 4,504 B.
    - `Box::pin` sites, all cold paths and each with an in-code comment:
      `reload_runtime_generation_inner` (3 reload stages),
      `load_default_configuration` (3 stages), `execute_reload_command`
      (inner), `execute_command_intent` (`persist_settings_change`,
      `execute_reload_command`), `persist_settings_change`
      (`reload_runtime_generation`), and `src/bin/clay-server.rs` `main`
      (`run`). Boxing is the only change: no behavior, ordering, or diagnostics
      changed.
    - New direct-handler tests (`src/server/connection/tests.rs`, +179 lines):
      `DirectHandlerState` builds a crafted `ConnectionCtx` from the existing
      test builders (no socket, no server, `reload_server: None`), and three
      tests drive handlers directly:
      `extracted_list_documents_handler_answers_without_the_loop` (empty
      `DocumentList`), `extracted_launcher_handler_answers_without_the_loop`
      (empty `LauncherEntries`, `pruned == 0`, client id echoed),
      `extracted_duplicate_hello_handler_rejects_without_the_loop`
      (`InvalidMessage`, “duplicate Hello message”).
    - Security (diff-level review): authorization and budget checks unchanged —
      Edit keeps `document_for_message` first plus the behavior-version gate;
      viewport keeps workspace-membership/version checks before any parse and
      the same rejection paths; selection query keeps `request.validate()`
      first and `document_for_message` before reading text; completion/LI keep
      `request.client_id = client_id` as the first statement and the same
      bounded result lanes/`dropped_results`; Menu/SDUI/Command keep their
      session, manifest, and package-action gates; `TabCommand` keeps its
      registry/binding checks; Agent keeps the same tab resolution and book-path
      dispatch (verbatim move); the handshake keeps the protocol-version gate,
      welcome snapshot, tab replay, and capability issuance order.
    - Verification: `cargo fmt --check` clean; `cargo clippy --all-targets
      -- -D warnings` exit 0; `cargo test --lib -- --test-threads=1` → 1,417
      passed, 0 failed (baseline 1,414 + 3 new tests); `cargo test --tests
      -- --test-threads=1` → 1,417 lib + 9 + 62 + 224 + 75 + 152 passed, 0
      failed; `cargo test --lib connection::` → 96 passed, 0 failed;
      `cargo bench --no-run` exit 0. Parallel-mode full run still hits the
      pre-existing `js_runtime` flake recorded in task 1 (passes in isolation
      and serially; unrelated to this path).
    - Files: `src/server/connection/{mod,documents,menus,runtime,tabs}.rs` and
      `tests.rs`; `src/server/mod.rs` (boxing only); `src/bin/clay-server.rs`
      (boxing only). No pre-existing test was modified.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: core editing, multi-cursor, caret/typography, and command modules re-run on a real Linux build; record pass/fail (pure refactor — expected all-pass; any failure is a defect).
    - Performance: none beyond perceived-input-latency sanity note.
    - Code Quality: no new manual steps expected; record that explicitly with the reason (internal refactor with automated regression net) if none are added.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`; `.agents/skills/create-plan/references/clay.md` (explicit-record rule for internal changes).
    - Chosen Approach:
      - Regression-only manual pass.
    - Files to Create/Edit:
      - None expected; record executed steps in task evidence.
    - References:
      - None beyond index.
  - Test Cases to Write:
    - None.
  - Evidence (2026-09-20):
    - Harness (new): `test-plan/artifacts/129-connection-loop/` — isolated
      live launcher (`run-live.sh start editing|cursors|cursorstyle|commands|chords`,
      mode-700 private root, private socket, tree-kill teardown), AT-SPI probe
      (`probe.py`: `editor`, `text`, `mark`, `rows`, `act`, `set`, `dump`),
      window-cropped portal screenshots (`portal-shot.py`), module fixtures
      (`init-completion.js` with a `Ctrl+J` completion trigger because this host
      owns `Ctrl+Space`, `init-chords.js`, `init-cursor-style.js`,
      `init-invalid-caret.js`), `README.md`, and `automated-companions.txt`.
    - Build: both binaries rebuilt before the pass (`cargo build --bins`,
      `cargo build --bins -p clay-desktop`; plan 118's mixed-binary
      rkyv/host-stopped ceiling avoided).
    - Module 04 (core editing) — PASS live: E2 (Backspace/Delete
      character-granular: 2,803 → 2,802 → 2,801), E3 (Enter keeps the indented
      block's leading whitespace on the new line), E5 (`}` electric-outdents
      from a 4-space line to column 0: 2,702 → 2,699), E6 (`(` inserts `()`
      with the caret inside; `)` skips the auto-close), E7 (Tab = exactly 4
      spaces for the rust mode: 2,701 → 2,705), E1 ASCII echo (`AB`, `zzmark2`
      inserted at the caret with caret/head/count agreeing), E8 undo
      (`Ctrl+Z` restored text and caret twice; 2,805 → 2,798). Every step was
      driven through the portal keyboard and verified against the live AT-SPI
      tree; character counts are the editor's bounded accessibility window, and
      comparisons stay inside one window.
    - **Defect found — module 04 E4 (comment continuation): FAIL live.**
      `Enter` at the end of `    // caret-here comment line` produced `\n    `
      (indent only, no `// ` continuation) although the bundled `@clay/rust`
      manifest declares `"comments":[{"linePrefix":"//","continuePrefix":"// "}]`
      and `docs/reference/packages/creating-packages.md` states that
      `continuePrefix` “controls comment continuation after Enter”.
      `frontend/src/editor/extensions/behavior.ts::applyEnterRule` handles only
      `continueLineMarkers`, `insertNewlineOnly`, and the default indent
      preservation, and `continuePrefix` has no consumer in `frontend/src`
      (only the type at `extensions/types.ts:155`; `controller.ts:723` reads
      `linePrefix` for toggle-comment). Pre-existing and client-local — the
      Enter rule never round-trips through the connection loop — so this is
      **not** caused by the refactor; it needs a fix-or-re-document decision
      (added to Further Actions).
    - Module 06 (multi-cursor): X1–X15 UNRESOLVED live (client-local CodeMirror
      operations; the host could not deliver the multi-key sequences) and PASS
      by automated equivalents on the refactored tree — fresh
      `npx vitest run src/editor` 9 files / 75 tests passed (editor extensions,
      position map, hot-path invariants).
    - Module 07 (caret/typography): T9 PASS live (window-cropped capture shows
      joined Fira Code ligatures on the sample line under the 16 px monospace
      pin); the invalid-caret negative check PASS live
      (`clientSetCursorStyle({shape:"triangle"})` →
      `clay server configuration failed [editor.invalid_set_cursor_style]: JavaScript runtime evaluation failed.`
      with the app still up). Caret-shape paint and the layout/theme reload
      steps are UNRESOLVED (no caret paint without real input; no reload
      keystroke delivered). Finding: T25's gutter expectation is stale — the
      shipped full-bleed editor has no line-number gutter by design
      (`docs/wiki/modules/react-shell.md`, `frontend/src/editor/extensions/behavior.ts`);
      recorded, step text left unchanged (not silently weakened).
    - Module 10 (keybindings/commands) — PASS live: K19/K29/K30 (AT-SPI `press`
      on the shell's `palette Ctrl X O` button opens the Control Center:
      `list box "Commands"` with 88 rows — built-ins, the `shell.client*`
      family, and package commands with provenance/detail) and K22/K35
      (activating `Reload Configuration and Packages` executed
      `runtime.reloadConfiguration`: `init.js` re-ran —
      `agentProfile.register 'coding' applied` plus a duplicate-command
      rejection — with no `Session lost` and the client still serving).
      K21 PARTIAL (activated row reported `focused,selected`). K33/K34
      UNRESOLVED live (AT-SPI `DoAction` on a palette row returned `True` but
      did not drive the client shell bridge — not an `Enter` substitute for
      client-first commands); K20/K23/K24/K31/K38/K42 and the chord/binding
      steps UNRESOLVED on the keyboard ceiling; all carried by the fresh
      `control_center` suite (28 passed).
    - Fresh automated regression on the refactored tree:
      `cargo test --lib connection::` 96 passed; `cargo test --lib completion::`
      31 passed; `cargo test --lib control_center` 28 passed; frontend
      `vitest run src/editor` 75 passed.
    - New host ceiling recorded: `xdg-desktop-portal-gnome` **segfaults** on
      RemoteDesktop keyboard sessions (`code=dumped, status=11/SEGV`,
      `g_hash_table_lookup` assertion), so keystrokes land only in short bursts
      after a fresh app launch; every longer interactive sequence is recorded
      UNRESOLVED with its reason instead of being inferred. `wtype` is
      installed but unusable (GNOME 50 has no virtual-keyboard protocol),
      `/dev/uinput` is denied, and there is no `ydotoold` socket, so pointer
      input is unavailable; AT-SPI named-node actions were used where a step
      did not need typing.
    - Code Quality (no new manual steps): none added, with the reason recorded
      in `test-plan/index.md` — an internal refactor whose user-visible
      behavior is unchanged, so the pass re-ran existing steps instead of
      writing new ones; the only new harness is the artifact-local launcher and
      probe.
    - Files: `test-plan/index.md` (execution record),
      `test-plan/{04-core-editing,06-multi-cursor,07-caret-and-typography,10-keybindings-and-commands}.md`
      (per-module records), and the new `test-plan/artifacts/129-connection-loop/`
      tree. Security: no manual security-relevant step in these modules; the
      refactor's authorization/budget diffs were reviewed in task 3.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed; all extracted handlers `pub(crate)`/`pub(super)`; verify via diff.
    - Performance: none.
    - Code Quality: registry/doc-guard suites pass.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach:
      - Verify-only.
    - Files to Create/Edit:
      - None expected.
    - References:
      - Decision log 2026-05-08-1509.
  - Test Cases to Write:
    - None.
  - Evidence (2026-09-20):
    - Public programmatic surface unchanged — verified three ways:
      (1) **diff scope**: the plan-129 change touches only
      `src/server/connection/{mod,documents,menus,runtime,tabs,workspace,tests}.rs`
      plus boxing-only edits in `src/server/mod.rs` and `src/bin/clay-server.rs`;
      `src/server/ops/**` (every `deno_core` op wrapper), `src/server/facades.rs`,
      `src/protocol/**`, `src/js_api/**`-adjacent JS files, `docs/index.md`, and
      `docs/reference/clay-js-api/**` are untouched by the diff, so no op name,
      facade export, stable ID, permission string, or documented contract
      changed. No `ClientMessage` variant was added or removed (31 arms before
      and after).
      (2) **visibility**: `git diff -U0` over the change filtered to
      `pub fn|pub struct|pub enum|pub mod|pub use|pub const|pub static|pub trait|pub type|pub(crate)`
      additions returns **empty** — no bare-public or `pub(crate)` item was
      introduced; every added `pub` line is a `pub(super)` field of the new
      internal `ConnectionCtx`/param structs. `rg '^\s*pub (fn|struct|enum|async fn|const|static|trait|type|use|mod)\b' src/server/connection/*.rs`
      (excluding `pub(super)`/`pub(crate)`) returns nothing: the 30 extracted
      handlers are all `pub(super) async fn` (`documents.rs` 11 — 8 document
      handlers plus `dispatch_edit_operation`, `handle_viewport_render_request`,
      `handle_selection_query_request`; `runtime.rs` 7; `workspace.rs` 6;
      `menus.rs` 5; `tabs.rs` 1), and the remaining ones (`handle_connection`,
      `handle_connection_loop`, `handle_agent_picker_outcome`) are private. New types `ConnectionCtx`,
      `EditOperationParams`, `ViewportRequestParams`, `DocumentChunkRequestParams`,
      `PendingViewportPatch`, `TabDispatch`, `PersistOutcome`, `Delivery`,
      `Outcome`, `Flow`, `OpenModeActivation` are all `pub(super)`. `mod connection;`
      stays private (`src/server/mod.rs:14`), and the only crate entry point is the
      pre-existing `pub(crate) async fn handle_connection_with_analysis`
      (`src/server/connection/mod.rs:389`) — so nothing in the extracted code is
      reachable as a programmatic surface, matching decision log 2026-05-08-1509
      (“functions that should stay internal are private or `pub(crate)`”).
      (3) **provider WIP attribution**: the three `docs/reference/clay-js-api/**`
      files that are dirty in this worktree are the pre-existing `moduleSpecifier`
      documentation edits for the completion/language provider registration APIs
      (mtimes 00:11–00:53, before this plan started at ~11:45; `moduleSpecifier`
      is already committed in `src/server/ops/{completion,language_intelligence}.rs`).
      They are unrelated to plan 129 and their registry is coherent (the freshness
      gate below passes).
    - Registry/doc-guard suites (fresh, non-mutating): `cargo test --test protocol`
      **224 passed, 0 failed** — includes `clay_js_api_inventory` (14, e.g.
      `every_public_api_has_generic_sections_facade_and_naming_contract`,
      `inventory_rust_paths_name_existing_source_files`,
      `public_inventory_docs_index_and_generated_matrix_match_exactly`),
      `clay_js_doc_registry` + `clay_js_facade_layout` (74, e.g.
      `generated_registry_is_current`,
      `every_public_api_contract_matches_generated_markdown_metadata`,
      `clay_js_api_inventory_unchanged_or_documented`,
      `clay_js_facade_declarations_parity_with_implementations`), and
      `documentation_coverage` (13). `cargo test --test security` **152 passed,
      0 failed** — includes `rust_visibility_api_mapping` (6, e.g.
      `phase22_8_per_tab_state_has_no_new_public_programmatic_surface`, which
      reads `src/server/connection/{mod,documents,workspace}.rs` and asserts
      those helpers stay crate-private and never become bare `pub`,
      `internal_runtime_mechanics_are_not_public`,
      `plan119_session_relay_helper_is_not_a_public_programmatic_surface`).
      `cargo run --bin update-doc-registry` was deliberately **not** run because
      it always rewrites checked-in artifacts; the freshness gates
      (`generated_registry_is_current`,
      `every_public_api_contract_matches_generated_markdown_metadata`) fail on a
      stale registry, which is the required non-mutating check per the JS API
      reference.
    - Security: no new op wrapper, permission string, authority marker, or
      registry entry; the public API inventory's `permissions`/`security_notes`
      rows are byte-identical (no diff into the inventory).

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: connection-handling wiki page(s) reflect the router/handler structure with the per-family map.
    - Performance: notes future-size reduction and its cause (per-arm state now boxed in smaller futures).
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: documents that authorization checks live at handler entry points.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach:
      - Update once after tests pass.
    - Files to Create/Edit:
      - `docs/wiki/modules/*.md` (connection page — locate via index), `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.
  - Evidence (2026-09-20):
    - Page located via `docs/wiki/index.md`: the connection page is
      `docs/wiki/modules/server-ipc-skeleton.md` (index line 31 links it; line 121
      carries the Plan 119 lane-policy pointer).
    - Updated the page:
      - **Source + module layout**: added the handler-submodule source lines
        (`documents/workspace/tabs/menus/runtime` + `tests.rs`) and rewrote the
        `mod.rs` / `runtime.rs` rows of the Plan 090 layout table for the router,
        `ConnectionCtx` + `ctx!`, `complete_first_message_handshake`, and the
        actual per-family handler sets (the old `runtime.rs` row listed
        `RequestResync`/`DecorationViewportRequest`, which live in `documents.rs`).
      - **New section “Dispatch Router and Handler Context (Plan 129)”**: the
        arm-family → handler map covering all 31 `ClientMessage` arms across the
        five submodules; the `ConnectionCtx` field inventory (routing handles +
        coordinators) with the borrow-only invariant and the `TabCommand`
        rebinding path; and the authorization-boundary paragraph.
      - **Performance**: the “Future sizes (Plan 129)” paragraph records the
        cause (per-arm state now in its own handler future; cold paths
        `Box::pin`ed so a nested future is not inlined into its caller) and the
        numbers — baseline 15 library `large_futures` sites on the connection
        path (85 unique incl. test helpers) at 20–28 KB; now zero library sites,
        zero library sites > 8 KB repo-wide under `future-size-threshold = 8192`
        (13 residual test-module sites), loop future ≈ 4.3 KB, largest awaited
        handlers 4,504–6,488 B, function 1,073 → 603 lines with a 249-line
        dispatch `match` — and names every `Box::pin` site.
      - **Security**: “Authorization checks live at handler entry points” states
        that the loop keeps only the post-`Hello` identity check and the tab-state
        route, and that each handler re-authorizes
        (`documents::document_for_message` requires routed-tab ownership **and**
        client access, selected-path capabilities are single-use, tab commands go
        through `TabRegistry`), with a typed rejection for a missing binding,
        unknown document, or spent capability.
      - Also added the new Invariants bullets (one handler call per arm,
        `pub(super)`-only handlers pinned by
        `tests/rust_visibility_api_mapping.rs`, authorization not inherited from
        the route), the `DirectHandlerState` entry in Tests, and Related links to
        `tabs-and-clients.md` and `plans/129-Connection-Loop-Decomposition.md`.
      - `docs/wiki/index.md`: enriched the existing Server IPC Skeleton line with
        the Plan 129 thin router (`ConnectionCtx`, one handler per arm family,
        handler-entry authorization, sub-8 KB futures) so the page stays
        discoverable.
    - Doc-alignment cleanup found while writing the page: the refactor left stale
      `#[allow(clippy::too_many_arguments)]` attributes on eight extracted
      handlers whose signatures now take the context (≤ 4 parameters, below
      clippy's > 7 threshold) — `menus::handle_menu_query_update`,
      `menus::handle_menu_activate`, `runtime::handle_command_intent`,
      `runtime::handle_completion_request`,
      `runtime::handle_language_intelligence_request`, `tabs::handle_tab_command`,
      `workspace::handle_open_selected_file`,
      `workspace::handle_add_selected_workspace_root` (one also carried an
      obsolete reason string). Removed so the page's claim matches the code; the
      loop and the genuinely wide state-threading helpers keep theirs.
      Re-verified afterwards: `cargo fmt --check` clean,
      `cargo clippy --all-targets -- -D warnings` clean,
      `cargo test --lib server::connection` 96 passed.
    - New deterministic guard (deviation from the planned “manual wiki review”
      only, recorded): `tests/documentation_coverage.rs::plan129_wiki_pages_describe_the_dispatch_router_and_future_sizes`
      asserts the page names the router, `ConnectionCtx`, `ctx!`, one handler per
      arm family, the handler-entry authorization sentence, `Box::pin` and the
      8 KB threshold, plus the index link — following the plan-125/127 wiki
      contracts already in that file, so the page cannot silently drift back to
      the inline-loop description.
    - Verification: `cargo test --test protocol` **225 passed, 0 failed**
      (includes the new test, `documentation_coverage` 14,
      `primitives_docs` 35 incl. `wiki_index_links_every_wiki_page`,
      `manual_smoke_docs`, and the `clay_js_doc_registry` freshness gates that
      keep docs and registry coherent); `cargo test --test security`
      **152 passed, 0 failed**.
    - Repo graph refreshed after the refactor: `graft build` → 11,490 nodes /
      20,646 edges (610 files replayed from cache).
    - Attribution note: the `docs/wiki/index.md` diff also carries plan-127 and
      plan-128 description lines, and the wiki edits on
      `persistent-runtime-hardening.md`, `embedded-js-runtime.md`,
      `parse-coordinator.md`, `completion-snippet-expansion.md`,
      `language-intelligence.md`, and `first-party-lsp-bridge-packages.md` are
      that earlier WIP — only the Server IPC Skeleton line, the connection page,
      and the new `documentation_coverage` test belong to Plan 129. Security: the
      page now states the per-handler authorization contract for every family.

## Compromises Made
- The manual pass is partial by host limitation, recorded per step rather than
  inferred: `xdg-desktop-portal-gnome` segfaults on RemoteDesktop keyboard
  sessions, so multi-key interactive legs (completion popup, multi-cursor
  sequences, Control Center typing/arrows/`Escape`, caret-shape paint, chord
  sequences) are `UNRESOLVED live` with their reasons, and each modules' fresh
  automated companions (connection 96, completion 31, control_center 28,
  frontend editor 75) carry the regression proof. No manual step was deleted or
  weakened.
- The loop's dispatch `match` region meets the thin-router target (249 lines),
  but the enclosing function is 603 lines because of fixed scaffolding the same
  acceptance criterion requires to stay in the loop (recorded in task 3).

## Further Actions

All three further actions were dispositioned on 2026-09-20 (see
`plans/143-Comment-Continuation-and-Manual-Pass-Follow-Ups.md`, which was
written from this list).

- **Fix or re-document comment continuation (module 04 E4, found by this
  plan's manual pass).** `@clay/rust` (and the docs) declare
  `comments[].continuePrefix`, but `frontend/src/editor/extensions/behavior.ts::applyEnterRule`
  never inserts it, so `Enter` on a `//` line only preserves indentation.
  Either implement the continuation branch (and a frontend test) or update
  `docs/reference/packages/creating-packages.md` and
  `docs/wiki/modules/behavior-runtime-registration.md` to stop promising it.
  Priority: medium (documented behavior mismatch; user-visible but no data
  loss). Pre-existing, not introduced by this refactor. → **Moved to plan 143**
  (primitive/semantics review, implementation + frontend tests, live re-run of
  E4, JS API/configuration verification, wiki guard); plan 143 keeps the
  docs-only path as its explicit fallback if review rejects implementation.
- **Re-scope module 07 T25's gutter clause** to the shipped full-bleed editor
  (`docs/wiki/modules/react-shell.md`: no line-number gutter; the manifest
  toggle is accepted and ignored) so the next execution does not fail a stale
  expectation. Priority: low (test-plan accuracy). → **Moved to plan 143**,
  broadened to the other stale gutter expectations found while scoping it
  (module 08 S21 fold chevron, module 13 S45/S46 split-pane gutter, the
  `test-plan/index.md` rows) plus the orphaned `GUTTER_PAINT_P95_BUDGET_MS`
  budget and the `docs/development/performance.md` row that names a test which
  no longer exists.
- **Host input path:** the portal-gnome segfault on RemoteDesktop keyboard
  sessions makes long interactive manual passes impossible; if future plans
  need multi-key live steps, either pin a working keyboard backend (portal fix
  / `ydotoold` socket / `uinput` access) or keep driving named-node AT-SPI
  actions and record the rest as UNRESOLVED. Priority: medium for plan 130+ if
  they add interactive manual steps. → **Moved to plan 143** (task: establish
  and document the durable local input path, verified with
  `computer_use_linux doctor` and one live typing leg, with the portal
  short-burst procedure recorded as the fallback).
