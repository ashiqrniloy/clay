# Command Registry

## Source

- `src/packages/commands.rs`
- `src/server/command_execution.rs`
- `src/server/control_center.rs`
- `src/server/locks.rs`
- `src/server/ops/mod.rs`
- `tests/package_primitive_gate.rs`

## Overview

The command registry is the Phase 16.5 server-side primitive gate for package-owned command metadata and behavior-manifest contributions. It validates package-prefixed command declarations, load/activation-time key routing data, and inert text-transform metadata. Phase 18.8 adds the server-owned command execution foundation in `src/server/command_execution.rs`: execution requests reuse registered command metadata or the small built-in server command table, validate provenance/permissions/arguments/targets/routing, and currently return a typed accepted result for downstream SDUI, keybinding, and transient-menu routing work. Phase 18.4 package input declarations and component-scoped action metadata reuse this registry boundary: input/action records may reference only already-registered package command IDs, and declaring input metadata does not create command execution authority. Phase 18.9 adds read-only mode-discovery built-in server commands (`modes.listActiveModes`/`modes.explainActiveMode`) resolved through a dedicated `CommandExecutor::execute_discovery` path that reads installed `ModeRegistry` state and carries no execution/document/workspace authority.

## Responsibilities

- Register package-owned command declarations with package name, version, prefix, routing policy, user-facing label, custom properties, key binding metadata, permissions, and provenance.
- Validate behavior-manifest contributions by composing package declarations into the existing inert `BehaviorManifest` schema and reusing `validate_manifest` for duplicate command and ambiguous key binding checks.
- Reject command registration without `command-registration`, invalid or reserved command IDs, undeclared command permissions, client-first package command authority, executable text transform fields, duplicate command IDs, and ambiguous key bindings.
- Provide the registered-command source of truth used by Phase 18.4 `PackageInputContribution` and layout/action defaults so component-scoped actions remain inert command intents rather than package callbacks.
- Validate `CommandExecutionRequest` values against the registry before any command side effect can be wired: command ID, server-owned routing policy, package provenance, expected permissions, bounded JSON-object arguments, and document/workspace target.
- Provide the command snapshot consumed by the built-in Control Center (`src/server/control_center.rs`). The Control Center lists executable commands, filters them by query, and routes selected commands through the same `CommandExecutor`, keeping command-palette behavior package-aware without a bespoke dispatcher.
- Phase 18.12 adds four built-in file-browser commands (`workspace.openFile`, `workspace.revealInTree`, `workspace.openFuzzyFile`, `workspace.toggleFileBrowser`) and a server-side workspace execution path (`CommandExecutor::execute_workspace`). Open commands resolve through `WorkspaceState::open_existing_file` for in-root paths and `WorkspaceState::open_selected_file` for out-of-root absolute paths, enforcing the selected-file single-file grant flow. Reveal is accepted as an inert UI state transition; toggle is validated here and applied by the bound connection to its per-tab workspace-pane visibility state. Save/save-as/rename/delete actions remain unregistered and deferred per roadmap.

It does not execute package JavaScript, install command handlers, grant filesystem/workspace/AI/shell/network authority, or add any synchronous package work to the keypress hot path. The Phase 18.8 executor is intentionally a foundation: it accepts validated server-owned commands and rejects unsafe shapes; later routing tasks attach SDUI/keybinding/menu sources to this same path.

## How It Works

`CommandRegistry::register_command` accepts a validated `ClayPackageManifest` and a `PackageCommandDeclaration`. Registration verifies that the package declared `command-registration`, that declaration provenance matches the manifest, and that the command ID uses the package `apiPrefix` or `apiPrefix.*` namespace. Package commands cannot declare `ClientFirstPredictable` or `ClientFirstRequiresAck`, because those routing policies imply built-in Rust client edit authority rather than package handler authority.

`CommandRegistry::validate_behavior_contribution` accepts `PackageBehaviorContribution` metadata for mode/package loading. It validates provenance and text transforms first, then builds a candidate `BehaviorManifest` by combining the default text manifest, package command declarations, contributed keymaps, editor rules, and already registered commands. The candidate goes through `src/behavior/manifest.rs::validate_manifest`, so existing manifest invariants continue to reject duplicate command IDs, unknown command targets, ambiguous key bindings, invalid editor rules, and authority/routing mismatches.

`PackageTextTransformDeclaration` is intentionally metadata-only. Its `kind` identifies a Rust-known transform category, while `javascript_callback` and `code` are forbidden fields used by the gate to reject executable payload shapes in fixtures before Phase 17 package loading expands the source of these declarations.

