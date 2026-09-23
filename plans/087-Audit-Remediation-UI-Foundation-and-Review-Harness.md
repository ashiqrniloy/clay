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
      - the UI guidance current at execution time, `vercel-labs/web-design-guidelines`, `ibelick/fixing-accessibility`; current interface guidelines fetched from their official source.
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

- Added executable `scripts/capture-ui-review.sh` with six named fixtures, fixed `900×600` logical-size metadata, bounded server/client startup and AT-SPI probe deadlines, interactive checkpoints for completion/Command Centre, portal PNG capture, Clay-only accessibility dumps, mode-700 temporary HOME/XDG/socket roots, fixture-only retained artifacts, and cleanup of raw process logs/temp roots. The wrapper copies each checked-in fixture into private `HOME/.config/clay/init.js` and uses the normal `clay server`/`clay client` path so runtime watcher reloads exercise end-user configuration without touching repository files; the document-bearing interactive layout uses a valid v2 leaf split tree.
- Added deterministic fixtures under `tests/fixtures/configuration/ui-review-*/`: empty default/recovery, loading SDUI, invalid-theme runtime error, Rust completion with `Ctrl+Space`, and global Command Centre with `Ctrl+Alt+P`.
- Added the documented workflow and unresolved-safe prerequisite contract to `docs/development/launch-and-gui-smoke.md` and `docs/development/ui-observability.md`; `tests/manual_smoke_docs.rs::plan087_ui_review_harness_command_and_prerequisites_are_documented` locks command names, fixture inventory, output files, isolation, fixed size, and `UNRESOLVED`/exit-2 behavior.
- Retained live artifacts under `code-reviews/screenshots/2026-08-14-plan087-ui-review/{default,loading,error,recovery}/`: default, loading-shell, runtime-error, and disconnected/recovery captures all produced PNG plus AT-SPI output with `PASS` status. The loading label is present in the fixture's published tree but was not exposed by the host's initial accessible tree, so that artifact records the shell/default tree rather than claiming that label was visually reviewed; Task 5 must inspect this state.
- Validation passed: `bash -n scripts/capture-ui-review.sh`; `node --check tests/fixtures/configuration/ui-review-*/init.js`; `cargo fmt --all -- --check`; `git diff --check`; `cargo test --test protocol manual_smoke_docs` (22 passed); `cargo test --lib server::js_runtime::tests::smoke_config_fixture_publishes_runtime_sdui_snapshot -- --exact --test-threads=1`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets` (all suites/benches passed, one existing ignored live test); and `cargo audit` (0 vulnerabilities, 3 documented allowed unmaintained warnings).

- [x] Replace the prototype welcome document with a useful Clay-owned entry state
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

### Task 3 Evidence (2026-08-14)

- Replaced the stale server welcome text with an empty server-owned sentinel in `src/server/mod.rs` and `src/server/document.rs`; the client contract test now expects an empty initial snapshot. Server tab/workspace/document authority and leases remain unchanged.
- Added the crate-private `src/masonry_welcome.rs` retained entry surface. It renders `Welcome to Clay`, Open File/Open Folder, shortcut help, basename-only workspace text, connection/access state, and bounded sanitized runtime guidance. Buttons expose AccessKit `Click` and keyboard activation through the existing `documents.clientOpenFileDialog` and `workspace.clientOpenFolderDialog` client-local routes; no JS, IPC, filesystem scan, recent-path query, or package-facing component kind was added to paint/layout.
- Integrated the welcome pod into `EditorWidget`/`PaneDocumentView`: fresh bootstrap and local-fallback views show it, text input/editor pointer handling is disabled while visible, and a real `DocumentOpened` hides it and restores the multiline editor role/input path. The welcome child stays registered and is stashed when hidden, preserving Masonry traversal invariants. Workspace/status changes refresh the retained render state without doing work in paint/layout.
- Added structural/accessibility coverage for exact button roles/actions, welcome status/workspace labeling, the real `accesskit_consumer::Tree` first update, document-open replacement, bounded sanitized long diagnostics/workspace names, narrow geometry, and client-local command routing. Corrected the shared accessibility truncator so its ellipsis remains inside the declared character ceiling. Updated the region-lock integration fixture for the now-empty default document.
- Validation passed: `cargo fmt --all -- --check`; `git diff --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --lib masonry_welcome -- --test-threads=1`; `cargo test --lib masonry_editor::tests::welcome_entry_exposes_actions_and_hides_after_document_open -- --exact --test-threads=1`; `cargo test --all-targets`; all benches completed successfully; and `cargo audit` reported 0 vulnerabilities with the 3 already-documented allowed unmaintained warnings. Plan 087 task 5 remains responsible for visual review of loading/error/recovery and completion states.

- [x] Give completion a compact caret-adjacent projection and dismiss empty/stale results
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

### Task 4 Evidence (2026-08-14)

- Added the Clay-internal `TransientMenuOrigin::Completion` path with a fixed-point `CompletionAnchor`; `completion_result_to_menu_session` remains modeless and inert, while `PaneDocumentView` injects the IME-aware caret bounds only after active request/document/version/behavior checks.
- Replaced the completion bottom-panel projection with one shared `completion_overlay_rect` helper: below-caret then above-caret placement, active-pane clamping, 480 logical-pixel width cap, eight visible-row cap, and zero-size-safe fallback. The same retained `PackageOverlayHost` continues to own z-order, action routing, and centered modal containment.
- Wrapped transient-menu lists in the existing retained `SduiScrollViewport`; selection updates set a row target so long Command Centre and completion lists keep the selected row visible without changing package-facing anchors or APIs. Completion items retain local accept metadata and expose no command action targets.
- Empty results dismiss the current menu; stale document/version/behavior results close the matching completion surface; timeout/provider-error results use non-blocking `completion.provider_timeout` / `completion.provider_error` status diagnostics. Added structural overlay observation using the same geometry helper.
- Added/updated tests: `src/shell/transient_menu.rs` completion origin projection; `src/shell/package_ui.rs` caret-edge/width geometry; `src/masonry_pane_document.rs` non-empty anchor plus empty/error/stale dismissal; `src/masonry_package_region.rs` selected-row scrolling and 60-result centered containment; `src/masonry_sdui.rs` bounded observable geometry; existing centered-menu tests remain green.
- Validation passed: `cargo fmt --all`; targeted shell/pane/package/SDUI/editor tests; `cargo test --test protocol performance_budgets -- --test-threads=1`; `cargo test --test protocol manual_smoke_docs -- --test-threads=1`; and `cargo clippy --all-targets -- -D warnings`. Final Linux gates also passed: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test --all-targets` (all suites/benches; one ignored live AT-SPI test), `cargo bench --no-run`, and `cargo audit` (0 vulnerabilities, 3 documented allowed warnings). The interactive completion harness was attempted twice; AT-SPI reached the isolated Clay window, but host keyboard/window targeting could not focus the editor and the run remains `UNRESOLVED`, not a false visual pass.

