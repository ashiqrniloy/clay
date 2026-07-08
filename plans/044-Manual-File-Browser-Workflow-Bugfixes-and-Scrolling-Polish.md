# Manual File Browser Workflow Bugfixes and Scrolling Polish

## Objectives

- Make the normal `cargo run` + user `~/.config/clay/init.js` file-browser workflow work on Linux/GNOME without relying on smoke fixtures.
- Fix user-reported regressions from manual testing: folder-picker keybinding not firing, nested Rust file opens failing, second file opens not replacing the editor, and file-browser actions breaking after Markdown activation/parse timeout.
- Fix layout polish regressions: left file browser must reserve editor space after document opens, remove the permanent purple decorative circle, remove or hide visible editor-card padding, make the file browser scrollable, and add a main text-area scroller.
- Preserve Clay's authority boundaries: client owns native input/rendering/selection/scroll state; server owns workspace roots, file authority, SDUI action validation, document opens, behavior manifests, and package/runtime execution.

## Expected Outcome

- A user can set documented keybindings in `~/.config/clay/init.js`, run `cargo run`, press `Ctrl+Shift+O` on GNOME/Linux, select a folder, browse nested directories, open `.rs`, `.ts`, `.js`, and `.md` UTF-8 files, open a second file after the first, and copy selected text.
- File-browser navigation and file-open actions remain usable after Markdown/package activation diagnostics, including `clay.parse.open_activation_timeout`.
- The left file browser and editor never visually overlap; the editor text area is clipped and measured inside the remaining main region after the left pane.
- The editor no longer paints the bottom-right purple decorative circle or a visible inset card/padding border around the working text area.
- The left file browser can scroll to later entries, and the main editor shows/uses a vertical scroller for long documents.
- Focused Linux validation passes: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, focused unit/integration/doc tests, and `cargo test --all-targets`.

## Tasks

- [x] Entry gate: reproduce and lock the manual bug contract
  - Acceptance Criteria:
    - Functional: The plan's target failures are captured as deterministic repro tests or documented manual repro notes before implementation changes: GNOME/Linux `Ctrl+Shift+O` keybinding route, nested `src/*.rs` action source mismatch, file-browser action rejection after Markdown open, second-file replacement failure, editor/file-browser overlap after open, permanent purple circle, visible editor card padding, missing file-browser scroll, and missing main text scroller.
    - Performance: Repro tests must not open a real GUI, spawn shell commands from test code, or require a desktop portal; GUI-only behaviors get headless state/geometry tests plus one manual smoke checklist.
    - Code Quality: Each repro names the owning layer: keybinding route, SDUI list/action identity, server `StaticSduiState`, editor region computation, editor paint chrome, SDUI scroll state, or editor scroll chrome.
    - Security: Repros must not widen filesystem authority; they use temp workspaces or existing fixtures and avoid raw absolute-path leakage in asserted diagnostics.
  - Approach:
    - Documentation Reviewed:
      - User manual findings/logs in this conversation: `ActionSourceMismatch(SduiNodeId(5))`, `UnknownActionCommand("clay.workspace.openFile")`, and `clay.parse.open_activation_timeout` after Markdown activation.
      - `docs/wiki/modules/workspace-file-browser.md`: Clay-owned file browser, directory navigation, SDUI actions, generic open follow-ups.
      - `docs/wiki/modules/server-driven-ui.md`: `StaticSduiState` action validation and client `SduiNativeState` rendering/action routing.
      - `docs/wiki/modules/masonry-editor.md`: editor owns input/rendering/status/copy; shell owns working-area geometry.
      - `docs/wiki/modules/masonry-shell.md`: `PaneSlotLayout` and editor main-region geometry.
      - Project patterns: `planning-checklist.md`, `authority-boundaries.md`, `protocol-and-performance.md`, `package-ui-layout.md`, `behavior-manifests.md`, `configuration-system.md`, `maintenance-validation.md`, `clay-js-api-boundary.md`.
    - Options Considered:
      - Reproduce only through manual GUI testing: realistic but not CI-stable.
      - Add small headless tests per bug and keep one final manual smoke: more durable and matches existing Clay validation style.
    - Chosen Approach:
      - Add the smallest headless tests that fail for each root cause, plus a manual smoke section for GNOME portal behavior and visual polish.
    - API Notes and Examples:
      ```text
      cargo test --lib shell::file_browser --quiet
      cargo test --lib masonry_sdui --quiet
      cargo test --lib masonry_editor --quiet
      cargo test --lib server::connection --quiet
      cargo test --test manual_smoke_docs --quiet
      ```
    - Files to Create/Edit:
      - `src/server/ops/keybindings.rs`: add or adjust keybinding route test for shifted character matching.
      - `src/shell/file_browser.rs`: add nested-file SDUI list item/action-source identity test.
      - `src/server/connection.rs`: add post-Markdown-open file-browser action and second-open replacement regression tests.
      - `src/masonry_sdui.rs`: add editor-region and file-browser scrolling geometry/action tests.
      - `src/editor/surface.rs`: add editor chrome/scroller paint/state tests where practical.
      - `src/masonry_editor.rs`: add pointer/scroll routing tests where practical.
      - `docs/development/launch-and-gui-smoke.md`: add final manual GNOME workflow checklist after fixes.
    - References:
      - `src/server/sdui.rs::StaticSduiState::validate_action`
      - `src/masonry_sdui.rs::editor_region_for_document`
      - `src/shell/file_browser.rs::FileBrowserEntry::to_sdui_list_item`
      - `src/editor/surface.rs::paint_in_rect`
      - `src/masonry_editor.rs::on_text_event` and `on_pointer_event`
  - Test Cases to Write:
    - `keybinding_shifted_character_routes_client_ui_command`: `Ctrl+Shift+O` matches the configured lowercase `o` binding on Linux-style keyboard input.
    - `file_browser_nested_file_action_source_matches_list_item_id`: nested file rows have a source item ID that matches the declared list item ID while preserving root-relative path arguments.
    - `file_browser_actions_still_validate_after_markdown_open_timeout`: a Markdown open/follow-up diagnostic does not invalidate workspace browser actions.
    - `opening_second_workspace_file_replaces_editor_snapshot`: opening a second file sends/applies a new `DocumentOpened` snapshot.
    - `file_browser_left_slot_still_reserves_editor_region_after_document_open`: editor main rect remains offset after document ID changes.
  - Execution Notes:
    - Created `docs/development/manual-file-browser-workflow-bug-contract.md` as the entry-gate repro contract for the real `cargo run` + `~/.config/clay/init.js` workflow, explicitly excluding the smoke-fixture path for this manual repro.
    - Locked all reported failures with owner layers: shifted `Ctrl+Shift+O` keybinding mismatch, nested `src/main.rs` `ActionSourceMismatch`, Markdown/open-time `UnknownActionCommand` and `clay.parse.open_activation_timeout`, second-file replacement failure, file-browser/editor overlap, purple circle, visible editor card padding, missing file-browser scroll, and missing main text scrollbar.
    - Added `tests/manual_smoke_docs.rs::manual_file_browser_workflow_bug_contract_locks_reported_failures` so the documented bug contract cannot silently drop repro evidence, owner layers, hot-path constraints, or authority constraints.
    - Linked the contract from `docs/development/launch-and-gui-smoke.md` under the end-to-end file browser workflow smoke section.
    - Validation passed: `cargo test --test manual_smoke_docs manual_file_browser_workflow_bug_contract_locks_reported_failures --quiet`, `cargo fmt --check`, and `cargo test --test manual_smoke_docs --quiet` (10/10).

