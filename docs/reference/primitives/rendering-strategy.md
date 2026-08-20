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
| Inline decorations | `DecorationSet` chunks carried by `IncrementalParseUpdate::decoration_updates` | Implemented Phase 18.16/Plan 056 | Existing protocol/client render hook | `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` | Package/native grammar code produces spans server-side; one parse/capture pass fans out complete captures into stable 128-byte sets, and the server validates every member's schema, byte ranges, document version, provenance, priority, and payload before streaming normal decoration messages. |
| Layout hints | `LayoutHintUpdate` or fields inside render-intent declarations | Planned/deferred until a concrete consumer needs it | New declaration shape | `DECORATION_PAYLOAD_BUDGET_BYTES` for editor-adjacent hints; `SDUI_*` budgets when represented as SDUI | Server validates bounded hint values; client maps them to known local layout affordances only. |
| Block/inline render intents | `RenderIntent` records for known intents such as preview block, code block adornment, emphasis band, or inline badge | Planned/deferred | New declaration shape, may share `DecorationUpdate` envelope initially | `DECORATION_PAYLOAD_BUDGET_BYTES` | Server validates intent kind/version and strips unknown executable data. Client renders only Rust-known intent kinds. |
| Panels/status/preview UI | `SduiTree` / `SduiTreeUpdate` | Exists/extend | Reuses `src/protocol/sdui.rs`, `src/masonry_sdui.rs`, `src/masonry_sdui_region.rs`, and `sdui.*` APIs | `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` | Server validates inert SDUI trees; client applies snapshots/updates via `SduiNativeState::apply_snapshot`/`apply_update`, which feed a retained reconciled Masonry subtree (`SduiRegionWidget`) hosted as a child of `EditorWidget`. |
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

## Semantic Typography Roles

Typography is inert rendering data governed by [Semantic Typography Roles](typography.md):

- mode `defaultFontRole` selects `monospace` or `proportional` for document text;
- syntax/semantic decoration `fontRole` may override an eligible byte range with `monospace` or `proportional`;
- text-bearing component `style.fontRole` selects `ui`, `monospace`, or `proportional`, defaulting to `ui`;
- packages never provide family names, stacks, font files/URLs/bytes/downloads, absolute sizes, raw Parley properties, CSS, or renderer callbacks.

The server validates semantic names, range/layer authorization, component-kind support, provenance, versions, UTF-8 boundaries, and payload bounds before client installation. The client normalizes visible decoration boundaries outside paint, resolves user-owned stacks/sizes through `TypographyRegistry`, and includes typography/style revisions plus document role in layout cache keys. Named-family fallback resolution is client-local; unavailable names retain a generic fallback without server font inspection or package notification.

Document line/scroll/caret geometry derives from resolved document profiles and shaped Parley metrics. Shell/SDUI/component paint, rows, hit regions, scrolling, status geometry, and accessibility bounds share resolved UI metrics. Paint, input, layout, pointer, scroll, and text-event paths perform no package JavaScript, blocking IPC, filesystem/network access, font download, or server-side installed-font discovery.

## Range Diagnostics

Range diagnostics are a distinct rendering path documented in [Range Diagnostics](diagnostics.md). They do not overload `DecorationSpan` metadata:

- `DiagnosticSet` publishes versioned, viewport-bounded, source-keyed `DiagnosticSpan` records with severity, code, message, and provenance;
- explicit analyzer packages and future LSP bridges share the `serverPublishDiagnostics` validation/transport/paint contract; Tree-sitter recovery nodes do not publish diagnostics;
- paint draws theme-owned squiggles from `StyleRegistry::diagnostic_style` and cached Parley line rectangles;
- Syntax, Semantic, Diagnostic, and Search layers remain additive; diagnostics cannot choose font roles or erase syntax/semantic styling;
- payload/count/cache limits use `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_MAX_SPANS_PER_SET`, and `DIAGNOSTIC_CACHE_BUDGET_BYTES`.

