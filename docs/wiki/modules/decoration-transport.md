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
- `src/server/syntax.rs`
- `runtime/js/web-tree-sitter-host.ts`
- `tests/syntax_grammar.rs`

## Overview

The decoration transport carries package-produced inline editor decorations as bounded inert data. Phase 18.17 range diagnostics use a parallel `DiagnosticSet` transport and client cache (see [Range Diagnostics](range-diagnostics.md)); they remain an additive paint layer and must not be folded into `DecorationSet` for source-keyed lifecycle. Phase 18.16.5 adds an optional closed document font-role override for syntax/semantic spans; it remains separate from user-owned concrete font profiles. The server validates a `DecorationSet` for document version, viewport/chunk range, payload size, package provenance, known style tokens/kinds, and `render-decorations` permission before it crosses the normal `rkyv` protocol codec. Phase 18 exposes the public `clay.decorations.serverPublishDecorations` facade/op contract so server-side packages can publish validated spans without calling raw Deno ops. Phase 18.18 moves first-party Markdown's default decoration role to native Tree-sitter; its package parser uses this path only as Tier 3 fallback. Phase 18.10 Tree-sitter syntax highlighting established this shared transport; Phase 18.16 reuses it for all syntax-engine tiers: native Tree-sitter and the web-tree-sitter host adapter produce capture records, while the shared mapper emits Phase 18.15 `TokenType` + `Modifiers` spans; Tier 3 package-JavaScript handlers publish through the same validation path. The transport enforces `DECORATION_PAYLOAD_BUDGET_BYTES` before cache insertion/publication, and the parse coordinator publishes only inert updates. Phase 18.5 treats each viewport-bounded `DecorationSet` as a decoration chunk: server/runtime state and the editor retain only visible or near-viewport chunks under `SYNTAX_CACHE_BUDGET_BYTES` while each IPC payload remains under `DECORATION_PAYLOAD_BUDGET_BYTES`. The client stores validated chunks and applies them in the native editor render path without invoking package JavaScript. When scrolling changes the visible byte range, `EditorWidget` sends one deduplicated best-effort `DecorationViewportRequest` while reserving one outbound queue slot for workspace actions, edits, and other user intent; the server schedules the already-registered document-selected native grammar against that bounded nonzero window, so large files gain decoration chunks incrementally instead of relying on the opening 4 KiB window.

## Responsibilities

- Define `DecorationSet`, `DecorationChunkKey`, `DecorationSpan`, `DecorationKind`, and `DecorationProvenance` as protocol-owned `rkyv` message types.
- Validate decoration publications against `DECORATION_PAYLOAD_BUDGET_BYTES`, stale document versions, viewport/chunk-bounded byte ranges, known inert style tokens/kinds, and package provenance.
- Retain syntax/decor chunks in a generic `SyntaxChunkCache` with byte-range keys, LRU eviction, near-viewport pruning, and the 30 MiB `SYNTAX_CACHE_BUDGET_BYTES` large-file budget.
- Accept a small language-neutral style-token allowlist for both Markdown markup scopes and future code modes such as Python (`keyword.control`, `string.quoted`, `comment.line`, `punctuation.definition`) without adding parser-specific Rust branches.
- Provide the runtime-backed `clay:decorations` facade and explicit `op_clay_decorations_publish_decorations` wrapper for package-side publication.
- Route `ServerMessage::DecorationSet` through the client connection event loop into `EditorWidget::apply_connection_event`.
- Store current validated spans in `EditorSurface` and normalize viewport-bounded syntax/semantic presentation runs into cached Parley foreground brushes for `LayoutState::paint_text`.

## Primitive Coverage

- **Decoration publication** — `DecorationSet`/`DecorationSpan` in `src/protocol/decorations.rs` is the reusable inert output primitive for Markdown, native/WASM Tree-sitter, and package-JavaScript parser adapters.
- **Vocabulary/theme boundary** — syntax spans carry `TokenType`, `Modifiers`, optional compatibility `scope`, optional `DocumentFontRole`, and provenance; `StyleRegistry` remains the single color resolver during paint. Typography resolves separately from `ActiveTheme`.
- **Validation/performance** — server validation enforces document/version, viewport, provenance, permission, serialized payload, and `SYNTAX_CACHE_BUDGET_BYTES` limits before publication or cache insertion. Paint consumes cached spans only.
- **Reuse rule** — new modes produce bounded spans through the existing facade or parse handler/coordinator path; they do not add parser callbacks, renderer hooks, raw CSS, client JavaScript, or language-specific paint branches.

## How It Works

