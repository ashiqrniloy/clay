# Quiet Instrument migration — surface inventory

> **Approved 2026-09-11.** This directory is the working copy; the approved
> snapshot is frozen at `design-artifacts/approved/quiet-instrument-migration/`
> (with hashes and the approval record). Iteration continues here; a change that
> alters what was approved needs a new approval round.

Planning artifact for plan 118, task 2. It is the checklist that tasks 3–6
(prototype), 8–25 (implementation) and 27 (visual review) close out: every
component kind, internal surface, chrome primitive and page/region in the app
appears exactly once, with the file that currently owns its paint, what it
consumes today, the approved `DESIGN.md` target, and where it is demonstrated.

**This directory is a prototype.** Nothing here is authority. `DESIGN.md` and
the frozen artifacts in `design-artifacts/approved/` are. Nothing in
`frontend/`, `src/` or `packages/` changes until task 7 freezes an approved
migration prototype set.

## 0. Authority and catalog rules (restated, unchanged by this migration)

1. **Recipes are inert data.** A design system is a declarative
   `clay.contributions.uiDesignSystem` manifest: no CSS, no scripts, no Tauri
   APIs, no package-name branching. The host renders and consumes; packages
   declare.
2. **Colour authority stays with themes.** Recipes may only reference theme
   colour roles (`surface.*`, `text.*`, `border.*`, `accent.*`, `focus.ring`,
   `diagnostic.*`, `transparent`). A literal colour (`#rrggbb`, `rgb()`) or a
   palette alias is rejected at parse time.
3. **Geometry, material and motion are the design system's.** Radii, border
   widths, shadows, blur, opacity, durations, timing and transform presets come
   from recipes and their `values` block — never from host CSS literals.
4. **No host branching on package names.** The frontend resolves whatever the
   runtime activates through the recipe/variable boundary; the server resolves
   trusted packages from the compiled bundled inventory (exact name + version +
   root + manifest fingerprint), never from `@clay/*` naming.
5. **Real values only.** Prototypes and implementation show values that can come
   from the app; no fabricated token counts, usage meters or tool output.

## 1. Coverage decision (what is already demonstrated, what is new work)

The approved artifact set (`design-artifacts/approved/quiet-instrument-language/`)
contains **two screens** — `workspace-rethink.html` and `agent-rethink.html` —
plus its kit (`ds-quiet.css`, `theme.css`, `ds.js`). It demonstrates the
language, not the catalog: it proves composition, type roles, state language and
materials for the two main surfaces, and nothing else.

| Surface group | Demonstrated by the approved artifact? | Prototype work |
| --- | --- | --- |
| Workspace composition (tree · editor 92ch · rail, on-demand path strip, status bar, entry walk, palette, keyboard map) | **Yes** — `workspace-rethink.html` | Re-render under the four *shipped* theme values (`themes.html`, task 5), otherwise reuse |
| Agent composition (header · transcript 72ch · composer · inspector tabs, capabilities in Context, landing framing) | **Partly** — `agent-rethink.html` shows the populated screen, not the *fresh-window landing* state | `agent-landing.html` (task 4) adds first-run, empty transcript, running, error, resumed-session |
| Component catalog (every kind × variant × slot × state) | **No** — only the states the two screens happen to use | `component-catalog.html` (task 3) |
| Shell/title bar, tab strip, pane split tree, SDUI panel/renderer, package workspace, settings, agent settings, command centre, overlay family (modal, tooltip, dropdown, menu, toast, scrim) | **No** | `shell.html`, `settings.html`, `agent-settings.html`, `command-centre.html`, `package-workspace.html`, `overlays.html` (task 4) |
| Theme values (four shipped themes with the proposed typed `designTokens`) | **No** — the approved kit carries six *proposal* palettes; only four ship | `themes.html` + `theme-values.md` (task 5) |
| Chat surface | **No, and never** — removed by tasks 23–24 | none (deleted, not re-skinned) |

## 2. Package-facing component kinds (`src/shell/components.rs` — 16 kinds + reserved `table`)

`Owner` = the CSS module that currently paints it; `today` = `--clay-ds-*`
references in that module; `§11` = the target recipe in `DESIGN.md`; `demo` =
where it becomes visible for review.

**Slots below are the package contract, not the prose.** They were read from the
then-shipped package (50 families / 142 keys, minus chat); the shipped contract
today is `packages/design-instrument/package.json` (40 families / 165 keys, the
`chat.default.*` keys still pending the chat removal). The first draft of this
table took its slots from `docs/development/ui-design-system-recipe-matrix.md`,
which documents a wider vocabulary than any shipped package declares — see
finding 6. The package key set is the contract; the prose is a wish list until a
surface consumes it.

**The `today` column is the prototype-era measurement, not the current state.**
Task 16 adopted the shared component modules and task 17 the SDUI, editor-chrome
and package-surface modules; consumption and drift are asserted continuously by
`frontend/src/test/design-system-consumption.test.ts` against the recorded
backlog in `frontend/src/test/fixtures/design-system-adoption-backlog.json`, and
what a module paints is read from the shipped manifest. Read the rows below for
the *target*, not for the ref count.

