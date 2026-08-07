# Creating Clay Packages

This guide explains how to design a Clay package and how packages are expected to work with Clay's editor, shell, UI, input, actions, logic, data, configuration, and theme systems.

Clay package APIs are evolving. This document intentionally distinguishes **current implemented public behavior**, **Phase 18.2 internal shell runtime behavior**, **Phase 18.3 runtime-backed slot UI contribution behavior**, **Phase 18.12 Clay-owned file browser behavior**, and **planned package-facing shell/layout/configuration behavior** so package authors and phase plans can update it iteratively as Clay's package architecture lands.

The canonical inventory of reusable UI components, primitives, style variables, and theme tokens lives in the clay-ui skill catalog (`.agents/skills/clay-ui/references/components.md` and `.agents/skills/clay-ui/references/tokens.md`). Package UI must be composed from that catalog; update the catalog in the same change as any component or token addition.

## Goals

A Clay package should be easy for users to load and safe for Clay to run:

**Implemented end-user default** (runtime-backed since Phase 18.6):

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
```

The one-line load path is the preferred default when Clay has the necessary generic primitives. Packages may expose optional customization APIs, but ordinary users should not have to copy package manifests, manually register every primitive, or paste smoke-fixture scripts into `~/.config/clay/init.js`.

Current implemented package API status: `clay.packages.loadPackage` / `loadPackage("@clay/markdown")` is the one-line end-user default. It resolves the specifier, validates the package metadata, enables the package, and imports and executes its `loadEntry`. `clay.packages.serverLoadPackage` / `serverLoadPackage(packageJson)` remains a lower-level validation helper for fixtures and internal use; it is not an end-user install, enable/disable, package-manager, or package-code execution wrapper.

Clay packages can contribute editor modes, commands, behavior manifests, parsers, decorations, UI, layout, actions, configuration, theme tokens, and documentation. They do so through Clay JS APIs and inert validated declarations, not through direct native widget access.

## Core Architecture

Clay's package model has three hard boundaries:

1. **Packages run server-side JavaScript.** Package logic runs in Clay's constrained server-side JavaScript runtime.
2. **The Rust client renders native UI.** The client owns Masonry/Vello/Parley rendering, input handling, focus, caret, selection, viewport, and transient native state.
3. **Packages send declarations and handlers, not client code.** The client receives validated behavior manifests, UI/component snapshots, protocol updates, decoration spans, and command/action intents. It does not execute package JavaScript.

Package authors should think in Clay concepts, not Masonry concepts. Masonry is Clay's internal native widget substrate.

```text
Package JS declarations/handlers
  -> Clay server validation/composition
  -> inert protocol/UI/behavior/render data
  -> Clay client native Masonry widgets/rendering
```

Performance authoring rule: package UI/layout declaration work happens at package load, package validation, configuration evaluation, explicit command handling, or explicit UI update time. The validation/publication timing for package UI declarations, component trees, overlays, actions, and theme token resolution is load/config/update time before client installation; no package JavaScript runs in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers. Typing, Masonry paint, Masonry layout, scroll, pointer, keypress, and text-event paths read already-validated inert state and do not run package JavaScript, package parsing, raw IPC waits, or package-authored native widget mutation.

## Package Surfaces

A complete package may declare or implement these surfaces:

| Surface | Purpose | Examples |
| --- | --- | --- |
| Manifest | Identity, entry points, permissions, docs, primitive contributions | `package.json` `clay` block |
| Loading | Default setup and optional customization | Implemented default `loadPackage("@clay/markdown")`; validation helper `serverLoadPackage(packageJson)` |
| UI/layout | Panels, components, editor views, overlays | main editor, preview panel, file tree |
| Input | Key/mouse/focus interests | Enter transform, click action, panel focus |
| Actions | Commands users/components can invoke | `markdown.togglePreview` |
| Logic | Server-side package code | parser, command handler, mode loader |
| Data/state | Package, document, pane, component state | parse status, preview visible |
| Configuration | User options through `init.js` Clay APIs | preview position, style density |
| Theme/styling | Semantic tokens and typed component styles | heading token, button variant |
| Documentation | User/agent/package API docs | package docs and Clay docs index links |
| Tests | Contract, validation, runtime, docs coverage | package load, commands, UI, parse |

## Current vs Planned Status

### Status markers used in this guide

- **Implemented/public-registry-backed** means a Clay JS API has public Markdown documentation, appears in `docs/index.md`, has generated registry coverage, and is backed by current Rust/op/facade code or current package validation behavior.
- **Implemented/runtime-backed public API** means a Clay JS facade and Rust op/validator exist, the API is recorded in `api-inventory.toml`, documented under `docs/reference/clay-js-api/`, linked from `docs/index.md`, and generated into the lookup registry.
- **Implemented/internal runtime** means Clay's Rust client uses the behavior internally, but it is not a public Clay JS API and package authors must not call or depend on the Rust shape.
- **Planned/target** means the concept is part of the Phase 18 architecture contract or `api-inventory.toml` with `status = "planned"`, but package authors must not treat the example as callable runtime code until a later phase ships and documents the API.
- **Fixture-only/current limitation** means a smoke fixture or temporary package scaffold exists for validation, but it is not the preferred authoring pattern or end-user setup.

### Implemented today

Clay currently has foundations for:

- `~/.config/clay/init.js` configuration loading.
- Server-side JavaScript runtime with curated `clay:*` facade imports.
- Package manifest validation and package record assembly through `clay.packages.serverLoadPackage` / `serverLoadPackage(packageJson)`; this validates metadata but does not install, enable/disable, execute package JavaScript, or run a package manager.
- Package identity/prefix/permission validation.
- Command registration and command metadata.
- Keybinding registration through `clay:keybindings`.
- Mode classification and major-mode activation primitives.
- Behavior manifest routing for key/input behavior.
- Server-driven UI helpers for inert UI trees (`clay:sdui`), including documented runtime-backed APIs such as `clay.sdui.definePanel`, `clay.sdui.defineButton`, `clay.sdui.defineLabel`, `clay.sdui.defineEditorView`, and `clay.sdui.publishTree`.
- Runtime-backed public APIs in `clay:ui` for Phase 18.3 inert package UI contributions: `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`.
- Package manifest UI metadata validation for `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, and `themeTokens`.
- Package manifest Phase 18.4 input/state/configuration metadata validation for `clay.contributions.input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions`, including deterministic conflict diagnostics and provenance.
- Phase 18.15 text styling: two-axis `TokenType` + `Modifiers` decoration vocabulary, inert `clay.contributions.textStyles` theme data, first-party Gruvbox Material Dark/Light theme packages, and `clay.theme.setTheme()` for one active startup theme.
- Phase 18.17 range diagnostics: `clay.diagnostics.serverPublishDiagnostics` for bounded explicit-analyzer `DiagnosticSet` publication and theme-owned `diagnosticError`/`diagnosticWarning`/`diagnosticInfo` squiggles; Tree-sitter highlighting does not claim diagnostic authority.
- Decoration publication and parse handler foundations.
- First-party `@clay/markdown` package scaffold and smoke fixtures.
- First-party `@clay/git` read-only status package consuming the server-owned `clay:git` discovery facade (Phase 18.13).

### Phase 18.2 shell/layout runtime and Phase 18.3 slot-aware package UI

The canonical Phase 18.1/18.2 shell/layout architecture reference is [Clay Shell and Package UI/Layout Strategy](../primitives/shell-layout-strategy.md). This guide summarizes the author-facing contract; the primitive reference owns the detailed vocabulary, Masonry boundary, validation expectations, internal runtime status, and planned primitive names.

Phase 18.2 has implemented internally:

- Clay-owned `ClayShellWidget` root above `EditorWidget`.
- Internal `WorkingAreaLayout` state for one working area, layout version, active/root pane, and editor component binding.
- Internal `PaneSplitTree` state for the one-leaf default plus generic horizontal/vertical split topology with bounded split ratios and deterministic validation.
- Internal `PaneSlotLayout` state with mandatory `main` plus optional fixed `left`, `right`, `top`, and `bottom` slots, including finite sizing, min/max clamp, visibility, collapse, and user-resize fields.
- A retained reconciled SDUI left-slot subtree (`SduiRegionWidget` in `src/masonry_sdui_region.rs`) placed in Clay-owned left-slot geometry, plus structural shell observability that omits document text, native handles, raw action authority, raw CSS, raw ops, renderer callbacks, and executable package code. SDUI kinds render as real Masonry widgets (label/button/list/editor-view under a Clay-owned scroll viewport); the earlier immediate-mode compatibility paint path is retired.

Phase 18.3 now adds runtime-backed public APIs for package-owned slot UI contributions:

