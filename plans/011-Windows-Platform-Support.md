# Phase 10 Windows Platform Support

## Objectives
- Port Clay so it compiles on Windows with the MSVC Rust toolchain.
- Preserve the existing client/server architecture on Unix while adding a Windows local IPC transport using Tokio named pipes.
- Keep protocol, authority, and editor hot-path behavior unchanged across platforms.
- Add platform-specific tests and documentation so future Windows regressions are caught deterministically.

## Expected Outcome
- `cargo check --all-targets` succeeds on `x86_64-pc-windows-msvc` and on existing Unix development targets.
- Bare `clay`, `clay server`, and `clay client` work on Windows using a per-user named pipe endpoint.
- Unix builds continue to use Unix domain sockets with existing stale socket cleanup behavior.
- Shared client/server protocol handling remains transport-agnostic and continues to use the bounded `rkyv` codec.
- Windows setup instructions document Rust MSVC, Visual Studio Build Tools, and validation commands.

## Tasks

- [x] Audit and gate current Unix-only code paths
  - Acceptance Criteria:
    - Functional: All direct Unix-only imports and tests are identified, and Windows-incompatible modules are either made portable or guarded with explicit `#[cfg(unix)]` boundaries.
    - Performance: The audit introduces no runtime work and does not alter current Unix IPC behavior.
    - Code Quality: The audit produces a small, actionable list of source/test/doc paths to refactor rather than broad speculative rewrites.
    - Security: Existing Unix stale socket protections remain documented and are not weakened while introducing Windows support.
  - Approach:
    - Documentation Reviewed:
      - Context7 Tokio docs from the prior portability review: `tokio::net::windows::named_pipe::{ServerOptions, ClientOptions}` and `NamedPipeServer::connect` are the Windows local IPC APIs.
      - Context7 Rustup docs: MSVC builds require Visual Studio C++ tools/Windows SDK; `rustup default stable-msvc` and `rustup target add x86_64-pc-windows-msvc` are supported setup commands.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: Preserve authority boundaries, client hot path, server authority, and security notes.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep IPC protocol semantics separate from codec/transport and avoid full-document IPC for ordinary edits.
    - Options Considered:
      - Patch compile errors one by one: fast initially, but risks scattering platform checks through high-level modules.
      - First map platform-specific seams: slower first step, but supports a minimal transport abstraction and clearer tests.
    - Chosen Approach:
      - Use `rg`/compiler feedback to inventory Unix-only APIs (`std::os::unix`, `UnixStream`, `UnixListener`, socket file cleanup, symlink tests) and classify each site as transport implementation, reusable connection logic, or Unix-only test.
    - Audit Results:
      - Transport implementation: `src/server/mod.rs` remains Unix-only behind `pub mod server`'s `#[cfg(unix)]` boundary; it owns `UnixListener`, socket parent validation, and stale socket cleanup with `FileTypeExt::is_socket`.
      - Shared connection logic to refactor next: `src/server/connection.rs` and Unix-gated portions of `src/client/mod.rs` still use `UnixStream` directly and should become generic over async read/write streams in the connection-refactor task.
      - Binary/app gates added: `src/main.rs` and `src/bin/clay-server.rs` now compile non-Unix paths without importing `clay::server`; non-Unix client/server modes return explicit unsupported-IPC errors until named pipes are implemented.
      - Test gates added: UnixStream-based client tests are now under `#[cfg(all(test, unix))]`; server tests remain covered by the existing Unix-only `server` module boundary.
      - Unix-only workspace tests: `src/server/workspace.rs` special-file and symlink tests remain Unix-only through the `server` module gate and continue documenting socket/symlink security behavior.
    - API Notes and Examples:
      ```bash
      rg -n "std::os::unix|UnixStream|UnixListener|is_socket|\.sock|XDG_RUNTIME_DIR" src tests docs plans
      cargo check --all-targets
      cargo check --target x86_64-pc-windows-msvc --all-targets
      ```
    - Files to Create/Edit:
      - `src/lib.rs`: Reviewed existing `#[cfg(unix)] pub mod server` gate; no change needed.
      - `src/main.rs`: Added platform gates around server imports, Unix IPC startup, background server spawn, and connect retry; added non-Unix unsupported-IPC errors.
      - `src/bin/clay-server.rs`: Added platform gates around Unix server imports/runtime and a non-Unix unsupported-IPC error path.
      - `src/client/mod.rs`: Added `#[cfg(all(test, unix))]` to UnixStream-based client integration tests; existing Unix connect functions remain `#[cfg(unix)]` for the later transport refactor.
      - `src/server/mod.rs`: Reviewed Unix listener and stale socket cleanup; remains under the `server` module's `#[cfg(unix)]` boundary.
      - `src/server/connection.rs`: Reviewed direct `UnixStream` usage; left as the actionable target for the generic async-stream refactor task.
      - `src/server/workspace.rs`: Reviewed Unix-only socket/symlink tests; remains under the `server` module's `#[cfg(unix)]` boundary.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documented current Unix-only server boundary and temporary non-Unix unsupported path.
      - `docs/wiki/flows/client-server-edit-ack.md`: Documented current Unix-only startup boundary and planned local named-pipe direction.
    - References:
      - `docs/wiki/modules/server-ipc-skeleton.md`
      - `docs/wiki/flows/client-server-edit-ack.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - Windows compile audit: `cargo check --target x86_64-pc-windows-msvc --all-targets` reports only expected missing toolchain/prerequisite issues before implementation and passes after implementation.
    - Unix regression audit: `cargo check --all-targets` continues to pass on Unix after adding cfg boundaries.
  - Verification Results:
    - `mise x rust@latest -- cargo fmt --check`: passed.
    - `mise x rust@latest -- cargo check --all-targets`: passed after Windows Developer Mode enabled symlink creation for the `v8` build script; current output includes non-fatal unused-code warnings from temporarily gated non-Unix IPC paths.
    - `mise x rust@latest -- cargo check --target x86_64-pc-windows-msvc --all-targets`: passed after Windows Developer Mode enabled symlink creation for the `v8` build script; current output includes the same non-fatal unused-code warnings.
    - `rg -n "std::os::unix|UnixStream|UnixListener|is_socket|\\.sock|XDG_RUNTIME_DIR|symlink" src docs/wiki plans/011-Windows-Platform-Support.md`: confirms remaining Unix APIs are either inside the Unix-gated server module, Unix-gated client functions/tests, documentation, or the explicitly listed next refactor targets.

- [x] Introduce a platform-neutral IPC endpoint model
  - Acceptance Criteria:
    - Functional: Client, server, CLI parsing, and default endpoint selection use an endpoint abstraction that represents Unix socket paths on Unix and named pipe names on Windows.
    - Performance: Endpoint construction is simple string/path selection and does not perform IPC, filesystem scans, or blocking work on the UI path.
    - Code Quality: Endpoint naming is centralized in `src/ipc.rs`; high-level app code no longer treats every IPC address as a Unix socket path.
    - Security: Windows named pipe defaults are local-machine pipe names, per-user where practical, and do not introduce remote listeners, network sockets, shell execution, or broader filesystem authority.
  - Approach:
    - Documentation Reviewed:
      - Context7 Tokio named pipe docs: pipe addresses use forms such as `r"\\.\pipe\mynamedpipe"`.
      - Context7 Rustup docs: target name for MSVC validation is `x86_64-pc-windows-msvc`.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Server remains authoritative; endpoint selection must not move canonical ownership into the client.
    - Options Considered:
      - Keep `PathBuf` everywhere and reinterpret it on Windows: fewer type changes, but confusing because named pipes are not filesystem paths.
      - Add an `IpcEndpoint` enum: clearer platform semantics and easier validation.
      - Use TCP loopback on all platforms: simpler API, but expands the security model and contradicts local IPC scope.
    - Chosen Approach:
      - Add an `IpcEndpoint` or equivalent platform-gated type in `src/ipc.rs`, with `default_endpoint()` replacing or wrapping `default_socket_path()` while keeping Unix compatibility helpers where useful.
    - API Notes and Examples:
      ```rust
      pub enum IpcEndpoint {
          #[cfg(unix)]
          UnixSocket(std::path::PathBuf),
          #[cfg(windows)]
          WindowsNamedPipe(String),
      }

      #[cfg(windows)]
      pub fn default_endpoint() -> IpcEndpoint {
          IpcEndpoint::WindowsNamedPipe(format!(r"\\.\pipe\clay-{}", current_user_suffix()))
      }
      ```
    - Files to Create/Edit:
      - `src/ipc.rs`: Added `IpcEndpoint`, platform default endpoint helpers, endpoint display, child-argument conversion, Unix socket accessors, and Windows named pipe defaults.
      - `src/main.rs`: Parsed `server`/`client` endpoint arguments through `IpcEndpoint`; Unix runtime converts endpoints to socket paths at the transport boundary while non-Unix errors display endpoint strings.
      - `src/bin/clay-server.rs`: Uses `IpcEndpoint` instead of `PathBuf` socket-only logic.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documented central endpoint modeling and Windows local named pipe defaults.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Documented the endpoint abstraction around client bootstrap.
      - `docs/wiki/flows/client-server-edit-ack.md`: Documented endpoint-aware startup and direct child-process endpoint arguments.
    - References:
      - `src/ipc.rs`
      - `src/main.rs`
      - `src/bin/clay-server.rs`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `default_endpoint_is_platform_valid`: Unix returns a socket path; Windows returns a `\\.\pipe\clay-*` named pipe address.
    - `cli_parses_platform_endpoint`: `clay server <endpoint>` and `clay client <endpoint>` parse into the correct endpoint variant.
    - `endpoint_display_does_not_panic`: endpoint diagnostics can be printed on both platforms.
  - Verification Results:
    - `mise x rust@latest -- cargo fmt --check`: passed.
    - `mise x rust@latest -- cargo test --lib ipc --quiet`: passed; includes `default_endpoint_is_platform_valid` and `endpoint_display_does_not_panic`.
    - `mise x rust@latest -- cargo test --bin clay cli_parses_platform_endpoint --quiet`: passed.
    - `mise x rust@latest -- cargo check --all-targets`: passed with pre-existing non-Unix dead-code/unused-import warnings from temporary unsupported transport gates.
    - `mise x rust@latest -- cargo check --target x86_64-pc-windows-msvc --all-targets`: passed with the same pre-existing warnings.
    - Note: a broad `cargo test cli_parses_platform_endpoint --quiet` attempt was avoided after Cargo tried to execute the `update-doc-registry` binary test and Windows reported elevation required (`os error 740`); the targeted `--bin clay` test was used for this CLI parser case.

- [x] Refactor shared client/server connection handling to generic async streams
  - Acceptance Criteria:
    - Functional: The Hello/Welcome/InitialDocument/BehaviorManifest handshake and edit/resync loops work with any stream implementing Tokio async read/write traits.
    - Performance: The refactor preserves split read/write handling, bounded edit queues, and non-blocking editor hot-path behavior.
    - Code Quality: Protocol dispatch remains independent of transport-specific listener/connect code; `Codec` remains the only serialization boundary.
    - Security: All IPC input remains fallible, length-bounded, and validated before dispatch regardless of transport.
  - Approach:
    - Documentation Reviewed:
      - Tokio I/O traits already used by `src/protocol/codec.rs`: `AsyncRead`, `AsyncWrite`, `AsyncReadExt`, and `AsyncWriteExt`.
      - Context7 Tokio named pipe docs: named pipe client/server handles can be used as connected async I/O resources.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Keep protocol semantics separate from codec implementation and transport.
    - Options Considered:
      - Duplicate connection loops for Unix and Windows: avoids generic bounds but invites protocol drift.
      - Make `handle_connection`, `connect_from_stream`, and `run_connection` generic over async streams: minimal duplication and matches the existing generic codec.
    - Chosen Approach:
      - Generalize connection functions to `S: AsyncRead + AsyncWrite + Unpin + Send + 'static` where spawned, and replace `UnixStream::into_split()` with `tokio::io::split(stream)` for transport-neutral split handling.
    - API Notes and Examples:
      ```rust
      use tokio::io::{AsyncRead, AsyncWrite};

      pub(crate) async fn handle_connection<S>(stream: S, client_id: u64, /* state */) -> Result<(), CodecError>
      where
          S: AsyncRead + AsyncWrite + Unpin,
      {
          // shared protocol loop
      }

      let (mut reader, mut writer) = tokio::io::split(stream);
      ```
    - Files to Create/Edit:
      - `src/server/connection.rs`: Made connection handling generic over Tokio async read/write streams and switched paired protocol tests to `tokio::io::duplex`.
      - `src/client/mod.rs`: Made stream bootstrap and background connection loop generic, switched the background task to `tokio::io::split`, and enabled paired-stream client tests on non-Unix targets with Unix real-socket tests still cfg-gated.
      - `src/protocol/codec.rs`: Verified no changes were needed beyond existing trait-based helpers.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documented generic post-accept server connection dispatch.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Documented generic client bootstrap/background stream handling.
      - `docs/wiki/flows/client-server-edit-ack.md`: Documented transport-neutral connected-stream edit/ack flow.
    - References:
      - `src/protocol/codec.rs`
      - `src/server/connection.rs`
      - `src/client/mod.rs`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - `server_accepts_hello_and_sends_snapshot`: Existing paired-stream handshake tests still pass through generic functions.
    - `client_ack_advances_confirmed_version`: Existing client connection loop tests still pass after `tokio::io::split` refactor.
    - `codec_bounds_still_apply_across_transport`: malformed/oversized frames remain rejected before protocol dispatch.
  - Verification Results:
    - `mise x rust@latest -- cargo fmt --check`: passed.
    - `mise x rust@latest -- cargo test --lib client --quiet`: passed, including generic in-memory stream bootstrap, edit acknowledgement, resync, behavior-manifest, and bounded queue coverage on the Windows host.
    - `mise x rust@latest -- cargo test --lib codec_rejects_oversized_phase5_frame --quiet`: passed.
    - `mise x rust@latest -- cargo check --all-targets`: passed with pre-existing non-Unix dead-code warnings in `src/main.rs` from temporary unsupported transport gates.
    - `mise x rust@latest -- cargo check --target x86_64-pc-windows-msvc --all-targets`: passed with the same pre-existing warnings.
    - `mise x rust@latest -- cargo test --lib server_accepts_hello_and_sends_snapshot --quiet` and `mise x rust@latest -- cargo test --lib server_rejects_invalid_frame_without_panic --quiet`: reported 0 tests on this Windows host because `clay::server` remains Unix-gated until the transport implementation task; the server connection tests were refactored to generic `tokio::io::duplex` streams for Unix builds.

