# Audit Remediation: UI Foundation and Review Harness

Prerequisite: Plan 086 complete with AT-SPI startup, audit, and serial Linux gates green.

Source review: P1-1, P1-2, P1-3, performance notes for transient menus, and test gaps 2 and 6 in `code-reviews/2026-08-14-comprehensive-implementation-and-ui-ux-review.md`.

Scope: Build the smallest repeatable visual-review workflow, replace the prototype welcome document with a useful Clay-owned entry state, and make completion compact/caret-adjacent. Broad aesthetic modernization belongs to Plan 088.

## Objectives

- Make first launch communicate how to begin editing and how to recover from connection/runtime problems.
- Stop empty completion from occupying a full-width 35%-height bottom surface.
- Establish repeatable screenshot/accessibility evidence using existing smoke fixtures and desktop tooling.
- Preserve Clay-owned layout, server authority, inert package UI, theme configurability, and editor hot paths.

## Expected Outcome

- Default, loading, disconnected/error, and recovery states are useful, token-driven, keyboard accessible, and visually reviewed.
- Completion is caret/line-adjacent, bounded, scrollable, selection-visible, and automatically absent for empty/expired results.
- Command/path centre remains centered, scrollable, and distinct; 60+ results stay within available window bounds with no clipped or unreachable rows.
- Empty/expired completion is dismissed rather than rendered as a full-width surface, and package-authored transient-menu item labels are bounded and sanitized before accessibility projection.
- A documented Linux review command creates fixed-size fixture states and retained screenshot/accessibility artifacts without pretending structural snapshots are pixel proof.

## Tasks

- [x] Establish entry evidence and review Clay UI primitive reuse
  - Acceptance Criteria:
    - Functional: Capture current default and completion states; trace welcome document creation, command dispatch, caret geometry, completion result application, menu projection, and overlay hosting end to end.
    - Performance: Record current completion layout/filter work and fixture startup time as advisory baseline.
    - Code Quality: Inventory existing `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, `paint_kbd_hint`, `paint_tooltip_shell`, transient-menu session, and smoke fixture paths before proposing code.
    - Security: Preserve native-dialog/server validation for open actions; welcome UI grants no direct filesystem/package authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`, `references/components.md`, `references/tokens.md`, `docs/reference/ui-components.md`.
      - `npx ui-skills start`, `vercel-labs/web-design-guidelines`, `ibelick/fixing-accessibility`; current interface guidelines fetched from their official source.
      - Project patterns: `package-ui-layout.md`, `ui-visual-review.md`, `ui-modernization.md`, `authority-boundaries.md`, `protocol-and-performance.md`.
    - Options Considered:
      - Add bespoke welcome/completion widgets immediately: rejected; catalog composition and shared menu host exist.
      - Reuse existing retained primitives and add only a proven generic geometry/state gap: chosen.
    - Chosen Approach:
      - Produce a state/primitive matrix first; use existing command IDs `documents.clientOpenFileDialog` and `workspace.clientOpenFolderDialog` for primary actions.
    - API Notes and Examples:
      ```text
      Welcome action → existing inert command intent → Driver native dialog → server-validated open/root binding
      Completion result → current request check → compact transient session → shared overlay host
      ```
    - Files to Create/Edit:
      - `plans/087-Audit-Remediation-UI-Foundation-and-Review-Harness.md`: record matrix/evidence.
    - References:
      - `src/server/mod.rs::TabServerState::from_workspace`, `src/masonry_pane_document.rs::apply_completion_result`, `src/shell/transient_menu.rs`, `src/shell/package_ui.rs`.
  - Test Cases to Write:
    - Primitive inventory proves every proposed surface maps to existing catalog entries or names one generic missing capability.

### Task 1 Evidence (2026-08-14)

- **Entry-state captures:** Plan 086's retained Linux review artifacts remain current because no UI production path changed after capture:
  - `code-reviews/screenshots/2026-08-14-plan086-a11y/default-single-tab.png` — 900×1116 Clay window, focused editor Entry, `Connected — Editable` status.
  - `code-reviews/screenshots/2026-08-14-plan086-a11y/completion-empty.png` — `Completion` menu with `No completions`; no malformed tree or process exit. Successful item geometry remains Task 4 coverage.
  - AT-SPI method/results: `code-reviews/screenshots/2026-08-14-plan086-a11y/review-log.md`.
