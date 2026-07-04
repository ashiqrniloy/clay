# Creating Clay Packages

This guide explains how to design a Clay package and how packages are expected to work with Clay's editor, shell, UI, input, actions, logic, data, configuration, and theme systems.

Clay package APIs are evolving. This document intentionally distinguishes **current implemented public behavior**, **Phase 18.2 internal shell runtime behavior**, **Phase 18.3 runtime-backed slot UI contribution behavior**, and **planned package-facing shell/layout/configuration behavior** so package authors and phase plans can update it iteratively as Clay's package architecture lands.

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

Phase 18.3 `clay:ui` contribution examples for panels, components, overlays, and theme tokens are runtime-backed public APIs. Historical Phase 18.3 status used the row `PackageLayoutOverride` | `clay.ui.serverSetLayoutOverride` | Planned for documented user/package layout overrides.; Phase 18.4 promotes that surface. Phase 18.6/18.7 promote the `loadPackage("@clay/markdown")` default, persistent-runtime mode/parse registration, and generic selected-file open-time activation. Plan 035 generalizes `loadPackage` to installed, authorized source-aware packages. Examples for working-area layout, pane splits, pane-slot mutation, durable state-value mutation, package enable/disable from configuration, and hot reload remain **Planned/target** design, not callable code. The Phase 18.2/18.3 Rust shell runtime shapes are not package author APIs.

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

`@clay/*` only means a package was shipped with Clay — it is not a more capable package. After a user installs and authorizes an npm, GitHub, git-URL, tarball, or local-path package, that package loads through the identical `loadPackage` one-line path, the identical resolver + `PackageService` validation, and the identical runtime authority model. `init.js` itself grants no capabilities: it only requests one-line package loads and (optionally) separate documented Clay APIs. Every powerful capability (filesystem, network, shell, AI, WASM, raw-ops, native-ui, client-runtime, package-control) must be a separately implemented, user-approved authorization grant recorded against the package identity/source/provenance — `init.js` cannot silently grant them.

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
- Packages do not gain filesystem, network, shell, AI, WASM, raw-op, native-widget, package-manager, package-control, or client-JS authority merely through loading, activation, UI contribution registration, or parsing; those capabilities require separate implementation and user approval.

Forbidden anti-patterns:

- Per-open runtimes or per-open `dist/` copies.
- Executable `handler`, `callback`, `onParse`, or `function` fields in the public parse registration payload.
- Raw `Deno.core.ops` calls as package/user-facing API.
- Markdown-only Rust branches such as `if path.ends_with(".md")`, `if mode_id == "markdown"`, or handwritten markdown-it token handling in server/client Rust.
- Publishing representative/fake decorations from `init.js` instead of returning an `IncrementalParseUpdate` from the package parse handler.
- Client-side JavaScript, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, or layout mutation hidden inside package UI/layout declarations.

## UI and Layout Model

Clay owns a consistent shell layout for all packages and modes. See the [Clay Shell and Package UI/Layout Strategy](../primitives/shell-layout-strategy.md) for the canonical Phase 18.1/18.2 vocabulary/runtime status and for the rule that Masonry is Clay's internal widget/layout/rendering substrate, not a package author API.

### Unified UI/layout authoring contract across package sources

The UI/layout authoring contract is identical for `@clay/*` packages and user-installed packages (npm, GitHub, git URL, tarball, local path). `@clay/*` only means a package was shipped by Clay — it is not a more capable package. After a user installs and authorizes a package from any source, it contributes UI/layout through the same `clay:ui` facades, the same `PackageService` validation, the same shell/slot/precedence rules, and the same conflict-resolution policy as a bundled Clay package.

