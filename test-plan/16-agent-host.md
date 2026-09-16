# 16 — Agent host (clay-agent, Phase 1)

Manual verification for the Phase 1 agent-host configuration surfaces: the
`clay:agent` facade controls (autonomy, compaction, search, session tree)
documented in `examples/init.js` section 12 and the `docs/reference/clay-js-api/agent/`
pages. Daemon-side behavior (coding tools, dirty buffers, approvals, durable
runs, workspace-scoped search, checkpoints, MCP allow-list, Obscura) is gated
by automated suites — listed under each step as "Automated" — so the manual
steps here cover only what a human can observe on a real build.

Deep references: `clay-agent/README.md`, `docs/wiki/modules/clay-agent.md`,
decision logs 2157 (autonomy default), 2158 (OM compaction defaults), 0714
(process authority), plan 107.

## Setup

```bash
cargo build
cd clay-agent && npm install && npm test   # daemon suites cited below
cp -r examples/. /tmp/clay-agent-manual-config/   # isolated config root
mkdir -p /tmp/clay-agent-manual-ws && echo "# scratch" > /tmp/clay-agent-manual-ws/notes.md
```

Launch the server with the isolated config and a workspace containing at
least one open document. The agent surface is the Coding Agent pane
(`@clay/coding-agent`, loaded by `packages/first-party.js`); the `@clay/chat`
landing this module originally drove was removed by plan 118, so its
`chat/fixture-*` and `chat-landing/` captures in module 09 are historical.
Most checks below are configuration-level and server-log observations.

## Configuration steps (init.js section 12)

| # | Action | Expected |
|---|--------|----------|
| A1 | `node --check examples/init.js`; launch with the copied example tree | Check passes; server starts with no `runtime.*` diagnostics; the agent surface (the Coding Agent pane contributed by `@clay/coding-agent`) renders unchanged by this configuration work — no new panels, tools or prompts come from the Phase 1 facades. The Chat surface this row originally referenced was removed with `@clay/chat` (plan 118): no step in this module loads or expects it |
| A2 | Read section 12 of `examples/init.js` | Documents the five `clay:agent` exports (`agent.compact`, `agent.searchSessions`, `agent.setFullAutonomy`, `agent.resumeRun`, `agent.sessionTree`) with commented examples only; the active/uncommented configuration is unchanged and copy-safe; no API keys or Obscura/MCP executable paths anywhere in the file |
| A3 | Cross-check the option names/enums/defaults in section 12 against `docs/reference/clay-js-api/agent/compact.md` and `set-full-autonomy.md` | `strategy` enum is `default`/`llm`/`om`; autonomy default `false` (decision 2157); `compactAfterTokens` default `80000` (decision 2158). Names match the inventory (`docs/reference/clay-js-api/api-inventory.toml`); no hidden-key alternative exists |
| A4 | Confirm no `agent*`/`provider*` credential option exists in `clay:configuration` and no API key appears in `examples/` | Provider credentials remain vault/keychain-only (set on first use of the agent surface); grep of the example tree shows no secret-shaped strings |

## Plan 121 RPC note (toolNames)

| # | Action | Expected |
|---|--------|----------|
| A20 | In a protocol/client harness call `session.prompt` with `toolNames` omitted, `[]`, a known subset, an unknown name, and malformed values | Omitted keeps the full registry; `[]` grants no tools; a known subset is forwarded; unknown names fail closed before a provider turn; malformed shapes return `-32602`; `run.resume` cannot widen the original grant. This is daemon RPC behavior, not a `clay:agent` or `init.js` surface (automated: `clay-agent/src/__tests__/tool-names.test.ts`) |

## Autonomy (decision 2157)

