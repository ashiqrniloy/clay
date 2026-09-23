# Composer palette stage flows + halo — approved design set (frozen 2026-09-18)

**Status: approved by the user on 2026-09-18.** This directory is the binding
design record for plan 125 (Composer Palette Stage Flows and Centered Sheet
Retirement). `DESIGN.md` is the normative text; this set is the reviewed picture
of it. Implementation copies what is here — it does not re-interpret it, and it
does not edit these files.

Approved work product of `design-artifacts/prototypes/composer-palette-stages/`
(the working copy, which stays live for iteration; losing variants stay there
marked *not approved*). The files below are a byte-identical snapshot **as
reviewed**, with hashes recorded so a later change to the working copy is
provably not a silent change to the approval.

## 1. Approval record

| | |
| --- | --- |
| Approved by | user |
| Date | 2026-09-18 (one review round; set frozen 02:41 local) |
| Scope approved | one bottom-anchored surface for the catalogue **and** every picker stage — provider, sign-in method, shielded secret, base URL, OAuth device code, model, session — with the stage's prompt line, the stage-aware foot (`↑↓`, `↵ <verb>`, `Esc`/`Alt+←` back, `Alt+↵` delete, result count), and the catalogue's `/` sigil vs a stage's sigil-free filter; the replacement of the palette's and the `@` mentions menu's drop shadow by the halo; and the lane's width rule — the lane spans the **middle pane only**, both rails keep their full height, both rails hidden gives the lane the working area's whole width again, and the sheet is the lane's width |
| Approval evidence | "The artifacts are approved…" — full statement quoted in §1.1 below |
| Approved *with* one required amendment | The same statement's lane-containment requirement. The working copy was changed accordingly before this freeze (three explicit grid columns, `0px` tracks for hidden rails, the lane inside the view pane, the sheet still anchored to the field), the two new scenes `lane-railed` / `lane-full` and their per-run assertions were added, and the amendment is **in** this frozen set. Plan 125's visual/accessibility review task confirms it in the running app; the review record reports it as a confirmed amendment, not a deviation |
| Not approved / not covered | the gate's own tooling (`design-artifacts/tools/capture-palette-stages.mjs`) and the captured PNGs, which stay with the working copy; `DESIGN.md` itself, which this approval does not edit (plan 125's amendment task does, and it may disagree — then the specification wins); the retired centered sheet's own artifact (`approved/quiet-instrument-migration/command-centre.html`), which this set supersedes without editing |

### 1.1 The approving statement (verbatim)

> The artifacts are approved. One thing to note here is that the agent lane
> should not go over the left and right side panes. The lane should only be
> spanning the middle pane if both left and right panes are active. If they are
> hidden then it should again take the whole width. And the composer palette
> should match the width of the lane. You need to make this explicit in the
> artifact while confirming the design. Otherwise all good

What that resolves, and what it does not:

- **Resolved:** the lane's containment and the rails' full height. This
  supersedes the plan-124 reading of `DESIGN.md` §12 ("the lane is the working
  area's own chrome strip … the sidebar *and* the rail end at its top edge and it
  spans the full working-area width") — that sentence produced plan-124 review
  defect D1 and is now wrong. §12/§16 are plan 125's amendment to sweep.
- **Drawn as:** the sheet is the width of the composer box — the lane's *inner*
  width, inset 18px inside the lane's edges by the composer's own padding —
  because plan 124 anchored the sheet to the field (defect D5) and that anchor is
  what produces the exact width match and the 6px gap. "Match the width of the
  lane" is read as *the sheet follows the lane 1:1 and never crosses into a
  rail*. Taking it as the lane's outer edges instead would widen the sheet 18px
  each side and move the anchor from the field to the lane — a change to approved
  plan-124 geometry, not a tweak. `README-prototype.md`'s decisions list (item 7)
  records it as the one open sub-decision; nothing in the implementation may pick
  the other reading silently.

## 2. What is frozen here

**8 files** — the single prototype page, its stylesheet, and the shared kit it
renders over, plus `README-prototype.md` (the working copy's `README.md` at
freeze time: the 18 scenes, coverage, catalog mapping, the stage model, the halo
proposal and the seven decisions the reviewer faced). This approval record is
documentation, not a reviewed artifact, and is deliberately not in the table.
Hashes are the drift detector: if a file here differs, the approval no longer
describes it.

| file | size | sha256[:16] |
| --- | --- | --- |
| `README-prototype.md` | 15.7 kB | `4a004c030755ebec` |
| `components.css` | 14.9 kB | `01f30073b77148f6` |
| `ds.js` | 53.0 kB | `31f53b20a194a826` |
| `ds-quiet.css` | 30.8 kB | `c2aee13d84b0944a` |
| `pages.css` | 3.3 kB | `697a63cdee3d8065` |
| `palette-stages.css` | 24.4 kB | `87d574284d0816b2` |
| `palette-stages.html` | 79.6 kB | `454f33360f6806c9` |
| `theme.css` | 10.4 kB | `b1a39839be745cdb` |

Two notes on the kit's ownership:

- The five kit files (`theme.css`, `ds-quiet.css`, `components.css`, `pages.css`,
  `ds.js`) are **byte-identical** to `approved/agent-lane-palette/`'s — same
  hashes, checked pairwise at freeze time. This approval therefore adds **no
  design-language value**: plan 125's ink is `palette-stages.html` (the scenes,
  the mode-aware sheet, the halo board, the rail-state scene inputs) and
  `palette-stages.css` (the stage furniture, the halo values, the rail/lane
  containment block). Everything the sheet wears is a `--clay-ds-*` recipe
  variable or a theme role the kit already defines.
