# Phase 18.11 Completion Provider Framework Primitive Review

## Source

- `roadmap.md`
- `plans/039-Phase18.11-Completion-Provider-Framework.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/backlog.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/reference/primitives/package-security.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/behavior-manifests.md`
- `docs/wiki/modules/transient-menu-session.md`
- `docs/wiki/modules/control-center.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/mode-registry.md`
- `docs/wiki/modules/syntax-grammar-registry.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/package-primitive-gate.md`
- `docs/wiki/modules/phase18.8-transient-menu-command-execution-primitive-review.md`
- `docs/wiki/modules/phase18.9-generic-text-code-modes-primitive-review.md`
- `docs/wiki/modules/phase18.10-tree-sitter-grammar-primitive-review.md`
- `src/protocol/mod.rs`
- `src/protocol/completion.rs`
- `src/client/behavior.rs`
- `src/client/mod.rs`
- `src/behavior/manifest.rs`
- `src/perf/budgets.rs`
- `src/shell/transient_menu.rs`
- `src/shell/package_ui.rs`
- `src/shell/mod.rs`
- `src/server/parse_coordinator.rs`
- `src/server/completion.rs`
- `src/server/ops/completion.rs`
- `src/server/ops/mod.rs`
- `src/server/ops/keybindings.rs`
- `src/server/js_runtime.rs`
- `src/server/syntax.rs`
- `src/server/control_center.rs`
- `src/server/document.rs`
- `src/packages/permissions.rs`
- `src/packages/service.rs`
- `src/packages/record.rs`
- `src/masonry_sdui.rs`
- `src/masonry_editor.rs`
- `src/editor/surface.rs`
- `src/editor.rs`
- `runtime/js/completion.js`
- `runtime/js/mod.ts`
- `tests/primitives_docs.rs`

## Overview

Phase 18.11 promotes `CompletionTriggerAndResult` from a deferred registry row into an implemented, reusable completion provider framework. This primitive review records the existing editor/package/primitive inventory and the generic completion gaps that must exist before any provider code is written.

The target primitive composes with existing Phase 18.8 `TransientMenuSession`/`CommandExecution`, Phase 18.9 `core.text`/`core.code` fallback modes and `AutocompleteTrigger` manifest data, and Phase 18.10 package-provenance/async-background patterns instead of replacing them. Completion is split into three concerns: trigger detection (manifest data, local), result computation (server-side `UiReactivePriority`, cancellable), and display/acceptance (reused `TransientMenuSession`).

The first implementation ships one minimal built-in buffer-word provider. LSP, AI, workspace-index, snippet-expansion, shell/tool, and network-backed providers remain later add-ons that require a future approved decision log and explicit permissions.

## Existing Primitive Inventory

### Behavior manifests and autocomplete trigger metadata

- `src/protocol/mod.rs` defines `EditorBehaviorRules.autocomplete_triggers: Vec<AutocompleteTrigger>`, where `AutocompleteTrigger { trigger, routing_policy }` is inert manifest data.
- `Behavior::default_code()` already ships `AutocompleteTrigger { trigger: ".", routing_policy: RoutingPolicy::UiReactivePriority }`, so trigger classification is already declared as inert `UiReactivePriority` manifest metadata, not executable package code.
- `src/behavior/manifest.rs` validates behavior-manifest payloads against `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` and rejects raw callbacks, raw ops, client-side JavaScript, native handles, and renderer hooks.
- `docs/wiki/modules/behavior-manifests.md` documents that autocomplete triggers are inert `UiReactivePriority` declarations so future completion UI can observe triggers without extension code or document mutation during trigger classification.

### Client behavior routing and local edit path

- `src/client/behavior.rs::ClientBehaviorState::autocomplete_trigger_for_key` already detects autocomplete triggers from installed manifest data and returns an `AutocompleteTriggerRoute`.
- `route_unbound_key` currently stores the result in a local `_autocomplete_trigger` binding and still returns the normal client text-insertion route, so completion trigger routing is detected but not yet wired to a request/result lifecycle.
- The client edit path is `ClientFirstPredictable`: ordinary typing edits locally first and must never block on synchronous IPC, provider execution, package JavaScript, or a filesystem scan before local paint.
- `docs/wiki/modules/flows/client-behavior-routing.md` documents atomic manifest installation and hot-path key classification without synchronous IPC.

