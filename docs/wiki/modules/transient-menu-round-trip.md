# Transient Menu Round Trip (Phase 24.1)

Server-owned interactive transient menu sessions: the wire protocol, the
per-connection session store, lifecycle edges, and client keystroke routing.
Built on the Phase 18.8 `TransientMenuSession` state model and the Phase
24.1 Command Centre foundation (`plans/081`).

## What it is

Before 24.1, every interactive transient menu (completion pickers, tab-close
confirm, document switcher) was client-owned: the session lived in the pane
view, key handling mutated local state, and no IPC round trip happened on the
accept hot path. Phase 24.1 adds a second, additive session class —
**server-owned** menus — where the server is the single authority: it owns
the session, filters on query updates, moves selection, and executes the
selected item. The client renders server-pushed bounded snapshots and only
forwards keystrokes. The proving session kind is the Control Center
(`controlCenter.open`); path mode (24.3) and the centered surface (24.4)
build on the same transport.

## Source files

- `src/protocol/menu.rs`: wire DTOs — `TransientMenuSnapshotData`,
  `TransientMenuItemData`, `TransientMenuStatusData`,
  `TransientMenuFocusPolicyData`, `TransientMenuOriginData` (rkyv-archived,
  inert display data only, no action payloads).
- `src/protocol/mod.rs`: `ClientMessage` variants `MenuQueryUpdate`,
  `MenuSelectionMove {delta}`, `MenuActivate`, `MenuCancel`; boxed
  `ServerMessage` variants `TransientMenuSnapshot(Box<TransientMenuSnapshotData>)`
  and `TransientMenuClosed {session_id}`.
- `src/server/menu_sessions.rs`: `ServerMenuSessions` per-connection store,
  `ServerMenuSession` wrapper, `snapshot_from_session` projection.
- `src/server/connection.rs`: intent handlers, the `controlCenter.open`
  special case in the `CommandIntent` arm, tab-switch cancel.
- `src/server/control_center.rs`: `ControlCenter` with persisted
  `selected_index` and `move_selection(delta)`.
- `src/client/mod.rs`: `ClientConnectionEvent` variants
  `TransientMenuSnapshot`/`TransientMenuClosed`; `ClientEditQueue`
  `enqueue_menu_query_update`/`enqueue_menu_selection_move`/
  `enqueue_menu_activate`/`enqueue_menu_cancel`.
- `src/masonry_pane_document.rs`: `PaneMenuSync` `server_owned` flag +
  `server_query_buffer`; `route_menu_key` ownership dispatch;
  `dispatch_server_menu_key`; `handle_text_event` Backspace interception.
- `src/main.rs`: driver routing — snapshots go to chrome + all pane views.
- `src/shell/transient_menu.rs`: `from_snapshot_data` hydration,
  `with_query`, `with_selected_index` builders.
- `src/perf/baselines.rs`: `encode_decode_max_transient_menu_snapshot`
  wire-size baseline (worst case under the 1 MiB frame cap).

## Protocol

Menu intents are connection-scoped transport frames (routed by
`client_message_identity` → `Some(client_id)`, require tab state). Session
ids: the server allocator uses `SERVER_MENU_SESSION_ID_HIGH_BIT` (`1 << 63`)
`| n`, which can never collide with the client-local `PaneMenuSync` allocator
(starting at 1) that owns local-session ids.

Phase 24.3 adds two protocol extensions beside `MenuQueryUpdate`: a
semantic `MenuBackspace` intent (a dedicated backspace instead of a full
query update) and a bounded `Primary`/`Secondary` activation `kind` on
`MenuActivate` (Enter/Tab vs Alt+Enter). `PROTOCOL_VERSION` bumped once
(15 → 16); Control Center behavior stayed byte-for-byte equivalent, no
path-specific wire variants exist, and activation resolves server-side
from installed entries only — never client-supplied paths.

