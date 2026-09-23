# Quiet Instrument migration — approved design set (frozen 2026-09-11)

**Status: approved by the user on 2026-09-11.** This directory is the binding
design record for plan 118 (Quiet Instrument migration). `DESIGN.md` is the
normative text; this set is the reviewed picture of it. Implementation copies
what is here — it does not re-interpret it, and it does not edit these files.

Approved work product of `design-artifacts/prototypes/quiet-instrument-migration/`
(the working copy, which stays live for iteration). The files below are a
byte-identical snapshot **as reviewed**, with the hashes recorded so a later
change to the working copy is provably not a silent change to the approval.

## 1. Approval record

| | |
| --- | --- |
| Approved by | user |
| Date | 2026-09-11 (artifact review; set frozen 23:31 local) |
| Scope approved | the whole migration design set: page surfaces, component states, theme values, the target IA (tab model, launcher, agent picker, session files), the composer treatment |
| Approval evidence | "Design approved and you can log it. Keep the halo and yes one workspace + One agent per launch, changeable after launch" — following the component-level round: "The selected package is highlighted with a blue tint but with also a blue thick only at the left side. Remove this outline from there and at component level. Otherwise all good." |
| Decision log | `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md` (the target IA, superseding point 5 of `2026-09-11-1700-…`); `decision-logs/2026-09-11-1615-quiet-instrument-design-language.md` (the language); `decision-logs/2026-09-11-1655-html-prototype-approval-gate-and-design-artifacts.md` (this gate) |
| Superseded by | nothing |

Decisions this round settled (each is now in `DESIGN.md` and in plan 118):

1. **The composer has one boundary**: the shell's accent border **plus its 3px
   halo** (halo deliberately kept), and the field inside draws no outline of its
   own; the field spans its zone with the send/cancel controls inside its
   trailing edge.
2. **A selected row is a fill, nothing else.** The 2px leading accent bar is
   retired at component level (`DESIGN.md` §14.13) — it appeared beside the fill
   on tree, outline and package rows, which read as two signals.
3. **A tab is one workspace plus one agent, with two views** (Workspace | Agent),
   the switcher in tab chrome, `⌘1`/`⌘2`.
4. **The launcher is the landing surface** (the content of every empty tab; `⌘T`
   opens a new tab on it): recent workspaces and available agents, each pane
   single-select, **at most one of each per launch**, both changeable in the tab
   afterwards (agent from the picker, folder from the workspace view).
5. **The agent view's title is the agent-type picker**; the Files tab is the
   session's file history, opened in the workspace view — not a file browser.

Superseded or not approved:

- The **Restrained Neobrutal** and **Luminous Glass** proposal pages
  (`design-artifacts/prototypes/quiet-instrument-language/`), and the two custom
  proposal themes (Kanagawa Wave, Catppuccin Latte): not part of this set, and
  removed from the product by `decision-logs/2026-09-11-1700-…`.
- The **Coding Agent as the window's landing surface** (`2026-09-11-1700` point
  5): superseded by the launcher (`2026-09-11-2331-…`).
- The agent page's **editor-in-the-inspector** behaviour (a Files tab that loaded
  documents): replaced by the session file history.
- **All prototype-only scaffolding** in the working copy — the review frame
  (`pages.css` chrome, the theme/width switchers, the page nav), the catalog's
  specimen cells, the `data-fx` state forcing, and the recorded
  `start-recents.json` fixture — is review apparatus. It binds the *look* of what
  it demonstrates, not an implementation detail; none of it ships as host CSS.

## 2. What is frozen here

**20 reviewed files** — the 10 page prototypes, the component catalog, the
theme-value board and its specification, the shared kit, the launcher's recorded
fixture — plus `README-inventory.md` (the working copy's `README.md` at freeze
time: surface inventory, coverage, findings). This approval record is
documentation, not a reviewed artifact, and is deliberately not in the table.
Hashes are the drift detector: if a file here differs, the approval no longer
describes it.

| file | size | sha256[:16] |
| --- | --- | --- |
| `README-inventory.md` | 40.6 kB | `6054f23f0fdc6952` | `6054f23f0fdc6952` |
| `agent-landing.html` | 85.0 kB | `1d547255a1a7bf5c` |
| `agent-settings.html` | 14.1 kB | `7e1767320cd0105b` |
| `command-centre.html` | 14.1 kB | `2dd0571848538349` |
| `component-catalog.html` | 120.2 kB | `55976ba2ac66642c` |
| `components.css` | 14.5 kB | `01f30073b77148f6` |
| `ds-quiet.css` | 30.1 kB | `c2aee13d84b0944a` |
| `ds.js` | 51.7 kB | `31f53b20a194a826` |
| `overlays.html` | 21.1 kB | `2d7d343086b8c253` |
| `package-workspace.html` | 15.0 kB | `e69db03daec91ebd` |
| `pages.css` | 3.2 kB | `697a63cdee3d8065` |
| `settings.html` | 15.1 kB | `b2d208a1e47e1174` |
| `shell.html` | 14.7 kB | `77eed8d72b482858` |
| `start-recents.json` | 1.1 kB | `43de705cc9ba5b17` |
| `start.html` | 21.6 kB | `0591d158b74d9d34` |
| `theme-values.json` | 7.9 kB | `c1e677aa930ed4ac` |
| `theme-values.md` | 21.7 kB | `fc634ac64c69c2d6` |
| `theme.css` | 10.2 kB | `b1a39839be745cdb` |
| `themes.html` | 49.6 kB | `69387ffe4ebbe2b2` |
| `workspace-data.js` | 22.8 kB | `48251dfe5de02e29` |
| `workspace.html` | 49.4 kB | `a73a4a83986aad85` |

