# 154 — Package Authority Follow-Ups: Module Withdrawal, Package Mode Activation, and Grant Sealing

Source: `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md`
→ `## Further Actions` (items recorded 2026-09-23). Plan 136 shipped the grant
surface (op, CLI verb, config call, durable provenance-bound grants) but three
package-authority gaps stayed open, all reproduced on the finished tree:

1. `PackageLoadEntryAllowlist::revoke_package` (`src/server/ops/packages.rs:110`)
   still has no production caller — it keeps a narrow
   `allow(dead_code, reason = "withdrawal capability for the disable/revoke path;
   no production caller yet (plan 131 task 4 finding)")`. A revoked or disabled
   package's recorded `clay://packages/...` module entries stay resolvable for
   the current runtime generation.
2. Package-owned modes register but never activate: `modes.serverRegisterModePattern`
   is grant-gated and works, yet nothing activates a registered package mode for
   an open document, so the package's parse handler never dispatches and the
   client never receives its `editorRules` or completion trigger characters.
   Third-party manifest contributions stay trusted-only
   (`src/server/ops/packages.rs:736-738` — `apply_package_record_contributions`
   runs only for `RuntimeDomain::Trusted`), and the only caller of
   `PackageModeRegistry::activate_major_mode` (`src/packages/modes.rs:677`) is
   `ClayOpState::activate_major_mode` (`src/server/ops/mod.rs:1104`) driven from
   the JS op; `classify`/`activate_major_mode` are otherwise called only from
   tests, so there is no document-open hook and no client/protocol activation
   message.
3. The grant-seal policy for a mid-generation grant is undecided: today only
   trusted user configuration/CLI can call `authorize`, the op refuses inside a
   package activation scope (`ensure_grant_authority_open`,
   `src/server/ops/packages.rs:437`), and `ensure_capability_grants`
   (`src/packages/service.rs`) re-checks at every enable — so no seal is needed
   yet, and the language-server precedent (`ensure_authorization_open`,
   `src/server/ops/language_server.rs:122`) shows the shape if one ever is.

Evidence for (2) and (3): `test-plan/artifacts/136-capability-grants/README.md`
(ceiling rows) and `code-reviews/2026-09-20-plan136-baseline/primitive-review.md`
§6. This plan does not touch app UI, so no HTML prototype gate applies; if the
activation work needs a new visible surface (e.g. a mode badge), the plan stops
and enters the prototype/approval loop instead of choosing visuals ad hoc.

## Objectives

- M1: a revoked or disabled package's recorded package module entries are
  withdrawn in the same operation, so `clay://packages/...` no longer resolves
  for a package that lost its authority, and the `dead_code` allow disappears
  because the method has a real caller.
- M2: a granted package mode activates for an open document on the host side —
  classification and activation happen on document open (and on package
  enable/disable and reload), the package's parse handler dispatches, and the
  client receives the activation's `EditorBehaviorRules`, command/keymap
  declarations, and completion trigger characters.
- M3: the mid-generation grant question has an explicit, recorded answer: either
  a seal is implemented (grants freeze after the first activation, as
  `ensure_authorization_open` does for language servers) or a decision log
  records that no seal is required, with the invariant that keeps it unnecessary.

## Expected Outcome

- `clay package revoke @fixture/lane` (or `clay package disable`) on a loaded
  package leaves no resolvable `clay://packages/...` module for it: a later
  import of that specifier fails closed instead of executing the package's code
  from the recorded path, proven by a test that revokes after a successful load
  and asserts the module no longer resolves.
- With `@fixture/lane` granted and loaded, opening a `*.lane` document activates
  the package mode: the parse handler runs (visible in
  `js_runtime.lane.third_party.general.dispatched`), the client shows the
  package's editor rules, and the fixture's completion trigger characters reach
  the client — the live leg that plan 136 task 6 had to record as a ceiling.
- Enabling a package whose mode matches an already-open document activates it
  without a reload, and disabling/revoking it returns the document to the
  built-in fallback mode with no stale package rules left in the client.
- A decision log or an implemented seal (M3) exists, and the invariant it relies
  on is pinned by a test so the "no seal needed" answer cannot silently expire.
