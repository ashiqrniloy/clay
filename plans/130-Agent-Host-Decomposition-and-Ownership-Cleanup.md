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

- [x] Baseline gates and module inventory
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
  - Evidence (2026-09-20):
    - Environment: HEAD `66648e3` ("WIP", dirty tree — in-flight Control Center migration work, ~66 modified files, none under `clay-agent/src/` or `src/server/agent.rs`); Node v24.19.0, rustc 1.96.1.
    - Gates: clay-agent suite `npm test` **exit 0** (181 tests: 180 pass, 1 skip, 0 fail; 29 files in `src/__tests__/`); `cargo fmt --check` **exit 0**; `cargo clippy --all-targets -- -D warnings` **exit 0**; root `cargo test` **exit 101** — 1416 passed / 1 failed / 1 ignored.
    - Root failure is pre-existing, deterministic under the full parallel run (2/2 runs) and passes in isolation: `server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults` (`src/server/js_runtime/tests.rs:11021`). Cause is the A1 target itself: another test's live host occupies `AGENT_HOST_AUTHORITY` first (stdout shows `agentProfile.register 'coding' host=live -> Ok(…"registered": true…)`), so the profile registers directly instead of queueing in `PENDING_PACKAGE_REGISTRATIONS`, and the test's `drain_all_pending_registrations()` finds no queued profile entry — the exact "first install wins" order-dependency task 4 removes. Recorded as baseline-red-accepted; the A1 regression test (`two_servers_independent_agent_hosts`) plus this suite re-run are the closure evidence.
    - Member→concern inventory of `ClayAgentHost` (`host.ts:948–4703`, ~100 class members; top-level helpers L134–946 are already pure and stay put), via `graft skeleton` + read pass. Split map for task 2 (final names per extraction step):
      - **host (wiring/dispatch, stays)**: constructor 1031, `create` 1061, `close` 1143, `request` 1246, `resolveReverse` 1270, `handle` 1283, `redactError` 1475, `refreshRedactor` 1481, `rememberSecret` 1485, `runIdentity` 4389, `runOwnership` 4404.
      - **agent roots / MCP config**: `ensureCapabilities` 1176, `capabilityTools` 1221, `mcpServerOutcomes` 1229, `resolveAgentRoot` 1384, `mcpAllowListFor` 1409, `defaultAgentConfig` 1418, `agentTypeOf` 1430, `agentRootConfig` 1441.
      - **sessions (lifecycle)**: `sessionNew` 1491, `createSession` 1587, `recreateSessionModel` 1779, `sessionList` 2233, `sessionLoad` 2252, `sessionResume` 2764, `ensureLive` 2784, `sessionDelete` 2844, `sessionAutonomy` 1471, `sessionSetAgent` 2690, `persistAgentType` 2736, `sessionResumable` 3386, `sessionCheckout` 3424, `sessionFork` 3512, `sessionClone` 3522, `sessionCheckpoint` 3548, `sessionTreeSummary` 4570, `renderTreeText` 4657.
      - **sessions (run)**: `sessionPrompt` 2860, `sessionCancel` 3286, `sessionSteer` 3294, `sessionCompact` 3303, `sessionSetAutonomy` 3335, `sessionSearch` 3349, `runResume` 3556, `resumeDecision` 3626, `durableRunState` 3157, `appendBranchSummary` 3439, `refineBranchSummary` 3461.
      - **session tooling/supervisor**: `runLimits` 1727, `autoCompaction` 1753, `resolveOmFlag` 1819, `profileWantsCodingTools` 1826, `sessionTools` 1835, `omWorkerConfig` 1873, `attachOm` 1913, `resolveCompaction` 1960, `buildSessionTools` 1975, `sessionCodingTools` 1995, `sessionSupervisor` 2036, `createChildAgent` 2081.
      - **observational memory**: `parseOmWorkers` 2523, `sessionOmSet` 2558, `persistOmWorkers` 2588, `sessionOmActivity` 2614, `omActivitySummary` 2673, `omRunScope` 3082, `countOpenedScopes` 3092, `omScopes` 3103, `omSpawnScopes` 3124.
      - **skills**: `resolveSkills` 2107, `ensureSkillDiscovery` 2151, `discoverWorkspaceSkills` 2172, `scanSkillsDir` 2195, `agentSkillEnabled` 2229, `skillRegister` 3912, `skillList` 3934, `visibleSkills` 3942.
      - **context/mentions**: `sessionContext` 2277, `activeContextEntries` 2299, `contextCategories` 2315, `contextItem` 2445, `resolveMentions` 3179, `readMentionFile` 3235, `workspaceListFiles` 3260.
      - **providers/OAuth/credentials**: `providerList` 3646, `providerConfigured` 3672, `providerStatus` 3687, `modelList` 3703, `modelSearch` 3727, `credentialPut` 3739, `refreshOllamaModels` 3760, `defaultCredentialName` 3787, `oauthStart` 3792, `oauthPoll` 3846, `credentialDelete` 3862 (+ top-level `PendingOauth` 796, `isOauth`/`authMethodsFor`/`withUrlMethod` 922–946).
      - **profiles/commands**: `profileList` 3878, `profileRegister` 3889, `commandRegister` 3959, `registeredCommand` 3987, `environmentList` 3998, `commandDispatch` 4042, `runSetOptions` 4065, `commandDrivers` 4409, `runHostCommand` 4421, `requireCommandSession` 4550, `requireLive` 4556, `emitCommandFeedback` 4686.
      - **wiki/graft enablement**: `knowledgeSetOptions` 4118, `resolveGraftDeepModel` 4167, `disableWiki` 4195, `enableWiki` 4202, `wikiTools` 4272, `graftTools` 4282, `enableGraft` 4298, `disableGraft` 4354, `ensureGraftBound` 4367.
      - Oversized methods confirmed inside the map: `sessionPrompt` 2860–3074 (214 lines), `emitCommandFeedback` 4686–4702+helpers, `createSession` 1587–1718 (131), `contextCategories` 2315–2442 (127), `runHostCommand` 4421–4547 (126) — task 3 targets.

