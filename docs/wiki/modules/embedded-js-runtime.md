# Embedded JavaScript Runtime

## Source

- `src/server/js_runtime/mod.rs`
- `src/server/js_runtime/tests.rs`
- `src/server/facades.rs`
- `runtime/js/*.js` and `runtime/js/*.d.ts`
- `src/server/ops/mod.rs`
- `src/server/ops/syntax.rs`
- `src/server/ops/typography.rs`
- `src/server/ops/completion.rs`
- `src/server/ops/language_intelligence.rs`
- `src/server/ops/language_server.rs`
- `src/server/ops/document_analysis.rs`
- `src/server/document_analysis.rs`
- `src/server/syntax.rs`
- `src/server/completion.rs`
- `src/server/mod.rs`

## Module layout (Plan 090)

`src/server/js_runtime.rs` was split into a directory module in Plan 090 (task 3). The facade and its adapters stay in `mod.rs`; the extracted responsibilities each own one private submodule:

| File | Contents |
|------|----------|
| `src/server/js_runtime/mod.rs` | `DomainRuntime`, `ClayJsRuntimeService` facade (service + channel + two-domain state ownership), and the `JsParseHandler`/`JsCompletionProvider`/`JsLanguageIntelligenceProvider` adapters; test-only service helpers remain `#[cfg(test)]` here |
| `src/server/js_runtime/error.rs` | `ClayRuntimeError`, `ClayRuntimeEvaluation`, `DocumentAnalysisInvocation` + diagnostic helpers |
| `src/server/js_runtime/worker.rs` | `RuntimeWorker`, `RuntimeEntry`, `RuntimeCommand`, `start_runtime_worker`/`run`/`create`/`prepare`, `harvest_op_state_evaluation`, `LoadedRuntimeEntry` |
| `src/server/js_runtime/source.rs` | `ClayModuleLoader` + `ModuleLoader` impl, `markdown_it_module_source`, `CONTROLLED_MAIN_SPECIFIER` |
| `src/server/js_runtime/evaluation.rs` | `evaluate_loaded_module`, `apply_persisted_preferences`, the `evaluate_js_*` bridges, `TerminationTimer` |
| `src/server/js_runtime/validation.rs` | parse/completion/language-intelligence/document-analysis JSON marshal/unmarshal + result validation |
| `src/server/js_runtime/tests.rs` | One sibling unit-test module retaining parent-private access; runtime, facade, package-load, trust-domain, loader, and configuration regressions |

External callers still reach the previously-`pub(crate)` types (`ClayRuntimeError`, `ClayRuntimeEvaluation`, `RuntimeEntry`, `RuntimeCommand`, `ClayJsRuntimeService`) at `crate::server::js_runtime::*` via `pub(crate)` re-exports in `mod.rs`.

## Overview

Clay embeds `deno_core` behind a persistent server-side runtime service. The runtime boundary supports evaluating controlled ES modules on one long-lived V8 owner thread per runtime generation, importing curated `clay:*` facade modules, installing Clay-owned ops, converting JavaScript failures into typed Rust errors, and keeping runtime work outside client rendering and ordinary typing paths.

## Responsibilities

- Own construction and lifetime of one `deno_core::JsRuntime` per `ClayJsRuntimeService` / server configuration generation.
- Evaluate controlled server-provided JavaScript on a dedicated runtime worker thread instead of synchronously from GUI or IPC hot paths.
- Enforce a hard wall-clock timeout on every evaluation so a runaway `while(true) {}` or hanging async op cannot block startup, file open, or package load indefinitely.
- Resolve only explicit Clay facade specifiers, allowed local configuration modules, and the vendored `markdown-it` ESM shim needed by the first-party Markdown parser adapter.
- Return typed Rust results/errors to callers rather than panicking on JavaScript exceptions or op validation failures.
- Convert runtime failures into sanitized `RuntimeDiagnostic` values for server logs, tests, and client status display.

## Two Runtime Trust Domains (Plan 061)

Clay runs exactly two persistent `JsRuntime` per `ClayJsRuntimeService`: one **Trusted** domain for Clay core, bundled first-party packages, and user configuration evaluation, and one **ThirdParty** domain for user-authorized third-party packages. The split is structural, not profile-based: `RuntimeDomain::Trusted` is granted by a compiled bundled inventory (`BUNDLED_PACKAGES` with FNV-1a-64 fingerprints over canonical roots), and everything else is `ThirdParty` with no promotion path into the trusted domain.

