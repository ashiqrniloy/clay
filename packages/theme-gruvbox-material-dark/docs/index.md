# @clay/theme-gruvbox-material-dark

A first-party **inert style-data** theme for Clay: the [Gruvbox Material](https://github.com/sainnhe/gruvbox-material) palette, **dark, medium contrast**, mapped onto Clay's two-axis syntax vocabulary (`TokenType` + `Modifiers`) and base UI color keys. Selected at runtime by the user via `setTheme("@clay/theme-gruvbox-material-dark")` in `~/.clay/init.js`.

This package carries **no executable authority**. All overrides are static `clay.contributions.textStyles` entries in [`package.json`](../package.json), parsed and validated by Clay at load and resolved into the single source of color (`StyleRegistry`). There is no runtime registration, no ops, no widgets, and no raw CSS — only hex colors + optional text-attribute flags (`bold`/`italic`/`underline`/`strike`).

## Palette

Reproduced from [sainnhe/gruvbox-material](https://github.com/sainnhe/gruvbox-material) (dark, medium). Accent hexes:

| Role        | Hex      | Role        | Hex      |
|-------------|----------|-------------|----------|
| bg0 (panel) | `#282828`| bg0 hard    | `#1d2021`|
| bg1 (status)| `#32302f`| fg0 (text)  | `#d4be98`|
| fg1         | `#ddc7a1`| grey0 (fade)| `#7c6f64`|
| red         | `#ea6962`| orange      | `#e78a4e`|
| yellow      | `#d8a657`| green       | `#a9b665`|
| aqua        | `#89b482`| blue        | `#7daea3`|
| purple      | `#d3869b`|             |          |

## Token mapping (summary)

- **Code (LSP base):** Keyword→red (bold), String→green, Comment→grey0 (italic), Number→purple, Regexp→aqua, Operator→aqua, Function/Method→yellow (Function bold), Macro/Decorator→purple, Namespace/Enum/Interface→blue, Type→yellow, Class/Struct/TypeParameter/Event→orange, Variable/Parameter/Property→fg1, EnumMember→green.
- **Prose (Clay extension):** Heading1-6→red/orange/yellow/green/aqua/blue (the first three bold), Quote→grey0 (italic), CodeSpan→orange, Link→blue (underline), ListItem/CodeBlock/Paragraph→fg0.
- **Base UI:** shellBg→bg0 (the canvas), panelBg→bg0 hard (the chrome), text→fg0, placeholder→grey0, selection→blue @ ~20%, caret→fg0, scrollbar→grey1, scrollbarTrack→grey0, statusBg→bg1, statusText→fg0.

## UI roles (`designTokens`)

The Quiet Instrument language's theme-side roles (`DESIGN.md` §10): geometry and material come from the design system, the *colours* are the theme's. All 13 roles below are declared in this package's `clay.contributions.designTokens`, identically in every shipped theme, so no role silently falls back to Clay's core palette. Approved board: `design-artifacts/approved/quiet-instrument-migration/themes.html`.

| Role | Value | What it is |
| --- | --- | --- |
| `border.hairline` | `#92837457` | Quiet zone divider: the theme's structural grey at 34 percent. Below the 3:1 floor on purpose — a divider is decoration, and a control is identified by its label and by its hover/active/focus states. |
| `border.subtle` | `#928374` | Structural boundary and hover outline: the same grey at full strength, and the step the 3:1 floor measures. |
| `border.strong` | `#d4be98` | Rare explicit separator: the theme's ink. |
| `surface.hover` | `#3c3836` | Opaque hover fill. |
| `surface.active` | `#45403d` | Opaque pressed fill. |
| `surface.selected` | `#3a4438` | Opaque selected fill. |
| `accent.primary` | `#7daea3` | The theme's accent (aqua) — previously the accent resolved to the caret beige `#d4be98`, so a state signal and the text colour were the same colour. |
| `accent.muted` | `#7daea3bf` | The accent one step down (75 percent alpha) for quieter affordances, still at or above 3:1 on the canvas. |
| `focus.ring` | `#7daea3` | Focus is a state, so focus is the accent. |
| `border.focus` | `#7daea3` | The same accent, so a focused control's border and its ring are one colour. |
| `text.muted` | `#b3a284` | Dim text step, at or above 4.5:1 on the panel. |
| `text.disabled` | `#ab9d89` | Meta/disabled step, at or above 4.5:1 on the canvas. |
| `surface.scrim` | `#1d2021` | The chrome colour `#1d2021`, darker than the canvas, so dimming still darkens. |

**`textStyles` correction (not a role):** `shellBg`/`panelBg` swap `#1d2021`/`#282828` → `#282828`/`#1d2021`. The approved palette puts the canvas at `#282828` and the chrome at `#1d2021`; shipping the pair the other way round made the panel read lighter than the canvas, the inverted depth direction `DESIGN.md` §10.2 forbids. The swap fixes the shell, the editor and the SDUI at once.

To retune a value, edit the `textStyles` and `designTokens` arrays in `package.json` — every color is a tunable inert value; no code change or rebuild of Clay is needed.