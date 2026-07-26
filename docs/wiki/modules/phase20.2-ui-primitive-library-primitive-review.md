# Phase 20.2 UI Primitive Library Primitive Review

## Source

- `src/shell/theme.rs`
- `src/shell/components.rs`
- `src/shell/layout.rs`
- `src/shell/package_ui.rs`
- `src/shell/transient_menu.rs`
- `src/editor/theme.rs`
- `src/editor/typography.rs`
- `src/editor/surface.rs`
- `src/masonry_sdui.rs`
- `src/masonry_shell.rs`
- `docs/reference/primitives/typography.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `.agents/skills/clay-ui/references/components.md`
- `.agents/skills/clay-ui/references/tokens.md`
- `tests/primitives_docs.rs`
- `tests/editor_performance_invariants.rs`

## Overview

Phase 20.2 builds the Clay-owned native chrome primitive layer that every current paint path (core and package-contributed) composes from. It does not add a design-system crate, a second theme registry, a custom component, a new rendering path, or a package-facing `ComponentKind`. Existing package component kinds and style-variable names remain unchanged.

The reusable flow stays:

```text
active theme + core fallbacks -> ResolvedUiTheme -> cached Masonry/SDUI reads
ActiveTypography + UiTypographyHierarchy -> TypographyRegistry -> cached Parley/UI metrics
package semantic token -> same-typed core fallback -> resolved component style
```

Phase 20.2 adds a small `pub(crate)` paint-primitive module (`src/shell/primitives.rs`) that reads cached resolved token values and paints chrome (dividers, focus rings, panel chrome, scroll chrome, badges, `kbd` hints, icon slots, tooltip shells). Existing one-off chrome draws in `src/masonry_sdui.rs`, `src/editor/surface.rs`, `src/shell/package_ui.rs`, and `src/shell/transient_menu.rs` route through these primitives. Package-declared components map onto primitives by construction because the SDUI paint path calls the primitive helpers.

## Existing Primitive Inventory

| Primitive | Current owner and behavior | Reuse decision |
| --- | --- | --- |
| `ThemeTokenType` | `src/shell/theme.rs`; ten domains: `color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, `density`. | Reuse unchanged. Primitives read resolved values from these domains. |
| `ResolvedThemeValue` / `ThemeTokenResolver` | `src/shell/theme.rs`; resolves 73 core tokens or package-prefixed aliases through exact same-type core fallbacks. | Keep one resolver. Primitives call `ResolvedUiTheme` accessors (`color`, `scalar_f64`, `opacity`, `dimension`, `elevation`, `motion_duration`, `z_level`, `density`, `panel_defaults`). |
| `ResolvedUiTheme` | `src/shell/theme.rs`; cached active-theme overlay plus core fallbacks. Installed atomically with `StyleRegistry`. | Primitives read cached typed values via accessors. No parsing or allocation in paint. |
| `SduiThemeStyle` | `src/shell/theme.rs`; projects selected resolved values (`panel_padding`, `title_text`/`body_text`/`status_text`, `panel_background`/`button_background`/`list_background`/`selected_background`/`text_color`/`muted_text_color`) for native SDUI paint. | Reuse for SDUI chrome. Primitives may read additional resolved values directly from `ResolvedUiTheme`. |
| `PanelDefaults` | `src/shell/theme.rs`; resolved panel/sidebar geometry (`side_default`/`side_min`/`side_max`, `vertical_default`/`vertical_min`/`vertical_max`, `sidebar_width`) from `dimension.*` tokens. | Reuse for panel chrome sizing. Primitives read `panel_defaults()` for panel title row, collapse affordance, resize handle chrome. |
| `StyleRegistry` | `src/editor/theme.rs`; immutable client color/text-attribute authority for 13 base UI keys, 35 `TokenType` entries, diagnostics, search, and editor chrome. | Preserve as editor color/text-attribute authority. Editor text/caret/selection/diagnostics paint stays on `StyleRegistry`; shell chrome (scrollbar, status bar) routes through primitives. |
| `TypographyRegistry` | `src/editor/typography.rs`; validates/resolves three profiles once, appends generic fallbacks, installs only newer revisions, and serves cached Parley stacks/sizes and shared geometry. | Reuse for text-bearing primitives (badge, `kbd` hint). Primitives read `UiTextVariant` metrics from `TypographyRegistry`. |
| `UiTextVariant` | `src/editor/typography.rs`; seven semantic variants (`Display`, `Title`, `Section`, `Body`, `Status`, `Detail`, `Caption`) resolve from a selected semantic `FontRole`. | Reuse for badge/`kbd` text. Primitives select `Body`, `Detail`, or `Caption` variants. |
| `ComponentKind` / style variables | `src/shell/components.rs`; 11 implemented kinds (`editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, `statusItem`), four reserved kinds (`table`, `dropdown`, `collapse`, `modal`), token-typed style variables, closed enum variants, semantic `fontRole`, and raw-style rejection. | Reuse unchanged. No new component kind in Phase 20.2. Primitives are `pub(crate)` paint helpers, not package-facing components. |
| `PaneSlotLayout` / `FixedSlotState` | `src/shell/layout.rs`; mandatory `main`, optional fixed `left`/`right`/`top`/`bottom` slots, finite ordered bounds, clamping, visibility/collapse, and deterministic geometry. | Reuse for panel chrome geometry. Primitives paint chrome around slot rectangles; they do not mutate slot state. |
| `PackageUiRuntimeState` | `src/shell/package_ui.rs`; accepted fixed panels and transient overlays composed into shell-owned slot geometry. | Reuse for package panel/overlay chrome. Primitives paint chrome around `FixedPackagePanel` and `TransientPackageOverlay` rectangles. |
| `TransientMenuSession` | `src/shell/transient_menu.rs`; bounded prompt/query/item list, selection index, status text, focus policy, accessibility labels, and inert activation actions. | Reuse for transient menu chrome. Primitives paint bottom-pane prompt chrome, completion pop-up chrome around `TransientMenuSession` rectangles. |
| Structural observations | `WorkingAreaLayoutObservation`, package panel/overlay observations, `SduiObservableSnapshot`, and `SduiStatusObservation`. | Reuse for geometry, accessibility, compatibility, and update-count assertions; do not expose raw theme maps or native handles. |

## Reusable Capability Before New Code

Current primitives already provide:

- typed token resolution across ten domains with same-type core fallbacks;
- cached `ResolvedUiTheme` and `SduiThemeStyle` for SDUI paint;
- cached `TypographyRegistry` and `UiTextVariant` metrics for text-bearing chrome;
- `PanelDefaults` for panel/sidebar geometry from resolved dimension tokens;
- `StyleRegistry` for editor text/caret/selection/diagnostics color;
- Clay-owned pane/slot geometry with min/default/max validation and fixed/transient separation;
- implemented package components sufficient for this foundation phase;
- bounded inert protocol/package records and structural regression observations;
- existing source guard (`style_registry_is_single_source_of_color_for_paint_paths` in `tests/editor_performance_invariants.rs:510-560`) covering `masonry_sdui.rs`, `masonry_shell.rs`, and editor files.

No new component kind, custom Masonry widget, renderer callback, theme selector, typography setter, or layout model is needed for Phase 20.2.

## Locked Generic Phase 20.2 Gaps

1. **Native chrome primitive module.** Add `src/shell/primitives.rs` with `pub(crate)` paint helpers for divider/separator, focus ring, panel chrome (title row, collapse affordance, resize handle chrome), scroll chrome (track + thumb), badge/tag, `kbd` hint, icon slot, and tooltip shell. Each primitive reads only cached resolved token values from `ResolvedUiTheme`/`SduiThemeStyle`/`ActiveTypography`; none parse theme, run JS, or hit IPC.

2. **Interaction state completeness.** Interactive primitives (panel collapse affordance, resize handle chrome, scroll thumb, badge when clickable, icon slot when clickable) render `hover`/`active`/`focus`/`disabled` from state color roles (`surface.hover`/`surface.active`/`surface.disabled`, `text.disabled`, `opacity.disabled`). Add a small `InteractionState` enum (`Rest`/`Hover`/`Active`/`Focus`/`Disabled`).

3. **Accessibility role contract.** Each primitive carries a Masonry accessibility role/label contract (separator, focusable, button, scrollbar, status, image, tooltip) consistent with existing editor accessibility. Primitives emit AccessKit nodes with stable roles/labels during Masonry accessibility passes.

4. **Token-driven paint.** Primitives consume tokens only: colors from `ResolvedUiTheme`, spacing/radius/dimension from resolved tokens, typography from `UiTextVariant` + `FontRole`, elevation from `elevation.*`, z-level from `z.*` where stacking is needed. Additive-only token additions (if any) with same-typed core fallbacks; no existing token renames.

5. **Route existing paint paths.** Replace one-off chrome draws in `src/masonry_sdui.rs` (SDUI panel/container/scroll chrome, sidebar/`SIDEBAR_WIDTH`-derived chrome), `src/editor/surface.rs` (scrollbar, status bar, and editor chrome that overlaps shell chrome), `src/shell/package_ui.rs` (fixed panel and overlay chrome), and `src/shell/transient_menu.rs` (bottom-pane prompt chrome, completion pop-up chrome) with calls to the new primitives. `SIDEBAR_WIDTH` and the `package_ui.rs` side/vertical defaults continue to resolve through Phase 20.1 `dimension.*` tokens; chrome around them now uses primitives.

6. **Conformance contract.** Add/extend a deterministic test enforcing that primitives are the only way to paint UI chrome: shell/SDUI chrome paint files (`src/shell/primitives.rs`, `src/shell/package_ui.rs`, `src/shell/transient_menu.rs`, `src/shell/file_browser.rs`, `src/masonry_sdui.rs`, `src/masonry_shell.rs`) contain no `Color::from_rgb8`/`Color::from_rgba8` literals and no hardcoded `f64` chrome sizes outside the primitive module and `src/shell/theme.rs` (the token-definition module). Package components map onto primitives by construction — assert `paint_package_component` routes chrome through primitive helpers.

7. **Structural primitive tests.** Add structural tests proving each primitive consumes tokens (no raw values) and renders all interaction states. Primitives panic-free on zero-size rects; `InteractionState::Disabled` applies `opacity.disabled`.

8. **Documentation and coverage.** Update the authoritative token catalog (`.agents/skills/clay-ui/references/tokens.md`), component catalog (`.agents/skills/clay-ui/references/components.md`), package authoring guide (`docs/reference/packages/creating-packages.md`), primitive reference docs (`docs/reference/primitives/`), implementation wiki, and deterministic documentation tests in their assigned later tasks. This review records the boundary; it does not mark planned primitives implemented.

## Primitive Inventory and Token Mapping

| Primitive | Tokens consumed | Interaction states | Accessibility role | Replaced one-off draws |
| --- | --- | --- | --- | --- |
| `paint_divider` | `border.hairline` (color), `dimension.border.hairline` (width) | None (static) | `Role::Separator` | Inline divider draws in `masonry_sdui.rs` panel separators, `package_ui.rs` panel dividers |
| `paint_focus_ring` | `focus.ring` (color), `dimension.border.focus` (width), `radius.xs` (corner radius) | `Focus` only | `Role::Focusable` (implicit via focus state) | Inline focus-ring draws in `masonry_sdui.rs` focused panel/button chrome |
| `paint_panel_chrome` | `surface.panel` (background), `border.subtle` (border), `radius.sm` (corner radius), `spacing.panel` (padding), `typography.title` (title text), `PanelDefaults` (title row height, collapse affordance size, resize handle size) | `Rest`/`Hover`/`Active`/`Focus`/`Disabled` for collapse affordance and resize handle | `Role::Pane` (panel), `Role::Button` (collapse affordance) | Inline panel title row, collapse affordance, resize handle chrome in `masonry_sdui.rs`, `package_ui.rs` |
| `paint_scroll_chrome` | `surface.scrollbar` (thumb), `surface.scrollbar.track` (track), `dimension.scrollbar.width` (thumb width), `radius.xs` (thumb corner radius), `opacity.disabled` (disabled state) | `Rest`/`Hover`/`Active`/`Disabled` for thumb | `Role::ScrollBar` | Inline scrollbar draws in `editor/surface.rs` (`SCROLLBAR_WIDTH`, `SCROLLBAR_MARGIN`, `SCROLLBAR_MIN_THUMB`), `masonry_sdui.rs` SDUI scroll chrome |
| `paint_badge` | `surface.badge` (background), `text.badge` (text color), `radius.xs` (corner radius), `spacing.badge` (padding), `typography.detail` or `typography.caption` (text), `opacity.disabled` (disabled state) | `Rest`/`Hover`/`Active`/`Disabled` when clickable | `Role::Status` | Inline badge/tag draws in `masonry_sdui.rs` status items, `package_ui.rs` status components |
| `paint_kbd_hint` | `surface.kbd` (background), `text.kbd` (text color), `border.kbd` (border), `radius.xs` (corner radius), `dimension.kbd.height` (height), `typography.caption` (text) | None (static) | `Role::Label` (implicit via text) | Inline `kbd` hint draws in `masonry_sdui.rs` keybinding hints, `transient_menu.rs` menu item details |
| `paint_icon_slot` | `text.icon` (glyph color), `dimension.icon.size` (slot size), `opacity.disabled` (disabled state) | `Rest`/`Hover`/`Active`/`Disabled` when clickable | `Role::Image` | Inline icon slot draws in `masonry_sdui.rs` panel icons, `package_ui.rs` component icons |
| `paint_tooltip_shell` | `surface.tooltip` (background), `text.tooltip` (text color), `border.hairline` (border), `radius.sm` (corner radius), `elevation.overlay` (elevation), `z.tooltip` (z-level), `spacing.tooltip` (padding), `typography.body` (text) | None (static shell; content/trigger wiring is Phase 20.5) | `Role::ToolTip` | Inline tooltip shell draws in `masonry_sdui.rs` hover tooltips, `transient_menu.rs` hover documentation |

## Additive Token Additions (Tentative)

Phase 20.2 may add the following tokens additively to `src/shell/theme.rs` `core_theme_value` with same-typed fallbacks. These are tentative and will be confirmed during implementation:

- `dimension.scrollbar.width` (dimension, fallback `8.0`)
- `dimension.icon.size` (dimension, fallback `16.0`)
- `dimension.kbd.height` (dimension, fallback `20.0`)
- `surface.badge` (color-role, fallback `surface.control`)
- `text.badge` (color-role, fallback `text.primary`)
- `spacing.badge` (spacing, fallback `spacing.xs`)
- `surface.kbd` (color-role, fallback `surface.control`)
- `text.kbd` (color-role, fallback `text.muted`)
- `border.kbd` (color-role, fallback `border.subtle`)
- `surface.tooltip` (color-role, fallback `surface.overlay`)
- `text.tooltip` (color-role, fallback `text.primary`)
- `spacing.tooltip` (spacing, fallback `spacing.sm`)

All additions are additive-only with same-typed core fallbacks. No existing token is renamed or repurposed.

## Additive Compatibility Contract

- Existing token names, values, types, component kinds, style-variable names, `fontRole` values, and `UiTextVariant` behavior remain valid.
- Existing `clay.contributions.themeTokens` records remain semantic aliases and require no migration.
- Existing Gruvbox theme manifests remain valid and receive every new UI value through same-typed core fallbacks.
- `StyleRegistry` remains the editor paint-path color authority; `TypographyRegistry` remains the font/geometry authority.
- Missing active-theme UI overrides use core fallbacks. Unknown names, wrong types, partial typography hierarchy, invalid bounds, and oversized payloads fail before client installation.
- Package-facing additions are inert and additive. No raw scalar escape, raw CSS, concrete package font setting, callback, or native handle is introduced.
- No new package-facing `ComponentKind` is added. Primitives are `pub(crate)` paint helpers, not package-declarable components.

## Hot-Path Boundary

Allowed at package/configuration load, runtime reload, bootstrap, or explicit settings change:

- manifest and JSON parsing;
- package prefix/provenance and same-type fallback validation;
- active-theme overlay construction;
- `BTreeMap` construction;
- typography family parsing/fallback insertion;
- protocol candidate validation and atomic registry installation.

Forbidden in Masonry paint/input/layout/pointer/scroll/key/text-event handlers and accessibility traversal:

- package JavaScript or configuration evaluation;
- package/theme validation or map construction;
- IPC waits or server round trips;
- font resolution/discovery;
- filesystem, network, shell, package-manager, AI, or WASM work;
- repeated token string parsing.

Hot paths may perform bounded reads from installed `StyleRegistry`, `ResolvedUiTheme`, `TypographyRegistry`, `PaneSlotLayout`, and inert package UI state. Primitive paint is O(1) per call, runs inside existing paint passes, and adds no per-frame theme resolution, no allocations on the hot path, and no package JavaScript.

## Security Boundary

Clay remains owner of Masonry widgets, Vello painting, Parley shaping, working areas, pane/split trees, slots, component catalog, active-theme installation, typography resolution, and final layout geometry. Server/package boundaries validate bounded inert declarations before client publication; client installation revalidates protocol candidates where applicable.

Theme/token/typography declarations grant no filesystem, network, shell, package-manager, AI, WASM, raw `Deno.core.ops`, native-widget, renderer-callback, client-JavaScript, document, workspace, persistence, font-download, or external-process authority. Ordinary packages may declare package-prefixed aliases, but only the existing first-party `setTheme` selection boundary may install theme-package concrete `designTokens` until a later approved authority model says otherwise.

Primitives are `pub(crate)` inert paint helpers with no package-facing surface. Packages continue to declare inert components only; they cannot call primitives directly.

## Phase Boundary

- **Phase 20.1:** token domains/catalog/defaults, active-theme UI values, semantic typography hierarchy, token-backed panel/density defaults, compatibility/docs/tests. (Complete.)
- **Phase 20.2:** native divider, focus-ring, panel chrome, resize-handle, scroll chrome, badge, `kbd`, icon, and tooltip primitives. (This phase.)
- **Phase 20.3:** user-facing split/panel resizing, collapse/restore, persistence, and inert package layout intents.
- **Phase 20.4:** component visual uplift and interaction-state rendering without component-kind/style-variable schema changes.
- **Phase 20.5:** reserved overlay/menu/input component kinds and interaction behavior.
- **Phase 20.6:** Modus Operandi/Vivendi packages, light/dark selection semantics, and theme/font settings UI.
- **Phases 20.7–20.8:** conformance enforcement and continuing reference/catalog maintenance.

Phase 20.2 must not implement those deferred primitives/components, restyle every component, ship Modus themes, or add resize/persistence behavior. Resize handle and collapse affordance paint chrome only — drag/resize behavior is Phase 20.3. Tooltip shell paints the anchored box; tooltip content/trigger wiring is Phase 20.5.

## Tests

- `tests/primitives_docs.rs::phase20_2_ui_primitive_library_primitive_review_is_linked_and_complete`: locks inventory, reuse, generic gaps, additive compatibility, hot-path/security boundaries, and phase ownership.
- Existing `src/shell/theme.rs`, `src/shell/components.rs`, `src/editor/typography.rs`, `src/shell/layout.rs`, `src/shell/package_ui.rs`, `src/masonry_sdui.rs`, theme-package, typography-protocol, payload-budget, accessibility, and editor source-guard tests provide the executable baseline.
- Phase 20.2 adds:
  - Unit/structural tests: each primitive consumes tokens (no raw values) and renders all applicable `InteractionState`s; primitives panic-free on zero-size rects; `InteractionState::Disabled` applies `opacity.disabled`.
  - Token-guard test: new tokens (if any) are additive with same-typed fallback and within domain bounds; no existing token renamed/repurposed.
  - Conformance guard: shell chrome paint files contain no color literals and no bare `f64` chrome-size constants outside `primitives.rs`/`theme.rs`; `paint_package_component` references primitive helpers.
  - Structural observability: SDUI/package UI/editor chrome structural snapshots remain equivalent (within token tolerance) before and after routing.
  - Hot-path invariant: no package JavaScript, IPC, or theme re-resolution in paint handlers (existing `editor_performance_invariants` guards extended to routed shell files).

```bash
cargo test --test protocol primitives_docs::phase20_2_ui_primitive_library_primitive_review_is_linked_and_complete
cargo test --test protocol primitives_docs::wiki_index_links_every_wiki_page
cargo test --test editor editor_performance_invariants::style_registry_is_single_source_of_color_for_paint_paths
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Phase 20.1 UI Design Language Primitive Review](phase20.1-ui-design-language-primitive-review.md)
- [Semantic Typography Roles](../../reference/primitives/typography.md)
- [Clay Shell and Package UI/Layout Strategy](../../reference/primitives/shell-layout-strategy.md)