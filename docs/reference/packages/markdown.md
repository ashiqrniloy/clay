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
- Grammar contribution: `native`, source `tree-sitter-md-025` (no package `.wasm` asset)
- Highlight query: `./queries/highlights.scm`
- Vocabulary styleMap: headings/prose/code/list/link/quote captures map directly to closed `TokenType` + `Modifiers`
- Completion provider `markdown.keywords`: priority-0 metadata-only provider carrying 16 inert Markdown construct text replacements, `#`/`[`/`` ` `` triggers, and 300 ms/32-item budgets; snippet transforms remain deferred to Phase 18.19
- Parser dependency: `markdown-it` (package-owned Tier 3 token-stream adapter boundary)

## Phase 18.16 syntax engine artifacts

Tier 1 native highlighting uses Clay's compiled `tree-sitter-md-025 = 0.5.6` dependency and the package query at `packages/markdown/queries/highlights.scm`. Tier 2 remains available to an explicitly selected package that supplies valid confined WASM metadata; `@clay/markdown` itself ships native metadata only, and package load order cannot replace native Tier 1. The existing package JavaScript parser remains Tier 3 fallback for decorations when no native handler is selected; `setSyntaxEnginePreference("markdown", "javascript")` suppresses native selection and leaves that fallback active. Preview is a separate package-JS SDUI adapter and is unchanged by syntax-engine selection. All Tree-sitter routes use the shared direct `TokenType` + `Modifiers` vocabulary pipeline; validated legacy style-token inputs remain a compatibility path for older packages. Until a WASM binary is committed, `packages/markdown/grammars/PROVENANCE.md` records the reproducible build command and required SHA-256 recording step. Runtime never fetches, builds, shells out, or loads native libraries for this artifact.

## Typography

Markdown declares semantic `defaultFontRole: "proportional"`. Inline-code and fenced/indented code-block syntax spans declare `fontRole: "monospace"`; prose inherits proportional. These generic mode/decoration fields resolve through user-owned typography and contain no concrete family or size. See [Semantic Typography Roles](../primitives/typography.md).

## Editing Behavior

`buildCodeEditingManifest` produces Markdown's inert editor rules: 2-space tabs, generic `ContinueLineMarkers` for `-`/`*`/`+`/ordered-dot lists, pairs for parentheses/brackets/strong/emphasis/code spans, and one-character `#`/`[`/`` ` `` completion triggers. Markdown declares no electric outdent or line-comment continuation. `markdown.toggleComment` handles HTML-style comment intent as a package-prefixed server-first command; registration grants no document, filesystem, network, shell, workspace, or AI authority. `markdown.status.mode` is a validated inert `statusItem`, not a fixed preview/status panel.

## Security

The package declares `mode-registration`, `mode-activation`, `command-registration`, `completion-provider`, `parse-document`, and `render-decorations` only. Installation records metadata without executing JavaScript. Client surfaces receive validated inert manifests, SDUI metadata, and decoration spans; no client-side JavaScript, raw Deno ops, filesystem, network, shell, AI, WASM, native-widget, package-enable, or workspace-mutation authority is granted. third-party/native grammar artifact loading remains deferred to Phase 23 and a separate trust decision.

## Runtime Boundary

Load-time package validation and activation are explicit package/configuration/document-open/reload operations. Open returns text and mode state before background parse completion; handler failures surface as sanitized `clay.parse.open_failed` diagnostics. Package-manager process work and JavaScript package loading are not part of keypress, paint, scroll, layout, or text-event handlers; parse/query work remains outside keypress, paint, layout, scroll, pointer, or text-event hot paths. Reload refresh uses generic mode activation plus bounded/background parse refresh for open Markdown documents; it does not send full-document IPC snapshots for unchanged open documents.

Default Markdown parse/decorations are produced by the compiled `tree-sitter-md-025` descriptor and `packages/markdown/queries/highlights.scm`. On document open, the generic syntax selector chooses the path-matching native contribution, installs a generation-scoped `TreeSitterSyntaxHandler` before package JS handlers, and publishes vocabulary `DecorationSpan`s through `ParseCoordinator`; no package JavaScript runs for native highlighting. `./dist/parser.js` remains registered as the same package/mode's Tier 3 fallback and uses `markdown-it` or its small scanner only when no native handler owns that key. Runtime replacement keeps stale generations from publishing. The Rust client receives only validated viewport-bounded spans from either engine.

The native grammar contribution caps each selected parse window at `4 KiB`. The Tier 3 fallback keeps the fixed Phase 18.5 policy: full highlighting through `1 MiB`, windowed highlighting above `1 MiB`, large-file behavior above `5 MiB`, `64 KiB` parse windows, `4 KiB` guard ranges, a `30 MiB` syntax/decor budget, and `5000 ms` parser timeout. These are not user-tunable `~/.config/clay/init.js` settings yet; the package declares no `contributions.configuration` entries and does not request `package-configuration`. The parse-window, guard, memory-budget, timeout, parse-unit, and viewport-priority registration fields are documented as `custom_properties` on `clay.parse.serverRegisterParseHandler` and are rejected by the server when unsafe. If syntax budget pressure is reported or the supplied window payload exceeds the budget, the adapter returns no Markdown spans and reports `plain-text-fallback` instead of invoking `markdown-it`.

Independently, the unchanged package-owned `./dist/sdui.js` adapter builds an inert preview/status panel with mode, parse, decoration, highlighting-policy, and preview labels plus a `markdown.togglePreview` button. Runtime SDUI validation requires package commands to be registered before a package-owned SDUI tree can target them, so disabling or invalidating the package falls back to plain text without stale Markdown command/keybinding authority. Status text uses fixed/sanitized package messages (`full`, `windowed`, `degraded`, `plain-text-fallback`) and does not include document text or absolute paths.

## UI/Layout Behavior

Phase 18.5 establishes the Markdown package UI/layout authoring contract:

- **No default fixed panel on load.** The package does not publish a default side panel or preview/status panel when loaded. Its inert `markdown.status.mode` status item may appear in Clay's status area; the editor still occupies `PaneSlotLayout.main` by default.
- **Optional preview as `PanelContribution`.** The package may register an optional preview panel through `serverRegisterPanelContribution` targeting the `right` slot with `defaultVisibility: "hidden"`. The panel appears only when the user enables it through `setPackageOption`, `serverSetLayoutOverride`, or the `markdown.togglePreview` command.
- **Theme tokens for preview styling.** Panel styles use `PackageThemeTokenDeclaration` with same-type core fallbacks (e.g., `markdown.preview.background` → `surface.panel`). Raw CSS, raw colors, and renderer callbacks are prohibited.
- **User customization through Clay JS APIs.** Preview visibility, slot, split ratio, and theme token mapping are controlled through `setPackageOption` and `serverSetLayoutOverride`, not through hidden JSON/TOML/ad hoc keys.
- **Generic primitive consumption.** The package consumes only generic shell/layout/UI/configuration primitives from Phases 18.1–18.4. No Markdown-specific Rust editor/parser/render/shell branch is required or added.

## Smoke Fixture

Use `cargo run -- smoke-gui --config-fixture markdown-mode` for a deterministic GUI smoke path. The fixture lives at `tests/fixtures/configuration/markdown-mode/`, opens `workspace/sample.md` when a workspace root is provided by tests, activates Markdown mode, registers parse/decorations, and publishes representative decorations. The fixture does not publish a default side panel; the optional preview is a `PanelContribution` the host opts into.
