# Phase 18.2 Shell Runtime Primitive Review

## Source

- `plans/025-Phase18.2-Masonry-Clay-Shell-and-Pane-Runtime-Foundation.md`
- `plans/024-Phase18.1-Clay-Shell-Working-Area-and-Package-UI-Layout-Architecture-Gate.md`
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/phase18.1-shell-layout-primitive-review.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/masonry-editor.md`
- `docs/wiki/modules/server-driven-ui.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/configuration-runtime.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/parse-coordinator.md`
- `.agents/skills/create-plan/references/clay.md`
- `.agents/skills/project-patterns/references/mode-primitive-first.md`
- `.agents/skills/project-patterns/references/package-ui-layout.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`

## Overview

This page records the Phase 18.2 primitive-first implementation review before adding the Clay shell runtime. Phase 18.1 established the architecture contract. Phase 18.2 must now implement only the generic runtime foundations needed to move `EditorWidget` below a Clay-owned shell root: `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout`.

The review confirms that existing editor, SDUI, behavior-manifest, command/action, configuration, package loading, parse/decorations, and Masonry surfaces are reusable building blocks, but none of them should be widened into a package-owned native shell API. The runtime implementation should add generic Rust state and a Clay-owned Masonry container above the editor, then let later Phase 18.3 and Phase 18.4 package UI/configuration APIs consume those primitives.

## Existing Runtime Primitive Inventory

| Primitive area | Current source paths | What Phase 18.2 can reuse now | Runtime classification | Security and authority boundary |
| --- | --- | --- | --- | --- |
| Current application root and driver actions | `src/main.rs` (`run_editor`, `Driver::on_start`, `Driver::on_action`, `spawn_client_connection_event_bridge`, `connection_event_user_event`) | Startup already creates one `NewWindow` and bridges `ClientConnectionEvent` values into Masonry actions. Phase 18.2 can replace `NewWidget::new(editor_widget)` with a Clay shell root while preserving the child editor widget ID for focus fallback and event routing. | Startup for root creation and bridge setup; mutation/update time for `RenderRoot::edit_widget`; not paint/layout/text-event work. | `WidgetId` remains internal. Packages do not receive root IDs, editor child IDs, native handles, raw ops, or action authority. |
| Editor component and local input surface | `src/masonry_editor.rs`, `src/editor/surface.rs`, `docs/wiki/modules/masonry-editor.md` | `EditorWidget` already owns local text input, caret, viewport, selection, edit queue emission, status chrome, SDUI event application, behavior manifest installation, and decoration installation. Phase 18.2 should keep it as the `main` component in a pane. | Masonry hot path for pointer/text/scroll/paint consuming local state; protocol/update time for `apply_connection_event`; no package validation in handlers. | Editor state is client-owned native state. It must not become a package-extensible native widget constructor, renderer callback, or raw `Deno.core.ops` target. |
| Fixed SDUI sidebar and editor-region helper | `src/masonry_sdui.rs` (`SIDEBAR_WIDTH`, `SduiNativeState::paint`, `editor_region`, `editor_region_for_document`, `SduiObservableSnapshot`) | Existing inert SDUI tree rendering, structural observability, editor binding detection, action hit regions, and accessibility traversal can be reused. The fixed `SIDEBAR_WIDTH` geometry is the gap to move behind shell slot state. | Protocol/update time for snapshot/update application; paint/layout-adjacent native work consumes local `SduiNativeState`; action hit testing is pointer time over cached regions. | SDUI nodes are inert and bounded; they are not Masonry widget handles. Actions remain command intents and cannot contain raw CSS, callbacks, native handles, client-side JavaScript, filesystem/network/shell/AI authority, or raw ops. |
| Behavior manifests and key routing | `src/protocol/mod.rs`, `src/behavior/manifest.rs`, `src/editor/surface.rs`, `docs/wiki/modules/behavior-manifests.md` | Installed `BehaviorManifest` data keeps client-first text transforms and key routing deterministic. Shell focus/input routing must preserve this installed-manifest path instead of introducing package JS in key handlers. | Manifest install is protocol/update time; `ClientFirstPredictable` routes run on the client-first editor hot path from Rust-known data. | Package behavior remains inert manifest data; executable transform fields, arbitrary client-first package commands, and synchronous IPC/JS before local paint remain rejected. |
| Command/action registry and UI command routes | `src/packages/commands.rs`, `runtime/js/commands.ts`, `runtime/js/keybindings.ts`, `src/protocol/sdui.rs`, `src/main.rs::handle_client_ui_command`, `docs/wiki/modules/command-registry.md` | Existing command metadata, routing policy validation, and SDUI `SduiActionIntent` shape can be reused for shell-aware actions later. Phase 18.2 should preserve current editor and client UI command routes without inventing package shell commands. | Load/configuration time for registration and keybinding validation; explicit user action/update time for intents; keypress reads installed route data only. | Commands must be registered, package-prefixed when package-owned, permission-compatible, and inert. Shell routing must not grant filesystem, network, shell, AI, WASM, native widget mutation, or raw-op authority. |
| Configuration runtime | `src/server/configuration.rs`, `src/server/ops/configuration.rs`, `runtime/js/configuration.ts`, `docs/wiki/modules/configuration-runtime.md` | `~/.config/clay/init.js` is already the right future user configuration entry point. Phase 18.2 should not add hidden split/slot/sidebar config keys unless it promotes a documented Clay JS API. | Startup/reload/configuration-change time only. Configuration must not run in Masonry paint/layout/pointer/scroll/key/text-event handlers. | Configuration cannot grant filesystem outside the config root, network, shell, AI, WASM, package enable/disable, raw ops, native handles, raw CSS, client-side JavaScript, or direct Masonry access. |
| Package loading and primitive contribution descriptors | `src/packages/record.rs`, `src/packages/service.rs`, `src/packages/conflict.rs`, `docs/wiki/modules/package-loading.md` | Package identity, prefix, permission, provenance, conflict, SDUI region, command, behavior, decoration, and configuration validation establish the load-time validation pattern future shell declarations should follow. | Package install/enable/load/activation time only; no package validation in ordinary typing, paint, layout, scroll, pointer, or text-event paths. | Packages contribute inert descriptors only. Phase 18.2 runtime internals should not bypass package-prefix, permission, provenance, conflict, or budget checks when later `clay:ui` surfaces are added. |
| Decoration transport and editor render data | `src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/editor/surface.rs`, `docs/wiki/modules/decoration-transport.md` | Decorations prove the model for server-validated inert data consumed by client paint. They are useful as a pattern, not as a pane/slot state carrier. | Background/protocol update time for validation/publication; paint consumes cached spans only. | Decoration spans cannot carry draw callbacks, CSS, native handles, raw ops, client JavaScript, or stale/oversize payloads. Shell layout must preserve the same no-executable-payload rule. |
| Parse coordinator and background package work | `src/server/parse_coordinator.rs`, `src/protocol/parse.rs`, `runtime/js/parse.ts`, `docs/wiki/modules/parse-coordinator.md` | Parse scheduling demonstrates cancellable server-side package work and versioned payload validation. Shell implementation must keep parse/package work away from native layout and input handlers. | Background only; parse scheduling and validation never block client-first input, edit acknowledgement, or Masonry paint/layout. | Parser execution stays server-side with `parse-document`; no parser functions, markdown-it tokens, or package JavaScript enter the Rust client shell. |
| Masonry widget/container substrate | `src/main.rs`, `src/masonry_editor.rs`, `src/masonry_sdui.rs`, local Masonry 0.4.0 docs reviewed in Phase 18.2 task 1 | Masonry provides `NewWidget`, `NewWindow`, `AppDriver`, `Widget`, `WidgetId`, `WidgetPod`, `RegisterCtx`, `ChildrenIds`, `LayoutCtx`, `PaintCtx`, `RenderRoot::edit_widget`, and child registration/layout APIs for a Clay-owned container. These are implementation details for `masonry_shell`/`shell` modules. | Startup for root construction; mutation/update paths for child changes; layout/paint should read already-installed state and place already-registered children only. | Masonry types are internal. Packages must not create widgets, mutate the native tree, receive native IDs/handles, provide layout callbacks, or depend on layout pass timing. |

## What Existing Primitives Can Achieve Before New Shell Code

- Clay can start a single native window, focus one editor widget, bridge server/client connection events onto the GUI thread, and route those events to `EditorWidget::apply_connection_event`.
- `EditorWidget` can remain the primary editor component for ordinary typing, scrolling, pointer selection, status rendering, edit acknowledgements, resyncs, behavior-manifest updates, SDUI snapshots/updates, decorations, and selected-file UI command handling.
- SDUI can publish and render inert component trees and route button/list interactions as command intents, but its current fixed-sidebar geometry is not a generic pane slot model.
- Behavior manifests, command declarations, keybindings, package loading, configuration, parse, and decoration paths already demonstrate Clay's desired pattern: validate/load/compute outside hot paths, then let client hot paths consume bounded inert state.
- Structural observability already exists for SDUI and status chrome. Phase 18.2 can copy that pattern for shell state without exposing document text, native handles, raw action authority, or a public Clay JS API.

## Generic Runtime Gaps to Implement in Phase 18.2

### `WorkingAreaLayout`

Implement a Clay-owned working-area state and shell root above the editor.

- Tentative files: `src/shell/mod.rs`, `src/shell/layout.rs`, `src/masonry_shell.rs`, `src/lib.rs`, and `src/main.rs`.
- State should record one native window working area, one active pane tree root, the active pane, a layout version, and the editor component binding needed for focus/action routing.
- The Masonry widget should be a Clay-owned container (for example `ClayShellWidget`) that registers the existing `EditorWidget` as a child and exposes internal child-ID accessors for the driver.
- Startup can construct the default one-pane working area. Mutation/update paths may later apply validated shell state. Masonry layout/paint should only read installed shell state and place already-registered children.
- The implementation should remain `pub(crate)` or private unless a later task deliberately promotes a documented Clay JS API.

### `PaneSplitTree`

Implement generic pane/split topology state independent of package or mode behavior.

- Tentative files: `src/shell/layout.rs` plus shell widget layout tests in `src/masonry_shell.rs`.
- Model leaf panes and horizontal/vertical split nodes with stable pane IDs, split orientation, bounded ratio/min/max validation, a default one-leaf tree, active-pane metadata, and geometry helpers.
- Validation must reject duplicate pane IDs, unsupported orientations, invalid split ratios, empty trees, oversize future payloads, raw native handles, raw CSS, raw ops, and client-JS hooks by type or validator.
- Split topology changes are startup/update work only. Layout computes rectangles from installed state and must not add/remove Masonry children during layout.

### `PaneSlotLayout`

Implement leaf-pane slot state and geometry for `main` plus optional fixed slots.

- Tentative files: `src/shell/layout.rs`, `src/masonry_shell.rs`, `src/masonry_sdui.rs`, and `src/masonry_editor.rs` if adapter methods are needed.
- Every leaf pane has exactly one mandatory `main` slot. Optional `left`, `right`, `top`, and `bottom` slots should carry visibility, collapsed state, current size, min/max size, and future resize provenance.
- Geometry should compute `main` from visible fixed slots and keep editor input, caret, selection, scroll, paint, and status behavior bounded to the main region.
- Existing `SduiNativeState` can be bridged as internal Clay-owned slot content for the current side panel until Phase 18.3 implements slot-aware `PanelContribution` and `ComponentContribution`.
- Slot state must reject or make impossible duplicate mandatory-main removal, invalid slot IDs, negative/NaN sizes, min/max inversions, raw native handles, raw CSS, client-side JavaScript, and unregistered action targets.

### Internal shell observability

Implement structural shell snapshots for tests and future agent inspection.

- Tentative file: `src/masonry_shell.rs`.
- Snapshot fields should include layout version, pane count, split count, active pane, visible slots, editor component binding, editor/main region non-empty booleans, and SDUI/status presence.
- The snapshot must omit document text, secrets, raw filesystem paths beyond existing sanitized UI state, native widget IDs/handles, raw action payload authority, and executable package code.
- This is not a public `clay:ui` query API.

## Deferred Package-Facing Gaps

Phase 18.2 should not implement package-facing `clay:ui` public APIs unless a later task explicitly promotes them with facade/op/docs/registry/test coverage. These surfaces remain planned for Phase 18.3 or Phase 18.4:

- `PanelContribution` and `clay.ui.serverRegisterPanelContribution`
- `ComponentContribution` and `clay.ui.serverRegisterComponentContribution`
- `TransientOverlayContribution` and `clay.ui.serverRegisterTransientOverlayContribution`
- `PackageThemeTokenDeclaration` and `clay.ui.serverRegisterThemeToken`
- `PackageUiStateScope` and `clay.ui.serverRegisterUiStateScope`
- `PackageLayoutOverride` and `clay.ui.serverSetLayoutOverride`
- User-visible split/slot/sidebar configuration keys through `~/.config/clay/init.js`
- Persisted layout overrides, multi-panel ordering inside one slot, cross-window layout sync, overlay z-order buckets, and package priority fields
- Public `clay:ui` configuration APIs remain planned until full facade/op/reference docs/registry/test coverage exists

## Hot-Path Classification for New Runtime Work

- **Startup:** Create default `WorkingAreaLayout`, default one-leaf `PaneSplitTree`, default `PaneSlotLayout`, Clay shell root widget, and editor child binding.
- **Mutation/update:** Apply future validated shell layout updates, selected-pane changes, slot visibility/resize state, client connection events, SDUI snapshots/updates, and render/accessibility requests.
- **Layout:** Compute working-area, split, pane, slot, and child rectangles from installed state. Do not parse packages, run package JavaScript, wait on IPC, deserialize full documents, validate package metadata, or mutate Masonry children during layout.
- **Paint:** Paint shell background/slot chrome if needed and delegate to editor/SDUI/status components using already-installed local state only.
- **Protocol/update:** Continue routing behavior manifests, edit acknowledgements, resyncs, SDUI, decorations, runtime diagnostics, and selected-file events through existing typed events. Add shell protocol messages only if they are bounded, versioned, inert, and tested.
- **Editor hot path:** Keypress, text-event, pointer selection, scroll, caret movement, local edit application, and first local paint after input remain client-first and Rust-known. No package JavaScript, package validation, configuration evaluation, package parsing, blocking IPC, or full-document serialization may enter these paths.

## Security and Validation Boundaries

- Shell state must validate or type-prevent invalid working-area versions, duplicate pane IDs, stale update versions, invalid split ratios, unsupported orientations, unknown slot IDs, missing mandatory `main`, invalid sizes, unsafe collapse/visibility state, and unregistered action targets.
- Package/user inputs remain future, server-validated inert requests with package provenance and precedence; Phase 18.2 internal defaults are not permission grants.
- Packages must not create Masonry widgets, mutate native layout, provide raw CSS/HTML/scripts, run client-side JavaScript, call raw `Deno.core.ops`, receive native widget IDs/handles, provide Vello/Parley callbacks, or smuggle filesystem/network/shell/AI/WASM authority through shell state.
- SDUI/component content remains bounded inert data. Shell observability and diagnostics must omit document text, secrets, native handles, raw action payload authority, and executable package code.
- Hidden JSON/TOML/ad hoc shell layout keys are rejected. Any user-visible setting must become a documented Clay JS API with `custom_properties`, registry coverage, docs-index links, backing Rust/op/facade metadata, and tests.

## Rejected Implementation Shapes

- Do not keep `EditorWidget` as the application shell and hide shell state inside it.
- Do not fork the fixed sidebar into `MarkdownPreviewSidebar`, `MarkdownPaneLayout`, `MarkdownMasonryPanel`, `MarkdownShellWidget`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust shell/layout branch.
- Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, `Split`, `Flex`, native handles, layout callbacks, Vello callbacks, Parley callbacks, or raw op names as package APIs.
- Do not add package validation, package parsing, configuration evaluation, JavaScript execution, or blocking IPC to Masonry paint/layout/pointer/scroll/key/text-event handlers.
- Do not promote planned `clay:ui` inventory stubs to callable APIs without full Clay JS facade/op/reference docs/registry/test coverage.

## Implementation Plan Summary

1. Add generic shell state (`WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`) in a reusable `src/shell` module.
2. Add a Clay-owned Masonry shell root widget in `src/masonry_shell.rs` that contains the existing editor child and later maps pane/slot geometry to child placement.
3. Update `src/main.rs` so startup owns a shell root while preserving the child editor widget ID for focus fallback and existing `EditorAction` routing.
4. Move fixed-sidebar geometry decisions toward `PaneSlotLayout` state while keeping existing inert `SduiNativeState` content/action handling as an internal bridge.
5. Add structural shell observability and deterministic tests for default one-pane state, split validation/geometry, slot geometry/validation, hot-path boundaries, security prohibitions, and docs/wiki coverage.
6. Keep public `clay:ui` APIs, package panels/components/overlays/theme tokens/state scopes/layout overrides, and shell configuration APIs planned unless a later Phase 18.2 task deliberately implements the full public API contract.

## Verification

- Inventory reviewed: editor root/driver actions, editor component, SDUI/fixed sidebar, behavior manifests, command/action registry, configuration runtime, package loading, decoration transport, parse coordinator, and Masonry container substrate.
- Generic gap review: Phase 18.2 implementation should add only `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, internal shell observability, and a Clay-owned shell widget above the editor.
- Deferred-surface review: package panels/components/overlays/theme tokens/state scopes/layout overrides and public `clay:ui` configuration APIs remain planned for Phase 18.3/18.4 unless fully promoted with docs/registry/tests.
- Hot-path review: startup/update/layout/paint/protocol/editor hot-path classifications explicitly keep package JavaScript, package validation, package parsing, blocking IPC, full-document serialization, and child mutation out of Masonry hot paths.
- Security review: shell state validation covers pane IDs, split ratios, slot IDs, sizes/collapse/visibility, action targets, SDUI/component content, package/user future inputs, and native/Masonry/raw-op/CSS/client-JS non-authorities.

## Tests

- `tests/primitives_docs.rs::phase18_2_shell_runtime_review_records_existing_inventory`
- `tests/primitives_docs.rs::phase18_2_shell_runtime_review_maps_generic_primitives`
- `tests/primitives_docs.rs::phase18_2_shell_runtime_review_rejects_mode_specific_shell_branches`
- `cargo test --test primitives_docs`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.1 Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Command Registry](command-registry.md)
- [Configuration Runtime](configuration-runtime.md)
- [Package Loading](package-loading.md)
- [Decoration Transport](decoration-transport.md)
- [Parse Coordinator](parse-coordinator.md)