- `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets
  -- -D warnings`, `cargo test --all-targets`, `cargo test -p clay-desktop
  --all-targets`, `npm run typecheck`, `npm run lint`, `npm test`, and
  `scripts/check.sh full` are green.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: the tree state (HEAD, branch, dirty-file count) and a full
      `scripts/check.sh full` run are recorded before any edit; the three gaps
      are each reproduced with a concrete observation — (a) `revoke_package` has
      no production caller (grep shows only the `#[cfg(test)]` caller
      `src/server/js_runtime/tests/load_package_and_module_loader.rs:386`), (b) a
      granted fixture package's mode never activates for an open document (the
      live plan-136 granted run's lane counters show only load-entry
      registration, no parse dispatch), (c) `authorize` is refused inside a
      package activation scope while no seal exists for the wider case.
    - Performance: the baseline records the `server.edit_ack` p50/p95 from a
      `run-live.sh markdown` or granted-lane live run (plan 136 baseline: p50
      231.8 µs / p95 282.1 µs) as the envelope this plan must not regress, plus
      the note that document-open classification must stay off the typing path.
    - Code Quality: the baseline records the exact code paths for each gap with
      file:line (`src/server/ops/packages.rs:56-130`, `:437`, `:736-738`;
      `src/packages/modes.rs:677`; `src/server/ops/mod.rs:1104-1212`), and
      confirms that `PackageModeRegistry` classification/activation already
      exists as a reusable primitive, so no mode-specific Rust branch is needed.
    - Security: the baseline restates the trust-domain invariants this plan must
      preserve: trusted classification only from the compiled bundled inventory,
      third-party contributions never applied to the trusted domain, grants
      provenance-bound, and no new authority for package code.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`,
        `docs/wiki/modules/primitive-architecture.md`,
        `docs/wiki/modules/package-loading.md`,
        `docs/wiki/modules/third-party-runtime-authority.md`: primitive inventory
        and the authority model before proposing any new Rust surface.
      - `.agents/skills/clay-execution/references/packages.md`: package authority
        boundaries and the two-runtime trust domains.
      - `docs/reference/packages/creating-packages.md`,
        `docs/reference/clay-js-api/packages/authorize.md`,
        `docs/reference/clay-js-api/modes/server-activate-major-mode.md`: the
        declared surfaces this plan must keep honest.
      - `test-plan/artifacts/136-capability-grants/README.md` and
        `code-reviews/2026-09-20-plan136-baseline/primitive-review.md`: the
        recorded ceilings and rejected options from plan 136.
    - Options Considered:
      - Revalidate package enablement at module resolution instead of withdrawing
        entries: keeps one choke point, but the allowlist lookup is on the module
        loader path and would need the package service lock there — rejected in
        favour of withdrawing entries at revoke/disable, which is where the
        authority actually changes.
      - Activate package modes from configuration JS (a document-open hook in the
        config domain): flexible, but config code would have to be present and
        correct for a package's own declared mode to work — rejected as the
        primary path; the hook stays optional and additive.
      - Activate host-side on document open from the enabled record (chosen):
        matches the trusted path (`activate_major_mode` + `record_behavior_for_activation`
        already build the activation payload from a record) and keeps packages
        declarative.
    - Chosen Approach: reproduce and inventory first, then withdraw module
      entries on authority loss, then wire host-side document-open activation
      through the existing `PackageModeRegistry` primitives, then settle the seal
      question with a decision log or an implementation.
    - API Notes and Examples:
      ```bash
      scripts/check.sh full                       # baseline + final gate
      cargo test --lib package_load_entry         # allowlist record/withdraw tests
      cargo test --test security package_loading::  # enable/disable/revoke paths
      cargo test --lib lanes_and_queues -- --nocapture   # lane counters after activation
      ```
      ```rust
      // the withdrawal the disable/revoke path should call
      let withdrawn = clay_state
          .package_load_entry_allowlist()
          .revoke_package(&package_name);
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/154-package-authority/baseline.md`: reproduction,
        code paths, gate log, perf envelope.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md`
        → `## Further Actions` items 1-3.
      - `src/server/ops/packages.rs:56-130`, `:437`, `:736-738`.
      - `src/packages/service.rs` (`disable`, `revoke_package_approval`,
        `capability_granted`), `src/launch.rs` (`clay package revoke|disable`).
  - Test Cases to Write:
    - Baseline reproduction: each gap observed once with its code path cited, so
      each later task can be shown to change exactly that behavior.

