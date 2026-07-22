# Phase 18.16.5 Semantic Typography Primitive Review

## Source

- Plan: `plans/048-Phase18.16.5-Semantic-Font-Roles-and-User-Owned-Typography.md` (task 2).
- Decision: `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`.
- Pattern: `.agents/skills/project-patterns/references/typography-role-ownership.md`.
- `src/protocol/mod.rs`, `src/protocol/decorations.rs`, `src/client/mod.rs`, `src/server/connection.rs`.
- `src/editor/layout.rs`, `src/editor/surface.rs`, `src/editor/theme.rs`, `src/editor/viewport.rs`.
- `src/masonry_editor.rs`, `src/masonry_sdui.rs`, `src/shell/theme.rs`, `src/shell/components.rs`, `src/shell/package_ui.rs`.
- `src/packages/record.rs`, `src/server/modes.rs`, `src/server/syntax.rs`.
- `tests/primitives_docs.rs`, `tests/editor_performance_invariants.rs`, `tests/decoration_transport.rs`.

## Overview

Phase 18.16.5 adds user-owned typography without making fonts a theme package property or package renderer capability. Clay owns three atomic profiles: `monospace`, `proportional`, and `ui`. A semantic role selects one complete profile, meaning both its ordered fallback stack and logical-pixel size. Packages and modes can declare only roles.

This review pins reusable primitives before implementation. It also locks the narrow generic gaps: typography wire/configuration state, a client `TypographyRegistry`, semantic role data on existing mode/decoration/component contracts, role-aware Parley layout, and shared UI geometry. No language-specific renderer or font implementation is needed.

## Existing Primitive Inventory

### Configuration, bootstrap, and live client delivery

`runtime/js/theme.js`, `src/server/ops/theme.rs`, `src/server/js_runtime.rs`, `src/server/mod.rs`, `src/server/connection.rs`, `src/client/mod.rs`, and `src/masonry_editor.rs` already carry one inert appearance snapshot end-to-end. `setTheme` validates and stores `ActiveTheme`, bootstrap sends `ServerMessage::ActiveTheme`, and `ClientConnectionEvent::ActiveTheme` installs a new `StyleRegistry` before paint.

That delivery path is reusable for configuration evaluation, atomic replacement, bootstrap ordering, runtime reload, and live event application. `ActiveTheme` itself is not reusable typography state: it contains a theme specifier and `TextThemeOverride` color/attribute data, while a font-stack/size change changes shaping and geometry. Typography therefore needs a separate snapshot and event, not fields added to `ActiveTheme`.

### Style registry and decoration transport

`src/editor/theme.rs::StyleRegistry` resolves `TokenType` + `Modifiers` to colors and text attributes. `src/protocol/decorations.rs::DecorationSpan` already provides validated byte ranges, `DecorationKind` layers (`Syntax`, `Semantic`, `Diagnostic`, `SearchMatch`), priority, provenance, viewport bounds, and rkyv transport. `DecorationSet` is cached and version-gated before `EditorSurface` paint consumes it. `src/server/syntax.rs` and `src/server/modes.rs` already turn generic package metadata into bounded mode/decorations data.

These are reusable semantic carriers. `StyleRegistry` remains color/text-attribute ownership and must not choose fonts. Decoration spans must gain only an optional enum role on authorized layout-owning layers; they must not gain raw Parley properties, family strings, sizes, callbacks, or package executable data.

### Cached Parley editor layout and UTF-8 geometry

`src/editor/layout.rs::LayoutState` owns the visible-text Parley `Layout`, shaping, wrapping, cursor hit testing, caret geometry, selection geometry, and visual metrics. `LayoutCacheKey` already invalidates by text revision, viewport revision, and width; `PaintCtx::fonts_changed()` also forces a rebuild. `src/editor/surface.rs` keeps extraction viewport-bounded and translates validated visible decoration byte ranges into layout-local offsets. Parley remains source of truth for UTF-8 caret, hit-test, selection, wrapping, and visible layout geometry.

Initial gap: `TEXT_FONT_SIZE` and `LineHeight::FontSizeRelative(1.4)` were fixed defaults; the cache key had no typography/layout-style revision; and the paint hook passed decoration colors, not normalized role/style runs. Replacing all layout code or shaping a full document remains unnecessary and would regress the bounded extraction contract.

### Viewport, scrolling, and editor chrome geometry

`src/editor/viewport.rs` owns logical visible-line windows and revisioning. `EditorSurface` uses one fixed line-height estimate for visible-line count, pixel scrolling, scrollbar progress, empty-document caret height, and visual-scroll clamping, then uses visible Parley metrics for rendered max-scroll/caret placement. This is reusable state and conservative large-file strategy.

