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
- `src/server/ops` and `src/server/js_runtime` must not name `AgentHost`.
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
- `cargo test --test protocol -- agent_protocol`
- `cargo test --test editor -- agent_daemon_work_is_absent`

## Related

- [Phase 25 Agent Protocol](../modules/agent-protocol.md)
- [clay-agent Daemon](clay-agent.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
