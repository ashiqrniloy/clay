# design-artifacts — prototype and approved-design contract

Every piece of Clay UI is designed here before it is implemented. This folder is
the visual record; `DESIGN.md` at the repository root is the normative language
specification (values, geometry, materials, motion, recipes, retired patterns).

```
design-artifacts/
├── README.md                       this contract page
├── prototypes/<slug>/              exploratory HTML — no authority
├── approved/<slug>/                user-approved, append-only — binding
├── screenshots/<date>-<what>/      captured evidence (current UI, reviews)
└── tools/                          checked-in generators for artifact data
```

Artifacts are committed to git. A reference that exists on one machine is not a
reference.

## Prototypes — no authority

`prototypes/<slug>/` holds exploration, one folder per design effort, open with
`file://` and no build step. A prototype:

- states its scope, variants and coverage in its own `README.md`;
- renders against real content-theme values (all shipped themes, via a theme
  switcher) rather than one flattering palette;
- covers the states that can ship — `rest`, `hover`, `active`, `focus`,
  `selected`, `disabled`, `invalid`, plus empty/loading/error/recovery where
  they apply — and both narrow and wide windows when layout can change;
- uses the catalog vocabulary (`component.variant.slot.state`, catalog kind and
  token names) so approval maps 1:1 onto catalog entries;
- may be replaced, extended or discarded at any time. Nothing in production may
  cite a prototype as its reference.

Losing variants stay here for history, marked as not approved.

## Approved — binding

`approved/<slug>/` holds what the user approved. An approved artifact:

- records the approval in its `README.md`: date, the approving user statement,
  chosen variant, requested changes, superseded variants, and the exact
  surface/state/theme/width coverage;
- is **append-only** — a later change is a new variant directory plus a new
  approval, never an in-place edit;
- is the reference implementation tasks read before editing and that visual
  review compares the running app against;
- does not replace `DESIGN.md`: the artifact shows surfaces and states, the
  specification owns the language. Where they disagree, `DESIGN.md` wins and the
  artifact is corrected through a new approval.

Deviating from an approved artifact is either a defect fix or an explicit
re-approval (with the reason recorded, and a decision log when the language
itself changes). Silent drift is a defect.

## Workflow

1. Prototype the surface under `prototypes/<slug>/` (all themes, all states).
2. Get explicit user approval; freeze the chosen variant under
   `approved/<slug>/` with its approval record.
3. Implement against the approved artifact; cite its path in the task.
4. Visual/accessibility review the running UI against it and record every
   deviation with its disposition.
5. When the language itself changes: edit `DESIGN.md`, change design-system
   package data (never host CSS), then re-approve through step 1.

Planning duty and task templates live in
`.agents/skills/create-plan/references/clay.md` → *UI Prototype and Explicit User
Approval Task*; execution rules in
`.agents/skills/clay-execution/references/ui.md`.

## Current contents

