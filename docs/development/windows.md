# Windows MSVC Development

This guide describes the supported Windows development setup for Clay and the validation commands used by the Windows platform support plan.

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

Run these commands from the repository root in PowerShell or a Visual Studio Developer PowerShell:

```powershell
cargo fmt --check
cargo check --all-targets
cargo check --target x86_64-pc-windows-msvc --all-targets
cargo test --lib client --quiet
cargo test --lib windows_named_pipe --quiet
cargo test --lib windows_second_client_is_read_only --quiet
cargo test --test rust_visibility_api_mapping --quiet
```

For a broader native-target pass, run:

```powershell
cargo test --all-targets
```

If a broad test command tries to execute a helper binary and Windows reports `os error 740` because elevation is required, rerun the targeted test command listed above for that validation area.

## Manual Smoke Test

After the checks pass, verify the command-first startup paths:

```powershell
cargo run
cargo run -- server
cargo run -- client
cargo run -- smoke-gui
```

Expected behavior:

- Bare `cargo run` connects to the default local named pipe, starts a background `clay server` process if needed with direct child-process arguments, and opens a GUI client.
- `cargo run -- server` starts a foreground server on the default local named pipe and prints the listening endpoint or an actionable bind/listen error.
- `cargo run -- client` connects to an already-running default server, or opens a local fallback GUI and reports the connection error if no server is available.
- `cargo run -- smoke-gui` is the app-managed GUI smoke launch mode. During the first implementation step it follows the same local-only startup path as the default GUI command; later smoke lifecycle work makes the endpoint isolated and cleans up the managed child server automatically.
- typing in the editor should not panic the IPC task; edit acknowledgements may be logged to stderr during current development phases.

Advanced endpoint arguments are optional debugging aids only. Normal validation should not require copying or typing a `\\.\pipe\...` named-pipe value:

```powershell
cargo run -- server \\.\pipe\clay-debug
cargo run -- client \\.\pipe\clay-debug
```