- [x] Bound package-authored transient-menu accessibility labels
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

### Task 5 Evidence (2026-08-14)

- Added `compose_menu_item_accessibility_label` in `src/editor/accessibility.rs` as the shared bounded semantic-label path. `TransientPackageOverlay::from_menu_session` applies it once per item before `MenuA11y` reaches Masonry; it removes control characters/path separators, falls back from invalid accessibility text to the display label and then `Menu item`, and keeps the selected suffix inside the 256-character ceiling. Display labels, action IDs, provenance, query text, selection state, result counts, and close behavior remain unchanged.
- `PackageRegionWidget` now consumes the already-final label instead of appending an unbounded ` selected` suffix during the accessibility pass. No package-facing API, component kind, command authority, or configuration surface changed.
- Added helper boundary tests for 255/256/257-character inputs, controls/separators, empty fallback, and selected-label sizing; added a 256-item package-authored menu test that inspects the real tree, asserts every semantic label is bounded/sanitized, and feeds the update through `accesskit_consumer::Tree` without panic.
- Updated `docs/development/accessibility.md`, `docs/wiki/modules/transient-menu-session.md`, `docs/wiki/modules/masonry-sdui-region.md`, and the Plan 086 accessibility wiki follow-up to document the label boundary and close the resolved ceiling finding.
- Validation passed: `cargo fmt --all -- --check`; `cargo test --lib editor::accessibility:: -- --test-threads=1`; `cargo test --lib masonry_package_region:: -- --test-threads=1`; `cargo test --test protocol primitives_docs -- --test-threads=1`; `cargo test --test editor package_ui_conformance -- --test-threads=1`; `cargo clippy --all-targets -- -D warnings`; and `git diff --check`. Full plan gates remain covered by the preceding Task 4 run; final changed-path validation also passed `cargo check --all-targets`, `cargo test --all-targets` (all suites/benches; one ignored live AT-SPI test), `cargo bench --no-run`, and `cargo audit` (0 vulnerabilities, 3 documented allowed warnings).

