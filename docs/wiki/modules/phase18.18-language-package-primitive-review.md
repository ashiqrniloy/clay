# Phase 18.18 First-Party Language Package Full Implementation Primitive Review

## Source

- Plan: `plans/050-Phase18.18-First-Party-Language-Package-Full-Implementation.md` (task 2).
- Roadmap: `roadmap.md` Phase 18.18.
- Decision: `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md` (first-party package expansion component) and `decision-logs/2026-07-11-1418-semantic-font-roles-and-user-owned-typography.md`.
- Patterns: `.agents/skills/project-patterns/references/language-capability-sequencing.md`, `mode-primitive-first.md`, `protocol-and-performance.md`, and `authority-boundaries.md`.
- Predecessor reviews: `docs/wiki/modules/phase18.14-language-package-expansion-primitive-review.md`, `docs/wiki/modules/phase18.16-tiered-tree-sitter-engine-primitive-review.md`, `docs/wiki/modules/phase18.16.5-typography-primitive-review.md`, `docs/wiki/modules/phase18.17-range-diagnostics-primitive-review.md`.
- `src/server/syntax.rs` (`FIRST_PARTY_NATIVE_GRAMMARS`, `NativeGrammarDescriptor`, `DEFAULT_NATIVE_STYLE_MAP`, `MARKDOWN_NATIVE_STYLE_MAP`, `SyntaxEngineTier`, `TreeSitterSyntaxHandler`, `SyntaxGrammarRegistry`).
- `src/packages/record.rs` (`SyntaxStyleMapEntry`, `SyntaxGrammarContributionDescriptor`, `is_known_syntax_style_token`).
- `src/protocol/decorations.rs` (`TokenType`, `Modifiers`, `DecorationSpan`, `classify_style_token`, `from_style_token`).
- `src/editor/theme.rs` (`StyleRegistry`).
- `src/protocol/completion.rs` (`CompletionItem`), `src/server/completion.rs`, `runtime/js/completion.ts` (`serverRegisterCompletionProvider`).
- `runtime/js/behavior.ts` (`buildCodeEditingManifest`), `runtime/js/syntax.ts` (`serverRegisterSyntaxGrammar`), `runtime/js/modes.ts` (`serverRegisterModePattern`), `runtime/js/commands.ts` (`serverRegisterCommand`), `runtime/js/ui.ts` (`serverRegisterComponentContribution`), `runtime/js/packages.ts` (`loadPackage`).
- `packages/{rust,typescript,javascript,markdown}/package.json`, `dist/index.js`, `dist/load.js`, `queries/highlights.scm`.
- `tests/primitives_docs.rs`, `tests/syntax_grammar.rs`, `tests/completion_provider.rs`, `tests/package_loading.rs`, `tests/editor_performance_invariants.rs`, `tests/performance_protocol.rs`.

## Overview

Phase 18.18 expands the Phase 18.14 grammar-only first-party packages (`@clay/rust`, `@clay/typescript`, `@clay/javascript`, `@clay/markdown`) into full first-party language packages: themed highlighting, full mode behavior, and base keyword completion, all on generic primitives and the Phase 18.16 Tier 1 native engine, rendered through the Phase 18.15 vocabulary + theme registry and the Phase 18.16.5 typography contract. This review records the primitive inventory available to the four packages after Phases 18.15–18.17 landed and identifies the generic gaps that must be filled before the expansion is implemented.

The target outcome is four first-party language packages whose grammar captures map to true Phase 18.15 vocabulary `TokenType` + `Modifiers` (not the lossy free-form `style_token` compatibility fallback), whose mode behavior rides the generic `buildCodeEditingManifest`, whose base completion rides the generic `serverRegisterCompletionProvider`, and whose Markdown decoration folds onto the Tier 1 native `tree-sitter-md` engine while the preview SDUI panel stays package-JS. No per-language Rust branch is added.

## Existing Primitive Inventory

### Tiered syntax engine and first-party native grammars

`src/server/syntax.rs::SyntaxGrammarRegistry::with_first_party_native()` already registers `src/server/syntax.rs::FIRST_PARTY_NATIVE_GRAMMARS` at server startup: `tree-sitter-rust` (rust, `.rs`), `tree-sitter-typescript` (typescript `.ts` + tsx `.tsx`), `tree-sitter-javascript` (javascript `.js`/`.jsx`/`.mjs`/`.cjs`), and `tree-sitter-md-025` (markdown `.md`/`.markdown`/`.mdown`). Each `NativeGrammarDescriptor` carries `grammar_source`, a `highlights_query_path` pointing at the package `queries/highlights.scm`, a static `style_map`, and a `language: fn() -> Language` constructor. The host knows no language names beyond this data table; dispatch is by grammar/extension lookup, never `match language_id`.

