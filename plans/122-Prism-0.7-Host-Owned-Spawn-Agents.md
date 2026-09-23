# Prism 0.7 Host-Owned Spawn Agents (Phase 6 Primitive)

Depends on: `plans/120-Adopt-Prism-0.7.0-Pin-And-Event-Mapper.md`. Plan 121
is useful (`toolNames`) but not a hard gate.

This plan is **Cut 3**: install Prism’s host-owned `spawn_agent` /
`wait_agent` / `cancel_agent` into coding profiles so Phase 6
(test/validation children) does **not** grow a second supervisor. It is
the primitive, not the build/fix orchestrator.

Out of this plan (Phase 5/6 later): `/start`, `runtime/workflows`, frozen
`create-test-plan` / `create-validation-plan`, ponytail-review-in-validation,
per-task commits, child findings appended onto the main plan, Memory tab
chrome.

Scope note: **no new UI**. Children appear as ordinary tool calls
(`spawn_agent` / `wait_agent` / `cancel_agent`) on the existing transcript.
`subagent_started` / `subagent_stopped` map onto existing
`AgentWireEvent::Tool` rows (or stay dropped if fields are insufficient —
do not add a panel). No prototype/approval gate. No Clay JS package.
Skip primitive-review, package-runtime, default-loading, grammar, package
UI/authoring, example-config, live launch-test. Spawn is daemon-owned, not
a package-triggered process grant (Agent Host: core spawns Prism; packages
cannot). Clay JS API and configuration tasks are verification-only.

Context7 has no `@arnilo/prism`. Authoritative APIs: local Prism 0.7.0.

## Objectives

- Coding profiles get `createSpawnAgentTool` + `createWaitAgentTool` +
  `createCancelAgentTool` over a host `createSupervisor` whose catalog is
  exactly two child ids: `test` and `validation`.
- Children are isolated Prism sessions (own history, own model = parent
  provider/model for now). They do not receive spawn tools (no recursion).
  Model arguments cannot supply child tools, identity, scopes, or higher
  limits (closed schema).
- Child identity **narrows** from the parent (`narrowIdentity` +
  `assertIdentityPropagation`). Parent abort cancels running children.
- Document I/O stays on the Clay document registry: children reuse the
  **parent** `sessionId` + `workspaceRoot` for `buildCodingTools`. Isolation
  this cut is transcript/model/tools, not git worktrees.
- Map `subagent_started` / `subagent_stopped` to existing Tool wire events
  so the panel already shows them. Unknown other lifecycle types stay
  dropped (plan 120).
- Async handles stay in-process: they die on daemon restart (document this;
  do not fake durability).

## Expected Outcome

- A coding session’s tool list includes `spawn_agent`, `wait_agent`,
  `cancel_agent`. Chat / tool-free profiles do not.
- `spawn_agent` with `childId: "test"|"validation"` runs. Unknown
  `childId` is a standard tool error before delegation.
- Child cannot call `spawn_agent`. Child identity scopes ⊆ parent.
- Transcript shows spawn/wait/cancel as tool rows. No new AG-UI event
  types.
- `cd clay-agent && npm test` and Linux cargo gates pass.
- Wiki states: primitive shipped; Phase 6 loop not shipped; worktrees
  deferred; handles not restart-durable.

## Tasks

