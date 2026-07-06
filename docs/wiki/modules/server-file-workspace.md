# Server File Workspace Model

## Source

- `src/server/workspace.rs`
- `src/server/document.rs`

## Overview

Phase 9 introduces a server-owned workspace/open-document model alongside the existing server-canonical `DocumentState`. Phase 19 extends the same model with selected-file single-file grants for native file-open dialogs. Phase 18.12 adds server-owned workspace-root discovery and bounded directory listing for the Clay-owned file browser. The model records authorized workspace roots, selected-file grants, canonical file paths, open document identity, duplicate-open behavior, file-backed dirty state, bounded file-list snapshots, and server-side path authorization without giving the native client or packages filesystem authority.

## Responsibilities

- `WorkspaceState` owns workspace roots, selected-file single-file grants, canonical path-to-document mapping, document ID allocation, path validation, file type checks, and per-document `Arc<Mutex<DocumentState>>` handles.
- `DocumentState` still owns canonical rope text, versions, edit validation, leases, region locks, and now the dirty flag for accepted edits.
- `ServerConfig::workspace_roots` records server startup workspace roots; `IpcServer::try_new` validates them into `WorkspaceState` before protocol dispatch integration.
- `WorkspaceState::open_existing_file` performs server-side UTF-8 file loading through Tokio async file IO and registers loaded files in the open-document registry.
- `WorkspaceState::open_selected_file` canonicalizes an explicit user-selected path, rejects directories/special files/invalid UTF-8, creates a single-file grant only after successful UTF-8 loading, and registers the opened document without authorizing sibling paths.
- `WorkspaceState::save_document` writes the current canonical document text to the authorized file path, checks stale on-disk metadata before overwriting, and marks the document clean only after a successful save.
- `WorkspaceState::reload_document` refreshes clean documents from disk, rejects dirty reloads unless forced, updates canonical text/version state, and keeps reload authorization server-side.
- `WorkspaceState::document_metadata`, `list_documents`, `open_document_snapshots`, `list_root_metadata`, `document_handle`, and `release_client_access` provide the minimal connection-dispatch, reload-refresh, and runtime-op surface for protocol open/save/reload/status/list messages and Clay JS document/workspace facade calls without exposing canonical host paths to clients.
- Phase 18.12 workspace discovery uses `add_root_from_cwd`, `discover_root_for_path`, and `add_explicit_user_grant` to add canonical roots from startup cwd/configured roots, opened-file ancestry with the closed `KNOWN_PROJECT_MARKERS` set (`.git`, `Cargo.toml`, `package.json`), and explicit user directory grants. File grants remain single-file grants and do not become listed workspace roots.
- Phase 18.12 bounded listing uses `list_directory(FileListRequest)` to produce a `FileListPage` with entry kind, relative path, size hint, child count, truncation/cancellation flags, and diagnostics. Listing enforces root containment, depth/count/child-scan ceilings, compiled ignore names, simple root `.gitignore` patterns, and cooperative cancellation tokens.
- `WorkspaceError::diagnostic` centralizes typed file/workspace diagnostics for protocol failures, server startup validation, logs, and future UI surfaces. Diagnostics distinguish missing roots, inaccessible container mounts, permission denied, outside-root paths, directories, special files, UTF-8 failures, dirty reload conflicts, stale-save conflicts, and oversized files.
- `MAX_OPENABLE_FILE_BYTES` (defined in `src/perf/budgets.rs`) is a hard size gate: `open_existing_file`, `open_selected_file`, and `reload_document` reject files whose observed filesystem size exceeds the budget with a typed `WorkspaceError::FileTooLarge` (`FileErrorCode::FileTooLarge`) *before* `tokio::fs::read` allocates the full contents, so an oversized file cannot be used as a memory-exhaustion vector and cannot pass the open gate only to fail at frame encode. The budget sits below the 1 MiB IPC codec frame limit (`DEFAULT_MAX_FRAME_SIZE`) so any file that opens also fits in a single full-text `InitialDocument`/`ResyncSnapshot`/`DocumentOpened`/`DocumentReloaded` frame.
- Disk-bearing workspace ops release the workspace mutex during the heavy `tokio::fs` read/write: each op is split into a `prepare_*` phase (mutex held; fast metadata + authority + registry lookup + size gate only), a free `*_io` phase (no workspace mutex), and a `commit_*` phase (mutex reacquired). The IpcServer and Clay JS ops call `open_existing_file_unlocked`, `open_selected_file_unlocked`, `save_document_unlocked`, and `reload_document_unlocked` so concurrent operations on unrelated documents are not serialized by a slow disk call; commits re-validate registry state on reacquire (concurrent-open dedup, reload dirty re-check, save tolerates a closed document and detects concurrent edits via `mark_clean_if_version`).
- Document saves are atomic: `save_document`/`save_io` write through `atomic_write_file`, which rejects Unix targets with no owner/group/other write bits, writes a unique temp file in the target's directory, `fsync`s it, restores the original file's permissions (Unix mode), and `rename`s the temp over the target (atomic in-place replace on POSIX and Windows). A crash/power loss or a write/rename failure leaves the original file intact — only the temp is ever partial, and a failed rename removes the orphaned temp. Temp names are unique per process (`.<name>.clay-save-<pid>-<counter>`) so concurrent saves of the same canonical path do not collide.

