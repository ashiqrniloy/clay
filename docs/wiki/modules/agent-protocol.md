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
permission variants; they exist so Phase 29 does not rewrite the wire. The
mapper (`server/agent/run.rs::map_event`, invoked from `agent.rs`'s
`route_daemon_line`) **drops** an event type it has no arm for: `None` becomes
`DaemonLine::Ignore`, so nothing reaches the wire. The removed catch-all used
to synthesize `Started`, which pinned the client's run "streaming" forever
when a non-lifecycle event arrived without one (the `agent_suspended`
incident); Prism 0.7's `attention_compiled` and `delegation_*` telemetry
exercises exactly that path. A lifecycle type Clay has to render gets an
explicit arm instead of a default. Plan 122 added the one such arm:
`subagent_started`/`subagent_stopped` (the daemon's supervisor lifecycle
bridge) map onto the existing Tool wire event — phase Started/Finished, `name`
= the stable `childId` (fallback `spawn_agent`), the `delegationId` pairing
the rows, and the stopped `status` riding the redacted output digest. No new
AG-UI event type or panel; events without a `childId`/`delegationId` stay
dropped, and everything else keeps the unknown-drop contract.

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

Plan 119 SC-6 additions: `TabState` and every run command
(`agent.submit`/`agent.cancel`/`agent.steer`) answer with
`AgentServerMessage::AgentRpc { code: "session.bound", result_json:
{ clientId, tabId, sessionId } }`, written on the requesting connection
(`session_bound_message`, `src/server/connection/mod.rs`; run commands in
`src/server/connection/runtime.rs`). The agent relay is a process-wide
broadcast, so this is the only way a tab can learn which session it owns: the
claim names the requesting client id, and an empty `sessionId` means the tab
has no session yet. The AG-UI relay additionally stamps every session-scoped
frame with the session it came from (`agent_agui::session_of` →
`AgentStreamEvent.session_id`, `src-tauri/src/bridge/agent.rs`); diagnostics,
agent-RPC replies, and a tab's pre-session STATE snapshot stay untagged.

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
- Agent sessions are keyed by `(agent type, workspace root)` (`SessionBook::
  sessions_by_workspace`, plan 119 SC-6): a tab resolves through
  `SessionBook::session_for_workspace`, so sibling tabs on one folder share one
  session, and agent tool calls resolve their workspace from the session
  (`AgentHost::session_workspace_root`) — the bootstrap/first-root fallback is
  gone, unregistered roots fail closed with `agent.workspace_unresolved`.
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
- The pane's mount STATE (`AgentHost::tab_state_snapshot`) also applies the
  coding surface's own profile rule (plan 108 task 8): an *empty* book profile
  is filled with `CODING_SURFACE_PROFILE_ID` (`agent:coding`, the profile the
  `@clay/coding-agent` package registers) before the tab's session is ensured.
  A pane restored by `layout.json`, entered through the view switcher, or
  rendered by the empty-tab landing never dispatched `coding-agent.profile`, so
  its session used to be created with the daemon-level `Chat` default — no
  coding tools and no MCP servers (`mcpServers: []`), which read in the panel
  as `MCP none`. A non-empty profile is never overwritten, so a deliberate
  selection survives the mount.
- The guess is checked against the daemon's own profile list first
  (`AgentHost::profile_available`, one `agentProfile.list` RPC; false when the
  daemon is unreachable or the host is inert). A profile no package registered
  — the package never loaded, a store without it, a hostless runtime — makes
  `session.new` fail with the daemon's `Unknown agent: coding`, which left the
  tab with no session at all: a dead pane with no error. With the profile
  absent the book stays unselected and the daemon's built-in `Chat` default
  serves the tab, so the mount still binds a session. The
  `coding-agent.profile` launch command applies the same check, and
  `tab_state_snapshot_skips_a_profile_the_daemon_does_not_have` pins both the
  fallback profile and the untouched book.
- A daemon exit clears the host's `Running` handle (identity-checked on the
  channel the actor owns), so the next agent call spawns a fresh daemon rather
  than addressing a dead channel; sessions survive it, because `session.prompt`
  for an existing session resumes it on the root recorded at `session.new`
  (`ensureLive`, plan 119 SC-6) — never the daemon's launch cwd.
- No `clay:agent` facade in this task.

## Tests

- `tests/agent_protocol.rs`: version pin, every command/message codec
  round-trip, secret omitted from Debug/ack, malformed frames, reserved domain.
- `src/server/agent.rs` (`tab_workspace_tests`):
  `map_event_drops_unknown_event_types` — `attention_compiled`,
  `subagent_started`/`subagent_stopped` **without identifying ids**,
  `delegation_started`/`delegation_finished`/`delegation_child_event`, and
  an arbitrary unknown type all map to `None` (never a fabricated
  `Started`); the unreported-usage case is dropped too, not mapped;
  `map_event_maps_subagent_lifecycle_onto_tool_rows` — identified
  `subagent_*` events map onto Tool Started/Finished with child-id name,
  delegation-id pairing, and status digest (plan 122);
  `book_selection_broadcast_keeps_the_tab_session` — after the pane mount
  establishes a session and a transcript row, an `Observation` worker selection
  must broadcast STATE carrying that `session_id`, its entries and its branch
  (fails with an empty session before the fix);
  `tab_state_snapshot_starts_the_session_and_carries_branch_and_environment`
  (also pins the mount's profile fill: `coding`, and a deliberate `custom`
  profile survives a later mount);
  `sibling_tabs_on_one_workspace_share_one_session` /
  `sibling_tabs_resolve_one_session_through_the_host` (one session per
  `(agent, root)`, two tabs resolve it),
  `session_workspace_root_is_the_recorded_root_only` (no launch-root
  fallback), `mcp_allow_list_follows_the_session_workspace_root`,
  `session_kept_while_workspace_root_matches` /
  `workspace_change_resolves_a_new_key` (the key, not the tab, decides).
- `cargo test --test protocol -- agent_protocol`
- `cargo test --lib -- book_selection_broadcast_keeps_the_tab_session`
- `cargo test --lib -- sessions_touch_only_their_own_workspace_root
  unresolved_session_root_fails_closed_with_a_diagnostic` (tools stay inside
  the session's root)
- `cargo test --test security -- agent_session_isolation` — live two-workspace
  pass over a real server socket: per-session writes land in the session's own
  root, a closed tab's session fails closed with `agent.workspace_unresolved`,
  and both a scripted daemon restart and the shipped daemon in `--mock` mode
  keep each session on its recorded root.

## Related

- [Phase 25 Agent Process Manager](../modules/agent-process-manager.md)
- [Protocol Codec](protocol-codec.md)
- [clay-agent Daemon](clay-agent.md)
