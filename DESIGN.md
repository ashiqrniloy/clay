# Clay Design System — Quiet Instrument

Normative visual, structural, and interaction specification for Clay's UI.
Read this before designing, implementing, reviewing, or planning any UI change.
It replaces the former Restrained-Neobrutal direction as Clay's design
language while keeping the same architecture: **content themes own color, the
UI design system owns geometry/material/motion, user config owns typography,
React Aria + Clay own behavior and accessibility.**

Status: **approved direction; implemented for package, themes, gates and
fallbacks — host CSS adoption in progress.** The machine-readable package
profile (§16) is implemented by `@clay/design-instrument`
(`packages/design-instrument/`, plan 118 task 8), the former Neobrutal and Glass
packages are gone (task 9), the four shipped content themes carry the theme-side
roles of §10.1/§10.3 (task 13), and the contrast gate, the core fallback set and
the host pre-bootstrap projection now speak this language (tasks 14/16 — §16).
What remains is host adoption: the component CSS modules still carry pre-migration
geometry in their inline fallbacks and consume only part of the recipe set (plan
118 tasks 18/22), and the launcher / tab-model / session-files surfaces are drawn
and approved (`design-artifacts/approved/quiet-instrument-migration/`) but not
yet built (tasks 33–36). Migration work must not reintroduce the retired patterns
in §14.

Shell composition amendment (2026-09-17): the persistent **agent lane** and the
**`/` palette** (§6/§7/§9/§11/§12/§13/§14) are approved
(`design-artifacts/approved/agent-lane-palette/`) and **built**: plan 124 landed
the lane, the palette session behind it, the lane-anchored sheet
(`frontend/src/shell/AgentLane.tsx`, `frontend/src/command-centre/CommandPalette.tsx`),
and the row fields the §12 chips read (`scope`, `bindings` — protocol version 31),
so §16's shipped-state bullets for the composer, the agent view's header and the
centred command sheet describe the previous shell.

Provenance: `design-artifacts/approved/quiet-instrument-language/workspace-rethink.html`,
`design-artifacts/approved/quiet-instrument-language/agent-rethink.html`, `design-artifacts/approved/quiet-instrument-language/ds-quiet.css`, `design-artifacts/approved/quiet-instrument-language/theme.css`
(approved design language, 2026-09-11), and
`design-artifacts/approved/agent-lane-palette/lane-palette.html` + `lane.css`
(approved shell composition, 2026-09-17; contract: `design-artifacts/README.md`).
The approved artifact is the binding surface/state reference; where it and this
document disagree, this document is normative and the artifact is re-approved.
Hand-maintained file: Impeccable design-documentation runs **merge into** this
document — never overwrite it.

Related: `docs/reference/ui-design-systems.md` (recipe architecture),
`docs/development/ui-design-system-recipe-matrix.md` (surface/slot matrix),
`.agents/skills/clay-execution/references/tokens.md` (token catalog),
`.agents/skills/clay-execution/references/components.md` (component catalog).

---

## 1. The five laws

1. **One surface, hairline zones.** Zones are separated by whitespace and a
   single 1px hairline. Nothing is a box inside a box; a panel is a tinted
   plane, not a frame.
2. **Two elevations.** Canvas and overlay. Static regions never float. Only
   transient surfaces (popover, sheet, palette, toast, modal) carry a shadow.
3. **Accent is state, never decoration.** Accent marks focus, selection,
   active navigation, and running work. It never draws a panel edge, a
   heading, or a decorative icon.
4. **Type does the structure.** A short size ladder, monospace for every
   datum (counts, paths, keys, timestamps, identifiers), 10px tracked
   uppercase micro-labels for section eyebrows. Hierarchy comes from weight,
   size, and color role — not from borders.
5. **Motion carries meaning.** Fast state changes, slower surface entrances,
   decelerating curves only. No looping decoration, no bounce, no animation
   that does not communicate a state change.

## 2. The world

Clay is an instrument, used for hours at a time under load (a 76-entry log, a
1,000-file repository, a 200-message session). The instrument must disappear
under the work. Concretely, the old shell spent attention on itself: every
panel outlined, every tree row boxed, three border weights competing, hard
shadows making static chrome look pressable.

Quiet Instrument answers that:

- **Quiet plane, loud state.** Rest state is almost invisible (transparent
  rows, hairline dividers). Interactive state is unmistakable (accent-tinted
  fill, 2px focus ring).
- **Density with air.** 32px rows, 30px controls, 11–13px type: dense
  horizontally, never cramped vertically. Air between groups replaces frames.
- **One signal per surface.** A selected row is one thing (accent fill), not
  three (fill + border + shadow + icon color).
- **Data is monospace, prose is not.** Counts, paths, keys, and identifiers
  align on a tabular grid; sentences read in the UI face.
- **The keyboard is the primary pointer.** Every action shows its key, every
  keyboard-driven focus move announces itself once, and no state depends on
  hover alone.

## 3. Layer contract

| Layer | Owns | Must never |
|---|---|---|
| **Content theme** (`@clay/theme-*`) | Every color: surfaces, text, borders, focus, selection, diagnostics, syntax; alpha values for hairline roles | Declare geometry, radii, motion, or typography sizes |
| **UI design system** (`@clay/design-*`, this document) | Radius, border width/style, fills' opacity, shadow layers, inner highlights, outlines, motion duration/curve, transform presets | Declare colors, palettes, fonts, concrete sizes, or raw CSS |
| **User typography** (`theme.setTypography`) | Font families and the variant scale ratios | Be overridden by packages or design systems |
| **Host (Clay shell + React)** | Layout geometry (grids, splits, row/control heights, panel widths, padding rhythm), flex/overflow, landmarks, ARIA, focus management, breakpoints, hit targets | Branch on a specific design-system package id, or hardcode a redesign |
| **Editor (`StyleRegistry`)** | Document colors, syntax colors, gutter, caret, selection, indent guides | Consume SDUI design-system recipe variables |

Projection: themes resolve semantic roles to `--clay-*` custom properties; the
design system resolves recipes to `--clay-ds-*` custom properties; host CSS
Modules consume both with deterministic fallbacks. Resolution happens once per
activation — never in paint, input, or layout hot paths.

## 4. Design-system values (package profile)

Declared in the design-system package's `values` block; recipes carry the same
numbers within their typed domain bounds. No schema extension is required (see
§16).

| Value | Type | Number | Used for |
|---|---|---|---|
| `radius.xs` | radius | 5 | Chips, kbd chips, badges, small selectable cells |
| `radius.control` | radius | 8 | Buttons, inputs, rows, tabs, popover items, tree rows |
| `radius.panel` | radius | 12 | Panels, popovers, dropdowns, composers, inset wells |
| `radius.surface` | radius | 16 | Window, sheets, command palette, modals |
| `radius.pill` | radius | 9999 | Tab items, segmented controls, chips (pill variants), toasts, progress tracks, status dots |
| `border.hairline` | border-width | 1 | Every structural boundary (zoning) |
| `border.emphasis` | border-width | 2 | State indicators only, rendered as inset shadows (§9), never as a box border |
| `motion.fast` | motion-duration | 150 | Hover, active, focus, border/fill/transform transitions |
| `motion.enter` | motion-duration | 240 | Popover/sheet/overlay entrance |
| `motion.flash` | motion-duration | 620 | Keyboard-initiated focus move (one-shot pulse) |
| `opacity.disabled` | opacity | 0.5 | Disabled controls (existing core token is 0.55; the profile rounds to 0.5) |
| `opacity.veil` | opacity | 0.55 | Panel/plane fill over the canvas |
| `opacity.soft` | opacity | 0.15 | Accent-tinted fill for selected/active surfaces |
| `blur.scrim` | backdrop-blur | 3 | Overlay scrim only |

Design-system `values` are declarative documentation in the current schema
(recipes carry literals); they exist so the language is readable in one place
and so the migration has a single source for its numbers.

## 5. Geometry and rhythm

**Radii ladder** — 5 / 8 / 12 / 16 / pill. Never 0px. Never mixed within one
component family. Never applied to a full-bleed region edge (the sidebar's
outer edge stays square and flush; its rows are rounded).

**Host-owned geometry** (design systems cannot set these; migration adjusts
host CSS + typed dimension tokens):

