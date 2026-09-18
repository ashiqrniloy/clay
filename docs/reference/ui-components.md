# UI Components, Tokens, and Conformance

Navigation and contract entry for Clay's reusable UI surface. This page links the authoritative catalogs and the rules for using them so any agent or developer can discover every reusable UI primitive/component and the conformance rules without reading paint code. It is maintained by Phase 20.8 and updated whenever the component catalog, token catalog, or conformance rules change.

## Design Language (Quiet Instrument)

[`DESIGN.md`](../../DESIGN.md) is the normative design system: the five laws
(one surface with hairline zones, two elevations, accent is state, type does
the structure, motion carries meaning), the value profile (radius ladder
5/8/12/16/pill, 1px hairlines, veil/accent opacities, the two shadow recipes,
150/240/620ms motion), geometry and reading measures, the per-surface recipe
binding for every component kind, shell composition rules, accessibility
invariants, the retired patterns, and the conformance checklist.

This page and the catalogs remain the entry points for *what exists*; `DESIGN.md`
owns *how it looks*. A change to appearance is a `DESIGN.md` edit plus
design-system package data — never a host CSS module, a component rewrite, or a
new token. A change to a component, primitive, style variable, or token updates
this navigation page, the catalogs, and the drift tests together.

## Single Source of Truth

The `clay-execution` catalog references are the authoritative catalog. This page links them; it does not duplicate them. When a UI phase adds, removes, or changes a component, primitive, style variable, token, or layout rule, the phase updates the catalog in the same change and this page stays a navigation entry.

- [Component catalog](../../.agents/skills/clay-execution/references/components.md) — every package-facing `ComponentKind`, typed style variable, Clay-native surface, chrome primitive, planned component, typography variant, and the rules for adding components. Status legend: **implemented** / **reserved** / **planned** / **internal**.
- [Token catalog](../../.agents/skills/clay-execution/references/tokens.md) — the ten typed token domains, every implemented core token, typography hierarchy, package token contributions, and the rules for adding tokens.

## Plan 087 package authoring boundary

Plan 087 changed Clay-owned native presentation without adding a package-facing
component kind, token, style variable, overlay anchor, manifest field, or JS
API. There is no new package-facing component kind, token, style variable, or
anchor; there is no package-facing anchor for the Plan 087 internal surfaces.
The package guide remains the authoring contract; these internal surfaces
are cataloged here so package authors do not mistake them for extension points:

- **Welcome entry surface:** `WelcomeWidget` is Clay-owned empty/local-fallback
  presentation when no `empty-tab` pane-content is loaded. It exposes existing
  file/folder command routes only; packages cannot replace this fallback or
  gain dialog authority. The empty-tab landing is package pane-content and no
  longer ships a product-named core branch: plan 118 deleted the `@clay/chat`
  landing, and the approved target information architecture makes the
  **launcher** the landing surface with the agent as a first-class view of the
  same tab (plan 118 Part D,
  `design-artifacts/approved/quiet-instrument-migration/start.html`).
- **Completion:** `TransientMenuOrigin::Completion` is a Clay-owned modeless
  caret/IME projection with an 8 visible-row and 480 logical-pixel cap,
  retained scrolling, stale/empty/error dismissal, and sanitized status/a11y
  data. Completion is not a package overlay anchor or component kind.
- **Command palette and picker stages (one composer-anchored surface):** since
  plan 125 the composer-anchored palette is the only transient selection surface
  (`TransientMenuOrigin::CommandPalette`). The sheet is the field's menu, 6
  logical pixels above the box and as wide as the box, veiled by the
  `modal.scrim` recipe over the working area through the inspector rail's full
  height while the agent lane — which holds the query — stays interactive above
  it, and it carries the design language's halo instead of a drop shadow. The
  session's `mode` field (protocol v32) selects what the sheet renders:
  `catalogue` (the generation-stamped command catalogue, opened by `/`),
  `path` (the Path Browser, also `/`), and the picker stages `picker`,
  `secret`, `url`, and `oauth` (no sigil). The `secret` stage is shielded — the
  credential is typed into a `type="password"` field inside the sheet, masked in
  the server's snapshot, and delivered only through the host's credential path,
  so it never reaches the composer's draft or the persistence layer.
  `TransientMenuOrigin::Centered` and its window-level sheet are **retired** by
  plan 125: the wire value still decodes for older frames and the core
  token `dimension.overlay.centered.width` still sizes package `modal`
  dialogs, but no live producer emits `centered` and it was never a package
  anchor. Package commands may be listed behind `/` but packages cannot open,
  drive, filter, configure, or intercept a palette session, cannot request the
  veil, and cannot claim the shielded stage.
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
  chrome, the persistent agent lane, the composer palette (the single transient
  selection surface since plan 125, which retired the centered sheet), core
  welcome fallback, file browser, and completion projection. The
  loaded empty-tab landing is package pane-content
  (whatever package contributes the empty-tab landing). Packages contribute inert component trees, action
  intents, input/state metadata, and typed semantic tokens only.