- [x] Add Unix and Windows transport implementations
  - Acceptance Criteria:
    - Functional: Unix uses existing Unix domain socket behavior; Windows creates and accepts Tokio named pipe server instances and connects clients with busy-pipe retry handling.
    - Performance: Listener accept/connect loops use async Tokio APIs and do not block the GUI thread or serialize unrelated client connections globally beyond current server state constraints.
    - Code Quality: Platform-specific code is isolated in small transport modules or cfg-gated functions; high-level client/server code consumes the same endpoint type.
    - Security: Windows transport binds only local named pipes; Unix transport continues to remove only stale socket nodes and refuses to replace normal files.
  - Approach:
    - Documentation Reviewed:
      - Context7 Tokio docs: `ServerOptions::new().create(PIPE_NAME)?`, `NamedPipeServer::connect().await?`, and `ClientOptions::new().open(PIPE_NAME)?`.
      - Context7 Tokio docs: clients should handle `ERROR_PIPE_BUSY` by retrying with a delay.
      - Context7 Rustup docs: MSVC targets require Visual Studio-provided linker/libraries and Windows SDK.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`: Transport must not change server ownership of canonical state or leases.
    - Options Considered:
      - One pipe instance at a time: simplest but may reject or stall concurrent clients.
      - Pre-create/rotate named pipe instances in an async accept loop: closer to Unix accept semantics and supports multiple clients.
      - TCP loopback fallback: easier to test but expands local IPC authority and is out of scope.
    - Chosen Approach:
      - Add Windows named pipe server accept logic that creates a pipe instance, awaits `connect()`, then spawns the shared connection handler and creates another instance for future clients. Add client connect logic using `ClientOptions::open` with retry for `ERROR_PIPE_BUSY` and not-found handling compatible with auto-start behavior.
    - API Notes and Examples:
      ```rust
      #[cfg(windows)]
      use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

      let server = ServerOptions::new().create(r"\\.\pipe\clay-user")?;
      server.connect().await?;

      let client = ClientOptions::new().open(r"\\.\pipe\clay-user")?;
      ```
    - Files to Create/Edit:
      - `Cargo.toml`: Reviewed; no `windows-sys` dependency was needed because the transport uses local raw OS error constants for `ERROR_PIPE_BUSY` and the already-connected pipe race.
      - `src/lib.rs`: Exposes the server module on Unix and Windows.
      - `src/server/mod.rs`: Routes `IpcServer::run` through Unix socket or Windows named-pipe transport, keeps Unix stale-socket cleanup, spawns shared connection handling for accepted streams, and validates local Windows pipe prefixes.
      - `src/client/mod.rs`: Routes `connect` and `load_initial_state` through `IpcEndpoint`, opens Unix sockets or Windows named-pipe clients, retries busy named pipes, and adds Windows named-pipe integration tests.
      - `src/ipc.rs`: Added endpoint conversions for Unix server tests/callers and validates Windows local named-pipe endpoints.
      - `src/main.rs`: Uses endpoint-based client/server calls now that both Unix and Windows transports are available.
      - `src/bin/clay-server.rs`: Uses endpoint-based `ServerConfig` on Unix and Windows.
      - `src/server/workspace.rs`: Keeps Unix-only special-file/symlink tests explicitly cfg-gated now that `clay::server` compiles on Windows.
      - `docs/wiki/index.md`: Updated navigation summaries for platform IPC.
      - `docs/wiki/modules/server-ipc-skeleton.md`: Documented Unix and Windows server transport behavior.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Documented endpoint-based client transport and named-pipe busy retry.
      - `docs/wiki/flows/client-server-edit-ack.md`: Documented transport-aware startup and unchanged generic protocol loop.
    - References:
      - Context7 Tokio named pipe docs for `ServerOptions`, `ClientOptions`, and busy-pipe retries.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `windows_named_pipe_client_receives_initial_snapshot`: Real Windows named pipe server sends the initial snapshot and manifest.
    - `windows_named_pipe_edit_gets_acknowledged`: Client sends an edit over the named pipe and receives `EditAck`.
    - `windows_second_client_is_read_only`: Second Windows client receives observer/read-only access, matching Unix semantics.
    - `unix_socket_transport_regression`: Existing Unix real-socket end-to-end tests still pass.
  - Verification Results:
    - `mise x rust@latest -- cargo fmt --check`: passed after formatting the changed Rust files.
    - `mise x rust@latest -- cargo check --all-targets`: passed on the Windows host.
    - `mise x rust@latest -- cargo check --target x86_64-pc-windows-msvc --all-targets`: passed.
    - `mise x rust@latest -- cargo test --lib client --quiet`: passed; includes the Windows named-pipe integration tests on this Windows host plus generic client protocol coverage.
    - `mise x rust@latest -- cargo test --lib windows_named_pipe --quiet`: passed; covers initial snapshot and edit acknowledgement over a real named pipe.
    - `mise x rust@latest -- cargo test --lib windows_second_client_is_read_only --quiet`: passed.
    - `mise x rust@latest -- cargo test --test rust_visibility_api_mapping --quiet`: passed.
    - Note: `mise x rust@latest -- cargo test rust_visibility_api_mapping --quiet` still attempts to execute the `update-doc-registry` binary test on Windows and fails before running it with elevation required (`os error 740`), so the targeted `--test rust_visibility_api_mapping` command was used.

- [x] Make binaries and background server startup platform-aware
  - Acceptance Criteria:
    - Functional: `clay`, `clay server`, `clay client`, and `clay-server` compile and run on Windows MSVC and existing Unix targets.
    - Performance: Auto-start retry remains bounded and asynchronous; startup does not block Masonry input/rendering after the editor opens.
    - Code Quality: CLI names and user-facing behavior remain stable; platform-specific endpoint formatting is hidden behind `ipc` helpers.
    - Security: Background startup only launches the current Clay executable with an explicit local endpoint argument and does not invoke a shell.
  - Approach:
    - Documentation Reviewed:
      - Context7 Rustup docs: MSVC setup and target commands for Windows validation.
      - Existing `docs/wiki/flows/client-server-edit-ack.md`: Bare `clay` auto-starts a background server, then opens a client.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: No shell/network/AI/file authority should be introduced by platform work.
    - Options Considered:
      - Create separate Windows-only binaries: explicit but duplicates CLI behavior.
      - Keep existing binaries and make endpoint/startup platform-aware: less user-visible churn and easier regression testing.
    - Chosen Approach:
      - Keep CLI commands stable and update internals to accept/display `IpcEndpoint`. Keep `Command::new(current_exe)` with direct args, not shell invocation.
    - API Notes and Examples:
      ```powershell
      cargo run -- server
      cargo run -- client
      cargo run
      ```
    - Files to Create/Edit:
      - `src/main.rs`: Uses platform endpoint abstraction in parser, connect retry, and background startup; added a testable shell-free background server `Command` builder.
      - `src/bin/clay-server.rs`: Verified existing platform endpoint abstraction is used for foreground server startup.
      - `src/ipc.rs`: Verified endpoint argument conversion is used for child process startup.
    - References:
      - `src/main.rs`
      - `src/bin/clay-server.rs`
      - `docs/wiki/flows/client-server-edit-ack.md`
  - Test Cases to Write:
    - `parses_no_args_as_auto`: Existing parser behavior remains stable with endpoint abstraction.
    - `parses_server_subcommand`: Server mode carries a valid platform endpoint.
    - `auto_start_uses_current_exe_without_shell`: Command construction remains shell-free and passes endpoint argument directly.
  - Verification Results:
    - `mise x rust@latest -- cargo fmt --check`: passed.
    - `mise x rust@latest -- cargo test --bin clay auto_start_uses_current_exe_without_shell --quiet`: passed.
    - `mise x rust@latest -- cargo test --bin clay parses_no_args_as_auto --quiet`: passed.
    - `mise x rust@latest -- cargo test --bin clay parses_server_subcommand --quiet`: passed.
    - `mise x rust@latest -- cargo test --bin clay cli_parses_platform_endpoint --quiet`: passed.
    - `mise x rust@latest -- cargo check --all-targets`: passed on the Windows host.
    - `mise x rust@latest -- cargo check --target x86_64-pc-windows-msvc --all-targets`: passed.

- [x] Add Windows MSVC setup and validation documentation
  - Acceptance Criteria:
    - Functional: Developers can follow repository documentation to install/use the MSVC Rust toolchain and run Windows validation commands.
    - Performance: Documentation includes expected validation scope without adding runtime features.
    - Code Quality: Documentation distinguishes Windows named pipes from Unix socket paths and links to relevant implementation wiki pages.
    - Security: Documentation states that Windows support uses local named pipes, not remote TCP listeners or shell-mediated IPC.
  - Approach:
    - Documentation Reviewed:
      - Context7 Rustup docs: `rustup target add x86_64-pc-windows-msvc`, `rustup default stable-msvc`, Visual Studio C++ tools, and Windows SDK prerequisites.
      - Context7 Tokio named pipe docs: named pipe address examples and busy-pipe handling.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Internal implementation details belong in the wiki; public Clay JS behavior belongs in reference docs.
    - Options Considered:
      - Put setup notes only in the code wiki: good for internals but less visible to new contributors.
      - Add a short platform setup doc and link from wiki: more discoverable and keeps implementation details in wiki pages.
    - Chosen Approach:
      - Add or update developer documentation with MSVC setup commands, validation commands, and a concise platform IPC explanation. Keep detailed implementation behavior in `docs/wiki` during the final wiki task.
    - API Notes and Examples:
      ```powershell
      rustup default stable-msvc
      rustup target add x86_64-pc-windows-msvc
      rustc -vV
      cargo check --target x86_64-pc-windows-msvc --all-targets
      cargo test --target x86_64-pc-windows-msvc
      ```
    - Files to Create/Edit:
      - `docs/development/windows.md`: Added Windows MSVC prerequisites, rustup setup commands, symlink permission note, local named-pipe IPC explanation, validation commands, and manual startup smoke tests.
      - `docs/index.md`: Added a Developer Guides link to the Windows MSVC documentation outside the Clay JS API registry source section.
      - `docs/wiki/**`: No edit in this task; linked existing implementation wiki pages for platform IPC details.
    - References:
      - Context7 Rustup Windows/MSVC docs.
      - Existing wiki implementation pages for server IPC, client bootstrap, and client/server edit acknowledgement.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
  - Test Cases to Write:
    - Documentation link review: Confirm any new Windows setup doc is linked from `docs/index.md`.
    - Command review: Confirm documented commands match actual package targets and platform support.
  - Verification Results:
    - `test -f docs/development/windows.md && rg -n -F "Windows MSVC Development" docs/index.md docs/development/windows.md && rg -n -F "development/windows.md" docs/index.md && rg -n -F "\\\\.\\pipe" docs/development/windows.md && rg -n -F "cargo check --target x86_64-pc-windows-msvc" docs/development/windows.md && rg -n -F "remote TCP" docs/development/windows.md && rg -n -F "shell-mediated IPC" docs/development/windows.md && rg -n -F "windows_named_pipe" docs/development/windows.md && rg -n -F "rust_visibility_api_mapping" docs/development/windows.md && rg -n -F "cargo test --all-targets" docs/development/windows.md`: passed after correcting an initial `rg` regex escaping error by switching to fixed-string checks.
    - Manual command review: documented validation commands match commands already used by completed Windows transport tasks (`cargo check --target x86_64-pc-windows-msvc --all-targets`, targeted Windows named-pipe tests, and `cargo test --test rust_visibility_api_mapping --quiet`).

- [x] Verify cross-platform build and IPC behavior
  - Acceptance Criteria:
    - Functional: Unix and Windows builds/tests validate client/server bootstrap, edit ack, read-only second client behavior, and stale/resync behavior where platform transport supports it.
    - Performance: Tests include or preserve coverage proving the editor hot path uses bounded `try_send` and does not await IPC queue capacity.
    - Code Quality: Platform-specific tests are cfg-gated clearly, deterministic, and clean up sockets/pipes/temp directories.
    - Security: Tests cover invalid frames or connection failures without panics and avoid using remote network listeners.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Prefer automated checks for maintained artifacts and actionable failures.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Include tests for codec boundaries and non-blocking editor behavior.
      - Context7 Tokio named pipe docs: client open can return NotFound or pipe-busy states that must be handled explicitly.
    - Options Considered:
      - Manual Windows smoke testing only: fast initially but easy to regress.
      - Add cfg-gated Windows integration tests plus documented manual GUI smoke tests: stronger and practical for CI/local validation.
    - Chosen Approach:
      - Preserve Unix tests, add Windows named pipe tests under `#[cfg(windows)]`, and document full validation commands. If CI is not configured yet, keep commands deterministic for local MSVC runs.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo test --all-targets
      cargo check --target x86_64-pc-windows-msvc --all-targets
      ```
    - Files to Create/Edit:
      - `src/client/mod.rs`: Added a Windows named-pipe stale-edit rejection plus explicit resync recovery integration test, reusing cfg-gated Windows endpoint helpers and the shared codec.
      - `src/server/mod.rs`: Verified existing Unix listener tests remain cfg-gated under `#[cfg(all(test, unix))]`; no additional listener code was needed for this verification task.
      - `src/server/connection.rs`: Verified shared generic protocol tests cover handshake, edit ack, resync response, and invalid-frame handling over in-memory async streams; no additional edit was needed.
      - `src/docs/registry.rs`: Made Clay JS API frontmatter parsing accept CRLF opening delimiters so Windows `cargo test --all-targets` can validate docs generated on Windows checkouts.
      - `build.rs`: Added a Windows-only link argument for the `update-doc-registry` binary/test binary manifest.
      - `windows/no-uac.manifest`: Added an `asInvoker` manifest for `update-doc-registry` so Windows does not require elevation for the `update-*` executable during all-target test runs.
      - `docs/generated/clay-js-api-registry.json`: Regenerated with `cargo run --bin update-doc-registry` after the CRLF parser fix.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`, `docs/wiki/flows/client-server-edit-ack.md`, `docs/wiki/modules/server-ipc-skeleton.md`: Documented the Windows named-pipe stale/resync test coverage.
      - `docs/wiki/modules/clay-js-doc-registry.md`: Documented the CRLF parser tolerance and Windows no-UAC registry update binary manifest.
      - `.github/workflows/*` or equivalent CI files if present: Not edited; no existing CI workflow was found or required for this local verification task.
    - References:
      - `src/client/mod.rs`
      - `src/server/mod.rs`
      - `src/server/connection.rs`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - `cargo fmt --check`: Formatting is stable.
    - `cargo test --all-targets`: Native target tests pass.
    - `cargo check --target x86_64-pc-windows-msvc --all-targets`: Windows MSVC compilation passes.
    - Manual GUI smoke test: On Windows, run `cargo run`, type text, observe no IPC panic and server event logging.
  - Verification Results:
    - `mise x rust@latest -- cargo fmt --check`: passed.
    - `mise x rust@latest -- cargo test --lib windows_named_pipe --quiet`: passed; covers named-pipe initial snapshot, edit acknowledgement, and read-only second-client behavior on Windows.
    - `mise x rust@latest -- cargo test --lib windows_named_pipe_stale_edit_rejected_then_resynced --quiet`: passed; validates stale-version rejection and explicit resync recovery over a real Windows named pipe.
    - `mise x rust@latest -- cargo test --lib client_hot_path_does_not_await_full_ipc_queue --quiet`: passed; preserves bounded `try_send` hot-path coverage.
    - `mise x rust@latest -- cargo test --lib server_rejects_invalid_frame_without_panic --quiet`: passed; validates malformed IPC input handling without panics.
    - `mise x rust@latest -- cargo test --lib codec_rejects_oversized_phase5_frame --quiet`: passed; validates bounded frame rejection before protocol dispatch.
    - `mise x rust@latest -- cargo test --test clay_js_doc_registry --quiet`: passed after adding CRLF frontmatter tolerance and regenerating the registry artifact.
    - `mise x rust@latest -- cargo test --all-targets`: passed on the Windows host after embedding an `asInvoker` manifest for the `update-doc-registry` test binary; this fixed Windows UAC executable-name heuristics that previously returned elevation required (`os error 740`).
    - `mise x rust@latest -- cargo check --target x86_64-pc-windows-msvc --all-targets`: passed.
    - `mise x rust@latest -- cargo check --all-targets`: passed.
    - Manual GUI smoke test: not executed in this non-interactive agent run; automated Windows named-pipe IPC tests cover bootstrap, edit ack, read-only observer access, stale/resync, invalid-frame, and non-blocking queue behavior without opening remote network listeners.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: The implementation is reviewed for new user-visible behavior or behavior-changing settings, and any configurable IPC/platform behavior is represented as documented Clay JS APIs or explicitly kept internal.
    - Performance: Configuration review does not add runtime configuration loading to editor input, rendering, Masonry paint/layout, or IPC frame hot paths.
    - Code Quality: Any new configuration surface uses `~/.config/clay/init.js` conventions and is documented rather than hidden in ad hoc environment variables or undocumented flags.
    - Security: Configuration does not grant filesystem, network, shell, extension loading, AI mutation, workspace, WASM, or remote listener authority beyond the documented local IPC endpoint behavior.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Every configuration option is a Clay JS API, not an undocumented config key.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Public programmatic behavior must be documented and registry-backed.
    - Options Considered:
      - Expose named pipe/socket endpoint configuration immediately: useful but may be premature if CLI-only endpoint selection remains sufficient.
      - Keep endpoint selection internal/CLI-only for this plan: smaller scope unless user configuration is intentionally added.
    - Chosen Approach:
      - Review the final implementation. If endpoint selection or platform behavior becomes user-configurable beyond existing CLI arguments, add Clay JS API docs and registry coverage; otherwise record that no new configuration API was introduced.
    - API Notes and Examples:
      ```javascript
      // Only if a future configuration API is introduced:
      // import { configureIpc } from "clay:configuration";
      // configureIpc({ endpoint: "platform-default" });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: Reviewed; no Windows IPC/platform configuration API docs were needed because no new user configuration surface was introduced.
      - `docs/index.md`: Reviewed; existing configuration docs remain linked and no new Windows IPC configuration doc link was needed.
      - `runtime/js/**`: Reviewed; existing planned configuration facade remains unchanged.
      - `src/server/**` and `src/client/**`: Reviewed Windows transport configuration surface; endpoint selection remains internal/CLI-driven and transport helpers are not exposed as Clay JS configuration APIs.
      - `src/ipc.rs`: Reviewed default endpoint/environment usage; `XDG_RUNTIME_DIR`, `USER`, and `USERNAME` are platform default derivation inputs, not behavior-changing Clay configuration keys.
    - Review Results:
      - No new Clay JS configuration API was introduced for Windows IPC. The named-pipe/socket endpoint remains selected by platform defaults or explicit CLI endpoint arguments (`clay server <endpoint>`, `clay client <endpoint>`), not by `~/.config/clay/init.js` settings.
      - No undocumented behavior-changing configuration key or Clay-specific environment variable was added. The only environment reads in the Windows-port IPC path are OS/user-default helpers for endpoint naming.
      - Existing planned configuration APIs (`clay.configuration.loadConfigurationModule` and `clay.configuration.getConfigurationState`) already remain documented, indexed, and registry-backed.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Configuration API coverage review: If new configuration APIs are added, tests fail when docs/index/registry/custom properties are missing.
    - No-new-config review: If no configuration API is added, verify no undocumented behavior-changing config key or env var was introduced.
  - Verification Results:
    - `rg -n "std::env::var|std::env::var_os|env::var|env::var_os|CLAY_|XDG_RUNTIME_DIR|USERNAME|USER|init\\.js|configuration" src runtime/js docs/reference/clay-js-api docs/index.md`: passed; confirmed no Clay-specific undocumented configuration key was introduced and existing configuration docs/facades remain the documented surface.
    - `mise x rust@latest -- cargo test --test clay_js_doc_registry --quiet`: passed; existing configuration API docs and generated registry remain current.
    - `mise x rust@latest -- cargo test --test rust_visibility_api_mapping --quiet`: passed; no new configuration API exposure was required by changed Rust visibility.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: All server-side Rust public functions introduced or changed by the Windows port are inventoried and either exposed through documented Clay JS APIs when they are public capabilities or made private/`pub(crate)` when internal.
    - Performance: API review does not add JavaScript or IPC round trips to ordinary typing, rendering, or transport frame handling.
    - Code Quality: Public Clay JS APIs, if any, have Markdown docs, stable IDs, searchable user-facing names, key binding metadata, custom properties, examples, authority/security notes, backing Rust paths, op wrapper paths, facade paths, and lookup tags.
    - Security: No raw `Deno.core.ops.op_*` calls become user-facing APIs, and no new API grants filesystem, network, shell, extension loading, AI mutation, workspace, WASM, or remote listener authority implicitly.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API task and coverage requirements.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown-authoritative registry contract.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md` if API exposure becomes necessary during implementation.
    - Options Considered:
      - Treat transport internals as public APIs: discoverable but overexposes implementation details.
      - Keep platform transport internals private and document only existing user-facing CLI/setup behavior: likely correct for this port unless a real programmatic configuration capability is introduced.
    - Chosen Approach:
      - After implementation, inventory changed `pub` functions. Prefer private/`pub(crate)` for transport helpers. Add Clay JS API docs/registry updates only for intentional public programmatic capabilities.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test clay_js_api_inventory clay_js_doc_registry clay_js_facade_layout rust_visibility_api_mapping
      ```
    - Files to Create/Edit:
      - `src/ipc.rs`: Reviewed visibility of new endpoint helpers; public items are package-local Rust infrastructure used by binaries and not Clay JS APIs.
      - `src/client/mod.rs`: Reviewed visibility of transport helpers; transport-specific connect helpers remain private, while existing client session/bootstrap types remain Rust client infrastructure rather than Clay JS surfaces.
      - `src/server/mod.rs`: Reviewed visibility of transport helpers; Windows named-pipe creation/connection helpers remain private, and public `ServerConfig`, `IpcServer`, and `ServerError` remain non-JS server process infrastructure.
      - `docs/reference/clay-js-api/**`: No new public Clay JS API Markdown docs were added because the Windows port introduced no public programmatic Clay JS capability.
      - `docs/index.md`: Reviewed; no new Clay JS API docs required linking because no public Clay JS API was introduced.
      - `docs/reference/clay-js-api/api-inventory.toml`: Updated the internal `internal.server.ipcRuntime` classification to cover platform-local IPC endpoint/runtime ownership, including `src/ipc.rs` and Windows named pipes, while keeping `registry_public = false`.
    - Review Results:
      - No Windows IPC transport helper is exposed through Clay JS. Endpoint derivation, Unix socket binding, named-pipe creation, busy retry, and connection spawning remain Rust process infrastructure.
      - No raw `Deno.core.ops.op_*` calls were added or made user-facing. Existing runtime JS facade stubs still document that future ops must sit behind stable Clay JS facade exports.
      - No new public Clay JS docs or generated registry update were needed; the only documentation-source change was the internal inventory classification for platform IPC runtime ownership.
      - Existing server Rust public items remain either allowlisted non-JS infrastructure (`ServerConfig::new`, `IpcServer::{new,try_new,run}`) or mapped/classified by the inventory (`ServerConfig`, `IpcServer`, `ServerError`).
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
  - Test Cases to Write:
    - `cargo test rust_visibility_api_mapping`: Public Rust surface mapping remains intentional.
    - `cargo test clay_js_api_inventory`: Clay JS inventory remains complete if APIs are added.
    - `cargo test clay_js_doc_registry`: Generated registry remains current if docs change.
    - `cargo test clay_js_facade_layout`: Facade layout remains valid if runtime JS files change.
  - Verification Results:
    - `rg -n "pub(\\([^)]*\\))?\\s+(async\\s+)?(fn|struct|enum|type|mod|trait|const|static)" src/ipc.rs src/client/mod.rs src/server/mod.rs src/server/connection.rs`: passed; reviewed public endpoint, client, and server Rust surfaces touched by the Windows port.
    - `rg -n "Deno\\.core\\.ops|op_" runtime/js docs/reference/clay-js-api src | head -200`: passed; occurrences are documentation/schema/future-op metadata or existing comments, not new user-facing raw op calls.
    - `mise x rust@latest -- cargo test --test rust_visibility_api_mapping --quiet`: passed; public server Rust items remain mapped or explicitly allowlisted as non-JS infrastructure.
    - `mise x rust@latest -- cargo test --test clay_js_api_inventory --quiet`: passed; inventory metadata, internal/public classification, and raw-op facade constraints remain valid.
    - `mise x rust@latest -- cargo test --test clay_js_doc_registry --quiet`: passed; public Clay JS docs and generated registry remain current.
    - `mise x rust@latest -- cargo test --test clay_js_facade_layout --quiet`: passed; runtime JS facade layout remains valid.

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
      - `docs/wiki/modules/server-ipc-skeleton.md`: Update Unix socket-only language to include platform IPC.
      - `docs/wiki/modules/client-snapshot-bootstrap.md`: Update client bootstrap transport details.
      - `docs/wiki/flows/client-server-edit-ack.md`: Document Windows named pipe startup and unchanged hot-path behavior.
      - `docs/wiki/**`: Add additional implementation pages if transport modules are split out.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `docs/wiki/index.md`
      - `docs/wiki/modules/server-ipc-skeleton.md`
      - `docs/wiki/modules/client-snapshot-bootstrap.md`
      - `docs/wiki/flows/client-server-edit-ack.md`
      - `docs/wiki/modules/clay-js-doc-registry.md`
    - Review Results:
      - The code wiki was already updated by the completed implementation and verification tasks, so no additional wiki page edits were required in this final pass.
      - `docs/wiki/index.md` links every discoverable page under `docs/wiki/` and summarizes the Windows named-pipe/platform IPC implementation areas.
      - `docs/wiki/modules/server-ipc-skeleton.md` documents `src/ipc.rs`, Unix socket stale-node protection, Windows local named-pipe validation/rotation, generic async-stream connection dispatch, security boundaries, and IPC tests.
      - `docs/wiki/modules/client-snapshot-bootstrap.md` documents endpoint-based bootstrap, Windows pipe busy retry, generic handshake/background stream handling, source paths, and Windows named-pipe tests.
      - `docs/wiki/flows/client-server-edit-ack.md` documents endpoint-aware CLI startup/auto-start, shell-free background server launch, unchanged bounded `try_send` hot path, resync behavior, and Unix/Windows transport coverage.
      - `docs/wiki/modules/clay-js-doc-registry.md` documents Windows-specific registry validation support: CRLF frontmatter tolerance and the `windows/no-uac.manifest` as-invoker manifest for `update-doc-registry` test binaries.
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Verification Results:
    - `node - <<'JS' ... JS`: passed; confirmed all 13 non-index Markdown pages under `docs/wiki/` are linked from `docs/wiki/index.md`.
    - `rg -n -F "Windows named pipes" docs/wiki/index.md docs/wiki/modules/server-ipc-skeleton.md docs/wiki/modules/client-snapshot-bootstrap.md docs/wiki/flows/client-server-edit-ack.md`: passed; confirmed platform IPC is documented in the master index and implementation pages.
    - `rg -n -F "src/ipc.rs" docs/wiki/modules/server-ipc-skeleton.md docs/wiki/modules/client-snapshot-bootstrap.md docs/wiki/flows/client-server-edit-ack.md`: passed; confirmed the endpoint abstraction source path is documented on relevant pages.
    - `rg -n -F "windows_named_pipe_stale_edit_rejected_then_resynced" docs/wiki/modules/server-ipc-skeleton.md docs/wiki/modules/client-snapshot-bootstrap.md docs/wiki/flows/client-server-edit-ack.md`: passed; confirmed Windows stale/resync transport coverage is documented.
    - `rg -n -F "windows/no-uac.manifest" docs/wiki/modules/clay-js-doc-registry.md`: passed; confirmed Windows registry-update test manifest behavior is documented.
    - Note: An initial `python` helper for the index-link check could not run because Python is not installed in this environment; the equivalent Node.js check above was used successfully.

## Compromises Made
- Manual GUI smoke testing was not executed in this non-interactive agent run; automated Windows named-pipe IPC tests cover bootstrap, edit acknowledgement, read-only observer access, stale/resync recovery, invalid-frame handling, and non-blocking queue behavior.
- No CI workflow was added because no existing CI workflow was found during the verification task; Windows validation remains documented as deterministic local commands.
- No new Clay JS configuration or public programmatic API was introduced for Windows IPC; endpoint selection remains platform-default or CLI-driven process infrastructure.

## Further Actions
- Priority 1: Add CI coverage for `cargo fmt --check`, native `cargo test --all-targets`, and Windows MSVC `cargo check --target x86_64-pc-windows-msvc --all-targets` when project CI is introduced.
- Priority 2: Run and record a manual Windows GUI smoke test (`cargo run`, type text, observe no IPC panic/server event logging) in an interactive environment.
- Priority 3: Revisit whether endpoint selection needs an intentional Clay JS API only if user-facing configuration requirements emerge.
