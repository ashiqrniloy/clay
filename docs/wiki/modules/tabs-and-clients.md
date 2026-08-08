# Tabs and Independent Client Views

The Phase 22.3 multi-connection model: each tab is an independent client
view with its own server connection, split tree, and document sessions. This
page covers the server-authoritative tab registry, the protocol messages,
the client-side multi-connection driver, event routing per tab, reconnect
and reclaim, the isolation invariants, and the Phase 22.5 client-owned
window-state persistence (restore of tabs, workspaces, split trees, and
per-pane documents). The shell chrome (tab bar, inactive-tab retention,
per-tab `TabChrome`) lives in
[Masonry Shell Runtime](masonry-shell.md); per-pane document hosting in
[Pane Document Views](pane-document-views.md); reconnect session restoration
in [Multi-Document Sessions](multi-document-sessions.md).

## Source

- `src/server/tab_registry.rs` — server-authoritative in-memory registry.
- `src/protocol/mod.rs` — `TabId`, `TabEntry`, `TabRegistrySnapshot`,
  `TabCommand`, `ClientMessage::TabCommand`, `ServerMessage::TabRegistry`
  (protocol v13; no 22.5 changes — restore rides the existing commands).
- `src/shell/layout_persist.rs` — Phase 22.5 `layout.json` v2 window state:
  `PersistedWindowState`/`PersistedTabState`/`PersistedTabLayout`,
  `serialize_window_state`/`parse_window_state`, `save_window_state`/
  `load_window_state`, `layout_from_persisted_tab` (see the Phase 22.5
  section below).
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

## Window-State Persistence (Phase 22.5)

Phase 22.5 makes the window survive a full quit/relaunch (client AND
server): tab order, the active tab, each tab's workspace root + split tree,
and each pane's open document persist to `layout.json` v2 and restore at
startup. The design is **client-owned persistence, server-rebuilt registry**:
the client owns the state file and all restored state; the server's
`TabRegistry` stays in-memory and is rebuilt at startup through the existing
`TabCommand::New`/`Activate` paths — **no new protocol messages, no new
server ops, no new authority**. Every restored tab rides the existing
per-connection validation (handshake, `add_root` canonicalization, `OpenDocument`
path checks), so a hostile `layout.json` can at most produce fewer/emptier
tabs, never a capability grant.

### Schema and bounds (`src/shell/layout_persist.rs`)

`layout.json` v2 shape: `{ version: 2, activeTab: <0-based index>,
tabs: [{ workspaceRoot, activePane, splitTree, slots, panes }] }`.
`splitTree` is a nested `{leaf: id}` / `{split: {orientation, ratio, first,
second}}` node (typed `PaneSplitNode` serialization); `panes` maps pane id →
workspace-relative document path (the identity is the relative path only —
the root id is re-learned at restore from the registry `TabEntry`); `slots`
carries only user-modified fixed slots, shared with the v1 collector.
Legacy v1 files (no `version`, `splits`/`slots` keys) still load and apply
to the single bootstrap tab exactly as Phase 20.3 — `is_legacy_layout`
distinguishes them. Parse is bounded and panic-free: tabs truncated to
`MAX_ACTIVE_CONNECTIONS` (64), invalid/missing `splitTree` degrades to the
default single-pane layout, `activePane` normalized, non-zero unique pane
ids and ratio 0.05–0.95 enforced, out-of-range `activeTab` dropped. `parse`
returns `None` on corrupt/legacy/empty state → bootstrap stays
byte-identical.

### Persistence triggers (collection + writes)

The shell's `persist_debounced` lost its 22.3 single-tab guard and the v1
`save_layout` writer was deleted; layout mutations (pointer drags, pane
commands incl. keyboard resize) now submit `EditorAction::PersistenceDue`
through a ≥ 500 ms debounce. The driver `persist_window_state` is the single
v2 writer and fires from four non-hot-path arms: the `PersistenceDue`
signal, every registry-snapshot reconcile (covers mount/close/reorder/
switch/workspace change), the `DocumentOpened` route, and the
`on_close_requested` quit flush. Collection (`collect_window_state`)
walks `tab_layout_data()` (per-tab `activePane` + tree + slots) and each
tab's pane targets, downcasting to `EditorWidget`/`PaneDocumentView` and
reading `active_document_identity()` — the ACTIVE document only; retained
but inactive sessions are never persisted (a closed document persisting as
open degrades to an empty pane on restore). Tab order comes from
`tab_order()`: registry order first, entry-less mounted tabs appended
(client-id order); `activeTab` is the position of `active_tab` in that
order. Nothing is written while the tab map is empty (initial handshake).

### Restore state machine (startup, `src/main.rs`)

