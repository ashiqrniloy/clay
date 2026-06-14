# Clay Shell and Package UI/Layout Strategy

Status: Phase 18.1 architecture reference with Phase 18.2 internal shell runtime progress, Phase 18.3 runtime-backed package UI contribution progress, and Phase 18.4 runtime-backed package input/state/configuration progress. This document is a planning and validation contract plus the primitive reference for the `clay:ui` package-facing shell facade. Examples marked **Planned** must not be treated as callable code until a later implementation phase documents and registers the corresponding Clay JS API. Phase 18.2 implemented internal Rust `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state in `src/shell/layout.rs` plus the native shell root in `src/masonry_shell.rs`; those internals are not package-facing JavaScript APIs. Phase 18.3 implements runtime-backed public APIs for package `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` registration with generated public registry/API pages. Phase 18.4 implements runtime-backed public APIs for `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, and package-owned options while keeping direct working-area/split/pane-slot mutation and durable state-value mutation planned.

## Sources and Evidence

- Approved decision: `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.
- Primitive review: `docs/wiki/modules/phase18.1-shell-layout-primitive-review.md`.
- Phase 18.2 runtime baseline: `src/main.rs` starts a `ClayShellWidget` root through `NewWidget`/`NewWindow`; `src/masonry_shell.rs` registers `EditorWidget` as a child component; `src/shell/layout.rs` owns internal `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state; `src/masonry_editor.rs` still owns editor hot-path behavior, SDUI bridge painting, and status bar; `src/masonry_sdui.rs` maps the temporary SDUI sidebar through Clay slot geometry; `src/protocol/sdui.rs` defines inert SDUI panels, actions, and editor views.
- Phase 18.3 runtime-backed package UI baseline: `runtime/js/ui.ts` exposes `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`; `src/server/ops/ui.rs` owns the op wrappers; `src/server/ui.rs` validates package provenance, declarations, registered action targets, typed style variables, and package theme tokens; `src/shell/components.rs` owns the Clay component catalog; `src/shell/theme.rs` owns typed core/package token resolution; `src/shell/package_ui.rs` composes accepted panels/overlays through shell-owned runtime state; `src/masonry_sdui.rs` renders the current native compatibility surface from validated component/theme state.
- Current package authoring guide: `docs/reference/packages/creating-packages.md`.
- Masonry documentation reviewed through Context7 `/linebender/xilem` on 2026-06-09: `Widget` trait, container widget methods, `masonry_winit` `NewWindow`/root-widget startup, `RenderRoot` widget-tree passes, `Flex`, `Portal`, typed properties, and actions. The docs confirm Masonry is Clay's native widget/layout/rendering substrate for building higher-level GUI libraries, not the package author API.

## Phase 18.2/18.3 Runtime Status

**Implemented/runtime-internal in Phase 18.2:**

- `src/main.rs` starts a Clay-owned `ClayShellWidget` as the native root widget and registers `EditorWidget` as the editor child component instead of treating the editor as the application shell.
- `src/shell/layout.rs` owns internal Rust `WorkingAreaLayout` state for one working area, a layout version, the active/root pane, and the editor component binding.
- `PaneSplitTree` supports the default one-leaf tree plus generic horizontal/vertical split nodes with stable pane IDs, bounded split ratios, duplicate-pane rejection, oversize tree rejection, and deterministic geometry calculation.
- `PaneSlotLayout` keeps exactly one mandatory `main` slot and optional fixed `left`, `right`, `top`, and `bottom` slots with finite size, min/max clamp, visibility, collapse, and user-resize state.
- The temporary SDUI sidebar is bridged through Clay-owned left-slot geometry; it is not a package slot API and is expected to be replaced by Phase 18.3 panel/component contributions.
- Inert local layout updates and structural observability are internal test/agent inspection surfaces. Observations record layout shape, slot geometry, and component binding without exposing document text, native widget handles, Masonry widget IDs, raw action authority, raw CSS, raw ops, renderer callbacks, or executable package code.

**Implemented/runtime-backed public APIs in Phase 18.3:**

