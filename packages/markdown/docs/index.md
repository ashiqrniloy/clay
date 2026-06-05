# @clay/markdown

`@clay/markdown` is Clay's first-party Markdown mode proof-of-concept package. It uses package metadata and server-side Clay JS APIs to declare Markdown mode support without adding Markdown-specific Rust package-loading paths.

## Package Contract

- Package name: `@clay/markdown`
- API prefix: `markdown`
- Major mode: `markdown`
- Supported extensions: `.md`, `.markdown`, `.mdown`
- Supported MIME type: `text/markdown`
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js`
- Parser/decorator adapter: `./dist/parser.js`
- SDUI preview/status adapter: `./dist/sdui.js`
- Documentation entry: `./docs/index.md`
- Large-file policy defaults: full highlighting for files up to `1 MiB`; viewport/windowed highlighting above `1 MiB`; large-file behavior above `5 MiB`; `64 KiB` parse windows; `4 KiB` guard ranges; `30 MiB` retained syntax/decor budget; `50 ms` parse timeout; `plain-text-fallback` when the budget is exhausted.
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
- A Markdown syntax decoration primitive, `markdown-it` parser dependency, parse handler metadata, deterministic `full`/`windowed`/`degraded`/`plain-text-fallback` status states, and safe span clearing when the syntax budget is exhausted.
- A package-owned parser adapter that converts markdown-it token streams and package-owned source/line indexes into viewport-bounded Clay decoration spans for ATX headings, strong/emphasis, inline code, fenced code blocks, and list markers.

Package installation remains separate from execution. Clay validates this metadata during package operations, configuration load, document open/reload, explicit mode activation, or explicit viewport/policy changes; typing, paint, scroll, layout, and text-event handlers do not load the package, run package-manager work, or compute large-file policy. SDUI status messages are fixed/sanitized package strings and never include document text or absolute paths. Configuration cannot install, enable, disable, or grant new permissions to this package; those authorities remain outside `init.js` package options.

## Smoke Fixture

The deterministic smoke fixture at `tests/fixtures/configuration/markdown-mode/` registers the package metadata, opens `workspace/sample.md` when a workspace root is available, activates Markdown mode, registers parse/decorations, publishes representative decorations, and publishes the inert Markdown preview/status SDUI panel. If no workspace root is available, the fixture falls back to document `1` so `cargo run -- smoke-gui --config-fixture markdown-mode` still validates the package SDUI workflow without expanding filesystem authority.
