# Phase 18.14 First-Party Rust, TypeScript, and JavaScript Language Package Expansion Primitive Review

## Source

- `roadmap.md`
- `plans/042-Phase18.14-First-Party-Rust-TypeScript-and-JavaScript-Language-Package-Expansion.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/package-loading.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/phase18.9-generic-text-code-modes-primitive-review.md`
- `docs/wiki/modules/phase18.10-tree-sitter-grammar-primitive-review.md`
- `docs/wiki/modules/phase18.11-completion-provider-primitive-review.md`
- `docs/wiki/modules/phase18.8-transient-menu-command-execution-primitive-review.md`
- `docs/wiki/modules/phase18.3-slot-ui-primitive-review.md`
- `docs/wiki/modules/phase18.4-input-state-config-primitive-review.md`
- `docs/wiki/modules/phase18.12-workspace-discovery-primitive-review.md`
- `docs/wiki/modules/phase18.13-git-discovery-primitive-review.md`
- `src/packages/modes.rs`
- `src/packages/record.rs`
- `src/packages/manifest.rs`
- `src/server/ops/modes.rs`
- `src/server/ops/commands.rs`
- `src/server/ops/completion.rs`
- `src/server/ops/syntax.rs`
- `src/server/ops/parse.rs`
- `src/server/ops/ui.rs`
- `src/server/ops/configuration.rs`
- `src/server/parse_coordinator.rs`
- `src/server/decorations.rs`
- `src/server/command_execution.rs`
- `runtime/js/modes.js`
- `runtime/js/commands.js`
- `runtime/js/completion.js`
- `runtime/js/syntax.js`
- `runtime/js/parse.js`
- `runtime/js/decorations.js`
- `runtime/js/ui.js`
- `runtime/js/configuration.js`
- `runtime/js/packages.js`
- `tests/primitives_docs.rs`

## Overview

Phase 18.14 expands `@clay/rust`, `@clay/typescript`, and `@clay/javascript` from grammar-only packages into real first-party language packages. The expansion must reuse existing editor/package primitives and add only generic, reusable gaps that future language packages can also consume. This review records the primitive inventory available to the three packages and identifies the gaps that must be filled before the expansion is implemented.

The target outcome is three first-party language packages that each declare a major mode, built-in commands, completion providers, behavior-manifest rules, optional package UI contributions, and configuration options while continuing to reuse the existing `SyntaxGrammarContribution` primitive for syntax highlighting.

## Existing Primitive Inventory

### Document classification and major-mode activation

- `src/packages/modes.rs::ModeRegistry` owns document classification, active major-mode state, fallback registration, and behavior manifest selection.
- Phase 18.9 supplies always-on `core.text` and `core.code` fallback modes through `DocumentClassification` and `MajorModeActivation`.
- `runtime/js/modes.js` exposes `serverRegisterModePattern`, `serverClassifyDocument`, `serverActivateMajorMode`, and `serverActivateClassifiedMode` for package load-time mode registration and open-time activation.
- `@clay/rust`, `@clay/typescript`, and `@clay/javascript` can register their own major modes for `.rs`, `.ts`/`.tsx`, and `.js`/`.jsx`/`.mjs`/`.cjs` files. Classification rules must remain generic: file-extension patterns, MIME-type hints, shebang probes, and bounded leading-content probes. No language-specific Rust classification branches are permitted.

### Behavior manifests and text transforms

- `src/server/ops/modes.rs` validates generic `EditorBehaviorRules` including `EnterRule`, `PairRule`, `CommentContinuationRule`, `TabRule`, and `ElectricCharacterRule`.
- Behavior manifests keep hot-path editing `ClientFirstPredictable` by installing inert rule data that the Rust client applies locally.
- Language packages can express electric characters, delimiter pairs, comment continuation, tab/indent policy, and autocomplete triggers through the existing manifest shape without new primitive categories.

### Command declaration and execution

- `src/packages/commands.rs` and `src/server/ops/commands.rs` register package-prefixed command metadata with routing policy and authority.
- `runtime/js/commands.js` exposes `serverRegisterCommand`, `serverExecuteCommand`, and `serverListCommands`.
- Phase 18.8 added the server-owned `CommandExecution` boundary in `src/server/command_execution.rs` so SDUI actions, package UI action intents, keybindings, and transient-menu selections all route through one validated path.
- Language packages can declare commands such as "rust.toggleTestOutline" or "typescript.organizeImports" with inert metadata and server-side handlers; registration does not grant execution authority, which is re-checked at activation time.