- [x] Add focused UI behavior, accessibility, and performance regression coverage
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
      - `src/masonry_welcome.rs`, `src/masonry_pane_document.rs`, `src/masonry_package_region.rs`, `src/shell/package_ui.rs`: focused state/accessibility/geometry tests.
      - `src/perf/baselines.rs`, `benches/window_baselines.rs`: benchmark-only completion projection, filter, and layout paths.
      - `tests/editor_performance_invariants.rs`, `tests/performance_budgets.rs`, `tests/rust_visibility_api_mapping.rs`: hot-path, budget/documentation, and no-public-facade guards.
      - `docs/development/performance.md`.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`.
  - Test Cases to Write:
    - State matrix from prior tasks; 60+ Command Centre results remain reachable inside the viewport; oversized/control-character item labels remain bounded; no completion work in ordinary paint when menu absent; list work bounded by visible/capped rows.

### Task 6 Evidence (2026-08-14)

- Added focused behavioral/accessibility coverage: `WelcomeState` now has a loading/connected/runtime-error/local-fallback/disconnected matrix with basename-only workspace labels; completion results with foreign document or behavior provenance are rejected before replacing the active menu; completion geometry is source-guarded to the eight-row/480-logical-pixel caps and absent from editor/overlay paint; and a real completion overlay exposes modeless `Menu`/selected `MenuItem` semantics with no command targets and passes through `accesskit_consumer::Tree`.
- Added `completion_overlay_height_uses_visible_row_cap` and retained the existing 60-result centered containment and 256-item sanitized-label consumer tests. No ambient config/path access was added; stale completion identity remains checked by document/version/behavior metadata before projection.
- Added benchmark-only helpers in `src/perf/baselines.rs` and three `window_baselines` Criterion groups: `completion_open_baselines`, `completion_filter_baselines`, and `completion_layout_baselines`. A local optimized 10-sample run measured open medians of `2.41/13.40/89.95/362.10 µs` for `1/8/60/256` items, filter medians of `12.21/73.61/416.08 µs` for `16/60/256` candidates, and layout medians of `0.98/0.88/0.89 µs` at representative caret positions. These remain advisory, not wall-clock CI thresholds; the benchmark shape changed from the earlier synthetic helper, so its Criterion comparison is not a product regression gate.
- Updated `docs/development/performance.md` and `tests/performance_budgets.rs` with the structural hard-gate/advisory-benchmark contract, commands, and local measurements. Benchmark-only helpers are denied from deno ops/facades by the existing Rust visibility guard.
- Validation passed: targeted welcome/pane/package-region tests; `cargo test --test editor editor_performance_invariants:: -- --test-threads=1` (32 passed); `cargo test --test protocol performance_budgets -- --test-threads=1` (19 passed); `cargo test --test security rust_visibility_api_mapping -- --test-threads=1` (11 passed); all three short Criterion groups; `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets` (all suites/benches passed, one ignored live AT-SPI test); `cargo bench --no-run`; and `cargo audit` (0 vulnerabilities, 3 documented allowed warnings).

- [x] Perform visual screenshot and accessibility review of changed UI
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

### Task 7 Evidence (2026-08-15)

- Completed a real Linux review with `get_app_state` before interaction, an isolated mode-700 server/client root, live AT-SPI dumps, native Open File selection, X11 keyboard delivery, and portal screenshots cropped to the Clay window. No secrets or absolute paths were retained.
- PASS artifacts cover default welcome, runtime error, disconnected/recovery, opened `review.md`, non-empty completion, empty completion dismissal, unfiltered 66-result Command Centre, filtered 8-result Command Centre, and sanitized package labels. Evidence is under `code-reviews/screenshots/2026-08-14-plan087-ui-foundation/`; `review-log.md` records method and state-by-state results.
- Non-empty completion exposed a `Menu` with 16 selected-capable items, `Recovery: Completion`, and 480×340 logical bounds. Empty requests delivered `CompletionResult { status: Empty, items: [] }` and left no overlay. Command Centre exposed modal `Dialog`/`Menu`, selection, result count, 66 semantic items, and bounded filtered results; package labels contained no path separators.
- Finding `P1-087-UI-1`: live renderer containment is not complete. Completion rows paint below the 480×340 shell and 66-result Command Centre rows paint below the 640×220 centered shell despite scrollbar presence; structural size/scroll tests pass but miss this renderer-level child clipping/accessibility containment defect. Shared scroll-host containment must be fixed before Plan 087 can claim bounded visual completion/Command Centre surfaces.
- Loading remains an explicit observability limitation: the fixture publishes its loading SDUI tree during watcher reload, but this host's initial AT-SPI tree exposes the welcome shell instead. Narrow/wide resizing remains unresolved because the host has no safe window-list/resize backend; no false visual pass was claimed. No production code was kept from this review task.
- Validation for the review artifacts: `bash -n scripts/capture-ui-review.sh`, portal PNG capture/crop, Python GI-Atspi dumps, and live client/server survival. Existing Linux gates from Task 6 remain green; the shared containment finding is a follow-up, not silently waived.

- [x] Update UI catalogs and package authoring contract
  - Acceptance Criteria:
    - Functional: Document changed internal welcome/completion surfaces, centered result bounds, transient-menu item-label sanitization, origins, geometry, dismissal, focus, and accessibility; package-facing API remains unchanged unless explicitly added.
    - Performance: Document caps and hot-path policy.
    - Code Quality: Catalog, package guide, navigation page, and drift tests agree.
    - Security: State that packages cannot request caret-native bounds, direct Masonry widgets, raw CSS, client JS, or dialog authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`, `.agents/skills/clay-ui/references/components.md`, `.agents/skills/clay-ui/references/tokens.md`, `docs/reference/packages/creating-packages.md`, `docs/reference/ui-components.md`, `docs/reference/primitives/shell-layout-strategy.md`, and catalog drift tests.
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
      - `.agents/skills/clay-ui/references/components.md` (internal welcome/completion/centered surface catalog; no token changes, so `tokens.md` remains unchanged).
      - `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`, `docs/reference/primitives/shell-layout-strategy.md`.
      - `tests/primitives_docs.rs` for cross-document contract drift; `tests/package_ui_conformance.rs` remains the existing code/catalog guard.
    - References:
      - `.agents/skills/create-plan/references/clay.md` UI/package authoring requirements.
  - Test Cases to Write:
    - Catalog, package guide, shell strategy, and navigation page agree on Clay-owned welcome/completion/centered surfaces, 8-row/480-pixel completion caps, centered bounds, sanitized labels, hot-path limits, and the four package anchors; drift test rejects stale bottom-overlay/`SduiNativeState` completion claims and internal anchor declarations.

