# Decoration Transport

## Source

- `src/protocol/decorations.rs`
- `src/server/decorations.rs`
- `src/server/ops/decorations.rs`
- `runtime/js/decorations.ts`
- `src/client/mod.rs`
- `src/editor/surface.rs`
- `src/editor/layout.rs`
- `src/masonry_editor.rs`
- `tests/decoration_transport.rs`
- `packages/markdown/dist/parser.js`

## Overview

The decoration transport carries package-produced inline editor decorations as bounded inert data. The server validates a `DecorationSet` for document version, viewport/chunk range, payload size, package provenance, known style tokens/kinds, and `render-decorations` permission before it crosses the normal `rkyv` protocol codec. Phase 18 exposes the public `clay.decorations.serverPublishDecorations` facade/op contract so server-side packages can publish validated spans without calling raw Deno ops. The first-party Markdown package now uses this path from its parser adapter for heading, emphasis, code, fence, and list-marker spans. Phase 18.5 treats each viewport-bounded `DecorationSet` as a decoration chunk: server/runtime state and the editor retain only visible or near-viewport chunks under `SYNTAX_CACHE_BUDGET_BYTES` while each IPC payload remains under `DECORATION_PAYLOAD_BUDGET_BYTES`. The client stores validated chunks and applies them in the native editor render path without invoking package JavaScript.

## Responsibilities

- Define `DecorationSet`, `DecorationChunkKey`, `DecorationSpan`, `DecorationKind`, and `DecorationProvenance` as protocol-owned `rkyv` message types.
- Validate decoration publications against `DECORATION_PAYLOAD_BUDGET_BYTES`, stale document versions, viewport/chunk-bounded byte ranges, known inert style tokens/kinds, and package provenance.
- Retain syntax/decor chunks in a generic `SyntaxChunkCache` with byte-range keys, LRU eviction, near-viewport pruning, and the 30 MiB `SYNTAX_CACHE_BUDGET_BYTES` large-file budget.
- Accept a small language-neutral style-token allowlist for both Markdown markup scopes and future code modes such as Python (`keyword.control`, `string.quoted`, `comment.line`, `punctuation.definition`) without adding parser-specific Rust branches.
- Provide the runtime-backed `clay:decorations` facade and explicit `op_clay_decorations_publish_decorations` wrapper for package-side publication.
- Route `ServerMessage::DecorationSet` through the client connection event loop into `EditorWidget::apply_connection_event`.
- Store current validated spans in `EditorSurface` and render native highlight rectangles via `LayoutState::paint_text`.

## How It Works

1. Package code produces spans server-side, but only the Rust validation path may publish them.
2. Package JavaScript calls `serverPublishDecorations`, which serializes options into `op_clay_decorations_publish_decorations`; the op reconstructs package provenance and rejects missing `render-decorations` permission before publication.
3. `validate_decoration_publication` first checks the package has `PackagePermission::RenderDecorations`, then delegates to `validate_decoration_set` for cheap range/version/style/provenance checks.
4. Validation rejects unknown style tokens/kinds, rejects spans outside the declared viewport, sorts spans viewport-first by intersection, priority, and byte range, then serializes the set with `rkyv::to_bytes` to enforce `DECORATION_PAYLOAD_BUDGET_BYTES` before sending.
5. The protocol sends validated data as `ServerMessage::DecorationSet(DecorationSet)` through the existing codec; there is no decoration-specific serialization side channel.
6. The client connection task converts the message into `ClientConnectionEvent::DecorationSet`.
7. `EditorWidget::apply_connection_event` installs the chunk through `EditorSurface::apply_decoration_set`, which rejects mismatched document IDs or versions.
8. `EditorSurface` stores the validated chunk by `DecorationChunkKey`, drops stale chunks on document version changes, prunes chunks outside the current visible/near-viewport guard, and keeps retained serialized chunk memory under `SYNTAX_CACHE_BUDGET_BYTES`.
9. During paint, `EditorSurface` intersects cached local chunks with the current `VisibleSnapshot`, maps known kind/style tokens to local Rust colors, and asks `LayoutState` to fill highlight rectangles before text rendering.

## Code Examples

```rust
let set = validate_decoration_publication(&package, current_version, decoration_set)?;
syntax_chunk_cache.insert_validated_set("markdown", set.clone())?;
let message = ServerMessage::DecorationSet(set);
```

## Invariants and Constraints

- Decoration spans are inert data only: no draw callbacks, native handles, CSS/script snippets, raw ops, or client JavaScript.
- Stale document versions are rejected on the server and again ignored by the client render hook.
- Ordinary typing, text events, and paint do not run package JavaScript or perform server validation; paint consumes already-applied local state.
- Decorations are viewport/chunk-bounded; spans outside the declared viewport are rejected.
- Large-file retained syntax/decor cache memory is bounded by `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB) and off-viewport chunks are evicted once they leave the near-viewport guard.
- Style-token validation is generic and allowlist-based; `markup.*` tokens serve Markdown, while `keyword.control`, `string.quoted`, `comment.line`, and `punctuation.definition` prove that non-Markdown language packages can publish syntax spans through the same primitive.

## Tests

- `tests/decoration_transport.rs`: oversized payload rejection, stale-version rejection, invalid range/unknown token rejection, off-viewport rejection, generic non-Markdown language package syntax-span acceptance, representative Markdown decoration payload budget coverage, near-viewport client pruning, stale-version cache clearing, protocol codec round trip, and client render-hook application.
- `src/server/decorations.rs::tests::large_file_decoration_cache_respects_30_mib_budget`: verifies server-side chunk-cache budget accounting and LRU eviction.
- `tests/performance_protocol.rs::decoration_chunk_protocol_payload_stays_bounded_for_large_file_viewport`: verifies chunk IPC payloads remain under the decoration transport budget.
- `tests/editor_performance_invariants.rs::paint_uses_cached_inert_spans_without_package_javascript`: guards paint/layout source against package JavaScript, parser, server, or op calls.
- `src/server/js_runtime.rs::phase18_parse_and_decoration_facades_are_runtime_backed`: controlled-runtime facade/op smoke for the public API.
- Relevant commands: `cargo test large_file_decoration_cache_respects_30_mib_budget --lib`, `cargo test --test decoration_transport`, `cargo test --test performance_protocol`, `cargo test --test editor_performance_invariants`, and `cargo test phase18_parse_and_decoration_facades_are_runtime_backed --lib`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Rendering Primitives](rendering-primitives.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
