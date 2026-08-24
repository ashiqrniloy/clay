# Windows MSVC Development

This guide describes the supported Windows development setup for Clay and the validation commands used by Windows-targeted work. Linux is the primary required development/CI host; Windows is a long-term target, not a blocking validation gate for ordinary Linux-host tasks unless a plan explicitly targets Windows. For the cross-platform command-first launch checklist, see [Launch and GUI Smoke Validation](launch-and-gui-smoke.md).

## Supported Toolchain

Clay targets the native Rust MSVC ABI on Windows:

- Host/target triple: `x86_64-pc-windows-msvc`
- Rust toolchain: stable MSVC, installed through `rustup`
- C/C++ prerequisites: Visual Studio Build Tools with the MSVC linker, C++ libraries, and a Windows SDK

Rustup's Windows guidance recommends the MSVC ABI for most Windows Rust development because it interoperates best with other Windows software. MSVC targets require Visual Studio-provided linker and Windows API import libraries.

## Install Prerequisites

1. Install Visual Studio Build Tools or Visual Studio.
2. In the installer, select the C++ build tools workload, including:
   - MSVC compiler/linker tools
   - Windows SDK
   - CMake/build tools are optional unless needed by local dependency builds
3. Install Rust with `rustup` and use the MSVC toolchain:

```powershell
rustup toolchain install stable-msvc
rustup default stable-msvc
rustup target add x86_64-pc-windows-msvc
rustc -vV
```

`rustc -vV` should report a host such as `x86_64-pc-windows-msvc` when the MSVC toolchain is active.

### Symlink Builds

Some dependencies may create symlinks during their build scripts. If Windows reports a symlink permission error during `cargo check` or `cargo test`, enable Windows Developer Mode or run from an elevated developer shell, then rerun the command.

## Platform IPC Model

Clay keeps the same client/server protocol on every platform, but the local IPC transport is platform-specific:

- Unix uses Unix domain socket paths and retains stale socket cleanup protections.
- Windows uses local named pipe names such as `\\.\pipe\clay-<user>`.

Windows support does **not** use remote TCP listeners, network sockets, shell-mediated IPC, or shell execution for client/server communication. Bare `clay` starts the background server by launching the current executable directly with an explicit local endpoint argument.

Implementation details are documented in:

- [Server IPC Skeleton](../wiki/modules/server-ipc-skeleton.md)
- [Client Snapshot Bootstrap](../wiki/modules/client-snapshot-bootstrap.md)
- [Client/Server Edit Acknowledgement Flow](../wiki/flows/client-server-edit-ack.md)

## Validation Commands

Run these commands from the repository root in PowerShell or a Visual Studio Developer PowerShell when the task targets Windows or when doing optional Windows regression checks:

```powershell
cargo fmt --check
cargo check --all-targets
cargo check --target x86_64-pc-windows-msvc --all-targets
cargo test --lib client --quiet
cargo test --lib windows_named_pipe --quiet
cargo test --lib windows_second_client_is_read_only --quiet
cargo test --test security rust_visibility_api_mapping:: --quiet
```

For a broader native-target pass on Windows, run:

```powershell
cargo test --all-targets
```

If a broad test command tries to execute a helper binary and Windows reports `os error 740` because elevation is required, rerun the targeted test command listed above for that validation area.

Do not use Linux-host cross-compilation as the required Windows gate for normal work. Some dependencies build C code and need Windows/MSVC headers; from Linux, `cargo check --target x86_64-pc-windows-msvc --all-targets` can fail before Clay code is checked because headers such as `stdio.h` are unavailable.

## Tauri desktop (Phase 11)

Linux remains the blocking CI and packaging host. The Tauri shell uses the
same local named-pipe IPC as the native client (`\\.\pipe\clay-<user>`).
`CLAY_ENDPOINT` may override the pipe name; values containing `://` are
rejected. WebView2 is the Windows renderer; do not add remote CSP origins or
privileged Tauri plugins to make Windows packaging easier.

Windows MSI/NSIS bundle targets are not required for Phase 11. When a Windows
host is available, optional checks are:

```powershell
npm --prefix frontend ci
npm --prefix frontend run build
cargo test -p clay-desktop --lib --quiet
cargo test -p clay-desktop --test config_security --quiet
```

## Manual Smoke Test

After the checks pass, verify the command-first startup paths documented in [Launch and GUI Smoke Validation](launch-and-gui-smoke.md):

```powershell
cargo run
cargo run -- smoke-gui
cargo run -- server
cargo run -- client
cargo run -- client
```

For Phase 19, the Windows-specific Markdown open-dialog smoke contract is documented in [Launch and GUI Smoke Validation](launch-and-gui-smoke.md#phase-19-windows-markdown-open-dialog-smoke-contract). Use the repeatable fixture command when validating the full Markdown path:

```powershell
cargo run -- smoke-gui --config-fixture windows-markdown-open
```

The fixture loads `@clay/markdown` and binds `Ctrl+O` to `documents.clientOpenFileDialog` through `init.js`. The route opens the Windows native file browser with Markdown filters (`.md`, `.markdown`, `.mdown`) plus an all-files fallback; cancellation is a non-error no-op. Selecting a regular UTF-8 file sends an explicit selected-file open request to the server, which validates and grants only that file before replacing the GUI buffer. When `@clay/markdown` is loaded, selected `.md`, `.markdown`, and `.mdown` files also activate Markdown mode and receive viewport-bounded Markdown decorations/status for manual smoke validation. Type in the opened file to confirm local editing remains responsive, then skip save for this phase.

Windows clipboard checks for Plan 060 T13 remain host-manual (Linux CI does not claim Windows execution): copy/cut/paste through native chords and configured client commands. Since the Plan 097 cutover the client performs clipboard work through the WebView2 webview/CodeMirror inside explicit commands only; the removed native `arboard` backend required a dedicated Windows smoke, which no longer applies.

Windows-specific expected behavior:

- All normal startup paths use local named pipes and do not require copying or typing a `\\.\pipe\...` value.
- Bare `cargo run` connects to the default local named pipe, starts a background `clay server` process if needed with direct child-process arguments, and opens a GUI client with `Connected — Editable` status when the server lease is available.
- `cargo run -- smoke-gui` creates an isolated local named-pipe endpoint, starts a managed child `clay server <endpoint>` with direct process arguments, waits for readiness with bounded client handshake retries, opens the GUI client, and terminates the child server when the GUI exits.
- `cargo run -- server` starts a foreground server on the default local named pipe and prints the listening endpoint or an actionable bind/listen error.
- The first `cargo run -- client` connects to the foreground default server and should show `Connected — Editable` in the GUI status line.
- A second `cargo run -- client` uses that same default local named pipe without any endpoint argument and should show `Connected — Read-only Observer` in the GUI status line.
- If no server is available, `cargo run -- client` opens a local fallback GUI and reports the connection error.
- Startup diagnostics identify common readiness states: server starting, no server found/background start, local fallback, endpoint validation or bind failure, child server exit before readiness, handshake/protocol failure, and successful client connection.
- Typing in the editor is local/optimistic and does not wait for IPC acknowledgements; edit acknowledgements may be logged to stderr during current development phases.

Advanced endpoint arguments are optional debugging aids only, for example when reproducing a custom named-pipe bind/listen issue:

```powershell
cargo run -- server \\.\pipe\clay-debug
cargo run -- client \\.\pipe\clay-debug
```