| Path | What it is |
|---|---|
| `approved/quiet-instrument-language/` | The approved Quiet Instrument design language (Workspace + Coding Agent), adopted 2026-09-11, spec in `DESIGN.md`. |
| `prototypes/quiet-instrument-language/` | The rejected neobrutal-improved alternatives from the same proposal, kept for history. |
| `screenshots/2026-09-11-current-ui/` | The pre-migration UI as it looked when the language was chosen (baseline evidence). |
| `screenshots/quiet-instrument-agent/`, `-settings/`, `-shell-workspace/`, `-overlays/`, `-tab-views/` | The plan-118 adoption evidence: captured app states per shipped theme and width for the agent view, the settings panel / agent settings page, the shell + workspace, the overlay family, and the tab's two-view chrome (each with a `report.json` of the asserted claims). |
| `screenshots/quiet-instrument-component-conformance/` | The component-level conformance run written by `tools/verify-component-conformance.mjs` (18/18 audit checks, zero browser mismatches across the shipped themes). |
| `prototypes/quiet-instrument-migration/screenshots/` | The migration prototype set as captured for review (66 PNGs + `report.json`), written by `tools/capture-prototypes.mjs`. Every run is asserted; the stored sample is documented in the set's README §8. |
| `approved/quiet-instrument-migration/` | The approved Quiet Instrument migration set (2026-09-11): the 10 page prototypes, the component catalog, the theme-value board + spec, the shared kit and the launcher's recorded fixture, frozen with per-file hashes and the approval record in its `README.md`. Binding for plan 118 implementation; the working copy stays in `prototypes/quiet-instrument-migration/`. |
| `prototypes/quiet-instrument-migration/` | The migration prototype set (plan 118): `component-catalog.html` (component specimen, 130 recipe keys over 49 families, generated from the package contract), nine page prototypes (the launcher `start.html`, shell, workspace, the agent view, settings, agent files, command centre, packages, overlays — see `README.md`), the theme-value board `themes.html` with its specification `theme-values.md` and source `theme-values.json`, the shared language (`components.css`, `pages.css`, `ds-quiet.css`, `theme.css`, `ds.js`), and the surface inventory in `README.md`. |
| `tools/make-workspace-data.py` | Regenerates `workspace-data.js` (real repository tree + file bodies) in every artifact folder that already ships one. Never hand-edit that file. |
| `tools/make-component-catalog.py` | Regenerates `prototypes/quiet-instrument-migration/component-catalog.html` from `packages/design-instrument/package.json` (the recipe contract; it read the removed Neobrutal package before plan 118 task 9). `--check` fails if the committed page has drifted; never hand-edit the page. Regenerating now emits the ten new-family specimens, so the committed approved page reports drift until the migration re-approves it. |
| `tools/make-pages.py` | Regenerates the eight page prototypes from the approved screens, the component language and the repository's own facts (inventory, manifests, file tree snapshot). `--check` fails if any page has drifted or violates the recipe contract; never hand-edit those pages. |
| `tools/capture-prototypes.mjs` | Verifies the whole migration prototype set in one headless-Chrome pass (10 pages × 4 themes × 2 widths + the scenes) and writes the review PNGs to `prototypes/quiet-instrument-migration/screenshots/`. Asserts zero console errors, zero non-`file://` requests, zero overflow/clipped boxes, the theme really applied, the claimed states present, the scene switcher's exclusivity, two live behaviour claims, and that the agent-file sizes printed on screen are the sizes on disk. Exits non-zero on any failure, so it is the gate task 10's approval rests on. `--no-shots` asserts without writing 15 MB. |
| `tools/make-theme-values.py` | Measures the proposed theme values and writes `prototypes/quiet-instrument-migration/themes.html` + `theme-values.md` (from `theme-values.json`). Ratios are computed on the composited colour, the four themes' ladders and floors are asserted, and the exit status is non-zero if a floor is missed. `--check` fails on drift. |
| `tools/verify-component-conformance.mjs` | Audits the running app against the approved specimen + the shipped manifest: offline manifest audits plus CDP computed-style comparison across the shipped themes and forced interaction states, with `tests/fixtures/design-system-reference-keys.txt` as the key baseline (130 reference keys + the 35 post-approval additions) and declared accepted deviations. Writes its evidence to `screenshots/quiet-instrument-component-conformance/`. |
| `tools/capture-tab-views.mjs` | Drives the real shell over CDP and asserts the tab chrome's claims — the view switcher present in the titlebar, consuming the `seg` family, inert with a reason on an uncommitted tab, and its rest/selected/disabled paints (state probes wait out the 150ms transition) — with zero horizontal overflow, writing `screenshots/quiet-instrument-tab-views/`. |
| `tools/capture-sidebar.mjs` | Drives the workspace sidebar over CDP (`?fixture=workspace-sidebar`) and asserts the approved head's claims — the filter field with its `/` chip, local filtering of the delivered rows, the live match count, the foot hints, the 244px/224px token-sized region, zero horizontal overflow — writing `screenshots/quiet-instrument-sidebar/`. |
| `tools/capture-agent-files.mjs` | Drives the real agent panel over CDP (`?fixture=coding-agent&state=conversation`) and asserts the session-files claims — the session's own records newest first on the `sessionRow` family with the recipe's paint, role-toned marks, basename + muted directory, the filter narrowing the list with the count following, the filter's one-ring well, zero horizontal overflow — writing `screenshots/quiet-instrument-agent-files/`. |
| `tools/capture-overlays.mjs` | Drives the dev app over CDP with each shipped theme's `--clay-*` roles projected as `ResolvedUiTheme::base_color` does, and asserts the overlay family's claims (palette sheet one-ring + bounded results + key-hint foot, menu popover origin, modal head/body/foot hairlines, tooltip, reduced-motion/reduced-transparency fallbacks) with zero horizontal overflow, writing `screenshots/quiet-instrument-overlays/`. |
