# Clay · Quiet Instrument — approved design language

**Status: approved 2026-09-11** (see `../README.md` for the artifact contract).
The design language below was adopted as Clay's design direction; the normative
specification lives in `DESIGN.md` at the repository root. This folder is the
surface/state reference that implementation and visual review compare against.

Content themes: the four shipped themes (`@clay/theme-modus-operandi`,
`@clay/theme-modus-vivendi`, `@clay/theme-gruvbox-material-dark`,
`@clay/theme-gruvbox-material-light`). The two extra themes in `theme.css`
(Kanagawa Wave, Catppuccin Latte) were proposal-only colour studies and are
**not** shipped — no new theme is implemented.

```
approved/quiet-instrument-language/
├── workspace-rethink.html   screen 1 · Workspace   · Quiet Instrument
├── agent-rethink.html       screen 2 · Coding Agent · Quiet Instrument
├── theme.css                content-theme token contract (incl. two proposal-only palettes)
├── ds-quiet.css             the language: geometry, borders, shadows, motion
├── ds.js                    shared behaviour: themes, palette, overlays, tree, tabs, editor
├── workspace-data.js        generated snapshot of the real repo (tree + file text)
└── README.md                this file
```

Open with `file://` — no build step, no server. The rejected neobrutal
alternatives (language A of this proposal) are kept for history under
`design-artifacts/prototypes/quiet-instrument-language/` and are not authority.
Regenerate the workspace snapshot with
`python3 design-artifacts/tools/make-workspace-data.py`.

---

## 1. What both languages keep

Nothing in the two shipped screens was dropped. The information and the controls
are all still on the page; only their placement and weight changed.

**Screen 1 · Workspace**
`CLAY` brand · `Workspace` tab · window controls · sidebar title `Workspace` and
subtitle `Workspace · clay` · the full file tree with every entry and its item
count · filter · breadcrumb `VENT.md` + path · `v1` · clean/modified ·
editable · Save · Undo · Redo · Close · the relative-path field + Open action ·
the real `VENT.md` markdown source in an editable surface · status bar
`/workspace` + `Connected`.

**Screen 2 · Coding Agent**
everything above, plus: agent title `Coding Agent` · mode `coding` · model
`GLM 5.3 Flash` · `0/1m` context usage · `Effort` · `Skills loaded` with all 13
skill names and descriptions · `MCP servers` `graft` `6 tools` ·
`No conversation yet.` · `Ready` · `Message` with `Ask, or type / or @` · send ·
cancel · `/home/arn/Projects/clay` + `git fix/coding-agent` · `MCP: graft` ·
inspector tabs Files · Memory · Context · Session Info · Settings with the seven
counted categories · status bar `/workspace  no active menu session for id
9223372036854775810 (client 3)`.

Two deliberate relocations, both to remove duplication:

| shipped | proposal |
|---|---|
| model in the header *and* the footer strip; `MCP: graft` twice | model once in the header, MCP once in the composer footer |
| skills + MCP occupying the centre of the agent screen | they live in the inspector's **Context** tab, next to the count table they belong to — the transcript gets the room |

Also retained but made on-demand: the relative-path open field. In language A it
is a strip below the document bar (one line, `⌘O`); in language B the same field
appears on `⌘O` or from the Open action, and the palette can open a path too.

The agent inspector keeps the shipped default tab: **Context** is selected on
first paint in language A, and the tab choice persists per surface
(`data-tab-key`, `data-tab-default`).

---

## 2. Language A — Restrained Neobrutal, rebuilt (`ds-neobrutal.css`)

Keeps Clay's shipped contract — 0 radius, structural borders, opaque fills,
hard zero-blur offset shadows, 100ms snappy motion — and fixes what made it
read as cluttered:

1. **Depth is systematic.** Three shadow steps (`--sh1` control, `--sh2` raised,
   `--sh3` overlay) replace ad-hoc 2–4px offsets; press state shifts into the
   shadow instead of just changing colour.
2. **Borders are semantic.** 2px `--c-line-2` outlines real containers; 1px
   `--c-line` only separates rows inside them. The shipped tree drew the same
   1px line around every row *and* every panel, which is why it read as a wall.
3. **Row rhythm.** Tree entries are one line with a right-aligned count badge
   instead of a two-line `name / 1 items` block: same information, half the
   height, counts finally align in a column. `1 items` → `1 item`.
