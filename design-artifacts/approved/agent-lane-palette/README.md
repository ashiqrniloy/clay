# Agent lane + `/` palette — approved design set (frozen 2026-09-17)

**Status: approved by the user on 2026-09-17.** This directory is the binding
design record for plan 124 (Persistent Agent Lane and Slash Command Palette).
`DESIGN.md` is the normative text; this set is the reviewed picture of it.
Implementation copies what is here — it does not re-interpret it, and it does
not edit these files.

Approved work product of `design-artifacts/prototypes/agent-lane-palette/`
(the working copy, which stays live for iteration; losing variants stay there
marked *not approved*). The files below are a byte-identical snapshot **as
reviewed**, with the hashes recorded so a later change to the working copy is
provably not a silent change to the approval.

## 1. Approval record

| | |
| --- | --- |
| Approved by | user |
| Date | 2026-09-17 (three review rounds: 2026-09-16 and twice on 2026-09-17; set frozen 03:43 local) |
| Scope approved | the whole plan-124 shell change: the persistent agent lane (composer box carrying the field, the tab's agent-type/model/effort pickers and the context meter, the hint row, the session-environment foot) mounted once below both view slots; the `/` palette as the one command surface (Control Centre consolidated), spanning that composer box 6px above it, over a shared 3px veil; the `@` mentions menu over the same veil; the agent view's header retired; the window mark as the run's signal and the state strip's working bars; the title bar as the app implements it; the lane-toggle and palette-open chords |
| Approval evidence | "Okay. Design artifact now approved" — after the third review round, following "You misunderstood one feedback. In the streaming state, the bar above the input box that says working, should show the animation with three bars instead of the dot. Remove the dot from there and show the animation." (that round's change is in the frozen `lane-palette.html`) |
| Not approved / not covered | the gate's own evidence tooling (`design-artifacts/tools/capture-agent-lane.mjs`) and the captured PNGs, which stay with the working copy; the lane's animation curves beyond the two named (§7 additions in §5 below); `DESIGN.md` itself, which this approval does not edit (plan 124's amendment task does, and it may disagree — then the specification wins) |

## 2. What is frozen here

**8 files** — the single prototype page, its lane stylesheet and the shared kit
it renders over, plus `README-prototype.md` (the working copy's `README.md` at
freeze time: variants, the 15 decisions, coverage, catalog mapping, named
language additions). This approval record is documentation, not a reviewed
artifact, and is deliberately not in the table. Hashes are the drift detector:
if a file here differs, the approval no longer describes it.

| file | size | sha256[:16] |
| --- | --- | --- |
| `README-prototype.md` | 27.3 kB | `dcee87539f9e292d` |
| `components.css` | 14.9 kB | `01f30073b77148f6` |
| `ds.js` | 53.0 kB | `31f53b20a194a826` |
| `ds-quiet.css` | 30.8 kB | `c2aee13d84b0944a` |
| `lane.css` | 15.0 kB | `0c6548f3fd38cf10` |
| `lane-palette.html` | 66.2 kB | `92795cd7a097deac` |
| `pages.css` | 3.3 kB | `697a63cdee3d8065` |
| `theme.css` | 10.4 kB | `b1a39839be745cdb` |

Two notes on the kit's ownership:

- **Comment-only correction (2026-09-17):** `lane.css` carried two stale
  `2026-06-24` review dates in its comments (the review rounds were 2026-09-16
  and 2026-09-17). The dates were corrected in both the frozen copy and the
  working copy, which is why `lane.css`'s hash is `0c6548f3fd38cf10` rather than
  the review-time `4e8509eea65a6061`; no rule, value or declaration changed, and
  the gate below was re-run against the corrected copy (88 runs, 0 failures).
- `ds-quiet.css` is **shared** with
  `design-artifacts/approved/quiet-instrument-language/ds-quiet.css` and the
  Quiet Instrument migration set — its hash here (`c2aee13d84b0944a`) is the
  same one that set froze. `theme.css`, `pages.css`, `components.css` and
  `ds.js` are likewise that kit's files unchanged (`b1a39839be745cdb`,
  `697a63cdee3d8065`, `01f30073b77148f6`, `31f53b20a194a826`): this approval
  adds one page and one stylesheet, and **no design-language value**. The lane's
  own ink is `lane.css`, which consumes only `--clay-ds-*` recipe variables and
  the §7 motion values.
- Baselines (68 PNGs + `report.json`) stay with the working copy at
  `design-artifacts/screenshots/agent-lane-palette/`, produced by
  `design-artifacts/tools/capture-agent-lane.mjs`.

## 3. What the approval binds

1. **The lane is one instance, shell-wide.** Mounted below both view slots in
   `WorkspacePanes` (sibling of the two `viewSlot` divs), driven by the active
   tab's agent store; the inspector keeps full height (the lane spans the view
   pane, not the window). Composition: an optional approval strip, the composer
   box (field + agent-control toolbar + hint row), the session-environment foot.
   The lane is never hidden as a consequence of a tab's state.
2. **The composer box is the one place the tab's agent, model and reasoning
   effort are chosen**; the agent view's header is retired (transcript, state
   strip, inspector remain).
3. **No Send button.** `↵` sends, the hint row says so, and Stop fills that slot
   exactly while a run is live. `button.primary` is not drawn in the lane.