- **Welcome/document flow:** `IpcServer::try_new` creates `TabServerState::from_workspace` (`src/server/mod.rs:495-519`), allocating a per-tab welcome `DocumentState` with the prototype text. Client `connect_with_workspace_root` sends `TabCommand::New` (`src/client/mod.rs:1021-1031`); the server binds the tab and sends `InitialDocument` plus the file-browser snapshot (`src/server/connection.rs:1202-1260`, `3025-3051`). `handshake_initial_state` receives the bound snapshot and `EditorWidget::with_initial_state` installs it into `PaneDocumentView`. Current welcome is therefore a server-owned editable document, not a Clay-owned actionable empty-state composition.
- **Primary-action flow:** `documents.clientOpenFileDialog` and `workspace.clientOpenFolderDialog` are recognized by `src/main.rs:91-125` / `1616-1647`; Linux dialogs run off the event loop, then `apply_native_dialog_completion` records the active-pane target. `EditorWidget::request_selected_file_open` / `request_selected_workspace_root` enqueue capability-bearing messages (`src/masonry_editor.rs:994-1025`, `src/client/mod.rs:597-634`). Server handlers validate the capability before opening the document or adding the root (`src/server/connection.rs:1019-1092`), preserving server authority.

- **Caret/completion flow:** `EditorSurface::route_key_with_event` applies local text first and derives a `UiReactivePriority` request (`src/editor/surface.rs:1882-1920`); `completion_request_event` stamps document/behavior versions, caret byte offset, and replacement range (`src/editor/surface.rs:3840-3859`). The pane enqueues it non-blockingly (`src/masonry_pane_document.rs:2061-2086`, `2505-2516`). The editor already computes caret geometry for paint/IME (`src/editor/surface.rs:3162-3180`), but that geometry is not yet transported to the completion overlay; this is the generic geometry gap for Task 4.
- **Result/projection flow:** `PaneDocumentView::apply_completion_result` drops mismatched request/document/behavior versions before creating a modeless `TransientMenuSession` (`src/masonry_pane_document.rs:1995-2007`). `completion_result_to_menu_session` maps each bounded result item once and emits the current empty/timeout/error status (`src/shell/transient_menu.rs:393-414`). `SduiNativeState::transient_overlays` adds the active session to the shared overlay projection; `TransientPackageOverlay::from_menu_session` maps it to the existing list/stack/status tree and origin anchor (`src/masonry_sdui.rs:572-585`, `src/shell/package_ui.rs:489-532`).
- **Overlay hosting:** `EditorWidget::sync_overlays` filters centered layers from pane-local overlays (`src/masonry_editor.rs:573-585`); `PackageOverlayHost::sync_overlays` retains matching widget IDs, reconciles changed trees, and removes stale children (`src/masonry_package_region.rs:3002-3070`). Layout currently uses `bottom_rect` — full main-pane width and `35%` height clamped to `120..=240` (`src/shell/package_ui.rs:718-726`, `878-885`) — then runs each hosted region's layout. Centered Command Centre sessions already use the separate retained window layer (`src/masonry_editor.rs:599-650`).

