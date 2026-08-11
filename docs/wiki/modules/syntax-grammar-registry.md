# Syntax Grammar Registry

## Source

- `src/packages/record.rs`
- `src/server/syntax.rs`
- `runtime/js/syntax.js`
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

Phase 18.16 turns `SyntaxGrammarContribution` metadata into a tiered syntax engine; Phase 18.18 promotes first-party package/native descriptors to direct `TokenType` + `Modifiers` styleMaps and honest `grammar.kind = "native"` metadata. The server stores validated grammar descriptors separately from active major modes, selects native first-party Tree-sitter by default, supports explicit web-tree-sitter WASM selection, and retains package-JavaScript parsing as Tier 3 fallback. All engines feed one bounded capture-to-`TokenType`/`Modifiers` decoration path, so a document remains editable through `core.code`, `core.text`, or its package mode when highlighting is absent or deferred.

## Primitive Coverage

- **`SyntaxGrammarContribution` / `SyntaxGrammarRegistry`** — owned by `src/server/syntax.rs`; package metadata and registry state carry provenance, `SyntaxEngineTier`, native source or WASM/query paths, vocabulary style maps, and bounded parse budgets.
- **Clay JS boundary** — `clay:syntax.serverRegisterSyntaxGrammar` registers inert grammar metadata; `clay:syntax.setSyntaxEnginePreference` records an explicit user tier preference. Public usage is documented in [`server-register-syntax-grammar`](../../reference/clay-js-api/syntax/server-register-syntax-grammar.md) and [`set-syntax-engine-preference`](../../reference/clay-js-api/syntax/set-syntax-engine-preference.md).
- **Parse/decorations primitives** — `ParseCoordinator` owns background scheduling, cancellation, stale-result rejection, and sanitized diagnostics; `DecorationSet`/`DecorationSpan` owns bounded inert output and `StyleRegistry` resolves colors during native paint. Syntax packages require `parse-document` and `render-decorations`.
- **Reuse rule** — future language packages add descriptor/package data and queries; they reuse the registry, host adapter, mapper, coordinator, and decoration transport rather than adding language-specific Rust/client branches.

## Package Record Validation

`assemble_package_record` now parses `clay.contributions.syntaxGrammars` into `SyntaxGrammarContributionDescriptor` values. Each descriptor retains package provenance through the enclosing `PackageRecord` and validates:

- package-owned optional `id` (default: `<apiPrefix>.<languageId>`)
- lowercase generic `languageId`
- `filePatterns.extensions` and/or `filePatterns.fileNames`
- `grammar.kind = "native"` with a required compiled source ID and no artifact path, or `grammar.kind = "tree-sitter-wasm"` with a confined `.wasm` path
- package-root-confined `.scm` query paths for `highlights`, optional `locals`, and optional `injections`
- capture-to-vocabulary `styleMap` objects using closed `TokenType` + `Modifiers`; known legacy style-token strings remain accepted and normalized for compatibility
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
- `packages/typescript/` -> `@clay/typescript`, `apiPrefix = typescript`, native TypeScript grammar for `.ts`, `.mts`, `.cts`, and native TSX grammar for `.tsx`
- `packages/javascript/` -> `@clay/javascript`, `apiPrefix = javascript`, `languageId = javascript`, extensions `.js`, `.jsx`, `.mjs`, `.cjs`
- `packages/markdown/` -> `@clay/markdown`, full Markdown mode package with a native `.md`/`.markdown`/`.mdown` descriptor and existing markdown-it Tier 3 parser fallback

Rust, TypeScript, JavaScript, and Markdown packages declare `native` source metadata, `./queries/highlights.scm`, direct vocabulary styleMaps, permissions, and their mode contributions. Markdown keeps its package-JavaScript preview/Tier 3 fallback contract while Tier 1 decorations use the native descriptor/query path. All syntax output still requires `parse-document` and `render-decorations`; package loading does not grant filesystem, network, shell, AI, native-library, raw-op, client-JavaScript, package-manager, or package-control authority.

The `dist/load.js` entries import `clay:syntax` and register only inert grammar metadata. `loadPackage("@clay/<language>")` validates/enables package data through the existing resolver, executes the load entry on the controlled runtime, and registers package contributions without exposing raw ops or executable callbacks to user config. Matching first-party native package metadata is shadowed by the already-seeded compiled descriptor. Explicit `setSyntaxEnginePreference("<language>", "wasm")` selects Tier 2 only when a package actually supplies valid WASM metadata; `javascript` preference retains package parser fallback. User config remains one-line `loadPackage` setup unless it explicitly requests a tier; hidden JSON/TOML syntax keys are not supported.

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

