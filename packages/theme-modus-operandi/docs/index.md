# @clay/theme-modus-operandi

A first-party **inert style-data** theme for Clay: the [Modus Operandi](https://protesilaos.com/emacs/modus-themes) palette — the canonical **light** Modus theme, white background, WCAG AAA contrast intent — mapped onto Clay's two-axis syntax vocabulary (`TokenType` + `Modifiers`) and base UI color keys. It is Clay's canonical light-mode default (selected via the `appearance` preference) and can also be pinned explicitly with `setTheme("@clay/theme-modus-operandi")` in `~/.config/clay/init.js`.

This package carries **no executable authority**. All overrides are static `clay.contributions.textStyles` entries in [`package.json`](../package.json), parsed and validated by Clay at load and resolved into the single source of color (`StyleRegistry`). There is no runtime registration, no ops, no widgets, and no raw CSS — only hex colors + optional text-attribute flags (`bold`/`italic`/`underline`/`strike`).

## Provenance and license

- Upstream: [Modus themes](https://protesilaos.com/emacs/modus-themes) by Protesilaos Stavrou — `modus-operandi` (main light theme), palette version 4.6.0.
- Palette source consulted: [Colours of the Modus themes](https://protesilaos.com/emacs/modus-themes-colors) and the upstream `modus-operandi-theme.el`; syntax mappings follow the upstream semantic mappings (keyword→magenta-cooler, string→blue-warmer, comment→fg-dim, type/namespace→cyan family, macro/special→red-cooler, headings bold per `markup.heading.*`).
- License: the Modus themes are distributed under the GNU GPL v3.0 or later; this palette data is a derived adaptation. Attribution: Copyright (C) 2020-2024 Protesilaos Stavrou.

## Palette (subset used)

| Role | Hex | Role | Hex |
|------|-----|------|-----|
| bg-main (shell) | `#ffffff` | bg-dim (panel) | `#f2f2f2` |
| fg-main (text) | `#000000` | fg-dim (muted) | `#595959` |
| fg-alt | `#193668` | magenta-cooler | `#531ab6` |
| magenta | `#721045` | magenta-warmer | `#8f0075` |
| magenta-faint | `#7c318f` | blue-warmer | `#3548cf` |
| blue-cooler | `#0000b0` | red-cooler | `#a0132f` |
| cyan | `#005e8b` | cyan-cooler | `#005f5f` |
| green-cooler | `#00663f` | yellow-faint | `#624416` |
| green-faint | `#2a5045` | red-faint | `#7f0000` |
| bg-mode-line-active | `#c8c8c8` | bg-completion | `#c0deff` |
| red (error) | `#a60000` | yellow-warmer (warning) | `#884900` |

## Token mapping (summary)

- **Code:** Keyword→magenta-cooler, Modifier→magenta-warmer, String→blue-warmer, Comment→fg-dim, Regexp→magenta-faint, Function/Method→magenta, Macro/Decorator/Event→red-cooler, Namespace/TypeParameter/Parameter/Property→cyan, Type/Class/Enum/Interface/Struct→cyan-cooler, EnumMember→blue-cooler, Operator/Number/Variable→fg-main (upstream keeps these uncolored).
- **Prose:** Heading1-6→fg-main/yellow-faint/fg-alt/magenta/green-faint/red-faint (all bold, per upstream `markup.heading.*`), Quote/ListItem→fg-dim, CodeSpan→green-cooler, Link→blue-warmer (underline), CodeBlock/Paragraph→fg-main.
- **Base UI:** shellBg→bg-main, panelBg→bg-dim, text→fg-main, placeholder→fg-dim, selection→bg-completion @40%, caret→fg-main, scrollbar/track→border tones, statusBg→bg-mode-line-active, statusText→fg-mode-line-active, diagnostics→red/yellow-warmer/cyan-cooler.

To retune a value, edit the `textStyles` array in `package.json` — every color is a tunable inert value; no code change or rebuild of Clay is needed.
