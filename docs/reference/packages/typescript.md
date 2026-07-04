# @clay/typescript Package

`@clay/typescript` is Clay's first-party grammar-only TypeScript syntax highlighting package. Its source scaffold lives under `packages/typescript/`.

## End-User Setup

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/typescript");
```

The package is explicit opt-in and is not auto-loaded. Without this line, `.ts` and `.tsx` files remain editable under `core.code` with no TypeScript syntax grammar.

## Contract

- `package.json` name: `@clay/typescript`
- Clay API prefix: `typescript`
- Language ID: `typescript`
- File patterns: `.ts`, `.tsx`
- Docs path: `./docs/index.md`
- Entries: `./dist/index.js`, `./dist/load.js`
- Grammar contribution: `tree-sitter-wasm` at `./grammars/typescript.wasm`
- Highlight query: `./queries/highlights.scm`
- Style map: `keyword` -> `keyword.control`, `string` -> `string.quoted`, `comment` -> `comment.line`, `punctuation` -> `punctuation.definition`
- Budgets: `timeoutMs = 5000`, `maxWindowBytes = 4096`; published decorations remain bounded by `DECORATION_PAYLOAD_BUDGET_BYTES` and cached syntax chunks by `SYNTAX_CACHE_BUDGET_BYTES`
- API dependency: `clay.syntax.serverRegisterSyntaxGrammar`

## Grammar-Only Scope

The package declares no major mode, no commands, no completions, SDUI, UI panels/components/overlays, key routing, text transforms, behavior manifests, configuration, theme tokens, layout overrides, or package options. Syntax grammar selection is separate from active major mode, so TypeScript files can stay `core.code` while using the TypeScript grammar for decorations.

## Security and Performance

Permissions are limited to `parse-document` and `render-decorations`. The package does not request filesystem, network, shell, AI, WASM-authority, raw-op, native-ui, client-runtime, package-manager, package-control, or workspace mutation authority. Grammar metadata is validated at package load/reload time. Parse/highlight work runs as background, cancellable, viewport-prioritized server work and never in keypress, paint, layout, scroll, pointer, or text-event hot paths; output is viewport-bounded by the server syntax pipeline. Phase 18.10 accepts grammar contributions from first-party `@clay/*` packages only and rejects arbitrary third-party/native grammar artifact loading.