### Source

- `src/packages/bundled.rs` — `BUNDLED_PACKAGES` inventory, `verify_bundled_trust`, `bundle_extension_points_match_real_contributions` test.
- `src/server/js_runtime/mod.rs` — `DomainRuntime`, `dispatch_to_domain`, `replay_third_party_domain`, `production_reload`, `third_party_registrations_snapshot`.
- `src/server/js_runtime/worker.rs` — `start_runtime_worker`, `harvest_op_state_evaluation`.
- `src/server/ops/mod.rs` — `init_trusted_extension` (67 ops) vs `init_package_extension` (35 ops).
- `src/server/facades.rs` — one compile-time table includes all 23 executable facade files and marks the 14 public-third-party rows.
- `src/server/cross_domain.rs` — cross-domain typed invocation, envelope validation, requester/target approval checks.
- `plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md`

### Domain Workers

Each domain has its own `DomainRuntime` struct holding a worker thread, an mpsc sender for `RuntimeCommand`, a shared `ClayOpState`, and a monotonic generation counter. The trusted worker (`clay-js-runtime` thread) evaluates user configuration and trusted package load entries. The third-party worker (`clay-js-runtime-third-party` thread) evaluates only approved third-party package load entries. Both share a single `PackageService` and `PackageLoadEntryAllowlist` via `Arc`.

### Op and Facade Partitioning

- **Trusted extension** (67 ops): all Clay ops including admin/config-only (configuration evaluation, package loading, theme management, language-server grant, bridge dispatch).
- **Package extension** (35 ops): public third-party ops only — mode registration/activation, command/key/palette registration, parse/completion/language-intelligence provider registration, SDUI/decoration/diagnostic publication, theme application, typography setting, status-item registration, bindKey/unbindKey, and 4 cross-domain bridge result ops.
- **Facade allowlists**: trusted worker can import all 23 `clay:*` facades; third-party worker can import only 14 rows marked public in `src/server/facades.rs`. The same row supplies the `include_str!` executable source, so specifier, source ownership, and domain classification cannot drift across separate Rust lists. Independent Plan 061 inventory tests still verify the 14 security classifications. Missing facades fail at module-load time with `'not allowed in the server runtime boundary'` (fail-closed by import denial).
- **Op absence enforcement**: calling a trusted-only op in the third-party runtime produces `TypeError: undefined is not a function` — fail-closed by type absence rather than runtime gating at op entry.

### Cross-Domain Bridge

The `op_clay_packages_load_in_package_domain` bridge op receives a validated third-party record from the trusted domain, dispatches an `Evaluate` command to the third-party worker, and absorbs the returned `ClayRuntimeEvaluation` registrations (parse handlers, completion providers, language intelligence providers, document analyzers, SDUI/decoration/diagnostic sets) into the trusted worker's `ClayOpState`. Runtime records are merged; command/mode/UI registries remain per-worker.

Four bridge result ops (`runtime_record`, `parse_store_update`, `completion_store_result`, `language_store_intelligence_result`) are the only sanctioned internal data flow between domains after initial load routing.

### Worker Replacement and Replay

When the third-party worker is poisoned (timeout, heap limit, or send failure), `dispatch_to_domain` replaces the worker, rewires the cross-domain bridge sender, and calls `replay_third_party_domain` to re-dispatch `Evaluate` commands for all enabled third-party packages. This restores provider registrations without server restart. Tokens are deterministic (`{apiPrefix}:{providerId}:{index}`), so replayed registrations match existing coordinator entries. Parse, completion, and language-intelligence ops all call the single `ops::registration_token` formatter; its unit test pins that identity shape.

On trusted-domain config reload, `production_reload(current_service)` creates a fresh trusted worker while sharing the third-party `DomainRuntime` Arc, `PackageService` Arc, and `PackageLoadEntryAllowlist` Arc — so third-party packages, registrations, and language-server sessions survive config evaluation. Only trusted-worker resources are torn down; the third-party worker is never replaced on config reload.

### Document-Analysis Workers

