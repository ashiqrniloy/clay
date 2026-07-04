# @clay/rust

`@clay/rust` is Clay's first-party grammar-only Rust syntax highlighting package. It supplies a Tree-sitter grammar, highlight query, and capture-to-style-token map so `.rs` files receive syntax decorations while their active major mode stays the built-in `core.code` fallback.

## Package Contract

- Package name: `@clay/rust`
- API prefix: `rust`
- Language ID: `rust`
- Supported extensions: `.rs`
- Grammar kind: `tree-sitter-wasm` (artifact `./grammars/rust.wasm`, source `tree-sitter-rust`)
- Highlight query: `./queries/highlights.scm`
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js` (exports `loadRustGrammar`; re-exported from `./dist/index.js`)
- Documentation entry: `./docs/index.md`
- Capture-to-style-token map: `keyword` -> `keyword.control`, `string` -> `string.quoted`, `comment` -> `comment.line`, `punctuation` -> `punctuation.definition`
- Budgets: `5000 ms` parse timeout, `4096 byte` max parse window

## Grammar-Only Scope

This package is grammar-only. It declares no major mode, no commands, no completions, no SDUI regions, no package UI, no key behavior manifests, and no language-specific Rust branches. An `.rs` file keeps active major mode `core.code` (or `core.text`); this package only adds syntax highlighting on top.

## Permissions

The package declares only the permissions required for syntax highlighting:

- `parse-document`
- `render-decorations`

It does not request filesystem, network, shell, AI, WASM-authority, raw Deno op, native widget, client runtime, package install/enable, or workspace mutation authority. Phase 18.10 accepts syntax grammar contributions from first-party `@clay/*` packages only and rejects arbitrary third-party/native grammar artifact loading.

## Default Load Path

```js
// ~/.config/clay/init.js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
```

The package is not auto-loaded. Without this line, `.rs` files remain editable under `core.code` with no syntax highlighting. Disabling or invalidating the package removes highlighting without changing editability.

## Validation

Grammar/query/style metadata is validated at package load time through the shared package metadata gate. Paths are package-root-confined `./` asset paths; grammar artifacts must be `tree-sitter-wasm`; query files must be `.scm`; style-map values must be known Clay style tokens. Parse/highlight work is background, cancellable, viewport-prioritized, and bounded by the shared parse/decor/cache budgets; it never runs in keypress, paint, layout, scroll, pointer, or text-event hot paths. See `docs/reference/packages/creating-packages.md` for the authoring contract.