- [x] Install supervisor + spawn/wait/cancel on coding profiles
  - Acceptance Criteria:
    - Functional: Coding `createAgent` / session tools include the three
      supervisor tools. Catalog ids are only `test` and `validation`.
      Closed schema: `childId`, `input`, optional `threadId`,
      `mode: "sync"|"async"` (default `sync`). Chat profiles unchanged.
      Children built with `buildCodingTools` (same workspace, parent
      session id for reverse-RPC document ops) plus ask/approval policy
      of the parent. Children do not get spawn/wait/cancel.
    - Performance: Supervisor is in-process. Sync spawn blocks the parent
      tool call until the child finishes (Prism). Async returns
      `{ delegationId, status: "running" }` without waiting. No extra
      daemon process.
    - Code Quality: Import from `@arnilo/prism-core/runtime/supervisor`.
      Do not wrap with a Clay `Supervisor` class. Do not add workflows.
    - Security: Host-owned catalog only. `narrowIdentity` from
      `this.runIdentity()`. Child cannot widen tenant/scopes/expiry.
      Results/errors redacted with the parent redactor. No ACP. No
      package-spawned process.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` §21
      - `/home/arn/Projects/prism/docs/supervisors.md`
      - `/home/arn/Projects/prism/docs/multi-agent-patterns.md`
      - `/home/arn/Projects/prism/docs/agent-identity.md` (narrowing)
      - `clay-agent/src/coding-tools.ts` `buildCodingTools`
      - `clay-agent/src/host.ts` `runIdentity()`, coding tool assembly
      - `.agents/skills/clay-execution/references/packages.md` (Agent Host)
    - Options Considered:
      - Hand-roll child `createAgent` without supervisor: duplicates 0.7.
      - Full Phase 6 frozen-plan loop now: out of scope.
      - Supervisor catalog `test` + `validation`, shared cwd/workspace:
        selected.
    - Chosen Approach:
      ```ts
      import {
        createCancelAgentTool,
        createSpawnAgentTool,
        createSupervisor,
        createWaitAgentTool,
      } from "@arnilo/prism-core/runtime/supervisor";

      const supervisor = createSupervisor({
        ownership: this.runIdentity(),
        children: {
          test: { createAgent: (ctx) => this.createChildAgent(ctx, live, "test") },
          validation: { createAgent: (ctx) => this.createChildAgent(ctx, live, "validation") },
        },
      });
      tools.push(
        createSpawnAgentTool({ supervisor }),
        createWaitAgentTool({ supervisor }),
        createCancelAgentTool({ supervisor }),
      );
      ```
      `createChildAgent` clones coding tools with parent `workspaceRoot` +
      parent `sessionId` for document ops, omits spawn tools, uses parent
      provider/model.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts`: supervisor per live coding session
      - `clay-agent/src/coding-tools.ts`: only if a child flag is cleaner
        than filtering spawn names
      - `clay-agent/src/__tests__/spawn-agent.test.ts` (new, small)
    - References:
      - `roadmap.md` Phase 6 (loop deferred; this is the primitive)
  - Test Cases to Write:
    - Coding profile tool names include `spawn_agent` / `wait_agent` /
      `cancel_agent`; chat does not.
    - Unknown `childId` → tool error, no child `createAgent` call.
    - Sync spawn of `test` with mock provider returns a child result;
      child tool list has no `spawn_agent`.
    - Identity: child scopes cannot be a superset (Prism throw /
      tool error).

- [x] Keep document-registry authority; defer worktrees
  - Acceptance Criteria:
    - Functional: Child `read`/`write`/`edit` still go through
      daemon→server reverse RPC with the **parent** session id. Dirty
      buffers in the parent workspace remain visible. Child writes are
      parent-workspace writes under the existing acceptance policy.
    - Performance: No `git worktree add` on spawn. No second sandbox.
    - Code Quality: Do not import `createWorktreeChildFactory` in this
      plan. Record the ceiling in wiki + this plan’s Compromises.
    - Security: No writes outside the session workspace without the
      existing host write-gate. No claim of OS isolation. Trusted
      subprocess language unchanged.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/coding-workspaces.md` (spawn
        isolation — **not adopted here**)
      - `clay-agent/src/document-ops.ts`
      - `.agents/skills/clay-execution/references/packages.md`
        (session workspace root is tool authority)
      - `roadmap.md` Phase 6 git isolation
    - Options Considered:
      - `createWorktreeChildFactory` now: child `cwd` is a worktree;
        Clay document RPC is session-workspace-keyed, so writes would
        miss the worktree or require unregistered roots. Unsafe/wrong
        without a child session root.
      - Child native fs tools bypassing the registry: violates
        “Rust document registry stays authoritative”.
      - Shared parent workspace + parent session id: selected.
    - Chosen Approach:
      - One paragraph in wiki/agent-host: worktrees wait until a child
        can own a registered workspace root (Phase 6 follow-up).
    - API Notes and Examples:
      ```ts
      // not this plan
      // createWorktreeChildFactory(factory, { workspaces, repositoryId })
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/clay-agent.md` (in the wiki task; this task
        only forbids the import and adds a daemon comment)
      - `clay-agent/src/host.ts`: comment on `createChildAgent`
    - References:
      - decision `2026-09-14-1705` (session owns workspace)
  - Test Cases to Write:
    - Child write via mock document RPC uses parent `sessionId`.
    - `clay-agent/src` contains no `createWorktreeChildFactory` import.

- [x] Map `subagent_*` onto existing Tool wire events
  - Acceptance Criteria:
    - Functional: `subagent_started` → `AgentWireEvent::Tool` phase
      Started; `subagent_stopped` → Tool phase Finished. `name` is
      `spawn_agent` (or the event’s child id if that is the stable
      field). Digests redacted/bounded like other tools. Other new
      types stay `None` (plan 120).
    - Performance: O(1) extra match arms. No new AG-UI event type, so
      frontend/AbstractAgent unchanged.
    - Code Quality: Reuse `AgentWireEvent::Tool`. No new variant.
    - Security: No raw child transcripts on the wire. Redact against
      session secrets.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/coding-agent-tools.md`
        (`observeSupervisorLifecycle`, `subagent_started` /
        `subagent_stopped`)
      - `src/server/agent/run.rs` `map_event` tool arms
      - `.agents/skills/clay-execution/references/protocol-perf.md`
    - Options Considered:
      - New `AgentWireEvent::Subagent` + panel chrome: UI gate, rejected.
      - Keep dropping `subagent_*`: spawn still works but the transcript
        only shows the parent tool call (often enough). Fallback if the
        event payload lacks `call.id`.
      - Map to Tool when `toolCallId`/`name` exist, else drop: selected.
    - Chosen Approach:
      ```rust
      "subagent_started" => AgentWireEvent::Tool {
          session_id: session_id.clone(),
          run_id,
          phase: AgentToolPhase::Started,
          name: json_string(event, &["name"]).or_else(|| "spawn_agent".into()),
          tool_call_id: json_string(event, &["toolCallId"]),
          // …redacted digests / no file…
      },
      ```
      Wire `observeSupervisorLifecycle` in clay-agent only if the coding
      session does not already emit `subagent_*` from the spawn tools.
      Check 0.7 coding-tools docs at implementation; do not double-emit.
    - Files to Create/Edit:
      - `src/server/agent/run.rs`
      - `src/server/agent.rs` mapper tests
      - `clay-agent/src/host.ts` only if lifecycle observer must be
        attached
    - References:
      - plan 120 unknown-drop contract
  - Test Cases to Write:
    - `subagent_started` / `subagent_stopped` map to Tool started/finished.
    - Missing ids → `None` (drop), not Started.
    - `attention_compiled` still `None`.