The snapshot DTO is protocol-owned and inert: `session_id`, `prompt`,
`query`, bounded `items` (id, label, detail, accessibility label),
`selected_index`, `status`, `focus_policy`, `origin`. All strings are clamped
to `TRANSIENT_MENU_MAX_*` at both client parse and server build. Actions
never cross the wire — activation is by opaque session id, and the action
stays server-side. `TransientMenuStatus::Cancelled` never crosses the wire:
`TransientMenuClosed` is the terminal message, and the projection maps
`Cancelled` back to `Active`.

## Server store and lifecycle

`ServerMenuSessions` lives as a local `let mut menu_sessions` in
`handle_connection_loop`, so drop-on-exit is the connection-close sweep
across every exit path. Invariant: at most one active session per
connection; `open_control_center` replaces any previous session and returns
the replaced id so the caller emits `TransientMenuClosed` before the new
snapshot.

`ServerMenuSession` wraps a kind enum (`ControlCenter`, `PathBrowser`
since 24.3) plus the persisted `selected_index`. The wrapper is the choke
point: `set_query` clamps to `TRANSIENT_MENU_MAX_QUERY_CHARS`, and
`move_selection` walks `delta.rem_euclid(len)` `select_next` steps to
inherit `TransientMenuSession` wrap semantics. Phase 24.3 extends the
wrapper: `set_query`/`backspace` return `MenuEdit` with an optional
`relist: Option<PathBuf>` (only the PathBrowser arm relists — on
directory-prefix change or empty-filter ascent), and `activate(kind)`
returns `ServerMenuActivateOutcome` — `Navigate` keeps the path session
open with exactly one snapshot, `Dispatch`/`OpenFile`/`OpenWorkspace`
consume and close it. Helpers `install_path_browser`/`set_path_browser_error`
are no-ops on the Control Center kind.

Lifecycle edges:

- **Tab switch** (`TabCommand::Activate` to a different tab): the server
  cancels the active session (`cancel_active`) and pushes
  `TransientMenuClosed` — cancel-on-tab-switch beats a hidden-but-alive
  per-tab menu.
- **Reopen while open**: replace → `TransientMenuClosed(old)` then
  snapshot(new); stale intents for the old id produce the bounded
  `menu.unknown_session` diagnostic, never a panic or disconnect.
- **Local menu opens while a server session is active**: the pane view
  enqueues `MenuCancel` in `push_menu` before installing the local session
  (the driver has no edit-queue access; queues are per-pane) — one-active
  invariant holds in both directions.
- **Disconnect**: drop-on-exit sweeps; no explicit closed message needed.

## Client routing

Snapshots fan out to chrome + every pane view (`apply_connection_to_chrome`
then `fan_out_event`, the `Disconnected`-arm pattern), so whichever pane is
focused can route keys. Repeated `set_active_menu` across panes is
idempotent; the N `request_render` calls coalesce into one frame.

`PaneMenuSync` tracks `server_owned: bool` and `server_query_buffer: String`
(a send-buffer mirroring what was sent; visuals update only from snapshots —
the server is authoritative, there is no optimistic echo). Hydration is
`TransientMenuSession::from_snapshot_data` in `src/shell/transient_menu.rs`,
producing inert items with no action payloads.

`route_menu_key` dispatches on ownership, not id heuristics. For
server-owned sessions, `dispatch_server_menu_key` (pure; no `EventCtx`)
enqueues intents and never mutates local selection/query:

| Key | Intent |
|-----|--------|
| Printable | `MenuQueryUpdate` (buffer appended, clamped) |
| Backspace | `MenuBackspace` (buffer popped) — intercepted in `handle_text_event`'s `NamedKey::Backspace` arm, which otherwise goes straight to `local_command(EditorCommand::Backspace)` |
| ArrowUp/Down | `MenuSelectionMove ±1` |
| Enter/Tab | `MenuActivate` kind `Primary` |
| Alt+Enter | `MenuActivate` kind `Secondary` (path browser workspace open; Phase 24.3); every other Alt-key chord falls through to the editor |
| Escape | `MenuCancel` (no optimistic clear; `TransientMenuClosed` confirms) |

