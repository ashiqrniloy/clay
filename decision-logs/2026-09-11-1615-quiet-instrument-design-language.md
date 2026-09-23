---
date: 2026-09-11 16:15
status: approved
decision_about: "Clay UI design language: Quiet Instrument replaces Restrained Neobrutal as the design direction"
proposed_by: "agent (proposal round), selected by user"
explicitly_approved_by_user: true
---

# Decision: Adopt Quiet Instrument as Clay's UI design language

## Decision

Clay's UI design language becomes **Quiet Instrument**: one continuous surface
zoned by 1px hairlines and whitespace, exactly two elevations (canvas and
overlay), accent reserved for state (focus, selection, active navigation,
running work), a 5/8/12/16/pill radius ladder, monospace type for every datum,
and 150ms/240ms decelerating motion with a one-shot keyboard-focus pulse. The
language is specified normatively in `DESIGN.md` and is implemented — in a
separate migration phase — by a new first-party design-system package
(`@clay/design-instrument`, display name "Quiet Instrument"), which becomes the
default. `@clay/design-neobrutal` and `@clay/design-glass` remain shipped and
user-selectable. No schema, authority, or color-authority change is involved.

## Context

Clay's design-system architecture (decision
`2026-08-28-2234-package-defined-ui-design-systems`) fixes the layering:
content themes own color, UI design systems own geometry/material/motion,
typography stays user-owned, React Aria + Clay own behavior and accessibility.
Within that architecture, the shipped default visual language was the
Restrained-Neobrutal direction (plan 104, legibility-revamped by plan 110):
0px corners, 2px ink borders, hard offset shadows, opaque boxed panels.

A design proposal round (2026-09-11) produced four self-contained HTML screens
under `design-artifacts/DS/`: for both the Workspace and Coding-Agent surfaces,
one legibility-improved Neobrutal variant and one ground-up rethinking, across
six themes (the four shipped themes plus Kanagawa Wave and Catppuccin Latte),
with a shared token layer whose contrast pairs were measured and repaired
(`design-artifacts/DS/README.md` records the WCAG table: text ≥ 4.5:1, UI ≥ 3:1
in all six themes).

The user reviewed the artifacts and selected the rethinking direction for both
surfaces as Clay's design language, then asked for this documentation
alignment ahead of a separate migration plan.

## Approval

- Proposed by: agent (two directions per screen, plus six-theme token layer);
  Neobrutal-improved and Quiet Instrument were both built and shown.
- Approved by user: Yes.
- Approval evidence: "Okay based on my analysis, I have decided that I will go
  with `design-artifacts/DS/workspace-rethink.html` and
  `design-artifacts/DS/agent-rethink.html` design language. What I need you to
  do is that you will now create a detailed design system that will replace
  every design related instructions in ... After documentation is aligned with
  the new design direction with all necessary details and instructions, we move
  forward with planning the migration to new design system."

## Alternatives Considered

1. **Keep Restrained Neobrutal as the default and ship Quiet Instrument as an
   optional package** — rejected: the user's direction is a change of default,
   and the diagnosis (structural attention spent on chrome in long sessions)
   applies to the shipped default, not to a niche alternative.
2. **Adopt the improved-Neobrutal variants (2px ink borders, hard ink shadows,
   solid row fills)** — not selected: these maximize structural legibility at
   the cost of the noise the rethinking direction removes; both were built and
   the rethinking direction was chosen.
3. **Adopt one of the three languages planned by `plans/111` (Graphite
   Precision, Warm Analog, Glass Cockpit)** — not selected: that plan's premise
   (three user-approved languages from an earlier proposal round) is superseded
   by this decision; plan 111 is unstarted, so the migration plan supersedes its
   scope rather than shipping two competing directions.
4. **Quiet Instrument as the new default direction (chosen)** — keeps every
   architectural invariant, needs no schema extension (verified: opacity fills,
   inset-shadow state marks, spread-shadow focus halos, and negative-spread
   soft elevation all fit the existing typed property domains), and its
   geometry/material/motion values are fully expressible as one inert
   `uiDesignSystem` contribution.

## Rationale and Evidence

- **The problem is structural attention, not contrast.** Plan 110's legibility
  revamp fixed Neobrutal's contrast, but the language still frames every panel
  and boxes every row, so three border weights compete for attention in a
  surface that stays open for hours. Quiet Instrument reserves structure for
  boundaries that carry meaning and lets type carry hierarchy.
