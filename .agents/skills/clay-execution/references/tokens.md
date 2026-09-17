# Clay Theme Tokens and Typography

Typed tokens are the only styling currency. Packages declare semantic tokens that resolve through same-typed core fallbacks (`ThemeTokenResolver`, `src/shell/theme.rs`). Users override theme and fonts via configuration; nothing hardcodes raw colors or sizes.

Content themes are the sole normal-rendering color authority. UI design-system recipes may select semantic theme color roles for component slots/states and apply typed opacity/effects, but must never define palettes, literal colors, or package-owned color values. Shell, component, border, focus, selection, diagnostic, overlay, and solid material fallback colors all resolve from the active theme. Browser/OS system colors are reserved for forced-colors accessibility behavior.

Resolution happens once at theme/configuration install time; paint/layout hot paths read cached resolved values only. The React client receives the active theme as a flat token map and projects it into CSS custom properties (`--clay-*`) via the frontend theme runtime (`frontend/src/theme/`) — no package JavaScript, theme parsing, raw IPC, or re-resolution runs per frame.

## Quiet Instrument Profile (shipped design system)

The normative design language is [`DESIGN.md`](../../../../DESIGN.md), shipped as
`@clay/design-instrument` (plan 118 task 8). It adds **no token**: it consumes the
catalog below and nominates design-system-local values, which the package declares
in its `values` block and recipes (no core token, type, or style variable change).
The four shipped content themes now carry the theme-side half of that language as
thirteen typed `designTokens` overrides — see "Shipped theme UI roles" below.

What the language consumes from this catalog:

| Purpose | Token / role | Notes |
|---------|--------------|-------|
| Canvas, chrome strips, sidebar/rail | `surface.main` | One surface; chrome zones are separated by a hairline, not by a different fill |
| Veil planes (grouped content) | `surface.panel` at design-system opacity 0.55 | Depth comes from role + opacity, never from another frame |
| Inset wells (fields, composers, meters) | `surface.control` | Opaque, hairline-bordered |
| Transient layers (popover, sheet, palette, modal) | `surface.overlay` | The only surfaces that carry a shadow |
| Hover / pressed / disabled surfaces | `surface.hover`, `surface.active`, `surface.disabled` | Plus `surface.selected`/`surface.list` where a role fill is preferred to an accent-opacity tint |
| Text | `text.primary`, `text.muted`, `text.disabled`, `text.icon` | Mono for data, UI face for prose |
| Keyboard hints, badges, tooltips | `surface.kbd`/`text.kbd`/`border.kbd`, `surface.badge`/`text.badge`, `surface.tooltip`/`text.tooltip` | Design-system radius and fill; token owns color |
| State, focus, boundaries | `accent.primary`, `accent.muted`, `focus.ring`, `border.focus`, `border.hairline`, `border.subtle`, `border.strong` | Each role's source is named in `DESIGN.md` §10.1: `border.hairline` is the theme's border grey at 34%, `border.subtle` the same grey at full strength, `border.strong` the theme's ink |
| Diagnostics | `diagnostic.error`/`warning`/`success`/`info` | Tints expressed as role + 0.15 opacity, never as extra opaque colors |
| Disabled / locked / scrim | `opacity.disabled` (0.55 core, 0.5 in the profile), `opacity.scrim` | Profile also declares veil 0.55 and accent-soft 0.15 as design-system values |
| Scroll chrome | `surface.scrollbar`, `surface.scrollbar.track`, `dimension.scrollbar.width` | Pill thumb, track transparent |
| Icon, kbd, hairline geometry | `dimension.icon.size`, `dimension.kbd.height`, `dimension.border.hairline` | Host-owned sizes stay on the dimension tokens |
| Centered overlay width | `dimension.overlay.centered.width` | Command palette / command centre |
| Overlay and popover entrance | `motion.normal` (200) is the nearest core token; the profile nominates 240ms and the 620ms focus pulse as design-system values | Curves stay `ease-out` / `spring-snappy` |
| Spacing rhythm | `spacing.xxs`…`spacing.xl` × `spacing_scale()` | Density scales this rhythm only |
| UI type sizes | `typography.*` variants (variant selectors, never sizes) | Concrete families/sizes stay user-owned via `theme.setTypography` |