`run_editor` receives `Option<PersistedWindowState>` (loaded via the
`clay::shell::load_window_state()` wrapper only when a real session
connected; the local-fallback/None paths never restore). Tab 0 rides the
bootstrap connection: its persisted root becomes the initial
`TabCommand::New` workspace root (the server `add_root`s it; the cwd root
stays unused) and the shell is built with `restored_single_editor`
(`TabChrome::with_layout` from `layout_from_persisted_tab`). Tabs 1..N
mount **sequentially inside the event loop, gated on registry
confirmation**: `advance_restore` (called after every snapshot) waits until
the last mounted tab's server-assigned `tab_id` appears in the registry,
then pops the queue and `spawn_restore_connect`s (existing `client::connect`
→ `OpenTabConnected` → `mount_restored_tab`, which installs the rebuilt
layout via `install_restored_tab` WITHOUT switching active). Each new tab's
`TabCommand::New` sets it active server-side, so the UI flips through tabs
as they mount; `finish_restore` fixes that with a final `Activate` of the
persisted active tab. `finish_restore` also runs `reopen_restored_documents`
(pending-opens keyed by `PaneId`, `enqueue_open_document(root_id, path)`
with the root id from the confirmed snapshot's `TabEntry`) and flushes
accumulated diagnostics. Sequencing is deterministic — no `MoveTo` reorder
needed: mounting in persisted order reproduces it exactly.

### Failure policy

- **Invalid/missing workspace root** (client pre-checks `is_dir`): that ONE
tab is skipped with a diagnostic (`clay.tabs.open_failed` via
`flush_restore_diagnostics`) and the queue continues — a stale file
degrades to fewer tabs, never a stall.
- **Server-rejected mount**: `TabCommand::New` answers `FileOperationFailed`
with NO registry snapshot, so the gate would stall; the
`RESTORE_CONFIRM_TIMEOUT` (15 s) deadline on `restore_gate` is checked at
the top of `on_action` and in `advance_restore` — expiry abandons the
remaining queue (`abandon_restore` = pure `cancel_restore` + diagnostic
flush; mounted tabs are KEPT).
- **Client connect failure** (`OpenTabFailed`): abandons the whole queue
(server-level failure), diagnostic surfaced, mounted tabs kept.
- **Missing document file**: reopen skips it (pane stays empty);
out-of-root paths are rejected server-side (`WorkspaceState`
`OutsideRoot`), so a hostile file cannot read outside the root.
- **Corrupt/legacy file**: `load_window_state` → `None` → bootstrap
byte-identical (no restore).
- **Unsaved edits / caret / viewport / scroll**: NOT persisted (documented
behavior — restore reopens documents at saved content); per-tab pane-focus
policy is config-driven (`setPaneFocusPolicy`), never persisted.

### Composition guards

22.5 verified pane/split commands (`ShellClientCommand` pane family,
`apply_layout_update`, divider/slot drags, pane-focus routing) mutate only
the ACTIVE tab's `TabChrome` via `active()`/`active_mut()` dispatch, and
reorder/switch leave every tab's internal state byte-identical (guard
tests: `pane_commands_only_mutate_the_active_tab`,
`divider_drag_credits_only_the_active_tab`,
`per_tab_routing_targets_are_isolated`,
`tab_switch_round_trip_preserves_split_trees_and_active_panes` in
`src/masonry_shell.rs`;
`per_tab_edit_queues_are_isolated` in `src/main.rs`;
`move_ops_change_order_only_and_preserve_entry_contents` in
`src/server/tab_registry.rs`).

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
- Persistence is client-owned: `layout.json` v2 is user-owned state written
  at any tab count (the 22.3 single-tab-only guard is gone); the server
  registry stays in-memory and is rebuilt at startup through existing
  `TabCommand` paths — no new protocol messages, ops, or authority.

## Known Ceilings

- Restart drops unsaved state: only tab order, active tab, per-tab
  workspace + split tree, and per-pane open documents are restored —
  unsaved edits, caret/viewport/scroll positions, and per-tab
  pane-focus-policy runtime changes are NOT (the `setPaneFocusPolicy` config
  API stays the policy source). There is no quit-time dirty confirm: closing
  the window with unsaved edits closes without asking.
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
- `src/shell/layout_persist.rs` (22.5): v2 schema round-trip/bounds/corrupt
  parsing (tabs capped at 64, invalid trees → single pane, ratio/pane-id
  bounds, legacy v1 detection, panic-free hostile input),
  `layout_from_persisted_tab_builds_validated_layout`. Command:
  `cargo test --lib layout_persist --quiet`.
- `src/main.rs` (bin, 22.5): restore state machine — gate waits for
  server-assigned `tab_id` before the next mount, missing-root tabs are
  skipped in order with diagnostics, deadline cancel drops the remaining
  queue, `reopen_restored_documents` attributes panes by `PaneId` and skips
  missing files, `tab_order_is_registry_order_with_entry_less_mounted_appended`.
  Command: `cargo test --bin clay --quiet`.
