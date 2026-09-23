# 156 — Runtime and Client Follow-Ups: Lane Budget Revisit, Op-Level Option Keys, and Palette Input Findings

Source: `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md`
→ `## Further Actions` items 5, 8, and 9 (recorded 2026-09-23), plus one item
already closed inside plan 136:

- Item 5 — the plan-136 lane measurement is a fixture measurement
  (`PLAN136_GRANTED_LANE busy_ms=500 idle_median_us=1619 busy_completion_us=2937`,
  inside the plan-127 record of 1567/2594 µs), and the tuning decision kept
  `JS_RUNTIME_LANES_PER_DOMAIN = 2` (`src/perf/budgets.rs:384`) and the 32 MiB
  latency ceiling (`JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES`,
  `src/perf/budgets.rs:389`) with explicit revisit triggers. A real-session mix
  (many providers, long sessions) is still missing before the numbers are final.
- Item 8 — the plan-136 option-surface guard
  (`tests/clay_js_api_inventory.rs::declared_option_keys_are_documented_for_every_public_api`)
  covers declared `.d.ts` options only. A package can still pass an extra JSON key
  that the facade forwards and the op silently ignores — the completion
  registration path does exactly that today for `providerId`/`items`/… (the
  completion page documents the ignore as prose).
- Item 9 — two live input findings from the plan-136 example-config launch test:
  (a) with the editor focused, the two-stroke `Ctrl+X Ctrl+O` chord resolves the
  Control Center palette *and* still fires the single-stroke `Ctrl+O` binding
  (`documents.clientOpenFileDialog`), because `manifestKeymaps`
  (`frontend/src/editor/extensions/behavior.ts:193`) registers
  `keymap.of(singles)` *before* the `chordKeymap` ViewPlugin (`:216`), so the
  second stroke matches the single-stroke keymap first; (b) Enter on the selected
  palette row did not run the row — the status bar reported
  `no active menu session for id … (client …)`
  (`src/server/connection/mod.rs:317-323`, `menu.unknown_session`), i.e. the
  activation intent carried a session id the server no longer knew. Evidence:
  `test-plan/artifacts/136-capability-grants/live-example-config/README.md`.
- Already closed: "planned inventory rows escape the Rust-path existence guard"
  was resolved inside plan 136 task 8 — `application.quit`'s `deno_op_path` now
  carries the `planned:` prefix
  (`docs/reference/clay-js-api/api-inventory.toml:737`) and the guard checks
  `deno_op_path` (`tests/clay_js_api_inventory.rs:563`). No task is needed.

**UI prototype gate: does not apply as written.** The two input findings are
behavior fixes on surfaces whose appearance is already specified: the palette is
the shipped Control Center surface (`design-artifacts/approved/agent-lane-palette/`),
and the chord handling is editor keymap plumbing with no new component, token,
typography role, or layout rule. This plan makes the shipped surfaces honor their
existing behavior. If the visual review (task 10) finds an appearance decision the
approved artifact does not settle, the plan stops and enters the prototype/approval
loop instead of choosing values ad hoc.

## Objectives

- L1: the lane-budget question is answered with real-session evidence or an
  explicit, recorded trigger — either a mixed-workload measurement (several
  providers, a long session, a real document mix) that confirms or changes
  `JS_RUNTIME_LANES_PER_DOMAIN` / the latency heap, or a recorded decision that
  the fixture numbers stand until a named trigger fires, with the measurement
  harness and the reading instructions documented.
- L2: the registration ops reject unknown JSON option keys instead of silently
  ignoring them, so `providerId`/`items`/any extra key fails loudly with a
  documented error, and the option-surface guard covers the op boundary rather
  than only the declared `.d.ts` surface.
- L3: a completed two-stroke chord consumes its strokes — the palette chord no
  longer also fires the single-stroke binding, proven by a unit test with a real
  two-stroke sequence.
- L4: activating a palette row with Enter runs the row, or the server's
  `menu.unknown_session` reason is fixed at its cause (a stale session id in the
  activation intent) with a test that fails if it returns.

