# Editor Viewport Render Patch

## Source

- `frontend/src/editor/extensions/controller.ts` — visible-range request state machine and envelope handling.
- `frontend/src/editor/extensions/render-patch.ts` — covered-range replacement, edit mapping, and bounded guard.
- `frontend/src/editor/extensions/decorations.ts` — CodeMirror decoration field.
- `frontend/src/editor/extensions/diagnostics.ts` — CodeMirror diagnostic field.
- `frontend/src/editor/extensions/folding.ts` — CodeMirror fold field.
- `frontend/src/editor/position-index.ts` — shared UTF-16/UTF-8 position index.
- `frontend/src/editor/sync/messages.ts` — `viewportRenderRequest` payload.
- `src/protocol/parse.rs` — `ViewportRenderPatch` and status types.
- `src/server/connection/documents.rs` — viewport request validation and scheduling.
- `src/server/connection/mod.rs` — request-scoped aggregation.
- `src/server/parse_coordinator.rs`, `src/server/syntax_session.rs` — bounded parse scheduling.
- `src-tauri/src/bridge/forwarder.rs` — whole-patch delivery.
- `src-tauri/src/bridge/session.rs` — identity stamping.
- `src/protocol/codec.rs` — bounded codec round trip.
- Tests: `src/protocol/codec.rs`, `src/server/connection/mod.rs`, `src-tauri/src/bridge/forwarder.rs`, `frontend/src/editor/extensions/controller.test.ts`, `frontend/src/editor/extensions/render-patch.test.ts`.

## Overview

Plan 099 replaced heuristic viewport acknowledgements and per-member bridge
coalescing with one explicit request/reply contract. A visible viewport sends
metadata only. The server validates it, schedules bounded syntax work, and
returns exactly one `ViewportRenderPatch` for its request ID. The patch contains
ordered decoration, diagnostic, and fold members plus the exact ranges those
members authoritatively cover. Its status is `complete`, `empty`, or
`rejected`.

This flow is internal transport. Packages publish validated inert data through
the existing server-side contribution boundaries; they cannot mint request IDs,
complete requests, or access client render state.

## Flow

1. **Viewport observation.** `EditorProjection.requestViewport` reads
   `EditorView.visibleRanges`. It converts UTF-16 positions to UTF-8 bytes using
   the shared `BytePositionIndex`, skips loading/out-of-view/empty requests,
   and sends the first new viewport immediately. While one request is in flight,
   later scroll changes set a single pending flag instead of starting a timer.
2. **Request construction.** `viewportRenderRequestPayload` carries only
   `clientId`, `documentId`, `documentVersion`, monotonic `requestId`, visible
   `byteStart`/`byteEnd`, and an optional numeric `traceId`. No document text or
   parser data crosses the client boundary.
3. **Bridge routing.** Tauri rejects oversized or malformed JSON, stamps the
   handshake-owned client identity, and queues the request through the normal
   typed client path. It does not interpret byte ranges or parser output.
4. **Server validation.** `handle_viewport_render_request` checks the bound
   document, access, version, and range. Invalid, unknown, stale, or failed
   requests receive one bounded rejected patch. A valid document without a
   renderable handler, or a range that clamps to no windows, receives an
   explicit empty patch so the client pipe cannot stall. Valid ranges are
   clamped to the canonical rope before `parse_windows_covering` (cap 1).
5. **Background parse.** The connection schedules one request-scoped job
   through the document's `SyntaxSession`. The native handler parses a single
   window; extra windows sharing the same request id would be dropped by the
   per-document session. The mailbox is latest-wins and the native handler
   runs on the shared four-permit blocking executor. Each terminal path
   carries the request ID and client ID, including failure, stale, and
   superseded completion, so pending aggregation cannot leak a request slot.
6. **Aggregation.** The connection stores pending members by document/request
   with `remaining = 1`. It waits for that job, validates every
   decoration/diagnostic/fold member, and derives `coveredRanges` from member
   output ranges. The wider parse context used for grammar correctness is never
   claimed as rendered coverage. One complete patch is then written for the
   request ID.
7. **Bridge delivery.** `Forwarder` places complete patches in a bounded
   latest-wins slot keyed only by document. A newer undelivered whole patch
   replaces the older one. Members inside a patch never coalesce; edit
   acknowledgements and other live events remain strict FIFO. Disconnect notices
   bypass both lanes.
