# Creating Clay Packages

This guide explains how to design a Clay package and how packages are expected to work with Clay's editor, shell, UI, input, actions, logic, data, configuration, and theme systems.

Clay package APIs are evolving. This document intentionally distinguishes **current implemented public behavior**, **Phase 18.2 internal shell runtime behavior**, **Phase 18.3 runtime-backed slot UI contribution behavior**, and **planned package-facing shell/layout/configuration behavior** so package authors and phase plans can update it iteratively as Clay's package architecture lands.

## Goals

A Clay package should be easy for users to load and safe for Clay to run:

**Planned/target default user load** (not a callable Phase 18.2 runtime API until a later package-loading phase documents it):

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
```

The one-line load path is the preferred default when Clay has the necessary generic primitives. Packages may expose optional customization APIs, but ordinary users should not have to copy package manifests, manually register every primitive, or paste smoke-fixture scripts into `~/.config/clay/init.js`.

Current implemented package API status: `clay.packages.serverLoadPackage` / `serverLoadPackage(packageJson)` validates a package record and returns inert metadata. It is not an end-user install, enable/disable, package-manager, or package-code execution wrapper.

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
| Loading | Default setup and optional customization | Planned target `loadPackage("@clay/markdown")`; current validation helper `serverLoadPackage(packageJson)` |
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
- Decoration publication and parse handler foundations.
- First-party `@clay/markdown` package scaffold and smoke fixtures.

### Phase 18.2 shell/layout runtime and Phase 18.3 slot-aware package UI

The canonical Phase 18.1/18.2 shell/layout architecture reference is [Clay Shell and Package UI/Layout Strategy](../primitives/shell-layout-strategy.md). This guide summarizes the author-facing contract; the primitive reference owns the detailed vocabulary, Masonry boundary, validation expectations, internal runtime status, and planned primitive names.

Phase 18.2 has implemented internally:

- Clay-owned `ClayShellWidget` root above `EditorWidget`.
- Internal `WorkingAreaLayout` state for one working area, layout version, active/root pane, and editor component binding.
- Internal `PaneSplitTree` state for the one-leaf default plus generic horizontal/vertical split topology with bounded split ratios and deterministic validation.
- Internal `PaneSlotLayout` state with mandatory `main` plus optional fixed `left`, `right`, `top`, and `bottom` slots, including finite sizing, min/max clamp, visibility, collapse, and user-resize fields.
- An internal SDUI left-slot compatibility bridge and structural shell observability that omit document text, native handles, raw action authority, raw CSS, raw ops, renderer callbacks, and executable package code.

Phase 18.3 now adds runtime-backed public APIs for package-owned slot UI contributions:

- `clay:ui` facade imports are available in the server-side package runtime through `runtime/js/ui.ts` and `src/server/js_runtime.rs`.
- `serverRegisterPanelContribution(manifest, declaration)` validates a fixed `PanelContribution` targeting `left`, `right`, `top`, or `bottom` slots and stores package provenance.
- `serverRegisterComponentContribution(manifest, declaration)` validates a bounded Clay component tree/catalog contribution.
- `serverRegisterTransientOverlayContribution(manifest, declaration)` validates an overlay/menu/dialog-like transient contribution with anchor, focus, and dismissal policies.
- `serverRegisterThemeToken(manifest, declaration)` validates package-prefixed typed theme tokens with same-type Clay core fallbacks.
- Package metadata validation accepts `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, and `themeTokens` descriptors for load-time diagnostics/conflicts.
- Runtime composition maps accepted fixed panels to Clay-owned `PaneSlotLayout` state and transient overlays to a separate overlay layer; the editor remains in the mandatory `main` slot.

Still planned for package authors:

- Public callable working-area, pane-split, and pane-slot layout mutation/default APIs.
- Package state/data scopes.
- User layout/style/input/theme overrides through documented configuration APIs.
- Updated Markdown package defaults after these package-facing foundations are consumed by first-party packages.
- A future one-line end-user package load wrapper such as `loadPackage("@clay/markdown")` once package spec resolution, install/enable/load-entry authority, and `init.js` package-service state recording are implemented and documented.