## How It Works

1. A workspace root is added with `WorkspaceState::add_root`, which canonicalizes the root and requires it to be a directory.
2. `add_root_from_cwd` installs the current working directory when no roots were configured. `discover_root_for_path` scans opened-file ancestors up to a fixed depth for the closed marker table and adds the discovered project root, while `add_explicit_user_grant` adds user-selected directories or single-file grants. Root IDs are deduplicated by canonical path and capped by `MAX_WORKSPACE_ROOTS`.
3. `list_directory` canonicalizes the requested directory under an authorized root, walks entries only up to the request/compiled depth and count bounds, skips compiled ignore names plus simple root `.gitignore` patterns, and returns partial diagnostics instead of broadening authority. Listing is server work, not paint/layout/input work.
4. `open_existing_file` and `register_loaded_file` canonicalize a requested relative or absolute path after joining relative paths to the authorized root. Canonicalization resolves `..` segments and symlinks before authorization.
5. The canonical file must still start with the canonical root. Escaping traversal and symlinks return `WorkspaceError::OutsideRoot` before a document entry exists.
6. The canonical path must be a regular file. Directories return `WorkspaceError::DirectoryOpen`; sockets and other non-ordinary file types return `WorkspaceError::UnsupportedFileType`.
7. Valid paths build `FileDocumentState` metadata with the root/grant ID, canonical path, display path, and last-known file metadata for stale-save checks.
8. If the canonical path is already open, the registry returns the existing document ID and document handle without re-reading disk. The existing `DocumentState::acquire_access` lease rules decide whether the caller receives editable or read-only access.
9. If the path is not open, `open_existing_file` first checks the observed file size against `MAX_OPENABLE_FILE_BYTES` (using the metadata returned by canonicalization) and rejects an oversized file with `WorkspaceError::FileTooLarge` (`FileErrorCode::FileTooLarge`) *before* reading; it then reads the file with `tokio::fs::read`, rejects invalid UTF-8 as `WorkspaceError::InvalidUtf8`, and only then registers a clean version-1 `DocumentState`. `open_selected_file` and `reload_document` apply the same pre-read size gate (reload checks the metadata returned by reauthorization). The budget sits below the 1 MiB IPC frame limit so an openable file always fits one full-text frame; pre-read rejection prevents large-file memory exhaustion.
10. `open_selected_file` performs the same regular-file/UTF-8 validation for an explicit selected absolute path, but creates a `WorkspaceAuthority::SingleFile` grant whose reauthorization accepts only that canonical file. Single-file grants are not returned by `list_root_metadata`, so they do not masquerade as broad configured workspace roots.
11. `register_loaded_file` keeps the test/protocol-ready path for callers that have already obtained trusted UTF-8 text after the same canonical path validation.
12. Accepted edits in `DocumentState::apply_edit` increment the document version and mark the document dirty.
13. `save_document` reauthorizes the canonical file path, compares current file metadata with the last-known metadata, rejects stale external changes with `WorkspaceError::StaleFileMetadata`, and writes `DocumentState::text()` to disk **atomically**: the `atomic_write_file` helper rejects Unix targets whose mode has no write bits, writes a unique temp file in the target's directory (`.\.<name>.clay-save-<pid>-<n>`), `fsync`s it for durability, restores the original file's permissions on Unix, and `rename`s the temp over the target. The rename is atomic on POSIX (overwrites in place) and on Windows uses Rust's `rename` (`MoveFileExW(MOVEFILE_REPLACE_EXISTING)`), so a crash or power loss during save leaves the target either fully old or fully new, never a torn write; a write/rename failure removes the temp and leaves the original intact. `save_document` then updates last-known metadata and clears dirty state only if no newer in-memory edit changed the document version during the save.
14. `reload_document` rejects dirty documents with `WorkspaceError::DirtyDocument` unless `force` is true. It reauthorizes the file path, reads UTF-8 text from disk, replaces the canonical rope through `DocumentState::replace_text_from_storage`, increments the document version when disk text differs, marks the document clean, and updates last-known metadata.
15. Connection dispatch maps workspace operations to protocol messages: open/reload return full snapshots, selected-file open returns the same `DocumentOpened` snapshot shape, save/status/list return metadata only, runtime reload uses `open_document_snapshots` internally to rerun generic activation without sending full-text snapshot messages, and `WorkspaceError::diagnostic` maps failures to stable `FileErrorCode` values plus user-facing messages and container/toolbox/distrobox hints.
14. Plan 030 (code-review remediation) authority hardening gates `OpenSelectedFile` behind a server-issued single-use capability token so the server, not the client, authorizes single-file opens. After the Hello handshake the server sends `ServerMessage::FileOpenCapabilityIssued { token }` once and re-issues one pending token after every `OpenSelectedFile` attempt. The connection owns a `FileOpenCapabilityPool` (`HashSet` of valid tokens); `OpenSelectedFile { capability, selected_path }` is rejected with a `RuntimeDiagnostic` (`clay.client.selected_file_open.unauthorized`) when the capability is missing, empty, stale, or already consumed, and no file grant or document is created. The client (`ClientEditQueue`) stores the latest issued token and attaches it to each picker open; if no token is pending it sends an empty capability, the server rejects and re-issues, and the user can retry. This is a structural authority gate, not a hard boundary against a malicious same-user client that can also complete Hello and receive a token — full defense requires the long-term OS-verifiable picker exchange. Workspace-root `OpenDocument` opens are unaffected and remain constrained to registered roots.
15. Phase 13 runtime ops in `src/server/ops/documents.rs` and `src/server/ops/workspace.rs` reuse the same `WorkspaceState` helpers for `clay:documents` and `clay:workspace` configuration calls. They serialize facade results as JSON, expose IDs as strings for the Clay JS API, and convert workspace diagnostics into JavaScript errors without exposing raw op names as the public API.
16. Startup root validation in `IpcServer::try_new` uses the same diagnostics, so a missing or inaccessible configured root reports that the path is not visible to the Clay server process and suggests mounting or choosing a root inside the server environment.
17. Plan 030 (code-review remediation) lock-release I/O: `open_existing_file`, `open_selected_file`, `save_document`, and `reload_document` are each split into a `prepare_*` phase (workspace mutex held; fast filesystem metadata + authority reauthorization + registry lookup + openable-size gate only), a free `*_io` phase (`tokio::fs` read/write with **no** workspace mutex held), and a `commit_*` phase (workspace mutex reacquired to mutate the registry). The IpcServer connection handlers and Clay JS document ops call the `*_unlocked` orchestration free functions (`open_existing_file_unlocked`, `open_selected_file_unlocked`, `save_document_unlocked`, `reload_document_unlocked`) so concurrent operations on unrelated documents are no longer serialized by a slow disk call. The `&mut self` one-shot methods remain as thin wrappers used by tests and direct callers that hold no outer mutex. Because the mutex is released across disk I/O, every `commit_*` re-validates registry state on reacquire instead of assuming it is unchanged: `register_canonical_file`/`register_selected_file` re-check the canonical path and return the existing lease if a concurrent open won (no duplicate document entry, no orphan `SingleFile` root); `commit_reload` re-checks dirtiness and refuses to clobber an unsaved edit unless `force`; `commit_save` tolerates a document closed during the write (the bytes are already on disk) and detects a concurrent edit through `mark_clean_if_version(prepared_version)`, leaving the document dirty rather than falsely clean. Lock ordering: the workspace mutex and the per-document `Arc<Mutex<DocumentState>>` are never held simultaneously across a `tokio::fs` await — the per-document lock is acquired only briefly inside `prepare`/`commit`/`*_io` to read or mutate text/version, then dropped before any cross-mutex await, so no new deadlock paths are introduced.