### Command registry, command execution, and manual completion trigger

- `src/protocol/mod.rs` already declares a built-in `completion.trigger` command as `CommandDeclaration::ui_reactive("completion.trigger", "Trigger Completion")`, and `KeyBindingContext::CompletionMenu` already exists for completion-menu key routing.
- `docs/wiki/modules/command-registry.md` and the Phase 18.8 `CommandExecution` path validate command ID, routing policy, package provenance, permissions, bounded arguments, and session/action freshness before side effects.
- Manual completion (`Ctrl+Space`/bound `completion.trigger`) can request completions without a trigger character by reusing the same request builder with a `Manual` trigger reason; it must not mutate document text.

### Transient menu session and overlay projection

- `src/shell/transient_menu.rs` owns the generic `TransientMenuSession`: bounded prompt/query/items/selection/status/focus/accessibility state with inert activation actions.
- `docs/wiki/modules/transient-menu-session.md` states the session model is generic for command palettes, completion pickers, file search, symbol search, Git pickers, and package-provided quick-pick workflows; activation produces inert command actions routed through `CommandExecutor`.
- `src/shell/package_ui.rs` and `src/masonry_sdui.rs` project active transient overlay/component state; `src/server/control_center.rs` is the first consumer that builds a `TransientMenuSession` from `CommandRegistry` snapshots and routes activation through `CommandExecutor`.
- Completion display should reuse this session/overlay path. Completion acceptance differs from command execution: it commits a validated text replacement in the active document rather than executing a command, so a completion-specific action/adapter variant is the expected minimal extension.

### Mode registry, fallback modes, and classification

- `src/packages/modes.rs::ModeRegistry` owns document classification, active major-mode state, fallback registration, and behavior manifest selection.
- Phase 18.9 supplies always-on `core.text` and `core.code` fallback modes through `DocumentClassification` and `MajorModeActivation`; any document remains editable even with no language package.
- Completion must stay generic across modes and must not add a `core.rust`/`core.typescript`/language-specific Rust mode branch. A `core.code` document can receive completions from a built-in buffer-word provider without a language package.

### Parse coordinator and background work

- `src/server/parse_coordinator.rs` owns cancellable background parse scheduling, handler registration, generation replacement, stale-version rejection, parse-window validation, syntax memory budgets, and per-document cancellation.
- `docs/wiki/modules/parse-coordinator.md` documents the generation/cancellation/stale-result model. Completion provider execution should reuse the same lifecycle shape (per-document/client active request, generation, abort/stale-drop, timeout, bounded payloads) on a `UiReactivePriority` lane instead of the `Background` parse lane.
- Tokio `JoinHandle::abort`/`AbortHandle` and `JoinSet` provide cancellation for async tasks; `spawn_blocking` tasks cannot be reliably aborted after start, so cancellable provider work must be async, not blocking.

### Syntax grammar registry and package provenance

- `src/server/syntax.rs` owns `SyntaxGrammarRegistry`, active syntax grammar selection, package-provenance records, and `TreeSitterSyntaxHandler`; `src/packages/record.rs` parses and validates `SyntaxGrammarContributionDescriptor` metadata.
- `docs/wiki/modules/syntax-grammar-registry.md` documents the registry/provenance/active-selection pattern. Completion provider registration should reuse the same package-prefixed ID, provenance, permission, and disable/revocation withdrawal model.
- Active completion providers are separate from active major mode and active syntax grammar: a document may have `active_major_mode = core.code`, `active_syntax_grammar = rust`, and `active_completion_providers = [core.bufferWords]` independently.

### Decoration transport and payload budgets

- `src/protocol/decorations.rs` and `src/server/decorations.rs` validate document versions, byte ranges, style tokens, permissions, provenance, and `DECORATION_PAYLOAD_BUDGET_BYTES` before cache insertion.
- Completion result payloads should follow the same bounded, versioned, provenance-bearing validation shape, using `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES` and bounded item counts/string lengths. Completion items are inert text replacement data, not decorations.

### Package loading, manifest validation, and permissions

