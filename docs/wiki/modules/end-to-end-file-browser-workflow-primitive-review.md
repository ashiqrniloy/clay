# End-to-End File Browser Workflow Primitive Review

## Source

- `plans/043-End-to-End-File-Browser-Folder-Navigation-Language-Open-and-Clipboard.md`
- `docs/wiki/modules/workspace-file-browser.md`
- `docs/wiki/modules/server-file-workspace.md`
- `docs/wiki/modules/client-file-dialog.md`
- `docs/wiki/modules/server-ipc-skeleton.md`
- `docs/wiki/modules/first-party-language-packages.md`
- `docs/wiki/modules/masonry-editor.md`
- `src/server/workspace.rs`
- `src/shell/file_browser.rs`
- `src/server/command_execution.rs`
- `src/server/connection.rs`
- `src/client/file_dialog.rs`
- `src/editor/surface.rs`
- `src/editor/buffer.rs`
- `src/masonry_editor.rs`
- `tests/primitives_docs.rs`

## Overview

This review locks the primitive-first gate for the six-step product workflow: open Clay, see a file browser, choose a system folder, navigate folders/files, open Rust/TypeScript/JavaScript files, and copy selected snippets to the OS clipboard.

Most of the workflow already maps to generic Clay primitives. The remaining gaps are generic and reusable: selected-folder client UI grants, file-browser directory navigation state, generic open-document follow-up activation for workspace/file-browser opens, and client-owned copy-selection clipboard write. No workflow step requires language-specific Rust branches, a package-owned file browser, raw path opens, shell commands, or server/package clipboard authority.

## Existing Primitive Inventory

### Workspace roots and bounded listing

- `WorkspaceState` owns workspace roots, canonicalization, root deduplication, selected-file single-file grants, `add_explicit_user_grant`, `discover_root_for_path`, and `list_directory`.
- `FileListRequest` / `FileListPage` already provide a bounded server file tree/list service with max-depth, max-entry, child-count, ignore, cancellation, and diagnostic constraints.
- `serverListWorkspaceRoots`, `serverAddWorkspaceRoot`, `serverListDirectory`, `serverCreateListingCancelToken`, and `serverCancelListing` are the existing Clay JS workspace facades.

### File browser SDUI and command execution

- `FileBrowserState::from_workspace` and `FileBrowserState::to_sdui_tree` build a Clay-owned left Workspace panel from server listing data and project it through inert SDUI primitives.
- `CommandExecutor::execute_workspace` validates built-in workspace command IDs and routes file opens through `WorkspaceState::open_existing_file` or selected-file grants.
- Existing commands include `clay.workspace.openFile`, `clay.workspace.openFuzzyFile`, `clay.workspace.revealInTree`, and `clay.workspace.toggleFileBrowser`.
- `TransientMenuSession` already backs bounded fuzzy-open style interactions from installed metadata.

### Client UI prompts and selected-file authority

- `clay.documents.clientOpenFileDialog` is a bindable `ClientUiCommand` route. The native client owns the modal prompt; the server owns validation and the selected-file grant.
- `FileOpenCapabilityPool` issues single-use tokens for selected-file opens, and `ClientMessage::OpenSelectedFile` is rejected without a valid token.
- Historical note: the Phase 19 Windows Markdown-file-only backend initially returned `Unsupported` on non-Windows; Phase 20 added Linux portal and macOS `NSOpenPanel` file-open backends while keeping selected-path grant consumption unchanged.

### Language activation and package behavior

- `classify_open_document`, behavior manifests, `schedule_open_parse`, the parse coordinator, syntax grammar registry, and first-party language packages already provide generic open-time activation and decoration work.
- `@clay/rust`, `@clay/typescript`, and `@clay/javascript` register modes, grammars, behavior, completions, and status using generic package primitives.
- `core.text` and `core.code` fallback modes keep UTF-8 files editable when packages are not loaded.

### Editor selection state

- `SelectionState` stores anchor/focus offsets and exposes normalized ranges.
- `EditorSurface` owns the local editor buffer and selection, and `EditorSurface::selected_text()` uses `EditorBuffer::text_range` to extract UTF-8-safe text ranges from the `crop::Rope`.
- The native client owns input, selection, and OS clipboard writes through the small `src/client/clipboard.rs` `arboard` wrapper. Clipboard support is write-only and limited to explicit user copy of the current selection.

## Generic Workflow Primitive Gaps

### Selected-folder client UI grant

Add a generic selected-folder flow, not a file-browser-only shortcut:

