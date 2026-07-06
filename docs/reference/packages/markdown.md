# @clay/markdown Package

`@clay/markdown` is the first-party Markdown mode POC package. Its source scaffold lives under `packages/markdown/`.

## End-User UX Baseline

The intended default end-user setup for Markdown mode is a small `~/.config/clay/init.js`:

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

This is the Markdown product baseline. It is distinct from the dev-only smoke fixtures (see [Smoke Fixture](#smoke-fixture)), which inline the package manifest object and call each Clay facade manually only to validate them deterministically. The fixture manifest block and manual `serverLoadPackage` / `serverRegisterModePattern` / `serverActivateMajorMode` / `serverRegisterCommand` / `serverRegisterParseHandler` / `serverPublishDecorations` / `publishTree` plumbing are dev validation, never the documented end-user path.

Baseline invariants for the end-user Markdown UX:

- **Editor-only main slot.** The editor occupies the mandatory `main` slot of `PaneSlotLayout`. No default `PanelContribution` (side/preview/status panel) is published on load.
- **Optional preview on demand.** An optional preview/status panel is a `clay:ui` `PanelContribution` targeting a slot such as `right` with `defaultVisibility: "hidden"`, shown only through `setPackageOption`, `serverSetLayoutOverride`, or `markdown.togglePreview`.
- **Selected-file open is edit-only.** Opening a file through `Ctrl+O` activates Markdown behavior/decorations through generic `MajorModeActivation` + `DocumentClassification`. Saving a file picked through the dialog is out of scope until a later phase.

Loading, contribution validation, selected-file activation, and Phase 19 reload refresh run at configuration/document-open/reload time only; typing, paint, scroll, layout, and text-event paths stay client-local/non-blocking and read only installed inert shell/contribution state. Hot reload reruns the one-line `await loadPackage("@clay/markdown")` setup in a fresh runtime generation with an empty `globalThis.__clayLoadedPackages` cache, replacing Markdown mode metadata and parse handler tokens without adding a Markdown-specific Rust branch. Failed reloads keep the prior Markdown generation active and surface sanitized diagnostics. The one-line loader does not broaden package installation, filesystem (beyond the selected file and config root), workspace expansion, shell, network, AI mutation, WASM, raw-op, native-widget, raw-CSS, renderer-callback, or client-side JavaScript authority beyond what the constrained first-party `@clay/*` resolver already grants.

## Contract

- `package.json` name: `@clay/markdown`
- Clay API prefix: `markdown`
- Mode: `markdown`
- File patterns: `.md`, `.markdown`, `.mdown`
- MIME type: `text/markdown`
- Docs path: `./docs/index.md`
- Entries: `./dist/index.js`, `./dist/load.js`, parser export `./dist/parser.js`, and SDUI export `./dist/sdui.js`
- Parser dependency: `markdown-it` (package-owned token-stream adapter boundary)

## Security

The package declares `mode-registration`, `mode-activation`, `command-registration`, `parse-document`, and `render-decorations` only. Installation records metadata without executing JavaScript. Client surfaces receive validated inert manifests, SDUI metadata, and decoration spans; no client-side JavaScript, raw Deno ops, filesystem, network, shell, AI, WASM, native-widget, package-enable, or workspace-mutation authority is granted.

## Runtime Boundary

Load-time package validation and activation are explicit package/configuration/document-open/reload operations. Package-manager process work and JavaScript package loading are not part of keypress, paint, scroll, layout, or text-event handlers. Reload refresh uses generic mode activation plus bounded/background parse refresh for open Markdown documents; it does not send full-document IPC snapshots for unchanged open documents.

Markdown parse/decorations are produced by the package-owned `./dist/parser.js` adapter. Runtime reload replaces generation-scoped parse registrations, cancels old-generation in-flight parse work, and prevents stale old-runtime-generation parse results from publishing decorations. The adapter uses `markdown-it` block tokens when the dependency is installed, otherwise a small built-in scanner covers headings, fences, list markers, emphasis, strong, and inline code so Linux cargo tests do not depend on preinstalled `node_modules`. Package-owned source/line indexes produce inert `syntax` decoration spans with Clay style tokens such as `markup.heading.1`, `markup.strong`, `markup.emphasis`, `markup.inline-code`, `markup.code-block`, and `markup.list-marker`. The Rust client receives only validated viewport-bounded decoration spans and maps known style tokens locally.

The package owns fixed large-file defaults verified by the Phase 18.5 configuration review: full highlighting through `1 MiB`, windowed highlighting above `1 MiB`, large-file behavior above `5 MiB`, `64 KiB` parse windows, `4 KiB` guard ranges, a `30 MiB` syntax/decor budget, and `5000 ms` parser timeout. These are not user-tunable `~/.config/clay/init.js` settings yet; the package declares no `contributions.configuration` entries and does not request `package-configuration`. The parse-window, guard, memory-budget, timeout, parse-unit, and viewport-priority registration fields are documented as `custom_properties` on `clay.parse.serverRegisterParseHandler` and are rejected by the server when unsafe. If syntax budget pressure is reported or the supplied window payload exceeds the budget, the adapter returns no Markdown spans and reports `plain-text-fallback` instead of invoking `markdown-it`.

The package-owned `./dist/sdui.js` adapter builds an inert preview/status panel with mode, parse, decoration, highlighting-policy, and preview labels plus a `markdown.togglePreview` button. Runtime SDUI validation requires package commands to be registered before a package-owned SDUI tree can target them, so disabling or invalidating the package falls back to plain text without stale Markdown command/keybinding authority. Status text uses fixed/sanitized package messages (`full`, `windowed`, `degraded`, `plain-text-fallback`) and does not include document text or absolute paths.

## UI/Layout Behavior

Phase 18.5 establishes the Markdown package UI/layout authoring contract:

- **No default fixed panel on load.** The package does not publish a default side panel, preview panel, or status panel when loaded. The editor occupies `PaneSlotLayout.main` by default.
- **Optional preview as `PanelContribution`.** The package may register an optional preview panel through `serverRegisterPanelContribution` targeting the `right` slot with `defaultVisibility: "hidden"`. The panel appears only when the user enables it through `setPackageOption`, `serverSetLayoutOverride`, or the `markdown.togglePreview` command.
- **Theme tokens for preview styling.** Panel styles use `PackageThemeTokenDeclaration` with same-type core fallbacks (e.g., `markdown.preview.background` → `surface.panel`). Raw CSS, raw colors, and renderer callbacks are prohibited.
- **User customization through Clay JS APIs.** Preview visibility, slot, split ratio, and theme token mapping are controlled through `setPackageOption` and `serverSetLayoutOverride`, not through hidden JSON/TOML/ad hoc keys.
- **Generic primitive consumption.** The package consumes only generic shell/layout/UI/configuration primitives from Phases 18.1–18.4. No Markdown-specific Rust editor/parser/render/shell branch is required or added.

## Smoke Fixture

Use `cargo run -- smoke-gui --config-fixture markdown-mode` for a deterministic GUI smoke path. The fixture lives at `tests/fixtures/configuration/markdown-mode/`, opens `workspace/sample.md` when a workspace root is provided by tests, activates Markdown mode, registers parse/decorations, and publishes representative decorations. The fixture does not publish a default side panel; the optional preview is a `PanelContribution` the host opts into.
