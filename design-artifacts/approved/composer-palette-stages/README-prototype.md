# Palette stage flows and the halo — plan 125 prototype

**This prototype has no authority.** It is a review device: what is approved is
frozen under `design-artifacts/approved/composer-palette-stages/`, and
implementation cites only that approved set plus `DESIGN.md`. Nothing here is a
reference implementation, a source of truth for markup, or a claim about the
code; where this page and the approved artifact disagree, the approved artifact
wins.

Plan 125 retires the window-centered Control Centre sheet and moves every flow
that used it — the agent picker's provider / model / agent / session stages, the
provider setup flow (API key, base URL, OAuth device code) — into the composer's
`/` palette, the sheet plan 124 anchored to the lane's field. This page draws
that single sheet in every state the plan's acceptance criteria name, plus a
side-by-side comparison of the candidate halo against today's drop shadow.

## How to open it

Open `palette-stages.html` directly (`file://`, no build step, no network); the
whole kit is local and copied byte-identically from
`design-artifacts/prototypes/agent-lane-palette/`:

| file | provenance |
| --- | --- |
| `theme.css`, `ds-quiet.css`, `components.css`, `pages.css`, `ds.js` | byte-identical copies of the plan-124 prototype kit (verified by md5) |
| `palette-stages.css` | plan 124's `lane.css` verbatim (the lane and palette geometry that shipped) plus a marked plan-125 block |
| `palette-stages.html` | the plan-124 page's shell chrome (title bar, sidebar, status bar, inspector, page styles) taken verbatim, with the new scenes, the stage-aware sheet and the halo board |
| `../tools/capture-palette-stages.mjs` | the capture/assertion tool that produced `design-artifacts/screenshots/composer-palette-stages/` |

The review bar carries: the scene switcher, the theme menu (four shipped
palettes), the palette-shadow toggle (`drop shadow` / `halo`), motion
(full/reduced) and the frame width (1500 / 1024). A scene, theme or width can be
linked directly:

```
palette-stages.html?scene=stage-secret&theme=gruvbox-material-dark&width=wide
```

`?theme=` is never written back to storage, so reviewing a theme does not change
anyone's default. `?scene=` names are the scene ids below.

## Scenes

Sixteen scenes; every one is reachable by keyboard in the page.

| scene | what it shows |
| --- | --- |
| `lane` | the lane as it ships (plan 124): no sheet, the field carries a draft |
| `lane-railed` | both rails active (agent view): the lane is the view pane's width and stops at the inspector's edge — the sheet is the lane's, and the veil covers both rails |
| `lane-full` | both rails hidden: the lane takes the working area's whole width again, and the sheet widens with it |
| `catalogue` | catalogue with a query: `/` sigil, scope chips, per-row chord chips |
| `catalogue-empty` | empty query: the whole catalogue, `type a command`, sigil stays accent |
| `catalogue-no-match` | no match: the sheet keeps its height, bare empty state pointing at `Esc` |
| `catalogue-scopes` | the `Files` scope chip selected — the filter is the session's |
| `catalogue-chords` | the `Shell` scope: one `kbd` chip per stroke, as the status bar spells them |
| `stage-provider` | stage 1 (providers), opened from a palette row; the filter is typed without a sigil |
| `stage-auth` | stage 2 (sign-in method): same sheet, same rows, the prompt names the question |
| `stage-secret` | stage 3: the **shielded** API key — value in the sheet's own field, never the composer draft |
| `stage-url` | the base-URL stage: same field composition, visible value |
| `stage-oauth` | the device-code stage: user code and verification URI as selectable text, the rows do the actions |
| `stage-model` | a long list: the sheet caps at `min(52vh, 420px)` and the list scrolls inside it |
| `stage-session` | sessions: `↵` resumes, `Alt+↵` deletes — the selected row carries the secondary hint |
| `stage-back` | depth 3, with the back affordance: `Esc` / `Alt+←` walk back one stage at a time |
| `mentions` | the `@` mentions menu (unchanged dropdown) wearing the same halo |
| `halo` | the comparison board: palette and mentions, today's drop shadow beside the candidate halo |

## Coverage and how it is verified

`design-artifacts/tools/capture-palette-stages.mjs` runs every scene at four
shipped themes (`modus-operandi`, `modus-vivendi`, `gruvbox-material-dark`,
`gruvbox-material-light`) and two frame widths (1500, 1024) — **144 matrix
runs** — and asserts, per run:

- the theme and the scene really applied; no console error; no non-`file://`
  request; no horizontal overflow;
- the lane is the **view pane's** width (edges within 1px) and never overlaps a
  visible rail; the view pane starts where the files rail ends and ends where
  the agent rail starts; the rail visibility the scene names is the rail
  visibility rendered; and with no rail showing, the lane spans the working
  area's whole width;
- the sheet is the composer box's own width (`delta 0`), is the lane's inner
  width (inset by the composer's own padding, so it can never cross into a
  rail), sits **6px** above the box, capped at `min(52vh, 420px)`, scrolling
  internally where the rows overflow (`stage-model`);
