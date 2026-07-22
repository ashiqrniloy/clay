# Decoration Transport

## Source

- `src/protocol/decorations.rs`
- `src/server/decorations.rs`
- `src/server/ops/decorations.rs`
- `runtime/js/decorations.js`
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

- Define `DecorationSet`, `DecorationChunkKey`, `DecorationSpan`, `DecorationKind`, and `DecorationProvenance` as protocol-owned `rkyv` message types; set-level package/layer identity keeps empty replacement chunks authoritative.
- Validate decoration publications against `DECORATION_PAYLOAD_BUDGET_BYTES`, stale document versions, viewport/chunk-bounded byte ranges, known inert style tokens/kinds, and package provenance.
- Retain syntax/decor chunks in a generic `SyntaxChunkCache` with package/range/layer keys, LRU eviction, near-viewport pruning, and the 30 MiB `SYNTAX_CACHE_BUDGET_BYTES` large-file budget.
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
4. Validation rejects unknown style tokens/kinds, rejects spans outside the declared viewport, sorts spans viewport-first by intersection, priority, and byte range, then serializes the set with `rkyv::to_bytes` to enforce `DECORATION_PAYLOAD_BUDGET_BYTES` before sending. Tree-sitter syntax handlers convert affected (changed+invalidated) ranges into a shared 128-byte UTF-8-safe replacement-chunk grid via `replacement_ranges`, query the full envelope covering every touched chunk, clip captures at exact chunk boundaries, and construct complete output ranges from the same grid — so every published chunk carries complete authoritative capture state for exactly the range it replaces. Chunking therefore bounds payloads without truncating dense windows or multiplying parser/query invocations.
5. The protocol sends validated data as `ServerMessage::DecorationSet(DecorationSet)` through the existing codec; there is no decoration-specific serialization side channel. Plan 059 task 6 (protocol version 5) adds `ServerMessage::DecorationBatch(Vec<DecorationSet>)`: when one parse update produces more than one 128-byte authority chunk, the connection ships all chunks in a single frame in viewport-key order instead of fanning out per-chunk frames. Single-chunk updates keep the plain `DecorationSet` wire shape. Batch validation is unchanged — each member set is validated before publication, and the whole batch still fits the 1 MiB frame ceiling because every chunk already respects `DECORATION_PAYLOAD_BUDGET_BYTES` and a parse window is budget-capped.
6. The client connection task converts the message into `ClientConnectionEvent::DecorationSet` or a single `ClientConnectionEvent::DecorationBatch`.
7. `EditorWidget::apply_connection_event` installs the chunk through `EditorSurface::apply_decoration_set`, which rejects mismatched document IDs or versions. A batch applies every chunk in order through the same per-set path (no short-circuit when an earlier chunk changes state), so staleness rejection and Plan 058 exact-range replacement semantics are identical to sequential single-set application.
8. A changed visible byte range enqueues a deduplicated `DecorationViewportRequest` through the existing nonblocking client queue only when doing so leaves one slot for user intent. Both pointer scrolling and local keyboard commands enqueue through this path; caret-following ArrowDown/ArrowUp movement must request newly visible chunks just like touchpad scrolling. The server validates client/document/version/range metadata, caps query/decor authority to `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` (4 KiB), and prepares UTF-8-safe parser context within the grammar's `ParsePolicy`. Guard bytes are now applied instead of discarded. If the full document fits the grammar policy, the parser window is the full document while the notification viewport remains bounded; this is how Markdown preserves fenced-block context without enlarging decoration output. Same-version requests reuse the cached tree and rerun only the viewport query. The resulting `IncrementalParseUpdate::decoration_updates` batch is fully validated before publication, then the connection ships the members as one `ServerMessage::DecorationBatch` (or a single `DecorationSet` when only one chunk changed).
9. `EditorSurface` stores validated chunks by `DecorationChunkKey`. During optimistic edits it transforms only retained near-viewport inert spans: unaffected geometry shifts, strict-interior syntax edits resize provisionally, narrow syntax (Keyword, Function, Type, Variable, Number) extends at token end only when every inserted character is a Unicode word character or `_` (same-word inheritance), broad `TokenType` families (Comment, String, Heading1-6, Quote, CodeBlock, CodeSpan, Link, Paragraph) inherit edge insertions unconditionally, syntax deletion/replacement keeps surviving geometry, and intersecting semantic/diagnostic/search spans invalidate rather than claim provisional authority. Transformed keys advance with edit acknowledgements. A current server set replaces only its exact half-open viewport: `EditorDecorationState::apply_set` subtracts that range from overlapping provisional chunks of the same package/layer, preserves left/right span fragments outside authority, and locally coalesces compatible residual chunks/spans. Empty sets therefore clear exactly their declared viewport without erasing shifted neighboring syntax or semantic chunks. Resync/document replacement still clears all state. Individual spans are intersected with the visible snapshot before local offset subtraction, preventing stale earlier spans in overlapping chunks from underflowing during scroll.
10. When the cached Parley layout misses, `EditorSurface` intersects cached local chunks with the current `VisibleSnapshot`, rejects malformed/out-of-document/non-UTF-8-boundary ranges again, maps known kind/style tokens to local Rust colors and attributes, then normalizes non-overlapping presentation runs. Font roles and foreground colors are considered only for `Syntax`/`Semantic`: higher priority wins, then semantic over syntax, then stable provenance; attributes compose. `LayoutState` assigns ranged `BrushIndex` values, caches the corresponding native brush table, and renders glyphs with theme colors instead of background highlight rectangles. Selection and range-diagnostic squiggles remain separate paint layers. Cache-hit paint does not rescan spans.

