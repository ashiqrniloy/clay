# Server File Workspace Model

## Source

- `src/server/workspace.rs`
- `src/server/document.rs`

## Overview

Phase 9 introduces a server-owned workspace/open-document model alongside the existing server-canonical `DocumentState`. The model records authorized workspace roots, canonical file paths, open document identity, duplicate-open behavior, file-backed dirty state, and server-side path authorization without giving the native client filesystem authority.

## Responsibilities

- `WorkspaceState` owns workspace roots, canonical path-to-document mapping, document ID allocation, path validation, file type checks, and per-document `Arc<Mutex<DocumentState>>` handles.
- `DocumentState` still owns canonical rope text, versions, edit validation, leases, region locks, and now the dirty flag for accepted edits.
- `ServerConfig::workspace_roots` records server startup workspace roots; `IpcServer::try_new` validates them into `WorkspaceState` before protocol dispatch integration.
- `WorkspaceState::open_existing_file` performs server-side UTF-8 file loading through Tokio async file IO and registers loaded files in the open-document registry. Save/reload protocol dispatch and writes remain later Phase 9 work.

## How It Works

1. A workspace root is added with `WorkspaceState::add_root`, which canonicalizes the root and requires it to be a directory.
2. `open_existing_file` and `register_loaded_file` canonicalize a requested relative or absolute path after joining relative paths to the authorized root. Canonicalization resolves `..` segments and symlinks before authorization.
3. The canonical file must still start with the canonical root. Escaping traversal and symlinks return `WorkspaceError::OutsideRoot` before a document entry exists.
4. The canonical path must be a regular file. Directories return `WorkspaceError::DirectoryOpen`; sockets and other non-ordinary file types return `WorkspaceError::UnsupportedFileType`.
5. Valid paths build `FileDocumentState` metadata with the root ID, canonical path, and workspace-relative display path.
6. If the canonical path is already open, the registry returns the existing document ID and document handle without re-reading disk. The existing `DocumentState::acquire_access` lease rules decide whether the caller receives editable or read-only access.
7. If the path is not open, `open_existing_file` reads the file with `tokio::fs::read`, rejects invalid UTF-8 as `WorkspaceError::InvalidUtf8`, and only then registers a clean version-1 `DocumentState`.
8. `register_loaded_file` keeps the test/protocol-ready path for callers that have already obtained trusted UTF-8 text after the same canonical path validation.
9. Accepted edits in `DocumentState::apply_edit` increment the document version and mark the document dirty. Later save/reload tasks will clear dirty state only after successful persistence or clean reload transitions.

## Code Examples

```rust
let mut workspace = WorkspaceState::new();
let root_id = workspace.add_root("/workspace/project")?;
let opened = workspace
    .open_existing_file(root_id, "src/main.rs", client_id)
    .await?;
```

## Invariants and Constraints

- The server is the only component that owns workspace roots, canonical paths, file-backed document handles, and dirty state.
- Duplicate opens are keyed by canonical path and reuse one `DocumentState`, preserving one editable lease with read-only observers.
- Ordinary edit application mutates only per-document state and does not perform file IO, workspace scans, JavaScript execution, AI work, or full-document IPC.
- File paths outside registered workspace roots are rejected before a file-backed document entry is created.
- Symlinks are authorized by their canonical target, not their link location, so an in-root symlink to an outside file is denied and an in-root symlink to an in-root file maps to the target's canonical relative path.
- Directory, special-file, read, and UTF-8 validation failures happen only at open/register boundaries, never during ordinary edit application or client painting/input.
- Invalid UTF-8 files do not create or poison registry entries; a later valid open can still use the same canonical path.

## Tests

- `src/server/workspace.rs`: `duplicate_open_reuses_document_and_preserves_lease_policy` verifies duplicate canonical registrations share the document ID and lease policy.
- `src/server/workspace.rs`: `open_existing_file_loads_utf8_text` verifies server-side file loading creates a clean version-1 document snapshot.
- `src/server/workspace.rs`: `duplicate_open_reuses_loaded_document_and_lease_policy` verifies duplicate opens reuse the existing in-memory document without re-reading changed disk contents.
- `src/server/workspace.rs`: `open_invalid_utf8_reports_file_io_error_without_document_entry` verifies invalid UTF-8 is reported and leaves registry indexes empty.
- `src/server/workspace.rs`: `workspace_rejects_path_traversal_outside_root` verifies `..` traversal cannot authorize a sibling file outside the root.
- `src/server/workspace.rs`: `workspace_rejects_directory_and_special_file_open` verifies directories and Unix socket files are rejected as document opens.
- `src/server/workspace.rs`: `workspace_canonicalizes_symlink_before_authorization` verifies escaping symlinks are denied and in-root symlinks canonicalize consistently.
- `src/server/workspace.rs`: `file_backed_document_dirty_state_tracks_accepted_edits_and_clean_marking` verifies loaded files start clean, accepted edits mark dirty, and clean marking is explicit.
- `src/server/mod.rs`: `server_accepts_configured_workspace_roots_and_reports_invalid_roots` verifies startup root configuration is validated and invalid roots produce a typed server error.
- Relevant commands: `cargo test workspace:: --lib`, `cargo test server:: --lib`, `cargo test`.

## Related

- [Server Document State](server-document-state.md)
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- `plans/010-Phase9-File-and-Workspace-Server.md`
