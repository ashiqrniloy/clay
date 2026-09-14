# UI Design System Visual Direction Contract

**Date:** 2026-09-11 (supersedes the 2026-09-05 Neobrutal contract)
**Status:** Approved Direction Contract — Quiet Instrument (**implemented**; package, themes, gates, fallbacks and the host CSS adoption are shipped; the target IA's tab-model work is the remaining plan-118 feature work — see [`DESIGN.md`](../../DESIGN.md) §16). The comparison columns below are the evaluation record of the 2026-09-11 round; the two comparator systems were removed by the migration (plan 118 task 9).
**Normative spec:** [`DESIGN.md`](../../DESIGN.md) — this document records the *why* and the comparison; `DESIGN.md` owns the values.
**Approved artifacts:** `design-artifacts/approved/quiet-instrument-language/workspace-rethink.html`, `design-artifacts/approved/quiet-instrument-language/agent-rethink.html`, `design-artifacts/approved/quiet-instrument-language/ds-quiet.css`, `design-artifacts/approved/quiet-instrument-language/theme.css`, `design-artifacts/approved/quiet-instrument-language/README.md` (contract: `design-artifacts/README.md`)
**Related Plans (historical records):** [101](../../plans/101-UI-Design-System-Recipe-Foundation.md), [102](../../plans/102-UI-Design-System-Activation-and-Frontend-Runtime.md), [103](../../plans/103-UI-Design-System-Component-and-Surface-Migration.md), [104](../../plans/104-Neobrutal-and-Glass-Design-System-Packages-and-Conformance.md), [110](../../plans/110-UI-Design-System-Consistency-Neobrutal-Legibility-Revamp.md)
**Product Reference:** [`PRODUCT.md`](../../PRODUCT.md)
**Surface Brief:** [`.impeccable/surfaces/operate-visual-direction.md`](../../.impeccable/surfaces/operate-visual-direction.md)
**Decision Log:** [`decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md`](../../decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md), [`decision-logs/2026-09-11-1615-quiet-instrument-design-language.md`](../../decision-logs/2026-09-11-1615-quiet-instrument-design-language.md)

---

## 1. Executive Summary

Clay's UI design language becomes **Quiet Instrument**, selected from a four-screen,
design proposal round (2026-09-11, approved artifacts in `design-artifacts/approved/quiet-instrument-language/`). The language
keeps Clay's architectural invariants unchanged — content themes are the sole
color authority, design systems own geometry/material/motion, typography stays
user-owned — and replaces the former Restrained-Neobrutal default with a single-surface,
hairline-zoned, state-driven instrument face.

The proposal round compared two directions per screen: a legibility-repaired
**former Neobrutal** (0px corners, 2px ink borders, hard offset shadows, since
removed) and a ground-up rethinking (hairline zoning, two elevations,
accent-as-state). The rethinking direction was chosen on the evidence of
long-session use: the former Neobrutal face spends structural attention on
itself (every panel framed, every row boxed, static chrome shadowed like a
control), which reads as noise in a surface that is open for hours.

---

## 2. Direction Contract

### THESIS
A quiet instrument: one continuous surface zoned by hairlines and whitespace,
exactly two elevations (canvas and overlay), accent reserved for state rather
than decoration, and typography — monospace for data, not for prose — carrying
the hierarchy that borders used to. It refuses boxed-in-boxes chrome, hard
offset shadows, 90-degree control corners, gradients, textures, and any motion
that does not report a state change.

### OWN-WORLD
- **Palette & Material Authority:** Content themes remain the sole color
  authority. The default surface is the canvas itself (`surface.main`);
  grouped content sits on a veil (`surface.panel` at `opacity.veil` = 0.55);
  insets (fields, composers, meters) use `surface.control`; transient layers use
  `surface.overlay` with the overlay/pop shadow recipes. Chrome strips inherit
  the canvas and are separated by one hairline (`border.hairline`).
- **Geometry & Structure:** A single radius ladder — 5 (chips/kbd/badges), 8
  (controls, rows, tabs), 12 (panels, popovers), 16 (window, sheets, modals),
  9999 (pills) — and one border weight (1px hairline). The two-pixel marks are
  states only — the focus ring and the composer shell's accent boundary, both
  inset shadow layers — while a leading selection bar and a tab underline are
  retired patterns (`DESIGN.md` §14.13).
  Full-bleed region edges stay square and flush.
- **Spatial Rhythm:** 32px rows (26 compact), 30px controls (28 compact), 40px
  title bar, 28px status bar; 4/8/12/16/24px spacing steps; panel interiors
  10–14px; sheets 18–20px. Density scales the spacing rhythm only.
- **Motion Grammar:** 150ms `ease-out` for state changes, 240ms
  `spring-snappy` for surfaces entering (fade + small translate/scale), a
  one-shot 620ms accent pulse when a keyboard action moves focus. No hover
  lift, no bounce, no loops except the running-work indicator. Reduced motion
  collapses everything to instant.
- **Type & Measure:** 20/15/13/12/11/10px ladder; monospace for every datum
  with tabular figures; 10px/0.14em uppercase micro-labels for section
  eyebrows; editor column 92ch, transcript 72ch, empty-state prose ≤48ch.

### STORY
The developer opens Clay and lands on the launcher — recent workspaces and
configured agents, one of each per launch — and then sees the document, not the
editor. Regions are separated by a single hairline and air; nothing is framed.
The eye lands on text and data. Moving the pointer tints a row; pressing a
button nudges it 1px; selecting a file tints its fill and changes the text role
(nothing is drawn on its leading edge).
When a keyboard action moves focus, the destination pulses once so the eye
finds it. Overlays lift above the plane on a soft, wide shadow — the only
shadows in the application — and then disappear without ceremony.

### FIRST VIEWPORT
Launcher (`⌘T`, and what a fresh window opens): recent workspaces and
configured agents side by side, each pane with a filter well, closed by one
action row whose primary button names what it will open. Choosing a workspace
opens the tab's workspace view: sidebar with the workspace tree (mono names,
right-aligned counts, count footer) · centered editor column with a line-number
gutter and a 92ch measure · optional right rail with document facts and the
entry outline · status bar carrying workspace, connection, word/entry counts,
and the keyboard hint row. `⌘2` switches the same tab to its agent view:
picker-as-title header, 72ch transcript, one-boundary composer, inspector with
the Files (session history), Memory, Context, Session Info and Settings tabs.
`⌘K` raises the centered command palette: radius-16 sheet, hairline border,
overlay shadow, mono group eyebrows, rows that tint on hover and on keyboard
selection.

