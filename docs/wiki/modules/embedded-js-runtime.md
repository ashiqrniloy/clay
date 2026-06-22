# Embedded JavaScript Runtime

## Source

- `src/server/js_runtime.rs`
- `src/server/ops/mod.rs`
- `src/server/mod.rs`
- `src/server/js_runtime.rs` tests

## Overview

Clay embeds `deno_core` behind a server-side runtime service. The runtime boundary supports evaluating a controlled main ES module, importing curated `clay:*` facade modules, installing Clay-owned ops, converting JavaScript failures into typed Rust errors, and keeping runtime work outside client rendering and ordinary typing paths.

## Responsibilities

- Own construction of `deno_core::JsRuntime` with Clay's extension and op state.
- Evaluate controlled server-provided JavaScript on a blocking runtime worker instead of synchronously from GUI or IPC hot paths.
- Enforce a hard wall-clock timeout on every evaluation so a runaway `while(true) {}` or hanging async op cannot block startup, file open, or package load indefinitely.
- Resolve only explicit Clay facade specifiers, allowed local configuration modules, and the vendored `markdown-it` ESM shim needed by the first-party Markdown parser adapter.
- Return typed Rust results/errors to callers rather than panicking on JavaScript exceptions or op validation failures.
- Convert runtime failures into sanitized `RuntimeDiagnostic` values for server logs, tests, and client status display.

## How It Works

`ClayJsRuntimeService::evaluate_controlled_module` accepts source text and uses `tokio::task::spawn_blocking` to isolate V8/runtime work from async server protocol tasks. The blocking worker builds a single-thread Tokio runtime, creates a `JsRuntime`, installs `clay_runtime_extension`, loads the source as `clay://runtime/main.js`, runs the event loop, awaits module evaluation, and returns collected op records.

Each evaluation is guarded by a configurable wall-clock timeout defaulting to [`JS_RUNTIME_EVALUATION_TIMEOUT_MS`](../../src/perf/budgets.rs) (currently 5 seconds). Before V8 starts executing, `evaluate_module_on_runtime` captures an `IsolateHandle` from `runtime.v8_isolate().thread_safe_handle()` and starts a watchdog thread. If the evaluation completes before the timeout, the watchdog is cancelled. If the timeout elapses first, the watchdog calls `IsolateHandle::terminate_execution()`, which injects an uncatchable V8 exception into the running isolate. The event loop future then returns a termination error; Rust maps this to `ClayRuntimeError::Timeout` and produces a `RuntimeDiagnostic` with code `clay.runtime.timeout`. The watchdog thread is detached on the happy path and exits cleanly when cancelled.

`src/server/ops/mod.rs` defines a focused Clay op extension. `op_clay_runtime_ping` proves explicit op dispatch is wired, and `op_clay_runtime_record` validates a string payload before storing it in server-owned `ClayOpState`. Public configuration code should use documented `clay:*` imports instead of raw op names. The extension also installs configuration ops, SDUI node-construction/publication ops, runtime-backed document/workspace ops, key binding/behavior manifest ops, and a shared planned-unavailable op used by facade functions whose runtime backing is intentionally deferred. `ClayOpState` stores the last validated JavaScript-published `SduiTree` plus a runtime-local `ActiveBehaviorManifest`; `ClayRuntimeEvaluation` returns changed SDUI and behavior manifest state to server startup code without sending protocol frames from inside V8.

Runtime evaluation output application is centralized in one primitive so server startup and the selected-file-open flow share identical state mutation and validation. `crate::server::apply_runtime_outputs` takes a `ClayRuntimeEvaluation` plus a target document id and the shared behavior/SDUI state, applies the behavior manifest (via `ActiveBehaviorManifest::publish_replacement`) and the per-document SDUI tree (via `StaticSduiState::replace_for_document_with_runtime_tree`), and returns a `RuntimeOutputApplication` carrying the applied results, the published decoration set (passed through for the caller to emit), and unified diagnostics (`clay.behavior.invalid_manifest`, `clay.sdui.invalid_tree`). `IpcServer::apply_runtime_evaluation` (server startup) pushes only the diagnostics, since behavior and SDUI are read lazily during the welcome handshake. `connection::selected_file_open_followup_messages` composes the per-client `BehaviorManifest`, `DecorationSet`, and `SduiSnapshot` messages from the same result. Parse-handler metadata and package-UI contribution snapshots are collected on `ClayRuntimeEvaluation` for test inspection but are **not** applied at this boundary: the config-eval runtime is short-lived so the JS parse-handler closures are already gone (real registration happens in the persistent runtime op-state), and the shell owns the package-UI registry that a snapshot would merge into. This replaces a previous silent drop with an explicit, documented deferral.

`ClayModuleLoader` is intentionally restrictive. The controlled main module can be loaded, any controlled/configuration module can import curated `clay:configuration`, `clay:sdui`, `clay:documents`, `clay:workspace`, `clay:keybindings`, `clay:behavior`, `clay:application`, and `clay:editor` generated ESM facades, and configuration evaluation can additionally resolve explicit relative `.js` files under the canonical configuration root. The only package-style import allowed today is `markdown-it`, which resolves to Clay's vendored first-party Markdown bundle under `packages/markdown/node_modules/markdown-it/dist/markdown-it.js` and is exposed as an ESM default export for the server-side Markdown parser adapter. Unknown URLs, other package-style imports, extensionless files, and traversal outside that root fail with typed runtime/configuration errors.