`SyntaxEngineTier::{Native, Wasm, JavaScriptFallback}` selects the engine. First-party languages default to Tier 1 `Native`; a package may declare a higher-priority Tier 2 wasm grammar to override it only through documented, user/package-initiated engine selection recorded in provenance. `is_shadowed_by_native_first_party` means the Phase 18.14 package `tree-sitter-wasm` contributions no longer drive first-party highlighting — the native registration owns the actual parse.

This surface is reusable unchanged: Phase 18.18 adds no new native grammar registration and no per-language Rust syntax branch. The only change is what the `style_map` emits.

### Capture-to-vocabulary mapping (current style-token state)

Today the native style maps (`DEFAULT_NATIVE_STYLE_MAP`, `MARKDOWN_NATIVE_STYLE_MAP`) and the package `src/packages/record.rs::SyntaxStyleMapEntry` emit **free-form style-token strings** such as `keyword.control`, `string.quoted`, `comment.line`, `punctuation.definition`, `markup.heading.1..6`, `markup.strong`, `markup.emphasis`, `markup.inline-code`, `markup.code-block`, and `markup.list-marker`. `is_known_syntax_style_token` is the closed allowlist.

These strings reach the two-axis model only through the Phase 18.15 compatibility mapping: `TokenType::classify_style_token` and `DecorationSpan::from_style_token` convert them into `TokenType` + `Modifiers` (e.g. `markup.strong` → `Paragraph` + `Bold`). The mapping is lossy: it cannot express `Function` + `Declaration`, `Heading1` directly, distinct `Type`/`Interface`/`Struct`, or `Bold|Italic` combinations, because the source vocabulary is the old flat style-token set. First-party grammars therefore render through the compatibility fallback rather than emitting true vocabulary tokens.

### Two-axis decoration, style registry, and native rendering

`src/protocol/decorations.rs::DecorationSpan` already carries the Phase 18.15 two axes: `token_type: TokenType` (closed LSP base set + Clay prose extension `Heading1..6`, `ListItem`, `Quote`, `CodeBlock`, `CodeSpan`, `Link`, `Paragraph`), `modifiers: Modifiers` (LSP base `Declaration`/`Definition`/`Readonly`/`Static`/`Deprecated`/`Abstract`/`Async`/`Modification`/`Documentation`/`DefaultLibrary` + Clay `Bold`/`Italic`/`Underline`/`Strikethrough`), the orthogonal `kind` layer (`Syntax`/`Semantic`/`Diagnostic`/`SearchMatch`), priority, optional syntax/semantic font role, and `DecorationProvenance`.

`src/editor/theme.rs::StyleRegistry` is the single source of color: `token_type + modifiers → StyleSpec{color, bold, italic, underline, strike}` plus base UI keys. Gruvbox Material Dark and Light first-party themes ship full token/UI mappings. Paint reads the resolved registry only; source-guard tests reject `Color::from_rgb8`/`Color::from_rgba8` literals outside theme-definition modules.

This rendering contract is the target the first-party styleMaps must emit into directly.

### Behavior manifests and text transforms

`runtime/js/behavior.ts::buildCodeEditingManifest` already accepts generic `CodeEditingManifestOptions`: `indentSize`, `lineComment`, `pairs: Array<{ open, close }>`, `electricOutdentCharacters`, and `autocompleteTriggers`. The editor core deserializes the result into language-agnostic `EnterRule`, `PairRule`, `CommentContinuationRule`, `TabRule`, and `ElectricCharacterRule` types. No language-specific behavior logic lives in the Rust client or server.

The Phase 18.14 packages already call `buildCodeEditingManifest` with placeholder values; Phase 18.18 tunes those values to language-appropriate parameters (rust indent 4; typescript/javascript indent 2; markdown prose-appropriate). This is package data, not a new primitive.

### Command declaration and execution

`runtime/js/commands.ts::serverRegisterCommand` plus the Phase 18.8 server-owned `CommandExecution` boundary register package-prefixed command metadata with routing policy and authority. First-party comment-toggle/insert commands (`rust.toggleLineComment`, `typescript.toggleLineComment`, `javascript.toggleLineComment`, plus existing markdown commands) are server-first inert metadata; registration grants no execution authority, which is re-checked at activation.

### Completion trigger and result providers

