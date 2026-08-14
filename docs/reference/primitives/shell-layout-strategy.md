# Clay Shell and Package UI/Layout Strategy

Status: Phase 18.1 architecture reference with Phase 18.2 internal shell runtime progress, Phase 18.3 runtime-backed package UI contribution progress, Phase 18.4 runtime-backed package input/state/configuration progress, Phase 18.8 runtime-internal transient menu/command execution progress, Phase 18.12 Clay-owned file-browser composition progress, Phase 20.3 layout primitives (split divider drag, slot resize/collapse, persistence, focus routing, inert layout intents), Phase 22.1 equal-area window splits, Phase 22.2 pane document views (per-pane editor surfaces, document-scoped event routing, duplicate-open focus routing, per-document behavior manifest layers), Phase 22.3 tabs as independent client views (server-authoritative in-memory tab registry, shell-owned tab bar chrome, one connection + one split tree per tab, per-tab lifecycle: open/close/switch/dirty-guard/reconnect), Phase 22.4 keyboard tab management (tab command IDs + default chords, Global/ClientUiCommand routing, explicit numbering/bounds/wraparound policies, rebindability, dirty-close confirm flow), and Phase 22.5 tab × split composition and persistence (composition contract pinned to the active tab, client-owned versioned `layout.json` v2 window-state persistence, sequenced restore gated on registry confirmation). This document is a planning and validation contract plus the primitive reference for the `clay:ui` package-facing shell facade. Examples marked **Planned** must not be treated as callable code until a later implementation phase documents and registers the corresponding Clay JS API. Phase 18.2 implemented internal Rust `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state in `src/shell/layout.rs` plus the native shell root in `src/masonry_shell.rs`; those internals are not package-facing JavaScript APIs. Phase 18.3 implements runtime-backed public APIs for package `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` registration with generated public registry/API pages. Phase 18.4 implements runtime-backed public APIs for `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, and package-owned options. Phase 18.8 implements the generic `TransientMenuSession` state model, the server-owned `CommandExecution` boundary, and the Control Center first consumer as runtime-internal Rust primitives. Phase 18.12 composes a Clay-owned left file browser panel and bottom fuzzy-open session from these generic primitives, while keeping workspace-root discovery, directory listing, and file open/reveal authority server-owned. Phase 24.3 adds the Path Browser: a second built-in consumer of the generic transient menu session that browses arbitrary user-authorized paths (`controlCenter.openPath`), with activation converting ephemeral browse authority into exactly one `SingleFile` or `Directory` grant. Plan 087 adds the Clay-owned welcome entry surface and caret-adjacent completion projection; there is no package-facing component kind, token, style variable, overlay anchor, manifest field, or JS API added, and no package JavaScript runs in their native paint/layout/input paths.

## Sources and Evidence

- Approved decision: `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`.
- Primitive review: `docs/wiki/modules/phase18.1-shell-layout-primitive-review.md`.
- Phase 18.2 runtime baseline: `src/main.rs` starts a `ClayShellWidget` root through `NewWidget`/`NewWindow`; `src/masonry_shell.rs` registers `EditorWidget` as a child component; `src/shell/layout.rs` owns internal `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state; `src/masonry_editor.rs` owns the editor hot-path behavior and status bar and hosts the retained SDUI/panel/overlay widget subtree as Masonry children; `src/masonry_sdui.rs` holds inert SDUI state and `src/masonry_sdui_region.rs` reconciles it into a retained Masonry subtree (`SduiRegionWidget`) placed in the Clay left-slot geometry; `src/protocol/sdui.rs` defines inert SDUI panels, actions, and editor views.
- Phase 18.3 runtime-backed package UI baseline: `runtime/js/ui.js` exposes `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`; `src/server/ops/ui.rs` owns the op wrappers; `src/server/ui.rs` validates package provenance, declarations, registered action targets, typed style variables, and package theme tokens; `src/shell/components.rs` owns the Clay component catalog; `src/shell/theme.rs` owns typed core/package token resolution; `src/shell/package_ui.rs` composes accepted panels/overlays through shell-owned runtime state; `src/masonry_package_region.rs` reconciles package component trees into retained Masonry widgets (`PackageRegionWidget`), hosted by `PackagePanelHost`/`PackageOverlayHost` children of `EditorWidget`.
- Current package authoring guide: `docs/reference/packages/creating-packages.md`.
- Phase 18.12 file-browser composition: `src/shell/file_browser.rs`, `src/server/workspace.rs`, `src/server/command_execution.rs`, and `docs/wiki/modules/phase18.12-workspace-discovery-primitive-review.md`.
- Masonry documentation reviewed through Context7 `/linebender/xilem` on 2026-06-09: `Widget` trait, container widget methods, `masonry_winit` `NewWindow`/root-widget startup, `RenderRoot` widget-tree passes, `Flex`, `Portal`, typed properties, and actions. The docs confirm Masonry is Clay's native widget/layout/rendering substrate for building higher-level GUI libraries, not the package author API.

## Phase 18.2/18.3 Runtime Status

**Implemented/runtime-internal in Phase 18.2:**

- `src/main.rs` starts a Clay-owned `ClayShellWidget` as the native root widget and registers `EditorWidget` as the editor child component instead of treating the editor as the application shell.
- `src/shell/layout.rs` owns internal Rust `WorkingAreaLayout` state for one working area, a layout version, the active/root pane, and the editor component binding.
- `PaneSplitTree` supports the default one-leaf tree plus generic horizontal/vertical split nodes with stable pane IDs, bounded split ratios, duplicate-pane rejection, oversize tree rejection, and deterministic geometry calculation.
- `PaneSlotLayout` keeps exactly one mandatory `main` slot and optional fixed `left`, `right`, `top`, and `bottom` slots with finite size, min/max clamp, visibility, collapse, and user-resize state.
- The SDUI sidebar is a retained reconciled Masonry subtree (`SduiRegionWidget`) placed in the Clay-owned left-slot geometry; it is not a package slot API. Package panel/component contributions compose through the generic Phase 18.3 runtime state and are hosted as separate Masonry children (`PackagePanelHost`/`PackageOverlayHost`).
- Inert local layout updates and structural observability are internal test/agent inspection surfaces. Observations record layout shape, slot geometry, and component binding without exposing document text, native widget handles, Masonry widget IDs, raw action authority, raw CSS, raw ops, renderer callbacks, or executable package code.

**Implemented/runtime-backed public APIs in Phase 18.3:**

- `clay:ui` is a curated server-side facade module, not a raw-op surface.
- `PanelContribution` / `serverRegisterPanelContribution` accepts package-prefixed fixed panels for `left`, `right`, `top`, or `bottom`, validates package provenance, registered action targets, bounded component trees, and payload budgets, and stores accepted declarations for shell runtime composition.
- `ComponentContribution` / `serverRegisterComponentContribution` accepts bounded component trees using the current Clay component catalog: `editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, `statusItem`, `dropdown`, `collapse`, `modal`, and `textInput`; `table` remains reserved.
- `TransientOverlayContribution` / `serverRegisterTransientOverlayContribution` accepts package-prefixed overlays with `working-area`, `active-pane`, `main`, or `pointer` anchors plus `none`, `restore`, or `trap` focus policies and `manual`, `escape`, `outside`, or `escape-or-outside` dismissal policies. Clay-internal `Completion` and `Centered` origins are not accepted package anchors.
- `PackageThemeTokenDeclaration` / `serverRegisterThemeToken` accepts package-prefixed typed token declarations across the ten additive domains: `color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, and `density`, with same-type Clay core fallbacks.
- Package manifest validation accepts Phase 18.3 `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, and `themeTokens` descriptors for load-time diagnostics and conflict checks.
- Accepted fixed panels compose into `PaneSlotLayout` geometry while preserving the mandatory `main` editor slot; transient overlays render separately and do not consume fixed slot geometry.

