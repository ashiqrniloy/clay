# @clay/markdown

`@clay/markdown` is Clay's first-party Markdown mode proof-of-concept package. It uses package metadata and server-side Clay JS APIs to declare Markdown mode support without adding Markdown-specific Rust package-loading paths.

## Package Contract

- Package name: `@clay/markdown`
- API prefix: `markdown`
- Major mode: `markdown`
- Supported extensions: `.md`, `.markdown`, `.mdown`
- Supported MIME type: `text/markdown`
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js` (exports `loadMarkdownPackage(clay, options)` for facade-driven callers and `markdownLoadMode(options)` as the package-owned one-line default entry; re-exported from `./dist/index.js`)
- Parser/decorator adapter: `./dist/parser.js`
- SDUI preview/status adapter: `./dist/sdui.js`
- Documentation entry: `./docs/index.md`
- Large-file policy defaults: full highlighting for files up to `1 MiB`; viewport/windowed highlighting above `1 MiB`; large-file behavior above `5 MiB`; `64 KiB` parse windows; `4 KiB` guard ranges; `30 MiB` retained syntax/decor budget; `5000 ms` parse timeout; `plain-text-fallback` when the budget is exhausted.
- Configuration status: Phase 18.5 verifies these values as fixed package-owned defaults. `@clay/markdown` declares no `contributions.configuration` entries, does not request `package-configuration`, and does not expose Markdown large-file tuning through `~/.config/clay/init.js` yet. The bounded parse-window values it passes to `clay.parse.serverRegisterParseHandler` are covered by that API's `custom_properties` and server validation; file-size thresholds and status labels remain package constants until a later configuration API is implemented.

## Permissions

The package declares only the permissions required by the Phase 18 Markdown POC:

- `mode-registration`
- `mode-activation`
- `command-registration`
- `parse-document`
- `render-decorations`

It does not request filesystem, network, shell, AI mutation, remote listener, WASM, raw Deno op, native widget, package install/enable, workspace mutation, or client-side JavaScript authority.

## Contributions

The package manifest declares inert contribution metadata for:

- Markdown document classification patterns.
- `markdown.togglePreview`, `markdown.insertHeading`, and `markdown.toggleList` commands.
- Preview, heading, and list key routing metadata.
- Client-first predictable list continuation, fenced-code indentation, and pair-handling transform descriptors.
- A Markdown preview/status SDUI region with inert mode, parse, decoration, highlighting-policy, and preview labels plus a `markdown.togglePreview` button action.
- A Markdown syntax decoration primitive, optional `markdown-it` parser dependency with a tiny built-in scanner fallback for uninstalled dev trees, parse handler metadata, deterministic `full`/`windowed`/`degraded`/`plain-text-fallback` status states, and safe span clearing when the syntax budget is exhausted.
- A package-owned parser adapter that converts markdown-it token streams and package-owned source/line indexes into viewport-bounded Clay decoration spans for ATX headings, strong/emphasis, inline code, fenced code blocks, and list markers.

Package installation remains separate from execution. Clay validates this metadata during package operations, configuration load, document open/reload, explicit mode activation, or explicit viewport/policy changes; typing, paint, scroll, layout, and text-event handlers do not load the package, run package-manager work, or compute large-file policy. SDUI status messages are fixed/sanitized package strings and never include document text or absolute paths. Configuration cannot install, enable, disable, or grant new permissions to this package; those authorities remain outside `init.js` package options.

## Default Load Path

The documented end-user default is one line from `~/.config/clay/init.js`:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
```

Plan 029 (Phase 18.6) implemented the generic `loadPackage("@clay/*")` resolver with the `op_clay_packages_load_package_by_specifier` gate and the `FirstPartyLoadEntryAllowlist` module-loader bridge. Phase 18.7 verifies the default init.js experience: this one line loads Markdown once on the persistent server runtime, and selected-file open reuses the registered mode and parse handler through generic open-time activation. The resolver is constrained to first-party `@clay/*` packages only; non-`@clay/*` registry resolution (third-party, npm, custom registries) is deferred to Phase 23 ecosystem hardening. See `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` for the authority rationale and security review.

The Markdown package's `loadEntry` (`markdownLoadMode` in `packages/markdown/dist/load.js`) is the default activation export that `loadPackage` invokes. The package-owned `markdownLoadMode()` entry remains available as a convenience alias for per-load options:

```js
import { markdownLoadMode } from "@clay/markdown";

await markdownLoadMode();
```

The explicit `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })` separation is preserved — file-open is app/editor configuration, not Markdown mode behavior.

## Engine tiers

- **Tier 1 native** is the default for `.md`, `.markdown`, and `.mdown`, using Clay's compiled Markdown grammar descriptor.
- **Tier 2 web-tree-sitter WASM** uses the package-root-confined `./grammars/markdown.wasm` only after `setSyntaxEnginePreference("markdown", "wasm")`; package load order cannot override native highlighting.
- **Tier 3 JavaScript fallback** remains the package-owned `markdown-it`/scanner parser for Markdown mode decorations and preview behavior, or when `setSyntaxEnginePreference("markdown", "javascript")` is selected.

Tree-sitter captures use one `TokenType`/`Modifiers` vocabulary mapper. Open returns before background parsing finishes; later failures publish sanitized `clay.parse.open_failed` diagnostics. Parse/query work remains outside keypress, paint, layout, scroll, pointer, or text-event hot paths. The package cannot use network, shell, AI mutation, remote listeners, raw `Deno.core.ops`, direct Masonry/widget mutation, or client-side JavaScript authority. The package keeps arbitrary third-party/native grammar artifact loading deferred to Phase 23 and a separate trust decision. See the [tiered syntax engine package-author contract](../../../docs/reference/packages/creating-packages.md#phase-1816-authoring-contract-tiered-syntax-engine).

## Typography

Markdown mode declares `defaultFontRole: "proportional"`. Inline-code and code-block syntax spans declare only `fontRole: "monospace"`; active user typography resolves families and sizes. Invalid roles fail closed before publication.

## Smoke Fixture

The deterministic smoke fixture at `tests/fixtures/configuration/markdown-mode/` registers the package metadata, opens `workspace/sample.md` when a workspace root is available, activates Markdown mode, registers parse/decorations, and publishes representative decorations. If no workspace root is available, the fixture falls back to document `1` so `cargo run -- smoke-gui --config-fixture markdown-mode` still validates the package workflow without expanding filesystem authority. The fixture does not publish a default side panel; the optional preview is a `PanelContribution` the host opts into.
