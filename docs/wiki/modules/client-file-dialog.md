# Client File Dialog Backend

> **Historical.** The neutral `src/client/file_dialog.rs` module (Windows COM
> `IFileOpenDialog`, Linux `zbus` portal, macOS `NSOpenPanel` backends) was
> superseded by the Tauri desktop bridge in Plan 097 (Phase 9 adapter, Phase 12
> cutover) and deleted in 2026-08-31 as unreferenced dead code. The only dialog
> implementation today is the narrow Tauri-side portal command in
> `src-tauri/src/commands.rs` feeding the same server grant paths:
> [React Command Centre](react-command-centre-desktop-workflows.md).

## Source

- `src-tauri/src/commands.rs` (dialog implementation)
- `frontend/src/bridge/client.ts`, `frontend/src/shell/workspace-controller.ts` (client dispatch)
- `src/client/behavior.rs` (client UI command routing contract)
- `src/client_commands.rs`
- `src/protocol/mod.rs` (`ClientMessage::OpenSelectedFile`, `AddSelectedWorkspaceRoot`)
- `src/server/workspace/mod.rs` (`WorkspaceState::open_selected_file`)
- `src/server/connection/workspace.rs`
- `runtime/js/documents.js`, `runtime/js/workspace.js`
- `docs/reference/clay-js-api/documents/client-open-file-dialog.md`
- `docs/reference/clay-js-api/workspace/client-open-folder-dialog.md`
- `docs/development/launch-and-gui-smoke.md`
- `docs/development/windows.md`

## Overview

The file dialog path is the user-mediated half of Phase 19's file-open smoke
path. A configured inert behavior route can produce
`documents.clientOpenFileDialog`; the desktop client (React) receives the
client UI command through the behavior manifest and invokes the Tauri command
`dialog_open_file` (see [React Command Centre](react-command-centre-desktop-workflows.md)).
The authoritative public programmatic surface is the `clientOpenFileDialog()`
command-ID facade documented in
`docs/reference/clay-js-api/documents/client-open-file-dialog.md`; this wiki
page explains the desktop implementation behind that stable ID.
`runtime/js/documents.js` is the executable source included by
`src/server/facades.rs`; `documents.d.ts` supplies its declaration contract.

The backend intentionally does only native user-mediated path picking: ask the
user to pick a file or folder and return the selected path, cancellation, or a
dialog error. It does not read file contents, scan directories, execute shell
commands, open network listeners, or broaden server workspace authority.
`src/server/ui.rs` (`CLIENT_DIALOG_ACTIONS`) validates these command IDs as
registered client actions. On Linux the Tauri command `pick_path` uses
`ashpd` (XDG `org.freedesktop.portal.FileChooser.OpenFile`); per-dialog busy
locks let one file picker and one folder picker coexist while duplicate
same-kind commands return the bridge `busy` error. When a file path is
selected, `BridgeState::accept_selected_path` feeds the path into
`ClientEditQueue::enqueue_open_selected_file`; the server canonicalizes and
validates the path before creating a selected-file single-file grant and
document snapshot. When a folder path is selected through
`workspace.clientOpenFolderDialog`, the same accept path enqueues
`ClientMessage::AddSelectedWorkspaceRoot`; the server consumes the single-use
selected-path capability, canonicalizes the directory, records it as a
workspace root, and sends a refreshed file-browser `SduiSnapshot`.

Phase 24.3 adds a built-in alternative that does **not** use this backend: the
[Path Browser](path-browser.md) (`controlCenter.openPath`) issues its own
user-authorized browse activation as the capability event (no single-use
token), converting ephemeral browse authority into exactly one `SingleFile` or
`Directory` grant.

## How It Works

The stable surfaces are:

- `runtime/js/documents.js::clientOpenFileDialog` returns the command ID
  `documents.clientOpenFileDialog`; `runtime/js/workspace.js::clientOpenFolderDialog`
  returns `workspace.clientOpenFolderDialog`. Both are declaration-pinned by
  adjacent `.d.ts` files and included by `src/server/facades.rs`.