### Syntax grammar contribution

- Phase 18.10 implemented `SyntaxGrammarContribution` as a generic package-provided Tree-sitter grammar primitive.
- `runtime/js/syntax.js` and `src/server/ops/syntax.rs` expose `serverRegisterSyntaxGrammar` for grammar-only package load entries.
- Active syntax grammar remains separate from active major mode: a document can be editable as `core.code` or a language major mode while a grammar supplies highlighting. This review records that active syntax grammar separate from active major mode is a reusable primitive boundary that language package expansion must preserve.
- Phase 18.14 keeps the existing grammar contributions in `@clay/rust`, `@clay/typescript`, and `@clay/javascript` unchanged and adds mode/command/completion/configuration metadata alongside them.

### Completion trigger and result providers

- Phase 18.11 implemented `CompletionTriggerAndResult` as a generic completion provider framework.
- `runtime/js/completion.js` exposes `serverRegisterCompletionProvider` for package load-time provider registration.
- The framework reuses behavior-manifest autocomplete triggers, client local-edit-first routing, a cancellable server-side `UiReactivePriority` lane, and `TransientMenuSession` for display/acceptance.
- Language packages can register completion providers for keywords, snippets, and buffer-word augmentations. LSP, workspace-index, AI, network, shell, and filesystem-backed providers are out of scope for Phase 18.14.

### Parse handler bridge and incremental parse updates

- `runtime/js/parse.js` exposes `serverRegisterParseHandler` for server-side package parse handlers.
- `src/server/parse_coordinator.rs` schedules cancellable background parse work, enforces parse-window budgets, rejects stale versions, and publishes viewport-prioritized results.
- Language packages can register mode-scoped parse handlers that return inert decoration/folding/diagnostic data. The handler must not run in Masonry hot paths, block local paint, or access filesystem/network/shell/AI authority.

### Decoration transport

- `runtime/js/decorations.js` exposes `serverPublishDecorations` for bounded server-side decoration publication.
- `src/server/decorations.rs` validates document versions, byte ranges, style tokens, permissions, provenance, and `DECORATION_PAYLOAD_BUDGET_BYTES`.
- Language packages can publish syntax, semantic, search-match, and diagnostic decoration spans derived from parse handlers or grammar contributions.

### Package UI contributions

- Phase 18.3 implemented runtime-backed `clay:ui` contribution APIs: `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`.
- Phase 18.4 added `serverRegisterInputContribution`, `serverRegisterUiStateScope`, and `serverSetLayoutOverride`.
- Language packages can declare fixed panels (outline, symbols, diagnostics), transient overlays (hover/documentation, quick fixes), status items, and package-prefixed theme tokens without direct Masonry widget access, raw CSS, or client-side JavaScript.

### Package configuration and layout overrides

- `runtime/js/configuration.js` exposes `setPackageOption` for package-prefixed typed options.
- `runtime/js/ui.js` exposes `serverSetLayoutOverride` for user/package/mode layout defaults.
- Language packages can expose options such as default panel visibility, preferred slot, formatter preferences, and theme-token remaps through documented `~/.config/clay/init.js` Clay JS APIs.

### Package loading and provenance

- `runtime/js/packages.js` exposes `loadPackage` as the one-line default end-user loader.
- Phase 18.6 generalized the resolver so bundled `@clay/*` packages and installed npm/GitHub/git/tarball/local-path packages share the same `PackageService` validation, authorization, package-root confinement, module-loader allowlist, and `loadEntry` execution path.
- Language packages load with `await loadPackage("@clay/rust")`, `await loadPackage("@clay/typescript")`, or `await loadPackage("@clay/javascript")` from `~/.config/clay/init.js`.

### Workspace discovery and Git status

- Phase 18.12 implemented server-owned workspace-root discovery and bounded directory listing.
- Phase 18.13 implemented a read-only `GitDiscoveryService` and first-party `@clay/git` package.
- Language packages may consume these services through command execution and package UI panels, but they do not own workspace root/marker/ignore authority or Git mutation authority.

## Generic Phase 18.14 Primitive Gaps

### Mode-scoped command/action metadata helper

Language packages need a concise way to declare commands that are scoped to their major mode and automatically included in the mode's behavior manifest. The existing `serverRegisterCommand` plus manual keymap/command wiring in `serverRegisterModePattern` works, but a small mode-scoped command helper would reduce boilerplate and ensure consistent prefix/routing/authority.