4. **Disabled states are quiet.** A muted solid border instead of a dashed one
   that looked like a rendering error.
5. **Keyboard affordances are part of the surface.** Shortcut-bearing controls
   carry a visible `kbd` chip; focus is a 2px accent ring with offset; the
   status bar is a hint rail; every zone can mark itself focused.

---

## 3. Language B — Quiet Instrument (`ds-quiet.css`)

Built from first principles rather than from the shipped look. The diagnosis:
the current chrome spends attention on itself. Every panel is outlined, three
border weights compete, hard shadows make static regions look pressable. Clay
is an instrument people sit inside for hours, so the shell should get out of the
way and the *document* should be the only loud thing on screen.

- **One surface.** Zones are separated by whitespace and a single hairline at
  34% ink. Nothing is a box inside a box.
- **Two elevations only.** Canvas and overlay. Overlays lift with a large soft
  negative-spread shadow; static regions never float.
- **Accent is state, not decoration.** Focus, selection, running work. It never
  colours a panel edge or a heading.
- **Type does the structure.** Geist for prose and controls, Geist Mono for every
  datum (counts, paths, keys, timestamps), 10px/0.14em micro-labels for section
  eyebrows. Geist is present locally on this machine and also fetched from Google
  Fonts, so the artifact looks identical online or offline.
- **The document is a real editor column.** Number gutter + a 92ch measure,
  centred. Lines still carry the raw markdown (backticks and all) — only the
  structure is coloured.
- **The file becomes navigable.** A right rail lists every timestamped entry with
  scroll-spy, click-to-jump and `⌥↑`/`⌥↓` stepping, plus the document facts that
  used to be scattered across badges (`v1`, clean, entries, words, language,
  encoding).
- **The agent screen becomes transcript-first.** Turns are hairline-separated
  blocks with a mono role eyebrow, no boxes; the composer is one input surface
  with a nested send island (button-in-button); skills, MCP and the count table
  live in the inspector's Context tab.
- **Motion carries meaning.** 150ms press, 240ms `cubic-bezier(.32,.72,0,1)` for
  surfaces entering, a 620ms pulse when a key moves focus somewhere new, and no
  animation at all when `prefers-reduced-motion` or the Settings toggle says so.
- **A 2% film grain** on a fixed, `pointer-events-none` layer — never on a scroll
  container.

---

## 4. The six themes (`theme.css`)

Themes own colour only; geometry, borders and motion belong to the design
language. Every screen reads the same ~45 semantic tokens, so a theme cannot
break a layout and a new design language gets all six themes for free.

| theme | state | accent | notable token work |
|---|---|---|---|
| `modus-operandi` | shipped | blue `#0031a9` | chrome is now *darker* than the canvas (was flat white-on-grey); added accent, hover, active, selected, kbd, focus and operator roles the manifest never declared; meta text repaired from #6f6f6f-class greys to pass 4.5:1 |
| `modus-vivendi` | shipped | blue `#2fafff` | hairline dropped from the shipped #646464 (too loud for every divider); status strip calmed from #505050; numbers are amber instead of colliding with plain white text |
| `gruvbox-material-dark` | shipped | aqua `#7daea3` | accent was the caret beige `#d4be98` — no state signal at all; placeholder `#7c6f64` (3.3:1) repaired; comment and number roles separated |
| `gruvbox-material-light` | shipped | blue `#076678` | placeholder `#a89984` (2.3:1) repaired — the worst offender of the four; ok/warn/err, heading-2/4 and four syntax roles nudged up to clear 4.5:1 on cream |
| `kanagawa-wave` | **proposed** | crystal blue `#7e9cd8` | sumi-ink dark, muted and low-chroma, deliberately unlike Gruvbox's warmth and Modus' neutrality; hard-shadow colour `#54546d` makes neobrutal depth visible on dark |
| `catppuccin-latte` | **proposed** | blue `#1a5ae0` | soft cool light; peach/yellow/green/purple all darkened from the upstream pastels so code stays readable at 13px |

Modus Operandi / Modus Vivendi remain the canonical light/dark defaults, and the
artifact mirrors Clay's appearance rule: stored choice first, otherwise
`prefers-color-scheme` picks between the two Modus themes.

Measured minimum contrast per theme (WCAG 2.1, `over()` composite for alpha
tokens, foreground against `--c-surface` unless the token sits on its own fill):

