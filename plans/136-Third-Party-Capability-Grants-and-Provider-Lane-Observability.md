# Plan 136 — Third-Party Capability Grants and Provider Lane Observability

Source: plan 127 → `## Further Actions` (items 1, 4, 5). Plan 127 gave every
runtime domain a general and a latency lane, bounded the command mailbox, and
restored the heap limit, but its further actions recorded that the lane work is
**not reachable by any non-bundled package**: `PackageService::authorize_package`
records capability grants with no user-facing entry point, so an adopted
third-party package can never be enabled with `parse-document` or
`completion-provider` and the latency lane can only be exercised by bundled
providers. This plan closes that gap, then makes lane occupancy measurable with
real third-party providers. It also hardens the Clay JS option-surface guard
against the class of documentation drift plan 127 found by hand (item 4).

Binding prior decisions:

- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`:
  one authority model; users grant Clay-defined capabilities to any package
  source; grants are recorded, visible, revocable, and deny-by-default.
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`: two
  trust domains, compiled bundled classification, typed inert cross-domain
  values, no V8 object crossing.
- `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`:
  Clay does not implement its own package manager or registry.
- `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`:
  explicit `loadPackage("…")` from `~/.clay/init.js`; no silent defaults.
- Plan 035 (`Unified User-Authorized Package Authority`) and plan 115 (Phase 3
  CLI verbs) own the existing authority model and lifecycle verbs this plan
  extends.

Roadmap position: this is Phase 3 follow-through that Phase 4
(`roadmap.md` → "Third-Party Workflow/Command Package Platform") depends on,
because Phase 4's exit gate requires contributions that "activate under
trust/permission policy **with user approval**".

## Objectives

- Promote the already-inventoried `packages.authorize` API from `planned` to
  implemented: a `deno_core` op (`op_clay_packages_authorize`), a real
  `clay:packages` facade, typed errors, and the missing
  `docs/reference/clay-js-api/packages/authorize.md` reference page — so the
  documented grant surface exists instead of a `plannedPackageApi` stub
  (`runtime/js/packages.js:66`).
- Add the host CLI grant surface (`clay package authorize <name> …` plus grant
  display in `clay package inspect`) so a user can enable an adopted third-party
  package with the capabilities its manifest declares, without hidden config
  keys and without leaving the current trust-lifecycle verb family.
- Prove the end-to-end third-party path with a fixture package: grant →
  enable → JS provider registration → module-backed provider served on the
  plan-127 **latency lane** while the general lane is busy, with the
  no-grant path still failing closed (`MissingCapabilityGrant`).
- Make provider lane occupancy measurable (`js_runtime.lane.*` counters), take
  a real measurement with the fixture, and record the decision on
  `JS_RUNTIME_LANES_PER_DOMAIN` / the 32 MiB latency heap ceiling as either
  "keep" or a tuning task with evidence.
- Close the drift class plan 127 found by hand: a guard that fails when a
  Clay JS facade/`.d.ts` accepts a behavior-changing option the inventory and
  the API page do not document (the completion provider page listed 17 option
  keys that the op never reads; `module`/`exportName`/`moduleSpecifier` were
  the only real ones).
- Keep the trust model unchanged: only the trusted domain and the user's own
  configuration/CLI can grant; no package can grant itself or another package
  anything, and grants remain provenance-bound, visible, and revocable.

Out of scope (recorded, not silently skipped): an **in-app GUI** grant surface.
The desktop app has no package-manager surface today (only package *UI
contributions* via `frontend/src/packages/PackageWorkspace.tsx`), so adding one
is app-UI work that must go through the prototype → explicit approval →
implementation loop in `create-plan/references/clay.md`. This plan therefore
ships the CLI + `init.js` surfaces and records the GUI surface as a further
action with its gate.

## Expected Outcome

- `authorize({ package, capabilities, runtimeProfile, source?, approvedBy? })`
  works from a trusted runtime/configuration context and records a
  `PackageAuthorizationRecord`; packages that call it from the third-party
  domain fail closed with the op absent (no self-grant).
- `clay package authorize @vendor/pkg --capability parse-document
  --capability completion-provider --runtime-profile native-trust` succeeds,
  is idempotent, and is visible in `clay package inspect`.
- The third-party fixture package enables, its manifest-declared completion
  provider registers, and its `moduleSpecifier` handler answers a completion
  request on the latency lane while a 500 ms busy parse handler holds the
  general lane — the first live third-party latency-lane evidence.
- Lane occupancy counters exist per lane and a measurement is recorded in
  `docs/development/performance.md`; the lane-count/heap decision is recorded
  with evidence.
- A failing-by-construction drift guard exists for the option surfaces
  (`module`, `exportName`, `moduleSpecifier`, and every future option), proven
  by a mutation check.
- No new authority: grants authorize only separately documented capabilities,
  configuration still cannot grant filesystem/network/shell/AI/workspace
  authority implicitly, and the two trust domains, bundled inventory
  classification, and revocation behavior are untouched.

## Tasks

- [x] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: the pre-change state is recorded (commit, diff hash) with the
      seven-stage gate result (`audit`, `fmt`, `check`, `clippy`, `test`,
      `bench-compile`, `bindings`) and the plan-127 known-red note
      (`server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`
      under parallel `--lib`).
    - Performance: the plan-127 lane/mailbox/heap measurements are re-recorded
      as this plan's performance baseline (`code-reviews/2026-09-18-plan127-baseline/`
      pointers plus a fresh `CLAY_PERF_REPORT_DIR` run).
    - Code Quality: baseline captured before any edit; logs stored under
      `code-reviews/2026-09-20-plan136-baseline/logs/`.
    - Security: `clay package enable` on the adopted-but-ungranted fixture still
      fails closed with `MissingCapabilityGrant` (the gap this plan closes is
      reproduced, not assumed).
  - Approach:
    - Documentation Reviewed:
      - `plans/127-JS-Runtime-Worker-Concurrency-and-Backpressure.md` →
        `## Further Actions`; `plans/115-Phase3-Package-Installation-Update-and-Clay-Distribution.md`
        (lifecycle verbs); `docs/development/performance.md` (JS runtime
        section); AGENTS.md → platform validation (Linux is the blocking host).
    - Options Considered:
      - Start from the plan-127 evidence and skip a fresh baseline — rejected:
        plan 136 changes the trust-lifecycle path, so its own gate baseline is
        needed.
    - Chosen Approach:
      - Run the gate script on the unmodified tree, reproduce the
        `MissingCapabilityGrant` gap with
        `test-plan/artifacts/127-lane-scheduling/` fixtures, and store both logs.
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings
      cargo test --lib -- --test-threads=1
      ```
    - Files to Create/Edit:
      - `code-reviews/2026-09-20-plan136-baseline/README.md`, `logs/` (new).
    - References:
      - `code-reviews/2026-09-18-plan127-baseline/README.md`,
        `test-plan/artifacts/127-lane-scheduling/run-live.sh`.
  - Test Cases to Write:
    - Baseline gate run: seven stages recorded with exit codes.
    - Gap reproduction: `clay package enable @fixture/lane` →
      `MissingCapabilityGrant` in the enable log.
  - Outcome (2026-09-23):
    - Tree: HEAD `40022f6` ("WIP 133") + plans 133/134/135 uncommitted work,
      plan-136-untouched (`runtime/js/packages.js:67` still the
      `plannedPackageApi("packages.authorize")` stub, no
      `op_clay_packages_authorize`); `git diff HEAD` sha256 `8d121616…522a19`,
      porcelain `15185fb5…e399` (108 entries), captured before any gate and
      re-verified identical after the gate, perf, and gap runs; this plan-file
      update is the only tracked change since.
    - Gates: `scripts/check.sh full` exit 0 in 247 s — the seven named stages
      plus the wrapper's `desktop-clippy`/`desktop-test`; `audit` 9 pre-existing
      RUSTSEC warnings; `test` 2006 passed / 0 failed / 1 ignored across 15
      result lines; serial `--test-threads=1` lib run green (1425 passed /
      1 ignored). The plan-127 known red
      (`server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`
      under parallel `--lib`) did not reproduce: plan 130 A1 (commit `36eabd2`)
      removed the process-global authority that caused it, matching plan 135's
      task-1 baseline outcome.
    - Performance: plan-127 pointers re-recorded from the plan Outcomes and
      `docs/development/performance.md` (the plan-127 `code-reviews/` dir is
      gitignored and gone). Fresh probes: `PLAN127_LANE busy_ms=500
      idle_median_us=1567 busy_completion_us=2594 workers_started=4`; mailbox
      flood `peak_pending=1 superseded=38 evicted=0` and capacity
      `peak_pending=64 evicted=7` identical to plan 127;
      `PLAN127_HEAP recovered_cap=33757330` (~32 MiB, not the 512 MiB ratchet);
      `PLAN127_LANES trusted_ops=194 third_party_ops=145 trusted_only=52`.
      Fresh `CLAY_PERF_REPORT_DIR` live run (bundled `@clay/markdown`, 1 MiB
      `notes.md`, 125 typed chars): `server.edit_ack` p50 231.8 µs / p95
      282.1 µs vs plan 127's 329 / 520 µs — no regression; no
      `js_runtime.lane.*` counter exists yet (the measurement task's gap).
    - Security: `clay package enable @fixture/lane` on the adopted-but-ungranted
      plan-127 fixture still fails closed —
      `MissingCapabilityGrant { package_name: "@fixture/lane", capability:
      CompletionProvider }` — while `clay package inspect` shows
      `Adoption: approved` and no grant surface; the server logs
      `packages.load_failed`. The gap is reproduced, not assumed.
    - Evidence: `code-reviews/2026-09-20-plan136-baseline/README.md` + `logs/`
      (`00-tree-state.txt`, `01-gate-exit-codes.txt`, `gate-check-full.log`,
      `support-test-lib-serial.log`, `02-perf-baseline.txt`,
      `lane-baseline-fresh.log`, `perf-markdown-typing-summary.json`,
      `gap-fixture-enable.log`, `gap-fixture-inspect.txt`,
      `gap-fixture-server.log`).
    - Host deviation (no repo change): this host has no `corepack`, so the
      plan-127 live harness ran through a `/tmp/clay-plan136-bin/corepack` →
      `/usr/bin/pnpm` 11.26.0 PATH shim; `computer-use-linux` window targeting
      was unavailable (GNOME Shell extension absent), so the typing came from
      `wtype` into the AT-SPI-focused editor.

- [x] Review package capability-grant and lane-observability primitives before implementation
  - Acceptance Criteria:
    - Functional: the existing primitives are inventoried with exact paths and
      what each already achieves: `PackageService::authorize_package`
      (`src/packages/service.rs:931`), `PackageAuthorizationRecord` /
      `RuntimeProfile` (`src/packages/authorization.rs`),
      `PackageService::ensure_capability_grants` (`:1607`),
      `authorize_bundled_defaults` (`:1042`), `authorize_language_server`
      (`:959`), `PackageServiceError::MissingCapabilityGrant`, the
      `packages.authorize` inventory entry
      (`docs/reference/clay-js-api/api-inventory.toml:2127`, `status = "planned"`,
      `deno_op = op_clay_packages_authorize`), the `plannedPackageApi` stub
      (`runtime/js/packages.js:66`), the CLI verb parser
      (`src/cli.rs:536`), the lane counters
      (`src/perf/metrics.rs`: `JS_RUNTIME_COMMAND_SUPERSEDED`,
      `JS_RUNTIME_COMMAND_EVICTED`), and the lane budgets
      (`src/perf/budgets.rs`: `JS_RUNTIME_LANES_PER_DOMAIN`,
      `JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES`).
    - Performance: the review states the hot-path position (grant lookup is a
      cheap check against already-loaded authorization state; no grant work in
      typing/paint/layout/scroll/text-event/edit-ack paths) and the counter
      overhead budget for per-lane occupancy.
    - Code Quality: the review names the primitive gaps to implement and
      rejects capabilities shaped around one package, one language, or one
      capability; every new Rust helper is generic.
    - Security: the review states the authority boundary explicitly — grants
      are user/admin/CLI/config records bound to package provenance, bundled
      packages keep their compiled auto-authorization, and no grant path may be
      reachable from the third-party domain.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`,
        `docs/wiki/modules/primitive-architecture.md`,
        `docs/wiki/modules/third-party-runtime-authority.md`,
        `docs/wiki/modules/persistent-runtime-hardening.md`;
        `plans/035-Third-Party-Package-Runtime-Authority-Policy.md`;
        `.agents/skills/clay-execution/references/packages.md` (authority
        boundaries) and `references/js-api.md` (naming/boundary);
        `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md`.
    - Options Considered:
      - Design a new grant primitive (for example a per-capability approval
        table with expiry) — rejected: `PackageAuthorizationRecord`,
        `RuntimeProfile`, and `ensure_capability_grants` already implement the
        approved model; the gap is the missing user-facing call, not the model.
      - Fold the review into the implementation task — rejected: the
        primitive-first rule applies to package runtime capability changes, and
        the grant/list/revoke shapes must be fixed before code starts.
    - Chosen Approach:
      - Read the cited sources, write the inventory + gap list, and fix the task
        list the following tasks implement. No production code in this task.
    - API Notes and Examples:
      ```rust
      // Existing primitive the grant surface must call, not replace.
      pub fn authorize_package(
          &mut self,
          package_name: &str,
          approved_capabilities: Vec<PackagePermission>,
          runtime_profile: RuntimeProfile,
          approved_by: impl Into<String>,
      ) -> Result<(), PackageServiceError>
      ```
    - Files to Create/Edit:
      - `code-reviews/2026-09-20-plan136-baseline/primitive-review.md` (new).
    - References:
      - `src/packages/service.rs`, `src/packages/authorization.rs`,
        `src/packages/permissions.rs`, `runtime/js/packages.js`,
        `docs/reference/clay-js-api/packages/load-package.md` (sibling page).
  - Test Cases to Write:
    - No new tests; the review is verified by the tasks that cite it.
  - Outcome (2026-09-23):
    - `code-reviews/2026-09-20-plan136-baseline/primitive-review.md` (new):
      inventory of `PackageAuthorizationRecord`/`RuntimeProfile`,
      `PackagePermission`, `authorize_package`/`authorize_bundled_defaults`/
      `authorize_language_server` (+ the config-time language-server seal),
      `ensure_capability_grants`/`ensure_package_control_grant`/
      `authorization_matches`, `MissingCapabilityGrant`, `approve_package`/
      `adoption_state`/`revoke_package_approval`, `PackageApprovalStore`/
      `approval_covers`, the CLI verb family, the `packages.authorize`
      inventory entry + facade stub, both op extensions with their counts, the
      lane enum/mailbox/counters/budgets, and the perf recorder — each with
      exact paths and line numbers (the plan's `service.rs` citations were off
      by one and are corrected there).
    - Findings that changed the task list: grants are **in-memory only**
      (`authorizations` at `src/packages/service.rs:357`; `open`/
      `open_production` load only the approval store and ledger), so the CLI
      `authorize` → `inspect`/`enable` acceptance criteria are unachievable
      without a durable grant → **new task added** (extend
      `PackageApprovalRecord` with an explicit grant section, consult it from
      `ensure_capability_grants`, clear it on revoke); the capability gate is
      name-keyed, not provenance-keyed (`authorization_matches` unused there);
      `authorize_package` does not validate that a capability is declared;
      revoke leaves the in-memory grant behind; lane occupancy is
      `#[cfg(test)]` only (`worker.rs:57`, `mod.rs:1544`) with no
      lane/domain-keyed counter, and `PerfSummary` groups by static name only,
      so report-time per-(domain, lane) gauges are the cheapest faithful
      surface; adoption must not imply a grant (the
      `..._and_without_it_fails_closed` test requires it).
    - Hot-path/authority position recorded: grant work is explicit-action-only
      (`HashMap` lookup behind the existing service mutex at
      enable/load/reload boundaries, nothing on typing/paint/layout/scroll/
      text-event/edit-ack paths); the op must be trusted-extension-only *and*
      refuse inside `in_package_activation` (a bundled `loadEntry` runs in the
      trusted domain), mirroring `require_editor_control`; no grant widens
      authority beyond separately implemented, separately validated documented
      APIs; bundled auto-authorization, the language-server seal, and the
      adoption gate are unchanged.
    - Findings recorded as task-list changes: task 3 acceptance now requires
      declared-capability validation inside `authorize_package`, a
      provenance-matched grant read, and the op-activation refusal (+ the
      trusted op-count assertion 98 → 99); the fixture task reuses
      `test-plan/artifacts/127-lane-scheduling/fixture-package` (already
      `moduleSpecifier`-backed + `BUSY_MS` hold) rather than a new fixture.
    - No production code changed in this task; tree state matches the task-1
      baseline plus this plan file.

