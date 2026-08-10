# Multi-Document Sessions (Phase 20)

## Source

- `src/editor/document_session.rs`
- `src/masonry_editor.rs`
- `src/masonry_pane_document.rs`
- `src/client/mod.rs` (`ClientSyncState`, `ClientEditQueue`)
- `src/driver/mod.rs` (`Driver` cross-pane aggregation; 22.7 extraction)
- `src/client/clipboard.rs`
- `runtime/js/editor.js`

## Overview

Phase 20 replaces the single-editor-buffer model with a bounded client-local multi-document session store. Opening a second file stashes the prior `EditorSurface` (text, caret/selection, viewport, edit history, dirty chrome) instead of destroying it. Users switch active documents through a client-local transient menu without re-downloading text or waiting on the server. Each document retains mode, status, dirty state, lease, manifest version, caret, viewport, and pending-edit state. Server open-document registry and lease authority remain authoritative.

Phase 22.2 extends the same mechanics per pane: each `PaneDocumentView` owns its own `DocumentSessionStore` (same 64-session LRU ceiling), the shared `ClientSyncState` became a per-document map so several panes' documents can have in-flight edits concurrently, and the open-documents switcher aggregates sessions across ALL panes with cross-pane focus-and-switch activation.

## Phase 22.8: Isolation verification

Phase 22.8 keeps this Phase 22.2 model intact while moving server workspace/document
state behind the tab binding. The verified path is: split a tab locally (bounded
by `MAX_PANES_PER_TAB`), mount one `PaneDocumentView` per opened file, share only
the connection's `ClientEditQueue`, and route acknowledgements, leases, versions,
dirty state, and behavior manifests by `DocumentId`. A duplicate open is resolved
by the driver to the existing owning pane; it never creates a second view.

The regression matrix covers two pane documents, local typing isolation, per-
document lease/version reservations, retained-session switching, concurrent
major-mode layers, the four-pane cap, and the server's disjoint per-tab
workspace/document sets. No split-tree, pane-host, protocol, or hot-path code
was added for 22.8. Commands: `cargo test --lib masonry_shell --quiet`,
`cargo test --lib masonry_pane_document --quiet`, `cargo test --bin clay --quiet`,
and `cargo test --all-targets`.

## Responsibilities

- `DocumentSessionStore` owns a `HashMap<DocumentId, RetainedDocumentSession>` with LRU eviction capped at `CLIENT_DOCUMENT_SESSION_MAX` (64, defined in `src/perf/budgets.rs`).
- `RetainedDocumentSession` holds the stashed `EditorSurface` (via `std::mem::take`), `ClientSyncState` clone, last-known document metadata, and dirty flag.
- `EditorWidget` delegates session management: `stash_active_session()` preserves the current surface before switching, `open_document_session()` restores or creates a fresh session, `activate_document()` switches the active document by `DocumentId`.
- `clientShowOpenDocuments` opens a `TransientMenuSession` listing all retained sessions with dirty markers and active-document indicator; `clientActivateDocument` extracts the `DocumentId` from the menu action arguments and calls `activate_document`.
- `ClientSyncState` gained a `document_id: Option<DocumentId>` field so `acknowledge`, `reject`, and `apply_resync_snapshot` validate the incoming document ID matches the owner before mutating state. Stale `EditAck`s for backgrounded documents are silently ignored.
- `ClientEditQueue::install_document_sync_state` restores stashed pending edits and confirmed version when a document becomes active again, preventing orphaned queues or lost sync.
- **Phase 22.2**: `ClientSyncState` refactored to `HashMap<DocumentId, DocumentSyncState>` (`confirmed_version`, `optimistic_version`, pending reservations, `last_resync`, `lease_id`). `enqueue_edit_event(document_id, ...)` attaches the per-document lease; `snapshot_for(document_id)` / `sync_snapshot_for(document_id)` expose one document's state; `total_pending_len()` / `confirmed_version_for(document_id)` keep the connection loop per-document. `DocumentReloaded` updates the specific document's state regardless of which pane is active. A single `ClientEditQueue` serves all panes — no per-pane clones.
- **Phase 22.2**: `DocumentSessionStore` gained `document_ids()` and `clear()` (pane-close cleanup) and `list_retained()` (cross-pane switcher aggregation). Each pane's store is independent; `PaneDocumentView::close_pane` releases the active document AND all retained sessions via the shared edit queue.

## How It Works

1. A `DocumentOpened` event for a document ID different from the active document triggers `stash_active_session`. The current `EditorSurface` is extracted via `std::mem::take` (not cloned — `LayoutState`, `PerfRecorder`, Parley caches are non-`Clone`). The `behavior_manifest` is cloned from the outgoing surface and reinstalled on the fresh default surface alongside theme/typography/specifier.
2. `open_document_session` checks the session store for the requested `DocumentId`. If found, it restores the stashed surface and applies `install_document_sync_state` to reconnect the edit queue to the correct pending-edits/confirmed-version. If not found (fresh document), a new default surface is initialized.
3. `activate_document` is client-local: it exchanges the active surface, swaps sync state, and requests render/layout/accessibility refresh. No server round-trip is needed for switching.
4. `DocumentOpened` no longer calls `apply_resync_snapshot` at the connection layer — the widget installs authority for the newly active document itself, preventing `DocumentOpened` from wiping live sync state for the current document.
5. `ResyncSnapshot` has a two-path guard: when no document has been opened yet (bootstrap/default state, `has_opened_document` false), it applies unconditionally to the active editor. When `has_opened_document` is true, `document_id` is validated against the session map, preventing stale resyncs for background documents.
6. Eviction removes the least-recently-used session when the store exceeds `CLIENT_DOCUMENT_SESSION_MAX`. `SessionEviction` returns each evicted document ID; `EditorWidget` enqueues `CloseDocument(force: true)` so server access and document-scoped coordinator state do not outlive the retained client session. Re-opening requires a fresh server `DocumentOpened` snapshot.
7. Each retained session stashes its own `EditorStatus`, dirty flag, `ClientSyncState`, and document metadata. Switching restores the corresponding status chrome, undo stack, and pending-edit queue.

