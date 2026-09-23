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
- `autotests = false` prevents Cargo from linking every `tests/*.rs` source separately. Four explicit roots in `tests/suites/security.rs`, `tests/suites/runtime.rs`, `tests/suites/presentation.rs`, and `tests/suites/protocol.rs` include all 38 source modules with plain `#[path] mod`; `integration_suite_inventory_assigns_every_source_once` detects omission or duplicate assignment.
- Documentation validation is schema-driven: `api-inventory.toml` plus generated registry metadata covers every public API; `documentation-contracts.json` enumerates every primitive/package reference page and the small security-marker set. Generic validators report IDs/paths/fields, recursively check wiki indexing, and ignore ordinary prose. Tests never run the mutating registry updater.
- Focused commands select the group and then the original source-module prefix, for example `cargo test --test security package_loading::`. Full source mapping, measurements, and cleanup guidance live in `docs/development/build-and-test.md`.

## Plan 133 decomposition and test layout

Plan 133 (a behavior-preserving structural refactor) split the remaining
oversized files and moved the giant inline test modules to sibling files:

- `src/protocol/mod.rs` is a re-export hub over family files
  (`messages.rs`, `behavior.rs`, `editor_rules.rs`, `typography.rs`, `theme.rs`,
  `caret.rs`, `document.rs`, `shell.rs`, `launcher.rs`), so `crate::protocol::X`
  paths and wire bytes are unchanged — see
  [Protocol Codec](protocol-codec.md).
- `src/server/mod.rs` keeps the `IpcServer` struct, accept loops, socket/pipe
  binding, and cleanup; `src/server/runtime_state.rs` owns the generation stores
  and runtime-output application, and `src/server/runtime_reload.rs` owns the
  configuration/generation commit pipeline — see
  [Server IPC Skeleton](server-ipc-skeleton.md).
- `src/server/ui.rs` closed-choice validation is one `CHOICE_FIELDS` table plus
  `validate_choice` (same allowed sets, rule kinds, and messages) — see
  [Slot-Aware Package UI](slot-aware-package-ui.md).
- `src/shell/theme.rs` is a re-export hub over `src/shell/theme/parse.rs`,
  `src/shell/theme/validate.rs`, and `src/shell/theme/resolve.rs` — see
  [Editor Theme Registry](editor-theme-registry.md).
- 15 hand-written `enum + parse + as_str` triples now come from one
  `string_enum_impl!` declaration in `src/str_enum.rs`, pinned by the golden
  `string_roundtrip_per_enum` test.

Test layout. The four giant in-crate test modules are now directories whose
`tests/mod.rs` holds the module doc, imports, shared helpers, and the suite map:
`src/server/js_runtime/tests/` (24 suites), `src/server/connection/tests/` (14),
`src/client/tests/` (8), and `src/server/workspace/tests/` (9). The
`src/server/mod.rs` inline test mods moved to
`src/server/runtime_outputs_tests.rs`, `src/server/runtime_generation_tests.rs`,
`src/server/tab_server_state_tests.rs`, and `src/server/windows_tests.rs`;
`src/server/ui.rs`'s suite moved to `src/server/ui/tests.rs`, the syntax suite to
`src/server/syntax/tests.rs`, and the theme suites to
`src/shell/theme/tests.rs` and `src/shell/theme/theme_snapshot_tests.rs`. Owned
by `#[cfg(test)] mod tests;` declarations, they remain unit tests with
parent-private access — the integration root (`tests/suites/*`, a separate
crate) cannot host them. New `tests/*.rs` roots were deliberately not added, so
the suite inventory guard is unaffected.

`scripts/check.sh full` runs the root stages (audit, fmt, check, clippy
`-D warnings`, test, bench-compile) plus `desktop-clippy`, `desktop-test`, and
the generated-webview-binding guard. Plan-133 evidence and logs:
`test-plan/artifacts/133-file-decomposition/` and the plan-133 record in
`test-plan/index.md`.