- `clay:ui` is a curated server-side facade module, not a raw-op surface.
- `PanelContribution` / `serverRegisterPanelContribution` accepts package-prefixed fixed panels for `left`, `right`, `top`, or `bottom`, validates package provenance, registered action targets, bounded component trees, and payload budgets, and stores accepted declarations for shell runtime composition.
- `ComponentContribution` / `serverRegisterComponentContribution` accepts bounded component trees using the Phase 18.3 Clay component catalog: `editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, and `statusItem`; `table`, `dropdown`, `collapse`, and `modal` are explicitly deferred.
- `TransientOverlayContribution` / `serverRegisterTransientOverlayContribution` accepts package-prefixed overlays with `working-area`, `active-pane`, `main`, or `pointer` anchors plus `none`, `restore`, or `trap` focus policies and `manual`, `escape`, `outside`, or `escape-or-outside` dismissal policies.
- `PackageThemeTokenDeclaration` / `serverRegisterThemeToken` accepts package-prefixed typed token declarations with `color-role`, `spacing`, `radius`, `typography`, or `opacity` types and same-type Clay core fallbacks.
- Package manifest validation accepts Phase 18.3 `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, and `themeTokens` descriptors for load-time diagnostics and conflict checks.
- Accepted fixed panels compose into `PaneSlotLayout` geometry while preserving the mandatory `main` editor slot; transient overlays render separately and do not consume fixed slot geometry.

**Still planned/package-facing after Phase 18.3:** public callable working-area, pane-split, and pane-slot layout mutation/default APIs; Historical Phase 18.3 handoff also kept `PackageLayoutOverride`; user shell-layout/theme-token override configuration APIs; and server-to-client package UI publication beyond the current local validated runtime/shell state were still planned after Phase 18.3. Planned-only `clay.ui.*` inventory entries remain `status = "planned"`, `registry_public = false`, and backed by `op_clay_runtime_unavailable` for working-area, split-tree, and direct pane-slot mutation; the four Phase 18.3 contribution entries are `status = "runtime-backed"`, `registry_public = true`, documented under `docs/reference/clay-js-api/ui/`, and generated into the public registry. Phase 18.4 promotes `PackageInputContribution` through `clay.ui.serverRegisterInputContribution`, `PackageUiStateScope` through `clay.ui.serverRegisterUiStateScope`, `PackageLayoutOverride` through `clay.ui.serverSetLayoutOverride`, and package-owned options through `clay.configuration.setPackageOption`; these Phase 18.4 entries are `status = "runtime-backed"`, `registry_public = true`; both Phase 18.4 entries are `status = "runtime-backed"`, `registry_public = true`, and preserve the same no-hot-path inert package UI/configuration boundary.

```rust
// Implemented/runtime-internal Rust shape, not a package-facing JavaScript API.
let editor_widget = EditorWidget::with_initial_state(initial_state).with_edit_queue(queue);
let shell_widget = ClayShellWidget::single_editor(editor_widget);
let editor_widget_id = shell_widget.editor_widget_id();
let root_widget = NewWidget::new(shell_widget);
```

## Architecture Boundary

Clay owns the package-facing shell vocabulary and compiles validated package declarations into native UI state. Packages do not own native widgets.

```text
Package JS declarations and server-side handlers
  -> Clay server validation, composition, provenance, and conflict checks
  -> bounded inert shell/layout/UI/action/state/style declarations
  -> Clay client state updates
  -> Clay-owned Masonry widgets, Vello painting, and Parley text layout
```

Masonry remains an implementation substrate. Likely internal substrates include `RenderRoot`, `Widget`, Masonry container widgets and container layout methods, `Split`, `Flex`, `Grid`, `ZStack`, `Portal`, typed widget properties, and Masonry actions. These names are evidence for implementation feasibility; they are not stable public package APIs. Clay may use built-in Masonry widgets or Clay-owned custom container widgets when shell invariants require stricter validation than a generic widget provides.

Package authors must use Clay concepts: working areas, panes, slots, panels, components, command/action intents, state scopes, and theme tokens. They must not depend on Masonry widget IDs, native widget handles, layout pass timing, typed property internals, Vello callbacks, Parley callbacks, or the shape of Clay's future widget tree.

## Vocabulary

### Application Shell

The Clay application shell is the Clay-owned root UI composition inside an OS window. In the Phase 18.2 runtime, `ClayShellWidget` is the native root widget and `EditorWidget` is registered as an editor component child inside that shell instead of acting as the whole application shell.

### Working Area

The working area is the drawable application region managed by Clay inside a native window. It excludes OS chrome and is the root of Clay's editor/package UI composition. A working area owns one pane/split tree.

### Pane/Split Tree

A `PaneSplitTree` is a Clay-owned tree whose leaves are panes and whose internal nodes are horizontal or vertical splits. Phase 18.2 implements the internal Rust state and geometry helpers for the one-leaf default and generic split topology; package-facing multi-pane editing and slot-targeted package placement remain planned.

```text
WorkingArea
└── PaneSplitTree
    ├── Pane
    │   ├── top slot?      fixed or transient panel/component region
    │   ├── left slot?     fixed or transient panel/component region
    │   ├── main slot      mandatory primary component region
    │   ├── right slot?    fixed or transient panel/component region
    │   └── bottom slot?   fixed or transient panel/component region
    └── Split(Pane, Pane)
```