- `src/packages/permissions.rs` already defines `PackagePermission::CompletionProvider` parsed from `completion-provider`.
- `src/packages/service.rs` already counts a `completions` withdrawal when a package with `CompletionProvider` permission is disabled/revoked, so package disable/revocation already removes completion contributions.
- `src/packages/record.rs` and `src/packages/manifest.rs` validate package identity, `apiPrefix`, entry/load-entry confinement, capabilities, package graph metadata, and bounded manifest payloads.
- `docs/wiki/modules/package-loading.md` documents first-party `loadPackage("@clay/*")` loading, package record assembly, provenance, rollback, and tests. Completion provider packages should reuse this loading boundary and must not auto-load silently.

### Performance budgets and protocol codec

- `src/perf/budgets.rs` defines `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES` (promoted in Phase 18.11 to 16 KiB so a full 256-item `TransientMenuSession`-bound result fits), `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`, and the transient menu item/query/string caps.
- `src/protocol/codec.rs` provides rkyv round-trip and oversized-frame rejection. Completion request/result messages should reuse this codec validation and reject oversized result payloads before client publication.
- `ClientMessage::CommandIntent` currently carries only `client_id`, `document_id`, `behavior_version`, and `command_id`; completion requests need additional cursor/trigger/replacement-range metadata, so a typed `CompletionRequest`/`CompletionResultSet` shape is the expected gap rather than overloading `CommandIntent`.

### Docs registry and wiki coverage

- `docs/reference/primitives/registry.md` is the canonical primitive taxonomy and already records `CompletionTriggerAndResult` as `Deferred`; Phase 18.11 promotes it to `New`/in-progress with implementation notes.
- `docs/reference/primitives/backlog.md` is the phase queue and currently lists `CompletionTriggerAndResult` under `Deferred`; Phase 18.11 moves it to a Phase-18.11 implementation row.
- `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md` must link this review so future plans find the primitive inventory.
- `tests/primitives_docs.rs` should fail if the review, registry/backlog rows, hot-path split, permission boundary, and wiki index links stop mentioning the completion primitive and its authority boundaries.

## Generic Phase 18.11 Primitive Gaps

### `CompletionRequest` / `CompletionResultSet` / `CompletionItem`

Typed completion protocol shapes are the first gap. `CompletionRequest` should carry `request_id`, `document_id`, `document_version`, `behavior_version`, `cursor_byte_offset`, `replacement_range` (prefix/word range), trigger reason (character or manual), and provider generation. `CompletionResultSet` should carry `request_id`, bounded `CompletionItem` list, replacement range, provider provenance, and status/diagnostics. `CompletionItem` should carry `label`, `insert_text`, `detail`, `commit_characters`, and provenance only.

This primitive is generic. Acceptable implementation names include `CompletionRequest`, `CompletionResultSet`, `CompletionItem`, `CompletionTrigger`/`CompletionTriggerReason`, and request/result lifecycle helpers. Rejected names include `RustCompletionProvider`, `TypeScriptCompletionProvider`, `LspCompletionItem`, or any `if language == "rust"` / `if extension == "ts"` / `if package == "@clay/javascript"` Rust server/client branch.

### `CompletionProviderRegistry`

A provider registry should validate and retain completion provider declarations by package prefix and provider ID, with required `completion-provider` permission for package providers, trigger metadata, word-boundary rules, provider priority, generation, timeout, max items, and payload budgets. It should support built-in Rust providers and resolver-validated package providers through one generic trait/adapter, and record why a provider was selected, skipped, disabled, or revoked.

The registry should run at package load/reload or explicit registration time. It must not run provider JavaScript in keypress, paint, layout, scroll, pointer, or text-event handlers. Provider priority/conflict ordering must be deterministic and preserve package/built-in provenance.

### `CompletionCoordinator` (cancellable UI-reactive lane)

A completion coordinator should own per-document/client active request state, spawn cancellable async provider work on a `UiReactivePriority` lane, abort or stale-drop older in-flight requests when a newer edit/cursor/mode/provider-generation request arrives, and validate results against the current document/behavior version before UI publication. It should mirror the `ParseCoordinator` generation/cancellation/stale-result lifecycle in a smaller form, without reusing the `Background` parse lane.

