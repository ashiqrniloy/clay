# Phase 18.1 Shell/Layout Primitive Review

## Source

- `plans/024-Phase18.1-Clay-Shell-Working-Area-and-Package-UI-Layout-Architecture-Gate.md`
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/server-driven-ui.md`
- `docs/wiki/modules/rendering-primitives.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/configuration-runtime.md`
- `docs/wiki/modules/masonry-editor.md`
- `.agents/skills/project-patterns/references/mode-primitive-first.md`
- `.agents/skills/project-patterns/references/package-ui-layout.md`

## Overview

This page records the Phase 18.1 primitive-first review that seeded the final shell/layout architecture gate artifacts (`docs/reference/primitives/shell-layout-strategy.md`, registry/backlog rows, and package-guide contract updates). The review audits existing editor/package UI primitives, states what they can already do, and identifies only generic shell/layout primitive gaps needed for later phases.

The core conclusion is that existing SDUI, commands, behavior manifests, configuration, package loading, decoration, parse, and editor-root surfaces provide useful pieces, but they do not yet define a Clay-owned working area, pane/split tree, slot model, fixed/transient panel contract, component contribution catalog, package UI state scopes, or shell/theme token declaration model. Those gaps must be filled as reusable primitives. Do not add Markdown-specific Rust shell branches, Markdown preview sidebar special cases, or package-specific Masonry widget paths.

## Existing Primitive Inventory

| Primitive area | Current source paths | What shell/package UI can already do | Timing / hot-path classification | Security and validation boundary |
| --- | --- | --- | --- | --- |
| SDUI tree publication | `src/protocol/sdui.rs`, `src/server/sdui.rs`, `src/server/ops/sdui.rs`, `runtime/js/sdui.js`, `src/masonry_sdui.rs`, `docs/wiki/modules/server-driven-ui.md` | Publish inert `SduiTree` snapshots/updates with `Panel`, `Label`, `Button`, `List`, `EditorView`, `Flex`, and `Stack` nodes; bind an editor view to the current document; route buttons/lists as command intents. | Protocol/update time for snapshots and updates; paint time consumes already-applied `SduiNativeState`; no package JavaScript in paint/layout/input/text-event handlers. | Server validates tree structure, editor document binding, action sources, registered command IDs, stable IDs, and SDUI payload budgets; nodes are not Masonry widget handles. |
| Action intents and command registry | `src/protocol/sdui.rs`, `src/packages/commands.rs`, `src/server/ops/commands.rs`, `runtime/js/commands.js`, `runtime/js/keybindings.js`, `docs/wiki/modules/command-registry.md` | Represent SDUI actions as `SduiActionIntent` command IDs and package commands as package-prefixed metadata with labels, key bindings, routing policy, custom properties, and permissions. | Load/activation time for command metadata; explicit user action time for server-routed command execution; key routing uses installed inert manifests on the client-first editor hot path. | Requires `command-registration` for package command declarations; duplicate commands, undeclared permissions, ambiguous key bindings, client-first package command authority, raw ops, and executable callbacks are rejected. |
| Editor views and current root widget | `src/launch.rs`, `src/masonry_editor.rs`, `src/masonry_sdui.rs`, `docs/wiki/modules/masonry-editor.md` | Start one Masonry `NewWindow` rooted at one `EditorWidget`; paint native editor content, current SDUI overlay, and status chrome; use `EditorView` bindings to constrain the editable region when a matching document binding exists. | Root creation and bootstrap are startup time; `apply_connection_event` is protocol/update time; `layout` and `paint` are Masonry hot paths that consume local state only; pointer/text handling remains client-first. | `EditorWidget` owns native input/rendering and status state; packages never receive widget IDs, native handles, Vello/Parley callbacks, or direct layout mutation authority. |
| Fixed-sidebar SDUI paint path | `src/masonry_sdui.rs` (`SIDEBAR_WIDTH`, `SduiNativeState::paint`, `editor_region`, `editor_region_for_document`) | Render one left-side fixed-width panel/sidebar and reserve editor space when a reachable `EditorView` binds to the active document. | Paint/layout-adjacent native work only after state was applied; no validation, package JavaScript, package parsing, or IPC waits occur inside the paint path. | The sidebar is Clay-owned native code; package SDUI data is inert, bounded, and cannot provide raw CSS, arbitrary geometry code, native widget IDs, or client-side scripts. |
| Behavior manifests | `src/protocol/mod.rs`, `src/behavior/manifest.rs`, `src/client/mod.rs`, `docs/wiki/modules/behavior-manifests.md` | Install inert behavior manifests for key routing and Rust-known text transforms so local editor behavior remains deterministic. | Manifest publication/install is protocol/update time; `ClientFirstPredictable` rules execute on the client-first editor hot path without package JavaScript or synchronous IPC. | Manifest validation rejects duplicate commands, ambiguous key bindings, unsupported routing, executable transform fields, and package attempts to become arbitrary client-first code. |
| Keybindings | `src/server/ops/keybindings.rs`, `runtime/js/keybindings.js`, `src/packages/commands.rs`, `docs/wiki/modules/command-registry.md` | Bind keys to declared commands and preserve routing metadata for server-routed actions and local deterministic editor commands. | Configuration/load time for binding registration; keypress hot path reads installed local routing data only. | Package-owned key routes must target declared commands and preserve provenance; ambiguous cross-package bindings require deterministic rejection/priority policy. |
| Configuration runtime | `src/server/configuration.rs`, `src/server/ops/configuration.rs`, `runtime/js/configuration.js`, `docs/wiki/modules/configuration-runtime.md` | Load `~/.config/clay/init.js`, allow local relative `.js` modules, expose configuration state, and keep package/mode option APIs as explicit Clay JS surfaces or planned-unavailable stubs. | Startup/reload/configuration-change time only; configuration evaluation never runs in paint, layout, keypress, pointer, scroll, or text-event handlers. | Configuration cannot grant filesystem outside the configuration root, network, shell, AI, WASM, package enable/disable, raw ops, native widget handles, raw CSS, or client-side JavaScript authority. |
| Package loading and contribution descriptors | `src/packages/record/mod.rs`, `src/packages/conflict.rs`, `src/packages/service.rs`, `docs/wiki/modules/package-loading.md` | Validate package identity, `apiPrefix`, permissions, modes, commands, configuration keys, key routing, text transforms, SDUI region descriptors, decoration descriptors, docs, budgets, and API dependencies before enable/load. | Package validation time and enable/reload time only; no package validation runs in ordinary typing/paint/layout/scroll/text-event paths. | Retains package provenance, rejects duplicate prefixes/modes/commands/config keys/SDUI regions/decorations/behavior entries, enforces permission declarations, and denies raw ops/native handles/client JS. |
| Decoration transport and rendering | `src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/editor/surface/mod.rs`, `src/editor/layout.rs`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/rendering-primitives.md` | Publish inert viewport-bounded decoration chunks/spans for inline editor rendering with package provenance and style tokens. | Background/protocol update time for validation and publication; paint consumes already-applied local decoration state only. | Requires `render-decorations`; validates document version, byte ranges, style tokens, payload size, provenance, and rejects CSS, draw callbacks, Parley/Vello callbacks, and executable data. |
| Parse coordinator | `src/server/parse_coordinator.rs`, `src/protocol/parse.rs`, `runtime/js/parse.js`, `docs/wiki/modules/parse-coordinator.md` | Run package parse handlers server-side as cancellable background tasks and publish validated `IncrementalParseUpdate` / decoration data. | Background only; scheduling records metadata, aborts superseded tasks, and never blocks client-first input or Masonry paint/layout. | Requires `parse-document`; validates package/mode provenance, parse windows, memory budgets, stale versions, result payloads, and decoration update consistency. |