- **Primitive reuse matrix:**

  | Surface/concern | Existing catalog or implementation path | Entry decision |
  |---|---|---|
  | Welcome layout/actions | `panel`, `flex`, `stack`, `label`, `button`, `statusItem`; retained package-region widgets in `src/masonry_package_region.rs` | Compose existing primitives; no new package kind or filesystem authority. |
  | Shortcut hints | Internal `paint_kbd_hint` in `src/shell/primitives.rs:421-455`, token-backed `surface.kbd`/`border.kbd`/`typography.caption` | Existing helper is a chrome shell only and has no active call site/text rendering; use existing labels plus a generic internal completion only if evidence requires it, not a new package component. |
  | Status/loading/recovery | `EditorStatus`, `statusItem`, shared accessibility label helpers, `TransientMenuSession` | Reuse status and recovery/menu semantics; sanitize at the existing boundary. |
  | Completion/menu content | `list`, `stack`, `scroll`, `overlay`, `portal`, `TransientMenuSession`, `PackageRegionWidget`/`PackageListRow` | Reuse the retained shared menu host; only caret-origin geometry is a candidate generic gap. |
  | Menu chrome/accessibility | `paint_tooltip_shell`, `MenuA11y`, `Role::Menu`/`MenuItem`/`Status`, `PackageOverlayHost` | Preserve current roles, modal/modeless policy, selection, and focus restoration. |
  | Fixed/transient hosting | `PackagePanelHost`, `PackageOverlayHost`, `TransientMenuOrigin`, `SduiNativeState` local/centered projections | Keep Clay-owned slot and overlay ownership; centered Command Centre remains window-layer-only. |
  | Review evidence | `smoke-gui`/`ManagedServer`, checked-in configuration fixtures, structural observability, `tests/manual_smoke_docs.rs`, `tests/live_atspi_smoke.rs` | Extend existing paths; no screenshot framework or second event loop. |

- **Performance/security baseline:**
  - Three direct `target/debug/clay smoke-gui --config-fixture runtime-sdui` runs reached `clay client connected` in `58.110 ms`, `78.198 ms`, and `61.125 ms` from process start (median `61.125 ms`; startup-to-handshake only, advisory, excluding compilation and screenshot time). Temporary mode-700 homes/endpoints were removed after each run.
  - Completion projection is currently one pass over result items (`O(n)`), capped at 256 items; completion result payload is capped at 16 KiB, item labels at 128 chars, details at 256 chars, and accessibility labels at 256 chars (`src/perf/budgets.rs`). `PackageRegionWidget` retains one row widget per projected item; the current menu projection is a stack/list without a scroll wrapper, with no visible-row virtualization or dedicated completion layout timer yet.
  - Completion itself does not filter a result set on the client. Command Centre filtering scores the bounded catalogue on each query (`src/server/control_center.rs:94-144`); path filter-only updates score installed entries without filesystem work (`src/server/menu_sessions.rs:190-218`, `src/shell/path_browser.rs:254-301`). These are the current filter baselines, not completion-specific new work.
  - Existing deterministic guards passed: `shell::transient_menu` 20 tests, `shell::package_ui` 9 tests, `editor_performance_invariants::completion_hot_paths_use_inert_state_and_nonblocking_enqueue_only`, and `package_ui_conformance::catalog_is_drift_free_across_doc_enum_and_paint_path`. Commands: `cargo test --lib shell::transient_menu:: -- --test-threads=1`, `cargo test --lib shell::package_ui:: -- --test-threads=1`, and exact filters under `cargo test --test editor`.
  - Authority remains unchanged: welcome actions dispatch existing client commands; native dialogs issue selected-path capabilities; server validates `OpenSelectedFile`/workspace-root operations; package UI remains inert and cannot choose native bounds or execute client JavaScript.

