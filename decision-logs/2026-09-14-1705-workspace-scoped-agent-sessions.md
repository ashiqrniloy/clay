---
date: 2026-09-14 17:05
status: approved
decision_about: "Agent session ownership: one workspace root + agent type → one agent session, with tools bound to the session's root and per-tab frontend session state"
proposed_by: "agent (code-reviews/2026-09-14-editor-and-agent-architecture-review.md §SC-6; plan 119 task SC-6)"
explicitly_approved_by_user: true
---

# Decision: Workspace-scoped agent sessions

## Decision

The agent's ownership unit is the **workspace**, not the process:

1. **Server:** the agent session registry is keyed by `(workspace root, agent
   type)`. A tab no longer *owns* a session — it *looks one up* for its
   `(root, agent)` pair. The registry is a rebindable view over
   `SessionBook` (`tab → (root, agent) → session id`), and creating a session
   still happens through the existing daemon `session.new` with the tab's
   agent type and workspace root.
2. **Tool authority:** every agent tool call resolves its workspace from the
   **session**, never from a launch/bootstrap fallback. The daemon stamps the
   session id on reverse-RPC `document.*` calls; the server resolves
   `session id → workspace root → \`WorkspaceRootId\`` through the existing
   `SessionBook.session_root` binding and the existing workspace authority
   (`WorkspaceState`). An unresolvable session root **fails closed** with a
   diagnostic instead of falling back to the first configured root.

   *Implementation note (2026-09-14, from the SC-6 frontend task):* the
   relay is a process-wide broadcast whose copies are stamped with the
   **receiving** connection, so "filter by session id" needed two things the
   client could not supply on its own: a session tag on every session-scoped
   frame (now `AgentStreamEvent.session_id`, derived by
   `agent_agui::session_of`) and a *server-answered* binding — a tab's own
   `TabState`/run-command answer carries `session.bound { clientId, tabId,
   sessionId }` on that connection, and a store adopts it only when the
   `clientId` is its own. Inferring the binding from "the first snapshot that
   arrives" was explicitly rejected (it is the bleed mechanism this decision
   removes). The client-id stamp is a *delivery* test, the session tag the
   *ownership* test; together they also make a multi-tab window's stream
   single-copy. The store is created by the lazy agent view and adopted by the
   tab runtime (`attachAgentStore`), because constructing it in the eager shell
   pulled the AG-UI stack back into the startup preload.

   *Implementation note (2026-09-14, from the SC-6 server task):* the server
   holds **one `WorkspaceState` per tab** (`IpcServer::tab_states`; only the
   first tab may reuse the bootstrap state), so a root id is per-state and the
   session's **root path** is the cross-state identity. Resolution therefore
   goes `session id → recorded root path → the tab state that has that folder
   open → that state's root id` (canonical-path index on `WorkspaceState`,
   registry and state lookups taken one at a time). The invariant above is
   unchanged — no launch/first/bootstrap fallback, fail closed otherwise.
3. **Frontend:** agent session state is scoped to the tab runtime — one
   `AgentSessionModule` (and its `TauriClayAgent` transport) per tab,
   created on first agent-view mount and disposed with the tab. There is no
   process-global session singleton and no mutable process-wide sender; the
   relay is per-tab *routed by session id* (`threadId` / `state.sessionId`),
   with the tab's own `(root, agent)`-bound STATE answer as the authority for
   which session a tab owns.

The invariant this records, in one line: **one workspace root + agent type →
one agent session → tools bound to that root.**

## Context

`decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`
fixed the information architecture: a tab is one workspace plus one agent, and
"several workspaces means several tabs". The runtime did not follow: ownership
is still process-global in two places, and both defects are real today, not
theoretical.

