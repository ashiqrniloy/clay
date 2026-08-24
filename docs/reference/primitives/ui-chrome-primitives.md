# UI Chrome Primitives (Phase 20.2)

Phase 20.2 introduced a chrome primitive layer (pre-cutover: the `pub(crate)` paint helpers in `src/shell/primitives.rs`, removed with the native client). These are inert chrome renderers reading from resolved theme tokens: dividers, focus rings, panel backgrounds/borders, scrollbars, badges, keyboard hints, icon slots, and tooltip shells — now realized as token-driven React components/CSS classes with the same catalog contract. Phase 20.4 added state-color helpers (`component_state_color`, `list_row_fill_color`, `disabled_text_color`) that centralize the `InteractionState`→token mapping.

## Architecture

Primitives are the **only** way to paint UI chrome in Clay. Shell/SDUI paint paths call primitive helpers; packages cannot call primitives directly. Package-declared `ComponentKind` components map onto primitives by construction (the SDUI paint path calls primitive helpers for chrome).

### Token-driven design

All primitives read color, spacing/radius/dimension, opacity, and typography from `ResolvedUiTheme` tokens. Radius values use the scalar accessor because `radius.*` belongs to the radius domain; logical pixel geometry uses `dimension.*`. No hardcoded values. This ensures:
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

### Tab bar chrome (Phase 22.3, accessibility Phase 22.6)

The tab strip is shell chrome, not a package-facing widget (pre-cutover it
lived in the removed native primitive/shell modules; the React tab bar
carries the same contract). Its accessibility surface is a shell-owned `TabList`
(`Window tabs`) with one `Tab` per card — sanitized workspace basename,
`selected` on the active card — present when tabs exist.
Split dividers were already covered by `paint_divider` → `separator`.
Announcements for tab/split changes come from the shell's persistent
`Status` live-region node (`Live::Polite`), one per user action. See
[Accessibility (Phase 22.6)](../development/accessibility.md).

### Editor-intelligence chrome (Phase 28)

Phase 28 extends editor data without adding package-facing chrome primitives:

- `FoldingRange` data is validated and delivered in the background. The editor
  surface owns fold chevrons in `paint_gutter`, collapsed-line hiding, and the
  client-local toggle command. Fold ranges are not package widgets or tab stops.
- `DecorationKind::Link` uses the existing `TokenType::Link` underline. Clay
  hit-tests visible spans, paints hover through `paint_tooltip_shell`, and
  handles typed hover/activate intent; packages publish targets, not callbacks.
- Inlay hints are `DecorationKind::InlayHint` spans with bounded inert labels.
  Clay paints them as muted overlay text after the main token layout with no
  text reflow. They are decorative/`aria-hidden`; the visibility toggle is a
  Clay command, not a package component.
- No package JavaScript runs in paint, layout, pointer, scroll, keypress, or
  text-event handlers. `render-folding` and `render-decorations` publish data
  only; link activation never mints a browse/filesystem grant and external
  HTTP targets are display-only.

## Primitive inventory

| Primitive | Purpose | Token mapping | Accessibility role |
|-----------|---------|---------------|-------------------|
| `paint_divider` | Horizontal/vertical separator | `border.hairline`, `dimension.border.hairline` | `separator` |
| `paint_focus_ring` | Focus indicator ring | `border.focus`, `dimension.border.thin`, `radius.xs` | Applied to focused element |
| `paint_panel_chrome` | Panel background/border with optional title/collapse/resize | `surface.panel`, `border.subtle`, `dimension.border.hairline`, `radius.sm`, `spacing.panel` | `region` or `complementary` |
| `paint_scroll_chrome` | Scrollbar track/thumb with interaction states | `surface.scrollbar.track`, `surface.scrollbar`, `dimension.scrollbar.width`, `radius.xs` | `scrollbar` |
| `paint_badge` | Badge/tag with label and interaction states | `surface.badge`, `text.badge`, `radius.xs`, `spacing.badge`, `typography.detail`/`caption` | `status` or `note` |
| `paint_kbd_hint` | Keyboard shortcut hint | `surface.kbd`, `text.kbd`, `border.kbd`, `radius.xs`, `dimension.kbd.height`, `typography.caption` | `kbd` (via label) |
| `paint_icon_slot` | Standardized icon placeholder | `dimension.icon.size`, `text.icon`, `opacity.disabled` | `img` or `presentation` |
| `paint_tooltip_shell` | Tooltip background/border | `surface.tooltip`, `text.tooltip`, `border.hairline`, `dimension.border.hairline`, `radius.sm`, `elevation.overlay`, `z.tooltip`, `spacing.tooltip`, `typography.body` | `tooltip` |
| `paint_scrim` (Phase 24.4) | Full-window dim behind centered Command Centre | `surface.scrim`, `opacity.scrim` | modal `Dialog` backdrop |
| `tab_card_chrome` (Phase 22.3) | Tab card background/text with interaction states and selection | `list_row_fill_color`/`disabled_text_color` state mapping, `surface.list`, `surface.selected`, `surface.hover`, `surface.active`, `text.disabled`, `opacity.disabled` | informational `Tab` under the shell `TabList` (virtual node, not a widget) |

## State-color helpers (Phase 20.4)

Phase 20.4 centralized the `InteractionState`→token mapping in three helpers. Pre-cutover they were `pub(crate)` Rust functions in `src/shell/primitives.rs`; the current client applies the identical mapping through CSS component classes keyed by state attributes, keeping token routing and state mapping single-source.