**Still planned/package-facing after Phase 18.3:** public callable working-area, pane-split, and pane-slot layout mutation/default APIs; Historical Phase 18.3 handoff also kept `PackageLayoutOverride`; user shell-layout/theme-token override configuration APIs; and server-to-client package UI publication beyond the current local validated runtime/shell state were still planned after Phase 18.3. Planned-only `ui.*` inventory entries remain `status = "planned"`, `registry_public = false`, and backed by `op_clay_runtime_unavailable` for working-area, split-tree, and direct pane-slot mutation; the four Phase 18.3 contribution entries are `status = "runtime-backed"`, `registry_public = true`, documented under `docs/reference/clay-js-api/ui/`, and generated into the public registry. Phase 18.4 promotes `PackageInputContribution` through `ui.serverRegisterInputContribution`, `PackageUiStateScope` through `ui.serverRegisterUiStateScope`, `PackageLayoutOverride` through `ui.serverSetLayoutOverride`, and package-owned options through `configuration.setPackageOption`; these Phase 18.4 entries are `status = "runtime-backed"`, `registry_public = true`; both Phase 18.4 entries are `status = "runtime-backed"`, `registry_public = true`, and preserve the same no-hot-path inert package UI/configuration boundary.

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

The working area is the drawable application region managed by Clay inside a native window. It excludes OS chrome and is the root of Clay's editor/package UI composition. A working area owns one pane/split tree. Since Phase 22.3 a window hosts one tab bar plus one working area **per tab**: each tab is an independent client view with its own server connection and its own working area/pane/split tree, so the plural form applies once more than one tab is open (the working area excludes the tab bar row when it is visible).

### Pane/Split Tree