- User-installed packages may request the same UI/layout/native/client capabilities as Clay packages — `render-decorations`, `render-folding`, `completion-provider`, `package-configuration`, `package-control`, `native-ui`, and `client-runtime` — through the unified capability vocabulary, subject to the explicit user authorization grants described in [Unified Package Capability Model](../primitives/package-security.md#unified-package-capability-model). A package source never confers a capability implicitly; every powerful capability must be a separately implemented, user-approved grant recorded against the package identity, source, and provenance.
- Native UI and client runtime are explicit capability/API work. A package does not get native widget handles, Masonry mutation, raw CSS, client-side JavaScript, or renderer callbacks merely because it was installed from npm or GitHub — those surfaces appear only when a documented `native-ui` / `client-runtime` capability is granted and a matching Clay API exists, is validated, and is revocable.
- UI/layout declarations remain validated load/reload/configuration work. Panel, component, overlay, input, state-scope, layout-override, theme-token, and option contributions are validated at package load/enable time and applied through documented Clay JS APIs at configuration/package-update time; no package JavaScript runs in Masonry paint, layout, pointer, scroll, keypress, text-event, or edit-ack handlers.
- UI/layout primitives stay generic and reusable. No UI/layout primitive branches on package source (no `if github_package` / `if npm_package` / `if third_party` Rust paths). Every package consumes the same shell/slot/component/theme primitives; Markdown and future modes consume these generic primitives rather than adding mode-specific Rust layout branches.

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

Phase 18.8 adds a server-owned `CommandExecution` boundary. SDUI actions, package UI action intents, behavior-manifest keybindings, and transient-menu selections all normalize to the same `CommandExecutionRequest`. The server validates command ID, routing policy, package provenance, declared permissions, target context, and bounded arguments before any side effect. Packages may declare commands and expose them in transient menus; they cannot execute commands directly from UI callbacks, bypass permission checks, or run command handlers in the Rust client.

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
    electricCharacters: [ { trigger: "}", effect: "outdent-one-level" } ]
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
- `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES = 2048` — behavior manifest payload budget; oversize manifests are rejected with `PayloadBudgetExceeded` at record time.

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

## Phase 18.10 authoring contract: grammar-only syntax packages

Phase 18.10 adds `SyntaxGrammarContribution` metadata for grammar-only language packages. A grammar-only package highlights documents whose active major mode may still be `core.code` or `core.text`; it does **not** register a full major mode, commands, completions, UI, key behavior, or language-specific Rust branches.

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
        "grammar": { "kind": "tree-sitter-wasm", "path": "./grammars/rust.wasm" },
        "queries": { "highlights": "./queries/highlights.scm" },
        "styleMap": {
          "keyword": "keyword.control",
          "string": "string.quoted",
          "comment": "comment.line",
          "punctuation": "punctuation.definition"
        },
        "budgets": { "timeoutMs": 5000, "maxWindowBytes": 4096 }
      }]
    }
  }
}
```

Validation is load-time only and reuses the package metadata budget. Phase 18.10 accepts grammar contributions from first-party `@clay/*` packages only; arbitrary third-party/native grammar artifact loading is out of scope. Grammar/query paths must be package-root-confined relative `./` asset paths; grammar artifacts must be `tree-sitter-wasm`, query files must be `.scm`, style-map values must be known Clay style tokens, and packages must declare both `parse-document` and `render-decorations`. Clay rejects non-`@clay/*` grammar packages, absolute paths, parent traversal, URLs/downloads, native libraries, package-manager/shell fields, raw ops, client JavaScript, CSS/raw colors, duplicate language IDs, and duplicate file-pattern claims. Parse/highlight work runs as `Background`, cancellable, viewport-prioritized server work bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`; it never runs in keypress, paint, layout, scroll, pointer, or text-event hot paths. First-party grammar packages are loaded explicitly from `~/.config/clay/init.js`; they are not auto-loaded.

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
```

Do not add hidden JSON/TOML/ad hoc syntax configuration keys for preferred grammar selection, grammar paths, style maps, capture styles, or auto-load behavior. If a later phase exposes any of those as user preferences, they must be promoted as documented Clay JS APIs with custom properties and registry coverage.

Shipped Phase 18.10 grammar-only packages are documented at:

- [`@clay/rust`](rust.md)
- [`@clay/typescript`](typescript.md)
- [`@clay/javascript`](javascript.md)

## Phase 18.11 authoring contract: completion providers

Phase 18.11 adds the `CompletionTriggerAndResult` primitive and a server-side completion provider framework. Completion providers are **metadata-only** in Phase 18.11: a package declares provider metadata and trigger/word-boundary parameters, and Clay owns trigger classification, result computation scheduling, and the completion picker UI. Package authors do **not** ship an executable completion handler, raw callback, raw op, native handle, client JavaScript, snippet with executable transforms, command side effect on accept, CSS, or any completion-specific popup widget.

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
        "priority": 10,
        "triggerCharacters": ["."],
        "wordBoundaryChars": [".", ","],
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

Validation is load/registration-time only and reuses the package metadata budget. Provider IDs must be package-owned (`<apiPrefix>.<name>`), must not claim the reserved `clay.*` namespace, and must be unique within a package. Trigger characters are inert single-character strings; word-boundary characters are inert strings. `timeoutMs` must be within `1..=5000` and `maxItems` within `1..=COMPLETION_RESULT_MAX_ITEMS`. Clay rejects raw callbacks (`handler`, `callback`, `complete`, `function`, `module`), raw ops, native handles, client-side JavaScript, snippets/commands, URLs, shell/network/AI/WASM/native/package-manager fields, duplicate provider IDs, and oversize metadata.

Result items are inert text-replacement data only: `label`, `insertText`, `detail`, `commitCharacters`, and provenance. They carry no callbacks, command side effects, file paths, shell/network/AI directives, raw op names, or client JavaScript. Providers may read only Clay-provided open-document content/windows; completion grants no filesystem/network/shell/AI/raw-op/native-UI/client-runtime authority without later documented APIs and an approved decision log. Per-field and result payload budgets (`COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `COMPLETION_RESULT_MAX_ITEMS`, and per-field char caps) are enforced before client publication.

Trigger classification is local manifest lookup: typing a trigger character edits locally first (`ClientFirstPredictable`) and then enqueues a typed `CompletionRequest` through a bounded non-blocking channel. Manual `completion.trigger` requests completions without mutating text. Provider execution runs server-side on a cancellable `UiReactivePriority` lane that aborts or stale-drops older in-flight requests and validates results against the current document/behavior version and provider generation before publication. Provider work is UI-reactive/cancellable and never runs on keypress-to-local-paint, paint, layout, scroll, pointer, or text-event hot paths.

Phase 18.11 ships one built-in `core.bufferWords` provider that suggests unique words from the bounded server-prepared document window around the cursor prefix; it is always available and is not removed by package disable/reload. Package providers registered through `clay.completion.serverRegisterCompletionProvider` are metadata-only: the registered provider metadata is retained, but no executable JS provider token is exposed this phase. A future constrained handler bridge may add executable package providers; until then the built-in buffer-word provider remains the only executable provider. Any future provider needing workspace, network, AI, shell, or filesystem authority must introduce explicit permissions and an approved decision log before implementation.

See [`clay.completion.serverRegisterCompletionProvider`](../clay-js-api/completion/server-register-completion-provider.md) for the authoritative API reference, and [`docs/wiki/modules/phase18.11-completion-provider-primitive-review.md`](../../wiki/modules/phase18.11-completion-provider-primitive-review.md) for the implementation review.

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
6. **UI tests** — slot placement, fixed/transient panel behavior, overlay geometry, action validation, and observability privacy.
7. **Theme/style tests** — token validation, same-type fallback mapping, typed style variables, and raw CSS/color rejection.
8. **Package metadata tests** — `clay.contributions.ui.panels`, `ui.components`, `ui.overlays`, `themeTokens`, duplicate fixed slot claims, and bounded payload diagnostics.
9. **Parse/render tests** — bounded snapshots, stale result rejection, decoration payload budgets.
10. **Docs tests** — package docs, primitive docs, and master index links stay current.
11. **Manual smoke tests** — actual GUI package loading and user workflow checks. For Phase 18.9, the smoke path in [Launch and GUI Smoke](../../development/launch-and-gui-smoke.md) opens files with no language package and confirms editable `core.text`/`core.code` fallback modes.

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
- Execute commands from UI callbacks or transient menu items without routing through the server-owned `CommandExecution` path.
- Bypass command permission/provenance validation from package code.
- Treat a transient menu session as a fixed bottom panel or as a generic `TransientOverlayContribution` that owns dynamic query state.
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