Document-analysis workers are dynamically forked from the owning domain worker's `ClayOpState` via `new_document_analysis_worker`. They share the parent's `PackageService` and `load_entry_allowlist` but are NOT additional persistent runtimes. Timeout poisons the entire owning domain worker (not just the analysis worker), matching the exactly-two-persistent-runtimes topology.

### Invariants

- Exactly two persistent `JsRuntime` at steady state. Analysis workers are temporary and do not increase persistent runtime count.
- Third-party trust domain is shared across all third-party packages (one disclosed trust cohort), not per-package V8 isolates. Cross-package mutation within the third-party runtime is possible and disclosed to users at adoption time.
- No promotion path: `RuntimeDomain::ThirdParty` never becomes `Trusted` through any user action. Clay core (`@clay/core`) is not replaceable.
- Op set for the third-party runtime is a strict compile-time subset of the trusted op set (enforced by `domain_extension_is_strict_subset` test).
- Facade allowlist for the third-party runtime is a strict compile-time subset of the trusted facade allowlist.
- Approval gating is durable: third-party packages require a `PackageApprovalRecord` persisted at `~/.config/clay/packages` before evaluation. No code executes before approval.
- Two-runtime overhead vs one-runtime: ~5 MiB RSS, 2 extra threads, warm evaluation median < 265 μs (within task 1 baseline).

## How It Works

`RuntimeGenerationStore` owns the active `{ id, ClayJsRuntimeService, diagnostics }` generation for the server. `IpcServer::trigger_developer_hot_reload` is the deterministic non-GUI reload trigger for tests and developer workflow; it is a thin wrapper around `IpcServer::reload_runtime_generation` and adds no package-manager, filesystem, network, shell, or third-party package authority. `IpcServer::reload_runtime_generation` builds the next `ClayJsRuntimeService` off to the side, evaluates configured `init.js`/default package loads on that fresh runtime, and swaps the store only after configuration evaluation succeeds. After a successful swap, `refresh_open_documents_after_reload` enumerates already-open server-owned documents, reruns the same generic selected-file classification/activation path for each document, and returns only follow-up `BehaviorManifest`, `DecorationSet`, or diagnostic messages; it does not send `DocumentOpened`/`DocumentReloaded` full-text snapshots. Failure records a sanitized runtime diagnostic and keeps the previous generation ID/service active. Existing connection tasks ask the store for `current()` before selected-file activation, so later opens use the newest successful generation without respawning IPC connections.

`ClayJsRuntimeService` starts TWO dedicated worker threads when constructed: one trusted and one third-party. Facade source is compiled into the binary from `runtime/js/*.js` by `src/server/facades.rs`; loading performs only a static 23-row lookup and never reads or transpiles facade files at runtime. Adjacent `*.d.ts` files are declarations only. Each worker owns one `deno_core::JsRuntime`, one single-thread Tokio runtime for driving `run_event_loop`, one mutable `ClayModuleLoader`, and one shared `ClayOpState`. Public async methods (`evaluate_controlled_module`, `load_configuration_from_root*`, and default configuration loading) send `RuntimeCommand::Evaluate` requests over a channel and await a oneshot response; the caller never holds or shares the V8 runtime.

The first evaluation is loaded as the runtime's main ES module. Later evaluations use Deno side modules with unique `clay://runtime/main-N.js` specifiers, so global JS state, imported package modules, the first-party `loadEntry` allowlist, and registered package metadata survive across evaluations. `ClayOpState::begin_evaluation` clears per-evaluation records/SDUI/decorations before each command while preserving long-lived package/mode/handler registries needed by Phase 18.7. `ClayOpState::set_runtime_context` updates the current workspace and document id for the command without rebuilding the runtime.

Each evaluation is guarded by a configurable wall-clock timeout defaulting to [`JS_RUNTIME_EVALUATION_TIMEOUT_MS`](../../../src/perf/budgets.rs) (currently 5 seconds). Before V8 starts executing, `evaluate_loaded_module` captures an `IsolateHandle` from `runtime.v8_isolate().thread_safe_handle()` and starts a watchdog thread. If the evaluation completes before the timeout, the watchdog is cancelled. If the timeout elapses first, the watchdog calls `IsolateHandle::terminate_execution()`, which injects an uncatchable V8 exception into the running isolate. The event loop future then returns a termination error; Rust maps this to `ClayRuntimeError::Timeout` and produces a `RuntimeDiagnostic` with code `runtime.timeout`. The watchdog thread is detached on the happy path and exits cleanly when cancelled.

