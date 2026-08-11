# Range Diagnostics

Phase 18.17 defines one reusable byte-range diagnostic contract for explicit package analyzers and future LSP bridges. Tree-sitter recovery nodes are excluded because bounded syntax fragments are not correctness authority. Status-level failures stay on `RuntimeDiagnostic`. Visual syntax/semantic styling stays on `DecorationSpan`. Range diagnostics carry severity, code, message, and source through a separate validated lifecycle and paint as theme-owned squiggles.

## Ownership Split

| Surface | Primitive | Role |
| --- | --- | --- |
| Document/session failure | `RuntimeDiagnostic` | Status chrome only: severity, code, message. No byte range, source chunk, or package provenance. |
| Syntax/semantic/search tint | `DecorationSpan` / `DecorationSet` | Additive visual vocabulary. `DecorationKind::Diagnostic` is a visual layer only and cannot carry diagnostic message metadata or source replacement. |
| Inline range diagnostic | `DiagnosticSpan` / `DiagnosticSet` | Versioned, viewport-bounded, source-keyed metadata rendered as severity squiggles. |

Diagnostics remain paint-only. They cannot choose `DocumentFontRole`, alter Parley shaping, erase syntax/semantic attributes, or replace decoration chunks.

## Protocol Shape

```rust
pub struct DiagnosticSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub severity: DiagnosticSeverity, // Info | Warning | Error
    pub code: String,
    pub message: String,
    pub source: String,
    pub provenance: DecorationProvenance,
}

pub struct DiagnosticSet {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub viewport_byte_start: u64,
    pub viewport_byte_end: u64,
    pub source: String,
    pub provenance: DecorationProvenance,
    pub spans: Vec<DiagnosticSpan>,
}
```

Replacement key is `DiagnosticChunkKey`: document, version, source, package prefix, and viewport. An empty `spans` array clears that source chunk without touching other sources.

## Diagnostic Authority

First-party Tier 1 native highlighting and Tier 2 web-tree-sitter highlighting do not publish range diagnostics. Tree-sitter `ERROR` and `MISSING` nodes describe parser recovery and can appear when a valid document is split at a bounded viewport edge. Converting them to error squiggles would falsely claim analyzer authority.

Only an explicit analyzer package may publish `DiagnosticSet` data through the validated facade. No language-name or package-name branch is allowed in transport or paint. Raw source snippets, parser internals, paths, callbacks, and executable authority must not leak. Future LSP packages reuse this contract when they land.

## Parse Side Channel

`IncrementalParseUpdate` carries optional `decoration_update` and `diagnostic_update`. `ParseCoordinator` validates matching document/version/viewport/provenance for both side channels, then enforces the combined `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` gate. Acceptance is atomic: invalid diagnostics reject the whole update; superseded/cancelled work publishes neither channel.

## Publication and Rendering

- Package analyzers publish through [`diagnostics.serverPublishDiagnostics`](../clay-js-api/diagnostics/server-publish-diagnostics.md) under existing `render-decorations`.
- Server validation lives in `src/server/diagnostics.rs`; transport is `ServerMessage::DiagnosticSet`.
- Client `EditorDiagnosticState` retains near-viewport chunks under `DIAGNOSTIC_CACHE_BUDGET_BYTES`.
- `StyleRegistry::diagnostic_style(severity)` resolves `diagnosticError` / `diagnosticWarning` / `diagnosticInfo`.
- Native paint strokes Clay-owned zig-zag marks from cached Parley line-local rectangles. Diagnostic arrival requests render without rebuilding text layout.
- Syntax, Semantic, Diagnostic, and Search remain additive layers; diagnostics cannot choose font roles or erase syntax/semantic styling.

Future LSP packages map LSP diagnostic fields onto this same inert contract. Phase 18.17 does not spawn language servers or grant LSP process authority.

## Budgets and Hot Paths

| Limit | Constant |
| --- | --- |
| Serialized chunk | `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES` (8 KiB) |
| Spans per set | `DIAGNOSTIC_MAX_SPANS_PER_SET` (128) |
| Code / message / source fields | `DIAGNOSTIC_MAX_CODE_BYTES`, `DIAGNOSTIC_MAX_MESSAGE_BYTES`, `DIAGNOSTIC_MAX_SOURCE_BYTES` |
| Retained cache | `DIAGNOSTIC_CACHE_BUDGET_BYTES` (8 MiB) |

Parse extraction, package publication, validation, and IPC stay off keypress, paint, layout, scroll, pointer, and text-event hot paths. Paint consumes prevalidated, viewport-filtered, locally cached spans only.

## Security

Publication requires `render-decorations`, matching package provenance, current document version, in-viewport ranges, and bounded sanitized strings. Executable fields (`handler`, `callback`, `clientJavaScript`, `rawOps`, `css`, `draw`, native handles) are rejected. The API grants no filesystem, network, shell, AI, WASM, workspace, language-server process, raw-op, client-JavaScript, CSS, or native-render authority.

## Coverage

Deterministic coverage: `tests/range_diagnostics.rs`, `tests/syntax_grammar.rs`, `tests/parse_coordinator.rs`, `tests/performance_protocol.rs`, `tests/editor_performance_invariants.rs`, `tests/package_loading_docs.rs`, and `tests/manual_smoke_docs.rs`. Manual invalid/repair matrix: `docs/development/launch-and-gui-smoke.md` Phase 18.17 section. Implementation wiki: `docs/wiki/modules/phase18.17-range-diagnostics-primitive-review.md`.
