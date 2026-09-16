# Prism 0.7 Work-Scope Observational Memory (Phase 7 Shape)

Depends on: `plans/120-Adopt-Prism-0.7.0-Pin-And-Event-Mapper.md`. Nested
child scopes need `plans/122-Prism-0.7-Host-Owned-Spawn-Agents.md`; if 122
is not done, ship per-prompt scopes only.

This plan is **Cut 4**: host-selected OM working sets via
`createWorkScopeController` / `withWorkScope`. It is the Phase 7 *shape*
(per-run projection, exact-id `recall` still sees everything). It is not
the Phase 7 product (Memory tab live activity, `om:status`/`om:view` UI,
fabric notes, per-task orchestrator cadence).

Scope note: **no UI**. No Memory tab changes, no prototype gate. No Clay
JS package. Skip primitive-review, package-runtime, default-loading,
grammar, external-process, package UI/authoring, example-config, live
launch-test. Clay JS API and configuration tasks are verification-only.

Context7 has no `@arnilo/prism`. Authoritative APIs: local Prism 0.7.0.

Not in scope: `@arnilo/prism-memory/fabric`, vector/working stores Clay
does not run, Memory tab, auto-compact wiring, workflow `/start`.

## Objectives

- When observational memory is attached, wrap each coding `session.prompt`
  in `withWorkScope` (`kind: "run"`, host id derived from `runId` /
  session id). Chat / OM-off sessions unchanged.
- If spawn (122) is present, open a child scope (`parentId` = current run
  scope) around each `test`/`validation` delegation. If 122 is absent,
  skip this bullet.
- While any host scope exists, rely on Prism to skip the observation
  dropper; folded-payload byte cap remains a storage cap. Exact-id
  `recall` still reads the full branch.
- Caps fail closed (256 scopes, depth 8). Invalid ids fail closed. Labels
  go through the existing secret redactor.
- No fabric. No new RPC.

## Expected Outcome

- OM-on coding prompts append `om.scope.*` entries to the existing OM
  ledger (same sqlite session store). OM-off sessions write none.
- `projectWorkMemory` for the current run scope returns that run’s
  observations/reflections, not the whole session pool.
- `recall` by exact 12-hex id still returns pre-scope observations.
- `cd clay-agent && npm test` and Linux cargo gates pass.
- Wiki states: scopes are host-owned; fabric not attached; UI unchanged.

## Tasks

