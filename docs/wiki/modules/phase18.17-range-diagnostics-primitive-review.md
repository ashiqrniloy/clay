# Phase 18.17 Range Diagnostics Primitive Review

> **Current implementation note:** the generic `DiagnosticSet` transport, validation, cache, facade, and squiggle renderer remain active for explicit analyzers. Automatic Tree-sitter `ERROR`/`MISSING` publication described in this historical review was removed: bounded parse-fragment recovery nodes are not correctness authority. First-party syntax highlighting emits no diagnostics; future LSP/analyzer packages must publish them explicitly.

## Source

- Plan: `plans/049-Phase18.17-Range-Diagnostics-and-Syntax-Error-Highlighting.md` (task 2).
- Decision: `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`.
- Patterns: `.agents/skills/project-patterns/references/language-capability-sequencing.md`, `mode-primitive-first.md`, `protocol-and-performance.md`, and `authority-boundaries.md`.
- `src/protocol/mod.rs`, `src/protocol/decorations.rs`, `src/protocol/parse.rs`.
- `src/server/decorations.rs`, `src/server/parse_coordinator.rs`, `src/server/syntax.rs`, `src/server/ops/decorations.rs`.
- `runtime/js/decorations.ts`, `src/client/mod.rs`, `src/masonry_editor.rs`.
- `src/editor/theme.rs`, `src/editor/surface.rs`, `src/editor/layout.rs`.
- `tests/primitives_docs.rs`, `tests/parse_coordinator.rs`, `tests/decoration_transport.rs`, `tests/syntax_grammar.rs`, `tests/editor_performance_invariants.rs`.

## Overview

Phase 18.17 needs source-associated byte-range diagnostics that can be produced by Tree-sitter now and by package analyzers or opt-in LSP bridges later. Existing Clay primitives already provide background parsing, stale-result rejection, bounded decoration transport, client chunk state, theme resolution, Parley range geometry, and native Vello paint. Those paths should be extended, not duplicated.

`src/protocol/diagnostics.rs` and `src/server/diagnostics.rs` now establish the first generic gap: first-class range records carrying severity, code, message, source, and provenance through versioned server validation and source-keyed replacement. `RuntimeDiagnostic` remains status-level failure reporting, while `DecorationSpan` remains visual styling data. Later Phase 18.17 tasks connect this contract to parse production, client chunk state, and paint.

## Existing Primitive Inventory

### Status diagnostics and diagnostic severity

`src/protocol/mod.rs::DiagnosticSeverity` already defines `Info`, `Warning`, and `Error`; reuse it. `src/protocol/mod.rs::RuntimeDiagnostic` carries severity, code, and message and is routed by `ClientConnectionEvent::RuntimeDiagnostic` to `EditorStatus` in `src/masonry_editor.rs`.

`RuntimeDiagnostic` is reusable for sanitized parser/runtime failure status only. It has no document ID, document version, byte range, source, or package provenance, and the client displays it in status chrome. Adding range fields to it would mix document/session failures with replaceable inline diagnostics and would force status consumers to understand editor geometry.

### Decoration layers, transport, and client cache

`src/protocol/decorations.rs::DecorationSpan` already supplies a validated non-empty byte range, `DecorationKind`, `TokenType` + `Modifiers`, priority, optional syntax/semantic font role, and `DecorationProvenance`. `DecorationSet` supplies document/version and viewport bounds. `src/server/decorations.rs::validate_decoration_set` rejects stale versions, empty/reversed/off-viewport ranges, invalid style scopes, unauthorized font roles, provenance mismatch, and payload overflow.

`src/client/mod.rs`, `src/masonry_editor.rs`, and `src/editor/surface.rs::EditorDecorationState` already route, apply, version-clear, near-viewport-prune, and paint bounded inert chunks outside package code. This lifecycle is reusable as a pattern. `DecorationSpan` itself is not the diagnostic metadata primitive: `DecorationKind::Diagnostic` has only visual token/layer data and cannot carry diagnostic code, message, source, or independent source-chunk replacement semantics.

Diagnostics must remain additive with existing Syntax, Semantic, and Search layers. A diagnostic set must not replace syntax/semantic decoration state, choose a font role, or alter shaping merely because it shares byte geometry.

### Style registry and native rendering

`src/editor/theme.rs::StyleRegistry` is the single color source. `style_for(DecorationKind::Diagnostic, ...)` keeps the legacy decoration-layer tint. Range diagnostics use `diagnostic_style(DiagnosticSeverity)` with theme-owned `diagnosticError` / `diagnosticWarning` / `diagnosticInfo` overrides (defaults plus Gruvbox Material packages).

