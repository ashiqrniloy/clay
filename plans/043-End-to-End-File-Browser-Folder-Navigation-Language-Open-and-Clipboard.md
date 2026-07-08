# End-to-End File Browser, Folder Navigation, Language Open, and Clipboard Workflow

## Objectives

- Make the product workflow complete: open Clay, see a file browser, choose a system folder, navigate folders/files, open Rust/TypeScript/JavaScript files, and copy selected snippets to the OS clipboard.
- Reuse existing Clay primitives first: server-owned workspace roots/listing/open authority, SDUI file-browser composition, command execution, client UI commands, language packages, and editor selection state.
- Add only generic missing capabilities: selected-folder grants, directory navigation state, workspace-open follow-up activation, and clipboard copy.
- Keep typing, paint, layout, pointer, scroll, and text-event hot paths free of filesystem scans, IPC waits, JavaScript, shell execution, and full-document serialization.
- Update docs, Clay JS APIs, generated registry coverage, manual smoke docs, and code wiki so the workflow is discoverable and testable.

## Expected Outcome

- `cargo run` opens the Clay GUI with a visible Clay-owned Workspace file browser for the current/default workspace root.
- A user can invoke a documented client folder picker command, choose a system folder, and see that folder become the active workspace root in the file browser.
- The file browser supports real directory navigation using bounded server listings, including parent/root navigation and file activation without treating directories as files.
- Opening `.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, or `.cjs` files from the browser displays file contents and reuses the generic language package/open-time activation path for behavior/decorations when the packages are loaded; missing packages still fall back to `core.code`/`core.text` editability.
- Drag/keyboard selection plus `Ctrl+C` on Linux/Windows or `Cmd+C` on macOS copies the selected editor text to the OS clipboard without mutating the document.
- Public/bindable command IDs and configuration docs exist for folder picking and copy where needed, with generated registry tests and manual smoke instructions.

## Tasks

- [x] Entry gate: verify current baseline and lock the workflow contract
  - Acceptance Criteria:
    - Functional: Current behavior is rechecked against the six-step workflow and the plan records exact starting gaps: folder picker, directory navigation, workspace-open follow-ups, and clipboard copy.
    - Performance: Baseline tests confirm existing edit and workspace behavior before changes; no new work is started from a failing or unknown baseline.
    - Code Quality: Conflicting docs are identified before implementation, especially the older Markdown/editor-only smoke wording versus this workflow's app-level Workspace browser requirement.
    - Security: Baseline authority boundaries are restated: server owns workspace roots/listings/file opens; client owns native UI prompts and selection/clipboard state.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/workspace-file-browser.md`: Existing file-browser composition and workspace command paths.
      - `docs/wiki/modules/server-file-workspace.md`: Workspace roots, selected-file grants, bounded listing, diagnostics, and lock-release IO.
      - `docs/wiki/modules/client-file-dialog.md`: Existing client UI command and selected-file authority pattern.
      - `docs/wiki/modules/masonry-editor.md`: Editor widget status/input responsibilities.
      - `docs/wiki/modules/first-party-language-packages.md`: Rust/TypeScript/JavaScript package behavior.
      - `docs/development/launch-and-gui-smoke.md`: Manual smoke contract that must be updated.
      - `.agents/skills/project-patterns/references/planning-checklist.md`, `authority-boundaries.md`, `protocol-and-performance.md`, `maintenance-validation.md`.
    - Options Considered:
      - Start implementation immediately: faster, but risks preserving stale docs/tests that still expect no app-level file browser.
      - Reconfirm baseline first: small cost, avoids wrong fixes and documents the workflow contract.
    - Chosen Approach:
      - Run focused baseline checks and record the contract in this plan before implementation tasks.
    - API Notes and Examples:
      ```text
      cargo check --all-targets
      cargo test --lib file_browser --quiet
      cargo test --lib workspace::tests --quiet
      cargo test --lib file_dialog --quiet
      cargo test --test manual_smoke_docs --quiet
      ```
    - Files to Create/Edit:
      - `plans/043-End-to-End-File-Browser-Folder-Navigation-Language-Open-and-Clipboard.md`: Record gate evidence during execution.
    - References:
      - `roadmap.md` Phase 18.12 and Phase 18.14 sections.
      - `decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md`.
      - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.
      - `decision-logs/2026-06-29-2006-package-provided-grammar-and-capability-phases.md`.
  - Test Cases to Write:
    - Gate check: focused baseline commands above pass or failures are recorded before code changes.
  - Execution Notes (2026-07-07):
    - Baseline commands passed on Linux:
      - `cargo check --all-targets`
      - `cargo test --lib file_browser --quiet` (6 passed)
      - `cargo test --lib workspace::tests --quiet` (52 passed)
      - `cargo test --lib file_dialog --quiet` (5 passed)
      - `cargo test --test manual_smoke_docs --quiet` (8 passed)
    - Workflow contract locked:
      - Step 1 app launch: baseline supported by command-first GUI/bootstrap smoke docs and passing checks.
      - Step 2 visible file browser: implemented when server sends Workspace/file-browser SDUI from a workspace root, but current launch smoke docs still say bare `cargo run` should be editor-only/no legacy Workspace sidebar.
      - Step 3 select system folder: missing; current native dialog is `clientOpenFileDialog`, Windows-only Markdown-file picker, and non-Windows returns `Unsupported`.
      - Step 4 navigate folders/files: partial; `FileBrowserEntry::to_sdui_list_item` assigns `clay.workspace.openFile` to directories and files, so directory clicks try to open directories instead of navigating.
      - Step 5 see Rust/TypeScript/JavaScript contents: mostly present for UTF-8 files and packages, but workspace/file-browser opens currently map only to `DocumentOpened`; selected-file opens alone run `selected_file_open_followup_messages` for classification/parse/decor follow-ups.
      - Step 6 copy snippets: missing; grep shows no clipboard/copy command path, while editor selection state exists.
    - Conflicting docs/tests identified before implementation:
      - `docs/development/launch-and-gui-smoke.md` lines around editor-only smoke say bare `cargo run` must not show the legacy `Workspace` side panel.
      - `tests/manual_smoke_docs.rs` asserts the same editor-only/no-left-Workspace wording.
      - Phase 19 docs still mark non-Windows native dialogs, directory opens, and workspace expansion to a selected file's parent directory out of scope.
    - Authority boundaries restated:
      - Server owns workspace roots, canonical path validation, bounded listings, file opens, document metadata/text, behavior manifests, and package/runtime execution.
      - Client owns native UI prompts, rendering/input, selection state, and future OS clipboard writes.
      - Selected paths remain untrusted until server capability/authorization checks pass; packages must not gain raw filesystem, shell, client-dialog, or clipboard authority.

