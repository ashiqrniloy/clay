# Phase 20.1 UI Design Language Primitive Review

## Source

- `src/shell/theme.rs`
- `src/shell/components.rs`
- `src/shell/layout.rs`
- `src/shell/package_ui.rs`
- `src/editor/theme.rs`
- `src/editor/typography.rs`
- `src/protocol/mod.rs`
- `src/packages/record.rs`
- `src/server/ui.rs`
- `src/masonry_sdui.rs`
- `docs/reference/primitives/typography.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `.agents/skills/clay-ui/references/components.md`
- `.agents/skills/clay-ui/references/tokens.md`
- `tests/primitives_docs.rs`

## Overview

Phase 20.1 extends Clay's existing theme, typography, component-validation, and shell-layout seams. It does not add a design-system crate, a second theme registry, a custom component, or a new rendering path. Existing package component kinds and style-variable names remain unchanged.

The reusable flow stays:

```text
active theme + core fallbacks -> ResolvedUiTheme -> cached Masonry/SDUI reads
ActiveTypography + UiTypographyHierarchy -> TypographyRegistry -> cached Parley/UI metrics
package semantic token -> same-typed core fallback -> resolved component style
```

## Existing Primitive Inventory

| Primitive | Current owner and behavior | Reuse decision |
| --- | --- | --- |
| `ThemeTokenType` | `src/shell/theme.rs`; closed `color-role`, `spacing`, `radius`, `typography`, and `opacity` categories. | Extend additively with genuinely distinct scalar domains; do not replace or reinterpret existing variants. |
| `ResolvedThemeValue` / `ThemeTokenResolver` | `src/shell/theme.rs`; resolves 21 core tokens or package-prefixed aliases through exact same-type core fallbacks. Package aliases live in a `BTreeMap`. | Keep one resolver and fallback table. Add typed values/accessors there rather than ad-hoc constants or another map. |
| `SduiThemeStyle` | `src/shell/theme.rs`; projects selected resolved values for native SDUI paint. `src/masonry_sdui.rs::sdui_theme_style` currently constructs the default view repeatedly. | Keep the projection concept, but install one complete `ResolvedUiTheme` and let paint/layout read cached fields. |
| `StyleRegistry` | `src/editor/theme.rs`; immutable client color/text-attribute authority for 13 base UI keys, 35 `TokenType` entries, diagnostics, search, and editor chrome. | Preserve as editor color/text-attribute authority. Extend active-theme delivery to install UI design-token values atomically beside it, not inside its typography state. |
| `ActiveTheme` | `src/protocol/mod.rs`; selected theme specifier plus inert `TextThemeOverride` values delivered at bootstrap/reload. | Extend the same snapshot with bounded typed `designTokens`; do not add `setUiTheme` or a second active-theme message. |
| `TypographyRegistry` | `src/editor/typography.rs`; validates/resolves three profiles once, appends generic fallbacks, installs only newer revisions, and serves cached Parley stacks/sizes and shared geometry. | Keep as sole client typography resolver and cache. Variant hierarchy becomes installed state, not theme color state. |
| `ActiveTypography` | `src/protocol/mod.rs`; atomic revisioned `monospace`, `proportional`, and `ui` profiles with bounded family stacks and logical-pixel sizes. | Extend atomically with user-owned `UiTypographyHierarchy`; preserve one snapshot and one invalidation path. |
| `UiTextVariant` | `src/editor/typography.rs`; `Body`, `Status`, `Title`, and `Detail` resolve from a selected semantic `FontRole`. | Add `Display`, `Section`, and `Caption`; preserve existing default scales and role ownership. |
| Component style validation | `src/shell/components.rs`, `src/server/ui.rs`, and `src/packages/record.rs`; 11 implemented kinds, four reserved kinds, token-typed style variables, closed enum variants, semantic `fontRole`, and raw-style rejection. | Reuse unchanged component kinds and style-variable names. New token domains become available to generic validators/accessors only where a documented variable consumes them. |
| Package theme-token declarations | `clay.contributions.themeTokens` and `ui.serverRegisterThemeToken`; package-prefixed semantic aliases with a description and same-typed core fallback, no raw value. | Preserve this alias contract. Theme package `designTokens` are a separate static value-overlay contract and must not let ordinary packages set concrete global values. |
| Panel/slot geometry | `PaneSlotLayout` and `FixedSlotState` in `src/shell/layout.rs`; mandatory `main`, optional fixed sides, finite ordered bounds, clamping, visibility/collapse, and deterministic geometry. | Keep geometry and validation. Replace only independent default sources in `src/shell/package_ui.rs` and `src/masonry_sdui.rs` with one token-resolved layout-default view. |
| Structural observations | `WorkingAreaLayoutObservation`, package panel/overlay observations, `SduiObservableSnapshot`, and `SduiStatusObservation`. | Reuse for geometry, accessibility, compatibility, and update-count assertions; do not expose raw theme maps or native handles. |

## Reusable Capability Before New Code

Current primitives already provide:

- package-prefixed semantic token aliases with same-type core fallbacks;
- typed component style validation and raw CSS/color/concrete-font rejection;
- one active editor theme selection path and one immutable editor paint registry;
- one atomic, revisioned user typography path across editor, SDUI, rows, hit regions, scrolling, and accessibility;
- Clay-owned pane/slot geometry with min/default/max validation and fixed/transient separation;
- implemented package components (`editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, `statusItem`) sufficient for this foundation phase;
- bounded inert protocol/package records and structural regression observations.

