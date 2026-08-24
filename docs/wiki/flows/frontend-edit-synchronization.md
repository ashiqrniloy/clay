# Frontend Edit Synchronization Flow

End-to-end trace of one user keystroke in the Tauri/React client, from DOM
event to server acknowledgement. Server authority and the retained Rust
client primitives are described in
[Versioned Text Synchronization](versioned-text-synchronization.md); this
page is the current client path.

## Source

- `frontend/src/editor/ClayEditor.tsx` — CodeMirror host.
- `frontend/src/editor/sync/session.ts` — `createDocumentSession`: per-tab
  document session, optimistic versions, inflight tracking.
- `frontend/src/editor/sync/operations.ts`, `messages.ts` — change→operation
  conversion and protocol payload builders.
- `frontend/src/editor/extensions/controller.ts` — behavior-manifest-driven
  keymap/completion projection (`EditorProjection`).
- `frontend/src/bridge/client.ts` — the only `invoke` path.

## Flow

1. **Keystroke → CodeMirror transaction.** CodeMirror applies the change
   locally first; paint never waits on IPC, JS packages, or the server.
2. **Origin filter.** The session tags programmatic writes with a
   `clayOrigin` annotation (`resync` / `correction` / `remote` /
   `programmatic`). The update listener ignores annotated transactions —
   only *user* changes emit edits, so echoes of our own or the server's text
   cannot loop back into the edit queue.
3. **Behavior gate + optimistic version.** `emitUserChanges` converts the
   diff to operations, assigns the next optimistic base version and a
   monotonic `transactionId`, records it in the `inflight` set, and builds
   the protocol `Edit` payload via `editPayload`. The local shadow text
   (`authoritativeText` baseline) is updated optimistically.
4. **Send without blocking.** The payload goes through `bridge/client.ts`
   `request()` (fire-and-forget promise; failures land in the document store
   as a diagnostic). No synchronous round trip before render.
5. **Server validation.** The Clay server applies the edit against its
   canonical rope: version check, lease check, behavior-version check. It
   replies with an acknowledgement carrying our `transactionId`.
6. **Ack / reject.** On ack, the session drops the id from `inflight`.
   On rejection (stale version, lease loss, behavior bump), pending edits are
   invalidated; the UI surfaces the typed rejection and can
   `requestResync()`, which replaces the document via an annotated
   transaction (step 2's filter keeps this from re-emitting).

## Invariants

- Keystroke-to-local-paint touches only CodeMirror + React state.
- At most one optimistic chain per document; every sent edit is tracked by
  `transactionId` until acknowledged.
- All full-text replacements enter CodeMirror as annotated transactions so
  origin classification stays total.
- Open-before-handshake races (layout restore) park the path in
  `pendingOpenPath` until metadata delivers the workspace root.

## Tests

- `frontend/src/test/editor.test.tsx` — host wiring, label sanitization.
- Position mapping: `frontend/src/editor/position-map.test.ts`.
- Rust-side queue/staleness semantics: `tests/suites/protocol.rs`
  (see [Desktop Typed Bridge](../modules/desktop-typed-bridge.md)).

## Related

- [React Client Bridge](../modules/react-client-bridge.md).
- [Versioned Text Synchronization](versioned-text-synchronization.md) —
  shadow/version state machine shared with the retained Rust client module.
