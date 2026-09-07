# Clay Coding Agent and `st` Autonomous Agent Roadmap

Status: draft for user iteration. This roadmap supersedes the completed
Tauri + React migration roadmap as the repository's forward plan. The
migration roadmap is finished (all 21 tasks of `plans/097` checked) and
remains in git history; its governing decision
(`decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`) stays
authoritative for architecture rules that this roadmap inherits.

Governing decisions already made:

- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`:
  Clay-owned Node `clay-agent` daemon wraps Prism directly; no ACP/AG-UI as
  the first-party agent bus; coding agent must reach CLI-parity inside Clay.
  Its original 0.3.0 pin was superseded by the 0.4.0 migration in Phase 0;
  live pins move to exact `0.5.0` in Phase 2.1 (`plans/109`).
- `decision-logs/2026-09-02-1440-direct-external-coding-agent-adapters.md`:
  Claude Code and Antigravity are direct, capability-declared external
  runtimes with Clay policy bundles, not Prism delegation.
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`:
  product surfaces are replaceable first-party packages; third-party packages
  extend/replace through declared extension points with user approval.
- Open decisions at the end need `decision-logs/` entries before their
  phase is planned. User-stated resolutions in this draft are not binding
  until logged.

## Product Shape

Three layers, strict separation:

1. **`clay-agent` daemon (Clay core, Node ≥ 20).** Hosts `@arnilo/prism`
   0.5.0 plus explicitly selected 0.5 family packages/subpaths. Owns native
   providers, models, credentials (vault + keychain), SQLite persistence,
   run ledger, tools, compaction strategies, skills, commands, workflows,
   supervision, and external-runtime lifecycle/policy projection. It does
   not proxy vendor authentication or own vendor agent loops. Packages never
   spawn or speak to it directly; they use public `agent.*` Clay JS APIs
   served by the Rust server.
2. **`@clay/coding-agent` (first-party Clay package, "base coding agent").**
   Minimal, pi-coding-agent-parity coding agent: coding tools against Clay
   documents, approvals, sessions, branching, compaction, steering, commands,
   plan files. Replaceable like `@clay/chat`; contains no workflow opinions.
3. **`st` (`@arnilo/st`, third-party Clay package on npm).** Author's own
   autonomous workflow, composed only from the base layer's primitives plus
   Prism packages. Installed with `clay install npm:@arnilo/st`. Not shipped
   or activated by default; nothing in the base layer depends on it.

The base agent stays minimal by design (pi-like). All autonomy policy
(phased roadmaps, sub-agent validation, per-task model routing, external
agent delegation, memory cadence) lives in `st` or in host-free orchestration
helpers the daemon exposes generically.

## Prism Capability Review (0.4.0 historical; live pins are 0.5.0)

Review method for the 0.4 cut: read the 0.4 migration guide,
package-consolidation plan, all then-11 published package manifests and
export maps, and the relevant family API docs in `/home/arn/Projects/prism`.
Prism 0.4 was a package/import migration with no persisted-store shape
migration. Phase 0 owned the Clay consumer smoke for that cut.

**Live pins (Phase 2.1 / plan 109):** Prism 0.5.0 lockstep, released
2026-09-06. Authoritative sources: `/home/arn/Projects/prism/docs/migrate-to-0.5.md`,
`CHANGELOG.md` `[0.5.0]`, `docs/thinking-and-reasoning.md`, `docs/mcp-tools.md`.
Package names and 0.4 subpaths stay valid. Breaking host surface: 27 dead
exports removed, MCP TypeScript SDK v2 modular (`@modelcontextprotocol/client`
+ `/server` 2.0.0, 2026-07-28), two thinking-effort wire moves, child-env
allow-list, keyring 2 locked-store errors, `better-sqlite3` 12 → 13. No
persisted-schema migration. Publishable families are 10 (`prism-antigravity-agent`
workspace removed in 0.5 — Clay already did not adopt it). Context7 has no
`@arnilo/prism` library.

0.4 table below remains the capability map. 0.5 deltas that Clay must honor
are in the following rows and in **Prism 0.5.0 lockstep (live)** after the
0.4 adoption map.

| Requirement | Prism primitive | 0.4 package/import | Verdict |
| --- | --- | --- | --- |
| Pi-parity coding tools | `shell`/`read`/`write`/`edit`, repository tools, opt-in Git tools, `coding_check`, and host `operations` seams | `@arnilo/prism-coding-tools/agent` | ✅ moved from `prism-coding-agent`; behavior retained |
| Approvals/sandboxing | `createCodingApprovalPolicy`, sandbox composition, `ExecutionPolicy` | `@arnilo/prism-coding-tools/security` | ✅ moved; security gates stay subpath-local |
| Sessions, branching, persistence | `AgentSession`; JSONL root export; SQLite/Postgres/NATS adapters | `@arnilo/prism`; `@arnilo/prism-core/sessions/sqlite` | ✅ SQLite remains Clay's store; `better-sqlite3` becomes an explicit optional peer |
| Commands and durable decisions | `CommandDefinition`, host `CommandDrivers`, `interruptBeforeTool`, lifecycle resume, pending-decision CAS | `@arnilo/prism` | ✅ unchanged root contract |
| Workflow-type picker and plan files | ask-user decision tools; `writeCodingPlanFile`/`parseCodingPlanTodos`; coding checkpoints | `@arnilo/prism-coding-tools/agent` | ✅ moved; 0.3.2 fixes retained |
| Loop-until-goal orchestration | workflows, replay, sagas, goal verification, bounded host-loop pattern | `@arnilo/prism-core/runtime/workflows` | ✅ moved from `prism-workflows` |
| Custom per-run loops and declarative agents | `AgentLoopStrategy`, `generateValidateReviseLoop`, `AgentDefinition`, skill/command/agent registries | `@arnilo/prism` | ✅ unchanged root contract |
| Wiki and context graph | Wiki commands/tools/skills; Graft tools/push packs/blast radius | `@arnilo/prism-memory/wiki`; `@arnilo/prism-memory/graft` | ✅ moved; Graft optional peer remains fail-closed |
| Sub-agent fan-out | `createSupervisor`, child models/permissions/budgets, durable nested approvals | `@arnilo/prism-core/runtime/supervisor` | ✅ moved from `prism-supervisor` |
| Per-task model selection | run/agent model overrides and `resolveUseCaseModel` | `@arnilo/prism`; `@arnilo/prism-core/governance/model-router` | ✅ moved router |
| External coding-agent delegation | Direct vendor runtime adapters, Clay policy projection, resume, event projection | Claude Code Agent SDK; Antigravity headless CLI | Phase 9–10; no Prism intermediary |
| Observational memory and recall | observe/reflect/drop workers, fast compaction, exact-id recall, OM commands | `@arnilo/prism-memory/compaction/observational-memory` | ✅ moved from standalone compaction package |
| LLM compaction | coding LLM compaction strategy | `@arnilo/prism-memory/compaction/llm` | ✅ moved; profile-only `prism-compaction` removed |
| MCP tools | bounded MCP client/server/OAuth bridge; 0.5 hosts the 2026-07-28 spec through modular SDK v2 (`@modelcontextprotocol/client` + `/server` 2.0.0). Clay uses `connectMcpTools` only — no direct SDK import | `@arnilo/prism-mcp` | ✅ package name retained; 0.5 transport is SDK v2 |
| Web search/fetch | generic providers plus Obscura web tools | `@arnilo/prism-web-tools`; `@arnilo/prism-web-tools/obscura` | ✅ Obscura moved under family subpath |
| Browser automation/e2e | CDP tools and Obscura/Playwright composition | `@arnilo/prism-web-tools/browser`; `/obscura` | ✅ moved; `playwright-core` stays opt-in |
| Linux desktop use | deny-by-default computer-use wrapper over host MCP binary | `@arnilo/prism-coding-tools/computer-use-linux` | ✅ moved under coding family |
| Validation/eval gating | scorers, datasets, experiments, thresholds | `@arnilo/prism-core/governance/evals` | ✅ moved under core governance |
| Provider adapters | 18 first-party adapters in 0.5 (`/hyper`, `/commandcode` added in 0.4.1/0.5); `/ai-sdk` exists and stays unused | `@arnilo/prism-providers/<adapter>` | ✅ one family dependency; explicit adapter imports, including hyper/commandcode in Phase 2.1 |
| JSON Schema tool validation | `createJsonSchemaToolArgumentValidator` | `@arnilo/prism-core/validation/json-schema` | ✅ moved under core validation |
| Node credentials | encrypted vault/keychain resolvers and OIDC | `@arnilo/prism-core/credentials/node` | ✅ moved; keyring dependency remains host-side |
| Office generation/parsing | documents, sheets, diagrams | `@arnilo/prism-office/{documents,sheets,diagrams}` | ⛔ not adopted: no Clay/`st` requirement |
| ACP interop | ACP adapter/CLI | `@arnilo/prism-acp-agent` | ⛔ not adopted: first-party bus decision unchanged |
| AG-UI/A2A/A2UI interop | Prism event adapters/renderers | `@arnilo/prism-ag-ui` | ⛔ not adopted in daemon: Clay keeps its bounded server-owned AG-UI projection |

