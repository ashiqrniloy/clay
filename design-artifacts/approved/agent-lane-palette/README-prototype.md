# Agent lane & `/` palette — plan 124 prototype

Exploration for **plan 124** (persistent agent lane and the Control Centre
consolidated into a `/` palette). Open `lane-palette.html` over `file://` — no
build step, no server:

```
xdg-open design-artifacts/prototypes/agent-lane-palette/lane-palette.html
# a known state, for review or capture:
#   lane-palette.html?theme=modus-vivendi&width=narrow&scene=palette
```

**No authority.** Nothing in production may cite this prototype as its
reference; the approved copy under `design-artifacts/approved/` is what
implementation tasks read. `DESIGN.md` wins over both.

## 1. Scope

Three new pieces of shell composition, drawn over the approved shell baseline
(`approved/quiet-instrument-migration/workspace.html` and `agent-landing.html`),
the third folded in by the review of 2026-09-16 (see §3):

1. **The agent lane** — the composer, the tab's agent controls and the
   session-environment foot lifted out of the agent view into one persistent
   lane at the bottom of the view pane, so the workspace view and the agent view
   show the same bottom section. One lane instance sits below both view slots:
   switching views never remounts it.
2. **The `/` palette** — the Control Centre's catalogue, spanning the composer
   box (the same box the `@` completions occupy) 6px above it and opened from
   the composer's draft or the titlebar trigger, over a 3px veil. The centered
   modal the Control Centre uses today is not drawn; the palette is the one
   command surface.
3. **The agent controls in the composer box** — agent type, model, reasoning
   effort and the context meter. The agent is always present, so these left the
   agent view's header (which is gone) and now live inside the lane's composer,
   reachable from either view.

Out of scope (unchanged, deliberately): the transcript, the inspector tabs, the
editor, the sidebar. The `@` mention completions keep their shipped geometry
(rows, sections, the same box) — only the veil below them changed (§3).

## 1a. Second review (2026-09-17)

Five more changes, all visible in the artifact:

1. **No Send button.** `↵` sends, the hint row says so, and the slot the island
   button filled is Stop's while a run is live.
2. **No provider never blocks typing.** The composer stays live; the model
   trigger stays disabled (`Configure a provider`) and the reason nothing will
   send is a warning-toned note in the lane's foot — visible in the workspace
   view too, where there is no state strip to say it.
3. **The run's signal is the window mark, not the lane.** The accent dot inside
   `Clay` pulses (1.1s, the motion the tab marker already uses) while the active
   tab's agent works, and is a steady accent dot otherwise. The lane's foot
   states the session's environment and nothing else — the `● Working` cue drawn
   in the round above is gone. In the agent view the state strip above the
   composer says the state in words and, **while a turn is in flight, swaps its
   tone dot for the transcript's three bars** (`Working` + `▮▮▮`, 1.1s) — motion
   in the words rather than a second blinking dot; the dot returns when the turn
   ends (`streaming__*`, `idle__*`).
4. **The `@` mentions menu gets the palette's veil.** One veil for both composer
   menus, the lane (their input) still above it.
5. **The title bar is the app's, as implemented** (`app-shell.tsx` +
   `shell.module.css`): the mark with its dot, the tab strip hugging it (label +
   the mono agent word), the spacer, then the window actions at the right edge —
   the icon-only Control Center trigger and the tab's view switcher. The
   prototype's window buttons and its inspector/files titlebar toggles are gone
   (the app keeps those in the status bar's hint family; the inspector toggle is
   drawn there instead).
   **Drift recorded:** the earlier approved artifact
   (`approved/quiet-instrument-migration/shell.html`) draws window buttons and a
   `palette · ? · Inspector` action set that the app does not have. The two
   cannot both be normative; the amendment (§7) has to say which one the shell
   follows — the prototype follows the app, because porting code to a drawing is
   the more expensive direction.

## 2. Variants and what happened to them

