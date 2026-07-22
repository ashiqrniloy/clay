# Phase 18.3 Slot-Aware Package UI Primitive Review

## Source

- `plans/026-Phase18.3-Slot-Aware-Package-UI-Components-Panels-and-Theme-Tokens.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/wiki/modules/phase18.1-shell-layout-primitive-review.md`
- `docs/wiki/modules/phase18.2-shell-runtime-primitive-review.md`
- `docs/wiki/modules/masonry-shell.md`
- `docs/wiki/modules/server-driven-ui.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/primitive-architecture.md`
- `src/shell/layout.rs`
- `src/masonry_shell.rs`
- `src/masonry_sdui.rs`
- `src/protocol/sdui.rs`
- `src/server/sdui.rs`
- `src/server/ops/sdui.rs`
- `src/server/js_runtime.rs`
- `runtime/js/sdui.js`
- `src/packages/manifest.rs`
- `src/packages/permissions.rs`
- `src/packages/commands.rs`
- `src/packages/conflict.rs`
- `src/packages/record.rs`
- `tests/primitives_docs.rs`
- `.agents/skills/create-plan/references/clay.md`
- `.agents/skills/project-patterns/references/mode-primitive-first.md`
- `.agents/skills/project-patterns/references/package-ui-layout.md`
- `.agents/skills/project-patterns/references/behavior-manifests.md`
- `.agents/skills/project-patterns/references/package-distribution.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`

## Overview

This page records the Phase 18.3 primitive-first review before adding runtime-backed `clay:ui` package UI APIs. Phase 18.1 defined the Clay-owned shell/layout vocabulary. Phase 18.2 implemented the internal runtime foundations (`WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout`) and left public `clay:ui` APIs planned/unavailable. Phase 18.3 should now promote only generic slot-aware package UI primitives: `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration`.

The review confirms that the current SDUI helpers, shell slot state, command/action validation, package manifest/provenance validation, package loading descriptors, documentation registry machinery, and structural observability are reusable building blocks. They are not enough by themselves to expose package-authored fixed panels, transient overlays, a Clay component catalog, or typed package theme tokens. The implementation must add reusable primitives and validators rather than Markdown preview/status Rust branches.

## Existing Package UI Primitive Inventory