- `clay:ui` facade imports are available in the server-side package runtime through `runtime/js/ui.js` and `src/server/js_runtime.rs`.
- `serverRegisterPanelContribution(manifest, declaration)` validates a fixed `PanelContribution` targeting `left`, `right`, `top`, or `bottom` slots and stores package provenance.
- `serverRegisterComponentContribution(manifest, declaration)` validates a bounded Clay component tree/catalog contribution.
- `serverRegisterTransientOverlayContribution(manifest, declaration)` validates an overlay/menu/dialog-like transient contribution with anchor, focus, and dismissal policies.
- `serverRegisterThemeToken(manifest, declaration)` validates package-prefixed typed theme tokens with same-type Clay core fallbacks.
- Package metadata validation accepts `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, and `themeTokens` descriptors for load-time diagnostics/conflicts.
- Runtime composition maps accepted fixed panels to Clay-owned `PaneSlotLayout` state and transient overlays to a separate overlay layer; the editor remains in the mandatory `main` slot.

Still planned for package authors:

- Public callable working-area, pane-split, and pane-slot layout mutation/default APIs. Packages cannot own, create, close, move, or directly mutate panes/splits (Phase 22.1); they interact only through the inert `serverRequestLayoutIntent` API.
- A future pane-content contribution path so workspace apps (such as a terminal emulator, preview renderer, or diff view) can occupy a pane `main` slot as a content host. This path is **not yet public**; panes are generic content hosts internally (`PaneContentHost`), and since Phase 22.2 panes host Clay-owned document views (`PaneDocumentView`), but no package-facing API for contributing pane content exists yet.
- Per-pane package chrome. Phase 22.2 keeps the SDUI sidebar, package panels, and package overlays connection-scoped (hosted per tab since Phase 22.3); packages cannot open documents into panes, name panes, target a pane, or contribute package UI inside a pane. Per-pane package chrome remains planned post-Phase 22.3. Pane↔document mapping, duplicate-open focus routing, and focused-pane open targeting are Clay-owned client behavior, not package APIs.
- Tabs and the tab bar. Phase 22.3 implemented tabs as independent client views (one server connection and one split tree per tab, server-authoritative in-memory registry, shell-owned tab bar row below the top fixed panel slot). Packages cannot own tabs or the tab bar, cannot open/close/move/reorder tabs, cannot contribute tab bar chrome or per-tab package chrome, and gain no new surface from the tab model; `serverRequestLayoutIntent` remains their only layout surface. Per-tab package chrome stays still-planned.
- Package state/data scopes.
- User layout/style/input/theme overrides through documented configuration APIs.
- Updated Markdown package defaults after these package-facing foundations are consumed by first-party packages.
- The one-line end-user package load wrapper `loadPackage("@clay/markdown")`, backed by first-party spec resolution, enable validation, load-entry execution, and persistent runtime state.

Expected shell/layout/package guide updates by phase:

| Phase | Authoring-contract update expected |
| --- | --- |
| Phase 18.1 | Architecture vocabulary, Masonry boundary, status markers, planned API inventory, conflicts/precedence, and anti-patterns documented here and in the primitive reference. |
| Phase 18.2 | Document implemented internal shell root, `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` runtime behavior while keeping public `clay:ui` package APIs marked planned/unavailable. |
| Phase 18.3 | Document runtime-backed public APIs for slot-aware `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` registration, examples, diagnostics, package metadata, package permissions, and generated registry/API coverage. |
| Phase 18.4 | Document implemented `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, and package option customization APIs. |
| Phase 18.5 | Document the Phase 18.5 authoring contract: no default fixed panel unless explicitly registered, optional preview as `PanelContribution` with `defaultVisibility: "hidden"`, main editor placement via `PaneSlotLayout.main`, theme token usage, and `setPackageOption`/`serverSetLayoutOverride` customization. Update Markdown package docs to consume generic shell/layout primitives and remove fixture-only UI guidance from user-facing defaults. |
| Phase 18.6/Plan 035 | Document the shipped one-line `loadPackage("@clay/markdown")` loader, source-aware package loading, and `PackageLoadEntryAllowlist` package-root boundary. |
| Phase 18.7 | Document persistent-runtime parse-handler registration, generic open-time mode activation, no-client-JS/no-hot-path-JS invariants, parse budgets, and forbidden per-mode/per-open shortcuts. |
| Phase 18.8 | Document the command execution lifecycle, inert action intents, transient menu sessions, and the difference between fixed panels, transient overlays, and bottom-pane transient menus. Update anti-patterns to reject client-side command execution, raw callbacks, and command-permission bypass. |
| Phase 18.12 | Document the file-browser-era shell contract: Clay owns workspace discovery, bounded listing, the left file tree, bottom fuzzy-open sessions, and workspace command routing; packages may reuse generic panel/overlay/action primitives but cannot add roots, markers, ignore rules, raw file listing, native widgets, or direct filesystem authority. |
| Phase 18.13 | Document the read-only Git package contract: `@clay/git` consumes the server-owned `clay:git` discovery facade, declares no permissions, publishes a sanitized status panel, and receives no shell/network/filesystem/mutating Git authority. Branch/status commands are server-owned built-ins (`clay.git.listStatuses`, `clay.git.refreshStatus`); the package only composes read-only display state. Mutating Git operations remain deferred. |
| Phase 18.15 | Document the locked text vocabulary (`TokenType` + `Modifiers`), inert `textStyles` theme-package contract, one-active-theme `setTheme()` selection API, and the separation between SDUI typed theme tokens and editor text `StyleRegistry` overrides. |
| Phase 18.17 | Document bounded analyzer-owned `DiagnosticSpan`/`DiagnosticSet` publication through `serverPublishDiagnostics`, theme severity colors, additive squiggle rendering, Tree-sitter authority exclusion, and the no-LSP-process boundary. |
| Phase 20.1 | Document the expanded typed token catalog (ten domains: `color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, `density`), the seven semantic `UiTextVariant` tokens and the user-owned `UiTypographyHierarchy`, `clay.contributions.designTokens` typed UI overrides shipped on `ActiveTheme`, and token-backed panel/sidebar/density defaults. Packages reference tokens and select variants only; they cannot ship concrete hierarchy scales, raw values, or new component kinds. |
| Phase 20.3 | Document layout primitives: split divider drag, fixed slot resize/collapse, layout persistence, inert versioned `LayoutIntent` API (`serverRequestLayoutIntent`), focus/input routing across splits, transient surface anchoring, and package limitations (no native layout mutation, no raw widget access). |
| Phase 20.4 | Document the core component uplift: every implemented `ComponentKind` now honors the active theme (SDUI paint reads `ResolvedUiTheme`, not core fallbacks), is state-complete (`Rest`/`Hover`/`Active`/`Focus`/`Disabled` from state tokens), and follows the 4pt spacing rhythm scaled by `density`/`spacing_scale()`; the status bar uses token-driven insets; editor chrome (caret/selection/scrollbar/diagnostics) stays on the editor `StyleRegistry`. Compatibility guarantee: no `ComponentKind`, style-variable, or token-name change — packages require no manifest or style edit. |
| Phase 22.2 | Document the pane document-view contract: each pane hosts at most one document of its tab's workspace (`PaneDocumentView`; the pane↔document mapping is client-local view state, server authority unchanged), duplicate opens focus the existing pane, all open flows target the focused pane, and major modes run concurrently per pane via per-document behavior-manifest layers. Packages gain no new surface: they still cannot own panes, open documents into panes, or contribute per-pane chrome (SDUI sidebar/panels/overlays remain window-scoped; per-pane package chrome stays planned). |
| Phase 22.3 | Document the tab contract: tabs are independent client views — each tab owns its own server connection, workspace, split tree, chrome, and pending-open attribution; the server holds an in-memory server-authoritative tab registry (order, active tab, per-tab workspace + client binding) that survives client reconnects (disk persistence is 22.5); the tab bar is a shell-owned chrome row below the top fixed panel slot above the working area, hidden at ≤1 tab. Packages cannot own or contribute tab bar chrome and cannot open/close/move tabs (inert `serverRequestLayoutIntent` remains the only package layout surface); per-tab package chrome is still-planned (needs a later phase). |
| Phase 22.4 | Document keyboard tab management: 24 Clay-owned `client_ui` tab command IDs (`clientTabNext`/`Prev`/`New`/`Close`/`MoveLeft`/`MoveRight` plus the numbered `clientTabActivate.1..9` and `clientTabMoveTo.1..9` families) with Global-scope default chords, server-registry reorder ops, explicit numbering/bounds/wraparound policies, and a driver-owned dirty-close confirm/save flow. Packages gain no surface: binding is a **user** configuration-time API (`clay.keybindings.bindKey`/`unbindKey` in `~/.config/clay/init.js`), packages cannot bind or issue tab commands, cannot open/close/move/reorder tabs, and receive no new authority from the tab command IDs. The tab commands are `ClientUiCommand`-routed and — like the pane commands — are not Control Center-executable by design. |
| Phase 20.5 | Document the overlay, menu, and input component phase: `dropdown`, `collapse`, `modal` promoted from reserved to implemented; `textInput` added (focus, placeholder, `style.validationState`/`style.placeholderColor`); `table` remains reserved (no first-party need); all transient surfaces (command palette, context menu, menu bar, completion pop-up) uplifted onto shared `paint_package_overlays` + `paint_tooltip_shell` with z-level stacking (`z.overlay`<`z.modal`<`z.tooltip`); `TransientMenuOrigin` selects overlay anchor; keyboard nav complete for all new surfaces (dropdown ArrowUp/Down/Enter/Space, collapse Enter/Space, modal Tab focus-trap). Compatibility guarantee: no existing `ComponentKind`, style-variable, or token-name change; `placeholderColor` and `validationState` are additive. |
| Plan 070 | Document the retained reconciliation cutover: SDUI kinds and package component trees now render through retained reconciled Masonry subtrees (`SduiRegionWidget` in `src/masonry_sdui_region.rs`, `PackageRegionWidget`/`PackagePanelHost`/`PackageOverlayHost` in `src/masonry_package_region.rs`) hosted as real children of `EditorWidget`, replacing the earlier immediate-mode `SduiNativeState::paint` compatibility bridge. Each kind maps to a real Masonry widget (`SduiLabel`/`SduiButton`/`SduiListRow`/`EditorViewWidget`, `PackageButton`/`PackageListRow`/`PackageCollapse`/`PackageDropdown`/`PackageTextInput`/`PackageModal`); Masonry routes layout/paint/pointer/focus/scroll/a11y through the standard widget tree. Compatibility guarantee: no `ComponentKind`, style-variable, token-name, or package-facing contract change — packages require no manifest or style edit; the cutover is a client-internal substrate change. |
| Phase 22.1 | Document equal-area window splits: Clay-owned `PaneSplitTree` now supports `split_pane` (capped at 4 panes per tab), `close_pane`, `add_equal_pane` (comb tree, N+1 equal areas), `move_pane` (reading-order swap), and `keyboard_resize` (deepest-bordering divider, clamped). `ClayShellWidget` hosts one `PaneContentHost` per pane leaf (editor or placeholder); panes are generic content hosts, not just editor views — a future terminal emulator or other workspace app can occupy a pane without a new shell primitive. Packages **cannot** own, create, close, move, or directly mutate panes/splits; they interact only through the inert `serverRequestLayoutIntent` API (Phase 20.3). Direct topology mutation (`serverRegisterPaneSplitTree`) stays a planned stub. Default split/focus/resize keybindings are Clay-owned and user-overridable via `bindKey`. |

Phase 18.3 `clay:ui` contribution examples for panels, components, overlays, and theme tokens are runtime-backed public APIs. Historical Phase 18.3 status used the row `PackageLayoutOverride` | `clay.ui.serverSetLayoutOverride` | Planned for documented user/package layout overrides.; Phase 18.4 promotes that surface. Phase 18.6/18.7 promote the `loadPackage("@clay/markdown")` default, persistent-runtime mode/parse registration, and generic selected-file open-time activation. Plan 035 generalizes `loadPackage` to installed, authorized source-aware packages. Phase 19 hot reload is implemented as ordinary one-line `loadPackage` re-evaluation in a fresh runtime generation (see [Package Reload Lifecycle](#package-reload-lifecycle-phase-19)); no package-specific reload callback, `force` flag, or copied-manifest bootstrap is required. Examples for working-area layout, pane splits, pane-slot mutation, durable state-value mutation, and package enable/disable from configuration remain **Planned/target** design, not callable code. The Phase 18.2/18.3 Rust shell runtime shapes are not package author APIs.

## Package Manifest

Every package must have a normal JavaScript package manifest plus Clay metadata.

Example:

```json
{
  "name": "@clay/markdown",
  "version": "0.1.0",
  "type": "module",
  "exports": {
    ".": "./dist/index.js",
    "./load": "./dist/load.js",
    "./parser": "./dist/parser.js"
  },
  "clay": {
    "apiPrefix": "markdown",
    "entry": "./dist/index.js",
    "loadEntry": "./dist/load.js",
    "permissions": [
      "mode-registration",
      "mode-activation",
      "command-registration",
      "parse-document",
      "render-decorations"
    ],
    "modes": ["markdown"],
    "docs": "./docs/index.md",
    "performance": {
      "estimatedManifestBytes": 2048
    },
    "apiDependencies": [
      "clay.modes.serverRegisterModePattern",
      "clay.modes.serverActivateMajorMode",
      "clay.commands.serverRegisterCommand",
      "clay.parse.serverRegisterParseHandler",
      "clay.decorations.serverPublishDecorations",
      "clay.ui.serverRegisterPanelContribution",
      "clay.ui.serverRegisterThemeToken",
      "clay.ui.serverRegisterInputContribution",
      "clay.ui.serverRegisterUiStateScope",
      "clay.ui.serverSetLayoutOverride",
      "clay.configuration.setPackageOption"
    ],
    "contributions": {
      "commands": [],
      "configuration": [],
      "keyRouting": [],
      "textTransforms": [],
      "sdui": [],
      "decorations": [],
      "ui": {
        "panels": ["markdown.preview"],
        "components": ["markdown.preview.root"],
        "overlays": ["markdown.preview.quickActions"]
      },
      "themeTokens": ["markdown.preview.background"],
      "input": ["markdown.preview.input"],
      "uiStateScopes": ["markdown.preview.visibility"],
      "layoutOverrides": ["markdown.preview:visibility"],
      "packageOptions": ["markdown.layout.defaultVisibility"]
    }
  }
}
```

Phase 18.4 accepts detailed object descriptors for `input`, `uiStateScopes`, `layoutOverrides`, and `packageOptions` when packages want load-time diagnostics before their runtime-backed APIs run. These descriptors preserve package provenance and reject duplicate input, duplicate UI state scope, duplicate layout override, and duplicate package option metadata before activation. Input descriptors validate package-prefixed input/component IDs, pointer/focus/selection policies, manifest-declared context modes, registered actions through command action targets, payload budgets, key-routing rejection, and raw callback/native/raw-op/CSS/client-JS rejection. UI state-scope descriptors validate package-prefixed IDs, no hidden path segments, supported scope/owner/lifetime/persistence/status values, target IDs, bounded schema kinds, state-value rejection, payload budgets, and provenance. Layout override descriptors validate package-prefixed targets, `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback` properties, package-default/global-package sources, typed values, registered inputs/actions/tokens, `package-configuration` permission, and no hidden/ad hoc keys. Package option descriptors validate package-prefixed supported option schemas (`layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`), typed defaults, payload budgets, `package-configuration` permission, and hidden-key rejection.

### Manifest requirements

- `name`: package name, commonly npm-compatible such as `@clay/markdown`.
- `version`: package version.
- `type`: normally `module`.
- `exports`: package JS entry points.
- `clay.apiPrefix`: short package-owned prefix, such as `markdown`.
- `clay.entry`: runtime entry.
- `clay.loadEntry`: load/default setup entry.
- `clay.permissions`: explicit permissions required by contributions.
- `clay.modes`: package-owned modes.
- `clay.docs`: package docs entry point.
- `clay.performance.estimatedManifestBytes`: static budget estimate.
- `clay.apiDependencies`: Clay JS APIs the package depends on.
- `clay.contributions`: inert contribution descriptors for validation/conflict checking.

Clay validates this metadata before package contributions become active. Phase 18.3 UI metadata is inert: it lets Clay diagnose duplicate panel/component/overlay/token IDs, fixed-slot collisions, unsupported component/style fields, invalid token fallbacks, and payload estimates during package load, but it does not install a package, execute package JavaScript, or grant panel/theme override authority by itself. Phase 18.4 input/state/configuration metadata is also inert: it lets Clay diagnose duplicate input IDs, state scope IDs, layout override targets/properties, and package option schemas with package name/version/apiPrefix provenance before enable/load; runtime behavior still flows through documented `clay:ui` and `clay:configuration` APIs.

## Package Prefix and IDs

Every package-owned public ID must use the package prefix.

Good:

```text
markdown.togglePreview
markdown.preview-panel
markdown.list-continuation
markdown.inline-code
```

Bad:

```text
togglePreview
preview-panel
clay.markdown.togglePreview
shell.run
```

Only first-party Clay core APIs may use the `clay.*` namespace.

## Permissions

Clay rejects packages that contribute permission-bearing behavior without declaring the permission.

Common permission scopes:

| Permission | Purpose |
| --- | --- |
| `mode-registration` | Register document/mode matching metadata |
| `mode-activation` | Activate a mode for a document |
| `command-registration` | Register package commands |
| `parse-document` | Receive bounded open-document text for parsing |
| `render-decorations` | Publish inert decoration spans |
| `render-folding` | Publish folding ranges when implemented |
| `completion-provider` | Provide completions when implemented |
| `package-configuration` | Behavior-changing package options when implemented |
| `editor-control` | Programmatic cursor/selection control in declared modes (see below) |

Phase 18.3 `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` declarations require no new permission when they are inert metadata. Their embedded action targets inherit the target command's registration/permission requirements, and future user overrides require documented configuration APIs and `package-configuration` where behavior-changing.

A permission declaration does not grant broad authority. Packages still cannot access arbitrary filesystem paths, network, shell, AI mutation, WASM, native widget handles, raw Deno ops, or client-side JavaScript by default.

## Editor Control (`editor-control`)

Packages that need to move the caret or change selection programmatically (AI-assisted symbol selection, snippet flows, macro replay) declare the `editor-control` permission **and** the exact modes they operate in:

```jsonc
"clay": {
    "permissions": ["editor-control"],
    "editorControl": {
        "modes": ["markdown", "core.code"] // exact mode IDs; no wildcards
    }
}
```

Boundary rules (enforced per call, deny-by-default):

- Every editor op requires approved `editor-control`; visibility of the ops grants nothing.
- The active document's major mode must be one of the declared `editorControl.modes`. Modes may be foreign (e.g. `core.code`) — a package does not need to own a mode to operate in it, but it must name it.
- Execution is triggered with `clientExecuteEditorCommand({ commandId })` (Plan 071 follow-up round). Only known editor command IDs are accepted; the request is pushed to the client as an advisory `EditorCommandRequest` and dispatched through the same path as keybinding-routed command IDs. Unknown IDs are dropped on both sides.
- Keybinding-driven behavior needs no push channel: a package that owns a mode contributes `keymaps` and `editorRules` through its mode declaration, and manifest routing takes precedence over built-in defaults once the package is activated.
- Conflicts: multiple packages may hold `editor-control` for the same mode; Clay does not arbitrate. If two packages fight over behavior in a mode, deactivate one (package disable/adoption revoke applies live via runtime reload).
- Revocation is immediate: disabling or unadopting the package removes the capability on the next runtime generation.

## Loading Packages from init.js

Package loading status:

- **Implemented end-user default:** users explicitly load packages from `~/.config/clay/init.js` with `await loadPackage("@clay/markdown")` or another installed, authorized package specifier. The resolver validates the specifier, runs the package metadata through `PackageService`, checks capability grants, enables the package, and imports and executes its declared `loadEntry` under Clay's authority. No inline manifest, no per-primitive registration, and no manual `clay` facade plumbing are required in user config. See `docs/reference/primitives/package-loading.md` for the package-root boundary, runtime-generation hot reload behavior, and carried-forward durable state work.
- **Implemented/runtime-backed today:** `loadPackage(specifier)` is the one-line end-user default for bundled and installed source-aware packages. `serverLoadPackage(packageJson)` remains a lower-level validation helper for fixtures and controlled configuration tests.
- **Phase 18.4 customization status:** optional customization after the future one-line load uses documented `setPackageOption` and `serverSetLayoutOverride` APIs. These are startup/configuration-change/package-load/update-time validators, not hidden JSON/TOML/ad hoc keys and not package enable/disable authority.

**Implemented default loader shape**:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
```

Implemented package-record validation helper:

```js
import { serverLoadPackage } from "clay:packages";

const loaded = serverLoadPackage(packageJson);
```

Do not present `serverLoadPackage` as ordinary end-user setup. It is useful for controlled package/configuration fixtures and load-contract validation, not for package installation or enablement; the end-user default is the runtime-backed `loadPackage(specifier)` facade.

Optional user configuration should be separate and explicit after the one-line package load helper exists:

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

Phase 18.6 shipped the generic one-line loader. Phase 18.7 extends it through selected-file open-time activation: startup `~/.config/clay/init.js` evaluates on the persistent server runtime, `await loadPackage("@clay/markdown")` validates/enables the package once, imports its declared `loadEntry`, registers mode metadata and parse handlers, and leaves those registrations resident for later opens. Opening `note.md` then classifies the path through the generic `clay:modes` registry, activates the matching mode for that document, and schedules the package parse handler through `ParseCoordinator`; user config does not copy package manifests, call raw ops, perform manual primitive registration, publish representative decoration publication payloads, or build per-open runtime roots. Plan 035 generalizes the resolver so `src/server/js_runtime.rs::ClayModuleLoader` accepts resolver-validated package `loadEntry` modules through a shared `PackageLoadEntryAllowlist` gate for bundled and installed source-aware packages. `loadPackage` is idempotent per runtime generation, so repeated startup/open-time calls reuse the first validated load; Phase 19 reload replaces the runtime generation, reruns `init.js`, rebuilds the package `loadEntry` allowlist, and starts the `globalThis.__clayLoadedPackages` cache empty. The `PackageService` resolve/enable/execute path (`src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier`) is implemented and wired into the `clay:packages` facade. The `clay.packages.loadPackage` inventory entry is `status = "runtime-backed"` and `registry_public = true` with full Markdown documentation. The generic loader/API boundary is a package-root allowlist that does not grant filesystem, network, shell, AI, WASM, raw-op, native-widget, client-JS, or package-manager authority without separate user-approved capabilities. See `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` for the unified authority model. The package-owned `markdownLoadMode()` fallback remains a documented convenience alias for per-load options, but `loadPackage("@clay/markdown")` is the preferred end-user path.

If a package supports one-line loading, that is the preferred path. The lower-level setup should be documented as a fallback for advanced use or per-load customization.

What the one-line default yields (Plan 071 task 11): after `await loadPackage("@clay/markdown")`, a `.md` document classifies to the Markdown mode and activates with the package-declared `editorRules` — prose word movement (`wordSeparators: "prose"`, no underscore or camelCase sub-words), the editor default bar caret, and ligatures from the mode's proportional font-role typography profile. The built-in `core.code`/`core.text` fallback modes ship the same defaults (code movement, default caret, role ligatures) with no package loaded, and a package load never changes unrelated modes. Customization is optional and declarative (`movement`/`caretStyle` in `editorRules`, ligatures via `setTypography` per font role); see the Behavior manifest bullets below.

A single `loadPackage("@clay/<lang>")` activates every contribution the package's `loadEntry` registers — modes, commands, parse handlers, decorations, UI components, and any number of completion providers. A package declares multiple providers in its `completionProviders` array (for example `rust.keywords` plus `rust.snippets`) and submits that package manifest once to `serverRegisterCompletionProvider`; generic `completion_provider_metas` maps the full array and `register_completion_provider_metadata` registers all distinct IDs together while rejecting duplicates. End-user config stays one line — `await loadPackage("@clay/rust")` — with no copied manifest, manual primitive registration, or low-level facade plumbing. Phase 18.19 snippets ride this same path: `textFormat: "snippet"` is inert item data, not a separate loader, op, permission, or subsystem.

## Package Code Shape

A typical package should separate:

```text
src/index.js       public exports
src/load.js        default package load/setup
src/parser.js      parser or provider code
src/ui.js          optional UI contribution helpers
src/config.js      optional configuration helpers
src/theme.js       optional theme token declarations
```

Compiled packages may publish `dist/` equivalents.

**Implemented persistent-runtime load entry shape** for a package `loadEntry`:

```js
import { serverRegisterCommand } from "clay:commands";
import { serverRegisterModePattern, serverActivateMajorMode } from "clay:modes";
import { serverLoadPackage } from "clay:packages";
import { serverRegisterParseHandler } from "clay:parse";
import { markdownPackageManifest } from "./index.js";

export async function loadMarkdownPackage(clay, options = {}) {
  const packageManifest = markdownPackageManifest();

  await clay.packages.serverLoadPackage(packageManifest);

  await clay.modes.serverRegisterModePattern(packageManifest, {
    modeId: "markdown",
    displayName: "Markdown",
    extensions: ["md", "markdown", "mdown"],
    mimeTypes: ["text/markdown"],
    editorRules: MARKDOWN_EDITOR_RULES,
    commands: MARKDOWN_COMMANDS,
    keymaps: MARKDOWN_KEYMAPS,
  });

  // Optional load-time activation for an explicit document. Selected-file open
  // later uses serverActivateClassifiedMode with the metadata stored above.
  await clay.modes.serverActivateMajorMode(packageManifest, {
    documentId: Number(options.documentId ?? 1),
    path: String(options.path ?? "sample.md"),
    editorRules: MARKDOWN_EDITOR_RULES,
    commands: MARKDOWN_COMMANDS,
    keymaps: MARKDOWN_KEYMAPS,
  });

  for (const command of MARKDOWN_COMMANDS) {
    await clay.commands.serverRegisterCommand(packageManifest, {
      commandId: command.id,
      displayName: command.displayName,
      routingPolicy: command.routingPolicy,
    });
  }

  const parserModule = await import("./parser.js");
  await clay.parse.serverRegisterParseHandler({
    packageManifest,
    mode: "markdown",
    parseUnit: "line-group",
    viewportPriority: true,
    module: parserModule,
    exportName: "parseMarkdownDecorationUpdate",
    maxWindowBytes: 64 * 1024,
    guardBytes: 4 * 1024,
    memoryBudgetBytes: 30 * 1024 * 1024,
    timeoutMs: 50,
  });
}

export default async function markdownLoadMode(options = {}) {
  return loadMarkdownPackage({
    packages: { serverLoadPackage },
    modes: { serverRegisterModePattern, serverActivateMajorMode },
    commands: { serverRegisterCommand },
    parse: { serverRegisterParseHandler },
  }, options);
}
```

The public registration contract is token-backed. `serverRegisterParseHandler` accepts a package module object plus `exportName`; the facade stores that function in the persistent server runtime behind a server-issued token. Rust never receives a JavaScript callback value. The op validates package identity, `PackagePermission::ParseDocument` / `"parse-document"`, parse unit, window/memory budgets, and timeout bounds before a `ParseCoordinator` handler is registered.

**Generic future-mode shape** (same primitives, no Markdown-specific Rust branch):

```js
import { serverRegisterCommand } from "clay:commands";
import { serverActivateMajorMode, serverRegisterModePattern } from "clay:modes";
import { serverLoadPackage } from "clay:packages";
import { serverRegisterParseHandler } from "clay:parse";
import * as parserModule from "./parser.js";
import { myLanguageManifest } from "./index.js";

export default async function loadMyLanguage(options = {}) {
  const packageManifest = myLanguageManifest();
  await serverLoadPackage(packageManifest);
  await serverRegisterModePattern(packageManifest, {
    modeId: "my-language",
    displayName: "My Language",
    extensions: ["my"],
    editorRules: MY_LANGUAGE_EDITOR_RULES,
    commands: MY_LANGUAGE_COMMANDS,
    keymaps: MY_LANGUAGE_KEYMAPS,
  });
  await serverActivateMajorMode(packageManifest, {
    documentId: Number(options.documentId ?? 1),
    path: String(options.path ?? "example.my"),
    editorRules: MY_LANGUAGE_EDITOR_RULES,
    commands: MY_LANGUAGE_COMMANDS,
    keymaps: MY_LANGUAGE_KEYMAPS,
  });
  for (const command of MY_LANGUAGE_COMMANDS) {
    await serverRegisterCommand(packageManifest, command);
  }
  await serverRegisterParseHandler({
    packageManifest,
    mode: "my-language",
    parseUnit: "line-group",
    module: parserModule,
    exportName: "parseMyLanguageUpdate",
    maxWindowBytes: 64 * 1024,
    guardBytes: 4 * 1024,
    memoryBudgetBytes: 30 * 1024 * 1024,
    timeoutMs: 50,
  });
}
```

Keep package docs current with the implemented Clay JS API reference; do not invent raw op or callback shortcuts.

### Semantic typography roles

Use [Semantic Typography Roles](../primitives/typography.md) for every package-controlled text surface. Packages declare intent only; user [`setTypography`](../clay-js-api/theme/set-typography.md) configuration owns concrete family fallback stacks and logical-pixel sizes.

A mode may set one document default:

```js
await serverRegisterModePattern(packageManifest, {
  modeId: "example-code",
  displayName: "Example Code",
  extensions: ["example"],
  defaultFontRole: "monospace",
});
```

`defaultFontRole` accepts `monospace` or `proportional`; omission inherits the base behavior manifest. `core.code` defaults monospace, while `core.text` and Markdown default proportional. Rust, TypeScript, and JavaScript package modes declare monospace. Future modes must use this generic field, not language-specific Rust rendering branches.

Syntax grammar style maps and published syntax/semantic decorations may request a range override:

```json
{
  "code": { "styleToken": "markup.code-block", "fontRole": "monospace" }
}
```

```js
{
  byteStart: 10,
  byteEnd: 16,
  layer: "syntax",
  styleToken: "markup.inline-code",
  fontRole: "monospace",
}
```

Range `fontRole` accepts `monospace` or `proportional`. Diagnostic/search layers, stale/out-of-bounds spans, and invalid UTF-8 boundaries cannot alter font role. Component text separately defaults to `ui` and follows the `style.fontRole` contract in [Components](#components).

Never declare `fontFamily`, `fontFamilies`, `fontSize`, `fontStack`, font paths/bytes/URLs/downloads, raw CSS, raw Parley properties, or renderer callbacks. Semantic roles add no filesystem, network, shell, package-manager, extension, AI, WASM, workspace, native-widget, raw-op, or client-side JavaScript authority. Validation and role normalization remain outside paint/input/layout hot paths.

### Persistent runtime, open-time activation, and parse boundaries

The end-user default stays one line in `~/.config/clay/init.js`:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
// Bundled and user-installed packages use the same one-line path after install
// and user authorization:
await loadPackage("@vendor/foo");
await loadPackage("github:user/repo");
```

`@clay/*` only means a package was shipped with Clay — it is not a more capable package. After a user installs and authorizes an npm, GitHub, git-URL, tarball, or local-path package, that package loads through the identical `loadPackage` one-line path, the identical resolver + `PackageService` validation, and the identical runtime authority model. `init.js` itself grants no capabilities: it only requests one-line package loads and (optionally) separate documented Clay APIs. Every powerful capability (filesystem, network, shell, AI, WASM, raw-ops, native-ui, client-runtime, package-control, language-server) must be a separately implemented, user-approved authorization grant recorded against the package identity/source/provenance — `init.js` cannot silently grant them.

#### Language-server packages require grant then load

A process-backed bridge declares `language-server` in `clay.capabilities` and fixed inert launch metadata. Runtime package code cannot choose executable, arguments, cwd, environment values, or roots:

```json
{
  "clay": {
    "apiPrefix": "lsp-rust",
    "capabilities": ["language-server", "parse-document"],
    "contributions": {
      "languageServers": [{
        "id": "lsp-rust.server",
        "executable": "rust-analyzer",
        "args": [],
        "inheritEnvironment": ["HOME", "RUSTUP_HOME", "CARGO_HOME"]
      }]
    }
  }
}
```

User configuration explicitly grants exact contribution and current directory roots before loading:

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/lsp-rust");
```

`loadPackage` alone neither authorizes nor launches server. First load seals language-server authority changes before package `loadEntry` executes; bundled `@clay/*` trust does not bypass grant. Unknown roots, undeclared/mismatched contributions, changed source/version/descriptor, revoked grants, extra option fields, and post-load authorization fail closed. Grant validation is configuration/load-time work only. Process start/read/write/stop arrives through bounded host session primitive; contribution/grant task itself starts no child.

The grant scopes Clay's launch API and audit identity, not host OS access. Once implemented, same-user child may access files outside selected roots, network, and other processes. Treat approved server as trusted subprocess, not sandboxed code. See `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

`loadPackage` executes the package `loadEntry` once per runtime generation. The registered mode patterns, activation metadata, command declarations, and parse-handler token remain resident in that generation. Phase 19 hot reload replaces the runtime generation, reruns `~/.config/clay/init.js`, rebuilds package state, and reruns the same package `loadEntry` with an empty `globalThis.__clayLoadedPackages` cache. Package authors should rebuild all runtime state from `loadEntry`; they should not rely on mutable JavaScript globals surviving reload. Failed reloads keep the previous generation active and report sanitized diagnostics.

On selected-file open or successful reload refresh, Clay classifies the path through the generic `clay:modes` registry, uses `serverActivateClassifiedMode` with the stored activation metadata to activate the matching major mode for that document, then schedules a bounded parse through `ParseCoordinator`. User config does not reload the package per open and does not manually register every primitive. Parse handler registrations are generation-scoped: a newer generation replaces old handler tokens, cancels old-generation parse work, and rejects late old-runtime-generation task results before publication.

Package parse work follows the no-client-JS / no-hot-path-JS invariant: it is never client JavaScript and never hot-path JavaScript. Parser functions run only on the server runtime worker after edit/open work has already been accepted. Ordinary keypress, Masonry paint/layout, pointer, scroll, and text-event handling read inert manifests/protocol data and do not call package JS. Slow parse work can make decorations stale, but it must not block local text display or edit acknowledgement.

Budget contract for package parse handlers:

- `timeoutMs`: package-declared parse timeout, validated as `1..=5000` ms; JS invocation uses the smaller of this value and the service guard, and runaway handlers surface as `clay.runtime.timeout`.
- `maxWindowBytes` / `parseWindowBytes`: max bytes per bounded parse window; Markdown uses `64 * 1024`.
- `guardBytes`: optional context bytes around a window; Markdown uses `4 * 1024`.
- `memoryBudgetBytes`: total syntax/parse memory budget, capped by `SYNTAX_CACHE_BUDGET_BYTES` (30 MiB).
- Emitted `IncrementalParseUpdate` payloads must fit `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`; parse-produced decorations also pass `DECORATION_PAYLOAD_BUDGET_BYTES` validation before client delivery.

Security and authority contract:

- Handler registration requires `PackagePermission::ParseDocument` / `"parse-document"` in package metadata.
- Only validated packages register live handlers; install/enable metadata alone does not grant parser execution.
- `ClayModuleLoader` loads curated `clay:*` facades, controlled config modules, the vendored parser shim, and resolver-validated package `loadEntry` modules through the shared `PackageLoadEntryAllowlist`; hot reload preserves validated package-root confinement and rebuilds the allowlist in the fresh generation. Bundled and installed source-aware packages (npm, GitHub/git, tarball, local path) resolve through the same `PackageService` path after install and user authorization.
- Packages do not gain filesystem, network, shell, AI, WASM, raw-op, native-widget, package-manager, package-control, language-server process, or client-JS authority merely through loading, activation, UI contribution registration, or parsing; those capabilities require separate implementation and user approval.

Forbidden anti-patterns:

- Per-open runtimes or per-open `dist/` copies.
- Executable `handler`, `callback`, `onParse`, or `function` fields in the public parse registration payload.
- Raw `Deno.core.ops` calls as package/user-facing API.
- Markdown-only Rust branches such as `if path.ends_with(".md")`, `if mode_id == "markdown"`, or handwritten markdown-it token handling in server/client Rust.
- Publishing representative/fake decorations from `init.js` instead of returning an `IncrementalParseUpdate` from the package parse handler.
- Client-side JavaScript, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, or layout mutation hidden inside package UI/layout declarations.

## Package Reload Lifecycle (Phase 19)

Hot reload preserves the one-line end-user default. Users keep writing ordinary loads in `~/.config/clay/init.js`; Clay does not introduce `loadPackage(spec, { force: true })`, package-authored `onReload` callbacks, or copied manifests:

```js
import { loadPackage } from "clay:packages";
import { authorizeLanguageServer } from "clay:language-server";
import { bindKey } from "clay:keybindings";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/rust");
await loadPackage("@clay/lsp-rust");
// Optional: bind the built-in reload command. There is no default binding and
// no filesystem watcher; reload is explicit only.
bindKey("Ctrl+Shift+R", "clay.runtime.reloadConfiguration", { scope: "global" });
```

### What reload does

1. Evaluate a candidate generation off to the side (fresh `ClayJsRuntimeService`, empty `globalThis.__clayLoadedPackages`, rebuilt `PackageLoadEntryAllowlist`).
2. Rerun the same `init.js` one-line loads so every `loadEntry` rebuilds modes, commands, syntax grammars, completion metadata, UI contributions, parse handlers, and language-server analyzer registrations from durable package metadata plus current user grants/overrides.
3. Validate all contributions in isolation; commit only when every validator succeeds.
4. Acquire the behavior-scope lock, atomically install the candidate, fan out one bounded `RuntimeStateSnapshot` per client, cancel older-generation workers/sessions, and refresh open documents with generic mode activation plus bounded background reparsing.
5. On any failure, drop the candidate, keep the previous generation active, and emit sanitized diagnostics.

### Generation-local vs persistent state

| Kind | Survives reload? | Examples | Authoring rule |
| --- | --- | --- | --- |
| Generation-local | No | `globalThis.__clayLoadedPackages`, module closures, parse/completion/intelligence tokens, language-server sessions, package-owned caches | Rebuild from `loadEntry` every generation. Do not treat JS globals as persistence. |
| Explicitly persistent user/workspace state | Yes (outside the runtime generation) | Open documents, leases, workspace roots, user-approved package/process grants re-applied by `init.js`, documented package options/layout overrides | Read durable configuration/package metadata; re-apply through documented Clay JS APIs in `init.js` or `loadEntry`. |
| Unsupported migration hooks | N/A | `onReload`, `migrateState`, force-reload flags, in-place V8 module mutation | Not provided. Generation replacement is the only supported rebuild mechanism. |

The same lifecycle applies equally to Markdown, Rust, TypeScript, JavaScript, Git, themes, and LSP bridges. There is no language-specific Rust reload branch.

### Authoring, performance, and security rules

- Register contributions at load/reload time only. Do not perform synchronous package work before local paint; reparsing is viewport-prioritized, cancellable, and background.
- Reload does not broaden package source trust or permissions. Exact language-server grants must be re-declared with `authorizeLanguageServer` before `loadPackage` in the fresh generation; first load seals grants again; old-generation workers/sessions are cleaned after commit.
- Reject executable client declarations, raw ops, and renderer callbacks in package contributions. Failed candidate evaluation never mutates live registries, grants, or open-document state.
- Test reload by rerunning the same one-line `init.js` loads and asserting generation-scoped contributions return; do not invent package-private reload helpers for fixtures.

## UI and Layout Model

Clay owns a consistent shell layout for all packages and modes. See the [Clay Shell and Package UI/Layout Strategy](../primitives/shell-layout-strategy.md) for the canonical Phase 18.1/18.2 vocabulary/runtime status and for the rule that Masonry is Clay's internal widget/layout/rendering substrate, not a package author API.

### Unified UI/layout authoring contract across package sources

The UI/layout authoring contract is identical for `@clay/*` packages and user-installed packages (npm, GitHub, git URL, tarball, local path). `@clay/*` only means a package was shipped by Clay — it is not a more capable package. After a user installs and authorizes a package from any source, it contributes UI/layout through the same `clay:ui` facades, the same `PackageService` validation, the same shell/slot/precedence rules, and the same conflict-resolution policy as a bundled Clay package.

- User-installed packages may request the same UI/layout/native/client capabilities as Clay packages — `render-decorations`, `render-folding`, `completion-provider`, `package-configuration`, `package-control`, `native-ui`, and `client-runtime` — through the unified capability vocabulary, subject to the explicit user authorization grants described in [Unified Package Capability Model](../primitives/package-security.md#unified-package-capability-model). A package source never confers a capability implicitly; every powerful capability must be a separately implemented, user-approved grant recorded against the package identity, source, and provenance.
- Native UI and client runtime are explicit capability/API work. A package does not get native widget handles, Masonry mutation, raw CSS, client-side JavaScript, or renderer callbacks merely because it was installed from npm or GitHub — those surfaces appear only when a documented `native-ui` / `client-runtime` capability is granted and a matching Clay API exists, is validated, and is revocable.
- UI/layout declarations remain validated load/reload/configuration work. Panel, component, overlay, input, state-scope, layout-override, theme-token, and option contributions are validated at package load/enable time and applied through documented Clay JS APIs at configuration/package-update time; no package JavaScript runs in Masonry paint, layout, pointer, scroll, keypress, text-event, or edit-ack handlers.
- UI/layout primitives stay generic and reusable. No UI/layout primitive branches on package source (no `if github_package` / `if npm_package` / `if third_party` Rust paths). Every package consumes the same shell/slot/component/theme primitives; Markdown and future modes consume these generic primitives rather than adding mode-specific Rust layout branches.
- Editor chrome is not SDUI (Plan 071 task 12). Caret shape/blink and the font-ligature baseline are editor/typography chrome, never `ComponentKind` components or theme tokens. Packages contribute them only as inert manifest/configuration data: `editorRules.caretStyle` in the mode's behavior manifest (shape/blink/dimensions; omitted means the editor default bar) and the mode's `defaultFontRole` selecting the typography profile whose user-owned `ligatures` policy applies. Caret color remains the theme-owned `caret` token. No package capability grants caret-shape or ligature-policy override authority; omitted fields always fall back to the built-in defaults, so customization is strictly opt-in. See [UI Chrome Primitives](../primitives/ui-chrome-primitives.md#package-authoring-contract) and [Semantic Typography Roles](../primitives/typography.md#ligature-policy).

```text
WorkingArea
└── PaneTree
    ├── Pane
    │   ├── top panel slot?    fixed or transient
    │   ├── left panel slot?   fixed or transient
    │   ├── main slot          mandatory
    │   ├── right panel slot?  fixed or transient
    │   └── bottom panel slot? fixed or transient
    └── Split/Pane ...
```

### Working area

The working area is the drawable Clay client region inside the native OS window.

### Pane/split tree

A working area may contain one pane or multiple panes split horizontally/vertically. Packages may request defaults or actions that open/split panes, but Clay validates and composes the result.

### Slots

Each leaf pane has these slots:

- `main` — mandatory, usually the editor or primary view.
- `left` — optional side panel.
- `right` — optional side panel.
- `top` — optional top panel/tool/status area.
- `bottom` — optional bottom panel/output/status area.

### Current Phase 18.3 runtime behavior

Clay now launches through an internal native shell root that contains the editor as the `main` component of a pane. Internally, the shell can represent one leaf pane or generic horizontal/vertical splits, and each leaf pane computes slot geometry from a mandatory `main` slot plus optional fixed `left`, `right`, `top`, and `bottom` slots. The current SDUI sidebar uses a Clay-owned left-slot bridge so existing status/SDUI behavior keeps working.

Phase 18.3 adds runtime-backed package contribution registration for fixed panels, component trees, transient overlays, and package theme tokens. Package code may register those declarations through `clay:ui` facade functions in the server runtime. Clay validates them, preserves provenance, composes accepted fixed panels through `PaneSlotLayout`, renders transient overlays without consuming fixed slot geometry, and routes UI actions only as registered command intents.

This still does **not** expose the whole shell as a package authoring API. Packages cannot create working areas, mutate pane split ratios, directly set pane-slot layouts, change shell configuration, persist UI state, or override user layout/theme choices through `clay:ui` in Phase 18.3. Those surfaces remain planned for Phase 18.4 unless a later task promotes them with full docs, registry entries, and tests.

### Fixed panels

A fixed panel participates in layout and reduces the main slot size.

Use fixed panels for:

- file trees
- outlines
- preview panes
- diagnostics panels
- terminals/output areas when implemented

### Transient panels

A transient panel overlays the main layout and is dismissible.

Use transient panels for:

- command palettes
- dropdowns
- hover docs
- modals
- temporary find/replace bars

## Conflict and Precedence Contract

The Phase 18.3 panel/component/overlay/token contribution APIs are runtime-backed; broader shell/layout override APIs are still planned. Package authors should design declarations around this deterministic precedence order:

1. Clay shell safety invariants and hard prohibitions
2. User configuration through documented Clay JS APIs
3. Active major mode layout defaults
4. Compatible minor mode contributions
5. Global package contributions
6. Package fallback/defaults

Clay validates every layer before it affects the shell. A user override can change a package/default layout request such as default panel visibility, preferred slot, panel order, or token mapping only through documented `~/.config/clay/init.js` Clay JS APIs. It cannot grant permissions, bypass slot safety, expose native widgets, accept raw CSS, or run package JavaScript in the client.

Package authors should expect deterministic diagnostics for:

- duplicate slot claims or ambiguous fixed/transient panel claims
- duplicate panel IDs, component IDs, overlay IDs, or theme token IDs
- duplicate command/action IDs or unregistered action targets
- invalid `clay.ui.*` API dependencies, package prefixes, unsupported slots, visibility values, overlay anchors, focus policies, or dismissal policies
- undeclared permissions for permission-bearing primitives
- unsupported state scopes or hidden state keys
- unknown typed style variables, unknown style/theme tokens, raw token values, or type-incompatible token fallbacks
- raw CSS, raw style strings, raw ops, native widget handles, Masonry widget constructors, client-side JavaScript, and native renderer callbacks
- oversize layout/component/state payloads and `estimatedPayloadBytes` declarations beyond budget

No package wins a layout conflict by load order alone. If a package needs slot priority, multi-panel ordering, overlay z-order, persisted pane selectors, or cross-window layout behavior, wait for a documented Clay API rather than inventing package-specific keys.

## UI Contribution Example

**Implemented/runtime-backed SDUI example** (current, not the final slot-aware shell contract):

```js
import { defineButton, defineLabel, definePanel, publishTree } from "clay:sdui";

await publishTree(
  definePanel({
    id: "markdown.preview.status",
    title: "Markdown",
    children: [
      defineLabel({ text: "Markdown preview ready" }),
      defineButton({
        label: "Toggle Preview",
        action: {
          commandId: "markdown.togglePreview",
          arguments: { source: "preview-button" },
        },
      }),
    ],
  }),
);
```

The current `clay:sdui` helpers publish bounded inert node trees through server validation. They do not create Masonry widgets directly, run client-side JavaScript, own pane slots, or define the future package layout contract.

`clay:ui` inventory targets for the shell/layout contract include:

| Primitive | Inventory target | Phase 18.3 package-facing status |
| --- | --- | --- |
| `WorkingAreaLayout` | `clay.ui.serverRegisterWorkingAreaLayout` | Internal Rust runtime implemented; public callable layout-default API planned/unavailable. |
| `PaneSplitTree` | `clay.ui.serverRegisterPaneSplitTree` | Superseded by `serverRequestLayoutIntent` (Phase 20.3); internal Rust runtime implemented; direct split-tree mutation API unavailable. |
| `PaneSlotLayout` | `clay.ui.serverSetPaneSlotLayout` | Internal Rust runtime implemented; public callable slot-layout/default API planned/unavailable. |
| `PanelContribution` | `clay.ui.serverRegisterPanelContribution` | Implemented/runtime-backed public API with per-API Markdown and generated registry coverage. |
| `ComponentContribution` | `clay.ui.serverRegisterComponentContribution` | Implemented/runtime-backed public API with per-API Markdown and generated registry coverage. |
| `TransientOverlayContribution` | `clay.ui.serverRegisterTransientOverlayContribution` | Implemented/runtime-backed public API with per-API Markdown and generated registry coverage. |
| `PackageThemeTokenDeclaration` | `clay.ui.serverRegisterThemeToken` | Implemented/runtime-backed public API with per-API Markdown and generated registry coverage. |
| `PackageUiStateScope` | `clay.ui.serverRegisterUiStateScope` | Implemented/runtime-backed public API for inert UI state schema/lifecycle declarations with per-API Markdown and generated registry coverage. |
| `PackageLayoutOverride` | `clay.ui.serverSetLayoutOverride` | Implemented/runtime-backed public API for documented user/package layout overrides with per-API Markdown and generated registry coverage. |

**Implemented/runtime-backed Phase 18.3 slot panel and token example:**

```ts
import {
  serverRegisterPanelContribution,
  serverRegisterThemeToken,
} from "clay:ui";
import { serverRegisterCommand } from "clay:commands";

serverRegisterCommand({
  id: "markdown.togglePreview",
  label: "Toggle Markdown Preview",
  routing: "ServerFirst",
});

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
    title: "Preview",
    style: { background: "markdown.preview.background", padding: "spacing.panel" },
    children: [
      { kind: "label", id: "markdown.preview.empty", text: "Preview unavailable" },
      {
        kind: "button",
        id: "markdown.preview.toggle",
        label: "Toggle Preview",
        action: { commandId: "markdown.togglePreview", arguments: { source: "preview-panel" } },
      },
    ],
  },
});
```

**Implemented/runtime-backed Phase 18.3 transient overlay example:**

```ts
import { serverRegisterTransientOverlayContribution } from "clay:ui";

serverRegisterTransientOverlayContribution(manifest, {
  id: "markdown.preview.quickActions",
  anchor: "main",
  focusPolicy: "restore",
  dismissalPolicy: "escape-or-outside",
  actionTargets: ["markdown.togglePreview"],
  component: {
    kind: "overlay",
    id: "markdown.preview.quickActions.root",
    children: [
      { kind: "button", id: "markdown.preview.quickActions.toggle", label: "Toggle preview", action: { commandId: "markdown.togglePreview" } },
    ],
  },
});
```

Packages should use these generic contribution APIs instead of teaching users to paste large fixture trees into `init.js`. Working-area, split-tree, and direct pane-slot mutation helpers remain planned until their own API docs and validators ship. Historical Phase 18.3 status used the row `PackageLayoutOverride` | `clay.ui.serverSetLayoutOverride` | Planned for documented user/package layout overrides.; Phase 18.4 promotes `serverSetLayoutOverride` as a runtime-backed configuration API.

## Components

Clay components are package-facing declarations mapped to native widgets internally.

Component catalog status (single source of truth: [`.agents/skills/clay-ui/references/components.md`](../../../.agents/skills/clay-ui/references/components.md); see also the [UI Components, Tokens, and Conformance](../ui-components.md) navigation page):

| Component kind | Status | Purpose |
| --- | --- | --- |
| `editorView` | Implemented/runtime-backed | Binds a UI region to an open document/editor surface. |
| `panel` | Implemented/runtime-backed | Titled or untitled container for slot UI. |
| `label` | Implemented/runtime-backed | Static text. |
| `button` | Implemented/runtime-backed | Actionable command button. |
| `list` | Implemented/runtime-backed | List of selectable/actionable items. |
| `flex` | Implemented/runtime-backed | Row/column composition. |
| `stack` / `overlay` | Implemented/runtime-backed | Layered/transient composition. |
| `scroll` / `portal` | Implemented/runtime-backed | Scrollable or portal-like component region. |
| `statusItem` | Implemented/runtime-backed | Status bar/panel status contribution. |
| `dropdown` | Implemented/runtime-backed (Phase 20.5) | Single-select drop-down; keyboard nav (ArrowUp/Down/Enter/Space). |
| `collapse` | Implemented/runtime-backed (Phase 20.5) | Expand/collapse section; Enter/Space toggles. |
| `modal` | Implemented/runtime-backed (Phase 20.5) | Blocking dialog; Tab focus-trap; `z.modal` stacking. |
| `textInput` | Implemented/runtime-backed (Phase 20.5) | Single-line editable field; focus ring, placeholder, `style.validationState`/`style.placeholderColor`. |
| `table` | Reserved/deferred | Structured rows/columns in a later component-catalog phase. |

Status markers match the `clay-ui` catalog legend: `implemented` (usable now), `reserved` (name locked, validation rejects use until its phase), `planned` (approved for a future UI revamp phase), `internal` (Clay-native surface, not package-facing). `table` is the only reserved kind; no planned package-facing kind remains unimplemented after Phase 20.5. The catalog's "Planned Components (UI Revamp)" table tracks composition-only planned surfaces (tooltip, badge, toast, kbd hint, icon slot) that reuse implemented kinds. `tabs` is implemented as a shell-level (internal, not package-facing) surface: the Phase 22.3 tab bar is a Clay-owned chrome row with token-state cards, not a package-facing `ComponentKind`.

Packages should not assume these are Masonry widget types. They are Clay components validated by `src/shell/components.rs` and rendered through Clay-owned native code.

### UI chrome conformance (Phase 20.2)

Phase 20.2 introduced a native chrome primitive layer (`src/shell/primitives.rs`) that is the only way to paint UI chrome (dividers, focus rings, panel backgrounds/borders, scrollbars, badges, keyboard hints, icon slots, tooltip shells). Primitives are `pub(crate)` inert paint helpers that read from `ResolvedUiTheme` tokens.

**Package mapping:** Package-declared `ComponentKind` components map onto primitives by construction. The SDUI paint path (`src/masonry_sdui.rs`) calls primitive helpers for chrome (panel backgrounds/borders, overlay backgrounds/borders, scrollbar chrome). Packages declare inert `ComponentKind` components only; they cannot call primitives directly.

**Conformance contract:**
- Packages must not directly create Masonry widgets, mutate native layout, provide raw CSS, run client-side JavaScript, or call raw `Deno.core.ops`.
- Packages must not attempt to paint UI chrome directly; chrome is painted by Clay-owned primitives.
- Package components are inert declarations; Clay renders them through native code and primitives.
- Primitive customization flows through token contributions (`clay.ui.serverRegisterThemeToken`, `clay.contributions.themeTokens`/`designTokens`), not package code.

**Enforcement:** The conformance contract is enforced by `tests/ui_primitive_conformance.rs`, which asserts:
- Shell/SDUI chrome paint files contain no `Color::from_rgb8`/`Color::from_rgba8` literals outside `primitives.rs` and `theme.rs`.
- Shell/SDUI chrome paint files contain no hardcoded chrome-size constants outside `primitives.rs` and `theme.rs`.
- Package components map onto primitives by construction (SDUI paint routes chrome through primitive helpers).
- Each primitive is token-driven and renders all declared interaction states.

See `.agents/skills/clay-ui/references/components.md` for the full primitive inventory and token mappings.

Component text defaults to the user-owned `ui` typography profile. Text-bearing `panel`, `label`, `button`, `list`, and `statusItem` declarations may request only a semantic `style.fontRole` of `"ui"`, `"monospace"`, or `"proportional"`; it selects the user-configured family stack and size together. Structural components and `editorView` cannot set `fontRole`. Packages must not provide `fontFamily`, `fontSize`, font stacks, raw Parley properties, CSS, font files, URLs, or renderer callbacks. `style.typography` remains a semantic Clay variant such as `typography.body`, `typography.title`, `typography.status`, `typography.display`, `typography.section`, `typography.detail`, or `typography.caption`, scaled from the configured role rather than an absolute size.

```json
{
  "kind": "label",
  "id": "markdown.preview.command",
  "text": "cargo test",
  "style": { "typography": "typography.body", "fontRole": "monospace" }
}
```

## Actions and Commands

Interactive UI routes through commands.

Implemented/current SDUI button action example:

```js
defineButton({
  label: "Toggle Preview",
  action: {
    commandId: "markdown.togglePreview",
    arguments: { source: "preview-button" },
  },
});
```

Command registration example:

```js
import { serverRegisterCommand } from "clay:commands";

serverRegisterCommand({
  id: "markdown.togglePreview",
  label: "Toggle Markdown Preview",
  routing: "ServerFirst",
});
```

Clay validates that:

- command IDs are registered before UI action targets become active
- package commands use package prefixes
- command permissions are declared and compatible with routing policy
- UI actions are inert command intents
- action arguments are bounded primitive data
- action arguments do not contain callbacks, raw op names, native handles, executable code, or authority-bearing filesystem paths
- stale action intents are rejected or disabled when command/component/package/document provenance no longer matches active state
- client UI commands do not mutate documents directly

Phase 18.4 component-scoped action routing composes this command contract with `serverRegisterInputContribution` and `serverSetLayoutOverride`: input/action defaults must reference registered package command IDs, declarations are validated at package load/configuration/update time, and Masonry hot paths read only installed inert action metadata.

Phase 18.8 adds a server-owned `CommandExecution` boundary. SDUI actions, package UI action intents, behavior-manifest keybindings, and transient-menu selections all normalize to the same `CommandExecutionRequest`. The server validates command ID, routing policy, package provenance, declared permissions, target context, and bounded arguments before any side effect. Packages may declare commands and expose them in transient menus; they cannot execute commands directly from UI callbacks, bypass permission checks, or run command handlers in the Rust client.

### Read-only server-facade packages (Phase 18.13)

Not every package owns a mode, parser, or command. A read-only package can compose display UI from a server-owned typed facade while declaring **no permissions at all**. The first-party `@clay/git` package is the reference shape:

```js
// ~/.config/clay/init.js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/git");
```

The package's `loadEntry` consumes the server-owned `clay:git` facade (`serverListGitStatuses`) and publishes an inert SDUI status tree (branch/dirty/refresh labels). The actual Git work — closed read-only command table, workspace-root confinement, bounded timeouts/output — stays in the server's `GitDiscoveryService` / `GitStatusCache`. The package declares `permissions: []`, lists no modes, registers no commands, and receives no shell, network, filesystem, or mutating Git authority. Branch/status commands (`clay.git.listStatuses`, `clay.git.refreshStatus`) are server-owned built-ins available regardless of package load; the package only adds the read-only status panel on top.

This is the preferred shape for packages that surface server-owned data: consume the documented facade, publish sanitized inert UI, and let the server own every capability. Mutating Git operations (checkout, stage, commit, reset, rebase, stash, push, pull, fetch) are deferred to a later phase with their own command authority UX. They will require explicit server-owned command IDs added to a closed command table, a new user-approved permission grant (the package's `permissions: []` is the read-only ceiling), conflict/state handling, and network authority for remote operations — never arbitrary argv, never a generic shell escape hatch, and never silent authority on this package.

## Input

Packages declare input interests. They do not receive raw arbitrary client input by default.

Use behavior manifests for client-first predictable editor behavior:

- Enter indentation
- Tab behavior
- bracket/quote pairing
- Markdown list continuation
- comment continuation
- autocomplete trigger declarations

Use commands for side effects and higher-level actions:

- toggle preview
- open file dialog
- open panel
- run export
- apply formatting command
- navigate UI

**Implemented/runtime-backed Phase 18.4 input contribution example:**

```js
import { serverRegisterInputContribution } from "clay:ui";

serverRegisterInputContribution(manifest, {
  id: "markdown.preview.input",
  scope: "component",
  componentId: "markdown.preview.root",
  pointer: {
    click: "action",
    action: "markdown.focusPreview",
    drag: "select",
  },
  focus: { policy: "restore-editor" },
  selectionPolicy: "component-local",
  context: { modes: ["markdown"] },
  actionTargets: ["markdown.togglePreview"],
});
```

Mouse/pointer input is declared by component/action metadata, not by package-owned client event handlers. `serverRegisterInputContribution` rejects `keys`, `keybindings`, and `onKey`; key routing remains behavior-manifest and `clay:keybindings` work.

**Implemented/runtime-backed Phase 18.4 state-scope lifecycle example:**

```ts
import { serverRegisterUiStateScope } from "clay:ui";

serverRegisterUiStateScope(manifest, {
  id: "markdown.preview.visibility",
  scope: "pane",
  targetId: "markdown.preview",
  owner: "shell",
  lifetime: "session",
  persistence: "client-local",
  implementationStatus: "implemented",
  valueSchema: { kind: "enum", values: ["visible", "hidden"] },
});
```

`serverRegisterUiStateScope` registers bounded schema and lifecycle metadata only. It rejects hidden path segments such as `markdown._secret`, raw/default state values, unsupported scopes/lifecycles/schemas, raw ops, native handles, raw CSS, executable callbacks, and client-side JavaScript. Workspace, document, user-config, and server-canonical persistence semantics must be declared explicitly and remain deferred unless a runtime-backed lifecycle is documented.

## Logic

Package logic is ordinary JavaScript running server-side through Clay's runtime and documented facades.

Good package logic:

- parse open document text slices supplied by Clay
- produce inert decoration spans
- register commands
- update package state through Clay APIs
- publish validated UI declarations
- handle server-routed commands

Rejected package logic:

- call raw `Deno.core.ops`
- read arbitrary files directly
- make network requests by default
- spawn shell commands
- mutate Masonry widgets
- run JavaScript in client paint/input handlers
- pass callbacks to Vello/Parley/native widgets

## Data and State

Package state should use explicit scopes. State keys and IDs should be package-prefixed when package-owned, and values must be bounded inert data rather than native handles, raw operation names, executable callbacks, or hidden globals.

Target state scopes:

| Scope | Example | Owner |
| --- | --- | --- |
| package-global | package defaults, feature flags | server/config |
| user-config | user layout/style preferences | `init.js` Clay APIs |
| workspace | workspace package settings | server/workspace when implemented |
| document | parse status, syntax cache metadata | server/document |
| pane | preview visible, active view, split ratio | shell/client+server depending on field |
| component | dropdown open, selected tab | shell/client transient unless persisted |
| transient-overlay | command palette/dropdown/modal open state | shell/client transient |

`serverRegisterUiStateScope` implements inert schema/lifecycle declarations for these scopes. It does not accept state values, arbitrary JSON blobs, hidden globals, or durable workspace/document mutation authority. Unsupported state scopes, ad hoc package keys, and package/user override bypass attempts are rejected before state affects the shell. Unsupported state scopes, hidden state keys, raw/default state values, and package/user override bypass attempts are rejected before state affects the shell. When a package needs new state, prefer a generic Clay primitive over a package-specific hidden global.

## Configuration

Configuration lives in `~/.config/clay/init.js` and is expressed through documented Clay JS APIs. Historical Phase 18.3 package UI configuration surfaces are declarations only; Phase 18.4 promotes two user/package customization surfaces:

- `clay.configuration.setPackageOption` records typed package options for `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback` with package-prefix validation and payload accounting.
- `clay.ui.serverSetLayoutOverride` records validated layout/theme/input/action overrides for `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback` with deterministic source precedence (`user-config`, active major mode, compatible minor mode, global package, package default).

**Implemented/runtime-backed configuration examples** (`clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride` are public runtime-backed Phase 18.4 APIs; historical **Planned configuration examples** in Phase 18.3 said `clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride` are inventory stubs, not public runtime-backed shell/layout configuration APIs in Phase 18.3). Before Phase 18.4, user-visible panel visibility/default-slot/theme-token override APIs remain planned inventory stubs; the Phase 18.4 status is now implemented through the documented APIs below.

```js
import { setPackageOption } from "clay:configuration";
import { serverSetLayoutOverride } from "clay:ui";

setPackageOption({
  packagePrefix: "markdown",
  option: "layout.defaultSlot",
  value: "right",
  source: "init-js",
});
setPackageOption({
  packagePrefix: "markdown",
  option: "layout.defaultVisibility",
  value: "hidden",
  source: "init-js",
});
serverSetLayoutOverride({
  targetId: "markdown.preview",
  property: "themeToken",
  value: { token: "markdown.heading.1", fallback: "text.primary" },
  source: "user-config",
});
```

Do not treat `preview.position`, `preview.defaultVisibility`, `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `theme.markdown.heading.1`, raw token override keys, or ad hoc style keys as hidden JSON/TOML/ad hoc keys. Use the documented Clay JS APIs above with type/default/allowed-value metadata and validation; unsupported option names, target IDs, tokens, raw values, or hidden keys remain rejected.

Configuration APIs must document:

- option name
- type
- default
- allowed values
- behavior-changing effects
- permissions/security notes
- generated registry metadata
- examples
- failure modes

Do not add hidden JSON/TOML/ad hoc keys for package options.

## Styling and Themes

Clay styling is centralized and token-based.

Packages should declare semantic tokens, not raw colors/CSS. Package-owned token names should use the package prefix, such as `markdown.heading.1`; Clay-owned token families such as `text.*`, `surface.*`, `border.*`, `accent.*`, `diagnostic.*`, `code.*`, and `selection.*` are reserved. Token declarations should include a semantic description, a token type, an optional same-type fallback, and package provenance.

**Implemented/runtime-backed theme-token declaration example** (`PackageThemeTokenDeclaration` / `clay.ui.serverRegisterThemeToken`):

```ts
import { serverRegisterThemeToken } from "clay:ui";

serverRegisterThemeToken(manifest, {
  token: "markdown.heading.1",
  type: "color-role",
  fallback: "text.primary",
  description: "Level 1 Markdown heading text",
});
serverRegisterThemeToken(manifest, {
  token: "markdown.preview.gap",
  type: "spacing",
  fallback: "spacing.panel",
  description: "Spacing between Markdown preview controls",
});
```

**Implemented/runtime-backed component style example** (typed style variables only):

```ts
serverRegisterComponentContribution(manifest, {
  kind: "panel",
  id: "markdown.preview.styleExample",
  title: "Preview",
  style: {
    variant: "muted",
    padding: "spacing.panel",
    background: "surface.panel",
    contentColor: "text.primary",
  },
  children: [],
});
```

Clay maps component style variables to typed native properties and render styles, such as:

- background
- content color
- border color
- border width
- padding
- corner radius
- font family/size where supported
- spacing
- selection/caret colors for editor components

Raw CSS is not supported as a package API. Unknown style tokens, duplicate package token names, type-incompatible fallbacks, native renderer callbacks, style strings, and raw colors without a typed token contract are rejected at package load, configuration, or UI update time.

### Phase 18.15 theme authoring: `textStyles` and `setTheme`

Implemented in Phase 18.15: editor text/chrome themes are first-party packages that declare inert `clay.contributions.textStyles` data and are activated from `~/.config/clay/init.js` with `clay.theme.setTheme()`. This is separate from `serverRegisterThemeToken`: SDUI theme tokens style package components; `textStyles` resolves editor/syntax/prose/base UI colors through `StyleRegistry`.

Authoritative vocabulary: [Text Vocabulary and Two-Axis Decoration Contract](../primitives/syntax-vocabulary.md).

A theme package declares no permissions and no modes. It contributes style data only:

```json
{
  "name": "@clay/theme-example-dark",
  "version": "0.1.0",
  "type": "module",
  "exports": { ".": "./dist/index.js", "./load": "./dist/load.js" },
  "clay": {
    "apiPrefix": "theme-example-dark",
    "entry": "./dist/index.js",
    "loadEntry": "./dist/load.js",
    "permissions": [],
    "modes": [],
    "docs": "./docs/index.md",
    "performance": { "estimatedManifestBytes": 2600 },
    "apiDependencies": [],
    "contributions": {
      "textStyles": [
        { "token": "panelBg", "color": "#282828" },
        { "token": "text", "color": "#d4be98" },
        { "token": "Keyword", "color": "#d3869b", "bold": true },
        { "token": "Comment", "color": "#7c6f64", "italic": true },
        { "token": "Link", "color": "#7daea3", "underline": true }
      ]
    }
  }
}
```

`textStyles` entry fields:

| Field | Status | Meaning |
| --- | --- | --- |
| `token` | required | One base UI key or one `TokenType` variant name. |
| `color` | optional | `#rgb`, `#rrggbb`, or `#rrggbbaa`. |
| `bold` | optional | Boolean default for token text. Syntax-token targets only. |
| `italic` | optional | Boolean default for token text. Syntax-token targets only. |
| `underline` | optional | Boolean default for token text. Syntax-token targets only. |
| `strike` | optional | Boolean strikethrough default. Syntax-token targets only. |

Base UI keys are: `shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`, `diagnosticWarning`, `diagnosticInfo`.

Token names are the `TokenType` variant names from the vocabulary contract: `Namespace`, `Type`, `Class`, `Enum`, `Interface`, `Struct`, `TypeParameter`, `Parameter`, `Variable`, `Property`, `EnumMember`, `Event`, `Function`, `Method`, `Macro`, `Keyword`, `Modifier`, `Comment`, `String`, `Number`, `Regexp`, `Operator`, `Decorator`, `Heading1`, `Heading2`, `Heading3`, `Heading4`, `Heading5`, `Heading6`, `ListItem`, `Quote`, `CodeBlock`, `CodeSpan`, `Link`, `Paragraph`.

Validation rules:

- Every entry must set at least one override field.
- Unknown `token` names are rejected.
- Duplicate token entries in one package are rejected with deterministic diagnostics.
- Invalid hex colors are rejected.
- `rawColor`, `value`, `css`, `rawCss`, `cssText`, executable callbacks, native widget handles, raw ops, client JavaScript, filesystem/network/shell authority, and renderer callbacks are rejected.
- `textStyles` is inert manifest data. It grants no permission and executes no styling code.

Runtime selection:

```js
import { setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
// or
setTheme({ specifier: "@clay/theme-gruvbox-material-light" });
```

Only one active theme is applied. `setTheme()` currently accepts first-party `@clay/*` theme packages; arbitrary local/registry theme specifiers are denied until package installation/authority is designed. Theme resolution happens during configuration/package-load and is sent to the client as an inert `ActiveTheme` snapshot before first paint. No theme JavaScript, package parser, or raw IPC runs in paint, layout, scroll, keypress, text-event, or edit-ack hot paths.

Full first-party examples live in `packages/theme-gruvbox-material-dark/` and `packages/theme-gruvbox-material-light/`.

#### Phase 20.6 canonical defaults vs opt-in themes

Phase 20.6 segregates the default themes into dedicated packages and pins the default-vs-opt-in loading contract:

- **Canonical defaults — `@clay/theme-modus-operandi` (light) and `@clay/theme-modus-vivendi` (dark).** These resolve **without any user `loadPackage` call**. When `init.js` runs and does not call `setTheme`, the runtime resolves the canonical default from the `appearance` preference (`light` → Modus Operandi, `dark` → Modus Vivendi, `system` → OS signal with a `dark` fallback). Resolution is a bundled-inventory lookup (`ensure_first_party_record`), so there is **no extra load cost** and no promotion-by-naming: a theme is a canonical default only because `canonical_default_specifier` names it, not because of its package prefix. The Modus packages are still regular first-party `@clay/theme-*` packages — identical manifest shape, inert `textStyles`, no permissions, no modes — and remain explicitly selectable by a one-line `setTheme("@clay/theme-modus-*")`.
- **Opt-in themes — Gruvbox Material Dark/Light.** These are never selected automatically. They require an explicit one-line `setTheme("@clay/theme-gruvbox-material-*")` in `init.js`. A silent `init.js` always resolves a Modus canonical default, never Gruvbox.
- **No silent behavior-changing package defaults beyond the pinned canonical pair.** A theme package cannot promote itself to a default by naming, manifest field, or load order. Only the two pinned Modus packages are canonical defaults; every other theme is opt-in via `setTheme`.
- **Trust classification preserved.** Canonical-default resolution reuses the same `ensure_first_party_record` path that validates bundled-inventory provenance, fingerprint, and first-party trust for explicit `setTheme` calls. Selecting a canonical default grants no authority that an explicit `setTheme` of the same package would not.

```js
// ~/.config/clay/init.js — canonical default, no loadPackage needed
// (appearance: system → dark → Modus Vivendi by default)

// Explicit one-line override of any bundled theme, no loadPackage needed:
import { setTheme } from "clay:theme";
setTheme("@clay/theme-modus-operandi");   // pin light canonical default
setTheme("@clay/theme-gruvbox-material-light"); // opt-in Gruvbox
```

See [Configuration: Phase 20.6 precedence and persistence](../clay-js-api/configuration.md#phase-206-themetypographyappearance-precedence-and-persistence) for the full source-order model (canonical/package default < `init.js` < UI session).

#### Phase 20.6 user override APIs and the settings UI

Users override theme, appearance, and typography through three surfaces, all bounded and authority-rejecting:

- **Programmatic — `clay:theme` facades in `~/.config/clay/init.js`.** `setTheme("@clay/theme-*")` selects the active theme; `setAppearance("light" | "dark" | "system")` sets the appearance preference that drives the canonical default (and is overridden by any explicit `setTheme`); `setTypography({...})` sets the monospace/proportional/ui font stacks, base sizes, and optional bounded hierarchy. These are init.js APIs (source `init-js` in the precedence model).
- **UI session — `@clay/settings` panel.** A first-party catalog-composed SDUI panel (`packages/settings/`) lets users switch theme, appearance, and typography from the UI. Controls emit inert `settings.*` command intents (`settings.setTheme`, `settings.setAppearance`, `settings.setTypography`, `settings.reset`) that the server validates, persists to `~/.config/clay/preferences.json` (source `ui-session`), and applies live through a runtime reload (persist → reload → `init.js` re-eval + preferences apply → `RuntimeStateSnapshot` fanout). No restart required. The panel uses only implemented `ComponentKind` kinds (`panel`, `collapse`, `dropdown`, `textInput`, `label`, `button`, `flex`) — no native chrome, no client JavaScript, no raw CSS.
- **Persistence — `preferences.json`.** The closed `ui-session` store (theme, appearance, typography) overrides `init.js` on every reload, so a UI choice survives restart and beats the equivalent `init.js` call. See the package doc at `packages/settings/docs/index.md` for the catalog-composition table and command flow.

Theme packages themselves declare **inert style data only** (`clay.contributions.textStyles` and optional `clay.contributions.designTokens`); they declare no permissions, no modes, and no override APIs. All override authority is user-owned through the three surfaces above. A theme package cannot promote itself, ship executable styling code, raw CSS, client JavaScript, or a third-party theme loader; `setTheme`/canonical-default resolution accepts only bundled first-party `@clay/*` specifiers.

### Phase 20.1 authoring contract: typed token catalog, typography hierarchy, and token-backed defaults

Phase 20.1 expanded the typed token catalog additively from five domains to ten. The full implemented catalog lives in the clay-ui skill reference (`.agents/skills/clay-ui/references/tokens.md`); this section records the package authoring contract.

**Typed domains** (`ThemeTokenType`, ten): `color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, `density`. Every package token `type` must be one of these, and every `fallback` must be a same-typed Clay core token. The original five domains are unchanged; `dimension`, `elevation`, `motion-duration`, `z-level`, and `density` are additive.

**Typography hierarchy**: the seven `UiTextVariant` tokens (`typography.body`, `typography.title`, `typography.status`, `typography.display`, `typography.section`, `typography.detail`, `typography.caption`) are semantic variant selectors, not absolute sizes. Their scale ratios form `UiTypographyHierarchy`, which is user-owned via [`setTypography`](../clay-js-api/theme/set-typography.md) and travels atomically with `ActiveTypography`. Packages select a variant name only; they **cannot** supply concrete scale ratios. A `clay.contributions.designTokens` entry targeting any `typography.*` token is rejected as a typography (variant) override, not a scale value.

**Typed UI design-token overrides** (`clay.contributions.designTokens` / `UiDesignTokenOverride`): a theme package may ship typed UI overrides that resolve client-side into `ResolvedUiTheme`. Each override carries a core Clay token name, a typed value variant (`Color`, `Scalar`, `Opacity`, or `Level`), and provenance. Validation rejects unknown tokens, value/type mismatch against the core token, raw CSS/color/size fields, duplicates, out-of-range scalars (dimension ordering, opacity `[0,1]`, `motion-duration` `[0,1000]`), invalid level names, and any `typography.*` override. Existing Gruvbox themes ship no `designTokens` and resolve through core fallbacks unchanged — no package migration is required.

**Token-backed panel/sidebar/density defaults**: `dimension.sidebar.default`, `dimension.panel.side.*`, `dimension.panel.vertical.*`, and `density.default` replace the prior hardcoded panel constants. Packages do not set these directly; theme/configuration overrides flow through `designTokens` or future documented configuration APIs. Invalid dimension ordering (`min > default` or `max < default`) falls back to the matching Clay constant tuple per domain before layout. `density.default` selects compact/default/spacious; it scales the token-owned UI spacing rhythm only and never scales panel dimensions or document typography. Phase 20.3 implemented resize/collapse persistence and split interaction on top of these defaults.

**Resolution and hot paths**: token resolution happens at theme/configuration install time. `ActiveTheme.design_tokens` is validated server-side and resolved client-side into one cached `ResolvedUiTheme`. Native paint, layout, pointer, scroll, keypress, and text-event paths read cached resolved values only — no package JavaScript, theme parsing, raw IPC, or re-resolution runs per frame.

```ts
import { serverRegisterThemeToken } from "clay:ui";

// Semantic package tokens resolve to same-typed Clay core fallbacks.
serverRegisterThemeToken(manifest, {
  token: "example.panel.elevation",
  type: "elevation",
  fallback: "elevation.raised",
  description: "Raised panel surface for the example panel",
});
serverRegisterThemeToken(manifest, {
  token: "example.overlay.z",
  type: "z-level",
  fallback: "z.overlay",
  description: "Stacking level for the example overlay",
});
```

### Phase 20.3 authoring contract: layout primitives, split interaction, and layout intents

Phase 20.3 implements user-facing layout primitives: draggable split dividers, fixed slot resize handles with collapse/restore, layout persistence, focus/input routing across splits, and an inert versioned layout intent API for packages.

**Split dividers and resize handles** (user-facing, not package-facing): users drag split dividers on `PaneSplitTree` to adjust pane ratios (clamped 0.05–0.95) and drag fixed slot resize handles to adjust panel sizes (clamped to token-backed min/max). Double-clicking a slot resize handle toggles collapse/restore. All interaction is client-side; no package JavaScript runs during drag, paint, or layout.

**Layout persistence**: user-modified split ratios (≠ 0.5) and slot sizes (resized or collapsed) persist to `~/.config/clay/layout.json` with ≥500ms debounce. Corrupt or missing files fall back to defaults. Packages cannot read or write this file.

**Focus and input routing**: Tab/Shift+Tab moves focus across panes in reading order. The active pane is tracked in `PaneSplitTree.active_pane_id`. A focus ring paints on the active pane when multiple panes exist. Package `PackageInputRouting` declarations scoped to a pane only receive events when that pane is focused. Transient surfaces (overlays, menus, completion pop-ups) anchor to the focused pane's geometry via `WorkingAreaLayout::focused_pane_rect()`, not the full working area.

**Layout intent API** (package-facing): packages request pane splits through `serverRequestLayoutIntent` from `clay:ui`. The intent is inert and versioned; Clay validates and stores it, then composes it into `WorkingAreaLayoutUpdate` at Clay's discretion. Packages cannot mutate native layout directly.

```ts
import { serverRequestLayoutIntent } from "clay:ui";

// Request a horizontal split of the active pane.
serverRequestLayoutIntent({
  id: "markdown.splitPreview",
  targetPane: "active",
  orientation: "horizontal",
  ratio: 0.5,
  position: "second",
});
```

Validation rejects: missing/wrong package prefix on `id`, duplicate `id`, invalid `orientation` (must be `horizontal` or `vertical`), `ratio` outside 0.05–0.95, invalid `position` (must be `first` or `second`; defaults to `second`), and payloads exceeding the SDUI budget.

**Package limitations**: packages cannot mutate `WorkingAreaLayout`, `PaneSplitTree`, or `PaneSlotLayout` directly. They cannot access Masonry widget IDs, raw callbacks, renderer state, or the layout persistence file. Layout authority is Clay-owned; packages participate through inert validated declarations only.

### Phase 20.4 authoring contract: core component uplift on the existing catalog

Phase 20.4 restyles every implemented `ComponentKind` to the minimalist design language using the Phase 20.1 tokens and Phase 20.2 primitives, **without changing component kinds, style-variable schemas, or token names**. It is a restyle, not a catalog expansion: no new kind, no new style variable, no new token was added.

**Active-theme routing**: SDUI component paint reads the active `ResolvedUiTheme` (the design-token registry layered over the core fallback catalog by the active theme/configuration), not core fallbacks. The prior `SduiThemeStyle::default()` core-fallback paint path is gone; `SduiThemeStyle::from_ui_theme(&ResolvedUiTheme)` resolves typed values from the active theme at each `&self` paint entry point. Theme packages (e.g. `@clay/theme-gruvbox-material-dark`) that contribute `clay.contributions.designTokens` overrides now flow through to SDUI component paint automatically — no manifest change required.

**State-complete components**: every interactive component derives all five `InteractionState` variants from state tokens:

| Kind | Rest fill | Hover | Active | Focus | Disabled |
|------|-----------|-------|--------|-------|----------|
| `button` | `surface.control` | `surface.hover` | `surface.active` | `accent.primary` + `paint_focus_ring` (`border.focus`) | `surface.disabled` × `opacity.disabled`, text `text.disabled` × `opacity.disabled`, action gated |
| `list` row | `surface.list` (unselected) / `surface.selected` (selected) | `surface.hover` | `surface.active` | `surface.selected` | `surface.disabled` × `opacity.disabled`, action gated |
| `label` / `statusItem` | text `text.muted` | — | — | focus ring | text `text.disabled` × `opacity.disabled` |
| `panel` / `overlay` | chrome via `paint_panel_chrome` / `paint_tooltip_shell` (state-independent chrome; collapse/resize affordances route through the primitive) | | | | |
| `editorView` | editor `StyleRegistry`-driven chrome (caret/selection/scrollbar/diagnostics); scrollbar reflects `Hover`/`Active` from pointer state (task 5) | | | | |
| `flex` / `stack` / `scroll` / `portal` | container — recurse children, no chrome of their own | | | | |

`InteractionState` is derived from client-local pointer/focus hit-testing (`SduiNativeState::interaction_state`): `Disabled` is checked first (gates actions out of the action region set), then `Active` (pointer pressed over the rect), `Hover` (pointer over the rect), `Focus` (click-to-focus; no Tab traversal yet), else `Rest`. The `component_state_color`/`list_row_fill_color`/`disabled_text_color` helpers in `src/shell/primitives.rs` centralize the token→state mapping.

**Spacing rhythm**: SDUI panel padding reads `spacing.md` (16, the 4pt-grid default content padding) scaled by `spacing_scale()` (density `compact`=0.875 / `default`=1.0 / `spacious`=1.125). The status bar uses token-driven insets (`spacing.sm` × `spacing_scale()`) with a `border.hairline` divider. Per-element `spacing.xs`/`sm`/`lg` differentiation is deferred to a later spacing pass; `panel_padding` is the single rhythm entry point consumed by SDUI geometry.

**Editor chrome stays on the editor theme**: caret, selection, diagnostics, and the status bar background/text read from the editor `StyleRegistry` (`BaseUiColors`: `caret`, `selection`, `diagnostic*`, `statusBg`, `statusText`), which remains the single source of color for editor paint and is separate from SDUI typed tokens. The editor scrollbar routes through `paint_scroll_chrome` with `InteractionState` derived from pointer state (`EditorSurface::scrollbar_interaction_state`). No new `BaseUiColorKey` was added.

**Compatibility guarantee**: Phase 20.4 changes no `ComponentKind` (`src/shell/components.rs`), no typed style variable, no token name, and no package manifest field. Existing first-party packages (`@clay/markdown`, `@clay/git`, `@clay/rust`, `@clay/typescript`, `@clay/javascript`, the `lsp-*` bridges, `@clay/theme-gruvbox-material-*`) pass their test suite unmodified. `PackageUiComponentTree` and `PackageUiListItem` gained a `disabled` field (parsed from JSON, defaults `false`) so packages can declare disabled state, but existing declarations that omit it are unchanged.

**Structural observability**: a test-local `component_state_palette` helper captures the resolved fill/border/text colors per component kind per `InteractionState` from the active `ResolvedUiTheme` — no pixel rendering, no GPU. This is test infrastructure only; no public `clay:sdui.queryUiState` introspection API is introduced (deferred per Phase 15/16 until a real agent-introspection need exists).

**Package limitations (restated)**: packages declare inert `ComponentKind` components and typed tokens only. No raw CSS, no raw colors, no client JavaScript in paint/layout/pointer/scroll/keypress/text-event handlers, no native widget handles, no raw ops, no direct primitive calls. Packages cannot read or drive `InteractionState` directly; Clay derives it from pointer/focus hit-testing and renders state chrome through primitives.

### Phase 20.7 authoring contract: UI conformance guardrails

**Status: Implemented/internal runtime.** Phase 20.7 hardens the host-authority validation boundary for package UI. Every check below runs at package parse/install or theme-apply time inside Clay's Rust host validator — none is exposed to packages as a callable API. Conformance is host authority, not package-facing: a third-party package cannot bypass validation, and no `clay.ui.validate*` op or `clay:*` facade exists for conformance (asserted by `tests/package_ui_conformance.rs`).

**Enforced checks:**

| Check | Status | What is rejected | Where enforced |
|------|--------|------------------|----------------|
| Typed-token-only styling | Implemented | Raw colors, raw CSS, style strings, renderer callbacks, unknown/mistyped style-variable tokens | `src/shell/components.rs` (`validate_style_variables`/`validate_style_variable`/`reject_raw_style_token`); mapped to `UiContributionRule::ProhibitedAuthority` (raw CSS/color) or `InvalidThemeToken` (mistyped token) at the runtime boundary |
| Reserved-kind gating | Implemented | Declaring a reserved `ComponentKind` (currently `table`) | `src/shell/components.rs` (`validate_component_kind`); `InvalidContributionDescriptor` |
| Contrast / legibility | Implemented | Active theme whose status-chrome pairs fall below `TEXT_CONTRAST_MIN` (4.5) or `UI_CONTRAST_MIN` (3.0) | `src/shell/theme.rs` (`validate_active_theme_contrast`) + `src/server/ops/theme.rs` (`enforce_contrast`); a below-AA theme is not activated and records a `clay.theme.contrast` diagnostic |
| State-completeness | Implemented | Catalog drift between the `applicable_states(kind)` table and the documented per-kind interaction notes | `src/shell/components.rs` (`applicable_states`); pinned by `tests/masonry_sdui.rs` against `component_state_palette` |
| Payload budgets | Implemented | SDUI snapshot estimate > 4096 B, update estimate > 1024 B; runtime tree > 16 KiB / > 128 nodes / > 16 depth / > 4096-char text node | `src/packages/record.rs` + `src/server/ui.rs` + `src/server/ops/sdui.rs`; `PayloadBudgetExceeded` |
| Raw-color / raw-size rejection | Implemented | Raw color values in component `style.*` variables; raw colors / raw CSS in `designTokens` overrides | `src/shell/components.rs` + `src/packages/record.rs` |
| Code-vs-catalog drift lint | Implemented | `ComponentKind` enum, `component_state_palette` match arms, typed-style-variable match arms, or `core_theme_value` match arms drifting from `components.md`/`tokens.md` | `tests/package_ui_conformance.rs` (four drift guards) |
| Author diagnostics | Implemented | Rejection messages that omit the rejected value, expected type, or field | `ComponentCatalogError::reject` (`src/shell/components.rs`) + `; got {actual}` appends (`src/packages/record.rs`) |
| Trust-domain boundaries | Implemented | Third-party raw values / oversized payloads reaching the trusted runtime; conformance exposed as a package-facing op/facade | `tests/package_ui_conformance.rs` (three trust-domain tests) |

**Diagnostic message format.** Component-catalog rejections use a single stable shape via `ComponentCatalogError::reject`:

```text
{field} = `{value}` rejected: expected {expected}; {reason}
```

Design-token `value` rejections append `; got {actual}` to the existing typed-shape message, naming the rejected value kind (e.g. `got number 12`, `got string "#zz"`). Example rejections a package author will see:

```text
style.background = `#ff00aa` rejected: expected color-role token; raw colors or raw CSS are not allowed; reference a Clay token (e.g. surface.main)
color-role design token `value` must be a #rgb, #rrggbb, or #rrggbbaa hex string; got number 12
```

The rejected value is sanitized (trimmed, backticks stripped, bounded to 80 characters) so an author-supplied string cannot break the diagnostic shape or inject markdown. Conformance helpers that run at theme-apply time are `pub(crate)`; `validate_active_theme_contrast` and `ContrastFailure` are re-exported from `clay::editor::theme` for integration tests only and are not wired to any `deno_core` op or JS facade (no package-facing trust path).

**Example rejection (raw color in a component style):**

```json
{
  "kind": "panel",
  "id": "vendor.preview",
  "style": { "background": "#ff00aa" }
}
```

This is rejected at `assemble_package_record` with `InvalidContributionDescriptor`, `contribution_id = "style.background"`, and the message above. No `PackageRecord` is produced, so no contribution descriptor is installed — the raw value never reaches the trusted runtime.

**Authority boundaries (security):**
- No raw CSS, no client JavaScript, no native widget handles, no raw `Deno.core.ops` in package UI contributions.
- No third-party theme loader: themes are first-party packages activated from `~/.config/clay/init.js`; a theme package below the AA contrast floor is not activated.
- Conformance is host authority, not package-facing: validation runs inside Clay's Rust host validator at parse/install/theme-apply time; no `clay.ui.validate*` op or `clay:*` facade exposes it.
- Trusted classification comes from the compiled bundled inventory and provenance/integrity, never package naming or `@clay/*` prefix.

See `.agents/skills/clay-ui/references/components.md` (conformance contract) and `.agents/skills/clay-ui/references/tokens.md` (rules) for the enforced-check lists. The conformance suite lives at `tests/package_ui_conformance.rs` and `tests/ui_primitive_conformance.rs`.

## Rendering and Decorations

Inline editor rendering uses inert decoration data, not UI components.

Decoration span shape (implemented protocol model):

```js
{
  byteStart: 0,
  byteEnd: 7,
  kind: "syntax",
  tokenType: "Heading1",
  modifiers: ["Bold"],
  scope: "markup.heading.1",
  priority: 80
}
```

Current compatibility facades may still accept legacy `styleToken` strings at package/parse boundaries and map them through `DecorationSpan::from_style_token`; new package code should target `TokenType` + `Modifiers` from the vocabulary contract. Packages should translate parser-specific output into generic Clay decoration spans. Rust should not branch on Markdown-specific token names.

### Phase 18.17 range diagnostics publication

Range diagnostics are a separate primitive from decorations. Use [`clay.diagnostics.serverPublishDiagnostics`](../clay-js-api/diagnostics/server-publish-diagnostics.md) for severity/code/message/source metadata; do not stuff diagnostic messages into `serverPublishDecorations`. Canonical contract: [Range Diagnostics](../primitives/diagnostics.md).

```ts
import { serverPublishDiagnostics } from "clay:diagnostics";

serverPublishDiagnostics({
  packageName: "@clay/example",
  packageVersion: "0.1.0",
  packagePrefix: "example",
  permissions: ["render-decorations"],
  documentId,
  documentVersion,
  viewport: { byteStart, byteEnd },
  source: "example-analyzer",
  spans: [{
    byteStart,
    byteEnd,
    severity: "error",
    code: "example.syntax-error",
    message: "Syntax error",
  }],
});
```

Rules:

- Requires `render-decorations`. Empty `spans` clears only that `source` chunk.
- Payload stays within `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES` and `DIAGNOSTIC_MAX_SPANS_PER_SET`.
- Themes color squiggles through `diagnosticError` / `diagnosticWarning` / `diagnosticInfo` via `setTheme`; no diagnostic enable/geometry config API exists.
- Rejected: executable handlers/callbacks, client JavaScript, raw ops, CSS/draw callbacks, native handles, language-server process spawning, filesystem/network/shell/AI authority.
- Future LSP packages map onto this same inert contract; Phase 18.17 does not add LSP process APIs.

## Phase 18.20 authoring contract: analyzer providers and language-server bridges

Canonical contract: [Language Intelligence and LSP 3.17 Bridge Contract](../primitives/language-intelligence.md). Analyzer packages and future `@clay/lsp-*` bridges share one Clay primitive surface.

### Analyzer providers (no process)

Register a feature-tagged provider that receives only Clay-provided open-document data:

```ts
import { serverRegisterLanguageIntelligenceProvider } from "clay:language";

serverRegisterLanguageIntelligenceProvider({
  packageName: "@clay/example",
  packageVersion: "0.1.0",
  packagePrefix: "example",
  permissions: ["parse-document"],
  id: "example.intelligence",
  modes: ["example"],
  features: ["hover", "definition", "codeAction", "signatureHelp"],
  priority: 10,
  timeoutMs: 500,
  exportName: "provideLanguageIntelligence",
});
```

Rules:

- Requires `parse-document`. Publishing semantic decorations or diagnostics still needs `render-decorations`; completion needs `completion-provider`.
- Return UTF-8 byte offsets / ranges only. No LSP `Position`, URI, JSON-RPC ID, or method name crosses the Clay boundary.
- Hover/signature markdown is inert bounded text. Definition locations use an open document or known workspace root + normalized relative path.
- Code-action `EditPreview` values are inert versioned previews; command-backed actions execute later through `CommandExecution`.
- Empty/timeout/error outcomes use typed statuses. Work is cancellable UI-reactive and must not run before local paint.

### Language-server packages (grant then load)

Process-backed bridges declare `language-server`, fixed contribution metadata, and an explicit pre-load grant. See [Language-server packages require grant then load](#language-server-packages-require-grant-then-load) for the manifest and `authorizeLanguageServer` example.

Additional bridge rules:

- Grant-before-load is mandatory; `loadPackage` alone neither authorizes nor launches a server.
- Contribution executable/argv/inherited-environment names are fixed and validated; runtime code cannot choose shell strings, cwd, or arbitrary environment values.
- Sessions expose only bounded UTF-8 `send`/`read`/`stop` under `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`, `LANGUAGE_SERVER_MAX_SESSIONS`, `LANGUAGE_SERVER_STDERR_BUDGET_BYTES`, and read timeouts; diagnostics are sanitized.
- LSP `Content-Length` framing, initialize/capabilities, document sync, cancellation, and position-encoding conversion are Phase 18.21 package adapters layered on the opaque session.
- Map LSP SemanticTokens, Diagnostic, Completion, Hover, Definition/DefinitionLink, CodeAction/Command/WorkspaceEdit, and SignatureHelp onto the Clay primitives documented in `language-intelligence.md`.
- External/out-of-root URIs are denied. Treat the child as trusted subprocess authority, not a sandbox: cwd/root identity does not OS-confine filesystem, network, or process access.

## Phase 18.21 authoring contract: LSP bridge packages

Phase 18.21 ships the full bridge authoring contract for `@clay/lsp-*` packages. Bridges register a long-lived document-analysis worker that speaks LSP 3.17 to a child process and converts responses to Clay decorations, diagnostics, completions, and intelligence results.

### Bridge package structure

Every LSP bridge package follows this layout:

```
packages/lsp-rust/
  package.json          # manifest with apiPrefix, capabilities, contributions, permissions
  dist/
    index.js            # exports packageManifest() with all constants
    load.js             # calls serverRegisterDocumentAnalyzer during loadEntry
    server.js           # creates the bridge factory + default handleDocumentAnalysis export
    shared/             # generated copy of packages/lsp-shared/
      framing.js
      positions.js
      mapping.js
      client.js
      utf8.js
  docs/
    index.md            # setup, authorization, behavior, security, troubleshooting
```

### Manifest requirements

```json
{
  "clay": {
    "apiPrefix": "lsp-rust",
    "capabilities": ["language-server"],
    "permissions": ["parse-document", "completion-provider", "render-decorations"],
    "contributions": {
      "languageServers": [{
        "id": "lsp-rust.server",
        "executable": "rustup",
        "args": ["run", "stable", "rust-analyzer"],
        "inheritEnvironment": []
      }],
      "completionProviders": [{
        "id": "lsp-rust.completion",
        "triggerCharacters": [":", ".", "'", "("],
        "priority": 100,
        "exclusive": false
      }],
      "languageIntelligenceProviders": [{
        "id": "lsp-rust.intelligence",
        "modes": ["rust"],
        "features": ["hover", "definition", "codeAction", "signatureHelp"],
        "priority": 100
      }]
    }
  }
}
```

Rules:

- `language-server` in `capabilities` array (not `permissions`). It is a prohibited authority that cannot be requested by default.
- `parse-document` and `language-server` permissions are both required for analyzer registration.
- `completion-provider` and `render-decorations` are needed for publishing completions, decorations, and diagnostics.
- Contribution IDs must use the package `apiPrefix` namespace (e.g. `lsp-rust.server`, `lsp-rust.completion`).
- `executable` and `args` are fixed and validated at install time. No shell strings, no user-tunable paths.
- `inheritEnvironment` lists explicit environment variable names inherited after `env_clear()`. An empty array gives the child an empty environment.

### load.js: document analyzer registration

```js
// packages/lsp-rust/dist/load.js
import { serverRegisterDocumentAnalyzer } from "clay:language";
import "./server.js"; // ensure module resolution
import { lspRustPackageManifest } from "./index.js";

serverRegisterDocumentAnalyzer({
  packageManifest: lspRustPackageManifest(),
  analyzer: {
    id: "lsp-rust.bridge",
    contribution: "lsp-rust.server",
    modes: ["rust"],
    moduleSpecifier: "clay://packages/@clay/lsp-rust/dist/server.js",
    exportName: "handleDocumentAnalysis",
  },
});
```

Rules:

- Registration is **package-load-time only**. It is inert — no worker or process starts until a document opens.
- An exact current `authorizeLanguageServer` grant must exist before registration. The load order for users is always: `authorizeLanguageServer` → `loadPackage` (base) → `loadPackage` (bridge).
- `analyzer.id` must use the package `apiPrefix` namespace (e.g. `lsp-rust.bridge`).
- `analyzer.contribution` must match a contribution declared in `clay.contributions.languageServers`.
- Rejected authority fields: `handler`, `callback`, `function`, `executable`, `args`, `cwd`, `environment`, `process`, `rawOps`.

### server.js: bridge factory

```js
// packages/lsp-rust/dist/server.js
import { startLanguageServerSession } from "clay:language-server";
import { serverPublishDecorations } from "clay:decorations";
import { serverPublishDiagnostics } from "clay:diagnostics";
import { LspClient } from "./shared/client.js";
import { VersionedDocument } from "./shared/positions.js";
import { semanticTokensToClay, diagnosticsToClay, completionToClay,
         hoverToClay, definitionToClay, codeActionsToClay,
         signatureHelpToClay } from "./shared/mapping.js";

export function createBridge(config) {
  let client = null;
  const documents = new Map(); // documentId → { uri, version, text, versionedDoc }

  async function ensureClient() {
    if (client) return client;
    const session = await startLanguageServerSession({
      package: config.packageName,
      contribution: config.contribution,
      workspaceRootId: config.workspaceRootId,
    });
    client = new LspClient(session.sendBytes.bind(session),
                           session.readBytes.bind(session));
    await client.initialize(config.capabilities, config.workspaceRootUri);
    return client;
  }

  async function handle(event) {
    // Verify identity on every event.
    if (event.package !== config.packageName ||
        event.analyzerId !== config.analyzerId) return;

    const cl = await ensureClient();
    switch (event.kind) {
      case "open":   await handleOpen(cl, event); break;
      case "change": await handleChange(cl, event); break;
      case "close":  await handleClose(cl, event); break;
    }
  }

  return { handle };
}

// ... handleOpen, handleChange, handleClose, refreshSemantic,
//     refreshDiagnostics, handleCompletion, handleIntelligence ...
```

Rules:

- Use `session.sendBytes(bytes: Uint8Array)` and `session.readBytes(maxBytes, timeoutMs)` — not the text-based `send`/`read` compatibility wrappers. Lossless byte transport avoids UTF-8 corruption on split multibyte sequences.
- Own all LSP framing (Content-Length), JSON-RPC envelope, capabilities, position encoding, URI conversion, and cancellation. Rust core stays LSP-wire neutral.
- Map LSP responses through `packages/lsp-shared/mapping.js` bounded converters. Never pass raw LSP data into Clay publication channels.
- Validate publication targets against the active document version before publishing decorations or diagnostics.
- External/out-of-root URIs are denied at the bridge level.

### Shared LSP adapter (packages/lsp-shared/)

All four first-party bridge packages share a canonical LSP 3.17 adapter at `packages/lsp-shared/`:

| Module | Purpose | Key limits |
|---|---|---|
| `framing.js` | Content-Length encode/decode, `FrameDecoder` | 1 MiB frame, 8 KiB header |
| `positions.js` | `VersionedDocument` with UTF-8/16/32 byte-to-position | CRLF normalization, surrogate rejection |
| `mapping.js` | LSP → Clay vocabulary converters | 128 tokens, 128 diagnostics, 256 completions, 64 definitions, 64 code actions, 16 signatures, 32 parameters, 4 KiB markdown, 8 KiB decoration payload, 8 KiB diagnostic payload, 16 KiB result payload |
| `client.js` | `LspClient` lifecycle (initialize, sync, request, notification) | Server request allowlist, cancellation |
| `utf8.js` | Pure-JS UTF-8 codec | No TextEncoder/TextDecoder dependency (Clay runtime lacks deno_web) |

Copies are distributed to each package via `scripts/update-first-party-lsp-shared.mjs` and verified by `tests/lsp_bridge.rs`. Package code imports from `./shared/` — a deterministic copy of the canonical source, not a symlink or dynamic resolution.

Never:
- Reference `TextEncoder` or `TextDecoder` (not available in Clay's deno_core runtime).
- Import shared modules across package boundaries at runtime.
- Duplicate adapter logic; changes flow through the canonical source and the update script.

### Worker lifecycle and budgets

- **Spawn**: lazy, on first eligible document open matching the analyzer's modes.
- **Stop**: after last monitored document closes. 2s graceful shutdown via `shutdown` event, then 5s kill.
- **Global cap**: `DOCUMENT_ANALYSIS_MAX_WORKERS` (4). Additional eligible packages queue or fall back to base behavior.
- **Per-worker**: max 32 documents, 8 MiB text, 64 MiB heap, 8 pending child requests.
- **Input mailbox**: 64 deltas / 2 MiB with `coalesce_reset` deduplication.
- **Output queue**: 64 events / 512 KiB. Saturation clears bridge outputs and retains Tree-sitter/base completion.
- **Edit ack and local paint**: never wait on worker, JS, or subprocess.

### Completion and intelligence routing

- LSP completion providers register at **priority 100 non-exclusive**, merging with base keyword providers (priority 0).
- Completion results exceeding `RESULT_PAYLOAD_BUDGET_BYTES` (16 KiB) should use halving-retry truncation: reduce the item list by half, re-encode, check budget, repeat.
- Intelligence requests (hover, definition, code action, signature help) route through `LanguageIntelligenceCoordinator` with cancellable UI-reactive priority.
- `document_changed` is called on every `EditAck` to abort stale in-flight work for both completion and intelligence coordinators.

### Per-server capability differences

| Capability | rust-analyzer | typescript-language-server | marksman |
|---|---|---|---|
| Text document sync | Incremental | Incremental | Full (change: 1) |
| Position encoding | UTF-8 | UTF-16 | UTF-16 |
| Semantic tokens | Full + delta | Full only | Full only (often empty data) |
| Diagnostics | Pull (`textDocument/diagnostic`) | Push (`textDocument/publishDiagnostics`) | Push + `.marksman.toml` marker required |
| Completion | Snippet format | Snippet format | textEdit format |
| Signature help | Yes | Yes | Not advertised |
| Code actions | Bare boolean true | resolveProvider: true | Mutating TOC only (filtered by bridge) |
| Special notes | Uses `rustup run stable rust-analyzer` | Needs `workspace/configuration` handler; `$/typescriptVersion` notification | Needs `.marksman.toml` project marker for hover/definition/completion |

Bridge packages must handle each server's actual behavior with graceful degradation:
- Servers that return empty semantic token data → publish no semantic decorations.
- Servers that return null hover/definition/completion → no Clay intelligence result.
- Mutating code actions (items with `.edit` field) → filtered by `codeActionsToClay`.
- Signature help when not advertised → return empty result.

### Anti-patterns

Never:
- Reference `Content-Length`, `jsonrpc`, `textDocument/*`, or `$/cancelRequest` in Rust core.
- Call `Deno.core.ops` directly from bridge code — use only `clay:*` facades.
- Create per-open runtimes or per-document child processes.
- Hardcode server paths, environment variables, or workspace root URIs in package code.
- Synchronously expect diagnostics after `didOpen` — poll via document change triggers for pull-diagnostic servers.
- Pass raw LSP positions, URIs, or JSON-RPC messages into Clay publication channels.
- Bypass the shared `packages/lsp-shared/` adapter with custom framing or mapping.
- Use `TextEncoder`/`TextDecoder` — Clay's deno_core runtime lacks the `deno_web` crate.
- Publish decorations or diagnostics without validating against the active document version.
- Block the event handler on long-running LSP requests during shutdown.

### Test patterns

Bridge packages use three test layers:

1. **Fake session tests** (always run): import `FakeLspSession` from `tests/fixtures/lsp/fake-server/session.mjs`. Configure with a profile matching the target server capabilities. Test manifest matching, feature mapping, forged identity rejection, and absent capability behavior.

```js
import { FakeLspSession } from "../../tests/fixtures/lsp/fake-server/session.mjs";
import { createBridge } from "../dist/server.js";

const session = new FakeLspSession("rust");
const bridge = createBridge({
  packageName: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  startSession: () => session,
  // ...
});
```

2. **Real smoke tests** (`CLAY_LSP_REAL_SMOKE=1`): spawn the actual language server as a child process. Gated behind environment variable so ordinary `cargo test` stays green without host tools.

3. **Rust integration tests**: verify package manifests, grant-before-load ordering, fixture-based loading, and adapter copy freshness via `tests/lsp_bridge.rs`.

## Phase 18.5 authoring contract: no-default-panel, optional preview, generic primitive consumption

Phase 18.5 replanned Markdown end-user loading around the generic shell/package primitives promoted in Phases 18.1–18.4. The key authoring contract changes are:

1. **No default fixed panel.** Packages do not publish a default fixed panel on load. A package that offers a preview, status, or auxiliary panel registers it as a `PanelContribution` with `defaultVisibility: "hidden"`. The panel appears only when the user explicitly enables it through `setPackageOption`, `serverSetLayoutOverride`, or a package command.

2. **Main editor placement via `PaneSlotLayout.main`.** The editor always occupies the mandatory `main` slot. Packages do not need to request or register this; Clay places the active editor in `main` by default. Package-owned panels target optional fixed slots (`left`, `right`, `top`, `bottom`).

3. **Optional preview as a `PanelContribution`.** A package preview panel is a `PanelContribution` targeting a Clay slot (e.g., `right`) with `defaultVisibility: "hidden"`. The package registers the contribution at load time; the shell validates it, composes it into `PaneSlotLayout`, and keeps it hidden until a user or package action changes visibility.

4. **Theme token usage for panel styling.** Preview and panel styles use `PackageThemeTokenDeclaration` with same-type core fallbacks (e.g., `markdown.preview.background` → `surface.panel`). Raw CSS, raw colors, and renderer callbacks remain prohibited.

5. **`setPackageOption` and `serverSetLayoutOverride` for customization.** User configuration changes preview visibility, slot, split ratio, or theme token mapping through documented Clay JS APIs, not through hidden JSON/TOML/ad hoc keys.

6. **Package-owned fallback alias retained after `loadPackage` shipped.** The generic `loadPackage("@clay/markdown")` one-line resolver is implemented in Phase 18.6 (see `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` for the authority rationale). The package-owned `markdownLoadMode()` entry consumes implemented generic primitives internally and remains a documented convenience alias for per-load options.

**Implemented/runtime-backed Phase 18.5 no-default-panel example** (Markdown as a consumer of generic primitives):

```ts
import { serverRegisterPanelContribution } from "clay:ui";
import { serverRegisterThemeToken } from "clay:ui";
import { setPackageOption } from "clay:configuration";
import { serverRegisterCommand } from "clay:commands";

// Register the toggle command first (actions target registered commands).
serverRegisterCommand({
  id: "markdown.togglePreview",
  label: "Toggle Markdown Preview",
  routing: "ServerFirst",
});

// Declare theme tokens for preview panel styling.
serverRegisterThemeToken(manifest, {
  token: "markdown.preview.background",
  type: "color-role",
  fallback: "surface.panel",
  description: "Markdown preview panel background",
});

// Register the optional preview panel. Hidden by default.
// The editor is always in PaneSlotLayout.main; this panel targets the right slot.
serverRegisterPanelContribution(manifest, {
  id: "markdown.preview",
  slot: "right",
  kind: "fixed",
  defaultVisibility: "hidden",
  actionTargets: ["markdown.togglePreview"],
  component: {
    kind: "panel",
    id: "markdown.preview.root",
    title: "Preview",
    style: { background: "markdown.preview.background", padding: "spacing.panel" },
    children: [
      { kind: "label", id: "markdown.preview.empty", text: "Preview unavailable" },
    ],
  },
});
```

**User enables preview from `init.js`** (implemented/runtime-backed configuration APIs):

```js
import { markdownLoadMode } from "@clay/markdown";
import { setPackageOption } from "clay:configuration";
import { serverSetLayoutOverride } from "clay:ui";
import { bindKey } from "clay:keybindings";

await markdownLoadMode();

// Optional: show the preview panel by default
setPackageOption({
  packagePrefix: "markdown",
  option: "layout.defaultVisibility",
  value: "visible",
  source: "init-js",
});

// Optional: move the preview to the left slot
serverSetLayoutOverride({
  targetId: "markdown.preview",
  property: "slot",
  value: "left",
  source: "user-config",
});

// Separate explicit Ctrl+O binding for file open
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

Performance rule remains: package UI/layout declaration, validation, panel registration, theme token declaration, and configuration evaluation are startup/load/configuration-time work. No package JavaScript runs in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers.

Anti-patterns for Phase 18.5:
- Do not publish a default fixed panel from the package load path; optional panels start hidden.
- Do not hard-code a side panel position or width; let the shell compose `PaneSlotLayout` from the declared `slot` and user overrides.
- Do not use the SDUI `publishTree` left-slot surface as a user-facing panel authoring pattern; it is a Clay-owned internal retained sidebar, not a package slot API.
- Do not add Markdown-specific Rust shell/layout/input/state/config branches.
- Do not present `serverLoadPackage(packageJson)` as the ordinary end-user load path.

## Phase 18.4 authoring contract summary

Package authors should treat input, actions, state, and configuration as one validated contract:

- Register pointer/focus/selection interests with `clay.ui.serverRegisterInputContribution`; keep keyboard/text behavior in behavior manifests and `clay:keybindings`.
- Route side effects through registered commands and component action intents; unregistered actions, callbacks, raw op names, executable arguments, and authority-bearing paths are rejected.
- Declare state lifecycle with `clay.ui.serverRegisterUiStateScope`; this records schema/lifecycle metadata only and does not write state values, hidden globals, arbitrary JSON blobs, or durable workspace/document/user-config data.
- Customize package defaults through `clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride`; hidden JSON/TOML/ad hoc layout, input, action, state, style, or theme keys are rejected.
- Declare `package-configuration` when package defaults or options change behavior, and keep package-owned IDs package-prefixed.
- Expect diagnostics to preserve package name, version, `apiPrefix`, primitive category, contribution ID, target, payload estimate, failed precedence rule, and failed validation rule.
- Keep validation/publication/configuration work at startup, package load, configuration reload, explicit command handling, or explicit UI update time. No package JavaScript, package parsing, configuration evaluation, raw IPC wait, full-document serialization, or package-authored native widget mutation may run in Masonry paint/layout/pointer/scroll/key/text-event hot paths.
- Phase 18.5 Markdown replanning: first-party Markdown preview/status/input/state/configuration consumes these generic APIs. The package-owned `markdownLoadMode()` fallback entry consumes `serverRegisterModePattern`, `serverActivateMajorMode`, `serverRegisterCommand`, and `serverRegisterParseHandler` internally; the optional preview is a `PanelContribution` with `defaultVisibility: "hidden"` targeting the `right` slot; user customization uses `setPackageOption` and `serverSetLayoutOverride`; no default fixed panel is published on load. See the Phase 18.5 authoring contract section above.

Deferred surfaces remain explicit: durable state-value mutation, persisted workspace/document/user-config storage, pane selector syntax, multi-panel ordering, overlay z-order, cross-window layout behavior, direct working-area/split/pane-slot mutation, and package enable/disable from configuration are planned/deferred until separate documented APIs ship.

## Phase 18.8 authoring contract: command execution and transient menus

Phase 18.8 closes the loop between command registration and command activation. A package registers commands with `clay:commands.serverRegisterCommand`; package UI components, keybindings, and transient menus activate those commands through inert command intents, not through callbacks or client-side handlers.

### Inert action intents

Every interactive package UI action must be an inert command intent:

```js
defineButton({
  label: "Toggle Preview",
  action: {
    commandId: "markdown.togglePreview",
    arguments: { source: "preview-button" },
  },
});
```

The intent carries only a registered command ID and bounded primitive arguments. It does not carry a JavaScript callback, a raw op name, a native handle, a filesystem path, or executable code. Clay rejects unregistered command targets, mismatched package provenance, undeclared permissions, malformed arguments, and oversize payloads before the action becomes active.

### Fixed panels, transient overlays, and transient menus

Package UI contributions use three distinct shell surfaces:

- **Fixed panels** participate in `PaneSlotLayout` and reduce the size of the `main` slot while visible. Register them with `clay.ui.serverRegisterPanelContribution` for `left`, `right`, `top`, or `bottom` slots.
- **Transient overlays** overlay the pane or working area and are dismissible/focus-scoped. Register them with `clay.ui.serverRegisterTransientOverlayContribution` for command palettes, dropdowns, hover docs, modals, or temporary find/replace bars.
- **Transient menus** are Clay-owned active sessions for bottom-pane command browsing and future picker workflows. They reuse the overlay/component primitives for rendering but are managed as a `TransientMenuSession` with prompt, query, bounded items, selection, status, and inert activation actions. The Control Center is the first consumer; future completion, file search, symbol search, and Git pickers can reuse the same generic session model.

A transient menu is not a fixed bottom panel and does not consume fixed `PaneSlotLayout` geometry unless a later explicit declaration installs fixed bottom chrome. It is also not a generic `TransientOverlayContribution`; the overlay contribution declares static overlay metadata, while the transient menu session owns dynamic query/selection/activation state.

### Command execution lifecycle

The lifecycle for package-invoked commands is:

1. **Register** the command with `serverRegisterCommand` at package load time.
2. **Declare** action targets in UI components, input contributions, keybindings, or transient menu items.
3. **Validate** at package load/configuration/update time that every action target resolves to a registered command with compatible routing and declared permissions.
4. **Enqueue** an inert command intent from the client when the user activates the action.
5. **Execute** server-side: `CommandExecutor` validates command ID, routing policy, provenance, permissions, target context, argument budget, and session/action freshness before running the handler.

Command execution is explicit server-first work. It may be async and cancellable, but it never runs synchronously in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers. Ordinary typing remains client-first and does not wait for command execution.

### Performance rule

Package command registration, action validation, and transient menu filtering are load/configuration/update-time work. The client may filter bounded installed metadata locally during query/selection movement, but command side effects run only through the server-owned execution path. No package JavaScript, command handler, package validation, IPC round trip, filesystem, network, shell, AI, WASM, or full-document serialization work may run in Masonry paint/layout/pointer/scroll/key/text-event handlers.

### Security rule

Packages may request transient UI and declare action intents, but they cannot:

- execute client-side JavaScript in the Rust client to handle commands
- create or mutate Masonry widgets directly
- call raw `Deno.core.ops` or expose raw op names as user-facing API
- bypass command permission/provenance validation
- grant themselves filesystem, network, shell, AI mutation, WASM, workspace mutation, package-manager, package installation, package enable/disable, native widget, or raw-op authority
- smuggle callbacks, native handles, filesystem paths, or executable code inside action arguments

Command execution authority is validated per request. Registration or inclusion in a menu does not grant execution authority; the server re-checks permissions, provenance, routing policy, and target context on every activation.

## Phase 18.12 authoring contract: Clay-owned file browser and workspace authority

Phase 18.12 makes the file browser the first real consumer of Clay shell slots and transient picker primitives. It is **Clay-owned**, not a package contribution. The left file tree, bottom fuzzy-open session, workspace-root discovery, bounded directory listing, and file open/reveal commands are first-party compositions of generic primitives so later packages can reuse the same shell concepts without owning workspace authority.

### Slot ownership and UI composition

Clay owns the working area, pane/split tree, fixed pane slots, component catalog, action routing, theme/style token mapping, and native Masonry widget implementation. For the file browser specifically:

- The left file tree is a Clay-owned fixed panel composed from inert SDUI/component data (`Panel`, `Stack`, `Label`, `List`, and `EditorView`) and installed into the `left` shell region. It demonstrates the fixed-panel contract but does not make the workspace tree package-owned.
- The fuzzy-open picker is a Clay-owned bottom transient menu session. It uses bounded installed listing metadata and local query filtering; it is not a fixed bottom panel and not a package `TransientOverlayContribution` that owns dynamic query state.
- File entries emit inert command intents such as `clay.workspace.openFile` and `clay.workspace.revealInTree` with bounded primitive arguments. They never carry JavaScript callbacks, raw op names, native handles, or package-owned filesystem authority.
- Native layout, focus, accessibility, theme token resolution, and Masonry rendering remain client/Rust implementation details hidden behind Clay primitives.

Packages can still declare their own fixed panels and transient overlays through the documented `clay:ui` APIs, but those contributions compose around Clay-owned workspace chrome. A package may request `left`, `right`, `top`, or `bottom` slots for package UI; Clay validates slot conflicts, visibility, component IDs, actions, input policies, and theme tokens, and user configuration may override defaults where documented. Packages must tolerate Clay-owned panels such as the file browser occupying preferred slots.

### Workspace/file authority boundary

File browser authority stays server-owned:

- Workspace roots come from server startup/cwd discovery, opened-file ancestry, explicit user grants, and Clay's closed marker set. Packages cannot add roots, marker names, root discovery rules, or root precedence.
- Directory listing runs through the bounded server listing service with Clay-defined ignore rules, depth/count limits, cancellation, refresh, and diagnostics. Packages cannot list arbitrary filesystem paths, bypass limits, or run directory scans from paint/layout/input handlers.
- In-root opens route through `WorkspaceState::open_existing_file`; out-of-root selected files route through `WorkspaceState::open_selected_file`, which creates a single-file grant. Packages cannot turn a selected-file grant into directory authority.
- `clay.workspace.openFile`, `clay.workspace.openFuzzyFile`, `clay.workspace.openDirectory`, `clay.workspace.revealInTree`, and `clay.workspace.toggleFileBrowser` are built-in server-first commands. Directory navigation requires Clay-provided root-relative arguments from the file-browser SDUI row; it is not a package-owned global key or raw path API. Save/save-as/rename/delete are not registered file-browser commands in this phase.
- `clay.workspace.clientOpenFolderDialog` and `clay.editor.clientCopySelection` / `clientCutSelection` / `clientPasteClipboard` are bindable client UI command IDs, not package authority grants. Packages cannot use them to receive native path handles, add workspace roots without server validation, or invent package clipboard-contents / arbitrary clipboard-text APIs.

Package UI may reference workspace-backed actions only through documented command/API surfaces. It must not pass raw client-chosen paths, call raw `Deno.core.ops`, execute client-side JavaScript, or read files directly from the Rust client.

### Performance contract

The file browser reinforces the shell hot-path rule:

- Fixed-panel rendering and transient menu rendering read already-installed inert state.
- Workspace discovery, directory listing, refresh, fuzzy snapshot creation, command validation, and file open/reveal handling happen at startup, explicit refresh, explicit command/action time, or server-side background work — never inside Masonry paint, layout, pointer, scroll, keypress, or text-event handlers.
- Fuzzy-open query movement filters bounded installed metadata locally. If a package needs broader search, it needs a separate bounded/cancellable server primitive rather than ad hoc paint-time filesystem work.

### Package-facing guidance

Use the file browser as a model for **composition**, not authority:

- Good: declare a package-prefixed preview/outline/diagnostics panel with inert components, registered action intents, documented theme tokens, and hidden-by-default layout defaults.
- Good: declare a transient overlay for package quick actions whose items activate registered package commands.
- Bad: declare a package file tree that scans paths directly, adds workspace roots/markers, claims the file browser's left slot by load order, passes raw paths in UI callbacks, or ships a custom Masonry widget.
- Bad: implement fuzzy search by running package JavaScript, filesystem scans, parse work, blocking IPC, or package command handlers in paint/layout/input hot paths.

Testing for package UI that coexists with the file browser should assert slot conflict diagnostics, fixed-vs-transient behavior, inert command arguments, no raw-op/native-widget/CSS usage, and no workspace authority beyond documented root/grant APIs.

## Phase 20 authoring contract: multi-document sessions, dirty/save status, and recovery chrome

Phase 20 hardens daily editing around Clay-owned shell chrome. Multi-document session switching, dirty/save status, conflict recovery menus, and pending-edit/disconnect/resync recovery surfaces are **Clay-owned**, not package layout contributions. Packages continue to declare inert panels, components, overlays, and command intents through `clay:ui` / `clay:commands`; they do not own document tabs, status chrome, native widgets, or recovery menus.

### Clay-owned surfaces packages must not replace

| Surface | Owner | Package role |
| --- | --- | --- |
| Working area, pane/split tree, fixed slots (`left`/`right`/`top`/`bottom`), mandatory `main` editor | Clay shell | May request optional slots for inert panels; must tolerate Clay chrome |
| Active document title, dirty marker, pending-edit count, theme label, recovery summary | Clay status chrome (`EditorStatus` / `SduiStatusObservation`) | May publish package status items; must not invent a second dirty/save bar |
| Open-documents switcher (`clay.editor.clientShowOpenDocuments`) | Clay client session map (`DocumentSessionStore`, bound 64) | May bind the command ID; must not ship a package tab host or native document switcher |
| Save / reload / conflict menus (`StaleFileMetadata`, `DirtyDocument`) | Clay transient menus + server document IO | May bind `clay.documents.serverSaveDocument` / `serverReloadDocument`; must not open native save dialogs or write files |
| Pending-edit / disconnect / resync recovery (`clientRequestResync`, `clientDismissRecovery`) | Clay recovery menus + existing `RequestResync` | May bind the command IDs; must not invent reconnect sockets or package-owned resync loops |
| Native file-open dialogs | Clay client platforms + selected-file grants | Must not call platform dialog APIs directly |

Clay remains the owner of shell slots, the document switcher, status chrome, and native widgets. Package UI contributions stay inert declarations that Clay validates, composes, and renders.

### What packages may observe and bind

Packages and `~/.config/clay/init.js` may use documented Clay JS surfaces only:

```js
import { bindKey } from "clay:keybindings";
import {
  clientShowOpenDocuments,
  clientRequestResync,
  clientDismissRecovery,
} from "clay:editor";
import { serverListDocuments } from "clay:documents";

// Bind Clay-owned chrome commands. These are inert client UI / server-first
// command IDs — not package authority grants.
bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
bindKey("Ctrl+Shift+E", clientShowOpenDocuments(), { scope: "editor" });
bindKey("Ctrl+Shift+R", clientRequestResync(), { scope: "editor" });
bindKey("Escape", clientDismissRecovery(), { scope: "editor" });

// Server-authoritative open-registry metadata (identity, dirty, lease).
// This is not the client DocumentSessionStore and does not switch sessions.
const documents = await serverListDocuments();
for (const document of documents) {
  console.log(document.documentId, document.dirty);
}
```

Rules for those surfaces:

- `serverListDocuments` / `serverGetDocumentStatus` report server open-registry metadata. They do not list or mutate the client-retained session map, caret/viewport/history stash, or recovery menu state.
- `clientShowOpenDocuments` opens Clay's transient open-documents menu. Activation stays client-local (`clientActivateDocument` via menu arguments). Packages must not reimplement tabs as fixed panels, Masonry widgets, or SDUI trees that claim shell ownership of the active document.
- Dirty/save chrome is updated by Clay from local edits, `DocumentSaved`, `DocumentReloaded`, and conflict diagnostics. Packages must not create native save dialogs, arbitrary file writes, or a parallel dirty indicator that bypasses server save/reload.
- Recovery menus are Clay `TransientMenuSession` instances. Package transient overlays remain separate: they cannot replace conflict/resync/disconnect recovery, inject callbacks into those menus, or escalate filesystem/network/shell authority from a recovery action.

### Performance contract

Phase 20 adds **no** package paint-path requirements:

- Document switch, dirty chrome, and recovery menu paint read already-installed client/server state.
- Package JavaScript still runs only at load, configuration evaluation, explicit command handling, or explicit UI update time.
- Masonry paint, layout, pointer, scroll, keypress, and text-event handlers must not run package JavaScript, scan the multi-document session map from package code, open dialogs, or perform save/reload IO.

### Security and deferred authority

Phase 20 does **not** give packages, configuration, or AI:

- clipboard-contents APIs (read/write arbitrary clipboard text beyond user-mediated cut/copy/paste command IDs)
- arbitrary file writes or direct native file/save dialogs
- filesystem, shell, network, or raw `Deno.core.ops` authority
- ownership of document sessions, leases, or recovery chrome

Broader package/config/AI authority over those surfaces remains deferred and must be established in a later dedicated decision. Binding `clientShowOpenDocuments`, `clientRequestResync`, `clientDismissRecovery`, `clientCutSelection` / `clientPasteClipboard`, or `clay.documents.clientOpenFileDialog` installs only the documented inert command route; it is not a grant of native handles, clipboard contents, or workspace expansion.

### Package-facing guidance

- Good: bind Clay save / open-documents / resync command IDs from `init.js`; declare package-prefixed status items or preview panels that compose around Clay chrome.
- Good: keep language/preview UI as inert `clay:ui` contributions that tolerate Clay-owned left file browser, bottom recovery menus, and status dirty markers.
- Bad: ship a package tab strip, dirty badge overlay, or save button that writes files, opens OS save dialogs, or mutates Masonry widgets.
- Bad: replace disconnect/resync recovery with package reconnect sockets, background resync loops, or paint-time session scans.
- Bad: treat `serverListDocuments` dirty flags as permission to overwrite disk or force-reload without Clay's conflict recovery path.

See also [File Open, Save, and Reload Workflow](../../development/file-open-save-reload-workflow.md) for the user-facing open/save/conflict flow and [Masonry Editor](../../wiki/modules/masonry-editor.md) for client session/recovery implementation notes.

## Phase 18.9 authoring contract: generic text/code fallback modes and generic key behavior

Phase 18.9 makes every document editable even when no language package is installed, disabled, or invalid, by registering always-on Clay-owned fallback major modes (`core.text`, `core.code`) at server startup. Language packages do **not** replace these fallbacks; they **extend** `core.code`/`core.text` by declaring modes, classification patterns, and behavior manifests through the same generic primitives every other mode uses: `DocumentClassification`, `MajorModeActivation`, `TextTransform`, `KeyRoutingOverride`, and `CommandDeclaration`. The canonical primitive detail lives in [Primitive Registry Schema](../primitives/registry.md); this section is the package-author contract.

### What a language package declares

A language package extends the built-in fallbacks with declarative metadata only — it must not add language-specific Rust branches or client-side JavaScript:

```js
import { serverRegisterModePattern, serverActivateMajorMode } from "clay:modes";

// Declare a package-owned major mode and its classification pattern.
serverRegisterModePattern({
  apiPrefix: "rust",
  modeId: "rust",
  patterns: [
    { kind: "extension", value: "rs" },
    { kind: "filename", value: "Cargo.toml" },
    { kind: "shebang", value: "rust-*" } // single-wildcard glob, optional
  ],
  editorRules: {
    tab: { mode: "insert-spaces", width: 4 },
    pairs: [ { opener: "{", close: "}" } /* ... */ ],
    electricCharacters: [ { trigger: "}", effect: "outdent-one-level" } ],
    // Optional (Plan 071 task 11): movement and caret appearance overrides.
    // Absent fields fall back to the code-editing / editor defaults, so
    // customization is strictly opt-in and never changes other modes.
    movement: { wordSeparators: "code", camelCaseSubWord: true },
    caretStyle: { shape: "bar", blink: "solid" }
  }
});