All consumed via `set_handled`; nothing reaches the editor. Local sessions
keep their exact pre-24.1 path unchanged.

## Phase 24.2: live catalogue, typed activation, generation invalidation

- **Open**. `controlCenter.open` now ships as a default `Ctrl+Shift+P`
  binding (`Global`, `ServerFirst`) in the default behavior manifest, and
  the ID is runtime-bindable (`is_runtime_bindable_command` allowlist), so
  the menu opens from the routed default key without an `init.js` binding.
  The `CommandIntent` arm clones the active behavior manifest and awaits
  `RuntimeGenerationStore::command_catalogue_snapshot(active_manifest)`, a
  generation-stamped four-source merge (22 built-ins, 38 `shell.client*`
  entries, trusted snapshot, third-party snapshot; deterministic sort,
  duplicate-ID fail-closed, fits `TRANSIENT_MENU_MAX_ITEMS` or open fails
  explicitly), then opens via `open_control_center(catalogue, generation_id)`.
- **Generation stamping**. The session records the runtime generation ID.
  `ServerMenuSession::activate(current_generation_id)` rejects a replaced
  runtime with `CommandExecutionRule::StaleRuntimeGeneration` (bounded
  diagnostic, never an error/disconnect), and the runtime
  generation-replacement broadcast cancels the open session with
  `TransientMenuClosed` before replaying `RuntimeStateSnapshot`, so menus
  cannot outlive their catalogue.
- **Activation**. `MenuActivate` cancels the session first (pushing
  `TransientMenuClosed`), then `ControlCenter::selected_activation` returns
  a typed `ServerMenuActivation`: `Command(CommandExecutionRequest)` for
  server/package commands — dispatched through the shared
  `execute_command_intent` with a live aggregated registry
  (`CommandRegistry::from_snapshots([trusted, third_party])`, later source
  wins; built-ins omitted because the executor falls back to the built-in
  table) — or `ShellClientCommand(String)` for `ClientUiCommand` items,
  shipped as the narrow `ServerMessage::ShellClientCommandRequest { command_id }`
  frame. The client maps it to
  `ClientConnectionEvent::ShellClientCommandRequest`, re-parses deny-by-
  default via `ShellClientCommand::from_command_id` (unknown/forged IDs are
  dropped with no state mutation), and routes through the extracted driver
  method `apply_shell_client_command` — the same path as keybinding-routed
  `shell.client*` commands, including tab commands and the ClosePane
  dirty-close gate. SDUI/`CommandIntent` call sites keep passing an empty
  registry; only the menu path carries the live one.
- **Query scoring**. Query filtering uses the shared bounded fuzzy
  subsequence matcher ([Fuzzy Matching](fuzzy-matching.md)); one bounded
  scan per query, never a registry re-snapshot or package JS.

## Control Center wiring (Phase 24.1 baseline)

`controlCenter.open` is special-cased in the `CommandIntent` dispatch arm
(after the stale-behavior-version gate, before request construction), not in
`execute_command_intent` (which lacks `menu_sessions` access and returns a
single `Option<ServerMessage>`). Phase 24.3 generalizes this into the shared
`open_command_centre_session` helper that also handles
`controlCenter.openPath` (seed resolution + one bounded user-browse
listing, then `open_path_browser`); generic execution of either id yields
nothing on the wire — the transport `CommandIntent` path is the only route,
and package JS cannot emit it. The JS `clay.commands.executeCommand` op path
still returns bare `Accepted` — opening UI requires the transport
`CommandIntent` path. The arm pushes `TransientMenuClosed` for a
replaced session plus the bounded snapshot. `ControlCenter` persists
`selected_index`: `move_selection(delta)` updates it, and `session()`
chains `with_query` + `with_selected_index` so produced sessions carry the
live query and selection. Activation details changed in 24.2 (see above);
the Path Browser adds `Navigate` (session stays open) and the
`OpenFile`/`OpenWorkspace` outcomes (24.3, see [Path Browser](path-browser.md)).