| Kind | Owner (current) | today | `DESIGN.md` §11 target | demo |
| --- | --- | --- | --- | --- |
| `button` (`default`/`muted`/`primary`/`danger` × rest/hover/active/focus/disabled; slot `root` only) | `components/button.module.css` | 85 refs (all 20 recipes) | transparent default +1px `border.hairline` +r8; primary `accent.primary` fill with `surface.main` text; danger = `diagnostic.error` @0.16 fill; muted = text action, no border; focus = 2px `focus.ring` offset 2; active `press-shift-down`; disabled `text.disabled` + `opacity.disabled` | catalog (task 3) + every page |
| `textInput` (slots `field`, `label`, `input`, `description`, `error`; input states rest/hover/focus/disabled/invalid) | `components/text-field.module.css` | 24 refs (9 recipes) | `surface.control` fill, 1px hairline, r12 (composer/textarea) or r8 (single line); focus = `accent.primary` border + accent halo (spread 3 @0.15); invalid = `diagnostic.error` border; `description`/placeholder `text.muted`; error text caption `diagnostic.error` | catalog + settings, command centre, agent composer |
| `dropdown` (slots `root`, `trigger` ×5 states, `popover`, `list`, `item` ×4 incl. selected) | `components/controls.module.css` | 40 refs (12 recipes) | trigger like a default button; popover = `surface.overlay`, r12, 1px hairline, pop shadow, `spacing.xxs` padding; item selected = accent @0.15, `text.primary` | catalog + agent header (model/effort, settings selects) |
| `list` (slot `row` ×rest/hover/active/focus/selected; no `root`) | `components/controls.module.css`, `agent-settings/agent-settings.module.css` (the agent Settings tab's delivered-file rows) | 23 refs (5 recipes) | transparent root, no border; row r8; hover `surface.hover`; selected accent @0.15 + 2px inset accent bar on navigation lists; `rowDetail` `text.muted` caption | catalog + every list in the app |
| `collapse` (slots `root`, `header` ×rest/hover/focus, `body`; the chevron is part of `header`, not a slot) | `components/controls.module.css` | 23 refs (5 recipes) | transparent header, hover `surface.hover`; body 1px hairline top; chevron `text.muted` rotating 150ms | catalog + settings groups, inspector sections |
| `modal` (slots `scrim`, `dialog`; no `root`) | `components/modal.module.css` | 16 refs (2 recipes) | scrim `surface.scrim` @`opacity.scrim` + `backdropBlur` 3; dialog `surface.overlay` r16 1px hairline + overlay shadow; 240ms `spring-snappy` from `translateY(-12px) scale(0.99)` | catalog + `overlays.html` |
| `panel` (`default` slots `root` + `header`; `fixed`/`transient` slot `root` only) | `sdui/registry.module.css` 13, `sdui/renderer.module.css` 7, `packages/package-workspace.module.css` 24, `routes/fixture.module.css` 9 | 53 refs | `surface.panel` @0.55 veil, 1px hairline, r12, no shadow; header hairline bottom `spacing.sm`; title `typography.section`; body transparent | catalog + SDUI pages, package workspace, agent inspector |
| `label` (variants `body`, `title`, `status`, `display`, `section`, `detail`, `caption`) | `components/text.module.css` | **0 refs — 8 recipes unconsumed** | text roles per `DESIGN.md` §8: body 13px, detail/caption 12px, section/title 14–16px, micro-labels 10px uppercase tracked +0.14em, mono for data/counts/paths | catalog (assignment is part of task 16) |
| `statusItem` | `app/layout/shell.module.css` (declared owner) | **0 refs — unconsumed** | `text.muted` at rest, `text.disabled` when inactive; live region | catalog + status bar |
| `flex` (`row`/`column`), `stack` | SDUI host (`sdui/registry.module.css` / `renderer.module.css`) | **0 refs — 4 recipes unconsumed** | layout-only, transparent to AT; `gap` from spacing tokens | catalog + SDUI page |
| `overlay`, `portal` | SDUI/renderer host | **0 refs — 2 recipes unconsumed** | overlay `surface.overlay` r12 1px hairline pop shadow; portal fixed z-layer | catalog + `overlays.html` |
| `scroll` (slots `root`, `scrollbarTrack`, `scrollbarThumb` rest/hover/active) | SDUI/editor host | **0 refs — 5 recipes unconsumed** | track transparent; thumb `surface.scrollbar` at `opacity.disabled`, hover/active 1.0, pill radius, 8–9px | catalog + `overlays.html` |
| `editor` family (slots `root`, `container`, `gutter`, `activeLine`, `selection`, `matchingBracket`, `findMatch`, `chrome`, `path`, `tooltip` ×rest/selected) | `editor/editor.module.css` (family `editor.default.*`) | 35 refs (11 recipes) | editor on canvas, 92ch measure; gutter `text.muted` mono; active line = subtle surface wash; indent guide hairline; diagnostics role colours; **no** backdrop blur or hard shadows | catalog + `workspace.html` |
| `tab` (slot `item` ×5) / `tabBar` (slot `root`) — no `tabList` family exists | `components/tab-strip.module.css` | 27 refs (tab 5 + tabBar 1) | item transparent, `text.muted`; hover `surface.hover` + `text.primary`; selected accent @0.15 + `accent.primary` text, pill radius; bar hairline bottom | catalog + `shell.html`, agent inspector |
| `table` (reserved kind) | — | 0 refs | reserved; no recipe family exists yet — **stay reserved** | catalog shows the empty state only |

## 3. Clay-native internal surfaces (11) and chrome primitives (8)

| Surface | Owner (current) | today | `DESIGN.md` §11/§12 target | demo |
| --- | --- | --- | --- | --- |
| `paneSplitTree` (group/pane/handle) | `shell/pane-tree.module.css` 12 · inline in `shell/PaneTree.tsx` · `coding-agent/coding-agent.module.css` 1 | 13 refs | handle = 1px hairline, hover/active `accent.primary` @0.4/0.7, focus ring, no fill at rest | catalog + `shell.html` |
| `statusBar` (slot `root`) | `app/layout/shell.module.css` 6 · `packages/package-workspace.module.css` 4 | 10 refs | inherit canvas, hairline top, mono 11px, `text.muted`; item hover `surface.hover` r8, 150ms | `shell.html`, `workspace.html` |
| `commandCentre` (slots `root`, `empty`, `status`; the palette's internals are host classes, not slots) | `command-centre/command-centre.module.css` | 15 refs | dialog r16 overlay shadow; input inset well + accent halo on focus; selected item accent @0.15; group labels micro-caps | `command-centre.html` |
| `fileBrowser` (slot `root`) | `packages/package-workspace.module.css` (SDUI tree: `src/shell/file_browser.rs` → `sdui/renderer`) | 4 refs | item selected accent @0.15 (the 2px leading bar is retired — DESIGN.md §14.13: selection is one signal, the fill); counts right-aligned mono `text.muted` | `workspace.html`, `package-workspace.html` |
| `settingsPanel` (slots `panel`, `heading`, `actions`) | `settings/settings-panel.module.css` | 15 refs | **shipped (plan 118's settings task):** fixed right slot, so veil fill + **no radius, no shadow**, boundary from the slot's divider; heading hairline bottom + `esc`/Close; actions note-then-buttons, primary last. The recipe's radius/hairline/opaque fill are spent in the drawer case (<760px). Rows are catalog components | `settings.html` |
| agent settings surface (not a recipe family — `list` rows + `empty` + host CSS) | `agent-settings/agent-settings.module.css` | **shipped (plan 118's settings task):** `list.default.row.*`, `empty.default.root.*`, `badge.*` via `ClayBadge` | one 760px-capped column: caption naming the sources, then a hairline-separated `list` row per delivered file (mono path, mono size, provenance badge); the first-run state is the `empty` recipe. No error scene — the server lists an unreadable skills directory as empty | `agent-settings.html` |
| `editorChrome` mappings (fallback naming for the `editor` family) | `editor/editor.module.css` | see `editorView` above | naming: package family is `editor.default.*`; core fallback is `editorChrome.*` — **the package key set is the contract** | as above |
| `chatPanel` / `chat` family | `chat/chat.module.css` | 38 refs | **removed** (tasks 23–24) — see §6 | none |
| `welcome`, `transientMenu`, `completion` | — | 0 refs; core fallback only, no consuming surface | not part of the 142-key package set; do not invent keys; `completion` stays an editor-internal popup painted by host CSS, and a transient menu, if one ships, must consume `menu.*` | catalog shows `menu.*` |
| `scrim`, `focusRing`, `scrollChrome`, `iconSlot` (chrome primitives) | `scrim` used via `modal`; `focusRing`/`iconSlot`/`scrollChrome` have no CSS consumer | 0 refs in modules (`scrim` 0, `focusRing` 0, `scrollChrome` 0, `iconSlot` 0) | focus ring formula = 2px `focus.ring` offset 2 (host-owned, verified in review, not a recipe key); icon 16px `text.icon` | catalog + focus states on every page |
| `badge`, `kbd`, `divider`, `tooltip` | `components/chrome.module.css` (badge 7, kbd 8, divider 3) · `components/tooltip.module.css` 9 | 27 refs | badge pill/r5 1px hairline, caption mono, semantic variants role text + role border @34%, accent @0.15; kbd veil r5 18px mono, on primary = `surface.main` @0.18 no border; divider 1px hairline; tooltip `surface.overlay` r8 hairline pop shadow `spacing.tooltip`, 100ms | catalog + `overlays.html` |

## 4. Pages and regions (`DESIGN.md` §12 composition)

| Page / region | Owner files | §12 composition target | prototype |
| --- | --- | --- | --- |
| App shell (window frame, title bar, tab strip, working area, status bar) | `app/layout/app-shell.tsx`, `tab-bar.tsx`, `working-area.tsx`, `app/router.tsx`, `app/layout/shell.module.css` | one command palette (`⌘K`), `?` keyboard map, layout state persists per surface; status bar carries workspace · connection · counts · hint row | `shell.html` |
| Workspace route + panes | `routes/workspace.tsx`, `routes/workspace.module.css`, `shell/WorkspacePanes.tsx`, `shell/workspace-panes.module.css` | sidebar (tree, filter, count footer) · editor 92ch centered · optional rail (`⌘I`); path is an on-demand strip (`⌘O`); document actions in one bar; path is text, not a control | `workspace.html` (content from `workspace-data.js`) |
| Document surface (`ClayEditor` + chrome) | `frontend/src/editor/ClayEditor.tsx`, `editor/editor.module.css` | gutter + 92ch measure, centered; no chrome literals — geometry from recipes | `workspace.html` |
| Agent surface (**landing**) | `coding-agent/CodingAgentPanel.tsx`, `coding-agent.module.css` | header (title, model, usage meter, effort) · transcript 72ch primary · composer inset well + nested island send + hint row · inspector tabs (Files, Memory, Context, Session Info, Settings); capabilities live in the inspector, not the centre | `agent-landing.html` |
| Settings | `settings/SettingsPanel.tsx`, `settings-panel.module.css`, `packages/PackageWorkspace.tsx` (slot mount) | **adopted:** the tab's fixed right slot — eyebrow-labelled collapse groups of hairline-separated rows, mono values, note-then-buttons actions row, no elevation, no nested frame; the SDUI panel in `@clay/settings` mirrors the rows and labels | `settings.html` |
| Agent settings | `agent-settings/AgentSettingsPanel.tsx`, `agent-settings.module.css` | **adopted:** one capped column, caption naming the sources, `list` row per delivered file (mono path + size, provenance badge), `empty` recipe for first run | `agent-settings.html` |
| Command centre | `command-centre/CommandCentre.tsx`, `command-centre.module.css` | the only global launcher — one opaque sheet (r16, overlay shadow) with a prompt-head, `list` rows, both empty states and a `keyHint` foot; menu origins take `popover.root` (r12, pop shadow); every action states its key in the row's mono detail | `command-centre.html`, `overlays.html` |
| Package workspace (SDUI host) | `packages/PackageWorkspace.tsx`, `package-workspace.module.css`, `sdui/registry.tsx`, `sdui/renderer.tsx` | package-delivered surfaces render through recipes, byte-identical to React counterparts | `package-workspace.html` |
| Overlay family (modal, scrim, palette, dropdown, menu, tooltip, toast) | `components/modal.tsx`, `tooltip.tsx`, `controls.tsx` | overlays never nest more than one level; transient surfaces never scroll the canvas | `overlays.html` |
| Dev fixture route (harness) | `routes/fixture.tsx`, `fixture.module.css` | not shipped UI; used to prove pre-bootstrap paint | excluded (task 10 keeps its pre-bootstrap paint correct) |
| Chat landing | `chat/ChatPanel.tsx`, `chat.module.css` | removed; the agent becomes the `empty-tab` landing | deleted (tasks 23–24) |

## 5. Baseline drift this inventory exists to close

Measured on the untouched tree (plan 118 task 1 baseline, 2026-09-11) with the
existing gate `frontend/src/test/design-system-consumption.test.ts`
(`STRICT_DS_GATE=1`):

- **54 of 142 package recipe keys are consumed by no CSS module.** Concentrated
  in: 5 of 6 `badge.*` variants, non-default `button.*` variants' disabled/focus
  (6), `card.*` (2), `collapse.header.hover` + `collapse.root`, `dropdown.item.*`
  + `dropdown.list` + `dropdown.root` (6), `flex.*`/`stack.*` (4),
  `label.*` (8), `menu.*` (6), `overlay`/`portal`/`popover` (3),
  `panel.default.header` + `panel.fixed/transient` root (3), `scroll.*` (5),
  `statusItem`, `textInput.description/error/input.hover` (3).
- **24 CSS variables are consumed but backed by neither a package recipe nor a
  `tokens.css` fallback** (collapse transitions, dropdown trigger text colours,
  `kbd` min-height, list row disabled/gap/shadow/transition, modal close/title,
  panel/tab shadows, textInput placeholder/transition).
- The drift gate is currently `it.fails` (expected-failure) with a
  `STRICT_DS_GATE=1` raw mode; tasks 16–22 close it and task 12 flips it to a
  hard assertion.
- Host pre-bootstrap fallbacks in `frontend/src/styles/tokens.css` cover only the
  consumed families (badge, button, chat, collapse, commandCentre, divider,
  dropdown, editor, fileBrowser, kbd, list, modal, paneSplitTree, panel,
  settingsPanel, shell, statusBar, tab, textInput). Families with no consumer
  (`flex`, `stack`, `overlay`, `portal`, `scroll`, `label`, `statusItem`,
  `welcome`, `transientMenu`, `completion`, `table`) have no fallbacks — and must
  not gain any until a surface consumes them.

## 6. Inventory findings that change plan tasks

1. **The 142-key set contains 12 `chat.default.*` keys with no consumer after
   tasks 23–24.** The consumption gate requires every declared key to be consumed, so
   the set cannot stay at 142 while the chat surface is deleted. Two coherent
   options: (a) ship 130 keys in `@clay/design-instrument` and update
   `tests/package_ui_conformance.rs`'s exact-count/parity assertions, or (b) keep
   12 keys with a documented exemption in the consumption gate. **Recommended:
   (a)** — dead recipe data contradicts the gate's purpose, and the chat
   replacement is deferred, not scheduled. Task 8 and tasks 23–24 state the choice;
   task 12 updates the assertions either way.
2. `label`, `statusItem`, `flex`, `stack`, `overlay`, `portal`, `scroll` recipes
   exist in the package and in core fallbacks but have **no CSS consumer today**:
   task 16 must assign owners (or the gate stays red) — they are not optional
   decoration, they are declared contract.
3. Package family names and catalog surface names differ for two internal
   surfaces (`chat`/`editor` in packages vs `chatPanel`/`editorChrome` in the
   catalog); `editorChrome.diagnostics` etc. resolve through the `editor` family.
   The **package key set** is the contract; the catalog keeps the readable names.
4. `welcome`, `transientMenu`, `completion` exist as core fallbacks but have no
   recipe family in the shipped packages and no consuming CSS module. The new
   package must not invent keys for them (bounds are enforced), and task 10 must
   decide whether their core fallbacks stay (they are reachable from the fallback
   map) or are dropped with the removed surfaces.
5. `agent-settings.module.css` (0 recipe refs, 14 theme vars) and
   `coding-agent.module.css` (1 ref, 99 px literals, 586 lines) are the two
   largest hand-painted surfaces — they cannot be fixed by adopting recipes
   alone; their layout must be rebuilt from catalog components (tasks 20–21).

6. **The documented recipe vocabulary is wider than the declared one.** The
   matrix documents `dropdown.triggerLabel`, `dropdown.indicator`,
   `list.rowTitle`, `collapse.chevron`, `panel.title`/`body`,
   `modal.title`/`body`, a `tabList` family, and internal surfaces
   `editorChrome.{gutter,indentGuide,bracketMatch,diagnostics}` +
   `focusRing.ring` with `core.*` fallback keys. No shipped package declares any
   of them, and no CSS module consumes them. Decision carried into task 8: the
   new package mirrors the **declared** 130-key set and invents nothing; a slot
   is added when a surface consumes it (the consumption gate would otherwise fail
   on dead keys). Task 15 corrects the prose that presents them as shipped.
7. **The theme contract has no scrim role.** `DESIGN.md` §6 and the recipe
   target say `surface.scrim` @0.5 + blur 3, and `modal.default.scrim` is a
   declared recipe, but `theme.css`/`src/shell/theme.rs` expose no `surface.scrim`
   role — the specimen has to compose `color-mix(in oklab, var(--c-bg) 50%,
   transparent)`. Task 13 must add `surface.scrim` (with `border.hairline`,
   `border.subtle`, `border.strong`, `accent.*`, `text.muted`), and task 14 must
   contrast-check it against the canvas.

## 7. Prototype coverage table (input to tasks 3–7)

Every state below is demonstrated by exactly one of four means, and the verifier
in `design-artifacts/tools/capture-prototypes.mjs` asserts the claim rather than
trusting it:

- **scene** — a state the page can show on demand (the review bar's state
  switcher, or `?scene=<id>` for scripted capture).
- **dom** — the markup is in the page and visible in the default capture.
- **behaviour** — the verifier drives the affordance and measures the result
  (typing into the palette, filtering the tree).
- **catalog** — a per-component state reviewed in `component-catalog.html`'s
  7-state matrix (rest, hover, active, focus, selected, disabled, invalid) or a
  component behaviour that belongs to React Aria, not to this design language.

| Artifact | Surfaces | States (how demonstrated) | Themes | Widths |
| --- | --- | --- | --- | --- |
| `start.html` (task 4 / Part D) | the launcher: recent workspaces, available agents, both filters, the action row, the first-run state | recently used, workspace picked, agent picked, both picked (**scene**, driven by the page's own selection code), first run (**scene**, no recents); picking and filtering are **behaviour** (asserted) | 4 shipped | both |
| `component-catalog.html` (task 3) | every kind in §2, every surface in §3, every primitive in §3 | 130 specimens; all 7 states per interactive kind with `data-fx` forcing (**dom**); live arrow-key tabs (**behaviour**) | 4 shipped | 1500×950 + 1024×800 |
| `shell.html` (task 4) | app shell, the workspace tab strip (one tab per folder, plus `+`), the tab's two-view switcher, pane split tree, status bar | empty tab + focused pane + the switcher (**dom**); dragged split (**catalog**) | 4 | both |
| `workspace.html` (task 4) | sidebar tree, editor + chrome, rail, on-demand path strip, entry walk | default (**dom**); tree filter (**behaviour**); no-headings rail, read-only and no-workspace are *not built* — the derived approved screen has one state (§9) | 4 | both |
| `agent-landing.html` (task 4) | the agent *view* of a tab: agent-type picker, transcript, composer, inspector tabs, session files, skills/MCP in Context | first-run, running, error, resumed, empty session, disconnected (**scene** ×6); "cancelled" is the running scene's Esc affordance, not a separate state; the composer's single-ring/inside-controls layout and the Files tab's session history are **layout** assertions | 4 | both |
| `settings.html` (task 4) | settings panel fields (text inputs, selects, toggles, groups) | invalid field and the not-yet-valid action bar (**dom**); disabled control (**catalog**). Shipped evidence (plan 118): `design-artifacts/screenshots/quiet-instrument-settings/` — 12 panel captures (4 themes × 1500/1024/760) + the expanded Typography group; the panel is a slot at 340/312px and a 480px drawer, with no shadow | 4 | both |
| `agent-settings.html` (task 4) | the agent's Settings tab: delivered files, sizes, provenance badges | delivered (**fixture `agent-settings`**) and nothing delivered (**`&state=empty`**); the unreadable scene was not adopted — the server treats an unreadable skills directory as an empty listing (`src/server/agent_settings.rs`), so the product cannot reach that state | 4 | both |
| `command-centre.html` (task 4) | palette dialog, input, result groups, empty state, status row | results (**dom**); no-results (**behaviour**); querying and keyboard-selected row (**catalog**). Shipped evidence (plan 118): `design-artifacts/screenshots/quiet-instrument-overlays/` — 32 captures (palette × 4 themes × 1500/1024/960, plus empty/path/menu scenes, the modal sheet and the tooltip); the sheet is r16 with the one focus ring, a menu origin is r12, and the prototype's scopes/groups are **not** fabricated (the server sends neither) | 4 | both |
| `package-workspace.html` (task 4) | SDUI panel/flex/stack/overlay surfaces, file browser, status bar | selected row and the failed-verification card (**dom**); hover (**catalog**). The host slot language shipped with plan 118's SDUI task (`registry`/`renderer`/`package-workspace`); the prototype's trust-domain *page* is not shipped, and its `panel.fixed` root resolves to the same values as `panel.default`, so it stays declared-and-unconsumed | 4 | both |
| `overlays.html` (task 4) | modal + scrim, dropdown + popover, menu, tooltip, toast | full veils and reduced transparency (**scene** ×2); dismissal and focus trapping are component behaviour (React Aria), not staged. Shipped (plan 118): the modal is a sheet (head/body/hairline foot, cancel leading, primary trailing), the dropdown popover, `/`/`@` menu (incl. pressed) and tooltip follow the same recipes; the **toast** is declared and unconsumed — nothing emits one, so it stays recorded in the adoption backlog | 4 | both |
| `themes.html` + `theme-values.md` (task 5) | every text/UI pair per theme, hairline ladder, accent roles | current vs proposed per theme with the composited ratio (**dom**); the core-fallback sample (**dom**) | 4 shipped | wide |

## 7.1 Delivered pages (task 4)

Eight pages, one per surface. Two are **derived**: their composition, CSS and
behaviour are the approved artefacts, wrapped in the review frame and extended
only with states a static capture cannot reach. Six are **authored** against the
same `ds-quiet.css` + `components.css` language, with content read from the
repository (manifests, the inventory, the file tree snapshot, real byte sizes)
rather than typed in.

| Page | Kind | Surfaces | States it shows | Data source |
| --- | --- | --- | --- | --- |
| `shell.html` | authored | title bar (brand, tabs, actions, window buttons), tab strip, two-pane split with handle, file tree, empty working area, status bar | tab active / inactive / dirty, tree dir/file/selected, split handle rest, empty working area with the keyboard path | `workspace-data.js` (38 files, 99 tree rows) |
| `workspace.html` | **approved** | file tree, document bar with real badges, 92ch editor with gutter, on-demand path strip, outline rail with facts | clean / modified badges, outline entries active, path strip hidden until ⌘O, open-path tones idle/ok/err, undo/redo availability | `workspace-data.js` (real `VENT.md`) |
| `agent-landing.html` | **approved** + scenes | landing surface: header (model, effort, context meter), 72ch transcript, composer with `/` `@` menu, 5-tab inspector, palette overlay, keyboard map overlay | `landing` (first run), `running`, `error`, `resumed`, `empty-session` | approved screen + the daemon's own session message |
| `settings.html` | authored | appearance and typography groups, design-system list, actions, status bar | group open / closed, theme dropdown open with the 4 shipped palettes, selected appearance, invalid size field, disabled Apply | 4 theme packages, 3 design systems, real setting labels |
| `agent-settings.html` | authored | the agent's Settings tab: tab strip + delivered-file list with provenance badges | list, loading, empty, built-in vs edited, long name truncation | `packages/*` skills, real sizes |
| `command-centre.html` | authored | palette (scopes, groups, rows, empty state, footer hints), path scope strip | results, selected row, live filter, no-match empty, scope groups, path entry | real command titles and ids from the package manifests |
| `package-workspace.html` | authored | package list (roots + helper), detail pane with facts, files and contributions, trust states | selected row, helper (not loadable), removed by plan 118, fingerprint mismatch | `bundled-inventory.toml` + `packages/*/package.json` + real file sizes |
| `overlays.html` | authored + scenes | modal, dropdown, context menu, tooltip, toasts, scrolling panel | `rest` (translucent veil + blur) and `reduced-transparency` (solid fill, no blur), danger menu row, disabled menu row, three toast tones | the approved screens' own strings (status, toasts, commands) |

Every page is checked at 1024×800 and 1500×950 in all four shipped themes, with
`?theme=<id>&width=narrow|wide` for scripted captures and a strip that carries
the same switches. `tools/make-pages.py` regenerates all eight and refuses to
write if any page claims a recipe key no package declares or leaves a component
element without its `data-clay-ds` key.

**Two findings the pages produced.**

1. **Surfaces with no recipe family.** The language styles controls the recipe
   contract has no key for: the toast (the kit creates one on every screen), the
   segmented control (`.seg`), the chip, the empty state, the status dot, the
   shortcut vocabulary around a key chip (`.hint`, `.keys`, `.key-row`), the
   theme swatch, and the agent's stat rows (`.stat*`, `.kv`). Either task 8
   declares them or the migration unifies them with an existing family — the
   chip and the badge are already the same idea twice.
2. **A banned pattern survived into the approved language.** `ds-quiet.css`
   draws a fixed film grain on `.desk` (`--grain-opacity: 0.02`), which
   `DESIGN.md` §14 bans and which no product surface should carry. The pages drop
   the `.desk` backdrop entirely (the review frame replaces it), and task 16
   must delete the rule rather than port it into the host CSS.


## 7.2 Theme-value board (task 5)

`themes.html` puts all four shipped themes on one page — current value next to proposed, for every
role the language needs — because theme values are the one thing that cannot be reviewed one theme
at a time. `theme-values.md` is its specification: a copy-paste `clay.contributions.designTokens`
block per theme, every pair measured, and the findings. Both are generated from
`theme-values.json` by `tools/make-theme-values.py`, which also exits non-zero if a floor is missed,
so the board cannot approve values the gates would reject.

| Theme | Ladder (hairline → subtle → strong, composited) | Boundary `border.subtle` on canvas, today → proposed | `text.disabled` today → proposed | Worst state fill vs its own text, today → proposed |
| --- | --- | --- | --- | --- |
| Modus Operandi | 1.45 → 3.59 → 21.00:1 | 1.84 → 3.59:1 | 7.00 → 5.33:1 | 2.94 → 14.73:1 |
| Modus Vivendi | 1.39 → 4.18 → 21.00:1 | 1.76 → 4.18:1 | 6.86 → 6.49:1 | 3.22 → 9.32:1 |
| Gruvbox Material Dark | 1.64 → 4.47 → 9.07:1 | 2.74 → 4.47:1 | 3.37 → 6.18:1 | 1.07 → 5.63:1 |
| Gruvbox Material Light | 1.47 → 3.67 → 7.82:1 | 2.06 → 3.67:1 | 3.37 → 5.21:1 | 1.03 → 5.97:1 |

Twelve enforced pairs per theme, all passing after the proposal; the ratios are composited (alpha
over the surface it is drawn on), which is what the eye sees and what the runtime gate does not do
yet. The board also records the core-fallback sample, the depth direction per theme (`DESIGN.md`
§10.2) and the two places where the shipped packages disagree with the approved palette — the
Gruvbox Material Dark canvas inversion (a two-value `textStyles` swap, no token) and the fact that
today every accent-driven role resolves to the monochrome caret.


## 8. Verification and evidence (task 6)

The whole set is asserted in one headless-Chrome pass, then the runs that are
review evidence are written to `screenshots/`:

```bash
# any Chrome works; the path is printed in the report
CHROME=/usr/bin/google-chrome node design-artifacts/tools/capture-prototypes.mjs
# asserts without writing 10 MB of PNGs
node design-artifacts/tools/capture-prototypes.mjs --no-shots
# one page/theme/state while iterating on a prototype
node design-artifacts/tools/capture-prototypes.mjs --only=agent-landing:modus-vivendi:running
```

| Result (2026-09-11, Chromium 1223, `file://`) | Value |
| --- | --- |
| Matrix entries | **11**: the 10 page prototypes (`start`, `shell`, `workspace`, `agent-landing`, `settings`, `agent-settings`, `command-centre`, `package-workspace`, `overlays`, `themes`) plus the component catalog |
| Runs asserted (11 entries × 4 themes × 2 widths, + 24 scene runs) | **112** |
| Console errors, script errors, failed requests | **0** |
| Remote requests (every resource resolved `file://`) | **0** |
| Horizontal overflow, clipped boxes, at either width | **0 px / 0** |
| Distinct page × theme canvases (each page paints its own theme's surface) | **40 / 40** |
| Screenshots stored (79 PNGs, ~9.7 MB) + `screenshots/report.json` | `screenshots/` |
| Behaviour claims driven and measured | tree filter 99 → 3 rows; palette no-results hidden → shown; launcher pick → "Open clay"; launcher filter 3 → 1 rows |
| Agent-file sizes checked against `.agents/skills/*/SKILL.md` on disk | 156/156 rows match |

**Every capture is the state as loaded** — the screenshot is taken before any
behaviour probe runs, so a PNG can never show the prober's own typing or picks.

**Screenshot naming.** `<page>__<theme>__<width>__<state>.png`, e.g.
`agent-landing__gruvbox-material-light__1500x950__disconnected.png`. Every run is
*asserted*; the stored sample is every page in every theme at 1500×950, every page
at 1024×800 in Gruvbox Material Light (the theme that exposes hairlines hardest),
and each non-default scene in Modus Vivendi and Gruvbox Material Light. The
narrow-width and remaining scene captures are skipped to keep the evidence under
15 MB — the assertion is the gate, the PNGs are for review.

**A caution the verifier learned the hard way:** `Page.loadEventFired` can belong
to the *previous* document, and a run that trusts it measures a half-built page —
in this set it silently reported another page's clipped boxes, an empty file tree
and a wrong scene. It now confirms the navigation committed (`document.URL`) and
that the theme resolves before it probes anything, and it captures the screenshot
*before* any behaviour probe can type or click.

**What the verification does not claim.** The prototypes are review material:
they carry no authority, and nothing in `design-artifacts/prototypes/` is binding
until task 7 freezes an approved copy. They are static HTML with no build step —
the in-page switchers and state forcing exist for capture, not as product
behaviour. The pages fetch nothing: no font, no script, no image, no analytics,
every resource is a sibling file, and the run fails if a single request escapes
`file://`. Nothing is invented to look real either — the file tree and document
bodies come from `workspace-data.js` (generated from the repository by
`tools/make-workspace-data.py`), package names, versions, roots and contribution
keys come from `packages/*/package.json` and `src/packages/bundled-inventory.toml`,
the agent-file sizes are read from `.agents/skills/*/SKILL.md` at generation time
*and* re-checked against disk at verification time, and the theme values are the
themes' own approved palettes. The only authored content is UI copy for states
the application has no words for yet — which is exactly what review is for.

**Delivered so far.** `component-catalog.html` (task 3) renders 130 specimens
over 49 families — every declared key exactly once, labelled with its key, on the
four shipped themes — generated from the package contract by
`design-artifacts/tools/make-component-catalog.py`. It consumes the language
stylesheet verbatim (`ds-quiet.css`, copied from
`design-artifacts/approved/quiet-instrument-language/`), a four-theme `theme.css`
(the two proposal-only palettes dropped) and a trimmed `ds.js` (theme switcher +
`?theme=<id>` + tabs + toasts). Its own `<style>` block is split into
**Section A, specimen scaffolding** (matrix layout, state forcing via
`data-fx`, key labels) and **Section B, language extensions proposed by this
catalog** — the surfaces `DESIGN.md` §11 specifies but the approved screens never
had to draw: disclosure, tooltip, split handle, invalid input, type roles, badge
roles, card, tab strip, panel variants, long control labels, editor chrome
(promoted from the approved workspace screen), plus the overlay/portal/scroll
structures. Section B is the part approval blesses; Section A is never binding.

## 9. Findings from the prototype set (tasks 3–6)

What building and verifying the set proved about the approved artifacts, the
language and the contract. Each one is either already carried by a plan task or
is a decision the approval in task 10 has to make.

1. **The approved language carries a pattern `DESIGN.md` §14 bans.** `ds-quiet.css`
   defines a film-grain overlay on `.desk` (`--grain-opacity: 0.02`). The review
   frame drops the desk, so no page shows it, but the rule is still in the
   approved stylesheet. Task 16 deletes it; it must not be ported into the
   product stylesheet.
2. **One step, two names.** `--c-line` and `--hairline` measure identically on all
   four themes and `--c-line` has no consumer left in the language. `border.hairline`
   replaces both (task 16).
3. **`surface.scrim` exists as a role with a core fallback but has no theme
   projection**, so a legacy theme dims with the core catalog's colour rather than
   its own (tasks 13–14).
4. **The runtime contrast gate does not composite alpha.** `contrast_ratio` reads
   `to_rgba8()` and ignores the alpha byte, so a 34 % hairline scores 21:1 while it
   renders at 1.45:1. Without the fix the new `border.subtle` floor is decoration
   (task 14). The theme board measures composited for exactly this reason.
5. **Accents are not accents today.** Every accent-driven role resolves to the
   monochrome caret, and the fills behind hover/active/selected are one 40 %-alpha
   selection colour (1.03:1 against its own text in Gruvbox Material Light).
   The core fallback catalog is not language-compliant either (boundaries
   1.3–2.0:1) — it is the design-system-less baseline, not a validated theme
   (tasks 13–14).
6. **Gruvbox Material Dark inverts depth.** It ships `shellBg` #1d2021 and
   `panelBg` #282828, so `surface.main` resolves to the chrome colour and the panel
   renders lighter than the canvas — the "inverted panel" §10.2 forbids. A
   two-value `textStyles` swap is the whole fix (task 13).
7. **The derived screens have exactly one state each.** The workspace page is the
   approved screen verbatim, so "filtered", "read-only", "no workspace" and the
   empty rail are not in it; the agent page's five states are prototype scenes
   because the approved screen only shows the landing state. Building the workspace
   states would mean inventing markup the approved artifact does not contain, which
   is a design decision, not a prototype fix.
8. **`agent-settings.html` is the agent's *Settings tab*, not a controls page.**
   It lists the delivered files with their real sizes and provenance badges. The
   plan's name ("agent settings, theme choice list") suggests model/effort/theme
   controls; those live in `settings.html` and the agent header. If approval wants a
   controls page here, that is a new artifact.
9. **The review frame's state switcher could not return to a default state that had
   no block of its own** (`setScene` only accepted block ids), so "First run" was a
   one-way door and `?scene=landing` silently did nothing. Fixed in `ds.js` — a
   prototype that cannot show its own default state is worse than no prototype.
10. **Surfaces with no recipe family** (from the task-4 gap analysis): `toast`,
    chip, the segmented control, the empty state, the status dot, the shortcut
    vocabulary (`.hint`, `.keys`, `.key-row`), the theme swatch and the agent stat
    rows. Task 8's acceptance criteria require the 130-key set to account for them
    explicitly (either a family or a documented host-CSS owner).
11. **The approved composer drew two highlighted borders and hung its controls
    outside the field.** The shell's accent ring plus the inner textarea's own
    `:focus-visible` outline violates §14.4 (one boundary per surface), and the
    send button floated beside a centred 74ch field. Corrected in the agent page
    (one ring on the shell, field spanning its zone, controls inside its trailing
    edge); tasks 8/21 encode the correction in the package and host CSS.
12. **The film grain is on the window, not just the presentation desk.**
    `ds-quiet.css` paints `--grain-opacity` on `.win::after` as well as on `.desk`,
    so every approved screen carried a §14.7 violation on the product surface
    itself. The prototype set suppresses it (documented correction) and task 16
    deletes both rules.
13. **Layout preferences leaked between surfaces.** `ds.js` inferred sidebar and
    inspector state from the DOM on unload, so a surface with no sidebar (the
    launcher) wrote `collapsed` and the *next* surface inherited a collapse that
    won over its own grid — the shell rendered its working area 24px wide. Fixed
    by saving only what the user actually toggled; the same class of bug will bite
    the real tab store, which is why task 33 persists layout per tab.
14. **The four new families have no specimen in the catalog yet.** `viewSwitch`,
    `agentPicker`, `recentRow` and `sessionRow` are specified in `DESIGN.md` §11 and
    reviewable in the pages (selected rows, both switcher states, the picker menu,
    populated and empty session files), but the catalog's specimen matrix is
    generated from the *declared* contract, so it cannot show a family no package
    declares. Task 8 adds the specimens with the recipes — not by hand-writing keys
    into a prototype.
15. **A one-agent assumption is already load-bearing.** The agent view's title,
    the `coding` chip and the launcher's agent pane all assume exactly one agent
    type, and the Files tab doubles as a workspace browser because a tab had no
    other way to reach a file. Both are Part D work (tasks 35 and 36).
16. **Every page still paints its own geometry when a theme changes.** The
    verification asserts 40/40 distinct page × theme canvases and one hairline
    ladder per theme — the recipes and the theme values are decoupled, which is the
    property most likely to break silently during the migration.
