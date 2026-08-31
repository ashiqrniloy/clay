# Clay Coding Agent and `st` Autonomous Agent Roadmap

Status: draft for user iteration. This roadmap supersedes the completed
Tauri + React migration roadmap as the repository's forward plan. The
migration roadmap is finished (all 21 tasks of `plans/097` checked) and
remains in git history; its governing decision
(`decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`) stays
authoritative for architecture rules that this roadmap inherits.

Governing decisions already made:

- `decision-logs/2026-08-21-1758-native-prism-host-no-acp-cli-parity.md`:
  Clay-owned Node `clay-agent` daemon wraps Prism 0.3.0; no ACP/AG-UI as the
  first-party agent bus; coding agent must reach CLI-parity inside Clay.
- `decision-logs/2026-08-21-2152-product-surfaces-are-replaceable-packages.md`:
  product surfaces are replaceable first-party packages; third-party packages
  extend/replace through declared extension points with user approval.
- Open decisions at the end need `decision-logs/` entries before their
  phase is planned. User-stated resolutions in this draft are not binding
  until logged.

## Product Shape

Three layers, strict separation:

1. **`clay-agent` daemon (Clay core, Node ≥ 20).** Hosts `@arnilo/prism`
   0.3.0 plus the first-party Prism packages. Owns providers, models,
   credentials (vault + keychain), SQLite persistence, run ledger, tools,
   compaction strategies, skills, commands, workflows, supervision, and
   delegation. Packages never spawn or speak to it directly; they use public
   `agent.*` Clay JS APIs served by the Rust server.
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

## Prism Capability Review (verified against 0.3.0)

Review method: full read of `@arnilo/prism@0.3.0` docs surface plus a
network-free smoke suite (`/tmp/prism-smoke/smoke.mjs`, 37 assertions, all
passing) exercising the exact seams this roadmap needs.