## Expected Outcome

- A written lane-budget answer exists with numbers (fixture vs real-session mix),
  the harness command to reproduce it, and the revisit trigger; if the numbers
  changed, the constants and `docs/development/performance.md` change together
  and the lane tests still pass.
- `serverRegisterCompletionProvider({ providerId: "x" })` (and the sibling
  registration ops) fail with a documented error naming the unknown key instead
  of registering and ignoring it; the docs and the guard agree.
- `Ctrl+X Ctrl+O` opens the Control Center palette and nothing else: the
  single-stroke `Ctrl+O` dialog binding does not fire, and `Ctrl+B`/other
  single-stroke bindings still work; a vitest case reproduces the two-stroke
  sequence and fails on the old ordering.
- Enter on the selected palette row runs the command (or the row's chord remains
  the documented workaround only if the fix is rejected, with the reason
  recorded); the `menu.unknown_session` diagnostic no longer appears for a
  freshly opened palette.
- `cargo fmt --check`, `cargo check/clippy --all-targets -- -D warnings`,
  `cargo test --all-targets`, `cargo test -p clay-desktop --all-targets`,
  `npm run typecheck`, `npm run lint`, `npm test`, `npm run build`, and
  `scripts/check.sh full` are green.

## Tasks

- [ ] Baseline gates on the unmodified tree
  - Acceptance Criteria:
    - Functional: tree state and a full `scripts/check.sh full` run are recorded;
      each of the three open items is reproduced concretely — (a) the lane
      numbers are re-measured on the current build and shown to come from the
      fixture harness only, (b) an extra JSON key passed to a registration op is
      accepted and ignored (one op-level reproduction with the exact key), (c) the
      chord double-dispatch and the palette Enter failure are reproduced on the
      live app with the plan-136 driver (`test-plan/artifacts/136-capability-grants/`),
      including the `menu.unknown_session` diagnostic.
    - Performance: the baseline records the envelope this plan must not regress —
      the plan-136 `server.edit_ack` p50/p95 (231.8 µs / 282.1 µs) and the lane
      counters from the granted run — plus the frontend suite runtime
      (`npm test`) as the budget for the client fixes.
    - Code Quality: the baseline records the exact code paths for each item with
      file:line (`frontend/src/editor/extensions/behavior.ts:193`, `:216`;
      `frontend/src/shell/use-shell-chords.ts`;
      `src/server/connection/mod.rs:317-323`,
      `src/server/menu_sessions.rs`,
      `src/server/connection/menus.rs:336`; the registration ops in
      `src/server/ops/`), and confirms which fixes are client-only, server-only,
      or both.
    - Security: the baseline records that none of the fixes may add a new op, IPC
      surface, or authority: op-level key rejection is input validation,
      chord/palette fixes are client behavior, and the lane question is a budget
      decision.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md` (plan-136 lane section, metric names,
        reading recipe), `docs/wiki/modules/persistent-runtime-hardening.md`.
      - `.agents/skills/clay-execution/references/ui.md` (client architecture,
        key routing) and `references/protocol-perf.md`.
      - `test-plan/artifacts/136-capability-grants/README.md` and
        `…/live-example-config/README.md`: the recorded findings and their
        reproduction steps.
    - Options Considered:
      - Treat all three as documentation bugs: rejected — each is a shipped
        behavior a user can hit today.
      - Reproduce first, then fix in place (chosen): the palette and chord fixes
        are small and testable; the lane question may legitimately end in
        "decision recorded, no change".
    - Chosen Approach: baseline all three with concrete reproductions, then fix
      the two client findings, then settle the lane question with evidence.
    - API Notes and Examples:
      ```bash
      scripts/check.sh full
      cd frontend && npx vitest run src/shell src/editor --reporter=verbose
      test-plan/artifacts/127-lane-scheduling/run-live.sh start granted-lane
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/156-runtime-client-followups/baseline.md`.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` items 5, 8, 9.
  - Test Cases to Write:
    - Baseline reproduction for each item, with the code path and the observed
      behavior recorded.