- the stage prompt is the stage's question; scope chips exist for the catalogue
  and never for a stage; the foot says what `↵` does and offers the back
  affordance; the session row and foot carry the `Alt+↵ delete` hint;
- the secret stage is a `type="password"` field whose value is **bullets only**,
  and the composer draft is empty — no row or label repeats the value;
- `backdrop-filter` is `none` on the sheet and the mentions menu (it belongs to
  the veil alone: `blur(3px)`, open exactly while a menu is);
- the halo board shows both values, and the live sheet flips between them when
  the review toggle is used.

Then it walks the keyboard path the design claims (15 steps): `/` opens the
catalogue, typing filters it, `↑↓` moves, `Esc` closes and the next keystroke
reopens with the draft intact, `@` opens mentions, `Ctrl+X Ctrl+O` opens the
catalogue, `Esc` / `Alt+←` walk a depth-3 stage trail back to the composer,
`Alt+↵` deletes the selected session, and a scope chip activates from the
keyboard.

```bash
CHROME_PATH=... node design-artifacts/tools/capture-palette-stages.mjs
node design-artifacts/tools/capture-palette-stages.mjs --no-shots --quiet
node design-artifacts/tools/capture-palette-stages.mjs "--filter=^stage-secret__.*__wide$"
```

Exit status is non-zero if any assertion fails. `--filter=` narrows the matrix
to matching `scene__theme__width` labels for a re-capture; `report.json` in the
evidence directory holds the full matrix and the keyboard walk.

## The stage flows, as drawn

One sheet, one row language, one veil. What changes between states is only the
*vocabulary* (`data-mode`), the prompt line, and which component owns the typed
value:

| mode | opened by | query lives in | `↵` | back |
| --- | --- | --- | --- | --- |
| `catalogue` | `/` sigil, `Ctrl+X Ctrl+O`, the title-bar trigger | the composer draft, after the sigil | run the row (a picker row opens its stage) | `Esc` → close (draft survives) |
| `list` (provider, auth, model, session) | a palette row, or a typed built-in (`/model`, `/resume`) | the composer draft, **no sigil** | choose / resume | `Esc`, `Alt+←` → previous stage, or close at the first |
| `secret`, `field` (base URL) | the auth stage | the sheet's own field (`textInput` composition; masked for the secret) | store / save | same |
| `oauth` | the auth stage | — (the stage is text + two action rows) | run the row | same |

The prompt line is the kept `.prompt` micro-label from the shipped host CSS —
the rule written for exactly this case ("a session whose prompt is the question
shows it as a micro-label"). The stage's field is the shipped `textInput`
composition: one well, one description line, no second focus ring. The `↑↓
navigate`, `↵ <verb>`, `Esc back`, `Alt+← back`, `Alt+↵ delete` hints are more
`kbd` chips in the sheet's existing foot.

The sheet is a child of the field's wrapper (`.field-slot` in this page, the
shipped `ClayTextField.fieldSlot` in the app), so it anchors to the field's
*border box*: that is where the exact width match and the full 6px gap come from
(plan 124 defect D5).

## The lane's width, and the sheet's width

The lane is the view pane's own chrome strip (plan-125 review, 2026-09-18): it
spans the **middle pane only** and never sits over a rail. Both rails run the
full height of the working area — the lane's hairline stops at their inner edges
— and hiding a rail gives its width back to the lane, so with both rails gone
the lane is the working area's whole width again.

The drawing encodes that instead of simulating it: `.side`, `.pane.main` and
`.inspector` are three explicit grid columns, a hidden rail's track collapses to
`0px`, and the lane lives *inside* `.pane.main` — so it takes the pane's width by
construction and widens on its own when a track collapses. That is also why the
view pane's column is explicit rather than auto-placed: a `display: none` rail
leaves the grid, and the pane would slide into the collapsed track.

The sheet's width is the lane's inner width — the composer box it answers to,
inset by the composer's own 18px padding inside the lane — and it tracks the lane
1:1 as rails toggle, so it can never cross into a rail. Two scenes and a
per-run assertion pin it: `lane-railed` (both rails; the sheet stops at the
inspector's edge, at both widths) and `lane-full` (both rails hidden; lane and
sheet span the working area).

