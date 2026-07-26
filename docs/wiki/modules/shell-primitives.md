# Shell Primitives (Phase 20.2)

**Module:** `src/shell/primitives.rs`
**Phase:** 20.2 — Clay UI Primitive Library Foundation
**Status:** Implemented (2026-07-24)

## Overview

Phase 20.2 introduces a native chrome primitive layer in `src/shell/primitives.rs`. These are `pub(crate)` inert paint helpers that centralize recurring shell/SDUI chrome drawing. They are **not** package-facing `ComponentKind` entries and are **not** exposed to JavaScript.

## Primitives

| Primitive | Function | Purpose |
|-----------|----------|---------|
| Divider | `paint_divider` | Hairline separator (horizontal/vertical) |
| Focus ring | `paint_focus_ring` | Focus outline around interactive elements |
| Panel chrome | `paint_panel_chrome` | Panel background, title row, collapse affordance, resize handle |
| Scroll chrome | `paint_scroll_chrome` | Scrollbar track and thumb |
| Badge/tag | `paint_badge` | Badge/tag with label |
| Kbd hint | `paint_kbd_hint` | Keyboard shortcut hint |
| Icon slot | `paint_icon_slot` | Token-sized icon slot (no package image assets) |
| Tooltip shell | `paint_tooltip_shell` | Tooltip background with border and shadow |

## Token Mapping

All primitives are token-driven. They read from `ResolvedUiTheme` and fall back to hardcoded values if a token is missing.

### Core Tokens (Phase 20.2)

| Token | Type | Purpose |
|-------|------|---------|
| `dimension.scrollbar.width` | dimension | Scrollbar width |
| `dimension.scrollbar.margin` | dimension | Scrollbar margin |
| `dimension.scrollbar.min_thumb` | dimension | Minimum scrollbar thumb size |
| `dimension.icon.size` | dimension | Icon size |
| `dimension.icon.slot.size` | dimension | Icon slot size |
| `dimension.kbd.height` | dimension | Kbd hint height |
| `dimension.focus.ring.width` | dimension | Focus ring width |
| `dimension.focus.ring.offset` | dimension | Focus ring offset |
| `surface.badge` | color | Badge background |
| `surface.kbd` | color | Kbd hint background |
| `surface.tooltip` | color | Tooltip background |
| `surface.icon` | color | Icon slot background |
| `border.focus` | color | Focus ring border |
| `border.kbd` | color | Kbd hint border |
| `border.tooltip` | color | Tooltip border |
| `text.badge` | color | Badge text |
| `text.kbd` | color | Kbd hint text |
| `text.tooltip` | color | Tooltip text |
| `text.icon` | color | Icon glyph |
| `radius.icon` | dimension | Icon slot corner radius |
| `spacing.badge.padding.x` | dimension | Badge horizontal padding |
| `spacing.badge.padding.y` | dimension | Badge vertical padding |
| `spacing.kbd.padding.x` | dimension | Kbd hint horizontal padding |
| `spacing.kbd.padding.y` | dimension | Kbd hint vertical padding |
| `typography.badge` | typography | Badge text style |
| `typography.kbd` | typography | Kbd hint text style |
| `typography.tooltip` | typography | Tooltip text style |

### Reused Tokens (Phase 20.1)

| Token | Type | Purpose |
|-------|------|---------|
| `border.hairline` | color | Divider color |
| `dimension.border.hairline` | dimension | Divider width |
| `dimension.border.thin` | dimension | Focus ring width (fallback) |
| `radius.xs` | dimension | Corner radius (focus ring, badge, kbd, tooltip) |
| `surface.panel` | color | Panel background |
| `border.panel` | color | Panel border |
| `surface.overlay` | color | Tooltip background (fallback) |
| `border.overlay` | color | Tooltip border (fallback) |
| `elevation.overlay` | elevation | Tooltip shadow |
| `text.muted` | color | Icon glyph (fallback) |
| `opacity.disabled` | opacity | Disabled state |

## Interaction States

Primitives support interaction states via the `InteractionState` enum:

```rust
pub(crate) enum InteractionState {
    Rest,
    Hover,
    Active,
    Focus,
    Disabled,
}
```

- **Rest:** Default state
- **Hover:** Mouse hover (lighten/darken background)
- **Active:** Mouse down/pressed (stronger lighten/darken)
- **Focus:** Keyboard focus (paint focus ring)
- **Disabled:** Disabled (apply `opacity.disabled`)

### State-color helpers (Phase 20.4)

Phase 20.4 adds three `pub(crate)` helpers (re-exported from `src/shell/mod.rs`) so component paint routes the five states through state tokens without re-deriving the mapping per call site:

| Helper | Signature | Token mapping |
| --- | --- | --- |
| `component_state_color` | `(theme, rest_token, state) -> Color` | Rest→`rest_token`, Hover→`surface.hover`, Active→`surface.active`, Focus→`accent.primary`, Disabled→`surface.disabled` × `opacity.disabled` |
| `list_row_fill_color` | `(theme, state, selected) -> Color` | selected & Rest/Focus→`surface.selected`, Hover→`surface.hover`, Active→`surface.active`, Disabled→`surface.disabled` × `opacity.disabled` |
| `disabled_text_color` | `(theme) -> Color` | `text.disabled` × `opacity.disabled` |

`apply_alpha(color, factor)` multiplies the alpha channel (it does **not** use peniko's `Brush::with_alpha`, which *sets* rather than multiplies). These helpers are consumed by `paint_package_component` (buttons, lists, labels, status items) and `paint_node` (sidebar buttons/list items) in `src/masonry_sdui.rs`; see [Phase 20.4 Core Component Uplift](phase20.4-core-component-uplift-primitive-review.md).

### Scroll chrome opacity (Phase 20.4)

`paint_scroll_chrome` maps Rest/Disabled → `opacity.disabled` (dim, near-invisible) and Hover/Active/Focus → `opacity.full`. No dedicated `opacity.scrollbar.rest` token was added — Rest reuses `opacity.disabled`. `paint_vertical_scrollbar` in `src/editor/surface.rs` now threads `EditorSurface::scrollbar_interaction_state` (O(1) pointer hit-test against `scrollbar_thumb_rect`) instead of a hardcoded `InteractionState::Rest`.

## Accessibility Roles

Primitives map to accessibility roles:

| Primitive | Role |
|-----------|------|
| Divider | `separator` |
| Focus ring | `focusable` (implicit via focus state) |
| Panel chrome | `region` or `complementary` |
| Scroll chrome | `scrollbar` |
| Badge/tag | `status` or `note` |
| Kbd hint | `label` (implicit via text) |
| Icon slot | `image` or `presentation` |
| Tooltip shell | `tooltip` |

## Conformance Contract

The Phase 20.2 conformance contract is enforced by `tests/ui_primitive_conformance.rs`:

1. **No color literals** in non-test paint paths outside `src/shell/primitives.rs` and `src/shell/theme.rs`
2. **No hardcoded chrome sizes** in non-test paint paths outside `src/shell/primitives.rs` and `src/shell/theme.rs`
3. **Primitive routing:** SDUI sidebar, package panels, overlays, and editor scrollbar route through primitives
4. **Token-driven:** Primitives read from `ResolvedUiTheme` with fallbacks
5. **State-complete:** Primitives handle all interaction states

## Paint Paths Routed Through Primitives

### SDUI (src/masonry_sdui.rs)

- **Sidebar background:** `paint_panel_chrome`
- **Package fixed panel backgrounds:** `paint_panel_chrome`
- **Overlay backgrounds:** `paint_tooltip_shell`

### Editor (src/editor/surface.rs)

- **Vertical scrollbar:** `paint_scroll_chrome`

## Performance

- **O(1) primitive paint:** Each primitive is a single fill/stroke operation
- **Cached token reads:** Tokens are resolved once in `ResolvedUiTheme` and cached
- **No hot-path JS/IPC:** Primitives are inert client paint; no JavaScript or IPC in paint paths
- **No per-frame allocations:** Primitives are deterministic and allocation-free

## Security

- **`pub(crate)` visibility:** Primitives are Clay-internal; packages cannot call them directly
- **No authority:** Primitives are inert paint; no filesystem, network, shell, AI, WASM, raw-op, or package-manager authority
- **Not exposed to JavaScript:** No `deno_core` op wraps a primitive; no Clay JS facade exports a primitive

## Source and Test Paths

- **Source:** `src/shell/primitives.rs`
- **Conformance tests:** `tests/ui_primitive_conformance.rs`
- **Unit tests:** `src/shell/primitives.rs` (3 tests: panic-free on zero-size rects, all interaction states render, disabled opacity applied; plus `component_state_color_maps_all_five_states_to_tokens` and `list_row_fill_color_honors_selected_and_state` from Phase 20.4)
- **Visibility test:** `tests/rust_visibility_api_mapping.rs` (`phase20_2_primitives_are_not_exposed_to_javascript`, `phase20_4_introduces_no_unexposed_public_rust_function`)
- **Hot-path guard:** `tests/editor_performance_invariants.rs::hot_path_no_theme_resolution_or_package_js` (Phase 20.4)

## Related Documentation

- [Phase 20.4 Core Component Uplift on the Existing Catalog](phase20.4-core-component-uplift-primitive-review.md)
- [Phase 20.2 UI Primitive Library Primitive Review](phase20.2-ui-primitive-library-primitive-review.md)
- [Masonry Shell](masonry-shell.md)
- [Server-Driven UI](server-driven-ui.md)
- [Phase 20.1 UI Design Language Primitive Review](phase20.1-ui-design-language-primitive-review.md)
- [UI Chrome Primitives Reference](../../reference/primitives/ui-chrome-primitives.md)
- [Clay UI Component and Primitive Catalog](../../../.agents/skills/clay-ui/references/components.md)