## Code Examples

```rust
let mut workspace = WorkspaceState::new();
let root_id = workspace.add_root("/workspace/project")?;
let opened = workspace
    .open_existing_file(root_id, "src/main.rs", client_id)
    .await?;
workspace.save_document(opened.document_id).await?;
let status = workspace.document_metadata(opened.document_id, client_id).await?;
let reloaded = workspace.reload_document(opened.document_id, false).await?;
```

## Invariants and Constraints

- The server is the only component that owns workspace roots, canonical paths, file-backed document handles, and dirty state.
- Duplicate opens are keyed by canonical path and reuse one `DocumentState`, preserving one editable lease with read-only observers.
- Ordinary edit application mutates only per-document state and does not perform file IO, workspace scans, JavaScript execution, AI work, or full-document IPC.
- Open-document snapshot enumeration is reload-only server work; it feeds bounded generic reactivation/parse refresh and does not authorize new paths.
- File paths outside registered workspace roots or outside a selected-file single-file grant are rejected before a file-backed document entry is created.
- `OpenSelectedFile` requires a server-issued single-use capability token; raw client-supplied paths without a valid token are rejected with `clay.client.selected_file_open.unauthorized` and create no grant or document. This is a structural gate; a malicious same-user client that completes Hello can still mint a token, so full defense needs the long-term OS-verifiable picker exchange.
- Symlinks are authorized by their canonical target, not their link location, so an in-root symlink to an outside file is denied and an in-root symlink to an in-root file maps to the target's canonical relative path.
- Directory, special-file, read, and UTF-8 validation failures happen only at open/register boundaries, never during ordinary edit application or client painting/input.
- Invalid UTF-8 files do not create or poison registry entries; selected-file grants are not created until UTF-8 validation succeeds, and a later valid open can still use the same canonical path.
- Save and reload reauthorize the stored canonical path before file IO so deleted/replaced/symlinked paths cannot bypass workspace-root authorization.
- Dirty reloads are rejected unless explicitly forced, preventing silent loss of accepted in-memory edits.
- Stale on-disk metadata is a save conflict, not an implicit overwrite; the in-memory document stays dirty so the caller can decide whether to reload, force a later operation, or surface a conflict.
- Workspace protocol and runtime-op errors are typed for programmatic handling and sanitize outside-root failures to avoid disclosing unauthorized host paths.
- Absolute paths from failed unauthorized requests are rendered as `<requested path>` in diagnostics unless they are server-authorized workspace roots. This keeps host path discovery out of client-visible messages while preserving actionable relative workspace paths.
- Container/toolbox/distrobox diagnostics are passive mappings of known IO failures. They do not run shell probes, scan mounts, access the network, or expand workspace authority.