1. Package code produces spans server-side, but only the Rust validation path may publish them.
2. Package JavaScript calls `serverPublishDecorations`, which serializes options into `op_clay_decorations_publish_decorations`; the op reconstructs package provenance and rejects missing `render-decorations` permission before publication.
3. `validate_decoration_publication` first checks the package has `PackagePermission::RenderDecorations`, then delegates to `validate_decoration_set` for cheap range/version/style/provenance checks. A non-inherit font role is accepted only on `Syntax` or `Semantic` spans; diagnostic and search spans remain paint-only.
4. Validation rejects unknown style tokens/kinds, rejects spans outside the declared viewport, sorts spans viewport-first by intersection, priority, and byte range, then serializes the set with `rkyv::to_bytes` to enforce `DECORATION_PAYLOAD_BUDGET_BYTES` before sending. Tree-sitter syntax handlers perform the same serialized payload check before inserting a validated syntax set into `SyntaxChunkCache`, so oversized query output fails closed instead of being cached.
5. The protocol sends validated data as `ServerMessage::DecorationSet(DecorationSet)` through the existing codec; there is no decoration-specific serialization side channel.
6. The client connection task converts the message into `ClientConnectionEvent::DecorationSet`.
7. `EditorWidget::apply_connection_event` installs the chunk through `EditorSurface::apply_decoration_set`, which rejects mismatched document IDs or versions.
8. A changed visible byte range enqueues a deduplicated `DecorationViewportRequest` through the existing nonblocking client queue only when doing so leaves one slot for user intent. The server validates client/document/version/range metadata and schedules the registered native handler with a UTF-8-safe window capped by its `ParsePolicy`.
9. `EditorSurface` stores the validated chunk by `DecorationChunkKey`, drops stale chunks on document version changes, prunes chunks outside the current visible/near-viewport guard, and keeps retained serialized chunk memory under `SYNTAX_CACHE_BUDGET_BYTES`. Individual spans are intersected with the visible snapshot before local offset subtraction, preventing stale earlier spans in an overlapping chunk from underflowing during scroll.
10. When the cached Parley layout misses, `EditorSurface` intersects cached local chunks with the current `VisibleSnapshot`, rejects malformed/out-of-document/non-UTF-8-boundary ranges again, maps known kind/style tokens to local Rust colors and attributes, then normalizes non-overlapping presentation runs. Font roles and foreground colors are considered only for `Syntax`/`Semantic`: higher priority wins, then semantic over syntax, then stable provenance; attributes compose. `LayoutState` assigns ranged `BrushIndex` values, caches the corresponding native brush table, and renders glyphs with theme colors instead of background highlight rectangles. Selection and range-diagnostic squiggles remain separate paint layers. Cache-hit paint does not rescan spans.

Phase 18.17 adds a parallel inert path for `ServerMessage::DiagnosticSet` / `ClientConnectionEvent::DiagnosticSet` / `EditorSurface::apply_diagnostic_set`. Source-keyed diagnostic chunks share the connection drain from `ParseCoordinator` updates, use `DIAGNOSTIC_CACHE_BUDGET_BYTES`, and remain independent from decoration chunk lifecycle. See [Phase 18.17 range diagnostics primitive review](phase18.17-range-diagnostics-primitive-review.md).

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
- Decorations are viewport/chunk-bounded; spans outside the declared viewport are rejected. Opening schedules the first bounded window, then deduplicated viewport requests schedule later windows without document text crossing from client to server.
- Large-file retained syntax/decor cache memory is bounded by `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB) and off-viewport chunks are evicted once they leave the near-viewport guard.
- Style-token validation is generic and allowlist-based; `markup.*` tokens serve Markdown, while `keyword.control`, `string.quoted`, `comment.line`, and `punctuation.definition` prove that non-Markdown language packages can publish syntax spans through the same primitive.
- A span may select only a closed semantic document role. Packages cannot send family names, sizes, raw Parley properties, CSS, callbacks, or UI roles through decoration transport. The client treats even malformed received data fail-closed: only syntax/semantic roles survive normalization, and only UTF-8-safe in-document ranges reach Parley.

## Tests

- `tests/decoration_transport.rs`: oversized payload rejection, stale-version rejection, invalid range/unknown token rejection, off-viewport rejection, generic non-Markdown language package syntax-span acceptance, representative Markdown decoration payload budget coverage, near-viewport client pruning, stale-version cache clearing, protocol codec round trip, and client render-hook application.
- `tests/syntax_grammar.rs`: native Tree-sitter fixtures for Rust, TypeScript, TSX, JavaScript, and Markdown produce bounded vocabulary decorations; tier selection, unmapped captures, transport-safe overflow truncation, scroll-sized source degradation, cached parsing, package provenance, semantic style-map roles, and concrete-font rejection are covered.
- `tests/typography_protocol.rs::decoration_font_role_is_limited_to_syntax_and_semantic_layers`: rejects diagnostic/search font-role transport before publication.
- `src/editor/surface.rs`: verifies Markdown code uses monospace inside a proportional document, overlap resolution/attribute composition is deterministic, stale earlier spans cannot underflow after scrolling, and diagnostic or invalid UTF-8 spans cannot affect font roles; `src/editor/layout.rs` verifies typography/style/default-role cache invalidation.
- `src/masonry_editor.rs::scrolling_enqueues_new_decoration_viewport_once`, `src/client/mod.rs::decoration_viewport_request_emits_bounded_range_metadata`, and `src/server/connection.rs::nonzero_viewports_produce_typescript_and_markdown_decorations` cover deduplicated client requests and nonzero native parse windows.
- `src/server/decorations.rs::tests::large_file_decoration_cache_respects_30_mib_budget`: verifies server-side chunk-cache budget accounting and LRU eviction.
- `tests/performance_protocol.rs::decoration_chunk_protocol_payload_stays_bounded_for_large_file_viewport`: verifies chunk IPC payloads remain under the decoration transport budget.
- `tests/editor_performance_invariants.rs::paint_uses_cached_inert_spans_without_package_javascript`: guards paint/layout source against package JavaScript, parser, server, or op calls.
- `src/server/js_runtime.rs::phase18_parse_and_decoration_facades_are_runtime_backed`: controlled-runtime facade/op smoke for the public API.
- Relevant commands: `cargo test large_file_decoration_cache_respects_30_mib_budget --lib`, `cargo test --test decoration_transport`, `cargo test --test performance_protocol`, `cargo test --test editor_performance_invariants`, and `cargo test phase18_parse_and_decoration_facades_are_runtime_backed --lib`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Rendering Primitives](rendering-primitives.md)
- [Range Diagnostics](range-diagnostics.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
