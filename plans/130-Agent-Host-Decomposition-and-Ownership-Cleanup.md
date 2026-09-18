# 130 — clay-agent Host Decomposition and Agent Ownership Cleanup

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
C2, A1, A2-residual (review §4, §10). Scope: `clay-agent/src/host.ts` (one class,
~3,900 lines, 115 members) and the server-side process-global agent authority
(`src/server/agent.rs`). No wire-protocol changes.

## Objectives

- C2: split `ClayAgentHost` into focused modules (sessions, skills/prompt
  assembly, persistence, OAuth, context/mentions, wiki/graft enablement) with the
  class reduced to wiring + RPC dispatch; identical external behavior (same RPC
  method set, same responses).
- A1: replace the process-global `AGENT_HOST_AUTHORITY`/pending-queue in
  `src/server/agent.rs` with dependency injection through the server's state
  (the ownership model used everywhere else), removing the "first install wins"
  test hazard documented in the code comment.
- A2-residual: verify the daemon resume path reads the recorded session-search
  key back (plan 119 SC-6 evidence flagged `host.ts:2537` never reading it);
  fix if still broken, as part of touching the session code.

## Expected Outcome

- `host.ts` is a thin dispatch file; no single extracted module > ~800 lines;
  `sessionPrompt` (222 lines), `emitCommandFeedback` (196), `createSession` (140)
  are themselves decomposed or moved with their tests.
- clay-agent suite (`clay-agent/src/__tests__/`, 29 files) passes unmodified —
  behavior identical.
- `src/server/agent.rs` has no process global; server construction owns the
  `AgentHost`; multi-server tests (the existing isolation suites) no longer
  depend on install order; a regression test constructs two servers in one
  process and asserts independent agent hosts.
- All Linux gates green (root + agent suites).

## Tasks

- [ ] Baseline gates and module inventory
  - Acceptance Criteria:
    - Functional: root + clay-agent suites pass untouched; record exit codes.
    - Performance: none (structural work).
    - Code Quality: record a member→concern inventory of `ClayAgentHost` (which members touch sessions/skills/persistence/OAuth/context/wiki/graft) as the split map.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §4 (C2), §10 (A1).
      - `clay-agent/src/host.ts` (top-level helpers already pure; class is the monolith).
    - Options Considered:
      - Skip inventory: the split map is what makes the refactor mechanical. Recorded.
    - Chosen Approach:
      - `graft skeleton clay-agent/src/host.ts` + read pass; store the map in task evidence.
    - Files to Create/Edit:
      - None.
    - References:
      - `plans/119-…` SC-3 (server `agent.rs` split precedent — same method, server side).
  - Test Cases to Write:
    - None (evidence-recording task).

- [ ] Extract host concerns into modules behind the class facade
  - Acceptance Criteria:
    - Functional: extracted modules (e.g. `sessions.ts`, `skills.ts`, `prompt-assembly.ts`, `host-persistence.ts`, `oauth.ts`, `context-mentions.ts`, `integrations.ts` — final names per inventory) own their logic; `ClayAgentHost` keeps the public RPC surface and delegates; all 29 test files pass unmodified.
    - Performance: no per-request behavior change; startup unchanged (no new eager I/O).
    - Code Quality: no exported-symbol renames visible to `main.ts`/`coding-tools.ts`/tests unless the test file is updated in the same step with an equivalent assertion; clippy/eslint (agent lint config) clean.
    - Security: the obscura flags assertion (`assertNoInsecureFlags`), redaction (`redact.ts`) call sites, and MCP allowlist wiring move verbatim — diff review recorded.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/host.ts:948–4881` (class), `main.ts` (construction), `coding-tools.ts`, `mcp.ts` (collaborators).
      - `plans/119-…` SC-3/SC-4 evidence (god-object decomposition method used for `agent.rs` and `CodingAgentPanel.tsx`).
    - Options Considered:
      - Split the class into multiple classes with shared state object: risks two-headed god object.
      - Facade + pure modules taking explicit state (chosen in plan 119 SC-3 analog). (Chosen.)
      - Rewrite as functions-only: too large a diff against 29 test files.
    - Chosen Approach:
      - One concern per step, suite green between steps; `LiveSession` state stays in the host, helpers become pure functions over it.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts` (shrinks), `clay-agent/src/host/*.ts` (new modules, tentative names per inventory).
    - References:
      - Review C2.
  - Test Cases to Write:
    - Existing 29 test files are the regression net; add module-level unit tests only where a pure helper gains a clear contract (e.g. branch summary ranking) — max 3 focused files.

