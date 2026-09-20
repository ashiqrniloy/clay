# 143 — Editor Comment Continuation, Test-Plan Gutter Accuracy, and the Interactive Input Path

Source: `plans/129-Connection-Loop-Decomposition.md` → `## Further Actions`
(all three items, recorded 2026-09-20 from its manual pass over modules 04,
06, 07, and 10). Only the first item changes behavior; the other two remove a
false expectation and a host blocker from the manual-verification path.

No UI prototype gate applies: the continuation change inserts text the package
manifest already declares (`comments[].continuePrefix`) — no component, layout,
token, typography, or design-language change, so there is nothing to prototype
or approve. The gutter item re-scopes stale test steps to the already shipped
full-bleed editor, and the input-path item is harness/host work.

## Objectives

- C1: make the client `Enter` transform honor `editorRules.comments[].continuePrefix`
  (indent-aware) as `docs/reference/packages/creating-packages.md` and
  `docs/wiki/modules/behavior-runtime-registration.md` already promise, with
  frontend tests and no server/IPC work in the keypress path. If implementation
  is rejected at review time, the fallback objective is the same task's
  documented alternative: stop promising it in both documents.
- C2: reconcile every stale gutter expectation in `test-plan/` with the shipped
  full-bleed React editor (no line-number gutter, no fold gutter — the manifest
  toggle is accepted and ignored), and settle the orphaned
  `GUTTER_PAINT_P95_BUDGET_MS` budget plus the `docs/development/performance.md`
  row that names a test which no longer exists.
- C3: give interactive manual passes a durable, documented input path on this
  host (or an explicit recorded fallback decision), verified with
  `computer_use_linux doctor` and one live typing leg, so later plans stop
  rediscovering the portal-gnome crash loop.

## Expected Outcome

- `Enter` inside `    // text` produces `\n    // ` for a mode whose manifest
  declares `{"linePrefix":"//","continuePrefix":"// "}`; a mode with no matching
  comment rule keeps the current behavior (indent only), and the Markdown list
  `continueLineMarkers` rule still wins for list items.
- `npx vitest run src/editor` covers the new branches; no new Clay JS API, op,
  option, or configuration key is introduced.
- `test-plan/07-caret-and-typography.md` T25, `test-plan/08-syntax-and-textobjects.md`
  S21, `test-plan/13-window-splits.md` S45/S46, and the corresponding
  `test-plan/index.md` rows describe the shipped full-bleed editor; the
  `test-plan/04-core-editing.md` E4 record flips from **FAIL live** to a
  verified pass; historical dated execution records stay untouched.