- [ ] Review package-authority, module-allowlist, and mode-activation primitives before implementation
  - Acceptance Criteria:
    - Functional: the review inventories the existing primitives this plan will
      reuse — `PackageLoadEntryAllowlist` (record/withdraw/lookup),
      `PackageService` authority transitions (enable/disable/revoke/rollback),
      `PackageModeRegistry` (classify, activate_major_mode,
      activate_builtin_major_mode, editor-rule/command/keymap declarations),
      `ClayOpState::activate_major_mode` + `record_behavior_for_activation`, the
      behavior manifest path to the client, and the parse coordinator — and
      states for each objective which primitive already covers it.
    - Performance: the review names where the new work runs relative to the hot
      paths (document open, package enable/disable/reload, module resolution) and
      confirms nothing is added to keypress/paint/edit-ack paths.
    - Code Quality: any genuinely new Rust surface is generic and reusable across
      future modes and packages (no mode- or package-specific branch, no
      hard-coded language name); the review lists the reference-doc, wiki, and
      test files each new/changed primitive must land in.
    - Security: the review records the trust-domain position of every touched
      primitive (which domain may call it, what authority it can confer, why
      third-party code cannot reach it) before implementation starts.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/registry.md`, `docs/reference/primitives/package-security.md`,
        `docs/reference/primitives/package-loading.md`.
      - `docs/wiki/modules/primitive-architecture.md`,
        `docs/wiki/modules/third-party-runtime-authority.md`,
        `docs/wiki/modules/parse-coordinator.md`.
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
        (primitive-first planning), `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`.
    - Options Considered:
      - Plan the activation layer as a package-specific Rust branch: rejected —
        the registry and activation payload builder are already mode-agnostic.
      - Treat the allowlist withdrawal as a security-only fix with no primitive
        review: rejected — module resolution is a documented primitive surface
        and the review is cheap.
    - Chosen Approach: inventory first, then plan only the generic gaps the
      inventory shows (expected: a document-open activation entry point and an
      optional document-open JS hook).
    - API Notes and Examples:
      ```bash
      grep -rn "activate_major_mode\|record_behavior_for_activation" src/ | grep -v "^src/server/ops/mod.rs:1[01]"
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/154-package-authority/primitive-review.md`: inventory,
        gaps, and the primitive-documentation obligations.
    - References:
      - `docs/reference/primitives/index.md`; `src/packages/modes.rs`;
        `src/server/ops/mod.rs:1104-1212`.
  - Test Cases to Write:
    - None (review task); the follow-up tasks carry the tests it prescribes.

- [ ] Withdraw a revoked or disabled package's recorded module entries
  - Acceptance Criteria:
      - Functional: `clay package revoke`, `clay package disable`, and the
        replacement/rollback paths that disable a package withdraw that package's
        recorded `clay://packages/...` entries through
        `PackageLoadEntryAllowlist::revoke_package`, so an import of the specifier
        afterwards fails closed; the withdrawal count is surfaced in the same
        diagnostics the verb already prints (or the server log for the
        replacement path).
      - Performance: withdrawal runs only on an authority transition (CLI verb,
        enable/disable, rollback), takes the allowlist lock once, and adds no work
        to module resolution or the typing path.
      - Code Quality: the `#[cfg_attr(not(test), allow(dead_code, reason = ...))]`
        on `revoke_package` is deleted with its caller in place; the withdrawal
        lives in one helper the revoke/disable/replacement paths share instead of
        three call sites with their own copies.
      - Security: a package that lost its authority cannot have its code executed
        through a stale recorded module path; the test proves the negative
        (post-revoke import fails closed) and that a *different*, still-enabled
        package's entries are untouched.
    - Approach:
      - Documentation Reviewed:
        - `docs/reference/primitives/package-loading.md`,
          `docs/wiki/modules/package-loading.md`: the allowlist gate and module
          loader contract.
        - `docs/wiki/modules/third-party-runtime-authority.md`: the
          revoke/disable/replacement lifecycle this change hooks into.
        - `src/server/js_runtime/mod.rs` (`resolve`/`load`), `src/server/ops/packages.rs:56-130`.
      - Options Considered:
        - Revalidate enablement on every resolution (see baseline options):
          rejected — puts the package service lock on the loader path.
        - Withdraw on authority loss (chosen): one lock acquisition where the
          authority actually changes.
      - Chosen Approach: one shared withdrawal helper called from the revoke,
        disable, and replacement-disable paths, plus the dead-code allow removal.
      - API Notes and Examples:
        ```rust
        // src/server/ops/packages.rs — the helper both paths call
        pub(crate) fn withdraw_package_modules(allowlist: &PackageLoadEntryAllowlist, package_name: &str) -> usize {
            allowlist.revoke_package(package_name)
        }
        ```
      - Files to Create/Edit:
        - `src/server/ops/packages.rs`: withdrawal helper, allow removal.
        - `src/packages/service.rs` and/or `src/launch.rs`: call the helper from
          the revoke/disable paths (exact seam decided from the baseline).
        - `src/server/js_runtime/mod.rs`: only if the replacement path needs the
          same call.
        - `src/server/js_runtime/tests/load_package_and_module_loader.rs`,
          `tests/package_loading.rs`: withdrawal tests.
      - References:
        - `plans/131-Dead-Code-and-Stale-Allow-Sweep.md` (the finding),
          `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` item 1.
    - Test Cases to Write:
      - `revoked_package_modules_are_withdrawn`: load a fixture package, revoke,
        then assert the recorded specifier no longer resolves and no package code
        runs; assert another package's entries survive.
      - `disabled_package_modules_are_withdrawn`: same for `disable`, including
        the replacement-disable path.