4. **No provider never blocks typing.** The model trigger stays disabled reading
   `Configure a provider`; effort and meter are absent; the foot states
   `no provider configured · Settings → Providers` in both views.
5. **The `/` palette is the one command surface** (Control Centre consolidated),
   anchored 6px above the composer box and spanning it edge to edge, with the
   server catalogue behind it and scope chips `All · Session · Shell · Files`;
   the composer's draft is the query, so the palette owns no input.
6. **One veil for both composer menus** — `@` mentions and `/` palette share the
   3px translucent scrim; the lane stays above it (it is the input).
7. **The run's signal is the window mark**: the accent dot inside `Clay` pulses
   (tab marker's 1.1s) while the active tab's agent works. The lane's foot
   carries the environment only; the agent view's state strip says the state in
   words and swaps its tone dot for the three working bars **while a turn is in
   flight**.
8. **The title bar is the app's as implemented** (mark + dot, tab strip hugging
   it, spacer, window actions at the right edge: the icon-only Control Centre
   trigger and the tab's view switcher) — no window buttons, no titlebar rail
   toggles.
9. **Chords**: `Ctrl+X Ctrl+P` toggles the lane, `Ctrl+X Ctrl+O` opens the
   palette (drawn pending approval; `Ctrl+X Ctrl+P` is rebound away from
   `controlCenter.open`).

Everything the review drew and rejected, and every instruction that changed the
drawing across the three rounds, is in `README-prototype.md` §2 (variants) and
§1a (the rounds themselves). Two items are recorded there as **open questions
left in the drawing**, not as approved behaviour: whether the app's own tab
marker keeps its `agentPulse` alongside the window mark's dot, and whether the
lane's pointer-only send affordance returns as a palette row if the mandatory
accessibility review finds `↵` alone insufficient.

## 4. Reproducing and re-verifying

```bash
# assert + capture the working copy (11 scenes x 4 themes x 2 widths, 88 runs)
node design-artifacts/tools/capture-agent-lane.mjs
# open the frozen page exactly as reviewed
xdg-open design-artifacts/approved/agent-lane-palette/lane-palette.html
```

`capture-agent-lane.mjs` asserts the claims this approval rests on: the palette
spanning the composer box 6px above it over a real 3px veil with the lane above
it (mentions too), no Send button while Stop tracks streaming, the foot free of
any run marker, the window mark's dot at 1.1s exactly while streaming, the state
strip's dot swapped for three 1.1s bars exactly while a turn is in flight, the
title bar in the app's composition, per-mode lane controls, `backdropBlur == 0`
on content surfaces, no overflow and no console error — then walks the keyboard
path and writes `screenshots/agent-lane-palette/report.json`. Hashes above are
the second check: re-hash the frozen files against the table. The same gate can
be pointed at this frozen copy (it passed here on 2026-09-17, 88 runs / 0
failures):

```bash
node design-artifacts/tools/capture-agent-lane.mjs \
  --page=design-artifacts/approved/agent-lane-palette/lane-palette.html \
  --no-shots --quiet --out=/tmp/frozen-gate
```

## 5. Where this is cited

- Plan: `plans/124-Persistent-Agent-Lane-and-Slash-Command-Palette.md` — every
  implementation and review task names this directory as its binding reference.
- Design system: **amended 2026-09-17 (plan 124 task 4)**. `DESIGN.md` §12 now
  holds this composition (the lane as the shell's one bottom section, the
  composer box owning the tab's agent controls, no Send button, `↵` sends, the
  lane's states, the command surface, the title bar as the app implements it),
  §7 holds the mark's pulse, the working bars and the bottom-anchored entrance,
  §9 the one-run-signal rule, §11 the shared scrim and the palette's anchor,
  §13.10 the veil-versus-its-own-input invariant, §14.14/.15 the retired
  centred command sheet and the retired second blinking dot, and §6/§5 the
  lane's material and the palette's width. §16's shipped-state bullets are
  marked superseded until the implementation tasks rewrite them; the recipe key
  set is unchanged (this approval added no key).
- **One recorded disagreement, resolved by the rule above.** §11 names the
  shipped `modal.scrim` recipe for the composer menus (`surface.scrim` @0.5 +
  `blur 3`), while this set's drawing fills the veil with the canvas at 66% +
  `blur 3` (the theme-neutral prototype kit's own veil, chosen for legibility at
  drawing scale). The specification wins: implementation uses the recipe, and
  plan 124's visual review compares the composited result against these PNGs —
  a mismatch the review finds unacceptable is a theme-side `surface.scrim`
  tweak or a re-drawn artifact, not silent drift in the code.
- Decision log: `decision-logs/2026-09-17-0345-agent-lane-and-slash-command-palette-shell-composition.md`
  records the shell-composition decision this approval implements.
- Build note (2026-09-17): the spec-vs-drawing gap this set shipped with — the
  palette's per-row chord chips and its scope segment needed item fields the wire
  did not carry — is closed. Plan 124's row-field task added them
  (`scope`, `bindings`, protocol version 31), and the shipped sheet draws both
  from those fields, so the drawing above and the specification agree
  (`frontend/src/command-centre/CommandPalette.tsx`). The veil disagreement is
  the one that remains, and it stays resolved the same way: the recipe wins.
