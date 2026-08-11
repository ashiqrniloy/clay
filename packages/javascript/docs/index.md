# @clay/javascript

`@clay/javascript` is Clay's first-party JavaScript language package. Phase 18.18 runs JS/JSX/MJS/CJS syntax through Clay's Tier 1 native engine and closed two-axis vocabulary on generic primitives.

## Package Contract

- Package name: `@clay/javascript`
- API prefix: `javascript`
- Language ID: `javascript`
- Mode ID: `javascript`
- Supported extensions: `.js`, `.jsx`, `.mjs`, `.cjs`
- Grammar kind: `native` (compiled source `tree-sitter-javascript`; no package `.wasm` asset)
- Highlight query: `./queries/highlights.scm`
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js` (exports `loadJavaScriptPackage`; re-exported from `./dist/index.js`)
- Documentation entry: `./docs/index.md`
- Vocabulary styleMap: captures map directly to `TokenType` + `Modifiers` (`keyword` -> `Keyword`, `function.declaration` -> `Function + Declaration`, plus string/comment/operator/number families)
- Budgets: `5000 ms` parse timeout, `4096 byte` max parse window

## Phase 18.14 Surfaces

- **Major mode `javascript`**: registered with generic file-extension probes.
- **Behavior manifest**: 2-space indentation, bracket/quote/template-literal pairs, `//` comment continuation, and electric `}`/`)`/`]` outdent.
- **Command `javascript.toggleLineComment`**: server-first command registered through the Clay `CommandExecution` path.
- **Completion provider `javascript.keywords`**: priority-0 metadata-only provider carrying 32 inert JavaScript keyword text replacements with `.` trigger and 300 ms/32-item budgets. Snippet transforms remain deferred to Phase 18.19.
- **Status item `javascript.status.mode`**: inert `statusItem` component contribution.

Phase 18.18 promotes the grammar contribution from legacy WASM/style-token metadata to native `TokenType` + `Modifiers` metadata. Active syntax grammar remains selectable independently of active major mode, so a `.js` file can use the JavaScript grammar while its major mode stays `core.code`. Loading the package does not silently change the mode of already-open documents.

All first-party language packages are implemented through generic primitives (syntax grammars, behavior manifests, completion providers, commands, and status items) without requiring per-language Rust branches. This generic approach ensures Phase 18.21 LSP enrichment can be added uniformly across all packages without architectural changes.

## Configuration

Phase 18.18 keeps JavaScript editing defaults (2-space indentation, `//` line comments, bracket/quote/template-literal pairs, electric `}`/`)`/`]` outdent, and `.` autocomplete trigger) as package-defined values. No new user-tunable configuration keys are introduced in this phase. Future phases may expose documented, package-prefixed options through `configuration.setPackageOption` after they are declared in `clay.contributions.packageOptions`.

## Permissions

- `mode-registration`
- `command-registration`
- `completion-provider`
- `parse-document`
- `render-decorations`

The package does not request filesystem, network, shell, AI, WASM-authority, raw Deno op, native widget, client runtime, package install/enable, or workspace mutation authority. LSP, full language-server protocol integration, workspace-wide symbol indexes, AI completions, network-backed completions, and mutating toolchain execution are out of scope for Phase 18.14.

## Default Load Path

```js
// ~/.config/clay/init.js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/javascript");
```

The package is not auto-loaded. Without this line, `.js`/`.jsx`/`.mjs`/`.cjs` files remain editable under `core.code` with no JavaScript-specific behavior.

## Engine tiers

- **Tier 1 native** is the default for `.js`, `.jsx`, `.mjs`, and `.cjs`, using Clay's compiled `tree-sitter-javascript` descriptor.
- **Tier 2 web-tree-sitter WASM** remains available to an explicitly selected package that actually supplies a confined WASM artifact; `@clay/javascript` itself ships native metadata only.
- **Tier 3 JavaScript fallback** remains available through the server parse-handler path when `setSyntaxEnginePreference("javascript", "javascript")` is selected or no grammar is available.

All tiers use one capture-to-`TokenType`/`Modifiers` vocabulary mapper. Open returns before parsing finishes; later failures publish sanitized `parse.open_failed` diagnostics. See the [tiered syntax engine package-author contract](../../../docs/reference/packages/creating-packages.md#phase-1816-authoring-contract-tiered-syntax-engine).

## Typography

JavaScript mode declares semantic `defaultFontRole: "monospace"`. Package metadata never declares concrete font families or sizes; active user typography owns both.

## Validation

Grammar/query/style metadata, mode patterns, command metadata, completion provider metadata, and UI component trees are validated at package load time. Native grammars require a compiled source ID and no artifact path; WASM grammars still require confined `.wasm` paths. Query files must be `.scm`; first-party styleMaps use closed `TokenType` + `Modifiers`, while validated legacy style tokens remain compatible. Parse/highlight work is background, cancellable, viewport-prioritized, and bounded by the shared parse/decor/cache budgets; it never runs in keypress, paint, layout, scroll, pointer, or text-event hot paths. Phase 18.16 retains the Phase 18.10 first-party-only rule and rejects arbitrary third-party/native grammar artifact loading; broader trust is deferred to Phase 23 and a separate security decision.
