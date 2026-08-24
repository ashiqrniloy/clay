# Driver Module Map (tab subsystem, Phases 22.7–22.8)

> **Historical — removed in Plan 097 Phase 12.** The native client driver was
> deleted; tab lifecycle, reconnect, and layout persistence now live in the
> Tauri bridge and React workspace controller:
> [Tauri/React Desktop Cutover](tauri-react-cutover.md),
> [React Tabs, Splits, and Layout Persistence](react-tabs-and-splits.md).

The app driver is the client-side orchestrator that owns the multi-connection
tab model: per-tab state, tab lifecycle (mount/switch/close/reconnect),
server registry reconciliation, the restore state machine, and window-state
persistence orchestration. Phase 22.7 (2026-08-09) extracted it out of
`src/main.rs` (6194 → 3603 lines) into `src/driver/` so the crate root keeps
only window/run-loop/dialog/CLI concerns. Phase 22.8 keeps this orchestration
client-local while binding every restored/reconnected connection to its own
server `TabServerState` before document/SDUI installation. Plan 090 (task 7)
then split the remaining crate-root concerns out of `src/main.rs` into three
sibling modules — `src/cli.rs` (CLI parsing), `src/launch.rs` (server/client
startup, window creation, lifecycle), and `src/app_driver.rs` (app event
dispatch + native dialog/action routing) — leaving `src/main.rs` as a thin
composition root (`main()` + the collocated test module).

## Module layout

| File | Contents |
|------|----------|
| `src/driver/mod.rs` | `Driver` struct + `TabState` + `PendingOpenRequest` + `TabRegistryReconcile`, tab lifecycle methods (`mount_tab`, `switch_tab`, `close_tab`, `reconnect_tab`), the per-tab event bridges (`spawn_client_connection_event_bridge` / `connection_event_user_event`), `RESTORE_CONFIRM_TIMEOUT`, typed access helpers (`with_shell` / `with_editor` / `with_view`), free fns (`take_pending_open_for`, `ordered_tab_clients`, `open_intent_pending_request`, `advance_pending_close_after_saves`, `tab_card_display_name`), `impl Driver` blocks spanning the three files, and `tests` |
| `src/driver/reconcile.rs` | Registry reconciliation: `accept_registry_snapshot`, `apply_tab_registry` (pure fill/removal/active/card computation), `apply_registry_reconcile` (shell-side uninstall/switch/focus/card refresh), `ordered_tab_clients`; focused registry tests |
| `src/driver/restore.rs` | Restore state machine (`advance_restore`, `mount_restored_tab`, `reopen_restored_documents`, `finish_restore`, `abandon_restore`) + `persist_window_state` (the `layout.json` v2 writer) + dirty-inventory/close-confirm helpers (`dirty_documents_in_tab`, `collect_window_state`, `show_tab_close_confirm_menu`, `save_all_then_close_tab`); focused restore tests |

`src/app_driver.rs` keeps the crate-root-side app concerns that were not
moved into `src/driver/`: window/run-loop/`DriverCtx` plumbing, `run_editor`
(now in `src/launch.rs`), native dialog reservation (`reserve_folder_dialog` /
`finish_folder_dialog`), menu sync (`apply_menu_sync`), `connect_with_retry_while`
(now in `src/launch.rs`), `is_linux_portal_dialog_command`,
`apply_connection_to_chrome`, and the `EditorAction` dispatch arms that route
into `src/driver/` methods (including the one-line `self.reconnect_tab(...)` arm).

## Visibility rules

Because `main.rs` is the crate root (an ancestor of `driver`), ancestors
cannot reach descendant-private items: every moved type, field, method, and
free fn the root calls is `pub(crate)` (`Driver`, `TabState`,
`PendingOpenRequest`, `RESTORE_CONFIRM_TIMEOUT`, the bridge fns, the typed
helpers). Crate-root-private items (`apply_connection_to_chrome`,
`apply_menu_sync`, dialog fns, `connect_with_retry_while`) are `pub(crate)`
in `src/app_driver.rs` / `src/launch.rs` so the sibling modules and the
`main.rs` test module can reach them. Shared test helpers
(`test_driver_with_tabs`, `tab_state_with_queue`, `tab_snapshot`) are
`pub(crate)` in `driver::tests` so `driver::reconcile::tests`,
`driver::restore::tests`, and `main.rs` tests reuse them.

## Typed access helpers

