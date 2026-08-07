# Tabs and Independent Client Views

The Phase 22.3 multi-connection model: each tab is an independent client
view with its own server connection, split tree, and document sessions. This
page covers the server-authoritative tab registry, the protocol messages,
the client-side multi-connection driver, event routing per tab, reconnect
and reclaim, and the isolation invariants. The shell chrome (tab bar,
inactive-tab retention, per-tab `TabChrome`) lives in
[Masonry Shell Runtime](masonry-shell.md); per-pane document hosting in
[Pane Document Views](pane-document-views.md); reconnect session restoration
in [Multi-Document Sessions](multi-document-sessions.md).

## Source

- `src/server/tab_registry.rs` — server-authoritative in-memory registry.
- `src/protocol/mod.rs` — `TabId`, `TabEntry`, `TabRegistrySnapshot`,
  `TabCommand`, `ClientMessage::TabCommand`, `ServerMessage::TabRegistry`
  (protocol v12).
- `src/server/mod.rs` — `IpcServer` owns the registry + broadcast sender.
- `src/server/connection.rs` — handshake replay, `TabCommand` dispatch,
  close-terminates-connection, reconciliation snapshots.
- `src/main.rs` — `Driver`: `TabState` map, `mount_tab`, `switch_tab`,
  `apply_tab_registry`/`apply_registry_reconcile`, per-tab event bridges,
  `start_tab_reconnect`, `ReconnectTabConnected` handler, `guard_tab_close`,
  `tab_close_allowed`, bootstrap `TabCommand::New`, `NewTab` affordance flow.
- `src/client/mod.rs` — `ClientEditQueue::enqueue_tab_command`;
  `ClientConnectionEvent::TabRegistry`.
- `src/masonry_shell.rs` — per-tab chrome (`TabChrome`) and tab bar (see
  masonry-shell page).
- `src/masonry_pane_document.rs`, `src/editor/document_session.rs` —
  reconnect: per-session `workspace_root_id`/`path` retention,
  `documents_for_reopen`, `PaneDocumentView::reconnect`.

## Overview

Before 22.3 the app had exactly one connection (one client view). 22.3 makes
the tab the unit of client views: `N` tabs = `N` connections to one server
process. The server keeps an in-memory registry of tabs (order, active id,
per-tab workspace + client binding) and broadcasts snapshots so every
connection converges on the same tab list. The client shell hosts every
tab's chrome in one window and shows the active tab's working area; the
driver routes each connection's events to its own tab's chrome.

Why this shape: tabs must be isolated like separate clients (independent
edit queues, leases, split trees, modes), but share one window and one
server. The authority boundary from earlier phases is untouched — each
connection still holds its own capability tokens, document leases, and
workspace grants; the registry only binds already-authorized connections.

## Server side: TabRegistry and protocol

- `TabRegistry` (`src/server/tab_registry.rs`) is `Arc<Mutex<...>>` on
  `IpcServer`; entries are `TabEntry { tab_id, workspace_root_id, client_id,
  workspace_root }`. `tab_id` and `client_id` are both `u64` newtypes
  (`TabId` follows the `ClientId = u64` alias pattern in
  `src/protocol/mod.rs`).
- Tab commands arrive as `ClientMessage::TabCommand` (v12):
  - `New { workspace_root }` — creates a tab bound to this connection; the
    root path is validated through the shared `WorkspaceState::add_root`
    (canonicalization, directory check, dedupe, `MAX_WORKSPACE_ROOTS`), which
    doubles as the structural gate (the message is Clay-owned, so no
    capability token is required).
  - `OpenWorkspace { workspace_root }` — adds a workspace root to the
    sending tab's workspace and renames the entry.
  - `Activate { tab_id }` — makes the tab active; `Close { tab_id }` —
    removes the tab and **terminates that tab's connection** (the closing
    connection exits its handler loop, releasing its connection permit and
    per-connection document leases through the existing disconnect cleanup
    path). `Reclaim { tab_id }` — rebinds a surviving registry entry to the
    calling connection (reconnect path: keeps `TabId`, rebinds `ClientId`).
- Every `Activate`/`Close`/`OpenWorkspace` attempt pushes a fresh
  `TabRegistrySnapshot` on the broadcast lane **even when rejected** — that
  is the reconciliation signal that lets an optimistically-switching client
  revert.
- Handshake: each connection subscribes to the registry lane and gets the
  current snapshot replayed right after the welcome snapshot + manifest.
  Snapshot broadcasts happen on the lane; a lagged receiver re-subscribes
  and re-reads the current state.
- Connection cap: the existing `MAX_ACTIVE_CONNECTIONS = 64` permit gate
  applies to tabs too — the 65th tab's `connect` fails cleanly
  (`TransportUnavailable`), surfaced as a `clay.tabs.open_failed`
  diagnostic; no half-mounted tabs.

## Client side: the multi-connection Driver