`DecorationKind::Diagnostic` remains a visual decoration tint only. Status-level `RuntimeDiagnostic` stays in chrome and never becomes an inline squiggle solely because it shares severity vocabulary.

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
- Client native state lives in `src/masonry_sdui.rs` (`SduiNativeState`); snapshots/updates are applied via `SduiNativeState::apply_snapshot`/`apply_update` **outside paint handlers**.
- Rendering is a retained reconciled Masonry subtree, not an immediate-mode paint pass: `SduiRegionWidget` (`src/masonry_sdui_region.rs`) diffs the SDUI tree by stable `SduiNodeId`, reuses surviving `WidgetPod`s in place, and maps each kind to a real Masonry widget — `SduiLabel`, `SduiButton`, `SduiListRow`, `EditorViewWidget` (a binding/slot component), under a Clay-owned `SduiScrollViewport`. It is hosted as a real child of `EditorWidget`, so Masonry routes layout, paint, pointer, focus, scroll, and a11y through the standard widget tree.
- `src/masonry_editor.rs` receives `ClientConnectionEvent::SduiSnapshot`/`SduiUpdate` in `apply_connection_event` and syncs the region in place (stable identity preserved across updates); no package work runs in `paint`/`layout`/event handlers.
- SDUI snapshots are bounded by `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`; updates are bounded by `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.

The earlier "temporary compatibility bridge" status (immediate-mode `SduiNativeState::paint`) is retired; the sidebar panel chrome is painted by `SduiRegionWidget` itself. SDUI remains the package UI contribution path for panels and preview/status regions. It is not the inline syntax-highlighting path; span-level editor decorations use `DecorationUpdate` so the editor can map ranges directly into its text layout/rendering pipeline.

## Client Rendering Attachment Points

Current client rendering is split between the editor surface and the retained SDUI/package-UI widget tree:

- `src/masonry_editor.rs::EditorWidget::apply_connection_event` applies server events before paint and syncs the hosted child widgets (`SduiRegionWidget`, `PackagePanelHost`, `PackageOverlayHost`) in place via `MutateCtx`. A future `DecorationUpdate` event should be handled here or in an editor-specific connection event path, then forwarded to `EditorSurface` as validated cached data.
- `src/masonry_editor.rs::EditorWidget::paint` fills the background and paints the editor canvas (`self.editor.paint_in_rect`); the SDUI sidebar, fixed package panels, and transient overlays render through hosted Masonry child widgets (children pass, after `paint`); `post_paint` draws the status line. No package work belongs in this function.
- `src/editor/surface/mod.rs::EditorSurface::paint` computes visible lines and delegates text rendering to the layout layer. A future decoration hook should attach at the visible snapshot/layout-cache boundary so only visible spans are translated into Parley style ranges.
- `src/masonry_sdui_region.rs::SduiRegionWidget` reconciles the SDUI tree into retained Masonry widgets (`SduiLabel`/`SduiButton`/`SduiListRow`/`EditorViewWidget` under `SduiScrollViewport`) which paint themselves with Parley text layout and Vello `Scene` fills/text draws; sidebar chrome is painted by the region widget. `SduiNativeState` (`src/masonry_sdui.rs`) holds inert validated state and drives reconciliation; it no longer has an immediate-mode paint path.

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

The Phase 18.5 [large-file Markdown primitive review](../../wiki/modules/phase18-large-file-markdown-primitive-review.md) records that decoration publication validates bounded `DecorationSet` chunks, while large-file modes need reusable chunk/cache primitives for retained near-viewport state. Runtime code treats each validated `DecorationSet` as a versioned decoration chunk; Plan 056 emits stable 128-byte chunks from one parse/capture pass rather than scheduling parser work per chunk.

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

## Phase 18.16/Plan 056 Tiered Syntax Capture and Vocabulary Validation

`SyntaxGrammarContribution` packages and native first-party descriptors translate captures from Tier 1 native tree-sitter, Tier 2 web-tree-sitter, or Tier 3 package-JS adapters through one `Background` no-hot-path parse/decor path before any `DecorationSet` reaches the client. `setSyntaxEnginePreference` selects a tier only at init/package-load/open/reclassification time; package load order cannot silently replace Tier 1. The package declares a `styleMap` such as:

```json
{
  "keyword": "keyword.control",
  "string": "string.quoted",
  "comment": "comment.line",
  "punctuation": "punctuation.definition"
}
```

The shared mapper converts the selected style token to the Phase 18.15 `TokenType` + `Modifiers` axes and preserves package scope/provenance. For a consecutive accepted edit, the matching stable-window tree is edited exactly once, parsed once, and queried over the full envelope covering a shared 128-byte UTF-8-safe replacement-chunk grid (`replacement_ranges` from Tree-sitter changed ranges plus explicit invalidations) — so query coverage and replacement coverage are identical. Complete intersecting captures survive the query and are clipped at exact chunk boundaries; grammar-owned token/comment/string/prose/code boundaries remain authoritative.

One capture result is split into stable 128-byte `DecorationSet` outputs. Changed/visible-intersecting sets are ordered first; output chunk count does not increase parser/query invocation count. Each member is validated for package/layer identity, document/version/range/provenance, decoration payload, and enclosing incremental-update budget before any member publishes. Empty syntax sets clear their exact authoritative range. `EditorDecorationState` may interpolate validated inert spans while a current result is pending; current authoritative package/layer sets subtract their exact half-open viewport from overlapping provisional chunks, preserving left/right span fragments outside authority and locally coalescing compatible residual chunks/spans.

Validation runs outside paint/key/text hot paths and remains bounded by `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, and the shared `SYNTAX_CACHE_BUDGET_BYTES` syntax-chunk cache:

