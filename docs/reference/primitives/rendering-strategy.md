# Rendering Customization Strategy

Rendering customization is **server-validated inert declarations**. Packages may describe what should be rendered; the client decides how to render it (React + CodeMirror in the Tauri shell). No package JavaScript runs in client paint — no package code runs in render, layout, keypress, scroll, or pointer handlers. Plan 099 implements covered-range application through CodeMirror state fields and protocol v29 atomic viewport patches.

## Goals

- Let mode and package authors provide syntax highlighting, semantic emphasis, diagnostics, layout hints, render intents, and package UI contributions.
- Preserve the authority boundary from `.agents/skills/project-patterns/references/authority-boundaries.md`: the server validates package output; the client owns native rendering and local UI state.
- Preserve `.agents/skills/project-patterns/references/protocol-and-performance.md`: no full-document IPC for ordinary edits, no synchronous JavaScript/server round trip before normal typing is painted locally, no IPC work in render or input handlers, and viewport-bounded updates.

## Rendering Primitive Paths

| Path | Primitive | Status | Existing/New | Payload budget | Owner and validation |
| --- | --- | --- | --- | --- | --- |
| Inline decorations | `DecorationSet` members carried by `IncrementalParseUpdate` or `ViewportRenderPatch` | Implemented Phase 18.16/Plan 056 and Plan 099 | Existing protocol/client render hook plus atomic patch effect | `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` | Package/native grammar code produces spans server-side; one parse/capture pass fans out complete captures into stable 128-byte sets, and the server validates every member before either normal edit-driven publication or one request-scoped atomic patch. |
| Layout hints | `LayoutHintUpdate` or fields inside render-intent declarations | Planned/deferred until a concrete consumer needs it | New declaration shape | `DECORATION_PAYLOAD_BUDGET_BYTES` for editor-adjacent hints; `SDUI_*` budgets when represented as SDUI | Server validates bounded hint values; client maps them to known local layout affordances only. |
| Block/inline render intents | `RenderIntent` records for known intents such as preview block, code block adornment, emphasis band, or inline badge | Planned/deferred | New declaration shape, may share `DecorationUpdate` envelope initially | `DECORATION_PAYLOAD_BUDGET_BYTES` | Server validates intent kind/version and strips unknown executable data. Client renders only Rust-known intent kinds. |
| Panels/status/preview UI | `SduiTree` / `SduiTreeUpdate` | Exists/extend | Reuses `src/protocol/sdui.rs` and the `sdui.*` APIs | `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`, `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES` | Server validates inert SDUI trees and ships generation-stamped snapshots; the React renderer (`frontend/src/sdui`) reconciles stable node IDs in place. |
| Behavior-driven local rendering setup | Behavior manifest install | Exists/extend | Reuses behavior manifest path for deterministic local editor behavior, not for arbitrary painting | `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` | Server publishes inert behavior manifests; client executes Rust-known behavior engines only. |

## Decoration and atomic viewport patch shape

`DecorationSet` is the existing bounded inert publication shape for syntax,
semantic, search, link, and inlay data. Its UTF-8 byte ranges, document/version
metadata, package provenance, closed token vocabulary, and payload are validated
before client delivery. Parse output remains split into stable 128-byte sets;
that output fan-out never creates more parser work.

Plan 099 adds the internal protocol v29 envelope:

```text
ViewportRenderRequest {
  client_id, document_id, document_version,
  request_id, byte_start, byte_end, trace_id?
}
ViewportRenderPatch {
  request_id, document_id, document_version,
  status: Complete | Empty | Rejected,
  reason?, covered_ranges,
  decorations[], diagnostics[], folds[], trace_id?
}
```

The request is metadata-only and never carries document text. The server clamps
and validates its range, schedules the selected syntax session, and answers
exactly once. A complete patch's `covered_ranges` are derived from the output
members and therefore do not falsely claim the wider parser context. Tauri may
coalesce an obsolete whole patch per document, but sibling members remain
strictly ordered and are applied together by one client transaction. Empty and
rejected patches are terminal responses that free the request slot immediately.