## Prism Readiness: 0.4 Migration and Retained Integration Requirements

**Status: Prism 0.4.0 is published across all 11 active packages.** The cut
replaces 62 manifests with 11 packages, removes five profile packages, and
ships no compatibility wrappers. Prism documents this as a package-name and
import-specifier migration: persisted store schemas do not change. The 0.3.2
Clay intake remains present in moved code: D1 fixed, E1/E2/E3/E6 resolved,
and DOCS-1 documented. Entries below are historical behavior requirements to
re-run after import migration; only E4/E5 remain optional and open.

### Defects (required)

- **D1 — `suspendAskUserDecision` drops `allowCustom` when omitted. —
  [FIXED in 0.3.2]** `toAskUserDecisionSuspendData` now normalizes via
  `parseAllowCustom` (omitted → `false`; non-boolean fails closed at accept
  time, never at resume time). Verified in the published
  `@arnilo/prism-coding-agent@0.3.2` dist and now lives at
  `@arnilo/prism-coding-tools/agent@0.4.0`. Phase 1 smoke still carries the
  resume round-trip regression without explicit `allowCustom`.

### Enhancements (required unless waived per phase)

- **E1 — `resolveAgentDefinition` requires `model` even when
  `context.overrides.model` is supplied. — [RESOLVED in 0.3.2, FEATURE-1]**
  `buildBaseConfig` falls back `definition.model ?? overrides.model`; a
  definition with neither source still fails closed.
- **E2 — Documented host pattern (or primitive) for bounded
  iterate-until-done orchestration. — [RESOLVED in 0.3.2, FEATURE-2/6]**
  `docs/workflows.md` documents the bounded host-loop pattern: one
  `runWorkflow` per iteration, iteration state in workflow inputs, explicit
  termination predicate and budgets, typed fail-closed `BudgetExhaustedError`
  (never a hang), `replayWorkflow` per iteration run id.
  `examples/autonomous-coding-loop.ts` is the conformance reference (goal →
  roadmap → per-task supervisor children → goal-verify → OM attach +
  task-boundary compact + recall → human gate with simulated restart →
  host-side bounded iterate-until-done with deterministic budget exhaustion,
  mock providers only). In-graph `loop` node stays future Prism work
  (Plan 045) — Clay adopts the documented pattern now.

### Enhancements (optional; workarounds already in phase designs)

- **E3 — Commands that drive. — [RESOLVED in 0.3.2, FEATURE-3]**
  `CommandExecutionContext.drivers?: CommandDrivers`
  (`startRun`/`startWorkflow`/`steer`) — host-injected, never package-supplied;
  absent drivers leave the context shape unchanged. Clay's daemon injects
  drivers so `/start` and future package commands launch `st` workflows
  natively.
- **E4 — Supervisor child event passthrough.** Supervisor `subscribe()`
  emits delegation lifecycle metadata only. `st`'s UI wants optional per-turn
  child event streaming (redacted, capped) for nested-run visibility;
  milestone-level events are an acceptable v1.
- **E5 — Cross-session observational memory scope.** OM is per
  session/branch. `st` phases delegate to child sessions; an opt-in shared
  scope (or a documented host composition funneling child summaries into the
  parent session store) would let recall span a whole build. v1 acceptable:
  parent session records delegation outcomes, so parent OM covers them
  naturally.
- **E6 — Composite example.** A single `examples/autonomous-coding-loop.ts`
  combining supervisor + workflows + OM + goal-verify would freeze the
  intended composition and become Clay's conformance reference.

### 0.4 package-migration constraints

- Clay pins direct Prism dependencies to exact reviewed `0.4.0` versions even
  though Prism's internal peer window is `^0.4.0`.
- Replace package names and imports atomically. No runtime path may mix retired
  0.3 package imports with 0.4 family imports.
- Import only explicit subpaths. Installing a family package must not activate
  sibling providers, databases, browsers, parsers, binaries, tools, or agents.
- Add only peers Clay uses: `better-sqlite3` for SQLite and exact
  `playwright-core@1.61.0` when Obscura/browser work lands; retain host-owned
  Graft, Obscura, computer-use-linux, and `agy` resolution. Missing optional
  capability must stay hidden/fail closed. `@arnilo/prism-web-tools` requires
  `@arnilo/prism-mcp` as a non-optional peer — install them together in Phase
  1, not as a first-party agent bus.
- `@arnilo/prism-memory` includes `pg` as a regular dependency in 0.4.0 even
  though Clay does not adopt PostgreSQL memory. Accept the family footprint;
  do not initialize PostgreSQL or add a second memory store. Skip
  `prism-web-tools/{brave,exa,firecrawl}` and `prism-providers/ai-sdk`.
- Preserve rollback by keeping the current lockfile available until the 0.4
  consumer suite passes. Rollback is package/import restoration to exact 0.3
  pins; no database rollback is expected for this reorganization.

### Behavioral constraints (accepted, not defects — they shape the design)

- `session.compact()` fails closed while a run is active, so `st` maps one
  plan task to one run and compacts at task boundaries. This also makes
  "trigger compaction after each task" mechanical.
- A suspended workflow node's `execute` is re-invoked with `ctx.resume`
  after approval; resume-blind nodes re-suspend silently. Every `st` node
  must be written resume-aware (`ctx.resume ? answer : suspend(...)`),
  verified by the smoke suite after fixing the node, not Prism.
- Supervisor child factories must return an `Agent` with a stable config and
  durable store for nested approvals to resume; returning a session crashes
  delegation (host-side requirement, keep in authoring docs).
- Workflows bounds: `maxNodes` default 1,000 — generated phase DAGs are
  comfortably inside limits, and definition `revision` must bump when node
  behavior changes.
- External coding-agent delegation is core-owned and direct: Prism never
  manages Claude Code or Google credentials. Clay hides a runtime when its
  documented SDK/CLI is absent or unauthenticated, and never calls a
  same-user child sandboxed merely because it has a workspace root.

## Prism 0.4 Adoption Map (all active packages)

Inventory covers all 11 published 0.4 packages. Family installation and
subpath adoption are separate: a family can be present while unused subpaths
remain unimported and inert.

