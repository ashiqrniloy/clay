# clay-agent Daemon

## Source

- `clay-agent/src/main.ts`
- `clay-agent/src/host.ts`
- `clay-agent/src/providers.ts`
- `clay-agent/src/rpc.ts`
- `clay-agent/src/redact.ts`
- `clay-agent/src/compaction.ts`
- `clay-agent/src/coding-tools.ts`
- `clay-agent/src/document-ops.ts`
- `clay-agent/src/mcp.ts`
- `clay-agent/src/obscura.ts`
- `clay-agent/src/resolve-obscura.ts`
- `clay-agent/README.md`
- `clay-agent/src/__tests__/host.test.ts`
- `clay-agent/src/__tests__/rpc.test.ts`
- `clay-agent/src/__tests__/compaction.test.ts`
- `clay-agent/src/__tests__/coding-tools.test.ts`
- `clay-agent/src/__tests__/durable-run.test.ts`
- `clay-agent/src/__tests__/skills-commands.test.ts`
- `clay-agent/src/__tests__/session-search-tree.test.ts`
- `clay-agent/src/__tests__/mcp-obscura.test.ts`
- `src/server/agent_documents.rs`
- `src/server/agent_checkpoints.rs`
- `src/server/agent_mcp_config.rs`
- `src/server/agent_settings.rs`
- `frontend/src/coding-agent/CodingAgentPanel.tsx`

## Overview

`clay-agent` is Clay’s Node >= 20 child process that hosts Prism 0.5.5. It is
**not** a Clay JS package and is not loaded by Deno. `AgentHost` in
`src/server/agent.rs` lazy-spawns one daemon per server. Package JS cannot
spawn or speak to it.

## Responsibilities

- Stdio JSON-RPC wrapping `createAgent` / `createAgentSession` / `AgentEvent`.
- SQLite session store under `--data-dir/sessions.sqlite`.
- Encrypted credential vault under `--data-dir/credentials.vault`; OS keychain
  when the secret service answers. No plaintext fallback.
- Load first-party Prism 0.5.5 provider packages through the extension kernel
  with a stored credential resolver (never `process.env`). Imports use family
  subpaths only: `@arnilo/prism` (agent/kernel API),
  `@arnilo/prism-core/{credentials/node,sessions/sqlite,validation/json-schema}`,
  `@arnilo/prism-memory/compaction/{observational-memory,llm}`, and
  `@arnilo/prism-providers/<adapter>`. All seven 0.5.5 family pins are exact
  (`prism`, `prism-core`, `prism-providers`, `coding-tools`, `web-tools`,
  `memory`, `mcp`) plus direct `better-sqlite3@13.0.3` for the SQLite subpath
  and `playwright-core@1.61.0` for CDP composition. The unused 0.3
  `prism-model-router` dependency was dropped with the consolidation; retired
  0.3 package names are denied by `agent_protocol::phase25_dependencies_deny_acp_agui_mcp`.
- Host-registered `AgentDefinition`s. Chat is a tool-free chat session
  (Phase 2 ships the `@clay/coding-agent` UI); coding profiles get the Phase 1
  tool surface described below.
- Coding-run options (`run.setOptions`, Clay JS `agent.setRunOptions`): Prism
  0.5.5 policy caps, all defaulting to `null` (unbounded) — tokens, turns,
  tool rounds, tool calls, wall time. A host that wants a fence sets finite
  values via `run.setOptions`. Provider
  request/response bytes are Prism HARD 64 MiB (null not allowed; omitting
  them used to leave DEFAULT 8 MiB). `compaction` defaults
  to `llm`. Call from `init.js` or a trusted config module; queued while the
  daemon is down. `compactAfterTokens` default 800_000 is stored and unused
  until auto-compact is wired (OM still uses decision 2158 / 80_000 via
  `agent.compact`).
- Opt-in wiki knowledge base (plan 108 task 12, decision 2156): off by
  default. `knowledge.setOptions { workspaceRoot, wiki }` — forwarded by the
  trusted `op_clay_agent_knowledge_set_options` (queued while the daemon is
  down, applied at initialize) — loads `@arnilo/prism-memory/wiki` via
  `kernel.load` with `autoDeploySkills: false` (wiki writes stay inside the
  workspace `.wiki/` tree; skills come from the kernel registry with
  progressive disclosure via `load_skill`). The loaded extension registers
  `/wiki-init`, `/wiki-refresh`, `/wiki-lint` commands (bare kernel names —
  the prompt intercept and `commandDispatch` both fall back from `/name` to
  the bare form), `wiki_search`/`wiki_read_page`/`wiki_record_insight`
  tools (appended to coding sessions whose workspaceRoot matches the
  binding), and the `wiki-searcher`/`wiki-maintainer` skills (resolved
  after the base skills; `skillList` filters them out while disabled).
  One wiki binding per daemon — enabling for another workspace disposes
  the previous extension; a failed load leaves the option disabled (fail
  closed). Optional `qmdPath` enables host-owned qmd hybrid search; deny
  by default, catalog fallback otherwise. Disposing restores the exact
  disabled state: commands gone, tools unresolvable, catalog clean.
