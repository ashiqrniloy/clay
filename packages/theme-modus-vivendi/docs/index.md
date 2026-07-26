# @clay/theme-modus-vivendi

A first-party **inert style-data** theme for Clay: the [Modus Vivendi](https://protesilaos.com/emacs/modus-themes) palette — the canonical **dark** Modus theme, black background, WCAG AAA contrast intent — mapped onto Clay's two-axis syntax vocabulary (`TokenType` + `Modifiers`) and base UI color keys. It is Clay's canonical dark-mode default (selected via the `appearance` preference) and can also be pinned explicitly with `setTheme("@clay/theme-modus-vivendi")` in `~/.config/clay/init.js`.

This package carries **no executable authority**. All overrides are static `clay.contributions.textStyles` entries in [`package.json`](../package.json), parsed and validated by Clay at load and resolved into the single source of color (`StyleRegistry`). There is no runtime registration, no ops, no widgets, and no raw CSS — only hex colors + optional text-attribute flags (`bold`/`italic`/`underline`/`strike`).

## Provenance and license

- Upstream: [Modus themes](https://protesilaos.com/emacs/modus-themes) by Protesilaos Stavrou — `modus-vivendi` (main dark theme), palette version 4.6.0.
- Palette source consulted: [Colours of the Modus themes](https://protesilaos.com/emacs/modus-themes-colors) and the upstream `modus-vivendi-theme.el`; syntax mappings follow the upstream semantic mappings (keyword→magenta-cooler, string→blue-warmer, comment→fg-dim, type/namespace→cyan family, macro/special→red-cooler, headings bold per `markup.heading.*`).
- License: the Modus themes are distributed under the GNU GPL v3.0 or later; this palette data is a derived adaptation. Attribution: Copyright (C) 2020-2024 Protesilaos Stavrou.

## Palette (subset used)

| Role | Hex | Role | Hex |
|------|-----|------|-----|
| bg-main (shell) | `#000000` | bg-dim (panel) | `#1e1e1e` |
| fg-main (text) | `#ffffff` | fg-dim (muted) | `#989898` |
| fg-alt | `#c6daff` | magenta-cooler | `#b6a0ff` |
| magenta | `#feacd0` | magenta-warmer | `#f78fe7` |
| magenta-faint | `#caa6df` | blue-warmer | `#79a8ff` |
| blue-cooler | `#00bcff` | red-cooler | `#ff7f86` |
| cyan | `#00d3d0` | cyan-cooler | `#6ae4b9` |
| green-cooler | `#00c06f` | yellow-faint | `#d2b580` |
| green-faint | `#88ca9f` | red-faint | `#ff9580` |
| bg-mode-line-active | `#505050` | bg-completion | `#2f447f` |
| red (error) | `#ff5f59` | yellow-warmer (warning) | `#fec43f` |

## Token mapping (summary)

- **Code:** Keyword→magenta-cooler, Modifier→magenta-warmer, String→blue-warmer, Comment→fg-dim, Regexp→magenta-faint, Function/Method→magenta, Macro/Decorator/Event→red-cooler, Namespace/TypeParameter/Parameter/Property→cyan, Type/Class/Enum/Interface/Struct→cyan-cooler, EnumMember→blue-cooler, Operator/Number/Variable→fg-main (upstream keeps these uncolored).
- **Prose:** Heading1-6→fg-main/yellow-faint/fg-alt/magenta/green-faint/red-faint (all bold, per upstream `markup.heading.*`), Quote/ListItem→fg-dim, CodeSpan→green-cooler, Link→blue-warmer (underline), CodeBlock/Paragraph→fg-main.
- **Base UI:** shellBg→bg-main, panelBg→bg-dim, text→fg-main, placeholder→fg-dim, selection→bg-completion @60%, caret→fg-main, scrollbar/track→bg-active tones, statusBg→bg-mode-line-active, statusText→fg-mode-line-active, diagnostics→red/yellow-warmer/cyan-cooler.

To retune a value, edit the `textStyles` array in `package.json` — every color is a tunable inert value; no code change or rebuild of Clay is needed.
