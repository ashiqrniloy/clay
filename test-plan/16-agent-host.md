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
least one open document. The agent chat surface (@clay/chat, loaded by
`packages/first-party.js`) is the UI entry; there is no Phase 1 agent panel
beyond Chat (Phase 2 UI), so most checks below are configuration-level and
server-log observations.

## Configuration steps (init.js section 12)

| # | Action | Expected |
|---|--------|----------|
| A1 | `node --check examples/init.js`; launch with the copied example tree | Check passes; server starts with no `runtime.*` diagnostics; Chat loads as before — Chat UI chrome is unchanged from the pre-Phase-1 baseline (same surface, no new panels, tools, or prompts) |
| A2 | Read section 12 of `examples/init.js` | Documents the five `clay:agent` exports (`agent.compact`, `agent.searchSessions`, `agent.setFullAutonomy`, `agent.resumeRun`, `agent.sessionTree`) with commented examples only; the active/uncommented configuration is unchanged and copy-safe; no API keys or Obscura/MCP executable paths anywhere in the file |
| A3 | Cross-check the option names/enums/defaults in section 12 against `docs/reference/clay-js-api/agent/compact.md` and `set-full-autonomy.md` | `strategy` enum is `default`/`llm`/`om`; autonomy default `false` (decision 2157); `compactAfterTokens` default `80000` (decision 2158). Names match the inventory (`docs/reference/clay-js-api/api-inventory.toml`); no hidden-key alternative exists |
| A4 | Confirm no `agent*`/`provider*` credential option exists in `clay:configuration` and no API key appears in `examples/` | Provider credentials remain vault/keychain-only (set on first use in Chat); grep of the example tree shows no secret-shaped strings |

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
| A10 | Override the OM threshold (`agent.compact({ sessionId, compactAfterTokens: 40000 })`); verify later auto-compaction triggers near the lower threshold | Override accepted (positive integer); negative/zero/non-numeric values rejected fail-closed (`-32602`); non-OM sessions ignore the value (Automated: `compactAfterTokens override validates and reaches the OM settings provider`) |
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
| A18 | Ensure no Obscura binary is resolvable (`CLAY_OBSCURA_BIN` unset, no binary on PATH); open a coding session | Browser/Obscura tools are simply absent from the tool list — hidden, not an error; Chat initialize is unaffected (Automated: `missing Obscura binary resolves to undefined`, `host hides Obscura tools when binary absent; initialize unaffected`) |
| A19 | Confirm the daemon source has no direct web-search/browser-vendor imports and no package-declared MCP | Automated: `no brave/exa/firecrawl imports in daemon source`; `phase25_dependencies_deny_acp_agui_mcp` pins the Rust deny list |

## Recorded results (Linux, 2026-09-02, Phase 1 task 14)

Automated evidence gathered on this host at plan-107 task-14 time; live GUI
interaction steps above remain the standing manual procedure (Chat UI
interaction itself is Phase 2 scope; no Phase 1 step requires driving Chat).

| # | Result | Evidence |
|---|--------|----------|
| Auto-config | PASS | `node --check examples/init.js` clean; canonical-example doc-registry tests green (Ctrl+B marker, import pins); protocol suite 203/203; section 12 documents all five exports with commented-only examples (diff limited to that comment block); Chat UI chrome untouched by Phase 1 |
| Auto-docs | PASS | `agent_configuration_options_are_documented_custom_properties_with_decision_defaults` (tests/clay_js_api_inventory.rs) pins `default:boolean=false` and `compactAfterTokens:number=80000` in inventory+docs; daemon source keeps `DEFAULT_COMPACT_AFTER_TOKENS = 80_000`; no secret-shaped strings or credential options in `examples/` |
| Auto-autonomy | PASS automated | `session.setAutonomy toggles full autonomy; default stays false`; `acceptance policy: in-root writes free, out-root gated by approval` (clay-agent suites 49/49); inventory custom property `default:boolean=false` with no settable init.js key |
| Auto-compaction | PASS automated | `manual compact on a mock session appends a compaction entry`, `session.compact while a run is active fails closed`, `strategy override llm with mock summary provider…`, `compactAfterTokens override validates and reaches the OM settings provider` |
| Auto-recall | PASS automated | `OM attach records an observation; recall round-trips a known id; invalid id fails closed`; `chat session without OM attach has no recall tool` |
| Auto-search | PASS automated | `session.search is workspace-scoped and returns hits with leafId`, `search hits are transcript data: checkout moves leaf without appending hit text` (session-search-tree.test.ts) |
| Auto-tree | PASS automated | `session.checkpoint captures via reverse RPC; restore failure fails checkout closed`; `fork and clone produce independent branches/sessions` |
| Auto-mcp-obscura | PASS automated | `empty allow-list connects nothing and close is a no-op`, non-canonical rejection, validation-precedes-connection, missing-Obscura-hidden, no-vendor-imports tests (mcp-obscura.test.ts); Rust `phase25_dependencies_deny_acp_agui_mcp` green |
| Live GUI steps | NOT RUN (host, as recorded in prior modules) | Same documented ceiling as modules 01–15: no safe keyboard/window targeting backend on this host (see index records for Plans 097/099/105). No Phase 1 manual step is weakenable by this; steps above remain the procedure for an input-capable host |

No existing module step was deleted or weakened by Phase 1; Chat's existing
automated coverage (chat host session stays tool-free) is unchanged.