The pane/window layout term means the logical layout inside a Clay window: working area -> pane/split tree -> leaf pane -> slots. It does not grant packages a native window handle, Masonry widget handle, or OS window mutation authority.

### Pane

A pane is a leaf in the pane/split tree. It owns exactly one mandatory `main` container and may own optional `left`, `right`, `top`, and `bottom` panel slots. Pane state may include active component, focus metadata, panel visibility, split ratios, and transient overlay state when later phases implement those fields.

### Slots

Slots are Clay-defined attachment points in a pane:

- `main` is mandatory. It normally contains the active editor view or another primary Clay component.
- `left` is optional and intended for side panels such as file trees or outlines.
- `right` is optional and intended for side panels such as previews or inspectors.
- `top` is optional and intended for tool/status/find-like regions that belong above the main content.
- `bottom` is optional and intended for diagnostics, output, status, or console-like regions.

Slots are declarations in the Clay layout model, not Masonry containers exposed to packages. Clay validates slot ownership, collision, visibility, and persistence before client state changes are applied.

### Fixed Panels

Fixed panels participate in layout and reduce the size of the `main` slot while visible. Examples include a file tree, outline, preview pane, diagnostics list, or output panel. A planned `PanelContribution` may request a fixed default, but Clay and user configuration determine the final composed layout.

### Transient Panels and Overlays

Transient panels overlay the pane or working area and are dismissible or focus-scoped. Examples include command palettes, dropdowns, hover documentation, modals, temporary find/replace bars, and menus. A planned `TransientOverlayContribution` describes transient UI as inert data; Clay owns focus trapping, dismissal, z-order, accessibility, and native overlay implementation.

### Components and Elements

A component is a Clay package-facing UI declaration that Clay maps to native widgets internally. Examples include `EditorView`, `Panel`, `Label`, `Button`, `List`, `Flex`, `Stack`, `Scroll`, `StatusItem`, `Table`, `Dropdown`, `Collapse`, and `Modal` as planned or existing categories. Elements are the serializable nodes/children within a component tree. Component declarations must be bounded, schema-validated, and prefix/provenance-aware.

Packages must not treat component names as Masonry widget types. The same package-facing `Panel` could be implemented by a Masonry `Flex`, `Grid`, `Portal`, custom widget, or a combination of widgets without changing the package contract.

### Action Intents

Actions are inert command intents. Package UI may declare an action target such as `markdown.togglePreview`, but Clay validates that the command is registered, package-prefixed when package-owned, permission-compatible, and safe for the declared routing policy. UI actions carry bounded primitive arguments only. Unregistered action targets are rejected.

### Package State Scopes

Package UI/layout state must be assigned an explicit scope before it affects the shell:

| Scope | Use | Phase 18.3 package-facing status |
| --- | --- | --- |
| `package-global` | Package defaults and feature flags | Planned |
| `user-config` | User overrides from `~/.config/clay/init.js` Clay APIs | Planned |
| `workspace` | Workspace-local package settings | Planned |
| `document` | Open-document metadata such as parse status | Partly exists for document/parse primitives |
| `pane` | Active view, panel visibility, split ratios | Internal shell/client state exists; package API planned |
| `component` | Component-local selection/open state | Internal editor component binding exists; package API planned |
| `transient-overlay` | Dismissible overlay/menu/modal state | Planned |

Hidden globals and ad hoc package state are not part of the architecture. Later phases must document any implemented state API as a Clay JS API or as an internal shell/client state field.

### Style and Theme Tokens

Package styling uses typed theme/style tokens, not CSS. A package may declare semantic tokens such as `markdown.heading.1` with fallbacks to Clay tokens such as `text.heading1`. Component style variables are typed fields such as variant, padding, background token, content color token, border token, corner radius, spacing, and font role where supported.

Clay maps these tokens to Masonry typed properties, Vello paint parameters, and Parley text styling internally. Unknown tokens, raw CSS, raw colors without a typed token contract, style strings, and renderer callbacks are rejected.

## Layout Conflict and Precedence Contract

Status: internal Phase 18.2 runtime invariants plus Phase 18.3 runtime-backed panel/component/overlay/token contribution validators and Phase 18.4 runtime-backed input/state-scope/layout-override/package-option validators. Phase 18.2 enforces local shell safety for layout versioning, pane-tree shape, split ratios, slot geometry, and stale/oversize update rejection. Phase 18.3 makes the four contribution APIs callable through `clay:ui`; Phase 18.4 adds callable input contribution, UI state-scope schema/lifecycle, layout override, and package option APIs, but it does not expose working-area mutation, split-tree mutation, direct pane-slot layout mutation, hidden configuration keys, package enable/disable authority, or durable workspace/document state-value mutation.