Phase 18.16.5 keeps typography ownership on this same server-runtime boundary. `clay:theme.setTypography` passes one complete inert three-profile candidate to `op_clay_theme_set_typography`; the op accepts no unknown fields, validates all profiles before runtime-local replacement, and `ClayRuntimeEvaluation` carries the candidate out only after JavaScript evaluation succeeds. `RuntimeGenerationStore` then revalidates, retains the active default or changed profile set, assigns its monotonic server revision, and broadcasts exactly one `ServerMessage::ActiveTypography` update to each active connection. The client registry/bootstrap consumer is intentionally a later task; this task never does installed-font enumeration, font-file I/O, font download, or paint-path work.

`src/server/ops/mod.rs` defines a focused Clay op extension. `op_clay_runtime_ping` proves explicit op dispatch is wired, and `op_clay_runtime_record` validates a string payload before storing it in server-owned `ClayOpState`. Public configuration code should use documented `clay:*` imports instead of raw op names. The extension also installs configuration ops, SDUI node-construction/publication ops, runtime-backed document/workspace ops, key binding/behavior manifest ops, syntax grammar registration ops, completion provider metadata registration ops, and a shared planned-unavailable op used by facade functions whose runtime backing is intentionally deferred. `ClayOpState` stores the last validated JavaScript-published `SduiTree`, a runtime-local `ActiveBehaviorManifest`, and configuration-owned keymap overlays. `bindKey`/`unbindKey` update those overlays only during configuration evaluation; each later package major-mode activation reapplies them after mode keymaps, so user chords such as `Ctrl+S` survive selected-file classification instead of disappearing when a package manifest replaces the active mode. `ClayRuntimeEvaluation` returns changed SDUI and behavior manifest state to server startup code without sending protocol frames from inside V8.

Runtime evaluation output application is centralized in one primitive so server startup and document-open flows share identical state mutation and validation. `crate::server::apply_runtime_outputs` takes a `ClayRuntimeEvaluation` plus a target document id and the shared behavior/SDUI state, applies the behavior manifest (via `ActiveBehaviorManifest::publish_replacement`) and the per-document SDUI tree (via `StaticSduiState::replace_for_document_with_runtime_tree`), and returns a `RuntimeOutputApplication` carrying the applied results, the published decoration set (passed through for the caller to emit), and unified diagnostics (`behavior.invalid_manifest`, `sdui.invalid_tree`). `IpcServer::apply_runtime_evaluation` also registers live JS parse handlers from the same evaluation by adapting each `JsParseHandlerRegistration` into `ParseCoordinator`. `connection::open_document_followup_messages` composes the per-client `BehaviorManifest`, `DecorationSet`, and `SduiSnapshot` messages from the same result for selected-file, workspace, and file-browser opens. Package-UI contribution snapshots are still collected on `ClayRuntimeEvaluation` for test inspection but are **not** applied at this boundary because the shell owns the package-UI registry that a snapshot would merge into.

`ClayModuleLoader` is intentionally restrictive. The persistent worker mutates only the loader's current entry state before an evaluation; the import policy stays deny-by-default. The controlled main/side module can be loaded, any controlled/configuration module can import curated `clay:configuration`, `clay:sdui`, `clay:documents`, `clay:workspace`, `clay:keybindings`, `clay:behavior`, `clay:syntax`, `clay:completion`, `clay:application`, and `clay:editor` generated ESM facades, and configuration evaluation can additionally resolve explicit relative `.js` files under the canonical configuration root. The only package-style import allowed today is `markdown-it`, which resolves to Clay's vendored first-party Markdown bundle under `packages/markdown/node_modules/markdown-it/dist/markdown-it.js` and is exposed as an ESM default export for the server-side Markdown parser adapter. Unknown URLs, other package-style imports, extensionless files, and traversal outside that root fail with typed runtime/configuration errors.