| # | Action | Expected |
|---|--------|----------|
| A5 | Fresh session, no `setFullAutonomy` call; trigger a gated tool call (e.g. agent writes outside workspace roots, or a shell-metacharacter command) | Approval prompt appears; the call does not execute until approved. Autonomy is off by default — no init.js key can pre-enable it (Automated: `session.setAutonomy toggles full autonomy; default stays false`, `acceptance policy: in-root writes free, out-root gated by approval`) |
| A6 | Enable autonomy for one session via the host (`agent.setFullAutonomy({ sessionId, enabled: true })`); repeat A5's gated call | Call executes without the approval prompt; other sessions are unaffected; restarting or starting a new session is off again (autonomy is session state, not persisted config) |
| A7 | Inspect `docs/reference/clay-js-api/agent/set-full-autonomy.md` custom properties | `default:boolean=false` documented as the autonomy default with an explicit statement that no settable "default autonomy" init.js key exists |

## Compaction (decisions 2158 / defaults)

| # | Action | Expected |
|---|--------|----------|
| A8 | In a coding session, request manual compaction (`agent.compact({ sessionId })`) with no strategy | Uses the session default (`om` for OM-attached sessions, otherwise `default`); a compaction entry appears in the persisted session; an active run fails closed instead of queueing (Automated: `manual compact on a mock session appends a compaction entry`, `session.compact while a run is active fails closed`) |
| A9 | Request `strategy: "llm"` with a provider configured, then with none configured | With provider: `llm` compaction entry. Without: fail-closed typed error, no partial entry |
| A10 | Override the OM threshold (`agent.compact({ sessionId, compactAfterTokens: 40000 })`); verify later auto-compaction triggers near the lower threshold | Override accepted (positive integer); negative/zero/non-numeric values rejected fail-closed (`-32602`); non-OM sessions ignore the value (Automated: `compactAfterTokens override validates and reaches the OM settings provider`). This is the OM threshold; the coding-session auto-compaction ceiling is `agent.setRunOptions` `compactAfterTokens` (module 17 C50–C52) — the two are distinct knobs |
| A11 | OM-attached session: check `recall` behavior | Observations are recorded during runs; recall is exact-id only and never auto-injects memory text into context (Automated: `OM attach records an observation; recall round-trips a known id; invalid id fails closed`) |

## Session search / tree / checkpoints

| # | Action | Expected |
|---|--------|----------|
| A12 | Run `agent.searchSessions({ sessionId, query })` for a phrase from an earlier session in this workspace, then for a phrase from a session opened under a different workspace root | Hits only include sessions stamped with the current workspace identity; hits carry `sessionId`+`leafId` metadata and are never appended to the conversation or injected into agent context (Automated: `session.search is workspace-scoped and returns hits with leafId`, `search hits are transcript data: checkout moves leaf without appending hit text`) |
| A13 | Search for text that exists only inside a tool result (e.g. bytes a `read` tool returned) | No hit: raw tool output is not indexed for search (only user/assistant text blocks are) |
| A14 | Use `agent.sessionTree({ sessionId, method: "checkout", leafId })` on a leaf that has a document checkpoint | Document buffers for that leaf are restored (buffer-only, left dirty for review) and the conversation leaf moves; the checkout does not delete the abandoned branch (Automated: `session.checkpoint captures via reverse RPC; restore failure fails checkout closed`) |
| A15 | `fork` then `clone` a session | Fork keeps the same `sessionId` at a different leaf; clone produces a new persisted `sessionId` with copied workspace metadata (Automated: `fork and clone produce independent branches/sessions`) |

## MCP allow-list and Obscura (fail-closed)

