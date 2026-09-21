# Plan 133 task 3 — `src/server/mod.rs` runtime assembly / accept loops / tests split

Task: "Split `src/server/mod.rs`: runtime assembly vs. accept loops vs. tests".
Pure move: no logic edits in the moved code (enumerated plumbing changes only).

## Result

`src/server/mod.rs`: **6,203 → 1,028 lines** (< the ~1,500 target).

| File | Lines | Contents |
| --- | --- | --- |
| `src/server/runtime_state.rs` (new) | 525 | `ServerConfig` (+impl), `RuntimeGeneration` (+impl), `RuntimeGenerationStore` (+impl), `ActiveRuntimeStateFanout` (+`Default`, impl), `ActiveTypographyState` (+`Default`, impl), `shell_command_catalogue`, `RuntimeOutputApplication` (+impl), `apply_runtime_outputs`, `apply_runtime_outputs_without_sdui` |
| `src/server/runtime_reload.rs` (new) | 1,165 | `effective_agent_root`, `RuntimeGenerationCandidate`, `ReloadedDocumentRefresh`, `RuntimeReloadOutcome`, `register_runtime_contributions`, `cancel_older_runtime_generations`, `withdraw_package_contributions`, `stage_typography`, `build_runtime_state_snapshot`, `runtime_candidate_error`, and `impl IpcServer` { `new`, `try_new`, configuration load, diagnostic recording, `trigger_developer_hot_reload`, `arm_reload_candidate_barrier`, `execute_reload_command`, the reload pipeline (`reload_runtime_generation*`, `prepare_runtime_generation_candidate`, `validate_runtime_registrations`, `commit_runtime_generation`, `refresh_open_documents_after_reload`), `enumerate_ui_choices` } |
| `src/server/runtime_outputs_tests.rs` (new) | 225 | `runtime_outputs_tests` module (4 tests) |
| `src/server/runtime_generation_tests.rs` (new) | 3,070 | `runtime_generation_tests` module (41 tests, incl. its column-0 JS fixture string) |
| `src/server/tab_server_state_tests.rs` (new) | 127 | `tab_server_state_tests` module (1 test) |
| `src/server/windows_tests.rs` (new) | 67 | `windows_tests` module (1 test, `#[cfg(all(test, windows))]`) |

Kept in `mod.rs` per the task: module decls + prelude, `IpcServer` struct, tab-state
plumbing (`new_tab_state` … `spawn_configuration_watcher`), `run` /
`accept_unix_loop` (all three platform variants), `spawn_connection`,
`sweep_expired_tabs`, `LiveClientGuard`, Unix socket + named-pipe binding and
validation (`bind_unix_listener`, `validate_socket_path`,
`validate_parent_directory_*`, `remove_stale_socket`, `create_named_pipe_server`,
`CurrentUserSecurityAttributes`, …), `ServerError`, `ReloadCandidateBarrier`,
`JS_RUNTIME_TEST_LOCK`.

## Test placement — deviation from the task text, with reason

The task expected the inline suites in `tests/suites/server_runtime.rs` (or an
existing suite). They cannot be integration tests: they exercise crate-internal
items (`apply_runtime_outputs` is `#[cfg(test)]`, `create_named_pipe_server` and
the socket helpers are private, several suites construct `IpcServer` and private
`RuntimeGeneration*` fields), and an integration root is a separate crate that
cannot reach `pub(crate)`/private items. They therefore moved to sibling
`<module>_tests.rs` unit-test files, matching the repository precedent
(`src/server/tests.rs`, `src/server/connection/tests.rs`,
`src/server/workspace/tests.rs`), declared from `mod.rs` under the same
`#[cfg(test)]` / `#[cfg(all(test, windows))]` gates as before. No new
`tests/*.rs` root was added, so the suite inventory guard is untouched.

## Method and verbatim check

Extraction script kept for provenance:
`task3-split-server.py` (parses top-level items with attribute/doc prefixes and
the method groups inside the two `impl IpcServer` blocks, then cuts those line
ranges). New modules start with a module doc and `use super::*;`; the inline test
modules were dedented one level and reflowed by `cargo fmt`.

- 44/44 moved production blocks found as order-preserving subsequences of the
  new files (stripped lines, ignoring the added preludes/`impl IpcServer {`
  wrappers).
- Test bodies byte-identical before rustfmt (228 / 3,075 / 135 / 67 lines) and
  token-equal after rustfmt, apart from the intentional import edits below.
- Token-multiset diff of all removed ranges vs the new files: only the expected
  deltas — 4 `mod … {` wrappers + their `#[cfg]` attrs, the 19 added
  `pub(super)` keywords, `use`-statement rewrites, module docs.

## Plumbing edits (the only non-move diffs)

- `src/server/mod.rs`: `mod runtime_reload;`, `mod runtime_state;`; re-exports
  `pub use runtime_reload::{ReloadedDocumentRefresh, RuntimeReloadOutcome};`,
  `pub use runtime_state::ServerConfig;`,
  `pub(crate) use runtime_state::{RuntimeGeneration, RuntimeGenerationStore, apply_runtime_outputs_without_sdui};`
  plus `#[cfg(test)] mod runtime_outputs_tests;` / `runtime_generation_tests` /
  `tab_server_state_tests` and `#[cfg(all(test, windows))] mod windows_tests;`.
- `pub(super)` on items now shared across module boundaries (previously
  module-private in the `server` subtree — same effective visibility):
  `RuntimeGeneration::{id, service, evaluation, diagnostics}`,
  `RuntimeGenerationStore::{current, typography, runtime_state, behavior_grace}`,
  those stores' `initial` / `push_diagnostic` / `swap`,
  `ActiveTypographyState::{current, updates}`, `RuntimeGenerationCandidate`,
  and `IpcServer::{load_default_configuration, load_configuration_for_service,
  prepare_runtime_generation_candidate, commit_runtime_generation}`.
- `src/server/connection/tests.rs`: five `super::super::…` paths for the moved
  items now read `crate::server::runtime_state::…`.
- The two moved test mods import their moved helpers from
  `crate::server::runtime_state` / `crate::server::runtime_reload`.
- `tests/performance_budgets.rs`: `is_sibling_test_file` now also accepts
  `<module>_tests.rs` siblings (it scanned the extracted test files as production
  code and false-positived on their folding-range fixtures; those files are
  declared `#[cfg(test)] mod …_tests;`).

## Metrics

- `src/server/mod.rs` 6,203 → **1,028 lines**; moved test suites 4 modules /
  47 test attributes (4 + 41 + 1 + 1), all module and test fn names unchanged.
- Test totals identical to the task-1 baseline: **1996 passed / 0 failed /
  1 ignored**; `scripts/check.sh full` **exit 0** in 281 s, clippy
  `-D warnings` clean, `cargo fmt --check` clean, desktop + bindings stages green.
- Compile-time A/B (incremental `cargo check --lib` after touching
  `src/server/mod.rs`): 4.85 s / 4.94 s on the split tree vs 5.43 s / 4.83 s on
  the pre-change tree (first sample ≈17 s on both sides from the fingerprint
  change) — neutral.
- Security: socket/permission validation and `ServerError` stayed in `mod.rs`
  untouched; all moved code is verbatim (check above), so no gate logic was
  altered.