All shell/layout composition must be deterministic, package-prefix-aware, provenance-preserving, and independent of package load order except where a later implemented API documents an explicit priority field. Higher-precedence declarations may hide, move, or override lower-precedence defaults only after the same schema, permission, slot, action, state, and style validation succeeds.

Planned precedence direction:

1. Clay shell safety invariants and hard prohibitions
2. User configuration through documented Clay JS APIs
3. Active major mode layout defaults
4. Compatible minor mode contributions
5. Global package contributions
6. Package fallback/defaults

Precedence meaning:

- Clay shell safety invariants always win. Clay preserves a valid working area, one pane/split tree, at least one pane, exactly one mandatory `main` slot per pane, bounded payload sizes, focus safety, accessibility requirements, and the Masonry/non-authority boundary. Raw CSS, raw ops, native widget handles, client-side JavaScript, renderer callbacks, unsupported state scopes, and oversize payloads are rejected before precedence is considered.
- User configuration is accepted only through documented `~/.config/clay/init.js` Clay JS APIs. User configuration can override package/default layout requests such as default visibility, preferred side slot, panel order, or token mapping when the target package declares the underlying option and the override stays within Clay shell invariants. User configuration cannot grant permissions or bypass validation.
- The active major mode owns the primary document experience for the current document/pane. Major mode defaults may request the `main` component, companion fixed panels, transient overlays, action targets, state scopes, and theme tokens for that mode.
- Compatible minor modes may add non-exclusive panels, overlays, actions, input hints, state, and tokens only when they declare compatibility with the active major mode. Minor modes must not replace the active major mode's `main` component or exclusive slot/default unless a future explicit override policy documents that behavior.
- Global package contributions provide package-wide or workspace-wide UI such as file trees, diagnostics, package status, or search. They can occupy slots only through explicit non-conflicting claims or documented user configuration.
- Package fallback/defaults are lowest-precedence defaults shipped by packages so one-line loading works without user boilerplate. They are not guaranteed final layout and must tolerate Clay or user overrides.

Conflict categories and planned handling:

| Conflict category | Deterministic handling |
| --- | --- |
| Shell safety invariant violation | Reject with diagnostics; no package/user declaration can remove the `main` slot, mutate native layout, or exceed payload budgets. |
| Duplicate shell slot claim | Reject ambiguous exclusive claims or require explicit slot priority/precedence metadata. Multiple panels in one side slot are allowed only when a later Clay slot container contract defines ordering; packages never win by load order alone. |
| Fixed/transient panel mismatch | Reject declarations that use a transient overlay as persistent layout chrome, make a fixed panel behave like a focus-trapping modal, or omit required dismissal/focus metadata for overlays. |
| Duplicate component ID or overlay ID | Reject unless the same package version replaces its own contribution through a documented update path. Component IDs and overlay IDs must be package-prefixed. |
| Duplicate command/action ID or unregistered action target | Reject package-owned duplicate command IDs, reject unregistered action targets, and reject action intents whose permissions or routing policy do not match the command registry. |
| Unsupported or undeclared state scope | Reject hidden globals, ad hoc state keys, unsupported state scopes, and state mutations outside their documented owner/lifecycle. |
| Unknown or duplicate style/theme token | Reject unknown tokens, type-incompatible fallbacks, duplicate package token names, raw CSS/style strings, raw colors without a typed token contract, and renderer callbacks. |
| Package/user override bypass | Reject overrides that target undeclared options, hidden keys, unregistered components, unknown style tokens, unsupported slots, or authorities the package did not declare. |

Unresolved implementation policy areas are deliberately deferred to later phases: exact multi-panel ordering inside one side slot, pane selector syntax, persisted split-ratio storage, cross-window layout sync, overlay z-order buckets, and whether any future package priority field is allowed. Those phases must document the chosen rule and add deterministic tests before enabling the behavior.

## Configuration and User Override Surfaces

Status: Phase 18.4 implements the first public configuration contract for package UI/input/action/state defaults. Phase 18.1 did not introduce a callable shell/layout configuration API; Phase 18.2 still does not introduce a callable shell/layout configuration API. Phase 18.3 introduces package declarations for panel defaults and theme tokens but still does not introduce a user-visible panel visibility/default-slot/theme-token override API. Historical Phase 18.3 status: `clay.ui.serverSetLayoutOverride` and `clay.configuration.setPackageOption` were planned stub only, `op_clay_runtime_unavailable`, non-registry-public, and had no `docs/reference/clay-js-api/ui/` page for override behavior. Phase 18.4 promotes the supported override/package-option subset through documented runtime-backed APIs from `~/.config/clay/init.js`; shell working-area mutation, pane selectors, multi-panel ordering, overlay z-order, cross-window layout, package enable/disable, and state-value mutation remain planned/deferred.

Implemented and planned shell/layout configuration surfaces:

| Surface | Clay JS API trace | Configurable behavior | Phase 18.4 public status |
| --- | --- | --- | --- |
| Package-owned option | `clay.configuration.setPackageOption` | Package-prefixed typed options for `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`. | Runtime-backed public API; documented under `docs/reference/clay-js-api/configuration/set-package-option.md` with generated registry coverage. |
| Layout override | `clay.ui.serverSetLayoutOverride` / `PackageLayoutOverride` | User, mode, global package, or package-default override for `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, or `fallback` through `~/.config/clay/init.js`. | Runtime-backed public API; documented under `docs/reference/clay-js-api/ui/server-set-layout-override.md` with generated registry coverage. |
| Theme token declaration | `clay.ui.serverRegisterThemeToken` / `PackageThemeTokenDeclaration` | Package declares typed tokens that `serverSetLayoutOverride` may remap only when the registered fallback type is compatible. | Implemented/runtime-backed public API for declarations; remaps are validated configuration/update work. |
| UI state scope | `clay.ui.serverRegisterUiStateScope` / `PackageUiStateScope` | Package declares allowed state scopes such as `user-config`, `pane`, `component`, or `transient-overlay`. | Runtime-backed inert schema/lifecycle declaration; durable workspace/document/user-config mutation and persisted values remain deferred unless separately documented. |
| Working area, split tree, and direct pane slot mutation | `clay.ui.serverRegisterWorkingAreaLayout`, `clay.ui.serverRegisterPaneSplitTree`, `clay.ui.serverSetPaneSlotLayout` | Direct shell topology mutation/default registration. | Planned stubs only; not registry-public. |

Every concrete shell/layout setting must remain a Clay JS API with `custom_properties`, types, defaults, allowed values, examples, errors, key binding metadata, permissions/security notes, backing Rust/op/facade metadata, `docs/index.md` links, generated registry coverage, and lookup metadata before users or agents can depend on it.

All hidden JSON/TOML/ad hoc layout, style, input, or theme keys are rejected. This includes keys named like `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `preview.position`, `preview.defaultVisibility`, or `theme.markdown.heading.1` when they bypass documented Clay JS APIs. User overrides cannot grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw Deno ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, client-side JavaScript, unsupported slots, unknown style tokens, unregistered actions, or hidden package state authority.

Configuration evaluation happens at startup, package load, configuration reload, or explicit setting-change time. Masonry paint/layout, pointer, scroll, keypress, text-event handling, and ordinary editor hot paths read already-validated inert state only; they must not run package JavaScript, wait on configuration IPC, parse package metadata, mutate native layout from package code, or recompute layout from user JavaScript.

## Clay JS API Inventory Status

Status: mixed Phase 18.4 implementation. Phase 18.2 implemented internal Rust runtime primitives, Phase 18.3 adds a runtime-backed `clay:ui` contribution facade for `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` registration, and Phase 18.4 adds runtime-backed `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, and `PackageOwnedConfiguration` APIs. Working-area, split-tree, direct pane-slot layout defaults, pane selectors, multi-panel ordering, overlay z-order, cross-window layout, package enable/disable, and durable state-value mutation remain planned inventory surfaces. The inventory verifies names and metadata so later shell/layout implementation phases can promote the remaining APIs without exposing raw Rust functions or raw `Deno.core.ops`. The implemented `clay:ui` and `clay:configuration` APIs now create `docs/reference/clay-js-api/` Markdown pages, generated public registry entries, and lookup-visible registry-public rows while keeping direct shell mutation/state-value APIs planned.

Rust visibility audit: Phase 18.2 introduces no new public server-side Rust shell/layout functions. `src/shell/mod.rs` and `src/shell/layout.rs` remain `pub(crate)` internal runtime state. `src/masonry_shell.rs::ClayShellWidget`, `ClayShellWidget::single_editor`, `ClayShellWidget::editor_widget_id`, and `ClayShellWidget::focus_fallback_widget_id` are Rust-public only for the Cargo package's binary/library boundary so `src/main.rs` can construct the native shell and route focus/actions; the module/type are `#[doc(hidden)]`, native-only, not server-side APIs, not package-extensibility APIs, and not backed by a `deno_core` op, JS facade, Markdown API page, docs-index registry link, generated registry entry, or lookup metadata. The behavior-changing shell update and observation helpers (`apply_layout_update`, `observable_snapshot`, `WorkingAreaLayoutUpdate`, `WorkingAreaLayoutObservation`) remain `pub(crate)` and are explicitly not callable from JavaScript.