**Server: tools bind to the bootstrap workspace.** `IpcServer::try_new` builds
one `AgentHost` with the *launch* workspace root (`src/server/mod.rs:705`) and
installs the document reverse-RPC handler bound to the **bootstrap**
workspace state (`src/server/mod.rs:721`:
`document_reverse_handler(Arc::clone(&bootstrap_state.workspace), …)`). The
module doc says so explicitly: "operations bind to the bootstrap workspace
(single-tab host model; multi-tab routing arrives with the Phase 2 agent UI)"
(`src/server/agent_documents.rs:12`). The daemon's document ops send only
`path` + payload — no session or root identity (`clay-agent/src/document-ops.ts:57`,
`:152`, `:173`, `:181`) — so `resolve_root` falls through to
`workspace.first_root_id()`, the **lowest** root id (`src/server/agent_documents.rs:148`,
`src/server/workspace/mod.rs:1293`). For a session bound to any other root the
containment check (`contained_existing_path`) runs against the wrong root:
a write meant for the tab's folder is either rejected as "path outside
workspace roots" or, when the path happens to sit inside root 1, lands through
root 1's document registry. The same launch-root assumption supplies the MCP
allow-list: `build_mcp_allow_list(config_root, launch_root, agent)`
(`src/server/agent_mcp_config.rs:150`) merges the *launch* folder's `.mcp.json`
into every session's grants, whichever folder the session actually belongs to.

The pieces the fix needs already exist. Sessions are already created per tab
with the tab's root and agent (`src/server/agent.rs:1477`→`:1585`, which
inserts `workspaceRoot` into `session.new`), the tab→session binding is
already root- and agent-invalidated (`session_for_root`,
`src/server/agent.rs:473`), and `book.session_root` already maps session →
root (`src/server/agent.rs:1488`). Tabs carry both the root string and the
root id (`src/server/tab_registry.rs:130`), and opening a folder in a tab
already registers the root (`workspace.add_root` → `registry.open_workspace`,
`src/server/connection/tabs.rs:122`), so `session root string → root id` is a
lookup, not a new concept. The low-level pieces are in place; only the
ownership key is wrong.

**Daemon: one more launch fallback.** `session.new` reads the root from params
(`clay-agent/src/host.ts:1423`) and records it in the session metadata
(`host.ts:1477`), and the coding tools are built from that root
(`host.ts:1809` → `buildCodingTools({ workspaceRoot })`,
`clay-agent/src/coding-tools.ts:243`: tool cwd, acceptance-policy roots,
wiki/graft binding). But `ensureLive` — the resume path — recreates the live
session with `workspaceRoot: process.cwd()` (`host.ts:2537`) and never reads
the recorded key back, so a resumed session's tools bind to the daemon's
launch directory. One daemon hosts many sessions already
(`private readonly live = new Map<string, LiveSession>()`,
`host.ts:883`), so this is a data-plumbing bug, not a capacity limit.

**Frontend: process-global session state.** `frontend/src/agent/state.ts:526`
creates the one `AgentSessionModule` behind `globalThis.__clayAgentSession`
("Process-wide agent session singleton (native parity: one stream per
client)"), and the relay stream is process-wide too
(`frontend/src/agent/events.ts:71`). Every agent view binds *into* that
singleton: `agentSession.agent.setSender(send)` with the pane's tab-stamped
sender and `agentSession.start()` per mount
(`frontend/src/coding-agent/CodingAgentPanel.tsx:495`–`:500`), where `send` is
the *active pane's* session request of whichever tab rendered last
(`frontend/src/shell/WorkspacePanes.tsx:91`, rendered into hidden slots so
every open tab stays mounted). With two tabs open, the last-bound sender
stamps every other tab's prompt/steer/cancel with its own tab id, and both
panes render the same snapshot. The relay therefore mixes sessions in one
store: the agent lane forwards the process-wide broadcast unfiltered
(`src/server/connection/delivery.rs:308`), the bridge tags each relay event
with the receiving `clientId`/`tabId` (`src-tauri/src/bridge/agent.rs:23`,
`src/client/mod.rs:1923`), and the run events already carry the owning session
as AG-UI `threadId` (`src/server/agent_agui.rs:263`) with `state.sessionId` on
snapshots — i.e. per-session routing is available without a protocol change.

Per-session book primitives (transcripts, efforts, checkpoints) are already
keyed by session id, so re-keying lookup does not disturb them:
`decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md`
made checkpoints session-keyed, and `apply_book_event`/`book_snapshot` take a
session id, not a tab.

## Approval