### Task 8 Evidence (2026-08-15)

- Updated `.agents/skills/clay-ui/references/components.md` with the Plan 087 Clay-owned Welcome entry surface, caret/IME completion projection, internal `Completion`/`Centered` origins, 8-visible-row/480-logical-pixel completion caps, centered Command Centre bounds, scroll/accessibility ownership, and bounded transient-menu label policy. No token entries changed; `tokens.md` remains authoritative and unchanged.
- Updated `docs/reference/packages/creating-packages.md` with the additive-only package boundary: welcome actions reuse existing client commands; completion remains a modeless Clay-internal projection with stale/empty/error dismissal and status diagnostics; Command Centre/Path Browser remain centered Clay-owned surfaces; package overlay anchors remain `working-area`/`active-pane`/`main`/`pointer`; package labels are normalized by `compose_menu_item_accessibility_label`; packages receive no caret-native bounds, Masonry widgets, raw CSS, client JavaScript, or dialog authority.
- Updated `docs/reference/ui-components.md` and `docs/reference/primitives/shell-layout-strategy.md` so navigation, shell vocabulary, component status, overlay anchors, and package guide agree. Removed stale pre-Plan-087 completion renderer claims and stale Phase 18.3 deferred-kind claims from the current contract references. The live renderer containment follow-up `P1-087-UI-1` remains explicitly recorded as host work; no package API was added.
- Added `plan087_ui_authoring_contract_is_consistent_across_catalog_and_guides` in `tests/primitives_docs.rs`. It checks cross-document Plan 087 markers, package-anchor allowlist, internal-origin rejection, cap/label/security wording, unchanged public surface, and absence of the retired completion renderer wording. Existing `package_ui_conformance` catalog/code guards remain green.
- Validation passed: `cargo fmt --all -- --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --test protocol primitives_docs -- --test-threads=1` (25 passed); `cargo test --test editor package_ui_conformance -- --test-threads=1` (10 passed); `cargo test --all-targets` (all suites/benches passed, one ignored live AT-SPI test); and `cargo audit` (0 vulnerabilities, 3 documented allowed warnings).

