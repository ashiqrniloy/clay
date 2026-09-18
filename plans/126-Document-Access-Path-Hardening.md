# 126 — Document-Access Path Hardening: Rope Windows and Op Budgets

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
D1, D2, D6, D7, P6, U6 (review §1, §2, §5). All fixes are on the server's
document-access hot paths; no protocol, UI, or package-surface shape changes.

## Objectives

- D1/P6/U6: eliminate full-document `text()` materialization from the completion
  and language-intelligence request paths (`src/server/connection/runtime.rs`);
  one shared `bounded_utf8_window` helper replaces the four hand-rolled
  "clamp to char boundary ± budget" implementations.
- D2: budget the package-facing `documents.open`/reload ops
  (`src/server/ops/documents.rs`) so no unbounded full-text handoff to V8 is possible.
- D6: replace `mode_activation_cache` clear-all eviction with LRU.
- D7: merge `release_single_document_access`'s two lock scopes into one.

## Expected Outcome

- No production call site materializes a whole document for a windowed consumer;
  a multi-MB document open with active completion providers no longer performs
  O(document) allocations per completion request (verifiable by allocation-count
  probe or timing).
- `documents.open` op on an oversized document returns a typed budget error
  (same shape as the analysis route's `analysis.document_too_large`), never a
  multi-MB JSON payload.
- All Linux gates green (`scripts/check.sh` full set) plus the new budget tests.
- No protocol/message changes; no new user-visible configuration surface.

## Tasks

- [ ] Baseline gates and hot-path measurements on the unmodified tree
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full stage set passes on the untouched tree; record per-stage exit codes.
    - Performance: record a pre-change timing/allocation baseline for a completion request and a `documents.open` op against a ≥4 MiB document (e.g. extend `tests/suites/../large_document.rs` fixtures with a one-shot timing probe, removed after recording).
    - Code Quality: no code changes in this task; probe committed only if reusable as a regression test input.
    - Security: none introduced.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §1 (D1, D2), §2 (P6).
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` PF-3 (large-document baseline method).
    - Options Considered:
      - Reuse the 2026-09-14 measurements: stale machine snapshots.
      - Fresh baselines: trustworthy before/after deltas. (Chosen.)
    - Chosen Approach:
      - Run the gate set; capture per-request timings with a temporary probe under the existing large-document fixtures.
    - Files to Create/Edit:
      - None (evidence-recording task).
    - References:
      - `scripts/check.sh`, `tests/large_document.rs` (suite wiring in `tests/suites/`), `src/perf/budgets.rs`.
  - Test Cases to Write:
    - None (evidence-recording task).

- [ ] Add `bounded_utf8_window` helper and rope-backed window construction (U6, D1 enabler)
  - Acceptance Criteria:
    - Functional: a helper on `DocumentState` (or a free function over `&Rope`) returns `(byte_start, byte_end, String)` for a cursor-centered budget, clamping to UTF-8 char boundaries, never materializing more than `budget + 1 line` bytes.
    - Performance: window construction is O(window) in bytes, not O(document); asserted by a test comparing allocations/runtime scaling on a 8 MiB document vs a 64 KiB one.
    - Code Quality: the four duplicated boundary-clamp loops (`src/server/connection/runtime.rs:439–510` ×2, `src/server/connection/documents.rs` analysis path, `src/server/document_analysis.rs`) call the helper; clippy clean under `-D warnings`.
    - Security: helper is `pub(crate)`; no new public API; window budgets come from `src/perf/budgets.rs` constants only.
  - Approach:
    - Documentation Reviewed:
      - `src/server/document.rs` (`bounded_prefix`, `bounded_byte_end`, `clone_rope` — existing rope-slicing precedents).
      - `crop` crate rope API — byte slicing via `Rope::byte_slice` (verify exact API against the vendored version with `cargo tree -i crop` + rustdoc before writing).
      - `src/perf/budgets.rs` (window budget constants).
    - Options Considered:
      - Keep `&str`-based helpers and fix callers one by one: leaves the O(doc) materialization in place.
      - Rope-first helper returning an owned bounded `String`: single choke point, testable scaling. (Chosen.)
    - Chosen Approach:
      - `DocumentState::window_around(cursor: usize, budget: usize) -> (u64, u64, String)` built on rope byte slices; static-prefix reads use a rope slice of `[start..end]` directly.
    - API Notes and Examples:
      ```rust
      // src/server/document.rs
      pub(crate) fn window_around(&self, cursor_byte: usize, budget_bytes: usize)
          -> (u64 /*start*/, u64 /*end*/, String)
      ```
    - Files to Create/Edit:
      - `src/server/document.rs`: add `window_around` (and rope-slice prefix accessor if needed).
      - `src/server/connection/runtime.rs`: `completion_document_window` / `language_intelligence_document_window*` delegate to it.
      - `src/server/document_analysis.rs`, `src/server/connection/documents.rs`: replace local boundary-clamp loops.
    - References:
      - Review U6; `src/server/decorations.rs` (budget-aware slicing precedent).
  - Test Cases to Write:
    - `window_at_document_edges_clamps_to_boundaries`: cursor 0 and cursor == byte_len return valid windows.
    - `window_respects_multibyte_boundaries`: document of CJK/emoji text never splits a scalar.
    - `window_cost_is_independent_of_document_size`: 8 MiB vs 64 KiB doc, same-budget window within a constant factor.

- [ ] Route completion and language-intelligence through rope windows (D1, P6)
  - Acceptance Criteria:
    - Functional: `src/server/connection/runtime.rs` no longer calls `document.text()`; static-provider prefix matching reads a rope slice of `replacement_range` only; provider-match check happens before any window is built.
    - Performance: completion request on a multi-MiB document shows no O(document) allocation (baseline comparison from task 1); JS-provider path builds its window exactly once.
    - Code Quality: no behavior change in completion results (existing completion suites pass unchanged, including `tests/completion_provider.rs`).
    - Security: unchanged authorization checks (`document_for_message`) stay ahead of any text access.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection/runtime.rs:975–1060` (completion arm), `:1138` (LI arm), `static_package_completion_result:513–570`.
      - `src/server/completion.rs` (`CompletionDocumentWindow` contract — window fields `byte_start/byte_end/text`).
    - Options Considered:
      - Materialize-then-slice (status quo): O(doc) per keystroke.
      - Rope window + rope-slice prefix: matches existing window contract, zero protocol change. (Chosen.)
    - Chosen Approach:
      - Static path: `rope.byte_slice(start..end).to_string()` only when a provider matches; window path via task-2 helper.
    - Files to Create/Edit:
      - `src/server/connection/runtime.rs`: both arms.
    - References:
      - Review D1, P6.
  - Test Cases to Write:
    - `static_completion_on_large_document_matches_small_document_results`: same provider set, same result for identical prefix.
    - `language_intelligence_window_budget_honored`: window byte length ≤ budget + boundary slack.

- [ ] Budget the `documents.open`/reload package ops (D2)
  - Acceptance Criteria:
    - Functional: `op_clay_documents_open` / reload return a typed error (e.g. `documents.document_too_large`) when `document.byte_len()` exceeds a new `DOCUMENTS_OP_MAX_DOCUMENT_BYTES` budget; under the budget the op result is byte-identical to today's.
    - Performance: oversized open stops before any rope→String conversion.
    - Code Quality: budget constant lives in `src/perf/budgets.rs`; error string and shape mirror the analysis route's precedent.
    - Security: package-facing ops can no longer pull unbounded document bytes into V8 heap (closes the budget bypass); trust-domain behavior unchanged.
  - Approach:
    - Documentation Reviewed:
      - `src/server/ops/documents.rs:30–60, 120–140`; `src/server/connection/documents.rs:400–410` (`analysis.document_too_large` precedent); `src/perf/budgets.rs`.
      - `.agents/skills/clay-execution/references/packages.md` (authority boundaries — op behavior change stays inside documented op surface).
    - Options Considered:
      - Chunked handoff (client-protocol parity): larger change, no current package need.
      - Size gate + typed error: smallest fix consistent with the analysis route. (Chosen.)
    - Chosen Approach:
      - Check `byte_len()` before locking/serializing; new budget constant defaulting to the analysis cap (256 KiB) unless a documented need exists for larger.
    - Files to Create/Edit:
      - `src/server/ops/documents.rs`, `src/perf/budgets.rs`.
    - References:
      - Review D2; decision log 2026-07-21 (trust domains — op set unchanged).
  - Test Cases to Write:
    - `documents_open_over_budget_returns_typed_error`: oversized doc, op errors, V8 never sees text.
    - `documents_open_under_budget_unchanged`: golden-result test vs current behavior.

- [ ] LRU eviction for `mode_activation_cache` (D6) and single lock scope in `release_single_document_access` (D7)
  - Acceptance Criteria:
    - Functional: cache hit rate survives 64+ distinct keys (oldest evicted, newest retained); `release_single_document_access` locks the document exactly once per call.
    - Performance: repeat-open of the first mode after 64 distinct modes still hits the cache (regression test).
    - Code Quality: no new dependency (manual LRU via generation-stamped entries or `HashMap + VecDeque`, mirroring `SyntaxChunkCache::next_access` pattern); workspace method holds one `MutexGuard`.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/mod.rs:940–965`; `src/server/decorations.rs:305–328` (LRU precedent); `src/server/workspace/mod.rs:1520–1535`.
    - Options Considered:
      - Keep clear-all: measurable re-eval spike on eviction churn.
      - LRU by access stamp: matches decorations cache. (Chosen.)
    - Chosen Approach:
      - Fold D6+D7 into one small task — both are local, independently testable changes.
    - Files to Create/Edit:
      - `src/server/js_runtime/mod.rs`, `src/server/workspace/mod.rs`.
    - References:
      - Review D6, D7.
  - Test Cases to Write:
    - `mode_activation_cache_evicts_oldest_not_all`: 65 keys, first evicted, second-oldest retained.
    - `release_single_document_access_locks_once`: lock-count assertion via test-only hook or code inspection recorded in evidence.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: affected modules identified (completion: `test-plan/09-packages-and-modes.md` or the completion module; large-document: `test-plan/03-files-and-workspace.md`); relevant numbered steps executed on a real Linux build, pass/fail recorded.
    - Performance: large-document completion latency steps (if present) re-run and recorded; add a numbered step asserting completions still appear on a ≥4 MiB document.
    - Code Quality: new steps added with expected results and negative checks; `test-plan/index.md` coverage matrix updated if a module file changes.
    - Security: no steps weakened; failures recorded as defects.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map), `.agents/skills/create-plan/references/clay.md` (manual test plan duty).
    - Options Considered:
      - Automated-only: changes are user-visible (completion latency, package op errors on big files) — manual verification required.
    - Chosen Approach:
      - Run the completion/files modules; add the two new steps above.
    - Files to Create/Edit:
      - `test-plan/03-files-and-workspace.md`, `test-plan/09-packages-and-modes.md` (tentative — exact module per index map), `test-plan/index.md`.
    - References:
      - `scripts/large-document-smoke.sh`.
  - Test Cases to Write:
    - Manual steps as described (not automated here).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: review the phase diff; the only JS-visible change is the `documents.open`/reload op's new typed error on oversized documents — document it in the existing `documents` API Markdown (errors/failure modes section), linked from `docs/index.md`.
    - Performance: none.
    - Code Quality: no new public Rust functions without `pub(crate)` justification; generated registry updated; registry/doc-guard tests pass (`cargo test documentation_coverage`).
    - Security: error string carries no path/content leakage.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`, `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Options Considered:
      - No docs change: op behavior changed for oversized inputs — failure-mode documentation is mandatory.
    - Chosen Approach:
      - Amend the existing documents-API doc's error section; no new API IDs.
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/documents.md` (tentative path — locate via `docs/index.md`), generated registry via `src/bin/update-doc-registry.rs`.
    - References:
      - Decision log 2026-05-08-1509 (JS API facade duty).
  - Test Cases to Write:
    - None beyond existing doc-guard suites.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: wiki pages covering document access / completion pipeline reflect rope-window construction and the op budget; master index links current.
    - Performance: wiki notes the O(window) invariant.
    - Code Quality: pages explain what changed, invariants, source/test paths.
    - Security: notes the closed budget bypass on package ops.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Options Considered:
      - Per-task updates: noisy. Update once after tests pass. (Chosen.)
    - Files to Create/Edit:
      - `docs/wiki/modules/completion-*.md` / document-pipeline pages (locate via `docs/wiki/index.md`), `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
