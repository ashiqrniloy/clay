# Maintenance Validation

## Covers

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- Follow-up plan: `plans/032-All-Target-Clippy-Cleanup.md`

## What it does

Clay now treats all-target Clippy as a runnable repository gate. Linux is the required host platform for normal validation; Windows remains a long-term target and task-specific validation target, not a blocking gate for ordinary Linux-host work unless a plan explicitly says so. The cleanup kept the gate useful by fixing mechanical warnings and keeping only narrow local allowances with `reason = ...` where the current code shape is intentional.

## How it works

- Mechanical Clippy suggestions were applied with `cargo clippy --fix --all-targets --allow-dirty --allow-staged -- -D warnings`.
- Remaining warnings were handled at the smallest source location:
  - staged SDUI/package-UI bridge descriptors use local `dead_code` allowances until dynamic publication callsites are wired;
  - hot-path paint and connection functions keep explicit argument lists instead of speculative heap context structs;
  - low-volume internal event enums keep direct payload variants until profiling shows boxing helps;
  - cold protocol rejection paths keep direct `ServerMessage` errors.
- No crate-wide `allow(warnings)` or skipped Linux target is used.

## Invariants

- Do not silence warnings at crate level.
- Prefer direct Clippy fixes over allowances.
- Local allowances must include a reason and name the architectural constraint.
- Security tests, package/runtime authority checks, and diagnostics stay in Linux `cargo test --all-targets`.
- Windows checks are run only on Windows-targeted tasks or when a Windows MSVC toolchain is available; a Linux host missing MSVC C headers/SDK is not a failure of ordinary work.
- Persistent-runtime hot reload is exercised headlessly through `IpcServer::trigger_developer_hot_reload`; the trigger calls the shared reload primitive and does not run during ordinary client event processing.
- Phase 19 end-to-end coverage also includes duplex barrier/edit, failed-reload no-snapshot, multi-client one-generation install, and LSP cleanup/authority denial tests in `server::runtime_generation_tests`.

## Verification

```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --test persistent_runtime_hot_reload
cargo test --test runtime_update_protocol
cargo test --test performance_protocol phase19_runtime_state
cargo test --lib typing_and_edit_ack_continue_while_candidate
cargo test --lib failed_reload_broadcasts_diagnostic_but_no_generation_snapshot
cargo test --lib successful_reload_is_observed_as_one_generation_by_all_clients
cargo test --lib reload_preserves_authority_denials_and_cleans_old_lsp_worker
```

Windows-targeted work additionally uses `docs/development/windows.md` on a native MSVC setup. Do not require `cargo check --target x86_64-pc-windows-msvc --all-targets` from a Linux host unless the host has the Windows SDK/MSVC C headers needed by native C dependencies.

Current cleanup verification: Clippy passed with no issues under `-D warnings`. Phase 19 hot reload verification includes `tests/persistent_runtime_hot_reload.rs` for success/rollback/sanitized diagnostics/authority denial, `tests/runtime_update_protocol.rs` for snapshot payload bounds, `tests/performance_protocol.rs` for Phase 19 budget locks, and the duplex barrier/multi-client/LSP cleanup suite in `server::runtime_generation_tests`.

## Related

- `plans/031-Phase18.7-Persistent-Server-Runtime-and-JS-ParseHandler-Bridge.md`
- `plans/032-All-Target-Clippy-Cleanup.md`
- `.agents/skills/project-patterns/references/maintenance-validation.md`
- `.agents/skills/project-patterns/references/authority-boundaries.md`