| Element | Default | Compact |
|---|---|---|
| Window title/tab bar | 40px | 40px |
| Status bar | 28px | 28px |
| List/tree row | 32px | 26px |
| Control height (button, field) | 30px | 28px |
| Small control (icon button) | 28px | 26px |
| Chip height / kbd height | 22 / 18px | 20 / 16px |
| Sidebar width | 244px (224px ≤1240px) | — (the workspace sidebar is the SDUI region the server sizes by token: `dimension.sidebar.default` / `.compact`) |
| Right rail / inspector width | 340px (312px ≤1240px) | — |
| Sheet width | 700px (880px wide variant) | — |
| Command palette width | the composer box's own width (it is anchored to that box, §12 — the centered overlay token `dimension.overlay.centered.width` does not apply to it) | — |
| Hit target minimum | 24px (WCAG 2.2) | — |

**Spacing** — 4-point grid plus the deliberate 6/10/14 steps already in the
token catalog. Use `spacing.xxs` (4) inside dense clusters, `spacing.xs` (8)
for control padding, `spacing.sm` (12) for panel/section padding, `spacing.md`
(16) for content padding, `spacing.lg` (24) between regions, `spacing.xl` (32)
for page-level separation. Panel interiors use 10–14px; sheets 18–20px.
Density (`density.default` → `spacing_scale()`) scales this rhythm only — never
panel dimensions or document typography.

**Reading measure** — the editor column is a real column: gutter + 92ch
measure, centered; the agent transcript is 72ch; empty-state prose is ≤48ch.
Text is never stretched edge to edge in a wide window.

## 6. Materials