- [x] Review existing primitives and record generic gaps before implementation
  - Acceptance Criteria:
    - Functional: Primitive review inventories what already exists for workspace roots, selected-file grants, client UI command routing, bounded listing, SDUI file-browser composition, command execution, language package activation, editor selection, and clipboard-adjacent native client state.
    - Performance: Review classifies folder picking, directory listing, file opening, language activation, parse/decor publication, and clipboard copy as explicit user/background work, never ordinary typing/paint/layout work.
    - Code Quality: New Rust changes are limited to generic reusable primitives; no file-browser-specific widget, language-specific open branch, or package-specific client behavior is planned.
    - Security: Review rejects client-side workspace scans, package-added roots/markers/ignore rules, raw path opens, shelling out for dialogs, server/package clipboard writes, and arbitrary filesystem/workspace expansion.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`.
      - `docs/wiki/modules/primitive-architecture.md`.
      - `docs/wiki/modules/phase18.12-workspace-discovery-primitive-review.md`.
      - `docs/wiki/modules/phase18.14-language-package-expansion-primitive-review.md`.
      - `.agents/skills/create-plan/references/clay.md` primitive-first and Clay JS API tasks.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`, `language-capability-sequencing.md`, `package-ui-layout.md`, `behavior-manifests.md`.
    - Options Considered:
      - Treat the workflow as app-specific glue only: shortest docs, but hides reusable selected-folder and clipboard boundaries.
      - Add a small primitive review page: matches Clay plan rules and keeps future agents from reinventing the same boundaries.
    - Chosen Approach:
      - Create a focused primitive-review wiki page for the end-to-end workflow, then add deterministic docs coverage.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/end-to-end-file-browser-workflow-primitive-review.md
      tests/primitives_docs.rs
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/end-to-end-file-browser-workflow-primitive-review.md`: New primitive inventory/gap review.
      - `docs/wiki/index.md`: Link the review page.
      - `docs/wiki/modules/primitive-architecture.md`: Add a short workflow primitive entry.
      - `tests/primitives_docs.rs`: Add coverage for required review content and links.
    - References:
      - `.agents/skills/create-plan/references/clay.md`.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`.
  - Test Cases to Write:
    - Primitive docs coverage: fails if the new review page, index link, primitive-architecture link, hot-path split, rejected shapes, or authority text is missing.
  - Execution Notes (2026-07-07):
    - Created `docs/wiki/modules/end-to-end-file-browser-workflow-primitive-review.md` with existing primitive inventory for workspace roots/listing, SDUI file-browser composition, command execution, selected-file client UI routing, language activation/packages, and editor selection.
    - Recorded generic workflow gaps only: selected-folder client UI grant, file-browser directory navigation, generic open-document follow-ups for workspace/file-browser opens, and client copy-selection clipboard write.
    - Classified folder picking, selected-folder grants, directory navigation/listing, file opening, language activation, and clipboard copy as explicit user/background work; ordinary typing/paint/layout stays free of filesystem scans, native dialogs, IPC waits, JavaScript, full-document serialization, shell/network/AI, and clipboard work.
    - Rejected non-generic shapes: file-browser/folder-picker widgets, client-side workspace scans, package roots/markers/ignore/listing scopes, raw client path opens, shell-backed dialogs, language-specific Rust open branches, server/package clipboard authority, and speculative paste/cut support.
    - Linked the review from `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md`.
    - Added deterministic docs coverage in `tests/primitives_docs.rs`.
    - Validation passed: `cargo test --test primitives_docs end_to_end_file_browser_workflow_primitive_review_records_inventory_and_gaps --quiet`; `cargo test --test primitives_docs --quiet` (96 passed).
    - Note: `cargo fmt --check` was probed and failed on pre-existing formatting diffs in unrelated files; this docs/test task did not run `cargo fmt` to avoid mutating unrelated code.

- [x] Add native folder picker and selected-folder workspace grant flow
  - Acceptance Criteria:
    - Functional: A documented client UI command opens a system folder picker on Linux and Windows where supported; selecting a directory adds/deduplicates it as a server workspace root and sends a refreshed file-browser SDUI snapshot for that root.
    - Performance: The modal picker and workspace-root canonicalization run only after explicit user command; no startup, typing, paint, layout, scroll, pointer, or text-event path waits on native UI, DBus, COM, or filesystem listing.
    - Code Quality: Reuse the existing client UI command route and selected-path capability pattern; avoid shelling out to `zenity`, `kdialog`, `xdg-open`, or ad hoc scripts.
    - Security: The selected directory is not trusted until the server consumes a single-use capability, canonicalizes it, verifies it is a directory, enforces root limits, and records only that explicit root; packages/server JS cannot trigger native folder UI or write arbitrary client-selected paths.
  - Approach:
    - Documentation Reviewed:
      - `src/client/file_dialog.rs`: Existing Windows COM `IFileOpenDialog` backend and non-Windows unsupported path.
      - `docs/reference/clay-js-api/documents/client-open-file-dialog.md`: Existing bindable client UI command pattern.
      - `docs/wiki/modules/client-file-dialog.md`: Client-native dialog authority boundary and tests.
      - Local `zbus 5.15.0` docs from Cargo registry README: `Connection::session().await?`, `#[proxy]`, and `default-features = false, features = ["tokio"]` usage.
      - Local `windows 0.62.2` crate source and existing COM usage in `src/client/file_dialog.rs`.
      - `docs/wiki/modules/server-file-workspace.md`: `WorkspaceState::add_explicit_user_grant` and `OpenSelectedFile` capability limitation.
    - Options Considered:
      - Add `rfd`: simplest API, but adds a GUI/dialog dependency without needing one for Windows and Linux can use the desktop portal directly.
      - Shell out to `zenity`/`kdialog`: short, but brittle, unsandboxed, and violates the no-shell boundary.
      - Extend current Windows COM backend and add Linux `xdg-desktop-portal` via `zbus`: more code, but native/user-mediated, no shell, and uses an already-resolved Rust crate family.
    - Chosen Approach:
      - Add `clientOpenFolderDialog` as a bindable client UI command. On Linux, call `org.freedesktop.portal.FileChooser.OpenFile` with `directory=true` over the session bus and parse returned `file://` URI(s). On Windows, reuse `IFileOpenDialog` with `FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR`. Keep unsupported/cancelled/failed statuses typed and sanitized.
    - API Notes and Examples:
      ```rust
      // zbus docs pattern
      let connection = zbus::Connection::session().await?;
      // Use a FileChooser proxy to call OpenFile(parent_window, title, options).
      ```
      ```rust
      // Windows COM shape extends existing file dialog code.
      // SAFETY comments stay required around COM calls.
      dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)?;
      ```
      ```ts
      import { clientOpenFolderDialog } from "clay:workspace";
      import { bindKey } from "clay:keybindings";

      bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `Cargo.toml`: Add direct `zbus = { version = "5.15.0", default-features = false, features = ["blocking-api"] }` and `url = "2.5.8"` for the Linux portal folder-picker path.
      - `src/client/file_dialog.rs`: Generalize dialog result/kinds and add folder picker backend.
      - `src/client/mod.rs`: Add enqueue method for selected-folder workspace grants using a single-use selected-path capability.
      - `src/protocol/mod.rs`: Add `ClientMessage::AddSelectedWorkspaceRoot` or equivalent selected-folder message.
      - `src/server/connection.rs`: Consume the capability, call `WorkspaceState::add_explicit_user_grant`, reject non-directories for folder flow, and publish refreshed file-browser SDUI.
      - `src/main.rs`: Route `clay.workspace.clientOpenFolderDialog` and convert dialog results into selected-folder queue events or diagnostics.
      - `runtime/js/workspace.ts`: Add `clientOpenFolderDialog()` command-ID helper.
      - `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`: New public command-ID doc. Deferred to the dedicated Clay JS API task to avoid half-updating registry/inventory artifacts.
      - `tests/fixtures/configuration/file-browser-workflow/init.js`: Bind folder picker for manual smoke. Deferred to the end-to-end smoke docs/fixture task.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`.
      - `docs/wiki/modules/client-file-dialog.md`.
  - Test Cases to Write:
    - Non-Windows/portal unit tests: URI parsing, cancellation, unsupported/missing portal diagnostics, and sanitized failure mapping.
    - Windows-gated tests: folder-dialog flags include `FOS_PICKFOLDERS` and every unsafe COM block has `// SAFETY:`.
    - Protocol round trip: selected-folder message serializes/deserializes through `Codec`.
    - Server connection test: valid capability + selected directory adds/deduplicates a root and returns `SduiSnapshot`; invalid/stale capability rejects without adding a root.
    - Main driver conversion test: cancelled folder picker is no-op; unsupported/failed picker surfaces `RuntimeDiagnostic`.
  - Execution Notes (2026-07-07):
    - Added Linux folder picker backend in `src/client/file_dialog.rs` using `zbus` blocking API against `org.freedesktop.portal.FileChooser.OpenFile` with `directory=true`, parsing returned `file://` URI values with `url`.
    - Added Windows folder picker support by reusing `IFileOpenDialog` and adding `FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR`; existing SAFETY-comment checks still pass.
    - Added `ClientMessage::AddSelectedWorkspaceRoot`, protocol round-trip coverage, and `ClientEditQueue::enqueue_add_selected_workspace_root` using the existing single-use selected-path capability token.
    - Added GUI route handling for `clay.workspace.clientOpenFolderDialog`, `open_folder_dialog()`, and `EditorWidget::request_selected_workspace_root`.
    - Added server handling that consumes the capability, calls `WorkspaceState::add_root` for the selected directory, refreshes `FileBrowserState`, stores the tree in `StaticSduiState`, and sends `ServerMessage::SduiSnapshot`; stale capabilities reject with `clay.client.selected_folder_open.unauthorized` and add no root.
    - Added `runtime/js/workspace.ts::clientOpenFolderDialog`, embedded runtime facade export, and `bindKey` allowlist/routing as `ClientUiCommand`.
    - Updated `docs/wiki/modules/client-file-dialog.md` with the folder picker/backend/authority flow. Public Clay JS API doc, generated registry, and smoke fixture remain intentionally deferred to later already-scoped plan tasks.
    - Validation passed:
      - `cargo check --all-targets`
      - `cargo test --lib --quiet` (587 passed)
      - `cargo test --lib file_dialog --quiet`
      - `cargo test --lib selected_folder --quiet`
      - `cargo test --lib connection_add_selected_workspace_root --quiet`
      - `cargo test --lib configuration_binds_client_ui_file_and_folder_dialogs --quiet`
      - `cargo test --lib protocol_round_trips_open_save_reload_messages --quiet`
      - `cargo test --bin clay file_dialog_result_conversion_reports_selected_and_sanitized_failures --quiet`
      - `cargo test --test clay_js_facade_layout --quiet`
      - `cargo test --test clay_js_api_inventory --quiet`
      - `cargo test --test clay_js_doc_registry --quiet`
      - `cargo test --lib workspace::tests --quiet`
    - Known unrelated validation blockers remain:
      - `cargo fmt --check` still fails on pre-existing formatting diffs in `src/packages/record.rs`, `src/server/js_runtime.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`; this task avoided applying broad unrelated formatting churn.
      - `cargo test --test rust_visibility_api_mapping --quiet` still fails on pre-existing unmapped public items `src/server/command_execution.rs::GitCommandResult` and `src/server/git.rs::GitCachedStatus`.