Phase 18.11's `CompletionTriggerAndResult` framework exposes `runtime/js/completion.ts::serverRegisterCompletionProvider` for package load-time provider registration. The framework reuses behavior-manifest autocomplete triggers, client local-edit-first routing, a cancellable server-side `UiReactivePriority` lane, and `TransientMenuSession` for display/acceptance.

`src/protocol/completion.rs::CompletionItem` is inert text-replacement data only: "No callbacks, snippets …". The snippet kind is **not yet present**; it is planned for Phase 18.19 (snippets, exclusive claim, disable-native). Phase 18.18 therefore ships **keyword-only** base providers (`rust.keywords`, `typescript.keywords`, `javascript.keywords`, `markdown.keywords`) and defers snippet content to Phase 18.19.

### Parse coordinator, incremental updates, and diagnostics

`src/server/parse_coordinator.rs::ParseCoordinator` owns background scheduling, cancellation, generation replacement, viewport prioritization, stale-version rejection, and the `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` gate. `IncrementalParseUpdate` carries optional `decoration_update` and (Phase 18.17) `diagnostic_update` side channels. `TreeSitterSyntaxHandler` reuses cached parsers/trees, maps generic captures through one language-neutral path, and emits Phase 18.17 `ERROR`/`MISSING` range diagnostics with no per-language branch.

First-party languages already get syntax-error squiggles through this generic path. Phase 18.18 changes nothing here; the vocabulary styleMap change flows through the same `decoration_update` channel.

### Markdown decoration versus preview SDUI

Markdown decoration already routes through the Tier 1 native `tree-sitter-md-025` engine and `MARKDOWN_NATIVE_STYLE_MAP`. The Markdown preview SDUI panel is produced by package-JS (`packages/markdown/dist/sdui.js`) and published through the validated `SduiSnapshot` path; the package-JS `parser.js` remains as the Tier 3 fallback for grammar-less languages. Decision 0352 fixes decoration and preview as separate paths: decoration folds onto the generic pipeline; preview stays package-JS.

Phase 18.18 moves the Markdown decoration styleMap onto the vocabulary contract (same gap as the code languages) and leaves preview SDUI untouched.

### Package UI, configuration, and loading

`runtime/js/ui.ts::serverRegisterComponentContribution` registers inert `statusItem` and other component contributions. `runtime/js/configuration.ts` and `runtime/js/ui.ts` expose package options and layout overrides. `runtime/js/packages.ts::loadPackage` is the one-line default end-user loader; the Phase 18.6 generic resolver carries each first-party package's `loadEntry` end-to-end with no copied manifests or manual primitive registration. The four packages already load through `loadPackage("@clay/*")`.

## Generic Phase 18.18 Gaps

### Promote first-party grammar styleMaps to vocabulary `TokenType` + `Modifiers`

This is the central generic gap. `SyntaxStyleMapEntry` (`src/packages/record.rs`) and the native `NativeGrammarDescriptor::style_map` (`src/server/syntax.rs`) must gain the ability to express Phase 18.15 `TokenType` + `Modifiers` directly, so first-party grammar captures emit true vocabulary tokens instead of the lossy `style_token` compatibility fallback.

Acceptable implementation: extend the style-map primitive generically — a style-map entry may carry either the legacy `style_token` string (kept for third-party/back-compat) or a `{ tokenType, modifiers }` vocabulary mapping (plus the existing optional `fontRole`). Update `DEFAULT_NATIVE_STYLE_MAP` and `MARKDOWN_NATIVE_STYLE_MAP` (and the four package `styleMap` contributions) to the vocabulary form: `keyword → Keyword`, `string → String`, `comment → Comment`, `function.declaration → Function + Declaration`, `type → Type`/`Interface`/`Struct`, Markdown `**x** → Paragraph + Bold`, `_x_ → Paragraph + Italic`, `# h1 → Heading1`, code spans/blocks → `CodeSpan`/`CodeBlock` (+ `Monospace` font role). The capture name in `queries/highlights.scm` is the join key; the styleMap maps capture → vocabulary, never capture → color. Unmatched captures stay unstyled (no crash, no default color leak).

Rejected implementation: per-language Rust branches that choose `TokenType` by `language_id`; encoding vocabulary into `style_token` strings; adding color data to the styleMap; a parallel styleMap primitive per language.

### Expand `queries/highlights.scm` captures to match vocabulary coverage

