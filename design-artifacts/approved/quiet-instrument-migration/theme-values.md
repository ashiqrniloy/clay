# Theme values — Quiet Instrument migration

The four shipped themes at their current values next to the values the language needs, with every
ratio measured off the composited colour by `design-artifacts/tools/make-theme-values.py`. Regenerate
with `python3 design-artifacts/tools/make-theme-values.py`; `--check` fails on drift or a missed floor.
The visual board is `themes.html` (all four themes at once, `file://`, no network).

**This file is the specification for tasks 13–14.** Each block below is the exact
`clay.contributions.designTokens` entry the package will gain. No value is a new colour source:
every one comes from the theme's own approved palette (`theme.css`), its ink, or its border grey.
Host CSS and design-system recipes receive **no colour** from this board: they name roles, and a value
lands only in a theme package.

- Floors: text ≥ 4.5:1, UI affordances ≥ 3.0:1 (`DESIGN.md` §13.1).
- Ratios are measured on the composited colour (alpha over the surface it is drawn on). The runtime's
  `contrast_ratio` currently ignores alpha — see finding 2.
- `border.hairline` is measured but exempt from the floor: it separates zones, it does not identify a
  control or a state (WCAG 1.4.11). What it must keep is order — quieter than `border.subtle`.

## modus-operandi (`@clay/theme-modus-operandi` 0.1.0)

```json
"clay": { "contributions": { "designTokens": [
    { "token": "border.hairline", "value": "#87878757" },
    { "token": "border.subtle", "value": "#878787" },
    { "token": "border.strong", "value": "#000000" },
    { "token": "surface.hover", "value": "#e6e6e6" },
    { "token": "surface.active", "value": "#d8d8d8" },
    { "token": "surface.selected", "value": "#c9e2ff" },
    { "token": "accent.primary", "value": "#0031a9" },
    { "token": "accent.muted", "value": "#0031a9bf" },
    { "token": "focus.ring", "value": "#0031a9" },
    { "token": "border.focus", "value": "#0031a9" },
    { "token": "text.muted", "value": "#4f4f4f" },
    { "token": "text.disabled", "value": "#6b6b6b" },
    { "token": "surface.scrim", "value": "#000000" }
] } }
```

| Role | Current value | Current on canvas | Proposed | Proposed on canvas | Composited vs canvas |
| --- | --- | --- | --- | --- | --- |
| `border.hairline` | `#9f9f9faa` | `#bfbfbfff` | `#87878757` | `#d6d6d6ff` | 1.45:1 |
| `border.subtle` | `#9f9f9faa` | `#bfbfbfff` | `#878787` | `#878787ff` | 3.59:1 |
| `border.strong` | `#9f9f9faa` | `#bfbfbfff` | `#000000` | `#000000ff` | 21.00:1 |
| `surface.hover` | `#c0deff66` | `#e6f2ffff` | `#e6e6e6` | `#e6e6e6ff` | 1.25:1 |
| `surface.active` | `#c0deff66` | `#e6f2ffff` | `#d8d8d8` | `#d8d8d8ff` | 1.43:1 |
| `surface.selected` | `#c0deff66` | `#e6f2ffff` | `#c9e2ff` | `#c9e2ffff` | 1.33:1 |
| `accent.primary` | `#000000` | `#000000ff` | `#0031a9` | `#0031a9ff` | 10.44:1 |
| `accent.muted` | `#595959` | `#595959ff` | `#0031a9bf` | `#4065bfff` | 5.48:1 |
| `focus.ring` | `#000000` | `#000000ff` | `#0031a9` | `#0031a9ff` | 10.44:1 |
| `border.focus` | `#000000` | `#000000ff` | `#0031a9` | `#0031a9ff` | 10.44:1 |
| `text.muted` | `#595959` | `#595959ff` | `#4f4f4f` | `#4f4f4fff` | 8.19:1 |
| `text.disabled` | `#595959` | `#595959ff` | `#6b6b6b` | `#6b6b6bff` | 5.33:1 |
| `surface.scrim` | `#000000` | `#000000ff` | `#000000` | `#000000ff` | 21.00:1 |