Validation requirements remain:

- byte ranges are valid UTF-8-safe ranges for the target document version;
- kinds, token/modifier values, priorities, link targets, inlay labels, diagnostic
  fields, and fold ranges are closed, inert, and bounded;
- package provenance, document/version metadata, and publication permissions
  match the active server contribution;
- `DECORATION_PAYLOAD_BUDGET_BYTES`, the enclosing parse budget, and the
  protocol frame ceiling are checked before publication;
- no package callback, CSS string, renderer instruction, raw operation, or
  client-side JavaScript crosses the server/Tauri boundary.

## Semantic Typography Roles

Typography is inert rendering data governed by [Semantic Typography Roles](typography.md):

- mode `defaultFontRole` selects `monospace` or `proportional` for document text;
- syntax/semantic decoration `fontRole` may override an eligible byte range with `monospace` or `proportional`;
- text-bearing component `style.fontRole` selects `ui`, `monospace`, or `proportional`, defaulting to `ui`;
- packages never provide family names, stacks, font files/URLs/bytes/downloads, absolute sizes, raw renderer properties, CSS, or renderer callbacks.

The server validates semantic names, range/layer authorization, component-kind support, provenance, versions, UTF-8 boundaries, and payload bounds before client installation. The client normalizes visible decoration boundaries outside paint, resolves user-owned stacks/sizes through `TypographyRegistry`, and includes typography/style revisions plus document role in layout cache keys. Named-family fallback resolution is client-local; unavailable names retain a generic fallback without server font inspection or package notification.

Document line/scroll/caret geometry derives from resolved document profiles and the editor's shaped metrics. Shell/SDUI/component rendering, rows, hit regions, scrolling, status geometry, and accessibility bounds share resolved UI metrics (installed as `--clay-*` custom properties). Render, input, layout, pointer, and scroll paths perform no package JavaScript, blocking IPC, filesystem/network access, font download, or server-side installed-font discovery.

## Range Diagnostics

Range diagnostics are a distinct rendering path documented in [Range Diagnostics](diagnostics.md). They do not overload `DecorationSpan` metadata:

- `DiagnosticSet` publishes versioned, viewport-bounded, source-keyed `DiagnosticSpan` records with severity, code, message, and provenance;
- explicit analyzer packages and future LSP bridges share the `serverPublishDiagnostics` validation/transport/paint contract; Tree-sitter recovery nodes do not publish diagnostics;
- rendering draws theme-owned squiggles from the diagnostic style registry and cached line rectangles;
- Syntax, Semantic, Diagnostic, and Search layers remain additive; diagnostics cannot choose font roles or erase syntax/semantic styling;
- payload/count/cache limits use `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_MAX_SPANS_PER_SET`, and `DIAGNOSTIC_CACHE_BUDGET_BYTES`.

`DecorationKind::Diagnostic` remains a visual decoration tint only. Status-level `RuntimeDiagnostic` stays in chrome and never becomes an inline squiggle solely because it shares severity vocabulary.

## Layout Hints and Render Intents

Layout hints describe editor-adjacent presentation without granting native widget or GPU authority.

