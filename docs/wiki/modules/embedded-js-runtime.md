# Embedded JavaScript Runtime

## Source

- `src/server/js_runtime.rs`
- `src/server/ops/mod.rs`
- `src/server/ops/syntax.rs`
- `src/server/ops/typography.rs`
- `src/server/ops/completion.rs`
- `src/server/syntax.rs`
- `src/server/completion.rs`
- `src/server/mod.rs`
- `src/server/js_runtime.rs` tests

## Overview

Clay embeds `deno_core` behind a persistent server-side runtime service. The runtime boundary supports evaluating controlled ES modules on one long-lived V8 owner thread per runtime generation, importing curated `clay:*` facade modules, installing Clay-owned ops, converting JavaScript failures into typed Rust errors, and keeping runtime work outside client rendering and ordinary typing paths.

## Responsibilities

- Own construction and lifetime of one `deno_core::JsRuntime` per `ClayJsRuntimeService` / server configuration generation.
- Evaluate controlled server-provided JavaScript on a dedicated runtime worker thread instead of synchronously from GUI or IPC hot paths.
- Enforce a hard wall-clock timeout on every evaluation so a runaway `while(true) {}` or hanging async op cannot block startup, file open, or package load indefinitely.
- Resolve only explicit Clay facade specifiers, allowed local configuration modules, and the vendored `markdown-it` ESM shim needed by the first-party Markdown parser adapter.
- Return typed Rust results/errors to callers rather than panicking on JavaScript exceptions or op validation failures.
- Convert runtime failures into sanitized `RuntimeDiagnostic` values for server logs, tests, and client status display.

## How It Works

`RuntimeGenerationStore` owns the active `{ id, ClayJsRuntimeService, diagnostics }` generation for the server. `IpcServer::trigger_developer_hot_reload` is the deterministic non-GUI reload trigger for tests and developer workflow; it is a thin wrapper around `IpcServer::reload_runtime_generation` and adds no package-manager, filesystem, network, shell, or third-party package authority. `IpcServer::reload_runtime_generation` builds the next `ClayJsRuntimeService` off to the side, evaluates configured `init.js`/default package loads on that fresh runtime, and swaps the store only after configuration evaluation succeeds. After a successful swap, `refresh_open_documents_after_reload` enumerates already-open server-owned documents, reruns the same generic selected-file classification/activation path for each document, and returns only follow-up `BehaviorManifest`, `DecorationSet`, or diagnostic messages; it does not send `DocumentOpened`/`DocumentReloaded` full-text snapshots. Failure records a sanitized runtime diagnostic and keeps the previous generation ID/service active. Existing connection tasks ask the store for `current()` before selected-file activation, so later opens use the newest successful generation without respawning IPC connections.

`ClayJsRuntimeService` starts a dedicated `clay-js-runtime` worker thread when the service is constructed. That worker owns one `deno_core::JsRuntime`, one single-thread Tokio runtime for driving `run_event_loop`, one mutable `ClayModuleLoader`, and one shared `ClayOpState`. Public async methods (`evaluate_controlled_module`, `load_configuration_from_root*`, and default configuration loading) send `RuntimeCommand::Evaluate` requests over a channel and await a oneshot response; the caller never holds or shares the V8 runtime.

The first evaluation is loaded as the runtime's main ES module. Later evaluations use Deno side modules with unique `clay://runtime/main-N.js` specifiers, so global JS state, imported package modules, the first-party `loadEntry` allowlist, and registered package metadata survive across evaluations. `ClayOpState::begin_evaluation` clears per-evaluation records/SDUI/decorations before each command while preserving long-lived package/mode/handler registries needed by Phase 18.7. `ClayOpState::set_runtime_context` updates the current workspace and document id for the command without rebuilding the runtime.

Each evaluation is guarded by a configurable wall-clock timeout defaulting to [`JS_RUNTIME_EVALUATION_TIMEOUT_MS`](../../src/perf/budgets.rs) (currently 5 seconds). Before V8 starts executing, `evaluate_loaded_module` captures an `IsolateHandle` from `runtime.v8_isolate().thread_safe_handle()` and starts a watchdog thread. If the evaluation completes before the timeout, the watchdog is cancelled. If the timeout elapses first, the watchdog calls `IsolateHandle::terminate_execution()`, which injects an uncatchable V8 exception into the running isolate. The event loop future then returns a termination error; Rust maps this to `ClayRuntimeError::Timeout` and produces a `RuntimeDiagnostic` with code `clay.runtime.timeout`. The watchdog thread is detached on the happy path and exits cleanly when cancelled.