| Active 0.4 package | Clay subpaths/capabilities | Phase | Decision |
| --- | --- | --- | --- |
| `@arnilo/prism` | Root agent/session/run/tool contracts, extension kernel, skills, commands, durable lifecycle, Node helpers and test conformance exports | 0–1 | **Adopt.** Keep dependency-free root and exact `0.4.0` direct pin. |
| `@arnilo/prism-core` | Phase 0: `/credentials/node`, `/sessions/sqlite`, `/validation/json-schema`; Phase 1: selected `/runtime/server` helpers and `/sessions/codecs`; Phase 5: `/runtime/workflows`; Phase 6: `/runtime/supervisor`, `/governance/model-router`; optional Phase 8: `/governance/evals` | 0–8 | **Adopt selected subpaths.** Do not use Postgres/NATS, enterprise, policy/OPA, prompts, observability, or work integrations without later demand. |
| `@arnilo/prism-providers` | Current 16 adapters as `/alibaba`…`/zai` (not `/ai-sdk`) | 0 | **Adopt one family dependency.** Import/register adapters explicitly; no ambient provider activation. Keep Azure/Bedrock/Vertex as host-config stubs until those flows exist. |
| `@arnilo/prism-coding-tools` | `/agent`, `/security`; optional `/document-reader`; Phase 5 `/computer-use-linux`; optional `/caveman` and `/impeccable`; Phase 6 `/ponytail` | 1–6 | **Adopt selected subpaths.** Skip `/openapi` and `/dev`; parser/persona peers only when used. |
| `@arnilo/prism-web-tools` | Root `web_search`/`web_fetch`; `/browser`; `/obscura` | 1–2 | **Adopt.** Skip `/brave`, `/exa`, `/firecrawl`. Keep Obscura host-owned and hidden when absent. `prism-mcp` is a required peer of this family; add exact `playwright-core@1.61.0` only when browser/Obscura plumbing lands. |
| `@arnilo/prism-memory` | `/compaction/llm`, `/compaction/observational-memory`, `/graft`, `/wiki` | 1–7 | **Adopt selected subpaths.** Skip root working/vector memory and `/rag`; installing family does not authorize PostgreSQL, Graft, QMD, or Context7 processes. |
| `@arnilo/prism-mcp` | Bounded MCP client/server/OAuth bridge for allow-listed package declarations, Obscura, and Clay-exposed tools | 1–6 | **Adopt with Phase 1 web/coding uplift.** Required peer of `prism-web-tools`. No package-spawned daemon or ambient server. |
| `@arnilo/prism-antigravity-agent` | Prism Antigravity delegation adapter | — | **Do not adopt.** Phase 10 runs the documented `agy` headless protocol directly; host owns authenticated `agy` lifecycle. |
| `@arnilo/prism-ag-ui` | Prism AG-UI/A2A/A2UI adapter and renderer | — | **Do not adopt in `clay-agent`.** Clay server remains the bounded Prism→AG-UI adapter for React; revisit only if replacing that owned projection is separately approved. |
| `@arnilo/prism-acp-agent` | ACP adapter and CLI | — | **Do not adopt.** First-party agent bus remains Prism-native; external ACP adapter is post-roadmap. |
| `@arnilo/prism-office` | `/documents`, `/sheets`, `/diagrams` | — | **Do not adopt.** No coding-agent or `st` requirement justifies office dependencies. |

**Removed 0.4 profiles:** `@arnilo/prism-base`, `prism-code`, `prism-sdk`,
`prism-all`, and `prism-compaction` have no replacement package. Clay already
selects capabilities explicitly, so it installs the seven Phase 0/1 families
without recreating a local umbrella. External-agent adapters remain direct
vendor integrations in Phases 9–10.

**Retired-name rule:** all standalone 0.3 provider, core/session/governance,
coding/persona, browser/Obscura, RAG/compaction/Graft/Wiki package names are
migration references only after Phase 0. New code and roadmap phase plans use
family subpaths exclusively.

## Prism 0.5.0 lockstep (live pins, Phase 2.1)

Prism 0.5.0 is a lockstep cut: all publishable manifests move `0.4.x` →
`0.5.0`, internal ranges `^0.4.0` → `^0.5.0`. Clay live-pins the seven
adopted families at exact `0.5.0` plus `better-sqlite3@13.0.3`. Plan:
`plans/109-Phase2.1-Coding-Agent-Defects-and-UX-Improvements.md`. Decision
log: write during that plan (same class as `2026-09-02-0121`).

Host-visible 0.5 work Clay must do:

- **Thinking:** use `applyThinkingLevelForModel`; pickers read
  `capabilities.thinkingLevels`; Anthropic body field is
  `output_config.effort`; xAI sends `reasoning_effort`; Google family is
  real (0.4 silent no-op is fixed). This unblocks coding-agent effort UI.
- **MCP:** `@arnilo/prism-mcp` keeps `connectMcpTools`. Transitive SDK is
  `@modelcontextprotocol/client` + `/server` `2.0.0` (not monolithic
  `@modelcontextprotocol/sdk` 1.30.0). Spec target 2026-07-28. Clay source
  must not import SDK modules. Child stdio env is allow-listed — do not
  rely on ambient `process.env`. Draft MCP tasks stay unadvertised.
- **Peers:** `better-sqlite3` 13.0.3; keyring 2 typed locked-store errors
  (not empty vault). No sqlite schema migration.
- **Adapters:** load `/hyper` and `/commandcode` explicitly with the other
  providers. Azure/Bedrock/Vertex stay stubs. Skip office, ACP, AG-UI in
  the daemon. `prism-antigravity-agent` is gone upstream; Phase 10 stays
  direct `agy`.
- **Dead exports:** do not import the 27 removed symbols in
  `docs/migrate-to-0.5.md` §3.
- Rollback = exact 0.4.x pins; nothing persisted changes.

Change requests were filed in the Prism repo at
`docs/clay-integration-findings.md` (BUG-1/BUG-2, FEATURE-1..6, DOCS-1
mapping one-to-one to D1/E1–E6 plus docs constraints); Prism 0.3.2 landed
the intake. Prism 0.4.0 changes package/import locations, not those contracts.
Phase 0 therefore verifies moved exports and behavior instead of reopening the
upstream feature work.

## Phase 0: Prism 0.4.0 Family Migration and Verification

### Scope

- Replace the current 22 direct 0.3 dependencies with exact `0.4.0` pins for
  `@arnilo/prism`, `@arnilo/prism-core`, and `@arnilo/prism-providers` only.
  Add a direct `better-sqlite3` pin compatible with prism-core's `^12.11.1`
  optional peer. Drop unused `@arnilo/prism-model-router` (pinned, never
  wired). Do not add coding/web/memory/MCP/Antigravity families yet.
- Rewrite existing daemon imports: credentials →
  `@arnilo/prism-core/credentials/node`; SQLite →
  `@arnilo/prism-core/sessions/sqlite`; JSON Schema validator →
  `@arnilo/prism-core/validation/json-schema`; providers →
  `@arnilo/prism-providers/<adapter>`. Azure/Bedrock/Vertex stay host-config
  stubs under the new names. Family install activates nothing.
- Update daemon version reporting, `clay-agent/README.md`, wiki clay-agent
  pages, and `tests/agent_protocol.rs` (`phase25_dependencies_deny_acp_agui_mcp`
  still denies ACP/AG-UI/MCP/retired `prism-coding-agent`; README assertion
  becomes `0.4.0`). Reject every retired 0.3 package name in
  `clay-agent/package.json`, lockfile, and source imports.
- Consumer smoke at the new imports: credentials/vault, SQLite round-trip of
  an existing Clay fixture, JSON Schema validator, explicit provider
  registration, and isolation (installing the three families does not open
  Postgres/NATS, browsers, or extra adapters). Keep E4/E5 optional.

### Exit Gate

