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

## Coding tools and document reverse-RPC (Phase 1)

Coding sessions register nine Prism tool factories from
`coding-tools.ts` — `shell`, `read`, `write`, `edit`, `repo_list`,
`repo_search`, `glob`, `delete`, `move` — plus `ask_user_decision`, all
backed by Clay document operations instead of direct filesystem access.
`withCapErrors` turns walk/scan truncation (`entries`/`files`/`depth`/`bytes`/`time`)
into a tool-caps.json remedy error. Pagination (`results`/`matches`, including
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
14. MCP is allow-list-only (decision 1758): the server sends `mcpAllowList`
    in `initialize` (`AgentMcpAllowListEntry` on `AgentHostConfig`); the
    daemon validates every entry fail-closed (absolute executable, literal
    argv, explicit env names) before any spawn, then connects via
    `@arnilo/prism-mcp`. Empty list → no MCP tools; validation errors fail
    closed; connection failures hide the tools.
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
node clay-agent/dist/main.js --data-dir DIR [--mock]
```

The data dir defaults to `<configuration-root>/agent` (the user's
`~/.config/clay/agent`), falling back to the system temp dir only when no
config root exists. It holds `sessions.sqlite`, `credentials.vault`,
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

## Invariants

- Frames > 1 MiB fail closed.
- Secrets never appear in RPC results, events, or logs.
- Omitted tools/skills activate none; unknown names throw before a provider turn.
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

Nine suites, 49 tests: host (mock prompt/persist/resume, cancel, oversize
frames, secret redaction, missing tools, unreadable-vault process exit),
rpc framing, coding tools (dirty-buffer read, CAS write, lease fail-closed,
acceptance policy, D1 smoke), compaction (strategies, active-run fail-closed,
OM round-trip, threshold override), durable run (suspend/resume once, stale
fail-closed, chat non-durable), skills/commands (duplicate fail-closed,
progressive disclosure, driver injection denied), session search/tree
(workspace scoping, no-injection, fork/clone, checkpoint capture/restore),
and MCP/Obscura (allow-list validation order, missing-binary-hidden,
no-vendor-imports). Rust side: `tests/protocol.rs` `agent_protocol::*`
(protocol round-trips, deny list, reverse-RPC) and
`src/server/agent_documents.rs` / `agent_checkpoints.rs` unit tests.

## Related

- [Phase 25 Agent Host and Pane Content Primitive Review](../archive/phase25-agent-host-primitive-review.md)
- [Agent Host project pattern](../../../.agents/skills/clay-execution/references/packages.md)
- Reference docs (authoritative for public usage): `docs/reference/clay-js-api/agent/`
- Manual test module: `test-plan/16-agent-host.md`
- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`
