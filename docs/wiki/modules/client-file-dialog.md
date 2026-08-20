# Client File Dialog Backend

## Source

- `src/client/file_dialog.rs`
- `src/client/mod.rs`
- `src/app_driver.rs`
- `src/server/js_runtime/mod.rs`
- `runtime/js/documents.js`
- `runtime/js/workspace.js`
- `docs/reference/clay-js-api/documents/client-open-file-dialog.md`
- `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`
- `docs/development/launch-and-gui-smoke.md`
- `docs/development/windows.md`

## Overview

The client file dialog backend is the native UI half of Phase 19's file-open smoke path. A configured inert behavior route can produce `documents.clientOpenFileDialog`; the native app driver handles that client UI command by calling `open_markdown_file_dialog()`. The authoritative public programmatic surface is the `clientOpenFileDialog()` command-ID facade documented in `docs/reference/clay-js-api/documents/client-open-file-dialog.md`; this wiki page explains the internal native dialog implementation behind that stable ID. `runtime/js/documents.js` is the executable source included by `src/server/facades.rs`; `documents.d.ts` supplies its declaration contract.

The backend intentionally does only native user-mediated path picking: ask the user to pick a file or folder and return the selected path, cancellation, unsupported platform status, or a dialog error. It does not read file contents, scan directories, execute shell commands, open network listeners, or broaden server workspace authority. On Linux, `Driver` owns independent `file_dialog_in_flight` and `folder_dialog_in_flight` generations, so one portal file picker and one portal folder picker may coexist but repeated commands of the same kind are ignored while their picker is open. When a file path is selected, the app enqueues `ClientMessage::OpenSelectedFile`; the server canonicalizes and validates the path before creating a selected-file single-file grant and document snapshot. When a folder path is selected through `workspace.clientOpenFolderDialog`, the app enqueues `ClientMessage::AddSelectedWorkspaceRoot`; the server consumes the same single-use selected-path capability, canonicalizes the directory, records it as a workspace root, and sends a refreshed file-browser `SduiSnapshot`.

Phase 24.3 adds a built-in alternative that does **not** use this backend: the [Path Browser](path-browser.md) (`controlCenter.openPath`) issues its own user-authorized browse activation as the capability event (no single-use token), converting ephemeral browse authority into exactly one `SingleFile` or `Directory` grant. The native dialogs remain the fallback capability issuers and are behaviorally unchanged — path mode never disables them.

## How It Works

`src/client/file_dialog.rs` exposes a platform-neutral API:

- `markdown_file_dialog_filters()` returns the fixed filter model used by tests and the Windows backend: Markdown files (`*.md`, `*.markdown`, `*.mdown`) plus an all-files fallback (`*.*`).
- `open_markdown_file_dialog()` returns `FileDialogResult::Selected(PathBuf)`, `Cancelled`, `Unsupported`, or `Failed`.

On Windows, the backend uses the `windows` crate and Shell COM APIs:

1. Initialize an apartment with `CoInitializeEx(COINIT_APARTMENTTHREADED)` for the explicit UI command.
2. Create `IFileOpenDialog` with `CoCreateInstance(FileOpenDialog, CLSCTX_INPROC_SERVER)`.
3. Install `COMDLG_FILTERSPEC` entries for Markdown and all files.
4. Set file-system-only, existing-file, existing-path, and no-current-directory-change flags.
5. Show the modal dialog and map `ERROR_CANCELLED` to `Cancelled`.
6. Convert `GetResult().GetDisplayName(SIGDN_FILESYSPATH)` to `PathBuf` and free the COM-allocated string.

On Linux, both `open_markdown_file_dialog()` and `open_folder_dialog()` use `xdg-desktop-portal` over the D-Bus session bus through `zbus` (`org.freedesktop.portal.FileChooser.OpenFile`). File open passes Markdown/all-files filters as portal `filters` (`a(sa(us))`, with `*.*` normalized to `*`); folder open sets `directory=true`. The driver runs these portal calls on a background thread (Linux only) and re-enters the GUI via `EventLoopProxy` so the Wayland event loop keeps pumping (blocking OpenFile on the UI thread prevents the chooser from presenting). Before spawning, it reserves the matching generation; a duplicate cannot create another thread or portal request. The worker always sends `FileDialogCompleted` or `FolderDialogCompleted`, including cancellation and failure. The GUI clears and applies only the matching generation, so a late result cannot clear or act as a newer request. Thread-spawn failure clears immediately; event-send failure means the event loop is already shutting down and the `Driver` state is dropped. Explicit exit/window close clears both generations. Requests subscribe to a predicted `handle_token` Request path before `OpenFile` to avoid the Response race, and response code `2` surfaces as `Failed` rather than silent cancel. Returned `file://` URIs are converted to paths. On macOS, both dialogs use `NSOpenPanel` (Markdown extension tokens plus `allowsOtherFileTypes` for the all-files fallback; folder mode chooses directories only). On platforms other than Windows/Linux/macOS, both dialogs return `Unsupported` so the app can report a status diagnostic without panicking.