## Plan 134 hygiene policies

Plan 134 (robustness/resource hygiene) established three repository-wide
conventions. Evidence and the per-site tables live under
`test-plan/artifacts/134-robustness/`; the implementation pages carry the
details.

**Lock poison policy (D5).** A `std::sync::Mutex` guard taken in a panicking
section poisons the lock. Self-healing state — registries, caches, snapshots,
counters, mailboxes, routing tables that a later write fully replaces or that
validate per entry — is locked through `src/lock_util.rs::LockOrRecover::lock_or_recover()`
and keeps serving; irreversible hand-offs keep a loud `expect` naming the
corruption (currently only the embedded tree-sitter `Parser`), and test-only
locks keep `expect` so tests fail loudly. Recovery never bypasses authority:
package allowlists fail closed on a miss, executing-package provenance is
re-resolved through the host's enabled set, and capability tokens are
server-issued and re-validated on use. Full class table and tests:
[Persistent Runtime Hardening](persistent-runtime-hardening.md).

**Async filesystem rule (P3).** Awaited production paths use `tokio::fs` or
`spawn_blocking`; a `std::fs` call left on a cold path must carry a ceiling
comment naming why it is not on the reactor (startup registration, JS-worker
threads, test hooks, the single `resolve_agent_type` stat). Error mapping and
authority checks are unchanged by the conversion. Sites and the remaining
ceiling list: [Server File Workspace](server-file-workspace.md).

**Float comparison policy (D4).** No `#[allow(clippy::float_cmp)]` anywhere;
every comparison is classified:

- *Identity* — round-trips, stored-then-compared values, parsed flags — compares
the pre-boundary source values or the bit pattern (`x.to_bits() == 0.0f32.to_bits()`),
not a float `==`. The runtime example is `src/shell/icons.rs::is_arc_flag`,
which accepts exactly `0.0`/`-0.0`/`1.0` and rejects near-flags.
- *Tolerance* — golden-value test assertions — uses one epsilon per type
  domain, `f32::EPSILON`/`f64::EPSILON`, as the house precedent in
  `src/shell/layout/mod.rs` and `src/editor/theme.rs` already did.
- *Floors* — contrast and density thresholds — stay exact `>=` comparisons
  (`TEXT_CONTRAST_MIN`/`UI_CONTRAST_MIN`), never an epsilon window.

Post-plan inventory: 81 unique `clippy::float_cmp` sites (3 runtime production,
78 test-side) → 0 under `cargo clippy --all-targets -- -W clippy::pedantic`.

Run `cargo test --test protocol primitives_docs` after wiki changes to re-check
the index/link guards.

## Plan 086 Release-Integrity Gate

Plan 086 keeps the Linux blocking gate serial and adds release-integrity evidence rather than weakening existing checks. The gate includes checked `rkyv 0.8.17` decoding, malformed-frame corpus coverage at `Codec::decode_frame`, AccessKit consumer validation for initial/incremental trees, hermetic configuration/Control Center workflows, dependency-warning documentation, and an environment-gated real AT-SPI smoke test. No audit ignore was added for the three directly fixed rkyv advisories; `cargo audit` reports zero vulnerabilities and the remaining unmaintained warnings are classified in `docs/development/security.md`.

The live test is an explicit integration source registered in the security suite because `Cargo.toml` sets `autotests = false`:

```bash
CLAY_LIVE_A11Y_SMOKE=1 cargo test --test security live_atspi_smoke::live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1
```

When the host lacks a usable AT-SPI bus, the test reports a prerequisite skip rather than a false pass. Consumer checks remain deterministic/blocking. Manual Linux evidence and host limitations are recorded in `test-plan/` and `docs/wiki/modules/plan086-release-integrity-and-accessibility.md`.

## Plan 060 Comprehensive Review Closure