Expected shell/layout/package guide updates by phase:

| Phase | Authoring-contract update expected |
| --- | --- |
| Phase 18.1 | Architecture vocabulary, Masonry boundary, status markers, planned API inventory, conflicts/precedence, and anti-patterns documented here and in the primitive reference. |
| Phase 18.2 | Document implemented internal shell root, `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` runtime behavior while keeping public `clay:ui` package APIs marked planned/unavailable. |
| Phase 18.3 | Document runtime-backed public APIs for slot-aware `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` registration, examples, diagnostics, package metadata, package permissions, and generated registry/API coverage. |
| Phase 18.4 | Document implemented `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, and package option customization APIs; verify the one-line `loadPackage("@clay/markdown")` default loader remains a planned generic package-service gap rather than a shipped end-user API. |
| Phase 18.5 | Document the Phase 18.5 authoring contract: no default fixed panel unless explicitly registered, optional preview as `PanelContribution` with `defaultVisibility: "hidden"`, main editor placement via `PaneSlotLayout.main`, theme token usage, and `setPackageOption`/`serverSetLayoutOverride` customization. Update Markdown package docs to consume generic shell/layout primitives and remove fixture-only UI guidance from user-facing defaults. |

Phase 18.3 `clay:ui` contribution examples for panels, components, overlays, and theme tokens are runtime-backed public APIs. Historical Phase 18.3 status used the row `PackageLayoutOverride` | `clay.ui.serverSetLayoutOverride` | Planned for documented user/package layout overrides.; Phase 18.4 promotes that surface. Phase 18.4 `serverRegisterInputContribution`, `serverRegisterUiStateScope`, `serverSetLayoutOverride`, and `setPackageOption` examples are also runtime-backed public APIs. Examples for working-area layout, pane splits, pane-slot mutation, durable state-value mutation, package enable/disable from configuration, and the future `loadPackage("@clay/markdown")` one-line loader remain **Planned/target** design, not callable code. The Phase 18.2/18.3 Rust shell runtime shapes are not package author APIs.

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

Phase 18.3 `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration` declarations require no new permission when they are inert metadata. Their embedded action targets inherit the target command's registration/permission requirements, and future user overrides require documented configuration APIs and `package-configuration` where behavior-changing.

A permission declaration does not grant broad authority. Packages still cannot access arbitrary filesystem paths, network, shell, AI mutation, WASM, native widget handles, raw Deno ops, or client-side JavaScript by default.

## Loading Packages from init.js

Package loading status:

- **Planned/target end-user default:** users explicitly load packages from `~/.config/clay/init.js` with a one-line helper once package spec resolution, package-service install/enable/load-entry authority, and activation state recording are implemented and documented.
- **Implemented/runtime-backed today:** `serverLoadPackage(packageJson)` validates Clay package metadata and returns inert summary metadata. It does not install a package, enable/disable a package, run package-manager work, execute a package `loadEntry`, or execute package JavaScript.
- **Phase 18.4 customization status:** optional customization after the future one-line load uses documented `setPackageOption` and `serverSetLayoutOverride` APIs. These are startup/configuration-change/package-load/update-time validators, not hidden JSON/TOML/ad hoc keys and not package enable/disable authority.

Planned preferred default form:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
```

Implemented package-record validation helper:

```js
import { serverLoadPackage } from "clay:packages";

const loaded = serverLoadPackage(packageJson);
```

Do not present `serverLoadPackage` as ordinary end-user setup. It is useful for controlled package/configuration fixtures and load-contract validation, not for package installation or enablement; any `serverLoadPackage(packageJson)` plus package-owned helper flow is a temporary validation/loading gap, not the preferred convention.

Optional user configuration should be separate and explicit after the one-line package load helper exists:

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

Phase 18.4 verifies that the generic one-line loader is not implemented yet: `runtime/js/packages.ts` and the embedded `clay:packages` facade export validation helpers but not `loadPackage`, and there is no public registry entry for `clay.packages.loadPackage`. The generic loader/API gap is a Clay package-service bridge that can resolve an installed package specifier, enable the package, execute/import its declared `loadEntry`, and record activation from `init.js` without granting package-manager, filesystem, network, shell, AI, WASM, raw-op, native-widget, or client-JS authority.

