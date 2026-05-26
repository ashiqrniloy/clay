# Developer-Friendly Launch and GUI Smoke Testing

## Objectives
- Make Clay's Windows and Unix developer launch paths usable without manually choosing IPC endpoints such as named pipes or socket paths.
- Treat `cargo run`, `cargo run -- server`, and `cargo run -- client` as first-class validation paths for the current app.
- Add an app-managed GUI smoke mode that creates an isolated local endpoint, starts a child server, launches the GUI client, and cleans up without requiring manual pipe/socket handling.
- Surface client/server connection, editable/read-only access, fallback, and disconnection state in the GUI instead of requiring stderr inspection.
- Preserve server-authoritative documents, bounded IPC queues, shell-free process startup, and transport-specific security boundaries.

## Expected Outcome
- A developer can validate the current implementation with simple commands:
  - `cargo run`
  - `cargo run -- server`
  - `cargo run -- client`
  - `cargo run -- smoke-gui` or the final command name chosen during implementation
- Bare `cargo run` starts or reuses the platform default local server and opens a connected GUI client with visible status.
- `cargo run -- server` starts a foreground server on the platform default endpoint and reports readiness or an actionable bind/connect error.
- `cargo run -- client` connects to the platform default server and clearly indicates connected, read-only, disconnected, or local-fallback state.
- The smoke/dev command internally creates an isolated local endpoint, starts a managed child server, waits for readiness, opens the GUI client, and terminates the child server when the GUI exits.
- GUI smoke validation covers ordinary editing, server edit acknowledgements, editable/read-only lease state, second-client observer behavior, connection loss/fallback state, and resync/manifest application where currently implemented.
- Windows named pipes and Unix sockets remain local IPC transports; no remote TCP listener, shell-mediated startup, or user-managed endpoint is required for normal smoke testing.

## Tasks