- [x] Implement the `packages.authorize` op, facade, and typed grant path
  - Acceptance Criteria:
    - Functional: `authorize({ package, capabilities, runtimeProfile, source?,
      approvedBy? })` records a grant through
      `PackageService::authorize_package` and returns a summary
      (`packageName`, `capabilities`, `runtimeProfile`, `approvedBy`,
      `granted: true`); re-authorizing the same package with the same
      capabilities is idempotent; `source` mismatching the installed provenance
      fails; an unknown package fails with `packages.not_installed`;
      `authorize_package` itself rejects any capability the assembled manifest
      does not declare (generic check, so every caller inherits it).
    - Performance: grant work happens only on explicit user/CLI/config action
      (install/enable/load/reload/explicit command); the enable path keeps its
      existing cheap `ensure_capability_grants` check; no grant work or
      authorization-state rebuild on typing, paint, layout, scroll, text-event,
      or edit-ack paths.
    - Code Quality: `runtime/js/packages.js::authorize` becomes a real facade
      call (the `plannedPackageApi("packages.authorize")` stub is deleted),
      `runtime/js/packages.d.ts` types the options and result instead of
      `never`, and the op body stays a thin validated wrapper over the service
      method.
    - Security: the op is registered **only** in the trusted extension
      (`src/server/ops/mod.rs::init_trusted_extension`) and absent from
      `init_package_extension`; a third-party isolate calling it gets
      `TypeError: undefined is not a function` (the
      `package_extension_is_strict_subset_without_admin_ops` op-count assertion
      moves 98 → 99); the op also refuses while
      `ClayOpState::in_package_activation()` is true, so a bundled package's
      `loadEntry` cannot grant capabilities to itself or another package; grants
      require an explicit `approvedBy` (`user`/`cli`/`config`) and never widen
      authority beyond the separately implemented documented APIs;
      `ensure_capability_grants` remains the single enforcement point, reading a
      provenance-matched authorization only, and revocation still fails closed.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/packages/load-package.md` (facade/op
        conventions, typed error mapping); `.agents/skills/clay-execution/references/js-api.md`;
        `references/packages.md` (Authority Boundaries, trust domains);
        `docs/wiki/modules/third-party-runtime-authority.md`.
    - Options Considered:
      - Let `loadPackage` auto-grant declared capabilities — rejected: silent
        authorization contradicts the unified-authority decision and the P56
        fail-closed behavior.
      - Expose grants through `clay.contributions` or an undocumented config
        key — rejected: the inventory's `security_notes` for
        `packages.authorize` state this is the documented surface and hidden
        keys cannot grant capabilities.
      - Implement the op for both runtime domains and let package code call it —
        rejected: that is self-grant authority.
    - Chosen Approach:
      - Promote the planned API exactly as inventoried: trusted-only op →
        `PackageService::authorize_package`, typed `JsErrorBox` mapping for
        `MissingCapabilityGrant`/`NotInstalled`/`InvalidClayMetadata`, facade +
        `.d.ts` update, and tests proving both the success path and the
        third-party-domain denial.
    - API Notes and Examples:
      ```js
      import { authorize } from "clay:packages";

      authorize({
        package: "@vendor/words",
        capabilities: ["completion-provider"],
        runtimeProfile: "native-trust",
        approvedBy: "config",
      });
      ```
    - Files to Create/Edit:
      - `src/server/ops/packages.rs`: `op_clay_packages_authorize` +
        validation + error mapping.
      - `src/server/ops/mod.rs`: register in the trusted extension only.
      - `runtime/js/packages.js`, `runtime/js/packages.d.ts`: real facade +
        types; remove the planned stub.
      - `src/packages/service.rs`: only if the review found a missing
        accessor (for example grant listing for the CLI inspect path).
      - `src/server/js_runtime/tests.rs` (or `tests/package_loading.rs`): grant
        path and denial tests.
    - References:
      - `src/packages/service.rs::authorize_package`,
        `src/packages/authorization.rs`, `docs/reference/clay-js-api/api-inventory.toml:2127`.
  - Test Cases to Write:
    - `authorize_records_capability_grant_for_adopted_package`: grant →
      `ensure_capability_grants` passes for a manifest declaring
      `completion-provider`.
    - `authorize_is_idempotent_and_rejects_unknown_package`: repeat call
      succeeds; unknown package → typed `packages.not_installed`.
    - `authorize_rejects_mismatched_provenance`: `source` that does not match
      the installed provenance fails closed.
    - `third_party_domain_cannot_call_authorize`: op absent in the package
      extension; direct lane dispatch yields a TypeError-shaped failure.
    - `bundled_package_load_entry_cannot_grant_capabilities`: a call made from
      inside package activation fails closed and records no grant.
    - `grant_does_not_survive_provenance_change`: a grant recorded for one
      version/source is inert after the installed provenance changes.
    - `revoked_package_loses_grants`: revoke after grant → enable and dispatch
      fail closed again (the grant record itself is withdrawn by the next task).
  - Outcome (2026-09-23):
    - Shipped: `op_clay_packages_authorize`
      (`src/server/ops/packages.rs`, registered **only** in
      `clay_runtime_trusted_extension`; trusted op count 98 → 99 in the ops
      comment, `package_extension_is_strict_subset_without_admin_ops`, and the
      plan-061 op inventory); the real `authorize` facade
      (`runtime/js/packages.js`; `plannedPackageApi("packages.authorize")`
      deleted); typed `PackageAuthorizeOptions`/`PackageAuthorizationGrant` in
      `runtime/js/packages.d.ts`; new
      `docs/reference/clay-js-api/packages/authorize.md` linked from
      `docs/index.md`; `api-inventory.toml` row flipped to
      `status = "runtime-backed"`, `registry_public = true`, real
      `facade_path`/`deno_op_path`/`backing_rust`/`current_rust_owner`;
      regenerated `docs/generated/clay-js-api-registry.json`; parity-ledger row
      `packages.authorize` added to `packages.modes.settings.themes`.
    - Op contract: `{package, capabilities, runtimeProfile?, source?,
      approvedBy}` with unknown keys rejected; `approvedBy` is required and must
      be `user|cli|config`; `runtimeProfile` defaults to `native-trust`;
      `capabilities` is 1..=21 deduplicated names from the closed vocabulary;
      the summary is `{packageName, version, sourceKind, capabilities,
      runtimeProfile, approvedBy, granted: true}`; errors are stable
      `packages.<code>` prefixes (`invalid_grant`, `unknown_permission`,
      `not_installed`, `provenance_mismatch`, `undeclared_capability`,
      `invalid_manifest`, `grant_during_activation`,
      `authorization_failed`).
    - Enforcement/service changes this task required (the acceptance hardening
      from the primitive review):
      - `PackageService::authorize_package` now rejects any capability the
        assembled manifest does not declare
        (`PackageServiceError::UndeclaredCapability`, new variant + Display).
        Validator nuance recorded in the docs page: authorities the validator
        refuses in `clay.permissions` (filesystem, network, shell, wasm,
        ai-tools, workspace-mutation, native-ui, client-runtime, raw-ops,
        language-server, package-control, package-import) are declarable only
        through the legacy `clay.capabilities` alias, which the check accepts.
      - `ensure_capability_grants` reads an authorization only when it still
        matches the installed provenance (`authorization_matches`), so a
        version/source change makes a grant inert until re-authorized (G3,
        in-memory half; the durable half is task 4).
      - `revoke_package_approval` withdraws the in-memory capability grant
        together with the approval (G5), so revoke → re-adopt → enable fails
        closed instead of reusing the revoked grant.
      - `enable_graph` reports a **revoked or stale** approval before the
        capability gate (extracted `ensure_execution_adoption`, called from
        `verify_relation_authority` after the owner-side relation checks), so
        the post-revoke diagnostic stays `AdoptionRequired`
        (`package_approval.revoked`/`identity_changed`) rather than a withdrawn
        grant. Packages with no approval record at all keep the existing
        capability-first precedence.
    - Tests (all in the repo gate): js end-to-end
      `config_authorize_grants_declared_capability_and_enables_package`
      (adoption alone does not enable; grant + idempotent repeat + load),
      `config_authorize_rejects_undeclared_capability`,
      `config_authorize_rejects_provenance_mismatch_and_unknown_package`
      (`src/server/js_runtime/tests/package_adoption.rs`); op unit
      `grant_authority_closes_during_package_activation`
      (`src/server/ops/packages.rs`); service-level
      `authorize_package_rejects_capability_the_manifest_never_declares`,
      `revoke_withdraws_the_recorded_capability_grant`,
      `grant_does_not_survive_installed_provenance_change`
      (`tests/package_loading.rs`); third-party denial is asserted structurally
      by `package_extension_is_strict_subset_without_admin_ops` (op in the
      trusted extension, absent from the package extension).
    - Fixture/test adjustments the stricter check required (all declare what
      they are granted, as a real package must): the package-graph and
      package-conflict `install_and_authorize` helpers add extra grants to
      `clay.capabilities`; `approval_for` covers `package-control`;
      the two `@vendor/*-repl` fixtures and
      `bundled_defaults_never_auto_grant_language_server` declare
      `package-control`/mode capabilities;
      `language_server_facade_round_trips_exact_uint8array_bytes` declares its
      base capabilities.
    - Gate: `scripts/check.sh full` PASSED (exit 0, all nine stages: audit, fmt,
      check, clippy, test, bench-compile, desktop-clippy, desktop-test,
      bindings) — log
      `code-reviews/2026-09-20-plan136-baseline/logs/gate-check-full-task3.log`.
      Suites: lib 1429 passed/1 ignored, security 155, protocol 227, runtime 75,
      presentation 62, desktop 32+2+1+4+16, bindings 1; no new audit
      advisories.
    - Deliberately left to later tasks: durable grant persistence and
      cross-process `inspect` visibility (task 4, which also owns the durable
      half of `grant_does_not_survive_provenance_change`); the CLI verb (task
      5); a JS-level self-grant attempt from a bundled `loadEntry` needs an
      inventory-verified bundled fixture, so it is covered here by the unit
      guard plus the op's absence from the package extension.

- [x] Make capability grants durable and consult them from the single enforcement point
  - Acceptance Criteria:
    - Functional: `PackageApprovalRecord` (`src/packages/approvals.rs:67`)
      carries an explicit grant section next to the adoption ceiling
      (`capabilities`): granted capability names, granted runtime profile,
      granted-by, granted-at, with an absent section meaning empty
      (deny-by-default, which is today's third-party behaviour);
      `PackageService::authorize_package` persists that section through the
      approval store and still updates the in-memory
      `PackageAuthorizationRecord` for same-generation effect;
      `ensure_capability_grants` passes when either the in-memory record or a
      current durable grant covers every declared capability;
      `clay package inspect` in a fresh process sees the grants; `revoke`
      clears the grant together with the approval.
    - Performance: the durable write happens only on explicit authorize/revoke;
      the enable-path check stays a map lookup plus one already-loaded store
      lookup (no I/O, no store re-read per enable, no allocation); no grant work
      on typing, paint, layout, scroll, text-event, or edit-ack paths.
    - Code Quality: one grant path (`authorize_package` + the store mutator), no
      second validator and no second store file; the store keeps its manual
      JSON conversion, version tag, bounded record/size limits, atomic `0o600`
      write, and fail-closed corruption handling; `approval_covers` semantics
      are unchanged (`capabilities` remains the adoption ceiling) and the grant
      list is validated as a subset of declared capabilities.
    - Security: a durable grant never outlives its provenance — it is matched on
      the same identity fields `approval_covers` checks (resolved version,
      source, package root, integrity, api prefix) and a version/source change
      makes it inert until re-authorized; a revoked or stale approval
      contributes no grant; with no grant the failure stays
      `MissingCapabilityGrant` before any package code runs.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-security.md` (Unified Package
        Capability Model), `docs/wiki/modules/third-party-runtime-authority.md`
        (Adoption Lifecycle, Durable Approval Store),
        `docs/reference/packages/creating-packages.md` (lifecycle verbs).
    - Options Considered:
      - New dedicated grant store file — rejected: duplicates the approval
        store's atomic write, version, corruption, and permission handling.
      - Overload `capabilities` as the granted subset — rejected: a manifest
        that later declares a new capability would be indistinguishable from an
        ungranted one, breaking `approval_covers`'s `CapabilityExpansion`
        semantics.
      - Adoption implies grants (`ensure_capability_grants` trusts
        `approval_covers` alone) — rejected: makes the per-capability CLI/op
        surface decorative and contradicts the plan's
        `..._and_without_it_fails_closed` test.
      - Grants only from `init.js`, no durability — rejected: cannot satisfy the
        cross-process CLI `inspect`/`enable` acceptance criteria in the next
        task.
    - Chosen Approach:
      - Extend the existing durable approval record with the explicit grant
        section (backward compatible: absent = empty), add the store mutator
        that rewrites it under the same atomic-write path, make
        `ensure_capability_grants` accept the union of the in-memory record and
        the identity-matched durable grant, and have revoke clear both.
    - API Notes and Examples:
      ```json
      {
        "package": "@vendor/words",
        "capabilities": ["completion-provider", "parse-document"],
        "grants": ["completion-provider"],
        "grantedRuntimeProfile": "native-trust",
        "grantedBy": "cli",
        "grantedAt": "2026-09-23T00:00:00Z"
      }
      ```
    - Files to Create/Edit:
      - `src/packages/approvals.rs` (grant section, mutator, validation, tests).
      - `src/packages/service.rs` (`authorize_package` persistence,
        `ensure_capability_grants` source, revoke clearing, inspect source).
      - `src/launch.rs` (`run_package_subcommand`: `inspect` print and
        `revoke`) where the CLI path needs the durable grant (the `authorize`
        verb itself is the next task).
      - `tests/package_loading.rs` or `src/packages/approvals.rs` tests.
    - References:
      - `src/packages/approvals.rs::PackageApprovalRecord`,
        `::PackageApprovalStore::upsert`, `::approval_covers`;
        `src/packages/service.rs::authorize_package`,
        `::ensure_capability_grants`, `::revoke_package_approval`.
  - Test Cases to Write:
    - `durable_grant_survives_a_new_service_process`: authorize through one
      `PackageService`, reopen the same store root, then enable succeeds.
    - `store_without_grants_still_fails_closed`: a version-1 record with no
      grant section enables nothing.
    - `grant_is_inert_after_provenance_change`: re-install a different
      version/source → enable fails with `MissingCapabilityGrant`.
    - `revoke_withdraws_the_durable_grant`: revoke → enable and dispatch fail
      closed, and the persisted record no longer lists the grant.
  - Outcome (2026-09-23):
    - Delivered: `CapabilityGrant` (`src/packages/approvals.rs`) and the
      optional `grant` section on `PackageApprovalRecord` (`capabilities`,
      `runtime_profile`, `granted_by`, `granted_at`; absent = no grant, so
      adoption stays deny-by-default and the format-version-1 store stays
      backward compatible). Store mutators: `record_grant` (annotates a
      current, unrevoked, identity-matching record; returns `false` when none
      exists) and `current_grant` (the durable read); `revoke` now clears the
      grant with the flag. The identity loop moved into one shared
      `identity_matches` used by both `approval_covers` and the grant read, and
      `CapabilityGrant::validate` (non-empty, who/when, subset of the record's
      adoption ceiling) runs at load and upsert, so a grant outside the ceiling
      fails closed instead of being dropped. `RuntimeProfile::parse` is now the
      single profile-name vocabulary (shared with the op).
    - Service: `authorize_package` persists the grant through the store before
      inserting the in-memory record (a store failure fails the call, never a
      half-granted state); `capability_granted(record, permission)` replaced
      `has_approved_capability` as the one capability read — the
      provenance-matched in-memory authorization or the identity-matched
      durable grant — and is used by `ensure_capability_grants`,
      `ensure_package_control_grant` (now record-keyed), and both op-dispatch
      checks in `src/server/ops/mod.rs`, so a grant recorded by an earlier
      process authorizes the same package there too; `granted_view` unions
      in-memory and durable grants for `PackageInspection`, and
      `clay package inspect` prints `Grants: <names> (<profile>)`. The op body,
      facade, typed options, docs page, registry row, and parity ledger from
      task 3 are unchanged (no API-surface change in this task).
    - Semantics recorded in the docs: a grant annotates an approval the user
      already made — with no current record (or a revoked/stale one) it stays
      in-memory for that generation and no durable section is written; the
      documented order is install → adopt → authorize → load; re-adoption
      rewrites the record (a previous identity's grant does not carry over);
      revoke clears the section. Bundled packages are unaffected (their
      authority is the compiled inventory, and the op requires an installed
      store package).
    - Tests: store-level `grant_round_trips_and_is_identity_bound` (grant
      survives a reopen, is inert on version and api-prefix drift, and is never
      manufactured by `record_grant`) and
      `revoke_clears_the_grant_and_oversized_grants_fail_closed` (revoke clears
      the persisted section; a grant outside the ceiling fails at upsert and at
      load); the four plan-named integration tests
      `durable_grant_survives_a_new_service_process`,
      `store_without_grants_still_fails_closed`,
      `grant_is_inert_after_provenance_change`,
      `revoke_withdraws_the_durable_grant` (`tests/package_loading.rs`, durable
      store root + fresh `PackageService`); the record-shape pin in
      `approval_records_carry_no_tab_client_or_workspace_keying` now includes
      `grant`; `tests/package_graph.rs`'s `approval_for` literal records
      `grant: None`.
    - Live evidence (`code-reviews/2026-09-20-plan136-baseline/logs/03-durable-grant-live.txt`,
      plan-127 fixture package in a private mode-700 root): adopted + no grant
      → `inspect` shows no `Grants:` line and `enable` fails with
      `MissingCapabilityGrant { capability: CompletionProvider }`; a server-side
      config `init.js` calling `packages.authorize` writes the durable section
      (0 grant/load failure lines) and loads the package; a fresh CLI process
      then shows `Grants: completion-provider, mode-registration,
      parse-document (native-trust)` and `enable` succeeds; `revoke` reports
      `Revoked approval`, `inspect` shows `approval revoked` with no grants,
      `enable` fails with `AdoptionRequired` (`package_approval.revoked`), and
      the persisted record has `revoked: true, grant: None`.
    - Gate: `scripts/check.sh full` PASSED (exit 0, all nine stages) — log
      `code-reviews/2026-09-20-plan136-baseline/logs/gate-check-full-task4.log`.
      Suites: lib 1431 passed/1 ignored, security 159, protocol 227, runtime 75,
      presentation 62, desktop 32+2+1+4+16, bindings 1; no new audit
      advisories. Performance: the durable write happens only on
      authorize/revoke, and the enable/dispatch read is two `HashMap` lookups
      plus one identity compare over state already in memory (no I/O, no store
      re-read, no allocation on the UTF-8 path); the perf-budget suite passes
      unchanged.
    - Docs updated: `docs/reference/primitives/package-security.md` (grant
      section in the `clay-package-approval-v1` schema + closed rules +
      enforcement paragraph, and the durable-authorization gap removed from the
      implementation status),
      `docs/wiki/modules/third-party-runtime-authority.md` (store fields,
      `current_grant` identity rule, Authorize/Enable/Revoke lifecycle),
      `docs/reference/packages/creating-packages.md` (adoption vs grant, durable
      visibility), `docs/reference/clay-js-api/packages/authorize.md`
      (durability, corrected adopt-then-authorize order, `capability_granted`
      as the enforcement point).
    - Deliberately left to later tasks: the CLI `authorize` verb and printing
      grant provenance (`granted_by`/`granted_at`) in `inspect` (task 5); the
      fixture grant → provider → latency-lane end-to-end proof (task 6, which
      owns updating the plan-127 `fixture` harness comment that still says no
      grant surface exists); durable language-server grants (separate sealed
      grant path, unchanged).

- [x] Add the host CLI grant surface and grant visibility in `clay package inspect`
  - Acceptance Criteria:
    - Functional: `clay package authorize <name> --capability <c>...`
      (`--capability` repeatable, `--runtime-profile native-trust|sandboxed|restricted`,
      `--approved-by` defaulting to `cli`) records the grant through the same
      service path; `clay package inspect <name>` prints granted capabilities,
      runtime profile, approval provenance/date, and the declared-but-ungranted
      capabilities; `CLI_USAGE` documents the verb.
    - Performance: CLI-only work; the server never parses grant flags, and the
      CLI stays out of hot paths.
    - Code Quality: the verb joins the existing `PackageCommand` family
      (`src/cli.rs:81`), reuses `parse_package_subcommand` (`:536`) and the
      existing install-record/service plumbing; no second grant path and no
      new package manager.
    - Security: capabilities are validated against the package's declared
      `clay.permissions` before being recorded (unknown or undeclared
      capability → error); the CLI cannot grant a capability the manifest never
      declares; `clay package revoke` continues to remove grants and disable; a
      grant introduces no filesystem, network, shell, or external-process
      authority of its own.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md` (lifecycle verbs,
        capability declarations); `plans/115-Phase3-*` task 4 (CLI verbs);
        `src/cli.rs` usage text.
    - Options Considered:
      - Fold grants into `clay package enable --grant …` — rejected for v1:
        enable stays "runs Clay-owned validation", and a separate verb keeps
        the audit trail per action; the flag can be added later if usage
        shows the two-step is friction.
      - Grant everything the manifest declares on enable — rejected:
        deny-by-default, capability-by-capability approval.
    - Chosen Approach:
      - Add `authorize`/`grant` as a lifecycle verb next to `adopt`/`revoke`,
        print the grant state in `inspect`, and document it in the usage block
        and the package guide.
    - API Notes and Examples:
      ```bash
      clay package authorize @vendor/words \
        --capability completion-provider \
        --capability parse-document \
        --runtime-profile native-trust
      clay package inspect @vendor/words
      ```
    - Files to Create/Edit:
      - `src/cli.rs` (`PackageCommand`, parser, usage), `src/packages/verbs.rs`
        (verb implementation), `src/packages/service.rs` (grant listing for
        inspect), `tests/package_cli.rs` (CLI tests).
    - References:
      - `src/cli.rs:81–122`, `src/cli.rs:536`, `src/packages/verbs.rs`,
        `tests/package_cli.rs`.
  - Test Cases to Write:
    - `cli_authorize_records_capability_grants`: authorize → inspect shows the
      grants and profile.
    - `cli_authorize_rejects_undeclared_capability`: manifest without
      `completion-provider` → error, no grant recorded.
    - `cli_authorize_then_enable_succeeds_and_without_it_fails_closed`: the P56
      gap closed from the CLI, with the ungranted path still failing.
  - Outcome (2026-09-23):
    - Delivered: `clay package authorize <name> --capability <cap>
      [--capability <cap>]... [--runtime-profile <p>] [--approved-by <who>]`
      (defaults `native-trust` / `cli`). New
      `PackageCliSubcommand::Authorize` + parser in `parse_package_subcommand`
      (repeatable `--capability`, missing value and unknown flag rejected),
      dispatch in `src/launch.rs`, and the verb body in
      `src/packages/verbs.rs::authorize` — which routes through
      `PackageService::authorize_package`, so there is no second grant path and
      the durable write, declared-capability check, and provenance binding are
      the task-3/4 ones. `CLI_USAGE` documents the verb, the subcommand list,
      and the three flags.
    - Semantics: adoption comes first — the verb refuses an unadopted package
      ("is not adopted; run `clay package adopt …` first so the grant is
      durable") because a CLI grant on an unadopted package is an in-memory
      record in a process about to exit. A grant is a complete set: a
      re-authorize replaces the previous grant rather than adding to it (the
      store keeps one `capabilities` list), documented in the usage text, the
      package guide, the JS API page, and the wiki. Unknown capability names
      are rejected with `parse_permission`, an unknown profile with
      `RuntimeProfile::parse` (single vocabulary), and undeclared capabilities
      by the service before anything is recorded. `clay package revoke` is
      unchanged and still clears the grant with the approval.
    - Inspect visibility: `PackageInspection` gained `grant_provenance:
      Option<GrantProvenance>` (`granted_by`/`granted_at` from the durable
      grant, `approved_by`/`approved_at` from the approval record); the private
      `granted_view` now returns one internal `GrantView` so capabilities,
      profile, and provenance come from a single record lookup, and
      `PackageApprovalStore::current_record` was extracted so `current_grant`
      delegates to the same identity rule. `verbs::format_grant_lines` is the
      one formatter (used by `inspect` and by `authorize`'s confirmation) and
      prints `Grants:` (capabilities + profile), `Granted by:`, `Approved by:`,
      and `Ungranted: <declared, not granted>`.
    - Tests: the three plan-named cases in `tests/package_cli.rs`
      (`cli_authorize_records_capability_grants` — output, inspect fields,
      provenance, `Ungranted:` line, and a fresh `PackageService` over the same
      root seeing the same grant; `cli_authorize_rejects_undeclared_capability`
      — undeclared, unknown, and bad-profile rejections with nothing recorded;
      `cli_authorize_then_enable_succeeds_and_without_it_fails_closed` —
      unadopted refusal, fail-closed enable, partial grant still blocked,
      replacement semantics, then a full grant enabling and clearing the
      `Ungranted:` line). Parser coverage extends
      `src/cli.rs::tests::parse_install_remove_list_and_lifecycle` (full flag
      set, defaults, `--capability requires a value`, unknown option).
    - Live evidence (`code-reviews/2026-09-20-plan136-baseline/logs/04-cli-grant-live.txt`,
      `target/debug/clay` against the plan-127 fixture package in the private
      mode-700 root): authorize before adoption → refusal; unknown capability →
      `unknown capability `not-a-capability``; adopt + partial grant →
      `Grants: completion-provider, mode-registration (sandboxed)`,
      `Granted by: user at …`, `Approved by: cli at …`,
      `Ungranted: parse-document (declared, not granted)`, and
      `enable` → `MissingCapabilityGrant { capability: ParseDocument }`; full
      grant → replaces the partial one, `Enabled @fixture/lane`; a fresh CLI
      process prints the same grant lines; `revoke` → `Adoption: approval
      revoked`, every capability back under `Ungranted:`, persisted record
      `revoked: True grant: None`.
    - Gate: `scripts/check.sh full` PASSED (exit 0, nine stages) — log
      `code-reviews/2026-09-20-plan136-baseline/logs/gate-check-full-task5.log`.
      Suites: lib 1431, bin 9, security 162 (three new), protocol 227, runtime
      75, presentation 62, desktop 32+2+1+4+16, bindings 1; no new audit
      advisories. Performance: CLI-only work — the server parses no grant
      flags, and the only server-side shape change is one extra
      `PackageInspection` field filled from an in-memory store read, so no hot
      path changes; the perf-budget suite passes unchanged.
    - Docs updated: `src/cli.rs::CLI_USAGE`;
      `docs/reference/packages/creating-packages.md` (verb, defaults,
      replacement semantics, inspect lines);
      `docs/reference/clay-js-api/packages/authorize.md` (CLI parity, complete
      set semantics); `docs/wiki/modules/third-party-runtime-authority.md`
      (Authorize lifecycle bullet). `docs/development/distribution.md` and
      `docs/wiki/modules/package-management.md` already defer the adoption/
      grant store to `third-party-runtime-authority.md` and needed no change.
    - Deliberately skipped: a `grant` alias for the verb (one name, matching the
      JS op and the acceptance criteria); folding grants into
      `clay package enable --grant …` (already rejected in this task's approach
      for v1); printing the ungranted list on `authorize` failures is not
      needed because the confirmation lines already carry it. Task 6 owns the
      fixture grant → provider → lane proof and updating the plan-127
      `fixture`-mode comment that still says no grant surface exists.

- [x] Prove the third-party grant → provider → latency-lane path with a fixture
  - Acceptance Criteria:
    - Functional: an adopted third-party fixture package with
      `parse-document` + `completion-provider` declares a slow parse handler and
      a `moduleSpecifier` completion provider; after grants and enable, a
      completion request is answered by the latency lane while a 500 ms busy
      parse handler holds the general lane, and the provider's answer carries
      the package's provenance.
    - Performance: measured completion latency with the general lane blocked
      stays in the lane-isolation envelope recorded by plan 127 task 3
      (`test-plan/artifacts/127-lane-scheduling/`, lane-latency logs), i.e. tens
      of microseconds to low milliseconds, not `busy_ms - timeout`; the
      measurement is recorded as evidence.
    - Code Quality: the fixture lives with the plan-136 artifacts and is driven
      by one script (no manual server babysitting); the existing
      `@fixture/lane` / `@fixture/laneblocked` packages are reused where they
      already match.
    - Security: without the grant the same fixture still fails closed with
      `MissingCapabilityGrant` and contributes nothing (plan 127 negative check
      preserved); the granted package still cannot call trusted-only ops, and
      the latency lane serves only its own package-owned module.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/artifacts/127-lane-scheduling/README.md` and `run-live.sh`;
        `docs/wiki/modules/third-party-runtime-authority.md`;
        `docs/wiki/modules/persistent-runtime-hardening.md` (lane routing).
    - Options Considered:
      - Automated integration test only — rejected: the package adoption +
        grant + live lane path is exactly the part that plan 127 could only
        reach with bundled packages, so at least one live run is the evidence
        worth having. (Automated tests still cover the same path in CI.)
      - Live run only — rejected: CI must keep the regression.
    - Chosen Approach:
      - Add a plan-136 artifact directory with the fixture, extend the existing
        `run-live.sh` harness with a `granted-lane` mode, and add an automated
        test that asserts the lane routing for the fixture without the GUI.
    - API Notes and Examples:
      ```bash
      clay install ./test-plan/artifacts/136-capability-grants/fixture
      clay package adopt @fixture/granted
      clay package authorize @fixture/granted --capability parse-document --capability completion-provider
      clay package enable @fixture/granted
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/136-capability-grants/` (fixture package(s),
        `run-live.sh` mode, `README.md`).
      - `tests/completion_provider.rs` or `src/server/js_runtime/tests.rs`:
        automated lane-routing test for the granted fixture.
    - References:
      - Plan 127 task 3/7 evidence; `src/server/ops/completion.rs` (contributions
        path); `src/packages/record/language.rs::parse_completion_provider_contributions`.
  - Test Cases to Write:
    - `granted_third_party_provider_serves_from_latency_lane_when_general_lane_busy`:
      guarded lane identity + latency envelope.
    - `ungranted_third_party_provider_still_fails_closed`: enable fails with
      `MissingCapabilityGrant`; no provider registered.
  - Outcome (2026-09-23):
    - Delivered: `test-plan/artifacts/136-capability-grants/` (`README.md` with
      the evidence index and the recorded ceiling, `init-granted-lane.js`,
      `drive-granted-lane.sh`), the two fixture changes the live path needed
      (`fixture-package/load.js` now registers its own mode pattern and passes
      `module` **and** `moduleSpecifier` to the completion registration;
      `package.json` declares the `.` trigger), the `granted-lane` /
      `ungranted-lane` modes in
      `test-plan/artifacts/127-lane-scheduling/run-live.sh`, and the automated
      cases in `src/server/js_runtime/tests/lanes_and_queues.rs`.
    - Automated evidence (`automated-lane/lane-measurement.txt`):
      `PLAN136_GRANTED_LANE busy_ms=500 idle_median_us=2281
      busy_completion_us=3572 workers_started=4` against the plan 127 task 3
      envelope (idle 1567 / busy 2594 µs; bound 250 ms). The same cases assert
      that the answer carries the fixture's `provenance.package_name` /
      `package_prefix`, that the latency lane may materialize only the owning
      package's module (load-entry allowlist, checked against a foreign package),
      that neither registration leaves the third-party `RuntimeDomain`, that a
      busy general lane replaces no worker, and that the ungranted fixture fails
      closed with `MissingCapabilityGrant` and starts no lane. 15/15 lane tests
      pass; `ungrafted_…` in this plan's Test Cases text is a typo for
      `ungranted_…`.
    - Live evidence (private mode-700 root, real `target/debug/clay` server and
      client): install → adopt → `clay package authorize` (completion-provider,
      mode-registration, parse-document; `native-trust`; `cli`) → `enable` →
      `inspect` shows the grant/provenance lines; the granted server loads the
      package in its own domain (`runtime.load_in_package_domain` = 1 in
      `live-granted-lane/perf-summary.json`) and the synthesized edit lands in
      `demo.lane` (`chars=29` → `35`). `ungranted-lane` reproduces
      `MissingCapabilityGrant { capability: CompletionProvider }`,
      `Ungranted: completion-provider, mode-registration, parse-document`, and
      the server-side `packages.load_failed` diagnostic (the plan 127 negative
      check, preserved).
    - Live ceiling (recorded, not worked around): no completion popup appears
      because a package-owned mode never activates in a live session. Third-party
      manifest contributions are applied only for trusted/bundled records
      (`src/server/ops/packages.rs:736-738`), and the only
      `modes.activate_major_mode` caller is the `clay:modes` JS op
      (`src/server/ops/mod.rs:1104`), which needs an already-open `documentId`;
      there is no document-open JS hook (`runtime/js/documents.js`), no
      client/protocol mode-activation message, and configuration runs before any
      document exists. So the fixture's parse handler is never dispatched live
      and the client never receives the fixture's editor rules/trigger
      characters. The functional/performance/security criteria are covered by the
      automated cases (the CI half this task's approach requires); the live step
      records the grant → enable → load half, the ungranted half, and the
      ceiling. File:line pointers in
      `test-plan/artifacts/136-capability-grants/README.md`.
    - Harness: one driver (`drive-granted-lane.sh`) focuses the window through
      the compositor's IPC, refuses to synthesize input unless `clay-desktop` owns
      focus, types the edit and the fixture's `.` trigger, and captures the
      AT-SPI tree plus a portal screenshot cropped to the window;
      `run-live.sh` keeps its store seeding, isolation, and SIGTERM
      perf-summary wiring. Host deviations recorded: `corepack` is absent, so the
      harness `pnpm` shim now falls back to `/usr/bin/pnpm` and the harness calls
      the shim instead of `corepack pnpm` (the old shape re-entered the shim
      recursively); the compositor is mango (dwl), so the GNOME-extension capture
      path the plan 126/127 scripts assume is unavailable, and portal captures do
      not contain the editor's accelerated text layer (the AT-SPI `chars=` count
      is the readable signal).
    - Gate: `scripts/check.sh full` PASSED (exit 0, nine stages) — log
      `code-reviews/2026-09-20-plan136-baseline/logs/gate-check-full-task6.log`.
      Suites: lib 1433 (+2 over the task 5 gate: the two plan-named lane cases),
      bin 9, presentation 62, protocol 227, runtime 75, security 162, desktop
      32+2+1+4+16, bindings 1, `webview bindings up to date`; 9 allowed audit
      warnings, unchanged from the task 1 baseline.
    - Docs updated: `test-plan/artifacts/127-lane-scheduling/README.md` (fixture
      changes, trigger note, updated ceiling, pointer to the plan 136 artifacts),
      the `fixture`-mode comment in `run-live.sh` that still claimed no
      user-facing grant surface exists, and the new plan 136 artifact README.
    - Deliberately skipped: a second fixture package (`@fixture/lane` already
      matched, per this task's approach note); GUI-level lane latency numbers
      (input synthesis and AT-SPI polling are slower than the completion round
      trip, so the automated measurement is the performance evidence); teaching
      `probe.py wait-ready` about this build's editor node (it exposes
      `Text`/`Action`/`Component`, not `EditableText`, so readiness is
      `probe.py editor`).

- [x] Measure provider lane occupancy and record the lane/tuning decision
  - Acceptance Criteria:
    - Functional: per-lane command counts and occupancy are observable in the
      perf summary produced by `CLAY_PERF_REPORT_DIR`: for each (domain, lane),
      the dispatch count plus mailbox occupancy (pending/peak), `superseded`,
      and `evicted`, with the lane-specific meaning stated in the doc (general
      = evaluation/parse/analysis work, latency = supersedable
      completion/language-intelligence mailbox); a real measurement is taken
      with the granted fixture and a mixed workload (general lane busy with
      parse work, latency lane serving completions) and recorded in
      `docs/development/performance.md`.
    - Performance: the per-command cost is one relaxed atomic increment (or
      nothing, for the mailbox stats the lane already tracks) — no locking, no
      allocation, and no perf-recorder snapshot on the dispatch path, because
      the lane counters are recorded **once at report time** (before
      `write_perf_report`, `src/launch.rs:295`) rather than once per command;
      that keeps the 4096-snapshot budget for per-edit metrics; the measurement
      states observed latency percentiles for the latency lane under load.
    - Code Quality: lane-keyed metric names are static (`&'static str`, e.g. a
      `[[&str; JS_RUNTIME_LANES_PER_DOMAIN]; 2]` table) because `PerfSummary`
      aggregates by name only and metadata is not grouped; the production
      accessor on `ClayJsRuntimeService` replaces the `#[cfg(test)]`-only
      `lane_queue_stats`/`CommandQueueStats` path (`mod.rs:1544`,
      `worker.rs:57`); counter names and the measurement live next to the
      plan-127 JS-runtime section in `docs/development/performance.md`; the
      tuning decision (keep `JS_RUNTIME_LANES_PER_DOMAIN = 2` and the 32 MiB
      latency ceiling, or raise them) is recorded with the numbers that
      motivated it.
    - Security: counters expose counts only — no document content, package
      names beyond existing provenance rules, or token values in the perf
      summary.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md` (JS runtime section written by plan
        127); `src/perf/metrics.rs`; `src/perf/budgets.rs`;
        `code-reviews/2026-09-20-plan136-baseline/logs/`.
    - Options Considered:
      - Measure with ad-hoc logs — rejected: counters ride the existing perf
        recorder and are testable.
      - Add per-request histograms per lane — rejected as premature: percentile
        data already exists for server work; lane occupancy is a count.
    - Chosen Approach:
      - Add lane-keyed metrics that are counted per command in atomics but
        recorded once at report time, take the fixture measurement, and write
        the decision. If the mix shows latency-lane work dominating, the
        follow-up is a concrete tuning task with the evidence attached.
    - API Notes and Examples:
      ```bash
      CLAY_PERF_REPORT_DIR=test-plan/artifacts/136-capability-grants/perf clay server --config-fixture granted-lane
      ```
    - Files to Create/Edit:
      - `src/perf/metrics.rs` (lane counter names), `src/server/js_runtime/worker.rs`
        or `src/server/js_runtime/mod.rs` (increment site), `src/perf/budgets.rs`
        (only if the decision changes a budget),
        `docs/development/performance.md` (measurement + decision).
    - References:
      - Plan 127 task 4/5 counter precedent
        (`JS_RUNTIME_COMMAND_SUPERSEDED`/`JS_RUNTIME_COMMAND_EVICTED`).
  - Test Cases to Write:
    - `lane_command_counters_track_general_and_latency_separately`: a
      completion on the latency lane and a parse on the general lane increment
      different counters.
    - `lane_counters_do_not_require_locking_on_the_dispatch_path`: counter type
      check (atomic), plus the existing lane isolation test suite staying green.
  - Outcome (2026-09-23):
    - Delivered: `js_runtime.lane.<domain>.<lane>.{dispatched,pending,
      peak_pending,superseded,evicted}` — static names in
      `JS_RUNTIME_LANE_METRICS` (`src/perf/metrics.rs`, indexed
      `[domain][lane]`, so the plan's `[[&str; …]; 2]` shape) — recorded once
      at report time by `ClayJsRuntimeService::record_lane_metrics`, wired
      through `RuntimeGenerationStore` → `IpcServer::record_lane_metrics` →
      the server's SIGTERM hook in `src/launch.rs` right before
      `write_perf_report`. `CommandQueueStats`/`lane_queue_stats`/`queue_stats`
      are production now (no `#[cfg(test)]`), the dispatch count is a relaxed
      `AtomicU64` on `CommandMailbox` bumped in `recv()` (compile-time type
      assertion next to the field), and the per-drop `global_recorder()` calls
      in `enqueue` are gone: drops only bump the mailbox counters plan 127
      already kept, so a supersede no longer spends a perf snapshot.
      `MetricSummary` gained `total` (summed counter/bytes amounts, last gauge
      value) because the summary previously carried only snapshot counts —
      without it a report-time counter would have been unreadable. The old
      global `js_runtime.command.superseded`/`.evicted` names are replaced by
      the per-lane ones (docs updated: `docs/development/performance.md`
      plan-127 table, `docs/wiki/modules/persistent-runtime-hardening.md`,
      `test-plan/11-performance.md`).
    - Measurement (granted plan-127 fixture, mixed workload: 500 ms package
      parse handler holding the general lane, 12 completions on the latency
      lane): latency p50 2264 µs / p95 2892 µs / max 2892 µs (idle median
      ≈ 2.4 ms, bound 250 ms); `third_party.general` dispatched 4 / peak 0;
      `third_party.latency` dispatched 13 / peak 1; `trusted.*` lanes 0. The
      existing plan-127 prints now come from the same per-lane counters:
      one-document burst (40 requests) peak 1 / superseded 38 / evicted 0,
      past-capacity (72 requests) peak 64 / superseded 0 / evicted 7. Live
      granted-lane session via `run-live.sh` + `CLAY_PERF_REPORT_DIR`:
      `trusted.general` 2, `trusted.latency` 0, `third_party.general` 5,
      `third_party.latency` 3 — and a client-free run of the same config shows
      the same 3 latency dispatches, so they are package-load registrations,
      not client requests (plan 136 task 6's mode-activation ceiling is
      unchanged). Summaries kept in
      `test-plan/artifacts/136-capability-grants/lane-occupancy/`.
    - Tuning decision: **keep `JS_RUNTIME_LANES_PER_DOMAIN = 2` and
      `JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES = 32 MiB`** — the split is not
      binding: latency p95 ~2.9 ms against a 500 ms busy general lane, backlog
      never above one pending command under a one-document burst (supersede
      collapsed 38/40 onto one slot), capacity eviction only in the synthetic
      72-request test, no lane heap-limit/timeout event, and no per-lane heap
      probe to justify raising a cap (the 32 MiB ceiling is a cap, not a
      reservation). Revisit when a real-session mix shows latency-lane pressure
      (heap-limit/poison event, or supersede/evict churn outside the burst
      tests). Rationale and numbers: `docs/development/performance.md`
      ("Plan 136 lane occupancy measurement and tuning decision").
      `src/perf/budgets.rs` is unchanged, as the decision requires.
    - Tests: `lane_command_counters_track_general_and_latency_separately`
      (separation across domain and lane, exact recorded values, zeroes for
      idle lanes, latency envelope) and
      `lane_counters_do_not_require_locking_on_the_dispatch_path` (exact
      dispatch accounting on the latency lane, general-lane counter untouched,
      no supersede for sequential work; the atomic type check is the
      compile-time assertion in `worker.rs`). 17/17 lane tests pass, including
      the plan-127 flood/capacity/isolation suites.
    - Gate: `scripts/check.sh full` PASSED (exit 0, nine stages) — log
      `code-reviews/2026-09-20-plan136-baseline/logs/gate-check-full-task7.log`.
      Suites: lib 1435 (+2 over the task 6 gate: the two lane-counter tests),
      bin 9, presentation 62, protocol 227, runtime 75, security 162, desktop
      32+2+1+4+16, bindings 1, `webview bindings up to date`; 9 allowed audit
      warnings, unchanged. The first attempt failed one unrelated test
      (`agent_protocol::mock_daemon_prompt_persists_no_secret_on_ack`, "Text file
      busy" while spawning `clay-agent` — environment flake, passes in
      isolation); the recorded log is the clean rerun.
    - Security: the five counters are counts and gauges only — no document
      text, provider source, paths, package names, or tokens; the recording
      reads the mailbox and writes the summary, and exposes no new authority.
    - Deliberately skipped: per-lane latency histograms (this plan's own
      "rejected as premature": the completion path already has percentiles),
      a per-lane heap probe (no JS heap API reachable from a provider
      invocation; the decision records the missing input instead), and a
      separate `pending` peak-vs-current distinction beyond the gauge pair.

- [x] Harden the Clay JS option-surface drift guard
  - Acceptance Criteria:
    - Functional: a test fails when a non-`never` option key declared in
      `runtime/js/*.d.ts` for an inventory API is missing from that API's
      `custom_properties` (page frontmatter + `api-inventory.toml`) or from the
      page's `## Options` section; it covers at least
      `completion.serverRegisterCompletionProvider`
      (`module`, `exportName`, `moduleSpecifier`),
      `language.serverRegisterLanguageIntelligenceProvider`, and
      `packages.authorize` (the five inventoried keys).
    - Functional (precondition for the guard): the declared option surfaces are
      first brought in line with the ops. `runtime/js/completion.d.ts` still
      declares eleven options the op never reads (`completionProvider`,
      `contribution`, `providerId`, `triggerCharacters`, `triggers`,
      `wordBoundaryChars`, `items`, `priority`, `exclusive`, `timeoutMs`,
      `maxItems`) — TypeScript autocomplete currently steers package authors
      straight into the silent-ignore trap. Each key is either deleted from the
      typings (metadata lives in `clay.contributions.completionProviders`) or
      implemented and documented, decided key by key and recorded in the task
      evidence; the same audit runs over `runtime/js/language.d.ts`. Deleting an
      ignored key is the expected outcome for the metadata keys, so a TypeScript
      user passing `items` gets a compile error instead of a no-op.
    - Performance: documentation-test only; no runtime cost.
    - Code Quality: the guard reuses the existing inventory parsing helpers
      (`tests/clay_js_api_inventory.rs`) instead of a second parser; keys that
      are intentionally op-only (for example `packageManifest`,
      `permissions`) are listed explicitly with a one-line reason, so the
      allowlist is a decision, not a hole.
    - Security: the guard never loosens the existing
      frontmatter↔inventory↔registry equality or the denied-authority checks;
      it only adds the facade-side direction that was missing.
  - Approach:
    - Documentation Reviewed:
      - `tests/clay_js_api_inventory.rs` (frontmatter↔inventory equality,
        `DENIED_AUTHORITIES`, required sections); `tests/clay_js_facade_layout.rs`
        (`.d.ts`↔`.js` export parity precedent); plan 127 → `## Further Actions`
        item 4 (the hand-found drift).
    - Options Considered:
      - Compare full key sets including `never`-typed reject keys — rejected:
        reject keys (`handler`, `callback`, …) are a denial list, not options.
      - No guard, rely on review — rejected: the exact drift plan 127 found by
        hand existed for months and was invisible to every gate.
    - Chosen Approach:
      - Extract option keys from the declared options type per facade (one level
        of nested object types where the API documents structured items), skip
        `never`-typed keys, and require the rest in inventory + Options section.
    - API Notes and Examples:
      ```ts
      export type ServerRegisterCompletionProviderOptions = {
        module?: Record<string, unknown>;
        moduleSpecifier?: string;
        exportName?: string;
        handler?: never; // denial list: skipped by the guard
      };
      ```
    - Files to Create/Edit:
      - `tests/clay_js_api_inventory.rs` (new test) or a new
        `tests/clay_js_option_surface.rs`.
      - `runtime/js/completion.d.ts`, `runtime/js/language.d.ts` (option-surface
        cleanup where keys are ignored); `docs/reference/clay-js-api/completion/server-register-completion-provider.md`,
        `docs/reference/clay-js-api/api-inventory.toml`,
        `docs/generated/clay-js-api-registry.json` if the cleanup changes a
        documented key; `tests/clay_js_facade_layout.rs` if the `.d.ts`↔`.js`
        parity test needs to learn the removal.
    - References:
      - `runtime/js/completion.d.ts`, `runtime/js/language.d.ts`,
        `runtime/js/packages.d.ts`, `docs/reference/clay-js-api/api-inventory.toml`.
  - Test Cases to Write:
    - `declared_option_keys_are_documented_for_every_public_api`: the guard
      itself; passes on the current tree.
    - Mutation check (run, not committed): drop `moduleSpecifier` from the
      completion page frontmatter and confirm the guard fails; re-add an
      ignored key to `completion.d.ts` and confirm it fails again.
  - Outcome (2026-09-23):
    - Guard: `tests/clay_js_api_inventory.rs::
      declared_option_keys_are_documented_for_every_public_api`. It reuses the
      existing `inventory_entries()`/`custom_property_names()` helpers and
      `ClayJsApiRegistry::from_docs` (no second parser), walks every public
      inventory entry whose facade declares an `options`/`declaration`
      parameter with a locally declared object type, and requires every
      non-`never` member in **both** the machine-readable `custom_properties`
      (page frontmatter + `api-inventory.toml`, dotted paths covering nested
      members) **and** a bullet/code-span-led `## Options` line. Coverage is
      asserted: 236 declared keys across 59 APIs on the fixed tree (the guard
      fails if it stops walking the surface), including the three named APIs.
      `never` members are the denial list; positional and `unknown` parameters
      declare no surface; `Record<string, unknown>` members are named options
      with an open value type and are checked (`module`, `args`). One allowlist
      entry with a reason: `syntax.serverRegisterSyntaxGrammar` (declared type
      mirrors the `clay.contributions.syntaxGrammars` manifest descriptor read
      from the host-enabled package record). No existing frontmatter↔inventory↔
      registry equality or denied-authority check was loosened.
    - Precondition, deleted from the typings because nothing reads them:
      `completion.d.ts`'s eleven metadata keys (`completionProvider`,
      `contribution`, `providerId`, `triggerCharacters`, `triggers`,
      `wordBoundaryChars`, `items`, `priority`, `exclusive`, `timeoutMs`,
      `maxItems` — the op reads only `exportName`, `moduleSpecifier`,
      `runtimeBridge`); the nested `module?: string` on
      `LanguageIntelligenceProviderDeclaration` (the op reads `moduleSpecifier`
      and the facade binds only the top-level `module` object);
      `decorations.d.ts`'s `behaviorVersion` (also dropped from the JS test
      fixture); `application.d.ts` gained the documented `reason` beside
      `force`.
    - Precondition, documented surface completed: the audit found the drift was
      bigger than plan 127's hand-found case, so the sweep added 58 custom
      properties, 20 `## Options` listings, and 39 `## Custom properties`
      mirror lines across 25 APIs — op-read keys that were invisible to the
      machine-readable surface (`documents.*` path/version options, `agent.*`
      daemon options incl. the missing `agent.commandDispatch.args`, `modes`
      displayName/defaultFontRole/shebangPatterns/contentProbes and the
      facade-cached commands/keymaps, `ui` style/action, `theme` specifier/
      appearance/typography profiles, `workspace` rootId/relativePath,
      `folding`/`diagnostics` currentDocumentVersion, `behavior` caretStyle/
      movement, `language` provider/budgets.timeoutMs). Two stale documented
      names were real bugs the guard caught: the parse op reads **`mode`** (the
      registry pinned `modeId`, which it never reads) and the decorations op
      requires **`viewport`** (the registry pinned `viewportByteRange`); the
      page-level `clay_js_doc_registry` test that pinned `modeId`/
      `viewportByteRange` was corrected to the names the ops read.
    - Mutation checks (run by hand, reverted; logs in
      `test-plan/artifacts/136-capability-grants/option-surface/`): dropping
      `moduleSpecifier` from the completion page frontmatter fails with "listed
      under ## Options but is missing from custom_properties"; re-adding
      `priority?: number` to `ServerRegisterCompletionProviderOptions` fails
      with "documented in neither custom_properties … nor a ## Options
      listing". Both reverts restore the passing guard (236 keys / 59 APIs).
    - Delegated item resolved (plan 131 task 6 in this plan's `## Further
      Actions`): `inventory_rust_paths_name_existing_source_files` now checks
      `deno_op_path` too and exempts only fields carrying the explicit
      `planned:` marker, and `application.quit`'s never-written
      `deno_op_path` carries that marker in the inventory and page frontmatter
      (evidence: `mutation-3-planned-row-without-marker.txt`).
    - Code quality: no new dependency, no second doc parser, no changes to
      `tests/clay_js_facade_layout.rs` (the removed keys were option properties, not
      exports). `docs/generated/clay-js-api-registry.json` regenerated with
      `cargo run --bin update-doc-registry`; the inventory↔frontmatter↔registry
      equality test passes unchanged.
    - Performance: documentation/inventory tests only; no runtime cost.
    - Security: the guard only adds a facade-side documentation direction; it
      never loosens the denied-authority checks, and the added metadata carries
      inert option names/types/defaults only.
    - Gate: `scripts/check.sh full` PASSED (exit 0, nine stages) — log
      `test-plan/artifacts/136-capability-grants/option-surface/gate-check-full.log`.
      Suites: lib 1435, protocol 228 (+1: the new guard), presentation 62, bin 9,
      runtime 75, security 162, desktop 32+2+1+4+16, bindings 1,
      `webview bindings up to date`; 9 allowed audit warnings, unchanged.
    - Follow-ups recorded in the evidence README (documented-but-unimplemented
      direction, needs the ops as the source of truth):
      `syntax.serverRegisterSyntaxGrammar`'s descriptor options (page now says
      the manifest is authoritative), `parse` `resultBudgetBytes`, the inert
      `client*` `documentId` surface target, and the completion page's
      ignored-metadata list.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: `packages.authorize` is promoted from `planned` to
      `runtime-backed` in `api-inventory.toml`, the missing
      `docs/reference/clay-js-api/packages/authorize.md` page exists with the
      full required section set (stable ID, user-facing name, key bindings,
      custom properties, what/why/when, JavaScript usage, example, options,
      return/async, errors, permissions/security, backing Rust, op wrapper,
      facade path, lookup tags), `docs/index.md`/the API index links it, and
      `docs/generated/clay-js-api-registry.json` is regenerated.
    - Performance: the new page documents the hot-path position (grant work is
      install/enable/load/reload/explicit-command only).
    - Code Quality: the new API and every Rust public function added by this
      plan are inventoried; any function that should not be public is
      `pub(crate)`; the dotted-ID rule holds (`packages.authorize` is an
      existing bare core domain).
    - Security: the page states the denied authorities and that a grant
      authorizes only separately implemented documented APIs; it states that
      packages cannot call this API (trusted domain/config/CLI only) and that
      grants are revocable and fail closed.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`;
        `docs/reference/clay-js-api/packages/load-package.md` (sibling);
        `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`.
    - Options Considered:
      - Keep `packages.authorize` planned and document only the CLI —
        rejected: the inventoried API is the documented user-configuration
        surface, and `init.js` must be able to use it.
    - Chosen Approach:
      - Promote the API, write the page from the implementation, regenerate the
        registry, and run the API/doc guard suites.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test protocol clay_js_api_inventory clay_js_doc_registry
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/packages/authorize.md` (new),
        `docs/reference/clay-js-api/api-inventory.toml`,
        `docs/generated/clay-js-api-registry.json`, `docs/index.md` (link if
        missing), `docs/development/tauri-react-parity-ledger.json` (API list,
        if the guard requires it).
    - References:
      - `docs/reference/clay-js-api/api-inventory.toml:2127` (the planned entry
        this task implements).
  - Test Cases to Write:
    - Existing guards: `clay_js_api_inventory`, `clay_js_doc_registry`,
      `clay_js_facade_layout`, `documentation_coverage` all green with the new
      entry; `generated_registry_is_current` after regeneration.
  - Outcome (2026-09-23):
    - Verified rather than rewritten: task 4's implementation work already
      landed the entry, so this task audited it against every acceptance line
      and fixed the one gap it found.
    - Functional: `packages.authorize` is `status = "runtime-backed"` in
      `api-inventory.toml` with `deno_op_path =
      src/server/ops/packages.rs::op_clay_packages_authorize` and
      `backing_rust = op_clay_packages_authorize; PackageService::
      authorize_package; PackageAuthorizationRecord`; the page exists at
      `docs/reference/clay-js-api/packages/authorize.md` and carries all 14
      `REQUIRED_DOC_SECTIONS` (Summary, Description, When to use, JavaScript
      usage, Example, Options, Key bindings, Custom properties, Return and
      async behavior, Errors, Permissions and security, Agent guidance,
      Backing implementation, Lookup metadata) plus the ```ts usage for
      `clay:packages`; `docs/index.md` links the page (line 145) and the
      inventory's docs-index↔registry equality holds;
      `docs/generated/clay-js-api-registry.json` regenerated and
      `generated_registry_is_current` passes.
    - Performance: the one missing acceptance item — the page had no hot-path
      statement. Added a Description paragraph in the sibling convention
      (`Authority: user-authorized-package-capability-grant`, `Runtime path:
      server-side-package-authorization-grant`) stating grant work is
      install/enable/load/reload/explicit-user-command only and that the
      enforcement read is a cheap check against already-loaded authorization
      state, never on keypress/paint/layout/scroll/text-event/edit-ack/pointer/
      client hot paths — matching the inventory `hot_path_policy`.
    - Code quality: every Rust item this plan added was enumerated from the
      plan-136-tagged diff and classified. Two were unnecessarily `pub` and are
      now `pub(crate)`: `JsRuntimeLaneMetrics` + `JS_RUNTIME_LANE_METRICS`
      (`src/perf/metrics.rs`, only `src/server/js_runtime` reads them) and
      `RuntimeProfile::parse` (`src/packages/authorization.rs`, callers are the
      op, verbs, and approvals). Both are locked by
      `tests/rust_visibility_api_mapping.rs::
      plan118_new_runtime_machinery_stays_crate_private`, whose list now carries
      the plan-136 entries. Already correct: `pub(super)
      op_clay_packages_authorize`, `pub(crate)` `capability_granted`,
      `CLI_USAGE`, `record_lane_metrics`, and `pub(crate)
      PackageApprovalStore` (so `record_grant`/`current_record`/
      `current_grant` are unreachable outside the crate). Deliberately still
      `pub` because the external CLI/package suites consume them:
      `CapabilityGrant`, `PackageInspection::grant_provenance` +
      `GrantProvenance`, `verbs::authorize`, `format_grant_lines`,
      `PackageService::authorize_package` (recorded in the visibility test's
      comment). No new `pub` item was added by this task.
    - Dotted-ID rule: `id == "{js_module}.{js_export}"` is enforced by
      `every_public_api_has_generic_sections_facade_and_naming_contract`;
      `packages.authorize` = `clay:packages` + `authorize`, an existing bare
      core domain, flat lowerCamelCase, `op_clay_*` wrapper, no raw op export.
      The sibling rows (`install`, `enable`, `disable`, `inspect`, `list`,
      `setConflictOverride`) stay `planned` with `planned:` markers — their ops
      are not written, and revoke is CLI-only today.
    - Security: the page's Permissions and security section states the
      trusted-only facade (absent from the shared third-party runtime) plus the
      activation refusal, the full denied-authority list, that a grant
      authorizes only separately implemented and separately validated
      documented APIs, and that grants are provenance-bound, revocable, and
      fail closed with `MissingCapabilityGrant`; the frontmatter `security:`
      field carries the same statements for the machine-readable surface.
    - Gate: `scripts/check.sh full` PASSED (exit 0, nine stages) — log
      `test-plan/artifacts/136-capability-grants/option-surface/gate-check-full-task9.log`
      (protocol 228, security 162 including the extended visibility mapping,
      lib 1435; 9 allowed audit warnings, unchanged).

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: the grant surface is usable from `~/.clay/init.js` as a
      documented Clay JS API (`authorize(...)`), with a documented ordering
      constraint (grant before `loadPackage`), and the configuration guide
      shows the package-author/reviewer example; no undocumented JSON/TOML key
      can grant a capability.
    - Performance: configuration-time only; documented as such.
    - Code Quality: every behavior-changing option of `authorize` is in
      `custom_properties` and the page's `## Options` section; the new guard
      from this plan covers the page.
    - Security: configuration never implicitly grants filesystem, network,
      shell, extension loading, AI mutation, or workspace authority; grants made
      in `init.js` are attributed (`approvedBy: "config"`) and remain
      revocable; a package cannot call `authorize` to change its own record.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md`;
        `docs/reference/configuration.md` (or the current configuration guide);
        plan 115 task on configuration APIs.
    - Options Considered:
      - CLI-only grants — rejected: users configure Clay in `init.js`, and the
        same action must be expressible there with attribution.
    - Chosen Approach:
      - Document the `init.js` usage, ordering, and attribution; add a
        configuration-guide example; rely on the gate from the previous task.
    - API Notes and Examples:
      ```js
      import { authorize, loadPackage } from "clay:packages";

      authorize({ package: "@vendor/words", capabilities: ["completion-provider"], approvedBy: "config" });
      await loadPackage("@vendor/words");
      ```
    - Files to Create/Edit:
      - `docs/reference/configuration.md` (or the current configuration
        reference page), `docs/reference/clay-js-api/packages/authorize.md`.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
  - Test Cases to Write:
    - Documentation gate coverage for the new page (existing suites); a
      configuration test that `authorize` before `loadPackage` succeeds and a
      package self-grant attempt fails.
  - Outcome (2026-09-23):
    - Functional: the grant is usable from `~/.clay/init.js` through the
      documented `packages.authorize` API, with the ordering constraint
      documented (install → adopt → `authorize` → `loadPackage`; a grant made
      before adoption stays in-memory and enable still fails closed with the
      adoption diagnostic). The existing task 3 config test
      `config_authorize_grants_declared_capability_and_enables_package` already
      proves the order end to end from a real config root (grant + `loadPackage`
      succeed, enable before the grant fails with `MissingCapabilityGrant`, the
      call is idempotent).
    - Configuration guide: new section `## Plan 136 third-party capability grant
      configuration review` in `docs/reference/clay-js-api/configuration.md` —
      surfaces table (config grant / CLI grant / CLI-only adoption+revocation /
      `loadPackage` consumption / compiled enforcement / internal durable
      storage), the reviewer `init.js` example (package-author side documented
      as the manifest declaration), the rejected-hidden-key list
      (`capabilityGrant`, `packages.authorizedCapabilities`, …), the
      trusted-only + `approvedBy: "config"` attribution statement, and the
      configuration-time-only hot-path statement. The plan 060/061 closure
      table gained the grant row, and its “JavaScript cannot approve itself”
      paragraph now covers capability grants.
    - No hidden key grants a capability: the guide and the page state it, the
      manifest route is validated (grant-only authorities are refused in
      `clay.permissions`; every granted capability must be declared), and
      enforcement fails closed — there is no key that turns
      `MissingCapabilityGrant` off.
    - Performance: documented as configuration-time only in both the guide and
      the page (task 9's `Authority:`/`Runtime path:` paragraph, matching the
      inventory `hot_path_policy`).
    - Code quality: `tests/clay_js_api_inventory.rs::
      plan136_configuration_documents_the_capability_grant_surface` pins the
      guide markers (section, ordering, attribution, rejected keys,
      trusted-only, hot-path) plus the page's authority/hot-path/
      separately-implemented-API statements and a non-empty
      `custom_properties` set. The task 8 guard already walks
      `packages.authorize`'s declared options with every other public API (236
      keys / 59 APIs), so the option-surface half of this criterion is covered
      without a duplicate check.
    - Security: new test `src/server/js_runtime/tests/package_adoption.rs::
      package_code_cannot_self_grant_capabilities_during_activation` — a
      package evaluation in the trusted domain (where `clay:packages` is
      importable) calls `authorize` while the activation scope is open and must
      be refused with `packages.grant_during_activation`. Mutation check
      (evidence `mutation-self-grant-gate-removed.txt`): deleting
      `ensure_grant_authority_open` from the op makes the test fail with
      `self-granted:true`, so the gate is load-bearing; reverted immediately.
      In the shared third-party runtime the facade is absent entirely
      (`Facade::trusted("clay:packages")`), pinned by the facade-allowlist
      guards, so package code cannot reach the op at all. Grants from `init.js`
      are attributed (`approvedBy: "config"`) and withdrawn by
      `clay package revoke` together with the approval.
    - Gate: `scripts/check.sh full` PASSED (exit 0, nine stages) — log
      `test-plan/artifacts/136-capability-grants/config-surface/gate-check-full-task10.log`
      (protocol 229 incl. the guide test, lib 1436 incl. the self-grant test,
      security 162; 9 allowed audit warnings, unchanged).

- [x] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: the example config gains a commented capability-grant section
      (granting, inspecting, and revoking a third-party package) with every
      option name, allowed value, and default annotated; the active section
      stays safe to copy verbatim.
    - Performance: comments only; startup cost unchanged.
    - Code Quality: `node --check examples/config/init.js` passes; the section
      follows the file's existing documentation style; option names/defaults
      match the validated parser, not prose.
    - Security: the example never grants filesystem/network/shell/AI/workspace
      authority in the active path; powerful capabilities stay commented with
      instructions.
  - Approach:
    - Documentation Reviewed:
      - `examples/config/init.js` (current style and ordering rules);
        `docs/reference/clay-js-api/packages/authorize.md`;
        `create-plan/references/clay.md` → Example Configuration Maintenance
        Task.
    - Options Considered:
      - Grant a real third-party package in the active path — rejected: the
        canonical config must not depend on an external package.
    - Chosen Approach:
      - Commented grant section next to the package-loading section.
    - API Notes and Examples:
      ```js
      // Capability grants — explicit, per capability, revocable.
      // authorize({ package: "@vendor/words", capabilities: ["completion-provider"], approvedBy: "config" });
      // await loadPackage("@vendor/words");
      ```
    - Files to Create/Edit:
      - `examples/config/init.js`.
    - References:
      - `examples/config/packages/` (fixture packages loaded by the example).
  - Test Cases to Write:
    - `node --check examples/config/init.js`; the existing example-config test
      suite (`tests/example_config_control_center_chord.rs` precedent) stays
      green.
  - Outcome (2026-09-23):
    - Functional: the canonical example tree gained a commented capability-grant
      section. It lives in `examples/config/packages/third-party.js` — the module
      init.js section 11 loads for third-party configuration — because that is
      where the install/remove/update/adopt contract already lives, and init.js
      section 11 now points at it and names the `clay package authorize` verb.
      The adoption step list was corrected: grant is now step 3 and `loadPackage`
      step 4, so the template no longer teaches a flow that fails closed with
      `MissingCapabilityGrant` for any capability-declaring package. The section
      annotates the CLI form (repeatable `--capability`; `--runtime-profile`
      `native-trust` default | `sandboxed` | `restricted`; `--approved-by` `cli`
      default | `user` | `config`), the `authorize({ package, capabilities,
      runtimeProfile, approvedBy })` form attributed `config`, the replacement
      semantics (a later grant replaces the whole set), the inspect output labels
      (`Grants: <names> (<profile>)`, `Granted by:`, `Approved by:`, `Ungranted:`),
      `clay package revoke` withdrawing approval and grant together, the rejected
      hidden keys, and the self-grant refusal. `examples/config/README.md`'s
      third-party row describes the surface.
    - Option accuracy: every name, allowed value, and default was taken from
      `CLI_USAGE` and `verbs::authorize`/`verbs::format_grant_lines`, not prose;
      a real third-party package is never granted in the active path (no
      third-party package ships with Clay), and the plan's rejected option
      (granting a real external package) is recorded in the template as a
      commented example only.
    - Performance: comments only. The only uncommented line in the third-party
      template is still the pre-existing `import { loadPackage } from
      "clay:packages";` (`node-check-and-active-lines.txt`), so startup cost and
      the active configuration are unchanged; the gate's example-config boot test
      evaluates the shipped tree unchanged.
    - Code quality: `node --check` passes for `init.js` and both package modules;
      the section follows the file's banner/indent style; and
      `tests/clay_js_api_inventory.rs::
      plan136_configuration_documents_the_capability_grant_surface` now also pins
      13 template markers plus the init.js pointer, so the documentation cannot
      silently rot.
    - Security: the whole section is commented, so the copy-safe scan
      (`tests/clay_js_doc_registry.rs::
      canonical_example_active_configuration_is_copy_safe`) stays green and no
      filesystem/network/shell/AI/workspace authority appears in an active line.
      The text states that only declared capabilities can be granted, that
      unknown names and grant-only authorities are refused, that `clay:packages`
      is absent from the third-party runtime (package code cannot self-grant),
      and that revoke withdraws the grant.
    - Gate: `scripts/check.sh full` PASSED (exit 0, nine stages) — log
      `test-plan/artifacts/136-capability-grants/example-config/gate-check-full-task11.log`
      (lib 1436, protocol 229 incl. the example-config boot test, security 162).

- [x] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: a real Linux GUI build launches against a **copy** of
      `examples/config/init.js` in a scratch config root; the client reaches
      Connected, configuration evaluation commits a generation with no
      `configuration failed` diagnostics, and the shell responds (open a pane,
      run a command, open the menu).
    - Performance: startup completes in the recorded baseline envelope; the copy
      adds no measurable startup cost versus the baseline run.
    - Code Quality: the launch command, scratch config path, and observed
      results are recorded in task evidence.
    - Security: the launch uses an isolated scratch `HOME`/`.clay`, never the
      developer's profile; no real credentials or user packages are involved.
  - Approach:
    - Documentation Reviewed:
      - `create-plan/references/clay.md` → Example Configuration Live
        Launch-Test Task; `docs/development/launch-and-gui-smoke.md`.
    - Options Considered:
      - Skip the live run because the change is a comment — rejected: the plan
        changes the documented grant path, and the launch test is the only
        check that the example config is actually runnable.
    - Chosen Approach:
      - Copy the config, launch the GUI, exercise shell interaction, and record
        evidence (or record the exact blocker if the environment forbids a GUI
        run, per the task's fallback rule).
    - API Notes and Examples:
      ```bash
      HOME="$(mktemp -d)" CLAY_CONFIG_ROOT="…" cargo tauri dev   # or the documented launch command
      ```
    - Files to Create/Edit:
      - Task evidence under `code-reviews/2026-09-20-plan136-baseline/` (or the
        plan's live-verification path).
    - References:
      - `docs/development/launch-and-gui-smoke.md`,
        `test-plan/artifacts/127-lane-scheduling/launch-live.sh` (harness
        precedent).
  - Test Cases to Write:
    - Live launch: Connected + generation committed + shell interaction
      recorded with screenshots.
  - Outcome (2026-09-23):
    - Functional: the real Linux GUI build launched against a copy of the
      canonical tree and the shell answered every interaction. The harness gained
      an `example-config` mode (`test-plan/artifacts/127-lane-scheduling/run-live.sh`)
      that copies `examples/config/.` verbatim into the scratch `HOME/.clay/` (the
      guide's own `cp -r` command), and `drive-example-config.sh` drives it. The
      client reached **Connected** (`window-initial.png` status bar; `client.log`
      has only GTK locale warnings), configuration evaluation committed with no
      `configuration failed` diagnostic (`config-failure-count.txt` = 0 lines for
      `configuration failed|configuration.module_failed|load_failed|MissingCapabilityGrant|panicked`,
      plus the `[agent-reg]` lines that only run once config JavaScript evaluates),
      and the config's effects are visibly live: gruvbox-material-dark theme,
      `@clay/markdown` prose mode with a 200-entry outline, the coding-agent lane,
      the file browser, and the example's chord hints.
    - Shell interaction (all screenshots + AT-SPI trees in
      `test-plan/artifacts/136-capability-grants/live-example-config/`): the header
      **Control Center** button pressed via AT-SPI opened the command palette
      (`window-menu.png`); typing filtered it to the selected row `Add Equal Pane
      client — built-in Ctrl+Shift+\` (`window-menu-filtered.png`); the row's own
      chord ran it and **opened a pane** (editor extents 1322 → 763 px, pane nodes
      5 → 8); the example config's own **Ctrl+B** binding toggled the file browser
      (node count 1 → 0); and typing ` ok` plus the toolbar **Save** button (AT-SPI
      press) round-tripped an edit to disk (`notes.md` now begins ` ok# heading 0`,
      fresh mtime). `probe.py` gained a `click <name>` command for AT-SPI action
      presses; every other input is real key synthesis behind the compositor-focus
      guard.
    - Performance: startup measured fresh on the copy (no baseline startup number
      existed — task 1 recorded the edit-ack envelope): socket ready 429 ms, first
      config effect 478 ms, editor visible 1.30 s, and the perf report shows
      `runtime.load_configuration_with_workspace` once at p50 67.9 ms for the whole
      canonical evaluation (bundled package loads included) with
      `evaluate_controlled_module` at 1.9 ms. `server.edit_ack` over the three
      typed edits was p50 194.7 µs / p95 342.5 µs against the task 1 baseline of
      p50 231.8 / p95 282.1 µs — no regression. Caveat recorded: the same-harness
      `markdown` control mode did not expose its window to AT-SPI inside the poll
      window because its client window was not on the active compositor tag (a
      harness artifact — the driver switches tags, the timing wrapper does not), so
      the copy's own numbers are the measurement.
    - Code quality: the launch command, scratch root, isolation rules, and observed
      results are recorded in `live-example-config/README.md` with per-check
      evidence files (`startup-timing.txt`, `interaction-summary.txt`,
      `perf-summary-example-config.txt`, `config-failure-count.txt`, seven window
      screenshots, and the AT-SPI trees behind each claim).
    - Security: the launch used a private mode-700 root for `HOME`,
      `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `TMPDIR`, the store, the workspace, and
      the socket, so the developer's `~/.clay` was never opened; the store started
      empty and the third-party module is a commented template, so nothing was
      installed, adopted, or granted, and no credentials were involved (the
      canonical config sets no provider — the status bar reads `no provider
      configured`).
    - Findings recorded, not fixed here (plan 136 changes no menu or chord code;
      both are pre-existing): (1) with editor focus the two-stroke `Ctrl+X Ctrl+O`
      resolves the palette *and* lets the single-stroke `Ctrl+O` binding fire, so
      the file dialog opens too (status bar `File dialog could not open` once the
      portal dialog was killed) — double dispatch; (2) Enter on the selected
      palette row (footer `↵ run`) left the palette open and ran nothing, with the
      status bar reporting `no active menu session for id 9223372036854775809
      (client 2)`. Both moved to `## Further Actions`.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: module 09 step P56 is flipped from the plan-127
      fail-closed-only check to a positive grant → enable → provider path (and
      keeps the negative no-grant sub-step); new numbered steps cover the CLI
      grant verb, the `authorize` config call, and third-party latency-lane
      completion responsiveness with the general lane busy; affected steps are
      executed on a real Linux build with pass/fail recorded.
    - Performance: the latency-lane step records perceived completion latency
      and the `server.edit_ack` p50/p95 envelope from `CLAY_PERF_REPORT_DIR`
      (plan 127 module 11 Q42 precedent).
    - Code Quality: steps include expected results, negative checks, and known
      ceilings; `test-plan/index.md` module map/coverage matrix and the
      `docs/development/tauri-react-parity-ledger.json` step references are
      updated in the same task.
    - Security: the negative steps prove fail-closed without a grant, that a
      granted package still cannot use trusted-only ops, and that revocation
      returns the system to fail-closed.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map/coverage matrix);
        `test-plan/04-core-editing.md`, `test-plan/09-packages-and-modes.md`,
        `test-plan/11-performance.md` (steps added by plan 127);
        `tests/documentation_coverage.rs` (step-ID ledger rule).
    - Options Considered:
      - Leave P56 as a negative-only step — rejected: the gap this plan closes
        is exactly what made P56 negative-only.
    - Chosen Approach:
      - Extend P56 with sub-steps, add the grant-surface and latency-lane
        steps, execute them, and update the ledger references.
    - API Notes and Examples:
      ```bash
      cargo run -- server --config-fixture plan136-grants &
      # E40/Q42-style keystroke probe against the granted fixture mode
      ```
    - Files to Create/Edit:
      - `test-plan/09-packages-and-modes.md`, `test-plan/11-performance.md`,
        `test-plan/index.md`, `docs/development/tauri-react-parity-ledger.json`.
    - References:
      - `test-plan/artifacts/127-lane-scheduling/README.md` (harness + ceilings
        precedent).
  - Test Cases to Write:
    - P56 positive: grant → enable → provider contribution visible in the
      editor.
    - P56 negative: no grant → `MissingCapabilityGrant`, no contribution.
    - New latency-lane step: completion served while the package parse handler
      is busy, with recorded latency.
  - Outcome (2026-09-23):
    - Functional: the manual plan now carries the positive grant path. P56 was
      flipped from the plan-127 fail-closed-only check to grant → enable → load
      (with the no-grant sub-step kept verbatim), and two steps were added: P57
      for the `clay package authorize`/`revoke` lifecycle and P58 for the config
      `authorize` call. All were executed on a fresh Linux build
      (`cargo build --bin clay -p clay`, `cargo build -p clay-desktop`) through
      the isolated harness. P56 positive: `Authorized @fixture/lane:
      completion-provider, mode-registration, parse-document (native-trust)` /
      `granted by: cli` → `Enabled @fixture/lane` → the live launch logged zero
      `configuration failed`/`packages.load_failed` lines with the client
      connected and typing landing (29 → 35 chars), and the fixture's
      registrations show up on their lanes (`third_party.latency.dispatched` 3 =
      module-backed completion provider, `…general.dispatched` 5 = parse
      handler). P56 negative: the same fixture adopted but never granted still
      fails closed with `Error: MissingCapabilityGrant { package_name:
      "@fixture/lane", capability: CompletionProvider }` plus the sanitized
      `configuration failed [packages.load_failed]: JavaScript runtime evaluation
      failed.` line and no contribution. P57: inspect gained and lost the
      `Grants:`/`Granted by:` lines across `authorize`/`revoke`, a second
      `authorize` replaced the whole set (re-granting only `mode-registration`
      dropped the other two and `enable` failed closed again), an undeclared
      capability was refused (`does not declare capability package-control in
      its manifest`), `authorize` on a revoked record refused to manufacture an
      approval, and `revoke` returned the system to fail-closed with
      `AdoptionRequired { code: "package_approval.revoked" }`. P58: an init.js
      `authorize({… approvedBy: "config"})` wrote
      `grant {capabilities, runtime_profile: native-trust, granted_by: config,
      granted_at}` into the store, a **separate** `clay` process printed
      `Grants: … (native-trust)` / `Granted by: config` and enabled
      successfully, and the relaunch on the same store loaded the package with
      no failure lines.
    - Performance: module 11 gained Q44 (third-party latency-lane completion
      while the package parse handler holds the general lane busy). Executed:
      `cargo test --lib lanes_and_queues -- --nocapture` (17 tests) printed
      `PLAN136_GRANTED_LANE busy_ms=500 idle_median_us=1619
      busy_completion_us=2937 workers_started=4` (inside the plan-127 record of
      1567/2594 µs) with `PLAN127_LANE busy_ms=500 idle_median_us=2278
      busy_completion_us=2852`, and the live granted run measured
      `server.edit_ack` p50 165.0 µs / p95 191.3 µs over 6 typed characters
      (baseline p50 231.8 / p95 282.1 µs) with `server.document.apply_edit`
      p50 42.6 µs. The new `js_runtime.lane.*` occupancy counters appear in the
      live summary for the first time (all 20 keys, 175 retained events, 0
      dropped).
    - Code Quality: `test-plan/09-packages-and-modes.md` carries P56 (positive +
      negative sub-step + trusted-only-op and mode-activation ceilings), P57 and
      P58 with expected results, negatives and ceilings, and its plan-127
      section was reduced to the historical record; `test-plan/11-performance.md`
      carries Q44; `test-plan/index.md` gained the module-map text for 09/11, a
      plan-136 coverage-matrix row (with the automated companion test names) and
      a `## Plan 136 manual-test-plan execution record (2026-09-23, task 13)`
      section; `docs/development/tauri-react-parity-ledger.json` references P57,
      P58 under `packages.modes.settings.themes` and Q44 under
      `performance.budgets.feel` (the step-ID ledger rule passes).
    - Security: the negative steps prove all three required fail-closed paths —
      no grant (`MissingCapabilityGrant` + sanitized `packages.load_failed`, no
      contribution), revocation (`AdoptionRequired { code:
      "package_approval.revoked" }`, `grant: null` in the store), and no
      self-grant (`clay:packages` is trusted-domain-only, and the automated
      `package_code_cannot_self_grant_capabilities_during_activation` pins the
      `packages.grant_during_activation` refusal). A granted package keeps its
      own domain op set (`third_party_lane_denies_trusted_ops`), and
      `grant_is_inert_after_provenance_change` pins fail-closed after a
      reinstall. Every run used a private mode-700 root for
      `HOME`/XDG/`TMPDIR`/socket, so no developer profile, credential, or
      non-scratch store was touched.
    - Ceilings recorded, not hidden: the live completion popup still cannot be
      driven (package-owned modes do not activate for open documents and a
      package mode rejects the built-in `completion.trigger` command), so Q44's
      latency numbers come from the automated lane harness while the live leg
      contributes the edit-ack envelope and the occupancy counters; the
      ungranted run's lane counters come from the bundled packages its config
      loads, not from the ungranted fixture.
    - Gate: `scripts/check.sh full` PASSED exit 0 (nine stages) with lib 1436
      passed / 1 ignored, bin 9, presentation 62, protocol 229, runtime 75,
      security 162, desktop 32+2+1+4+16, bindings 1, and the 9 allowed audit
      warnings — log
      `test-plan/artifacts/136-capability-grants/manual-plan/gate-check-full-task13.log`.
      Harness additions: `run-live.sh` mode `config-granted-lane` (with
      `CLAY_LIVE_KEEP_ROOT=1` keeping the store across launches) and the fixture
      config `init-config-granted-lane.js`. Evidence index:
      `test-plan/artifacts/136-capability-grants/manual-plan/README.md`
      (`granted/`, `granted-typed/`, `ungranted/`, `config/`,
      `automated-lane.txt`).

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/third-party-runtime-authority.md` (or the
      page that owns package authority) documents the grant surface — CLI verb,
      `authorize` API, trusted-domain-only reachability, provenance binding,
      revocation, and the fail-closed default — including the lane-observability
      counters and how to read them.
    - Performance: wiki updates add no runtime work; the lane-occupancy
      measurement and tuning decision are documented with their numbers.
    - Code Quality: pages explain what changed, how it works, invariants and
      tradeoffs, source/test paths, and are linked from `docs/wiki/index.md`.
    - Security: the pages document the touched trust boundary (no self-grant,
      trusted-domain-only op, capability-by-capability approval, revocation
      behavior) without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`;
        `docs/wiki/modules/third-party-runtime-authority.md`;
        `docs/wiki/modules/persistent-runtime-hardening.md`.
    - Options Considered:
      - Update per task — rejected (noisy); update once after tests pass.
    - Chosen Approach:
      - Update after all implementation tasks pass, plus the index navigation.
    - Files to Create/Edit:
      - `docs/wiki/modules/third-party-runtime-authority.md`,
        `docs/wiki/modules/persistent-runtime-hardening.md` (lane counters),
        `docs/wiki/index.md`.
    - References:
      - Plan 127's wiki section (lane topology) and its contract test
        `plan127_wiki_pages_describe_lane_scheduling_and_the_trust_domain_constraint`.
  - Test Cases to Write:
    - Manual wiki review; `tests/documentation_coverage.rs` wiki contract tests
      stay green (including the plan-127 lane contract).
  - Outcome (2026-09-23):
    - Functional: `docs/wiki/modules/third-party-runtime-authority.md` gained a
      `## Capability Grants` section documenting the whole surface — the four
      read surfaces behind the single `PackageService::capability_granted`
      path, provenance matching, the three grant sources (the Clay JS
      `authorize` API linked to
      `docs/reference/clay-js-api/packages/authorize.md`, the
      `clay package authorize` CLI verb with its defaults and its refusal of
      unadopted packages, and the `native-trust`/`sandboxed`/`restricted`
      runtime profile), what a grant can never do (no self-grant:
      `clay:packages` is `Facade::trusted` and
      `ensure_grant_authority_open` refuses with
      `packages.grant_during_activation`; no undeclared capability; no widening
      by repetition; no approval), revocation clearing the grant (`grant: null`)
      and returning to `AdoptionRequired { code: "package_approval.revoked" }`,
      and a pointer to the lane counters. Its Source list gained
      `authorization.rs`, `op_clay_packages_authorize`, `facades.rs`,
      `packages.js` and the CLI verb; Invariants gained the grant rules; Tests
      gained the durable-grant, CLI-verb and self-grant commands; Related gained
      `authorize.md`, the configuration guide and `performance.md`.
      `docs/wiki/modules/persistent-runtime-hardening.md` gained a
      `### Lane occupancy counters` subsection: the five counters with where
      each is bumped, the 20 static names from `JS_RUNTIME_LANE_METRICS`, why
      the counters are atomic on the dispatch path and recorded once at report
      time (`&'static str` names, capped snapshot buffer, `MetricSummary.total`),
      a runnable `CLAY_PERF_PROFILE`/`CLAY_PERF_REPORT_DIR` recipe for reading
      them, the measured granted-fixture numbers, the client-free comparison
      finding, the burst-test peak/supersede/evict numbers, and the tuning
      decision with its revisit triggers. `docs/wiki/index.md` now describes the
      capability-grant surface and the occupancy counters in its module map.
    - Performance: wiki updates add no runtime work (Markdown only). The
      lane-occupancy measurement and tuning decision are documented with their
      numbers in the hardening page (`trusted.general` 1–2,
      `third_party.general` 5, `third_party.latency` 3, other lanes 0, 0 dropped;
      burst `peak_pending=1`/`superseded=38` and `peak_pending=64`/`evicted=7`;
      latency ~1.6–2.3 ms idle / ~2.6–3.6 ms busy against a 500 ms busy general
      lane; `JS_RUNTIME_LANES_PER_DOMAIN = 2` and the 32 MiB latency ceiling
      kept).
    - Code Quality: both pages state what changed, how it works, the invariants
      and the tradeoffs, with source and test paths and links to the
      authoritative reference docs instead of duplicating them; both are linked
      from `docs/wiki/index.md`. A new contract test
      `tests/documentation_coverage.rs::plan136_wiki_pages_describe_capability_grants_and_lane_counters`
      pins 17 markers across the two pages and the index, and a stale-claim scan
      over `docs/wiki/modules` and `docs/wiki/flows` fails if the pre-plan-136
      state (“no user-facing entry point for grants”, “no capability grant
      surface”, …) reappears; two mutation checks (renaming the grant heading,
      replacing the counter identifiers) each fail it with the missing marker
      named. The plan-127 wiki contract test stays green.
    - Security: the pages document the touched trust boundary without secrets —
      no self-grant (trusted-domain-only facade plus the activation-scope
      refusal), capability-by-capability approval (the grant must stay inside the
      adoption ceiling and the manifest's declarations), provenance binding (a
      grant is inert once the record's identity no longer matches the installed
      package), revocation returning to fail-closed, and the fail-closed default
      (`RuntimeProfile::parse` rejects unknown profiles).
    - Gate: `scripts/check.sh full` PASSED exit 0 (nine stages) with lib 1436
      passed / 1 ignored, bin 9, presentation 62, protocol 230 (+1 for the new
      wiki contract test), runtime 75, security 162, desktop 32+2+1+4+16,
      bindings 1, and the 9 allowed audit warnings — log
      `test-plan/artifacts/136-capability-grants/wiki/gate-check-full-wiki.log`;
      evidence index
      `test-plan/artifacts/136-capability-grants/wiki/README.md` plus
      `mutation-checks.txt`. With this task the plan's Compromises Made section
      was filled in, and no checkbox remains open.

## Compromises Made
- **Grants are per-package sets, not per-capability toggles.** `authorize`
  replaces the whole granted set, so revoking one capability means re-granting
  the rest. This kept the durable record and the op surface small (one
  `CapabilityGrant`, one `--capability` repeatable flag) at the cost of a
  coarser verb; a per-capability grant/ungrant pair can be added later without
  changing the store shape.
- **Adoption and grants stay separate steps.** `authorize` refuses to create an
  approval (`run clay package adopt … first`) instead of adopting implicitly,
  which keeps the "no code executes before adoption" invariant readable at the
  cost of one extra command for CLI users. The canonical example configuration
  and `test-plan` P57/P58 spell out the four-step order.
- **No GUI grant surface.** The CLI and `init.js` ship here; a GUI-only user
  still cannot grant a third-party capability. That is deliberate — an in-app
  package-manager surface is app-UI work and needs the `create-plan` UI
  prototype plus explicit user approval, so it is recorded as a Further Action
  rather than smuggled into this plan.
- **Lane occupancy is recorded once at report time, not per event.** The
  counters are atomics read on the `SIGTERM` path, so a crash or a killed
  server yields no occupancy numbers (the same limitation the existing perf
  report already had), and per-lane time series are not available. Chosen
  because the alternative — one perf snapshot per dispatched command — would
  spend the capped snapshot buffer (4096 events) on counters and slow the
  dispatch path this plan is trying to protect.
- **The tuning decision keeps the current lane topology.** `JS_RUNTIME_LANES_PER_DOMAIN
  = 2` and the 32 MiB latency ceiling are unchanged on fixture evidence
  (~1.6–2.3 ms idle, ~2.6–3.6 ms busy, backlog ≤ 1). A real-session mix (many
  providers, long sessions) is still needed before the numbers are final, and
  the revisit triggers are recorded in `docs/development/performance.md` and the
  wiki.
- **The live completion popup remains undrivable.** Package-owned mode
  activation for open documents is not wired, so the manual plan's latency step
  records the automated lane measurement plus the live edit-ack envelope and
  occupancy counters, and states the ceiling instead of claiming a live popup
  number. Fixing it is a separate plan (package mode activation).
- **Fixture packages are the only third-party test subjects.** There is no
  bundled test package, so the "a bundled load entry cannot self-grant"
  guarantee is covered by the `ensure_grant_authority_open` unit test and the
  structural admin-op denial list rather than end-to-end; a bundled test package
  would be needed to close that gap.
- **Task 12 (the launch test) was closed without a full gate run.** It changed
  no tracked production or doc source — only the harness scripts
  (`run-live.sh`, `probe.py`, validated with `bash -n` and by being exercised
  live), untracked evidence, and the plan file — so the task's own acceptance
  (the live launch) was the verification. Every other task ran the nine-stage
  gate, including the final one (task 14), which re-ran it on the finished tree.

## Further Actions

- **`PackageLoadEntryAllowlist::revoke_package` has no production caller**
  (delegated from plan 131 task 4; priority: high, security-adjacent): a
  disabled/revoked package's recorded `clay://packages/...` module entries are
  never withdrawn, so they remain resolvable for the current runtime generation.
  This plan owns the lifecycle verbs (`clay package authorize`/`revoke`, disable),
  so wire the withdrawal into the revoke/disable path here, or revalidate package
  enablement at module resolution and delete the method (it currently keeps a
  narrow `allow(dead_code, reason = …)`). Evidence paths:
  `src/server/ops/packages.rs`,
  `src/packages/service.rs` disable path, `src/server/js_runtime/mod.rs` allowlist
  records.
- **Planned inventory rows escape the Rust-path existence guard** (delegated from
  plan 131 task 6; priority: low): `api-inventory.toml` lists
  `application.quit.deno_op_path =
  "src/server/ops/application.rs::op_clay_application_quit"`, but neither that file
  nor that op exists. `status = "planned"` skips the existence check, and
  `application.quit` is the only planned row whose path lacks the explicit
  `planned:` prefix (the seven `packages.*` rows mark theirs). Task 6 (“Harden the
  Clay JS option-surface drift guard”, still unchecked) is the natural home:
  either require the `planned:` marker for all planned rows or point the row at the
  real owner (`src/client_commands.rs::EditorClientCommand`).
- **Build the in-app capability-grant surface (GUI).** The CLI and `init.js`
  entry points ship here, but the desktop app has no package-manager surface
  yet, so a GUI user's only route to a third-party capability grant is the
  CLI. That surface is app-UI work and must go through
  `create-plan/references/clay.md` → UI Prototype and Explicit User Approval:
  prototype under `design-artifacts/prototypes/<slug>/`, explicit user
  approval, then implementation citing `design-artifacts/approved/<slug>/`.
  Rationale: it is the last blocker for a GUI-only user to adopt a powerful
  third-party package.
- **Seal capability grants after the first package activation** (only if a
  mid-generation grant path ever becomes reachable). Today only trusted user
  configuration/CLI can call `authorize`, the op refuses inside package
  activation, and `ensure_capability_grants` re-checks at every enable, so no
  seal is needed; the language-server precedent (`ensure_authorization_open`)
  already shows the shape if one is ever required. Priority: low; source:
  `code-reviews/2026-09-20-plan136-baseline/primitive-review.md` §6.
- **Revisit the LANES_PER_DOMAIN/latency-heap numbers once real sessions
  exist.** The plan records a fixture measurement; a real-session mix (many
  providers, long sessions) is still needed before treating the numbers as
  final. Priority: low unless the fixture measurement already shows latency-lane
  pressure.
- **Consider capability presets for common packages.** `decision-logs/2026-08-18-1758-package-capability-presets.md`
  already approved the preset model; once grants are user-visible, a one-click
  "adopt a completion provider" preset would remove most per-capability
  friction. Priority: medium, after the GUI surface exists.
- **Activate package-owned modes for open documents.** Plan 136 task 6 could not
  finish the live third-party path: a package can register a mode
  (`serverRegisterModePattern`, granted), but nothing activates it for an open
  document, so the package's parse handler never dispatches and the client never
  receives its editor rules or completion trigger characters. Third-party
  manifest contributions are trusted-only
  (`src/server/ops/packages.rs:736-738`) and the single
  `modes.activate_major_mode` caller is the JS op, which needs an already-open
  `documentId`; there is no document-open hook and no client/protocol activation
  message. The natural shape is host-side classification/activation on document
  open for registered package modes (with the activation layer built from the
  enabled record, as the trusted path already does), plus a JS document-open hook
  if configuration code is meant to drive activation. Priority: medium — it is
  the remaining blocker for a third-party package's editor contribution to be
  visible in a live session; source: plan 136 task 6 outcome
  (`test-plan/artifacts/136-capability-grants/README.md`).
- **Widen the option-surface guard to op-level JSON keys.** The guard added
  here covers declared `.d.ts` options; a package can still pass an extra JSON
  key the facade forwards and the op ignores (the completion registration
  path does exactly that today for `providerId`/`items`/…). A follow-up can
  reject unknown keys at the op boundary for the registration ops, turning
  silent ignores into errors. Priority: low (the current behavior is
  documented).
- **Two live input findings from the plan 136 task 12 launch test.** (1) With the
  editor focused, the two-stroke `Ctrl+X Ctrl+O` chord resolves the Control Center
  palette *and* still fires the single-stroke `Ctrl+O` binding
  (`documents.clientOpenFileDialog`), so a file chooser opens on top of the
  palette; the editor keymap's `chordKeymap` (`frontend/src/editor/extensions/behavior.ts`)
  returns `true` for the completed chord, so the second stroke should not also
  reach the single-stroke keymap — worth reproducing in a unit test with a real
  two-stroke sequence. (2) Enter on the selected palette row (footer `↵ run`) did
  not run the row in a live session; the status bar reported `no active menu
  session for id 9223372036854775809 (client 2)`, i.e. the activation intent
  carried a session id the server no longer knew. Priority: medium (the palette is
  a primary command surface; the row's chord is the workaround). Source: plan 136
  task 12 outcome and `test-plan/artifacts/136-capability-grants/live-example-config/README.md`.

## Follow-Up Disposition (2026-09-23)

Where each `## Further Actions` item went after this plan closed:

| Item | Destination |
| --- | --- |
| `PackageLoadEntryAllowlist::revoke_package` has no production caller (high, security) | `plans/154-Package-Authority-Follow-Ups-Module-Withdrawal-Mode-Activation-and-Grant-Sealing.md` — task "Withdraw a revoked or disabled package's recorded module entries" |
| Planned inventory rows escape the Rust-path existence guard (low) | Closed inside this plan: task 8 prefixed `application.quit`'s `deno_op_path` with `planned:` (`docs/reference/clay-js-api/api-inventory.toml:737`) and the guard now checks `deno_op_path` (`tests/clay_js_api_inventory.rs:563`) — no further task |
| Build the in-app capability-grant surface (GUI) | `plans/155-In-App-Capability-Grant-Surface-Package-Panel-and-Presets.md` — prototype, approval, and implementation tasks |
| Seal capability grants after the first package activation (low) | `plans/154-…` — task "Decide and implement the capability-grant seal for mid-generation changes" |
| Revisit the LANES_PER_DOMAIN/latency-heap numbers once real sessions exist (low) | `plans/156-Runtime-and-Client-Follow-Ups-Lane-Budget-Op-Level-Option-Keys-and-Palette-Input-Findings.md` — task "Revisit the lane budget and latency heap with a real-session mix" |
| Consider capability presets for common packages (medium) | `plans/155-…` — task "Add capability presets over the grant surface" |
| Activate package-owned modes for open documents (medium) | `plans/154-…` — tasks "Activate package-owned modes for open documents" and "Deliver the package mode's editor rules and completion triggers to the client" |
| Widen the option-surface guard to op-level JSON keys (low) | `plans/156-…` — task "Reject unknown option keys at the registration op boundary" |
| Two live input findings from the launch test (medium) | `plans/156-…` — tasks "Consume a completed two-stroke chord…" and "Fix palette row activation so Enter runs the selected row" |
