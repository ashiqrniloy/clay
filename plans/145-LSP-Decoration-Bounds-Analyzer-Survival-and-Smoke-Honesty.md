# LSP Decoration Bounds, Analyzer Survival, and Smoke Honesty

Follow-up to plan 128 (`plans/128-LSP-Shared-Incremental-Position-Index.md`)
whose live run found that the first real LSP payload on an ordinary source file
kills the analyzer worker, and that two size caps keep the language route off
large documents regardless of index speed. Plan 128 made the index cheap; this
plan makes the route *survive* what the index now makes affordable.

## Objectives

- A bound violation anywhere in the LSP payload pipeline degrades to a bounded
  subset — it never destroys the analyzer worker (`analysis.worker_failed`).
  The live failure is `lsp.invalid_semantic_tokens: bounded five-integer
  records required` from `packages/lsp-shared/mapping.js:134` at ≈250 KiB, after
  which the status bar reads "Document analyzer stopped; baseline language
  support remains active."
- LSP decorations reach ordinary files (>128 semantic tokens) again, with the
  publication shape decided from existing primitives rather than a bigger
  constant: either a viewport-scoped analyzer/decoration window or an explicit
  bounded truncation, whichever the primitive review supports.
- The two hard ceilings on the language route are decided with measurements, not
  left implicit: `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` (256 KiB,
  `src/perf/budgets.rs:33`) and `MAX_FRAME_BYTES` (1 MiB,
  `packages/lsp-shared/framing.js`).
- `lsp_real_servers` fails only for real reasons: an empty `inlayHint` payload
  must not mask the semantic payload, and one failing server must not hide the
  other smoke results.
- Scope statement (binding): **no new user-facing configuration and no new
  authority.** Bounds stay derived from the primitive that owns them; if any task
  concludes an option is unavoidable, the plan is amended with the Clay
  Configuration, example-config, and example-config-launch-test tasks before
  merge. No UI prototype gate applies — this plan changes decoration *content*
  and worker survival, not chrome, layout, tokens, or typography.

## Expected Outcome

- Opening a real Rust crate (≈250 KiB open module) shows LSP semantic
  decorations and keeps the analyzer worker alive across edits and refreshes.
- Oversized or structurally unexpected LSP payloads produce a bounded
  publication (with a recorded truncation/diagnostic signal) instead of a dead
  worker; the analyzer error path is reserved for genuine faults.
- The large-file behavior is one explicit, tested decision: either the caps rise
  with evidence, or the fail-closed stay is documented in reference docs and
  covered by a test naming the ceiling.
- `CLAY_LSP_REAL_SMOKE=1 cargo test --test lsp_real_servers` reports every
  server's result, passes for markdown/rust-analyzer on this host, and skips
  typescript-language-server with an actionable reason.
- `test-plan/08-syntax-and-textobjects.md` (S35) records the new live behavior;
  the code wiki documents the bound/degrade invariants.

## Tasks