No new component kind, custom Masonry widget, renderer callback, theme selector, typography setter, or layout model is needed for Phase 20.1.

## Locked Generic Phase 20.1 Gaps

1. **Typed catalog domains.** Extend `ThemeTokenType`/`ResolvedThemeValue` with `dimension`, `elevation`, `motion-duration`, `z-level`, and `density`. Add the roadmap's semantic color, spacing, radius, border/focus, panel-default, elevation, motion, overlay-level, density, and typography tokens to the central core fallback catalog. Values must have finite, bounded, domain-specific validation.
2. **Resolved active UI theme.** Extend `ActiveTheme` with a bounded inert `designTokens` value overlay and construct one internal `ResolvedUiTheme` from core fallbacks plus active-theme overrides. Existing Gruvbox packages omit `designTokens` and continue through core fallbacks. `StyleRegistry` and `ResolvedUiTheme` install from the same active snapshot; they remain separate views because editor vocabulary styling and generic UI scalar styling have different consumers.
3. **User-owned hierarchy.** Add a complete `UiTypographyHierarchy` to `ActiveTypography`, then expand `UiTextVariant` with `Display`, `Section`, and `Caption`. `TypographyRegistry` resolves scales from that installed hierarchy. Packages continue to select role plus variant only; they cannot provide ratios, families, sizes, paths, URLs, or font bytes.
4. **One layout-default view.** Derive panel side/vertical default, minimum, maximum, and density defaults from `ResolvedUiTheme`. Both package fixed-panel construction and the legacy SDUI left-slot bridge consume that view. `PaneSlotLayout` remains geometry authority; Phase 20.3 owns drag, resize, collapse persistence, and user-sized state.
5. **Cached native reads.** Parse package declarations and active-theme values during load/configuration/reload; validate the protocol candidate before install; build maps and resolve fonts once. Paint, input, layout, pointer, scroll, text-event, hit-test, and accessibility paths read installed native values only.
6. **Documentation and coverage.** Update the authoritative token catalog, typography/package contracts, public APIs that actually change, implementation wiki, and deterministic documentation tests in their assigned later tasks. This review records the boundary; it does not mark planned tokens or components implemented.

## Additive Compatibility Contract

- Existing token names, values, types, component kinds, style-variable names, `fontRole` values, and `UiTextVariant` behavior remain valid.
- Existing `clay.contributions.themeTokens` records remain semantic aliases and require no migration.
- Existing Gruvbox theme manifests remain valid and receive every new UI value through same-typed core fallbacks.
- `StyleRegistry` remains the editor paint-path color authority; `TypographyRegistry` remains the font/geometry authority.
- Missing active-theme UI overrides use core fallbacks. Unknown names, wrong types, partial typography hierarchy, invalid bounds, and oversized payloads fail before client installation.
- Package-facing additions are inert and additive. No raw scalar escape, raw CSS, concrete package font setting, callback, or native handle is introduced.

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

