# Phase 19 Windows Markdown File Open Dialog Smoke

## Objectives
- Add a Windows 11 native file-open dialog that can be invoked from a user-configured Clay key binding, not from a hard-coded Rust shortcut.
- Let a user select a `.md` file, open it as a server-authorized Clay document, and edit it in the GUI without implementing save in this phase.
- Exercise the first-party Markdown mode against a real user-selected file so the manual smoke round can assess whether Markdown rendering/decorations are usable.
- Preserve Clay authority boundaries: the client owns native UI/file-picker interaction, the server owns canonical document state and file validation, package JavaScript runs server-side only, and keypress/paint/text-event paths never wait on JavaScript, IPC, or file IO except for the explicit open-dialog command.

## Expected Outcome
- A user can configure `Ctrl+O` in `~/.config/clay/init.js` with `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })`.
- On Windows 11, pressing the configured key opens the OS file browser with a Markdown file filter.
- Selecting a `.md` file sends an explicit user-selected file-open request to the server; the server validates the selected regular UTF-8 file and opens it without expanding authority to arbitrary host paths.
- The editor replaces the current buffer with the selected file contents, installs the applicable behavior manifest, activates Markdown mode when the first-party Markdown package is loaded, and publishes viewport-bounded Markdown decorations/status for the opened document.
- The user can type/edit the opened file locally with existing optimistic edit behavior. Saving the file is explicitly out of scope for this plan.
- Windows is the only OS with native dialog support in this phase. Other platforms report an unsupported command diagnostic/status without panics.

## Tasks

