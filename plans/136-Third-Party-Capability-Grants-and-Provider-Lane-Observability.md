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

- [ ] Baseline gates on the unmodified tree
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

- [ ] Review package capability-grant and lane-observability primitives before implementation
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

- [ ] Implement the `packages.authorize` op, facade, and typed grant path
  - Acceptance Criteria:
    - Functional: `authorize({ package, capabilities, runtimeProfile, source?,
      approvedBy? })` records a grant through
      `PackageService::authorize_package` and returns a summary
      (`packageName`, `capabilities`, `runtimeProfile`, `approvedBy`,
      `granted: true`); re-authorizing the same package with the same
      capabilities is idempotent; `source` mismatching the installed provenance
      fails; an unknown package fails with `packages.not_installed`.
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
      `TypeError: undefined is not a function`; grants require an explicit
      `approvedBy` (`user`/`cli`/`config`) and never widen authority beyond the
      separately implemented documented APIs; `ensure_capability_grants` remains
      the single enforcement point and revocation still fails closed.
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
    - `revoked_package_loses_grants`: revoke after grant → enable and dispatch
      fail closed again.

- [ ] Add the host CLI grant surface and grant visibility in `clay package inspect`
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

- [ ] Prove the third-party grant → provider → latency-lane path with a fixture
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
    - `ungrafted_third_party_provider_still_fails_closed`: enable fails with
      `MissingCapabilityGrant`; no provider registered.

- [ ] Measure provider lane occupancy and record the lane/tuning decision
  - Acceptance Criteria:
    - Functional: per-lane command counts are observable
      (`js_runtime.lane.commands` keyed by domain+lane, or equivalent
      counters for general/latency per domain) in the perf summary produced by
      `CLAY_PERF_REPORT_DIR`; a real measurement is taken with the granted
      fixture and a mixed workload (general lane busy with parse work, latency
      lane serving completions) and recorded in `docs/development/performance.md`.
    - Performance: counters are a single relaxed atomic increment per command
      (no locking, no allocation on the dispatch path); the measurement states
      observed latency percentiles for the latency lane under load.
    - Code Quality: counter names and the measurement live next to the plan-127
      JS-runtime section in `docs/development/performance.md`; the tuning
      decision (keep `JS_RUNTIME_LANES_PER_DOMAIN = 2` and the 32 MiB latency
      ceiling, or raise them) is recorded with the numbers that motivated it.
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
      - Add lane-keyed command counters, take the fixture measurement, and write
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

- [ ] Harden the Clay JS option-surface drift guard
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

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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

- [ ] Create or verify Clay configuration APIs
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

- [ ] Update the canonical example configuration (examples/config/init.js)
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

- [ ] Launch-test the app with the canonical example config
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

- [ ] Execute and update the manual test plan (test-plan/)
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

- [ ] Update or verify the code wiki after implementation
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

## Compromises Made
- To be filled after tasks are completed and tests pass.

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
- **Revisit the LANES_PER_DOMAIN/latency-heap numbers once real sessions
  exist.** The plan records a fixture measurement; a real-session mix (many
  providers, long sessions) is still needed before treating the numbers as
  final. Priority: low unless the fixture measurement already shows latency-lane
  pressure.
- **Consider capability presets for common packages.** `decision-logs/2026-08-18-1758-package-capability-presets.md`
  already approved the preset model; once grants are user-visible, a one-click
  "adopt a completion provider" preset would remove most per-capability
  friction. Priority: medium, after the GUI surface exists.
- **Widen the option-surface guard to op-level JSON keys.** The guard added
  here covers declared `.d.ts` options; a package can still pass an extra JSON
  key the facade forwards and the op ignores (the completion registration
  path does exactly that today for `providerId`/`items`/…). A follow-up can
  reject unknown keys at the op boundary for the registration ops, turning
  silent ignores into errors. Priority: low (the current behavior is
  documented).
