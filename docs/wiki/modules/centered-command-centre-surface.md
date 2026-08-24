# Centered Command Centre Surface (Phase 24.4)

## Scope

Phase 24.4 changes only presentation, accessibility, and input containment for
Clay's built-in Command Centre command/path sessions. Command execution,
filesystem browsing, fuzzy matching, keybindings, package APIs, and authority
remain unchanged.

## Flow

1. `ControlCenter::session` and `PathBrowserSession::menu_session` set
   `TransientMenuOrigin::Centered`.
2. The origin round-trips through `TransientMenuOriginData` and protocol
   snapshots (`src/shell/transient_menu.rs`, `src/protocol/menu.rs`).
3. `TransientPackageOverlay::from_menu_session` maps the origin to the
   internal `PackageOverlayAnchor::Centered`, adds sanitized menu labels,
   selected state, and a bounded result-count string.
4. `SduiNativeState` separates centered overlays from pane-local overlays.
   `EditorWidget` keeps the centered menu in server-owned menu state for input,
   while `Driver` owns only the optional window-layer `WidgetId`.
5. `EditorWidget::reconcile_centered_overlay_layer` mounts or reuses one
   `PackageOverlayHost::new_centered()` with `RenderRoot::add_layer` at the
   window origin. Query and selection snapshots reconcile that host in place;
   close, tab changes, disconnect, registry removal, and runtime replacement
   remove it idempotently.

Only the active tab's centered menu is mounted. Completion, context-menu,
menu-bar, and package overlays stay on their existing pane-local host.

## Geometry and paint

`PackageOverlayHost::layout` derives the full window rectangle from its parent
size and resolves `dimension.overlay.centered.width` from the cached
`ResolvedUiTheme`. Width clamps to the window; the existing bounded transient
height and `spacing.panel` inset remain in use. Centered paint calls the generic
`paint_scrim` primitive once over the window rectangle, then paints the existing
`paint_tooltip_shell` chrome and retained component children.

The scrim uses `surface.scrim` plus `opacity.scrim`. Defaults are black, 0.5,
and 640 logical pixels. No backdrop blur, offscreen texture, shader, or filter
pass exists. Theme values resolve at install/reload, not during paint/layout.

## Accessibility and input

The centered root host reports a named modal `Role::Dialog`. Its hosted region
reports `Role::Menu`, `Role::MenuItem` rows, and one `Role::Status` result-count
node with `Live::Polite`. Count grammar is `0 results`, `1 result`, or `{n}
results`. Menu item nodes use retained-region-derived namespaced IDs, AccessKit
selected state, and the existing selected-label suffix. The status ID remains
stable across selection/query snapshots; only a count change changes its label.
Prompt and item text pass existing accessibility bounds/sanitization.

Masonry focus remains on the originating pane. While a server-owned centered
menu is active, `PaneDocumentView` consumes every keyboard event, clipboard
paste, and IME event before editor routing. Recognized keys enqueue existing
menu intents; unsupported keys and queue failures are still consumed. The
centered root host swallows pointer-down events and restores/retains the
originating focus target, so scrim clicks cannot mutate the document.

## Package and authority boundary

`PackageOverlayAnchor::Centered` is Clay-internal. `parse("centered")` falls
back to the normal package anchor, and the package JS `OverlayAnchor` surface
remains `working-area | active-pane | main | pointer`. Packages cannot request
the built-in centered layer, paint its scrim, open/drive server menu sessions,
intercept menu input, or obtain browse authority.

The snapshots remain inert display data. Path activation continues through the
server-owned `PathBrowserSession` and existing grant conversion rules.

## Plan 097 Phase 9 React projection

`frontend/src/command-centre/CommandCentre.tsx` consumes the same bounded
snapshot and intent families. React Aria supplies modal focus containment,
Escape handling, focus restoration, labelled textbox/listbox/option semantics,
and the polite result count. The query and selected row remain controlled by
the server snapshot; React sends full query updates, semantic backspace,
relative selection movement, opaque primary/secondary activation, and cancel.
Command, Path Browser, and picker sessions share this component. The per-tab
workspace controller drops the session on the existing close/reload/tab
lifecycle events. No second catalogue, fuzzy matcher, path resolver, grant
logic, or package extension point exists in the frontend.

## Tests and extension guidance

Focused implementation coverage lives beside the code:

- `src/masonry_package_region.rs`: full-window scrim ordering, geometry clamp,
  modal shield, retained host reuse, and menu accessibility.
- `src/masonry_editor.rs`: local-host filtering, root-layer lifecycle, stable
  dialog/menu/status tree, pointer containment, and count/status identity.
- `src/masonry_pane_document.rs`: modal queue-failure/modifier containment.
- `tests/ui_primitive_conformance.rs`: token-driven single scrim/no-blur guard.
- `tests/suites/editor.rs` via `editor_performance_invariants`: hot-path policy.

Run focused checks with:

```bash
cargo test --lib centered_layer_reconciles_in_place_and_removes_idempotently
cargo test --lib masonry_package_region::tests::centered_overlay_host_clamps_width_and_reuses_layer_on_resize
cargo test --test editor editor_performance_invariants
cargo test --test protocol primitives_docs
```

Future built-in transient menus should reuse `TransientMenuSession`, the
origin/protocol projection, `PackageOverlayHost`, and the existing accessibility
shape. Add a new origin only with an explicit authority/lifecycle decision;
do not add another command-centre renderer or a package-facing centered anchor.

## Related pages

- [Transient Menu Session](transient-menu-session.md)
- [Transient Menu Round Trip](transient-menu-round-trip.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md)
- [Accessibility contract](../../development/accessibility.md)
- [Performance workflow](../../development/performance.md)
- [UI Chrome Primitives](../../reference/primitives/ui-chrome-primitives.md)