```rust
// Illustrative deferred shape; no package renderer callback is implied.
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

- Protocol shapes live in `src/protocol/sdui.rs`; server-side validation and
  composition live in `src/server/ui.rs` and `src/protocol/runtime.rs`
  (generation-stamped snapshots with host-stamped provenance/trust labels).
- The React renderer (`frontend/src/sdui`) reconciles stable node IDs in place
  — surviving DOM keeps state, and no package work runs inside render or event
  handlers. Slots (top/left/right/bottom/status), overlays, and empty-tab
  content are composed by shell-owned components
  (`frontend/src/shell/PackageWorkspace.tsx`).
- SDUI snapshots are bounded by `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES`; updates
  are bounded by `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`.

SDUI remains the package UI contribution path for panels, overlays, and
preview/status regions. It is not the inline syntax-highlighting path;
span-level editor decorations use `DecorationSet` and the atomic patch effect
so CodeMirror can map ranges directly into its text-layout pipeline.

## Client Rendering Attachment Points

Current client rendering splits between the CodeMirror editor host and the React
SDUI/package projection:

- `frontend/src/shell/workspace-controller.ts` applies server events outside
  render and routes document features to the owning pane session.
- `frontend/src/editor/ClayEditor.tsx` hosts CodeMirror; the shared
  `bytePositionField` converts UTF-8 protocol offsets, while
  `frontend/src/editor/extensions/render-patch.ts` carries one atomic effect.
- `decorations.ts`, `diagnostics.ts`, and `folding.ts` own CodeMirror state
  fields. They map a patch's bounded members once, replace only the declared
  authority/range, map retained items through local edits, and prune bounded
  overscan. Diagnostic lint state is synchronized in the same transaction.
- `frontend/src/sdui/SduiRenderer.tsx` reconciles the SDUI tree by stable node
  ID; `frontend/src/shell/PackageWorkspace.tsx` composes slots, overlays, and
  status items with visible provenance.

New inline decoration rendering maps validated spans to known CodeMirror mark,
widget, and link objects before dispatch. The hook must not allocate or parse
unbounded package data during render; all range validation, priority
resolution, stale-version checks, and viewport filtering happen server-side or
before the render commit.

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

- The client sends one metadata-only `ViewportRenderRequest` for the current
  visible byte range; a monotonic request ID and version guard stale replies.
- The server schedules one request-scoped job through the document's
  `SyntaxSession` and returns exactly one complete, empty, or rejected
  `ViewportRenderPatch`.
- A complete patch replaces only its declared covered range and same-authority
  items. Other package/layer state outside that range remains intact; an empty
  patch clears only that range.
- Ordinary edits do not trigger full-document decoration repaint or
  full-document IPC. Existing edit/open/resync frames remain separate from
  viewport completion.
- For large files, validated server chunks/cache entries are retained only
  within the syntax cache and near-viewport guard; client fields prune to the
  visible range plus bounded overscan.
- A newer document version, request, or runtime generation supersedes older
  work; stale output is discarded before client publication. Scroll rendering
  remains subject to `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` and the
  decoration/diagnostic/fold/SDUI payload budgets.

## Security Rules

Packages cannot:

- Inject arbitrary GPU draw calls.
- Mutate client component internals or DOM directly.
- Run synchronous JavaScript in client render, layout, keypress, pointer, or scroll handlers.
- Provide CSS, native widget callbacks, renderer scene callbacks, raw `Deno.core.ops`, or client-side JavaScript hooks.
- Bypass declared package prefix/provenance or payload budgets.
- Gain filesystem, network, shell, AI, WASM, or native-widget authority through rendering declarations.

All package rendering flows through validated server-produced declarations. The Rust client renders those declarations locally using known code paths only.

## Phase 18.5 Large-File Decoration-Cache Primitive

The Phase 18.5 [large-file Markdown primitive review](../../wiki/modules/phase18-large-file-markdown-primitive-review.md) records that decoration publication validates bounded `DecorationSet` chunks, while large-file modes need reusable chunk/cache primitives for retained near-viewport state. Runtime code treats each validated `DecorationSet` as a versioned decoration chunk; Plan 056 emits stable 128-byte chunks from one parse/capture pass rather than scheduling parser work per chunk.

Implemented reusable primitives:

- `DecorationChunkKey`: a validated chunk key with document ID, document version, package prefix, and byte range.
- `SyntaxChunkCache`: a bounded LRU-style server/runtime cache for syntax/decor chunks with stale-version separation, viewport/near-viewport pruning, and deterministic retained-byte accounting.
- Frontend CodeMirror state fields in `frontend/src/editor/extensions/{render-patch,decorations,diagnostics,folding}.ts`: store validated projected items outside paint, map them through local edits, replace exact covered authority, clear stale versions through reset/patch effects, and prune outside the bounded viewport guard.
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

One capture result is split into stable 128-byte `DecorationSet` outputs. Changed/visible-intersecting sets are ordered first; output chunk count does not increase parser/query invocation count. Each member is validated for package/layer identity, document/version/range/provenance, decoration payload, and enclosing incremental-update budget before any member publishes. Empty syntax sets clear their exact authoritative range. The frontend decoration, diagnostic, and folding state fields interpolate validated inert spans while a current result is pending; current authoritative package/layer sets subtract their exact half-open viewport from overlapping provisional items, preserving geometry outside authority and pruning only the bounded overscan guard.

Validation runs outside paint/key/text hot paths and remains bounded by `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, and the shared `SYNTAX_CACHE_BUDGET_BYTES` syntax-chunk cache:

