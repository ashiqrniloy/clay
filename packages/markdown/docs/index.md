# @clay/markdown

`@clay/markdown` is Clay's first-party Markdown mode proof-of-concept package. It uses package metadata and server-side Clay JS APIs to declare Markdown mode support without adding Markdown-specific Rust package-loading paths.

## Package Contract

- Package name: `@clay/markdown`
- API prefix: `markdown`
- Major mode: `markdown`
- Supported extensions: `.md`, `.markdown`, `.mdown`
- Supported MIME type: `text/markdown`
- Grammar kind: `native` (compiled source `tree-sitter-md-025`; no package `.wasm` asset)
- Highlight query: `./queries/highlights.scm`
- Vocabulary styleMap: headings map to `Heading1..6`; strong/emphasis map to `Paragraph + Bold/Italic`; code spans/blocks, list markers, links, and quotes map to their closed prose `TokenType`s
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js` (exports `loadMarkdownPackage(clay, options)` for facade-driven callers and `markdownLoadMode(options)` as the package-owned one-line default entry; re-exported from `./dist/index.js`)
- Default decoration engine: Tier 1 compiled `tree-sitter-md-025` through the generic native syntax handler
- Tier 3 fallback adapter: `./dist/parser.js`
- Package-JS SDUI preview/status adapter: `./dist/sdui.js`
- Behavior manifest: 2-space indentation, generic list continuation, Markdown delimiter pairs, `#`/`[`/`` ` `` autocomplete triggers, and no electric or line-comment continuation rules
- Completion provider `markdown.keywords`: priority-0 metadata-only provider carrying 16 inert Markdown construct text replacements with 300 ms/32-item budgets; snippet transforms remain deferred to Phase 18.19
- Status item: inert `markdown.status.mode` component
- Documentation entry: `./docs/index.md`
- Parse bounds: Tier 1 native contribution windows are capped at `4 KiB`; Tier 3 fallback policy uses `64 KiB` windows, `4 KiB` guards, a `30 MiB` retained syntax/decor budget, and `5000 ms` timeout with `plain-text-fallback` under budget pressure.
- Configuration status: Phase 18.5 verifies these values as fixed package-owned defaults. `@clay/markdown` declares no `contributions.configuration` entries, does not request `package-configuration`, and does not expose Markdown large-file tuning through `~/.config/clay/init.js` yet. The bounded parse-window values it passes to `clay.parse.serverRegisterParseHandler` are covered by that API's `custom_properties` and server validation; file-size thresholds and status labels remain package constants until a later configuration API is implemented.

## Permissions

The package declares only the permissions required by the Phase 18 Markdown POC:

- `mode-registration`
- `mode-activation`
- `command-registration`
- `completion-provider`
- `parse-document`
- `render-decorations`

It does not request filesystem, network, shell, AI mutation, remote listener, WASM, raw Deno op, native widget, package install/enable, workspace mutation, or client-side JavaScript authority.

## Contributions

The package manifest declares inert contribution metadata for:

- Tier 1 native Markdown grammar/query metadata and its capture-to-`TokenType`/`Modifiers` vocabulary styleMap.
- Markdown document classification patterns.
- `markdown.toggleComment`, `markdown.togglePreview`, `markdown.insertHeading`, and `markdown.toggleList` server-first commands.
- Priority-0 `markdown.keywords` static text items plus `#`/`[`/`` ` `` trigger metadata through generic completion primitives.
- Preview, heading, and list key routing metadata.
- Client-first predictable list continuation and pair handling installed through `buildCodeEditingManifest`; fenced-code metadata remains available to the background parser.
- A Markdown preview/status SDUI region with inert mode, parse, decoration, highlighting-policy, and preview labels plus a `markdown.togglePreview` button action.
- Tier 1 native Markdown syntax decoration through the package query and vocabulary styleMap; the manifest no longer declares `parser.js` as the default decoration contribution.
- A registered Tier 3 `markdown-it`/scanner fallback adapter that can still produce viewport-bounded legacy-compatible spans when no native handler is selected.

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
- **Tier 2 web-tree-sitter WASM** remains available to an explicitly selected package that actually supplies a confined WASM artifact; `@clay/markdown` itself ships native metadata only.
- **Tier 3 JavaScript fallback** remains the package-owned `markdown-it`/scanner parser for decorations when no native handler is selected; `setSyntaxEnginePreference("markdown", "javascript")` suppresses native selection. Preview remains independently package-JS in `./dist/sdui.js`; it never routes through Tree-sitter.

Tree-sitter captures use one direct `TokenType`/`Modifiers` vocabulary mapper; validated legacy style-token inputs remain a compatibility path for older packages. Open returns before background parsing finishes; later failures publish sanitized `clay.parse.open_failed` diagnostics. Parse/query work remains outside keypress, paint, layout, scroll, pointer, or text-event hot paths. The package cannot use network, shell, AI mutation, remote listeners, raw `Deno.core.ops`, direct Masonry/widget mutation, or client-side JavaScript authority. The package keeps arbitrary third-party/native grammar artifact loading deferred to Phase 23 and a separate trust decision. See the [tiered syntax engine package-author contract](../../../docs/reference/packages/creating-packages.md#phase-1816-authoring-contract-tiered-syntax-engine).

All first-party language packages are implemented through generic primitives (syntax grammars, behavior manifests, completion providers, commands, and status items) without requiring per-language Rust branches. This generic approach ensures Phase 18.21 LSP enrichment can be added uniformly across all packages without architectural changes.

## Typography

Markdown mode declares `defaultFontRole: "proportional"`. Inline-code and code-block syntax spans declare only `fontRole: "monospace"`; active user typography resolves families and sizes. Invalid roles fail closed before publication.

## Smoke Fixture

The deterministic smoke fixture at `tests/fixtures/configuration/markdown-mode/` registers the package metadata, opens `workspace/sample.md` when a workspace root is available, activates Markdown mode, and exercises bounded decoration publication. If no workspace root is available, the fixture falls back to document `1` so `cargo run -- smoke-gui --config-fixture markdown-mode` still validates the package workflow without expanding filesystem authority. The fixture does not publish a default side panel; the optional preview is a `PanelContribution` the host opts into.