`src/editor/layout.rs::LayoutState` owns cached visible Parley layout, `Selection::geometry`, caret geometry, wrapping, and UTF-8-aware local offsets. Paint maps visible `DiagnosticSpan` ranges through that cached geometry, then draws Clay-owned zig-zag underlines via Vello `Scene::stroke`. Diagnostic arrival updates chunk state and requests render without bumping `layout_style_revision`.

### Parse coordinator and incremental update transport

`src/server/parse_coordinator.rs::ParseCoordinator` owns background scheduling, cancellation, generation replacement, viewport prioritization, stale-version rejection, sanitized failure reporting, and the `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` gate. `src/protocol/parse.rs::IncrementalParseUpdate` now carries optional `decoration_update` and `diagnostic_update` side channels.

The scheduler is reused unchanged. Coordinator validation matches diagnostic document/version/viewport/package provenance to the enclosing update, runs centralized diagnostic validation, then checks the combined serialized update budget. Invalid diagnostic output rejects the complete parse update without partial decoration publication; no second diagnostic scheduler exists.

### Tiered syntax engine and Tree-sitter error nodes

`src/server/syntax.rs::TreeSitterSyntaxHandler` already receives bounded parse windows, reuses cached parsers/trees, and maps generic captures through one language-neutral path. Tree-sitter v0.25.10 exposes `Node::{has_error,is_error,is_missing,byte_range,walk}`; `(ERROR)` and `(MISSING)` query patterns provide the equivalent capture model.

Generic extraction can short-circuit on `root.has_error()`, walk only the bounded parsed tree, and translate local node bytes through the parse window. No Rust, TypeScript, TypeScript/TSX, JavaScript, or Markdown branch is needed. Tier 2 and Tier 3 adapters should emit the same inert engine-neutral diagnostic capture shape when operational.

### Package permissions and Clay JS publication

`runtime/js/decorations.ts` and `src/server/ops/decorations.rs` expose `clay:decorations.serverPublishDecorations` under `render-decorations`. It can publish `kind: "diagnostic"`, but only as a visual `DecorationSpan`; it cannot represent message/code/source metadata or diagnostic-source replacement.

Implemented: `runtime/js/diagnostics.ts` and `src/server/ops/diagnostics.rs` expose `clay:diagnostics.serverPublishDiagnostics` under the same `render-decorations` permission. It publishes distinct bounded `DiagnosticSet` data with provenance validation, rejects executable/raw-authority fields, and grants no LSP process or extra authority.

## Implemented Protocol and Validation Contract

`DiagnosticSpan` reuses `DiagnosticSeverity` and `DecorationProvenance`, while retaining range-specific code, message, and source. `DiagnosticSet` carries document/version/viewport plus set-level source and provenance; duplicating identity at set level is intentional because an empty set must still address and clear its prior source chunk. `DiagnosticChunkKey` defines replacement by document, version, source, package prefix, and viewport.

`validate_diagnostic_set` rejects stale versions, reversed viewports, excessive span counts, empty/reversed/off-viewport ranges, empty/oversized/control-containing metadata and provenance, source/set mismatches, provenance/set mismatches, package identity mismatches, serialization failures, and payload overflow. `validate_diagnostic_publication` additionally requires existing `render-decorations` permission. Accepted spans receive deterministic range/severity/code/message ordering.

Typed limits live in `src/perf/budgets.rs`: `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_MAX_SPANS_PER_SET`, code/message/source/provenance field limits, and `DIAGNOSTIC_CACHE_BUDGET_BYTES`. `ServerMessage::DiagnosticSet` uses the normal codec. `IncrementalParseUpdate::diagnostic_update` implements the atomic inert side channel; `SyntaxDiagnosticCapture`/`SyntaxDiagnosticKind` define the engine-neutral local `ERROR`/`MISSING` shape for the next extraction task. Runtime-backed handlers may return inert `{ diagnostics: { source, spans } }` JSON, while Rust derives provenance from accepted package registration.

Coverage lives in `tests/range_diagnostics.rs` and `tests/performance_protocol.rs`. Run `cargo test --test range_diagnostics` and `cargo test --test performance_protocol representative_diagnostic_chunk_payload_stays_bounded`.

## What Existing Primitives Already Achieve

Without new scheduling, package execution, typography, or rendering authority, Clay can already:

- run native or package parse work asynchronously over bounded server-provided windows;
- cancel superseded work and stale-drop old document/runtime generations;
- retain package/core provenance and enforce `parse-document` plus `render-decorations`;
- transport versioned viewport chunks through validated `rkyv` messages;
- cache only visible/near-viewport inert state;
- resolve all editor colors through `StyleRegistry`;
- obtain UTF-8-aware wrapped range rectangles from cached Parley layout;
- render native Vello marks without package JavaScript in the client.

Phase 18.17 therefore needs a small data/validation/application extension, not a language diagnostics subsystem or renderer plugin framework.

