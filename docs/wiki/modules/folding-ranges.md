# Folding Ranges

## Source

- `src/protocol/folding.rs` — `FoldingRange`, `FoldingRangeSet`, and provenance.
- `src/protocol/parse.rs` — optional fold member on `IncrementalParseUpdate` and `ViewportRenderPatch`.
- `src/protocol/mod.rs` — fold message/event variants and protocol versioning.
- `src/server/folding.rs` — publication validation and generic Tree-sitter derivation.
- `src/server/syntax.rs` — core fold generation beside native syntax parsing.
- `src/server/ops/folding.rs`, `runtime/js/folding.js` — package publication boundary.
- `src/server/parse_coordinator.rs`, `src/server/connection/mod.rs` — validation and delivery.
- `src/client/mod.rs` and `src-tauri/src/bridge/{dto,forwarder}.rs` — typed event and bridge transport.
- `frontend/src/editor/extensions/{folding,render-patch,controller}.ts` — CodeMirror fold projection.
- `frontend/src/editor/position-index.ts`, `frontend/src/editor/position-map.ts` — byte conversion.
- Tests: `src/server/folding.rs`, `src/protocol/codec.rs`, `src/server/connection/mod.rs`, `frontend/src/editor/extensions/{extensions,render-patch,performance}.test.ts`, `tests/{editor_intelligence_protocol,editor_performance,performance_budgets}.rs`.

## Overview

Folding is a host-owned, inert range primitive. Server syntax handlers or
authorized packages publish bounded byte ranges. Clay validates them, transports
them as `FoldingRangeSet` data, and the React client projects them into a local
CodeMirror fold service. Collapsing a range changes only the editor view; it
never mutates canonical text, document versions, or server state.

Viewport responses carry folds as members of the atomic protocol-v29
`ViewportRenderPatch`. Edit-driven parse results may still arrive as a
standalone `FoldingRangeSet`. Neither path runs package JavaScript during
input, layout, scrolling, or paint.

## Responsibilities

- Validate document/version, provenance, ordering, nesting, ranges, permission,
  and `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES` before publication.
- Derive generic core ranges from server-side syntax trees without language-name
  branches.
- Route fold data through `IncrementalParseUpdate`, viewport patch aggregation,
  or the existing edit-driven event path.
- Keep projected ranges sorted and mapped through local edits in a CodeMirror
  state field.
- Provide local fold lookup, gutter presentation, and keymap behavior without
  a server mutation or package callback.

## Primitive Coverage

- **Wire:** `FoldingRangeSet` binds document/version/package provenance to
  ordered `FoldingRange` values with byte start/end and optional inert labels.
- **Publication:** `folding.serverPublishFoldingRanges` uses the existing
  `render-folding` permission and server package context. Caller JSON cannot
  forge provenance or add executable behavior.
- **Core derivation:** `folds_from_syntax_tree` walks named multiline nodes,
  skips the root, caps depth, and stops before serialized output exceeds the
  folding budget.
- **Client:** `foldField` owns projected `FoldItem[]`; `foldPatch` converts byte
  ranges through `BytePositionIndex`, and `foldService` binary-searches the
  sorted list. `applyRenderPatch` is the shared atomic effect for viewport
  members.
- **Reuse rule:** future modes/analyzers publish generic ranges through the
  existing protocol and permission boundary; they do not add language-specific
  client branches, widgets, callbacks, or raw styles.

## How It Works

1. Native Tree-sitter or an authorized package creates a bounded range set.
   `validate_folding_publication` checks `render-folding`, current version,
   provenance, positive UTF-8 ranges, monotonic ordering, proper nesting,
   serialized size, and the independent folding budget.
2. The parse coordinator validates a fold side channel with the enclosing
   document/version/viewport. Invalid decoration/fold/diagnostic combinations
   do not publish a partial result. A viewport request aggregates validated fold
   members with decoration and diagnostic members into one complete patch.
3. `foldPatch` converts every byte boundary with `utf8ToUtf16Batch` using the
   shared position index, rejects invalid or empty projected ranges, tags each
   item with its package authority, and emits `applyRenderPatch`.
4. `foldField` replaces the package authority's whole set and maps retained
   ranges through local CodeMirror changes. It keeps items sorted by `(from,to)`.
   The `foldService` binary-searches to the first possible range at a line and
   scans only ranges beginning on that line.
5. CodeMirror's `foldGutter` paints the local affordance and `foldKeymap` routes
   the standard fold actions. The collapsed state is CodeMirror-local; the
   canonical document remains unchanged.
6. A reset caused by a new document snapshot clears the projected ranges. A
   stale or mismatched set is rejected by controller identity/version checks
   before it becomes a state effect.

## Code Example

```text
ViewportRenderPatch {
  status: complete,
  coveredRanges,
  folds: [FoldingRangeSet { packagePrefix, ranges[] }]
}
```

Fold sets in a viewport patch are members, not independent acknowledgements.
The patch's request ID and terminal status control viewport completion.

## Invariants and Constraints

- Ranges are inert, bounded, UTF-8-safe data. They cannot contain callbacks,
  URLs, filesystem paths, CSS, native handles, raw ops, or client JavaScript.
- A fold set matches the current document version and package provenance; stale,
  malformed, unordered, improperly nested, or over-budget sets fail closed.
- Package publication requires `render-folding`; it does not grant parse,
  filesystem, network, shell, workspace, UI mutation, or AI authority.
- Fold hiding is a projection only. Edits, undo/redo, leases, canonical versions,
  and server text are untouched.
- `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES` is 2,048 bytes per set. Native derivation
  also caps recursive depth; client lookup does not scan every range per line.
- The client retains only current projected field data; viewport patch retention
  is bounded by the shared render guard.

## Tests

- `src/server/folding.rs` — permission/version/provenance validation, payload
  budget, generic multiline-node derivation, and folding/render permission
  separation.
- `src/protocol/codec.rs::protocol_round_trips_viewport_render_patches` —
  fold members inside a complete viewport patch.
- `src/server/connection/mod.rs::viewport_render_requests_answer_one_patch_per_request_id` —
  request-scoped fold aggregation and terminal completion.
- `frontend/src/editor/extensions/extensions.test.ts` — fold projection and
  editor integration.
- `frontend/src/editor/extensions/render-patch.test.ts` and
  `frontend/src/editor/extensions/performance.test.ts` — atomic state effects,
  mapping, and bounded multi-pane retention.
- `tests/editor_intelligence_protocol.rs`, `tests/editor_performance.rs`, and
  `tests/performance_budgets.rs` — wire, matrix, and budget coverage.

Run focused coverage with:

```bash
cargo test --lib server::folding
cargo test --test protocol editor_intelligence_protocol::folding_range_set_round_trips_through_codec_within_budget
cd frontend && npm test -- --run src/editor/extensions/extensions.test.ts
```

## Related

- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Parse Coordinator](parse-coordinator.md)
- [Syntax Sessions](syntax-sessions.md)
- [React CodeMirror Editor](react-codemirror-editor.md)
- `docs/reference/clay-js-api/folding/server-publish-folding-ranges.md`
- `docs/reference/primitives/registry.md#foldingrange`
