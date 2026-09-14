---
date: 2026-09-11 17:00
status: approved
decision_about: "Quiet Instrument migration scope: one design system, four existing themes, chat removed, Coding Agent as the landing surface"
proposed_by: "user (scope), agent (implementation consequences)"
explicitly_approved_by_user: true
---

# Decision: Quiet Instrument migration scope — single design system, no new themes, chat removed, Coding Agent lands the app

> **Point 5 superseded on 2026-09-11:** the landing surface is the launcher (a
> tab is one workspace plus one agent with two views), not the Coding Agent —
> see `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`.
> Points 1–4 (one design system, four themes, chat removed, Landing as a revisited
> product decision) stand unchanged.

## Decision

The migration to the Quiet Instrument language (decision
`2026-09-11-1615-quiet-instrument-design-language`) is scoped as follows:

1. **All components and all surfaces adopt the language** at component level
   (every cataloged component kind, every state) and page level (shell, workspace,
   coding agent, settings, command centre, package workspace, overlays).
2. **Themes stay decoupled and user-changeable.** Themes remain the sole
   normal-rendering colour authority; the migration changes theme *values* only
   (typed `designTokens` overrides for hairlines, accent, muted steps and state
   fills) plus the small structural work needed to express them. No theme
   contract, no new theme, and no extra palette ships: Clay keeps exactly the four
   content themes (`@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi`,
   `@clay/theme-gruvbox-material-dark`, `@clay/theme-gruvbox-material-light`).
3. **`@clay/design-neobrutal` and `@clay/design-glass` are removed**, not merely
   deprecated, together with their fixtures, harness states, settings entries,
   tests and documentation. `@clay/design-instrument` (display name "Quiet
   Instrument") becomes the single shipped design system and the default; the
   `@clay/core` fallback stays as the built-in baseline and is aligned with the
   same geometry so pre-bootstrap paint matches.
4. **The chat interface is removed completely**: `@clay/chat` (its package, the
   empty-tab landing contribution, `chat.*` commands and their legacy
   `chat.open*Picker` aliases, server intent handling, docs, fixtures, tests) and
   `frontend/src/chat/`. The shared AG-UI agent transport stays — it is the
   coding agent's event/state path, not a chat feature.
5. **The Coding Agent becomes the landing surface**: `@clay/coding-agent`
   contributes the `empty-tab` pane content, so a fresh window (and a new empty
   tab) opens the agent instead of a chat greeting. The core fallback for "no
   empty-tab contribution installed" stays Open File / Open Folder. This is a
   product decision for now and is expected to be revisited later.

## Context

The design language was adopted on 2026-09-11 with the expectation that the
design-system packages already shipped would remain selectable
(`2026-09-11-1615`). The user's migration directive changes that retention
position and fixes the shipping scope: one language, one design system, the four
existing themes, no new theme, no chat surface, and the agent as the entry point.
The repository state this lands on:

- Three design-system packages exist conceptually (`@clay/core` fallback,
  `@clay/design-neobrutal` as runtime default, `@clay/design-glass` as
  reference), each with 142 recipe keys, plus `@clay/core` fallback recipes in
  `core_design_system_fallbacks()` and matching `--clay-ds-*` fallbacks in
  `frontend/src/styles/tokens.css`.
- Four theme packages ship legacy `textStyles` only; the typed
  `clay.contributions.designTokens` override path already exists and accepts
  `#rrggbbaa` colour roles for core tokens, so the theme work needs no schema
  change and no Rust change.
- `@clay/chat` owns the `empty-tab` landing (`chat.entry`) and `@clay/coding-agent`
  owns a `pane` surface; the host renders trusted first-party surfaces with
  dedicated React panels (`ChatPanel`, `CodingAgentPanel`) and package-name
  provenance checks.
- Removing packages touches the checked-in bundled inventory
  (`src/packages/bundled-inventory.toml` + build-time fingerprints), the design
  system conformance matrix, harness fixtures/states, the settings dropdown data,
  the generated Clay JS API registry, and documentation.

## Approval

- Proposed by: user (scope, removals, landing decision), agent (consequences and
  boundaries recorded above).
- Approved by user: Yes.
- Approval evidence: "Change all components and all surfaces to adopt the new
  design direction at component level and page level, maintain the theme and
  design methodology meaning theme remains decoupled and can be changed by the
  user, remove both neobrutal and glass design system that is currently
  implemented, do not implement any new theme. We work with the existing four
  themes. Additionally, I want to remove the chat interface completely. It is not
  needed. For now, the landing page should be the Coding agent itself. We will
  change that later."

## Alternatives Considered

1. **Keep neobrutal and glass as selectable packages** (the position recorded in
   `2026-09-11-1615`) — rejected by the user: two dead visual languages keep
   recipes, fixtures, harness states, docs and conformance obligations alive for
   no shipping value, and they invite future work to imitate them.
2. **Remove design systems entirely and collapse everything into the core
   fallback catalog** — rejected: it deletes the typed recipe boundary that
   decision `2026-08-28-2234` established (declarative package data, versioned
   additive contracts, revocation/conformance) and with it the ability to switch
   or add a design system later without host CSS surgery. One package keeps the
   architecture and its tests honest.
3. **Derive the hairline/subtle/strong ladder in Rust from the theme ink colour
   instead of adding theme values** — viable and smaller, but rejected: it takes
   tuning away from theme authors, cannot express a themed accent (the caret is
   ink on all four themes), and makes colour authority partly host-side. The
   typed `designTokens` path keeps every colour decision in the theme package.
4. **New palette set (Kanagawa Wave / Catppuccin Latte from the proposal)
   instead of the four existing themes** — rejected by the user: no new theme.
5. **Keep chat as a non-landing pane/surface** — rejected: the shared AG-UI
   transport already backs the coding agent, so the chat surface, its commands and
   its package only add a second product surface to migrate and document.
6. **Let the host render the Coding Agent for the empty tab without a package
   contribution** — rejected: product landings are package contributions
   (decision `2026-08-21-2152`); a compiled landing stub would make the agent
   irreplaceable and re-introduce product-named pane kinds in core.

## Consequences

- `@clay/design-instrument` must be authored (142 recipe keys + design-system
  values) and mirrored into `core_design_system_fallbacks()` and
  `frontend/src/styles/tokens.css`.
- Themes gain typed `designTokens`; `REQUIRED_CONTRAST_PAIRS` grows a structural
  boundary pair, and the theme conformance matrix stops asserting "zero
  designTokens" and validates the resolved palette instead.
- Persisted `designSystem` preferences naming a removed package must fall back
  safely to the new default with a diagnostic instead of failing activation.
- Harness fixtures/states for neobrutal and glass are deleted; the design-system
  harness states are re-pointed at the shipped system.
- Removing `@clay/chat` deletes a first-party manifest, so the bundled inventory
  fingerprints and the example configuration (`examples/config/init.js`,
  `examples/config/packages/first-party.js`) change in the same phase.
- The roadmap's Phase 2 agent-UI binding spec (chat extension points, launch
  split) is reconciled with the approved agent surface in the migration's
  documentation work.