- [x] Review existing UI/input/SDUI primitives before fixing workflow bugs
  - Acceptance Criteria:
    - Functional: Inventory the existing primitives that should be reused: `bindKey`/behavior-manifest routing, `ClientUiCommandRoute`, `FileBrowserState`, `StaticSduiState`, `SduiNativeState`, `PaneSlotLayout`, `EditorSurface` visual scroll state, and `WorkspaceState` open/list APIs.
    - Performance: The review must preserve hot-path policy: no IPC/JS/filesystem work in paint, pointer, text, scroll, or ordinary edit handling.
    - Code Quality: The review must reject special-case Markdown/Rust branches and identify generic fixes that apply to any workspace file/package mode.
    - Security: The review must confirm fixes do not grant packages direct filesystem, clipboard, native widget, shell, network, WASM, AI, or raw `Deno.core.ops` authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md` and `docs/reference/primitives/registry.md`.
      - `docs/wiki/modules/primitive-architecture.md`.
      - `docs/wiki/modules/workspace-file-browser.md`, `server-driven-ui.md`, `masonry-shell.md`, `masonry-editor.md`, `server-ipc-skeleton.md`, `client-file-dialog.md`.
      - `.agents/skills/project-patterns/references/package-ui-layout.md` and `behavior-manifests.md`.
    - Options Considered:
      - Fix each symptom locally: likely to re-break sibling paths.
      - Fix primitives once at the shared routing/layout/state boundaries: smaller long-term diff and fewer duplicated guards.
    - Chosen Approach:
      - Document the root-cause primitive boundaries first, then implement fixes at those shared boundaries.
    - API Notes and Examples:
      ```rust
      // Shared shape: visible row ID validates the action source;
      // root-relative path remains a separate command argument.
      SduiListItem { id, action: Some(SduiActionIntent { source, arguments }) }
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/manual-file-browser-workflow-bugfix-primitive-review.md`: new primitive review page, if needed.
      - `docs/wiki/index.md`: link the review page, if created.
      - `tests/primitives_docs.rs`: deterministic coverage for the primitive review, if a new page is created.
    - References:
      - `plans/043-End-to-End-File-Browser-Folder-Navigation-Language-Open-and-Clipboard.md`
      - `docs/wiki/modules/end-to-end-file-browser-workflow-primitive-review.md`
  - Test Cases to Write:
    - `manual_file_browser_workflow_bugfix_primitive_review_records_root_causes`: docs coverage asserts the review records keybinding, SDUI action identity, SDUI state separation, layout, and scroll primitives.
  - Execution Notes:
    - Created `docs/wiki/modules/manual-file-browser-workflow-bugfix-primitive-review.md` documenting the existing primitives to reuse before bug fixes: behavior-manifest key routing, `ClientUiCommandRoute`, `FileBrowserState`/`WorkspaceState`, `StaticSduiState`, `SduiNativeState`, `PaneSlotLayout`, `EditorSurface` scroll/paint state, and open-document follow-ups.
    - Mapped each locked manual failure to a generic owner/fix boundary: shifted key matching, SDUI row/action identity, workspace-browser validation-state separation, parse timeout as diagnostic-only, second-file replacement, left-slot editor region reservation, editor chrome removal, file-browser local scrolling, and editor scrollbar chrome.
    - Rejected mode/file-type-specific implementation shapes, `StaticSduiState` validation relaxation, server-side scrolling, hidden config keys, package-owned native widgets, raw CSS/native handles/client-side JavaScript, and raw `Deno.core.ops` authority.
    - Linked the new review from `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md`.
    - Added `tests/primitives_docs.rs::manual_file_browser_workflow_bugfix_primitive_review_records_root_causes` to keep the primitive inventory, root-cause map, and hot-path/security boundaries present.
    - Validation passed: `cargo test --test primitives_docs manual_file_browser_workflow_bugfix_primitive_review_records_root_causes --quiet`, `cargo fmt --check`, and `cargo test --test primitives_docs --quiet` (97/97).

- [x] Fix `Ctrl+Shift+O` and shifted character keybinding routing on Linux/GNOME
  - Acceptance Criteria:
    - Functional: A `bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" })` route fires when the user presses `Ctrl+Shift+O` on Linux/GNOME, and still does not insert text into the editor.
    - Performance: Key matching remains an in-memory client behavior-manifest lookup; no IPC or server round trip is needed before recognizing the client UI command.
    - Code Quality: Normalize character key matching in one shared place instead of adding one-off code only for `O`; preserve exact matching for non-character keys and modifiers.
    - Security: The fix only routes the already-declared `ClientUiCommand`; it does not open dialogs without explicit user key input and does not grant filesystem authority before the server capability/root validation flow.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: `bindKey` is the documented configuration route.
      - `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`: folder picker command-ID helper and authority boundary.
      - `src/server/ops/keybindings.rs`: chord parsing lowercases single-character keys.
      - `src/client/behavior.rs`: route lookup currently compares `KeyStroke` equality.
      - `src/masonry_editor.rs`: native key events are translated to `KeyStroke` for routing.
    - Options Considered:
      - Lowercase every `Key::Character` at the native event boundary: simple, but could alter actual text insertion for shifted printable text if reused incorrectly.
      - Make behavior-manifest key matching case-insensitive only for character command lookup while preserving inserted character text separately: safer.
    - Chosen Approach:
      - Normalize character keys only for manifest route matching or create a `KeyStroke::matches_binding` helper used by `ClientBehaviorState::route_key`.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      import { clientOpenFolderDialog } from "clay:workspace";
      bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `src/client/behavior.rs`: match character bindings case-insensitively while modifiers still match exactly.
      - `src/masonry_editor.rs`: if needed, ensure client-ui outcomes from key routing are submitted to the app driver.
      - `src/server/ops/keybindings.rs`: add parser/serialization regression coverage if needed.
      - `src/server/js_runtime.rs`: keep existing configuration fixture tests passing.
    - References:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - `shifted_character_key_binding_matches_lowercase_manifest_rule`: `Ctrl+Shift+O` routes to `clay.workspace.clientOpenFolderDialog`.
    - `shifted_printable_unbound_character_still_inserts_shifted_text`: unbound shifted text remains normal editor input.
    - `configuration_binds_client_ui_file_folder_and_copy_commands`: keep existing fixture coverage green.
  - Execution Notes:
    - Added shared `key_matches_binding()` lookup in `src/client/behavior.rs`: modifiers still match exactly, non-character keys still use exact equality, and character bindings compare case-insensitively so parsed lowercase manifest chords match shifted native event text like `"O"`.
    - Kept unbound shifted printable text insertion untouched through `route_unbound_key`, so `Shift+!`/uppercase text still inserts the native event text when no Ctrl/Alt/Super modifier blocks insertion.
    - Added `shifted_character_key_binding_matches_lowercase_manifest_rule` and `shifted_printable_unbound_character_still_inserts_shifted_text` tests; existing `configuration_binds_client_ui_file_folder_and_copy_commands` still passes.
    - Updated `docs/wiki/modules/behavior-manifests.md` and `docs/wiki/flows/client-behavior-routing.md` with the shifted-character binding invariant and new tests.
    - Validation passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, targeted three tests, and `cargo test --lib --quiet` (600/600).

