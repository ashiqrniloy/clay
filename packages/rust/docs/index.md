# @clay/rust

`@clay/rust` is Clay's first-party Rust language package. Phase 18.14 expanded it from a grammar-only syntax highlighter into a full language package on generic Clay primitives.

## Package Contract

- Package name: `@clay/rust`
- API prefix: `rust`
- Language ID: `rust`
- Mode ID: `rust`
- Supported extensions: `.rs`
- Supported file names: `Cargo.toml`
- Grammar kind: `tree-sitter-wasm` (artifact `./grammars/rust.wasm`, source `tree-sitter-rust`)
- Highlight query: `./queries/highlights.scm`
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js` (exports `loadRustPackage`; re-exported from `./dist/index.js`)
- Documentation entry: `./docs/index.md`
- Capture-to-style-token map: `keyword` -> `keyword.control`, `string` -> `string.quoted`, `comment` -> `comment.line`, `punctuation` -> `punctuation.definition`
- Budgets: `5000 ms` parse timeout, `4096 byte` max parse window

## Phase 18.14 Surfaces

- **Major mode `rust`**: registered with generic file-extension/file-name probes.
- **Behavior manifest**: 4-space indentation, delimiter pairs, `//` comment continuation, electric `}` outdent.
- **Command `rust.toggleLineComment`**: server-first command registered through the Clay `CommandExecution` path.
- **Completion provider `rust.keywords`**: metadata-only provider with `.`/`::` triggers.
- **Status item `rust.status.mode`**: inert `statusItem` component contribution.

The existing Phase 18.10 grammar contribution is unchanged. Active syntax grammar remains selectable independently of active major mode, so a `.rs` file can use the Rust grammar while its major mode stays `core.code`.

## Configuration

Phase 18.14 keeps Rust editing defaults (4-space indentation, `//` line comments, delimiter pairs, electric `}` outdent, `.`/`::` autocomplete triggers) as package-defined values. No new user-tunable configuration keys are introduced in this phase. Future phases may expose documented, package-prefixed options through `clay.configuration.setPackageOption` after they are declared in `clay.contributions.packageOptions`.

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

await loadPackage("@clay/rust");
```

The package is not auto-loaded. Without this line, `.rs` files remain editable under `core.code` with no Rust-specific behavior.

## Engine tiers

- **Tier 1 native** is the default for `.rs` and `Cargo.toml`, using Clay's compiled `tree-sitter-rust` descriptor.
- **Tier 2 web-tree-sitter WASM** uses `./grammars/rust.wasm` only after `setSyntaxEnginePreference("rust", "wasm")`; package load order cannot override native highlighting.
- **Tier 3 JavaScript fallback** remains available through the server parse-handler path when `setSyntaxEnginePreference("rust", "javascript")` is selected or no grammar is available.

All tiers use one capture-to-`TokenType`/`Modifiers` vocabulary mapper. Open returns before parsing finishes; later failures publish sanitized `clay.parse.open_failed` diagnostics. See the [tiered syntax engine package-author contract](../../../docs/reference/packages/creating-packages.md#phase-1816-authoring-contract-tiered-syntax-engine).

## Validation

Grammar/query/style metadata, mode patterns, command metadata, completion provider metadata, and UI component trees are validated at package load time. Paths are package-root-confined `./` asset paths; grammar artifacts must be `tree-sitter-wasm`; query files must be `.scm`; style-map values must be known Clay style tokens. Parse/highlight work is background, cancellable, viewport-prioritized, and bounded by the shared parse/decor/cache budgets; it never runs in keypress, paint, layout, scroll, pointer, or text-event hot paths. Phase 18.16 retains the Phase 18.10 first-party-only rule and rejects arbitrary third-party/native grammar artifact loading; broader trust is deferred to Phase 23 and a separate security decision.