A `PaneSplitTree` is a Clay-owned tree whose leaves are panes and whose internal nodes are horizontal or vertical splits. Phase 18.2 implemented the internal Rust state and geometry helpers for the one-leaf default and generic split topology; Phase 22.1 added the user-facing split lifecycle (`split_pane` capped at 4 panes, `add_equal_pane`, `close_pane`, `move_pane`, `keyboard_resize`) and multi-pane hosting in `ClayShellWidget`; Phase 22.2 added per-pane document views (each pane hosts at most one document of its tab's workspace; the pane↔document mapping is client-local view state); package-facing pane-content contribution and slot-targeted package placement remain planned.

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

A pane is a leaf in the pane/split tree. It owns exactly one mandatory `main` container and may own optional `left`, `right`, `top`, and `bottom` panel slots. Pane state may include active component, focus metadata, panel visibility, split ratios, and transient overlay state when later phases implement those fields. Since Phase 22.2 the `main` content of an editor pane is a document view: one `PaneDocumentView` (independent `EditorSurface`, caret/selection/viewport, undo history, request-id allocators, and status line) per open document.

### Slots

Slots are Clay-defined attachment points in a pane:

- `main` is mandatory. It normally contains the active editor view or another primary Clay component.
- `left` is optional and intended for side panels such as file trees or outlines.
- `right` is optional and intended for side panels such as previews or inspectors.
- `top` is optional and intended for tool/status/find-like regions that belong above the main content.
- `bottom` is optional and intended for diagnostics, output, status, or console-like regions.

Slots are declarations in the Clay layout model, not Masonry containers exposed to packages. Clay validates slot ownership, collision, visibility, and persistence before client state changes are applied.

### Fixed Panels

Fixed panels participate in layout and reduce the size of the `main` slot while visible. Examples include a file tree, outline, preview pane, diagnostics list, or output panel. The runtime-backed `PanelContribution` API may request a fixed default, but Clay and user configuration determine the final composed layout.

### Transient Panels and Overlays

Transient panels overlay the pane or working area and are dismissible or focus-scoped. Examples include package command palettes, dropdowns, hover documentation, modals, temporary find/replace bars, and menus. The runtime-backed `TransientOverlayContribution` describes package UI as inert data; Clay owns focus trapping, dismissal, z-order, accessibility, and native overlay implementation. The built-in completion and centered Command Centre surfaces are separate Clay-owned projections, not package overlay contributions.

### Bottom Transient Menus and Command Execution

Phase 18.8 specializes the transient panel family with a generic, implemented `TransientMenuSession` for Clay-owned command browsing and picker workflows. A session records prompt text, query text, bounded item metadata, selected index, status text, focus policy, accessibility labels, inert actions, and a stable session ID. Server-routed Control Center/Path Browser sessions use the centered host; completion reuses the retained renderer as a client-local `TransientMenuOrigin::Completion` projection with an IME/caret anchor, modeless focus, and no package-facing anchor. It is not a fixed bottom panel and does not consume fixed `PaneSlotLayout` geometry unless a later declaration explicitly installs fixed bottom chrome.

Phase 24.3 adds the Path Browser as a second built-in consumer of the `TransientMenuSession` round trip. `controlCenter.openPath` opens a `PathBrowserSession` (src/shell/path_browser.rs) seeded from the active document's canonical parent, then the bound tab's workspace root, then the server cwd. The session holds a canonical current directory, the editable path input (the server-owned query line — not a package `textInput`), a derived filter fragment, bounded installed entries, persisted selection, and a sticky error status. A listing (`BuiltInUserBrowseListing`, src/server/workspace.rs) runs once per directory-prefix change or empty-filter ascent on Tokio's blocking pool (`spawn_blocking`) with no workspace/tab/menu lock held, capped at `TRANSIENT_MENU_MAX_ITEMS` depth-1 entries sorted deterministically (directories first with an empty filter, fuzzy order otherwise via the shared matcher); filter-only edits perform no filesystem work. Primary activation on a directory descends, on a file opens it (closing the session); Alt+Enter (secondary activation) on a directory opens it as the tab's workspace; Backspace on an empty filter ascends; direct path-bar edits jump to any path (a typed directory needs a trailing separator). Opening a file converts the ephemeral browse authority into exactly one `SingleFile` grant, and a workspace open converts it into exactly one `Directory` root grant for the bound tab only; navigation alone creates no grant, other tabs are untouched, and the native file/folder dialogs remain the fallback capability issuers. The Path Browser session is Clay-owned and server-routed end to end: packages cannot open, populate, intercept, or receive paths from it, and it never runs package JavaScript, blocking IPC, or filesystem work on paint/layout/input paths.

Command listing/filtering and command execution remain separate. Query updates and selection movement score already installed bounded metadata locally through the shared fuzzy subsequence matcher (`src/shell/fuzzy.rs`, bounded input/candidate caps, deterministic tie-breaks), never re-consulting the registry or running package JavaScript. Phase 24.2 activation produces a typed `ServerMenuActivation` instead of executing inline: server/package commands dispatch through the shared `CommandExecution` boundary with the live aggregated registry, while `ClientUiCommand`-routed shell commands ship the narrow server-approved `ShellClientCommandRequest { command_id }` frame and run in the client shell driver (see the pane/tab command bullets below). SDUI actions, package UI action intents, behavior-manifest keybindings, and transient-menu selections all normalize to this same server-owned execution boundary.

Package authors declare commands with `commands.serverRegisterCommand` and expose them through inert action intents. A command intent carries only a registered command ID and bounded primitive arguments; it never carries a JavaScript callback, raw op name, native handle, filesystem path, or executable code. Transient menu items are built from the same registered command metadata and activate through the same `CommandExecution` path. Registration or menu inclusion does not grant execution authority; the server re-checks every activation.

No bottom or caret-adjacent transient menu path may run package JavaScript, command handlers, package validation, configuration evaluation, blocking IPC, filesystem, network, shell, AI, WASM, package-manager, package installation, package enable/disable, full-document serialization, raw op, raw CSS, native-widget, or client-side JavaScript work in Masonry paint/layout/pointer/scroll/key/text-event handlers. Paint and layout read installed inert menu/overlay state only; ordinary editor typing remains client-first. Package-authored menu accessibility labels are normalized once at Clay's host boundary and bounded to the existing 256-character policy.

Phase 24.4 presents built-in command and path sessions as one centered
window-level `PackageOverlayHost` above the shell. `TransientMenuOrigin::Centered`
uses cached `dimension.overlay.centered.width` (640 logical-pixel default) and
one `paint_scrim` fill from `surface.scrim`/`opacity.scrim`; its default result
surface is 220 logical pixels high before available-window clamping and adds no
blur, filter, or offscreen render target. Completion, context-menu, menu-bar,
and package overlays preserve existing local anchors. The centered anchor is
Clay-internal, not a package `OverlayAnchor` value. The centered host reports
modal Dialog/Menu/MenuItem/Status accessibility and keeps server-owned input
containment on the originating pane. Plan 087's live review tracks retained
scroll-child clipping separately as `P1-087-UI-1`; it does not add package
authority.

### Phase 18.12 Clay-Owned File Browser

The Phase 18.12 file browser is a first-party composition of existing shell primitives, not a new package-owned primitive category. It uses the `left` fixed panel shape for a workspace tree/list and the bottom transient menu shape for fuzzy-open. Both surfaces read bounded server-prepared state and emit inert command intents.

Workspace and filesystem authority stay outside the package UI contract:

- Workspace-root discovery is server-owned (`cwd`/startup roots, opened-file ancestry, explicit user grants, closed Clay marker set).
- Directory listing is server-owned and bounded by ignore rules, depth/count limits, cancellation, refresh, and diagnostics.
- File opening and reveal actions route through `CommandExecution` and `WorkspaceState`; selected out-of-root files receive single-file grants only.
- Packages cannot add workspace roots, marker names, ignore rules, listing providers, raw file paths, native file-tree widgets, or file-browser-specific Rust rendering branches.

Packages may learn from this composition when declaring package panels or overlays, but they do not own Clay's file browser slot, root discovery, listing service, fuzzy-open session, or file command authority. If a later package needs project search, symbol search, or diagnostics browsing, it should reuse generic bounded/cancellable query and transient menu primitives rather than performing filesystem, parse, or package-JS work in paint/layout/input paths.

### Phase 20.1 Token-Backed Panel and Density Defaults

Phase 20.1 moved the legacy hardcoded panel/sidebar dimension constants behind typed tokens. `dimension.sidebar.default`, `dimension.panel.side.default/min/max`, and `dimension.panel.vertical.default/min/max` feed `ResolvedUiTheme::panel_defaults()` (`src/shell/theme.rs`), the single shared Clay panel/sidebar geometry source. The SDUI left-slot compatibility bridge and package fixed-panel `Left`/`Right`/`Top`/`Bottom` slot state both read from this one view, so the legacy 240px sidebar and package side-panel default stay in lockstep.

Resolved dimensions are finite, non-negative, and ordered `min <= default <= max`. A missing, non-finite, or misordered override triple falls back to the matching Clay constant tuple per domain before layout — invalid token ordering never constructs a misordered `FixedSlotState`. `FixedSlotState` validation/clamping in `src/shell/layout.rs` runs unchanged on the token-backed values.

`density.default` selects the active `DensityLevel` (`compact`/`default`/`spacious`); `ResolvedUiTheme::spacing_scale()` returns the `0.875`/`1.0`/`1.125` multiplier. Density scales the token-owned UI spacing rhythm only (consumed by Phase 20.4 component uplift); it never scales panel dimensions or document typography, which live on the separate `TypographyRegistry`.

Resolution occurs at theme/configuration install time. The client caches one `ResolvedUiTheme`; `panel_defaults()` is a cached read over installed state. Native paint, layout, pointer, scroll, keypress, and text-event paths perform no package JavaScript, theme parsing, raw IPC, or re-resolution. Phase 20.3 implemented split-ratio drag, slot resize/collapse interaction, layout persistence (`~/.config/clay/layout.json`), focus/input routing across splits, and the inert versioned `LayoutIntent` API (`serverRequestLayoutIntent`); these consume the token-backed defaults without redesigning their ownership.

### Phase 22.1 Equal-Area Window Splits

Phase 22.1 closes the gap between the Phase 20.3 split-primitive runtime and user-facing multi-pane editing. The working area — the drawable region inside a native window excluding fixed slot chrome (left/right/top/bottom panels) — is divided into panes by a Clay-owned `PaneSplitTree`. Phase 22.1 adds the lifecycle operations the Phase 20.3 tree lacked:

- **`split_pane`** (capped at `MAX_PANES_PER_TAB = 4`): splits the focused pane along a chosen orientation. `SplitOrientation::Horizontal` places panes side by side (vertical divider line); `SplitOrientation::Vertical` stacks them (horizontal divider line). The cap is enforced at the operation level, not the constructor.
- **`add_equal_pane`**: redivides the working area into N+1 equal-area leaves along the root split orientation, retaining existing panes in reading order with one new empty pane. A right-leaning comb tree with ratios `1/(N+1)`, `1/N`, …, `1/2` gives each leaf exactly `1/(N+1)` of the parent area.
- **`close_pane`**: removes a leaf and promotes its sibling subtree; the last pane is protected (no-op). The active pane becomes the sibling's first leaf.
- **`move_pane`**: swaps leaf IDs in reading order, preserving tree shape and split ratios. No-op at the ends.
- **`keyboard_resize`**: adjusts the ratio of the deepest ancestor split whose orientation and child-side match the resize direction, clamped to `MIN_SPLIT_RATIO`/`MAX_SPLIT_RATIO` in `KEYBOARD_RESIZE_STEP` increments. No-op if no bordering divider exists.

`ClayShellWidget` hosts one `PaneContentHost` widget per pane leaf. A `PaneContentHost` is a generic content host — it wraps either an `EditorWidget` or a placeholder (surface-panel-filled empty pane). Panes are **not** tied to files or to editor views: the architecture treats a pane as a generic workspace-bound content host so a future terminal emulator or other workspace app can occupy a pane without introducing a new shell primitive. The pane-content contribution path is **not yet public**; packages cannot contribute pane content today.

Pane focus is configurable: **click-to-focus** (default) activates a pane on pointer-down; **focus-follows-cursor** activates the pane under the pointer on hover. Editor panes consume pointer-down internally, so editor focus is derived from Masonry `ChildFocusChanged` updates rather than pointer bubbling. Placeholder panes bubble pointer-down to the shell for direct activation.

Clay owns all topology mutation. The split/close/add-equal/move/resize commands (`shell.clientSplitPaneVertical`, `shell.clientSplitPaneHorizontal`, `shell.clientAddEqualPane`, `shell.clientClosePane`, `shell.clientFocusPaneNext`/`prev`, `shell.clientResizePaneLeft`/`right`/`up`/`down`, `shell.clientMovePaneNext`/`prev`) are `ClientUiCommand`-routed, Clay-owned built-in commands registered in `default_commands()` with `CommandAuthority::ClientUi`. Default keybindings ship in `default_keymaps()` with `KeyBindingContext::Global` (so they fire without editor text focus) and are user-overridable through `keybindings.bindKey` in `~/.config/clay/init.js`. `route_key` checks `EditorTextFocus` rules before `Global` rules so an editor-scoped binding for the same chord wins.

Packages still interact with layout only through the inert, versioned `serverRequestLayoutIntent` API (Phase 20.3). Direct topology mutation (`ui.serverRegisterPaneSplitTree`) remains a planned stub backed by `op_clay_runtime_unavailable`; it is superseded by `serverRequestLayoutIntent` and is not registry-public. Packages cannot own, create, close, move, or directly mutate panes or splits.

### Phase 22.2 Pane Document Views Within One Workspace

Phase 22.2 turns panes into simultaneous document views of the single workspace their tab owns. Each pane hosts at most one open document at a time — a 1:1 pane↔document mapping that is **client-local view state only**. The server still owns documents, leases, access, and per-document major modes; the client never mirrors server path canonicalization or document authority.

- **Per-pane document view hosting.** `EditorWidget` remains the connection owner: it holds all connection-wide chrome — SDUI sidebar region, package panel/overlay hosts, theme/typography/behavior baseline — and hosts the focused pane's document view. Every additional pane mounts its own lightweight `PaneDocumentView` (a `PaneContentHost::Document` content leaf) with an independent `EditorSurface`, caret, selection, viewport, undo history, status line, and request-id allocators. Placeholder panes remain available.
- **Document-scoped event routing.** Connection events carrying a `document_id` route to the pane that owns that document; connection-wide baseline events (theme, typography, behavior manifest, caret style, disconnects, runtime diagnostics) fan out to every mounted pane view. Unmapped `DocumentOpened` events (server-initiated opens, reconnect recovery) fall back to the active pane, which mounts a view if it was a placeholder.
- **Duplicate opens focus the owner.** The server canonicalizes paths and answers an already-open path with the existing lease and a fresh snapshot (`SelectedOpenPrepare::Existing`). The client detects the duplicate from the returned `document_id` — never by client-side path matching — applies the redundant snapshot only to the owning view (a same-document open is a no-op on the live surface: buffer, caret, selection, and pending edits survive), focuses the owner, and creates no second view. Opening the same file twice in one workspace never produces two views.
- **Open flows target the focused pane.** Every open flow records the requesting pane before dispatch — native open-dialog completion, open-selected-file, file-browser/fuzzy `workspace.openFile`/`openFuzzyFile` intents, and definition-navigation intents dispatched from pane menus — and the answering `DocumentOpened` loads into exactly that pane. Attribution is bounded: one entry per pane (replaced on re-request, removed on pane close, consumed on answer) matched by the client-known identity (absolute path for dialog/selected-file flows, `(workspaceRootId, relativePath)` for browser/fuzzy flows). No client-side path canonicalization.
- **Per-pane major modes.** Mode activation is per-document on the server (`active_major_modes` keyed by `document_id`); Phase 22.2 made the behavior manifest per-document as well: the server `ActiveBehaviorManifest` keeps a global manifest plus per-document layers (every publish advances one connection-wide `behaviorVersion`), the client tracks a document behavior registry, and each pane view installs only the manifest layer scoped to its document — other layers bump the version without changing keymaps, completion triggers, or editor rules. A `.rs` pane and a `.md` pane in the same tab run different major modes concurrently without cross-pane bleed; runtime snapshot recovery restores each document's manifest layer.
- **Pane close safety.** Closing a pane whose document is dirty is blocked; the pane's save-conflict menu (existing server-owned save/reload path) must resolve before the pane closes. Clean panes release their document lease and close retained sessions on close.
- **Open-documents menu.** `shell.clientShowOpenDocuments` opens on the focused pane and lists every pane's active document plus retained sessions with active/dirty markers; cross-pane entries carry the owning pane ID, and activating one focuses the owner and switches its document (stashing its prior session as today) — consistent with the one-view-per-document rule. `shell.clientActivateDocument` semantics remain focused-pane-scoped.
- **Chrome scope in 22.2.** The SDUI sidebar, package panels, and package overlays remain connection/window-scoped chrome hosted by the connection owner, not per-pane surfaces. Per-pane package chrome — package UI rendered inside each pane — is **not** a public contribution path in Phase 22.2 and remains post-22.2 roadmap work (it needs the 22.3 tab/client model first).

Packages still interact with layout only through the inert `serverRequestLayoutIntent` API. No package-facing API opens documents into panes or contributes pane content in Phase 22.2; pane content hosting (`PaneContentHost` / `PaneDocumentView`) is internal Clay machinery.

### Phase 22.3 Tabs as Independent Client Views

Phase 22.3 makes each tab a fully independent client view of its own workspace. Tabs are **not** pane-local UI state and **not** a package surface: they are the top-level window organization, above the working area. The tab bar is a shell-owned chrome row below the top fixed panel slot and above the working area, painted only when more than one tab is open; cards show the workspace name (the validated root's display path final segment), and the trailing `+` affordance opens a new tab from the native folder picker. Packages cannot own, contribute to, open, close, move, or reorder tabs or the tab bar; the inert `serverRequestLayoutIntent` API remains the only package layout surface.

- **Server-authoritative registry.** The server owns an in-memory `TabRegistry` (`src/server/tab_registry.rs`): tab order, the active tab, and each tab's workspace root + client binding. `TabCommand` messages (`New`, `OpenWorkspace`, `Close`, `Activate`, `Reclaim`) mutate it; every mutation pushes a `TabRegistrySnapshot` broadcast (including rejections, so an optimistic client reverts), and the handshake replays the current snapshot. The registry survives client reconnects and single-client process restarts (`Reclaim` rebinds a `TabId` to a new `ClientId`); disk persistence of the registry and per-tab split trees is Phase 22.5, which persists them **client-owned** — the server registry stays in-memory and is rebuilt at startup through the existing `TabCommand::New`/`Activate` paths.
- **One connection per tab.** Each tab holds its own `ClientSession` (edit queue, sync state) and its own chrome (SDUI region, package panels/overlays, runtime generation), split tree, pane targets, focus policy, and pending-open attribution — full isolation between tabs. The shell keeps inactive tabs' chrome registered in the widget tree at zero size so connection events keep applying; switching swaps which chrome is laid out at the active working-area rect.
- **Tab lifecycle.** Open: `+` → folder picker → connect → `New` (server-validated via `add_root`); a refused connection (e.g. the `MAX_ACTIVE_CONNECTIONS` cap) surfaces a diagnostic and mounts no tab. Switch: card click activates optimistically, the registry reconciles. Close: the last tab cannot close; a dirty-guard generalizes the Phase 22.2 pane-close gate across the tab's panes (first dirty view surfaces its save-conflict menu and blocks the close), and a clean close removes the entry and ends the connection (permit + leases release through the existing disconnect cleanup). Reconnect: a dropped connection auto-reconnects with backoff, `Reclaim`s its tab, and re-opens its retained documents through the plain `OpenDocument` path; split trees and per-pane document state restore from retained in-memory chrome/sessions.
- **Package surface unchanged.** No package-facing tab/tab-bar/pane-content surface is introduced in Phase 22.3. Per-tab package chrome — package UI rendered inside each tab's panes — remains still-planned (needs a later phase; Phase 22.2's connection/window-scoped SDUI sidebar, package panels, and overlays are hosted per tab, exactly as they were per connection).

### Phase 22.4 Keyboard Tab Management

Phase 22.4 adds keyboard and command control for the Phase 22.3 tab model: 24 Clay-owned `client_ui` command IDs (6 flat + two numbered families of 9), Global-scope default chords, server-registry reorder operations, and a driver-owned dirty-close confirm/save flow. Tab operations remain **Clay-owned client behavior** — the same command routing model as the Phase 22.1 pane commands; packages gain no surface.

- **Command IDs and defaults.** The flat commands `shell.clientTabNext` (`Ctrl+Tab`), `clientTabPrev` (`Ctrl+Shift+Tab`), `clientTabNew` (`Ctrl+T`), `clientTabClose` (`Ctrl+Shift+W`), `clientTabMoveLeft` (`Ctrl+Shift+[`), and `clientTabMoveRight` (`Ctrl+Shift+]`), plus the dotted-variant numbered families `clientTabActivate.1..9` (`Ctrl+1..9`) and `clientTabMoveTo.1..9` (`Ctrl+Shift+1..9`). All are declared in `default_commands()` with `CommandAuthority::ClientUi`, ship in `default_keymaps()` with `KeyBindingContext::Global` (so they fire without editor text focus, like the pane chords), and are user-overridable through `keybindings.bindKey`/`unbindKey` with `{ scope: "global" }` in `~/.config/clay/init.js` — the existing deny-by-default validation accepts exactly these IDs (the `tab_family_variant` parse accepts variants `1..=9` only; `.0`/`.10` are rejected at bind time). `route_key` checks `EditorTextFocus` rules before `Global` rules, so an editor-scoped binding for the same chord wins.
- **Explicit policies.** (1) Card numbering is 1-based in the user-visible card order (registry order, entry-less mounted tabs appended); `clientTabActivate.N` is a silent no-op when `N` exceeds the tab count. (2) Variants beyond 9 do not exist — `Ctrl+0` and `Ctrl+Shift+0` are unbound, and 10+ tabs are reachable only by next/prev or card click (22.6 may extend this). (3) `clientTabNext`/`Prev` wrap around the card order; with fewer than two tabs they are silent no-ops. (4) `clientTabMoveLeft`/`MoveRight` move the active tab one card position and are no-ops at the ends (no wraparound); `clientTabMoveTo.N` moves to the 1-based position and is a no-op beyond the tab count. (5) The last tab cannot close. (6) The `+` affordance and `clientTabNew` share one new-tab flow (folder picker → connect → server-validated `New`); a second new-tab request while one is in flight is ignored.
- **Server authority and reordering.** The `TabRegistry` gains `move_left`/`move_right`/`move_to` (1-based, boundary no-ops, position-bounds rejection), dispatched from new `TabCommand::MoveLeft`/`MoveRight`/`MoveTo` protocol variants (protocol v13). Every mutation — including rejections — broadcasts a fresh `TabRegistrySnapshot` to all connections so optimistic clients reconcile against one server-authoritative order; `move_to` accepts the same bound-client/tab-exists validation as the 22.3 commands. Active-tab status is preserved by `TabId`, not by position.
- **Dirty-close confirm flow.** Closing a tab with unsaved documents no longer blocks on the per-pane save-conflict menu (the 22.3 `guard_tab_close` walk is replaced): the driver inventories the tab's dirty panes and pushes a driver-owned `TransientMenuSession` with **Save all and close** / **Discard and close** / **Cancel**, naming the tab and every dirty document. Save all enqueues each pane's own existing save path and tracks the awaited `DocumentId`s; the close (`TabCommand::Close`) fires only after every save acked — a failed save or a disconnect cancels the close and surfaces the pane's existing save diagnostic. Discard closes immediately (the server's disconnect teardown force-releases the tab's documents); Cancel keeps the tab. The menu's action IDs (`shell.clientTabCloseSaveAll`/`Discard`/`Cancel`) are driver-local orchestration — never declared commands, never server-routed — so tab-confirm and per-view save-conflict sessions cannot cross-route.
- **Control Center listing.** Since Phase 24.2, the pane and tab commands are `ClientUiCommand`-routed and **are** listed in the Control Center catalogue: activation closes the menu and ships the server-approved `ShellClientCommandRequest { command_id }` frame, which the client re-parses through `ShellClientCommand::from_command_id` deny-by-default into the same driver path local keybindings use (tab commands and the dirty-close gate included). The menu never executes client commands server-side; the server only approves the narrow ID, and unknown/forged IDs are dropped client-side with no state mutation.
- **Package surface unchanged.** Packages cannot bind keys to or issue tab commands (`bindKey` is a configuration-time **user** API, not a package API), cannot open/close/move/reorder tabs, and gain no new surface from the tab command IDs or reorder operations. No package-facing API, `ComponentKind`, or token changes in Phase 22.4.

### Phase 22.5 Tab × Split Composition and Persistence

Phase 22.5 pins the composition contract (every pane/split operation is scoped to the **active tab's** working area) and adds client-owned window-state persistence: the whole multi-tab window — tab order, active tab, each tab's workspace root, split tree, and per-pane open documents — survives restarts and reconnects via a versioned `layout.json`.

- **Composition contract (verified, not changed).** Pane/split commands (`split_pane`/`add_equal_pane`/`close_pane`/`move_pane`/`keyboard_resize`), divider drags, slot drags, and pane-focus routing mutate only the active tab's `TabChrome`; tab reorder/switch leave every tab's internal state byte-identical. Guard tests: `per_tab_routing_targets_are_isolated` (main.rs), `install_tab_switches_to_new_tab_and_retains_previous`/`set_active_tab_keeps_widget_ids_stable`/`rekey_tab_moves_chrome_and_keeps_widget_ids_stable` (masonry_shell), `apply_tab_registry_reorder_preserves_per_tab_state` (main), `move_ops_change_order_only_and_preserve_entry_contents` (tab_registry).
- **Persistence is client-owned; the server stays stateless on disk.** The server `TabRegistry` remains in-memory (it survives reconnects via `Reclaim`, but not server restarts). The client owns `$XDG_CONFIG_HOME/clay/layout.json` (v2): `{ "version": 2, "activeTab": <0-based index>, "tabs": [ { "workspaceRoot", "activePane", "splitTree" (nested `leaf`/`split` with orientation/ratio/first/second), "slots" (user-modified fixed slots only), "panes": { <paneId>: <workspace-relative document path> } } ] }`. Legacy v1 files (no `version`, `splits`/`slots` keys) still load and apply to the single bootstrap tab exactly as Phase 20.3; v2 files silent-skip the v1 apply. Parse policy: corrupt/missing/partial → defaults, never a panic; tabs truncated to `MAX_ACTIVE_CONNECTIONS` (64); out-of-range `activeTab` → default first tab; non-zero unique pane ids; pane count ≤ 4; node count ≤ 64; ratio finite within 0.05..=0.95; unknown keys ignored; a structurally invalid `splitTree` degrades that tab to the default single pane.
- **Persistence triggers.** The shell's 500ms-debounced mutation hook (pointer drags, keyboard topology/resize) emits a `PersistenceDue` signal instead of writing; the driver also persists on registry snapshots (mount/close/reorder/switch/workspace change) and on document open, and flushes synchronously on quit. The shell never touches disk; the driver is the single writer. Per-pane document identity is the pane's active document only (retained-but-inactive sessions are not persisted).
- **Restore flow.** `run_client` loads the v2 state (server-connected sessions only; local fallback unchanged). Tab 0 rides the bootstrap connection (persisted root drives the initial `TabCommand::New`; persisted split tree installed pre-event-loop); tabs 1..N mount sequentially inside the event loop, each gated on the registry snapshot confirming the previous mount's server `TabId` (server append order = persisted order — no reorder messages). Per-pane documents reopen through the plain `OpenDocument` path with the 22.2 pending-open attribution (`workspace_root_id` + workspace-relative path); the persisted active tab activates last via the shared `Activate` path. Failure policy: a missing workspace root skips that tab with a diagnostic and the restore continues; a refused connection abandons the remaining queue; a server-rejected mount (root vanished — the server answers `FileOperationFailed`, never a snapshot) is bounded by a 15s confirmation deadline that abandons the remaining restore; mounted tabs always stay — restore can degrade to fewer tabs, never stall, never fail to launch. **Not restored:** unsaved edits (a restart drops them, like Phase 20.3), caret/viewport/scroll positions, and per-tab pane-focus-policy runtime changes (the `setPaneFocusPolicy` config API stays the policy source).
- **No new authority.** Persistence adds no protocol messages (protocol stays v13), no server ops, no config keys, no key bindings, and no Clay JS API entries. `layout.json` is user-owned internal state: packages cannot read or write it, cannot observe or contribute to tab/split persistence, and it grants no capability (restored documents ride the same per-connection validation, `add_root` canonicalization, and `OpenDocument` path checks as any open; a tampered file with out-of-root document paths is rejected server-side). All new Rust items are `pub(crate)`/`#[doc(hidden)]` lib↔binary plumbing.

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
| `transient-overlay` | Dismissible overlay/menu/modal state, including Phase 18.8 `TransientMenuSession` query/selection/status state when active | Planned/Phase 18.8 |

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

Status: Phase 18.4 implements the first public configuration contract for package UI/input/action/state defaults. Phase 18.1 did not introduce a callable shell/layout configuration API; Phase 18.2 still does not introduce a callable shell/layout configuration API. Phase 18.3 introduces package declarations for panel defaults and theme tokens but still does not introduce a user-visible panel visibility/default-slot/theme-token override API. Historical Phase 18.3 status: `ui.serverSetLayoutOverride` and `configuration.setPackageOption` were planned stub only, `op_clay_runtime_unavailable`, non-registry-public, and had no `docs/reference/clay-js-api/ui/` page for override behavior. Phase 18.4 promotes the supported override/package-option subset through documented runtime-backed APIs from `~/.config/clay/init.js`; shell working-area mutation, pane selectors, multi-panel ordering, overlay z-order, cross-window layout, package enable/disable, and state-value mutation remain planned/deferred.

Implemented and planned shell/layout configuration surfaces:

| Surface | Clay JS API trace | Configurable behavior | Phase 18.4 public status |
| --- | --- | --- | --- |
| Package-owned option | `configuration.setPackageOption` | Package-prefixed typed options for `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`. | Runtime-backed public API; documented under `docs/reference/clay-js-api/configuration/set-package-option.md` with generated registry coverage. |
| Layout override | `ui.serverSetLayoutOverride` / `PackageLayoutOverride` | User, mode, global package, or package-default override for `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, or `fallback` through `~/.config/clay/init.js`. | Runtime-backed public API; documented under `docs/reference/clay-js-api/ui/server-set-layout-override.md` with generated registry coverage. |
| Theme token declaration | `ui.serverRegisterThemeToken` / `PackageThemeTokenDeclaration` | Package declares typed tokens that `serverSetLayoutOverride` may remap only when the registered fallback type is compatible. | Implemented/runtime-backed public API for declarations; remaps are validated configuration/update work. |
| UI state scope | `ui.serverRegisterUiStateScope` / `PackageUiStateScope` | Package declares allowed state scopes such as `user-config`, `pane`, `component`, or `transient-overlay`. | Runtime-backed inert schema/lifecycle declaration; durable workspace/document/user-config mutation and persisted values remain deferred unless separately documented. |
| Working area, split tree, and direct pane slot mutation | `ui.serverRegisterWorkingAreaLayout`, `ui.serverRegisterPaneSplitTree`, `ui.serverSetPaneSlotLayout` | Direct shell topology mutation/default registration. | Planned stubs only; not registry-public. `serverRegisterPaneSplitTree` superseded by `serverRequestLayoutIntent` (Phase 20.3). |

Every concrete shell/layout setting must remain a Clay JS API with `custom_properties`, types, defaults, allowed values, examples, errors, key binding metadata, permissions/security notes, backing Rust/op/facade metadata, `docs/index.md` links, generated registry coverage, and lookup metadata before users or agents can depend on it.

All hidden JSON/TOML/ad hoc layout, style, input, or theme keys are rejected. This includes keys named like `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `preview.position`, `preview.defaultVisibility`, or `theme.markdown.heading.1` when they bypass documented Clay JS APIs. User overrides cannot grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw Deno ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, client-side JavaScript, unsupported slots, unknown style tokens, unregistered actions, or hidden package state authority.

Configuration evaluation happens at startup, package load, configuration reload, or explicit setting-change time. Masonry paint/layout, pointer, scroll, keypress, text-event handling, and ordinary editor hot paths read already-validated inert state only; they must not run package JavaScript, wait on configuration IPC, parse package metadata, mutate native layout from package code, or recompute layout from user JavaScript.

## Clay JS API Inventory Status

Status: mixed Phase 18.4 implementation. Phase 18.2 implemented internal Rust runtime primitives, Phase 18.3 adds a runtime-backed `clay:ui` contribution facade for `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` registration, and Phase 18.4 adds runtime-backed `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, and `PackageOwnedConfiguration` APIs. Working-area, split-tree, direct pane-slot layout defaults, pane selectors, multi-panel ordering, overlay z-order, cross-window layout, package enable/disable, and durable state-value mutation remain planned inventory surfaces. The inventory verifies names and metadata so later shell/layout implementation phases can promote the remaining APIs without exposing raw Rust functions or raw `Deno.core.ops`. The implemented `clay:ui` and `clay:configuration` APIs now create `docs/reference/clay-js-api/` Markdown pages, generated public registry entries, and lookup-visible registry-public rows while keeping direct shell mutation/state-value APIs planned.

Rust visibility audit: Phase 18.2 introduces no new public server-side Rust shell/layout functions. `src/shell/mod.rs` and `src/shell/layout.rs` remain `pub(crate)` internal runtime state. `src/masonry_shell.rs::ClayShellWidget`, `ClayShellWidget::single_editor`, `ClayShellWidget::editor_widget_id`, and `ClayShellWidget::focus_fallback_widget_id` are Rust-public only for the Cargo package's binary/library boundary so `src/main.rs` can construct the native shell and route focus/actions; the module/type are `#[doc(hidden)]`, native-only, not server-side APIs, not package-extensibility APIs, and not backed by a `deno_core` op, JS facade, Markdown API page, docs-index registry link, generated registry entry, or lookup metadata. The behavior-changing shell update and observation helpers (`apply_layout_update`, `observable_snapshot`, `WorkingAreaLayoutUpdate`, `WorkingAreaLayoutObservation`) remain `pub(crate)` and are explicitly not callable from JavaScript.

| Primitive category | JS module specifier | Planned JS export/callable | Stable registry ID | User-facing name | Phase 18.3 public status |
| --- | --- | --- | --- | --- | --- |
| `WorkingAreaLayout` | `clay:ui` | `serverRegisterWorkingAreaLayout` | `ui.serverRegisterWorkingAreaLayout` | Register Working Area Layout | Planned stub only; `op_clay_runtime_unavailable`; not registry-public. |
| `PaneSplitTree` | `clay:ui` | `serverRegisterPaneSplitTree` | `ui.serverRegisterPaneSplitTree` | Register Pane Split Tree | Superseded by `serverRequestLayoutIntent` (Phase 20.3); planned stub only; `op_clay_runtime_unavailable`; not registry-public. |
| `PaneSlotLayout` | `clay:ui` | `serverSetPaneSlotLayout` | `ui.serverSetPaneSlotLayout` | Set Pane Slot Layout | Planned stub only; `op_clay_runtime_unavailable`; not registry-public. |
| `PanelContribution` | `clay:ui` | `serverRegisterPanelContribution` | `ui.serverRegisterPanelContribution` | Register Panel Contribution | Runtime-backed public API; `op_clay_ui_register_panel_contribution`; registry-public with per-API docs. |
| `ComponentContribution` | `clay:ui` | `serverRegisterComponentContribution` | `ui.serverRegisterComponentContribution` | Register Component Contribution | Runtime-backed public API; `op_clay_ui_register_component_contribution`; registry-public with per-API docs. |
| `TransientOverlayContribution` | `clay:ui` | `serverRegisterTransientOverlayContribution` | `ui.serverRegisterTransientOverlayContribution` | Register Transient Overlay Contribution | Runtime-backed public API; `op_clay_ui_register_transient_overlay_contribution`; registry-public with per-API docs. |
| `TransientMenuSession` | `clay:ui` or internal shell module | `serverOpenTransientMenu` (only if public) | `ui.serverOpenTransientMenu` (only if public) | Open Transient Menu | Phase 18.8 planned generic active-session primitive; public API only if facade/op/docs/registry/tests are added. |
| `PackageThemeTokenDeclaration` | `clay:ui` | `serverRegisterThemeToken` | `ui.serverRegisterThemeToken` | Register Theme Token | Runtime-backed public API; `op_clay_ui_register_theme_token`; registry-public with per-API docs. |
| `PackageUiStateScope` | `clay:ui` | `serverRegisterUiStateScope` | `ui.serverRegisterUiStateScope` | Register UI State Scope | Runtime-backed inert schema/lifecycle declaration; registry-public with facade/op/docs/tests. |
| `PackageLayoutOverride` | `clay:ui` | `serverSetLayoutOverride` | `ui.serverSetLayoutOverride` | Set Layout Override | Runtime-backed public API; `op_clay_ui_set_layout_override`; registry-public with per-API docs. |

The naming layers are deliberately distinct: the module specifier groups imports, the lower-camel-case export is what JavaScript would call, the stable registry ID is the globally searchable `ui.*` identifier, and the user-facing name is the English help/search label. Raw Rust paths, raw op names, Masonry type names, protocol DTO names, and generated registry IDs must not become package-facing callable names.

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

- Internal `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state exists. Phase 22.1 added user-facing Clay-owned split lifecycle operations (split/close/add-equal/move/resize) and multi-pane hosting; Phase 22.2 added per-pane document views with document-scoped event routing; Phase 22.3 added the server-authoritative in-memory tab registry, the shell-owned tab bar (open/close/switch, dirty-guarded close, reconnect), and per-tab split trees; Phase 22.4 added keyboard tab management (24 tab command IDs + Global default chords, registry reorder ops, bounds/wraparound policies, the driver-owned dirty-close confirm/save flow); Phase 22.5 closed the persistence gap (client-owned versioned `layout.json` v2: tab order, active tab, per-tab workspace + split tree + per-pane documents survive restarts; unsaved edits do not). No public/package-facing API for working-area mutation, split-tree mutation, direct pane-slot layout defaults, pane-content contribution, tab/tab-bar contribution, or per-tab package chrome is callable yet; packages interact only through inert `serverRequestLayoutIntent`. Phase 22.6 completed the window-model accessibility contract (roles/names/announcements — see [Accessibility (Phase 22.6)](../development/accessibility.md)); still-planned tab work: tab-bar keyboard focus traversal (per-card widget focus for AT focus handling), plus multi-client/multi-window persistence (Phase 21).
- The SDUI sidebar uses an internal `PaneSlotLayout` bridge for fixed left-slot geometry and renders through a retained reconciled Masonry subtree (`SduiRegionWidget`); package panels/overlays are hosted through the generic Phase 18.3 runtime state as separate Masonry children (`PackagePanelHost`/`PackageOverlayHost`).
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