- Retained package/SDUI hosts clip children to their owning bounds and expose
  clipped-child accessibility semantics. A nested `scroll` component receives
  bounded flex space inside panels; `modal` Escape routes its declared inert
  `PackageModalDismiss` intent; `statusItem` and disabled controls expose their
  AT-SPI state.
- Responsive slot yielding, label clipping, path sanitization, focus
  containment, and active user typography propagation remain Clay-native
  layout/render responsibilities. Package overlays remain limited to
  `working-area`, `active-pane`, `main`, and `pointer`; `completion` and
  `centered` are internal origins — `completion` live, `centered` decode-only
  since plan 125 retired it — and neither is a package anchor.
- Existing typed tokens and cached `ResolvedUiTheme`/typography metrics are
  reused. Packages cannot declare breakpoints, concrete fonts/sizes, raw
  CSS/colors, native widgets, renderer callbacks, client JavaScript, or direct
  renderer mutation. `table` remains reserved.

See [Creating Clay Packages — Plan 088 UI modernization authoring contract](packages/creating-packages.md#plan-088-ui-modernization-authoring-contract)
and the [token catalog](../../.agents/skills/clay-execution/references/tokens.md#plan-088-token-consumption-no-additions).

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

## Plan 110 unified tab-strip primitive and design-system selector

- **`ClayTabStrip` catalog primitive** (`frontend/src/components/tab-strip.tsx`, plan 108 G2 + plan 110 task 5): one React Aria `Tabs` implementation shared by the shell window tab bar (`frontend/src/app/layout/tab-bar.tsx`), the SDUI `PackageTabList` registry entry, and `CodingAgentPanel`. Closed recipe attributes (`tabList.root`/`strip`/`tab`/`panel`); visual styling flows through the `tab.default.item.*` and `tabBar.default.root.rest` design-system recipes. Packages rendering tab UI must use the `tabList` SDUI kind — never custom tab strips.
- **Design-system selector UI** (plan 110 task 10): the Settings panel renders *Theme* and *Design system* dropdowns from the server-enumerated `ui_choices` snapshot (`themes` from installed `@clay/theme-*` records, `design_systems` with Core baseline first), plus an *Appearance* selector hydrated from the persisted preference. Switching sends `settings.setDesignSystem`, which persists the choice and reloads the runtime — whole-shell restyle with no component remount. See [`settings.setDesignSystem`](clay-js-api/settings/set-design-system.md) and [`theme.setDesignSystem`](clay-js-api/theme/set-design-system.md).
- **Authoring contract:** design-system package authors document keys against the canonical slot list and consumption-tested contract in [Creating Clay Packages — UI Design-System declarations](packages/creating-packages.md#ui-design-system-declarations-claycontributionsuidesignsystem-plans-101104).

## Plan 112 configurable icon packs and shared icon rendering

- **`ClayIcon` shared icon renderer** (`frontend/src/components/icon.tsx`, plan 112 task 7): the React projection of the `paint_icon_slot` contract — bounded, host-validated vector geometry from the active icon pack (`frontend/src/state/icon-store.ts`, fed by the `activeIconPack` runtime snapshot), falling back to the bundled Regular subset (`frontend/src/icons/fallback.generated.ts`) with zero configuration. Inline `currentColor` SVG sized by `dimension.icon.size` and colored by `text.icon`; `aria-hidden` by default, `role="img"` with a single accessible name only when a label is passed. Unknown keys render an empty decorative slot — never a broken glyph, never package SVG/CSS.
- **`ClayIconButton` composition** (`frontend/src/components/button.tsx`, plan 112 task 7): icon-only controls require a `label` prop (the single accessible name), show a tooltip on hover **and** keyboard focus via `ClayTooltip`, keep a ≥24px CSS hit target around the 16px glyph (WCAG 2.2), and fall back to the visible text label when the icon key resolves to no geometry — a failed pack never leaves a blank control.
- **`ClayTooltip`** (`frontend/src/components/tooltip.tsx`, plan 112 task 7): React Aria `TooltipTrigger`/`Tooltip` with host-owned string content only; consumes the `tooltip.default.root.rest` design-system recipe. See the [React UI catalog mapping](../development/react-ui-catalog-mapping.md) for the implementation table.
- **Icon-pack activation API** (plan 112 task 12): [`theme.setIconPack`](clay-js-api/theme/set-icon-pack.md) activates a bundled `@clay/icons-*` pack or an enabled package's `iconPack` contribution; selection is independent of theme/appearance/design-system selections and re-validated at every commit. Package authors: see [Icon Packs](icon-packs.md) and [Creating Clay Packages — Icon-pack declarations](packages/creating-packages.md#icon-pack-declarations-claycontributionsiconpack-plan-112).

## Plan 124 agent lane and composer `/` palette

The persistent agent lane and the palette it answers to add no package-facing
kind, style variable, token, overlay anchor, manifest field, permission, or JS
API. Recorded here because both are easy to mistake for extension points:

- **Agent lane** (`frontend/src/shell/AgentLane.tsx`): the view pane's own
  bottom chrome row — approval strip, composer box (prompt field plus the tab's
  agent-type/model/effort controls), hint row, session-environment foot — shared
  by the Workspace and Agent views of the same tab, toggled by
  `shell.toggleAgentLane`, its per-tab visibility persisted by the client. It is
  not a `PanelContribution` slot, a `ComponentKind`, a `TransientMenuOrigin`, or
  an `OverlayAnchor`; the pane `bottom` slot still composes inside its pane,
  above the lane. Clay-owned presentation over the documented agent session APIs.
  Plan 125 confined it to the view pane; plan 126 made the workspace sidebar a rail of the same working-area grid (the tree's token-sized region is host-placed, so the sidebar runs the full height beside the lane): the workspace sidebar (a split panel of
  the working area) and the inspector rail (a grid column) both run the full
  working-area height, and the lane stops at the rail's inner edge, taking the
  full width only while the rail is collapsed.
- **`/` palette** (`frontend/src/command-centre/CommandPalette.tsx`): the
  generation-stamped command catalogue drawn as the composer field's own menu
  (`TransientMenuOrigin::CommandPalette`), 6 logical pixels above the box and as
  wide as it, with the `modal.scrim` veil over the working area (never over the
  lane — the field that holds the query stays interactive). Rows carry the
  server's own scope and chord fields (`scope`, `bindings`); package-registered
  commands appear automatically, routing-policy-filtered, with no authority.
  `commandPalette` is not a package anchor, and no package API opens or drives
  the session.

### Plan 125 continuation: one sheet for every picker, and the halo

Plan 125 finishes the boundary plan 124 opened: the retired centered sheet's
hosts move onto the palette, and no package-facing vocabulary grows a centered
option.

- **Every picker is a palette session.** The agent picker's list, provider
  setup, auth-method, secret, URL, and OAuth stages are the same
  `TransientMenuSession` round trip as the catalogue, distinguished by the
  session's `mode` (protocol v32: `catalogue`, `path`, `picker`, `secret`,
  `url`, `oauth`, bounded to 16 characters). The server renders the rows, the
  prompts, and the flow (backspace on an empty query ascends a stage and closes
  the session at its entry), so a package sees exactly the same surface it
  already saw: none.
- **The shielded stage is Clay's, and only Clay's.** `secret` mode is the one
  stage whose input lives inside the sheet (`type="password"`, composer field
  disabled); the value is masked in every snapshot, flushed on `Enter`, and
  transmitted only through the host credential path. No package can request,
  style, or claim that stage, and no package API can read its value.
- **The halo replaces the drop shadow** on the palette and the `@` mentions
  menu: two zero-offset layers derived from `text.primary` (`0 0 14px -2px` at
  14% and `0 0 3px 0` at 8%), with the border retained. Standard dropdowns and
  other popovers keep their `shadow.pop` recipe; the halo covers only these two
  surfaces (`DESIGN.md` §6).
- **Package UI dialogs, `modal`, and the token.** `PackageOverlayAnchor` gains
  no option: `package_ui.rs` no longer has a centered anchor at all, and the
  remaining internal anchor for package overlays is the work-area/bottom shape.
  `dimension.overlay.centered.width` stays a core theme token for package
  `modal` dialogs (it sizes their `max-width`), not for any Clay transient
  surface.

Complete package-facing explanation:
[Creating Clay Packages — Plan 124 authoring contract](packages/creating-packages.md#plan-124-authoring-contract-the-persistent-agent-lane-and-the-composers--palette)
and the normative shell composition in [`DESIGN.md` §12](../../DESIGN.md).

## Reference Documents

- [Clay Design System — Quiet Instrument](../../DESIGN.md) — normative design language: laws, values, geometry, materials, motion, typography, state language, per-surface component recipes, shell composition, accessibility invariants, retired patterns, review checklist, and the design-system implementation profile.
- [UI Design Systems](ui-design-systems.md) — public specification for typed UI design-system recipe contributions, property domains, state mapping, deterministic fallbacks, accessibility layers, the shipped system (`@clay/design-instrument`) plus the `@clay/core` baseline, the composited content-theme contrast gate, and programmatic activation (Plans 101–103, 118).
- [UI Design-System Recipe Matrix](../development/ui-design-system-recipe-matrix.md) — comprehensive matrix of all package component kinds, internal surfaces, chrome primitives, semantic recipe slots, applicable interaction states, allowed property families, layout-neutrality classifications, active-theme color role sources, accessibility invariants, and deterministic fallback resolution. `†` marks a host slot the shipped package declares no recipe for; the markers are drift-tested.
- [UI Chrome Primitives](primitives/ui-chrome-primitives.md) — the chrome primitive catalog (divider, focus ring, panel chrome, scroll chrome, badge, kbd hint, icon slot, tooltip shell, token-driven scrim). The Phase 20.2 native `src/shell/primitives.rs` paint helpers are gone with the native client; the same contract is realized as token-driven React components/CSS classes in `frontend/src/components/chrome.tsx`.
- [Clay Shell and Package UI/Layout Strategy](primitives/shell-layout-strategy.md) — shell vocabulary, working area, pane/split tree, fixed/transient slots, package UI/state/style contract, and the Tauri/React client implementation boundary.
- [Creating Clay Packages](packages/creating-packages.md) — package authoring guide. The Components section and the UI and Layout Model section define the package-facing authoring contract; the Styling and Themes section and the Phase 20.1/20.4/20.7 authoring contracts define token/theme usage. Implemented-vs-planned markers in the guide match the component catalog exactly.

## Conformance Rules (Phase 20.7)

Clay is the host authority for UI conformance. Validation runs inside Clay's Rust host validator at parse/install/theme-apply time; no package-facing op or facade exposes it. Third-party packages physically cannot inject raw styling, undocumented components/tokens, oversized UI payloads, or sub-contrast themes.

- **Contrast / legibility:** active-theme role pairs must meet `TEXT_CONTRAST_MIN` (4.5) for text, `UI_CONTRAST_MIN` (3.0) for structural boundaries/accent/focus/state fills, and `HAIRLINE_VISIBILITY_MIN` (1.2) for the decorative `border.hairline` — each pair measured **composited** (alpha over its backdrop), with the border ladder monotonic on the same surface (`validate_active_theme_contrast`, `src/shell/theme.rs`; `enforce_contrast`, `src/server/ops/theme.rs`). A below-floor theme is not activated and the previous theme stays installed.
- **State-completeness:** `applicable_states(kind)` (`src/shell/components.rs`) is the per-`ComponentKind` interaction-state contract; the SDUI paint path renders every applicable state from tokens (`component_state_palette`).
- **Payload budgets:** SDUI snapshot ≤ 4096 B, update ≤ 1024 B; runtime `publishTree` tree ≤ 16 KiB / ≤ 128 nodes / ≤ 16 depth / ≤ 4096-char text node (`src/packages/record/mod.rs`, `src/server/ui.rs`, `src/server/ops/sdui.rs`).
- **Code-vs-catalog drift:** the `ComponentKind` enum, typed style variables, and `core_theme_value` arms stay in sync with the catalog tables in `components.md` / `tokens.md` (enforced by `tests/package_ui_conformance.rs`).
- **Author diagnostics:** rejection messages name the rejected value, expected token type, and offending field via `ComponentCatalogError::reject`.
- **Trust domains:** third-party raw values and oversized payloads are rejected at `assemble_package_record` without reaching the trusted runtime; no conformance op or `clay:*` facade is exposed.

See the [Phase 20.7 wiki record](../wiki/archive/phase20.7-package-ui-conformance-and-aesthetic-guardrails.md) for the full implementation detail.

## Agent and Plan Conventions

Agents and plan documents that touch app UI must follow the create-plan UI requirements, which route through this page and the `clay-execution` catalog before proposing new UI code.

- [Create-plan UI requirements](../../.agents/skills/create-plan/references/clay.md) — the Clay UI Primitives-First Task, package UI/layout authoring contract task, and the catalog files every UI plan must read first.

## Rules for Changing the UI Surface

1. Reuse cataloged components, primitives, style variables, and tokens first; a custom component outside the catalog requires explicit justification.
2. New components, primitives, tokens, and style variables are additive-only and token-driven (no raw colors, CSS, concrete font families, or point sizes).
3. Every new component ships state-complete (all applicable `InteractionState` variants styled from tokens), accessible, and conformant with the Quiet Instrument binding in `DESIGN.md` §11 and the retired patterns in §14.
4. Update `DESIGN.md` when design-language values or per-surface rules change; update the component catalog, the token catalog when token entries change, `docs/reference/packages/creating-packages.md`, and the documentation-drift tests in the same change. Plans 087, 124, and 125 record the Clay-owned welcome, completion, agent-lane, and composer-palette surfaces and keep them out of package-facing anchor enums — with the centered transient surface retired outright, exactly one transient selection surface exists. Documentation drift fails `cargo test`.