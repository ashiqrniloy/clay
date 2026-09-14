# Design Artifact Gate (Plan 118)

**Files:** `design-artifacts/README.md` (the contract), `design-artifacts/approved/**`, `design-artifacts/prototypes/**`, `design-artifacts/screenshots/**`, `design-artifacts/tools/{make-component-catalog.py,make-pages.py,make-theme-values.py,make-workspace-data.py,capture-prototypes.mjs,verify-component-conformance.mjs,capture-overlays.mjs,capture-tab-views.mjs,capture-agent-files.mjs}`, `scripts/capture-ui-review.sh`, `.agents/skills/create-plan/references/clay.md`, `.agents/skills/clay-execution/references/ui.md`
**Tests:** `tests/manual_smoke_docs.rs` (harness contract), the `--check` modes of the generators, `design-artifacts/tools/verify-component-conformance.mjs` (18/18 audit), `design-artifacts/tools/capture-prototypes.mjs` (112-run matrix)
**Reference Docs:** `DESIGN.md` (normative language spec), `design-artifacts/README.md`, `docs/development/ui-design-system-conformance.md`, `docs/development/ui-design-system-recipe-matrix.md`

---

## 1. Why a gate exists

Clay's UI changes are designed here before they are implemented, because the
failure mode this folder prevents is silent drift: a surface that "looks close
enough" gets shipped, and the next change has no reference to compare against.
The gate is therefore a **state machine over artifacts**, not a screenshot
folder:

```
prototypes/<slug>/   exploratory, real theme values, every state   — no authority
        │  explicit user approval (recorded in the set's README.md)
        ▼
approved/<slug>/     frozen, append-only, hash-recorded             — binding
        │  implementation tasks cite it; visual review compares the running app
        ▼
screenshots/<what>/  captured evidence from the running app         — review, not CI goldens
```

`DESIGN.md` outranks both: an artifact shows surfaces and states, the
specification owns the language. Where they disagree, `DESIGN.md` wins and the
artifact is corrected through a new approval. A deviation is either a defect fix
or an explicit re-approval with the reason recorded (and a decision log when the
language itself changes).

## 2. Prototypes — no authority

A prototype under `prototypes/<slug>/`:

- opens with `file://` and no build step, and states its scope, variants and
  coverage in its own `README.md`;
- renders against **real content-theme values** for every shipped theme through a
  theme switcher (`?theme=` and `data-theme` swatches), never one flattering
  palette;
- covers the states that can ship — `rest`, `hover`, `active`, `focus`,
  `selected`, `disabled`, `invalid`, plus empty/loading/error/recovery where they
  apply — and both narrow and wide windows when layout can change;
- uses the catalog vocabulary (`component.variant.slot.state`) so approval maps
  1:1 onto catalog entries;
- may be replaced or discarded at any time. **No production code may cite a
  prototype as its reference.**

Losing variants stay in the folder, marked as not approved.

## 3. Approved — binding

`approved/<slug>/` records the approval itself: date, the approving user
statement, chosen variant, requested changes, superseded variants, and the exact
surface/state/theme/width coverage. The set is **append-only**: a later change is
a new variant directory plus a new approval, never an in-place edit. Per-file
hashes are recorded so a later edit is detectable.

The plan-118 set (`approved/quiet-instrument-migration/`, approved 2026-09-11)
contains the ten page prototypes (`start.html`, `shell.html`, `workspace.html`,
`agent-landing.html`, `settings.html`, `agent-settings.html`,
`command-centre.html`, `package-workspace.html`, `overlays.html`), the component
catalog, the theme-value board and its specification, the shared kit
(`theme.css`, `ds-quiet.css`, `components.css`, `pages.css`, `ds.js`), the
generated `workspace-data.js`, and the launcher's recorded fixture
`start-recents.json`. It is the reference the migration tasks read before
editing and that the visual review compares the running app against.

## 4. Generators (never hand-edit their output)

| Tool | Output | Gate |
|---|---|---|
| `tools/make-workspace-data.py` | `workspace-data.js` (real repository tree + file bodies) in every artifact folder that ships one | Deterministic regeneration from the live repo |
| `tools/make-component-catalog.py` | `component-catalog.html` from `packages/design-instrument/package.json` | `--check` fails on drift; a 1:1 renderer ↔ declared-slot guard fails on slot drift |
| `tools/make-pages.py` | the page prototypes from the approved screens, the shared language and the repository's own facts | `--check` fails if a page drifted or violates the recipe contract (`CLASS_KEYS` → declared keys only) |
| `tools/make-theme-values.py` | `themes.html` + `theme-values.md` from `theme-values.json` | Computes ratios on the **composited** colour and exits non-zero if a floor is missed; `--check` fails on drift |