- Opt-in graft knowledge base (plan 108 task 13, decision 2156): same
  activation path and binding discipline as the wiki — `graft: true` on
  `knowledge.setOptions` loads `@arnilo/prism-memory/graft` via
  `kernel.load`, registering the `graft_ask`/`graft_grep`/`graft_callers`/
  `graft_skeleton`/`graft_map`/`graft_blast` pull tools (mode `pull`,
  default), the `graft`/`graft-build`/`graft-check`/`graft-viz` commands
  (bare names, reachable through the `/name` slash fallback), and a
  host-side `graft` skill copy (the package's skill body is not exported).
  `graftMode: "push"` additionally wires the retrieval pack, first-turn
  orientation injector, and edit-watch (`graft:dirty` on mutating-tool
  results, persisted as graft-state entries against the run in flight —
  ceiling: the extension API carries no session key on `getEntries`, so
  patches ride the active run). CLI resolution (`cliPath` → host package
  root → `@nanonets/graft` peer) runs BEFORE load and fails closed: an
  absent CLI leaves the option off with tools hidden and the agent
  unperturbed. The CLI runs as a host-owned child (bounded time, capped
  stdout, telemetry off unless explicitly allowed); package JavaScript
  never spawns it. Disabling restores the exact disabled state.
- Web/browser capability surfacing (plan 108 task 14, decision 2159): the
  Phase 1 Obscura harness assembles the full surface — `obscura_*` MCP
  tools, `web_search`/`web_fetch` (replaceable HTML search profile),
  native `obscura_fetch`/`obscura_scrape` (public-HTTP(S)-only URL
  validation, byte/count/timeout caps, `allowEval` deny-by-default,
  untrusted-content labeling), and the prism browser tools
  (`browser_open`/`browser_snapshot`/`browser_act`/policy-gated
  `browser_evaluate`/`browser_close`, plus CDP block/throttle/emulate)
  over the connectOverCDP attachment. Coding sessions get the tools
  through `capabilityTools()` when the engine is present; everything is
  hidden when the binary is absent (capability reduction, never an
  error). Assembly-failure cleanup closes the MCP child and the serve
  process (no leaked half-started harness). Browser egress containment:
  loopback/private hosts denied by default; external egress requires the
  contained-proxy attestation.
- Filesystem skill discovery (plan 117): on the first session for a
  workspace, the daemon discovers repo-level skills from three roots —
  workspace `<root>/.agents/skills/`, the per-agent config root
  `<agentConfigRoot>/skills/`, and the home root `<homePath>/skills/`
  (default `~/.agents`) — via Prism's `discoverContributions` and merges
  them into the kernel registry (duplicate names fail closed, so reserved
  agent-delivered names `graft`/`wiki-searcher`/`wiki-maintainer` are
  skipped by disk discovery). Gating + bounds live in
  `<agentConfigRoot>/skills.json` (`loadSkillsConfig`): per-root enable
  flags, a home path override (tilde-expanded, absolute only — a
  relative path disables the home root fail-closed), per-agent-skill
  toggles, `MAX_SKILLS_PER_ROOT = 64`; an absent or malformed file
  defaults everything on with a bounded stderr warning. Discovered
  skills whose `toolNames` are not a subset of the session's active
  tools are skipped at activation (never brick a session); built-in
  skills keep the fail-closed semantics.
- Prompt layers (plan 117): at session build the daemon composes two
  file-backed layers on top of the profile's base instructions — the
  user-owned `SYSTEM.md` (per-agent config root, seeded EMPTY when
  absent, source `user`, rank 0) and the workspace `AGENTS.md` (source
  `app`), both through a shared `readPromptLayer` with a 64 KiB cap;
  oversized/unreadable files are skipped with a warning (SYSTEM.md) or
  silently (AGENTS.md, which additionally enforces realpath containment
  so symlink escapes are excluded). Layers are byte-stable per session
  so they ride the cached prompt prefix; edits apply on next session
  build. The context inspector renders every layer by source rank with
  friendly labels (plus a base-instructions item, so instruction-only
  profiles never show an empty System Prompt group).
- Seeded agent-delivered skill files (plan 117): `graft`,
  `wiki-searcher`, and `wiki-maintainer` are file-backed — install/first
  launch seeds `<agentConfigRoot>/skills/<name>/SKILL.md` from built-in
  definitions and records `{sizeBytes, mtimeMs}` in
  `.seed-manifest.json` (the settings page's provenance source).
  Tool names stay daemon-owned (user edits cannot break activation);
  content always loads from disk (256 KiB cap, oversized falls back to
  the built-in seed, user file never clobbered); deletions regenerate
  from the seed. Edits apply on next daemon start.
- MCP configuration surface (plan 117, amending decision 1758's
  posture): the allow list is CONFIG-BUILT, not empty. The server
  merges `<configRoot>/agents/coding-agent/mcp.json` (user-owned) with
  the repo-root `.mcp.json` (`src/server/agent_mcp_config.rs`):
  user-file-wins on server-id collision, bare command names are
  PATH-resolved canonically server-side (the daemon still rejects
  anything non-absolute — two-layer design), relative paths rejected,
  `MAX_MCP_SERVERS = 32`, `MAX_ARGS = 64`; malformed/unreadable files
  warn and contribute nothing (never fail startup). Both files connect
  WITHOUT an approval gate — the user's config and their repo are
  trusted sources by decision (security posture is the user's
  responsibility, as with Claude Code). Per-server fault isolation
  (`Promise.allSettled` in `mcp.ts`): a failing/over-cap server hides
  only its own tools; per-server outcomes (`{serverId, connected,
  tools, error}`) ride `environment.list` → the panel's MCP card and
  composer section. Call timeout: `timeoutMs` (positive, ≤ 30 min hard
  ceiling), default 60 s.
- Graft default-on (plan 117): the FIRST coding session for a workspace
  root attempts the graft pull-mode binding once per root per daemon
  (`graftBindAttempted`); CLI resolution is the plan 108 fail-closed
  chain (explicit `HostOptions.graftCliPath` test seam → host package
  root → `@nanonets/graft` peer) — absent CLI means no binding, no
  tools, no error surface. The graft skill body registers from disk
  regardless of tool availability (usage guidance independent of the
  tools). The wiki counterpart stays opt-in: the daemon intercepts
  `/wiki-init` as a prompt prefix, enables the binding internally, then
  dispatches the extension command (with the Rust environment cache
  invalidated for that prefix); `/wiki-init` with wiki gated off stays
  a chat-safe prompt.

## Coding tools and document reverse-RPC (Phase 1)

Coding sessions register nine Prism tool factories from
`coding-tools.ts` — `shell`, `read`, `write`, `edit`, `repo_list`,
`repo_search`, `glob`, `delete`, `move` — plus `ask_user_decision`, all
backed by Clay document operations instead of direct filesystem access.
`withCapErrors` turns walk/scan truncation (`entries`/`files`/`depth`/`bytes`/`time`)
into a tool-caps.json remedy error (the caps file is per-agent config:
`<agentConfigRoot>/tool-caps.json`, legacy data-dir path still honored).
Pagination (`results`/`matches`, including
per-call `maxResults`) stays a successful truncated result.
`document-ops.ts` implements the tool side of a daemon-initiated reverse
RPC: `document.read` (dirty buffer via server snapshot), `document.write`
(`apply_edit` + CAS save), `document.edit` (same via Prism edit ops), and
`document.stat`. The Rust server side lives in `src/server/agent_documents.rs`
and always acts as the server runtime identity (client 0), acquiring the
same lease (`open_existing_file_unlocked`) and going through `apply_edit`,
the single mutation path — the agent can never bypass CAS/leases, and a
user-held lease fails the tool closed.

Acceptance policy (decision 2157): workspace writes and reads are free;
out-of-workspace writes, delete/move, and shell metacharacter commands are
permission-gated. Gate checks run before the tool executes; with
`fullAutonomy: true` (host-set only, default false) gated calls skip the
prompt. `interruptBeforeTool: true` means gated calls suspend the durable
run and surface as `pendingDecisions` even before the gate prompt.

Inline gating rides the reverse-RPC approval bridge (Phase 2):
`approval.request` (mutation gate) and `approval.askUserDecision` surface to
connected clients as `clay.approvalRequest` AG-UI custom events and resolve
via the `ApprovalResolve`/`AskDecisionResolve` client commands. Timeout,
dropped consumer, or no subscriber denies fail-closed (the daemon maps any
error to deny, so the tool never executes).

How It Works items 6–15 cover the remaining Phase 1 families (compaction/OM,
skills/commands/drivers, durable runs, search/tree/checkpoints, MCP
allow-list, Obscura). Phase boundaries: Phase 2 adds the Chat UI for these
facades, Phase 4 package contribution feeding of skills/commands, Phase 5
workflows (`startWorkflow` driver errors until then), Phase 6 supervisors.

## Coding Agent panel surfaces (plan 117)

`frontend/src/coding-agent/CodingAgentPanel.tsx` renders the plan 117
user-visible surfaces, all state-driven from snapshot state / AG-UI custom
events (never invented client-side):

- **Skills card** — pinned at the top of the transcript from session start
  (persists once messages arrive); lists the catalog skills
  (name+description) that the daemon discovered from the three roots.
- **MCP card + composer section** — per-server connection outcomes
  (`{serverId, connected, tools, error}`) from snapshot state; failed
  servers show their error, hidden when empty. Display-only (no
  restart/connect actions).
- **@ mentions** — trailing `@token` opens a sectioned dropdown (Skills +
  Files, type-to-filter, ArrowUp/Down/Tab/Escape); `@skill:<name>` embeds
  the skill as an explicit user instruction (body loaded for that run),
  `@file:<path>` attaches file content (images as image blocks).
  Workspace file list comes from the `workspace.files` daemon RPC
  (bounded 200 paths, depth 8).
- **Token meter** — status-row occupancy `used/ceiling` from the LAST
  provider turn's prompt tokens (never the run-total `agent_finished`
  usage, which double-counts) vs the model's context window (resolved
  client-side from the models inventory); warning above 60%, error above
  80%; live updates ride the `clay.contextTokens` AG-UI custom event;
  when usage is unreported the meter estimates from transcript
  chars-per-token (calibration, not measurement).