- [ ] Decompose the oversized methods (`sessionPrompt`, `emitCommandFeedback`, `createSession`, `contextCategories`, `runHostCommand`)
  - Acceptance Criteria:
    - Functional: no method > ~80 lines; behavior identical (suite green).
    - Performance: `sessionPrompt` assembly cost unchanged or better (no added per-call allocation beyond today's).
    - Code Quality: each extracted step has a name that says what it does; no boolean-flag parameters introduced to multiplex behaviors.
    - Security: prompt assembly keeps the read-layer byte caps (`readPromptLayer`) and ranking (`rankedSystemPromptContributions`) — caps applied at the same points.
  - Approach:
    - Documentation Reviewed:
      - `clay-agent/src/host.ts` at the five method sites (line spans recorded in review).
    - Options Considered:
      - Leave long methods if "cohesive": five methods over 100 lines in one class is the review finding; decompose. (Chosen.)
    - Chosen Approach:
      - Named private steps within the owning module from task 2.
    - Files to Create/Edit:
      - The task-2 modules owning these methods.
    - References:
      - Review C2 (method sizes).
  - Test Cases to Write:
    - None beyond suite (pure decomposition); `sessionPrompt` contract covered by existing om/prompt tests.

- [ ] Replace the process-global agent authority with injected ownership (A1)
  - Acceptance Criteria:
    - Functional: `AGENT_HOST_AUTHORITY` OnceLock and the global `PENDING_PACKAGE_REGISTRATIONS` queue are removed; `AgentHost` is owned by the server's state graph (constructed in server build, passed to ops that need it); package registrations queue on the host instance, not a process global.
    - Performance: none (wiring); no extra lock on the RPC path.
    - Code Quality: `install_global`/`global()`/`take_pending_package_registrations` deleted with their call sites; clippy clean; the "first install wins" comment hazard gone.
    - Security: fail-closed behavior preserved — a missing host still errors `ServiceStopped`; no new path grants agent authority by existing; js-runtime op wiring reaches the host only through the injected route (tests deny ops when no host is wired).
  - Approach:
    - Documentation Reviewed:
      - `src/server/agent.rs:280–380` (authority + pending queue), `src/server/ops/packages.rs` (queue call sites), `src/server/js_runtime/worker.rs` (`ClayOpState` construction — injection point).
      - Plan 119 SC-6 (workspace-keyed registry — the ownership model to match).
    - Options Considered:
      - Keep global, add generation tokens: preserves the hazard the comment admits.
      - Inject through `ClayOpState`/server state like every other coordinator. (Chosen.)
    - Chosen Approach:
      - `ClayOpState` carries `Option<Arc<AgentHost>>` (or the existing handle type) set at server construction; ops resolve from state.
    - Files to Create/Edit:
      - `src/server/agent.rs`, `src/server/ops/mod.rs`, `src/server/ops/packages.rs`, `src/server/js_runtime/worker.rs`, `src/server/mod.rs` (construction).
    - References:
      - Review A1; decision logs referenced by SC-6 for ownership consistency.
  - Test Cases to Write:
    - `two_servers_independent_agent_hosts`: construct two `IpcServer`s in one process; assert each routes to its own host (the case the old comment says was broken).
    - `package_registration_without_host_fails_closed`: unchanged error semantics.

- [ ] Verify and fix the daemon resume workspace binding (A2-residual)
  - Acceptance Criteria:
    - Functional: a resumed session's tool cwd/acceptance roots/wiki-graft binding derive from the session's recorded search key (not `process.cwd()`), per plan 119 SC-6 evidence note at `host.ts:2537`; if already fixed, record evidence and close.
    - Performance: none.
    - Code Quality: resume path reads the persisted key once at session recreation; no per-tool re-resolution added.
    - Security: acceptance roots derived from recorded workspace must still pass the existing root-validation on use.
  - Approach:
    - Documentation Reviewed:
      - `plans/119-…md` SC-6 decision-log task evidence (the `host.ts:2537` note); `clay-agent/src/host.ts` resume path.
    - Options Considered:
      - Defer to a language-server authority plan: this is agent-session ownership, belongs here. (Chosen: fix here.)
    - Chosen Approach:
      - Persisted key → `createSession` workspace resolution; test with a scratch workspace differing from daemon cwd.
    - Files to Create/Edit:
      - `clay-agent/src/host/*.ts` (session module from task 2).
    - References:
      - Decision log `2026-09-14-1705-workspace-scoped-agent-sessions.md`.
  - Test Cases to Write:
    - `resume_binds_recorded_workspace_not_cwd`: spawn host in dir A with session recorded in dir B; resumed tool write lands under B.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: agent-surface modules re-run on a real Linux build (session create/resume/branch, slash palette, tool approvals); record pass/fail.
    - Performance: none.
    - Code Quality: add a step for "resume a session whose workspace differs from the daemon cwd" if the module lacks one.
    - Security: approval/root-gating steps re-run.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`.
    - Chosen Approach:
      - Regression pass + the one new step.
    - Files to Create/Edit:
      - Agent module file per index (tentative), `test-plan/index.md`.
    - References:
      - None beyond index.
  - Test Cases to Write:
    - Manual step as described.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed (agent RPC method set identical); verify via diff; record "no JS API change".
    - Performance: none.
    - Code Quality: registry/doc-guard suites pass.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach:
      - Verify-only.
    - Files to Create/Edit:
      - None expected.
    - References:
      - Decision log 2026-05-08-1509.
  - Test Cases to Write:
    - None.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/clay-agent.md` and the server agent page reflect the module map and injected ownership (no process global).
    - Performance: none.
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: wiki documents the fail-closed no-host path and that registrations queue per-host, not process-wide.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`; `docs/wiki/modules/clay-agent.md`, `docs/wiki/modules/agent-process-manager.md`.
    - Chosen Approach:
      - Update once after tests pass.
    - Files to Create/Edit:
      - `docs/wiki/modules/clay-agent.md`, agent-related pages, `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