Two rules make the outputs trustworthy: every page/prototype data source is the
repository itself (inventory, manifests, file metadata — no fabricated rows),
and every generator is asserted rather than trusted (`--check` in review and as a
pre-commit habit).

## 5. Verification tools

**`tools/capture-prototypes.mjs` — prototype-set gate.** One headless-Chrome pass
over 10 pages × 4 themes × 2 widths plus the named scenes (112 runs). It asserts
zero console errors, zero non-`file://` requests, zero horizontal overflow and
zero clipped boxes, that the requested theme really applied, that each page's
claimed states are present, scene-switcher exclusivity, the live behaviour
probes (picking a launcher row makes the tab openable, the workspace filter
narrows the recents, the tree filter narrows the file list, the composer draws
exactly one focus ring on the shell and spans its zone, and a no-match query
reveals the palette's empty state), and that the agent-file sizes printed on
screen are the sizes on disk. `--no-shots` re-asserts without
writing ~15 MB of PNGs. It exits non-zero on any failure, and it is the gate the
approval round rested on.

**`tools/verify-component-conformance.mjs` — app-vs-specimen gate.** Audits the
running app against the approved specimen and the shipped manifest:
offline manifest audits, then CDP computed-style comparison across the four
shipped themes (plus `@clay/core`), including forced interaction states (hover,
focus, press, selection, transform), radius/border/colour normalisation, and a
`FLUSH_KEYS` allowlist for the genuinely 0-radius full-bleed regions. It uses
`tests/fixtures/design-system-reference-keys.txt` as the baseline: 130 reference
keys (142 minus the 12 removed `chat.default.*` keys) plus the 35 post-approval
additions. Accepted deviations are declared in the tool, not ignored ad hoc.
Recorded result: **18/18 audit checks, zero mismatches**.

**`tools/capture-tab-views.mjs` / `tools/capture-agent-files.mjs` — the tab-chrome
and session-files gates.** The first asserts the titlebar switcher (the `seg`
family, inert with a reason on an uncommitted tab, its rest/selected/disabled
paints); the second asserts the agent view's Files tab (session records newest
first on the `sessionRow` family, role-toned marks, the filter and its count, one
ring on the well). Both drive the real app over CDP and write their own
`screenshots/` set.

**`tools/capture-overlays.mjs` — overlay-family gate.** Drives the dev app over
CDP with each shipped theme's `--clay-*` roles projected the way
`ResolvedUiTheme::base_color` does, then records the overlay family's measurable
claims: the palette sheet (one ring, bounded results, key-hint foot), menu
popover origins, the modal head/body/foot with hairline separations, tooltips,
and the reduced-motion / reduced-transparency fallbacks — with zero horizontal
overflow.

**`scripts/capture-ui-review.sh` — real-app gate.** Separate concern, same
review posture: it boots the actual Linux GUI build in a mode-700 isolated root
and captures AT-SPI + portal PNG evidence. See
[Repeatable UI Review Harness](ui-review-harness.md); the plan-118 launch test
uses its `--example-config` leg against a copy of the canonical
`examples/config/` tree.

## 6. What the artifact gate does not do

- It does not replace automated tests: structural conformance lives in
  `tests/package_ui_conformance.rs`, `tests/theme_packages.rs`, the frontend
  consumption and surface-adoption suites. Artifacts are evidence, not
  assertions.
- It does not produce CI pixel goldens; the PNGs are review artifacts inspected by
  a human (or an agent) before a result is recorded.
- It does not carry runtime weight: nothing here ships in the bundle. The only
  production artifact it touches is the design-system package manifest, whose
  values the catalog reads.

## 7. Cost and drift control

- The prototype set and its captured PNG sample (79 files in the current run)
  live in the repository
  (`design-artifacts/prototypes/quiet-instrument-migration/screenshots/` with
  `report.json`) so a reviewer sees what was approved without regenerating it.
- `--check` modes turn "the artifact drifted from the contract" into a
  non-zero exit, which is what keeps the approved specimen honest as the
  package grows (the shipped manifest is now 165 keys while the approved catalog
  records the 130-key specimen — that delta is explicit in the tooling, not
  silent).
- `tests/package_ui_conformance.rs` enforces the machine-readable half of the
  contract: the recipe matrix's `†` markers are recomputed from the shipped
  manifest in both directions, and no catalog page may present a removed design
  system as shipped.

## 8. Related

- [UI Design System Runtime](ui-design-system-runtime.md) — the recipe contract
  the catalog and specimen are generated from
- [Repeatable UI Review Harness](ui-review-harness.md) — real-app capture with
  `review.status` PASS/UNRESOLVED semantics
- [Launcher Landing Surface](launcher-landing-surface.md) — the plan-118 surface
  whose reference is `start.html`
- `DESIGN.md` — the normative language specification
- `design-artifacts/README.md` — the binding contract page (workflow steps,
  approval record, current contents)