- [x] Implement real file-browser directory navigation and SDUI refresh
  - Acceptance Criteria:
    - Functional: Directory entries no longer route through file open. Clicking a directory navigates into that directory (or expands it, if implementation chooses expand/collapse), renders a parent/root affordance, and keeps file entries openable from the current directory.
    - Performance: Directory navigation uses `WorkspaceState::list_directory` with existing max-depth/max-entry bounds and sends one bounded SDUI snapshot/update per explicit navigation; no filesystem reads happen in Masonry paint/layout/input hot paths.
    - Code Quality: Keep the UI Clay-owned and inert SDUI-based; do not add a native `FileTreeWidget`, Masonry child mutation branch, or package-contributed file browser.
    - Security: Every directory path is root-relative, canonicalized server-side, traversal-checked, and scoped to a known workspace root; packages cannot add roots, markers, ignore rules, or arbitrary list scopes.
  - Approach:
    - Documentation Reviewed:
      - `src/shell/file_browser.rs`: Current state, SDUI list actions, and fuzzy session.
      - `src/server/workspace.rs`: `FileListRequest`, traversal checks, ignore rules, child counts, truncation.
      - `src/server/command_execution.rs`: Built-in workspace command validation and `WorkspaceActionResult`.
      - `docs/wiki/modules/workspace-file-browser.md`: Existing file-browser composition and deferred navigation.
      - `.agents/skills/project-patterns/references/package-ui-layout.md`: Clay-owned UI/component boundary.
    - Options Considered:
      - Full expandable tree with per-client expanded set: richer, but more state and more UI complexity than needed.
      - Single-current-directory navigation with a `../` parent item: smaller and enough to navigate folders/files.
      - Keep showing depth-2 snapshot only: already fails the requirement because directory clicks try to open directories.
    - Chosen Approach:
      - Implement the smallest real navigator: `FileBrowserState::from_workspace_at(root_id, relative_path)` lists the current directory, renders a parent row when not at root, renders directory rows with a new directory-navigation command, and renders file rows with `openFile`. Preserve root selection after selected-folder grants.
    - API Notes and Examples:
      ```rust
      let browser = FileBrowserState::from_workspace_at(&workspace, root_id, "src".into())?;
      let tree = browser.to_sdui_tree(active_document_id, active_document_version);
      ```
      ```json
      { "workspaceRootId": 1, "relativePath": "src" }
      ```
    - Files to Create/Edit:
      - `src/shell/file_browser.rs`: Add current-directory state, parent/root rows, directory action IDs, and tests.
      - `src/server/command_execution.rs`: Add/validate `clay.workspace.openDirectory` or `clay.workspace.navigateDirectory` result shape.
      - `src/server/connection.rs`: Map directory navigation results to refreshed `SduiSnapshot`/`SduiUpdate` using the active editor document id/version.
      - `src/server/sdui.rs`: Add helper if needed to query/update current document binding safely.
      - `docs/reference/clay-js-api/commands/server-open-directory.md` or equivalent: Document any public programmatic command facade if promoted.
    - References:
      - `docs/wiki/modules/phase18.12-workspace-discovery-primitive-review.md`.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`.
  - Test Cases to Write:
    - File-browser unit: directory rows carry directory-navigation action; file rows carry open-file action; parent row navigates upward; traversal-like paths are never emitted.
    - Workspace command test: directory navigation rejects unknown roots, files-as-directories, and traversal escapes; valid directories return a bounded listing state.
    - Connection test: an SDUI directory action returns a refreshed `SduiSnapshot` instead of `FileOperationFailed`/directory-open.
    - GUI state test: after opening a file, refreshed file-browser SDUI binds the editor region to the current document id/version so the panel does not orphan the editor binding.
  - Execution Notes (2026-07-07):
    - Added `FileBrowserState::from_workspace_at(workspace, root_id, relative_path)` and a `current_directory` field. Root snapshots and selected-folder snapshots still call `from_workspace`; navigation snapshots call `from_workspace_at`.
    - Changed file-browser listing snapshots to current-directory depth-1 pages. Directory rows now emit `clay.workspace.openDirectory`; file/symlink/other rows keep `clay.workspace.openFile`. Non-root directories render a `../` parent row that navigates upward.
    - Added `WorkspaceActionResult::Navigated { root_id, relative_path }`, built-in command registration/display text, argument parsing, and validation through `WorkspaceState::list_directory` with tight bounds before any SDUI refresh.
    - Updated connection command routing so validated SDUI directory actions rebuild the Clay-owned file-browser tree, store it in `StaticSduiState`, and send one `ServerMessage::SduiSnapshot` for the explicit navigation click. Invalid SDUI actions are now rejected before command execution, so arbitrary undeclared list scopes do not bypass `StaticSduiState` validation.
    - Reused the same `file_browser_snapshot_message` helper for selected-folder root additions and directory navigation. No native widget, package UI contribution, Masonry child mutation branch, or paint/layout filesystem work was added.
    - Updated `docs/wiki/modules/workspace-file-browser.md` with current-directory navigation behavior and tests.
    - Validation passed:
      - `cargo check --all-targets`
      - `cargo clippy --all-targets -- -D warnings`
      - `cargo test --lib --quiet` (589 passed)
      - `cargo test --lib file_browser --quiet`
      - `cargo test --lib workspace_directory_action_sends_refreshed_file_browser_snapshot --quiet`
      - `cargo test --lib command_execution --quiet`
      - `cargo test --test clay_js_api_inventory --quiet`
      - `cargo test --test clay_js_doc_registry --quiet`
      - `cargo test --test clay_js_facade_layout --quiet`
    - Known unrelated validation blockers remain:
      - `cargo fmt --check` still fails on pre-existing formatting diffs in `src/packages/record.rs`, `src/server/js_runtime.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`. Task-touched Rust files were formatted with `rustfmt --edition 2024`.
      - `cargo test --test rust_visibility_api_mapping --quiet` still fails on pre-existing unmapped public items `src/server/command_execution.rs::GitCommandResult` and `src/server/git.rs::GitCachedStatus`.

- [x] Reuse generic open-time language activation for workspace/file-browser opens
  - Acceptance Criteria:
    - Functional: Files opened through `OpenDocument`, file-browser `openFile`, and fuzzy-open receive the same behavior manifest/decorations/runtime diagnostics path as selected-file opens. `.rs`, `.ts/.tsx`, and `.js/.jsx/.mjs/.cjs` opened from the browser display contents and activate loaded language-package behavior/decorations where available.
    - Performance: Open-time classification/parse work happens after explicit open and remains bounded/background; ordinary edits still use delta IPC and local paint without JavaScript or full-document snapshots.
    - Code Quality: Replace selected-file-specific follow-up naming with a generic open-document helper; no Rust branches for Rust, TypeScript, JavaScript, Markdown, or file-browser origin.
    - Security: Classification uses already-open metadata/text slices only; package loading and parse handlers stay server-side and permissioned; no workspace scan/toolchain/network/shell authority is added.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection.rs`: former `selected_file_open_followup_messages`, `classify_open_document`, `schedule_open_parse`.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Generic selected-file open-time parse activation.
      - `docs/wiki/modules/first-party-language-packages.md`: Language package mode/grammar/completion/status flow.
      - `docs/wiki/modules/syntax-grammar-registry.md`: Syntax grammar selection and background parse/decor path.
      - `.agents/skills/project-patterns/references/language-capability-sequencing.md`.
    - Options Considered:
      - Add follow-up calls only for file-browser actions: fixes visible path but leaves `OpenDocument` inconsistent.
      - Promote selected-file follow-up helper to all successful document opens: one shared root-cause fix.
    - Chosen Approach:
      - Rename/generalize `selected_file_open_followup_messages` to `open_document_followup_messages` and call it from `OpenDocument`, `OpenSelectedFile`, and workspace command `Opened` results.
    - API Notes and Examples:
      ```rust
      let messages = open_document_followup_messages(
          client_id,
          &metadata,
          &text,
          &behavior,
          &sdui,
          runtime.id,
          &runtime.service,
          &parse_coordinator,
      ).await;
      ```
    - Files to Create/Edit:
      - `src/server/connection.rs`: Generalize helper and call sites.
      - `src/server/command_execution.rs`: Preserve `WorkspaceActionResult::Opened(snapshot)` result; no language-specific changes.
      - `src/masonry_editor.rs`: Ensure `DocumentOpened`, later `BehaviorManifest`, and `DecorationSet` events apply in order.
      - `tests/fixtures/configuration/file-browser-workflow/`: Add small `.rs`, `.ts`, and `.js` files plus init loading language packages.
      - `tests/selected_file_markdown_smoke.rs` or new integration test: Cover generic open follow-ups for workspace opens.
    - References:
      - `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`.
  - Test Cases to Write:
    - Connection test: `ClientMessage::OpenDocument` for `.rs` with language packages loaded sends `DocumentOpened`, `BehaviorManifest`, and optional `DecorationSet`/diagnostic.
    - SDUI file-browser test: list-item open for `.ts` or `.js` sends the same follow-ups.
    - Fallback test: with no language packages loaded, opening `.rs` still displays text and installs `core.code` behavior without diagnostics.
    - Regression test: selected-file Markdown smoke still passes after helper rename.
  - Execution Notes (2026-07-07):
    - Renamed the selected-file-only helper to `open_document_followup_messages` and removed the unused client-id argument. The helper still classifies already-open metadata/text through the persistent JS runtime, lazily loads first-party packages when needed, applies runtime outputs, schedules bounded parse work, and emits behavior/decorations/diagnostics.
    - Updated `ClientMessage::OpenDocument` to send `DocumentOpened` first, then run `open_document_followup_messages`; this replaces the old manifest-only follow-up path.
    - Updated selected-file opens to use the renamed generic helper with unchanged selected-file capability semantics and replenishment ordering.
    - Updated validated SDUI workspace/file-browser opens (`WorkspaceActionResult::Opened`) to send `DocumentOpened` and then the same generic open follow-ups. Directory navigation snapshots remain separate and do not run mode activation.
    - Runtime hot-reload refresh now calls `connection::open_document_followup_messages` for already-open documents.
    - Added `file_browser_open_uses_generic_open_document_followups`, covering a file-browser list-item Markdown open that receives `DocumentOpened`, Markdown `BehaviorManifest`, and `DecorationSet` through the same helper. Existing `OpenDocument` and selected-file Markdown tests pass after the helper rename.
    - Updated wiki docs: `server-ipc-skeleton.md`, `workspace-file-browser.md`, `embedded-js-runtime.md`, `persistent-runtime-hot-reload.md`, and the Phase 18.5 primitive review/test wording.
    - Validation passed:
      - `cargo check --all-targets`
      - `cargo clippy --all-targets -- -D warnings`
      - `cargo test --lib --quiet` (590 passed)
      - `cargo test --lib file_browser_open_uses_generic_open_document_followups --quiet`
      - `cargo test --lib selected_markdown_file_publishes_manifest_and_decorations --quiet`
      - `cargo test --lib connection_open_document_sends_snapshot_and_manifest_without_full_document_on_edit_ack --quiet`
      - `cargo test --test selected_file_markdown_smoke --quiet`
      - `cargo test --test clay_js_api_inventory --quiet`
      - `cargo test --test clay_js_doc_registry --quiet`
      - `cargo test --test clay_js_facade_layout --quiet`
      - `cargo test --test primitives_docs phase18_5_markdown_replan_primitive_review_records_existing_inventory --quiet`
    - Known unrelated validation blockers remain:
      - `cargo fmt --check` still fails on pre-existing formatting diffs in `src/packages/record.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`.
      - `cargo test --test rust_visibility_api_mapping --quiet` still fails on pre-existing unmapped public items `src/server/command_execution.rs::GitCommandResult` and `src/server/git.rs::GitCachedStatus`.