| Surface | Fill | Opacity | Border | Radius | Shadow |
|---|---|---|---|---|---|
| Window / canvas | `surface.main` | 1.0 | 1px `border.hairline` (window edge only) | 16 | overlay shadow (window only) |
| Chrome strips (title bar, agent lane, status bar) | inherits canvas | — | 1px `border.hairline` on the inner edge | — | none |
| Sidebar / rail | `surface.main` | 1.0 | 1px `border.hairline` right/left edge | — | none |
| Panel / plane ("veil") | `surface.panel` | 0.55 | 1px `border.hairline` | 12 | none |
| Inset well (field, composer, stat block) | `surface.control` | 1.0 | 1px `border.hairline` | 12 | none |
| Row / tab (rest) | transparent | — | none | 8 | none |
| Popover, dropdown, menu | `surface.overlay` | 1.0 | 1px `border.hairline` | 12 | pop shadow |
| Sheet, palette, modal | `surface.overlay` | 1.0 | 1px `border.hairline` | 16 | overlay shadow |
| Bottom-anchored sheet (the composer's palette) | `surface.overlay` | 1.0 | 1px `border.hairline` | 16 | overlay shadow (rises from its own edge, §7) |
| Scrim | `surface.scrim` | 0.5 (`opacity.scrim`) | none | — | none, `backdropBlur: 3` |
| Toast | `surface.panel` | 0.88 | 1px `border.hairline` | pill | pop shadow, `backdropBlur: 8` |

**Shadow recipes** (structured layers, no literals — ink role at bounded
opacity):

- `overlay` = `[{0, 24, blur 60, spread -16, text.primary @0.42}, {0, 2, blur 10, spread -4, text.primary @0.22}]`
  (the approved artifact draws `-20`; the recipe schema bounds spread at ±16, so the
  package uses the nearest legal step and the host keeps the artifact's softer edge)
- `pop` = `[{0, 14, blur 34, spread -14, text.primary @0.34}, {0, 1, blur 3, spread -1, text.primary @0.16}]`
- static surfaces and rows: **no shadow at all**

**Blur** is permitted only on the overlay scrim (3px) and toast (8px). The
editor canvas, gutter, scroll track, panels, rows, and lists are always
`backdropBlur: 0` — a hard performance invariant (`backdropBlur == 0.0` on
editor/scroll paths is conformance-tested).

The agent lane (§12) is chrome, not a panel: it inherits the canvas, draws one
hairline on its inner edge, and stays **above the veil** while the composer's
menus are open — the veil covers content, never the control the menu answers to.

**No** gradients on chrome, no inner-highlight rims, no textures, and no film
grain: the proposal artifact's fixed film-grain layer is deliberately **not**
carried into the product (it costs a full-window composited layer, softens
small text, and no typed recipe property expresses it).

## 7. Motion

| Situation | Duration | Curve | Transform |
|---|---|---|---|
| Hover / focus / fill / border change | 150 (`motion.fast`) | `ease-out` | none |
| Press on a button | 150 | `ease-out` | `press-shift-down` (translateY 1px) — buttons only |
| Popover / dropdown entrance | 240 (`motion.enter`) | `spring-snappy` | from `translateY(-6px) scale(0.985)` |
| Sheet / palette / modal entrance | 240 | `spring-snappy` | from `translateY(-12px) scale(0.99)` |
| Bottom-anchored sheet entrance (the composer's palette) | 240 | `spring-snappy` | from `translateY(10px) scale(0.99)` — rises from its own edge |
| Keyboard focus move | 620 (`motion.flash`) | `ease-out` | one-shot accent halo pulse |
| Running work indicator | 1.1s loop | `ease-in-out` | opacity pulse on the running-work marker: the window mark's dot (the tab in view) and the working bars (the state strip while a turn is in flight, and the transcript's live turn). The one looping animation §14 allows |

Rules: no hover lift anywhere (hover changes fill, not position); no motion on
scroll, typing, resize, or data updates; `prefers-reduced-motion: reduce`
collapses every duration to instant and removes transforms; the design-system
motion enums are limited to the validated set (`linear`, `ease-out`,
`spring-snappy`, `spring-smooth`) — Quiet Instrument uses `ease-out` and
`spring-snappy` only, and never `linear` for a state transition.

**Running work is motion in exactly two places, both of them the run itself:**
the window mark's dot (whose run it is — the tab in view) and the working bars
(what the agent is doing). No surface adds a third: a tab's own marker steps to
the accent colour, the lane's foot states the environment only, and the status
bar states facts. The bars are three 4×11px accent bars at the text's cap
height, pill radius, pulsing 1.1s `ease-in-out` with 160ms offsets — the mark's
cadence, drawn as bars so "working" is motion in the words rather than another
dot.

## 8. Typography and roles

Sizes (before user hierarchy scaling): 20 (window/sheet hero), 15 (titles),
13 (UI base), 12 (detail / secondary), 11 (status, meta), 10 (micro-label).

| Use | Clay role | Notes |
|---|---|---|
| Window, sheet, modal title | `typography.title` | 600 weight |
| Panel / section title | `typography.section` | 600 weight, 12–13px |
| Body copy, row prose, empty-state text | `typography.body` | 13px, 1.5 line-height |
| Row names, tree names, paths, counts, keys, timestamps, ids, status items, field input text | `typography.detail`/`status` **with monospace font role** | tabular figures, `font-variant-numeric: tabular-nums` |
| Section eyebrow, kbd chips, stat headers | `typography.caption` | host applies 10px + 0.14em tracking + uppercase |
| Empty-state title | `typography.body` | 600 weight |

Rules: never uppercase prose (micro-labels only); never letter-space body
text; never use mono for a sentence; never let a design system or package pick
a concrete family or size (user-owned via `theme.setTypography`); numbers in
tables, meters, and status always use tabular figures.

## 9. State language

State is a fill change plus, where needed, one shape signal. Exactly one.

| State | Controls (button/field) | Rows / list items | Tabs |
|---|---|---|---|
| rest | transparent (default/ghost) or 1px hairline border; field = `surface.control` | transparent, no fill | transparent, `text.muted` |
| hover | `surface.hover` fill; border → `border.subtle` | `surface.hover` fill | `surface.hover` fill, `text.primary` |
| active (pressed) | `surface.active` fill + `press-shift-down` | `surface.active` fill | `surface.active` fill |
| focus (`:focus-visible`) | 2px `focus.ring` outline, offset 2px; fields and composers additionally get the accent halo | 2px `focus.ring` outline, offset 2px | 2px `focus.ring` outline, offset 2px |
| selected / current | primary button = `accent.primary` fill with `surface.main` text | `accent.primary` @0.15 fill, `text.primary` | `accent.primary` @0.15 fill + `accent.primary` text, pill radius |
| disabled | `text.disabled` + `opacity.disabled`, no shadow, no transform, action gated | same | same |
| invalid | `diagnostic.error` border + `diagnostic.error` message text | — | — |

**Focus ring formula** (every interactive element, no exceptions):
`outlineColor: focus.ring`, `outlineWidth: 2`, `outlineOffset: 2`,
`outlineStyle: solid`.

**Accent halo** (a shell that owns the boundary — text fields, the textarea
composer, and the composer box the command palette answers to): a zero-blur,
3px-spread shadow layer in
`accent.primary` at 15%
(`shadow: [{x:0, y:0, blur:0, spread:3, colorRole: accent.primary, opacity:0.15}]`)
so the field reads as active without a second border weight.

**Running work** is motion, not a fill, and it reads once per window (§7): the
mark's dot pulses while the tab in view is working, the working bars say what
the agent is doing, and a tab's own marker carries its state in the accent
colour alone (no pulse of its own, tooltip naming the state) — so work in a tab
that is not in view stays visible without a second blinking dot.

**Edge indicators** — a selected row is a fill and nothing else: no leading
accent bar and no underline (§14.13, retired 2026-09-11). Where a component
genuinely needs a 2px mark that is not a fill (the split handle's drag state), it
is a fill change on the handle, never a one-sided border (the schema has no
per-edge border) and never an extra element.

**Keyboard focus announcement** — when a keyboard action moves focus to a new
region, the receiving element pulses once (620ms accent halo fade). Pointer
focus does not pulse.

## 10. Color-role map

Themes own the values; the design system only picks roles. Roles Quiet
Instrument relies on (existing catalog names):

| Role | Use |
|---|---|
| `surface.main` | Canvas, window, chrome strips, sidebar/rail, editor background |
| `surface.panel` | Plane/veil fills, toasts, popovers fallback |
| `surface.control` | Inset wells, fields, composers, selected control fills |
| `surface.overlay` | Popovers, dropdowns, sheets, palette, modals |
| `surface.scrim` | Overlay dim |
| `surface.hover`, `surface.active`, `surface.disabled` | Control/row states |
| `surface.list`, `surface.selected` | Row fallbacks where a role-based fill is preferred over an opacity-tinted accent |
| `surface.scrollbar`, `surface.scrollbar.track` | Scroll thumb/track |
| `text.primary`, `text.muted`, `text.disabled`, `text.icon` | Text and glyph roles |
| `text.kbd`, `surface.kbd`, `border.kbd` | kbd chips and shortcut hints |
| `text.badge`, `surface.badge` | Badges/tags |
| `text.tooltip`, `surface.tooltip` | Tooltips |
| `accent.primary`, `accent.muted` | Selection, active navigation, running work, primary button fill |
| `focus.ring`, `border.focus` | Focus ring and focused border |
| `border.hairline` | Zone separators and resting control outlines: the theme's own border grey at 34% alpha (§10.1) |
| `border.subtle` | The structural boundary — the same border grey at full strength, and the step the contrast gate measures |
| `border.strong` | Rare explicit separators: the theme's ink at full strength |
| `diagnostic.error`/`warning`/`success`/`info` | Status, validation, error text and tints (tints via `backgroundOpacity`) |

Theme-side requirements for this language (value changes only, no schema
change):

1. **The border ladder is built from two theme colours, and the sources differ
   per step** (the shipped themes declare all three roles as typed
   `designTokens`; measured values per theme in
   [`docs/reference/ui-design-systems.md` §7](docs/reference/ui-design-systems.md)):

   | Role | Source | Shipped alpha | What it is for |
   |---|---|---|---|
   | `border.hairline` | the theme's **border grey** | 34% | Zone separators, resting control outlines. Decorative: it identifies no component and is exempt from the 3:1 floor (§13.1), but must stay quieter than `border.subtle`. |
   | `border.subtle` | the same **border grey** | 100% | The structural boundary — panel edges, hover borders. The approved screens already draw this exact grey for the window frame, so it is the one step that can carry 3:1. |
   | `border.strong` | the theme's **ink** | 100% | Rare explicit separators, where the boundary is the content. |

   Earlier revisions of this section asked for ink at 34/62/100%. That is not
   what the approved language renders: at 34% ink a hairline composites ≈0.2
   darker than the approved divider, and at 62% ink `border.subtle` composites
   to ≈2:1 on all four shipped palettes — below the 3:1 boundary floor the same
   artifact requires (§13.1). The border grey is therefore the source for the
   first two steps, and the measured difference between the two definitions is
   recorded in `design-artifacts/approved/quiet-instrument-migration/theme-values.md`
   (finding 1) and §7 of the reference page. A theme that has not moved to typed
   `designTokens` can still express the accent and the ladder: `textStyles`
   accepts `accent`, `borderHairline`, `borderSubtle` and `borderStrong`, and the
   projection prefers them over the `caret` / `scrollbar` stand-ins it used
   before they existed (plan 118 task E7).
2. **Depth direction is a per-theme check, not an assumption.** `surface.main`
   resolves from the theme's `shellBg` and `surface.panel` from `panelBg` for
   every theme, and whether the canvas or the panel is the lighter plane is a
   property of the palette: three shipped themes put the lighter colour on the
   canvas (Modus Operandi `#ffffff`/`#f2f2f2`, Gruvbox Material Light
   `#fbf1c7`/`#f2e5bc`, Gruvbox Material Dark `#282828`/`#1d2021`) and Modus
   Vivendi is black canvas over a `#1e1e1e` panel. What the language requires
   is a real luminance step plus the hairline between them — an inverted pair
   is a defect to fix in `textStyles` (the Gruvbox Material Dark pair was
   swapped in plan 118 task 13 for exactly that reason), and each theme's
   direction is verified when its values are authored rather than inferred
   from `appearance`.
3. `surface.selected` and hover/active steps must be opaque and pass 3:1
   against their own text; accent must pass 3:1 against `surface.main` when
   used as a fill behind `surface.main` text. Both are measured **composited**:
   a fill with alpha is blended over its surface first and the text over that,
   which is the layer order the user sees (§13.1).
4. Diagnostic tints are expressed as role + `backgroundOpacity` (0.15), never
   as extra opaque colors.

## 11. Component recipes

Recipe keys are `component.variant.slot.state`. `@clay/design-instrument` ships
165 keys: the recorded reference set of 142 minus the 12 removed `chat.default.*` keys
(the chat surface was removed in plan 118), plus the 35 keys the target information
architecture and the drawn-but-unbacked surfaces need (§16). The values below are the ones that
define the language; the package carries the full set. Which keys the host actually
consumes is a separate, gated question: the not-yet-adopted remainder is recorded in
`frontend/src/test/fixtures/design-system-adoption-backlog.json`, and the mirror
case — a host slot with no shipped recipe — is marked `†` in
[`docs/development/ui-design-system-recipe-matrix.md`](docs/development/ui-design-system-recipe-matrix.md).

### Controls

- `button.default.root.rest` — transparent fill, `borderColor: border.hairline`,
  `borderWidth: 1`, `borderRadius: 8`, `textColor: text.primary`,
  `padding: spacing.xs`, `gap: spacing.xs`, `transitionDuration: 150`,
  `transitionTiming: ease-out`, `transformPreset: none`, no shadow.
- `button.default.root.hover` — `backgroundColor: surface.hover`,
  `borderColor: border.subtle`.
- `button.default.root.active` — `backgroundColor: surface.active`,
  `transformPreset: press-shift-down`.
- `button.default.root.focus` — 2px `focus.ring` outline, offset 2.
- `button.default.root.disabled` — `textColor: text.disabled`,
  `opacity: 0.5`, no shadow, `transformPreset: none`.
- `button.primary.root.rest` — `backgroundColor: accent.primary`,
  `textColor: surface.main`, `borderColor: transparent`, radius 8.
- `button.primary.root.hover` — same fill at `backgroundOpacity: 0.92`
  (the artifact's mix-toward-text darkening is not expressible; this is the
  documented approximation), `textColor: surface.main`.
- `button.danger.root.hover` — `diagnostic.error` at 0.16 opacity fill,
  `textColor: diagnostic.error`, transparent border.
- `button.muted.root.rest` — transparent fill, `textColor: text.muted`, no
  border (a text action, not a box).
- `button.icon` — same states; square 28px host box, `text.icon` glyph,
  `radius.control`.
- `textInput.field` / `textInput.input` — `surface.control` fill, 1px
  `border.hairline`, radius 12 (composer/textarea) or 8 (single line),
  `textColor: text.primary`, placeholder `text.muted`.
- `textInput.input.focus` — `borderColor: accent.primary` + accent halo
  (spread-3 shadow at 0.15).
- `textInput.input.invalid` — `borderColor: diagnostic.error`,
  `textColor: text.primary`; error text `diagnostic.error`
  (`typography.caption`).
- `textInput.description` — `text.muted`; `textInput.placeholderColor` =
  `text.muted`.

### Navigation, lists, tabs

- `list.root` — transparent fill, no border, no shadow.
- `list.row.rest` — transparent, radius 8.
- `list.row.hover` — `surface.hover`; `list.row.active` — `surface.active`.
- `list.row.selected` — `backgroundColor: accent.primary`,
  `backgroundOpacity: 0.15`, `textColor: text.primary`. The fill is the whole
  signal: no leading bar, no underline, no added edge, no icon recolor (§14.13).
- `list.rowTitle` — `text.primary` (mono role at the host for data-like rows).
- `list.rowDetail` — `text.muted`, `typography.detail`.
- `tab.default.item.rest` — transparent, `textColor: text.muted`, pill radius.
- `tab.default.item.hover` — `surface.hover` fill, `textColor: text.primary`.
- `tab.default.item.selected` — `accent.primary` @0.15 fill,
  `textColor: accent.primary`, pill radius.
- `tabBar.default.root.rest` — inherit canvas, 1px `border.hairline` bottom.
- `seg.default.root.rest` / `.item.rest` — the segmented control, and therefore
  the tab's Workspace | Agent view switcher: a pill shell (1px `border.hairline`,
  transparent fill) with transparent items at `text.muted`; `.item.hover` —
  `surface.hover`, `text.primary`; `.item.selected` — `accent.primary` @0.15 fill,
  `textColor: accent.primary`; `.item.disabled` — `text.disabled`, no fill. One
  family: the switcher is a segmented control, so it declares no second name. In
  tab chrome it sits at the titlebar's trailing edge, never inside a view (§12).
- `agentPicker.trigger.rest` — transparent, radius 8, `textColor: inherit` (it is
  the view's title); `.trigger.hover` — `surface.hover`; `.trigger.expanded` —
  `accent.primary` @0.15 fill, `textColor: accent.primary`; the chevron is
  `text.muted`. Its menu body is `dropdown.default.list`.
- `recentRow.root.rest` — the launcher's workspace/agent rows: `list.row`
  geometry plus a trailing meta column — name `text.primary`, path `text.muted`
  (mono), meta `text.muted` (mono, tabular). `.selected` — `accent.primary` @0.15
  fill, name at `accent.primary`, no edge. The leading dot is `statusDot` below.
- `sessionRow.root.rest` — the Files tab's session-history row: a role mark
  (`text.muted`; `diagnostic.warning` written, `diagnostic.success` created,
  `diagnostic.error` deleted), the basename at `text.primary`, the directory at
  `text.muted` (mono, ellipsised before the basename is), and the role word at
  `text.muted`. Hover is `surface.hover`; a session file is opened, not selected.
- `statusDot.{success,warning,error,muted,busy}.root.rest` — a 6px dot: the
  diagnostic role for the first three, `text.muted` for muted, `accent.primary`
  for busy (the 1.1s pulse is host motion, §7).
- `toast.default.root.rest` — a veil at 0.88, pill radius, 1px `border.hairline`,
  pop shadow, `backdropBlur: 8` (the only blur besides the scrim).
- `empty.default.root.rest` — transparent, centred, no border: an empty state
  carries its structure in `label.*` typography and stays within the 48ch measure.
- `keyHint.default.{root,row,keys}.rest` — the shortcut vocabulary: a hint line, a
  shortcut-map row (label left, keys right), and the row of `kbd` chips it holds.
- `swatch.default.root.{rest,hover,focus,selected}` — the theme swatch in
  settings: rim, radius 5 and state; the three stripes are host composition.
- `statRow.default.root.{rest,hover}` + `bar.rest` — the inspector's stat rows
  (label, mono value, meter bar in `accent.primary`, pill radius).
- `badge.*` is the chip: the badge and the chip were the same idea drawn twice, so
  the chip declares no family of its own — `badge.{accent,success,warning,error,
  muted}` covers its variants.
- `collapse.header` — transparent, hover `surface.hover`; `collapse.body` —
  transparent, 1px hairline top; `collapse.chevron` — `text.muted`, rotates
  150ms, `aria-hidden`.

### Panels, overlays, dialogs

- `panel.default.root.rest` — `surface.panel` @0.55, 1px
  `border.hairline`, radius 12, no shadow; `panel.header` — 1px hairline
  bottom, `padding: spacing.sm`; `panel.title` — `typography.section`,
  `text.primary`; `panel.body` — transparent, `padding: spacing.sm`.
- `overlay.default.root.rest` — `surface.overlay`, radius 12, 1px hairline,
  pop shadow.
- `dropdown.trigger` — like a default button; `dropdown.popover` —
  `surface.overlay`, radius 12, 1px hairline, pop shadow, `padding: spacing.xxs`;
  `dropdown.item.selected` — accent @0.15 fill, `textColor: text.primary`.
- `modal.scrim` — `surface.scrim` @ `opacity.scrim` + `backdropBlur: 3`;
  `modal.dialog` — `surface.overlay`, radius 16, 1px hairline, overlay
  shadow; entrance 240ms `spring-snappy` from `translateY(-12px) scale(0.99)`.
  The scrim has **two callers**: a modal dialog, and the composer's menus in
  the agent lane (§12) — the `/` palette and the `@` mentions — which draw the
  same treatment over the **working area only**, the lane itself staying above
  it, because the lane's field is the menu's own input. No second scrim key:
  one recipe, one tier (3px), two callers.
- `commandCentre.root` — radius 16, overlay shadow, opaque overlay fill; rows
  are `list` rows and a selected row is the accent @0.15 fill. In the shell it
  is the composer's palette: it spans the composer box's own width, sits 6px
  above that box, caps at `min(52vh, 420px)` and scrolls internally, and it
  owns **no input** — the field below it is the query, so the sheet draws no
  well and no ring of its own (§9 halo belongs to the composer box) and its
  head echoes the query; the boundary in focus is the box's, not the sheet's
  (§14.4); `commandCentre.empty` — the bare centred empty
  state; `commandCentre.status` — the foot's key-hint row. A menu session
  (`contextMenu`/`menuBar`) takes `popover.root` instead — radius 12, pop
  shadow, content-sized.
- `tooltip.default.root.rest` — `surface.overlay`, radius 8, 1px hairline,
  pop shadow, `padding: spacing.tooltip`, `typography.body`,
  `transitionDuration: 150` (`motion.fast`; the 100 this section first stated
  was never what any package declared).
- `badge` — pill or radius-5, 1px `border.hairline`, transparent or veil
  fill, `typography.caption`, mono; semantic variants use the role text with
  a role-tinted 34% border; accent variant = accent @0.15 fill + accent text.
- `kbd` — veil fill, 1px `border.hairline`, radius 5, 18px high,
  `typography.caption` mono; on a primary button: `surface.main` @0.18 fill,
  no border.

### Chrome

- `shell.default.root.rest` — `surface.main`, no border.
- `shell.default.header.rest` — 1px `border.hairline` bottom, radius 0 on the
  window's own edges, transparent fill.
- `shell.default.footer.rest` — the agent lane's own boundary (§12):
  transparent fill over the canvas, 1px `border.hairline` top,
  `padding: spacing.xxs`, mono `text.muted` for the session-environment foot.
  The composer box inside the lane is `textInput`'s composer slot
  (`surface.control`, radius 12, one hairline, focus = accent border + halo);
  the agent-control toolbar row inside that box (agent-type picker, model,
  effort, context meter) is the `agentPicker`/`dropdown` family at muted
  weight, and the box stays the only boundary between them.
- `statusBar.default.root.rest` — inherit canvas, 1px `border.hairline` top,
  mono 11px, `textColor: text.muted`; `statusBar.item` — `text.muted`, hover
  `surface.hover`, radius 8, `transitionDuration: 150`.
- `paneSplitTree.handle` — 1px `border.hairline`, hover/active
  `accent.primary` at 0.4/0.7 opacity, focus ring; no fill at rest.
- `scroll.scrollbarTrack` — transparent; `scroll.scrollbarThumb` — rest
  `surface.scrollbar` at `opacity.disabled`, hover/active at 1.0, pill
  radius, 8–9px width.
- `divider.default.root.rest` — `border.hairline`, 1px, no margin.
- `focusRing.default.ring` — `focus.ring`, 2px, offset 2, radius 5.
- `iconSlot` — 16px, `text.icon`, `opacity.disabled` when inactive.
- `fileBrowser.default.item.selected` — accent @0.15 fill, no leading bar
  (§14.13); counts right-aligned, mono, `text.muted`.
- The `chatPanel.*` family and the chat surface were removed in plan 118 task 9;
  the agent surface takes its place as `@clay/coding-agent` (`DESIGN.md` §12,
  approved artifact `agent-landing.html`).

## 12. Composition rules for shells

**The tab is the unit.** One tab is one workspace (a folder) plus one agent, and
it holds exactly two views: the **workspace view** (the folder — editor today,
other viewers as they exist) and the **agent view** (the agent attached to the
tab). The switcher between them is tab chrome: it sits at the titlebar's trailing
edge, right-aligned beside the tab strip, with the palette trigger
immediately to its left — the icon trigger of the window actions opens the
composer's `/` palette and puts the field in query mode (`/` typed, caret after
it), never a window-centred modal — never inside a view it switches between. A
tab with no agent yet shows the lane's agent picker; a tab with no folder shows
the workspace view's prompt. Tab titles are the folder's basename with the full path in the tooltip and
a hairline marker when an agent is attached. `⌘T` opens a new tab on the launcher,
which is also what a fresh window shows.

**The title bar is the app's own composition** — the `Clay` mark with its accent
dot, the tab strip hugging it, a spacer, then the window actions at the trailing
edge (the palette trigger, then the tab's view switcher). The shell draws **no
window buttons**: the OS window frame owns minimise/maximise/close. Where the
earlier approved migration artifact (`approved/quiet-instrument-migration/shell.html`)
drew a `palette · ? · Inspector` action set and its own window buttons, the app
as implemented governs, and `approved/agent-lane-palette/` is the approved
drawing of the shell's chrome.

**The lane is the shell's one bottom section.** Every tab draws one persistent
agent lane at the bottom of the working area — the working area's own chrome
strip (§6/§11), so it sits below both views and their rails (the workspace
sidebar and the agent inspector end at its top edge, and the lane's hairline is
the boundary between them) — and it is the same lane in both views: it mounts
once per tab beside the two view slots, so switching views never remounts it and
never moves the draft, the run or the pickers. Top to
bottom: the approval strip when a tool is suspended (warning-toned text, Allow
and Deny, `alertdialog`, focus moved in and handed back), the **composer box**
(the field's row, then the tab's own agent controls as one toolbar row inside
that box — agent-type picker, model, reasoning effort, context meter — then the
hint row), and the session-environment foot (workspace · branch · extensions ·
MCP summary). `Ctrl+X Ctrl+P` toggles it — a state, not a deletion, persisted
per tab like the rails. The field is always typeable: with no provider
configured the composer stays live and the foot says why nothing will send
(`no provider configured · Settings → Providers`); with no agent the box offers
the agent picker (`Attach an agent`) and the field is inert. **There is no Send
button** — `↵` sends, the hint row says so, and Stop takes that slot exactly
while a run is live. The lane's foot never carries a run indicator.

**The command surface.** The Control Centre is consolidated into the composer's
`/` palette: one command surface, opened by typing `/` in the lane's field or by
`controlCenter.open` (`Ctrl+X Ctrl+O`), over the server's command catalogue, with
each row's chord as chips and scope chips (`All · Session · Shell · Files`) that
filter it — both read from the server's own item fields (a row states no scope it
was not tagged with, and the sheet renders no control it cannot fill). The palette
owns no input and no output zone of its own — it is the field's menu, spanning
the composer box's own width 6px above it — and it rises from its own edge (§7)
over the shared veil: the `/` palette and the `@` mentions menu are the same
gesture on the same field, so they share one veil, and the lane stays above it.
`controlCenter.openPath` keeps its own chord and stays a palette row.

A tab's record — what the shell owns and what persistence round-trips — is
**the folder it has picked** (empty while nothing has been picked: the launcher's
state), **the agent identity** (the agent's registry key plus its config root;
inert display data that grants nothing), and **the view that is up**. The session
the tab rides always has a real root on the server (the configured root or the
cwd fallback), so "the folder" is the *picked* folder: a fresh window, a `⌘T` tab,
and an agent-only tab are uncommitted and land on the launcher, while a restored
tab comes back to its picked folder and view.

**The switcher's states.** An uncommitted tab shows both items inert, each with
its reason in the tooltip ("Pick a workspace first" / "Pick an agent first"). Once
either half exists both views are reachable — the missing half renders its own
prompt — and the item for the view that is up carries the only selection signal
(accent tint + accent text; the underline and the leading accent bar are retired,
§14.13). Switching is tab chrome: `Ctrl+1` workspace view, `Ctrl+2` agent view,
and the inactive view stays mounted (state kept, nothing re-fetched, no package
re-activated). Positional tab activation therefore sits on `Ctrl+Alt+<N>`, and the
tab's own marker carries its state in the accent colour — the pulse lives in the
window mark (§7).

**Launcher surface.** The landing surface, and the content of every empty tab that
has picked nothing yet (an uncommitted tab): two
panes side by side — recent workspaces (name, path, branch) and available agents
(label, config root, skills) — each with a filter field, over one action row whose
primary button names exactly what it will open ("Open clay", "Open Coding Agent",
"Open clay + Coding Agent") and stays inert until something is picked. Keyboard:
`⇥` between panes, `↑↓` move, `⏎` pick, `⌘⏎` open both, `esc` clear, `⌘O` open
folder. Picking a workspace opens the workspace view; picking an agent opens the
agent view; both open one tab holding both. **One of each per launch** — each pane
is single-select, and the launcher sets a tab's first state, not its only state:
the agent is changed afterwards from the lane's agent picker, the folder from the
workspace view, each without discarding the other half of the tab. Several
workspaces means several tabs. Recents are real (a deleted folder is pruned, not
shown) and a first-run state with nothing to list is a designed state, not an
empty box.

**Workspace surface.** Left sidebar (tree, filter field, count footer — the
approved head: a search well with the `/` chip, and a foot with the live match
count and the move/open hints) ·
center editor column (gutter + 92ch measure, centered) · optional right rail
(document facts + outline, `⌘I`). The relative-path field is not permanently
visible: it is an on-demand strip (`⌘O`). All document actions (save, undo,
redo, close) live in one document bar above the canvas; the path is text, not
a control. The status bar carries workspace, connection, counts, and the
keyboard hint row. The workspace view keeps the lane at its foot: the composer
belongs to the tab, not to the agent view, so a document can prompt the tab's
agent without leaving the page.

**Agent surface.** The transcript and the state strip; no header — the agent's
own controls live in the lane's composer box below (one place, both views). The
`agentPicker` family's trigger is the lane's agent control: it opens the shared
dropdown popover over the configured agent types (the same server enumeration
the launcher lists, one directory scan of the data root's `agents/`), marks the
current one, and its note states where more come from (`~/.clay/agents/`).
Picking one switches the tab's
agent in place — the workspace half is untouched, the tab keeps its session, and
the config the next run reads (system prompt, skill roots, tool caps, MCP servers,
model/effort defaults) is that agent's own. A transcript that spans a switch keeps
every turn labelled with the agent that produced it, with one note marking the
boundary; a turn is never attributed to an agent that did not write it. ·
transcript as the primary column (72ch) · state strip
at the foot of the column, directly above the lane (status dot + message + note,
mono 11px — the working bars replace the tone dot while a turn is in flight) ·
inspector on the right with tabs (Files, Memory,
Context, Session Info, Settings). The composer is one boundary: the shell owns
the focus ring, the field inside it draws none (the shell's focus state is the accent border plus
the 3px halo, §9 — one boundary, deliberately), the box spans the lane's width
(never a centred measure narrower than its own controls), and Stop sits inside
the shell's trailing edge while a run is live — no Send button, and nothing in
that slot at rest. The **Files
tab is the session's file history** — the files this session has read, written,
created or deleted, newest first, one row per path (role mark, basename, muted
directory, role word), with `⏎` switching the tab to its workspace view at that
file. The rows are the transcript's own tool records, so the list survives a
resume and clears with the session — it is not a file browser: the workspace tree
is the workspace view's job, and the inspector never grows a second one. Its head
carries the count and a filter field (`F` focuses it) and its foot says what the
list is; the empty state says the same and offers the two keystrokes that move
the session forward instead (`@`, `⌘1`). Capability inventories (skills,
MCP servers) are reference data and belong in the inspector — not in a permanent
center panel. Counters and meters show real values only: no fabricated usage, no
invented tool output, no placeholder numbers.

**Settings surfaces.** Two surfaces, one composition. The Settings panel is the
tab's **fixed right slot** (340px, 312px ≤ 1240px): a heading row with the close
affordance and its chord, then one column of hairline-separated rows — each row a
micro-label, its control, and a caption note where a note is needed — under an
eyebrow-labelled group per subject, and an actions row whose note states what the
commit will do with the buttons at the trailing edge, primary last. Sections ship
closed except the one that is the reason the panel was opened. The panel is a
slot, so the boundary is the slot's divider hairline, there is no radius and no
elevation; the panel's own recipe border and radius are spent only when it
becomes a drawer over the content (below 760px), where it also goes opaque. The
Agent Settings page (the agent view's Settings tab) is the same composition
narrowed: a caption that says where the numbers come from, then a hairline-
separated row per delivered file (mono path, size as data, provenance as a
badge), with a designed empty state. Neither surface frames a box inside a box:
the only bordered control is the input well.

**Universal.** One command surface (§ above) is the only global launcher; every
action it lists also shows its key. `?` opens the keyboard map. Layout state
(sidebar, rail, lane, inspector, density, theme, tab) persists per surface — the
rail, the lane and the agent inspector per **tab**, with the tab's folder and
view. Transient surfaces
never scroll the canvas, and overlays never nest more than one level
(popover inside sheet inside scrim is the limit).

## 13. Accessibility invariants

1. Text ≥ 4.5:1 against its own surface. The affected 3:1 floor — structural
   boundaries, focus ring, selected/state fills and icons — applies to the
   roles that identify or bound a component: `border.subtle`, `border.strong`,
   `border.focus`, `focus.ring`, `accent.primary` and `accent.muted` against
   the surface they sit on, the `surface.hover`/`surface.active`/
   `surface.selected` fills against their own text, and `text.disabled` against
   the surface its text sits on. `border.hairline` is exempt by
   §10.1 — a 34% zone divider identifies nothing, so the floor is scoped by
   role rather than by alpha — but it keeps a **1.2:1 visibility floor** (a
   divider nobody can see is not quiet, it is absent) and must stay
   monotonically quieter than `border.subtle` on the same surface. Every pair
   is measured **composited** — alpha blended over the backdrop first — since
   a 34%-alpha hairline renders at 1.39–1.61:1 across the shipped palettes
   while its raw bytes describe opaque ink at up to 21:1 on the light canvases.
   Enforced at theme validation (`validate_active_theme_contrast`
   behind `enforce_contrast`), not by review: a theme that misses a floor is
   refused activation, the diagnostic names the specifier, pair, ratio and
   threshold, and the previously active theme stays installed. State fills are
   gated in the layer order they render in (text composited *over* the fill,
   fill over the surface), so a translucent selection tint passes where the
   pre-migration uniform 40%-alpha selection colour scored 1.03–1.43:1.
2. Focus is always visible: 2px ring, offset 2px, on every interactive
   element, in every state, in every theme. Where a control lives inside a
   shell (`input-shell`, `field`), the shell draws the focus state and the inner
   control draws none — one ring per surface, per §14.4, never a ring inside a
   ring.
3. Hit targets ≥ 24px (icons get a padded box), including icon-only buttons.
4. No state is conveyed by color alone: focus adds a ring, selection adds a
   fill step plus a text/role change (in forced colors the system `Highlight`
   carries it, §13.5), errors add text.
5. `forced-colors: active` yields to system colors for canvas, text,
   highlight, borders, and focus; structure must remain readable without
   fills.
6. `prefers-reduced-motion: reduce` → instant state changes, no transforms,
   no pulse.
7. `prefers-reduced-transparency: reduce` → all fills opaque, no blur.
8. Mono data uses tabular figures; text remains user-scalable through the
   typography hierarchy and never clips its own UI geometry.
9. ARIA, focus management, and keyboard flows stay host-owned; styling never
   removes an affordance a screen reader or keyboard user needs.
10. A menu whose input lives outside itself keeps that input usable: the
   composer's `/` palette veils the content it covers and leaves the lane above
   the veil (the field below it is the menu's own query, and its focus ring is
   the composer box's). The palette is a `listbox` the field names through
   `aria-controls`/`aria-activedescendant`; the veil is never a modal barrier
   over the input it serves, and `esc` returns focus to the field it came from.

## 14. Retired patterns (do not reintroduce)

1. Hard offset shadows (`3px 3px 0`) and any zero-blur non-inset shadow.
2. `borderRadius: 0` on controls, rows, popovers, or overlays.
3. Borders wider than 1px on controls, rows, or panels. State marks are a fill
   step and a text/role change — never an added 2px edge (see §14.13).
4. Two competing border weights on one surface (frame + inner divider + state
   border). One boundary per surface.
5. Backdrop blur on canvas, gutter, scroll, panels, or rows.
6. Accent used as decoration (panel edges, headings, non-state icons).
7. Gradients, inner-highlight rims, film grain, and textured backgrounds.
8. Uppercase or letter-spaced body text; mono for prose sentences.
9. Hover lift/scale on static controls; any looping animation except the
   running-work pulse.
10. Fill-only hover on touch/keyboard surfaces where the target is invisible
    without pointer position.
11. Fabricated data in mock or real surfaces (token meters, tool output,
    counts) — show real state or an explicit "not available" state.
12. A second, competing visual language inside one surface (boxed tree rows
    inside a plane panel, outlined tabs inside an outlined panel).
13. A leading accent bar on a selected row (2px inset, tree/outline/package
    lists). The approved screens drew it beside the accent fill, so a selected
    row read as two signals; the fill alone is the signal (§11
    `list.row.selected`). An editor's current-line marker is not a row and is
    unaffected — it is document chrome, not selection.
14. A window-centred command sheet as the standing command surface. The
    Control Centre's catalogue belongs to the composer that queries it (§12):
    the palette spans that composer box, rises from its own edge, and is the
    only command list — the composer's local `/` completion list stops being a
    second surface and *is* the palette. A centred palette is a modal, and a
    modal competes with the field it answers to. (The `modal.dialog` recipe
    keeps its callers — the app's own dialogs — and its scrim is now shared with
    the composer's menus; what is retired is the centred *command* sheet.)
15. A second blinking dot for the run. The window mark's dot and the working
    bars are the run's motion (§7/§9); a tab's own marker, the lane's foot and
    the status bar state it without motion, so a busy window never shows two
    blinking dots in one titlebar row.

## 15. Review checklist

Use this in every UI task, plan acceptance, and visual review:

- [ ] One surface per zone; boundaries are hairlines + whitespace only.
- [ ] Radii come from the 5/8/12/16/pill ladder; no 0px corners.
- [ ] Shadows exist only on transient surfaces, with the overlay/pop recipes.
- [ ] Accent appears only for focus, selection, active nav, running work.
- [ ] Every interactive state implemented (rest/hover/active/focus/selected/
      disabled/invalid) and rendered from tokens, not literals.
- [ ] Focus ring present and visible in every theme (light **and** dark).
- [ ] Data (counts, paths, keys, timestamps) is monospace with tabular
      figures; prose is not.
- [ ] Text contrast ≥4.5:1 and UI contrast ≥3:1 measured, not eyeballed.
- [ ] Editor/transcript respects the reading measure; no edge-to-edge prose.
- [ ] `backdropBlur == 0` everywhere except scrim/toast.
- [ ] Reduced motion, reduced transparency, and forced colors verified.
- [ ] Keyboard path complete: every action reachable, every shortcut shown.
- [ ] Palette chips (scope and per-row chords) are read from the server's item
      fields — a row states no scope it was not tagged with, renders no chord it
      does not have, and the filter it selects is the session's own.
- [ ] Run state appears once (the window mark's dot, the working bars); nothing
      else pulses, and the lane's foot states the environment only.
- [ ] No retired pattern from §14 present.
- [ ] Catalog/docs updated if a primitive, token, or component changed.

## 16. Implementation profile (shipped state)

Machine-readable contract this language is implemented by — **no schema extension
required**, and no schema extension was needed:

*Shipped state below is plan 118's, as implemented, with plan 124's lane shipped
by its task 6: the agent view is now the transcript, the state strip (tone dot at
rest, the three working bars while a turn is in flight) and the inspector, and
the composer, the tab's agent controls and the session-environment foot live in
the shell's lane (`frontend/src/shell/AgentLane.tsx`), which mounts once per tab
below the two view slots. Sentences below that describe the agent header, the
view's own composer, or the window-centred command sheet are plan 118's record —
plan 124's §6/§7/§9/§11/§12/§13/§14 above supersede them (the lane landed in its
task 6, the palette session in task 7, the lane-anchored sheet in task 8; the
palette's scope/chord item fields are the one piece still scheduled), and the key
set stays exactly as counted below — the lane and the palette add no recipe key.*

- **Package:** `@clay/design-instrument`, `displayName: "Quiet Instrument"`,
  `schemaVersion: 1`, no `extends`, inert data only, zero permissions, no
  `entry` (shipped by plan 118 task 8 as `packages/design-instrument/`).
- **Values:** §4 (radius/border/motion/opacity/blur names as declared).
- **Recipes:** the key set shared with the existing reference packages **minus
  the 12 `chat.default.*` keys** (the chat surface is removed), **plus the
  families the target IA and the drawn-but-unbacked surfaces need** (`seg` — which
  is also the tab's view switcher, `agentPicker` (the lane's agent trigger),
  `recentRow`, `sessionRow`,
  `toast`, `empty`, `statusDot`, `keyHint`, `swatch`, `statRow`). **165 keys,
  fixed by plan 118 task 8 and asserted in task 12**; no key is declared without a
  consumer, and the two name collisions were resolved by unification rather than
  addition (the chip is `badge.*`, the view switcher is `seg.*`). Values per §11.
- **Expressiveness proof** (why no schema work is needed):
  - veil/accent/disabled fills → `backgroundOpacity` on a single color role;
  - 2px edge indicators → `shadow` layers with `inset` and zero blur;
  - focus halo → zero-blur `shadow` layer with positive `spread`;
  - soft elevation → two `shadow` layers with negative `spread` and low
    opacity (≥ -16 spread, ≤ 64 blur is enough);
  - radii 5/8/12/16/9999 within `[0, 32] ∪ 9999`; border width 1–2 within
    `[0, 8]`; motion 150/240/620 within `[0, 1000]`.
- **Host-side work (not design-system data):** control/row heights, panel and
  rail widths, padding rhythm, reading measures, the focus-flash behaviour,
  the on-demand path strip, and the agent inspector grouping.
- **Theme-side work:** the four alpha/step requirements in §10, applied
  through existing typed `designTokens` overrides per theme package.
- **Compatibility:** the former `@clay/design-neobrutal` and `@clay/design-glass`
  packages are **removed** (plan 118 task 9) — the catalog is not additive here,
  and no shipped surface may reference them; `@clay/design-instrument` is the only
  shipped design system and is the implicit default (its recipes are what
  `@clay/core` resolves for every host-consumed key).
- **Core fallback is the same language, not a second one:** `@clay/core` keeps the
  host-consumed subset of the shipped recipes — the component kinds and shell
  surfaces the frame paints before a snapshot exists (buttons and their states,
  text inputs, modal, panel, the kind/surface roots) — at the same ladder,
  hairline weights, elevation stacks and motion tiers. The static `--clay-ds-*`
  block in `frontend/src/styles/tokens.css` is that subset's projection, so a build
  with no design system installed paints the migrated language and activating the
  package cannot move geometry, material or motion (both equalities are asserted in
  `tests/package_ui_conformance.rs`).
- **Settings surface (shipped, plan 118's settings task):** `@clay/settings`
  declares the panel as `slot: right, kind: fixed`, and the host mounts the React
  panel in that same slot track (`packages/package-workspace.module.css`
  `.rightFixed` widens the track to the slot's own width so nothing is clipped).
  The composition is §12: heading (`settingsPanel.heading`, hairline bottom, `esc`
  + Close) / one column of eyebrow-labelled groups (`collapse.*`, chevron leading)
  of hairline-separated rows / actions (`settingsPanel.actions`, note then buttons
  at the trailing edge, primary last). Rows carry a micro-label
  (`label.caption` role, uppercase, tracked) and the field's own label is clipped
  (`ClayTextField labelHidden`) so no label paints twice; values that are data
  (sizes, ratios) use the mono role (`ClayTextField role="monospace"`). The panel
  is a fixed slot: veil fill from `settingsPanel.panel` (`surface.panel` @0.55),
  **no radius, no elevation**, boundary from the slot's divider; below 760px it
  becomes a drawer and then spends the recipe's radius, hairline and opaque fill.
  The Agent Settings tab (`agent-settings/`) is the same language narrowed: one
  760px-capped column, a caption naming the sources (`.agents/skills/*/SKILL.md`,
  `SYSTEM.md`), and `list.default.row`s per delivered file (mono path, mono size,
  `badge.*` provenance) with the `empty.*` recipe for the first-run state. The
  SDUI panel in `@clay/settings` (`package.json` + `dist/load.js`, mirrored)
  renders the same rows, sections and action labels for hosts that paint the
  package's tree instead of the React panel. The design-system dropdown and the
  server-enumerated `ui_choices.design_systems` list carry `@clay/core` plus
  `@clay/design-instrument` (the static fallback list was updated in task 9,
  alongside the removal of the two retired entries). Consumed recipes:
  `settingsPanel.*`, `collapse.*`, `dropdown.*`, `textInput.*`, `list.*`,
  `empty.*`, `badge.*`, `kbd.*`, `divider.*`. Evidence:
  `design-artifacts/screenshots/quiet-instrument-settings/` (`report.json` —
  12 panel captures = 4 themes × 1500/1024/760, plus delivered/empty agent-file
  scenes: slot 340/312/480-drawer flush against the pane's right edge, no shadow,
  two eyebrow sections, ten rows of which eight carry hairlines, 4/4 numeric
  values in mono, three dropdowns from the snapshot, 0px horizontal overflow).
- **Theme-side roles (shipped, plan 118 task 13):** all four shipped content
  themes (`@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi`,
  `@clay/theme-gruvbox-material-dark`, `@clay/theme-gruvbox-material-light`)
  declare the same thirteen typed `designTokens` — `border.hairline`,
  `border.subtle`, `border.strong`, `surface.scrim`, `accent.primary`,
  `accent.muted`, `focus.ring`, `border.focus`, `text.muted`, `text.disabled`,
  `surface.hover`, `surface.active`, `surface.selected` — built from their own
  palette per §10.1/§10.3. Values, rationale and measured ratios:
  `design-artifacts/approved/quiet-instrument-migration/theme-values.{json,md}`
  (binding) with the shipped numbers in
  [`docs/reference/ui-design-systems.md` §7](docs/reference/ui-design-systems.md).
  Gruvbox Material Dark's `shellBg`/`panelBg` were swapped in the same task so
  `surface.main` is the canvas (§10.2).
- **Contrast gate (plan 118 task 14):** `src/shell/theme.rs` gates text at 4.5:1,
  boundaries/focus/state fills at 3.0:1, the decorative hairline at a 1.2:1
  visibility floor, and `border.hairline < border.subtle` on the same surface —
  every pair measured composited (`editor::theme::composited_contrast_ratio`).
  A failing theme is refused activation with the pair and ratio named and the
  previous theme left installed; the core catalog satisfies the same policy so
  the pre-bootstrap palette is not a bypass.
- **Consumer-side gates (plan 118 task 21):** `frontend/src/test/design-system-consumption.test.ts`
  holds the recipe/variable drift as a hard assertion against
  `frontend/src/test/fixtures/design-system-adoption-backlog.json` (it may shrink,
  never grow), and `design-system-conformance.test.tsx` asserts the language
  invariants over both shipped systems plus reduced-motion, reduced-transparency
  and DOM-continuity behaviour.
- **Shell and Workspace composition (shipped, plan 118's shell/Workspace task):**
  the window is three rows — a 40px titlebar (mono, tracked `Clay` mark with the
  accent dot, pill tabs with their new-tab button, and the right-aligned action
  group: the palette icon trigger then the tab's view switcher), the
  working area, and a 28px status bar
  (mono, tabular figures, `text.muted`, hairline top, workspace · document ·
  connection and one hint row whose items run the commands they name). The
  Workspace view is one grid: the flat file-browser region (the SDUI tree emits a
  stack, not a panel — the host's left slot paints the region), the full-bleed
  document column (no centred measure, no line-number gutter), and the optional
  right rail (`⌘I`; document
  facts + outline, or a right-hand drawer below 1000px). The document bar holds
  identity, state and every document action in one row; the relative-path field
  is an on-demand strip, not a permanent control. Consumed recipes:
  `shell.*`, `statusBar.*`/`statusItem.*`, `paneSplitTree.*`, `editor.*`,
  `divider.*`, `list.*`, `panel.transient.*` (the drawer), `badge.*`, `kbd.*`.
- **Launcher composition (shipped, plan 118's launcher task):** the empty tab and
  a fresh window render `@clay/launcher`'s `empty-tab` pane content, which the
  host paints as its compiled launcher panel for the bundled package's trusted
  provenance (a third-party `empty-tab` contribution renders through generic
  SDUI; with none installed the core fallback stays Open File / Open Folder, no
  product name in core). One centered column (1320px cap, 92ch head): the title
  and one-paragraph framing, then two panes side by side (one column ≤ 1080px) —
  **Workspaces** (count chip, filter well, the server's recent roots as
  `list.default.row` rows: basename at `text.primary` mono, real path at
  `text.muted`, single-select per pane) and **Agents** (the configured
  directories under the Clay data root's `agents/`, labelled, with the resolved
  config root and skill count) — closed by one action row: the key-hint row, a
  mono summary of the picks, and a primary button whose label names exactly what
  it will open, inert until something is picked. Rows are server data: a folder
  that no longer exists is pruned on read (one bounded `launcher.recents_pruned`
  diagnostic), an agent type exists only as a directory, and removing a recent
  names an index in the server's own list — never a path. Consumed recipes:
  `panel.default.root.*` (the two panes), `list.default.row.*` (rows),
  `textInput.default.input.*` (filter wells), `badge.default.root.*` (count
  chips), `divider.default.root.*` (pane head/foot and action row hairlines),
  `button.*`, `kbd.*`. Deliberate deviations from the approved `start.html`:
  rows carry name + real path (no branch/MCP meta — that needs the cached git
  service, still a follow-up), the filter wells are the shipped input wells
  rather than the prototype's quiet variant, and the panes take the panel
  recipe's veil fill (the prototype drew them unfilled).
- **Coding Agent composition (shipped, plan 118's Coding Agent task):** the agent
  view of a tab is a two-column grid — the agent column and the inspector
  (340px, 312px ≤ 1240px, a right-hand drawer with the pop shadow below 1000px),
  both hairline-separated from the canvas. The column is header (agent name as the
  view title, the model control or the active `provider/model`, the context meter
  drawn from the stat-row bar, the effort control) / transcript / state strip
  (status dot + message + note, mono 11px) / composer / environment foot
  (workspace · branch · extensions · MCP summary). The transcript is a 72ch
  measure of hairline-separated turns — role label, right-aligned note, body —
  where machine output (`tool`, `skill`, `usage`) is one inset well with a
  uniform three-line clamp, so a turn's height never depends on how much it
  printed; turns round only in their fill states. The composer draws **exactly
  one** boundary (the field well and its focus ring — `textInput`'s composer
  slot, §14.4), text left-aligned, Send/Stop/Close inside the field's trailing
  edge — *superseded in plan 124: the composer is the lane's, draws no Send
  button, and holds Stop alone while a run is live.* Skills, MCP servers and context counts are the inspector's Context tab
  (reference data, §12), not transcript cards. Consumed recipes: `shell.footer.*`,
  `list.*` (turns and rows), `textInput.*` (the composer and the open strip),
  `menu.*` (the `/` and `@` completion menus), `empty.*`, `statusDot.*`,
  `keyHint.*`, `statRow.*` (meter, context counts), `sessionRow.*`, `badge.*`,
  `kbd.*`, `divider.*`, `panel.transient.*` (the drawer). Evidence:
  `design-artifacts/screenshots/quiet-instrument-agent/` (`report.json`, 12
  captures = 4 themes × 1500/1024/960) — 72ch measure resolving to 535.4px,
  4 turns with 3 hairline separators, a 57px clamped tool well, **one** composer
  boundary, inspector 340/312/400-in-drawer, 0px horizontal overflow, and no
  blur/filter/animation on the transcript or composer in any run.
  Its **Files tab** is the session's file history (plan 118 task 36): one row per
  path the session touched, newest first, on `sessionRow.root` (role mark, mono
  basename, ellipsised mono directory, role word), the mark toned by the theme's
  diagnostic roles (`M` warning, `A` success, `D` error, `R` muted), the head's
  count as a `badge` and its filter as the shipped input well (`F` focuses it,
  one ring per surface), and a foot that says the list is session history. Rows
  are buttons — `⏎` opens that path in the tab's workspace view, which is how the
  dual-view model pays off. Evidence:
  `design-artifacts/screenshots/quiet-instrument-agent-files/` (`report.json`):
  the three seeded records (created/modified/read) in order with their family and
  paint (r8, spacing.xs padding, transparent fill), role-toned marks, the filter
  narrowing 3 → 1 with the count following, 0px horizontal overflow.
- **Overlay family (shipped, plan 118's command-centre/overlay task):** the
  command palette is one opaque elevated sheet (`commandCentre.root`: r16,
  hairline, overlay shadow) with a head (search glyph, the server's prompt as a
  micro-label, the query input), the scrolling results as `list` rows, an empty
  state, and a foot of `keyHint` rows plus the live result count
  (`commandCentre.status`). One ring per surface (§14.4): the sheet's boundary
  turns accent and takes the halo on focus, and the input draws none. The same
  component renders a menu session (`contextMenu`/`menuBar`) as a narrower
  `popover.root` surface (r12, pop shadow) under its own prompt. The modal is a
  sheet: head (title + close) and actions foot separated by the divider
  hairline, cancel leading and the primary action trailing, with a scrolling
  body; `flush` hands the surface and the head to its content (the palette). The
  dropdown popover (r12, pop shadow), the `/`/`@` completion menu
  (`menu.*`, including the pressed state), and the tooltip (r8, pop shadow)
  complete the family. Shadows exist on these transient surfaces only — a host
  test asserts the closed owner allowlist — and every veil resolves to an opaque
  surface (scrim → canvas) with no blur left running under
  `prefers-reduced-transparency`. Evidence:
  `design-artifacts/screenshots/quiet-instrument-overlays/` (`report.json`, 32
  captures = palette × 4 themes × 1500/1024/960, empty/path/menu scenes, modal,
  tooltip), zero failures.
- **Still open at the host (not design-system data):** the tab view switcher and
  the agent picker are approved
  (`design-artifacts/approved/quiet-instrument-migration/`) but not built
  (tasks 33/35), and the launcher's rows carry no branch/MCP meta yet (they
  consume `list.default.row.*`, so `recentRow.*` — the declared name + trailing
  meta column — ships with that follow-up) — the
  catalog marks the families without a host consumer as
  declared-and-unconsumed rather than pretending they paint: `toast` (no shipped
  surface emits one — a host with no emitter would be dead code), `swatch` (the
  Settings panel cannot render per-theme palettes the server does not send) and
  the launcher's richer `recentRow` (its rows paint `list.default.row` until the
  branch/MCP meta column ships). The workspace sidebar's head is shipped with its
  filter (plan 118 task E1): the server marks the listing filterable, the host
  renders the approved tools row and count foot and filters the delivered,
  bounded rows locally — keystroke-local, never a round-trip. Two deviations from
  the prototype are deliberate: the app's sidebar lists **one directory at a
  time** (with its `..` row), so the prototype's "a matching file keeps its
  ancestors visible" rule has no tree to apply to, and the field is the shipped
  single-line input well (r8, §11) rather than the prototype's field-painted
  strip. The workspace rail's outline follows the editor's
  viewport while the reader scrolls, and an explicit jump holds its entry until
  the next manual scroll (the approved behaviour in `workspace.html`; plan 118
  task E2). The input's declared error slot
  (`textInput.default.error.rest`) is consumed: a field in the error state renders
  its own message below the well, announced through the input's description, and
  the Settings panel's typography fields say which value is wrong rather than
  toning the whole group (plan 118 task E6). `seg`, `agentPicker`, `sessionRow`, `keyHint`,
  `empty`, `statusDot` and `statRow` are consumed and are part of the host-owned
  baseline too (the core fallback map mirrors their consumed rest keys, so
  `@clay/core` and the shipped package cannot drift apart); `frontend/src/test/fixtures/design-system-adoption-backlog.json`
  is the exact remaining set and is gate-enforced.
- **Naming:** `@clay/design-instrument` is the shipped specifier (display name
  "Quiet Instrument"); the earlier `@clay/design-quiet-instrument` alternative was
  not taken.