- [x] Define supported launch modes and CLI behavior
  - Acceptance Criteria:
    - Functional: `cargo run`, `cargo run -- server`, `cargo run -- client`, and an app-managed GUI smoke command have explicit semantics, expected output, and failure behavior on Windows and Unix.
    - Performance: CLI parsing and endpoint selection remain simple argument/default derivation and do not perform blocking IPC on the GUI input or paint paths.
    - Code Quality: Launch mode parsing remains centralized and testable in `src/main.rs`; endpoint formatting remains behind `src/ipc.rs` helpers.
    - Security: Default and smoke endpoints remain local only; background startup continues to use direct `Command` arguments and never invokes a shell.
  - Approach:
    - Documentation Reviewed:
      - `plans/011-Windows-Platform-Support.md`: Windows transport is complete, but manual GUI smoke testing was not executed and manual endpoint handling remains a UX gap.
      - `docs/development/windows.md`: Current manual smoke test still documents explicit server/client commands and endpoint details that should become unnecessary for normal validation.
      - `docs/wiki/flows/client-server-edit-ack.md`: Documents bare `clay` auto-start, endpoint defaults, edit ack logging, and the current limitation that auto-started servers outlive the client.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Preserve server ownership of canonical state and leases.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep IPC and acknowledgement work off Masonry input/paint paths.
    - Options Considered:
      - Keep manual endpoint instructions: no code work, but leaves Windows GUI validation awkward and error-prone.
      - Only improve docs around default endpoints: smaller, but still lacks isolated smoke testing and child cleanup.
      - Add explicit launch modes with tested behavior: clearer developer UX and easier regression validation.
    - Chosen Approach:
      - Extend the existing `ClayCommand` parser with a named smoke/dev mode while preserving current `server`, `client`, auto, and explicit endpoint behavior for advanced debugging.
    - API Notes and Examples:
      ```powershell
      cargo run
      cargo run -- server
      cargo run -- client
      cargo run -- smoke-gui
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Add/adjust command enum variants, parser tests, help/error text, and mode dispatch.
      - `src/ipc.rs`: Add helpers only if unique smoke endpoint generation needs to be centralized.
      - `docs/development/windows.md`: Replace manual named-pipe smoke instructions with command-first validation.
    - References:
      - `src/main.rs`
      - `src/ipc.rs`
      - `docs/development/windows.md`
      - `docs/wiki/flows/client-server-edit-ack.md`
  - Test Cases to Write:
    - `parses_smoke_gui_subcommand`: The smoke/dev command selects an app-managed launch mode.
    - `parses_default_launch_modes`: Existing server/client/auto parser behavior remains stable.
    - `launch_modes_do_not_require_manual_endpoint`: Default command variants carry platform defaults when no endpoint is supplied.
    - `rejects_extra_cli_arguments`: Endpoint-taking launch modes fail with usage text when extra positional arguments are supplied.
  - Verification:
    - `cargo fmt --check`
    - `cargo test --bin clay --quiet`

- [ ] Add managed smoke endpoint generation and child server lifecycle
  - Acceptance Criteria:
    - Functional: The smoke/dev command creates a unique local endpoint, launches a child `clay server <endpoint>`, waits for readiness, opens the GUI client, and terminates the child server when the GUI exits.
    - Performance: Readiness waiting is bounded and asynchronous around startup only; no per-key or per-frame work is added.
    - Code Quality: Child process ownership is represented by a small lifecycle type with deterministic cleanup and targeted tests for command construction.
    - Security: The child server is launched through `std::process::Command` with direct arguments, inherited or controlled stdio, no shell, and only a local endpoint.
  - Approach:
    - Documentation Reviewed:
      - Context7 Winit docs for cross-thread/event-loop communication: `EventLoopProxy` can wake the event loop from background work; GUI updates should be marshalled back to the event loop instead of mutating UI from background tasks.
      - Rust standard library process API used in existing `background_server_command` in `src/main.rs`.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer deterministic checks over instruction-only maintenance.
    - Options Considered:
      - Reuse the platform default endpoint for smoke mode: simple, but can collide with a developer's background server and hide lifecycle bugs.
      - Generate an isolated endpoint per smoke run: avoids collisions and makes cleanup/diagnostics deterministic.
      - Add a separate external test harness binary: useful later, but the requested UX is direct `cargo run` commands.
    - Chosen Approach:
      - Add an internal smoke endpoint helper using process ID plus a short monotonic/atomic suffix where needed. Start a managed child server with that endpoint and hold the child handle for shutdown after `run_editor` returns.
    - API Notes and Examples:
      ```rust
      let endpoint = IpcEndpoint::smoke_test_endpoint("gui")?;
      let mut server = ManagedServer::spawn(std::env::current_exe()?, &endpoint)?;
      wait_for_server_ready(&endpoint).await?;
      run_editor(editor_widget)?;
      server.shutdown();
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Add `ManagedServer`, unique smoke endpoint generation call site, readiness wait, and cleanup on normal GUI exit.
      - `src/ipc.rs`: Add `unique_local_endpoint`/`smoke_endpoint` helpers if platform-specific name construction belongs with endpoint modeling.
      - `docs/wiki/flows/client-server-edit-ack.md`: Document managed smoke launch and child cleanup once implemented.
    - References:
      - `src/main.rs`
      - `src/ipc.rs`
      - Context7 `/rust-windowing/winit` docs on event loop wake-up and user-event style cross-thread communication.
  - Test Cases to Write:
    - `smoke_endpoint_is_platform_local_and_unique`: Windows endpoints use `\\.\pipe\...`; Unix endpoints use socket paths under an appropriate temp/runtime directory.
    - `managed_server_command_uses_current_exe_without_shell`: Smoke startup builds a direct child-process command.
    - `managed_server_shutdown_is_called_on_gui_exit`: Lifecycle cleanup is exercised with a test double or isolated helper where practical.