### FORM
Quiet Instrument (approved direction from the 2026-09-11 four-screen proposal
round; two rethinking screens — workspace and coding-agent — plus a shared
token/behaviour layer). Ancestry: technical instrument panels and editorial
typography — the density discipline of a mixing desk, the restraint of a
specimen sheet.

### FINISH CONDITION
Unreviewed and undocumented is unfinished: the migration ends with the design
system package shipped, the visual/accessibility review recorded, `DESIGN.md`
conformance proven across every recipe-styled surface in the matrix, and the
retired patterns of `DESIGN.md` §14 provably absent.

---

## 3. Visual System Comparison & Conformance Grammar

| Attribute | **Quiet Instrument** (`@clay/design-instrument`, shipped default) | Former Neobrutal (removed, plan 118 task 9) | Former Luminous Glass (removed, plan 118 task 9) | Conformance rule |
| --- | --- | --- | --- | --- |
| **Corner radius** | 5 / 8 / 12 / 16 / pill; full-bleed edges square | `0px` everywhere | 4–14px + pills | Radii resolve from recipe variables; no CSS overrides in components |
| **Borders** | 1px `border.hairline` only; 2px state marks are inset shadows | 2px `text.primary` ink at rest | 1px `border.subtle`, translucent | Border colors reference theme roles; no literals |
| **Surfaces** | Single surface + veil planes (`panel` @0.55) + `surface.control` insets | Opaque boxed panels and rows | Translucent fills + blur | Depth comes from role + opacity, never from an added frame |
| **Elevation** | Two levels: canvas and overlay (`0 24px 60px -20px` @0.42 + `0 2px 10px -4px` @0.22) | Hard `3px 3px 0` offsets | Diffuse ambient shadows | Max 3 shadow layers; static surfaces carry none |
| **Hover feedback** | Fill change (`surface.hover`) | Position lift + shadow growth | Brightness lift | No position change on hover |
| **Press feedback** | `press-shift-down` (1px) + `surface.active` | Collapse shadow to `1px 1px 0` | Scale 0.99 | Tactile confirmation without layout movement |
| **Focus** | 2px `focus.ring`, offset 2px, plus 620ms pulse on keyboard moves | 2px ink outline | 2px glow ring | WCAG 2.1 AA visible focus in every theme |
| **Blur** | Scrim (3px) and toast (8px) only | none | 8–24px on many surfaces | Editor canvas, gutter, scroll, panels, rows: `backdropBlur == 0` |
| **Type role** | Mono for data, UI face for prose, tracked micro-labels | same roles | same roles | Concrete families/sizes stay user-owned |
| **Density** | 32px rows / 30px controls (26/28 compact) | same host geometry | same host geometry | Layout geometry stays host-owned and identical across systems |

