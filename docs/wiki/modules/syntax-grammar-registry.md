# Syntax Grammar Registry

## Source

- `src/packages/record.rs`
- `src/server/syntax.rs`
- `runtime/js/syntax.ts`
- `runtime/js/web-tree-sitter-host.ts`
- `src/server/ops/syntax.rs`
- `tests/syntax_grammar.rs`
- `tests/parse_coordinator.rs`
- `tests/manual_smoke_docs.rs`
- `docs/development/launch-and-gui-smoke.md`
- `packages/{rust,typescript,javascript,markdown}/queries/`
- `packages/{rust,typescript,javascript,markdown}/grammars/PROVENANCE.md`
- `docs/reference/clay-js-api/syntax/server-register-syntax-grammar.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/clay-js-api/syntax/set-syntax-engine-preference.md`
- `docs/reference/packages/creating-packages.md`

## Overview

Phase 18.16 turns the Phase 18.10 `SyntaxGrammarContribution` metadata into a tiered syntax engine. The server stores validated grammar descriptors separately from active major modes, selects native first-party Tree-sitter by default, supports explicit web-tree-sitter WASM selection, and retains package-JavaScript parsing as Tier 3 fallback. All engines feed one bounded capture-to-`TokenType`/`Modifiers` decoration path, so a document remains editable through `core.code`, `core.text`, or its package mode when highlighting is absent or deferred.

## Primitive Coverage

- **`SyntaxGrammarContribution` / `SyntaxGrammarRegistry`** — owned by `src/server/syntax.rs`; package metadata and registry state carry provenance, `SyntaxEngineTier`, grammar/query paths, style maps, and bounded parse budgets.
- **Clay JS boundary** — `clay:syntax.serverRegisterSyntaxGrammar` registers inert grammar metadata; `clay:syntax.setSyntaxEnginePreference` records an explicit user tier preference. Public usage is documented in [`server-register-syntax-grammar`](../../reference/clay-js-api/syntax/server-register-syntax-grammar.md) and [`set-syntax-engine-preference`](../../reference/clay-js-api/syntax/set-syntax-engine-preference.md).
- **Parse/decorations primitives** — `ParseCoordinator` owns background scheduling, cancellation, stale-result rejection, and sanitized diagnostics; `DecorationSet`/`DecorationSpan` owns bounded inert output and `StyleRegistry` resolves colors during native paint. Syntax packages require `parse-document` and `render-decorations`.
- **Reuse rule** — future language packages add descriptor/package data and queries; they reuse the registry, host adapter, mapper, coordinator, and decoration transport rather than adding language-specific Rust/client branches.

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

`SyntaxGrammarRegistry` in `src/server/syntax.rs` is server-owned registry state for already validated descriptors. Phase 18.16 adds Tier 1 native first-party registration: `SyntaxGrammarRegistry::with_first_party_native()` seeds compiled-in descriptors at server runtime startup for Rust, TypeScript, TSX, JavaScript/JSX/MJS/CJS, and Markdown before package load entries run. `register_package` still stages package grammar contributions from a `PackageRecord`, checks deterministic conflicts, and commits only after the whole package contribution set is valid; if a first-party WASM grammar contribution is fully shadowed by the matching native Tier 1 descriptor, it is skipped instead of conflicting so the rest of the language package can load. Explicit Tier 2 selection uses `register_package_with_explicit_tier2_override`, which removes only the matching first-party native descriptor before inserting the WASM contribution.

The registry indexes by:

- contribution ID
- language ID
- extension
- exact filename

It rejects duplicate contribution IDs, duplicate language IDs, duplicate extension claims, and duplicate filename claims with typed `SyntaxGrammarRegistryError` values. The registry stores provenance (`package_name`, `package_version`, `package_prefix`) beside grammar/query/style/budget metadata, `SyntaxEngineTier`, and native descriptor lookup data for diagnostics and active-syntax selection.

## Active Syntax Grammar Selection

`SyntaxGrammarRegistry::select_for_document` records syntax highlighting selection separately from the active major mode. It receives the existing `DocumentClassificationInput`, the already-selected `MajorModeActivation`, and the current document version, then stores a `SyntaxGrammarSelection` keyed by document ID.

Selection uses only already-known open-document metadata: exact file name first, then extension. With the native registry seeded, known first-party extensions select Tier 1 native descriptors by default. It does not recompute major-mode classification, mutate behavior version, change command routing, read the filesystem, run package JavaScript, or activate package modes. The stored selection copies the active major mode ID and behavior version only as diagnostics so discovery surfaces can explain states like:

```text
active_major_mode: core.code
active_syntax_grammar: rust from @clay/rust, selected by extension rs
behavior_version: unchanged from core.code activation
```

When no loaded grammar matches, `active_syntax_grammar` is `None` and the document remains editable through its existing `core.code`, `core.text`, or package major mode. Disabling or invalidating a grammar package therefore removes highlighting without changing editability.