serverActivateMajorMode(/* ... via the documented activation path ... */);
```

Packages contribute **parameters** (rule data) only; Clay executes only Rust-known transform engines. Unknown `electricCharacters` effects are dropped, not executed. A language package is the recommended way to change classification/behavior — there is no undocumented configuration key to override `core.*` defaults (see [Configuration Runtime](../../wiki/modules/configuration-runtime.md)).

### Classification precedence (package authors must not rely on load order)

When a document opens, Clay chooses one active major mode from a deterministic precedence ladder; no package wins by load order:

1. User override via documented Clay JS APIs (e.g., an init.js-declared package pattern)
2. Package-declared pattern: exact filename > wildcard filename > extension > MIME
3. Shebang line (interpreter matches a declared pattern)
4. Bounded leading-content probe (literal marker at document start)
5. `core.code` (code-like extensions and any shebang)
6. `core.text` (universal fallback)

On an equal-precedence tie, a package-declared mode beats a built-in, and only same-provenance ties raise an `AmbiguousClassification` diagnostic. Probes read only a bounded constant prefix (`MAX_LEADING_CONTENT_BYTES = 512`) of an already-open document supplied by the open path; they perform no filesystem scan, directory walk, or arbitrary package predicate, and oversize slices are rejected and classified to a fallback mode.

### Generic transform kinds are reusable across future modes

The `TextTransform` kinds that ship `core.code` are deliberately generic and reusable by any future language mode, not Markdown-only or Python-only:

- **Pair insertion** (`PairRule`) — opener/close pairs inserted client-side from inert manifest data.
- **Comment continuation** (`CommentContinuationRule`) — Enter inside a comment continues the comment marker.
- **Electric characters** (`ElectricCharacterRule` + `ElectricEffect::OutdentOneLevel`) — typing a closing `}`/`)`/`]` auto-outdents an over-indented line.
- **Tab** (`TabRule`) and **Enter** (`EnterRule`) — indentation and newline transforms.

Two default rule sets ship: `EditorBehaviorRules::default_text()` (plain text, no electric) via `BehaviorManifest::minimal_text_editing`, and `EditorBehaviorRules::default_code()` (code-oriented, with electric reflow) via `BehaviorManifest::core_code_editing`. `core.code` ships a default electric set for `}`/`)`/`]`; `core.text` ships none. A future language package declares its own rule parameters under `clay.modes` editor rules and only the `outdent-one-level` electric effect is accepted.

### Discovery command contract

Package authors and the Control Center can inspect active modes through built-in, read-only, server-first commands (not Clay JS facades, no execution/document/workspace authority):

- `clay.modes.listActiveModes` — returns per-document `modeId`, `provenance` (`CoreBuiltIn` or `Package`), and `classificationSource`.
- `clay.modes.explainActiveMode` — returns the active mode, display name, `fallbackUsed` flag, and a human-readable `why` rationale (e.g., "core.text universal fallback: no language package matched").

These commands carry empty permissions and are resolved server-side via `CommandExecutor::execute_discovery`; they introduce no new authority.

### Performance budgets (hot-path contract)

Generic key behavior is `ClientFirstPredictable`: the Rust client executes inert behavior-manifest data for Tab/Enter/pair/comment/electric transforms with **no synchronous JavaScript, IPC, or server round trip before local paint**. Packages must respect:

- `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS = 16` — keypress-to-local-paint budget; no sync JS before paint.
- `MODE_ACTIVATION_P95_BUDGET_MS = 100` — mode-activation budget.
- `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES = 8192` — package manifest metadata payload budget (including inert `clay.contributions.*` and versioned `clay.extensionPoints` declarations; sized to fit first-party theme packages' full `textStyles` mappings, Plan 046, raised for extension-point declarations in Plan 061); oversize manifests are rejected with `ManifestValidationFailed`/`PayloadBudgetExceeded` at record time.

Mode/classification defaults are compile-time (no configuration-evaluation cost at paint/text time). Configuration evaluation is bounded to init.js/package load or explicit setting change by `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS = 25`.

### Security boundaries

- **`core.*` ID ownership is reserved.** The `core.` mode-ID prefix is reserved for Clay-owned built-ins and cannot be registered by a package (`register_mode`/`register_minor_mode` reject `core.*` and `clay.*`).
- **Deny-by-default authority.** Built-in fallback modes require no package and grant no package authority. Packages cannot grant themselves filesystem, network, shell, AI mutation, WASM, package-manager, package installation/enable-disable, native widget, raw-op, or client-side-JavaScript authority.
- **Packages contribute parameters/declarations only.** Electric characters, pair insertion, and comment continuation are declarative manifest data; Rust-known engines execute them. Packages must not add language-specific Rust branches, raw `Deno.core.ops`, native handles, raw CSS, callbacks, or client-side JavaScript.
- **Built-in defaults cannot be overridden to grant authority.** `setPackageOption` uses a closed suffix allowlist and rejects Phase 18.9 behavior-changing keys (`core.preferredFallbackMode`, `core.electricCharacters`, `core.pairInsertion`, `core.commentContinuation`) as unsupported options.
- **Bounded probing.** Shebang/content probes read only a bounded prefix of an already-open document; no filesystem scan authority is introduced.

### Migration notes and limitations

- Files that previously produced "no classification match" now open into `core.text` (or `core.code` for code-like extensions / any shebang); packages that asserted `NoClassificationMatch` in tests should assert `core.text`/`core.code` fallback instead.
- A language package with a higher-precedence pattern continues to win over `core.code`/`core.text`; no migration is needed for existing package-declared modes.
- Built-in modes ship their own default behavior manifests without an owning package; `select_behavior_manifest_for_document` detects the `core.` prefix and bypasses package-record lookup.
- There is intentionally no runtime configuration knob for the fallback mode or electric toggles (YAGNI; the package system is the override escape hatch). Do not add undocumented `setPackageOption` keys for these.

## Phase 18.18 authoring contract: native grammars and vocabulary styleMaps

Phase 18.10 introduced `SyntaxGrammarContribution`; Phase 18.18 promotes first-party packages to native grammar metadata and direct `TokenType` + `Modifiers` styleMaps. A grammar-only package highlights documents whose active major mode may still be `core.code` or `core.text`; it does **not** register a full major mode, commands, completions, UI, key behavior, or language-specific Rust branches.

Declare grammar assets under `clay.contributions.syntaxGrammars`:

```json
{
  "clay": {
    "apiPrefix": "rust",
    "permissions": ["parse-document", "render-decorations"],
    "apiDependencies": ["clay.syntax.serverRegisterSyntaxGrammar"],
    "contributions": {
      "syntaxGrammars": [{
        "languageId": "rust",
        "filePatterns": { "extensions": ["rs"] },
        "grammar": { "kind": "native", "source": "tree-sitter-rust" },
        "queries": { "highlights": "./queries/highlights.scm" },
        "styleMap": {
          "keyword": { "type": "Keyword" },
          "string": { "type": "String" },
          "comment": { "type": "Comment" },
          "punctuation": { "type": "Operator" },
          "function.declaration": {
            "type": "Function",
            "modifiers": ["Declaration"]
          }
        },
        "budgets": { "timeoutMs": 5000, "maxWindowBytes": 4096 }
      }]
    }
  }
}
```

Validation is load-time only and reuses the package metadata budget. Grammar contributions remain first-party-only here; arbitrary third-party native artifact loading is out of scope. Tier 1 native entries require a compiled source ID and reject an artifact path; Tier 2 WASM entries require a package-root-confined `.wasm` path. Query files must be confined `.scm` assets. Vocabulary styleMaps accept closed `TokenType` variant names and closed `Modifiers` names; known legacy style tokens remain compatible. Packages must declare both `parse-document` and `render-decorations`. Clay rejects non-`@clay/*` grammar packages, absolute paths, parent traversal, URLs/downloads, native libraries, package-manager/shell fields, raw ops, client JavaScript, CSS/raw colors, duplicate language IDs, and duplicate file-pattern claims. Parse/highlight work runs as `Background`, cancellable, viewport-prioritized server work bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`; it never runs in keypress, paint, layout, scroll, pointer, or text-event hot paths. First-party grammar packages are loaded explicitly from `~/.config/clay/init.js`; they are not auto-loaded.

### Text-object grammar contributions (`queries/textobjects.scm`, Plan 071 task 10)

Alongside `highlights.scm`, first-party native grammars ship a `queries/textobjects.scm` query file (`@clay/rust`, `@clay/typescript`/TSX, and `@clay/javascript` today). The contribution lives in the compiled-in native descriptor — the same route as `highlights.scm` — not in package `package.json` metadata: the `queries` object in grammar contributions accepts only `highlights`, `locals`, and `injections` and rejects any other key deny-by-default, so text-object queries cannot be declared in package metadata at all. Text-object queries carry the same `parse-document` permission and package-root path confinement. They grant no file, network, shell, AI, WASM, or client-side JavaScript authority.

Capture schema (mirrors the Helix/Nvim convention, Clay-prefixed):

```scheme
(function_item) @textobject.function.around
(function_item body: (block) @textobject.function.inner)
```

- **Kinds**: `function`, `class`, `argument`, `comment`, `loop`, `conditional`, `call`, `statement` (8 closed kinds; unknown kinds are rejected by the op validators).
- **Scopes**: `around` covers the whole node; `inner` covers the meaningful interior (typically the body/list). Kinds without an `inner` capture fall back to `around` at query time.
- **Directions** (`current`/`next`/`previous`) are runtime concerns selected by the command ID, not query captures: e.g. `clay.editor.clientSelectTextobject.function.inner.next`.

Runtime behavior is advisory: `clientSelectTextobject`/`clientSmartSelect` send the document's selection set to the server, which answers with one optional range per caret (multi-cursor-aware); grammars without a textobjects query (Markdown, Tier 2/3 handlers) answer no ranges and the carets stay put. Selection queries never block editing — any miss (no grammar, parse timeout, no handler) degrades to empty ranges. The 50 command IDs (48 textobject + 2 smart-select) are not enumerated in the built-in command table; they are auto-declared the first time a package binds a key to one via `bindKey`, and validated by prefix parse rather than string lists. Smart-select (`expand` walks the syntax tree up, `shrink` walks down) works for any parsed grammar even without a textobjects query.

```js
// In a package loadEntry: bind a text-object command (single-stroke chords
// only; multi-stroke chords like "]f" are not runtime-backed yet).
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+F", "clay.editor.clientSelectTextobject.function.around.current", { scope: "editor" });
```

Testing guidance: query files must compile against their grammar (`tests/syntax_grammar.rs` covers compilation plus inner/around/direction semantics for the shipped languages); new first-party grammars add their `textobjects.scm` next to `highlights.scm` and extend the descriptor. Limitations: third-party grammar contributions cannot declare textobjects through metadata today (native descriptors only); selection ranges are byte-offset based and recomputed per request rather than cached.

For each accepted consecutive document version and stable parse window, Clay supplies one exact `ParseInputEdit`. A matching Tree-sitter tree is edited with `Tree::edit`, parsed once, and queried over the UTF-8-safe envelope of Tree-sitter changed ranges plus explicit invalidations intersected with the visible range. `QueryCursor::set_byte_range` returns intersecting complete captures; package grammar boundaries define whole-token, comment, string, prose, and code correction rather than whitespace or idle timing. One parse/capture pass fans out complete spans into stable 128-byte `DecorationSet` outputs. Changed/visible output is published first, all members are validated atomically, and output chunk count never creates sibling parser jobs or multiplies parse work. Open, resync, and viewport-only work use bounded full/visible fallback without fabricated edit metadata.

The client may provisionally interpolate already-validated inert syntax spans through optimistic edits. Broad generic token families can inherit edge insertions while narrow tokens wait for authoritative grammar output; current server sets replace affected package/layer ranges, including empty authoritative syntax sets. Semantic, diagnostic, and search layers remain separate. Package authors do not add client parsers, per-keystroke callbacks, language-specific Rust branches, hidden debounce/word-boundary settings, or manual decoration publication to user configuration.

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");
```

Do not add hidden JSON/TOML/ad hoc syntax configuration keys for preferred grammar selection, grammar paths, style maps, capture styles, or auto-load behavior. If a later phase exposes any of those as user preferences, they must be promoted as documented Clay JS APIs with custom properties and registry coverage.

Shipped first-party grammar packages are documented at:

- [`@clay/rust`](rust.md)
- [`@clay/typescript`](typescript.md)
- [`@clay/javascript`](javascript.md)
- [`@clay/markdown`](markdown.md)

## Phase 18.16 authoring contract: tiered syntax engine

Phase 18.16 introduced engine selection; Phase 18.18 lets package metadata honestly declare Tier 1 `native` sources or Tier 2 `tree-sitter-wasm` assets. Clay's host registry chooses the engine at package-load/open/reclassification time:

1. **Tier 1 — native first-party.** Clay seeds compiled-in `tree-sitter-*` grammar data for Rust, TypeScript/TSX, JavaScript, and Markdown. Dispatch is by descriptor data (`languageId`, extensions/file names, query path, and style map), not language-specific Rust branches. First-party package load remains required for package-owned mode behavior; native syntax registration does not silently auto-load a package.
2. **Tier 2 — web-tree-sitter WASM.** A package-root-confined `./grammars/*.wasm` plus `./queries/*.scm` contribution uses the shared host adapter. It replaces Tier 1 only after explicit user selection, for example `setSyntaxEnginePreference("rust", "wasm")`; package load order alone cannot promote it.
3. **Tier 3 — package JavaScript fallback.** Existing `clay.parse.serverRegisterParseHandler` handlers remain available for grammar-less packages, Markdown-specific parser behavior, or an explicit `javascript` preference. This route uses the existing server-issued handler token and does not run package JavaScript in the client.

All tiers feed one grammar/capture-to-vocabulary path. A query capture maps through `styleMap` to Phase 18.15 `TokenType` + `Modifiers` and publishes only bounded inert `DecorationSet` spans with provenance. Direct vocabulary entries are scope-less; validated legacy style tokens preserve their compatibility scope. Unmapped captures are ignored (unstyled) rather than assigned a fallback color. The active syntax grammar remains separate from the active major mode, so a document can stay editable as `core.code` or `core.text` while highlighting is selected. Token-boundary tests use the real Rust, TypeScript, TSX, JavaScript, and Markdown package queries; package grammar captures remain the source of truth for transitions such as `le` → `let`, heading changes, punctuation insertion, delimiter completion, and broad comment/string/prose/code growth.

```js
import { loadPackage } from "clay:packages";
import { setSyntaxEnginePreference } from "clay:syntax";

// Normal first-party setup: Tier 1 native is selected by default.
await loadPackage("@clay/rust");

// Optional explicit selection, evaluated before package registration/open.
setSyntaxEnginePreference("rust", "wasm"); // or "javascript"
```

Open is enqueue-only: text and the initial mode manifest return before background parse completion. Later parse failures or invalid results publish sanitized `RuntimeDiagnostic` values such as `clay.parse.open_failed`; they must never block typing or leak paths/source text. Parse windows, decoration payloads, and retained syntax cache remain bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`.

Tier 2 binaries may be supplied by a package that explicitly declares WASM metadata. First-party packages currently ship native metadata only; each `grammars/PROVENANCE.md` still records the upstream crate/release and reproducible `tree-sitter build --wasm`/hash procedure for future audited artifacts. Runtime does not fetch, build, shell out, install packages, load native libraries, or execute client-side JavaScript. First-party artifact loading and package-root confinement remain required; third-party grammar/native trust is deferred to Phase 23 and a separate security decision.

Use the documented `clay:syntax` API for engine preference. Do not add hidden JSON/TOML keys for grammar paths, query paths, style maps, auto-loading, or tier selection. See [`setSyntaxEnginePreference`](../clay-js-api/syntax/set-syntax-engine-preference.md) and the [Syntax Grammar Registry](../../wiki/modules/syntax-grammar-registry.md) implementation guide.

## Phase 18.11 authoring contract: completion providers

Phase 18.11 adds the `CompletionTriggerAndResult` primitive and server-side completion framework; Phase 18.18 extends its metadata-only package contract with bounded static text items. A package declares provider metadata, trigger/word-boundary parameters, and optional inert `items`, and Clay owns trigger classification, result computation scheduling, and the completion picker UI. Package authors do **not** ship an executable completion handler, raw callback, raw op, native handle, client JavaScript, snippet with executable transforms, command side effect on accept, CSS, or any completion-specific popup widget.

Completion reuses the Phase 18.8 `TransientMenuSession` bottom overlay and `SduiNativeState` active-menu rendering with `KeyBindingContext::CompletionMenu`; do not add a completion-specific Masonry widget tree, custom popup, or fixed bottom panel for completions. Accepting a completion commits a validated text replacement in the active document only — it never executes a command, raw op, or provider code.

Declare completion provider contributions under `clay.contributions.completionProviders` and register the metadata from the package load entry through `clay.completion.serverRegisterCompletionProvider`:

```json
{
  "clay": {
    "apiPrefix": "words",
    "permissions": ["completion-provider"],
    "apiDependencies": ["clay.completion.serverRegisterCompletionProvider"],
    "contributions": {
      "completionProviders": [{
        "id": "words.buffer",
        "priority": 0,
        "triggerCharacters": ["."],
        "wordBoundaryChars": [".", ","],
        "items": ["const", "function", "return"],
        "budgets": { "timeoutMs": 500, "maxItems": 64 }
      }]
    }
  }
}
```

```js
import { serverRegisterCompletionProvider } from "clay:completion";

export default function load() {
  serverRegisterCompletionProvider({
    packageName: "@vendor/words",
    packageVersion: "0.1.0",
    packagePrefix: "words",
    permissions: ["completion-provider"],
    providerId: "words.buffer",
    triggerCharacters: ["."],
    wordBoundaryChars: [".", ","],
    items: ["const", "function", "return"],
    timeoutMs: 500,
    maxItems: 64
  });
}
```

End users load a completion provider package with one explicit `loadPackage` call; no provider package auto-loads silently:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@vendor/words");
```

Validation is load/registration-time only and reuses the package metadata budget. Provider IDs must be package-owned (`<apiPrefix>.<name>`), must not claim the reserved `clay.*` namespace, and must be unique within a package. Trigger characters are inert single-character strings; word-boundary characters are inert strings. Static `items` must be unique non-empty strings, fit `CompletionItem` label/insert-text limits, and contain no more entries than `maxItems` or `COMPLETION_RESULT_MAX_ITEMS`. `timeoutMs` must be within `1..=5000` and `maxItems` within `1..=COMPLETION_RESULT_MAX_ITEMS`. Clay rejects raw callbacks (`handler`, `callback`, `complete`, `function`, `module`), raw ops, native handles, client-side JavaScript, snippets/commands, URLs, shell/network/AI/WASM/native/package-manager fields, duplicate provider IDs, and oversize metadata.

Result items are inert text-replacement data only: `label`, `insertText`, `detail`, `commitCharacters`, and provenance. They carry no callbacks, command side effects, file paths, shell/network/AI directives, raw op names, or client JavaScript. Providers may read only Clay-provided open-document content/windows; completion grants no filesystem/network/shell/AI/raw-op/native-UI/client-runtime authority without later documented APIs and an approved decision log. Per-field and result payload budgets (`COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `COMPLETION_RESULT_MAX_ITEMS`, and per-field char caps) are enforced before client publication.

Trigger classification is local manifest lookup: typing a trigger character edits locally first (`ClientFirstPredictable`) and then enqueues a typed `CompletionRequest` through a bounded non-blocking channel. Manual `completion.trigger` requests completions without mutating text. Provider execution runs server-side on a cancellable `UiReactivePriority` lane that aborts or stale-drops older in-flight requests and validates results against the current document/behavior version and provider generation before publication. Provider work is UI-reactive/cancellable and never runs on keypress-to-local-paint, paint, layout, scroll, pointer, or text-event hot paths.

Phase 18.11 ships one built-in `core.bufferWords` provider that suggests unique words from the bounded server-prepared document window around the cursor prefix; it is always available and is not removed by package disable/reload. Phase 18.18 package providers registered through `clay.completion.serverRegisterCompletionProvider` remain callback-free: registered static strings normalize to provenance-bearing `CompletionItem` text replacements, and the connection path filters the active package's Rust snapshot by replacement prefix without running package JavaScript. A future constrained handler bridge may add computed package providers; current package execution is limited to bounded static text. Any future provider needing workspace, network, AI, shell, or filesystem authority must introduce explicit permissions and an approved decision log before implementation.

See [`clay.completion.serverRegisterCompletionProvider`](../clay-js-api/completion/server-register-completion-provider.md) for the authoritative API reference, and [`docs/wiki/modules/phase18.11-completion-provider-primitive-review.md`](../../wiki/modules/phase18.11-completion-provider-primitive-review.md) for the implementation review.

## Phase 18.19 authoring contract: snippets, exclusive claim, and disable-native

Phase 18.19 extends the package completion contract with three capabilities: inert snippet items, exclusive provider claim, and a `serverDisableCompletion` configuration API.

### Snippet items (`textFormat: "snippet"`)

Package completion provider `items` now accept structured objects alongside plain strings:

```jsonc
{
  "completionProviders": [{
    "id": "rust.snippets",
    "priority": 0,
    "triggerCharacters": ["."],
    "items": [
      { "label": "fn", "insertText": "fn ${1:name}(${2:args}) {\n\t$0\n}", "textFormat": "snippet", "detail": "function" }
    ]
  }]
}
```

Snippet `insertText` carries inert LSP placeholder syntax (`$1`, `${2:default}`, `$0`). Accepting a snippet item expands the text client-local, selects the first non-final placeholder, and Tab/Shift-Tab navigates between placeholders with Escape to exit. No provider code runs on accept. Mixing plain-text and snippet items in one provider is rejected so independently targetable providers stay explicit.

### Exclusive claim (`exclusive: true`)

A completion provider may set `exclusive: true` to suppress strictly lower-priority matching providers. When the exclusive provider is in the highest-priority tier for a trigger, all lower-priority matches are dropped from the result set; equal-priority peers remain. A lower-priority exclusive provider cannot claim a request from a higher-priority non-exclusive match. The field is inert metadata consulted at selection time with no provider execution.

### Disable-native (`serverDisableCompletion`)

`clay.completion.serverDisableCompletion` suppresses a registered completion provider by exact ID (`core.bufferWords`, `rust.snippets`) or package prefix (`rust`). Use from `~/.config/clay/init.js`:

```js
import { serverDisableCompletion } from "clay:completion";
serverDisableCompletion({ provider: "core.bufferWords" });
serverDisableCompletion({ packagePrefix: "rust" });
```

The target is recorded in a server-side disabled-provider set consulted by every trigger selection path. In-flight results are stale-dropped via a provider generation bump. Disabled state persists across runtime reloads; re-enabling requires a package reload or runtime restart. The API grants no filesystem, network, shell, AI, workspace, or other authority; it only suppresses already-registered inert metadata.

See [`clay.completion.serverDisableCompletion`](../clay-js-api/completion/server-disable-completion.md) for the full API reference.

## Documentation Requirements

Each package should include docs for:

- package purpose
- install/load instructions
- default `init.js` setup
- optional configuration
- modes and file patterns
- commands and keybindings
- UI/layout contributions
- input behavior
- theme tokens/style options
- permissions and non-authorities
- performance expectations
- troubleshooting
- examples
- tests/smoke validation

Docs path example:

```json
{
  "clay": {
    "docs": "./docs/index.md"
  }
}
```

When a phase adds or changes package UI/layout/input/action/state/style APIs, update this guide in the same phase.

## Testing Package Behavior

Recommended test categories:

1. **Manifest validation** — required fields, prefix, permissions, docs, performance metadata.
2. **Conflict tests** — duplicate modes, commands, keybindings, slots, config keys, theme tokens.
3. **Runtime loader tests** — `serverLoadPackage` remains a lower-level validation helper for fixtures, while `loadPackage(specifier)` is the implemented runtime-backed end-user default. Customization after the one-line load uses `setPackageOption` / `serverSetLayoutOverride`, and the module loader only accepts resolver-validated package load entries.
4. **Mode tests** — classification, activation, behavior manifest composition. For Phase 18.9: assert unknown/plain-text files fall back to `core.text` and code-like extensions/shebangs to `core.code`; assert a package-declared pattern wins precedence over built-ins; assert electric/pair/comment transforms execute client-side from the manifest without IPC; assert `core.*`/`clay.*` mode IDs are rejected at registration; assert oversize behavior manifests are rejected at the payload budget.
5. **Input tests** — key routing, command routing, mouse/component actions.
6. **UI tests** — slot placement, fixed/transient panel behavior, overlay geometry, action validation, and observability privacy. Phase 18.12 package UI tests should also cover Clay-owned file-browser coexistence. Phase 20 package UI docs/tests should assert coexistence with Clay-owned multi-document switcher, dirty/save status chrome, and recovery menus without package-owned tabs, native save dialogs, or paint-path session scans.
7. **Theme/style tests** — token validation, same-type fallback mapping, typed style variables, and raw CSS/color rejection.
8. **Package metadata tests** — `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, `themeTokens`, duplicate fixed slot claims, and bounded payload diagnostics.
9. **Parse/render tests** — bounded snapshots, stale result rejection, decoration payload budgets.
10. **Docs tests** — package docs, primitive docs, and master index links stay current.
11. **Manual smoke tests** — actual GUI package loading and user workflow checks. For Phase 18.9, the smoke path in [Launch and GUI Smoke](../../development/launch-and-gui-smoke.md) opens files with no language package and confirms editable `core.text`/`core.code` fallback modes. For Phase 18.12, smoke the left file browser and bottom fuzzy-open against a bounded workspace and confirm opens/reveals route through server commands, not package callbacks.

## Phase 18.14 authoring contract: upgrading grammar-only language packages to full language packages

Phase 18.14 expands `@clay/rust`, `@clay/typescript`, and `@clay/javascript` from grammar-only syntax packages into full first-party language packages. The upgrade path keeps the existing `clay.contributions.syntaxGrammars` contribution unchanged, adds a `clay.modes` entry and mode-specific metadata, and registers additional surfaces from the package `loadEntry` using generic Clay primitives. A document's active syntax grammar remains selectable independently of its active major mode; loading the language package must not silently change the mode of already-open fallback documents.

End-user default remains one explicit line per package in `~/.config/clay/init.js`:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
```

Optional customization is exposed through documented Clay/package JS APIs, not by copying the package manifest into `init.js`. Examples include binding a language command to a key, toggling a package option, or changing the default panel visibility.

### Declarative additions beyond grammar-only

Keep the `syntaxGrammars` block exactly as shipped in Phase 18.10. The same metadata is the Tier 2 package contribution in Phase 18.16; Tier 1 native selection and Tier 3 JavaScript fallback are host/runtime decisions. Add the following full-language surfaces through generic primitives:

- **Major mode**: declare `clay.modes` and register a mode pattern with `clay.modes.serverRegisterModePattern`. The pattern uses generic file-extension, MIME-type, and bounded shebang/leading-content probes; do not add language-specific Rust classification branches.
- **Behavior manifest**: declare editor rules (indentation, tab, generic Enter rule, delimiter pairs, comment continuation, electric characters, movement, caret appearance) through the behavior manifest API. Use `clay.behavior.buildCodeEditingManifest({ indentSize, enter, lineComment, pairs, electricOutdentCharacters, autocompleteTriggers, movement, caretStyle })` to produce validated inert rules for code or prose modes; do not add language-specific behavior branches in core.
  - `movement` (optional, Plan 071 task 11): word/paragraph motion policy. Prose modes (e.g. Markdown) declare `{ wordSeparators: "prose", treatUnderscoreAsWord: false, camelCaseSubWord: false }`; code modes declare `{ wordSeparators: "code" }` (identical to the built-in default; declaring it is optional and documents intent). Absent fields fall back to the code-editing defaults.
  - `caretStyle` (optional): caret shape/blink override (`shape: "bar" | "line" | "block" | "underline"`, `blink: "solid" | "blink" | "phase" | "smooth"`, `widthPx`, `heightPct`, `hollow`, `stopBlinkOnTyping`). Absent means the reduced-motion-safe editor default bar; `clientSetCursorStyle` overrides per-mode values at runtime.
  - Ligatures are typography-owned, not mode-owned: a mode's font role (`defaultFontRole`) selects the `FontProfile` whose `ligatures` policy applies. Users customize ligatures per role with `clay.theme.setTypography` (`ligatures: { enableStandard, enableContextual, discretionaryFeatures, rawFeatures, disableFeatures }`); packages never set ligatures directly and loading a package grants no ligature authority.
- **Commands**: register package-prefixed commands with `clay.commands.serverRegisterCommand`. Commands must route through the server-owned `CommandExecution` path, declare permissions, and avoid shell/network/filesystem authority unless explicitly approved.
- **Completion providers**: register keyword/snippet providers with `clay.completion.serverRegisterCompletionProvider`. Completion providers remain metadata-only and never ship executable handlers, raw callbacks, or client JavaScript. Derive `triggerCharacters` from the major-mode behavior manifest with `clay.completion.completionTriggerCharactersFromEditorRules(editorRules)` so the editor's autocomplete triggers and the completion framework's provider selection stay aligned.
- **Parse handlers**: register a mode-scoped parse handler with `clay.parse.serverRegisterParseHandler` to derive decorations, folding ranges, diagnostics, or outline data. The handler runs as `Background`, cancellable, viewport-prioritized server work and never in paint/typing hot paths.
- **Range diagnostics**: publish bounded `DiagnosticSet` data with `clay.diagnostics.serverPublishDiagnostics` under `render-decorations`. Keep status failures on `RuntimeDiagnostic`; keep visual tints on `serverPublishDecorations`. See [Range Diagnostics](../primitives/diagnostics.md).
- **UI contributions**: declare optional components, status items, transient overlays, panels, and theme tokens through the `clay:ui` contribution APIs. All UI contributions are inert declarations that Clay validates, composes, and renders through Clay-owned Masonry widgets. Packages never create Masonry widgets, mutate native layout, provide raw CSS, run client-side JavaScript, or call raw `Deno.core.ops`.
- **Configuration**: Phase 18.14 language packages keep indent size, comment token, delimiter pairs, and autocomplete triggers as package-defined defaults. They do not introduce new user-tunable configuration keys in this phase. When user customization is justified in a later phase, expose package-prefixed options through the documented `clay.configuration.setPackageOption` API and layout defaults through `clay.ui.serverSetLayoutOverride`. Do not invent hidden JSON/TOML keys or undocumented config paths in `init.js`.

### Configuration contract for language packages

Language packages in Phase 18.14 ship opinionated defaults and do not expose per-language tuning knobs. The authoring contract is:

- Defaults (indent size, line comment token, delimiter pairs, electric outdent, autocomplete triggers) are declared in the package `editorRules` and validated at load time.
- No new `clay.contributions.packageOptions` entries are required for Phase 18.14 first-party language packages.
- End-user customization is intentionally deferred; users who need different defaults should wait for a documented `setPackageOption` option or override behavior at the workspace/command layer, not by editing the package manifest.
- When a future phase adds options, they must be package-prefixed (e.g., `rust.indentSize`), declared in `clay.contributions.packageOptions`, and read through the documented configuration API. Until then, `setPackageOption` with ad hoc language-package keys is unsupported and will be rejected by validation.

### Rejected shapes

Do not:

- Add `if mode == "rust"` or `if extension == "ts"` branches in the Rust client or server core.
- Implement language-specific native widgets, status bars, sidebars, or overlays.
- Run package JavaScript in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers.
- Bypass `loadPackage` by pasting the package manifest into `init.js`.
- Promote LSP, full language-server protocol integration, workspace-wide symbol indexes, AI completions, network-backed completions, or mutating package-manager/toolchain execution in Phase 18.14.

### UI/layout authoring contract for language packages

Language packages may contribute UI, but the package author API is limited to validated, inert declarations. The Rust client owns the working area, pane/split tree, fixed slots (`left`, `right`, `top`, `bottom`), the mandatory `main` editor slot, the component catalog, theme token resolution, and transient overlay rendering. Packages declare contributions and command/action targets; Clay composes them.

Allowed `clay:ui` surfaces for language packages in Phase 18.14:

- **Status items** (`serverRegisterComponentContribution` with `kind: "statusItem"`): lightweight, read-only labels or indicators that read package/component state. Example: a `rust.status.mode` label that shows the active mode name.
- **Transient overlays** (`serverRegisterTransientOverlayContribution`): dismissible panels anchored to the working area, active pane, main slot, or pointer. Use for transient language actions such as a quick-help overlay; default focus/dismissal policies keep them out of the editor hot path.
- **Theme tokens** (`serverRegisterThemeToken`): package-prefixed semantic tokens with a Clay core fallback of the same type. The ten typed domains are `color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, and `density` (Phase 20.1 extended the catalog additively from the original five).
- **Input/state scopes** (`serverRegisterInputContribution`, `serverRegisterUiStateScope`): declare bounded input interests and durable state schemas when a language package owns a panel or overlay.
- **Fixed panels** (`serverRegisterPanelContribution`): only when the language package genuinely owns persistent auxiliary UI. Fixed panels default to `defaultVisibility: "hidden"`; packages must not open a visible panel from `loadEntry` without explicit user opt-in through configuration.

Packages must not:

- Create or mutate Masonry widgets directly.
- Provide raw CSS strings, HTML, renderer callbacks, native handles, or client-side JavaScript hooks.
- Add fixed panels that consume permanent chrome by default.
- Add file-browser roots, workspace markers, ignore rules, or raw directory listings.
- Use `Deno.core.ops` directly or bypass `clay:ui` APIs.

Example status-item contribution from a language package `loadEntry`:

```js
import { serverRegisterComponentContribution } from "clay:ui";

await serverRegisterComponentContribution(rustPackageManifest(), {
  kind: "statusItem",
  id: "rust.status.mode",
  style: { variant: "muted" },
  children: [{ kind: "label", id: "rust.status.mode.label" }]
});
```

The same declaration must also be listed in `clay.contributions.ui.components` inside the package manifest so Clay can validate provenance, budgets, and duplicate IDs at load time.

Layout overrides and package options are declared through `clay.ui.serverSetLayoutOverride` and `clay.configuration.setPackageOption`, but only after the package has registered the referenced panels, inputs, actions, and tokens. Package defaults never override explicit user configuration.

Shipped first-party language package docs:

- [`@clay/rust`](rust.md)
- [`@clay/typescript`](typescript.md)
- [`@clay/javascript`](javascript.md)

## Phase 18.18 authoring contract: complete first-party language packages

Phase 18.18 combines the already-documented generic primitives into the complete first-party language-package shape. `@clay/rust`, `@clay/typescript`, `@clay/javascript`, and `@clay/markdown` are explicit opt-in packages, not core defaults:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");
```

One `loadPackage` call validates the package, loads its `loadEntry` once per runtime generation, and registers only its declared inert contributions. Do not copy a manifest, call raw ops, or register each primitive from `init.js`. Optional customization belongs to an already-documented Clay API; no per-language option is introduced here.

### Full package contribution shape

Each first-party package owns data and load-time registration, while Clay owns selection, validation, rendering, command execution, completion UI, shell layout, and client input:

| Package | Native grammar + vocabulary styleMap | Major-mode behavior | Commands/completion/status |
| --- | --- | --- | --- |
| `@clay/rust` | `tree-sitter-rust`; code `TokenType` captures | 4-space, `//`, bracket/quote pairs, `}` electric outdent, `.`/`:` triggers | `rust.toggleLineComment`, priority-0 `rust.keywords`, `rust.status.mode` |
| `@clay/typescript` | `tree-sitter-typescript` and TSX descriptors; code captures | 2-space, `//`, bracket/quote/backtick pairs, `}`/`)`/`]` electric outdent | `typescript.toggleLineComment`, priority-0 `typescript.keywords`, `typescript.status.mode` |
| `@clay/javascript` | `tree-sitter-javascript`; code captures | 2-space, `//`, bracket/quote/backtick pairs, `}`/`)`/`]` electric outdent | `javascript.toggleLineComment`, priority-0 `javascript.keywords`, `javascript.status.mode` |
| `@clay/markdown` | `tree-sitter-md-025`; prose `TokenType` + `Modifiers` captures | 2-space list continuation, prose pairs, `#`/`[`/backtick triggers | server-first Markdown commands, priority-0 `markdown.keywords`, `markdown.status.mode` |

A language `loadEntry` calls the same generic facades: `serverRegisterSyntaxGrammar`, `serverRegisterModePattern` with `buildCodeEditingManifest`, `serverRegisterCommand`, `serverRegisterCompletionProvider`, and `serverRegisterComponentContribution`. Completion `items` are bounded, static text replacements at priority 0; they are not snippets or callback providers. Phase 18.19 owns snippet transforms, exclusive claims, and native-provider suppression.

Vocabulary styleMap entries map grammar captures directly to closed `TokenType`/`Modifiers` values; see [Clay Text Vocabulary and Two-Axis Decoration Contract](../primitives/syntax-vocabulary.md#package-stylemap-authoring). Grammar selection remains independent from major-mode selection. Tier 1 native is the normal first-party engine, Tier 2 requires an explicitly selected package-confined WASM artifact, and Tier 3 is an explicit JavaScript fallback. Arbitrary third-party grammar/native loading and LSP process authority remain deferred pending their own security decisions.

### Markdown decoration and preview are separate

Markdown uses the same package contract but has two intentionally independent outputs:

```text
native tree-sitter-md-025 + queries/highlights.scm
  -> bounded vocabulary DecorationSet
packages/markdown/dist/sdui.js
  -> optional validated preview/status SduiSnapshot
```

The default decoration path is Tier 1 native. `parser.js` is registered only as the Tier 3 JavaScript fallback and is selected only when native syntax is unavailable or the user explicitly selects `setSyntaxEnginePreference("markdown", "javascript")`. The package manifest has no default parser-backed `decorations` contribution. The preview remains package-JS SDUI, is optional, and does not create a fixed panel, Masonry widget, raw CSS, client-side JavaScript, or a native layout mutation. It may publish a bounded status/preview snapshot only through the documented SDUI contract.

### UI/layout, performance, and authority rules

Status items and optional SDUI preview are inert `clay:ui`/`clay:sdui` contributions. Fixed panels consume a declared slot; transient overlays do not. Clay owns slot composition, action routing, focus, user-over-package precedence, typed style tokens, and Masonry rendering. A package cannot win a slot by load order and cannot use a status item or SDUI preview to bypass fixed-versus-transient rules.

All package JavaScript runs at load, explicit command, explicit SDUI update, or bounded server parse/completion work—not in client keypress, paint, layout, scroll, pointer, or text-event hot paths. Behavior manifests, parse windows, decoration payloads, SDUI snapshots, completion items, and syntax cache retention remain subject to their typed budgets. Contributions preserve package provenance and require declared permissions; `loadPackage` grants no capabilities. Packages receive no filesystem, network, shell, AI, raw-op, native-widget, client-runtime, arbitrary WASM, third-party grammar, or LSP authority from this contract.

## Extending and Replacing Packages (Plan 061)

Clay runs exactly two JavaScript runtime trust domains: a trusted runtime for
configuration and integrity-verified bundled packages, and one shared
third-party runtime. Package code never imports another package's
implementation modules and never sees raw ops; all cross-package composition
flows through inert manifest metadata plus the public `clay:*` registration
APIs. The canonical schemas and rejection rules live in
`docs/reference/primitives/package-security.md#package-runtime-trust-domains-and-extension-authority`.

### Declaring extension points (target/owner side)

A package that allows others to build on it declares `clay.extensionPoints`
(`clay-extension-point-v1`) in its manifest:

```json
"extensionPoints": [
  {
    "id": "myprefix.completionProviders",
    "version": 1,
    "operations": ["append", "replace"],
    "contributionKinds": ["completionProvider"],
    "scopes": ["myprefix.keywords"],
    "summary": "Add or replace completion providers."
  }
]
```

Rules:

- `id` is `apiPrefix.name`; the prefix segment must equal the package's own
  `apiPrefix`. Bump `version` on any incompatible change.
- `scopes` names the exact package-prefixed contribution IDs a relation may
  mutate (or a single trailing `myprefix.*` wildcard). Every scope must name a
  real contribution of the package.
- Declarations are inert metadata; they grant nothing by themselves.

### Requesting a relation (requester side)

A package building on another declares structured relation entries
(`clay-package-relation-v1`) in `clay.extends` (or `imports`/`overrides`):

```json
"extends": [
  {
    "package": "@clay/markdown",
    "extensionPoint": "markdown.completionProviders",
    "version": 1,
    "operation": "append",
    "scopes": [],
    "justification": "Wikilink completions for Markdown."
  }
]
```

The requesting package then registers its own provider through the normal
public API — it never touches the target's JavaScript:

```javascript
await serverRegisterCompletionProvider({ id: "myprefix.wikilinks", modes: ["markdown"], ... });
```

Enabling a third-party package with structured relations requires an exact
durable user approval (`clay-package-approval-v1`) covering identity,
capabilities, processes, and every requested relation. Version/integrity
drift, scope expansion, or target replacement invalidates the approval;
narrowing requires re-approval. Trusted bundled packages skip the durable
approval requirement but still require the target's declared extension point.

### Replacing a package

Whole-package semantic rewrites use `replaces` plus a user-approved
replacement record (`clay-package-replacement-v1`) instead of extension
points. Replacement is host-owned and atomic, preserves replacement
provenance, and never grants the target's trusted runtime placement, identity,
or language-server grants.

### What users see at adoption

Installing a package never executes it. The first load of a third-party
package opens a host-rendered authority overlay (native catalog components
only — package code does not run and cannot render the prompt) showing
identity/provenance, the shared third-party runtime disclosure, requested
capabilities, external processes, dependency and mutation scopes, and any
withdrawn contributions for replacements. The package executes only after an
exact approval; rejection leaves it installed-but-disabled and re-prompts on
the next load. Approvals go stale on version/integrity drift or target
replacement and must be re-approved; expanded authority is shown as a diff.
Adopted third-party packages execute in the shared third-party runtime,
never in the trusted configuration runtime: `loadPackage` of a third-party
spec routes its load entry through a host bridge into that runtime, a
configuration reload leaves third-party providers and language-server
sessions running untouched, and a crashed/timed-out third-party runtime is
replaced once with only the current approved graph replayed.

Users can inspect, disable, revoke, and roll back at any time from the same
host UI. The CLI exposes the same single authority path:
`clay package inspect <name>` (shows adoption state),
`clay package adopt <name>` (writes the durable exact approval),
`clay package revoke <name>` (revokes approval and disables the package),
`clay package rollback <name>` (disables the active replacement of the named
target, re-adopts the target if third-party, and re-enables it). A committed
replacement revokes the replaced target's durable approval, so restoration is
always this explicit action — never a silent reuse of the old approval — and
dependency edges cannot silently re-enable a replaced target
(`clay.package_replacement.target_replaced`).
The full interaction contract (states, keyboard/focus, motion,
layout) lives in
`docs/wiki/modules/first-party-package-extension-api-review.md#adoption-and-replacement-interaction-contract-plan-061-task-9`.

### `packages/lsp-shared`

`packages/lsp-shared` is a private pure-JavaScript helper shared by the
bundled LSP bridge packages. Decision (Plan 061 task 8): it stays private.
Third-party language-server bridges use the public `clay:language-server`
session APIs with their own approved fixed contribution; they do not import
`lsp-shared` or any other package's modules.

## Minimal Package Checklist

For a simple package:

- [ ] `package.json` has name/version/type/exports.
- [ ] `clay.apiPrefix` is present and package-owned.
- [ ] `clay.entry` and `clay.loadEntry` are present.
- [ ] Permissions match contributions.
- [ ] Docs path exists.
- [ ] Performance metadata exists.
- [ ] Public IDs use the package prefix.
- [ ] Default load path is documented.
- [ ] Optional configuration is documented through Clay JS APIs.
- [ ] Commands/actions are registered before UI targets them.
- [ ] UI contributions are inert and slot-aware when shell APIs are available.
- [ ] Command action intents are inert and transient menu items are bounded; no callbacks or client-side handlers.
- [ ] Package UI coexists with Clay-owned file browser chrome and does not add roots, markers, ignore rules, raw listings, or file-browser-specific native widgets.
- [ ] Package UI coexists with Clay-owned multi-document switcher, dirty/save status chrome, and recovery menus; it does not own tabs, native save dialogs, or reconnect/resync loops.
- [ ] Style uses tokens/typed variables, not CSS.
- [ ] Tests cover validation, loading, runtime behavior, and docs.

## Anti-Patterns

Do not:

- Ask users to paste full package manifests into `init.js` for normal setup.
- Use unprefixed command IDs.
- Claim `clay.*` package IDs.
- Call raw `Deno.core.ops`.
- Execute package JavaScript in the Rust client.
- Create or mutate Masonry widgets directly from package code.
- Provide CSS, HTML, script, draw callbacks, or native handles.
- Do filesystem/network/shell/AI/WASM work without an approved permissioned API.
- Add Markdown-specific Rust UI/layout branches for package behavior.
- Publish a default fixed panel from the package load path; optional panels should start with `defaultVisibility: "hidden"`.
- Use the SDUI `publishTree` left-slot surface as a user-facing panel authoring pattern; it is a Clay-owned internal retained sidebar, not a package slot API.
- Hard-code a side panel position or width; let the shell compose `PaneSlotLayout` from the declared `slot` and user overrides.
- Treat smoke fixtures as user-facing setup instructions.
- Treat planned working-area/split-tree/slot-layout/state/override `clay:ui` snippets or planned configuration helpers as callable runtime code before public API docs, docs-index links, generated registry entries, and backing ops ship. In current Phase 18.4 wording, this means planned working-area/split-tree/direct pane-slot/state-value mutation snippets, package enable/disable helpers, or any undocumented configuration helper.
- Execute commands from UI callbacks or transient menu items without routing through the server-owned `CommandExecution` path.
- Bypass command permission/provenance validation from package code.
- Treat a transient menu session as a fixed bottom panel or as a generic `TransientOverlayContribution` that owns dynamic query state.
- Treat the Clay-owned file browser as a package-owned panel, package workspace-root provider, package marker/ignore-rule extension point, raw directory-listing API, or custom Masonry widget.
- Treat multi-document sessions, dirty/save status chrome, conflict recovery menus, or pending-edit/disconnect/resync recovery as package-owned layout surfaces, native widgets, or clipboard/filesystem authority grants.
- Open native save dialogs, write arbitrary files, invent package clipboard-contents APIs, or run reconnect/resync loops from package UI in place of Clay's documented command IDs and recovery menus.
- Pass raw client-chosen filesystem paths through package UI actions instead of using documented workspace root/grant command APIs.
- Treat `serverLoadPackage` as ordinary end-user package installation, enablement, or execution authority.

## Example: Markdown as a Package

**Implemented/runtime-backed default user setup**:

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

**Implemented/package-owned fallback alias** (Phase 18.5, retained after `loadPackage` shipped):

```js
import { markdownLoadMode } from "@clay/markdown";
import { bindKey } from "clay:keybindings";

await markdownLoadMode();
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

Target Markdown package behavior (Phase 18.5 authoring contract):

- registers `markdown` major mode
- matches `.md`, `.markdown`, `.mdown`, and `text/markdown`
- registers Markdown parser/decorator handler
- publishes syntax decorations with style tokens
- registers package commands such as `markdown.togglePreview`
- defaults to editor-only `main` layout; the editor occupies `PaneSlotLayout.main`
- does not publish a default fixed panel on load
- offers preview/status as an optional `PanelContribution` targeting the `right` slot with `defaultVisibility: "hidden"`
- user can enable preview through `setPackageOption` or `serverSetLayoutOverride`
- preview panel styling uses `PackageThemeTokenDeclaration` with same-type core fallbacks
- consumes only generic shell/layout/UI/configuration primitives; no Markdown-specific Rust branches

Current Markdown smoke/configuration fixtures may still validate package metadata, parse/decorations, and inert SDUI preview/status publication. Those fixtures are validation tools, not the long-term user setup or shell/layout authoring convention.

## Keeping This Guide Current

This guide is part of the package contract. Every phase that adds or changes package loading, shell layout, components, actions, input, state, configuration, styling, permissions, docs, or testing must update this file.

Roadmap phases 18.1 through 18.5 explicitly require iterative updates here while Clay moves from the current SDUI/package foundation to the Clay-owned shell/layout package model.