- `npm ci`, `npm run build`, and `npm test` pass in `clay-agent`; `npm ls
  --all` contains only `@arnilo/prism`, `@arnilo/prism-core`, and
  `@arnilo/prism-providers` from the Prism graph, plus `better-sqlite3`, and
  no retired package names.
- Startup reports Prism `0.4.0`; existing SQLite session fixtures open and
  round-trip without a schema migration; chat/mock flows stay byte-compatible.
- Rollback drill restores the committed 0.3 package/import set with `npm ci`;
  no database rollback or compatibility shim is required.

## Phase 1: Base Coding Agent Host Uplift (`clay-agent`)

### Scope

- Add exact `0.4.0` pins for `@arnilo/prism-coding-tools`,
  `@arnilo/prism-web-tools`, `@arnilo/prism-memory`, and `@arnilo/prism-mcp`.
  `prism-web-tools` requires the `prism-mcp` peer. Narrow
  `phase25_dependencies_deny_acp_agui_mcp` so ACP/AG-UI and retired 0.3 names
  stay forbidden while `@arnilo/prism-mcp` is allowed as the
  package-declared MCP bridge (Phase 1 used `@modelcontextprotocol/sdk`
  1.30.0 transitively; Phase 2.1 / Prism 0.5 replaces that with modular
  `@modelcontextprotocol/client` + `/server` 2.0.0 — still not a Cargo.toml
  or clay-agent direct SDK import). Keep D1 omitted-`allowCustom`
  resume in the daemon smoke suite at `@arnilo/prism-coding-tools/agent`.
- Register `@arnilo/prism-coding-tools/agent` tools with Clay-operation
  backends:
  `read`/`write`/`edit` route through Clay document authority (versions,
  leases, dirty buffers) via the documented `operations` seams per the
  2026-08-21 decision; `shell`/list/search/glob stay workspace-confined.
- **Default acceptance policy (both clay and `st` agents).** Inside the
  operating folder (workspace): full agent freedom, no approvals. Host
  system outside the workspace: reads allowed; any write/change requires
  explicit user permission. Egress (network, package installs) follows the
  existing sandbox/ExecutionPolicy rules. User option: a **full autonomy**
  toggle that lifts the outside-workspace write gate for the session/run;
  off by default.
- Add compaction strategies (root default +
  `@arnilo/prism-memory/compaction/llm` +
  `@arnilo/prism-memory/compaction/observational-memory`) and daemon RPC to
  select/per-run override.
- Add skills registry plumbing and package-contributed skill text
  (progressive disclosure, `load_skill`).
- Add command dispatch (`CommandDefinition` → daemon RPC) so packages can
  register `/verb` commands; inject host-owned `CommandDrivers`
  (`startRun`/`startWorkflow`/`steer`, retained in Prism 0.4.0) so contributed
  commands can drive runs/workflows natively.
- Add durable run support (`interruptBeforeTool`, lifecycle resume) and run
  state RPC for approval surfaces.
- **Workspace-scoped session index (shared by clay + `st`).** Store a stable
  workspace identity beside every SQLite session/branch/checkpoint; Clay owns
  a SQLite FTS index over user/agent messages, branch summaries, plan/task
  metadata, and OM reflections (not raw tool output by default). Search is
  workspace-scoped — never cross-workspace in v1 — and returns session,
  branch, matching entry/snippet, timestamp, and status. Search results are
  transcript data only, never automatic agent context.
- **Session tree + document checkpoints (pi `/tree` model, extended to the
  workspace).** Sessions stay single-file JSONL trees (`id`/`parentId`);
  `checkout`/`fork`/`clone` parity per pi. Document authority records a
  **version checkpoint** at every task boundary and user checkpoint, linked
  to the session entry active at that moment. Branching from entry E
  branches both the conversation and the document version tree at E's
  checkpoint — "discard" reverts the files and the conversation together;
  nothing is deleted, the abandoned side just stops being the leaf.
- Add `@arnilo/prism-mcp` wiring from package-declared MCP manifests
  (allow-listed). Not a first-party agent bus.
- **Obscura web/browser engine (host-owned binary).** Daemon lifecycle for
  the host-installed Obscura binary (fail-closed spawn, readiness, group
  close — mirrors `agy` handling); expose its complete advertised MCP
  surface, managed/external CDP (`connectObscuraCdp`), and Playwright
  `connectOverCDP` composition. Add exact `playwright-core@1.61.0` when this
  plumbing lands, not merely because `prism-web-tools` is installed. All
  capabilities hidden when the binary is absent. Skip `/brave`, `/exa`,
  `/firecrawl`.
- Keep mock-mode and existing chat flows byte-compatible.

### Exit Gate

- Linux gates pass: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, server/daemon tests.
- A session can run the nine coding tools against an open Clay document with
  dirty-buffer fidelity, approval prompts, and persisted history. D1 resume
  without explicit `allowCustom` still passes.
- Compaction (manual and threshold) runs mid-session; observational memory
  attach works against the daemon store.

## Phase 2: `@clay/coding-agent` Package (Minimal Base Agent)

### Scope

- First-party package registering the `coding` agent profile: coding tool
  set, system prompt layer, `create-plan`-style skill, slash commands
  (`/compact`, `/new`, `/branch`+checkout map, `/model` picker hook), plan
  file conventions (`plans/`), and the agent UI surface per the binding spec
  below, composed from existing SDUI primitives and the chat extension
  points.
- Pi-parity behaviors: streaming, steering/cancel, session list/resume,
  branch fork/checkout, manual + auto compaction, provider/model switching.
  Full tree parity: `/tree` in-place navigation with branch summaries
  (LLM summary of the abandoned path back to the common ancestor), `/fork`,
  `/clone`, `/n`, `--session`/`--fork` CLI equivalents — and post-hoc discard
  of any branch, which rolls conversation and workspace back to the linked
  checkpoint in one action.
- **Session search.** One workspace-scoped `session.search` RPC and session
  picker/command surface (not a new permanent tab) for clay and `st`: query
  the shared FTS index; selecting a result resumes/opens that session at the
  matching tree entry. Results never inject transcript content into the
  current agent context unless the user explicitly opens or attaches it.
- **Knowledge-base options (opt-in, per workspace).** Both load via
  `kernel.load(...)` only when the user enables them; both surface to `st`
  by inheritance (st extends this agent).
  - `@arnilo/prism-memory/wiki` (llm-wiki): per-codebase `.wiki/` knowledge base —
    `/wiki-init`, `/wiki-refresh` (SHA-256 Merkle-diff incremental),
    `/wiki-lint`; `wiki_search`/`wiki_read_page`/`wiki_record_insight` tools;
    OKF v0.2 bundles; `wiki-maintainer`/`wiki-searcher` skills; profiles
    codebase/pkm/hybrid/auto; optional `qmd` CLI for hybrid search with
    catalog fallback when absent.
  - `@arnilo/prism-memory/graft`: context-graph code search — pull tools
    (`graft_ask`/`graft_grep`/`graft_callers`/`graft_skeleton`/`graft_map`/
    `graft_blast`), gated push retrieval packs + first-turn orientation,
    post-edit blast radius (`graft:dirty`). Graft CLI resolved fail-closed
    (`cliPath` / host `packageRoot` / optional `@nanonets/graft` peer);
    tools hidden when the CLI is absent (mirrors `agy` handling).
- **Web + browser capability (Obscura-based, opt-in when the engine is
  present).** Everything Obscura can do, surfaced to the agent:
  `web_search` + `web_fetch` through the replaceable HTML search profile;
  native `obscura_fetch`/`obscura_scrape` (public-HTTP(S)-only validation,
  byte/count/timeout caps, `allowEval`-gated expressions, untrusted-content
  labeling); browser automation through CDP
  (`@arnilo/prism-web-tools/browser` surface:
  observe, `browser_evaluate` policy-gated, block/throttle/emulate); and
  Playwright-driven e2e testing via the CDP composition. Untrusted web
  content never reaches tool-authoritative state without the normal
  validation rules.