- [x] Confirm baseline gaps, scope, and manual smoke contract
  - Acceptance Criteria:
    - Functional: The implementation starts from a written baseline that distinguishes currently working launch/edit/Markdown fixture paths from missing Windows dialog, selected-file open, live Markdown activation, and save behavior.
    - Performance: The baseline states that the explicit open-dialog command may perform modal UI/file-open work, but ordinary typing/rendering remains client-local and non-blocking.
    - Code Quality: The manual smoke contract defines the exact in-scope scenario and explicitly excludes save, full HTML preview, non-Windows dialogs, and file associations.
    - Security: The baseline records that a user-selected path is an explicit open request, not unrestricted client filesystem authority or workspace expansion.
  - Approach:
    - Documentation Reviewed:
      - `plans/010-Phase9-File-and-Workspace-Server.md`: Server workspace, open, dirty, save/reload state, and runtime-backed document facades.
      - `plans/012-Developer-Friendly-Launch-and-GUI-Smoke.md`: Command-first launch, status UI, and smoke fixture limitations.
      - `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md`: Markdown package activation/decorations and remaining API/configuration tasks.
      - `plans/021-Phase18.5-Large-File-Markdown-Performance-and-Memory.md`: Windowed Markdown parser/decor cache contract.
      - `docs/development/launch-and-gui-smoke.md`: Current manual smoke commands and Markdown fixture behavior.
    - Options Considered:
      - Treat the current `markdown-mode` fixture as sufficient: rejected because it does not open a user-selected file.
      - Implement file associations and save together: rejected as too broad for the requested manual rendering test.
      - Add a focused Windows open-dialog/edit-only smoke path: selected.
    - Chosen Approach:
      - Document the Phase 19 smoke contract first, then implement only the minimum user-facing file-open path needed to test Markdown rendering/edit usability.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      import { bindKey } from "clay:keybindings";

      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`: Added the Phase 19 Windows Markdown open-dialog baseline, in-scope edit-only checklist, out-of-scope exclusions, and performance/security contract.
      - `docs/development/windows.md`: Linked the Windows-specific manual test and marked current Phase 19 dialog/selected-file behavior as known baseline gaps until implementation tasks complete.
      - `tests/manual_smoke_docs.rs`: Added deterministic documentation guards for the Phase 19 smoke scope and file-association exclusion.
      - `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`: Keep scope and verification notes current during execution.
    - References:
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `phase19_manual_smoke_docs_define_open_dialog_scope`: Documentation states Windows-only open dialog, Markdown file selection, edit-only scope, and save exclusion.
    - `phase19_manual_smoke_docs_reject_file_association_requirement`: Documentation does not require Windows Explorer file association for this phase.
  - Verification Completed:
    - Re-read Phase 9 file/workspace, Phase 12 launch/smoke, Phase 18 Markdown mode, Phase 18.5 large-file Markdown, current GUI smoke docs, Windows docs, and relevant project patterns before editing.
    - Added `docs/development/launch-and-gui-smoke.md` section `Phase 19 Windows Markdown open-dialog smoke contract` distinguishing currently working launch/edit/Markdown fixture paths from missing Windows dialog, bindable client UI command, selected-file IPC/grant, live selected-file Markdown activation/decorations, and selected-file save behavior.
    - Recorded the exact edit-only Windows 11 manual scenario: configure `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })`, open the OS file browser, select a regular UTF-8 Markdown file, let the server validate/grant only that file, replace the buffer, activate Markdown/decorations when loaded, type locally, and skip save.
    - Explicitly excluded save, full HTML preview/browser rendering, non-Windows dialogs, Windows Explorer file associations, double-click open, drag/drop, recent files, directory opens, package install/network/shell, workspace expansion to the parent directory, and client-side package JavaScript.
    - Linked the Phase 19 smoke contract from `docs/development/windows.md` and marked the native dialog, command routing, selected-file grant, and live selected-file Markdown path as known baseline gaps until implementation tasks complete.
    - Added `tests/manual_smoke_docs.rs` documentation guards for the Phase 19 scope and file-association exclusion.
    - `cargo fmt --check`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test manual_smoke_docs`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Review existing editor primitives and plan generic primitive gaps before implementation
  - Acceptance Criteria:
    - Functional: Existing primitives are inventoried before implementation: keybinding/configuration, behavior manifests, client command routing, IPC document open messages, server workspace validation, mode activation, parse handler registration, decoration transport, SDUI/status, and Markdown package adapters.
    - Performance: The review identifies which work is configuration-time, explicit UI-command time, server file-open time, background parse/decor time, or hot-path typing/paint work.
    - Code Quality: Any new Rust primitive is named generically, such as client UI command intent, selected-file open grant, parser adapter execution, or document-open activation, not Markdown-specific or Windows-specific beyond the dialog backend.
    - Security: The review records permission/authority boundaries for the file dialog, selected path, server validation, package parser execution, and inert client rendering.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/reference/primitives/parse-update-strategy.md`, and `docs/reference/primitives/rendering-strategy.md`.
      - `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/client-snapshot-bootstrap.md`, `docs/wiki/flows/client-server-edit-ack.md`, `docs/wiki/modules/server-file-workspace.md`, `docs/wiki/modules/behavior-runtime-registration.md`, `docs/wiki/modules/first-party-markdown-package.md`, `docs/wiki/modules/parse-coordinator.md`, and `docs/wiki/modules/decoration-transport.md`.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`: Primitive-first planning for mode/package work.
    - Options Considered:
      - Add a Markdown-only Rust open path: rejected because mode-specific Rust branches violate the package boundary.
      - Reuse only existing `OpenDocument { workspaceRootId, path }`: insufficient for user-selected files outside configured workspace roots.
      - Add reusable primitives for client UI command routing and explicit selected-file grants: selected.
    - Chosen Approach:
      - Produce/update a primitive review artifact before coding, then add only generic reusable primitives needed by this file-open smoke path.
    - API Notes and Examples:
      ```text
      Key binding -> client UI command intent -> Windows file dialog -> selected-file IPC request -> server single-file grant/open -> DocumentOpened event -> mode activation/decorations.
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase19-windows-file-open-primitive-review.md`: New primitive review artifact.
      - `docs/wiki/index.md`: Link the review.
      - `tests/primitives_docs.rs`: Deterministic coverage for the review, index link, and generic-only primitive guidance.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `phase19_file_open_primitive_review_records_existing_inventory`: Review artifact lists existing primitives and timing classifications.
    - `phase19_file_open_primitive_review_records_generic_gaps_only`: Review rejects Markdown-specific Rust parser/open branches and records reusable primitive gaps only.
  - Verification Completed:
    - Re-read the primitive reference docs, listed wiki pages, Phase 19 plan task, Clay plan requirements, and relevant project patterns before editing.
    - Added `docs/wiki/modules/phase19-windows-file-open-primitive-review.md` with an inventory of keybinding/configuration, behavior manifests, client command routing, IPC document open messages, server workspace validation, client snapshot replacement, mode activation, parse handler registration, decoration transport, SDUI/status, and Markdown package adapters.
    - Classified work as configuration-time, explicit UI-command time, server file-open time, document-open/background time, and hot-path typing/paint/text-event work.
    - Recorded only generic primitive gaps: `ClientUiCommandIntent`, `SelectedFileOpenRequest`, `SelectedFileGrant`, `DocumentOpenApplied`, `DocumentOpenActivation`, `ParserAdapterExecution`, and `ClientFileDialogBackend`.
    - Rejected Markdown-specific Rust parser/open branches and limited Windows-specific code to the dialog backend abstraction.
    - Linked the review from `docs/wiki/index.md` and added deterministic coverage in `tests/primitives_docs.rs` for the review, timing inventory, index link, and generic-only gap guidance.
    - `cargo fmt --check`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test primitives_docs`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Add a bindable client UI command for opening the file dialog
  - Acceptance Criteria:
    - Functional: `clay.documents.clientOpenFileDialog` can be bound through `init.js` using `bindKey`, routes through the inert behavior manifest, and reaches a client-side command handler without executing JavaScript on keypress.
    - Performance: Key routing is a local manifest lookup; the modal dialog starts only after the explicit command route and never from paint/layout/text-event handlers.
    - Code Quality: Client UI commands are represented as a small generic command route separate from built-in text edits and server-first commands; command names follow Clay JS API authority markers.
    - Security: The command grants only a user-mediated native file-selection prompt. It does not grant filesystem scanning, package enable/disable, shell, network, AI, WASM, raw-op, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/behavior-runtime-registration.md`: `bindKey` compiles config-time keybindings into inert behavior manifests.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Public keybinding API contract and custom properties.
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Use manifests for local routing, not executable behavior.
    - Options Considered:
      - Hard-code `Ctrl+O` in `EditorWidget`: rejected by the user requirement and configuration pattern.
      - Route the command as `ServerIntent`: insufficient because the native dialog is a client UI action before the server can open the document.
      - Add a generic `ClientUiIntent`/client-owned command route: selected.
    - Chosen Approach:
      - Add a documented command/API ID `clay.documents.clientOpenFileDialog`, allow `bindKey` to register it, and extend client behavior routing so configured keybindings produce a client UI intent handled by the native app.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs`: Added `RoutingPolicy::ClientUiCommand`, `CommandAuthority::ClientUi`, and `CommandDeclaration::client_ui`.
      - `src/behavior/manifest.rs`: Validates client UI command policy/authority pairs while keeping executable/side-effect combinations rejected.
      - `src/client/behavior.rs`: Routes configured client UI commands to `ClientUiCommandRoute`, including `clay.documents.clientOpenFileDialog` coverage and no-hard-coded-`Ctrl+O` coverage.
      - `src/editor/surface.rs` and `src/masonry_editor.rs`: Carry client UI command outcomes through `EditorKeyOutcome` and submit `EditorAction::ClientUiCommand` to the app driver without local text mutation.
      - `src/main.rs`: Added the native app client UI command handler stub for `clay.documents.clientOpenFileDialog`; dialog invocation remains in the next task.
      - `src/server/ops/keybindings.rs`, `src/server/ops/mod.rs`, and `src/server/ops/behavior.rs`: Added the command to the runtime-bindable allowlist, declaration construction, route listing, and authority names.
      - `src/server/js_runtime.rs`: Added runtime/configuration and keypress routing tests for the bindable client UI command.
      - `src/packages/commands.rs`, `src/server/ops/commands.rs`, and `tests/package_loading.rs`: Kept package command registration from granting native client UI authority and updated exhaustive routing-policy handling.
      - `docs/wiki/flows/client-behavior-routing.md` and `docs/wiki/modules/behavior-runtime-registration.md`: Updated implementation wiki coverage for the client UI route.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
  - Test Cases to Write:
    - `configuration_bind_ctrl_o_to_client_open_file_dialog`: `init.js` binding creates a validated behavior route for `Ctrl+O`.
    - `client_routes_open_file_dialog_as_client_ui_intent`: Client key routing returns a client UI intent, not a text edit or server-first route.
    - `open_file_dialog_binding_is_not_hard_coded`: Without config binding, `Ctrl+O` remains unhandled/default text behavior as appropriate.
    - `editor_routes_client_ui_command_without_local_mutation`: Editor surface carries the route without changing text or creating a server intent.
    - `keypress_routing_can_reach_client_ui_command_without_js`: Runtime-created manifest routes `Ctrl+O` locally as a client UI command.
  - Verification Completed:
    - Re-read the Phase 19 plan task, Clay plan requirements, relevant project patterns, behavior runtime wiki, keybinding API docs, client behavior routing wiki, and primitive-review artifact before editing.
    - Added a generic client UI command primitive with `RoutingPolicy::ClientUiCommand` and `CommandAuthority::ClientUi` rather than hard-coding `Ctrl+O` or treating the dialog command as server-first.
    - Allowed `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` to publish an inert validated behavior route from server-side configuration evaluation.
    - Routed the key locally through `ClientBehaviorState`, `EditorSurface`, `EditorWidget`, and `EditorAction::ClientUiCommand` so it reaches the native app command handler without executing JavaScript on keypress and without local text mutation.
    - Kept package command registration from granting native client UI authority and left actual dialog invocation for the next Windows backend task.
    - Updated relevant wiki pages for behavior runtime registration and client behavior routing.
    - `cargo fmt --check`: passed.
    - `cargo test js_runtime --quiet`: passed (42 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test open_file_dialog --quiet`: passed (3 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test editor_routes_client_ui_command_without_local_mutation --quiet`: passed (1 test), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test package_loading keypress_routing_uses_manifest_without_javascript --quiet`: passed (1 test), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo check --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Implement the Windows 11 native open-file dialog backend
  - Acceptance Criteria:
    - Functional: On Windows, the client UI command opens the OS file browser, filters for Markdown files (`.md`, `.markdown`, `.mdown`) plus an all-files fallback, returns a selected path, and reports cancellation as a non-error no-op.
    - Performance: The dialog is modal only for the explicit user command; it is not invoked during startup, typing, paint, scroll, layout, or background IPC handling.
    - Code Quality: Platform-specific dialog code is isolated behind `src/client/file_dialog.rs` or an equivalent small abstraction; non-Windows builds compile and return an unsupported diagnostic/status.
    - Security: The dialog backend does not execute shell commands, open network listeners, scan directories, read file contents, or grant authority beyond returning the user-selected path to the server-open flow.
  - Approach:
    - Documentation Reviewed:
      - Context7 CLI `npx ctx7@latest library "windows-rs" "Complete the task Implement the Windows 11 native open-file dialog backend from @plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md"` selected `/microsoft/windows-rs`.
      - Context7 CLI `MSYS_NO_PATHCONV=1 npx ctx7@latest docs /microsoft/windows-rs "Rust Windows file open dialog IFileOpenDialog COM API Windows 11 select file FileOpenDialog GetResult Show COM initialization"`: confirmed `windows` crate COM imports and that `windows` is the appropriate crate when COM support is needed.
      - Context7 CLI `MSYS_NO_PATHCONV=1 npx ctx7@latest docs /websites/microsoft_github_io_windows-docs-rs_doc_windows "IFileOpenDialog SetFileTypes COMDLG_FILTERSPEC GetDisplayName SIGDN_FILESYSPATH Rust windows crate"`: checked current docs-rs surface before implementation; local compile clarified exact `windows 0.62.2` module paths and unsafe method signatures.
      - Windows Shell COM API names used through `windows`: `CoInitializeEx`, `CoCreateInstance`, `IFileOpenDialog`, `FileOpenDialog`, `SetFileTypes`, `Show`, `GetResult`, `GetDisplayName`, and `SIGDN_FILESYSPATH`.
    - Options Considered:
      - Add `rfd`: considered, but Context7 lookup for `rfd` did not identify the Rust crate docs; this phase prefers direct Windows COM binding with `windows` for a Windows-only implementation.
      - Spawn Explorer or shell commands: rejected for security and control.
      - Use `windows` crate Shell COM APIs under `cfg(windows)`: selected.
    - Chosen Approach:
      - Add a Windows-only dialog backend using the `windows` crate with Shell/COM features, wrapped by a platform-neutral client API that returns `FileDialogResult::Selected(PathBuf)`, `Cancelled`, `Unsupported`, or `Failed`.
    - API Notes and Examples:
      ```rust
      #[cfg(windows)]
      let dialog: windows::Win32::UI::Shell::IFileOpenDialog = unsafe {
          windows::Win32::System::Com::CoCreateInstance(
              &windows::Win32::UI::Shell::FileOpenDialog,
              None,
              windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
          )?
      };
      dialog.SetFileTypes(&filters)?;
      dialog.Show(None)?;
      let item = dialog.GetResult()?;
      let path = item.GetDisplayName(windows::Win32::UI::Shell::SIGDN_FILESYSPATH)?;
      ```
    - Files to Create/Edit:
      - `Cargo.toml`: Added a Windows-targeted `windows` dependency with Foundation, COM, Shell, and Shell Common features.
      - `Cargo.lock`: Recorded the resolved `windows` crate family dependencies.
      - `src/client/file_dialog.rs`: Added the platform abstraction, Markdown/all-files filter model, Windows Shell COM implementation, cancellation mapping, non-Windows unsupported fallback, and unit tests.
      - `src/client/mod.rs`: Exported the file dialog abstraction.
      - `src/main.rs`: Invokes the dialog from the `clay.documents.clientOpenFileDialog` client UI command path and converts unsupported/failure results into runtime diagnostics/status.
      - `docs/development/launch-and-gui-smoke.md`: Updated the Phase 19 baseline to mark the bindable command and Windows dialog backend as implemented while preserving remaining selected-file/server/Markdown gaps.
      - `docs/development/windows.md`: Documented Windows-only support, Markdown filters, cancellation behavior, and remaining selected-file gaps.
      - `docs/wiki/modules/client-file-dialog.md`: Documented the implementation, authority boundary, and tests.
      - `docs/wiki/index.md`: Linked the client file dialog backend wiki page.
      - `tests/manual_smoke_docs.rs`: Updated documentation guards for the implemented backend and cancellation behavior.
    - References:
      - Context7 `/microsoft/windows-rs` docs fetched for COM support.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `docs/wiki/modules/phase19-windows-file-open-primitive-review.md`
      - `docs/wiki/flows/client-behavior-routing.md`
  - Test Cases to Write:
    - `windows_file_dialog_filter_allows_markdown_extensions`: Unit-test filter construction without opening a real dialog.
    - `non_windows_open_file_dialog_reports_unsupported`: Non-Windows path compiles and reports unsupported.
    - `non_windows_client_open_file_dialog_command_reports_status_diagnostic`: Non-Windows command handling reports an unsupported diagnostic/status.
    - Manual test: Press configured `Ctrl+O` on Windows 11 and verify the native file browser opens.
  - Verification Completed:
    - Re-read the Phase 19 task, authority/performance patterns, primitive review, client behavior routing wiki, behavior runtime wiki, and Windows development docs before editing.
    - Added `src/client/file_dialog.rs` with a fixed Markdown filter model (`*.md`, `*.markdown`, `*.mdown`) plus `*.*`, a `FileDialogResult` enum, a Windows `IFileOpenDialog` COM backend, and non-Windows unsupported fallback.
    - Added target-specific `windows = 0.62.2` dependency features for Foundation, COM, Shell, and Shell Common APIs.
    - Wired `clay.documents.clientOpenFileDialog` in `src/main.rs` to invoke the backend only from the explicit client UI command action; cancellation is a no-op, unsupported/failure results become runtime diagnostics/status, and selected-file server open remains intentionally deferred to the next plan task.
    - Updated Windows/manual smoke docs and the project wiki to document the implemented backend, Markdown filters, cancellation behavior, unsupported status, and selected-path authority boundary.
    - `cargo fmt --check`: passed.
    - `cargo test file_dialog --quiet`: passed (4 matching tests on Windows host), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test manual_smoke_docs --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo check --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `rustc -vV`: host `x86_64-pc-windows-msvc`, release `1.95.0`.
    - `cargo check --target x86_64-pc-windows-msvc --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Add explicit selected-file open IPC and server single-file grants
  - Acceptance Criteria:
    - Functional: After a file is selected, the client sends an explicit selected-file open request; the server canonicalizes the selected path, validates it is a regular UTF-8 text file, opens it as a Clay document, and returns a snapshot/metadata event.
    - Performance: Full text is sent only as the initial open snapshot. Subsequent edits remain delta-based through the existing bounded queue.
    - Code Quality: Selected-file authorization is modeled as a reusable server workspace primitive, such as a single-file grant, rather than broad parent-directory workspace expansion or Markdown-specific open logic.
    - Security: The server does not trust the client path blindly; it canonicalizes, rejects directories/special files/invalid UTF-8, sanitizes diagnostics, and grants at most the selected file for this phase.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/server-file-workspace.md`: Existing server-owned file/workspace model and validation.
      - `docs/wiki/flows/client-server-edit-ack.md`: Existing client background queue and event bridge.
      - `docs/wiki/modules/protocol-codec.md`: File/workspace protocol shape and codec boundaries.
    - Options Considered:
      - Reuse configured workspace roots only: rejected because arbitrary user-selected files may not be under a configured root.
      - Add the selected file's parent as a workspace root: rejected because it grants broader directory authority than the user selected.
      - Add a single-file grant/open primitive: selected.
    - Chosen Approach:
      - Extend protocol and workspace state with a selected-file open request. The server creates or reuses a file-backed document with a single-file grant root/record so subsequent document metadata remains compatible without exposing more host path authority.
    - API Notes and Examples:
      ```rust
      ClientMessage::OpenSelectedFile {
          client_id,
          selected_path,
      }
      // Server response: existing DocumentOpened + BehaviorManifest + later decorations/status.
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs` and `src/protocol/codec.rs`: Added and round-tripped `OpenSelectedFile` as a typed client IPC message.
      - `src/server/workspace.rs`: Added selected-file single-file grant authority, validation, duplicate-open reuse, and grant reauthorization.
      - `src/server/connection.rs`: Dispatches selected-file opens, returns `DocumentOpened` snapshots on success, sends the active behavior manifest after open, and returns typed `FileOperationFailed` failures.
      - `src/client/mod.rs`: Added non-edit outbound enqueue support for selected-file opens and client events for `DocumentOpened`/`FileOperationFailed`.
      - `src/masonry_editor.rs` and `src/main.rs`: Sends selected-file requests after dialog selection, applies opened-document snapshots, updates edit queue authority/version, and keeps subsequent edits delta-based.
      - `docs/development/launch-and-gui-smoke.md`, `docs/development/windows.md`, `docs/wiki/modules/client-file-dialog.md`, `docs/wiki/modules/server-file-workspace.md`, `docs/wiki/modules/protocol-codec.md`, and `docs/wiki/flows/client-server-edit-ack.md`: Updated docs/wiki for selected-file IPC, grants, and buffer replacement.
      - `tests/manual_smoke_docs.rs`: Updated documentation guard for the remaining live-Markdown gap.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases Written:
    - `selected_file_open_grants_only_the_selected_file`: Server opens the selected file without authorizing sibling files.
    - `selected_file_open_rejects_directory_and_invalid_utf8_without_document_entry` and `selected_file_open_rejects_special_file_without_document_entry`: Typed errors are returned for directories, special files, and invalid UTF-8 without document entries or selected-file grants.
    - `protocol_round_trips_open_save_reload_messages`: New `OpenSelectedFile` IPC message encodes/decodes safely with file/workspace commands.
    - `connection_open_selected_file_sends_snapshot_and_single_file_grant`: Connection dispatch returns a selected-file snapshot/manifest and rejects sibling opens through the single-file grant.
    - `selected_file_open_request_emits_non_edit_message`: Client queues selected-file open without creating an edit transaction.
    - `client_applies_document_opened_snapshot_from_selected_file`: Client state receives opened-document snapshots and resets synchronization versions.
    - `document_opened_event_replaces_editor_snapshot`: GUI/client state replaces the buffer and updates document status.
    - `opened_file_edits_continue_as_deltas`: After selected-file open, typing uses existing edit messages, not full-document IPC.
    - `client_receives_file_operation_failed_event`: Typed selected-file/file-operation failures become client events.
  - Verification Completed:
    - Re-read the Phase 19 task, Clay plan requirements, authority/performance patterns, and relevant wiki pages before editing.
    - Added `ClientMessage::OpenSelectedFile { client_id, selected_path }` and codec coverage while keeping ordinary edits as delta messages.
    - Added `WorkspaceAuthority::SingleFile` and `WorkspaceState::open_selected_file`, which canonicalizes the selected path, validates a regular UTF-8 file, rejects directories/special files/invalid UTF-8 before document registration, creates grants only after successful text loading, and reauthorizes saves/reloads against the exact canonical selected file rather than the parent directory.
    - Wired server connection dispatch to return `DocumentOpened` plus the current behavior manifest for selected-file opens and typed `FileOperationFailed` diagnostics for validation failures.
    - Wired client/app/editor handling so a successful dialog selection sends a bounded non-edit request, `DocumentOpened` replaces the GUI buffer, the edit queue updates lease/version authority, and subsequent typing emits existing `ClientMessage::Edit` deltas.
    - Updated development docs and implementation wiki pages for selected-file IPC, single-file grants, document-open application, diagnostics, and remaining live Markdown activation/decorations gap.
    - `cargo fmt --check`: passed.
    - `cargo test --test manual_smoke_docs --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test selected_file --quiet`: passed (5 matching tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test document_opened --quiet`: passed (2 matching tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test opened_file --quiet`: passed (1 matching test), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test workspace:: --lib --quiet`: passed (16 tests on Windows host; Unix-only selected special-file coverage is cfg-gated).
    - `cargo test protocol --quiet`: passed (26 unit tests plus matching integration filters), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test client --quiet`: passed (45 unit tests plus matching integration filters), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test connection --quiet`: passed (12 unit tests plus matching integration filters), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo check --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Activate Markdown mode and publish real decorations for opened `.md` files
  - Acceptance Criteria:
    - Functional: When a selected `.md`, `.markdown`, or `.mdown` file opens and `@clay/markdown` is loaded, Clay activates the Markdown major mode for the opened document, installs the package behavior manifest, runs the package parser/decorator adapter on a bounded initial viewport/window, and publishes inert decorations/status visible in the GUI.
    - Performance: Markdown parsing/decorations run as document-open/background work with viewport/window bounds; typing and paint use installed manifests/local decoration chunks and do not call JavaScript synchronously.
    - Code Quality: Parser execution uses a generic package parse-adapter contract and existing parse/decor primitives, not Markdown-specific Rust token parsing or client rendering branches.
    - Security: Package JavaScript runs server-side only through documented facades/validators; the client receives only decoded behavior manifests, decorations, and SDUI/status updates.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/first-party-markdown-package.md`: Current Markdown package contract, parser adapter, windowed behavior, and fixture limitation.
      - `docs/wiki/modules/parse-coordinator.md` and `docs/wiki/modules/decoration-transport.md`: Existing generic parse/decor primitives.
      - `docs/reference/packages/markdown.md`: First-party package permissions and runtime boundary.
    - Options Considered:
      - Rely on `markdown-mode` config fixture publication only: rejected because it does not parse arbitrary selected files.
      - Add Rust Markdown parser logic: rejected by the primitive-first/package-boundary rule.
      - Add a generic server-side package parse adapter execution path: selected.
    - Chosen Approach:
      - Reuse the loaded package's mode patterns and parser adapter metadata. On selected-file open, classify the path, select/activate the major mode, create a bounded parse-window request from the opened document, execute the package parser adapter server-side, validate spans through `serverPublishDecorations`/decoration primitives, and send updates through existing client events.
    - API Notes and Examples:
      ```text
      Open selected note.md -> classify markdown -> activate package major mode -> parse window [viewport + guard] -> publish DecorationSet -> client paints local inert spans.
      ```
    - Files to Create/Edit:
      - `src/server/connection.rs`: Added selected-file Markdown follow-up routing after `DocumentOpened`; when the Markdown package is already loaded, selected `.md`/`.markdown`/`.mdown` opens run first-party package activation, publish the opened document's behavior manifest, publish real parser-produced decorations, and publish document-bound Markdown status SDUI.
      - `src/server/js_runtime.rs`: Added a document-ID-aware configuration evaluation path and a deny-by-default `markdown-it` vendored ESM shim for the first-party Markdown parser adapter.
      - `src/server/ops/mod.rs`, `src/server/ops/sdui.rs`, and `src/server/sdui.rs`: Let runtime SDUI validation bind to the opened document ID instead of always document `1`, then update server SDUI state for the opened document.
      - `docs/development/launch-and-gui-smoke.md` and `docs/development/windows.md`: Updated the Phase 19 docs to mark live selected-file Markdown activation/decorations/status as implemented when `@clay/markdown` is loaded.
      - `docs/wiki/modules/first-party-markdown-package.md` and `docs/wiki/modules/embedded-js-runtime.md`: Documented selected-file Markdown activation, bounded parser input, vendored markdown-it runtime resolution, document-bound runtime SDUI, tests, and hot-path boundaries.
      - `tests/manual_smoke_docs.rs` and `src/server/connection.rs`: Added/updated deterministic coverage for documentation and selected-file Markdown activation/decorations/status.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
  - Test Cases Written:
    - `selected_markdown_file_publishes_manifest_decorations_and_status`: Opening a selected `.md` file while Markdown is loaded installs the Markdown manifest for the opened document, emits real markdown-it/package-adapter heading/list/inline-code spans, and sends Markdown status SDUI.
    - `markdown_open_runtime_uses_bounded_parse_window_for_large_file`: Large selected Markdown input evaluates only the initial 64 KiB UTF-8 parse window and publishes bounded decoration spans.
    - `phase19_manual_smoke_docs_define_open_dialog_scope`: Documentation now states selected-file Markdown activation/decorations/status are expected when `@clay/markdown` is loaded.
    - Existing `markdown_typing_does_not_wait_for_markdown_it_parse`: Continues to guard that slow parser work cannot delay local typing acknowledgement/application.
  - Verification Completed:
    - Re-read the Phase 19 task, mode primitive-first and package-distribution patterns, authority/performance patterns, first-party Markdown package wiki, parse coordinator wiki, decoration transport wiki, Markdown package reference docs, Markdown mode activation wiki, and server file workspace wiki before editing.
    - Added server-side selected-file Markdown follow-up messages after successful `OpenSelectedFile`: `DocumentOpened` remains the full snapshot boundary, then Markdown activation publishes a behavior manifest, `DecorationSet`, and document-bound `SduiSnapshot` only for supported Markdown extensions and only when the active manifest indicates `@clay/markdown` is loaded.
    - Kept Markdown tokenization in `packages/markdown/dist/parser.js` through markdown-it and `serverPublishDecorations`; Rust only prepares a bounded UTF-8 window, invokes the first-party package via documented facades, validates inert outputs, and forwards protocol messages.
    - Added a deny-by-default runtime resolution exception for the vendored `markdown-it` bundle and a document-ID-aware runtime/SDUI validation path so selected-file documents with IDs other than `1` can publish status UI safely.
    - Updated development docs and project wiki pages for selected-file Markdown activation/decorations/status, bounded parser input, server-side-only JavaScript execution, and runtime import/document-binding constraints.
    - `cargo test selected_markdown_file_publishes_manifest_decorations_and_status --lib --quiet`: passed.
    - `cargo test markdown_open_runtime_uses_bounded_parse_window_for_large_file --lib --quiet`: passed.
    - `cargo test markdown_package_runtime_loads_markdown_it_workflow --lib --quiet`: passed.
    - `cargo test --test markdown_mode --quiet`: passed (38 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test parse_coordinator --quiet`: passed (14 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test decoration_transport --quiet`: passed (11 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test manual_smoke_docs --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo fmt --check`: passed.
    - `cargo check --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Add a Windows Markdown open-dialog manual smoke fixture and verification path
  - Acceptance Criteria:
    - Functional: Documentation and optional development fixture show how to configure `Ctrl+O`, launch Clay, select a `.md` file, view Markdown decorations/status, type edits, and intentionally skip save.
    - Performance: The manual checklist tells testers to confirm typing remains responsive while decoration refresh may arrive asynchronously.
    - Code Quality: The smoke path uses normal configuration/keybinding/package APIs rather than test-only Rust branches.
    - Security: The fixture/checklist does not install packages, fetch network resources, execute shell commands, or broaden workspace authority beyond the selected file.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`: Existing command-first smoke validation and updated Phase 19 contract.
      - `docs/development/windows.md`: Windows-specific manual validation guidance.
      - `tests/fixtures/configuration/markdown-mode/init.js`: Current fixture that loads Markdown and publishes static representative decorations.
      - `docs/wiki/modules/configuration-runtime.md`: Configuration fixture/root constraints and server-side-only runtime behavior.
      - `docs/wiki/modules/first-party-markdown-package.md`: Markdown package fixture and selected-file activation behavior.
      - `.agents/skills/project-patterns/references/configuration-system.md` and `.agents/skills/project-patterns/references/maintenance-validation.md`.
    - Options Considered:
      - Require users to hand-edit `~/.config/clay/init.js` only: workable but less repeatable.
      - Add a repository fixture plus docs: selected for deterministic validation while keeping the binding configured through init.js.
    - Chosen Approach:
      - Added a development configuration fixture that loads the first-party Markdown package, registers Markdown mode/parser/decorations/status, and binds `Ctrl+O` to `clay.documents.clientOpenFileDialog` with `bindKey`. Manual docs point users to the fixture command and the same user-configurable binding snippet.
    - API Notes and Examples:
      ```bash
      cargo run -- smoke-gui --config-fixture windows-markdown-open
      # Press Ctrl+O, select a .md file, type edits, observe Markdown decorations/status.
      ```
    - Files Created/Edited:
      - `tests/fixtures/configuration/windows-markdown-open/init.js`: New dev fixture for first-party Markdown load/status plus `Ctrl+O` client open-dialog binding through `bindKey`.
      - `tests/fixtures/configuration/windows-markdown-open/workspace/sample.md`: Fixture workspace Markdown sample for deterministic runtime tests.
      - `docs/development/launch-and-gui-smoke.md`: Added the fixture command, manual Windows checklist, expected visible status/decorations, async decoration/typing note, and fixture security boundary.
      - `docs/development/windows.md`: Linked the fixture command and Windows-specific open/edit/skip-save instructions.
      - `tests/markdown_mode.rs`: Added static fixture/doc guards for `bindKey`, package load/mode registration, and absence of raw ops/callable dialog hooks in the fixture.
      - `src/server/js_runtime.rs`: Added runtime coverage that the fixture loads Markdown, publishes SDUI/decorations, and emits a client UI `Ctrl+O` behavior route.
      - `tests/manual_smoke_docs.rs`: Extended documentation guards for the fixture command and expected smoke panel text.
      - `docs/wiki/modules/first-party-markdown-package.md`: Documented the new Phase 19 fixture, authority boundary, and tests.
      - `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`: Updated task status and verification notes.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `docs/wiki/modules/configuration-runtime.md`
      - `docs/wiki/modules/first-party-markdown-package.md`
  - Test Cases Written:
    - `windows_markdown_open_fixture_binds_ctrl_o_without_hardcoding`: Fixture uses `bindKey`, not raw ops or callable dialog hooks, and docs include the fixture command.
    - `windows_markdown_open_fixture_loads_markdown_package`: Fixture validates first-party Markdown package load/mode/parser/decorations/status setup for later selected-file activation.
    - `server::js_runtime::tests::windows_markdown_open_config_fixture_loads_markdown_and_binds_ctrl_o`: Runtime loads the fixture with a workspace root, publishes Markdown SDUI/decorations, and installs the `Ctrl+O` `ClientUiCommand` route.
    - `phase19_manual_smoke_docs_define_open_dialog_scope`: Documentation includes the fixture command, visible panel marker, selected-file scope, and authority notes.
    - Manual Windows test documented: Run fixture, press `Ctrl+O`, select `.md`, confirm text loads and can be edited; interactive execution remains manual.
  - Verification Completed:
    - Re-read the Phase 19 task, Clay plan requirements, configuration and maintenance-validation patterns, existing Markdown fixture, launch smoke docs, Windows docs, configuration runtime wiki, and first-party Markdown package wiki before editing.
    - Added `tests/fixtures/configuration/windows-markdown-open/init.js` by reusing the normal first-party Markdown configuration/package APIs and adding `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` after Markdown command registration so the final behavior manifest preserves both Markdown package commands and the client UI route.
    - Added `tests/fixtures/configuration/windows-markdown-open/workspace/sample.md` for deterministic workspace-backed runtime validation.
    - Updated manual smoke docs so testers can run `cargo run -- smoke-gui --config-fixture windows-markdown-open`, confirm visible Markdown status/decorations, press `Ctrl+O`, select a Markdown file, type locally while decorations may refresh asynchronously, and intentionally skip save.
    - Preserved security boundaries: the fixture resolves only as a repository configuration fixture, uses server-side Clay JS facades, does not install packages, fetch network resources, execute shell commands, call raw ops, run client-side JavaScript, or broaden workspace authority beyond a server-validated selected file.
    - Updated project wiki coverage for the new fixture and its tests.
    - `cargo fmt --check`: passed.
    - `cargo test windows_markdown_open --quiet`: passed (1 matching lib test plus matching integration-test filters), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test manual_smoke_docs --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test markdown_mode windows_markdown_open --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `node --check tests/fixtures/configuration/windows-markdown-open/init.js`: passed.
    - `cargo check --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Verify launch, file-open, Markdown rendering, and editing behavior
  - Acceptance Criteria:
    - Functional: Automated checks cover command binding/routing, Windows dialog abstraction, selected-file server open, client snapshot replacement, Markdown activation/decorations, and edit-after-open behavior; manual checklist covers the real Windows file browser.
    - Performance: Existing hot-path tests still pass, and new tests prove selected-file open does not reintroduce full-document IPC for ordinary edits or parser work in typing/paint paths.
    - Code Quality: Tests are deterministic, platform-gated where necessary, and avoid requiring an interactive dialog except for the documented manual Windows smoke step.
    - Security: Tests cover path validation, single-file grant scope, unsupported OS behavior, cancellation/no-op behavior, and sanitized diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic checks with actionable failures.
      - Existing validation commands from `plans/011`, `plans/012`, `plans/020`, and `plans/021`.
    - Options Considered:
      - Rely on manual testing only: rejected.
      - Try to automate a real Windows COM dialog: rejected for CI/determinism; use unit tests plus manual GUI smoke.
      - Combine deterministic coverage with one documented interactive Windows check: selected.
    - Chosen Approach:
      - Add focused unit/integration tests for all non-interactive pieces, keep real dialog selection as a manual Windows smoke step, and run the standard cargo/doc-registry checks.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo test --test markdown_mode
      cargo test --test decoration_transport
      cargo test --test performance_protocol
      cargo test --test editor_performance_invariants
      cargo test --all-targets
      cargo check --target x86_64-pc-windows-msvc --all-targets
      ```
    - Files Created/Edited:
      - `src/main.rs`: Factored pure file-dialog-result conversion so cancellation/no-op, selected-file handoff, unsupported-platform diagnostics, and failure diagnostics can be tested without opening an interactive dialog.
      - `tests/markdown_mode.rs`, `tests/decoration_transport.rs`, `tests/performance_protocol.rs`, `tests/editor_performance_invariants.rs`, `tests/manual_smoke_docs.rs`, and relevant module tests: Verified deterministic coverage for Markdown activation/decorations, decoration transport, edit hot paths, and manual smoke docs.
      - `docs/development/launch-and-gui-smoke.md`: Verified the final manual checklist documents the real Windows file-browser step.
      - `docs/wiki/modules/client-file-dialog.md`: Updated test coverage notes for deterministic cancellation/result-conversion verification.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases Written/Verified:
    - `file_dialog_cancellation_is_a_no_op`: Dialog cancellation converts to no app action, status, or selected-file open request.
    - `file_dialog_result_conversion_reports_selected_and_sanitized_failures`: Selected paths, unsupported-platform diagnostics, and dialog failures convert deterministically without invoking the native dialog.
    - Existing focused coverage verified command binding/routing, no hard-coded `Ctrl+O`, selected-file IPC/grants, client snapshot replacement, Markdown activation/decorations/status, edit-after-open delta behavior, path validation, sanitized diagnostics, and hot-path parser/paint boundaries.
    - Full verification command set above passed.
    - Manual Windows smoke remains documented: configure/init fixture, run Clay, press `Ctrl+O`, select `.md`, confirm visible Markdown decorations/status and editable text. Interactive dialog execution is intentionally manual and not automated.
  - Verification Completed:
    - Re-read the Phase 19 verification task, Clay plan requirements, maintenance-validation and protocol/performance patterns, launch smoke docs, Windows docs, and relevant tests before editing.
    - Added deterministic main/app-driver tests for file-dialog cancellation as a no-op and selected/unsupported/failure result conversion so the real COM dialog remains the only manual step.
    - Verified docs already record `cargo run -- smoke-gui --config-fixture windows-markdown-open`, expected visible Markdown panel/status/decorations, selected-file buffer replacement, responsive local editing, and skip-save scope.
    - Updated the client file dialog wiki test list for deterministic cancellation/no-op and result-conversion coverage.
    - `cargo fmt --check`: passed after formatting.
    - `cargo test --test markdown_mode --quiet`: passed (40 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test decoration_transport --quiet`: passed (11 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test performance_protocol --quiet`: passed (7 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test editor_performance_invariants --quiet`: passed (7 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test manual_smoke_docs --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test open_file_dialog --quiet`: passed (4 matching lib tests plus 2 matching bin tests on Windows host), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test selected_file --quiet`: passed (5 matching tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test document_opened --quiet`, `cargo test opened_file --quiet`, and `cargo test windows_markdown_open --quiet`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --all-targets`: passed (including 310 lib tests, 25 bin tests, integration tests, and benchmark harness checks), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo check --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `rustc -vV`: host `x86_64-pc-windows-msvc`, release `1.95.0`.
    - `cargo check --target x86_64-pc-windows-msvc --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The new `Ctrl+O` behavior is configurable only through documented Clay JS APIs, with no hard-coded Rust shortcut; any dialog filters/defaults or Markdown-open fixture behavior are documented as fixed defaults or exposed through Clay JS configuration APIs.
    - Performance: Configuration evaluation remains server startup/load-time work and does not run on keypress/paint/scroll/text-event paths.
    - Code Quality: Configuration docs/inventory include custom properties for behavior-changing settings and tests fail for hidden config keys.
    - Security: Configuration cannot grant arbitrary filesystem authority, package installation/enablement, shell, network, AI, WASM, raw-op, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration-as-Clay-JS-API rule.
      - `docs/reference/clay-js-api/configuration.md` and `docs/reference/clay-js-api/keybindings/bind-key.md`.
    - Options Considered:
      - Hard-code `Ctrl+O`: rejected.
      - Add a new dialog settings API immediately: only if real user-facing filter/default settings are introduced.
      - Use existing `bindKey` with a documented command ID and fixed Windows Markdown filter defaults: selected unless implementation requires configurable filters.
    - Chosen Approach:
      - Verify `bindKey` is sufficient for the key binding and document the dialog command/default filter behavior. Promote new configuration APIs only if implementation introduces real user-tunable settings.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";
      bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files Created/Edited:
      - `docs/reference/clay-js-api/configuration.md`: Added the Phase 19 configuration review, confirming `bindKey` is the configuration API, `Ctrl+O` is not hard-coded in Rust, dialog filters/defaults are fixed defaults, and no dialog-settings API was promoted.
      - `docs/reference/clay-js-api/keybindings/bind-key.md`: Added a `clay.documents.clientOpenFileDialog` binding example and authority notes for fixed dialog defaults and selected-file server validation.
      - `tests/clay_js_api_inventory.rs`: Added Phase 19 configuration coverage for init.js binding, no hard-coded `Ctrl+O` in `EditorWidget`, no broad filesystem authority, no hidden dialog keys, and bindKey custom-property metadata.
      - `docs/reference/clay-js-api/api-inventory.toml` and `docs/generated/clay-js-api-registry.json`: Verified unchanged because no new user-tunable configuration API or registry-frontmatter change was promoted in this task.
      - `tests/clay_js_doc_registry.rs` and `tests/markdown_mode.rs`: Existing registry and fixture tests were re-run for coverage.
    - References:
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases Written:
    - `open_file_dialog_keybinding_is_configured_through_init_js`: Binding exists through the Windows Markdown config fixture and `bindKey` docs, routes as `ClientUiCommand`, and `EditorWidget` has no hard-coded `Ctrl+O` branch.
    - `open_file_dialog_configuration_does_not_grant_broad_filesystem_authority`: Configuration and keybinding docs deny broad external authority, record selected-file-only validation/granting, and the fixture avoids raw ops, hidden dialog keys, and callable client hooks.
    - `configuration_docs_cover_open_file_dialog_defaults_or_options`: Configuration docs record fixed Windows Markdown/all-files defaults rather than hidden `init.js` keys, and the bindKey inventory keeps behavior-changing `key`, `command`, `scope`, and `when` custom properties.
  - Verification Completed:
    - Re-read the Phase 19 task, Clay plan requirements, configuration-system and doc-registry patterns, configuration decision log, configuration reference docs, bindKey docs, configuration runtime wiki, behavior runtime wiki, and client file dialog wiki before editing.
    - Verified the implementation did not introduce real user-tunable dialog settings; `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` remains the only Phase 19 configuration surface, and Markdown file dialog filters/defaults remain documented fixed defaults.
    - Added the Phase 19 configuration review and `bindKey` example/authority notes while keeping `api-inventory.toml` and generated registry artifacts unchanged because no new public configuration API/frontmatter was promoted.
    - Added deterministic inventory tests for init.js binding, no hard-coded Rust shortcut, fixed-default documentation, bindKey custom-property coverage, selected-file-only authority, and hidden-dialog-key rejection.
    - `cargo fmt --check`: passed after formatting.
    - `cargo test --test clay_js_api_inventory open_file_dialog --quiet`: passed (3 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test clay_js_api_inventory --quiet`: passed (22 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test clay_js_doc_registry --quiet`: passed (23 tests), with existing `src/masonry_sdui.rs` dead-code warnings. An earlier attempt to pass two test filters to one `cargo test` command failed due cargo CLI usage, then the full registry test was re-run successfully.
    - `cargo test --test markdown_mode windows_markdown_open --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Every public programmatic surface introduced or changed by this plan is exposed through stable Clay JS/TS facades and Markdown docs, or kept private/`pub(crate)` when internal.
    - Performance: Public docs state hot-path policy: binding/routing is local, dialog/open work is explicit user command work, parse/decorations are background/viewport-bounded, and ordinary editing remains delta-based.
    - Code Quality: API docs include stable ID, JS module/export, facade path, op/backing Rust path where applicable, user-facing name, key bindings or empty list, custom properties or empty list, examples, options, errors, permissions, authority notes, lookup tags, and app/help/agent visibility.
    - Security: Permission/authority notes cover native client UI, user-selected file grants, server validation, sanitized diagnostics, no raw ops, and no broad filesystem/workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API verification task.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`, `clay-js-api-boundary.md`, `clay-js-api-naming.md`, and `doc-registry-tests.md`.
      - `docs/reference/clay-js-api/configuration.md`, `docs/reference/clay-js-api/keybindings/bind-key.md`, `docs/reference/clay-js-api/documents/server-open-document.md`, `docs/index.md`, and existing Clay JS API inventory/registry tests.
    - Options Considered:
      - Treat `clientOpenFileDialog` as an undocumented internal command: rejected because it is user-bindable from `init.js`.
      - Add a direct JS callable that opens the native dialog: rejected because server-side Clay JavaScript must not execute client UI or gain filesystem authority.
      - Document it as a public Clay JS command-ID facade while keeping internal selected-file grant helpers private: selected.
    - Chosen Approach:
      - Added `clay.documents.clientOpenFileDialog` as a documented public Clay JS command-ID API. The `clientOpenFileDialog()` facade returns the stable command ID for `bindKey`; it does not open the dialog directly.
      - Updated the workspace `serverOpenDocument` docs to distinguish configured-workspace opens from the private selected-file IPC/single-file grant path used by the native dialog.
      - Kept Windows COM helpers, selected-file IPC response helpers, `WorkspaceAuthority::SingleFile`, `WorkspaceState::open_selected_file`, parser execution helpers, and cache state internal/private or `pub(crate)` unless already mapped as implementation ownership.
    - API Notes and Examples:
      ```ts
      import { clientOpenFileDialog } from "clay:documents";
      import { bindKey } from "clay:keybindings";

      bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
      // Equivalent: bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
      ```
    - Files Created/Edited:
      - `docs/reference/clay-js-api/documents/client-open-file-dialog.md`: New public API/command-ID doc with stable ID, module/export, facade path, binding op/backing Rust paths, examples, options/defaults, errors, hot-path policy, key bindings/custom properties, lookup tags, app/help visibility, and security notes.
      - `docs/reference/clay-js-api/documents/server-open-document.md`: Documented that `serverOpenDocument` remains workspace-root based while native dialog opens use `clientOpenFileDialog` plus private selected-file IPC/single-file grants.
      - `runtime/js/documents.ts`: Added `ClientOpenFileDialogCommandId` and `clientOpenFileDialog()` facade export.
      - `docs/reference/clay-js-api/api-inventory.toml`: Added the public registry inventory entry for `clay.documents.clientOpenFileDialog`.
      - `docs/index.md`: Linked the new API doc in the authoritative registry source section.
      - `docs/generated/clay-js-api-registry.json`: Regenerated from Markdown with `cargo run --bin update-doc-registry`.
      - `tests/clay_js_api_inventory.rs`: Added docs/index/facade/security coverage for the new command-ID API.
      - `tests/clay_js_doc_registry.rs`: Added generated-registry and lookup coverage for `clientOpenFileDialog`.
      - `tests/clay_js_facade_layout.rs`: Added the new `clay:documents` facade export to the facade layout guard.
      - `tests/rust_visibility_api_mapping.rs`: Added targeted Rust visibility coverage to keep dialog helpers and selected-file grant internals private/`pub(crate)` or inventory-mapped.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
  - Test Cases Written:
    - `client_open_file_dialog_api_is_documented_indexed_and_facade_backed`: Docs, index, inventory, and facade export exist and document fixed defaults/hot-path policy.
    - `generated_registry_contains_client_open_file_dialog_command_api`: Generated registry exposes the stable ID, module/export, lookup tags, empty key bindings/custom properties, and security metadata.
    - `open_dialog_internal_helpers_are_private_or_inventory_mapped`: Rust visibility test covers the public command/Rust boundary and private Windows COM/selected-file helper boundaries.
    - `open_dialog_api_security_notes_cover_selected_file_authority`: Docs/inventory mention Windows/native UI authority, selected-file validation/grants, sanitized diagnostics, raw-op denial, and no broad filesystem/workspace authority.
    - Existing registry/facade tests verify docs/index/inventory consistency, generated registry freshness, lookup availability, naming, facade exports, and public/private Rust API mapping.
  - Verification Completed:
    - Re-read the Phase 19 task, Clay plan requirements, documentation-as-code, Clay JS API boundary/naming, doc-registry patterns, configuration/bindKey/open-document docs, docs index, runtime facade, and relevant inventory/registry/visibility tests before editing.
    - Added `clientOpenFileDialog()` as a pure command-ID facade in `runtime/js/documents.ts` so public programmatic use can import the stable ID without causing server-side JavaScript to open native client UI.
    - Added authoritative public API docs and registry inventory for `clay.documents.clientOpenFileDialog`; documented no default key binding, no custom properties, fixed Windows Markdown/all-files defaults, explicit user command work, background/viewport-bounded parse/decorations, delta-based editing, unsupported/cancel/failure behavior, and selected-file-only server validation/granting.
    - Regenerated `docs/generated/clay-js-api-registry.json` with `cargo run --bin update-doc-registry`.
    - Updated `serverOpenDocument` docs so arbitrary host selected-file opens are not presented as workspace-root API usage.
    - Verified internal helper boundaries: Windows COM helper functions and main command dispatch remain private, `WorkspaceAuthority` remains private, `WorkspaceState::open_selected_file` remains `pub(crate)`, and connection selected-file response helper remains private.
    - `cargo fmt --check`: passed.
    - `cargo test --test clay_js_api_inventory client_open_file_dialog --quiet`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test clay_js_doc_registry client_open_file_dialog --quiet`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test clay_js_facade_layout --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test rust_visibility_api_mapping open_dialog_internal_helpers --quiet`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test clay_js_api_inventory --quiet`: passed (24 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test clay_js_doc_registry --quiet`: passed (24 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test rust_visibility_api_mapping --quiet`: passed (5 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo check --all-targets`: passed, with existing `src/masonry_sdui.rs` dead-code warnings.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic checks for wiki/docs coverage where practical.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md` and `.agents/skills/project-patterns/references/authority-boundaries.md`: Keep selected-file IPC, hot-path, and authority-boundary documentation explicit.
      - `docs/wiki/index.md`, `docs/wiki/modules/client-file-dialog.md`, `docs/wiki/modules/server-file-workspace.md`, `docs/wiki/flows/client-server-edit-ack.md`, `docs/wiki/modules/first-party-markdown-package.md`, `docs/wiki/modules/protocol-codec.md`, `docs/wiki/modules/embedded-js-runtime.md`, `docs/wiki/modules/behavior-runtime-registration.md`, and `docs/wiki/flows/client-behavior-routing.md`.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: selected; keeps docs aligned with final code.
    - Chosen Approach:
      - Verified that implementation wiki coverage had been updated during prior completed tasks, then made one final wiki pass to link the new public command-ID API from the internal client dialog page and add deterministic wiki coverage for Phase 19 implementation pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/client-file-dialog.md
      docs/wiki/modules/server-file-workspace.md
      docs/wiki/flows/client-server-edit-ack.md
      docs/wiki/modules/first-party-markdown-package.md
      docs/reference/clay-js-api/documents/client-open-file-dialog.md
      ```
    - Files Created/Edited:
      - `docs/wiki/index.md`: Verified Phase 19 implementation pages are linked and updated the client dialog summary to mention the public command-ID facade boundary.
      - `docs/wiki/modules/client-file-dialog.md`: Verified native dialog/client command routing coverage and added the authoritative Clay JS API reference/facade source links.
      - `docs/wiki/modules/server-file-workspace.md`: Verified selected-file single-file grant behavior, validation, and sibling-path denial are documented.
      - `docs/wiki/flows/client-server-edit-ack.md`: Verified selected-file non-edit request, `DocumentOpened` snapshot replacement, edit-queue reset, and delta-after-open behavior are documented.
      - `docs/wiki/modules/first-party-markdown-package.md`: Verified selected-file Markdown activation, bounded parser input, document-bound status/decorations, and hot-path boundaries are documented.
      - `tests/manual_smoke_docs.rs`: Added deterministic wiki coverage for Phase 19 index links and key implementation/security/performance markers.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases Written/Verified:
    - Manual wiki review: Confirmed the master index links relevant pages and updated pages explain what changed implementation does and how it works.
    - `phase19_code_wiki_documents_open_dialog_path`: Deterministic coverage that the wiki index links Phase 19 implementation pages and that client dialog, workspace, edit-ack, and Markdown package pages document public API links, selected-file grants, non-edit IPC, bounded Markdown activation, and hot-path boundaries.
    - Existing `phase19_file_open_primitive_review_records_existing_inventory` and `phase19_file_open_primitive_review_records_generic_gaps_only`: Re-run to verify the primitive review remains linked and generic.
  - Verification Completed:
    - Re-read the final wiki task, project wiki workflow, maintenance-validation/protocol/performance/authority patterns, and relevant Phase 19 wiki pages before editing.
    - Verified `docs/wiki/modules/client-file-dialog.md`, `docs/wiki/modules/server-file-workspace.md`, `docs/wiki/flows/client-server-edit-ack.md`, `docs/wiki/modules/first-party-markdown-package.md`, `docs/wiki/modules/protocol-codec.md`, `docs/wiki/modules/embedded-js-runtime.md`, `docs/wiki/modules/behavior-runtime-registration.md`, and `docs/wiki/flows/client-behavior-routing.md` document the final implementation state.
    - Added explicit links from the client dialog wiki page to `runtime/js/documents.ts` and `docs/reference/clay-js-api/documents/client-open-file-dialog.md` so public API usage remains in authoritative reference docs while the wiki explains internals.
    - Added deterministic documentation coverage for Phase 19 wiki navigation and implementation markers in `tests/manual_smoke_docs.rs`.
    - `cargo fmt --check`: passed.
    - `cargo test --test manual_smoke_docs --quiet`: passed (3 tests), with existing `src/masonry_sdui.rs` dead-code warnings.
    - `cargo test --test primitives_docs phase19 --quiet`: passed (2 tests), with existing `src/masonry_sdui.rs` dead-code warnings.

## Compromises Made
- The real Windows file browser interaction remains a documented manual smoke step rather than an automated test, preserving deterministic CI/test behavior and avoiding scripted interaction with a modal OS dialog.
- Non-Windows native dialog support remains out of scope for Phase 19; non-Windows builds return an unsupported diagnostic/status instead of opening a native picker.
- Save-after-open remains out of scope for this phase; selected files can be edited in memory through existing optimistic delta behavior, but persistence is deferred.

## Further Actions
- Priority High: Run the documented Windows 11 manual smoke (`cargo run -- smoke-gui --config-fixture windows-markdown-open`, press configured `Ctrl+O`, select a `.md` file, confirm Markdown status/decorations and responsive edit-only behavior) before treating the interactive OS-dialog experience as release-validated.
- Priority Medium: Plan a later save/conflict workflow for selected-file single-file grants if Phase 20+ requires persistence back to the user-selected file.
- Priority Medium: Consider platform-native file dialogs for macOS/Linux in a separate phase with the same client UI command and selected-file grant primitives.
