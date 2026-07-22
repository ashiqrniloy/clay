# Maintenance Validation

## Covers

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo audit`
- Routine build profiles, integration-suite topology, artifact retention, and CI reporting
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
- `Cargo.toml` uses line-table debug information for routine `dev`/`test` builds and keeps full DWARF available through the opt-in `debugging` profile.
- `autotests = false` prevents Cargo from linking every `tests/*.rs` source separately. Four explicit roots in `tests/suites/{security,runtime,editor,protocol}.rs` include all 33 source modules with plain `#[path] mod`; `integration_suite_inventory_assigns_every_source_once` detects omission or duplicate assignment.
- Documentation validation is schema-driven: `api-inventory.toml` plus generated registry metadata covers every public API; `documentation-contracts.json` enumerates every primitive/package reference page and the small security-marker set. Generic validators report IDs/paths/fields, recursively check wiki indexing, and ignore ordinary prose. Tests never run the mutating registry updater.
- Focused commands select the group and then the original source-module prefix, for example `cargo test --test security package_loading::`. Full source mapping, measurements, and cleanup guidance live in `docs/development/build-and-test.md`.

## Plan 060 Comprehensive Review Closure

The 2026-07-19 P0–P3 review is closed through executable evidence recorded in `plans/060-Comprehensive-Codebase-Review-Remediation.md`; documentation alone closes no behavioral finding. Final implementation areas and their detailed wiki owners are:

| Area | Implementation wiki |
|---|---|
| Two package runtime domains, adoption, provenance, extension consent, replacement, replay | [Embedded JavaScript Runtime](embedded-js-runtime.md), [Package Extension and Adoption Authority](third-party-runtime-authority.md) |
| Connection identity, authorized output routing, close/disconnect cleanup | [Server IPC Skeleton](server-ipc-skeleton.md), [Protocol Codec](protocol-codec.md), [Multi-Document Sessions](multi-document-sessions.md), [Parse Coordinator](parse-coordinator.md) |
| Bounded reads, atomic save identity, directory traversal/ignore grammar | [Server File Workspace](server-file-workspace.md), [Workspace File Browser](workspace-file-browser.md) |
| Bounded LSP actors and process I/O | [Language Server Process Service](language-server-process-service.md) |
| Single-source facades and public/configuration API closure | [Embedded JavaScript Runtime](embedded-js-runtime.md), [Clay JS Documentation Registry](clay-js-doc-registry.md), [Configuration Runtime](configuration-runtime.md) |
| Native dialog generations and clipboard limitation | [Client File Dialog](client-file-dialog.md), [Masonry Editor](masonry-editor.md) |

Routine dev/test profiles use line-table debug information, with full DWARF under `--profile debugging`. Consolidating 33 integration source modules into four suite roots reduced the clean all-target artifact snapshot from roughly 22 GiB/43 expected harness executables/89 s to 6.1 GiB/14 Cargo harnesses/68 s on the same Linux host; times are advisory, while suite inventory and artifact topology are deterministic. Runtime/resource ceilings remain compiled host policy rather than configuration. `cargo audit` currently passes with the documented unmaintained warnings and expiring quick-xml exceptions enforced by `audit_exceptions_are_documented_and_unexpired`.

## Invariants

- Do not silence warnings at crate level.
- Prefer direct Clippy fixes over allowances.
- Local allowances must include a reason and name the architectural constraint.
- Security tests, package/runtime authority checks, and diagnostics stay in Linux `cargo test --all-targets`; harness consolidation must preserve the normalized pre-change test-name multiset.
- Routine commands share repository-local `target/`; do not create `target/pi-verify` or another duplicate verification tree. CI reports `target/debug/{deps,incremental}` and total target size after the blocking Linux gate.
- Windows checks are run only on Windows-targeted tasks or when a Windows MSVC toolchain is available; a Linux host missing MSVC C headers/SDK is not a failure of ordinary work.
- Persistent-runtime hot reload is exercised headlessly through `IpcServer::trigger_developer_hot_reload`; the trigger calls the shared reload primitive and does not run during ordinary client event processing.
- Phase 19 end-to-end coverage also includes duplex barrier/edit, failed-reload no-snapshot, multi-client one-generation install, and LSP cleanup/authority denial tests in `server::runtime_generation_tests`.

## Verification

```bash
cargo fmt --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo audit
cargo test --test runtime persistent_runtime_hot_reload::
cargo test --test runtime runtime_update_protocol::
cargo test --test protocol performance_protocol::phase19_runtime_state
cargo test --test protocol clay_js_api_inventory::
cargo test --test protocol primitives_docs::
cargo test --test protocol package_loading_docs::
cargo test --test protocol clay_js_doc_registry::generated_registry_is_current
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
- `docs/development/build-and-test.md`
- `Cargo.toml`
- `tests/suites/`
