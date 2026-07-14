# Range Diagnostics

## Source

- `src/protocol/diagnostics.rs` — `DiagnosticSpan`, `DiagnosticSet`, `DiagnosticChunkKey`.
- `src/protocol/parse.rs` — `SyntaxDiagnosticCapture`, `SyntaxDiagnosticKind`, `IncrementalParseUpdate.diagnostic_update`.
- `src/protocol/mod.rs` — `RuntimeDiagnostic`, `DiagnosticSeverity`, `ServerMessage::DiagnosticSet`.
- `src/server/diagnostics.rs` — `validate_diagnostic_publication`, `validate_diagnostic_set`, `DiagnosticChunkCache`, `DiagnosticValidationError`.
- `src/server/ops/diagnostics.rs` — `op_clay_diagnostics_publish_diagnostics`.
- `src/server/parse_coordinator.rs` — side-channel validation and `next_update` / `next_diagnostic` delivery.
- `src/server/syntax.rs` — native highlighting deliberately leaves `diagnostic_update` empty.
- `src/server/connection.rs` — connection `select!` drains parse decorations + diagnostic sets.
- `src/server/ops/mod.rs` — `ClayOpState::published_diagnostic_set` / `publish_diagnostic_set`.
- `src/client/mod.rs` — `ClientConnectionEvent::DiagnosticSet`.
- `src/masonry_editor.rs` — `apply_diagnostic_set` vs status-level `RuntimeDiagnostic`.
- `src/editor/surface.rs` — `EditorDiagnosticState`, `apply_diagnostic_set`, `visible_diagnostic_ranges`.
- `src/editor/layout.rs` — `paint_squiggle`, `diagnostic_mark_rects_in_layout`.
- `src/editor/theme.rs` — `StyleRegistry::diagnostic_style`, severity colors.
- `src/perf/budgets.rs` — `DIAGNOSTIC_*` budgets.
- `runtime/js/diagnostics.ts`, `runtime/js/web-tree-sitter-host.ts::collectWebTreeSitterDiagnostics`.
- Tests: `tests/range_diagnostics.rs`, `tests/syntax_grammar.rs`, `tests/parse_coordinator.rs`, `tests/performance_protocol.rs`, `tests/editor_performance_invariants.rs`, `tests/primitives_docs.rs`, `tests/clay_js_doc_registry.rs`, `tests/rust_visibility_api_mapping.rs`, `tests/manual_smoke_docs.rs`.
- Authoritative public API: [`clay.diagnostics.serverPublishDiagnostics`](../../reference/clay-js-api/diagnostics/server-publish-diagnostics.md).
- Authoritative primitive contract: [Diagnostics](../../reference/primitives/diagnostics.md).
- Primitive review: [Phase 18.17 Range Diagnostics Primitive Review](phase18.17-range-diagnostics-primitive-review.md).

## Overview

Phase 18.17 adds source-associated range diagnostics as an additive editor layer. Explicit analyzer packages publish viewport-bounded `DiagnosticSet` values. Native Tree-sitter highlighting does not publish diagnostics: recovery nodes from bounded parse fragments are not correctness authority and caused false squiggles. The server validates analyzer output, ships `ServerMessage::DiagnosticSet`, and the client caches source-keyed chunks for paint-only severity squiggles. Status failures stay on `RuntimeDiagnostic`; range diagnostics never choose font roles or replace syntax/semantic decoration state.

## Responsibilities

- Own inert `DiagnosticSpan` / `DiagnosticSet` protocol shapes, budgets, and deterministic ordering.
- Keep grammar highlighting separate from analyzer-owned diagnostic authority.
- Validate explicit package publication through `render-decorations` + provenance without a new permission.
- Deliver diagnostics asynchronously beside decorations; keep typing/open non-blocking.
- Cache source-keyed chunks near-viewport under `DIAGNOSTIC_CACHE_BUDGET_BYTES`.
- Paint severity squiggles from cached Parley geometry and theme-owned colors only.

## How It Works

### Protocol and validation

`DiagnosticSpan` carries `byte_start`/`byte_end`, `DiagnosticSeverity`, bounded `code`/`message`/`source`, and provenance. `DiagnosticSet` is document/version/viewport/source scoped; empty spans for a source clear that source's prior chunk. `validate_diagnostic_publication` checks permission then `validate_diagnostic_set` (version, viewport, span count, ranges, field sanitization, provenance, payload and cache budgets).

### Parse side channel

`IncrementalParseUpdate.diagnostic_update` rides beside optional decorations. `ParseCoordinator::validate_update` requires matching document/version/viewport/provenance for both channels and accepts or rejects them together. Connection loops drain `next_update()` → `DecorationSet` / `DiagnosticSet` and `next_diagnostic()` → sanitized status `RuntimeDiagnostic`.