**Open sub-decision:** what "the lane's width" means for the sheet. This page
draws the sheet the width of the composer box (the lane's inner width: 18px
inside the lane's edges on each side), because plan 124 anchored the sheet to
the field and that anchor is what produces the exact width match and the 6px
gap. Taking it as the lane's *outer* edges instead would widen the sheet 18px on
each side and move the anchor from the field to the lane — a change to the
approved plan-124 geometry, not a tweak.

## The halo, as proposed

One value, replacing the two shipped elevation stacks' values — no new recipe
key, no new token, no new slot:

| recipe key | today | candidate |
| --- | --- | --- |
| `commandCentre.default.root.rest.shadow` | `0 24px 60px -16px` @ 42% + `0 2px 10px -4px` @ 22%, `text.primary` | `0 0 14px -2px` @ 14% + `0 0 3px 0` @ 8%, `text.primary` |
| `menu.default.root.rest.shadow` | `0 14px 34px -14px` @ 34% + `0 1px 3px -1px` @ 16%, `text.primary` | the same halo value |

Both layers sit at offset `0`, so the light wraps the box instead of falling
from it: 14px soft + 3px tight, the theme's `text.primary` at 14% and 8%. The
border is untouched. The values are inside the design system's existing `shadow`
bound (≤3 layers, blur ≤64, spread −16…16, a theme colour role plus opacity) and
blur stays `> 0`, so this is not a banned hard offset shadow. The prototype's
`--halo` in `palette-stages.css` is the candidate value; `--shadow-overlay` and
`--shadow-pop` stay beside it so the board can show today's drawing unchanged.

`popover.default.root.rest` (trigger-anchored dropdown popovers, which have no
veil behind them) still ships `menu`'s old values. Whether the three surfaces
should share one elevation is **open** (see below).

## Decisions this page puts in front of the reviewer

1. **The halo values** (`0 0 14px -2px` @ 14% + `0 0 3px 0` @ 8%, `text.primary`)
   and one value for both the sheet and the mentions menu.
2. **`popover` divergence**: keep the dropdown popover's drop shadow, or give
   the three surfaces the same halo.
3. **The back affordance is keyboard-only** (`Esc`, `Alt+←`, stated in the
   foot). A pointer back target is deliberately absent — the sheet is modeless
   and the composer keeps focus — but a reviewer may want a back row or a head
   button instead. This page does not invent either.
4. **The stage's field vs the composer as filter**: stages with a value
   (secret, base URL) own a field inside the sheet; list stages keep the
   composer as their filter. This is the split the plan asks for; the reviewer
   should confirm the reflection of it in the head (a stage with an empty filter
   shows the prompt alone, with no echo).
5. **The model list is a plain scrolling list** (no virtualization, no grouping).
   It is capped by the server's 256-item ceiling; if a real inventory ever
   measures badly, virtualizing is a later change, not a design decision here.
6. **Known drawing divergence (carried from plan 124):** this page's veil is the
   kit's canvas tint (`color-mix(--c-bg 66%)` + blur 3), while the shipped veil
   is the `modal.scrim` recipe (`surface.scrim` @ 50% + blur 3). The shipped
   recipe is the implementation source; the prototype was not re-cut for it.
7. **The lane's containment and the sheet's width** (above): the lane is the
   view pane's width, the rails keep their full height, and the sheet is the
   lane's inner width (the composer box). Approve as drawn, or say the sheet
   should be flush with the lane's outer edges instead.

## Security

The secret scene renders **masked characters only** — a literal bullet string,
never a credential, key, token or URL that resolves anywhere. The approved
behavior this page encodes is: the typed characters go to the picker session as
its query (the server masks the echo on the wire), and they are **never** echoed
into the composer's draft, which is persisted layout state. No authority is
added anywhere in this flow: the picker session already owns the credential
step, and no package code or JS API is involved.

## Catalog mapping

Every element is an existing cataloged owner; the page adds no family, slot,
key or token. `data-clay-ds` on the markup names the recipe a reviewer can look
up in `docs/development/ui-design-system-recipe-matrix.md`.

| element | cataloged owner | in this page |
| --- | --- | --- |
| the sheet | `commandCentre` root (whole-shell family, host CSS consumer) | `.lane-palette`, `data-clay-ds="commandCentre.default.root.rest"` |
| rows, selection, detail, meta | `list.row` / `rowTitle` / `rowDetail` | `.pal-item` + `.pal-title` / `.pal-detail` / `.pal-meta` |
| scope chips | `seg.root` / `seg.item` | the head's `All / Session / Shell / Files` |
| chords and hints | `kbd` (+ `keyHint` gaps) | every `kbd` chip, the foot, the row hints |
| empty / no-match | `empty` + `commandCentre.default.empty.rest` | `.pal-empty` |
| the stage prompt | host `.prompt` rule (kept, not redesigned) | `data-clay-ds="label.default.root.rest"` |
| the stage's field | `textInput.field` / `label` / `input` / `description` / `error` | `.stage-field`, `.stage-input`, `.stage-desc` |
| the veil | `modal.scrim` (shipped); kit tint here | `.lane-scrim` |
| the lane, composer, controls | `shell.footer` + `textInput` composer + `agentPicker` / `dropdown` triggers + the meter's `statRow` bar | unchanged from plan 124 |

The halo board and the scene-note strip are review scaffolding, not design.

## Files

```
composer-palette-stages/
  palette-stages.html    the scenes
  palette-stages.css     lane.css (verbatim) + the plan-125 block
  README.md              this file
  theme.css ds-quiet.css components.css pages.css ds.js   the kit, byte-identical
design-artifacts/screenshots/composer-palette-stages/
  report.json            the full matrix + the keyboard walk
  <scene>__<theme>__<width>.png   the review frames
design-artifacts/tools/capture-palette-stages.mjs   the capture/assertion tool
```