# UI Chrome Primitives (Phase 20.2)

Phase 20.2 introduced a native chrome primitive layer in `src/shell/primitives.rs`. These are `pub(crate)` inert paint helpers that read from `ResolvedUiTheme` tokens and render UI chrome (dividers, focus rings, panel backgrounds/borders, scrollbars, badges, keyboard hints, icon slots, tooltip shells). Phase 20.4 added state-color helpers (`component_state_color`, `list_row_fill_color`, `disabled_text_color`) that centralize the `InteractionState`→token mapping used by the SDUI paint path.

## Architecture

Primitives are the **only** way to paint UI chrome in Clay. Shell/SDUI paint paths call primitive helpers; packages cannot call primitives directly. Package-declared `ComponentKind` components map onto primitives by construction (the SDUI paint path calls primitive helpers for chrome).

### Token-driven design

All primitives read color, dimension, opacity, and typography from `ResolvedUiTheme` tokens. No hardcoded values. This ensures:
- Theme packages can customize all chrome through token contributions.
- User preferences (density, reduced motion, high contrast) flow through tokens.
- Chrome remains consistent across the shell and editor.

### Interaction states

Interactive primitives handle `InteractionState` variants:
- `Rest`: default state
- `Hover`: pointer over element
- `Active`: pointer down / pressed
- `Focus`: keyboard focus
- `Disabled`: inactive (applies `opacity.disabled`)

### Accessibility

Each primitive maps to an accessibility role:
- `paint_divider` → `separator`
- `paint_focus_ring` → applied to focused element
- `paint_panel_chrome` → `region` or `complementary`
- `paint_scroll_chrome` → `scrollbar`
- `paint_badge` → `status` or `note`
- `paint_kbd_hint` → `kbd` (via label)
- `paint_icon_slot` → `img` or `presentation`
- `paint_tooltip_shell` → `tooltip`

## Primitive inventory

| Primitive | Purpose | Token mapping | Accessibility role |
|-----------|---------|---------------|-------------------|
| `paint_divider` | Horizontal/vertical separator | `color.border`, `dimension.border.width` | `separator` |
| `paint_focus_ring` | Focus indicator ring | `color.focus.ring`, `dimension.focus.ring.width`, `dimension.focus.ring.offset` | Applied to focused element |
| `paint_panel_chrome` | Panel background/border with optional title/collapse/resize | `color.surface.panel`, `color.border`, `dimension.border.width`, `dimension.radius.panel`, `spacing.panel.padding` | `region` or `complementary` |
| `paint_scroll_chrome` | Scrollbar track/thumb with interaction states | `color.scrollbar.track`, `color.scrollbar.thumb`, `dimension.scrollbar.width`, `dimension.scrollbar.margin`, `dimension.scrollbar.min.thumb`, `dimension.radius.scrollbar` | `scrollbar` |
| `paint_badge` | Badge/tag with label and interaction states | `color.surface.badge`, `color.text.badge`, `dimension.radius.badge`, `spacing.badge.padding.x`, `spacing.badge.padding.y`, `typography.badge` | `status` or `note` |
| `paint_kbd_hint` | Keyboard shortcut hint | `color.surface.kbd`, `color.text.kbd`, `dimension.radius.kbd`, `spacing.kbd.padding.x`, `spacing.kbd.padding.y`, `typography.kbd` | `kbd` (via label) |
| `paint_icon_slot` | Standardized icon placeholder | `dimension.icon.size`, `dimension.icon.slot.size`, `color.text.muted`, `dimension.radius.icon` | `img` or `presentation` |
| `paint_tooltip_shell` | Tooltip background/border | `color.surface.overlay`, `color.border`, `dimension.border.width`, `dimension.radius.tooltip`, `spacing.tooltip.padding` | `tooltip` |

## State-color helpers (Phase 20.4)

Phase 20.4 added three `pub(crate)` helpers in `src/shell/primitives.rs` that centralize the `InteractionState`→token mapping. The SDUI paint path (`src/masonry_sdui.rs`) calls these instead of branching on state inline, so token routing and state mapping have one source.

| Helper | Signature | Mapping |
|--------|-----------|---------|
| `component_state_color` | `(theme, rest_token: &str, state: InteractionState) -> Color` | `Rest`→`theme.color(rest_token)`; `Hover`→`surface.hover`; `Active`→`surface.active`; `Focus`→`accent.primary`; `Disabled`→`surface.disabled` × `opacity.disabled` (alpha-multiply) |
| `list_row_fill_color` | `(theme, state: InteractionState, selected: bool) -> Color` | `Disabled`→`surface.disabled` × `opacity.disabled`; `Hover`/`Active`→`surface.hover`/`surface.active` (override selection); `Rest`/`Focus`→`surface.selected` if `selected` else `surface.list` |
| `disabled_text_color` | `(theme) -> Color` | `text.disabled` × `opacity.disabled` (alpha-multiply) |