## Tests

- `src/server/workspace.rs`: `duplicate_open_reuses_document_and_preserves_lease_policy` verifies duplicate canonical registrations share the document ID and lease policy.
- `src/server/workspace.rs`: `open_existing_file_loads_utf8_text` verifies server-side file loading creates a clean version-1 document snapshot.
- `src/server/workspace.rs`: `duplicate_open_reuses_loaded_document_and_lease_policy` verifies duplicate opens reuse the existing in-memory document without re-reading changed disk contents.
- `src/server/workspace.rs`: `open_invalid_utf8_reports_file_io_error_without_document_entry` verifies invalid UTF-8 is reported and leaves registry indexes empty.
- `src/server/workspace.rs`: `selected_file_open_grants_only_the_selected_file` verifies an explicit selected-file open creates a single-file grant that rejects sibling paths.
- `src/server/workspace.rs`: `selected_file_open_rejects_directory_and_invalid_utf8_without_document_entry` and `selected_file_open_rejects_special_file_without_document_entry` verify selected directories, special files, and invalid UTF-8 files do not create document entries or grants.
- `src/server/workspace.rs`: `workspace_rejects_path_traversal_outside_root` verifies `..` traversal cannot authorize a sibling file outside the root.
- `src/server/workspace.rs`: `workspace_rejects_directory_and_special_file_open` verifies directories and Unix socket files are rejected as document opens.
- `src/server/workspace.rs`: `workspace_canonicalizes_symlink_before_authorization` verifies escaping symlinks are denied and in-root symlinks canonicalize consistently.
- `src/server/workspace.rs`: `file_backed_document_dirty_state_tracks_accepted_edits_and_clean_marking` verifies loaded files start clean, accepted edits mark dirty, and clean marking is explicit.
- `src/server/workspace.rs`: `accepted_edit_marks_file_document_dirty_and_save_marks_clean` verifies accepted edits dirty file-backed documents and successful saves clear dirty state.
- `src/server/workspace.rs`: `save_writes_canonical_rope_text_to_disk` verifies saves persist the server canonical rope text, including UTF-8 text.
- `src/server/workspace.rs`: `reload_dirty_document_requires_force_or_rejects` verifies dirty reloads are rejected unless forced and forced reloads replace canonical text.
- `src/server/workspace.rs`: `reload_clean_document_refreshes_disk_text_and_marks_clean` verifies clean reloads refresh from disk and stay clean.
- `src/server/workspace.rs`: `save_missing_file_returns_typed_error_and_keeps_dirty` verifies missing files produce typed errors without clearing dirty state.
- `src/server/workspace.rs`: `save_stale_metadata_returns_typed_error_and_keeps_dirty` verifies external on-disk changes are stale-save conflicts and preserve unsaved edits.
- `src/server/workspace.rs`: `workspace_diagnostic_for_missing_root_is_actionable` verifies missing root diagnostics include a stable code and container/toolbox/distrobox hint.
- `src/server/workspace.rs`: `workspace_diagnostic_sanitizes_unauthorized_paths` verifies outside-root diagnostics avoid leaking the unauthorized path.
- `src/server/workspace.rs`: `workspace_permission_denied_keeps_document_dirty` verifies permission-denied saves report a stable diagnostic and preserve dirty in-memory state.
- `src/server/mod.rs`: `server_accepts_configured_workspace_roots_and_reports_invalid_roots` verifies startup root configuration is validated and invalid roots produce a typed server error.
- `src/server/connection.rs`: `connection_open_document_sends_snapshot_and_manifest_without_full_document_on_edit_ack` verifies open dispatch returns the initial file snapshot and manifest while later edit acknowledgements remain metadata-only.
- `src/server/connection.rs`: `file_io_errors_are_typed_protocol_failures` verifies workspace IO failures map to stable protocol error codes.
- `src/server/js_runtime.rs`: `document_facade_open_status_list_round_trip`, `workspace_roots_facade_reports_authorized_roots`, and `document_facade_rejects_unauthorized_paths` verify the runtime-backed `clay:documents`/`clay:workspace` subset reuses server workspace validation.
- Phase 18.12 workspace discovery/listing tests in `src/server/workspace.rs`: root deduplication, cwd fallback, marker ancestry discovery, no-marker fallback, explicit directory/file grants, grant deduplication, unknown marker rejection, bounded listing, max-depth/max-entry truncation, default and root `.gitignore` ignores, traversal rejection, cancellation, child counts, and permission-denied diagnostics.
- Relevant commands: `cargo test workspace:: --lib`, `cargo test server:: --lib`, `cargo test`.

## Related

- [Workspace Discovery and File Browser](workspace-file-browser.md)
- [Server Document State](server-document-state.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- `plans/010-Phase9-File-and-Workspace-Server.md`