## What Existing Primitives Can Achieve Today

- A package can publish a bounded, inert SDUI panel tree containing labels, buttons, lists, stacks/flex rows, and a document-bound `EditorView` through `clay:sdui` helper objects and `publishTree`.
- Clay can validate package-owned command metadata and route SDUI button/list activations as registered command intents instead of executable UI callbacks.
- The current native app can compose an editor surface, a fixed left SDUI sidebar, and status chrome in one `EditorWidget` root while preserving client-owned input, caret, viewport, and paint authority.
- Package records can retain provenance and reject SDUI region collisions at package enable/load time, which is a useful precursor for future slot conflict handling.
- Configuration and planned package option APIs already provide the correct `~/.config/clay/init.js` boundary for future user overrides, but shell/layout options are not implemented yet.
- Decoration and parse primitives solve inline document rendering and background parser work; they should not be reused as shell/panel layout declarations.

## Generic Shell/Layout Primitive Gaps

The review found these reusable gaps before Phase 18.1 registry/backlog updates and Phase 18.2+ implementation work:

1. **`WorkingAreaLayout`** — Clay needs a package-facing working-area primitive above Masonry. Current startup creates one `NewWindow` with one root `EditorWidget`, but there is no canonical state model for a Clay working area, pane collection, active pane, or package/user layout defaults.
2. **`PaneSplitTree`** — Clay needs a generic split-tree primitive for panes/windows. Existing SDUI `Flex`/`Stack` nodes are component layout nodes, not editor-pane lifecycle/split state; they do not model pane IDs, active document bindings, split ratios, persistence, focus, or per-pane validation.
3. **`PaneSlotLayout`** — Clay needs a slot contract for mandatory `main` and optional `left`, `right`, `top`, and `bottom` slots. The current fixed sidebar is a hard-coded left region (`SIDEBAR_WIDTH`) rather than a reusable slot model with side selection, fixed/transient behavior, collision handling, and user overrides.
4. **`PanelContribution`** — Existing `SduiPanelStatusContribution` can carry inert panel-like trees, but it does not declare fixed panel ownership, default visibility, preferred slot, component IDs, title/icon metadata, or package/user precedence in a generic shell model.
5. **`TransientOverlayContribution`** — There is no generic primitive for overlays, popovers, command palette-like panels, dismissible previews, or pointer/focus capture rules. Transient UI must be Clay-owned state, not package-created Masonry widgets or client JavaScript.
6. **`ComponentContribution`** — SDUI has node kinds, but Clay lacks a component catalog primitive that maps package-owned component IDs to validated schema, action targets, state scopes, docs metadata, accessibility labels, and renderer-safe native component implementations.
7. **`PackageThemeTokenDeclaration`** — Decoration style tokens exist for inline rendering, but there is no shell/layout theme-token declaration primitive for package panel styles, spacing, semantic colors, typography, density, or component variables. The primitive must use typed Clay tokens and must reject raw CSS.
8. **Package UI/state contribution categories** — Future panels need scoped state/data declarations (`package-global`, `user-config`, `workspace`, `document`, `pane`, `component`, and `transient-overlay`) plus size/payload budgets, provenance, and deterministic reset/cleanup rules.
9. **Layout conflict and precedence contract** — Current package conflict checks can reject duplicate SDUI regions, commands, config keys, and decorations, but Phase 18.1 still needs deterministic precedence across Clay safety defaults, user configuration, active major mode, compatible minor modes, global packages, and package defaults.
10. **Focus/input/action routing for shell UI** — Existing text/key routing and SDUI action intents are reusable, but a slot-aware shell must define which actions are registered, how focus moves between panels and editor panes, how transient overlays dismiss, and which input routes remain client-first.

