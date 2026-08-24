# React AG-UI Chat Stream (Plan 097 Phase 10)

Status: implemented (Phase 10). Chat prompt, streaming, cancellation,
transcript, session list/resume/delete, thinking, usage, and error states flow
through one core-owned AG-UI event stream. The Prism daemon protocol stays
internal; the webview only ever sees standard `@ag-ui/core` events adapted in
Rust.

## Ownership

| Concern | Owner |
|---|---|
| Agent host, sessions, transcripts, bounds, secrets | Clay server (`src/server/agent.rs`) |
| Internal-event → AG-UI adaptation | Rust adapter (`src/server/agent_agui.rs`) |
| Webview event delivery | Tauri relay (`src-tauri/src/bridge/agent.rs`) |
| Prompt/cancel/session requests | Existing validated bridge path (`session_request`) |
| Event pipeline (chunk expansion, verification, message/state application) | `AbstractAgent` from `@ag-ui/client` — never duplicated |
| Custom transport | `frontend/src/agent/TauriClayAgent.ts` (`run()` over the relay) |
| Presentation binding | `frontend/src/agent/state.ts` + `frontend/src/chat/ChatPanel.tsx` |

## Event mapping (Rust adapter)

| Clay wire message | AG-UI output |
|---|---|
| `Snapshot(transcript)` | `MESSAGES_SNAPSHOT` (entries → user/assistant/reasoning messages; error/usage entries keep roles with `metadata.clayKind`) + `STATE_SNAPSHOT` (sessionId/profile/provider/model) |
| `Inventory` | `STATE_SNAPSHOT` (bounded providers/models/profiles/sessions) |
| `Event::Started` | `RUN_STARTED {threadId, runId}` |
| `Event::MessageDelta` | `TEXT_MESSAGE_CHUNK {messageId: clay-text-{runId}, delta}` |
| `Event::ThinkingDelta` | `REASONING_MESSAGE_CHUNK {messageId: clay-reasoning-{runId}}` |
| `Event::Tool` / `Permission` / `Overflow` | inert `CUSTOM` events (`clay.toolPhase`, `clay.permissionRequest`, `clay.overflow`) — display-only, no execution surface |
| `Event::Finished` | `RUN_FINISHED {result: {usage}}` |
| `Event::Error` | `RUN_ERROR {message}` |
| `Picker` | dropped (pickers are Command Centre domain) |
| `CredentialAck` / `Diagnostic` | `CUSTOM clay.credentialAck` / `clay.diagnostic` (no secret fields exist on these variants) |

The adapter is pure, total, and bounded by upstream caps (transcript entry
caps, delta byte caps, inventory limits).

## Transport shape

1. The webview calls `agent_subscribe(on_event: Channel<AgentStreamEvent>)`.
   The relay fans every adapted event out to all subscribed channels and
   prunes dead channels on send failure.
2. Each connection pump intercepts `ClientConnectionEvent::Agent`, adapts it,
   and delivers `{clientId, tabId?, ...event}` over the relay. Raw Clay agent
   frames never reach the webview envelope stream.
3. Only the active client's pump relays agent events, so multi-tab desktops do
   not receive duplicate streams.
4. A run is one `runAgent()` call: `TauriClayAgent.run()` sends the validated
   `chat.submit` intent through `session_request` (server-side prompt
   validation/bounds reused) and forwards exactly that run's events to the
   upstream pipeline, completing at `RUN_FINISHED`/`RUN_ERROR`. Empty prompts
   complete locally without touching the wire. `abortRun()` sends the
   validated `chat.cancel` intent.
5. Out-of-run snapshots (transcript restore, inventory) are applied through
   the agent's own public `setMessages`/`setState` APIs by
   `state.ts` — there is no parallel Clay-only reducer anywhere in React.

## Presentation

`ChatPanel` mounts only for the bundled `@clay/chat` empty-tab surface
(provenance-exact selection, mirroring the SettingsPanel precedent).
Greeting/hint copy and setup buttons are read from the package's declared
component tree, so package authority over landing presentation is preserved
and disabling/replacing `@clay/chat` removes the view automatically.

- Transcript rows are memoized; per-token deltas rerender only the streaming
  row.
- Store listener notifications coalesce per animation frame, so burst deltas
  cause at most one rerender per frame.
- Status line mirrors native parity: `Streaming` while a run is open, last
  error otherwise, `Ready` when clean; `agent.cancelled` clears streaming
  without an error entry and empty submits stay silent no-ops.
- Sessions list comes from `listSessions` inventory state; Resume/Delete send
  typed agent-family commands through the same validated request path.
- Editor input never waits on agent work: the stream is asynchronous channel
  delivery and nothing in the composer or editor hot paths blocks on it.

## Security

- Credentials have no field on any mapped variant; phase25 daemon tests plus
  the adapter's structural key test pin this.
- Tool/permission payloads are inert data; future coding-agent work gains
  display transport without gaining execution authority.
- Packages cannot spawn or speak to the daemon, acquire Tauri APIs, or
  subscribe to the relay; replacement of `@clay/chat` removes the landing and
  profile but not host security. ACP remains absent.

## Verification

- Rust: adapter unit tests (mapping, JSON shape, terminal diagnostics),
  relay fan-out tests, full workspace clippy `-D warnings`.
- Frontend: transport tests (end-to-end run through the real `@ag-ui/client`
  pipeline, intent payloads, cancel), state-glue tests, ChatPanel component
  tests. Production budgets keep AG-UI out of the startup shell: the review
  harness is a DEV-only `React.lazy` route, and `ChatPanel` is a 37.1 kB gzip
  lazy chunk (shell 160.4 / 180 kB, total 342.9 / 400 kB).