Phase 18.5 (`plans/028` Task 4) investigated that bridge and deferred it with a decision-log-backed rationale: the controlled server-side runtime is deny-by-default (`src/server/js_runtime.rs::ClayModuleLoader`) and confines loadable modules to the configuration root (`src/server/configuration.rs::canonical_local_file`), so a working `loadPackage("@clay/*")` requires a security-critical module-loader extension plus a `PackageService` resolve/enable/execute path and a new op. That authority expansion warrants its own focused phase; see `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`. The documented temporary fallback for first-party packages is the package-owned default entry that imports the Clay facades directly. For Markdown that entry is `markdownLoadMode()` (in `packages/markdown/dist/load.js`, re-exported from `./dist/index.js`), which consumes only implemented generic primitives and contains no copied manifest:

If a package cannot yet support one-line loading because Clay lacks a generic primitive, document the longer setup as a temporary limitation rather than the preferred path.

**Implemented/package-owned temporary fallback** (Phase 18.5, until the generic `loadPackage` resolver ships):

```js
import { markdownLoadMode } from "@clay/markdown";

await markdownLoadMode();
```

This fallback imports the Clay facades internally and reuses the package's own default setup; it does not paste a manifest into `init.js`. It becomes end-to-end callable once the constrained first-party module-loader bridge lands; until then, deterministic smoke/configuration fixtures continue to validate the package through `serverLoadPackage(packageJson)` plus the same package-owned helpers.

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

**Planned/target default loader shape** for a package load entry after the one-line load contract is implemented:

```js
import { serverRegisterCommand } from "clay:commands";
import { serverRegisterModePattern, serverActivateMajorMode } from "clay:modes";
import { serverRegisterParseHandler } from "clay:parse";
import { parseMarkdownDecorations } from "./parser.js";

export async function markdownLoadPackage(options = {}) {
  serverRegisterCommand({
    id: "markdown.togglePreview",
    label: "Toggle Markdown Preview",
    routing: "ServerFirst",
  });

  serverRegisterModePattern({
    mode: "markdown",
    extensions: ["md", "markdown", "mdown"],
    mimeTypes: ["text/markdown"],
  });

  serverRegisterParseHandler({
    mode: "markdown",
    parse: parseMarkdownDecorations,
  });

  if (options.activateDocumentId) {
    await serverActivateMajorMode(options.activateDocumentId, "markdown");
  }
}
```

The exact implemented API names may differ by phase. Keep package docs current with the implemented Clay JS API reference.

## UI and Layout Model

Clay owns a consistent shell layout for all packages and modes. See the [Clay Shell and Package UI/Layout Strategy](../primitives/shell-layout-strategy.md) for the canonical Phase 18.1/18.2 vocabulary/runtime status and for the rule that Masonry is Clay's internal widget/layout/rendering substrate, not a package author API.

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
| `PaneSplitTree` | `clay.ui.serverRegisterPaneSplitTree` | Internal Rust runtime implemented; public callable split-tree API planned/unavailable. |
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

Phase 18.3 component catalog status:

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
| `table` | Planned/deferred | Structured rows/columns in a later component-catalog phase. |
| `dropdown` | Planned/deferred | Selection/action menu in a later component-catalog phase. |
| `collapse` | Planned/deferred | Expand/collapse sections in a later component-catalog phase. |
| `modal` | Planned/deferred | Shell-owned transient dialog in a later component-catalog phase. |

Packages should not assume these are Masonry widget types. They are Clay components validated by `src/shell/components.rs` and rendered through Clay-owned native code.

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

## Rendering and Decorations

Inline editor rendering uses inert decoration data, not UI components.

Decoration span example:

```js
{
  byteStart: 0,
  byteEnd: 7,
  kind: "syntax",
  styleToken: "markup.heading.1",
  priority: 80
}
```

Packages should translate parser-specific output into generic Clay decoration spans. Rust should not branch on Markdown-specific token names.