- [ ] Improve server readiness, connection diagnostics, and default launch reliability
  - Acceptance Criteria:
    - Functional: Auto and smoke launch modes report clear diagnostics for server not found, server starting, endpoint already occupied, endpoint validation failure, child process exit, and handshake failure.
    - Performance: Startup retry remains bounded; ordinary editor interaction continues to use bounded queues and no blocking IPC in UI handlers.
    - Code Quality: Connection/bootstrap errors are categorized enough for actionable messages without string matching in high-level launch code.
    - Security: Diagnostics do not expose secrets; endpoint errors preserve local-only Windows pipe validation and Unix stale-socket protection.
  - Approach:
    - Documentation Reviewed:
      - `plans/011-Windows-Platform-Support.md`: Windows named-pipe busy retry and default endpoints are implemented; no CI workflow was added.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Documents client connect/bootstrap and named-pipe busy retry behavior.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documents Unix stale-socket checks and Windows local named-pipe validation.
    - Options Considered:
      - Continue logging raw errors: lowest effort, but hard to use in smoke testing.
      - Add a full health-check protocol message: robust but larger than needed before workspace/file features are complete.
      - Improve bounded connect/handshake readiness and error categories first: minimal and fits existing protocol.
    - Chosen Approach:
      - Keep readiness as a bounded `client::connect` retry for now, but wrap startup paths in clearer `LaunchError`/diagnostic messages and detect child early-exit in smoke mode.
    - API Notes and Examples:
      ```rust
      match connect_with_retry(&endpoint).await {
          Ok(session) => LaunchState::Connected(session),
          Err(error) => LaunchState::LocalFallback { reason: error.to_string() },
      }
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Add launch diagnostics, child early-exit checks, and tests.
      - `src/client/mod.rs`: Refine bootstrap error variants only if necessary for launch diagnostics.
      - `docs/development/windows.md`: Document expected messages for default and smoke launches.
    - References:
      - `src/client/mod.rs`
      - `src/server/mod.rs`
      - `docs/wiki/modules/client-snapshot-bootstrap.md`
      - `docs/wiki/modules/server-ipc-skeleton.md`
  - Test Cases to Write:
    - `connect_retry_reports_last_error`: Bounded retry returns actionable final error.
    - `client_mode_falls_back_with_status_when_server_missing`: Client mode can still open a local editor while reporting fallback.
    - `smoke_mode_fails_if_child_server_exits_before_ready`: Smoke command does not silently open an unconnected GUI when managed server startup fails.

- [ ] Route client connection events into the GUI event loop
  - Acceptance Criteria:
    - Functional: `ClientConnectionEvent` values that affect UI state are delivered to the Masonry application and applied to `EditorWidget` while the GUI is running, not only printed to stderr.
    - Performance: Background IPC tasks send bounded or coalesced UI notifications and never mutate widgets directly from Tokio tasks.
    - Code Quality: The bridge between Tokio client events and Masonry/winit events is explicit, testable, and isolated from editor text mutation logic.
    - Security: IPC data remains decoded and validated by existing protocol code before any GUI event is constructed; no raw IPC bytes enter the widget tree.
  - Approach:
    - Documentation Reviewed:
      - Context7 Winit docs: `EventLoopProxy` supports waking the event loop from background tasks; older docs describe `UserEvent(T)` sent by proxy for cross-thread communication.
      - Local `masonry_winit` 0.4.0 source: `MasonryUserEvent::Action(WindowId, ErasedAction, WidgetId)` is handled by `MasonryState::handle_user_event`, which emits an action signal consumed by `AppDriver::on_action`.
      - `src/masonry_editor.rs`: `EditorWidget::apply_connection_event` already applies resync snapshots and behavior manifest installation but is not wired into live app event delivery.
    - Options Considered:
      - Keep stderr-only event logging: insufficient for GUI smoke validation and misses live resync/status behavior.
      - Poll a channel from paint/layout handlers: violates UI hot-path constraints.
      - Use Masonry/winit user events or action forwarding: aligns with the event-loop model and keeps background tasks from touching widgets directly.
    - Chosen Approach:
      - Create an app-level event/action bridge. Prefer sending a typed app action through `masonry_winit::event_loop_runner::MasonryUserEvent::Action` if public API access is sufficient; otherwise introduce a narrow custom runner/wrapper or driver-owned channel that is drained only from event-loop callbacks.
    - API Notes and Examples:
      ```rust
      #[derive(Debug)]
      enum AppEvent {
          ClientConnection(ClientConnectionEvent),
      }

      // Background task sends AppEvent through an EventLoopProxy or driver bridge.
      // Driver applies the event to EditorWidget/Editor app state on the GUI thread.
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Create event bridge, store app/window/widget identifiers, and dispatch connection events to GUI state.
      - `src/masonry_editor.rs`: Expose a small action/state update path if direct widget mutation through driver context requires it.
      - `src/client/mod.rs`: No protocol changes expected; event receiver usage moves from stderr-only logging to GUI dispatch.
      - `docs/wiki/flows/client-server-edit-ack.md`: Document live GUI event routing.
    - References:
      - Context7 `/rust-windowing/winit` EventLoopProxy docs.
      - Local `masonry_winit` 0.4.0 `event_loop_runner.rs` and `app_driver.rs`.
      - `src/masonry_editor.rs`
  - Test Cases to Write:
    - `resync_event_replaces_editor_snapshot`: Existing widget-level test remains passing.
    - `connection_event_action_is_dispatched_to_driver`: App bridge forwards a connection event into the driver path.
    - `background_event_bridge_does_not_block_on_full_gui_queue`: If a bounded queue is used, full-queue behavior remains non-blocking or coalesced.