- **Effort dropdown + branch from session start** — effort levels resolve
  client-side from the models inventory's `thinkingLevels` (no longer
  gated on the first prompt); the git branch is recorded at session
  creation, not just on rebind.
- **Labeled /resume** — Files-tab recent rows render human labels (first
  user-message words) from the workspace-scoped `session.resumable` list;
  selection resumes the full transcript without an entry-less snapshot
  clobbering it. Unit-variant agent commands (`listSessions`,
  `resumableSessions`) deserialize from the bare-string wire form ONLY —
  the `{variant: {}}` map form is rejected by serde (protocol invariant,
  pinned by a codec test).
- **Resume row identity** — a `/resume` row is the session's opening prompt,
  not its profile: every session in a workspace shares one profile, so the
  picker rendered N identical rows. The daemon stamps that prompt (first five
  words, bounded) on the session's **first** entry's `label` through the store
  seam (`labelFirstPromptStore` in `clay-agent/src/host.ts`); Prism's session
  search reads the *newest non-null* label, so one stamp keeps the identity
  stable for the session's whole life and later prompts never re-label it.
  Only entries with no `parentId` are eligible, which is stateless (no
  in-memory "already labelled" set to lose on restart). Rows also carry the
  last-active time as a local `YYYY-MM-DD HH:MM` (`updatedAtLabel`) beside the
  raw ISO value — the daemon runs on the user's machine, so its timezone is
  the user's, and the server-rendered picker has no timezone database (no
  `chrono`/`time` dependency). Read paths (`session.list`, `session.load`,
  search) stay on the unwrapped store. Sessions written before the stamp read
  "Untitled session"; nothing is backfilled.