Design-system-local values the profile nominates (declared in the design-system package, not here): radius ladder 5 / 8 / 12 / 16 / pill; border widths 1 (hairline) and 2 (state marks, rendered as inset shadows); motion 150 / 240 / 620; opacity 0.55 (veil), 0.15 (accent-soft), 0.5 (disabled); scrim blur 3. Exact numbers, per-surface recipes, and the migration profile live in `DESIGN.md` §4, §6, §11, §16.

Scroll chrome is the `scroll` family's: the thumb is the `surface.scrollbar` role at
the recipe's rest opacity (0.4), firmed to 0.8 on hover and 1.0 while dragging, with
the pill radius. Standard `scrollbar-color` carries the rest state app-wide from
`frontend/src/styles/global.css` and the WebKit pseudo-elements carry the state pair,
so a scrolling region never invents a thumb colour. `backgroundOpacity` is composed
by the host, not by the recipe: `color-mix(in srgb, var(--clay-ds-…-background-color)
calc(var(--clay-ds-…-background-opacity, 1) * 100%), transparent)`.

Editor-specific consumption is unchanged: the canvas, gutter, caret, selection, and syntax colors come from the editor `StyleRegistry`, not from SDUI tokens, and the editor inset constants stay aligned to the spacing scale (Phase 26.6). `backdropBlur == 0` on canvas, gutter, scroll, panels, and rows is a hard performance invariant of this language.

## Shipped theme UI roles (plan 118 tasks 13–14)

The language's color half is theme data. All four shipped themes
(`@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi`,
`@clay/theme-gruvbox-material-dark`, `@clay/theme-gruvbox-material-light`) declare
the same **thirteen** `clay.contributions.designTokens` roles, each derived from
their own palette:

| Role | Source (per theme) | Obligation |
|------|--------------------|------------|
| `border.hairline` | the theme's border grey at 34% | decorative zone separator: exempt from the 3:1 floor, must clear a 1.2:1 visibility floor and stay quieter than `border.subtle` |
| `border.subtle` | the same border grey at full strength | structural boundary: ≥ 3:1 against `surface.main` **and** `surface.panel` |
| `border.strong` | the theme's ink at full strength | rare explicit separators: ≥ 3:1 |
| `surface.scrim` | the theme's dimmest plane | overlay dim, ≥ 3:1 against `text.primary` where it carries text |
| `accent.primary` / `accent.muted` | the theme's accent / accent at 75% | ≥ 3:1 against the surface behind them |
| `focus.ring` / `border.focus` | the theme's accent | ≥ 3:1, and the ring must be visible on every surface it can sit on |
| `text.muted` / `text.disabled` | attenuated ink | `text.disabled` is measured against its surface **before** the host's `opacity.disabled` attenuation |
| `surface.hover` / `surface.active` / `surface.selected` | opaque or tinted fill steps | the fill passes 3:1 against the text painted on it, measured composited (fill over its surface, then text over the fill) |

Every pair is measured **composited** — alpha blended over the backdrop before the
ratio is taken, because a 34%-alpha hairline renders at 1.39–1.61:1 across the four
palettes while its raw bytes describe opaque ink at up to 21:1. A theme that misses
a floor is refused activation (`validate_active_theme_contrast` behind
`enforce_contrast`), the diagnostic names the specifier, pair, ratio and threshold,
and the previously active theme stays installed. Per-theme values, rationale and
the measured table: `design-artifacts/approved/quiet-instrument-migration/theme-values.{json,md}`
and [`docs/reference/ui-design-systems.md` §7](../../../../docs/reference/ui-design-systems.md).

## Token Types

`ThemeTokenType` (`src/shell/theme.rs`) — ten additive typed domains. A package token's `type` must be one of these and its `fallback` must be a same-typed Clay core token.