- No autonomy, workflows, sub-agents, or memory cadence policy — those are
  `st` concerns.
- Replaceable like `@clay/chat`; third-party packages extend/replace via
  declared extension points with user approval.

### Coding Agent UI (binding spec)

Applies to `@clay/coding-agent` and `@arnilo/st` alike — same surface,
`st` fills more of it over time. Heavily inspired by the pi coding agent
TUI; outside the specifics below, interaction mimics pi. Implementation
follows `clay-ui`: primitives-first, token-only styling, pane split tree,
and the mandatory seven-file design-skill stack listed in the plan's
`Approach -> Documentation Reviewed`.

**Layout.** Launching the agent splits the working area down the middle
vertically: two equal panes (50/50, user-resizable, ratio-clamped).

- **Left pane — agent activity.** Chronological transcript exactly like
  the pi TUI: user prompt, agent message, tool outputs (including MCP),
  skills loaded — in arrival order.
  - Tool output boxes show a fixed number of lines and truncate after
    that, so all transcript boxes keep a uniform height.
  - Box color distinguishes content type (user / agent / tool / skill /
    MCP); colors come from typed theme tokens only.
  - Selecting/clicking a truncated box renders its **full** content in the
    right pane — the right split is the detail surface for anything cut
    off in the transcript.
- **Right pane — three tabs at the top.**
  1. **Files** — the file view as it exists today; path browser loads
     workspace files on demand.
  2. **Observational Memory** — runtime OM activity (st agent): what the
     agent is observing, reflecting on, and dropping, live. Tab ships in
     Phase 2 chrome; live population lands with `st` memory cadence
     (Phase 7).
  3. **Context** — the agent's current context, categorized: system
     prompt, user prompts, agent messages, tool outputs, skills loaded,
     files loaded.
- **Input + status area (left pane bottom).**
  - Composer text box that grows vertically as the message lengthens.
    Supports `/` commands like pi: provider selection, model selection,
    configuration, compaction, and every registered command.
  - `Shift+Tab` cycles thinking/reasoning effort of the selected model
    (pi behavior; keybind configurable, this default).
  - Status row below the composer: **left** = loaded workspace path +
    git branch; **right** = selected provider + model + current context
    size vs context window.
  - Extension strip below that: active extensions (caveman, ponytail,
    …) and active MCP servers.

### Exit Gate

- Pi-parity checklist (documented in the plan) passes manually on Linux with
  at least one real provider and the mock provider in CI.
- Tree/discard drill: `/tree` branch from an earlier entry restores the
  workspace to that entry's document checkpoint; the abandoned branch keeps
  its summary; `/fork` and `/clone` produce independent sessions.
- UI spec conformance: 50/50 agent split with three right-pane tabs
  (Files, OM chrome, Context); uniform-height truncated transcript boxes
  with type-colored borders from tokens; box selection shows full content
  in the right pane; growing composer with `/` commands; `Shift+Tab`
  reasoning cycle; status row (workspace+branch | provider+model+context
  usage); extension/MCP strip. Visual + accessibility inspection per
  `clay-ui` Step 1.
- Fixture `build`/`fix` knowledge checks: with wiki enabled, `/wiki-init` +
  `/wiki-refresh` produce an OKF bundle that `wiki_search` answers from;
  with graft enabled, `graft_ask`/`graft_callers` return ranked spans;
  with the Obscura engine present, `web_search`/`web_fetch`/`obscura_fetch`
  answer, CDP automation drives a fixture page, and a Playwright e2e run
  passes through the CDP composition; all three disabled, no residue in the
  base agent.
- Session-search drill: matching entries from sessions and branches in the
  current workspace are returned with the correct tree location; an identical
  fixture session in another workspace never appears; selecting a result
  resumes it without implicitly attaching its transcript to the current run.
- Deleting/disabling the package leaves the daemon and chat fully functional.

### Phase 2.1 Defects, UX, and Prism 0.5.0 (`plans/109`)

Mid-execution: I1–I3 landed (workspace bind, per-workspace model auto-load,
`/model` + dropdown). Remaining work in plan 109, in this order:

- Adopt Prism 0.5.0 lockstep in `clay-agent` (pins, MCP SDK v2, sqlite 13,
  keyring 2, hyper/commandcode, thinking adapter). Log the pin set.
- I4 reasoning effort: yes, supported in Prism 0.5.0 via
  `applyThinkingLevelForModel` + declared `thinkingLevels` (0.4.0 blocked
  this). UI dropdown + configurable `Shift+Tab`.
- Remaining original UX: full transcript (I5), Files-as-editor-view (I6),
  Context drawer (I7), OM activity + worker models (I8), `/resume` (I9),
  Session Info as fourth right-pane tab (I10), R1–R5. Bind UI to plan 110
  (`ClayTabStrip`, recipe consumption). Phase 2's three-tab chrome is the
  108 ship; 109 adds Session Info and stops duplicating the workspace tree.
- Image support: still deferred (not in plan 109).

### Phase 2.2 Prism update

Absorbed into Phase 2.1 / plan 109 (Prism 0.5.0 lockstep). Do not schedule
a second Prism-update phase for 0.5.0.

### Phase 2.3 Implementation review, refactor

Unchanged: post-2.1 review after plan 109 closes.


## Phase 3: Package Installation, Update, and Clay Distribution

Fix the existing `clay package add` path (pnpm-only store, parsed-but-unused
`github:` specs, no self-update) and replace the user-facing verbs with the
pi model. Must land before any third-party package (`st`) is planned.

### Scope

- **Package CLI (pi verbs).** Top-level commands, not `clay package add`:
  - `clay install npm:@arnilo/st` / `clay install npm:@arnilo/st@1.2.3`
  - `clay remove npm:@arnilo/st`
  - `clay list`
  - `clay update` — update Clay itself from the channel that installed it
  - `clay update --extensions` — update installed packages only
  - `clay update --all` — Clay + packages
  - `clay update npm:@arnilo/st` — one package
- **One package source in v1.** No git clone, no local path, no tarball URL,
  no Clay-owned registry.
  - `npm:<name>` / `npm:@scope/name[@version]` — npm registry. Versioned
    specs are pinned and skipped by `update --extensions` (pi rule). Fetch
    still delegated to an npm-compatible manager (existing 2026-05-08
    decision); Clay does not become a registry.
- **Install ≠ execute, but install does write the load line.** `clay install`
  fetches, records provenance, and appends an idempotent
  `loadPackage("<name>")` to `~/.config/clay/init.js`. Enable, adopt, revoke,
  rollback stay Clay-owned (`clay package enable|disable|adopt|revoke|
  inspect|rollback`). Third-party JS still does not run until adopt; a load
  line without adopt is fail-closed. First-party `@clay/*` packages are
  unchanged (bundled, explicit load).
- **Clay self-distribution (v1).** npm package `@arnilo/clay` and curl installer only. Homebrew
  skipped for now (post-roadmap). `clay update` uses the npm channel or running the curl update command; no
  third updater. Unsigned payloads stay rejected (`src-tauri/src/release.rs`).
- Keep in-app package UI on the same service as the CLI. Do not invent a
  Clay registry or a second package manager.
- Bundle installation of used binaries. Such as: qmd, graft, obscura, ripgrep etc. Check if host has binaries installed. If not, install with permission. Include any other binaries needed.

### Exit Gate

- Linux: `clay install npm:<fixture>`  round-trip
  to store + `clay remove` + `clay update --extensions` against pinned vs
  floating specs; lifecycle scripts stay off unless `--allow-scripts`.
