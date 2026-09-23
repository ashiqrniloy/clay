# Desktop Typed Bridge (Tauri v2)

## Source

- `src-tauri/src/bridge/dto.rs` — typed bootstrap, runtime/theme DTOs, envelope projection, and the **single hand-written definition of the webview contract** (the contract types derive `TS` under the codegen feature).
- `frontend/src/bridge/generated/` — generated TypeScript for that contract (checked in; `scripts/check-bindings.sh` fails on drift).
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
| `dto.rs`       | `BootstrapDto`, runtime/theme/typography snapshots, `DesignSystemSnapshotDto`/`DesignSystemProvenance` (Plan 102), and `BridgeEnvelope`.                                                    |
| `errors.rs`    | Sanitized `{ code, message }` errors with `MAX_REQUEST_BYTES = 512 KiB` request protection.                          |
| `session.rs`   | Single live client session, handshake/reconnect generations, request parsing, client-id stamping, and event pumping. |
| `forwarder.rs` | Live FIFO lane (capacity 512), whole viewport-patch latest-wins slots, lifecycle bypass, and delivery metrics.       |
| `editor.rs`    | Renderer-neutral editor conversion helpers and camelCase shape pins.                                                 |

The bridge does not own document text, parser state, syntax executors, render
fields, request completion, package execution, or filesystem authority.

## Generated webview contract (Plan 119 SC-1)

`dto.rs` plus `errors.rs` are the only hand-written definition of what the
webview receives. `#[derive(ts_rs::TS)]` on those types (gated by the
`ts-bindings` feature) generates `frontend/src/bridge/generated/bridge.ts` and
`…/serde_json/JsonValue.ts`; nothing in the frontend restates a contract shape.