- [ ] Baseline: reproduce the analyzer kill, the diagnostics gap, and the smoke reds on the unmodified tree
  - Acceptance Criteria:
    - Functional: the ≈250 KiB analyzer kill is reproduced from package JavaScript (a fake-server or direct-adapter call is enough; the live leg is task 8), and the three smoke behaviors are recorded: the `rust-real-smoke` assertion failure, the missing `mismatched types` gutter marker, and which servers report when one fails.
    - Performance: record the current semantic-token publication cost and payload size for a document that exceeds the bound (bytes published, ms), as the pre-change comparison for tasks 3–5.
    - Code Quality: evidence under `code-reviews/2026-09-20-plan142-baseline/` with exact commands, environment, and file:line of every failure; no source edits.
    - Security: none (read-only baseline).
  - Approach:
    - Documentation Reviewed:
      - `plans/128-LSP-Shared-Incremental-Position-Index.md` (task 4 record); `test-plan/artifacts/128-lsp-incremental-index/README.md` (live harness); `.agents/skills/clay-execution/references/protocol-perf.md`.
    - Options Considered:
      - Live-first baseline: closest to the user-visible failure, but needs the GUI and portal input consent (unavailable on this host).
      - Adapter-first baseline plus the existing fake-server matrix: deterministic, no GUI, and the matrix already drives all four bridges.
    - Chosen Approach:
      - Adapter + fake-server baseline for the kill and payload numbers; reuse the plan-128 harness for the live reproduction in task 8. Add a fake-server profile that returns more than `MAX_SEMANTIC_TOKENS * 5` integers so the failure is deterministic in CI.
    - API Notes and Examples:
      ```bash
      node --import ./tests/fixtures/lsp/register-lsp-shared.mjs --test \
        tests/fixtures/lsp/fake-server/matrix.test.mjs
      git show HEAD:packages/lsp-shared/mapping.js > /tmp/mapping-baseline.js   # <=> compare pre-change behavior
      CLAY_LSP_REAL_SMOKE=1 cargo test --test lsp_real_servers -- --nocapture
      ```
    - Files to Create/Edit:
      - `code-reviews/2026-09-20-plan142-baseline/`: README + logs (new, evidence only).
      - `tests/fixtures/lsp/fake-server/profiles.mjs`: add an oversized-semantic profile (new case; see task 4).
    - References:
      - `packages/lsp-shared/mapping.js:128,134`; `packages/lsp-shared/bridge.js:199,262,283` (full-document viewport); `src/server/document_analysis.rs:1166`; `packages/lsp-rust/rust-real-smoke.test.mjs:93`.
  - Test Cases to Write:
    - Baseline probe: a document whose token count exceeds the bound must currently throw `lsp.invalid_semantic_tokens` (the pre-change assertion of record).