Newer requests cancel older work; stale results publish nothing. Scheduling must return without blocking edit acknowledgement or local paint. Provider execution must be async and cancellable (`tokio::spawn` + `abort`/`AbortHandle`), not `spawn_blocking`, because blocking tasks cannot be reliably aborted after start.

### Behavior-manifest trigger routing and manual trigger

Client routing should return a first-class completion trigger route instead of dropping the existing `autocomplete_trigger_for_key` result. Typing a trigger character must edit locally first, then enqueue a `CompletionRequest` asynchronously through a bounded non-blocking channel. Manual `completion.trigger` should build the same request with a `Manual` trigger reason and must not mutate text.

Trigger classification remains local manifest lookup. Packages declare trigger metadata and word-boundary parameters only; executable trigger callbacks, regex bombs, raw JavaScript functions, or arbitrary predicates are rejected by manifest validation.

### `TransientMenuSession` completion display/accept adapter

Completion results display through the existing `TransientMenuSession` bottom overlay path with prompt/query/status, selected index, provider provenance, accessibility labels, commit characters, Enter/Tab acceptance, arrow navigation, and Escape dismissal. The extension is a completion-specific `CompletionMenuAcceptAction` adapter stored on `TransientMenuAction`; it commits a validated text replacement instead of calling `CommandExecutor`.

Filtering and selection movement are bounded local work. Accepting a completion produces a local text replacement edit for the active document only; it does not execute provider code, commands, raw ops, or side effects in paint/key/text handlers. No completion-specific Masonry widget tree was added: `TransientPackageOverlay::from_menu_session` still projects the menu, and completion items are omitted from command action targets so pointer/action routing cannot execute them as commands.

### Built-in buffer-word provider

The minimal built-in provider is `BufferWordCompletionProvider` in `src/server/completion.rs`, registered as `core.bufferWords` via the same `CompletionProvider` trait/adapter as package providers. It suggests unique words from the server-prepared `CompletionDocumentWindow` around the current replacement prefix, excludes the current prefix duplicate, uses deterministic `BTreeSet` sorting, caps item count/string lengths/result payloads, returns the request replacement range unchanged, and carries `core`/built-in provenance on the result and every item.

It reads only the bounded provider window supplied by the coordinator. Oversized windows are rejected before provider execution, and generated results stop before `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`; no full-document, filesystem path, workspace index, shell output, network, AI, package code, or UI branch is involved.

### Clay JS completion provider registration API

The public Clay JS API is `clay:completion.serverRegisterCompletionProvider` (`completion.serverRegisterCompletionProvider`), permission `completion-provider`, following Clay JS API naming/boundary rules. The source-tree facade lives in `runtime/js/completion.js`; the runtime includes that same file through `src/server/facades.rs`; the op wrapper is `src/server/ops/completion.rs::op_clay_completion_register_completion_provider`; the registry/docs entry is `docs/reference/clay-js-api/completion/server-register-completion-provider.md`.

Phase 18.11 implements metadata-only package registration. The op reuses `assemble_package_record`, validates `clay.contributions.completionProviders`, requires `completion-provider`, enforces package-owned provider IDs and duplicate rejection, stores `CompletionProviderMeta` snapshots in `ClayOpState`, and exposes them on `ClayRuntimeEvaluation` for tests. It rejects arbitrary executable handler values (`handler`, `callback`, `complete`, `function`, `module`), raw ops, client JavaScript, native handles, snippets/commands, URLs, shell/network/AI/WASM/native/package-manager authority, and does not grant a package execution token. The built-in `core.bufferWords` provider is Phase 18.11's only computed provider; a future constrained handler bridge is still required for package-supplied computation.

Phase 18.18 extends this same metadata boundary with bounded package-owned `items: string[]`. `src/packages/record.rs` validates unique non-empty strings against `CompletionItem` field/count and contribution payload limits; `src/server/ops/completion.rs` normalizes them to provenance-bearing `CompletionProviderMeta.items: Vec<CompletionItem>`. `ClayJsRuntimeService` retains the successful evaluation's inert snapshot, and `src/server/connection.rs::static_package_completion_result` selects the active package/trigger, prefix-filters items, and returns a bounded result without invoking package JavaScript. This does not add callbacks or external authority: first-party base providers remain priority-0 inert static text data, and snippet transforms remain deferred to Phase 18.19.

