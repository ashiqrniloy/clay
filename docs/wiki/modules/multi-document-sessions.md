# Multi-Document Sessions (Phase 20)

## Source

- `src/editor/document_session.rs`
- `src/masonry_editor.rs`
- `src/client/mod.rs` (`ClientSyncState`, `ClientEditQueue`)
- `src/client/clipboard.rs`
- `runtime/js/editor.ts`

## Overview

Phase 20 replaces the single-editor-buffer model with a bounded client-local multi-document session store. Opening a second file stashes the prior `EditorSurface` (text, caret/selection, viewport, edit history, dirty chrome) instead of destroying it. Users switch active documents through a client-local transient menu without re-downloading text or waiting on the server. Each document retains mode, status, dirty state, lease, manifest version, caret, viewport, and pending-edit state. Server open-document registry and lease authority remain authoritative.

## Responsibilities

- `DocumentSessionStore` owns a `HashMap<DocumentId, RetainedDocumentSession>` with LRU eviction capped at `CLIENT_DOCUMENT_SESSION_MAX` (64, defined in `src/perf/budgets.rs`).
- `RetainedDocumentSession` holds the stashed `EditorSurface` (via `std::mem::take`), `ClientSyncState` clone, last-known document metadata, and dirty flag.
- `EditorWidget` delegates session management: `stash_active_session()` preserves the current surface before switching, `open_document_session()` restores or creates a fresh session, `activate_document()` switches the active document by `DocumentId`.
- `clientShowOpenDocuments` opens a `TransientMenuSession` listing all retained sessions with dirty markers and active-document indicator; `clientActivateDocument` extracts the `DocumentId` from the menu action arguments and calls `activate_document`.
- `ClientSyncState` gained a `document_id: Option<DocumentId>` field so `acknowledge`, `reject`, and `apply_resync_snapshot` validate the incoming document ID matches the owner before mutating state. Stale `EditAck`s for backgrounded documents are silently ignored.
- `ClientEditQueue::install_document_sync_state` restores stashed pending edits and confirmed version when a document becomes active again, preventing orphaned queues or lost sync.

## How It Works

1. A `DocumentOpened` event for a document ID different from the active document triggers `stash_active_session`. The current `EditorSurface` is extracted via `std::mem::take` (not cloned — `LayoutState`, `PerfRecorder`, Parley caches are non-`Clone`). The `behavior_manifest` is cloned from the outgoing surface and reinstalled on the fresh default surface alongside theme/typography/specifier.
2. `open_document_session` checks the session store for the requested `DocumentId`. If found, it restores the stashed surface and applies `install_document_sync_state` to reconnect the edit queue to the correct pending-edits/confirmed-version. If not found (fresh document), a new default surface is initialized.
3. `activate_document` is client-local: it exchanges the active surface, swaps sync state, and requests render/layout/accessibility refresh. No server round-trip is needed for switching.
4. `DocumentOpened` no longer calls `apply_resync_snapshot` at the connection layer — the widget installs authority for the newly active document itself, preventing `DocumentOpened` from wiping live sync state for the current document.
5. `ResyncSnapshot` has a two-path guard: when no document has been opened yet (bootstrap/default state, `has_opened_document` false), it applies unconditionally to the active editor. When `has_opened_document` is true, `document_id` is validated against the session map, preventing stale resyncs for background documents.
6. Eviction removes the least-recently-used session when the store exceeds `CLIENT_DOCUMENT_SESSION_MAX`. Evicted sessions are dropped; re-opening an evicted document requires a fresh server `DocumentOpened` snapshot.
7. Each retained session stashes its own `EditorStatus`, dirty flag, `ClientSyncState`, and document metadata. Switching restores the corresponding status chrome, undo stack, and pending-edit queue.

## Invariants and Constraints

- `EditorSurface` is non-`Clone`; stashing uses `std::mem::take` with explicit restoration of non-`Clone` state (`behavior_manifest`, theme specifier, typography).
- `DocumentSessionStore` is bounded at 64 (`CLIENT_DOCUMENT_SESSION_MAX`); eviction is silent and does not preserve unsaved edits (dirty documents should be saved before eviction under user control).
- Server authority is unchanged: the server still owns the canonical document registry, leases, and dirty state. Multi-document sessions are a client-local UI convenience.
- `EditorSurface::set_theme_specifier` is used to restore the theme label after `std::mem::take` stashing, since `set_active_theme` is the normal path but is not available after surface replacement.
- Duplicate-open detection is server-side by canonical path; the client multi-document store is keyed by `DocumentId`, not path.
- Pending edits for backgrounded documents are preserved in stashed `ClientSyncState` and restored on activation.

## Tests

- `src/masonry_editor.rs`: `retain_prior_session_on_second_file_open` validates stash/switch behavior.
- `src/masonry_editor.rs`: `document_switch_restores_edit_history_and_dirty_state` validates undo stack and dirty flag restoration.
- `src/masonry_editor.rs`: `show_open_documents_menu_lists_all_sessions_with_active_and_dirty_markers` validates transient menu composition.
- `src/masonry_editor.rs`: `session_eviction_drops_least_recently_used` validates LRU eviction at `CLIENT_DOCUMENT_SESSION_MAX`.
- `src/client/mod.rs`: `stale_ack_for_backgrounded_document_is_silently_ignored` validates document-scoped sync acks.
- `src/client/mod.rs`: `client_forwards_document_opened_without_replacing_live_sync_state` validates `DocumentOpened` no longer resets sync at connection layer.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot` and `opened_file_edits_continue_as_deltas` validate multi-doc bootstrap guards.
- Relevant commands: `cargo test masonry_editor --quiet`, `cargo test client --quiet`.

## Related

- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Phase 20 Daily Editing Product Hardening Primitive Review](phase20-daily-editing-product-hardening-primitive-review.md)
- [File Open, Save, and Reload Workflow](../../development/file-open-save-reload-workflow.md)
- `src/editor/document_session.rs`
- `src/masonry_editor.rs`