- A recorded procedure with before/after `computer_use_linux doctor` evidence
  (and `ydotoold` socket state) lets a future plan type into the running app,
  or the recorded decision names the fallback and the legs it leaves
  unresolved.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --test protocol`, and the frontend editor suite are green.

## Tasks

- [ ] Baseline: reproduce the continuation gap and inventory the stale documents
  - Acceptance Criteria:
    - Functional: the gap is reproduced on the unmodified tree at both levels — a frontend unit-level reproduction (`applyEnterRule` with a Rust-shaped rule set produces indent only) and, where input is available, a live leg; the manifest data path is confirmed end to end: package `package.json` → `src/server/ops/modes.rs` (`comments` parsed at L269–L336) → bridge `EditorBehaviorRules` (`src/protocol/mod.rs:1566`) → frontend `BehaviorManifestDto.editorRules.comments` (`frontend/src/editor/extensions/types.ts:155`).
    - Performance: the record states why the fix belongs in the local keypress path (no IPC, no server round trip, no package JavaScript) and notes the current frontend editor suite runtime as the regression baseline.
    - Code Quality: every stale document is listed with its exact line and the shipped behavior that contradicts it — `docs/reference/packages/creating-packages.md:2943`, `docs/wiki/modules/behavior-runtime-registration.md:67`, `test-plan/07-caret-and-typography.md:121` (T25), `test-plan/08-syntax-and-textobjects.md:120` (S21), `test-plan/13-window-splits.md:333–334` (S45/S46), `test-plan/index.md:641/827`, `test-plan/11-performance.md:136`, `docs/development/performance.md:428` — against the full-bleed evidence (`frontend/src/editor/extensions/behavior.ts:63-64`, `frontend/src/editor/extensions/folding.ts:50-52`, `docs/wiki/modules/react-shell.md:79-80`).
    - Security: the baseline records that the continuation input is inert package-declared string data (already validated server-side), and that no step in this plan widens filesystem, network, process, or package authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`; `docs/reference/primitives/registry.md`; `docs/wiki/modules/primitive-architecture.md`; `docs/reference/packages/creating-packages.md` (Manifest-driven line-prefix transforms); `docs/wiki/modules/behavior-runtime-registration.md`; `.agents/skills/clay-execution/references/planning-checklist.md` (behavior-manifest and client-hot-path duties).
    - Options Considered:
      - Reproduce live first: strongest evidence, but the host input path is the third item of this plan. (Chosen: unit-level reproduction now, live leg in task 6.)
      - Wait for the input-path work before any reproduction: blocks the plan on host state for no benefit.
    - Chosen Approach:
      - Record the unit-level reproduction plus the data-path trace now, and re-run the live leg in task 6 after the input path is settled.
    - API Notes and Examples:
      ```bash
      npx vitest run src/editor/extensions/extensions.test.ts
      grep -n "comments" frontend/src/editor/extensions/behavior.ts frontend/src/editor/extensions/types.ts
      ```
    - Files to Create/Edit:
      - None (baseline evidence only; recorded in this task).
    - References:
      - Plan 129 task 4 record (`test-plan/index.md:825`) and `test-plan/04-core-editing.md:295` (the E4 finding this plan acts on).
  - Test Cases to Write:
    - None new; the reproduction asserts the current failing behavior before the fix.

