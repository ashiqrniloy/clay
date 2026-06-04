# Phase 18.5 Large-File Markdown Primitive Review

## Source

- `plans/021-Phase18.5-Large-File-Markdown-Performance-and-Memory.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/rendering-primitives.md`
- `docs/wiki/modules/first-party-markdown-package.md`
- `src/server/parse_coordinator.rs`
- `src/protocol/parse.rs`
- `src/server/document.rs`
- `src/protocol/decorations.rs`
- `src/server/decorations.rs`
- `src/editor/viewport.rs`
- `src/editor/surface.rs`
- `src/perf/budgets.rs`

## Overview

This review completes the primitive-first checkpoint before Phase 18.5 large-file Markdown implementation. The goal is not to add Markdown-specific Rust behavior. The goal is to identify which existing language-neutral primitives already support large-file mode behavior and which generic parse/cache primitives are missing for any future large-file mode.

Large-file Markdown must continue to run parser-specific logic inside `@clay/markdown` package JavaScript. Rust may schedule, snapshot, validate, cache, budget, and publish generic parse/decor data, but it must not branch on Markdown syntax, markdown-it token names, headings, lists, fences, or package-specific parser concepts.

## Existing Primitive Inventory

| Primitive area | Current source paths | What works today | Large-file gap | Security / hot-path boundary |
| --- | --- | --- | --- | --- |
| Package loading, permissions, and provenance | `src/packages/manifest.rs`, `src/packages/record.rs`, `src/packages/service.rs`, `docs/reference/primitives/package-security.md` | Validates package identity, `apiPrefix`, declared permissions, docs path, mode declarations, API dependencies, and contribution payload estimates before load/enable. | No large-file parse/cache gap; package metadata can declare fixed defaults or future generic configuration keys. | Load/enable/configuration only; preserves `parse-document`, `render-decorations`, package-prefix, raw-op, client-JS, filesystem, network, shell, AI, and WASM prohibitions. |
| Document classification and major-mode activation | `src/packages/modes.rs`, `src/server/ops/modes.rs`, `runtime/js/modes.ts` | Static extension/MIME/file-name classification and one-major-mode activation publish generic behavior manifests with package provenance. | No parse/cache gap; activation can choose a generic parse policy later without mode-specific Rust branches. | Open/reload/configuration only; no keypress, paint, scroll, layout, or text-event package work. |
| Behavior manifest and text transforms | `src/behavior/manifest.rs`, `src/packages/commands.rs`, `src/protocol/mod.rs`, `src/editor/surface.rs` | Commands, key routing, pair rules, and declared enter-rule data are inert manifest primitives. | Not the large-file parse/cache bottleneck; future list/fence editing engines must remain generic if implemented. | `ClientFirstPredictable` behavior uses Rust-known rules only; no parser JavaScript before local paint. |
| Parse coordinator and parse protocol | `src/server/parse_coordinator.rs`, `src/protocol/parse.rs`, `src/server/ops/parse.rs`, `runtime/js/parse.ts` | Registers permission-gated handlers, validates viewport/invalidated ranges, schedules background tasks, aborts superseded tasks, rejects stale versions, and enforces `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`. | `ParseEditNotification` carries metadata only; it does not deliver bounded text. There is no `ParseWindowSnapshot`, no server-canonical range snapshot helper, no guard-range policy, no per-window memory budget metadata, and no retained syntax cache accounting. | Requires `parse-document`; background only; current registration rejects executable callback fields and timeout abuse. |
| Server document storage | `src/server/document.rs` | Server owns canonical `crop::Rope`, versions, edit validation, leases, region locks, full initial/resync snapshots, and UTF-8 boundary checks. | Existing public/internal helpers expose full-document strings for snapshots and tests, but not validated bounded range snapshots or line-start metadata for package parser windows. | Server owns canonical content; any future range snapshot must expose only already-open document text inside validated byte ranges. |
| Viewport primitives | `src/editor/viewport.rs`, `src/editor/surface.rs`, `src/client/mod.rs` | Client owns line-based visible range and extracts visible text locally; parse scheduling accepts byte viewport metadata. | There is no explicit generic server-side viewport report/update primitive that converts the current client visible range into a versioned byte range for parse-window scheduling and near-viewport prefetch. | Client still owns immediate paint; server viewport metadata is advisory background scheduling input only. |
| Decoration transport and rendering | `src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/server/ops/decorations.rs`, `runtime/js/decorations.ts`, `src/editor/surface.rs`, `src/editor/layout.rs` | Validates inert viewport-bounded `DecorationSet` payloads for current document version, package provenance, known style tokens, range shape, and `DECORATION_PAYLOAD_BUDGET_BYTES`; client paints already-applied spans locally. | Current `EditorSurface::apply_decoration_set` replaces one span set for the document/version. There is no generic chunk key, no partial chunk invalidation, no LRU chunk cache, and no retained decoration/syntax memory accounting. | Requires `render-decorations`; paint consumes local inert spans only and does not call server, parser, package JS, or validation. |
| SDUI status | `src/protocol/sdui.rs`, `src/server/sdui.rs`, `runtime/js/sdui.ts`, `packages/markdown/dist/sdui.js` | Publishes inert status/preview trees and command-targeting actions through bounded server-validated SDUI. | Status can report fixed/windowed/degraded policy later; no parser/cache primitive gap. | SDUI updates are out-of-band and sanitized; no document text, raw paths, executable callbacks, or authority grants. |
| Configuration surfaces | `runtime/js/configuration.ts`, `src/server/configuration.rs`, `docs/reference/primitives/registry.md` | `~/.config/clay/init.js` and planned `setParsePolicy`/`setPackageOption` surfaces define the route for user-visible settings. | Concrete large-file thresholds, parse-window sizes, parser timeouts, and cache budgets need either fixed documented defaults or bounded generic configuration APIs with `custom_properties`. | Configuration cannot grant package enable/disable, filesystem, network, shell, AI, raw ops, workspace mutation, or client-side JavaScript. |
| Benchmark and budget primitives | `src/perf/budgets.rs`, `docs/development/performance.md`, `docs/wiki/modules/performance-fixtures.md`, `tests/performance_budgets.rs` | Existing payload and latency constants cover edit, decoration, parse-result, SDUI, local paint, and scroll-adjacent budgets; Phase 18.5 docs define Markdown-specific overhead accounting. | There is no generic `SYNTAX_CACHE_BUDGET_BYTES`/syntax-overhead budget constant, no parse-window retained-memory accounting type, and no deterministic cache accounting test yet. | Benchmarking must stay local/sanitized and must not print document contents, secrets, absolute user paths, or package JavaScript source bodies. |