Open-time activation reuses the persistent runtime instead of building a temporary mode-specific runtime root. `connection::open_document_followup_messages` asks the runtime to classify the opened path through `clay:modes`; if no mode is registered yet, the runtime scans first-party `@clay/*` package specifiers and calls idempotent `loadPackage` until classification succeeds. Package mode declarations store activation metadata in the persistent `clay:modes` facade, so after startup `await loadPackage("@clay/markdown")`, opening or reload-refreshing `note.md` activates Markdown for that document without reloading the package in the same generation. The same helper runs for `OpenDocument`, selected-file opens, and file-browser/list-item opens. On Phase 19 reload, package authors get a fresh generation: `init.js` reruns, `loadPackage` starts with an empty `globalThis.__clayLoadedPackages` cache, and the package `loadEntry` must rebuild mode, command, UI, and parse registration state. When a package owns the match, the connection schedules a bounded initial parse window through `ParseCoordinator` and emits the validated `DecorationSet`. This keeps package JS on the server runtime worker, avoids per-open `JsRuntime` construction, and removes the former Markdown-specific dist-copy/init-script branch.

For Phase 13 configuration use, the runtime op state can share the server's `WorkspaceState`. `IpcServer` normally evaluates `~/.config/clay/init.js` when present; development smoke can instead set `ServerConfig::configuration_root` through `cargo run -- smoke-gui --config-fixture runtime-sdui`, which points the child server at `tests/fixtures/configuration/runtime-sdui/init.js`. `clay:documents` ops call the existing Phase 9 `open_existing_file`, `save_document`, `reload_document`, `document_metadata`, and `list_documents` helpers, while `clay:workspace` lists configured root metadata. Results are serialized as facade JSON with string IDs and sanitized workspace-relative paths. Workspace errors are converted through `WorkspaceError::diagnostic`, so traversal, unknown roots/documents, invalid UTF-8, dirty reloads, stale saves, and IO failures remain typed server validation failures instead of raw filesystem access.

Syntax grammar registration follows the same server-runtime boundary. `clay:syntax.serverRegisterSyntaxGrammar` validates first-party grammar package metadata through `op_clay_syntax_register_syntax_grammar`, reusing `assemble_package_record` before inserting into `SyntaxGrammarRegistry`. The facade/op reject executable callbacks, raw op fields, native handles, client JavaScript, non-`@clay/*` grammar packages, arbitrary/native artifact paths, raw CSS/colors, and other authority-bearing metadata. `ClayRuntimeEvaluation` exposes the runtime-local registered syntax grammar snapshot for tests; actual Tree-sitter parse/highlight work still runs later through `ParseCoordinator` as Background no-hot-path work.

Completion provider registration follows the same server-runtime boundary but is metadata-only in Phase 18.11. `clay:completion.serverRegisterCompletionProvider` validates package-shaped completion provider metadata through `op_clay_completion_register_completion_provider`, reusing `assemble_package_record` before storing `CompletionProviderMeta` snapshots in `ClayOpState` / `ClayRuntimeEvaluation`. The facade/op require `completion-provider`, package-owned provider IDs, duplicate rejection, inert trigger/word-boundary metadata, and bounded timeout/item caps. They reject `handler`, `callback`, `complete`, `function`, `module`, client JavaScript, native handles, raw ops, snippets, commands, URLs, shell/network/AI/WASM/native/package-manager authority, and any package provider execution token; `core.bufferWords` remains the executable provider until a future handler bridge is implemented.

Parse-handler registration follows the same server-runtime boundary. `clay:parse.serverRegisterParseHandler` validates package metadata and budgets through `op_clay_parse_register_parse_handler`; executable `handler`/`callback`/`onParse`/`function` keys are rejected in the facade and op. The JS facade stores the package module export behind a server-issued token in the persistent runtime, and Rust registers that token with `ParseCoordinator` under the owning runtime generation ID. Hot reload replaces same package/mode handlers with the new generation and cancels old-generation parse tasks before swap. Rust later invokes the active token through `RuntimeCommand::Parse` with the smaller of the service timeout and the handler's registered `timeoutMs`. The handler returns inert update JSON, which Rust converts to `IncrementalParseUpdate` and lets `ParseCoordinator` validate generation/document freshness before publication.

