# Rendering Customization Strategy

Phase 16 defines rendering customization as **server-validated inert declarations**. Packages may describe what should be rendered, but the Rust client decides how to render it with Masonry, Parley, and Vello. No package JavaScript runs in client paint, layout, keypress, scroll, pointer, or text-event handlers.

This document is architecture-only. It introduces no runtime code in Phase 16.

## Goals

- Let mode and package authors provide syntax highlighting, semantic emphasis, diagnostics, layout hints, render intents, and package UI contributions.
- Preserve the authority boundary from `.agents/skills/project-patterns/references/authority-boundaries.md`: the server validates package output; the client owns native rendering and local UI state.
- Preserve `.agents/skills/project-patterns/references/protocol-and-performance.md`: no full-document IPC for ordinary edits, no synchronous JavaScript/server round trip before normal typing is painted, no IPC work in Masonry paint or text-event handlers, and viewport-bounded updates.

## Rendering Primitive Paths

| Path | Primitive | Status | Existing/New | Payload budget | Owner and validation |
| --- | --- | --- | --- | --- | --- |
| Inline decorations | `DecorationUpdate` containing `DecorationSpan` records | Planned for Phase 17/18 | New protocol/client render hook | `DECORATION_PAYLOAD_BUDGET_BYTES` | Package code may produce spans server-side; server validates schema, byte ranges, document version, provenance, priority, and payload size before sending to the client. |
| Layout hints | `LayoutHintUpdate` or fields inside render-intent declarations | Planned/deferred until a concrete consumer needs it | New declaration shape | `DECORATION_PAYLOAD_BUDGET_BYTES` for editor-adjacent hints; `SDUI_*` budgets when represented as SDUI | Server validates bounded hint values; client maps them to known local layout affordances only. |
| Block/inline render intents | `RenderIntent` records for known intents such as preview block, code block adornment, emphasis band, or inline badge | Planned/deferred | New declaration shape, may share `DecorationUpdate` envelope initially | `DECORATION_PAYLOAD_BUDGET_BYTES` | Server validates intent kind/version and strips unknown executable data. Client renders only Rust-known intent kinds. |
| Panels/status/preview UI | `SduiTree` / `SduiTreeUpdate` | Exists/extend | Reuses `src/protocol/sdui.rs`, `src/masonry_sdui.rs`, and `clay.sdui.*` APIs | `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` | Server validates inert SDUI trees; client applies snapshots/updates via `SduiNativeState::apply_snapshot` and `SduiNativeState::apply_update`. |
| Behavior-driven local rendering setup | Behavior manifest install | Exists/extend | Reuses behavior manifest path for deterministic local editor behavior, not for arbitrary painting | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | Server publishes inert behavior manifests; client executes Rust-known behavior engines only. |

## Decoration Span Shape

A decoration update is the primary Phase 18 path for Markdown syntax highlighting, semantic spans, diagnostics, and inline emphasis.

```rust
// Proposed documentation-only shape. Runtime code is deferred.
pub struct DecorationSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub kind: DecorationKind,
    pub style_token: String,
    pub priority: u8,
}

pub enum DecorationKind {
    Syntax,
    Semantic,
    Diagnostic,
    Emphasis,
    InlineCode,
    Link,
}

pub struct DecorationUpdate {
    pub document_id: DocumentId,
    pub document_version: u64,
    pub behavior_version: u64,
    pub render_intent_version: u64,
    pub package_prefix: String,
    pub viewport_byte_start: u64,
    pub viewport_byte_end: u64,
    pub spans: Vec<DecorationSpan>,
}
```

Validation requirements:

- `byte_start <= byte_end`; both must be valid for the target document version.
- Spans must intersect the declared viewport range unless the update is explicitly marked as cached non-visible data.
- `kind` must be a known enum value.
- `style_token` is a token such as `keyword.control`, `string.quoted`, `comment.line`, `punctuation.definition`, `markup.heading.1`, `diagnostic.error`, or `markup.inline-code`; it is not an arbitrary CSS string or draw callback. Phase 18 validation uses an explicit language-neutral allowlist so Markdown and non-Markdown packages can publish syntax spans without parser-specific Rust branches.
- `priority` controls deterministic overlap resolution. Higher priority wins when ranges overlap; equal priority preserves package/load order after server conflict resolution.
- The serialized update must be bounded by `DECORATION_PAYLOAD_BUDGET_BYTES`.
- `document_version`, `behavior_version`, `render_intent_version`, and `package_prefix` provide stale-update rejection and provenance.

