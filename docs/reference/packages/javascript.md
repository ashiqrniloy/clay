# @clay/javascript Package

`@clay/javascript` is Clay's first-party JavaScript language package. Phase 18.14 expanded it from a grammar-only syntax highlighter into a full language package: it contributes a `javascript` major mode, behavior manifest, package-prefixed command, keyword completion provider, and an optional status-item UI contribution while keeping the Tree-sitter grammar contribution from Phase 18.10 unchanged.

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
- Grammar contribution: `tree-sitter-wasm` at `./grammars/javascript.wasm`
- Highlight query: `./queries/highlights.scm`
- Style map: `keyword` -> `keyword.control`, `string` -> `string.quoted`, `comment` -> `comment.line`, `punctuation` -> `punctuation.definition`
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
- **Behavior manifest**: indentation (2 spaces), delimiter pairs, line-comment continuation (`//`), and an electric `}` outdent rule. These are expressed through generic `EditorBehaviorRules` primitives.
- **Command `javascript.toggleLineComment`**: a server-first command registered for validated execution through the Clay `CommandExecution` path.
- **Completion provider `javascript.keywords`**: metadata-only provider with `.` trigger and bounded item/timeout budgets.
- **Status item `javascript.status.mode`**: an inert `statusItem` component contribution validated by Clay before client publication.

Active syntax grammar remains selectable independently of active major mode, so a `.js` file can use the JavaScript grammar while its major mode is still `core.code`, and loading the package does not silently change the mode of already-open documents.

## Configuration

Phase 18.14 keeps JavaScript editing defaults (2-space indentation, `//` line comments, delimiter pairs, electric `}` outdent, `.` autocomplete trigger) as package-defined values. No new user-tunable configuration keys are introduced in this phase. Future phases may expose documented, package-prefixed options through `clay.configuration.setPackageOption` (for example, `javascript.indentSize`) after they are declared in `clay.contributions.packageOptions`.

## Security and Performance

Permissions are limited to `mode-registration`, `command-registration`, `completion-provider`, `parse-document`, and `render-decorations`. The package does not request filesystem, network, shell, AI, WASM-authority, raw-op, native-ui, client-runtime, package-manager, package-control, or workspace mutation authority.

Grammar metadata, mode patterns, commands, completion provider metadata, and UI component trees are validated at package load/reload time. Parse/highlight work runs as background, cancellable, viewport-prioritized server work and never in keypress, paint, layout, scroll, pointer, or text-event hot paths. Phase 18.10 accepts grammar contributions from first-party `@clay/*` packages only and rejects arbitrary third-party/native grammar artifact loading. The same package-root confinement and first-party-only rule applies to the Tree-sitter grammar asset used by this language package.

LSP, full language-server protocol integration, workspace-wide symbol indexes, AI completions, network-backed completions, and mutating toolchain execution are out of scope for Phase 18.14.