| # | Action | Expected |
|---|--------|----------|
| A16 | Start with no `mcp.json` in the data dir (empty allow-list); open a coding session | No MCP tools are offered, no subprocess is spawned, no error surfaces (Automated: `empty allow-list connects nothing and close is a no-op`) |
| A17 | Add an allow-list entry whose `command` is not a canonical absolute path; start the daemon | Validation rejects the entry before any connection attempt; no partial spawn (Automated: `allow-list validation precedes any connection (fail closed, no partial spawn)`, `allow-list entry with non-canonical command is rejected`) |
| A18 | Ensure no Obscura binary is resolvable (`CLAY_OBSCURA_BIN` unset, no binary on PATH); open a coding session | Browser/Obscura tools are simply absent from the tool list — hidden, not an error; agent-session initialize is unaffected (Automated: `missing Obscura binary resolves to undefined`, `host hides Obscura tools when binary absent; initialize unaffected`) |
| A19 | Confirm the daemon source has no direct web-search/browser-vendor imports and no package-declared MCP | Automated: `no brave/exa/firecrawl imports in daemon source`; `phase25_dependencies_deny_acp_agui_mcp` pins the Rust deny list |

## Recorded results (Linux, 2026-09-02, Phase 1 task 14)

Automated evidence gathered on this host at plan-107 task-14 time; live GUI
interaction steps above remain the standing manual procedure (driving the
agent surface is Phase 2 scope, module [17](17-coding-agent-parity.md); no
Phase 1 step requires it).

| # | Result | Evidence |
|---|--------|----------|
| Auto-config | PASS | `node --check examples/init.js` clean; canonical-example doc-registry tests green (Ctrl+B marker, import pins); protocol suite 203/203; section 12 documents all five exports with commented-only examples (diff limited to that comment block); no agent-surface change came from Phase 1 |
| Auto-docs | PASS | `agent_configuration_options_are_documented_custom_properties_with_decision_defaults` (tests/clay_js_api_inventory.rs) pins `default:boolean=false` and `compactAfterTokens:number=80000` in inventory+docs; daemon source keeps `DEFAULT_COMPACT_AFTER_TOKENS = 80_000`; no secret-shaped strings or credential options in `examples/` |
| Auto-autonomy | PASS automated | `session.setAutonomy toggles full autonomy; default stays false`; `acceptance policy: in-root writes free, out-root gated by approval` (clay-agent suites 49/49); inventory custom property `default:boolean=false` with no settable init.js key |
| Auto-compaction | PASS automated | `manual compact on a mock session appends a compaction entry`, `session.compact while a run is active fails closed`, `strategy override llm with mock summary provider…`, `compactAfterTokens override validates and reaches the OM settings provider` |
| Auto-recall | PASS automated | `OM attach records an observation; recall round-trips a known id; invalid id fails closed`; `agent session without OM attach has no recall tool` |
| Auto-search | PASS automated | `session.search is workspace-scoped and returns hits with leafId`, `search hits are transcript data: checkout moves leaf without appending hit text` (session-search-tree.test.ts) |
| Auto-tree | PASS automated | `session.checkpoint captures via reverse RPC; restore failure fails checkout closed`; `fork and clone produce independent branches/sessions` |
| Auto-mcp-obscura | PASS automated | `empty allow-list connects nothing and close is a no-op`, non-canonical rejection, validation-precedes-connection, missing-Obscura-hidden, no-vendor-imports tests (mcp-obscura.test.ts); Rust `phase25_dependencies_deny_acp_agui_mcp` green |
| Live GUI steps | NOT RUN (host, as recorded in prior modules) | Same documented ceiling as modules 01–15: no safe keyboard/window targeting backend on this host (see index records for Plans 097/099/105). No Phase 1 manual step is weakenable by this; steps above remain the procedure for an input-capable host |

No existing module step was deleted or weakened by Phase 1. Plan 118 removed
the `@clay/chat` package and the chat landing this module originally drove, so
the rows above name the agent surface instead; the agent host session's
automated coverage (tool-free profile) is unchanged.

## Plan 120 pin record (2026-09-16, Prism 0.7.0 / Node >= 22)

Automated-only cut: the clay-agent family moved 0.5.5 → 0.7.0 in one jump,
the daemon process floor rose to Node >= 22, and unknown Prism `AgentEvent`
types are now dropped by the Rust mapper instead of being mapped to
`AgentEvent::Started`. None of this is observable in the agent surface, so no
step above changed and no live pass is claimed. The standing A-steps remain
the procedure for an input-capable host.