## Layout Hints and Render Intents

Layout hints describe editor-adjacent presentation without granting native widget or GPU authority.

```rust
// Proposed documentation-only shape.
pub struct LayoutHint {
    pub byte_start: u64,
    pub byte_end: u64,
    pub intent: LayoutIntentKind,
    pub block_or_inline: RenderFlow,
    pub margin_class: Option<String>,
    pub emphasis_level: u8,
    pub priority: u8,
}
```

Allowed baseline fields:

- `block_or_inline`: `Block` or `Inline`; controls whether the hint attaches around a line/block range or inside text.
- `margin_class`: a bounded token such as `none`, `compact`, `normal`, or `spacious`; not arbitrary pixel/CSS input.
- `emphasis_level`: small bounded integer for known local styles.
- `intent`: known Rust-rendered intent such as `MarkdownCodeFence`, `MarkdownHeading`, `DiagnosticSquiggle`, or `InlineBadge`.

Unknown intents are ignored or rejected by the server according to the render-intent schema version. The client never imports package-supplied renderer functions.

## SDUI Contributions from Packages

Package-owned panels, status views, preview panes, buttons, and lists reuse the existing SDUI path:

- Protocol shapes live in `src/protocol/sdui.rs`.
- Client native state and application live in `src/masonry_sdui.rs` via `SduiNativeState::apply_snapshot`, `SduiNativeState::apply_update`, and `SduiNativeState::paint`.
- `src/masonry_editor.rs` receives `ClientConnectionEvent::SduiSnapshot` and `ClientConnectionEvent::SduiUpdate`, applies them outside paint handlers, and calls `self.sdui.paint(ctx, scene)` during widget painting.
- SDUI snapshots are bounded by `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`; updates are bounded by `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.

SDUI remains the package UI contribution path for panels and preview/status regions. It is not the inline syntax-highlighting path; span-level editor decorations use `DecorationUpdate` so the editor can map ranges directly into its text layout/rendering pipeline.

## Client Rendering Attachment Points

Current client rendering is split between the editor surface and SDUI:

- `src/masonry_editor.rs::EditorWidget::apply_connection_event` applies server events before paint. A future `DecorationUpdate` event should be handled here or in an editor-specific connection event path, then forwarded to `EditorSurface` as validated cached data.
- `src/masonry_editor.rs::EditorWidget::paint` fills the background, calls `self.editor.paint(ctx, scene)`, paints SDUI via `self.sdui.paint(ctx, scene)`, and paints the status line. No package work belongs in this function.
- `src/editor/surface.rs::EditorSurface::paint` computes visible lines and delegates text rendering to the layout layer. A future decoration hook should attach at the visible snapshot/layout-cache boundary so only visible spans are translated into Parley style ranges.
- `src/masonry_sdui.rs::SduiNativeState::paint` renders validated SDUI nodes with Masonry `render_text`, Parley style properties, and Vello `Scene` fills/text draws.

New inline decoration rendering should map validated spans to known Parley style attributes and Vello scene primitives inside Rust. The hook should not allocate or parse unbounded package data during paint; all range validation, priority resolution, stale-version checks, and viewport filtering happen before the next paint.

## Server-Side Compilation and Validation

Package rendering declarations are compiled server-side before publication:

1. Package load validates primitive schemas, package prefix, declared permissions, and render-intent schema version.
2. Background parse/render work may produce decoration, layout-hint, or SDUI declarations for a document version.
3. The server checks document version, behavior version, package provenance, byte ranges, known kinds/tokens, priority bounds, and serialized size.
4. The server filters or chunks declarations by visible viewport byte range.
5. The client receives only bounded, inert declarations.
6. The client applies the declarations to cached editor/SDUI state outside paint and text-event handlers.

Validation failures produce server diagnostics or package load/runtime errors. They must not panic the client and must not appear on the typing hot path.

## Viewport-Prioritized Incremental Updates

Rendering updates are incremental and viewport-prioritized:

- The server tracks the client's visible byte range and prioritizes spans intersecting that range.
- Ordinary edits do not trigger full-document decoration repaint or full-document IPC.
- For large files, off-viewport declarations may be cached server-side and sent when scrolling brings them near the viewport.
- If package parsing/render preparation lags behind local edits, the client keeps the last validated decorations for still-valid ranges and may clear stale ranges for the edited region after a server stale-version notice.
- A newer document version cancels or supersedes older background rendering work; stale updates are discarded before client publication.
- Scroll rendering must respect `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`; update payloads must stay within `DECORATION_PAYLOAD_BUDGET_BYTES`, `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, or `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` depending on path.

