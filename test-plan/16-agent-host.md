# 16 — Agent host (clay-agent, Phase 1)

Manual verification for the Phase 1 agent-host configuration surfaces: the
`clay:agent` facade controls (autonomy, compaction, search, session tree)
documented in `examples/config/init.js` section 12 and the `docs/reference/clay-js-api/agent/`
pages (the example tree moved under `examples/config/` while these rows were
written as `examples/init.js`; the path was corrected in the plan 130 record). Daemon-side behavior (coding tools, dirty buffers, approvals, durable
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
cp -r examples/config/. /tmp/clay-agent-manual-config/   # isolated config root (~/.clay)
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
| A1 | `node --check examples/config/init.js`; launch with the copied example tree | Check passes; server starts with no `runtime.*` diagnostics; the agent surface (the Coding Agent pane contributed by `@clay/coding-agent`) renders unchanged by this configuration work — no new panels, tools or prompts come from the Phase 1 facades. The Chat surface this row originally referenced was removed with `@clay/chat` (plan 118): no step in this module loads or expects it |
| A2 | Read section 12 of `examples/config/init.js` | Documents the six `clay:agent` exports (`agent.compact`, `agent.knowledgeSetOptions`, `agent.searchSessions`, `agent.setFullAutonomy`, `agent.resumeRun`, `agent.sessionTree`) with commented examples only; the active/uncommented configuration is unchanged and copy-safe; no API keys or Obscura/MCP executable paths anywhere in the file |
| A3 | Cross-check the option names/enums/defaults in section 12 against `docs/reference/clay-js-api/agent/compact.md` and `set-full-autonomy.md` | `strategy` enum is `default`/`llm`/`om`; autonomy default `false` (decision 2157); `compactAfterTokens` default `80000` (decision 2158). Names match the inventory (`docs/reference/clay-js-api/api-inventory.toml`); no hidden-key alternative exists |
| A4 | Confirm no `agent*`/`provider*` credential option exists in `clay:configuration` and no API key appears in `examples/` | Provider credentials remain vault/keychain-only (set on first use of the agent surface); grep of the example tree shows no secret-shaped strings |

## Plan 121 RPC note (toolNames)

| # | Action | Expected |
|---|--------|----------|
| A20 | In a protocol/client harness call `session.prompt` with `toolNames` omitted, `[]`, a known subset, an unknown name, and malformed values | Omitted keeps the full registry; `[]` grants no tools; a known subset is forwarded; unknown names fail closed before a provider turn; malformed shapes return `-32602`; `run.resume` cannot widen the original grant. This is daemon RPC behavior, not a `clay:agent` or `init.js` surface (automated: `clay-agent/src/__tests__/tool-names.test.ts`) |

## Autonomy (decision 2157 — **superseded in behavior**, see the plan 130 record)

**Current behavior (verified 2026-09-20):** a session created without an
explicit `fullAutonomy` is **autonomous** — approvals are opt-out
(`clay-agent/src/host/sessions.ts`: `params.fullAutonomy !== false`, comment
"user decision 2026-09-05"), so a gated call executes without a prompt at
creation time. The docs/inventory still document the older
`default:boolean=false` (decision 2157), and a **resumed** session comes up
non-autonomous because `ensureLive` writes the live record with
`fullAutonomy: false` while the session itself is created from the recorded
value. The steps below keep both directions reachable and the divergence is
recorded as a finding; a doc-or-code decision is needed.

| # | Action | Expected |
|---|--------|----------|
| A5 | Fresh session, no `setFullAutonomy` call; trigger a gated tool call (e.g. an out-root write, or a shell-metacharacter command) | The call executes with **no** prompt: autonomy is on by default (decision 2026-09-20-2049). Gated-when-off is pinned by `acceptance policy: in-root writes free, out-root gated by approval`, `… shell metacharacters gated`; the default by `session.setAutonomy toggles full autonomy; default stays true` and by the inventory gate, which now pins the documented `default:boolean=true` **and** the daemon expression `params.fullAutonomy !== false` |
| A6 | Block autonomy for one session via the host (`agent.setFullAutonomy({ sessionId, enabled: false })`); repeat A5's gated call | The gated call now needs an approval decision; other sessions are unaffected; autonomy is session state, not persisted config (a fresh session starts autonomous again). A **resumed** session restores the autonomy recorded with it — created autonomous it resumes autonomous, created with `fullAutonomy: false` it resumes gated (decision 2026-09-20-2049; pinned by `resumed session binds its tools to the recorded workspace root, not the daemon cwd` and `resumed session keeps a recorded autonomy block (approvals stay armed)`) |
| A7 | Inspect `docs/reference/clay-js-api/agent/set-full-autonomy.md` custom properties | `default:boolean=true` is documented with an explicit statement that no settable "default autonomy" init.js key exists, and the page states plainly that gated calls run unprompted unless autonomy is blocked (inventory `api-inventory.toml` carries the same `default:boolean=true`). The divergence this step recorded is resolved by decision 2026-09-20-2049 |

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

## Resume binding (workspace root, not the daemon cwd)

| # | Action | Expected |
|---|--------|----------|
| A25 | Create a coding session in workspace root B (a real graft workspace), stop the daemon, then start it again with its launch cwd set to an unrelated directory A and resume that session | The resumed session reports and uses B — `session.resume` echoes the **recorded** root, tool cwd and acceptance roots are B, never A or the daemon's cwd (a session created without a root keeps the documented `process.cwd()` fallback, which is what makes the pair meaningful). Resume also **re-activates the workspace's coding surfaces**, because a restarted daemon has neither: `environment.list` shows the graft extension loaded for B and the allow-listed MCP server re-connected, and a graft tool is offered again (Automated: `resumed session binds its tools to the recorded workspace root, not the daemon cwd`; live protocol probe: `test-plan/artifacts/130-agent-host/live-daemon.log`, whose three re-activation legs fail when the activation is removed — `live-daemon-falsify.log`) |

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

Note for a future runner: the A-step text above said `examples/init.js`; the
canonical example file gate-read by the suites is `examples/config/init.js`.
That path staleness predates plan 120 and is recorded here rather than reused as
a pass claim. **Resolved by the plan 130 record below** (the paths are corrected
in the steps, and `node --check examples/config/init.js` is a live PASS).

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

## Plan 130 execution record (2026-09-20, agent-host decomposition + resume binding)

Plan 130 injected the agent-host authority into the runtime lanes (no
process-global handle, per-lane registration queue) and split `ClayAgentHost`
into `clay-agent/src/host/*.ts` with every function inside the 80-line budget —
neither is observable from the agent surface, so the pass is a regression pass
plus the one behavioral fix (A2: a resumed coding session re-activates its
workspace's capabilities and graft binding, new step A25).

Build: `target/debug/clay` 19:36, `target/debug/clay-desktop` 19:37,
`clay-agent/dist` 19:37, `frontend/dist` unchanged (no frontend source newer).
Evidence: `test-plan/artifacts/130-agent-host/` (`README.md`, `live-daemon.log`,
`live-daemon-falsify.log`, `gui/`, `automated-legs.txt`, `config-legs.txt`).

| # | Result | Evidence |
|---|--------|----------|
| A1–A4 (config level) | PASS live | `node --check examples/config/init.js` parses; section 12 documents the six `clay:agent` exports with commented-only examples; inventory carries `compactAfterTokens:number=80000` and `default:boolean=false`; no uncommented credential option or secret-shaped string in `examples/` (`config-legs.txt`). Path/export-count corrections applied to the steps above |
| A5–A7 (autonomy) | PASS automated + **policy decided** | Fresh daemon suite pins both directions (`session.setAutonomy toggles full autonomy; default stays true`, `acceptance policy: out-root gated by approval`); live probe toggles `session.setAutonomy` both ways on the real daemon. The divergence this pass recorded — opt-out creation vs `default:boolean=false` docs/inventory, plus `ensureLive` writing its live record with `fullAutonomy: false` — was decided by the user (decision 2026-09-20-2049): **autonomy stays the default and the docs moved to it**; the resume path now restores the recorded value. Steps A5–A7 rewritten to the resolved policy, and `clay-agent/src/__tests__/resume.test.ts` gained a falsified case (`fullAutonomy: false` at creation → resumed session stays gated) |
| A8–A11 (compaction, OM/recall) | PASS automated + live | Fresh daemon suite; live probe `session.compact` appends a persisted compaction entry on the real daemon (3 entries, `compaction` kind present) |
| A12–A15 (search, tree, fork/clone) | PASS automated; live for search/fork/clone | Fresh daemon suite; live probe: workspace-scoped `session.search` (hit for its own session, no hit for a phrase nothing contains), `session.resumable` scoped to the recorded root (0 rows for another root), `session.clone` (new id), `session.fork` (same id, new leaf). `session.checkpoint` needs a document backend, so it stays automated-only |
| A16–A19 (MCP allow-list, Obscura, no vendor imports) | PASS automated + live | Fresh daemon suite; live probe: an allow-listed stdio MCP server connects (`environment.list` → `connected:true, tools:2`) and its `mcp:live:` tool is in the session's tool list, while an empty allow-list offers none; Obscura stays hidden (`CLAY_OBSCURA_BIN` empty) and no browser tool appears |
| A20 (toolNames) | PASS automated | Fresh daemon suite (`tool-names` cases) |
| **A25 (new: resume binding)** | **PASS live + automated** | Live protocol probe on the real build: session recorded in B (`/home/arn/Projects/clay`, a real graft repo), second daemon launched in an unrelated cwd A resumes it → `session.resume` reports B, graft skill + graft tool re-bound, `environment.list` shows the graft extension and the re-connected MCP server; the same probe with the activation removed fails exactly those three legs (`live-daemon-falsify.log`). Automated: `resumed session binds its tools to the recorded workspace root, not the daemon cwd` (fresh `npm test`, 181 pass / 1 skip / 0 fail) |
| Authority ownership (plan 130 A1) | PASS live + automated | Server log of the live GUI run: every `agentProfile.register`/`command.register` takes the `[agent-reg] … host=live -> Ok({"queued": true})` branch (the lane's injected host resolved — the A1 ownership path) and the daemon then reports `[daemon] … applied`; automated `server::tests::two_servers_in_one_process_own_independent_agent_hosts`, `agent_protocol::initialize_handshake_carries_the_built_mcp_allow_list` (protocol/security suites) |
| Regression suites | PASS automated | `cargo test` 0 failures (lib 1421 pass / 1 ignored; presentation 62; protocol 226; runtime 75; security 152); fresh `clay-agent npm test` 182 tests (181 pass / 1 skip / 0 fail); `frontend vitest run src/agent src/shell` 11 files / 125 tests |
| Live GUI steps (lane, composer, palette) | PASS live via AT-SPI | Isolated launch, no input synthesis: `Agent lane` + `Message` entry + `Coding Agent Agent type` picker present at rest; palette opened through the status-bar button lists all daemon slash commands (`/resume`, `/branch`, `/fork`, … `server-first — @clay/coding-agent@0.1.0`); the `Session` scope chip narrows it to those 14 rows; lane hide/restore verified by node counts (`gui/`) |
| Composer typing, send, approvals, session create/resume from the lane, `/resume` row activation | UNRESOLVED live | No keyboard/pointer synthesis on this host (standing ceilings: no `/dev/uinput`, no `xdotool`/`ydotool`, portal keyboard crash loop) and the isolated profile has no provider credentials (`[agent] ensure_tab_session(1): empty selection (provider='' model='')`, status bar `no provider configured · Settings · Providers`). Automated legs cited per row; A21 covers the resume semantics at protocol level |
| Portal screenshots | NOT RETAINED | The portal surface exposes only the currently visible workspace/monitor and the isolated client opens on another workspace; the first crop (from the plan-126/129 `portal-shot.py` copy) contained an unrelated desktop window and was deleted before entering the repo. `portal-shot.py` here is hardened (AT-SPI frame first, compositor rect only when inside it, fail-closed exit 2) and refused every capture — recorded as a follow-up for the older copies |

## Plan 124 steps (the lane is Clay-owned shell chrome, 2026-09-17)

Deep references: `DESIGN.md` §12, `docs/reference/packages/creating-packages.md`
(plan 124 authoring contract), `frontend/src/coding-agent/surface-state.ts`.

| # | Action | Expected |
|---|--------|----------|
| A21 | Open a tab with an agent, open the agent view, and inspect the lane + the view together; hide/show the lane; finally switch the tab's workspace root | The lane and the agent view read **one** tab session (the store is hoisted to the tab runtime): the same transcript, model, effort, and environment foot, and `session.list` shows one session for the tab — hiding/showing the lane forms no second session and no remount, and a workspace re-bind moves both surfaces together. Automated: `frontend/src/shell/WorkspacePanes.test.tsx` (one store per tab runtime), daemon session-isolation suite |
| A22 | With no provider configured, inspect the lane; then try to make a package render into the lane (package UI/layout manifest, overlay or panel contribution) | The lane is Clay-owned shell chrome, not a contribution slot: no package manifest field or SDUI contribution can compile into the lane's composer, controls, approval strip, or foot — package surfaces keep rendering through their existing panels/overlays/dialogs. The lane's foot reports host truth only (workspace root, git branch, loaded extensions, MCP connection summary) and with no provider it reports `no provider configured · Settings · Providers` with a disabled model trigger rather than a fabricated provider row. Automated: `packages/coding-agent` manifest tests, `tests/package_ui_conformance.rs`, `AgentLane.test.tsx` |

## Plan 125 steps (palette authority + agent-less truth, 2026-09-18)

Deep references: `docs/reference/packages/creating-packages.md` (plan 125
authoring contract), `docs/reference/ui-components.md`,
`design-artifacts/approved/composer-palette-stages/`,
`plans/125-Composer-Palette-Stage-Flows-and-Centered-Sheet-Retirement.md`.

| # | Action | Expected |
|---|--------|----------|
| A23 | Try to drive the palette from package context: open or close a session, request the veil or the halo, reach the credential stage, and ask for the retired centered anchor (`centered` in a package UI/layout manifest) | All four are Clay-owned: no package API or SDUI contribution opens, drives, or dismisses a palette session; the veil/halo belong to the composer palette and the `@` menu and cannot be requested; the shielded credential stage is not reachable from package code (no secret reaches a package frame); and the retired `centered` anchor value maps to a bottom / working-area projection instead of a window-centered surface (wire compatibility only — `PackageOverlayAnchor::parse("centered")` no longer yields a centered rect). The lane remains Clay chrome on the same terms as A22. Automated: `tests/package_ui_conformance.rs` (halo value, no new elevation chrome, centered-removal guards), `src/shell/package_ui.rs` anchor tests, `docs/reference/packages/creating-packages.md` contract pins |
| A24 | On a tab with no agent and no provider, type in the lane and submit; then click the agent picker with no agent types listed | The field is **always typable** (the palette and the picker stay reachable), so submitting reports the configuration error in the lane/foot rather than silently dropping the prompt, and `/` still opens the catalogue. With no agent types listed the picker offers `Attach an agent` and the foot states that nothing would send (`no agent types listed`) — no fabricated provider, model, or agent row appears, and no provider call is made. Automated: `AgentLane.test.tsx` (offers the agent picker and keeps the field typable with no agent; keeps typing open with no provider and states the reason in the foot), `Composer.test.tsx` (agent-less placeholder), `WorkspacePanes.test.tsx` (agent-less state from the approved artifact) |

## Plan 124 execution record (Linux, 2026-09-17)

| Step | Result | Evidence |
|---|---|---|
| A21 | PASS automated | One store per tab runtime, adoption/diposal on close, and shared derived state are pinned by `frontend/src/shell/WorkspacePanes.test.tsx` + `AgentLane.test.tsx`; the daemon-side one-session-per-workspace rule is unchanged by this plan. Live: the lane and the agent view rendered from the same tab session in the canonical launch (single `session.list` entry per tab). |
| A22 | PASS live (no-provider truth) + automated (boundary) | Live: the canonical config configures no provider, and the lane reported `no provider configured · Settings · Providers` with the disabled `Configure a provider` model trigger (capture `test-plan/artifacts/124-agent-lane/14-agent-attached.png`). The "no package UI in the lane" boundary is documented in the plan-124 authoring contract and pinned by the package UI conformance tests. |

## Plan 125 execution record (Linux, 2026-09-18)

| Step | Result | Evidence |
|---|---|---|
| A23 (palette authority + retired centered anchor) | PASS automated | The plan-125 authoring contract is documented and pinned: packages cannot open/drive the palette, request the veil or halo, or reach the shielded stage (`docs/reference/packages/creating-packages.md`, `docs/reference/ui-components.md`, `tests/clay_js_doc_registry.rs::plan125_configuration_contract_keeps_picker_flows_palette_owned`); the picker command ids stay inert as server-side built-ins (`src/server/command_execution.rs::builtin_picker_commands_stay_inert_command_ids`); the retired centered anchor maps to a bottom projection (`src/shell/package_ui.rs` anchor tests, `src/server/menu_sessions.rs::no_session_constructor_produces_the_retired_centered_origin`); and the halo is a value on existing recipes rather than new elevation chrome (`tests/package_ui_conformance.rs::plan125_palette_and_mentions_take_the_halo_not_a_drop_shadow`). NOT RUN live (needs package JS + row activation; the standing host ceiling). |
| A24 (agent-less lane truth) | PASS live (states) + PASS automated | Live (canonical config): the lane's agent-type picker showed the adopted `Coding Agent`, the model trigger was the disabled `Configure a provider` row, the foot reported `no provider configured · Settings · Providers`, and the composer field stayed present and `entry Message` typable (capture `01-rest`, `accessibility.txt`). Submitting a prompt with no provider and the `no agent types listed` variant are pinned by `AgentLane.test.tsx` / `Composer.test.tsx` (no provider call, error surfaced, field never disabled). |