Phase 18.16.5 keeps typography ownership on this same server-runtime boundary. `clay:theme.setTypography` passes one complete inert three-profile candidate to `op_clay_theme_set_typography`; the op accepts no unknown fields, validates all profiles before runtime-local replacement, and `ClayRuntimeEvaluation` carries the candidate out only after JavaScript evaluation succeeds. `RuntimeGenerationStore` then revalidates, retains the active default or changed profile set, assigns its monotonic server revision, and broadcasts exactly one `ServerMessage::ActiveTypography` update to each active connection. The client registry/bootstrap consumer is intentionally a later task; this task never does installed-font enumeration, font-file I/O, font download, or paint-path work.

`src/server/ops/mod.rs` defines a focused Clay op extension. `op_clay_runtime_ping` proves explicit op dispatch is wired, and `op_clay_runtime_record` validates a string payload before storing it in server-owned `ClayOpState`. Public configuration code should use documented `clay:*` imports instead of raw op names. The extension also installs configuration ops, SDUI node-construction/publication ops, runtime-backed document/workspace ops, key binding/behavior manifest ops, syntax grammar registration ops, completion provider metadata registration ops, and a shared planned-unavailable op used by facade functions whose runtime backing is intentionally deferred. `ClayOpState` stores the last validated JavaScript-published `SduiTree` plus a runtime-local `ActiveBehaviorManifest`; `ClayRuntimeEvaluation` returns changed SDUI and behavior manifest state to server startup code without sending protocol frames from inside V8.

Runtime evaluation output application is centralized in one primitive so server startup and document-open flows share identical state mutation and validation. `crate::server::apply_runtime_outputs` takes a `ClayRuntimeEvaluation` plus a target document id and the shared behavior/SDUI state, applies the behavior manifest (via `ActiveBehaviorManifest::publish_replacement`) and the per-document SDUI tree (via `StaticSduiState::replace_for_document_with_runtime_tree`), and returns a `RuntimeOutputApplication` carrying the applied results, the published decoration set (passed through for the caller to emit), and unified diagnostics (`clay.behavior.invalid_manifest`, `clay.sdui.invalid_tree`). `IpcServer::apply_runtime_evaluation` also registers live JS parse handlers from the same evaluation by adapting each `JsParseHandlerRegistration` into `ParseCoordinator`. `connection::open_document_followup_messages` composes the per-client `BehaviorManifest`, `DecorationSet`, and `SduiSnapshot` messages from the same result for selected-file, workspace, and file-browser opens. Package-UI contribution snapshots are still collected on `ClayRuntimeEvaluation` for test inspection but are **not** applied at this boundary because the shell owns the package-UI registry that a snapshot would merge into.

`ClayModuleLoader` is intentionally restrictive. The persistent worker mutates only the loader's current entry state before an evaluation; the import policy stays deny-by-default. The controlled main/side module can be loaded, any controlled/configuration module can import curated `clay:configuration`, `clay:sdui`, `clay:documents`, `clay:workspace`, `clay:keybindings`, `clay:behavior`, `clay:syntax`, `clay:completion`, `clay:application`, and `clay:editor` generated ESM facades, and configuration evaluation can additionally resolve explicit relative `.js` files under the canonical configuration root. The only package-style import allowed today is `markdown-it`, which resolves to Clay's vendored first-party Markdown bundle under `packages/markdown/node_modules/markdown-it/dist/markdown-it.js` and is exposed as an ESM default export for the server-side Markdown parser adapter. Unknown URLs, other package-style imports, extensionless files, and traversal outside that root fail with typed runtime/configuration errors.

