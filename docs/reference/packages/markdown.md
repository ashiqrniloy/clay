# @clay/markdown Package

`@clay/markdown` is the first-party Markdown mode POC package. Its source scaffold lives under `packages/markdown/`.

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

Load-time package validation and activation are explicit package/configuration/document-open operations. Package-manager process work and JavaScript package loading are not part of keypress, paint, scroll, layout, or text-event handlers.

Markdown parse/decorations are produced by the package-owned `./dist/parser.js` adapter. The adapter uses `markdown-it` block tokens, inline child tokens, and package-owned source/line indexes to produce inert `syntax` decoration spans with Clay style tokens such as `markup.heading.1`, `markup.strong`, `markup.emphasis`, `markup.inline-code`, `markup.code-block`, and `markup.list-marker`. The Rust client receives only validated viewport-bounded decoration spans and maps known style tokens locally.

The package-owned `./dist/sdui.js` adapter builds an inert preview/status panel with mode, parse, decoration, and preview labels plus a `markdown.togglePreview` button. Runtime SDUI validation requires package commands to be registered before a package-owned SDUI tree can target them, so disabling or invalidating the package falls back to plain text without stale Markdown command/keybinding authority.

## Smoke Fixture

Use `cargo run -- smoke-gui --config-fixture markdown-mode` for a deterministic GUI smoke path. The fixture lives at `tests/fixtures/configuration/markdown-mode/`, opens `workspace/sample.md` when a workspace root is provided by tests, activates Markdown mode, registers parse/decorations, publishes representative decorations, and shows the Markdown preview/status SDUI panel.
