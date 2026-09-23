# UI Design System Conformance & Replacement Proof

## 1. Overview and Invariant Contract

This document records the conformance proof and verification contract for package-defined UI design systems in Clay (Plan 104 Task 5).

Clay enables full UI design system replacement purely through declarative data contributions (`clay.contributions.uiDesignSystem`), with zero JavaScript execution and zero host component branching.

### Core Conformance Invariants

| Invariant | Requirement | Verification Method |
| --- | --- | --- |
| **DOM & State Continuity** | Switching between the shipped choices (`@clay/design-instrument` and the `@clay/core` baseline) MUST NOT remount the DOM or reset node identity, user input, focus, scroll, selection, or disclosure states. | `frontend/src/test/design-system-conformance.test.tsx` (parameterized over both choices) |
| **Zero Host Branching** | Host components and backend code MUST NOT branch conditionally on design-system package specifiers. | `tests/package_ui_conformance.rs` (`plan118_source_independence_guard_rejects_package_name_branching`) |
| **Color Authority** | Design systems MUST NOT declare concrete color palettes or literal hex/RGB/HSL colors. All colors trace to active content-theme tokens (`var(--clay-*)`). | `tests/package_ui_conformance.rs` & `tests/theme_packages.rs` |
| **Theme Orthogonality** | Switching active content themes recolors the UI instantaneously through CSS custom properties with zero design system recipe modifications. | `frontend/src/test/design-system-conformance.test.tsx` |
| **Design-Language Conformance** | The default design system conforms to [`DESIGN.md`](../../DESIGN.md): no 0px control radii, no hard offset shadows, no blur outside scrim/toast, accent only in state recipes. | `tests/package_ui_conformance.rs` (plan 118 suite) + `docs/development/ui-design-system-visual-direction.md` review evidence |
| **Performance Safety** | Large scroll viewports, editor typing canvas, and line gutters MUST NOT apply backdrop filters (`backdropBlur == 0.0`). | `tests/package_ui_conformance.rs` (`plan118_design_instrument_radius_state_and_material_discipline`: blur allow-listed to scrim + toast) |
| **Clean Revocation** | Revoking or disabling an active design system cleanly restores default/core CSS variables with no orphaned properties. | `frontend/src/test/design-system-conformance.test.tsx` |
| **Accessibility Fallbacks** | `prefers-reduced-motion: reduce` collapses every transition to instant and removes transforms; `prefers-reduced-transparency: reduce` resolves each veil to an opaque theme surface and stops every backdrop filter. | `frontend/src/test/design-system-conformance.test.tsx` + `frontend/src/styles/global.css` |
| **Theme Contrast Gate** | Every required foreground/background pair must be measured *composited* (alpha over its backdrop) and clear its floor: prose 4.5:1, affordance/boundary/state fill 3.0:1, hairline 1.2:1 visibility. A failing pair rejects activation atomically, and a role a theme omits falls back to the core catalog and is measured there. | `src/shell/theme.rs` (`theme_meets_contrast`) + `tests/theme_packages.rs` + the published table in `docs/reference/ui-design-systems.md` §7 |
| **Catalog Currency** | The catalogs and the recipe matrix must not present a removed design system as shipped, must mark every host slot the shipped package declares no recipe for (`†`), and must not claim a retired pattern is part of the language. | `tests/package_ui_conformance.rs` (`plan118_recipe_matrix_marks_undeclared_slots`, `plan118_catalog_pages_do_not_ship_removed_systems`); the component tests read both catalogs |
| **Consumption Coverage** | Every declared recipe key is consumed by its recorded owning host stylesheet (module or `global.css`), every consumed variable is backed by a fallback or recipe, and the not-yet-adopted remainder is an exact, shrinking recording. | `frontend/src/test/design-system-consumption.test.ts` + `frontend/src/test/fixtures/design-system-adoption-backlog.json` |

---

## 2. Package Replacement Verification Matrix