- `src/masonry_shell.rs` (22.5): `layout_mutation_signals_persistence_with_multiple_tabs`,
  `keyboard_resize_signals_persistence`, `tab_layout_data_returns_every_mounted_tab_layout`,
  `restored_single_editor_mounts_persisted_split_tree`,
  `install_restored_tab_mounts_persisted_tree_without_switching`.
- `src/client/mod.rs` (22.5): real-server restore-shape E2E
  `real_server_restore_sequence_orders_tabs_and_opens_documents` — three
  sequential `TabCommand::New`s reproduce persisted order with no `MoveTo`,
  per-pane `OpenDocument` lands in the tab's own root, missing root →
  `FileOperationFailed` with no registry entry, persisted active tab
  activates. Command: `cargo test --lib real_server_restore --quiet`.
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

## Phase 22.6: Authority Review, Tab Accessibility, and Protocol Guards

Phase 22.6 (2026-08-08) hardened the tab model: an authority review with
regression tests (task 6), the tab-bar accessibility surface (tasks 3–4,
implementation on the masonry-shell page), and protocol compatibility
tests for all Phase 22 tab messages (task 7). No production behavior
changed in tasks 6–7 — guards only.

### Authority review outcome (task 6)

- `TabRegistry` binds identity only: `TabId -> ClientId -> workspace root`
  (`TabEntry` has exactly the 4 identity fields), one `ClientId` per tab,
  and **grants nothing** — it re-points already-authorized connections.
  All real authority stays per-connection: workspace file grants are keyed
  by `client_id` (`workspace.rs` `DocumentAccess`/`acquire_access`/
  `release_access`), so a document opened in tab A is invisible to tab B,
  `OutsideRoot` rejects out-of-root opens per tab, and disconnect teardown
  (`cleanup_connection_documents` → `release_client_access`) releases every
  grant; a finalized document is never re-attached (re-open after
  disconnect is a fresh grant on a fresh document id). Reclaim re-points a
  surviving registry entry to a fresh handshake-authenticated connection
  and regains only that tab's own re-opened documents.
- **Package scopes are tab-independent**: `PackageApprovalRecord`
  (approvals.rs) serializes to exactly 13 keys (package identity,
  capabilities, processes, relations, replacements, approved_by/at,
  revoked) with no client/tab/pane/workspace keying; LSP grants are
  rechecked from `PackageService` with no tab keying. Tab create/close/
  move neither widens nor narrows package authority — truthful containment
  (no "sandboxed" claims beyond the OS-enforced external-process boundary).
- Regression tests (all green, no production change):
  - `src/server/tab_registry.rs`: `tab_entries_carry_identity_bindings_
    only_and_grants_nothing` (TabEntry literal compile-pin),
    `reclaim_rebinds_only_the_reclaiming_connection` (old client ops fail
    after reclaim; the reclaiming client cannot operate other tabs).
  - `src/server/connection.rs`: `reconnected_tab_regains_only_its_own_
    reopened_grants` (A opens 2 docs → disconnect releases both → fresh
    connection inherits nothing → reopens one, the other stays
    `UnknownDocument`).
  - `src/packages/approvals.rs`: `approval_records_carry_no_tab_client_or_
    workspace_keying` (JSON key-set pin).

### Tab accessibility surface (tasks 3–4)

The shell exposes `Role::TabList` (`Workspace tabs`) with one `Role::Tab`
per card (sanitized workspace basename, `selected` on the active card) when
2+ cards exist; single-tab windows keep the old tree. Inactive tabs' pane
hosts are unreachable from the a11y root. Announcements (single persistent
polite `Status` node): `Switched to tab {position}: {name}` /
`Opened tab {position}: {name}` / `Closed tab {position}: {name};
{n} tabs open` — fired only from user-initiated driver paths after a REAL
change (`activate_tab` after `switch_tab -> true`, `mount_tab`'s single
new-tab-dialog call site, `remove_tab`/registry reconcile), never on
startup/restore, focus moves, or no-ops. Exact strings and ceilings:
`docs/development/accessibility.md`.

### Protocol guards (task 7)

`tests/window_management_protocol.rs` (protocol suite): pins
`PROTOCOL_VERSION == 13`; round-trips all 8 `TabCommand` variants,
`TabRegistrySnapshot`, and the handshake hello through the rkyv
length-prefixed codec unchanged; rejects malformed/truncated/oversize/
corrupt tab frames without panic (`matches!` on `CodecError` — no
PartialEq). rkyv trailing alignment padding means corrupt-byte probes must
flip a payload byte past the length prefix + message discriminant (index 8),
not the last byte.

### Tests

- Task 6 suites: `cargo test --lib tab_registry --quiet`,
  `cargo test --lib connection --quiet` (filters above),
  `cargo test --lib approvals --quiet`.
- Protocol: `cargo test --test protocol -- window_management_protocol`.
- A11y: `cargo test --lib masonry_shell --quiet` (tab a11y + announcement
  tests); manual `test-plan/14-tabs.md` T50–T56.
