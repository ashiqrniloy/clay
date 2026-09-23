# TauRPC / Tauri IPC Spike — Plan 097 Phase 3 decision record

Date: 2026-08-23
Status: **TauRPC not adopted; native commands + serde-derived protocol types selected.**

## What was evaluated

| Option | Versions checked | Outcome |
| --- | --- | --- |
| TauRPC | `taurpc` 2.0.0 (specta v2 rewrite, user-driven exporter, `ErrorHandlingMode::Result`) | Rejected |
| Native Tauri commands + `tauri::ipc::Channel` | tauri 2.11.5, `@tauri-apps/api` 2.11.1 | Selected |

## Why TauRPC was rejected

1. **No net code reduction.** Our wire surface is a single internally tagged
   pair of enums (`ClientMessage`/`ServerMessage`, ~66 variants) that already
   round-trip through serde with zero per-family glue. TauRPC's trait-based
   procedure definitions would restate the same families a second time.
2. **Extra moving parts for equivalent guarantees.** The exporter step,
   specta version pinning, and tsup-generated TS bindings add build
   reproducibility and maintenance risk without removing any hand-written
   code: our hand-maintained `frontend/src/bridge/types.ts` is smaller than
   generated output would be, because the shell consumes only the bootstrap +
   envelope shapes today.
3. **Channel semantics we need are native.** Bounded ordered delivery with
   latest-wins coalescing lives in our Rust `Forwarder`; TauRPC channels would
   sit underneath it anyway.

## What the bridge does instead

- Blanket serde derives on every `src/protocol` type (165 container attrs)
  plus adjacently tagged camelCase envelopes: one semantic definition, two
  encodings (`rkyv` server-side, JSON across IPC). Verified by exhaustive
  family round-trip tests.
- IDs: menu session ids (`1 << 63` partition) serialize as strings via
  `menu_session_id_serde`; counter ids stay numbers by construction.
- Requests are size-capped raw JSON parsed in Rust; identity/protocol version
  are stamped or rejected bridge-side (`Hello` forbidden).

## Exact pins recorded

- `tauri = "2"` (resolved 2.11.5), `tauri-build = "2"` (2.6.3)
- `@tauri-apps/api ^2.11.1`
- `serde = { version = "1", features = ["derive"] }` added to root crate for
  protocol derives; `serde_json` already present

Revisit trigger: if Phase 4+ React surfaces need generated TS for the full
manifest tree, prefer adding specta derives to protocol types over adopting
the TauRPC layer.