| Variant | Status | Why |
|---|---|---|
| Lane below the view slots, one instance; palette spanning the composer box whose query is the composer's own draft; agent controls inside that box | **this prototype** | The composer the user asked to keep is the palette's input: no second input, no focus jump, one ring per surface. The controls ride inside the same box, so the sheet above can hide nothing in the lane. |
| Palette as its own sheet with its own input (the shipped Command Centre shape, merely moved down) | rejected | Duplicates the composer directly beneath it, and re-introduces two inputs for one query. |
| Palette as a 640px centered sheet anchored to the lane, veil unchanged | **superseded by review 2026-09-16** | The user asked for the sheet to span the Control Centre input box like the mentions menu; wider rows, one box of reference. |
| Palette scoped to the composer with no veil, like `@` | rejected | The catalogue is a window-level surface; without the veil a long list competes with the transcript for attention, and the user asked for the Control Centre's blurred-backdrop aesthetic. |
| Agent controls as a lane row *above* the composer box | rejected (drawn, then moved) | The sheet hugs the composer, so a row above it is hidden while the palette is open, and the lane grew a chrome row the composer box can carry. |
| Send island button in the composer (`button.primary`, `Send ↵`) | **removed by review 2026-09-17** | `↵` already sends and the hint row says so; a permanent primary button in a surface that is idle most of the day is decoration, and the slot is what Stop needs while a run is live. |
| No provider ⇒ inert composer with `Configure a provider first` in the placeholder | **superseded by review 2026-09-17** | Blocking the text box punishes drafting: a prompt can be written now and sent after Settings → Providers. The reason is stated in the lane's foot instead. |
| `@` mentions menu without the veil (shipped shape, no scrim) | **superseded by review 2026-09-17** | Both menus belong to the same composer and should read as the same gesture: the veil marks "this menu owns the window" and keeps the lane as the one bright input. |
| Streaming animation only in the transcript turn | **superseded by review 2026-09-17** | In the workspace view the transcript is not on screen, so a run in flight gave the lane no sign of life. |
| `● Working` + typing bars in the lane's foot (first round of this review) | **removed by the second round of review 2026-09-17** | The foot is the session's environment — root, branch, extensions, MCP; a run indicator there competes with those facts and duplicates the state strip. The signal belongs at the window mark. |
| Titlebar copied from the earlier prototype kit (labelled `Commands` chip with two chord chips, `Hide files` / `Hide inspector` buttons, drawn window buttons) | **superseded by review 2026-09-17** | The app's title bar is the source: mark, tab strip, spacer, window actions. Adopting it removes three invented surfaces and the false window chrome. |
| Model/effort pickers kept in the agent view's header **and** copied into the lane | rejected | Two owners of one value, and a header for a view that is no longer where the agent lives. |
| Titlebar Control Centre trigger removed, `/` the only entry point | considered, not drawn | Fewer surfaces, but the trigger is the discoverable pointer path and the plan keeps it. Drawn as **Decision A** below. |
| Lane rendered inside each view and hidden when inactive | rejected | Two composer state owners; the lane must be one instance per tab. |

## 3. Decisions called out for approval

These are the questions the prototype deliberately puts in front of the
reviewer; each is visible in the artifact.

1. **The lane spans the view pane, not the window.** It sits in the middle
   column (inside `WorkspacePanes`) so the agent view's inspector keeps its full
   height. In the workspace view the lane therefore ends where the sidebar ends
   (screenshot `idle__modus-vivendi__wide.png`).
2. **The lane stays above the veil.** The scrim covers the working area —
   sidebar, view content, inspector — but not the lane, because the lane *is*
   the palette's input and must stay legible while the palette is open. The
   titlebar and status bar are chrome and are outside the veil
   (`palette__modus-operandi__wide.png`).
3. **The palette echoes the composer's draft** in its header (mono, accent `/`)
   instead of owning an input; the scope chips (`All · Session · Shell · Files`)
   filter the catalogue, and `Files` is where `controlCenter.openPath` lives.
   It spans the composer box exactly: same left/right edges, 6px above it — the
   `@` completions' own anchor (`palette__modus-operandi__wide.png`).
4. **Decision A — the titlebar trigger.** Drawn the way the app draws it (an
   icon-only Control Center button in the window actions), opening the palette
   in the lane (it focuses the composer with `/`); its tooltip carries the
   proposed new chord `Ctrl+X Ctrl+O`. The rejected alternative is removing the
   trigger entirely.