- The kit's own responsive default collapses the inspector below 1240px
  (`ds.js`). A scene names its rail state and the lane's width follows from it,
  so the page re-asserts the scene on `load`; a scene's rail state is the
  review's subject, not the kit's window-size heuristic.

## 3. What the approval binds

1. **One sheet, two vocabularies.** The command catalogue and every picker stage
   render in the same bottom-anchored sheet: same rows, same foot, same veil.
   The catalogue needs the `/` sigil; a stage's filter does not. The window
   centered sheet is retired — no flow of this family may reopen a centered
   position (plan 125's retirement task owns the wire-level part).
2. **The stage prompt is the stage's question**, drawn with the kept `.prompt`
   micro-label from the shipped host CSS (plan 124's amendment wrote that rule;
   this set uses it, it does not redesign it). A stage with an empty filter or a
   value-owning field shows no query echo.
3. **Security — the shielded secret stage.** The credential is typed into a
   `type="password"` field *inside the sheet*, never into the composer draft (the
   draft is persisted layout state). The wire echo is the server's bullet mask
   (`merge_secret_query`); the picker session already owns credential capture, so
   this adds no authority anywhere. The frozen page renders bullets only — no
   real credential, key, token or resolving URL appears in it.

   *Recorded after the freeze (2026-09-18; wording only — no reviewed artifact
   changed):* the implementation also makes the composer box behind the sheet
   non-typable while that stage is up, so a paste aimed at the lane cannot land
   the credential in the draft. The stage's own echo is not drawn at all (item 2
   already rules it out for a value-owning field): the shield is the only place
   the value is visible, and the server's mask is never read back into the page.

   *Same date, same rule — the two stages whose field the page draws but the
   specification does not own:* `DESIGN.md` §12 gives an owned input to the
   shielded credential alone, so the **base-URL stage keeps the composer field
   as its filter** (no sigil, one intent per keystroke, exactly like the other
   list stages) instead of the in-sheet field the page draws; and the **OAuth
   device code is drawn from the session's own rows** (code and verification URL
   are row details the server already sends) rather than as the page's separate
   `.stage-code` block. Both render the stage prompt, the stage verb and the
   walk-back keys exactly as frozen.
4. **Mode-aware foot and rows:** `↑↓ navigate`, `↵ run|choose|resume|store|save`,
   `Esc close` / `Esc back`, `Alt+← back`, `Alt+↵ delete` on session rows, and
   the result count as `<output aria-live="polite">` at the foot's start.
   `Esc`/`Alt+←` walk the stage trail back one stage at a time (derived from the
   session kind — no trail field on the wire).
5. **One refraction.** The sheet and the mentions menu have `backdrop-filter:
   none`; the veil alone carries the blur and is up exactly while a menu is. The
   veil is the `modal.scrim` recipe — the recipe wins over this page's drawn
   canvas tint (carried plan-124 divergence, recorded in `README-prototype.md`'s
   decisions list, item 6).
6. **The halo** — the values below replace the *values* of two existing keys;
   no key, token, family or slot is added. Bounded by the design system's shadow
   grammar (≤3 layers, spread ≥ −16, blur ≤ 64, blur > 0, a theme colour role +
   opacity); the hairline border stays; `popover.default.root.rest` (no veil
   behind it) keeps today's drop shadow.

   ```json
   "commandCentre.default.root.rest": { "shadow": [
     { "x": 0, "y": 0, "blur": 14, "spread": -2, "colorRole": "text.primary", "opacity": 0.14 },
     { "x": 0, "y": 0, "blur": 3,  "spread": 0,  "colorRole": "text.primary", "opacity": 0.08 }
   ] }
   "menu.default.root.rest": { "shadow": "<the same value>" }
   ```
