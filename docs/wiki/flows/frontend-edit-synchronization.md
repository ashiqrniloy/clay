# Frontend Edit Synchronization Flow

This is the current Tauri/React client path for one user edit. The server
remains canonical; CodeMirror owns the current local text and applies user
changes before IPC returns.

## Source

- `frontend/src/editor/create-editor.ts` — CodeMirror state, input listener, and shared position field.
- `frontend/src/editor/ClayEditor.tsx` — one view/session mount and read-only projection.
- `frontend/src/editor/sync/session.ts` — per-pane document session, optimistic versions, chunks, and envelopes.
- `frontend/src/editor/sync/operations.ts` — byte-range operation conversion.
- `frontend/src/editor/sync/messages.ts` — protocol payload builders.
- `frontend/src/editor/position-index.ts` — incremental UTF-16/UTF-8 mapping.
- `frontend/src/editor/transactions.ts` — user/programmatic origin annotations.
- `frontend/src/state/document-store.ts` — metadata and stable projections.
- `frontend/src/shell/workspace-controller.ts` — pane/tab routing and persistence gating.
- `frontend/src/bridge/client.ts` — only Tauri `invoke` wrapper.
- `src/server/document.rs` and `src/server/connection/mod.rs` — canonical validation and acknowledgements.
- Tests: `frontend/src/editor/sync/session.test.ts`, `frontend/src/test/editor.test.tsx`, `frontend/src/editor/position-map.test.ts`, `frontend/src/editor/extensions/performance.test.ts`, `tests/editor_performance.rs`.

## Flow

1. **Local transaction.** CodeMirror applies the user change immediately. Its
   state contains one current `Text`, the selection/history, and the
   `bytePositionField` value for that document version.
2. **Origin filter.** `createEditor` emits only transactions accepted by
   `shouldEmitEdit`. Head/chunk/reload/resync/correction writes carry
   `programmaticAnnotations()` (`clayOrigin: "programmatic"` plus
   `Transaction.addToHistory.of(false)`), so server-authored text cannot loop
   back as a user edit or pollute undo history.
3. **Indexed conversion.** The listener passes the transaction-start `Text`,
   changes, and transaction-start `BytePositionIndex` to
   `emitUserChanges` → `changesToOperations`. Conversion is a tree descent plus
   a bounded line scan; it does not rebuild a full-document index on each edit.
4. **Optimistic enqueue.** The session assigns a monotonic transaction ID,
   increments pending metadata, marks dirty, and sends compact UTF-8 byte-range
   `Edit` operations without awaiting the server. The browser paints independently
   of IPC, Tauri, package JavaScript, and parser work.
5. **Server authority.** The server checks document identity, base version,
   lease, behavior version, UTF-8 boundaries, range locks, and operation bounds
   before mutating its canonical `crop::Rope`. An accepted edit increments the
   version once and returns `EditAck`; a rejection leaves canonical text and
   version unchanged.
6. **Ack/reject.** The session removes the acknowledged transaction and updates
   confirmed/pending metadata. Recoverable rejection requests a fresh resync;
   resync installs an authoritative snapshot through the same no-history path.
   Late/mismatched events are ignored by document/version checks.
7. **Background render.** Accepted edits schedule server-side syntax sessions.
   Resulting decorations, diagnostics, and folds update only their covered
   ranges. Viewport work uses the separate [atomic viewport render patch](editor-viewport-render-patch.md)
   flow and never blocks edit acknowledgement.

## Progressive load and ownership

Open, reload, and resync first install a bounded `DocumentTextHead`. The session
keeps one outstanding `DocumentChunkRequest`, appends returned UTF-8 text as
annotated no-history transactions, and keeps the document read-only while
`loading` is true. When no view is attached, `detachedDoc` is the one current
snapshot; when a view is attached, `view.state.doc` is authoritative. Detach
stores the latest user text and remount restores it without a second live text
owner. Same-length reloads compare `Text.eq`, not only length, before deciding
whether to replace content.

Each pane gets one `DocumentSession` from `workspace-controller.ts`. There is
no app-wide `session-singleton.ts`. The workspace controller routes envelopes
by owning tab/pane/document, tracks in-flight open paths during restore, and
only schedules persistence when document identity/path/dirty changes. It still
publishes shell loading/diagnostic changes, but version/pending acknowledgements
stay pane-local and do not cause whole-shell persistence churn.

## Invariants

- CodeMirror owns current text; React stores metadata only.
- One edit path produces compact deltas; full text is used only for bounded
  head/chunk/open/reload/resync installation.
- Programmatic text installation is silent to the edit emitter and undo history.
- No await or full-document scan occurs between local input and local paint.
- Server identity, lease, version, provenance, and access checks cannot be
  bypassed by client payloads.
- Each pane receives only its own document events; duplicate open replies are
  attributed by in-flight path before placeholder IDs.

## Tests

- `frontend/src/editor/sync/session.test.ts` — ack ordering, resync, chunk
  dedupe, 50 MiB head request count, same-length replacement, no-history load,
  and detach/remount ownership.
- `frontend/src/test/editor.test.tsx` — local dispatch before blocked IPC and
  read-only reconfiguration behavior.
- `frontend/src/editor/position-map.test.ts` — golden Unicode conversion and
  incremental-index differential/property coverage.
- `frontend/src/editor/extensions/performance.test.ts` — real editor path,
  retained render bounds, one current `Text`, and pane-isolated work.
- `frontend/src/shell/workspace-controller.test.ts` — restore/open routing and
  shell-status/persistence subscription isolation.
- `tests/editor_performance.rs` — real-protocol edit/version/save/reload/resync
  and viewport-patch matrix.

Run focused coverage with:

```bash
cd frontend && npm test -- --run src/editor/sync/session.test.ts
cargo test --test runtime editor_performance_matrix_holds_deterministic_invariants -- --exact
```

## Related

- [Document Chunked Loading](document-chunked-loading.md)
- [Editor Viewport Render Patch](editor-viewport-render-patch.md)
- [React CodeMirror Editor](../modules/react-codemirror-editor.md)
- [React Client Bridge](../modules/react-client-bridge.md)
- [Server Document State](../modules/server-document-state.md)
- [Syntax Sessions](../modules/syntax-sessions.md)
- [Versioned Text Synchronization](versioned-text-synchronization.md)