The shipped design system (`@clay/design-instrument` — the approved Quiet
Instrument language) and the built-in `@clay/core` baseline are **one language**,
not two: the baseline is the host-consumed subset of the package's recipes (plan
118 task 16), so for every key it declares its resolved values equal the package's
(asserted by `plan118_core_fallbacks_match_the_shipped_language` and
`plan118_host_fallback_block_matches_the_resolved_package`) and it declares nothing
else. Switching to `@clay/core` is therefore a change of *coverage*, not of look —
the regions with no baseline key fall through the deterministic chain to the same
ladder and tiers.

The conformance surface is 16 SDUI component kinds (`ComponentKind`,
`src/shell/components.rs`), the reserved `table` kind, 11 Clay-native internal
surfaces and 8 chrome primitives — 36 rows in
`docs/development/ui-design-system-recipe-matrix.md`, where `†` marks a host slot
the shipped package declares no recipe for. The baseline's deliberate deviations
are the full-bleed flush roots (`dropdown.root`, `fileBrowser.root`, `flex.root`,
`grid.root`, `label.root`, `modal.root`, `modal.scrim`, `paneSplitTree.root`,
`scroll.root`, `statusBar.root`, `tabBar.root`), allow-listed by the no-0px-radius
invariant, plus `textInput.default.root` — a fallback-only bordered shell the
package does not need because its own `field`/`input` slots carry the well.

The *replacement* proof does not rest on the baseline looking different. A
synthetic third-party fixture dresses the same DOM in deliberately different
geometry and material and must project its own `--clay-ds-*` variables with zero
host branching (the frontend suite below); the removed
`@clay/design-neobrutal`/`@clay/design-glass` packages, whose contrasting
treatments this table used to record, are gone (plan 118 task 9).

---

## 3. Automated Test Coverage

### Rust Integration & Conformance Tests
- **`tests/package_ui_conformance.rs`**:
  - `plan118_source_independence_guard_rejects_package_name_branching`: Static source scan confirming zero conditional branches on package specifiers in host source code.
  - `plan118_design_instrument_covers_required_components_and_enforces_color_authority`: Asserts complete recipe presence for the required component kinds (24 — the chat surface is gone) and literal color absence.
  - `plan118_core_fallbacks_obey_the_language_rules` / `…_match_the_shipped_language` / `plan118_host_fallback_block_matches_the_resolved_package`: the `@clay/core` baseline is the host-consumed subset of the shipped language — same radii, motion tiers, elevations and state language, with zero value disagreement against `packages/design-instrument/package.json` or the `tokens.css` fallback block.
- **`tests/theme_packages.rs`** (plan 118 tasks 13–14, content themes as colour authority):
  - `shipped_theme_roles_match_the_approved_board` / `…_obey_the_language`: the four shipped themes declare the
    approved board's 13 UI roles with the board's values verbatim, and resolve them through the paint path
    (`resolve_theme_token_snapshot` → `--clay-*`) — not from a core fallback substitute.
  - `shipped_theme_border_ladder_is_monotonic`: `border.hairline < border.subtle < border.strong` measured
    composited on both the canvas and the panel, for every shipped theme.
  - `shipped_theme_invisible_boundary_is_rejected` / `…_invisible_hairline_is_rejected` / `…_accent_below_the_floor_is_rejected`:
    mutating a shipped `border.subtle`, `border.strong`, `border.hairline` or `accent.primary` to a value that
    cannot be seen (its own canvas, or an opaque hairline in the canvas colour) is rejected by pair name, with
    the measured ratio, before install.
