# Syntax Grammar Registry

## Source

- `src/packages/record.rs`
- `src/server/syntax.rs`
- `runtime/js/syntax.ts`
- `src/server/ops/syntax.rs`
- `tests/syntax_grammar.rs`
- `tests/manual_smoke_docs.rs`
- `docs/development/launch-and-gui-smoke.md`
- `docs/reference/clay-js-api/syntax/server-register-syntax-grammar.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/packages/creating-packages.md`

## Overview

Phase 18.10 introduces `SyntaxGrammarContribution` as load-time package metadata for grammar-only syntax packages. The implementation stores validated Tree-sitter grammar descriptors separately from active major modes, so a document can remain editable through `core.code` or `core.text` while a package-provided grammar supplies syntax highlighting later.

## Package Record Validation

`assemble_package_record` now parses `clay.contributions.syntaxGrammars` into `SyntaxGrammarContributionDescriptor` values. Each descriptor retains package provenance through the enclosing `PackageRecord` and validates:

- package-owned optional `id` (default: `<apiPrefix>.<languageId>`)
- lowercase generic `languageId`
- `filePatterns.extensions` and/or `filePatterns.fileNames`
- `grammar.kind = "tree-sitter-wasm"`
- package-root-confined `grammar.path` ending in `.wasm`
- package-root-confined `.scm` query paths for `highlights`, optional `locals`, and optional `injections`
- capture-to-style-token `styleMap` using known Clay style tokens only
- optional `budgets.timeoutMs` and `budgets.maxWindowBytes`
- bounded contribution metadata under `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`
- required `parse-document` and `render-decorations` permissions

The validator rejects non-`@clay/*` syntax grammar packages in Phase 18.10, external URLs, absolute paths, parent traversal, native library fields, package-manager/download/shell fields, raw ops, client JavaScript, callbacks, CSS, raw colors, unknown style tokens, duplicate language IDs within a package, and duplicate contribution IDs.

## Registry State

`SyntaxGrammarRegistry` in `src/server/syntax.rs` is server-owned registry state for already validated descriptors. `register_package` stages all grammar contributions from a `PackageRecord`, checks deterministic conflicts, and commits only after the whole package contribution set is valid.

The registry indexes by:

- contribution ID
- language ID
- extension
- exact filename

It rejects duplicate contribution IDs, duplicate language IDs, duplicate extension claims, and duplicate filename claims with typed `SyntaxGrammarRegistryError` values. The registry stores provenance (`package_name`, `package_version`, `package_prefix`) beside grammar/query/style/budget metadata for diagnostics and future active-syntax selection.

## Active Syntax Grammar Selection

`SyntaxGrammarRegistry::select_for_document` records syntax highlighting selection separately from the active major mode. It receives the existing `DocumentClassificationInput`, the already-selected `MajorModeActivation`, and the current document version, then stores a `SyntaxGrammarSelection` keyed by document ID.

Selection uses only already-known open-document metadata: exact file name first, then extension. It does not recompute major-mode classification, mutate behavior version, change command routing, read the filesystem, run package JavaScript, or activate package modes. The stored selection copies the active major mode ID and behavior version only as diagnostics so discovery surfaces can explain states like:

```text
active_major_mode: core.code
active_syntax_grammar: rust from @clay/rust, selected by extension rs
behavior_version: unchanged from core.code activation
```

When no loaded grammar matches, `active_syntax_grammar` is `None` and the document remains editable through its existing `core.code`, `core.text`, or package major mode. Disabling or invalidating a grammar package therefore removes highlighting without changing editability.

## First-Party Grammar-Only Packages

Phase 18.10 ships three first-party grammar-only package scaffolds:

- `packages/rust/` -> `@clay/rust`, `apiPrefix = rust`, `languageId = rust`, extension `.rs`
- `packages/typescript/` -> `@clay/typescript`, `apiPrefix = typescript`, `languageId = typescript`, extensions `.ts`, `.tsx`
- `packages/javascript/` -> `@clay/javascript`, `apiPrefix = javascript`, `languageId = javascript`, extensions `.js`, `.jsx`, `.mjs`, `.cjs`

Each package declares exactly one `clay.contributions.syntaxGrammars` entry, a package-root-confined `tree-sitter-wasm` grammar path, a `./queries/highlights.scm` query, a capture-to-known-style-token `styleMap`, docs, performance metadata, and the `clay.syntax.serverRegisterSyntaxGrammar` API dependency. They request only `parse-document` and `render-decorations` and declare no modes, commands, completions, SDUI, UI, key routing, text transforms, behavior manifests, configuration, theme tokens, layout overrides, or package options.

The `dist/load.js` entries import `clay:syntax` and call `serverRegisterSyntaxGrammar` with the same inert grammar contract declared in package metadata. `loadPackage("@clay/<language>")` validates/enables the package through the existing resolver, canonicalizes the package-root-confined load entry, executes that load entry on the controlled runtime, and registers the grammar in runtime-local `SyntaxGrammarRegistry` state without exposing raw ops or executable callbacks to user config. The `loadPackage` summary reports `contributions.syntaxGrammars` so init.js fixture tests can verify that grammar-only packages loaded without declaring modes. Loading from `init.js` does not grant filesystem, network, shell, AI, WASM, raw-op, native-ui, client-runtime, package-manager, or package-control authority; grammar packages still receive only their declared `parse-document` and `render-decorations` permissions. User config remains one-line `loadPackage` setup; there are no hidden JSON/TOML/ad hoc syntax keys for preferred grammar selection, grammar paths, style maps, capture styles, or auto-load behavior.