- [ ] Activate package-owned modes for open documents
  - Acceptance Criteria:
    - Functional: opening a document whose classification matches an enabled,
      granted package mode activates that mode host-side and dispatches its parse
      handler; the activation happens on document open, on package enable (for
      documents already open), on disable/revoke (falling back to the built-in
      mode), and after a runtime reload; a package mode that lost its grant does
      not activate and produces the same sanitized diagnostics as today.
    - Performance: classification runs on document open (and on the transitions
      above), never per keystroke or per edit-ack; the parse handler stays on its
      lane and the parse coordinator still bounds one pending job per document;
      the baseline `server.edit_ack` envelope is unchanged (re-measured live).
    - Code Quality: the activation layer is built from the enabled record through
      the existing registry/activation-payload path (no package-specific Rust
      branch, no hard-coded mode or language name); one entry point serves
      document open, enable, disable, and reload, and the optional JS
      document-open hook is additive (config absence changes nothing).
    - Security: only enabled packages with a `mode-activation` grant activate;
      third-party contributions still never enter the trusted domain; the hook
      confers no new authority and cannot activate a mode the package does not
      declare.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-loading.md`, `docs/reference/primitives/package-security.md`,
        `docs/wiki/modules/third-party-runtime-authority.md`,
        `docs/wiki/modules/parse-coordinator.md`,
        `docs/wiki/modules/behavior-manifests.md`.
      - `docs/reference/clay-js-api/modes/server-activate-major-mode.md`,
        `docs/reference/clay-js-api/modes/server-register-mode-pattern.md`.
      - `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`,
        `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`
        (package contributions stay declarative).
    - Options Considered:
      - Configuration-driven activation only (JS document-open hook): rejected as
        the primary path (a package's own mode would depend on user config being
        present and correct).
      - Host-side activation from the enabled record (chosen): the trusted path
        already does exactly this through `activate_major_mode` +
        `record_behavior_for_activation`; the mode registry is mode-agnostic.
      - Client-side activation (client asks the server to activate on open):
        rejected — the server owns classification and the mode registry; the
        client keeps consuming the activation result it already knows how to
        apply.
    - Chosen Approach: add the document-open activation entry point on the server
      (classification input assembled from the open document, activation built
      from the enabled record), reuse `record_behavior_for_activation` for the
      payload, and add the optional JS document-open hook only if the review
      shows configuration code needs it.
    - API Notes and Examples:
      ```rust
      // the existing pieces the new entry point composes (src/server/ops/mod.rs)
      let activation = clay_state.activate_major_mode(&classification)?;   // :1104
      let payload = clay_state.record_behavior_for_activation(&activation); // :1185
      ```
      ```bash
      # live leg: granted fixture opens a *.lane document
      test-plan/artifacts/127-lane-scheduling/run-live.sh start granted-lane
      ```
    - Files to Create/Edit:
      - `src/server/ops/mod.rs` and/or the document-open path in
        `src/server/` (exact seam from the baseline): host-side activation entry
        point.
      - `src/packages/modes.rs`: only if classification input assembly needs a
        generic helper.
      - `src/server/js_runtime/tests/` and `tests/package_loading.rs`: activation
        and fallback tests.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` item 7; plan 136 task 6 outcome.
      - `src/server/ops/packages.rs:736-738` (trusted-only contributions),
        `src/packages/modes.rs:677`.
  - Test Cases to Write:
    - `granted_package_mode_activates_for_an_open_document`: classification
      matching an enabled granted package mode produces an activation whose owner
      is the package, and the parse handler dispatches.
    - `package_mode_falls_back_on_disable_and_revoke`: disable/revoke returns the
      document to the built-in mode and no package rules remain.
    - `ungranted_package_mode_does_not_activate`: a package without the
      `mode-activation` grant never activates (fail closed, sanitized
      diagnostic).

