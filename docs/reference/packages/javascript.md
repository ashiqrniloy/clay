# @clay/javascript Package

`@clay/javascript` is Clay's first-party JavaScript language package. Phase 18.14 expanded it from a grammar-only syntax highlighter into a full language package: it contributes a `javascript` major mode, behavior manifest, package-prefixed command, keyword completion provider, and an optional status-item UI contribution with Phase 18.18 native grammar metadata and a direct vocabulary styleMap.

## End-User Setup

Default `~/.config/clay/init.js` loading is one explicit line:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/javascript");
```

The package is explicit opt-in and is not auto-loaded. Without this line, `.js`/`.jsx`/`.mjs`/`.cjs` files remain editable under `core.code` with no JavaScript syntax grammar or JavaScript-specific editing behavior.

Optional customization is exposed through documented Clay/package JS APIs. For example, after `loadPackage("@clay/javascript")` a user can bind `javascript.toggleLineComment` to a key or toggle a package option; the default load line itself never needs to inline the package manifest.

## Contract

- `package.json` name: `@clay/javascript`
- Clay API prefix: `javascript`
- Language ID: `javascript`
- Mode ID: `javascript`
- File patterns: `.js`, `.jsx`, `.mjs`, `.cjs`
- Docs path: `./docs/index.md`
- Entries: `./dist/index.js`, `./dist/load.js`
- Grammar contribution: `native`, source `tree-sitter-javascript` (no package `.wasm` asset)
- Highlight query: `./queries/highlights.scm`
- Vocabulary styleMap: JS/JSX/MJS/CJS captures map directly to closed `TokenType` + `Modifiers`, including `Function + Declaration`
- Budgets: `timeoutMs = 5000`, `maxWindowBytes = 4096`; published decorations remain bounded by `DECORATION_PAYLOAD_BUDGET_BYTES` and cached syntax chunks by `SYNTAX_CACHE_BUDGET_BYTES`
- API dependencies:
  - `clay.syntax.serverRegisterSyntaxGrammar`
  - `clay.modes.serverRegisterModePattern`
  - `clay.commands.serverRegisterCommand`
  - `clay.completion.serverRegisterCompletionProvider`
  - `clay.ui.serverRegisterComponentContribution`

## Phase 18.14 Language Package Surfaces

The package now declares:

- **Major mode `javascript`**: registered with generic file-extension probes (`*.js`, `*.jsx`, `*.mjs`, `*.cjs`). No JavaScript-specific classification branch lives in Clay core.
- **Behavior manifest**: indentation (2 spaces), bracket/quote/template-literal pairs, line-comment continuation (`//`), and electric `}`/`)`/`]` outdent rules. These are expressed through generic `EditorBehaviorRules` primitives.
- **Command `javascript.toggleLineComment`**: a server-first command registered for validated execution through the Clay `CommandExecution` path.
- **Completion provider `javascript.keywords`**: priority-0 metadata-only provider with 32 inert JavaScript keyword text replacements, `.` trigger, language-appropriate boundaries, and 300 ms/32-item budgets. Snippet transforms remain deferred to Phase 18.19.
- **Status item `javascript.status.mode`**: an inert `statusItem` component contribution validated by Clay before client publication.

Active syntax grammar remains selectable independently of active major mode, so a `.js` file can use the JavaScript grammar while its major mode is still `core.code`, and loading the package does not silently change the mode of already-open documents.

## Configuration

Phase 18.18 keeps JavaScript editing defaults (2-space indentation, `//` line comments, bracket/quote/template-literal pairs, electric `}`/`)`/`]` outdent, and `.` autocomplete trigger) as package-defined values. No new user-tunable configuration keys are introduced in this phase. Future phases may expose documented, package-prefixed options through `clay.configuration.setPackageOption` (for example, `javascript.indentSize`) after they are declared in `clay.contributions.packageOptions`.

## Phase 18.16 syntax engine artifacts

Tier 1 native highlighting uses Clay's compiled `tree-sitter-javascript = 0.25.0` dependency and the package query at `packages/javascript/queries/highlights.scm`. Tier 2 remains available to an explicitly selected package that supplies valid confined WASM metadata; `@clay/javascript` itself ships native metadata only, and package load order cannot replace native Tier 1. Tier 3 remains the server-side package-JS parse-handler route when `setSyntaxEnginePreference("javascript", "javascript")` is selected or no grammar is available. All routes map captures through the shared `TokenType` + `Modifiers` vocabulary pipeline. Until a WASM binary is committed, `packages/javascript/grammars/PROVENANCE.md` records the reproducible build command and required SHA-256 recording step. Runtime never fetches, builds, shells out, or loads native libraries for this artifact.

## Typography

JavaScript mode declares semantic `defaultFontRole: "monospace"`. User typography owns the selected family fallback stack and size; package metadata supplies no concrete font values. See [Semantic Typography Roles](../primitives/typography.md).

## Security and Performance

Permissions are limited to `mode-registration`, `command-registration`, `completion-provider`, `parse-document`, and `render-decorations`. The package does not request filesystem, network, shell, AI, WASM-authority, raw-op, native-ui, client-runtime, package-manager, package-control, or workspace mutation authority.

Grammar metadata, mode patterns, commands, completion provider metadata, and UI component trees are validated at package load/reload time. Open returns before parse completion; background failures publish sanitized `clay.parse.open_failed` diagnostics and do not block editing. Parse/highlight work runs as background, cancellable, viewport-prioritized server work and never in keypress, paint, layout, scroll, pointer, or text-event hot paths. Phase 18.16 retains the Phase 18.10 first-party-only policy and rejects arbitrary third-party/native grammar artifact loading; broader third-party trust is deferred to Phase 23 and a separate security decision. The same package-root confinement and no-runtime-download/no-shell rule applies to every Tree-sitter grammar asset used by this language package.

LSP, full language-server protocol integration, workspace-wide symbol indexes, AI completions, network-backed completions, and mutating toolchain execution are out of scope for Phase 18.14.