- **Resume must restore the transcript** — `session.load` answers with
  persisted `SessionEntry` records, `{ kind, message: { role, content:
  [blocks] }, summary }`. `snapshot_from_load` parsed a flat `{ role, content }`
  shape that no daemon ever sent: every entry found neither role nor text and
  was dropped, so resume always restored an empty transcript and the click
  looked inert. The parser now walks the real shape and maps blocks to the
  same rows the live path builds (`apply_tool_event`): text → user/assistant,
  `thinking` → thinking, `tool_call` → a tool row carrying its arguments and
  call id, `tool_result` → the `"… -> output"` suffix on that same row (one row
  per call, never a second). Image/video/file blocks carry no transcript text,
  matching the live path.
- **A resume binds the tab's root, not just its session** — `session_for_root`
  *prunes* a tab whose recorded root does not match `current_root`, so
  `resume_tab` recording `tab_session` without `tab_session_root` meant the
  prompt after a resume started a brand-new session and silently abandoned the
  one the user had just opened.

## How It Works

1. `main.ts` refuses Node < 20, requires `--data-dir`, reads NDJSON JSON-RPC
   concurrently (`readNdjsonConcurrent`). Reverse-RPC replies share stdin with
   long `session.prompt` calls; awaiting each line handler before the next
   line deadlocks `document.stat` / `document.read` until the 30s reverse timeout.
