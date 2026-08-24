# Editor Chrome and Layout Geometry

> **Historical (native editor removed in Plan 097 Phase 12).** The chrome/layout
> contract is carried by the React editor extensions and theme adapter:
> [React CodeMirror Editor](react-codemirror-editor.md). Protocol shapes and
> server-side validation referenced below remain current.

## Source

- `src/protocol/mod.rs` — `EditorChrome` (including `inlay_hints`), `WrapPolicy`, `EditorLayoutRules`, `EditorBehaviorRules.chrome`/`layout`.
- `src/server/ops/modes.rs` — `parse_chrome`, `parse_layout`, `clamp_column`.
- `src/editor/surface/chrome.rs` — chrome resolution, bracket scan, gutter paint.
- `src/editor/surface/mod.rs` — `EditorSurface` chrome/layout state, `set_editor_layout`, `scroll_horizontal_pixels`, asymmetric insets, wrap-aware overscan.
- `src/editor/layout.rs` — `TextFrame`, `TextChromeLayers`, `wrap_width_for_layout`, `layout_max_width`, chrome fills in `paint_text`.
- `src/editor/viewport.rs` — `Viewport::set_overscan_lines`.
- `src/editor/theme.rs` — chrome color keys (`GutterFg`, `GutterFgActive`, `LineHighlight`, `IndentGuide`, `BracketMatch`).
- `src/server/ops/editor.rs` — `op_clay_editor_set_editor_layout` (trusted-extension-only).
- `src/server/js_runtime/mod.rs` — `editor_layout_publisher`/`editor_layout_state`, `subscribe_editor_layout`, `editor_layout_override`.
- `src/server/connection/mod.rs` — handshake `EditorLayoutOverride` + live broadcast.
- `src/client/mod.rs` — `ClientConnectionEvent::EditorLayoutOverride`.
- `src/masonry_pane_document.rs` — per-pane `set_editor_layout` application.
- `runtime/js/editor.js` — `clientSetEditorLayout` facade.
- `src/perf/budgets.rs` — chrome/decoration advisory paint budgets.
- Tests: `src/editor/surface/mod.rs`, `src/editor/surface/chrome.rs`, `src/editor/layout.rs`, `src/editor/theme.rs`, `src/server/js_runtime/mod.rs`, `tests/editor_performance_invariants.rs`.
- Authoritative public API: [`editor.clientSetEditorLayout`](../../reference/clay-js-api/editor/client-set-editor-layout.md).
- Authoritative package contract: [Creating Clay Packages — editor chrome and layout](../../reference/packages/creating-packages.md).

## Overview

Phase 26.5 (editor chrome), Phase 26.6 (layout geometry), and Phase 28
(editor commands, folding, links, and inlay hints) turn the editor's surroundings
and wrapping behavior into generic, mode-declared primitives. Chrome — the
line-number gutter, active-line highlight, indent guides, bracket-match
highlight, fold chevrons, link tooltip, and inlay visibility default — is a
visual/overlay layer derived from validated manifest/decor data and cached
geometry; it never enters the layout cache key except the required fold
revision and never runs package code. Layout geometry — asymmetric token-driven
insets, a `WrapPolicy` (none/viewport/column), and horizontal scrolling for
unwrapped code — is mode-declared manifest data with a user-facing runtime
override.

Both surfaces follow the same authority model as caret styling: packages contribute **inert manifest data** (`editorRules.chrome`, `editorRules.layout`), the client resolves it against theme tokens, and no package capability grants chrome-shape or wrap-policy override authority. The only runtime override is the user's `clientSetEditorLayout`, whose op is registered in the trusted extension domain only, so third-party workers cannot forge it.

## Editor chrome

### Declaration and defaults

`EditorChrome { gutter, active_line, indent_guides, bracket_match, inlay_hints }` travels in `EditorBehaviorRules.chrome: Option<EditorChrome>`. Constructors:

- `EditorChrome::prose()` — all five off (Markdown, `core.text`).
- `EditorChrome::code()` — all five on (Rust, TypeScript, JavaScript, `core.code`).
- `EditorChrome::from_font_role(DocumentFontRole)` — `Monospace` → code, `Proportional`/`Inherit` → prose.