- Proposed by: agent (review §SC-6; the plan's option analysis below).
- Approved by user: Yes.
- Approval evidence: the user approved Plan 119, whose SC-6 task records the
  chosen approach verbatim — "Workspace-scoped sessions. (Chosen — matches the
  approved tab model.)" — and then directed this record explicitly:
  "Complete SC-6 decision log: workspace-scoped agent sessions and update the
  plan once done." The IA this implements was itself user-approved in
  `2026-09-11-2331` ("A tab is one workspace plus one agent … several
  workspaces means several tabs", `explicitly_approved_by_user: true`).

## Alternatives Considered

1. **Keep process-global agent state, route by active tab (status quo).**
   Leaves wrong-workspace tool writes reachable, lets one tab's prompt carry
   another tab's identity, and mixes transcripts/approvals in one store. The
   review already named this the genuine architectural gap; rejected.
2. **Workspace-scoped sessions (chosen).** One session per
   `(workspace root, agent type)`; tools resolve from the session; frontend
   state per tab. Matches the approved IA, composes with session-keyed
   checkpoints, and needs no protocol change. Cost: the server's session
   lookup is re-keyed and the frontend store becomes a registry — the largest
   diff in Plan 119.
3. **Per-tab sessions, fixed binding only** (keep `tab → session` ownership;
   just pass the root on every tool call and scope the frontend store per
   tab). Smallest diff, but two tabs on the same folder hold two independent
   sessions for one workspace — two transcripts, two contexts, two
   checkpoint trees for one folder — contradicting "a tab is one workspace
   plus one agent" and doubling daemon work per folder. Rejected.
4. **Resolve the workspace from the tab on every tool call** (pass `tabId`
   in reverse RPC instead of the session). The daemon knows its session, not
   its tab; a tab can rebind to another root, and resumed sessions have no
   live tab. Makes authority depend on mutable routing state; rejected.
5. **Key the frontend store by session id instead of tab.** The store must
   exist before the first STATE (a freshly opened tab has no session yet) and
   the tab is the ownership unit the shell renders and disposes. The store
   stays tab-keyed and *adopts* the session id from its own tab-stamped STATE
   answer, then filters the relay by it; rejected as the primary key, kept as
   the routing rule.
6. **Filter the agent broadcast per connection on the server** (send a client
   only its own tabs' sessions). Correct in principle and cheaper on
   serialization, but not needed for correctness once the client routes by
   session id, and it would re-introduce per-connection agent state into the
   delivery lanes SC-2 just made uniform. Deferred: revisit if relay traffic
   from other windows becomes measurable.

## Rationale and Evidence

Facts behind the chosen approach:

- The launch-root defects are reachable through normal use: the daemon's
  `document.*` params carry no root or session identity (`clay-agent/src/document-ops.ts:57`,
  `:152`, `:173`, `:181`), and the server resolves an absent `workspaceRootId`
  to `first_root_id()` (`src/server/agent_documents.rs:148`,
  `src/server/workspace/mod.rs:1293`). Verified by reading the handler and its
  `serve` dispatch (`src/server/agent_documents.rs:60`).
- Sessions already carry their workspace root end-to-end on the *new-session*
  path: `create_session` inserts `workspaceRoot` (`src/server/agent.rs:1585`),
  the tab registry is the single source of tab→root truth (`set_tab_registry`,
  `src/server/mod.rs:729`; `tab_workspace_root`, `src/server/agent.rs:906`),
  and the daemon records the root in session metadata for session search
  (`clay-agent/src/host.ts:1477`). The fix consumes existing bindings; it does
  not add a parallel source of truth (plan SC-6 code-quality criterion: "no
  copy of workspace state into agent state").
- `session → root` is already durable on the server: `book.session_root` is
  written on session creation and refreshed on rebind
  (`src/server/agent.rs:1488`), so the reverse handler can resolve a root from
  a session id without new persisted state. `book.json` stores *selections* per
  root and per `(agent, root)` (`src/server/agent.rs:3700`, `:3761`) — it
  contains no session ids, so re-keying sessions does not change the file
  format.
- Root-id resolution is a lookup, never a filesystem probe: `WorkspaceState`
  holds `roots: HashMap<WorkspaceRootId, WorkspaceRoot>` with a canonical path
  per root (`src/server/workspace/mod.rs:473`, `:82`) and exposes the
  canonical directory roots (`directory_roots`, `src/server/workspace/mod.rs:576`).
  Every tab-bound root is registered at open time
  (`src/server/connection/tabs.rs:122`→`:139`), so an unresolved session root
  means stale state — a diagnostic, not a silent rebind.
- The daemon already runs many sessions (`live: Map<string, LiveSession>`,
  `clay-agent/src/host.ts:883`) and builds tools per session
  (`host.ts:1809`), so workspace-keyed registry ownership changes only *which*
  session a tab looks up, not how many the daemon can serve.
- The frontend can route without a protocol change: relay events are tagged
  with the receiving `clientId`/`tabId` (`src-tauri/src/bridge/agent.rs:23`)
  and carry the owning session as `threadId`/`state.sessionId`
  (`src/server/agent_agui.rs:263`, `frontend/src/agent/state.ts:294`).
- Session-keyed primitives are untouched: checkpoints are keyed by session
  (`src/server/agent_checkpoints.rs`; decision `2026-08-30-2200`), and
  `apply_book_event`/`book_snapshot` take session ids.

Implementation record (SC-6 frontend task, 2026-09-14):

- One `AgentSessionModule` per tab: `createAgentSession({ send, clientId })`,
  born subscribed, `dispose()` at tab close; no process-global store.
- Filter: `clientId` = this tab's connection, plus `sessionId` = this tab's
  server-answered binding for session-tagged frames (untagged = process-wide).
- Binding: `session.bound` claims (never inferred); a dropped, unattributable
  session-tagged frame triggers a debounced refresh.
- Transport: `TauriClayAgent` takes its sender and its accept-predicate at
  construction (`setSender` deleted); the panel's agent commands ride the
  tab's own connection.
- Bundle boundary: the store is constructed inside the lazy agent chunk; the
  shell references only its type and the `attachAgentStore` lifetime hook.

Implementation record (SC-6 server task, 2026-09-14):

- Registry: `SessionBook::sessions_by_workspace: HashMap<(agent type, root),
  session id>` with pure `session_for_workspace` lookups; the tab-keyed
  `tab_session*` maps are gone. `rebind_tab_agent` takes the pre-switch agent
  type (the tab registry handler already reads it) and moves the one session to
  the new key.
- Tool authority: `first_root_id()` and the `workspaceRootId` escape hatch are
  deleted; `agent.workspace_unresolved` is emitted as both the tool error and a
  broadcast diagnostic.
- MCP allow-list: built per session from `(agent's mcp.json, session root's
  .mcp.json)` for every session (agent type or not); the host no longer keeps a
  launch workspace root.
- Daemon: `ensureLive` reads the recorded `workspaceRoot` metadata key (cwd only
  for pre-workspace records); `session.resume` echoes the bound root; every
  `document.*` call carries the session id.

Implementation record (SC-6 verification task, 2026-09-14):

- The multi-tab pass is automated over the real binaries
  (`tests/agent_session_isolation.rs`, in the `security` suite): a real
  `clay server` process, two real clients on two roots, and either a scripted
  daemon whose identical write is attempted against **both** roots (only the
  server's containment check can decide) or the shipped daemon in `--mock`
  mode reporting each session's own `workspace.files` listing. Asserted: per
  session writes land in that session's own root; routed `session.bound`
  names each tab's own session; a closed tab's session fails closed with
  `agent.workspace_unresolved`; sessions and their roots survive a daemon
  restart (scripted *and* real daemon).
- Consequence of the pass (behavior now fixed, not merely stated): a daemon
  exit clears the host's `Running` handle (`AgentHost::forget_running`,
  identity-checked with `mpsc::Sender::same_channel`), so the next call
  respawns the daemon and the session resumes on its recorded root. Before
  this, a crashed daemon was never respawned and every later call addressed a
  channel nobody read — the resume scenario this log mandates was unreachable
  until the server restarted.

Assumptions (stated, not verified):

- Two tabs pointing at the same root are expected to *share* the workspace's
  agent session (same transcript, same context). This follows from the IA
  decision rather than from a user statement about that specific case; if
  users want independent conversations per tab over one folder, that is a
  change of this decision, not a bug in it.
- The per-tab frontend store can be created without knowing the session id up
  front because its first tab-stamped STATE answer supplies it; the store
  accepts relay events only after it has adopted an id, which makes the
  pre-session window empty rather than wrong.

## References

- `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-6 — the
  gap, the fix shape, and the composition with per-session primitives.
- `plans/119-Editor-and-Agent-Architecture-Remediation.md` — SC-6 tasks
  (decision log, server registry + tool resolution, frontend per-tab state,
  multi-tab verification), with the option analysis and acceptance criteria
  this log fixes.
- `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md`
  — user-approved IA: one tab = one workspace + one agent; several workspaces
  means several tabs.
- `decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md`
  — session-keyed checkpoint/branch primitives this re-keying must not disturb.
- `src/server/mod.rs:705`, `:721`, `:729`, `:734` — agent host construction,
  reverse-RPC binding, tab registry, agent roots.
- `src/server/agent_documents.rs:12`, `:60`, `:148` — bootstrap binding,
  reverse-RPC dispatch, `first_root_id()` fallback.
- `src/server/workspace/mod.rs:82`, `:473`, `:576`, `:1293` — root registry,
  canonical paths, `first_root_id`.
- `src/server/agent.rs:473`, `:906`, `:1477`, `:1585` — `session_for_root`,
  tab→root lookup, session creation, `workspaceRoot` on the wire.
- `src/server/agent_mcp_config.rs:150` — allow-list built from the launch root.
- `clay-agent/src/document-ops.ts:57`, `:152`, `:173`, `:181` /
  `clay-agent/src/coding-tools.ts:243` / `clay-agent/src/host.ts:883`, `:1423`,
  `:1477`, `:1809`, `:2537` — daemon-side root plumbing, resume fallback.
- `frontend/src/agent/state.ts:526` / `frontend/src/agent/events.ts:71` /
  `frontend/src/coding-agent/CodingAgentPanel.tsx:495` /
  `frontend/src/shell/WorkspacePanes.tsx:91` — the process-global store and its
  per-tab bindings.
- `src/server/connection/delivery.rs:308` / `src/client/mod.rs:1923` /
  `src/server/agent_agui.rs:263` — the broadcast agent lane and the session
  identity already present on relayed events.

## Consequences

Positive:

- Agent tool writes/reads are constrained to the session's own workspace root;
  the `first_root_id()` fallback disappears, so an unresolvable root fails
  closed instead of silently authorizing the wrong folder.
- A workspace's agent history becomes the workspace's history: the folder's
  transcript, context, MCP grants and checkpoints belong to one session
  regardless of which tab is looking at it.
- Per-tab frontend state ends cross-tab bleed: prompts carry the issuing tab's
  identity, transcripts/approvals render per tab, and the relay is routed by
  session id.
- MCP grants stop leaking the launch folder's `.mcp.json` into sessions bound
  to other folders.

Risks and follow-up work:

- Transient migration effect: today two tabs on one root own two daemon
  sessions; after the change they converge on the workspace's session (the
  older sessions remain on disk and stay resumable). Expect one "stale
  session" reconciliation per folder the first time it is opened twice.
- The daemon resume path (`host.ts:2537`) must start reading the recorded
  workspace root; until it does, a resumed session's tools still bind to the
  daemon cwd — the same invariant, caught by the SC-6 verification task.
- Server-side relay filtering stays deferred (alternative 6); if relay traffic
  or serialization from unrelated windows grows, revisit it there.
- Windows/other-platform behavior is unaffected: the change is
  identity/ownership plumbing, not filesystem semantics.

Conditions that would cause revisiting this decision:

- Users wanting several independent conversations per workspace root (per-tab
  sessions over one folder). The remedy would be an explicit session choice
  (`/fork`, `/clone`, `--session` already exist in the daemon) on top of the
  workspace key, not a return to process-global ownership.
- A future multi-client story where one workspace's session must be observed
  by several windows simultaneously (shared-live session), which needs the
  deferred server-side routing work.