### Diagnostic authority

Tree-sitter `ERROR`/`MISSING` recovery nodes are parser recovery details, especially for bounded viewport fragments; they are not proof that user code is invalid. Tier 1 native highlighting therefore returns `diagnostic_update: None`. First-party language packages emit no squiggles until an explicit analyzer such as a future Phase 18.21 LSP package publishes a validated `DiagnosticSet`. The generic diagnostic transport and package facade remain available without language-name branches.

### Server/client chunk lifecycle

`DiagnosticChunkCache` mirrors decoration chunk retention with near-viewport guard and LRU under `DIAGNOSTIC_CACHE_BUDGET_BYTES`. `EditorDiagnosticState` applies matching sets, composes multiple sources, ignores stale/mismatched IDs/versions, and clears on version advance / snapshot load before async reparse.

### Package publication

`serverPublishDiagnostics` → `op_clay_diagnostics_publish_diagnostics` → `validate_diagnostic_publication` → `ClayOpState::publish_diagnostic_set`. JS facade rejects executable/raw-authority fields. No LSP process, hover, quick-fix, or filesystem/network/shell authority.

### Theme and paint

`StyleRegistry::diagnostic_style(severity)` maps Error/Warning/Info to theme keys `diagnosticError` / `diagnosticWarning` / `diagnosticInfo`. `visible_diagnostic_ranges` maps spans to colors; `paint_text` strokes zig-zag paths via `paint_squiggle` after text. `apply_diagnostic_set` requests render but does not bump `layout_style_revision`.

## Code Examples

```js
import { serverPublishDiagnostics } from "clay:diagnostics";

serverPublishDiagnostics({
  documentId,
  documentVersion,
  viewport: { byteStart, byteEnd },
  source: "my-analyzer",
  spans: [{
    byteStart,
    byteEnd,
    severity: "error",
    code: "parser.syntax-error",
    message: "Syntax error",
  }],
});
```

```bash
cargo test --test range_diagnostics
cargo test --test syntax_grammar
cargo test --test primitives_docs range_diagnostics
```

## Primitive Coverage

- **Range diagnostic publication** — `DiagnosticSet` in `src/protocol/diagnostics.rs`; facade `runtime/js/diagnostics.ts::serverPublishDiagnostics`.
- **Analyzer-neutral transport** — explicit package analyzers and future LSP bridges publish the same inert `DiagnosticSet` records.
- **Permissions / budgets** — reuses `render-decorations`; `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_MAX_SPANS_PER_SET`, `DIAGNOSTIC_CACHE_BUDGET_BYTES`.
- **Hot-path policy** — parse/validate/serialize on background server path; paint uses cached spans + Parley rects + theme colors only.
- **Reuse rule** — future analyzers/LSP bridges publish `DiagnosticSet`; they do not add schedulers, language paint branches, or font-role authority.

## Invariants and Constraints

- Additive layer: Syntax, Semantic, Diagnostic, and Search compose; diagnostics cannot choose `DocumentFontRole`.
- `RuntimeDiagnostic` stays status-only; `DiagnosticSet` stays inline/paint-only.
- Empty source chunk clears only that source.
- No language-name branches in server/client/editor diagnostic code.
- No raw Deno ops, CSS, callbacks, native handles, or LSP process spawning in this phase.
- Tree-sitter highlighting never masquerades as analyzer diagnostics.
- Paint-path sources must not hardcode diagnostic colors (`tests/editor_performance_invariants.rs`).

## Tests

- `tests/range_diagnostics.rs` — transport, client apply, multi-source clear, paint/layout invariants, facade authority denial.
- `tests/syntax_grammar.rs` — native highlighting emits decorations but no analyzer diagnostics, including invalid first-party fixtures.
- `tests/performance_protocol.rs` — non-blocking typing under slow parse.
- `tests/editor_performance_invariants.rs` — no hot-path extraction, theme-owned paint colors.
- `tests/primitives_docs.rs` — reference + wiki coverage guards.
- `tests/clay_js_doc_registry.rs` / `tests/rust_visibility_api_mapping.rs` — facade/op/docs/visibility.

## Related

- [Phase 18.17 Range Diagnostics Primitive Review](phase18.17-range-diagnostics-primitive-review.md)
- [Parse Coordinator](parse-coordinator.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Decoration Transport](decoration-transport.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md)
- [Diagnostics primitive](../../reference/primitives/diagnostics.md)
- [`serverPublishDiagnostics`](../../reference/clay-js-api/diagnostics/server-publish-diagnostics.md)