`parse_chrome` (`src/server/ops/modes.rs`) reads `editorRules.chrome` JSON keys `gutter`/`activeLine`/`indentGuides`/`bracketMatch`/`inlayHints`. Omitted chrome defaults to prose for backward compatibility, and to code when `document_font_role == Monospace` at publish time; an explicit `chrome` object always wins. `runtime/js/behavior.js::buildCodeEditingManifest` forwards an optional `chrome` object from language package options.

### Resolution and paint

`src/editor/surface/chrome.rs` owns the chrome layer:

- `resolved_chrome()` — manifest chrome, else `EditorChrome::from_font_role(document_font_role)`.
- `visible_caret_offsets()` — caret byte offsets for the active-line highlight.
- `visible_bracket_ranges()` — scans the active `BehaviorManifest.pairs` via `matching_pair_byte_within` (src/editor/buffer.rs) with a `BRACKET_MATCH_SCAN_BYTES = 64 KiB` ceiling in both directions, so bracket matching is bounded even on huge lines.
- `indent_tab_width()` — the manifest indentation tab width for indent-guide columns.
- `paint_gutter()` — right-aligned line numbers using `gutter_number_origin_x` (right edge minus number width minus `GUTTER_PAD = 8.0`), monospace profile, `gutter_foreground` for normal lines and `gutter_foreground_active` for the current line; it also paints validated fold chevrons through `paint_fold_chevron`.
- `paint_inlay_overlays()` — renders bounded `InlayHintPayload` labels after the main Parley token layout, using muted editor color and no virtual-text reflow.

`LayoutState::paint_text` receives a `TextChromeLayers { active_line_offsets, active_line_color, bracket_ranges, bracket_color, indent_tab, indent_color }` and paints, inside the text clip layer and **before** selection rects and glyphs:

1. gutter fill (drawn by `paint_gutter` in `src/editor/surface/mod.rs` after `paint_text` returns metrics, as a visual overlay on the left inset area),
2. bracket-match rect fills,
3. active-line highlight rect,
4. indent-guide segments (computed by `indent_guide_segments` from parley `Cursor::from_byte_index` geometry),
5. selection rects,
6. decoration background fills,
7. text.

Chrome colors resolve from five `BaseUiColorKey` variants added in Phase 26.5 — `GutterFg`, `GutterFgActive`, `LineHighlight`, `IndentGuide`, `BracketMatch` — with `clay_default()` values (gutter `0x8d86a3`, gutter-active `0xf4f1ff`, line highlight `0xffffff12`, indent guide `0xffffff22`, bracket match `0x8a6fff55`). Theme packages override them through the same inert `textStyles` entries as every other base key.

### Hot-path policy

Chrome is deliberately **not** in `LayoutCacheKey`: gutter and active-line are visual overlays derived from existing cached `CaretCell` metrics (`x`, `line_top`, `line_bottom`), so toggling chrome never triggers a layout rebuild. Bracket scanning is capped at 64 KiB per direction; indent guides reuse cached line geometry. Advisory paint budgets in `src/perf/budgets.rs`: `GUTTER_PAINT_P95_BUDGET_MS = 2`, `ACTIVE_LINE_PAINT_P95_BUDGET_MS = 1`, `BRACKET_MATCH_PAINT_P95_BUDGET_MS = 1`, `DECORATION_BACKGROUND_FILL_P95_BUDGET_MS = 2`; a compile-time assertion in `tests/editor_performance_invariants.rs` verifies their sum fits inside `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` (16 ms).

## Phase 28 editor-intelligence chrome and intent

`EditorSurface` owns the client-local fold set and `paint_gutter` hides/marks
validated ranges; `fold_revision` is the only fold-related addition to
`LayoutCacheKey`. `MasonryPaneDocument` resolves visible Link spans through
`decoration_target_at`, applies `DecorationIntent::{Hover, Activate}`, and
paints hover with the separate `paint_link_hover`/`paint_tooltip_shell` path so
completion and command menus are not stolen. Workspace/document targets are
resolved by Clay; display-only/unsafe targets never open and activation never
mints a browse grant.