`CommandExecutor::execute` accepts a `CommandRegistry` plus `CommandExecutionRequest`. One `builtin_commands!` declaration in `src/server/command_execution.rs` owns every built-in ID, display name, and routing category. It emits named ID constants, the ordered discovery slice, and the metadata table used by lookup and category predicates; command strings no longer repeat across ID arrays, lookup matches, display-name matches, and routing matches. Security-sensitive routing remains an independently validated field on each row. The executor looks up the registered command, falls back to this built-in table for Clay-owned IDs such as `controlCenter.open`, rejects client-first/client-UI routing, checks optional provenance against the registered package name/version/prefix, requires every `expected_permissions` entry to appear on the registered command, limits arguments to `null` or a JSON object under 4 KiB, rejects document target `0`, and requires `workspace-mutation` for workspace targets. `ClayOpState::execute_command` exposes the same path inside the server runtime op state. `src/server/connection/mod.rs` also normalizes inbound `ClientMessage::SduiAction` and `ClientMessage::CommandIntent` values into `CommandExecutionRequest`, so SDUI actions, package UI action regions, behavior-manifest server-first keybindings, transient-menu selections, and Control Center selections share the same executor instead of parallel dispatchers. Phase 24.2 `RuntimeGenerationStore::command_catalogue_snapshot` builds the generation-stamped `CommandCatalogue` (built-ins, declared `shell.client*` entries, trusted and third-party registry snapshots, deterministic sort, duplicate-ID fail-closed), `ControlCenter::open_catalogue` populates the generic `TransientMenuSession` from it, and `ServerMenuSession::activate` produces a typed activation (server/package `CommandExecutionRequest` or the narrow `ShellClientCommand` bridge) that the connection routes through the same shared dispatcher. The catalogue merge uses `CommandRegistry::snapshot()` (inert clone of registered metadata) harvested across both runtime trust domains (`ClayJsRuntimeService::command_registry_snapshots`), and dispatch rebuilds a live aggregated registry via `CommandRegistry::from_snapshots([trusted, third_party])` (later source wins) so package commands selected from the menu execute instead of failing as `UnknownCommand`; built-ins are omitted from the aggregated registry because the executor falls back to the built-in table. Sessions are stamped with the runtime generation ID and `activate` rejects stale generations (`StaleRuntimeGeneration`); the runtime replacement broadcast cancels open menus. Query ranking across menus uses the shared bounded fuzzy subsequence scorer (`src/shell/fuzzy.rs`, see [Fuzzy Matching](fuzzy-matching.md)), never substring filters.

### Trusted package loading and key-routing records

`op_clay_packages_load_package_by_specifier` applies each trusted package's
validated `package.json` record before importing its execute-only `loadEntry`.
`apply_package_record_contributions` registers package commands and converts
`clay.contributions.keyRouting` through the shared key-sequence parser into
`KeyBindingRule` metadata, preserving parsed modifiers in the command
catalogue. Execute-only entries must not re-register commands already supplied
by the record; `@clay/settings` registers only its panel at load time.

This split keeps package metadata single-source while leaving package JavaScript
for runtime work such as parse handlers. Invalid package chords or routing
policies fail the load rather than creating raw-character or accepted no-op
routes. The behavior is covered by the live package catalogue and runtime
reload tests in `src/server/mod.rs`.

### Phase 18.9 mode discovery commands

Phase 18.9 registers two read-only built-in server commands for Control Center and package diagnostics: `modes.listActiveModes` and `modes.explainActiveMode`. They are declared in the built-in server command table (`builtin_server_command`/`builtin_server_command_ids`) with `RoutingPolicy::ServerFirst` and an empty permissions list, in the same list as `controlCenter.open`, `workspace.refresh`, `document.focus_active`, and `document.open_recent`. They have **no op wrapper and no Clay JS facade** — the built-in command ID is the user-facing surface, so they are server-first commands rather than Clay JS APIs (mirroring `controlCenter.open`).

Because discovery commands resolve payload data rather than execute side effects, they bypass the general `CommandExecutor::execute` path and route through a dedicated resolver `CommandExecutor::execute_discovery(mode_registry, request)`. The resolver still runs the shared `validate()` helper (command ID, arguments, provenance, permissions, target) but then branches on the command ID:

- `modes.listActiveModes` -> calls `ModeRegistry::list_active_modes()` returning `Vec<ActiveModeSummary>`, one entry per document with an active major mode: `{ document_id, mode_id, package_name, api_prefix, provenance (CoreBuiltIn | Package), classification_source (ModePatternKind) }`.
- `modes.explainActiveMode` -> extracts a `documentId` argument (rejects missing/non-`u64` with `InvalidArguments`) and calls `ModeRegistry::explain_active_mode(document_id)` returning `Option<ModeExplanation>`: `{ active_mode, display_name, package_name, package_version, api_prefix, provenance, classification_source, fallback_used, why }` where `why` is a human-readable rationale derived from the stored `matched_by` kind and `is_builtin` flag.