Phase 18.20 language intelligence follows the token-backed parse-handler pattern. `clay:language.serverRegisterLanguageIntelligenceProvider` stores resolver-validated module exports under `globalThis.__clayLanguageIntelligenceHandlers[token]`; `RuntimeCommand::LanguageIntelligence` invokes the handler on the persistent worker and converts returned JSON into validated analyzer-neutral results. Process authority stays separate: `clay:language-server.authorizeLanguageServer` is open only during configuration-root evaluation and seals before package load, while opaque session I/O routes to the dedicated language-server process thread so long reads do not block the Deno worker.

Phase 18.21 adds a document-analysis worker lifecycle (`src/server/document_analysis.rs`) for long-lived package analysis. Document analyzers are spawned as dedicated `JsRuntime` instances sharing `ClayOpState` via `Arc<Mutex<PackageService>>`, so they have the same permission boundary as the main runtime. Analyzer registration (`serverRegisterDocumentAnalyzer`) validates that descriptor objects carry no authority fields (executable, args, cwd, environment, handler, callback, process), requires both `parse-document` and `language-server` permissions, and uses the package `apiPrefix` namespace. Lossless byte transport ops (`op_clay_language_server_send_bytes`/`read_bytes`) were added to the Clay op extension. A dynamic completion adapter (`register_completion_provider` with `runtimeBridge: true` and `exportName`) creates `JsCompletionProviderRegistration` entries stored in `ClayOpState` for production completion routing through the persistent runtime worker.

Key binding registration follows the same server-runtime boundary. `clay:keybindings` ops parse and validate single key chords, scopes, and allowlisted command IDs, then compile registrations into a versioned `BehaviorManifest` through `ActiveBehaviorManifest::publish_replacement`. `clay:behavior` ops expose summaries and routes for the active runtime manifest. The client still receives and routes inert manifests; no JavaScript handler is installed for keypresses.

Runtime error reporting is intentionally narrow and sanitized. `ClayRuntimeError::diagnostic` maps syntax errors, invalid imports, configuration module denials, op validation failures, SDUI validation failures, document/workspace validation failures, keybinding command denials, timeouts, and heap-limit termination into `RuntimeDiagnostic { severity, code, message }`. Diagnostic messages use stable Clay error codes and generic safe detail instead of raw source snippets, environment dumps, tokens, or absolute local paths. `IpcServer` stores diagnostics produced during default configuration loading or runtime-produced SDUI/behavior application; `handle_connection` publishes current diagnostics as `ServerMessage::RuntimeDiagnostic` after bootstrap snapshots so connected clients can update GUI status asynchronously.