Two notes on the kit's ownership:

- `ds-quiet.css` (the approved Quiet Instrument language) is **shared** with
  `design-artifacts/approved/quiet-instrument-language/ds-quiet.css`: the
  language has one definition, and a change to it is a change to this approval
  too. `theme.css` (the four shipped content palettes) and `ds.js` (the
  prototype interaction layer) are likewise the same files that kit shipped,
  with the prototype-specific query-parameter handling in `ds.js`.
- `theme-values.json` / `theme-values.md` are the theme migration specification
  (typed `designTokens` per theme), not a page. `themes.html` is the board that
  reviews them.
- Baselines (79 PNGs, `screenshots/report.json`) stay with the working copy at
  `design-artifacts/prototypes/quiet-instrument-migration/screenshots/` — the
  structure in `design-artifacts/README.md` keeps captured baselines next to the
  prototype set that produced them.

## 3. Reproducing and re-verifying

```bash
python3 design-artifacts/tools/make-pages.py            # writes the prototype copy
python3 design-artifacts/tools/make-component-catalog.py
python3 design-artifacts/tools/make-theme-values.py
python3 design-artifacts/tools/make-workspace-data.py
node design-artifacts/tools/capture-prototypes.mjs      # asserts + captures
```

The generators write the **prototype** directory, never this one: this is a
record of what was approved, and it changes only with a new approval round.
`capture-prototypes.mjs` asserts, per run: no console error, every referenced
resource a sibling file (no network), no document overflow, no clipped visible
box, the theme actually applied, the declared evidence elements present, the
declared scenes present and exclusive, the behaved claims (tree filter, palette
empty state, launcher pick/selection) actually behaving, and — for the
agent-file and package pages — that printed sizes and paths match the files on
disk. Last run at approval: **112 runs / 24 scene runs, 0 failures, 79 PNGs**.

## 4. Coverage at the approval level

Full per-surface inventory (component kinds, internal surfaces, chrome
primitives, page regions, owner files): the working copy's
`README.md` §2–§7. At approval level:

| artifact | what it binds |
| --- | --- |
| `start.html` | the launcher: two panes, per-pane filter, single-select, action row naming what it opens, first-run state |
| `shell.html` | the app shell: workspace tab strip (+ `+`), the tab's Workspace \| Agent switcher, pane split, status bar |
| `workspace.html` | the workspace view: tree, 92ch document column, on-demand path strip, outline rail with entry walk |
| `agent-landing.html` | the agent view: agent picker, 72ch transcript, one-boundary composer, inspector (Files = session history), six state scenes |
| `agent-settings.html` | delivered agent files (skills with real paths/sizes) over a Settings tab |
| `settings.html` | settings composition: grouped form, invalid state, collapse, theme popover with live swatches |
| `command-centre.html` | the command palette: query, grouped results, keyboard row, no-results state |
| `package-workspace.html` | the trust domain as a surface: roots, helpers, fingerprint status, package files |
| `overlays.html` | popover, dropdown, menu, tooltip, modal, scrim — including the reduced-transparency fallback |
| `themes.html` | the four shipped palettes with proposed values: border ladder, state fills, composited contrast verdicts |
| `component-catalog.html` | every declared recipe key (130) across the 7 canonical states, plus the language extensions for surfaces the contract does not name |

## 5. What is not covered by this approval

Carried into the plan as findings, not silently accepted — the working copy's
`README.md` §9 lists all 16. The ones that touch this approval:

- The **workspace view** is approved in one state. The no-workspace, read-only
  and filtered states are not prototyped (plan 118 task 10 requires a decision on
  whether they need artifacts of their own).
- The four new families (`viewSwitch`, `agentPicker`, `recentRow`, `sessionRow`)
  have no catalog specimen — the specimen matrix is generated from the *declared*
  contract, so they get theirs with the recipes in task 8.
- The film grain on `.win`/`.desk` in the language kit is a `DESIGN.md` §14.7
  violation that the prototype set suppresses; task 16 deletes it.
- `themes.html`'s verdicts are computed from composited alpha; the runtime's
  current `contrast_ratio` ignores alpha (task 14).