## What Existing Primitives Can Achieve

Existing primitives are enough to keep large-file work out of client hot paths:

- Package load, mode activation, command registration, parse handler registration, and decoration publication are already load/background-time surfaces.
- The parse coordinator can cancel stale background work and prioritize invalidated byte ranges that intersect a viewport range.
- The decoration validator can reject stale, off-viewport, malformed, oversized, or provenance-mismatched decoration payloads.
- The client can paint validated local decorations without invoking package JavaScript, IPC, server validation, or parser work.
- Server document state already enforces UTF-8 byte boundaries and document versions, which future range snapshots should reuse.

These primitives are not enough to meet Phase 18.5 large-file memory targets by themselves because the current parser path still needs a bounded source-delivery primitive and the current decoration state has no chunk/cache memory policy.

## Generic Large-File Primitive Gaps

The implementation should add only language-neutral primitives that future large-file modes can reuse:

1. **`ParseWindowSnapshot` / `ParseRangeSnapshot` primitive**
   - Provide bounded server-canonical text slices with `document_id`, `document_version`, `byte_start`, `byte_end`, `base_line`, and optional line-start metadata.
   - Validate UTF-8 boundaries and requested byte ranges before copying.
   - Derive windows from viewport and invalidated ranges with generic guard lines/guard bytes.
   - Keep source delivery under `parse-document`; package JavaScript receives only the bounded window text, never the ordinary full document for large-file edits.

2. **`ParseWindowRequest` / `ParsePolicy` primitive**
   - Carry `viewport`, `invalidated_ranges`, `window`, `parse_unit`, `timeout_ms`, `memory_budget_bytes`, and package/mode provenance.
   - Preserve cancellation/generation behavior already present in the parse coordinator.
   - Use generic small/medium/large policy thresholds or package-owned fixed defaults; user-visible settings must go through Clay JS configuration APIs if exposed.

