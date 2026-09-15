# React AG-UI Chat Stream (Plan 097 Phase 10)

Status: implemented (Phase 10). Chat prompt, streaming, cancellation,
transcript, session list/resume/delete, thinking, usage, and error states flow
through one core-owned AG-UI event stream. The Prism daemon protocol stays
internal; the webview only ever sees standard `@ag-ui/core` events adapted in
Rust.

## Ownership

| Concern                                                                   | Owner                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Agent host, sessions, transcripts, bounds, secrets                        | Clay server (`src/server/agent.rs`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Internal-event → AG-UI adaptation                                         | Rust adapter (`src/server/agent_agui.rs`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Webview event delivery                                                    | Tauri relay (`src-tauri/src/bridge/agent.rs`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Prompt/cancel/session requests                                            | Existing validated bridge path (`session_request`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Event pipeline (chunk expansion, verification, message/state application) | `AbstractAgent` from `@ag-ui/client` — never duplicated                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Custom transport                                                          | `frontend/src/agent/TauriClayAgent.ts` (`run()` over the relay; construction-scoped sender + session filter)                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Per-tab session store                                                     | `createAgentSession()` in `frontend/src/agent/state.ts` — created by the agent view on first mount, adopted by the tab runtime (`TabRuntime.agent`, `frontend/src/shell/workspace-controller.ts`), disposed with the tab                                                                                                                                                                                                                                                                                                     |
| Presentation binding                                                      | `frontend/src/coding-agent/CodingAgentPanel.tsx` and the components it composes — `TranscriptList.tsx` (turns + per-turn agent labels), `Composer.tsx` (input lane), `InspectorTabs.tsx` + `FilesTab.tsx`/`MemoryTab.tsx`/`ContextTab.tsx`/`SessionInfoTab.tsx`/`SettingsTab.tsx`/`BoundedText.tsx` (right column), `ApprovalStrip.tsx` (plan 119 SC-4); plan 108: bounded `tools` rows + cumulative `toolStats` from `clay.toolPhase` CUSTOM events — counts only, never payloads; renders the store its tab runtime passes |

## Event mapping (Rust adapter)

| Clay wire message                         | AG-UI output                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Snapshot(transcript)`                    | `MESSAGES_SNAPSHOT` (entries → user/assistant/reasoning messages; error/usage entries keep roles with `metadata.clayKind`) + `STATE_SNAPSHOT` (sessionId/profile/provider/model + plan 108 `mcpServers`: server-built allow-list names for the Coding Agent extension strip; `contextTokens` from the last Finished event for the status row). Book snapshots (empty `sessionId`, published on provider/model/profile switches) emit `STATE_SNAPSHOT` only — a messages snapshot there would wipe the live transcript on every picker selection. |
| `Inventory`                               | `STATE_SNAPSHOT` (bounded providers/models/profiles/sessions; models carry `contextWindow` for context-size-vs-window reporting)                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `Event::Started`                          | `RUN_STARTED {threadId, runId}`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `Event::MessageDelta`                     | `TEXT_MESSAGE_CHUNK {messageId: clay-text-{runId}, delta}`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `Event::ThinkingDelta`                    | `REASONING_MESSAGE_CHUNK {messageId: clay-reasoning-{runId}}`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `Event::Tool` / `Permission` / `Overflow` | inert `CUSTOM` events (`clay.toolPhase`, `clay.permissionRequest`, `clay.overflow`) — display-only, no execution surface                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `Event::Finished`                         | `RUN_FINISHED {result: {usage}}`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `Event::Error`                            | `RUN_ERROR {message}`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `Picker`                                  | dropped (pickers are Command Centre domain)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `CredentialAck` / `Diagnostic`            | `CUSTOM clay.credentialAck` / `clay.diagnostic` (no secret fields exist on these variants)                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `AgentRpc`                                | `CUSTOM clay.agentRpc { code, result }` — `result_json` is parsed into a JSON object so the Context / Memory tabs can apply it (`typeof result === "object"`). A non-JSON payload stays a string and is ignored.                                                                                                                                                                                                                                                                                                                                 |

The adapter is pure, total, and bounded by upstream caps (transcript entry
caps, delta byte caps, inventory limits).

## Transport shape

1. The webview calls `agent_subscribe(on_event: Channel<AgentStreamEvent>)`.
   The relay fans every adapted event out to all subscribed channels and
   prunes dead channels on send failure.
2. Each connection pump intercepts `ClientConnectionEvent::Agent`, adapts it,
   and delivers `{clientId, tabId?, sessionId?, ...event}` over the relay. Raw
   Clay agent frames never reach the webview envelope stream.
3. The agent host is a process-wide broadcast, so **every** connection's pump
   relays every event, each copy stamped with the connection that received it
   (`clientId`/`tabId` = the delivery) and with the session the message belongs
   to (`sessionId`, from `agent_agui::session_of`; absent for process-wide
   messages such as diagnostics and agent-RPC replies).
4. A per-tab store therefore accepts a frame only when both tags agree with
   its own tab: its connection (one copy of its own tab's traffic — the other
   tabs' copies of the same broadcast are dropped) and, for session-tagged
   frames, the session the server bound that tab to. Untagged frames are
   process-wide and pass to every store.
5. A run is one `runAgent()` call: `TauriClayAgent.run()` sends the validated
   `agent.submit` intent through the tab's own `session_request` lane
   (server-side prompt validation/bounds reused) and forwards exactly that
   run's events to the upstream pipeline, completing at
   `RUN_FINISHED`/`RUN_ERROR`. Empty prompts complete locally without touching
   the wire. `abortRun()` sends the validated `agent.cancel` intent. (Plan 118
   renamed these from `chat.*`; the ids are agent-scoped, and the host-rendered
   composer is their single authorization owner.)
6. Out-of-run snapshots (transcript restore, inventory) are applied through
   the agent's own public `setMessages`/`setState` APIs by the tab store —
   there is no parallel Clay-only reducer anywhere in React.

### Tab ↔ session binding

The relay cannot tell a tab which session it owns (every connection receives
every session's traffic), so it is answered, not inferred: a tab's `TabState`
request is answered with `AgentServerMessage::AgentRpc { code:
"session.bound", result_json: { clientId, tabId, sessionId } }` written on that
connection (server: `session_bound_message`, `src/server/connection/mod.rs`),
and every `agent.submit`/`cancel`/`steer` answer carries the same binding
(`src/server/connection/runtime.rs`) — which is what hands a freshly created
session to the store _before_ the daemon's `RUN_STARTED`. The store adopts a
claim only when `clientId` matches its own connection; an empty `sessionId`
means "this tab owns no session yet", never "unknown owner". A store that sees
session-tagged traffic it cannot attribute asks for its binding again
(debounced to one request per 500 ms), which is how an agent switch, a resume,
or a sibling tab's action re-binds it without the panel knowing about it.

## Presentation

`CodingAgentPanel` is the module's only consumer: it mounts for the bundled
`@clay/coding-agent` pane surface (provenance-exact selection, mirroring the
SettingsPanel precedent). Plan 119 SC-4 split that one component into a
composition root plus focused children: the panel still owns the store, the
snapshot subscription, the send authority (`submit`, the run lifecycle) and
the left column's layout, while the transcript list, the composer, the approval
strip and the inspector (with one file per tab) are props-in/props-out
components with their own state machines. The transcript's previous-agent
attribution is one memoized forward pass (`transcript-model.ts`
`previousAgents`) instead of a per-turn reverse scan, and the composer's pure
pieces (chord match, effort cycle, `@`-token parse) are exported for unit
tests — see `frontend/src/coding-agent/Composer.test.tsx` and
`transcript-model.test.ts`. Plan 118 removed the empty-tab landing panel and its
`@clay/chat` package, so the module carries session naming
(`AgentSessionModule`/`AgentSnapshot`/`AgentStatus`) and no product-named
branch. Plan 119 SC-6 removed the process-global store: the panel creates its
tab's store on first mount (or keeps the one the runtime already adopted) —
the creation lives in this lazy chunk on purpose, so the eager shell never
takes a runtime dependency on the agent lane; the controller sees only the
`AgentSessionModule` _type_ and the `attachAgentStore` lifetime hook (a full
store on startup would move the AG-UI stack into the entry chunk's preload).
A standalone mount (fixtures, component tests) simply keeps its own store. Its
agent commands
(`listSessions`, picker selects, `workspace.files`, `session.context`,
`session.om.*`, `runResume`, SDUI intents) ride that tab's connection instead
of the bridge's active client, so two tabs cannot address each other's
session. The store is born subscribed and released by `dispose()`: the tab (not
the mounted surface) owns the transcript, so a run that outlives a view switch
keeps streaming into it.

- Transcript rows are memoized; per-token deltas rerender only the streaming
  row.
- Store listener notifications coalesce per animation frame, so burst deltas
  cause at most one rerender per frame.
- Status line mirrors native parity: `Streaming` while a run is open, last
  error otherwise, `Ready` when clean; `agent.cancelled` clears streaming
  without an error entry and empty submits stay silent no-ops.
- Sessions list comes from `listSessions` inventory state; Resume/Delete send
  typed agent-family commands through the same validated request path (the
  tab's own connection, see above).
- Editor input never waits on agent work: the stream is asynchronous channel
  delivery and nothing in the composer or editor hot paths blocks on it.

## Security

- Credentials have no field on any mapped variant; phase25 daemon tests plus
  the adapter's structural key test pin this.
- Tool/permission payloads are inert data; future coding-agent work gains
  display transport without gaining execution authority.
- Packages cannot spawn or speak to the daemon, acquire Tauri APIs, or
  subscribe to the relay; replacing a landing or agent package removes its
  contributions but not host security. ACP remains absent.

## Verification

- Rust: adapter unit tests (mapping, JSON shape, terminal diagnostics),
  relay fan-out tests, full workspace clippy `-D warnings`.
- Frontend: transport tests (end-to-end run through the real `@ag-ui/client`
  pipeline, intent payloads, cancel, per-tab isolation: a store drops another
  tab's delivery copy, another session's snapshot/lifecycle, and a binding
  claim addressed to another connection; `dispose()` stops applying and
  notifying), state-glue tests, CodingAgentPanel component tests (each mount
  passes its own store), and controller tests (one store per tab runtime,
  tab-stamped binding request, store disposed on tab close). Production budgets keep the panel out of the startup shell:
  the review harness is a DEV-only `React.lazy` route, and `CodingAgentPanel`
  is an ≈11 kB gzip lazy chunk (shell 173.7 / 180 kB; total 400.3 / 404 kB —
  plan 119 SC-4's split added ≈0.5 kB gzip of prop/plumbing code to a ceiling
  that was already at its headroom; the total ceiling moved 400 → 404 kB with
  that evidence in decision-logs/2026-09-15-1153-frontend-total-bundle-ceiling-404-kb.md).
  `frontend/vite.config.ts` also gives `bridge/client.ts` its own chunk: rollup
  hoists a manual chunk's dependencies, so without that split the eager shell
  (which imports the bridge) pulled the whole 36.4 kB-gzip `agent-core` chunk
  into startup as a preloaded chunk. Plan 118's chat-frontend task fixed the
  hoist, which is what makes the agent lane genuinely lazy.

Panel details the plan-119 further-actions pass settled (2026-09-15,
`code-reviews/screenshots/2026-09-15-plan119-further-actions/review-log.md`):

- **Approval strip focus (F3).** `ApprovalStrip.tsx` keeps `role="alertdialog"`
  and now moves focus to the first action on appearance, returning it to the
  previously focused control when the strip unmounts — the announcement and the
  keyboard entry point match.
- **Inspector strip affordance (D2).** The 340 px inspector needs ~337 px for
  its five labels plus the hide toggle, so the strip scrolls (the approved
  artifact declares `.inspector .tabs { overflow-x: auto }`).
  `coding-agent.module.css` restores the host's thin scrollbar for that strip
  only, so the overflow is visible to mouse users without changing the tab
  recipe.
- **No unhandled bridge rejections.** Fire-and-forget shell calls (menu
  commands, tab commands, SDUI intents, layout persistence) route through
  `frontend/src/lib/detached.ts`; waiting callers keep their own error paths.
