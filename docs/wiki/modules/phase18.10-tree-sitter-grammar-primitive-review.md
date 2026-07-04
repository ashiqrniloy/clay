# Phase 18.10 Tree-sitter Grammar Primitive Review

## Source

- `roadmap.md`
- `plans/038-Phase18.10-Package-Provided-Tree-sitter-Grammar-and-Syntax-Highlighting-Primitive.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/mode-registry.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/phase18.9-generic-text-code-modes-primitive-review.md`
- `src/packages/modes.rs`
- `src/packages/manifest.rs`
- `src/packages/permissions.rs`
- `src/server/parse_coordinator.rs`
- `src/server/decorations.rs`
- `src/protocol/parse.rs`
- `src/protocol/decorations.rs`
- `runtime/js/parse.ts`
- `runtime/js/decorations.ts`
- `tests/primitives_docs.rs`

## Overview

Phase 18.10 adds package-provided syntax grammars without adding language-specific editor modes. The primitive review records existing editor/package primitives and the generic gaps that must exist before `@clay/rust`, `@clay/typescript`, and `@clay/javascript` grammar-only packages are implemented.

The target primitive is `SyntaxGrammarContribution`: package metadata that names a language, a Tree-sitter grammar artifact, highlight/query files, capture-to-style-token mapping, provenance, and performance budgets. It composes with existing `DocumentClassification`, `MajorModeActivation`, `IncrementalParseUpdate`, and `DecorationRange` primitives instead of replacing them.

## Existing Primitive Inventory

### Document classification and major-mode activation

- `src/packages/modes.rs::ModeRegistry` owns document classification, active major-mode state, fallback registration, and behavior manifest selection.
- Phase 18.9 already supplies always-on `core.text` and `core.code` fallback modes through `DocumentClassification` and `MajorModeActivation`.
- Existing classification can decide that a `.rs`, `.ts`, or `.js` file should be editable as `core.code` without a language package.
- Phase 18.10 must keep active syntax grammar separate from active major mode: a document may have `active_major_mode = core.code` and `active_syntax_grammar = rust`.

### Package loading and manifest validation

- `src/packages/manifest.rs` validates package name/version, `clay.apiPrefix`, entry/load entry confinement, capabilities, package graph metadata, and bounded manifest payloads.
- `src/packages/permissions.rs` already defines `parse-document` and `render-decorations`; prohibited authorities include filesystem, network, shell, WASM, AI, workspace mutation, native UI, client runtime, raw ops, package control, and package import.
- `docs/wiki/modules/package-loading.md` documents first-party `loadPackage("@clay/*")` loading, package record assembly, provenance, rollback, and tests.
- Grammar packages should reuse package loading and manifest validation. They should not introduce a package manager, auto-install path, or hidden default load.

### Parse coordinator and background work

- `src/server/parse_coordinator.rs` owns cancellable background parse scheduling, handler registration, generation replacement, stale-version rejection, parse-window validation, and syntax memory budgets.
- `src/protocol/parse.rs` defines `ParseWindowSnapshot`, `ParsePolicy`, `ParseWindowRequest`, `ParseEditNotification`, and `IncrementalParseUpdate`.
- `runtime/js/parse.ts` exposes `clay.parse.serverRegisterParseHandler` for server-side package parse handlers and rejects callback-shaped executable options.
- Phase 18.10 should reuse this lifecycle for Tree-sitter parse/highlight work as `Background` work. It should not add a second scheduler.

### Decoration transport and style-token validation

- `src/protocol/decorations.rs` defines `DecorationSpan`, `DecorationKind::Syntax`, `DecorationSet`, and package provenance.
- `src/server/decorations.rs` validates document versions, byte ranges, style tokens, permissions, provenance, and `DECORATION_PAYLOAD_BUDGET_BYTES`, then stores chunks under `SYNTAX_CACHE_BUDGET_BYTES`.
- `runtime/js/decorations.ts` exposes `clay.decorations.serverPublishDecorations` for bounded server-side publication.
- Known generic code tokens already include `keyword.control`, `string.quoted`, `comment.line`, `punctuation.definition`, and `text`.
- Tree-sitter captures must be mapped to Clay-known style tokens before publication; capture names and query files must not become raw CSS or renderer callbacks.