Hot paths may perform bounded reads from installed `StyleRegistry`, `ResolvedUiTheme`, `TypographyRegistry`, `PaneSlotLayout`, and inert package UI state.

## Security Boundary

Clay remains owner of Masonry widgets, Vello painting, Parley shaping, working areas, pane/split trees, slots, component catalog, active-theme installation, typography resolution, and final layout geometry. Server/package boundaries validate bounded inert declarations before client publication; client installation revalidates protocol candidates where applicable.

Theme/token/typography declarations grant no filesystem, network, shell, package-manager, AI, WASM, raw `Deno.core.ops`, native-widget, renderer-callback, client-JavaScript, document, workspace, persistence, font-download, or external-process authority. Ordinary packages may declare package-prefixed aliases, but only the existing first-party `setTheme` selection boundary may install theme-package concrete `designTokens` until a later approved authority model says otherwise.

## Phase Boundary

- **Phase 20.1:** token domains/catalog/defaults, active-theme UI values, semantic typography hierarchy, token-backed panel/density defaults, compatibility/docs/tests.
- **Phase 20.2:** native divider, focus-ring, panel chrome, resize-handle, scroll chrome, badge, `kbd`, icon, and tooltip primitives.
- **Phase 20.3:** user-facing split/panel resizing, collapse/restore, persistence, and inert package layout intents.
- **Phase 20.4:** component visual uplift and interaction-state rendering without component-kind/style-variable schema changes.
- **Phase 20.5:** reserved overlay/menu/input component kinds and interaction behavior.
- **Phase 20.6:** Modus Operandi/Vivendi packages, light/dark selection semantics, and theme/font settings UI.
- **Phases 20.7–20.8:** conformance enforcement and continuing reference/catalog maintenance.

Phase 20.1 must not implement those deferred primitives/components, restyle every component, ship Modus themes, or add resize/persistence behavior.

## Implementation status (Phase 20.1 tasks 1–10)

Tasks 1–10 of plan 062 are complete as of 2026-07-23:

- Typed catalog expanded to 73 core tokens across ten domains (`src/shell/theme.rs`).
- `ActiveTheme.design_tokens`, package `designTokens`, and client `ResolvedUiTheme` install atomically beside `StyleRegistry`.
- `UiTypographyHierarchy` and seven `UiTextVariant` values are user-owned through `setTypography`.
- `PanelDefaults` and density `spacing_scale()` derive from resolved dimension/density tokens; package UI and SDUI left-slot geometry consume one layout-default view.
- Authoritative reference docs, Clay JS API surfaces, configuration review, hot-path source guards, and manual smoke matrix are updated.
- Implementation details: [Editor Theme Registry](editor-theme-registry.md), [Typography Registry and Font Roles](typography-registry-and-font-roles.md), [Slot-Aware Package UI](slot-aware-package-ui.md), [Masonry Editor](masonry-editor.md).

Task 11 (this wiki pass) records the final implementation state in those pages and the master index.

## Tests

- `tests/primitives_docs.rs::phase20_1_ui_design_language_primitive_review_is_linked_and_complete`: locks inventory, reuse, generic gaps, additive compatibility, hot-path/security boundaries, and phase ownership.
- Existing `src/shell/theme.rs`, `src/shell/components.rs`, `src/editor/typography.rs`, `src/shell/layout.rs`, `src/shell/package_ui.rs`, `src/masonry_sdui.rs`, theme-package, typography-protocol, payload-budget, accessibility, and editor source-guard tests provide the executable baseline.

```bash
cargo test --test protocol primitives_docs::phase20_1_ui_design_language_primitive_review_is_linked_and_complete
cargo test --test protocol primitives_docs::wiki_index_links_every_wiki_page
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Semantic Typography Roles](../../reference/primitives/typography.md)
- [Clay Shell and Package UI/Layout Strategy](../../reference/primitives/shell-layout-strategy.md)