All three are token-driven (read `ResolvedUiTheme::color` / `opacity`) and apply `opacity.disabled` via the module's alpha-multiply pattern (not `Brush::with_alpha`, which sets rather than multiplies). They are `pub(crate)`; packages cannot call them. The SDUI paint path uses `component_state_color(theme, "surface.control", state)` for `button` fills, `list_row_fill_color(theme, state, item.selected)` for `list` rows, and `disabled_text_color(theme)` for `label`/`statusItem`/`button` disabled text.

## Routing

### SDUI chrome routing (src/masonry_sdui.rs)

- **Sidebar chrome**: `paint()` → `paint_panel_chrome()`
- **Package fixed panel chrome**: `paint_package_fixed_panels()` → `paint_panel_chrome()`
- **Package overlay chrome**: `paint_package_overlays()` → `paint_tooltip_shell()`

### Editor chrome routing (src/editor/surface.rs)

- **Scrollbar chrome**: `paint_vertical_scrollbar()` → `paint_scroll_chrome()`
- Editor text/caret/selection/diagnostics remain on `StyleRegistry` (editor-owned color authority)

### No changes needed

- `src/shell/package_ui.rs`: data structures only; paint done in `masonry_sdui.rs`
- `src/shell/transient_menu.rs`: data structures only; paint done in `masonry_sdui.rs`

## Conformance contract

Enforced by `tests/ui_primitive_conformance.rs`:

1. **No color literals**: Shell/SDUI chrome paint files contain no `Color::from_rgb8`/`Color::from_rgba8` literals outside `primitives.rs` and `theme.rs`. Phase 20.4 added `src/editor/surface.rs` (editor chrome) to the color-guard set.
2. **No hardcoded sizes**: Shell/SDUI chrome paint files contain no hardcoded chrome-size constants (`SCROLLBAR_WIDTH`, `BORDER_WIDTH`, etc.) outside `primitives.rs` and `theme.rs`. Phase 20.4 added `src/masonry_editor.rs` (status bar) to the size-guard set.
3. **Primitive routing**: Package components map onto primitives by construction (SDUI paint routes chrome through primitive helpers).
4. **Token-driven**: Each primitive reads from `ResolvedUiTheme` (`theme.color()`, `theme.dimension()`, `theme.opacity()`).
5. **State-complete**: Interactive primitives handle all `InteractionState` variants including `Disabled`.

## Package authoring contract

Packages **cannot** call primitives directly. Packages declare inert `ComponentKind` components only; Clay renders them through native code and primitives.

Primitive customization flows through token contributions:
- `clay.ui.serverRegisterThemeToken` (register new tokens)
- `clay.contributions.themeTokens` / `designTokens` (override token values)

Editor chrome is **not** SDUI chrome (Plan 071 task 12): caret shape/blink and the font-ligature baseline are editor/typography chrome, not `ComponentKind` components or theme tokens. Packages cannot register a caret style, blink policy, or ligature policy through `serverRegisterThemeToken`, `designTokens`, or any component contribution. The only package surfaces are:

- **Caret shape/blink**: inert `editorRules.caretStyle` manifest data (`shape`, `blink`, `widthPx`, `heightPct`, `hollow`, `stopBlinkOnTyping`), validated at mode registration like every other editor rule. Omitted means the reduced-motion-safe editor default bar (`StyleRegistry` caret style).
- **Caret color**: stays theme-owned — the `caret` theme token, overridable through theme-token contributions like any other color. Shape/blink and color are deliberately separate authorities.
- **Ligatures**: follow the mode's font role. Each `FontProfile` (monospace/proportional/ui) carries a user-owned `LigaturePolicy`; a mode's `defaultFontRole` selects which profile applies to its document text. No package capability grants ligature overrides. See [Semantic Typography Roles](typography.md#ligature-policy).

Rendering of both surfaces stays in native code (`paint_caret` in `src/editor/surface.rs`; parley `StyleProperty::FontFeatures` in the layout path); no package JavaScript runs in caret paint or text shaping.

See [Creating Clay Packages](../packages/creating-packages.md#ui-chrome-conformance-phase-202) for the full package authoring contract.

## Performance

- Primitives are deterministic and allocation-free in paint paths.
- No per-frame theme re-resolution; tokens are cached in `ResolvedUiTheme`.
- No layout mutation during paint.
- No package JavaScript in paint/layout/pointer/scroll/keypress/text-event handlers.

## Security

- Primitives are `pub(crate)` inert paint helpers; not exposed to JavaScript.
- No new filesystem, network, shell, AI, WASM, raw-op, or package-manager authority.
- Packages still cannot call primitives directly or paint chrome themselves.

## References

- `.agents/skills/clay-ui/references/components.md` — full primitive inventory and token mappings
- `.agents/skills/clay-ui/references/tokens.md` — typed token catalog
- `docs/reference/packages/creating-packages.md` — package authoring contract
- `docs/wiki/modules/phase20.2-ui-primitive-library-primitive-review.md` — primitive review wiki page
- `tests/ui_primitive_conformance.rs` — conformance tests