- The desktop bridge `src-tauri/src/commands.rs` implements `pick_path` on
  `ashpd`'s XDG file-chooser portal (`OpenFileRequest`): Markdown filters
  (`*.md`, `*.markdown`, `*.mdown`) plus an all-files fallback for files, and
  `directory=true` for folders. Cancelled responses are mapped to "no
  selection" (not an error); other portal errors surface as the bridge's
  sanitized `file dialog failed` diagnostic.
- `dialog_open_file` and `dialog_open_folder` hold independent per-dialog
  tokio locks, so one file picker and one folder picker may coexist while
  duplicate same-kind requests are rejected as busy instead of queuing a
  second picker.
- A selected `file://` URI is converted to a `PathBuf` and passed to
  `BridgeState::accept_selected_path`, never to React. Selections go directly
  to the server-selected-file / selected-folder grant flow, exactly like the
  pre-cutover native dialog path did.
- `tab_open_dialog` is the new-tab variant: it returns the picked folder as a
  `BootstrapDto` through `BridgeState::open_tab`.

Platform status: Linux is implemented through `xdg-desktop-portal`. Windows
and macOS native pickers are long-term platform targets and are not
implemented by the current desktop bridge; they will be added at the same
Tauri command seam without changing the server contract. (The deleted
`src/client/file_dialog.rs` COM/`NSOpenPanel` backends belonged to the removed
native client and were never wired into this bridge.)

## Invariants and Constraints

- Dialog invocation happens only from an explicit `ClientUiCommand` command dispatch, never during startup, typing, paint, scroll, layout, text events, background IPC reads, or JavaScript evaluation.
- Cancellation is a non-error no-op and releases the matching per-dialog lock.
- File and folder dialogs are limited independently to one in flight; duplicate commands receive the typed bridge `busy` error without spawning extra portals.
- A selected path is not an authorization grant by itself; the server validates it through `WorkspaceState::open_selected_file` before granting only that canonical file.
- The main webview retains `core:default` only: no Tauri filesystem, shell, process, network, dialog, or clipboard plugin permission is exposed to package or frontend JavaScript.
- The bridge never returns selected paths to React for document/workspace opens.
- Phase 24.3 adds a built-in alternative that does **not** use a dialog token: the [Path Browser](path-browser.md) (`controlCenter.openPath`) issues its own user-authorized browse activation as the capability event (no single-use token), converting ephemeral browse authority into exactly one `SingleFile` or `Directory` grant. The native dialogs remain an alternative capability issuer — path mode never disables them.

## Tests

- `tests/suites/../manual_smoke_docs.rs::phase19_code_wiki_documents_open_dialog_path` pins this page's markers against the Phase 19 smoke contract.
- `src/server/js_runtime/mod.rs::tests::configuration_binds_client_ui_file_folder_and_copy_commands` imports the `clientOpenFileDialog()` and `clientOpenFolderDialog()` sources included by `src/server/facades.rs`; `tests/clay_js_facade_layout.rs` prevents source/declaration/include drift.
- `src/client/mod.rs::tests::selected_file_open_request_emits_non_edit_message`
- `src/client/mod.rs::tests::selected_folder_root_request_emits_non_edit_message`
- `src/server/connection/mod.rs::tests::connection_add_selected_workspace_root_sends_file_browser_snapshot`
- `src/server/connection/mod.rs::tests::connection_add_selected_workspace_root_rejects_stale_capability`
- `src-tauri` bridge tests cover `accept_selected_path` grant consumption (see `src-tauri/src/bridge/`).

## Related

- [Client Behavior Routing](../flows/client-behavior-routing.md)
- [Path Browser](path-browser.md) — the built-in browse alternative
- [Phase 19 Windows File Open Primitive Review](phase19-windows-file-open-primitive-review.md)
- [Server File Workspace Model](server-file-workspace.md)
- [Client Open File Dialog Clay JS API](../../reference/clay-js-api/documents/client-open-file-dialog.md)
- [Client Open Folder Dialog Clay JS API](../../reference/clay-js-api/workspace/client-open-folder-dialog.md)
- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)