## Generic Phase 18.17 Gaps

### Distinct `DiagnosticSpan` and `DiagnosticSet`

Add a protocol-owned range record containing non-empty byte range, reused `DiagnosticSeverity`, bounded code/message/source strings, and package/core provenance. Add a document/version/viewport-bounded set with source-chunk replacement semantics. Empty sets are meaningful: they clear prior diagnostics for the same document/version/source/provenance/viewport chunk.

Keep `RuntimeDiagnostic` unchanged for status failures and keep `DecorationSpan` focused on visual vocabulary. Reuse `DecorationProvenance` or rename it generically only if that avoids duplication without broad churn.

### Central diagnostic validation and budgets

Add one server validator for current document/version, ordered in-viewport ranges, field length/control-character rules, provenance, deterministic ordering, span count, serialized payload, and retained cache limits. The client rechecks document/version/range/UTF-8 boundaries before geometry use.

Tree-sitter `MISSING` nodes are zero-width, while visible diagnostics and current decoration validation require non-empty ranges. Normalize them before `DiagnosticSpan` validation: choose the next UTF-8 scalar inside the parse window; at window/document end choose the previous scalar; emit nothing only for an empty document. This anchoring policy is parser-independent and belongs in generic syntax-diagnostic shaping, not language queries.

### Engine-neutral parse diagnostic side channel

`IncrementalParseUpdate` includes `diagnostic_update: Option<DiagnosticSet>`. Parse coordinator validation checks matching document/version/viewport/provenance and the combined serialized update budget. Decoration and diagnostic outputs from one parse update are accepted or rejected together. Superseded/cancelled work reaches neither side channel.

`TreeSitterSyntaxHandler` now fills this channel from the same cached native parse tree. Valid roots short-circuit through `Node::has_error` and publish an empty `tree-sitter` source set; invalid roots use one iterative, viewport-bounded `Node::{is_error,is_missing,byte_range,walk}` traversal. Nested/equal recovery ranges are deduplicated, output is capped at `DIAGNOSTIC_MAX_SPANS_PER_SET`, and zero-width missing nodes receive a UTF-8-safe neighboring-scalar anchor. Fixed Clay-owned codes/messages prevent source, parser-internal, query, or path leakage. `runtime/js/web-tree-sitter-host.ts::collectWebTreeSitterDiagnostics` mirrors the generic local capture shape for Tier 2 without package callbacks or language branches.

Native Tree-sitter produces a small engine-neutral capture (`byte range` plus error/missing kind). Tier 2/Tier 3 adapters map to the same capture/result shape. Translation to protocol diagnostics occurs after parse-window local-to-document byte translation.

### Source-keyed server/client chunk lifecycle

Route a dedicated `ServerMessage::DiagnosticSet` and `ClientConnectionEvent::DiagnosticSet`. The live connection loop drains accepted `ParseCoordinator` updates and forwards decoration plus diagnostic side channels (and sanitized `RuntimeDiagnostic` failures). `ClayOpState` owns a `DiagnosticChunkCache` under `DIAGNOSTIC_CACHE_BUDGET_BYTES`; empty sets clear matching source chunks without growing retained bytes. `EditorSurface` keeps independent `EditorDiagnosticState` keyed by document/version/source/package/viewport: matching chunks replace atomically, multiple sources compose, mismatched IDs/versions are ignored, and version advance/snapshot load clears stale diagnostics before async reparse. Near-viewport pruning reuses `DECORATION_NEAR_VIEWPORT_GUARD_BYTES`.

Do not hide these records inside `ServerMessage::DecorationSet`: independent source replacement and metadata must survive transport even though native paint later uses only range/severity.

### Severity-aware theme resolution and native squiggle geometry

Implemented: `StyleRegistry::diagnostic_style(severity)` plus `diagnosticError` / `diagnosticWarning` / `diagnosticInfo` theme override targets. `EditorSurface::visible_diagnostic_ranges` maps cached spans through severity styles; `LayoutState::paint_text` strokes Clay-owned squiggles from Parley line-local rectangles after text, clipped to the editor viewport. Themes contribute inert colors only.

Diagnostics remain paint-only. They cannot select `DocumentFontRole`, inject Parley properties, erase syntax/semantic attributes, or force full-document shaping. Syntax, Semantic, Diagnostic, and Search remain additive layers.

### Bounded package publication for future analyzers/LSP bridges

Implemented: `clay:diagnostics.serverPublishDiagnostics` facade/op uses existing package context, provenance, `render-decorations` permission, and server publication. The API accepts inert metadata only. Phase 18.17 does not implement LSP process management, hover, quick fixes, analyzer registration, or language-specific package branches. Public contract: `docs/reference/clay-js-api/diagnostics/server-publish-diagnostics.md`.