All new names must be generic. Acceptable names include `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, `PackageUiStateScope`, and `PackageLayoutOverride`. Rejected names include `MarkdownPreviewSidebar`, `MarkdownPaneLayout`, `MarkdownMasonryPanel`, `MarkdownThemeCss`, `MarkdownOverlay`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust shell-layout branch.

## Hot-Path Classification

- **Configuration/load time:** package manifest validation, contribution descriptor validation, permission checks, command/keybinding/config registration, and future layout/theme declaration validation.
- **Package validation time:** shell/layout declarations must be schema-validated, permission-checked, package-prefixed, provenance-retaining, bounded, and conflict-checked before activation.
- **Protocol/update time:** SDUI snapshots/updates, behavior manifest installs, decoration updates, layout state updates, pane-tree updates, and panel visibility changes are applied to local native state outside paint/text-event handlers.
- **Layout/update time:** Clay may recompute pane/slot geometry from already-validated inert state, but package JavaScript must not run during Masonry layout and package payload validation must not occur in the layout hot path.
- **Paint time:** `EditorWidget::paint`, `SduiNativeState::paint`, and editor decoration rendering consume local cached state only. They must not run package JavaScript, parse packages, wait for IPC, deserialize full package payloads, or mutate package-owned widgets.
- **Client-first editor hot path:** keypress, text-event, pointer, scroll, caret, selection, local edit, and first paint after local edit use Rust-known behavior manifests and native state only. Proposed package JavaScript remains outside paint/layout/input/text-event handlers.

## Security and Authority Boundaries

- **UI/layout declarations:** Server-side validation must reject malformed schemas, duplicate or unprefixed IDs, duplicate slot claims without explicit precedence, unknown components, oversize payloads, native widget handles, Masonry widget constructors, Vello/Parley callbacks, raw CSS, script strings, client-side JavaScript hooks, and raw `Deno.core.ops` references.
- **Action intents:** Package UI actions must target registered package or Clay commands through inert `SduiActionIntent` / command metadata. Unknown commands, duplicate commands, undeclared permissions, client-first package command authority, and unregistered action targets must be rejected with diagnostics.
- **Style/theme tokens:** Packages may request typed semantic Clay tokens or package-prefixed theme token declarations only after the primitive exists. They cannot provide CSS, arbitrary color/spacing strings that bypass validation, renderer callbacks, draw commands, or native style mutation.
- **Package state/data declarations:** Future state scopes must be explicit, bounded, package-prefixed, and validated. State declarations cannot grant filesystem, network, shell, AI, WASM, workspace mutation, package enable/disable, raw-op, native-widget, or client-side JavaScript authority.
- **Package/user overrides:** User overrides must flow through documented Clay JS configuration APIs from `~/.config/clay/init.js`; hidden JSON/TOML layout keys and package-default precedence hacks are rejected. Clay shell safety invariants remain above package/user requests.

## Review Decision

Proceed to define Phase 18.1 shell/layout docs and registry/backlog rows using existing primitives as building blocks, but add only generic reusable shell/layout primitives. Existing SDUI remains the inert component/tree publication substrate for package UI content; Masonry remains a Clay-owned native implementation substrate; the new shell model should organize working areas, panes, slots, panels, components, overlays, state scopes, action routing, and theme tokens above SDUI/Masonry without exposing native widget authority.

No Markdown-specific or package-specific Rust UI branch is required by this review. Markdown preview/status behavior should consume future `PanelContribution` / `PaneSlotLayout` primitives the same way another package would.

## Verification

- Inventory reviewed: SDUI tree publication, action intents, editor views, behavior manifests, command registry, keybindings, configuration runtime, package loading, decoration transport, parse coordinator, current `EditorWidget` root, and fixed-sidebar SDUI paint path.
- Hot-path review: package load/configuration/validation work stays out of keypress, paint, layout, pointer, scroll, and text-event handlers; paint/layout consume local inert state only.
- Security review: package UI/layout, action, style/theme, package state/data, and user override declarations remain server-validated, prefix/provenance-aware, bounded, and denied raw Masonry/native/CSS/client-JS/raw-op authority.
- Generic gap review: future work should add reusable primitives such as `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, and package UI/state contribution categories rather than Markdown-specific layout code.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- `cargo test --test protocol primitives_docs::`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Rendering Primitives](rendering-primitives.md)
- [Package Loading](package-loading.md)
- [Command Registry](command-registry.md)
- [Configuration Runtime](configuration-runtime.md)
- [Masonry Editor Widget Status Observability](masonry-editor.md)
