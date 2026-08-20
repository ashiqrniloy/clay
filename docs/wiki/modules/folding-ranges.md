# Folding Ranges

## Source

- `src/protocol/folding.rs` — `FoldingRange`, `FoldingRangeSet`, and provenance.
- `src/protocol/parse.rs` — `IncrementalParseUpdate::folding_update`.
- `src/protocol/mod.rs` — `ServerMessage::FoldingRangeSet` and protocol-version history.
- `src/server/folding.rs` — validation, the crate-private registry, and the generic Tree-sitter walk.
- `src/server/syntax.rs` — core fold generation beside native syntax parsing.
- `src/server/ops/folding.rs` — `op_clay_folding_publish_ranges`.
- `src/server/js_runtime/evaluation.rs` and `src/server/js_runtime/error.rs` — package publication harvest.
- `src/server/connection/mod.rs` — bounded parse-update delivery to clients.
- `src/client/mod.rs` — connection event decoding.
- `src/editor/surface/mod.rs` — client-local fold state, hidden-line snapshots, caret/selection remapping, and toggle.
- `src/editor/surface/chrome.rs` — gutter chevron paint.
- `src/editor/layout.rs` — fold revision in `LayoutCacheKey`.
- `src/masonry_pane_document.rs` and `src/masonry_editor.rs` — command routing and editor event application.
- `runtime/js/folding.js` — `clay:folding` facade.
- `docs/reference/clay-js-api/folding/server-publish-folding-ranges.md` — authoritative public API contract.
- `docs/reference/primitives/registry.md` — primitive inventory and budget contract.
- Tests: `src/server/folding.rs`, `src/editor/surface/mod.rs`, `tests/performance_budgets.rs`, `tests/editor_performance_invariants.rs`, `tests/primitives_docs.rs`.

## Overview

Folding is a host-owned editor primitive. Server-side syntax handlers or
authorized package code publish bounded byte ranges as inert data; Clay validates
and transports those ranges, then the client owns collapsed/expanded state. The
client hides interior lines at the visible-snapshot/line-metrics boundary and
paints a Clay-owned gutter chevron. Neither package JavaScript nor a syntax
query enters the keypress, paint, layout, pointer, or scroll paths.

Phase 28.3 added the wire set and parse-update field. Later Phase 28 protocol
versions add links, inlays, and completion recency; folding remains the same
versioned `FoldingRangeSet` primitive introduced at protocol version 20.

## Responsibilities

- `src/protocol/folding.rs` defines document/version/provenance-bound folding data.
- `src/server/folding.rs` rejects stale, malformed, unordered, improperly nested,
  provenance-mismatched, unserializable, or over-budget publications.
- `src/server/syntax.rs` derives generic core ranges from the cached Tree-sitter
  tree without language-name branches.
- `src/server/ops/folding.rs` exposes the package publication boundary and derives
  provenance from the current authorized package, never from caller JSON.
- `IncrementalParseUpdate` carries one optional fold set without adding another
  parse message family; the connection sends it as `ServerMessage::FoldingRangeSet`.
  Ordinary parse updates retain the 4096-byte envelope; an attached fold set
  uses the derived `INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES`
  envelope (4096 + the independently capped 2048-byte folding payload).
- `EditorSurface` stores ranges by provenance and keeps collapse state local to
  the document surface. `editor.clientToggleFold` is an argless client command,
  not a server mutation or package callback.

## Primitive Coverage

- **Protocol shape:** `FoldingRangeSet` contains `document_id`, exact
  `document_version`, `package_prefix`, and ordered `FoldingRange` values.
  Each range carries `byte_start`, `byte_end`, an optional inert label, and
  `FoldingProvenance`.
- **Publication API:** `clay:folding` exports
  `serverPublishFoldingRanges`; stable API ID is
  `folding.serverPublishFoldingRanges`. It requires `render-folding` and is a
  background/package publication surface, not a client callback surface.
- **Validation:** `validate_folding_publication` checks the package permission
  and current document version, then `validate_folding_set` checks non-empty
  provenance, positive ranges, monotonic starts, proper nesting, per-range
  provenance, serialization, and `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`. The
  parse coordinator applies the derived combined envelope only when a fold set
  is attached; it never enlarges the ordinary parse-window budget.
- **Core derivation:** `folds_from_syntax_tree` walks named multiline Tree-sitter
  nodes, skips the root, caps recursion at `MAX_DEPTH = 32`, and stops adding
  candidates when the serialized set would exceed the folding budget.
- **Server storage:** `FoldingRangeRegistry` is a crate-private replacement and
  merge helper keyed by document and package provenance. The active parse wire
  carries one validated set in `IncrementalParseUpdate`; package publications are
  harvested from runtime op state when the parser result does not already provide
  a fold set, rather than creating a second client protocol.
- **Client presentation:** validated sets replace their provenance slot. The
  surface chooses the smallest containing range at the caret, toggles its start
  offset in a local `BTreeSet`, and increments `folds.revision`.
- **Reuse rule:** future modes and analyzers publish generic byte ranges through
  the existing parse/folding boundary. They do not add language-specific folding
  branches, renderer callbacks, custom widgets, or package-authored client code.

## How It Works

### Package publication

```js
import { serverPublishFoldingRanges } from "clay:folding";

serverPublishFoldingRanges({
  documentId,
  documentVersion,
  ranges: [{ byteStart: 0, byteEnd: 48, label: "section" }],
});
```

