# Phase 13: Embedded JavaScript Runtime

## Objectives
- Embed a constrained server-side `deno_core` runtime so Clay can execute documented Clay JS configuration and extension entry points without running arbitrary JavaScript in the Rust client.
- Make `~/.config/clay/init.js` and local modular configuration files executable through documented Clay JS facade APIs.
- Runtime-back the planned Clay JS APIs needed to test configuration and SDUI, especially `clay:sdui` helpers and server-side SDUI publication.
- Preserve Clay's server-authoritative document, file/workspace, behavior manifest, and SDUI boundaries while keeping ordinary typing/rendering off the JavaScript and IPC hot paths.
- Add deterministic tests proving a real configuration fixture can create or mutate SDUI and that the validated SDUI reaches the client through the Phase 12 protocol path.

## Expected Outcome
- Clay can load a local `init.js` configuration file, import documented `clay:*` facade modules, and execute supported configuration/extension registration code on the server.
- A test configuration can import `clay:sdui`, construct a validated editor/sidebar UI, publish it through the server, and a client/test harness can observe the resulting `SduiSnapshot` or `SduiUpdate`.
- Planned Phase 9 file/workspace and Phase 12 SDUI facade stubs move toward explicit runtime-backed `deno_core` op wrappers where in scope for this phase.
- Key binding and behavior-manifest registrations can be compiled into inert server-issued behavior manifests, with client hot-path execution still using manifests rather than JavaScript.
- Runtime errors, validation failures, and permission denials are reported through typed server/UI diagnostics instead of panics or silent failure.
- Clay JS API docs, generated registry entries, inventory metadata, and implementation wiki pages reflect the new runtime-backed surfaces.

## Tasks

