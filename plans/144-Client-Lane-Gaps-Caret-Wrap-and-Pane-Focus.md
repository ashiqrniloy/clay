# 144 — Client Lane Gaps: Caret Delivery and Rendering, Wrap Policy, and Pane Focus

Source: `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → `## Further Actions`
(the four client gaps found by its manual pass, recorded 2026-09-21). All four
were reproduced on the pre-change build with the plan-132 changes stashed, so
none is a plan-132 regression: the lanes deliver their values correctly and the
webview drops or ignores them. Evidence and reproduction notes:
`test-plan/artifacts/132-fanout-dedup/README.md`.

No UI prototype gate applies. Every affected value is already specified and
validated outside this plan: `examples/config/init.js:204-248` documents the
caret and wrap option sets, `examples/config/init.js:578-589` documents
`paneFocusPolicy`, `src/protocol/mod.rs` (`CaretStyle`, `WrapPolicy`,
`ShellPreferences`) validates them server-side, and `test-plan/07` T2/T3/T5/T21-T24
plus `test-plan/13` S14/S17 already state the expected result. This plan makes
the shipped webview honor those declarations — no component, token, typography
role, layout rule, or design-language value is introduced or changed. If the
visual review (task 6) finds a caret appearance decision that the declared
values do not settle (outline width, blink rhythm), the plan stops and enters the
prototype/approval loop instead of choosing values ad hoc.

## Objectives

- C1: a caret override that arrives during connection initial sync paints on the
  editor that is created afterwards, and every field of `CaretStyle` that
  `examples/config/init.js` and the protocol document takes effect: `shape`
  (bar/line/block/underline), `widthPx`, `heightPct`, `hollow`, `blink`
  (solid/blink/phase/smooth with their parameters), and `stopBlinkOnTyping`.
  `smoothAnimationMs` stays a documented reserved value.
- C2: `clientSetEditorLayout` honors all three documented `wrapPolicy` forms —
  `"none"`, `"viewport"`, and `"column"` with `columnCap` — and an out-of-range
  cap is clamped to the protocol's 16–240 window instead of producing an
  undefined wrap width.
- C3: `setPaneFocusPolicy` has a real consumer: `"click"` keeps today's
  behavior, `"cursor"` focuses the pane the pointer is over, and the webview
  reads the shell-preferences lane instead of ignoring it. If implementation is
  rejected at review, the fallback objective is the documented alternative:
  re-word `test-plan/13` S14/S17 and the `examples/config/init.js` comment as a
  known ceiling.

## Expected Outcome

- With a caret override set in `init.js` and the app started fresh (override
  arrives on initial sync), the caret renders with that shape/width/height and
  honors `hollow` and the blink policy; reloading configuration (override
  arrives on the live lane) renders identically — one code path, not two.
- `wrapPolicy: "none"` produces horizontal scroll with no wrap, `"viewport"`
  wraps at the pane width, `"column"` with `columnCap: 100` wraps at 100 columns,
  and `columnCap: 9999` clamps to 240; no configuration evaluation or client
  handler throws for any documented form.
- With `paneFocusPolicy: "cursor"`, moving the pointer into a split pane focuses
  it; with `"click"` nothing changes relative to today.
- `npx vitest run src/editor` (and the shell tests) cover each new branch; the
  manual pass records per-step results for `test-plan/07` T2/T3/T5/T21-T24 and
  `test-plan/13` S14/S17 with burst-capture evidence for the caret steps.
- `cargo fmt --check`, `cargo check/clippy --all-targets -- -D warnings`,
  `cargo test --all-targets`, `cargo test -p clay-desktop --all-targets`,
  `npm run typecheck`, `npm run lint`, `npm test`, `npm run build` are green.

## Tasks