| Type | Resolved value kind | Bounds |
|------|---------------------|--------|
| `color-role` | RGBA bytes | hex `#rgb`/`#rrggbb`/`#rrggbbaa` |
| `spacing` | finite f64 px | `[0, 8192]` |
| `radius` | finite f64 px | `[0, 8192]` |
| `typography` | `UiTextVariant` selector | one of the seven semantic variants |
| `opacity` | finite f32 | `[0, 1]` |
| `dimension` | finite f64 px | `[0, 8192]` (panel/sidebar/border logical-pixel defaults) |
| `elevation` | `ElevationLevel` | `none` / `raised` / `overlay` |
| `motion-duration` | finite f64 ms | `[0, 1000]` |
| `z-level` | `ZLevel` | `base` / `panel` / `overlay` / `modal` / `tooltip` |
| `density` | `DensityLevel` | `compact` / `default` / `spacious` |

## Core Tokens (implemented)

Core tokens live in `core_theme_value` (`src/shell/theme.rs`) and are the only same-typed fallback a package token may reference. Every implemented token is listed below. Phase 20.1 made the catalog additive: legacy names and values are unchanged; new domains (`dimension`, `elevation`, `motion-duration`, `z-level`, `density`) and new tokens inside existing domains extend the catalog without repurposing anything.

### Color roles

| Token | Purpose |
|-------|---------|
| `surface.main` | App background |
| `surface.panel` | Panel background |
| `surface.overlay` | Floating layer background |
| `surface.scrim` | Dim behind the window-centered Command Centre sheet (Phase 24.4) and behind the composer's `/` and `@` menus since plan 124 (over the working area, never over the agent lane) |
| `surface.control` | Button/control background |
| `surface.list` | List background |
| `surface.selected` | Selected row/item |
| `surface.hover` | Hovered surface (Phase 20.1) |
| `surface.active` | Pressed/active surface (Phase 20.1) |
| `surface.disabled` | Disabled surface (Phase 20.1) |
| `text.primary` | Primary text |
| `text.muted` | Secondary text |
| `text.disabled` | Disabled text (Phase 20.1) |
| `accent.primary` | Accent / focus |
| `accent.muted` | Muted accent (Phase 20.1) |
| `focus.ring` | Focus ring color (Phase 20.1) |
| `border.hairline` | Hairline border color (Phase 20.1) |
| `border.subtle` | Subtle divider color (Phase 20.1) |
| `border.strong` | Strong divider color (Phase 20.1) |
| `border.focus` | Focused border color (Phase 20.1) |
| `border.kbd` | kbd hint border color (Phase 20.2) |
| `surface.badge` | Badge/tag background (Phase 20.2) |
| `text.badge` | Badge/tag text color (Phase 20.2) |
| `surface.kbd` | kbd hint background (Phase 20.2) |
| `text.kbd` | kbd hint text color (Phase 20.2) |
| `surface.tooltip` | Tooltip background (Phase 20.2) |
| `text.tooltip` | Tooltip text color (Phase 20.2) |
| `text.icon` | Icon glyph color (Phase 20.2) |
| `surface.scrollbar` | Scrollbar thumb (Phase 20.2) |
| `surface.scrollbar.track` | Scrollbar track (Phase 20.2) |
| `diagnostic.error` | Error |
| `diagnostic.warning` | Warning (Phase 20.1) |
| `diagnostic.info` | Info (Phase 20.1) |
| `diagnostic.success` | Success (Phase 20.1) |

Editor base UI color keys (`src/editor/theme.rs` `BaseUiColors`/`StyleRegistry`, theme-package contributed): `shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`/`Warning`/`Info`, `searchMatch`, `unused`, `gutterFg`(+`Active`), `lineHighlight`, `indentGuide`, `bracketMatch`, `accent`, `borderHairline`/`borderSubtle`/`borderStrong`, plus syntax tokens. `StyleRegistry` is the single color source for editor paint paths, separate from SDUI typed tokens.