- **`src/shell/theme.rs`** (in-crate): `core_catalog_meets_every_required_contrast_pair` (the core catalog is
  measured against the same policy — it is the palette painted before any snapshot, and the only path with no
  runtime activation), `host_theme_role_block_mirrors_the_core_catalog` (the `--clay-*` block in
  `frontend/src/styles/tokens.css` equals the catalog value per role, so the shell cannot repaint on the first
  frame), and `composited_contrast_measures_alpha_over_the_backdrop` (the WCAG engine's alpha semantics).

- **`tests/package_ui_conformance.rs`** (plan 118 suite):
  - `plan118_design_instrument_is_inert_data_with_the_quiet_instrument_profile`: Asserts the shipped package loads as inert data inside every declared bound (values, payload, no literals, no executable text).
  - `plan118_design_instrument_radius_state_and_material_discipline` / `…_keys_are_the_reference_set_minus_chat_plus_the_new_families` / `…_rejects_mutated_probes`: Assert the radius ladder, blur allow-list, focus offsets, the 165-key set, and that mutated declarations fail closed.
  - `plan118_removed_design_systems_are_absent`: Asserts the removed Neobrutal/Glass specifiers appear nowhere in `packages/`, `src/`, `frontend/src/`, `tests/fixtures/`, or `scripts/`.
  - `plan118_recipe_matrix_marks_undeclared_slots`: Recomputes the declared `(family, slot)` set from `packages/design-instrument/package.json` and asserts `docs/development/ui-design-system-recipe-matrix.md` marks exactly the undeclared slots with `†` — a missing or stale marker fails, and the parity of the generated index in `components.md` is checked against the same set.
  - `plan118_catalog_pages_do_not_ship_removed_systems`: Scans the catalog pages for the removed specifiers and for prose that presents a removed system as current; a mention is allowed only on a line that marks it as removed/former/historical/superseded, or on a page designated a historical record.

### Frontend Conformance Suite
- **`frontend/src/test/design-system-conformance.test.tsx`** — parameterized over
  the shipped choice set (`@clay/design-instrument`, `@clay/core`; the baseline is
  "no adopted snapshot", so its half of the matrix proves the host fallback block
  paints the same language):
  - Validates CSS custom property projection (`--clay-ds-*`) without literal colors.
  - Proves DOM node identity, user-typed buffer text, focus, scroll offset, selection
    and disclosure state survive live switching without remounts, and that controls
    stay operable afterwards.
  - Proves content-theme token updates recolor the UI without modifying design-system recipes.
  - Proves clean variable revocation on reset, and that the core baseline paints the
    language's geometry (8px controls, 16px surfaces) afterwards.
  - Proves the shipped system and a synthetic third-party system both project validated
    `--clay-ds-*` variables, and that a third-party swap changes geometry without
    touching the DOM.
  - Proves the focus contract (2px ring at offset 2 on the focused element; no second
    ring on a nested control), `prefers-reduced-motion` collapsing motion to instant
    and `prefers-reduced-transparency` resolving veils to opaque fills — the latter by
    reading the component/slot attributes off a rendered modal and an opened dropdown,
    so no selector in `global.css` can rot into an unmatchable rule.
- **`frontend/src/test/design-system-consumption.test.ts`** — the drift gate:
  every declared recipe key must be consumed by its recorded owning module
  (`COMPONENT_CSS_OWNERSHIP`, from the surface inventory), every consumed
  `--clay-ds-*` variable must be backed by a `tokens.css` fallback or a package
  recipe, and every unmapped family must fail. The scan covers every host
  stylesheet (`*.css` under `frontend/src` except `styles/tokens.css`, which
  *states* the fallback values rather than consuming them) — the universal scroll
  chrome in `styles/global.css` is a consumer like any module. The not-yet-adopted
  remainder is recorded by exact equality in
  `frontend/src/test/fixtures/design-system-adoption-backlog.json`: the host-CSS
  adoption tasks shrink both lists to empty (then delete the file), and any new
  drift fails immediately — the gate is hard, never `it.fails`.

- **`frontend/src/test/workspace-composition.test.tsx`** — the shell/Workspace
  composition gate: the rail's facts and outline come from the open document's
  real metadata, the rail toggles without remounting the editor, every document
  action sits in the one document bar with the path field on demand, the 92ch
  measure and the 340/312px rail are host geometry with token overrides, the
  chrome strips are mono/tabular with recipe hairlines, and none of the three
  surfaces blurs, filters or animates. `src/shell/shell-chords.test.tsx` covers
  the `Ctrl+I` rail chord.
- **`frontend/src/sdui/surface-adoption.test.tsx` and the agent composition
  evidence** — the agent view of a tab is the composition gate at page scale: the
  transcript is a 72ch measure of hairline-separated turns with one clamped inset
  well for machine output, the composer draws exactly one boundary (the well, not
  the inner control), the inspector is 340/312px and a drawer below 1000px, and
  the meter/context bars come from the stat-row family. `CodingAgentPanel.test.tsx`
  holds the same claims at unit scale (34 tests: turns and their roles, the Context
  tab's capability data, the foot's branch/extensions/MCP segments, the effort
  chord and the composer intents). The same file also scans the editor,
  package-workspace, settings and agent-settings stylesheets for hardcoded border
  or shadow geometry, and asserts the settings panel's fixed-slot chrome (no
  radius, no elevation, recipe radius/hairline/opaque fill only in the drawer). The measured browser pass lives in
  `design-artifacts/screenshots/quiet-instrument-agent/report.json` — 12 captures
  (4 themes × 1500/1024/960) recording the measure in px, the separator count, the
  clamped box height, the composer boundary count, the inspector geometry, 0px
  horizontal overflow and the absence of blur/filter/animation.