- [x] Create or verify Clay JS APIs for public programmatic surfaces
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

### Task 9 Evidence (2026-08-15)

- Inventory of every Plan 087 source change (git diff over `src/`) found no new bare-public server function, deno_core op, JS facade export, or API doc/registry entry: the only new `pub` items are the `#[doc(hidden)]` bench proxies `completion_open_projection_work`/`transient_menu_filter_work`/`completion_layout_work` (src/perf/baselines.rs) and the perf constants `COMPLETION_MAX_VISIBLE_ROWS`/`COMPLETION_MAX_WIDTH_PX` (src/perf/budgets.rs), all bench/catalog infrastructure with no runtime surface.
- Welcome actions verified to reuse existing documented command IDs only: `documents.clientOpenFileDialog` and `workspace.clientOpenFolderDialog` (OPEN_FILE_COMMAND/OPEN_FOLDER_COMMAND in src/masonry_welcome.rs), both present in `docs/generated/clay-js-api-registry.json` with docs under `docs/reference/clay-js-api/`; no welcome-specific command was added.
- All Plan 087 presentation internals stay crate-private or private: `pub(crate) mod masonry_welcome` (src/lib.rs) with `WelcomeState`/`WelcomeWidget` pub(crate) and `WelcomeButton` private; `compose_menu_item_accessibility_label` (src/editor/accessibility.rs) and `completion_overlay_rect` (src/shell/package_ui.rs) pub(crate); `TransientMenuOrigin` (with the internal `Completion` variant) and `CompletionAnchor` pub(crate) (src/shell/transient_menu.rs); no new bare `pub` in src/masonry_editor.rs, src/masonry_pane_document.rs, src/masonry_package_region.rs, src/masonry_sdui.rs, or the server files.
- Package-facing anchor contract unchanged: `VALID_OVERLAY_ANCHORS` remains exactly `working-area`/`active-pane`/`main`/`pointer` (src/server/ui.rs) and `PackageOverlayAnchor::parse` never produces the internal Completion/Centered anchors, so packages cannot request caret-native bounds, raw menu sessions, or widget handles.
- Added `plan087_welcome_and_completion_internals_are_not_public_programmatic_surfaces` in tests/rust_visibility_api_mapping.rs (security suite) locking the pub(crate)/private declarations, the welcome command reuse, the four-anchor allowlist, the parse-scope anchor rejection, and absence of all eight internal names from `src/server/ops/*`, `runtime/js/*`, and the generated registry while asserting both welcome command IDs remain in the registry.
- Validation passed: new visibility test (1 passed), full `rust_visibility_api_mapping` suite (12 passed), `clay_js_*` doc-registry/facade/inventory suites (55 passed), and the security test target runs clean. No new API, facade, or registry entry was added.