Inlay data remains a decoration overlay. `paint_inlay_overlays` is viewport
bounded, decorative/`aria-hidden`, and can be toggled with
`editor.toggleInlayHints`; it does not alter the main text layout. All three
surfaces consume cached inert protocol data. Package JavaScript, parsing,
network, filesystem, and blocking IPC stay off keypress, paint, layout,
pointer, scroll, and text-event paths.

## Layout geometry

### Asymmetric insets

`TEXT_INSET` (48.0) was replaced by three constants in `src/editor/surface/mod.rs`:

- `TEXT_INSET = 32.0` — horizontal inset without gutter (spacing.xl).
- `TEXT_INSET_GUTTER = 48.0` — horizontal inset with gutter (spacing.xxl).
- `TEXT_INSET_Y = 20.0` — vertical inset (spacing.md).

`inset_x()` returns the gutter variant when chrome gutter is on; `inset_y()` returns `TEXT_INSET_Y`. These are Clay-owned constants derived from the spacing token scale — they are not SDUI tokens and packages cannot override them.

### Wrap policy

`WrapPolicy` (`src/protocol/mod.rs`) has three variants:

- `None` — no wrapping; the document can scroll horizontally. Code default.
- `Viewport` — wrap at the pane content width. Historical default.
- `Column(u16)` — wrap at `min(pane_width, column_cap * average_advance)`, default cap 72. Prose mode.

`EditorLayoutRules { wrap: WrapPolicy }` travels in `EditorBehaviorRules.layout: Option<EditorLayoutRules>`. `parse_layout` reads `wrapPolicy` (`"none"`/`"viewport"`/`"column"`, deny-by-default) and optional `columnCap` clamped to `MIN_COLUMN(16)..=MAX_COLUMN(240)` via `clamp_column`. When omitted, `resolved_wrap()` falls back to `WrapPolicy::from_font_role` (Monospace → `None`, Proportional → `Column(72)`).

`wrap_width_for_layout` (`src/editor/layout.rs`) converts the policy into parley's `break_all_lines` `Option<f32>`: `None` disables wrapping (full-width horizontal scroll), `Some(width)` wraps. `layout_max_width(pane_width)` computes the content width per policy (`f32::MAX` for `None`). `LayoutCacheKey` includes the resolved wrap policy and width, so a wrap change invalidates the cached layout.

### Horizontal scrolling

`EditorSurface` owns `visual_scroll_x` and `last_visual_max_scroll_x` (reset on `load_snapshot`, `load_resync_snapshot`, `set_typography`, and wrap changes). `scroll_horizontal_pixels(delta)` is a no-op for `Viewport`/`Column` and clamps to `last_visual_max_scroll_x` for `None`. `VisualLayoutMetrics.width` comes from `layout.full_width()` so the scrollbar reflects the unwrapped content width. `TextFrame { inset_x, inset_y, scroll_x, clip_width }` carries the geometry into `paint_text`; the clip rect stays at pane bounds while content translates by `-scroll_x`.

### Viewport metrics

`update_visible_line_count_for_height` uses wrap-aware overscan: 4 overscan lines for `WrapPolicy::None`, 12 for `Viewport`/`Column` (wrapped prose needs more lookahead). `Viewport::set_overscan_lines` bumps the viewport revision. `document_line_height()` remains the conservative shared baseline for visible-line estimation and scrollbar progress; Parley supplies exact rendered metrics.

### User override transport

`clientSetEditorLayout({ wrapPolicy, columnCap })` (`runtime/js/editor.js`) is the only runtime override. The full transport mirrors the caret-style pattern:

1. `op_clay_editor_set_editor_layout` (`src/server/ops/editor.rs`) validates the value (deny-by-default `wrapPolicy`, clamped `columnCap`) and requires `require_editor_control`; it is registered **only** in `clay_runtime_trusted_extension`, so third-party package workers cannot resolve it.
2. `ClayOpState`/`ClayJsRuntimeService` hold `editor_layout_publisher` + `editor_layout_state`; `publish_editor_layout_override` writes both.
3. The connection handshake subscribes and sends `ServerMessage::EditorLayoutOverride(Option<WrapPolicy>)` (protocol v18) after `CaretStyleOverride`; live changes broadcast with lag-replay fallback.
4. `ClientConnectionEvent::EditorLayoutOverride` routes to `PaneDocumentView` → `EditorSurface::set_editor_layout(wrap)`, which updates `layout_override`, resets horizontal scroll, and bumps `layout_style_revision`.