## Invariants and Constraints

- `EditorSurface` is non-`Clone`; stashing uses `std::mem::take` with explicit restoration of non-`Clone` state (`behavior_manifest`, theme specifier, typography).
- `DocumentSessionStore` is bounded at 64 (`CLIENT_DOCUMENT_SESSION_MAX`); eviction discards the retained session's unsaved edits and therefore sends `CloseDocument(force: true)`. Explicit dirty-document close without force remains server-rejected.
- Server authority is unchanged: the server owns the canonical document registry, leases, dirty state, access-holder set, and final-holder teardown. Multi-document sessions are a client-local UI convenience.
- `EditorSurface::set_theme_specifier` is used to restore the theme label after `std::mem::take` stashing, since `set_active_theme` is the normal path but is not available after surface replacement.
- Duplicate-open detection is server-side by canonical path; the client multi-document store is keyed by `DocumentId`, not path.
- Pending edits for backgrounded documents are preserved in stashed `ClientSyncState` and restored on activation.

## Reconnect (Phase 22.3)

A tab whose connection drops auto-reconnects (driver `start_tab_reconnect` → `Reclaim` the registry `TabId` → re-open documents). The client-side restore relies on retained in-memory state, so per-document sessions now retain their open identity:

- `RetainedDocumentSession` carries `workspace_root_id` + `path` (recorded when the session is stashed; the active document's identity is kept on the view).
- `PaneDocumentView::documents_for_reopen` returns the active + retained identities (deduped by path) for re-opening through the plain `OpenDocument` message — a fresh connection holds no selected-file capability for previously open documents, and `RequestResync` fails closed for documents the new connection lacks access to.
- **Phase 22.5**: the same per-session identity feeds window-state persistence — `PaneDocumentView::active_document_identity()` / the `EditorWidget` wrapper return `(workspace_root_id, path)` for the ACTIVE document only (retained-but-inactive sessions are excluded by design: persisted state is "what the panes show", and a closed document persisting as open degrades to an empty pane on restore). The persisted identity is the workspace-relative path; the root id is re-learned at restore from the registry `TabEntry.workspace_root_id`. See [Tabs and Independent Client Views](tabs-and-clients.md) Phase 22.5 for the restore flow.
- `PaneDocumentView::reconnect` swaps in the new connection's edit queue, clears the disconnect recovery menu, and sets `pending_reconnect_resync` so the next `DocumentOpened` for the active document reinstalls server state instead of the 22.2 duplicate-open no-op. Retained sessions are replaced by their re-opened snapshots; the view's split tree and pane hosts are untouched (the shell `rekey_tab` moves the whole `TabChrome`).
- Server-side authority is unchanged: the registry entry survives a connection drop (the tab is reclaimed, not recreated); the connection's document access/leases release via `cleanup_connection_documents`, so re-opens grant fresh access on the new connection.

## Tests

- `src/masonry_pane_document.rs` (22.2): per-pane session stash/activate, close-pane release of active + retained sessions, cross-pane menu aggregation + `ActivateDocumentInPane` routing, per-document edit queueing, duplicate-open no-op. Command: `cargo test --lib masonry_pane_document --quiet`.
- `src/masonry_editor.rs`: `retain_prior_session_on_second_file_open` validates stash/switch behavior.
- `src/masonry_editor.rs`: `document_switch_restores_edit_history_and_dirty_state` validates undo stack and dirty flag restoration.
- `src/masonry_editor.rs`: `show_open_documents_menu_lists_all_sessions_with_active_and_dirty_markers` validates transient menu composition.
- `src/masonry_editor.rs`: `session_eviction_drops_least_recently_used` validates LRU eviction at `CLIENT_DOCUMENT_SESSION_MAX`; queue tests cover forwarding evicted IDs as `CloseDocument`.
- `src/server/connection.rs`: close/disconnect tests prove shared-document survival, final-holder registry teardown, access loss, and cleanup when peer close races asynchronous output.
- `src/client/mod.rs`: `stale_ack_for_backgrounded_document_is_silently_ignored` validates document-scoped sync acks.
- `src/client/mod.rs`: `client_forwards_document_opened_without_replacing_live_sync_state` validates `DocumentOpened` no longer resets sync at connection layer.
- `src/masonry_editor.rs`: `resync_event_replaces_editor_snapshot` and `opened_file_edits_continue_as_deltas` validate multi-doc bootstrap guards.
- Relevant commands: `cargo test masonry_editor --quiet`, `cargo test client --quiet`.

## Related

- [Pane Document Views](pane-document-views.md) — per-pane session stores and cross-pane aggregation (22.2)
- [Tabs and Independent Client Views](tabs-and-clients.md) — per-tab connections and reconnect/reclaim (22.3)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Phase 20 Daily Editing Product Hardening Primitive Review](phase20-daily-editing-product-hardening-primitive-review.md)
- [File Open, Save, and Reload Workflow](../../development/file-open-save-reload-workflow.md)
- `src/editor/document_session.rs`
- `src/masonry_editor.rs`