The current package queries emit only a small capture set (`keyword`, `string`, `comment`, `punctuation`). To populate the vocabulary styleMap, the queries must capture function/type/class/interface names, numbers, operators, and Markdown prose structures with named captures the styleMap then maps. This is inert query-data authoring per package (recorded provenance from upstream `tree-sitter-*` releases), not Rust work. Two-axis coverage is test-enforced: a Markdown `**x**` span emits `modifiers=Bold` with no token-type color; a function name can carry `modifiers=Bold|Declaration`.

### Markdown decoration onto the vocabulary contract; preview stays package-JS

Markdown decoration must emit prose vocabulary tokens (`Heading1..6`, `Paragraph + Bold`, `Paragraph + Italic`, `CodeSpan`, `CodeBlock`, `ListItem`, `Link`, `Quote`) through the same generic styleMap promotion as the code languages. The Markdown preview SDUI panel remains package-JS and is unchanged in behavior; decoration and preview must be independently activatable. The package-JS `parser.js` decoration path is demoted from the decoration hot path where the Tier 1 native engine now covers it and remains only as the Tier 3 fallback.

### Priority-0 base keyword completion providers per language

Each language ships a base keyword completion provider (`rust.keywords`, `typescript.keywords`, `javascript.keywords`, `markdown.keywords`) at the documented base priority through the existing generic `serverRegisterCompletionProvider`. Snippets are deferred to Phase 18.19 because `CompletionItem` does not yet carry a snippet kind; Phase 18.18 ships keyword-only providers and records the deferral. Implemented shape: package-owned `items: string[]` data is validated for uniqueness, item/result count, field length, metadata payload, permission, and provenance by generic `CompletionProviderContributionDescriptor` parsing, then normalized to `CompletionProviderMeta.items: Vec<CompletionItem>`. The successful runtime evaluation retains an inert Rust snapshot, and the generic connection path selects active-package/trigger metadata and prefix-filters static items without invoking package JavaScript; richer providers sort ahead of base priority 0. No Rust module contains language keyword tables or provider-ID branches.

### Full mode behavior tuning on generic primitives

Each language mode tunes `buildCodeEditingManifest` parameters (indent size, line comment, electric/outdent characters, bracket/quote pairs, autocomplete triggers) to language-appropriate values and registers the comment-toggle command and status-item contribution through generic facades. Markdown list-continuation behavior is added only if the generic manifest helper already supports it; otherwise the gap is recorded here and deferred (no Markdown-specific Rust). This is package data, not a new primitive.

## What Existing Primitives Already Achieve

Without new scheduling, package execution authority, typography, or rendering authority, Clay can already:

- run Tier 1 native Tree-sitter for rust/typescript/tsx/javascript/markdown asynchronously over bounded server-provided windows, reusing cached parsers/trees;
- cancel superseded work and stale-drop old document/runtime generations;
- emit Phase 18.17 syntax-error range diagnostics from `ERROR`/`MISSING` nodes with no per-language branch;
- transport versioned viewport decoration/diagnostic chunks through validated `rkyv` messages;
- resolve all editor colors through `StyleRegistry` from `TokenType + Modifiers`;
- apply cached Parley typography-aware geometry and paint native Vello spans;
- register modes, commands, completion providers, parse handlers, UI contributions, and theme tokens through generic facades;
- load each first-party package with a one-line `loadPackage("@clay/*")`.

Phase 18.18 therefore needs a style-map vocabulary promotion plus package data/behavior tuning, not a language subsystem, a new engine, or a renderer plugin framework.

## Data Flow and Reuse Rule

```text
package queries/highlights.scm (capture names)
  + package/native styleMap (capture -> TokenType + Modifiers [+ fontRole])
  -> Tier 1 native TreeSitterSyntaxHandler parse over bounded windows
  -> DecorationSpan { token_type, modifiers, kind=Syntax, font_role, provenance }
  -> IncrementalParseUpdate::decoration_update
  -> StyleRegistry resolves token_type + modifiers -> StyleSpec
  -> native Vello paint through cached Parley geometry

Markdown preview: packages/markdown/dist/sdui.js -> validated SduiSnapshot (separate path)
No language name enters Rust syntax/manifest/completion branches.
```

Future first-party language additions and Phase 18.21 LSP enrichment reuse this pipeline; they do not add a per-language Rust highlighter, a parallel styleMap, a client widget, or a language-specific paint branch.

## Hot-Path Classification