- **`frontend/src/test/settings-panel-choices.test.tsx` and the settings
  composition evidence** — the settings surfaces are the gate for *slot* chrome:
  the panel is the tab's fixed right slot (340px, 312px ≤ 1240px, a drawer below
  760px), its groups are eyebrow-labelled `collapse`s of hairline-separated rows,
  values that are data carry the mono role, the only framed control is the input
  well (no panel inside the panel, no elevation on a fixed slot), and the actions
  row states what the commit will do with its primary last. The same file holds
  the choice-set contract: theme and design-system dropdowns project the
  server-enumerated `ui_choices` and nothing else. `agent-settings/AgentSettingsPanel.test.tsx`
  holds the narrowed twin (delivered-file rows as `list.default.row`s, mono sizes,
  provenance badges, designed empty state). The measured browser pass lives in
  `design-artifacts/screenshots/quiet-instrument-settings/report.json` — 12 panel
  captures (4 themes × 1500/1024/760) plus the delivered/empty agent-file scenes,
  recording the slot geometry, the absence of a shadow, the section/row counts,
  the hairline count, the mono-value count, the dropdown inventory and 0px
  horizontal overflow.
- **`frontend/src/test/overlay-composition.test.ts` and the overlay-family
  evidence** — the floating surfaces are the gate for elevation and fallbacks:
  `box-shadow` may only appear in the closed allowlist of transient owners (a
  static region that gains one fails), the overlay modules carry no literal
  border radius/width/outline/duration, the palette is bounded with a scrolling
  result list, the sheet is the one focus ring (input outlined `none`), the modal
  renders head + scrolling body + hairline actions foot (`space-between`, cancel
  leading, flush mode painting no second head), and the reduced-motion /
  reduced-transparency blocks collapse transitions and resolve every veil (scrim
  → canvas, veil surfaces → opaque overlay plane, no blur left). The measured
  browser pass lives in
  `design-artifacts/screenshots/quiet-instrument-overlays/report.json` — 32
  captures (palette × 4 themes × 1500/1024/960, plus the empty, path and menu
  scenes, the modal and the tooltip), recording the radii (16/12/8), the
  hairline separations, the key-hint and result counts, the focus boundary
  against the theme's accent plus the halo, the fabrication check (no scope
  control the server did not send), the scrim blur and 0px horizontal overflow.
- **The agent-type picker (plan 118 task 35)** — the agent view's title is the
  picker, and it is the gate for the family's consumption: the trigger carries
  `data-clay-component="agentPicker"`/`data-clay-slot="trigger"` and
  `frontend/src/coding-agent/coding-agent.module.css` references every declared
  `agentPicker.default.trigger.*` property (rest fill/radius/colour/transition,
  hover, expanded = accent tint + accent text, the focus ring, disabled), which is
  what removed the five keys from the adoption backlog.
  `frontend/src/coding-agent/CodingAgentPanel.test.tsx` pins the picker's claims at
  unit scale (title = current agent, only server-listed types, current marked,
  provenance note, no fabricated row, the pick handed to the shell and the current
  row a no-op, inert without a shell) and the transcript's per-turn attribution
  (each turn labelled with its producer, exactly one switch note, no labels while
  one agent wrote the transcript). The switch itself is server-owned:
  `clay-agent/src/__tests__/agent-types.test.ts` (per-session config root,
  containment, in-place switch, record metadata, entry stamps), `src/server/launcher.rs`
  (the name rule), `src/server/agent_settings.rs` (per-agent reads contained),
  `src/server/tab_registry.rs` (`set_agent_type` binding), and the per-agent book
  cases in `src/server/agent.rs`.