- [x] Add a repeatable Linux GUI review fixture and artifact workflow
  - Acceptance Criteria:
    - Functional: One documented command launches isolated fixed-size default/loading/error/recovery/completion/command-centre fixtures and records screenshots plus accessibility observations under a caller-selected artifact directory.
    - Performance: Harness has fixed startup/interaction/cleanup deadlines and reuses normal `target/`; it adds no production work.
    - Code Quality: Extend `smoke-gui`, config fixtures, and existing structural observability; use a small stdlib/shell wrapper only if one command cannot express orchestration.
    - Security: Use mode-700 temporary config/IPC roots, fixture-only documents, no ambient `~/.config/clay`, no remote listener, and sanitized logs.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`, `docs/development/ui-observability.md`, `docs/development/build-and-test.md`.
      - XDG desktop portal screenshot contract used by current audit; computer-use-linux `get_app_state` workflow.
    - Options Considered:
      - New screenshot test framework/GPU goldens: rejected; production-faithful deterministic prerequisites remain absent.
      - Manual ad hoc screenshots: rejected; not repeatable.
      - Existing smoke fixtures plus thin capture/orchestration: chosen.
    - Chosen Approach:
      - Add named app states to existing fixture plumbing, fixed logical window dimensions, and a documented capture script/command; keep images review artifacts rather than hard CI goldens.
    - API Notes and Examples:
      ```bash
      cargo run -- smoke-gui --config-fixture ui-review-default
      scripts/capture-ui-review.sh --fixture ui-review-completion --output code-reviews/screenshots/<run>/
      ```
    - Files to Create/Edit:
      - `src/main.rs`: fixture/window-size support only if existing flags cannot cover it.
      - `tests/fixtures/configuration/ui-review-*/`: deterministic state fixtures.
      - `scripts/capture-ui-review.sh` (tentative): thin portal/computer-use orchestration.
      - `docs/development/launch-and-gui-smoke.md`, `docs/development/ui-observability.md`.
      - `tests/manual_smoke_docs.rs`: command/fixture documentation guard.
    - References:
      - `decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md`.
  - Test Cases to Write:
    - Each fixture boots without ambient config, reaches named state, and cleans server/socket/processes on timeout.
    - Missing screenshot/computer-use capability reports an unresolved manual gate, not success.

### Task 2 Evidence (2026-08-14)

- Added executable `scripts/capture-ui-review.sh` with six named fixtures, fixed `900×600` logical-size metadata, bounded server/client startup and AT-SPI probe deadlines, interactive checkpoints for completion/Command Centre, portal PNG capture, Clay-only accessibility dumps, mode-700 temporary HOME/XDG/socket roots, fixture-only retained artifacts, and cleanup of raw process logs/temp roots. The wrapper copies each checked-in fixture into private `HOME/.config/clay/init.js` and uses the normal `clay server`/`clay client` path so runtime watcher reloads exercise end-user configuration without touching repository files.
- Added deterministic fixtures under `tests/fixtures/configuration/ui-review-*/`: empty default/recovery, loading SDUI, invalid-theme runtime error, Rust completion with `Ctrl+Space`, and global Command Centre with `Ctrl+Alt+P`.
- Added the documented workflow and unresolved-safe prerequisite contract to `docs/development/launch-and-gui-smoke.md` and `docs/development/ui-observability.md`; `tests/manual_smoke_docs.rs::plan087_ui_review_harness_command_and_prerequisites_are_documented` locks command names, fixture inventory, output files, isolation, fixed size, and `UNRESOLVED`/exit-2 behavior.
- Retained live artifacts under `code-reviews/screenshots/2026-08-14-plan087-ui-review/{default,loading,error,recovery}/`: default, loading-shell, runtime-error, and disconnected/recovery captures all produced PNG plus AT-SPI output with `PASS` status. The loading label is present in the fixture's published tree but was not exposed by the host's initial accessible tree, so that artifact records the shell/default tree rather than claiming that label was visually reviewed; Task 5 must inspect this state.
- Validation passed: `bash -n scripts/capture-ui-review.sh`; `node --check tests/fixtures/configuration/ui-review-*/init.js`; `cargo fmt --all -- --check`; `git diff --check`; `cargo test --test protocol manual_smoke_docs` (22 passed); `cargo test --lib server::js_runtime::tests::smoke_config_fixture_publishes_runtime_sdui_snapshot -- --exact --test-threads=1`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets` (all suites/benches passed, one existing ignored live test); and `cargo audit` (0 vulnerabilities, 3 documented allowed unmaintained warnings).