| Work | Allowed location |
| --- | --- |
| Tree-sitter parse, capture extraction, vocabulary mapping, `ERROR`/`MISSING` traversal | server background parse task over bounded windows |
| styleMap validation, capture-to-vocabulary resolution, serialization | grammar registration (load) and parser result path |
| `queries/highlights.scm` query compilation | grammar registration (load), cached thereafter |
| Mode/completion/command/UI contribution registration | package load time only |
| Protocol decode, version/source replacement, cache pruning | client event application before paint |
| Parley geometry creation | existing visible layout rebuild/cache path |
| Masonry paint, layout, keypress, pointer, scroll, text-event handlers | cached local decoration spans, cached Parley geometry, resolved theme style, and installed behavior manifests only |

No package JavaScript, Tree-sitter traversal, query compilation, IPC, server validation, capture-to-vocabulary mapping, full-document scan, completion computation, or configuration evaluation belongs in Masonry paint, layout, keypress, pointer, scroll, or text-event handlers. `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, `SYNTAX_CACHE_BUDGET_BYTES`, `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `MODE_ACTIVATION_P95_BUDGET_MS`, and `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` remain applicable.

## Security and Authority Boundary

First-party grammar/query/styleMap data and mode/completion/command contributions are inert metadata. Server publication validates package identity, permission, current document/version, viewport bounds, style scope, provenance, and serialized size. This phase adds no filesystem, network, shell, AI, workspace mutation, language-server subprocess, native-ui, package-control, package-manager, raw-ops, client-runtime, raw CSS, Vello/Parley callback, native handle, or arbitrary WASM authority. Packages declare only the existing documented permissions (`mode-registration`, `mode-activation`, `command-registration`, `completion-provider`, `parse-document`, `render-decorations`). Arbitrary third-party grammar/native artifact loading and LSP subprocess authority remain out of scope; LSP enrichment is Phase 18.20/18.21 under a separate explicit decision and permissioned package.

## Rejected Implementation Shapes

- Do not add `RustSyntaxHandler`, `TypeScriptSyntaxHandler`, `JavaScriptSyntaxHandler`, `MarkdownSyntaxHandler`, or any `if language_id == "rust"` / `if mode == "typescript"` branch in server/client/editor code.
- Do not encode vocabulary into `style_token`/`scope` strings; promote the style-map primitive to `TokenType` + `Modifiers` generically.
- Do not add color data to grammar styleMaps; color lives only in `StyleRegistry` via theme packages.
- Do not add a per-language Rust keyword completion table; ship package keyword data through generic `serverRegisterCompletionProvider`.
- Do not run package JavaScript, Tree-sitter traversal, query compilation, IPC, validation, capture mapping, completion computation, or configuration evaluation in Masonry paint/layout/input paths.
- Do not route Markdown preview through the Tree-sitter engine; preview stays package-JS by decision 0352.
- Do not auto-load first-party language packages; end users use explicit one-line `loadPackage` calls from `~/.config/clay/init.js`.
- Do not implement LSP process spawning, hover, go-to-definition, code actions, or rename in Phase 18.18; those are Phase 18.20/18.21.

## Tests

- `tests/primitives_docs.rs::phase18_18_language_package_primitive_review_is_linked_and_complete`: locks inventory, vocabulary styleMap gap, Markdown decoration/preview split, completion keyword deferral, hot-path split, authority boundary, and rejected shapes.
- Implementation coverage (later tasks): `tests/syntax_grammar.rs` (vocabulary styleMap resolution, unmatched captures, multi-language-id parity, no-language-branch), `tests/completion_provider.rs` (base keyword provider registration/merge), `tests/package_loading.rs` (one-line load + no silent defaults), `tests/editor_performance_invariants.rs` (single-source-of-color, no-hot-path guards), `tests/performance_protocol.rs` (per-language payload/cache bounds).

Run:

```bash
cargo test --test primitives_docs phase18_18_language_package_primitive_review_is_linked_and_complete
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.14 First-Party Rust, TypeScript, and JavaScript Language Package Expansion Primitive Review](phase18.14-language-package-expansion-primitive-review.md)
- [Phase 18.16 Tiered Tree-sitter Syntax Engine Primitive Review](phase18.16-tiered-tree-sitter-engine-primitive-review.md)
- [Phase 18.16.5 Semantic Typography Primitive Review](phase18.16.5-typography-primitive-review.md)
- [Phase 18.17 Range Diagnostics Primitive Review](phase18.17-range-diagnostics-primitive-review.md)
- [Rendering Primitives](rendering-primitives.md)
- [Decoration Transport](decoration-transport.md)
- [Parse Coordinator](parse-coordinator.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md)
- [Text Vocabulary and Two-Axis Decoration Contract](../../reference/primitives/syntax-vocabulary.md)
- [Primitive Registry](../../reference/primitives/registry.md)
