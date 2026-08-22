# Phase 25 Agent Process Manager

## Source

- `src/server/agent.rs`
- `src/server/mod.rs` (`IpcServer.agent`)
- `src/server/connection/mod.rs` (`ClientMessage::Agent` + broadcast write-back)
- `tests/agent_protocol.rs`
- `tests/editor_performance_invariants.rs` (`agent_daemon_work_is_absent_from_editor_hot_paths`)

## Overview

One `AgentHost` per Clay server. First `AgentClientCommand` lazy-spawns one
`clay-agent` child. Later commands reuse that child. Missing Node is a
diagnostic, not a hang. Package runtimes never receive this type.

## Responsibilities

- Resolve Node (`CLAY_NODE` or `PATH`) and `clay-agent/dist/main.js`
  (`CLAY_AGENT_MAIN` or next-to-exe / repo path).
- `Command` + `env_clear` spawn. Never a shell string.
- NDJSON JSON-RPC over stdin/stdout. 1 MiB line cap.
- Map RPC results and `method: "event"` lines to `AgentServerMessage`.
- Redact known secrets (vault passphrase, put secrets) from diagnostics.
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
`ServerMessage::Agent` when a subscriber is live. Lagged subscribers drop
events (same overflow policy as the daemon).

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
- `src/server/ops` and `src/server/js_runtime` must not name `AgentHost`.
- stderr is drained and discarded so a full pipe cannot stall the child.
- `# ponytail: global AgentHost lock, per-session queues if prompt throughput matters`

## Tests

- Missing Node: diagnostic in < 1s.
- Mock daemon: `session.new` snapshot, prompt event, credential ack without secret.
- Slow daemon: `dispatch` returns before the child replies.
- `cargo test --test protocol -- agent_protocol`
- `cargo test --test editor -- agent_daemon_work_is_absent`

## Related

- [Phase 25 Agent Protocol](phase25-agent-protocol.md)
- [clay-agent Daemon](clay-agent.md)
- [Server IPC Skeleton](server-ipc-skeleton.md)
