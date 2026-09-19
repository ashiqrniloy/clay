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

- [x] Baseline gates and hot-path measurements on the unmodified tree
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
  - Outcome (2026-09-19):
    - Evidence: `code-reviews/2026-09-18-plan126-baseline/` (README with number
tables, per-stage logs, probe sources).
    - Gates on HEAD `b04f46b`, clean tree: audit/fmt/check/clippy/bench-compile/
bindings exit 0; `test` exit 101 because of two pre-existing reds unrelated to
this plan — `server::js_runtime::tests::coding_agent_clean_init_one_line_activates_working_defaults`
(parallel lib run only; serial lib run 1389/1389 green) and
`agent_session_isolation::real_daemon_serves_one_session_per_workspace`
(deterministic 30 s `session bound` timeout in the security suite).
`cargo test --all-targets --no-fail-fast` recorded per-target results (the
normal `test` stage's fail-fast masks everything after the lib failure).
    - Baselines (debug build, 12-core host, probe removed after recording):
completion request over real IPC on a 4 MiB document — median 427–569 µs round
trip and **8,417,738 bytes allocated per request (2× document)** vs 160,170
bytes at 64 KiB; `documents.open` op body — **4,210,561-byte JSON payload** and
a gated `text()`+JSON phase of 80–82 ms / 29.4 MB allocated vs 1.27 ms / 459 KB
at 64 KiB. Both paths scale linearly with document size.
    - No in-tree probe code or `[[test]]`/Cargo.toml change remains; probe
sources live only under the evidence directory.

- [x] Add `bounded_utf8_window` helper and rope-backed window construction (U6, D1 enabler)
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
  - Outcome (2026-09-19):
    - Deliverable is split along the plan's two allowed shapes: free
`cursor_window_bounds` / `char_boundary_at_or_before` / `char_boundary_at_or_after`
in `src/server/document.rs` own the bounds, and
`DocumentState::window_around(cursor, budget) -> (u64, u64, String)` owns the
rope-backed materialization (one `String::with_capacity` then the window's rope
chunks, via the new `rope_slice_string` helper shared with `document_text_head`,
`bounded_prefix`, and `document_chunk_message`).
    - Every clamp in the changed surface now routes through those primitives: the
completion and language-intelligence window builders (`connection/runtime.rs`),
`DocumentState`'s rope clamps, `bounded_byte_end`, and the `&str` adapters in
`connection/documents.rs` (`floor_char_boundary`, `ceil_char_boundary`,
`bounded_utf8_prefix`, parse-window clamps). No hand-rolled clamp loop remains in
`runtime.rs`, `connection/documents.rs`, or `document.rs`.
    - Finding: the review's fourth site, `document_analysis.rs`, has no clamp loop
at all — analysis windows arrive pre-built from the two runtime builders — so
there was nothing to route there. Remaining `is_char_boundary` loops elsewhere
(`configuration.rs` prefix clamp, `protocol/agent.rs` text clamp,
`syntax/mod.rs` grammar chunking) are different shapes and were left alone.
    - Budgets: added `COMPLETION_DOCUMENT_WINDOW_BUDGET_BYTES` to `perf/budgets.rs`
and used it in the completion coordinator's window validator, replacing a private
`64 * 1024` literal that had to stay in sync with the builder by hand.
    - Deliberate behavior tightening: the language-intelligence builder previously
ceil-clamped its window end, so a provider could receive `budget + 3` bytes; it now
floor-clamps like the completion builder and never exceeds its budget. Tail
content differs by at most 3 bytes; no test or package contract depended on it.
    - Measurements: a 64 KiB-budget window allocates 65,568 bytes on a 64 KiB
document and 65,600 bytes on an 8 MiB one — +32 bytes for a 128× larger document
(asserted, no timing assertion: µs-scale ratios are flaky). Task 3 supplies the
end-to-end request-path numbers.
    - Checks: `--lib` 1392 passed (serial); runtime 75, protocol 222, presentation
62 passed; audit, fmt, `check --all-targets`, `clippy --all-targets -- -D warnings`,
bench-compile, bindings all exit 0; security stays at its task-1 baseline (151
passed, pre-existing `real_daemon_serves_one_session_per_workspace` red).

- [x] Route completion and language-intelligence through rope windows (D1, P6)
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
  - Outcome (2026-09-19):
    - Both arms no longer touch `document.text()`; `grep '\.text()'` in
`src/server/connection/runtime.rs` is empty. The document lock is now held only for
one rope read: `DocumentState::text_range(replacement range)` on the static path
and `window_around(cursor, budget)` when a JS provider matched.
    - Provider matching happens first and without text: static provider match,
dynamic provider match, and analysis-provider lookup all run before the lock is
taken, and the window is built once (`dynamic_provider.zip(window)`) only for the
provider that will actually be scheduled.
    - `static_package_completion_result` now takes `replacement_text` (the bytes
covered by `request.replacement_range`) instead of the whole document, dropping its
internal `str::get` — the caller's `text_range` applies the same
reversed/out-of-bounds/non-boundary rejections, so the no-match fallback to
`builtin_core` is unchanged.
    - Window builders are rope-backed: `completion_document_window(request,
&DocumentState, package_prefix)` and the two language-intelligence builders take
`&DocumentState`; the LI arm clones the per-document manifest under the behavior
lock and takes the document lock only after releasing it, so the two locks are
never nested (as before).
    - Measured, same probe and method as task 1 (4 runs each; before/after table
appended to `code-reviews/2026-09-18-plan126-baseline/README.md`, raw logs
`logs/probe-completion-request{,-after}.log`):
      - 4 MiB document, completion round trip: **8,417,738 → 31,118 bytes allocated
per request (−99.6 %)**, 427–569 µs → 155.8–166.6 µs median (~3.2× faster).
      - 64 KiB control: 160,170 → 31,118 bytes; the 4 MiB request now allocates
byte-identically to the 64 KiB one (zero size-dependent term) with the same 770
allocation calls, i.e. no O(document) copy remains on the path.
    - Checks: `--lib` 1394 passed serial (2 new tests); runtime 75 (includes
`tests/completion_provider.rs`, unchanged), protocol 222, presentation 62 passed;
audit, fmt, `check --all-targets`, `clippy --all-targets -- -D warnings`,
bench-compile, bindings exit 0; security stays at its task-1 baseline (151 passed,
pre-existing `real_daemon_serves_one_session_per_workspace` red).

- [x] Budget the `documents.open`/reload package ops (D2)
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
  - Outcome (2026-09-19):
    - `DOCUMENTS_OP_MAX_DOCUMENT_BYTES = 256 * 1024` added to `src/perf/budgets.rs`
(public, like the other document budgets), same cap as the analysis route, pinned by
`performance_budgets::chunked_document_security_budgets_are_pinned` and listed in the
`docs/development/performance.md` hard-guard table.
    - `ensure_within_package_document_budget` in `src/server/ops/documents.rs` checks
`DocumentState::byte_len()` and returns `documents.document_too_large: document is N
bytes; the package documents limit is 262144 bytes (open larger files in the Clay
editor)`. It runs in `op_clay_documents_open_document` after the lease and before
metadata/text, and in `op_clay_documents_reload_document` before `.text()` — both
ahead of any rope→String conversion or JSON serialization.
    - Op-scoped by construction: the workspace open path, the client chunked
transfer, and the 256 MiB resident budget are untouched, so a human can still open a
large file and a package can still operate on documents it already holds below the
budget. The reload leg is the real bypass shape (open a small file, let it grow on
disk, reload) and is now gated on the freshly reloaded `byte_len`.
    - Tests: `documents_open_over_budget_returns_typed_error` drives both ops from
`init.js` and asserts `["open:typed:no-text", "reload:typed:no-text"]` — the typed
code appears and the fixture's `SECRETPAYLOAD` marker never reaches the JS error
path. Verified to bite: with the two gate calls removed the same test reports
`["open:accepted", "reload:accepted"]`, i.e. the oversized text was handed to V8.
`documents_open_under_budget_unchanged` pins the golden JSON contract
(`metadata,text` keys; `dirty,documentId,leaseId,path,readOnly,version,workspaceRootId`
values `1:1:false:1:false:1:note.txt`; `hello` text on open and reload).
    - Docs: both `docs/reference/clay-js-api/documents/server-{open,reload}-document.md`
pages gained a package-budget note plus the typed error in their Errors sections.
    - Checks: `--lib` 1396 passed serial (2 new); protocol 222 (includes the budget
pin), runtime 75, presentation 62 passed; audit, fmt, `check --all-targets`,
`clippy --all-targets -- -D warnings`, bench-compile, bindings exit 0; security
stays at its task-1 baseline (151 passed, pre-existing
`real_daemon_serves_one_session_per_workspace` red).

- [x] LRU eviction for `mode_activation_cache` (D6) and single lock scope in `release_single_document_access` (D7)
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
  - Outcome (2026-09-19):
    - D6: `ModeActivationCache` in `src/server/js_runtime/mod.rs` now owns
`HashMap<ModeActivationKey, (u64 last_access, CachedModeActivation)>` plus an
`access_counter`, with `get` (marks most-recently used) and `insert` (evicts only
the `min_by_key(last_access)` entry when at `MODE_ACTIVATION_CACHE_ENTRIES` and the
key is new). Linear scan over <= 64 entries, mirroring `SyntaxChunkCache::next_access`;
no new dependency. `cache_mode_activation` no longer clears the cache, and
`cached_mode_activation` no longer needs `.cloned()` because the lookup is `&mut`.
    - D7: `release_single_document_access` in `src/server/workspace/mod.rs` now
takes exactly one `MutexGuard` per call — `release_access`, `access_holder_count`,
and `byte_len` all run inside one block, and the byte count released is the one read
under that same guard. Previously the document was locked three times and a
concurrent access grant could land between the holder check and the registry
removal. Evidence for "locks exactly once" is code inspection (no test-only lock
hook was added to production code); the behavioural half is covered by
`release_single_document_access_releases_bytes_with_last_holder`.
    - Tests: `mode_activation_cache_evicts_oldest_not_all` drives 65 distinct real
markdown activations through `classify_open_document` (distinct leading content per
key), then asserts the second-oldest key still classifies from cache (evaluation
count unchanged) and the oldest re-evaluates (+1). Verified to bite: with the
eviction body reverted to `entries.clear()` the test fails at "second-oldest key must
not re-evaluate its generated module" (66 vs 65 evaluations), which is the ~15 ms
per-activation re-evaluation spike D6 described. The plan's performance line
("repeat-open of the first mode ... still hits") conflicts with its functional line;
the functional rule is authoritative because the cap is 64 entries, so the 65th
distinct activation evicts the oldest. `release_single_document_access_releases_bytes_with_last_holder`
opens one file for two clients and asserts: releasing one holder returns false with
the entry and 5 resident bytes kept, releasing the last returns true with the entry
gone and resident bytes back to 0.
    - Docs: `docs/development/performance.md` mode-activation-cache row now states
least-recently-used eviction; `docs/wiki/modules/syntax-sessions.md` lists the new
regression test.
    - Checks: `--lib` 1398 passed serial (2 new); protocol 222, runtime 75,
presentation 62 passed; audit, fmt, `check --all-targets`, `clippy --all-targets --
-D warnings`, bench-compile, bindings exit 0; security stays at its task-1 baseline
(151 passed, pre-existing `real_daemon_serves_one_session_per_workspace` red).

- [x] Execute and update the manual test plan (test-plan/)
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
  - Outcome (2026-09-19):
    - Affected modules identified and amended: **03** (large-document open), **04**
(completion), **09** (package documents ops), **11** (performance/feel). New
numbered steps with expected results and negative checks: **F56** (≥4 MiB fixture
opens chunked with bounded accessible text and the analysis-limit status note),
**E39** (completion trigger/accept/dismiss on that document), **P55** (package
`serverOpenDocument`/`serverReloadDocument` over the 256 KiB op budget returns the
typed `documents.document_too_large` diagnostic without gating the client path),
**Q41** (completion latency/allocation on a large document). All four are
referenced exactly once in `docs/development/tauri-react-parity-ledger.json`
(`updated` bumped to 2026-09-19) and the index gained a plan-126 record plus a
coverage-matrix row; `cargo test --test protocol documentation_coverage` and
`manual_smoke` pass.
    - Live execution on a real Linux build (isolated mode-700 root, private socket,
fixture-only config, GNOME Wayland 1920×1200): F56 PASS (4,231,903-byte
`review.rs`, 808,003 words, editor AT-SPI text bounded to 2,759 chars, status note
present); E39 PASS (popup `rust | fn | fn function snippet`, snippet accepted →
`fn name(args) {    }`, `Escape` no-op, `zzzz` + `Ctrl+Space` Empty with no panel);
P55 PASS (server diagnostic `clay server runtime reload failed
[documents.document_too_large]: Document/workspace operation failed server
validation.`, sanitized status line, client kept editing the same file); Q41 PASS
measured (server round trip ~161 µs median, 31,118 bytes per request on 4 MiB —
identical to 64 KiB). Automated companions re-run: `cargo test --test runtime
large_document::` (50 MiB, 2 passed in 2.11 s), `static_completion_on_large_document_matches_small_document_results`,
`documents_open_over_budget_returns_typed_error`, `documents_open_under_budget_unchanged`,
`performance_budgets` pins.
    - **Host capability change recorded:** input synthesis works through the
xdg-desktop-portal remote-desktop keyboard session (computer-use-linux MCP
`press_key`/`type_text`) — typing and chords reached the Clay window and were
verified in the document text — so the live legs above were driven for real rather
than recorded as blocked. `ydotool`/`wtype` remain unusable and the WebKit editor
exposes no AT-SPI `EditableText`, so the AT-SPI `--drive` path still cannot type
into the editor; the index record states both.
    - Test tooling added (outside test-plan/, kept small and reusable):
`scripts/capture-ui-review.sh` gained the `ui-review-large-document` fixture
(≥4 MiB `review.rs` from `tests/fixtures/configuration/ui-review-large-document/init.js`,
restored through `layout.json`) and an `insert` drive action (`InsertText` at
caret/end) whose `EditableText` limitation is documented in the script help;
fixture tables updated in `docs/development/launch-and-gui-smoke.md` and
`docs/wiki/modules/ui-review-harness.md`. Artifacts live in
`test-plan/artifacts/126-access-paths/` (`launch-live.sh` with tree-kill teardown,
`probe.py`, `portal-shot.py`, `op-budget-init.js`, and the three run directories).
    - No step was deleted or weakened; no failure was found that needed a defect
record. Recorded ceilings: no live frame-timing number for completion on a large
document (the AT-SPI probe's tree walk is coarser than the event), and a harness
screenshot is discarded when the Clay window is off-screen or overlapped.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
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
  - Outcome (2026-09-19):
    - **JS-visible surface audit (the only change is the one documented).** Phase
diff reviewed end to end: `runtime/`, `packages/`, and `frontend/` are untouched
(no facade export, binding, or option changed); no op declaration, protocol
message, or DTO was added or renamed; the only JS-visible behavior change is the
typed `documents.document_too_large` failure on the `documents.open`/`reload` ops.
No new public Rust surface either: `DocumentState::window_around`/`text_range`,
`char_boundary_at_or_before`/`_after`, `cursor_window_bounds`, and the budget
constants are `pub(crate)`/`pub const` in the existing budgets module, and
`rope_slice_string`, `ensure_within_package_document_budget`, and
`ModeActivationCache` are private to their module (`git diff` shows no added
`pub fn`).
    - **No new API IDs; existing entries verified.** `docs/reference/clay-js-api/api-inventory.toml`
is unchanged: `documents.serverOpenDocument` and `documents.serverReloadDocument`
already carry the right `documentation_path`, permissions, and security notes, so
the registry needed no new IDs — only the failure-mode prose (the plan's chosen
approach). The generated registry was regenerated to prove it is in sync:
`cargo run --bin update-doc-registry` rewrote
`docs/generated/clay-js-api-registry.json` byte-identically (no diff), and
`tests/clay_js_api_inventory.rs` / `tests/clay_js_doc_registry.rs` pass.
    - **Documentation updated (task 4, verified here).**
`docs/reference/clay-js-api/documents/server-open-document.md` and
`server-reload-document.md` now state the package budget
(`DOCUMENTS_OP_MAX_DOCUMENT_BYTES`, 256 KiB) in the return/async section and name
`documents.document_too_large` in `## Errors` with the "open larger files in the
Clay editor" remedy and the note that the client chunked path is unaffected; both
pages are linked from `docs/index.md` (lines 74–75). The two other runtime-backed
document ops that could hand text to JavaScript (`serverGetDocumentSnapshot`,
`serverGetDocumentLease`) are `status = "planned"` and throw "not implemented", and
the remaining ops return metadata only, so the budget bound holds across the whole
public documents surface — verified by reading the ops (`src/server/ops/documents.rs`
has exactly two `.text()` sites, both immediately behind the gate) rather than
assumed.
    - **Budget made documentable/pinnable.** The plan's Code Quality criterion asked
for the compiled budgets to be documented, so `docs/development/performance.md`'s
deterministic hard-guard table gained rows for the completion provider window
(`COMPLETION_DOCUMENT_WINDOW_BUDGET_BYTES`) and the language-intelligence provider
window (`LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES`), and
`tests/performance_budgets.rs` gained
`plan126_provider_document_window_budgets_are_pinned_and_documented`: it pins both
at 64 KiB, pins their equality (one shared budget, the point of task 2), and asserts
the performance guide names both markers. Negative check run: deleting the new
table row makes that test fail (`performance guide must document provider window
budget marker ...`, exit 101) instead of passing silently.
    - **Security criterion met.** The JS-visible message is
`documents.document_too_large: document is <bytes> bytes; the package documents
limit is 262144 bytes (open larger files in the Clay editor)` — a code, a size, and
a remedy, with no path, no filename, and no document content; the user-facing
diagnostic is further sanitized to `Document/workspace operation failed server
validation.` (confirmed live in the task-6 run and asserted in
`documents_open_over_budget_returns_typed_error`, which checks the exact code
string and the absence of the payload marker for both ops).
    - Verification commands: `cargo run --bin update-doc-registry` (no diff),
`cargo test --test protocol -- --test-threads=1` (223 passed, includes
`clay_js_api_inventory`, `clay_js_doc_registry`, `documentation_coverage`,
`performance_budgets`, `package_loading_docs`), `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`.

- [x] Update or verify the code wiki after implementation
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
  - Outcome (2026-09-19):
    - **Pages updated (one pass, after tests passed, as the plan chose).** Each
addition is labelled "Plan 126 (Document Access Path Hardening)" because the wiki
already uses a bare "Plan 126" for an unrelated sidebar-rail change, so the two
cannot be confused.
      - `docs/wiki/modules/server-document-state.md` — new Source entry
(`src/perf/budgets.rs`), a responsibilities bullet, a How-It-Works section on the
shared `window_around`/`text_range`/`rope_slice_string` helpers and
`char_boundary_at_or_before`/`_after`/`cursor_window_bounds` predicates (including
why the guard exists: `crop` panics out of bounds where `str::is_char_boundary`
returns false), two invariants (O(window) extraction; budget-respecting,
code-point-safe windows), the three helper tests, and Related links to the flow
and the two consumers.
      - `docs/wiki/modules/server-file-workspace.md` — the package documents-op
budget and single-lock release in Responsibilities, the gate detail in How-It-Works
item 18 (which ops return whole-document text, where the gate sits, what the typed
error does and does not contain, which stubs are still `planned`), two invariants
(no unbounded JS handoff + the recorded post-open/read compromise; single-scope
release), and the four tests.
      - `docs/wiki/modules/language-intelligence.md` — the flow diagram and a
How-It-Works paragraph now say the window is a rope window (O(window),
floor-clamped end at the 64 KiB budget, built after cloning the manifest so no
nested document lock), plus an invariant and the budget test.
      - `docs/wiki/modules/completion-snippet-expansion.md` — Source entries, a
paragraph on the reshaped request path (provider match before text access,
replacement-range rope slice for static prefix filtering, exactly one provider
window), and the parity/allocation tests.
      - `docs/wiki/flows/document-chunked-loading.md` — the client-chunked-path vs
package-op-budget distinction (this path stays ungated so multi-MiB opens work),
an invariant, the budget/parity tests, and Related links.
      - `docs/wiki/modules/parse-coordinator.md` — the parse-window prefix now
delegates to the shared boundary predicate; helper tests listed.
      - `docs/wiki/modules/syntax-sessions.md` — the mode-activation cache paragraph
states LRU eviction at the 64-entry cap instead of clear-all.
      - `docs/wiki/index.md` — the seven affected module/flow one-liners now carry
their plan-126 clause, so the master index is current without a new plan-map
section.
    - **Verification.** All new test/file references were resolved against the tree
(eleven test names, each found in the cited file) rather than written from memory;
a link sweep over the edited pages plus `docs/wiki/index.md` found no broken
relative link and no module/flow page missing from the index; the doc-guard suites
that read the wiki pass (`cargo test --test protocol` 223 passed — includes
`documentation_coverage`, `manual_smoke_docs`, `primitives_docs`,
`performance_budgets`; `cargo test --test presentation` 62 passed). No wiki page
claims a new public API, and the budget/bypass wording matches the JS-API pages
that carry the public contract.
    - **Final gate run (unmodified-tree reds unchanged).** audit, fmt, `check
--all-targets`, clippy `-D warnings`, lib (1398 passed), protocol (223), runtime
(75), presentation (62), bench-compile, bindings — all exit 0. The security suite
still fails only on the pre-existing
`agent_session_isolation::real_daemon_serves_one_session_per_workspace`
(deterministic session-bind timeout recorded in task 1), and the parallel-only lib
ordering coupling from task 1 remains the other pre-existing red; both stay in
Further Actions and neither is caused by plan 126. (Note: `cargo test --test editor`
is not a valid target in this tree — the editor guards live in
`tests/editor_performance.rs` under the `runtime` suite; several older docs still
spell the historical `editor_performance_invariants` name, which is a repo-wide
staleness outside this plan's scope.)

## Compromises Made
- The live completion-on-a-large-document leg records presence and acceptance, not
a frame number: the AT-SPI probe's resolution (one tree walk, ~0.9 s warm) is
coarser than the event, so the budget evidence stays the server-side measurement
(~161 µs median round trip, constant allocation).
- `release_single_document_access_locks_once` is evidenced by code inspection plus a
  behavioural holder/byte-accounting test rather than a lock-count assertion: proving
"one guard" needs a test-only counter on `DocumentState`, and the observable
consequence (a concurrent grant between the holder check and the removal) needs
interleaving control, so neither was worth production test scaffolding.
- The mode-activation LRU keeps its "least-recently used" promise only within one
generation: `production_reload` still rebuilds the cache from scratch, which is
deliberate (the registries it keys on are rebuilt too).
- The documents-op budget is checked after the workspace has read the file into its
  resident rope (post-open, pre-serialization), so an oversized `open` can leave the
document registered under the runtime client with no id returned to the caller.
Gating before the read would need a size-limited workspace open API (the prepare
step already stats the file); not needed for the V8-heap bypass this closes, and the
resident rope stays inside the existing 256 MiB budget.
- Task 3 has no in-tree end-to-end test for the JS-provider arm: no test drives
  `handle_completion_request` through a connection with a registered dynamic
  provider, so that branch is covered by compilation, by the shared window
  helper's tests, and by the recorded probe (static path only). Add a connection
  test with a registered provider if the arm grows.
- The "no O(document) allocation" regression is proven by the archived probe and
  by helper-level allocation tests rather than a permanent test: a counting
  allocator across a live IPC round trip in the `runtime` suite is order- and
  thread-sensitive (see `VENT.md` on probe targets), so it stays out-of-tree.
- Task 2 adds the rope window path before its callers move (task 3), so
  `window_around` carries an `expect(dead_code)` under `not(test)`; task 3 must
  delete that attribute when it routes the request paths through the rope.
- `window_cost_is_independent_of_document_size` asserts allocated bytes rather than
  wall time: µs-scale timing ratios on shared hardware are flaky, while the
  allocation delta already separates O(window) from O(document) by 128×.
- Task 1 found two pre-existing gate reds on the untouched tree (`test` stage:
  parallel-only lib ordering coupling; `security` suite: real-daemon
  `session bound` timeout). Both were dispositioned on 2026-09-19: the
  `security` red is fixed in-tree (see Further Actions — the shared temp agent
  data dir plus an unvalidated coding-profile guess), and the parallel-only lib
  red is owned by `plans/135-Agent-Registration-Queue-Test-Isolation.md`. So
  the Expected Outcome's "all Linux gates green" holds for every suite
  (`security` 152/152, `lib` serial 1401/1401, `protocol`/`runtime`/
  `presentation` green) except the parallel `cargo test --lib` ordering flake,
  which is now another plan's task rather than an unowned red.

## Further Actions

Both task-1 items were reviewed and dispositioned on 2026-09-19 (below): the
`security` red is fixed in-tree, the parallel-only lib red moved to plan 135, and
the probe-method note is stale.

- **Resolved — `agent_session_isolation::real_daemon_serves_one_session_per_workspace`
  (the deterministic `security` red).** Root cause chain, found by replaying the
  test with its server log captured (the scratch dir is deleted on drop, so the
  log has to be copied while the test runs):
  1. The agent host's data dir fell back to a **shared `temp_dir()/clay-agent`**
     whenever the server had no explicit configuration root — which is the
     normal desktop launch (`clay auto` → `run_desktop` passes no root) and every
     isolated-HOME test. That one directory held `book.json`,
     `credentials.vault`, `sessions.sqlite`, and `vault.passphrase` for every
     profile and run, so the test loaded a persisted `book.json` whose profile
     was `coding` (written by an earlier run) while the package that registers
     that profile was not loaded in this run.
  2. `ensure_tab_session` therefore sent `session.new(profile="coding")`, and the
     daemon answered `Rpc("Unknown agent: coding")` (the Prism agents registry:
     `Unknown ${label}: ${key}` in `createContributionRegistry`), so no session
     ever bound and the test timed out after 30 s.
  3. Two smaller defects in the same path made the first-launch profile noisy:
     package discovery spawned the manager with `current_dir(store.root)` before
     that directory existed (ENOENT on a fresh HOME), and an empty store's
     `npm list --json` reply (no `dependencies` key) was reported as a discovery
     failure.
  - Fixes: `AgentHostConfig::for_server` resolves the per-user Clay root
    (`~/.clay/agents/coding-agent/data`, the daemon's own homedir() default) when
    no root is given, with a per-process temp root under `cfg(test)` so unit
    tests never touch the developer's profile;
    `PackageService::ensure_store_root` runs before discovery and install;
    `NpmBackend::list_installed` treats a `dependencies`-less reply as an empty
    store; and the coding-surface profile is now adopted only when the daemon
    actually lists it (`AgentHost::profile_available`, checked at both the pane
    mount and the launch command), so a missing profile falls back to the
    daemon's built-in `Chat` instead of a dead pane.
  - Evidence: `cargo test --test security -- --test-threads=1` → **152 passed, 0
    failed** (the red used to time out at 30 s; the test now binds in ~2 s);
    new tests `src/server/agent.rs::tab_state_snapshot_skips_a_profile_the_daemon_does_not_have`
    (asserts the daemon is asked for `Chat`, and that `coding` is never recorded),
    `for_server_without_a_configuration_root_keeps_agent_state_off_the_shared_temp_dir`,
    `packages::manager::tests::npm_backend_list_treats_a_dependencies_less_reply_as_an_empty_store`,
    and `packages::manager::tests::discovery_creates_a_missing_store_root_before_spawning_the_manager`
    (both package tests verified to fail with their fix reverted); the existing
    `tab_state_snapshot_starts_the_session_and_carries_branch_and_environment`
    now has a mock daemon that lists the coding profile.
  - Wiki kept in sync with the behavior change:
    `docs/wiki/modules/clay-agent.md` (data-dir root resolution and why the
    shared temp dir failed), `docs/wiki/modules/agent-protocol.md` (the
    profile-availability guard and its fallback), and
    `docs/wiki/modules/agent-process-manager.md` (data-dir responsibility,
    invariant, and the tests that pin it).
- **Moved — parallel-only lib red (`coding_agent_clean_init_one_line_activates_working_defaults`).**
  Plan 126 does not own it; it is now `plans/135-Agent-Registration-Queue-Test-Isolation.md`
  (baseline task + isolation fix + the sibling order-dependent tests). Diagnosis
  handed over: the process-global `AGENT_HOST_AUTHORITY` (`OnceLock`, first
  install wins, never uninstalled) means any test that drives an agent op spawns
  the daemon on the shared host, and `ensure_running` then drains the host's
  pending-registration queue — so another test's declarations are applied to a
  foreign daemon and never observed by the draining test. Reproduced 3/4 in
  default-thread `cargo test --lib` runs; skipping one sibling test
  (`agent_facade_fails_closed_without_an_attached_host`) makes five declaration
  tests fail instead, showing the coupling is broader than one test.
- **Resolved (stale) — the probe-method note.** Task 2's
  `window_cost_is_independent_of_document_size` already uses the task-1 method:
  a `MeasuringAllocator` (`GlobalAlloc`) with an `allocation_counter::allocated_bytes`
  seam and a 64 KiB/8 MiB scaling pair (`src/server/document.rs`), so no separate
  harness was built and nothing remains to reuse.