| Requirement | Prism primitive | Package | Verdict |
| --- | --- | --- | --- |
| Pi-parity coding tools | `shell`/`read`/`write`/`edit` are documented behavioral ports of pi tools; plus `repo_list`/`repo_search`/`glob`/`delete`/`move`, opt-in `createGitTools`, `coding_check`; host `operations` seams for Clay document authority | `@arnilo/prism-coding-agent` | ✅ smoke-verified |
| Approvals/sandboxing | `createCodingApprovalPolicy`, `createSandboxCodingComposition`, Docker/native backends, `ExecutionPolicy` | `@arnilo/prism-coding-security` | ✅ smoke-verified |
| Sessions, branching, persistence | `AgentSession` run/stream/steer/compact/abort/checkout/fork/clone; branching handles; JSONL/SQLite/Postgres stores | `@arnilo/prism`, `@arnilo/prism-session-store-sqlite` | ✅ (SQLite already wired in daemon) |
| Slash commands | `CommandDefinition` contributions via extension kernel; RPC `command` seam; ACP `available_commands_update` parity; host-opt-in `CommandExecutionContext.drivers` (`startRun`/`startWorkflow`/`steer`) shipped 0.3.2 | `@arnilo/prism` | ✅ drivers available (E3 resolved) |
| Workflow-type picker ("ask me") | `createAskUserDecisionTool` (blocking) and `suspendAskUserDecision` + `createAskUserDecisionResumeValidator` (durable) | `@arnilo/prism-coding-agent` | ✅ smoke-verified; D1 fixed in 0.3.2 (`allowCustom` defaults `false` in suspend data) |
| Loop-until-goal orchestration | Bounded DAG workflows (`defineWorkflow`/`runWorkflow`/`resumeWorkflow`/`replayWorkflow`), durable suspend/resume, sagas, `runCodingGoalVerify`; documented bounded iterate-until-done **host-loop pattern** (one `runWorkflow` per iteration, iteration state in inputs, explicit termination predicate + budgets, typed fail-closed `BudgetExhaustedError`, `replayWorkflow` per iteration run id) | `@arnilo/prism-workflows` | ✅ documented + `examples/autonomous-coding-loop.ts` conformance reference (0.3.2, FEATURE-2/6) |
| Custom per-run loops | `AgentLoopStrategy` escape hatch with `LoopContext`; durable snapshot/restore; `generateValidateReviseLoop` for validated artifacts | `@arnilo/prism` | ✅ smoke-verified |
| Plan generation | `writeCodingPlanFile`/`parseCodingPlanTodos`, bounded `state.coding` checkpoint metadata; custom `create-plan` skill via skill registry + progressive disclosure | `@arnilo/prism-coding-agent`, `@arnilo/prism` | ✅ |
| Per-codebase LLM wiki (llm-wiki) | `createWikiExtension`: `/wiki-init`/`/wiki-refresh` (SHA-256 Merkle diff)/`/wiki-lint`; `wiki_search`/`wiki_read_page`/`wiki_record_insight`; OKF v0.2 bundles; `wiki-maintainer`/`wiki-searcher` skills; optional `qmd` hybrid search with catalog fallback | `@arnilo/prism-wiki` 0.0.3 | ✅ adopt as opt-in knowledge base (Phase 2) |
| Context-graph code search | `createGraftExtension`: pull tools `graft_ask`/`graft_grep`/`graft_callers`/`graft_skeleton`/`graft_map`/`graft_blast`; gated push retrieval packs + first-turn orientation; post-edit blast radius (`graft:dirty`); fail-closed CLI resolution (`cliPath`/`packageRoot`/optional `@nanonets/graft` peer) | `@arnilo/prism-graft` 0.0.1 | ✅ adopt as opt-in search layer (Phase 2) |
| Sub-agent fan-out (criteria/tests/validation) | `createSupervisor` allow-listed children, per-child models, narrowed permissions, budgets, durable nested approvals | `@arnilo/prism-supervisor` | ✅ smoke-verified |
| Per-task model selection | `AgentDefinition.model`, `RunOptions.model`, use-case bindings (`resolveUseCaseModel`), governance via `@arnilo/prism-model-router` | `@arnilo/prism` + router | ✅ |
| External agent delegation (Antigravity) | `createAntigravityCliAgent` + `createAntigravityDelegationTool`; per-run ephemeral MCP server, conversation resume, event projection | `@arnilo/prism-antigravity-agent` | ✅ documented (host owns `agy` auth; not smoke-tested) |
| Observational memory + auto-compaction | `createObservationalMemory().attach()`: post-run observe/reflect/drop workers with independent models, `compactAfterTokens`, fast model-free compaction strategy | `@arnilo/prism-compaction-observational-memory` | ✅ smoke-verified |
| Exact-id recall | `createRecallMemoryTool` (`{id}` exact recall + cursor paging), `om:status`/`om:view` command factories | same | ✅ smoke-verified (fail-closed behavior confirmed) |
| Declarative third-party agents | `AgentDefinition` + `resolveAgentDefinition`/`resolveAgentBundle`, extension kernel `registerAgent`/`registerSkill`/`registerCommand` | `@arnilo/prism` | ✅ smoke-verified; E1 fixed in 0.3.2 (`overrides.model` fallback) |
| Durable human gates mid-run | `interruptBeforeTool`, `AgentRunLifecycle`/`resumeAgentRun`, pending-decision CAS | `@arnilo/prism` | ✅ documented |
| MCP tools in agent runs | `@arnilo/prism-mcp` client bridge (bounded, OAuth) | `@arnilo/prism-mcp` | ✅ documented |
| Web search + content fetch | `web_search`/`web_fetch` via `createObscuraWebTools` (replaceable HTML search profile) + native `obscura_fetch`/`obscura_scrape`; public-HTTP(S)-only, byte/count/timeout caps, untrusted-content labeling; `@arnilo/prism-web-tools` base for direct WebProvider backends | `@arnilo/prism-obscura` 0.3.0, `@arnilo/prism-web-tools` | ✅ adopt Obscura engine (Phase 1–2) |
| Browser automation + e2e | Obscura engine over a host-installed binary: fail-closed `spawnObscuraProcess`, full advertised MCP surface (`obscura_*` tools), `connectObscuraCdp` managed/external CDP + Playwright `connectOverCDP`; `@arnilo/prism-browser` CDP surface (`browser_evaluate` gated, observe/block/throttle/emulate) and Playwright runs for e2e tests | `@arnilo/prism-obscura`, `@arnilo/prism-browser`, optional `playwright-core` peer | ✅ adopt (Phase 1–2) |
| Linux desktop use | `@arnilo/prism-computer-use-linux` wraps a host-owned `computer-use-linux` MCP binary; DeviceAdapter deny-by-default admission, opt-in setup tools, mutating calls require approval/ExecutionPolicy, bounded untrusted results | `@arnilo/prism-computer-use-linux` | ✅ adopt as `st` extension capability (Phase 5) |
| Validation/eval gating | `@arnilo/prism-evals` scorers/datasets/experiments/CI thresholds | `@arnilo/prism-evals` | ✅ documented |