The resolved payload is returned as `CommandExecutionStatus::Discovery(DiscoveryResult::ActiveModes(_) | ModeExplanation(_))`. `ClayRuntimeOpState::execute_command` (in `src/server/ops/mod.rs`) routes discovery command IDs to `execute_discovery` with `ModeRegistry` access rather than plain `execute`.

`list_active_modes` and `explain_active_mode` are crate-internal (`pub(crate)`) resolver methods on `ModeRegistry` used only by `execute_discovery` within the same crate; they are read-only entrypoints over installed registry state and are **not** part of the public Rust embedder API or the Clay JS surface. The built-in `modes.*` commands are the user-facing surface. Discovery commands carry no execution/document/workspace authority: they read installed `ModeRegistry` state, perform no filesystem scan, package evaluation, or parse work, and never mutate document/workspace state. This satisfies the deny-by-default model — adding a command to the built-in list grants no package authority.

### Phase 18.12 workspace file-browser commands

Phase 18.12 registers four built-in server-first commands for the Clay-owned file browser UI: `workspace.openFile`, `workspace.revealInTree`, `workspace.openFuzzyFile`, and `workspace.toggleFileBrowser`. They are declared in the built-in server command table alongside existing commands and validate through the same `CommandExecutor` boundary, but open commands additionally resolve against `WorkspaceState` root/grant APIs through `CommandExecutor::execute_workspace`.

`CommandExecutor::execute_workspace(registry, workspace, request)` first validates the request (routing policy, provenance, permissions, arguments, target) and then branches on the command ID:

- `workspace.openFile` and `workspace.openFuzzyFile` extract bounded arguments (`workspaceRootId` + `relativePath` for in-root paths, or `absolutePath` for out-of-root selected-file fallback). In-root paths call `WorkspaceState::open_existing_file`; out-of-root paths call `WorkspaceState::open_selected_file`, which creates a single-file grant and enforces the same size/type/UTF-8 validation as the client file-dialog flow. The result is returned as `CommandExecutionStatus::Workspace(WorkspaceActionResult::Opened(snapshot))` where `snapshot` contains the opened document's metadata and text.
- `workspace.revealInTree` accepts a `documentId` argument, validates that the document is open through `WorkspaceState::document_metadata`, and returns `WorkspaceActionResult::Revealed`; the next SDUI snapshot can use this to focus the tree node.
- `workspace.toggleFileBrowser` returns `WorkspaceActionResult::Toggled`; the bound connection applies the per-tab visibility flag and publishes either the bounded tree or an editor-only SDUI snapshot.

`ClayOpState::execute_command` (in `src/server/ops/mod.rs`) routes workspace command IDs to `execute_workspace` and is now exposed through the `op_clay_commands_execute_command` op. `runtime/js/commands.js` provides `serverExecuteCommand`, `serverOpenFile`, and `serverRevealInTree` facades so server-side configuration or first-party JS can request opens through the same validated boundary. The connection handler (`src/server/connection/mod.rs`) also routes SDUI action intents and `ClientMessage::CommandIntent` values for workspace commands through `execute_command_intent`, sending a `DocumentOpened` server message when a file is opened.

These commands carry bounded workspace authority only: they cannot access paths outside known roots or selected-file grants, cannot add roots or markers, and cannot perform save/save-as/rename/delete operations. Save-related destructive operations remain deferred per roadmap.

### Phase 18.20 language-intelligence commands

Four built-in `UiReactivePriority` commands—`language.hover`, `language.goToDefinition`, `language.codeActions`, and `language.signatureHelp`—are discoverable with empty default key bindings. Client routing captures current document/version/cursor metadata and emits a `LanguageIntelligenceRequest` rather than executing through a package callback. Definition and code-action menu selections return to the shared command path: workspace definitions reuse `workspace.openFile`; command-backed actions must already be registered and validated by `CommandExecution`; edit previews never mutate text in Phase 18.20.

### Phase 19 explicit runtime reload command

`runtime.reloadConfiguration` is a Clay-owned global command named **Reload Configuration and Packages**. Its built-in metadata declares `ServerFirstWithLock { lock_scope: Behavior }`, a global `Ctrl+Shift+R` default binding, no permissions, and no package provenance. `ControlCenter::open` discovers it from the existing built-in command table and displays that binding. User configuration can override or remove it through `bindKey`/`unbindKey`, producing the same inert behavior-manifest route as other server-first commands.

Connection command intents and validated SDUI/menu actions call `IpcServer::execute_reload_command`. That method reruns shared `CommandExecutor` validation, rejects a concurrent attempt with `CommandExecutionRule::ReloadInProgress`, evaluates a fresh candidate outside scoped locks, and acquires `ScopedLockTarget::Behavior` only for compare-and-swap commit. RAII drops both attempt and behavior guards on every return/unwind path. Package-side `serverExecuteCommand` rejects this ID with `UnauthorizedTarget`; package JavaScript can declare UI/key routing but cannot directly invoke reload authority. No reload-specific client message or dispatcher exists.

