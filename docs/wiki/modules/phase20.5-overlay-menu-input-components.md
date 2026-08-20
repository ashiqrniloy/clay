# Phase 20.5: Overlay, Menu, and Input Components

Phase 20.5 promoted `dropdown`, `collapse`, and `modal` from reserved to implemented, added `textInput` as a new kind, and uplifted all transient surfaces onto the shared overlay primitive with z-level stacking and complete keyboard navigation.

## Source

- `src/shell/components.rs` — `ComponentKind` (15 variants), `DeferredComponentKind` (only `Table`), style variable validation (`placeholderColor`, `validationState`)
- `src/shell/package_ui.rs` — `PackageUiComponentTree.validation_state`, `TransientPackageOverlay.z_level_token`, `from_menu_session` anchor-by-origin
- `src/shell/transient_menu.rs` — `TransientMenuOrigin` enum, `TransientMenuSession.origin` field
- `src/masonry_package_region.rs` — retained package widgets (`PackageDropdown`, `PackageCollapse`, `PackageModal`, `PackageTextInput`), widget-local keyboard/focus routing, clipping, and inert action types
- `src/masonry_sdui_region.rs` — retained SDUI region/scroll host and shared accessibility clipping
- `src/masonry_sdui.rs` / `src/masonry_editor.rs` / `src/app_driver.rs` — inert render inputs, host synchronization, action downcast, and overlay-layer routing

## New Component Kinds

| Kind | Accessibility role | Paint | Keyboard |
|------|-------------------|-------|----------|
| `dropdown` | `Role::ComboBox` | Button-like trigger; selected item label from `dropdown_selected_index`; children painted when focused | ArrowUp/Down cycles, Enter/Space confirms |
| `collapse` | `Role::Group` | Title row with `ui.collapseToggle` action; children painted only when `is_collapse_expanded` | Enter/Space toggles |
| `modal` | `Role::Dialog` | `paint_tooltip_shell` chrome + title + children; clipped retained subtree | Tab/Shift+Tab focus-trap via `modal_focusable_intents`; Escape emits optional inert dismiss intent |
| `textInput` | `Role::TextInput` | Bordered field, `surface.control` fill, validation-state border, placeholder in `text.muted`, shared focus ring, token-driven spacing/border | Focus routing via action intent |

`table` remains the only deferred kind (no first-party package need identified).

## New Style Variables

| Variable | Type | Applies to |
|----------|------|------------|
| `placeholderColor` | color-role token | `textInput` placeholder text |
| `validationState` | enum: `none`/`error`/`warning`/`success` | `textInput` border state |

Both are additive; no existing style variable or token name changed.

## Z-Level Stacking

`TransientPackageOverlay` carries `z_level_token: &'static str`. `PackageOverlayHost` sorts retained overlay hosts before layout/paint:

```
z.overlay (0) < z.modal (1) < z.tooltip (2)
```

`from_menu_session` sets `"z.overlay"`. The overlay cursor inset reads `self.ui_theme.scalar_f64("spacing.panel")` directly (not the `theme_style()` cache), consistent with `paint_tooltip_shell`.

## Surface Origin

`TransientMenuOrigin` on `TransientMenuSession` selects the overlay anchor:

- `CommandPalette` → `PackageOverlayAnchor::Bottom` (default)
- `ContextMenu` → `PackageOverlayAnchor::Pointer`
- `MenuBar` → `PackageOverlayAnchor::Main`

## Client-Local Interaction State

The original Phase 20.5 state model was reviewed during the retained Masonry cutover. Current production state is widget-local and carries no package authority:

- `PackageDropdown.selected_index` / `open` — selected item and trigger/list state
- `PackageCollapse.expanded` — expand/collapse state
- `PackageModal` focusable-descendant list — local Tab/Shift+Tab trap
- `PackageTextInput` local text — optimistic while focused, server value adopted when unfocused

`SduiNativeState` remains inert render input/observability state; it does not own dropdown/collapse/modal pointer or focus fields. Stable package source hashes still key retained component identity, so surviving widgets keep their local state across compatible reconciles.

## Keyboard Routing

Widget-local key handling in `src/masonry_package_region.rs` (the Phase 22.x
split moved package-region widgets out of `src/masonry_editor.rs`; the old
`route_package_component_key` dispatcher no longer exists). Each focusable
package widget owns its route in its `on_text_event`; the pane's `local_key`
(`src/masonry_pane_document.rs`) runs `route_menu_key` before editor routing:

- `ui.dropdownToggle` focused: ArrowUp/Down → `cycle`, Enter/Space → `activate` (confirm), Escape while open → close
- `ui.collapseToggle` focused: Enter/Space → `collapse_toggle`
- Any other focused action + Tab: modal focus-trap cycles `modal_focusable_intents()`

## Plan 088 overlay/panel hardening

`PackageRegionWidget` clips retained package subtrees at the owning panel/overlay frame and marks panel, overlay, modal, and SDUI-region accessibility nodes as clipping their children. Nested `scroll` components receive flex sizing when used as bounded panel content, so long fixed panels remain scrollable instead of painting below their shell. Disabled package controls expose AccessKit disabled state; `statusItem` leaves expose `Role::Status`.

`PackageModal` preserves widget-local Tab trapping and restores the existing Escape action path: when its declaration has an action, `PackageModalDismiss` carries that inert `SduiActionIntent` to `main.rs`, which routes it through the same server-first provenance path as other package controls. No action means local dismissal signal only.

No `ComponentKind`, style-variable, or token-name change. `placeholderColor` and `validationState` are additive. Packages require no manifest or style edit; the first-party settings panel uses existing `panel` + `scroll` composition.

## Tests

```text
cargo test --lib shell::components --quiet
cargo test --lib masonry_sdui:: --quiet
cargo test --lib masonry_editor:: --quiet
cargo test --lib shell::transient_menu --quiet
cargo test --lib shell::package_ui --quiet
cargo test --test editor --quiet
```

## Related

- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Transient Menu Session](transient-menu-session.md)
- [Shell Primitives](shell-primitives.md)
- [Phase 20.4 Core Component Uplift](phase20.4-core-component-uplift-primitive-review.md)
- [UI Component Catalog](../../.agents/skills/clay-ui/references/components.md)
- [Package Authoring Guide](../../reference/packages/creating-packages.md)
