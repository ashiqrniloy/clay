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
- No `clay:agent` facade in this task.

## Tests

- `tests/agent_protocol.rs`: version pin, every command/message codec
  round-trip, secret omitted from Debug/ack, malformed frames, reserved domain.
- `cargo test --test protocol -- agent_protocol`

## Related

- [Phase 25 Agent Process Manager](../modules/agent-process-manager.md)
- [Protocol Codec](protocol-codec.md)
- [clay-agent Daemon](clay-agent.md)