- [ ] Replace the prototype welcome document with a useful Clay-owned entry state
  - Acceptance Criteria:
    - Functional: Fresh tabs show Open File, Open Folder, concise shortcut help, workspace/connection/runtime status, and actionable loading/error/recovery copy; opening a real document replaces entry content without altering canonical document authority.
    - Performance: Initial composition is bounded, contains no filesystem scan/recent-file query unless already available, and runs no JS/IPC/file I/O during paint/layout.
    - Code Quality: Compose cataloged retained primitives and existing command IDs; remove stale “Phase 4 IPC server” product copy from production paths; do not add a package-facing component kind unless primitive review proves composition impossible.
    - Security: Open actions route through existing native dialog and server grant paths; status text is sanitized; no recent-path leakage or implicit filesystem authority.
  - Approach:
    - Documentation Reviewed:
      - `src/server/mod.rs::TabServerState`, `src/server/document.rs`, `src/main.rs` client command dispatch.
      - Clay UI catalog and fetched content/accessibility rules for actionable empty states, specific labels, focus, async/error status.
    - Options Considered:
      - Rich text inserted into editable welcome document: rejected; remains a prototype and makes actions undiscoverable.
      - New independent UI framework/surface: rejected.
      - Clay-owned retained composition selected for the server-owned welcome identity: chosen; keeps ownership while using existing widgets/actions.
    - Chosen Approach:
      - Introduce one internal welcome-state composition in the pane content path; server still owns tab/workspace/document state and emits existing status, while client owns native presentation and command intents.
    - API Notes and Examples:
      ```text
      [Open File]  → documents.clientOpenFileDialog
      [Open Folder] → workspace.clientOpenFolderDialog
      Runtime error → Status/live text + retry guidance via existing runtime.reloadConfiguration command
      ```
    - Files to Create/Edit:
      - `src/server/mod.rs`, `src/server/document.rs`, `src/client/mod.rs`: remove/replace stale welcome text contract as needed.
      - `src/masonry_pane_document.rs` or a small internal `src/masonry_welcome.rs` (only if composition cannot remain local): retained welcome composition.
      - `src/main.rs`: reuse existing command dispatch; no new authority.
      - `src/shell/components.rs`, `src/shell/primitives.rs`: only proven generic gaps.
    - References:
      - `docs/reference/clay-js-api/documents/client-open-file-dialog.md` (existing command descriptor).
      - `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`.
  - Test Cases to Write:
    - Fresh tab shows all actions/status with exact roles/names; keyboard activation emits existing command IDs.
    - Loading, disconnected, runtime-error, recovery, narrow width, long sanitized workspace name.
    - Opening/reclaiming a real document removes entry state and preserves server document/lease rules.

- [ ] Give completion a compact caret-adjacent projection and dismiss empty/stale results
  - Acceptance Criteria:
    - Functional: Current non-empty completion appears adjacent to caret/line, clamps within active pane, has bounded width/height, scrolls long lists, keeps selected row visible, and accepts keyboard/IME-safe interaction. Empty/expired/rejected completion closes rather than showing “No completions”; provider timeout/error uses non-blocking status/recovery feedback without a blocking panel. Centered Command Centre results stay inside available window bounds, remain scrollable/reachable for 60+ results, and preserve modal containment/focus restoration.
    - Performance: Layout/render touches only visible bounded rows; no full-width relayout per keystroke; stale results are dropped before projection.
    - Code Quality: Reuse `TransientMenuSession`, `PackageOverlayHost`, `Portal`, `Scroll`, list rows, and one shared geometry helper; preserve centered command/path session semantics and modal ownership while fixing bounds/scroll.
    - Security: Exact request/document/version/behavior provenance checks remain before display/accept; packages cannot choose native anchor bounds or execute client JS.
  - Approach:
    - Documentation Reviewed:
      - `src/masonry_pane_document.rs:apply_completion_result`, `src/shell/transient_menu.rs::completion_result_to_menu_session`, `src/shell/package_ui.rs::bottom_rect` and overlay anchors.
      - Project pattern `package-ui-layout.md`: one shared session/overlay system.
    - Options Considered:
      - Tune 35% bottom panel height: rejected; wrong interaction model.
      - Separate completion overlay subsystem: rejected; duplicates focus/z/accessibility ownership.
      - Add an internal completion/caret anchor to shared host: chosen.
    - Chosen Approach:
      - Carry an internal completion origin/anchor projection with caret bounds from the active pane; dismiss empty/stale sessions before host reconciliation.
    - API Notes and Examples:
      ```rust
      enum TransientMenuOrigin { /* existing */, Completion }
      // Internal only; package-facing anchor enum remains unchanged.
      ```
    - Files to Create/Edit:
      - `src/shell/transient_menu.rs`: completion empty/error projection and internal origin.
      - `src/shell/package_ui.rs`: bounded caret-adjacent geometry.
      - `src/masonry_pane_document.rs`, `src/masonry_editor.rs`, `src/masonry_package_region.rs`: caret anchor/host reconciliation and a11y as needed.
      - `src/perf/budgets.rs`: explicit row/extent ceilings if not already represented.
    - References:
      - `.agents/skills/clay-ui/references/components.md` transient menu/completion entries.
  - Test Cases to Write:
    - Empty/current, empty/stale, timeout/error, non-empty, narrow pane, caret at each edge, multi-pane, scroll/selection visibility, IME preedit, keyboard accept/cancel, stale accept denial.
    - Centered command/path geometry remains centered, bounded, scrollable, and modal; 60+ result lists do not clip below the window.