- [x] Fix nested file open actions and second-file replacement from the file browser
  - Acceptance Criteria:
    - Functional: Clicking nested files such as `src/main.rs`, `src/main.ts`, `src/main.js`, and `docs/foo.md` opens the selected file; opening a second file replaces the editor buffer/status with the second file's document snapshot.
    - Performance: The fix keeps one bounded SDUI action and one server file-open command per click; no directory rescan beyond existing root-relative validation.
    - Code Quality: The displayed list item ID, SDUI action source item ID, and command argument semantics are explicit: source IDs validate UI identity, arguments carry root-relative file paths.
    - Security: Root-relative path arguments remain server-validated through `WorkspaceState::open_existing_file`; mismatched source/action IDs remain rejected.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/workspace-file-browser.md`: file rows use `clay.workspace.openFile`, directory rows use `clay.workspace.openDirectory`.
      - `docs/wiki/modules/server-driven-ui.md`: `validate_action` checks list item action source matches declared row identity.
      - User log: nested `src/main.rs` row had `id: "main.rs"` but source `item_id: "src/main.rs"`, producing `ActionSourceMismatch(SduiNodeId(5))`.
    - Options Considered:
      - Change server validation to accept either display ID or relative path: broadens validation and hides malformed UI data.
      - Change file-browser row construction so `SduiListItem.id` and action source item ID are identical: smaller and preserves strict validation.
    - Chosen Approach:
      - Keep validation strict; make file-browser rows internally consistent. Use the visible row ID for source validation and keep `relativePath` only in action arguments.
    - API Notes and Examples:
      ```rust
      let item_id = self.name.clone();
      let relative_path = self.relative_path.to_string_lossy().to_string();
      // source.item_id == item.id; relativePath argument carries nested path.
      ```
    - Files to Create/Edit:
      - `src/shell/file_browser.rs`: fix `FileBrowserEntry::to_sdui_list_item` source item ID construction.
      - `src/masonry_editor.rs`: add widget-level coverage that a second `DocumentOpened` event replaces the active editor snapshot.
      - `docs/wiki/modules/workspace-file-browser.md`: document row identity vs path argument invariant.
    - References:
      - `src/server/sdui.rs::StaticSduiState::validate_action`
      - `src/server/command_execution.rs::execute_open`
      - `src/server/workspace.rs::open_existing_file`
  - Test Cases to Write:
    - `file_browser_nested_file_row_source_id_matches_declared_item_id`: source ID equals list item ID for nested files.
    - `workspace_nested_file_action_opens_file_through_workspace_api`: clicking nested `.rs` row returns `DocumentOpened`.
    - `opening_second_file_browser_file_replaces_editor_snapshot`: second open updates `EditorWidget` visible text/document ID.
  - Execution Notes:
    - Fixed `FileBrowserEntry::to_sdui_list_item` so `SduiActionSource::ListItem.item_id` uses the same row identity as `SduiListItem.id` (`self.name`) instead of the nested root-relative path. `relativePath` still carries `src/main.rs` as an action argument for server-side workspace validation.
    - Added `file_browser_nested_file_row_source_id_matches_declared_item_id` to lock row identity vs path argument semantics for nested files.
    - Renamed/enhanced the nested open command test to `workspace_nested_file_action_opens_file_through_workspace_api`, asserting the nested row source ID matches the declared item ID and that the workspace API opens `src/main.rs`.
    - Added `opening_second_file_browser_file_replaces_editor_snapshot` in `src/masonry_editor.rs` to verify sequential `DocumentOpened` events replace visible text, document ID/version, and status with the second file.
    - Updated `docs/wiki/modules/workspace-file-browser.md` to document that list item IDs validate UI row identity while `relativePath` is only a typed argument revalidated by `WorkspaceState`.
    - Validation passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, targeted task tests, `cargo test --lib shell::file_browser --quiet`, and `cargo test --lib --quiet` (602/602).

- [x] Keep Clay-owned file-browser SDUI usable across document opens, Markdown activation, and parse timeouts
  - Acceptance Criteria:
    - Functional: After opening an `.md` file and receiving a `BehaviorManifest`, `DecorationSet`, or `clay.parse.open_activation_timeout` diagnostic, clicking file-browser directories/files still validates and executes. The file browser must not be replaced by package/open-time SDUI state.
    - Performance: Open follow-ups may classify/parse asynchronously as before, but they must not trigger extra full-document IPC, workspace scans, or UI tree rebuilds on ordinary typing/paint.
    - Code Quality: Separate Clay-owned workspace chrome from package/open-time SDUI publication. Avoid Markdown-specific exceptions; the same rule applies to Rust/TypeScript/JavaScript/package opens.
    - Security: Runtime/package outputs must not gain authority to erase or replace Clay-owned workspace action validation unless they intentionally publish through the documented package UI/SDUI path with validation and permissions.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/server-ipc-skeleton.md`: all open origins run `open_document_followup_messages`.
      - `docs/wiki/modules/server-driven-ui.md`: `StaticSduiState` is the server validation state for inbound SDUI actions.
      - User log: after Markdown activation, browser clicks produce `UnknownActionCommand("clay.workspace.openFile")` even though client still paints old workspace rows.
      - `src/server/mod.rs::apply_runtime_outputs`: currently applies both behavior manifests and runtime-published SDUI to shared `StaticSduiState`.
      - `src/server/connection.rs::classify_open_document`: open-time classification calls `apply_runtime_outputs`.
    - Options Considered:
      - Re-send file-browser snapshot after every open-time package activation: works but races with package SDUI replacement and adds unnecessary snapshots.
      - Stop applying runtime-published SDUI from open-time classification/follow-ups; only behavior/parse outputs should update open-document state. Runtime SDUI publication remains for explicit config/package UI flows.
      - Split server SDUI into separate workspace-chrome and package-SDUI validation states: more complete but larger.
    - Chosen Approach:
      - Use the smallest safe split now: open-time classification applies behavior/parse/decorations but does not replace the Clay-owned file-browser `StaticSduiState`. If future package UI needs document-open panels, route through the existing package UI runtime, not the workspace browser state.
    - API Notes and Examples:
      ```rust
      // Open-time follow-up should publish behavior/decorations only.
      apply_runtime_behavior_outputs(&evaluation, metadata.document_id, behavior).await;
      // Do not replace StaticSduiState used for file-browser action validation.
      ```
    - Files to Create/Edit:
      - `src/server/mod.rs`: add helper or option to apply runtime outputs without SDUI replacement.
      - `src/server/connection.rs`: use the behavior/decor-only path from `classify_open_document`/`open_document_followup_messages`.
      - `src/server/sdui.rs`: add validation/state regression tests only if needed.
      - `docs/wiki/modules/server-ipc-skeleton.md` and `server-driven-ui.md`: document separation.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
  - Test Cases to Write:
    - `file_browser_action_survives_markdown_open_followup_diagnostic`: after opening Markdown and receiving timeout/diagnostic, a directory/file action still validates.
    - `open_time_runtime_sdui_output_does_not_replace_workspace_browser_state`: runtime output with a published tree during classification does not erase workspace action validation.
    - `markdown_open_timeout_is_status_only_not_navigation_poison`: diagnostic appears but subsequent workspace navigation works.
  - Execution Notes:
    - Added `apply_runtime_outputs_without_sdui` in `src/server/mod.rs` for open-document follow-ups. It applies behavior manifests and passes decorations through, but deliberately ignores `published_sdui_tree` so package/open-time activation cannot replace Clay-owned workspace-browser validation state.
    - Changed `classify_open_document` in `src/server/connection.rs` to use the behavior/decorations-only runtime-output path. Explicit startup/config SDUI publication still uses `apply_runtime_outputs` and can replace shared SDUI after validation.
    - Added `open_time_runtime_sdui_output_does_not_replace_workspace_browser_state` to lock that a published tree during open-time output application does not replace existing workspace SDUI actions.
    - Added `file_browser_action_survives_markdown_open_followup_diagnostic` to verify Markdown open-time follow-ups leave file-browser row actions valid.
    - Updated `docs/wiki/modules/server-ipc-skeleton.md` and `docs/wiki/modules/server-driven-ui.md` to document the split between explicit runtime SDUI publication and open-time behavior/decorations-only activation.
    - Validation passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, targeted task tests, `cargo test --lib runtime_outputs_tests --quiet`, `cargo test --lib connection::tests --quiet`, and `cargo test --lib --quiet` (604/604).