- [x] Create or verify Clay configuration APIs
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

### Task 10 Evidence (2026-08-15)

- No new configuration API was needed: the `clay:configuration` facade stays closed at exactly six exports (locked by `configuration_surface_is_closed_and_security_controls_are_not_properties` in tests/clay_js_api_inventory.rs), and no new option key, registry entry, or `examples/init.js` change was introduced.
- Verified no hidden configuration exists for the Plan 087 surfaces: welcome entry state and completion projection geometry/dismissal have zero configuration lookups — `rg configuration|set_package_option` across src/masonry_welcome.rs, src/shell/package_ui.rs, src/shell/transient_menu.rs, src/masonry_pane_document.rs, src/masonry_editor.rs, src/masonry_package_region.rs, and src/masonry_sdui.rs finds only prose/diagnostic strings. `completion_overlay_rect` reads compiled budget constants (`COMPLETION_MAX_VISIBLE_ROWS`/`COMPLETION_MAX_WIDTH_PX`) plus cached typography/token metrics; the centered Command Centre width is the validated `dimension.overlay.centered.width` design token, not a configuration key. No config parsing exists in paint/layout/keypress paths.
- Extended `plan060_internal_security_and_performance_controls_are_not_configurable` (src/server/configuration.rs) so `setPackageOption` fails closed with `unsupported package option` for `completion.maxVisibleRows`, `completion.maxWidthPx`, `completion.anchor`, `welcome.enabled`, `welcome.entryState`, and `centered.overlayWidth` — configuration cannot supply arbitrary overlay coordinates, raw style values, paths, or callbacks, and the completion anchor stays Clay-internal.
- Canonical example verified against the new UI states: `node --check examples/init.js` passes and the three hermetic config tests (`example_configuration_loads_cleanly_and_applies_effects`, `control_center_opens_filters_activates_and_cancels`, `runtime_generation_replacement_cancels_open_control_center`) all pass unchanged with the welcome/completion-bearing client surface.
- docs/reference/clay-js-api/configuration.md already states completion menu geometry, item count, bounds, and focus policy are Clay-owned compiled constants and not hidden `init.js` keys; no doc or registry edit was needed.

- [x] Execute and update the manual test plan (test-plan/)
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

### Task 11 Evidence (2026-08-15)