`ClayOpState::new_for_document` uses `SyntaxGrammarRegistry::with_first_party_native()` so runtime op state starts with Tier 1 native entries. Each static descriptor also embeds its package highlight query with `include_str!`, avoiding runtime filesystem authority. `ClayRuntimeEvaluation` carries the grammar list and engine-preference snapshot across the worker boundary. On document open, `select_grammar_for_path` applies exact-file/extension matching plus user tier preference; `ClayJsRuntimeService::register_native_syntax_handler` builds the selected `TreeSitterSyntaxHandler` once per generation/package/grammar ID before `schedule_open_parse` enqueues work. The package/mode JS fallback remains separately keyed and is selected only when no native handler is chosen. Later opens reuse the selected native handler and its parser/tree caches.

Runtime native installation uses the document-selected grammar contribution ID as the coordinator handler mode key while preserving package provenance. This lets `typescript.typescript` and `typescript.tsx` coexist in one runtime generation without last-opened-grammar replacement; open scheduling records the selected handler ID separately from the active package mode (`typescript`). Rust, JavaScript, and Markdown use the same generic path with their single selected grammar IDs.

## Tier 2 Web-tree-sitter Artifact Contract

`SyntaxGrammarContribution::web_tree_sitter_artifact_contract()` turns a validated `tree-sitter-wasm` contribution into a host adapter contract containing only inert package provenance plus package-root-confined `./grammars/*.wasm` and `./queries/*.scm` paths. It rejects native contributions, non-WASM grammar kinds, parent traversal, external URLs, absolute paths, and non-query/non-WASM suffixes.

`runtime/js/web-tree-sitter-host.ts` is the internal host adapter scaffold. It follows the upstream `web-tree-sitter` shape (`Parser.init`, `Language.load`), pins the runtime wasm lookup to `clay://runtime/tree-sitter.wasm`, caches language/query initialization, validates the same package-local path contract, and intentionally contains no `fetch`, HTTP URL, npm/package-manager, shell, raw-op, native-handle, or client-JavaScript authority. It returns capture records for the shared Rust-side syntax decoration pipeline; package load entries do not get per-language wrappers.

## Tier 3 JavaScript Fallback and Engine Preference

Tier 3 is intentionally not another grammar registry path. Existing package JavaScript parse handlers registered through `clay:parse.serverRegisterParseHandler` remain the fallback for grammar-less languages or packages that force `javascript` engine preference. When no syntax grammar is selected, the active major mode and behavior version stay untouched, so the package parse handler can still publish decorations/status through the existing parse/decor transport.

`clay:syntax.setSyntaxEnginePreference(target, tier)` records user-initiated engine choice in `SyntaxGrammarRegistry` at init/package-load time only. Targets match a language id, package `apiPrefix`, or package name. Valid tiers are `native`, `wasm`, and `javascript` (`js` alias). Default is no preference: seeded Tier 1 native wins for first-party languages; first-party WASM metadata is shadowed. A forced `wasm` preference allows a matching package grammar to replace the native descriptor through the existing explicit Tier 2 override path. A forced `javascript` preference suppresses syntax-grammar selection and leaves highlighting to the package JS parse handler. Packages cannot silently self-promote over native tier because ordinary `serverRegisterSyntaxGrammar` calls still register through `register_package` and only see override behavior when the user preference already exists.

Selection diagnostics include the chosen tier in `SyntaxGrammarSelection::why` and `ActiveSyntaxGrammar::engine_tier`. Preference lookup and package registration are load/open/reclassification work; no preference or engine selection code runs from paint, layout, scroll, pointer, or key/text-event handlers.

## First-Party Query and Artifact Provenance

Phase 18.18 expands first-party highlight queries under `packages/*/queries/highlights.scm` for keywords, strings, comments, functions/declarations, types, numbers, punctuation, and Markdown prose. Direct package/native styleMap entries produce Phase 18.15 `TokenType` + `Modifiers` with no scope/color data. Query captures absent from the styleMap are skipped as unstyled; they never receive a default color. Plan 059 task 5 adds an optional validated `priority` (0-100, default 70) per styleMap entry: higher priority wins overlapping syntax-layer spans, so Markdown narrow inline captures (link/code-span/strong/emphasis at 80) outrank broad prose captures regardless of emission order; equal priorities keep the existing `font_role_precedes` tie-break chain.