Open-time activation reuses the persistent runtime instead of building a temporary mode-specific runtime root. `connection::open_document_followup_messages` asks the runtime to classify the opened path through `clay:modes`; if no mode is registered yet, the runtime scans first-party `@clay/*` package specifiers and calls idempotent `loadPackage` until classification succeeds. Package mode declarations store activation metadata in the persistent `clay:modes` facade, so after startup `await loadPackage("@clay/markdown")`, opening or reload-refreshing `note.md` activates Markdown for that document without reloading the package in the same generation. The same helper runs for `OpenDocument`, selected-file opens, and file-browser/list-item opens. On Phase 19 reload, package authors get a fresh generation: `init.js` reruns, `loadPackage` starts with an empty `globalThis.__clayLoadedPackages` cache, and the package `loadEntry` must rebuild mode, command, UI, and parse registration state. When a package owns the match, the connection schedules a bounded initial parse window through `ParseCoordinator` and emits the validated `DecorationSet`. This keeps package JS on the server runtime worker, avoids per-open `JsRuntime` construction, and removes the former Markdown-specific dist-copy/init-script branch.

For Phase 13 configuration use, the runtime op state can share the server's `WorkspaceState`. `IpcServer` normally evaluates `~/.config/clay/init.js` when present; development smoke can instead set `ServerConfig::configuration_root` through `cargo run -- smoke-gui --config-fixture runtime-sdui`, which points the child server at `tests/fixtures/configuration/runtime-sdui/init.js`. `clay:documents` ops call the existing Phase 9 `open_existing_file`, `save_document`, `reload_document`, `document_metadata`, and `list_documents` helpers, while `clay:workspace` lists configured root metadata. Results are serialized as facade JSON with string IDs and sanitized workspace-relative paths. Workspace errors are converted through `WorkspaceError::diagnostic`, so traversal, unknown roots/documents, invalid UTF-8, dirty reloads, stale saves, and IO failures remain typed server validation failures instead of raw filesystem access.

Syntax grammar registration follows the same server-runtime boundary. `clay:syntax.serverRegisterSyntaxGrammar` validates first-party grammar package metadata through `op_clay_syntax_register_syntax_grammar`, reusing `assemble_package_record` before inserting into `SyntaxGrammarRegistry`. The facade/op reject executable callbacks, raw op fields, native handles, client JavaScript, non-`@clay/*` grammar packages, arbitrary/native artifact paths, raw CSS/colors, and other authority-bearing metadata. `ClayRuntimeEvaluation` exposes the runtime-local registered syntax grammar snapshot for tests; actual Tree-sitter parse/highlight work still runs later through `ParseCoordinator` as Background no-hot-path work.

Completion provider registration follows the same server-runtime boundary but is metadata-only in Phase 18.11. `clay:completion.serverRegisterCompletionProvider` validates package-shaped completion provider metadata through `op_clay_completion_register_completion_provider`, reusing `assemble_package_record` before storing `CompletionProviderMeta` snapshots in `ClayOpState` / `ClayRuntimeEvaluation`. The facade/op require `completion-provider`, package-owned provider IDs, duplicate rejection, inert trigger/word-boundary metadata, and bounded timeout/item caps. They reject `handler`, `callback`, `complete`, `function`, `module`, client JavaScript, native handles, raw ops, snippets, commands, URLs, shell/network/AI/WASM/native/package-manager authority, and any package provider execution token; `core.bufferWords` remains the executable provider until a future handler bridge is implemented.

Parse-handler registration follows the same server-runtime boundary. `clay:parse.serverRegisterParseHandler` validates package metadata and budgets through `op_clay_parse_register_parse_handler`; executable `handler`/`callback`/`onParse`/`function` keys are rejected in the facade and op. The JS facade stores the package module export behind a server-issued token in the persistent runtime, and Rust registers that token with `ParseCoordinator` under the owning runtime generation ID. Hot reload replaces same package/mode handlers with the new generation and cancels old-generation parse tasks before swap. Rust later invokes the active token through `RuntimeCommand::Parse` with the smaller of the service timeout and the handler's registered `timeoutMs`. The handler returns inert update JSON, which Rust converts to `IncrementalParseUpdate` and lets `ParseCoordinator` validate generation/document freshness before publication.