- [x] Fix editor/file-browser geometry after file opens and remove visible decorative chrome
  - Acceptance Criteria:
    - Functional: When a workspace file opens as a new document ID, the editor main region still starts after the left file-browser pane, so no file-browser text overlaps document text. The purple bottom-right circle is removed. The visible inset editor card/padding is removed or made visually invisible while preserving readable text placement.
    - Performance: Geometry remains a pure local layout calculation; paint still clips editor content to the editor rect and does not allocate or run server/runtime work.
    - Code Quality: Use one shared editor-region rule for Clay-owned left file-browser state. Do not patch individual document types or Markdown opens.
    - Security: Visual/layout changes expose no document text, raw paths, native handles, or package authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/masonry-shell.md`: working area and `PaneSlotLayout` own fixed left slot geometry.
      - `docs/wiki/modules/masonry-editor.md`: `EditorWidget::editor_main_rect` uses `editor_region_for_document`.
      - `src/masonry_sdui.rs::editor_region_for_document`: falls back to full rect when SDUI editor binding document ID differs from active document.
      - `src/server/connection.rs::file_browser_snapshot_message`: file-browser snapshots currently bind the bootstrap `DocumentState`, not newly opened workspace document IDs.
      - `src/editor/surface.rs::paint_in_rect`: paints a 24px inset panel and purple `ACCENT_COLOR` circle.
    - Options Considered:
      - Rebuild file-browser snapshots bound to each opened document: more server churn and loses current-directory state unless tracked.
      - Client-side geometry reserves the left slot whenever Clay-owned SDUI side panel exists, regardless of stale editor binding: minimal and directly fixes overlap.
      - Remove all text padding: maximizes area but can make caret/text hug edges.
      - Keep modest invisible text inset but paint background to the full editor rect: satisfies visibility without noisy chrome.
    - Chosen Approach:
      - Reserve fixed left slot for any visible Clay-owned file-browser SDUI root/panel, not only matching editor binding, while keeping safety checks for unknown package editor views. Remove the decorative purple circle and visible inner canvas/card; keep only minimal text inset if needed, painted on the same background.
    - API Notes and Examples:
      ```rust
      let editor_rect = editor_region_for_document(size, &sdui, active_document_id);
      editor.paint_in_rect(ctx, scene, editor_rect); // rect excludes left file browser.
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui.rs`: adjust `editor_region_for_document`/left-slot handling and tests.
      - `src/editor/surface.rs`: remove `Circle`/`ACCENT_COLOR` decorative paint and visible inset-card fill.
      - `src/masonry_editor.rs`: keep pointer hit testing aligned with the updated editor rect.
      - `docs/wiki/modules/masonry-shell.md` and `masonry-editor.md`: update geometry/chrome details.
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
      - `src/masonry_sdui.rs::sdui_panel_left_slot_rect`
      - `src/editor/surface.rs::TEXT_INSET`
  - Test Cases to Write:
    - `workspace_browser_reserves_left_slot_after_document_id_changes`: active document ID no longer causes full-rect fallback overlap.
    - `editor_surface_paint_has_no_decorative_accent_circle`: structural paint test or source-guard test preventing the permanent purple circle from returning.
    - `editor_surface_uses_full_rect_background_without_visible_card_inset`: verifies no 24px inset panel/card rect is painted.
    - `editor_pointer_hit_testing_uses_non_overlapping_editor_region_after_open`: clicks in left pane do not place caret under the panel.
  - Execution Notes:
    - Rewrote `editor_region_for_document` in `src/masonry_sdui.rs` to reserve the Clay-owned left file-browser slot whenever a Clay-owned SDUI panel exists (`root_id` set or an editor binding present), not only when the SDUI editor binding matches the active document. Opening a workspace file under a new document ID no longer falls back to the full rect and overlap the file browser.
    - Removed the decorative purple accent `Circle` and `ACCENT_COLOR` from `EditorSurface::paint_in_rect` in `src/editor/surface.rs`, removed the `Circle` import, and replaced the 24px inset card fill with a full-rect editor background fill while keeping the small `TEXT_INSET` text inset.
    - Renamed `unknown_editor_view_document_uses_safe_full_editor_region` to `workspace_browser_reserves_left_slot_after_document_id_changes` and asserted the new bounded region. Added `editor_pointer_hit_testing_uses_non_overlapping_editor_region_after_open` in `src/masonry_editor.rs` covering the SDUI-panel plus `DocumentOpened` flow. Added two source-guard tests in `src/editor/surface.rs` (`editor_surface_paint_has_no_decorative_accent_circle`, `editor_surface_uses_full_rect_background_without_visible_card_inset`) using compile-time `include_str!("surface.rs")` to avoid reintroducing `std::fs` into the editor hot path.
    - Updated `docs/wiki/modules/masonry-shell.md`, `docs/wiki/modules/masonry-editor.md`, and `docs/wiki/modules/server-driven-ui.md` to document panel-presence-based editor-region reservation and the removed editor chrome.
    - Validation passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (lib 607/607 plus all integration/doc suites, including `editor_performance_invariants`, `primitives_docs` 97/97, `manual_smoke_docs` 10/10).

- [x] Add scroll support for the left file browser
  - Acceptance Criteria:
    - Functional: When the file-browser list has more rows than fit vertically, mouse wheel/trackpad scroll over the left pane reveals later entries and lets users click them. Scrolled hit testing must activate the row currently under the pointer, not the pre-scroll row.
    - Performance: Scrolling is client-local state and paint math only; it does not relist directories, call the server, run JavaScript, serialize documents, or enqueue workspace actions until the user clicks a visible row.
    - Code Quality: Implement scroll offset/bounds once in `SduiNativeState` for panel/list rendering and action-region rebuild. Keep row-height math shared between paint and hit testing.
    - Security: Scrolling reveals only entries already present in the bounded server-provided snapshot; it does not bypass listing budgets or request hidden filesystem paths.
  - Approach:
    - Documentation Reviewed:
      - `src/masonry_sdui.rs`: current panel/list paint uses `cursor_y` with no scroll offset or max-scroll state.
      - `src/masonry_editor.rs::on_pointer_event`: all scroll events currently go to `EditorSurface`, regardless of pointer location.
      - `src/shell/file_browser.rs`: server listing is bounded before it reaches the client.
    - Options Considered:
      - Implement server-side pagination first: larger and not needed for already-sent entries.
      - Add client-local scroll offset to the SDUI left panel: minimal and sufficient for current bounded snapshots.
    - Chosen Approach:
      - Add vertical scroll state for the SDUI left panel. Route pointer scroll to SDUI when the pointer is inside the left panel; otherwise keep editor scrolling. Clamp scroll offset to content height minus viewport height. Apply offset consistently to paint and action regions.
    - API Notes and Examples:
      ```rust
      if self.sdui.scrolls_point(point) {
          self.sdui.scroll_vertical_pixels(delta_pixels, ctx.size())
      } else {
          self.editor.scroll_vertical_pixels(delta_pixels)
      }
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui.rs`: add scroll offset/max calculation, panel hit-test helper, scroll method, and paint/action offset logic.
      - `src/masonry_editor.rs`: route `PointerEvent::Scroll` to SDUI when pointer is over the file browser.
      - `docs/wiki/modules/workspace-file-browser.md` and `server-driven-ui.md`: document client-local file-browser scroll.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `src/server/workspace.rs::list_directory` bounded listing guarantees.
  - Test Cases to Write:
    - `file_browser_scroll_reveals_later_rows_without_relisting`: scroll offset changes visible/action rows without server calls.
    - `file_browser_scrolled_action_hits_visible_row`: after scrolling, clicking a row activates that row's `SduiActionIntent`.
    - `editor_scroll_events_still_scroll_editor_outside_file_browser`: scroll routing remains correct outside left pane.
  - Execution Notes:
    - Added `scroll_offset`, `content_height`, and `viewport_height` fields to `SduiNativeState` in `src/masonry_sdui.rs` plus `scrolls_point(size, point)`, `scroll_vertical_pixels(size, delta)`, `scroll_lines(size, lines)`, and `scroll_offset()` accessors. Scroll is clamped to `[0, (content_height - viewport_height).max(0)]` and reset to zero on `apply_snapshot`/`apply_update`.
    - `paint()` now fills the sidebar, opens a `scene.push_clip_layer` over the sidebar rect, advances `cursor_y = sidebar.y0 + panel_padding - scroll_offset`, measures `content_height = (cursor_y - sidebar.y0 + scroll_offset).max(0)` after painting rows, pops the clip layer, and clamps the offset. `rebuild_action_regions_for_test` applies the same offset so action hit regions track the painted rows.
    - `EditorWidget::on_pointer_event` in `src/masonry_editor.rs` routes `PointerEvent::Scroll` to `sdui.scrolls_point`/`scroll_lines`/`scroll_vertical_pixels` when the pointer is inside the left file-browser panel; otherwise the existing editor scroll path runs unchanged. The scroll event handler now reads `event.state.position` via `ctx.local_position`.
    - Added tests in `src/masonry_sdui.rs`: `file_browser_scroll_reveals_later_rows_without_relisting` (offset changes, content exceeds viewport, clamping), `file_browser_scrolled_action_hits_visible_row` (a pixel that showed `item-0` shows `item-2` after scrolling), and `scrolls_point_routes_scroll_to_file_browser_only_inside_left_pane` (routing predicate true only inside the sidebar). The editor-outside scroll path is unchanged and still covered by existing editor scroll tests.
    - Updated `docs/wiki/modules/workspace-file-browser.md` (client-local scroll subsection + test list + sources) and `docs/wiki/modules/server-driven-ui.md` (scroll state documentation + test list) plus the primitive review test list.
    - Validation passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (lib 610/610 plus all integration/doc suites, `primitives_docs` 97/97, `manual_smoke_docs` 10/10).

- [x] Add main text-area scroller and preserve existing editor scrolling behavior
  - Acceptance Criteria:
    - Functional: Long files can be scrolled in the main editor, and the text area paints a visible vertical scrollbar/thumb that reflects current scroll position. The scroller must stay inside the editor main region and not overlap the file browser or status bar.
    - Performance: The scroller uses existing `EditorSurface` viewport/visual scroll state and paint-time metrics; no server/IPC/JS/file IO runs during scroll or paint.
    - Code Quality: Reuse `visual_scroll_y` and `last_visual_max_scroll_y`; do not introduce a second conflicting scroll model. Keep scrollbar drawing small and deterministic.
    - Security: Scrollbar state is local UI state only and exposes no document contents beyond visible text already painted.
  - Approach:
    - Documentation Reviewed:
      - `src/editor/surface.rs`: already supports `scroll_lines`, `scroll_vertical_pixels`, `visual_scroll_y`, and `last_visual_max_scroll_y`.
      - `src/editor/layout.rs`: computes layout metrics and max scroll.
      - `src/masonry_editor.rs`: pointer scroll routes to `EditorSurface`.
    - Options Considered:
      - Add a full interactive scrollbar widget with drag/page controls now: more code and not required to restore core workflow.
      - Paint a slim native scrollbar indicator driven by existing scroll state and keep wheel/trackpad scrolling: smallest useful scroller.
    - Chosen Approach:
      - Add a visible vertical scrollbar indicator to `EditorSurface::paint_in_rect` using existing max/current scroll state. If time allows within the same task, add simple thumb-drag support; otherwise document drag as deferred and keep wheel/trackpad support.
    - API Notes and Examples:
      ```rust
      if self.last_visual_max_scroll_y > 0.0 {
          self.paint_vertical_scrollbar(scene, rect, self.visual_scroll_y, self.last_visual_max_scroll_y);
      }
      ```
    - Files to Create/Edit:
      - `src/editor/surface.rs`: paint scrollbar indicator, expose test-only scroll metric if needed.
      - `src/masonry_editor.rs`: route/clip scroll interactions inside updated editor region; optionally add scrollbar drag state if implemented.
      - `docs/wiki/modules/masonry-editor.md`: document editor scroller behavior.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `src/editor/viewport.rs` existing scroll tests.
  - Test Cases to Write:
    - `editor_scrollbar_thumb_reflects_visual_scroll_position`: thumb moves as `visual_scroll_y` changes.
    - `editor_scrollbar_hidden_when_content_fits`: no scrollbar for short content.
    - `editor_scrollbar_stays_inside_main_editor_region_with_left_browser`: scrollbar x-range is inside editor rect, not file-browser rect.
  - Execution Notes:
    - Added a slim vertical scrollbar indicator to `EditorSurface::paint_in_rect` in `src/editor/surface.rs`, painted inside the existing clip layer so it stays within the editor rect. Reuses the existing `visual_scroll_y`/`last_visual_max_scroll_y` state; no second scroll model.
    - Added `pub(crate) fn scrollbar_thumb_rect(&self, rect: Rect) -> Option<Rect>` computing the thumb deterministically (returns `None` when `last_visual_max_scroll_y <= 0`, thumb height proportional to `available_height/(available_height+max_scroll)` with a `SCROLLBAR_MIN_THUMB` floor, position clamped to the track inside the editor rect). `paint_vertical_scrollbar` shares the helper, painting a faint track plus the thumb in `SCROLLBAR_COLOR`.
    - Added constants `SCROLLBAR_COLOR`, `SCROLLBAR_TRACK_COLOR`, `SCROLLBAR_WIDTH`, `SCROLLBAR_MARGIN`, `SCROLLBAR_MIN_THUMB`. The scrollbar sits at `rect.x1 - SCROLLBAR_MARGIN`, so it never crosses into the file browser or past the editor right edge.
    - Added tests in `src/editor/surface.rs`: `editor_scrollbar_thumb_reflects_visual_scroll_position` (thumb moves down as `visual_scroll_y` grows, pins to bottom at max scroll), `editor_scrollbar_hidden_when_content_fits` (`None` when `max_scroll == 0`), and `editor_scrollbar_stays_inside_main_editor_region_with_left_browser` (thumb rect fully inside editor `[240,900]x[0,600]`).
    - Updated `docs/wiki/modules/masonry-editor.md` (scroller invariant + test list) and the primitive review test list. Thumb-drag interaction is documented as deferred per the chosen approach (wheel/trackpad scroll restored first).
    - Validation passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (lib 613/613 plus all integration/doc suites).

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Existing documented config route remains enough for this workflow: `bindKey("Ctrl+Shift+O", clientOpenFolderDialog())`, `Ctrl+B` browser toggle, and native `Ctrl+C` copy. If a default product binding is added, it is documented explicitly.
    - Performance: Configuration evaluation remains startup/open-time work only; no configuration JavaScript runs during typing, paint, pointer, or scroll.
    - Code Quality: No hidden JSON/TOML/ad hoc keys are added for scrollbars, padding, folder picker, file-browser size, or diagnostics. Any configurable behavior is a Clay JS API.
    - Security: Configuration does not implicitly grant broad workspace, filesystem, clipboard read, shell, network, WASM, AI, native widget, or raw-op authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/keybindings/bind-key.md`
      - `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`
      - `docs/reference/clay-js-api/editor/client-copy-selection.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
    - Options Considered:
      - Add new config keys for file-browser scroll/padding: unnecessary and against project patterns.
      - Verify/fix existing `bindKey` route only: sufficient for reported bugs.
    - Chosen Approach:
      - Treat this as configuration API verification unless implementation chooses to add default keybindings. Update docs/tests only if behavior changes.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      import { clientOpenFolderDialog } from "clay:workspace";
      bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: update only if shifted-character behavior/defaults are clarified.
      - `docs/reference/clay-js-api/configuration.md`: update only if defaults/docs change.
      - `tests/clay_js_api_inventory.rs`: adjust if docs assertions need new wording.
    - References:
      - `tests/fixtures/configuration/file-browser-workflow/init.js`
      - `src/server/js_runtime.rs::file_browser_workflow_config_fixture_loads_packages_and_bindings`
  - Test Cases to Write:
    - `configuration_shifted_folder_binding_routes_on_linux_key_event`: config route works for `Ctrl+Shift+O`.
    - Existing Clay JS API inventory/doc registry tests remain green.
  - Execution Notes:
    - Treated as configuration API verification per the chosen approach: no new config keys, no defaults added, no hidden JSON/TOML/ad hoc keys for scrollbars, padding, folder picker, file-browser size, or diagnostics. The existing documented route (`bindKey("Ctrl+Shift+O", clientOpenFolderDialog())`, `Ctrl+B` toggle, native `Ctrl+C` copy) remains the workflow configuration surface.
    - Added `configuration_shifted_folder_binding_routes_on_linux_key_event` to `src/client/behavior.rs`, locking the configuration contract that a lowercase manifest `Ctrl+Shift+O` chord routes a Linux/GNOME uppercase-`O` key event to `clay.workspace.clientOpenFolderDialog` (ClientUiCommand). Confirmed the config-published manifest + behavior route chain end-to-end alongside `configuration_binds_client_ui_file_folder_and_copy_commands` and `file_browser_workflow_config_fixture_loads_packages_and_bindings`.
    - Documented the shifted character case-insensitive matching behavior in `docs/reference/clay-js-api/keybindings/bind-key.md` (manifest stores lowercase `o`, client matches modifiers exactly but character keys case-insensitively, unbound shifted printable still inserts shifted text), and added the new test to `docs/wiki/flows/client-behavior-routing.md` and `docs/wiki/modules/behavior-manifests.md`.
    - Validation passed: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (lib 614/614), `clay_js_api_inventory` 54/54, `clay_js_doc_registry` 29/29, `clay_js_facade_layout` 4/4, `primitives_docs` 97/97, `manual_smoke_docs` 10/10.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: No raw Rust/public protocol surface is exposed for these bugfixes unless a real public programmatic capability is added. Existing APIs (`clientOpenFolderDialog`, `clientCopySelection`, `serverOpenDirectory`, `bindKey`) remain documented and registry-covered.
    - Performance: API verification adds no runtime work.
    - Code Quality: New Rust helpers should be private or `pub(crate)` unless they intentionally back a documented Clay JS API. Existing public Rust items touched by the plan are either already covered or explicitly allowlisted as internal infrastructure.
    - Security: No public API exposes raw clipboard writes/reads, raw file paths, raw portal handles, native widget handles, or arbitrary SDUI validation bypasses.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `docs/index.md`
    - Options Considered:
      - Add new APIs for scrollbars/layout polish: not needed; behavior is internal UI implementation.
      - Verify existing APIs and visibility mapping: correct for bugfix plan.
    - Chosen Approach:
      - Run API inventory/registry/visibility tests and update docs only if the implementation changes public API behavior.
    - API Notes and Examples:
      ```text
      cargo test --test clay_js_api_inventory --quiet
      cargo test --test clay_js_doc_registry --quiet
      cargo test --test rust_visibility_api_mapping --quiet
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: only if public API docs need clarification.
      - `docs/reference/clay-js-api/api-inventory.toml`: only if a public API changes.
      - `docs/generated/clay-js-api-registry.json`: regenerate only if inventory/docs change.
      - `tests/rust_visibility_api_mapping.rs`: update only for legitimate internal public Rust visibility.
    - References:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API task requirement.
  - Test Cases to Write:
    - Existing `clay_js_api_inventory`, `clay_js_doc_registry`, `clay_js_facade_layout`, and `rust_visibility_api_mapping` suites pass.
  - Execution Notes:
    - Verification task per chosen approach: the bugfix tasks added no new public programmatic Clay JS surface. All new Rust helpers are private or `pub(crate)`: `key_matches_binding` (private module fn, `src/client/behavior.rs`); `scrolls_point`/`scroll_vertical_pixels`/`scroll_lines`/`scroll_offset` (`pub(crate)` on `SduiNativeState`, `src/masonry_sdui.rs`); `scrollbar_thumb_rect` (`pub(crate)`, `src/editor/surface.rs`); `paint_vertical_scrollbar` (private fn); `apply_runtime_outputs_without_sdui` (`pub(crate)`, `src/server/mod.rs`).
    - The one `pub` method touched (`EditorWidget::copy_selection_to_system_clipboard` in `src/masonry_editor.rs`) is a client Masonry widget method, outside the `rust_visibility_api_mapping` server scan; the public Clay JS surface is the documented `clay.editor.clientCopySelection` command id, already registry-covered from Plan 043.
    - Existing public APIs remain documented and registry-covered: `clay.workspace.clientOpenFolderDialog`, `clay.editor.clientCopySelection`, `clay.commands.serverOpenDirectory`, and `clay.keybindings.bindKey`. No raw clipboard read/write, raw file path, raw portal handle, native widget handle, or SDUI validation bypass was exposed.
    - Validation passed: `cargo fmt --check`, `cargo test --all-targets` (all 29 binaries green, lib 614/614), `clay_js_api_inventory` 54/54, `clay_js_doc_registry` 29/29, `clay_js_facade_layout` 4/4, `rust_visibility_api_mapping` 11/11. No doc/registry regeneration needed (no public API behavior changed).