## First-Party Language Packages and Native Descriptors

Phase 18.16 ships expanded first-party language packages plus a compiled-in native descriptor for each supported language:

- `packages/rust/` -> `@clay/rust`, `apiPrefix = rust`, `languageId = rust`, extension `.rs`
- `packages/typescript/` -> `@clay/typescript`, `apiPrefix = typescript`, `languageId = typescript`, extensions `.ts`, `.tsx`
- `packages/javascript/` -> `@clay/javascript`, `apiPrefix = javascript`, `languageId = javascript`, extensions `.js`, `.jsx`, `.mjs`, `.cjs`
- `packages/markdown/` -> `@clay/markdown`, full Markdown mode package with a native `.md`/`.markdown`/`.mdown` descriptor and existing markdown-it Tier 3 parser fallback

Rust, TypeScript, and JavaScript packages declare package-root-confined `tree-sitter-wasm` metadata, `./queries/highlights.scm`, known style maps, permissions, and full mode contributions. Markdown keeps its full mode/package-JavaScript parser contract while its Tree-sitter descriptor and query are owned by the native first-party registry. All syntax output still requires `parse-document` and `render-decorations`; package loading does not grant filesystem, network, shell, AI, native-library, raw-op, client-JavaScript, package-manager, or package-control authority.

The `dist/load.js` entries import `clay:syntax` and register only inert grammar metadata. `loadPackage("@clay/<language>")` validates/enables package data through the existing resolver, executes the load entry on the controlled runtime, and registers package contributions without exposing raw ops or executable callbacks to user config. Matching first-party WASM metadata is normally shadowed by the native descriptor; an explicit `setSyntaxEnginePreference("<language>", "wasm")` selects Tier 2, while `javascript` retains package parser fallback. User config remains one-line `loadPackage` setup unless it explicitly requests a tier; hidden JSON/TOML syntax keys are not supported.