`src/server/locks.rs` supplies immediate, non-waiting range/document/behavior/workspace lock acquisition. Workspace locks conflict with every scope; behavior locks conflict with behavior/workspace; document locks conflict with ranges/documents for the same document; range locks conflict only on overlapping ranges in the same document. Range overlap reuses the helper used by `DocumentState` region-lock validation.

## Code Examples

```rust
let manifest = validate_manifest_value(&package_json)?;
let mut registry = CommandRegistry::new();
registry.register_command(&manifest, PackageCommandDeclaration {
    package_name: "@clay/markdown".into(),
    package_version: "0.1.0".into(),
    api_prefix: "markdown".into(),
    command_id: "markdown.togglePreview".into(),
    display_name: "Toggle Markdown Preview".into(),
    routing_policy: RoutingPolicy::ServerFirst,
    key_bindings: vec![],
    custom_properties: BTreeMap::new(),
    permissions: vec![],
})?;
```

## Invariants and Constraints

- Built-in command IDs have one Rust declaration; table tests reject duplicates and verify routing/display metadata. Cross-language facade/docs copies remain independently validated contracts rather than selecting security policy.
- Command IDs are package-owned and unique among enabled package commands.
- Command registration does not grant execution authority; command-specific permissions must already be present in the package manifest and are rechecked by command execution requests.
- Behavior contributions are load/activation-time validation work and only return inert manifest data for the client.
- Client-first local paint behavior remains Rust-known manifest behavior; package commands cannot become arbitrary client-first handlers.
- Command execution requests are server-first validation work and must not run package JavaScript, raw ops, filesystem/network/shell/AI/WASM work, or synchronous client hot-path work.

## Tests

- `tests/package_primitive_gate.rs`: validates duplicate command rejection, package-aware key binding ambiguity rejection, successful inert behavior contribution validation, executable text-transform rejection, client-first and client-ui routing rejection, provenance, permissions, and budget references.
- `src/server/command_execution.rs` unit tests: validate unique single-table built-in IDs/metadata, successful built-in and registered command execution, unknown command rejection, provenance mismatch rejection, undeclared expected permission rejection, client-first route rejection, malformed/oversize arguments, and unauthorized workspace targets. Phase 18.12 adds `workspace_commands` tests covering open in-root, open out-of-root via selected-file grant, reveal, toggle, missing-argument rejection, and unregistered save-related commands.
- `tests/command_execution.rs`: integration/security tests for reload command behavior-lock metadata/discovery/shared validation, unknown command rejection, client-first/client-ui routing rejection, provenance mismatch, undeclared permission, malformed/oversize arguments, invalid document target, workspace-mutation target requirement, and duplicate command ID rejection. It also covers Phase 18.9 mode-discovery commands: `modes.explainActiveMode` reports `core.code` built-in fallback rationale when no language package matched (and `core.text` universal fallback), `modes.listActiveModes` reports package vs `core` built-in provenance with classification source, unknown documents return `None`, discovery commands are reachable from the Control Center built-in command listing, and discovery commands reject no-authority violations (invalid arguments, unauthorized workspace target, non-discovery/bogus command IDs). Phase 28.7 additionally proves first-party comment/list/heading aliases are not accepted as metadata-only server commands; they must route to the native editor engine.
- `src/server/connection/mod.rs` unit tests: validate that SDUI/package UI actions and keybinding/menu command intents share command execution and reject unregistered package action targets.
- `src/client/mod.rs` unit tests: validate that server-first keybindings enqueue bounded `ClientMessage::CommandIntent` values and use `try_send` backpressure.
- `src/editor/surface/mod.rs` unit tests: validate that ordinary character typing updates local text synchronously while server-first keybindings produce only an intent, preserving the no-block-during-typing invariant.

Run focused coverage with:

```text
cargo test --test security package_primitive_gate::
cargo test --test runtime command_execution::
cargo test --lib command_execution
cargo test --lib locks::tests
cargo test --lib runtime_generation_tests
cargo test --lib editor
```

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Mode Registry](mode-registry.md)
- [Package Input, State, and Configuration Integration](package-input-state-configuration.md)
- [Language Intelligence](language-intelligence.md)
- The Phase 22.4 bindable tab command surface (24 `client_ui` IDs, default
  Global chords, numbering/wraparound/no-op policies, deny-by-default
  numbered bounds 1..=9) is documented in [Tabs and Independent Client
  Views](tabs-and-clients.md) (Keyboard Management section).
- `docs/reference/primitives/registry.md#CommandDeclaration`
- `docs/reference/primitives/registry.md#KeyRoutingOverride`
- `docs/reference/primitives/registry.md#TextTransform`