The runtime is created with a server-owned V8 heap limit from [`JS_RUNTIME_HEAP_LIMIT_BYTES`](../../../src/perf/budgets.rs) using `v8::Isolate::create_params().heap_limits(...)` through `deno_core::RuntimeOptions::create_params`. A near-heap-limit callback records that the heap guard fired, calls `terminate_execution()`, and lets Rust map the failure to `ClayRuntimeError::HeapLimit` / `runtime.heap_limit` with sanitized text. The heap limit is not exposed through `init.js` or any Clay configuration API.

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
assert_eq!(error.diagnostic().code, "runtime.timeout");
```

## Invariants and Constraints

- JavaScript execution is server-side only; the native Rust client does not run arbitrary JavaScript.
- Runtime lifetime is per server/configuration generation; Phase 19 hot reload replaces the service through `RuntimeGenerationStore`, while ordinary evaluation reuses the active generation's worker-owned `JsRuntime`.
- Runtime evaluation is startup/configuration/open-time work and must not be called from Masonry paint, text-event handling, or ordinary client-first typing.
- The runtime does not grant network, shell, package loading, direct client filesystem, WASM, or AI mutation authority; the document/workspace facade subset can only use server-configured workspace roots through existing server validation.
- Import support is deny-by-default except for the configuration entry point, curated `clay:*` facades, canonicalized relative `.js` modules below the configuration root, and the vendored `markdown-it` shim used by the first-party Markdown package.
- Runtime diagnostics must preserve safe detail only: stable code, severity, and actionable generic message; no raw absolute paths, source snippets, secrets, tokens, or capability-bearing handles.
- Every evaluation has a hard timeout; a runaway module is terminated and surfaced as `runtime.timeout` instead of hanging the server.
- The persistent runtime has a hard V8 heap limit; heap growth is terminated and surfaced as `runtime.heap_limit` without source text, paths, tokens, or package internals.
- Timeout or heap-limit termination stops the worker; later controlled evaluations run on a fresh worker, while parse-handler recovery stays generation-scoped through `RuntimeGenerationStore` and `ParseCoordinator`.
- Runtime facades may call Clay-owned ops internally, but user configuration should import facade functions and must not depend on raw `Deno.core.ops.op_*` names.
- Typography configuration is one atomic candidate validated before mutation. It exposes fallback-stack names and logical sizes only, never installed-font discovery, font files/bytes/URLs, downloads, renderer data, or extra authority.
- `clay:syntax.serverRegisterSyntaxGrammar` is a package-load-time public facade for first-party grammar packages only; ordinary user config should use `loadPackage("@clay/<language>")` and must not copy manifests or call raw syntax ops.
- `clay:completion.serverRegisterCompletionProvider` is a package-load-time metadata facade. It records provider metadata only and rejects executable package completion handlers in Phase 18.11.
- `clay:language.serverRegisterLanguageIntelligenceProvider` registers token-backed bounded providers under `parse-document`; provider results cannot self-assert provenance or process authority.
- `clay:language-server` grant mutation is configuration-root-only and sealed before package execution; session methods expose no process or stdio handles.
- Document/workspace runtime ops are startup/configuration/server-first work. They are not invoked from client paint, text-event handling, or ordinary local edit application.
- Key binding registration compiles to inert behavior manifests. Client key routing uses installed manifests and never calls JavaScript synchronously.

## Tests

- `src/server/mod.rs`: `reload_runtime_generation_swaps_only_after_successful_configuration_load`, `successful_reload_refreshes_open_documents_without_full_snapshots`, and `failed_reload_keeps_previous_runtime_generation_active` verify generation ID changes, fresh service state after success, open-document refresh through generic mode activation, no full-text snapshot refresh frames, stale service retention after failure, and sanitized diagnostics.
- `tests/persistent_runtime_hot_reload.rs`: `developer_hot_reload_trigger_reports_success_and_sanitized_failure` verifies the non-GUI developer trigger reports success, returns sanitized failure diagnostics, and keeps the previous generation active after failure.
- `src/server/js_runtime/tests.rs`: 198 passing unit tests plus one ignored manual resource probe cover persistent evaluation, trust-domain separation/replay, exact helper-export loading, package load/activation, manifest/keybinding APIs, parse/completion/language-intelligence bridges, editor-layout configuration, syntax ownership, timeout/heap recovery, and sanitized failures. `cargo test --lib server::js_runtime::tests -- --test-threads=1` is the focused move/regression command.
- `src/server/js_runtime/mod.rs`: owns only runtime implementation and `#[cfg(test)]` service inspection helpers; it includes the sibling module with `#[cfg(test)] mod tests;` and does not contain the test mass.
- `src/client/mod.rs::tests::selected_file_edit_then_save_persists_and_reports_clean` starts a real Unix IPC server with a `Ctrl+S` configuration overlay, opens and activates a selected Rust file, verifies mode activation preserved the save binding, queues edit then save, and checks both clean `DocumentSaved` metadata and persisted bytes.
- `src/server/connection/mod.rs`: `client_receives_js_generated_sdui_snapshot` verifies a runtime-generated tree stored in server SDUI state is emitted as the bootstrap `SduiSnapshot`; `server_sends_runtime_diagnostics_after_bootstrap` verifies stored diagnostics are published after bootstrap.
- `src/server/js_runtime/tests.rs`: typography transaction/rejection tests verify complete replacement and no raw authority fields; `src/server/mod.rs::typography_update_reaches_connected_clients_once` verifies a changed configuration emits one bounded live server update.
- `src/server/document_analysis.rs`: 6 unit tests covering worker lifecycle, open/change/close/reset/completion/intelligence/shutdown flows, stale output rejection, grant revocation, oversize document rejection, mailbox coalescing, and root/generation cancellation.
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

- [First-Party LSP Bridge Packages](first-party-lsp-bridge-packages.md)
- [Configuration Runtime](configuration-runtime.md)
- [Behavior Runtime Registration](behavior-runtime-registration.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Language Intelligence](language-intelligence.md)
- [Language Server Process Service](language-server-process-service.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
- `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`
- `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`