- `clay install` appends `loadPackage` once, never enables, adopts, or
  executes package JS.
- `clay update` (self) is a no-op or skip on unmanaged/dev checkouts; on an
  npm-managed `@arnilo/clay` install it upgrades Clay without touching
  packages unless `--all`.
- Publish dry-run docs exist for `@arnilo/clay`; no requirement to actually
  publish in this phase.

## Phase 4: Third-Party Agent Package Platform

### Scope

- Public `agent.*` Clay JS API surface for packages: register agent profiles
  (declarative `AgentDefinition` data), skills, commands, tool descriptors
  bound to server-executed implementations, and workflow definitions; list
  and start workflows; subscribe to run/workflow event streams; surface
  ask-user decisions and approvals into Clay UI.
- Daemon-side enforcement: package contributions are inert data until the
  daemon activates them under trust/permission policy; no package code runs
  in the daemon; secret containment unchanged.
- Authoring docs + conformance tests + a reference toy package (not `st`)
  proving the extension path end to end.

### Exit Gate

- A test-only third-party package registers an agent, a skill, a command,
  and a durable workflow, and runs it through the public API with user
  approval, on Linux.

## Phase 5: `st` Package — Orchestration Core

### Scope

- `/start` command and `st start` entry: asks the workflow type via
  `ask_user_decision` (durable suspend path after D1). Initial types: `build`
  and `fix`. Each is a registered loop implementation; more types are
  post-roadmap.
- **Shared session search annotations.** `st` writes workflow type, run,
  phase, task, checkpoint, and status labels into Clay's workspace-scoped
  session index. It uses the same `session.search` surface as the base coding
  agent — never a second session store.
- **Linux desktop capability (`st` extension only).**
  `@arnilo/prism-coding-tools/computer-use-linux` over a host-owned
  `computer-use-linux` MCP binary: accessibility-tree observation,
  screenshots, window targeting, input synthesis. DeviceAdapter admission
  stays deny-by-default, setup tools opt-in, mutating calls require
  approval/ExecutionPolicy (respecting the default acceptance policy:
  inside-workspace actions free; anything on the host outside the workspace
  follows the write-gate), results bounded and untrusted. Hidden when the
  binary is absent. Enables `st` workflows that drive real GUI apps for
  testing/verification.
- **Git for both loops.** Start of a run creates a dedicated branch. After a
  task passes test + validation, orchestrator auto-commits on that branch.
  No push unless the user asks. Failures do not commit.
- **Iteration checkpoints + post-hoc discard.** The orchestrator records a
  checkpoint (git commit on the run branch + document version checkpoint +
  session tree entry) before each loop iteration. A wrong implementation is
  discardable after the fact, from the UI or `/tree`: branch the session
  from the pre-iteration entry, `git reset` the run branch to the checkpoint
  commit, restore the matching document versions — one action, three layers,
  consistent. Experiments ride the same mechanism: go forward from any
  checkpoint in a scratch branch, keep or discard. Observational memory
  keeps a record of the dropped iteration (what was tried, why discarded)
  even though the code is gone.
- **Loop, not hope.** Skills are prompt bodies. The host orchestrator
  **auto-prompts** each required step (plan, decision gate, execute, test,
  validate, commit, compromises/further-actions). An agent that never loads a
  skill still hits the step because the loop injects it. Do not rely on
  markdown sitting in `.agents/skills/` being followed.
- **Skill shape (plan + execution).** Generic `SKILL.md` (principles +
  workflow) plus project `references/` where the skill has a generic base.
  Optional `scripts/` / `assets/`. Load `references/default.md` then
  `references/<git-root-basename>.md` only for skills that have a generic
  core (`create-plan`, `execute-plan`, test/validation siblings).
- **`create-plan` / `execute-plan`.** Planning: numbered `plans/` docs, every
  task independently checkable, four-way acceptance criteria, evidence-based
  Approach. Execution is a sibling skill: general loop + **routing into
  project execution references grouped by unit** (UI, frontend, API,
  documentation, database, …) and current docs before editing. This repo's
  combined create-plan is the principle source; `st` splits plan vs execute.
- **Project patterns are not a skill.** They are `execute-plan/references/`
  (and the matching execute-tests/validation refs). No generic base — always
  built from the project. This repo's `project-patterns` skill is the old
  shape; `st` folds that content into execution references.
- **`create-decision-log` is a skill and is embedded.** `create-plan` and
  `execute-plan` always run the decision gate (auto-prompted). Critical =
  architecture, security, or performance with real tradeoffs. Surface to the
  human. After approval: write the log **and** update the matching execution
  reference file(s). Non-critical choices do not log.
- **Ask the user.** Planning and execution may always `ask_user_decision`.
  Every such prompt includes: follow recorded laws/principles (decision logs
  + execution references) and do what the agent thinks best under those — no
  new policy.
- **Compromises Made / Further Actions are never dropped.** The loop fills
  them after execution and **surfaces them to the user**. User can configure:
  always take them up in a later plan by appending them to the workflow's
  roadmap document (the `build` artifact, or `fix` equivalent).
- Implementation plan is the contract. It does **not** own tests or
  validation procedures; those are sibling plans from child agents (Phase 6).
- `build` loop: capture goal → phased roadmap → per-phase `create-plan` →
  for each task: execute-plan → test child → validation child → commit if
  both pass. Decision gate whenever a critical tradeoff appears. End of plan:
  surface Compromises / Further Actions. Per-task memory observe/reflect +
  compaction.
- `fix` loop: ingest issues → one `create-plan` → walk issues one by one
  with the same per-task execute → test → validate → commit loop, same
  decision gate and end-of-plan surface. Done when every issue is resolved.
- Orchestrator runs host-side in the daemon with
  `@arnilo/prism-core/runtime/workflows@0.5.0`, using the retained bounded
  iterate-until-done host-loop pattern: one `runWorkflow` per
  iteration (task / implement / test / validate), iteration state in
  workflow inputs, explicit termination predicates + budgets with typed
  fail-closed `BudgetExhaustedError`, `replayWorkflow` per iteration run id
  for audit and resume; `examples/autonomous-coding-loop.ts` is the
  conformance reference. User-visible pause/cancel/steer; durable resume
  across daemon restarts (checkpoints + `definitionRevision`).

### Exit Gate

- On a fixture repository with the mock provider, `build` goes from goal to
  roadmap to per-phase plans to executed tasks with suspend/resume across a
  simulated daemon restart, all decisions auto-answered by test policy.
- Post-hoc discard drill: after a wrong iteration, discard from its
  pre-iteration checkpoint restores git run branch, document versions, and
  session position consistently; the loop continues from the checkpoint;
  OM records the dropped attempt.

## Phase 6: `st` Sub-Agent Validation Loop and Delegation

### Scope

- **Both `build` and `fix` use the same two children.** Test child and
  validation child. Each is an isolated session with its own model
  (`AgentDefinition.model`). Implementation agent never writes tests or
  validates (tunnel-vision split is structural: separate sessions, separate
  histories). Children never see the implementation agent's transcript.
- **Unbiased two-phase children.** After the implementation plan exists and
  **before** implementation:
  1. Test child is given **only** that plan (acceptance criteria, not impl
     chat or diffs) and writes a frozen test plan (`create-test-plan`).
  2. Validation child is given **only** that plan and writes a frozen
     validation plan (`create-validation-plan`).
  After a task is implemented, the same children write/run tests and
  validation **following those frozen plans** (`execute-tests`,
  `execute-validation`). They may read the workspace to execute, but they
  must not revise the frozen plans to match the implementation. Commit only
  if both pass.