Editor layout insets (Phase 26.6) are Clay-owned constants aligned to the spacing scale, not new SDUI tokens: horizontal `spacing.xl` (32) without a gutter, `spacing.xxl` (48) when the gutter is on, vertical 20. Wrap policy is `editorRules.layout`, not a theme token.

Theme `textStyles` extra axes (Phase 26.3/26.4): `background` (`#rgb`/`#rrggbb`/`#rrggbbaa`) and `scale` (finite `(0, 4]`). Defaults: Quote/CodeBlock/SearchMatch/Deprecated fills; heading ladder H1 1.50 … H6 0.92, CodeSpan 0.90. Not SDUI tokens.

Legacy `textStyles` themes are projected into modern UI roles by `ResolvedUiTheme::with_base_ui` when no typed `designTokens` override wins (panel/list/overlay→`panelBg`, controls/badges/kbd→`statusBg`, selection→`selection`, focus/accent→`caret`, feedback→diagnostics, text→`text`/`placeholder`/`statusText`); `text.muted` promotes a low-contrast legacy placeholder to `text` so the same WCAG AA gate applies to light and dark themes. No package-facing token names change; overrides stay cached before paint/layout.

### Spacing

4pt base scale (Phase 20.1) plus legacy named spacing.

| Token | Value | Use |
|-------|-------|-----|
| `spacing.none` | 0 | Flush |
| `spacing.inline` | 6 | Tight sibling grouping (legacy) |
| `spacing.panel` | 14 | Panel padding (legacy) |
| `spacing.row` | 26 | List row height (legacy) |
| `spacing.xxs` | 4 | Dense control grouping |
| `spacing.xs` | 8 | Control padding |
| `spacing.sm` | 12 | Card/section padding |
| `spacing.md` | 16 | Default content padding |
| `spacing.lg` | 24 | Section separation |
| `spacing.xl` | 32 | Region separation |
| `spacing.xxl` | 48 | Page-level separation |
| `spacing.badge` | 4 | Badge/tag padding (Phase 20.2) |
| `spacing.tooltip` | 8 | Tooltip padding (Phase 20.2) |

### Radius

| Token | Value |
|-------|-------|
| `radius.none` | 0 |
| `radius.panel` | 6 (legacy) |
| `radius.xs` | 2 |
| `radius.sm` | 4 |
| `radius.lg` | 8 |

### Typography