- [ ] Deliver the package mode's editor rules and completion triggers to the client
  - Acceptance Criteria:
    - Functional: the activation's `EditorBehaviorRules`, command/keymap
      declarations, and completion trigger characters reach the client for a
      package-owned mode, so the client applies the package's editor rules and
      offers completion on its declared triggers; a mode change replaces the
      previous package's rules instead of merging them.
    - Performance: rules and triggers are transported on the activation event
      only (document open / mode change / reload), never per keystroke; the client
      installs them through the existing behavior-manifest/compartment path with
      no new per-frame work.
    - Code Quality: the client reuses the existing behavior manifest and
      editor-extension plumbing (no package-specific client branch, no second
      rules parser); the wire shape stays the documented behavior manifest rather
      than a new ad-hoc message.
    - Security: the client treats the payload as inert data (validated
      server-side), adds no new IPC surface, and cannot be induced by a package to
      bind a trusted-only command.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/behavior/` pages and
        `docs/wiki/modules/behavior-manifests.md`,
        `docs/wiki/modules/behavior-runtime-registration.md`.
      - `frontend/src/editor/extensions/behavior.ts` (manifest keymaps and the
        chord plugin), `frontend/src/editor/extensions/controller.ts`.
      - `DESIGN.md` and `.agents/skills/clay-execution/references/ui.md`: editor
        chrome ownership, accent-for-state, reduced-motion expectations.
    - Options Considered:
      - A new package-mode wire message: rejected — the behavior manifest already
        carries rules, commands, and keymaps for an activation.
      - Reuse the behavior manifest path (chosen): one shape for trusted and
        package modes, already tested.
    - Chosen Approach: project the activation into the existing behavior manifest
      for the active document, and verify the completion trigger characters reach
      the client's completion configuration.
    - API Notes and Examples:
      ```ts
      // existing client entry point the activation result feeds
      behaviorCompartment.dispatch({ effects: setBehaviorManifest(manifest) });
      ```
    - Files to Create/Edit:
      - `src/server/` activation-to-manifest projection (exact file from the
        baseline), `frontend/src/editor/extensions/behavior.ts` only if a
        trigger-character field is missing, plus tests.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` item 7 (client never receives
        editor rules or trigger characters).
      - `test-plan/artifacts/127-lane-scheduling/fixture-package/load.js`
        (`editorRules.autocompleteTriggers`, `completionProviders[].triggerCharacters`).
  - Test Cases to Write:
    - `package_mode_activation_carries_editor_rules_and_triggers`: the projected
      manifest for a package mode contains the package's rules and trigger
      characters.
    - Client-side test: applying a package manifest installs its rules and
      replaces the previous package's set on mode change.

- [ ] Decide and implement the capability-grant seal for mid-generation changes
  - Acceptance Criteria:
    - Functional: the question is answered explicitly — either `authorize`
      refuses once the package has been activated in the current runtime
      generation (the `ensure_authorization_open` shape), or the plan records in a
      decision log that no seal is required and pins the invariant that makes it
      unnecessary with a test (a grant can only widen authority for a package that
      is not currently executing, and `ensure_capability_grants` re-checks at
      every enable).
    - Performance: the chosen check (if implemented) is a generation/activation
      lookup on the grant path only — never on dispatch, resolution, or the typing
      path.
    - Code Quality: the outcome is recorded in a decision log with the rejected
      alternative and its rationale; any implemented seal reuses the existing
      activation-scope/authorization-open pattern instead of a new flag.
    - Security: the answer covers the concrete risk (a mid-generation grant
      widening authority for code that is already running), states what the seal
      does and does not protect against, and is proven by a test that fails if the
      guard is removed (mutation check) when a seal is implemented.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/third-party-runtime-authority.md`,
        `docs/reference/primitives/package-security.md`.
      - `src/server/ops/language_server.rs:122` (`ensure_authorization_open`) as
        the precedent, `src/server/ops/packages.rs:437`
        (`ensure_grant_authority_open`).
      - `code-reviews/2026-09-20-plan136-baseline/primitive-review.md` §6;
        `.agents/skills/create-decision-log/SKILL.md`.
    - Options Considered:
      - Implement the seal now: cheapest while the invariant is fresh, but adds a
        guard against a path that is currently unreachable from third-party code.
      - Record the decision with a pinning test (chosen if the invariant holds):
        honest and reversible, with the test failing if the invariant is broken.
      - Leave it undecided: rejected — this is exactly the kind of "later means
        never" item the plan exists to close.
    - Chosen Approach: verify the invariant first (only trusted config/CLI can
      call `authorize`; the op refuses inside a package activation scope; enable
      re-checks the grant), then either implement the seal or write the decision
      log plus the pinning test.
    - API Notes and Examples:
      ```rust
      // the precedent shape, if a seal is implemented
      ensure_grant_authority_open(&clay_state)?;   // src/server/ops/packages.rs:437
      ```
    - Files to Create/Edit:
      - `decision-logs/<date>-capability-grant-seal.md` (or the seal
        implementation in `src/server/ops/packages.rs` / `src/packages/service.rs`),
        plus `src/server/js_runtime/tests/package_adoption.rs` for the pinning
        test.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` item 4;
        `decision-logs/2026-07-14-2023-language-server-package-authority.md`.
  - Test Cases to Write:
    - `capability_grant_after_activation_is_refused_or_pinned`: with a seal, the
      post-activation grant is refused (`packages.grant_after_activation`); with
      the decision-log answer, the test asserts the invariant that keeps grants
      inert for running code.