Resolution order: runtime override → per-mode `editorRules.layout.wrap` → `WrapPolicy::from_font_role`. Packages cannot clear a user override.

`editor.toggleInlayHints` is a bindable client-local command. It flips an
`EditorSurface` override over `EditorChrome.inlay_hints`; the overlay paint
uses inert decoration payloads and does not reflow the main Parley layout.
Code chrome defaults inlays on, prose chrome defaults them off, and a user
binding changes only the local visibility decision.

## Security

- Chrome and layout are inert manifest data inside existing `EditorBehaviorRules` validation; no package capability grants caret-shape, chrome-shape, or wrap-policy override authority.
- `clientSetEditorLayout` is trusted-extension-only: third-party workers get `op-not-found` even though the `clay:editor` facade is importable.
- Backgrounds and scales (Phase 26.3/26.4) are inert theme data validated by the existing `textStyles` parser (known keys, valid hex, finite scale in `(0, 4.0]`, no executable/raw-CSS/native authority fields).
- Bracket scanning is bounded (64 KiB ceiling) and runs on the client against cached manifest pairs — no server IPC, no package JavaScript in paint.

## Tests

- `src/editor/surface/chrome.rs`: chrome defaults from font role, explicit override, inlay defaults, indent-guide columns, gutter right-alignment.
- `src/editor/surface/mod.rs`: `toggle_inlay_hides_overlay`, `wrap_defaults_follow_document_font_role`, `user_wrap_override_beats_manifest`, `column_wrap_is_narrower_than_viewport`, `horizontal_scroll_only_applies_when_unwrapped`, `insets_are_asymmetric_and_gutter_widens_left`, `horizontal_scroll_does_not_change_layout_cache_key`, search-match/quote background join tests.
- `src/editor/layout.rs`: `style_run_backgrounds_paint_before_glyphs`, `heading_scale_increases_parley_line_height`, chrome underlay paint order.
- `src/editor/theme.rs`: `style_for_resolves_theme_owned_backgrounds`, `text_style_overrides_can_set_background_axis`, `size_scale_ladder_descends_headings_and_clamps_theme_overrides`.
- `src/editor/buffer.rs`: `matching_pair_within_stops_at_byte_ceiling`.
- `src/server/js_runtime/mod.rs`: `editor_layout_config_eval_stays_within_hard_timeout`.
- `tests/editor_performance_invariants.rs`: no-hardcoded-Color source guard includes `src/editor/surface/chrome.rs`; chrome budget sum assertion; height/line-count geometry constants updated for `TEXT_INSET_Y`.
- Commands: `cargo test --lib editor`, `cargo test --test editor editor_performance_invariants::`, `cargo test --test protocol`.

## Related

- [Editor Theme Registry](editor-theme-registry.md) — chrome color keys, background axis, size ladder.
- [Decoration Transport](decoration-transport.md) — background fills between selection and text, plus Link/Inlay payloads.
- [Folding Ranges](folding-ranges.md) — validated ranges, Tree-sitter derivation, client-local collapse, and chevron ownership.
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md) — size ladder and document line height.
- [Behavior Manifests](behavior-manifests.md) — `editorRules` schema and per-document manifest layers.
- [Masonry Editor Widget Status Observability](masonry-editor.md) — editor surface hosting and paint order.
- [Editor Movement, Selection, Caret, Ligatures, and Text Objects](editor-movement-selection-caret.md) — caret geometry reused by chrome.
- [`editor.clientSetEditorLayout`](../../reference/clay-js-api/editor/client-set-editor-layout.md) — authoritative public API.
- [Creating Clay Packages](../../reference/packages/creating-packages.md) — chrome/layout manifest authoring.
- [Performance budgets](../../development/performance.md) — chrome paint advisory budgets.