- [ ] Add visible connection, access, and synchronization status to the GUI
  - Acceptance Criteria:
    - Functional: The window title, status line, or editor chrome visibly communicates Local Fallback, Connecting, Connected Editable, Connected Read-only, Disconnected, and latest known document/version state.
    - Performance: Status rendering is cheap and piggybacks on normal render requests; it does not force full-document layout or IPC round trips.
    - Code Quality: Status state is separate from the rope/text model and can be tested independently from text editing behavior.
    - Security: Status UI does not expose sensitive local paths or secrets; endpoint display is concise and safe for diagnostics.
  - Approach:
    - Documentation Reviewed:
      - `src/editor/surface.rs`: Editor document state already records `DocumentAccess` and versions.
      - `src/masonry_editor.rs`: Widget owns `EditorSurface`, applies snapshots/manifests, and currently exposes accessibility labels.
      - `docs/wiki/flows/client-server-edit-ack.md`: Current observability relies on stderr edit ack logging.
    - Options Considered:
      - Keep status in stderr: inadequate for manual GUI smoke tests.
      - Update only the window title: small, but less flexible for multi-state diagnostics.
      - Add a simple in-window status overlay/line plus optional title update: visible, testable, and useful for screenshots.
    - Chosen Approach:
      - Add a minimal status model to the editor widget/app driver and render it as a small status line. Include document access and latest confirmed/server version where available.
    - API Notes and Examples:
      ```text
      Clay — Connected — Editable — v12
      Clay — Connected — Read-only Observer — v12
      Clay — Local Fallback — No Server
      Clay — Disconnected
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs`: Add status state, update methods, painting/accessibility text, and tests.
      - `src/editor/surface.rs`: Expose document version/access read-only data if needed.
      - `src/main.rs`: Initialize status based on launch mode and update it from connection events.
      - `docs/wiki/flows/client-server-edit-ack.md`: Document visible status states.
    - References:
      - `src/masonry_editor.rs`
      - `src/editor/surface.rs`
      - `src/protocol/mod.rs` `DocumentAccess`
  - Test Cases to Write:
    - `status_reflects_connected_editable_initial_state`: Initial connected client displays editable/access state.
    - `status_reflects_read_only_observer`: Second client/read-only snapshot displays observer state.
    - `status_updates_after_edit_ack_or_resync`: Version/status changes when relevant connection events are applied.
    - `status_reflects_local_fallback_when_no_server`: Missing server fallback is visible in GUI state.