1. Package metadata validation rejects raw CSS, raw color strings, unknown style tokens, native handles, raw ops, client JavaScript, and external/traversing grammar/query paths. Tier 2 assets must be package-root-confined `./grammars/*.wasm` and `./queries/*.scm` files.
2. Native descriptor/query construction and the web-tree-sitter host adapter compile/cache query state outside hot paths. Package-JS fallback handlers remain behind the existing server-issued parse-handler token.
3. The engine-neutral capture mapper rejects any capture without a `styleMap` entry, returning an actionable diagnostic such as an unmapped `@function` capture. It never contains language-name branches.
4. `DecorationSet` validation and payload checks run before insertion into `SyntaxChunkCache` or delivery to the existing decoration transport. Open itself returns before this work completes; failures become sanitized `RuntimeDiagnostic` values such as `parse.open_failed`.

Invalid or unsupported queries, artifacts, or captures fail closed for that package: Clay keeps the document editable through its active major mode and publishes no syntax decorations for the failed grammar. Runtime performs no network fetch, shell/package-manager build, native-library load, or client-side JavaScript execution.

## Phase 26 rendering axes (implemented)

Status: **implemented** in Phase 26.1–26.6. These stay theme/layout primitives — no new `DecorationKind`, no package pixels, no paint-path JS.

| Axis | Primitive | Where |
| --- | --- | --- |
| Opaque syntax colors | `StyleSpec.color` | `src/editor/theme.rs` |
| Background fill | `StyleSpec.background` → `VisibleTextStyleRun.background`, painted before glyphs | `src/editor/layout.rs` |
| Size ladder | `StyleSpec.scale` on Syntax/Semantic only | `src/editor/layout.rs` FontSize |
| Editor chrome | `EditorChrome` (gutter / active line / indent guides / bracket match) | `src/editor/surface/chrome.rs` |
| Wrap / insets | `WrapPolicy` + token-aligned insets | `src/editor/surface/mod.rs` |

`DecorationSpan` is unchanged (no rkyv background/scale fields). Themes contribute `background` and `scale` through `textStyles`. Chrome and wrap are `editorRules` data, not SDUI.

## Phase 17/18 Follow-Up

- Extend chunk publication with explicit empty chunk-clearing metadata if package adapters need to clear an individual off-viewport package chunk without publishing replacement spans.
- Define concrete protocol messages for `DecorationUpdate` and, if needed, `LayoutHintUpdate`.
- Add client event handling outside paint handlers and store validated decorations in editor state.
- Add a Parley/Vello style-range application hook in the editor text layout path.
- Add payload-bound, stale-version, priority-overlap, and viewport-filtering tests.
- Reuse SDUI for Markdown preview/status panels instead of extending inline decoration payloads for panel UI.