- [ ] Bound package-authored transient-menu accessibility labels
  - Acceptance Criteria:
    - Functional: Every package-authored transient-menu item label is normalized through the shared bounded accessibility-text path before `MenuA11y` reaches Masonry; empty labels retain a safe item fallback, control characters/path separators are removed, and labels are truncated at the existing 256-character accessibility ceiling. Prompt, item, selected-state, result-count, query, selection, and close flows preserve current menu semantics.
    - Performance: Normalization is one bounded pass while constructing a menu session, remains O(visible items), and adds no work to paint, layout, typing, or accessibility passes beyond the existing item cap.
    - Code Quality: Reuse the existing accessibility sanitization/truncation helpers and menu budget constants; do not create a second label policy or duplicate menu projection path.
    - Security: Package-authored text cannot leak absolute paths, control characters, or unbounded content through AccessKit labels; server command/provenance validation remains unchanged.
  - Approach:
    - Documentation Reviewed:
      - `src/shell/package_ui.rs::MenuA11y`, `src/editor/accessibility.rs` sanitization helpers, `src/perf/budgets.rs` transient-menu limits, and `docs/development/accessibility.md`.
      - `.agents/skills/clay-ui/references/components.md` transient-menu/completion catalog and `.agents/skills/clay-ui/references/tokens.md` token-only UI rules.
    - Options Considered:
      - Preserve raw package labels: rejected; the existing item-count cap does not bound label size or content.
      - Sanitize separately in each Masonry widget/pass: rejected; it duplicates policy and permits inconsistent reachable labels.
      - Normalize once at `MenuA11y` construction with the shared ceiling: chosen.
    - Chosen Approach:
      - Keep menu display/action data intact while deriving bounded accessibility labels at the retained menu-session projection boundary; selected-state suffixes remain inside the final 256-character output.
    - API Notes and Examples:
      ```rust
      // Internal projection only; no package-facing label API changes.
      TransientPackageOverlay::from_menu_session(session) // bounded item labels
      ```
    - Files to Create/Edit:
      - `src/shell/package_ui.rs`: normalize item labels at `MenuA11y` construction.
      - `src/editor/accessibility.rs`: reuse or minimally extend the shared bounded text helper if required.
      - `src/masonry_package_region.rs`: consumer/accessibility regression cases.
      - `docs/development/accessibility.md`: record the item-label ceiling and boundary.
    - References:
      - Plan 086 Task 2 finding and Plan 086 Task 12 wiki page.
  - Test Cases to Write:
    - Empty, 255/256/257-character, control-character, separator/path-like, selected, query, selection, close, and 256-item menu cases; assert bounded labels and consumer-valid reachable trees.

