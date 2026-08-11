# Phase 18.16 Tiered Tree-sitter Syntax Engine Primitive Review

## Source

- `roadmap.md` Phase 18.16
- `plans/047-Phase18.16-Tiered-Tree-sitter-Syntax-Engine.md`
- `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/syntax-vocabulary.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/syntax-grammar-registry.md`
- `docs/wiki/modules/phase18.10-tree-sitter-grammar-primitive-review.md`
- `docs/wiki/modules/editor-theme-registry.md`
- `src/server/syntax.rs`
- `src/server/parse_coordinator.rs`
- `src/server/connection.rs`
- `src/protocol/decorations.rs`
- `src/editor/theme.rs`
- `src/editor/surface.rs`
- `src/server/ops/syntax.rs`
- `runtime/js/syntax.js`
- `runtime/js/parse.js`
- `packages/rust/package.json`
- `packages/typescript/package.json`
- `packages/javascript/package.json`
- `packages/markdown/package.json`
- `tests/syntax_grammar.rs`
- `tests/parse_coordinator.rs`
- `tests/decoration_transport.rs`
- `tests/editor_performance_invariants.rs`

## Overview

Phase 18.16 replaces the earlier web-tree-sitter-only implementation target with a tiered syntax engine behind one generic grammar-to-decoration pipeline:

```text
SyntaxEngineTier::Native -> SyntaxEngineTier::Wasm -> SyntaxEngineTier::JavaScriptFallback
SyntaxGrammarContribution -> SyntaxEngineSelection -> ParseCoordinator -> DecorationSet(TokenType, Modifiers)
```

Tier 1 is native compiled-in first-party Tree-sitter grammar data, Tier 2 is a host-side `web-tree-sitter` WASM adapter, and Tier 3 is the existing package JavaScript parser fallback. This review inventories reusable primitives before implementation and records only generic gaps. Language names may exist in package/registration data, but not as Rust server/client/editor control-flow branches.

## Existing Primitive Inventory

### Grammar registry and package grammar metadata

- `src/packages/record.rs::SyntaxGrammarContributionDescriptor` already parses package `clay.contributions.syntaxGrammars` metadata: `languageId`, `filePatterns`, grammar artifact path, query paths, `styleMap`, budgets, and package provenance.
- `src/server/syntax.rs::SyntaxGrammarRegistry` indexes validated grammar contributions by contribution ID, language ID, extension, and exact filename.
- `SyntaxGrammarRegistry::select_for_document` keeps active syntax grammar separate from active major mode, so `core.code` / `core.text` editability remains available when highlighting is absent or invalid.
- Existing first-party packages provide partial grammar metadata: `@clay/rust`, `@clay/typescript`, and `@clay/javascript` have `queries/highlights.scm` and placeholder `grammars/README.md`; `@clay/markdown` currently has a markdown-it JS parser package and no Tree-sitter grammar metadata/artifact yet.
- Current package metadata still names `tree-sitter-wasm` artifacts; Phase 18.16 must generalize this into a tiered descriptor without breaking the package-root confinement and provenance checks.

### Parse coordinator and open-time parse path

- `src/server/parse_coordinator.rs::ParseCoordinator` already owns background scheduling, handler registration, generation replacement, cancellation, stale-version rejection, bounded parse windows, payload validation, and `ParseCoordinatorStats`.
- `ParseCoordinator::schedule_parse_with_windows` delivers server-prepared `ParseWindowSnapshot` values to handlers and enforces `ParsePolicy`, `SYNTAX_CACHE_BUDGET_BYTES`, and window metadata checks before parser code sees document text.
- `ParseCoordinator::finish_task` validates successful updates and records handler failures, but Phase 18.16 still needs visible sanitized `RuntimeDiagnostic` publication for handler errors/timeouts rather than silent failed-task stats only.
- `src/server/connection.rs` already has selected-file/open follow-up paths and runtime diagnostic plumbing. Phase 18.16 must ensure open document text renders first and syntax work follows asynchronously.

### Decoration transport and vocabulary/theme registry

- `src/protocol/decorations.rs::DecorationSpan` already carries `token_type`, `modifiers`, optional `scope`, `kind`, priority, and provenance. Compatibility helpers still map legacy `style_token` values through `DecorationSpan::from_style_token`.
- `src/server/decorations.rs` validates document version, byte ranges, viewport/chunk bounds, payload size, permissions, and provenance before decoration publication.
- `src/editor/theme.rs::StyleRegistry` is the single source of color and maps `TokenType + Modifiers` to `StyleSpec`.
- `src/editor/surface.rs` stores validated decoration chunks outside paint and resolves style through `StyleRegistry` during native rendering.
- Phase 18.16 should keep one capture-to-vocabulary mapper. It may preserve `scope` for compatibility but must emit first-party syntax through `TokenType` and `Modifiers`, not legacy free-form-only `style_token` families.