| Helper | Signature | Mapping |
|--------|-----------|---------|
| `component_state_color` | `(theme, rest_token: &str, state: InteractionState) -> Color` | `Rest`→`theme.color(rest_token)`; `Hover`→`surface.hover`; `Active`→`surface.active`; `Focus`→`accent.primary`; `Disabled`→`surface.disabled` × `opacity.disabled` (alpha-multiply) |
| `list_row_fill_color` | `(theme, state: InteractionState, selected: bool) -> Color` | `Disabled`→`surface.disabled` × `opacity.disabled`; `Hover`/`Active`→`surface.hover`/`surface.active` (override selection); `Rest`/`Focus`→`surface.selected` if `selected` else `surface.list` |
| `disabled_text_color` | `(theme) -> Color` | `text.disabled` × `opacity.disabled` (alpha-multiply) |

All three are token-driven (resolved theme color/opacity tables) and apply `opacity.disabled` by alpha multiplication rather than replacement; packages cannot call them. The rendered surfaces use the same mapping for `button` fills, `list` rows, and `label`/`statusItem`/`button` disabled text.

## Routing

### SDUI chrome routing

- **Sidebar chrome**: panel chrome styling on the SDUI sidebar surface.
- **Package fixed panel chrome**: same panel chrome applied to projected package panels.
- **Package overlay chrome**: tooltip/popover chrome on transient overlays.
- **Centered Command Centre backdrop**: one translucent scrim behind the modal dialog; no blur, filter, or offscreen pass.

### Editor chrome routing

- **Scrollbar chrome**: rendered through the shared scroll-chrome primitive styling.
- Editor text/caret/selection/diagnostics remain on the editor theme registry (editor-owned color authority).

### No changes needed

- `src/shell/package_ui.rs`: data/state only; rendering done by the React projection (`frontend/src/sdui`)
- `src/shell/transient_menu.rs`: server-side session state only; rendering done by the React Command Centre

## Conformance contract

Enforced by `tests/ui_primitive_conformance.rs`:

1. **No color literals**: component styles contain no raw color literals outside the token catalog (`frontend/src/**.module.css` guard + catalog tests).
2. **No hardcoded sizes**: component styles use token-driven spacing/radius variables, not fixed chrome-size constants.
3. **Primitive routing**: Package components map onto primitives by construction (SDUI paint routes chrome through primitive helpers).
4. **Token-driven**: Each primitive reads from `ResolvedUiTheme` (`theme.color()`, `theme.scalar_f64()` for spacing/radius, `theme.dimension()` for logical dimensions, and `theme.opacity()`).
5. **State-complete**: Interactive primitives handle all `InteractionState` variants including `Disabled`.

## Package authoring contract

Packages **cannot** call primitives directly. Packages declare inert `ComponentKind` components only; Clay renders them through native code and primitives.

Primitive customization flows through token contributions:
- `ui.serverRegisterThemeToken` (register new tokens)
- `clay.contributions.themeTokens` / `designTokens` (override token values)

Editor chrome is **not** SDUI chrome (Plan 071 task 12): caret shape/blink and the font-ligature baseline are editor/typography chrome, not `ComponentKind` components or theme tokens. Packages cannot register a caret style, blink policy, or ligature policy through `serverRegisterThemeToken`, `designTokens`, or any component contribution. The only package surfaces are:

- **Caret shape/blink**: inert `editorRules.caretStyle` manifest data (`shape`, `blink`, `widthPx`, `heightPct`, `hollow`, `stopBlinkOnTyping`), validated at mode registration like every other editor rule. Omitted means the reduced-motion-safe editor default bar (`StyleRegistry` caret style).
- **Caret color**: stays theme-owned — the `caret` theme token, overridable through theme-token contributions like any other color. Shape/blink and color are deliberately separate authorities.
- **Ligatures**: follow the mode's font role. Each `FontProfile` (monospace/proportional/ui) carries a user-owned `LigaturePolicy`; a mode's `defaultFontRole` selects which profile applies to its document text. No package capability grants ligature overrides. See [Semantic Typography Roles](typography.md#ligature-policy).

Rendering of both surfaces stays inside the editor implementation (CodeMirror caret extension; font-feature settings from the resolved profile); no package JavaScript runs in caret rendering or text shaping.

See [Creating Clay Packages](../packages/creating-packages.md#ui-chrome-conformance-phase-202) for the full package authoring contract.

## Performance

- Primitives are deterministic and allocation-free in paint paths.
- No per-frame theme re-resolution; tokens are cached in `ResolvedUiTheme`.
- No layout mutation during paint.
- Centered Command Centre paint is one token-driven scrim fill plus the bounded retained overlay subtree; width/scrim tokens resolve before paint and are cached.
- No backdrop blur, filter, or offscreen render target.
- No package JavaScript in paint/layout/pointer/scroll/keypress/text-event handlers.

## Security

- Primitives are `pub(crate)` inert paint helpers; not exposed to JavaScript.
- No new filesystem, network, shell, AI, WASM, raw-op, or package-manager authority.
- Packages still cannot call primitives directly or paint chrome themselves. `paint_scrim` and the internal centered root layer are Clay-owned; package anchors remain `working-area`, `active-pane`, `main`, and `pointer`.

## References

- `.agents/skills/clay-ui/references/components.md` — full primitive inventory and token mappings
- `.agents/skills/clay-ui/references/tokens.md` — typed token catalog
- `docs/reference/packages/creating-packages.md` — package authoring contract
- `docs/wiki/modules/phase20.2-ui-primitive-library-primitive-review.md` — primitive review wiki page
- `tests/ui_primitive_conformance.rs` — conformance tests