- [ ] Add focused UI behavior, accessibility, and performance regression coverage
  - Acceptance Criteria:
    - Functional: Structural snapshots cover welcome states, bounded Command Centre overflow, completion geometry/dismissal, and sanitized menu labels; accessibility trees expose names, roles, status, selection, and modal/modeless containment correctly.
    - Performance: Deterministic guards bound rows/layout work; Criterion/advisory metrics record completion open/filter/layout without hard wall-clock promotion.
    - Code Quality: Behavioral tests protect state transitions; source-text assertions are used only for unique hot-path absence contracts.
    - Security: Tests cover stale provenance rejection and no ambient path/config leakage.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/ui-observability.md`, `docs/development/performance.md`, existing `benches/window_baselines.rs`.
    - Options Considered:
      - Pixel goldens: deferred by existing decision.
      - Structural + live evidence + bounded geometry benchmarks: chosen.
    - Chosen Approach:
      - Extend observable state and existing benchmark groups minimally.
    - API Notes and Examples:
      ```bash
      cargo test --lib masonry_pane_document
      cargo test --lib masonry_package_region
      cargo test --test editor editor_performance_invariants::
      cargo bench --bench window_baselines --no-run
      ```
    - Files to Create/Edit:
      - In-module tests in changed Masonry/shell files.
      - `tests/editor_performance_invariants.rs`, `tests/performance_budgets.rs`, `benches/window_baselines.rs` only for behavioral/budget coverage.
      - `docs/development/performance.md`.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`.
  - Test Cases to Write:
    - State matrix from prior tasks; 60+ Command Centre results remain reachable inside the viewport; oversized/control-character item labels remain bounded; no completion work in ordinary paint when menu absent; list work bounded by visible/capped rows.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: Capture default, loading, disconnected, runtime error/recovery, opened document, non-empty completion at pane edges, empty completion dismissal, long-list scroll, 60+ result Command Centre overflow/scroll, sanitized transient-menu labels, and narrow/wide states.
    - Performance: Typing/filtering/selection feels immediate; no full-pane jump or overlay duplication.
    - Code Quality: Evidence paths/findings are recorded; screenshot defects block completion or become explicit prioritized follow-ups.
    - Security: Fixture content contains no secrets/absolute paths; tree labels remain sanitized.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/ui-visual-review.md`; UI review harness task output.
    - Options Considered:
      - Automated structure alone: rejected.
      - Real screenshot + `get_app_state` + keyboard-only flow: chosen.
    - Chosen Approach:
      - Use review fixtures at fixed and narrow/wide dimensions; query accessibility before/after interactions.
    - API Notes and Examples:
      ```text
      get_app_state → open/activate/type/scroll/cancel → get_app_state → screenshot
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-14-plan087-ui-foundation/*.png`.
      - This plan: findings.
    - References:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write:
    - Focus visibility/order, exact accessible names/selection/status, empty dismissal, centered modal containment, focus restoration.

- [ ] Update UI catalogs and package authoring contract
  - Acceptance Criteria:
    - Functional: Document changed internal welcome/completion surfaces, centered result bounds, transient-menu item-label sanitization, origins, geometry, dismissal, focus, and accessibility; package-facing API remains unchanged unless explicitly added.
    - Performance: Document caps and hot-path policy.
    - Code Quality: Catalog, package guide, navigation page, and drift tests agree.
    - Security: State that packages cannot request caret-native bounds, direct Masonry widgets, raw CSS, client JS, or dialog authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md`, `docs/reference/ui-components.md`, catalog drift tests.
    - Options Considered:
      - Leave internal changes undocumented: rejected.
      - Update authoritative catalog/navigation once implementation settles: chosen.
    - Chosen Approach:
      - Keep package contract truthful and additive.
    - API Notes and Examples:
      ```text
      Completion anchor: Clay-internal; package overlays keep documented anchors only.
      ```
    - Files to Create/Edit:
      - `.agents/skills/clay-ui/references/components.md`, `references/tokens.md` only if token changes.
      - `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`.
      - `tests/package_ui_conformance.rs`, `tests/primitives_docs.rs` as drift coverage requires.
    - References:
      - `.agents/skills/create-plan/references/clay.md` UI/package authoring requirements.
  - Test Cases to Write:
    - Catalog and package guide list the same implemented/package-facing surfaces and reject internal completion anchor declarations.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Welcome actions reuse documented `documents.clientOpenFileDialog`, `workspace.clientOpenFolderDialog`, and existing recovery commands; inventory all changed Rust visibility.
    - Performance: No new JS round trip is added to completion display/input.
    - Code Quality: Any new public capability follows bare `<domain>.<name>` IDs, facade/docs/index/registry coverage; internal presentation remains private.
    - Security: No arbitrary path, native bounds, raw menu session, or widget handle is exposed.
  - Approach:
    - Documentation Reviewed:
      - Project patterns `clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `documentation-as-code.md`.
    - Options Considered:
      - New welcome-specific commands: rejected unless existing commands cannot express an action.
      - Reuse current commands: chosen.
    - Chosen Approach:
      - Verify registry and visibility; add no API by default.
    - API Notes and Examples:
      ```text
      documents.clientOpenFileDialog
      workspace.clientOpenFolderDialog
      runtime.reloadConfiguration
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`, `docs/index.md`, generated registry only for a proven new public capability.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API Task.
  - Test Cases to Write:
    - Welcome buttons emit existing IDs; doc registry/visibility mapping stays complete.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Confirm entry-state and completion geometry/dismissal need no hidden configuration; existing keybinding/theme/typography APIs keep working.
    - Performance: No config parsing in paint/layout/keypress paths.
    - Code Quality: If a behavior-changing option is genuinely required, expose it as a documented Clay JS API and update the canonical example.
    - Security: Configuration cannot supply arbitrary overlay coordinates, raw style values, paths, or callbacks.
  - Approach:
    - Documentation Reviewed:
      - `configuration-system.md`, `docs/reference/clay-js-api/configuration.md`, `examples/init.js`.
    - Options Considered:
      - Configurable completion geometry immediately: rejected (YAGNI).
      - Good token-driven defaults: chosen.
    - Chosen Approach:
      - Preserve current configuration surface and record no-new-API result unless implementation proves otherwise.
    - API Notes and Examples:
      ```bash
      node --check examples/init.js
      ```
    - Files to Create/Edit:
      - Config/API docs, registry, and `examples/init.js` only if a new setting is introduced.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`.
  - Test Cases to Write:
    - Existing theme/typography/keybinding example loads cleanly with new UI states.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: Add/execute steps for entry states, primary actions, completion placement/dismissal/scroll, multi-pane, IME, centered command centre non-regression, and review harness.
    - Performance: Record typing/filter/scroll feel in module 11.
    - Code Quality: Use stable step IDs and exact expected/negative outcomes.
    - Security: Verify open actions still require user dialog/validated grant and stale completion cannot apply.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`; modules 01, 03, 04, 07, 10, 11, 13.
    - Options Considered:
      - Plan-only evidence: rejected.
      - Maintain reusable manual modules: chosen.
    - Chosen Approach:
      - Update only affected modules and coverage matrix.
    - API Notes and Examples:
      ```bash
      cargo build
      scripts/capture-ui-review.sh --fixture ui-review-default --output <artifact-dir>
      ```
    - Files to Create/Edit:
      - `test-plan/01-launch-and-connection.md`, `03-files-and-workspace.md`, `04-core-editing.md`, `10-keybindings-and-commands.md`, `11-performance.md`, `13-window-splits.md`, `test-plan/index.md`.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Manual Test Plan Task.
  - Test Cases to Write:
    - Manual state matrix and negative checks described above.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki explains review harness, welcome-state ownership/flow, completion lifecycle/geometry, focus/accessibility, and test commands; index links pages.
    - Performance: Document bounded rows/layout and no-hot-path authority work.
    - Code Quality: Include source/test paths, invariants, and extension guidance.
    - Security: Explain dialog/grant and stale completion boundaries without duplicating public API docs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Per-task wiki churn: rejected.
      - One final synchronized update: chosen.
    - Chosen Approach:
      - Update existing implementation pages or add one focused page, then link index.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/masonry-shell.md
      docs/wiki/modules/pane-document-views.md
      docs/wiki/modules/transient-menu-session.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, relevant modules above, and a welcome-state page only if needed.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`.
  - Test Cases to Write:
    - Manual wiki index/link and content review; documentation drift tests pass.

## Compromises Made

- GPU pixel goldens remain deferred because current Masonry testing is CPU-only and not production-renderer faithful. The plan delivers repeatable live artifacts plus deterministic structural checks instead.

## Further Actions

- Broad visual-system modernization is deliberately deferred to Plan 088 after these foundations are stable.