| Primitive area | Current source paths | What Phase 18.3 can reuse now | Runtime classification | Security and validation boundary |
| --- | --- | --- | --- | --- |
| SDUI helpers and runtime publication | `runtime/js/sdui.js`, `src/server/ops/sdui.rs`, `src/server/sdui.rs`, `src/protocol/sdui.rs`, `docs/wiki/modules/server-driven-ui.md` | Existing `clay:sdui` helpers can define inert `panel`, `label`, `button`, `list`, `editorView`, `flex`, and `stack` nodes and `publishTree` can convert JSON-like helper output into typed `SduiTree` state after server validation. | Package load/config/update work for helper execution and publication; protocol/client update work for `SduiSnapshot` / `SduiUpdate`; paint/layout state read for `SduiNativeState`. | SDUI already rejects unsupported node kinds, unknown document bindings, executable action payloads, unregistered action targets, stale updates, and payloads over `SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES` / `SDUI_UPDATE_PAYLOAD_BUDGET_BYTES`. Nodes are not Masonry widgets. |
| Shell slot state and geometry | `src/shell/layout.rs`, `src/masonry_shell.rs`, `docs/wiki/modules/masonry-shell.md` | Internal `PaneSlotLayout` has a mandatory `main` slot plus optional fixed `left`, `right`, `top`, and `bottom` slots with finite size, min/max clamp, visibility, collapse, user-resize metadata, deterministic geometry, and structural observations. | Startup/update work for installed shell state; paint/layout state read for geometry; editor hot path remains client-first and Rust-owned. | Validation rejects malformed split/slot state, stale layout versions, missing editor panes, oversize slot payloads, non-finite sizes, and duplicate/missing pane targets. Native widget IDs and Masonry handles remain internal. |
| Fixed-sidebar SDUI compatibility bridge | `src/masonry_sdui.rs` (`SIDEBAR_WIDTH`, `sdui_panel_left_slot_rect`, `editor_region`, `editor_region_for_document`, `SduiObservableSnapshot`) | Current SDUI content is rendered as a temporary fixed left-slot panel through `PaneSlotLayout`, preserving editor-main-region calculation and action hit regions until slot-aware package panels replace it. | Protocol/client update work for applying snapshots; paint/layout state read for side-panel geometry and already-validated nodes. | The bridge is Clay-owned native code. It still uses hardcoded color/size constants such as `PANEL_BACKGROUND`, `BUTTON_BACKGROUND`, `LIST_BACKGROUND`, `TEXT_COLOR`, `PANEL_PADDING`, `ROW_HEIGHT`, `TITLE_TEXT_SIZE`, and `BODY_TEXT_SIZE`, so Phase 18.3 needs a typed theme-token resolver instead of raw package styles. |
| Command registry and action validation | `src/packages/commands.rs`, `src/server/ops/commands.rs`, `runtime/js/commands.js`, `runtime/js/keybindings.js`, `src/server/ops/sdui.rs`, `docs/wiki/modules/command-registry.md` | Package commands are package-prefixed metadata with routing policy, user-facing label, key bindings, custom properties, and permissions. SDUI button/list actions already use `SduiActionIntent` and validate targets against registered command IDs before publication. | Package load/activation work for command metadata; explicit command/UI update work for action publication and user intent emission; keypress hot path reads installed behavior/key routing only. | Commands require `command-registration`; package commands cannot claim client-first edit authority; action targets must resolve to registered commands and must not carry callbacks, raw op names, native handles, filesystem/network/shell/AI/WASM authority, or executable code. |
| Package manifest, permissions, and provenance | `src/packages/manifest.rs`, `src/packages/permissions.rs`, `src/packages/record.rs`, `docs/wiki/modules/package-loading.md` | Manifest validation already checks `name`, `version`, `clay.apiPrefix`, known permissions, `entry`, optional `loadEntry`, package-prefixed modes, docs, performance estimates, API dependencies, and inert contribution descriptors. | Package load/config/update work only. Package validation must never run from Masonry paint/layout/pointer/scroll/key/text-event handlers. | Existing permissions include `command-registration`, `package-configuration`, `parse-document`, `render-decorations`, and other mode/render permissions. Prohibited authorities include `filesystem`, `network`, `shell`, `ai-mutation`, `wasm-execution`, `raw-deno-ops`, `native-widget`, `client-javascript`, `package-installation`, `package-enable-disable`, and `workspace-mutation`. |
| Package contribution metadata and conflicts | `src/packages/record.rs`, `src/packages/conflict.rs` | The record assembler validates command/configuration/keyRouting/textTransforms/sdui/decorations descriptors, checks package-owned IDs, estimates SDUI budgets, rejects executable widget fields, and records API dependencies. The conflict pass detects duplicate prefixes, modes, commands, key bindings, configuration keys, SDUI regions, decorations, and behavior entries. | Package enable/load/reload work only. | Phase 18.3 should reuse the same provenance-preserving diagnostic pattern for panel IDs, component IDs, overlay IDs, slot claims, action targets, theme tokens, and typed style variables. Current conflict kinds do not yet cover package UI panels/overlays/tokens. |
| Clay JS API inventory and documentation registry | `docs/reference/clay-js-api/api-inventory.toml`, `docs/reference/clay-js-api/`, `docs/wiki/modules/clay-js-doc-registry.md`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_facade_layout.rs`, `tests/rust_visibility_api_mapping.rs` | Planned `clay.ui.*` inventory rows already reserve names and metadata. Existing tests enforce runtime status, docs/index links, generated registry visibility, key binding metadata, custom properties, lookup tags, facade paths, op wrappers, and Rust visibility boundaries. | Documentation/build/test work, not runtime hot-path work. | Public Phase 18.3 APIs must be promoted through the Clay JS facade/op/docs/registry path; raw Rust paths, raw ops, Masonry names, and protocol DTOs must not become package-facing callable names. |
| Structural observability | `src/masonry_sdui.rs`, `src/masonry_shell.rs`, `tests/primitives_docs.rs`, `docs/wiki/modules/masonry-shell.md`, `docs/wiki/modules/server-driven-ui.md` | Headless snapshots already record shell/SDUI structure, visible slots, panel titles, node kinds, editor bindings, action hit regions, accessibility roles, and non-empty editor regions for deterministic tests. | Test/agent inspection and explicit update work only; no user-facing `clay:ui` query API. | Observations omit document text, secrets, native handles, Masonry widget IDs, raw action payload authority, raw CSS, raw ops, Vello/Parley callbacks, and executable package code. |

## What Existing Primitives Can Achieve Before New Phase 18.3 Code

- A package can publish a bounded inert `clay:sdui` tree with labels, buttons, lists, flex/stack layout, panels, and an editor view, then Clay can render the tree through native SDUI code.
- Clay can validate SDUI action targets against registered commands and can reject stale or unregistered command authority before publication.
- The client can compute fixed `left`/`right`/`top`/`bottom` slot geometry internally with `PaneSlotLayout`, and the existing editor remains in the mandatory `main` slot.
- Package records can retain package name, version, API prefix, docs, dependencies, contribution IDs, performance metadata, and deterministic conflict diagnostics for existing contribution kinds.
- Documentation registry tests can prevent public APIs from appearing without Markdown docs, generated registry metadata, lookup tags, key binding/custom property metadata, facade/op paths, and Rust visibility mapping.

These surfaces still do not let packages declare fixed panels, target slots directly, install transient overlays, declare component catalog entries beyond current SDUI nodes, resolve typed style/theme tokens, or define user overrides/state scopes through public APIs.

## Generic Phase 18.3 Primitive Gaps

### `PanelContribution`

Implement a generic slot-aware fixed panel declaration, not a Markdown preview sidebar.

- Tentative implementation files: `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/server/ui.rs` or `src/shell/contributions.rs`, `src/shell/components.rs`, `src/protocol/ui.rs` or `src/protocol/sdui.rs`, `src/masonry_sdui.rs`, `src/masonry_shell.rs`, `src/packages/record.rs`, `src/packages/conflict.rs`, and public docs/tests.
- The declaration should include a package-prefixed panel ID, target slot (`left`, `right`, `top`, or `bottom`), fixed/transient kind separation, default visibility, title/icon metadata where supported, action targets, component root, payload estimate, and package provenance.
- Registration is package load/config/update work. Client publication is protocol/client update work. Masonry layout and paint must only read installed panel/component/theme state.
- Validators must reject duplicate panel IDs, duplicate exclusive slot claims, invalid slot names, unregistered action targets, action permission mismatches, component roots with unsupported IDs, raw CSS, raw ops, native widget handles, direct Masonry widget constructors, client-side JavaScript, renderer callbacks, and oversize component payloads.

### `ComponentContribution`

Implement a Clay component catalog above SDUI/Masonry rather than exposing SDUI internals as the final API.

- Tentative implementation files: `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/shell/components.rs`, `src/protocol/ui.rs` or `src/protocol/sdui.rs`, `src/masonry_sdui.rs`, and package/docs/API tests.
- Reuse existing SDUI node semantics for `panel`, `label`, `button`, `list`, `editorView`, `flex`, and `stack` where possible.
- Add or explicitly defer `scroll/portal`, `statusItem`, `table`, `dropdown`, `collapse`, and `modal` with exact status docs/tests. The Phase 18.3 plan should implement only the generic subset required to prove fixed panels and transient overlays safely.
- Component declarations should state that component IDs must be package-prefixed or Clay-owned; action targets must be registered commands, and style variables must reference typed tokens rather than raw CSS/style strings/raw colors.
- Paint/layout state read uses already-validated component state only. No package JavaScript, schema validation, package parsing, full-document serialization, raw IPC wait, or child mutation should happen inside Masonry paint/layout/pointer/scroll/key/text-event handlers.

### `TransientOverlayContribution`

Implement generic transient overlay declarations for dismissible/focus-scoped UI.

- Tentative implementation files: `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/shell/components.rs`, `src/protocol/ui.rs` or `src/protocol/sdui.rs`, `src/masonry_sdui.rs`, `src/masonry_shell.rs`, and structural tests.
- The declaration should include a package-prefixed overlay ID, component root, focus policy, dismissal policy, accessibility role/label metadata, optional anchor/pane scope when implemented, z-order policy if supported, action targets, and provenance.
- Registration/defaults are package load/config/update work. Opening/dismissing an overlay is explicit command/UI update work. Painting an active overlay is paint/layout state read over installed inert data.
- Validators must reject duplicate overlay IDs, unsupported focus policies, non-dismissible modal-like overlays without Clay-owned policy, unregistered actions, raw native handles, raw CSS/style strings, renderer callbacks, client-side JavaScript, raw Deno ops, and oversize payloads.

### `PackageThemeTokenDeclaration`

Implement typed semantic package tokens and component style variables above hardcoded SDUI constants.

- Tentative implementation files: `runtime/js/ui.js`, `src/server/ops/ui.rs`, `src/shell/theme.rs`, `src/shell/components.rs`, `src/masonry_sdui.rs`, `docs/reference/primitives/package-security.md`, `docs/reference/packages/creating-packages.md`, and docs/API tests.
- Reuse Clay core token categories from `shell-layout-strategy.md`: `text.*`, `surface.*`, `border.*`, `accent.*`, `diagnostic.*`, `code.*`, and `selection.*`.
- Package-owned tokens must use the package prefix and include a semantic description, typed token kind, optional same-type fallback token, and package provenance.
- Component style variables should reference known typed tokens or documented enum/size values. Unknown tokens, duplicate package token names, type-incompatible fallbacks, raw CSS, raw style strings, raw colors without a typed token contract, Vello/Parley/native renderer callbacks, and hidden token override keys are rejected.
- Token declaration and user override validation are package load/config/update work. Paint reads resolved typed tokens without package JavaScript or raw style parsing.

## Deferred to Phase 18.4 Unless Deliberately Promoted

The review keeps `PackageUiStateScope` and `PackageLayoutOverride` as Phase 18.4 state/configuration work unless Phase 18.3 deliberately promotes them with full facade/op/reference-doc/registry/test coverage.

Deferred surfaces:

- `clay.ui.serverRegisterUiStateScope`
- `clay.ui.serverSetLayoutOverride`
- user panel visibility/default-slot overrides
- user package theme-token override APIs
- package/user layout precedence beyond the validated fixed/transient contribution defaults
- persisted pane/component/transient-overlay state scopes
- hidden JSON/TOML/ad hoc panel/style/layout configuration keys such as `preview.position`, `layout.preview.defaultSlot`, `preview.defaultVisibility`, or raw token override keys

If any of these become user-visible in Phase 18.3, they must be documented Clay JS APIs with `custom_properties`, docs/index links, generated registry coverage, key binding metadata, permissions/security notes, backing Rust/op/facade paths, and tests.

## Hot-Path Classification

| Work category | Phase 18.3 examples | Allowed timing |
| --- | --- | --- |
| Package load/config/update work | manifest validation, API dependency validation, contribution parsing, prefix/provenance checks, permission checks, duplicate ID checks, slot conflict checks, component tree validation, token fallback validation, payload budget checks | Package load, enable, configuration reload, or explicit package UI update only. |
| Explicit command/UI update work | showing/hiding a fixed panel, opening/dismissing a transient overlay, emitting a validated command intent, replacing a bounded component tree | User action or server-routed command/update paths; never synchronous typing paint. |
| Protocol/client update work | publishing accepted panel/component/overlay/token snapshots or deltas, applying versioned state to client caches | Typed protocol/client event handling outside paint/text-event handlers. |
| Paint/layout state read | computing `PaneSlotLayout` geometry, painting component trees, hit testing installed action regions, resolving already-installed theme tokens | Masonry hot paths may read cached inert state only. |
| Editor hot path work | keypress, text-event, caret/selection movement, local edit application, scroll, first local paint after input | Must remain Rust-known and client-first; no package JavaScript, package validation, package parsing, blocking IPC, full-document serialization, or package-authored native widget mutation. |

## Security and Authority Boundaries

- Slot declarations must target known Clay slots and preserve the mandatory `main` editor slot. Invalid slot names, duplicate exclusive slot claims, stale target panes, and hidden layout override keys are rejected with structured diagnostics.
- Fixed panels and transient overlays are inert declarations. Clay owns focus, dismissal, z-order, accessibility, native widgets, Masonry layout, and final composition.
- Component IDs, panel IDs, overlay IDs, token names, action targets, and package-owned state/config keys must use package prefixes unless Clay owns the `clay.*` namespace.
- Action targets must resolve to registered commands before a UI contribution becomes active; stale or unregistered actions are rejected or disabled.
- Accessibility/focus metadata is declarative and bounded. Packages do not receive raw input callbacks, focus callbacks, native widget IDs, or event-loop handles.
- Style/theme values must use typed theme tokens and typed component style variables. Raw CSS, raw style strings, raw colors without a typed token contract, HTML/script injection, Vello callbacks, Parley callbacks, and renderer callbacks are denied.
- Payload ceilings must cover panel descriptors, component trees, overlay descriptors, token declarations, and protocol snapshots/updates. Oversize payloads fail at package load/config/update or protocol update time, not in paint/layout.
- Provenance must be retained for diagnostics: package name, package version, API prefix, primitive kind, contribution ID, slot/component/overlay/action/token field, payload size, source path/manifest field when available, and failed rule.
- Package UI declarations grant no filesystem, network, shell, AI mutation, WASM, package-manager execution, package enable/disable, workspace mutation, raw Deno op, native widget, or client-side JavaScript authority.

## Rejected Implementation Shapes

- Do not add `MarkdownPreviewSidebar`, `MarkdownPaneLayout`, `MarkdownMasonryPanel`, `MarkdownThemeCss`, `MarkdownOverlay`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust shell/UI branch.
- Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, `Flex`, `Portal`, `Split`, native handles, layout callbacks, Vello callbacks, Parley callbacks, or raw op names as package APIs.
- Do not promote planned `clay:ui` APIs by wiring only raw ops or inventory rows. Runtime-backed public APIs require facade, op wrapper, server validation, reference docs, docs/index links, generated registry coverage, API inventory metadata, and tests.
- Do not add package validation, package parsing, configuration evaluation, JavaScript execution, or blocking IPC to Masonry paint/layout/pointer/scroll/key/text-event handlers.
- Do not treat hidden config keys, raw CSS, raw style strings, or arbitrary color strings as temporary package author APIs.

## Implementation Plan Summary

1. Add `clay:ui` facade/op boundaries for `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken` with package provenance and validator-owned diagnostics.
2. Add generic component catalog and typed theme-token modules that can reuse SDUI node semantics but separate public component declarations from Masonry widget internals.
3. Extend package record/conflict validation to understand panel/component/overlay/token IDs, slot targets, action targets, API dependencies, and payload budgets without making Markdown the architecture owner.
4. Compose fixed panels through `PaneSlotLayout` and transient overlays through a separate overlay layer while preserving the editor in `main`.
5. Update public docs, package guide, primitive registry/backlog/status docs, Clay JS API docs/registry, wiki pages, and deterministic tests before marking APIs runtime-backed.
6. Keep `PackageUiStateScope`, `PackageLayoutOverride`, hidden configuration keys, and user override APIs planned for Phase 18.4 unless fully promoted with the same public API contract.

## Verification

- Inventory reviewed: SDUI helpers/publication, shell slot state, fixed-sidebar compatibility bridge, command/action validation, package manifest permissions/provenance, package contribution descriptors/conflicts, Clay JS API registry machinery, and structural observability.
- Generic gap review: Phase 18.3 should implement reusable `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` primitives only.
- Deferred-surface review: `PackageUiStateScope`, `PackageLayoutOverride`, user override APIs, and hidden layout/style keys remain Phase 18.4 unless deliberately promoted with full docs/API/registry/tests.
- Hot-path review: package load/config/update work, explicit command/UI update work, protocol/client update work, paint/layout state reads, and editor hot-path work are classified separately; package JavaScript and package validation stay out of Masonry hot paths.
- Security review: slot, fixed/transient panel, component ID, action target, accessibility/focus metadata, style/theme token, payload ceiling, provenance, and package permission boundaries are recorded before implementation.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Command: `cargo test --test protocol primitives_docs:: --quiet`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.1 Shell/Layout Primitive Review](phase18.1-shell-layout-primitive-review.md)
- [Phase 18.2 Shell Runtime Primitive Review](phase18.2-shell-runtime-primitive-review.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Command Registry](command-registry.md)
- [Package Loading](package-loading.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Package Security Reference](../../reference/primitives/package-security.md)