| theme | body text | meta / counts | accent + fills | status + kbd | syntax | markdown | diagnostics | focus ring |
|---|---|---|---|---|---|---|---|---|
| `modus-operandi` | 8.19 | 5.33 | 10.44 | 16.83 | 7.00 | 7.05 | 7.00 | 10.44 |
| `modus-vivendi` | 11.91 | 6.49 | 8.70 | 9.46 | 7.28 | 8.77 | 7.03 | 8.70 |
| `gruvbox-material-dark` | 5.91 | 5.56 | 5.94 | 5.90 | 4.70 | 5.37 | 4.70 | 5.94 |
| `gruvbox-material-light` | 5.70 | 5.21 | 5.82 | 4.64 | 4.97 | 4.97 | 4.61 | 5.82 |
| `kanagawa-wave` | 7.48 | 5.43 | 5.94 | 8.24 | 4.53 | 5.59 | 5.09 | 5.94 |
| `catppuccin-latte` | 5.53 | 4.92 | 5.18 | 4.73 | 4.56 | 4.56 | 4.56 | 5.18 |

Every text pairing is ≥ 4.5:1 and every focus ring ≥ 3:1 on its own surface.
This matches the AA floor Clay already enforces for canonical defaults in
`src/server/ops/theme.rs`.

---

## 5. Keyboard map (identical in all four files)

| keys | action |
|---|---|
| `⌘K` | command palette — files, entries, skills, MCP, actions, themes |
| `?` | keyboard map |
| `⌘B` / `⌘I` | toggle file list / inspector (or the outline rail) |
| `⌥1…6`, `` ⌥` ``, `⌥T` | theme by number, cycle, theme menu |
| `⌘S` `⌘Z` `⇧⌘Z` `⌘W` | save, undo, redo, close file |
| `⌘O` | open-path field |
| `/` | filter the file tree (language A) / focus filter |
| `↑ ↓ ← → ↵ Home End` | tree navigation, expand/collapse, open |
| `⌥↑ ⌥↓` | previous / next document entry (language B workspace) |
| `⌘1…⌘5` | inspector tabs (agent screens) |
| `/` then `↑↓ ↵` | slash commands in the composer (agent screens) |
| `@` | mention a workspace file, one of the 13 skills, or `graft` |
| `↵` / `⇧↵` / `Esc` | send · newline · cancel a running turn or clear the draft |
| `F` | filter the skills list |

Keys are resolved against the platform: the artifacts render `Ctrl K` on Linux
and `⌘K` on macOS. Focus rings are always visible; keyboard-driven jumps pulse
the target once (`ClayDS.flash`), and a polite live region announces what
happened for screen-reader users.

---

## 6. How this was checked

- **Contrast**: every theme token set was parsed back out of `theme.css` and every
  meaningful pairing measured (the table above). All pairs pass 4.5:1 / 3:1.
- **Runtime**: each of the four files was loaded in headless Chrome at
  1680×1050, 1280×880, 1024×800 and 900×760, in light and dark themes, with zero
  console errors, zero horizontal overflow (`overflowPx: 0`) and no clipped
  interactive labels.
- **Behaviour**: palette filtering and execution, theme switching and
  persistence (`localStorage`), tree filtering (matching files keep their
  parent folder visible) and keyboard navigation, file opening / saving / undo /
  redo / close, outline jump + scroll-spy, inspector tab switching, skills
  filtering and `@` insertion, slash and mention menus, send / cancel turn, the
  empty → transcript transition, and the Context counters updating from a sent
  turn were all exercised in headless Chrome.
- **No model is simulated**: sends produce a labelled stub and never invent
  token usage, file contents or tool output.
- **Reduced motion** and a compact density are toggleable in Settings and honour
  the OS `prefers-reduced-motion`.

Deliberate stubs, so nothing looks like a false claim:

- The agent does not run a model. A sent turn produces a labelled stub response
  and a note saying so; the context meter keeps the reported `0/1m` instead of
  inventing token usage.
- `workspace-data.js` snapshots the real tree and eight real files (`VENT.md`,
  `README.md`, `AGENTS.md`, `DESIGN.md`, `PRODUCT.md`, `Cargo.toml`,
  `.gitignore`, `build.rs`). Opening any other file shows
  "content … not bundled in this design artifact" rather than invented text.
- Window controls are visual only; clipboard writes fall back to a toast.

Regenerate the snapshot after repo changes:

```bash
python3 design-artifacts/tools/make-workspace-data.py
```
