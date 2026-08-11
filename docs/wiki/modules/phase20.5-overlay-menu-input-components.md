# Phase 20.5: Overlay, Menu, and Input Components

Phase 20.5 promoted `dropdown`, `collapse`, and `modal` from reserved to implemented, added `textInput` as a new kind, and uplifted all transient surfaces onto the shared overlay primitive with z-level stacking and complete keyboard navigation.

## Source

- `src/shell/components.rs` — `ComponentKind` (15 variants), `DeferredComponentKind` (only `Table`), style variable validation (`placeholderColor`, `validationState`)
- `src/shell/package_ui.rs` — `PackageUiComponentTree.validation_state`, `TransientPackageOverlay.z_level_token`, `from_menu_session` anchor-by-origin
- `src/shell/transient_menu.rs` — `TransientMenuOrigin` enum, `TransientMenuSession.origin` field
- `src/masonry_sdui.rs` — paint arms for new kinds, z-level sort in `paint_package_overlays`, `dropdown_selected`/`collapse_expanded` state, `modal_focusable_intents`, `collect_component_intents`
- `src/masonry_editor.rs` — `route_package_component_key` keyboard routing

## New Component Kinds

| Kind | Accessibility role | Paint | Keyboard |
|------|-------------------|-------|----------|
| `dropdown` | `Role::ComboBox` | Button-like trigger; selected item label from `dropdown_selected_index`; children painted when focused | ArrowUp/Down cycles, Enter/Space confirms |
| `collapse` | `Role::Group` | Title row with `ui.collapseToggle` action; children painted only when `is_collapse_expanded` | Enter/Space toggles |
| `modal` | `Role::Dialog` | `paint_tooltip_shell` chrome + title + children | Tab/Shift+Tab focus-trap via `modal_focusable_intents` |
| `textInput` | `Role::TextInput` | Bordered field, `surface.control` fill, validation-state border, placeholder in `text.muted`, focus ring | Focus routing via action intent |

`table` remains the only deferred kind (no first-party package need identified).

## New Style Variables

| Variable | Type | Applies to |
|----------|------|------------|
| `placeholderColor` | color-role token | `textInput` placeholder text |
| `validationState` | enum: `none`/`error`/`warning`/`success` | `textInput` border state |

Both are additive; no existing style variable or token name changed.

## Z-Level Stacking

`TransientPackageOverlay` carries `z_level_token: &'static str`. `paint_package_overlays` sorts overlays before painting:

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

`SduiNativeState` gains two client-local fields (no package authority):

- `dropdown_selected: BTreeMap<u64, usize>` — selected item index per dropdown node hash
- `collapse_expanded: BTreeSet<u64>` — expanded state per collapse node hash

Node hash is `stable_package_source_id(component_id)` (FNV-1a, same as `SduiNodeId`).

## Keyboard Routing

`route_package_component_key` (`src/masonry_editor.rs`) is called before `route_menu_key` in `local_key`:

- `ui.dropdownToggle` focused: ArrowUp/Down → `dropdown_cycle`, Enter/Space → clear focus (confirm)
- `ui.collapseToggle` focused: Enter/Space → `collapse_toggle`
- Any other focused action + Tab: modal focus-trap cycles `modal_focusable_intents()`

## Compatibility

No `ComponentKind`, style-variable, or token-name change. `placeholderColor` and `validationState` are additive. Packages require no manifest or style edit.

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