| Primitive category | JS module specifier | Planned JS export/callable | Stable registry ID | User-facing name | Phase 18.3 public status |
| --- | --- | --- | --- | --- | --- |
| `WorkingAreaLayout` | `clay:ui` | `serverRegisterWorkingAreaLayout` | `clay.ui.serverRegisterWorkingAreaLayout` | Register Working Area Layout | Planned stub only; `op_clay_runtime_unavailable`; not registry-public. |
| `PaneSplitTree` | `clay:ui` | `serverRegisterPaneSplitTree` | `clay.ui.serverRegisterPaneSplitTree` | Register Pane Split Tree | Planned stub only; `op_clay_runtime_unavailable`; not registry-public. |
| `PaneSlotLayout` | `clay:ui` | `serverSetPaneSlotLayout` | `clay.ui.serverSetPaneSlotLayout` | Set Pane Slot Layout | Planned stub only; `op_clay_runtime_unavailable`; not registry-public. |
| `PanelContribution` | `clay:ui` | `serverRegisterPanelContribution` | `clay.ui.serverRegisterPanelContribution` | Register Panel Contribution | Runtime-backed public API; `op_clay_ui_register_panel_contribution`; registry-public with per-API docs. |
| `ComponentContribution` | `clay:ui` | `serverRegisterComponentContribution` | `clay.ui.serverRegisterComponentContribution` | Register Component Contribution | Runtime-backed public API; `op_clay_ui_register_component_contribution`; registry-public with per-API docs. |
| `TransientOverlayContribution` | `clay:ui` | `serverRegisterTransientOverlayContribution` | `clay.ui.serverRegisterTransientOverlayContribution` | Register Transient Overlay Contribution | Runtime-backed public API; `op_clay_ui_register_transient_overlay_contribution`; registry-public with per-API docs. |
| `PackageThemeTokenDeclaration` | `clay:ui` | `serverRegisterThemeToken` | `clay.ui.serverRegisterThemeToken` | Register Theme Token | Runtime-backed public API; `op_clay_ui_register_theme_token`; registry-public with per-API docs. |
| `PackageUiStateScope` | `clay:ui` | `serverRegisterUiStateScope` | `clay.ui.serverRegisterUiStateScope` | Register UI State Scope | Runtime-backed inert schema/lifecycle declaration; registry-public with facade/op/docs/tests. |
| `PackageLayoutOverride` | `clay:ui` | `serverSetLayoutOverride` | `clay.ui.serverSetLayoutOverride` | Set Layout Override | Runtime-backed public API; `op_clay_ui_set_layout_override`; registry-public with per-API docs. |

The naming layers are deliberately distinct: the module specifier groups imports, the lower-camel-case export is what JavaScript would call, the stable registry ID is the globally searchable `clay.ui.*` identifier, and the user-facing name is the English help/search label. Raw Rust paths, raw op names, Masonry type names, protocol DTO names, and generated registry IDs must not become package-facing callable names.

Package-owned shell/layout IDs inside declarations, action targets, component IDs, token names, state keys, and override targets must use package prefixes such as `markdown.preview` or `markdown.togglePreview`. First-party Clay APIs may use `clay.*`; packages must not claim the Clay namespace, unprefixed IDs, native widget IDs, raw Rust function names, or raw op names.

Every future promoted API must add full Markdown documentation under `docs/reference/clay-js-api/`, link it from `docs/index.md`, update generated registry artifacts, provide lookup metadata, list key binding metadata and `custom_properties`, document backing Rust/op/facade paths, and preserve the same bounded inert payload, server validation, Clay-owned Masonry rendering, no-hot-path package-JS, raw-op denial, native-widget denial, client-JS denial, style-token constraint, and action-target validation requirements recorded here. The four Phase 18.3 package contribution APIs plus Phase 18.4 `serverRegisterInputContribution`, `serverRegisterUiStateScope`, `serverSetLayoutOverride`, and `setPackageOption` APIs satisfy this contract under `docs/reference/clay-js-api/ui/` and `docs/reference/clay-js-api/configuration/`.

## State Scope Contract

Package UI/layout state must declare one of the supported scopes and must use package-prefixed keys when package-owned. State values are bounded inert data, not native widget handles or executable callbacks.

| Scope | Planned owner/lifecycle | Allowed examples | Rejections |
| --- | --- | --- | --- |
| `package-global` | Server/package defaults for an enabled package | default feature flags, package fallback layout values | hidden globals, cross-package mutable state, filesystem/network/shell/AI/WASM authority |
| `user-config` | Documented `~/.config/clay/init.js` Clay JS APIs | default preview slot, panel visibility default, token remap | hidden JSON/TOML/ad hoc keys, permission grants, unknown options |
| `workspace` | Future workspace-scoped Clay APIs | workspace package settings, workspace search panel defaults | implicit workspace mutation, undeclared persistence, raw filesystem authority |
| `document` | Server/document primitives and protocol metadata | parse status, document diagnostics summary, document-specific preview mode | full-document UI snapshots for ordinary edits, stale document versions |
| `pane` | Clay shell/client state plus validated server updates | active component, panel visibility, split ratios, focused panel | package-owned native widget state, unsupported pane selectors |
| `component` | Clay shell/client transient state unless persisted by a documented API | selected tab, open list section, local selection in a panel | hidden persisted state, action arguments that smuggle authority |
| `transient-overlay` | Clay shell/client transient overlay state | command palette open state, dropdown selection, modal dismissal | non-dismissible overlays without Clay authority, z-order/focus traps without metadata |

