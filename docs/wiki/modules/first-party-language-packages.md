# First-Party Rust, TypeScript, and JavaScript Language Packages

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
- `runtime/js/behavior.ts`
- `runtime/js/completion.ts`
- `runtime/js/modes.ts`
- `runtime/js/commands.ts`
- `runtime/js/ui.ts`
- `src/server/ops/modes.rs`
- `src/server/ops/completion.rs`
- `src/server/ops/commands.rs`
- `src/server/ops/ui.rs`
- `src/server/ops/behavior.rs`
- `src/server/ops/packages.rs`
- `src/packages/record.rs`
- `src/packages/permissions.rs`
- `src/packages/modes.rs`
- `src/server/completion.rs`
- `src/server/js_runtime.rs`
- `docs/reference/packages/rust.md`
- `docs/reference/packages/typescript.md`
- `docs/reference/packages/javascript.md`
- `docs/reference/clay-js-api/behavior/build-code-editing-manifest.md`
- `docs/reference/clay-js-api/completion/completion-trigger-characters-from-editor-rules.md`
- `docs/reference/clay-js-api/completion/server-list-completion-providers-for-trigger.md`
- `docs/development/launch-and-gui-smoke.md`
- `tests/fixtures/configuration/language-packages/init.js`
- `tests/fixtures/configuration/language-packages/workspace/main.rs`
- `tests/fixtures/configuration/language-packages/workspace/main.ts`
- `tests/fixtures/configuration/language-packages/workspace/main.js`
- `tests/fixtures/configuration/file-browser-workflow/init.js`
- `tests/fixtures/configuration/file-browser-workflow/workspace/main.rs`
- `tests/fixtures/configuration/file-browser-workflow/workspace/main.ts`
- `tests/fixtures/configuration/file-browser-workflow/workspace/main.js`
- `tests/package_loading_docs.rs`
- `tests/manual_smoke_docs.rs`
- `tests/primitives_docs.rs`
- `src/server/js_runtime.rs` (integration tests)

## Overview

Phase 18.14 expands the first-party `@clay/rust`, `@clay/typescript`, and `@clay/javascript` packages from grammar-only syntax-highlighting contributions into full language packages. Each package now declares a major mode, a behavior manifest, a line-comment command, a metadata-only completion provider, and a status-item UI contribution. All of this is built on generic Clay primitives already provided by the runtime and server ops; no language-specific Rust branch was added for Rust, TypeScript, or JavaScript editing behavior.

## Responsibilities

- Register a package-owned major mode with generic file-extension/file-name probes and semantic `defaultFontRole: "monospace"`.
- Publish a validated `EditorBehaviorRules` manifest (indentation, delimiter pairs, comment continuation, electric outdent, autocomplete triggers).
- Register one server-first command (`<lang>.toggleLineComment`).
- Register one metadata-only completion provider (`<lang>.keywords`) with trigger characters derived from the behavior manifest.
- Register one inert `statusItem` UI contribution (`<lang>.status.mode`).
- Keep syntax-grammar registration unchanged from Phase 18.10.
- Remain explicit opt-in via `loadPackage("@clay/*")`; do not auto-activate or shadow built-in `core.code`/`core.text` fallbacks.

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
- `clay.apiDependencies`: the Clay JS APIs the package calls (e.g., `clay.modes.serverRegisterModePattern`, `clay.commands.serverRegisterCommand`, `clay.completion.serverRegisterCompletionProvider`, `clay.ui.serverRegisterComponentContribution`, `clay.behavior.buildCodeEditingManifest`, `clay.completion.completionTriggerCharactersFromEditorRules`).
- `clay.contributions`: `modePatterns`, `commands`, `completionProviders`, `ui.components`, and the unchanged `syntaxGrammars` entry.

Manifest payloads are minified to stay under the behavior-manifest payload budget (`BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` = 2048).

### Load entry

`dist/load.js` is the default `loadEntry` evaluated by `loadPackage`. It imports the generic Clay facades and the package's own manifest builder, then:

1. Registers the Phase 18.10 syntax grammar via `serverRegisterSyntaxGrammar`.
2. Registers the major-mode pattern via `serverRegisterModePattern`, passing `editorRules` built by `buildCodeEditingManifest`.
3. Registers the line-comment command via `serverRegisterCommand`.
4. Registers the completion provider via `serverRegisterCompletionProvider`, deriving `triggerCharacters` from `editorRules` using `completionTriggerCharactersFromEditorRules`.
5. Registers the status item via `serverRegisterComponentContribution` with `kind: "statusItem"`.