The ~27 `render_root().edit_widget(id)` + `try_downcast::<W>()` boilerplate
sites that moved with the driver were converted to three typed helpers:

```rust
pub(crate) fn with_shell<R>(
    render_root: &mut RenderRoot, id: WidgetId,
    f: impl FnOnce(&mut ClayShellWidget, &mut MutateCtx) -> R,
) -> Option<R> { /* edit_widget + try_downcast::<ClayShellWidget> */ }
// with_editor / with_view are the same shape for EditorWidget / PaneDocumentView
```

- Pattern A (shell `Option`-return): `with_shell(...).unwrap_or(default)`.
- Pattern B (shell side-effect): `with_shell(...)` with a `|shell, ctx|` closure.
- Pattern D (either-or editor/view branches):
  `with_editor(...).or_else(|| with_view(...))` — with `.flatten()` where the
  closure itself returns `Option` (e.g. `editor_widget_id_for`,
  `active_pane_target_for`, `take_pending_menu`).

Contract: `None` means the downcast failed; a MISSING widget id still panics
via masonry `edit_widget` (unchanged from the pre-extraction call sites —
the connection-owner contract keeps owner ids in the tree).

No logic changed in the extraction — the diff is moves + helper calls, and
the sorted `cargo test --bin clay` test-name set is byte-identical to the
pre-move baseline (62 bin tests).

## Driver state

- `Driver.tabs: BTreeMap<ClientId, TabState>` — keyed by `ClientId` (the
  connection id), not `TabId` (the server-assigned id arrives
  asynchronously via the registry snapshot). `TabState` = `ClientEditQueue`
  clone, per-tab `pending_opens`, the known `tab_id`, and the workspace
  root display path.
- `Driver.active_tab: ClientId`, `Driver.registry: TabRegistrySnapshot`
  (latest), `folder_dialog_in_flight`, `pending_close_after_saves`,
  `restore_gate: Option<(ClientId, Instant)>` (restore confirmation
  deadline, `RESTORE_CONFIRM_TIMEOUT`).

## Phase 22.8: tab-bound restore and reconnect

The driver does not select or expose arbitrary server tabs. At startup,
`run_editor` chooses each persisted tab's validated `workspace_root` before
`client::connect_with_workspace_root` performs the handshake-bound `New`.
During a live connection drop, `start_tab_reconnect` retries
`client::connect_for_reclaim_or_new`: `Reclaim` preserves the existing
`TabServerState`; an unknown/stale `TabId` falls back to `New` with the
persisted root. `reconnect_tab` swaps the fresh queue into the existing
`TabChrome` and pane views, reopens retained documents through ordinary
`OpenDocument`, and preserves the split tree without reviving old leases or
selected-file capability tokens.

`accept_registry_snapshot` treats an empty server registry as a reset: it
clears stale client-side `TabId`s without unmounting still-mounted tabs, so
partial replacement `New`/`Reclaim` snapshots cannot look like tab closes.
All restore/reconnect work is bounded by the existing tab/session caps and
runs outside typing/paint/layout hot paths. Server authority remains in
`TabRegistry`/`TabServerState`; the driver only owns view routing and
persistence state.

Tests include `real_server_reconnect_reclaims_tab_binding`,
`real_server_restart_rebuilds_reconnect_from_persisted_workspace_root`,
`accept_registry_snapshot_rejects_stale_relays_and_resets_on_restart`, and
`apply_tab_registry_skips_removals_on_empty_snapshot`.

## Tests

- `cargo test --bin clay --quiet` — 62 tests across `main.rs` (dialog
  routing, command-ID routing, smoke) and `driver` (`mod` 23, `reconcile`
  7, `restore` 4 — restore tests live only in `driver::restore::tests`).
- Reconcile/restore tests import the shared helpers from
  `driver::tests`; `restore.rs` defines `persisted_tab_state` locally;
  `reconcile.rs` defines `registry_snap` locally.

## Related

- [Tabs and Independent Client Views](tabs-and-clients.md) — the
  multi-connection model the driver orchestrates.
- [Masonry Shell Runtime](masonry-shell.md) — the shell side
  (`TabChrome` map, tab bar) the driver drives.
- [Multi-Document Sessions](multi-document-sessions.md) — reconnect
  document identity retention.
- [Pane Document Views](pane-document-views.md) — per-pane views the
  driver routes document-scoped events to.