The fixed-size arithmetic was the gap. `TypographyRegistry::document_line_height()` now derives a shared baseline from the largest active monospace/proportional profile using `DOCUMENT_LINE_HEIGHT_MULTIPLIER`; it drives extraction, pixel-scroll progression, and logical scrollbar progress. Visible Parley `Layout::height()` and cursor geometry continue to supply actual shaped max-scroll and caret geometry, including the placeholder caret. This phase does not add full-document shaping or an exact global metric index.

### Native UI, SDUI, components, and accessibility

`src/editor/typography.rs::UiTextMetrics` now resolves semantic body/status/title/detail variants from cached `TypographyRegistry` profiles. `src/masonry_editor.rs` uses UI profile status metrics and stack for status paint/bounds. `src/shell/theme.rs::SduiThemeStyle` maps typed title/body/status tokens to variants, not scalar font sizes. `src/masonry_sdui.rs` uses one profile-derived metric for native text builders, rows, scroll increments, action rectangles, and AccessKit bounds. `src/shell/components.rs`, `src/shell/package_ui.rs`, and `src/server/ui.rs` validate/store bounded inert component `style.fontRole` declarations and package provenance.

This is the reusable Clay-owned UI pipeline. Existing typed `typography` tokens are semantic variants; they are not package-selected font families or absolute font sizes. Text-dependent row geometry, hit regions, scroll increments, and accessibility bounds use the same resolved metric.

### Package validation and authority boundary

`src/packages/record.rs`, `src/server/ui.rs`, and `reject_ui_prohibited_authority` already enforce package provenance, bounded declarations, typed style values, and rejection of raw CSS, raw ops, native handles/widgets, renderer callbacks, and client JavaScript. `docs/reference/primitives/package-security.md` supplies shared permission/hot-path rules.

Reuse these validators for enum role declarations. A role carries no permission and must not introduce font-file paths/bytes/URLs, downloads, installed-font discovery requests, native handles, raw CSS, raw Parley values, or renderer callbacks.

## Generic Phase 18.16.5 Gaps

### Separate `ActiveTypography` snapshot and atomic configuration

Add bounded `FontProfile { families, size }` values for all three profiles and an `ActiveTypography` snapshot with a revision. `clay:theme.setTypography` validates a complete candidate and replaces all profiles together. The server sends this state at bootstrap and on successful changes through a protocol message separate from `ActiveTheme`.

The server validates syntax/bounds only. The client resolves named-family fallback through its local Parley/Fontique context. Missing named fonts must retain a generic fallback; no server font scan occurs.

### `TypographyRegistry` and semantic roles

Add a client-side `TypographyRegistry` that converts validated profiles once into cached Parley stacks/sizes and resolves `monospace`, `proportional`, and `ui` roles. It owns the typography revision. `StyleRegistry` stays independent and owns only colors/text attributes.

Documents carry a default role: `core.code` is monospace; `core.text` and Markdown are proportional. Syntax/Semantic spans may carry an optional non-inherit document override. Clay/package UI defaults to UI; validated components may use an allowed semantic role, never a concrete font value.

### Normalized role-aware layout runs

`LayoutState` needs a bounded visible-range normalization step before Parley building. It starts with the document default and applies only valid Syntax/Semantic role overrides. It merges adjacent equal results, then applies default and ranged `FontStack`, `FontSize`, and existing text-attribute properties to Parley.

Role precedence is deterministic: document default first; then normalized Syntax/Semantic spans using existing layer, priority, and provenance ordering; equal candidates retain the existing deterministic decoration order. `Diagnostic` and `SearchMatch` remain paint-only and cannot change a font. Invalid, stale, empty, or out-of-visible-range records are ignored before layout.

### Typography-aware cache and conservative geometry

`LayoutCacheKey` must include typography revision, layout-style revision, and document default role in addition to text, viewport, and width. A changed typography snapshot rebuilds the visible layout, requests render/layout/accessibility work, and resets or clamps visual scroll. Unchanged revisions do nothing.

Replace `TEXT_FONT_SIZE`/`STATUS_TEXT_SIZE` as geometry authority. `TypographyRegistry::document_line_height()` is the documented conservative editor baseline from active document profiles for extraction, pixel-scroll progression, and logical scrollbar progress; Parley supplies actual visible layout height, wrapping, cursor, selection, rendered max-scroll, and placeholder-caret geometry. This keeps large files viewport-bounded until a later full-document metric decision.

### Shared UI typography metrics

`UiTextMetrics` supplies family selection through the cached profile, relative title/body/status/detail size, line height, row pitch, and text layout to status, SDUI, package components, menus, labels, buttons, lists, overlays, pointer rectangles, scroll math, and accessibility bounds. Component `style.fontRole` defaults to `ui`; only text-bearing panel, label, button, list, and statusItem declarations may select `monospace` or `proportional`. Validation rejects concrete family/size and unsupported roles; packages remain inert.