5. **The chord swap.** `Ctrl+X Ctrl+P` is drawn as the **lane toggle**
   (`shell.toggleAgentLane`, in the palette as a catalogue row) and
   `controlCenter.open` moves to the palette-open chord. The status bar keeps a
   hint for each so the keyboard path is discoverable, including while the lane
   is hidden (`hidden__modus-operandi__wide.png`).
6. **An agent-less tab keeps the lane, and only the agent picker.** The lane
   stays in place with a disabled composer (`Attach an agent to this tab to
   send a prompt`), the picker reads `Attach an agent` (it is how one is
   attached), and model/effort/meter are absent — there is no agent for them to
   belong to. The conversation area shows the picker prompt
   (`no-agent__modus-operandi__wide.png`). The lane is never hidden as a
   consequence of the tab's state.
7. **No provider configured** keeps the same lane shape and stays typeable
   (review 2026-09-17): the model trigger is disabled and reads
   `Configure a provider`, effort and meter are absent, `MCP…hidden`, and the
   foot states why nothing will send
   (`disabled__modus-vivendi__wide.png`): the agent-type picker stays live (an
   agent is local configuration).
8. **Approval keeps its shipped component** (`ApprovalStrip`, plan 119 SC-4)
   and moves into the lane above the composer box, focus on `Allow`
   (`approval__modus-vivendi__wide.png`). No new visual language was invented
   for it.
9. **The agent controls are inside the composer box, not above it** (review
   2026-09-16): the agent-type picker, the model picker (`provider/model`), the
   effort picker, and the context meter form the box's second row, and the
   agent view has no header at all. Consequences drawn: the box keeps one ring
   (`:focus-within` on the shell); the sheet above covers nothing in the lane;
   the menus open upward, which the shipped `ClayDropdown` popover already does
   by flipping at the window's edge. A row above the box was drawn first and
   rejected — the sheet would hide it while open (§2).
10. **The pickers are real dropdowns, drawn open** (`menu-effort__*__wide.png`):
    three levels, the current one marked, the choice written back into the
    trigger. Model options are grouped by provider (Anthropic, OpenAI) with the
    provenance note `Providers come from Settings → Providers.`; the agent menu
    carries the shipped footer (`Coding Agent · 12 skills · from
    ~/.clay/agents/`) and the effort menu the cycle chord (`Shift+Tab`).
11. **No Send button** (review 2026-09-17). The lane's composer is a text field,
    the agent controls, the hint row (`/ commands`, `@ mention file or skill`,
    `↵ send · ↵ newline`) and the meter; Stop appears in the field row only
    while a run is live (`streaming__*`, `idle__*`). Consequence for the
    implementation task: the lane's composer is a `Composer` variant with no
    submit button — the server intent path is unchanged, only the button is
    gone.
    **Recorded consequence:** `↵` is then the only send path, so a
    pointer-only user has no submit affordance in the lane. Accepted by the
    review of 2026-09-17; if the mandatory accessibility review (§7) finds it
    needed, the affordance returns as a palette row (`Send prompt`) rather
    than as a permanent button. The stop affordance stays a button while a run
    is live, so an in-flight run is still reachable by pointer.
12. **No provider keeps the composer live** (review 2026-09-17): typing,
    `/ commands` and `@ mentions` all work; the model trigger is disabled and
    reads `Configure a provider`, effort and meter are absent, and the foot
    carries `no provider configured · Settings → Providers` in warning tone
    (`disabled__*__wide.png`). The agent-less tab is the only state that still
    blocks typing (there is nobody to send to).
13. **The run's signal is the window mark** (review 2026-09-17, second round;
    strip cue second/third round): the accent dot inside `Clay` pulses at 1.1s
    while the active tab's agent works (`streaming__*`), a steady accent dot
    otherwise (`idle__*`). The lane's foot carries the environment and the
    no-provider note only. The agent view's state strip says the state in words
    and, while a turn is in flight, replaces its tone dot with the transcript's
    three bars at the same 1.1s (`streaming__modus-operandi__wide.png`) — so
    "working" is drawn as motion in the words, no dot blinks in the strip, and
    the tone dot comes back the moment the turn ends.
    **Open question, drawn for review:** the app's tab marker also pulses while
    the agent works (plan 118 T33, `tab-strip.module.css` `agentPulse`). The
    prototype draws that marker steady (accent, `data-busy="true"`, title
    `Agent working`) so only the mark blinks; keeping both pulses, or moving the
    pulse to the tab marker alone, are one-line changes either way.