- **Child findings become new main-plan tasks.** If test or validation
  wants a fix, it does not privately retry. It appends a new task on the
  **implementation plan**, typed `bug` | `enhancement` | `validation`. Main
  agent executes that task (`execute-plan`). Then **both** children re-run
  their **whole** frozen suites (not only the failing check). Frozen
  test/validation plans are not rewritten for the feedback task. Bounded
  retry on this append→implement→full-rerun cycle; then ask the user.
- **Validation always includes ponytail-review.** Not optional. Use
  `@arnilo/prism-coding-tools/ponytail`. Over-engineering review is part of
  every validation pass, alongside AC checks.
- **Skills `st` ships.** Loop auto-prompts each. Generic `SKILL.md` + project
  `references/` except project execution refs (no generic base):
  - `create-plan` / `execute-plan` — impl plan and impl execution. Execution
    refs = project patterns by unit (UI, frontend, API, docs, database, …).
  - `create-test-plan` / `execute-tests`
  - `create-validation-plan` / `execute-validation` (ponytail-review in both)
  - `create-decision-log` — also embedded in create-plan and execute-plan.
- Planning-time task typing in the roadmap schema (`type`, `model`,
  `delegate` fields) maps here only to a base-agent run or specific Prism
  model. External-agent execution is deliberately deferred to the direct
  runtime adapters in Phases 9–10; no Prism Antigravity adapter is added.
- Optional: `@arnilo/prism-core/governance/evals` wired as a release gate for
  `st` behavior regressions.

### Exit Gate

- Fixture `build` and `fix` complete with test + validation plans authored
  from the implementation plan only (store inspection: children never saw
  the implementation session); a child failure appends a typed task on the
  main plan, main agent executes it, then both children re-run the full
  frozen suites; ponytail-review ran on every validation pass; a critical
  tradeoff auto-prompted the decision gate, logged, and updated an execution
  reference; Compromises / Further Actions were surfaced (and optionally
  written onto the workflow roadmap); per-task commits exist only after both
  pass; follow-recorded-principles choice present; per-task model routing
  with at least two providers/models in one run; and an unavailable external
  runtime is reported without a silent Prism or model-provider fallback.

## Phase 7: `st` Memory Cadence and Autonomy Hardening

### Scope

- `@arnilo/prism-memory/compaction/observational-memory` attached to the `st`
  orchestrator session and each durable child session: per-task compaction
  (fast strategy), recall tool
  active, `om:status`/`om:view` surfaces in Clay UI, and the right-pane
  Observational Memory tab (Phase 2 UI spec) shows live observe/reflect/drop
  activity. Defaults: worker models configured separately from the session
  model; `compactAfterTokens` default **80,000** tokens (user-configurable);
  retention **per session**, not per workspace, to start.
- Memory-aware auto-prompts: each task prompt renders prior memory state plus
  plan excerpt; post-task flush before the next task.
- Guardrails: per-workflow budgets (tokens, wall time, tool calls), loop
  termination predicates, stuck detection with user escalation, full audit
  trail via run ledger + policy ledger.
- Failure drills: compaction loss, child failure, provider outage, daemon
  crash mid-task, restart resume.

### Exit Gate

- In a scripted long-build fixture, recall demonstrably recovers pre-crash
  decisions after restart, budgets terminate a runaway loop deterministically,
  and the escalation path reaches the Clay UI.

## Phase 8: Hardening, Documentation, and Distribution

### Scope

- `roadmap.md` (this file) finalized post-review; per-phase numbered plans
  in `plans/` (105+) created at phase start per `create-plan`; decision logs
  for remaining open decisions and any new ones.
- Package authoring guide for agent packages; `@arnilo/st` published on npm
  as the reference third-party agent package (install path is Phase 3);
  base agent shipped with Clay unchanged for users who never install `st`.
- Code-wiki updates, manual test modules, registry/doc truth tests per
  project documentation-as-code rules.
- Performance and security review: daemon event throughput, memory worker
  overhead, approval UX latency, redaction coverage across new event kinds.

### Exit Gate

- Linux blocking gates, daemon tests, package conformance, and manual test
  plan all pass; documentation is internally consistent; `st` install/
  remove cycles leave no residue in the base agent.

## Phase 9: External Runtime Contract and Claude Code Delegation

### Scope

- Introduce a direct, core-owned external-runtime boundary beside the native
  Prism host. Keep it deliberately small: lifecycle (`start`, `prompt`,
  `cancel`, `resume`, `respond`), normalized bounded events, and a declared
  capability set. Clay owns the delegation record, policy-bundle fingerprint,
  process lifecycle, redaction, and UI; the vendor owns its agent loop and
  conversation context. Do not model a foreign coding agent as a Prism
  provider, and do not require ACP.
- Compile one task-scoped Clay policy bundle into each runtime's documented
  controls: task instructions, translated skills, declared tool policy,
  Clay MCP server/configuration, workspace, and acceptance policy. Clay MCP
  tools are an independent tool plane, not agent lifecycle control. Preserve
  same-user-child authority disclosure; launch without a shell, with bounded
  I/O, cancellation, and cleanup.
- Add a Claude Code Agent SDK adapter. Use custom `systemPrompt`,
  `settingSources: []`, explicit built-in tool selection/disallow rules,
  explicit skills/agents/hooks, and `strictMcpConfig` with only Clay-declared
  MCP servers. Mediate `canUseTool` in Clay so an approval can allow, deny,
  or safely modify the requested input. Preserve vendor/organization safety
  policy as non-overridable.
- Extend session/event persistence and AG-UI projection with runtime identity,
  vendor session identity, capability state, and approval/question state.
  Raw vendor tool output stays out of FTS and logs by default; credentials
  remain in vendor-supported authentication stores and never cross an event
  or UI boundary.
- Use existing Command Centre picker patterns and capability-driven controls;
  never add controls that the runtime has not declared. Detailed UI work must
  follow the mandatory Clay UI skill stack and visual/accessibility review.

### Exit Gate

- A Claude Code fixture proves isolated configuration, Clay prompt/skill/tool
  projection, strict MCP selection, approval allow/deny/modified-input flow,
  cancellation, and resumed-session identity. Tests prove denied controls do
  not execute, unsupported controls do not render, secrets do not reach
  events/logs, and unavailable/auth-failed SDK state is fail-closed. A
  separately recorded authenticated manual run verifies the real vendor path.

## Phase 10: Antigravity Delegation and Truthful Control Boundary

### Scope

- Add a direct Antigravity adapter around the documented long-lived headless
  protocol: `agy --input-format stream-json --output-format stream-json`.
  Map `init`, `step_update`, and terminal `result` events; preserve the
  vendor `conversation_id`; support documented model/effort/agent selection,
  conversation resumption, SIGINT/process-group cancellation, and bounded
  shutdown. Hide this runtime when `agy` is absent or authentication fails.
- Generate and load Clay's task policy through Antigravity's documented custom
  agents, skills, plugins, MCP, sandbox, and permission configuration. Treat
  this as a vendor-native instruction overlay and policy constraint, not a
  replacement for Antigravity's base system behavior or a claim that
  built-in tools are removed.
- Represent the documented limitation explicitly: headless Antigravity uses
  permission policy, not Clay's live per-tool approval callback; its stream
  rejects `control_request` and `control_response`. Do not offer Clay
  approve/edit/reject controls, do not use `--dangerously-skip-permissions`,
  and do not claim ACP support. Offer a PTY terminal fallback for users who
  need the agent's native interactive controls.
- Keep runtime capability declarations, policy-bundle validation, event-size
  bounds, transcript redaction, process cleanup, and user-visible failure
  states shared with Phase 9; vendor-specific event data remains inert and
  cannot grant Clay filesystem, shell, network, or mutation authority.

### Exit Gate

