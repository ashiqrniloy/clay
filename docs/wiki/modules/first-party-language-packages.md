# First-Party Rust, TypeScript, JavaScript, and Markdown Language Packages

## Source

- `packages/rust/dist/index.js`
- `packages/rust/dist/load.js`
- `packages/rust/package.json`
- `packages/typescript/dist/index.js`
- `packages/typescript/dist/load.js`
- `packages/typescript/package.json`
- `packages/javascript/dist/index.js`
- `packages/javascript/dist/load.js`
- `packages/javascript/package.json`
- `packages/markdown/dist/index.js`
- `packages/markdown/dist/load.js`
- `packages/markdown/package.json`
- `runtime/js/behavior.js`
- `runtime/js/completion.js`
- `runtime/js/modes.js`
- `runtime/js/commands.js`
- `runtime/js/ui.js`
- `src/server/ops/modes.rs`
- `src/server/ops/completion.rs`
- `src/server/ops/commands.rs`
- `src/server/ops/ui.rs`
- `src/server/ops/behavior.rs`
- `src/server/ops/packages.rs`
- `src/packages/record/mod.rs`
- `src/packages/permissions.rs`
- `src/packages/modes.rs`
- `src/server/completion.rs`
- `src/server/parse_coordinator.rs`
- `src/server/syntax.rs`
- `src/server/js_runtime/mod.rs`
- `docs/reference/packages/rust.md`
- `docs/reference/packages/typescript.md`
- `docs/reference/packages/javascript.md`
- `docs/reference/packages/markdown.md`
- `docs/reference/packages/creating-packages.md`
- `docs/reference/primitives/syntax-vocabulary.md`
- `docs/reference/clay-js-api/behavior/build-code-editing-manifest.md`
- `docs/reference/clay-js-api/completion/completion-trigger-characters-from-editor-rules.md`
- `docs/reference/clay-js-api/completion/server-list-completion-providers-for-trigger.md`
- `docs/development/launch-and-gui-smoke.md`
- `tests/fixtures/configuration/language-packages/init.js`
- `tests/fixtures/configuration/language-packages/workspace/main.rs`
- `tests/fixtures/configuration/language-packages/workspace/main.ts`
- `tests/fixtures/configuration/language-packages/workspace/main.js`
- `tests/fixtures/syntax/{rust,typescript,typescript.tsx,javascript,javascript.jsx,javascript.mjs,javascript.cjs,markdown}.<ext>` and their four `*-invalid` counterparts
- `tests/fixtures/configuration/file-browser-workflow/init.js`
- `tests/fixtures/configuration/file-browser-workflow/workspace/main.rs`
- `tests/fixtures/configuration/file-browser-workflow/workspace/main.ts`
- `tests/fixtures/configuration/file-browser-workflow/workspace/main.js`
- `benches/first_party_language_baselines.rs`
- `tests/performance_protocol.rs`
- `tests/editor_performance_invariants.rs`
- `docs/development/performance.md`
- `tests/package_loading_docs.rs`
- `tests/manual_smoke_docs.rs`
- `tests/primitives_docs.rs`
- `src/server/js_runtime/mod.rs` (integration tests)

## Overview