`typography.*` tokens select a semantic `UiTextVariant` (not an absolute size). See [Typography Hierarchy](#typography-hierarchy-implemented).

| Token | Variant |
|-------|---------|
| `typography.body` | `Body` |
| `typography.title` | `Title` |
| `typography.status` | `Status` |
| `typography.display` | `Display` (Phase 20.1) |
| `typography.section` | `Section` (Phase 20.1) |
| `typography.detail` | `Detail` (Phase 20.1) |
| `typography.caption` | `Caption` (Phase 20.1) |

### Opacity

| Token | Value | Use |
|-------|-------|-----|
| `opacity.disabled` | 0.55 | Disabled state |
| `opacity.full` | 1.0 | Default |
| `opacity.scrim` | 0.5 | Scrim dim behind the centered Command Centre sheet and the composer's menus (Phase 24.4; plan 124 re-anchored the palette onto it) |

### Dimension (Phase 20.1)

Panel, sidebar, and border logical-pixel defaults. These feed `ResolvedUiTheme::panel_defaults()`; invalid ordering (`min > default` or `max < default`) falls back to the matching Clay constant tuple per domain.

| Token | Value | Use |
|-------|-------|-----|
| `dimension.sidebar.default` | 244 | The workspace sidebar's SDUI region (an SDUI node sized by token, plan 118 task E1) + package `Left` fixed panel; hidden workspace-pane snapshots reserve no left slot |
| `dimension.sidebar.compact` | 224 | The same region at ≤1240px (DESIGN.md §5) |
| `dimension.panel.side.default` | 244 | Left/Right fixed panel default size (the same number as the sidebar's, DESIGN.md §5) |
| `dimension.panel.side.min` | 48 | Left/Right minimum |
| `dimension.panel.side.max` | 480 | Left/Right maximum |
| `dimension.panel.vertical.default` | 120 | Top/Bottom fixed panel default size |
| `dimension.panel.vertical.min` | 48 | Top/Bottom minimum |
| `dimension.panel.vertical.max` | 240 | Top/Bottom maximum |
| `dimension.border.hairline` | 1 | Hairline border width |
| `dimension.border.thin` | 2 | Thin border width |
| `dimension.border.thick` | 4 | Thick border width |
| `dimension.scrollbar.width` | 8 | Scrollbar thumb width (Phase 20.2) |
| `dimension.icon.size` | 16 | Icon slot size (Phase 20.2) |
| `dimension.kbd.height` | 20 | kbd hint height (Phase 20.2) |
| `dimension.overlay.centered.width` | 640 | Centered Command Centre surface width, clamped to available window width (Phase 24.4) |

### Elevation (Phase 20.1)

Near-invisible levels per minimalist direction; reserved for overlay/raised surfaces. Phase 20.4 component uplift consumes these.

| Token | Level |
|-------|-------|
| `elevation.none` | `none` |
| `elevation.raised` | `raised` |
| `elevation.overlay` | `overlay` |

### Motion duration (Phase 20.1)

Bounded transition durations. Consumed by Phase 20.4 component uplift; no animation runs in current paint paths.

| Token | Value (ms) |
|-------|------------|
| `motion.instant` | 0 |
| `motion.fast` | 100 |
| `motion.normal` | 200 |
| `motion.slow` | 400 |

### Z-level (Phase 20.1)

Ordered overlay stacking. Consumed by Phase 20.5 overlay/menu component work.

| Token | Level |
|-------|-------|
| `z.base` | `base` |
| `z.panel` | `panel` |
| `z.overlay` | `overlay` |
| `z.modal` | `modal` |
| `z.tooltip` | `tooltip` |

### Density (Phase 20.1)

Compact/default/spacious intent. `density.default` selects the active level; `ResolvedUiTheme::spacing_scale()` returns `0.875`/`1.0`/`1.125`. Density scales the token-owned UI spacing rhythm only (Phase 20.4); it never scales panel dimensions or document typography.

| Token | Level |
|-------|-------|
| `density.compact` | `compact` |
| `density.default` | `default` |
| `density.spacious` | `spacious` |

## Typography Hierarchy (implemented)

Font roles (`FontRole`, user-configurable family stack + base size per role): `ui`, `monospace`, `proportional`.

UI text variants (`UiTextVariant`, `src/editor/typography.rs`) scale from the configured role size — never absolute point sizes:

| Variant | Default scale vs role base | Use |
|---------|----------------------------|-----|
| `Display` | 1.5 (Phase 20.1) | Hero/top-level text |
| `Title` | 15/13 (plan 110 task 9; was 14/12) | Panel/section titles |
| `Section` | 13/12 (Phase 20.1) | Sub-section headings |
| `Body` | 1.0 | Main UI text |
| `Status` | 1.0 | Status bar |
| `Detail` | 12/13 (plan 110 task 9; was 10/12) | Secondary/detail text |
| `Caption` | 0.75 (Phase 20.1) | Hint/footnote text |

The seven scale ratios form `UiTypographyHierarchy`, user-owned, traveling atomically with `ActiveTypography` via [`clay.theme.setTypography`](../../../../docs/reference/clay-js-api/theme/set-typography.md). Each scale must be finite, positive, ≤ 4; a partial hierarchy is rejected atomically; a changed hierarchy increments the typography revision and invalidates layout once (no churn when unchanged).

Packages/components select a semantic variant name only; a `clay.contributions.designTokens` entry targeting any `typography.*` token is rejected as a variant override, not a scale value.

## Package Token Contributions

Packages declare semantic tokens through `clay.ui.serverRegisterThemeToken` (`runtime/js/ui.js`) or the `clay.contributions.themeTokens`/`designTokens` manifest descriptors: `token` (package-prefixed), `type` (one of the ten), `fallback` (same-typed core token), `description`.

Theme packages may also ship typed overrides via `clay.contributions.designTokens` (`UiDesignTokenOverride`), validated into `ActiveTheme.design_tokens` and resolved into `ResolvedUiTheme`; each override must match the core token's type and pass domain bounds (dimension ordering, opacity `[0,1]`, `motion-duration` `[0,1000]`, valid level names). Raw CSS, raw colors, style strings, renderer callbacks, native handles, and raw ops are rejected at load time.

## UI Design-System Value Domains (Plan 101)

UI design systems (`clay.contributions.uiDesignSystem`) declare typed non-color values and recipe mappings that separate visual styling (geometry, material, state, motion) from content-theme color authority.

- **Theme Color Role References (`themeColor`):** every recipe color property must reference an active-theme color role (`surface.control`, `text.primary`, `accent.primary`, `border.focus`, …) or `transparent`; literal colors (hex/rgb/hsl/named) and package-owned palettes are rejected at validation.
- **Namespaced Non-Color Values (`values`):** `dimension` `[0,8192]` px; `radius` `[0,32]` px or `9999` (pill); `border-width` `[0,8]` px; `opacity` `[0,1]`; `backdrop-blur` `[0,32]` px; `backdrop-saturate` `[1.0,2.0]`; `motion-duration` `[0,1000]` ms; `border-style` `none|solid|dashed|dotted`; `transition-timing` `linear|ease-out|spring-snappy|spring-smooth`; `transform-preset` `none|press-subtle|press-shift-down|hover-lift`.
- **Structured Shadow Layers (`shadow`):** up to 3 layers `{ x:[-32..32], y:[-32..32], blur:[0..64], spread:[-16..16], colorRole, opacity:[0..1], inset }`.
- **Inner Highlight Rim (`innerHighlight`):** `{ colorRole, opacity:[0..1], width:[1..4] }`.

## Plan 088 token consumption (no additions)

Plan 088 Tasks 3–7 use the existing typed token catalog; no core token or package token domain was added. The modernization contract is consumption-only:

- Shell/pane/panel/overlay/tab/status/package chrome use the cached `ResolvedUiTheme` surface/text/border, spacing, radius, opacity, density, z-level, elevation, dimension, and semantic typography tokens listed above.
- Responsive decisions use token-backed panel/sidebar defaults plus user UI typography metrics (fixed slot or clipped bounded content); packages cannot declare breakpoints, concrete pixel sizes, font families, or raw CSS.
- `typography.*` is a variant selector over user-owned `UiTypographyHierarchy`; `theme.setTypography` owns concrete sizes/families; `designTokens` cannot supply hierarchy scales or typography-token overrides.
- Token resolution happens once at theme/configuration install or reload; paint/layout hot paths read cached values projected into CSS custom properties. No package JavaScript, raw IPC, parsing, or re-resolution runs in those hot paths.
- Contrast, same-typed fallbacks, bounds, state completeness, and code-vs-catalog parity remain host validation rules. `tokens.md` must stay synchronized with `core_theme_value`; no visual alias is added without a concrete generic consumer.

## Rules

1. Reference tokens by name; raw values are rejected by validation.
2. New tokens must be one of the ten typed categories and have a same-typed Clay core fallback.
3. Token additions are additive-only; never repurpose an existing token's meaning.
4. Theme packages contribute values, not structure: a theme ships `textStyles`
   plus the typed `designTokens` roles its appearance needs — for the shipped
   themes that is the thirteen roles above, derived from the theme's own palette
   and validated against the composited contrast floors. A theme that declares
   none still resolves (borders fall back to the core catalog values), but it is
   then measured on those fallbacks: a palette that cannot clear the structural
   floor from its own base colors is refused activation rather than installed
   half-styled.
5. `typography.*` tokens are variant selectors, not scale values; packages cannot ship concrete hierarchy scales.
6. Update this file when tokens, variants, or hierarchy defaults change — and update [`DESIGN.md`](../../../../DESIGN.md) when the design language's values or per-surface rules change. Appearance is design-system data: never encode a color, radius, shadow, or motion decision in host CSS or in a component.
7. **Contrast/fallback correctness enforced at validation (Phase 20.7, extended by plan 118 task 14):** text pairs must meet `TEXT_CONTRAST_MIN` (4.5); structural boundaries, focus rings and state fills must meet `UI_CONTRAST_MIN` (3.0); the decorative `border.hairline` must meet `HAIRLINE_VISIBILITY_MIN` (1.2) and stay monotonically quieter than `border.subtle` on the same surface (`validate_active_theme_contrast`, `src/shell/theme.rs`; `enforce_contrast`, `src/server/ops/theme.rs`) — every pair measured composited, and a below-floor theme is not activated. Package `fallback` must be a same-typed core token (`core_fallback_matches_type`); raw colors/CSS/sizes in `designTokens` or `style.*` are rejected at load time. Host-authority checks only; no package-facing op or facade exposes them (see creating-packages.md § "Phase 20.7 authoring contract").
8. **Code-vs-catalog drift linted (Phase 20.7):** `core_theme_value` arms must stay in sync with the Core Tokens tables; `tests/package_ui_conformance.rs::core_token_catalog_matches_tokens_md` fails the build on drift.

## Phase 24.4 consumption (centered Command Centre)

Phase 24.4 adds three core tokens consumed by the Clay-internal centered Command Centre surface: `surface.scrim` (color role), `opacity.scrim` (0.5), and `dimension.overlay.centered.width` (640). Plan 124 re-anchors the command palette onto the composer box as the `/` palette: it keeps `surface.scrim` / `opacity.scrim` for its veil, but `dimension.overlay.centered.width` no longer applies to it (the sheet is exactly as wide as the composer box it answers to; the token stays for the surfaces that are still `Centered` — agent picker, package dialogs). All three resolve once at active-theme install into the cached `ResolvedUiTheme` and are read on paint/layout from cache — never re-resolved per frame. The centered host adds no blur/filter/offscreen work; `dimension.overlay.centered.width` clamps to the available window width. Authority: the scrim and centered surface are Clay-owned — theme packages may override the three typed values through `designTokens` (same validation rules as any core token), but packages cannot paint, configure, or request the centered surface; package overlay anchors remain `working-area` | `active-pane` | `main` | `pointer`.

## Phase 20.4 consumption (no new tokens)

Phase 20.4 (core component uplift) consumes existing state, spacing, opacity, border, and typography tokens only — **no new token was added**.

- States: `surface.hover`/`active`/`disabled` drive `button`/`list` fills via `component_state_color`; `surface.control` (button `Rest`), `surface.list`/`selected` (rows), `surface.panel`/`overlay` (chrome).
- Focus: `accent.primary` (focused `button`), `border.focus` (ring/border), `focus.ring` (`paint_focus_ring`).
- Disabled text: `text.disabled` × `opacity.disabled` for `label`/`statusItem`/`button`.
- Rhythm: `spacing.md` × `spacing_scale()` (SDUI panel padding), `spacing.sm` × `spacing_scale()` (status bar insets), `border.hairline` (status bar divider).
- Opacity: `opacity.disabled` (0.55) for `Disabled`; `opacity.full` (1.0) for scrollbar `Hover`/`Active`/`Focus` (Rest reuses `opacity.disabled`; `opacity.scrollbar.rest` is the upgrade path if rest must differ).
- Typography: `typography.title`/`body`/`status` resolved through `SduiThemeStyle::from_ui_theme`.

Density (`density.default` → `spacing_scale()`) scales the token-owned UI spacing rhythm only; it never scales panel dimensions or document typography. Per-element `spacing.xs`/`sm`/`lg` differentiation across components is deferred to a later spacing pass.