# @clay/theme-gruvbox-material-light

A first-party **inert style-data** theme for Clay: the [Gruvbox Material](https://github.com/sainnhe/gruvbox-material) palette, **light, medium contrast**, mapped onto Clay's two-axis syntax vocabulary (`TokenType` + `Modifiers`) and base UI color keys. Selected at runtime by the user via `setTheme("@clay/theme-gruvbox-material-light")` in `~/.clay/init.js`.

This package carries **no executable authority**. All overrides are static `clay.contributions.textStyles` entries in [`package.json`](../package.json), parsed and validated by Clay at load and resolved into the single source of color (`StyleRegistry`). There is no runtime registration, no ops, no widgets, and no raw CSS — only hex colors + optional text-attribute flags (`bold`/`italic`/`underline`/`strike`).

## Palette

Reproduced from [sainnhe/gruvbox-material](https://github.com/sainnhe/gruvbox-material) (light, medium). Accent hexes:

| Role        | Hex      | Role        | Hex      |
|-------------|----------|-------------|----------|
| bg0 (shell) | `#fbf1c7`| bg1 (panel) | `#f2e5bc`|
| bg5 (status)| `#ebd9b1`| fg0 (text)  | `#504940`|
| fg1         | `#3c3836`| grey0 (fade)| `#a89984`|
| red         | `#c14a4a`| orange      | `#c35e0a`|
| yellow      | `#b47109`| green       | `#6c782e`|
| aqua        | `#4c7a4d`| blue        | `#45707a`|
| purple      | `#945e80`|             |          |

## Token mapping (summary)

- **Code (LSP base):** Keyword→red (bold), String→green, Comment→grey0 (italic), Number→purple, Regexp→aqua, Operator→aqua, Function/Method→yellow (Function bold), Macro/Decorator→purple, Namespace/Enum/Interface→blue, Type→yellow, Class/Struct/TypeParameter/Event→orange, Variable/Parameter/Property→fg1, EnumMember→green.
- **Prose (Clay extension):** Heading1-6→red/orange/yellow/green/aqua/blue (the first three bold), Quote→grey0 (italic), CodeSpan→orange, Link→blue (underline), ListItem/CodeBlock/Paragraph→fg0.
- **Base UI:** shellBg→bg0, panelBg→bg1, text→fg0, placeholder→grey0, selection→blue @ ~20%, caret→fg0, scrollbar→grey1, scrollbarTrack→grey0, statusBg→bg5, statusText→fg0.

## UI roles (`designTokens`)

The Quiet Instrument language's theme-side roles (`DESIGN.md` §10): geometry and material come from the design system, the *colours* are the theme's. All 13 roles below are declared in this package's `clay.contributions.designTokens`, identically in every shipped theme, so no role silently falls back to Clay's core palette. Approved board: `design-artifacts/approved/quiet-instrument-migration/themes.html`.

| Role | Value | What it is |
| --- | --- | --- |
| `border.hairline` | `#8a7a6357` | Quiet zone divider: the theme's structural grey at 34 percent. Below the 3:1 floor on purpose — a divider is decoration, and a control is identified by its label and by its hover/active/focus states. |
| `border.subtle` | `#8a7a63` | Structural boundary and hover outline: the same grey at full strength, and the step the 3:1 floor measures. |
| `border.strong` | `#504940` | Rare explicit separator: the theme's ink. |
| `surface.hover` | `#ece0b6` | Opaque hover fill. |
| `surface.active` | `#e3d3a5` | Opaque pressed fill. |
| `surface.selected` | `#dbdfae` | Opaque selected fill. |
| `accent.primary` | `#076678` | The theme's accent (aqua-blue) — previously the accent resolved to the text colour. |
| `accent.muted` | `#076678bf` | The accent one step down (75 percent alpha) for quieter affordances, still at or above 3:1 on the canvas. |
| `focus.ring` | `#076678` | Focus is a state, so focus is the accent. |
| `border.focus` | `#076678` | The same accent, so a focused control's border and its ring are one colour. |
| `text.muted` | `#6b5c48` | Dim text step, at or above 4.5:1 on the panel. |
| `text.disabled` | `#71624b` | Meta/disabled step, at or above 4.5:1 on the canvas. |
| `surface.scrim` | `#504940` | The theme's ink, so the dim is this palette's darkest neutral rather than Clay's pure black. |



To retune a value, edit the `textStyles` and `designTokens` arrays in `package.json` — every color is a tunable inert value; no code change or rebuild of Clay is needed. The default Clay theme (no overrides) remains available as the fallback when no `@clay/theme-*` is selected.