- [x] Update end-to-end manual smoke docs and fixtures for real `cargo run` workflow
  - Acceptance Criteria:
    - Functional: Documentation describes the actual supported manual path: set `~/.config/clay/init.js`, run `cargo run`, press `Ctrl+Shift+O` on GNOME/Linux, select a folder, navigate nested folders, open Rust/TypeScript/JavaScript/Markdown files, open a second file, scroll file browser/editor, and copy text.
    - Performance: Manual smoke docs call out that scroll, paint, selection, and ordinary typing stay client-local.
    - Code Quality: Docs distinguish product `cargo run` workflow from smoke fixture workflow and keep test fixture docs in sync.
    - Security: Docs restate that selected-folder grants are server-validated, file opens stay root-relative/selected-file validated, and clipboard is copy-selection write-only.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`
      - `tests/fixtures/configuration/file-browser-workflow/init.js`
      - `docs/reference/clay-js-api/configuration.md`
    - Options Considered:
      - Keep docs fixture-only: misses the user's actual workflow.
      - Add explicit `cargo run` manual smoke section: documents the real path and catches future regressions.
    - Chosen Approach:
      - Update manual smoke docs and coverage assertions to include both the fixture command and the product `cargo run` + config path.
    - API Notes and Examples:
      ```bash
      cargo run
      # with ~/.config/clay/init.js binding Ctrl+Shift+O to clientOpenFolderDialog()
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`: update end-to-end workflow section.
      - `tests/manual_smoke_docs.rs`: assert docs include the real `cargo run` path and reported bug regressions.
      - `tests/fixtures/configuration/file-browser-workflow/init.js`: update only if config fixture changes.
    - References:
      - User manual findings from this conversation.
  - Test Cases to Write:
    - `end_to_end_file_browser_workflow_smoke_covers_cargo_run_config_path`: docs mention `cargo run`, `~/.config/clay/init.js`, `Ctrl+Shift+O`, nested `.rs`, second file open, file-browser scroll, editor scroller, and copy.
  - Execution Notes:
    - Added a `#### Product \`cargo run\` configuration path` subsection to `docs/development/launch-and-gui-smoke.md` documenting the real end-user workflow (`~/.config/clay/init.js` + bare `cargo run`) as the regression-checked product path on Linux/GNOME, distinct from the checked-in smoke fixture command. Includes the init.js shape, the shifted-character `Ctrl+Shift+O` routing note, and a regression checklist: folder picker, server-validated root addition, nested `.rs`/`.ts`/`.js`/`.md` opens, second-file buffer replacement, file browser surviving Markdown activation and `clay.parse.open_activation_timeout`, file-browser scroll, editor scrollbar thumb, and copy-selection.
    - Documents that typing/paint/layout/pointer/scroll stay client-local and restates the security/authority contract (selected-folder grants server-validated, file opens root-relative/selected-file validated, clipboard copy write-only).
    - Added `end_to_end_file_browser_workflow_smoke_covers_cargo_run_config_path` to `tests/manual_smoke_docs.rs` asserting the docs cover the cargo run config path markers: product subsection heading, `cargo run`, `~/.config/clay/init.js`, `Ctrl+Shift+O`, nested `src/main.rs`, second-file replacement, file-browser scroll, editor scrollbar, copy, client-local hot-path note, and security phrases.
    - No fixture changes needed (`tests/fixtures/configuration/file-browser-workflow/init.js` already mirrors the product init.js shape); the existing `end_to_end_file_browser_workflow_smoke_has_runnable_fixture_contract` test still covers the fixture path.
    - Validation passed: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (no failures; `manual_smoke_docs` 11/11, `primitives_docs` 97/97).

