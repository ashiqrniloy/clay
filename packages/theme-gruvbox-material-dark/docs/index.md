# @clay/theme-gruvbox-material-dark

A first-party **inert style-data** theme for Clay: the [Gruvbox Material](https://github.com/sainnhe/gruvbox-material) palette, **dark, medium contrast**, mapped onto Clay's two-axis syntax vocabulary (`TokenType` + `Modifiers`) and base UI color keys. Selected at runtime by the user via `setTheme("@clay/theme-gruvbox-material-dark")` in `~/.config/clay/init.js`.

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
- **Base UI:** shellBg→bg0 hard, panelBg→bg0, text→fg0, placeholder→grey0, selection→blue @ ~20%, caret→fg0, scrollbar→grey1, scrollbarTrack→grey0, statusBg→bg1, statusText→fg0.

To retune a value, edit the `textStyles` array in `package.json` — every color is a tunable inert value; no code change or rebuild of Clay is needed.