## Invariants and Constraints

- Dialog invocation happens only from an explicit `ClientUiCommand` app-driver action, never during startup, typing, paint, scroll, layout, text events, background IPC reads, or JavaScript evaluation.
- Platform backends are isolated behind `#[cfg(windows)]`, `#[cfg(target_os = "linux")]`, and `#[cfg(target_os = "macos")]` seams in `src/client/file_dialog.rs`.
- Cancellation is a non-error no-op and still releases the matching in-flight generation.
- Linux file and folder portal requests are limited independently to one in flight; duplicate commands are ignored without adding threads or portal requests.
- Generation equality is checked before selected paths or diagnostics are applied; stale completions have no authority.
- Unsupported platforms report a diagnostic/status through the app command handler.
- A selected path is not an authorization grant by itself; the server validates it through `WorkspaceState::open_selected_file` before granting only that canonical file.
- Every `unsafe` block in the Windows COM path (`CoCreateInstance`, `GetOptions`/`SetOptions`, `Show`, `GetResult`, `GetDisplayName`, `PWSTR::to_string`, `CoTaskMemFree`, `SetFileTypes`, `CoInitializeEx`, `CoUninitialize`) carries a `// SAFETY:` comment stating the invariant that makes it safe (apartment affinity, COM-allocated string ownership + matching deallocator, filter-table borrow outlives the call, init/uninit pairing via `ApartmentCom::Drop`). The `file_dialog_unsafe_blocks_have_safety_comments` regression test scans the source and fails if any `unsafe` block lacks a preceding `// SAFETY:` comment.

## Tests

- `src/client/file_dialog.rs::tests::windows_file_dialog_filter_allows_markdown_extensions`
- `src/client/file_dialog.rs::tests::portal_glob_normalizes_all_files_sentinel`
- `src/client/file_dialog.rs::tests::macos_extensions_ignore_all_files_sentinel_and_keep_markdown_tokens`
- `src/client/file_dialog.rs::tests::linux_portal_filters_use_file_chooser_signature` on Linux
- `src/client/file_dialog.rs::tests::unsupported_platform_open_file_dialog_reports_unsupported` on non-Windows/Linux/macOS targets
- `src/main.rs::tests::file_dialog_cancellation_is_a_no_op`
- `src/main.rs::tests::file_dialog_result_conversion_reports_selected_and_sanitized_failures`
- `src/main.rs::tests::native_dialog_generations_limit_duplicates_and_reject_stale_results`
- `src/main.rs::tests::unsupported_platform_client_open_file_dialog_command_reports_status_diagnostic` on non-Windows/Linux/macOS targets
- `src/server/js_runtime/mod.rs::tests::configuration_binds_client_ui_file_folder_and_copy_commands` imports the `clientOpenFileDialog()` and `clientOpenFolderDialog()` sources included by `src/server/facades.rs`; `tests/clay_js_facade_layout.rs` prevents source/declaration/include drift
- `src/client/mod.rs::tests::selected_file_open_request_emits_non_edit_message`
- `src/client/mod.rs::tests::selected_folder_root_request_emits_non_edit_message`
- `src/server/connection/mod.rs::tests::connection_add_selected_workspace_root_sends_file_browser_snapshot`
- `src/server/connection/mod.rs::tests::connection_add_selected_workspace_root_rejects_stale_capability`
- Commands used during implementation/verification:
  - `cargo test file_dialog --quiet`
  - `cargo test --test protocol manual_smoke_docs:: --quiet`
  - `cargo test --all-targets`
  - `cargo check --all-targets`
  - `cargo check --all-targets`
- `cargo clippy --all-targets -- -D warnings`

## Related

- [Client Behavior Routing](../flows/client-behavior-routing.md)
- [Path Browser](path-browser.md) — the built-in browse alternative; this backend stays the fallback capability issuer
- [Phase 19 Windows File Open Primitive Review](phase19-windows-file-open-primitive-review.md)
- [Server File Workspace Model](server-file-workspace.md)
- [Client Open File Dialog Clay JS API](../../reference/clay-js-api/documents/client-open-file-dialog.md)
- [Client Open Folder Dialog Clay JS API](../../reference/clay-js-api/workspace/client-open-folder-dialog.md)
- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)