- [x] Extract host concerns into modules behind the class facade
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
  - Evidence (2026-09-20):
    - Extracted modules (`clay-agent/src/host/`, all new, untracked): `internals.ts` 376 (shared types, constants, pure helpers `reqString`/`optString`/`rpcError`/`clipUtf8`/`LiveSession`/`omWorkerModels`), `skills.ts` 504, `session-build.ts` 582 (largest — under the ~800 ceiling), `session-run.ts` 505, `sessions.ts` 455, `context-mentions.ts` 456, `commands.ts` 454, `integrations.ts` 332, `session-tree.ts` 304, `providers.ts` 278, `agent-roots.ts` 231, `om.ts` 178. `host.ts` **4703 → 1267 lines**.
    - Method used: each module exports free functions `f(host: ClayAgentHost, …)`; the class keeps its members and public RPC surface as one-line delegates (e.g. `private async sessionPrompt(p) { return hostSessionRun.sessionPrompt(this, p); }`). `ClayAgentHost` itself is the module contract (`import type`, no runtime cycle); members reached from modules lost the `private` modifier but kept `readonly`. State (`live`, `persistence`, `kernel`, `secrets`, `runConfig`, …) stays on the host — no second god object.
    - Deviations from the tentative inventory names: `oauth.ts` folded into `providers.ts` (OAuth is credential-store state, not a separate concern), `prompt-assembly.ts` into `skills.ts` + `session-build.ts` (prompt layers are profile/skill-driven), `host-persistence.ts` not created (persistence is a Prism `SqlitePersistence` handle the host owns and passes to the modules).
    - Gates: `npx tsc -p tsconfig.json --noEmit` **exit 0**; `npm test` (tsc + `node --test`) **exit 0** — 181 tests, 180 pass, 1 skip, 0 fail, **all 29 test files unmodified** (`git status clay-agent/src/__tests__` clean). No TS lint config exists in `clay-agent` (no eslint/biome); tsc + suite are the gates. Root Rust gates untouched by this step (no Rust files changed); root `cargo test` remains baseline-red from the task-4 A1 hazard.
    - Security posture: redaction call sites moved verbatim (`redactor.redact` in `sessions.ts`/`providers.ts`, `redactAgentEvent` in `session-run.ts`, `redactError` stays on the host); `assertNoInsecureFlags` call sites in `obscura.ts` untouched; MCP allow-list wiring (`mcpAllowListFor`/`mcpAllowListByRoot`/`ensureCapabilities`) moved verbatim into `agent-roots.ts`/`sessions.ts`.
    - Task 3 scope is now measured, not estimated: remaining >70-line functions are `sessionPrompt` 215 (`session-run.ts`), `contextCategories` 129 (`context-mentions.ts`), `runHostCommand` 128 (`commands.ts`), `createSession` 126 (`session-build.ts`), `sessionNew` 95 (`sessions.ts`), `sessionTreeSummary` 84 (`session-tree.ts`); `emitCommandFeedback` is 20 lines after the move, so that review item is closed by extraction.

