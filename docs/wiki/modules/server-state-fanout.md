# Server State Fanout Lanes

## Source

- `src/server/fanout.rs` — `Fanout<T>` (broadcast only) and `StateFanout<T>`
  (broadcast + current value), with their unit tests.
- `src/server/js_runtime/mod.rs` — the four JS-runtime lanes
  (`editor_commands`, `caret_styles`, `editor_layouts`, `shell_preferences`),
  their accessors, publisher wiring, and `production_reload` handle sharing.
- `src/server/ops/mod.rs` — `ClayOpState`'s publisher slots plus the
  `set_*_publisher`/`publish_*` pairs the JS ops call.
- `src/server/mod.rs` — `ActiveRuntimeStateFanout` (runtime-generation
  notifications) and `ActiveTypographyState` (typography snapshots).
- `src/server/connection/mod.rs` — per-connection subscriptions and the
  initial-sync reads; `src/server/connection/delivery.rs` — per-lane policy.
- Tests: `src/server/fanout.rs`, `src/server/js_runtime/tests/`
  (`lane_channel_capacities_are_preserved`, the caret/layout/shell lane tests),
  `src/server/connection/delivery.rs` (`lagged_state_lane_writes_the_current_value`,
  `lagged_advice_lane_writes_nothing`), `src/server/connection/tests/`.

## Overview

Clay's server owns a handful of small live channels that push one value family
to every connected client: programmatic editor commands, the caret-appearance
override, the wrap-policy override, shell preferences, typography snapshots, and
runtime-generation notifications. Each used to be a raw
`tokio::sync::broadcast::Sender` plus, where a late subscriber needed the value
it missed, a second `Arc<Mutex<T>>` store — with the store's lock ladder
repeated at every publish site and in `production_reload`.

Plan 132 U2/C5 collapses that duplication into two shapes in
`src/server/fanout.rs`, keeping each lane's capacity, value type, and lag policy
exactly as they were:

- `Fanout<T>` — bounded broadcast with no current value. Live delivery only: a
  lagged receiver drops what it missed.
- `StateFanout<T>` — `Fanout<T>` plus `Arc<std::sync::Mutex<T>>`. Every
  `publish` records the value before sending, `current()` replays it for
  connection initial sync and for a receiver that observed
  `RecvError::Lagged`, so the store is never behind the channel.

Both are cheap `Clone` handles that share one channel (and one store), which is
what lets a reloaded service, its op states, and every connection publish into
and read from the same lane.

## Responsibilities

- Own the bounded channel, the current-value store, and the lock discipline for
  one value family; nothing else in the server hand-rolls that pair.
- Keep a lane's lag policy explicit: `Fanout` = drop, `StateFanout` = replay
  current.
- Keep the publish API shaped so existing call sites (and their return values)
  did not change when the lanes were introduced.

Non-responsibilities: no protocol framing, no per-client narrowing, no
persistence, no retry, and no buffering beyond the channel capacity. Lanes
carry already-validated values; validation stays in the op/service that
produces them.

## How It Works

### The lanes

| Lane | Shape | Capacity | Lag policy | Current value |
|------|-------|----------|------------|---------------|
| `editor_commands` (`js_runtime/mod.rs:146`) | `Fanout<EditorCommandRequest>` | 16 | advice — drop | none |
| `caret_styles` (`:151`) | `StateFanout<Option<CaretStyle>>` | 4 | state — replay | `None` until a runtime sets one |
| `editor_layouts` (`:155`) | `StateFanout<Option<WrapPolicy>>` | 4 | state — replay | `None` until a runtime sets one |
| `shell_preferences` (`:159`) | `StateFanout<ShellPreferences>` | 4 | state — replay | seeded `pane_focus_policy: "click"` |
| typography (`src/server/mod.rs:266`) | `Fanout<ActiveTypography>` | 16 | state — replay | outside the lane (`Arc<tokio::sync::Mutex<ActiveTypography>>`) |
| runtime generation (`:179`) | `Fanout<RuntimeGenerationId>` | `RUNTIME_STATE_BROADCAST_CAPACITY` (16) | state — replay | outside the lane (latest snapshot + per-client acknowledgements) |

`tab_registry` (`broadcast::Sender<TabRegistrySnapshot>`) and the coding-agent
event lane stay raw broadcasts on purpose: the tab lane's channel type differs
from the state it guards (the registry itself is the replay source), and the
agent lane is advisory with no store.

### Publish path

`ClayOpState` holds one `Mutex<Option<…>>` slot per JS-runtime lane
(`src/server/ops/mod.rs:199-217`); `None` means "no publisher wired" and is the
normal state in unit tests. The JS ops call the `publish_*` methods
(`src/server/ops/mod.rs:706-816`), which lock the slot, publish through the
handle *while holding that outer guard*, and return `bool` = "a publisher is
wired". That return value is the JS-visible `published` flag for
`clientExecuteEditorCommand` (`src/server/ops/editor.rs`), so it had to survive
the refactor unchanged.

Lock order is always outer slot → lane store, and `set_*_publisher` only takes
the outer lock, so the nesting cannot deadlock. A state-lane publish therefore
takes two short `std::sync::Mutex` acquisitions (advisory lanes take one) and
no clone of the handle.

`ClayJsRuntimeService` owns the lanes (`src/server/js_runtime/mod.rs:142-162`)
and exposes them as accessors: `subscribe_*` for the connection loop and
`caret_style_override` / `editor_layout_override` / `shell_preferences` for
initial sync and tests (`:420-465`). `wire_runtime_publishers` attaches the same
handles to each domain's `ClayOpState`, and `production_reload` copies four
`Clone` handles into the new service (`:289-292`) instead of re-locking seven
publisher/store fields.

### Consumption

