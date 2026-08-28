# Client-Local Parsing Fail-Fast Spike and Parser-Placement Decision

## Objectives

- Run one reversible, production-representative CodeMirror/Lezer fail-fast spike on `spike/client-local-parsing` before implementing unchecked work from Plan 099.
- Test only the risks that can disqualify stock Lezer quickly: current-language grammar parity, WebKitGTK main-thread latency, distant viewport freshness, 10-50 MiB document behavior, memory, and one/four-pane scaling.
- Stop Lezer work on the first hard-gate failure instead of building complete parity, a frontend-worker Tree-sitter path, a hybrid path, or speculative package APIs.
- If Lezer fails, confirm the approved server-side per-document Tree-sitter-session direction and replan Plan 099 around the evidence. Evaluate frontend-worker Tree-sitter only if the completed server overhaul later misses its metric gates.
- If Lezer passes every fail-fast gate, publish the evidence and obtain a new explicit parser-placement decision before extending the prototype.

## Expected Outcome

- A small disposable Lezer prototype and dated report provide enough real Tauri/WebKitGTK evidence to accept or reject stock client-local parsing without implementing a complete alternative editor architecture.
- Any hard-gate failure ends the Lezer candidate and leaves no requirement to finish folds, text objects, semantic composition, adopted-package execution, or all-language visual polish for a rejected engine.
- The likely outcome is confirmation of server-native Tree-sitter sessions with atomic viewport patches, while keeping CodeMirror local text, viewport, incremental position indexing, and inert projection ownership.
- Plan 099 stays paused only until this fail-fast spike and resulting parser-placement approval complete. It is then updated in place and resumed, rather than replaced solely because this spike existed.
- No frontend-worker parser, hybrid dual-engine design, public Clay JS API, `init.js` option, package permission, production dependency, or new client package trust domain is adopted by this plan.

## Authority and Spike Boundaries

- Server remains canonical owner of document ropes, versions, edit ordering, leases, persistence, workspaces, package installation/provenance, semantic/LSP analysis, and external processes.
- CodeMirror remains local owner of optimistic text, selection, history, viewport, and presentation. Lezer becomes a branch-only experimental base-syntax owner for measured runs only.
- Exactly one base syntax owner is active in a run. Server semantic, diagnostic, search, link, and inlay overlays remain separately identified inert layers.
- Only exact bundled first-party Lezer language modules may execute in the main webview. Adopted-package JavaScript, arbitrary module URLs, package CSS, WASM, raw Tauri access, filesystem, network, shell, package-control, AI, and external-process authority remain denied.
- Raw measurements live under `target/perf/client-local-parsing/`; committed evidence uses generated fixtures and sanitized paths only.
- Branch-only dependencies, switches, instrumentation, and parser adapters are disposable. Plan 099 must deliberately reimplement any winning idea rather than merge spike code wholesale.
- Frontend-worker Tree-sitter and hybrid parsing are out of scope. They reopen only after a completed server-session implementation still misses approved metrics.

## Fail-Fast Decision Gates

Evaluate gates in this order and stop on the first failure:

1. **Grammar freshness:** Current Clay fixtures parse as expected, and a frozen modern-syntax corpus covers Rust async closures, let chains, gen blocks/reserved syntax, plus TypeScript decorators and import attributes. A recovery node that changes tokens, folds, or structural selection is a failure unless an already-released exact parser version fixes it.
2. **Local editing:** On the designated minimum Linux device, keystroke-to-CodeMirror-update p95 is at most 8 ms and maximum at most 16 ms. Parser cost is separately attributed.
3. **Main thread:** Zero tasks exceed 50 ms during five-second typing, distant-jump, and fling-scroll traces. Stock `@codemirror/language` scheduling must be measured rather than assumed safe.
4. **Viewport freshness:** Requested viewport syntax becomes current within 100 ms up to 1 MiB and 200 ms for 10-50 MiB files, including jumps near the middle/end before background parsing catches up. Plain text while pending is allowed; stale or indefinitely partial syntax is not.
5. **Memory:** One-pane and four-pane 50 MiB runs remain inside Clay's 256 MiB large-file resident-memory envelope. Same-document panes must not create unacceptable per-pane parser-tree multiplication.
6. **Reproducibility:** Three stable runs on the designated minimum Linux device, or explicit user approval of a documented proxy when that device is unavailable.

Advisory pre-screen evidence from 2026-08-26 does not replace these gates:

- Exact installed `@codemirror/language 6.12.4` allows 20 ms synchronous parse work during state updates, background slices up to 100 ms, 3,000 ms work per 30-second chunk, and 100,000-character parse-ahead.
- Latest isolated Node probes parsed current Clay fixtures without error nodes but found modern Rust/TypeScript recovery-node gaps and document-shape-sensitive 10 MiB latency/memory risk.
- Direct full-parser probes are not product benchmarks because CodeMirror normally keeps trees partial and schedules work incrementally. Real WebKitGTK traces decide.

## Tasks

- [x] Pause Plan 099 and narrow Plan 100 to the approved Lezer fail-fast scope
  - Acceptance Criteria:
    - Functional: Branch is `spike/client-local-parsing`; Plan 099 is paused; Plan 100 removes required frontend-worker, hybrid, full-parity, and exhaustive all-candidate implementation.
    - Performance: No production parser or performance implementation occurs during plan narrowing.
    - Code Quality: Plan records ordered stop conditions, conditional follow-up, and in-place replanning of Plan 099.
    - Security: Scope change grants no package, browser, worker, Tauri, filesystem, network, shell, or public API authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/SKILL.md`
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
      - `plans/099-Clay-Editor-Performance-Overhaul.md`
      - `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
      - `decision-logs/2026-08-26-1857-spike-parser-placement-before-performance-implementation.md`
    - Options Considered:
      - Execute original five-candidate Plan 100: rejected because Lezer has cheap disqualifying tests and worker/hybrid work is speculative.
      - Skip the spike and resume server implementation immediately: rejected because one bounded real WebKitGTK check resolves the remaining uncertainty.
      - Run a short Lezer fail-fast gate, then replan Plan 099: chosen.
    - Chosen Approach:
      - Keep only work needed to falsify stock Lezer. Stop after any hard failure; do not make a rejected candidate feature-complete.
    - API Notes and Examples:
      ```bash
      git branch --show-current
      # spike/client-local-parsing
      ```
    - Files to Create/Edit:
      - `plans/100-Client-Local-Parsing-Spike-and-Parser-Placement-Decision.md`: Replace exhaustive matrix with ordered fail-fast work.
      - `plans/099-Clay-Editor-Performance-Overhaul.md`: Align pause/resume wording.
      - `decision-logs/2026-08-26-2137-lezer-fail-fast-before-server-syntax-overhaul.md`: Record approved narrowed workflow.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Record durable fail-fast/reopen rule.
    - References:
      - [Lezer system guide](https://lezer.codemirror.net/docs/guide/)
      - [CodeMirror language reference](https://codemirror.net/docs/ref/#language)
      - [Tree-sitter parser guide](https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html)
  - Test Cases to Write:
    - Plan structure review: Confirm no task requires a worker, hybrid parser, full Lezer migration, or successor plan.
    - Cross-reference review: Confirm Plans 099/100 and the approved decision logs describe the same work order.
  - Completion Evidence:
    - User explicitly approved continuing Plan 100 as a narrowed Lezer fail-fast gate, stopping on hard failure, deferring worker Tree-sitter until after measured server implementation, and replanning Plan 099 afterward.
    - Original required worker/hybrid/full-candidate tasks were removed rather than retained as speculative optional work.
    - Approved decision recorded in `decision-logs/2026-08-26-2137-lezer-fail-fast-before-server-syntax-overhaul.md`.

- [x] Build the smallest production-representative Lezer prototype and measurement harness
  - Acceptance Criteria:
    - Functional: A development-only selector installs exact bundled Rust, JavaScript, TypeScript, TSX, or Markdown `LanguageSupport` through `languageCompartment`; default/release behavior remains server syntax.
    - Performance: Instrument CodeMirror transaction duration, parser advancement, viewport change/current syntax, long tasks, frame-adjacent work, JS heap, process RSS, and pane count without flattening documents or recording source.
    - Code Quality: One generic mode registry, one Clay tag-to-theme adapter, one bounded recorder, and one runner are sufficient. Do not implement text objects, worker protocols, hybrid ownership, or unrelated Plan 099 fixes.
    - Security: Selector is branch/development-only; modules come from exact lockfile dependencies; no adopted package can select modules or execute in editor paths.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - Context7 `/websites/codemirror_net`: `LanguageSupport`, `Compartment`, `syntaxTreeAvailable`, highlighting, folding, and parse scheduling.
      - Context7 `/codemirror/lang-javascript`: JavaScript/TypeScript/JSX/TSX dialect configuration.
      - `frontend/node_modules/@codemirror/language/dist/index.js`: Exact 6.12.4 scheduler behavior.
      - `docs/reference/primitives/syntax-vocabulary.md`: Clay theme vocabulary authority.
    - Options Considered:
      - Implement full local structural parity first: rejected because performance or grammar gates may discard all work.
      - Benchmark parser functions outside CodeMirror only: rejected because it omits WebKit scheduling and editor integration.
      - Install native CodeMirror language support with minimal theme mapping and instrumentation: chosen.
    - Chosen Approach:
      - Lazy-load exact official language packages, map Lezer highlight tags to existing Clay CSS variables, and expose no product setting. The selector is accepted only in Vite development or the exact `client-local-parsing-spike` build mode. Preserve one active base owner per measured run.
    - API Notes and Examples:
      ```ts
      const support = await loadLezerLanguage("rust");
      view.dispatch({
        effects: languageCompartment.reconfigure([
          support.extension,
          clayLezerHighlighting,
        ]),
      });
      ```
    - Files to Create/Edit:
      - `frontend/package.json`, `frontend/package-lock.json`: Exact branch-only language dependencies.
      - `frontend/src/editor/spike/lezer-languages.ts`: Lazy mode registry.
      - `frontend/src/editor/spike/clay-highlighter.ts`: Closed Lezer-tag mapping.
      - `frontend/src/editor/spike/performance.ts`: Content-free bounded metrics.
      - `frontend/src/editor/spike/mode.ts`: Development-only selector.
      - `frontend/src/editor/create-editor.ts`: Explicit spike extension installation.
      - `frontend/src/editor/extensions/decorations.ts`: Development-only base-syntax ownership filter while retaining semantic and other inert layers.
      - `frontend/src/editor/spike/*.test.ts`: Registry, ownership, theme, and recorder checks.
      - `scripts/client-local-parsing-spike.sh`: Reproducible real-app runner.
      - `docs/wiki/index.md`, `docs/wiki/modules/client-local-parsing-spike.md`: Code-wiki coverage for the disposable harness and authority boundary.
      - `target/perf/client-local-parsing/**`: Uncommitted raw results.
    - References:
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
  - Test Cases to Write:
    - Default-off test: Production/default launch cannot select local parsing.
    - Single-owner test: Local run suppresses server base syntax but retains separately identified semantic/diagnostic layers.
    - Lazy-load test: Plain text and one selected language load no unrelated parser module.
    - Theme test: Theme changes update styles without replacing document/history or reparsing solely for color.
    - Recorder privacy test: Metrics remain bounded and contain no source sentinel or absolute path.
  - Completion Evidence:
    - Added exact lockfile-pinned `@codemirror/lang-rust`, `@codemirror/lang-javascript`, `@codemirror/lang-markdown`, and `@lezer/highlight` dependencies.
    - Added one lazy language registry for Rust, JavaScript, TypeScript, TSX, and Markdown; `createEditor` installs it only for development/spike builds through `languageCompartment`.
    - Added the closed Lezer-tag to Clay-token adapter and a base-owner filter that suppresses only server `syntax` decorations during local runs; semantic, diagnostic, link, search, and inlay layers remain separate.
    - Added a numeric-only recorder capped at 4096 samples for transactions, parser advancement/current viewport syntax, viewport changes, long tasks, frame-adjacent intervals, JS heap, externally sampled process RSS, and pane count. It is exposed only through the development `window.__clayClientPerformance` handle.
    - Added `scripts/client-local-parsing-spike.sh`, which builds the exact spike mode, launches the real desktop smoke path, samples the complete process tree RSS, and stores raw output under `target/perf/client-local-parsing/`.
    - `npm test --prefix frontend`, `npm run typecheck --prefix frontend`, `npm run lint --prefix frontend`, `npm run format:check --prefix frontend`, production and spike-mode frontend builds, `bash -n scripts/client-local-parsing-spike.sh`, and Linux `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets` pass.
    - A one-second harness smoke with the Rust selector launched and stopped the real desktop without leaving Clay processes, recording external process RSS under `target/perf/client-local-parsing/`. Timed shutdown intentionally produces no frontend unload snapshot; a normal window close captures `CLAY_PERF_SNAPSHOT`. It was a lifecycle check, not a fail-fast gate run.
    - The existing frontend bundle budget command still reports its pre-existing 266.1 KiB shell / 454.9 KiB total gzip overages; no Plan 099 bundle work was added here.

- [x] Run the ordered Lezer fail-fast grammar, latency, viewport, and memory gates (stopped at grammar gate)
  - Acceptance Criteria:
    - Functional: Current fixtures plus frozen modern Rust/TypeScript corpus run first; subsequent gates execute only while all prior gates pass. First failure is preserved with exact version, fixture, device, trace, and reproduction command.
    - Performance: Measure 1, 10, and 50 MiB mixed-Unicode, many-short-line, long-line, newline-heavy, dense-code, and dense-Markdown shapes; type top/middle/end; jump before parse catch-up; fling scroll; compare one pane and four same-document panes.
    - Code Quality: One machine-readable result schema reports pass/fail without hand-edited values. A failed candidate stops, leaving unexecuted scenarios explicitly marked not applicable.
    - Security: Generated fixtures stay under approved target roots; traces contain no source, ambient path, credential, or home-directory data.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/impeccable/reference/optimize.md`
      - `docs/development/editor-performance-review-2026-08-26.md`
      - `docs/wiki/modules/performance-fixtures.md`
      - `src/perf/budgets.rs`
      - `scripts/large-document-smoke.sh`
    - Options Considered:
      - Run full matrix before checking grammar: rejected because known grammar gaps may end the candidate immediately.
      - Use Node/jsdom results as acceptance evidence: rejected because WebKitGTK scheduling and process memory are decisive.
      - Run ordered real-app gates and stop on first failure: chosen.
    - Chosen Approach:
      - Execute grammar, local-edit, long-task, viewport-freshness, then memory/pane gates. Record three stable minimum-device runs only for scenarios reached before failure.
    - API Notes and Examples:
      ```bash
      scripts/client-local-parsing-spike.sh --candidate lezer --runs 3
      ```
    - Files to Create/Edit:
      - `tools/spikes/client-local-parsing/criteria.json`: Frozen thresholds and fixture identities.
      - `tools/spikes/client-local-parsing/fixtures/**`: Small grammar/parity sources.
      - `tools/spikes/client-local-parsing/summarize.mjs`: Ordered gate validation.
      - `target/perf/client-local-parsing/grammar.json`, `target/perf/client-local-parsing/summary.json`: Raw machine-readable gate traces and summary.
      - `docs/development/client-local-parsing-spike-2026-08-26.md`: Method and reached results.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Grammar freshness test: Modern valid syntax produces required token/fold structure without material recovery-node divergence.
    - 50 MiB edit test: Parser adds no document-sized synchronous editor transaction work (not reached after grammar failure).
    - Distant-jump test: Middle/end viewport becomes current inside 200 ms without parsing-induced long tasks (not reached after grammar failure).
    - Four-pane test: Same-document memory/work remains inside the large-file envelope (not reached after grammar failure).
    - Stop-rule test: One hard failure prevents later expensive scenarios and reports why they were skipped.
  - Completion Evidence:
    - `node tools/spikes/client-local-parsing/summarize.mjs --run` verified exact lockfile versions and parsed all five current syntax fixtures with zero recovery nodes and required structure markers.
    - The first frozen modern probe, `tools/spikes/client-local-parsing/fixtures/rust-modern.rs`, failed the zero-recovery-node grammar gate with `@lezer/rust` 1.0.2: four recovery nodes at numeric parser offsets `257`, `326–329`, and `467` twice. The probe covers an async closure, let-chain, and gen block.
    - The frozen modern TypeScript probe also records two recovery nodes at offsets `184` and `189` with `@codemirror/lang-javascript` 6.2.5, covering import attributes alongside decorators and `satisfies` syntax.
    - `target/perf/client-local-parsing/grammar.json` preserves dependency versions, fixture IDs, parser versions, node markers, numeric recovery offsets, and pass/fail values. `target/perf/client-local-parsing/summary.json` preserves the first failure, marks edit latency, main-thread long tasks, viewport freshness, memory envelope, four-pane scaling, and reproducibility `not-applicable`, and marks all 90 planned size/shape/pane scenarios `not-applicable` under the ordered stop rule.
    - No WebKitGTK latency, 1/10/50 MiB viewport, memory, or pane scenario was run after the hard grammar failure. Full result and security methodology are recorded in `docs/development/client-local-parsing-spike-2026-08-26.md`.

- [x] If Lezer passes every fail-fast gate, complete only the minimum structural and authority parity needed for a decision
  - Acceptance Criteria:
    - Functional: This task runs only after every prior hard gate passes. It verifies highlighting vocabulary, Markdown fences, indentation, folds, brackets/comments, current text objects, theme switching, semantic overlays, reload/resync, and explicit server fallback. If a prior gate fails, completion records this task as not applicable without implementation.
    - Performance: Added parity behavior preserves passed thresholds and causes zero theme-only parser runs, zero React commits per normal edit, and no full-document conversion.
    - Code Quality: Missing behavior uses existing CodeMirror APIs or a small generic tree traversal. No custom query language, worker architecture, or public extension platform is introduced.
    - Security: Only bundled modules execute locally; adopted/server-only packages retain server fallback and never create simultaneous base owners.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/wiki/modules/first-party-language-packages.md`
      - `docs/wiki/modules/first-party-lsp-bridge-packages.md`
      - `packages/{rust,typescript,javascript,markdown}/queries/*.scm`
    - Options Considered:
      - Build parity regardless of gate failures: rejected as throwaway work.
      - Invent Lezer equivalents for every Tree-sitter query: rejected unless a passing engine makes specific parity necessary.
      - Add only decision-critical parity after all gates pass: chosen.
    - Chosen Approach:
      - Preserve one base owner and reuse CodeMirror language services. Any substantial grammar fork or generic query framework becomes a documented rejection reason, not spike implementation.
    - API Notes and Examples:
      ```ts
      const treeReady = syntaxTreeAvailable(state, viewport.to);
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/spike/lezer-languages.ts`: Decision-critical language configuration.
      - `frontend/src/editor/spike/local-structure.ts`: Minimal generic structural adapter only if required.
      - `frontend/src/editor/spike/syntax-owner.ts`: Explicit local/server fallback.
      - `frontend/src/editor/spike/*.test.ts`: Reached parity checks.
      - `docs/development/client-local-parsing-spike-2026-08-26.md`: Parity result or not-applicable evidence.
    - References:
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/language-capability-sequencing.md`
  - Test Cases to Write:
    - Structural parity test: Reached modes match frozen fold/indent/bracket/comment/text-object expectations.
    - Overlay test: Current semantic/diagnostic data composes without deleting local base syntax.
    - Fallback test: Unsupported/adopted package activates exactly one server base owner.
    - Reload test: Generation/version changes cannot leave mixed old/new parser output.
  - Completion Evidence:
    - Not applicable: the preceding grammar gate failed on frozen modern Rust (`rust-modern`, `@lezer/rust` 1.0.2, four recovery nodes), so this task's implementation condition was not met.
    - No structural or authority parity code, tests, or public surface were added. Existing prototype fallback remains the only reached behavior.
    - `target/perf/client-local-parsing/summary.json` records the grammar first failure and marks all later gates and 90 planned scenarios `not-applicable`.

- [x] Perform bounded visual screenshot and accessibility review of reached Lezer states
  - Acceptance Criteria:
    - Functional: Real Linux Tauri review captures baseline and reached Lezer states for small/large files, malformed/modern syntax, distant-jump pending/current syntax, one/four panes, and light/dark/custom themes. Failed gates require only evidence of the failure and editable plain-text fallback, not polished rejected states.
    - Performance: Screenshots link to trace IDs; review rejects blank text, stale-color flash, stuck syntax, focus/selection loss, scroll hitch, or theme-triggered parse churn.
    - Code Quality: One inspection pass, one batched correction pass when the candidate remains viable, and at most one confirmation pass.
    - Security: Synthetic fixtures and sanitized chrome only; screenshots/accessibility dumps contain no user source, credentials, or ambient paths.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
    - Options Considered:
      - Skip review for branch-only code: rejected because visible syntax freshness/fallback is a decision gate.
      - Exhaustively polish a failed candidate: rejected.
      - Capture reached states and perform correction only while viable: chosen.
    - Chosen Approach:
      - Begin with `computer-use-linux.get_app_state`, inspect accessibility tree, use keyboard-only editing/folding where reached, and store bounded evidence with the report.
    - API Notes and Examples:
      ```text
      code-reviews/screenshots/2026-08-26-plan100-lezer-fail-fast/
      ```
    - Files to Create/Edit:
      - `code-reviews/screenshots/2026-08-26-plan100-lezer-fail-fast/**`: Screenshots, accessibility dumps, trace links, and findings.
      - `docs/development/client-local-parsing-spike-2026-08-26.md`: Visual/accessibility summary.
    - References:
      - `.agents/skills/clay-ui/SKILL.md#step-1-mandatory-inspect-implemented-ui`
  - Test Cases to Write:
    - Keyboard-only test: Editor focus, selection, typing, and reached structural commands remain operable.
    - Accessibility text test: Syntax markup does not duplicate or corrupt editable text.
    - Fallback test: Failed/pending syntax remains readable and editable.
  - Completion Evidence:
    - Not applicable: the grammar gate failed before any reached Lezer state existed. The dated report records the failure and the cleanup restored the unchanged server-owned editor path; no visual pass or Lezer accessibility claim is made.
    - No screenshot or accessibility artifact was created for the rejected candidate.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory touched Rust/TypeScript surfaces and confirm parser selection, registry, recorder, and ownership remain internal.
    - Performance: No public API starts synchronous parser work, loads modules, returns syntax trees, or changes budgets.
    - Code Quality: Expected public API delta is zero; any unavoidable capability requires separate approval before implementation.
    - Security: No parser object, CodeMirror extension, worker handle, module URL, trace content, raw op, or browser execution grant reaches package JavaScript.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
    - Options Considered:
      - Expose candidate selection for convenience: rejected.
      - Keep all spike surfaces internal and verify inventory unchanged: chosen.
    - Chosen Approach:
      - Treat any requested public client parser authority as a future decision input, not spike scope.
    - API Notes and Examples:
      ```text
      Expected public API delta: none.
      ```
    - Files to Create/Edit:
      - `docs/development/client-local-parsing-spike-2026-08-26.md`: API inventory result.
      - `docs/reference/clay-js-api/**`: No change expected.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
  - Test Cases to Write:
    - API inventory/registry guards pass unchanged.
    - Source review confirms spike selectors are absent from `clay:*` facades and raw package ops.
  - Completion Evidence:
    - No public API delta: the Lezer selector, recorder, and parser objects were never exposed through Clay facades or package operations. Existing API inventory and protocol validation remain green.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Candidate selection and tracing remain branch-only development inputs, not `init.js` behavior.
    - Performance: No configuration changes parser concurrency, memory, slice, timeout, viewport, or payload limits.
    - Code Quality: Expected configuration delta is zero; Plan 099 proposes any production need after final approval.
    - Security: Configuration grants no module, browser JavaScript, WASM, worker, filesystem, network, shell, Tauri, or process authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/clay-js-api/configuration.md`
      - `examples/init.js`
    - Options Considered:
      - User-selectable parser engine: rejected as unsupported product behavior.
      - Development-only closed selector: chosen.
    - Chosen Approach:
      - Keep canonical configuration unchanged and verify spike controls cannot ship.
    - API Notes and Examples:
      ```text
      No parser-placement option is added to init.js.
      ```
    - Files to Create/Edit:
      - `docs/development/client-local-parsing-spike-2026-08-26.md`: Configuration boundary result.
      - `examples/init.js`: No change expected.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - `node --check examples/init.js` passes unchanged.
    - Production/default build rejects or ignores spike parser selection.
  - Completion Evidence:
    - No configuration delta: the branch-only selector and `VITE_CLAY_PERF` input were removed with the spike integration; `init.js` and public configuration remain unchanged.

- [x] Execute relevant manual test-plan modules for reached Lezer behavior
  - Acceptance Criteria:
    - Functional: Run existing Linux launch, files/workspace, core editing, syntax/text-object, performance, splits, and tabs steps only for behavior reached before rejection. Record skipped steps as not applicable after the first hard failure.
    - Performance: Record device, WebKit version, fixture, gate, parser owner, p95/max, long-task count, and memory for reached performance steps.
    - Code Quality: Existing product steps are not weakened and branch-only behavior is not added as permanent product coverage.
    - Security: Manual runs use generated fixtures and sanitized evidence only.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `test-plan/index.md`
      - `test-plan/01-launch-and-connection.md`
      - `test-plan/03-files-and-workspace.md`
      - `test-plan/04-core-editing.md`
      - `test-plan/08-syntax-and-textobjects.md`
      - `test-plan/11-performance.md`
      - `test-plan/13-window-splits.md`
      - `test-plan/14-tabs.md`
    - Options Considered:
      - Run every module after early rejection: rejected as waste.
      - Run reached parity/performance contract and preserve stop evidence: chosen.
    - Chosen Approach:
      - Use current manual plan as user-visible contract without promoting experimental parser modes into permanent steps.
    - API Notes and Examples:
      ```text
      Candidate: client-lezer
      Gate: distant viewport freshness
      Result: pass or first hard failure
      ```
    - Files to Create/Edit:
      - `docs/development/client-local-parsing-spike-2026-08-26.md`: Manual results.
      - `test-plan/**`: No change expected unless a candidate-independent measurement procedure proves durable.
    - References:
      - `decision-logs/2026-08-04-1645-manual-test-plan-folder-and-per-plan-duty.md`
  - Test Cases to Write:
    - Manual completeness review: Every reached gate has pass/fail evidence and later skipped work names the stopping failure.
  - Completion Evidence:
    - Not applicable: no Lezer behavior remained reached after the grammar failure. The report explicitly records the first failure and marks all later manual/performance scenarios not applicable.

- [x] Publish the fail-fast report and recommend parser placement
  - Acceptance Criteria:
    - Functional: Report includes exact package/commit/device identity, method, raw links, reached gates, first failure or all-pass result, visual/manual findings, grammar evidence, package/headless consequences, and one recommendation.
    - Performance: Claims separate measured WebKitGTK results, isolated advisory probes, derived estimates, and assumptions.
    - Code Quality: If Lezer fails, recommendation confirms server-side per-document Tree-sitter sessions and identifies disposable spike files. If Lezer passes, report requests a new explicit decision before more prototype work.
    - Security: Report states executable trust domain, denied capabilities, adopted-package fallback, and why no worker/public parser surface was created.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `docs/development/editor-performance-review-2026-08-26.md`
      - `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
      - `decision-logs/2026-08-26-2137-lezer-fail-fast-before-server-syntax-overhaul.md`
    - Options Considered:
      - Continue after a failed hard gate to gather completeness: rejected.
      - Hide unexecuted work: rejected.
      - Publish first-failure evidence and a definitive next step: chosen.
    - Chosen Approach:
      - Make stop behavior part of result, not a limitation. No independent-model review is required unless the user requests one.
    - API Notes and Examples:
      ```text
      Result: rejected at gate 1-6, or all fail-fast gates passed.
      Next step: replan Plan 099, or request explicit scope before further comparison.
      ```
    - Files to Create/Edit:
      - `docs/development/client-local-parsing-spike-2026-08-26.md`: Final report and recommendation.
    - References:
      - `target/perf/client-local-parsing/**`
      - `code-reviews/screenshots/2026-08-26-plan100-lezer-fail-fast/**`
  - Test Cases to Write:
    - Report audit: Every claim links to raw evidence or is labeled assumption.
    - Stop audit: Report contains no claimed result for an unexecuted post-failure scenario.
  - Completion Evidence:
    - `docs/development/client-local-parsing-spike-2026-08-26.md` publishes the exact first grammar failure, current-fixture results, unexecuted-gate status, security boundary, and server-authoritative recommendation.

- [x] Obtain explicit parser-placement approval and record the resulting decision
  - Acceptance Criteria:
    - Functional: User explicitly approves either confirmed server Tree-sitter sessions after Lezer rejection or a separately scoped next step after all-pass evidence.
    - Performance: Decision records measured gate outcomes and exact condition that can reopen frontend-worker parsing.
    - Code Quality: New chronological log references, rather than rewrites, earlier 2026-08-26 decisions; reusable patterns reflect final approved placement.
    - Security: Decision fixes base owner, package model, fallback, client execution, provenance, and denied authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `.agents/skills/project-patterns/SKILL.md`
      - `docs/development/client-local-parsing-spike-2026-08-26.md`
      - `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
      - `decision-logs/2026-08-26-1857-spike-parser-placement-before-performance-implementation.md`
      - `decision-logs/2026-08-26-2137-lezer-fail-fast-before-server-syntax-overhaul.md`
    - Options Considered:
      - Treat this plan-scope approval as final parser approval: rejected; measured result still requires review.
      - Log recommendation without approval: prohibited.
      - Ask one exact approval question after report: chosen.
    - Chosen Approach:
      - Create a new decision only after explicit approval, then update the smallest relevant pattern files.
    - API Notes and Examples:
      ```text
      Approval scope: base parser owner, fallback, package trust, and worker reopen condition.
      ```
    - Files to Create/Edit:
      - `decision-logs/`: Add one chronologically named approved parser-placement record.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Final parser-placement rule.
      - `.agents/skills/project-patterns/references/tauri-react-client.md`: Update only if client execution boundary changes.
      - `.agents/skills/project-patterns/references/language-capability-sequencing.md`: Update only if parser/package sequencing changes.
    - References:
      - `.agents/skills/create-decision-log/SKILL.md`
  - Test Cases to Write:
    - Decision review: Approval evidence, alternatives, measurements, consequences, and reopen conditions are present.
    - Pattern review: Durable guidance matches approved decision without copying temporary spike details.
  - Completion Evidence:
    - `decision-logs/2026-08-27-0159-resume-server-authoritative-editor-performance.md` records the user's approval to resume Plan 099 with server-authoritative syntax after the Lezer rejection.
    - `.agents/skills/project-patterns/references/protocol-and-performance.md` records the durable server-authoritative resume rule.

- [x] Replan and resume Plan 099 under the approved parser direction
  - Acceptance Criteria:
    - Functional: Plan 099 pause is removed only after parser approval; parser-dependent tasks, files, tests, and ordering are updated from measured evidence while universal frontend fixes remain intact.
    - Performance: Plan 099 retains 8 ms edit, 16 ms scroll, zero over-50-ms task, 200 ms large-file syntax-freshness, 256 MiB memory, and physical-device gates unless user explicitly changes them.
    - Code Quality: Every spike file is classified for deletion or deliberate reimplementation; no branch selector, raw recorder, or duplicate base syntax path becomes production accidentally.
    - Security: Replanned work follows approved package/artifact/client authority and retains one base syntax owner throughout migration.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/SKILL.md`
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/SKILL.md`
      - `.agents/skills/project-patterns/references/planning-checklist.md`
      - Approved final parser-placement decision.
      - `plans/099-Clay-Editor-Performance-Overhaul.md`
      - `docs/development/editor-performance-review-2026-08-26.md`
      - `docs/development/client-local-parsing-spike-2026-08-26.md`
    - Options Considered:
      - Create another plan solely because Plan 100 existed: rejected as unnecessary churn.
      - Resume stale Plan 099 unchanged: rejected because measured evidence must update parser-dependent work.
      - Replan Plan 099 in place, preserving its audit and valid tasks: chosen.
    - Chosen Approach:
      - Remove pause after approval, update only affected approaches/dependencies, preserve historical completion evidence, and execute the first remaining unchecked task.
    - API Notes and Examples:
      ```text
      Plan 099 status after approval: active, replanned from Plan 100 evidence.
      ```
    - Files to Create/Edit:
      - `plans/099-Clay-Editor-Performance-Overhaul.md`: Replan and reactivate.
      - `plans/100-Client-Local-Parsing-Spike-and-Parser-Placement-Decision.md`: Record final decision and Plan 099 resume evidence.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - Coverage review: Every still-valid Plan 099 bottleneck remains planned; rejected parser work is removed or rewritten.
    - No-dual-authority review: Every migration stage has one production base syntax owner.
    - Plan structure check: Updated tasks retain acceptance, approach, files, references, and tests.
  - Completion Evidence:
    - `plans/099-Clay-Editor-Performance-Overhaul.md` is active and re-planned in place with server-authoritative Tree-sitter sessions as the current direction.
    - Lezer-only dependencies, editor integration, recorder, and runner were removed; the dated report and decision records remain available.

- [x] Update or verify the code wiki after spike completion
  - Acceptance Criteria:
    - Functional: Wiki is updated once after spike verification, final decision, pattern updates, and Plan 099 replanning, or explicitly verified unchanged because disposable code was removed and no stable implementation changed.
    - Performance: Wiki records only approved stable architecture and measurement commands, not advisory Node probes or rejected branch behavior as guarantees.
    - Code Quality: Changed pages explain responsibilities, flow, invariants, source/test paths, extension guidance, and remain linked from `docs/wiki/index.md`.
    - Security: Wiki records approved parser/package/client authority and denied capabilities without exposing secrets or ambient paths.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
    - Options Considered:
      - Document every disposable prototype module: rejected.
      - Update only durable approved architecture, or record verified-no-change: chosen.
    - Chosen Approach:
      - Keep experimental details in the dated development report and update wiki only where stable ownership guidance changed.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/react-codemirror-editor.md
      docs/wiki/modules/parse-coordinator.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Update navigation only when stable pages change.
      - `docs/wiki/modules/react-codemirror-editor.md`: Approved client parser boundary if changed.
      - `docs/wiki/modules/parse-coordinator.md`: Approved server syntax role if changed.
      - `docs/wiki/modules/decoration-transport.md`: Approved base/semantic transport role if changed.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - Approved final parser-placement decision.
  - Test Cases to Write:
    - Manual wiki review: Index links every changed page and no page presents branch-only code as production.
    - Documentation guards: Existing wiki/source-path and primitive documentation tests pass.
  - Completion Evidence:
    - Removed the disposable `client-local-parsing-spike` wiki page and its index link with the spike implementation; retained the dated development report as the historical source of truth.
    - `cargo test --test protocol` passes, including wiki navigation and documentation coverage guards.

## Compromises Made

- Plan 100 intentionally does not build frontend-worker Tree-sitter, a hybrid parser, or complete Lezer parity before stock Lezer passes its cheapest disqualifying gates.
- Isolated Node measurements guide fixture selection only; real Tauri/WebKitGTK evidence remains mandatory.
- The ordered run stopped at grammar freshness: current fixtures passed, frozen modern Rust/TypeScript did not. Later edit-latency, main-thread, browser, viewport, memory, pane, and reproducibility gates are explicitly not applicable rather than inferred passes.
- Structural parity, visual review, manual Lezer behavior, public API, and configuration work were not performed for a rejected candidate; each is recorded as not applicable.
- The disposable Lezer production integration and dependencies were removed while the dated report and decision history were retained.

## Further Actions

- Continue `plans/099-Clay-Editor-Performance-Overhaul.md` from its first remaining task on the server-authoritative path.
- Frontend-worker Tree-sitter may be reconsidered only after the completed server-session overhaul misses approved metrics and traces attribute the remaining delay to server/bridge placement.
- Treat this plan as historical evidence for the rejected client-local candidate; do not revive its removed spike code wholesale.