`tree-sitter-md-025::LANGUAGE` is the block grammar; inline syntax has a separate `INLINE_LANGUAGE`. Plan 059 task 4 replaced the old block-query regex predicates (which could only classify standalone inline forms) with a generic injection executor: `packages/markdown/queries/injections.scm` declares `(inline)` and `(pipe_table_cell)` ranges as `markdown_inline` injection content (plus fenced-code info-string languages), the handler re-parses each range set with `tree_sitter_md_025::INLINE_LANGUAGE` via `Parser::set_included_ranges`, and `packages/markdown/queries/inline-highlights.scm` captures `strong_emphasis`/`emphasis`/`code_span`/link nodes under the same styleMap keys (`strong`, `emphasis`, `code-span`, `link`), so mixed inline runs now style through the identical vocabulary pipeline. The executor is grammar-agnostic: `NativeGrammarDescriptor.injections_query` is opt-in per grammar, injection language names resolve only against `FIRST_PARTY_EMBEDDED_GRAMMARS` (unregistered names such as fenced-code info strings are skipped), and embedded layer parsers are cached per name. Markdown block meaning can begin before a viewport (especially fenced code), so its native descriptor's data-only `max_window_bytes` is `MAX_OPENABLE_FILE_BYTES` (768 KiB): the scheduler supplies full-document parse context for every openable Markdown file but caps query/decor output to `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` (4 KiB). A second same-version viewport request reuses the cached full `Tree` (`tree-sitter:markdown:cached`) and queries only the new viewport; it does not reparse the file. This also anchors 128-byte authority chunks to document offset zero instead of shifting chunk grids with each scroll window. Code grammars retain 4 KiB parse windows, and over-limit scratch documents fall back to the existing guarded bounded window. Smoke fixtures live in `tests/fixtures/syntax/{rust.rs,typescript.ts,typescript.tsx,javascript.js,markdown.md}` and are parsed through `TreeSitterSyntaxHandler` in `tests/syntax_grammar.rs::first_party_language_fixtures_produce_themed_vocabulary_decorations`; mixed-run behavior is locked by `markdown_inline_injection_styles_mixed_runs`, while `same_version_markdown_scroll_reuses_full_document_tree_context` locks fence/prose correctness and one parse invocation across scroll.

Tier 2 WASM binaries are not committed yet; each `packages/*/grammars/PROVENANCE.md` records the exact upstream crate/release used by Tier 1, a reproducible `tree-sitter build --wasm` command, and the required SHA-256 recording step for the eventual `*.wasm` file. `first_party_artifact_provenance_is_recorded` keeps that contract from regressing. Clay runtime still performs no network fetch, package-manager install, shell build, or native-library load for grammar artifacts.

Plan 071 (task 10) adds first-party **text-object queries** alongside highlights: `packages/{rust,typescript,javascript}/queries/textobjects.scm` (TypeScript/TSX share one file) ship capture schemas `@textobject.<kind>.<scope>` for eight kinds (function/class/argument/comment/loop/conditional/call/statement, `inner` falling back to `around`). Like highlights, they are compiled-in `NativeGrammarDescriptor` fields (`textobjects_query_path`/`textobjects_query`) — package grammar metadata cannot declare them: `src/packages/record.rs` rejects any `queries` key outside {highlights, locals, injections} deny-by-default, and grammar contributions remain first-party-only. The queries feed the advisory `clientSelectTextobject`/`clientSmartSelect` wire path; smart select itself needs no query file. See [Editor Movement, Selection, Caret, Ligatures, and Text Objects](editor-movement-selection-caret.md).

## Tree-sitter Parse/Highlight Handler

`TreeSitterSyntaxHandler` in `src/server/syntax.rs` is the server-side parse handler for validated grammar contributions. It is generic: callers provide the validated `SyntaxGrammarContribution`, a resolved `tree_sitter::Language`, and the package highlight query text. The handler:

1. compiles the highlight `Query` once at handler construction
2. creates and configures one `tree_sitter::Parser` per handler, reused across all parses for that grammar instead of recreating it per document open (avoids repeated wasm language instantiation in debug builds)
3. leaves query captures absent from the package `styleMap` unstyled while still rejecting invalid query syntax
4. runs from `ParseCoordinator` as a `ParseHandler`
5. selects the viewport-intersecting `ParseWindowSnapshot`
6. enforces the contribution `maxWindowBytes` before parsing
7. sets parser/query timeouts from `timeoutMs`
8. reuses a consecutive cached `Tree` for exact edits when document, stable window identity, and implied old-window length match; same-version viewport-only requests reuse that tree directly without invoking `Parser::parse`; exact edits convert the server-relative change to Tree-sitter `InputEdit`, apply `Tree::edit`, and pass the edited tree to `Parser::parse`
9. computes `old_tree.changed_ranges(&new_tree)`, unions those ranges with explicit accepted-edit invalidations, clamps to the visible bounded window, expands by at most one UTF-8 scalar at each edge, and deterministically sorts/merges overlaps
10. converts affected (changed+invalidated) ranges into a shared 128-byte UTF-8-safe replacement-chunk grid via `replacement_ranges`; queries the full envelope covering every touched replacement chunk once with `QueryCursor::set_byte_range` — so query coverage and replacement coverage are identical; intersecting captures retain their whole token/comment/string range and are clipped at exact chunk boundaries; full/open/viewport or incompatible-cache fallback explicitly queries the bounded visible window
11. maps the complete capture result to inert `DecorationSpan` values with package provenance
12. constructs complete `DecorationSet` members from the same replacement-chunk grid (not the original affected ranges), so every published chunk carries complete authoritative capture state for exactly the UTF-8-safe range it replaces; orders chunks intersecting explicit invalidations before adjacent chunks
13. validates every `DecorationSet`, enforces `DECORATION_PAYLOAD_BUDGET_BYTES`, and inserts each set into `SyntaxChunkCache` for near-viewport/LRU budget enforcement
14. returns one decoration-only `IncrementalParseUpdate::decoration_updates` batch; the coordinator validates all members atomically and the connection sends one `DecorationBatch` frame for multiple chunks (or a plain `DecorationSet` for one chunk)
15. leaves `diagnostic_update` empty because parser recovery nodes from bounded fragments are not analyzer authority; explicit analyzers own diagnostic publication

Capture extraction is engine-neutral after parse: Tree-sitter and future web-tree-sitter adapters produce `SyntaxCapture { byte_start, byte_end, capture_name }` records. `map_capture_to_vocabulary` is the one shared capture-to-vocabulary mapper. Direct entries copy closed `token_type` + `modifiers` and emit scope-less spans; legacy entries are normalized through `TokenType::classify_style_token` and preserve the validated original token as `scope`. Extraction skips absent mappings, while direct calls remain fallible. Native and WASM tiers feed the same `DecorationSpan` construction path.

### Diagnostic Authority

Tree-sitter `ERROR` and `MISSING` nodes are recovery artifacts, not correctness judgments. Bounded viewport parsing can create such nodes at otherwise valid fragment boundaries, so native syntax handlers never convert them into `DiagnosticSet` squiggles. First-party language packages remain decoration-only until an explicit analyzer, including future LSP packages, publishes diagnostics through the generic validated diagnostic facade. This keeps highlighting, diagnostic authority, and language-specific analysis separate without callbacks or language-name branches.

The handler publishes only through the existing parse/decor path: `ParseCoordinator::schedule_parse_with_windows` executes the handler in background work, validates stale document versions and payload budgets, then emits an `IncrementalParseUpdate` containing a fully validated `decoration_updates` batch. The existing `SyntaxChunkCache` enforces `SYNTAX_CACHE_BUDGET_BYTES` retention policy for validated syntax chunks. Open-time parse scheduling is non-blocking: document follow-up messages return after enqueue, while parsed decorations arrive later through the coordinator update channel. A classified grammar-backed mode with no Tier 3 JS handler treats `ParseCoordinatorError::HandlerNotRegistered` as the normal native/WASM path rather than publishing `parse.open_activation_failed`; registered-handler failures and invalid results still publish sanitized `RuntimeDiagnostic` values via the coordinator diagnostic channel instead of blocking open or leaking parser details. The client still receives decoration updates and any separately authorized analyzer diagnostics through their normal transport paths; no package code runs in the Rust client.

## Hot Path and Security Boundary

Package metadata validation and registry insertion happen at package load/reload time. Tree-sitter parser/query work runs only inside parse coordinator `Background` no-hot-path tasks over server-prepared bounded parse windows. It is not called from keypress, paint, layout, scroll, pointer, or text-event handlers, and `tests/editor_performance_invariants.rs` statically guards paint/layout sources against Tree-sitter/package parser calls.

The handler does not load arbitrary paths, fetch network resources, run package managers/shells, call raw ops, or execute client-side JavaScript. Grammar artifacts must already be resolver-validated first-party package assets before a `tree_sitter::Language` reaches the handler.

The runtime [`clay:syntax.serverRegisterSyntaxGrammar`](../../reference/clay-js-api/syntax/server-register-syntax-grammar.md) facade/op registers the same inert grammar contract declared in `package.json` during the package load entry. User config still uses only one-line `loadPackage` calls; it does not perform manual primitive registration or raw op calls.