Key binding registration follows the same server-runtime boundary. `clay:keybindings` ops parse and validate single key chords, scopes, and allowlisted command IDs, then compile registrations into a versioned `BehaviorManifest` through `ActiveBehaviorManifest::publish_replacement`. `clay:behavior` ops expose summaries and routes for the active runtime manifest. The client still receives and routes inert manifests; no JavaScript handler is installed for keypresses.

Runtime error reporting is intentionally narrow and sanitized. `ClayRuntimeError::diagnostic` maps syntax errors, invalid imports, configuration module denials, op validation failures, SDUI validation failures, document/workspace validation failures, keybinding command denials, timeouts, and heap-limit termination into `RuntimeDiagnostic { severity, code, message }`. Diagnostic messages use stable Clay error codes and generic safe detail instead of raw source snippets, environment dumps, tokens, or absolute local paths. `IpcServer` stores diagnostics produced during default configuration loading or runtime-produced SDUI/behavior application; `handle_connection` publishes current diagnostics as `ServerMessage::RuntimeDiagnostic` after bootstrap snapshots so connected clients can update GUI status asynchronously.

The runtime is created with a server-owned V8 heap limit from [`JS_RUNTIME_HEAP_LIMIT_BYTES`](../../src/perf/budgets.rs) using `v8::Isolate::create_params().heap_limits(...)` through `deno_core::RuntimeOptions::create_params`. A near-heap-limit callback records that the heap guard fired, calls `terminate_execution()`, and lets Rust map the failure to `ClayRuntimeError::HeapLimit` / `clay.runtime.heap_limit` with sanitized text. The heap limit is not exposed through `init.js` or any Clay configuration API.

Timeout and heap-limit termination are treated as runtime-worker poisoning. The worker sends the sanitized diagnostic for the failed command, then exits instead of reusing the isolate. The next controlled evaluation lazily starts a fresh worker, so stale `globalThis` state and half-mutated module state are not reused. JS parse-handler tokens are not replayed into the fresh worker; server generation reload and `ParseCoordinator` generation checks remain responsible for replacing handlers and ignoring stale parse work.

## Code Examples

```rust
let service = ClayJsRuntimeService::default();
let result = service
    .evaluate_controlled_module(
        r#"Deno.core.ops.op_clay_runtime_record("configured");"#,
    )
    .await?;
assert_eq!(result.op_records, vec!["configured"]);
```

```rust
// Short timeout for tests; default is 5 seconds.
let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
let error = service
    .evaluate_controlled_module(r#"while (true) {}"#)
    .await
    .unwrap_err();
assert!(matches!(error, ClayRuntimeError::Timeout));
assert_eq!(error.diagnostic().code, "clay.runtime.timeout");
```

## Invariants and Constraints

- JavaScript execution is server-side only; the native Rust client does not run arbitrary JavaScript.
- Runtime lifetime is per server/configuration generation; Phase 19 hot reload replaces the service through `RuntimeGenerationStore`, while ordinary evaluation reuses the active generation's worker-owned `JsRuntime`.
- Runtime evaluation is startup/configuration/open-time work and must not be called from Masonry paint, text-event handling, or ordinary client-first typing.
- The runtime does not grant network, shell, package loading, direct client filesystem, WASM, or AI mutation authority; the document/workspace facade subset can only use server-configured workspace roots through existing server validation.
- Import support is deny-by-default except for the configuration entry point, curated `clay:*` facades, canonicalized relative `.js` modules below the configuration root, and the vendored `markdown-it` shim used by the first-party Markdown package.
- Runtime diagnostics must preserve safe detail only: stable code, severity, and actionable generic message; no raw absolute paths, source snippets, secrets, tokens, or capability-bearing handles.
- Every evaluation has a hard timeout; a runaway module is terminated and surfaced as `clay.runtime.timeout` instead of hanging the server.
- The persistent runtime has a hard V8 heap limit; heap growth is terminated and surfaced as `clay.runtime.heap_limit` without source text, paths, tokens, or package internals.
- Timeout or heap-limit termination stops the worker; later controlled evaluations run on a fresh worker, while parse-handler recovery stays generation-scoped through `RuntimeGenerationStore` and `ParseCoordinator`.
- Runtime facades may call Clay-owned ops internally, but user configuration should import facade functions and must not depend on raw `Deno.core.ops.op_*` names.
- Typography configuration is one atomic candidate validated before mutation. It exposes fallback-stack names and logical sizes only, never installed-font discovery, font files/bytes/URLs, downloads, renderer data, or extra authority.
- `clay:syntax.serverRegisterSyntaxGrammar` is a package-load-time public facade for first-party grammar packages only; ordinary user config should use `loadPackage("@clay/<language>")` and must not copy manifests or call raw syntax ops.
- `clay:completion.serverRegisterCompletionProvider` is a package-load-time metadata facade. It records provider metadata only and rejects executable package completion handlers in Phase 18.11.
- Document/workspace runtime ops are startup/configuration/server-first work. They are not invoked from client paint, text-event handling, or ordinary local edit application.
- Key binding registration compiles to inert behavior manifests. Client key routing uses installed manifests and never calls JavaScript synchronously.