- [x] Run focused and full verification
  - Acceptance Criteria:
    - Functional: All focused workflow tests and full Linux test gates pass.
    - Performance: Existing editor/performance invariant tests remain green; no new full-document IPC or hot-path server/JS work is introduced.
    - Code Quality: `cargo fmt --check`, `cargo check --all-targets`, and `cargo clippy --all-targets -- -D warnings` pass on Linux.
    - Security: Protocol/API/docs/security coverage tests pass, including no raw API/authority drift.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
    - Options Considered:
      - Run only changed modules: faster but misses cross-layer GUI/protocol regressions.
      - Run focused tests during tasks and full gate at end: required for confidence.
    - Chosen Approach:
      - Run focused tests per task, then full gate before marking the plan complete.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      ```
    - Files to Create/Edit:
      - `plans/044-Manual-File-Browser-Workflow-Bugfixes-and-Scrolling-Polish.md`: mark tasks complete and record final validation.
    - References:
      - Linux is the blocking validation host per project instructions.
  - Test Cases to Write:
    - No new tests unless final verification reveals an untested gap; record final command results in the plan.
  - Execution Notes:
    - Full Linux gate passed: `cargo fmt --check` (exit 0), `cargo check --all-targets` (exit 0), `cargo clippy --all-targets -- -D warnings` (exit 0), `cargo test --all-targets` (exit 0, all 29 binaries green).
    - Lib suite: 614/614 passed (was 598 at Plan 043 baseline; +16 from Plan 044 repro/regression tests across shifted keybinding, nested file action source, second-file replacement, file-browser SDUI survival, editor geometry, file-browser scroll, and editor scrollbar).
    - Focused Plan 044 workflow suites: `client::behavior::` 22/22 (shifted folder routing, unbound shifted text insertion, config contract), `shell::file_browser::` 8/8 (nested file row/action source identity, directory navigation), `masonry_sdui::` 36/36 (left-slot geometry, file-browser scroll reveals rows, scrolled action hit testing, scroll-routing boundary), editor scrollbar tests 3/3 (thumb position, hide-on-fit, editor-region containment), `file_browser_action_survives_markdown_open_followup_diagnostic` 1/1, `open_time_runtime_sdui_output_does_not_replace_workspace_browser_state` 1/1.
    - Protocol/API/docs/security coverage: `clay_js_api_inventory` 54/54, `clay_js_doc_registry` 29/29, `clay_js_facade_layout` 4/4, `rust_visibility_api_mapping` 11/11, `primitives_docs` 97/97, `manual_smoke_docs` 11/11 (including `end_to_end_file_browser_workflow_smoke_covers_cargo_run_config_path`).
    - Performance: editor/performance invariant tests remain green; Plan 044 added only client-local paint/action/scroll math and deterministic scrollbar geometry — no new full-document IPC or hot-path server/JS work.
    - No pre-existing blockers remained: the Plan 043 cargo fmt, GitCommandResult/GitCachedStatus visibility allowlist, and syntax_grammar test resolutions carried forward cleanly.

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
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/modules/workspace-file-browser.md`: Nested file action IDs, post-open robustness, file-browser scroll.
      - `docs/wiki/modules/server-driven-ui.md`: SDUI validation/state separation and scroll/action-region behavior.
      - `docs/wiki/modules/masonry-editor.md`: editor scroller, chrome removal, copy unchanged.
      - `docs/wiki/modules/masonry-shell.md`: fixed left slot/editor region non-overlap after document opens.
      - `docs/wiki/modules/server-ipc-skeleton.md`: open follow-up no longer replaces workspace browser validation state, if changed.
      - `docs/wiki/modules/client-file-dialog.md`: GNOME/Linux folder keybinding note, if docs change.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
    - Add deterministic docs/wiki coverage tests where new primitive-review or required wiki content is introduced.
  - Execution Notes:
    - Verified the code wiki after all Plan 044 implementation tasks. Wiki updates landed incrementally per task (3-8); this task is the final review/consolidation pass confirming every touched implementation area is documented and linked.
    - `docs/wiki/index.md`: links all updated module pages plus the Manual File Browser Workflow Bugfix Primitive Review (line 45).
    - `docs/wiki/modules/workspace-file-browser.md`: documents client-local file-browser scroll (`scroll_offset`, `scrolls_point`, `scroll_vertical_pixels`/`scroll_lines` clamping, `push_clip_layer`, no relisting/server/JS), row/action source identity for nested files, and the source/test list.
    - `docs/wiki/modules/server-driven-ui.md`: documents open-time `apply_runtime_outputs_without_sdui` state separation (package activation cannot erase Clay-owned workspace/file-browser `StaticSduiState`), client-local scroll, and the new scroll/test references.
    - `docs/wiki/modules/masonry-editor.md`: documents the slim vertical scrollbar indicator (`scrollbar_thumb_rect`, hidden-when-fits, no second scroll model), full-rect background with no decorative accent circle/inset card, left-slot reservation invariant, and the four new source-guard/scrollbar test references.
    - `docs/wiki/modules/masonry-shell.md`: documents `editor_region_for_document` reserving the Clay-owned left file-browser slot by SDUI panel presence (not editor-binding match) so opening a workspace file under a new document ID cannot overlap the browser.
    - `docs/wiki/modules/server-ipc-skeleton.md`: documents all three document-open origins running generic `open_document_followup_messages` with `apply_runtime_outputs_without_sdui`, so Clay-owned action validation survives package activation and parse diagnostics.
    - `docs/wiki/modules/behavior-manifests.md` and `docs/wiki/flows/client-behavior-routing.md`: document case-insensitive character-key matching for shifted chords (`Ctrl+Shift+O` lowercase manifest chord routes uppercase `O` native events) and the new `configuration_shifted_folder_binding_routes_on_linux_key_event` test.
    - `docs/wiki/modules/manual-file-browser-workflow-bugfix-primitive-review.md`: test list now includes all 13 implemented repro/regression test names.
    - `docs/wiki/modules/client-file-dialog.md`: no change required \u2014 the shifted-binding fix is a behavior/keybindings-layer concern (documented in behavior-manifests/client-behavior-routing/bind-key), not the folder-picker backend; the dialog backend did not change in Plan 044.
    - Validation passed: `primitives_docs` 97/97 (including `manual_file_browser_workflow_bugfix_primitive_review_records_root_causes`), `manual_smoke_docs` 11/11.