## Hot-Path Classification

- Trigger classification / local edit: typing a trigger character edits locally first (`ClientFirstPredictable`); trigger metadata is read from installed inert manifest data. No provider execution, package JavaScript, IPC wait, filesystem scan, or full-document serialization before local paint.
- Request enqueue: after the local edit/event path updates shadow state, a typed `CompletionRequest` is enqueued through a bounded non-blocking channel. No synchronous IPC await before local paint.
- Provider execution / result computation: server-side `UiReactivePriority`, cancellable, generation-checked, stale-version-rejecting, bounded by `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`. Newer requests abort or stale-drop older in-flight work. The built-in buffer-word provider uses the same lane and provider trait as package providers.
- Menu render / selection / accept: paint and key/text handlers read installed inert result/session state only; filtering/selection are bounded local work; acceptance commits a validated text replacement. No provider execution, package JavaScript, raw ops, or blocking IPC in paint/key/text handlers.

Phase 18.11 must preserve `ClientFirstPredictable` local editing from Phase 18.9. Completion computation is `UiReactivePriority`, cancellable, stale-version-rejecting, and bounded by `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`, and the transient menu item/string caps. No synchronous IPC before local paint.

## Security and Authority Boundary

- Completion provider permission required: package providers must declare and be validated for `completion-provider`; undeclared authority is rejected before registration. Disabled/revoked providers are removed through the existing `completions` withdrawal count and degrade to no completion while documents remain editable.
- Inert result items only: `CompletionItem` data is text replacement data (label, insert text, detail, commit characters, provenance). No callbacks, snippets with executable transforms, command side effects on accept, raw op names, native handles, CSS, file paths, shell/network/AI directives, or client-side JavaScript fields are accepted.
- No new default authority: completion adds no filesystem access beyond already-open Clay-provided document snapshots, network, shell, AI, workspace index, WASM, raw ops, native widgets, client-side JavaScript, package-manager execution, package enable/disable, or side-effectful accept actions. Any future provider needing workspace, network, AI, shell, or filesystem authority must introduce explicit permissions and a future approved decision log before implementation.
- Package provenance and loading boundary: package providers require package provenance and explicit one-line `loadPackage("@vendor/provider")` loading for normal setup; no provider package auto-loads silently, and arbitrary executable callbacks from user configuration are rejected. Phase 18.11 stores package completion metadata only; it does not expose executable JS provider tokens.
- Trigger metadata is manifest data only: packages declare trigger characters and word-boundary parameters as inert manifest metadata; executable trigger callbacks, regex bombs, raw JavaScript functions, or arbitrary predicates are rejected.

## Rejected Implementation Shapes

- Do not add language-specific Rust completion branches such as `RustCompletionProvider`, `TypeScriptCompletionProvider`, `JavascriptCompletionProvider`, or `if language == "rust"` / `if extension == "ts"` server/client code.
- Do not add a completion-only menu widget, bespoke completion popup, or completion-specific Masonry widget tree; reuse `TransientMenuSession` and the existing transient overlay path.
- Do not run provider JavaScript or completion computation in Masonry paint, layout, keypress, pointer, scroll, or text-event handlers.
- Do not block local typing/rendering on synchronous IPC, provider execution, package JavaScript, filesystem scans, or full-document serialization before local paint.
- Do not accept raw callbacks, executable snippets, command side effects on accept, raw op names, native handles, CSS, file paths, or client-side JavaScript in completion items.
- Do not grant filesystem, network, shell, AI, workspace-index, WASM, raw-op, native-widget, client-JS, package-manager, or package-enable/disable authority by default.
- Do not silently auto-load completion provider packages; end users should use explicit one-line `loadPackage("@vendor/provider")` setup when package providers ship.
- Do not implement LSP, AI, workspace-index, snippet-expansion, shell/tool, or network-backed providers in this phase; they remain later add-ons requiring a future approved decision log and explicit permissions.

## Final Implementation Status