- [ ] Baseline: reproduce the four lane gaps, inventory their code paths, and confirm no Rust primitive is missing
  - Acceptance Criteria:
    - Functional: each gap is reproduced with a concrete observation and its exact code path — (a) initial-sync caret drop: `frontend/src/editor/extensions/controller.ts:627-628` returns while `this.view` is null, and the same override delivered on the live lane (`case "caretStyleOverride"`, `controller.ts:419-421`) does paint, shown by one live leg (fresh start vs Control Center reload) plus the code path; (b) wrap policy: `controller.ts:660-671` parses the override with `"none" in wrap`, which throws `TypeError` for the string forms the protocol emits (`WrapPolicy` = `"none" | "viewport" | { "column": number }`, `frontend/src/bridge/generated/bridge.ts:629`), reproduced in a unit-level harness; (c) caret fields: `applyCaret` reads only `shape` and `widthPx`, so `heightPct`, `hollow`, `blink`, and `stopBlinkOnTyping` are delivered and ignored; (d) `paneFocusPolicy`: `grep -rn "paneFocus\|shellPreferences" frontend/src --include=*.ts --include=*.tsx` returns no non-generated consumer.
    - Performance: the baseline records the frontend editor suite runtime and states the budget each fix must respect — the caret/layout handlers run on a configuration event, never per keystroke, and a policy that did not change must not re-dispatch.
    - Code Quality: the baseline lists every document that already promises the behavior, with the line that would become stale if the fallback wording is chosen instead — `examples/config/init.js:204-248`, `:578-589`, `test-plan/07-caret-and-typography.md` (T2/T3/T5/T21-T24), `test-plan/13-window-splits.md` (S14/S17), and the caret/wrap/focus deep-reference docs under `docs/`; it also confirms, after reading `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, and `docs/wiki/modules/primitive-architecture.md`, that all four gaps are client-side rendering/behavior over values the server already validates, so no new Rust primitive is required (if that turns out false, the task records the primitive gap and stops).
    - Security: the baseline records that all four values are inert presentation data validated server-side (`CaretStyle` bounds, `WrapPolicy::clamp_column`, `ShellPreferences` policy string) and that no fix may add a new op, IPC surface, or authority; focus-follows-pointer is a client-side focus decision only.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/wiki/modules/primitive-architecture.md`: primitive inventory before proposing any new Rust surface.
      - `.agents/skills/clay-execution/references/ui.md` and `DESIGN.md`: editor chrome vs shell UI ownership, accent-for-state rule, reduced-motion expectations for blinking.
      - `src/protocol/mod.rs` (`CaretStyle`, `BlinkStyle`, `CaretShape`, `WrapPolicy`, `ShellPreferences`), `frontend/src/bridge/generated/bridge.ts`: the validated value sets and their exact wire forms.
      - `test-plan/07-caret-and-typography.md`, `test-plan/13-window-splits.md`, `test-plan/index.md`: the steps that define pass for each behavior.
    - Options Considered:
      - Treat the gaps as documentation bugs and re-word the test-plan steps: cheapest, but three of the four are documented product options users can already set, so the app would keep silently ignoring its own configuration.
      - Implement each gap client-side (chosen): the values are already validated and transported; the missing work is rendering and one parse fix.
      - Move any of it server-side (e.g. resolve the wrap width in Rust): rejected — the pane width is a client layout fact, and the server already publishes the policy in the protocol's own units.
    - Chosen Approach: reproduce and inventory first, then fix in the webview only, reusing the existing lane payloads and the existing CodeMirror compartments.
    - API Notes and Examples:
      ```bash
      # unit-level reproduction of the wrap parse bug (no GUI needed)
      npx vitest run src/editor --reporter=verbose
      # live legs: harness from the plan-132 manual pass
      test-plan/artifacts/132-fanout-dedup/live/run-live.sh caret
      ```
      ```ts
      // the wire forms the client must accept (generated contract)
      // WrapPolicy = "none" | "viewport" | { "column": number }
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/144-client-lane-gaps/baseline.md`: reproduction, code paths, stale-document inventory, primitive confirmation.
    - References:
      - `plans/132-Fanout-and-Protocol-Contract-Deduplication.md` → `## Further Actions`.
      - `frontend/src/editor/extensions/controller.ts:419-421`, `:627-671`.
      - `frontend/src/bridge/generated/bridge.ts:35`, `:88-103`, `:629`.
  - Test Cases to Write:
    - Baseline reproduction: each of the four gaps observed once with its code path cited, so a later fix can be shown to change exactly that behavior.