`src/server/connection/mod.rs:530-548` subscribes the six lanes per connection
and hands every receive outcome to `src/server/connection/delivery.rs`, whose
per-family helper encodes the policy: `Delivery::State` lanes
(`typography`, `caret_style`, `editor_layout`, `shell_preferences`,
`tab_registry`, `runtime_generation`) write the family's *current* value on
`RecvError::Lagged` — a gap in state is worse than a repeated value — while
`Delivery::Advice` (`editor_command`) drops a lagged request, whose moment has
passed. A closed sender ends the connection on those lanes. Initial sync reads
the same current values for a late or reconnecting client
(`src/server/connection/mod.rs:1265-1293`).

Two `Fanout`-only lanes keep their state deliberately outside the lane:

- **Runtime generation** broadcasts only the `RuntimeGenerationId`. The
  snapshot store narrows per client (`for_client`) and the fanout also tracks
  per-client acknowledgements, so it is not a plain current-value lane; the
  connection replays `latest_runtime_snapshot_for` after a lag.
- **Typography** needs its store guard held across
  `IpcServer::commit_runtime_generation`'s six-state conflict check, and it
  broadcasts only after the other guards drop. No publish-and-record API can
  express "compare, hold, then publish", so `ActiveTypographyState` keeps
  `current` and `updates: Fanout<ActiveTypography>` side by side
  (`src/server/mod.rs:266-320`), with the test-only `replace_typography` writing
  the store under the lock, dropping it, then publishing.

```rust
// A state lane: seeded current value, replayed to late subscribers and laggers.
let caret_styles = StateFanout::new(4, None);
let mut updates = caret_styles.subscribe();
caret_styles.publish(Some(style)); // records the value, then sends it
let initial_sync = caret_styles.current(); // the same value for a late client

// An advisory lane: no store, lagged receivers drop.
let editor_commands = Fanout::new(16);
editor_commands.publish(request); // live subscribers only
```

## Invariants and Constraints

- **Capacities are per lane and part of the contract:** 16 for
  `editor_commands`, 4 for caret/layout/shell preferences, 16 for typography and
  runtime generation. `broadcast::Receiver::len()` is *not* clamped to the
  capacity, so capacity tests assert on `TryRecvError::Lagged(n)` after
  overflowing instead of on `len()`.
- **Store never behind channel:** `StateFanout::publish` records the clone
  before sending, so a receiver can never observe a value newer than
  `current()`.
- **Replay is per family, not per message:** a lagged state lane skips to the
  current value and never replays the backlog; an advice lane replays nothing.
- **No `await` while holding a lane lock.** The stores are `std::sync::Mutex`,
  held only for the clone/write; publishing happens after or through a
  non-async call.
- **Handles share, never copy:** cloning a lane shares the channel and the
  store; the service, op states, and connections all observe one stream.
- **`Debug` is shape-only** (`Fanout`/`StateFanout`), so state types do not need
  `T: Debug` just to be debug-printed as part of `RuntimeGenerationStore`.
- **Unwired means silent:** publishing with no publisher wired stores nothing
  and sends nothing; the `bool` return is the only signal.
- **Snapshot clones are the recorded R2 ceiling (plan 134, no action):**
  `StateFanout::publish`/`current` clone the state value
  (`src/server/fanout.rs`), and `latest_for` clones the runtime snapshot before
  per-client narrowing (`src/server/runtime_state.rs`); the publish lane itself
  carries only the generation id. Desktop scale is capped at 64 connections,
  64 documents per client, and 64 snapshot documents (`src/perf/budgets.rs`),
  and no profile has named the clone paths, so `Arc`-sharing is deferred
  (YAGNI). Revisit when those 64 caps rise, when the existing diff-review
  triggers fire (snapshot payload p95 > 768 KiB or install p95 > 16 ms;
  `src/perf/budgets.rs`), or when a profile names the clone paths.

## Tests

- `src/server/fanout.rs` — `late_subscriber_replays_current`,
  `lagged_receiver_replays_current`, `publish_honors_configured_capacity`,
  `clones_share_channel_and_state`. Run: `cargo test --lib server::fanout`.
- `src/server/js_runtime/tests/::lane_channel_capacities_are_preserved` —
  publishes past each lane's capacity and asserts the exact `Lagged` counts, so
  a changed literal fails.
- `src/server/js_runtime/tests/` caret/layout/shell lane tests
  (`set_cursor_style_publishes_runtime_caret_override`,
  `set_editor_layout_publishes_runtime_wrap_override`,
  `set_pane_focus_policy_publishes_shell_preferences`,
  `shell_preferences_default_to_click_when_unset`).
- `src/server/connection/delivery.rs` — `lagged_state_lane_writes_the_current_value`
  and `lagged_advice_lane_writes_nothing` pin the two policies.
- `src/server/connection/tests/` — end-to-end lane delivery through a real
  connection, including initial sync and typography replacement.

```bash
cargo test --lib server::fanout
cargo test --lib -- caret editor_layout shell_preferences
cargo test --lib connection::
```

## Related

- [Server IPC Skeleton](server-ipc-skeleton.md) — the connection loop and its
  per-lane delivery policy.
- [Typography Registry and Font Roles](typography-registry-and-font-roles.md) —
  the typography snapshot this lane carries.
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md) — the
  runtime-generation commit that publishes on the generation lane.
- [Embedded JavaScript Runtime](embedded-js-runtime.md) — the service that owns
  the four JS-runtime lanes and wires them into op state.
- [Editor Movement, Selection, Caret, Ligatures, and Text Objects](editor-movement-selection-caret.md) —
  `CaretStyle` and the caret-override transport.
- `src/server/fanout.rs`, `src/server/js_runtime/mod.rs`, `src/server/ops/mod.rs`