- [x] Document restart/non-durability and abort propagation
  - Acceptance Criteria:
    - Functional: Daemon restart forgets in-flight async handles (Prism
      contract). Parent `session.cancel` aborts running children. Tests
      assert cancel propagation with the mock provider.
    - Performance: No durable child checkpoint store.
    - Code Quality: README + wiki state “in-process handles”.
    - Security: Stale `delegationId` after restart is a tool error, not
      a cross-session resume.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` §21 (handles
        do not survive host restart)
      - `clay-agent/README.md` Durable runs
    - Options Considered:
      - Persist handles in sqlite: new store, rejected.
      - Document + cancel test: selected.
    - Chosen Approach:
      - One daemon test: async spawn, parent abort, child does not
        continue. README sentence.
    - API Notes and Examples:
      ```ts
      // after daemon restart, wait_agent(delegationId) → tool error
      ```
    - Files to Create/Edit:
      - `clay-agent/src/__tests__/spawn-agent.test.ts`
      - `clay-agent/README.md`
    - References:
      - `clay-agent/src/host.ts` `session.cancel` path
  - Test Cases to Write:
    - Parent abort → child not still running.
    - Fake `delegationId` → `wait_agent` error.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: No new Clay JS API. Spawn is a Prism tool on coding
      profiles, not `clay:agent.spawn`.
    - Performance: Verification adds no runtime work.
    - Code Quality: `map_event` stays `pub(super)`.
    - Security: No package API to spawn children.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - `agent.spawnChild()` JS API: rejected (packages cannot own the
        daemon; Agent Host).
      - Verification-only: selected.
    - Chosen Approach:
      - Diff review; expected no new APIs.
    - API Notes and Examples:
      ```ts
      // none
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `docs/index.md`
  - Test Cases to Write:
    - Existing doc-registry tests pass.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: No `init.js` child-model or spawn-enable flag. Coding
      profiles that already have tools get spawn; Chat stays tool-free.
    - Performance: No config reload change.
    - Code Quality: Child model inherits parent this cut (Phase 6 may
      add `AgentDefinition.model` per child later).
    - Security: No config grant that widens child identity.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - `agent.setOptions({ spawn: false })`: extra knob, YAGNI.
      - Verification-only: selected.
    - Chosen Approach:
      - Example config unchanged.
    - API Notes and Examples:
      ```js
      // no new init.js surface
      ```
    - Files to Create/Edit:
      - None expected.
    - References:
      - `examples/init.js`
  - Test Cases to Write:
    - None.