## Phase 18.5 authoring contract: no-default-panel, optional preview, generic primitive consumption

Phase 18.5 replanned Markdown end-user loading around the generic shell/package primitives promoted in Phases 18.1–18.4. The key authoring contract changes are:

1. **No default fixed panel.** Packages do not publish a default fixed panel on load. A package that offers a preview, status, or auxiliary panel registers it as a `PanelContribution` with `defaultVisibility: "hidden"`. The panel appears only when the user explicitly enables it through `setPackageOption`, `serverSetLayoutOverride`, or a package command.

2. **Main editor placement via `PaneSlotLayout.main`.** The editor always occupies the mandatory `main` slot. Packages do not need to request or register this; Clay places the active editor in `main` by default. Package-owned panels target optional fixed slots (`left`, `right`, `top`, `bottom`).

3. **Optional preview as a `PanelContribution`.** A package preview panel is a `PanelContribution` targeting a Clay slot (e.g., `right`) with `defaultVisibility: "hidden"`. The package registers the contribution at load time; the shell validates it, composes it into `PaneSlotLayout`, and keeps it hidden until a user or package action changes visibility.

4. **Theme token usage for panel styling.** Preview and panel styles use `PackageThemeTokenDeclaration` with same-type core fallbacks (e.g., `markdown.preview.background` → `surface.panel`). Raw CSS, raw colors, and renderer callbacks remain prohibited.

5. **`setPackageOption` and `serverSetLayoutOverride` for customization.** User configuration changes preview visibility, slot, split ratio, or theme token mapping through documented Clay JS APIs, not through hidden JSON/TOML/ad hoc keys.

6. **Package-owned fallback entry while `loadPackage` is deferred.** The generic `loadPackage("@clay/markdown")` one-line resolver is deferred (see `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`). The package-owned `markdownLoadMode()` entry consumes implemented generic primitives internally and is the documented temporary fallback.

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
- Do not use the SDUI `publishTree` left-slot bridge as a user-facing panel authoring pattern; it is a Clay-owned internal compatibility bridge.
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
3. **Runtime loader tests** — implemented validation helpers (`serverLoadPackage`) stay separate from planned one-line end-user load helpers (`loadPackage`), docs identify the current generic loader/API gap, customization uses `setPackageOption` / `serverSetLayoutOverride`, and the one-line load path registers defaults once it exists.
4. **Mode tests** — classification, activation, behavior manifest composition.
5. **Input tests** — key routing, command routing, mouse/component actions.
6. **UI tests** — slot placement, fixed/transient panel behavior, overlay geometry, action validation, and observability privacy.
7. **Theme/style tests** — token validation, same-type fallback mapping, typed style variables, and raw CSS/color rejection.
8. **Package metadata tests** — `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, `themeTokens`, duplicate fixed slot claims, and bounded payload diagnostics.
9. **Parse/render tests** — bounded snapshots, stale result rejection, decoration payload budgets.
10. **Docs tests** — package docs, primitive docs, and master index links stay current.
11. **Manual smoke tests** — actual GUI package loading and user workflow checks.

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
- Use the SDUI `publishTree` left-slot bridge as a user-facing panel authoring pattern; it is a Clay-owned internal compatibility bridge.
- Hard-code a side panel position or width; let the shell compose `PaneSlotLayout` from the declared `slot` and user overrides.
- Treat smoke fixtures as user-facing setup instructions.
- Treat planned working-area/split-tree/slot-layout/state/override `clay:ui` snippets or planned configuration helpers as callable runtime code before public API docs, docs-index links, generated registry entries, and backing ops ship. In current Phase 18.4 wording, this means planned working-area/split-tree/direct pane-slot/state-value mutation snippets, package enable/disable helpers, or any undocumented configuration helper.
- Treat `serverLoadPackage` as ordinary end-user package installation, enablement, or execution authority.

## Example: Markdown as a Package

**Planned/target default user setup** after the one-line package load helper exists:

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

**Implemented/package-owned temporary fallback** (Phase 18.5, until `loadPackage` resolver ships):

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