14. **One veil for both composer menus** (review 2026-09-17): the `@` mentions
    menu opens over the same `blur(3px)` translucent scrim as the palette, with
    the lane above it (`mention__modus-operandi__wide.png`). Its rows, sections
    and geometry are unchanged.
15. **The title bar is the app's** (review 2026-09-17, second round): mark +
    dot, tab strip hugging it (`clay` + the mono `agent` word + the add-tab
    button), spacer, then the window actions at the right edge — the icon-only
    Control Center trigger (`control-center.open`, the app's own glyph) and the
    tab's view switcher. The prototype's labelled `Commands` chip, its two
    `Hide …` toggles and its window buttons are gone: the app has no window
    buttons (the frame is the OS window) and keeps rail toggles in the status
    bar's hint family, where the prototype now draws the inspector toggle.

## 4. Coverage

Eleven scenes, four shipped themes, two window widths (`1500` and `1024`); the
capture tool asserts every combination (88 runs) and stores 68 PNGs — every
scene × every theme at 1500, every scene at 1024 in Modus Operandi and Modus
Vivendi, plus `menu-effort__<theme>__wide.png` for the open dropdown.
`report.json` holds each run's measurements plus the keyboard walk.

| Scene | What it shows |
|---|---|
| `idle` | Lane in the **workspace view**, idle: controls row, no submit button (`↵` sends), session foot. |
| `agent` | Lane in the **agent view**, idle, transcript + inspector intact, no header. |
| `streaming` | A run in flight: `Stop` in the composer, the transcript's typing bars, the state strip's tone dot swapped for those same three bars beside `Working`, and the window mark's dot pulsing beside `Clay` — the lane's foot unchanged. |
| `approval` | Suspended run: approval strip in the lane, `Allow` focused, state `Suspended`. |
| `no-agent` | Agent-less tab: inert composer, `Attach an agent` only, picker prompt in the view. |
| `disabled` | No provider: composer live (typing, `/`, `@` all work), model trigger disabled, foot note `no provider configured · Settings → Providers`, MCP hidden. |
| `hidden` | `Ctrl+X Ctrl+P` state: lane gone, status bar says `Lane hidden` and keeps the hint. |
| `palette` | `/` palette open over the veiled workspace view, 12 catalogue rows, spanning the composer box. |
| `palette-agent` | The same palette over the agent view — the lane does not move. |
| `palette-empty` | No-match query (`/zzz`): empty state, `0 results`, `Esc` hinted. |
| `mention` | `@` completions: shipped rows and sections above the same box, now over the same veil as the palette. |

States verified by the tool rather than by eye: the scene/theme really applied,
`--c-bg` matches the theme, the lane's visibility, the palette's left/right
edges on the composer box's (±2px, never narrower) with its bottom 6px above
that box, veil `blur(3px)` with a translucent fill and a z-index under the lane,
`backdropBlur == 0` on the editor, transcript, sidebar and rows (DESIGN.md
§14.5), no horizontal overflow at either width, no element wider than the window
frame, no console error, no remote request, **no Send button anywhere** in the
DOM, `Stop` present exactly while streaming,
the composer's `Stop`/disabled state per scene, no agent header in the agent
view, the lane's foot carrying **no** run indicator (and never the word
`Working`), the window mark's dot pulsing at 1.1s **exactly** while streaming,
and the state strip above the composer doing the reverse — its tone dot present
with no turn in flight and replaced by exactly three 1.1s bars beside `Working`
while one is (never both) — the tab marker carrying its state by colour with no
animation of its own, the title bar laid out as the app has it (mark, one window
tab in the
strip, spacer, Control Center trigger + view switcher inside `Application
controls`, view switcher within 16px of the bar's right edge, no window
buttons), and the
per-mode
control set (no-agent: only the picker; no provider: **live composer** +
disabled model picker + the foot note and no effort/meter; otherwise model
`provider/model`, effort, meter).