Acceptable implementation: a package-side helper that calls `serverRegisterCommand` and returns a command descriptor for inclusion in `serverRegisterModePattern`. Rejected implementation: a new primitive that bypasses `CommandDeclaration` or `CommandExecution` validation.

### Language-package behavior-manifest presets

Electric characters, delimiter pairs, and comment rules are nearly identical across C-family languages. A generic `behaviorManifestPreset` helper or documented JSON preset for "curly-brace language", "C-style comments", and "indent with spaces/tabs" would keep the three packages declarative without language-specific Rust branches.

Acceptable implementation: package-side preset objects in `@clay/rust`/`@clay/typescript`/`@clay/javascript` load entries. Rejected implementation: Rust code that chooses behavior based on `modeId`.

### Parse-handler lifecycle for language modes

Grammar-only packages use `SyntaxGrammarContribution` for Tree-sitter highlighting. Full language packages need a parse handler that can reuse the same Tree-sitter tree for semantic decorations, folding, diagnostics, and outline data. The generic gap is a documented pattern for registering a parse handler that shares a Tree-sitter parser instance with the syntax grammar contribution, or a clear boundary when the parse handler should run independently.

Acceptable implementation: a package-side adapter that imports the grammar artifact and Tree-sitter query files from the package root and registers a `serverRegisterParseHandler` for the language mode. Rejected implementation: Rust code that hard-codes Tree-sitter grammar paths for Rust/TypeScript/JavaScript.

### Completion provider for language keywords/snippets

The three packages need a lightweight completion provider that can serve static keyword and snippet lists. The existing `CompletionTriggerAndResult` framework supports this, but a small package-side helper for "static keyword list + snippet expansion" would avoid duplicated boilerplate.

Acceptable implementation: a package-side helper that calls `serverRegisterCompletionProvider` with a static item list. Rejected implementation: a new Rust primitive that maintains per-language keyword tables.

### Symbol/outline panel contribution

A generic `PanelContribution` can already host a list of symbols or outline entries. The gap is a documented component pattern and optional server-side helper that converts parse-handler output into bounded component-tree updates for a `list`/`label` panel. This is a composition guide, not a new primitive.

Acceptable implementation: package-side code that builds `ComponentContributionDefinition` trees from parse results and registers a panel. Rejected implementation: a Rust `LanguageOutlinePanel` branch.

### Status-item contribution

Language packages need a small status area contribution for mode/line-ending/indentation state. The existing `ComponentContribution` catalog includes `statusItem`. The gap is documentation and a package-side helper for registering a status item with action targets.

Acceptable implementation: package-side helper calling `serverRegisterComponentContribution` with `kind: "statusItem"`. Rejected implementation: a Rust status-bar branch per language.

## What the Three Packages Can Achieve with Existing Primitives

With the inventory above, `@clay/rust`, `@clay/typescript`, and `@clay/javascript` can implement:

- A major mode for their file extensions via `serverRegisterModePattern` + `serverActivateMajorMode`.
- Built-in commands via `serverRegisterCommand` + `serverExecuteCommand` for formatting, toggling comments, inserting snippets, and opening symbol/outline panels.
- Behavior-manifest rules for electric characters, delimiter pairs, comment continuation, tab/indent policy, and autocomplete triggers.
- Syntax highlighting via the unchanged Phase 18.10 `SyntaxGrammarContribution`.
- Semantic decorations, folding ranges, diagnostics, and outline data via `serverRegisterParseHandler` + `serverPublishDecorations`.
- Keyword/snippet completion providers via `serverRegisterCompletionProvider`.
- Package UI panels (outline, symbols, diagnostics), transient overlays (hover/help), status items, and theme tokens via `clay:ui` contribution APIs.
- Configuration options and layout overrides via `clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride`.
- One-line default loading via `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, or `loadPackage("@clay/javascript")`.

## Hot-Path Classification

- Package load / mode registration: validate manifest metadata, register mode pattern, commands, completion providers, parse handlers, UI contributions, theme tokens, state scopes, and configuration options.
- Document open / reload / reclassification: classify document, activate language major mode or fall back to `core.code`, select active syntax grammar independently.
- Background parse / completion work: run Tree-sitter parsing, semantic analysis, and completion computation as `Background` or `UiReactivePriority` no-hot-path work; publish bounded decoration/completion payloads.
- Command execution: server-first validation of command ID, routing policy, permissions, target context, argument budget, and session/action freshness before side effects.
- Configuration / layout override evaluation: package load or explicit setting-change time only.
- Paint/text-event/key hot path: read already-installed behavior manifests, decoration sets, and UI state. No package JavaScript, parse handlers, completion providers, command handlers, configuration evaluation, or synchronous IPC in Masonry paint/layout/pointer/scroll/key/text-event handlers.

## Security and Authority Boundary

- No language-specific Rust branches: all language behavior is expressed through package JavaScript, manifest data, and generic Clay primitives.
- No arbitrary file IO, shell, network, package-manager, AI, WASM, or raw-op authority: language packages receive only the permissions they declare (`mode-registration`, `command-registration`, `parse-document`, `render-decorations`, `completion-provider`, `package-configuration`, etc.) and user authorization.
- No client-side JavaScript: the Rust client receives only inert manifests, decorations, parse updates, SDUI/component trees, and validated UI state.
- No direct Masonry/native widget access: package UI is composed from the Clay component catalog and validated by Clay before client publication.
- Package-root confinement: grammar artifacts, query files, parse-handler modules, and load entries remain under the validated package root.
- LSP, full language-server protocol integration, workspace-wide symbol indexes, network-backed completions, AI-assisted code generation, arbitrary process execution, and package enable/disable authority remain out of scope for Phase 18.14.

## Rejected Implementation Shapes

- Do not add Rust server/client branches such as `if mode == "rust"`, `if extension == "ts"`, `RustMode`, `TypeScriptMode`, or `JavaScriptMode` outside the generic mode registry.
- Do not implement language-specific parser branches for Tree-sitter grammars; reuse the generic `SyntaxGrammarContribution` and `serverRegisterParseHandler` bridge.
- Do not create language-specific native widgets, status bars, sidebars, or overlays; reuse `clay:ui` contribution primitives.
- Do not run language package JavaScript in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers.
- Do not publish raw AST nodes, parser state, executable snippets, or renderer callbacks to the Rust client.
- Do not auto-load language packages; end users use explicit one-line `loadPackage` calls from `~/.config/clay/init.js`.

## Final Implementation Status

Phase 18.14 has not yet implemented the expanded language packages. The entry gate (Phase 18.10 grammar primitive, Phase 18.11 completion framework, Phase 18.8 command execution, Phase 18.9 generic fallback modes) is complete and passing. This review records that the reusable primitive surface is sufficient for the expansion and identifies only small package-side helper/documentation gaps rather than new primitive categories.

Expected implementation artifacts:

- Updated `@clay/rust`, `@clay/typescript`, and `@clay/javascript` `package.json` files with `modes`, `commands`, `completionProviders`, `configuration`, and `contributions` metadata.
- Updated `dist/load.js` entries that register mode patterns, commands, completion providers, parse handlers, UI contributions, theme tokens, and configuration options.
- Package docs under `docs/reference/packages/rust.md`, `docs/reference/packages/typescript.md`, and `docs/reference/packages/javascript.md`.
- End-to-end tests proving `loadPackage("@clay/rust")` activates the Rust mode, grammar, commands, and completion provider.

## Tests

- `tests/primitives_docs.rs`: static coverage that this review is linked from the wiki index and primitive architecture page; registry/backlog mention language-package-relevant primitives; the review records first-party language package expansion, active syntax grammar versus active major mode separation, hot-path split, and security boundaries.
- `cargo test --test protocol primitives_docs::`: runs the primitive documentation coverage suite.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.9 Generic Text/Code Modes Primitive Review](phase18.9-generic-text-code-modes-primitive-review.md)
- [Phase 18.10 Tree-sitter Grammar Primitive Review](phase18.10-tree-sitter-grammar-primitive-review.md)
- [Phase 18.11 Completion Provider Framework Primitive Review](phase18.11-completion-provider-primitive-review.md)
- [Phase 18.8 Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md)
- [Phase 18.3 Slot-Aware Package UI Primitive Review](phase18.3-slot-ui-primitive-review.md)
- [Phase 18.4 Input, State, and Configuration Primitive Review](phase18.4-input-state-config-primitive-review.md)
- [Phase 18.12 Workspace Discovery and File Browser Foundation Primitive Review](phase18.12-workspace-discovery-primitive-review.md)
- [Phase 18.13 Git Discovery Service Primitive Review](phase18.13-git-discovery-primitive-review.md)
- [Mode Registry](mode-registry.md)
- [Command Registry](command-registry.md)
- [Package Loading](package-loading.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Primitive Registry](../../reference/primitives/registry.md)
- [Primitive Backlog](../../reference/primitives/backlog.md)