- Added Plan 087 step tables with stable IDs to six modules: 01 launch (L12–L14 welcome entry state + review harness contract + no-stale-copy), 03 files/workspace (F32–F37 welcome entry state, native Open File/Open Folder dialogs, welcome-return after close, no-path-leak negative), 04 core editing (E16–E21 completion placement/dismissal/scroll, empty-result dismissal, stale-accept negative, IME coexist), 10 keybindings (K69–K72 fixture `completion.trigger` binding, Command Centre 60+ non-regression, filter, menu key containment), 11 performance (Q11–Q14 completion caps/feel, scroll, Command Centre feel, non-blocking under pending edits), 13 window splits (S33–S35 completion-in-split anchoring, welcome-return on pane close); index.md gained a Plan 087 coverage-matrix row, module-map updates for 04/11, and a task 11 execution record summary.
- Executed on real Linux (X11-backend clay client on the review host) with an isolated mode-700 root and the `ui-review-completion` fixture init.js: PASS for the welcome entry state (sanitized labels, `Ready to edit; Open a file or folder…`, `Open File`/`Open Folder` buttons, no `Phase 4 IPC server` copy); PASS for `Open File` → native Nautilus dialog → `review.md` opened as doc 3 with basename-only labels; PASS for the live completion popup (Menu 480×340 at caret, 16 `@clay/markdown` items, ≤ 8 visible rows, selected row, modeless — editor stayed focused); PASS for Escape dismissal and for empty-result dismissal (`status: Empty`, no popup, no blocking `No completions` panel, no diagnostic); client and server stayed alive throughout.
- **BLOCKED by host (not a false pass):** this session's xdg-desktop-portal keyboard delivery could not hold Ctrl across the two strokes of `Ctrl+X Ctrl+P` (pending-chord timeout ~1.5 s), so Command Centre/split re-runs were not repeated in this instance; the Command Centre open/filter/Escape round trip with 66 results was verified live earlier in this plan (task 7 captures, same build) and split/welcome flows carry plan 086 manual evidence plus automated coverage. `P1-087-UI-1` remains tracked in Further Actions.
- Per-module records: [01](test-plan/01-launch-and-connection.md#linux-execution-record-plan-087-task-11-2026-08-15), [03](test-plan/03-files-and-workspace.md#linux-execution-record-plan-087-task-11-2026-08-15), [04](test-plan/04-core-editing.md#linux-execution-record-plan-087-task-11-2026-08-15), [10](test-plan/10-keybindings-and-commands.md#linux-execution-record-plan-087-task-11-2026-08-15), [11](test-plan/11-performance.md#linux-execution-record-plan-087-task-11-2026-08-15), [13](test-plan/13-window-splits.md#linux-execution-record-plan-087-task-11-2026-08-15), plus the [index summary](test-plan/index.md#plan-087-task-11-linux-execution-record-2026-08-15).

- [x] Update or verify the code wiki after implementation
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

### Task 12 Evidence (2026-08-15)

- Added one focused wiki page [Repeatable UI Review Harness](docs/wiki/modules/ui-review-harness.md) (project-wiki template: Source/Overview/How It Works/Fixtures/Artifacts/Invariants/Related) documenting `scripts/capture-ui-review.sh --fixture <ui-review-*> --output <dir>`, the six fixture states, mode-700 isolation, the watcher-reload fixture path (init.js copy + touch instead of `--config-fixture`), the AT-SPI probe's app-index scan and per-call timeouts, `review.status` PASS/UNRESOLVED (exit 2) semantics with no false pass, the X11-backend window note and multi-stroke chord limitation, screenshot-as-review-artifact stance, and the manual_smoke_docs drift guard.
- masonry-shell.md gained a 'Plan 087: welcome hosting, completion projection, and review harness' section (welcome hosting in pane hosts with no new authority, completion overlay hosting through `PackageOverlayHost` with `completion_overlay_rect` geometry, modeless focus/accessibility for welcome Group/Status/Buttons and completion Menu, and the P1-087-UI-1 follow-up); pane-document-views.md gained the focus/accessibility paragraph (modeless completion, welcome `STATUS` virtual node, 256-char ceiling, consumer validation, harness cross-link) and a Related link; transient-menu-session.md gained the harness cross-link in Related; index.md links the new page and updated the masonry-shell blurb for Plan 087.
- All links resolve (page is discoverable from the master index) and the documentation drift guard `primitives_docs` passes 25/25, including `wiki_index_links_every_wiki_page`. This is the final synchronized wiki update for the plan — no per-task wiki churn.

## Compromises Made

- GPU pixel goldens remain deferred because current Masonry testing is CPU-only and not production-renderer faithful. The plan delivers repeatable live artifacts plus deterministic structural checks instead.

## Further Actions

- **P1-087-UI-1:** Fix shared retained scroll-host clipping/accessible containment so live Completion and 60+ Command Centre rows stay inside their painted shells; add a renderer-level regression capture before Plan 087 closes.
- Broad visual-system modernization is deliberately deferred to Plan 088 after these foundations are stable.