Phase 18.20 semantic intelligence reuses this path directly. `DecorationSpan::from_vocabulary` and `serverPublishDecorations({ kind: "semantic", tokenType, modifiers })` publish scope-less two-axis spans; legacy `styleToken` input remains compatible. `StyleRegistry` resolves both Syntax and Semantic through the same per-`TokenType` color table, while additive chunks retain syntax beneath semantic refinements. The `language-server` permission does not bypass `render-decorations`.

Phase 18.21 LSP bridge packages publish semantic tokens through the document-analysis worker's output channel. The worker receives LSP `textDocument/semanticTokens` responses (full or delta), converts them to Clay vocabulary via `mapping.js`, and routes the resulting `DecorationSet` through `validate_decoration_publication`. Semantic token publication is background/viewport-bounded work; paint consumes only cached validated inert state.

After each accepted workspace edit, `refresh_native_syntax_after_edit` schedules the existing native syntax handler around the changed byte offset, capped by the grammar's `ParsePolicy.max_window_bytes`. Before that asynchronous result exists, `EditorDecorationState::apply_edit` performs bounded provisional interpolation over retained chunks only. Insert/delete/replace arithmetic is byte-based, overflow-checked, UTF-8 text lengths come from the already accepted local operation, and reversed ranges fail closed. `EditAck` advances both state and chunk-key versions instead of clearing colors. No full-document state, parser, IPC wait, package JavaScript, or language/delimiter branch enters this client path.

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
- Decorations are viewport/chunk-bounded; spans outside the declared viewport are rejected. Opening and deduplicated viewport requests each schedule one bounded parse window; one capture result fans out into stable output chunks without document text crossing from client to server.
- Large-file retained syntax/decor cache memory is bounded by `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB) and off-viewport chunks are evicted once they leave the near-viewport guard.
- Style-token validation is generic and allowlist-based; `markup.*` tokens serve Markdown, while `keyword.control`, `string.quoted`, `comment.line`, and `punctuation.definition` prove that non-Markdown language packages can publish syntax spans through the same primitive.
- A span may select only a closed semantic document role. Packages cannot send family names, sizes, raw Parley properties, CSS, callbacks, or UI roles through decoration transport. The client treats even malformed received data fail-closed: only syntax/semantic roles survive normalization, and only UTF-8-safe in-document ranges reach Parley.

## Tests

- `tests/decoration_transport.rs`: oversized payload rejection, stale-version rejection, invalid range/unknown token rejection, off-viewport rejection, generic non-Markdown language package syntax-span acceptance, representative Markdown decoration payload budget coverage, near-viewport client pruning, stale-version cache clearing, protocol codec round trip, client render-hook application, and optimistic comment continuity through authoritative empty replacement.
- `tests/syntax_grammar.rs`: native Tree-sitter fixtures for Rust, TypeScript, TSX, JavaScript, and Markdown produce bounded vocabulary decorations; tier selection, unmapped captures, complete payload-safe dense fan-out, scroll-sized source output, cached parsing, package provenance, semantic style-map roles, and concrete-font rejection are covered.
- `tests/typography_protocol.rs::decoration_font_role_is_limited_to_syntax_and_semantic_layers`: rejects diagnostic/search font-role transport before publication.
- `src/editor/surface.rs`: verifies UTF-8 interior insertion, broad edge inheritance, narrow edge behavior, syntax delete/replace resizing, exact half-open authoritative subtraction, crossing-span residual splits, local residual coalescing and bounded chunk count, package/layer isolation, reversed edit rejection, resync clearing, edit-ack survival, deterministic overlap composition, scroll safety, and font-role validation; `src/editor/layout.rs` verifies typography/style/default-role cache invalidation.
- `src/masonry_editor.rs::scrolling_enqueues_new_decoration_viewport_once`, `src/masonry_editor.rs::local_commands_request_decorations_for_keyboard_driven_viewport_changes`, `src/client/mod.rs::decoration_viewport_request_emits_bounded_range_metadata`, `src/server/connection.rs::native_windows_schedule_once_for_each_first_party_language`, and `src/server/syntax.rs::same_version_markdown_scroll_reuses_full_document_tree_context` cover deduplicated client requests, bounded query output, full Markdown block context, one cached parse across scroll, correct prose after a closing fence, and multi-chunk output for nonzero native parse windows.
- `src/server/decorations.rs::tests::large_file_decoration_cache_respects_30_mib_budget`: verifies server-side chunk-cache budget accounting and LRU eviction.
- `tests/performance_protocol.rs::decoration_chunk_protocol_payload_stays_bounded_for_large_file_viewport`: verifies chunk IPC payloads remain under the decoration transport budget.
- `tests/editor_performance_invariants.rs::paint_uses_cached_inert_spans_without_package_javascript`: guards paint/layout source against package JavaScript, parser, server, or op calls.
- `src/server/js_runtime.rs::phase18_parse_and_decoration_facades_are_runtime_backed`: controlled-runtime facade/op smoke for the public API.
- Relevant commands: `cargo test large_file_decoration_cache_respects_30_mib_budget --lib`, `cargo test --test editor decoration_transport::`, `cargo test --test protocol performance_protocol::`, `cargo test --test editor editor_performance_invariants::`, and `cargo test phase18_parse_and_decoration_facades_are_runtime_backed --lib`.

## Related

- [First-Party LSP Bridge Packages](first-party-lsp-bridge-packages.md)
- [Protocol Codec](protocol-codec.md)
- [Rendering Primitives](rendering-primitives.md)
- [Range Diagnostics](range-diagnostics.md)
- [Language Intelligence](language-intelligence.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md) — Phase 19 `DocumentRuntimeRenderState` reset flags clear stale generation decoration caches during atomic client install.
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
