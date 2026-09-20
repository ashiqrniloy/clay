# Agent Process Manager

## Source

- `src/server/agent.rs` (host, spawn, actor, RPC framing)
- `src/server/agent/run.rs` (`run`/`run_inner` command dispatch, daemon event mapping)
- `src/server/agent/book.rs` (session book the run pipeline reads and writes)
- `src/server/agent/mcp.rs` (MCP inventory and allow-list state)
- `src/server/connection/delivery.rs` (agent-lane restart/oversize delivery policy)
- `src/server/mod.rs` (`IpcServer.agent`)
- `src/server/connection/mod.rs` (`ClientMessage::Agent` + broadcast write-back)
- `tests/agent_protocol.rs`
- `tests/editor_performance_invariants.rs` (`agent_daemon_work_is_absent_from_editor_hot_paths`)

## Overview

One `AgentHost` per Clay server. First `AgentClientCommand` lazy-spawns one
`clay-agent` child. Later commands reuse that child. Missing Node is a
diagnostic, not a hang. Package runtimes never receive this type. If the daemon
actor exits, `forget_running` clears only its own stale channel so the next
command respawns the daemon and can resume persisted sessions.

The server also owns the **authority handle**: an `AgentHostHandle` it injects
into every JavaScript runtime lane's `ClayOpState` (plan 130 A1). Nothing is
process-global, so two `IpcServer`s in one process route agent ops to their own
hosts and a lane's authority is exactly the handle it was wired with.

## Responsibilities

- Resolve Node (`CLAY_NODE` or `PATH`) and `clay-agent/dist/main.js`
  (`CLAY_AGENT_MAIN` or next-to-exe / repo path).
- `Command` + `env_clear` spawn (plan 117: inherits exactly
  `HOME`/`USERPROFILE`/`PATH` so the daemon's config root follows the
  server's isolated profile and MCP bare commands can PATH-resolve
  server-side). Never a shell string.
- NDJSON JSON-RPC over stdin/stdout. 1 MiB line cap.
- Map RPC results and `method: "event"` lines to `AgentServerMessage`.
  Event types with no `AgentWireEvent` arm are dropped, never forwarded (a
  synthesized `Started` would pin the client run "streaming" forever).
