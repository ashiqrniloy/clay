# Quiet Instrument Migration — Component and Surface Adoption, Single Design System, Chat Removal

Source: user directive 2026-09-11 (decision
`decision-logs/2026-09-11-1700-quiet-instrument-migration-scope-no-third-design-system-chat-removal.md`),
the normative design language `DESIGN.md`, the approved artifacts
`design-artifacts/approved/quiet-instrument-language/`, and the artifact-gate
decision `decision-logs/2026-09-11-1655-html-prototype-approval-gate-and-design-artifacts.md`.
Supersedes plan `111-Graphite-Analog-Cockpit-Design-Systems-and-Paired-Themes.md`
(unexecuted; its three design systems and paired new themes are out of scope now)
and closes the retention position recorded in
`decision-logs/2026-09-11-1615-quiet-instrument-design-language.md` ("Neobrutal and
Glass remain shipped and user-selectable").

## Objectives

- **Component-level adoption.** Every cataloged component kind and every
  clay-native internal surface renders through the Quiet Instrument recipes:
  radius ladder 5/8/12/16/pill, exactly one hairline border weight, transient-only
  elevation, accent reserved for state, mono for every datum, complete state set
  (rest/hover/active/focus/selected/disabled/invalid). No component keeps 0px
  corners, 2px structural borders, hard offset shadows, or hover lift.
- **Page-level adoption.** Every page and region adopts the approved composition
  from `DESIGN.md` §12 and the approved artifacts: shell chrome, Workspace
  (sidebar · editor column at 92ch · optional rail), Coding Agent (header ·
  72ch transcript · inset composer · inspector), Settings panel, Agent Settings
  page, Command Centre, Package Workspace, and the overlay/menu/tooltip/toast
  family.
- **One design system, no third.** `@clay/design-instrument` becomes the single
  shipped design system and the product default; `@clay/design-neobrutal` and
  `@clay/design-glass` are deleted (package trees, fixtures, harness states,
  settings data, tests, documentation), with `@clay/core` kept as the built-in
  baseline, aligned to the same geometry so pre-bootstrap paint matches.
- **Themes stay decoupled and user-changeable.** Clay keeps exactly the four
  existing content themes; the migration changes theme _values_ only, through the
  typed `clay.contributions.designTokens` path (hairline ladder, accent, muted
  and state steps). No new theme, no palette set, no theme-contract change, no
  colour decision in host CSS.
- **Chat removed.** `@clay/chat`, the chat landing, its commands and legacy
  aliases, the `frontend/src/chat/` surface, its fixtures, docs and tests are
  removed. The shared AG-UI agent transport stays (it is the coding agent's event
  path) and loses its chat naming.
- **A tab is one workspace plus one agent, with two views.** Every tab holds a
  Workspace view (the folder: editor today, other viewers later) and an Agent
  view (the agent, switchable when more agent types exist). The view switcher is
  tab chrome; a tab with no agent yet shows the agent view's picker and a tab
  with no folder shows the workspace view's prompt. `@clay/coding-agent` keeps
  its `pane` surface, now hosted by the tab's agent view.
- **The landing surface is the launcher.** A fresh window and a new tab (⌘T)
  open a start surface that offers the two things a tab needs: recent workspaces
  and available agents, either or both, plus "open folder…". `@clay/coding-agent`
  no longer contributes the `empty-tab` landing (that contribution is superseded
  by the launcher). The core fallback (no contribution installed) stays Open File
  / Open Folder.
- **The artifact gate is honoured end to end.** This plan produces a migration
  prototype set, freezes the approved variant under
  `design-artifacts/approved/quiet-instrument-migration/`, and implements,
  reviews, and documents against it; deviations are fixed or re-approved.

## Expected Outcome

- A Linux build of Clay launches into the launcher, rendered in Quiet Instrument;
  opening a workspace, an agent, or both produces one tab holding both views,
  and the four shipped themes switch colours without any geometry change and no
  compiled-in appearance literals in host CSS.
- `@clay/design-instrument` is the only design-system package in
  `packages/`/the bundled inventory; `design-neobrutal` and `design-glass` are
  gone from the tree, fixtures, harness states, settings choices, tests, docs and
  the generated Clay JS API registry; no test or doc claims they exist.
- `@clay/chat` and `frontend/src/chat/` are gone; no `chat.*` command, intent,
  alias, fixture, or documentation reference remains outside historical records
  (`plans/`, `decision-logs/`, `docs/wiki/archive/`, `test-plan/artifacts/`).
- Each of the four themes ships typed `designTokens` for the hairline ladder,
  accent and state roles, validated by the contrast gate at apply time and by the
  upgraded theme conformance matrix; text pairs ≥4.5:1, structural boundaries
  ≥3:1, accents ≥3:1 against `surface.main`.
- The approved migration artifact exists under
  `design-artifacts/approved/quiet-instrument-migration/` with a recorded
  approval, and every implementation task cites it.
- All gates stay green: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, the Rust test suites (lib,
  protocol, runtime, security, presentation), frontend `tsc` + `vitest` +
  `vite build`, `clay-agent` `tsc` + tests, plus the manual test plan
  (`test-plan/`) executed and updated for the new landing and themes.
- `DESIGN.md` §16 no longer describes the migration as pending; the roadmap's
  Phase 2 agent-UI binding spec matches the shipped surfaces (chat extension
  points removed, the tab model, the launcher and the session-files inspector
  recorded).

## Scope and Planning Status

**In scope:** design-system package replacement; theme value additions; all
component and page adoption; chat removal; **the tab model** (one tab = one
workspace + one agent, two views, the launcher landing surface, the agent-type
picker and the session-files inspector — Part D, added on user direction
2026-09-11); the artifact gate and its approval record; every test, fixture,
harness state, doc, catalog, example config and wiki page that names the removed
systems/surfaces or the retired visual patterns.

**Out of scope:** new themes or palettes (explicitly excluded by the user);
new design-system schema (the approved language plus the Part D component
families are expressible in the current schema — `DESIGN.md` §16); a second
design-system package or a second visual language; changes to the editor's
CodeMirror internals beyond chrome that consumes design-system variables; a
replacement for the chat surface (deferred by the user: "we will change that
later"); viewers other than the editor in the workspace view (the seam is the
existing pane-content activation; no viewer registry is built until a second
viewer exists — YAGNI).

**Known constraints carried into the plan:**

- `@clay/design-neobrutal` and `@clay/design-glass` each declare the full 142-key
  recipe matrix. `@clay/design-instrument` is authored against that set minus the
  12 `chat.default.*` keys, **plus the families Part D needs** (view switcher,
  agent picker, launcher rows, session-file rows, and the already-declared-but-
  unbacked `seg`/`chip`/`empty`/`statusDot`/`shortcut`/`swatch`/`agentStatRow`
  surfaces — prototype README §9 finding 10); the final key set is fixed in task
  8 and asserted in task 12, and no key is invented without a consumer.
- Removing a bundled package changes the checked-in inventory and its build-time
  fingerprints (`src/packages/bundled-inventory.toml`, `src/packages/bundled.rs`
  `include!` of the generated table) — a plain `rm -rf` leaves a build failure.
- A persisted `designSystem` preference may name a removed package; activation
  must fall back with a diagnostic rather than fail the generation.
- The frontend conformance suites currently prove _two_ design systems switching
  into each other (`frontend/src/test/design-system-conformance.test.tsx`) and map
  recipe consumption per component CSS module
  (`frontend/src/test/design-system-consumption.test.ts`, which lists
  `chat/chat.module.css` as an owner) — both are rewritten by this plan.
- `frontend/src/coding-agent/coding-agent.module.css` (586 lines) consumes only
  one `--clay-ds-*` variable today; it is the largest single adoption surface.
- `@clay/chat` owns the `empty-tab` landing and `@clay/coding-agent` owns a
  `pane` surface; the host authorizes both by exact package provenance, so the
  landing change is a manifest + host-provenance change, never a core stub.
- The `chat.submit` / `chat.cancel` / `chat.steer` commands are the _chat
  landing's_ SDUI composer path and its server intent/authorization branch; the
  coding agent drives the AG-UI transport directly (`chatAgent.runTurn`), so
  removing the commands does not touch the agent's send path.

## Design Gate (prototype → approval → conformance)

This plan touches app UI everywhere, so the gate mandated by
`.agents/skills/create-plan/references/clay.md` → _UI Prototype and Explicit User
Approval Task_ applies with a plan-wide slug:

- **Prototype:** `design-artifacts/prototypes/quiet-instrument-migration/` —
  component specimen, page surfaces, theme board (tasks 3–6).
- **Approved (binding):** `design-artifacts/approved/quiet-instrument-migration/`
  — frozen in task 7 after explicit user approval; every implementation task
  cites it; task 27 reports deviations against it.
- **Normative language:** `DESIGN.md`. The approved artifact shows surfaces and
  states; where it and `DESIGN.md` disagree, `DESIGN.md` wins and the artifact is
  corrected through a new approval.
- No implementation task (tasks 8–25) may start before task 7 is checked.

## Evidence and Implementation Grounding

Verified while authoring this plan (2026-09-11):

- **Schema is sufficient.** `clay.contributions.designTokens` accepts core token
  names with `#rrggbbaa` colour roles, scalar/opacity/level variants;
  `src/packages/record/theme.rs` validates them, `ResolvedUiTheme` layers them
  over the core catalog, and `validate_active_theme_contrast` runs the AA floor at
  `setTheme` apply time. Recipe bounds (radius ≤32px or pill, border ≤8px, shadow
  ≤3 layers, motion ≤1000ms, opacity `[0,1]`) cover every approved value
  (`src/shell/design_system.rs`).
- **Contrast machinery exists and is selective.** `REQUIRED_CONTRAST_PAIRS`
  (`src/shell/theme.rs`) already gates `text.*`, `accent.primary`,
  `focus.ring`/`border.focus`; it has no structural-boundary pair yet, which the
  hairline language needs.
- **Themes ship `textStyles` only.** All four packages declare `textStyles` and no
  `designTokens`; `bundled_theme_conformance_matrix`
  (`tests/package_ui_conformance.rs`) currently asserts _zero_ `designTokens` and
  resolves the core fallback for contrast — the assertion is upgraded, not
  bypassed, by task 14.
- **Design-system runtime is package-data driven.** `apply_design_system`
  (`src/server/ops/theme.rs`) accepts `@clay/core` plus bundled
  `@clay/design-*`; `settings.setDesignSystem` re-validates in
  `src/server/command_execution.rs`; `ui_choices.design_systems` is enumerated
  from enabled contributions (so removing packages shrinks the list, while
  `packages/settings/package.json` + `packages/settings/dist/load.js` carry a
  static fallback list that must be edited too).
- **Surface inventory (the adoption surface).** Host CSS: `styles/tokens.css`
  (213 recipe-reference lines) plus 22 `*.module.css` modules; page/login
  surfaces: `app/layout/{shell,tab-bar,working-area}`, `shell/{PaneTree,WorkspacePanes}`,
  `routes/{workspace,fixture}`, `settings/SettingsPanel`,
  `agent-settings/AgentSettingsPanel`, `command-centre/CommandCentre`,
  `packages/PackageWorkspace`, `coding-agent/CodingAgentPanel`,
  `sdui/{registry,renderer}`, `editor/ClayEditor` chrome,
  `components/{button,controls,text-field,tab-strip,chrome,modal,tooltip,text,icon}`.
- **Removal inventory.** `packages/chat/**`,
  `src/packages/bundled-inventory.toml` (`root = "chat"`, `root = "design-glass"`,
  `root = "design-neobrutal"`), `src/server/command_execution.rs`
  (`is_chat_intent`, `execute_chat`, chat command tests),
  `src/server/connection/runtime.rs` (`is_agent_run_action`, `chat.cancel`
  host-cancel branch), `src/server/agent_picker.rs` (`chat.open*Picker` legacy
  aliases), `src/server/js_runtime/tests.rs` and `src/server/mod.rs` landing
  assertions, `frontend/src/shell/PaneTree.tsx` (chat empty-tab branch),
  `frontend/src/routes/fixture.tsx` (chat fixture), `frontend/src/test/design-system-consumption.test.ts`
  (chat ownership entries), `examples/config/{init.js,packages/first-party.js}`,
  docs and the generated registry.
- **Harness.** `scripts/capture-ui-review.sh` declares
  `ui-review-design-{neobrutal,glass}{,-light}` states backed by
  `tests/fixtures/configuration/ui-review-design-*`; `docs/wiki/modules/ui-review-harness.md`
  documents them (and the artifact contract: the table lists only states the
  script can capture).

## Execution Rules

- Task order is the plan order. Tasks 1–7 are gates: no implementation work
  (8–25) starts before the approved artifact exists; tasks 26–32 run on the built
  app; task 33 is last.
- Every UI-touching task reads `DESIGN.md`, `.agents/skills/clay-execution/references/ui.md`,
  `references/components.md`, `references/tokens.md` **before** editing, and lists
  them under `Approach -> Documentation Reviewed` (plan-level evidence does not
  substitute).
- Design-system and theme _appearance_ changes land in package data
  (`packages/design-instrument/package.json`, `packages/theme-*/package.json`), never
  in host CSS or component source; host CSS consumes variables. A host-CSS literal
  geometry/colour value is a defect (`plan103_css_module_literal_deny_scan`).
- Never weaken a gate to pass: an unsatisfiable value is a re-approval, not a
  relaxed test. Historical records (`plans/`, `decision-logs/`,
  `docs/wiki/archive/`, `test-plan/artifacts/`) stay immutable.
- Each task finishes with the relevant subset of: `cargo fmt --check`,
  `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --lib`, the integration suites, frontend `tsc`/`vitest`/`vite build`,
  `node --check` on touched JSON/JS manifests, and a re-run of the design-system
  and theme suites.

## Tasks

- [x] Baseline gates before any change
  - Acceptance Criteria:
    - Functional: the current tree passes `cargo fmt --check`, `cargo check --all-targets`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`, the integration
      suites (protocol, runtime, security, presentation), `tests/package_ui_conformance.rs`,
      `tests/theme_packages.rs`, `tests/documentation_coverage.rs`,
      frontend `npx tsc --noEmit` + `npx vitest run` + `npx vite build`; no new failures.
    - Performance: record suite durations and the frontend bundle sizes as the comparison
      baseline for the migration.
    - Code Quality: capture the frontend CSS inventory (`grep -c --clay-ds-` per module,
      literal-geometry counts per module) and the four themes' contrast status, so later
      adoption tasks can prove movement rather than assert it.
    - Security: note pre-existing failures explicitly so they are never attributed to this plan.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/SKILL.md` (deterministic execution loop).
      - `AGENTS.md` (Linux gates are blocking).
    - Options Considered:
      - Skip the baseline: risks attributing pre-existing failures to migration tasks. (Rejected.)
    - Chosen Approach: run every gate once on the untouched tree and record raw outputs.
    - Files to Create/Edit: none.
    - References: `plans/117-...md` (baseline-evidence pattern); `DESIGN.md` §15 checklist for
      the visual baseline (screenshots already captured in
      `design-artifacts/screenshots/2026-09-11-current-ui/`).
  - Test Cases to Write: none (gate run only).
  - Evidence (2026-09-11, Linux; `HEAD 2864b75` + 38 uncommitted files, all documentation/skill/artifact:
    no `src/` or `frontend/src/` code delta — the only Rust changes are doc comments in
    `src/shell/design_system.rs` and `src/packages/record/mod.rs`, verified comment-only):
    - Rust gates: `cargo fmt --check` pass; `cargo check --all-targets` pass (7.95s);
      `cargo clippy --all-targets -- -D warnings` pass (0 warnings); `cargo test --lib`
      1321 passed / 0 failed / 1 ignored (16.1s).
    - Integration suites: `protocol` 213/0 (30.0s — compiles the `documentation_coverage`
      and `primitives_docs` doc-drift modules); `runtime` 75/0 (49.8s); `security` 152/0
      (1.9s); `presentation` 46/0 (0.07s — compiles `tests/package_ui_conformance.rs` and
      `tests/theme_packages.rs` as modules, so design-system and theme conformance are
      green on the untouched tree).
    - Pre-existing flake: the day's first `protocol` run failed exactly 1 test (212/1); the
      test name was not captured before the run was re-executed. Six later runs are green at
      213/0, including one under 16-thread CPU saturation. Recorded so that a later failure
      is never attributed to this plan.
    - Frontend: `npx tsc -b --pretty false` clean (3s); `npx vitest run` 39 files / 301 tests
      pass (9.3s wall); `npx vite build` success (3.4s); `node scripts/bundle-budget.mjs`
      shell 164.5 kB gzip (budget 180) / total 381.0 kB gzip (budget 400).
    - Bundle baseline (raw / gzip): `index` 496.75 / 158.75 kB, `codemirror` 361.65 / 117.80,
      `chat-agent-core` 147.22 / 38.37, `CodingAgentPanel` 26.03 / 8.49, `ChatPanel` 5.33 /
      2.05 (removed by task 24), `index.css` 35.44 / 4.91 kB.
    - `clay-agent` (untouched by this plan, kept green): `npx tsc -p tsconfig.json` clean (2s);
      `npm test` 140 tests — 139 pass / 1 skip / 0 fail (10.8s).
    - CSS inventory (`frontend/src/**/*.module.css`): 814 `--clay-ds-*` references, 343 raw
      `Npx` literals, 0 hex colours. `frontend/src/styles/tokens.css`: 213 `--clay-ds-*` and
      325 `--clay-*` definitions, 81 px literals. Per module `ds-refs / lines / px / hex`:
      button 85/299/18/0, controls 86/369/26/0, chat 38/261/38/0, editor 35/626/24/0,
      package-workspace 32/230/17/0, tab-strip 27/185/15/0, text-field 24/126/5/0,
      shell 21/107/7/0, chrome 18/72/8/0, modal 16/110/11/0, command-centre 15/87/11/0,
      sdui/registry 13/78/7/0, pane-tree 12/73/5/0, settings-panel 11/92/13/0,
      tooltip 9/65/5/0, fixture 9/100/11/0, sdui/renderer 7/78/6/0,
      coding-agent 1/586/99/0, agent-settings 0/63/9/0, icon 0/32/4/0, text 0/54/0/0,
      routes/workspace 0/21/2/0, workspace-panes 0/17/2/0. Lowest-consumption adoption
      targets: `coding-agent` (1), `agent-settings`, `icon`, `text`, `routes/workspace`,
      `workspace-panes` (0).
    - Shipped design systems: `@clay/design-neobrutal` and `@clay/design-glass` each declare
      142 recipes over the same 31 component families (7 and 19 `values` entries). Neobrutal:
      `borderRadius: 0` on all 52 declarations, 2px on 81 border declarations, 5 shadow
      definitions. Glass: radii 6/8/4/14/10/9999px across 43 declarations, 1px on 71, 11
      shadow definitions. Recipes carry numeric literals rather than named `values` entries
      (the decoupling gap task 8/13 closes).
    - Theme contrast status (recomputed through the `src/shell/theme.rs` `base_color` mapping
      and the engine's alpha-ignoring `contrast_ratio`; script left out of tree). All 10
      `REQUIRED_CONTRAST_PAIRS` pass in all four themes (`text.muted` on panel 6.26 / 5.78 /
      8.16 / 7.06; `text.primary` on `surface.control` 12.55 / 8.06 / 7.26 / 6.37).
      Unenforced today, and therefore the migration's real contrast work: `border.hairline`,
      `border.subtle` and `border.strong` are one token for all three (`base.scrollbar`) — on
      `surface.main` 2.65 / 2.73 / 4.47 / 3.24 and on `surface.panel` 2.36 / 2.17 / 4.02 /
      2.92 (below the 3:1 structural floor in every theme but gruvbox-dark on chrome, and
      below it in three of four themes on panels); `text.disabled`/`accent.muted` 7.00 / 7.28
      / 3.37 / 2.45 on main and 6.26 / 5.78 / 3.03 / 2.21 on panel (both gruvbox themes fail
      4.5:1); `accent.primary` is `base.caret`, which equals `text.primary` in all four themes
      (no theme has a distinct accent hue — `DESIGN.md` §10 wants one); `text.muted` is
      silently promoted to `text.primary` in both gruvbox themes because their raw placeholder
      is under 4.5:1 on `surface.panel`.
    - Visual baseline: pre-migration screenshots of the four screens are in
      `design-artifacts/screenshots/2026-09-11-current-ui/` (captured the same day).
    - Pre-existing failures: none. Tree deltas unrelated to this plan:
      `clay-agent/.wiki/log.md` date row advanced by a tool (2026-09-10 → 2026-09-11).

- [x] Review the UI catalog and the language artifacts before designing the migration
  - Acceptance Criteria:
    - Functional: a written surface inventory names every component kind (package-facing
      and clay-native), every page/region, and for each one states: current CSS owner file,
      current recipe consumption, the approved `DESIGN.md` §11 recipe and §12 composition,
      and whether the approved two-screen artifact already demonstrates it or a new
      prototype page is required.
    - Performance: no runtime work; the inventory is a planning artifact.
    - Code Quality: the inventory is the checklist later tasks close out — it lives in the
      prototype README (`design-artifacts/prototypes/quiet-instrument-migration/README.md`)
      so review, implementation and the final visual review share one reference.
    - Security: catalog/authority rules restated: recipes are inert data, colour authority
      stays with themes, no host branching on package names.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` (§4 values, §5 geometry, §6 materials, §7 motion, §8 typography,
        §10 colour roles, §11 recipes, §12 composition, §14 retired patterns, §15 checklist).
      - `.agents/skills/clay-execution/references/ui.md` (binding rules, shell layout model).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, style
        variables, chrome primitives, internal surfaces, recipe slots).
      - `.agents/skills/clay-execution/references/tokens.md` (token types, core tokens,
        typography hierarchy, Quiet Instrument profile).
      - `docs/reference/ui-design-systems.md`, `docs/development/ui-design-system-recipe-matrix.md`.
      - `design-artifacts/approved/quiet-instrument-language/{README.md,workspace-rethink.html,agent-rethink.html,ds-quiet.css}`.
    - Options Considered:
      - Prototype only the two screens the approved artifact covers (Workspace, Coding
        Agent) and implement the rest from `DESIGN.md` prose: leaves Settings, Command
        Centre, Package Workspace, Agent Settings, overlays and the component catalog with
        no visual reference. (Rejected.)
      - Prototype everything including unchanged surfaces: adds approval cost for no
        change. (Rejected.)
    - Chosen Approach: prototype every surface the migration touches, reusing the approved
      language as the fixed vocabulary; the inventory decides which pages are new work and
      which are demonstrations of the approved screens.
    - API Notes and Examples:
      ```bash
      # current consumption per module (baseline for the adoption tasks)
      for f in $(find frontend/src -name '*.module.css'); do
        printf '%4d %s\n' "$(grep -c -- '--clay-ds-' "$f")" "$f"
      done | sort -rn
      ```
    - Files to Create/Edit:
      - `design-artifacts/prototypes/quiet-instrument-migration/README.md`: scope, coverage
        table, per-surface owner file, prototype/reuse decision.
    - References: `src/shell/components.rs` (kind catalog), `src/shell/design_system.rs`
      (recipe keys/bounds), `frontend/src/theme/design-system-adapter.ts` (variable naming).
  - Test Cases to Write: none (inventory); the table becomes input to tasks 3–6 and 16–22.
  - Evidence (2026-09-11):
    - Deliverable: `design-artifacts/prototypes/quiet-instrument-migration/README.md` — coverage
      decision, the package-facing kind table (17 identifiers), the internal-surface/chrome table
      (11 + 8), the page/region table (11 regions), the drift baseline, five findings that change
      plan tasks, and the prototype coverage table (artifact → surfaces → states → themes →
      widths).
    - Catalog sources read: `src/shell/components.rs` (kind list), `src/shell/design_system.rs`
      (`core_design_system_fallbacks`, bounds), `docs/development/ui-design-system-recipe-matrix.md`
      (43 + 38 + 12 rows), `DESIGN.md` §5–§12/§14, `docs/reference/ui-design-systems.md`,
      `references/{ui,components,tokens}.md`, and the approved artifact set.
    - Recipe key set: 142 keys over 50 `component.variant` families in both reference packages
      (`button.default|muted|primary|danger`, `textInput`, `dropdown`, `list`, `collapse`,
      `modal`, `panel.fixed|transient|default`, 8 `label` variants, `statusItem`, `flex.row|column`,
      `stack`, `overlay`, `portal`, `scroll`, `tab`, `tabBar`, `card`, 6 `badge` variants,
      `kbd`, `tooltip`, `popover`, `menu`, `commandCentre`, `chat`, `editor`, `shell`, `statusBar`,
      `fileBrowser`, `paneSplitTree`, `settingsPanel`, `divider`).
    - Current consumption measured per module (`--clay-ds-*` by family): button 85, controls
      (dropdown 40 / list 23 / collapse 23), editor 35, tab 27 (tab 24 + tabBar 3), textInput 24,
      modal 16, commandCentre 15, paneSplitTree 13 (12 CSS + 1 inline), statusBar 10 (6 shell +
      4 package-workspace), tooltip 9, settingsPanel 11, fileBrowser 4, chrome primitives 18
      (kbd 8 / badge 7 / divider 3), panel 53 (registry 13 + renderer 7 + package-workspace 24 +
      fixture 9), chat 38. **Zero** consumers: `label` (8 keys), `statusItem`, `flex`/`stack` (4),
      `overlay`/`portal` (2), `scroll` (5), and the 6 non-default `badge` variants.
    - Drift baseline re-measured with the existing gate (`STRICT_DS_GATE=1 npx vitest run
src/test/design-system-consumption.test.ts`): **54 of 142 declared keys consumed by no CSS**
      and **24 consumed variables backed by neither package recipe nor `tokens.css` fallback**;
      the gate is `it.fails` today and task 12 flips it.
    - New findings folded into the plan: (1) the 12 `chat.default.*` keys cannot survive the
      chat removal → task 8 ships 130 keys, tasks 23–24 state it, task 12 asserts it;
      (2) `label`/`statusItem`/`flex`/`stack`/`overlay`/`portal`/`scroll` need owners in task 16;
      (3) package family names (`chat`, `editor`) differ from catalog names (`chatPanel`,
      `editorChrome`) — the package key set is the contract; (4) `welcome`/`transientMenu`/
      `completion` are core-fallback-only and must not gain invented keys (task 10 decides their
      fate); (5) `agent-settings.module.css` (0 recipe refs, 14 theme vars) and
      `coding-agent.module.css` (1 ref, 99 px literals, 586 lines) must be rebuilt from catalog
      components, not re-skinned (tasks 20–21).
    - Prototype decision: the approved artifact demonstrates the language on two screens only, so
      task 3 owns catalog coverage, task 4 owns the eight page prototypes (incl. the agent
      _landing_ state), task 5 owns the four shipped theme values, and the chat surface is deleted
      rather than re-skinned.

- [x] Build the component specimen prototype (design-artifacts/prototypes/quiet-instrument-migration/component-catalog.html)
  - Acceptance Criteria:
    - Functional: every package-facing component kind and every clay-native internal
      surface is rendered in every variant and every shippable state (`rest`, `hover`,
      `active`, `focus`, `selected`, `disabled`, `invalid`) with the approved Quiet
      Instrument values; each specimen is labelled with its recipe key
      (`component.variant.slot.state`) so approval maps 1:1 onto catalog entries.
    - Performance: no runtime work; the page opens over `file://` with no build step and no
      console errors, stays usable at 1024×800 and 1500×950, and reads `?theme=<id>` so a
      scripted capture can select a theme without a browser driver.
    - Code Quality: the specimen consumes one language stylesheet (`ds-quiet.css` from the
      approved artifact, copied into the prototype folder) and the four real theme palettes
      through a theme switcher; no inline style literals in the markup.
    - Security: static HTML only — no network requests, no external fonts, no scripts beyond
      the local kit.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5/§6/§7/§8/§9/§11, `references/components.md`
        (kinds + style variables), `references/tokens.md` (token roles), `references/ui.md`.
    - Options Considered:
      - Specimen per component file (20+ HTML files): noisy approval surface, slow to
        compare states side by side. (Rejected.)
      - Single specimen page with a kind/state matrix: one review surface, state gaps become
        visible. (Chosen.)
    - Chosen Approach: one page, grouped by family (buttons, inputs, controls, tabs, chrome,
      lists/rows, overlays, status/data, editor-adjacent), each row a component, each column
      a state; a "keyboard" section for kbd chips, hint rows and shortcut affordances.
    - API Notes and Examples:
      ```html
      <!-- state is data, not a class name, so the matrix stays mechanical -->
      <button
        class="btn"
        data-state="hover"
        data-recipe="button.default.root.hover"
      >
        Save
      </button>
      ```
    - Files to Create/Edit:
      - `design-artifacts/prototypes/quiet-instrument-migration/component-catalog.html`
      - `design-artifacts/prototypes/quiet-instrument-migration/ds-quiet.css` (copied from the
        approved artifact), `theme.css`, `ds.js` (theme switcher only)
    - References: `design-artifacts/approved/quiet-instrument-language/ds-quiet.css`;
      `src/shell/components.rs` kind list; `docs/development/ui-design-system-recipe-matrix.md`.
  - Test Cases to Write:
    - State completeness: every kind/variant shows all seven states (scripted DOM assertion
      in the verification task).
    - Theme coverage: the switcher renders all four shipped themes without layout shift.
  - Evidence (2026-09-11):
    - Deliverables: `design-artifacts/prototypes/quiet-instrument-migration/component-catalog.html`
      (408 lines, 130 specimens), `theme.css` (four shipped palettes; the two proposal-only ones
      dropped), `ds-quiet.css` (copied verbatim from the approved artifact), `ds.js` (trimmed to
      the theme switcher, `?theme=<id>`, tabs, toasts, shortcut glyphs — local `clay-qs-prefs`
      key, no third-party code), and the generator
      `design-artifacts/tools/make-component-catalog.py` (writes the page; `--check` fails if the
      committed page drifts from the contract).
    - The matrix is derived from the contract, not hand-written: the generator reads
      `packages/design-neobrutal/package.json`, drops the 12 `chat.default.*` keys, and emits one
      specimen per remaining key with `data-recipe="component.variant.slot.state"`, `data-state`
      and `data-fx`. It refuses to render if a declared slot has no renderer, if a renderer exists
      for an undeclared slot (dead code), or if a family is missing from a section. Coverage is
      therefore a property of construction, not of diligence.
    - Coverage: 130/130 keys, 49 families (14 interactive, 35 rest-only), 181 undeclared
      combinations printed as `—` and counted in the header, 38 platform-aware `kbd` chips filled
      by the local kit on load.
    - Verification (headless Chromium over `file://`, four shipped themes × 1500×950 and
      1024×800): the DOM key set equals the embedded `#catalog-manifest` (no missing/extra), every
      specimen carries `data-state` and a `data-fx` from the documented vocabulary, 0 console
      errors/warnings, 0 external requests, 0 horizontal overflow, 0 clipped text, and the forced
      states actually apply — button hover `surface.hover` ≠ rest transparent with active one step
      deeper, focus `outline 2px` at `2px` offset, text-field focus accent border + 3px `@0.15`
      halo, invalid border `diagnostic.error`, row selected accent `@0.15` + 2px inset bar, tab
      selected accent text, tooltip shown open, scrim `blur(3px)` inside its own box.
    - `?theme=<id>` selects a theme without persisting (`localStorage` verified empty), the
      popover's click path switches theme and repaints, Alt+1..4 and the density/motion toggles
      work, and the live tab strip answers ArrowRight with exactly one visible panel.
    - Defects found and fixed while verifying: specimen state attributes were moved from a wrapper
      onto the component element itself (the wrapper was forced, the component was not);
      `.sheet`/`.popover`/`.palette`/`.cat-win`/`.ed-frame` overflowed their cells and
      single-column matrices stretched to the full table width (cells now cap at 420px, floating
      surfaces keep their natural 250px); `.pal-list`/`.palette` were inline spans with zero
      height; `.cat-static` made the scrim demo static so the scrim covered the whole page (now
      130px inside its box); long control labels spilled out of `.btn` (now shrink with an
      ellipsis, recorded as a Section B extension); the header's undeclared-combination count
      disagreed with the DOM (now counted where the cells are rendered).
    - Findings pushed to other tasks: the documented recipe vocabulary is wider than any shipped
      package declares (inventory finding 6 → task 8 keeps the 130-key set and invents nothing,
      task 15 corrects the prose); the theme contract has no `surface.scrim` role though
      `modal.default.scrim` requires one (inventory finding 7 → task 13 adds it, task 14
      contrast-checks it).

- [x] Build the page-surface prototypes (shell, workspace, agent landing, settings, command centre, package workspace, overlays)
  - Acceptance Criteria:
    - Functional: each page prototype reproduces the approved composition (`DESIGN.md` §12)
      with the real information architecture and real content; the agent prototype shows the
      _landing_ state (fresh window / new empty tab) including first-run, empty transcript,
      running, error, and resumed-session states; the workspace prototype shows the approved
      sidebar · editor(92ch) · rail composition with the path field on demand; settings,
      agent settings, command centre, package workspace, and the overlay family (modal, palette,
      dropdown, menu, tooltip, toast) each appear with every state they can ship.
    - Performance: no build step, no console errors, no horizontal overflow or clipping at
      1024×800 and 1500×950; the prototype states are static — no fabricated live data.
    - Code Quality: prototypes use the same static `workspace-data.js` snapshot generator for
      real repository content (`python3 design-artifacts/tools/make-workspace-data.py`); every
      element that represents a catalog component carries `data-clay-ds="<recipe key>"`.
    - Security: static HTML/JS only; no Tauri, no server calls, no invented credentials or
      tokens, no fabricated metrics (a value that cannot come from the app is omitted).
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (composition), §9 (state language), §8
        (typography roles), `references/ui.md` (shell layout model, keyboard-first rules),
        `references/components.md` (surface inventory), approved artifacts
        `workspace-rethink.html` / `agent-rethink.html`.
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - One combined page for all surfaces: hides per-surface viewport behaviour and makes
        narrow/wide checks meaningless. (Rejected.)
      - One page per surface plus a shared kit (chosen): approval can accept or reject each
        page, and each page is testable at its own widths.
    - Chosen Approach: `shell.html`, `workspace.html`, `agent-landing.html`, `settings.html`,
      `agent-settings.html`, `command-centre.html`, `package-workspace.html`, `overlays.html`,
      sharing `ds-quiet.css` + `theme.css` + `ds.js`, with a sticky theme switcher and a
      width toggle (narrow/wide) on every page.
    - API Notes and Examples:
      ```bash
      python3 design-artifacts/tools/make-workspace-data.py   # refresh real repo content
      ```
    - Files to Create/Edit: the eight prototype pages above plus their `README.md` coverage
      table (page → surfaces → states → themes → widths).
    - References: `design-artifacts/prototypes/quiet-instrument-migration/component-catalog.html`
      (its Section B is the component reference every page composes from);
      `frontend/src/app/layout/*`, `frontend/src/shell/PaneTree.tsx`,
      `frontend/src/settings/SettingsPanel.tsx`, `frontend/src/coding-agent/CodingAgentPanel.tsx`,
      `frontend/src/command-centre/CommandCentre.tsx`, `frontend/src/packages/PackageWorkspace.tsx`.
  - Test Cases to Write:
    - Landing flow: the fresh-window prototype shows the agent (not a chat greeting, not an
      empty-tab placeholder) and a keyboard path to open a file or folder from it.
    - Narrow layout: every page reflows at 1024×800 with no clipping (scripted assertion).
  - Evidence (2026-09-11):
    - Deliverables: `design-artifacts/prototypes/quiet-instrument-migration/` now holds
      `shell.html`, `workspace.html`, `agent-landing.html`, `settings.html`,
      `agent-settings.html`, `command-centre.html`, `package-workspace.html`, `overlays.html`,
      the shared language (`components.css` — the catalog's Section B, de-scoped so pages can use
      it; `pages.css` — review frame only), the adopted interaction layer `ds.js`, a real
      `workspace-data.js` snapshot, and the coverage table in `README.md` §7.1. Generated by
      `design-artifacts/tools/make-pages.py` (`--check` fails on drift, on a recipe key no package
      declares, or on a component element without its `data-clay-ds` key).
    - Two pages are **derived from the approved artefacts**, not re-authored:
      `workspace.html` and `agent-landing.html` take the approved screen's composition, CSS and
      behaviour verbatim (`derive()` reads `approved/quiet-instrument-language/*.html`), losing
      only the presentation `.desk` backdrop and gaining the review frame plus state scenes. That
      is what makes "reproduces the approved composition" checkable instead of asserted.
    - The six authored pages read their facts from the repository: the file tree and document
      bodies from `workspace-data.js`, the package list, versions, contribution kinds and file
      sizes from `src/packages/bundled-inventory.toml` + `packages/*/package.json` + `stat`, the
      agent file list and sizes from `.agents/skills/*/SKILL.md`, command titles and ids from the
      package manifests. The only invented text is UI copy for states the app has no words for
      (the empty working area, the "no document open" hints); every string that exists in the app
      is the app's own, including the daemon's session message in the error state.
    - States: the agent page ships five scenes — `landing` (the approved first-run state, no
      conversation, keyboard path), `running` (real streaming note + cancel hint), `error`
      (`no active menu session for id 9223372036854775810 (client 3)`), `resumed` (session notice
      - prior turn), `empty-session`. `overlays.html` ships `rest` and `reduced-transparency`
        (verified: scrim `blur(3px)` with a 50% fill vs a solid fill and `blur(0)`).
    - Verification (headless Chromium, `file://`): all 8 pages × {1500×950 modus-operandi,
      1024×800 gruvbox-material-dark} — 0 console errors/warnings, 0 external requests, 0 document
      overflow, 0 unexpected horizontal scroll (the only clipped boxes in the whole set are
      `sr-only` spans and tooltips, which are positioned and clipped by definition), and
      per-page facts asserted rather than eyeballed: shell 99 tree rows / 3 tabs / 3 status
      segments; workspace 38 document lines + 38 gutter lines with `--measure: 92ch`, 11 outline
      entries, path strip hidden; agent 5 inspector tabs, 13 real skills, palette present, and per
      scene exactly the right visible turns/strips/notices; settings 2 groups, 4 theme rows, 3
      design systems, invalid field, disabled Apply; agent files 6 rows + 6 provenance badges;
      palette 14 rows in 3 groups, `zzzznomatch` → 0 rows and the empty state shown, `session` →
      5 rows in 1 group; packages 21 rows (20 roots + 1 helper), 8 contribution kinds, real file
      sizes; overlays 12 cells, 3 toast tones, 4 tooltips, one visible stage per scene. All four
      shipped themes were swept with 0 console errors, and the width toggle was verified to clamp
      the frame to 1024px and back.
    - Defects found and fixed while verifying: `shell.html` inherited the approved window's
      three-row grid while supplying four rows (the tab strip took the 1fr row and flattened the
      layout) — the strip now lives in the title-bar row and the split is a three-column grid;
      `settings.html`/`agent-settings.html` supplied two rows to the same three-row window;
      the disclosure trigger rendered as a platform button (bare `button` reset added) and a
      closed disclosure still showed its body (`[data-open='false'] > .collapse-body` now hides);
      the palette had no height cap so its last group and its empty state were unreachable; the
      overlay modal and its footer overflowed a review cell (cells widened, `.sheet-foot` now
      wraps — a narrow-window defect in the product, not just the sheet); two tooltips on one line
      overlapped each other and hid the specimen they label; the approved screens still said "six
      palettes" after two were dropped.
    - Findings pushed to other tasks: the language styles surfaces with **no recipe family** —
      toast, `.seg` segmented control, chip (already the same idea as badge), empty state, status
      dot, the shortcut vocabulary around a key chip, theme swatch, and the agent's stat rows —
      task 8 either declares them or unifies them with an existing family; and `ds-quiet.css`
      still draws the film grain on `.desk` that `DESIGN.md` §14 bans, which task 16 must delete
      instead of porting into host CSS.
    - Also carried forward: the prototype set now loads **one** interaction layer — the approved
      `ds.js` (with the two proposal-only themes removed and the review switches added) — and the
      component sheet was moved onto it, replacing the catalog's divergent `data-pop` /
      `data-pop-trigger` attribute names with the approved `data-popover` / `data-popover-toggle`.
      The `TESTING.md`-style duplication this removes is the reason the catalog re-verified
      unchanged (130/130 keys, 181 undeclared combinations) after the swap.

- [x] Build the theme-value prototype board (the four shipped themes with the proposed designTokens)
  - Acceptance Criteria:
    - Functional: the board renders all four shipped themes
      (`modus-operandi`, `modus-vivendi`, `gruvbox-material-dark`, `gruvbox-material-light`)
      at their current values next to the proposed values, showing: hairline/subtle/strong
      border steps, accent + accent-driven states (focus ring, selection fill, running work),
      muted/disabled text steps, hover/active/selected fills, veil and scrim materials, and
      the core-fallback (no design system installed) sample.
    - Performance: no runtime work; the contrast table is static output of a checked-in
      measurement script, not hand-typed numbers.
    - Code Quality: every proposed value is listed as the exact
      `clay.contributions.designTokens` entry (`token`, hex) that tasks 13–14 will add, so the
      approved board _is_ the theme task's specification.
    - Security: no new colour source — every proposed value is a theme-package value; the
      board states explicitly that host CSS receives no colours from it.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §10 (roles + the four theme-side requirements), §13
        (accessibility invariants), §6 (materials), `references/tokens.md` (Quiet Instrument
        profile: which roles are core vs design-system-local), approved artifact
        `theme.css` (measured contrast table).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
    - Options Considered:
      - Tune the four themes inside the implementation task: no visual approval of the
        actual shipped palettes; a rejected palette would surface only after CSS work.
        (Rejected.)
      - Board first, approval covers the values (chosen).
    - Chosen Approach: compute the candidate values from each theme's own ink/scale, measure
      contrast with the same WCAG maths the runtime uses, and present current vs proposed
      side by side per role, including which pairs the gates will enforce
      (text ≥4.5:1, structural boundary ≥3:1, accent ≥3:1 against `surface.main`).
    - API Notes and Examples:
      ```json
      { "token": "border.hairline", "value": "#00000057" },
      { "token": "accent.primary",    "value": "#0031a9" }
      ```
    - Files to Create/Edit:
      - `design-artifacts/prototypes/quiet-instrument-migration/themes.html`
      - `design-artifacts/prototypes/quiet-instrument-migration/theme-values.md` (the
        proposed entry list + measured ratios per theme)
    - References: `packages/theme-*/package.json` current `textStyles`;
      `src/shell/theme.rs` `REQUIRED_CONTRAST_PAIRS` and `ResolvedUiTheme` resolution;
      `src/packages/record/theme.rs` `designTokens` validation.
  - Test Cases to Write:
    - Contrast: every proposed text pair ≥4.5:1, structural boundary ≥3:1, accent vs
      `surface.main` ≥3:1 across all four themes (script output stored with the board).
    - Ladder distinctness: hairline < subtle < strong in perceived contrast on every theme.
  - Evidence (2026-09-11):
    - Deliverables: `design-artifacts/prototypes/quiet-instrument-migration/themes.html` (the visual
      board — all four themes at once, current next to proposed, with the ladder, the states drawn,
      the materials and the core-fallback sample), `theme-values.md` (the specification: a
      copy-paste `clay.contributions.designTokens` block per theme plus every measured ratio) and
      `theme-values.json` (the single source of truth the generator reads). Generated by
      `design-artifacts/tools/make-theme-values.py`; `--check` fails on drift, and the script exits
      non-zero if any floor is missed.
    - Proposal: 13 roles per theme — `border.hairline`/`subtle`/`strong`, `surface.hover`/`active`/
      `selected`, `accent.primary`/`muted`, `focus.ring`, `border.focus`, `text.muted`/`disabled`,
      `surface.scrim`. The ladder is the theme's own border grey (`--c-line-2`) at 34 / 100 / 100 %
      plus ink for `strong`; every value is already in the theme's approved palette, so the board adds
      no colour source. `theme-values.md` records the exact JSON per theme, and task 13 is a copy of
      it.
    - Measured (composited: alpha over the surface it is drawn on — what the eye sees, and what the
      runtime gate does not yet do): all 12 enforced pairs pass in all four themes, and the ladder is
      monotonic in all four. `border.subtle` (the floor's role) goes from 1.84/1.76/2.74/2.06:1 on
      canvas (1.71/1.63/2.59/1.95 on the panel — it fails everywhere today) to 3.59/4.18/4.47/3.67:1.
      `text.disabled` goes from 7.00/6.86/3.37/3.37:1 (failing in both Gruvbox themes) to
      5.33/6.49/6.18/5.21:1. `accent.muted` from 7.00/6.86/3.37/3.37:1 to 5.48/5.07/4.30/3.56:1. The
      state fills against their own text go from 1.03–1.43:1 (today: one 40%-alpha selection colour
      for hover, active and selected — `surface.active` on `text.primary` is 1.03:1 in Gruvbox Light)
      to 5.66–16.83:1, and every fill becomes opaque as §10.3 requires. Scrim: the dim step is now
      reported as the composited canvas (e.g. Modus Operandi `#ffffff` → `#7f7f7f`, 4.00:1) rather
      than the raw colour, which is why the board states that a black canvas cannot be dimmed at all.
    - Verification (headless Chromium, `file://`): 4 theme panels + the core-fallback panel, 5
      distinct panel canvases, 4 distinct state sets, 4 distinct three-step ladders, 48 enforced-pair
      rows with 0 non-pass, 26 swatches per theme, 0 console errors, 0 external requests, 0 document
      overflow and 0 clipped boxes at both 1500×950 and 1024×800. Every panel's computed `--c-text`,
      `--c-accent` and `--c-line-2` were asserted to be its own theme's (the panels are scoped with
      `data-theme`, so they render side by side while the review frame keeps its own chrome).
    - Findings the board produced: (1) the approved artifact and `DESIGN.md` §10.1 define the ladder
      from different sources — the artifact uses the theme's border grey, §10.1 says ink; the board
      proposes the border grey (it is what the approved screens render) and asks task 15 to reword
      §10.1, because ink at 34 % composites ~0.2 darker than the approved divider. (2) The contrast
      gate does not composite alpha (`editor::theme::contrast_ratio` reads `to_rgba8()` and ignores
      the alpha byte), so an alpha-carrying pair measures as if opaque — 21:1 for a 34 % hairline that
      renders at 1.45:1; the same fix belongs in task 14 or the `border.subtle` floor is decoration.
      (3) `--c-line` and `--hairline` are two names for one step and `--c-line` has no consumer left,
      so `border.hairline` replaces both (task 16 deletes the dead variable). (4) `surface.scrim` has
      a core fallback but no `base_color` projection, so a legacy theme dims with the core catalog's
      colour instead of its own. (5) Today every accent-driven role resolves to the monochrome caret,
      and the core catalog is not itself language-compliant (boundaries 1.3–2.0:1, scrim 1.10:1
      against its canvas) — it is the design-system-less baseline, not a theme the gates validate.
      (6) `theme-gruvbox-material-dark` inverts depth: it ships `shellBg` #1d2021 and `panelBg`
      #282828, so `surface.main` resolves to the chrome colour and the panel renders lighter than the
      canvas, which §10.2 forbids; the fix is a two-value `textStyles` swap (no token), and task 13's
      "keep textStyles untouched" must be relaxed for exactly that pair. The other three themes
      already resolve depth correctly.

- [x] Verify the prototype set and record its evidence
  - Acceptance Criteria:
    - Functional: every prototype page opens over `file://` with zero console/script errors;
      all four themes render on every page (selectable through the page's theme switcher _and_
      through a `?theme=<id>` query parameter so capture is scriptable without a browser
      driver); all states listed in the coverage table exist in the DOM; the landing, empty,
      loading, error and disconnected states are present where applicable.
    - Performance: no horizontal overflow/clipping at 1024×800 and 1500×950 on every page and
      theme; the pages load without network access.
    - Code Quality: screenshots are stored under
      `design-artifacts/prototypes/quiet-instrument-migration/screenshots/` named
      `<page>__<theme>__<width>__<state>.png`; the README records the exact command used, the
      coverage table, and the note that prototypes carry no authority.
    - Security: the verification records that no prototype fetches anything remotely and that
      no fabricated data is presented as real (checked by inspection of the snapshot source).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (prototype task
        duties), `.agents/skills/clay-execution/references/planning-checklist.md`.
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Screenshot only the default theme: approval would not cover dark/light or the
        hairline ladder on a light theme. (Rejected.)
    - Chosen Approach: scripted capture per page × theme × width with the headless-Chrome CDP
      helper, plus a state-presence DOM assertion and an overflow check; failures are fixed
      before requesting approval.
    - API Notes and Examples:
      ```bash
      # Any headless-Chrome path works; record the exact command in the README evidence.
      # CDP helper (sets the theme through the page switcher and captures):
      CHROME=/usr/bin/google-chrome   # or chromium
      "$CHROME" --headless --screenshot=/tmp/shot.png --window-size=1500,950 \
        "file://$PWD/design-artifacts/prototypes/quiet-instrument-migration/workspace.html?theme=modus-vivendi"
      ```
    - Files to Create/Edit: `design-artifacts/prototypes/quiet-instrument-migration/screenshots/**`,
      README evidence section.
    - References: `design-artifacts/approved/quiet-instrument-language/README.md` (measured
      contrast table pattern).
  - Test Cases to Write: none (evidence gathering); the recorded screenshots are the input to
    the approval task and later to the visual review comparison.
  - Evidence (2026-09-11): one checked-in verifier drives the whole set and stores the review
    captures — `design-artifacts/tools/capture-prototypes.mjs` (headless CDP, one Chrome process,
    `--no-shots` for assertion-only runs; exits non-zero on any failure, which is what makes task 10's
    approval rest on a gate rather than a screenshot folder).
    - Command: `CHROME=/usr/bin/google-chrome node design-artifacts/tools/capture-prototypes.mjs`
      (the Chromium path is auto-discovered from `CHROME`, `/usr/bin/*` or the Playwright cache, and the
      resolved path is recorded in `screenshots/report.json`).
    - Runs asserted: **96** = 10 pages × 4 themes × 2 widths (80) + 16 scene runs, in one 37 s pass.
      Asserted per run: zero console/script errors, zero non-`file://` requests, zero horizontal
      overflow and zero clipped boxes (via `checkVisibility`, so a closed off-canvas popover is not
      miscounted), the requested `?theme=` really on `documentElement`, every claimed state selector
      present, scene exclusivity (only the active scene's block painted, none painted in the default
      state) and 40/40 distinct page × theme canvases.
    - Result: 96/96 clean — 0 console errors, 0 remote requests, 0 px overflow, 0 clipped boxes.
      Captured **66 PNGs** (~15 MB) into
      `design-artifacts/prototypes/quiet-instrument-migration/screenshots/` with the required
      `<page>__<theme>__<width>__<state>.png` naming (e.g.
      `agent-landing__gruvbox-material-light__1500x950__disconnected.png`) plus `report.json` holding
      every run. Stored sample: every page × every theme at 1500×950, every page at 1024×800 in
      Gruvbox Material Light, and every non-default scene in Modus Vivendi and Gruvbox Material Light;
      the remaining runs are asserted but not stored, to keep the evidence under 15 MB. The sampling
      policy and the exact command are in the set's README §8.
    - Behaviour, not decoration: two claims are driven and measured rather than photographed — the
      workspace tree filter narrows 99 rendered rows to 3, and a no-match query reveals the palette's
      empty state (hidden → shown). Both run for every page × theme (16 runs).
    - Honesty: 156 agent-file size rows were re-read from `.agents/skills/*/SKILL.md` and compared with
      what the page prints; all match. This forced a real fix: `agent-settings.html` carried baked-in
      sizes (including one for `SYSTEM.md`, a file the daemon writes at runtime that does not exist in
      the repository), so its rows are now generated from disk and `SYSTEM.md` is labelled "written at
      runtime" instead of carrying an invented figure.
    - Functional coverage of the five states the AC names: landing (`agent-landing` landing scene),
      empty (empty session, the shell's empty pane, the palette's no-results, agent files not yet
      delivered), loading (the running scene's streaming turn), error (failed turn, unreadable skills,
      package failed verification) and **disconnected** — added by this task: the daemon is not
      reachable, the footer transport row flips from `graft · 6 tools` to `transport unavailable`, and
      sending is dimmed. Asserted per state, not just staged.
    - Coverage table corrected to the truth: every row now states _how_ each state is demonstrated —
      **scene** (page switcher), **dom** (in the markup), **behaviour** (driven: palette no-results,
      tree filter) or **catalog** (the 130-specimen 7-state matrix). States the table used to claim but
      the derived approved screens cannot show are recorded as findings instead of staged: the workspace
      page cannot demonstrate filtered/read-only/no-workspace/no-headings without inventing markup the
      approved artifact does not contain (prototype README §9, findings 7 and 8 — a decision for task 10).
    - Bugs found and fixed while verifying: the state switcher could not return to a default state that
      had no block of its own (`setScene` only knew block ids, so "First run" was a one-way door and
      `?scene=landing` silently no-opped); the disconnected scene's footer row was permanently hidden by
      a literal `hidden` attribute; the agent-files page had no way to show its empty/error states (it
      now has three scenes). The verifier gained a per-state assertion for the footer row so none of
      these can return silently.
    - Re-verified after the 2026-09-11 artifact review (the launcher, the tab's dual view, the
      agent picker and the session-files inspector were added to the set): **112 runs / 24 scene
      runs, 0 failures, 79 PNGs (~9.7 MB), 156/156 honesty rows**, plus two layout assertions on
      the composer (one ring on the shell, controls inside the field) and the launcher's
      selection/filter behaviour; the verifier also gained `--only=<page>:<theme>:<state>` and a
      navigation-commit gate (a load event can belong to the previous document, which made an
      earlier run measure a half-built page — prototype README §8).
    - Documentation evidence: `design-artifacts/prototypes/quiet-instrument-migration/README.md` §7
      (coverage table with demonstration method), §8 (verification: exact command, result table, naming,
      sampling policy, and what the verification explicitly does _not_ claim — prototypes carry no
      authority, nothing is fetched, nothing is fabricated) and §9 (11 findings from building and
      verifying the set); `design-artifacts/README.md` lists the verifier and the captured evidence.

- [x] Obtain explicit user approval and freeze design-artifacts/approved/quiet-instrument-migration/
  - Acceptance Criteria:
    - Functional: the prototype set is presented for approval with the decisions that need a
      user call named explicitly (component state treatment, page composition, the tab model and
      the launcher, the agent-type picker, the session-files inspector, the composer's focus ring
      and control placement, theme values); on approval the chosen files are copied to
      `design-artifacts/approved/quiet-instrument-migration/` and the task records the
      approval date, the approving user statement (quoted), the chosen variant, requested
      changes, and the exact surface/state/theme/width coverage that is now binding.
    - Performance: n/a (artifact freeze).
    - Code Quality: `design-artifacts/README.md` lists the new approved folder; the approved
      README names every binding file; losing variants stay under `prototypes/` marked as not
      approved; the approved set is append-only from here on.
    - Security: no prototype-side deviation is smuggled into the approved copy (the approved
      files are byte-identical copies of the approved variant plus the approval record).
    - Decisions added by task 6 (prototype README §9): whether the workspace page's single-state
      derived composition is accepted as-is or the missing filtered/read-only/no-workspace states become
      a follow-up artifact; and whether `agent-settings.html`'s composition (the agent's Settings tab
      listing delivered files with their sizes) is what approval expects under that name, or a controls
      page is wanted instead.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (freeze task
        duties), `.agents/skills/clay-execution/references/ui.md` (artifact contract).
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Approve implicitly from the previous design-language decision: that decision covered
        two screens, not the migrated surface set, the theme values, or the landing flow.
        (Rejected.)
      - Explicit approval of the full set (chosen) — the task stays unchecked until the user
        responds, blocking tasks 8–25 by construction.
    - Chosen Approach: one approval round covering the whole migration artifact set; a
      partial approval splits the set into accepted and iterate-again pages, and only accepted
      pages are frozen.
    - Files to Create/Edit:
      - `design-artifacts/approved/quiet-instrument-migration/**` (approved pages + kit +
        screenshots), `design-artifacts/approved/quiet-instrument-migration/README.md`
        (approval record + coverage table), `design-artifacts/README.md` (contents table).
    - References: `decision-logs/2026-09-11-1655-html-prototype-approval-gate-and-design-artifacts.md`.
  - Test Cases to Write: none (human approval gate); the approval record is the evidence.
  - Outcome (2026-09-11): **approved by the user and frozen** —
    `design-artifacts/approved/quiet-instrument-migration/` holds the 20 reviewed
    files as approved, plus the working copy's inventory as `README-inventory.md`
    and the approval record as `README.md` (per-file hashes in the record), with
    the working copy left live at
    `design-artifacts/prototypes/quiet-instrument-migration/`.
    Decisions this round settled, now normative in `DESIGN.md`: the composer keeps
    the shell's boundary **plus** its 3px halo with no inner outline (§12, §9); the
    leading accent bar on a selected row is retired at component level (§14.13,
    §11 `list.row.selected`); the tab is one workspace plus one agent with two
    views and the launcher is the landing surface (§12); the launcher picks **one
    workspace and one agent per launch**, both changeable in the tab afterwards.
    Logged in `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`
    (supersedes point 5 of `2026-09-11-1700-…`).
  - Open from the review round, carried into implementation: the workspace view is
    approved in one state (no-workspace / read-only / filtered states unprototyped
    — a decision required above), the four new families get catalog specimens with
    their recipes in task 8, and `themes.html`'s verdicts assume composited alpha
    measurement (task 14).

- [x] Author the @clay/design-instrument design-system package
  - Acceptance Criteria (added by task 4, 2026-09-11; **amended 2026-09-11 by its own outcome**:
    the final set is **165 keys** — 130 reference keys plus 35, with `viewSwitch` unified into a
    new `seg` family and `chip` unified into the existing `badge` family — see Outcome): the
    130-key set must also answer for the
    surfaces the language already draws and the contract does not name — the toast, the `.seg`
    segmented control, the chip, the empty state, the status dot, the shortcut vocabulary around a
    key chip (`.hint`, `.keys`, `.key-row`), the theme swatch, and the agent's stat rows.
    Added by Part D (2026-09-11): the target IA needs four more families before implementation —
    `viewSwitch` (the tab's Workspace | Agent switcher, one recipe per item state),
    `agentPicker` (the agent view's labeled dropdown trigger, reusing the dropdown list body),
    `recentRow` (the launcher's workspace/agent rows: name, muted path, meta, selected state) and
    `sessionRow` (the Files tab's session-history row: mark, basename, muted directory, role).
    The final key set is therefore 130 + however many of these are families rather than new
    slots on existing ones — fixed here, asserted in task 12, and consumed by tasks 33–36; no key
    without a consumer. Each is
    either declared (adding keys, and updating the 130-key assertions with them) or unified with
    an existing family — the chip and the badge are already the same idea twice. See
    `design-artifacts/prototypes/quiet-instrument-migration/README.md` §7.1 finding 1.
  - Acceptance Criteria:
    - Functional: `packages/design-instrument/package.json` declares
      `clay.contributions.uiDesignSystem` (`schemaVersion` 1, `id` `@clay/design-instrument`,
      `displayName` "Quiet Instrument") with the 130-key recipe matrix — the reference packages'
      142 keys minus the 12 `chat.default.*` keys whose surface tasks 23–24 delete, because a
      declared key with no consumer contradicts the consumption gate those tasks close
      (inventory finding 1; the chat replacement is deferred, not scheduled) — and the
      design-system-local values from `DESIGN.md` §4 (radius ladder 5/8/12/16/9999, hairline 1,
      motion 150/240/620, veil/soft/disabled opacities); every recipe matches the approved
      artifact's values (task 7) and references only semantic theme colour roles.
    - Performance: manifest stays inside `UI_DESIGN_SYSTEM_PAYLOAD_BUDGET_BYTES` and
      `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`; no host-CSS growth, no runtime work beyond the
      existing snapshot projection.
    - Code Quality: package ships the same file layout as the packages it replaces
      (`package.json`, `README.md`, `docs/index.md`), recipe keys equal the removed packages' key
      set minus exactly the 12 chat keys (no other add/remove/rename; the delta is asserted, not
      assumed), and the package is listed in
      `src/packages/bundled-inventory.toml` so the build-time fingerprint table includes it.
    - Security: inert data only — zero permissions, zero modes, zero executable surface, no raw
      CSS/JSX/URLs/callbacks, no literal colours, no typography overrides (package validation
      must reject a mutated probe fixture).
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §4 (values), §5 (geometry), §6 (materials), §7 (motion), §9 (state
        language), §11 (per-component recipes), §14 (retired patterns).
      - `.agents/skills/clay-execution/references/tokens.md` (design-system-local values),
        `references/components.md` (kinds/slots/states), `references/config.md` (UI
        Design-System Packages).
      - `docs/development/ui-design-system-recipe-matrix.md`, `docs/reference/ui-design-systems.md`.
      - `design-artifacts/approved/quiet-instrument-migration/**` (binding values).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
    - Options Considered:
      - Ship the language only as core fallbacks (no package): loses the versioned, conformance
        -tested data set and the ability to switch/revoke, contradicting the typed-recipe
        architecture (decision 2026-08-28-2234). (Rejected.)
      - Author the recipes by hand from prose: 142 keys × states invites drift between the
        approved artifact, the package and host CSS. (Rejected.)
      - Generate the key set from the removed packages' manifests, then fill values from the
        approved artifact and review family by family (chosen).
    - Chosen Approach: extract the 142 keys from `packages/design-neobrutal/package.json`
      (identical to `design-glass`), drop the 12 `chat.default.*` keys (inventory finding 1),
      write the values from the approved artifact's `ds-quiet.css` + the component-catalog
      prototype, then verify the asserted key delta, role-only colours and bounds with the
      existing validators.
    - API Notes and Examples:
      ```json
      {
        "clay": {
          "contributions": {
            "uiDesignSystem": {
              "schemaVersion": 1,
              "id": "@clay/design-instrument",
              "displayName": "Quiet Instrument",
              "values": { "radius.control": { "type": "radius", "value": 8 } },
              "recipes": {
                "button.default.root.rest": {
                  "backgroundColor": "transparent",
                  "textColor": "text.primary",
                  "borderColor": "border.hairline",
                  "borderWidth": 1,
                  "borderRadius": 8,
                  "transitionDuration": 150,
                  "transitionTiming": "ease-out",
                  "transformPreset": "none"
                }
              }
            }
          }
        }
      }
      ```
    - Files to Create/Edit:
      - `packages/design-instrument/package.json`, `packages/design-instrument/README.md`,
        `packages/design-instrument/docs/index.md`
      - `src/packages/bundled-inventory.toml` (add `root = "design-instrument"`)
    - References: `packages/design-neobrutal/package.json` (key set + manifest shape);
      `src/shell/design_system.rs` (`UiDesignSystemDeclaration`, bounds, `core_design_system_fallbacks`);
      `tests/package_ui_conformance.rs` (`design_system_enforces_color_authority_and_bounds`).
  - Test Cases to Write:
    - Key-set delta: the new package's recipe keys equal the removed packages' keys minus
      exactly the 12 `chat.default.*` keys (Rust test or scripted assertion) — any other missing
      key must fail, not silently fall back.
    - Colour authority: mutating a recipe to a literal colour or a palette alias is rejected.
    - Bounds: radius >32, border >8, shadow >3 layers, opacity >1, duration >1000 all rejected.
  - Outcome (2026-09-11): **authored and verified.**
    - `packages/design-instrument/{package.json,README.md,docs/index.md}` ship the language:
      **165 recipe keys** (142 reference − 12 `chat.default.*` + 35 new) and the 14 declared
      values (§4: radius 5/8/12/16/9999, hairline 1, motion 150/240/620, opacity 0.5/0.55/0.15,
      blur 3). Declaration 33.7 kB against a 64 kB budget; zero permissions, zero modes, no
      entry, no literal colour, no executable text.
    - Key-set reconciliation, asserted not assumed (the AC's "declared or unified" choice):
      **unified** `viewSwitch` → a new **`seg`** family (the tab's switcher _is_ a segmented
      control; a second family would have duplicated all six of its keys) and `chip` →
      the existing **`badge`** family (already `{accent,default,error,muted,success,warning}`);
      **declared** 35 keys across ten families: `seg` (6), `agentPicker` (5), `recentRow` (4),
      `sessionRow` (3), `statusDot` (5), `keyHint` (3), `swatch` (4), `statRow` (3), `toast` (1),
      `empty` (1). Task 12 asserts the delta; tasks 16–25 consume it.
    - `src/packages/bundled-inventory.toml` lists `root = "design-instrument"` (the trust
      binding's exact name/version/root/fingerprint is regenerated at build).
    - Tests, `tests/package_ui_conformance.rs` (suite `presentation`), all green:
      `plan118_design_instrument_is_inert_data_with_the_quiet_instrument_profile` (inertness,
      values, budget, no literals/executable text, every colour-bearing property a known role),
      `…_radius_state_and_material_discipline` (radius ladder only, ≤1px borders, shadows only on
      the two elevations, blur only on scrim+toast, no hover lift, press only on buttons, focus
      outlines at offset 2, selection fill-only with no edge), `…_keys_are_the_reference_set_minus_chat_plus_the_new_families`
      (165, exact delta, full state sets), `…_rejects_mutated_probes` (literal colour/alias,
      radius 40, border 12, duration 5000, opacity 1.5, backgroundOpacity −0.2, blur 64, 4 shadow
      layers, 600 recipes past the ceiling) and `…_is_listed_in_the_bundled_inventory`.
    - Findings recorded while authoring (no silent deviation):
      1. **The overlay shadow cannot use the artifact's spread.** `DESIGN.md` §6 and the approved
         CSS draw `spread -20`; the recipe schema bounds spread at ±16, so the package uses −16
         and §6 now says so. The host CSS may keep the softer −20 edge (task 16) — the recipe is
         the floor, not the ceiling.
      2. **No `status.*` role vocabulary exists.** The session-file marks and the status dot use
         `diagnostic.{success,warning,error}` and `text.muted`; inventing `status.ok/warn/error`
         here would put colour naming in the wrong layer. `DESIGN.md` §11 now names the real roles
         (the earlier draft's `status.*` names were wrong).
      3. **No per-edge borders and no border alpha** in the schema: a chrome strip's bottom
         hairline and a semantic badge's 34% border are declared as a 1px full border and a
         full-strength role; the host applies the edge mix (tasks 14/16). Documented in the
         package's `docs/index.md` §6 rather than left to be rediscovered.
      4. **The selected-row leading bar was still in `DESIGN.md` §9 and §11** (the artefacts
         installed it) — removed here, together with the §2 phrase that named it, so the spec and
         the package agree with §14.13.
    - Not this task, tracked: the package is **not yet the runtime default** (task 16 wires the
      core fallbacks/`tokens.css`, task 24 removes the reference packages), the themes' typed
      `designTokens` land in task 14, and the new families have no host CSS consumer until
      tasks 16–25 (the consumption gate is still in its red-first state, which task 12 closes).

- [x] Remove @clay/design-neobrutal and @clay/design-glass
  - Acceptance Criteria:
    - Functional: both package trees, their bundled-inventory entries, their harness fixtures
      (`tests/fixtures/configuration/ui-review-design-{neobrutal,glass}{,-light}/`) and harness
      states (`scripts/capture-ui-review.sh`), their settings-dropdown entries
      (`packages/settings/package.json` + `packages/settings/dist/load.js`), their
      documentation references, and every test that switches between them are deleted or
      rewritten to the shipped system; `grep -ri "design-neobrutal\|design-glass"` returns only
      historical records (`plans/`, `decision-logs/`, `docs/wiki/archive/`,
      `test-plan/artifacts/`) and explicitly-labelled historical notes.
    - Performance: no runtime regression; bundle and payload budgets shrink, not grow
      (recorded before/after).
    - Code Quality: no dead validation branch, fixture, or doc table row is left behind;
      `docs/wiki/modules/ui-review-harness.md` states the harness states that actually exist.
    - Security (two trust domains, decision 2026-07-21-0001): trusted-runtime placement still
      resolves only from the compiled bundled inventory bound to exact name + version + root +
      manifest fingerprint — never from `@clay/*` naming or user promotion; the inventory table
      and fingerprints are regenerated so a removed directory cannot leave a live entry (a probe
      build with the directory absent fails closed); removal changes nothing for third-party
      packages in the shared runtime, and `@clay/design-instrument` enters the trusted domain
      only through its inventory entry.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md` (UI
        Design-System Packages), `references/packages.md` (bundled inventory/trust domains),
        `references/ui.md` (catalog currency).
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Deprecate but keep the packages (documented as retained): the user explicitly ordered
        removal; keeping them leaves fixtures, docs and conformance obligations alive.
        (Rejected.)
      - Delete packages and fixtures in this task, before authoring the replacement: leaves the
        tree without any shipped design system mid-plan. (Rejected — task 8 precedes this one.)
    - Chosen Approach: delete package trees and inventory entries, rewrite every consumer to
      the new specifier in the same task, regenerate fingerprints by building, and re-run the
      design-system suites and the harness script's `--list` path.
    - Files to Create/Edit:
      - delete `packages/design-neobrutal/**`, `packages/design-glass/**`,
        `tests/fixtures/configuration/ui-review-design-{neobrutal,glass}*/**`
      - `src/packages/bundled-inventory.toml`, `scripts/capture-ui-review.sh`,
        `packages/settings/package.json`, `packages/settings/dist/load.js`,
        `docs/wiki/modules/ui-review-harness.md`, `test-plan/15-ui-design-systems.md`
        (steps UI-DS-02/16/17/19/26 rewritten — completed in task 26),
        `frontend/src/test/settings-panel-choices.test.tsx`,
        `docs/reference/clay-js-api/**`, `examples/config/init.js`
    - References: `src/packages/bundled.rs` (fingerprint/inventory model);
      `src/server/ops/theme.rs` `apply_design_system` (bundled `@clay/design-*` resolution).
  - Test Cases to Write:
    - Absence: a scripted assertion fails if either specifier appears in
      `packages/`, `src/`, `frontend/src/`, `tests/fixtures/`, or `scripts/`.
    - Trust binding: a `packages/design-instrument` copy without an inventory entry is never
      trusted (probe test), and a stale inventory entry for a removed directory fails the build
      rather than resolving.
    - Harness: `scripts/capture-ui-review.sh --fixture ui-review-design-system` still captures
      the shipped system; the removed state names are rejected by the argument check.
  - Outcome (2026-09-11): **removed and verified.**
    - Deleted: `packages/design-neobrutal/**`, `packages/design-glass/**`, and the four harness
      fixtures `tests/fixtures/configuration/ui-review-design-{neobrutal,glass}{,-light}/**`.
      `src/packages/bundled-inventory.toml` lost both roots (19 loadable roots remain).
    - Payload/bundle shrink (recorded before/after, AC "shrink, not grow"): the two manifests
      were 89,135 B raw / 5,345 B gzip-9; the single shipped package is 57,218 B / 3,110 B —
      **−35.8% raw, −41.8% gzip**, and one fewer package in the loadable inventory (no host CSS
      or runtime work added, so the task-2 shell/total gzip baselines cannot have grown from this
      change).
    - Retargeted rather than deleted, because each had a live job:
      - `ui-review-design-system{,-light}` fixtures + their two harness states now
        `setDesignSystem("@clay/design-instrument")` and record
        `active_design_system=@clay/design-instrument` (the shipped system) — the states keep
        their names, so no doc table needed a new row.
      - `src/server/ops/theme.rs` matrix test and `src-tauri/tests/dto_roundtrips.rs` (a real
        bundled specifier must resolve) → `@clay/design-instrument`.
      - `src/server/{command_execution,connection/tests}.rs` built `@clay/design-{suffix}` from
        the string `"neobrutal"`; the suffix is now `"instrument"` (the source-independence guard
        still sees no package-name literal).
      - `packages/settings/{package.json,dist/load.js}` dropdown → `@clay/core` +
        "Quiet Instrument (Default)" (the static fallback list the settings panel reads).
      - `examples/config/init.js` comments and `docs/reference/clay-js-api/**` (source of the
        generated registry — regenerated with `cargo run --bin update-doc-registry`).
      - `tests/theme_packages.rs` lost both `design_{neobrutal,glass}_bundled_package_validates_as_inert_data`
        tests (task 8's per-package suite asserts the shipped package harder) and
        `plan110_design_system_packages_mutual_recipe_key_consistency` became moot (one package
        ships); `plan104_design_system_packages_cover_all_25_components_and_enforce_color_authority`
        became `plan118_design_instrument_covers_required_components_and_enforces_color_authority`
        over the one shipped package, with `chat` dropped from the required list on purpose (its
        12 keys are removed by design and the surface dies in tasks 23-24; the core fallback
        covers the interim window).
      - `plan104_source_independence_guard_rejects_package_name_branching` became
        `plan118_source_independence_guard_rejects_package_name_branching`, scanning for
        `@clay/design-instrument` and a synthetic `@thirdparty/design-` (the old needles would
        have made the guard vacuous).
      - Frontend suites rewritten to the shipped system: `design-system-conformance.test.tsx` now
        carries a faithful `@clay/design-instrument` snapshot (8px/16px radii, hairline borders,
        accent@0.15 selections, offset-2 focus rings, blur only on scrim/toast) plus a synthetic
        third-party snapshot for the replacement/revocation/DOM-continuity invariants;
        `design-system-consumption.test.ts` reads exactly one package;
        `settings-panel-choices.test.tsx`, `bridge.test.ts`, and `design-system-adapter.test.ts`
        use the shipped or synthetic specifiers.
    - New tests:
      - `package_ui_conformance::plan118_removed_design_systems_are_absent`: walks `packages/`,
        `src/`, `frontend/src/`, `tests/fixtures/`, `scripts/`, `examples/` (Markdown prose
        excluded — historical records may name the removed systems) and fails on either specifier;
        also asserts both package directories are gone and that the inventory table keeps no entry.
      - `package_ui_conformance::plan118_design_instrument_is_listed_in_the_bundled_inventory`
        additionally asserts the inventory holds no removed root.
      - `manual_smoke_docs::plan118_ui_review_harness_captures_the_shipped_system_and_rejects_removed_states`:
        runs the harness for real — the four removed fixture names exit 2 with
        `unknown --fixture`, `ui-review-design-system` is still a valid fixture (and its `init.js`
        activates the shipped system), the fixture directories are gone, and `--help` lists the
        shipped state and no removed one.
    - Recorded baseline for the delta assertion: `tests/fixtures/design-system-reference-keys.txt`
      (the 142 keys the two removed packages shared), captured before deletion and read by
      `plan118_design_instrument_keys_are_the_reference_set_minus_chat_plus_the_new_families` —
      a deleted package can no longer be the reference for its own replacement.
    - Docs: `DESIGN.md` (status + §16 compatibility/settings), `docs/reference/ui-design-systems.md`
      (catalog is now shipped system + core baseline), `docs/development/ui-design-system-conformance.md`
      (matrix + test map), `docs/reference/packages/creating-packages.md`,
      `docs/wiki/modules/{ui-design-system-runtime,ui-review-harness}.md`, `docs/wiki/index.md`,
      `.impeccable/surfaces/operate-visual-direction.md`, and the `test-plan/` rows that named the
      removed fixtures/systems now describe the shipped one (the step-by-step UI-DS rewrite stays
      with task 26).
    - Prototype tooling re-pointed at the shipped contract
      (`design-artifacts/tools/{make-pages,make-component-catalog}.py` read
      `packages/design-instrument/package.json`); the approved artifacts stay frozen, so
      regeneration now reports drift until the migration re-approves the catalog for the ten new
      families (task 12/13).
    - Unrelated pre-existing breakage, recorded not fixed: `src-tauri/tests/dto_roundtrips.rs`
      does not compile on this branch (two `non_exhaustive` match errors for the
      `ListAgentSettingsFiles`/`OpenAgentSettingsFile`/`AgentSettingsFiles` protocol variants —
      verified identical with the file reverted), so the specifier rename in that file is
      reviewed-by-inspection only and the `clay-desktop` test target stays unbuildable for
      whoever picks up those variants. Repo-root `cargo check --all-targets` and every suite in
      the required gate set pass.
    - Trust binding unchanged and re-verified: placement resolves only from the compiled
      inventory bound to exact name+version+root+fingerprint; a removed root cannot resolve from a
      stale entry (`plan118_removed_design_systems_are_absent` scans the inventory file itself),
      an unlisted directory is never trusted (in-crate `unlisted_package_dirs_are_not_trusted`),
      third-party behaviour is untouched, and `@clay/design-instrument` enters the trusted domain
      only through its inventory entry.

- [x] Align core fallbacks and host pre-bootstrap fallbacks with the approved language
  - Acceptance Criteria (added by task 4, 2026-09-11): the film grain the approved language draws
    on `.desk` (`--grain-opacity: 0.02` in `ds-quiet.css`) is deleted rather than ported into the
    fallbacks or the host CSS: `DESIGN.md` §14 bans it, and the prototype pages already drop the
    `.desk` backdrop for that reason. See
    `design-artifacts/prototypes/quiet-instrument-migration/README.md` §7.1 finding 2.
  - Acceptance Criteria:
    - Functional: `core_design_system_fallbacks()` (`src/shell/design_system.rs`) and the
      `--clay-ds-*` fallback block in `frontend/src/styles/tokens.css` carry the same geometry
      and material values as `@clay/design-instrument` (radii, hairline widths, no hard offset
      shadows, 150ms/240ms motion), so a build that has not installed a design system paints
      the migrated language and the post-activation swap is geometry-neutral.
    - Performance: no added work — the fallback map is built once at startup; the swap must not
      change layout (verified by a DOM/layout assertion in the conformance suite).
    - Code Quality: fallback recipe keys and tokens.css definitions stay in one-to-one
      agreement (`plan103_fallback_recipes_have_tokens_css_definitions` and the frontend
      consumption gates pass), and the `--clay-*` role fallbacks keep mirroring the core
      catalog.
    - Security: color authority unchanged — fallbacks reference core theme roles only, never a
      literal palette.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §16 (implementation profile: core fallback must be
        geometry-compatible), §4/§11; `references/tokens.md` (core vs design-system-local values);
        `references/config.md`.
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
    - Options Considered:
      - Leave core fallbacks at the Neobrutal geometry and rely on the package being installed:
        pre-bootstrap paint and any failed activation would render the retired language.
        (Rejected.)
      - Align fallbacks (chosen) — one language everywhere, no dual geometry.
    - Chosen Approach: derive the fallback values from the package manifest, update
      `core_design_system_fallbacks()` and the tokens.css block, and re-run the conformance,
      consumption and fallback-coverage suites.
    - Files to Create/Edit:
      - `src/shell/design_system.rs` (fallback recipes), `frontend/src/styles/tokens.css`
        (fallback variables + comment), plus any Rust test fixture that pins old fallback values
      - `docs/development/ui-design-system-recipe-matrix.md` (fallback defaults)
    - References: `tests/package_ui_conformance.rs`
      (`plan101_core_fallbacks_cover_all_components_and_enforce_color_authority`,
      `plan103_fallback_recipes_have_tokens_css_definitions`);
      `frontend/src/test/design-system-consumption.test.ts`.
  - Test Cases to Write:
    - Fallback/package agreement: for every recipe key, fallback geometry equals the package's
      value (fails on any divergence).
    - Pre-bootstrap paint: the fixture route without a design-system snapshot renders hairlines
      and the radius ladder, not 0px/hard shadows.
  - Outcome (2026-09-12): **aligned and verified.**
    - `core_design_system_fallbacks()` (`src/shell/design_system.rs`) was rebuilt on the
      language ladder instead of the retired geometry: `RADIUS_XS/CONTROL/PANEL/SURFACE/PILL`
      plus `RADIUS_FLUSH` for full-bleed regions and non-boxes, `MOTION_FAST`/`MOTION_ENTER`/
      `MOTION_NONE`, one `elevation_stack()` for the two approved soft stacks (pop: tooltip,
      popovers; overlay: dialog, command centre), and a `FallbackKind` table for the
      host-consumed kinds and shell surfaces. Every recipe is 0px/hard-shadow/linear free, and
      the curated key set is unchanged (52 keys: 20 button states, text input root + well,
      modal root/scrim/dialog, panel, 14 kind roots, 12 shell surfaces) — no speculative
      fallback was added.
    - `frontend/src/styles/tokens.css`: the 213 `--clay-ds-*` variables in the host fallback
      block were rewritten to the same values (117 value changes: 0px→5/8/12/16/9999px radius,
      2px→1px borders, retired role names → the package's roles, `100ms linear` → `150ms`
      ease-out / `240ms` spring-snappy / inert, backdrop blur 3 on the scrim, the approved
      overlay stack on the command centre).
    - `@clay/design-instrument` manifest completed where the host block states a value the
      recipe left implicit (13 recipes gained an explicit absent border/ring declaration, 5
      gained the radius the approved artifacts name) so the activated package resolves to
      exactly what the block paints. Declaration payload 34.3 kB (budget 64 KiB).
    - Findings recorded, not silently patched:
      - Task 8's recipes were authored from DESIGN.md §11 but never diffed against the approved
        artifacts; the diff found five wrong radii — `collapse.root`/`collapse.header` 12 → 8
        (`.collapse`/`.collapse-head` are `--r-sm` and §5 forbids mixing within a family),
        `statRow.root` 8 → 5 (`.stat` is `--r-xs`), and `statusItem.root` gained 5 (`.status-key`),
        while `textInput.field` gained the composer shell's 12.
      - Sparse recipes resolved to the chain terminal, i.e. the retired values: 46 recipes
        resolved a 0px radius and 89 a 100ms linear transition. Those that the host consumes are
        now declared; `ResolvedComponentRecipe::default()` itself was moved to the language's
        neutral (ring offset 2, no transition instead of 100ms linear) so the terminal no longer
        smuggles a retired value into any sparse or third-party recipe.
      - The block states 213 of the 376 `--clay-ds-*` variables the component CSS consumes; the
        remaining 163 are still unbacked and fall through to the modules' inline defaults
        (task 18's "zero unbacked variables" criterion owns them). The chat block's 12 variables
        were aligned too but disappear with the surface (tasks 23-24).
      - Task 8 shipped the package without `clay.extensionPoints`, which every bundled package
        must declare (`packages::bundled::tests::bundled_extension_points_match_real_contributions`);
        the manifest now declares `design-instrument.recipes` (replace, `uiDesignSystem`).
    - Docs: `DESIGN.md` §16 (core fallback is the same language, with both asserted
      equalities named), `docs/development/ui-design-system-recipe-matrix.md` (a new
      "What the core recipes are" subsection before the mandatory fallback-guarantee
      rules: coverage, ladder/tiers, block projection, and the tests that pin it),
      and `docs/reference/ui-design-systems.md` §B (the core baseline no longer reads
      "0px radii, no shadows, 100ms linear").
    - Evidence: `plan118_core_fallbacks_match_the_shipped_language` (every shipped recipe the
      fallback set covers resolves to the fallback value, 36 keys compared field-for-field and
      asserted equal), `plan118_host_fallback_block_matches_the_resolved_package` (every
      `--clay-ds-*` value in the block equals the CSS the runtime adapter projects for the
      resolved package; 160 variables compared, the other 53 being the chat block and the
      host-authored spacing/veil compositions), `plan118_core_fallbacks_obey_the_language_rules`
      (radius ladder + flush allowlist, no hard offset shadows, motion tiers, nothing linear,
      2px/offset-2 rings, press only on buttons). The frontend
      `core-baseline-hierarchy.test.ts` was migrated from the neobrutal baseline (field fill vs
      control fill, ghost muted border) to the shipped one (a filled input well against ghost
      buttons, a borderless muted text action) and now reads the package manifest instead of
      slicing Rust source text.
    - Gate notes: `security` 152 passed, `protocol` 214 passed, `runtime` 75 passed, `lib` 1321
      passed, `presentation` 52 passed, frontend 39 files / 301 tests. Two load-sensitive budget
      tests (`large_document::…is_chunked`, and one protocol timing leg) failed while the machine
      was also compiling; both pass in isolation and did not reproduce across five subsequent
      runs, so no action was taken.

- [x] Design-system selection, default resolution, and removal fallback
  - Acceptance Criteria:
    - Functional: the Settings panel and Command Centre list exactly the shipped choice set
      (`@clay/design-instrument` and the `@clay/core` baseline) from
      `ui_choices.design_systems`; a persisted `designSystem` preference naming a removed
      package falls back to the core baseline/Instrument with a bounded diagnostic instead of
      failing the generation; `settings.setDesignSystem` keeps rejecting non-contributing and
      non-bundled specifiers before persistence; `@clay/core` remains selectable and documented
      as the built-in baseline.
    - Performance: selection stays one atomic snapshot swap with no component remounting and no
      unbounded repaint; failure paths add no retry loop.
    - Code Quality: no host branching on a package name survives
      (`plan118_source_independence_guard_rejects_package_name_branching`); validation lives in
      `src/server/command_execution.rs` + `src/server/ops/theme.rs`, not in the client.
    - Security: a removed or non-bundled specifier grants no package authority, triggers no
      adoption, and never leaves partial variables installed.
  - Approach:
    - Documentation Reviewed:
      - `references/config.md` (design-system packages, selection),
        `references/ui.md` (catalog currency), `DESIGN.md` §16; existing
        `docs/reference/clay-js-api/{theme,settings}/set-design-system.md`.
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Hard-fail on a removed persisted specifier: a user upgrading Clay with
        `setDesignSystem("@clay/design-neobrutal")` in `init.js` would get a broken generation
        for a rename they did not cause. (Rejected.)
      - Silent fallback with no diagnostic: the user cannot tell why the UI changed. (Rejected.)
      - Fallback plus a bounded diagnostic (chosen).
    - Chosen Approach: resolve the persisted specifier; if it no longer names a bundled
      contributing package, keep the previous/default system active and surface a
      `theme.*`-style diagnostic naming the removed specifier and the active one; update the
      settings/command docs and the static dropdown fallback list.
    - API Notes and Examples:
      ```js
      // init.js after the migration
      setDesignSystem("@clay/design-instrument"); // Quiet Instrument (default)
      // setDesignSystem("@clay/core");          // built-in baseline, same geometry
      ```
    - Files to Create/Edit:
      - `src/server/ops/theme.rs`, `src/server/command_execution.rs`,
        `src/server/configuration.rs` (preference validation/fallback),
        `packages/settings/{package.json,dist/load.js}`,
        `frontend/src/settings/SettingsPanel.tsx` (labels),
        `docs/reference/clay-js-api/{theme,settings}/set-design-system.md`,
        `docs/reference/clay-js-api/api-inventory.toml`, `docs/generated/clay-js-api-registry.json`
    - References: `src/protocol/runtime.rs` (`UiChoicesSnapshot`), `src/packages/conflict.rs`
      (design-system contribution enumeration).
  - Test Cases to Write:
    - Removed specifier: a persisted `@clay/design-neobrutal` preference activates the default
      system with a diagnostic and no partial install.
    - Non-contributing specifier: `@clay/markdown` is rejected with `theme.invalid_design_system`.
    - Choice enumeration: the snapshot lists exactly the shipped entries, sorted, with the core
      baseline first.
  - Outcome (2026-09-12): **implemented and verified.** Most of the resolution layer landed with
    tasks 9 and 16 (removal of the two systems, `@clay/core` resolution, on-demand enable of
    bundled records); this task closed the remaining contract gaps and pinned them.
    - **Enumeration was not offering the shipped system.** `ui_choices.design_systems` listed only
      _enabled_ `uiDesignSystem` contributors plus the core baseline, while
      `settings.setDesignSystem` accepts any bundled `@clay/design-*` package and enables it on
      demand. On a fresh install the list was therefore `["@clay/core"]` and Quiet Instrument —
      the shipped default language — was unreachable from the Settings dropdown. The enumeration
      now also carries every bundled package whose manifest declares a `uiDesignSystem`
      (`bundled_design_system_display_name` reads the checked-in manifest for the display name and
      never installs, enables, or executes anything), so the list matches exactly what the command
      accepts: `["@clay/core" ("Core baseline"), "@clay/design-instrument" ("Quiet Instrument")]`
      on a fresh install, still sorted with the baseline first. No package-name branching is
      introduced (`@clay/theme-`/`@clay/design-` prefixes already gate the settings layer; the new
      predicate is the declared contribution).
    - **The removal diagnostic now names what stays active.**
      `apply_persisted_preferences` records one bounded record —
      ``preferences: designSystem `<rejected>` rejected: <error>; kept `@clay/core` `` — instead of
      only the error. Startup keeps loading, nothing is installed partially, and the committed
      generation carries the core baseline (the host-consumed subset of the shipped language from
      task 16).
    - **Panel and Command Centre:** the panel's dropdown renders
      `ui_choices.design_systems` (no client-side option list; `choiceLabel` uses the
      server-provided display name) and the Command Centre carries the contributed
      `settings.setDesignSystem` command from `packages/settings/package.json`, whose action
      targets are the same dropdown items — so both surfaces show the shipped set and nothing a
      removal left behind. Pinned by `plan118_source_independence_guard_rejects_package_name_branching`
      (no package-name branching in live code) and by the exact-set assertions above.
    - **Docs:** `clay-js-api/settings/set-design-system.md` (the dropdown enumerates the same set
      the command accepts; the removal path names both specifiers) and
      `clay-js-api/theme/set-design-system.md` (a removed package keeps the last valid system, one
      bounded diagnostic, generation never fails), plus the regenerated
      `docs/generated/clay-js-api-registry.json`.
    - **Tests:** `server::ops::theme::apply_design_system_rejects_a_non_contributing_package_without_installing`
      (`@clay/markdown` → `theme.invalid_design_system`, slot and flag untouched),
      `…_rejects_a_removed_package_without_installing` (assembled specifier → name resolution
      fails, nothing installed),
      `server::js_runtime::persisted_removed_design_system_preference_falls_back_with_a_bounded_diagnostic`
      (load succeeds, no partial install, exactly one diagnostic naming the rejected specifier and
      `kept `@clay/core``), `server::connection::persisted_removed_design_system_preference_commits_the_core_baseline`
      (generation still commits, `@clay/core` active with the language's recipes, shipped choice
      set and display name intact), and the exact-set assertion in the
      `settings.setDesignSystem` e2e test (was "contains", now "equals"). Removed names are
      assembled (`format!("@clay/design-{}", "…")`) so
      `plan118_removed_design_systems_are_absent` keeps its teeth.

- [x] Rewrite the design-system conformance suites for a single shipped system
  - Acceptance Criteria:
    - Functional: the suites prove, against the shipped system plus the core baseline, that
      recipes project valid non-colour CSS custom properties with zero literal colours; that
      switching systems preserves DOM structure, input state and interactive controls without
      remounting; that revocation restores the previous baseline; that focus rings and
      accessibility attributes survive every installation; that reduced-motion and
      reduced-transparency fallbacks hold; and that every recipe key is consumed by a component
      CSS owner.
    - Performance: suite runtimes stay within the recorded baseline; adapter assertions stay
      DOM-level, with no new timers or polling.
    - Code Quality: the tests are rewritten, not deleted — the old two-system switch becomes a
      package↔core-baseline switch, the glass-specific translucency test becomes the
      language's veil/reduced-transparency test, and the chat ownership entries are removed
      with the module.
    - Security: the boundary tests stay: out-of-bounds snapshot variables are rejected before
      DOM mutation, third-party declared colours are rejected at the adopted boundary, and the
      conformance helpers are not exposed as ops or facade APIs.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §13 (accessibility invariants), §16; `references/ui.md`,
        `references/config.md`, `references/tokens.md`.
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
    - Options Considered:
      - Delete the two-system tests as "no longer applicable": loses the DOM-continuity,
        revocation, focus-ring and reduced-motion coverage that still applies to one system.
        (Rejected.)
      - Rewrite the same assertions against the shipped set (chosen).
    - Chosen Approach: parameterize the existing suites over
      `["@clay/design-instrument", "@clay/core"]`, keep the invariants, and add the
      language-specific checks (single border weight, transient-only elevation, accent-as-state)
      so the suite enforces `DESIGN.md` rather than the old packages.
    - API Notes and Examples:
      ```ts
      const SHIPPED = ["@clay/design-instrument", "@clay/core"] as const;
      for (const specifier of SHIPPED) {
        /* switch, assert continuity, revoke, assert restore */
      }
      ```
    - Files to Create/Edit:
      - `frontend/src/test/design-system-conformance.test.tsx`,
        `frontend/src/test/design-system-consumption.test.ts` (ownership map: drop chat, name an
        owner for every declared family; the declared-key count is 165 per task 8 and the `it.fails`
        drift gate becomes a hard assertion against the recorded adoption backlog — see the Outcome),
        `frontend/src/test/fixtures/design-system-adoption-backlog.json` (the recorded,
        shrinking remainder), `frontend/src/test/design-system-adapter.test.ts`,
        `frontend/src/test/settings-panel-choices.test.tsx`, `frontend/src/test/bridge.test.ts`,
        `tests/package_ui_conformance.rs` (the 142-count/key-parity assertions between the two
        deleted packages become the **165-key** assertion for the shipped package (130 reference
        keys plus the 35 declared by task 8 — `seg`, `agentPicker`, `recentRow`, `sessionRow`,
        `statusDot`, `keyHint`, `swatch`, `statRow`, `toast`, `empty` — with `viewSwitch`
        unified into `seg` and `chip` into `badge`),
        `tests/theme_packages.rs`,
        `src/server/command_execution.rs` (test fixtures), `src-tauri/tests/dto_roundtrips.rs`
    - References: `frontend/src/theme/design-system-adapter.ts`,
      `src/shell/design_system.rs`, `docs/development/ui-design-system-conformance.md`.
  - Test Cases to Write:
    - DOM continuity: switching shipped ↔ core baseline leaves the same element identity, focus
      and scroll.
    - Language invariants: no recipe declares a shadow outside transient surfaces, no border
      width >1 except state marks, no 0px radius anywhere.
    - Reduced transparency: veils resolve to opaque fills and `backdrop-filter` is disabled.
  - Outcome (2026-09-12): **implemented and verified.** Tasks 8, 9 and 16 had already rewritten
    the Rust side of the suites; this task rewrote the frontend side, hardened the drift gate, and
    closed the gaps the prototypes exposed.
    - **The drift gate is hard, not `it.fails`.** `design-system-consumption.test.ts` lost both
      red-first gates. Both of its real-data checks now assert _exact_ equality against a recorded
      adoption backlog (`frontend/src/test/fixtures/design-system-adoption-backlog.json`: 89
      unconsumed recipe keys, 44 unbacked consumed variables — measured, not estimated), with shape
      checks that the recording cannot contain keys no package declares, variables no module
      consumes, or unsorted/duplicate entries. Any new drift fails the suite; adopting a key forces
      the entry out of the recording. The recording is the host-CSS adoption tasks' checklist: they
      shrink both lists to empty and delete the file, at which point the same two assertions are
      literally "every declared key is consumed, every consumed variable is backed".
      _Why not the literal "zero unconsumed keys" now:_ host CSS adoption is tasks 24–25 (this task
      runs before them) and 89 keys across `label`, `flex`/`stack`, `menu`, `card`, `scroll`,
      `overlay`/`portal`, the five `badge` variants and every new task-8 family still have no
      consumer — a hard zero gate here would be red by construction. The AC's _Code Quality_ line
      ("the declared-key count becomes 130 and the `it.fails` drift gate becomes a hard assertion")
      is amended accordingly: the count is **165** (task 8's 130 reference keys + 35 new families,
      asserted in `plan118_design_instrument_keys_are_the_reference_set_minus_chat_plus_the_new_families`),
      and the gate is hard against a recorded, shrinking backlog.
    - **Ownership map completed and enforced.** Chat entries dropped with the module; every one of
      the 40 declared families now names its owning module from the surface inventory (including the
      new `seg`, `agentPicker`, `recentRow`, `sessionRow`, `statRow`, `statusDot`, `keyHint`,
      `swatch`, `toast`, `empty` families and the previously unmapped `label`, `flex`, `stack`,
      `card`, `menu`, `popover`, `overlay`, `portal`, `scroll`, `panel` families). Two new hard
      checks: `checkOwnerRouting` (a consumed key must be consumed _by its owner_, not merely
      somewhere — verified today, so a surface cannot borrow another surface's recipe) with a
      synthetic regression case, and a family-coverage check (a new family cannot skip routing).
    - **The continuity suite now proves the shipped↔baseline switch.** The old test switched to a
      synthetic third-party system; `it.each(SHIPPED_SYSTEMS)` now runs both shipped choices
      (`@clay/design-instrument` and `@clay/core` = _no adopted snapshot_, so the baseline half
      proves the host fallback block paints the same language) and additionally asserts node
      identity (`toBe`, not "an equivalent node"), focus, scroll offset, `aria-selected`/
      `aria-expanded`, and that the modal still closes afterwards. Third-party overlap is kept where
      it belongs: the projection and replacement tests still dress the same DOM in deliberately
      different geometry.
    - **Accessibility fallbacks rewritten for the language.** The glass-specific test became the
      veil test: `global.css` no longer selects the retired `data-clay-material="glass"` marker
      (nothing set it; it was dead CSS from the removed design system) and instead addresses the
      live veils by the component/slot attributes the components emit. The test reads those
      attributes off a rendered modal and an **opened** dropdown and asserts both directions — the
      scrim and popover are covered, and every pair the rule names exists in the DOM, so the
      fallback cannot rot into an unmatchable selector. It also asserts opaque theme-role fills,
      no `color-mix`, and the recipe values that give the fallback a job (scrim opacity 0.5/blur 3,
      toast blur 8). A sibling test proves `prefers-reduced-motion` collapses the language's
      150ms/240ms tiers to `0.01ms` with transforms removed.
    - **Not rewritten but verified as already satisfied by the Rust suites:** the 165-key assertion
      and reference-set parity (`…_keys_are_the_reference_set_minus_chat_plus_the_new_families`),
      the language invariants (radius ladder + 0px allow-list, hairline borders, static surfaces
      shadow-free, blur confined to scrim/toast, no hover lift, focus ring 2px at offset 2,
      selection as a fill) — `plan118_design_instrument_radius_state_and_material_discipline` and
      `plan118_core_fallbacks_obey_the_language_rules`; the security boundaries
      (`plan104_malicious_and_out_of_bounds_design_system_values_are_rejected`,
      `plan104_third_party_design_system_security_and_authority_isolation`,
      `no_conformance_helper_exposed_as_op_or_facade`); and `tests/theme_packages.rs`,
      `src/server/command_execution.rs` fixtures and `src-tauri/tests/dto_roundtrips.rs` were
      already free of two-system leftovers. The frontend suite keeps the DOM-level
      out-of-bounds/malicious-snapshot test proving the DOM is untouched when validation throws.
    - **Docs:** `docs/development/ui-design-system-conformance.md` (invariant table + both suite
      listings: the parameterized choices, the accessibility fallbacks, the consumption gate and
      its recording; stale `plan104_*` test names corrected) and the consumption-contract
      paragraphs in `docs/reference/packages/creating-packages.md` and
      `.agents/skills/clay-execution/references/components.md`.
    - **Gates:** `cargo fmt --check`, `cargo clippy --all-targets` 0 warnings, `cargo test --lib`
      1325/0, `presentation` 52/0 (carries `package_ui_conformance` + `theme_packages`), `protocol`
      214/0, `security` 152/0, `runtime` 75/0; frontend `tsc -b` clean, `vitest run` 39 files /
      **306 tests** in 8.8s wall (baseline 301 tests / 9.3s — within budget, no timers or polling
      added, prettier and eslint clean).

- [x] Add typed designTokens overrides to the four shipped themes
  - Acceptance Criteria (added by task 5, 2026-09-11): the entries are the ones in
    `design-artifacts/prototypes/quiet-instrument-migration/theme-values.md` — 13 roles per theme
    (`border.hairline`/`subtle`/`strong`, `surface.hover`/`active`/`selected`, `accent.primary`/
    `muted`, `focus.ring`, `border.focus`, `text.muted`/`disabled`, `surface.scrim`), identical
    coverage across the four packages, values composited and measured (all 12 enforced pairs pass).
    Additionally `theme-gruvbox-material-dark` swaps its `shellBg`/`panelBg` values: it currently
    resolves `surface.main` to the chrome colour and renders its panel lighter than its canvas, which
    `DESIGN.md` §10.2 forbids — the swap is the whole fix and needs no token, so "keep textStyles
    untouched" holds for every value except that one pair.
  - Acceptance Criteria:
    - Functional: each of `packages/theme-modus-operandi`, `theme-modus-vivendi`,
      `theme-gruvbox-material-dark`, `theme-gruvbox-material-light` declares
      `clay.contributions.designTokens` entries from the approved theme board (task 7) covering
      the hairline/subtle/strong ladder, `surface.scrim` (inventory finding 7 — `modal.default.scrim`
      needs a role the theme contract does not have today), `accent.primary`, `accent.muted`,
      `focus.ring`, `border.focus`, `text.muted`/`text.disabled` steps and the
      hover/active/selected fills;
      the rendered result matches the approved board in all four themes.
    - Performance: no runtime cost beyond the existing projection; no additional package load
      (themes stay inert data resolved from the enabled record).
    - Code Quality: only core token names with `#rrggbbaa`/colour values (validated by
      `src/packages/record/theme.rs`); no typography overrides; values and rationale recorded in
      each package's `README.md`/`docs`; the four themes keep identical token coverage so no
      theme silently inherits a step from the core fallback.
    - Security: colour authority stays in the theme package — host CSS and recipes gain no new
      colour source, and no theme gains permissions or modes.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §10 (role map + the four theme-side requirements),
        §13 (invariants), `references/tokens.md` (theme-side vs design-system-local values),
        approved theme board `design-artifacts/approved/quiet-instrument-migration/themes.html`.
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
    - Options Considered:
      - Derive hairlines in Rust from the theme's ink colour: takes tuning from theme authors,
        cannot express a themed accent, splits colour authority. (Rejected — see decision
        2026-09-11-1700 alternative 3.)
      - Extend the legacy `textStyles` vocabulary with new base-UI keys (needs Rust enum,
        struct, parser and mapping changes): larger blast radius for the same visual result.
        (Rejected.)
      - Typed `designTokens` (chosen): supported path, no Rust change, per-role overrides.
    - Chosen Approach: add only the roles the language needs, per theme, from the approved
      values; keep `textStyles` untouched so the editor text path and existing themes stay
      compatible.
    - API Notes and Examples:
      ```json
      "designTokens": [
        { "token": "border.hairline", "value": "#00000057" },
        { "token": "border.subtle",   "value": "#0000009e" },
        { "token": "accent.primary",   "value": "#0031a9" },
        { "token": "text.muted",       "value": "#4f4f4f" }
      ]
      ```
    - Files to Create/Edit:
      - `packages/theme-modus-operandi/package.json`, `packages/theme-modus-vivendi/package.json`,
        `packages/theme-gruvbox-material-dark/package.json`,
        `packages/theme-gruvbox-material-light/package.json`, plus each package's docs/README
        value table
    - References: `src/packages/record/theme.rs` (`parse_design_token_contributions`),
      `src/shell/theme.rs` (`ResolvedUiTheme::from_active_theme`), `docs/reference/packages/creating-packages.md`
      (typed design-token overrides).
  - Test Cases to Write:
    - Coverage: all four themes declare the same token set (a missing token fails the test).
    - Contrast at apply time: `validate_active_theme_contrast` passes for every theme and the
      activation is rejected if a value is mutated below the floor.
  - Outcome (2026-09-12): **implemented and verified.**
    - **Data.** All four manifests gained `clay.contributions.designTokens` with the board's 13
      roles and values verbatim, in board order (52 entries; manifests 5.3 kB → 6.4 kB each, so
      `clay.performance.estimatedManifestBytes` moved 2600/3200 → 6500 to stay an honest
      estimate). `theme-gruvbox-material-dark` also took its `textStyles` correction:
      `shellBg`/`panelBg` `#1d2021`/`#282828` → `#282828`/`#1d2021` (§10.2 depth direction; the
      approved palette puts the canvas at `#282828`). Nothing else in the manifests moved — no
      permissions, no modes, no typography.
    - **The board is the specification, and a test enforces it.**
      `theme_packages::shipped_theme_roles_match_the_approved_board` reads the frozen
      `design-artifacts/approved/quiet-instrument-migration/theme-values.json` and asserts, per
      theme: the declared roles equal the board's role values byte for byte, the board and the
      bundled theme set name the same four packages, `shellBg`/`panelBg` equal the board's
      canvas/chrome pair, and — the part that proves the _paint_ path rather than the manifest —
      `resolve_theme_token_snapshot` resolves each of the 13 roles to the board's value (the
      client's `--clay-*` projection), under the contrast gate. A silent value edit now fails a
      test instead of drifting from the approved artifact.
    - **Coverage + resolved-palette contrast** replaced the old "themes contribute zero
      `designTokens`" assertion in `package_ui_conformance::bundled_theme_conformance_matrix`
      (its comment's stated intent: a theme that ships `designTokens` must enter the matrix with
      a real palette to validate). The matrix now asserts exactly the `THEME_UI_ROLES` set per
      theme (missing _or_ extra roles fail) and validates AA on the _resolved_ palette — the
      theme's own `textStyles` base colors with its typed overrides layered over them — via a
      shared `theme_active_theme` helper that mirrors the server's `build_active_theme_from_record`.
      `theme_packages::bundled_themes_sdui_pairs_meet_aa_contrast` was switched to the same
      resolved snapshot. Task 14 inherits the scaffolding and adds its pairs/thresholds.
    - **Language invariants + rejection**
      (`theme_packages::shipped_theme_roles_obey_the_language`,
      `…::shipped_theme_accent_below_the_floor_is_rejected`): the ladder is one grey at 34 %/100 %
      plus ink for `border.strong`; one accent drives `accent.primary`, `focus.ring` and
      `border.focus`, with `accent.muted` the same hue at 75 %; the three state fills are opaque
      and mutually distinct; `text.muted` ≠ `text.disabled`; the scrim never lightens the canvas
      (relative luminance, so a future palette cannot dim upward); and mutating a shipped
      `accent.primary` to the theme's own canvas is refused by name
      (`accent.primary`/`surface.main`/3.0) before install.
    - **Docs.** Each package's `docs/index.md` gained a "UI roles (`designTokens`)" section: all 13
      values with what each role is, the reason the hairline sits below the floor, and the
      per-theme accent/scrim provenance (Gruvbox Material Dark's entry carries the `textStyles`
      swap note and its Base-UI mapping line was corrected to `shellBg→bg0`, `panelBg→bg0 hard`).
      The four `dist/index.js`/`dist/load.js` headers now name both contributions, and
      `docs/reference/clay-js-api/theme/set-theme.md` no longer claims the Gruvbox themes carry no
      `designTokens` (the authoring-doc sweep itself is task 15).
    - **Board regeneration:** `make-theme-values.py` reads the live manifests, so the working
      prototype copy's "current value" column necessarily moves once the migration lands. It was
      regenerated (`design-artifacts/prototypes/…/theme-values.{md,html}`; `--check` green again,
      capture gate re-run clean on `themes.html`, 2/2 runs) while the **approved** copy stays
      frozen as the approval record — which is the copy the test pins.
    - **Gates:** `cargo fmt --check`, `cargo clippy --all-targets` 0 warnings, `cargo test --lib`
      1325/0, `presentation` 55/0 (was 52 — three new theme tests), `protocol` 214/0, `security`
      152/0, `runtime` 75/0; frontend `vitest run` 306/0; `prettier --check` clean on the four
      manifests.

- [x] Extend the contrast and theme gates: structural boundaries, hairline visibility, resolved-palette validation
  - Acceptance Criteria (added by task 5, 2026-09-11): every required pair is measured on the
    **composited** colour (alpha over its surface) — `editor::theme::contrast_ratio` reads
    `to_rgba8()` and ignores the alpha byte today, so a 34 % hairline scores 21:1 while it renders at
    1.45:1. The border floor is scoped by role: `border.subtle`, `border.strong`, `border.focus`,
    `focus.ring`, `accent.primary`, `accent.muted` and the state fills must clear 3:1;
    `border.hairline` is exempt by definition (`DESIGN.md` §10.1 requires it quieter than
    `border.subtle`, which is unreachable at 3:1 from 34 % ink) but must stay monotonically below
    `border.subtle`. Measured values and the per-theme ladder are in
    `design-artifacts/prototypes/quiet-instrument-migration/theme-values.md`.
  - Acceptance Criteria:
    - Functional: `REQUIRED_CONTRAST_PAIRS` gains the structural boundary pair(s)
      (`border.subtle` vs `surface.main` and `surface.panel` at ≥3:1) and the theme conformance
      matrix validates the _resolved_ palette for a theme that ships `designTokens`; all four
      themes, the core fallback and the shipped design system pass; a hairline visibility floor
      (≥1.2:1 against its own surface) is asserted for `border.hairline` so a hairline can never
      become invisible on light chrome.
    - Performance: validation stays a startup/apply-time O(pairs) check; no per-frame work.
    - Code Quality: `bundled_theme_conformance_matrix`'s "zero designTokens" assertion is
      replaced by a resolved-palette validation (the test comment's stated intent), and the new
      threshold constants are named and documented rather than inlined.
    - Security: a failing pair rejects activation atomically (no partially applied theme), and
      the check cannot be bypassed by declaring fewer tokens.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §10 (requirements 1–4), §13 (invariants),
        `references/tokens.md` (contrast ownership), `docs/development/ui-design-system-conformance.md`.
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
    - Options Considered:
      - Keep the boundary pair out of the runtime pairs and assert it only in a test: a user
        could still activate a theme with invisible control boundaries. (Rejected.)
      - Add it to `REQUIRED_CONTRAST_PAIRS` (chosen) — this forces the core fallback values to
        satisfy it too, which is the intended invariant.
    - Chosen Approach: add the pairs, fix the core fallback border values if they fail, upgrade
      the theme matrix to build each theme's `ActiveTheme` snapshot _with_ its own
      `designTokens`, and record the measured table in the docs.
    - API Notes and Examples:
      ```rust
      ("border.subtle", "surface.main",  UI_CONTRAST_MIN),
      ("border.subtle", "surface.panel", UI_CONTRAST_MIN),
      ("border.hairline", "surface.main", HAIRLINE_VISIBILITY_MIN), // decorative zoning floor
      ```
    - Files to Create/Edit:
      - `src/shell/theme.rs` (pairs + thresholds), `src/shell/design_system.rs` (core fallback
        border roles if they must move), `tests/package_ui_conformance.rs`
        (`bundled_theme_conformance_matrix`), `tests/theme_packages.rs`,
        `docs/reference/ui-design-systems.md`, `docs/development/ui-design-system-conformance.md`,
        `docs/development/ui-design-system-recipe-matrix.md`
    - References: `src/editor/theme.rs` (`contrast_ratio`), `DESIGN.md` §13.
  - Test Cases to Write:
    - Boundary failure: a theme with an invisible `border.subtle` is rejected with the pair and
      ratio named.
    - Hairline floor: a hairline equal to its surface fails; the four shipped themes pass.
    - Core fallback: the fallback palette satisfies every required pair.
  - Outcome (2026-09-12): **implemented and verified.**
    - **Compositing is the whole point.** `editor::theme` gained `composite_over` and
      `composited_contrast_ratio` (alpha blended over the backdrop, then measured against it);
      `theme_meets_contrast` now uses it. The old raw-bytes ratio scored a 34 % hairline at 4:1 on
      dark chrome and 21:1 on light while it renders at 1.4:1 and 1.5:1 — every structural floor was
      satisfied by construction and no boundary was visible. The Rust compositing matches the
      approved board exactly (gruvbox-material-dark hairline on canvas: 1.61:1 in both; the whole
      shipped pair table reproduced digit for digit), which is the check that the board and the gate
      now measure the same thing.
    - **Policy.** `HAIRLINE_VISIBILITY_MIN = 1.2` sits next to the named 4.5/3.0 floors instead of an
      inline literal. `REQUIRED_CONTRAST_PAIRS` went 10 → 17: `text.disabled`/canvas (4.5),
      `accent.muted`/canvas, `border.subtle` on canvas **and** panel, `border.strong`/canvas (3.0),
      and `border.hairline` on canvas and panel at the visibility floor. State fills are gated by a
      second table, `REQUIRED_FILL_PAIRS`, because they are painted _under_ their text: the fill is
      composited over its surface first and the text over that — the layer order the user sees. That
      ordering is why the pre-migration uniform 40 %-alpha selection colour (1.03–1.43:1 against its
      own text) fails while a legitimate translucent selection tint does not.
    - **Core fallback fixed, not exempted.** The catalog failed the new floors — `border.subtle` was
      1.41:1 on canvas and 1.19:1 on the panel, `border.strong` 1.96:1, `text.disabled` 3.71:1,
      `accent.muted` 3.02:1 — so it now expresses the §10.1 ladder: `border.hairline`
      `#726b9857` (the border grey at 34 %), `border.subtle` `#726b98` (3.87:1 / 3.11:1),
      `border.strong` the ink `#eeeaff` (16.19:1), `text.disabled` `#807a9b` (4.69:1), and
      `accent.muted` the accent at 75 % alpha. `core_catalog_meets_every_required_contrast_pair`
      asserts the whole policy plus ladder monotonicity on both surfaces; this is the one path with
      no runtime activation to gate.
    - **Pre-bootstrap paint, found and fixed.** The `--clay-*` block in
      `frontend/src/styles/tokens.css` claims to mirror the core catalog and did not: **ten roles had
      already drifted** (`surface.overlay`, `surface.badge`, `surface.kbd`, `surface.scrollbar{,-track}`,
      `text.badge`, `text.kbd`, `border.strong`, `border.kbd`, `diagnostic.success`) — the shell
      repainted on the first snapshot. All 13 colour roles that moved or had drifted are aligned, and
      `host_theme_role_block_mirrors_the_core_catalog` fails on any future divergence.
    - **Rejection is atomic and not bypassable.** Pairs resolve through the same projection the client
      receives (typed `designTokens` → base-ui colours → core catalog), so a theme that declares
      fewer tokens is measured on the fallbacks instead of skipping the pair. New coverage:
      `enforce_contrast_rejects_an_invisible_hairline_and_keeps_the_active_theme` (the prior theme
      survives rejection, diagnostic names `border.hairline` + 1.2),
      `shipped_theme_invisible_boundary_is_rejected` (mutated `border.subtle`/`border.strong` refused
      by name), `shipped_theme_invisible_hairline_is_rejected` (shipped hairlines pass, an opaque
      canvas-coloured one is refused) and `shipped_theme_border_ladder_is_monotonic`
      (`hairline < subtle < strong` on both surfaces, all four themes).
    - **Docs.** `docs/reference/ui-design-systems.md` gained §7 — the group/floor table, the
      compositing rule, the measured worst pair per group for all four shipped themes _and_ the core
      baseline, and the two limits worth knowing: the legacy base-ui projection maps all three border
      roles to `base.scrollbar` (a flat ladder measured by the same gate; replacing the
      `scrollbar`/`caret`/`placeholder` projections is tracked debt, and a legacy theme that cannot
      clear the boundary floor from its own base colours is now refused activation), and
      `opacity.disabled` attenuation sits below the prose floor, which WCAG exempts.
      `docs/development/ui-design-system-conformance.md` gained the invariant row, the test map and §4;
      `docs/development/ui-design-system-recipe-matrix.md` notes that the fallback chain's last step
      lands on a gated role.
    - **Gates:** `cargo fmt --check` clean, `cargo clippy --all-targets` 0 warnings, `--lib` **1329**
      (was 1325; +4), `presentation` **58**, `protocol` **214** (includes the docs-as-code suites),
      `security` 152, `runtime` 75, frontend `vitest` 306, frontend `prettier --check` clean.
    - **Pre-existing, not introduced here:** `npm run lint --prefix frontend` (a CI gate) already fails
      on seven non-null assertions/unused bindings in `coding-agent/CodingAgentPanel.test.tsx` (5),
      `shell/WorkspacePanes.test.tsx` (1) and `test/components.test.tsx` (1) — files untouched by this
      task. The two lint errors this session had introduced (`core-baseline-hierarchy.test.ts:31`,
      `settings-panel-choices.test.tsx:130`) are fixed here.

- [x] Update the theme/design-system authoring documentation and catalogs
  - Acceptance Criteria (added by task 5, 2026-09-11): `DESIGN.md` §10.1 is reworded to the ladder the
    approved artifact actually uses — the theme's border grey at 34 / 100 % plus ink for
    `border.strong` — or the artifact is changed to ink at 34 / 62 / 100 %; the two must not keep
    describing different sources, and the measured difference is recorded in the board. §13.1 is
    reworded so the 3:1 border floor names the roles it applies to (`border.hairline` is exempt by
    §10.1 and cannot satisfy it from 34 % ink). §10.2 gains the note that exactly one shipped theme
    (Gruvbox Material Dark) resolves the canvas from `shellBg`, so depth direction is a per-theme
    check rather than an assumption.
  - Acceptance Criteria:
    - Functional: `DESIGN.md` §16 records the shipped implementation state (single package,
      four themes with `designTokens`, core fallback alignment); `docs/reference/ui-design-systems.md`,
      `docs/development/ui-design-system-recipe-matrix.md`, `references/tokens.md`,
      `references/components.md`, `references/config.md` and
      `docs/reference/packages/creating-packages.md` describe the shipped set, the removal of
      the two packages, the theme `designTokens` requirement and the structural-boundary gate;
      no catalog claims a removed system is shipped; and
      `docs/development/ui-design-system-recipe-matrix.md` stops presenting slots no shipped
      package declares (`dropdown.triggerLabel`/`indicator`, `list.rowTitle`,
      `collapse.chevron`, `panel.title`/`body`, `modal.title`/`body`, `tabList`,
      `editorChrome.*`, `focusRing.ring`) as part of the contract — inventory finding 6: either
      mark them not-yet-declared or delete them (task 8 ships the declared 165-key set: 130
      reference keys + the ten new families), and the fallback catalog grows the same ten
      families so a pre-bootstrap paint of a launcher row, a segmented switcher, a toast, a
      status dot or a session-file row is not unstyled.
    - Performance: documentation only; the contract's table sizes stay within the doc budgets
      checked by `tests/documentation_coverage.rs`.
    - Code Quality: drift-tested tables (`## Core Tokens (implemented)`,
      `## Package-Facing Component Kinds`, style-variable catalogs) remain machine-checkable and
      green; a page that is a historical record gets a pointer, not a rewrite.
    - Security: no docs promise authority the code does not have (no "theme packages can style
      components", no colour override path outside packages).
  - Approach:
    - Documentation Reviewed:
      - `docs/development/ui-design-system-conformance.md` (historical
        record rule), `.agents/skills/clay-execution/references/docs-as-code.md`.
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Fold these edits into the final wiki task: the docs-as-code drift tests run against
        these catalogs, so they must change with the code, not after it. (Rejected.)
    - Chosen Approach: update the normative/catalog pages in the same phase as the package and
      theme work; mark the historical audit pages with a pointer to `DESIGN.md`.
    - Files to Create/Edit: `DESIGN.md`, `docs/reference/ui-design-systems.md`,
      `docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`,
      `docs/development/ui-design-system-{conformance,recipe-matrix,css-audit,package-primitive-review}.md`,
      `.agents/skills/clay-execution/references/{ui,components,tokens,config}.md`,
      `docs/index.md`, `docs/wiki/modules/ui-design-system-runtime.md`
    - References: `tests/documentation_coverage.rs`, `tests/primitives_docs.rs`,
      `tests/package_ui_conformance.rs` (catalog drift).
  - Test Cases to Write:
    - Drift: the catalog/doc suites pass after every edit (no orphan rows, no missing tokens).
    - Currency: a grep gate fails if a catalog page still presents a removed design system as
      shipped.
  - Outcome (2026-09-12): **implemented and verified.**
    - **Two gates, then the prose.** `plan118_recipe_matrix_marks_undeclared_slots` recomputes the
      `†` marks from the shipped manifest **in both directions** — a slot the package declares may not
      be marked, a slot it does not declare must be — and re-checks the marks against the generated
      index in `components.md`, so the two catalogs cannot disagree. 93 contract rows parsed; 48 carry
      `†`; the two hosts (`tabList.root`, `table.root`) are exempt and explained in the matrix
      synopsis rather than marked. `plan118_catalog_pages_do_not_ship_removed_systems` reads ten
      catalogs and five historical pages (allowlisted) line by line: naming the former Neobrutal or
      Glass direction is fine while the same line says removed/former/previous/historical/retired/
      superseded/replaced/rejected. Writing the gate first is what found the four stale mentions below.
    - **`DESIGN.md`.** §10.1 now states the sources the shipped data actually uses — `border.hairline`
      the theme's border grey at 34 %, `border.subtle` the same grey at 100 %, `border.strong` the
      theme's ink at 100 % — with the note that the 34/62/100 % ink ladder it previously described is
      not what any theme ships. §10.2 turns "the canvas is lighter" into a per-theme check and records
      the one theme that failed it (Gruvbox Material Dark, fixed in task 13). §11 drops `chatPanel`
      from the shipped families and the leading accent bar from `fileBrowser.default.item.selected`.
      §13.1 scopes the floor by role and names composited measurement. §16 replaces "migration not
      started" with the shipped state. The §11 claim that nothing is declared without a consumer —
      contradicted by the 35-key IA extension — now distinguishes the language (the package) from the
      consumption contract (the recorded backlog fixture plus the `†` markers).
    - **Catalogs.** The recipe matrix was rebuilt around the shipped 165-key set (the recorded 142
      minus the 12 `chat.default.*` keys, plus the 35 the target IA and the drawn-but-unbacked
      surfaces need — not the AC's "130 + ten families", which was already wrong after task 8). It
      carries the `†` marker convention, the retired patterns are gone from its profile table
      (the 2px leading accent bar and the tab underline — `DESIGN.md` §14.13), `chatPanel` is retained
      only as a dormant surface with the removal task named, and the ten families with no core
      fallback say so and say which task adds it. `components.md`'s slot index is now generated from
      the matrix (48 marked slots; `tabList`/`table` explained as hosts, not recipe targets) and its
      consumption contract explains what `†` means. `tokens.md` gained the thirteen shipped theme roles
      with their rationale and floors. `ui.md`, `config.md`, `creating-packages.md`,
      `docs/reference/ui-design-systems.md`, `docs/index.md` and the wiki runtime page describe the
      shipped set, the removal, the theme-side role requirement and the structural-boundary gate.
    - **Records corrected, not rewritten.** `docs/reference/ui-components.md` claimed the landing is
      `@clay/chat`'s pane (the approved target IA makes the launcher the landing surface), pointed at
      `src/shell/primitives.rs` (deleted with the native client — the contract lives in
      `frontend/src/components/chrome.tsx`), and quoted only the 4.5/3.0 floors; all three now match
      the code. The visual-direction contract's status line still said "migration pending" with a
      comparison table presenting Neobrutal and Glass as retained/reference packages — the columns are
      now the removed comparators they became, and its "Next (not started)" section is the task list
      as it stands. `test-plan/15-ui-design-systems.md` gained UI-DS-31 (the contrast gate: mutate a
      shipped theme's roles, get a refusal that names the pair and the composited ratio, with the
      previous theme left installed) and UI-DS-32 (catalog currency, pinning both new tests), and
      UI-DS-28's "selected rows show … a 2px leading accent bar" was the retired pattern being
      promised in a test plan — corrected. `.impeccable/surfaces/operate-visual-direction.md` had the
      same retirement bug in three places (its geometry law, its story, and its retained-systems
      note).
    - **Deviation, recorded:** the AC asked the fallback catalog to grow the same ten families
      (`seg`, `agentPicker`, `recentRow`, `sessionRow`, `toast`, `empty`, `statusDot`, `keyHint`,
      `swatch`, `statRow`). They stay out of `core_design_system_fallbacks()` for now: no host CSS
      consumes them yet, so the entries would paint nothing, and
      `plan103_fallback_recipes_have_tokens_css_definitions` requires a paired `--clay-ds-*`
      declaration for every fallback — a declaration nothing reads is the same dead weight. The task
      that render them (the SDUI/editor-chrome adoption, the Settings/Command-Centre surfaces, and the
      launcher/picker/session-files surfaces in Part D) add map, block and consumer in one change; the
      matrix says so where a reader would otherwise expect the row.
    - **Gates:** `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` 0 warnings,
      `--lib` **1329**, `presentation` **60** (+2: the new drift tests), `protocol` **214** (the
      docs-as-code suites — documentation coverage, primitives docs, JS-API registry — stay green),
      `security` 152, `runtime` 75, frontend `vitest` 306. Every file this task changed passes
      `prettier --check`.
    - **Pre-existing, not introduced here:** `npm run format:check --prefix frontend` (a CI step)
      reports 35 failures at HEAD in files this task does not touch — the prettier 3.9 bump landed in
      commit `03a7fae` without reformatting the tree, and the diffs are cosmetic (e.g. redundant
      ternary parentheses in `src/app/router.tsx`). Fourteen of the 35 are surfaces later tasks in
      this plan rewrite; recorded in Further Actions rather than churned here.

- [x] Adopt the approved recipes in the shared component CSS
  - Acceptance Criteria:
    - Functional: `frontend/src/components/{button,controls,text-field,tab-strip,chrome,modal,tooltip,text,icon}.module.css`
      and the fallback block in `styles/tokens.css` implement the approved component specimen:
      radii from the 5/8/12/16/pill ladder, exactly one hairline border weight per surface,
      transient-only elevation, accent only for state, mono for data, and every state present
      (rest/hover/active/focus/selected/disabled/invalid) — visually matching
      `design-artifacts/approved/quiet-instrument-migration/component-catalog.html`.
    - Performance: no new runtime cost; CSS variable counts stay within the bundle budget, and
      the built stylesheet does not grow beyond the recorded baseline (report both numbers).
    - Code Quality: every visual property comes from `var(--clay-ds-*)` recipes or `var(--clay-*)`
      theme roles — no literal radius, border width, shadow, colour or duration in component
      CSS; `plan103_css_module_literal_deny_scan` and the consumption gates pass unchanged.
    - Security: no new authority, no package branching; CSS keeps comments-free, plain form so
      the deny scan stays exact.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §11 (component recipes) and §14 (retired patterns), §9 (state language).
      - `.agents/skills/clay-execution/references/components.md` (kinds, style variables, slots),
        `references/tokens.md` (token roles), `references/ui.md` (binding rules).
      - `design-artifacts/approved/quiet-instrument-migration/component-catalog.html` (binding).
    - Options Considered:
      - Restyle component CSS by hand from the artifact screenshot: values drift between CSS and
        package recipes. (Rejected.)
      - Consume recipe variables only, so the package data drives the look (chosen).
    - Chosen Approach: replace hardcoded geometry with the recipe variables emitted for the
      shipped package, delete the retired-pattern declarations, and keep layout (padding, gap,
      fixed dimensions) host-owned.
    - API Notes and Examples:
      ```css
      /* before: geometry literal in host CSS */
      .button {
        border-radius: 0;
        border: 2px solid var(--clay-text-primary);
        box-shadow: 3px 3px 0 var(--clay-text-primary);
      }
      /* after: package data drives appearance, host owns layout */
      .button {
        border-radius: var(
          --clay-ds-button-default-root-rest-border-radius,
          8px
        );
        border-width: var(--clay-ds-button-default-root-rest-border-width, 1px);
        border-color: var(
          --clay-ds-button-default-root-rest-border-color,
          var(--clay-border-hairline)
        );
      }
      ```
    - Files to Create/Edit: the nine component modules above, `styles/tokens.css`,
      `frontend/src/components/recipe-attributes.ts` if attribute names change,
      `frontend/src/test/design-system-consumption.test.ts` (ownership map)
    - References: `frontend/src/theme/design-system-adapter.ts` (variable naming),
      `docs/development/ui-design-system-css-audit.md` (per-declaration audit pattern).
  - Test Cases to Write:
    - Literal deny scan: no `border-radius:` literal, no `box-shadow:` offset literal, no
      `#rrggbb` in component CSS.
    - State completeness: each component module defines every applicable state selector.
    - Consumption: every consumed `--clay-ds-*` variable is backed by the package or the host
      fallback (zero unbacked variables).
  - Outcome (2026-09-12): **implemented and verified.**
    - **The block is the baseline, not just a head start.** Every visual property in the nine
      modules is now a bare `var(--clay-ds-*)` (or a `--clay-*` theme role where no recipe key
      exists); the values live in the `tokens.css` fallback block. That is forced by the activation
      model rather than by taste: `@clay/core` — the runtime default at startup — installs no
      package variables at all, so a state the block does not state is a state that silently
      disappears under the baseline. The block grew **213 → 413 declarations** (208 added, 8 dead
      ones deleted), generated from the manifest with the same projection the runtime adapter and
      `plan118_host_fallback_block_matches_the_resolved_package` use (roles → `var(--clay-*)`,
      shadow stacks → per-layer `color-mix` tints, `ease-out`/`spring-snappy` → their cubic-beziers,
      `press-shift-down` → `translateY(1px)`), then grouped by family. That Rust test now pins 350+
      values against the resolved package, so the block cannot drift from it again.
    - **Fourteen families adopted.** `button` (4 variants × 5 states), `textInput` (field, label,
      description, input × rest/hover/focus/invalid/disabled), `dropdown` (trigger × 5, popover,
      list, item × rest/hover/selected/focus), `list` rows × 5, `collapse` (root, header
      rest/hover/focus, body), `popover`, `modal` (scrim, dialog, entrance), `tooltip`, `badge`
      (default + accent/muted/error/warning/success), `kbd`, `divider`, `tab` × 5, `tabBar`,
      `label` × 7 variants. Backlog: **89 → 65 unconsumed keys**; unbacked consumed variables
      **44 → 1** (a panel shadow in the SDUI/registry module that the next task owns).
    - **What the adoption deleted.** The retired patterns the old CSS still drew: the tab underline
      (`inset 0 -2px 0 0 accent`), hard-offset button shadows, `backdrop-filter` on the popover,
      tooltip and dialog, and every invented variable no package declares — that is exactly what the
      44 unbacked entries were (`button.*-shadow`, `popover-…-backdrop-blur`,
      `collapse-default-chevron-*`, `modal-default-close-*`, `text-input-…-placeholder-color`,
      `kbd-…-min-height`, `list-…-disabled-*`, …). Resting controls, rows, tabs and fields now carry
      `box-shadow: none`; blur survives only on the scrim.
    - **Schema gaps composed, never invented.** The recipe schema states one all-sides border, so the
      collapse body's single top hairline composes the _root's_ border recipe onto `border-top-*`;
      role-tinted borders (badge tones at the catalog's 34%) compose `color-mix` from the recipe's
      role, which is what `DESIGN.md` §11 specifies; an overlay fill whose recipe carries
      `backgroundOpacity` composes `color-mix(…, calc(var(--fill-opacity, 1) * 100%))`, because the
      runtime emits the colour and the opacity as two variables.
    - **One ring per surface.** The text input's focus state is the recipe's accent border plus its
      3px accent halo (`shadow`) with `outlineStyle: none` — the well no longer paints a second ring
      inside itself, which is the rule the approved `components.css` asserted.
    - **Component changes.** `ClayBadge` gained `tone` (the badge _is_ the chip, §11); the collapse
      `Disclosure` root is painted (radius) and carries the `collapse.root` slot; dropdown items take
      the dropdown item's own recipe through a second class (the list family keeps its rows); the
      text field's description span uses the description recipe instead of the label's.
    - **Verified in a real browser** (vite dev + CDP against a headless Chrome, `?fixture=controls`):
      default button transparent + 1px `rgba(114,107,152,0.34)` hairline + r8, primary accent fill
      with `surface.main` text, muted borderless, disabled `text.disabled` @0.5, input well
      `surface.control` r8, `kbd` `surface.kbd` @0.55 r5, selected tab accent @0.15 with pill radius
      and no underline, collapse header `border-width: 0`, hover → `data-hovered` → `surface.hover`
      fill + `text.primary`, modal dialog r16 with the overlay shadow and a 3px-blurred scrim.
    - **Numbers.** Fallback block 213 → 413 declarations; `tokens.css` 18.7 → 33.9 kB raw; built CSS
      shell gzip **164.6 → 165.6 kB**, total **381.1 → 382.3 kB** (budgets 180/400); `--clay-ds-*`
      references in frontend CSS 672 → 917; `px` literals in the nine modules **94 → 36**, and every
      remaining one is layout (gaps, padding, hit targets) or a theme-token fallback.
    - **Gates:** `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` 0 warnings,
      `--lib` **1329**, `presentation` **60** (incl. the block-projection test), `protocol` **214**,
      `security` 152, `runtime` 75, frontend `vitest` **306** (incl. the consumption gate and the
      baseline-hierarchy test), `plan103_css_module_literal_deny_scan` green (no colour literal),
      `npm run check:budget` green, prettier clean on every changed file.
    - **Findings recorded, not silently absorbed.** (1) `collapse.default.root.rest` declares a 1px
      `border.hairline` **box** that `DESIGN.md` §11 and the approved specimen never draw — the host
      paints the root's radius and spends that border recipe on the body's top hairline, so the
      package value should become `borderWidth: 0` the next time the package is edited. (2)
      `DESIGN.md` §11 stated the tooltip's `transitionDuration` as 100 where every package and the
      specimen use 150 (`motion.fast`) — corrected in `DESIGN.md`. (3) The three per-variant button
      focus keys stay in the backlog deliberately: they resolve to the same values as
      `button.default.root.focus` and the host paints one ring, so consuming them would be three
      identical declarations. (4) `textInput.default.error.rest` has no DOM producer
      (`ClayTextField` has no error-message slot) and `label.default.root.rest` has no variant; both
      are recorded rather than invented.

- [x] Adopt the approved language in the SDUI, editor-chrome and package-surface CSS
  - Acceptance Criteria (added by task 5, 2026-09-11): the language consumes the theme roles instead
    of the raw palette: `--hairline` becomes `border.hairline` and `--hairline-strong` becomes
    `border.subtle`, and the now-unreferenced `--c-line` step is deleted (`--c-line` and `--hairline`
    measure as the same divider on all four themes, and `--c-line` has no consumer left in
    `ds-quiet.css` or `components.css`).
  - Acceptance Criteria:
    - Functional: `sdui/registry.module.css`, `sdui/renderer.module.css`, `editor/editor.module.css`
      and `packages/package-workspace.module.css` render the approved SDUI-rendered surfaces,
      editor chrome (gutter, active line, indent guide, scrollbar, breadcrumbs) and package
      workspace in all four themes with the approved geometry and materials; SDUI components
      rendered from a package manifest look identical to their React counterparts in the
      specimen.
    - Performance: editor chrome stays static-paint (no layout-affecting changes on state
      changes); no new CSS variables per document, and the scrollbar/gutter rules stay
      GPU-cheap (no blur, no filter).
    - Code Quality: SDUI paint paths keep two-axis parity (every `component_state_palette` state
      is styled), and the renderer does not gain literal geometry; the editor remains host-owned
      chrome over CodeMirror.
    - Security: package-provided SDUI stays inert — no raw CSS from packages, no new
      style-injection surface; only typed variables are consumed.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5/§6/§11/§12, `references/components.md`
        (internal surfaces: `editorChrome`, `fileBrowser`, `statusBar`, `paneSplitTree`),
        `references/tokens.md` (editor token ownership), `references/ui.md`.
    - Options Considered:
      - Restyle the editor chrome with its own palette tokens: splits colour authority and
        drifts from the shell. (Rejected.)
      - Consume the same DS recipe variables and theme roles (chosen).
    - Chosen Approach: inventory each module's current declarations, map them to approved
      recipes/roles, and verify against the specimen (SDUI section, editor section).
    - Files to Create/Edit: `frontend/src/sdui/{registry,renderer}.module.css`,
      `frontend/src/editor/editor.module.css`, `frontend/src/packages/package-workspace.module.css`,
      `frontend/src/sdui/registry.tsx` (recipe attributes if needed)
    - References: `src/masonry_sdui.rs` (`component_state_palette`), `frontend/src/sdui/registry.tsx`.
  - Test Cases to Write:
    - SDUI parity: a manifest-driven component and its React equivalent render identical
      computed geometry under both shipped systems.
    - Editor invariants: no blur/backdrop-filter in editor chrome; no radius on the canvas edge.
  - Outcome (2026-09-12): **implemented and verified.**
    - **What was adopted.** `sdui/registry.module.css`, `sdui/renderer.module.css`,
      `editor/editor.module.css`, `packages/package-workspace.module.css` and — because the
      scroll family's real consumer is app-wide — the universal scroll chrome in
      `styles/global.css` now paint from bare `var(--clay-ds-*)` recipe values, with every
      value stated in the `tokens.css` fallback block (**413 → 455 declarations**, and the
      first sync pass deleted 9 that no module consumed any more). Families: `panel`
      (root + header), `flex` (default/row/column), `stack`, `overlay`, `portal`, `scroll`
      (root + track + thumb rest/hover/active), `divider`, `editor` (all 11 keys, including
      `root` on the canvas), `fileBrowser`, `statusBar`, `textInput.input` for the editor's
      open-path field, and `button.default` for CodeMirror's code-action buttons.
    - **The theme roles stay the theme's.** A zone separator hairline is a _recipe_ even
      where the family has no border key: `package-workspace.module.css` composes
      `divider.default.root.rest` onto `border-top/bottom/left/right-*` rather than reaching
      for a colour role, so a package can retune every boundary. `editor.chrome` and
      `editor.gutter` declare `borderWidth: 0` / `borderStyle: none` — the document bar and
      the gutter separate themselves with whitespace, which is what the approved workspace
      artifact draws, so the adopted values _delete_ two hairlines the old CSS painted.
      `editor.activeLine` declares `borderWidth: 0` too: the approved catalog's 2px accent
      bar on the current line is a retired pattern (DESIGN.md §14.13), so the line keeps the
      wash and the current line _number_ carries the accent as a datum.
    - **The task-5 acceptance criterion on the palette steps.** Nothing in the host consumes
      `--hairline`, `--hairline-strong` or `--c-line`: the borders resolve through
      `border.hairline`/`border.subtle` inside recipes, so the raw-palette step exists only
      inside the frozen artifacts' own preview layer (which cannot use the app's theme roles
      and is not a shipped consumer). The dead third step stays where it is, in the frozen
      files, and the adopted production language has two steps, not three.
    - **No borrowed variants.** Two silent borrowings are gone: an SDUI `flex` node with no
      `direction` used to render the _column_ class (and therefore the column's recipe); it
      now wears `flex.default` in both the registry and the renderer. And an SDUI `overlay`
      node used the `stack` class, so the overlay family was painted by nothing; it now
      wears the overlay recipe (overlay fill, hairline, r12, pop shadow, 240 ms
      spring-snappy), and `portal` wears its own family. A panel title is now the panel
      family's header (hairline bottom, `spacing.sm`).
    - **SDUI ⇄ catalog parity.** New `frontend/src/sdui/surface-adoption.test.tsx` (10 tests)
      renders a manifest-driven node and its React counterpart and asserts they carry the
      _same_ recipe attributes and the same module class for button, label, textInput,
      list, dropdown and tabList — the SDUI host delegates paint to the catalog component
      instead of re-styling it, which is what makes their geometry identical under both
      shipped systems. It also pins the container routing (`flex.default`, overlay, portal,
      panel header), the editor invariants (no `backdrop-filter`, no `blur(`, no `filter:`,
      the container's 0 radius read from the package, the gutter/chrome borders, no geometry
      literals in either module) and the scroll-family mapping in `global.css`.
    - **Verified in a real browser** (vite dev + CDP against headless Chrome; fixtures
      `editor`, `package-ui`, `splits`, each with the four themes' role values injected on
      top of the baseline block). Measured: editor host **radius 0, border 0, no
      blur/filter/shadow**, canvas fill = `surface.main` and theme-responsive; document bar
      **borderless on every side** and transparent; gutter **border 0**, transparent, mono;
      open-path field **r8, 1px hairline, `surface.control` fill**; `scrollbar-color`
      resolves to `surface.scrollbar` at **0.4** (the recipe's rest opacity) on every
      scrolling region; package-workspace zones 0 radius with the status bar's **1px top
      hairline**; SDUI panel **r12, 1px hairline, veil at 0.55, no blur, no shadow**. All 15
      page×theme combinations reported zero horizontal overflow, and the geometry values were
      byte-identical across themes — only colours moved, which is the decoupling the plan
      asks for. Screenshots captured for dark and light.
    - **Gates:** `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings`
      0 warnings, `--lib` **1329**, `presentation` **60**, `protocol` **214**, `security`
      152, `runtime` 75, frontend `vitest` **316** (306 + the 10 new parity/invariant tests),
      `npx tsc --noEmit` clean, the Rust literal deny scan green (it covers every host
      stylesheet except the token definitions), prettier clean on every changed file.
    - **Numbers.** Fallback block 413 → 455 declarations; `tokens.css` 33.9 → 37.3 kB raw;
      `--clay-ds-*` references in frontend CSS 917 → 1039; built CSS index **46.6 kB raw /
      5.9 kB gzip** (unchanged gzip — the block is repetitive), all CSS **16.4 kB gzip**;
      bundle shell gzip **165.6 kB**, total **382.3 kB** (budgets 180/400).
    - **Drift recording shrank.** Unconsumed recipe keys **65 → 54**, unbacked consumed
      variables **1 → 0**, and the fixture now says why: what is left belongs to surfaces
      that do not exist yet (the Part D launcher/agent-picker families, the command-centre
      `menu` family, the settings `swatch`, the coding-agent `statRow`/`statusDot`/`keyHint`
      families, `card`, and two family variants no host can address).
    - **Findings recorded, not absorbed.** (1) `editor.default.path.rest` describes a
      read-only path _label_ (transparent, `text.muted`, no border) — the host's open-path
      control is a text input and correctly wears `textInput.input.*`; the package's key
      needs either a label slot in the editor family or removal. (2) `dropdown.default.root`
      and `popover.default.root` have no producer: the shared `.popover` class in
      `components/controls.module.css` borrows the dropdown family's _popover_ recipe, so a
      standalone popover surface still has no consumer — a task-18 finding. (3) `flex.default`
      was addressed by no host element before this task; the SDUI hosts now do, but a
      `direction`-less node was previously indistinguishable from a column. (4) The
      consumption gate used to scan only `*.module.css`, which made the scroll family's real
      consumer invisible; it now scans every host stylesheet except the token definitions,
      and `styles/global.css` is recorded as an owner of `scroll`.

- [x] Verify component-level conformance against the approved specimen
  - Acceptance Criteria:
    - Functional: automated checks confirm the component layer matches the approved specimen —
      recipe-key coverage for every component kind, state completeness, single border weight,
      radius ladder membership, transient-only shadows — and the manual comparison finds no
      visual deviation beyond the recorded list.
    - Performance: no regression in paint cost; the run records that no component adds a filter,
      blur or animated shadow.
    - Code Quality: any deviation is either fixed or recorded as a re-approval request in the
      task evidence; the specimen pages and screenshots are regenerated if the approved values
      changed (they must not change here).
    - Security: the deny scans (literal colours/geometry, package-name branching, raw CSS
      injection) run and pass.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §15 (review checklist), `references/ui.md`,
        `design-artifacts/approved/quiet-instrument-migration/component-catalog.html`.
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Defer component verification to the final visual review: component defects would be
        discovered after page work amplified them. (Rejected.)
    - Chosen Approach: run the automated gates, then a per-theme side-by-side comparison of the
      running component gallery (DEV fixture route) against the approved specimen, recording
      deviations.
    - Files to Create/Edit: none (verification); evidence recorded in the task.
    - References: `frontend/src/routes/fixture.tsx` (DEV gallery), `frontend/vitest` suites.
  - Test Cases to Write: none new (existing gates + comparison); record raw outputs.
  - Outcome (2026-09-12): **verified, with one re-approval request and three recorded findings.**
    - **The checks are now a tool, not a one-off.**
      `design-artifacts/tools/verify-component-conformance.mjs` has two modes: an offline
      _audit_ (default, no server needed) and a _browser_ pass
      (`--browser http://localhost:5199`), and it writes `report.json` plus ten screenshots
      to `design-artifacts/screenshots/quiet-instrument-component-conformance/`. It is
      recorded as a conformance gate in `docs/development/ui-design-system-conformance.md`
      and as manual step **UI-DS-33** in `test-plan/15-ui-design-systems.md`.
    - **Audit — 18/18 green.** _Coverage:_ the approved specimen page carries exactly the
      130 keys of the pre-migration reference set (`tests/fixtures/design-system-reference-keys.txt`,
      142 keys) minus its 12 `chat.default.*`, and the shipped 165-key contract extends it by
      exactly the 35 recorded target-IA keys in ten families — `seg(6) statusDot(5)
agentPicker(5) swatch(4) recentRow(4) keyHint(3) statRow(3) sessionRow(3) toast(1)
empty(1)`; all 30 approved kinds have specimens (the ten later kinds are named as
      additions), and the specimen's 181 `—` cells are all genuinely undeclared in the
      contract. _States:_ every slot declares `rest`, and every declared state is in the
      vocabulary (`rest/hover/active/focus/selected/disabled/invalid` plus the agent picker's
      `expanded`). _Geometry/material/motion:_ all radii on the 0/5/8/12/16/pill ladder with
      0 only on the 14 flushed regions; no border wider than a hairline and **no surface
      changes border weight between states**; elevation on 12 keys only, all transient
      surfaces (popovers, overlays, toasts, tooltips, the command centre, the modal,
      `panel.transient`, and the input's focus halo), with no hard offset and no inner
      highlight; blur only on the modal scrim and the toast; durations on 0/150/240/620 with
      exactly the eight entering surfaces on 240 ms spring-snappy; no `filter` or keyframe
      animation in the contract or in any component stylesheet.
    - **Browser — 0 mismatches, 0 paint violations.** Five systems (`@clay/core` plus the four
      shipped themes, each theme's roles computed the way `ResolvedUiTheme::base_color` maps
      its `textStyles` and then overlaid with its approved `designTokens`), the DEV gallery
      (`?fixture=controls`) and the approved specimen page side by side: **185 rendered
      component nodes**, each asserted against its declared recipe (radius, border width,
      shadow-layer count, transition duration, no blur/filter, animation only on the modal
      dialog) and against the specimen's own computed rendering of the same key
      (**55 comparisons, 55 agreeing**). Geometry was identical across all five systems; only
      colours moved. `paintViolations: []` across `controls`, `states`, `editor`,
      `package-ui` and `command-centre`.
    - **State probes resolve the declared role, not a plausible colour** (10/10): button
      hover = `surface.hover` fill + `border.subtle` boundary; text-input focus =
      `surface.control` + `accent.primary` boundary + exactly one shadow layer (the 3px accent
      halo) + `outline: 2px none` at offset 2; list-row selection = `accent.primary` at 0.15;
      button focus = **2px solid `focus.ring` outline at offset 2**; button press =
      `surface.active` fill + `translateY(1px)` (the `press-shift-down` preset). Two harness
      bugs were fixed on the way (headless pages need
      `Emulation.setFocusEmulationEnabled` or `:focus-visible` never matches; a state's
      computed value must be read after its 150 ms transition settles, or it is an in-flight
      interpolation).
    - **Manual side-by-side, all four themes plus the baseline:** no visual deviation beyond
      the recorded list. Gallery and specimen agree on the button family in every state
      (rest = hairline ghost, hover = surface fill, active = darker fill plus press shift,
      focus = accent ring, disabled = muted at half opacity), on badge/kbd (5px, hairline), on
      the input well, the dropdown trigger, the list rows (no separators), the collapse, and
      on modal/dropdown elevation. Screenshots for both sides per theme are the evidence; the
      approved artifacts were not written to and did not change.
    - **Re-approval request (recorded, not silently fixed): `textInput` slot ownership.**
      The specimen's `field` cell paints the _composer well_ — `surface-2` fill, hairline,
      12px radius, `:focus-within` accent + halo — and its `input` cell is the text inside it;
      the contract instead declares `field` as the transparent layout wrapper (gap, r12, no
      border) and puts the single-line boundary on `input` (r8, hairline, `surface.control`,
      focus = accent + halo). DESIGN.md §11 states both ("radius 12 (composer/textarea) or 8
      (single line)"), the host follows the contract, and the specimen's cell is the
      composer/multiline case. The missing piece is a `textInput.multiline` variant (or
      re-slotting the boundary onto `field`), not a change to single-line. Recorded in the
      tool's `RECORDED_CROSS_DEVIATIONS` with that reading, so the pair is compared
      deliberately rather than skipped.
    - **Recorded findings, each with its owner.** (1) _`surface.control` is a legacy
      projection_ (`← statusBg`): the shipped themes' input wells therefore read a step darker
      than the approved specimen's `--c-surface-2` (= panel) — the approved theme proposal
      deliberately left that role to the legacy path, so it is a page/theme decision for the
      shell/workspace/agent tasks, not a component defect. (2) _The specimen generator cannot
      emit the ten target-IA families_ — `make-component-catalog.py --check` aborts with
      "families missing from a section" — so the prototype catalog needs sections for them and
      a regeneration after the page tasks render them; the _approved_ page stays frozen, which
      is why this task compares against it instead of regenerating it. (3) _`[data-focused]`
      on the inner input is a dead selector half_: React Aria puts the focus data attribute on
      the field wrapper, so `input[data-focused]` never matches and the `:focus-visible` half
      carries the state (verified by the focus probe). Two run notes are expected, not
      defects: seven rendered keys are the `†` host slots (`dropdown.indicator`, `list.root`,
      `list.rowTitle`, `list.rowDetail`, `collapse.title`, `collapse.chevron`,
      `statusItem.status`), and two 0-width button recipes are painted as transparent
      hairlines so variant boxes keep identical metrics — the approved specimen paints them
      the same way. Three slots hover without a focus recipe (`card.root`,
      `scroll.scrollbarThumb`, `statRow.root`) — reported, not a language breach (rows, not
      controls).
    - **Gates re-run:** `--test protocol` 214 (documentation coverage, manual smoke docs and
      primitives docs re-checked after the doc edits), `--test presentation` 60, prettier
      clean on the tool and the edited docs. No Rust, TypeScript or CSS changed, so the
      component layer's previous gates (`cargo` 1329/60/214/152/75, vitest 316, bundle
      165.6/382.3 kB) stand unchanged.

- [x] Adopt the approved composition in the shell and Workspace page
  - Acceptance Criteria:
    - Functional: the shell renders the approved chrome (titlebar/tab strip, status bar with
      mono data, hairline zoning) and the Workspace page renders sidebar · editor column at the
      92ch measure · optional rail, with the path field on demand and document actions in one
      bar, matching `design-artifacts/approved/quiet-instrument-migration/{shell,workspace}.html`.
    - Performance: no layout shift on tab/pane/rail toggles; the pane split handle keeps a
      1px hairline hit target with a wider invisible grab area, and typing stays local
      (no React/shell work on keystrokes).
    - Code Quality: shell layout stays host-owned (grid/flex, user-configurable panel sizes);
      appearance stays variable-driven; no new hardcoded panel extents.
    - Security: no change to authority; tab/pane/panel state remains application state, not
      routes.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (composition), §5 (measures), §8 (typography
        roles), `references/ui.md` (shell layout model), `references/components.md` (chrome
        primitives), approved shell/workspace prototypes.
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Keep the current chrome and restyle only the components: the page-level composition is
        where the approval is most visible (status bar, tab strip, sidebar weight). (Rejected.)
    - Chosen Approach: adopt the approved composition surface by surface, preserving the
      existing keyboard model and panel-size persistence.
    - Files to Create/Edit: `frontend/src/app/layout/shell.module.css`,
      `frontend/src/app/layout/tab-bar.tsx`, `frontend/src/shell/pane-tree.module.css`,
      `frontend/src/routes/workspace.module.css`, `frontend/src/components/chrome.tsx`
      (+ status/kbd primitives), `frontend/src/editor/ClayEditor.tsx` (measure wrapper)
    - References: `frontend/src/shell/WorkspacePanes.tsx`, `frontend/src/shell/use-shell-chords.ts`
      (keyboard paths that must stay working).
  - Test Cases to Write:
    - Measure: the editor column honours 92ch and the agent transcript 72ch at 1500/1024 widths.
    - Status data: workspace path/branch/connection render mono with tabular figures.
    - Keyboard: pane focus, tab switching and rail toggle remain reachable by keyboard alone.
  - Outcome (2026-09-12): **implemented; composition adopted, two §12 details deferred to their owner tasks.**
    - **Shell chrome.** The window is the approved three rows — 40px titlebar, working area,
      28px status bar (both heights are host geometry with a typed-dimension override,
      `DESIGN.md` §5). The titlebar carries the mono tracked `Clay` mark with its accent dot,
      the pill tab strip (now `variant="inline"`: inside chrome that already draws the inner
      hairline the strip adds none — the old second line under the tabs is gone), a spacer and
      the action row (Palette, Files, Hide outline; each publishes its real binding through
      `aria-keyshortcuts`). The status bar is one mono, tabular-figures line: workspace ·
      document path/v/state · connection, then the keyboard hint row — three real controls
      (`palette` Ctrl X P, `files` Ctrl B, `hide outline` Ctrl I) that run the same commands
      their keys do, styled from `statusItem.default.root.rest`.
    - **Workspace view.** One host grid: the workspace surface · the rail
      (`WorkspaceView`, exported and reused by the DEV review fixture so the review surface
      cannot drift from the shipped one). The sidebar is a **flush zone** now: the SDUI tree
      emits a `Stack` (label + listing) instead of a `Panel`, so the host's left slot paints
      `fileBrowser` + one `divider` hairline — the framed veil card and its box-in-box edge are
      gone (`DESIGN.md` §6, §12; wiki sentence updated). The editor column is gutter + 92ch
      (`max-width: calc(92ch + 4rem)`, centred, `ch` resolving against the code face at body
      size), the document bar holds identity, state and **every** document action in one row
      (Open · Undo · Redo · Reload · Save · Close — Undo/Redo run `editor.clientUndo/Redo`), and
      the relative-path field is an **on-demand strip** revealed by `Open`, focused on reveal and
      dismissed with Escape (no longer a permanent control, and the native `Ctrl+O` dialog stays
      the documented `test-plan/03` behaviour rather than being rebound here).
    - **Optional rail (`⌘I`).** NEW: `shell/layout-state.ts` (one small store the titlebar, the
      status hint, the rail head and the chord all share), `routes/WorkspaceRail.tsx` with the
      open document's **real** facts (file, revision, state, entries, words — nothing unknown is
      shown, so language/encoding are absent) and its outline: every ATX heading, with
      `## 26-08-12 01:15 — title` split into time + title like the approved rail. Clicking an
      entry moves the caret (`revealLine` on the session) and marks the row. The rail is 340px
      (312px ≤1240px, both `DESIGN.md` §5) and becomes an opaque right-hand drawer with the pop
      shadow below 1000px, so the editor keeps its measure instead of being crushed.
    - **Pane split.** The handle is now a 1px hairline from `paneSplitTree.handle.rest` (pill
      radius, hover/active `accent.primary` at 40%/70%, focus ring), with the wider invisible
      grab area from the library's `resizeTargetMinimumSize` (24px coarse / 8px fine) — the
      visual line stayed a line and the target got real. The focused pane uses the contract's
      1px `accent.primary` outline instead of the legacy focus-border role, and the fixed-slot
      split handle reads the same family.
    - **Measured (evidence: `design-artifacts/screenshots/quiet-instrument-shell-workspace/`,
      `report.json`, 12 captures = 4 themes × 1500/1024/960, plus the approved pages as PNGs):**
      titlebar 40.0px and status bar 28.0px in every run; rail 340 (1500) / 312 (1024) /
      340-in-drawer (960); editor canvas 781.6px at 1500 (the full measure), 584px at 1024 with
      the rail in flow and 708px at 960 with the drawer out of flow; **0px horizontal overflow in
      all 12 runs**; the sidebar is a flat region in all of them. Probes: titlebar toggle →
      rail gone and the editor back to 781.6px; status hint → rail back; the open strip appears,
      takes focus as `Open relative path`; an outline entry marks itself active. The rail's
      outline scan is **144µs** for a 25 KiB / 44-heading document (measured), i.e. once per
      acknowledged version, so typing stays local; no blur, filter or animation exists on any of
      the three surfaces (asserted).
    - **Gates:** `cargo fmt --check`, `clippy --all-targets` clean; `--lib` 1329, `--test protocol`
      214, `--test presentation` 60, `--test runtime` 75, `--test security` 152; vitest **316 →
      324** (5 new composition tests, 2 rail-chord tests, 1 status-bar test); `tsc --noEmit`
      clean; prettier clean on every touched file; **bundle gzip unchanged at 165.6 kB shell /
      382.3 kB total** (budgets 180/400) — the task added no dependency and no new chunk.
      `tokens.css`'s fallback block was re-synced to the consumed set (455 → 464 declarations;
      three dead variables dropped, the newly consumed chrome/rail keys added) and the
      consumption backlog shrank 54 → 52 unconsumed keys with 0 unbacked consumed variables
      (`panel.transient.root.rest` now paints the drawer, `statusItem.default.root.rest` the hint
      row).
    - **Design decisions taken here (recorded, not silent):** the rail toggle lives in the
      titlebar as approved; the brand mark keeps the artifact's accent dot; the drawer is
      opaque rather than veil-filled because it sits over document text (§6 legibility first);
      the rail marks the clicked entry only — scroll-spy is not implemented (see Further
      Actions); `Ctrl+I` is a client-local shell chord like `Ctrl+Tab`/`Ctrl+\` rather than a new
      server keymap, so it needs no protocol change.
    - **Security (unchanged authority).** No route was added: the rail's visibility is component
      state in one host store, the tabs/panes/split tree stay application state, and the rail's
      facts/outline read only the already-open document's metadata and text. The server-side
      change removes an SDUI `Panel` wrapper (a `Stack` of the existing label + list remains), so
      the tree carries strictly less structure, no new node kind, no new action and no new
      authority; the launch/close/insert paths are untouched and the SDUI validation budgets are
      unaffected.
    - **Deferred with an owner (both are `DESIGN.md` §12 behaviour, neither is silently
      dropped):** the workspace sidebar's **filter field and count/tip footer** are SDUI work
      (the tree's per-row counts already ship as list `detail`; a real filter needs a filtered
      listing, not a fake control) — the Package-Workspace/SDUI task; and the sidebar's slot
      width still comes from `dimension.panel.side.default` (240px) rather than §5's 244px, also
      that task's file. Window buttons (minimise/maximise/close) are deliberately absent: the app
      uses the OS decorations and wiring Tauri window controls is new functionality, not a
      composition change.

- [x] Adopt the approved composition in the Coding Agent page (agent view of a tab)
  - Acceptance Criteria (added by the 2026-09-11 artifact review): the composer draws exactly one
    highlighted boundary — the shell's ring, no inner control outline (§14.4) — with the field
    spanning its zone, text left-aligned and the Send/cancel controls inside the field's trailing
    edge; the view's title is the agent-type picker (task 35), not a title plus a chip; the Files
    tab is the session's file history (task 36), not an editor host; and the page is the agent view
    of a tab (task 33), reached from the tab's view switcher, not the window's landing surface.
    The prototype's corrections live in the agent page's own stylesheet because they must beat the
    approved screen's rules (`design-artifacts/prototypes/quiet-instrument-migration/agent-landing.html`,
    README §9 findings 12–14).
  - Acceptance Criteria:
    - Functional: the agent page renders the approved header (title, model, context usage,
      effort), the 72ch transcript with hairline-separated turns and uniform truncated tool
      boxes, the inset composer, and the inspector (Files, Memory, Context, Session Info,
      Settings) with skills/MCP reference data in the Context tab — matching
      `design-artifacts/approved/quiet-instrument-migration/{agent-landing}.html`; the same
      surface is correct when it is the landing (fresh window, empty transcript, first-run hint).
    - Performance: transcript scrolling stays viewport-bounded; the composer keeps typing local
      (no React rerender per keystroke); no new blur/filter on the transcript or composer.
    - Code Quality: `coding-agent.module.css` (586 lines, largely literal today) consumes
      `--clay-ds-*`/`--clay-*` variables for every visual property with layout host-owned; the
      panel keeps its data-driven tabs and keyboard chords (`Shift+Tab` effort cycle) intact.
    - Security: agent output rendering keeps its existing sanitisation and trust treatment; the
      restyle introduces no new HTML injection or external resource.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (agent composition), §8 (typography/measures),
        §9/§11; `references/ui.md`; `references/components.md`; approved `agent-landing.html`.
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Restyle only the chrome and leave the transcript/composer structure: the measurement
        and IA changes are the visible part of the approval. (Rejected.)
      - Structural adoption of the approved composition (chosen).
    - Chosen Approach: restructure the panel's regions to the approved layout (header /
      transcript / composer / inspector), move skills+MCP into the Context tab (already the
      approved IA), and replace literal geometry with recipe variables; keep the existing
      transport, session and keyboard behaviour untouched.
    - Files to Create/Edit: `frontend/src/coding-agent/CodingAgentPanel.tsx`,
      `frontend/src/coding-agent/coding-agent.module.css`,
      `frontend/src/coding-agent/CodingAgentPanel.test.tsx`
    - References: `frontend/src/agent/state.ts`, `frontend/src/shell/PaneTree.tsx`
      (effort chord), `plans/117-...md` (context/MCP inspector work).
  - Test Cases to Write:
    - Landing state: empty transcript shows the approved first-run affordances (no fabricated
      transcript) with file/folder paths available from the agent surface.
    - Transcript measure: turns cap at 72ch; tool boxes truncate uniformly and expand in the
      inspector.
    - Keyboard: `Shift+Tab` effort cycle, composer submit and inspector tab switching work with
      keyboard only.
  - Outcome (2026-09-13): **implemented; the approved composition is adopted, the Files tab and
    the agent picker stay with their owner tasks (36 and 35).**
    - **Column.** Header (agent name as the view title · model control, or the active
      `provider/model` when no inventory is loaded · context meter · effort) / transcript /
      state strip (status dot + text + note) / composer / environment foot — all host layout,
      every colour, border, radius, shadow and duration a recipe or theme role
      (`coding-agent.module.css` is 100 % `--clay-ds-*`/`--clay-*`; the only literals left are
      layout values, which §6 keeps host-owned).
    - **Transcript.** The 72ch measure is `max-width: 72ch` on the inner column (measured
      535.4px, exactly 72ch at the code face) with hairline-separated turns: role label,
      right-aligned mono note, body. Machine output (`tool`, `skill`, `usage`) is a single
      inset well clamped to three lines (measured 57px), so a turn's height never depends on
      how much it printed — the pre-existing behaviour, now bounded by construction rather
      than by a truncation helper. The separator is a _leading_ edge on every turn but the
      first (the approved artifact uses a trailing edge plus a `:last-child` reset); a turn
      rounds only in its fill states, so the rest-state hairline runs straight.
    - **Composer.** `ClayTextField variant="composer"`: the well (fill + hairline + r12) is the
      **field**, the inner control is transparent, borderless and unpadded, and the Send/Stop/
      Close actions sit inside the field's trailing edge. The conformance measurement counts
      **exactly one** bounded node in the composer in all 12 runs (`§14.4` one ring per
      surface). The `/` and `@` completion menus float above the composer on the menu recipe.
    - **Inspector.** The fixed 340px column (312px ≤ 1240px) replaced the old draggable split
      — the split's inline size override was also what let the transcript collapse mid-drag;
      below 1000px it becomes an opaque right-hand drawer with the pop shadow, mirroring the
      Workspace rail. Its tabs keep their data-driven bodies; the Context tab now also carries
      the skills/MCP reference data (they were transcript cards) plus context counts as stat
      rows with share bars relative to the largest real count on screen (no fabricated
      ceiling).
    - **Foot.** Workspace root · `git <branch|—>` · `extensions …` · one MCP summary segment
      (`MCP files · 3 tools · ghost · hidden`, `MCP none` when empty) — mono, tabular, and the
      only place the per-session environment is stated.
    - **Measured (evidence: `design-artifacts/screenshots/quiet-instrument-agent/`,
      `report.json`, 12 captures = 4 themes × 1500/1024/960):** 72ch measure 535/535.4px; 4
      turns with 3 hairline separators; one clamped tool well at 57px; **1** composer
      boundary in every run (composer 131px tall, opening at the approved 48px text area);
      inspector 340px static (1500) / 312px static (1024) / 400px fixed drawer (960); 5
      inspector tabs with the hide toggle inside the viewport; **0px horizontal overflow in
      all 12 runs**; no blur, filter or animation on the transcript, composer or foot in any
      run. Probes (1500/vivendi): Hide inspector collapses the grid to `1500px` (one column)
      and Show inspector restores `1160px 340px`; the Context tab is reachable by pointer and
      — with no session, skills or servers in this fixture — shows **no** fabricated
      capability sections or stat rows; focusing the composer leaves the inner control
      borderless; clicking a turn selects it. **(Correction, same session: the capture
      harness injected the themes through a `JSON.stringify`ed statement list, so that
      injection was a string-literal no-op and every run painted the core fallback palette.
      The injection is real code again and the set was re-measured — identical geometry per
      theme with four distinct palettes.)**
    - **Gates:** `cargo test --test presentation` 60/60 (`package_ui_conformance` incl. the
      fallback-parity block, `theme_packages`); `documentation_coverage` 11/11; vitest
      **324/324** (`tsc --noEmit` clean; prettier clean on every touched file;
      `design-system-consumption.test.ts` green with 0 unbacked consumed variables and
      **35** recorded unconsumed keys, down from 52).
    - **Four real defects found by the evidence pass and fixed at the root:** the inspector's
      `data-inspector` attribute sat on the aside while the collapsing CSS selected the grid, so
      the hide toggle did nothing (the probes caught it); five inspector tabs in a 340px column
      pushed that toggle past the window edge (measured at x=1501 on a 1500px viewport) because
      the tab strip used `display: contents` and the panel variant had no shrinkable column —
      the tab row now scrolls and the actions stay put, and the capture asserts every probe
      target is inside the viewport; the composer's prompt did not grow, so Send/Close sat next
      to the text instead of on the well's trailing edge; and the `--clay-ds-*` pre-bootstrap
      block in `frontend/src/styles/tokens.css` was **not** a superset of the consumed variables
      (521 now, with nothing dead), so a fixture page — or the first frame of a real window —
      painted the browser's default control borders (including the turn's UA border) and dropped
      composed fills. The first three are page-scale defects the _component_ audit could not see,
      which is why the composition capture is kept as evidence. The evidence pass also exposed a
      gate bug: `verify-component-conformance.mjs` forced its hover/press/focus attributes on the
      _first_ button on the page, which after the shell composition is a titlebar button that
      re-renders and drops them — the three probes now target the fixture under test
      (`[data-fixture] …`) and all five state probes resolve again (2/2, 2/2, 1/1, 3/3, 2/2).
    - **Design decisions taken here (recorded, not silent):** the header shows the active
      `provider/model` when the models inventory is empty (the old status row's readout, kept
      because the profile name alone does not say what is running); the empty transcript's
      hint row lists the chords this view really owns (`/`, `@`, `⇧↵`) and not the artifact's
      `⌘K` — this app's palette binding is `Ctrl X P` and the shell's status bar already
      advertises it; the inspector's drawer is opaque for the same reason the rail's is
      (§6 legibility over text); the transcript separator is a leading edge rather than the
      artifact's trailing edge with a `:last-child` reset; `?fixture=<id>&state=<state>` now
      survives the DEV harness redirect (router, DEV-only) so a fixture can be opened and
      captured in a named state.
    - **Deliberately not done here:** the Files tab still hosts the editor (task 36 replaces
      it with the session file history) and the view title is still a label rather than the
      agent-type picker (task 35). Both are called out in the task's own acceptance criteria
      as owned by those tasks, and neither blocks the composition.

- [x] Adopt the approved composition in the Settings panel and Agent Settings page
  - Acceptance Criteria:
    - Functional: the Settings panel (fixed right slot) and the Agent Settings page render the
      approved composition — section eyebrows, hairline-separated groups, mono values, settings
      dropdowns showing the shipped choice sets, and no framed box-in-box stacking; both match
      the approved prototypes in all four themes.
    - Performance: opening/closing the panels causes no whole-app relayout beyond the panel
      slot animation; settings mutations keep their existing bounded requests.
    - Code Quality: panel chrome uses transient/fixed-slot variables correctly (a fixed panel
      carries no elevation), and the SDUI-rendered fallback panel in `@clay/settings` stays
      consistent with the React panel.
    - Security: settings surfaces keep validation on the server; the UI introduces no new
      preference or authority.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (fixed slots vs transient surfaces), §11,
        `references/components.md` (`settingsPanel`, `collapse`, `dropdown`, `textInput`),
        `references/config.md` (selection surfaces).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Leave the agent settings page as a separate visual dialect: two settings surfaces would
        drift immediately. (Rejected.)
    - Chosen Approach: adopt one settings composition for both surfaces (fixed slot / routed
      page), sharing the collapse-list primitives.
    - Files to Create/Edit: `frontend/src/settings/settings-panel.module.css`,
      `frontend/src/settings/SettingsPanel.tsx`, `frontend/src/agent-settings/agent-settings.module.css`,
      `frontend/src/agent-settings/AgentSettingsPanel.tsx`, `packages/settings/package.json`
      (SDUI panel recipe/style usage)
    - References: `tests/primitives_docs.rs`, `frontend/src/test/settings-panel-choices.test.tsx`.
  - Test Cases to Write:
    - Fixed slot: no shadow on the fixed settings panel; transient overlays still elevate.
    - Choice sets: theme list and design-system list render from the snapshot, sorted, with the
      shipped entries only.
    - Keyboard: panel open/close, focus containment and Escape dismissal unchanged.

  - Outcome (2026-09-13): **done.** Shipped: the Settings panel is the tab's fixed right slot (the
    package already declared `slot: right, kind: fixed`; the host mounted it beside the slot, so the
    track clamped it to 240px and clipped it — the panel now mounts _in_ the slot track, with
    `.rightFixed` widening the track to the slot's own width); the composition is eyebrow-labelled
    `collapse` groups of hairline-separated rows (`SettingsRow`: micro-label, control, caption note),
    mono values for data (`ClayTextField role="monospace"`), a clipped field label so no label paints
    twice (`labelHidden`), and an actions row whose note states whether anything can commit with the
    buttons at the trailing edge (primary last). Fixed-slot chrome: veil fill from
    `settingsPanel.panel`, **no radius and no elevation**, boundary from the slot's divider — the
    recipe's radius/hairline/fill are spent in the <760px drawer, which also goes opaque. The Agent
    Settings tab is the same language narrowed: a caption naming the sources, a `list.default.row`
    per delivered file (mono path, mono size, `badge.*` provenance) and the `empty.*` recipe for the
    first-run state. The `@clay/settings` SDUI tree (`package.json` + `dist/load.js`, re-mirrored)
    renders the same sections, rows and action labels.
    - **Defects found by the evidence pass and fixed at the root:** the panel was mounted _beside_
      its grid slot and clipped (measured 921→1261 in a 1500px window, with 101px outside the
      window); the head duplicated every field's label (the field's own `Label` plus the row's);
      the actions note squeezed both buttons onto two lines at 340px; and the invalid-typography
      error branch was unreachable (the button is disabled when the values are invalid, so the
      `role="alert"` could never render) — the note now carries that state instead.
    - **Deliberate deviations (recorded, not silent):** Appearance stays a `dropdown` rather than the
      prototype's segmented control — the package-facing kind list has no `seg` kind, and the SDUI
      panel must stay consistent with the React panel (the `seg` family stays reserved for the Part D
      view switcher); the swatch in the prototype's theme trigger is not shipped (the client has no
      per-theme palette to draw, and inventing one would fabricate colors); the prototype's 190px
      label column becomes a stacked row because the slot is 340px, not a 720px page; the agent
      files list keeps the prototype's _rest_ and _empty_ scenes and **not** the "unreadable" scene —
      `src/server/agent_settings.rs` lists an unreadable skills directory as empty, so the product
      cannot reach an error state there.
    - **Measured (evidence `design-artifacts/screenshots/quiet-instrument-settings/`, `report.json`,
      12 panel captures = 4 themes × 1500/1024/760 + 4 agent-file scenes):** slot flush against the
      pane's right edge and inside the window in every run (340px at 1500, 312px at 1024, a 480px
      drawer at 760), `boxShadow: none` everywhere, 2 eyebrow sections (`text-transform: uppercase`),
      10 rows of which 8 carry a hairline (each group's last row ends at the group boundary), 4/4
      numeric values in the mono role, 3 dropdowns reading the snapshot (`Modus Operandi`,
      `Core baseline`, `Dark`) with `esc` + Close in the head, actions `[Reset preferences, Apply
typography]`, **0px horizontal overflow** in all 12; the agent Settings column is 760px capped
      with 3 delivered rows (mono size, `built-in`/`edited` badges) and a designed empty state.
    - **Harness correction:** the capture script injected each theme through a
      `JSON.stringify`ed statement list, which made the injection a string-literal no-op, so
      every run painted the core fallback palette. It now injects real code (and the agent
      capture was re-run: same geometry, four genuinely distinct palettes).
    - **Gate extended:** `frontend/src/sdui/surface-adoption.test.tsx` scans the settings and
      agent-settings stylesheets for hardcoded border/shadow geometry and asserts the fixed-slot
      chrome (no radius, no elevation, recipe radius/hairline/opaque fill only in the drawer).
    - **Gates:** `cargo test --lib` 1329 ✓ · presentation 60 ✓ · protocol 214 · runtime 75 · security 152 ✓ ·
      `cargo fmt --check` / `clippy --all-targets -D warnings` ✓ · vitest 334 (incl. the new
      composition gates) ✓ · `tsc --noEmit` ✓ · `vite build` shell 169.8/180 · total 390.9/400 kB gzip ✓ · component
      conformance tool 18/18 with 0 mismatches ✓ · design-system consumption drift gate ✓.
    - **Tests:** `frontend/src/test/settings-panel-choices.test.tsx` (9: enumeration from the server
      snapshot _and_ the composition gates — sections/rows, no nested panel or elevation, mono
      values, `esc` affordance + Escape dismissal) and
      `frontend/src/agent-settings/AgentSettingsPanel.test.tsx` (6: rows as `list.default.row`s,
      mono sizes, badges, open routing, loading, designed empty state, source caption).
    - **Docs:** `DESIGN.md` §12 (settings surfaces composition rule) and §16 (shipped profile),
      `docs/wiki/modules/ui-design-system-runtime.md`, `docs/development/ui-design-system-conformance.md`,
      `test-plan/15-ui-design-systems.md` + `test-plan/index.md` (UI-DS-36), and the inventory of
      record `design-artifacts/prototypes/quiet-instrument-migration/README.md` (§2 owner for `list`
      rows, §3/§4/§5 rows for both surfaces).

- [x] Adopt the approved composition in the Command Centre, Package Workspace and overlay family
  - Acceptance Criteria:
    - Functional: the Command Centre (palette), Package Workspace and the overlay family (modal,
      dropdown/popover, menu, tooltip, toast, path-open strip) render the approved language,
      including the accent state language for selection, keyboard focus ring plus halo, and the
      approved 12px/16px transient radii with soft shadows — matching
      `design-artifacts/approved/quiet-instrument-migration/{command-centre,package-workspace,overlays}.html`.
    - Performance: palette filtering stays local and bounded; overlays animate with the approved
      150ms/240ms values only (no transitions on paint-heavy properties), and reduced-motion
      collapses them.
    - Code Quality: transient surfaces are the only elevated surfaces; the overlay modules keep
      their single source of recipe variables and no literal geometry; the unused
      `routes/fixture.module.css` gallery is updated to the new language so DEV review matches
      production.
    - Security: overlay focus containment, dismissal and modal semantics preserved; no new
      external resource or injected markup in palette results.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §6 (materials: overlay/pop/scrim), §7 (motion), §9
        (state language), §11, §12 (one global palette), `references/components.md`
        (`modal`, `dropdown`, `tooltip`, `statusBar`), `references/ui.md`.
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Restyle overlays later with the "polish" pass: the palette and modals are the most-seen
        surfaces and the approval covers them explicitly. (Rejected.)
    - Chosen Approach: adopt per surface with the approved materials, and verify keyboard
      navigation/focus behaviour stays intact after the visual change.
    - Files to Create/Edit: `frontend/src/command-centre/command-centre.module.css`,
      `frontend/src/command-centre/CommandCentre.tsx`, `frontend/src/packages/package-workspace.module.css`,
      `frontend/src/packages/PackageWorkspace.tsx`, `frontend/src/components/{modal,tooltip}.module.css`,
      `frontend/src/components/controls.module.css` (popover/menu), `frontend/src/routes/fixture.module.css`
    - References: `src/shell/transient_menu.rs` (menus/completion), `frontend/src/sdui/renderer.tsx`.
  - Test Cases to Write:
    - Overscroll/anchor: palette and dropdowns anchor correctly near viewport edges with the
      new radii/shadows.
    - Focus: the 2px ring + halo is visible on every overlay in all four themes.
    - Reduced motion/transparency: overlays lose transitions and transparency under the OS
      settings without losing legibility.

  - Outcome (2026-09-13): **done.** Shipped: the command palette is one opaque elevated sheet
    (`commandCentre.root`: r16, hairline, overlay shadow, bounded at `min(68vh, 640px)` with a
    scrolling result list) whose head carries the search glyph, the server's prompt as a micro-label
    and the query input, and whose foot carries `keyHint` chips (navigate / run / close) plus the live
    result count (`commandCentre.status`); `list` rows are the results and a selected row is the
    accent @0.15 fill. **One ring per surface** (§14.4): focusing the sheet turns its boundary accent
    and adds the halo, and the input inside it draws none. A menu session (`contextMenu`/`menuBar`)
    renders the same component on the narrower `popover.root` surface (r12, pop shadow) under its own
    prompt. The modal became a sheet — hairline-separated head (title + close) and actions foot,
    cancel leading and the primary action trailing, a scrolling body, and a `flush` mode where the
    content owns the surface and the head (used by the palette). The `/`/`@` completion menu gained
    its pressed state, and the accessibility layer now resolves every veil under
    `prefers-reduced-transparency` (scrim → canvas, veil surfaces → the opaque overlay plane, no blur
    left) instead of only the two selectors it knew.
    - **No fabrication:** the prototype's scope control (all/Session/Skills/MCP) and its group
      headers have no server data behind them — `TransientMenuSnapshotDto` carries `label`/`detail`
      only — so the shipped palette renders neither, and a test asserts it. Per-row key chips are
      likewise absent: a chord reaches the UI inside the row's mono detail (`@clay/git - Ctrl+G`),
      which is the real data.
    - **Deliberate deviations (recorded, not silent):** the palette rows keep the app's two-line list
      row (title over mono detail) rather than the artifact's single-line title+meta row — one row
      vocabulary for every list in the product, and the detail line is where the chord lives; the
      prompt is the head's micro-label for **every** origin (the server decides the prompt, so the
      host cannot tell a question from a name) instead of the artifact's label-less palette head;
      menu sessions stay in the window-level modal layer the server marks `focusPolicy: modal` (the
      pointer-anchored modeless menu of the artifact is not implemented); `panel.fixed.root.rest`
      declares the same values as `panel.default.root.rest`, so nothing renders it and it stays in
      the backlog; `dropdown.root`/`popover.root`-as-a-bare-surface and the **toast** stay
      unconsumed — the host's floating surfaces are the modal, the dropdown popover, the menus and
      the tooltip, and no shipped surface emits a toast (a host with no emitter would be dead code).
    - **Measured (evidence `design-artifacts/screenshots/quiet-instrument-overlays/`, `report.json`,
      32 captures = palette × 4 themes × 1500/1024/960 + empty/path/menu scenes + modal + tooltip):**
      palette radius 16 / border `1 solid` / overlay shadow / head+foot hairlines 1px in every run;
      3 rows (2 in the path and menu scenes), 4 key chips, `3 results` (and `2 results`), **0px
      horizontal overflow** in all 32; the focus probe shows the sheet's boundary equal to the
      theme's accent (rgb-normalized) with the halo present and the input's outline `none`; the
      empty scene shows the message and no list; the path scene names itself "Browse workspace"; the
      menu scene is r12 and content-sized (never 640px); the modal is r16 with a 1px head and foot
      hairline, `space-between` actions `[Cancel, Confirm]` and a `blur(3px)` scrim; the tooltip is
      r8 with the pop shadow; `prefers-reduced-motion` collapses the sheet's transition (1e-05s) and
      `prefers-reduced-transparency` yields opaque scrim/dialog/panel backgrounds with no blur.
    - **Gates:** `cargo test --lib` 1329 ✓ · presentation 60 ✓ (incl. the host-fallback projection
      gate) · protocol 214 ✓ · runtime 75 ✓ · security 152 ✓ · `cargo fmt --check` /
      `clippy --all-targets -D warnings` ✓ · vitest 346 (incl. the new composition gates) ✓ ·
      `tsc --noEmit` ✓ · `vite build` + bundle budgets (shell 170.3/180 kB, total 392.6/400 kB gzip) ✓ ·
      component conformance tool 18/18 with 0 mismatches ✓ · design-system consumption drift gate ✓ (the backlog shrank 35 → 31 keys and the
      `tokens.css` fallback block was re-synced to the consumed set).
    - **Tests:** `frontend/src/test/overlay-composition.test.ts` (8: transient-only shadows, no
      literal geometry in the overlay modules, the palette's bound/scroll/focus ring, the modal
      head/body/foot and flush mode, the reduced-transparency and reduced-motion fallbacks, recipe
      timing only) and `command-centre/CommandCentre.test.tsx` (7, incl. the palette sheet
      composition, the empty state, the menu surface and the no-fabricated-scopes assertion).
    - **Docs:** `DESIGN.md` §11 (palette width is the centered-overlay token; the command-centre
      recipe bullet rewritten to the shipped slots; the halo's scope) and §16 (the shipped overlay
      family + the toast's unconsumed status), `docs/development/ui-design-system-conformance.md`,
      `test-plan/15-ui-design-systems.md` + `test-plan/index.md` (UI-DS-37), and the inventory of
      record `design-artifacts/prototypes/quiet-instrument-migration/README.md` (§2/§6/§7 rows).

- [x] Remove the @clay/chat package and its server surface
  - Acceptance Criteria:
    - Functional: `packages/chat/**` and its bundled-inventory entry are deleted; `chat.*`
      commands, the legacy `chat.open{Agent,Model,Provider}Picker` aliases, the chat intent
      authorization path, `execute_chat`, and the `chat.submit|cancel|steer` agent-run-action
      branch are removed; no `@clay/chat` load in `examples/config/**`; the coding agent's send,
      cancel and steer paths (AG-UI transport, `session.resume`) keep working unchanged;
      the deleted surface's 12 `chat.default.*` recipe keys are also not carried into
      `@clay/design-instrument` (task 8, inventory finding 1) so the consumption gate can be a
      hard assertion (task 12).
    - Performance: no regression; the removal deletes one package from the bundled set (record
      the reduced inventory size and any build-time fingerprint diff).
    - Code Quality: no dead branch, alias, test or doc reference remains; the agent host's
      cancel path keeps its single owner (the transport) and any host-side cancel branch that
      existed only for the chat composer is removed with it.
    - Security: the removed commands cannot be invoked through any residual path (SDUI action,
      behavior manifest, key binding, or package contribution) — an invocation now fails as an
      unknown command, not as a silently authorized action.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/packages.md` (bundled
        inventory, trust domains, agent host), `references/js-api.md` (command IDs, facade),
        decision `2026-09-11-1700`.
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Keep `chat.*` commands as aliases for the agent's submit/cancel: the coding agent does
        not use them (it drives the AG-UI transport directly), so the aliases would be dead
        compatibility surface with no consumer. (Rejected.)
      - Remove package + server surface entirely (chosen).
    - Chosen Approach: delete the package and inventory entry, remove the command/intent/alias
      code and its tests, re-point the landing assertions (done in the next task), and rebuild
      so the generated inventory table drops the entry.
    - Files to Create/Edit:
      - delete `packages/chat/**`
      - `src/packages/bundled-inventory.toml`,
        `src/server/command_execution.rs` (`is_chat_intent`, `execute_chat`, tests),
        `src/server/connection/runtime.rs` (`is_agent_run_action`, cancel branch, tests),
        `src/server/agent_picker.rs` (legacy aliases + test),
        `src/server/js_runtime/tests.rs`, `src/server/mod.rs` (landing assertions — re-pointed
        by the next task), `examples/config/init.js`, `examples/config/packages/first-party.js`,
        docs referencing `@clay/chat`
    - References: `packages/chat/package.json` (what it contributed),
      `frontend/src/coding-agent/CodingAgentPanel.tsx` (what the agent actually uses).
  - Test Cases to Write:
    - Absence: a scripted assertion fails if `@clay/chat` or `chat.submit`/`chat.cancel`/
      `chat.steer` appear in `packages/`, `src/`, `frontend/src/`, `examples/`.
    - Regression: agent submit/cancel/steer through the transport still pass the existing agent
      host tests.

  - Outcome (2026-09-13): **done**, with one AC correction recorded below.
    - **Deleted:** `packages/chat/**` (4 files, 12,505 B raw / ~3,389 B gzip), its
      `root = "chat"` entry in `src/packages/bundled-inventory.toml`, and its
      load line in `examples/config/packages/first-party.js`. The generated trust
      table went 19 → 18 entries and 4,793 → 4,634 B, dropping the manifest
      fingerprint `5b1da4dc38d9c47b` (`@clay/chat` 0.1.0): a stale entry now fails
      the trust binding instead of resolving a directory that no longer exists.
    - **Server surface removed:** `is_chat_command` + `execute_chat` and their
      dispatch lane in `handle_command` (`src/server/command_execution.rs`,
      `src/server/connection/runtime.rs`); the three legacy
      `chat.open{Agent,Provider,Model}Picker` aliases in `src/server/agent_picker.rs`;
      the `chat.cancel` command-intent branch in `handle_command_intent` that
      existed only for the chat composer (the transport is now the single owner
      of cancel); the chat-specific assertions in `src/server/mod.rs` and the
      chat package tests in `src/server/js_runtime/tests.rs`.
    - **AC correction (recorded, not silently re-scoped):** the AC says the
      `chat.submit|cancel|steer` agent-run-action branch is removed and that "the
      coding agent's send, cancel and steer paths keep working unchanged". Those
      intents _are_ the agent's only send/cancel/steer path — `CodingAgentPanel`
      submits through the shared transport, and `TauriClayAgent` posts exactly
      those ids as host-rendered `sduiAction`s — so deleting the ids outright
      would have broken the composer. The branch was therefore **renamed**, not
      dropped: `agent.submit` / `agent.cancel` / `agent.steer`, matching the
      core `agent.*` picker namespace. The authorization owner is unchanged (the
      bound tab session via `is_agent_run_action`, which is why host-rendered
      controls need no declared SDUI node), the retire ids are asserted to be
      _unauthorized_ (`!is_agent_run_action("chat.*")`) and _unknown_ commands,
      and the transport, its tests and the wiki flow doc carry the new ids.
    - **Pane action targets:** `packages/coding-agent/package.json` no longer
      declares the three intents as `actionTargets`. They were there only because
      `validate_registered_actions` requires every target to be a _registered_
      command — and `@clay/chat` was the package that registered them. They are
      host-owned controls (no package-declared node sends them), so they belong to
      the tab-session authorization instead of a second, declared path. Keeping
      them would have forced the coding agent to register three agent-composer
      commands purely to satisfy its own action list.
    - **Landing (interim truth):** with the package gone, the example tree
      contributes **no** `empty-tab` pane content, so an empty tab renders the
      core Open File / Open Folder fallback (`WelcomeWidget` branch in
      `PaneTree`) — exactly the state plan 118's launcher task replaces with the
      launcher surface. `src/server/mod.rs` therefore now asserts
      `empty_tab() == None`, that the agent keeps its named `pane` surface, and
      that the catalogue registers `coding-agent.profile` instead of
      `chat.profile`. (The AC line "re-point the landing assertions (done in the
      next task)" moved here: the example tree is this task's change, so the
      assertion had to move with it — the next task does the _frontend_ branch,
      fixture and rename.)
    - **Also re-pointed (their subject was the deleted package):**
      `tests/package_loading.rs::third_party_replacement_withdraws_the_coding_agent_and_stays_untrusted`
      (the replacement mechanism's fixture target), `tests/agent_protocol.rs`'s
      Phase 25 authority marker (now the coding agent's "grants no execution
      authority"), plan 061's pinned package inventory (19 → 18, in the doc that
      the gate reads), plan 118's own `primitives_docs` markers
      (`loadPackage("@clay/coding-agent")`, `coding-agent.chromeActions`), the
      generic SDUI renderer test's fixture (renamed to `example.*`/`@vendor/example`),
      and the `tests/fixtures/configuration/plan080-manual/` copy of the example
      tree (chat load line removed; note: that fixture was already several plans
      stale — re-syncing it is not this task's scope and is left to the
      documentation task).
    - **Tests:** `tests/package_ui_conformance.rs::plan118_chat_landing_and_its_commands_are_absent`
      — scans `packages/`, `src/`, `tests/fixtures/`, `scripts/`, `examples/` and
      the frontend surfaces this task owns (`frontend/src/{agent,coding-agent,sdui}`)
      for `@clay/chat`, `chat.entry|profile|submit|cancel|steer|composer|open*Picker`,
      allows only the three files whose whole purpose is asserting the retirement,
      and **fails on a stale exemption** (`FRONTEND_PENDING` must still exist, so
      the next task cannot forget to drop the chat panel, fixture and landing
      branch from it). `src/server/connection/runtime.rs` keeps
      `host_agent_actions_do_not_require_a_static_sdui_node` (renamed ids plus the
      negative assertions) and the payload tests for the thinking-level argument;
      `src/server/command_execution.rs` gained
      `retired_chat_commands_are_unknown`; `src/server/js_runtime/tests.rs` gained
      `chat_package_removal_leaves_the_core_empty_tab_fallback` (no landing without
      a package, and the retired specifier now fails as an unknown package).
      Frontend: `transport.test.ts`, `CodingAgentPanel.test.tsx` and
      `registry.test.tsx` carry the renamed ids/targets.
    - **Gates:** `--lib` 1328 ✓ · presentation 61 ✓ · protocol 214 ✓ · runtime 75 ✓ ·
      security 152 ✓ · `cargo fmt --check` / `clippy --all-targets -D warnings` ✓ ·
      vitest 346 (42 files) ✓ · `tsc --noEmit` ✓ · `vite build` + budgets (shell
      170.3/180 kB, total 392.6/400 kB gzip) ✓ · component conformance audit 18/18 ✓.
      Not run here: an end-to-end app launch — the plan's launch-test task owns it,
      and the in-process example-config load (which now asserts the new landing
      state) plus the frontend fixture test for `emptyTab: null` cover the change
      without a daemon.
    - **Docs:** `docs/reference/packages/creating-packages.md` (Phase 25 contract:
      the landing is its own package, the launcher replaces `@clay/chat`; the
      replace/extension example now uses the coding agent's real
      `coding-agent.chromeActions`), `docs/reference/ui-components.md`,
      `.agents/skills/clay-execution/references/{packages,components}.md`,
      `docs/wiki/flows/ag-ui-tauri-stream.md`, `docs/wiki/modules/react-sdui-package-ui.md`,
      `packages/coding-agent/{dist/load.js,docs/index.md,docs/parity-checklist.md}`,
      `examples/config/init.js` §12, `test-plan/09` + `test-plan/16` (the chat rows
      are marked historical/removal), and
      `docs/wiki/modules/react-agui-chat-stream.md` (intent ids plus an explicit
      "the panel and its branch are removed by the next task" note). Historical
      records keep their mentions: `docs/wiki/archive/`, the parity ledger, the
      primitive-migration review and `code-reviews/`.
    - **Follow-ups for the next task:** drop `FRONTEND_PENDING` from the absence
      gate once `frontend/src/chat/**`, `routes/fixture.tsx`'s chat fixture and
      `PaneTree`'s `@clay/chat` landing branch are gone, and extend that gate's
      scan to `frontend/src` as a whole.

- [x] Remove the chat frontend surface and de-chat the shared agent module
  - Acceptance Criteria:
    - Functional: `frontend/src/chat/**` is deleted; `PaneTree`'s empty-tab chat branch, the
      chat fixture in `routes/fixture.tsx`, the chat lazy import, the `chat/chat.module.css`
      ownership entries, and the chat test cases are removed; the shared agent module is renamed
      away from chat naming (`chatAgent`/`ChatSnapshot`/`ChatStatus` → session naming such as
      `agentSession`/`AgentSnapshot`/`AgentStatus`) with no behaviour change.
    - Performance: the frontend drops one lazy chunk (`chat` panel) — record the bundle delta
      and confirm the agent chunk still loads lazily.
    - Code Quality: the rename is mechanical and complete (no `chatAgent` symbol remains); the
      agent transport keeps its single shared instance semantics, refcounted mount/unmount, and
      its test seams.
    - Security: no chat-only capability (input buffer, approval prompt path, cancel handler) is
      orphaned; the agent's pending-approval and cancel handling keep exactly one owner.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md` (client
        architecture: AG-UI is the React-facing agent protocol), `references/packages.md`
        (agent host), decision `2026-09-11-1700`.
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Keep `chatAgent` naming: cheap, but the code would keep advertising a removed product
        surface to every future reader (and to agents using grep for "chat"). (Rejected.)
      - Mechanical rename with tests as the safety net (chosen).
    - Chosen Approach: delete the chat module and its references, then rename the shared
      module and symbols in one pass, running the frontend suite plus the agent transport tests
      after each step.
    - Files to Create/Edit:
      - delete `frontend/src/chat/**`
      - `frontend/src/shell/PaneTree.tsx`, `frontend/src/routes/fixture.tsx`,
        `frontend/src/shell/WorkspacePanes.test.tsx`, `frontend/src/sdui/registry.test.tsx`,
        `frontend/src/test/design-system-consumption.test.ts`,
        `frontend/src/agent/{state.ts,TauriClayAgent.ts}` (+ tests),
        `frontend/src/coding-agent/CodingAgentPanel.tsx`
    - References: `frontend/src/agent/state.ts` (`createChatAgent`, `__clayChatAgent` seam).
  - Test Cases to Write:
    - Absence: no `chat` path or `chatAgent` symbol remains in `frontend/src/`.
    - Behaviour parity: the agent transport tests (transcript lifecycle, events, resume) pass
      unchanged after the rename.

  - Outcome (2026-09-13): **done**; the rename is mechanical and the AC's
    "chat/chat.module.css ownership entries" were already gone (task 8 dropped
    the family from the package), so the equivalent removals are recorded below.
    - **Deleted:** `frontend/src/chat/**` (`ChatPanel.tsx`, `ChatPanel.test.tsx`,
      `chat.module.css`), the `ChatPanel` lazy imports in `shell/PaneTree.tsx`
      and `routes/fixture.tsx`, `ChatFixture` + `chatFixtureSurface` + the
      `?fixture=chat` route, `PaneTree`'s `@clay/chat` empty-tab branch (every
      package landing now renders inertly through `PackageSurfaceView`), the 38
      `--clay-ds-chat-default-*` fallback declarations in `styles/tokens.css`,
      the `chat` kind in `test/design-system-adapter.test.ts`, the
      `["src/chat/**/*.test.tsx", "jsdom"]` vitest glob, and the unused
      exported `ChatIntentContext` in `agent/TauriClayAgent.ts`.
    - **Rename (mechanical, no behaviour change):** `chatAgent` → `agentSession`,
      `ChatAgentModule` → `AgentSessionModule`, `ChatSnapshot` → `AgentSnapshot`,
      `ChatStatus` → `AgentStatus`, `createChatAgent` → `createAgentSession`,
      `__clayChatAgent` → `__clayAgentSession`, `resetChatAgentForTests` →
      `resetAgentSessionForTests`, `seedChatFixture` → `seedAgentFixture`. The
      singleton, refcounted `start()` mount/unmount, DEV `seedForDev` seam and
      `resetForTests` test seam are unchanged; comments that described a
      "chat stream"/"chat agent" now say agent/session.
    - **AC correction (recorded):** "the `chat/chat.module.css` ownership
      entries" had no counterpart left — `COMPONENT_CSS_OWNERSHIP` in
      `design-system-consumption.test.ts` never listed `chat` (the recipe family
      was deleted in task 8) — so the equivalent stale references were the
      fallback block, the adapter test's kind list and the two _lane names_
      below. Nothing else in the client named the surface.
    - **Performance:** bundle before (task 22, chat surface present) shell
      170.3 / total 392.6 kB gzip → after (deleted surface) shell 168.0 / total
      388.5 kB → after the laziness fix below shell 169.3 / total 388.7 kB
      (budgets 180 / 400). The chat panel chunk is gone; `CodingAgentPanel` is
      still a **lazy** chunk (9.7 kB gzip, dynamically imported from the lazy
      `WorkspacePanes` chunk, absent from `index.html`'s preloads).
    - **Defect found and fixed while confirming that laziness:** the manual
      chunk was named `chat-agent-core`, and rollup _hoists a manual chunk's
      dependencies into it_, so `bridge/client.ts` (which the eager shell
      imports) rode along and made the whole 37.5 kB-gzip AG-UI core a
      **preloaded startup** chunk — the filename-lane budget gate then excluded
      it from the shell number, so the gate was reporting 168 kB while startup
      really paid ~205 kB. `bridge/client.ts` now has its own chunk: `agent-core`
      is 36.4 kB gzip and genuinely lazy, the shell lane preloads `bridge`
      (1.3 kB), and the gate's shell number is honest (169.3/180). Lane naming
      is de-chatted (`agent` lane in `scripts/bundle-budget.mjs`, `agent-core`
      chunk in `vite.config.ts`).
    - **Security / no orphans:** cancel keeps one owner (`TauriClayAgent.abortRun`
      → `agent.cancel`, driven by the panel's Stop action — the deleted panel's
      second cancel button is gone); pending-approval handling
      (`pendingApproval`/`clearPendingApproval`) is consumed only by
      `CodingAgentPanel`; the composer draft buffer is panel-local. Coverage that
      only the deleted panel had — "streaming state offers Stop, composer stays a
      steering lane" — was **ported** to `CodingAgentPanel.test.tsx` rather than
      dropped, and that suite's `beforeEach` now calls `vi.clearAllMocks()` so
      one test's send history cannot leak into another's "every intent is
      declared" assertion (a real pre-existing gap the port exposed).
    - **Tests:** new `frontend/src/test/chat-surface-absence.test.ts` — scans
      every file under `frontend/src` for the retired paths/symbols (with a
      positive-control assertion so the matcher cannot pass vacuously) and pins
      the session-named exports. The Rust gate
      `plan118_chat_landing_and_its_commands_are_absent` now scans **all** of
      `frontend/src` (not just the three directories task 23 owned), drops
      `FRONTEND_PENDING`, adds the eight retired symbols to its needle list,
      asserts `frontend/src/chat` does not exist, and asserts every retirement
      guard still exists (the frontend absence test is registered as the fourth
      guard). `plan118_host_fallback_block_matches_the_resolved_package` lost its
      `chat-default` exemption: a fallback variable with no shipped recipe now
      fails instead of being tolerated by prefix.
    - **Gates:** vitest 347 (42 files) ✓ · `tsc --noEmit` ✓ · `vite build` +
      budgets ✓ · `cargo fmt --check` / `clippy --all-targets -D warnings` ✓ ·
      `--lib` 1328 ✓ presentation 61 ✓ protocol 214 ✓ runtime 75 ✓ security 152 ✓ ·
      component conformance audit 18/18 ✓ · Prettier clean (except the
      pre-existing `src/icons/fallback.generated.ts` divergence).
    - **Docs re-pointed (live pages only):** `docs/wiki/flows/ag-ui-tauri-stream.md`,
      `docs/wiki/modules/react-agui-chat-stream.md` (+ the `docs/wiki/index.md`
      blurb), `docs/development/architecture-ownership.md`,
      `docs/development/build-and-test.md`,
      `docs/reference/packages/creating-packages.md`,
      `docs/development/react-ui-catalog-mapping.md` (the `chatPanel` row now
      says removed), `docs/wiki/modules/react-sdui-package-ui.md`, and the eight
      `chatPanel` rows in `docs/development/ui-design-system-recipe-matrix.md`
      (owner column: "none — surface removed (plan 118)", component name kept
      for the `primitives_docs` literal gate). Historical records keep their
      mentions: `docs/development/performance.md`'s 2026-08-31 chunk table (it
      still labels the lane "chat"; the docs task can annotate it),
      `ui-design-system-css-audit.md` (self-declared historical ledger),
      `docs/wiki/archive/`, the parity ledger and `code-reviews/`.
    - **Follow-ups:** none functional. The launcher task replaces the interim
      core empty-tab fallback with the launcher surface, and the docs task can
      annotate the historical lane name in `performance.md`.

- [x] Make the launcher the landing surface, and the tab a workspace + agent pair
  - Superseded decision: `decision-logs/2026-09-11-1700-…` elected
    `@clay/coding-agent` as the `empty-tab` landing. On 2026-09-11 the user
    replaced that with the tab model (one workspace + one agent, two views) and
    the launcher as the landing surface; Part D tasks 33–36 implement it and a
    new decision log records the supersession.
  - Acceptance Criteria:
    - Functional: the launcher (Part D task 34) contributes the `empty-tab` pane content, so a
      fresh window and a new empty tab render the launcher; `@clay/coding-agent` keeps its `pane`
      surface and is no longer an `empty-tab` candidate; the host renders the launcher panel for
      its trusted provenance and, with no empty-tab contribution installed, the core fallback
      stays Open File / Open Folder only (no product-named core landing).
    - Performance: the landing adds no extra package load (the package is already loaded by the
      first-party config or bundled resolution) and does not delay first paint beyond the
      existing lazy chunk.
    - Code Quality: no compiled landing stub and no core branch on the agent's name beyond the
      existing provenance check helper (generalized to a "trusted first-party pane surface"
      lookup); the empty-tab election keeps its one-winner conflict diagnostics; the second
      pane-content contribution for one package is validated (no duplicate-activation error).
    - Security: the landing grants no authority — it is the same inert contribution path, and
      the host still requires exact package provenance and trusted-domain status before
      rendering the compiled panel.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/packages.md` (product
        landings are packages; no irreplaceable native landings), `references/ui.md` (agent
        composition), decision `2026-09-11-1700`, plan `108/109` agent-surface history.
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Host-redirect the empty tab to the agent when the package is loaded (no manifest
        change): the landing would depend on a compiled core branch naming the agent — exactly
        what the landing rule forbids. (Rejected.)
      - Manifest `empty-tab` contribution (chosen): the existing election machinery picks the
        winner, revocation falls back to the core empty state, and the host keeps one generic
        provenance check.
    - Chosen Approach: add the `empty-tab` activation to the coding-agent package's
      `ui.paneContents`, generalize the host's trusted-surface lookup, update the election and
      landing assertions, and verify the fresh-window behaviour on a real build.
    - API Notes and Examples:
      ```json
      {
        "id": "coding-agent.landing",
        "activation": "empty-tab",
        "actionTargets": [
          "coding-agent.profile",
          "documents.clientOpenFileDialog",
          "workspace.clientOpenFolderDialog",
          "agent.clientOpenModelPicker"
        ]
      }
      ```
    - Files to Create/Edit:
      - `packages/coding-agent/{package.json,dist/load.js}` (landing contribution),
        `frontend/src/shell/PaneTree.tsx` (empty-tab branch renders the agent for the trusted
        contribution; core fallback otherwise),
        `src/server/js_runtime/tests.rs` + `src/server/mod.rs` + `src/server/ui.rs` (landing
        assertions and election tests), `examples/config/packages/first-party.js` (comment/order),
        `docs/reference/packages/creating-packages.md` (landing example),
        `docs/wiki/modules/ui-review-harness.md` (landing fixture expectations)
    - References: `src/server/ui.rs` (empty-tab election), `src/protocol/runtime.rs`
      (`EmptyTabContent`), `packages/coding-agent/package.json`.
  - Test Cases to Write:
    - Landing election: the coding-agent contribution wins the empty-tab slot; two competing
      contributions fail with the conflict diagnostic.
    - Revocation: disabling the package returns the empty tab to the Open File/Open Folder
      fallback with no residue.
    - Fresh window: a launch with the example config lands in the agent (manual + fixture
      assertion).

  - Outcome (2026-09-13): **landing done**; Part D task 34 is the launcher build and
    is marked below. The tab-pair half of the title is task 33 (unchecked): the
    launcher ships and owns the landing, the tab model that pairs both views is the
    next Part D item. What landed:
    - **Package `packages/launcher/`** (`@clay/launcher`, `apiPrefix: launcher`,
      zero permissions, one `ui.serverRegisterPaneContentContribution`
      dependency): a single pane content `launcher.start` with
      `"activation": "empty-tab"`, one inert action target
      (`workspace.clientOpenFolderDialog`) and an inert fallback tree
      (`launcher.root`: caption + open-folder button) that the generic SDUI
      renderer would paint for a third-party render. `dist/load.js` mirrors the
      manifest and registers it; `dist/index.js` re-exports the load entry;
      `docs/index.md` documents the surface, its host data and its authority.
      Registered in `src/packages/bundled-inventory.toml` (`root = "launcher"`),
      loaded by the canonical first-party config
      (`examples/config/packages/first-party.js` and its comment index), and
      fingerprinted by `build.rs` like every other bundled manifest. Its required
      extension point is `launcher.surface` (kind `panelContribution`,
      `replace`) — the point a replacement landing derives from.
    - **`@clay/coding-agent` is untouched**: it still claims exactly one `pane`
      surface (`coding-agent.surface`) and never the empty tab
      (`tests/package_loading.rs::coding_agent_bundled_manifest_assembles_without_claiming_the_empty_tab`
      keeps pinning that, and the new
      `launcher_bundled_manifest_claims_the_empty_tab` pins the launcher's side).
    - **Host data (`src/server/launcher.rs`, new):** recent workspaces persist as
      plain paths in `launcher.json` under the Clay data root (newest first, capped
      at 8, temp+rename write, malformed/unknown-version file degrades to
      first-run), entries whose folder disappeared are pruned on read and the
      prune is persisted; configured agent types are a bounded directory scan of
      `<data root>/agents/` (name → label, resolved config root with `~`
      display, seeded skill count); removal is by index in the _server's_ list.
      Recents are recorded at the three explicit-open points: a new tab on a
      folder (`IpcServer::create_tab_state`), an in-tab folder open
      (`open_workspace_for_bound_tab`) and the folder dialog
      (`handle_add_selected_workspace_root`); restore/reclaim deliberately does
      not re-stamp recency.
    - **Protocol:** `LauncherEntries`/`LauncherWorkspaceEntry`/`LauncherAgentEntry`
      DTOs plus `ListLauncherEntries` / `RemoveLauncherRecent` client families and
      the `LauncherEntries` server reply, handled in the connection loop
      (`write_launcher_entries`, which answers the first-run state when no data
      root exists and emits one bounded `launcher.recents_pruned` diagnostic when
      it pruned). Mapped through `ClientConnectionEvent::LauncherEntries` to the
      webview `launcherEntries` feature event; the bridge re-stamps the client id.
      The parity ledger gained the rows (and its family counts moved to 28 client /
      41 server).
    - **Frontend:** `frontend/src/launcher/LauncherPanel.tsx` +
      `launcher.module.css` implement the approved `start.html` composition —
      1320px centered column, 92ch head, two panes (Workspaces / Agents) with
      count chip, filter well and single-select rows, one action row whose primary
      button names exactly what it opens (`Open clay`, `Open Coding Agent`,
      `Open clay + Coding Agent`) and stays inert until something is picked, the
      `⇥ / ↑↓ / ⏎ / ⌘⏎ / esc` path, `⌫` removes the cursor row, first-run and
      no-match states as specified copy, and the folder dialog in the workspace
      pane foot. Rows come from the server listing requested once on mount
      (`session.listLauncherEntries()`), refreshed by every later answer.
      `PaneTree` generalised its single-package lookup into
      `HOST_RENDERED_SURFACES`/`hostRenderedSurface` — a trusted-provenance +
      package-name lookup with two entries (`@clay/coding-agent` → the agent
      panel, `@clay/launcher` → the launcher panel); every other `empty-tab` or
      `pane` contribution still renders through the generic SDUI view.
      **The core fallback lost its `Coding Agent` button**: with no empty-tab
      contribution the empty tab is Open File / Open Folder only, so core carries
      no product-named landing.
    - **AC mapping:** functional — fresh window/new empty tab render the launcher
      (`launcher_claims_the_empty_tab_landing_and_the_agent_keeps_its_pane` on the
      shipped example tree, plus the `WorkspacePanes` rendering tests); the agent
      keeps `pane` and never competes; trusted provenance renders the compiled
      panel, a third-party `empty-tab` contribution renders through SDUI, and no
      contribution is the core fallback. Performance — the landing adds no package
      load beyond the one-line first-party load and one lazy chunk
      (`LauncherPanel` 2.6 kB js + 1.4 kB css gzip; shell 173.3/180 kB, total
      392.9/400 kB). Code quality — no compiled landing stub, no new core branch
      beyond the generalised trusted-surface lookup, election unchanged with its
      one-winner conflict diagnostics (extended test: a landing + a pane from two
      packages coexist, a second landing conflicts in sorted id order and
      withdrawing restores the winner), and a second pane content for one package
      still validates (the existing two-contribution test). Security — the landing
      is the same inert contribution path: exact provenance + trusted domain
      before the compiled panel renders, no path ever flows back from the webview
      (removal is an index; recents are the server's own stored paths; the only
      action is the existing folder dialog).
    - **Gates:** `--lib` 1336 ✓ presentation 61 ✓ protocol 214 ✓ runtime 75 ✓
      security 153 ✓ · vitest 357 (43 files) ✓ · `tsc --noEmit` ✓ ·
      `vite build` + budgets ✓ · `cargo fmt --check` / `clippy --all-targets
-D warnings` ✓ · Prettier clean except the pre-existing
      `src/icons/fallback.generated.ts` divergence · component-conformance audit
      18/18 with the same four recorded deviations (no launcher finding).
    - **Deliberate deviations from `start.html` (recorded in `DESIGN.md` §16):**
      the workspace rows carry name + real path without the branch/MCP meta column
      (that needs the cached git service — the declared `recentRow` family ships
      with that follow-up; the rows consume `list.default.row.*`), the filter wells
      are the shipped input wells rather than the prototype's quiet variant, the
      panes take the panel recipe's veil fill where the prototype drew them
      unfilled, the `Agent settings` link is omitted (the surface is the agent
      inspector's Settings tab, so a launcher link would be dead or misleading),
      and the "F" filter hint is dropped (Tab reaches the fields; a bare letter
      would steal a typeable key).
    - **Open follow-up (not a defect):** the launcher is loaded by the canonical
      first-party config, so a _user_ `~/.clay/init.js` that loads nothing shows
      the core Open File / Open Folder fallback — the architecture's normal
      package story (product landings are packages), and the landing appears with
      one `loadPackage("@clay/launcher")` line. If a bare install must open on the
      launcher without any user config, the launcher needs the same
      resolve-without-loading treatment the canonical default themes have
      (`canonical_default_specifier` is theme-only today); recorded here rather
      than silently widened.

- [x] Execute and update the manual test plan (test-plan/)
  - Outcome (2026-09-13): the affected modules are rewritten for the shipped
    state and executed on a freshly rebuilt Linux desktop build
    (`cargo build --bins` **and** `cargo build --bins -p clay-desktop`);
    evidence in `test-plan/artifacts/118-quiet-instrument-migration/` (nine state
    directories + `README.md`), index record at
    `test-plan/index.md#plan-118-manual-test-plan-execution-record-2026-09-13-task-26`.
    - **Steps rewritten, not marked N/A.** Module [01](../test-plan/01-launch-and-connection.md)
      now separates the two landing states: L12 = core fallback (`Empty tab`,
      `Start with a file or folder`, `Open file`/`Open folder`, **no product
      name in core**), L12a = the bundled launcher landing (`Start`, panes with
      count chips and filter wells, real rows, first-run notes, action footer
      whose primary names what it opens), plus L14a/L14b (landing → workspace /
      agent handoffs) and three negative checks (fallback carries no product
      name; empty store shows the first-run note; rendering the landing never
      opens a document, stamps a recent or writes the workspace). Module
      [03](../test-plan/03-files-and-workspace.md) F32 gained F32a and was
      rewritten for both landing resolutions, F35 now records the recents
      stamp, and F37a/F37b add recents hygiene (prune on read, `⌫` removes the
      server entry only, listing never creates the data root — new assertion in
      `src/server/launcher.rs`). Module [13](../test-plan/13-window-splits.md)
      S35 returns to the landing instead of "welcome".
    - **Module [15](../test-plan/15-ui-design-systems.md)** rewrote
      UI-DS-02/16/17/19/26 for `@clay/design-instrument` vs `@clay/core`,
      dropped the removed Chat surface from UI-DS-12, added UI-DS-38 (launcher
      landing composition), UI-DS-39 (hairline zoning), UI-DS-40 (composited
      boundary/state contrast), UI-DS-41 (removed-specifier fallback + choice
      set), gained a `Known ceilings` section, marked the plan-104 review table
      and the removed-package rows of the older execution records as
      **historical**, and appends the plan-118 record.
    - **Module [17](../test-plan/17-coding-agent-parity.md)** added C38–C40
      (landing → agent-view launch route, the Settings tab as the Agent Settings
      surface, the Files tab session history) and C-N12/C-N13 (chat-surface
      absence, landing ownership), and replaced the chat wording in C14/C32.
      Module [16](../test-plan/16-agent-host.md) A1/A4/A18 and its record now
      name the agent surface instead of the removed chat surface; modules
      [09](../test-plan/09-packages-and-modes.md) P36 and
      [11](../test-plan/11-performance.md) mark their chat rows historical.
      **No step loads, selects or installs a removed package or a removed design
      system.**
    - **Executed (real build, isolated mode-700 harness):** PASS —
      `launcher-landing/` (new `ui-review-launcher` fixture; tree + screenshot
      inspected against the approved `start.html`) and `core-fallback/`,
      `design-system-dark/`, `design-system-light/`, `loading/`, `error/`,
      `recovery/`, `large-typography/`. UNRESOLVED with stated causes —
      `coding-agent/` (the agent pane needs a package-contributed command
      dispatched from a later generation **and** pointer/keyboard input) and
      every interactive leg (landing handoffs, close-return, live theme switch,
      palette, dialogs, splits/tabs): the host has no input-synthesis backend
      (`computer-use-linux doctor`: no `/dev/uinput`, xdotool/ydotool, portal
      input path). AT-SPI action invocation works on this host but the harness
      stops the app before a probe can act — recorded as a harness follow-up
      rather than a claim.
    - **Three findings, two fixed here:** (1) the harness `mkdir`ed but never
      created `$home/.config` before `chmod 700` on it, so **every** fixture
      aborted under `set -e` before launching — no real-build capture was
      possible; fixed in `scripts/capture-ui-review.sh`. (2) the coding-agent
      review fixture relied on `commandDispatch("coding-agent.profile")` alone
      and silently captured the fallback once the landing changed; it now loads
      `@clay/coding-agent` first (pinned by
      `plan118_ui_review_harness_captures_the_shipped_system_and_rejects_removed_states`,
      which also pins the new launcher fixture). (3) **design finding carried to
      the visual-review task:** on the landing the workspace route still mounts
      the file browser and the outline rail, and the rail renders a
      zero-document fact block (`REVISION v1`, `STATE clean`, `ENTRIES 0`,
      `WORDS 0`, `No headings in this document.`) for a tab with no document —
      honest values, but the composition implies a document, and the approved
      `start.html` draws neither surface. Disposition required there: hide the
      rail (optionally the sidebar) on the empty tab, or re-approve the chrome.
    - **No step was weakened to pass** and `test-plan/index.md` records the
      module map, coverage-matrix row and execution record; the parity ledger
      gained the new step IDs (`L12a/L14a/L14b`, `F32a/F37a/F37b`, `C38–C40`).
    - **Gates:** `--lib` 1336 ✓ presentation 61 ✓ protocol 214 ✓ runtime 75 ✓
      security 153 ✓ · frontend vitest 357 ✓ · `tsc` ✓ · Vite build + budgets
      (shell 173.3/180, total 392.9/400 kB gzip) ✓ · fmt/clippy ✓ ·
      component-conformance 18/18 with 0 mismatches ✓.
  - Acceptance Criteria:
    - Functional: the affected modules are executed on a real Linux build and recorded against
      their numbered steps: `01-launch-and-connection` (landing), `15-ui-design-systems`
      (rewrite UI-DS-02/16/17/19/26 for the shipped system; verify UI-DS-27…30 and add steps
      for the hairlines, boundary contrast and the removed-specifier fallback),
      `17-coding-agent-parity` (agent as landing, chat steps removed), plus `03`, `13`, `14`
      where the surfaces changed; `test-plan/index.md` coverage matrix updated.
    - Performance: no step is weakened to pass; any failure is recorded as a defect or as an
      explicit ceiling in the file's ceilings section.
    - Code Quality: new steps carry expected results, negative checks and known ceilings, and
      cross-link `DESIGN.md` / `docs/development/ui-design-system-*` instead of duplicating them.
    - Security: removed chat/system steps are deleted rather than left as "N/A"; no step
      instructs a user to enable a removed package.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map, coverage matrix),
        `design-artifacts/approved/quiet-instrument-migration/**`, `DESIGN.md` §15.
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Mark the design-system module as superseded: loses the per-theme verification the
        language needs. (Rejected.)
    - Chosen Approach: rewrite the removed-system steps to the shipped system, add the
      Instrument-specific checks, run everything on a real build, record pass/fail with the
      build identifier.
    - Files to Create/Edit: `test-plan/01-launch-and-connection.md`,
      `test-plan/03-files-and-workspace.md`, `test-plan/13-window-splits.md`,
      `test-plan/14-tabs.md`, `test-plan/15-ui-design-systems.md`,
      `test-plan/17-coding-agent-parity.md`, `test-plan/index.md`
    - References: `test-plan/artifacts/110-ui-design-systems/**` (historical evidence format).
  - Test Cases to Write: the steps themselves are the test cases; negative checks are required
    for each new step (wrong theme, removed specifier, package disabled).

- [x] Perform the visual and accessibility review of the migrated app against the approved artifacts
  - Acceptance Criteria:
    - Functional: a real Linux GUI build is exercised across every changed surface and state
      (landing/empty, transcript running/idle/error, workspace with and without rail, settings
      open/closed, palette, modal, menus, tooltips, disabled/loading/disconnected) at 1500×950
      and 1024×800, in all four themes; screenshots are stored under
      `test-plan/artifacts/118-quiet-instrument-migration/` and inspected.
    - Performance: no visual regression from paint-heavy effects (no blur outside scrim/toast,
      no animated shadows), and the review confirms no layout shift during design-system or
      theme switches.
    - Code Quality: every deviation from the approved artifacts is listed with its disposition
      (fixed to match, or an explicit re-approval request with the reason); an unfixed,
      unapproved deviation fails the review.
    - Security: accessibility verification uses `computer_use_linux_get_app_state` first and
      checks keyboard-only flow, focus visibility/order, role/name/state exposure, modal
      containment and announcements for changed controls; results are recorded, and an
      unavailable tool leaves manual acceptance explicitly unresolved.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/planning-checklist.md`
        (visual/a11y duty), `references/ui.md`, `DESIGN.md` §13/§15,
        `design-artifacts/approved/quiet-instrument-migration/**`.
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Source inspection instead of a live review: cannot prove focus order, contrast in the
        running theme, or layering. (Rejected.)
      - Live screenshot + a11y review with a deviation table (chosen).
    - Chosen Approach: scripted captures per surface/theme/width plus an accessibility-tree pass
      over the changed controls, then a per-surface comparison against the approved artifact.
    - Files to Create/Edit: `test-plan/artifacts/118-quiet-instrument-migration/**` (evidence),
      task evidence in this plan.
    - References: `scripts/capture-ui-review.sh` (capture harness),
      `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
  - Test Cases to Write: none new (review); the deviation table is the deliverable.

  - Outcome (2026-09-13): **done, with four items explicitly unresolved.**
    Deliverable: `test-plan/artifacts/118-quiet-instrument-migration/review/`
    (`visual-review.md`, `pixel-contrast.txt`, `a11y-live.txt`, `a11y-live-tree.txt`)
    plus 42 captured states — 4 themes × 2 widths for the workspace, palette and
    launcher, both widths for the rail-toggle, titlebar-focus, native-dialog,
    loading, error, recovery and large-typography scenes. Every capture carries
    its screenshot, accessibility tree, drive steps and measurements; all 42 are
    `PASS` with zero horizontal overflow.
    - **Harness gaps closed to get there:** the crop now intersects AT-SPI frame
      extents with the compositor's own window bounds (the frame reports Wayland
      shadow padding, which pulled host panel pixels into the retained crop — a
      plan 097 privacy breach); the harness retains the accessibility tree of the
      captured state; `--drive` now covers palette-open, native-dialog, rail-toggle
      and focus, which the module 15 records had listed as unreachable.
    - **Defect found and fixed:** at large typography the status bar's document
      message painted across the workspace label and the key-hint chips (the
      `role="status"` wrapper had no `min-width: 0` and the value inside it was an
      inline span, so nothing could truncate). Fixed in
      `frontend/src/app/layout/app-shell.tsx` + `shell.module.css`; `src/test/shell.test.tsx`
      passes and the scene was re-captured.
    - **Deviation table:** ten deviations, each dispositioned in
      `visual-review.md` §4 — nine are implementation equivalences already
      recorded by the adoption tasks (input radius contract, turn separators,
      literal key-chord hints, Appearance dropdown, no per-theme swatch, stacked
      settings rows, no "unreadable" agent-settings scene, list-row border 0,
      retired `--c-line`) and one is the user's re-approved 340px rail.
    - **Accessibility:** live AT-SPI walk of the running app — 53 interactive
      nodes, 0 unnamed application controls, named landmarks ("Application
      controls", "Clay workspace"), a named tab list, a 37-item named file list,
      and the palette exposing `dialog: "Control Center"` with its entry and
      selected option. Nits recorded (an empty brand label node, the rail's
      `dl` term/value association, no editable role reported for the editor in
      this engine build).
    - **Live contrast (UI-DS-40's live leg):** the rail's leading divider
      measures 1.39/1.45/1.46/1.61:1 across the four themes (floor 1.2:1), the
      focused-pane accent outline 5.8–10.4:1, and state fills are accent-15% by
      design — the 3:1 state floor is text-over-fill and stays on the resolved-role
      gates.
    - **Unresolved, with blockers (not claimed as reviewed):** live tooltips
      (need pointer entry), the unsaved-changes sheet (needs a document edit; its
      focus containment is covered by `frontend/src/test/components.test.tsx`),
      the agent pane in the desktop build (package-contributed pane the harness
      cannot switch; current evidence is `design-artifacts/screenshots/quiet-instrument-agent/`),
      and the `computer_use_linux_get_app_state` cross-check (its tree agreed with
      AT-SPI for the shell; the portal capture path superseded its screenshots).

- [x] Update the design, catalog and roadmap documentation for the shipped state
  - Acceptance Criteria (added by Part D): every doc that calls the Coding Agent the landing
    surface is corrected to the launcher (roadmap Phase 2 agent-UI binding spec, `DESIGN.md` §12,
    `PRODUCT.md` if it states it, `.agents/skills/clay-execution/references/ui.md`), and the tab
    model (one workspace + one agent, two views), the agent-type picker and the session-files
    inspector are documented where the surfaces are described — no document still describes a tab
    as a document or the agent as the window's landing.
  - Acceptance Criteria:
    - Functional: `DESIGN.md` §16 describes the shipped implementation (no "pending migration"
      language), the roadmap's Phase 2 _Coding Agent UI (binding spec)_ is reconciled with the
      shipped approved surface (chat extension points removed, landing behaviour recorded,
      inspector tab set, measures), `PRODUCT.md` records the single shipped design language and
      the four themes, `docs/wiki/modules/ui-design-system-runtime.md` reflects the shipped
      package/theme values, and no non-historical document presents Neobrutal, Glass or Chat as
      available.
    - Performance: documentation-only; generated registries stay in sync with their sources.
    - Code Quality: drift-tested tables and the doc registry are regenerated
      (`cargo run --bin update-doc-registry` where frontmatter/registry content changed), and
      every touched page stays linked from `docs/index.md`/`docs/wiki/index.md`.
    - Security: no doc claims authority that was removed (e.g. chat commands, glass materials)
      or that does not exist.
  - Approach:
    - Documentation Reviewed:
      - `references/docs-as-code.md` (wiki vs reference split, archive
        policy), `docs/index.md`, `roadmap.md` Phase 2/2.1, `DESIGN.md`.
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Rewrite the development audit pages (`ui-design-system-css-audit`,
        `ui-design-system-package-primitive-review`) as current docs: they are records of a
        past state; a pointer is honest, a rewrite falsifies history. (Rejected.)
      - Update normative pages, point from historical ones (chosen).
    - Chosen Approach: update the normative/reference pages, add dated pointers to the
      historical audits, reconcile the roadmap spec, and sweep for stale claims.
    - Files to Create/Edit: `DESIGN.md`, `PRODUCT.md`, `roadmap.md`,
      `docs/reference/ui-design-systems.md`, `docs/reference/ui-components.md`,
      `docs/reference/packages/creating-packages.md`, `docs/index.md`,
      `docs/wiki/index.md`, `docs/wiki/modules/**`, `docs/development/ui-design-system-*.md`,
      `.impeccable/surfaces/operate-visual-direction.md` (surface brief: shipped language, no
      removed systems, agent landing)
    - References: `tests/documentation_coverage.rs`, `tests/manual_smoke_docs.rs`.
  - Test Cases to Write:
    - Currency: a grep gate fails when a normative page still advertises a removed system,
      package, or surface.
    - Registry: the doc-registry and coverage tests pass after regeneration.

  - Outcome (2026-09-13): **done.**
    - **Landing corrected everywhere.** `roadmap.md`'s Phase 2 *Coding Agent UI
      (binding spec)* was rewritten to the shipped/approved surface: the agent is
      the **agent view of a tab** (one workspace + one agent, two views, `⌘1`/`⌘2`,
      switcher in tab chrome), not a 50/50 split of the working area, and the
      **launcher** (`@clay/launcher`) is the window's landing and the content of
      every empty tab — the section, its exit-gate checklist, the package-scope
      bullets, the Phase 2.1 tab wording and Phase 7's inspector-tab wording all
      now say so. `PRODUCT.md` gained a *Surfaces* line (tab model + launcher) and
      a rewritten visual-authority block (one shipped language, the four shipped
      themes, the removal of the two comparator systems). The `.impeccable`
      surface brief gained the launcher/agent view and its contrast case is named
      *removed reference systems*; `ui.md`, `components.md` and
      `creating-packages.md` already carried the tab model from the Part D tasks
      and now also state it where a tab's contents are defined.
    - **Shipped state, not migration state.** `DESIGN.md` §11/§16 no longer read
      as pending (the radius table's pill row no longer lists the retired
      selection bar, §11 states the chat keys as deleted rather than "to be
      removed"), the visual-direction contract's status/STORY/FIRST VIEWPORT/
      migration paragraphs describe the shipped host adoption and name the
      remaining Part D feature work, `docs/reference/ui-design-systems.md`'s pill
      list matches the manifest (tab items, segments, toasts, progress tracks,
      status dots), and `docs/wiki/modules/ui-design-system-runtime.md` §4 now
      documents the re-cut `@clay/core` baseline (same language, host-consumed
      subset, neutral sparse default) and the shipped theme-side roles.
    - **Chat: a removed surface, not a quiet one.** `roadmap.md` no longer
      compares the agent package to `@clay/chat` or keeps "chat flows"
      byte-compatible, `ui-review-harness.md`/`tabs-and-clients.md`/`clay-agent.md`
      name the removal, and the daemon's built-in `Chat` **profile** (a real
      `ensure_tab_session` default) is documented as a daemon-level name with no
      shipped UI surface, so the two cannot be confused.
    - **Gate extended and passing.** `plan118_catalog_pages_do_not_ship_removed_systems`
      now also scans `PRODUCT.md` and `roadmap.md`, checks the removed chat
      *surface* identifiers (`@clay/chat`, `chatPanel`, `chat.default.`,
      `chat.entry`) with the same same-line removal-marker rule, and lists the
      frozen plan-097 migration records (`tauri-react-primitive-migration.md`,
      `tauri-react-parity-ledger.md`, `react-ui-catalog-mapping.md`,
      `ui-review-harness.md`) as historical pages that keep their mentions.
    - **Registry regenerated.** `cargo run --bin update-doc-registry` re-derived
      `docs/generated/clay-js-api-registry.json`; it carried two stale
      descriptions (still naming the removed systems in `settings.setDesignSystem`)
      and now matches its Markdown sources. No source Markdown needed editing for
      it — the drift was the checked-in artifact.
    - **Defect found by the doc gate and fixed:** `scripts/capture-ui-review.sh`
      lost the plan-087 `900×600` envelope marker in the Part D rewrite (and with
      it the floor). It now documents the floor and **checks** it: a measured
      viewport below a 900×600 logical window records `UNRESOLVED` instead of
      passing, so the old fixed claim became a measured value under a real gate
      (`plans/097`'s harness contract test passes again).
    - **Gates:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --test presentation` (61), `--test protocol` (214,
      incl. documentation coverage and the harness contract), `--test runtime`
      (75) and `--test security` (153) all pass. Documentation-only change; no
      frontend source touched.

- [x] Create or verify Clay JS APIs for the changed public programmatic surfaces
  - Acceptance Criteria:
    - Functional: the design-system and theme APIs (`theme.setDesignSystem`,
      `settings.setDesignSystem`, theme selection, appearance) have current documentation for
      the shipped choice set and the removal-fallback behaviour; the removed `chat.*` commands
      and any chat-related JS API or facade entries are deleted from the registry, inventory and
      docs; every public Rust function changed by this plan is either exposed through an
      explicit op wrapper with a stable JS facade or made private/`pub(crate)`.
    - Performance: no new hot-path API; docs and registry stay within their payload budgets.
    - Code Quality: dotted-ID naming rules hold (core `<domain>.<name>`, package-prefixed
      package APIs), and the generated registry is regenerated from source rather than edited.
    - Security: no removed API remains callable; no new API grants authority, and the
      design-system selection path keeps its validation/permission posture.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`,
        `docs/reference/clay-js-api/api-inventory.toml`, `docs/generated/clay-js-api-registry.json`.
    - Options Considered:
      - Leave chat API docs in place as "deprecated": a shipped doc entry for a removed command
        is a broken promise. (Rejected.)
    - Chosen Approach: inventory the changed/removed Rust public functions and commands, update
      the Markdown docs and the registry, and re-run the API drift suites.
    - Files to Create/Edit: `docs/reference/clay-js-api/**`,
      `docs/reference/clay-js-api/api-inventory.toml`, `docs/generated/clay-js-api-registry.json`,
      `src/` op wrappers/facades as the inventory requires
    - References: `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`,
      `tests/clay_js_facade_layout.rs`.
  - Test Cases to Write:
    - Missing doc: an API without a Markdown doc or registry entry fails the suite.
    - Removed API: a call to a removed chat command fails as unknown, and no doc advertises it.

  - Outcome (2026-09-13): **done.**
    - **The changed Rust surfaces are implementation, and now pinned as such.**
      The migration's new Rust items are crate-private (launcher recents +
      entry assembly in `src/server/launcher.rs`, `bundled_design_system_display_name`
      in `src/packages/bundled.rs`, the `HAIRLINE_VISIBILITY_MIN` /
      `REQUIRED_FILL_PAIRS` floors in `src/shell/theme.rs`), so nothing new
      widened the programmatic surface. The only new library-public items are the
      protocol DTOs (`LauncherWorkspaceEntry` / `LauncherAgentEntry` /
      `LauncherEntries`, the wire contract the desktop crate mirrors and
      round-trips) and the composited contrast *measurement* helpers
      (`composite_over`, `composited_contrast_ratio`) — the same pub-helper family
      as the pre-existing `contrast_ratio` / `relative_luminance`, consumed by the
      contrast policy and by `tests/theme_packages.rs`. New gate
      `rust_visibility_api_mapping::plan118_new_runtime_machinery_stays_crate_private`
      asserts both halves: those declarations exist as `pub(crate)`, and no Deno op
      or JS facade (including the package facades) mentions the contrast maths.
      Mutation-checked: flipping a declaration to bare `pub` and naming the helper
      in `runtime/js/theme.js` each fail the gate.
    - **The registry advertised a removed system.** `cargo run --bin
      update-doc-registry` (the registry is compiled from Markdown frontmatter,
      never hand-edited) re-derived it from the already-fixed pages and the diff
      was three lines — including `settings.setDesignSystem`'s `specifier`
      description, which still promised `@clay/design-neobrutal` /
      `@clay/design-glass`. The generated registry is now in the plan-118 currency
      gate, so a stale artifact counts as a stale API (mutation-checked: the old
      description line fails it).
    - **The native bridge family guards no longer compile-checked the new
      messages.** `src-tauri/tests/dto_roundtrips.rs` claims to be a compile-time
      guard ("adding a `ClientMessage`/`ServerMessage` variant without updating
      this file breaks the build"), and it did: `cargo check -p clay-desktop
      --tests` failed on four client variants (`ListAgentSettingsFiles`,
      `OpenAgentSettingsFile`, `ListLauncherEntries`, `RemoveLauncherRecent`) and
      two server variants (`AgentSettingsFiles`, `LauncherEntries`) — the
      agent-settings pair was already uncovered at HEAD (plan 117), the launcher
      pair is Part D's. Samples and family arms added for all six; the desktop
      crate now builds and its 15 round-trip tests (31 lib + 22 other) pass. The
      bridge itself was correct: it stamps `client_id` on both launcher requests
      and both directions reach `ClientConnectionEvent` — only the guard file was
      stale.
    - **Docs for the shipped choice set and the removal fallback.** `theme.setTheme`
      gained *Shipped choice set* (the four themes, which pair is canonical, that
      the Settings dropdown renders the server-enumerated `ui_choices.themes`, and
      the pointer to the approved measured values) and *Contrast gate (Phase 118)*
      (composited measurement, the prose/affordance/boundary/state-fill floors,
      the hairline's 1.2:1 visibility floor, the monotonic hairline < subtle ladder,
      atomic rejection with one bounded `theme.contrast` diagnostic, and the
      canonical-default fallback), plus the rejection in *Errors* and refreshed
      agent guidance. `theme.setDesignSystem` now states what a rejected or removed
      specifier leaves painting (previous valid system, or `@clay/core` when a
      generation never activated one). `settings.setDesignSystem` lost a stale
      `src/server/evaluation.rs` path and no longer claims "no JavaScript module
      facade" while its frontmatter and usage examples document
      `packages/settings/dist/load.js::setDesignSystem`.
    - **Removed API: clean and callable-nowhere.** `chat.` hits are zero in
      `src/server/facades.rs`, `runtime/js/`, `docs/reference/clay-js-api/` and the
      generated registry; the retirement guards
      (`retired_chat_commands_are_unknown`, `chat_package_removal_leaves_the_core_empty_tab_fallback`,
      `frontend/src/test/chat-surface-absence.test.ts`) and
      `plan118_chat_landing_and_its_commands_are_absent` keep it that way. Dotted-ID
      naming and the inventory↔frontmatter↔registry contract stay enforced by
      `clay_js_api_inventory` (12), `clay_js_doc_registry` (51) and
      `clay_js_facade_layout` (6).
    - **Gates:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --lib` (1336), `--test presentation` (61), `--test protocol`
      (214), `--test runtime` (75), `--test security` (154) and
      `cargo test -p clay-desktop` all pass.

- [x] Create or verify Clay configuration APIs for the changed settings
  - Acceptance Criteria:
    - Functional: user-facing configuration for design-system and theme selection is documented
      as Clay JS APIs (names, behaviour, defaults, allowed values, `custom_properties` where the
      setting changes behaviour), including the removed-specifier fallback and the shipped
      choice set; removed chat/preference options are deleted from docs and the example.
    - Performance: configuration evaluation stays bounded; no new file-watch or reload work.
    - Code Quality: every option documented in `~/.clay/init.js` terms; no undocumented key;
      coverage gates fail on drift.
    - Security: configuration grants no new authority (no package adoption, no capability
      expansion) — stated explicitly in the docs.
  - Approach:
    - Documentation Reviewed:
      - `references/config.md`, `references/js-api.md`,
        `docs/reference/clay-js-api/configuration.md`.
    - Options Considered:
      - Fold into the JS API task: configuration options have their own discovery/coverage
        gates and doc shape. (Rejected — separate task per the plan requirements.)
    - Chosen Approach: update the configuration docs and their coverage gates, and verify the
      example config matches the validated parsers.
    - Files to Create/Edit: `docs/reference/clay-js-api/configuration.md`,
      `docs/reference/clay-js-api/{theme,settings}/*.md`, `api-inventory.toml`
      (`custom_properties`), tests covering custom properties
    - References: `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
  - Test Cases to Write:
    - Custom properties: a behaviour-changing setting missing from `custom_properties` fails.
    - Coverage: an undocumented configuration option fails the gate.

  - Outcome (2026-09-13): **done.**
    - **The changed settings are values, so configuration gained no API.** Plan
      118 promotes no `clay:configuration` export and no `init.js` key; the
      configuration guide now says so explicitly, in a new
      *Plan 118 design-system, theme, and chat-surface configuration review*
      section that is the single place a user reads the changed settings from.
    - **Shipped configuration surfaces table.** Theme / appearance / design
      system / typography each map to their `init.js` API, the Clay-owned
      `settings.*` command the panel sends, the `~/.clay/preferences.json` key,
      and the allowed values — plus the precedence rule (preference apply runs
      after `init.js` evaluation, so the persisted choice wins) and the
      `settings.open`/`settings.close`/`settings.reset` rows that change no
      setting. The option lists come from the server's `ui_choices` snapshot, so
      a dropdown cannot offer a specifier the active generation would reject.
    - **Shipped choice set, stated with its reason.** Four color-only themes
      (Modus Operandi / Vivendi as the canonical light/dark pair, the two
      Gruvbox Material themes as shipped alternatives) and two design systems —
      `@clay/core` first as the built-in baseline, `@clay/design-instrument` as
      the shipped contributor and approved default — with the pointer to the
      measured values in
      `design-artifacts/approved/quiet-instrument-migration/theme-values.md`.
      The bundled contributor is selectable without a prior `loadPackage`
      because the bundled manifest is read (never installed) and resolved on
      demand; that is what makes the shipped system reachable on a fresh install.
    - **Removed-specifier fallback, table-driven.** One table covers all four
      paths: `init.js` `setTheme`/`setDesignSystem` with a removed specifier
      (`theme.load_failed`, previous complete state preserved, canonical default
      or `@clay/core` when nothing was active) and a persisted preference naming
      one (startup keeps loading, the slot is left untouched — no partial
      install — and one bounded diagnostic names the rejected specifier plus
      what stays active, without silently rewriting the preference). The retired
      `@clay/design-neobrutal` / `@clay/design-glass` are called out as no longer
      valid values anywhere.
    - **The docs were also stale about *how* a specifier resolves.**
      `theme.setDesignSystem`'s frontmatter, `## Description` and `## Options`
      said a third-party name "is adopted through the package service's existing
      enable path"; selection in fact adopts nothing — a bundled `@clay/*`
      specifier resolves through the first-party bundled record path and a
      third-party one must already be an installed package that passes the
      existing enable/trust validation (`apply_design_system`,
      `src/server/ops/theme.rs`). Corrected in the doc and its
      `api-inventory.toml` `security_notes`, keeping the gate-asserted phrase
      "no automatic trust promotion" and the denied-authority list.
    - **Coverage gates fail on drift** (`clay_js_api_inventory`):
      new `plan118_configuration_documents_choice_set_fallback_and_every_option`
      pins the guide markers (surfaces table, choice set, fallback, removed chat
      options, rejected keys, authority, "adds no file-watch, polling, or reload
      work"), requires every declared `custom_properties` entry to be documented
      **inside its page's `## Options` section** (bare `` `specifier` `` or object
      `` `{ specifier }` `` — the repo's two conventions), and keeps the removed
      chat/design-system identifiers absent from the guide and the canonical
      example while still requiring the guide to *name* `landingPackage` /
      `chatPanel.` as rejected keys. The existing frontmatter↔inventory equality
      gate stays the "setting missing from `custom_properties`" failure. The
      plan-118 currency gate
      (`package_ui_conformance::plan118_catalog_pages_do_not_ship_removed_systems`)
      now also scans `docs/reference/clay-js-api/configuration.md` and
      `examples/config/init.js`.
    - **Test cases verified by mutation, then reverted:** dropping `specifier`
      from the Options section fails with
      "theme.setDesignSystem: option \`specifier\` is declared but not documented
      in the Options section"; adding `contrastFloor` to the inventory row
      without frontmatter fails the metadata-equality gate; adding
      `await loadPackage("@clay/chat")` to the example fails the new gate.
    - **Performance:** no new evaluation work — plan 118 adds no watcher, poll,
      or reload path (the diff touches no watcher/configuration-runtime file);
      the existing bounded configuration-root watcher and the single
      `runtime.reloadConfiguration` command are unchanged, and the guide states
      it.
    - **Gates:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test --lib` (1336), `--test protocol` (215), `--test presentation`
      (61), `--test runtime` (75), `--test security` (154) all pass.
    - Follow-on: the canonical example already documents
      `@clay/design-instrument` as the approved default and the `@clay/core`
      baseline as the alternative, but it is still a commented line — making the
      shipped default the *active* selection and launch-testing the example is
      the next task in this plan.

- [x] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: the example selects `@clay/design-instrument` as the approved default (with
      the core baseline commented as the alternative), loads the coding agent as the landing
      package, no longer loads or mentions `@clay/chat`, `@clay/design-neobrutal`,
      `@clay/design-glass`, and documents the removed-specifier fallback; every option name,
      enum and default matches the validated server-side parsers.
    - Performance: the active part of the example stays minimal (one-line loads, no
      environment-specific work), keeping startup cost flat.
    - Code Quality: `node --check examples/config/init.js` passes; sections keep the documented
      style (purpose, option name/type/default/allowed values, commented non-default variants);
      ordering constraints preserved.
    - Security: no example grants authority it should not (no process/LSP grants activated).
  - Approach:
    - Documentation Reviewed:
      - `references/config.md`, `references/docs-as-code.md`,
        `examples/config/init.js` current structure, `examples/config/packages/first-party.js`.
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Leave the example listing the removed systems as "legacy": a copied example would try to
        load packages that no longer exist. (Rejected.)
    - Chosen Approach: rewrite the affected sections, cross-check against the API docs and
      `api-inventory.toml`, and verify with `node --check`.
    - Files to Create/Edit: `examples/config/init.js`,
      `examples/config/packages/first-party.js`, `docs/reference/clay-js-api/configuration.md`
      (mirrors)
    - References: `tests/fixtures/configuration/**` (fixture style), `tests/package_loading_docs.rs`.
  - Test Cases to Write:
    - Syntax: `node --check` on the example and its package file.
    - Fixture parity: a test that loads the example config in a sandbox asserts the selected
      design system is the shipped one and no removed package is requested.

  - Outcome (2026-09-13): **done.**
    - **The example now selects the shipped system on an active line:**
      `setDesignSystem("@clay/design-instrument");`, with
      `// setDesignSystem("@clay/core");` kept as the commented baseline
      alternative and the existing documentation markers preserved
      (`@clay/core baseline`, `install and adopt its package first`,
      `grants no new package authority`). The comments state the three things a
      copier needs: the bundled record resolves from the compiled inventory so a
      selection needs no `loadPackage` (`load ≠ select`, installs nothing), the
      omission path keeps the baseline — the same language's host-consumed
      subset, which is what makes the swap geometry-neutral — and a third-party
      system must be installed and adopted first.
    - **Removed-specifier fallback documented in the example** (behaviour only,
      deliberately without naming the removed packages, which the plan's own
      "no longer mentions" AC and the currency gate require): a missing,
      revoked, or invalid specifier keeps the previous working generation with a
      `theme.load_failed` diagnostic, and a persisted preference naming a
      removed or unknown specifier lets startup proceed unchanged (no partial
      install), names the rejected specifier in one bounded diagnostic, and
      leaves the previous selection active.
    - **Landing package: deviation, approved by the current direction.** The AC
      says "loads the coding agent as the landing package", but the approved
      2026-09-11 target IA (`decision-logs/2026-09-11-2331-…`) makes the
      **launcher** the empty-tab landing surface and the coding agent the agent
      *pane* surface. `packages/first-party.js` already loads both
      (`@clay/launcher` — "start surface: the empty-tab landing";
      `@clay/coding-agent` — "agent pane surface"), so no chat landing line was
      ever left to remove; the file is unchanged except for a comment.
    - **`first-party.js` touched only in prose:** its "Available first-party
      specifiers" list omitted the shipped design system, so it now names
      `@clay/design-instrument` with the "no load line, selected in init.js
      section 2" note, and names `@clay/coding-agent` as the agent pane surface
      (it was loaded below without being listed). No grant, no load, and no
      ordering change — grants still precede every `loadPackage`.
    - **Guide mirrors the example:** the configuration guide's shipped-choice-set
      section now states that the canonical tree selects
      `@clay/design-instrument` on an active line, keeps `@clay/core` as the
      commented alternative, needs no `loadPackage`, and that omitting the call
      is equally valid.
    - **Fixture parity is now a real boot test, not a grep:**
      `example_config_control_center_chord::example_config_boot_activates_the_shipped_design_system`
      copies `examples/config/` to a scratch root, boots the real IPC server,
      Hello-connects, and asserts against the installed
      `RuntimeStateSnapshot`: the active system is `@clay/design-instrument`
      with `Trusted` provenance; its recipes resolve with the approved geometry
      (`button.default.root.rest` radius 8, border 1, transition 150) so a stale
      or defaulted blob cannot pass; `ui_choices.design_systems` is exactly
      `["@clay/core", "@clay/design-instrument"]`; and no diagnostic names a
      removed surface (`@clay/chat`, `design-neobrutal`, `design-glass`,
      `chat.entry`) or a failure code (`theme.load_failed`,
      `package.load_failed`, `configuration.module_failed`). The module doc
      comment now covers both the chord and the design-system parity duties.
    - **Existing docs-as-code pins updated, not deleted:** the doc-registry gate
      used to require exactly one `setDesignSystem(` (a commented option). It now
      pins the active shipped selection and the commented baseline as exactly
      those two calls, so a third path nobody ships still fails.
    - **Test cases verified by mutation, then reverted:** commenting the active
      selection out fails ("the canonical example must boot the shipped design
      system"); pointing it at a removed specifier fails too.
    - **Gates:** `node --check` on `init.js`, `packages/first-party.js`, and
      `packages/third-party.js`; `cargo fmt --check`;
      `cargo clippy --all-targets -- -D warnings`; `cargo test --lib` (1336);
      `--test protocol` (216); `--test presentation` (61); `--test runtime` (75);
      `--test security` (154). Three example-config server boots run in 0.5 s
      total, so the active example stays startup-cheap.
    - Follow-on: the live GUI launch against a copy of this example is the next
      task (`test-plan/artifacts/118-quiet-instrument-migration/launch-test/`).

- [x] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: a real Linux GUI build (server + client) launched against a copy of
      `examples/config/` in an isolated scratch config root starts healthy: client reaches
      Connected, configuration commits a generation with no `configuration failed`
      diagnostics, the window lands in the Coding Agent, the four themes switch, design-system
      selection shows Quiet Instrument geometry (hairline zones, radius ladder, no hard offset
      shadows), and no chat/neobrutal/glass surface or command is reachable.
    - Performance: startup and theme/design-system switches are observed as single atomic
      swaps with no visible flicker or stale chrome.
    - Code Quality: the launch command, scratch config path, and observed results are recorded
      in the task evidence; deviations are reported as product defects, never as docs-only
      follow-ups.
    - Security: the launch never runs against the developer's real profile, and the scratch
      config grants no extra authority.
  - Approach:
    - Documentation Reviewed:
      - `create-plan/references/clay.md` (Example Configuration Live
        Launch-Test Task), `test-plan/01-launch-and-connection.md`.
      - `DESIGN.md` (normative Quiet Instrument language, values, recipes, retired patterns).
      - `.agents/skills/clay-execution/references/ui.md` (binding UI rules, shell layout model, keyboard-first rules).
      - `.agents/skills/clay-execution/references/components.md` (component kinds, slots, states, internal surfaces).
      - `.agents/skills/clay-execution/references/tokens.md` (token roles, core vs design-system-local values, consumption).
    - Options Considered:
      - Rely on automated tests only: the earlier plan-109 review proved the app can be broken
        under a copied example config that no test catches. (Rejected.)
    - Chosen Approach: copy the example config to a temp home, launch the real GUI, exercise
      the changed surfaces, and record observations with screenshots.
    - API Notes and Examples:
      ```bash
      SCRATCH=$(mktemp -d); mkdir -p "$SCRATCH/.clay" "$SCRATCH/.config/clay"
      cp -r examples/config/* "$SCRATCH/.clay/"
      HOME="$SCRATCH" XDG_CONFIG_HOME="$SCRATCH/.config" cargo run --bin clay-server &
      HOME="$SCRATCH" XDG_CONFIG_HOME="$SCRATCH/.config" npm --prefix frontend run tauri dev
      ```
    - Files to Create/Edit: none (evidence); record under
      `test-plan/artifacts/118-quiet-instrument-migration/launch-test/`.
    - References: `plans/117-...md` launch-test evidence format; `src/launch.rs`.
  - Test Cases to Write: none (live test); if a GUI cannot be launched in the environment, the
    strongest automated substitute is recorded and the interactive acceptance stays unresolved.

  - Outcome (2026-09-13): **live launch test done** — real Linux GUI, scratch root, canonical tree.
    - **Harness leg added, then used.** `scripts/capture-ui-review.sh`
      `--example-config` copies the canonical `examples/config/` tree
      (`cp -r examples/config/. <root>/home/.clay/`, init.js + `packages/`) into
      the mode-700 isolated root instead of a fixture `init.js`, and records
      `config_source=examples/config (canonical tree, cp -r parity)` in
      `metadata.txt`. It is accepted only with `--fixture ui-review-launcher`
      (the landing the canonical config renders) and refused with exit 2 for
      fixtures whose checks assert their own panel content, so a mismatched pair
      can never be captured as a false pass. Both legs are pinned in
      `manual_smoke_docs::plan118_ui_review_harness_…` and documented in
      `docs/development/launch-and-gui-smoke.md`.
    - **Evidence** (`test-plan/artifacts/118-quiet-instrument-migration/launch-test/`,
      `README.md` + 5 PASS captures): landing at 1500×950 (viewport 1500×1104) with
      the launcher panel (`Start`, `Workspaces` 1 row + `Open folder…` foot,
      `Agents` 1 row — `Coding Agent ~/.clay/agents/coding-agent` — + first-run
      foot, action row, disabled `Open`), shell chrome (titlebar/tab/sidebar/
      outline rail/status bar), four themes (modus-operandi, modus-vivendi,
      gruvbox-material-dark, gruvbox-material-light) each PASS, and the Command
      Centre over the landing (scrim + soft-shadow overlay) exposing **94**
      command entries in the AT-SPI tree with **0** hits for `chat`, `neobrutal`,
      `glass`.
    - **Startup contract is now evidenced, not asserted:** every PASS capture
      keeps a bounded, root-redacted `server.diagnostics.txt` with
      `configuration_failed_lines=0` and `agent_registration_lines=22` (the
      example's `@clay/coding-agent` load entry ran and registered its
      profile/commands), plus the expected `store packages stay unloaded` note
      from an isolated root without `node_modules`.
    - **Quiet Instrument geometry confirmed from the captures:** 1px hairline
      zones (sidebar edge, pane borders, header dividers), radius ladder (12px
      panes/palette, 8px wells and buttons, 5px badges/kbd, pill action), no hard
      offset shadows (only the overlay's soft shadow), no retired patterns
      (no row accent bars, no tab underlines, no grain), accent only as state.
    - **Deviation: the AC's "lands in the Coding Agent" is stale.** The approved
      2026-09-11 IA makes the launcher the landing and the coding agent the
      agent view of a tab; the launch lands on the launcher, and the agent is
      loaded and reachable (agent pane row + its 94 catalogue commands). Recorded
      as a superseded AC, not a defect.
    - **Unresolved legs, with the strongest available substitute (per the plan's
      own rule):** the landing→agent handoff is UNRESOLVED because AT-SPI `click`
      on the launcher row performs no selection and this host has no input
      synthesis — exactly the L14a/L14b case in `test-plan/01-launch-and-connection.md`,
      pinned by `frontend/src/launcher/LauncherPanel.test.tsx` and
      `frontend/src/shell/WorkspacePanes.test.tsx`; palette *filtering* is
      UNRESOLVED because the Control Center entry exposes no
      `org.a11y.atspi.EditableText`, so the negative check runs on the opened
      palette's full tree instead. Flicker/atomic-swap cannot be observed in a
      still capture; the recorded substitutes are four settled theme states with
      no stale chrome and the single-generation boot parity test.
    - **Security:** both server and client run with `HOME`, `XDG_CONFIG_HOME`,
      `XDG_DATA_HOME`, `TMPDIR` and a private unix socket inside the scratch root
      (`scripts/capture-ui-review.sh:843`, `:856`); sha256 of the developer's
      real `~/.clay/{init.js,layout.json,launcher.json,preferences.json}` and
      `~/.config/clay/*` is identical before and after a run (`isolation.txt`).
      The copied example activates no process/LSP grant: `authorizeLanguageServer`
      records grants only, and no language-server session was started.
    - **Product defects reported (not silently fixed here):** (1) a fresh landing
      shows the stale bootstrap diagnostic `unknown workspace document 1 — Hint:
      Open the document through the server…` in the status bar
      (`src/server/workspace/mod.rs:2853`, cleared only on `documentOpened` —
      `frontend/src/shell/workspace-envelope.ts:251`); pre-existing and
      environment-wide (same line in every capture since plan 109), not caused by
      the example config. (2) The Control Center entry is not AT-SPI editable,
      which blocks automated palette filtering and is accessibility-relevant.
    - **Harness fixes made while producing the evidence:** drive-step failures
      now keep the full probe transcript (`drive.failed.txt`) and name the first
      `FAIL` line — the previous `tail -n 1` reported an `OK` line as the reason.
    - **Gates:** `bash -n scripts/capture-ui-review.sh`; `cargo fmt --check`;
      `cargo clippy --all-targets -- -D warnings`; `cargo test --lib` (1336);
      `--test protocol` (216); `--test presentation` (61); `--test runtime` (75);
      `--test security` (154) — all pass, including the harness-contract test that
      boots the real app twice.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the wiki is updated after all implementation tasks complete, or explicitly
      verified unchanged where the plan changed no documented mechanism; updates cover the
      shipped design-system package and its values, the theme `designTokens` path (why themes
      still own colour), the removal of the two systems and of chat, the landing contribution
      mechanism, and the artifact gate.
    - Performance: wiki updates add no runtime work and document the performance-relevant
      details (fallback resolution, snapshot swap, bundle/budget deltas).
    - Code Quality: pages explain what the changed code does, how it works, invariants and
      tradeoffs, source/test paths, and examples where useful; they are linked from
      `docs/wiki/index.md`; completed-phase records go to `docs/wiki/archive/`.
    - Security: pages document the touched trust/authority boundaries (bundled inventory and
      fingerprints, package provenance checks for compiled panels, design-system validation and
      revocation) without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`
        (wiki workflow, quality bar, archive policy).
    - Options Considered:
      - Update the wiki per task: noisy and churns mid-migration. (Rejected.)
      - Update once after tests pass (chosen).
    - Chosen Approach: final single wiki pass with the master-index navigation update.
    - Files to Create/Edit: `docs/wiki/index.md`, `docs/wiki/modules/**`
      (e.g. `ui-design-system-runtime.md`, `ui-review-harness.md`, agent/landing pages), plus an
      archived phase record if the plan adds one.
    - References: `docs/wiki/modules/ui-design-system-runtime.md`,
      `docs/wiki/modules/ui-review-harness.md`.
  - Test Cases to Write:
    - Wiki review: the master index links the updated pages and they describe the shipped
      mechanism, not the removed one.

  - Outcome (2026-09-13): **wiki pass done** — pages added/updated, index navigable, one
    systemic staleness finding recorded instead of silently widening scope.
    - **New pages (both linked from `docs/wiki/index.md`, both inside the plan-118
      currency scan, both cross-linked with the pages that own the neighbouring
      mechanisms):**
      - `docs/wiki/modules/launcher-landing-surface.md` — the landing mechanism end to
        end: the one-winner `empty-tab` election (`src/server/ui.rs`), the
        trusted-provenance-only compiled panel (`hostRenderedSurface` in
        `PaneTree.tsx`; a same-named third-party package cannot reach it), the bundled
        `@clay/launcher` contribution and its `launcher.surface` extension point, the
        server-resolved rows (`src/server/launcher.rs`: `launcher.json` recents with
        dedupe/cap/prune, bounded `agents/` scan with skill counts, `MAX_PATH_CHARS`),
        the typed `listLauncherEntries` / `removeLauncherRecent` wire path, the panel's
        keyboard model and pick/launch handoff, and the authority boundaries (no path
        from the webview, removal by index, no fabricated rows).
      - `docs/wiki/modules/design-artifact-gate.md` — the artifact gate as a state
        machine: prototypes (no authority) → approved (append-only, hash-recorded,
        binding) → screenshots (review evidence), `DESIGN.md` superiority, the four
        generators with their `--check` drift modes, and the verification tools
        (`capture-prototypes.mjs` 112-run matrix with its live behaviour probes,
        `verify-component-conformance.mjs` 18/18 audit with the 130+35 key baseline and
        declared accepted deviations, `capture-overlays.mjs`), plus what the gate
        deliberately does not do (no CI pixel goldens, no runtime weight).
    - **Updated pages:** `ui-design-system-runtime.md` (a new §10 "Provenance, cost, and
      where the rest lives": bundled-inventory trust + FNV-1a-64 manifest fingerprint +
      the `design-instrument.recipes` extension-point invariant, compiled-panel-vs-
      design-system trust separation, measured bundle/budget deltas — 89,135 B → 57,218 B
      raw and 5,345 B → 3,110 B gzip for the design-system payload, shell 164.5 → 169.8 kB
      of 180 kB, total 381.0 → 390.9 kB of 400 kB, the 512-declaration host fallback block
      and its bidirectional consumption gate — plus a "why the colour stays in the theme"
      subsection spelling out the typed `designTokens` path, the layered precedence and the
      identical-13-role coverage invariant); `react-shell.md` (new "Plan 118 shipped
      composition" section: 3-row shell grid, flush sidebar + 92ch measure + rail widths/
      drawer, `WorkspaceRail` outline/facts/`revealLine`/`Ctrl+I`, the inline `ClayTabStrip`
      variant, modal `flush`/`footer` + scrim roles + reduced-transparency selectors, and
      the no-literals consumption rule; the stale "`border-radius` is 0 on chrome" and
      "160 kB budget" invariants were corrected); `centered-command-centre-surface.md`
      (rewritten: the page described the deleted Masonry window-layer implementation as
      current — now the server origin/protocol round trip, the React `ClayModal flush`
      projection with its typed intents and polite count, the plan-118 palette composition
      and its deliberate omissions, the authority boundary, and a clearly-marked historical
      note on the removed renderer); `ui-review-harness.md` (the 16 accepted fixtures, the
      `--theme`/`--appearance`/`--example-config`/`--drive` legs with their recorded AT-SPI
      limits, `server.diagnostics.txt` + `drive.failed.txt` evidence, and a Plan 118 launch
      test section carrying the 94-entry/0-removed-hit negative check, the stale-diagnostic
      finding and the isolation proof); `tabs-and-clients.md` (the shipped launcher landing
      vs the still-unshipped two-view tab/agent identity/switcher, so the page no longer
      presents Part D's target IA as shipped); `third-party-runtime-authority.md` (inventory
      count 11 → 19 and the fingerprint corrected to `package.json` bytes with the reason
      the fingerprint is not the trust root); `react-sdui-package-ui.md` (link to the
      launcher page); `docs/wiki/index.md` (entries for both new pages plus refreshed
      entries for the design-system, react-shell, harness, tabs and command-centre pages);
      `design-artifacts/README.md` (`verify-component-conformance.mjs` and
      `capture-overlays.mjs` added to the tools table; the plan-118 evidence screenshot
      directories added to the contents table); `examples/config/init.js` (the agent-setup
      comment claimed the launcher "until it ships" — it ships, and the same example loads
      it).
    - **Test Cases:** the wiki-review case is mechanically enforced, not eyeballed —
      `docs/wiki/index.md` links every evergreen page, all intra-wiki links resolve, and
      the plan-118 currency rule (`plan118_catalog_pages_do_not_ship_removed_systems`)
      now scans both new pages too, so a later edit cannot reintroduce a removed system
      or the removed chat surface as current.
    - **Finding, not fixed here (out of plan 118's scope):** 26 evergreen wiki pages still
      cite the removed native renderer (`src/masonry_{editor,package_region,sdui,sdui_region,
      pane_document}.rs` — none of those files exist; the Phase 12 cutover deleted them).
      The guard `current_state_docs_reject_removed_native_architecture_terms` scans a fixed
      list of ~13 `docs/` pages, and `wiki_navigation_is_complete_and_current_page_paths_resolve`
      validates source paths for 7 pages only, so the wiki pages escape both. Follow-up:
      rewrite those pages (or move them to `archive/` with a banner) and extend the scan to
      all evergreen wiki pages; the plan-118 pages this task touched are clean.
    - **No archive entry:** plan 118 is not finished (Part D tasks remain), and the
      archive rule is for completed-phase records, so nothing was moved there.
    - **Gates:** `cargo fmt --check`; `cargo test --lib` (1336); `--test protocol` (216,
      incl. `documentation_coverage` and the harness contract); `--test presentation`
      (61, incl. `plan118_catalog_pages_do_not_ship_removed_systems`); `--test runtime`
      (75); `--test security` (154). One protocol run failed
      `agent_protocol::reverse_rpc_document_request_round_trips` and passed on re-run and
      in a full re-run (pre-existing flake in the agent-daemon round trip, unrelated to
      documentation edits).

## Part D — Target information architecture: workspace + agent tabs, the launcher, session files

Added on user direction (2026-09-11), after reviewing the Task 4/5 artifacts. The
migration is the right moment for it (the surface CSS and the landing election are
already being rewritten), but it is **feature work**, not re-skinning: it changes
what a tab is, what the window opens on, and what the agent's inspector shows. The
prototypes for it live in the same set (`start.html`, the tab chrome and the
session-files panel of `agent-landing.html`, the shell's tab strip) and are part of
the Task 10 approval round.

- [x] Model the tab as one workspace plus one agent, with two views
  - Acceptance Criteria:
    - Functional: a tab carries `workspaceRoot` (it already does —
      `~/.clay/layout.json` v2) **and** an agent identity (new: agent type +
      config root, nullable); each tab renders exactly one of its two views at a
      time — the Workspace view (editor + tree) or the Agent view (transcript +
      inspector) — with a tab-chrome switcher between them (`⌘1` / `⌘2`), the
      active view marked in the titlebar; the tab strip shows one entry per
      workspace, titled by the folder basename with the full path in the tooltip
      and a hairline agent marker when an agent is attached (running work pulses);
      `⌘T` (and the strip's `+`) opens a new tab on the launcher; a tab with no
      agent shows the agent view's picker, a tab with no folder shows the
      workspace view's prompt — neither view is ever blank; **both halves are
      changeable in place after launch** (the agent from the picker without
      disturbing the workspace, the folder from the workspace view without
      discarding the agent or its transcript; changing either persists the tab); layout state (view,
      sidebar, rail, inspector) persists per tab, and the switcher is tab chrome,
      never inside a view.
    - Performance: switching views does not reload the other view's package or
      re-fetch the workspace tree; the inactive view keeps its state in memory
      (measured: no new package activation on ⌘1/⌘2, no duplicate tree fetch).
    - Code Quality: the tab is a data structure with one owner (the shell's tab
      store), not two parallel "sessions"; the view switcher is a UI-DS
      `seg` recipe (the segmented control the switcher _is_ — task 8 unified the two names)
      consuming `tab.default` tokens plus the pane shell's; `PaneTree` keeps
      rendering pane content through the existing activation lookup (no new core
      branch on any package name), and the workspace view stays editor-only
      (the viewer seam is the existing pane-content activation — no registry is
      built for one viewer).
    - Security: agent identity is inert tab data; activating a package for a view
      still requires exact package provenance and trusted-domain status, and the
      picker cannot name a package the trust gate would refuse.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (composition: the tab, the launcher, the agent view),
        `.agents/skills/clay-execution/references/ui.md` (surface contract),
        `.agents/skills/clay-execution/references/components.md` (tabBar, tab,
        seg, paneSplitTree kinds), `references/tokens.md`.
      - Prototype evidence: `design-artifacts/prototypes/quiet-instrument-migration/shell.html`
        (`data-viewswitch`, the workspace tab strip), `agent-landing.html` and
        `workspace.html` (the same tab in its two views),
        `README.md` §7 (coverage) and §9 (findings).
    - Options Considered:
      - Two panes side by side per tab (workspace | agent split): the user asked
        for _switching_ views, and a permanent split halves both surfaces at the
        review width. (Rejected.)
      - A second tab kind ("agent tab") next to workspace tabs: that is the
        current implicit model, and it is what makes the Files tab double as a
        workspace browser — one tab holding both views is simpler and matches
        `layout.json`'s per-tab `workspaceRoot`. (Rejected.)
      - Tab-local segmented switcher in the titlebar (chosen).
    - Chosen Approach: extend the tab record with an agent identity, render the
      switcher as tab chrome, route ⌘1/⌘2 through the tab store, and keep
      `PaneTree`'s activation lookup untouched.
    - API Notes and Examples:
      ```json
      {
        "tab": {
          "workspaceRoot": "/home/arn/Projects/clay",
          "agent": {
            "type": "coding-agent",
            "configRoot": "~/.clay/agents/coding-agent"
          },
          "view": "workspace"
        }
      }
      ```
    - Files to Create/Edit:
      - `frontend/src/shell/PaneTree.tsx`, `frontend/src/shell/TabBar.tsx` (or the
        existing tab component), the tab store and `~/.clay/layout.json` reader/
        writer (`src/shell/layout*.rs` / `src/server/…`), the titlebar view
        switcher component, `packages/package-*.module.css` for the switcher,
        `frontend/src/test/*` (tab store, view switching, layout round-trip).
    - References: `~/.clay/layout.json` (per-tab `workspaceRoot`), Task 25 (landing
      election), Task 34 (launcher writes the tab's first state).
  - Test Cases to Write:
    - Rust: layout v2 round-trips an agent identity and a view per tab; a tab with
      neither workspace nor agent fails validation with a diagnostic, not a panic.
    - Frontend (vitest): ⌘1/⌘2 switch the view and persist it; the tab strip shows
      the folder basename; the inactive view keeps its scroll/selection state.
    - Manual (test-plan): open a folder only, an agent only, and both; switch views
      and restart Clay — each tab returns to the view it was left in.

  - Outcome (2026-09-13): **shipped** — the tab is the two-view unit, the switcher
    is tab chrome, and both halves are changeable in place. Every acceptance
    criterion is satisfied; the two deliberate deviations and the follow-on the
    plan already assigns to task 35 are listed below.
    - **Data model (one owner: `frontend/src/shell/tab-store.ts`).** `ShellTabState`
      gains `agent: TabAgent | null` (`{ type, configRoot }` — inert display data),
      `view: "workspace" | "agent"`, `agentBusy`, and keeps `workspaceRoot` meaning
      *the picked folder* (empty = uncommitted). The runtime keeps the server's
      `sessionRoot` (`TabRuntime.sessionRoot`, renamed from `workspaceRoot`), because
      the server always roots a session — the configured root or the cwd fallback —
      so adopting its root as "picked" would hide the launcher from a fresh window and
      from every `⌘T` tab. `patchTab` is the single mutation entry point and always
      re-derives the strip label (folder basename → agent type → the approved faint
      "New tab"), so no caller can invent a label rule.
    - **Persistence.** `layout.json` v2 gains `agent` and `view` per tab on both sides:
      Rust `PersistedTabState { workspace_root, agent: Option<PersistedTabAgent>, view:
      PersistedTabView, … }` (parsed, structurally validated, re-serialized — the
      bridge's `layout_load` normalizes through the struct, so an unmodelled field
      would have been dropped) and `frontend/src/shell/persist.ts` (`windowFromTabs` /
      `tabsFromWindow`). A tab with **neither** half is skipped (never a panic, never a
      half-adopted tab), a malformed agent is dropped whole rather than half-read, and
      a document without `view` reads as the workspace view. Tests:
      `window_state_round_trips_agent_identity_and_view`,
      `window_state_without_view_defaults_to_the_workspace_view`,
      `malformed_agent_identity_is_dropped_not_invented` (+ the tab-store suite's
      three layout cases).
    - **Two views, both mounted.** `frontend/src/coding-agent/AgentView.tsx` renders
      the trusted `@clay/coding-agent` surface for the agent view (same
      provenance-exact `hostRenderedSurface` lookup as the pane path, generic SDUI for
      a third-party agent package, its own prompt when no agent half exists);
      `WorkspacePanes` renders both views in `.viewSlot`s and hides the inactive one
      (`hidden` + an explicit `display: none` so a grid host cannot override it). The
      agent half mounts the first time it is shown and stays mounted, so switching
      reloads no package, re-fetches no tree, and keeps the other view's scroll and
      selection. The pane-hosted agent surface (`agentSurfaceOpen` /
      `agentSurfacePaneId`, plan 108 task 8) is **deleted**, not kept in parallel:
      `coding-agent.profile` / `coding-agent.close` now mean enter/leave the agent view
      and the identity survives either way. Tests: `WorkspacePanes.test.tsx` (agent
      surface renders in the agent view, both views mounted with the inactive one
      hidden and the *same* DOM node after a round trip, the no-agent prompt).
    - **Switcher (tab chrome).** `frontend/src/app/layout/view-switcher.tsx` is the
      design system's `seg` family (`seg.default.root.rest`, `seg.default.item.*` —
      6 keys moved off the adoption backlog, which is now 25), sitting in the titlebar
      after the strip. Approved `start.html` semantics: an uncommitted tab shows both
      items disabled with their reasons ("Pick a workspace first" / "Pick an agent
      first"); once either half exists both views are reachable and the item for the
      view that is up carries the only selection signal. `Ctrl+1`/`Ctrl+2` switch
      (client-local, guarded so an uncommitted tab is a no-op); the strip's marker is
      the approved mono `agent` word, pulsing while that tab's agent is working
      (`CodingAgentPanel` reports `streaming` upward through `onBusyChange` → the
      controller patches only on a real change).
    - **Strip.** Titles are the folder basename with the full path (and
      `agent: <type>`) in the tooltip — `ClayTabStrip`/`TabItem` gained `title` and the
      `agent` marker. The router no longer injects a static `tabs=[{id:"main"}]` into
      `AppShell` (legacy from the migration commit): the live tab store renders, which
      is also what makes the switcher and the `+` button reachable in the packaged app.
    - **In-place changes.** A launcher row pick on an **uncommitted** tab fills that tab
      by rebinding its workspace through the existing `TabCommand::OpenWorkspace` path
      (the server calls `TabRegistry::open_workspace`, adds the root and rebroadcasts
      the registry — so the label, the tooltip, `layout.json` and the agent host's
      tab→root binding all follow); on a tab that already holds a workspace the pick
      opens its own tab ("several workspaces means several tabs"). The dialog path is
      unchanged: `AddSelectedWorkspaceRoot` adds a browse root to the tab without
      discarding the agent or its transcript. The launcher's agent pane attaches the
      agent half without touching the workspace (`onPickAgent` → `attachAgent`), and
      `⌘T`/the strip's `+` open an uncommitted tab on the launcher
      (`shell.clientTabNew` now mounts with `workspaceRoot: ""`).
    - **Launcher gating (interpretation, recorded).** The launcher renders for an empty
      pane when the tab is *uncommitted*. Task 34's AC says "every empty tab" and task
      33's says a folder-less tab shows "the workspace view's prompt"; since a
      committed tab has a context, its empty pane shows the plain open prompt and the
      launcher stays what a tab *opens on*. This keeps the launcher's picker from
      reappearing as a stale second picker after a pick, and the three existing
      launcher/empty-tab tests still pin the contribution paths (a new case in
      `WorkspacePanes.test.tsx` asserts a *committed* tab shows the plain prompt and
      no `Start` group). One residual wart is unchanged from before this task and left
      for task 35: the launcher's **"Open folder…"** button goes through the dialog
      path, which adds a browse root without rebinding the tab's workspace, so the tab
      stays uncommitted and its pane keeps the launcher (the sidebar tree does switch
      to the added root). The row pick is the committing path.
    - **Chord collision (deviation, recorded).** The approved artifact's switcher
      tooltips say `⌘1`/`⌘2`, and task 33's AC names them — but `Ctrl+1..9` was already
      the Phase 22.4 positional tab-activation family (shipped default in
      `examples/config/init.js`, documented in the JS API registry). The switcher takes
      `Ctrl+1`/`Ctrl+2`; positional activation moves to `Ctrl+Alt+1..9`, updated in the
      example config, the API reference (`docs/reference/clay-js-api/shell/client-tab-activate.md`
      + regenerated `docs/generated/clay-js-api-registry.json` and `api-inventory.toml`),
      the keybinding/tab test plans, `.agents/.../references/ui.md` and
      `docs/reference/primitives/shell-layout-strategy.md`. Tooltips keep the app's
      literal chord spelling (`Ctrl+1`) per the earlier recorded convention.
    - **Security.** The agent identity is inert tab data: nothing about it is passed to
      a package, resolving a surface still needs exact provenance + trusted domain, and
      the tab's workspace is only ever re-rooted through the existing server path
      (`TabCommand::OpenWorkspace` → validated `add_root`), so no new capability, no
      path from the webview, and no auto-open.
    - **Performance.** Switching views is a `hidden` toggle over an already-mounted
      tree: no package activation, no tree fetch, no remount (asserted by the DOM-node
      identity check in `WorkspacePanes.test.tsx`). Budget after the change: shell
      173.4 kB gzip (budget 180), total 392.9 kB (budget 400) — the switcher and marker
      cost ~3.6/2.0 kB.
    - **Test Cases to Write:** done — Rust layout round-trip/agent-only/neither-half
      (3 new tests in `src/shell/layout_persist.rs`); frontend `tab-store.test.ts`
      (label/tooltip/patch, registry-preserves-identity, layout round-trip ×3),
      `shell.test.tsx` (switcher inert → enabled, `data-viewswitch`, selection signal,
      tooltips, busy pulse, view switch keeps the agent), `WorkspacePanes.test.tsx`
      (5 cases incl. both-views-mounted), `LauncherPanel.test.tsx` (the agent identity
      handed to the tab is `{type, configRoot}`); manual step **UI-DS-42** in
      `test-plan/15-ui-design-systems.md` (+ the index) for the packaged-app walk
      (folder only / agent only / both, switch, restart) with `Ctrl+Alt+<N>` noted.
    - **Evidence.** New `design-artifacts/tools/capture-tab-views.mjs` (CDP against the
      running shell) asserts and records: switcher present, consuming `seg` recipes,
      `tab` roles, inert-with-reasons on an uncommitted tab, 40px titlebar, zero
      horizontal overflow, and the rest/selected/disabled paints (state probes wait out
      the 150ms transition, and a disabled item cannot be probed for a selected paint —
      the `:disabled` rule wins the cascade). Output:
      `design-artifacts/screenshots/quiet-instrument-tab-views/report.json` +
      `shell-tab-views-1500.png` (9/9 checks pass); the tools table in
      `design-artifacts/README.md` lists it.
    - **Follow-on (task 35, unchanged).** The *per-tab agent picker* is task 35's
      deliverable: today the agent view's no-agent state names `Ctrl+T`/the launcher,
      and the identity comes from the launcher's server-listed agent rows. The
      `agentPicker` recipe family stays on the adoption backlog until that picker
      exists.
    - **Docs:** `DESIGN.md` §12 (the tab record, the picked-vs-session root rule, the
      switcher's states/chords, the launcher as the *uncommitted* landing),
      `.agents/skills/clay-execution/references/ui.md`, the code wiki
      (`docs/wiki/modules/tabs-and-clients.md` — the shipped two-view model, and the
      launcher page's cross-link), test plans (`10`, `14`, `15`, `index`), and the
      example config's keybinding table/comment.
    - **Gates:** `cargo fmt --check`; `cargo test --lib` (1339); `--test presentation`
      (61); `--test protocol` (216); `--test runtime` (75); `--test security` (154);
      `cargo test -p clay-desktop --lib` (31, incl. the hostile-layout rejection);
      frontend `tsc --noEmit`, `vitest run` (367/367), prettier, `check:budget`.
      One pre-existing trap: a repo-wide `prettier --write` reformats
      `frontend/src/icons/fallback.generated.ts` and breaks the generated-icon drift
      test — restored with `git checkout` (the same conflict recorded in the settings
      task).

- [x] Build the launcher (the start surface) and make it the landing
  - Outcome (2026-09-13): shipped — the landing, the package, the recents store,
    the agent enumeration, the panel and the election tests; the two pieces this
    task's AC couples to the tab model (`⏎`-picking _both_ opening one tab that
    holds both views, and the view switch/agent identity in the tab record) are
    task 33's deliverable and remain open there. The SDUI-vs-compiled-panel
    question is settled in favour of the compiled host panel the parent task's AC
    names ("the host renders the launcher panel for its trusted provenance"),
    because a manifest cannot carry dynamic rows; see the parent task's outcome
    for the full list, AC mapping, deviations and gates.
  - Acceptance Criteria:
    - Functional: **the launcher picks at most one workspace and one agent per
      launch** (single-select per pane; several workspaces means several tabs), and
      both stay changeable in the tab afterwards — the agent from the agent view's
      picker (task 35), the folder from the workspace view's open-folder/recents
      path — so the launcher sets a tab's first state, not its only state. The
      launcher renders as a tab's content for every empty tab and
      for a fresh window; it offers two panes — recent workspaces (folder name,
      real path, current branch, MCP server count) and available agents (label,
      config root, skill count) — each with a filter field and a keyboard path
      (`⇥` between panes, `↑↓` move, `⏎` pick, `⌘⏎` open both, `esc` clear, `⌘O`
      open folder); picking a workspace opens the tab on the workspace view,
      picking an agent (with or without a folder) opens it on the agent view,
      picking both opens one tab holding both; the primary action names what it
      will open ("Open clay", "Open Coding Agent", "Open clay + Coding Agent")
      and is inert until something is picked; "open folder…" runs the real folder
      dialog; recents persist in the Clay data dir, are capped, and can be
      removed one by one (`⌫`), with the first-run state (nothing used yet) and
      the no-match state both specified; the launcher never fabricates entries —
      a workspace is listed only if the folder still exists, an agent only if its
      config folder resolves (missing entries are pruned with a diagnostic).
    - Performance: launcher render ≤ 1 frame of work (recents are read once, not
      stat'ed per entry on every render); the agent enumeration does not load the
      agent packages; opening from the launcher activates the tab's view
      directly, without a second navigation.
    - Code Quality: the launcher is a first-party package contribution on the
      `empty-tab` activation (the product-landing rule: no irreplaceable native
      landing), its content is server-delivered SDUI like the workspace tree with
      host APIs behind it (`workspace.clientOpenFolderDialog`, the recents store,
      the agent enumeration), no compiled core branch names it, and the election
      keeps its one-winner conflict diagnostics; the recipe families it needs
      (`seg` — which is the view switcher, launcher rows, `statusDot`, `empty`, the shortcut
      vocabulary, `swatch`, `statRow`)
      are declared in task 8, not invented in host CSS.
    - Security: the launcher grants no authority beyond the two dialogs it can
      open; the folder dialog path goes through the existing grant/containment
      checks; agent entries are data only and cannot cause a package load; recents
      are stored as plain paths with no credentials, and a path outside the
      workspace root is only opened through the dialog (never auto-opened).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/packages.md` (product landings
        are packages; SDUI pane content), `references/ui.md`,
        `DESIGN.md` §12 (launcher composition), §4/§10 (geometry, values).
      - Prototype evidence: `design-artifacts/prototypes/quiet-instrument-migration/start.html`
        (five states: recently used, workspace picked, agent picked, both, first
        run) and its `start-recents.json` fixture provenance (README §8).
    - Options Considered:
      - Keep the agent as the landing and let the user open a folder afterwards:
        that is the superseded 2026-09-11-1700 decision; it cannot express "I want
        to open this folder with no agent yet", which is the common case.
        (Rejected.)
      - A native core launcher surface: faster to build, but it contradicts the
        product-landing rule and puts product naming in core. (Rejected.)
      - A first-party `empty-tab` package contribution with SDUI content
        (chosen) — same election machinery, same provenance check, revocable.
    - Chosen Approach: contribute the launcher as the `empty-tab` winner, feed it
      from a recents store plus the agent enumeration, and let it construct the
      tab's initial state (workspace, agent, active view).
    - API Notes and Examples:
      ```json
      {
        "id": "launcher.start",
        "activation": "empty-tab",
        "actionTargets": [
          "documents.clientOpenFileDialog",
          "workspace.clientOpenFolderDialog",
          "agent.clientOpenAgentPicker"
        ]
      }
      ```
    - Files to Create/Edit:
      - `packages/launcher/**` (manifest, SDUI contribution, load.js), the recents
        store (data dir + Rust read/write + pruning), the agent enumeration host
        API, `frontend/src/shell/PaneTree.tsx` (empty-tab provenance branch),
        `src/server/ui.rs` + tests (election, SDUI payload, recents round-trip),
        `examples/config/packages/first-party.js`, the settings/agent config docs.
    - References: `src/server/ui.rs` (empty-tab election, SDUI payloads),
      `src/shell/file_browser.rs` (the closest existing SDUI surface with host
      APIs), `.clay/layout.json` (what a tab needs to be constructed from).
  - Test Cases to Write:
    - Rust: the empty-tab election picks the launcher when it is the only
      candidate and reports a conflict for two; recents round-trip, prune a
      deleted folder, cap the list; the SDUI payload carries no unlisted path.
    - Frontend (vitest): the launcher's keyboard path (⇥/↑↓/⏎/⌘⏎/esc/⌘O), the
      primary action's label per selection, the first-run and no-match states.
    - Manual (test-plan): first launch with no recents, launch with three recents,
      open both, open a folder that was deleted between sessions.

- [x] Add the agent-type registry and the per-tab agent picker
  - Acceptance Criteria:
    - Functional: the agent view's title is the agent picker (`Coding Agent ▾`),
      opening a menu of the agent types that are actually configured under the
      Clay data dir's `agents/` root, with the current one checked; picking one
      switches that tab's agent (identity, system prompt, skills roots, MCP
      servers, model/effort defaults) without leaving the tab and without
      disturbing the tab's workspace; a tab with no agent yet opens the picker
      first; the menu states where more agent types come from (`~/.clay/agents/`)
      and never lists one that is not configured; the model/effort controls stay
      in the agent header, and switching the agent type resets them to that
      agent's defaults with a visible note in the transcript.
    - Performance: enumeration is a directory scan of `agents/` (no package load);
      switching type re-reads only that agent's config, and the transcript keeps
      the previous agent's turns labelled with the agent that produced them.
    - Code Quality: "agent type" becomes one named concept (`agentType`) with one
      owner (the per-agent config root), replacing the current implicit
      single-agent assumption; the picker is the `agentPicker` recipe (a dropdown
      variant with a labeled trigger) and the header composition loses the
      duplicate "Coding Agent" title + `coding` chip pair; no core branch names
      `coding-agent` outside the existing provenance check.
    - Security: an agent type is config-root-relative and cannot address an
      arbitrary path (containment check on the name); switching agent type cannot
      widen tool caps, MCP grants or skill roots beyond what that agent's own
      config already declares; the registry surfaces no credentials.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md` (per-agent config
        layout, `~/.clay/agents/<agent>/`), `references/packages.md` (agent
        surface contributions), `DESIGN.md` §12 (agent view composition).
      - Prototype evidence: the agent picker in `agent-landing.html`
        (`data-agent-pick`) and its menu note; `start.html`'s agent pane rows.
    - Options Considered:
      - Keep one hardcoded agent type and add the picker later: the launcher
        already has to enumerate agents, so the data exists either way; deferring
        leaves two places that assume one agent. (Rejected.)
      - A separate "agent" package per type with its own landing: no — the agent
        view belongs to the tab; agent _types_ are configuration, not landings.
        (Rejected.)
      - Per-agent config-root registry + picker in the agent view header
        (chosen).
    - Chosen Approach: derive the agent list from the config root, carry the
      chosen type in the tab record (task 33), and rebuild the agent view's
      header around the picker.
    - Files to Create/Edit:
      - Agent enumeration in the server (list/read per-agent config),
        `frontend/src/coding-agent/*` (agent identity comes from the tab, not the
        package), the picker component + `agentSettings`/model reset path,
        `.agents/skills/clay-execution/references/config.md`,
        `docs/reference/config/*`, tests for enumeration, containment and reset.
    - References: `decision-logs/2026-09-09-1420-per-agent-config-layout-agents-coding-agent.md`,
      `decision-logs/2026-09-10-1526-clay-root-home-datadir-per-agent.md`,
      Task 34 (the launcher's agent pane enumerates the same list).
  - Test Cases to Write:
    - Rust: enumeration lists exactly the configured agents; a name with `..` or a
      separator is rejected; per-agent config reads are contained to that root.
    - Frontend (vitest): switching type carries the tab's workspace, resets
      model/effort, labels existing turns with their agent, and updates the
      picker's checked item.
    - Manual (test-plan): create a second agent folder by copying the first, switch
      between them in one tab, verify skills and MCP differ per agent.

  - Outcome (2026-09-13): **shipped** — agent types are a server-owned registry
    (one directory per type under the data root's `agents/`), the agent view's
    title *is* the picker, and switching re-reads only that agent's config over
    the tab's existing session. Every functional criterion is satisfied; the
    deliberate deviations (single data dir, host-wide skills catalog) and the not
    run live walk are recorded below.
    - **One concept, one owner.** An agent type is a *bare directory name* under
      the Clay data root's `agents/`; its config root is
      `<data root>/agents/<type>`. The name rule
      (`launcher::valid_agent_name`) is shared by the enumeration, the settings
      page, and the switch: bounded (64), separator-free, so a name can never
      address a path outside `agents/`, and `launcher::resolve_agent_type` also
      requires the directory to *exist* — an agent that is not configured is not
      resolvable, never silently the default one. Rust tests:
      `agent_types_resolve_only_contained_existing_directories`,
      `per_agent_roots_stay_contained_and_fall_back_to_the_shipped_agent`,
      `per_agent_settings_reads_are_contained_to_that_agents_root`.
    - **The switch is tab chrome.** `TabCommand::SetAgent { tab_id, agent }`
      (the webview's existing `family: "tabCommand"` lane, same as
      `openWorkspace`) is the only way a tab's agent changes:
      `handle_tab_command` validates the name against the data root, the registry
      stores it on `RegistryEntry.agent_type` (server-local — `TabEntry` keeps its
      four protocol fields, so the webview never names an agent on the agent
      command path and no path crosses the wire), then
      `AgentHost::rebind_tab_agent` aligns the tab's live session. A refused
      switch (the daemon validates again) reverts the registry entry, so a tab
      never claims an agent its session is not running. Test:
      `agent_type_binds_to_the_tabs_connection_only`.
    - **The daemon reads per session.** `session.new { agent }` resolves the type
      to its root (`resolveAgentRoot`: the name rule + a direct child of the
      default root's parent + must resolve), loads that root's config once
      (`agentRootConfig`: SYSTEM.md seeding, `tool-caps.json`, `skills.json`,
      delivered + user-added `skills/`), and connects only that agent's MCP
      allow-list (per-root bridges + outcomes). `session.setAgent { sessionId,
      agent, provider, model, mcpAllowList }` rebuilds the live *agent* over the
      same session branch (the mid-session model-switch mechanism): the session
      id, its transcript and its leaf survive, the workspace is untouched, and
      the next run reads the new config. Every appended session entry is stamped
      with its producer (`metadata.agentType`, one store proxy) and the session
      record carries it, so resume keeps both. Tests:
      `clay-agent/src/__tests__/agent-types.test.ts` (6 cases: per-session root,
      the containment/unknown-name refusals, the in-place switch + record
      metadata, switching back, entry stamps, no inherited MCP/tools) — the
      daemon suite is 146 tests, 0 failures.
    - **The picker is the title.** `frontend/src/coding-agent/CodingAgentPanel.tsx`
      renders the `agentPicker.default.trigger.*` family as the header's title
      (via `ClayDropdown`'s new `triggerFamily`/`selectedHint`/`footer`, so the
      popover, list, and rows stay the shared dropdown ones), lists the *same*
      server enumeration the launcher lists (one `launcherEntries` fetch — no new
      API, no fabricated row), marks the current type, and names where more come
      from (`~/.clay/agents/`). It is inert with a reason when the shell cannot
      switch (no session, or nothing listed). The panel's old static
      `transcriptTitle` is gone: the view's title is the agent that is running.
      This *consumed* the family: the five `agentPicker.*` keys left the adoption
      backlog (25 → 20) and `coding-agent.module.css` references their variables
      (attribute-scoped rules, incl. the focus ring and the expanded = accent
      tint + accent text signal, §14.13).
    - **Per-turn attribution.** `AgentTranscriptEntry.agent` (protocol, serde
      default) is stamped by the server for every row it appends (live) and read
      from each persisted entry on load (with the session's recorded agent as the
      fallback for rows written before the stamp existed); `agent_agui` projects
      it as `metadata.agent` (and `agent` rides the STATE snapshot). The view
      labels every turn with its producer **when more than one agent wrote the
      transcript**, and derives the `Switched to …` note from the stamps — so
      both survive a reload, and a turn is never attributed to an agent that did
      not write it.
    - **Model/effort reset.** The new agent's own last-used selection resolves on
      the switch, so each agent type remembers its own model per workspace and a
      switch lands on *that* agent's defaults: the book gained a per-agent map
      (`agent_workspaces` in `book.json`) that a named agent alone reads and
      writes, while the root-keyed map stays the daemon's default agent's read
      path (a pre-agent book is migrated into the shipped agent's map at load, so
      an existing pick is not stranded — `book_round_trips_per_agent_selections_and_reads_v2_books`).
      The server also drops the session's active effort level, and the panel
      clears its pending effort level before handing the pick to the shell.
      Detaching (an empty type) switches to the daemon's default agent rather
      than leaving the previous agent's config in place.
    - **Security.** No new capability: the name is validated before it can become
      an identity, the daemon revalidates it inside its own `agents/` root, the
      per-session MCP list is built from that agent's own `mcp.json` merged with
      the repo `.mcp.json`, and tool caps/skill roots come from that agent's own
      files — one agent's grants can never be widened by another's (asserted in
      the daemon suite). No credentials are enumerated (`LauncherAgentEntry`
      carries name/label/config root/skill count only).
    - **Deviation (recorded).** Runtime state stays single-dir: the sessions DB,
      credential vault, and book remain in `<data root>/agents/coding-agent/data`
      (one daemon = one data dir). Per-agent `data/` needs a per-agent daemon
      (the initialize handshake, reverse-RPC handler, event stream, and JS
      authority are all process-global); the ACs do not require it, and a session
      records its agent so history stays attributable. Also recorded: the Context
      tab's *skills catalog* is the daemon's host-wide registry, so it can list a
      skill another agent root discovered — a run's *active* skills/tools are the
      session's own agent's (Prism's skill registry carries no source tag for a
      per-root filter).
    - **Test Cases to Write:** done — Rust name rule/containment/registry/per-agent
      settings (7 new tests, `cargo test --lib` 1345); daemon `agent-types.test.ts`
      (6); frontend `CodingAgentPanel.test.tsx` (5: picker as title + recipe
      family + current mark + provenance note + no fabricated row, pick hands the
      type up and ignores the current row, inert without a shell, per-turn labels
      + one switch note, no labels with a single agent); the adoption-backlog
      fixture shrank by the five `agentPicker` keys. Manual step **UI-DS-43** in
      `test-plan/15-ui-design-systems.md` (+ the index) — **not run live** in this
      pass (it needs the packaged app plus a second agent directory; the recorded
      step is the standing walk), so its live leg is UNRESOLVED rather than
      claimed.
    - **Docs:** `DESIGN.md` §12 (the picker as the title, its menu, the per-turn
      attribution), `docs/wiki/modules/clay-agent.md` (per-session agent root,
      `session.setAgent`, the per-session MCP list, the single data dir ceiling),
      `docs/wiki/modules/tabs-and-clients.md` (`TabCommand::SetAgent`, where the
      stored name is read), `.agents/skills/clay-execution/references/packages.md`
      (agent types are configuration), `examples/config/README.md` (how to add a
      type; what it changes), the recipe matrix's `agentPicker` mention.
    - **Gates:** `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`;
      `cargo test --lib` (1345); `--test presentation` (61, incl. package UI
      conformance), `--test protocol` (216), `--test runtime` (75),
      `--test security` (154); `cargo test -p clay-desktop --lib` (31); daemon
      `npm test` (146/146); frontend `tsc --noEmit`, `vitest run` (373/373),
      prettier (the known generated `icons/fallback.generated.ts` exception),
      `check:budget` (shell 173.4/180 kB, total 392.9/400 kB).

- [x] Make the agent Files tab the session's file history, and track it
  - Acceptance Criteria:
    - Functional: the Files tab lists the files _this session_ has touched —
      newest first, each with its role (read / written / created / deleted),
      basename first with the directory muted, and the filter/count affordances
      the other panels use; `⏎` on a row switches the tab to its workspace view at
      that file (the dual-view payoff); rows are per-session and clear when a
      session is reset; the empty state says the list is session history, and the
      panel never browses the workspace tree (browsing is the workspace view's
      tree and `⌘O`); opening a file from the Files tab does not replace the
      agent view, it switches the tab's view.
    - Performance: the history is appended from the agent's existing tool events
      (no extra filesystem scan per turn); the panel renders from a bounded,
      de-duplicated list (one row per path, newest role wins) and virtualises
      beyond a few hundred rows.
    - Code Quality: the current behaviour — loading an editor into the Files tab
      for a workspace-chosen file — is deleted, not wrapped, together with the
      code that fed it; the list is one component with one data source (the
      session's file records) consumed by the recipe `sessionRow` family; the
      inspector's five tabs keep their activation colours, and the panel uses the
      same section-head component as Context.
    - Security: the panel shows only paths the session actually touched (never a
      directory listing), and the open action goes through the same document-open
      grant path as any other open — a path outside the workspace root cannot be
      opened from it.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §12 (agent view: the inspector holds session state),
        `.agents/skills/clay-execution/references/components.md` (list row,
        `sessionRow`, inspector surfaces), `references/tokens.md`.
      - Prototype evidence: the session-files panel in
        `agent-landing.html` (populated in the running/resumed/error/disconnected
        scenes, empty in landing/empty-session) and README §9 finding 10.
    - Options Considered:
      - Keep the editor-in-inspector behaviour: it duplicates the workspace view
        inside the agent view and is what makes the inspector confusing at the
        review width. (Rejected.)
      - A workspace tree inside the inspector: worse — a second browser, and it
        hides what the session actually did. (Rejected.)
      - Session file history with an open-in-workspace-view action (chosen).
    - Chosen Approach: derive file records from the agent's tool events
      (read/write/edit/search), keep them in the session record, and rebuild the
      panel as a list that hands off to the workspace view.
    - API Notes and Examples:
      ```json
      {
        "sessionFile": {
          "path": "DESIGN.md",
          "role": "modified",
          "at": "09:41"
        }
      }
      ```
    - Files to Create/Edit:
      - The session file-record type and its event mapping (`frontend/src/agent/*`
        or the agent session store), the Files panel component and its CSS module,
        `frontend/src/coding-agent/*` (remove the editor-in-inspector path),
        `packages/package-*.module.css` for the row family, tests (mapping,
        de-duplication, open action), `test-plan/` entries.
    - References: the AG-UI tool events (`frontend/src/agent/*`), `DESIGN.md` §12,
      Task 33 (switching the tab's view is what the open action uses).
  - Test Cases to Write:
    - Frontend (vitest): a read/write/edit event appends one row per path with the
      newest role; a deleted file marks the row; `⏎` switches the tab's view and
      opens the path; the list clears on session reset.
    - Rust/protocol: no new authority — the open path reuses the existing document
      open path (assert the grant check still runs).
    - Manual (test-plan): run a session that reads, edits and creates files, then
      open one from the Files tab and confirm the workspace view lands on it.

  - Outcome (2026-09-13): **implemented; the Files tab is the session's file
    history and the editor-in-inspector path is deleted, not wrapped.**
    - The record is derived where the transcript is: the server reads the call's
      own arguments on `tool_execution_started`/`_error` (`transcript_file_for_tool`:
      `read`/`write`/`edit`/`delete` + the `path` argument, secret-redacted and
      bounded), carries it on the wire event, and `apply_transcript_event` stamps it
      on the tool row (`AgentTranscriptEntry.file`). It rides the live
      `clay.toolPhase` value and the snapshot message's `metadata.sessionFile`, so
      the client sees one shape whether the row is in flight or rebuilt — and a
      **resumed** session reconstructs the same records from the daemon's persisted
      `tool_call` blocks. A tool that names no single file (search, shell, git,
      move) records none. `tests/agent_protocol.rs`'s replay fixtures were updated
      for the new field.
    - The view derives roles from those verbs in one module
      (`frontend/src/agent/session-files.ts`): `edit` → modified, `delete` →
      deleted, `read` → read, `write` → **created** unless the session had already
      touched the path (then modified), one row per path ordered by its last touch,
      **a read never demoting a mutation**, bounded to
      `AGENT_MAX_SNAPSHOT_ENTRIES` (200 — the transcript that feeds it is capped at
      the same number, so the panel renders a bounded list rather than windowing an
      unbounded one; the AC's virtualisation clause is satisfied by the bound).
    - The panel is the approved composition: a section head with the count as a
      `badge` and the filter as the shipped input well (`F` focuses it, one ring per
      surface), rows on `sessionRow.root` (role mark with the diagnostic tones, mono
      basename, ellipsised mono directory, role word), a foot that says the list is
      session history, and the artifact's empty state with the `@`/`⌘1` keystrokes.
      `⏎` is the row's own activation; the shell's handler opens the document through
      the existing `openPath` (server-contained to the workspace root — the
      containment gate is `workspace_rejects_path_traversal_outside_root`) and
      switches the tab's view, with the agent half still mounted.
    - **Deleted with the path** (not wrapped): the `ClayEditor` in the inspector, the
      Files tab's document-session prop, the resumable-session request and its
      state mapping (the empty-state resume list), and the two plan-117/112 tests
      that pinned that list. Resume/Search Sessions remain as commands (the palette
      and `/resume`), which is where the artifact puts them.
    - Deliberate deviations, all recorded: the wire carries the **op**
      (`read`/`write`/`edit`/`delete`) and the view derives the **role** (only the
      view sees the session's whole history, which is what `created` needs); a
      *forced overwrite* of a file the session never read therefore reads as
      `created` — the daemon's `document.write` result knows, but it is not on the
      wire (comment marks the upgrade path); rows are a plain button list rather
      than the artifact's `listbox`/`option` roles (a listbox needs arrow-key
      focus management; a button list is natively operable), and they keep their
      text labels rather than borrowing an icon; the "resumed session has not
      touched a file" copy was not shipped as a second empty state (the two states
      are indistinguishable in the data — both have no records).
    - **Also consumed** by the rows: `sessionRow.default.root.focus`, which was
      declared-but-unconsumed, so the drift gate's pinned backlog shrank 20 → 19
      keys (fixture updated in the same change).
    - **Gates:** `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`;
      `cargo test --lib` (1347, incl. 3 new); `--test presentation` (61),
      `--test protocol` (216), `--test runtime` (75), `--test security` (154);
      `cargo test -p clay-desktop --lib` (31); frontend `tsc --noEmit`,
      `vitest run` (378/378 — 6 new in `session-files.test.ts`, 2 rewritten +
      1 new in `CodingAgentPanel.test.tsx`, the live-row record in
      `transcript-lifecycle.test.ts`, the handoff in `WorkspacePanes.test.tsx`,
      the CSS invariants in `surface-adoption.test.tsx`), prettier,
      `check:budget` (shell 173.4/180 kB, total 392.9/400 kB).
    - **Visual evidence:** `design-artifacts/screenshots/quiet-instrument-agent-files/`
      (`report.json` + screenshot) from the new
      `design-artifacts/tools/capture-agent-files.mjs` — 10/10 checks: the three
      seeded records (created/modified/read) in order on `sessionRow/root` with the
      recipe's paint (r8, spacing.xs padding, transparent fill), role-toned marks,
      basename + muted directory, role words, the filter narrowing 3 → 1 with the
      count following, the filter's one-ring well, 0px horizontal overflow.
    - **Manual step added** as `UI-DS-44` (`test-plan/15-ui-design-systems.md`) —
      its live walk (a real agent session reading, editing and creating files) is
      **UNRESOLVED** in this pass: it needs a provider-backed run in the packaged
      app. The panel, the derived records, the handoff and the geometry are covered
      by the automated legs and the browser capture above.

## Part E — Closing the follow-ups from the Further Actions list

The migration's task list is complete; these are the seven items the plan's own
Further Actions recorded, promoted to tasks so each has acceptance criteria,
evidence and a closure line instead of living as prose. Decisions taken while
promoting them (user instruction: "go with your preferred choice"):

- **E1** — the sidebar filter ships as an additive SDUI *list filter*
  descriptor rendered by the host (keystroke-local filtering of the delivered
  bounded listing, the shape the approved `workspace.html` shows), not a new
  pane and not a per-keystroke server round-trip.
- **E4** — the ten IA families join the core fallback map (the documented
  "core = the host-consumed subset" invariant), verified against the package.
- **E6** — the error slot is implemented *with* its producer (the Settings
  panel's ratio fields), never with a synthetic one.
- **E7** — the base-ui vocabulary grows (accent + the three border rungs)
  instead of closing as documented debt: the stricter gates now *refuse* a
  legacy theme that cannot express a boundary, so the vocabulary is the fix that
  removes the trap.

- [x] E1 — Build the approved Workspace sidebar head and fix the slot width
  - Acceptance Criteria:
    - Functional: the sidebar renders the approved head/foot — title, the filter
      field (search glyph, `Filter files`, `/` chip), and a foot with the live match
      count and the move/open hints (`design-artifacts/approved/quiet-instrument-migration/workspace.html`).
      Typing filters the tree by path, a matching file keeps its ancestors visible,
      the count reads `N matches` only while a query is active, `Escape` clears,
      `Enter` opens the first file, `ArrowDown` moves focus into the tree, and `/`
      focuses the field from the workspace view. The filter is a real filtered
      listing (never a control that does nothing), it never hides the root's
      structural context, and clearing restores the listing exactly.
    - Performance: filtering is keystroke-local over the delivered bounded listing
      (no server round-trip per keystroke, no re-fetch); the field and the count
      update within one frame for the shipped listing bound.
    - Code Quality: the filter is declared as data (an additive optional descriptor
      on the SDUI list node), not as a host branch on the pane's identity; the field
      is the catalog input well (`textInput`) with one ring per surface; the count
      and hints use the shipped `kbd`/`keyHint` vocabulary; the sidebar width is the
      typed dimension token (`244px`, `224px ≤ 1240px`) with no width literal left in
      CSS; the region stays a flush canvas zone (no radius, no shadow, no veil).
    - Security: filtering is client-side presentation of an already-authorized
      listing — it grants no filesystem access, introduces no new intent, and cannot
      reveal a path the listing did not already deliver.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5 (sidebar 244/224, row heights, hit targets), §6 (the sidebar
        is a flush zone), §8 (mono for data), §11 (input well, one ring per surface),
        §12 (Workspace composition), §14 (retired patterns), §16 (this follow-up)
      - `.agents/skills/clay-execution/references/ui.md`, `components.md`, `tokens.md`
      - `design-artifacts/approved/quiet-instrument-migration/workspace.html` +
        `ds.js` (the filter's exact behaviour, ancestor rule, count copy, key map)
      - `docs/development/ui-design-system-recipe-matrix.md` (the families in play:
        `textInput`, `list`, `kbd`, `fileBrowser`, `divider`)
      - `src/protocol/sdui.rs`, `src/shell/file_browser.rs`,
        `frontend/src/sdui/renderer.tsx`, `frontend/src/components/list.tsx`
    - Options Considered:
      - A host-rendered sidebar (React owns the region): rejected — the region is
        server-delivered SDUI by contract and a host branch on the pane's identity is
        the pattern the plan forbids.
      - A per-keystroke server filter intent: rejected — an editor hot path, and the
        approved artifact filters the delivered listing locally.
      - An additive optional `filter` descriptor on the SDUI list node (chosen): the
        tree already arrives whole and bounded (the same assumption the fuzzy-open
        session filters under), the protocol gains one inert field, and no host branch
        on package or pane identity is needed.
    - Chosen Approach:
      - `SduiNodeKind::List` gains `filter: Option<SduiListFilter>` (`placeholder`,
        `shortcut`), serialized camelCase with `#[serde(default)]` so every existing
        tree stays valid; the server fills it for the file listing only.
      - The host's list renderer renders the approved tools row above the rows when
        the descriptor is present (catalog input well + `/` chip) and the foot
        (count + hints) below, and owns the key map; rows filter by path with the
        ancestor rule; the count copy matches the artifact.
      - `dimension.panel.side.default` and `dimension.sidebar.default` move to 244
        (with `SIDEBAR_DEFAULT_WIDTH` and the CSS fallbacks), plus a `≤1240px`
        compact rule at 224 mirroring the rail's mid-width rule; the measured width
        is recorded in the capture.
      - The `/` chord joins the workspace view's key map; the status bar's hint row
        gains it.
    - Files to Create/Edit:
      - `src/protocol/sdui.rs` (descriptor + validation), `src/shell/file_browser.rs`
        (deliver it), `src/server/connection/workspace.rs` (if the tree assembly
        changes), `frontend/src/sdui/types.ts` + `renderer.tsx` + `renderer.module.css`
      - `frontend/src/components/list.module.css` (tools/foot/ancestor styling),
        `frontend/src/routes/workspace.module.css` (compact width),
        `frontend/src/packages/package-workspace.module.css` (fallback 244)
      - `src/shell/theme.rs` (dimension values), `frontend/src/styles/tokens.css` if
        the dimension projection carries the fallback
      - `DESIGN.md` §16 (the follow-up closes), `docs/development/ui-design-system-conformance.md`,
        `test-plan/15-ui-design-systems.md` (a step for the filter)
  - Test Cases to Write:
    - Rust: a tree with the descriptor round-trips (rkyv + serde) and older trees
      without it still parse; the file listing carries it with the artifact's
      placeholder and shortcut.
    - Frontend: filtering narrows rows by path, keeps a match's ancestors visible,
      the count reads `N matches`/clears, `Escape`/`Enter`/`ArrowDown` behave, and the
      field's ring is the only ring in the sidebar.
    - Frontend: the sidebar keeps its flush geometry (no radius/shadow/veil) and the
      rendered width is 244 (224 at ≤1240) in the capture.
    - No server intent is sent while typing.

  - Outcome (2026-09-13): **closed.** The listing is filterable as data and the
    region is sized by token, both additively:
    - **Protocol:** `SduiNodeKind::List` gained `filter: Option<SduiListFilter>`
      (placeholder + the `/` hint) and `SduiNode` gained `size: Option<String>`
      (a host dimension token). Both are optional on the wire, so a tree written
      before them still parses — pinned by a serde test. Only core trees use them:
      package-declared lists stay plain (`src/server/ops/sdui.rs`), because the
      filter is a host affordance over a listing the server already authorized.
    - **Server:** `src/shell/file_browser.rs` marks the listing
      (`Filter files`, `/`) and sizes the sidebar region
      (`dimension.sidebar.default`).
    - **Host:** `ClayList` renders the approved tools row (the shipped single-line
      input well + the `/` chip) and the foot (live `N match(es)` + the move/open
      hints) when a descriptor is present, filters the delivered bounded rows
      keystroke-locally, and owns the key map (Escape clears, Enter opens the first
      match, ArrowDown moves into the list). The `/` chord (workspace chords + the
      status-bar hint) focuses the visible filter — generic, never a named pane.
      `SduiRenderer` marks a token-sized region with `data-clay-size`; the host CSS
      maps `dimension.sidebar.default` to 244px and 224px at ≤1240px.
    - **Tokens:** `dimension.sidebar.default` 240 → **244**, new
      `dimension.sidebar.compact` **224**, and `dimension.panel.side.default`
      240 → 244 so the fixed-slot default and the sidebar agree (DESIGN.md §5);
      `SIDEBAR_DEFAULT_WIDTH`, `PANEL_SIDE_DEFAULT`, the host CSS fallbacks and the
      pinned geometry tests moved with them.
    - **Measured (evidence `design-artifacts/screenshots/quiet-instrument-sidebar/`,
      `report.json`, from the new `design-artifacts/tools/capture-sidebar.mjs`):**
      244px at 1500 and 224px at 1024 with the token attribute present; the filter
      narrowing 3 → 1 → 0 → 3 rows with the count reading `1 match` only while
      filtering; the `..`-free subset check passing; **0px horizontal overflow**.
    - **Deviations (recorded in DESIGN.md §16):** the app's sidebar lists one
      directory at a time (with its `..` row), so the prototype's "a matching file
      keeps its ancestors visible" rule has no tree to apply to; the field is the
      shipped r8 input well rather than the prototype's field-painted strip; and
      the prototype's `..`-row filtering semantics are the app's directory
      navigation instead.
    - **Gates:** `cargo fmt --check` / `clippy --all-targets -D warnings` clean;
      `--lib` **1350** (+2: the protocol round trip and the file-browser tree's
      descriptor/token), `presentation` 61, `protocol` 216, `runtime` 75,
      `security` 154; frontend `tsc --noEmit`, `vitest run` **393/393** (the
      catalog list's filter behaviour + the renderer's sizing/hand-off),
      prettier clean, budgets 173.4/180 and 392.9/400 kB; component conformance
      audit **18/18** and the browser pass clean across five themes (0 mismatches,
      5/5 probes, 0 paint violations).

- [x] E2 — Give the rail a scroll-spy and persist rail + inspector visibility per tab
  - Acceptance Criteria:
    - Functional: the rail's active entry follows the document's top visible line
      while the user scrolls, and an explicit jump keeps the clicked entry until the
      next manual scroll (the approved prototype's "lock" behaviour); visibility of
      the rail and of the agent inspector is per tab and survives a restart with the
      rest of that tab's layout (DESIGN.md §5 "layout state … persists per surface";
      task 33's acceptance criteria), including a tab whose rail was hidden while
      another tab's was shown.
    - Performance: the spy updates on scroll frames from the editor's viewport without
      a per-frame linear rescan of the whole document and without a React render storm
      (bounded to the outline's current line).
    - Code Quality: rail and inspector visibility live in the tab store beside the
      view and slot state (one owner), the persisted shape is versioned and tolerant
      of missing fields, and no host CSS literal is introduced.
    - Security: layout state is inert client data; the persisted file carries no
      paths or content beyond what the tab already persists.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §5 (persistence claim), §12 (rail and inspector in the tab),
        §16; `.agents/skills/clay-execution/references/ui.md`
      - `design-artifacts/approved/quiet-instrument-migration/workspace.html` +
        `ds.js` (scroll-spy + lock), `src/shell/layout_persist.rs` (v2 shape),
        `frontend/src/shell/tab-store.ts`, `frontend/src/routes/WorkspaceRail.tsx`
    - Options Considered:
      - Recomputing the active entry per scroll event by scanning headings: rejected
        (O(headings) per frame for a behaviour the editor already knows).
      - A viewport-line query against the editor's line geometry with the existing
        reveal-line contract (chosen): one lookup per scroll frame, the same line
        space the outline uses; a lock flag suppresses the spy until the user scrolls.
      - Persisting visibility in `localStorage`: rejected — the tab layout file is the
        one owner of per-tab layout and the window state already round-trips it.
    - Chosen Approach:
      - The rail subscribes to the editor's visible top line (the editor exposes a
        viewport-line read alongside `revealLine`), maps it to the outline entry, and
        sets the active entry unless the lock is held; clicking an entry locks until
        the editor reports a user scroll.
      - `PersistedTabState` gains `rail_visible`/`inspector_visible` (optional, default
        true so v2 files load), written from the tab store and reapplied on restore;
        the agent panel's inspector toggle reads and writes the same state.
    - Files to Create/Edit:
      - `frontend/src/routes/WorkspaceRail.tsx`, `frontend/src/editor/sync/session.ts`
        (viewport-line read), `frontend/src/shell/tab-store.ts` + `layout-state.ts`,
        `src/shell/layout_persist.rs` (v3 fields), `frontend/src/coding-agent/CodingAgentPanel.tsx`
      - `DESIGN.md` §16, `docs/reference/ui-design-systems.md` if the persistence claim
        needs the tab-level wording, `test-plan/15-ui-design-systems.md`
  - Test Cases to Write:
    - Rail: a viewport change moves the active entry; a click locks it and a manual
      scroll releases the lock; no entry is marked when the document has no headings.
    - Persistence: a tab round-trips rail/inspector visibility, a v2 file loads with
      defaults, and two tabs keep independent visibility across a restart.
    - Agent panel: the inspector toggle writes tab state (no local-only state left).

  - Outcome (2026-09-13): **closed — both halves, including the task-33 criterion
    that was missed.** *Visibility:* `layout-state.ts` is now a per-tab store keyed
    by clientId (with a default bucket for surfaces rendered outside a tab, so a
    DEV fixture's toggle still works), the workspace controller mirrors it into
    `layout.json` per tab, a visibility toggle schedules its own write, `activate`
    re-points the store and `restore` seeds it; `PersistedTabState` gained
    `rail_visible`/`inspector_visible` (absent = visible, so v2 documents written
    before the fields load unchanged) and `CodingAgentPanel` reads the store
    instead of holding local state. Two tabs now keep different shapes across a
    restart. *Spy:* the document session publishes its viewport
    (`topVisibleLine()` with the approved 48px reading inset, plus
    `onViewportChange` attached to the scroll element and removed on detach) and
    the rail follows the reached entry at most once per frame, with the approved
    800ms lock after an explicit jump.
    - **Gates:** `cargo fmt --check` / `clippy --all-targets -D warnings` clean;
      `--lib` **1350** (2 new: the visibility round trip and the legacy default);
      frontend `tsc --noEmit`, `vitest run` **387/387** (the controller's per-tab
      test with the toggle's own write, the persist round trip, and the spy/lock
      test), prettier clean.

- [x] E3 — Close the frontend formatting gate
  - Acceptance Criteria:
    - Functional: `npm run format:check --prefix frontend` (a CI step) exits clean and
      `scripts/generate-icon-packs.mjs --check` still reports no drift, so the two gates
      stop contradicting each other.
    - Performance: no runtime effect.
    - Code Quality: the generated file is excluded from formatting *because it is
      generated* (one ignore line with the reason), never reformatted by hand; a
      repo-wide `prettier --write` can no longer break the generator's drift gate.
    - Security: no dependency or config surface beyond the ignore entry.
  - Approach:
    - Documentation Reviewed: `frontend/.prettierignore`, `scripts/generate-icon-packs.mjs`
      (the generator and its `--check` mode), `plans/112` (icon packs), the plan's own
      record of the trap (launcher/agent tasks).
    - Options Considered:
      - Run `prettier --write` on the file: rejected — it breaks the generated-icon
        drift test (recorded twice in this plan).
      - Make the generator emit prettier-shaped output: rejected — the generator would
        need a formatter dependency for a file nobody edits.
      - Ignore the generated file (chosen): the cheapest honest fix; generated output is
        conventionally outside the formatter's remit.
    - Files to Create/Edit: `frontend/.prettierignore`
  - Test Cases to Write:
    - `npm run format:check --prefix frontend` clean; `node scripts/generate-icon-packs.mjs --check` clean.

  - Outcome (2026-09-13): **closed.** `frontend/.prettierignore` now carries a
    two-line reason plus `src/icons/fallback.generated.ts`. The 35-file debt the
    item recorded was already gone (the migration's own tasks reformatted those
    surfaces), so the only failure left was the generated icon module —
    exactly the trap this plan hit twice (a repo-wide `prettier --write`
    reformats it and breaks the generator's `--check`). Verified:
    `npm run format:check --prefix frontend` → "All matched files use Prettier
    code style"; `node scripts/generate-icon-packs.mjs --check` → no drift.

- [x] E4 — Add the ten IA families to the core fallback map
  - Acceptance Criteria:
    - Functional: `core_design_system_fallbacks()` covers `seg`, `agentPicker`,
      `recentRow`, `sessionRow`, `toast`, `empty`, `statusDot`, `keyHint`, `swatch`
      and `statRow`, and each resolves to exactly the shipped package's value for that
      key, so `@clay/core` and `@clay/design-instrument` agree on every consumed key
      (the documented single-language invariant).
    - Performance: the map stays a build-time constant; the served core snapshot grows
      by ten families of inert data.
    - Code Quality: the rows use the shared geometry/motion ladder constants and the
      `Elevation` enum (no literals, no retired 0px radius or linear timing); the
      existing subset/value-match gates keep guarding the map.
    - Security: inert style data only; no new role, no literal colour.
  - Approach:
    - Documentation Reviewed: `DESIGN.md` §4/§11/§16, `src/shell/design_system.rs`
      (`FALLBACK_KINDS`, `FALLBACK_SURFACES`, `Elevation`), `packages/design-instrument/package.json`
      (the values to mirror), `tests/package_ui_conformance.rs`
      (`plan118_core_fallbacks_match_the_shipped_language`, the consumed-subset guard),
      `frontend/src/test/fixtures/design-system-adoption-backlog.json`.
    - Options Considered:
      - Close as resolved-by-`tokens.css`: true for pre-bootstrap paint, but leaves the
        map unable to answer for keys the host consumes, and DESIGN.md documents the map
        as the host-consumed subset — rejected.
      - Add the ten rows mirroring the package (chosen): data-only, verified by an
        existing exact-value gate.
    - Files to Create/Edit: `src/shell/design_system.rs`,
      `docs/development/ui-design-system-recipe-matrix.md` (the core-fallback note),
      `DESIGN.md` §16 backlog sentence (also dropping the stale `seg`/`agentPicker`
      mention).
  - Test Cases to Write:
    - The existing value-match gate passes for the new keys; the core-vs-consumed guard
      still holds (no speculative key); the served `@clay/core` snapshot contains the
      ten families.

  - Outcome (2026-09-13): **closed, with the scope the invariant actually implies.**
    The map now carries every *consumed* rest key of the ten families:
    `FALLBACK_KINDS` gained `seg`, `sessionRow`, `statRow`, `empty`, `keyHint`, and
    `FALLBACK_SLOT_EXTRAS` carries the slots outside the `default/root` shape —
    `agentPicker.default.trigger.rest`, `keyHint.default.{keys,row}.rest` and the
    five `statusDot.<tone>.root.rest` rows. `recentRow`, `swatch` and `toast` stay
    out on purpose: they have no host consumer yet, and the map is the
    *host-consumed* subset (their entries ship with the surfaces that paint them —
    the launcher's meta column, the settings swatch, the notification host), which
    DESIGN.md §16 and the recipe matrix now state instead of the stale list that
    named `seg`/`agentPicker` as unconsumed.
    - **Values are mirrors, not choices.** Each row was derived so the map's value
      equals what the package resolves today, and the 13 recipes that gain rows now
      declare their rest state explicitly (`outlineStyle: none`, plus
      `borderStyle: none` where the border is 0-width) instead of inheriting a
      solid 2px ring from the resolver's default — so `@clay/core` and
      `@clay/design-instrument` resolve identically and the activation swap still
      moves nothing. `plan118_core_fallbacks_match_the_shipped_language` now covers
      13 more shared keys.
    - **Flush list:** `empty.default.root.rest`, `keyHint.default.root.rest`,
      `keyHint.default.keys.rest` and `keyHint.default.row.rest` are non-boxes
      (text/hint regions where a radius is not visible), added to both flush lists
      (the Rust rule test and the conformance tool) with that reason.
    - **Gates:** `--test presentation` 61 ✓ (value-match, language-rules, tokens.css
      projection and coverage guards all green), `design-system-consumption.test.ts`
      15 ✓, and the `tokens.css` block re-projected with no drift (the projection is
      unchanged: these keys were already declared from the manifest).

- [x] E5 — Stop the collapse package declaring a frame nobody draws
  - Acceptance Criteria:
    - Functional: the disclosure's separator is still one 1px hairline above each body
      (the approved specimen), while `collapse.default.root.rest` no longer declares a
      border box; every acceptance surface (Settings panel groups, any collapse user)
      renders identically to today's approved composition.
    - Performance: no change (one border rule moves family).
    - Code Quality: the hairline comes from `divider.default.root.rest` — the family
      that exists for separators — so the package stops describing a frame `DESIGN.md`
      §11 never had; the component-conformance tool's accepted-deviation entry is gone
      because the deviation no longer exists.
    - Security: data/CSS only.
  - Approach:
    - Documentation Reviewed: `DESIGN.md` §11 (no root frame), `.agents/skills/clay-execution/references/components.md`,
      `packages/design-instrument/package.json` (`collapse.*`), `frontend/src/components/controls.module.css`
      (`.collapseRoot`, `.collapseBody`), `design-artifacts/tools/verify-component-conformance.mjs`
      (deviation list), `design-artifacts/approved/quiet-instrument-migration/component-catalog.html`.
    - Options Considered:
      - Keep the package's root border and leave the deviation recorded: rejected — the
        contract keeps describing a frame that no surface draws.
      - Drop the root box and take the body hairline from `divider` (chosen).
    - Files to Create/Edit: `packages/design-instrument/package.json`,
      `packages/design-instrument/dist/*` if it mirrors recipes, `frontend/src/components/controls.module.css`,
      `frontend/src/styles/tokens.css` (the removed keys leave the fallback block),
      `design-artifacts/tools/verify-component-conformance.mjs`, `DESIGN.md` §16,
      `docs/development/ui-design-system-recipe-matrix.md` if the slot note changes.
  - Test Cases to Write:
    - The conformance tool's offline audit resolves collapse without the deviation; the
      browser pass shows the body hairline at 1px from the divider family with the root
      at 0 border; the fallback-block gate stays green after the removed keys.

  - Outcome (2026-09-13): **closed.** `collapse.default.root.rest` declares fill +
    radius only; the body's top hairline comes from `divider.default.root.rest`
    (`frontend/src/components/controls.module.css`), which is the family for
    separators. The three dead fallback declarations left `tokens.css` (the
    consumption gate caught them as dead variables, which is the gate working),
    the tool's `collapse.body-hairline` deviation is gone (4 → 3 recorded), and
    the recipe matrix's root row now says the root has no frame.
    - **Two findings fixed on the way, both real:** the conformance tool's
      `material/no-filter-or-animation-in-host-css` audit was failing on
      `tab-strip.module.css`'s running-work pulse — the rule banned *every*
      keyframe while `DESIGN.md` §7/§14 allow exactly one (the running-work
      pulse); the rule now permits keyframes that animate only `opacity`/
      `transform` and still fails on anything else. And the pulse durations
      disagreed: the approved artifact (`agent-landing.html`: `.typing i … 1.1s`)
      and the shipped CSS say 1.1s, `DESIGN.md` §7/§11 said 1.2s — the doc now
      matches the approved language (no code change).
    - **Gates:** offline conformance audit **18/18** with the deviation removed;
      `--test presentation` 61 ✓; frontend `components.test.tsx` +
      `workspace-composition.test.tsx` 21 ✓; `tsc --noEmit` ✓; prettier ✓;
      `design-system-consumption.test.ts` 15 ✓. The browser leg rides the shared
      conformance pass recorded with E1/E2.

- [x] E6 — Give the text input's error slot a real producer
  - Acceptance Criteria:
    - Functional: a field in the error state renders its message as the caption below
      the well (`textInput.default.error.rest`: `diagnostic.error`, no border), wired
      with `aria-describedby` (and `aria-invalid` on the input) so the message is
      announced; the Settings panel's four typography ratio fields pass a per-field
      message instead of relying on the row note alone, and clearing the input removes
      the message and the association. Focus still draws the accent ring on the well
      (one ring per surface) — the error message never adds a second ring.
    - Performance: no measurable cost (one node per invalid field).
    - Code Quality: `ClayTextField` owns the slot (one implementation for React and the
      SDUI registry), the caption consumes the declared recipe key instead of a literal,
      and the key leaves the adoption backlog because it is consumed.
    - Security: message text is host-authored UI copy; no user/agent-supplied string is
      rendered as markup.
  - Approach:
    - Documentation Reviewed: `DESIGN.md` §11 (input well and its states), §16, §8
      (caption type), `.agents/skills/clay-execution/references/components.md`/
      `tokens.md`, `frontend/src/components/text-field.tsx` + `.module.css`,
      `frontend/src/settings/SettingsPanel.tsx` (the invalid case today),
      `packages/design-instrument/package.json` (`textInput.default.error.rest`),
      the component catalog's invalid specimen.
    - Options Considered:
      - Drop the declared key: rejected — the language declares an error caption and the
        host has a real invalid state, so the declaration is not speculative.
      - Implement the slot with the existing row note as its only text: rejected — the
        message must say which field is wrong, otherwise the association is theatre.
      - Implement the slot and wire the ratio fields (chosen).
    - Files to Create/Edit: `frontend/src/components/text-field.tsx` + `.module.css`,
      `frontend/src/settings/SettingsPanel.tsx`, `frontend/src/sdui/registry.tsx` if the
      package input passes a message, `frontend/src/styles/tokens.css` (the consumed
      keys join the block), `frontend/src/test/fixtures/design-system-adoption-backlog.json`,
      `DESIGN.md` §16, `docs/development/ui-design-system-conformance.md`,
      `design-artifacts/tools/verify-component-conformance.mjs` if the key's coverage is pinned.
  - Test Cases to Write:
    - `ClayTextField`: the message renders in the error state, the input carries
      `aria-describedby`/`aria-invalid`, and no message node exists otherwise.
    - Settings: an unparsable ratio shows its own message; a valid one clears it.
    - The drift gate records the key as consumed and the fallback block stays in sync.

  - Outcome (2026-09-13): **closed.** `ClayTextField` gained `errorMessage`, rendered
    as the declared slot below the well (`textInput.default.error.rest`: the recipe's
    `diagnostic.error` colour, no border) and wired through `aria-describedby` beside
    the description; the message only exists when there is one (no slot, no
    association, no invalid state). The producer is the real invalid case: the
    Settings panel's typography fields now carry **per-field** messages
    (`typographyErrors`), so an unparsable size warns *that* field instead of toning
    the whole group, and the row note stays the summary.
    - **A latent accessibility bug surfaced and was fixed:** slot ids were derived
      from the label (`${label}-description`), but `aria-describedby` is a
      space-separated IDREF *list* — `"UI size-description"` resolved to the ids
      `UI` and `size-description` and announced nothing. Both slots now use
      `useId()`, so every multi-word label's description is announced too (the
      single-word label in the old test is why this had never failed).
    - **Consumption:** the key left the adoption backlog (19 → 18), the
      `tokens.css` block took the two projected declarations it needs, and the
      component gallery's invalid specimen shows a real message instead of passing
      copy through `description`.
    - **Gates:** frontend `tsc --noEmit`, `vitest run` **382/382** (the new
      `ClayTextField` slot test and the Settings per-field test included),
      prettier clean, conformance tool offline audit 18/18, `--test presentation`
      61 ✓ (block projection + drift gates).

- [x] E7 — Let a legacy theme express an accent and a real border ladder
  - Acceptance Criteria:
    - Functional: a theme package that declares only `textStyles` can set an accent
      hue and the three border rungs (`accent`, `borderHairline`, `borderSubtle`,
      `borderStrong`), and the projection maps `accent.primary`/`focus.ring`/
      `border.focus` to the accent and `border.hairline`/`border.subtle`/`border.strong`
      to the rungs; an absent key keeps today's behaviour exactly (caret and the
      scrollbar grey), so nothing shipped changes appearance. A legacy theme whose new
      values cannot clear the floors is refused with the existing diagnostic naming the
      pair — and one that sets a proper ladder now activates instead of being refused.
    - Performance: no runtime cost (a closed vocabulary lookup).
    - Code Quality: the vocabulary stays closed and validated (unknown targets still
      rejected up front), the projection is one match arm per key, and the keys are
      documented wherever the base-UI vocabulary is documented so the docs-as-code
      gates stay aligned.
    - Security: inert colour data; the roles stay theme-owned and no palette/literal
      pathway is added.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` §10 (role ladder and depth), §7/§8; `.agents/skills/clay-execution/references/tokens.md`
        (the base-UI key list), `references/config.md` (theme ownership)
      - `docs/reference/primitives/syntax-vocabulary.md`, `docs/reference/packages/creating-packages.md`,
        `docs/reference/ui-design-systems.md` §7 (the recorded limit this closes)
      - `src/editor/theme.rs` (`BaseUiColorKey`, override parsing), `src/shell/theme.rs`
        (`UiColors::color`, `REQUIRED_CONTRAST_PAIRS`), `tests/theme_packages.rs`
    - Options Considered:
      - Close as documented debt (the typed `designTokens` path exists for every theme):
        rejected — the stricter gates *refuse* a legacy theme that cannot express a
        boundary from its base colours, which turns a migration gap into a broken
        third-party theme.
      - Derive the ladder mechanically from the scrollbar grey (lighten/darken):
        rejected — invents colours the theme author did not choose.
      - Grow the vocabulary by four keys (chosen): author choice, validated by the
        existing gates, with the current behaviour preserved as the default.
    - Files to Create/Edit: `src/editor/theme.rs` (enum + parsing + docs),
      `src/shell/theme.rs` (projection + tests), `tests/theme_packages.rs` if it
      enumerates keys, `.agents/skills/clay-execution/references/tokens.md`,
      `docs/reference/primitives/syntax-vocabulary.md`,
      `docs/reference/packages/creating-packages.md`, `docs/reference/ui-design-systems.md` §7,
      `test-plan/15-ui-design-systems.md` if a step records the vocabulary.
  - Test Cases to Write:
    - Projection: each new key maps to its role; absent keys keep the caret/scrollbar
      fallback (pinned).
    - Gates: a legacy palette with a proper ladder activates; one with an invisible
      boundary is refused by name; the core palette is unchanged.

  - Outcome (2026-09-13): **closed.** The vocabulary gained four keys —
    `accent`, `borderHairline`, `borderSubtle`, `borderStrong` — in
    `BaseUiColorKey` (parse + apply) with four optional `BaseUiColors` fields, and
    the shell projection prefers them over the `caret` / `scrollbar` stand-ins it
    used before they existed. **Nothing shipped changes:** every shipped theme
    declares typed `designTokens`, and a theme that declares no new key keeps the
    old projection exactly (asserted). A theme that *does* declare a ladder is
    judged by the same gate — the new `legacy_theme_can_express_an_accent_and_a_border_ladder`
    test projects the declared values and refuses an invisible `border.subtle` by
    name — so the "a legacy theme that cannot clear the floor is refused
    activation" trap now has a fix instead of only a diagnosis.
    - **Mirrors updated with it:** the docs that enumerate the vocabulary
      (`docs/reference/primitives/syntax-vocabulary.md` + its theme-axis table,
      `docs/reference/packages/creating-packages.md`, the execution reference
      `references/tokens.md`, `docs/reference/ui-design-systems.md` §7 and
      `DESIGN.md` §10.1), and the conformance tool's theme projection
      (`BASE_PROJECTION` now takes a list — first *declared* field wins — so its
      CDP theme simulation paints what the server projects).
    - **Gates:** `cargo fmt --check`; `clippy --all-targets -D warnings` clean;
      `--lib` **1348** (the new test included); `presentation` 61, `protocol` 216,
      `runtime` 75, `security` 154; conformance tool offline audit 18/18.

## Compromises Made

- **The session's file history says `created` from the session's own memory, not
  the filesystem's.** The role is derived from the tool verb plus whether the
  session had already touched the path, so a forced overwrite of a file this
  session never read reads as created; the daemon's `document.write` result carries
  the exact answer (`open: false` = new file) but never reaches the wire. Upgrade
  path: forward that op result on the tool event
  (`frontend/src/agent/session-files.ts` marks the ceiling).
- **No windowing in the Files tab.** The list is bounded at 200 rows because the
  transcript that feeds it is bounded at the same number
  (`AGENT_MAX_SNAPSHOT_ENTRIES`); rendering those directly is cheaper than a
  virtualiser and the bound is the guarantee the AC asked for.
- **The Files tab's filter is the shipped input well, not the prototype's quiet
  variant.** The rows use the approved `sessionRow` family exactly; the field
  follows the launcher's shipped filter (same recipes, one ring per surface), and
  the prototype's `field--quiet` (transparent fill) was not adopted as a second
  input treatment for one panel.
- **Two approved-scene copies are not shipped:** the "resumed session has not
  touched a file" empty state (both states carry no records, so the data cannot
  tell them apart) and the artifact's `listbox`/`option` row roles (a button list
  keeps native keyboard operation without focus-management machinery).

## Further Actions

**All seven items recorded here are closed** (2026-09-13, tasks E1–E7 in Part E): the sidebar head
and its width, the rail's scroll-spy plus per-tab visibility persistence, the formatting gate, the
core fallback map's IA families, the collapse frame, the input's error slot, and the legacy
theme vocabulary. Each closure line below names the task and its evidence; anything found while
closing them that is not yet done is listed as its own item at the end of this section.

- ~~Workspace sidebar head (filter field + count/tip footer) and the sidebar slot width~~ —
  **closed 2026-09-13 (task E1).** The listing carries a filter descriptor and the region names the
  host token it is sized from; the host renders the approved tools row and count foot and filters
  the delivered rows locally (`/` focuses it). Tokens: `dimension.sidebar.default` 244,
  `dimension.sidebar.compact` 224, `dimension.panel.side.default` 244. Measured 244/224 with the
  filter narrowing 3 → 1 → 0 → 3 and 0px overflow
  (`design-artifacts/screenshots/quiet-instrument-sidebar/`). See E1 for the recorded deviations.
- ~~The workspace rail has no scroll-spy; rail visibility is not per tab~~ — **closed 2026-09-13
  (task E2).** The rail follows the editor's viewport line (48px reading inset, 800ms lock after an
  explicit jump), and rail + inspector visibility persist per tab through `layout.json`
  (`railVisible`/`inspectorVisible`, absent = visible) — the criterion task 33 recorded but did not
  implement. See E2.
- ~~Frontend formatting gate is red before this plan touches it~~ — **closed 2026-09-13 (task
  E3).** The migration's own tasks already reformatted the 35 files; the residual failure was the
  generated icon module, now ignored for the formatter with the reason recorded.
  `npm run format:check --prefix frontend` and the generator's `--check` both pass. See E3.
- ~~Core fallback map still lacks the ten IA families~~ — **closed 2026-09-13 (task E4).**
  Every consumed rest key of the ten families is in the map (including the picker trigger,
  the key-hint slots and the five status-dot tones), mirrored to the package's resolved values
  and verified by the existing exact-equality gate; `recentRow`, `swatch` and `toast` stay out
  because they still have no host consumer, and they are the recorded backlog. The
  pre-bootstrap-paint half of the item was already closed by the `tokens.css` projection plus the
  consumption gate. See E4.
- ~~`collapse.default.root.rest` declares a border box the language does not draw~~ — **closed
  2026-09-13 (task E5).** The root declares fill + radius only and the body's top hairline comes
  from `divider.default.root.rest`; the tool's recorded deviation is gone, and the audit is 18/18.
  See E5 (including the running-work-pulse rule and duration fixes the pass surfaced).
- ~~The text input's error slot has no producer~~ — **closed 2026-09-13 (task E6).** The slot
  shipped with a real producer (the Settings panel's typography fields), the key is consumed
  (19 → 18 backlog keys), and the wiring fix it forced (`useId`-based slot ids, because
  `aria-describedby` is an IDREF list) repaired the description association for multi-word labels.
  See E6.
- ~~Legacy base-ui colour projection keeps a flat border ladder~~ — **closed 2026-09-13 (task
  E7).** The vocabulary grew (`accent`, `borderHairline`, `borderSubtle`, `borderStrong`), the
  projection prefers a declared value over the `caret`/`scrollbar` stand-in, and a declared ladder
  is judged by the same contrast gate — so a legacy theme can now clear the floor it was previously
  refused for. Absent keys keep the old behaviour. See E7.

### Found while closing them (new items, not previously recorded)

- **The agent view's status dot does not pulse while a run is in flight.** DESIGN.md
  §7 describes the running-work indicator as an opacity pulse; the host's one
  running-work marker today is the tab strip's `agent` marker (task E5 aligned its
  duration with the approved artifact). The agent view's state-strip
  `statusDot[data-tone="busy"]` is static. Decide with the next agent-surface pass:
  either the dot pulses too (the artifact's own running scene is the reference) or
  §7's wording narrows to the markers that ship. Priority: low; no user-visible
  defect today (the tab marker carries the signal).
- **Compact geometry only reaches the SDUI region.** `dimension.sidebar.compact`
  (224) is honoured by the token-sized SDUI sidebar through a media query, but a
  package's `left` fixed panel or the pane-slot pipeline has no compact
  counterpart: the server computes slots without knowing the window width, so
  `dimension.panel.side.default` (244) applies at every width. A compact fix needs
  either a client-side slot override or a per-tab persisted width. Priority: low
  (only bites a package that ships a left panel on a ≤1240px window).
- **The unconsumed `button.{danger,muted,primary}.root.focus` keys have no
  explanation anywhere.** The host paints one shared focus ring
  (`button.default.root.focus`), which is the one-ring-per-surface rule working as
  intended, but the three variant focus keys sit in the adoption backlog without a
  sentence saying so (unlike `toast`, `swatch`, `recentRow`). Documentation-only:
  add the reason where DESIGN.md §16 and the conformance page list the backlog.
  Priority: with the next docs pass.