Native scheduling submits one bounded parse window per document/version/window rather than decoration-chunk parser jobs. One query/capture pass queries the full envelope covering a shared 128-byte replacement-chunk grid (`replacement_ranges`) and fans complete mapped output into validated 128-byte sets built from the same grid — so query coverage and replacement coverage are identical, and dense capture overflow no longer truncates after 32 spans. Viewport-change requests schedule one missing stable window as the user scrolls; output chunk count changes only transport/cache work, never parser/query invocation count.

## Tests

Run:

```bash
cargo test --test runtime syntax_grammar:: --quiet
cargo test --test runtime parse_coordinator:: --quiet
cargo test --test editor decoration_transport:: --quiet
cargo test --test editor editor_performance_invariants:: --quiet
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
- Markdown open replaces its registered Tier 3 fallback with the path-selected native handler and publishes builtin-provenance vocabulary spans
- static no-branch guard for language-specific highlighter/control-flow shapes
- valid grammar contribution provenance, asset paths, style map, and budgets
- required `parse-document` + `render-decorations` permissions
- rejection of third-party syntax grammar packages, external/traversing paths
- rejection of executable/native/client/CSS authority fields
- rejection of unknown token types/modifiers, unknown legacy style tokens, and raw CSS/colors
- registry provenance and extension lookup
- active syntax grammar selection separate from active major mode and behavior version
- no-grammar fallback that preserves editability through the active major mode
- first-party package validation and fixture coverage for `@clay/rust`, `@clay/typescript`, `@clay/javascript`, and `@clay/markdown`
- real fixture highlight smoke for `tests/fixtures/syntax/rust.rs`, `typescript.ts`, `typescript.tsx`, `javascript.js`, and `markdown.md` producing bounded vocabulary `DecorationSet` values through the generic handler
- first-party grammar artifact provenance files that record upstream release, deterministic WASM build command, and SHA-256 recording requirement
- deterministic manual-smoke surrogate `manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow`: loads all three grammar packages, selects `.rs`/`.ts`/`.js` syntax grammars while preserving `core.code`, parses before/after a small edit, and verifies unloaded no-highlight fallback editability
- explicit `syntax-grammars-init.js` one-line load fixture and `tests/fixtures/configuration/syntax-grammars/init.js` smoke-gui fixture
- deterministic duplicate language and duplicate pattern conflicts
- engine-neutral `SyntaxCapture` to `TokenType`/`Modifiers` vocabulary mapping with unmapped captures left unstyled
- Tree-sitter highlight query capture extraction into bounded decoration spans
- direct first-party capture mapping for code/prose `TokenType` families and declaration/bold/italic modifiers, plus legacy style-token compatibility
- exact cached-tree edits for consecutive stable-window versions, complete-replacement-chunk-grid querying (query coverage == replacement coverage via `replacement_ranges`), whole keyword/comment/string recovery, newline-sensitive line comments, UTF-8-safe range merging, same-word narrow-syntax provisional inheritance (Unicode alphanumeric/underscore extends, whitespace/newline/punctuation stops), and bounded full fallback
- `first_party_package_queries_keep_authoritative_token_boundaries`: real Rust, TypeScript, TSX, JavaScript, and Markdown package queries preserve complete authoritative keyword/prose-heading, declaration/identifier, and punctuation captures after exact incremental edits; keyword removal drops only the affected capture
- `first_party_package_queries_keep_broad_captures_continuous`: real line/block comments, raw/template multiline strings, Markdown prose, code spans, and fenced code blocks retain their complete capture range after incremental edits; package grammar boundaries, not whitespace/idle heuristics, define correction
- first-party valid and invalid grammar fixtures remain decoration-only and never masquerade as analyzer diagnostics
- Tier 2 host-side highlighting remains separate from explicit analyzer diagnostic publication
- invalid query fail-closed behavior and unmapped-capture no-color-leak behavior
- parse-window budget enforcement before parsing
- one native parse task per stable window/version across first-party languages
- dense 4 KiB capture output fans out completely into independently payload-bounded sets without parser amplification
- visible/changed output ordering, atomic invalid-member rejection, and syntax/semantic chunk-key separation
- parse coordinator publication through existing `IncrementalParseUpdate`/`DecorationSet` path

## Related

- [Phase 18.16 Tiered Tree-sitter Engine Primitive Review](phase18.16-tiered-tree-sitter-engine-primitive-review.md)
- [Phase 18.10 Tree-sitter Grammar Primitive Review](phase18.10-tree-sitter-grammar-primitive-review.md)
- [Package Loading](package-loading.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
