# Agent Protocol

## Source

- `src/protocol/agent.rs`
- `src/protocol/mod.rs` (`PROTOCOL_VERSION` 24, boxed `ClientMessage::Agent` / `ServerMessage::Agent`)
- `src/packages/manifest.rs` (`RESERVED_CORE_API_DOMAINS` includes `"agent"`)
- `tests/agent_protocol.rs`
- `tests/suites/protocol.rs`

## Overview

Phase 25 adds one boxed IPC pair for the core-owned clay-agent host. The client
sends `AgentClientCommand`. The server returns `AgentServerMessage`. Composer
keystrokes never wait on these messages. Package JavaScript has no import of
this module.

## Responsibilities

- Carry sessions, inventory, pickers, credential intents, and a compact
  `AgentWireEvent` union (including unused tool/permission variants for Phase 29).
- Keep the rkyv union floor small via `Box`.
- Never put credential secret bytes on `AgentServerMessage`.
- Reserve the `agent` Clay JS domain so packages cannot claim it.

## How It Works

`PROTOCOL_VERSION` is 30. Handshake still rejects older servers before any
agent frame is decoded. Version 30 adds the selected `provider` and `model`
to `AgentInventory`, letting a freshly mounted host-rendered agent surface
learn the current book selection from `listSessions`. The pin test
(`phase25_protocol_version_is_pinned`) is the contract.

`AgentClientCommand::CredentialPut` is the only command that carries
`AgentSecret`. `Debug` for `AgentSecret` always prints `[redacted]`. Acks are
`CredentialAck { provider, name, stored }` with no secret field.

`AgentWireEvent` maps Prism events to a small Rust enum. Chat ignores tool and
permission variants; they exist so Phase 29 does not rewrite the wire.

Payload ceilings live next to the types: `AGENT_MAX_PROMPT_BYTES` (32 KiB),
`AGENT_MAX_SNAPSHOT_ENTRIES` (200), `AGENT_DAEMON_MAX_LINE_BYTES` (1 MiB).
Oversized prompts fail as `Diagnostic` before daemon I/O.

Phase 2 additions: `NewSession` carries the daemon-side `workspaceRoot`/
`fullAutonomy` passthrough; `SkillRegister`/`CommandRegister`/`CommandDispatch`
forward the daemon skill/command RPCs (dispatch args ride a JSON string like
`RunResume.decision_json`, daemon-validated fail-closed); and the approval
bridge pairs `AgentServerMessage::ApprovalRequest { requestId, kind,
payloadJson }` (AG-UI `clay.approvalRequest` CUSTOM event) with
`ApprovalResolve`/`AskDecisionResolve` answers. Pending requests live in the
server-side registry (`PendingApprovals`); timeouts, dropped senders, and
zero subscribers deny fail-closed, and stale resolves return a diagnostic
without mutating anything.

Plan 117 additions: `AgentSessionSnapshot.mcp_servers` carries per-server
connection outcomes (`AgentMcpServerInfo { server_id, connected, tools,
error }` — camelCase serde, rkyv archived) instead of bare id strings;
`AgentSessionSnapshot.context_tokens` (Option) feeds the token meter;
`AgentModelInfo.thinking_levels` + `context_window` ride the models
inventory so the panel resolves effort levels and the context ceiling from
session start; `WorkspaceFiles { session_id }` lists bounded workspace
paths for the @-mention dropdown; `ResumableSessions` (unit variant)
broadcasts the labeled, workspace-scoped resume list via
`AgentServerMessage::AgentRpc { code: "session.resumable", … }`; and
`ListAgentSettingsFiles`/`OpenAgentSettingsFile` (with
`AgentSettingsFileInfo { name, display_path, size_bytes, modified_ms,
edited }` in `src/protocol/mod.rs`) back the agent's Settings tab. The
listing reply crosses to the webview as
`ClientConnectionEvent::AgentSettingsFiles { client_id, files }`
(`src/client/mod.rs`) — without an arm in the client read loop the reply fell
through the `Ok(_) => {}` catch-all and the tab sat on "Loading…" forever
(pinned by `tests/agent_settings_listing.rs`, which drives a real
`clay::client` connection rather than a raw socket).
`AgentSessionInfo` also carries `updated_at_label` (daemon-rendered local
`YYYY-MM-DD HH:MM` beside the raw ISO `updated_at`): the `/resume` picker is
server-rendered and has no timezone database, so a raw UTC stamp read as the
wrong hour.
Wire
invariant: unit `AgentClientCommand` variants (`ListSessions`,
`ResumableSessions`) deserialize from the bare-string form ONLY — the
`{variant: {}}` map form is rejected by serde (pinned by
`agent_unit_commands_deserialize_from_the_bare_string_only`).

## Code Examples

```rust
use clay::protocol::{AgentClientCommand, ClientMessage};

let message = ClientMessage::Agent {
    client_id: 1,
    command: Box::new(AgentClientCommand::Prompt {
        session_id: "s1".into(),
        text: "Hi".into(),
    }),
};
```

## Invariants and Constraints

- Adding/reordering wire variants requires a protocol bump.
- Truncated, invalid, and oversized frames fail closed (`CodecError`).
- `message_requires_tab_state` does not include `ClientMessage::Agent`; chat
  works with no workspace open.
- A book-selection broadcast (provider/model/profile/OM worker) must carry the
  tab's own session. `AgentHost::publish_book_snapshot(tab)` publishes
  `snapshot_for(session)` whenever the tab already has one, falling back to the
  session-less `unconfigured_snapshot()` only for a tab that has none: the
  panel merges STATE shallowly, so an empty `session_id`/`entries`/`branch`
  wipes the live session, transcript and status row (the Memory/Context/
  Settings tabs then read "No active agent session."). Every caller passes the
  tab it resolved with `tab_for_client(client_id).unwrap_or(client_id)` — the
  same resolution the panel's own mount (`TabState`) uses, so the selection
  lands on the tab whose STATE it belongs to.
- No `clay:agent` facade in this task.

## Tests

- `tests/agent_protocol.rs`: version pin, every command/message codec
  round-trip, secret omitted from Debug/ack, malformed frames, reserved domain.
- `src/server/agent.rs` (`tab_workspace_tests`):
  `book_selection_broadcast_keeps_the_tab_session` — after the pane mount
  establishes a session and a transcript row, an `Observation` worker selection
  must broadcast STATE carrying that `session_id`, its entries and its branch
  (fails with an empty session before the fix);
  `tab_state_snapshot_starts_the_session_and_carries_branch_and_environment`.
- `cargo test --test protocol -- agent_protocol`
- `cargo test --lib -- book_selection_broadcast_keeps_the_tab_session`

## Related

- [Phase 25 Agent Process Manager](../modules/agent-process-manager.md)
- [Protocol Codec](protocol-codec.md)
- [clay-agent Daemon](clay-agent.md)