- [ ] Review the existing enter-rule/comment primitives and fix the continuation semantics
  - Acceptance Criteria:
    - Functional: the task states, with file references, that comment continuation is an existing primitive (mode manifest `editorRules` data consumed by the client transform engine) and that no new Rust primitive, op, or protocol field is needed; it then fixes the exact semantics against the shipped contract: continuation applies when the line's text after its leading whitespace starts with a declared `linePrefix` and the rule's `continuePrefix` is non-empty; `continueLineMarkers` handling keeps precedence so list items behave exactly as today; the rule is a safe no-op when no comment rule matches (the documented "not a blind insertion of `//`"); only the main selection drives the computed suffix (today's `applyEnterRule` shape — multi-caret continuation is a further action).
    - Performance: the semantics require no scan beyond the caret line and keep the transform O(line), consistent with the existing enter-rule cost model.
    - Code Quality: the decision is recorded in this task's evidence with the options and the rejected alternatives, before any implementation file is touched; any semantic the docs do not yet state is listed for task 3's documentation update.
    - Security: the review confirms continuation inserts only manifest-declared literal text, never package JavaScript, file content, or user content from another document.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`; `docs/reference/primitives/registry.md`; `docs/wiki/modules/primitive-architecture.md`; `docs/reference/packages/creating-packages.md:2918–2950`; `docs/wiki/modules/behavior-runtime-registration.md:65–75`; `runtime/js/behavior.js:39` (`buildCodeEditingManifest` derives `continuePrefix` from `lineComment`); `docs/reference/clay-js-api/behavior/build-code-editing-manifest.md:126`.
    - Options Considered:
      - New Rust primitive for comment continuation: rejected — the manifest data and the client transform engine already exist; a server-side primitive would put keypress behavior behind IPC.
      - New manifest field (e.g. `exitOnEmptyComment`): rejected in scope — no current declaration asks for it; recorded as a further action instead.
      - Continue whenever `continuePrefix` is non-empty and the line is inside a declared comment (Chosen): faithful to the two shipped docs, adds no vocabulary.
      - Re-document instead of implement: the manifest contract, the JS facade default, and the first-party Rust/TypeScript/JavaScript/Markdown declarations all promise continuation, so the docs-only path would have to weaken four shipped declarations — kept as the explicit fallback only if review rejects the implementation.
    - Chosen Approach:
      - Reuse the existing enter-rule primitive and the already-transported `comments` array; implement the precedence/no-op/single-caret rules above and document them.
    - API Notes and Examples:
      ```json
      { "comments": [{ "linePrefix": "//", "continuePrefix": "// " }] }
      ```
    - Files to Create/Edit:
      - None in this task (decision recorded in evidence; implementation is task 3).
    - References:
      - `packages/rust/package.json` and `packages/typescript/package.json` (`continuePrefix: "// "`), `packages/markdown/package.json` (`"<!-- "`), plan 095's manifest example (`plans/095-Phase28.1-28.6-Editor-Command-and-Intelligence-Primitives.md:1358`).
  - Test Cases to Write:
    - None in this task; the semantics listed here become task 3's test cases.

- [ ] Implement indent-aware comment continuation in the client Enter transform
  - Acceptance Criteria:
    - Functional: `frontend/src/editor/extensions/behavior.ts` passes the manifest's comment rules into the Enter transform and inserts the declared `continuePrefix` after the preserved indentation; `    // note` + `Enter` → `\n    // `; a line with no matching `linePrefix`, or a mode whose manifest has no `comments`, keeps today's indent-only result; Markdown list `continueLineMarkers` behavior (including `exitOnEmptyItem`) is unchanged; the outline/document text stays the only mutation (no decoration, diagnostic, or SDUI side effect).
    - Performance: the transform stays client-local and O(caret line) — no IPC, server round trip, document serialization, or package JavaScript in the keypress path; the frontend editor suite runtime does not regress beyond noise.
    - Code Quality: `npx vitest run src/editor` passes with new cases for continuation, indentation, precedence, empty-comment body, and the no-rule no-op; the updated semantics are documented in `docs/reference/packages/creating-packages.md` (precedence, no-op rule, single-caret scope) and `docs/wiki/modules/behavior-runtime-registration.md`; `npx vitest run` and `cargo test --test protocol` stay green.
    - Security: the transform reads only the current line, the caret, and the manifest-declared rule; it introduces no new command, op, authority, or package JavaScript path, and unknown/malformed rule shapes fall back to the current behavior.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md` (Manifest-driven line-prefix transforms); `docs/wiki/modules/behavior-runtime-registration.md`; `.agents/skills/clay-execution/references/planning-checklist.md` (Client hot path; Behavior manifest); task 2's recorded semantics.
      - CodeMirror 6 keymap/dispatch contract (Context7 `/codemirror/view`, matching the pinned `@codemirror/view` 6.43.9 / `@codemirror/state` 6.7.1 in `frontend/package.json`): a `keymap.of([...])` command returns a boolean for handled/not-handled, and edits go through `view.dispatch({ changes, selection })` with `state.doc.lineAt` / `state.sliceDoc` for the caret line.
    - Options Considered:
      - Extend `applyEnterRule` with an optional third argument (Chosen): smallest diff, keeps the exported test seam, and preserves the existing `(view, raw)` call shape for other callers/tests.
      - Move continuation into a new extension with its own keymap entry: two Enter handlers racing on the same key, rejected.
      - Compute continuation in the Rust client instead of the frontend: rejected — the editor text and caret are client-local by contract (client-first leased edits, no server round trip before local paint).
    - Chosen Approach:
      - Add the comment branch inside the existing Enter transform, after the marker branch, using the rules the manifest already provides; add the missing frontend tests and align the two documentation pages with the implemented semantics.
    - API Notes and Examples:
      ```ts
      // @codemirror/view — handler returns true when it consumed the key
      { key: "Enter", run: (view) => applyEnterRule(view, rules.enter, rules.comments) }

      // @codemirror/state + /view — the edit shape the branch must keep using
      const line = view.state.doc.lineAt(view.state.selection.main.head)
      view.dispatch({ changes: { from: caret, insert: `\n${suffix}` }, selection: { anchor: caret + 1 + suffix.length } })
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/extensions/behavior.ts`: comment-continuation branch + rule plumbing.
      - `frontend/src/editor/extensions/extensions.test.ts`: continuation/precedence/no-op cases (existing enter test at L143 is the template).
      - `frontend/src/editor/extensions/types.ts`: only if a narrower type removes an `unknown` cast — no shape change expected.
      - `docs/reference/packages/creating-packages.md`: exact semantics (precedence, empty-comment body, single-caret scope).
      - `docs/wiki/modules/behavior-runtime-registration.md`: the same semantics for the runtime-registration reader.
    - References:
      - `frontend/src/editor/extensions/controller.ts:723` (existing `linePrefix` consumer for toggle-comment); `frontend/src/editor/extensions/keymaps.ts` (`insertAtSelections`).
  - Test Cases to Write:
    - `applyEnterRule` with a Rust-shaped `comments` rule: indented `// note` continues as `\n    // ` (exact document/caret assertion).
    - Markdown-shaped rules: `- item` + Enter still produces the list marker, not the comment prefix (precedence).
    - Empty comment body (`//` alone) follows the recorded semantics (task 2) rather than inventing an exit rule.
    - No matching `linePrefix` (e.g. `let x = 1;`) and no `comments` array: indent-only result, identical to today.
    - Negative: an unknown/malformed rule object does not throw and does not insert a prefix.

- [ ] Reconcile the stale gutter expectations and the dormant gutter budget with the shipped full-bleed editor
  - Acceptance Criteria:
    - Functional: `test-plan/07-caret-and-typography.md` T25, `test-plan/08-syntax-and-textobjects.md` S21, and `test-plan/13-window-splits.md` S45/S46 are re-scoped to what ships (no line-number gutter; no fold gutter; active-line wash, indent guides, and bracket-match highlight remain) and point at the design source (`docs/wiki/modules/react-shell.md:79-80`) instead of restating a gutter that does not exist; the `test-plan/index.md` rows that repeat the stale claim are corrected; the dated execution records that reported the Phase 26 (retired client) gutter are left as history, marked as belonging to the retired client rather than rewritten.
    - Performance: `GUTTER_PAINT_P95_BUDGET_MS` (`src/perf/budgets.rs:300`) and the `docs/development/performance.md:428` row are settled explicitly — either deleted with the other unused chrome budgets (`ACTIVE_LINE_PAINT_*`, `BRACKET_MATCH_PAINT_*`, `DECORATION_BACKGROUND_FILL_*` are also unreferenced) or kept and marked dormant with the reason — and the docs row stops naming `phase26_7_chrome_paint_budgets_fit_inside_keypress_envelope`, which does not exist in the current test tree.
    - Code Quality: no step is deleted or weakened; each re-scope keeps the checkable clause (what should be visible) and records the maintainer decision that the shipped editor has no gutter; `test-plan/index.md` module map and coverage-matrix rows stay consistent with the module files.
    - Security: no security-relevant step changes; the re-scope adds the negative check that no `.cm-gutters` element is painted (the same assertion shape as `frontend/src/sdui/surface-adoption.test.tsx:248`), so a future regression toward a gutter fails visibly.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map, host ceilings); `test-plan/07-caret-and-typography.md`; `test-plan/08-syntax-and-textobjects.md`; `test-plan/13-window-splits.md`; `test-plan/11-performance.md`; `docs/wiki/modules/react-shell.md`; `docs/development/performance.md`; `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Options Considered:
      - Rewrite T25/S45/S46 to assert the full-bleed behavior (Chosen): keeps the steps meaningful and testable.
      - Delete the gutter clauses: loses the check entirely, and the historical records then dangle.
      - Reintroduce a gutter to match the old steps: rejected — the full-bleed editor is the documented design (`docs/wiki/modules/react-shell.md`), and the manifest toggle is documented as accepted-and-ignored.
    - Chosen Approach:
      - Re-scope the steps to the shipped design, add the negative `.cm-gutters` check, and settle the orphaned budget and the stale docs row in the same pass because they are the same claim.
    - API Notes and Examples:
      ```bash
      grep -rn "cm-gutters" frontend/src/editor/extensions/folding.ts frontend/src/sdui/surface-adoption.test.tsx
      ```
    - Files to Create/Edit:
      - `test-plan/07-caret-and-typography.md`, `test-plan/08-syntax-and-textobjects.md`, `test-plan/13-window-splits.md`, `test-plan/index.md`, `test-plan/11-performance.md`.
      - `src/perf/budgets.rs` and `docs/development/performance.md` (budget decision only).
    - References:
      - `frontend/src/editor/extensions/behavior.ts:63-64`; `frontend/src/editor/extensions/folding.ts:50-52`; `frontend/src/editor/editor.module.css:151`; plan 129 task 4's stale-expectation note (`test-plan/07-caret-and-typography.md:170`).
  - Test Cases to Write:
    - Manual/checkable step text: code pane shows no gutter numbers and no fold chevrons; prose pane unchanged; both remain checkable on the next module run.
    - Assertion reused by later live runs: no `.cm-gutters` node in the editor DOM/AT-SPI tree.

- [ ] Establish and document a durable local input path for interactive manual passes
  - Acceptance Criteria:
    - Functional: one recorded procedure produces working keyboard input for a live Clay build on this host, verified end to end (a typed literal appears in the document text and the AT-SPI probe reads it back). Candidate paths, in order: `ydotoold` with a connectable socket (`/dev/uinput` needs a root-owned udev rule or equivalent grant — explicit user approval required before any root change); the existing xdg-desktop-portal RemoteDesktop keyboard session used in short bursts with a fresh session per batch; AT-SPI named-node actions for steps that do not need typing.
    - Performance: the procedure records the practical limits (how many keystrokes land per session; the observed `xdg-desktop-portal-gnome` crash/restart before and after) so a later plan can size a manual pass instead of rediscovering it.
    - Code Quality: the procedure lands in the repo's harness documentation (`docs/development/launch-and-gui-smoke.md` host-input section, `test-plan/index.md` host note) with before/after `computer_use_linux doctor` output and the socket/service state; if the root step is declined, the fallback and its unresolved legs are recorded explicitly rather than claimed as passes.
    - Security: the procedure states what each backend can do (input synthesis only), that no repo file grants new authority, that a udev/uinput change is a host-level change requiring user approval, and that no step types secrets into the app.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md` (harness and readiness flags); `test-plan/index.md` host notes; `test-plan/artifacts/129-connection-loop/README.md` (portal crash record); `test-plan/artifacts/126-access-paths/` (`launch-live.sh`, `probe.py`, `portal-shot.py`, and the live-large records for the portal path that worked) and `test-plan/artifacts/127-lane-scheduling/README.md` (same portal path, `Ctrl+Space` ceiling).
    - Options Considered:
      - `ydotoold` + udev grant (Chosen first attempt): doctor currently reports `can_send_development_input=false` because `/dev/uinput` is root-only (`Permission denied (os error 13)`) and there is no socket at `/run/user/1000/.ydotool_socket` or `/tmp/.ydotool_socket`; `ydotool` itself is installed and detected.
      - Portal RemoteDesktop sessions (current working path): keystrokes land in short bursts, then `xdg-desktop-portal-gnome` core-dumps (`journalctl --user`: `Failed with result 'core-dump'`) and the in-flight key is dropped.
      - `wtype`: installed but unusable (GNOME 50 exposes no virtual-keyboard protocol).
      - AT-SPI actions only: works for named nodes, cannot type into the WebKit editor node (`supports_editable_text=false`).
    - Chosen Approach:
      - Try the daemon path with explicit user approval; keep the portal path documented as the fallback with its burst limits; never claim a typing leg that did not land.
    - API Notes and Examples:
      ```bash
      computer_use_linux doctor            # expect can_send_development_input: true after the grant
      ls -l /dev/uinput /run/user/$(id -u)/.ydotool_socket
      ```
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md` (host-input procedure and readiness flags).
      - `test-plan/index.md` (host note: what works, what is unresolved, and the recovery rule).
      - A per-run artifacts README under `test-plan/artifacts/143-manual-follow-ups/` for the doctor output and the typing leg.
    - References:
      - Plan 129's host-ceiling record (`test-plan/artifacts/129-connection-loop/README.md`); `test-plan/index.md:396–402` (portal path works) and `test-plan/index.md:241` (`can_send_development_input=false`).
  - Test Cases to Write:
    - Live check: `type_text "cont-probe"` into the editor followed by the AT-SPI `text` probe reading the literal back.
    - Negative: a step whose keystroke is dropped is recorded `UNRESOLVED` with the doctor/socket state, never inferred from the config alone.

- [ ] Execute and update the manual test plan (test-plan/) and the visual/accessibility review
  - Acceptance Criteria:
    - Functional: `test-plan/04-core-editing.md` E4 is re-run on a real Linux build (`@clay/rust` fixture) and passes — `Enter` at the end of `// caret-here comment line` yields `\n    // ` — and the recorded outcome replaces the current **FAIL live** row in both the module file and `test-plan/index.md`; the re-scoped `07` T25 step is executed against the shipped full-bleed editor (no gutter painted) with its result recorded; new numbered steps are added only for genuinely new user-visible behavior (the re-scope changes step wording, not IDs).
    - Performance: the record states the client-local nature of the change (no measurable IPC/server leg) and does not invent a latency number; if a keypress→paint number is claimed it comes from the perf harness, not the AT-SPI walk.
    - Code Quality: artifacts land under `test-plan/artifacts/143-manual-follow-ups/` (screenshots, AT-SPI text captures, logs) and are referenced from the module records; `test-plan/index.md` module map/records stay consistent.
    - Security: no existing step is weakened or deleted; the negative checks stay (plain-text mode performs no continuation; the editor keeps its focus/selection behavior).
    - Note: this plan changes inserted text, not chrome/layout/tokens, so the UI prototype gate does not apply; the visual/accessibility review duty for the change is performed inside this task (per-state screenshots of the continued comment line, AT-SPI focus/role check on the editor, and a keyboard-only pass over the continuation flow).
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`; `test-plan/04-core-editing.md`; `test-plan/07-caret-and-typography.md`; `.agents/skills/clay-execution/references/ui.md` and `.agents/skills/clay-execution/references/docs-as-code.md`; task 5's recorded input procedure.
      - UI gate for the review task: `DESIGN.md` (normative Quiet Instrument language, §14 retired patterns), `.agents/skills/clay-execution/references/components.md` and `.agents/skills/clay-execution/references/tokens.md` — the continuation change introduces no component, token, or layout surface, so the review confirms (rather than re-designs) that the inserted text uses the existing editor styling and adds no retired pattern.
    - Options Considered:
      - Re-run only E4 and T25 (Chosen): the other module steps were freshly verified by plan 129 and are unaffected by this change.
      - Re-run all four plan-129 modules again: duplicates work with no new signal.
    - Chosen Approach:
      - Drive E4 and the re-scoped T25 on an isolated (`/tmp`, mode-700) build using plan 129's harness (`test-plan/artifacts/129-connection-loop/run-live.sh`), capture screenshots plus AT-SPI text, and update the module records and `test-plan/index.md`.
    - API Notes and Examples:
      ```bash
      test-plan/artifacts/129-connection-loop/run-live.sh start editing
      python3 test-plan/artifacts/129-connection-loop/probe.py text
      ```
    - Files to Create/Edit:
      - `test-plan/04-core-editing.md`, `test-plan/07-caret-and-typography.md`, `test-plan/index.md`, `test-plan/artifacts/143-manual-follow-ups/`.
    - References:
      - Plan 129 task 4 records for the harness and evidence style; `.agents/skills/clay-execution/references/planning-checklist.md` (Visual and Accessibility Review Duty).
  - Test Cases to Write:
    - Manual steps: E4 continuation (positive) and plain-text no-continuation (negative); T25 full-bleed chrome check; keyboard-only flow from editor focus to the continued line.

- [ ] Create or verify Clay JS APIs and configuration coverage for comment continuation
  - Acceptance Criteria:
    - Functional: verify (expected) that no public programmatic surface is added — `comments[].linePrefix`/`continuePrefix` already ship as the `behavior.buildCodeEditingManifest` contract (`docs/reference/clay-js-api/behavior/build-code-editing-manifest.md`, `runtime/js/behavior.js:39`), and the client consuming `continuePrefix` implements an existing documented API instead of creating one; if any surface does change, it is implemented as a real Clay JS API with dotted ID, facade, Markdown doc, registry entry, and coverage tests.
    - Performance: none (manifest data is parsed at load; the transform stays client-local).
    - Code Quality: `cargo test --test protocol` passes in full, including `clay_js_api_inventory`, `clay_js_doc_registry` (generated registry current), `clay_js_facade_layout`, and `documentation_coverage`; the two updated docs stay the single source for the semantics.
    - Security: the verified API keeps its existing inert-data contract (no package JavaScript, no filesystem/network/process authority); the plan introduces no configuration key, so no trust-boundary declaration changes.
    - Note: the example-configuration duty does not apply — `continuePrefix` lives in package/mode manifests, not `~/.clay/init.js` (`examples/config/init.js` carries no `editorRules.comments` section), so updating it would document a surface the config file does not own. Recorded here instead of silently skipping.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`; `docs/reference/clay-js-api/api-inventory.toml` (`behavior.buildCodeEditingManifest`); `docs/reference/clay-js-api/behavior/build-code-editing-manifest.md`; `.agents/skills/clay-execution/references/config.md`.
    - Options Considered:
      - Verify-only (Chosen): the API exists and is documented; this plan changes the consumer.
      - Promote a new "comment continuation" API: rejected — it would duplicate the manifest contract.
    - Chosen Approach:
      - Verify the registry/doc guards, confirm the JS facade derives `continuePrefix` from `lineComment`, and record the example-config exemption with its reason.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js
      ```
    - Files to Create/Edit:
      - None expected (verification-only); `docs/reference/clay-js-api/behavior/build-code-editing-manifest.md` only if the wording needs to state that the client now honors it.
    - References:
      - Plan 142's verify-only JS API task (same pattern) and plan 136's option-surface drift guard (context for keeping the facade honest).
  - Test Cases to Write:
    - None beyond the existing registry/doc guards.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: `docs/wiki/modules/behavior-runtime-registration.md` documents the implemented continuation semantics (precedence after list markers, the no-rule no-op, the single-caret scope, and the test paths that guard them); `docs/wiki/modules/ui-review-harness.md` documents the settled input path so future passes do not rediscover it; `docs/wiki/index.md` navigation stays accurate; a `tests/documentation_coverage.rs` guard pins the continuation markers so the page cannot drift back to the pre-fix claim.
    - Performance: the pages state that the transform is client-local and O(caret line), with no IPC in the keypress path.
    - Code Quality: pages explain what changed, the invariants/tradeoffs, and the exact source/test paths (following the plan 127/129 wiki-guard precedent); no archive page is created for this work.
    - Security: the pages restate that continuation inserts only manifest-declared literal text and that the manifest is validated server-side; the input-path section documents host-level access without publishing host secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md` (wiki workflow, quality bar, guard precedents); `docs/wiki/modules/react-shell.md` (full-bleed design source); `docs/wiki/modules/ui-review-harness.md`.
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass, with one deterministic guard (Chosen): matches the plan 125/127/129 pattern.
    - Chosen Approach:
      - One pass across the three pages plus the index, then add the guard test asserting the continuation semantics markers and the input-path note.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol documentation_coverage
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/behavior-runtime-registration.md`, `docs/wiki/modules/ui-review-harness.md`, `docs/wiki/index.md`, `tests/documentation_coverage.rs`.
    - References:
      - `tests/documentation_coverage.rs::plan129_wiki_pages_describe_the_dispatch_router_and_future_sizes` (guard style and marker granularity).
  - Test Cases to Write:
    - New guard test: the runtime-registration page names the continuation precedence, the no-rule no-op, and the client-local invariant; the harness page records the settled input path; the index links both.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.

Known items deliberately not scheduled here (trigger conditions instead):

- Multi-caret comment continuation: today's transform computes one suffix for the main selection. Extend to every selection only if a user workflow hits it; the plan's tests pin the current scope so a change is deliberate.
- Exit-on-empty-comment semantics: VS Code-style "stop commenting on an empty comment line" would need a new manifest field (e.g. `exitOnEmptyComment`); add it only when a package asks, as a real manifest/Clay JS API change.
- Typed-prefix continuation (e.g. `///` doc comments, `<!--` inside Markdown prose): wait until a shipped first-party manifest declares the variant rather than guessing the rule shape.
- Reintroducing a gutter: rejected here because the full-bleed editor is the documented design (`docs/wiki/modules/react-shell.md`); a future chrome decision would go through the UI prototype + explicit approval loop, not a test-plan edit.