8. **Atomic client application.** `EditorProjection.handleEnvelope` drops a
   patch older than the newest request ID. For a complete patch it prepares all
   valid member effects, calls `viewportArrived`, and dispatches the effects in
   one CodeMirror transaction. Empty and rejected patches dispatch no render
   effects but still free the in-flight slot immediately. A pending latest
   viewport is requested after that terminal reply; no 400 ms safety valve is
   involved.
9. **Covered-range projection.** Decoration, diagnostic, and fold fields replace
   only their own authority inside the declared coverage, map retained items
   through local edits, and prune outside the visible range plus bounded
   overscan. A patch cannot erase another package, layer, document, or pane.

## Protocol shape

```text
ViewportRenderRequest {
  clientId, documentId, documentVersion,
  requestId, byteStart, byteEnd, traceId?
}

ViewportRenderPatch {
  requestId, documentId, documentVersion,
  status: complete | empty | rejected,
  reason?, coveredRanges,
  decorations[], diagnostics[], folds[], traceId?
}
```

Edit-driven parse output still uses its existing `DecorationSet`,
`DecorationBatch`, `DiagnosticSet`, and `FoldingRangeSet` events. Those events
are not used to acknowledge a viewport request; only its atomic patch does that.

## Invariants and Constraints

- Each request ID has one terminal patch, including empty and rejected outcomes.
- The request's parse context range and authoritative output coverage stay
  separate; only validated member ranges enter `coveredRanges`.
- Request IDs are monotonic per editor projection. Stale patch IDs are ignored.
- Forwarder coalescing is whole-patch and document-scoped; sibling members
  remain intact and distinct documents never share a slot.
- The client applies one complete patch in one CodeMirror transaction. Render
  fields, not React state or bridge caches, own projected marks, links, inlays,
  diagnostics, and folds.
- `VIEWPORT_OVERSCAN` is 4,096 UTF-16 positions, widened to the covered range
  for small viewports. Native parse windows remain bounded by grammar policy and
  the maximum viewport-window count.
- Server payload, syntax-cache, document-memory, and trace capacities remain
  host-owned. Profiling metadata is numeric and source-free.
- Server/package authority is unchanged: validation, provenance, versions,
  access, parser execution, and request completion stay outside the webview.

## Tests

- `src/protocol/codec.rs::protocol_round_trips_viewport_render_patches` —
  complete split-member, empty, and rejected patch codec coverage.
- `src/server/connection/mod.rs::viewport_render_requests_answer_one_patch_per_request_id` —
  stale/invalid rejection, range clamping, ordered complete output, and no
  duplicate member frames.
- `src-tauri/src/bridge/forwarder.rs::coalescing_keeps_latest_whole_patch_and_live_order` —
  whole-patch replacement, live FIFO, and disconnect bypass.
- `src-tauri/src/bridge/forwarder.rs::sibling_members_stay_one_complete_patch` —
  mixed syntax/semantic members remain one intact patch.
- `frontend/src/editor/extensions/controller.test.ts` — stale request dropping
  and explicit empty/rejected completion.
- `frontend/src/editor/extensions/render-patch.test.ts` — exact authority and
  coverage replacement, edit mapping, and bounded retention.
- `frontend/src/editor/extensions/performance.test.ts` — 100 sliding patches
  retain constant-size render state and four-pane work stays linear.

Run focused coverage with:

```bash
cargo test --test protocol protocol_round_trips_viewport_render_patches
cargo test --test runtime viewport_render_requests_answer_one_patch_per_request_id
cd frontend && npm test -- --run src/editor/extensions/controller.test.ts
```

## Related

- [React CodeMirror Editor](../modules/react-codemirror-editor.md)
- [Decoration Transport](../modules/decoration-transport.md)
- [Range Diagnostics](../modules/range-diagnostics.md)
- [Folding Ranges](../modules/folding-ranges.md)
- [Syntax Sessions](../modules/syntax-sessions.md)
- [Desktop Typed Bridge](../modules/desktop-typed-bridge.md)
- [Frontend Edit Synchronization](frontend-edit-synchronization.md)
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `plans/099-Clay-Editor-Performance-Overhaul.md`
