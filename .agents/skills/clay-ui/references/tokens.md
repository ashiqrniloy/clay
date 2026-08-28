# Clay Theme Tokens and Typography

Typed tokens are the only styling currency. Packages declare semantic tokens that resolve through same-typed core fallbacks (`ThemeTokenResolver`, `src/shell/theme.rs`). Users override theme and fonts via configuration; nothing hardcodes raw colors or sizes.

Content themes are the sole normal-rendering color authority. UI design-system recipes may select semantic theme color roles for component slots/states and apply typed opacity/effects, but must never define palettes, literal colors, or package-owned color values. Shell, component, border, focus, selection, diagnostic, overlay, and solid material fallback colors all resolve from the active theme. Browser/OS system colors are reserved for forced-colors accessibility behavior.

Resolution happens at theme/configuration install time. Native paint, layout, pointer, scroll, keypress, and text-event hot paths read cached resolved values only — no package JavaScript, theme parsing, raw IPC, or re-resolution runs per frame.

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
| `surface.scrim` | Full-window dim behind the centered Command Centre surface (Phase 24.4) |
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

Editor base UI color keys (`src/editor/theme.rs` `BaseUiColors` / `StyleRegistry`, theme-package contributed): `shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`, `diagnosticWarning`, `diagnosticInfo`, `searchMatch`, `unused`, `gutterFg`, `gutterFgActive`, `lineHighlight`, `indentGuide`, `bracketMatch`, plus syntax tokens. The editor `StyleRegistry` is the single source of color for editor paint paths and is separate from SDUI typed tokens. Chrome keys (`gutterFg*`, `lineHighlight`, `indentGuide`, `bracketMatch`) style the Phase 26.5 gutter / active-line / indent-guide / bracket-match surfaces.

Editor layout insets (Phase 26.6) are Clay-owned constants aligned to the spacing scale, not new SDUI tokens: horizontal `spacing.xl` (32) without a gutter, `spacing.xxl` (48) when the gutter is on, vertical 20. Wrap policy is `editorRules.layout`, not a theme token.

Theme `textStyles` extra axes (Phase 26.3/26.4): `background` (`#rgb`/`#rrggbb`/`#rrggbbaa`) and `scale` (finite `(0, 4]`). Defaults: Quote/CodeBlock/SearchMatch/Deprecated fills; heading ladder H1 1.50 … H6 0.92, CodeSpan 0.90. Not SDUI tokens.

Legacy `textStyles` themes are projected into modern UI roles by `ResolvedUiTheme::with_base_ui` (`src/shell/theme.rs`) when no typed `designTokens` override wins: panel/list/overlay surfaces use `panelBg`, controls/badges/kbd use `statusBg`, selection/state surfaces use `selection`, focus/accent uses `caret`, feedback uses the diagnostic colors, and text roles use `text`/`placeholder`/`statusText`. UI `text.muted` promotes a low-contrast legacy placeholder to `text` so the same WCAG AA gate applies to light and dark themes. This compatibility projection changes no package-facing token names and keeps theme overrides cached before paint/layout.

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
| `opacity.scrim` | 0.5 | Scrim dim behind the centered Command Centre surface (Phase 24.4) |

### Dimension (Phase 20.1)

Panel, sidebar, and border logical-pixel defaults. These feed `ResolvedUiTheme::panel_defaults()`; invalid ordering (`min > default` or `max < default`) falls back to the matching Clay constant tuple per domain.

| Token | Value | Use |
|-------|-------|-----|
| `dimension.sidebar.default` | 240 | Visible SDUI left-slot + package `Left` fixed panel; hidden workspace-pane snapshots reserve no left slot |
| `dimension.panel.side.default` | 240 | Left/Right fixed panel default size |
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
| `Title` | 14/12 | Panel/section titles |
| `Section` | 13/12 (Phase 20.1) | Sub-section headings |
| `Body` | 1.0 | Main UI text |
| `Status` | 1.0 | Status bar |
| `Detail` | 10/12 | Secondary/detail text |
| `Caption` | 0.75 (Phase 20.1) | Hint/footnote text |

The seven scale ratios form `UiTypographyHierarchy`, which is user-owned and travels atomically with `ActiveTypography` via [`clay.theme.setTypography`](../../../docs/reference/clay-js-api/theme/set-typography.md). Each scale must be finite, positive, and at most 4. Omitting `hierarchy` keeps Clay defaults; a partial hierarchy (any missing field) is rejected atomically so half-installed scales never reach layout. A changed hierarchy increments the typography revision and invalidates editor/UI layout once; an unchanged hierarchy does not churn layout.

Packages and components select a semantic variant name only; they cannot supply concrete scale ratios. A `clay.contributions.designTokens` entry targeting any `typography.*` token is rejected as a typography (variant) override, not a scale value.

## Package Token Contributions

Packages declare semantic tokens through `clay.ui.serverRegisterThemeToken` (`runtime/js/ui.js`) or the `clay.contributions.themeTokens`/`clay.contributions.designTokens` manifest descriptors. A package token declaration carries `token` (package-prefixed name), `type` (one of the ten types above), `fallback` (a same-typed Clay core token), and `description`.

Theme packages may also ship typed UI design-token overrides via `clay.contributions.designTokens` (`UiDesignTokenOverride`), validated into `ActiveTheme.design_tokens` and resolved client-side into `ResolvedUiTheme`. Each override's value variant must match the core token's type and pass domain bounds (dimension ordering, opacity `[0,1]`, `motion-duration` `[0,1000]`, valid level names). Raw CSS, raw colors, style strings, renderer callbacks, native handles, and raw ops are rejected at load time.

