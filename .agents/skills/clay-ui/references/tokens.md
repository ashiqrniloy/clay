# Clay Theme Tokens and Typography

Typed tokens are the only styling currency. Packages declare semantic tokens that resolve through same-typed core fallbacks (`ThemeTokenResolver`, `src/shell/theme.rs`). Users override theme and fonts via configuration; nothing hardcodes raw colors or sizes.

## Token Types

`color-role`, `spacing`, `radius`, `typography`, `opacity` (`ThemeTokenType`).

## Core Tokens (implemented)

### Color roles

| Token | Purpose |
|-------|---------|
| `surface.main` | App background |
| `surface.panel` | Panel background |
| `surface.overlay` | Floating layer background |
| `surface.control` | Button/control background |
| `surface.list` | List background |
| `surface.selected` | Selected row/item |
| `text.primary` | Primary text |
| `text.muted` | Secondary text |
| `accent.primary` | Accent / focus |
| `diagnostic.error` | Error (plus `diagnosticWarning`/`diagnosticInfo` in editor style registry) |

Editor base UI color keys (`src/editor/theme.rs` `BaseUiColors`, theme-package contributed): `shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`, `diagnosticWarning`, `diagnosticInfo`, plus syntax tokens.

### Spacing

| Token | Value | Use |
|-------|-------|-----|
| `spacing.none` | 0 | Flush |
| `spacing.inline` | 6 | Tight sibling grouping |
| `spacing.panel` | 14 | Panel padding |
| `spacing.row` | 26 | List row height |

### Radius

| Token | Value |
|-------|-------|
| `radius.none` | 0 |
| `radius.panel` | 6 |

### Typography

| Token | Variant |
|-------|---------|
| `typography.body` | `UiTextVariant::Body` |
| `typography.title` | `UiTextVariant::Title` |
| `typography.status` | `UiTextVariant::Status` |

### Opacity

| Token | Value | Use |
|-------|-------|-----|
| `opacity.disabled` | 0.55 | Disabled state |
| `opacity.full` | 1.0 | Default |

## Typography Hierarchy (implemented)

Font roles (`FontRole`, user-configurable family stack + base size per role): `ui`, `monospace`, `proportional`.

UI text variants (`UiTextVariant`, `src/editor/typography.rs`) scale from the configured role size — never absolute point sizes:

| Variant | Scale vs role base | Use |
|---------|--------------------|-----|
| `Title` | 14/12 | Panel/section titles |
| `Body` | 1.0 | Main UI text |
| `Status` | 1.0 | Status bar |
| `Detail` | 10/12 | Secondary/detail text |

## Planned Token Additions (UI Revamp Phase 20.1)

- Expanded spacing scale (4pt base: 4/8/12/16/24/32/48) as `spacing.xxs`…`spacing.xxl`.
- Typography hierarchy expansion: `Display`, `Section`, `Caption` variants for clearer title levels.
- `border.*` (width/color roles), `focus.ring` color, `elevation.*` (shadow levels, kept near-invisible per minimalist direction), `motion.*` (durations), `z.*` (overlay levels), `diagnostic.success`.
- State color roles: `surface.hover`, `surface.active`, `text.disabled`.

## Rules

1. Reference tokens by name; raw values are rejected by validation.
2. New tokens must be one of the five typed categories and have a core fallback.
3. Token additions are additive-only; never repurpose an existing token's meaning.
4. Theme packages (e.g. `@clay/theme-gruvbox-material-dark`) contribute values, not structure.
5. Update this file when tokens or variants change.