- [x] Attach a per-prompt work-scope on OM coding runs
  - Acceptance Criteria:
    - Functional: OM-attached coding `sessionPrompt` opens a scope, runs
      `stream()` inside `withWorkScope`, and leaves/closes on settle
      (success, abort, throw). Scope ids match
      `[A-Za-z0-9._:/-]{1,128}` with no `..`. Chat / `observationalMemory:
      false` never constructs a controller.
    - Performance: Scope entries are small ledger appends, not extra
      provider turns. No second sqlite database.
    - Code Quality: Import from
      `@arnilo/prism-memory/compaction/observational-memory`. Reuse the
      existing OM `appendEntry` / session. Do not fork the ledger.
    - Security: Labels/kinds pass `secrets` into
      `createWorkScopeController`. No new network. Tenant stays the
      clay-agent tenant.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/migrate-to-0.7.md` §20
      - `/home/arn/Projects/prism/docs/compaction-observational-memory.md`
        (`createWorkScopeController`, `withWorkScope`, caps)
      - `clay-agent/src/host.ts` OM attach / `sessionPrompt`
      - `.agents/skills/clay-execution/references/packages.md` (Agent Host)
    - Options Considered:
      - Wait for Phase 5 task ids: no orchestrator yet; per-prompt is
        the only real unit.
      - Fabric `remember({ kind: "fact" })` on close: extra store Clay
        does not run.
      - Per-prompt `withWorkScope` on OM coding only: selected.
    - Chosen Approach:
      ```ts
      import {
        createWorkScopeController,
        withWorkScope,
      } from "@arnilo/prism-memory/compaction/observational-memory";

      const scopes = createWorkScopeController({
        session: attached.session,
        appendEntry: (entry, options) => this.persistence.append(entry, options),
        secrets: [...this.secrets],
      });
      await withWorkScope(scopes, { id: `run:${sessionId}`, kind: "run" }, () =>
        live.session.stream(promptInput, streamOptions),
      );
      ```
      Use a stable per-prompt id (session + increment or Prism `runId`
      once assigned). If `runId` is only known after start, open the
      scope with `run:${sessionId}:${n}` from a per-session counter.
    - Files to Create/Edit:
      - `clay-agent/src/host.ts` (`attachOm` / `sessionPrompt`)
      - `clay-agent/src/__tests__/` OM test file (extend existing OM
        tests if one exists; else small new file)
    - References:
      - `clay-agent/src/compaction.ts`
      - `roadmap.md` Phase 7
  - Test Cases to Write:
    - OM-on coding prompt writes `om.scope.open` / leave/close entries.
    - OM-off: zero `om.scope.*` entries.
    - Invalid id (`../x`) fails closed, prompt does not start.
    - After two prompts, `projectWorkMemory(..., { from: lastRun })`
      does not include the first run’s observations; `recall` of an
      earlier id still works.

- [x] Nest spawn-child scopes when plan 122 is present
  - Acceptance Criteria:
    - Functional: If spawn tools exist, each `test`/`validation`
      delegation opens a child scope with `parentId` equal to the
      current run scope and `kind: "child"`. Depth stays ≤ 8. If 122
      is not merged, this task is skipped and recorded as skipped in
      completion evidence (not a silent pass).
    - Performance: One extra ledger append pair per spawn, not a nested
      daemon.
    - Code Quality: Same controller instance as the parent run when
      possible; do not create a second OM attach.
    - Security: Child scope ids cannot escape the charset. Child cannot
      bind another session’s records.
  - Approach:
    - Documentation Reviewed:
      - `/home/arn/Projects/prism/docs/compaction-observational-memory.md`
        parent/child caps
      - `plans/122-Prism-0.7-Host-Owned-Spawn-Agents.md`
    - Options Considered:
      - Always skip child scopes: loses the only nested unit we have.
      - Nest when supervisor is live: selected.
    - Chosen Approach:
      ```ts
      await withWorkScope(
        scopes,
        { id: `child:${childId}:${delegationId}`, parentId: runScopeId, kind: "child" },
        () => childRun(),
      );
      ```
    - Files to Create/Edit:
      - `clay-agent/src/host.ts` child factory
      - `clay-agent/src/__tests__/spawn-agent.test.ts` or OM tests
    - References:
      - plan 122 `createChildAgent`
  - Test Cases to Write:
    - Spawn `test` under an OM parent: ledger has parent run scope +
      child scope with matching `parentId`.
    - Skip evidence if 122 absent.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: No new Clay JS API. Scopes are daemon-side OM
      ledger entries. `recall` stays exact-id (existing).
    - Performance: Verification adds no runtime work.
    - Code Quality: No `clay:memory.scopeOpen` export.
    - Security: No package API to bind/unbind another session’s ids.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - Public `om.scope.*` JS API: rejected (packages cannot touch the
        daemon ledger).
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
    - Functional: No new `init.js` scope option. OM remains
      profile / `session.new observationalMemory`.
    - Performance: No config reload change.
    - Code Quality: Caps stay Prism’s, not Clay settings.
    - Security: Config still cannot grant OM on Chat by surprise
      (existing fail-closed).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - `om.setOptions({ scopes: true })`: extra knob, default should
        just work when OM is on.
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
      - `examples/config/init.js`
  - Test Cases to Write:
    - None.

- [x] Execute and update the manual test plan (`test-plan/`)
  - Acceptance Criteria:
    - Functional: Record this cut as **automated-only** in modules 16/17:
      no new chrome; Memory tab still shows existing OM activity if it
      already did, and is not required to show scope outlines. Do not
      weaken existing OM steps.
    - Performance: No new live-launch gate.
    - Code Quality: Coverage matrix notes plan 123 as automated.
    - Security: No change to recall-id rules in manual steps.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/16-agent-host.md`, `test-plan/index.md`
      - `.agents/skills/create-plan/references/clay.md`
    - Options Considered:
      - Memory tab scope outline UI: out of scope.
      - Automated-only note: selected.
    - Chosen Approach:
      - Matrix row: plan 123 host scopes, no UI.
    - API Notes and Examples:
      ```text
      test-plan/index.md — plan 123 automated-only (OM ledger scopes)
      ```
    - Files to Create/Edit:
      - `test-plan/16-agent-host.md`
      - `test-plan/17-coding-agent-parity.md`
      - `test-plan/index.md`
    - References:
      - plan 120 automated-only precedent
  - Test Cases to Write:
    - None beyond the matrix note.

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
      - `docs/wiki/modules/clay-agent.md`: per-prompt scopes, optional
        child nest, dropper skip, recall still exact-id, no fabric.
    - Files to Create/Edit:
      - `docs/wiki/index.md`
      - `docs/wiki/modules/clay-agent.md`
  - Test Cases to Write:
    - Manual wiki review: index links; pages match shipped behavior.