- [x] Add editor selection copy to OS clipboard
  - Acceptance Criteria:
    - Functional: When editor selection is non-empty, `Ctrl+C` on Linux/Windows and `Cmd+C` on macOS copies exactly the selected UTF-8 text to the system clipboard; collapsed/no selection is a no-op with no document mutation.
    - Performance: Copy reads only the selected rope range on explicit user command; it sends no edit IPC, performs no server work, executes no JavaScript, and does no filesystem work.
    - Code Quality: Add the smallest reusable editor method (`selected_text`) and a small client clipboard wrapper; avoid app-wide clipboard services or speculative paste/cut support.
    - Security: Only the native client can write the current editor selection to clipboard after a user key/command route; server packages cannot set arbitrary clipboard text, read clipboard contents, or trigger hidden clipboard writes.
  - Approach:
    - Documentation Reviewed:
      - `src/editor/selection.rs`: Anchor/focus normalized byte range.
      - `src/editor/buffer.rs`: `EditorBuffer::text_range` UTF-8-safe extraction.
      - `src/editor/surface.rs`: Existing `selected_range` and selection tests.
      - `src/masonry_editor.rs`: Key routing and local command flow.
      - Context7 `/websites/rs_arboard` docs: `Clipboard::new()?`, `set_text(text)`, Linux `SetExtLinux` / `LinuxClipboardKind::Clipboard` examples.
      - Local `copypasta 0.10.2` docs were considered because it is already in `Cargo.lock`, but `ClipboardContext` aliases X11 on Unix and Wayland support needs raw display plumbing.
    - Options Considered:
      - Per-platform clipboard code: avoids a dependency but is more code and risk.
      - `copypasta`: already resolved transitively, but Wayland default ergonomics are weaker for this app.
      - `arboard`: one small direct dependency with current text clipboard API and Linux/Windows/macOS support.
    - Chosen Approach:
      - Add `arboard` for the OS clipboard write path and keep tests on a fake in-memory sink so CI does not require a desktop clipboard.
    - API Notes and Examples:
      ```rust
      let mut clipboard = arboard::Clipboard::new()?;
      clipboard.set_text(selected_text)?;
      ```
      ```rust
      assert_eq!(surface.selected_text(), Some("snippet".to_string()));
      ```
    - Files to Create/Edit:
      - `Cargo.toml`: Add `arboard` with Linux clipboard support verified during implementation.
      - `src/client/clipboard.rs`: Small wrapper around `arboard` plus test fake sink hook.
      - `src/client/mod.rs`: Re-export clipboard result/function if needed.
      - `src/editor/surface.rs`: Add `selected_text() -> Option<String>`.
      - `src/masonry_editor.rs`: Handle copy shortcut/client UI command before text insertion routing; surface diagnostics on clipboard failure.
      - `runtime/js/editor.ts`: Add `clientCopySelection()` command-ID helper if exposing bindable copy.
      - `docs/reference/clay-js-api/editor/client-copy-selection.md`: Document the command ID if exposed.
    - References:
      - `.agents/skills/project-patterns/references/behavior-manifests.md` for client-first predictable/native UI split.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md` if `clientCopySelection` is public.
  - Test Cases to Write:
    - Editor surface unit: forward/backward selections return exact selected text; collapsed selections return `None`; Unicode byte boundaries are safe.
    - Masonry editor unit: copy command with fake clipboard writes selected text and emits no edit event.
    - Masonry editor unit: no selection/collapsed selection is no-op.
    - Failure path unit: clipboard backend failure surfaces sanitized `RuntimeDiagnostic` without panicking or mutating text.
  - Execution Notes (2026-07-07):
    - Added direct `arboard = "3.6.1"` dependency for cross-platform OS clipboard text writes. Context7 verified `Clipboard::new()` and `set_text()` accept UTF-8 text; Linux-specific docs also show `LinuxClipboardKind::Clipboard` for traditional copy/paste clipboard behavior.
    - Added `src/client/clipboard.rs`: tiny `ClipboardSink` trait, `SystemClipboard` wrapper, `ClipboardError`, and `copy_text_to_system_clipboard`. Tests use fake/in-memory sinks only, so CI does not require a desktop clipboard.
    - Added `EditorSurface::selected_text() -> Option<String>`, backed by normalized selection ranges and `EditorBuffer::text_range`; collapsed selections return `None`.
    - Added `EditorWidget::copy_selection_to_system_clipboard()` and `copy_selection_to_clipboard_with()`. Native text-event routing handles `Ctrl+C` on Linux/Windows and `Cmd+C` on macOS before character insertion/manifest routing. Success emits no edit event, no IPC, and no server/JS/filesystem work; failure emits sanitized `clay.client.clipboard.write_failed` runtime diagnostic.
    - Deliberately skipped paste, cut, clipboard read, server/package clipboard APIs, and arbitrary clipboard writes.
    - Updated wiki docs: `masonry-editor.md` and `end-to-end-file-browser-workflow-primitive-review.md`.
    - Validation passed:
      - `cargo check --all-targets`
      - `cargo clippy --all-targets -- -D warnings`
      - `cargo test --lib --quiet` (596 passed)
      - `cargo test --lib selected_text --quiet`
      - `cargo test --lib copy_selection --quiet`
      - `cargo test --lib clipboard_sink_accepts_utf8_text --quiet`
      - `cargo test --test primitives_docs end_to_end_file_browser_workflow_primitive_review_records_inventory_and_gaps --quiet`
    - Known unrelated validation blockers remain:
      - `cargo fmt --check` still fails on pre-existing formatting diffs in `src/packages/record.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`.
      - `cargo test --test rust_visibility_api_mapping --quiet` still fails on pre-existing unmapped public items `src/server/command_execution.rs::GitCommandResult` and `src/server/git.rs::GitCachedStatus`.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Folder picker, file-browser navigation, fuzzy-open/toggle, language package loading, and copy selection are reachable through documented configuration/keybinding paths where configurability is needed.
    - Performance: Configuration evaluation happens at startup/reload only; configured key routes install inert behavior-manifest entries and do not run JavaScript on keypress.
    - Code Quality: No hidden config keys are added. Every configurable command is a documented Clay JS API or documented built-in command ID accepted by `bindKey`.
    - Security: Configuration cannot grant arbitrary filesystem, network, shell, extension loading, package manager, AI mutation, workspace, clipboard-read, or client-side JavaScript authority; client UI commands remain user-routed intents.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`.
      - `src/server/ops/keybindings.rs`: Runtime-bindable command allowlist and routing policy mapping.
      - `docs/wiki/modules/behavior-runtime-registration.md`: Client UI command routing.
    - Options Considered:
      - Hard-code new shortcuts only: works but violates existing client UI command/configuration convention.
      - Use `bindKey` for folder picker and optional copy command while keeping standard copy shortcut native: discoverable and consistent.
    - Chosen Approach:
      - Add `clay.workspace.clientOpenFolderDialog` and, if public, `clay.editor.clientCopySelection` to the bindKey allowlist with `RoutingPolicy::ClientUiCommand`. Keep `openDirectory`/`openFile` server-first command IDs documented for server-side actions.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      import { clientOpenFolderDialog } from "clay:workspace";
      import { clientCopySelection } from "clay:editor";
      import { loadPackage } from "clay:packages";

      await loadPackage("@clay/rust");
      await loadPackage("@clay/typescript");
      await loadPackage("@clay/javascript");
      bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
      bindKey("Ctrl+C", clientCopySelection(), { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `src/server/ops/keybindings.rs`: Allowlist/routing policy for new bindable client UI commands and server-first navigation command if bindable.
      - `runtime/js/workspace.ts`, `runtime/js/editor.ts`: Command-ID helper exports.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Examples and security notes.
      - `tests/fixtures/configuration/file-browser-workflow/init.js`: Workflow config fixture.
      - `src/server/js_runtime.rs`: Inline facade copies/tests if this project still mirrors runtime JS there.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
      - `.agents/skills/project-patterns/references/configuration-system.md`.
  - Test Cases to Write:
    - JS runtime test: `bindKey("Ctrl+Shift+O", "clay.workspace.clientOpenFolderDialog")` produces `ClientUiCommand` route.
    - JS runtime test: `bindKey("Ctrl+C", "clay.editor.clientCopySelection")` produces `ClientUiCommand` route if exposed.
    - Rejection test: unknown dialog/clipboard raw command IDs and raw `Deno.core.ops` strings remain rejected.
    - Fixture test: workflow fixture loads three language packages and registers folder/copy bindings without installing hidden keys.
  - Execution Notes (2026-07-07):
    - Added public synchronous `clientCopySelection()` command-id helper to `runtime/js/editor.ts` and the embedded `clay:editor` facade. It returns only the fixed string `clay.editor.clientCopySelection`; it performs no clipboard work during configuration evaluation.
    - Added `clay.editor.clientCopySelection` to `bindKey` runtime allowlist with `RoutingPolicy::ClientUiCommand`. Added `clay.workspace.openFuzzyFile` and `clay.workspace.toggleFileBrowser` to the allowlist so the previously documented file-browser bindings are actually accepted. Folder picker (`clay.workspace.clientOpenFolderDialog`) remains a client UI command.
    - Wired `clay.editor.clientCopySelection` through `main.rs` client UI command handling to call `EditorWidget::copy_selection_to_system_clipboard()` on the shell-owned editor child. Alternate configured copy chords therefore share the same client-only copy implementation as the native shortcut.
    - Added `tests/fixtures/configuration/file-browser-workflow/init.js`: loads `@clay/rust`, `@clay/typescript`, and `@clay/javascript`; binds `Ctrl+Shift+O` to `clientOpenFolderDialog()`, `Ctrl+P` to `clay.workspace.openFuzzyFile`, `Ctrl+B` to `clay.workspace.toggleFileBrowser`, and `Ctrl+Shift+C` to `clientCopySelection()`.
    - Updated `bindKey` and configuration docs/wiki to document folder picker, fuzzy/toggle, language package load fixture, and copy-selection configuration as documented Clay JS APIs/command IDs rather than hidden configuration keys.
    - Kept directory row navigation out of user keybinding configuration because `clay.workspace.openDirectory` needs SDUI-provided `{ workspaceRootId, relativePath }` arguments; it remains a validated file-browser action, not a global chord.
    - Security boundary verified: configuration installs inert behavior-manifest routes only; it does not grant arbitrary filesystem paths, raw dialogs, raw clipboard writes, clipboard reads, paste/cut, server/package clipboard APIs, network, shell, package-manager, AI, WASM, raw op, or client-JS authority.
    - Validation passed:
      - `cargo check --all-targets`
      - `cargo clippy --all-targets -- -D warnings`
      - `cargo test --lib --quiet` (598 passed)
      - `cargo test --lib configuration_binds_client_ui_file_folder_and_copy_commands --quiet`
      - `cargo test --lib raw_clipboard_and_dialog_command_bindings_are_rejected --quiet`
      - `cargo test --lib file_browser_workflow_config_fixture_loads_packages_and_bindings --quiet`
      - `cargo test --bin clay client_copy_selection_command_routes_to_editor_widget --quiet`
      - `cargo test --test clay_js_facade_layout --quiet`
      - `cargo test --test clay_js_api_inventory --quiet`
      - `cargo test --test clay_js_doc_registry --quiet`
      - `cargo test --test primitives_docs end_to_end_file_browser_workflow_primitive_review_records_inventory_and_gaps --quiet`
    - Known unrelated validation blockers remain:
      - `cargo fmt --check` still fails on pre-existing formatting diffs in `src/packages/record.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`.
      - `cargo test --test rust_visibility_api_mapping --quiet` still fails on pre-existing unmapped public items `src/server/command_execution.rs::GitCommandResult` and `src/server/git.rs::GitCachedStatus`.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Every public/bindable surface added or changed by this plan has a stable Clay JS API doc, JS facade export, generated registry entry, docs index link, user-facing name, key binding metadata, custom properties metadata, permissions/security notes, and lookup tags.
    - Performance: API docs state hot-path policy and async/sync behavior; public helpers that return command IDs stay synchronous and side-effect free.
    - Code Quality: New server-side Rust public functions are either exposed through explicit ops/facades/docs or kept private/`pub(crate)`; raw `Deno.core.ops` are not user-facing.
    - Security: API docs explicitly deny broad filesystem/workspace, network, shell, extension loading, package manager, AI, WASM, raw-op, native-widget, client-JS, and clipboard-read authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API task.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `documentation-as-code.md`, `doc-registry-tests.md`.
      - Existing docs: `docs/reference/clay-js-api/documents/client-open-file-dialog.md`, `docs/reference/clay-js-api/commands/server-open-file.md`, `docs/reference/clay-js-api/workspace/server-add-workspace-root.md`, `docs/reference/clay-js-api/workspace/server-list-directory.md`.
    - Options Considered:
      - Treat new command IDs as internal only: less docs now, but not discoverable through help/configuration.
      - Promote the user-facing command IDs and server navigation command with docs/registry coverage: matches Clay's documentation-as-code policy.
    - Chosen Approach:
      - Document new command-ID helpers (`clientOpenFolderDialog`, optional `clientCopySelection`) and any public `serverOpenDirectory`/navigation helper. Re-run registry generation and tests.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test clay_js_doc_registry --quiet
      cargo test --test clay_js_api_inventory --quiet
      cargo test --test clay_js_facade_layout --quiet
      cargo test --test rust_visibility_api_mapping --quiet
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`: New API doc.
      - `docs/reference/clay-js-api/editor/client-copy-selection.md`: New API doc if public.
      - `docs/reference/clay-js-api/commands/server-open-directory.md`: New API doc if public.
      - `docs/reference/clay-js-api/api-inventory.toml`: Inventory rows if required by current docs tooling.
      - `docs/index.md`: Link new docs.
      - `docs/generated/clay-js-api-registry.json`: Regenerate.
      - `tests/clay_js_doc_registry.rs`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_facade_layout.rs`, `tests/rust_visibility_api_mapping.rs`: Add/adjust coverage.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`.
      - `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`.
  - Test Cases to Write:
    - Registry lookup test: new API IDs resolve by stable ID, JS export, tags, and app/help lookup.
    - Inventory test: docs include user-facing name, key bindings list, custom properties list, security text, facade/op/backing paths.
    - Rust visibility test: no new server-side public Rust function lacks a Clay JS API or explicit `pub(crate)` boundary.
  - Execution Notes (2026-07-07):
    - Added public Clay JS API docs:
      - `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md` for `clay.workspace.clientOpenFolderDialog` / `clientOpenFolderDialog()`.
      - `docs/reference/clay-js-api/editor/client-copy-selection.md` for `clay.editor.clientCopySelection` / `clientCopySelection()`.
      - `docs/reference/clay-js-api/commands/server-open-directory.md` for `clay.commands.serverOpenDirectory` / `serverOpenDirectory()`.
    - Added `serverOpenDirectory()` to `runtime/js/commands.ts` and the embedded `clay:commands` facade. It wraps `clay.workspace.openDirectory`, validates the `navigated` command result, and returns `{ workspaceRootId, relativePath }`.
    - Updated `docs/reference/clay-js-api/api-inventory.toml`, `docs/index.md`, and regenerated `docs/generated/clay-js-api-registry.json` with `cargo run --bin update-doc-registry`.
    - Added registry coverage in `tests/clay_js_doc_registry.rs` verifying stable IDs, JS exports, lookup tags, app/help visibility, custom property metadata, and security metadata for the folder picker, copy selection, and directory navigation APIs.
    - Updated facade layout coverage for `clientCopySelection` and `serverOpenDirectory`. Adjusted the Clay JS naming convention test to allow `clientCopySelection`; the previous raw-op substring guard falsely matched the ordinary word `Copy`.
    - API/security docs state sync/async behavior and hot-path boundaries:
      - command-ID helpers are synchronous and side-effect free;
      - `serverOpenDirectory()` is asynchronous server action work;
      - no API grants broad filesystem/workspace authority, network, shell, extension loading, package manager, AI, WASM, raw Deno ops, native widget authority, client-side JavaScript, clipboard reads, paste/cut, or arbitrary clipboard writes.
    - Validation passed:
      - `cargo run --bin update-doc-registry`
      - `cargo check --all-targets`
      - `cargo clippy --all-targets -- -D warnings`
      - `cargo test --lib --quiet` (598 passed)
      - `cargo test --test clay_js_doc_registry --quiet` (29 passed)
      - `cargo test --test clay_js_api_inventory --quiet` (54 passed)
      - `cargo test --test clay_js_facade_layout --quiet` (4 passed)
    - Known unrelated validation blockers remain:
      - `cargo fmt --check` still fails on pre-existing formatting diffs in `src/packages/record.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`.
      - `cargo test --test rust_visibility_api_mapping --quiet` still fails on pre-existing unmapped public items `src/server/command_execution.rs::GitCommandResult` and `src/server/git.rs::GitCachedStatus`.

- [x] Update end-to-end docs, fixtures, and manual smoke contract
  - Acceptance Criteria:
    - Functional: Docs tell a user exactly how to complete the six-step workflow on Linux: launch app, see Workspace browser, open folder picker, navigate folders/files, open `.rs`/`.ts`/`.js`, select text, and copy to clipboard.
    - Performance: Manual smoke docs call out expected asynchronous behavior for language decorations and no blocking ordinary typing.
    - Code Quality: Old docs/tests that say bare `cargo run` must not show a Workspace side panel are updated or narrowed so they do not contradict the app-level file-browser workflow.
    - Security: Manual docs explain selected-folder root authority, selected-file/file-open validation, no shell/network/package-manager access, and clipboard write-only/current-selection limits.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md` current smoke contracts.
      - `tests/manual_smoke_docs.rs`: Static docs coverage.
      - `tests/fixtures/configuration/language-packages/init.js`: Existing language package load fixture.
      - `docs/reference/packages/creating-packages.md`: Package authoring contract if impacted by file-browser docs.
    - Options Considered:
      - Add a new doc section only: faster, but stale assertions can keep failing or misleading users.
      - Update docs plus tests and fixture: keeps docs authoritative and prevents regression.
    - Chosen Approach:
      - Add a dedicated "End-to-end file browser workflow smoke" section and a deterministic fixture that loads language packages and binds the folder/copy commands. Narrow older Markdown "no default panel" wording to package-published Markdown panels only, not the app-level Workspace browser.
    - API Notes and Examples:
      ```bash
      cargo run -- smoke-gui --config-fixture file-browser-workflow
      ```
      ```js
      import { bindKey } from "clay:keybindings";
      import { loadPackage } from "clay:packages";
      import { clientOpenFolderDialog } from "clay:workspace";

      await loadPackage("@clay/rust");
      await loadPackage("@clay/typescript");
      await loadPackage("@clay/javascript");
      bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`: Add workflow smoke section and update contradictory default-surface wording.
      - `tests/manual_smoke_docs.rs`: Add coverage for the new smoke section and adjust old assertions.
      - `tests/fixtures/configuration/file-browser-workflow/init.js`: New fixture.
      - `tests/fixtures/configuration/file-browser-workflow/workspace/main.rs`, `main.ts`, `main.js`: Small files for smoke docs/tests.
      - `docs/reference/packages/creating-packages.md`: Update if package/browser boundaries need clarification.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
  - Test Cases to Write:
    - Manual smoke docs test: launch docs include the exact six-step workflow and fixture command.
    - Fixture test: `file-browser-workflow/init.js` loads the three language packages and binds folder picker/copy command IDs.
    - Docs regression: older Markdown no-default-panel tests now distinguish package Markdown panels from Clay-owned Workspace browser.
  - Execution Notes (2026-07-07):
    - Updated `docs/development/launch-and-gui-smoke.md` with a dedicated `End-to-end file browser workflow smoke` section for Linux. It documents the six-step workflow exactly: launch app, see Workspace file browser, select a system folder, navigate folders/files, open Rust/TypeScript/JavaScript files, and copy text snippets to the OS clipboard.
    - Added the runnable command `cargo run -- smoke-gui --config-fixture file-browser-workflow` to quick commands and the workflow section.
    - Documented the fixture shape using only end-user Clay JS APIs: `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, `loadPackage("@clay/javascript")`, `clientOpenFolderDialog()`, `clay.workspace.openFuzzyFile`, `clay.workspace.toggleFileBrowser`, and `clientCopySelection()`.
    - Narrowed older Markdown default-surface wording: Markdown package loading still publishes no package-owned preview/status `PanelContribution`, but this no longer claims that bare app workflows must never show Clay-owned Workspace file-browser chrome.
    - Narrowed the Phase 19 Windows Markdown open-dialog out-of-scope wording so Linux folder selection, directory navigation, and workspace-root expansion are explicitly covered by the separate end-to-end file-browser smoke.
    - Updated `docs/reference/packages/creating-packages.md` to clarify that `clay.workspace.openDirectory` is a built-in server-first command requiring Clay-provided root-relative SDUI arguments, and that folder picker/copy-selection command IDs do not grant packages native path handles or clipboard authority.
    - Updated `docs/wiki/modules/workspace-file-browser.md` with links to the new smoke contract and test/fixture coverage.
    - Added `tests/manual_smoke_docs.rs::end_to_end_file_browser_workflow_smoke_has_runnable_fixture_contract`, covering the exact six-step docs, fixture command, fixture package loads/bindings, sample `main.rs`/`main.ts`/`main.js` paths, directory navigation, copy behavior, security markers, and raw-authority rejection in the fixture.
    - Updated existing manual smoke docs regression tests to distinguish Markdown package panels from Clay-owned Workspace file-browser chrome, and to scope the old Phase 19 out-of-scope markers to the Windows Markdown file-dialog smoke only.
    - Validation passed:
      - `cargo check --all-targets`
      - `cargo clippy --all-targets -- -D warnings`
      - `cargo test --lib --quiet` (598 passed)
      - `cargo test --test manual_smoke_docs --quiet` (9 passed)
      - `cargo test --lib file_browser_workflow_config_fixture_loads_packages_and_bindings --quiet`
      - `cargo test --test clay_js_api_inventory --quiet` (54 passed)
      - `cargo test --test clay_js_doc_registry --quiet` (29 passed)
      - `cargo test --test clay_js_facade_layout --quiet` (4 passed)
    - Known unrelated validation blockers remain:
      - `cargo fmt --check` still fails on pre-existing formatting diffs in `src/packages/record.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`.
      - `cargo test --test rust_visibility_api_mapping --quiet` still fails on pre-existing unmapped public items `src/server/command_execution.rs::GitCommandResult` and `src/server/git.rs::GitCachedStatus`.

- [x] Run focused and full verification
  - Acceptance Criteria:
    - Functional: All implementation tests, docs/registry tests, and Linux full-target gates pass; manual smoke instructions are executable.
    - Performance: Existing performance/hot-path invariant tests still pass, and no new source path adds filesystem/dialog/clipboard/JS work to paint/layout/ordinary edit hot paths.
    - Code Quality: `cargo fmt --check`, `cargo check --all-targets`, and `cargo clippy --all-targets -- -D warnings` pass on Linux.
    - Security: Authority regression tests pass for selected-folder capability, workspace traversal, docs registry security metadata, raw-op rejection, and no shell execution.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/maintenance-validation.md`.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`.
    - Options Considered:
      - Run only focused tests: faster, but this workflow touches protocol, GUI routing, docs, workspace, and packages.
      - Run focused tests plus full Linux gates: required for normal Clay work.
    - Chosen Approach:
      - Run focused tests during each implementation task, then full Linux gates at the end. Treat Windows checks as best-effort unless running on Windows/MSVC.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo test --lib file_browser --quiet
      cargo test --lib workspace::tests --quiet
      cargo test --lib file_dialog --quiet
      cargo test --lib clipboard --quiet
      cargo test --test clay_js_doc_registry --quiet
      cargo test --test clay_js_api_inventory --quiet
      cargo test --test manual_smoke_docs --quiet
      cargo test --test primitives_docs --quiet
      ```
    - Files to Create/Edit:
      - `plans/043-End-to-End-File-Browser-Folder-Navigation-Language-Open-and-Clipboard.md`: Mark completed tasks and record final validation evidence.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`.
      - `docs/wiki/modules/maintenance-validation.md`.
  - Test Cases to Write:
    - No new test needed beyond implementation/docs tests; this task executes the complete listed suite and records outcomes.
  - Execution Notes (2026-07-08):
    - Ran the full Linux verification suite listed for this task with `set -e` so failures stop the gate:
      - `cargo fmt --check`
      - `cargo check --all-targets`
      - `cargo clippy --all-targets -- -D warnings`
      - `cargo test --all-targets`
      - `cargo test --lib file_browser --quiet`
      - `cargo test --lib workspace::tests --quiet`
      - `cargo test --lib file_dialog --quiet`
      - `cargo test --lib clipboard --quiet`
      - `cargo test --test clay_js_doc_registry --quiet`
      - `cargo test --test clay_js_api_inventory --quiet`
      - `cargo test --test manual_smoke_docs --quiet`
      - `cargo test --test primitives_docs --quiet`
    - All gates passed after resolving two stale validation blockers uncovered by full verification:
      - Ran `cargo fmt` to clear stale formatting diffs in `src/packages/record.rs`, `src/server/ops/mod.rs`, and `tests/package_loading_docs.rs`.
      - Added an explicit non-JS infrastructure allowlist entry for `GitCommandResult` and `GitCachedStatus` in `tests/rust_visibility_api_mapping.rs`; the public Clay JS API docs cover the Git command facades, not these Rust transport helper payload types.
      - Updated `tests/syntax_grammar.rs::first_party_language_packages_load_with_required_assets` to match Phase 18.14 reality: `@clay/rust`, `@clay/typescript`, and `@clay/javascript` now provide language-mode, command, completion, and status-item contributions in addition to syntax grammar metadata, instead of the older grammar-only contract.
    - Final focused results included `cargo test --lib --quiet` (598 passed), `cargo test --test syntax_grammar --quiet` (23 passed), `cargo test --test rust_visibility_api_mapping --quiet` (11 passed), `cargo test --test manual_smoke_docs --quiet` (9 passed), `cargo test --test clay_js_api_inventory --quiet` (54 passed), `cargo test --test clay_js_doc_registry --quiet` (29 passed), and `cargo test --test primitives_docs --quiet` (96 passed).
    - `cargo test --all-targets` also ran the benchmark harnesses as test targets; all reported `Success` (with plotters fallback where `gnuplot` was unavailable).

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
      - `docs/wiki/modules/client-file-dialog.md`: Folder picker backend and selected-folder flow.
      - `docs/wiki/modules/workspace-file-browser.md`: Directory navigation, selected-root refresh, and open follow-ups.
      - `docs/wiki/modules/server-ipc-skeleton.md`: New selected-folder protocol/capability message and generic open follow-ups.
      - `docs/wiki/modules/server-file-workspace.md`: Selected-folder workspace root grant/capability flow.
      - `docs/wiki/modules/masonry-editor.md`: Clipboard copy routing and status diagnostics.
      - `docs/wiki/modules/first-party-language-packages.md`: If workflow docs alter language-open behavior.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`.
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Execution Notes (2026-07-08):
    - Verified and updated code wiki after all implementation and full verification tasks passed.
    - Updated `docs/wiki/index.md` descriptions for affected pages so the master index now advertises selected-folder grants, file/folder dialogs, clipboard selection copy, server IPC selected-root dispatch, and first-party language package workflow fixture use.
    - Updated `docs/wiki/modules/client-file-dialog.md` source/related links to include `runtime/js/workspace.ts`, `client-open-folder-dialog.md`, Linux folder picker coverage, and current Linux validation commands.
    - Updated `docs/wiki/modules/server-ipc-skeleton.md` to document `AddSelectedWorkspaceRoot`, shared selected-path capability issuance, file-browser `SduiSnapshot` refresh, generic open-document follow-ups, and relevant tests.
    - Updated `docs/wiki/modules/server-file-workspace.md` to document selected-folder workspace-root grants, server-side `WorkspaceState::add_root` validation, selected-path capability rejection, and selected-folder connection tests.
    - Updated `docs/wiki/modules/masonry-editor.md` source/related links for `src/client/clipboard.rs`, `src/editor/surface.rs`, `src/main.rs`, `runtime/js/editor.ts`, and `client-copy-selection.md`.
    - Updated `docs/wiki/modules/first-party-language-packages.md` to include the file-browser workflow fixture files, configuration snippet, and workflow tests that prove Rust/TypeScript/JavaScript package activation works through the file-browser workflow.
    - `docs/wiki/modules/workspace-file-browser.md` already covered directory navigation, selected-root refresh, open follow-ups, API docs, smoke fixture, and tests from earlier plan tasks; no further content change was needed beyond index linkage.
    - Validation passed after wiki updates:
      - `cargo test --test primitives_docs --quiet` (96 passed)
      - `cargo test --test manual_smoke_docs --quiet` (9 passed)
      - `cargo test --test package_loading_docs --quiet` (35 passed)
      - `cargo test --test clay_js_api_inventory --quiet` (54 passed)

## Compromises Made

- Kept clipboard scope intentionally minimal: copy-selection only, write-only, no paste/cut/read/arbitrary clipboard JS API. This closes the requested workflow without adding broader clipboard authority.
- Kept file-browser navigation simple: one current directory snapshot plus `../` parent row, no expand/collapse tree state or recursive native file-tree widget. Directory listing remains server-bounded and easy to validate.
- Used the existing selected-path capability family for selected-folder grants instead of inventing a separate folder-token protocol. Simpler protocol, same server-side validation shape.
- Linux folder picker is implemented with xdg-desktop-portal and Windows folder picking with COM; other platforms still return `Unsupported`.

## Further Actions

- Add paste/cut only if requested; requires new explicit authority and tests for clipboard read/mutation boundaries.
- Add persistent file-browser expand/collapse or recent folders only after users need more than current-directory navigation.
- Add save support for selected-file opens in a separate task; current workflow remains view/edit/copy oriented.
- Run a manual GUI smoke on a real Linux desktop session with xdg-desktop-portal available before release packaging.
