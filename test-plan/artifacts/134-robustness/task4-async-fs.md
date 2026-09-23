# Plan 134 task 4 — P3 hot-path blocking fs moved off the reactor (2026-09-22)

Baseline inventory: `baseline-fs-inventory.txt` (task 1). Policy applied:
blocking `std::fs` on the connection reactor → `tokio::fs`; heavy/non-tokio
work (or whole sync helper chains) → `tokio::task::spawn_blocking`; worker-thread
and startup paths stay `std::fs` with a ceiling comment.

## Moved to `tokio::fs` (reactor path)

`src/server/workspace/mod.rs` — open/save/reload path:

- `canonical_file_state` (canonicalize + metadata, both root authorities) →
  async; callers `prepare_open_existing`, `register_loaded_file` await.
- `reauthorize_open_file` → async; `prepare_save` (now async) and
  `prepare_reload` await. Callers `save_document`/`save_document_unlocked` await.
- `canonical_selected_file` → async; `prepare_open_selected` awaits.
- `contained_existing_path` → async; callers in `agent_checkpoints.rs`,
  `agent_documents.rs` await.
- `contained_new_file_path` → async (ancestor walk uses `tokio_fs::canonicalize`);
  callers in `agent_documents.rs` await.
- `matches_current` → async; `atomic_write_chunks` awaits.
- `discover_root_for_path` → async (`tokio_fs::canonicalize`/`metadata`, marker
  probes via async metadata); caller `op_clay_workspace_discover_root_for_path`
  awaits.
- `add_explicit_user_grant` → async; caller `op_clay_workspace_add_root` awaits.
- `add_root` split: sync startup wrapper keeps `std::fs`; new async
  `add_root_async` + pure `add_root_state` (validation/dedup/insert) serve the
  reactor callers.
- `atomic_write_chunks`: pre-write read-only mode check, original-permissions
  capture, and `set_permissions` now `tokio_fs` (save path was the remaining
  sync straggler beside `tokio_fs` chunk writes).

`src/server/command_execution.rs`:
- `OPEN_DIRECTORY` command now builds the listing plan under the workspace guard
  and runs `traverse_directory` on `spawn_blocking` — the previous direct
  `WorkspaceState::list_directory` ran a recursive `std::fs` walk on the reactor.

`src/server/connection/runtime.rs`:
- `persist_settings_change`: the whole root-resolution + preferences read/write
  step runs in one `spawn_blocking` closure (this also covers
  `effective_configuration_root`'s `canonicalize`/`is_file`).

`src/server/runtime_reload.rs`:
- `enumerate_ui_choices` is async; the persisted-appearance probe (config-root
  canonicalize + preferences read + `effective_configuration_root`) moved to new
  `persisted_appearance(server)` on the blocking pool. The package-service guard
  is dropped before the await (no std guard held across await).

`src/server/launcher.rs` (called from async connection/tab handlers):
- `read_store`, `write_store`, `record_recent_workspace`,
  `remove_recent_workspace`, `launcher_entries`, `list_agents`, `count_skills`
  → async `tokio::fs` (recents temp+rename write keeps the same crash-safety).
- Callers in `connection/tabs.rs`, `connection/workspace.rs`, `server/mod.rs`
  await; launcher tests are `#[tokio::test]`.

## Remaining `std::fs` sites and why (ceiling)

- `configuration.rs`: module doc added. All sync sites
  (`from_config_root`, `load_module_source`, `load_preferences`,
  `write_preferences`, `canonical_local_file`,
  `validate_local_module_path_allow_missing`, one-time legacy migration) run on
  the embedded-JS runtime worker thread or at startup; reactor callers wrap them
  in `spawn_blocking` (the two above). No user-visible change.
- `ops/packages.rs`: module doc added. Package resolution / loadEntry scans run
  on the JS worker thread (op execution / module loading), bounded by the
  runtime mailbox lanes.
- `workspace/mod.rs`: module doc added — `add_root`/`add_root_from_cwd` are
  startup-only (`IpcServer::try_new`); `traverse_directory` and helpers
  (`read_auxiliary_file_bounded`, `count_visible_children`) only run on the
  blocking pool; `FileMetadata::capture` has no production caller; `#[cfg(test)]`
  hooks stay sync.
- `launcher.rs`: `resolve_agent_type` kept sync with an inline note — one bounded
  `is_dir` stat, sync `agent` caller.
- `server/mod.rs`: `spawn_configuration_watcher` reads
  `effective_configuration_root` at startup only.
- `agent_documents.rs` / `ops/theme.rs`: already `tokio::fs` on all production
  paths (task-1 finding), unchanged.

## Error mapping and authority (unchanged)

- Every moved call keeps its existing `map_err` into
  `WorkspaceError::{RootUnavailable, FileUnavailable, ...}`,
  `UserBrowseError::Unavailable`, `ConfigurationError::{Root, ReadModule}` and
  the same `settings.*` messages. `tokio::fs` returns the same `io::Error`/`Metadata`.
- Permission checks still precede reads: canonical containment (`starts_with`
  root / single-file grant equality) and `validate_regular_file_metadata` run on
  every async path exactly as before; `atomic_write_chunks` keeps the read-only
  target check and fail-closed permission preservation (now async).
- `spawn_blocking` closures own only cloned paths/arguments/`IpcServer` handles;
  no workspace lock or std guard is held across an await.

## Tests

- New: `open_existing_file_reports_file_unavailable_when_root_disappears`
  (`workspace_roots.rs`) — async open path maps a vanished root through
  `FileUnavailable`; `explicit_user_grant_rejects_missing_path` now exercises the
  async root path and still asserts `RootUnavailable`.
- Converted to async where the API became async: workspace root tests, launcher
  store/agent tests, `save_concurrency` prepare test.
- Targeted: workspace 84, launcher 15, connection 96, runtime_reload 2,
  configuration 71, ops 88, command_execution 27 — green.
- Pedantic regression check: `clippy::float_cmp` still 0
  (`task4-clippy-pedantic-float-cmp.json`).
- Full gate: see `task4-exit-codes.txt`.