- [x] Decompose the oversized methods (`sessionPrompt`, `emitCommandFeedback`, `createSession`, `contextCategories`, `runHostCommand`)
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
  - Evidence (2026-09-20):
    - Named steps per method (line counts before → after, measured over the whole function):
      - `sessionPrompt` 215 → **22** (`session-run.ts`): `dispatchSlashCommand` 28 (registered-command dispatch + the `/wiki-init` enable-then-dispatch path + transcript feedback), `applyRunOverrides` 22 (provider/model rebuild + persist), `resolveRunOptions` 11 = `resolveThinkingLevel` 11 + `resolveToolNames` 9 (validation order preserved), `streamPromptRun` 50 (stream + event loop + work-scope wrapping, returns `PromptOutcome`), `promptReply` 22 (suspension stash + reply shape). `omRunScope`/`countOpenedScopes` already extracted in task 2.
      - `createSession` 126 → **51**: `resolveSystemPromptLayers` 17 (base → SYSTEM.md → AGENTS.md + profile contributions), `attentionSetup` 20 (compiler gate + auto-compaction pair), `agentConfig` 40 (the Prism `createAgent` options object; `id` stays the profile name, not `def.name`).
      - `contextCategories` 129 → **43**: `systemPromptItems` 20, `collectMessageItems` 50 (per-block categories), `skillEventItems` 17 (legacy `tool_execution_started` skill events), shared `ContextBuckets` type; the returned category list/order and `MAX_CONTEXT_ITEMS` slicing are unchanged.
      - `runHostCommand` 128 → **20**: `runDriverCommand` 40 (steer/startRun/startWorkflow, fail-closed on absent drivers), `runSessionCommand` 62 (the nine session-scoped handlers, now one `callRpc` shape + `requireEntryId`), `callRpc` 7.
      - Also split while in scope: `sessionNew` 95 → **40** with `resolveAgentBinding` (shared verbatim with `sessionSetAgent`), `persistSessionRecord`, `liveSessionFor` (shared by `sessionNew`/`ensureLive` so the two live-book shapes cannot drift); `sessionTreeSummary` 84 → **26** with `checkpointedEntryIds` (2 s advisory budget), `treeEntryRow`, `entryPreview` (the unused `summarized` local map was dropped — dead in the original); host dispatch `handle` 91 → `handle` **57** + `handleAgentSurface` **40** (non-session half of the table).
      - `emitCommandFeedback`: no further work — extraction in task 2 left it at 20 lines.
    - Result: **no function or class method in `clay-agent/src/host*` exceeds 80 lines; largest is 62** (`runSessionCommand`). `host.ts` 1195 lines; modules 231–603.
    - Gates: `npx tsc -p tsconfig.json --noEmit` **exit 0**; `npm test` **exit 0** — 181 tests, 180 pass, 1 skip, 0 fail, all 29 test files unmodified. No TS lint config in `clay-agent`; tsc + suite are the gates.
    - Security/behavior: prompt read-layer caps untouched (`readPromptLayer` + `MAX_PROMPT_LAYER_BYTES` in `skills.ts:182,203`); `rankedSystemPromptContributions` call sites preserved (`context-mentions.ts:205,312`); `redact*` call sites unchanged; no boolean-flag parameters introduced (helpers take data objects; the one optional callback in the slash path was replaced by a local closure). Per-run option validation still precedes `resolveMentions` (§I/O), so an invalid `thinkingLevel`/`toolNames` cannot read files or mutate loaded skills on the way to failing.
    - Performance: `sessionPrompt` adds one small options object + one outcome object per run, no new per-turn allocation of note; option/validation order and the single `stream()` call are unchanged.
    - Pre-existing oddity noticed while extracting `ensureLive` (left as-is, out of scope): the restored live book records `fullAutonomy: false` while `createSession` receives `metadata.fullAutonomy !== false`, so a resumed coding session re-arms tool-approval interrupts. Flagged for a separate decision — not changed here.

