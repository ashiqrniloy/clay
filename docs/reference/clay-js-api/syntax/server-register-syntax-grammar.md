---
id: clay.syntax.serverRegisterSyntaxGrammar
kind: clay-js-api
js_module: "clay:syntax"
js_export: serverRegisterSyntaxGrammar
js_facade: runtime/js/syntax.ts::serverRegisterSyntaxGrammar
backing_rust: src/server/syntax.rs::SyntaxGrammarRegistry::register_package
deno_op: op_clay_syntax_register_syntax_grammar
deno_op_path: src/server/ops/syntax.rs::op_clay_syntax_register_syntax_grammar
name: serverRegisterSyntaxGrammar
user_facing_name: Register Syntax Grammar
summary: Register first-party package-provided Tree-sitter grammar metadata for server-side syntax highlighting.
owner: server
phase: Phase 18.10
visibility: public
permissions: ['parse-document', 'render-decorations']
key_bindings: []
custom_properties:
  - name: packageManifest
    type: object
    default: optional
    description: Full package.json-shaped manifest; when provided, Clay validates its syntaxGrammars metadata directly.
  - name: packageName
    type: string
    default: required-without-packageManifest
    description: First-party package name such as @clay/rust.
  - name: packagePrefix
    type: string
    default: required-without-packageManifest
    description: Package apiPrefix used for contribution ownership and provenance.
  - name: permissions
    type: string[]
    default: required-without-packageManifest
    description: Must include parse-document and render-decorations.
  - name: syntaxGrammar
    type: object
    default: required-without-packageManifest
    description: Inert syntax grammar contribution descriptor matching clay.contributions.syntaxGrammars.
  - name: languageId
    type: string
    default: required-inside-syntaxGrammar
    description: Lowercase language identifier selected independently from the active major mode.
  - name: filePatterns
    type: object
    default: required-inside-syntaxGrammar
    description: Extensions or exact file names that select this grammar.
  - name: grammar
    type: object
    default: required-inside-syntaxGrammar
    description: Native Tier 1 source ID with no path, or a Tier 2 tree-sitter-wasm artifact path confined to the package root.
  - name: queries
    type: object
    default: required-inside-syntaxGrammar
    description: Package-root-confined highlights .scm query path.
  - name: styleMap
    type: object
    default: required-inside-syntaxGrammar
    description: Tree-sitter capture names mapped to closed TokenType + Modifiers vocabulary objects; known legacy Clay style tokens remain compatible.
  - name: budgets
    type: object
    default: optional
    description: timeoutMs and maxWindowBytes metadata bounded by shared syntax budgets.