3. **`SyntaxCacheBudget` / memory accounting primitive**
   - Define a generic retained syntax/decor/cache budget, targeting 30 MiB for large Markdown workflows in this plan.
   - Separate total RSS, runtime baseline, canonical document memory, temporary parser allocations, and retained syntax/decoration cache memory.
   - Add deterministic accounting tests that fail when retained chunks exceed budget without relying on machine-specific RSS.

4. **`DecorationChunk` / `SyntaxChunkCache` primitive**
   - Store validated decorations by document/version/package/range chunk instead of one full-document set.
   - Support chunk invalidation after edits, stale-version rejection, LRU eviction, memory accounting, and viewport/near-viewport publication.
   - Keep cached values inert: no callbacks, raw styles, parser tokens, AST nodes, or executable data.

5. **ViewportRangeReport primitive**
   - Report client visible byte ranges and near-viewport guard ranges to the server as scheduling hints.
   - Keep the client authoritative for immediate visible text and local paint; reports must not introduce a synchronous scroll/paint round trip.

## Rejected Markdown-Specific Rust Work

Do not add Rust types, functions, enum variants, source branches, cache keys, budget constants, or renderer paths named for Markdown parser syntax. Rejected names and branch markers include `MarkdownParser`, `MarkdownHeading`, `MarkdownFence`, `MarkdownList`, `MarkdownItToken`, `heading_open`, `list_item_open`, `fence`, `strong_open`, `em_open`, `code_inline`, `if mode == "markdown"`, and `if mode_id == "markdown"`.

Acceptable names are reusable primitive names such as `ParseWindowSnapshot`, `ParseRangeSnapshot`, `ParseWindowRequest`, `ParsePolicy`, `SyntaxCacheBudget`, `SyntaxChunkCache`, `DecorationChunk`, `RangeSnapshot`, `LineIndex`, and `ViewportRangeReport`.

Markdown-specific handling remains in `packages/markdown/dist/parser.js` and `packages/markdown/src/parser.js`, where the package may call `markdownIt.parse(windowText, {})`, build package-owned source indexes, and translate token-derived ranges to absolute Clay byte ranges.

## Planned Implementation Order

1. Add generic parse-window snapshot and parse-policy support around `src/protocol/parse.rs`, `src/server/parse_coordinator.rs`, `src/server/document.rs`, `src/server/ops/parse.rs`, and `runtime/js/parse.ts`.
2. Add generic syntax/decor cache budget and chunk metadata around `src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/client/mod.rs`, `src/editor/surface.rs`, and `src/perf/budgets.rs`.
3. Rewrite the Markdown adapter to consume bounded windows and publish absolute generic decoration spans.
4. Add fallback/status/configuration behavior only after the generic scheduler/cache policy is verified.
5. Update reference docs, wiki pages, and deterministic primitive tests whenever a primitive shape changes.

## Verification

This review satisfies the Phase 18.5 primitive gate:

- Inventory reviewed: parse coordinator, decoration transport, document storage, viewport, behavior manifest, configuration, package loading, SDUI, and benchmark/budget primitives.
- Generic gaps recorded: `ParseWindowSnapshot`, `ParseWindowRequest`, `ParsePolicy`, `SyntaxCacheBudget`, `SyntaxChunkCache`, `DecorationChunk`, `RangeSnapshot`, `LineIndex`, and `ViewportRangeReport`.
- Markdown-specific Rust parser/render branches rejected explicitly.
- Security boundaries preserved: package permission validation, stale-version rejection, range validation, payload budgets, no client-side package JavaScript, and no filesystem/network/shell/AI/WASM/raw-op authority.

## Tests

- `tests/primitives_docs.rs::phase18_large_file_markdown_review_records_generic_parse_window_gaps`
- `tests/primitives_docs.rs::phase18_large_file_review_links_reference_and_wiki_docs`
- `tests/primitives_docs.rs::rust_large_file_primitives_have_no_markdown_token_branches`
- `cargo test --test primitives_docs`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Rendering Primitives](rendering-primitives.md)
- [First-Party Markdown Package](first-party-markdown-package.md)
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
