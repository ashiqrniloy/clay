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
- A Markdown preview/status SDUI region with inert mode, parse, decoration, and preview labels plus a `markdown.togglePreview` button action.
- A Markdown syntax decoration primitive, `markdown-it` parser dependency, and parse handler metadata.
- A package-owned parser adapter that converts markdown-it token streams and package-owned source/line indexes into viewport-bounded Clay decoration spans for ATX headings, strong/emphasis, inline code, fenced code blocks, and list markers.

Package installation remains separate from execution. Clay validates this metadata during package operations, configuration load, document open/reload, or explicit mode activation; typing, paint, scroll, layout, and text-event handlers do not load the package or run package-manager work.

## Smoke Fixture

The deterministic smoke fixture at `tests/fixtures/configuration/markdown-mode/` registers the package metadata, opens `workspace/sample.md` when a workspace root is available, activates Markdown mode, registers parse/decorations, publishes representative decorations, and publishes the inert Markdown preview/status SDUI panel. If no workspace root is available, the fixture falls back to document `1` so `cargo run -- smoke-gui --config-fixture markdown-mode` still validates the package SDUI workflow without expanding filesystem authority.