**Keyboard-only pass** (in `report.json`, both light and dark): `Ctrl+X Ctrl+P`
hides and restores the lane; `Ctrl+X Ctrl+O` opens the palette with `/`;
`/zzz` shows the empty state; `/compact` narrows to the matching row; `Esc`
closes the palette and hands the composer back; `@` opens mentions (and the
veil); clearing the
draft closes them; the effort menu opens with its three levels and a pick
(`high`) writes into the trigger; the agent menu opens and a pick
(`Code Reviewer`) writes into the lane.

Re-run the evidence:

```
node design-artifacts/tools/capture-agent-lane.mjs            # asserts + writes screenshots/agent-lane-palette/
node design-artifacts/tools/capture-agent-lane.mjs --no-shots --quiet
```

## 5. Catalog mapping (approval → catalog entries)

The page carries `data-clay-ds` recipe attributes so every surface maps to a
shipped key; nothing here needs a new *package-facing* kind or token.

| Prototype element | Catalog / recipe | Notes |
|---|---|---|
| Palette sheet | `commandCentre.default.root.rest` | `surface.overlay`, radius 16, overlay shadow, 1px hairline. Reused as-is; only its **placement** is new (the composer box's width, 6px above it). |
| Palette rows, lane pickers | `list.default.row.rest`, `dropdown.default.{trigger,popover,item}.rest` | The three lane pickers are the shared dropdown family; the agent-type trigger additionally carries `agentPicker.default.trigger.rest` as it does today. Menus open upward — the react-aria popover flip, drawn here as `.popover--up`. |
| Palette veil | `modal.default.scrim.rest` | `surface.scrim` @0.5, `backdropBlur: 3` — the same recipe `ClayModal` already projects, now shared by both composer menus (palette + `@` mentions). |
| Palette empty / status | `commandCentre.default.empty.rest` / `.status.rest` | Shipped keys. |
| Palette rows | `list.default.row.rest` | Selected row = `accent.primary` @0.15 fill, no leading bar (§14.13). |
| Lane foot | `shell.default.footer.rest` | Shipped: transparent, `text.muted`, 1px hairline. |
| Lane composer well | `textInput.default.field.rest` + `textInput.default.input.rest` | Composer variant: radius 12 well owns the ring (`:focus-within` accent + spread-3 halo); the textarea inside draws none (§14.4). |
| Send / Stop / Allow / Deny | `button.danger.root.rest`, `button.default.root.rest` | Stop is the icon-only danger button; `Deny` is a default button, exactly as the shipped `ApprovalStrip` renders it. **No primary button in the lane** (review 2026-09-17 — `↵` sends), so `button.primary.root.rest` is not drawn here. |
| Lane foot, no-provider note | `shell.default.footer.rest` (footer), *(no recipe family)* for the note | The foot is the shipped key. The note is warning tone in the foot's mono scale — text, no surface. |
| Window mark's dot | `shell.default.brand.rest` | The accent dot itself is host chrome (`shell.module.css` `.brand::before`); the run's signal animates **that** dot with the tab marker's own 1.1s `agentPulse` — no new recipe value, one §7 motion sentence pending approval. |
| Hints, kbd chips, status dots | `kbd.default.root.rest`, `statusDot.{success,busy,warning,muted}.root.rest` | The strip's tone dot is the shipped `statusDot` and is **steady**: the busy tone is not a dot at all — while a turn is in flight the strip draws the working bars (see §6.9) in the dot's place, at the text's own scale. |
| Working bars (transcript's live turn, state strip) | *(no recipe family)* | Three 4×11px accent bars, pill radius, 1.1s pulse with 160ms offsets — the same cadence as the tab marker's `agentPulse`, drawn as bars instead of a fade. New markup + one keyframe; nothing here needed a recipe key. |
| View switcher, scope chips | `seg.default.item.rest` | Tab chrome, per §12. |
| Agent-type / model / effort pickers | `agentPicker.trigger.rest`, `dropdown.default.trigger.rest` | They ride inside the composer box; the meter beside them is type/`statusDot`-scale text, not a new surface. |
| Approval strip | *(no recipe family)* | Ships as `ApprovalStrip` today and is not re-skinned; it is *moved*, and stays warning-toned text plus two text buttons. |

## 6. Needed language / catalog additions (named)

1. **The lane is a Clay-native internal shell surface** — like the tab bar and
   the status bar. It is host-owned composition, not a package-facing
   `ComponentKind`, and it needs a row in the Clay-native surface table
   (`docs/reference/ui-components.md`, `references/components.md`) plus the §12
   shell-composition amendment.
2. **§7 motion addition: a bottom-anchored sheet entrance.** The shipped
   sheet/palette entrance is the top-anchored `translateY(-12px) scale(0.99)`;
   a palette that grows upward from the lane should rise from its own edge
   (`translateY(10px) scale(0.99)`, 240ms `spring-snappy`, unchanged curve and
   duration). Geometry only — no new value.
3. **Geometry: the palette's placement** — the composer box's own width (the
   `@`-completion anchor), 6px above that box; the lane's own padding at ≤1000px.
   No new token, and `dimension.overlay.centered.width` is not involved.
4. **The composer box carries two rows** (field + agent-control toolbar) and one
   ring. This is a `Composer` composition change, not a new recipe; the pickers
   are the shared `dropdown` family inside the `textInput.default.field` ring.
5. **Protocol, already in plan 124:** `shell.toggleAgentLane` on
   `Ctrl+X Ctrl+P` (client-local, like the rail toggle) and `controlCenter.open`
   rebound to the palette-open chord. Drawn as `Ctrl+X Ctrl+O` pending approval.
6. **The veil gains a second caller, not a new recipe** (review 2026-09-17):
   the `@` mentions menu opens `modal.default.scrim.rest` alongside the palette.
   If the amendment prefers a distinct scrim key for composer menus, that ships
   as additive data per §16 — the drawn treatment is the existing one.
7. **No new recipe keys are required.** Nothing in this prototype needed a
   `commandCentre.*`, `shell.*` or scrim addition; if approval changes the
   palette's variant (e.g. a distinct bottom-anchored variant), that ships as
   additive design-system data per §16.
8. **§7 motion: the window mark is the run's signal** (review 2026-09-17,
   second round). The accent dot inside the mark (`shell.default.brand.rest`,
   drawn by the shell's own CSS) pulses with the tab marker's existing 1.1s
   `agentPulse` while the active tab's agent works; §12 gains the sentence that
   the lane's foot states the environment only and that the mark is the window's
   single blinking element. No new value; the §14.4 "one signal per state" note
   needs the same sentence so the tab marker's `busy` colour (no motion) and the
   mark's pulse do not read as two competing signals.
9. **§7 motion: the working bars** (review 2026-09-17, third round). Three 4px
   accent bars at the text's cap height, pulsing 1.1s `ease-in-out` with 160ms
   offsets — the tab marker's cadence, drawn as bars so "working" is motion in
   the words rather than another dot. Drawers: the transcript's live turn
   (already drawn in the prototype) and, new, the agent view's state strip,
   where they **replace** the tone dot while a turn is in flight
   (`CodingAgentPanel.tsx` `styles.stateStrip`). One motion entry, one markup
   pattern shared by two callers; no recipe key, and the shipped
   `statusDot.busy` fill then has no caller in the strip.

## 7. Security

Placeholder data only: `/workspace` as the root, `feat/agent-lane` as the
branch, catalogue ids copied from `src/protocol/mod.rs` and the shipped
`coding-agent` package, and session-file names that are repo-relative. No
credentials, no absolute machine paths, no real workspace contents. The
prototype has no command authority: rows are inert markup, and activation shows
a toast — it never dispatches anything.

## 8. Files

| File | What it is |
|---|---|
| `lane-palette.html` | The shell: titlebar, sidebar, both views, lane (composer box with the agent controls, hints, session foot), palette, inspector, status bar, scenes and live behaviour. |
| `lane.css` | The three new pieces of composition (lane, controls row, palette), colours from `theme.css` roles and geometry from `ds-quiet.css` values only. |
| `theme.css`, `ds-quiet.css`, `components.css`, `pages.css`, `ds.js` | The approved kit, copied from `prototypes/quiet-instrument-migration/` so the prototype opens standalone (its `.agent-head` rules are now unused here — the agent view has no header). |
| `../../tools/capture-agent-lane.mjs` | The gate: asserts the 88-run matrix, the keyboard walk and the lane's dropdowns; writes the PNGs and `report.json`. |
| `../../screenshots/agent-lane-palette/` | The captured evidence (68 PNGs + `report.json`). |
