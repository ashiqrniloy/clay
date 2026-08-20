# UI Components, Tokens, and Conformance

Navigation and contract entry for Clay's reusable UI surface. This page links the authoritative catalogs and the rules for using them so any agent or developer can discover every reusable UI primitive/component and the conformance rules without reading paint code. It is maintained by Phase 20.8 and updated whenever the component catalog, token catalog, or conformance rules change.

## Single Source of Truth

The `clay-ui` skill references are the authoritative catalog. This page links them; it does not duplicate them. When a UI phase adds, removes, or changes a component, primitive, style variable, token, or layout rule, the phase updates the catalog in the same change and this page stays a navigation entry.

- [Component catalog](../../.agents/skills/clay-ui/references/components.md) — every package-facing `ComponentKind`, typed style variable, Clay-native surface, chrome primitive, planned component, typography variant, and the rules for adding components. Status legend: **implemented** / **reserved** / **planned** / **internal**.
- [Token catalog](../../.agents/skills/clay-ui/references/tokens.md) — the ten typed token domains, every implemented core token, typography hierarchy, package token contributions, and the rules for adding tokens.

## Plan 087 package authoring boundary

Plan 087 changed Clay-owned native presentation without adding a package-facing
component kind, token, style variable, overlay anchor, manifest field, or JS
API. There is no new package-facing component kind, token, style variable, or
anchor; there is no package-facing anchor for the Plan 087 internal surfaces.
The package guide remains the authoring contract; these internal surfaces
are cataloged here so package authors do not mistake them for extension points:

- **Welcome entry surface:** `WelcomeWidget` is Clay-owned empty/local-fallback
  presentation. It exposes existing file/folder command routes only; packages
  cannot replace it or gain dialog authority.
- **Completion:** `TransientMenuOrigin::Completion` is a Clay-owned modeless
  caret/IME projection with an 8 visible-row and 480 logical-pixel cap,
  retained scrolling, stale/empty/error dismissal, and sanitized status/a11y
  data. Completion is not a package overlay anchor or component kind.
- **Command Centre/Path Browser:** `TransientMenuOrigin::Centered` is a
  Clay-owned modal window-level surface using the token-backed centered width
  (640 logical-pixel default), retained result scrolling, and a single scrim.
  Package commands may be listed but packages cannot open, drive, configure, or
  intercept the session. `centered` is not a package anchor.
- **Package overlays:** package declarations remain limited to
  `working-area`, `active-pane`, `main`, and `pointer`; no package JavaScript
  runs in paint/layout/input paths. Package-authored transient-menu labels are
  normalized once at the host boundary and remain within the 256-character
  accessibility ceiling.

The retained package/SDUI hosts now clip scroll-child rendering and expose
clipped-child semantics to accessibility consumers, closing host-only follow-up
`P1-087-UI-1` without changing the package contract.

## Plan 088 UI modernization package contract

Plan 088 consumes the existing catalog and adds no package-facing kind, style
variable, token, overlay anchor, manifest field, permission, or JavaScript API.
The package guide contains the full authoring contract; this navigation page
records the boundaries that are easiest to confuse with package extension
points:

- Clay owns the working area, pane/split tree, fixed slots, tab bar, status
  chrome, welcome surface, file browser, completion projection, and centered
  Command Centre. Packages contribute inert component trees, action intents,
  input/state metadata, and typed semantic tokens only.
- Retained package/SDUI hosts clip children to their owning bounds and expose
  clipped-child accessibility semantics. A nested `scroll` component receives
  bounded flex space inside panels; `modal` Escape routes its declared inert
  `PackageModalDismiss` intent; `statusItem` and disabled controls expose their
  AccessKit state.
- Responsive slot yielding, label clipping, path sanitization, focus
  containment, and active user typography propagation remain Clay-native
  layout/render responsibilities. Package overlays remain limited to
  `working-area`, `active-pane`, `main`, and `pointer`; `completion` and
  `centered` are internal origins.
- Existing typed tokens and cached `ResolvedUiTheme`/typography metrics are
  reused. Packages cannot declare breakpoints, concrete fonts/sizes, raw
  CSS/colors, native widgets, renderer callbacks, client JavaScript, or direct
  Masonry mutation. `table` remains reserved.

See [Creating Clay Packages — Plan 088 UI modernization authoring contract](packages/creating-packages.md#plan-088-ui-modernization-authoring-contract)
and the [token catalog](../../.agents/skills/clay-ui/references/tokens.md#plan-088-token-consumption-no-additions).

## Phase 28 editor-intelligence chrome

Phase 28 keeps editor intelligence outside the package component catalog:

- Packages publish validated folding ranges with `render-folding`; Clay paints
  gutter chevrons, hides collapsed lines, and owns `editor.clientToggleFold`.
- Packages publish Link targets and inert InlayHint labels through the existing
  decoration transport with `render-decorations`; Clay owns decoration
  hit-testing, `paint_tooltip_shell`, link activation, and the no-reflow inlay
  overlay.
- Link activation is a typed decoration intent, not a package callback or a
  browse/filesystem grant. HTTP/absolute/traversal targets remain display-only
  or denied. Inlay labels are decorative and `aria-hidden`.
- No new `ComponentKind`, token, style variable, package overlay anchor, raw
  renderer, or client-side JavaScript path was added. Paint/layout/pointer
  paths read cached inert data only.

See [Creating Clay Packages — Phase 28 authoring contract](packages/creating-packages.md#phase-28-authoring-contract-editor-commands-folding-decoration-intent-and-inlay-hints)
and the [UI Chrome Primitives](primitives/ui-chrome-primitives.md) reference.

## Reference Documents

- [UI Chrome Primitives](primitives/ui-chrome-primitives.md) — Phase 20.2 native chrome primitive layer (`src/shell/primitives.rs`): divider, focus ring, panel chrome, scroll chrome, badge, kbd hint, icon slot, tooltip shell, and the Phase 24.4 token-driven scrim; token mapping, interaction states, accessibility roles, and the conformance contract.
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
4. Update the component catalog, the token catalog when token entries change, `docs/reference/packages/creating-packages.md`, and the documentation-drift tests in the same change. Plan 087 also records Clay-owned welcome/completion/centered surfaces and keeps them out of package-facing anchor enums. Documentation drift fails `cargo test`.