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
- This roadmap adds no new binding decisions. Open decisions are listed at
  the end and need `decision-logs/` entries before implementation.

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
3. **`st` (third-party Clay package, author's own workflow).** A
   comprehensive autonomous coding agent composed entirely from the base
   layer's primitives plus Prism packages. Not shipped or activated by
   default; nothing in the base layer depends on it.

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
| Slash commands | `CommandDefinition` contributions via extension kernel; RPC `command` seam; ACP `available_commands_update` parity exists | `@arnilo/prism` | ✅ with host dispatch (see E3) |
| Workflow-type picker ("ask me") | `createAskUserDecisionTool` (blocking) and `suspendAskUserDecision` + `createAskUserDecisionResumeValidator` (durable) | `@arnilo/prism-coding-agent` | ✅ smoke-verified, blocked by defect D1 |
| Loop-until-goal orchestration | Bounded DAG workflows (`defineWorkflow`/`runWorkflow`/`resumeWorkflow`/`replayWorkflow`), durable suspend/resume, sagas, `runCodingGoalVerify` | `@arnilo/prism-workflows` | ✅ smoke-verified with host iteration (see E2) |
| Custom per-run loops | `AgentLoopStrategy` escape hatch with `LoopContext`; durable snapshot/restore; `generateValidateReviseLoop` for validated artifacts | `@arnilo/prism` | ✅ smoke-verified |
| Plan generation | `writeCodingPlanFile`/`parseCodingPlanTodos`, bounded `state.coding` checkpoint metadata; custom `create-plan` skill via skill registry + progressive disclosure | `@arnilo/prism-coding-agent`, `@arnilo/prism` | ✅ |
| Sub-agent fan-out (criteria/tests/validation) | `createSupervisor` allow-listed children, per-child models, narrowed permissions, budgets, durable nested approvals | `@arnilo/prism-supervisor` | ✅ smoke-verified |
| Per-task model selection | `AgentDefinition.model`, `RunOptions.model`, use-case bindings (`resolveUseCaseModel`), governance via `@arnilo/prism-model-router` | `@arnilo/prism` + router | ✅ |
| External agent delegation (Antigravity) | `createAntigravityCliAgent` + `createAntigravityDelegationTool`; per-run ephemeral MCP server, conversation resume, event projection | `@arnilo/prism-antigravity-agent` | ✅ documented (host owns `agy` auth; not smoke-tested) |
| Observational memory + auto-compaction | `createObservationalMemory().attach()`: post-run observe/reflect/drop workers with independent models, `compactAfterTokens`, fast model-free compaction strategy | `@arnilo/prism-compaction-observational-memory` | ✅ smoke-verified |
| Exact-id recall | `createRecallMemoryTool` (`{id}` exact recall + cursor paging), `om:status`/`om:view` command factories | same | ✅ smoke-verified (fail-closed behavior confirmed) |
| Declarative third-party agents | `AgentDefinition` + `resolveAgentDefinition`/`resolveAgentBundle`, extension kernel `registerAgent`/`registerSkill`/`registerCommand` | `@arnilo/prism` | ✅ smoke-verified (see E1 friction) |
| Durable human gates mid-run | `interruptBeforeTool`, `AgentRunLifecycle`/`resumeAgentRun`, pending-decision CAS | `@arnilo/prism` | ✅ documented |
| MCP tools in agent runs | `@arnilo/prism-mcp` client bridge (bounded, OAuth) | `@arnilo/prism-mcp` | ✅ documented |
| Validation/eval gating | `@arnilo/prism-evals` scorers/datasets/experiments/CI thresholds | `@arnilo/prism-evals` | ✅ documented |

## Prism Readiness: Defects and Enhancements Required Before Phase 1

Land these in the Prism repo first; ship as a 0.3.1 patch (defects) plus
optional 0.4.0 items (enhancements). "Required" = blocks a roadmap phase;
"optional" = has an acceptable host workaround that is already reflected in
the phase designs below.

### Defects (required)

- **D1 — `suspendAskUserDecision` drops `allowCustom` when omitted.**
  `toAskUserDecisionSuspendData` copies `request.allowCustom` without
  defaulting. After the durable JSON round-trip the key vanishes, and the
  package's own `isAskUserDecisionSuspendData` then fails (`typeof
  allowCustom === "boolean"`), so `createAskUserDecisionResumeValidator`
  throws `suspension.data missing ask_user_decision request` on every resume.
  Reproduced in the smoke suite; the tool path normalizes correctly, only the
  workflow suspend path is broken. **Fix:** default `allowCustom: false` in
  `toAskUserDecisionSuspendData` (matching the tool path) and add a resume
  round-trip test without explicit `allowCustom`. This defect blocks `st`'s
  durable `/start` flow.

### Enhancements (required unless waived per phase)

- **E1 — `resolveAgentDefinition` requires `model` even when
  `context.overrides.model` is supplied.** Resolution throws `Agent "<name>"
  has no model` before overrides apply, contradicting the doc table where
  `model` is optional. Either allow `overrides.model` (or a context-level
  default model) to satisfy resolution, or document model-or-`create()` as
  mandatory. Reproduced in smoke suite. Clay can work around this by
  injecting the user's selected model into the definition before resolution;
  enhancement preferred so third-party agent packages stay declarative.
- **E2 — Documented host pattern (or primitive) for bounded
  iterate-until-done orchestration.** Workflows are acyclic and
  revision-fingerprinted by design. `st`'s "loop until the goal is achieved"
  therefore needs either a Prism helper (e.g. a bounded `iterateUntil` saga
  variant with explicit state, budgets, and termination predicates) or a
  documented host-loop pattern: one workflow run per phase, host re-enters
  with updated state until exit criteria pass. Cheapest acceptable outcome is
  the documented pattern plus a runnable example; the daemon then implements
  exactly that pattern.

### Enhancements (optional; workarounds already in phase designs)

- **E3 — Commands that drive.** `CommandExecutionContext` carries only
  ids/signal/metadata, so a contributed command cannot start a run, steer, or
  launch a workflow. Clay's daemon will map slash commands to host drivers
  (`/start` → `st` workflow start) outside Prism. Optional Prism enhancement:
  optional host-supplied driver hooks on command context.
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

## Phase 0: Prism 0.3.1/0.4.0 Readiness

Change requests are filed in the Prism repo at
`docs/clay-integration-findings.md` (BUG-1/BUG-2, FEATURE-1..6, DOCS-1
mapping one-to-one to D1/E1–E6 plus docs constraints).

### Scope

- Fix D1 with a regression test covering durable suspend/resume without an
  explicit `allowCustom`.
- Resolve E1 (preferred: overrides/context fallback model for declarative
  definitions; minimum: doc correction).
- Resolve E2 (preferred: documented host-loop pattern + example; no new
  runtime machinery unless the pattern proves awkward).
- Optionally land E3–E6 by need during later phases.
- Publish and bump `clay-agent` dependency pins as one atomic set.

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
- Wire `ExecutionPolicy` approvals (`@arnilo/prism-coding-security`) through
  the server to Clay confirmation UI; identity-scoped caching of decisions.
- Add compaction strategies (default + coding LLM compaction +
  observational-memory fast compaction) and daemon RPC to select/per-run
  override.
- Add skills registry plumbing and package-contributed skill text
  (progressive disclosure, `load_skill`).
- Add command dispatch (`CommandDefinition` → daemon RPC) so packages can
  register `/verb` commands; E3 workaround: commands resolve host drivers.
- Add durable run support (`interruptBeforeTool`, lifecycle resume) and run
  state RPC for approval surfaces.
- Add MCP server wiring from package-declared MCP manifests (allow-listed).
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
  file conventions (`plans/`), and the minimal UI surface (transcript +
  composer + approval cards) composed from existing SDUI primitives and the
  chat extension points.
- Pi-parity behaviors: streaming, steering/cancel, session list/resume,
  branch fork/checkout, manual + auto compaction, provider/model switching.
- No autonomy, workflows, sub-agents, or memory cadence policy — those are
  `st` concerns.
- Replaceable like `@clay/chat`; third-party packages extend/replace via
  declared extension points with user approval.

### Exit Gate

- Pi-parity checklist (documented in the plan) passes manually on Linux with
  at least one real provider and the mock provider in CI.
- Deleting/disabling the package leaves the daemon and chat fully functional.

## Phase 3: Third-Party Agent Package Platform

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

## Phase 4: `st` Package — Orchestration Core

### Scope

- `/start` command and `st start` entry: asks the workflow type via
  `ask_user_decision` (durable suspend path after D1); types are a registered
  set of loop implementations (initial set approved in a decision log;
  `build` is the reference implementation).
- `build` loop: capture goal (prompt or attached file via document/resource
  APIs) → generate phased roadmap (validated artifact via
  `generateValidateReviseLoop` or structured output) → persist roadmap as
  workspace Markdown + bounded `state.coding` metadata.
- Per phase: auto-prompt plan creation using the package's customized
  `create-plan` skill, then drive tasks one run at a time; per-task boundary
  triggers observational-memory observe/reflect and explicit compaction.
- Orchestrator runs host-side in the daemon per phase workflows (E2 pattern)
  with user-visible pause/cancel/steer, budgets, and durable resume across
  daemon restarts (checkpoints + `definitionRevision`).

### Exit Gate

- On a fixture repository with the mock provider, `build` goes from goal to
  roadmap to per-phase plans to executed tasks with suspend/resume across a
  simulated daemon restart, all decisions auto-answered by test policy.

## Phase 5: `st` Sub-Agent Validation Loop and Delegation

### Scope

- Supervisor children, each an isolated session with its own model
  (`AgentDefinition.model` per task type): acceptance-criteria writer, test
  writer, validator. Implementation agent never writes its own acceptance
  tests (tunnel-vision separation is structural: separate sessions, separate
  histories).
- Planning-time task typing in the roadmap schema (`type`, `model`,
  `delegate` fields) mapped at execution to: base agent run, specific Prism
  model, or Antigravity delegation via
  `createAntigravityDelegationTool`-equivalent wiring behind a feature check
  for `agy` availability.
- Validation node runs named checks (`coding_check`) with diagnostic deltas
  and feeds failures back as the next auto-prompt; bounded retry budget per
  task; escalation to user decision after budget exhaustion.
- Optional: Prism evals wired as a release gate for `st` behavior
  regressions.

### Exit Gate

- Fixture build completes with criteria/tests authored by child agents that
  never saw the implementation agent's session, and proven by store
  inspection; per-task model routing demonstrated with at least two
  providers/models in one run; Antigravity path proven or proven-absent with
  a clean skip.

## Phase 6: `st` Memory Cadence and Autonomy Hardening

### Scope

- Observational memory attached to the `st` orchestrator session and each
  durable child session: per-task compaction (fast strategy), recall tool
  active, `om:status`/`om:view` surfaces in Clay UI.
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

## Phase 7: Hardening, Documentation, and Distribution

### Scope

- `roadmap.md` (this file) finalized post-review; per-phase numbered plans
  in `plans/` (105+) created at phase start per `create-plan`; decision logs
  for the open decisions below and any new ones.
- Package authoring guide for agent packages; `st` published as the reference
  third-party agent package; base agent shipped with Clay unchanged for
  users who never install `st`.
- Code-wiki updates, manual test modules, registry/doc truth tests per
  project documentation-as-code rules.
- Performance and security review: daemon event throughput, memory worker
  overhead, approval UX latency, redaction coverage across new event kinds.

### Exit Gate

- Linux blocking gates, daemon tests, package conformance, and manual test
  plan all pass; documentation is internally consistent; `st` install/
  remove cycles leave no residue in the base agent.

## Open Decisions (need `decision-logs/` before implementation)

1. `st` package name, registry id, and distribution channel.
2. The initial workflow-type set beyond `build` (fix? research? review?) and
   their loop semantics.
3. Whether E2 lands as a documented pattern or a new Prism primitive.
4. Default acceptance: auto-approve scope for `st` (which actions still
   require explicit user approval in fully-autonomous runs).
5. Observational memory defaults: worker models, thresholds, retention per
   session vs per workspace.

## Post-Roadmap (not in scope)

- Additional `st` workflow types and meta-agent packages (Personal, Work,
  Research, Finance per the superseded roadmap's post-parity list).
- Cross-session shared memory scopes (E5) if per-session composition proves
  insufficient.
- Remote/distributed `st` runs over the Clay server protocol.