## Data Flow and Reuse Rule

```text
Tree-sitter / package analyzer
  -> engine-neutral diagnostic captures
  -> DiagnosticSet(document/version/source/provenance/viewport)
  -> ParseCoordinator + server diagnostic validation
  -> ServerMessage::DiagnosticSet
  -> ClientConnectionEvent::DiagnosticSet
  -> source-keyed near-viewport client cache
  -> cached Parley range geometry + StyleRegistry severity style
  -> native Vello squiggle
```

Future package analyzers and opt-in LSP bridges publish `DiagnosticSet`; they do not add a parser scheduler, protocol variant family, client widget, or language-specific Rust paint branch.

## Hot-Path Classification

| Work | Allowed location |
| --- | --- |
| Parser/query execution and `ERROR`/`MISSING` traversal | server background parse task over bounded windows |
| Range anchoring, metadata shaping, validation, sorting, serialization | parser result/server publication path |
| Protocol decode, version/source replacement, cache pruning | client event application before paint |
| Parley geometry creation | existing visible layout rebuild/cache path |
| Masonry paint, layout, keypress, pointer, scroll, text-event handlers | cached local diagnostic spans, cached Parley geometry, and resolved theme style only |

No parser/package JavaScript, IPC, server validation, query compilation, full-document scan, source-chunk sorting, metadata sanitization, or allocation-heavy tree traversal belongs in Masonry paint, layout, keypress, pointer, scroll, or text-event handlers. `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, `SYNTAX_CACHE_BUDGET_BYTES`, `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_CACHE_BUDGET_BYTES`, and `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` remain applicable.

## Security and Authority Boundary

Range diagnostics are bounded inert strings, enums, byte ranges, and provenance. Server publication validates package identity, permission, current document/version, viewport bounds, source ownership, UTF-8-compatible ranges, field limits, and serialized size. Runtime/parser failures continue to use sanitized `RuntimeDiagnostic` status messages without source text, paths, query text, or parser internals.

This phase adds no filesystem, network, shell, AI, workspace mutation, language-server subprocess, native-ui, package-control, package-manager, raw-ops, client-runtime, raw CSS, Vello/Parley callback, native handle, or arbitrary WASM authority. Future LSP process authority remains a separate explicit decision and permissioned package phase.

## Rejected Implementation Shapes

- Do not add `RustDiagnosticProvider`, `TypeScriptDiagnosticProvider`, `JavaScriptDiagnosticProvider`, `MarkdownDiagnosticProvider`, or language-name branches in server/client/editor code.
- Do not encode message, code, source, or severity into `style_token`, `scope`, or `TokenType` strings.
- Do not add range/document fields to `RuntimeDiagnostic` or status rendering logic to `DiagnosticSpan` consumers.
- Do not add a second parse/diagnostic scheduler beside `ParseCoordinator`.
- Do not publish one protocol frame per diagnostic span or full-document diagnostic snapshots on ordinary edits.
- Do not run Tree-sitter traversal, package JavaScript, IPC, validation, sorting, or metadata parsing in Masonry paint/layout/input paths.
- Do not let diagnostics choose font roles, raw colors, squiggle paths, widths, animations, CSS, callbacks, or native handles.
- Do not implement LSP process spawning, hover, quick fixes, or analyzer-provider abstractions in Phase 18.17.

## Tests

- `tests/primitives_docs.rs::phase18_17_range_diagnostics_primitive_review_is_linked_and_complete`: locks inventory, generic gaps, zero-width anchoring, additive layers, hot-path split, authority boundary, and rejected shapes.
- `tests/primitives_docs.rs::range_diagnostics_implementation_wiki_is_linked_and_complete`: locks the implementation wiki page and master-index link.
- Implementation coverage: `tests/range_diagnostics.rs`, `tests/parse_coordinator.rs`, `tests/syntax_grammar.rs`, `tests/performance_protocol.rs`, `tests/editor_performance_invariants.rs`, `tests/clay_js_doc_registry.rs`, `tests/rust_visibility_api_mapping.rs`, `tests/manual_smoke_docs.rs`.

Run:

```bash
cargo test --test primitives_docs phase18_17_range_diagnostics_primitive_review_is_linked_and_complete
cargo test --test primitives_docs range_diagnostics_implementation_wiki_is_linked_and_complete
```

## Related

- [Range Diagnostics](range-diagnostics.md)
- [Primitive Architecture](primitive-architecture.md)
- [Rendering Primitives](rendering-primitives.md)
- [Decoration Transport](decoration-transport.md)
- [Parse Coordinator](parse-coordinator.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Package Primitive Security](../../reference/primitives/package-security.md)
- [Diagnostics primitive](../../reference/primitives/diagnostics.md)