2. `initialize { passphrase }` opens the vault (exit 1 if unreadable) and SQLite.
3. `--mock` registers `createMockProvider`; production loads `providers.ts`.
4. Azure/Bedrock/Vertex packages are installed but only register auth stubs
   until a later task supplies endpoint/region/project.
5. `session.prompt` uses `session.stream` with `maxQueuedEvents: 256` and
   `overflow: "drop_oldest"`. Events go out as `{ method: "event", params }`.
6. `session.compact` runs Prism `session.compact()`; an active run fails closed.
   Named strategies: `default`, `llm`, `om`. OM attaches only on opt-in
   (`observationalMemory: true`); worker models come from host config, not the
   session model; `compactAfterTokens` default 80000 (2158). OM workers
   derive their own kernel correlation id (`om:{attached session id}`, plan
   113: asserted on worker generate options); LLM compaction reuses the
   agent session id so summaries hit the same prompt cache. A positive
   `compactAfterTokens` on `session.compact` overrides the OM auto-compaction
   threshold for that session via the OM settings provider.
7. `skill.register`/`skill.list` keep a kernel skill registry (duplicate names
   fail closed). Profiles list skills by name; the host validates each skill's
   `toolNames` against active tools before any provider turn and adds Prism's
   `load_skill` tool for progressive disclosure (catalog = name+description
   only).
8. `command.register`/`command.dispatch` run host commands. RPC carries inert
   command data plus a host-side `handler` name (`startRun`/`startWorkflow`/
   `steer`); `CommandDrivers` are injected at dispatch time and never accepted
   over RPC. Drivers are absent when no live session exists (context key
   omitted). `startWorkflow` returns the Phase 5 "not in this phase" error.
   Phase 2 adds the pi-parity slash surface: `session.prompt` intercepts text
   whose first token matches a registered command name (exact match; `{json}`
   args parse as the args object, free text carries as `{ input }`;
   unknown `/x` stays a prompt — chat-safe). The dispatch result rides the
   transcript lane as a synthetic `agent_started`/`message_delta`/
   `agent_finished` triple (runId `cmd-<name>`): the result renders as an
   assistant message and the client's AG-UI run closes — the bare dispatch
   reply would leave the run observable open and the composer stuck.
   Built-in daemon handlers
   back the Coding Agent surface: `compact`, `newSession`, `checkout`,
   `discard`, `forkSession`, `cloneSession`, `tree` (branch summaries,
   checkpoints, branches from live entries), `openSession`, and
   `openSessionAsFork` — each routes to the documented session RPCs; none
   widen tool or acceptance authority.
9. Profiles must be re-registered after a daemon restart; session rows survive.
10. Coding prompts run with Prism `runState` (`interruptBeforeTool: true`,
    SQLite checkpoint store, revision `clay-agent.1`); a suspended prompt
    returns `status: "suspended"` + `runId` + `version` + redacted
    `pendingDecisions`. `run.resume` validates decision shape first, then
    CAS-resumes via `resumeAgentRun` (stale version / fingerprint mismatch
    fails closed; dispatched tools never replay). Chat stays non-durable.
    `sessionPrompt` drains via `session.stream()` (not `subscribe()+run()`:
    durable subscriptions stay open past settlement and would hang the RPC
    reply) and rebuilds the suspended payload from the `agent_suspended`
    stream event. Provider/model switching (plan 108 task 9) rebuilds the
    session's agent with the new model config (`recreateSessionModel`) —
    durable runs fingerprint on AgentConfig and reject per-run model
    overrides; the switch is validated against registries, persisted to the
    session record, and survives resume; the rebuild only runs when the
    target differs from the live book (the server echoes the current model
    every prompt — a needless rebuild would reset the leaf) and carries the
    old `leafId` over so the next append chains onto the same branch.
    `model.list` carries
    `contextWindow`; `Finished` events and snapshots carry `contextTokens`
    for the surface's context-size-vs-window status row.
    Every coding run carries a host-verified identity (`createAgent` config
    + run options; never sourced from RPC params): the daemon is the trust
    boundary that owns the workspace root and acceptance policy, so it
    vouches for its own runs with `{tenantId: "clay", principal:
    {kind: "service", id: "clay-agent"}, scopes: ["workspace"], verified:
    true}`. Durable tool effects require this — without it every mediated
    write fails closed with `ERR_PRISM_TOOL_EFFECT_CONFLICT`, including
    post-resume turns. `run.resume` projects the same ownership via
    `ownershipFromIdentity` so checkpoint reads/writes match the run's
    recorded scope (mismatch = "Checkpoint ownership mismatch").