## Tests

- `src/server/mod.rs`: `reload_runtime_generation_swaps_only_after_successful_configuration_load`, `successful_reload_refreshes_open_documents_without_full_snapshots`, and `failed_reload_keeps_previous_runtime_generation_active` verify generation ID changes, fresh service state after success, open-document refresh through generic mode activation, no full-text snapshot refresh frames, stale service retention after failure, and sanitized diagnostics.
- `tests/persistent_runtime_hot_reload.rs`: `developer_hot_reload_trigger_reports_success_and_sanitized_failure` verifies the non-GUI developer trigger reports success, returns sanitized failure diagnostics, and keeps the previous generation active after failure.
- `src/server/js_runtime.rs`: evaluates controlled modules on a persistent worker-owned runtime, verifies global JS state survives between evaluations, imports `clay:*` facades, rejects unsafe/unknown imports and platform authorities, converts JavaScript/op failures into typed errors and sanitized diagnostics, publishes runtime-generated SDUI, validates the runtime SDUI smoke fixture, runtime-backs the configuration-needed document/workspace facade subset, rejects executable parse callbacks/missing parse permission, bridges JS parse handlers into `ParseCoordinator`, registers first-party syntax grammar metadata through `clay:syntax`, registers completion provider metadata through `clay:completion`, enforces registered parse-handler `timeoutMs`, supports generic open-time path classification, compiles key binding registrations into behavior manifests, rejects unauthorized workspace paths/unknown commands, asserts ordinary typing does not enter the runtime, terminates runaway modules with the configured timeout, terminates heap growth with `clay.runtime.heap_limit`, restarts on the next controlled evaluation after timeout/heap worker poisoning, and verifies short timeouts do not break fast evaluations.
- `src/server/connection.rs`: `client_receives_js_generated_sdui_snapshot` verifies a runtime-generated tree stored in server SDUI state is emitted as the bootstrap `SduiSnapshot`; `server_sends_runtime_diagnostics_after_bootstrap` verifies stored diagnostics are published after bootstrap.
- `src/server/js_runtime.rs`: typography transaction/rejection tests verify complete replacement and no raw authority fields; `src/server/mod.rs::typography_update_reaches_connected_clients_once` verifies a changed configuration emits one bounded live server update.
- Command: `cargo test js_runtime --quiet`
- Command: `cargo test persistent_js_runtime_retains_global_state_between_evaluations --lib`
- Command: `cargo test js_parse_handler_bridge_runs_registered_markdown_handler --lib`
- Command: `cargo test syntax_facade --lib`
- Command: `cargo test load_package_registers_first_party_syntax_grammars --lib`
- Command: `cargo test js_parse_handler_timeout_uses_registered_budget --lib`
- Command: `cargo test js_runtime_infinite_loop_is_terminated_with_timeout --quiet`
- Command: `cargo test js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic --lib`
- Command: `cargo test js_runtime_timeout_recovery_uses_fresh_worker --lib`
- Command: `cargo test js_runtime_heap_limit_recovery_uses_fresh_worker --lib`
- Command: `cargo test client_receives_js_generated_sdui_snapshot --quiet`
- Command: `cargo test smoke --quiet`

## Related

- [Configuration Runtime](configuration-runtime.md)
- [Behavior Runtime Registration](behavior-runtime-registration.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
- `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`
- `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`