Phase 18.14 expands the first-party `@clay/rust`, `@clay/typescript`, and `@clay/javascript` packages from grammar-only syntax-highlighting contributions into full language packages; Phase 18.18 completes those packages and brings `@clay/markdown` onto the same native grammar, behavior, command, priority-0 static completion, and status-item primitives. Each provider carries bounded inert package data rather than a language-specific Rust table or executable JavaScript callback. All of this is built on generic Clay primitives already provided by the runtime and server ops; no language-specific Rust branch was added for Rust, TypeScript, JavaScript, or Markdown editing behavior. The two-axis vocabulary (TokenType+Modifiers) emitted by the Tier 1 native grammars is LSP SemanticTokenType/SemanticTokenModifiers-aligned, so Phase 18.21 LSP enrichment builds directly on the same capture-to-vocabulary pipeline and active `StyleRegistry` without architectural changes. The authoritative package-author walkthrough is [Creating Clay Packages](../../reference/packages/creating-packages.md#phase-1818-authoring-contract-complete-first-party-language-packages); its vocabulary companion locks `styleMap` authoring, while this page documents implementation flow.

## Responsibilities

- Register a package-owned major mode with generic file-extension/file-name probes and semantic `defaultFontRole: "monospace"`.
- Publish a validated `EditorBehaviorRules` manifest (indentation, delimiter pairs, comment continuation, electric outdent, autocomplete triggers).
- Register one server-first command (`<lang>.toggleLineComment`).
- Register priority-0 metadata-only completion providers with trigger characters derived from the behavior manifest: `<lang>.keywords` uses plain strings; Rust and TypeScript also ship dedicated `.snippets` providers whose bounded structured items become provenance-bearing client-expanded `CompletionItem`s.
- Register one inert `statusItem` UI contribution (`<lang>.status.mode`).
- Render first-party syntax through compiled Tier 1 grammar descriptors, package queries, direct vocabulary styleMaps, and the generic background parse/decor transport.
- Remain explicit opt-in via `loadPackage("@clay/*")`; do not auto-activate or shadow built-in `core.code`/`core.text` fallbacks.

Phase 18.21 LSP bridge packages (`@clay/lsp-rust`, `@clay/lsp-typescript`, `@clay/lsp-javascript`, `@clay/lsp-markdown`) are separate packages that enrich loaded base language packages with live LSP capabilities when a language-server grant is authorized. Base packages operate independently: removing a bridge `loadPackage` or revoking its grant preserves Tier 1 syntax, keyword/snippet completion, Markdown preview, and all behavior. Bridge packages do not replace base providers; they merge at priority 100 non-exclusive with `serverDisableCompletion` as override. See [First-Party LSP Bridge Packages](first-party-lsp-bridge-packages.md) for the full bridge architecture.

Non-responsibilities:
- No LSP or language-server protocol integration.
- No workspace-wide symbol indexes, AI completions, network-backed completions, or toolchain execution.
- No client-side JavaScript, raw Deno ops, native widget creation, or raw CSS.
- No user-tunable configuration keys in Phase 18.14.

## How It Works

### Package manifest

Each package `package.json` declares:

- `clay.modes`: `["rust"]`, `["typescript"]`, or `["javascript"]`.
- `clay.permissions`: `mode-registration`, `mode-activation`, `command-registration`, `completion-provider`, `parse-document`, `render-decorations`.
- `clay.apiDependencies`: the Clay JS APIs the package calls (e.g., `modes.serverRegisterModePattern`, `commands.serverRegisterCommand`, `completion.serverRegisterCompletionProvider`, `ui.serverRegisterComponentContribution`, `behavior.buildCodeEditingManifest`, `completion.completionTriggerCharactersFromEditorRules`).
- `clay.contributions`: `modePatterns`, `commands`, `completionProviders`, `ui.components`, and `syntaxGrammars`. Completion descriptors carry unique bounded string or structured items; structured snippets use inert `insertText` plus `textFormat: "snippet"` and must not mix with plain items in one provider.

Manifest payloads are minified to stay under the behavior-manifest payload budget (`BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` = 2048).

### Load entry

`dist/load.js` is the default `loadEntry` evaluated by `loadPackage`. It imports the generic Clay facades and the package's own manifest builder, then:

1. Registers the Phase 18.10 syntax grammar via `serverRegisterSyntaxGrammar`.
2. Registers the major-mode pattern via `serverRegisterModePattern`, passing `editorRules` built by `buildCodeEditingManifest`.
3. Registers the line-comment command via `serverRegisterCommand`.
4. Registers priority-0 completion contributions via `serverRegisterCompletionProvider`, deriving `triggerCharacters` from `editorRules`; Rust/TypeScript package manifests include separate keyword and snippet providers, both loaded by the same call/path.
5. Registers the status item via `serverRegisterComponentContribution` with `kind: "statusItem"`.
6. For Markdown only, retains `parser.js` as a registered Tier 3 fallback; the package manifest no longer advertises it as the default decoration contribution.

On document open, generic classification returns the package/mode key. `ClayRuntimeEvaluation` carries grammar metadata and engine preferences; `register_native_syntax_handler` selects the path-matching native descriptor, compiles its query from bundled package query text, and installs it under the selected grammar ID. `schedule_open_parse` then enqueues bounded background work against that ID; the package/mode JS fallback remains separately keyed. This lets TypeScript and TSX handlers coexist. Markdown native spans carry builtin provenance, while its preview remains independently opt-in package-JS SDUI.

### Behavior manifest helper

`clay:behavior` exposes `buildCodeEditingManifest(options)`, a pure helper that turns language-specific parameters (`indentSize`, `lineComment`, `electricOutdentCharacters`, `autocompleteTriggers`, optional `pairs`/`blockCommentStart`/`blockCommentEnd`) into the `editorRules` shape validated by `op_clay_modes_register_pattern`. This keeps the three packages from hand-rolling editor rules that could drift from the server validator.

The helper is implemented in `runtime/js/behavior.js` and included directly by `src/server/facades.rs`; no second inline body exists.

### Completion trigger wiring

`clay:completion` exposes `completionTriggerCharactersFromEditorRules(editorRules)`, which extracts trigger strings from `editorRules.autocompleteTriggers`. The completion provider declaration uses the returned array as `triggerCharacters`, so behavior-manifest autocomplete triggers and completion-provider selection stay aligned.

`serverListCompletionProvidersForTrigger(trigger)` queries generic provider metadata and returns structured item fields including `textFormat`. Package-record validation accepts backward-compatible plain strings or exact structured `{ label, insertText, detail?, textFormat? }` objects; it enforces per-field/result budgets, unique labels, and separate plain/snippet providers before normalizing to `CompletionItem`. `ClayJsRuntimeService` retains the last successful evaluation's inert provider snapshot. On a completion request, `static_package_completion_result` selects all active-package providers matching the trigger, applies shared priority/exclusive semantics, prefix-filters and merges items within total result budgets, and returns versioned provenance-bearing results without package JavaScript. Snippet accept then expands locally and uses existing selection state for Tab/Shift-Tab navigation.

### Classification and fallback

`ModeRegistry::classify` implements the precedence ladder: user override > exact filename > wildcard filename > extension > MIME > shebang > bounded leading-content probe > built-in `core.code` > built-in `core.text`. Package-declared patterns always win over built-in fallbacks, so `.rs` files classify to `rust` when `@clay/rust` is loaded, but fall back to `core.code` when it is not.

## Code Examples

End-user `~/.config/clay/init.js`:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
```

The deterministic Phase 18.18 manual matrix uses `cargo run -- smoke-gui --config-fixture language-packages`, then opens every valid/invalid fixture under `tests/fixtures/syntax/`. It confirms asynchronous Gruvbox-themed vocabulary decorations, static keyword completion, behavior, diagnostic repair, responsiveness, and per-package `core.code`/`core.text` fallback; `tests/manual_smoke_docs.rs` checks the document and fixture files without requiring a GUI.

The file-browser workflow smoke fixture layers documented app commands on top of the same package loads:

```js
import { bindKey } from "clay:keybindings";
import { clientCopySelection } from "clay:editor";
import { loadPackage } from "clay:packages";
import { clientOpenFolderDialog } from "clay:workspace";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");

bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
bindKey("Ctrl+P", "workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+B", "workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
```

Package load entry (excerpt from `packages/rust/dist/load.js`):

```js
import { serverRegisterSyntaxGrammar } from "clay:syntax";
import { serverRegisterModePattern } from "clay:modes";
import { serverRegisterCommand } from "clay:commands";
import { serverRegisterCompletionProvider } from "clay:completion";
import { serverRegisterComponentContribution } from "clay:ui";
import {
  modeId, rustCommands, rustEditorRules, rustStatusItem,
  supportedExtensions, supportedFileNames,
} from "./index.js";

export default async function loadRustPackage() {
  // Host-stamped package context selects manifest declarations and grants.
  await serverRegisterSyntaxGrammar({});
  await serverRegisterModePattern({
    modeId,
    displayName: "Rust",
    defaultFontRole: "monospace",
    extensions: supportedExtensions,
    fileNames: supportedFileNames,
    editorRules: rustEditorRules,
  });
  for (const command of rustCommands) {
    await serverRegisterCommand({
      commandId: command.id,
      displayName: command.userFacingName,
      routingPolicy: command.routingPolicy,
      permissions: command.permissions,
    });
  }
  await serverRegisterCompletionProvider({});
  await serverRegisterComponentContribution(rustStatusItem);
}
```

## Primitive Coverage

| Primitive | Used for | Source |
|-----------|----------|--------|
| `SyntaxGrammarContribution` | Syntax highlighting (unchanged from Phase 18.10) | `src/server/ops/syntax.rs`, `runtime/js/syntax.js` |
| `MajorModeActivation` / `serverRegisterModePattern` | Mode registration and pattern probes | `src/server/ops/modes.rs`, `runtime/js/modes.js` |
| `EditorBehaviorRules` / `buildCodeEditingManifest` | Indentation, pairs, comments, electric outdent, autocomplete triggers | `runtime/js/behavior.js`, `src/server/ops/modes.rs` |
| `CommandExecution` / `serverRegisterCommand` | `rust.toggleLineComment`, `typescript.toggleLineComment`, `javascript.toggleLineComment` | `src/server/ops/commands.rs`, `runtime/js/commands.js` |
| `CompletionTriggerAndResult` / `serverRegisterCompletionProvider` | Priority-0 static keyword/Markdown-construct providers | `src/server/completion.rs`, `src/server/ops/completion.rs`, `runtime/js/completion.js` |
| `serverListCompletionProvidersForTrigger` | Query providers by trigger character | `src/server/ops/completion.rs`, `runtime/js/completion.js` |
| `ComponentContribution` / `statusItem` | Mode status item in editor chrome | `src/server/ops/ui.rs`, `runtime/js/ui.js`, `src/shell/components.rs` |
| `loadPackage` / first-party package authority | One-line opt-in loading | `src/server/ops/packages.rs`, `src/packages/record/mod.rs` |

Permissions required: `mode-registration`, `mode-activation`, `command-registration`, `completion-provider`, `parse-document`, `render-decorations`. Not requested: filesystem, network, shell, AI, WASM authority, raw ops, native UI, client runtime, package control, workspace mutation.

Hot-path policy: parse/highlight work and completion resolution run as background, cancellable, viewport-prioritized server work. Mode registration, command registration, completion-provider metadata, and UI component declarations are validated at package load time and never run in keypress/paint/layout/scroll/pointer/text-event hot paths.

### Performance verification

`tests/performance_protocol.rs` executes each compiled native descriptor/query against the representative Rust, TypeScript, TSX, JavaScript, and Markdown fixture. It serializes the real decoration and update payloads against `DECORATION_PAYLOAD_BUDGET_BYTES` and the ordinary `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`; updates carrying the independently capped folding set use the derived `INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES` envelope. It verifies each native contribution's parse-window/timeout limits and proves delayed open parse does not delay initial editor text. `benches/first_party_language_baselines.rs` measures optimized open parse, alternating one-character incremental edits with cached-tree reuse, and decorated scroll work for the same five cases. `docs/development/performance.md` is the measured-results record and explains the local Criterion baseline/RSS method; machine timings remain advisory while payload/enqueue/cache/no-hot-path guards remain deterministic.

## Invariants and Constraints

- Active syntax grammar remains selectable independently of active major mode; loading a language package does not change the mode of already-open documents.
- Grammar artifacts are still first-party `@clay/*` only; arbitrary third-party/native grammar artifact loading is rejected.
- Package modes always win over built-in `core.code`; built-in fallbacks remain editable with no package loaded.
- Rust, TypeScript, and JavaScript declare `monospace` through generic mode metadata. No language name is inspected by server/client rendering code, and concrete font families/sizes remain user-owned.
- Completion provider `triggerCharacters` must be derivable from the major-mode behavior manifest to keep autocomplete triggers consistent.
- All UI contributions are inert declarations; packages never create Masonry widgets, mutate native layout, provide raw CSS, or run client-side JavaScript.
- Configuration is deferred: indent size, comment tokens, etc., are package-defined defaults in Phase 18.14. Future user tuning must use package-prefixed `clay.contributions.packageOptions` entries read through `configuration.setPackageOption`.

## Tests

- `tests/performance_protocol.rs::first_party_decoration_payloads_stay_within_budget_per_language`
- `tests/performance_protocol.rs::first_party_open_parse_does_not_block_initial_render_per_language`
- `benches/first_party_language_baselines.rs::{first_party_open_parse,first_party_incremental_edit,first_party_decorated_scroll}`
- `tests/syntax_grammar.rs::markdown_decoration_renders_through_tier1_native_engine`
- `tests/markdown_mode.rs::markdown_preview_sdui_panel_remains_package_js_and_unchanged`
- `tests/markdown_mode.rs::markdown_decoration_and_preview_are_independently_activatable`
- `src/server/connection/mod.rs::tests::default_init_js_load_package_powers_selected_markdown_open`
- `src/server/js_runtime/mod.rs::rust_package_expansion_registers_mode_command_completion_and_status`
- `src/server/js_runtime/mod.rs::typescript_package_expansion_registers_mode_command_completion_and_status`
- `src/server/js_runtime/mod.rs::javascript_package_expansion_registers_mode_command_completion_and_status`
- `src/server/js_runtime/mod.rs::language_packages_classify_with_core_fallbacks_and_no_conflicts`
- `src/server/js_runtime/mod.rs::language_package_classification_is_deterministic_across_load_orders`
- `src/server/js_runtime/mod.rs::language_package_rejects_unauthorized_completion_provider`
- `src/server/js_runtime/mod.rs::language_package_completion_trigger_metadata_is_queryable`
- `tests/completion_provider.rs::each_language_registers_a_base_keyword_completion_provider`
- `tests/completion_provider.rs::base_keyword_provider_merges_with_future_providers_at_documented_priority`
- `tests/completion_provider.rs::completion_registration_has_no_per_language_rust_branch`
- `src/server/js_runtime/mod.rs::build_code_editing_manifest_produces_valid_editor_rules`
- `src/server/js_runtime/mod.rs::language_packages_config_fixture_loads_and_registers_all_contributions`
- Package reference documentation uses generic manifest/API/security validators in `tests/package_loading_docs.rs`; executable package/runtime tests remain authoritative for behavior.
- `tests/manual_smoke_docs.rs::phase18_18_manual_smoke_documents_first_party_language_matrix`
- `tests/manual_smoke_docs.rs::first_party_syntax_fixtures_exist_per_language`
- `tests/manual_smoke_docs.rs::end_to_end_file_browser_workflow_smoke_has_runnable_fixture_contract`
- `src/server/js_runtime/mod.rs::file_browser_workflow_config_fixture_loads_packages_and_bindings`
- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.

Run the relevant suites:

```bash
cargo test --lib packages
cargo test --test protocol package_loading_docs::
cargo test --test protocol manual_smoke_docs::
cargo test --test protocol primitives_docs::
```

## Related

- [First-Party LSP Bridge Packages](first-party-lsp-bridge-packages.md)
- [Phase 18.14 First-Party Rust, TypeScript, and JavaScript Language Package Expansion Primitive Review](phase18.14-language-package-expansion-primitive-review.md)
- [Mode Registry](mode-registry.md)
- [Command Registry](command-registry.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Behavior Manifests](behavior-manifests.md)
- [Package Loading](package-loading.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md) — Phase 18.19 snippet accept, exclusive claim, and serverDisableCompletion
- `docs/reference/packages/rust.md`
- `docs/reference/packages/typescript.md`
- `docs/reference/packages/javascript.md`
- `docs/reference/packages/creating-packages.md`
- `docs/development/launch-and-gui-smoke.md`
- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)
- [Workspace Discovery and File Browser](workspace-file-browser.md)