- The `Driver` owns `tabs: BTreeMap<ClientId, TabState>` — keyed by
  `ClientId`, **not** `TabId`, because the server-assigned `TabId` arrives
  asynchronously via the registry snapshot (at mount time the client only
  knows its connection id). `TabState` holds the `ClientEditQueue` clone,
  the per-tab `pending_opens`, the `tab_id` once known, and the workspace
  root display path (used for tab-bar labels before the registry entry
  arrives).
- The shell mirrors this as `tabs: BTreeMap<ClientId, TabChrome>` +
  `active_tab`; see the masonry-shell page for `install_tab`,
  `set_active_tab`, `tab_for_chrome`, and zero-size inactive retention.
- `mount_tab` (new-tab flow): connect a fresh session → send
  `TabCommand::New` on it → install + activate the chrome → spawn the
  per-tab event bridge. The bootstrap tab is created the same way in
  `run_editor` (reads `workspace_root` from `ClientInitialState`, added in
  v12, and sends `New` explicitly) — the server's replay snapshot then
  matches what the client mounted.
- Event routing: each tab's bridge tags events with that tab's chrome
  `WidgetId`; the driver resolves the tab via `tab_for_chrome` and routes
  document-scoped events, fan-outs, runtime snapshots, and editor commands
  to that tab's targets only. `ShellPreferences` apply to the sending tab's
  pane-focus policy; `TabRegistry` snapshots are driver-level state.
- `apply_tab_registry` is a **pure** reconciliation (fills `tab_id`s,
  computes removed tabs, new active, and the card list); the shell side
  `apply_registry_reconcile` uninstalls removed chromes (removing their
  pane-host pods and orphans from the widget tree), switches + focuses the
  survivor, and refreshes the tab bar. Removals are skipped against an
  empty snapshot (server restart; the lifecycle task re-registers).

## Lifecycle

- **Open**: `+` affordance → native folder picker → async `client::connect`
  → `OpenTabConnected` → `mount_tab`. Failure surfaces a runtime diagnostic
  on the active tab; nothing is mounted.
- **Close**: card `✕` / `Ctrl+Shift+W` → `tab_close_allowed` (never the
  last tab) → `close_tab`: clean tabs enqueue `TabCommand::Close`; dirty
  tabs get the Phase 22.4 driver-owned confirm menu (Save all and close /
  Discard and close / Cancel — see Keyboard Management below) → server
  removes the entry and ends the connection. The closing connection never
  reads its own broadcast removal (its handler returns first) — other
  connections observe the removal snapshot.
- **Switch**: card click → optimistic `switch_tab` + `TabCommand::Activate`
  → server snapshot reconciles (a rejected activate pushes the real state,
  reverting the optimistic switch).
- **Reconnect**: on `Disconnected`/`ConnectionError`, `start_tab_reconnect`
  spawns a per-tab task retrying `client::connect` (existing backoff,
  50 × 20 ms then 200 ms per cycle) until success or the tab is removed
  (per-tab `Arc<AtomicBool>` cancellation set by `apply_registry_reconcile`).
  On success the `ReconnectTabConnected` handler: swaps the fresh session's
  queue into the chrome and every pane view (`PaneDocumentView::reconnect` —
  clears the disconnect menu and re-arms the reinstall so the active
  document is not swallowed by the 22.2 duplicate-open no-op); re-keys the
  tab (`rekey_tab` — chrome moves wholesale, widget ids stable); rebinds the
  registry entry (`TabCommand::Reclaim { tab_id }`, or `New` if the tab
  never got its id); re-opens every retained document through the plain
  `OpenDocument` path (a fresh connection holds no selected-file capability
  token, so `OpenSelectedFile` is unusable; `ClientEditQueue::
  enqueue_open_document` sends `OpenDocument` with the retained
  `workspace_root_id` + path); spawns a new event bridge; restores focus if
  the tab was active. In-flight `pending_opens` died with the old
  connection and are cleared.
- **Client restart reclaim**: the registry is in-memory on the server, so a
  local client process restart reconnects per tab and `Reclaim`s the
  entries — tabs survive client restarts but not a full server restart
  (disk persistence is 22.5).

## Keyboard Management (Phase 22.4)

