# Rendering Primitives

## Source

- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/package-security.md`
- `src/masonry_editor.rs`
- `src/masonry_sdui.rs`
- `src/editor/surface.rs`
- `src/protocol/sdui.rs`
- `src/perf/budgets.rs`
- `tests/primitives_docs.rs`

## Overview

Phase 16 defines package-controlled rendering as server-validated inert declarations. Packages can describe syntax spans, semantic spans, diagnostics, layout hints, render intents, and SDUI panels, but the Rust client performs all native rendering locally with known Masonry, Parley, and Vello code paths.

The canonical public design is `docs/reference/primitives/rendering-strategy.md`; this wiki page explains the internal attachment points and why inline editor decoration is separated from SDUI panel rendering.

## Responsibilities

- Explain the split between inline editor decorations and SDUI panel/status/preview contributions.
- Document the implemented bounded `DecorationSet` and `DecorationSpan` flow and the remaining future rendering primitive scope.
- Identify where future client rendering hooks should attach outside paint and text-event handlers.
- Record the performance and security constraints that keep package rendering off the hot path.

## How It Works

Rendering customization has two primary paths:

1. **Inline editor decorations** use the implemented `DecorationSet`/`DecorationSpan` protocol shape. Package JavaScript may produce span data server-side, but the server validates byte ranges, document version, package provenance, known decoration kinds, style tokens, priority, viewport range, and payload size before client delivery.
2. **Panels, status UI, previews, buttons, and lists** reuse the existing SDUI path. Package code publishes inert SDUI trees server-side; `src/masonry_sdui.rs` applies validated snapshots/updates and renders native widgets locally.

The split avoids forcing text highlighting through SDUI. Inline spans need to attach near editor text layout so Parley style ranges and Vello draws are computed for visible text only. SDUI remains better suited for block-level UI, preview panels, status regions, and actions routed back to server commands.

## Decoration Shape

Phase 17 implements the bounded inline decoration protocol as `DecorationSet`/`DecorationSpan` in `src/protocol/decorations.rs`:

```rust
pub struct DecorationSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub kind: DecorationKind,
    pub style_token: String,
    pub priority: u8,
}

pub struct DecorationSet {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub viewport_byte_start: u64,
    pub viewport_byte_end: u64,
    pub spans: Vec<DecorationSpan>,
}
```

Important fields:

- `document_version` rejects stale decorations.
- Per-span `DecorationProvenance` preserves package name/version/prefix for diagnostics and overlap handling.
- `viewport_byte_start` and `viewport_byte_end` ensure ordinary updates are viewport-bounded.
- `style_token` is a known token such as `markup.heading.1` or `diagnostic.error`, not CSS, a callback, or a draw command.

## Client Attachment Points

The implemented render hook preserves these boundaries:

- `src/masonry_editor.rs::EditorWidget::apply_connection_event` receives `ClientConnectionEvent::DecorationSet` and stores validated updates before paint.
- `src/editor/surface.rs::EditorSurface` holds current document/version-matched decoration state near visible snapshot/layout-cache computation.
- `src/editor/layout.rs::LayoutState::paint_text` fills native highlight rectangles for visible decoration ranges before text rendering; future richer text styling can extend this Parley/Vello boundary.
- `src/masonry_editor.rs::EditorWidget::paint` does not run package JavaScript, validate large payloads, or parse package declarations. It only renders already-applied state.
- `src/masonry_sdui.rs::SduiNativeState::apply_snapshot` and `apply_update` remain the SDUI panel/status application path, while `SduiNativeState::paint` renders the validated native UI tree.

## Viewport and Incremental Policy

Rendering updates are expected to be viewport-prioritized:

- The server sends spans that intersect the visible range first.
- Adjacent/off-viewport decorations may be cached and sent later, especially on scroll.
- Ordinary edits must not trigger full-document decoration IPC or full-document repaint.
- If parsing/render preparation lags, the client may keep last-known valid decorations for unaffected ranges or clear stale edited ranges after a server notice.

Payloads use the advisory and existing budget constants:

- `DECORATION_PAYLOAD_BUDGET_BYTES` for inline decoration updates.
- `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` and `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` for SDUI trees/updates.
- `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` for scroll-adjacent render latency.

## Invariants and Constraints

- Rendering declarations are inert data. They cannot contain GPU callbacks, Vello scene callbacks, Parley builder callbacks, CSS/script injection, native handles, raw ops, or client-side JavaScript.
- The server validates schema, `render-decorations` permission, package provenance, version metadata, byte ranges, inert style tokens, and serialized payload size before publication.
- The client maps known decoration/style tokens to local Rust-rendered highlight styles only and ignores mismatched document/version updates.
- SDUI is for package UI contributions; inline syntax/semantic text styling should use decoration ranges.
- Validation failures become package diagnostics or runtime errors, never client panics and never typing-hot-path work.

## Tests

- `tests/primitives_docs.rs::rendering_strategy_doc_linked_from_index`: verifies the reference rendering strategy remains linked from `docs/index.md`.
- `tests/primitives_docs.rs::rendering_strategy_covers_inert_client_rendering_contract`: checks for decoration shapes, layout hints, render intent versioning, server validation, client attachment points, Parley/Vello references, and security prohibitions.
- `tests/primitives_docs.rs::rendering_strategy_references_payload_budgets`: checks decoration and SDUI budget references plus viewport-prioritized updates.
- `tests/decoration_transport.rs`: validates payload budget rejection, stale version rejection, codec round trip, and the client render hook.
- `cargo test --test primitives_docs` and `cargo test --test decoration_transport`: run the documentation and implementation coverage.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Decoration Transport](decoration-transport.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