## Compromises Made

- Scrollbar is an indicator only (wheel/trackpad scroll restored); interactive thumb-drag, page-up/down, and click-to-jump are deferred until a real user asks for drag scrolling. The deterministic `scrollbar_thumb_rect` helper makes adding drag low-cost later.
- File-browser scroll is a single client-local vertical offset over the already-listed bounded snapshot; it does not extend listing budgets or load more rows on demand. Deep directories still rely on the bounded `WorkspaceState::list_directory` budgets; scroll reveals only what the snapshot already contains.
- Shifted-character matching is case-insensitive for character keys only, with exact modifier-set comparison. It deliberately does not normalize non-character keys or locale-specific shifted symbols beyond what the native event reports; unbound shifted printable input inserts the exact event text.
- Open-time runtime outputs apply behavior/decorations only (`apply_runtime_outputs_without_sdui`) and never replace `StaticSduiState` during classification. Explicit config/runtime SDUI publication remains the sole path that swaps shared validation state; packages that need a live panel must publish through the explicit `clay.sdui.publishTree` path.
- Decorative chrome removal (purple accent circle + inset card) is unconditional and not configurable; visual theming of the editor background/scrollbar is not exposed as a Clay JS API.

## Further Actions

- Manual GNOME/Linux GUI smoke before release: run the documented product `cargo run` + `~/.config/clay/init.js` path through the full regression checklist (folder picker, nested `.rs`/`.ts`/`.js`/`.md` opens, second-file replacement, file browser surviving Markdown activation/timeout, file-browser scroll, editor scrollbar, copy). Headless tests lock the contracts; a final human visual pass is the release gate.
- Interactive scrollbar drag support (low priority): add pointer-down/pointer-drag state on the `scrollbar_thumb_rect` region mapping pointer Y to `visual_scroll_y`, reusing the existing deterministic thumb geometry. Add only if users request drag scrolling.
- On-demand deeper listing for very large directories (low priority): extend file-browser scroll to trigger a bounded `clay.workspace.serverListDirectory` continuation when scrolled near the budget boundary, keeping the client-local-scroll invariant for already-listed rows. Requires careful interaction with `WorkspaceState::list_directory` budgets and ignore rules.
- Cross-platform shifted-key event verification: the case-insensitive matching is verified headlessly on Linux/GNOME semantics; confirm macOS `Cmd+Shift+O` and Windows `Ctrl+Shift+O` native events route identically when those platforms are exercised manually.
- Consider surfacing a Clay JS API for editor/file-browser scroll position or theming only if a concrete package need emerges; current scroll/chrome behavior is deliberately compiled-internal, not configurable.