7. **The lane's containment and the sheet's width** (the amendment):
   `.side` · the view pane · the agent rail are three explicit grid columns; a
   hidden rail's track collapses to `0`; the lane lives inside the view pane and
   takes its width — the whole working area's width once both rails are gone;
   both rails run the full height of the working area and the lane never sits
   over either; the sheet is the lane's inner width (the composer box it answers
   to, `delta 0`, 6px above it). The plan-124 veil that spans the working area is
   kept, and with the rails at full height it must now cover their lower part
   too, with the lane above it (z 41 over z 40) — hidden = not veiled, visible =
   veiled.

   *Recorded after the freeze (2026-09-18; wording only — no reviewed artifact
   changed):* the three columns are this page's mechanism. In the app the
   workspace sidebar is the working area's left split panel (it already runs the
   full height and never sat in the lane's grid), while the agent rail is
   `.view`'s second column. What binds the implementation is the picture, not
   the mechanism: the lane is `.view`'s first column and never spans the rail's
   track, the agent rail spans both of `.view`'s rows, and hiding either rail
   hands its width back to the lane.
8. **No new design-system surface.** Rows are `list.*`, scope chips `seg.*`,
   chords `kbd`, the empty state `commandCentre.default.empty.rest`, the stage
   field `textInput.*`, the prompt `label.default.root.rest`, the veil
   `modal.scrim`, the lane `shell.default.footer.rest`. `README-prototype.md`
   §"Catalog mapping" is the 1:1 table; the component-conformance and
   consumption gates stay green without new recipes.
9. **Everything plan 124 approved stays approved** except where this set says
   otherwise: the lane's composition (approval strip, composer box with the
   agent controls toolbar, hint row, session-environment foot), the run signal
   (one pulsing dot at the window mark), the chords (`Ctrl+X Ctrl+P` lane,
   `Ctrl+X Ctrl+O` palette), the per-tab lane visibility.

## 4. Reproducing and re-verifying

```bash
# the gate the approval rests on: 18 scenes × 4 themes × 2 widths + the keyboard path
node design-artifacts/tools/capture-palette-stages.mjs            # 159 checks, 144 runs, 15 keyboard steps
node design-artifacts/tools/capture-palette-stages.mjs --no-shots --quiet
node design-artifacts/tools/capture-palette-stages.mjs "--filter=^lane-(railed|full)__.*__narrow$"
```

The tool drives `palette-stages.html` over `file://` (also against this frozen
copy, whose page needs only the files in this directory) and asserts per run:
theme and scene applied, no console error, no non-`file://` request, no
horizontal overflow, the sheet's geometry (composer-box width `delta 0`,
lane-inner width, 6px above the box, `min(52vh, 420px)` cap with internal
scroll), the sheet's and mentions' `backdrop-filter: none` with the veil's
`blur(3px)`, the stage prompt vs the catalogue head, scope chips only in the
catalogue, the mode-aware foot, the session row's `Alt+↵ delete`, the secret
field's masking with an empty composer draft, and — the amendment — the lane's
containment: lane == view pane width, no overlap with a visible rail, rails at
the state the scene names, full working-area width when no rail shows, and the
sheet inside the lane. It then walks the keyboard path (`/`, typing, `↑↓`,
`Esc` + reopen, `@`, `Ctrl+X Ctrl+O`, a depth-3 `Esc`/`Alt+←` trail, `Alt+↵`,
a scope chip). Exit status is the gate.

## 5. Where this is cited

- `plans/125-Composer-Palette-Stage-Flows-and-Centered-Sheet-Retirement.md`
  — the approval/freeze task (this record), the DESIGN.md amendment task
  (§12/§16 sweep + the halo recipe values), and the lane-containment
  implementation task.
- Implementation reads this set before editing: the palette renderer
  (`frontend/src/command-centre/CommandPalette.tsx`, `Composer.tsx`,
  `WorkspacePanes.tsx`), the lane (`frontend/src/shell/AgentLane.tsx`,
  `agent-lane.module.css`), the working-area grid
  (`frontend/src/routes/workspace.module.css` + its regression test), the package
  recipe data (`packages/design-instrument/package.json`) and the host fallbacks
  (`frontend/src/styles/tokens.css`).
- Visual/accessibility review compares the running app against this directory and
  records every deviation with its disposition.