- **The workspace sidebar head (plan 118 task E1)** — the server's file listing
  carries a filter descriptor (`SduiNodeKind::List.filter`, core trees only) and
  the sidebar region names the host token it is sized from
  (`SduiNode.size = "dimension.sidebar.default"`, host CSS maps it to 244px /
  224px ≤1240px). The host renders the approved tools row (shipped input well +
  `/` chip) and foot (live match count + move/open hints) and filters the
  delivered bounded rows locally. Gates: `src/shell/file_browser.rs`
  (the tree carries the descriptor and the size token),
  `src/protocol/sdui.rs` (both fields are optional: a tree written before them
  still parses), `frontend/src/test/components.test.tsx` (the filter narrows
  rows, the count follows, Enter opens the first match, Escape restores, one
  ring), `frontend/src/test/renderer-size.test.tsx` (the renderer marks the
  sized region and hands the descriptor to the catalog list) and
  `design-artifacts/tools/capture-sidebar.mjs` →
  `design-artifacts/screenshots/quiet-instrument-sidebar/report.json` (244px at
  1500, 224px at 1024, filtering 3 → 1 → 0 → 3 with the count, 0px overflow).

- **Per-tab visibility and the rail's scroll-spy (plan 118 task E2)** — the rail
  and the agent inspector are per-tab layout state: one store keyed by the tab's
  clientId (`frontend/src/shell/layout-state.ts`), mirrored into `layout.json` by
  the workspace controller, written back per tab (`PersistedTabState::rail_visible`
  / `inspector_visible`, absent = visible so older documents load) and restored on
  boot. The rail's outline marks the entry the reader has reached, reading the
  editor session's viewport line (`DocumentSession::topVisibleLine`, with the
  approved 48px reading inset) at most once per animation frame, and an explicit
  jump locks the spy for the prototype's 800ms window. Gates:
  `frontend/src/shell/workspace-controller.test.ts` (per-tab values, the toggle's
  own write, restore), `frontend/src/shell/tab-store.test.ts` (round trip +
  absent-means-visible), `frontend/src/test/workspace-composition.test.tsx` (the
  spy follows the viewport; a click holds its entry; the lock releases) and
  `src/shell/layout_persist.rs` (the round trip and the legacy default).

- **The input's error slot (plan 118 task E6)** — the declared
  `textInput.default.error.rest` is consumed by `ClayTextField`'s error message:
  the caption renders below the well (colour from the recipe, no border), the
  input carries `aria-describedby` for both the description and the message, and
  the panel that has a real invalid state — the Settings panel's typography
  fields — passes a per-field message instead of toning the whole group. The slot
  ids come from `useId`, not from the label: `aria-describedby` is an IDREF *list*,
  so the label-derived ids the description had been using silently announced
  nothing for multi-word labels (found while wiring this). Gates:
  `frontend/src/test/components.test.tsx` (slot + association + absence),
  `frontend/src/test/settings-panel-choices.test.tsx` (the invalid field warns
  itself; the sibling fields stay clean), and the drift fixture recorded the key as
  consumed (19 → 18 backlog keys).