- Redact known secrets (vault passphrase, put secrets) from diagnostics.
- Resolve the daemon's data dir from the Clay root:
  `<configuration root>/agents/coding-agent/data`, or the per-user
  `~/.clay/agents/coding-agent/data` when the server has no explicit root (the
  desktop's `clay auto` launch, a bare `clay server`) — the same root the
  daemon's `homedir()` default resolves to. Unit tests resolve a per-process
  temp root instead, so no test reads or writes the developer's profile. One
  shared `temp_dir()/clay-agent` for every root-less server used to hold every
  profile's `book.json`, `credentials.vault`, `sessions.sqlite`, and
  `vault.passphrase` at once.
- Fire-and-forget `dispatch` so the connection loop never awaits the child.

## How It Works

`IpcServer::new` stores `AgentHost::for_server(configuration_root)`. Tests that
build a stub server use `AgentHost::inert()`, which returns
`agent.unavailable` without spawn.

**Plan 130 A1 — injected ownership, fail-closed hostless lanes, per-lane
registration queue.** Server construction reaches the runtime service once:
`RuntimeGenerationStore::initial(handle)` → `ClayJsRuntimeService::set_agent_host`
→ `wire_runtime_publishers`, which calls `ClayOpState::set_agent_host` on every
domain lane (trusted/third-party × general/latency). Replacement workers from a
poison restart start unwired and travel the same path, so a lane never loses the
server's host. `set_agent_host` is idempotent and hands over anything the lane
queued before the wiring existed (`take_pending_agent_registrations` →
`AgentHostHandle::queue_registration_now`, stopping at a full host queue exactly
as the removed process-global drain did).

Agent ops resolve the host from their own lane (`lane_host(state)`); a lane
without one fails closed with
`agent.unavailable: no agent host is attached to this runtime`, never with a
fallback to another server's host. The package-facing registration ops are the
one deliberate exception: with no host they queue the declaration on that lane's
own `ClayOpState` (bounded by `PENDING_REGISTRATION_CAP`, 64) and answer
`{"queued": true}`, so a package load entry never fails just because its runtime
has no agent subsystem. The host keeps its own pending queue of the same size
for declarations addressed while its daemon is still starting, drained in order
after the first `initialize` handshake.

`dispatch` clones the host and `tokio::spawn`s `run`. `run` calls `ensure_running`:
create `--data-dir`, load or create a 0600 `vault.passphrase`, spawn, send
`initialize { passphrase }` with a 5s timeout. RPC timeout is 30s.

A single actor owns stdin, stdout, and pending oneshot map. `select!` writes
requests and reads lines. `method == "event"` publishes on a broadcast channel.
The connection loop has a dedicated `select!` arm that writes
`ServerMessage::Agent` when a subscriber is live. Its agent delivery helper
ignores `Lagged` and `Closed` outcomes so a daemon restart does not close the
view; an event that exceeds the frame budget becomes a bounded
`agent.frame_too_large` diagnostic. Other connection lanes use their own
State/Advice policy in `connection/delivery.rs`.

`CLAY_AGENT_MOCK` adds `--mock` for tests against the real script. Unit tests
in this task spawn a tiny Python NDJSON stand-in so they do not need Node.

On actor exit the child is `kill()`ed. `shutdown` sends the `shutdown` method
then drops the channel.

## Code Examples

```rust
server.agent.dispatch(AgentClientCommand::Prompt {
    session_id: id,
    text: "Hi".into(),
});
```

## Invariants and Constraints

- No `std::process::Command` / `tokio::process::Command` in editor, client, or
  package-ops hot paths for this daemon.
- Agent runtime state (book, vault, sessions, passphrase) belongs to exactly one
  Clay root: never a shared temp directory, and never the developer's real
  profile from a unit test.
- No process-global agent authority may exist: `AGENT_HOST_AUTHORITY`,
  `PENDING_PACKAGE_REGISTRATIONS` and `install_global` are gone; the handle
  lives in the server's state graph and in each lane's `ClayOpState`, and
  `tests/agent_protocol.rs::agent_authority_is_server_state_not_a_process_global`
  keeps it that way.
- `src/server/ops` and `src/server/js_runtime` must not name the daemon owner
  `AgentHost`; they carry the opaque `AgentHostHandle` and resolve it per lane,
  failing closed when their lane has no host.
- Registration declarations queue per lane (bounded), then hand over to that
  lane's host — never to a process-wide queue, and never across servers.
- stderr is drained and discarded so a full pipe cannot stall the child.
- `# ponytail: global AgentHost lock, per-session queues if prompt throughput matters`

## Tests

- Missing Node: diagnostic in < 1s.
- Mock daemon: `session.new` snapshot, prompt event, credential ack without secret.
- Slow daemon: `dispatch` returns before the child replies.
- `tests/agent_session_isolation.rs` — real-server scripted-daemon and shipped
  mock-daemon checks for root isolation, fail-closed tab closure, and respawned
  session-root recovery; its isolated `HOME` is what proves the per-user data
  dir (a shared temp dir made it read another run's persisted profile and time
  out).
- `src/server/agent.rs`: `for_server_without_a_configuration_root_keeps_agent_state_off_the_shared_temp_dir`,
  `for_server_moves_a_legacy_agent_data_dir_under_the_per_agent_root`, and
  `tab_state_snapshot_skips_a_profile_the_daemon_does_not_have` pin the root
  resolution and the profile-availability fallback.
- `cargo test --test protocol -- agent_protocol` — incl.
  `agent_authority_is_server_state_not_a_process_global` (no process-global
  authority or queue, the lane op state owns the handle) and
  `two_servers_in_one_process_own_independent_agent_hosts`.
- `cargo test --test editor -- agent_daemon_work_is_absent`
- `cargo test --test protocol -- plan130_wiki_pages_describe_host_modules_and_injected_ownership`
  — keeps this page and [clay-agent Daemon](clay-agent.md) describing the
  injected ownership and the host module map.

## Related

- [Phase 25 Agent Protocol](../modules/agent-protocol.md)
- [clay-agent Daemon](clay-agent.md)
- [Persistent Runtime Hardening](../modules/persistent-runtime-hardening.md) — the lane topology the handle is injected into
- [Server IPC Skeleton](server-ipc-skeleton.md)