---

## 4. Accessibility and Platform Constraints

1. **Contrast (Plan 110 Gate):** text ≥ 4.5:1, UI affordances ≥ 3:1, enforced
   at theme activation. The proposal's six themes each needed contrast repairs
   (hairline, meta text step, syntax number/comment roles, focus ring) — those
   repairs are recorded in `design-artifacts/approved/quiet-instrument-language/README.md` and are part of the
   theme-side migration work.
2. **Focus visibility:** a 2px `focus.ring` outline at 2px offset on every
   interactive element; the accent halo is *additional* to the ring, never a
   substitute (`accent.primary` must clear 3:1 on the surface it outlines).
3. **Hover is never load-bearing:** every hover-revealed affordance is also
   keyboard-reachable and visible in rest or focus state.
4. **Forced colors:** system colors replace canvas, text, highlights, borders,
   and focus; hairline-only structure must still read as structure.
5. **Reduced motion / transparency:** durations collapse to instant, transforms
   are removed, all fills become opaque, blur is disabled.
6. **Keystroke latency:** recipes compile to static `--clay-ds-*` custom
   properties once per activation; zero measurements, observers, or style
   injection in paint/input/layout paths.

---

## 5. Traceability and Next Steps

- **Approved (2026-09-11):** the four-screen proposal (approved language in `design-artifacts/approved/quiet-instrument-language/`, rejected neobrutal variants in `design-artifacts/prototypes/quiet-instrument-language/`):
  former-Neobrutal-improved and rethinking variants of the Workspace and
  Coding-Agent screens, six themes, contrast-verified token layer.
  *(Update, plan 118 task 9: the former Neobrutal and Glass packages were removed;
  `@clay/design-instrument` is the only shipped first-party design system.)*
- **Approved (2026-09-11):** Quiet Instrument chosen as Clay's design language;
  `DESIGN.md` rewritten as the normative specification. The two comparator
  packages were still shipped at that moment and were removed later in plan 118
  task 9.
- **Documentation (complete):** direction contract, product commitments,
  design-system reference, component/token catalogs, recipe-matrix profile,
  conformance document, CSS audit note, wiki runtime page, manual test plan,
  package authoring guide, create-plan UI gate, and the `clay-execution` UI
  references all point to `DESIGN.md`; no former-Neobrutal design instruction
  remains.
- **Migration (plan 118, in progress):** done — the package (values + 165 recipe
  keys, zero schema change), the four themes' thirteen UI roles with the
  composited contrast gate, the core fallback alignment and the pre-bootstrap
  projection, the selection/enumeration contract, the conformance suites, the
  host CSS adoption across shell / workspace / agent / settings / overlays, the
  removal of `@clay/chat` and of the two comparator systems, the launcher as the
  landing surface, and the visual/accessibility review. The
  unstarted `plans/111-Graphite-Analog-Cockpit-Design-Systems-and-Paired-Themes.md`
  direction is superseded. Remaining — the target IA's tab model, agent picker
  and session-files surfaces (tasks 33–36) plus the documentation/API close-out.