Later phases may mark individual fields within a scope as client-owned, server-owned, persisted, or ephemeral. Unsupported UI state scope names are rejected with package, contribution, key, scope, and source diagnostics before they affect the shell.

## Input and Action Contract

Input routing remains Clay-owned. Packages declare input interests and action intents; they do not receive raw arbitrary client input handlers.

- Editor text behavior uses behavior manifests and Rust-known client-first transforms for predictable hot-path editing. Package JavaScript does not run in keypress or text-event handlers.
- Component/panel pointer, focus, menu, and button interactions are expressed as inert `SduiActionIntent`-style command intents or future `clay:ui` action intents. The client may enqueue the validated intent, but it does not run package code locally.
- Every action target must resolve to a registered command before the UI declaration becomes active. Package command IDs must use the package prefix, and target command permissions/routing policy must be compatible with the component/action location.
- Action arguments must be bounded primitive data such as strings, numbers, booleans, arrays/maps within schema limits, document IDs, pane IDs, component IDs, or token IDs. They must not contain callbacks, raw op names, native handles, filesystem paths that bypass document/workspace APIs, or executable code.
- Focus and input precedence starts with Clay shell safety and the focused pane/component, then applies validated user configuration, active major mode behavior, compatible minor mode behavior, and global package contributions. Ambiguous key/pointer claims require explicit routing policy or are rejected/disabled with diagnostics.
- Stale action intents are rejected or disabled when their target command, component, package, pane, or document provenance no longer matches the active validated state.

## Style and Theme Token Contract

Style/theme declarations are typed tokens and typed component style variables. They are validated at package load, configuration, or UI update time and are applied as inert native state.

- Clay core token names such as `text.*`, `surface.*`, `border.*`, `accent.*`, `diagnostic.*`, `code.*`, and `selection.*` are reserved for Clay-owned tokens.
- Package-owned token names must use the package prefix, such as `markdown.heading.1` or `markdown.inlineCode`. Unprefixed package tokens and `clay.*` package claims are rejected.
- Every package token declaration must provide a semantic description, token type, optional fallback token of the same type, and provenance. Type examples include color role, text role, spacing, radius, border, opacity, font role, and component variant.
- Component style variables must reference known typed tokens or documented enum/size values. Unknown style tokens, type-incompatible fallbacks, duplicate token declarations, raw CSS, native renderer callbacks, style strings, and raw colors without a typed token contract are rejected.
- User token overrides must flow through documented configuration APIs and stay type-compatible with the declared token. Overrides do not grant renderer access, native widget access, filesystem/network/shell/AI/WASM authority, or client-side JavaScript execution.

## Planned Package-Facing Shape

**Runtime-backed Phase 18.3 package-facing contribution API:**

```ts
import {
  serverRegisterPanelContribution,
  serverRegisterTransientOverlayContribution,
  serverRegisterThemeToken,
} from "clay:ui";

serverRegisterThemeToken(manifest, {
  token: "markdown.preview.background",
  type: "color-role",
  fallback: "surface.panel",
  description: "Markdown preview panel background",
});

serverRegisterPanelContribution(manifest, {
  id: "markdown.preview",
  slot: "right",
  kind: "fixed",
  defaultVisibility: "hidden",
  actionTargets: ["markdown.togglePreview"],
  component: {
    kind: "panel",
    id: "markdown.preview.root",
    style: { background: "markdown.preview.background", padding: "spacing.panel" },
    children: [{ kind: "label", id: "markdown.preview.empty", text: "Preview unavailable" }],
  },
});

serverRegisterTransientOverlayContribution(manifest, {
  id: "markdown.preview.quickActions",
  anchor: "main",
  focusPolicy: "restore",
  dismissalPolicy: "escape-or-outside",
  component: { kind: "overlay", id: "markdown.preview.quickActions.root", children: [] },
});
```

**Implemented Phase 18.4 package-facing input/state/configuration APIs:** input contribution registration, UI state-scope schema/lifecycle registration, layout override setting, and package option setting are runtime-backed documented APIs. **Planned package-facing layout/state APIs only:** working-area registration, pane split registration, direct pane slot layout setting, pane selectors, multi-panel ordering, overlay z-order, cross-window layout, package enable/disable, durable workspace/document/user-config persistence, and state-value mutation remain inventory stubs or deferred until later phases add validators, docs, registry entries, and tests.