End-user opt-in fixture:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");
```

## Tier 1 Native First-Party Registration

Tier 1 native descriptors are static data, not control-flow branches. Each descriptor records package identity, contribution ID, language ID, extensions/file names, grammar source, query path, style map, `SyntaxEngineTier::Native`, and a compiled-in `tree_sitter::Language` constructor. Current descriptors:

- `@clay/rust`: `.rs` via `tree-sitter-rust`.
- `@clay/typescript`: `.ts` via `tree-sitter-typescript`.
- `@clay/typescript`: `.tsx` via the TSX grammar from `tree-sitter-typescript`.
- `@clay/javascript`: `.js`, `.jsx`, `.mjs`, `.cjs` via `tree-sitter-javascript`.
- `@clay/markdown`: `.md`, `.markdown`, `.mdown` via `tree-sitter-md-025`.

`ClayOpState::new_for_document` uses `SyntaxGrammarRegistry::with_first_party_native()` so runtime op state starts with Tier 1 native entries. `SyntaxGrammarRegistry::native_language()` exposes the compiled language for later generic handler construction; parser instances remain cached inside `TreeSitterSyntaxHandler` per contribution.

## Tier 2 Web-tree-sitter Artifact Contract

`SyntaxGrammarContribution::web_tree_sitter_artifact_contract()` turns a validated `tree-sitter-wasm` contribution into a host adapter contract containing only inert package provenance plus package-root-confined `./grammars/*.wasm` and `./queries/*.scm` paths. It rejects native contributions, non-WASM grammar kinds, parent traversal, external URLs, absolute paths, and non-query/non-WASM suffixes.

`runtime/js/web-tree-sitter-host.ts` is the internal host adapter scaffold. It follows the upstream `web-tree-sitter` shape (`Parser.init`, `Language.load`), pins the runtime wasm lookup to `clay://runtime/tree-sitter.wasm`, caches language/query initialization, validates the same package-local path contract, and intentionally contains no `fetch`, HTTP URL, npm/package-manager, shell, raw-op, native-handle, or client-JavaScript authority. It returns capture records for the shared Rust-side syntax decoration pipeline; package load entries do not get per-language wrappers.

## Tier 3 JavaScript Fallback and Engine Preference

Tier 3 is intentionally not another grammar registry path. Existing package JavaScript parse handlers registered through `clay:parse.serverRegisterParseHandler` remain the fallback for grammar-less languages or packages that force `javascript` engine preference. When no syntax grammar is selected, the active major mode and behavior version stay untouched, so the package parse handler can still publish decorations/status through the existing parse/decor transport.

`clay:syntax.setSyntaxEnginePreference(target, tier)` records user-initiated engine choice in `SyntaxGrammarRegistry` at init/package-load time only. Targets match a language id, package `apiPrefix`, or package name. Valid tiers are `native`, `wasm`, and `javascript` (`js` alias). Default is no preference: seeded Tier 1 native wins for first-party languages; first-party WASM metadata is shadowed. A forced `wasm` preference allows a matching package grammar to replace the native descriptor through the existing explicit Tier 2 override path. A forced `javascript` preference suppresses syntax-grammar selection and leaves highlighting to the package JS parse handler. Packages cannot silently self-promote over native tier because ordinary `serverRegisterSyntaxGrammar` calls still register through `register_package` and only see override behavior when the user preference already exists.

Selection diagnostics include the chosen tier in `SyntaxGrammarSelection::why` and `ActiveSyntaxGrammar::engine_tier`. Preference lookup and package registration are load/open/reclassification work; no preference or engine selection code runs from paint, layout, scroll, pointer, or key/text-event handlers.

## First-Party Query and Artifact Provenance

Phase 18.16 ships real first-party highlight queries for Rust, TypeScript, TSX, JavaScript, and Markdown under `packages/*/queries/highlights.scm`. These queries emit only captures present in the package/native `styleMap`, so the shared mapper produces Phase 18.15 vocabulary tokens and modifiers instead of legacy free-form-only families. Smoke fixtures live in `tests/fixtures/syntax/{rust.rs,typescript.ts,typescript.tsx,javascript.js,markdown.md}` and are parsed through `TreeSitterSyntaxHandler` in `tests/syntax_grammar.rs::first_party_language_fixtures_produce_themed_vocabulary_decorations`.

Tier 2 WASM binaries are not committed yet; each `packages/*/grammars/PROVENANCE.md` records the exact upstream crate/release used by Tier 1, a reproducible `tree-sitter build --wasm` command, and the required SHA-256 recording step for the eventual `*.wasm` file. `first_party_artifact_provenance_is_recorded` keeps that contract from regressing. Clay runtime still performs no network fetch, package-manager install, shell build, or native-library load for grammar artifacts.

## Tree-sitter Parse/Highlight Handler

`TreeSitterSyntaxHandler` in `src/server/syntax.rs` is the server-side parse handler for validated grammar contributions. It is generic: callers provide the validated `SyntaxGrammarContribution`, a resolved `tree_sitter::Language`, and the package highlight query text. The handler:

1. compiles the highlight `Query` once at handler construction
2. creates and configures one `tree_sitter::Parser` per handler, reused across all parses for that grammar instead of recreating it per document open (avoids repeated wasm language instantiation in debug builds)
3. fails closed if a query capture is not present in the package `styleMap`
4. runs from `ParseCoordinator` as a `ParseHandler`
5. selects the viewport-intersecting `ParseWindowSnapshot`
6. enforces the contribution `maxWindowBytes` before parsing
7. sets parser/query timeouts from `timeoutMs`
8. reuses a cached prior `Tree` for later document versions on the same window, applying a whole-window `InputEdit` before incremental reparsing so stale recovery nodes cannot survive changed text
9. extracts query captures with `QueryCursor::set_byte_range`
10. maps captures to inert `DecorationSpan` values with package provenance
11. walks the same cached parse tree only when `root.has_error()` and emits generic Tree-sitter `ERROR`/`MISSING` recovery nodes through `diagnostic_update`
12. rejects per-viewport highlight capture output above the syntax span cap instead of silently truncating query output
13. validates the resulting `DecorationSet` with the existing decoration validator
14. enforces `DECORATION_PAYLOAD_BUDGET_BYTES` before cache insertion or publication
15. inserts the validated set into the existing `SyntaxChunkCache` for near-viewport/LRU budget enforcement
16. returns one `IncrementalParseUpdate` containing both current syntax decorations and the current `tree-sitter` diagnostic source set

Capture extraction is engine-neutral after parse: Tree-sitter and future web-tree-sitter adapters produce `SyntaxCapture { byte_start, byte_end, capture_name }` records. `map_capture_to_vocabulary` is the one shared capture-to-vocabulary mapper; it looks up the descriptor `style_map`, converts the style token to the Phase 18.15 closed `TokenType` + `Modifiers` axes, preserves the original token as `scope`, and fails closed for unmapped captures. Native and WASM tiers therefore feed the same `DecorationSpan` construction path.

### Generic Tree-sitter Recovery Diagnostics

Native extraction uses only grammar-neutral `Node::has_error`, `is_error`, `is_missing`, `byte_range`, and `walk` APIs. A valid root takes the constant-time empty path and still emits an empty current-source `DiagnosticSet`, allowing replacement semantics to clear old syntax errors. Error roots are traversed iteratively over the already bounded parse window; only error-bearing children are visited. Candidate ranges are clipped to the viewport, nested/equal ranges are reduced deterministically to innermost visible ranges, and output stops at `DIAGNOSTIC_MAX_SPANS_PER_SET`.

Tree-sitter `MISSING` nodes have zero-width ranges. `visible_scalar_range` anchors them to the next UTF-8 scalar, or the previous scalar at end-of-window; empty text emits no span. Published metadata is Clay-owned and fixed: source `tree-sitter`, severity `Error`, codes `syntax.error` / `syntax.missing`, and messages `syntax error` / `missing syntax`. Raw source snippets, parser node names, query text, and paths never enter diagnostic metadata. Provenance comes from the selected `SyntaxGrammarContribution`.

Tier 2 mirrors the local capture contract through `collectWebTreeSitterDiagnostics` in `runtime/js/web-tree-sitter-host.ts`; Tier 3 uses the already documented inert parse diagnostic records. Neither adapter adds callbacks or language-name branches.

The handler publishes only through the existing parse/decor path: `ParseCoordinator::schedule_parse_with_windows` executes the handler in background work, validates stale document versions and payload budgets, then emits an `IncrementalParseUpdate` containing a validated `DecorationSet`. The existing `SyntaxChunkCache` enforces `SYNTAX_CACHE_BUDGET_BYTES` retention policy for validated syntax chunks. Open-time parse scheduling is non-blocking: document follow-up messages return after enqueue, while parsed decorations arrive later through the coordinator update channel. Handler failures and invalid results publish sanitized `RuntimeDiagnostic` values via the coordinator diagnostic channel instead of blocking open or leaking parser details. The client still receives the normal decoration/diagnostic transport message path; no package code runs in the Rust client.

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

- Tier 1 native first-party registration/default selection for Rust, TypeScript, TSX, JavaScript/JSX/MJS/CJS, and Markdown
- native first-party descriptors shadow matching WASM package metadata without blocking package load
- explicit Tier 2 WASM override can replace matching native Tier 1 selection when authorized
- explicit user engine preference can force `wasm` provenance or `javascript` parser fallback
- package WASM metadata cannot silently override native tier without user preference
- web-tree-sitter artifact contract accepts package-confined `grammars/*.wasm` + `queries/*.scm` and rejects native/unconfined inputs
- internal web-tree-sitter host adapter is bundled/local-only and caches runtime/language/query initialization
- native parser instance identity is stable per grammar handler
- static no-branch guard for language-specific highlighter/control-flow shapes
- valid grammar contribution provenance, asset paths, style map, and budgets
- required `parse-document` + `render-decorations` permissions
- rejection of third-party syntax grammar packages, external/traversing paths
- rejection of executable/native/client/CSS authority fields
- rejection of unknown style tokens/raw CSS
- registry provenance and extension lookup
- active syntax grammar selection separate from active major mode and behavior version
- no-grammar fallback that preserves editability through the active major mode
- first-party package validation and fixture coverage for `@clay/rust`, `@clay/typescript`, `@clay/javascript`, and `@clay/markdown`
- real fixture highlight smoke for `tests/fixtures/syntax/rust.rs`, `typescript.ts`, `typescript.tsx`, `javascript.js`, and `markdown.md` producing bounded vocabulary `DecorationSet` values through the generic handler
- first-party grammar artifact provenance files that record upstream release, deterministic WASM build command, and SHA-256 recording requirement
- deterministic manual-smoke surrogate `manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow`: loads all three grammar packages, selects `.rs`/`.ts`/`.js` syntax grammars while preserving `core.code`, parses before/after a small edit, and verifies unloaded no-highlight fallback editability
- explicit `syntax-grammars-init.js` one-line load fixture and `tests/fixtures/configuration/syntax-grammars/init.js` smoke-gui fixture
- deterministic duplicate language and duplicate pattern conflicts
- engine-neutral `SyntaxCapture` to `TokenType`/`Modifiers` vocabulary mapping with fail-closed unmapped captures
- Tree-sitter highlight query capture extraction into bounded decoration spans
- valid capture mapping to `keyword.control`, `string.quoted`, `comment.line`, and `punctuation.definition`
- cached-tree reuse for later document versions with whole-window `InputEdit` correctness
- generic `ERROR` and `MISSING` extraction, valid-tree empty-set clearing, UTF-8-safe missing anchors, viewport/deduplication/count bounds, first-party invalid grammar coverage, and no language-specific branches
- Tier 2 host-side generic error/missing capture contract
- invalid query/unmapped capture fail-closed behavior with actionable diagnostics
- parse-window budget enforcement before parsing
- per-viewport capture overflow rejection before decoration publication
- parse coordinator publication through existing `IncrementalParseUpdate`/`DecorationSet` path

## Related

- [Phase 18.16 Tiered Tree-sitter Engine Primitive Review](phase18.16-tiered-tree-sitter-engine-primitive-review.md)
- [Phase 18.10 Tree-sitter Grammar Primitive Review](phase18.10-tree-sitter-grammar-primitive-review.md)
- [Package Loading](package-loading.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