- [ ] Revisit the lane budget and latency heap with a real-session mix
  - Acceptance Criteria:
    - Functional: a mixed workload is measured on a real build — more than one
      provider package, a long session, and a document mix that exercises parse,
      analysis, and completion together — and the result either confirms the
      current `JS_RUNTIME_LANES_PER_DOMAIN = 2` and the 32 MiB latency ceiling or
      changes them with the measurement attached; the outcome is recorded with the
      reproduce command and the revisit trigger.
    - Performance: the measurement reports `js_runtime.lane.<domain>.<lane>.{dispatched,pending,peak_pending,superseded,evicted}`
      plus the latency percentiles and heap events; the decision states whether any
      lane showed backlog, supersede/evict churn, a heap-limit event, or a timeout
      under the mixed workload.
    - Code Quality: if a constant changes, `src/perf/budgets.rs`,
      `docs/development/performance.md`, the wiki, and the lane tests change
      together; if nothing changes, the decision is recorded in
      `docs/development/performance.md` (and the wiki page keeps its pointer)
      rather than only in the task evidence.
    - Security: the measurement harness runs in an isolated scratch root with no
      developer profile, and the decision states the resource-bound meaning of any
      change (what it does and does not protect against).
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md` (plan-127 metric table and plan-136
        lane-occupancy section), `docs/wiki/modules/persistent-runtime-hardening.md`
        (lane topology, budgets table, counters).
      - `src/perf/budgets.rs`, `src/server/js_runtime/worker.rs`,
        `src/server/js_runtime/mod.rs` (`RuntimeLane`, mailbox counters).
    - Options Considered:
      - Keep the fixture numbers and record a trigger (chosen if the mixed run
        shows no pressure): honest, cheap, and the trigger is already written.
      - Raise lanes or the heap pre-emptively: rejected — no evidence of pressure,
        and each extra lane costs an isolate.
    - Chosen Approach: build the mixed harness on the existing fixture packages
      (`@fixture/lane`, `@fixture/laneblocked`) plus the bundled providers, measure,
      then decide and record.
    - API Notes and Examples:
      ```bash
      CLAY_PERF_PROFILE=1 CLAY_PERF_REPORT_DIR=/tmp/perf156 \
        test-plan/artifacts/127-lane-scheduling/run-live.sh start granted-lane
      python3 -c 'import json;m=json.load(open("/tmp/perf156/clay-server-perf-summary.json"))["metrics"];print({k:v["total"] for k,v in m.items() if k.startswith("js_runtime.lane.")})'
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/156-runtime-client-followups/lane-mixed/` (summaries +
        README), `docs/development/performance.md` (decision), and
        `src/perf/budgets.rs` only if the numbers change.
    - References:
      - `plans/127-JS-Runtime-Worker-Concurrency-and-Backpressure.md` metrics, `test-plan/artifacts/136-capability-grants/lane-occupancy/`.
  - Test Cases to Write:
    - `lane_counters_track_general_and_latency_separately` and the other lane tests
      stay green after any constant change; the measurement is recorded as manual
      evidence.

- [ ] Reject unknown option keys at the registration op boundary
  - Acceptance Criteria:
    - Functional: the registration ops (`serverRegisterCompletionProvider`,
      `serverRegisterLanguageIntelligenceProvider`, `serverRegisterDocumentAnalyzer`,
      and the other option-taking registration ops the review lists) reject an
      unknown JSON key with a documented error naming the key and the op, instead
      of accepting and ignoring it; every documented option keeps working
      unchanged, and `runtimeBridge`/internal keys stay accepted where the facade
      owns them.
    - Performance: validation is a key-set check on the registration call (a
      configuration/load-time path), never per request or per keystroke; the
      registration payload stays within its existing budget.
    - Code Quality: one shared helper does the key-set validation for all listed
      ops (no per-op copy), the accepted key set is derived from the same source
      the docs and the guard read, and the guard is extended so an undocumented
      *op-level* key fails a test — not only an undeclared `.d.ts` key.
    - Security: the change is input validation only (no new op, no new authority);
      a rejected key never partially registers, so a package cannot smuggle a
      behavior-changing key past the facade.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md` (facade/op boundary),
        `docs/reference/clay-js-api/completion/server-register-completion-provider.md`
        (the current documented ignore), the sibling registration pages.
      - `tests/clay_js_api_inventory.rs` (`declared_option_keys_are_documented_for_every_public_api`,
        the helpers `declared_function_parameters`, `option_object_type_name`,
        `object_type_members`), `runtime/js/completion.js`, `runtime/js/language.js`.
    - Options Considered:
      - Keep ignoring unknown keys and document them: rejected — the ignore list is
        already a documentation wart and a silent-failure trap for authors.
      - Reject unknown keys at the op boundary (chosen): turns a silent ignore into
        an error at the only place that knows the real key set.
      - Reject in the JS facade only: rejected — the facade can be bypassed by a
        package calling the op-shaped facade path, and the op is the trust
        boundary.
    - Chosen Approach: a shared key-set validator used by the listed ops, with the
      accepted set documented per op and pinned by the extended guard.
    - API Notes and Examples:
      ```ts
      // becomes an error naming the key instead of a silent ignore
      serverRegisterCompletionProvider({ module, moduleSpecifier, providerId: "x" });
      // → Error: unknown option `providerId` for completion.serverRegisterCompletionProvider
      ```
    - Files to Create/Edit:
      - `src/server/ops/` (the registration ops + shared helper),
        `runtime/js/*.js` / `*.d.ts` where a key is removed from the declared
        surface, `docs/reference/clay-js-api/**` pages,
        `tests/clay_js_api_inventory.rs` (guard extension), plus tests.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` item 8 and task 8 outcome (the
        declared-surface guard), `docs/reference/clay-js-api/completion/*`.
  - Test Cases to Write:
    - `unknown_option_key_is_rejected_by_registration_ops`: one case per listed op
      asserting the error names the key and that no registration happened.
    - `documented_option_keys_still_register`: the happy path for each op.
    - Guard extension: an undocumented op-level key fails the inventory suite.

- [ ] Consume a completed two-stroke chord so the single-stroke binding does not also fire
  - Acceptance Criteria:
    - Functional: with the editor focused, `Ctrl+X Ctrl+O` opens the Control
      Center palette and does not fire `documents.clientOpenFileDialog`; other
      two-stroke chords behave the same, and single-stroke bindings (`Ctrl+B`,
      `Ctrl+O` alone, `Ctrl+Shift+\`) keep working; the pending-chord state clears
      on timeout, on a non-matching stroke, and on blur.
    - Performance: chord handling stays a keydown-path decision with the existing
      1 s timer; no polling, no extra dispatch per keystroke, and the pending state
      is per-editor.
    - Code Quality: the fix is one ordering/consumption change in the existing
      manifest keymap construction (`frontend/src/editor/extensions/behavior.ts`)
      — no second chord parser, no duplicated modifier matching — and the shell
      path (`frontend/src/shell/use-shell-chords.ts`) keeps its server-first rule
      for non-editor focus.
    - Security: no new authority; the fix cannot bind a trusted-only command and
      does not change which commands a manifest may declare.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/ui.md` (client architecture, key
        routing), `docs/wiki/modules/behavior-manifests.md`.
      - `frontend/src/editor/extensions/behavior.ts` (`manifestKeymaps`,
        `chordKeymap`, `eventMatches`, `isChordNoise`),
        `frontend/src/shell/use-shell-chords.ts`.
      - CodeMirror 6 keymap/`ViewPlugin` ordering rules (extension order decides
        which handler sees the event first).
    - Options Considered:
      - Put the chord plugin before the single-stroke keymap: fixes the ordering,
        but a completed chord must also stop the event from reaching the later
        keymap — the consumption contract has to be explicit.
      - Track a pending-chord flag that the single-stroke keymap consults
        (chosen): makes consumption explicit and testable, and keeps both keymaps
        independent of CodeMirror's registration order.
      - Move chord handling into the shell handler: rejected — chords must work
        with editor focus, which is exactly where the shell handler defers.
    - Chosen Approach: an explicit pending-chord state shared by both keymaps
      (or, if simpler after reading the plugin lifecycle, one combined handler that
      resolves chords before singles), plus a vitest case that feeds the real
      two-stroke sequence.
    - API Notes and Examples:
      ```ts
      // the ordering today (behavior.ts:193) — singles are registered first
      return [keymap.of(singles), chordKeymap(chords, onCommand)];
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/extensions/behavior.ts`,
        `frontend/src/editor/extensions/*.test.ts` (new two-stroke case),
        `frontend/src/shell/shell-chords.test.tsx` if the shell path shares the
        state.
    - References:
      - `test-plan/artifacts/136-capability-grants/live-example-config/README.md`
        (finding 1, with the reproduction steps).
  - Test Cases to Write:
    - `completed_chord_does_not_fire_the_single_stroke_binding`: feed
      `Ctrl+X` then `Ctrl+O`, assert one dispatch (the palette command) and no
      `documents.clientOpenFileDialog`.
    - `single_stroke_bindings_still_fire` and `pending_chord_clears_on_timeout_and_blur`.

- [ ] Fix palette row activation so Enter runs the selected row
  - Acceptance Criteria:
    - Functional: with the palette open and a row selected, Enter runs that row
      (the footer's `↵ run` promise), and the server does not report
      `menu.unknown_session` for a freshly opened palette; the row's chord keeps
      working; if the investigation shows the activation intent is inherently
      stale for one path (e.g. a client-side session id minted before the server
      session existed), that path is fixed or the step is re-worded as a recorded
      ceiling with the reason and the workaround.
    - Performance: activation is a single client→server intent on the keypress;
      no polling and no re-send loop; the palette's existing render path is
      unchanged.
    - Code Quality: the fix is at the cause (session id lifecycle in
      `ServerMenuSessions` / the client intent) rather than a retry, with one
      comment naming the invariant (an activation intent carries a session id the
      server still holds), and the existing menu tests
      (`src/server/connection/tests/control_center_and_menus.rs`) cover the fixed
      path.
    - Security: the fix does not widen what an activation can do — the same
      command dispatch and authorization apply, and a stale session still fails
      closed with the diagnostic (never a silent wrong-command run).
  - Approach:
    - Documentation Reviewed:
      - `src/server/menu_sessions.rs` (`ServerMenuSessions`,
        `ServerMenuActivateOutcome`), `src/server/connection/menus.rs:336`
        (`handle_menu_activate`), `src/server/connection/mod.rs:317-323`
        (`unknown_menu_session_diagnostic`).
      - `docs/wiki/modules/transient-menu-round-trip.md` and
        `docs/wiki/modules/control-center.md` (the session lifecycle the pages
        promise).
      - `frontend/src/shell/` palette component and its activation intent path.
    - Options Considered:
      - Client retries with a fresh session id: rejected — hides a lifecycle bug
        and can double-run a command.
      - Fix the session id lifecycle (chosen): the intent must reference the
        session the server holds; the diagnostic stays for genuinely stale
        intents.
      - Re-word the step as a ceiling: fallback only if the review shows the
        intent cannot carry a live id.
    - Chosen Approach: reproduce with the plan-136 driver, trace the session id
      from the palette snapshot to the activation intent, fix the mismatch at the
      owner, and pin it with a server-side test plus a live leg.
    - API Notes and Examples:
      ```bash
      # the live reproduction the driver already performs
      test-plan/artifacts/136-capability-grants/drive-example-config.sh
      grep -n "menu.unknown_session" /tmp/clay-plan136-example/server.log
      ```
    - Files to Create/Edit:
      - `src/server/menu_sessions.rs` and/or `src/server/connection/menus.rs`,
        `frontend/src/shell/` palette activation path,
        `src/server/connection/tests/control_center_and_menus.rs`.
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` → `## Further Actions` item 9 (finding 2).
  - Test Cases to Write:
    - `palette_row_activation_uses_a_live_session`: activating a row from a
      freshly opened palette dispatches the command and produces no
      `menu.unknown_session` diagnostic.
    - `stale_menu_session_still_fails_closed`: an activation with an unknown
      session id keeps the bounded diagnostic and dispatches nothing.

- [ ] UI prototype gate coverage check (prototype only if uncovered)
  - Acceptance Criteria:
    - Functional: the task confirms the plan changes no component, token,
      typography role, layout rule, or design-language value — the palette surface
      is covered by the approved artifact
      (`design-artifacts/approved/agent-lane-palette/`) and the chord work is
      keymap plumbing — and records that confirmation with the artifact paths and
      the coverage they provide.
    - Performance: no new UI surface means no new render path; the check records
      that the palette's existing render triggers are unchanged.
    - Code Quality: if any state the fix touches is *not* shown by the approved
      artifact (e.g. a new error/status text in the palette), the task stops and
      opens the prototype/approval loop for that state instead of inventing it.
    - Security: the check confirms the fixes add no user-visible authority claim
      (no new action, no new destructive affordance).
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/README.md`, `design-artifacts/approved/agent-lane-palette/README.md`
        (coverage: states, themes, widths).
      - `DESIGN.md` §12/§14, `.agents/skills/clay-execution/references/ui.md`.
    - Options Considered:
      - Skip the check because the plan is behavior-only: rejected — the gate is
        checked, not assumed (plan 147 precedent).
      - Check coverage and proceed (chosen): the touched states are already
        approved.
    - Chosen Approach: record the coverage check; escalate to the prototype loop
      only for an uncovered state.
    - API Notes and Examples:
      ```bash
      ls design-artifacts/approved/agent-lane-palette/
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/156-runtime-client-followups/ui-gate-check.md`.
    - References:
      - `plans/147-Client-Lane-Gaps-Caret-Wrap-and-Pane-Focus.md` (same check).
  - Test Cases to Write:
    - None (coverage check); the outcome is recorded in the task evidence.

- [ ] Verify Clay JS APIs and configuration surfaces are unchanged (automated-only plan)
  - Acceptance Criteria:
    - Functional: the plan's fixes change no Clay JS API, no op surface (the
      op-level key rejection adds validation, not a new key), and no configuration
      key; the verification records the commands and their output showing the
      inventory, registry, parity ledger, and docs are unchanged.
    - Performance: no facade or configuration path gains work; the only op change
      is a key-set check on a registration call.
    - Code Quality: the verification runs the inventory/registry/doc guard tests
      (`cargo test --test protocol clay_js_api_inventory:: primitives_docs::
      documentation_coverage::`) and records that no generated artifact moved.
    - Security: the verification confirms the rejected-key error is documented on
      the affected API pages (so the behavior change is visible to authors) and
      that no new authority was introduced.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`;
        `docs/reference/clay-js-api/api-inventory.toml`.
      - `plans/148-Gate-Hygiene-Flake-Determinism-and-Toolchain-Pinning.md`
        (the "unchanged" verification precedent).
    - Options Considered:
      - Treat the op-key change as an API change requiring promotion work:
        rejected — no new key or facade is added; the error text is documented on
        the existing page.
    - Chosen Approach: verify and record; document the new error on the touched
      API pages.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js_api_inventory:: primitives_docs:: documentation_coverage::
      git status --porcelain docs/generated docs/index.md
      ```
    - Files to Create/Edit:
      - The affected `docs/reference/clay-js-api/**` pages (error text only).
    - References:
      - `plans/136-Third-Party-Capability-Grants-and-Provider-Lane-Observability.md` task 8 outcome (guard contract).
  - Test Cases to Write:
    - Existing guard tests stay green; the recorded output shows no generated
      artifact drift.

- [ ] Update the canonical example configuration (examples/config/init.js) or record no change
  - Acceptance Criteria:
    - Functional: if the plan adds no configuration surface, the task records the
      verification (`node --check` plus a diff check showing the example is
      untouched); if the op-key rejection changes a documented example, the
      affected comment is corrected.
    - Performance: `node --check examples/config/init.js` passes.
    - Code Quality: the example keeps one annotated occurrence per surface and the
      documented ordering constraints.
    - Security: no active grant or credential is introduced by the example change.
  - Approach:
    - Documentation Reviewed:
      - `examples/config/README.md`, `examples/config/packages/third-party.js`;
        `tests/clay_js_doc_registry.rs::canonical_example_active_configuration_is_copy_safe`.
    - Options Considered:
      - Skip the task: rejected — the duty is to verify and record, even when the
        answer is "no change".
    - Chosen Approach: verify and record; edit only if an example comment is now
      wrong.
    - API Notes and Examples:
      ```bash
      node --check examples/config/init.js
      git diff --stat examples/config/
      ```
    - Files to Create/Edit:
      - `examples/config/init.js` (only if a comment is now stale).
    - References:
      - user instruction 2026-08-03 (example config maintenance duty).
  - Test Cases to Write:
    - `canonical_example_active_configuration_is_copy_safe` stays green.

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: a real Linux build launched against a copy of `examples/config/`
      in an isolated scratch root reaches Connected with no `configuration failed`
      diagnostics, and the two fixed behaviors are observed live: the
      `Ctrl+X Ctrl+O` chord opens the palette without opening a file dialog, and
      Enter on the selected palette row runs it with no `menu.unknown_session`
      diagnostic.
    - Performance: startup timings are recorded and compared with the plan-136
      baseline (socket 425 ms, first config effect 493 ms, editor visible
      1388 ms); the palette opens in the same interaction budget as before.
    - Code Quality: the launch command, scratch root, driver script, and observed
      results are recorded; a regression under the example config is a defect.
    - Security: mode-700 scratch root with its own HOME/XDG/TMPDIR and socket;
      PID-based teardown (`run-live.sh stop`), never pattern kills.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/artifacts/136-capability-grants/live-example-config/README.md`
        (the exact reproduction steps for both findings).
      - `test-plan/artifacts/136-capability-grants/drive-example-config.sh`
        (verified affordances: mmsg focus guard, portal screenshots, cross-stroke
        chord synthesis).
      - `test-plan/artifacts/127-lane-scheduling/run-live.sh` (`example-config`
        mode).
    - Options Considered:
      - Reuse the plan-136 driver unchanged (chosen): it already reproduces both
        findings and its affordance list is verified.
      - Write a new driver: rejected — the existing one is the evidence baseline.
    - Chosen Approach: launch, drive the chord and the palette row, capture the
      screenshots/trees, and confirm no `menu.unknown_session` line.
    - API Notes and Examples:
      ```bash
      CLAY_LIVE_ROOT=/tmp/clay-plan156-example \
        test-plan/artifacts/136-capability-grants/drive-example-config.sh
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/156-runtime-client-followups/live-example-config/`.
    - References:
      - user instruction 2026-09-07 (launch-test duty).
  - Test Cases to Write:
    - Manual launch leg: chord opens only the palette; Enter runs the row; no
      unknown-session diagnostic.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: the palette and editor-focus states touched by the fixes are
      captured from the running app and compared with the approved artifact; each
      deviation is fixed or re-approved with the reason recorded.
    - Performance: captures record the palette open time and confirm no new render
      work was added.
    - Code Quality: captures cover the palette in its states (open, filtered,
      selected, executed, error/status text) across the shipped content themes and
      both widths, plus a keyboard-only pass over the chord and the Enter
      activation.
    - Security: the review confirms the status/diagnostic text does not leak
      package or workspace paths and that the fixes add no new affordance.
  - Approach:
    - Documentation Reviewed:
      - `design-artifacts/approved/agent-lane-palette/README.md` (binding),
        `DESIGN.md` review checklist, `references/ui.md`.
      - `scripts/capture-ui-review.sh` and the `ui-review-*` fixtures.
    - Options Considered:
      - Component tests only: rejected — the gate requires a running-app
        comparison.
    - Chosen Approach: capture with the existing harness, compare, record
      deviations.
    - API Notes and Examples:
      ```bash
      scripts/capture-ui-review.sh --fixture ui-review-control-center
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/156-runtime-client-followups/visual-review/`.
    - References:
      - user instruction 2026-09-11 (visual/accessibility review duty).
  - Test Cases to Write:
    - Review record: per-surface deviation table with resolution and the
      keyboard-only pass result.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: the affected steps are executed and updated on a real Linux
      build — the Control Center chord step now asserts the palette opens *and* no
      file dialog appears, the palette row step asserts Enter runs the row, and
      the lane-budget decision is recorded against the performance module's lane
      step (Q44) — with pass/fail per step.
    - Performance: the steps record the lane counters and the `server.edit_ack`
      envelope from the live run against the plan-136 baseline, and state the
      lane-budget answer.
    - Code Quality: `test-plan/index.md` (module map, coverage matrix, execution
      record) and the parity ledger are updated in the same task; any step whose
      expectation changes is re-worded honestly rather than deleted.
    - Security: the steps note that the fixes add no authority and that a stale
      menu session still fails closed with the bounded diagnostic.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/02-*.md` / `test-plan/13-*.md` (the palette and chord steps —
        exact module identified in the baseline), `test-plan/11-performance.md`
        (Q44), `test-plan/index.md`, `tests/documentation_coverage.rs`.
      - `test-plan/artifacts/136-capability-grants/manual-plan/README.md`.
    - Options Considered:
      - Add new step IDs: only if the existing palette steps cannot carry the
        assertion; otherwise tighten the existing steps (the honest fix).
    - Chosen Approach: tighten the existing palette/chord steps and record the
      lane decision, then update the ledger and index.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol documentation_coverage::parity_ledger
      ```
    - Files to Create/Edit:
      - The affected `test-plan/` module file(s), `test-plan/index.md`,
        `docs/development/tauri-react-parity-ledger.json`.
    - References:
      - plan 136 task 13 outcome (manual-plan execution pattern).
  - Test Cases to Write:
    - Manual steps with expected results and negatives; ledger and
      documentation-coverage tests stay green.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the page that owns the Control Center round trip
      (`docs/wiki/modules/transient-menu-round-trip.md` / `control-center.md`)
      documents the session-id invariant behind row activation and the chord
      consumption rule with the single-stroke fallback; the runtime page carries
      the final lane-budget answer; the registration-op key rejection is
      documented where authors read it (`docs/reference/clay-js-api/**` plus the
      relevant wiki page if the op boundary is explained there).
    - Performance: the wiki states the lane decision with its numbers and the
      trigger, and confirms the client fixes add no per-keystroke work.
    - Code Quality: pages follow the wiki template, link the authoritative API
      pages, and list source/test paths; `docs/wiki/index.md` descriptions stay
      accurate.
    - Security: the pages state that a stale menu session fails closed, that the
      chord fix grants no authority, and that unknown option keys are now
      rejected rather than ignored.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md` (wiki workflow,
        template, quality bar), `tests/documentation_coverage.rs`.
    - Options Considered:
      - Update per task: rejected — update once after tests pass (plan-136
        precedent).
    - Chosen Approach: update the menu/control-center and runtime pages plus the
      touched API pages, add a contract test with markers, and update the index
      descriptions.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol documentation_coverage::
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/transient-menu-round-trip.md` (or `control-center.md`),
        `docs/wiki/modules/persistent-runtime-hardening.md`,
        `docs/reference/clay-js-api/**` (error text), `docs/wiki/index.md`,
        `tests/documentation_coverage.rs`.
    - References:
      - plan 136 task 14 outcome (wiki contract-test pattern).
  - Test Cases to Write:
    - `plan156_wiki_pages_describe_chord_consumption_and_lane_budget`: marker test
      plus a stale-claim scan for "the palette row's Enter is not wired" and the
      superseded lane-trigger wording.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