security: Requires parse-document and render-decorations permissions with server-side validation of first-party @clay/* package provenance, compiled-in native source IDs or package-root-confined tree-sitter-wasm paths, confined .scm paths, closed vocabulary/validated legacy style maps, duplicate registry conflicts, and Background/no-hot-path parse/decor scheduling. does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, arbitrary native/WASM loading, or client-side JavaScript authority; rejects raw ops, executable callbacks, arbitrary artifact paths, URLs, parent traversal, native handles/libraries, raw CSS/colors, package-manager/download fields, and third-party native grammar loading.
agent_guidance: Use `clay.syntax.serverRegisterSyntaxGrammar` only from first-party grammar package load entries after manifest validation. Prefer `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, or `loadPackage("@clay/javascript")` for ordinary user setup. Do not expose raw Deno ops or register arbitrary third-party/native grammars in this phase.
lookup_tags: [js-api, syntax, tree-sitter, grammar, highlighting]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterSyntaxGrammar

## Summary

Registers package-provided syntax grammar metadata with Clay's server-side syntax registry. The API is for first-party language packages such as `@clay/rust`, `@clay/typescript`, `@clay/javascript`, and `@clay/markdown`; ordinary users load those packages with `loadPackage(...)` from `~/.config/clay/init.js` rather than calling this registration API directly.

## Description

`serverRegisterSyntaxGrammar` is the public Clay JS API for the Phase 18.10 `SyntaxGrammarContribution` primitive. It validates inert Tree-sitter grammar metadata and inserts it into `SyntaxGrammarRegistry`. Active syntax grammar selection remains independent from active major mode: a document may stay in `core.code` or `core.text` while a loaded grammar package supplies highlighting.

The API is runtime-backed by a `deno_core` op wrapper, but callers never touch raw `Deno.core.ops` or other raw Deno ops. The facade rejects executable callback/raw-authority fields before the op validates the package-shaped metadata through the same package record assembler used by package loading.

## When to use

Use this API from a first-party grammar package load entry when the package declares `clay.contributions.syntaxGrammars` metadata and wants that metadata available for syntax provider selection. End-user configuration should use one-line package loading instead:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
```

## JavaScript usage

```ts
import { serverRegisterSyntaxGrammar } from "clay:syntax";

serverRegisterSyntaxGrammar({
  packageName: "@clay/rust",
  packageVersion: "0.1.0",
  packagePrefix: "rust",
  permissions: ["parse-document", "render-decorations"],
  syntaxGrammar: {
    languageId: "rust",
    filePatterns: { extensions: ["rs"] },
    grammar: { kind: "native", source: "tree-sitter-rust" },
    queries: { highlights: "./queries/highlights.scm" },
    styleMap: {
      keyword: { type: "Keyword" },
      string: { type: "String" },
      comment: { type: "Comment" },
      punctuation: { type: "Operator" },
      "function.declaration": {
        type: "Function",
        modifiers: ["Declaration"]
      }
    },
    budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
  }
});
```

## Example

A first-party grammar-only package load entry can export the same registration object it passes to the facade:

```ts
import { serverRegisterSyntaxGrammar } from "clay:syntax";

export default function loadRustGrammar() {
  return serverRegisterSyntaxGrammar({
    packageName: "@clay/rust",
    packageVersion: "0.1.0",
    packagePrefix: "rust",
    permissions: ["parse-document", "render-decorations"],
    syntaxGrammar: {
      languageId: "rust",
      filePatterns: { extensions: ["rs"] },
      grammar: { kind: "native", source: "tree-sitter-rust" },
      queries: { highlights: "./queries/highlights.scm" },
      styleMap: { keyword: { type: "Keyword" } }
    }
  });
}
```

## Options

- `packageManifest` (`object`, optional): Full package manifest containing `clay.contributions.syntaxGrammars`. If provided, Clay validates and registers that manifest's grammar contributions.
- `packageName`, `packageVersion`, `packagePrefix`/`apiPrefix`, `permissions`: Package context fields used when a load entry passes one grammar descriptor instead of a full manifest. `packageName` must be first-party `@clay/*` in Phase 18.10.
- `syntaxGrammar` / `contribution`: A syntax grammar contribution descriptor. Top-level `languageId`, `filePatterns`, `grammar`, `queries`, `styleMap`, and `budgets` are also accepted and normalized into a descriptor.
- `languageId`: Lowercase identifier such as `rust`, `typescript`, or `javascript`.
- `filePatterns`: Bare extensions and/or exact file names used for deterministic selection.
- `grammar`: Tier 1 uses `{ kind: "native", source: "tree-sitter-rust" }` with no `path`; Tier 2 uses `{ kind: "tree-sitter-wasm", path: "./.../*.wasm" }` with package-root confinement.
- `queries`: Must include `highlights: "./.../*.scm"`; optional locals/injections query paths use the same confinement.
- `styleMap`: Capture names without `@` map to vocabulary objects such as `{ type: "Function", modifiers: ["Declaration"] }`; `type` must be a closed `TokenType` variant and modifiers must be closed `Modifiers` names. Known legacy style-token strings remain accepted for compatibility.
- `budgets`: Optional `timeoutMs` and `maxWindowBytes`; actual parse/highlight and decoration transport remain bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`.

## Key bindings

No default key binding is assigned.

## Custom properties

- `packagePrefix`: package `apiPrefix` used for contribution ownership and decoration provenance.
- `permissions`: package permissions; must include `parse-document` and `render-decorations`.
- `languageId`: syntax provider selection identity.
- `filePatterns`: extension/file-name selection contract.
- `grammar`: compiled-in native source metadata or package-root-confined Tier 2 `tree-sitter-wasm` artifact metadata.
- `queries`: package-root-confined highlight query metadata.
- `styleMap`: capture-to-`TokenType`/`Modifiers` vocabulary map with validated legacy compatibility.
- `budgets`: load-time syntax parse budget metadata.

## Return and async behavior

Returns registration metadata synchronously: package name/version/prefix, registered grammar count, and registered language IDs. Later parsing and highlighting run as cancellable `Background` parse coordinator work and never on keypress, paint, layout, scroll, pointer, or text-event hot paths.

## Errors

Fails with Clay error codes when package metadata is malformed, permissions are missing, the package is not first-party, native source/query or WASM asset paths are invalid, grammar kind is neither `native` nor `tree-sitter-wasm`, token types/modifiers or legacy style tokens are unknown, raw CSS/colors appear, duplicate language/file-pattern registrations conflict, or executable/raw-authority fields are present.

## Permissions and security

Requires: `parse-document` and `render-decorations`.

Clay performs server-side validation of first-party package provenance, declared permissions, native source or package-root-confined WASM/query metadata, closed vocabulary maps (plus validated legacy compatibility), and duplicate registry conflicts. This API does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority; it also rejects raw ops, native UI handles, client-runtime hooks, executable callbacks, and arbitrary native/WASM artifact loading. Phase 18.10 grammar loading is first-party-only and rejects arbitrary third-party/native grammar artifact loading.

## Agent guidance

Prefer one-line `loadPackage("@clay/<language>")` for user setup. Package authors should declare the grammar in `package.json` and call this facade from a load entry only with the same inert metadata; do not copy manifests into user config, call raw ops, pass callbacks, or add hidden syntax configuration keys.

## Lookup metadata

- Stable ID: `clay.syntax.serverRegisterSyntaxGrammar`
- Module/export: `clay:syntax` / `serverRegisterSyntaxGrammar`
- User-facing name: Register Syntax Grammar
- Tags: `js-api`, `syntax`, `tree-sitter`, `grammar`, `highlighting`

## Backing implementation

- JS facade: `runtime/js/syntax.ts::serverRegisterSyntaxGrammar`
- Runtime facade: `src/server/js_runtime.rs::CLAY_FACADE_SYNTAX`
- Op wrapper: `src/server/ops/syntax.rs::op_clay_syntax_register_syntax_grammar`
- Registry: `src/server/syntax.rs::SyntaxGrammarRegistry::register_package`
- Package validation: `src/packages/record.rs::assemble_package_record`
- Tests: `src/server/js_runtime.rs::syntax_facade_registers_grammar_metadata_without_raw_ops`, `tests/clay_js_doc_registry.rs`, `tests/clay_js_api_inventory.rs`, `tests/syntax_grammar.rs`