## Security Rules

Packages cannot:

- Inject arbitrary GPU draw calls.
- Mutate Masonry widgets directly.
- Run synchronous JavaScript in client Masonry paint, layout, keypress, pointer, scroll, or text-event handlers.
- Provide CSS, native widget callbacks, Vello scene callbacks, Parley builder callbacks, raw `Deno.core.ops`, or client-side JavaScript hooks.
- Bypass declared package prefix/provenance or payload budgets.
- Gain filesystem, network, shell, AI, WASM, or native-widget authority through rendering declarations.

All package rendering flows through validated server-produced declarations. The Rust client renders those declarations locally using known code paths only.

## Phase 18.5 Large-File Decoration-Cache Primitive

The Phase 18.5 [large-file Markdown primitive review](../../wiki/modules/phase18-large-file-markdown-primitive-review.md) records that decoration publication validates one viewport-bounded `DecorationSet`, while large-file modes need reusable chunk/cache primitives for retained near-viewport state. Runtime code now treats each validated `DecorationSet` as a versioned decoration chunk.

Implemented reusable primitives:

- `DecorationChunkKey`: a validated chunk key with document ID, document version, package prefix, and byte range.
- `SyntaxChunkCache`: a bounded LRU-style server/runtime cache for syntax/decor chunks with stale-version separation, viewport/near-viewport pruning, and deterministic retained-byte accounting.
- `EditorDecorationState`: a client-local chunk cache that stores validated chunks outside paint, clears stale chunks on document version changes, prunes chunks outside the near-viewport guard, and paints only spans intersecting the current visible snapshot.
- `SyntaxCacheBudget`: `SYNTAX_CACHE_BUDGET_BYTES` is the generic retained-cache budget. Phase 18.5 targets a 30 MiB Markdown-specific overhead cap for large-file workflows, but the primitive remains reusable by Python, Org, AsciiDoc, log-file, and other modes.

Security and performance rules:

- Cached chunks remain inert data: no parser tokens, AST nodes, CSS, draw callbacks, raw styles, native handles, client-side JavaScript, or executable closures.
- Chunk publication still enforces package provenance, document version, byte-range validation, style-token validation, stale-version rejection, and `DECORATION_PAYLOAD_BUDGET_BYTES` per transport payload.
- Paint consumes already-applied local spans only; cache eviction, chunk validation, parser execution, and package JavaScript remain outside paint, keypress, scroll, layout, and text-event handlers.
- Rust cache/renderer primitives do not branch on Markdown syntax or markdown-it token names; package adapters translate language-specific parser output to generic decoration chunks before publication.

## Phase 17/18 Follow-Up

- Extend chunk publication with explicit empty chunk-clearing metadata if package adapters need to clear an individual off-viewport package chunk without publishing replacement spans.
- Define concrete protocol messages for `DecorationUpdate` and, if needed, `LayoutHintUpdate`.
- Add client event handling outside paint handlers and store validated decorations in editor state.
- Add a Parley/Vello style-range application hook in the editor text layout path.
- Add payload-bound, stale-version, priority-overlap, and viewport-filtering tests.
- Reuse SDUI for Markdown preview/status panels instead of extending inline decoration payloads for panel UI.