### Docs registry and wiki coverage

- `docs/reference/primitives/registry.md` is the canonical primitive taxonomy and must record `SyntaxGrammarContribution`.
- `docs/reference/primitives/backlog.md` is the phase queue and must record Phase 18.10 syntax grammar work before first language packages.
- `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md` must link this review so future plans find the primitive inventory.
- `tests/primitives_docs.rs` should fail if the review, registry, backlog, or wiki links stop mentioning the new primitive and its authority boundaries.

## Generic Phase 18.10 Primitive Gaps

### `SyntaxGrammarContribution`

`SyntaxGrammarContribution` is a package contribution primitive for Tree-sitter grammar metadata. It should contain package provenance, `languageId`, file pattern metadata, a resolver-validated grammar artifact path, highlight query path, optional locals/injections query paths, capture-to-style-token mapping, contribution version, and parse/highlight budgets.

This primitive is generic. Acceptable implementation names include `SyntaxGrammarContribution`, `SyntaxGrammarRegistry`, query/capture validation, capture style map, grammar artifact descriptor, and syntax provider selection. Rejected names include `RustSyntaxHighlighter`, `TypeScriptSyntaxMode`, `JavaScriptParserBranch`, or any `if language == "rust"` / `if extension == "ts"` / `if package == "@clay/javascript"` Rust server/client branch.

### Grammar registry

A grammar registry should validate and retain loaded syntax grammar contributions by package prefix and language ID. It should select at most one active syntax grammar for a document/version, record why a grammar was selected or skipped, and stay independent from active major-mode activation.

The registry should run at package load, package reload, document open/reload, or explicit reclassification time. It must not run Tree-sitter parsing or package JavaScript in keypress, paint, layout, scroll, pointer, or text-event handlers.

### Query/capture validation

Tree-sitter highlight queries should compile at package load or grammar registration time where possible. Captures must map through a package-declared style map to known Clay style tokens before they become `DecorationSpan` data.

Unknown captures, invalid queries, raw CSS strings, raw colors, renderer callbacks, client-side JavaScript, raw op names, native handles, and executable query-side payloads fail closed with actionable diagnostics. Editing must continue under `core.code`/`core.text` even if highlighting is disabled.

### Syntax provider selection

Syntax provider selection is separate from major-mode selection. Phase 18.10 should support this shape:

```text
active_major_mode: core.code
active_syntax_grammar: rust from @clay/rust
edit behavior: core.code behavior manifest
highlight provider: SyntaxGrammarContribution -> Tree-sitter -> DecorationSet
```

A grammar package cannot change active major mode, behavior manifests, command routing, package UI, file authority, or editor text transforms. Later full language modes may build on the same grammar primitive, but grammar-only packages in this phase provide highlighting assets only.

## Hot-Path Classification

- Package load / grammar validation: validate manifest metadata, package-root-confined artifact/query paths, capability requirements, query/capture shape, and style-map tokens.
- Document open / reload / explicit reclassification: choose active syntax grammar from validated registry metadata.
- Background parse/highlight work: run Tree-sitter parse/query work through server-owned background scheduling and publish bounded `DecorationSet` values.
- Paint/text-event/key hot path: read already validated/cached decoration spans only. No Tree-sitter parsing, query compilation, package JavaScript, raw ops, package loading, filesystem scans, and no synchronous IPC before local paint.