### Package loading and JS parse bridge

- `loadPackage("@clay/*")` already provides explicit user opt-in loading from `~/.config/clay/init.js` for first-party packages.
- `runtime/js/syntax.js` and `src/server/ops/syntax.rs` expose `clay:syntax.serverRegisterSyntaxGrammar` for inert grammar metadata registration.
- `runtime/js/parse.js` and `src/server/ops/parse.rs` expose `parse.serverRegisterParseHandler`; Rust stores a handler token, not a JavaScript function value.
- Existing Tier 3 package-JS parser fallback is represented by package parse handlers and should remain the fallback for grammar-less packages.

## Existing Achievements Reused As-Is

- `SyntaxGrammarContribution` remains the package-visible primitive name.
- `ParseCoordinator` remains the scheduler; do not add a second syntax scheduler.
- `DecorationSet` / `DecorationSpan` remain the inert publication unit; do not add parser-token or AST transport.
- `StyleRegistry` remains the render-time style resolver; do not reintroduce hardcoded color tables in paint paths.
- `loadPackage` remains explicit setup; do not silently auto-load language packages.

## Generic Phase 18.16 Gaps

### `SyntaxEngineTier` and engine selection/provenance

Add a generic tier model such as `SyntaxEngineTier::Native`, `SyntaxEngineTier::Wasm`, and `SyntaxEngineTier::JavaScriptFallback`, plus an engine-selection record that explains selected tier, package provenance, grammar ID, matched extension/filename, active major mode, and user/package override rationale.

Selection must run only at package load, document open/reload, explicit reclassification, package reload, or user configuration time. Package priority alone must not silently override native first-party Tier 1; override native requires explicit user/package-initiated engine selection with provenance.

### Tier 1 native first-party descriptors

Add native descriptor data for first-party Rust, TypeScript, TSX, JavaScript/JSX/MJS/CJS, and Markdown grammars. The descriptor owns package identity, language ID, file patterns, query path, capture map, native `tree_sitter::Language`, budgets, and tier. Adding another first-party native grammar should be data registration plus Clay rebuild, not a new parser branch.

### Tier 2 web-tree-sitter host adapter

Add one host-side `web-tree-sitter` adapter that consumes resolver-validated package `grammars/*.wasm` and `queries/highlights.scm`, caches runtime/grammar/query initialization, and returns the same capture records as Tier 1. It must be host-owned shared code, not per-package wrapper logic.

### Tier 3 JS parser fallback

Keep existing package JS parse handlers for grammar-less languages. The fallback should feed the same decoration validation and vocabulary publication path when possible. It must not execute from paint/key/text/layout hot paths.

### One capture-to-vocabulary mapper

Replace Phase 18.10 free-form-only style-map output with one shared mapper that turns Tree-sitter captures or JS parser scopes into `TokenType`, `Modifiers`, and optional `scope`. Unmapped captures fail closed with diagnostics. First-party Rust/TypeScript/JavaScript/Markdown mappings must target the Phase 18.15 LSP + Clay prose vocabulary.

### Open-parse diagnostics

Extend the existing parse/open plumbing so handler errors, timeouts, invalid query/capture mapping, stale unsafe output, and payload-budget failures become sanitized `RuntimeDiagnostic` values. Diagnostics must not contain untrusted raw paths, source snippets, raw query text, native handles, or package-internal secrets.

## Hot-Path Classification

- Package load / grammar validation: validate metadata, tier descriptors, artifact/query paths, package prefix, permissions, style maps, budgets, and provenance.
- Document open / reload / explicit reclassification: compute `SyntaxEngineSelection`; schedule syntax work but render text immediately.
- Background parse/highlight work: execute native Tree-sitter, web-tree-sitter, or JS parser fallback through `ParseCoordinator` with bounded `ParseWindowSnapshot` inputs and viewport-prioritized output.
- Paint/text-event/key/layout/scroll/pointer hot path: read already validated decoration chunks and `StyleRegistry` only.

No parser/query compilation, `loadPackage`, package JavaScript, web-tree-sitter WASM initialization, native Tree-sitter parse, IPC round trip, filesystem scan, package-manager work, shell command, and no runtime configuration evaluation may run in keypress, paint, layout, scroll, pointer, or text-event handlers.

