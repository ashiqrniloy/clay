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
- Resolve only explicit Clay facade specifiers and allowed local configuration modules.
- Return typed Rust results/errors to callers rather than panicking on JavaScript exceptions or op validation failures.
- Convert runtime failures into sanitized `RuntimeDiagnostic` values for server logs, tests, and client status display.

## How It Works

`ClayJsRuntimeService::evaluate_controlled_module` accepts source text and uses `tokio::task::spawn_blocking` to isolate V8/runtime work from async server protocol tasks. The blocking worker builds a single-thread Tokio runtime, creates a `JsRuntime`, installs `clay_runtime_extension`, loads the source as `clay://runtime/main.js`, runs the event loop, awaits module evaluation, and returns collected op records.

`src/server/ops/mod.rs` defines a focused Clay op extension. `op_clay_runtime_ping` proves explicit op dispatch is wired, and `op_clay_runtime_record` validates a string payload before storing it in server-owned `ClayOpState`. Public configuration code should use documented `clay:*` imports instead of raw op names. The extension also installs configuration ops, SDUI node-construction/publication ops, runtime-backed document/workspace ops, key binding/behavior manifest ops, and a shared planned-unavailable op used by facade functions whose runtime backing is intentionally deferred. `ClayOpState` stores the last validated JavaScript-published `SduiTree` plus a runtime-local `ActiveBehaviorManifest`; `ClayRuntimeEvaluation` returns changed SDUI and behavior manifest state to server startup code without sending protocol frames from inside V8.

`ClayModuleLoader` is intentionally restrictive. The controlled main module can be loaded, any controlled/configuration module can import curated `clay:configuration`, `clay:sdui`, `clay:documents`, `clay:workspace`, `clay:keybindings`, `clay:behavior`, `clay:application`, and `clay:editor` generated ESM facades, and configuration evaluation can additionally resolve explicit relative `.js` files under the canonical configuration root. Unknown URLs, package-style imports, extensionless files, and traversal outside that root fail with typed runtime/configuration errors.

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

## Invariants and Constraints

- JavaScript execution is server-side only; the native Rust client does not run arbitrary JavaScript.
- Runtime evaluation is startup/configuration-style work and must not be called from Masonry paint, text-event handling, or ordinary client-first typing.
- The runtime does not grant network, shell, package loading, direct client filesystem, WASM, or AI mutation authority; the document/workspace facade subset can only use server-configured workspace roots through existing server validation.
- Import support is deny-by-default except for the configuration entry point, curated `clay:*` facades, and canonicalized relative `.js` modules below the configuration root.
- Runtime diagnostics must preserve safe detail only: stable code, severity, and actionable generic message; no raw absolute paths, source snippets, secrets, tokens, or capability-bearing handles.
- Runtime facades may call Clay-owned ops internally, but user configuration should import facade functions and must not depend on raw `Deno.core.ops.op_*` names.
- Document/workspace runtime ops are startup/configuration/server-first work. They are not invoked from client paint, text-event handling, or ordinary local edit application.
- Key binding registration compiles to inert behavior manifests. Client key routing uses installed manifests and never calls JavaScript synchronously.

## Tests

- `src/server/js_runtime.rs`: evaluates a controlled module, imports `clay:*` facades, rejects unsafe/unknown imports, converts JavaScript/op failures into typed errors and sanitized diagnostics, publishes runtime-generated SDUI, validates the runtime SDUI smoke fixture, runtime-backs the configuration-needed document/workspace facade subset, compiles key binding registrations into behavior manifests, rejects unauthorized workspace paths/unknown commands, and asserts ordinary typing does not enter the runtime.
- `src/server/connection.rs`: `client_receives_js_generated_sdui_snapshot` verifies a runtime-generated tree stored in server SDUI state is emitted as the bootstrap `SduiSnapshot`; `server_sends_runtime_diagnostics_after_bootstrap` verifies stored diagnostics are published after bootstrap.
- Command: `cargo test js_runtime --quiet`
- Command: `cargo test client_receives_js_generated_sdui_snapshot --quiet`
- Command: `cargo test smoke --quiet`

## Related

- [Configuration Runtime](configuration-runtime.md)
- [Behavior Runtime Registration](behavior-runtime-registration.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