## Prism Readiness: Defects and Enhancements Required Before Phase 1

**Status: Prism 0.3.2 (plan 050, 2026-08-29) landed the Clay integration
intake** — D1 fixed, E1/E2/E3/E6 resolved, DOCS-1 integrator contracts
documented in `docs/workflows.md` / `docs/supervisors.md` /
`docs/compaction-and-retry.md`. Entries below are the historical record;
only E4/E5 remain open (optional, workarounds hold).

### Defects (required)

- **D1 — `suspendAskUserDecision` drops `allowCustom` when omitted. —
  [FIXED in 0.3.2]** `toAskUserDecisionSuspendData` now normalizes via
  `parseAllowCustom` (omitted → `false`; non-boolean fails closed at accept
  time, never at resume time). Verified in the published
  `@arnilo/prism-coding-agent@0.3.2` dist. Clay's smoke suite still carries
  the resume round-trip regression without explicit `allowCustom`.

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
- Antigravity delegation requires the host-owned authenticated `agy` binary;
  Prism never manages Google credentials. Clay treats it as an optional
  delegation target that is hidden when `agy` is absent.

## Prism Adoption Map (tools, commands, skills by phase)

Complete inventory of the 59-package Prism graph against Clay's phases.
"Tools" = agent ToolDefinitions; "Commands" = registered `/verbs`;
"Skills" = skill-registry texts loaded via `load_skill`.