- [x] Build the server-side `deno_core` runtime boundary
  - Acceptance Criteria:
    - Functional: The server owns a `deno_core::JsRuntime` wrapper that can evaluate a controlled main module and dispatch explicit Clay ops while returning typed runtime results/errors to Rust callers.
    - Performance: Runtime evaluation runs on an isolated server task/thread boundary and is never invoked synchronously from Masonry paint, text-event handling, or ordinary client-first typing.
    - Code Quality: Runtime creation, op state, module loading, errors, and tests live in focused server modules instead of connection plumbing; public Rust functions are intentionally documented through Clay JS APIs or kept private/`pub(crate)`.
    - Security: Runtime options do not expose unsafe V8 natives/GC, arbitrary network, shell, package loading, client-side JS execution, WASM, AI mutation, or direct client filesystem authority.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 13: Embed `deno_core`, evaluate configuration, expose stable facades/ops, keep JavaScript out of the typing hot path.
      - Context7 `/websites/rs_deno_core`: `JsRuntime`, `RuntimeOptions`, `load_module`, `mod_evaluate`, `deno_core::extension!`, and op extension structure.
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`: JavaScript runs server-side; behavior and SDUI effects flow through manifests/protocol updates.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns JavaScript execution and behavior definitions; client owns native rendering/input only.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: No synchronous JavaScript/IPC path before rendering ordinary typing.
    - Options Considered:
      - Run JavaScript in the client: rejected because Clay's architecture forbids arbitrary Rust-client JavaScript execution.
      - Evaluate configuration inline in server connection handlers: simpler, but risks blocking protocol handling and mixing runtime concerns with IPC.
      - Add an isolated server runtime service: preferred because it centralizes JS execution, op state, permissions, diagnostics, and tests.
    - Chosen Approach:
      - Add a server runtime service around `deno_core::JsRuntime` with explicit op state, controlled runtime options, and async evaluation APIs. Expose only Clay-owned ops and facade modules.
    - API Notes and Examples:
      ```rust
      let mut runtime = JsRuntime::new(RuntimeOptions {
          extensions: vec![clay_runtime_extension::init_ops_and_esm()],
          unsafe_expose_natives_and_gc: false,
          ..Default::default()
      });
      let module_id = runtime.load_module("file:///home/user/.config/clay/init.js", source).await?;
      let result = runtime.mod_evaluate(module_id).await;
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime.rs`: Runtime service, runtime options, evaluation API, typed errors, and tests.
      - `src/server/ops/mod.rs`: Clay op extension module root.
      - `src/server/mod.rs`: Own or initialize the runtime service where server state is created.
      - `Cargo.toml`: Added direct `deno_error` dependency used by `deno_core` op and module-loader error boundaries.
      - `docs/wiki/modules/embedded-js-runtime.md`: Document server-side runtime boundary and invariants.
      - `docs/wiki/index.md`: Link the embedded runtime wiki page.
    - References:
      - `roadmap.md` Phase 13
      - `.agents/skills/project-patterns/references/extensions-and-ai.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - Context7 `/websites/rs_deno_core`
  - Test Cases to Write:
    - `js_runtime_evaluates_controlled_module`: Evaluates a minimal local configuration module through the runtime service.
    - `js_runtime_rejects_unsafe_or_unknown_imports`: Unknown module specifiers, URLs, and package-style imports fail with typed errors.
    - `js_runtime_errors_are_typed_not_panics`: JavaScript exceptions and op validation failures are converted to typed Rust errors.
    - `ordinary_typing_does_not_enter_js_runtime`: Existing editor hot-path tests confirm typing applies without runtime evaluation.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Implement constrained `~/.config/clay/init.js` and local module loading
  - Acceptance Criteria:
    - Functional: The server can load a configured or test-provided `init.js`, resolve allowed relative local modules, and evaluate them in deterministic order through the runtime service.
    - Performance: Configuration loading is startup/reload work and does not block ordinary editor input, paint, IPC frame decoding, or document edit acknowledgement paths.
    - Code Quality: Module resolution is isolated, canonicalized, testable, and does not depend on shell commands or process-global current-directory changes.
    - Security: Module loading is limited to the configuration directory contract; URLs, package specifiers, workspace scans, shell forms, traversal outside the config root, and implicit extension/package loading are rejected.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/configuration.md`: Configuration starts at `~/.config/clay/init.js` and may load local modular files.
      - `docs/reference/clay-js-api/configuration/load-configuration-module.md`: Planned modular loading contract and rejection cases.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration options are Clay JS APIs, not undocumented settings.
      - Context7 `/websites/rs_deno_core`: `load_module` and `mod_evaluate` handle ES module loading/evaluation and top-level await.
    - Options Considered:
      - Load only an embedded string in Phase 13: useful for tests, but insufficient to validate actual configuration files.
      - Allow Deno/npm-like resolution: too much authority before the package system and permission model exist.
      - Implement a narrow local configuration module loader: matches Phase 8 contract and enables testable runtime-backed configuration.
    - Chosen Approach:
      - Add a Clay module loader that accepts the entry point and relative local module specifiers under the config root, with fixture overrides for tests.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      import { loadConfigurationModule } from "clay:configuration";

      await loadConfigurationModule({ path: "./ui.js" });
      ```
    - Files to Create/Edit:
      - `src/server/configuration.rs`: Configuration paths, module resolution, loading state, and validation.
      - `src/server/js_runtime.rs`: Hooked the configuration loader into module loading/evaluation and added inline runtime fixture tests.
      - `src/server/mod.rs`: Load default local configuration during server startup when `~/.config/clay/init.js` exists.
      - `runtime/js/configuration.ts`: Replaced planned stubs with runtime-backed facade calls where in scope.
      - `src/server/ops/configuration.rs`: `loadConfigurationModule` and `getConfigurationState` op wrappers.
      - `docs/wiki/modules/configuration-runtime.md`: Document runtime loading flow and constraints.
      - `docs/wiki/modules/embedded-js-runtime.md`: Update runtime module-loading constraints.
      - `docs/wiki/index.md`: Link the configuration runtime wiki page.
    - References:
      - `docs/reference/clay-js-api/configuration.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - `configuration_runtime_loads_init_js_fixture`: Loads a test `init.js` from a controlled config root.
    - `configuration_runtime_loads_relative_module`: `loadConfigurationModule({ path: "./ui.js" })` loads only an allowed local file.
    - `configuration_runtime_rejects_traversal_and_urls`: `../`, absolute unauthorized paths, `http:`, `npm:`, and package specifiers fail.
    - `configuration_state_reports_entry_point_and_loaded_modules`: Runtime state is queryable through the documented configuration API.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test configuration_runtime --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Expose Clay facade modules through runtime ESM imports and explicit ops
  - Acceptance Criteria:
    - Functional: Configuration code can import supported `clay:*` modules from `runtime/js/*.ts` or generated ESM equivalents and call runtime-backed operations instead of raw `Deno.core.ops` names.
    - Performance: Facade import/evaluation happens during configuration/runtime work and does not add per-keypress or paint-time overhead.
    - Code Quality: Every runtime-backed facade function maps to a documented op wrapper and stable registry ID; unsupported planned APIs fail with clear planned/unavailable errors.
    - Security: Facades do not expose raw ops, host globals, network, shell, package loading, extension loading, workspace authority, WASM, AI mutation, or client-side script execution unless a documented permissioned API explicitly allows it.
  - Approach:
    - Documentation Reviewed:
      - `runtime/js/README.md` and `runtime/js/mod.ts`: Current facade source-tree organization.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Raw Rust/ops are not the public API; facades are.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Module/export/stable-ID naming rules.
      - Context7 `/websites/rs_deno_core`: `deno_core::extension!` can bundle ops and ESM modules.
    - Options Considered:
      - Let users call `Deno.core.ops` directly: rejected by Clay API boundary.
      - Generate ESM facades from Markdown docs immediately: attractive later, but too broad for the first runtime wiring.
      - Bundle curated facade modules into a Clay extension: fits `deno_core` and preserves documented module names.
    - Chosen Approach:
      - Define a Clay runtime extension that exposes curated ESM facade modules for `clay:configuration`, `clay:sdui`, and the subset of file/workspace/keybinding/behavior APIs implemented in this phase. Keep unsupported APIs as documented planned errors.
    - API Notes and Examples:
      ```rust
      deno_core::extension!(
          clay_runtime_extension,
          ops = [op_clay_configuration_load_module, op_clay_sdui_publish_tree],
          esm = ["configuration.js", "sdui.js"],
      );
      ```
    - Files to Create/Edit:
      - `src/server/ops/mod.rs`: Extension assembly and op exports.
      - `src/server/ops/configuration.rs`: Configuration ops.
      - `src/server/ops/planned.rs`: Shared planned/unavailable op for deferred facade APIs.
      - `src/server/ops/sdui.rs`: SDUI helper/publication ops.
      - `src/server/ops/documents.rs`: Runtime-backed Phase 9 document ops where included.
      - `src/server/ops/workspace.rs`: Runtime-backed workspace-root/document-list ops where included.
      - `runtime/js/*.ts`: Update facade calls and planned/runtime status.
      - `tests/clay_js_facade_layout.rs`: Extend facade layout checks for runtime-backed modules.
      - `docs/wiki/modules/embedded-js-runtime.md`: Document facade import resolution and op boundaries.
      - `docs/wiki/modules/clay-js-facade-skeleton.md`: Document runtime-backed SDUI helpers and planned-unavailable facade behavior.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - Context7 `/websites/rs_deno_core`
  - Test Cases to Write:
    - `runtime_imports_clay_sdui_facade`: `import { definePanel } from "clay:sdui"` resolves through the runtime module loader.
    - `runtime_facades_do_not_require_raw_ops`: Fixture code uses documented imports and no raw `Deno.core.ops` calls.
    - `unsupported_facade_returns_planned_error`: APIs not implemented in Phase 13 fail explicitly rather than silently succeeding.
    - `facade_op_mapping_matches_inventory`: Runtime-backed facade/op names match `api-inventory.toml`.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test clay_js_facade --test clay_js_facade_layout --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Runtime-back SDUI construction and publication from JavaScript
  - Acceptance Criteria:
    - Functional: A JavaScript configuration fixture can create panels, labels, buttons, lists, editor views, flex containers, and stacks through `clay:sdui`, publish the resulting tree/update, and cause the server to emit a validated `SduiSnapshot` or `SduiUpdate`.
    - Performance: JS-generated SDUI publication is bounded startup/configuration or explicit update work; ordinary editing continues to use document edit protocol messages and client-local rendering without JavaScript execution.
    - Code Quality: JS-to-Rust conversion is typed and validated at the server boundary; SDUI helper construction remains separate from native Masonry reconciliation.
    - Security: JS-generated SDUI remains inert declarative UI; action intents are server-routed commands only and cannot embed executable client script, filesystem/network/shell authority, package loading, WASM, or AI mutation.
  - Approach:
    - Documentation Reviewed:
      - `plans/013-Phase12-Server-Driven-UI.md`: Phase 12 deferred JavaScript-generated SDUI to Phase 13.
      - `src/protocol/sdui.rs`: Existing typed SDUI schema, stable node IDs, editor bindings, and update forms.
      - `src/server/sdui.rs`: Existing static tree generation and validation helpers.
      - `runtime/js/sdui.ts`: Planned facade helpers currently throwing planned-runtime errors.
      - `docs/reference/clay-js-api/sdui/*.md`: Planned public API contracts for SDUI helpers.
    - Options Considered:
      - Let each JS helper immediately publish protocol updates: simple, but makes partial trees and ordering difficult to validate.
      - Have helpers build inert JS objects and use an explicit publish/apply operation: clearer validation boundary and easier testing.
      - Replace Rust SDUI types with JSON: rejected because Phase 12 established typed Rust protocol schema and validation.
    - Chosen Approach:
      - Keep helper APIs as inert builders, add an explicit documented publication/apply API if required, and convert the JS object graph into Rust `SduiSnapshot`/`SduiUpdate` only at the server op boundary.
    - API Notes and Examples:
      ```js
      import { definePanel, defineFlex, defineEditorView, defineList, publishTree } from "clay:sdui";

      const root = defineFlex({
        direction: "row",
        children: [
          definePanel({ title: "Workspace", children: [defineList({ items: [] })] }),
          defineEditorView({ documentId: 1 }),
        ],
      });

      await publishTree(root);
      ```
    - Files to Create/Edit:
      - `runtime/js/sdui.ts`: Runtime-backed helpers and explicit `publishTree` API for testing.
      - `src/server/ops/sdui.rs`: SDUI op wrappers and JS value validation/conversion.
      - `src/server/ops/mod.rs`: Store the last validated JavaScript-published SDUI tree in runtime op state.
      - `src/server/js_runtime.rs`: Return a published SDUI tree in `ClayRuntimeEvaluation`.
      - `src/server/sdui.rs`: Accept validated JS-generated trees alongside static defaults.
      - `src/server/mod.rs`: Apply default configuration SDUI publication to server SDUI state during startup.
      - `src/server/connection.rs`: Publish JS-generated SDUI through existing snapshot flow when runtime state changes.
      - `src/protocol/sdui.rs`: No wire-schema changes needed.
      - `docs/wiki/modules/embedded-js-runtime.md`: Document runtime publication capture.
      - `docs/wiki/modules/server-driven-ui.md`: Document JavaScript-generated SDUI validation/publication.
      - `docs/wiki/modules/clay-js-facade-skeleton.md`: Document runtime-backed `publishTree` behavior.
      - `docs/wiki/index.md`: Update module summaries.
      - `tests/fixtures/configuration/sdui-init.js`: Fixture that builds/publishes a test UI.
    - References:
      - `plans/013-Phase12-Server-Driven-UI.md`
      - `docs/reference/clay-js-api/sdui/*.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `configuration_can_publish_sdui_snapshot`: A fixture `init.js` uses `clay:sdui` and the server stores/emits the resulting snapshot.
    - `client_receives_js_generated_sdui_snapshot`: Client/test harness receives a typed Phase 12 `SduiSnapshot` generated from JS configuration.
    - `js_generated_sdui_rejects_unknown_document_binding`: Invalid editor bindings fail safely.
    - `js_generated_sdui_rejects_executable_action_payloads`: Action intents cannot include executable code or unknown command authority.
    - `js_generated_sdui_update_preserves_editor_state`: Updating a side panel from JS does not reset document text/caret/version state.
    - `js_generated_sdui_payloads_stay_within_phase12_budgets`: Representative JS-generated snapshot/update sizes stay under documented budgets.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test client_receives_js_generated_sdui_snapshot --quiet` passed.
    - `cargo test clay_js_facade --test clay_js_facade_layout --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Runtime-back the Phase 9 file/workspace facade subset needed by configuration
  - Acceptance Criteria:
    - Functional: Supported `clay:documents` and `clay:workspace` APIs have explicit op wrappers for server-owned open/save/reload/status/list/root-metadata behavior selected for Phase 13.
    - Performance: File/workspace operations remain server-first asynchronous work and are never part of client paint/text-event hot paths or ordinary local edit application.
    - Code Quality: Ops reuse existing Phase 9 server validation/state instead of duplicating path, root, dirty-state, lease, or document registry logic.
    - Security: Direct client filesystem authority is not introduced; workspace roots, path traversal, special files, invalid UTF-8, and persistence errors remain server-validated and typed.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 13: Wire documented Phase 9 file/workspace facades for opening, saving, reloading, listing, querying documents, and workspace-root metadata.
      - `docs/reference/clay-js-api/documents/*.md` and `docs/reference/clay-js-api/workspace/server-list-workspace-roots.md`: Planned public file/workspace contracts.
      - `src/server/workspace.rs` and `src/server/document.rs`: Existing Phase 9 server-owned file/workspace behavior.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server owns file/workspace authority.
    - Options Considered:
      - Runtime-back every documented API immediately: desirable, but risks bloating Phase 13 beyond the SDUI/config testing goal.
      - Runtime-back only SDUI and defer all document ops: limits useful configuration tests and leaves Phase 13 roadmap expectations unmet.
      - Runtime-back the documented subset needed for configuration and SDUI examples, leaving advanced flows documented as planned if needed.
    - Chosen Approach:
      - Implement explicit op wrappers over existing server helpers for the Phase 9 APIs needed by startup configuration and SDUI examples, with tests and docs marking exact runtime-backed status.
    - API Notes and Examples:
      ```js
      import { serverListDocuments } from "clay:documents";
      import { serverListWorkspaceRoots } from "clay:workspace";

      const documents = await serverListDocuments();
      const roots = await serverListWorkspaceRoots();
      ```
    - Files to Create/Edit:
      - `src/server/ops/documents.rs`: Added document op wrappers for open/save/reload/status/list over existing server workspace state.
      - `src/server/ops/workspace.rs`: Added workspace-root metadata op wrapper.
      - `src/server/ops/mod.rs`: Registered document/workspace ops and shared `WorkspaceState` in runtime op state.
      - `src/server/js_runtime.rs`: Wired runtime ESM facades to document/workspace ops, added workspace-aware configuration evaluation, and added runtime facade tests.
      - `src/server/mod.rs`: Passed server workspace state into default configuration evaluation.
      - `src/server/workspace.rs`: Added workspace-root metadata listing for runtime/API use.
      - `runtime/js/documents.ts`: Runtime-backed facade calls for implemented subset.
      - `runtime/js/workspace.ts`: Runtime-backed facade call for implemented subset.
      - `docs/reference/clay-js-api/documents/*.md`: Updated runtime status for implemented APIs.
      - `docs/reference/clay-js-api/workspace/server-list-workspace-roots.md`: Updated runtime status for implemented API.
      - `docs/generated/clay-js-api-registry.json`: Regenerated after reference doc updates.
      - `docs/wiki/modules/embedded-js-runtime.md`: Documented workspace-backed runtime op state and boundaries.
      - `docs/wiki/modules/server-file-workspace.md`: Documented runtime-op reuse of workspace validation.
      - `docs/wiki/modules/clay-js-facade-skeleton.md`: Documented runtime-backed document/workspace subset.
      - `tests/fixtures/configuration/file-workspace-init.js`: Fixture exercising document/workspace APIs.
    - References:
      - `plans/010-Phase9-File-and-Workspace-Server.md`
      - `docs/reference/clay-js-api/documents/*.md`
      - `docs/reference/clay-js-api/workspace/server-list-workspace-roots.md`
  - Test Cases to Write:
    - `document_facade_open_status_list_round_trip`: Runtime facade calls existing server document behavior.
    - `workspace_roots_facade_reports_authorized_roots`: Runtime facade exposes root metadata without direct client filesystem access.
    - `document_facade_rejects_unauthorized_paths`: Path traversal/outside-root attempts fail with existing typed errors.
    - `file_workspace_facades_do_not_block_editor_hot_path`: Existing editor responsiveness tests remain independent of file/workspace ops.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo run --bin update-doc-registry` regenerated `docs/generated/clay-js-api-registry.json`.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test clay_js_facade --test clay_js_facade_layout --quiet` passed.
    - `cargo test workspace:: --lib --quiet` passed.
    - `cargo test --test clay_js_api_inventory --test clay_js_doc_registry --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Compile key binding and behavior registrations into inert behavior manifests
  - Acceptance Criteria:
    - Functional: Runtime-backed key binding and behavior registration APIs can update server-owned behavior definitions and publish versioned inert behavior manifests to clients.
    - Performance: Client keypress handling uses already-installed manifests and never calls JavaScript synchronously.
    - Code Quality: Registration validation is explicit, versioned, and reuses behavior manifest structures instead of inventing a second routing model.
    - Security: Unknown commands, permission-bearing commands, malformed key chords/scopes, and attempts to execute client-side JavaScript are rejected.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`: Use manifests for predictable hot-path behavior and routing, not arbitrary client JavaScript.
      - `src/protocol/mod.rs` and `src/server/behavior.rs`: Existing behavior manifest schema and active manifest state.
      - `docs/reference/clay-js-api/keybindings/*.md`: Planned key binding configuration APIs.
      - `docs/reference/clay-js-api/behavior/*.md`: Planned behavior query APIs.
    - Options Considered:
      - Let JS key handlers run on every keypress: rejected by performance and security rules.
      - Store key binding configuration only in runtime memory: insufficient because clients need inert manifests.
      - Compile registration calls into manifest updates: fits existing architecture and testability.
    - Chosen Approach:
      - Implement runtime-backed key binding registration over server-owned behavior manifest builders, publishing atomic manifest updates when configuration changes.
    - API Notes and Examples:
      ```js
      import { bindKey } from "clay:keybindings";

      bindKey("Ctrl+S", "clay.documents.serverSaveDocument");
      ```
    - Files to Create/Edit:
      - `src/server/ops/keybindings.rs`: Added `bindKey`, `unbindKey`, and `listKeyBindings` op wrappers with chord, scope, command allowlist, and manifest-update validation.
      - `src/server/ops/behavior.rs`: Added behavior manifest summary and route query ops for the implemented subset.
      - `src/server/ops/mod.rs`: Added runtime-local `ActiveBehaviorManifest` state, keybinding/behavior op registration, and manifest update helpers.
      - `src/server/js_runtime.rs`: Wired embedded `clay:keybindings` and `clay:behavior` facades, returned changed behavior manifests from evaluation, and added runtime tests.
      - `src/server/mod.rs`: Applies runtime-produced behavior manifests to the server active manifest during default configuration loading.
      - `src/server/behavior.rs`: Removed obsolete dead-code lint expectations now that manifest access/replacement are used by runtime plumbing.
      - `runtime/js/keybindings.ts`: Runtime-backed facade calls.
      - `runtime/js/behavior.ts`: Runtime-backed facade calls for implemented subset.
      - `docs/wiki/modules/behavior-runtime-registration.md`: Documented JS-to-manifest flow.
      - `docs/wiki/modules/behavior-manifests.md`: Documented runtime registration integration.
      - `docs/wiki/modules/embedded-js-runtime.md`: Documented behavior manifest op state and invariants.
      - `docs/wiki/index.md`: Linked the behavior runtime registration wiki page.
    - References:
      - `.agents/skills/project-patterns/references/behavior-manifests.md`
      - `docs/reference/clay-js-api/keybindings/*.md`
  - Test Cases to Write:
    - `configuration_bind_key_updates_behavior_manifest`: Runtime `bindKey` produces a new manifest with the configured route.
    - `configuration_unbind_key_updates_behavior_manifest`: Runtime `unbindKey` removes the route atomically.
    - `unknown_command_binding_is_rejected`: Binding to undocumented/unknown command IDs fails.
    - `keypress_routing_uses_manifest_not_js`: Client key routing remains manifest-based and independent of JS runtime availability.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Report runtime errors and diagnostics in server/client UI state
  - Acceptance Criteria:
    - Functional: JavaScript syntax errors, thrown exceptions, invalid imports, permission denials, op validation errors, and SDUI/configuration validation failures are surfaced as typed diagnostics visible to server logs/tests and client UI status where practical.
    - Performance: Diagnostic publication is asynchronous and does not block input/rendering hot paths.
    - Code Quality: Diagnostic types are structured, testable, and avoid leaking sensitive absolute paths or source snippets beyond documented safe detail.
    - Security: Diagnostics do not expose secrets, raw environment dumps, unauthorized path details, tokens, or capability-bearing handles.
  - Approach:
    - Documentation Reviewed:
      - `roadmap.md` Phase 13: Report runtime errors in the Clay UI.
      - `docs/development/launch-and-gui-smoke.md`: Existing GUI status expectations.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic validation and actionable failures.
    - Options Considered:
      - Log only to stderr: easy, but invisible to GUI/runtime tests.
      - Add a broad error protocol now: useful later, but potentially too large.
      - Add a narrow runtime diagnostic event/status integrated with existing GUI status: enough for Phase 13 validation.
    - Chosen Approach:
      - Add typed runtime diagnostic state and publish it through existing server/client event paths or a small protocol message if needed.
    - API Notes and Examples:
      ```rust
      RuntimeDiagnostic {
          severity: DiagnosticSeverity::Error,
          code: "clay.runtime.invalid_import".into(),
          message: "Only clay:* and relative local configuration modules are allowed".into(),
      }
      ```
    - Files to Create/Edit:
      - `src/server/js_runtime.rs`: Runtime diagnostics and error conversion.
      - `src/server/mod.rs`: Stores sanitized startup/runtime diagnostics produced by default configuration evaluation and runtime SDUI/behavior application.
      - `src/server/connection.rs`: Publishes stored runtime diagnostics after bootstrap snapshots.
      - `src/protocol/mod.rs`: Adds `DiagnosticSeverity`, `RuntimeDiagnostic`, and `ServerMessage::RuntimeDiagnostic`.
      - `src/protocol/codec.rs`: Round-trips runtime diagnostic messages through the existing bounded codec.
      - `src/client/mod.rs`: Adds `ClientConnectionEvent::RuntimeDiagnostic` and routes protocol diagnostics to the client event queue.
      - `src/masonry_editor.rs`: Displays runtime diagnostic code/message in the existing GUI status region.
      - `docs/development/launch-and-gui-smoke.md`: Adds runtime diagnostic smoke expectations.
      - `docs/wiki/modules/embedded-js-runtime.md`: Documents diagnostic conversion and sanitization.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Documents client diagnostic event/status flow.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documents runtime diagnostic bootstrap publication.
      - `docs/wiki/modules/protocol-codec.md`: Documents the runtime diagnostic protocol payload and codec coverage.
      - `docs/wiki/index.md`: Updates master wiki summaries for changed diagnostic/runtime pages.
    - References:
      - `roadmap.md` Phase 13
      - `docs/development/launch-and-gui-smoke.md`
  - Test Cases to Write:
    - `runtime_syntax_error_reports_diagnostic`: Syntax errors become typed diagnostics.
    - `runtime_permission_error_reports_sanitized_diagnostic`: Rejected imports/paths report sanitized messages.
    - `client_receives_runtime_diagnostic_event`: Runtime diagnostics reach the GUI event/status path if protocol publication is implemented.
    - `runtime_op_validation_error_reports_diagnostic`: Op validation errors become typed diagnostics.
    - `server_sends_runtime_diagnostics_after_bootstrap`: Stored server diagnostics publish through the bootstrap/status protocol path.
    - `runtime_diagnostic_updates_status_text`: GUI status includes runtime diagnostic code and safe message.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test runtime_diagnostic --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Verify launch/smoke behavior with runtime-backed configuration and SDUI
  - Acceptance Criteria:
    - Functional: A developer can run a documented smoke command with a fixture or local config that creates SDUI through JavaScript and observe connected/editable status plus the configured SDUI panel/editor layout.
    - Performance: Smoke validation confirms startup/configuration/runtime work is separate from ordinary typing, paint, and edit acknowledgement paths.
    - Code Quality: Automated tests cover runtime, configuration loading, op wiring, SDUI publication, behavior manifest updates, docs registry, and GUI event routing; manual smoke covers native visual observation.
    - Security: Smoke mode remains local IPC only with no remote TCP listener, shell-mediated startup, package install, network fetch, direct client filesystem authority, arbitrary client JS, WASM, or AI mutation.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md`: Existing smoke commands and SDUI expectations.
      - `plans/013-Phase12-Server-Driven-UI.md`: Phase 12 smoke validated static SDUI and GUI event bridge.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Combine automated checks with documented manual GUI validation where visual observation is required.
    - Options Considered:
      - Rely only on unit tests: misses launch/runtime wiring.
      - Add full screenshot testing now: deferred unless UI complexity exceeds current needs.
      - Add a fixture-driven smoke path and keep visual validation documented: appropriate for proving Phase 13 testability.
    - Chosen Approach:
      - Extend smoke docs and launch tests so a runtime-backed config fixture can publish SDUI during startup while preserving existing managed local server/client lifecycle.
    - API Notes and Examples:
      ```powershell
      cargo run -- smoke-gui --config-fixture runtime-sdui
      cargo test --all-targets --quiet
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Added `smoke-gui --config-fixture runtime-sdui` parsing and direct child-server fixture forwarding.
      - `src/server/mod.rs`: Added optional `ServerConfig::configuration_root` runtime configuration override used by smoke fixtures.
      - `src/server/js_runtime.rs`: Added fixture-backed runtime SDUI publication coverage.
      - `docs/development/launch-and-gui-smoke.md`: Runtime-backed SDUI smoke command and expectations.
      - `docs/wiki/flows/client-server-edit-ack.md`: Documented runtime SDUI smoke launch flow and tests.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documented config-fixture child process forwarding and tests.
      - `docs/wiki/modules/embedded-js-runtime.md`: Documented the runtime SDUI fixture path and validation.
      - `docs/wiki/index.md`: Updated summaries for changed wiki pages.
      - `tests/fixtures/configuration/runtime-sdui/init.js`: Smoke/test configuration fixture.
    - References:
      - `plans/012-Developer-Friendly-Launch-and-GUI-Smoke.md`
      - `plans/013-Phase12-Server-Driven-UI.md`
  - Test Cases to Write:
    - `smoke_launch_evaluates_runtime_config_fixture`: Managed smoke startup evaluates the config fixture.
    - `managed_server_command_forwards_config_fixture_without_shell`: Managed child server receives direct `--config-fixture runtime-sdui` arguments without shell mediation.
    - `smoke_config_fixture_publishes_runtime_sdui_snapshot`: The checked-in runtime smoke fixture imports `clay:sdui` and publishes a validated panel/editor tree.
    - `smoke_launch_routes_sdui_events_to_gui`: Runtime-generated SDUI reaches the GUI event bridge.
    - Manual smoke: Runtime-backed smoke command is documented for visual validation of configured SDUI panel/editor render and editing acknowledgement/status updates.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo test smoke --quiet` passed.
    - `cargo test managed_server_command_forwards_config_fixture_without_shell --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Every public runtime-backed API, op, command, SDUI publication helper, configuration behavior, file/workspace operation, key binding API, and behavior query introduced or changed by this phase has a stable Clay JS facade, Markdown docs, generated registry entry, lookup coverage, and inventory mapping.
    - Performance: Docs generation and lookup checks remain offline/test-time operations and add no editor input/paint-path work.
    - Code Quality: Server-side Rust public functions introduced or changed by the runtime work are inventoried and either exposed through explicit `deno_core` op wrappers plus stable facades or made private/`pub(crate)`.
    - Security: Public docs state exact permissions and negative authority boundaries, including no client-side JavaScript, no undocumented filesystem/network/shell/package/WASM/AI authority, and server-side validation for permission-bearing APIs.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API task.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Stable Clay JS/TS facades are the public surface.
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`: Required metadata, key bindings, custom properties, and security notes.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown plus `docs/index.md` is authoritative.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Registry freshness and lookup coverage tests.
    - Options Considered:
      - Treat runtime wiring as internal only: invalid because Phase 13 turns planned public Clay APIs into executable APIs.
      - Update only inventory metadata: insufficient without Markdown docs/index/registry tests.
      - Update exact APIs implemented by the phase and keep deferred APIs explicitly planned: preserves documentation truth.
    - Chosen Approach:
      - Review the final implementation, update the docs/index/inventory/generated registry for every runtime-backed API, and keep non-implemented APIs clearly marked as planned/unavailable.
    - API Notes and Examples:
      ```powershell
      cargo run --bin update-doc-registry
      cargo test --test clay_js_doc_registry --quiet
      cargo test --test rust_visibility_api_mapping --quiet
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**/*.md`: Updated runtime status, examples, errors, permissions, op paths, and the new `clay.sdui.publishTree` API.
      - `docs/reference/clay-js-api/api-inventory.toml`: Runtime-backed facade/op mappings for configuration, SDUI, file/workspace, key binding, and behavior APIs.
      - `docs/index.md`: Master index link for `publishTree` and verified runtime-backed API docs.
      - `docs/generated/clay-js-api-registry.json`: Regenerated registry.
      - `tests/clay_js_doc_registry.rs`: Lookup/security/runtime status coverage, including `publishTree`.
      - `tests/clay_js_api_inventory.rs`: Inventory coverage for runtime-backed APIs.
      - `tests/rust_visibility_api_mapping.rs`: Public Rust/function mapping coverage.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
  - Test Cases to Write:
    - `cargo run --bin update-doc-registry`: Regenerates registry after docs changes.
    - `cargo test --test clay_js_doc_registry --quiet`: Fails for missing/stale docs, index links, generated entries, lookup, key binding, custom property, or security metadata.
    - `cargo test --test rust_visibility_api_mapping --quiet`: Fails when new server-side public Rust surfaces are not mapped or made internal.
    - Runtime status lookup: Registry distinguishes runtime-backed APIs from planned/unavailable APIs.
  - Verification:
    - `cargo run --bin update-doc-registry` passed and regenerated `docs/generated/clay-js-api-registry.json`.
    - `cargo test --test clay_js_doc_registry --test clay_js_api_inventory --test rust_visibility_api_mapping --quiet` passed.
    - `cargo fmt --check` passed.
    - `cargo test clay_js_facade --test clay_js_facade_layout --quiet` passed.
    - `cargo test --all-targets --quiet` passed.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Configuration execution through `~/.config/clay/init.js`, local module loading, key binding updates, behavior-changing SDUI settings, and any user-facing runtime options are documented as Clay JS APIs rather than undocumented settings.
    - Performance: Configuration docs/registry checks remain test-time work; runtime configuration evaluation is startup/reload work and never part of the ordinary input/rendering hot path.
    - Code Quality: Configuration APIs include user-facing names, default key bindings or empty lists, custom properties for behavior-changing settings, examples, async/return behavior, errors, and lookup tags.
    - Security: Configuration docs preserve no-authority-by-default behavior and explicitly reject implicit filesystem, network, shell, extension loading, package, AI mutation, WASM, workspace, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration starts at `~/.config/clay/init.js`; every option is a Clay JS API.
      - `docs/reference/clay-js-api/configuration.md`: Current configuration contract.
      - `docs/reference/clay-js-api/configuration/*.md`: Planned configuration entrypoint APIs.
    - Options Considered:
      - Add hidden runtime flags for configuration behavior: rejected by configuration-as-API rule.
      - Add dedicated SDUI layout settings immediately: defer unless Phase 13 introduces real user-facing layout/visibility options beyond JS tree construction.
      - Document only the executable configuration APIs implemented by Phase 13: preferred and testable.
    - Chosen Approach:
      - Update configuration docs for executable `init.js`/module-loading behavior, document any new user-facing configuration APIs, and explicitly defer dedicated SDUI panel visibility/layout preference APIs unless implemented.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js
      import { loadConfigurationModule } from "clay:configuration";
      import { bindKey } from "clay:keybindings";

      await loadConfigurationModule({ path: "./ui.js" });
      bindKey("Ctrl+S", "clay.documents.serverSaveDocument");
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Update runtime status and security notes.
      - `docs/reference/clay-js-api/configuration/*.md`: Runtime-backed docs for implemented configuration APIs.
      - `docs/reference/clay-js-api/keybindings/*.md`: Runtime-backed docs for implemented key binding configuration APIs.
      - `docs/reference/clay-js-api/sdui/*.md`: Add configuration examples if SDUI is configurable through `init.js`.
      - `docs/index.md`: Link any new configuration docs.
      - `docs/generated/clay-js-api-registry.json`: Regenerate after docs changes.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/clay-js-api/configuration.md`
  - Test Cases to Write:
    - Registry metadata check: Runtime-backed configuration APIs include required key binding/custom property/security metadata.
    - Config fixture check: `~/.config/clay/init.js` fixture can import `clay:sdui` and local modules.
    - No-hidden-setting review: Any behavior-changing runtime/configuration option is documented as a Clay JS API or explicitly kept internal.
  - Verification:
    - `cargo run --bin update-doc-registry` passed and regenerated `docs/generated/clay-js-api-registry.json`.
    - `cargo test --test clay_js_doc_registry --test clay_js_api_inventory --quiet` passed.
    - `cargo test configuration_runtime --quiet` passed.
    - `cargo test --test rust_visibility_api_mapping --quiet` passed.
    - Reviewed configuration, key binding, and SDUI publication docs for hidden runtime/configuration settings; behavior-changing surfaces remain documented as Clay JS APIs with security metadata.

- [x] Run final verification for Phase 13
  - Acceptance Criteria:
    - Functional: Runtime embedding, configuration loading, facade imports, op wrappers, JS-generated SDUI, behavior/key binding manifest updates, file/workspace facade subset, diagnostics, docs registry, and smoke validation pass.
    - Performance: Verification proves ordinary typing/rendering remains independent of synchronous JavaScript, full-document IPC, blocking file IO, package/network work, or server waits.
    - Code Quality: Formatting, all-target tests, docs registry generation/checks, inventory mapping, visibility mapping, and smoke documentation are current.
    - Security: Final checks confirm no remote TCP listener, shell startup path, raw user-facing ops, arbitrary client JavaScript, undocumented filesystem/network/package/WASM/AI authority, or direct client filesystem authority was introduced.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Deterministic validation for maintained artifacts.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Final authority, hot-path, docs-as-code, configuration, security, and phase-boundary checks.
    - Options Considered:
      - Validate only runtime modules: insufficient because Phase 13 crosses runtime/protocol/server/client/docs boundaries.
      - Run focused runtime tests plus full all-target suite and smoke: appropriate for a major architecture phase.
    - Chosen Approach:
      - Run formatting, generated registry update/checks, focused runtime/configuration/SDUI tests, all-target tests, and documented smoke validation before marking the phase complete.
    - API Notes and Examples:
      ```powershell
      cargo fmt --check
      cargo run --bin update-doc-registry
      cargo test --all-targets --quiet
      cargo run -- smoke-gui --config-fixture runtime-sdui
      ```
    - Files to Create/Edit:
      - `plans/014-Phase13-Embedded-JavaScript-Runtime.md`: Update checkboxes, verification notes, compromises, and further actions after execution.
      - `docs/development/launch-and-gui-smoke.md`: Update final smoke expectations if behavior differs from plan.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
  - Test Cases to Write:
    - Full suite: `cargo test --all-targets --quiet` passes.
    - Formatting: `cargo fmt --check` passes.
    - Registry freshness: `cargo run --bin update-doc-registry` followed by registry tests passes.
    - Runtime fixture: A JS config fixture publishes SDUI and updates behavior/key binding state without hot-path JavaScript.
    - Manual smoke: Runtime-backed smoke command renders JS-generated SDUI and editing/status behavior still works.
  - Verification:
    - `cargo fmt --check` passed.
    - `cargo run --bin update-doc-registry` passed and refreshed `docs/generated/clay-js-api-registry.json`.
    - `cargo test --test clay_js_doc_registry --test clay_js_api_inventory --test rust_visibility_api_mapping --quiet` passed.
    - `cargo test js_runtime --quiet` passed.
    - `cargo test configuration_runtime --quiet` passed.
    - `cargo test smoke --quiet` passed.
    - `cargo test managed_server_command_forwards_config_fixture_without_shell --quiet` passed.
    - `cargo test --all-targets --quiet` passed.
    - `cargo run -- smoke-gui --config-fixture runtime-sdui` launched the managed local IPC server/client, emitted the runtime-generated `SduiSnapshot` containing `Runtime Smoke Workspace` and `Runtime SDUI smoke ready`, and opened the GUI; the command was stopped by the 60-second harness timeout because the interactive window remained open for manual observation.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Verification:
    - Reviewed `docs/wiki/index.md` and confirmed every `docs/wiki/**/*.md` page is linked from the master index.
    - Reviewed Phase 13 wiki pages for embedded runtime, configuration runtime, behavior runtime registration, Clay JS facades, server-driven UI, file/workspace, protocol/runtime diagnostics, IPC/bootstrap, and runtime smoke flow.
    - Ran a link/source-reference sanity script confirming no wiki pages are missing from `docs/wiki/index.md`, no index links are broken, and referenced source/test/document paths exist.

## Compromises Made
- Runtime SDUI visual smoke remains a documented manual observation step because the interactive GUI command intentionally stays open until the developer closes it; automated coverage validates launch wiring, event routing, protocol publication, and fixture evaluation.

## Further Actions
- Low priority: Consider adding a Markdown/wiki lint command to automate the index-link and source-reference sanity checks used for the final wiki review.