- With an authenticated `agy` fixture, Clay projects text/tool/result events,
  resumes the exact conversation, and cancels a running task without a shell
  leak. Permission-policy denial blocks configured command/MCP cases; the UI
  never displays unsupported live-approval controls. Missing binary/auth,
  malformed NDJSON, oversized events, and interrupted sessions fail closed;
  a PTY fallback test proves native controls remain reachable without screen
  scraping as a structured adapter.

## Resolved this iteration (logged 2026-08-30)

1. **`st` identity and channels.** Product `st`. npm `@arnilo/st`. Clay npm
   `@arnilo/clay`. Homebrew skipped for now. Sources: npm + GitHub Releases.
   Pi verbs. `clay install` fetches, records, and appends idempotent
   `loadPackage("<name>")` to `init.js`. Adopt still required before
   third-party JS runs.
2. **Initial `st` workflow types: `build` + `fix`.** Host **loop auto-prompts**
   every required step (skills are bodies, not hope). Project patterns are
   `execute-plan/references/` grouped by unit — not a skill, no generic base.
   `create-decision-log` embedded in create-plan and execute-plan: critical
   arch/security/performance tradeoffs surface to the human; after approval,
   log + update execution refs. Compromises Made / Further Actions always
   surfaced; configurable to enqueue onto the workflow roadmap for a later
   plan. Rest unchanged: git branch, child isolation, frozen plans, typed
   feedback tasks, full-suite rerun, ponytail-review, ask-user with
   follow-recorded-principles.
3. **E2 — adopt the documented host-loop pattern.** Prism 0.3.2 shipped
   FEATURE-2 (bounded iterate-until-done docs: one `runWorkflow` per
   iteration, state in inputs, termination predicates, typed
   `BudgetExhaustedError`, `replayWorkflow` per iteration) + FEATURE-6
   (`examples/autonomous-coding-loop.ts` conformance reference). The
   in-graph `loop` node is explicitly deferred upstream (Plan 045). Clay
   implements the documented pattern; revisit the primitive only if Prism
   ships it and the host loop shows measured pain (iteration latency, state
   serialization overhead).
4. **Knowledge bases adopted as opt-in options.** `@arnilo/prism-memory/wiki`
   (llm-wiki: per-codebase `.wiki/` knowledge base, OKF v0.2, Merkle refresh,
   `wiki_*` tools) and `@arnilo/prism-memory/graft` (context-graph code search,
   pull tools + push retrieval packs + blast radius) load via `kernel.load`
   when enabled; both available to the clay coding agent and inherited by
   `st`. Graft CLI resolves fail-closed and hides when absent; `qmd`
   optional with catalog fallback.
5. **Default acceptance policy (clay + `st`).** Workspace folder: anything
   goes, no approvals. Host system outside workspace: reads free, writes
   gated on explicit user permission. Opt-in full-autonomy toggle lifts the
   outside-workspace write gate. Off by default.
6. **Observational memory defaults.** Worker models separately configurable;
   compaction threshold default 80,000 tokens (user-configurable); retention
   per session, not per workspace, to start.
7. **Web/browser via Obscura; desktop via computer-use-linux.** Web search,
   content fetch, CDP browser automation, and Playwright e2e all ride the
   Obscura engine (`@arnilo/prism-web-tools/obscura` +
   `@arnilo/prism-web-tools/browser` + optional `playwright-core`),
   host-installed binary, full advertised MCP surface,
   hidden when absent — for clay and `st` alike. `st` additionally ships the
   `computer-use-linux` extension capability (deny-by-default, approvals per
   the acceptance policy).
8. **Worktree/rollback = pi tree model extended to the workspace.** Session
   trees (single-file JSONL, `id`/`parentId`, `/tree` navigation, branch
   summaries, `/fork`/`/clone`) get a linked document version checkpoint at
   every task boundary; branching restores both layers; `st` iterations
   checkpoint git + documents + session entry so any wrong implementation is
   discardable post hoc.
9. **Workspace-scoped session search (clay + `st`).** One shared SQLite
   session/branch/checkpoint store, keyed by stable workspace identity, with
   Clay-owned FTS over user/agent messages, branch summaries, plan/task
   metadata, and OM reflections (no raw tool output by default). Current
   workspace only; search results open/resume a matching tree entry but never
   become agent context without explicit user action. `st` adds workflow/run/
   phase/task/checkpoint/status annotations, not another store.

## Resolved this iteration (Prism 0.4.0 draft)

- Replace the 22 clay-agent 0.3 pins with 0.4 family packages. Phase 0
  migrates the live daemon (`@arnilo/prism`, `@arnilo/prism-core`,
  `@arnilo/prism-providers` + `better-sqlite3`; drop unused
  `prism-model-router`). Phase 1 adds `prism-coding-tools`, `prism-web-tools`,
  `prism-memory`, `prism-mcp`. Do not adopt `prism-antigravity-agent`,
  `prism-office`, `prism-acp-agent`, or `prism-ag-ui` in the daemon. Phase 10
  owns direct `agy` integration. Import subpaths explicitly; no 0.3/0.4 mix;
  no compatibility shims; no persisted-schema migration.
  **Live pins:** superseded by 0.5.0 in Phase 2.1; 0.4 family/subpath rules
  stay.

## Resolved this iteration (Prism 0.5.0 / Phase 2.1)

- Lockstep exact `@arnilo/prism*@0.5.0` for the seven adopted families +
  `better-sqlite3@13.0.3`. MCP 2026-07-28 via prism-mcp / SDK v2 modules.
  Thinking effort is model-aware (`applyThinkingLevelForModel`). Explicit
  `/hyper` and `/commandcode` loads. No office/ACP/AG-UI; antigravity
  package removed upstream (Phase 10 still direct `agy`). No persisted
  schema migration. Plan 109 owns the cut. Pin decision log is a 109 task.

## Open Decisions (need `decision-logs/` before implementation)

1. ~~`st` package name, registry id, and distribution channel.~~
2. ~~Initial workflow-type set beyond `build`.~~
3. ~~Whether E2 lands as a documented pattern or a new Prism primitive.~~
   (Resolved: documented host-loop pattern, Prism 0.3.2.)
4. ~~Default acceptance: auto-approve scope for `st`.~~ (Resolved: workspace
   = full freedom; host reads free, host writes outside workspace gated;
   opt-in full-autonomy toggle. Applies to clay and `st` alike.)
5. ~~Observational memory defaults.~~ (Resolved: worker models configured
   separately from the session model; `compactAfterTokens` default 80,000,
   user-configurable; retention per session — not per workspace — to start.)
6. ~~Prism 0.4.0 family pin set in this draft. Log before Phase 0 is
   planned.~~ (Resolved: exact `@arnilo/prism@0.4.0`, `prism-core@0.4.0`,
   `prism-providers@0.4.0` + direct `better-sqlite3@12.11.1`; subpath imports
   only; `model-router` dropped; coding/web/memory/MCP deferred to Phase 1;
   Antigravity is a direct Phase 10 adapter, while office/ACP/AG-UI stay out
   of the daemon. `decision-logs/2026-09-02-0121-prism-0.4.0-clay-agent-family-pins.md`.)
7. **Prism 0.5.0 family pin set.** Directed for Phase 2.1 (`plans/109`).
   Log before the pin bump, same shape as 0121: exact `0.5.0` seven-family
   pins + `better-sqlite3@13.0.3`; MCP via `prism-mcp` public API only;
   hyper/commandcode explicit; no office/ACP/AG-UI. User directed the
   adoption; the log is a plan-109 task.

## Post-Roadmap (not in scope)

- Homebrew distribution of Clay (formula/cask).
- Additional `st` workflow types and meta-agent packages (Personal, Work,
  Research, Finance per the superseded roadmap's post-parity list).
- Cross-session shared memory scopes (E5) if per-session composition proves
  insufficient.
- Remote/distributed `st` runs over the Clay server protocol.
