# AG-UI over Tauri Channel Stream Flow

End-to-end trace of one agent turn: Clay agent wire events become AG-UI
events in Rust, cross the bridge on a Tauri channel, and drive the React chat
surface. Session/provider/model selection stays server-owned; the webview
only renders and forwards composer input.

## Source

- `src/server/agent_agui.rs` — `adapt_agent_message`: maps
  `AgentServerMessage` (snapshot + wire events + transcript) to AG-UI
  lifecycle events (`TEXT_MESSAGE_CHUNK`, `REASONING_MESSAGE_CHUNK`,
  `MESSAGES_SNAPSHOT`, `STATE_SNAPSHOT`, `RUN_STARTED/FINISHED/ERROR`, tool
  events as inert customs). Stable per-run chunk ids; terminal diagnostics
  map to `RUN_ERROR`.
- `src-tauri/src/bridge/agent.rs` — `AgentRelay`: fan-out of adapted events
  to every subscribed webview `Channel`, each event tagged with
  `client_id` (+ optional `tab_id`) and flattened so the AG-UI `"type"`
  discriminator stays top-level.
- `frontend/src/agent/events.ts` — process-wide relay stream (refcounted
  `agent_subscribe` / `agent_unsubscribe`).
- `frontend/src/agent/TauriClayAgent.ts` — `AbstractAgent` subclass; the
  stream feeds AG-UI's reducer, so message state is standard.
- `frontend/src/chat/ChatPanel.tsx` — presentation only.

## Flow

1. The clay-agent sidecar emits wire events; the server session adapts each
   `AgentServerMessage` through `adapt_agent_message` — exactly one producer
   of AG-UI events exists, and the webview never sees Clay-only agent frames.
2. Adapted events travel the existing server→bridge pump as part of the
   agent message family; the relay tags and serializes one copy per
   subscribed channel (multiple windows may observe the same stream).
3. The frontend relay subject pushes tagged events into the
   `TauriClayAgent`; AG-UI's runtime reduces them into a messages snapshot,
   preserving status across notification cycles.
4. **Prompts/cancel reuse the validated request path**: composer submit and
   cancel are sent as inert `sduiAction` intents (`chat.submit`,
   `chat.cancel`) through `session_request` — no second write path, so
   validation, provenance, and authorization are unchanged.

## Invariants

- Credentials never appear in events; provider/model selection is
  server-owned state surfaced via snapshots.
- Tool/permission payloads are inert data rendered as declared UI.
- Unsubscribe at refcount zero; re-subscribing registers a fresh channel.
- Without Tauri IPC (tests/fixtures) the stream stays subscribed but inert.

## Tests

- Rust mapping: `cargo test agent_agui` (`src/server/agent_agui.rs` tests:
  snapshot mapping, run lifecycle chunk-id stability, error/tool mapping).
- Relay: `cargo test -p clay-desktop agent`.
- Frontend transport: `frontend/src/agent/transport.test.ts`; component:
  `frontend/src/chat/ChatPanel.test.tsx`.

## Related

- [React AG-UI Chat Stream](../modules/react-agui-chat-stream.md) — module view.
- [Server-Driven UI Protocol Schema](../modules/server-driven-ui.md) — intent routing.
