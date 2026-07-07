# @clay/typescript

`@clay/typescript` is Clay's first-party TypeScript language package. Phase 18.14 expanded it from a grammar-only syntax highlighter into a full language package on generic Clay primitives.

## Package Contract

- Package name: `@clay/typescript`
- API prefix: `typescript`
- Language ID: `typescript`
- Mode ID: `typescript`
- Supported extensions: `.ts`, `.tsx`, `.mts`, `.cts`
- Grammar kind: `tree-sitter-wasm` (artifact `./grammars/typescript.wasm`, source `tree-sitter-typescript`)
- Highlight query: `./queries/highlights.scm`
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js` (exports `loadTypescriptPackage`; re-exported from `./dist/index.js`)
- Documentation entry: `./docs/index.md`
- Capture-to-style-token map: `keyword` -> `keyword.control`, `string` -> `string.quoted`, `comment` -> `comment.line`, `punctuation` -> `punctuation.definition`
- Budgets: `5000 ms` parse timeout, `4096 byte` max parse window

## Phase 18.14 Surfaces

- **Major mode `typescript`**: registered with generic file-extension probes.
- **Behavior manifest**: 2-space indentation, delimiter pairs, `//` comment continuation, electric `}` outdent.
- **Command `typescript.toggleLineComment`**: server-first command registered through the Clay `CommandExecution` path.
- **Completion provider `typescript.keywords`**: metadata-only provider with `.` trigger.
- **Status item `typescript.status.mode`**: inert `statusItem` component contribution.

The existing Phase 18.10 grammar contribution is unchanged. Active syntax grammar remains selectable independently of active major mode, so a `.ts` file can use the TypeScript grammar while its major mode stays `core.code`. Loading the package does not silently change the mode of already-open documents.

## Configuration

Phase 18.14 keeps TypeScript editing defaults (2-space indentation, `//` line comments, delimiter pairs, electric `}` outdent, `.` autocomplete trigger) as package-defined values. No new user-tunable configuration keys are introduced in this phase. Future phases may expose documented, package-prefixed options through `clay.configuration.setPackageOption` after they are declared in `clay.contributions.packageOptions`.

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

await loadPackage("@clay/typescript");
```

The package is not auto-loaded. Without this line, `.ts`/`.tsx` files remain editable under `core.code` with no TypeScript-specific behavior.

## Validation

Grammar/query/style metadata, mode patterns, command metadata, completion provider metadata, and UI component trees are validated at package load time. Paths are package-root-confined `./` asset paths; grammar artifacts must be `tree-sitter-wasm`; query files must be `.scm`; style-map values must be known Clay style tokens. Parse/highlight work is background, cancellable, viewport-prioritized, and bounded by the shared parse/decor/cache budgets; it never runs in keypress, paint, layout, scroll, pointer, or text-event hot paths. Phase 18.10 accepts syntax grammar contributions from first-party `@clay/*` packages only and rejects arbitrary third-party/native grammar artifact loading.