- [ ] Deliver the caret override on initial sync and render every documented caret field
  - Acceptance Criteria:
    - Functional: an override that arrives before the editor view exists is remembered and applied when the view is created (or the handler re-reads the stored override at view creation), so a fresh start with `clientSetCursorStyle({ shape: "block", hollow: true })` renders an outlined block exactly as a live reload does; `shape` covers bar/line/block/underline, `widthPx` and `heightPct` are applied, `hollow` renders `Block` as an outline, `blink` implements `"solid"`, `{"blink":{onMs,offMs,waitMs}}`, `{"phase":{periodMs}}`, and `{"smooth":{periodMs}}`, and `stopBlinkOnTyping: true` resets the blink to visible on typing; `smoothAnimationMs` remains unread and is documented as reserved.
    - Performance: caret work happens on the override event and on view creation only; no per-keystroke dispatch beyond the existing typing reset, and no polling timer — blink uses CSS/`requestAnimationFrame`-free scheduling that stops when the window is hidden or the caret is not blinking.
    - Code Quality: one projection function from the validated DTO to the CodeMirror theme/extension configuration (no second parser), each branch unit-tested in `frontend/src/editor`, and the deferred-override path covered by a test that constructs the controller without a view; no new Clay JS API, op, or protocol field.
    - Security: no new authority; the client clamps geometry defensively (width ≥ 1, height within 0.1–2.0) even though the server validates, and colour continues to resolve through the active theme token rather than a literal.
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md` and `.agents/skills/clay-execution/references/ui.md`: accent-for-state, reduced-motion, and editor-chrome ownership rules.
      - `src/protocol/mod.rs` `CaretStyle`/`BlinkStyle`/`CaretShape` docs; `examples/config/init.js:204-222`; `test-plan/07-caret-and-typography.md` T2/T3/T5.
      - CodeMirror 6 `EditorView.theme`/`drawSelection` behaviour for `.cm-cursor` (cursor blink is not built in; the theme must supply it).
    - Options Considered:
      - Custom `requestAnimationFrame` blink loop: full control, but a timer per editor and a reduced-motion hazard.
      - CSS animation on `.cm-cursor` gated by `prefers-reduced-motion` (chosen for the blink variants): declarative, no timer, honors the OS setting; phase/smooth parameters map to animation duration/delay.
      - Ignore `hollow`/`blink` and re-word the docs: rejected — the options are shipped in the API, the example config, and the manual test plan.
    - Chosen Approach: remember the last override on the controller, apply it at view creation and on every later override through the existing `behaviorCompartment` reconfigure, and express blink as theme CSS derived from the DTO values.
    - API Notes and Examples:
      ```ts
      // initial-sync delivery: keep the value, apply when the view exists
      private applyCaret(raw: unknown): void {
        this.pendingCaret = raw ?? this.pendingCaret;   // remember
        if (!this.view) return;                          // apply later
        // ...existing reconfigure with shape/widthPx/heightPct/hollow/blink...
      }
      ```
      ```js
      clientSetCursorStyle({ shape: "block", hollow: true, blink: { blink: { onMs: 500, offMs: 500, waitMs: 1000 } } });
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/extensions/controller.ts`: caret projection (remember/apply, shape/width/height/hollow/blink), doc comment for the reserved field.
      - `frontend/src/editor/extensions/*.test.ts`: caret projection and deferred-application tests.
      - `test-plan/07-caret-and-typography.md`: step results; wording changes only if a documented value is re-scoped.
    - References:
      - `frontend/src/editor/extensions/controller.ts:627-656` (current `applyCaret`), `:419-421` (live lane).
      - `docs/wiki/modules/editor-movement-selection-caret.md`: `CaretStyle` field semantics.
  - Test Cases to Write:
    - `applyCaret` before view creation then view creation applies the remembered override: validates the initial-sync path.
    - One test per `shape`/`hollow`/`blink` variant asserting the generated `.cm-cursor` style or class: validates rendering.
    - `stopBlinkOnTyping` typing reset: validates the documented reset behavior.
    - Reduced-motion (`matchMedia`) run: blink suppressed, caret still visible.

- [ ] Accept the documented wrap-policy forms and clamp the column cap
  - Acceptance Criteria:
    - Functional: `applyLayout` accepts `"none"`, `"viewport"`, and `{ "column": N }` without throwing, maps them to the manifest `layout.wrap` values, applies `columnCap` when present, and clamps an out-of-range or missing cap to the protocol window (16–240, default 72) instead of emitting `{column: undefined}`; an unknown value leaves the previous policy in place rather than throwing.
    - Performance: an unchanged policy does not re-dispatch the behavior compartment, and the handler does no layout measurement itself (the client-side wrap extension already owns that).
    - Code Quality: one exhaustive parse over the three documented wire forms with a `default` that ignores unknown input, covered by unit tests for each form plus the previously throwing string cases; no new dependency.
    - Security: the client-side clamp mirrors `WrapPolicy::clamp_column` bounds without becoming a second authority (the server value stays validated), and no new op or IPC is introduced.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/mod.rs` `WrapPolicy` (`MIN_COLUMN`/`MAX_COLUMN`/`DEFAULT_COLUMN`, `clamp_column`) and the generated `WrapPolicy` union.
      - `examples/config/init.js:225-248`; `test-plan/07-caret-and-typography.md` T21-T24.
      - `frontend/src/editor/extensions/behavior.ts` / `layout.ts` wrap extension: how the manifest policy reaches CodeMirror.
    - Options Considered:
      - Stringify/sniff with `in` checks as today: rejected, throws for the unit variants.
      - Discriminated parse on `typeof raw === "string"` first, then the object form (chosen): matches the generated union exactly.
      - Loosen the protocol to always send `{column: N}`: rejected — it would change the wire contract for a client-only bug.
    - Chosen Approach: parse the three documented forms in the generated contract's own shape, clamp with the same constants, and ignore anything else.
    - API Notes and Examples:
      ```ts
      type WrapPolicy = "none" | "viewport" | { column: number };
      const parseWrap = (raw: unknown): WrapPolicy | null =>
        raw === "none" || raw === "viewport"
          ? raw
          : raw && typeof raw === "object" && typeof (raw as { column?: unknown }).column === "number"
            ? { column: Math.min(240, Math.max(16, (raw as { column: number }).column)) }
            : null;
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/extensions/controller.ts`: `applyLayout` parse + clamp.
      - `frontend/src/editor/extensions/*.test.ts`: per-form tests.
      - `test-plan/07-caret-and-typography.md`: T21-T24 results.
    - References:
      - `frontend/src/editor/extensions/controller.ts:660-687` (current parse).
      - `src/protocol/mod.rs` `WrapPolicy::clamp_column`.
  - Test Cases to Write:
    - Each wire form (`"none"`, `"viewport"`, `{column:100}`, `{column:9999}`, `{column:1}`, `{}`, `null`) produces the expected manifest policy or leaves it unchanged: validates the parse and clamp.

- [ ] Consume the pane-focus policy (focus follows pointer) or re-word the steps as a ceiling
  - Acceptance Criteria:
    - Functional: the webview reads the shell-preferences lane and maps `paneFocusPolicy` to the shell's focus behavior — `"click"` keeps today's behavior exactly, `"cursor"` focuses the pane under the pointer as the pointer enters it (as `src/protocol/mod.rs` `ShellPreferences` documents: "focus follows pointer hover"), and a policy change takes effect without restart. If implementation is rejected at review, the fallback is recorded instead: `test-plan/13` S14/S17 and the `examples/config/init.js:578-589` comment are re-worded as a known ceiling, and the unused lane is documented as delivered-but-unconsumed.
    - Performance: focus changes only on pointer entry/pane switch, never on pointer move sampling; no new listener on the editor keypress path, and the shell-preferences subscription must not cause a shell re-render per event when the policy is unchanged.
    - Code Quality: one place maps the policy string to behavior (a typed `PaneFocusPolicy`-style union in the shell layer, matching the protocol comment's expectation), covered by a unit test for both values plus an unknown string; the lane subscription is wired the same way as the other shell-level lanes.
    - Security: no new authority — focus is a client-side interaction decision, the policy string stays validated server-side, and unknown values fall back to `"click"`.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/mod.rs` `ShellPreferences` (including the "client maps the string to its `PaneFocusPolicy` enum" expectation), `src/server/ops/shell.rs` validation.
      - `test-plan/13-window-splits.md` S14/S17; `examples/config/init.js:578-589`.
      - `.agents/skills/clay-execution/references/ui.md` and `DESIGN.md`: focus visibility and keyboard-flow expectations (a pointer-driven focus policy must not break keyboard focus order).
    - Options Considered:
      - Implement `"cursor"` as focus-follows-pointer (chosen): matches the protocol doc and the option's name; low risk because `"click"` stays the default.
      - Implement focus-follows-*caret*: rejected — the protocol documents pointer hover, and it would change typing behavior in unfocused panes.
      - Re-word the docs and keep the lane unconsumed (fallback): acceptable only if review rejects the behavior change.
    - Chosen Approach: wire the existing shell-preferences value into the shell's pane focus handling, defaulting to today's behavior for every unknown value.
    - API Notes and Examples:
      ```js
      setPaneFocusPolicy({ paneFocusPolicy: "cursor" }); // focus follows pointer hover
      setPaneFocusPolicy({ paneFocusPolicy: "click" });  // default
      ```
    - Files to Create/Edit:
      - `frontend/src/shell/**`: policy type, lane subscription, pane focus handling.
      - `frontend/src/shell/**/*.test.ts`: policy mapping tests.
      - `test-plan/13-window-splits.md`: S14/S17 results (or the re-worded ceiling).
      - `examples/config/init.js`: comment correction only if the fallback is chosen.
    - References:
      - `src/server/ops/shell.rs` `publish_shell_preferences`; `src/server/js_runtime/mod.rs:159` lane.
      - `docs/wiki/modules/server-state-fanout.md`: the lane's delivery/replay semantics.
  - Test Cases to Write:
    - Policy mapping: `"click"`, `"cursor"`, unknown, and missing values resolve to the expected behavior.
    - Lane delivery: a policy published after connect changes behavior without a restart.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: a real Linux build is exercised for every changed state — caret shape/width/height/hollow/blink variants (default, block hollow, underline, blinking, reduced-motion), the three wrap policies with a long-line fixture, and both pane-focus policies in a split layout — with screenshots stored under `test-plan/artifacts/144-client-lane-gaps/review/` and the paths recorded; caret evidence is a burst capture (at least four frames spanning a blink cycle), never a single frame.
    - Performance: the review notes any visible input latency or layout jank introduced by the new handlers, and the automated editor suite runtime is compared with the baseline.
    - Code Quality: findings are recorded per state with the screenshot path, and each deviation is either fixed or filed as a prioritized follow-up; the review explicitly states whether caret appearance needed design decisions beyond the declared values (which would require the prototype/approval loop).
    - Security: the review confirms the caret colour still resolves through the active theme token, that reduced-motion suppresses blinking, and that keyboard-only focus order is unchanged under `paneFocusPolicy: "cursor"` (pointer focus must not trap or steal keyboard focus).
  - Approach:
    - Documentation Reviewed:
      - `DESIGN.md`; `.agents/skills/clay-execution/references/ui.md`; `test-plan/index.md` (Conventions, including the burst-capture rule for blink evidence).
      - Existing harness: `test-plan/artifacts/132-fanout-dedup/live/run-live.sh`, `probe.py`, `portal-shot.py`.
    - Options Considered:
      - Reuse the plan-132 harness with caret/wrap/panes modes (chosen): already exercises the real app with isolated roots and AT-SPI probing.
      - New harness: rejected, no new capability needed.
    - Chosen Approach: run the existing harness per state, capture bursts, and record findings with the step IDs they correspond to.
    - API Notes and Examples:
      ```bash
      test-plan/artifacts/132-fanout-dedup/live/run-live.sh caret
      # burst capture of one blink cycle, then crop to the caret region
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/144-client-lane-gaps/review/**`: screenshots + findings.
    - References:
      - `test-plan/artifacts/132-fanout-dedup/README.md`: the false-negative lesson that motivated burst capture.
  - Test Cases to Write:
    - Manual state matrix (caret × wrap × focus): each state captured and judged against the declared expectation.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: the plan adds no public programmatic surface — `clientSetCursorStyle`, `clientSetEditorLayout`, and `setPaneFocusPolicy` already exist with documented IDs, so the task verifies each documented option is now honored and records "no JS API change" with the diff evidence; any newly read `CaretStyle`/`WrapPolicy` field is confirmed to be documented in its API page (or the page is updated).
    - Performance: no new op or facade call is added, so no new IPC cost; the task records that the fixes stay in the client projection.
    - Code Quality: the API docs' option tables, defaults, and allowed values match `src/protocol/mod.rs` and the generated contract after the fix; `cargo test` documentation gates stay green.
    - Security: API docs keep the "validated server-side" and permission notes unchanged; no API is promoted from planned to implemented by this plan.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`: dotted-ID convention and the documentation gates.
      - `docs/reference/**` pages for `clientSetCursorStyle`, `clientSetEditorLayout`, `setPaneFocusPolicy`; `api-inventory.toml` if the touched options carry custom properties.
    - Options Considered:
      - Add a new API for caret blink control: rejected — the existing option covers it.
      - Verify-only with recorded evidence (chosen): the plan implements documented behavior rather than new surface.
    - Chosen Approach: verify-only, with any doc drift for the touched fields fixed in the same task.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol -- js_api_docs
      ```
    - Files to Create/Edit:
      - `docs/reference/**`: touched-option rows only if they drifted from the protocol.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
  - Test Cases to Write:
    - Documentation gate run: existing JS API doc/lookup tests stay green with no new entry required.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: the three configuration surfaces this plan makes effective (`clientSetCursorStyle` fields, `clientSetEditorLayout` wrap forms, `setPaneFocusPolicy`) are verified as documented Clay JS APIs with their option names, types, defaults, and allowed values matching the validated parsers; no new configuration key is introduced.
    - Performance: configuration evaluation cost is unchanged (same ops, same payloads); the task records that no new generation or reload path was added.
    - Code Quality: any option table or custom-property entry that claimed a behavior the client did not implement is corrected to the shipped behavior.
    - Security: configuration still grants no filesystem, network, shell, extension, or workspace authority; the task confirms none of the three APIs touches those boundaries.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/config.md`; `docs/index.md` links for the three APIs; `api-inventory.toml` custom properties for the touched options.
    - Options Considered:
      - New configuration surface: not needed — the plan implements existing options.
      - Verify + correct drift (chosen).
    - Chosen Approach: verify each option end to end (docs → parser → protocol → client) and correct drift in the same task.
    - API Notes and Examples:
      ```bash
      node --check examples/config/init.js
      cargo test --test protocol -- config
      ```
    - Files to Create/Edit:
      - `docs/reference/**`, `api-inventory.toml`: only if a touched option's documentation drifted.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
  - Test Cases to Write:
    - Option parity check: for each touched option, docs, parser, and client behavior agree (recorded as a table in the task evidence).

- [ ] Update the canonical example configuration (examples/config/init.js)
  - Acceptance Criteria:
    - Functional: the caret section (`:204-222`), the wrap section (`:225-248`), and the pane-focus section (`:578-589`) stay comprehensive and accurate: every documented option appears once with its type, default, and allowed values, the commented examples reflect the implemented behavior, and `smoothAnimationMs` is annotated as reserved if it remains unread; the file stays valid JavaScript and its uncommented part stays safe to copy.
    - Performance: no change to what the example does at load time (the same one-line calls), so example-config launch cost is unchanged.
    - Code Quality: option names, enums, and defaults are cross-checked against `src/protocol/mod.rs` and the API docs rather than prose; ordering constraints elsewhere in the file are preserved.
    - Security: the example grants no new authority; any permission/trust note in the touched sections stays accurate.
  - Approach:
    - Documentation Reviewed:
      - `examples/config/init.js` (existing section style); `docs/reference/**` for the three APIs; `src/protocol/mod.rs` for defaults.
    - Options Considered:
      - Leave the file untouched: rejected — the sections document values whose behavior changes in this plan (and one that stays reserved).
      - Update the touched sections only (chosen): the file is already comprehensive elsewhere.
    - Chosen Approach: annotate the touched options, verify with `node --check`, and keep the active lines identical to today's.
    - API Notes and Examples:
      ```bash
      node --check examples/config/init.js
      ```
    - Files to Create/Edit:
      - `examples/config/init.js`: caret/wrap/pane-focus comments and examples.
    - References:
      - User instruction 2026-08-03 (canonical example config maintenance duty).
  - Test Cases to Write:
    - `node --check` plus a manual read-through of the three sections against the protocol definitions.

- [ ] Launch-test the app with the canonical example config
  - Acceptance Criteria:
    - Functional: a real Linux GUI build runs against a copy of `examples/config/init.js` in an isolated scratch config root (never the developer profile), reaches Connected, commits a generation with no `configuration failed` diagnostics, and the plan's surfaces behave as configured: caret shape/hollow/blink render, the configured wrap policy applies to the right font role, and `paneFocusPolicy` behaves as configured in a split layout; the launch command, scratch path, and observations are recorded.
    - Performance: startup and reload times are recorded and compared with the plan-132 manual pass as a sanity baseline.
    - Code Quality: any surface that only works after a reload, or only on a fresh start, is recorded as a defect rather than a nuance.
    - Security: the launch uses an isolated root with no real workspace or credentials; the copied config is unmodified apart from the scratch path.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` Prerequisites; `test-plan/artifacts/132-fanout-dedup/live/run-live.sh` (isolated-root pattern).
    - Options Considered:
      - Launch against the developer profile: rejected (mutates real config and hides drift).
      - Isolated scratch root copy (chosen).
    - Chosen Approach: copy the config, launch server + client with `HOME`/config overridden, exercise the three surfaces, record evidence.
    - API Notes and Examples:
      ```bash
      # scratch root pattern used by the plan-132 harness
      test-plan/artifacts/132-fanout-dedup/live/run-live.sh caret
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/144-client-lane-gaps/example-config-launch.md`: command, paths, observations.
    - References:
      - User instruction 2026-09-07 (example config must be launch-tested, not just edited).
  - Test Cases to Write:
    - Live launch with the copied example config: caret, wrap, and pane-focus surfaces verified from configuration rather than from a manual override.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: `test-plan/07-caret-and-typography.md` T2/T3/T5/T21-T24 and `test-plan/13-window-splits.md` S14/S17 are executed on a real Linux build and recorded pass/fail against the numbered steps; if a step's expectation changes (e.g. the pane-focus fallback), the step is re-worded with the reason and the known ceiling recorded in the file's ceilings section, never deleted.
    - Performance: the pass records any step that could not be judged because the harness cannot sample the state (e.g. blink rhythm) and the method used instead (burst capture).
    - Code Quality: `test-plan/index.md` gains the plan-144 execution record and any module/coverage-matrix update the changes require; existing dated records stay untouched.
    - Security: no step's authority or permission check is weakened to make a pass; the record notes that all changed values remain server-validated.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map, coverage matrix, Conventions); modules 04, 07, 13.
    - Options Considered:
      - Re-run only the touched steps (chosen): the plan changes caret rendering, wrap parsing, and pane focus.
      - Full regression pass over every module: rejected as unnecessary for a client-side fidelity fix, with module 04's typing legs covered by the caret/wrap steps.
    - Chosen Approach: execute the touched steps, add no new numbered steps (no new user-visible behavior beyond the documented options), and record results.
    - API Notes and Examples:
      ```bash
      test-plan/artifacts/132-fanout-dedup/live/run-live.sh caret
      ```
    - Files to Create/Edit:
      - `test-plan/index.md`: plan-144 execution record.
      - `test-plan/07-caret-and-typography.md`, `test-plan/13-window-splits.md`: results, re-worded steps if the fallback is taken.
    - References:
      - `test-plan/artifacts/132-fanout-dedup/README.md`: prior false-negative lesson.
  - Test Cases to Write:
    - Manual execution of the eight touched steps with per-step evidence paths.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the wiki reflects the shipped caret projection, the wrap-policy parse, and the pane-focus consumer, or is explicitly verified unchanged with the reason.
    - Performance: wiki updates add no runtime work and note the event-driven (not per-keystroke) nature of the handlers.
    - Code Quality: pages explain what the client projection does, its invariants (deferred override, clamped cap, unknown-value fallback), and link from `docs/wiki/index.md`; `cargo test --test protocol -- documentation` stays green.
    - Security: pages state that all three values stay server-validated and that focus policy carries no authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki workflow and quality bar.
    - Options Considered:
      - Update once after tests pass (chosen).
      - Update per task: noisy.
    - Chosen Approach: update `docs/wiki/modules/editor-movement-selection-caret.md` (caret rendering), the editor/layout page for wrap parsing, and the shell page for pane focus, plus the index entries.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol -- documentation
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/*.md`: changed client behavior.
    - References:
      - `docs/wiki/modules/server-state-fanout.md`: the lane semantics the client now consumes.
  - Test Cases to Write:
    - Manual wiki review plus the documentation coverage test run.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