- [ ] Verify the package default init.js loading experience
  - Acceptance Criteria:
    - Functional: after this plan, one explicit line in `~/.clay/init.js`
      (`await loadPackage("@fixture/lane")`, or the real package a user adopts)
      still yields the package's default behavior — its mode activating on the
      matching document, its parse handler dispatching, its completion provider
      available — with no extra manifest plumbing, mode registration, or
      activation call in user configuration.
    - Performance: the one-line path adds no configuration-time work beyond what
      the grant/enable path already does.
    - Code Quality: if any package now needs a longer setup, the plan identifies
      the generic Clay primitive/API gap and documents the fallback as a
      limitation rather than a convention.
    - Security: the one-line load still requires an explicit grant before
      execution, and the verification records that loading alone confers no
      capability.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-09-0219-explicit-init-js-package-loading-with-one-line-defaults.md`.
      - `docs/reference/clay-js-api/packages/load-package.md`,
        `docs/reference/packages/creating-packages.md`.
      - `examples/config/packages/third-party.js` (the adopt → authorize →
        loadPackage order).
    - Options Considered:
      - Fold this into the launch-test task: rejected — the one-line experience is
        a documented contract with its own tests and docs.
    - Chosen Approach: verify the one-line load on the granted fixture after the
      activation work, and record the exact setup steps the fixture needs.
    - API Notes and Examples:
      ```js
      import { loadPackage } from "clay:packages";
      await loadPackage("@fixture/lane");
      ```
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`: only if the setup changed.
    - References:
      - `test-plan/artifacts/127-lane-scheduling/fixture-package/`,
        `test-plan/artifacts/136-capability-grants/init-granted-lane.js`.
  - Test Cases to Write:
    - `one_line_load_activates_the_package_mode`: the fixture loaded by a single
      `loadPackage` line activates its mode for a matching document.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: every Rust public function or behavior introduced or changed by
      this plan is either exposed through an explicit op + stable Clay JS/TS
      facade, or made private/`pub(crate)`; the JS document-open hook (if added)
      is documented with its stable ID, options, and authority boundary.
    - Performance: no facade adds work to a hot path; the activation entry point
      stays host-side.
    - Code Quality: Markdown docs carry stable ID, user-facing name, key
      bindings, custom properties, usage, options, return/async behavior, errors,
      permissions, backing Rust path, op wrapper, facade path, and lookup tags;
      `docs/index.md` links them; the generated registry is regenerated.
    - Security: any new facade documents that it cannot widen package authority,
      names its trusted-only or package-facing position, and keeps the
      denied-authority list accurate.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`,
        `.agents/skills/clay-execution/references/docs-as-code.md`.
      - `docs/reference/clay-js-api/modes/server-activate-major-mode.md`,
        `docs/reference/clay-js-api/packages/authorize.md` (page template).
    - Options Considered:
      - Expose the activation entry point to packages: rejected — activation is
        host-owned; packages declare modes.
    - Chosen Approach: verify the existing mode/packages facades stay accurate and
      add only the document-open hook if the review showed it is needed.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test --test protocol clay_js_api_inventory::
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`, `docs/reference/clay-js-api/api-inventory.toml`,
        `docs/index.md`, `docs/generated/clay-js-api-registry.json`,
        `docs/development/tauri-react-parity-ledger.json` (as required).
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
  - Test Cases to Write:
    - Existing inventory/registry tests stay green; a new test only if a facade
      is added.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: any configuration surface this plan changes (e.g. an optional
      document-open hook) is a documented Clay JS API with its options, defaults,
      and allowed values; unchanged surfaces are verified rather than restated.
    - Performance: configuration work happens at configuration/activation time,
      not per keystroke.
    - Code Quality: `~/.clay/init.js` remains the entry point; docs and the
      generated registry match the implementation.
    - Security: configuration never implicitly grants filesystem, network, shell,
      extension loading, or package authority, and the plan states that granting
      still requires the explicit `authorize` call.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`;
        `docs/reference/clay-js-api/configuration.md`.
    - Options Considered:
      - Add configuration keys for mode activation: rejected — activation is
        host-side; a hook (if any) is additive and cannot be required.
    - Chosen Approach: verify the configuration surface, documenting the hook only
      if it ships.
    - API Notes and Examples:
      ```js
      // only if the optional hook ships
      import { onDocumentOpen } from "clay:documents";
      onDocumentOpen(({ documentId, path }) => { /* ... */ });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md` and the relevant page(s).
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → task 10 outcome (configuration review precedent).
  - Test Cases to Write:
    - `plan154_configuration_documents_the_activation_surface` (marker test), or
      an explicit no-change verification recorded in the task evidence.

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: the example keeps every supported configuration surface exactly
      once in its section, with all documented options annotated; if the plan adds
      a configuration surface, it appears there; if nothing changes, the task
      records the verification with the exact command output.
    - Performance: `node --check examples/config/init.js` passes and the active
      (uncommented) part stays copy-safe.
    - Code Quality: documented ordering constraints (adopt → authorize →
      `loadPackage`) are preserved; option names/enums/defaults match the
      validated server-side parsers.
    - Security: the example never ships an active grant for a powerful capability
      or a credential.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/clay-js-api/configuration.md`;
        `examples/config/README.md`; `examples/config/packages/third-party.js`.
      - `tests/clay_js_doc_registry.rs::canonical_example_active_configuration_is_copy_safe`.
    - Options Considered:
      - Skip the task when nothing changed: rejected — the duty is to verify, and
        the verification is one command plus a diff check.
    - Chosen Approach: update only if a surface changed; otherwise record the
      verification.
    - API Notes and Examples:
      ```bash
      node --check examples/config/init.js && node --check examples/config/packages/third-party.js
      ```
    - Files to Create/Edit:
      - `examples/config/init.js`, `examples/config/packages/third-party.js`
        (only if the surface changed).
    - References:
      - user instruction 2026-08-03 (canonical example config maintenance duty).
  - Test Cases to Write:
    - `canonical_example_active_configuration_is_copy_safe` and the plan-136
      configuration marker test stay green.

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: a real Linux build launches against a copy of
      `examples/config/` in an isolated scratch root (never the developer
      profile), reaches Connected, commits a configuration generation with no
      `configuration failed` diagnostics, and shows the package path working:
      the granted fixture's mode activating on a matching document with its
      editor rules visible.
    - Performance: startup timings are recorded (socket ready, first config
      effect, editor visible) and compared against the plan-136 example-config
      baseline (`socket_ready_ms=425`, `config.first_effect_ms=493`,
      `editor_visible_ms=1388`); no regression beyond noise.
    - Code Quality: the launch command, scratch config path, and observed results
      are recorded as task evidence; a degraded app under the example config is
      treated as a product defect, not a docs fix.
    - Security: the scratch root is mode-700 with its own HOME/XDG/TMPDIR and
      socket; teardown uses PIDs (`run-live.sh stop`), never pattern kills.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/artifacts/136-capability-grants/live-example-config/README.md`
        (the plan-136 launch-test recipe and its two deferred input findings).
      - `test-plan/artifacts/127-lane-scheduling/run-live.sh` (`example-config`
        mode), `test-plan/artifacts/127-lane-scheduling/probe.py`.
    - Options Considered:
      - Reuse the plan-136 `example-config` mode as-is: chosen — the harness,
        isolation, and timings are already comparable.
    - Chosen Approach: launch with the copied config, confirm the package mode
      activates for a matching document, record timings and screenshots.
    - API Notes and Examples:
      ```bash
      CLAY_LIVE_ROOT=/tmp/clay-plan154-example test-plan/artifacts/127-lane-scheduling/run-live.sh start example-config
      CLAY_LIVE_ROOT=/tmp/clay-plan154-example test-plan/artifacts/127-lane-scheduling/run-live.sh stop
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/154-package-authority/live-example-config/`:
        screenshots, AT-SPI trees, timings, README.
    - References:
      - user instruction 2026-09-07 (launch-test duty);
        `docs/wiki/modules/configuration-runtime.md`.
  - Test Cases to Write:
    - Manual launch leg (recorded): healthy startup, no configuration
      diagnostics, package mode active on the fixture document.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: module 09 steps gain the activation and withdrawal legs — the
      granted fixture's mode activating for an open document (positive), the
      revoked/disabled package losing its module entries and falling back to the
      built-in mode (negative), and the seal decision recorded against the
      authorize step — executed on a real Linux build with pass/fail per step.
    - Performance: the affected steps record the `server.edit_ack` p50/p95
      envelope and the lane counters (`js_runtime.lane.third_party.general.dispatched`)
      from the live run, with the plan-136 baseline values for comparison.
    - Code Quality: `test-plan/index.md` (module map, coverage matrix, execution
      record) and the parity ledger (`packages.modes.settings.themes`) are updated
      in the same task; no existing step is weakened or deleted.
    - Security: the negative legs prove fail-closed behavior — no grant means no
      activation, revoked means no resolvable module and no package mode, and the
      grant surface still cannot be reached from package code.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/09-packages-and-modes.md`, `test-plan/11-performance.md`,
        `test-plan/index.md`, `tests/documentation_coverage.rs` (step-ID ledger rule).
      - `test-plan/artifacts/136-capability-grants/manual-plan/README.md` (the
        harness and ceiling precedent).
    - Options Considered:
      - Add a new module for package activation: rejected — module 09 owns
        packages and modes; the steps belong there.
    - Chosen Approach: extend module 09's plan-136 section with the new legs and
      record the execution.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol documentation_coverage::parity_ledger
      ```
    - Files to Create/Edit:
      - `test-plan/09-packages-and-modes.md`, `test-plan/index.md`,
        `docs/development/tauri-react-parity-ledger.json`.
    - References:
      - plan 136 task 13 outcome (the manual-plan execution pattern).
  - Test Cases to Write:
    - Manual steps with expected results, negatives, and ceilings; ledger and
      documentation-coverage tests stay green.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/third-party-runtime-authority.md` documents
      the module-withdrawal invariant and the grant-seal decision,
      `docs/wiki/modules/package-loading.md` documents when module entries are
      withdrawn and what resolution does afterwards, and the mode-activation
      flow (document open → classification → activation → behavior manifest) is
      documented on the page that owns modes (`docs/wiki/modules/behavior-manifests.md`
      and/or a mode-activation page), including how to debug it.
    - Performance: wiki updates add no runtime work; the activation timing
      position (document open, not keystroke) is stated.
    - Code Quality: pages follow the wiki template (source, overview, how it
      works, invariants, tests, related), link the authoritative reference docs
      instead of duplicating them, and are linked from `docs/wiki/index.md`.
    - Security: the pages state the trust boundary — third-party contributions
      never enter the trusted domain, activation requires an enabled package with
      a mode-activation grant, and withdrawal removes resolvable module paths.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md` (wiki workflow,
        page template, quality bar).
      - `tests/documentation_coverage.rs` (wiki contract tests).
    - Options Considered:
      - Update per task: rejected — noisy; update once after tests pass (plan-136
        precedent).
    - Chosen Approach: update after all tasks pass, with a contract test pinning
      the new markers, plus the index descriptions.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol documentation_coverage::
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/third-party-runtime-authority.md`,
        `docs/wiki/modules/package-loading.md`,
        `docs/wiki/modules/behavior-manifests.md`, `docs/wiki/index.md`,
        `tests/documentation_coverage.rs` (contract test).
    - References:
      - plan 136 task 14 outcome (wiki contract-test pattern).
  - Test Cases to Write:
    - `plan154_wiki_pages_describe_module_withdrawal_and_mode_activation`: marker
      test plus a stale-claim scan for the pre-plan-154 claims ("nothing activates
      a registered package mode", "module entries are never withdrawn").

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
