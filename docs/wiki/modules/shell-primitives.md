# Shell Primitives (Phase 20.2)

**Module:** `src/shell/primitives.rs`
**Phase:** 20.2 — Clay UI Primitive Library Foundation
**Status:** Implemented (2026-07-24)

## Overview

Phase 20.2 introduces a native chrome primitive layer in `src/shell/primitives.rs`. These are `pub(crate)` inert paint helpers that centralize recurring shell/SDUI chrome drawing. They are **not** package-facing `ComponentKind` entries and are **not** exposed to JavaScript. Missing reads use the resolved core-token fallback (or a transparent/zero no-op where the primitive is intentionally optional), never a package-supplied raw style.

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
| Scrim | `paint_scrim` | Single-pass centered Command Centre backdrop |
| Tab card chrome | `tab_card_chrome` | State-resolved tab fill/text/close/focus-ring projection |

## Token Mapping

All primitives are token-driven. They read from cached `ResolvedUiTheme` values and use core-token/transparent fallbacks only when a value is absent; package declarations cannot inject raw values.

### Core Tokens (Phase 20.2)

| Token | Type | Purpose |
|-------|------|---------|
| `dimension.scrollbar.width` | dimension | Scrollbar width |
| `dimension.icon.size` | dimension | Icon glyph slot size |
| `dimension.kbd.height` | dimension | Kbd hint height |
| `surface.scrollbar` / `surface.scrollbar.track` | color | Scrollbar thumb/track |
| `surface.badge` | color | Badge background |
| `surface.kbd` | color | Kbd hint background |
| `surface.tooltip` | color | Tooltip background |
| `border.focus` | color | Focus ring border |
| `border.kbd` | color | Kbd hint border |
| `text.badge` | color | Badge text |
| `text.kbd` | color | Kbd hint text |
| `text.tooltip` | color | Tooltip text |
| `text.icon` | color | Icon glyph |
| `spacing.badge` | spacing | Badge padding |
| `spacing.tooltip` | spacing | Tooltip padding |
| `typography.detail` / `typography.caption` / `typography.body` | typography | Badge, kbd, and tooltip text roles |
| `elevation.overlay` | elevation | Tooltip elevation role |
| `z.tooltip` | z-level | Tooltip stacking role |
| `surface.scrim` / `opacity.scrim` | color / opacity | Centered Command Centre backdrop |

### Reused Tokens (Phase 20.1)

| Token | Type | Purpose |
|-------|------|---------|
| `border.hairline` | color | Divider/tooltip border |
| `dimension.border.hairline` | dimension | Divider/panel/tooltip border width |
| `dimension.border.thin` | dimension | Focus ring width |
| `radius.xs` | radius | Focus ring, badge, kbd, and scrollbar corners |
| `radius.sm` | radius | Panel and tooltip corners |
| `surface.panel` | color | Panel background |
| `border.subtle` | color | Panel border |
| `text.muted` | color | Icon glyph fallback |
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

`paint_scroll_chrome` preserves the theme-authored `surface.scrollbar` alpha at Rest/Disabled and lifts it toward opaque on Hover/Active/Focus. No dedicated scrollbar interaction token was added; `paint_vertical_scrollbar` in `src/editor/surface/mod.rs` threads `EditorSurface::scrollbar_interaction_state` (O(1) pointer hit-test against `scrollbar_thumb_rect`).

## Shell chrome consumers (Plan 088)

Tab cards and the pinned new-tab affordance consume `component_state_color`, `list_row_fill_color`, `tab_card_chrome`, and cached radius/text tokens. Split focus rings are painted after child panes so active focus remains visible. The centered Command Centre consumes one `paint_scrim` pass plus `paint_tooltip_shell`; package panel/overlay hosts reuse the panel/tooltip primitives and clip their retained children. These are still Clay-internal paint paths; no package-facing primitive or JavaScript API was added.

### Production status

`paint_divider`, `paint_focus_ring`, `paint_panel_chrome`, `paint_scroll_chrome`, `paint_tooltip_shell`, `paint_scrim`, and `tab_card_chrome` have production consumers. `paint_badge`, `paint_kbd_hint`, and `paint_icon_slot` remain internal chrome skeletons: their token/state contracts are documented and tested, but label/glyph rendering is deferred and no production path promotes them to package-facing kinds. This is why the component catalog keeps Badge/tag, kbd hint, and Icon slot marked planned.

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
| Scrim | `dialog` backdrop (Clay-internal) |

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

### Editor (src/editor/surface/mod.rs)

- **Vertical scrollbar:** `paint_scroll_chrome`

### Shell / centered surface

- **Tab cards and pinned `+`:** `tab_card_chrome` plus cached UI typography metrics
- **Split-pane focus:** `paint_focus_ring` after pane-host paint
- **Centered Command Centre:** one `paint_scrim` fill, then `paint_tooltip_shell`

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
- **Unit tests:** `src/shell/primitives.rs` (`primitives_panic_free_on_zero_size_rects`, `primitives_render_all_interaction_states`, `disabled_state_applies_opacity`, state-color/tab-card mappings, scrollbar-alpha regression, and deterministic single-pass scrim tests)
- **Visibility test:** `tests/rust_visibility_api_mapping.rs` (`phase20_2_primitives_are_not_exposed_to_javascript`, `phase20_4_introduces_no_unexposed_public_rust_function`)
- **Hot-path guards:** `tests/editor_performance_invariants.rs::hot_path_no_theme_resolution_or_package_js` and `centered_overlay_work_is_bounded_and_scrim_is_single_pass`
- **Commands:** `cargo test --lib shell::primitives --quiet`; `cargo test --test editor ui_primitive_conformance`; `cargo test --test protocol primitives_docs`

## Related Documentation

- [Phase 20.4 Core Component Uplift on the Existing Catalog](phase20.4-core-component-uplift-primitive-review.md)
- [Phase 20.2 UI Primitive Library Primitive Review](phase20.2-ui-primitive-library-primitive-review.md)
- [Masonry Shell](masonry-shell.md)
- [Server-Driven UI](server-driven-ui.md)
- [Phase 20.1 UI Design Language Primitive Review](phase20.1-ui-design-language-primitive-review.md)
- [UI Chrome Primitives Reference](../../reference/primitives/ui-chrome-primitives.md)
- [Clay UI Component and Primitive Catalog](../../../.agents/skills/clay-ui/references/components.md)