- [ ] Review viewport/window primitives and analyzer event shape before choosing the publication fix
  - Acceptance Criteria:
    - Functional: the plan records what already exists for bounded, viewport-scoped publication — `DecorationSet` viewport ranges and near-viewport eviction (`src/server/decorations.rs`), the plan-099 syntax viewport continuity flow, range-scoped provisional replacement (plans 057/058), `CompletionDocumentWindow` — and states whether a **generic** analyzer-viewport event is missing (`DocumentAnalysisEvent` currently has Open/Change/Reset/Completion/LanguageIntelligence/Shutdown and no viewport variant).
    - Performance: the chosen shape must keep per-refresh payload within `DECORATION_PAYLOAD_BYTES` (8 KiB) and per-refresh work bounded, with the expected refresh cost stated before implementation.
    - Code Quality: reuse before invention; any new primitive is generic (any analyzer: LSP tokens, diagnostics, future analyzers), never LSP-specific or named after a language; the decision and rejected options are recorded in the task evidence.
    - Security: the chosen shape adds no cross-domain authority: only bounded, inert, host-serialized values reach the analyzer and the decoration pipeline.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/range-diagnostics.md`, `docs/wiki/modules/first-party-lsp-bridge-packages.md` (`positions.js`, `mapping.js`); `.agents/skills/clay-execution/references/packages.md` (authority boundaries), `references/protocol-perf.md`.
    - Options Considered:
      - Raise `MAX_SEMANTIC_TOKENS` and the 8 KiB decoration budget: smallest diff, but the decoration budget exists for a reason (client payload/compositor cost) and a 4 MiB file still exceeds any constant.
      - Bounded truncation only (publish the first N records): unblocks the worker immediately, keeps colors at the top of the file, no new primitive — but colors stop at an arbitrary offset and the client cannot tell why.
      - Viewport-scoped publication: analyzer is told the visible range and publishes only intersecting tokens, reusing the existing decoration viewport machinery; needs a generic viewport event on the analysis route.
      - Re-request on scroll (client-driven refresh): no server change, but the analyzer would refresh per scroll event — hot-path work with no bounds.
    - Chosen Approach:
      - Decide in-task with evidence: prefer viewport-scoped publication through existing decoration viewport primitives plus the smallest generic analyzer-viewport event; keep bounded truncation as the guaranteed floor so no configuration can produce a dead worker.
    - API Notes and Examples:
      ```js
      publishDecorations({
        packageManifest, documentId, documentVersion: version, currentDocumentVersion: version,
        viewport: { byteStart: windowStart, byteEnd: windowEnd },   // instead of 0..byteLength
        spans: semanticTokensToClay(windowedData, legend, document),
      });
      ```
    - Files to Create/Edit:
      - `plans/145-...md`: record the decision + rejected options (this task's evidence).
      - `docs/reference/primitives/registry.md`, `docs/wiki/modules/decoration-transport.md`: only if the primitive inventory changes.
    - References:
      - `packages/lsp-shared/bridge.js:199,262,283` (all viewport fields are the whole document); `src/server/decorations.rs:96,109,272,283`; plans 057/058/099 records.
  - Test Cases to Write:
    - None (review task); its decision feeds task 4's tests.

- [ ] Make LSP payload bounds degrade instead of killing the analyzer worker
  - Acceptance Criteria:
    - Functional: a payload above a mapping bound (token records, diagnostics, completions, inlay hints) produces a bounded publication plus an explicit machine-readable truncation signal (e.g. `truncated: true` / dropped count) and an operator-visible status or log line; `lsp.invalid_semantic_tokens` remains only for structurally invalid data (record length not a multiple of five, unknown legend entry, invalid modifier bits). A bound violation must not reach the worker error path described by `analysis.worker_failed`.
    - Performance: a truncated publication costs no more than the unbounded one on a document that overflows (measure both); the bound check stays O(payload).
    - Code Quality: the truncation policy lives once in `mapping.js` (shared by all four bridges), is unit-tested, and is documented in the bridge module docs; no per-package special cases. Analyzer handlers must not be able to convert a mapping exception into a worker death by default.
    - Security: truncation keeps every existing validation (legend membership, modifier bits, range containment, package provenance, document version) — a truncated set is a subset of a validated set, never a relaxed one; no new op, permission, or cross-domain authority (trust domains unchanged: bounded inert values only).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/packages.md` (authority boundaries, trust domains), `references/protocol-perf.md`, `docs/wiki/modules/first-party-lsp-bridge-packages.md`, `docs/wiki/modules/range-diagnostics.md`; decision logs `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`, `decision-logs/2026-07-15-1750-phase18.21-package-worker-authority.md`.
    - Options Considered:
      - Catch mapping errors in the analyzer handler: keeps bad payloads out of the worker's death path but hides real bugs and still publishes nothing.
      - Bound at publication in `mapping.js` with an explicit signal: fixes every caller at once and keeps the contract honest. (Chosen.)
      - Push the bound down to the LSP client (reject frames earlier): wrong layer — the frame is valid LSP, only Clay's publication budget is smaller.
    - Chosen Approach:
      - Truncate deterministically at the mapping boundary with an explicit signal and a single shared policy, and make the analyzer handler degrade rather than die on unexpected mapping input (defense in depth), so a single oversized payload cannot disable language support for the session.
    - API Notes and Examples:
      ```js
      const { spans, truncated, dropped } = boundedSpans(items, MAX_SEMANTIC_TOKENS, (item) => tokenToSpan(item, legend, document));
      // publish spans + report truncated/dropped with the decoration set (never throw for size)
      ```
    - Files to Create/Edit:
      - `packages/lsp-shared/mapping.js`: bound-violation behavior + signal (truncate, keep structural throws).
      - `packages/lsp-shared/bridge.js`: publish/report the signal; degrade instead of propagating size errors.
      - `packages/lsp-shared/adapter.test.mjs`: bound-behavior tests.
      - `tests/fixtures/lsp/fake-server/profiles.mjs` + `matrix.test.mjs`: oversized-semantic profile that returns more records than the bound.
      - `src/server/document_analysis.rs` (only if the degrade path requires a status/event plumbing change).
    - References:
      - `packages/lsp-shared/mapping.js:3,128,134,144,151`; `src/server/decorations.rs` (payload validation); `src/server/document_analysis.rs:1166`.
  - Test Cases to Write:
    - Oversized semantic payload: the bridge publishes a bounded set, reports truncation, and the analyzer worker stays alive for the next refresh.
    - Structurally invalid payload (length not a multiple of five, unknown token type): still throws the existing typed error.
    - Each of diagnostics/completions/inlay hints above its bound: bounded publication, no throw.
    - Regression: a payload exactly at the bound is published whole with `truncated: false`.