All Phase 18.11 plan tasks are now complete. The protocol shapes (`CompletionRequest`/`CompletionResultSet`/`CompletionItem`/`CompletionTrigger`), provider registry (`CompletionProviderRegistry`), cancellable UI-reactive lane (`CompletionCoordinator`), behavior-manifest trigger routing and manual `completion.trigger`, `TransientMenuSession` completion display/accept adapter (`CompletionMenuAcceptAction`), built-in `core.bufferWords` provider (`BufferWordCompletionProvider`), and Clay JS registration API (`completion.serverRegisterCompletionProvider`, metadata-only) are implemented and verified. The registry/backlog rows promote `CompletionTriggerAndResult` from `Deferred` to Phase-18.11 implemented status. LSP, AI, workspace-index, snippet-expansion, shell/tool, and network-backed providers remain later add-ons requiring a future approved decision log and explicit permissions; Phase 18.11 did not expand provider execution authority, so no new decision log was needed.

## Tests

- `tests/primitives_docs.rs`: static coverage that this review is linked from the wiki index and primitive architecture page; registry/backlog mention `CompletionTriggerAndResult` and Phase 18.11; the review records existing inventory, generic completion gaps, hot-path split, and the `completion-provider` permission/security boundary.
- `tests/completion_provider.rs`: covers completion registry/coordinator behavior plus `core.bufferWords` unique sorted prefix matches, empty status, result payload cap, bounded-window rejection, stale document-version rejection after newer requests, duplicate provider-ID conflict diagnostics, disabled package provider fallback to built-in buffer words, oversized result rejection before publication, package cancellation preserving built-in fallback, and registry budget validation.
- `src/server/js_runtime.rs`: `completion_facade_registers_provider_metadata_without_raw_ops`, `completion_facade_rejects_callbacks_missing_permission_and_bad_prefix`, and `load_package_completion_provider_fixture_registers_metadata` cover the runtime facade/op metadata registration path, authority rejection, and explicit `loadPackage` fixture path.
- `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, and `tests/clay_js_facade_layout.rs`: cover the public `clay:completion` facade, registry/docs entry, generated registry freshness, and source-tree facade layout.
- `tests/rust_visibility_api_mapping.rs`: allowlists `src/server/completion.rs` public items as non-JS server infrastructure (only `op_clay_completion_register_completion_provider` is the public JS API backing) and verifies `TransientMenuSession` stays `pub(crate)`.
- `tests/manual_smoke_docs.rs`: `phase18_11_manual_completion_smoke_has_runnable_contract` verifies `docs/development/launch-and-gui-smoke.md` defines the Phase 18.11 completion smoke contract (manual trigger binding, menu display/navigation/commit/dismiss, trigger-character local-first edit, stale-result drop, disabled-provider fallback, performance/security contract, and automated coverage list).
- Package reference documentation uses generic manifest/API/security validators in `tests/package_loading_docs.rs`; executable package/runtime tests remain authoritative for behavior.
- Clay JS API documentation uses generic inventory/index/generated-registry/facade/security validators in `tests/clay_js_api_inventory.rs`.
- `tests/package_primitive_gate.rs`: covers completion provider contribution permission requirements, duplicate IDs, package-owned ID validation, oversize metadata, and rejection of raw ops, command/snippet, shell, URL, and client-JavaScript authority fields.
- `tests/editor_performance_invariants.rs`: `completion_hot_paths_use_inert_state_and_nonblocking_enqueue_only` statically guards the editor key/text/paint path against provider/coordinator/package/runtime work while requiring bounded `try_send` request enqueue.
- `tests/performance_protocol.rs`: covers representative completion result codec/payload budget alongside the shared protocol payload guardrails.
- Commands: `cargo test --test protocol primitives_docs:: --quiet`.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.10 Tree-sitter Grammar Primitive Review](phase18.10-tree-sitter-grammar-primitive-review.md)
- [Phase 18.9 Generic Text/Code Modes Primitive Review](phase18.9-generic-text-code-modes-primitive-review.md)
- [Phase 18.8 Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md)
- [Behavior Manifests](behavior-manifests.md)
- [Transient Menu Session](transient-menu-session.md)
- [Control Center](control-center.md)
- [Command Registry](command-registry.md)
- [Mode Registry](mode-registry.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [Package Loading](package-loading.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Primitive Registry](../../reference/primitives/registry.md)
- [Primitive Backlog](../../reference/primitives/backlog.md)
- [Package Security](../../reference/primitives/package-security.md)