Budgets remain `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, `SYNTAX_CACHE_BUDGET_BYTES`, and `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`.

## Security and Authority Boundary

- First-party native grammars are compiled-in Clay-maintained grammar data. They do not grant third-party native library loading.
- Tier 2 WASM artifacts must be resolver-validated, package-root-confined `grammars/*.wasm` files for this phase. No runtime downloads, package-manager execution, shell execution, network fetch, external URL, absolute path, parent traversal, native library handle, or unapproved third-party grammar trust is added.
- Tier 3 JS parser fallback stays server-side through existing runtime handler tokens. Rust never accepts executable callback fields or raw JS functions from public APIs.
- The client receives only inert `DecorationSet`/`RuntimeDiagnostic` data; no parser objects, AST nodes, raw CSS/colors, draw callbacks, native widget handles, raw ops, client JavaScript, or package-controlled renderer callbacks cross the boundary.
- Permissions remain `parse-document` and `render-decorations` for syntax work. filesystem, network, shell, AI, workspace mutation, native-ui, package-control, package-manager, raw-ops, and client-runtime authority stay out of scope unless a later decision log approves them.

## Rejected Implementation Shapes

- Do not add `RustSyntaxHighlighter`, `TypeScriptSyntaxHighlighter`, `JavaScriptSyntaxHighlighter`, `MarkdownTreeSitterHighlighter`, or `if language == "rust"` / `if extension == "ts"` / `if package == "@clay/javascript"` Rust server/client/editor branches.
- Do not add a second parse scheduler for syntax highlighting.
- Do not run Tree-sitter, web-tree-sitter, package JavaScript, query compilation, or package loading in Masonry paint, layout, keypress, pointer, scroll, or text-event handlers.
- Do not publish raw parser tokens, AST nodes, raw CSS, raw colors, Vello/Parley callbacks, native handles, raw ops, or client-side JavaScript.
- Do not silently let packages override Tier 1 native highlighting by load order or priority alone.
- Do not add hidden JSON/TOML syntax-engine keys; user-visible engine preference must be a documented Clay JS API if implemented.

## Implementation Plan Mapping

- Engine-neutral grammar-to-vocabulary pipeline: reuse `TreeSitterSyntaxHandler`, `ParseCoordinator`, `DecorationSet`, `DecorationSpan`, and `StyleRegistry`; add only generic capture/vocabulary mapping and diagnostic publication.
- Tier 1 native: add descriptor registration data and parser/query cache reuse for first-party grammars.
- Tier 2 web-tree-sitter: add one host adapter and artifact contract.
- Tier 3 JS fallback: retain existing `serverRegisterParseHandler` bridge and feed validated decorations through the same transport.
- Configuration/API tasks: expose only actual user-facing engine preference/configuration; keep internal helpers private or `pub(crate)`.
- Docs/tests: update public primitive/package docs after implementation and keep this primitive review linked from the wiki index, primitive architecture page, and `tests/primitives_docs.rs`.

## Final Implementation Status

Plan 047 completed the generic gaps recorded here without adding language-specific server/client branches:

- `SyntaxGrammarRegistry::with_first_party_native()` seeds five data-only Tier 1 descriptors for Rust, TypeScript, TSX, JavaScript, and Markdown.
- `SyntaxGrammarContribution::web_tree_sitter_artifact_contract()` and `runtime/js/web-tree-sitter-host.ts` define the package-confined, local-only Tier 2 boundary with cached runtime/language/query initialization.
- `setSyntaxEnginePreference` provides explicit `native`, `wasm`, and `javascript` selection; ordinary package registration cannot silently replace native descriptors, and Tier 3 remains the existing package parse-handler route.
- `SyntaxCapture` and `map_capture_to_vocabulary` feed all Tree-sitter captures into `TokenType` + `Modifiers` `DecorationSpan` output with fail-closed unmapped captures.
- `ParseCoordinator` keeps parse work background and enqueue-only for open, publishes bounded updates through `next_update()`, and publishes sanitized `parse.open_failed` diagnostics through `next_diagnostic()`.

The implementation uses the existing `DecorationSet`, `StyleRegistry`, package permissions, provenance validation, parse budgets, and docs/API registry boundaries. Public usage remains documented in the package authoring guide and [`setSyntaxEnginePreference`](../../reference/clay-js-api/syntax/set-syntax-engine-preference.md); this page remains the primitive rationale and reuse guide.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Future implementation tests named in Plan 047: `syntax_pipeline_maps_captures_to_vocabulary_tokens`, `finish_task_publishes_runtime_diagnostic_for_handler_error`, `tier1_native_first_party_is_default_for_known_extensions`, `web_tree_sitter_runtime_is_bundled_and_loadable_without_network`, and `js_parser_fallback_still_runs_without_tree_sitter_grammar`.
- Existing focused suites to preserve: `cargo test --test runtime syntax_grammar::`, `cargo test --test runtime parse_coordinator::`, `cargo test --test editor decoration_transport::`, and `cargo test --test editor editor_performance_invariants::`.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.10 Tree-sitter Grammar Primitive Review](phase18.10-tree-sitter-grammar-primitive-review.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Primitive Registry](../../reference/primitives/registry.md)
- [Parse Update Strategy](../../reference/primitives/parse-update-strategy.md)
- [Rendering Strategy](../../reference/primitives/rendering-strategy.md)
- [Package Security](../../reference/primitives/package-security.md)
- [Syntax Vocabulary](../../reference/primitives/syntax-vocabulary.md)