| # | Result | Evidence |
|---|--------|----------|
| Pin lockstep | PASS automated | `cargo test --test protocol phase25_dependencies_deny_acp_agui_mcp`: exact `0.7.0` for `@arnilo/prism`, `prism-core`, `prism-providers`, `prism-coding-tools`, `prism-web-tools`, `prism-memory`, `prism-mcp`, plus `better-sqlite3@13.0.3` and `playwright-core@1.63.0`; README carries `0.7.0` and `Node >= 22`; ACP/AG-UI/MCP deny list intact |
| Node floor | PASS automated + host | Daemon guard is `MIN_NODE = 22` (private, not an `init.js` option); initialize reports `prism: "0.7.0"`; host runs Node v24.19.0; server diagnostic string is `Node >= 22 is required for clay-agent but was not found` |
| Unknown-event drop | PASS automated | `cargo test --lib map_event`: `map_event_drops_unknown_event_types` covers `attention_compiled`, `subagent_started`/`subagent_stopped`, `delegation_started`/`delegation_finished`, and a non-existent type; each maps to `None`, and the Rust router turns `None` into `DaemonLine::Ignore` |
| Daemon suite | PASS automated | Fresh `clay-agent npm test`: 149 pass / 0 fail / 1 skip |
| Live GUI steps | NOT RUN | Same host ceiling as the Phase 1 record and the plan 119 record; this cut adds no user-visible chrome, so no manual step is weakenable by it |

Note for a future runner: the A-step text above still says `examples/init.js`;
the canonical example file gate-read by the suites is
`examples/config/init.js` (`node --check` passes there). That path staleness
predates plan 120 and is recorded here rather than reused as a pass claim.

## Plan 123 work-scope record (2026-09-16, Prism 0.7 per-prompt scopes)

Automated-only cut. Plan 123 attaches an internal Prism work-scope per coding
prompt on OM-attached sessions and nests a closed child scope per delegated
subagent run. Scopes are daemon-side `om.scope.*` ledger entries: no panel,
control, transcript row, or Memory-tab chrome was added, so no A-step above
changed and no live pass is claimed. The Memory tab keeps rendering the
existing OM activity (`session.om.activity` worker state and observation /
reflection feed) exactly as before and is **not** required to show a scope
outline. A11 recall semantics are unchanged: recall stays exact-id only, with
no scope-filtered or widened identifier lookup, and no memory text is
auto-injected into context. The standing A-steps remain the procedure for an
input-capable host.

| # | Result | Evidence |
|---|--------|----------|
| Work-scope attach | PASS automated | `om work scope: OM-on coding prompt opens, enters, and leaves one run scope` — `om.scope.opened` / `entered` / `left` for `run:<sessionId>:1`, observations bound to the run scope |
| Work-scope gate | PASS automated | `om work scope: OM-off coding prompt writes zero scope entries` — no scope ledger entries on non-OM coding sessions |
| Fail-closed ids | PASS automated | `om work scope: an invalid scope id fails closed before the prompt starts` |
| Scoped projection vs recall | PASS automated | `om work scope: per-run projection stays separate while exact-id recall sees the whole branch` — a second run's projection excludes the first run's observations while exact-id recall still resolves them |
| Child scope nesting | PASS automated | `om work scope: a delegation nests a closed child scope under the run scope` — `child:<sessionId>:1` with `parentId` = the run scope, depth 2, never entered |
| Recall rules unchanged | PASS automated | `OM attach records an observation; recall round-trips a known id; invalid id fails closed`; `chat session without OM attach has no recall tool` (unchanged suites) |
| Daemon suite | PASS automated | Fresh `clay-agent npm test`: 180 pass / 0 fail / 1 pre-existing skip (181 total; `spawn-agent` supervisor steps apply unchanged) |
| Live GUI steps | NOT RUN | Same host ceiling as the plan 120 record; this cut adds no user-visible chrome, so no manual step is weakenable by it |