- **Codegen:** `cargo test -p clay-desktop --features ts-bindings --lib export_webview_contract_bindings` (the export test lives in `dto.rs`'s `runtime_projection_tests`). `scripts/check-bindings.sh` = regenerate + fail on any change; `scripts/check.sh full` runs it last.
- **Feature:** `ts-bindings` is off for shipped builds and for the frontend toolchain. It enables ts-rs derives on the serde-JSON types the DTO layer is composed of (protocol/editor/shell projection types); the rkyv Rust-to-Rust wire is untouched either way.
- **Excluded on purpose:** `BridgeEnvelope`'s `event`/`routed` variants are `ts(skip)`-ed. Generating the full `ClientConnectionEvent` union would pull the internal event graph (server messages, agent frames, viewport patches, completion sets) into the contract; `frontend/src/bridge/types.ts` narrows those to `ShellEvent` + the families the shell consumes, with an opaque catch-all for the rest, and composes the envelope as `generated ∪ {event,routed}`.
- **64-bit ids:** generated 64-bit integers are `number` (`Config::with_large_int`), matching the wire; ids that can exceed the safe-integer range cross as strings (menu session ids).

### One projection per family (Plan 132 U1)

Plan 132 removed the last hand-copied DTO structs. A family is now written
**once**, in whichever of two shapes fits it:

| Family shape | Mechanism | Examples |
| --- | --- |
| Identical to the source type | the DTO *is* the source type; one newtype at most | `TypographySnapshotDto` = `ActiveTypography` (a single-field tuple struct, so serde and ts-rs both treat it as the source type and the generated TS keeps the contract name), `DesignSystemProvenance`, `PackageUiProvenance`, `FontProfile`, `UiTypographyHierarchy` |
| Narrowing or renamed | one `From`/`TryFrom` impl whose destructure names **every** source field (no `..`), so adding, renaming or removing a source field breaks the build at the mapping | `ComponentRecipeDto`, `ShadowLayerDto` (drops `inset`, unwraps `colorRole` to a plain string), `InnerHighlightDto`, `IconGeometryDto`/`IconPathDto` (`f32`→`f64`, SVG `d` via `IconPath::to_d`), `InitialDocumentDto` (`document_version`→`version`), the package-UI surface DTOs |

Consequences worth knowing before touching a family:

- Adding a `#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]` +
  `ts(export_to = "bridge.ts")` to a core type is the established way to stop
  copying it (Plan 119 added 71 such derives; Plan 132 added two more —
  `ActiveTypography` and `DesignSystemProvenance`).
- Colour-denial validation stays a separate pass *before* projection, because a
  `From<&ShadowLayer>` impl cannot fail; the error strings
  (`color denial: invalid <field> …`) are unchanged.
- The design-system variables table is generated from the **projected** recipe
  DTO, not from the source recipe, so the wire contract has exactly one author.
- **Three drift guards:** exhaustive destructuring (compile time), a wire-key-set
  comparison against the source type in `dto_roundtrips.rs` (catches a serde
  rename, which destructuring cannot see), and `scripts/check-bindings.sh`
  (regenerate + git-diff the generated TS).
- **The projection boundary did not move.** Every rewrite replaced a struct
  literal with a projection of the same source type: no field was added to the
  wire, the deliberate narrowings stay (`ShadowLayer.inset` and
  `EmptyTabContent.package_name` are still dropped, raw theme overrides still
  never cross, package UI is still parsed Rust-side, identity is still stamped
  Rust-side), and the two new `ts_rs::TS` derives are behind the `ts-bindings`
  feature, so shipped builds pay nothing at runtime.

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
   Since Plan 105 the two dispatch families live beside the controller:
   `workspace-envelope.ts` (`handleEnvelope` over `EnvelopeContext`) owns the
   server-event lanes and `workspace-commands.ts` (`dispatchClientCommand` over
   `CommandContext`) owns routed client commands; the controller wires their
   context adapters and keeps tab/pane state.

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

## Design-system snapshot DTO (Plan 102)

The runtime snapshot carries `activeDesignSystem: DesignSystemSnapshotDto` —
specifier, `schemaVersion`, `generation`, `provenance`, and the resolved
recipe table projected to bounded camelCase variables.
`DesignSystemProvenance` is the exact trust-domain record:
`packageName`, `packageVersion`, `apiPrefix`, `trustDomain` — the source shell
type itself since Plan 132 U1, so it cannot drift from the Rust value.

- **Deny fields:** package source paths, entry points, manifest JSON,
  approval state, and any recipe text beyond the validated bounded values are
  never projected; the webview receives finished variables, not declarations.
- **Authority:** provenance is informational — it grants the webview nothing
  and is not accepted back as request input.
- **Bounds:** the full core recipe table (30+ recipes, 300+ variables) must
  serialize under 128 KiB; `dto_roundtrips.rs` fails the build budget if the
  projection grows past it.
- `BootstrapDto` carries the same shape (`ActiveDesignSystem::core_fallback`
  is the default when no package design system is active), so reconnects and
  cold boots see one coherent type.

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
  event/message shape coverage, including design-system snapshot/provenance
  round trips, authority/size pins, the 128 KiB serialized-snapshot ceiling
  (`runtime_snapshot_dto_with_active_design_system_round_trip`,
  `design_system_snapshot_dto_round_trip_and_variables`), the projected
  recipe/shadow/inner-highlight **wire-key-set** drift check inside that test,
  and `typography_dto_is_a_transparent_projection_of_the_protocol_type`
  (serializing the DTO equals serializing `ActiveTypography`).
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
cargo test --test runtime editor_performance_
cd frontend && bun run test src/test/bridge.test.ts
```

## Related

- [React Client Bridge](react-client-bridge.md)
- [Server State Fanout Lanes](server-state-fanout.md) — the live server lanes whose values reach the webview through this bridge.
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- [Protocol Codec](protocol-codec.md)
- [Syntax Sessions](syntax-sessions.md)
- [Tauri Desktop Shell](tauri-desktop-shell.md)
- `src-tauri/src/bridge/forwarder.rs`
- `src-tauri/src/bridge/session.rs`