- [x] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Add 17-coding-agent steps: coding session lists
      `spawn_agent`; Chat does not; a mock/sync spawn of `test` shows a
      tool row; unknown child id errors. Worktrees are explicitly
      out-of-coverage. Do not weaken existing steps.
    - Performance: Spawn is not on the typing path.
    - Code Quality: Coverage matrix lists plan 122.
    - Security: Negative: model cannot name a child id that is not
      `test`/`validation`.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/17-coding-agent-parity.md`, `test-plan/index.md`
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - Skip manual (tools-only): rejected; spawn is user-visible.
      - Module 17 steps: selected.
    - Chosen Approach:
      - Numbered C-steps + C-N negative unknown child id.
    - API Notes and Examples:
      ```text
      test-plan/17-coding-agent-parity.md
      ```
    - Files to Create/Edit:
      - `test-plan/17-coding-agent-parity.md`
      - `test-plan/index.md`
    - References:
      - `packages/coding-agent/docs/parity-checklist.md` if spawn should
        be named there (parity list only if it already tracks tool names)
  - Test Cases to Write:
    - Manual steps above; record GUI blocker if live spawn against a
      real model is not run.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation
      tasks are complete, or explicitly verified as unchanged for non-code
      work.
    - Performance: Wiki updates add no runtime work and document
      performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it
      works, invariants/tradeoffs, source/test paths, examples where
      useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries,
      permissions, validation, secrets handling, or external authority
      without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass: selected.
    - Chosen Approach:
      - `docs/wiki/modules/clay-agent.md`: supervisor catalog, shared
        workspace, no worktrees, in-process handles, identity narrowing.
      - `docs/wiki/modules/agent-protocol.md`: `subagent_*` → Tool.
    - Files to Create/Edit:
      - `docs/wiki/index.md`
      - `docs/wiki/modules/clay-agent.md`
      - `docs/wiki/modules/agent-protocol.md`
  - Test Cases to Write:
    - Manual wiki review: index links; pages match shipped behavior.

## Compromises Made

- Shared parent workspace and parent document `sessionId`. Not git
  worktrees. Parallel children can collide on files. Upgrade:
  `createWorktreeChildFactory` only after a child owns a registered
  workspace root.
- Child model = parent model. Per-child `AgentDefinition.model` waits
  for Phase 6 routing.
- Async handles die on daemon restart. Durable nested approvals still
  use the parent’s existing checkpoint path only.
- No frozen test/validation plans, no `/start` workflow.

## Further Actions

- Plan 123 work-scopes around parent runs and (later) children.
- Phase 6 loop: frozen test/validation plans, ponytail-review,
  append-to-main-plan, per-task model routing.
- Worktree isolation once child sessions can bind a distinct registered
  root inside the workspace (`worktreeRoots` host-approved).

## Execution Record (Linux, 2026-09-17)

Gates: `cd clay-agent && npm test` 175 passed / 1 pre-existing skip (176
total — six new `spawn-agent.test.ts` cases); `cargo fmt --check`,
`cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`
clean; `cargo test --test protocol` 216/216 (new
`map_event_maps_subagent_lifecycle_onto_tool_rows`; ledger updated with
C53–C55); `cargo test --lib` green (1370/1370 on repeated runs — the
`js_runtime::coding_agent_clean_init…` global-queue test flakes roughly 1 in
3 full-lib runs on this tree **with this plan's changes stashed too**, so it
is a pre-existing plan-121-tree flake, not a plan 122 regression; it passes
in isolation and in module-filtered runs).

Implementation notes:

- `sessionSupervisor` (host.ts) creates one Prism supervisor per live coding
  session from `@arnilo/prism-core/runtime/supervisor` (no Clay wrapper
  class); `createChildAgent` returns an isolated `Agent` on the parent
  provider/model with the shared `sessionCodingTools` extraction (parent
  `sessionId` + workspace root, parent ask/approval policy, no spawn tools).
- `runIdentity()` gained `userId: "clay-agent"`: Prism supervisor ownership
  validation requires an account/user anchor and
  `assertIdentityMatchesOwnership` requires identity/ownership agreement.
  Scope `"workspace"` is preserved (not narrowed away) so mediated child
  writes keep working.
- Lifecycle pump (`observeSupervisorLifecycle` → daemon `event` emissions)
  stops on session delete, model rebuild (`recreateSessionModel`), and daemon
  shutdown (`LiveSession.stopSubagentLifecycle`).
- Rust mapper: one new arm for identified `subagent_started`/`subagent_stopped`
  → existing `AgentWireEvent::Tool` (name = `childId`, id = `delegationId`,
  status digest on stop); missing ids and `delegation_*` keep the plan-120
  drop contract. `map_event` stays `pub(super)`.
- Verification-only tasks (Clay JS APIs, configuration): diff-verified — no
  new `clay:agent` API, no `init.js` surface, no config flag; spawn is a
  Prism tool on coding profiles only.
- Manual plan: module 17 gained C53–C55 + C-N15 with an execution record;
  coverage matrix row and parity ledger updated. Live GUI legs unresolved on
  the standing host input ceiling (same blocker as plans 119/121 records);
  mock-provider automated suites pin the same paths. Worktree isolation
  explicitly out of coverage.

Wiki: `docs/wiki/modules/clay-agent.md` (plan 122 section + tests list),
`docs/wiki/modules/agent-protocol.md` (subagent → Tool arm), index row.
README: `clay-agent/README.md` "Supervisor children" section (in-process
handles, restart non-durability, shared workspace).