No document is activated at load time; activation happens later when a document is opened and the editor/classification path activates the classified mode.

### Behavior manifest helper

`clay:behavior` exposes `buildCodeEditingManifest(options)`, a pure helper that turns language-specific parameters (`indentSize`, `lineComment`, `electricOutdentCharacters`, `autocompleteTriggers`, optional `pairs`/`blockCommentStart`/`blockCommentEnd`) into the `editorRules` shape validated by `op_clay_modes_register_pattern`. This keeps the three packages from hand-rolling editor rules that could drift from the server validator.

The helper is implemented in `runtime/js/behavior.ts` and mirrored in the hardcoded `CLAY_FACADE_BEHAVIOR` string in `src/server/js_runtime.rs` because Clay's server runtime currently injects facade source as inline strings rather than compiling `runtime/js/*.ts` dynamically.

### Completion trigger wiring

`clay:completion` exposes `completionTriggerCharactersFromEditorRules(editorRules)`, which extracts trigger strings from `editorRules.autocompleteTriggers`. The completion provider declaration uses the returned array as `triggerCharacters`, so behavior-manifest autocomplete triggers and completion-provider selection stay aligned.

Phase 18.14 also adds `serverListCompletionProvidersForTrigger(trigger)` to query the generic completion framework for providers matching a trigger character. The server-side `CompletionProviderRegistry.providers_for_trigger_character` filters by trigger and sorts by priority descending, then id ascending.

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
bindKey("Ctrl+P", "clay.workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+B", "clay.workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
```

Package load entry (excerpt from `packages/rust/dist/load.js`):

```js
import { loadPackage } from "clay:packages";
import { serverRegisterSyntaxGrammar } from "clay:syntax";
import { serverRegisterModePattern } from "clay:modes";
import { serverRegisterCommand } from "clay:commands";
import { serverRegisterCompletionProvider, completionTriggerCharactersFromEditorRules } from "clay:completion";
import { serverRegisterComponentContribution } from "clay:ui";
import { buildCodeEditingManifest } from "clay:behavior";
import { rustPackageManifest, rustGrammarContract, rustCommands, rustCompletionProvider, rustStatusItem } from "./index.js";

export default async function loadRustPackage() {
  const packageManifest = rustPackageManifest();
  await serverRegisterSyntaxGrammar(rustGrammarContract(packageManifest));
  await serverRegisterModePattern(packageManifest, {
    modeId: "rust",
    displayName: "Rust",
    extensions: ["rs"],
    fileNames: ["Cargo.toml"],
    editorRules: buildCodeEditingManifest({
      indentSize: 4,
      lineComment: "//",
      electricOutdentCharacters: ["}"],
      autocompleteTriggers: [".", "::"],
    }),
  });
  await serverRegisterCommand(packageManifest, rustCommands[0]);
  await serverRegisterCompletionProvider({
    packageManifest,
    ...rustCompletionProvider,
    triggerCharacters: completionTriggerCharactersFromEditorRules(
      buildCodeEditingManifest({
        indentSize: 4,
        lineComment: "//",
        electricOutdentCharacters: ["}"],
        autocompleteTriggers: [".", "::"],
      })
    ),
  });
  await serverRegisterComponentContribution(packageManifest, rustStatusItem);
}
```

## Primitive Coverage

| Primitive | Used for | Source |
|-----------|----------|--------|
| `SyntaxGrammarContribution` | Syntax highlighting (unchanged from Phase 18.10) | `src/server/ops/syntax.rs`, `runtime/js/syntax.ts` |
| `MajorModeActivation` / `serverRegisterModePattern` | Mode registration and pattern probes | `src/server/ops/modes.rs`, `runtime/js/modes.ts` |
| `EditorBehaviorRules` / `buildCodeEditingManifest` | Indentation, pairs, comments, electric outdent, autocomplete triggers | `runtime/js/behavior.ts`, `src/server/ops/modes.rs` |
| `CommandExecution` / `serverRegisterCommand` | `rust.toggleLineComment`, `typescript.toggleLineComment`, `javascript.toggleLineComment` | `src/server/ops/commands.rs`, `runtime/js/commands.ts` |
| `CompletionTriggerAndResult` / `serverRegisterCompletionProvider` | Metadata-only keyword providers | `src/server/completion.rs`, `src/server/ops/completion.rs`, `runtime/js/completion.ts` |
| `serverListCompletionProvidersForTrigger` | Query providers by trigger character | `src/server/ops/completion.rs`, `runtime/js/completion.ts` |
| `ComponentContribution` / `statusItem` | Mode status item in editor chrome | `src/server/ops/ui.rs`, `runtime/js/ui.ts`, `src/shell/components.rs` |
| `loadPackage` / first-party package authority | One-line opt-in loading | `src/server/ops/packages.rs`, `src/packages/record.rs` |

Permissions required: `mode-registration`, `mode-activation`, `command-registration`, `completion-provider`, `parse-document`, `render-decorations`. Not requested: filesystem, network, shell, AI, WASM authority, raw ops, native UI, client runtime, package control, workspace mutation.

Hot-path policy: parse/highlight work and completion resolution run as background, cancellable, viewport-prioritized server work. Mode registration, command registration, completion-provider metadata, and UI component declarations are validated at package load time and never run in keypress/paint/layout/scroll/pointer/text-event hot paths.

## Invariants and Constraints

- Active syntax grammar remains selectable independently of active major mode; loading a language package does not change the mode of already-open documents.
- Grammar artifacts are still first-party `@clay/*` only; arbitrary third-party/native grammar artifact loading is rejected.
- Package modes always win over built-in `core.code`; built-in fallbacks remain editable with no package loaded.
- Rust, TypeScript, and JavaScript declare `monospace` through generic mode metadata. No language name is inspected by server/client rendering code, and concrete font families/sizes remain user-owned.
- Completion provider `triggerCharacters` must be derivable from the major-mode behavior manifest to keep autocomplete triggers consistent.
- All UI contributions are inert declarations; packages never create Masonry widgets, mutate native layout, provide raw CSS, or run client-side JavaScript.
- Configuration is deferred: indent size, comment tokens, etc., are package-defined defaults in Phase 18.14. Future user tuning must use package-prefixed `clay.contributions.packageOptions` entries read through `clay.configuration.setPackageOption`.

## Tests

- `src/server/js_runtime.rs::rust_package_expansion_registers_mode_command_completion_and_status`
- `src/server/js_runtime.rs::typescript_package_expansion_registers_mode_command_completion_and_status`
- `src/server/js_runtime.rs::javascript_package_expansion_registers_mode_command_completion_and_status`
- `src/server/js_runtime.rs::language_packages_classify_with_core_fallbacks_and_no_conflicts`
- `src/server/js_runtime.rs::language_package_classification_is_deterministic_across_load_orders`
- `src/server/js_runtime.rs::language_package_rejects_unauthorized_completion_provider`
- `src/server/js_runtime.rs::language_package_completion_trigger_metadata_is_queryable`
- `src/server/js_runtime.rs::build_code_editing_manifest_produces_valid_editor_rules`
- `src/server/js_runtime.rs::language_packages_config_fixture_loads_and_registers_all_contributions`
- `tests/package_loading_docs.rs::phase18_14_language_package_default_init_js_loading_is_documented`
- `tests/package_loading_docs.rs::phase18_14_ui_layout_authoring_contract_is_documented`
- `tests/package_loading_docs.rs::phase18_14_behavior_manifest_helper_is_documented`
- `tests/package_loading_docs.rs::phase18_14_configuration_contract_defers_user_tunable_keys`
- `tests/manual_smoke_docs.rs::phase18_14_language_package_expansion_smoke_has_runnable_fixture_contract`
- `tests/manual_smoke_docs.rs::end_to_end_file_browser_workflow_smoke_has_runnable_fixture_contract`
- `src/server/js_runtime.rs::file_browser_workflow_config_fixture_loads_packages_and_bindings`
- `tests/primitives_docs.rs::phase18_14_language_package_expansion_primitive_review_records_inventory_and_gaps`

Run the relevant suites:

```bash
cargo test --lib packages
CARGO_TARGET_DIR=target/pi-verify cargo test --test package_loading_docs
CARGO_TARGET_DIR=target/pi-verify cargo test --test manual_smoke_docs
CARGO_TARGET_DIR=target/pi-verify cargo test --test primitives_docs
```

## Related

- [Phase 18.14 First-Party Rust, TypeScript, and JavaScript Language Package Expansion Primitive Review](phase18.14-language-package-expansion-primitive-review.md)
- [Mode Registry](mode-registry.md)
- [Command Registry](command-registry.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Behavior Manifests](behavior-manifests.md)
- [Package Loading](package-loading.md)
- [Package Primitive Gate](package-primitive-gate.md)
- `docs/reference/packages/rust.md`
- `docs/reference/packages/typescript.md`
- `docs/reference/packages/javascript.md`
- `docs/reference/packages/creating-packages.md`
- `docs/development/launch-and-gui-smoke.md`
- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)
- [Workspace Discovery and File Browser](workspace-file-browser.md)