11. `session.new` stamps `metadata.workspaceRoot`
    (`SESSION_SEARCH_WORKSPACE_METADATA_KEY`) so Prism's FTS search is
    workspace-scoped. `session.search` wraps `persistence.searchSessions`
    with `tenantId` + `workspaceRoot` (cross-workspace queries return empty,
    not an error); hits are transcript metadata (sessionId, leafId, label,
    summary, redacted snippet) and are never auto-injected into agent
    context. Tool output is not indexed by Prism FTS (tool_result blocks are
    not text), so raw tool output is not searchable by design. Search uses
    Prism's SQLite FTS schema (v4) with Clay's indexing policy — same
    store, no second database. Plan 108 task 11 surfaces it as the
    Command Centre `SessionSearch` picker (builtin
    `agent.clientOpenSessionSearchPicker` + a Coding Agent panel button):
    each query keystroke calls `AgentHost::search_sessions(tab, query, 50)`
    — the tab's bound session supplies the workspace scope — and installs
    the bounded hit page (local filtering is skipped for this kind; the
    index already matched). Items encode `search:{sessionId}|{leafId}`;
    activating returns `Resume { session_id, entry_id }` →
    `resume_tab` → `session.load { entryId }`, whose daemon handler
    (`branchEntries`) returns the read-only path root→matched entry —
    the live leaf and tree are untouched, so opening a hit never injects
    transcript content into agent context; the next prompt resumes from
    the live leaf. Unknown entry ids fall back to the default tail.