- **Command IDs**: 24 Clay-owned `client_ui` IDs — `clientTabNext`/`Prev`/`New`/`Close`/`MoveLeft`/`MoveRight` plus dotted families `clientTabActivate.1..9` and `clientTabMoveTo.1..9` — declared in `default_commands()`/`default_keymaps()` (Global scope), allow-listed + routed in `src/server/ops/keybindings.rs` (`tab_family_variant` accepts `1..=9` only), mapped to `ShellClientCommand` variants, exported by `runtime/js/shell.js`, and user-rebindable via `bindKey`/`unbindKey` (`{ scope: "global" }`).
- **Driver dispatch**: `handle_client_ui_command` returns `ShellCommand`; the driver's `apply_tab_command` intercepts tab commands before the shell widget (whose tab arms stay inert), resolving positions from the card order (`tab_order`/`tab_position_of`/`tab_at_position`/`tab_at_offset`) and routing through the shared execution paths the tab bar also uses.
- **Policies**: 1-based card numbering; `Activate.N` silent no-op beyond tab count; no variants beyond 9 (`Ctrl+0` unbound); next/prev wrap around; move left/right no-op at ends (no wrap); `MoveTo.N` no-op beyond count; last tab cannot close; `+` and `Ctrl+T` share the new-tab flow (in-flight guard).
- **Server reorder**: `TabCommand::MoveLeft/MoveRight/MoveTo` → `TabRegistry::move_left/move_right/move_to` (protocol v13); every mutation broadcasts a snapshot on acceptance **and** rejection; active-tab status is preserved by `TabId`.
- **Dirty close**: `close_tab` inventories dirty panes (`dirty_documents_in_tab`) instead of the old `guard_tab_close` walk; a dirty tab gets a driver-owned confirm session (`clay::shell::tab_close_confirm_session` — Save all and close / Discard and close / Cancel) hosted on the active pane view + chrome overlay. Save all tracks awaited `DocumentId`s in `pending_close_after_saves`; `advance_pending_close_after_saves` counts `DocumentSaved` acks and enqueues `TabCommand::Close` only after all ack, cancelling on `FileOperationFailed`/disconnect. Discard enqueues close immediately; Cancel clears the session. The menu action IDs (`clay.shell.clientTabClose*`) are driver-local, never declared/routed, so they cannot cross-route with per-view save-conflict menus.

## Invariants and Constraints

- One connection per tab: separate `ClientEditQueue`/sync state, chrome,
  split tree, pane targets, focus policy, pending-opens. Typing in one tab
  never mutates another tab's state (edit queues are per-tab channels).
- The client tab map is view/routing state and grants nothing; the server
  registry is authoritative for tab order, active id, and bindings.
- Tab ops grant no filesystem/network/extension authority — they ride the
  existing per-connection capability/lease path; `Reclaim` only rebinds a
  surviving entry to a fresh handshake-authenticated connection.
- The window never drops below one tab; a dirty tab can only close through
  its save-conflict resolution.
- `layout.json` persistence saves only while exactly one tab is open
  (per-tab persistence is 22.5).

## Known Ceilings

- No disk persistence of the registry or per-tab split trees (22.5); a full
  server restart resets to the single bootstrap tab.
- No multi-client tab reclamation (Phase 21); no per-tab package chrome.
- Single-tab behavior matches pre-22.3 exactly (no tab bar, no per-tab
  overhead visible).

## Tests

- `src/server/tab_registry.rs`: registry unit tests (incl. 22.4 reorder:
  valid moves, boundary no-ops, bound-client validation, position bounds).
- `src/server/mod.rs` + `src/server/connection.rs`: handshake replay order,
  `TabCommand` dispatch (incl. move variants: reorder broadcast + rejection
  snapshots), rejected-command reconciliation snapshots,
  close-terminates-connection.
- `src/client/mod.rs`: real-server tab end-to-end — bootstrap `New`
  registers the tab (name/order in snapshot), rejected `Activate` pushes a
  reconciling snapshot, `Close` ends the connection + removes the entry,
  dropped connection's entry reclaimed by a new connection, connection-cap
  refusal, tab move commands reorder/broadcast/reject. Commands:
  `cargo test --lib real_server_tab --quiet`,
  `cargo test --lib real_server_ --quiet`.
- `src/main.rs` (bin): registry reconciliation (fills ids + builds cards,
  removes closed tabs + activates survivor, skips removals on empty
  snapshots), per-tab edit-queue isolation, `tab_close_allowed`, 22.4
  policy resolvers (`tab_order`/`tab_at_position`/`tab_at_offset`),
  move enqueues + boundary no-ops, command-ID routing, tab-confirm menu
  naming, `advance_pending_close_after_saves` bookkeeping. Command:
  `cargo test --bin clay --quiet`.
- `src/masonry_shell.rs`: install/switch/retention/rekey/zero-size-layout
  tests (see masonry-shell page). Command:
  `cargo test --lib masonry_shell --quiet`.

## Related

- [Masonry Shell Runtime](masonry-shell.md) — tab chrome, tab bar, zero-size
  inactive retention, per-tab routing queries.
- [Multi-Document Sessions](multi-document-sessions.md) — reconnect document
  identity retention and re-open.
- [Pane Document Views](pane-document-views.md) — per-pane views and the
  close guard reused for dirty tabs.
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md) — connection
  bootstrap each tab reuses.
- [Document Leases and Region Locks](../flows/document-leases-and-region-locks.md)
  — the per-connection authority path tab ops ride on.