The 2026-07-19 P0–P3 review is closed through executable evidence recorded in `plans/060-Comprehensive-Codebase-Review-Remediation.md`; documentation alone closes no behavioral finding. Final implementation areas and their detailed wiki owners are:

| Area | Implementation wiki |
|---|---|
| Two package runtime domains, adoption, provenance, extension consent, replacement, replay | [Embedded JavaScript Runtime](embedded-js-runtime.md), [Package Extension and Adoption Authority](third-party-runtime-authority.md) |
| Connection identity, authorized output routing, close/disconnect cleanup | [Server IPC Skeleton](server-ipc-skeleton.md), [Protocol Codec](protocol-codec.md), [Multi-Document Sessions](multi-document-sessions.md), [Parse Coordinator](parse-coordinator.md) |
| Bounded reads, atomic save identity, directory traversal/ignore grammar | [Server File Workspace](server-file-workspace.md), [Workspace File Browser](workspace-file-browser.md) |
| Bounded LSP actors and process I/O | [Language Server Process Service](language-server-process-service.md) |
| Single-source facades and public/configuration API closure | [Embedded JavaScript Runtime](embedded-js-runtime.md), [Clay JS Documentation Registry](clay-js-doc-registry.md), [Configuration Runtime](configuration-runtime.md) |
| Native dialog generations and clipboard limitation | [Client File Dialog](client-file-dialog.md), [Masonry Editor](../archive/masonry-editor.md) |

Routine dev/test profiles use line-table debug information, with full DWARF under `--profile debugging`. Consolidating 33 integration source modules into four suite roots reduced the clean all-target artifact snapshot from roughly 22 GiB/43 expected harness executables/89 s to 6.1 GiB/14 Cargo harnesses/68 s on the same Linux host; times are advisory, while suite inventory and artifact topology are deterministic. Runtime/resource ceilings remain compiled host policy rather than configuration. `cargo audit` currently passes with the documented unmaintained warnings and expiring quick-xml exceptions enforced by `audit_exceptions_are_documented_and_unexpired`.

## Plan 088 code-wiki maintenance

`tests/primitives_docs.rs::plan088_code_wiki_documents_modernization_contract` keeps the master wiki index and the implementation pages for theme, shell, retained UI, workspace labels, performance baselines, and review evidence aligned with the Plan 088 architecture. It is a deterministic content gate only; it never mutates the wiki. `wiki_index_links_every_wiki_page` remains the broader navigation guard.

Run `cargo test --test protocol primitives_docs` after wiki changes. The final Plan 088 wiki update documents cached theme/typography resolution, Clay-owned shell/package boundaries, clipping/accessibility semantics, responsive constraints, security/path sanitization, advisory-vs-blocking performance, and explicit visual-review blockers.

## Plan 089 validation and performance gates

Plan 089 adds three validation layers on top of the existing Linux blocking gate:

**`scripts/check.sh` wrapper.** The serial gate is now `scripts/check.sh full` (acquires `target/.clay-full-check.lock`, runs `cargo audit`, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo bench --no-run` in order, reports the failed stage on exit). `scripts/check.sh quick` runs `cargo fmt --check` then `cargo test --lib` (non-release fast path). `scripts/check.sh report` prints advisory `target/` size breakdowns without deleting or masking failures. CI invokes `scripts/check.sh full` as a single gate step. The lock prevents concurrent full runs from corrupting the shared `target/` tree.

**Bounded async-test helpers.** `src/server/runtime_generation_tests.rs::wait_until` polls every 10 ms with a 5 s deadline, panicking with scenario name, generation id, and runtime diagnostics codes on timeout. The four `configuration_watcher_*` poll loops were converted to `wait_until` to fix a production watcher race (the post-reload baseline scan adopted a change landing during the reload, absorbing the recovery write). The helper is test-only; production timeout sites (`connection/mod.rs` provider fallback, `js_runtime/mod.rs` reply waits) remain direct patterns.

**Build artifact reporting.** `scripts/check.sh report` prints `target/` total size, `debug/deps`, `debug/incremental`, and executable count. Advisory only; no cleanup or masking. The 50 GiB / 20 GiB cleanup thresholds and `cargo clean --profile dev --package clay` workflow are documented in `docs/development/build-and-test.md`.

**Criterion triage.** Plan 089 ran three fixed-input `window_baselines` passes (sample-size 10, warm-up 1 s, measurement 2 s) on an idle host and classified every group as machine variance except `centered_overlay` as benchmark instability (~0.25–0.55 ps below useful timer resolution). No reproducible implementation regression was found; no budget was raised; nothing was promoted to CI policy. The broad after-run shifts recorded in `docs/development/performance.md` are advisory evidence, not failures.

**Generated state-machine coverage.** Plan 089 added deterministic stdlib-generated tests (no property-test dependency) for protocol frame mutations (`compact_generated_frame_mutations_fail_closed_without_panicking`), chord state-machine transitions (`editor_generated_chord_sequences_preserve_prefix_mismatch_and_timeout_transitions`), and menu intent ordering (`generated_menu_intent_ordering_preserves_lifecycle_and_authority`). Each test uses a local `Lcg` split-mix generator seeded deterministically; every case asserts fail-closed behavior (rejection or safe validation, never panic). The strategy is compact deterministic coverage for bounded state spaces; a dedicated fuzzer is deferred until it finds value unavailable from these cases.

**Live platform validation.** `CLAY_LIVE_WINDOW_SMOKE=1 cargo test --test security live_atspi_smoke::live_multi_window_scale_smoke -- --ignored --exact --test-threads=1` launches two real Clay client processes on a Wayland host with AT-SPI prereqs, verifies both frames have positive physical bounds with scale factors between 0.5 and 4.0 via PID-separated AT-SPI identity, and confirms both status bars contain `Clay —`. The headless `rescale_event_recomputes_logical_bounds_from_physical_size` test sends `WindowEvent::Rescale(2.0)` plus `Resize(1800x1200 physical)` and asserts logical size remains 900×600. Safe window targeting requires the GNOME Shell extension (`can_query_windows=true`, `can_focus_windows=true`); blind portal input is never used.

Run `scripts/check.sh full` for the serial gate. Run `cargo test --test protocol primitives_docs` after wiki changes. Run `cargo test --test security rust_visibility_api_mapping` to verify the `#[doc(hidden)]` pub allowlist (exactly 4: reconcile bridge + shell widget methods) and benchmark-helper `pub(crate)` pins.

## Phase 28 manual test-plan execution

The Phase 28 manual plan is maintained in `test-plan/index.md` and modules
04, 08, 09, 10, and 11. The 2026-08-21 Linux rerun used
`cargo build --bin clay`, the isolated `scripts/capture-ui-review.sh` fixtures,
AT-SPI dumps, and xdg-desktop-portal screenshots. Fresh default, runtime-error,
recovery, and large-typography states passed their static shell/accessibility
checks under `code-reviews/screenshots/2026-08-21-phase28-manual/`.

Interactive rows remain explicitly unresolved where host tooling cannot prove
them: the completion/Command Centre harness requires a TTY for keyboard
capture; the editor editable-text interface, compositor targeting, link
pointer activation, and GUI `lsp-shared` worker remain known ceilings. The
plan does not promote structural tests to live passes. Focused transform,
keymap, completion-ranking, folding/inlay-budget, completion-payload, and
package-conformance tests plus `cargo test --all-targets --no-fail-fast` remain
the automated companion gate.

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
- `.agents/skills/clay-execution/references/protocol-perf.md`
- `.agents/skills/clay-execution/references/packages.md`
- `docs/development/build-and-test.md`
- `Cargo.toml`
- `tests/suites/`