- **The session-files panel (plan 118 task 36)** — the agent view's Files tab is
  the session's file history, and it is the gate for the `sessionRow` family's own
  rows: each row carries `data-clay-component="sessionRow"`/`data-clay-slot="root"`
  and `frontend/src/coding-agent/coding-agent.module.css` references every declared
  property (rest fill/radius/colour/transition, hover, the focus ring), which is
  what removed the family's focus key from the adoption backlog.
  `frontend/src/agent/session-files.test.ts` pins the projection (verb → role, one
  row per path with the newest role, a read never demoting a mutation, rows
  ordered by their last touch, the transcript-cap bound),
  `frontend/src/coding-agent/CodingAgentPanel.test.tsx` pins the panel (rows from
  the transcript's records, count + filter, the session-history empty state, `⏎`
  handing the path to the shell, no editor or tree in the inspector), and
  `frontend/src/shell/WorkspacePanes.test.tsx` pins the handoff (the row opens
  through `openPath` — the same contained document-open path every other open uses
  — and the tab switches to its workspace view with the agent half still mounted).
  Server-side, `src/server/agent.rs` derives the record from the call's own
  arguments (redacted, bounded; a tool that names no single file records none) and
  `src/server/agent_agui.rs` carries it on the live row event and the snapshot
  metadata, so a resumed session rebuilds the same list. `surface-adoption.test.tsx`
  holds the CSS invariants (no geometry literals in
  `coding-agent.module.css`, the row painted only from the family, the filter's one
  ring) and `design-artifacts/tools/capture-agent-files.mjs` measures the browser:
  `design-artifacts/screenshots/quiet-instrument-agent-files/report.json` — the
  three seeded records (created/modified/read) in order with their family and paint
  (r8, spacing.xs padding, transparent fill), role-toned marks, the filter
  narrowing 3 → 1 with the count following, 0px horizontal overflow.
- **`design-artifacts/tools/capture-tab-views.mjs`** — the tab-chrome pass (plan
  118 task 33): drives the running shell over CDP and asserts the view switcher is
  present in a 40px titlebar, consumes the `seg` family (`seg.root` /
  `seg.item` attributes on the real elements), is inert with its reason on an
  uncommitted tab, and paints rest / selected (accent tint + accent text) /
  disabled through the theme's role variables — state probes wait out the 150 ms
  transition, and a disabled item is enabled before its selected paint is probed,
  because the `:disabled` rule wins the cascade. Evidence:
  `design-artifacts/screenshots/quiet-instrument-tab-views/report.json`.
- **`design-artifacts/tools/verify-component-conformance.mjs`** — the component
  conformance pass: the *audit* mode proves the approved specimen is exactly the
  130-key reference set minus the chat surface and that the shipped contract only
  extends it by the ten recorded target-IA families, then checks the language over
  the manifest (radius ladder, one non-zero border weight per surface,
  transient-only elevation with no hard offset, blur only on the scrim/toast, the
  eight entering surfaces on 240 ms spring-snappy, no filter/animation in any
  component stylesheet). The *browser* mode (`--browser <url>`) renders the DEV
  gallery and the approved specimen in `@clay/core` and the four shipped themes
  and asserts every painted component matches its declared recipe and the
  specimen's own rendering of the same key (screenshots + `report.json` land in
  `design-artifacts/screenshots/quiet-instrument-component-conformance/`).

---

## 4. Content-Theme Contrast Gate

The gate's policy, floors and the measured worst pair per group for the four
shipped themes and the `@clay/core` baseline are published in
[`docs/reference/ui-design-systems.md` §7](../reference/ui-design-systems.md). Two
properties matter for conformance review:

1. **Composited measurement.** `editor::theme::contrast_ratio` ignores alpha; the
   gate uses `composited_contrast_ratio`, which alpha-blends the foreground over
   the background first. Without it a 34 % hairline scored 4:1 on dark chrome and
   21:1 on light while rendering at 1.4:1 and 1.5:1 — every structural floor was
   satisfied by construction and no boundary was actually visible.
2. **No bypass by omission.** Pairs are resolved through the same projection the
   client receives (typed `designTokens` → base-ui colours → core catalog), so a
   theme that declares fewer tokens is measured on the fallback values instead of
   skipping the pair. A failing pair returns `ContrastFailure` naming the pair, the
   measured ratio and the floor, and the activation is refused with the previous
   theme still installed.

The border ladder is role-driven wherever a theme declares the roles: the theme's
border grey at 34 % (`border.hairline`), the same grey at 100 % (`border.subtle`)
and the theme's ink (`border.strong`). The legacy base-ui projection (a theme's
`scrollbar` colour standing in for all three) is a flat ladder; it is measured by
the same gate, and replacing it is tracked debt rather than a rule, because
third-party themes written before the Quiet Instrument migration depend on it.