The implemented package UI surface now includes the documented `clay:sdui` foundation plus the runtime-backed Phase 18.3 `clay:ui` contribution facade. Future `clay:ui` APIs should continue to build on generic primitives such as `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, `PackageUiStateScope`, and package layout override categories rather than creating Markdown-specific Rust branches.

## Performance Contract

Package UI/layout declarations are validated and applied as inert state updates. Package JavaScript runs server-side during package load, configuration change, explicit command handling, or other documented server-side phases; no package logic runs during Masonry paint, layout, pointer, scroll, keypress, or text-event handlers. In short: no package JavaScript runs in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers.

Client hot paths may read already-validated inert state and client-owned transient state. They must not perform package parsing, JavaScript execution, raw IPC waits, full-document serialization, or package-authored native widget mutation. Layout/component payloads should remain bounded and versioned so client updates can reject stale or oversized data before affecting UI state.

## Security and Non-Authorities

The shell/layout boundary forbids packages from obtaining or declaring:

- raw CSS, raw style strings, HTML, style strings, arbitrary draw code, or arbitrary colors outside typed token contracts;
- arbitrary client JavaScript or JavaScript executed in the Rust client;
- raw `Deno.core.ops` or raw Deno op names as package-facing APIs;
- direct Masonry widget handles, Masonry widget constructors, native widget IDs, native widget handles, layout pass callbacks, or native layout mutation;
- Vello callbacks, Parley callbacks, renderer callbacks, or GPU drawing authority;
- filesystem, network, shell, AI mutation, WASM execution, remote listener, package-manager execution, or extension-loading authority unless a future approved decision and documented permissioned Clay JS API grants a narrow capability;
- unregistered action targets, duplicate component IDs, duplicate command/action IDs, duplicate slot claims, unknown style/theme tokens, unsupported state scopes, or oversize component/state payloads.

Validation failures must become deterministic diagnostics at package load, configuration, or UI update time. They must not panic in Masonry handlers and must not silently grant authority.

## Current Implementation Gaps

Phase 18.2 closed the internal Clay shell runtime foundation, and Phase 18.3 adds generic runtime-backed package UI contribution primitives. Current remaining gaps that later tasks/phases must close are:

- Internal `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state exists, but no public/package-facing API for working-area mutation, split-tree mutation, or direct pane-slot layout defaults is callable yet.
- The SDUI sidebar now uses an internal `PaneSlotLayout` bridge for fixed left-slot geometry, but it remains a temporary compatibility bridge while package panels/overlays are bridged through the generic Phase 18.3 runtime state.
- Phase 18.3 validates fixed-vs-transient slot claims, component catalog declarations, package theme token declarations, and package UI metadata, but public generated registry/API pages for the four runtime-backed `clay:ui` contribution functions are still pending the API documentation task.
- Historical Phase 18.3 wording: Durable package UI state values, user/package layout overrides, persisted panel visibility, user theme-token remaps, multi-panel ordering within one slot, overlay z-order policy, pane selectors, and cross-window layout behavior remain Phase 18.4 or later work. Current Phase 18.4 status: layout overrides are runtime-backed, while durable package UI state values, persisted panel visibility beyond the validated session/local contract, durable user theme-token remap storage, multi-panel ordering within one slot, overlay z-order policy, pane selectors, package enable/disable, and cross-window layout behavior remain later planned/deferred work.

## Verification Contract

Documentation and implementation phases that depend on this reference should keep deterministic checks for:

- links from `docs/index.md`, `docs/reference/primitives/index.md`, and package authoring docs;
- vocabulary coverage for working area, pane/split tree, pane/window layout, mandatory `main`, optional `left`/`right`/`top`/`bottom`, fixed panels, transient panels, components/elements, action intents, package state scopes, and style/theme tokens;
- precedence and conflict coverage for Clay shell safety, user configuration, active major mode defaults, compatible minor modes, global packages, package fallback/defaults, duplicate slots/components/actions, unsupported state scopes, and unknown style/theme tokens;
- Masonry boundary wording that treats `RenderRoot`, `Widget`, `Split`, `Flex`, `Grid`, `ZStack`, `Portal`, typed properties, and actions as internal implementation evidence only;
- performance wording that keeps package JavaScript and package parsing out of Masonry paint/layout/pointer/scroll/keypress/text-event handlers;
- security wording that rejects raw CSS, arbitrary client JavaScript, raw `Deno.core.ops`, direct Masonry/native widget access, Vello/Parley callbacks, filesystem/network/shell/AI/WASM authority, and unregistered action targets.