- [ ] Implement the chosen bounded publication shape (viewport window or explicit truncation) with tests
  - Acceptance Criteria:
    - Functional: an ordinary Rust file (≈250 KiB, more than 128 semantic tokens) shows LSP decorations over the region the design covers, and scrolling/paging updates them; an inlay-hint-only or diagnostics-only payload still publishes independently; the analyzer is never asked to publish more than its budgets allow. If the primitive review chose viewport windowing, the analyzer receives the visible range through the generic event and the published set carries the matching `viewport`; if it chose truncation, the client shows a bounded region with a truthful truncation signal.
    - Performance: per-refresh published bytes stay within `DECORATION_PAYLOAD_BYTES` (8 KiB) and per-refresh work is bounded (record p50/p95 for a 250 KiB and a 1 MiB document); no O(document) payload on any refresh.
    - Code Quality: exactly one publication path shared by all four bridges; the fake-server matrix covers the new shape for rust/typescript/javascript/markdown, and `tests/lsp_bridge.rs` still passes; the shape is documented in `docs/reference/packages/creating-packages.md` (LSP bridge section) if any author-visible behavior changes.
    - Security: viewport values are validated inert integers bounded by the document length; no package-supplied range can widen publication beyond the document; one-line `loadPackage("@clay/lsp-rust")` remains sufficient — no new configuration is required for correct behavior (packages never become behavior-changing defaults silently, and here they stay behavior-changing *without* config).
  - Approach:
    - Documentation Reviewed:
      - Task 2's notes; `docs/reference/packages/creating-packages.md` (LSP bridge section); `docs/wiki/modules/first-party-lsp-bridge-packages.md`; `test-plan/08-syntax-and-textobjects.md` (S33/S34 viewport continuity steps); `.agents/skills/clay-execution/references/packages.md`.
      - LSP 3.17 semantic tokens (`textDocument/semanticTokens/full` vs `/full/delta`) — https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_semanticTokens — defines the full-document data array the current bridge sends to the bound check; the plan keeps the wire contract and changes only Clay's publication budget.
    - Options Considered:
      - As in task 2; this task implements the recorded choice and may not silently change it (a change re-opens task 2's record).
    - Chosen Approach:
      - Implement the task-2 choice on top of the existing decoration viewport primitives, with bounded truncation as the floor, so the analyzer is resilient regardless of file size.
    - API Notes and Examples:
      ```js
      // viewport-scoped publication (if chosen): the set carries the window it describes
      publishDecorations({ ..., viewport: { byteStart: start, byteEnd: end }, spans });
      ```
    - Files to Create/Edit:
      - `packages/lsp-shared/bridge.js` (+ `client.js` if the request needs a range), `packages/lsp-shared/mapping.js` (windowed helpers), `packages/lsp-shared/adapter.test.mjs`, `tests/fixtures/lsp/fake-server/*`, `src/server/document_analysis.rs` / `src/server/connection/documents.rs` only if the generic viewport event is required.
      - `tests/lsp_bridge.rs`: matrix/assertion updates.
    - References:
      - `packages/lsp-shared/bridge.js:236–300`; `src/server/decorations.rs:96–300`; plans 057/058/099.
  - Test Cases to Write:
    - Fake-server matrix: oversized document publishes windowed/truncated decorations for all four bridges, then a viewport change re-publishes the new region (or the truncation signal stays stable).
    - Stale window: a version bump during a windowed refresh publishes nothing for the old version.
    - Payload bound: published bytes for a 4 MiB document stay within the decoration budget.
    - Package adapter test: `lsp-rust` decorations for a fixture with >128 tokens are non-empty.

- [ ] Decide and implement the large-document caps (analysis 256 KiB, frame 1 MiB)
  - Acceptance Criteria:
    - Functional: one recorded decision per cap with evidence: raise it (measured cost accepted) or keep it (documented fail-closed stay with a test naming the exact boundary and the user-visible status text). `DocumentAnalysisEvent`/`framing.js` limits and the status messages in `src/server/document_analysis.rs` must agree with the decision; no silent mismatch between "analysis limit" and "frame too large".
    - Performance: if a cap rises, the plan records the resulting end-to-end cost on the largest allowed document (open, edit, refresh) with the incremental index in place; if it stays, the plan records the measured headroom (index edit cost vs. remaining bottlenecks) so the stay is a decision, not an accident.
    - Code Quality: the cap lives in `src/perf/budgets.rs` with the other typed budgets, is referenced by tests (`tests/performance_budgets.rs` / `tests/performance_protocol.rs`), and the reference docs state the behavior for documents above it.
    - Security: a raised cap must not become an unbounded allocation or a synchronously blocking open path: input/output queue bounds, coalesced resets, and admission checks stay enforced.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md`; `.agents/skills/clay-execution/references/protocol-perf.md`; `plans/119-...md` (the 256 KiB cap recorded as the PF-1 mitigation, now superseded by the incremental index); `src/perf/budgets.rs`.
    - Options Considered:
      - Raise the analysis cap (e.g. 256 KiB → 1 MiB) and the frame budget to match: unlocks the plan's large-file language route, needs queue/memory evidence on a 1 MiB open plus incremental edits.
      - Keep both caps and document them as a known ceiling: honest and cheap; leaves large-file language intelligence explicitly out of scope with a test and a reference-doc statement.
      - Raise only the analysis cap: gives decorations without the frame budget for `didOpen`/full-sync changes — incoherent unless protocol framing stops carrying whole documents.
    - Chosen Approach:
      - Measure first (open/edit/refresh at 256 KiB, 512 KiB, 1 MiB with the incremental index), then pick the coherent option and cover it with budgets tests.
    - API Notes and Examples:
      ```bash
      CLAY_PERF_PROFILE=1 CLAY_PERF_REPORT_DIR=/tmp/p142 cargo test --test runtime large_document::
      ```
    - Files to Create/Edit:
      - `src/perf/budgets.rs`, `packages/lsp-shared/framing.js`, `src/server/document_analysis.rs` (messages/checks as needed), `tests/performance_budgets.rs`, `tests/lsp_bridge.rs`, `docs/development/performance.md`, `docs/reference/packages/creating-packages.md`.
    - References:
      - `src/perf/budgets.rs:33`; `packages/lsp-shared/framing.js` (`MAX_FRAME_BYTES`); `tests/large_document.rs`.
  - Test Cases to Write:
    - Boundary test at the decided cap (±1 byte) asserting the allowed path and the user-visible status text for the denied path.
    - Budget lock test so the cap cannot drift silently.

- [ ] Fix the diagnostics rendering gap (rust-analyzer type error produced no gutter marker)
  - Acceptance Criteria:
    - Functional: a fixture with a real rust-analyzer `mismatched types` error produces a visible diagnostic marker at the reported range (pull or push route as the package declares), and the same holds for the fake-server diagnostics profiles; if the gap is a rendering/transport ceiling rather than a bridge bug, the plan records the ceiling with a failing-example test name and the user-visible consequence.
    - Performance: diagnostics publication stays within `DIAGNOSTIC_PAYLOAD_BYTES` and does not add per-keystroke work (diagnostics arrive on the analyzer refresh).
    - Code Quality: the fix lands on the generic diagnostic path (`diagnosticsToClay` → `DiagnosticSet` → transport), not in the Rust bridge package; a test fails without the fix.
    - Security: diagnostic text stays sanitized/bounded as today; no package-supplied URI or range can escape the open-document set.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/range-diagnostics.md`, `docs/wiki/modules/first-party-lsp-bridge-packages.md`, `docs/reference/primitives/registry.md` (DiagnosticSet), `.agents/skills/clay-execution/references/protocol-perf.md`.
    - Options Considered:
      - Bridge-side fix: correct only if `diagnosticsToClay` drops or mis-maps the items (verify first with a captured payload).
      - Transport/render fix: likely if the set publishes but the pane shows nothing — this is the generic DiagnosticSet path shared with syntax diagnostics.
      - Record as ceiling: correct only if the gap is an environment artifact of this host's rust-analyzer.
    - Chosen Approach:
      - Trace one captured `textDocument/publishDiagnostics` / pull response end to end (mapping output → protocol → pane) before changing anything, then fix the layer that actually drops it.
    - API Notes and Examples:
      ```bash
      CLAY_LSP_REAL_SMOKE=1 node --import ./tests/fixtures/lsp/register-lsp-shared.mjs \
        --test packages/lsp-rust/rust-real-smoke.test.mjs
      ```
    - Files to Create/Edit:
      - Tentative: `packages/lsp-shared/mapping.js` and/or `packages/lsp-shared/bridge.js`, `tests/fixtures/lsp/fake-server/profiles.mjs`, `src/server/diagnostics.rs` / decoration transport only if the trace proves it.
    - References:
      - Task 4 observation (no marker after ~30 s); `packages/lsp-shared/mapping.js` (`diagnosticsToClay`); `src/server/diagnostics.rs`.
  - Test Cases to Write:
    - Fixture diagnostic (fake server) reaches the pane's diagnostic set with the expected range and source.
    - Real rust-analyzer smoke: a type error yields at least one diagnostic with a non-empty message.

- [ ] Make the real-server smoke honest (kind-filtered assertions, no short-circuit, provisioning)
  - Acceptance Criteria:
    - Functional: `packages/lsp-rust/rust-real-smoke.test.mjs` asserts on a decoration set of the kind it means (semantic vs `inlayHint`) so an empty inlay payload cannot mask a semantic payload; `tests/lsp_real_servers.rs` runs every available server and reports all failures instead of stopping at the first; unavailable servers skip with an actionable reason (install command) instead of a generic message.
    - Performance: smoke runtime unchanged within noise (it is environment-gated, never in hot paths).
    - Code Quality: assertion helpers express intent (`lastSpansOfKind(decorations, "semantic")`), and the gate remains opt-in (`CLAY_LSP_REAL_SMOKE=1`).
    - Security: none (test-only change); smoke keeps using host toolchains and must not gain implicit network or install authority from the test run.
  - Approach:
    - Documentation Reviewed:
      - `tests/lsp_real_servers.rs`, `test-plan/artifacts/128-lsp-incremental-index/README.md` (the observed masking), `.agents/skills/clay-execution/references/packages.md` (external process authority: tests never gain install authority).
    - Options Considered:
      - Filter by kind in the assertion: fixes the mask without weakening the check. (Chosen.)
      - Skip empty decoration payloads: hides real regressions; the payload sequence is part of the contract.
      - Install typescript-language-server as part of the test: gives tests install authority — rejected; provisioning is documented for the host/CI instead.
    - Chosen Approach:
      - Fix the assertion, make the Rust runner report per-server results, and document the `typescript-language-server` provisioning step (`npm i -g typescript-language-server typescript`) in the smoke's skip message and in the test-plan harness README.
    - API Notes and Examples:
      ```bash
      npm i -g typescript-language-server typescript   # host provisioning, documented, not automatic
      CLAY_LSP_REAL_SMOKE=1 cargo test --test lsp_real_servers -- --nocapture
      ```
    - Files to Create/Edit:
      - `packages/lsp-rust/rust-real-smoke.test.mjs`, other `*-real-smoke.test.mjs` if the same pattern exists, `tests/lsp_real_servers.rs`, `test-plan/artifacts/128-lsp-incremental-index/README.md` (provisioning note).
    - References:
      - Plan 128 further actions (smoke masking + typescript provisioning); `tests/lsp_real_servers.rs:35` (`run_node_tests` asserts the first failing batch).
  - Test Cases to Write:
    - Smoke runner with one intentionally failing server: both results are reported (no short-circuit).
    - Assertion helper unit case: a decoration sequence ending in an empty `inlayHint` set still finds the semantic set.

- [ ] Verify live on a real Linux build (analyzer survival, decoration coverage, large-file decision)
  - Acceptance Criteria:
    - Functional: on a real GUI build with rust-analyzer available, a ≈250 KiB open module shows LSP semantic decorations and the analyzer stays alive across several edits (no "Document analyzer stopped", no `analysis.worker_failed` in the log); behavior at ≥1 MiB matches the task-5 decision; the diagnostics fixture from task 6 shows its marker.
    - Performance: record `bridge.patch_delivery` p50/p95 and `server.edit_ack`/`edit_apply` samples from `CLAY_PERF_PROFILE`; published payload sizes stay within their budgets.
    - Code Quality: reuse the isolated harness (`test-plan/artifacts/128-lsp-incremental-index/launch-live.sh`) with window-cropped screenshots as evidence; if host input consent is still missing, record the blocker and leave the typing leg unresolved rather than claiming a pass.
    - Security: private HOME/XDG/socket/TMPDIR, host toolchain symlinked read-only; no secrets, no network grants added to configuration.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/artifacts/128-lsp-incremental-index/README.md`, `test-plan/08-syntax-and-textobjects.md`, `.agents/skills/clay-execution/references/protocol-perf.md`.
    - Options Considered:
      - Reuse the plan-128 harness and fixtures: consistent evidence shape and comparable numbers. (Chosen.)
      - Build a new harness: no benefit; the harness is already parameterized by document size.
    - Chosen Approach:
      - Re-run the existing harness in `mid` mode (≈250 KiB) and `large` mode (≥1 MiB) plus the diagnostics fixture; capture window crops and the perf summary into `test-plan/artifacts/145-lsp-bounds/`.
    - API Notes and Examples:
      ```bash
      test-plan/artifacts/128-lsp-incremental-index/launch-live.sh start mid
      # ... observe, screenshot, then:
      test-plan/artifacts/128-lsp-incremental-index/launch-live.sh stop
      ```
    - Files to Create/Edit:
      - `test-plan/artifacts/145-lsp-bounds/` (new evidence dir: screenshots, perf summary, log excerpts); reuse of the plan-128 scripts rather than copies.
    - References:
      - `src/server/document_analysis.rs` (status messages), `packages/lsp-shared/bridge.js`.
  - Test Cases to Write:
    - Live: open ≈250 KiB Rust module → semantic decorations present, no analyzer stop, patch delivery samples recorded.
    - Live: edit and save → analyzer survives, version accepted, decorations refresh.
    - Live: ≥1 MiB document → behavior equals the documented decision (route works, or fail-closed with the named status text).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: verify (expected outcome) that no public programmatic surface changed — the work stays inside `packages/lsp-shared/`, the analyzer event plumbing, and budgets. If a public surface does change (e.g. a truncation/diagnostic signal exposed to packages, or a documented option), it is implemented as a real Clay JS API: dotted ID, stable facade, Markdown doc with custom properties, generated registry entry, and coverage tests.
    - Performance: none.
    - Code Quality: `cargo test --test protocol` passes in full, including `clay_js_api_inventory`, `clay_js_doc_registry` (generated registry current), `clay_js_facade_layout`, `documentation_coverage`.
    - Security: any new API documents authority boundaries; none of this work grants filesystem, network, process, or configuration authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`; `docs/reference/clay-js-api/api-inventory.toml`; decision log `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
    - Options Considered:
      - Verify-only (expected): the bounds are internal policy, not a user-facing knobs surface. (Chosen.)
      - Expose a tunable bound: rejected in this plan's scope statement — it would create a configuration surface with no evidence anyone needs it.
      - Expose a truncation counter to packages: only if task 3's signal proves useful to bridge authors beyond diagnostics.
    - Chosen Approach:
      - Verify-only, with the truncation signal kept as an internal bridge/log contract (documented in the wiki and module docs, not in the public API registry) unless a package-facing need appears during implementation.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js
      ```
    - Files to Create/Edit:
      - None expected (verification-only).
    - References:
      - Plan 128 task 5 record (same verify-only pattern and its evidence style).
  - Test Cases to Write:
    - None beyond the existing registry/doc guards.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: `test-plan/08-syntax-and-textobjects.md` S35 (and module 11 Q43 where latency is claimed) is re-run on a real build and its execution record updated with the new behavior: ≈250 KiB shows decorations with a live analyzer; ≥1 MiB matches the task-5 decision; the diagnostics marker from task 6 is visible or explicitly unresolved. New steps are added only for genuinely new user-visible behavior, with expected results and negative checks.
    - Performance: the record states measured numbers (patch delivery, edit ack) or names the ceiling that prevented them; no unmeasured "feels fast" claims.
    - Code Quality: `test-plan/index.md` updated when the module map, coverage matrix, or plan-execution records change; steps cross-link `docs/development/performance.md` instead of duplicating numbers.
    - Security: no step weakens an existing check; the analyzer-survival step includes the negative check "no analyzer-stopped status appears".
    - Note: this plan changes decoration content and worker survival, not chrome/layout/tokens, so no UI prototype gate or UI-specific review task applies; live visual verification happens inside this task.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`, `test-plan/08-syntax-and-textobjects.md`, `test-plan/11-performance.md`, `test-plan/artifacts/128-lsp-incremental-index/README.md`, `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Options Considered:
      - Update the existing S35 record (chosen): the step already asks the right question; only the outcome changes.
      - Add a new step for analyzer survival: add only if the negative check (no analyzer stop) needs its own numbered step per module conventions.
    - Chosen Approach:
      - Re-run S35 legs b/c/d after implementation, replace the recorded outcomes with the new pass/fail, and add a numbered step for analyzer survival/truncation signaling if the module's granularity requires it.
    - API Notes and Examples:
      ```bash
      test-plan/artifacts/128-lsp-incremental-index/launch-live.sh start mid
      ```
    - Files to Create/Edit:
      - `test-plan/08-syntax-and-textobjects.md`, `test-plan/11-performance.md`, `test-plan/index.md`, `docs/development/tauri-react-parity-ledger.json` (register any new step IDs).
    - References:
      - `docs/development/manual-editor-capabilities-test-plan.md` (module contract), plan 128 task 4 record.
  - Test Cases to Write:
    - Manual steps as described (no new automated suites here).

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the LSP bridge wiki documents the bound/degrade policy (which violations truncate, which throw, what the truncation signal is), the chosen publication shape, and the large-document cap decision; `docs/wiki/index.md` stays navigable; any primitive learned here is recorded in the primitive registry/wiki.
    - Performance: the pages state the per-refresh payload bounds and the measured p50/p95 for the windowed/truncated path.
    - Code Quality: pages explain what changed (not just what exists), the invariants, the tradeoffs, and the test paths that guard them; no archive page is created for an unfinished phase.
    - Security: the pages restate the boundaries touched (bounded inert cross-domain values, no new authority, host-owned text only for the index).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki workflow, quality bar, archive policy.
    - Options Considered:
      - Update after each task: noisy.
      - Update once after tests pass: keeps docs aligned with final behavior. (Chosen.)
    - Chosen Approach:
      - One pass at the end across `docs/wiki/modules/first-party-lsp-bridge-packages.md` (primary), `decoration-transport.md` / `range-diagnostics.md` (if the publication shape changes), and `docs/wiki/index.md` descriptions.
    - API Notes and Examples:
      ```markdown
      ### Bound violations
      - Oversized payload → bounded publication + truncation signal (never a worker death)
      - Structurally invalid payload → typed `lsp.invalid_*` error
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/first-party-lsp-bridge-packages.md`; `docs/wiki/index.md`; `docs/wiki/modules/decoration-transport.md` or `range-diagnostics.md` if the shape changes them.
    - References:
      - `docs/wiki/modules/first-party-lsp-bridge-packages.md` (plan-128 update for style/level).
  - Test Cases to Write:
    - Manual wiki review: master index links relevant pages; updated pages explain what changed and how it is tested.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.

Known items deliberately not scheduled here (trigger conditions instead):

- Port the frontend's long-line scan blocks (`SEGMENT_UNITS`) into `lsp-shared/positions.js` only if a workflow hits multi-megabyte single-line files on the LSP route; `long_single_line_does_not_regress` bounds current behavior (plan 128, task 2).
- Expose a package-facing byte-offset helper from the position index only if a package needs it; that becomes a real Clay JS API change (inventory + generated registry + facade guards).
- Host input consent for live typing legs (portal "Allow Remote Interaction") is environment state recorded in `test-plan/index.md`, not plan work; plan 146's input-path task settles the durable local procedure (`ydotoold`/uinput or the portal short-burst fallback) this task's typing legs can then use.