End-user opt-in fixture:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
```

## Tree-sitter Parse/Highlight Handler

`TreeSitterSyntaxHandler` in `src/server/syntax.rs` is the server-side parse handler for validated grammar contributions. It is generic: callers provide the validated `SyntaxGrammarContribution`, a resolved `tree_sitter::Language`, and the package highlight query text. The handler:

1. compiles the highlight `Query` once at handler construction
2. creates and configures one `tree_sitter::Parser` per handler, reused across all parses for that grammar instead of recreating it per document open (avoids repeated wasm language instantiation in debug builds)
3. fails closed if a query capture is not present in the package `styleMap`
4. runs from `ParseCoordinator` as a `ParseHandler`
5. selects the viewport-intersecting `ParseWindowSnapshot`
6. enforces the contribution `maxWindowBytes` before parsing
7. sets parser/query timeouts from `timeoutMs`
8. reuses a cached prior `Tree` for later document versions on the same window
9. extracts query captures with `QueryCursor::set_byte_range`
10. maps captures to inert `DecorationSpan` values with package provenance
11. rejects per-viewport capture output above the syntax span cap instead of silently truncating query output
12. validates the resulting `DecorationSet` with the existing decoration validator
13. enforces `DECORATION_PAYLOAD_BUDGET_BYTES` before cache insertion or publication
14. inserts the validated set into the existing `SyntaxChunkCache` for near-viewport/LRU budget enforcement
15. returns an `IncrementalParseUpdate`

The handler publishes only through the existing parse/decor path: `ParseCoordinator::schedule_parse_with_windows` executes the handler in background work, validates stale document versions and payload budgets, then emits an `IncrementalParseUpdate` containing a validated `DecorationSet`. The existing `SyntaxChunkCache` enforces `SYNTAX_CACHE_BUDGET_BYTES` retention policy for validated syntax chunks. The client still receives the normal decoration transport message path; no package code runs in the Rust client.

## Hot Path and Security Boundary

Package metadata validation and registry insertion happen at package load/reload time. Tree-sitter parser/query work runs only inside parse coordinator `Background` no-hot-path tasks over server-prepared bounded parse windows. It is not called from keypress, paint, layout, scroll, pointer, or text-event handlers, and `tests/editor_performance_invariants.rs` statically guards paint/layout sources against Tree-sitter/package parser calls.

The handler does not load arbitrary paths, fetch network resources, run package managers/shells, call raw ops, or execute client-side JavaScript. Grammar artifacts must already be resolver-validated first-party package assets before a `tree_sitter::Language` reaches the handler.

The runtime [`clay:syntax.serverRegisterSyntaxGrammar`](../../reference/clay-js-api/syntax/server-register-syntax-grammar.md) facade/op registers the same inert grammar contract declared in `package.json` during the package load entry. User config still uses only one-line `loadPackage` calls; it does not perform manual primitive registration or raw op calls.

## Tests

Run:

```bash
CARGO_TARGET_DIR=target/pi-verify cargo test --test syntax_grammar --quiet
CARGO_TARGET_DIR=target/pi-verify cargo test --test parse_coordinator --quiet
CARGO_TARGET_DIR=target/pi-verify cargo test --test decoration_transport --quiet
CARGO_TARGET_DIR=target/pi-verify cargo test --test editor_performance_invariants --quiet
```

Coverage:

- valid grammar contribution provenance, asset paths, style map, and budgets
- required `parse-document` + `render-decorations` permissions
- rejection of third-party syntax grammar packages, external/traversing paths
- rejection of executable/native/client/CSS authority fields
- rejection of unknown style tokens/raw CSS
- registry provenance and extension lookup
- active syntax grammar selection separate from active major mode and behavior version
- no-grammar fallback that preserves editability through the active major mode
- first-party grammar-only package validation for `@clay/rust`, `@clay/typescript`, and `@clay/javascript`
- real fixture highlight smoke for `tests/fixtures/syntax/rust.rs`, `typescript.ts`, and `javascript.js` producing bounded `DecorationSet` values through the generic handler
- deterministic manual-smoke surrogate `manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow`: loads all three grammar packages, selects `.rs`/`.ts`/`.js` syntax grammars while preserving `core.code`, parses before/after a small edit, and verifies unloaded no-highlight fallback editability
- explicit `syntax-grammars-init.js` one-line load fixture and `tests/fixtures/configuration/syntax-grammars/init.js` smoke-gui fixture
- deterministic duplicate language and duplicate pattern conflicts
- Tree-sitter highlight query capture extraction into bounded decoration spans
- valid capture mapping to `keyword.control`, `string.quoted`, `comment.line`, and `punctuation.definition`
- cached-tree reuse for later document versions
- invalid query/unmapped capture fail-closed behavior with actionable diagnostics
- parse-window budget enforcement before parsing
- per-viewport capture overflow rejection before decoration publication
- parse coordinator publication through existing `IncrementalParseUpdate`/`DecorationSet` path

## Related

- [Phase 18.10 Tree-sitter Grammar Primitive Review](phase18.10-tree-sitter-grammar-primitive-review.md)
- [Package Loading](package-loading.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