- **Expressiveness verified against the real schema, not assumed.**
  `src/shell/design_system.rs` (bounds: radius `[0,32] ∪ 9999`, border width
  `[0,8]`, blur `[0,32]`, shadow layers ≤ 3 with spread `[-16,16]`, motion
  `[0,1000]`, `transform-preset` and `transition-timing` closed enums) and
  `frontend/src/theme/design-system-adapter.ts` (inset shadow layers,
  `color-mix` opacity, spread support) cover every value the language needs:
  veil/accent/disabled fills → `backgroundOpacity`; 2px leading bars and tab
  underlines → inset zero-blur shadow layers; focus halos → zero-blur shadow
  with positive spread; two-level elevation → two-layer negative-spread
  shadows. **No additive schema work is required.**
- **The color-authority invariant is untouched.** The language selects existing
  theme color roles; the four theme-side changes it needs (hairline role at
  ~34% ink alpha, subtle at ~62%, canvas/chrome depth steps, opaque
  hover/selected steps) are value overrides through existing typed
  `designTokens`, not new color authority for packages.
- **Architecture compatibility.** Inert data only, zero permissions, no
  `entry`, 142-key recipe set already consumed by host CSS (the
  design-system consumption gate keeps every key honest), whole-shell restyle
  without DOM remounting, revocation fallback to the previous valid system.
- **Measured contrast evidence.** The proposal token layer failed WCAG in 32
  pairings across the six themes before repair and passes after
  (`design-artifacts/DS/README.md`); those repairs are inputs to the theme-side
  migration work rather than a design-system concern.

## References

- `DESIGN.md` — normative Quiet Instrument specification (laws, values,
  geometry, materials, motion, typography, state language, component recipes,
  shell composition, accessibility, retired patterns, checklist, migration
  profile).
- `design-artifacts/DS/` — `workspace-rethink.html`, `agent-rethink.html`,
  `ds-quiet.css`, `ds.js`, `theme.css`, `README.md` (proposal, rationale,
  measured contrast table).
- `docs/development/ui-design-system-visual-direction.md` — direction contract
  and comparison grammar (this decision).
- `docs/reference/ui-design-systems.md`,
  `docs/development/ui-design-system-recipe-matrix.md`,
  `.agents/skills/clay-execution/references/{ui,components,tokens}.md` —
  updated catalogs and rules.
- `decision-logs/2026-08-28-2234-package-defined-ui-design-systems.md` —
  architecture that makes this a data-only change.
- `plans/110-UI-Design-System-Consistency-Neobrutal-Legibility-Revamp.md` —
  the legibility baseline this decision supersedes as a *direction* while
  keeping its invariants (contrast, state completeness, consumption gate).
- `plans/111-Graphite-Analog-Cockpit-Design-Systems-and-Paired-Themes.md` —
  unstarted plan whose language scope this decision supersedes.

## Consequences

- **Positive:** the default surface stops competing with its content; chrome
  becomes one hairline and air; state becomes unambiguous because accent is
  scarce; long-session reading gets a real measure (92ch editor, 72ch
  transcript); the design language is expressible as pure data, so the
  migration touches packages, theme values, host geometry, and docs — not the
  recipe engine or authority boundaries.
- **Costs / follow-up work:**
  - Author the `@clay/design-instrument` package (values + 142 consumed recipe
    keys) and add it to the bundled inventory.
  - Apply theme-side value overrides (hairline alphas, depth steps, opaque
    state steps) to the shipped themes; re-verify contrast per theme.
  - Host-side geometry work: row/control heights, rail widths, reading
    measures, on-demand path strip, agent inspector grouping, focus-flash
    behaviour, and the `@clay/core` fallback profile (no 0px radius, no hard
    shadow).
  - Settings design-system enumeration and the static list in
    `packages/settings/package.json` need the new entry.
  - Plan the migration as its own plan; supersede plan 111's language scope
    explicitly.
- **Risks:**
  - Hairline-only zoning can read as unfinished if a theme ships an opaque
    hairline or a flat depth step — the four theme-side requirements in
    `DESIGN.md` §10 are load-bearing, not cosmetic.
  - The primary-button hover darkening in the proposal (accent mixed toward
    text) is not expressible; the documented approximation is an opacity step.
  - Three shipped design systems multiply the cross-product conformance surface
    (3 systems × N themes).
- **Conditions that would cause revisiting:** a measured legibility or
  focus-visibility regression in review; the theme-side alpha steps proving
  incompatible with a shipped theme's palette (forcing a different depth
  strategy); or the language failing to express a surface the recipe matrix
  requires, at which point additive typed properties are considered instead of
  host CSS overrides.