## Data Flow and Reuse Rule

```text
init.js -> clay:theme.setTypography -> validated ActiveTypography
        -> bootstrap/live ClientConnectionEvent -> TypographyRegistry
        -> mode document default + normalized Syntax/Semantic roles
        -> cached Parley layout -> native editor/UI paint and geometry
```

Future modes declare a default role and optional semantic style-map role only. Markdown marks code spans/blocks monospace through package data; Rust, TypeScript, and JavaScript inherit `core.code`. Do not add language-name checks in client/editor/server rendering code.

## Hot-Path Classification

| Work | Allowed location |
| --- | --- |
| Configuration validation, profile atomicity, runtime reload | init/configuration/reload path |
| Protocol bounds validation and client profile conversion | bootstrap/live update, outside paint |
| Mode/style-map validation and role normalization | package/parse/update path; visible-range normalization cached with layout inputs |
| Parley font stack/size/range application | visible layout rebuild only |
| Paint/text-event/key/layout/scroll/pointer hot path | cached typography/profile/style/layout reads only |

No family parsing, installed-font discovery, package JavaScript, server IPC, style-map processing, span sorting/allocation, or configuration evaluation belongs in Masonry paint, input, pointer, scroll, or keypress paths. Existing `DECORATION_PAYLOAD_BUDGET_BYTES`, `SYNTAX_CACHE_BUDGET_BYTES`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`, and viewport extraction bounds remain in force.

## Security and Authority Boundary

Typography values are inert ordered family names plus finite bounded logical-pixel sizes. Packages/modes declare only a closed role enum. The client receives only validated `ActiveTypography`, mode defaults, and decoration/component role data.

This phase adds no font-file/byte/path/URL authority, font download, network, filesystem, shell, AI, workspace mutation, native-ui, package-control, package-manager, raw-ops, or client-runtime authority. It does not expose Masonry widgets, Parley callbacks, raw style objects, or renderer code to packages.

## Rejected Implementation Shapes

- Do not add font family or size fields to `ActiveTheme`, `TextThemeOverride`, or theme packages.
- Do not add `MarkdownFontRenderer`, `RustFontRenderer`, `TypeScriptFontRenderer`, or language-specific font `match` branches.
- Do not let packages set absolute sizes, families, raw `FontStack` values, CSS, font URLs, bytes, paths, or callbacks.
- Do not apply raw overlapping decoration spans in arrival order or let Diagnostic/Search spans alter layout typography.
- Do not parse font stacks or discover fonts during Masonry paint/input/layout paths.
- Do not shape full documents merely to make scrolling exact.
- Do not add hidden JSON/TOML typography keys or three independent profile setters.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- `src/editor/typography.rs`: `document_line_height_uses_largest_document_profile_not_ui` locks the shared baseline; `ui_variants_scale_from_configured_role_size` locks title/body/status scale ownership.
- `src/masonry_sdui.rs`: `ui_size_change_scales_row_hit_and_accessibility_bounds_together` and `package_component_font_role_uses_selected_profile_without_concrete_sizes` lock shared UI geometry and package role use.
- `src/server/ui.rs`: `package_component_font_role_is_semantic_and_text_only` rejects concrete and structural-component typography.
- `src/editor/layout.rs`: `mixed_role_line_height_keeps_largest_inline_profile_in_bounds` confirms Parley preserves the largest inline role's line metrics; `unicode_and_emoji_shape_with_unavailable_named_font_fallback` verifies generic fallback shapes UTF-8 without rendering failure.
- `src/editor/surface.rs`: custom-typography viewport/scrollbar/reset and placeholder-caret tests retain bounded editor geometry; `mixed_role_normalization_stays_bounded_by_visible_span_boundaries` locks visible-span normalization bounds.
- `tests/editor_performance_invariants.rs`: `typography_geometry_uses_shared_profile_baseline_not_fixed_font_size` prevents fixed font-size geometry returning; `typography_updates_do_not_enter_editor_hot_paths` excludes JavaScript, IPC, filesystem, network, shell, and font-discovery work.
- `tests/manual_smoke_docs.rs::phase18_16_5_typography_smoke_covers_fallback_geometry_and_authority` locks the Linux GUI matrix for themes, sizes, code/prose/Markdown/UI, fallback, reload/reconnect, geometry, and authority.

Run:

```bash
cargo test --test protocol primitives_docs::
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Decoration Transport](decoration-transport.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Typography Role Ownership](../../../.agents/skills/project-patterns/references/typography-role-ownership.md)
- [Package Primitive Security](../../reference/primitives/package-security.md)