| Phase | Prism package(s) | Adopt | Notes |
| --- | --- | --- | --- |
| 1 | `prism` (core) | `AgentSession` run/stream/steer/compact/abort/checkout/fork/clone; skills registry + `load_skill`; `CommandDefinition` dispatch + host-injected `CommandDrivers`; `AgentDefinition`/`registerAgent`; durable runs (`interruptBeforeTool`, `AgentRunLifecycle` resume); `AgentLoopStrategy`, `generateValidateReviseLoop` | Document-authority `operations` seams for read/write/edit |
| 1 | `prism-coding-agent` | `shell`/`read` (incl. `findText`)/`write`/`edit`/`repo_list`/`repo_search`/`glob`/`delete`/`move`; opt-in `createGitTools`; `coding_check` | Backend registration; agent-facing surface lands Phase 2 |
| 1 | `prism-coding-security` | `createCodingApprovalPolicy`, `createSandboxCodingComposition` (Docker + native netns backends), `ExecutionPolicy` | Default acceptance policy enforced here |
| 1 | `prism-compaction` + `compaction-llm` | Default + coding LLM compaction strategies; manual + threshold triggers | |
| 1 | `prism-compaction-observational-memory` | `createObservationalMemory().attach()`; `recall` tool; `om:status`/`om:view` commands | Attach only; cadence + UI in Phase 7 |
| 1 | `prism-session-store-sqlite` | Session/branch/checkpoint persistence; Clay-owned workspace metadata + SQLite FTS index | Shared by clay + `st`; no second `st` store |
| 1 | `prism-mcp` | MCP client bridge for allow-listed package-declared servers | |
| 1 | `prism-providers` + `provider-*` (16 adapters) | Provider adapters behind pi-parity model/provider switching | Host selection, never ambient |
| 1 | `prism-obscura` + `web-tools` + `browser` (+ optional `playwright-core`) | Obscura binary lifecycle, CDP, Playwright composition (engine plumbing) | Agent surface in Phase 2; hidden when binary absent |
| 1 | `prism-document-reader` (optional) | `read` tool `documentReader` slot (PDF/Office) | Adopt if users need spec-file reading |
| 1 | `prism-work-tools`, `prism`, `tool-validator-json-schema`, `session-store-codecs`, `server` | Internal seams used transitively by the daemon (process env isolation, tool schema validation, codec helpers, RPC handler) | No direct Clay-facing surface |
| 2 | `prism-coding-agent` | `ask_user_decision` tool + `suspendAskUserDecision` durable path; `writeCodingPlanFile`/`parseCodingPlanTodos`; `state.coding` checkpoints | `/start` picker reuse in Phase 5 |
| 2 | (Clay-authored via `prism` skill registry) | **Skill:** `create-plan` | Generic principles; project refs split per Phase 5 decision |
| 2 | `prism-wiki` | **Tools:** `wiki_search`, `wiki_read_page`, `wiki_record_insight`. **Commands:** `/wiki-init`, `/wiki-refresh`, `/wiki-lint`. **Skills:** `wiki-maintainer`, `wiki-searcher` | OKF v0.2; opt-in; `qmd` optional |
| 2 | `prism-graft` | **Tools:** `graft_ask`, `graft_grep`, `graft_callers`, `graft_skeleton`, `graft_map`, `graft_blast`; push retrieval packs; `graft:dirty` post-edit blast radius | Opt-in; CLI fail-closed |
| 2 | `prism-obscura` + `web-tools` + `browser` | **Tools:** `web_search`, `web_fetch`, `obscura_fetch`, `obscura_scrape`, full `obscura_*` MCP surface, CDP tools (`browser_observe`, gated `browser_evaluate`, `block_urls`/`unblock_urls`/`throttle`/`emulate`); Playwright e2e runs | Everything Obscura can do |
| 2 | `prism-caveman`, `prism-impeccable` (optional) | **Skills:** caveman output style, impeccable UI design | Ship as optional toggles, off by default |
| 5 | `prism-workflows` | `defineWorkflow`/`runWorkflow`/`resumeWorkflow`/`replayWorkflow`, sagas, `runCodingGoalVerify`; documented bounded host-loop pattern + `BudgetExhaustedError` | One `runWorkflow` per iteration |
| 5 | (Clay-authored) | **Skills:** `create-plan`/`execute-plan` split, generic + `references/` | Project patterns live in execute refs |
| 5 | `prism-computer-use-linux` | Desktop MCP tools: accessibility tree, screenshots, input synthesis; deny-by-default admission, approvals per acceptance policy | `st` extension only |
| 6 | `prism-supervisor` | `createSupervisor` isolated children, per-child models, narrowed permissions, durable nested approvals | Test/validation children |
| 6 | `prism-model-router` | `resolveUseCaseModel` + budgets for per-task routing | Governance optional |
| 6 | `prism-antigravity-agent` | `createAntigravityCliAgent` + `createAntigravityDelegationTool` | Host owns `agy`; hidden when absent |
| 6 | `prism-ponytail` | **Skill:** ponytail + `ponytail-review` | Mandatory in every validation pass |
| 6 | `prism-evals` (optional) | Scorers/datasets as `st` behavior regression gate | Phase 8 if adopted |
| 7 | `prism-compaction-observational-memory` | Per-task compaction cadence, worker models separate, `compactAfterTokens` 80k default, per-session retention, recall surfaces + OM tab | |
| 8 | `prism-evals` (optional) | Release gate thresholds | If adopted in Phase 6 |