- [ ] Make second-client and default-command GUI smoke testing endpoint-free
  - Acceptance Criteria:
    - Functional: A developer can run one default foreground server and two default clients without specifying an endpoint; the first client is editable and the second is read-only/observer in the GUI.
    - Performance: Multiple clients do not serialize unrelated GUI work beyond existing server document lease constraints.
    - Code Quality: Smoke instructions are command-based and do not rely on hidden environment variables or manual endpoint copying.
    - Security: All clients use the same local platform default endpoint and do not open remote listeners.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/flows/client-server-edit-ack.md`: First client gets editable lease; later clients are read-only observers.
      - `plans/006-Phase5-Versioned-Text-Synchronization-and-Leases.md`: Documents lease and read-only observer behavior.
      - `plans/011-Windows-Platform-Support.md`: Windows named pipe tests already validate second-client read-only behavior at the transport level.
    - Options Considered:
      - Keep second-client validation automated-only: leaves a visible GUI behavior unverified by the requested manual smoke path.
      - Add special CLI for second client: unnecessary because default client mode should already cover it.
      - Make default server/client paths robust and visibly state lease status: best matches desired developer workflow.
    - Chosen Approach:
      - Ensure no code path requires endpoint arguments for the second-client scenario and update status UI/docs so read-only observer behavior is obvious.
    - API Notes and Examples:
      ```powershell
      cargo run -- server
      cargo run -- client
      cargo run -- client
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Adjust default client/server diagnostics if necessary.
      - `src/masonry_editor.rs`: Display read-only observer status.
      - `docs/development/windows.md`: Replace custom-pipe second-client instructions with default commands.
    - References:
      - `src/server/document.rs`
      - `src/client/mod.rs`
      - `docs/wiki/flows/client-server-edit-ack.md`
  - Test Cases to Write:
    - `default_server_and_clients_use_same_platform_endpoint`: Parser/default endpoint behavior is stable.
    - `read_only_status_applies_from_initial_snapshot`: Read-only access from server appears in widget status.
    - Manual smoke checklist: Run default server plus two default clients and verify first editable/second read-only with no endpoint argument.

- [ ] Update developer documentation for command-first GUI smoke validation
  - Acceptance Criteria:
    - Functional: Windows and general development docs explain the simple launch commands, expected GUI status states, and when advanced endpoint arguments are only needed for debugging.
    - Performance: Documentation preserves the expectation that GUI typing is local/optimistic and does not wait on IPC acknowledgements.
    - Code Quality: Docs link to implementation wiki pages for launch/event routing details and avoid duplicating internal code-level explanations.
    - Security: Docs explicitly state that smoke/default launch uses local named pipes or Unix sockets, direct child-process arguments, and no shell-mediated IPC or remote TCP listener.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/windows.md`: Current validation commands and manual smoke section.
      - `docs/index.md`: Developer guide index.
      - `docs/wiki/flows/client-server-edit-ack.md`: Implementation-level launch and event flow docs.
    - Options Considered:
      - Add only a short note to the Windows doc: quick but insufficient once new launch mode exists.
      - Update the Windows doc, docs index, and wiki links together: keeps public/developer and internal docs aligned.
    - Chosen Approach:
      - Rewrite the manual smoke section around command-first validation and move explicit pipe/socket usage into an advanced debugging note.
    - API Notes and Examples:
      ```powershell
      cargo run
      cargo run -- smoke-gui
      cargo run -- server
      cargo run -- client
      ```
    - Files to Create/Edit:
      - `docs/development/windows.md`: Update validation and manual GUI smoke sections.
      - `docs/index.md`: Update link text if needed.
      - `docs/wiki/flows/client-server-edit-ack.md`: Link from docs to implementation details.
    - References:
      - `docs/development/windows.md`
      - `docs/index.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - `rg` documentation check: docs contain command-first smoke instructions and no longer require manual `\\.\pipe` examples for normal smoke testing.
    - Manual doc review: Advanced endpoint usage is clearly marked optional/debug-only.

