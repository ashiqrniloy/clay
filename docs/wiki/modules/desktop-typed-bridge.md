# Desktop Typed Bridge (Tauri v2)

## What it is

`src-tauri/src/bridge/` connects the React webview to the Clay server through
one live client session. It reuses `clay::client` (handshake, optimistic edit
queue, staleness validation) instead of reimplementing the protocol; the
bridge adds serde typing, bounded delivery, session lifecycle, and identity
stamping.

## Modules

| File | Responsibility |
| --- | --- |
| `dto.rs` | `BootstrapDto`, resolved theme/typography DTOs, Rust-parsed package component DTOs, and `BridgeEnvelope` (`event` / `routed` / `disconnected` / `themeSnapshot` / `runtimeSnapshot`) |
| `errors.rs` | `BridgeError { code, message }`; sanitized, length-capped; hard request cap `MAX_REQUEST_BYTES = 512 KiB` |
| `forwarder.rs` | Bounded ordered delivery: FIFO live lane (512) for everything except viewport-resynthesizable decoration/folding sets, which take latest-wins slots keyed by `(document, provenance, kind)` and are folded with a coalesced counter. Lifecycle notices bypass both lanes. Sinks that fail delivery are removed (window closed). |
| `session.rs` | `BridgeState`: bootstrap/reconnect state machine, event pump, request validation/stamping/routing |
| `editor.rs` | UTF-16↔UTF-8 map re-export and camelCase document-event JSON pins (Phase 5) |

## Session semantics

- **One live session.** `bootstrap()` is idempotent while connected (cached
  snapshot). A concurrent bootstrap returns `busy`.
- **Reconnect** aborts the old pump *before* the new handshake, so stale
  stream data from a dead connection structurally cannot reach the webview.
  Generation increments per session and is echoed in the bootstrap.
- **Adoption**: if a protocol-compatible server already listens on the
  endpoint (handshake probe), the supervisor reports `Connected` without
  spawning (`pid: null`) and the bridge talks to that instance; incompatible
  listeners are refused with a typed reason instead of being adopted.
- **Tab reclaim**: the pump records our `(tab_id, workspace_root)` from
  `TabRegistry` events; reconnect uses `connect_for_reclaim_or_new`.

## Request path

Frontend sends raw JSON text of a protocol `ClientMessage`. The bridge:

1. rejects bodies over `MAX_REQUEST_BYTES`;
2. parses strictly (serde errors → sanitized `invalidRequest`);
3. rejects `Hello` (handshake is bridge-owned);
4. routes `Edit` through `ClientEditQueue::enqueue_edit_event` so optimistic
   version bookkeeping runs;
5. stamps every other variant's `client_id` over whatever the caller supplied
   (`stamp_client_id`, exhaustive - new variants fail compilation).

Protocol v27 document loading uses the same path: `DocumentChunkRequest` is identity-stamped before enqueue, while `DocumentChunk` and `DocumentChunkRejected` return as typed `ClientConnectionEvent` values. The bridge never interprets rope offsets or completion; it only projects validated camelCase DTOs.

Responses arrive asynchronously as envelope events; slow provider lanes
self-stale-drop server-side.

## JSON surface

Every protocol type carries blanket serde derives (added alongside rkyv,
single semantic source). Runtime-generation snapshots are intercepted before
forwarding: Rust validates the generation, resolves theme data, parses bounded
package component JSON into inert values, and preserves client/tab routing.
Raw theme overrides and raw component strings never enter the webview.
Envelope enums are adjacently tagged
(`{"family":...,"payload":...}` / `{"kind":...,"data":...}`) with camelCase names;
unit enums serialize as plain strings. `DocumentTextHead` projects as `{ totalBytes, firstChunk }`; chunk and typed rejection fields preserve document ID, document version, and UTF-8 byte offset. Menu session ids cross as strings via
`menu_session_id_serde` because they carry the server high bit (`1 << 63`)
and exceed JavaScript's safe integers; other ids are sequential counters.

## Tests

- `tests/dto_roundtrips.rs`: JSON round trip for every `ClientMessage`
  variant and constructible `ServerMessage` families; exhaustive family
  matchers make adding a variant a compile error until samples are updated;
  menu-id string assertion; theme/typography/tab-registry round trips.
- `tests/bridge_session.rs`: real-server end-to-end — bootstrap fields,
  TabRegistry delivery, typed `TabCommand::New` round trip (registry revision
  bump), disconnect notice on server death, reconnect with generation 2 and a
  fresh identity. Multi-thread tokio flavor (the pump starves on the
  single-thread one).
- `forwarder.rs` unit tests: latest-wins coalescing, live-lane ordering,
  disconnected bypass, distinct keys never fold together.

## Authority boundaries

The webview never sees archive bytes, frame codecs, sockets, or protocol
versions, cannot spawn processes (server spawn is desktop-Rust authority only),
and cannot forge its identity or protocol version — both are stamped or
rejected in Rust before anything reaches the server.