12. `session.checkpoint` → reverse `checkpoint.capture` snapshots every open
    document buffer server-side, keyed `(sessionId, entryId)` in
    `agent_checkpoints.rs`. `session.checkout` restores the checkpoint first
    (server routes snapshot text through `open_existing_file_unlocked` +
    `apply_edit`, the same lease/CAS path as all mutations; buffer-only —
    no disk rewrite per decision 2200), then re-roots the leaf.
    A foreign-lease document fails the restore and the checkout closed; a
    missing checkpoint restores as a no-op so pure-conversation checkout
    works. Snapshots are in-memory (daemon restart → restore becomes no-op).
    Plan 108 task 10 (decision 2200): before the leaf moves, the abandoned
    path (current leaf back to the branch point) gets a `kind: "summary"`
    entry parented at the branch point — on the NEW branch, pi's
    branch_summary model. It lands immediately as a deterministic bounded
    preview (≤ 2 KiB, ≤ 24 entries) and is refined in the background by a
    one-shot no-tool LLM run (`refineBranchSummary`, worker agent on the
    session's provider with a memory store, never throws into checkout);
    the refined entry appends as a sibling and re-roots the leaf only when
    the session has not moved on. `checkpoint.list` (reverse, advisory,
    2 s budget) flags which entries carry document checkpoints for the
    `/tree` render. `session.checkout` returns
    `{leafId, summaryEntryId, summarizing: true}`.
    `/discard` is checkout semantics with a two-step confirm: without
    `confirm` the dispatch replies `{needsConfirm, entryId}` and changes
    nothing; with `confirm` it restores the checkpoint and re-roots —
    nothing deleted, the abandoned side keeps its entries and summaries.
13. `session.fork` returns a live session on the same id at a leaf (abandoned
    sides kept). `session.clone` pre-creates the tenanted session record
    (Prism's clone entry-append otherwise auto-creates a `tenant_id: NULL`
    row), then copies the branch to a new session id with `workspaceRoot`
    metadata stamped. The Rust `SessionTree` client command forwards
    checkout/fork/clone/checkpoint to the daemon.
14. MCP is allow-list-only (decision 1758, config surface added plan 117):
    the server builds `mcpAllowList` from the per-agent `mcp.json` + the
    repo-root `.mcp.json` (see Responsibilities) and sends it in
    `initialize` (`AgentMcpAllowListEntry` on `AgentHostConfig`); the
    daemon validates every entry fail-closed (absolute executable,
    literal argv, explicit env names) before any spawn, then connects
    per-server with fault isolation via `@arnilo/prism-mcp`. Empty list
    → no MCP tools; validation errors fail closed; per-server
    connection failures hide only that server's tools. The daemon's
    spawn inherits exactly `HOME`/`USERPROFILE`/`PATH` (config-root
    isolation + bare-command PATH resolution happen server-side; the
    launch test proved the isolation end-to-end). Per-server outcomes
    ride `environment.list` as `mcpServers` for the panel's MCP card;
    MCP is display-only in the UI (no restart/connect actions).
15. Obscura is host-owned (decision 2159): binary resolves from
    `CLAY_OBSCURA_BIN` → `PATH` → `/usr/local/bin/obscura`; absent = hidden,
    never an error. Present: the daemon owns the `obscura serve` (CDP on
    127.0.0.1 only) and `obscura mcp` children, registers the complete
    advertised tool surface with the `obscura_` prefix plus Prism browser
    tools (`playwright-core@1.61.0` is the exact CDP-composition peer —
    never `connect`/`launch`), and kills both children on shutdown.
    Capabilities activate lazily on the first coding session; nothing is
    called sandboxed. `/brave` `/exa` `/firecrawl` subpaths are not imported.

## Spawn

```text
node clay-agent/dist/main.js --data-dir DIR [--agent-config-root DIR] [--mock]
```

The data dir is `<configuration-root>/agents/coding-agent/data` (the
user's `~/.clay/agents/coding-agent/data` — decision 2026-09-10-1526
moved runtime data under the per-agent root), falling back to the system
temp dir only when no config root exists. The per-agent root holds
config and content (`skills.json`, `mcp.json`, `tool-caps.json`, the
seeded `skills/` directory, `SYSTEM.md`, `.seed-manifest.json`) and the
data subdir holds runtime state. A legacy `<config-root>/agent/` data
dir is renamed into `agents/coding-agent/data` at server start (rename
failure keeps the legacy dir serving — credentials never orphan).
The SERVER passes `--agent-config-root
<configuration-root>/agents/coding-agent` explicitly (plan 117); without
it the daemon derives the root from the home directory — the launch test
showed that path leaking the real home under the daemon's env-clear
spawn, which is why the server always passes it. The spawn environment is
cleared except `HOME`/`USERPROFILE`/`PATH` (`for_server`), so MCP bare
commands resolve and homedir follows the isolated profile. The data dir
holds
`sessions.sqlite`, `credentials.vault`,
`vault.passphrase`, and `book.json` (the server-side persisted
provider/model/profile selection, reloaded at server boot so a configured
book survives restarts). First request:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"passphrase":"…"}}
```

The daemon also registers a built-in minimal `Chat` agent profile at boot —
the server's `ensure_tab_session` default profile must always resolve —
and `session.prompt` passes unlisted model ids through (the discovery
catalog is convenience, not authority; the provider rejects bad ids).

## Security

- No ACP, AG-UI, or Antigravity dependencies; no package JS can spawn or
  speak to the daemon (`agent_protocol::phase25_dependencies_deny_acp_agui_mcp`
  + `package_runtime_cannot_import_a_daemon_handle`).
- Obscura and MCP children are same-user subprocesses spawned by the daemon
  itself (not sandboxed, not privileged); the MCP allow-list is validated
  server-side before any spawn, and the Obscura binary is host-resolved —
  never a config or package string.
- No `process.env` secrets: provider credentials come from the encrypted
  vault / OS keychain through the stored credential resolver.
- Write authority: workspace writes are free, out-of-workspace writes are
  gated (decision 2157); all document mutations route through the server's
  lease/CAS path (`apply_edit`) regardless of caller.
- Search hits are redacted metadata and never auto-injected into agent
  context.
- MCP servers connect with no approval gate (plan 117 amendment to
  decision 1758): the config surface is user-owned (`mcp.json`) or
  repo-owned (`.mcp.json`) and both are trusted sources by decision —
  the security posture is the user's responsibility, as with Claude
  Code. What stays enforced: the daemon only accepts absolute commands
  (the server canonicalizes bare names via PATH), argv/env stay
  literal and bounded, servers cap at 32, and MCP children are
  same-user subprocesses owned by the daemon.
- Prompt layers and seeded skill files are size-capped (64 KiB /
  256 KiB) and never execute anything — they are prompt text only;
  symlink escapes for workspace `AGENTS.md` and `@file:` mentions are
  excluded by realpath containment.

## Invariants

- Frames > 1 MiB fail closed.
- Secrets never appear in RPC results, events, or logs.
- Omitted tools/skills activate none; unknown names throw before a provider turn.
- `session.load` payloads are persisted `SessionEntry` records; the server
  parses `entry.message.{role,content}` and never a flat `{role,content}` — a
  mock that speaks the flat shape is lying about the contract (it hid an
  always-empty resume).
- A resumed tab records both `tab_session` and `tab_session_root`; recording
  only the session makes the next `ensure_tab_session` prune the binding and
  start a fresh session.
- Only the session's first entry is labelled, so a session's `/resume`
  identity never drifts to its latest prompt.
- No ACP, AG-UI, or Antigravity dependencies. Phase 1 pins coding-tools,
  web-tools, memory, and mcp; import only `/compaction/llm` and
  `/compaction/observational-memory` from memory (no wiki/graft/rag).

## Clay JS API surface (`clay:agent`)

User-facing host controls are exposed through the trusted-only
`clay:agent` facade (`runtime/js/agent.js`, ops in
`src/server/ops/agent.rs`): `compact`, `searchSessions`,
`setFullAutonomy`, `resumeRun`, and `sessionTree`. Each op forwards to the
daemon's validated RPC and fails closed with `agent.unavailable` when no
agent host is installed (process-global `AgentHostHandle`, installed at
server startup). The package-facing registration ops
(`profileRegister`, `skillRegister`) are the exception: malformed
declarations fail closed with `agent.invalid_params`, but a missing host
queues the declaration process-globally (`PENDING_PACKAGE_REGISTRATIONS`)
and `install_global` moves it into the host's pending queue, where it
applies right after the daemon's first initialize handshake — package
load entries never fail or block for a missing host or daemon. The package (third-party) extension registers none of
these ops and cannot import `clay:agent`. Rust protocol additions:
`AgentClientCommand::{SetAutonomy, SearchSessions, RunResume}` and
`AgentServerMessage::AgentRpc`; `session.setAutonomy` toggles the live
session's `fullAutonomy` flag (default false, host-set only).

## Tests

```text
cd clay-agent && npm test
```

Fourteen suites, 138 tests (2026-09-10): host (mock
prompt/persist/resume, cancel, oversize frames, secret redaction, missing
tools, unreadable-vault process exit), rpc framing, coding tools
dirty-buffer read, CAS write, lease fail-closed, acceptance policy, D1
smoke), compaction (strategies, active-run fail-closed, OM round-trip,
threshold override), durable run (suspend/resume once, stale fail-closed,
chat non-durable), skills/commands (duplicate fail-closed, progressive
disclosure, driver injection denied, wiki intercept, three-root skill
discovery + skills.json gating), mentions (@skill/@file parsing,
containment, unavailable-tool skips), session search/tree (workspace
scoping, no-injection, fork/clone, checkpoint capture/restore), and
MCP/Obscura (allow-list validation order, per-server fault isolation,
missing-binary-hidden, no-vendor-imports). Rust side:
`tests/protocol.rs` `agent_protocol::*` (protocol round-trips, deny list,
reverse-RPC, `resume_after_daemon_load_restores_bounded_history` against a
mock daemon that speaks the real persisted entry shape),
`src/server/agent_documents.rs` /
`agent_checkpoints.rs` / `agent_mcp_config.rs` / `agent_settings.rs` unit
tests, `src/server/agent.rs` `tab_workspace_tests::*`
(`book_selection_broadcast_keeps_the_tab_session`,
`resumed_tab_keeps_its_session_on_the_next_prompt`),
`src/server/agent_picker.rs`
`session_picker_rows_show_the_label_and_the_local_stamp`, and the frontend
CodingAgentPanel suites (cards, mentions, token meter, effort/resume/branch —
301 tests).

## Related

- [Phase 25 Agent Host and Pane Content Primitive Review](../archive/phase25-agent-host-primitive-review.md)
- [Agent Host project pattern](../../../.agents/skills/clay-execution/references/packages.md)
- Reference docs (authoritative for public usage): `docs/reference/clay-js-api/agent/`
- Configuration reference: `examples/config/README.md` (skills.json / mcp.json /
  SYSTEM.md / SKILL.md schemas, defaults, apply semantics) and
  `docs/reference/clay-js-api/configuration.md`
- Manual test modules: `test-plan/16-agent-host.md`, `test-plan/17-coding-agent-parity.md`
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
- `decision-logs/2026-09-10-1526-clay-root-home-datadir-per-agent.md` (root at
  `~/.clay`, per-agent data dir, tool-caps.json as agent config)
- `plans/117-Phase2.2-Skill-Discovery-Wiki-Graft-Prompt-MCP-Context-Inspector.md`