- [x] Replace the process-global agent authority with injected ownership (A1)
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
  - Evidence (2026-09-20):
    - Removed from `src/server/agent.rs` (≈150 lines): the `AGENT_HOST_AUTHORITY` `OnceLock`, `install_global`, `global()`, the process-global `PENDING_PACKAGE_REGISTRATIONS` queue with `queue_package_registration`/`take_pending_package_registrations`, both test-only drains, and the "first install wins" comment that documented the hazard. Added: `AgentHostHandle::new` (server-owned construction) and `queue_registration_now` (`AgentHostHandle:347`, `AgentHost:925`) — non-async `try_lock` move bounded by `PENDING_REGISTRATION_CAP` (64, `agent.rs:306`) → `ServiceStopped` when full, so wiring never awaits on the RPC path.
    - Injection route (the ownership model plan 119 SC-6 uses, deviation from the plan's `Option<Arc<AgentHost>>`: the lane carries the same `AgentHostHandle` the ops already used, keeping `rpc`/`rpc_or_queue` semantics): `IpcServer` construction constructs the handle (`src/server/mod.rs:756`) → `RuntimeGenerationStore::initial(handle)` (`:787`) → `RuntimeGeneration::initial` installs it into the service (`ClayJsRuntimeService::set_agent_host`, `js_runtime/mod.rs:608`) → `wire_domain_lanes`/`wire_runtime_publishers` (`:545`) sets it on every lane's `ClayOpState` → ops resolve it from their own lane (`ops/agent.rs:15` `lane_host(state)`, used by all 11 async agent ops and the 5 registration ops; no `state` in signature → compile error).
    - Per-lane registrations replace the process global: `ClayOpState` gains `agent_host: Mutex<Option<AgentHostHandle>>` + `pending_agent_registrations: Mutex<Vec<PackageRegistration>>` (`ops/mod.rs:308,424`) with `set_agent_host()` 675 (attaches + drains the lane queue into the host's queue), `agent_host()` 695 (fail closed when unwired), `queue_agent_registration()` 708 (same 64 cap, over-cap declaration still fails closed), `take_pending_agent_registrations()` 729. A hostless runtime (embedded worker, unit harness) queues on its own state and the wiring step hands it over — the old handoff, now per-lane state.
    - Reload/poison-restart durability: the handle cell is carried across `production_reload` (`js_runtime/mod.rs:299`) and `wire_runtime_publishers` re-installs it on replacement lanes (`:534,586`), so no lane can come up with absent or stale authority. One service can never see another service's handle (asserted below).
    - Tests (all green):
      - `server::tests::two_servers_in_one_process_own_independent_agent_hosts` (`src/server/tests.rs`) — the plan's `two_servers_independent_agent_hosts` case: two `IpcServer`s in one process, each `runtime_generation` lane routes to its own handle and neither is handed the other's (identity via cfg(test) `AgentHostHandle::same_host`). This test cannot pass on the old code (first install won → one server silently used the other's host).
      - `server::js_runtime::tests::agent_host_wiring_reaches_every_lane_and_stays_per_service` — Trusted + ThirdParty × General + Latency lanes all carry the installed handle; a second, hostless service still fails closed (`agent_host()` → `Err`), i.e. no cross-service reach.
      - `server::js_runtime::tests::registrations_queued_hostless_hand_over_to_a_late_attached_host` — hostless declaration stays on the lane queue, moves exactly once into the host's own pending queue on attach, re-wiring does not duplicate it.
      - `server::js_runtime::tests::agent_facade_fails_closed_without_an_attached_host` (existing, comment updated) — unwired lane → typed `agent.unavailable`; no other test in the process can hand it a host (that possibility was the baseline failure).
      - `tests/agent_protocol.rs::agent_authority_is_server_state_not_a_process_global` (new) + the updated trusted-op guard — assert `src/server/agent.rs` contains no `AGENT_HOST_AUTHORITY`/`PENDING_PACKAGE_REGISTRATIONS`/`install_global`, that the op module resolves `agent_host()` from its lane and never a global or global queue, and that `ClayOpState` owns both the handle and the queue.
    - Determinism (the hazard this task closes, measured on the final code): `cargo test --lib` with default parallel threads **5 consecutive runs, 1421 passed / 0 failed / 1 ignored each** (25.4s, 16.5s, 15.7s, 19.1s, 30.0s; the suite takes ~15-30s in parallel) — the baseline had the declaration test failing in the full suite (2/2 runs). The polluting process-wide queue no longer exists, so a test can only see declarations it queued itself.
    - Boundary kept: `src/server/ops` and `src/server/js_runtime` still never name `AgentHost` (the wiki invariant for those modules) — they carry only `AgentHostHandle`; the test harnesses use cfg(test) `AgentHostHandle::inert()`/`host()`/`same_host()` accessors instead of constructing a host in the runtime modules.
    - `server::js_runtime::tests::lane_registration_queue_fails_closed_at_capacity` — the lane queue keeps the host queue's `PENDING_REGISTRATION_CAP` (64) ceiling: declarations at the ceiling queue, the next one is refused, none is dropped silently. This is the plan's `package_registration_without_host_fails_closed` case in its current shape (the unwired facade error `agent.unavailable` is covered by the existing test above; the queue's own error semantics are unchanged from the global queue's).
    - Related plan: `plans/135-Agent-Registration-Queue-Test-Isolation.md` (test-only isolation seam for the same flake, unstarted) is superseded by this task and now carries a supersession note pointing here.
    - Deviations from the planned file list: the registration queue lived in `src/server/ops/agent.rs` (not `ops/packages.rs`) and `ClayOpState` is constructed in `src/server/ops/mod.rs` (one state value cloned into every lane by `start_runtime_worker`, so `js_runtime/worker.rs` needed no change); `src/server/js_runtime/mod.rs` gained the service-level cell/wiring.
    - Test-only plumbing added with the route: `AgentHostHandle::same_host`, `ClayJsRuntimeService::test_lane_op_state` (cfg(test)), `drain_all_pending_registrations(service)` now reads its own lane queue instead of a process-wide drain, and the `src/server/tests.rs` scaffold wires an inert host so server-level tests own a host exactly like production.
    - Gates: root `cargo test` **exit 0** — lib 1422 tests (1421 pass, 1 ignored, 0 fail; +4 new guard tests over the baseline's 1418; the baseline-red `coding_agent_clean_init_one_line_activates_working_defaults` now passes in the full parallel run, closing task 1's recorded failure), integration 9 / 62 / 226 / 75 / 152 pass, 0 failures anywhere, 2 pre-existing ignores. `cargo fmt --all --check` **exit 0**; `cargo check --all-targets` **exit 0**; `cargo clippy --all-targets -- -D warnings` **exit 0** (the now-unused test drains were deleted, not `allow`ed). clay-agent untouched by this task and unaffected: `npm test` **exit 0** (180 pass / 1 skip).
    - Performance: the RPC path swaps a `OnceLock` read for one uncontended `std::sync::Mutex` read plus an `Arc` clone inside the already-borrowed lane `OpState` (no async lock, no await added); wiring drains with `try_lock` so it cannot block a lane. No new per-request allocation of note.
    - Docs: `docs/wiki/modules/clay-agent.md` (`clay:agent` section) no longer claims a process-global handle/`PENDING_PACKAGE_REGISTRATIONS`; it now documents the lane-injected handle and per-lane queue. Graft graph is a gitignored local cache and was refreshed with `graft build`.

- [x] Verify and fix the daemon resume workspace binding (A2-residual)
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
  - Evidence (2026-09-20):
    - Verification outcome: the flagged defect (`host.ts:2537`, pre-decomposition — recreating the live session with `workspaceRoot: process.cwd()` instead of reading the key written at `:1477`) was **already fixed** by plan 119 SC-6, and the fix survived the plan-130 decomposition. `ensureLive` reads `SESSION_SEARCH_WORKSPACE_METADATA_KEY` from the record metadata (`clay-agent/src/host/sessions.ts:383`) with `process.cwd()` only as the pre-workspace-record fallback, and that value reaches `createSession` → `sessionTools` → `buildSessionTools` → `buildCodingTools`, i.e. the tool cwd (`options.workspaceRoot`) and the acceptance roots (`createClayAcceptancePolicy({ roots: [options.workspaceRoot] })`); `session.resume` echoes it (`sessions.ts:351`).
    - Second finding, fixed here (the other half of the same binding): a resumed session never re-activated its live capabilities or graft binding. `ensureCapabilities` (MCP bridge + Obscura harness) and `ensureGraftBound` were called only from `sessionNew` (plus capabilities from `sessionSetAgent`), so after a daemon restart a resumed coding session came up with the right root but **no** MCP tools, no browser tools and no graft tools; an agent switch from a Chat profile to a coding one also never bound the workspace's graft.
    - Fix (one shared gate, not per-caller): `activateCodingSession(host, profile, agentRoot, allowList, workspaceRoot)` (`sessions.ts:47`) owns `profileWantsCodingTools` → `ensureCapabilities` + `ensureGraftBound`, and is called from all three paths that can start a coding session — `sessionNew` (`:77`), `ensureLive` (`:391`; the single funnel behind `session.resume`, `session.setAgent`, `session.prompt`, `run.resume`, context/mentions and tree reads), `sessionSetAgent` (`:291`). `ensureLive` now reads the agent's allow-list once (`host.mcpAllowListFor(agentRoot)`, `:386`) for the activation and for the live record/createSession options, replacing two inline reads. No new I/O: `ensureCapabilities` keeps its single lazy default-root promise and per-root skip, `ensureGraftBound` returns early once bound (or once attempted) for the root. `sessions.ts` is 540 lines after this task (task 2 recorded 455) and the longest function there is still under the 80-line budget.
    - Test: `clay-agent/src/__tests__/resume.test.ts:193` "resumed session binds its tools to the recorded workspace root, not the daemon cwd" (the plan's `resume_binds_recorded_workspace_not_cwd`; node:test names are sentences). Two hosts over one data dir: the first creates a coding session in temp root B and closes; the resuming host runs with `process.chdir(temp root A)` (a different real directory). The mock model calls a **relative** `write` (relative on purpose — the resolved cwd decides where it lands), driven through an emulated `document.*` reverse backend. Assertions: `session.resume` reports B; the resumed session's tool list contains `graft_ask` (which `graftTools` returns only when the bound root equals the queried one — i.e. the activation used B, not A); the `document.write` path is exactly `join(B, "resumed.txt")` and nothing under A. The resumed session's live record is non-autonomous (the task-3-flagged `fullAutonomy: false` write, now asserted explicitly in the test rather than implied), so the test toggles `session.setAutonomy` before prompting, as a user does, rather than driving an approval drill.
    - Falsification recorded (the guards are the closure evidence, and each was shown to fail without its fix): rewriting `ensureLive`'s root back to `process.cwd()` fails the test on the resume/write-root assertions; removing `activateCodingSession` from `ensureLive` fails it on the graft-tool assertion; the restored code passes both.
    - Gates: `npx tsc -p tsconfig.json --noEmit` **exit 0**; `npm test` **exit 0** — 182 tests (181 pass, 1 skip, 0 fail; one added case over the 181-test baseline). No Rust file changed by this task, so the root Linux gates carry task A1's run (`cargo test` **exit 0**, fmt/check/clippy clean); nothing in `clay-agent/src/__tests__/` was modified other than the one new case in `resume.test.ts`.
    - Docs: `docs/wiki/modules/clay-agent.md` ("A resume binds the workspace, not just the session") now states the daemon side (recorded key, cwd only for pre-workspace records, capability/graft re-activation).
    - Out of scope, recorded for follow-up: the **wiki** binding is an opt-in toggle with no persistence, so a restarted daemon loses an enabled wiki for every session (new ones included) — configuration persistence, not the resume binding (plan 139 territory). The restored-session autonomy reset (tool approvals re-arm after a restart) is the oddity flagged in task 3; unchanged here.

- [x] Execute and update the manual test plan (test-plan/)
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
  - Executed (2026-09-20). Files edited: `test-plan/16-agent-host.md` (new step **A25**, "Resume binding (workspace root, not the daemon cwd)", plus a Plan 130 execution record; stale `examples/init.js` paths and the "five exports" count corrected — `examples/config/init.js`, six exports incl. `agent.knowledgeSetOptions`), `test-plan/17-coding-agent-parity.md` (C43's expectation extended with the tool/capability re-activation and the new automated + live legs), `test-plan/index.md` ("Plan 130 agent-host decomposition execution record (2026-09-20)"), artifacts under `test-plan/artifacts/130-agent-host/` (README, `live-daemon.mjs` + logs, `gui/`, `gui-live.sh`, `probe.py`, hardened `portal-shot.py`, `automated-legs.txt`, `config-legs.txt`).
  - Evidence (2026-09-20):
    - Build: `cargo build --bins` 19:36, `cargo build --bins -p clay-desktop` 19:37, `npx tsc -p clay-agent/tsconfig.json` 19:37; `frontend/dist` reused unchanged (no frontend source newer than its 2026-09-18 build, `git status frontend/src` clean).
    - Live daemon protocol probe on the real build (two `clay-agent` processes, one data dir, different cwds; **24/24 legs PASS**): create with a recorded root, prompt to `agent_finished`, workspace-scoped `session.search` (hit + no-hit), `session.resumable` root scoping, `session.clone`/`session.fork`, `session.setAutonomy` both ways, `session.compact` with a persisted compaction entry, the documented `process.cwd()` fallback for a root-less session, an allow-listed stdio MCP server connecting (`environment.list` → `{"serverId":"live","connected":true,"tools":2}`) with its `mcp:live:` tool in the tool list, Obscura hidden, and after restarting in an unrelated cwd the resume leg: recorded root, graft skill, graft extension, MCP re-connect, system-prompt layers, the autonomy toggle on the resumed session, follow-up prompt. Falsification: with `ensureLive`'s activation removed, exactly the three re-activation legs fail (`live-daemon-falsify.log`).
    - Live GUI pass on the isolated real desktop build (`CLAY_AGENT_MOCK=1`, canonical example config, no input synthesis): `Agent lane` + `Message` + `Coding Agent Agent type` at rest; palette opened by AT-SPI action lists all daemon slash commands (`/branch /clone /compact /discard /fork /n /new /open-session /open-session-as-fork /resume /tree`, `server-first — @clay/coding-agent@0.1.0`); `Session` scope → 14 rows; lane hide/restore → 0/1 lane nodes. Server log shows the plan-130 A1 ownership path live (`[agent-reg] … host=live -> Ok({"queued": true})` — the lane's injected host resolved, then `[daemon] … applied`). Steps that need typing or a provider stay UNRESOLVED with the standing ceilings (no `/dev/uinput`, portal keyboard crash loop, no credentials in an isolated profile → `no provider configured · Settings · Providers`); each keeps its automated leg.
    - Regression suites on the crafted tree: `cargo test` 0 failures (lib 1421 pass / 1 ignored; presentation 62; protocol 226 incl. `agent_protocol::*` + `agent_session_isolation::*` — e.g. `real_daemon_serves_one_session_per_workspace`; runtime 75; security 152); fresh `clay-agent npm test` 182 tests (181 pass / 1 skip / 0 fail, incl. the A2 resume case with the MCP leg added); `frontend vitest run src/agent src/shell` 11 files / 125 tests.
    - Security/root-gating re-run: the root/acceptance legs are covered by the acceptance-policy suite and by the resume probe's recorded-root assertions (relative write resolves under the recorded root) in `clay-agent npm test`.
    - Findings recorded instead of hidden: (0) **autonomy**: the pass found module 16 A5–A7 asserting the stale "approvals on by default (decision 2157)" expectation. Creation is opt-out (`clay-agent/src/host/sessions.ts`: `params.fullAutonomy !== false`, user decision 2026-09-05; `session.setAutonomy toggles full autonomy; default stays true`), while `docs/reference/clay-js-api/agent/set-full-autonomy.md` and `api-inventory.toml` still document `default:boolean=false`; and a resumed session starts non-autonomous because `ensureLive` writes the live record with `fullAutonomy: false` while the session itself is created from the recorded value (task-3 oddity, now pinned by `resumed session binds its tools to the recorded workspace root, not the daemon cwd`). Steps corrected, divergence recorded, no behavior changed here — a doc-or-code decision is needed; (1) portal screenshots are unusable for an isolated client on this host (portal surface = visible workspace only), and the plan-126/129 `portal-shot.py` copy cropped an unrelated desktop window — that image was deleted before entering the repo, this directory's copy is hardened to fail closed without writing a PNG, and the follow-up to replace the older copies is recorded in the index; (2) module 16's step text named the removed `examples/init.js` — corrected; (3) two load-sensitive flakes seen once each (`server::config_watch::watcher_detects_new_and_deleted_watched_files` under three-suite parallelism; `clay-agent` resumable-ordering timestamp tie), both green standalone and in the serial rerun.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
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
  - Evidence (2026-09-20): **no JS API change** — verified by diff, not by assertion. Full run record in `test-plan/artifacts/130-agent-host/js-api-verification.txt`.
    - Three layers diffed against `HEAD` (66648e3, pre-decomposition) with the decomposed work tree: (a) the daemon contract behind `clay:agent` — the dotted method-id set and the `handle()`/command dispatch `case` labels are **identical** (50 ids, 51 cases, zero added, zero removed; the 12 "missing" command cases in `host.ts` alone are the ones that moved to `host/commands.ts`); (b) the Rust wrappers — the 11 `op_clay_agent_*` ops and every method string they send are unchanged (`src/server/ops/agent.rs` only gained the `Rc<RefCell<OpState>>` threading from A1); (c) the public JS surface — `runtime/js/agent.js` + `.d.ts` diff **0 lines**, all 11 `docs/reference/clay-js-api/agent/*.md` pages untouched, and the 11 `agent.*` entries in `docs/generated/clay-js-api-registry.json` are deep-equal to HEAD (the registry's other modifications are the unrelated completion/language work already in the tree; `api-inventory.toml` gained no `agent.` line).
    - Doc-guard suites: `cargo test` **exit 0** — lib 1421 pass / 1 ignored, main 9, presentation 62 (`package_ui_conformance`), protocol 226, runtime 75, security 152. Inside protocol: the 74 `clay_js_api_inventory` + `clay_js_doc_registry` + `clay_js_facade_layout` cases and the 14 `documentation_coverage` cases — including `generated_registry_is_current`, `clay_js_api_inventory_unchanged_or_documented`, `plan109_coding_agent_client_command_docs_and_internal_rpc_boundary` (no `runtime/js` facade may expose `session.context`, `environment.list`, `omActivity`, …), `agent_configuration_options_are_documented_custom_properties_with_decision_defaults`, and security's `rust_visibility_api_mapping` (internal agent/runtime helpers must not acquire a public facade). Non-mutating: md5 of `api-inventory.toml` and the generated registry identical before/after the suites.
    - Update command stays a no-op on a current tree: `cargo run --bin update-doc-registry` rewrote `docs/generated/clay-js-api-registry.json` **byte-identically** (md5 checked).
    - One doc file did change, forced by the guard rather than by a surface change: `documentation_coverage::parity_ledger_covers_every_manual_step_public_api_and_protocol_family` failed after task A2's new step landed (`test-plan/16-agent-host.md: ledger covers 24 of 25 steps; missing: ["A25"]`), so `docs/development/tauri-react-parity-ledger.json` now lists A25 under `agent.host.phase1`, adds `clay-agent/src/host/*.ts` to that capability's owner list, and records this plan's manual pass. The task's "Files to Create/Edit: none expected" held for the API surface itself.
    - Carried finding (documentation-vs-code, not a surface change): that same guard pins `default:boolean=false` on `agent.setFullAutonomy` ("decision 2157") while the daemon has been opt-out since the 2026-09-05 user decision (`clay-agent/src/host/sessions.ts`: `params.fullAutonomy !== false`). Recorded in the manual-test pass; needs a fix-or-re-document decision.

- [x] Update or verify the code wiki after implementation
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
  - Evidence (2026-09-20): wiki updated after the suites passed, and pinned by a new deterministic guard so it cannot drift back.
    - `docs/wiki/modules/clay-agent.md` — Source now lists `clay-agent/src/host/*.ts`; the Overview states the daemon half is decomposed the same way as the server half; new evergreen section **Host module map (plan 130)** with the 12-module table (`internals`, `skills`, `context-mentions`, `providers`, `integrations`, `agent-roots`, `om`, `session-build`, `session-tree`, `sessions`, `session-run`, `commands`), the three contract rules (class is the contract, members `readonly` but shared; type-only class imports so there is no runtime cycle, `internals.ts` as the leaf; one delegate per moved method plus the 80-line helper budget) and the statement that no public surface changed; Invariants gained the budget + unchanged-method-set bullet. The A1/A2 ownership and resume text added by the earlier tasks stays in the `clay:agent` surface section.
    - `docs/wiki/modules/agent-process-manager.md` (the server agent page) — Overview states the server owns the `AgentHostHandle` and injects it per lane (two servers in one process route to their own hosts); new plan-130 A1 paragraph in **How It Works** covering the wiring path (`RuntimeGenerationStore::initial` → `ClayJsRuntimeService::set_agent_host` → `wire_runtime_publishers` → `ClayOpState::set_agent_host`, replacement workers included, idempotent hand-over via `queue_registration_now`), the fail-closed hostless lane (`agent.unavailable: no agent host is attached to this runtime`), the per-lane bounded registration queue (`PENDING_REGISTRATION_CAP` 64) and the host's own pending queue drained after the first `initialize`; Invariants now state the three ownership rules (no process-global authority; ops/js_runtime name the handle, not `AgentHost`; registrations queue per lane and never across servers); Tests list the two `agent_protocol` guards plus the new wiki guard; Related links the lane page.
    - `docs/wiki/modules/embedded-js-runtime.md` — the reload/wiring paragraph now names `wire_runtime_publishers` as the single per-lane host-channel wiring point and the agent handle as one of those channels, pointing at the agent page.
    - `docs/wiki/index.md` — both agent entries now name the plan-130 host module map / injected ownership.
    - Guard: `tests/documentation_coverage.rs::plan130_wiki_pages_describe_host_modules_and_injected_ownership` (13 markers across the two pages + index links to both) — `cargo test --test protocol -- plan130_wiki`. It is the drift guard for exactly the claims the acceptance criteria list, and it fails if a page loses the module map, the injection, the fail-closed path, or the queue bound.
    - Citation audit instead of blind edits: all 11 `docs/reference/clay-js-api/agent/*.md` pages name `clay-agent/src/host.ts::<fn>` as their backing path; each named function still exists there as the thin delegate (checked symbol by symbol), so the citations stay true and were left alone. One wiki citation that had genuinely moved was corrected (`labelFirstPromptStore` → `clay-agent/src/host/internals.ts`). No wiki page still claims a process-global authority or queue (`AGENT_HOST_AUTHORITY`, `PENDING_PACKAGE_REGISTRATIONS`, `install_global`, `queue_package_registration` appear nowhere outside the removal note).
    - Parity ledger: `agent.codingAgent.parity` now also owns `clay-agent/src/host/*.ts`, matching the `agent.host.phase1` row fixed in the Clay JS API task.
    - Gates: `cargo test` **exit 0** — lib 1421 pass / 1 ignored, main 9, presentation 62, protocol **227** (226 + the new guard), runtime 75, security 152; `cargo fmt --check` clean (the new test was formatted), `cargo clippy --all-targets -- -D warnings` clean. Wiki pages are Markdown, so no TypeScript or `clay-agent` change was needed.

## Compromises Made
- The decomposition kept the `ClayAgentHost` class as the runtime contract instead of an `HostInternals` interface: the class is what the 29 `clay-agent` test files and the RPC surface already speak, and inventing a second contract would have been churn with no reader. Cost: members shared across modules are `readonly` rather than `private` — the compiler still stops writes, but not reads. Revisit only if the module split grows a second consumer.
- The JS API task was verify-only by design (no facade changed); the one *behavioral* finding it surfaced — the `agent.setFullAutonomy` `default:boolean=false` docs/inventory claim versus the opt-out daemon default — was decided by the user in the follow-up above (docs moved to the code, resume restores the recorded value), so it is no longer an open compromise.
- Accepted risk from that decision, recorded in the log and the reference page: with autonomy on by default, out-of-workspace writes, delete/move, and shell-metacharacter commands run without a prompt in every session unless the caller blocks autonomy, including sessions created by the coding-agent panel (which sends no `full_autonomy`). The panel has no autonomy toggle yet.

## Further Actions
- **[Done 2026-09-20] Autonomy default: decided doc-or-code.** User decision: "Default should be autonomy. User can configure to block autonomy" — logged as `decision-logs/2026-09-20-2049-agent-autonomy-default-on-and-resume-restores-recorded-autonomy.md` (supersedes the "off by default" part of 2157; 2157's workspace-free/host-gated acceptance policy stands for sessions with autonomy off). Code: `ensureLive` now restores the recorded autonomy instead of hardcoding `fullAutonomy: false` into the live record, so a block survives a daemon restart. Docs: reference page (`default:boolean=true` + plain-language security note), `api-inventory.toml`, regenerated `docs/generated/clay-js-api-registry.json`, `examples/config/init.js`, the code wiki, the DTO doc comment in `src/protocol/agent.rs`, and the code comment sweep. Gates: the inventory test now pins the documented default **and** the daemon expression (`params.fullAutonomy !== false`, `metadata.fullAutonomy !== false`), the resume suite gained `resumed session keeps a recorded autonomy block (approvals stay armed)` (falsified: reintroducing the hardcoded `false` fails `resumed session binds its tools to the recorded workspace root…` on the autonomy assertion), and `test-plan/16-agent-host.md` A5–A7 + the coverage row now state the resolved policy.
- Original item text, kept for context: "Autonomy default: decide doc-or-code** (highest priority; blocks nothing else). The daemon creates sessions opt-out (`params.fullAutonomy !== false`, user decision 2026-09-05) while `docs/reference/clay-js-api/agent/set-full-autonomy.md`, `api-inventory.toml` and `agent_configuration_options_are_documented_custom_properties_with_decision_defaults` still assert `default:boolean=false` (decision 2157). Also decide whether a resumed session should come up non-autonomous (`ensureLive` writes its live record with `fullAutonomy: false` while the session itself is created from the recorded value) — recorded in the manual-test pass and pinned by the resume case.
- **Replace the older `portal-shot.py` copies** (follow-up from the manual pass): plan-126/129 artifacts still trust compositor bounds alone; the hardened fail-closed copy lives in this plan's artifact directory. One implementation under `scripts/` would remove the copy-forward trap.
- **Wiki drift check** (small automation win): a test that fails when a per-plan wiki guard marker disappears is now in place for plan 130; the general version would be a check that evergreen wiki pages do not cite source paths/symbols that no longer exist (the `labelFirstPromptStore` citation was found by hand, not by a gate).
- Not this plan: the wiki/graft knowledge binding still has no configuration persistence, so a restarted daemon loses an enabled wiki (recorded in the manual-test pass; plan 139 territory).
