# UI Components, Tokens, and Conformance

Navigation and contract entry for Clay's reusable UI surface. This page links the authoritative catalogs and the rules for using them so any agent or developer can discover every reusable UI primitive/component and the conformance rules without reading paint code. It is maintained by Phase 20.8 and updated whenever the component catalog, token catalog, or conformance rules change.

## Single Source of Truth

The `clay-ui` skill references are the authoritative catalog. This page links them; it does not duplicate them. When a UI phase adds, removes, or changes a component, primitive, style variable, token, or layout rule, the phase updates the catalog in the same change and this page stays a navigation entry.

- [Component catalog](../../.agents/skills/clay-ui/references/components.md) — every package-facing `ComponentKind`, typed style variable, Clay-native surface, chrome primitive, planned component, typography variant, and the rules for adding components. Status legend: **implemented** / **reserved** / **planned** / **internal**.
- [Token catalog](../../.agents/skills/clay-ui/references/tokens.md) — the ten typed token domains, every implemented core token, typography hierarchy, package token contributions, and the rules for adding tokens.

## Reference Documents

- [UI Chrome Primitives](primitives/ui-chrome-primitives.md) — Phase 20.2 native chrome primitive layer (`src/shell/primitives.rs`): divider, focus ring, panel chrome, scroll chrome, badge, kbd hint, icon slot, tooltip shell; token mapping, interaction states, accessibility roles, and the conformance contract.
- [Clay Shell and Package UI/Layout Strategy](primitives/shell-layout-strategy.md) — shell vocabulary, working area, pane/split tree, fixed/transient slots, package UI/state/style contract, and the Masonry implementation boundary.
- [Creating Clay Packages](packages/creating-packages.md) — package authoring guide. The Components section and the UI and Layout Model section define the package-facing authoring contract; the Styling and Themes section and the Phase 20.1/20.4/20.7 authoring contracts define token/theme usage. Implemented-vs-planned markers in the guide match the component catalog exactly.

## Conformance Rules (Phase 20.7)

Clay is the host authority for UI conformance. Validation runs inside Clay's Rust host validator at parse/install/theme-apply time; no package-facing op or facade exposes it. Third-party packages physically cannot inject raw styling, undocumented components/tokens, oversized UI payloads, or sub-contrast themes.

- **Contrast / legibility:** active-theme status-chrome token pairs must meet `TEXT_CONTRAST_MIN` (4.5) for text and `UI_CONTRAST_MIN` (3.0) for accent/border/focus UI pairs (`validate_active_theme_contrast`, `src/shell/theme.rs`; `enforce_contrast`, `src/server/ops/theme.rs`). A below-AA theme is not activated.
- **State-completeness:** `applicable_states(kind)` (`src/shell/components.rs`) is the per-`ComponentKind` interaction-state contract; the SDUI paint path renders every applicable state from tokens (`component_state_palette`).
- **Payload budgets:** SDUI snapshot ≤ 4096 B, update ≤ 1024 B; runtime `publishTree` tree ≤ 16 KiB / ≤ 128 nodes / ≤ 16 depth / ≤ 4096-char text node (`src/packages/record.rs`, `src/server/ui.rs`, `src/server/ops/sdui.rs`).
- **Code-vs-catalog drift:** the `ComponentKind` enum, typed style variables, and `core_theme_value` arms stay in sync with the catalog tables in `components.md` / `tokens.md` (enforced by `tests/package_ui_conformance.rs`).
- **Author diagnostics:** rejection messages name the rejected value, expected token type, and offending field via `ComponentCatalogError::reject`.
- **Trust domains:** third-party raw values and oversized payloads are rejected at `assemble_package_record` without reaching the trusted runtime; no conformance op or `clay:*` facade is exposed.

See the [Phase 20.7 wiki page](../wiki/modules/phase20.7-package-ui-conformance-and-aesthetic-guardrails.md) for the full implementation detail.

## Agent and Plan Conventions

Agents and plan documents that touch app UI must follow the create-plan UI requirements, which route through this page and the `clay-ui` catalog before proposing new UI code.

- [Create-plan UI requirements](../../.agents/skills/create-plan/references/clay.md) — the Clay UI Primitives-First Task, package UI/layout authoring contract task, and the catalog files every UI plan must read first.

## Rules for Changing the UI Surface

1. Reuse cataloged components, primitives, style variables, and tokens first; a custom component outside the catalog requires explicit justification.
2. New components, primitives, tokens, and style variables are additive-only and token-driven (no raw colors, CSS, concrete font families, or point sizes).
3. Every new component ships state-complete (all applicable `InteractionState` variants styled from tokens) and accessible.
4. Update the component catalog, the token catalog, `docs/reference/packages/creating-packages.md`, and the documentation-drift tests in the same change. Documentation drift fails `cargo test`.