**Not adopted (explicit):** `acp-agent` + `ag-ui` (ACP/AG-UI rejected as
first-party bus per the 2026-08-21 decision; revisit only as external-agent
adapters post-roadmap), `rag` + `memory` (knowledge covered by wiki + graft;
pgvector stack is server-side scale we don't need), `session-store-postgres`
+ `session-store-nats` + `enterprise-postgres` (SQLite is sufficient;
enterprise stores post-roadmap), `policy` standalone (ExecutionPolicy via
coding-security covers Clay), `obscura`-adjacent `observability-opentelemetry`
(and `credentials-node` beyond what providers need), `openapi-tools`,
`prism-all`/`prism-base`/`prism-sdk`/`prism-code` umbrella profiles (Clay
pins exact packages), `compaction-*` beyond the two strategies named above.



Change requests were filed in this repo at
`docs/clay-integration-findings.md` (BUG-1/BUG-2, FEATURE-1..6, DOCS-1
mapping one-to-one to D1/E1–E6 plus docs constraints); **Prism 0.3.2
(plan 050) landed the intake** — D1, FEATURE-1 (E1), FEATURE-2/6 (E2),
FEATURE-3 (E3), DOCS-1. This phase is now verification, not upstream
fixes.

## Phase 0: Prism 0.3.2 Adoption and Verification

### Scope

- Bump `clay-agent` dependency pins to the 0.3.2 set (`@arnilo/prism`,
  `prism-coding-agent`, `prism-workflows@0.3.1`, `prism-supervisor`,
  `prism-coding-security`, `prism-compaction-observational-memory`;
  add `prism-wiki@0.0.3`, `prism-graft@0.0.1`, `prism-antigravity-agent`,
  `prism-obscura`, `prism-web-tools`, `prism-browser`, optional
  `playwright-core`, and `prism-computer-use-linux`) as one atomic set.
- Verify against 0.3.2: D1 resume path without explicit `allowCustom`;
  `overrides.model` declarative resolution; `CommandExecutionContext.drivers`
  injection; the documented bounded host-loop pattern +
  `examples/autonomous-coding-loop.ts` as Clay's conformance reference.
- Remaining optional Prism items: E4 (child event passthrough) and E5
  (cross-session OM scope) — workarounds hold; pick up by need.

### Exit Gate

- The Clay smoke suite (moved from `/tmp` into `clay-agent/src/__tests__`)
  passes against the published packages, including the D1 resume path with
  omitted `allowCustom`.

## Phase 1: Base Coding Agent Host Uplift (`clay-agent`)

### Scope

- Register `@arnilo/prism-coding-agent` tools with Clay-operation backends:
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
- Add compaction strategies (default + coding LLM compaction +
  observational-memory fast compaction) and daemon RPC to select/per-run
  override.
- Add skills registry plumbing and package-contributed skill text
  (progressive disclosure, `load_skill`).
- Add command dispatch (`CommandDefinition` → daemon RPC) so packages can
  register `/verb` commands; inject host-owned `CommandDrivers`
  (`startRun`/`startWorkflow`/`steer`, shipped in Prism 0.3.2) so contributed
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
- Add MCP server wiring from package-declared MCP manifests (allow-listed).
- **Obscura web/browser engine (host-owned binary).** Daemon lifecycle for
  the host-installed Obscura binary (fail-closed spawn, readiness, group
  close — mirrors `agy` handling); expose its complete advertised MCP
  surface, managed/external CDP (`connectObscuraCdp`), and Playwright
  `connectOverCDP` composition; optional `playwright-core` peer for browser
  automation and e2e test runs. All capabilities hidden when the binary is
  absent.
- Keep mock-mode and existing chat flows byte-compatible.

### Exit Gate

- Linux gates pass: `cargo fmt --check`, `cargo check --all-targets`,
  `cargo clippy --all-targets -- -D warnings`, server/daemon tests.
- A session can run the nine coding tools against an open Clay document with
  dirty-buffer fidelity, approval prompts, and persisted history.
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
  - `@arnilo/prism-wiki` (llm-wiki): per-codebase `.wiki/` knowledge base —
    `/wiki-init`, `/wiki-refresh` (SHA-256 Merkle-diff incremental),
    `/wiki-lint`; `wiki_search`/`wiki_read_page`/`wiki_record_insight` tools;
    OKF v0.2 bundles; `wiki-maintainer`/`wiki-searcher` skills; profiles
    codebase/pkm/hybrid/auto; optional `qmd` CLI for hybrid search with
    catalog fallback when absent.
  - `@arnilo/prism-graft`: context-graph code search — pull tools
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
  labeling); browser automation through CDP (`prism-browser` surface:
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

## Phase 3: Package Installation, Update, and Clay Distribution

Fix the existing `clay package add` path (pnpm-only store, parsed-but-unused
`github:` specs, no self-update) and replace the user-facing verbs with the
pi model. Must land before any third-party package (`st`) is planned.

### Scope

- **Package CLI (pi verbs).** Top-level commands, not `clay package add`:
  - `clay install npm:@arnilo/st` / `clay install npm:@arnilo/st@1.2.3`
  - `clay install github:arnilo/st` / `clay install github:arnilo/st@v1.2.3`
  - `clay remove npm:@arnilo/st`
  - `clay list`
  - `clay update` — update Clay itself from the channel that installed it
  - `clay update --extensions` — update installed packages only
  - `clay update --all` — Clay + packages
  - `clay update npm:@arnilo/st` — one package
- **Two package sources in v1.** No git clone, no local path, no tarball URL,
  no Clay-owned registry.
  - `npm:<name>` / `npm:@scope/name[@version]` — npm registry. Versioned
    specs are pinned and skipped by `update --extensions` (pi rule). Fetch
    still delegated to an npm-compatible manager (existing 2026-05-08
    decision); Clay does not become a registry.
  - `github:owner/repo[@tag]` — GitHub **Releases** source tarball or a
    single `*.tgz` release asset, not `git clone`. Untagged → latest
    release. Tag is pinned like npm versions.
- **Install ≠ execute, but install does write the load line.** `clay install`
  fetches, records provenance, and appends an idempotent
  `loadPackage("<name>")` to `~/.config/clay/init.js`. Enable, adopt, revoke,
  rollback stay Clay-owned (`clay package enable|disable|adopt|revoke|
  inspect|rollback`). Third-party JS still does not run until adopt; a load
  line without adopt is fail-closed. First-party `@clay/*` packages are
  unchanged (bundled, explicit load).
- **Clay self-distribution (v1).** npm package `@arnilo/clay` only. Homebrew
  skipped for now (post-roadmap). `clay update` uses the npm channel; no
  third updater. Unsigned payloads stay rejected (`src-tauri/src/release.rs`).
- Keep in-app package UI on the same service as the CLI. Do not invent a
  Clay registry or a second package manager.

### Exit Gate

- Linux: `clay install npm:<fixture>` / `github:<fixture-release>` round-trip
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
  `@arnilo/prism-computer-use-linux` over a host-owned
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
- Orchestrator runs host-side in the daemon per the documented Prism 0.3.2
  bounded iterate-until-done host-loop pattern: one `runWorkflow` per
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
- **Validation always includes ponytail-review.** Not optional. Use Prism's
  already-shipped ponytail adaptation. Over-engineering review is part of
  every validation pass, alongside AC checks.
- **Skills `st` ships.** Loop auto-prompts each. Generic `SKILL.md` + project
  `references/` except project execution refs (no generic base):
  - `create-plan` / `execute-plan` — impl plan and impl execution. Execution
    refs = project patterns by unit (UI, frontend, API, docs, database, …).
  - `create-test-plan` / `execute-tests`
  - `create-validation-plan` / `execute-validation` (ponytail-review in both)
  - `create-decision-log` — also embedded in create-plan and execute-plan.
- Planning-time task typing in the roadmap schema (`type`, `model`,
  `delegate` fields) mapped at execution to: base agent run, specific Prism
  model, or Antigravity delegation via
  `createAntigravityDelegationTool`-equivalent wiring behind a feature check
  for `agy` availability.
- Optional: Prism evals wired as a release gate for `st` behavior
  regressions.

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
  with at least two providers/models in one run; Antigravity path proven or
  proven-absent with a clean skip.

## Phase 7: `st` Memory Cadence and Autonomy Hardening

### Scope

- Observational memory attached to the `st` orchestrator session and each
  durable child session: per-task compaction (fast strategy), recall tool
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
4. **Knowledge bases adopted as opt-in options.** `@arnilo/prism-wiki`
   (llm-wiki: per-codebase `.wiki/` knowledge base, OKF v0.2, Merkle refresh,
   `wiki_*` tools) and `@arnilo/prism-graft` (context-graph code search,
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
   Obscura engine (`@arnilo/prism-obscura` + `prism-browser` + optional
   `playwright-core`), host-installed binary, full advertised MCP surface,
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

## Post-Roadmap (not in scope)

- Homebrew distribution of Clay (formula/cask).
- Additional `st` workflow types and meta-agent packages (Personal, Work,
  Research, Finance per the superseded roadmap's post-parity list).
- Cross-session shared memory scopes (E5) if per-session composition proves
  insufficient.
- Remote/distributed `st` runs over the Clay server protocol.