The facade serializes options and calls `op_clay_folding_publish_ranges`. The op
requires the current package's `render-folding` capability, uses the host package
record for `package_name`, `package_version`, and `package_prefix`, rejects a
version mismatch as a dropped update, validates every range, and stores only the
validated set in `ClayOpState`. The parse-runtime evaluation harvests that set
into `IncrementalParseUpdate.folding_update` when the returned parser JSON did
not include one.

### Core syntax path

`TreeSitterSyntaxHandler::parse` reuses its cached parser/tree and calls
`folds_from_syntax_tree` after producing decoration chunks. The fold walker is
language-neutral: any named node spanning multiple rows is a candidate, with
nested ranges retained in source order. It carries `FoldingProvenance::core()`
and applies the payload budget while walking, so a dense or deeply nested tree
cannot force an unbounded fold publication.

### Parse-to-client flow

```text
Tree-sitter/package parser
  -> IncrementalParseUpdate { decoration_updates, folding_update }
  -> bounded per-update validation
  -> connection-local parse subscription
  -> ServerMessage::FoldingRangeSet
  -> ClientConnectionEvent::FoldingRangeSet
  -> PaneDocumentView -> EditorSurface::apply_folding_set
```

`apply_folding_set` ignores another document or version, replaces the incoming
provenance slot, removes collapsed starts that no longer exist, and increments
the fold revision. The revision is included in `LayoutCacheKey`; changing
collapsed state therefore invalidates cached line geometry without changing
text or decoration layout state.

### Hide, navigate, and paint

`visible_snapshot` applies `fold_visible_snapshot` after extracting the viewport,
which omits lines strictly inside collapsed ranges while leaving the fold-start
line visible. `visible_byte_offset`, selection clipping, cursor movement, and
edit offset remapping account for hidden bytes through the surface's fold-aware
helpers. The canonical buffer and document version are never rewritten.

`paint_gutter` paints a chevron on lines whose byte start matches a validated
range. The chevron is visual chrome, not an individual Tab/AT-SPI target; the
keyboard command operates at the caret through the closed
`EditorClientCommand::from_command_id` table. The same table keeps
`editor.clientToggleFold` available to package/user keymaps while unknown IDs
remain rejected.

## Invariants and Constraints

- Collapse state is client-local and is cleared with a new document snapshot;
  packages cannot request or persist a collapsed state.
- Package output is inert: no fold callback, renderer hook, URL, filesystem path,
  raw op, native handle, or client-side JavaScript crosses the publication path.
- `render-folding` is required for package publication. It does not grant
  filesystem, workspace, network, shell, extension-loading, AI, WASM, or UI
  mutation authority.
- Ranges must match the current document version, have `byte_start < byte_end`,
  be ordered by start, and be properly nested. Stale and over-budget data is
  dropped or denied rather than truncated into ambiguous UI state.
- `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES` is 2,048 bytes per set. The tree walk's
  depth and serialized-size checks are the producer-side ceilings; the protocol
  and client paths retain the named budget comments used by the performance gate.
- Fold hiding changes the visible projection only. Text edits, canonical history,
  leases, package authority, and server document versions remain unchanged.
- Core folding does not require a `folds.scm` file. The generic Tree-sitter node
  walk is the current deliberate ceiling; syntax-specific fold queries can be
  added only as a reusable, bounded primitive rather than a per-language branch.

## Tests

- `src/server/folding.rs`:
  `folding_publish_round_trip_and_budget_deny`,
  `folding_stale_version_dropped`,
  `package_publish_without_render_folding_denied`, and
  `tree_walk_emits_only_multiline_named_nodes` cover permission, version,
  payload, provenance, and language-neutral tree-walk behavior.
- `src/editor/surface/mod.rs`:
  `toggle_fold_hides_and_restores_interior_lines` and
  `nested_parent_collapse_hides_child` cover local collapse, nested visibility,
  and restoration without document mutation.
- `tests/performance_budgets.rs::folding_and_inlay_payloads_deny_above_cap`
  locks the named folding budget in publication/propagation source paths.
- `tests/editor_intelligence_protocol.rs::folding_range_set_round_trips_through_codec_within_budget`
  locks the version-23 `FoldingRangeSet` codec envelope and bounded payload.
- `tests/editor_performance_invariants.rs` keeps folding publication out of
  paint/layout hot-path files and checks fold-related cache/hot-path policy.
- `tests/markdown_mode.rs` and `tests/primitives_docs.rs` cover package-facing
  contract/documentation linkage; `tests/decoration_transport.rs` covers the
  adjacent shared payload boundary.

Run focused coverage with:

```bash
cargo test --lib server::folding::tests
cargo test --lib editor::surface::tests::toggle_fold
cargo test --test protocol performance_budgets::folding_and_inlay_payloads_deny_above_cap
cargo test --test protocol primitives_docs::wiki_index_links_every_wiki_page
```

## Related

- [Editor Chrome and Layout Geometry](editor-chrome-and-layout.md) — chevron,
  inlay, layout-cache, and paint ownership.
- [Decoration Transport](decoration-transport.md) — shared bounded inert range
  transport for links and inlays.
- [Parse Coordinator](parse-coordinator.md) — background parse scheduling and
  `IncrementalParseUpdate` delivery.
- [Behavior Runtime Registration](behavior-runtime-registration.md) — package
  key routing and closed client-command backing.
- [Masonry Editor Widget Status Observability](masonry-editor.md) — command-ID
  facade and pane/editor event routing.
- [Publish Folding Ranges API](../../reference/clay-js-api/folding/server-publish-folding-ranges.md)
- [Primitive Registry](../../reference/primitives/registry.md#foldingrange)
- [Package Security](../../reference/primitives/package-security.md#unified-package-capability-model)