## Plan 088 token consumption (no additions)

Plan 088 Tasks 3–7 use the existing typed token catalog; no core token or package token domain was added. The modernization contract is consumption-only:

- Shell, pane, panel, overlay, tab, status, and package chrome use the cached `ResolvedUiTheme` surface/text/border, spacing, radius, opacity, density, z-level, elevation, dimension, and semantic typography tokens already listed above.
- Responsive decisions use token-backed panel/sidebar defaults plus user UI typography metrics and Masonry constraints. They may yield a fixed slot or clip bounded content, but packages cannot declare breakpoints, concrete pixel sizes, font families, or raw CSS.
- `typography.*` remains a variant selector over user-owned `UiTypographyHierarchy`; `theme.setTypography` owns concrete sizes/families. `designTokens` cannot supply hierarchy scales or typography-token overrides.
- Token resolution happens once at theme/configuration install or reload. Paint, layout, pointer, scroll, keypress, and text-event paths read cached values only; no package JavaScript, raw IPC, parsing, or re-resolution runs in those hot paths.
- Contrast, same-typed fallbacks, bounds, state completeness, and code-vs-catalog parity remain host validation rules. `tokens.md` must stay synchronized with `core_theme_value`; no visual alias is added without a concrete generic consumer.

## Rules

1. Reference tokens by name; raw values are rejected by validation.
2. New tokens must be one of the ten typed categories and have a same-typed Clay core fallback.
3. Token additions are additive-only; never repurpose an existing token's meaning.
4. Theme packages (e.g. `@clay/theme-gruvbox-material-dark`) contribute values, not structure. Existing Gruvbox themes need no manifest change — they resolve through core fallbacks.
5. `typography.*` tokens are variant selectors, not scale values. Packages cannot ship concrete hierarchy scales.
6. Update this file when tokens, variants, or hierarchy defaults change.
7. **Contrast and fallback correctness are enforced at validation (Phase 20.7).** The active theme's status-chrome token pairs must meet `TEXT_CONTRAST_MIN` (4.5) and `UI_CONTRAST_MIN` (3.0) (`validate_active_theme_contrast`, `src/shell/theme.rs`; `enforce_contrast`, `src/server/ops/theme.rs`) — a below-AA theme is not activated. A package token's `fallback` must be a same-typed Clay core token; type mismatches and invalid units are rejected (`core_fallback_matches_type`, `src/shell/theme.rs`; parsed in `src/packages/record.rs`). Raw colors, raw CSS, and raw sizes in `designTokens` overrides or component `style.*` variables are rejected at load time. These are host-authority checks run inside Clay's Rust host validator; no package-facing op or facade exposes them. See `docs/reference/packages/creating-packages.md` § "Phase 20.7 authoring contract: UI conformance guardrails".
8. **Code-vs-catalog drift is linted (Phase 20.7).** The `core_theme_value` match arms in `src/shell/theme.rs` must stay in sync with the Core Tokens tables above; `tests/package_ui_conformance.rs::core_token_catalog_matches_tokens_md` fails the build if they drift.

## Phase 24.4 consumption (centered Command Centre)

Phase 24.4 adds three core tokens consumed by the Clay-internal centered
Command Centre surface: `surface.scrim` (color role), `opacity.scrim` (0.5),
and `dimension.overlay.centered.width` (640). Consumption policy:

- **Hot path:** all three resolve once at active-theme install into the cached
  `ResolvedUiTheme` and are read on paint/layout from cache — never re-resolved
  per frame. The centered host performs exactly one `paint_scrim` fill per
  paint pass and adds no blur/filter/offscreen work; `dimension.overlay.centered.width`
  clamps to the available window width.
- **Authority:** the scrim and centered surface are Clay-owned. Theme packages
  may override the three typed values through `designTokens` (same validation
  rules as any core token), but packages cannot paint, configure, or request
  the centered surface; package overlay anchors remain
  `working-area` | `active-pane` | `main` | `pointer`.

## Phase 20.4 consumption (no new tokens)

Phase 20.4 (core component uplift) consumes existing state, spacing, opacity, border, and typography tokens only — **no new token was added**. Recorded consumption:

- State surfaces: `surface.hover`, `surface.active`, `surface.disabled` drive `button`/`list` `Hover`/`Active`/`Disabled` fills via `component_state_color`; `surface.control` (button `Rest`), `surface.list`/`surface.selected` (list rows), `surface.panel`/`surface.overlay` (panel/overlay chrome).
- Focus: `accent.primary` (focused `button` fill), `border.focus` (focus ring/border), `focus.ring` (`paint_focus_ring`).
- Disabled text: `text.disabled` × `opacity.disabled` (alpha-multiply) for `label`/`statusItem`/`button` disabled text.
- Spacing rhythm: `spacing.md` × `spacing_scale()` (density) for SDUI panel padding; `spacing.sm` × `spacing_scale()` for status bar insets; `border.hairline` for the status bar divider.
- Opacity: `opacity.disabled` (0.55) for `Disabled` alpha-multiply; `opacity.full` (1.0) for scrollbar `Hover`/`Active`/`Focus` (Rest reuses `opacity.disabled` for a near-invisible rest scrollbar — a dedicated `opacity.scrollbar.rest` is the upgrade path if rest needs to differ from disabled).
- Typography: `typography.title`/`body`/`status` resolved through `SduiThemeStyle::from_ui_theme`.

Density (`density.default` → `spacing_scale()`) scales the token-owned UI spacing rhythm only; it never scales panel dimensions or document typography. Per-element `spacing.xs`/`sm`/`lg` differentiation across components is deferred to a later spacing pass.
