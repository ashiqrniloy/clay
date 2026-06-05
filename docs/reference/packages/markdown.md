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

The package owns fixed large-file defaults verified by the Phase 18.5 configuration review: full highlighting through `1 MiB`, windowed highlighting above `1 MiB`, large-file behavior above `5 MiB`, `64 KiB` parse windows, `4 KiB` guard ranges, a `30 MiB` syntax/decor budget, and `50 ms` parser timeout. These are not user-tunable `~/.config/clay/init.js` settings yet; the package declares no `contributions.configuration` entries and does not request `package-configuration`. The parse-window, guard, memory-budget, timeout, parse-unit, and viewport-priority registration fields are documented as `custom_properties` on `clay.parse.serverRegisterParseHandler` and are rejected by the server when unsafe. If syntax budget pressure is reported or the supplied window payload exceeds the budget, the adapter returns no Markdown spans and reports `plain-text-fallback` instead of invoking `markdown-it`.

The package-owned `./dist/sdui.js` adapter builds an inert preview/status panel with mode, parse, decoration, highlighting-policy, and preview labels plus a `markdown.togglePreview` button. Runtime SDUI validation requires package commands to be registered before a package-owned SDUI tree can target them, so disabling or invalidating the package falls back to plain text without stale Markdown command/keybinding authority. Status text uses fixed/sanitized package messages (`full`, `windowed`, `degraded`, `plain-text-fallback`) and does not include document text or absolute paths.

## Smoke Fixture

Use `cargo run -- smoke-gui --config-fixture markdown-mode` for a deterministic GUI smoke path. The fixture lives at `tests/fixtures/configuration/markdown-mode/`, opens `workspace/sample.md` when a workspace root is provided by tests, activates Markdown mode, registers parse/decorations, publishes representative decorations, and shows the Markdown preview/status SDUI panel.