Document-open runtime evaluation can set a runtime document ID instead of always validating SDUI against document `1`. Phase 19 uses this for selected Markdown file opens: `ClayJsRuntimeService::load_configuration_from_root_for_document` evaluates a temporary first-party Markdown activation root for the opened document ID, and `clay:sdui` validates editor bindings against that document before returning the status tree to Rust.

For Phase 13 configuration use, the runtime op state can share the server's `WorkspaceState`. `IpcServer` normally evaluates `~/.config/clay/init.js` when present; development smoke can instead set `ServerConfig::configuration_root` through `cargo run -- smoke-gui --config-fixture runtime-sdui`, which points the child server at `tests/fixtures/configuration/runtime-sdui/init.js`. `clay:documents` ops call the existing Phase 9 `open_existing_file`, `save_document`, `reload_document`, `document_metadata`, and `list_documents` helpers, while `clay:workspace` lists configured root metadata. Results are serialized as facade JSON with string IDs and sanitized workspace-relative paths. Workspace errors are converted through `WorkspaceError::diagnostic`, so traversal, unknown roots/documents, invalid UTF-8, dirty reloads, stale saves, and IO failures remain typed server validation failures instead of raw filesystem access.

Key binding registration follows the same server-runtime boundary. `clay:keybindings` ops parse and validate single key chords, scopes, and allowlisted command IDs, then compile registrations into a versioned `BehaviorManifest` through `ActiveBehaviorManifest::publish_replacement`. `clay:behavior` ops expose summaries and routes for the active runtime manifest. The client still receives and routes inert manifests; no JavaScript handler is installed for keypresses.

Runtime error reporting is intentionally narrow and sanitized. `ClayRuntimeError::diagnostic` maps syntax errors, invalid imports, configuration module denials, op validation failures, SDUI validation failures, document/workspace validation failures, and keybinding command denials into `RuntimeDiagnostic { severity, code, message }`. Diagnostic messages use stable Clay error codes and generic safe detail instead of raw source snippets, environment dumps, tokens, or absolute local paths. `IpcServer` stores diagnostics produced during default configuration loading or runtime-produced SDUI/behavior application; `handle_connection` publishes current diagnostics as `ServerMessage::RuntimeDiagnostic` after bootstrap snapshots so connected clients can update GUI status asynchronously.

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
- Runtime evaluation is startup/configuration-style work and must not be called from Masonry paint, text-event handling, or ordinary client-first typing.
- The runtime does not grant network, shell, package loading, direct client filesystem, WASM, or AI mutation authority; the document/workspace facade subset can only use server-configured workspace roots through existing server validation.
- Import support is deny-by-default except for the configuration entry point, curated `clay:*` facades, canonicalized relative `.js` modules below the configuration root, and the vendored `markdown-it` shim used by the first-party Markdown package.
- Runtime diagnostics must preserve safe detail only: stable code, severity, and actionable generic message; no raw absolute paths, source snippets, secrets, tokens, or capability-bearing handles.
- Every evaluation has a hard timeout; a runaway module is terminated and surfaced as `clay.runtime.timeout` instead of hanging the server.
- Runtime facades may call Clay-owned ops internally, but user configuration should import facade functions and must not depend on raw `Deno.core.ops.op_*` names.
- Document/workspace runtime ops are startup/configuration/server-first work. They are not invoked from client paint, text-event handling, or ordinary local edit application.
- Key binding registration compiles to inert behavior manifests. Client key routing uses installed manifests and never calls JavaScript synchronously.

## Tests

- `src/server/js_runtime.rs`: evaluates a controlled module, imports `clay:*` facades, rejects unsafe/unknown imports, converts JavaScript/op failures into typed errors and sanitized diagnostics, publishes runtime-generated SDUI, validates the runtime SDUI smoke fixture, runtime-backs the configuration-needed document/workspace facade subset, compiles key binding registrations into behavior manifests, rejects unauthorized workspace paths/unknown commands, asserts ordinary typing does not enter the runtime, terminates runaway modules with the configured timeout, and verifies short timeouts do not break fast evaluations.
- `src/server/connection.rs`: `client_receives_js_generated_sdui_snapshot` verifies a runtime-generated tree stored in server SDUI state is emitted as the bootstrap `SduiSnapshot`; `server_sends_runtime_diagnostics_after_bootstrap` verifies stored diagnostics are published after bootstrap.
- Command: `cargo test js_runtime --quiet`
- Command: `cargo test js_runtime_infinite_loop_is_terminated_with_timeout --quiet`
- Command: `cargo test client_receives_js_generated_sdui_snapshot --quiet`
- Command: `cargo test smoke --quiet`

## Related

- [Configuration Runtime](configuration-runtime.md)
- [Behavior Runtime Registration](behavior-runtime-registration.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
