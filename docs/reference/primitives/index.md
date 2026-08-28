# Clay Primitives Reference

This directory is the Phase 16 architecture source and current reference for package- and mode-controlled Clay primitives. These documents define the registry, security baseline, rendering/parse strategies, Markdown POC prerequisites, and implementation backlog for Phase 17 and Phase 18, including Phase 18.3 runtime-backed slot-aware package UI contribution primitives, Phase 18.4 runtime-backed package input/state/layout-override/configuration primitives, Phase 18.9 generic `core.text`/`core.code` fallback modes, shebang/content-probe classification, generic key-behavior (electric/pair/comment) transforms, mode-discovery commands, the Phase 18.16 tiered `SyntaxGrammarContribution` engine, and the Plan 099 server-authoritative editor performance primitives (package-author contract in [Creating Clay Packages](../packages/creating-packages.md#phase-1816-authoring-contract-tiered-syntax-engine)).

## Plan 098 document chunk transfer

`DocumentChunkTransfer` is an internal protocol v27 primitive for initial, open, reload, resync, and persisted-document restore text. Server-issued `DocumentTextHead` values carry total bytes plus one bounded first chunk; versioned client requests receive at most `MAX_CHUNK_BYTES` (256 KiB) from the authorized canonical rope. Invalid sizes, offsets, access, or versions fail with typed rejection. Transfer stays asynchronous and outside CodeMirror transactions, React render, paint, layout, and input handlers. See [Primitive Registry Schema](registry.md#category-matrix), [Protocol Codec](../../wiki/modules/protocol-codec.md), and [Desktop Typed Bridge](../../wiki/modules/desktop-typed-bridge.md).

## Plan 099 editor performance primitives

Plan 099 is implemented on the server-authoritative Tauri/React path. These
three primitives are internal implementation contracts, not package-facing
Clay JS APIs:

- `BytePositionIndex` (`frontend/src/editor/position-index.ts`) is the shared
  CodeMirror `bytePositionField`. It builds once for a `Text`; each document
  change path-copies only changed whole-line chunks of 64 lines. Leaves retain
  numeric UTF-16/UTF-8 widths only, so edit, viewport, decoration, diagnostic,
  fold, completion, intelligence, and selection paths share structure instead
  of rebuilding a document-sized line table. A single 1 MiB line remains an
  explicit O(line) conversion ceiling.
- `ViewportRenderPatch` (`src/protocol/parse.rs`, protocol v29) pairs one
  metadata-only `ViewportRenderRequest` id with exactly one complete, empty, or
  rejected answer. `covered_ranges` describe output authority, not parser
  context. Ordered decoration, diagnostic, and fold members apply in one client
  render transaction; Tauri may discard only obsolete whole patches per
  document, never sibling members.
- `SyntaxSession` (`src/server/syntax_session.rs`, owned by
  `ParseCoordinator`) keeps one latest-wins mailbox per document/grammar/
  generation. Native handlers run under four shared blocking permits, each
  document owns its parser/tree state, the retained tree cache is capped at 64
  states, and the per-generation mode-activation cache is capped at 64 entries.
  JavaScript handlers remain on the server runtime worker.

Packages continue to use documented server-side registration/publication APIs
such as `parse.serverRegisterParseHandler`,
`syntax.serverRegisterSyntaxGrammar`, `decorations.serverPublishDecorations`,
`diagnostics.serverPublishDiagnostics`, and
`folding.serverPublishFoldingRanges`. Package code does not receive the
position index, patch completion protocol, parser handles, trace contents, or
scheduler controls. See [Primitive Registry Schema](registry.md), [Incremental
Parse and Background Parse Update Strategy](parse-update-strategy.md),
[Rendering Customization Strategy](rendering-strategy.md), and [Performance
Fixtures and Baseline Workflow](../../development/performance.md).

## Phase 18.16 syntax engine summary

The Phase 18.10 grammar metadata baseline remains documented in [Creating Clay Packages](../packages/creating-packages.md#phase-1810-authoring-contract-grammar-only-syntax-packages). The syntax primitive is tiered behind one generic grammar-to-vocabulary path: **Tier 1** compiled first-party native `tree-sitter-*` descriptors, **Tier 2** package-root-confined web-tree-sitter WASM/query assets selected only by explicit user preference, and **Tier 3** server-side package-JavaScript fallback handlers. `setSyntaxEnginePreference` is evaluated at init/package-load/open/reclassification time; captures map to `TokenType` + `Modifiers`, open is non-blocking, and failures publish sanitized `RuntimeDiagnostic` values such as `parse.open_failed`. Parse/query work stays outside keypress, paint, layout, scroll, pointer, and text-event hot paths and remains bounded by parse/decor/cache budgets. Runtime performs no network fetch, shell/package-manager build, native-library load, or client-side JavaScript execution; third-party grammar trust remains deferred to Phase 23.

## Plan 056 low-latency syntax contract

The implemented path accepts one canonical `ParseInputEdit` for each consecutive document version and stable bounded window. `TreeSitterSyntaxHandler` reuses the matching Tree-sitter tree, applies `Tree::edit`, reparses once, and queries the UTF-8-safe envelope formed from `Tree::changed_ranges` plus explicit invalidations intersected with the visible range. One parse/capture pass produces complete captures and fans them into stable 128-byte `DecorationSet` outputs; output chunk count never creates parser jobs. Changed/visible chunks publish first, every member is validated atomically, and empty syntax chunks remain authoritative replacements.

Frontend decoration, diagnostic, and folding state fields interpolate validated inert syntax spans through optimistic edits, with generic broad-token edge inheritance and authoritative current-version replacement. Package grammar captures—not whitespace, idle, caret movement, or a language-specific scheduler branch—define whole-token and comment/string/prose/code boundaries. First-party packages remain opt-in through one explicit `await loadPackage("@clay/<language>")` line in `~/.config/clay/init.js`; no copied manifest or manual parser/decorator registration is required.

## Documents

- [Existing Primitive Audit](audit.md) — existing behavior manifest, SDUI, configuration, document/workspace, editor, and observability primitives.
- [Primitive Registry Schema](registry.md) — canonical primitive taxonomy, schema vocabulary, authority boundaries, performance budgets, planned Clay JS API shape stubs, and the Phase 18.16 tiered `SyntaxGrammarContribution` engine row.
- [Rendering Customization Strategy](rendering-strategy.md) — inert decoration/layout/render declarations and SDUI reuse for package rendering.
- [Clay Shell and Package UI/Layout Strategy](shell-layout-strategy.md) — Phase 18.1/18.2 architecture and runtime status plus Phase 18.3 runtime-backed package panel/component/overlay/theme-token contribution status and Phase 18.4 runtime-backed input/state-scope/layout-override/package-option status for the working area, pane/split tree, pane slots, package UI/state/style declarations, and the client implementation boundary.
- [UI Chrome Primitives](ui-chrome-primitives.md) — Phase 20.2 native chrome primitive layer (`src/shell/primitives.rs`), token-driven design, interaction states, accessibility roles, routing, conformance contract, and package authoring contract.
- [Incremental Parse and Background Parse Update Strategy](parse-update-strategy.md) — server-side parse task lifecycle, cancellation, viewport filtering, and fallback behavior.
- [Markdown Mode POC Requirements](markdown-mode-requirements.md) — Phase 18 readiness checklist for `@clay/markdown`.
- [Package Primitive Security and Provenance Requirements](package-security.md) — package prefix, permission, validation, conflict, and prohibited-authority baseline.
- [Phase 17 Package Loading Runtime Facades](package-loading.md) — package load/runtime boundaries, conflict handling, runtime facade wiring, hot-path policy, and Phase 18 decoration/parse handoff.
- [Primitive Implementation Gate](implementation-gate.md) — Phase 16.5 runtime validation gate, fixture format, load/activation scope boundary, and Phase 17/18 handoff.
- [Prioritized Primitive Backlog](backlog.md) — sortable Phase-17-required, Phase-18-required, and deferred primitive implementation backlog plus the Phase 17 prerequisite checklist.
- [Text Vocabulary and Two-Axis Decoration Contract](syntax-vocabulary.md) — Phase 18.15 locked LSP-based `TokenType` + `Modifiers` vocabulary, Clay prose/text-attribute extensions, open-string scope escape, compatibility mapping from free-form `style_token`, and the single-source-of-color `StyleRegistry` invariant.
- [Semantic Typography Roles](typography.md) — Phase 18.16.5 package/mode contract for document defaults, syntax/semantic range overrides, UI/component roles, user-owned fallback stacks/sizes, invalidation, hot paths, and prohibited concrete font authority.
- [Range Diagnostics](diagnostics.md) — Phase 18.17 `DiagnosticSpan`/`DiagnosticSet` contract for explicit analyzers, theme-owned squiggles, source-keyed replacement, and future LSP reuse without treating Tree-sitter recovery nodes as correctness authority or overloading `RuntimeDiagnostic`/`DecorationSpan`.
- [Language Intelligence and LSP 3.17 Bridge Contract](language-intelligence.md) — Phase 18.20 engine-neutral hover/definition/code-action/signature-help primitives, semantic/diagnostic/completion reuse, deny-by-default `language-server` sessions, and the package-boundary LSP 3.17 mapping checklist for Phase 18.21 bridges.

## Phase 20.1 UI design language, token catalog, and typography hierarchy

Phase 20.1 expanded the typed token catalog additively from five domains to ten (`color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, `density`), added the seven semantic `UiTextVariant` tokens (`body`, `title`, `status`, `display`, `section`, `detail`, `caption`) with a user-owned `UiTypographyHierarchy`, wired typed UI design-token overrides through `ActiveTheme.design_tokens` into a cached client-side `ResolvedUiTheme`, and moved panel/sidebar/density defaults behind resolved tokens. The authoritative token catalog lives in the clay-ui skill reference (`.agents/skills/clay-ui/references/tokens.md`); the package authoring contract is documented in [Creating Clay Packages](../packages/creating-packages.md#phase-201-authoring-contract-typed-token-catalog-typography-hierarchy-and-token-backed-defaults) and the panel/density default boundary in [Clay Shell and Package UI/Layout Strategy](shell-layout-strategy.md#phase-201-token-backed-panel-and-density-defaults). No new component kind was added; Phase 20.2/20.4/20.5 component and primitive work remain deferred.

## Phase 17 Readiness Summary

Phase 17 should implement package loading and mode primitives before Phase 18 starts the Markdown POC. The minimum Phase 17 gates are:

1. Package manifests carry Clay metadata (`apiPrefix`, permissions, modes, load/runtime entries) and are validated at enable/load time.
2. `DocumentClassification`, `MajorModeActivation`, and `CommandDeclaration` have planned Clay JS API stubs and implementation tasks.
3. Phase 16.5's [Primitive Implementation Gate](implementation-gate.md) validates fixtures and future package metadata before Phase 17 package installation/enable/load workflows expand.
4. Package contributions preserve prefix/provenance and reject duplicate mode names, duplicate command IDs, ambiguous key bindings, and undeclared permissions deterministically.
5. Per-document/per-mode behavior manifest selection can atomically install client-safe `ClientFirstPredictable` text transforms and server-routed commands.
6. Phase 18 primitives (`DecorationRange`, `IncrementalParseUpdate`, and Markdown SDUI/keybinding extensions) have explicit handoff entries in [backlog.md](backlog.md).
7. Phase 18.2 shell runtime primitives (`WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout`) are implemented as internal Rust foundations; Phase 18.3 package UI primitives (`PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, and `PackageThemeTokenDeclaration`) are runtime-backed inventory APIs through `clay:ui`; Phase 18.4 package input, UI state-scope, layout override, and package option APIs are runtime-backed through documented Clay JS APIs; working-area/split/direct slot mutation, durable state-value persistence, pane selector, multi-panel ordering, overlay z-order, cross-window layout, and package enable/disable remain planned/deferred.
