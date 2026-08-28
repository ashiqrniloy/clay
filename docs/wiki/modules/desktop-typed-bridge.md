# Desktop Typed Bridge (Tauri v2)

## Source

- `src-tauri/src/bridge/dto.rs` — typed bootstrap, runtime/theme DTOs, and envelope projection.
- `src-tauri/src/bridge/errors.rs` — bounded sanitized bridge errors.
- `src-tauri/src/bridge/session.rs` — bootstrap/reconnect lifecycle, request validation, identity stamping, and event pump.
- `src-tauri/src/bridge/forwarder.rs` — bounded FIFO/latest-wins delivery lanes.
- `src-tauri/src/bridge/editor.rs` — renderer-neutral editor DTO helpers.
- `src/client/mod.rs` — typed client queue and connection events.
- `src/protocol/{mod,parse,decorations}.rs` — v29 viewport patch and trace-bearing protocol shapes.
- `frontend/src/bridge/{client,types,errors}.ts` — the only frontend Tauri boundary.
- Tests: `src-tauri/src/bridge/forwarder.rs`, `src-tauri/tests/{bridge_session,dto_roundtrips}.rs`, `tests/editor_performance.rs`, `frontend/src/test/bridge.test.ts`.

## Overview

The bridge connects one React webview session to the Clay server without
reimplementing protocol or document authority. Rust owns the Tauri invoke
surface, bootstrap/reconnect lifecycle, identity stamping, bounded delivery,
and process supervision. The webview sees typed camelCase DTOs, envelope events,
and a request function carrying one protocol message as JSON.

Plan 099 keeps viewport rendering on this same boundary: protocol-v29
`ViewportRenderPatch` values are complete request-scoped answers. The bridge
may replace an obsolete whole patch for one document, but never coalesces the
patch's decoration/diagnostic/fold members independently.

## Responsibilities

| Module         | Responsibility                                                                                                       |
| -------------- | -------------------------------------------------------------------------------------------------------------------- |
| `dto.rs`       | `BootstrapDto`, runtime/theme/typography snapshots, and `BridgeEnvelope`.                                            |
| `errors.rs`    | Sanitized `{ code, message }` errors with `MAX_REQUEST_BYTES = 512 KiB` request protection.                          |
| `session.rs`   | Single live client session, handshake/reconnect generations, request parsing, client-id stamping, and event pumping. |
| `forwarder.rs` | Live FIFO lane (capacity 512), whole viewport-patch latest-wins slots, lifecycle bypass, and delivery metrics.       |
| `editor.rs`    | Renderer-neutral editor conversion helpers and camelCase shape pins.                                                 |

The bridge does not own document text, parser state, syntax executors, render
fields, request completion, package execution, or filesystem authority.

## Session and request flow

1. `bootstrapSession` subscribes the webview before bootstrap so events emitted
   during handshake cannot be lost. The bridge keeps one connected session and
   rejects webview-supplied `Hello` messages.
2. Rust probes/adopts a compatible local server or starts one under the desktop
   supervisor. Reconnect aborts the old event pump before starting a new
   generation, preventing stale stream data from reaching React.
3. `request_on` rejects bodies above `MAX_REQUEST_BYTES`, parses a strict
   `ClientMessage`, including `DocumentChunkRequest` for progressive loading,
   resolves the target tab/client, stamps post-Hello identity,
   and queues it through the existing typed client queue. `Edit` uses the queue's
   optimistic bookkeeping; other messages use exhaustive identity stamping.
4. Server events become `ClientConnectionEvent` values, then `BridgeEnvelope`
   values. The bridge forwards validated DTOs without interpreting rope offsets,
   parser output, or patch completion.
5. `workspace-controller.ts` routes each document/tab envelope to its owning
   pane session. There is no global frontend document-session mirror.

## Viewport patch delivery

`ViewportRenderPatch` is placed in the latest-wins map under
`vrpatch|<documentId>`. A newer undelivered patch for that document replaces the
older complete answer wholesale; the client already drops stale request IDs.
All other events, including edit acknowledgements, standalone decoration/
diagnostic/fold events, behavior/runtime updates, and document chunks, use the
bounded live FIFO lane. A disconnected event is delivered immediately outside
both lanes so recovery status is not trapped behind render traffic.

The forwarder records only numeric trace metadata when profiling is enabled.
`BridgeEnvelope` is cloned for each sink; failed sinks are removed, which makes
window teardown stop delivery without leaving a subscriber behind.

```text
server ClientConnectionEvent
  -> session identity / DTO projection
  -> Forwarder
       live FIFO: edits, chunks, status, members, runtime events
       latest slot: one whole ViewportRenderPatch per document
  -> Tauri Channel
  -> workspace controller / owning pane session
```

## JSON and identity boundaries

Protocol types use serde-derived adjacent envelopes and camelCase names. The
bridge preserves bounded heads/chunks, document/version metadata, and typed
rejections. Menu session IDs cross as strings because they can use the server
high bit; ordinary sequential editor/request IDs remain JSON numbers.

Tauri overwrites forged nested `clientId` values. The server still validates
access, lease, document, version, request range, provenance, and completion
identity; bridge stamping is correlation protection, not a grant.

## Performance and security constraints

- The bridge uses bounded queues and natural backpressure; it never grows an
  unbounded event list or waits on a parser to answer an edit.
- Viewport patch coalescing is whole-message/document-scoped only. Sibling
  ranges, package layers, diagnostics, and folds remain intact.
- Document chunks are size-capped and identity-stamped. The bridge never holds a
  second full document buffer.
- Profiling is opt-in. Reports retain bounded numeric stage data and no source,
  path, credential, package code, or raw diagnostic content.
- The webview cannot access sockets, archive bytes, frame codecs, process spawn,
  raw Tauri commands, leases, parser handles, or package runtime authority.
- `script-src` remains strict; the scoped CodeMirror style allowance is
  documented with the React editor because it does not grant script execution.

## Tests

- `src-tauri/tests/bridge_session.rs` — real server bootstrap, tab registry,
  disconnect, reconnect generation, and identity lifecycle.
- `src-tauri/tests/dto_roundtrips.rs` — typed JSON round trips and exhaustive
  event/message shape coverage.
- `src-tauri/src/bridge/forwarder.rs::coalescing_keeps_latest_whole_patch_and_live_order` —
  latest whole-patch replacement, FIFO ordering, and disconnect bypass.
- `src-tauri/src/bridge/forwarder.rs::sibling_members_stay_one_complete_patch` —
  24 mixed members remain one complete patch.
- `frontend/src/test/bridge.test.ts` — frontend bridge stores, dispatcher, and
  normalized errors.
- `tests/editor_performance.rs` — protocol matrix verifies one patch per
  request ID, exact edit/version accounting, and close retirement.

Run focused coverage with:

```bash
cargo test -p clay-desktop --all-targets
cargo test --test runtime editor_performance_matrix_holds_deterministic_invariants -- --exact
cd frontend && npm test -- --run src/test/bridge.test.ts
```

## Related

- [React Client Bridge](react-client-bridge.md)
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- [Protocol Codec](protocol-codec.md)
- [Syntax Sessions](syntax-sessions.md)
- [Tauri Desktop Shell](tauri-desktop-shell.md)
- `src-tauri/src/bridge/forwarder.rs`
- `src-tauri/src/bridge/session.rs`