## Compromises Made

- Scope unit is a prompt, not a Phase 5 workflow task. Re-bind when
  `/start` exists.
- Fabric not attached. Typed `fact`/`procedure` notes wait until Clay
  runs those stores.
- No Memory tab outline of scopes.
- Child scopes skipped if 122 is not merged.

## Further Actions

- Phase 5/7: one scope per orchestrator task; Memory tab outline;
  `om:status` / `om:view` if those commands are not already enough.
- Fabric attach only if something needs typed notes beyond OM.
- Attention `compactRatio` trigger (plan 121 Further Actions) still
  independent of scopes.

## Execution Record (Linux, 2026-09-16) — task 1

Gates: `cd clay-agent && npm test` 179 passed / 1 pre-existing skip
(`obscura-web-surface.test.ts`, gated on `CLAY_WEB_E2E`), 180 total — four
new `om.test.ts` cases (8/8 in that file); `cargo fmt --check`,
`cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`
clean (no Rust touched).

Implementation notes:

- `omRunScope` (host.ts) is the only scope surface: OM attached AND
  `hasCodingTools(live.tools)`. Chat, OM-off, and non-coding OM profiles
  never build a controller (the existing `Om` activity drill keeps its
  dropper rows).
- Ids are host-generated `run:<sessionId>:<n>`; the per-session counter
  (`LiveSession.omRunCount`) is seeded lazily by counting existing
  `om.scope.opened` entries with that prefix, so a resumed session never
  reopens an id. Once seeded, it increments synchronously; the first lazy
  ledger read is intentionally unserialized (Clay keeps Prism's default
  `toolConcurrency` of 1; see task 2's ceiling for the upgrade path).
- `withWorkScope(controller, spec, runStream)` wraps creation AND draining of
  `live.session.stream(...)`. A rejected scope id (e.g. a session id
  containing `..`) throws before Prism's run-depth proxy is entered, leaving
  no stranded run depth; Prism's stream `finally` flush runs with the scope
  still entered, so auto-bind lands on the run scope.
- The controller reuses the existing OM `appendEntry`
  (`this.persistence.append`) and the OM-attached proxy session, with the
  host redactor's secrets. No second sqlite database, no provider turn, no
  new RPC / JS API / config surface.
- Tests use `session.load` for the ledger and assert: opened/entered/left
  (never closed), `om.scope.bound` on the run scope, zero scope entries when
  OM is off, `a..b` session id fails closed with no message entry, and two
  prompts give disjoint `projectWorkMemory` sets while
  `recallObservationalMemory` still returns run 1's exact id.

Boundaries observed (not task 1 scope): with 256 scopes already on a branch,
`open` fails closed and the prompt is refused — Prism's cap, per this plan's
caps criterion. `run.resume` streams keep their pre-existing path (the host
calls Prism's base `resumeAgentRunStream`, so no OM flush rides a resume).

### task 2 — spawn-child scopes (plan 122 present, not skipped)

Shipped in `clay-agent/src/host.ts` + `om.test.ts` (new case: 180 pass / 1
pre-existing skip, 181 total; `tsc` clean; `cargo fmt --check` + `clippy`
clean — no Rust touched).

- `omSpawnScopes(sessionId)` is passed to `createSupervisor` as
  `hooks.before/after`: `before` opens `child:<sessionId>:<n>` with
  `parentId = await controller.leaf()` and `kind: "child"`, `after` closes
  it. No-op when OM is off or `leaf()` is the session root (no scoped run).
- Pattern deviation, deliberate: the plan sketched
  `withWorkScope(..., () => childRun())` (open + enter + leave). Task 2 uses
  open/close only. The ledger's enter/leave stack is session-global, not
  async-local: children spawned async keep running while the parent turn is
  still open, so a child entering would make the parent's own flush
  (`leaf()`) bind the parent run's observations to the child scope. Prism
  has no per-context scope stack in 0.7.0, so hooking open/close is the only
  shape that cannot mis-bind. It also matches the plan's performance line
  (one append pair per spawn) exactly.
- Id deviation, deliberate: the sketch's `child:${childId}:${delegationId}`
  is not ledger-safe — Prism's delegation sequence restarts whenever the
  supervisor is rebuilt (every model switch / `session.om.set`), so a later
  spawn would reissue `supervisor-1` and the `has` guard would reuse the
  already-closed scope. Child ids use the same ledger-seeded per-branch
  counter as run scopes (`LiveSession.omChildCount`, seeded from
  `om.scope.opened` with the `child:<sessionId>:` prefix); the delegation's
  `childId` is kept as the scope `label` so `test`/`validation` stays
  readable. Charset exposure is identical to run ids (Prism validates, `..`
  fails closed).
- Same controller instance: `omScopes(live)` caches one controller per OM
  session, keyed on the session object (a model switch swaps in a fresh OM
  proxy), so the run scope and its children share it and no second OM attach
  or sqlite handle is created. `sessionPrompt` now uses the same helper.
- Depth: child scope is 2 hops from the session root (session → run →
  child), inside Prism's 8 cap; `open` is the enforcement point, so a
  deeper nesting would fail the spawn closed rather than write a shallow
  scope. A failed open surfaces as a delegation error/tool error (Prism
  fails closed), never a silently orphaned child run.
- Idempotency: `before` re-runs when a suspended child resumes (Prism
  contract), guarded by a per-supervisor `Map<delegationId, scopeId>`; the
  terminal `after` closes once and swallows a lost scope (advisory ledger
  bookkeeping must not fail a delegation, which Prism would report as a
  `delegation_error`).
- Test coverage: one prompt delegates to `test`; ledger has
  `run:<sid>:1` (entered) + `child:<sid>:1` (`parentId` = the run scope,
  `label` `test`, one close, never entered, status closed), depth 2. The
  child runs its own provider turn through the existing mock, so the test
  fails if the hook never opens the scope or the delegation never happens.
- Scope-id seeding is lazy and unserialized, same shape as task 1's run
  counter: a collision needs two allocations before the first ledger read
  resolves, which needs parallel dispatch. Clay does not set Prism's loop
  `toolConcurrency` (default 1) and a session takes one prompt at a time, so
  it is not reachable today; the `ponytail:` note on `omRunCount` names the
  upgrade path (promise-chain the allocations) if that ever changes.

### task 3 — Clay JS API surface verification (no new API)

Verification-only task complete. Plan 123 adds daemon-side `om.scope.*` ledger
entries only; it does not add a Clay JS/TS facade, Rust public function, op,
configuration option, RPC, or package surface.

- Diff review: task 123's implementation is limited to private
  `ClayAgentHost` scope helpers and OM tests. No `runtime/js/*.js` or `.d.ts`,
  `src/server/facades.rs`, `docs/index.md`, API Markdown, inventory, or
  generated-registry change is needed for scopes. `omRunScope`, `omScopes`,
  and `omSpawnScopes` are private; `createWorkScopeController` is used only by
  the daemon host.
- Public-boundary scan: no `clay:memory` module, `memory.scope*`,
  `clay:memory.scopeOpen`, or work-scope export exists outside the daemon
  implementation. `runtime/js/mod.ts` exposes no memory module; the facade
  table remains 24 modules (14 public to third-party code). Existing exact-id
  recall behavior remains on the daemon/OM path and was not widened to a JS
  API or cross-session identifier binding.
- Documentation/registry verification: `docs/index.md` remains the explicit
  source list; inventory, Markdown metadata, facade exports, naming, security
  fields, master-index links, and generated registry all match. No
  `cargo run --bin update-doc-registry` rewrite was required because no public
  API documentation changed.
- Checks: `cargo test --test protocol clay_js_doc_registry::` (51 passed),
  `cargo test --test protocol clay_js_api_inventory::` (13 passed),
  `cargo test --test protocol clay_js_facade_layout::` (6 passed),
  `cargo test --test security
  rust_visibility_api_mapping::third_party_facade_allowlist_exactly_matches_plan_public_inventory`
  (1 passed), `cargo test --lib js_runtime` (220 passed / 1 ignored),
  scope-export scan clean, and `git diff --check` clean.

### task 4 — Clay configuration API verification (no new scope option)

Verification-only task complete. No source or configuration changes were needed
for this task. Work-scope activation remains host-owned: `session.new` and the
registered profile resolve `observationalMemory`; `clay:configuration` exposes
no OM or work-scope setter.

- Canonical example verified at `examples/config/init.js` (the task's
  `examples/init.js` shorthand points at this repository path). It contains no
  `observationalMemory`, `workScope`, `om.setOptions`, scope allocator, or
  work-scope-cap option. Existing keybinding `scope` fields are unrelated.
- The exact `clay:configuration` surface remains six exports: runtime-backed
  `loadConfigurationModule`, `getConfigurationState`, and `setPackageOption`,
  plus three explicit planned/unavailable stubs (`setModePreference`,
  `setDecorationTheme`, `setParsePolicy`). `setPackageOption` keeps its closed
  typed allowlist; Prism's work-scope caps remain implementation policy, not
  Clay configuration.
- Security and reload behavior remain unchanged: configuration is trusted-only,
  internal authority/resource controls are rejected, and configuration cannot
  turn a Chat profile into an OM coding run. Existing configuration reload and
  canonical-example tests remain green.
- Checks: `cargo test --lib configuration` (61 passed),
  `cargo test --test protocol clay_js_api_inventory::configuration_surface_is_closed_and_security_controls_are_not_properties`
  (1 passed), `cargo test --test protocol
  clay_js_doc_registry::generated_registry_configuration_security_denies_implicit_external_authority`
  (1 passed), `cargo test --test protocol
  clay_js_doc_registry::configuration_api_no_authority_grant` (1 passed),
  `cargo test --test protocol
  clay_js_facade_layout::clay_js_facade_modules_exist_with_expected_exports`
  (1 passed), no-OM-config scan clean, `cargo fmt --check`,
  `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`,
  and `git diff --check` clean.

### task 5 — manual test plan (automated-only, no new chrome)

Executed as an automated-only cut, following the plan 120 precedent. Plan 123
adds daemon-side `om.scope.*` ledger entries only, so there is no new
interactive step and no live GUI pass is claimed. No existing module 16/17
step was deleted, weakened, or re-scoped.

- Module 16 (`test-plan/16-agent-host.md`) gains a "Plan 123 work-scope
  record": the A11 recall rule (exact-id only, no auto-injected memory text)
  is explicitly unchanged; the Memory tab keeps rendering the existing OM
  activity and is not required to show a scope outline. Rows pin the
  OM-on/open-enter-leave, OM-off zero-entry, invalid-id fail-closed,
  scoped-projection-vs-recall, and delegation child-scope tests.
- Module 17 (`test-plan/17-coding-agent-parity.md`) gains a short Plan 123
  note: the C53–C55 spawn expectations (catalog, lifecycle rows, cancel,
  fail-closed handles) still hold; scopes render nothing new and recall stays
  exact-id only. Git-worktree isolation remains out of coverage per plan 122.
- `test-plan/index.md` gains the "Plan 123 manual-test-plan execution record"
  section and a coverage-matrix row marking the cut automated-only
  (16 work-scope record, 17 existing agent steps unchanged).
- Checks: fresh `cd clay-agent && npm test` 180 pass / 0 fail / 1
  pre-existing skip (181 total; the skip is the `CLAY_WEB_E2E`-gated
  `obscura-web-surface.test.ts` step); `git diff --check` clean. No new
  live-launch gate was added.

### task 6 — code wiki after implementation

Done. `docs/wiki/modules/clay-agent.md` gains a "Plan 123 additions
(per-prompt OM work-scopes)" section plus invariant/test updates;
`docs/wiki/index.md` extends the clay-agent entry with the plan 123 summary.
The page states the shipped behavior, not the plan sketch.

- Run scope: `sessionPrompt` builds `run:<sessionId>:<n>` (kind `run`) only
  for `live.observationalMemory && hasCodingTools(live.tools)` and wraps the
  full stream in `withWorkScope`; the stream is constructed inside the scope
  closure so a rejected id fails closed before Prism's run-depth proxy, and
  `finally` leaves on success/abort/throw (never closes).
- Controller/ids: one memoized `createWorkScopeController` per live session
  (keyed on the session object, shared with spawn children, `secrets` wired);
  `omRunCount`/`omChildCount` lazily seed by counting `om.scope.opened`
  entries so ids stay monotonic across daemon restarts, with the
  unserialized-allocation ceiling and its `ponytail:` upgrade path recorded.
- Projection: Prism binds each run's new observation/reflection refs to its
  leaf scope; the injected `observational-memory` block projects
  `from = leaf, include: "self+ancestors", closed: "hide"`, so sibling run
  scopes never leak into a later prompt. Exact-id `recall` still reads the
  whole branch and the Memory tab reads the ledger directly.
- Child nest: `sessionSupervisor` hooks open `child:<sessionId>:<n>` under
  `scopes.leaf()` (skipped at the session root) and close it once; children
  are never entered because Prism's enter/leave stack is session-global.
- Caps/tradeoffs documented: 256 host scopes / depth 8 / stack 8 / 4096 binds
  fail closed (256-prompt ceiling accepted), dropper skipped while host scopes
  exist, no fabric, no new RPC/JS/config/UI.
- Checks: `cargo test --test protocol documentation_coverage::` (11 passed,
  including `wiki_navigation_is_complete_and_current_page_paths_resolve`) and
  `cargo test --test protocol primitives_docs::` (34 passed);
  `git diff --check` clean.