- [ ] Verify launch, GUI smoke, IPC, and regression behavior
  - Acceptance Criteria:
    - Functional: Automated checks and manual GUI smoke cover default auto-start, managed smoke, foreground server/client, second-client read-only behavior, edit acknowledgement, and local fallback.
    - Performance: Existing hot-path tests still prove editor input does not await IPC queue capacity or server acknowledgements.
    - Code Quality: Tests are deterministic, cfg-gated where platform-specific, and do not depend on a developer's existing default server state.
    - Security: Tests avoid remote networking and verify shell-free child server startup remains intact.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Use deterministic checks for maintained artifacts and actionable failure modes.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Preserve non-blocking editor behavior and bounded IPC validation.
      - `plans/011-Windows-Platform-Support.md`: Existing Windows named-pipe automated validation commands.
    - Options Considered:
      - Rely on manual GUI smoke only: insufficient for regressions.
      - Add only unit tests: misses command UX and visible GUI expectations.
      - Combine targeted unit/integration tests with a documented manual smoke checklist: practical for native GUI behavior.
    - Chosen Approach:
      - Add deterministic parser/lifecycle/status/bridge tests and keep manual GUI smoke as the final interactive validation step.
    - API Notes and Examples:
      ```powershell
      cargo fmt --check
      cargo test --all-targets
      cargo check --target x86_64-pc-windows-msvc --all-targets
      cargo run -- smoke-gui
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Parser, command construction, readiness, and bridge tests.
      - `src/masonry_editor.rs`: Status and event-application tests.
      - `src/client/mod.rs`: Existing IPC tests reused; add tests only if error categorization changes.
      - `docs/development/windows.md`: Manual smoke checklist updated after implementation.
    - References:
      - `plans/011-Windows-Platform-Support.md`
      - `docs/development/windows.md`
      - `src/main.rs`
      - `src/masonry_editor.rs`
  - Test Cases to Write:
    - `cargo fmt --check`: Formatting is stable.
    - `cargo test --all-targets`: Native target tests pass.
    - `cargo check --target x86_64-pc-windows-msvc --all-targets`: Windows MSVC compilation still passes.
    - `cargo test --bin clay smoke`: CLI/lifecycle smoke-related tests pass.
    - Manual GUI smoke: Run `cargo run -- smoke-gui`, type text, observe Connected/Editable status and edit acknowledgements/status updates without manual endpoint input.
    - Manual default commands: Run `cargo run -- server`, `cargo run -- client`, and a second `cargo run -- client`; verify first editable and second read-only.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The implementation is reviewed for new behavior-changing settings or launch customization, and any intentional user-configurable behavior is exposed through documented Clay JS configuration APIs or explicitly kept CLI/internal.
    - Performance: Configuration review does not add runtime config loading to text input, rendering, Masonry paint/layout, or IPC frame hot paths.
    - Code Quality: If configuration is added, it uses `~/.config/clay/init.js` conventions and documentation/registry coverage rather than undocumented environment variables.
    - Security: Configuration does not grant filesystem, network, shell, extension loading, AI mutation, workspace, WASM, or remote listener authority beyond existing local IPC behavior.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Every configuration option is a Clay JS API, not an undocumented config key.
      - `.agents/skills/project-patterns/references/configuration-system.md`: Configuration surfaces should be intentional and documented.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Public programmatic behavior must be documented and registry-backed.
    - Options Considered:
      - Make smoke endpoint naming configurable: likely unnecessary; smoke endpoints should be internal and isolated.
      - Keep launch/smoke behavior as CLI/internal process behavior: preferred unless user-facing configuration requirements emerge.
    - Chosen Approach:
      - Review final implementation. Add Clay JS configuration docs/registry coverage only if user-configurable launch or endpoint behavior is intentionally introduced.
    - API Notes and Examples:
      ```javascript
      // Only if intentionally introduced later:
      // import { configureLaunch } from "clay:configuration";
      // configureLaunch({ startupMode: "auto" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: Update only if a public configuration API is introduced.
      - `docs/index.md`: Link any new configuration API docs if needed.
      - `runtime/js/**`: Update only if configuration facade changes are required.
      - `src/main.rs` and `src/ipc.rs`: Review for undocumented behavior-changing env vars or config keys.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Configuration API coverage review: If configuration APIs are added, tests fail when docs/index/registry/custom properties are missing.
    - No-new-config review: If no configuration API is added, verify no undocumented Clay-specific env var or config key was introduced.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: All Rust public functions/types introduced or changed by this launch/smoke implementation are inventoried and either exposed through documented Clay JS APIs when they are public capabilities or kept private/`pub(crate)` when internal.
    - Performance: API review does not add JavaScript or IPC round trips to ordinary typing, rendering, launch readiness, or connection event dispatch.
    - Code Quality: Public Clay JS APIs, if any, have Markdown docs, stable IDs, searchable user-facing names, key binding metadata, custom properties, examples, authority/security notes, backing Rust paths, op wrapper paths, facade paths, and lookup tags.
    - Security: No raw `Deno.core.ops.op_*` calls become user-facing APIs, and no new API grants filesystem, network, shell, extension loading, AI mutation, workspace, WASM, or remote listener authority implicitly.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API task and coverage requirements.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Clay JS APIs are stable facades over explicit ops, not raw Rust functions.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown docs and generated registry are authoritative for public APIs.
    - Options Considered:
      - Expose launch/smoke internals as Clay JS APIs: not appropriate if they are process/testing infrastructure.
      - Keep launch lifecycle helpers private/internal and document CLI behavior: likely correct for this plan.
    - Chosen Approach:
      - Review changed visibility after implementation. Prefer private/`pub(crate)` for launch lifecycle and smoke endpoint helpers unless an intentional public Clay JS capability is added.
    - API Notes and Examples:
      ```bash
      cargo test --test rust_visibility_api_mapping --quiet
      cargo test --test clay_js_api_inventory --quiet
      cargo test --test clay_js_doc_registry --quiet
      cargo test --test clay_js_facade_layout --quiet
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Review helper visibility.
      - `src/ipc.rs`: Review any new endpoint helper visibility.
      - `src/masonry_editor.rs`: Review any new status/event APIs.
      - `docs/reference/clay-js-api/**`: Update only if a public Clay JS API is intentionally added.
      - `docs/reference/clay-js-api/api-inventory.toml`: Update internal classifications if new internal Rust surfaces are visible.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
  - Test Cases to Write:
    - `cargo test --test rust_visibility_api_mapping --quiet`: Public Rust surface mapping remains intentional.
    - `cargo test --test clay_js_api_inventory --quiet`: Clay JS inventory remains complete if APIs are added.
    - `cargo test --test clay_js_doc_registry --quiet`: Generated registry remains current if docs change.
    - `cargo test --test clay_js_facade_layout --quiet`: Runtime JS facade layout remains valid.

- [ ] Update or verify the code wiki after implementation
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
      docs/wiki/flows/<flow>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/flows/client-server-edit-ack.md`: Update launch, event routing, smoke mode, and status behavior.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Update startup/status behavior if bootstrap diagnostics change.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Update managed launch/endpoint lifecycle details if server startup behavior changes.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `docs/wiki/index.md`
      - `docs/wiki/flows/client-server-edit-ack.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
    - Wiki index check: Confirm any new wiki pages are linked from `docs/wiki/index.md`.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