Phase 18.10 must preserve `ClientFirstPredictable` local editing from Phase 18.9. Syntax work is `Background`, cancellable, stale-version-rejecting, viewport-prioritized, and bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`.

## Security and Authority Boundary

- First-party-only grammar artifact scope for this phase: `@clay/rust`, `@clay/typescript`, and `@clay/javascript` may provide resolver-validated package assets; arbitrary third-party grammar/native artifact loading is out of scope.
- Package-root path confinement: grammar artifacts and query files must be relative to the validated package root; absolute paths, parent traversal, network URLs, package-manager execution, shell execution, and runtime downloads are rejected.
- No arbitrary native/third-party artifact loading: adding dynamic native library loading, broad WASM execution, third-party parser packages, or package-manager installation requires a future approved decision log and explicit authority model.
- No new filesystem, network, shell, AI, WASM, native-widget, raw-op, client-side JavaScript, package-manager, package-install, package-enable/disable, or package-control authority is introduced by `SyntaxGrammarContribution`.
- Grammar packages should request only the capabilities needed for server-side parse and decoration publication, such as `parse-document` and `render-decorations`, plus any documented registration capability if added.

## Rejected Implementation Shapes

- Do not add language-specific Rust parser/highlighter branches such as `RustSyntaxHighlighter`, `TypescriptHighlighter`, `JavascriptHighlighter`, or `if language == "rust"` server/client code.
- Do not implement `@clay/rust`, `@clay/typescript`, or `@clay/javascript` as full major modes in this phase.
- Do not let grammar packages register commands, completions, SDUI panels, behavior manifests, or language-specific text transforms as part of the grammar-only package contract.
- Do not run Tree-sitter or package JavaScript in Masonry paint, layout, keypress, pointer, scroll, or text-event handlers.
- Do not publish raw parser tokens, AST nodes, raw CSS, raw colors, renderer callbacks, native handles, or client-side JavaScript to the Rust client.
- Do not silently auto-load grammar packages; end users should use explicit one-line `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, or `loadPackage("@clay/javascript")` setup when package loading is implemented for the language packages.

## Final Implementation Status

Phase 18.10 implemented the primitive as generic infrastructure rather than language-specific Rust branches:

- `src/packages/record.rs` parses and validates `SyntaxGrammarContributionDescriptor` metadata under `clay.contributions.syntaxGrammars`, including first-party-only `@clay/*` scope, `tree-sitter-wasm` grammar paths, package-root-confined `.wasm`/`.scm` assets, known style tokens, duplicate-language checks, and `parse-document` + `render-decorations` permissions.
- `src/server/syntax.rs` owns `SyntaxGrammarRegistry`, active syntax grammar selection, `TreeSitterSyntaxHandler`, query/capture diagnostics, incremental cached-tree reuse, viewport-bounded capture extraction, `DecorationSet` validation, payload checks before cache insertion, and `SyntaxChunkCache` retention.
- `runtime/js/syntax.ts` and `src/server/ops/syntax.rs` expose the public `clay:syntax.serverRegisterSyntaxGrammar` facade/op for package load entries, while ordinary end-user config remains one-line `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, or `loadPackage("@clay/javascript")`.
- `packages/rust`, `packages/typescript`, and `packages/javascript` are grammar-only first-party packages: no major modes, commands, completions, SDUI, package UI, behavior manifests, configuration keys, or language-specific text transforms.
- `docs/development/launch-and-gui-smoke.md` and `tests/fixtures/configuration/syntax-grammars/init.js` document the manual smoke path; `tests/syntax_grammar.rs::manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow` provides deterministic load/select/decorate/edit/fallback coverage for non-interactive verification.

## Tests

- `tests/primitives_docs.rs`: static coverage that this review is linked from the wiki index and primitive architecture page; registry/backlog mention `SyntaxGrammarContribution`; the review records first-party grammar-only packages, active syntax grammar vs active major mode separation, hot-path split, and first-party-only artifact security.
- `tests/syntax_grammar.rs`: grammar contribution validation, query/capture mapping, package-root path rejection, disabled/invalid package fallback, no language-specific branch guards, Rust/TypeScript/JavaScript fixture `DecorationSet` generation, capture overflow rejection, and deterministic manual-smoke coverage.
- `tests/package_loading_docs.rs`: documentation-as-code coverage for grammar package authoring, loadPackage-only configuration, final wiki coverage, and package-loading authority boundaries.
- Commands: `CARGO_TARGET_DIR=target/pi-verify cargo test --test primitives_docs --test syntax_grammar --test package_loading_docs --quiet`.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.9 Generic Text/Code Modes Primitive Review](phase18.9-generic-text-code-modes-primitive-review.md)
- [Mode Registry](mode-registry.md)
- [Package Loading](package-loading.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Primitive Registry](../../reference/primitives/registry.md)
- [Primitive Backlog](../../reference/primitives/backlog.md)
- [Parse Update Strategy](../../reference/primitives/parse-update-strategy.md)
- [Rendering Strategy](../../reference/primitives/rendering-strategy.md)
- [Package Security](../../reference/primitives/package-security.md)