1. Package metadata validation rejects raw CSS, raw color strings, unknown style tokens, native handles, raw ops, client JavaScript, and external/traversing grammar/query paths. Tier 2 assets must be package-root-confined `./grammars/*.wasm` and `./queries/*.scm` files.
2. Native descriptor/query construction and the web-tree-sitter host adapter compile/cache query state outside hot paths. Package-JS fallback handlers remain behind the existing server-issued parse-handler token.
3. The engine-neutral capture mapper rejects any capture without a `styleMap` entry, returning an actionable diagnostic such as an unmapped `@function` capture. It never contains language-name branches.
4. `DecorationSet` validation and payload checks run before insertion into the server `SyntaxChunkCache` or delivery to the existing decoration transport. Open itself returns before this work completes; failures become sanitized `RuntimeDiagnostic` values such as `parse.open_failed`. The frontend then applies projected items through one `applyRenderPatch` effect.

Invalid or unsupported queries, artifacts, or captures fail closed for that package: Clay keeps the document editable through its active major mode and publishes no syntax decorations for the failed grammar. Runtime performs no network fetch, shell/package-manager build, native-library load, or client-side JavaScript execution.

## Phase 26 rendering axes (implemented; pre-cutover paths historical)

Status: implemented in Phase 26.1–26.6 as theme/layout primitives — no new
`DecorationKind`, no package pixels, no render-path JS. The axes below were
first realized in the removed native editor and are carried by the current
React/CodeMirror client through the same theme/`editorRules` data:

| Axis | Primitive | Current carrier |
| --- | --- | --- |
| Opaque syntax colors | `StyleSpec.color` | `src/editor/theme.rs` → theme adapter → CodeMirror highlight style |
| Background fill | `StyleSpec.background` → visible text run background | CodeMirror decoration mark class from `textStyles` |
| Size ladder | `StyleSpec.scale` on Syntax/Semantic only | editor font-size classes from resolved typography |
| Editor chrome | `EditorChrome` (gutter / active line / indent guides / bracket match) | CodeMirror gutter/active-line/indentmark extensions (`frontend/src/editor/extensions`) |
| Wrap / insets | `WrapPolicy` + token-aligned insets | CodeMirror `EditorView.lineWrapping` + theme spacing variables |

`DecorationSpan` is unchanged (no background/scale fields). Themes contribute
`background` and `scale` through `textStyles`. Chrome and wrap are
`editorRules` data, not SDUI.

## Current Implementation References

- `frontend/src/editor/position-index.ts` provides the shared incremental
  UTF-16/UTF-8 index; `frontend/src/editor/extensions/render-patch.ts` provides
  the renderer-neutral atomic effect.
- `frontend/src/editor/extensions/{decorations,diagnostics,folding}.ts` own
  projected marks/links/inlays, diagnostics/lint, and fold ranges. They replace
  exact covered authority, map local edits, and retain only bounded overscan.
- `src/protocol/parse.rs::ViewportRenderPatch` and
  `src/server/connection/mod.rs` aggregate request-scoped members; the Tauri
  forwarder coalesces only obsolete whole patches.
- Package authors continue to publish validated inert data through documented
  server APIs. No package-facing renderer, parser, CSS, callback, or patch
  completion API is added by Plan 099.
- Reuse SDUI for Markdown preview/status panels instead of extending inline
  decoration payloads for panel UI.