- A bindable `ClientUiCommand` such as `clay.workspace.clientOpenFolderDialog` asks the native client to prompt the user for a directory.
- The server receives a selected directory only with a single-use selected-path capability, canonicalizes it, verifies it is a directory, deduplicates it through `WorkspaceState::add_root` / `add_explicit_user_grant`, and returns refreshed Workspace SDUI.
- The Linux path should use a native portal/DBus API or equivalent native backend; it must not shell out to `zenity`, `kdialog`, `xdg-open`, or scripts.

### File-browser directory navigation

Add generic directory navigation state on top of existing bounded listing:

- Directory rows must route to a directory-navigation command, not `clay.workspace.openFile`.
- Navigation accepts only `{ workspaceRootId, relativePath }`, must reuse `WorkspaceState::list_directory`, and emits a refreshed bounded SDUI tree.
- A single-current-directory model with a parent row is enough; a full expand/collapse tree is deferred until needed.

### Generic open-document follow-ups

Promote selected-file-only follow-ups into a generic open-document helper:

- `OpenDocument`, file-browser `openFile`, fuzzy-open, and selected-file opens should all reuse the same classification, behavior-manifest, parse/decor, and diagnostic follow-up path.
- The helper must not branch on Rust, TypeScript, JavaScript, Markdown, file-browser origin, or package name.

### Client copy-selection clipboard write

The minimal client-owned clipboard primitive is implemented:

- `EditorSurface::selected_text()` extracts the selected text through normalized byte ranges and `EditorBuffer::text_range`.
- `EditorWidget` handles native copy shortcuts (`Ctrl+C` on Linux/Windows, `Cmd+C` on macOS) and writes that text to the OS clipboard on explicit user action.
- No paste, cut, clipboard read, server clipboard op, package clipboard op, or arbitrary clipboard write is part of this workflow.

## Hot-Path Classification

| Work | Classification | Allowed path |
| --- | --- | --- |
| Folder picker | Explicit client UI command | Modal native prompt after a user command only |
| Selected-folder grant | Explicit server authorization | Capability consume, canonicalize, directory check, add root |
| Directory listing/navigation | Explicit server command | `WorkspaceState::list_directory` with existing bounds |
| File opening | Explicit server command/open request | `open_existing_file` or selected-file grant validation |
| Language activation | Open-time/background work | `classify_open_document`, behavior manifest, bounded parse/decor publication |
| Clipboard copy | Explicit client command | Extract selected range and write OS clipboard |
| Editor typing/paint/layout | Client hot path | No filesystem scans, native dialogs, IPC waits, JavaScript, full-document serialization, shell, network, AI, or clipboard work |

## Rejected Implementation Shapes

- Do not add `FileBrowserWidget`, `FolderPickerWidget`, `CopyService`, language-specific open paths, or `if extension == "rs"` / `if package == "@clay/rust"` Rust branches.
- Do not implement client-side workspace scans or file listing; the server owns roots, paths, listings, and file-open validation.
- Do not let packages add workspace roots, marker files, ignore rules, listing scopes, native folder dialogs, or clipboard writes.
- Do not pass raw client-chosen paths directly to document open or root add APIs without a server-issued single-use capability and canonical validation.
- Do not shell out for native folder picking.
- Do not run package JavaScript, parser work, filesystem listing, modal UI, or clipboard work from Masonry paint/layout/key text-event hot paths.
- Do not add paste/cut, save-as, rename/delete, file watchers, recursive live tree updates, or full expand/collapse state for this minimum workflow unless a later task explicitly requires them.

## Security and Authority Boundary

This workflow introduces no broad client or package filesystem authority.

- Server owns workspace roots, canonical path validation, bounded directory listing, file opens, document text/version authority, language/package runtime execution, and selected-path capability validation.
- Client owns native prompts, rendering/input, selection state, and OS clipboard writes from explicit user actions.
- Selected folder/file paths are untrusted strings until server capability, canonicalization, type, root-limit, traversal, UTF-8, and size checks pass.
- Packages cannot list arbitrary paths, add roots, invoke native folder UI, write/read clipboard content, execute shell/network/AI/WASM/raw ops, or broaden workspace authority.
- Clipboard support is write-only for the current editor selection; server and packages get no clipboard-read or arbitrary clipboard-write capability.

## Planned Documentation and Test Coverage

- This page records the existing primitive inventory, generic workflow gaps, hot-path classification, rejected shapes, and authority boundary.
- `docs/wiki/index.md` links this review.
- `docs/wiki/modules/primitive-architecture.md` records the workflow primitive gate.
- `tests/primitives_docs.rs` verifies this page, links, required inventory/gap text, hot-path split, rejected shapes, and security boundary.

## Related

- [Workspace Discovery and File Browser](workspace-file-browser.md)
- [Server File Workspace Model](server-file-workspace.md)
- [Client File Dialog Backend](client-file-dialog.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [First-Party Rust, TypeScript, and JavaScript Language Packages](first-party-language-packages.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