### Border ladder

Border grey (the approved palette's `line-2`): `#878787` at 34 / 62 / 100 %.

| Step | Value | Renders as | Composited vs canvas |
| --- | --- | --- | --- |
| `border.hairline` | `#87878757` | `#d6d6d6ff` | 1.45:1 |
| `border.subtle` | `#878787` | `#878787ff` | 3.59:1 |
| `border.strong` | `#000000` | `#000000ff` | 21.00:1 |

Monotonic: **yes** — the quiet line stays quieter than the structural one, which stays quieter than the explicit separator.

Depth direction (`DESIGN.md` §10.2): chrome `#f0f0f0`, canvas `#ffffff` — canvas is lighter, as required. Composed depth step 1.12:1 (today 1.12:1).

Scrim: `#000000` darkens the canvas -> at `opacity.scrim` 0.50 the canvas becomes `#7f7f7fff` (4.00:1 dim step, 21.00:1 from the raw colour). The theme supplies the colour; the design system supplies the opacity and the 3px blur (`DESIGN.md` §6) - the dim step is what the eye reads.

### Enforced pairs

| Pair | Floor | Current | Proposed | |
| --- | --- | --- | --- | --- |
| `text.muted on surface.panel` | 4.5:1 | 6.26:1 | 7.32:1 | ok |
| `text.disabled on surface.main` | 4.5:1 | 7.00:1 | 5.33:1 | ok |
| `accent.primary on surface.main` | 3.0:1 | 21.00:1 | 10.44:1 | ok |
| `accent.muted on surface.main` | 3.0:1 | 7.00:1 | 5.48:1 | ok |
| `focus.ring on surface.main` | 3.0:1 | 21.00:1 | 10.44:1 | ok |
| `border.focus on surface.main` | 3.0:1 | 21.00:1 | 10.44:1 | ok |
| `border.subtle on surface.main` | 3.0:1 | 1.84:1 (fails today) | 3.59:1 | ok |
| `border.subtle on surface.panel` | 3.0:1 | 1.71:1 (fails today) | 3.21:1 | ok |
| `border.strong on surface.main` | 3.0:1 | 1.84:1 (fails today) | 21.00:1 | ok |
| `surface.selected on text.primary` | 3.0:1 | 2.94:1 (fails today) | 15.81:1 | ok |
| `surface.hover on text.primary` | 3.0:1 | 2.94:1 (fails today) | 16.83:1 | ok |
| `surface.active on text.primary` | 3.0:1 | 2.94:1 (fails today) | 14.73:1 | ok |


## modus-vivendi (`@clay/theme-modus-vivendi` 0.1.0)

```json
"clay": { "contributions": { "designTokens": [
    { "token": "border.hairline", "value": "#6f6f6f57" },
    { "token": "border.subtle", "value": "#6f6f6f" },
    { "token": "border.strong", "value": "#ffffff" },
    { "token": "surface.hover", "value": "#262626" },
    { "token": "surface.active", "value": "#333333" },
    { "token": "surface.selected", "value": "#2f447f" },
    { "token": "accent.primary", "value": "#2fafff" },
    { "token": "accent.muted", "value": "#2fafffbf" },
    { "token": "focus.ring", "value": "#2fafff" },
    { "token": "border.focus", "value": "#2fafff" },
    { "token": "text.muted", "value": "#c3c3c3" },
    { "token": "text.disabled", "value": "#8f8f8f" },
    { "token": "surface.scrim", "value": "#000000" }
] } }
```

| Role | Current value | Current on canvas | Proposed | Proposed on canvas | Composited vs canvas |
| --- | --- | --- | --- | --- | --- |
| `border.hairline` | `#535353aa` | `#373737ff` | `#6f6f6f57` | `#262626ff` | 1.39:1 |
| `border.subtle` | `#535353aa` | `#373737ff` | `#6f6f6f` | `#6f6f6fff` | 4.18:1 |
| `border.strong` | `#535353aa` | `#373737ff` | `#ffffff` | `#ffffffff` | 21.00:1 |
| `surface.hover` | `#2f447f99` | `#1c294cff` | `#262626` | `#262626ff` | 1.39:1 |
| `surface.active` | `#2f447f99` | `#1c294cff` | `#333333` | `#333333ff` | 1.66:1 |
| `surface.selected` | `#2f447f99` | `#1c294cff` | `#2f447f` | `#2f447fff` | 2.25:1 |
| `accent.primary` | `#ffffff` | `#ffffffff` | `#2fafff` | `#2fafffff` | 8.70:1 |
| `accent.muted` | `#989898` | `#989898ff` | `#2fafffbf` | `#2383bfff` | 5.07:1 |
| `focus.ring` | `#ffffff` | `#ffffffff` | `#2fafff` | `#2fafffff` | 8.70:1 |
| `border.focus` | `#ffffff` | `#ffffffff` | `#2fafff` | `#2fafffff` | 8.70:1 |
| `text.muted` | `#989898` | `#989898ff` | `#c3c3c3` | `#c3c3c3ff` | 11.91:1 |
| `text.disabled` | `#989898` | `#989898ff` | `#8f8f8f` | `#8f8f8fff` | 6.49:1 |
| `surface.scrim` | `#000000` | `#000000ff` | `#000000` | `#000000ff` | 1.00:1 |

### Border ladder

Border grey (the approved palette's `line-2`): `#6f6f6f` at 34 / 62 / 100 %.

| Step | Value | Renders as | Composited vs canvas |
| --- | --- | --- | --- |
| `border.hairline` | `#6f6f6f57` | `#262626ff` | 1.39:1 |
| `border.subtle` | `#6f6f6f` | `#6f6f6fff` | 4.18:1 |
| `border.strong` | `#ffffff` | `#ffffffff` | 21.00:1 |

Monotonic: **yes** — the quiet line stays quieter than the structural one, which stays quieter than the explicit separator.

Depth direction (`DESIGN.md` §10.2): chrome `#000000`, canvas `#000000` — canvas is lighter, as required. Composed depth step 1.26:1 (today 1.26:1).

Scrim: `#000000` darkens the canvas -> at `opacity.scrim` 0.50 the canvas becomes `#000000ff` (1.00:1 dim step, 1.00:1 from the raw colour). The theme supplies the colour; the design system supplies the opacity and the 3px blur (`DESIGN.md` §6) - a canvas this dark cannot be dimmed further, so separation comes from the overlay surface and its shadow - the scrim's job is only to stop the canvas competing.

### Enforced pairs

| Pair | Floor | Current | Proposed | |
| --- | --- | --- | --- | --- |
| `text.muted on surface.panel` | 4.5:1 | 5.78:1 | 9.46:1 | ok |
| `text.disabled on surface.main` | 4.5:1 | 7.28:1 | 6.49:1 | ok |
| `accent.primary on surface.main` | 3.0:1 | 21.00:1 | 8.70:1 | ok |
| `accent.muted on surface.main` | 3.0:1 | 7.28:1 | 5.07:1 | ok |
| `focus.ring on surface.main` | 3.0:1 | 21.00:1 | 8.70:1 | ok |
| `border.focus on surface.main` | 3.0:1 | 21.00:1 | 8.70:1 | ok |
| `border.subtle on surface.main` | 3.0:1 | 1.76:1 (fails today) | 4.18:1 | ok |
| `border.subtle on surface.panel` | 3.0:1 | 1.63:1 (fails today) | 3.32:1 | ok |
| `border.strong on surface.main` | 3.0:1 | 1.76:1 (fails today) | 21.00:1 | ok |
| `surface.selected on text.primary` | 3.0:1 | 3.22:1 | 9.32:1 | ok |
| `surface.hover on text.primary` | 3.0:1 | 3.22:1 | 15.13:1 | ok |
| `surface.active on text.primary` | 3.0:1 | 3.22:1 | 12.63:1 | ok |


## gruvbox-material-dark (`@clay/theme-gruvbox-material-dark` 0.1.0)

```json
"clay": { "contributions": { "designTokens": [
    { "token": "border.hairline", "value": "#92837457" },
    { "token": "border.subtle", "value": "#928374" },
    { "token": "border.strong", "value": "#d4be98" },
    { "token": "surface.hover", "value": "#3c3836" },
    { "token": "surface.active", "value": "#45403d" },
    { "token": "surface.selected", "value": "#3a4438" },
    { "token": "accent.primary", "value": "#7daea3" },
    { "token": "accent.muted", "value": "#7daea3bf" },
    { "token": "focus.ring", "value": "#7daea3" },
    { "token": "border.focus", "value": "#7daea3" },
    { "token": "text.muted", "value": "#b3a284" },
    { "token": "text.disabled", "value": "#ab9d89" },
    { "token": "surface.scrim", "value": "#1d2021" }
] } }
```

| Role | Current value | Current on canvas | Proposed | Proposed on canvas | Composited vs canvas |
| --- | --- | --- | --- | --- | --- |
| `border.hairline` | `#928374aa` | `#6b6258ff` | `#92837457` | `#4c4742ff` | 1.61:1 |
| `border.subtle` | `#928374aa` | `#6b6258ff` | `#928374` | `#928374ff` | 4.02:1 |
| `border.strong` | `#928374aa` | `#6b6258ff` | `#d4be98` | `#d4be98ff` | 8.16:1 |
| `surface.hover` | `#7daea333` | `#303c3bff` | `#3c3836` | `#3c3836ff` | 1.27:1 |
| `surface.active` | `#7daea333` | `#303c3bff` | `#45403d` | `#45403dff` | 1.44:1 |
| `surface.selected` | `#7daea333` | `#303c3bff` | `#3a4438` | `#3a4438ff` | 1.45:1 |
| `accent.primary` | `#d4be98` | `#d4be98ff` | `#7daea3` | `#7daea3ff` | 5.94:1 |
| `accent.muted` | `#7c6f64` | `#7c6f64ff` | `#7daea3bf` | `#688c84ff` | 3.98:1 |
| `focus.ring` | `#d4be98` | `#d4be98ff` | `#7daea3` | `#7daea3ff` | 5.94:1 |
| `border.focus` | `#d4be98` | `#d4be98ff` | `#7daea3` | `#7daea3ff` | 5.94:1 |
| `text.muted` | `#d4be98` | `#d4be98ff` | `#b3a284` | `#b3a284ff` | 5.91:1 |
| `text.disabled` | `#7c6f64` | `#7c6f64ff` | `#ab9d89` | `#ab9d89ff` | 5.56:1 |
| `surface.scrim` | `#000000` | `#000000ff` | `#1d2021` | `#1d2021ff` | 1.11:1 |

### Border ladder

Border grey (the approved palette's `line-2`): `#928374` at 34 / 62 / 100 %.

| Step | Value | Renders as | Composited vs canvas |
| --- | --- | --- | --- |
| `border.hairline` | `#92837457` | `#4c4742ff` | 1.61:1 |
| `border.subtle` | `#928374` | `#928374ff` | 4.02:1 |
| `border.strong` | `#d4be98` | `#d4be98ff` | 8.16:1 |

Monotonic: **yes** — the quiet line stays quieter than the structural one, which stays quieter than the explicit separator.

Depth direction (`DESIGN.md` §10.2): chrome `#1d2021`, canvas `#282828` — canvas is lighter, as required. Composed depth step 1.11:1 (today 1.11:1). The canvas also moves `#1d2021` → `#282828`, because today's pair is inverted (the panel is the lighter surface, which §10.2 forbids).

**Not a designTokens entry — a `textStyles` correction this theme needs.**

Swap `shellBg`/`panelBg`: ``#1d2021``/``#282828`` → ``#282828``/``#1d2021``.

The approved palette puts the canvas at #282828 and the chrome at #1d2021; the package ships the opposite (shellBg #1d2021 as the canvas), so today the theme inverts depth: the panel reads lighter than the canvas. DESIGN.md 10.2 requires the chrome to be the darker of the two on every theme. Swapping the two textStyles values fixes the shell, the editor and the SDUI in one move and needs no designTokens entry - the roles keep resolving from textStyles as they do today.

Scrim: `#1d2021` darkens the canvas -> at `opacity.scrim` 0.50 the canvas becomes `#222424ff` (1.06:1 dim step, 1.11:1 from the raw colour). The theme supplies the colour; the design system supplies the opacity and the 3px blur (`DESIGN.md` §6) - a canvas this dark cannot be dimmed further, so separation comes from the overlay surface and its shadow - the scrim's job is only to stop the canvas competing.

### Enforced pairs

| Pair | Floor | Current | Proposed | |
| --- | --- | --- | --- | --- |
| `text.muted on surface.panel` | 4.5:1 | 8.16:1 | 6.57:1 | ok |
| `text.disabled on surface.main` | 4.5:1 | 3.37:1 (fails today) | 5.56:1 | ok |
| `accent.primary on surface.main` | 3.0:1 | 9.07:1 | 5.94:1 | ok |
| `accent.muted on surface.main` | 3.0:1 | 3.37:1 | 3.98:1 | ok |
| `focus.ring on surface.main` | 3.0:1 | 9.07:1 | 5.94:1 | ok |
| `border.focus on surface.main` | 3.0:1 | 9.07:1 | 5.94:1 | ok |
| `border.subtle on surface.main` | 3.0:1 | 2.74:1 (fails today) | 4.02:1 | ok |
| `border.subtle on surface.panel` | 3.0:1 | 2.59:1 (fails today) | 4.47:1 | ok |
| `border.strong on surface.main` | 3.0:1 | 2.74:1 (fails today) | 8.16:1 | ok |
| `surface.selected on text.primary` | 3.0:1 | 1.07:1 (fails today) | 5.63:1 | ok |
| `surface.hover on text.primary` | 3.0:1 | 1.07:1 (fails today) | 6.42:1 | ok |
| `surface.active on text.primary` | 3.0:1 | 1.07:1 (fails today) | 5.66:1 | ok |


## gruvbox-material-light (`@clay/theme-gruvbox-material-light` 0.1.0)

```json
"clay": { "contributions": { "designTokens": [
    { "token": "border.hairline", "value": "#8a7a6357" },
    { "token": "border.subtle", "value": "#8a7a63" },
    { "token": "border.strong", "value": "#504940" },
    { "token": "surface.hover", "value": "#ece0b6" },
    { "token": "surface.active", "value": "#e3d3a5" },
    { "token": "surface.selected", "value": "#dbdfae" },
    { "token": "accent.primary", "value": "#076678" },
    { "token": "accent.muted", "value": "#076678bf" },
    { "token": "focus.ring", "value": "#076678" },
    { "token": "border.focus", "value": "#076678" },
    { "token": "text.muted", "value": "#6b5c48" },
    { "token": "text.disabled", "value": "#71624b" },
    { "token": "surface.scrim", "value": "#504940" }
] } }
```

| Role | Current value | Current on canvas | Proposed | Proposed on canvas | Composited vs canvas |
| --- | --- | --- | --- | --- | --- |
| `border.hairline` | `#928374aa` | `#b5a890ff` | `#8a7a6357` | `#d4c8a5ff` | 1.47:1 |
| `border.subtle` | `#928374aa` | `#b5a890ff` | `#8a7a63` | `#8a7a63ff` | 3.67:1 |
| `border.strong` | `#928374aa` | `#b5a890ff` | `#504940` | `#504940ff` | 7.82:1 |
| `surface.hover` | `#07667833` | `#cad5b7ff` | `#ece0b6` | `#ece0b6ff` | 1.16:1 |
| `surface.active` | `#07667833` | `#cad5b7ff` | `#e3d3a5` | `#e3d3a5ff` | 1.31:1 |
| `surface.selected` | `#07667833` | `#cad5b7ff` | `#dbdfae` | `#dbdfaeff` | 1.22:1 |
| `accent.primary` | `#504940` | `#504940ff` | `#076678` | `#076678ff` | 5.82:1 |
| `accent.muted` | `#a89984` | `#a89984ff` | `#076678bf` | `#44898cff` | 3.56:1 |
| `focus.ring` | `#504940` | `#504940ff` | `#076678` | `#076678ff` | 5.82:1 |
| `border.focus` | `#504940` | `#504940ff` | `#076678` | `#076678ff` | 5.82:1 |
| `text.muted` | `#504940` | `#504940ff` | `#6b5c48` | `#6b5c48ff` | 5.70:1 |
| `text.disabled` | `#a89984` | `#a89984ff` | `#71624b` | `#71624bff` | 5.21:1 |
| `surface.scrim` | `#000000` | `#000000ff` | `#504940` | `#504940ff` | 7.82:1 |

### Border ladder

Border grey (the approved palette's `line-2`): `#8a7a63` at 34 / 62 / 100 %.

| Step | Value | Renders as | Composited vs canvas |
| --- | --- | --- | --- |
| `border.hairline` | `#8a7a6357` | `#d4c8a5ff` | 1.47:1 |
| `border.subtle` | `#8a7a63` | `#8a7a63ff` | 3.67:1 |
| `border.strong` | `#504940` | `#504940ff` | 7.82:1 |

Monotonic: **yes** — the quiet line stays quieter than the structural one, which stays quieter than the explicit separator.

Depth direction (`DESIGN.md` §10.2): chrome `#f2e5bc`, canvas `#fbf1c7` — canvas is lighter, as required. Composed depth step 1.11:1 (today 1.11:1).

Scrim: `#504940` darkens the canvas -> at `opacity.scrim` 0.50 the canvas becomes `#a59d83ff` (2.39:1 dim step, 7.82:1 from the raw colour). The theme supplies the colour; the design system supplies the opacity and the 3px blur (`DESIGN.md` §6) - the dim step is what the eye reads.

### Enforced pairs

| Pair | Floor | Current | Proposed | |
| --- | --- | --- | --- | --- |
| `text.muted on surface.panel` | 4.5:1 | 7.06:1 | 5.14:1 | ok |
| `text.disabled on surface.main` | 4.5:1 | 2.45:1 (fails today) | 5.21:1 | ok |
| `accent.primary on surface.main` | 3.0:1 | 7.82:1 | 5.82:1 | ok |
| `accent.muted on surface.main` | 3.0:1 | 2.45:1 (fails today) | 3.56:1 | ok |
| `focus.ring on surface.main` | 3.0:1 | 7.82:1 | 5.82:1 | ok |
| `border.focus on surface.main` | 3.0:1 | 7.82:1 | 5.82:1 | ok |
| `border.subtle on surface.main` | 3.0:1 | 2.06:1 (fails today) | 3.67:1 | ok |
| `border.subtle on surface.panel` | 3.0:1 | 1.95:1 (fails today) | 3.31:1 | ok |
| `border.strong on surface.main` | 3.0:1 | 2.06:1 (fails today) | 7.82:1 | ok |
| `surface.selected on text.primary` | 3.0:1 | 1.03:1 (fails today) | 6.41:1 | ok |
| `surface.hover on text.primary` | 3.0:1 | 1.03:1 (fails today) | 6.72:1 | ok |
| `surface.active on text.primary` | 3.0:1 | 1.03:1 (fails today) | 5.97:1 | ok |


## Core fallback (no design system installed)

What the same roles resolve to with no design system active — `core_theme_value` in
`src/shell/theme.rs`, parsed and measured by the same script, because the migration must not
leave a role depending on it once a shipped theme covers it (task 13: identical token coverage
across the four themes).

| Role | Core fallback | Composited on core canvas |
| --- | --- | --- |
| `border.hairline` | `#282638` | 1.29:1 |
| `border.subtle` | `#2f2c40` | 1.41:1 |
| `border.strong` | `#45415c` | 1.96:1 |
| `surface.hover` | `#2d2b3d` | 1.38:1 |
| `surface.active` | `#343147` | 1.52:1 |
| `surface.selected` | `#3d385c` | 1.74:1 |
| `accent.primary` | `#7c6fff` | 5.06:1 |
| `accent.muted` | `#5a52b8` | 3.02:1 |
| `focus.ring` | `#968aff` | 6.69:1 |
| `border.focus` | `#7c6fff` | 5.06:1 |
| `text.muted` | `#b9b2cf` | 9.37:1 |
| `text.disabled` | `#6f6a87` | 3.71:1 |
| `surface.scrim` | `#000000` | 1.10:1 |

## Findings

1. **The approved artifact and `DESIGN.md` §10.1 disagree about how the ladder is defined.**
   §10.1 says the steps are the theme's *ink* at 34 / 62 / 100 %; the approved language
   implements them as the theme's *border grey* (`--c-line-2`) at those alphas
   (`--hairline` / `--hairline-strong` in `ds-quiet.css`). Both are mechanical. This board
   proposes the border grey, because that is what the approved screens render (ink at 34 %
   composites ~0.2 darker than the approved divider) and because the same grey at 100 % is the
   structural border the screens already draw. Task 15 must reword §10.1 to match, or the spec
   and the artifact stay in disagreement.
2. **The contrast gate does not composite alpha.** `editor::theme::contrast_ratio` reads
   `to_rgba8()` and ignores the alpha byte, so today a 34 %-alpha hairline measures as opaque
   ink (21:1 on white) while it renders at ~1.5:1. Any required pair whose foreground carries
   alpha must composite first, or the `border.subtle` floor this board adds is decoration.
   Belongs to task 14.
3. **One step, two names.** `theme.css` defines `--c-line` and the language computes
   `--hairline` from `--c-line-2` at 34 %; measured, they land within a few percent of each
   other on all four themes, and `--c-line` has no consumer left in `ds-quiet.css` or
   `components.css`. `border.hairline` replaces both. Task 16 deletes the dead variable.
4. **`surface.scrim` has a core fallback but no projection from `textStyles`.** `base_color`
   has no arm for it, so a legacy theme dims with the core catalog's pure black (or worse, a
   dark theme's core value) rather than its own ink. It is in the proposal for that reason:
   the theme should say what dimming means for it.
5. **Today every state fill is the same 40 %-alpha selection colour and every accent-driven
   role resolves to the monochrome caret** (`accent.primary`, `accent.muted`, `focus.ring`,
   `border.focus`). `DESIGN.md` §10.3 requires the fills to be opaque, and a focus ring the same
   colour as the text is not a ring. The proposal replaces both with values the theme already
   owns.
6. **One theme inverts depth, and fixing it is a `textStyles` swap rather than a token.** `theme-gruvbox-material-dark` ships `shellBg` #1d2021 and `panelBg` #282828, so `surface.main` resolves to the *chrome* colour and the panel comes out lighter than the canvas - the 'inverted panel' `DESIGN.md` §10.2 exists to forbid, and the reason the approved palette swaps them. The other three themes already resolve depth correctly (Modus Operandi and Gruvbox Material Light ship the lighter colour as `shellBg`; Modus Vivendi is black on black by design). Swapping the two values fixes the shell, the editor and the SDUI at once, needs no designTokens entry, and is why the board records it beside the token list instead of hiding it in one. Task 13's 'keep textStyles untouched' must be relaxed for exactly that one pair.

## Approval

This board is a **prototype** (`design-artifacts/README.md`): exploratory until the user
approves it. On approval it is frozen to
`design-artifacts/approved/quiet-instrument-migration/themes.html` and becomes the binding
specification for tasks 13–14.