## Performance and budgets

- One bounded snapshot per keystroke (~70 KiB worst case, under the 1 MiB
  frame cap); the `encode_decode_max_transient_menu_snapshot` baseline
  guards the ceiling. A diff protocol was rejected unless profiling demands.
- One catalogue snapshot per menu open (not per keystroke) and one bounded
  fuzzy scan per query; open latency is one snapshot merge + one projection.
- No advisory latency constants were added — measurements did not justify
  them (task 8).
- The no-hot-path rule still holds: handlers run on the server; the client
  key path only enqueues non-blocking `try_send` intents.

## Security and authority

- Menu intents are transport-level `ClientMessage` frames, never a package
  facade; no package API can open, populate, or drive a server session.
- Activation routes through registered command authority — the unified
  user-authorized package model; no new authority is granted by 24.1.
- `controlCenter.open` cannot be shadowed: package command ids must use
  their `apiPrefix` namespace, which matches no built-in id, and
  `register_command` rejects duplicate ids. `controlCenter.openPath` (24.3)
  follows the same reserved-domain rule and is allowlisted exactly.
- Client trusts snapshots only after session-id membership checks; stale or
  unknown ids produce bounded diagnostics, never disconnects.

## Tests

- `src/protocol/menu.rs`: 9 codec tests (clamp, round trip, frame size).
- `src/server/menu_sessions.rs`: 16 store tests (high-bit ids, replace,
  query filter, selection wrap incl. `i64::MAX` modulo, activation through
  `CommandExecutor`, cancel, snapshot projection, `cancel_active`,
  adversarial intent ordering, path-browser navigate/activate/helpers/
  cancel/frame-ceiling).
- `src/server/connection.rs`: `menu_intents_for_unknown_sessions_produce_bounded_diagnostics`,
  `control_center_opens_filters_activates_and_cancels`,
  `tab_switch_cancels_the_active_server_menu_session`,
  `menu_backspace_deletes_one_char_and_secondary_activation_matches_primary`,
  and the Phase 24.3 path-browser e2e family (open from keybinding +
  catalogue, sticky-error seed, descend/ascend/jump, file-open grant
  conversion, workspace rebind, vanished-directory denial, no-grant
  navigation, cross-client denial, tab-switch/disconnect survival, reload
  dismissal).
- `src/masonry_pane_document.rs`: server-menu routing tests
  (single query update per key, arrows without local mutation, activate/
  cancel intents, snapshot hydration, closed-id matching, replace + buffer
  resync, backspace emits `MenuBackspace`, Alt+Enter emits `Secondary`
  only) + `local_menu_open_cancels_the_active_server_session`.
- `src/client/mod.rs`: `client_forwards_transient_menu_snapshot_and_closed_events`.
- `src/perf/baselines.rs` → `benches/protocol_server_baselines.rs`.

Run with:

```text
cargo test --lib menu_sessions --quiet
cargo test --lib control_center --quiet
cargo test --lib masonry_pane_document --quiet
cargo test --lib server::connection::tests --quiet
cargo test --test protocol --quiet
```

## Related

- [Transient Menu Session](transient-menu-session.md) — the shared state model
- [Path Browser](path-browser.md) — the second server-owned session kind (Phase 24.3)
- [Control Center](control-center.md) — the first server-owned session kind
- [Protocol Codec](protocol-codec.md) — framing and rkyv conventions
- [Command Registry](command-registry.md) — registration authority
- `docs/reference/primitives/shell-layout-strategy.md` — transient menu family contract
- `docs/reference/packages/creating-packages.md` — menu session ownership for packages
- `plans/081-Phase24.1-Transient-Menu-Interaction-Round-Trip.md`
