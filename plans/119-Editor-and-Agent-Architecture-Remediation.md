# 119 — Editor and Agent Architecture Remediation

Source: `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` (commit
`2864b75`). Scope = that review's Recommended Sequence (items 1–7), with SC-1
executed per the user-approved approach of **2026-09-14: ts-rs codegen from the
DTO layer, not the protocol layer** — `src/protocol/*` (rkyv) is untouched.

## Objectives

- Close every re-verified finding from the 2026-09-14 review: P2-3 dead sandbox,
  version drift, P1-1 eager large-document open, P1-2 long-line position budget,
  P1-3 MCP connect/call timeout conflation, P2-1 unbounded dirty-buffer reads,
  P2-2 non-atomic new-file writes.
- SC-2: extract the connection loop's per-family delivery policy into testable
  helpers (`Delivery` replay enum), loop becomes a thin router.
- SC-1: single-definition webview contract — TS types generated from the DTO
  layer via ts-rs; hand-maintained `types.ts` mirrors deleted; `src/protocol`
  and the DTO projection boundary unchanged in shape and authority.
- SC-6: workspace-scoped agent sessions — server registry keyed by workspace
  root, agent tools resolve workspace from session, frontend agent state scoped
  to the tab runtime.
- SC-3/SC-4: decompose the agent actor and the coding-agent panel; fold the PF-2
  O(n²) transcript scan fix into the panel split.
- Preserve the review's keep-list: budgets centralization, trust boundaries,
  rkyv wire, atomic-save path, DTO projection boundary, checkpoint primitives.

## Expected Outcome

- All Linux gates green (`scripts/check.sh`, `ci.yml`): cargo fmt/check/clippy
  root + clay-desktop, frontend typecheck + suite, agent suite.
- Opening a multi-MiB document delivers a bounded head chunk before any full
  read; 1 MiB single-line position conversion under 15 ms with the existing
  budget test passing (not rebaselined).
- clay-agent MCP servers connect regardless of a small per-call `timeoutMs`;
  slow tool calls still time out.
- `frontend/src/bridge/generated/` is the only TS definition of the webview
  contract; `types.ts` gone (or reduced to branded hand types + re-exports);
  a staleness check in `scripts/check.sh` fails CI on un-regenerated bindings.
- Two tabs over different workspace roots hold independent agent sessions;
  agent tool writes land in the session's workspace, verified by test.
- `handle_connection_loop` reduced to a thin select router; replay-vs-drop
  policy per stream expressed in one enum with unit tests.
- `agent.rs` split into focused modules; `CodingAgentPanel.tsx` split into
  Composer/TranscriptList/ApprovalStrip/InspectorTabs with identical rendered
  output.
- Decision-log entries exist for SC-1 and SC-6.

Out of scope (recorded, not silently dropped): PF-1 lsp-shared
`VersionedDocument` incremental port — latent only, capped at 256 KiB by
`DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` on the analysis route; SC-5 beyond what
SC-1's generated envelope gives; 2026-09-11 review items not re-verified here
(P1-4 js_runtime flake, P1-5 eslint, P3 junk file) — they predate this plan and
remain tracked in that review.

## Tasks

- [x] Baseline gates and performance baselines on the unmodified tree
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` (or the `ci.yml` equivalent set) passes on the untouched tree; record exit codes.
    - Performance: record the current 1 MiB single-line position-map timing (`frontend/src/editor/position-map.test.ts` P1-2 case, ~29.9 ms known) and `scripts/large-document-smoke.sh` timings as pre-change baselines for comparison in tasks below.
    - Code Quality: no code changes in this task.
    - Security: none introduced.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` (PF/SC findings).
      - `code-reviews/2026-09-11-comprehensive-code-review.md` (P1/P2 evidence).
    - Options Considered:
      - Skip baseline, compare against review numbers only: cheaper but review numbers are one-machine snapshots.
      - Record fresh baselines: makes the perf tasks' before/after deltas trustworthy. (Chosen.)
    - Chosen Approach:
      - Run the full gate set plus the two smoke scripts; store outputs under `code-reviews/2026-09-14-editor-and-agent-architecture-review.md`-adjacent evidence paths or the task evidence log.
    - Files to Create/Edit:
      - None.
    - References:
      - `scripts/check.sh`, `.github/workflows/ci.yml`, `scripts/large-document-smoke.sh`, `scripts/editor-performance-smoke.sh`.
  - Test Cases to Write:
    - None (evidence-recording task).
  - Evidence (2026-09-14):
    - Full record: `code-reviews/2026-09-14-plan119-baseline/README.md` + `logs/`.
    - Environment: run-time HEAD `2864b75` (branch `redesign/uiux`) with 240 modified / 7 staged / 735 untracked in-flight plan 118 work; that content was committed mid-session as `1088ad0` ("UI Migration complete", branch `review/arch`). Committing does not modify files, so the baseline maps to the content of commit `1088ad0`; Node v24.19.0, rustc 1.96.1.
    - Gates 11/13 exit 0 (frontend format/build/budget, agent-test, package-smoke, cargo audit/fmt/check/clippy/test-all/bench-compile). Candidate path: `scripts/check.sh full` was not run as one stage — per-stage runs recorded instead so every exit code is captured.
    - `frontend-lint` exit 1 (pre-existing, plan-118 territory): 12 errors / 4 warnings, including a real `rules-of-hooks` violation at `frontend/src/shell/WorkspacePanes.tsx:62`; the rest is the 2026-09-11 review's P1-5 class (test-file non-null assertions, one unused var).
    - `frontend-test` exit 1 is accounting-only: 45 files / 391 tests passed; the single unhandled rejection originates in `src/test/shell.test.tsx` (`invoke` undefined) — the pre-existing flake noted in the review.
    - Position baseline: 1 MiB single line = **1.531 ms/conversion median** (5 runs) vs the 15 ms budget; `chunks: 1, nodes: 1` confirms the structural long-line collapse persists; the review's 29.9 ms figure is stale. Probe preserved at `code-reviews/2026-09-14-plan119-baseline/position-baseline-probe.ts.txt`.
    - Large-document baselines: chunked open/edit/save/reload roundtrip test 2.04 s; oversize/binary refusal 0.4 s. `scripts/large-document-smoke.sh` is a manual GUI checklist with no timing output — kept for the manual test-plan task.
    - Backlog re-verified: P2-3 sandbox + binary still present (module referenced only from `src/server/mod.rs`); P2-2 direct `tokio::fs::write` still at `src/server/agent_documents.rs:304`.
    - No plan-119 code changes; the temporary measurement probe was removed from the tree.

- [x] Delete the dead runtime sandbox (P2-3)
  - Acceptance Criteria:
    - Functional: `RuntimeSandboxSupervisor` module and the `clay-runtime-sandbox` binary are removed; no references remain (grep clean); `cargo check --all-targets` green.
    - Performance: build time and binary set shrink; nothing else changes.
    - Code Quality: dead security surface no longer ships; `src/bin/` auto-build wiring updated.
    - Security: removes an unreviveable unsafe path (no heap limit, ignored payload cap) from the shipped set.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P2-3 (0 in-edges, unsafe if revived).
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` backlog.
    - Options Considered:
      - Wire heap limit + payload cap and keep it: keeps speculative process-isolation milestone alive — YAGNI; in-process `DomainRuntime` superseded it.
      - Delete. (Chosen.)
    - Chosen Approach:
      - `git rm` `src/server/runtime_sandbox.rs` and the `src/bin/clay-runtime-sandbox*` source; remove module registration and any build references.
    - Files to Create/Edit:
      - `src/server/runtime_sandbox.rs`: delete.
      - `src/bin/` (sandbox binary): delete.
      - `src/server/mod.rs`: remove module line.
    - References:
      - In-process `DomainRuntime` (128 MB heap, 5 s timeout) is the live mechanism.
  - Test Cases to Write:
    - Existing suites must stay green; no new test for deleted code.
  - Evidence (2026-09-14):
    - Deleted: `src/server/runtime_sandbox.rs` (281 lines), `src/bin/clay-runtime-sandbox.rs` (88 lines), `tests/runtime_sandbox_harness.rs` (168 lines) — 556 deletions / 32 insertions across 11 files.
    - Plan correction: P2-3's "0 in-edges" held for production only. The module was `#[doc(hidden)] pub`, so the compiler never flagged it, and it had a live harness test wired into `tests/suites/security.rs` that spawned the binary through `CARGO_BIN_EXE_clay-runtime-sandbox`. Deleting the module therefore also meant removing that suite wiring and the binary's test-only role.
    - Reference cleanup (guarded surfaces): `src/server/mod.rs` registration; wiki page `docs/wiki/modules/persistent-runtime-hardening.md` plus its `docs/wiki/index.md` description (the wiki-navigation guard requires current-state pages to name only existing paths); parity-ledger `current_tests` for `packages.modes.settings.themes` and `security.trust-domains`; `docs/reference/clay-js-api/inventory.md`.
    - Design intent kept: `docs/design/persistent-runtime-sandbox.md` remains as an unbuilt migration gate (decision logs reference it), with a Status section and past-tense harness sections — the removed harness's evidence claims are marked removed rather than silently dropped.
    - Verification (all green): `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets` exit 0, 0 failures; doc-guard suite (`cargo test --test protocol documentation_coverage`) 11/11 passed, including the wiki-navigation and ledger well-formedness guards.
    - Grep clean for `src/`, `tests/`, `scripts/`, `Cargo.toml`; remaining mentions are explicit removal notes plus history (`plans/`, `decision-logs/`, `code-reviews/`, `docs/wiki/archive/`, a plan-112 test artifact).
    - Binary set shrink: `src/bin/` is back to `clay-server` + `update-doc-registry`; a stale 103 MB `target/debug/clay-runtime-sandbox` artifact plus its `deps/` copies were removed by hand (cargo does not prune) — 17 artifacts to 0.
    - Graph refreshed: `graft build` (10531 nodes); both stale nodes (`graft/src/server/runtime_sandbox.md`, `graft/tests/runtime_sandbox_harness.md`) are gone.

- [x] Fix clay-agent package.json description drift
  - Acceptance Criteria:
    - Functional: `clay-agent/package.json` description matches the pinned Prism family (0.5.5).
    - Performance: n/a.
    - Code Quality: description no longer contradicts dependencies.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P3.
    - Options Considered:
      - One-line edit vs leaving it: one-line edit. (Chosen.)
    - Chosen Approach:
      - Update the description string only; do not touch dependency pins.
    - Files to Create/Edit:
      - `clay-agent/package.json`: description `0.5.4` → `0.5.5`.
    - References:
      - `decision-logs/2026-09-07-2149-prism-0.5.1-clay-agent-family-pins.md` (pin discipline).
  - Test Cases to Write:
    - `npm test` in clay-agent stays green.
  - Evidence (2026-09-14):
    - `clay-agent/package.json` description `Prism 0.5.4` → `Prism 0.5.5`; dependency pins untouched.
    - Drift survey (same class, whole repo): only this description was stale. All seven `@arnilo/prism*` deps resolve to a single pin family (`0.5.5`); no other `package.json` description names a Prism version; the `Cargo.lock` `0.5.4` hit is the unrelated `event-listener-strategy` crate; `roadmap.md`'s 0.5.4 mentions are historical bump-log lines that already end at 0.5.5.
    - Left unfixed (out of task scope, reported): `src/server/ops/agent.rs:360` still labels a comment "Prism 0.5.4: number | null, no product HARD" — the run-limit semantics it describes were not changed by 0.5.5 (per-frame byte caps), but the version label is now behind the pin.
    - No guard catches this drift today (`scripts/package-smoke.sh` checks release-artifact versions only, never description text), so the string will rot again on the next Prism bump unless a guard or a version-free description lands.
    - Verification: `clay-agent/package.json` parses, description matches every Prism pin, and `npm test` in `clay-agent` exits 0 — 146 tests, 145 pass, 1 skipped, 0 fail.

- [x] P1-1: route large-document open through the chunked protocol
  - Acceptance Criteria:
    - Functional: `workspace::read_file_streamed` no longer eagerly streams the whole file; large opens deliver a bounded head chunk first and continue through the existing `DocumentChunk` progressive path (`onDocumentChunk` in `frontend/src/editor/sync/session.ts`), consistent with `decision-logs/2026-08-25-1253-chunked-document-loading.md`.
    - Performance: `scripts/large-document-smoke.sh` shows head-chunk-first latency for an 8 MiB file within the review's open-latency budget; total transfer unchanged.
    - Code Quality: no second chunking implementation — reuse the existing document-chunk protocol end to end.
    - Security: chunk caps remain server-enforced; no new client trust.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §PF-3.
      - `decision-logs/2026-08-25-1253-chunked-document-loading.md`.
      - `docs/development/editor-performance-review-2026-08-26.md`.
    - Options Considered:
      - Parallel/turbo read: more code, no latency win while head-chunk-first is missing.
      - Reuse existing chunk path for open. (Chosen.)
    - Chosen Approach:
      - Convert the eager path in `src/server/workspace/mod.rs` to emit the head chunk then stream remainder chunks over the existing protocol; verify frontend progressive path handles it without change.
    - Files to Create/Edit:
      - `src/server/workspace/mod.rs`: `read_file_streamed` → chunked emission.
      - `tests/performance_budgets.rs`: add a source-level assertion that the open path references the chunk budget (existing pattern in `workspace_open_path_stays_streamed_and_head_bounded`).
    - References:
      - `src/server/connection/documents.rs` (chunk delivery), `frontend/src/editor/sync/session.ts` (`onDocumentChunk`).
  - Test Cases to Write:
    - Budget test: open of an oversized file emits head first; no single frame exceeds the chunk budget.
  - Scope Correction (2026-09-14, before implementation):
    - The planned approach ("convert the eager path to emit the head then stream the remainder") does not survive the evidence. The 2026-09-11 review's own root cause states the eager full read is intentional — the resident rope is required for editing/save/analysis and "the budget is what's wrong"; the 2026-08-25 decision log + plan 098 already implement the pull-based chunk protocol end to end (`DocumentTextHead` + `DocumentChunkRequest` → rope slices; frontend `requestChunk`/`onDocumentChunk`), and plan 098 measured `open->head` 287–302 ms for 52 MiB (debug) as an accepted result.
    - Sending the head before the resident rope exists is not a re-route, it is a new partial-document state: background fill, edit/save/analysis gating on a loaded watermark, mid-fill failure teardown, and frontend retry semantics. Deferred as an explicit option below rather than smuggled into a remediation task.
    - Two evidence-backed defects remained: the flat 500 ms `open->head` assertion is a debug-profile test carrying an unstated release-profile assumption (plan 098 recorded a 538 ms host-variance trip; the 2026-09-11 review measured 1.0–1.4 s on a loaded host), and `read_file_streamed` allocated a fresh `combined` Vec per 64 KiB read (~800 allocations for 50 MiB) — the hotspot the 2026-09-11 review named. Both fixed here.
  - Evidence (2026-09-14):
    - `src/server/workspace/mod.rs`: `read_file_streamed` now uses one reused scratch buffer (`FILE_READ_BUFFER_BYTES + UTF8_CARRY_BYTES`) with `copy_within` carrying the incomplete UTF-8 scalar inside it — zero per-chunk allocations, unchanged 64 KiB read window, unchanged UTF-8/NUL-sniff semantics. New `UTF8_CARRY_BYTES` const.
    - Measured on this host, 50 MiB fixture, debug, `--nocapture`: `open->head` 270/270/280 ms before → 250/255/259 ms after (3-way concurrent rerun 251–257 ms); `open->full` 578–590 ms → 533–563 ms; `save->ack` 417–421 ms → 391–406 ms.
    - `tests/large_document.rs`: flat 500 ms head budget replaced by `open_latency_budget(bytes) = max(500 ms, bytes / 25 MiB/s)` → 2 s for this fixture, ~8× the measured debug cost and above the worst recorded loaded-host number; the derived budget is printed every run. The 5 s full-load budget is unchanged.
    - `tests/performance_budgets.rs`: source guard extended (helper must contain `copy_within`, must not contain `let mut combined`) so the per-chunk allocation regression cannot return silently.
    - `src/server/workspace/tests.rs`: new `streamed_read_carries_utf8_scalars_across_read_boundaries` — a 4-byte scalar split 1/2/3 bytes before the 64 KiB boundary must reassemble byte-identically, and a scalar truncated at EOF still refuses with `InvalidUtf8`. The suite had no carry-boundary coverage before.
    - Verification (final tree): `cargo fmt --check` 0, `cargo check --all-targets` 0, `cargo clippy --all-targets -- -D warnings` 0, `cargo test --all-targets` exit 0 with 0 failures; `large_document` suite green with `open->head` 262 ms against the 2 s budget.
    - Deferred (needs a decision, would amend the 2026-08-25 chunked-loading decision): true head-first server loading so first paint no longer waits for the full read — ~250 ms → ~5 ms (debug, 50 MiB) at the cost of a new document loading state. `scripts/large-document-smoke.sh` is a manual GUI checklist and was not re-run; `tests/large_document.rs` is its automated counterpart (manual re-run stays with the plan's manual test-plan task).

- [x] P1-2: segment long lines in the frontend position index
  - Acceptance Criteria:
    - Functional: the treap chunker in `frontend/src/editor/position-map.ts` splits chunks on byte length (≈128 KiB) in addition to the 64-line rule; conversions on a 1 MiB single-line document are sub-chunk scans.
    - Performance: the existing `position-map.test.ts` 1 MiB single-line budget test (<15 ms) passes for real — the test is the guard and is NOT rebaselined, weakened, or deleted.
    - Code Quality: segmentation is a chunker rule change only; index-vs-fresh-rebuild consistency test still passes.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P1-2 (root cause: 64-line chunking collapses a 1 MiB line into one chunk).
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` backlog.
    - Options Considered:
      - Re-baseline budget and track segmentation later: keeps a documented red ceiling alive.
      - Byte-length segmentation now. (Chosen — the test comment already names it as the fix.)
    - Chosen Approach:
      - Add a max-chunk-bytes rule to the chunker; split on whichever bound hits first; keep conversion math unchanged.
    - Files to Create/Edit:
      - `frontend/src/editor/position-map.ts`: chunker rule.
      - `frontend/src/editor/position-map.test.ts`: no weakening; add a multi-boundary consistency case if coverage is thin.
    - References:
      - Review keep-list: incremental `BytePositionIndex` is also the model for PF-1 later.
    - Baseline Note (2026-09-14, task 1): median 1.531 ms/conversion at 1 MiB single line vs the 15 ms budget — the review's 29.9 ms figure is stale; `chunks: 1, nodes: 1` shows the structural long-line collapse. This task is now a structural/headroom fix (segmentation so the chunk count scales), with the budget test as the pass guard, not a red-budget rescue.
  - Test Cases to Write:
    - 1 MiB single-line conversion under 15 ms (existing test, now passing).
    - Random-edit consistency across segmented long lines (extend existing).
  - Scope Correction (2026-09-14, before implementation):
    - The planned mechanism (byte-length chunk splitting in the treap chunker) does not bound anything. A 1 MiB single line is one *entry* in one leaf, and a leaf's scan is already capped at `CHUNK_LINES` (64) entries — the quadratic-looking cost was never the chunker, it was the intra-line scan in `position-map.ts` walking up to 1 MiB of characters per conversion (`DocumentChunk`-style byte splitting of chunks would have left that untouched). The chunker also lives in `position-index.ts`, not `position-map.ts`.
    - What the evidence supports instead: a line longer than one 8 KiB scan block records its block starts (UTF-16 + UTF-8 offsets, both on scalar boundaries), the scan resumes at the containing block, and only that block's text is read. Chunk byte-splitting was rejected separately: it would make leaf membership depend on region history (incremental regions regroup) without changing leaf scan cost.
  - Evidence (2026-09-14):
    - `frontend/src/editor/position-index.ts`: `SEGMENT_UNITS = 8 * 1024`; per-entry `LineSegments` block tables (`char`/`byte` arrays) built by `pushLine` in the same pass that measured the line width, kept out of the tree shape so line-number semantics, splits/joins, and O(log lines) edits are unchanged; `locateLine16`/`locateLine8` return `scan16`/`scan8`/`scanEnd16`; `lineScanText` reads the whole line only when it fits one block, otherwise the block via `sliceString`; `LineSource` gains an optional `sliceString`.
    - `frontend/src/editor/position-map.ts`: `utf16ToUtf8Indexed` scans from `scan16` (index loop, no `for...of` copy), `utf8ToUtf16Batch` re-anchors its resumable cursor at each block start (forward-only, safe because inputs are sorted) for both the single and dense paths; `utf8Width` exported for the table builder.
    - Measured (1 MiB single line, same probe method as the baseline, this host): median **1.531 ms → 0.0104 ms** per conversion (147×); line-end worst case **0.0199 ms** (the reported 29.9 ms failure mode); `scan16: 1040384` with `windowUnits: 8192` at a deep offset, i.e. one block, not the line. Index install 1 MiB line: **3.22 ms** (was 6.4 ms for the width pass alone before the ASCII fast path in the long-line walk was added). Dense batch: 4000 offsets in 6.33 ms. Probe + log: `code-reviews/2026-09-14-plan119-baseline/position-after-probe.ts.txt`, `logs/position-after.log`; README section "Position index — 1 MiB single line: after P1-2".
    - Tests (`frontend/src/editor/position-map.test.ts`, 13 tests in the file):
      - The existing 1 MiB budget test is untouched in substance — same offsets, same `< 15` ms ceiling, no rebaseline (only its stale "O(line) by design" comment updated).
      - New `segments a long line so one conversion scans a single block`: mixed 1/2/4-byte scalars, asserts `scan16 > 0` and `window ≤ SEGMENT_UNITS + 1` for deep offsets, scalar-boundary agreement (`utf16ToUtf8(text, scan16) === scan8`), per-offset equality with the linear reference every ~1 KiB, and batch agreement.
      - New `never slices a surrogate pair in half at a block boundary`: a pair straddling the block limit (block ends at 8193, so a flat 8192 slice would under-count it by one byte). Verified to fail on the flat slice and pass on the recorded block end.
      - New `keeps long-line segmentation consistent across edits`: two long lines, in-block insert, mid-block split, rejoin, a new long line at the front then dropped, and an astral insert near the end; after each edit, incremental vs fresh rebuild vs linear reference across both offset spaces, including reversed batch input (cursor re-anchor).
    - Verification (final tree): `tsc -b` 0; `eslint .` **identical to baseline** (16 problems, 12 errors, 4 warnings — none in the edited files); `prettier --check` 0; `vitest run` 45 files / 394 tests passed, exit 1 only for the pre-existing `shell.test.tsx` unhandled IPC rejection recorded in the baseline; `vite build` 0; `check:budget` 0 (shell 173.3/180 kB, total **399.0/400 kB** — 0.4 kB above the baseline, headroom nearly gone); `cargo fmt --check` 0 and `cargo test --test protocol` 216 passed (docs/parity guards unaffected).
  - Follow-ups (measured, deliberately not done here):
    - The short-line path still uses `utf8Length` (`for...of`): 6.4 ms/MiB versus 1.6 ms/MiB for the `charCodeAt` walk now in the long-line path, i.e. ~3-4× on every index install (a 50 MiB document pays ~300 ms). One shared measure helper would fix it; it is a build-cost task (P1-1 adjacent), not a conversion task.
    - Bundle budget: total gzip is at 399.0/400 kB, so the next editor feature has ~1 kB of headroom before the ceiling must move.

- [x] P1-3: split MCP connect timeout from call timeout in clay-agent
  - Acceptance Criteria:
    - Functional: `clay-agent/src/mcp.ts` keeps `timeoutMs` as the tool-call ceiling and gives connect/handshake its own floor (`max(timeoutMs, 5000)` or a distinct Prism connect timeout if exposed); servers whose process needs ~1 s to spawn + `tools/list` connect even with a small `timeoutMs`.
    - Performance: slow tool calls still time out at `timeoutMs`; connect completes within the floor.
    - Code Quality: both semantics asserted in tests; no log-line-only failure mode.
    - Security: no new authority; allow-list validation untouched.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P1-3 (Prism 0.5.5 bridge passes `limits.callTimeoutMs` as connect pagination timeout).
      - `decision-logs/2026-09-09-1341-mcp-stdio-config-sources-no-approval-gate.md`.
    - Options Considered:
      - Plumb distinct Prism connect timeout: exact if the API exists — verify against the pinned `@arnilo/prism-mcp` 0.5.5 bridge at execution time.
      - Floor of `max(timeoutMs, 5 s)` at our boundary: works regardless of Prism API shape. (Chosen default.)
    - Chosen Approach:
      - Enforce the floor in `clay-agent/src/mcp.ts`; update `mcp-v2.test.ts` to assert both semantics (slow tool call times out AND the server still connects).
    - Files to Create/Edit:
      - `clay-agent/src/mcp.ts`: connect budget.
      - `clay-agent/src/mcp-v2.test.ts`: dual-semantics assertions.
    - References:
      - Fixture server boot ~1 s; sibling passing tests ~1000 ms each (review evidence).
    - Baseline Note (2026-09-14, task 1): the test relocated to `clay-agent/src/__tests__/mcp-v2.test.ts` and is green — it already asserts both semantics (server bridges with `timeoutMs: 200`; slow call times out at 200 ms). Prism's bridge still applies `limits.callTimeoutMs` to connect pagination (`@arnilo/prism-mcp` `dist/bridge.js:81-82`). Execution starts by re-verifying whether a small `timeoutMs` can still hide a paginating server; if not reproducible, close the task with that evidence instead of adding a speculative floor.
  - Test Cases to Write:
    - `timeoutMs: 200` + slow-boot server → connects; slow tool call → times out.
  - Investigation First (2026-09-14):
    - The report is reproducible, not stale — but only with an explicit boot delay. The plain fixture boots in well under 200 ms on this host (which is why the baseline flake probe looked green), so the failure was reproduced by giving the fixture a knob: `CLAY_MCP_BOOT_DELAY_MS` now delays `serveStdio` attach, and with `timeoutMs: 200` + 1 s delay the connect failed deterministically: `[mcp] server "slowboot" failed to connect: Request timed out`.
    - Prism 0.5.5 exposes **one** timeout knob, so the plan's preferred option (a distinct connect/handshake budget) does not exist at the pinned version: `callTimeoutMs` is validated in `@arnilo/prism-mcp/dist/limits.js:54` and used for the `initialize` handshake (`dist/capabilities.js:18`, `dist/bridge.js:27-31`), every `tools/list` page (`dist/bridge.js:81-82`), the per-call timer (`dist/bridge.js:310-311`), and the SDK call's own `timeout`/`maxTotalTimeout` (`dist/bridge.js:320-321`). Nothing on the public surface (`ConnectMcpToolsOptions`, `McpToolBridge`) separates the phases, so a dependency bump or an upstream option would be required for that route — out of this plan's scope (all `@arnilo/prism*` pins are version-locked together).
    - The floor alone cannot satisfy the acceptance criteria either: Prism's single knob means "floor the knob" would also floor every call and break `timeoutMs` as the call ceiling (and the existing 200 ms call-timeout test).
    - Available seam found while investigating: Prism's `callRemoteTool` forwards the execution context's `AbortSignal` into the SDK call (`dist/bridge.js:296-322`), aborts it on our signal, cancels the request, and maps the rejection to a normal error `ToolResult` (`dist/bridge.js:362-368`). So the call ceiling can be enforced by Clay without a second Prism knob.
  - Evidence (2026-09-14):
    - `clay-agent/src/mcp.ts`: `CONNECT_FLOOR_MS = 5000` with the coupling rationale; the bridge now receives `callTimeoutMs: Math.max(timeoutMs, CONNECT_FLOOR_MS)`; `withCallDeadline(tool, timeoutMs)` wraps each bridged tool when `timeoutMs` is below the floor, deriving its own `AbortController` from the context signal (already-aborted and later-abort cases both forwarded) and aborting at `timeoutMs` with `MCP tool call timed out after ${timeoutMs}ms` — the same wording Prism uses, so the model sees the configured value, not the floor; `boundedTools()` applies the wrapper per server (tools for `timeoutMs >= floor` and for the absent-`timeoutMs` (Prism 60 s) case are untouched, so no behaviour change where none is needed); `bridges` now carries each bridge with its entry's `timeoutMs` for wrapping and `close()`.
    - `clay-agent/src/__tests__/mcp-fixture-server.mjs`: `CLAY_MCP_BOOT_DELAY_MS` delays transport attach (the boot window a call-shaped timeout used to swallow).
    - Tests (`clay-agent/src/__tests__/mcp-v2.test.ts`):
      - New `timeoutMs bounds calls, not the connect handshake: a slow-booting server still connects`: 1 s boot + `timeoutMs: 200` → outcome reports `connected: true` with tools bridged, and the same connection's `sleep` tool still times out at 200 ms — both semantics in one flow (the plan's test case). Failed before the fix (`Request timed out`, server hidden), passes after.
      - New `a run cancellation still aborts an in-flight bounded call`: a pre-aborted execution-context signal yields an error result carrying the run's reason — the wrapper forwards cancellations instead of replacing them (regression guard on the new branch).
      - Existing tests unchanged and green: `timeoutMs` absent → default connect (60 s) still works; 200 ms slow-call test still asserts `/timed out after 200ms/`.
    - Verification: `npx tsc -p tsconfig.json` 0; `npm test --prefix clay-agent` → 148 tests, 147 pass, 1 skipped, 0 fail (baseline: 146 total); `cargo test --test protocol` 216 passed (docs/parity guards, including the wiki page edited below).
    - Failure visibility (acceptance criterion "no log-line-only failure mode") re-verified end to end: connect failures stay in the daemon outcome `error`, ride `environment.list` → `mcpServers`, are bounded in Rust (`src/server/agent.rs:431-455`, 48/96 chars) and rendered by the panel as `hidden: <reason>` (`frontend/src/coding-agent/CodingAgentPanel.tsx:392`, asserted in `CodingAgentPanel.test.tsx:1295`).
    - Docs: `docs/wiki/modules/clay-agent.md` states the new split (call ceiling + connect floor + why) and lists "connect floor vs call ceiling" in the daemon's test coverage.
  - Follow-up: if a future `@arnilo/prism-mcp` exposes a distinct connect/handshake budget, pass it and delete `withCallDeadline` — the wrapper exists only because 0.5.5 has one knob.

- [x] P2-1: cap the dirty-buffer path in `document_read`
  - Acceptance Criteria:
    - Functional: agent `documentRead` on an open (dirty) document returns at most `MAX_READ_BYTES` with a `truncated: true` signal — same cap as the disk path.
    - Performance: no unbounded payload can reach the daemon stdio bridge regardless of resident document size.
    - Code Quality: the reverse-RPC response carries the truncation flag; agent-side byte caps in `document-ops.ts` are defense-in-depth, not the only bound.
    - Security: bounded cross-process payload; no new capability.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P2-1.
    - Options Considered:
      - Rely on agent-side caps: single point of failure at the wrong boundary.
      - Server-side cap + flag. (Chosen.)
    - Chosen Approach:
      - Apply `MAX_READ_BYTES` to the dirty-buffer return in `src/server/agent_documents.rs`; add the flag the response shape already has room for.
    - Files to Create/Edit:
      - `src/server/agent_documents.rs`: dirty-path cap + truncation flag.
      - `src/perf/budgets.rs`: only if the flag plumbing needs a named constant (prefer reusing `MAX_READ_BYTES`).
    - References:
      - Review keep-list: budgets.rs discipline — new ceilings belong there with rationale.
  - Test Cases to Write:
    - Open a document > cap, dirty it beyond cap, `documentRead` returns exactly cap bytes + truncated flag.
  - Evidence (2026-09-14):
    - `src/server/agent_documents.rs::document_read`: the open-document branch now resolves `cap = min(params.maxBytes ?? MAX_READ_BYTES, MAX_READ_BYTES)`, reports `truncated` and `totalBytes`, and — when cut — returns `document.bounded_prefix(cap)` instead of `document.text()`. That reuses the rope-slice helper the chunked-open path already uses (`src/server/document.rs:217`, scalar-safe via `bounded_byte_end`, covered by `bounded_prefix_floors_to_utf8_boundary`), so a 256 MiB resident buffer is never materialized just to be cut. The disk branch reports the same `truncated: false` / `totalBytes` shape (`read_contained_bytes` still refuses oversize files rather than cutting them, so the two paths keep their existing cap semantics).
    - `clay-agent/src/document-ops.ts`: `DocumentReadResult` gains `truncated?`/`totalBytes?`, and `readDocument` — the single seam every read route uses (`readFile`, `readText`, `edit.readFile`) — fails loudly on `truncated: true` with `Document is <totalBytes> bytes, exceeds the <cap> byte read cap`. Fail-loud matters: `readText` pages the returned text, so presenting a prefix would read as EOF to the model. The pre-existing agent-side byte ceilings stay as defense in depth. A crate/daemon version skew without the flag simply keeps the old behaviour (no truncation happened server-side then).
    - Rust tests (`src/server/agent_documents.rs`, 7 in the module, + a shared `open_dirty_document` helper for the open-and-dirty setup):
      - `document_read_caps_the_dirty_buffer_and_flags_the_cut`: 4096-byte dirty buffer read with `maxBytes: 1024` → exactly 1024 bytes, `truncated: true`, `totalBytes: 4096`, `open: true`, `dirty: true`; the same document at `maxBytes: 8192` → full text, `truncated: false` (proves the flag tracks the cut, not the document).
      - `document_read_never_splits_a_scalar_at_the_dirty_buffer_cap`: 1023 ASCII bytes then a 4-byte scalar with a 1024-byte cap → 1023 bytes of valid UTF-8, still flagged.
      - `document_read_clamps_the_requested_cap_to_its_ceiling`: 8 MiB + 1 KiB dirty buffer requested with `maxBytes: 512 MiB` → exactly `MAX_READ_BYTES` returned, flagged — the agent cannot raise its own bound.
      - Existing disk-path test extended with the `truncated: false` / `totalBytes` shape.
    - Agent test (`clay-agent/src/__tests__/coding-tools.test.ts`, mock server now echoes the two fields): `read fails loudly when the server cut the buffer at its read cap` drives the real `read` tool against a `truncated` response and asserts an error result naming the real size with none of the cut text in the result. Verified to fail without the `document-ops.ts` guard (teeth check).
    - Verification: `cargo fmt --check` clean; `cargo clippy --all-targets` clean; `cargo test --all-targets` → 1354 lib + 9 + 61 + 216 + 75 + 149, all pass (the 216 are the docs/parity guards); `npx tsc -p tsconfig.json` 0; `npm test --prefix clay-agent` 149 tests (148 pass, 1 skipped; baseline 146).
    - Docs: `docs/wiki/modules/clay-agent.md` now states the read cap on both paths, the rope-slice behaviour, and the agent's loud failure.
  - Follow-up found while scoping (same bridge, different feature — not fixed here): `src/server/agent_checkpoints.rs::checkpoint` still copies every open document's full text with `document.text()` into an unbounded in-memory `HashMap` per (session, entry). It is not a cross-process payload the way `document_read` was (`checkpoint.capture` returns counts; content stays server-side), but it is unbounded server memory, one copy per checkpoint, and silently truncating it would corrupt restore semantics — so it needs its own policy. Cheap lead: the store could hold `document.clone_rope()` (O(1) shared chunks) and restore through the existing `replace_rope_from_storage`, instead of allocating a String per document per checkpoint.

- [x] P2-2: make new-file `document_write` atomic
  - Acceptance Criteria:
    - Functional: new-file creation routes through `contained_new_file_path` + `atomic_write_file` (the pair the workspace open/save paths already use); no direct `tokio::fs::write` remains in the agent write path.
    - Performance: one extra rename+fsync on file creation only.
    - Code Quality: reuses the tested atomic primitive; failure-path temp cleanup inherited.
    - Security: crash mid-write can no longer leave a partial file the agent already reported as success.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P2-2.
    - Options Considered:
      - fsync-only: still leaves partial-file window on crash.
      - Full atomic path. (Chosen.)
    - Chosen Approach:
      - Replace the direct write for the new-file branch with the existing atomic pair.
    - Files to Create/Edit:
      - `src/server/agent_documents.rs`: new-file branch.
    - References:
      - Review keep-list: "Atomic saves done right" — extend the same primitive.
  - Test Cases to Write:
    - New-file write failure injection (unwritable dir) leaves no partial file and reports failure.
  - Evidence (2026-09-14):
    - `src/server/workspace/mod.rs`: added `pub(crate) async fn atomic_create_file(target, bytes) -> io::Result<()>`, a thin wrapper over the existing `atomic_write_file(.., expected: None)` that maps `AtomicSaveError` back to `io::Error` — the save primitive keeps its private error type, and the call site keeps its existing `write failed: {error}` phrasing. Everything the review asked for comes from the primitive the open/save path already uses: exclusive unpredictable temp in the target's directory (`create_exclusive_temp`, `create_new(true)`, mode `0o600`), `fsync` before the replace, atomic `rename`, temp removal on every failure path, and the Unix no-write-bits rejection. `expected: None` is deliberate and documented in the wrapper: a path that did not exist has no stable identity to revalidate, so a file created by someone else between the containment check and the rename is replaced without a change check (pre-atomic `fs::write` clobbered it too — this at least never leaves it torn). Directory `fsync` stays the documented `ponytail:` skip inside `atomic_write_chunks`, so the rename itself is not power-loss durable; that is consistent with every other save in the tree.
    - `src/server/agent_documents.rs::document_write`: the new-file branch calls `atomic_create_file(&canonical, content.as_bytes())` instead of `tokio::fs::write`; nothing else changed (containment still from `contained_new_file_path`, parents from `create_dir_all`).
    - Side effect worth knowing: new files now inherit the temp's `0o600` rather than a umask-derived `0o644`. That is the primitive's documented "safe default for a brand-new file" and matches the save path, but it is an observable permission change for agent-created files.
    - Review claim corrected: `contained_new_file_path` and `atomic_write_file` were *not* already a pair in use — `contained_new_file_path` had exactly three callers (the agent reverse methods) and `atomic_write_file` was only reached by `save_io`, which requires a pre-existing `TargetIdentity` and therefore never creates a file. This wrapper is the first wiring of the two together.
    - Swept for siblings: `document_write` was the only non-atomic *content* write reachable from a user or the agent. Frontend file creation reaches disk through the existing save path, the JS API has no file-creation surface, and the remaining `fs::write`/`std::fs::write` hits in `src/` are test modules, generated artifacts (`src/docs/registry.rs`), or temp-then-rename stores (`src/server/launcher.rs`).
    - Test (`src/server/agent_documents.rs`): `document_write_creates_new_files_through_the_atomic_path` drives the real reverse RPC to a path with two missing parent directories and asserts `saved: true` / `open: false` / `version: 1`, the exact content on disk, no surviving `.clay-save-*` temp in the target directory, and (Unix) mode `0o600`. The mode assertion is what gives the test teeth: verified to fail against the old `tokio::fs::write` path, which produced `0o644`. The original planned failure-injection case is already covered one level down by `atomic_write_file_rename_failure_returns_error_and_cleans_temp` (`src/server/workspace/tests.rs`), which asserts exactly "error returned, temp removed, target untouched" — the wrapper adds no failure path of its own.
    - Verification: `cargo fmt --check` clean; `cargo clippy --all-targets` clean; `cargo test --all-targets` → 1355 lib (one more than before this task) + 9 + 61 + 216 docs/parity + 75 + 149 integration, all pass; docs guards re-run after the wiki edits.
    - Docs: `docs/wiki/modules/server-file-workspace.md` (atomic primitive now also backs creation, with the `0o600` note) and `docs/wiki/modules/clay-agent.md` (`document.write` split into existing-file CAS save vs new-file `atomic_create_file`).

- [x] SC-2: extract the connection loop's delivery policy
  - Acceptance Criteria:
    - Functional: byte-for-byte wire behavior unchanged; `handle_connection_loop` (`src/server/connection/mod.rs` L496–L1701) reduced to a thin select router delegating to per-family delivery helpers.
    - Performance: no extra copies or hops on the hot loop; the replay-vs-drop policy per stream is expressed in one `Delivery` enum (`Advice` → drop on lag; `State` → replay latest) with unit tests.
    - Code Quality: the replay policy becomes testable without a live connection; extend the `documents.rs` `deliver_parse_update` pattern to the remaining families.
    - Security: connection-scoped session isolation (`ServerMenuSessions`) preserved.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-2.
      - `src/server/connection/documents.rs` (the in-repo extraction pattern to copy).
    - Options Considered:
      - Actor-per-family rewrite: bigger blast radius, no behavior gain.
      - Delivery-helper extraction. (Chosen.)
    - Chosen Approach:
      - Introduce `Delivery` enum; write one helper per broadcast family (typography, editor commands, caret, layout, prefs, tab registry, runtime generation, diagnostics); each helper owns its lag/close policy; the loop body becomes one-liners.
    - Files to Create/Edit:
      - `src/server/connection/mod.rs`: loop slimming.
      - `src/server/connection/documents.rs` or a new `src/server/connection/delivery.rs`: helpers + `Delivery`.
    - References:
      - Existing behavior map: Lagged typography/prefs/tab-registry/caret/layout → replay latest; editor commands/parse → drop; runtime generation → close menu session then replay; broadcast `Closed` → return.
  - Test Cases to Write:
    - Unit: lagged `State` stream replays latest; lagged `Advice` stream drops; `Closed` propagates.
  - Evidence (2026-09-14):
    - New `src/server/connection/delivery.rs`: `Delivery` (`State` = replay the family's current value on lag, `Advice` = drop the backlog), `Outcome<T>` (`Value`/`Replay`/`Drop`/`Closed`) classified by the pure `Delivery::on(...)`, `Flow` (`Continue`/`Close`) for the loop, and one helper per lane: `typography`, `editor_command`, `caret_style`, `editor_layout`, `shell_preferences`, `tab_registry`, `runtime_generation`, `result` (bounded mpsc result lanes, generic over the frame mapping), `analysis` (the decorations/diagnostics/diagnostic projection), `agent`. The runtime state lanes share one private `state_lane` helper, so the shape exists once and only the framing (message variant + replay source) is per family.
    - `src/server/connection/mod.rs`: `mod delivery;` plus a nine-line local `lane!` macro (`Flow::Continue => continue`, `Flow::Close => return Ok(())`) that makes each select arm a one-liner. Fourteen hand-rolled lane arms became ten delegations; the select block is now 84 lines, down from 240 (`git show HEAD` measurement), and `handle_connection_loop` is 1061 lines, down from 1206.
    - Behavior preserved, deliberately including the special cases the inline arms encoded:
      - `runtime_generation`: close the generation-bound menu session first, then send the *latest complete* snapshot for this client (the event carries only the id; a lagged receiver must not replay intermediate generations), and send nothing when no snapshot is committed.
      - `agent`: a lagged or closed broadcast never ends the connection (the subscription outlives a daemon restart) and an event over the frame budget becomes an `agent.frame_too_large` diagnostic written best-effort, never a codec error that kills the view. The `async { match agent_rx.as_mut() { .. } }` await shape (no agent runtime ⇒ pending) stays in the loop.
      - Bounded mpsc result lanes: `None` (closed lane) keeps serving — the client is bounded by lane capacity and its drop counter, not by teardown.
      - The parse-update lane stays family-owned in `documents.rs` (`deliver_parse_update`), the in-repo pattern this task extended.
    - Scope note: the extraction covers *delivery*. The loop's remaining bulk is pre-loop subscription setup and the client-message dispatch match, whose arms already delegate to the plan-090 family modules (`documents`/`menus`/`runtime`/`tabs`/`workspace`); shrinking that further is a dispatch task, not a delivery one, and was left alone here.
    - Tests (`delivery.rs`, no live connection required — the plan's requirement): `lag_policy_is_replay_for_state_and_drop_for_advice` (all six policy/outcome pairs), `lagged_state_lane_writes_the_current_value` (a real `TabRegistry` fixture, `Err(Lagged)` in, a decodable `ServerMessage::TabRegistry` frame out, equal to the registry's current snapshot), `lagged_advice_lane_writes_nothing`, `closed_state_lane_ends_the_connection`, and `agent_lane_bounds_oversized_events_and_survives_a_closed_daemon` (forwards a normal event, writes nothing on `Closed`, and turns an 8 KiB event over a 512-byte frame budget into the diagnostic — that guard had no test before). Teeth check: flipping `Delivery::State`'s lag arm to `Drop` fails the policy test and the replay test.
    - Verification: `cargo fmt --check` clean; `cargo clippy --all-targets` clean (no new warnings); `cargo test --lib connection::` → 88 pass, including `live_typography_update_reaches_connection_once` and the runtime-generation/menu-cancel and parse-isolation scenario tests; `cargo test --all-targets` → 1360 lib (5 more than before this task) + 9 + 61 + 216 docs/parity + 75 + 149 integration, all pass.
    - Docs: `docs/wiki/modules/server-ipc-skeleton.md` gains a `delivery.rs` row in the Plan 090 module table plus a "Lane policy (Plan 119 SC-2)" paragraph naming which lanes replay, which drop, and the two agent-lane exceptions.

- [x] SC-1 decision log: ts-rs codegen from the DTO layer
  - Acceptance Criteria:
    - Functional: decision-log entry created under `decision-logs/` (create-decision-log skill format) recording: webview contract source of truth = `src-tauri/src/bridge/dto.rs`; TS generated via ts-rs 12; `src/protocol` (rkyv) untouched; DTO projection boundary preserved as a trust feature, not duplication.
    - Performance: n/a (decision task).
    - Code Quality: rejected alternatives recorded (specta/tauri-specta, JSON-schema+quicktype, conformance-test-only, Vite-time codegen) with reasons.
    - Security: notes that the webview contract stays narrowed by the DTO projections.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-1, §SC-5.
      - Session discussion 2026-09-14 (user approved ts-rs-from-DTO-layer; explicitly not touching `src/protocol`).
      - ts-rs docs via ctx7 `/aleph-alpha/ts-rs` (serde-compat parses `rename_all`, `tag`, `content`, `skip`, `flatten`; export via `cargo test`).
    - Options Considered: as recorded above.
    - Chosen Approach:
      - Log the decision before implementation (this plan is the user's approval of direction; the entry records it).
    - Files to Create/Edit:
      - `decision-logs/2026-09-14-HHMM-ts-rs-generated-webview-contract-from-dto-layer.md` (timestamp at execution).
    - References:
      - Per planning-checklist: after logging, add the reusable guidance line to the smallest relevant `clay-execution` reference (likely `references/protocol-perf.md` or `references/ui.md` → Client Architecture).
  - Test Cases to Write:
    - None (decision task).
  - Evidence (2026-09-14):
    - Log: `decision-logs/2026-09-14-1602-ts-rs-generated-webview-contract-from-dto-layer.md` (status `approved`, `explicitly_approved_by_user: true`).
    - Recorded decision: one hand-written webview contract = the serde-JSON DTO layer (`src-tauri/src/bridge/dto.rs` + `bridge/errors.rs`); TS generated with ts-rs 12 into `frontend/src/bridge/generated/`, checked in, staleness-guarded; `src/protocol` (rkyv) untouched and core crates carry no codegen derives; the DTO layer stays a *projection* (narrowing is a trust feature with its proving test `runtime_snapshot_parses_package_components_and_hides_raw_theme_overrides`).
    - Rejected alternatives recorded: status quo hand mirrors (third definition, already drifted — `theme/types.ts` relaxed `editorStyles` to optional vs required `ThemeSnapshotDto`); specta/tauri-specta (needs `#[derive(Type)]` + runtime `Types` registry and targets Tauri's invoke/command + event model, which the frame-stream `BridgeEnvelope` is not); JSON-Schema + quicktype (two-step, weaker serde-attr fidelity); conformance-tests-only (same work, deletes nothing — and `src-tauri/tests/dto_roundtrips.rs`'s 15 tests already exist without preventing mirror drift); Vite/build-time codegen (frontend build would need a Rust toolchain, and a build-time artifact hides the contract diff from review); protocol-layer derives (explicitly rejected by the user — would leak internal wire shape and delete the projection's narrowing).
    - ts-rs facts cited from ctx7 `/aleph-alpha/ts-rs` at write time: `serde-compat` default (reads `rename_all`/`rename_all_fields`/`tag`/`content`/`skip`/`flatten`, so the generated shape cannot diverge from the wire JSON without the wire changing), serde `tag`+`content` → TS discriminated unions (matches `BridgeEnvelope`'s `kind`/`data` today), `#[ts(export)]` regenerates during `cargo test`, `export_to`/`Config` set the output dir. Version 12.0.1; `grep -rn 'ts-rs' Cargo.toml src-tauri/Cargo.toml Cargo.lock` empty (greenfield, dev-only addition).
    - Inventory evidence recorded: `dto.rs` 910 lines / 20 `pub` structs+enums + `BridgeEnvelope` (:785); `bridge/errors.rs` `BridgeErrorCode`/`BridgeError`; hand TS mirrors `bridge/types.ts` 239 + `theme/types.ts` 142 + `icons/types.ts` 25 + `sdui/types.ts` 227 = 633 lines.
    - Coverage boundary recorded (so the implementation cannot silently widen scope): `BridgeEnvelope::Event` carries `clay::client::ClientConnectionEvent` (`src/client/mod.rs:882`) 1:1 and DTO fields reference core-crate projection types (e.g. `clay::shell::theme::ThemeTokenValueDto`); those crates gain no derives, so coverage is either a DTO-layer projection in `dto.rs` or a TS-side narrowing — never a hand copy.
    - Reusable guidance (per the planning checklist): one line added to the Client Architecture section of `.agents/skills/clay-execution/references/ui.md` naming the single contract definition, the generated-output path, the staleness guard, and the no-derives-in-core-crates rule; the new log is cited in that section's Sources line.
    - No code changes (decision task); the implementation is the next task.

- [x] SC-1: generate the webview TS contract from the DTO layer
  - Acceptance Criteria:
    - Functional: `frontend/src/bridge/generated/` holds TS types generated from the DTO structs; `types.ts` hand-mirrors deleted as covered (leaf DTOs first, `BridgeEnvelope` last); remaining hand file keeps only branded types (`DocumentId`, `MenuSessionId`) and re-exports; frontend typecheck green against generated types.
    - Performance: codegen runs via `cargo test -p clay-desktop` (dev-dependency); frontend builds stay Rust-free.
    - Code Quality: ts-rs 12 as dev-dependency of clay-desktop; `#[derive(TS)] #[ts(export)]` on DTO structs — existing `#[serde(rename_all = "camelCase")]` attrs drive the TS shape via serde-compat (verified against current docs at execution); export fn lives in the existing `runtime_projection_tests` module with `Config` output dir pointing at `frontend/src/bridge/generated/`.
    - Security: no contract widening — generated types mirror exactly what dto.rs serializes today; a shape mismatch surfaced by the migration is a drift bug to fix, not paper over.
  - Approach:
    - Documentation Reviewed:
      - ts-rs 12.0.1 (crates.io, confirmed current): serde-compat default; export via `cargo test`; `Config::new().with_out_dir(...)`; `#[ts(tag)]` for discriminated unions.
      - Verify the exact v12 export/Config API shape against ctx7 `/aleph-alpha/ts-rs` docs during execution — the API moved major versions recently.
      - `src-tauri/src/bridge/dto.rs` (DTO inventory: ~20 structs, ~5 enums incl. tagged `BridgeEnvelope`).
      - `frontend/src/bridge/types.ts` (mirror inventory + branded types to preserve).
    - Options Considered:
      - specta/tauri-specta: geared to Tauri invoke/commands; Clay's frame protocol is the wrong shape.
      - JSON-schema + quicktype: two-step, weaker serde-attr fidelity.
      - Conformance tests without codegen: same upfront work, deletes nothing, keeps three definitions.
      - ts-rs from DTO layer. (Chosen.)
    - Chosen Approach:
      - Incremental migration: leaves → envelope; `types.ts` becomes a re-export barrel, deleted when empty; each step's proof is `npm run check` (typecheck) — mismatch = real drift bug found.
      - Drift guard: `scripts/check.sh` regenerates bindings then fails on `git diff --exit-code -- frontend/src/bridge/generated`.
    - Files to Create/Edit:
      - `src-tauri/Cargo.toml`: `ts-rs = "12"` dev-dependency.
      - `src-tauri/src/bridge/dto.rs`: derives + export test.
      - `frontend/src/bridge/generated/`: generated output (checked in).
      - `frontend/src/bridge/types.ts`: shrink to branded types + barrel, then delete.
      - `scripts/check.sh`: staleness check.
    - References:
      - SC-1 decision-log entry (previous task).
  - Test Cases to Write:
    - `cargo test -p clay-desktop` regenerates bindings; CI diff check fails on stale bindings.
    - Frontend typecheck consumes generated `BridgeEnvelope` with no `any` escape hatches.
  - Evidence (2026-09-14):
    - Generation: `frontend/src/bridge/generated/bridge.ts` (609 lines) + `frontend/src/bridge/generated/serde_json/JsonValue.ts`, written by `runtime_projection_tests::export_webview_contract_bindings` in `src-tauri/src/bridge/dto.rs` with `Config::new().with_large_int("number").with_out_dir("../frontend/src/bridge/generated")`. Every contract type exports to the same file, so ts-rs merges them into one module with same-file imports elided. Run: `cargo test -p clay-desktop --features ts-bindings --lib export_webview_contract_bindings`.
    - Derives: 20 `dto.rs` types + `BridgeError`/`BridgeErrorCode`, plus 71 core-crate serde-JSON types reachable from those roots (`src/protocol/{mod,sdui,runtime,decorations,diagnostics,language_intelligence}.rs`, `src/shell/theme.rs`), each `#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]` + `ts(export_to = "bridge.ts")` — +142 lines total, zero effect on normal builds.
    - Plan deviation (recorded): ts-rs is an **optional dependency + `ts-bindings` feature** in both crates, not a clay-desktop dev-dependency. The derive sits on lib types, which a dev-dependency cannot name outside `cfg(test)`; the feature keeps the default graph clean — `cargo tree -p clay-desktop | grep ts-rs` and `cargo tree -p clay | grep ts-rs` are 0 (3 with the feature).
    - Coverage boundary (measured, not assumed): the first sweep annotated `ClientConnectionEvent` and immediately pulled `ServerMessage`, `AgentServerMessage`, `CompletionResultSet`, `ViewportRenderPatch`, `SelectionQueryResult`, … into the contract. `BridgeEnvelope::Event`/`Routed` are therefore `#[ts(skip)]`-ed with the reason in the Rust doc comment, and `frontend/src/bridge/types.ts` composes `BridgeEnvelope = generated ∪ { kind: "event"; data: ShellEvent } | RoutedEvent`, keeping `ShellEvent` as the shell's narrowing with its opaque catch-all. This is the decision log's rule (no widening) and the acceptance wording "as covered".
    - Frontend migration: `bridge/types.ts` (239 lines) is now branded ids + the event narrowing + composed envelope + explicit re-exports of the 98 generated names (envelope excluded); `theme/types.ts` 142→26 lines (aliases + the frontend's own text-variant constants), `icons/types.ts` 25→8, `sdui/types.ts` wire types aliased with `Omit<Dto, "component"> & { component: PackageComponentNode }` narrowings for surfaces/panels/overlays/snapshot plus the hand-authored `PackageComponentNode`/`PackageAction`/`SduiTreeUpdate` models, `editor/extensions/types.ts` 238→161 (generated decoration/diagnostic shapes aliased; folding, viewport-patch, completion, language-intelligence and keybinding payloads stay hand as event narrowing). New `sdui/state.ts::installPackageUi` is the single `JsonValue → PackageComponentNode` narrowing at the bridge boundary; `state/document-store.ts` re-exports the generated `DocumentAccess`; `state/connection-store.ts` uses `ThemeSnapshotDto`.
    - Drift the generation exposed (fixed in fixtures/consumers, not papered over): `DocumentAccess` was hand-typed `{ readOnly: null }` but the wire is the string `"readOnly"`; `ThemeSnapshot.editorStyles` was optional "for old fixtures" while Rust always serializes it; `BootstrapDto.tabId`/`performanceProfile` were optional but are required on the wire; `UiChoiceOption.displayName`/`IconPathDto.opacity` nullability; `PackageUiSnapshotDto.surfaces` is required; `SduiListItem.icon`/label `icon` are required-nullable; `ComponentRecipeDto.padding`/`gap`/`innerHighlight`; `LigaturePolicy`'s five fields. Test fixtures now use `frontend/src/test/contract-fixtures.ts` builders (`behaviorManifestFixture`, `themeSnapshotFixture`, `fontProfileFixture`, `typographySnapshotFixture`, `emptyEditorRules`) so future contract growth does not re-break them.
    - Fidelity fix found by inspection: ts-rs's serde-compat does not carry `skip_serializing_if` into struct-variant fields, so `BridgeEnvelope::Disconnected`'s `client_id`/`tab_id` carry explicit `#[ts(optional)]` (generated output has `?`, matching the wire, which omits them).
    - Guard: `scripts/check-bindings.sh` regenerates and fails on any modified **or untracked** file under `frontend/src/bridge/generated` (so un-regenerated output cannot land); it is the last stage of `scripts/check.sh full`, which is the `ci.yml` step. Teeth check: appending a comment to `bridge.ts` makes it exit 1 with the regeneration hint. It intentionally fails while the generated files are uncommitted.
    - Prettier: `frontend/.prettierignore` excludes `src/bridge/generated/` (drift-guarded instead of hand-formatted); every hand-edited frontend file is `prettier --check` clean.
    - Docs-as-code: `documentation_coverage.rs`'s forbidden-authority-marker scan now excludes `bridge/generated` (generated declarations quote Rust doc comments, e.g. `rkyv`, with no frontend authority) while still scanning every hand-written bridge source; the full `cargo test --test protocol` suite (216) passes.
    - Docs: `docs/wiki/modules/desktop-typed-bridge.md` gains a "Generated webview contract (Plan 119 SC-1)" section (codegen command, feature, excluded variants, large-int policy) plus source rows; `react-client-bridge.md` and `flows/document-chunked-loading.md` now point at generated shapes instead of claiming mirrors.
    - Verification: `npx tsc -b` 0 errors; `npm run build` ok; `check:budget` total gzip 399.3 kB / 400; vitest 45 files / 394 tests pass (the two pre-existing `shell.test.tsx` `invoke`-undefined unhandled rejections remain, unchanged in cause); prettier clean; eslint 11 errors / 4 warnings vs the baseline 12/4 (the removed one was in a rewritten test — no new lint issues); `cargo fmt --check`; `cargo check --all-targets`; `cargo clippy --all-targets` clean **and** `cargo clippy -p clay-desktop --features ts-bindings --all-targets` clean; `cargo test --all-targets` → 1360 + 9 + 61 + 216 + 75 + 149 pass; `cargo test -p clay-desktop --features ts-bindings` → lib 31 + 2 + 1 + 4 + 15 pass.
    - Scope note: the generated contract is wider than the old hand mirrors where the frontend only consumed part of a shape (e.g. the full `BehaviorManifest`, `DocumentRuntimeRenderState`, decoration/diagnostic sets). That is the intended direction — the frontend now narrows explicitly (`Omit`/`Extract` over generated types) instead of restating fields, and every consumer still typechecks.

- [x] SC-6 decision log: workspace-scoped agent sessions
  - Acceptance Criteria:
    - Functional: decision-log entry recording the ownership model change: server agent registry keyed by workspace root; tools resolve workspace from session id; frontend agent state scoped to the tab runtime (no module-global session store).
    - Performance: n/a (decision task).
    - Code Quality: migration/compat story recorded (existing single-workspace users keep one session; per-workspace books and checkpoints already keyed by session).
    - Security: tool writes constrained to the session's workspace — an explicit tightening of today's bootstrap-workspace binding.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-6.
      - `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md` (a tab is one workspace + one agent).
      - `decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md` (session-keyed checkpoint primitives).
    - Options Considered:
      - Keep process-global agent + route by active tab: leaves wrong-workspace writes and cross-tab state bleed.
      - Workspace-scoped sessions. (Chosen — matches the approved tab model.)
    - Chosen Approach:
      - Log before implementation; the entry names the invariant: one workspace root → one agent session → tools bound to that root.
    - Files to Create/Edit:
      - `decision-logs/2026-09-14-HHMM-workspace-scoped-agent-sessions.md` (timestamp at execution).
    - References:
      - Per planning-checklist: update the smallest relevant `clay-execution` reference if guidance is reusable.
  - Test Cases to Write:
    - None (decision task).
  - Evidence (2026-09-14):
    - Log: `decision-logs/2026-09-14-1705-workspace-scoped-agent-sessions.md` (status `approved`, `explicitly_approved_by_user: true`).
    - Recorded decision: one `(workspace root, agent type)` → one agent session, with a tab *looking up* that session instead of owning one; every agent tool call resolves its workspace from the **session** (session id → recorded root → registered root id, fail-closed when unresolved); the frontend keeps one `AgentSessionModule` + transport per tab, routed by session id, never a process singleton. Invariant in one line: one workspace root + agent type → one agent session → tools bound to that root.
    - Research findings now part of the record (they sharpen the two implementation tasks):
      - The server binds agent tool authority to the **bootstrap** workspace (`src/server/mod.rs:721` passes `bootstrap_state.workspace` to `document_reverse_handler`; module doc `src/server/agent_documents.rs:12`) and `resolve_root` falls back to `workspace.first_root_id()` (lowest id, `src/server/workspace/mod.rs:1293`) because the daemon's document ops send no session/root identity (`clay-agent/src/document-ops.ts:57`, `:152`, `:173`, `:181`). Effect: a session bound to any other root has its reads/writes containment-checked against root 1.
      - The MCP allow-list merges the **launch** root's `.mcp.json` into every session regardless of its own root (`build_mcp_allow_list(config_root, launch_root, agent)`, `src/server/agent_mcp_config.rs:150`; roots installed at `src/server/mod.rs:734`). Same invariant violation.
      - The daemon resume path recreates the live session with `workspaceRoot: process.cwd()` and never reads the recorded session-search key back (`clay-agent/src/host.ts:2537`; key written at `:1477`), so a resumed session's tool cwd, acceptance roots and wiki/graft binding fall back to the daemon launch directory. Same bug class in the daemon; it belongs to the SC-6 (server) task's scope (server + daemon are one wire).
      - Per-session primitives are already correct: `session_for_root` invalidates on root *or* agent change (`src/server/agent.rs:473`), `book.session_root` maps session → root (`:1488`), `book.json` stores only selections per `(agent, root)` and no session ids (`:3700`, `:3761`), and checkpoints are session-keyed. So this is a re-key, not a re-model.
      - Frontend bleed mechanism confirmed: one relay stream + one store (`frontend/src/agent/events.ts:71`, `frontend/src/agent/state.ts:526`), every agent view binding the *active pane's* tab-stamped sender into it (`frontend/src/coding-agent/CodingAgentPanel.tsx:495`–`:500`, `frontend/src/shell/WorkspacePanes.tsx:91`) while hidden tab slots stay mounted — last mount wins, every other tab's prompt carries its identity. Per-session routing needs no protocol change: relay events already carry `clientId`/`tabId` (`src-tauri/src/bridge/agent.rs:23`) and run events carry the owning session as AG-UI `threadId` (`src/server/agent_agui.rs:263`), snapshots as `state.sessionId`. The server agent lane is an unfiltered process-wide broadcast (`src/server/connection/delivery.rs:308`) — server-side per-connection filtering was considered and explicitly deferred (decision alternative 6), so filtering is the store's job and a tab's own tab-stamped STATE answer is the authority for which session it owns.
    - Compat/migration recorded: single-workspace users keep one session; `book.json` format unchanged; two tabs on one root converge on that workspace's session (older daemon sessions stay on disk and remain resumable) — one reconciliation per folder the first time it is opened twice.
    - Security recorded as a tightening: the `first_root_id()` fallback is deleted (unresolvable session root → diagnostic, never a silent bind), containment checks anchor to the session's root, and cross-folder MCP grants stop leaking from the launch root.
    - Reusable guidance: `.agents/skills/clay-execution/references/packages.md` → Agent Host gained three lines (workspace is the ownership unit; tool authority resolves from the session and never from launch/bootstrap/first-root/active-tab fallbacks; frontend agent state is per tab, routed by session id) and cites the new log in its Sources line.
    - Documentation Reviewed for the entry: review §SC-6; plan 119 SC-6; `2026-09-11-2331` (one tab = one workspace + one agent; several workspaces means several tabs); `2026-08-30-2200` (session-keyed checkpoints/branches); the code paths listed above.
    - No code changes (decision task). No wiki edit here: the wiki's agent page carries no bootstrap-binding claim to correct, and the ownership statement lands with the SC-6 (server) docs step.

- [x] SC-6 (server): workspace-keyed agent registry and tool resolution
  - Acceptance Criteria:
    - Functional: the server agent registry keys sessions by workspace root; agent tool execution resolves the workspace from the session instead of the bootstrap workspace; existing single-workspace flows behave identically.
    - Performance: registry lookup O(1) per session; no per-request workspace scanning.
    - Code Quality: no copy of workspace state into agent state — resolution goes through the existing workspace authority (`src/server/workspace/mod.rs`).
    - Security: a session's tools cannot address files outside its workspace root (existing containment checks now anchored to the right root).
    - Scope added by the SC-6 decision log (2026-09-14-1705) — three same-invariant violations, all in this task:
      1. reverse-RPC root resolution: daemon stamps the session id on `document.*` calls (`clay-agent/src/document-ops.ts`), server resolves session → root → `WorkspaceRootId` via `SessionBook.session_root` + `WorkspaceState.directory_roots()`, and `first_root_id()` (`src/server/agent_documents.rs:148`) is **deleted** — an unresolved root becomes an `agent.workspace_unresolved` diagnostic.
      2. MCP allow-list source: build it from the **session's** root, not the launch root installed at `src/server/mod.rs:734` (`src/server/agent_mcp_config.rs:150`).
      3. daemon resume: read the recorded workspace root from session metadata instead of `process.cwd()` (`clay-agent/src/host.ts:2537`; key written at `:1477`) so a resumed session's tool cwd, acceptance roots and wiki/graft binding match its workspace.
  - Approach:
    - Documentation Reviewed:
      - Review §SC-6; `src/server/agent.rs` (registry, `apply_book_event`, `book_snapshot`), `src/server/agent_documents.rs` (tool execution/reverse RPC), `src/server/mod.rs` L723 (registration).
    - Options Considered:
      - Per-tab agent instances at the transport layer: duplicates sessions per view; the workspace root is the real identity.
      - Workspace-root keying. (Chosen.)
    - Chosen Approach:
      - Key the session registry by workspace root; thread the resolved root through tool execution; keep the book/checkpoint keying unchanged.
    - Files to Create/Edit:
      - `src/server/agent.rs`: registry keying + session→workspace resolution.
      - `src/server/agent_documents.rs`: tool paths resolve workspace from session.
      - `src/server/mod.rs`: registration wiring if the bootstrap binding changes shape.
    - References:
      - SC-6 decision-log entry (previous task).
  - Test Cases to Write:
    - Two registries (two roots), two sessions: tool write from session A lands under root A; B under B.
    - Session resume restores into the same workspace key.
    - A session whose recorded root is unknown/unregistered fails closed with a diagnostic — never root 1.
    - Two tabs on one root resolve to the same session id.
  - Evidence (2026-09-14):
    - Registry re-key (`src/server/agent.rs`): the three tab-keyed maps
      (`tab_session` / `tab_session_root` / `tab_session_agent`) are replaced by
      `SessionBook::sessions_by_workspace: HashMap<(agent, root), session>` with
      `session_for_workspace` (pure lookup) + `bind_workspace_session`; the key
      is `workspace_key(agent, root)` (empty agent = daemon default, empty root =
      no workspace), so the legacy root-less entries keep resolving by
      construction. `ensure_tab_session` resolves/adopts through the key,
      `resume_tab` records the session's root *and* the key (including the
      recorded agent when the tab has none), and `AgentHost::tab_session_id(tab)`
      is the one reader for the tab-scoped paths (`cancel_tab`, `steer_tab`,
      `search_sessions`, `rebind_tab_agent`, `publish_book_snapshot`).
      `rebind_tab_agent(tab, previous_agent)` takes the pre-switch agent type
      (the caller already had it) and *moves* the one session to the new key —
      no root scan, no ambiguity.
    - Tool authority (`src/server/agent_documents.rs`): `first_root_id()` and the
      `workspaceRootId` param branch are gone. Every `document.*` / `checkpoint.*`
      call names `sessionId`; `AgentHost::session_workspace_root` (live book only,
      no daemon round-trip — a `session.load` inside a reverse handler could
      deadlock on the daemon's request loop) yields the recorded root, and
      `SessionWorkspaces::resolve` maps root path → the tab state that has that
      folder open → that state's root id (canonical-path index,
      `WorkspaceState::root_id_for_canonical_path`, O(1); registry and state locks
      taken one at a time, never nested). Unresolvable session/root → error string
      + a broadcast `AgentServerMessage::Diagnostic { code:
      "agent.workspace_unresolved" }` (the daemon maps the error to the model's
      tool result, the view gets the diagnostic).
    - Implementation refinement of the decision log (recorded there too): root ids
      are **per tab state** (`IpcServer::tab_states`, a fresh `WorkspaceState` per
      tab beyond the bootstrap), so the session's root path — not a root id — is
      the cross-state identity, and it is resolved to the state the user has that
      folder open in. The invariant is unchanged: never a launch/first/boot-strap
      root, fail closed otherwise. Ceiling (documented in `resolve`): two tabs on
      one folder share a session and the agent's document ops go to the
      lowest-id tab state that has that root.
    - Dirty-buffer / lease authority unchanged: the call still routes through the
      resolved state's `open_existing_file_unlocked` → `apply_edit` →
      `save_document_unlocked` (client 0), so CAS/leases stay server-side; only
      *which* state is now the session's own.
    - Checkpoints (`src/server/agent_checkpoints.rs`): `checkpoint(workspace,
      root_id, …)` captures only open documents *contained in* the session's root
      and `restore(workspace, root_id, …)` restores through that root id — so
      capture/restore are symmetric and a multi-root tab state can no longer leak
      another root's documents into a session checkpoint (restore fails closed on
      a path outside the root).
    - MCP allow-list (`src/server/agent.rs`, `agent_session_params`): built from
      the **session's** workspace root (`build_mcp_allow_list(config_root,
      session_root, agent)`) and sent for every session, agent type or not; the
      host no longer holds a launch workspace root at all
      (`set_agent_roots(config_root)`, `AgentRoots.workspace_root` deleted,
      `src/server/mod.rs:734` updated). The daemon's spawn-time launch list is now
      only the initialize-time default for direct callers.
    - Daemon resume (`clay-agent/src/host.ts`): `ensureLive` reads
      `metadata[SESSION_SEARCH_WORKSPACE_METADATA_KEY]` (written at `session.new`,
      `host.ts:1477`) for `ensureSkillDiscovery`, `createSession`, and
      `live.workspaceRoot`; `process.cwd()` remains only for pre-workspace records.
      `session.resume` echoes the bound `workspaceRoot`. Daemon document ops
      (`document-ops.ts`, `coding-tools.ts`) stamp `sessionId` on every
      `document.read/write/mkdir/stat` call (`CodingToolsOptions.sessionId`, passed
      from `buildSessionTools`).
    - Server wiring (`src/server/mod.rs`): `SessionWorkspaces::new(tab_registry,
      tab_states)` (the `Arc`s are created before the agent wiring and moved into
      the struct) replaces the bootstrap-workspace binding at
      `document_reverse_handler(...)`.
    - Tests written (all new/updated):
      `src/server/agent.rs` — `sibling_tabs_on_one_workspace_share_one_session`,
      `sibling_tabs_resolve_one_session_through_the_host` (two tabs, one session,
      no daemon), `session_workspace_root_is_the_recorded_root_only` (no launch
      fallback), `mcp_allow_list_follows_the_session_workspace_root` (two
      workspaces + no-agent case), `session_kept_while_workspace_root_matches`,
      `unbound_legacy_session_survives_missing_registry`,
      `bound_session_rebinds_when_registry_disappears`,
      `workspace_change_resolves_a_new_key`, and
      `resumed_tab_keeps_its_session_on_the_next_prompt` now also asserts
      `session_workspace_root` = the tab's root;
      `src/server/agent_documents.rs` — `sessions_touch_only_their_own_workspace_root`
      (A writes A, B writes B, cross-root write/read both refused and nothing
      created) and `unresolved_session_root_fails_closed_with_a_diagnostic`
      (unrecorded + unregistered root → error + two `agent.workspace_unresolved`
      diagnostics, no file written);
      `src/server/agent_checkpoints.rs` tests updated for the root-id signature;
      `clay-agent/src/__tests__/coding-tools.test.ts` — "document calls name their
      session so the server picks the workspace"; `clay-agent/src/__tests__/host.test.ts`
      — the resume case now asserts the restored session echoes its recorded
      `workspaceRoot`.
    - Teeth checks (mutation runs, each reverted): daemon resume back to
      `process.cwd()` → host test fails (148/1); drop `sessionId` from
      `document.read` → coding-tools test fails (148/1); resolver falls back to
      the first tab's root → `unresolved_session_root_fails_closed…` and
      `sessions_touch_only_their_own_workspace_root` fail; allow-list built
      without the session root → `mcp_allow_list_follows…` fails; tab lookup
      ignoring the workspace root → `sibling_tabs_resolve_one_session…` fails.
    - Gates: `cargo fmt --all --check` clean; `cargo clippy --all-targets -- -D
      warnings` clean; `cargo check --all-targets` clean; `cargo test --lib` 1366
      passed / 0 failed / 1 ignored; `--test protocol` 216, `--test security` 149,
      `--test runtime` 75, `--test presentation` 61 (all 0 failed); clay-agent
      `npm test` 150 tests / 149 pass / 1 skipped (pre-existing skip) / 0 fail;
      `scripts/package-smoke.sh` PASSED. `scripts/check.sh full` passes audit, fmt,
      check, clippy, `test --all-targets`, and bench-compile, then fails **only**
      at the SC-1 `bindings` stage because `frontend/src/bridge/generated/` is
      still untracked in this working tree (`?? frontend/src/bridge/generated/`);
      the guard compares `git status`, so it cannot pass before those files are
      committed. No DTO/protocol type changed in this task, so that guard's
      verdict is unrelated to SC-6 (and the guard's own `--features ts-bindings`
      regeneration produced no diff).
    - Docs: `docs/wiki/modules/clay-agent.md` (session-scoped document ops, resume
      reads the metadata key, session-root MCP allow-list, key-based resume note,
      new invariants + test names), `docs/wiki/modules/agent-protocol.md`
      (ownership key, `publish_book_snapshot` note, test list),
      `docs/wiki/modules/tabs-and-clients.md` (session ownership is the workspace,
      allow-list source), `docs/wiki/index.md` (both index lines);
      `tests/documentation_coverage.rs` guard re-run green via `--test protocol`.
      `graft build` re-run (11 changed files) so the graph matches the code.
    - Behavioral note for the verification task: a session outlives its tab
      (`TabCommand::Close` removes the tab state but does not cancel the agent
      run), so a *closed* workspace's tool calls now fail closed until that
      folder is open in a tab again — the intended trade against binding the
      launch root, and the recovery path is reopening the folder (the session
      and its transcript are untouched).
    - Deferred (recorded, not silently dropped): server-side per-connection agent
      relay filtering (decision alternative 6 — the client filters by session id;
      revisit if cross-window relay traffic matters); MCP servers are still
      connected only at `session.new` (`ensureCapabilities`), so a resumed session
      reconnects nothing (pre-existing behavior, unchanged by this task); the
      checkpoint store still holds full document text (the P2-1 follow-up noted in
      that task).

- [x] SC-6 (frontend): scope agent session state to the tab runtime
  - Acceptance Criteria:
    - Functional: agent session state lives on the tab runtime (`frontend/src/shell/workspace-controller.ts` `TabRuntime`) instead of the process-global store (`frontend/src/agent/state.ts`); switching tabs switches the agent view's session; closing a tab disposes its agent session state.
    - Performance: no cross-tab re-render cascades; per-tab stores follow the existing dependency-free observable pattern.
    - Code Quality: no module-global mutable agent state remains; AG-UI custom-event handling (`clay.*`) routes per-session.
    - Security: no session transcript bleed across tabs.
    - Routing rule from the SC-6 decision log: the store adopts the session id from its own tab-stamped STATE answer, then accepts relay events only by session id (`threadId` on run events, `state.sessionId` on snapshots) — the relay is a process-wide broadcast (`src/server/connection/delivery.rs:308`) and server-side filtering is explicitly deferred, so filtering is the store's job. `TauriClayAgent` instances are per tab, so `setSender` becomes construction-scoped (no mutable process-wide sender).
  - Approach:
    - Documentation Reviewed:
      - Review §SC-6; `frontend/src/agent/state.ts`, `frontend/src/agent/TauriClayAgent.ts`, `frontend/src/shell/workspace-controller.ts`, `frontend/src/shell/workspace-envelope.ts`.
      - `.agents/skills/clay-execution/references/ui.md` → Client Architecture (server authority, stable-ID React reconciliation, narrow Tauri capabilities).
    - Options Considered:
      - Keep global store + active-tab indirection: the status quo's leak, dressed up.
      - Per-tab runtime ownership. (Chosen.)
    - Chosen Approach:
      - Move session ownership into `TabRuntime`; the panel reads from its tab's runtime; disposal on tab close.
    - Files to Create/Edit:
      - `frontend/src/agent/state.ts`: store scoped per session instance.
      - `frontend/src/shell/workspace-controller.ts`: runtime owns agent session.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: consume from runtime prop/context.
    - References:
      - SC-6 decision-log entry.
  - Test Cases to Write:
      - Tab A + tab B over different roots: transcript/state isolated; switching preserves each.
      - Closing a tab disposes its agent state (no leaked subscriptions).
  - Evidence (2026-09-14):
    - Store (`frontend/src/agent/state.ts`): the process-global singleton,
      `resetAgentSessionForTests`, and the module-scope `__clayAgentSession`
      global are gone. `createAgentSession({ send, clientId })` returns one
      tab's store, and it is **born subscribed** (`agentStream.retain()` +
      `pipeRelay`) with `dispose()` as its single release: the tab owns the
      transcript, so a run that outlives a view switch still streams into it
      (`start()` is now only an extra refcount for a mounted surface).
    - Traffic rule (`accepts`): a frame passes only when
      `event.clientId` is this tab's connection **and** — for a
      session-tagged frame — `event.sessionId` is this tab's server-answered
      binding. Untagged frames (diagnostics, agent-RPC replies, a pre-session
      STATE snapshot) are process-wide and pass. Two consequences worth
      recording: the client-id test is a *delivery* test (the relay is a
      process-wide broadcast, so each tab's connection relays its own copy of
      every session's traffic — the filter also drops the other tabs' copies,
      which is what makes a multi-tab window's stream single-copy), and the
      session-id test is the *ownership* test.
    - Binding, never inference: the store adopts a session only from a
      `clay.agentRpc` `session.bound` claim whose `clientId` matches its own
      connection (empty `sessionId` = "this tab owns nothing yet"); a
      session-tagged frame it cannot attribute asks for a refresh (debounced
      500 ms), so an agent switch or a resume re-binds without the panel
      knowing about it. It never adopts "the first snapshot that arrives",
      which is the old bleed mechanism.
    - Transport (`frontend/src/agent/TauriClayAgent.ts`): `setSender` is gone;
      the sender and the `accept` predicate are construction options
      (`new TauriClayAgent({}, { sender, accept })`) and `run()` filters the
      relay, so a run can no longer adopt another session's `RUN_STARTED`.
    - Ownership + lifetime (`frontend/src/shell/workspace-controller.ts`):
      `TabRuntime.agent` (+ `TabRuntime.send`, the lazy tab-stamped sender) —
      the agent view creates the store on first mount and the runtime adopts it
      (`attachAgentStore`, idempotent), disposing it on tab close and on
      `reset()`. The controller references the store's **type only** and never
      constructs it: the first attempt imported `createAgentSession` here and
      the entry chunk gained a static `import … from "./agent-core-…js"` plus
      an `index.html` preload — i.e. the whole AG-UI stack (38 kB gzip) back in
      the startup path. Verified after the restructure: the entry imports no
      agent chunk, `dist/index.html` no longer preloads `agent-core`, and the
      store's strings (`session.bound`, `tabState`, `clay.toolPhase`,
      `agent_subscribe`) appear only in `agent-core-*.js`.
    - Panel commands (`frontend/src/coding-agent/CodingAgentPanel.tsx`,
      `AgentView.tsx`, `WorkspacePanes.tsx`): every agent command
      (`listSessions`, picker `select`, `workspace.files`, `session.context`,
      `session.om.*`, `runResume`, SDUI intents) rides the tab's own connection
      through the store (`command`/`sendPayload`) instead of the bridge's
      active client, so two tabs cannot address each other's session; the
      standalone-mount fallback keeps the same behavior it had (a store with no
      sender falls back to `sendRequest`), which is why the 27 existing panel
      tests only needed their store injected.
    - Server/daemon support (recorded in the SC-6 server task, used here):
      `session.bound { clientId, tabId, sessionId }` on the requesting
      connection for `TabState` (`session_bound_message`,
      `src/server/connection/mod.rs`) and for every run command
      (`src/server/connection/runtime.rs`, written before the command's own
      answer) — the latter is what hands a *newly created* session to the store
      before any of its snapshot or run events; and
      the AG-UI relay now stamps `sessionId` on every session-scoped frame
      (`agent_agui::session_of` → `AgentStreamEvent.session_id`,
      `src-tauri/src/bridge/agent.rs`), without which text/reasoning chunks and
      tool phases could not be attributed at all.
    - Tests written:
      `frontend/src/agent/transport.test.ts` — "drops another tab's delivery
      and another session's traffic" (foreign copy + foreign session's
      transcript/lifecycle dropped; own session's snapshot applies),
      "ignores a binding claim addressed to another tab", "stops applying and
      notifying once the tab closes";
      `frontend/src/shell/workspace-controller.test.ts` — "adopts one agent
      session store per tab and disposes it on close" (no store before the
      agent view mounts, per-tab identity, idempotent adopt, tab-stamped lazy
      `send`, dispose on close);
      `frontend/src/shell/WorkspacePanes.test.tsx` — "keeps a tab's agent store
      on the relay after its view unmounts". Every panel test now mounts with
      its own store (`agent={store}`), the fixture owns and seeds its store
      (`frontend/src/routes/fixture.tsx`), and
      `frontend/src/test/chat-surface-absence.test.ts` pins the new module
      surface (factory + `AgentSessionModule`, no singleton, no reset seam).
    - Teeth checks (each mutation reverted): client-id test removed →
      "drops another tab's delivery…" fails; session test relaxed to accept any
      session → same test fails; `dispose()` no longer disposing the runtime's
      store → the controller test fails; `runtime.agent.start()` removed →
      the WorkspacePanes relay test fails.
    - Test-infra fix found on the way: `WorkspacePanes.test.tsx`'s relay
      harness `emit()` was a no-op (`listeners.add(listener)` instead of
      invoking it), so the first test that actually emits would have silently
      passed nothing; it now fans out, and the new test is the one that proves
      the runtime keeps its store subscribed.
    - Lint fix found on the way: `WorkspacePanes` called `openInWorkspace`
      after an early return (a latent "more hooks than during the previous
      render" crash when a runtime appears); both callbacks now sit above the
      return. eslint is 9 errors / 4 warnings vs the baseline 12 / 4 (the rest
      are pre-existing test-file non-null assertions).
    - Verification: `npx tsc -b` 0 errors; `npx vitest run` 45 files / 399
      tests pass (3 unhandled `invoke`-undefined rejections — the same class as
      the baseline's 2, and re-running the suite with this task's changes
      stashed still reports 3, so the third is not from SC-6). Two full runs
      passed 399/399; on a later run with the host at load ~40, the lazy-fixture
      `shell.test.tsx` suspense wait (1 s `findAllByText`) and the
      lazy-Suspense panel mount timed out — both pass in isolation and both are
      load-sensitive waits, not assertion failures (same class as the Rust
      `js_runtime` flake above); recorded rather than papered over with a
      timeout bump; prettier clean;
      `npm run build` ok; `check:budget` shell 173.6 kB / 180, total 399.9 kB /
      400 — **identical** before and after this task (measured by rebuilding
      with the task's frontend changes stashed), i.e. the store's bytes live in
      the lazy `agent-core` chunk; `cargo fmt --check` clean; `cargo clippy
      --all-targets -- -D warnings` clean; `cargo test --lib` 1368 (2 new:
      `agent_agui::session_of_tags_only_session_scoped_variants`,
      `connection::tests::tab_state_binding_names_the_requesting_client_and_session`),
      `--test protocol` 216, `--test security` 149, `--test runtime` 75,
      `--test presentation` 61, `-p clay-desktop --lib` 31 (all 0 failed);
      clay-agent `npm test` 150 tests / 149 pass / 1 skipped / 0 fail and
      `scripts/package-smoke.sh` PASSED. `scripts/check.sh full` passes audit,
      fmt, check, clippy and then fails at its `test` stage on the pre-existing
      flaky `server::js_runtime::tests::third_party_poison_replays_approved_graph_and_restores_providers`
      (worker `RecvError` under `--all-targets` parallelism — the P1-4 flakiness
      recorded earlier in this plan). Mechanism, now observed directly while
      re-running under load average ~40: the poisoning replay schedules a
      completion and the *worker channel closes* before replying, i.e. a
      JS-runtime budget (its own, tighter than the test's 3 s wait) fires under
      CPU starvation. The test's wall-clock budget is a symptom, not the cause —
      a follow-up belongs with the P1-4 item, not this task. Verified not from this task: the same
      stage fails with this task's Rust changes stashed (and the test passes in
      isolation; `cargo test --lib` alone is 1368/1368 green). The final stage,
      `bindings`, still fails for the SC-1 reason: `frontend/src/bridge/generated/`
      is untracked, so the staleness guard cannot pass before that commit.
    - Docs: `docs/wiki/modules/react-agui-chat-stream.md` (ownership row,
      the relay's delivery-vs-ownership stamping, the "Tab ↔ session binding"
      section, presentation/lifetime, verification),
      `docs/wiki/modules/agent-protocol.md` (Plan 119 SC-6 additions: the
      `session.bound` answer and the per-frame session stamp),
      `docs/wiki/modules/tabs-and-clients.md` (per-tab store + binding),
      `docs/wiki/index.md`; reusable rule in
      `.agents/skills/clay-execution/references/ui.md`.
    - Deferred (recorded, not silently dropped): server-side per-connection
      relay filtering stays deferred (the store filters; a `session.bound`
      refresh is the recovery path); the informational agent RPCs whose
      payloads carry no session id (`session.resumable`, `workspace.files`)
      are attributed by their own answers' content, not by a per-tab claim —
      revisit if a case appears where that matters; and the panel's
      `state.sessionId` (not the store's binding) is what its RPC payloads
      carry, which is equivalent while the filter holds.

- [x] SC-6 (verification): multi-tab agent integration pass
  - Acceptance Criteria:
    - Functional: full flow on a real Linux build — two workspaces open, one agent tab each, prompts, a file write per session, resume per session; writes land in the right roots.
    - Performance: startup and session-switch latency within baseline (task 1) ± noise.
    - Code Quality: covered by automated tests where possible; manual pass records the rest.
    - Security: verified no cross-root tool addressing.
  - Approach:
    - Documentation Reviewed:
      - Review §SC-6; test-plan module map (`test-plan/index.md`).
    - Options Considered:
      - Automated-only: the ownership bug class is cross-component; a live pass is the cheap insurance.
      - Automated + live. (Chosen.)
    - Chosen Approach:
      - Live two-workspace session; record evidence paths in task completion.
    - Files to Create/Edit:
      - `test-plan/` (numbered steps added in the manual-test-plan task below).
    - References:
      - Review keep-list: checkpoint primitives are the resume backbone.
  - Test Cases to Write:
    - End-to-end two-tab isolation + resume scenario.
    - Per the SC-6 decision log (2026-09-14-1705): a **resumed** session (daemon restarted, tab reopened) still writes inside its own workspace root, and a session whose root is not registered fails closed with a diagnostic rather than touching root 1. (Both are asserted live below; the manual GUI step list stays with the manual test-plan task, which owns `test-plan/`.)
  - Outcome — the live two-workspace pass is automated, not manual:
    `tests/agent_session_isolation.rs` (registered in `tests/suites/security.rs`)
    spawns the **real `clay server` binary** (`CARGO_BIN_EXE_clay`) on a real
    socket with an isolated `HOME`, binds two real clients to two workspace
    roots through `clay::client::connect_with_workspace_root`, configures both
    tabs through the panel's own lanes (`AgentClientCommand::Select`,
    `TabState`, and the composer's `agent.submit` SDUI action), and asserts the
    SC-6 invariant end to end. Run:
    `cargo test --test security -- agent_session_isolation` (2 tests, ~1.4 s).
    - `two_workspaces_keep_their_agent_writes_in_their_own_root` (scripted
      daemon via `CLAY_NODE`/`CLAY_AGENT_MAIN`, so the daemon knows nothing
      about which session owns which root): every prompt attempts the **same**
      `document.write` against **both** roots, so only the server's containment
      check can decide the winner. Asserts, per prompt: the session's own root
      accepted the write with that session's id and the sibling root has no
      file; both tabs got their own routed `session.bound`
      (`clientId`/`tabId`/`sessionId`) with different session ids; after
      `TabCommand::Close` of tab A, the daemon's probe of that stale session is
      rejected (`agent.workspace_unresolved`, logged server-side as "*session
      mock-session-N workspace root is not open*") and root A's content is
      untouched; after killing the daemon child (found through
      `/proc/<pid>/task/*/children`), the next prompt rebuilds it and the
      **same session id** writes into the **same root**, with root A still
      untouched. Evidence: the test's own assertions plus the scratch
      `writes.log` (per-attempt ok/err) and `server.log` diagnostics.
    - `real_daemon_serves_one_session_per_workspace` (the **shipped daemon** in
      `--mock` mode — `CLAY_AGENT_MOCK=1`, no model call, credential put
      through the panel's own `CredentialPut`): each session's own
      `workspace.files` listing names its root's marker file and never the
      sibling's, and stays that way **after the daemon is killed and restarted**
      — the resumed session reads the root recorded in its metadata, not the
      daemon's launch directory. This is the daemon half of the fix verified
      over a real wire, with the run streaming real daemon events tagged with
      the tab's session id.
    - Teeth (mutation-checked while writing the test): reverting the daemon's
      `ensureLive` to `process.cwd()` makes the resumed-session assertion fail
      (`workspace.files` no longer names the root's marker); disabling the new
      `forget_running` clear makes the post-restart write never land. The
      server-side authority mutations were already covered by the in-crate
      SC-6 tests mutated in the SC-6 (server) task.
    - Bug found by this pass and fixed: **a daemon exit left the host's
      `Running` handle stale**, so every later call addressed a channel nobody
      read — a crashed daemon never came back (the SC-6 resume scenario was
      impossible until the server restarted). `AgentHost::forget_running`
      (`src/server/agent.rs`) now clears the handle from the actor's exit path,
      identity-checked with `mpsc::Sender::same_channel` so a respawn that raced
      the exit keeps its own handle; `daemon_actor` takes the host + its own
      command sender to do it. Verified by the post-restart step above and by
      the mutation just described.
    - Acceptance mapping: functional — two workspaces, one agent tab each,
      prompts, a write per session, session survival across a daemon restart
      (no GUI was driven: the pass runs the real binaries, socket, and reverse
      RPC; the manual GUI step list stays with the manual test-plan task);
      performance — no eager work was added to startup or session switches: the
      only production change sits on the daemon *exit* path, and the agent
      latency budgets still pass (`tests/protocol.rs`
      `mock_spawn_creates_owner_only_passphrase_within_budget` at
      `AGENT_DAEMON_SPAWN_P95_BUDGET_MS` and
      `mock_daemon_prompt_persists_no_secret_on_ack` at
      `AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS`); the two new tests finish in
      ~1.4 s together; code quality — the
      scenarios are automated and in CI's `security` suite rather than a
      one-off manual run; security — no cross-root addressing: the sibling-root
      writes are rejected by containment (`path outside workspace roots`) and
      the closed-session case fails closed with a diagnostic instead of a
      fallback root.
    - Gates after the change: `cargo fmt --all --check` clean; `cargo clippy
      --all-targets -- -D warnings` clean; `cargo test --lib` 1368,
      `--test protocol` 216, `--test security` **151** (149 + the 2 new),
      `--test runtime` 75, `--test presentation` 61, `-p clay-desktop --lib`
      31, all 0 failed; clay-agent `npm test` 150 / 0 fail. `scripts/check.sh
      full` passes audit, fmt, check, clippy, its whole `test` stage (lib,
      protocol, runtime, presentation, security, desktop — the earlier
      `js_runtime` flake did not reproduce) and bench-compile, then stops at
      `bindings` for the SC-1 reason alone: `frontend/src/bridge/generated/` is
      still untracked, so the staleness guard cannot pass before that commit.
    - Docs: `docs/wiki/modules/clay-agent.md` (the daemon-exit lifecycle in
      Spawn + an Invariants bullet + the live pass in Tests) and
      `docs/wiki/modules/agent-protocol.md` (respawn/resume invariant + the new
      test command); both pass `tests/documentation_coverage.rs`.

- [x] SC-3: split the server agent actor into focused modules
  - Acceptance Criteria:
    - Functional: pure mechanical split — `agent/book.rs` (apply/snapshot/load), `agent/run.rs` (run pipeline, approvals), `agent/mcp.rs` (inventory) extracted from `src/server/agent.rs`; behavior identical; all agent tests green.
    - Performance: no call-path changes.
    - Code Quality: follows the existing `agent_documents.rs`/`agent_checkpoints.rs` extraction pattern; no logic edits beyond visibility.
    - Security: boundary unchanged.
  - Approach:
    - Documentation Reviewed:
      - Review §SC-3; `src/server/agent.rs` (L496–L1701 loop is connection, agent actor is ~L3400+), existing extraction precedent.
    - Options Considered:
      - Split first, then SC-6: double-churn on hot files.
      - Split after SC-6 lands (behavioral change on final state). (Chosen — review's "opportunistic" guidance.)
    - Chosen Approach:
      - Module moves only; `git mv`-style history preserved where practical.
    - Files to Create/Edit:
      - `src/server/agent.rs` → `src/server/agent/` (book.rs, run.rs, mcp.rs; mod wiring).
    - References:
      - Review §SC-3.
  - Test Cases to Write:
    - None new; existing suites are the guard.
  - Outcome — mechanical split, `src/server/agent.rs` 5859 → 3262 lines:
    - `src/server/agent/book.rs` (1219): `SessionBook` + `BookSelection` (state,
      `book.json` persistence, `apply_book_event`, `book_snapshot`), the
      `(agent type, workspace root)` session key (plan 119 SC-6), branch reads,
      `snapshot_from_new` / `snapshot_from_load` / `append_loaded_entry` and the
      STATE snapshots derived from the book (`snapshot_for`, `tab_state_snapshot`,
      `unconfigured_snapshot`, `decorate_snapshot`), picker selection
      (`select_picker`, `select_worker`, `ensure_default_model`,
      `publish_book_snapshot`), session listing (`resumable_sessions`,
      `search_sessions`) and the picker types.
    - `src/server/agent/run.rs` (1015): the run pipeline
      (`begin_prompt[_with_effort]`, `cancel_tab`, `steer_tab`, `resume_tab`,
      `run`, `run_inner`), approvals (`request_user_approval[_for]`,
      `resolve_approval`, `APPROVAL_WAIT`, `MAX_APPROVAL_PAYLOAD_BYTES`,
      `PendingApprovals`), secret capture/redaction helpers and the daemon event
      mapper (`map_event` + tool digests).
    - `src/server/agent/mcp.rs` (479): `AgentMcpAllowListEntry` (+ `to_json`
      wire form), the per-session allow-list in `agent_session_params`, and the
      daemon inventory — `DaemonEnvironment` + `parse_environment`,
      `environment`, `inventory`, `inventory_rich`, `picker_inventory` and the
      provider/model/profile/session parsers.
    - `src/server/agent.rs` (1636 non-test lines, 3262 with its tests): the host
      itself — config, spawn/`daemon_actor`/RPC framing, tab↔session resolution,
      credentials/OAuth, package-registration queue, diagnostics.
    - `agent.rs` stays the module root instead of becoming `agent/mod.rs`
      (deviation from the task's "→ `src/server/agent/`" wording, same result):
      `foo.rs` + `foo/` is a legal Rust layout with precedent here
      (`src/editor.rs` + `src/editor/`), and it avoided churn in
      `tests/agent_protocol.rs` (which reads `src/server/agent.rs` for the spawn
      invariants) and in every wiki `Source` row.
    - Wiring: `mod book; mod mcp; mod run;` plus re-exports that keep every
      path outside the module unchanged (`pub use mcp::AgentMcpAllowListEntry`,
      `pub(crate) use book::{AgentPickerAuth, AgentPickerInventory,
      AgentPickerProvider}`). Inherent methods need no re-export, so all
      `AgentHost` call sites (connection loop, menus, `js_runtime`) compile
      untouched.
  - Outcome — proof it is a pure move (no behavior edit):
    - Method: normalized code-line multiset of the pre-split
      `src/server/agent.rs` (`/tmp/agent.rs.presplit`, the working tree before
      this task) vs the concatenation of the four files, ignoring blank lines,
      braces, `use`/`mod`/`//!` lines and visibility keywords.
      Result: **5 lines only in the original, 40 only in the new**, and all 45
      are accounted for by this task's intended edits — the 5 are long
      signatures/lines that rustfmt wrapped because `pub(super)` made them
      longer (and `super::agent_settings` / `super::agent_mcp_config` became
      `crate::server::…` inside a deeper module); the 40 are the new module
      docs, import lists, three `impl AgentHost {` wrappers and the wrapped
      continuations. No moved statement differs from its original.
    - Visibility: moved items are `pub(super)` (visible inside the `agent`
      module tree only) and the external types keep their exact previous
      visibility through the root re-exports, so
      `tests/rust_visibility_api_mapping.rs` (public-API discipline) is
      unaffected and the crate's public surface is unchanged.
    - Cross-module dependencies are explicit and one-directional in the reading
      order (root ← book/run/mcp; run imports the book's snapshot builders; mcp
      imports the picker types), all via `use super::…` — no `super::super::`
      escapes, no glob imports. The two JSON accessors (`json_text`,
      `json_string`) that both the event mapper and the transcript loader use
      were hoisted to the root rather than left in one concern module.
  - Deliberately left in place: the three in-file test modules
    (`tab_workspace_tests`, `approval_tests`, `data_dir_migration_tests`, 1626
    lines) stay in `agent.rs` and reach the moved items through the root's
    imports; they exercise the whole host rather than one concern. Extracting
    them the way `connection/tests.rs` does is the next step if the root file's
    size matters again — recorded, not done (the plan's "no new tests" scope).
  - Gates: `cargo fmt --all --check` clean; `cargo clippy --all-targets -- -D
    warnings` clean; `cargo test --lib` **1368**, `--test protocol` **216**,
    `--test security` **151**, `--test runtime` **75**, `--test presentation`
    **61**, `-p clay-desktop --lib` **31** — every suite identical to the
    pre-split counts (0 failed), i.e. the extraction changed no behavior; the
    daemon source and `clay-agent` tests are untouched. `scripts/check.sh full`
    passes audit, fmt, check, clippy, its whole `test` stage and bench-compile,
    then stops at `bindings` for the SC-1 reason alone (untracked
    `frontend/src/bridge/generated/`).
  - Docs: `docs/wiki/modules/clay-agent.md` (the three submodules in Source +
    a paragraph describing the module tree) and
    `docs/wiki/modules/agent-process-manager.md` (Source rows: host/actor vs
    `run.rs` dispatch vs `book.rs`); `tests/documentation_coverage.rs` passes.
    `docs/reference/clay-js-api/*` `backing_rust` entries still name
    `src/server/agent.rs::AgentHost::run`, which resolves through the module
    root — the guard only checks the file exists
    (`clay_js_api_inventory.rs`), and the symbol lives one level down in
    `agent/run.rs` now; left as-is because `agent.rs` *is* the module root.

- [x] SC-4: decompose CodingAgentPanel (fold PF-2 in)
  - Acceptance Criteria:
    - Functional: `CodingAgentPanel.tsx` (~2300 lines) split into `Composer` (slash + @-mention + effort), `TranscriptList` (+ the PF-2 previous-agent scan fixed as one reverse-pass memo, replacing the per-box `slice().reverse().find()` O(n²)), `ApprovalStrip`, `InspectorTabs`; rendered output and behavior identical; existing panel tests green unchanged.
    - Performance: transcript render drops from O(n²) to O(n) on the agent-label scan; render-count parity verified on a representative session.
    - Code Quality: each extracted component independently testable; state machines unchanged inside components.
    - Security: approval flow (`runResume`) semantics untouched.
  - Approach:
    - Documentation Reviewed:
      - [`DESIGN.md`](../../DESIGN.md) (normative Quiet Instrument language; §12 agent surface; §14 retired patterns).
      - `.agents/skills/clay-execution/references/ui.md` (client architecture, skill routing), `references/components.md`, `references/tokens.md` (catalogs), `docs/reference/ui-components.md`.
      - Review §SC-4, §PF-2.
    - Options Considered:
      - Prototype gate: NOT triggered — this is a structural refactor with byte-identical rendered output; no new surface, state, or aesthetic change. Conformance target: existing approved set `design-artifacts/approved/quiet-instrument-migration/`. Any deviation surfaced during the refactor re-enters the prototype loop.
      - Extract everything at once vs leaf-first: leaf-first (Composer last — most coupled). (Chosen.)
    - Chosen Approach:
      - Extract leaf components with props-in/props-out; keep the panel as the composition root; fix the PF-2 scan inside `TranscriptList`.
    - Files to Create/Edit:
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: shrink to composition root.
      - `frontend/src/coding-agent/` new: `Composer.tsx`, `TranscriptList.tsx`, `ApprovalStrip.tsx`, `InspectorTabs.tsx` (tentative names).
      - `frontend/src/coding-agent/CodingAgentPanel.test.tsx`: unchanged expectations; component tests added where extracted logic is non-trivial (mention tokenizer, effort chord).
    - References:
      - Review §SC-4, §PF-2; DESIGN.md §12 (the agent view's title is the agent-type picker; Files tab is session file history).
  - Test Cases to Write:
    - Component-level: mention-token parser edge cases; effort-chord cycling; approval allow/deny payloads.
  - Outcome — `CodingAgentPanel.tsx` 2335 → 999 lines, ten focused modules:
    - `CodingAgentPanel.tsx` (composition root): props, the tab store
      (create-on-first-mount + `onAgentStore` adoption), the snapshot
      subscription, the view-model parses (transcript boxes, slash commands,
      skills, files, MCP servers, meter, picker options), the send authority
      (`submit` → prompt/steer/client built-ins, `runResume`, `sendIntent`),
      tab/session state, and the left column's layout.
    - `transcript-model.ts` (pure): `TranscriptBox`/`TranscriptKind`,
      `kindOf`/`agentOf`/`toolMeta`/`agentLabel`, and `previousAgents` — the
      PF-2 fix.
    - `TranscriptList.tsx`: the scroll column, memoized `TranscriptTurn`
      rows, the empty state, and the switch-note pass.
    - `Composer.tsx`: the message field, slash/@-mention completions, and the
      effort chord — the whole input state machine, plus exported pure pieces
      (`chordMatches`, `nextEffortLevel`, `mentionTokenOf`).
    - `ApprovalStrip.tsx`: the durable-run Allow/Deny alert (presentation; the
      decision payload stayed with the panel's run authority).
    - `InspectorTabs.tsx`: the right column's strip and tab wiring.
    - `FilesTab.tsx`, `MemoryTab.tsx`, `ContextTab.tsx` (+
      `CapabilitySections` + the context view types/guards),
      `SessionInfoTab.tsx`, `SettingsTab.tsx`, `BoundedText.tsx`: the five tab
      bodies, each independently testable.
  - Outcome — PF-2 and behavior parity:
    - PF-2: the per-turn `transcript.slice(0, index).reverse().find(...)` is
      gone; `previousAgents` answers every turn's predecessor in one forward
      pass and `TranscriptList` memoizes it on the box list. Two guards pin
      it: a property-read count (≤ 3n for 500 boxes — the old shape was O(n²))
      and a source-shape check that the reverse scan cannot come back
      (`transcript-model.test.ts`). Render parity: the memo is asserted to run
      once per box list across re-renders (`TranscriptList.test.tsx`).
    - Behavior parity: `CodingAgentPanel.test.tsx` expectations are unchanged
      (39 tests before, all green after) and the whole frontend suite is
      green: 48 files / 425 tests (baseline 45 / 399; SC-4 added 26).
    - New component tests: mention-token edge cases (mid-word `@`, trailing
      token, newline boundary, `@skill:`/`@file:` narrowing), effort cycling
      (wrap from the declared top, stale level restart, single-level), chord
      match (default Shift+Tab strictness, rebound chord), the
      consumed-submit contract (draft kept when the panel ignores input),
      mention embedding, the one-shot per-session file fetch, Stop/Close
      lanes, transcript switch notes/selection/memo, and the approval
      `runResume` decision JSON (`allow_once` + request id) with the
      optimistic strip clear.
    - Found and fixed while touching the send plumbing (a plan 119 SC-6 gap):
      `AgentView.tsx` dropped `send` when the panel started taking the tab
      store, so a panel-created store fell back to the bridge's process-wide
      active client — the very cross-tab lane SC-6 closed. `AgentView` now
      forwards `send={send ?? undefined}`; two tests pin it: the panel gives
      its creation-scoped sender to the store it creates
      (`CodingAgentPanel.test.tsx`) and, end to end, the mounted view's
      `listSessions` arrives on the tab's stamped sender and never on the
      bridge client (`WorkspacePanes.test.tsx`). Mutation-checked: removing
      the forwarding fails the workspace test.
  - Gates: `npx tsc -b` clean; `npx eslint src/coding-agent/` clean;
    `npm run build` ok; full frontend suite 48 files / 423 tests green
    (3 known unhandled-rejection accounting errors, the baseline class);
    the panel file no longer carries the baseline's three
    `react-hooks/exhaustive-deps` warnings (the model-group memo reads its two
    state slices into locals) so repo-wide lint is 4 errors / 1 warning —
    baseline 12 / 4; the remaining four are in files outside this task
    (`CommandCentre.test.tsx`, `tab-store.test.ts` ×2,
    `components.test.tsx`).
  - Budget finding (new compromise, recorded below): `check:budget` now fails —
    total 400.3 kB gzip against the 400 kB ceiling (shell 173.6 / 180). The
    split cost ≈0.5 kB gzip (panel chunk 10.95 → 11.41 kB gzip): module
    boundaries are free under rollup flattening (folding one back measured
    0.0), so the cost is the new props/plumbing and the exported pure helpers —
    i.e. exactly the structure the review asked for — against a ceiling that
    had ≈0.1 kB of headroom at baseline (399.9). The ceiling is a policy
    number in `frontend/scripts/bundle-budget.mjs` (plan 097 Phase 4/5), so
    raising it (e.g. to 402 kB, +0.5%) is a decision for the user; the plan
    records both options rather than silently re-scoping the gate.
  - Docs: `docs/wiki/modules/react-agui-chat-stream.md` (ownership row, the
    SC-4 module split, the PF-2 pass, current chunk/budget figures),
    `docs/wiki/modules/clay-agent.md` (Source list + a split sentence),
    `docs/wiki/modules/react-tabs-and-splits.md` (the inspector's tab strip);
    `tests/documentation_coverage.rs` green.

- [x] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: real Linux GUI build exercised over the agent panel (default, streaming, approval, empty transcript, inspector tabs) and two-tab agent sessions; screenshots per state stored under a named artifact path; findings recorded.
    - Performance: narrow + wide layouts captured where layout can change.
    - Code Quality: comparison against `design-artifacts/approved/quiet-instrument-migration/` surface by surface; every deviation recorded with disposition (fixed to match or explicitly re-approved); zero unrecorded deviations.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
      - `.agents/skills/clay-execution/references/planning-checklist.md` (Visual and Accessibility Review Duty).
      - `scripts/capture-ui-review.sh`.
    - Options Considered:
      - Source-inspection only: explicitly forbidden by the duty.
      - Live review with screenshots + `computer-use-linux` `get_app_state` a11y pass. (Chosen.)
    - Chosen Approach:
      - Exercise changed states, inspect the a11y tree, verify keyboard-only flow over composer/completions/approval strip; record blocker verbatim if GUI tooling is unavailable rather than claiming a pass.
    - Files to Create/Edit:
      - Evidence under `code-reviews/screenshots/` or task evidence paths.
    - References:
      - SC-4, SC-6 tasks.
  - Test Cases to Write:
    - None (review task; evidence in completion record).
  - Outcome — evidence: `code-reviews/screenshots/2026-09-15-plan119-sc4-agent-review/`
    (`review-log.md` is the completion record).
    - Browser layer (the real React panel + shell through the Vite dev client):
      six states (`landing`, `conversation`, `streaming`, `approval`, `error`,
      plus the five inspector tabs driven by keyboard) at 1440×900 and 780×900,
      each with a viewport PNG and a renderer accessibility tree (`ax.txt`).
    - Keyboard-only layer: 24-stop tab order from a cold load, slash-command
      completion (list → ArrowDown → dispatch → Escape), `@`-mention completion
      (skills), the Shift+Tab effort chord (cycles and wraps, draft preserved),
      approval Allow activation (keyboard reach at stop 8, Enter → optimistic
      clear), inspector tabs by ArrowRight — transcripts in
      `keyboard/keyboard-flow.txt`.
    - Live layer: the real Tauri build through
      `scripts/capture-ui-review.sh --fixture ui-review-coding-agent --drive
      '[{"find":{"role":"tab","name":"Agent"},"do":"click"}]'` — the AT-SPI
      drive step reaches the agent view, so this fixture is a **PASS for the
      first time** (it historically recorded UNRESOLVED for lack of input
      synthesis). Retained: AT-SPI dump (`landmark "Coding Agent"`,
      `log "Transcript"`, `entry "Message"`, `button "Send"`, `page tab list
      "Agent detail"`), drive transcript, bounded server diagnostics
      (`configuration_failed_lines=0`). The portal PNG was inspected and
      deleted: the status bar showed the isolated review root's path (harness
      privacy rule).
    - Approved-artifact comparison: surface by surface against
      `design-artifacts/approved/quiet-instrument-migration/agent-landing.html`
      (header/picker/meter/effort, transcript rows, tool blocks, empty
      transcript, composer, status strip, inspector tabs, Files rows, capability
      sections, agent settings) — table in `review-log.md`; two deviations, both
      recorded (D1 fixed, D2 open).
    - The fixture route gained the state the review needed (DEV-only,
      `frontend/src/routes/fixture.tsx`): the conversation state now seeds
      `models`/`providers`/`skills`/`mcpServers`/`branch`/`extensions`/
      `contextTokens`/`sessionId`/`omView` and a new `approval` state seeds a
      suspended durable run — an earlier pass recorded three "missing picker /
      meter / effort" deviations that were fixture gaps, not panel defects.
  - Outcome — findings (full table in `review-log.md`):
    - **D1 (fixed to match):** `FilesTab`'s empty state used
      `styles.sessionFilesEmpty`, a class defined in no CSS module (also at
      HEAD — pre-existing), so its title and body rendered flush. It now uses
      the shared `.empty`/`.emptyTitle`/`.emptyText`/`.keyHints` treatment the
      transcript's empty state uses; the dead `.sessionFileKeys` rule is gone.
      Visible in both the browser captures and the live app.
    - **D2 (open decision):** the inspector tab strip needs 337 px in a 303 px
      strip (inspector fixed at 340 px), so it scrolls at every viewport width,
      where the approved artifact fits all five tabs. The spacing comes from the
      design-system tab recipe (the prototype predates `@clay/design-instrument`)
      and all five tabs stay keyboard-reachable, so this is a token-vs-prototype
      delta: either tighten the inspector strip's tab padding to fit 303 px or
      re-approve the scroll. Not re-scoped unilaterally.
    - Recorded follow-ups (not regressions from this plan): the agent-type
      picker is announced `Agent type Agent type` (label = placeholder, the plan
      097 class); the approval strip is `role="alertdialog"` without focus
      movement; the Context tab's MCP region is named `MCP servers connected`
      while listing hidden servers; transcript rows are individual tab stops, so
      the composer is one stop per row away; the stale bootstrap
      `unknown workspace document 1 …` diagnostic is re-confirmed in the agent
      view and the real app (plan 118 finding, still open).
    - New guard (F1): `frontend/src/test/css-module-classes.test.ts` resolves
      every `styles.X` against the imported CSS module across `frontend/src` —
      an undefined class renders unstyled and throws nothing, which is exactly
      the failure mode a module split introduces. Mutation-verified: reverting
      D1's fix fails it.
  - Gates: `npx tsc -b` clean; `npx eslint .` 4 errors / 1 warning (the
    pre-existing outside-scope set; 12/4 at plan start); frontend suite
    **49 files / 426 tests** pass (48/425 before the guard); `npm run build` ok;
    live capture PASS at viewport 1280×1104 (above the 900×600 floor).
    `check:budget` remains red at 400.3 kB / 400 kB — the SC-4 budget finding
    above, still awaiting the same decision.
  - Open gaps recorded, not waived: two-tab agent sessions stay
    **UNRESOLVED** for a live visual pass (the review root configures no
    provider, so no daemon session exists to multiplex; isolation is covered by
    `tests/agent_session_isolation.rs` and
    `frontend/src/shell/WorkspacePanes.test.tsx`); the shipping Settings tab
    body renders only with a bound document session, so the fixture route shows
    its session-less fallback and the neighbouring agent-settings surface was
    captured instead.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: review all Rust public functions and behaviors introduced/changed by this plan; confirm none requires a new public Clay JS API (plan changes internal architecture, protocol delivery, and agent session ownership — no new user-programmable capability); any function that should not be exposed is private/`pub(crate)`.
    - Performance: n/a.
    - Code Quality: registry/docs coverage checks (`cargo test` gates) green.
    - Security: no implicit authority exposure.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
    - Options Considered:
      - Skip: the clay.md requirement makes this task mandatory per plan; it closes as "verified, none needed" with evidence.
      - Verify-only task. (Chosen.)
    - Chosen Approach:
      - Inventory changed public Rust functions (SC-2 helpers, SC-6 registry, P2 paths); confirm privacy or JS API coverage.
    - Files to Create/Edit:
      - `src/server/agent_agui.rs`: verify the AG-UI adapter has no public session-tagging helper (no net source change required).
      - `src-tauri/src/bridge/agent.rs`: keep session tagging private to the desktop relay, where it is consumed.
      - `tests/rust_visibility_api_mapping.rs`: add the SC-6 relay-boundary regression guard.
    - References:
      - `src/packages/manifest.rs` `RESERVED_CORE_API_DOMAINS` naming rules if anything new surfaces.
  - Test Cases to Write:
    - `server::agent_agui::tests`: preserve AG-UI adapter behavior after removing the cross-crate session helper.
    - `bridge::agent::tests::session_tags_only_session_scoped_messages`: verify event/snapshot/process-wide tagging in the private desktop relay.
    - `security::rust_visibility_api_mapping`: verify SC-6 session tagging is not a public Rust or Clay JS surface.
    - Clay JS inventory, documentation-registry, facade-layout, and facade-op mapping tests: verify the public API matrix remains unchanged and current.
  - Outcome — verified with no new Clay JS API:
    - Reviewed the plan's changed public-Rust surface: SC-3's `AgentMcpAllowListEntry` re-export preserves an existing module path; `AgentHost::dispatch` and the public agent methods preserve existing server orchestration paths; SC-6 session/root maps, `SessionWorkspaces`, reverse-RPC handlers, connection delivery policy, P2 dirty-buffer/atomic-create paths, and workspace root indexing are private or `pub(crate)` implementation details.
    - The only newly-added bare-public candidate was `agent_agui::session_of`, introduced solely so the Tauri relay could stamp `AgentStreamEvent.session_id`. Moved that helper into `src-tauri/src/bridge/agent.rs` as a private function; no user-facing behavior changed and no server-side Clay JS surface was created.
    - `AgentStreamEvent.session_id` and the generated DTO/protocol types remain desktop/webview wire contract data, not server JavaScript APIs. The runtime facade table, `api-inventory.toml`, Markdown API docs, generated registry, and `docs/index.md` therefore require no new entries.
    - No changed behavior grants filesystem, network, shell, workspace, package, AI-mutation, raw-op, or client-side-JavaScript authority to a Clay JS caller. Existing curated facades remain the only public programmatic boundary.
    - Added `plan119_session_relay_helper_is_not_a_public_programmatic_surface` to prevent the relay helper from becoming a public Rust or facade export again.
  - Verification:
    - `cargo fmt --all -- --check` passed.
    - `cargo test --lib server::agent_agui::tests`: 14 passed.
    - `cargo test -p clay-desktop --lib bridge::agent::tests`: 5 passed.
    - `cargo test --test security -- rust_visibility_api_mapping`: 6 passed.
    - `cargo test --test protocol -- clay_js_api_inventory`: 14 passed; `-- clay_js_doc_registry`: 51 passed; `-- clay_js_facade_layout`: 6 passed.
    - `cargo test --test protocol -- documentation_coverage`: 11 passed; `cargo check --all-targets` and `cargo clippy --all-targets -- -D warnings` passed.
    - Existing facade inventory/op-mapping coverage remains green: 24 curated modules, 14 public module entries, and the controlled-module op mapping test (1 test each).

- [x] Create or verify Clay configuration surfaces
  - Acceptance Criteria:
    - Functional: review changed behaviors (chunked open, MCP connect floor, workspace-scoped agents, delivery policy); confirm none introduces a user-facing configuration option; record the review conclusion.
    - Performance: n/a.
    - Code Quality: if any option is found to be needed (e.g., an MCP connect-timeout override), it lands as a documented Clay JS API with `custom_properties` coverage — not an undocumented key.
    - Security: configuration never implicitly grants workspace/network/shell authority.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
      - `docs/reference/clay-js-api/configuration.md` and the configuration API docs/inventory.
      - `examples/config/init.js` and `examples/config/README.md` (canonical copy-safe configuration and file-backed agent settings).
      - `src/server/configuration.rs` (closed package-option validator and authority rejection) and its configuration tests.
      - `src/server/agent_mcp_config.rs` and `docs/wiki/modules/clay-agent.md` (existing `mcp.json`/`.mcp.json` contract).
      - `clay-agent/src/mcp.ts` and `clay-agent/src/__tests__/mcp-v2.test.ts` (Plan 119 connection floor versus existing call timeout).
      - `src/perf/budgets.rs` and `src/server/connection/delivery.rs` (host-owned budgets and delivery policy).
    - Options Considered:
      - Add `init.js` keys for chunk sizes, resident-memory limits, position-index segmentation, delivery policy, session roots, or MCP handshake timeout: rejected as internal correctness/security/performance policy and speculative configuration.
      - Reclassify the existing per-server `mcp.json` `timeoutMs` as a new Clay JS API: rejected; Plan 119 preserves the Plan 117 file-backed call-timeout contract and only fixes its interaction with the bridge handshake.
      - Verify-only. (Chosen.)
    - Chosen Approach:
      - Confirm zero new user-facing configuration surface; keep agent workspace scoping structural (session root follows the tab/workspace model), keep the 5-second MCP connection floor host-owned, and leave existing documented APIs/config files unchanged.
    - Files to Create/Edit:
      - None expected: verification found no new `init.js` API, option, key binding, package load, trust declaration, API-registry entry, or canonical-example change.
    - References:
      - `src/perf/budgets.rs` (internal ceilings stay internal).
      - `src/server/configuration.rs::validate_package_option_name` and `reject_prohibited_authority` (closed, authority-free package options).
      - `src/server/agent_mcp_config.rs::build_mcp_allow_list` (explicit user/repository MCP sources and bounds).
  - Test Cases to Write:
    - None expected; existing configuration, registry, canonical-example, documentation, and MCP contract tests cover this verify-only task.
  - Outcome — verified; no new Clay configuration surface:
    - P1-1 large-document loading remains the existing client-pulled chunk protocol with fixed `MAX_CHUNK_BYTES = 256 KiB` and `DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES = 256 MiB`; the buffer-hoist/throughput fix does not add a user knob.
    - P1-2 long-line position indexing is an internal bounded segmentation/performance implementation; no `init.js` setting or package option controls segment size, conversion scans, or index budgets.
    - P1-3 keeps the existing declarative agent `mcp.json`/workspace `.mcp.json` `timeoutMs` as a per-server tool-call ceiling. `CONNECT_FLOOR_MS = 5_000` is a fixed bridge safeguard, not a configurable handshake timeout; calls below the floor still enforce the requested timeout through the execution-context signal. No `mcp.connectTimeoutMs`, `mcp.handshakeTimeout`, or equivalent Clay JS API is needed.
    - SC-2's `Delivery::{State,Advice}` lag policy and bounded lanes are server implementation details; no queue-capacity or delivery-policy configuration is exposed.
    - SC-6 session ownership is structural: `sessionId` resolves the recorded workspace root and fails closed when the root is not live. There is no `agent.workspaceRoot`, `agent.sessionRoot`, or cross-tab authority option; session-specific MCP allow-list resolution does not create a new configuration surface.
    - Existing file-backed agent settings remain explicitly documented as complementary declarative data (`skills.json`, `mcp.json`, `SYSTEM.md`, and `tool-caps.json`), not hidden `init.js` knobs. Their existing authority posture is unchanged: explicit user/repository MCP configuration, bounded literal argv, explicit environment names, canonical executable resolution, and per-entry failure isolation.
    - The configuration boundary remains closed and trusted-only: `setPackageOption` accepts only its documented package-prefixed suffixes, rejects hidden/ad hoc/raw-authority values, and configuration evaluation stays outside typing, paint, layout, scroll, and protocol hot paths. No filesystem, network, shell, workspace, package-control, AI-mutation, or client-JavaScript authority is added.
  - Verification:
    - `node --check examples/config/init.js` passed.
    - `cargo test --lib server::configuration::tests`: 19 passed.
    - `cargo test --test protocol -- example_config_control_center_chord`: 3 passed.
    - `cargo test --test protocol -- clay_js_api_inventory`: 14 passed.
    - `cargo test --test protocol -- clay_js_doc_registry`: 51 passed.
    - `cargo test --test protocol -- clay_js_facade_layout`: 6 passed.
    - `cargo test --test protocol -- documentation_coverage`: 11 passed.
    - `(cd clay-agent && npm test)`: 149 passed, 1 skipped.
    - `git diff --check` passed.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: identify affected modules in `test-plan/index.md`; execute relevant steps on a real Linux build, recording pass/fail against numbered steps; add new numbered steps for new user-visible behavior (chunked large-file open, MCP servers connecting under small `timeoutMs`, per-workspace agent sessions/two-tab isolation, agent resume per workspace).
    - Performance: large-file open latency step added with its budget.
    - Code Quality: no existing step weakened or deleted; failures are defects or explicitly documented ceilings.
    - Security: multi-tab agent isolation steps include the cross-root negative check.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map + coverage matrix).
    - Options Considered:
      - Automated-only: plan changes user-visible flows; the manual duty applies.
      - Execute + extend. (Chosen.)
    - Chosen Approach:
      - Map plan tasks → test-plan modules; add steps; run them on the final build.
    - Files to Create/Edit:
      - `test-plan/03-files-and-workspace.md`: added F55 for the current size-scaled large-document throughput/round-trip check.
      - `test-plan/11-performance.md`: updated Q34 from the stale flat debug ceiling to the measured 25 MiB/s + 500 ms-floor budget and recorded the current run.
      - `test-plan/17-coding-agent-parity.md`: added C41–C44 and C-N14 for small-timeout slow MCP boot, workspace/session isolation, daemon restart/root recovery, root-loss denial, and the SC-4 panel review.
      - `test-plan/index.md`: added the Plan 119 coverage-matrix row and execution record.
      - `test-plan/artifacts/119-editor-agent-remediation/live-agent/`: current isolated real-app AT-SPI/drive/diagnostic evidence (portal PNG inspected then deleted because it exposed the isolated root).
    - References:
      - SC-6 verification task (live pass feeds this).
  - Test Cases to Write:
    - The added numbered steps are the deliverable.
  - Outcome:
    - Added F55, C41–C44, and C-N14; updated Q34 to the actual size-scaled budget rather than silently retaining the flaky flat debug threshold. The coverage matrix now maps Plan 119 to modules 03, 11, and 17 (with existing module 14 tab coverage retained).
    - Fresh real-app capture passed at a 1280×1104 measured viewport after AT-SPI selected Agent. It exposes the Coding Agent landmark, Transcript log, Message entry, Send action, and five Agent-detail tabs. The isolated harness had zero configuration failures and 13 agent registrations; its optional package-discovery `npm` lookup was unavailable, so this capture does not claim package-install coverage. Full fixture state/keyboard evidence remains under `code-reviews/screenshots/2026-09-15-plan119-sc4-agent-review/`.
    - The providerless live fixture cannot create two real daemon sessions, so the live two-tab visual leg remains explicitly UNRESOLVED. Real-server Layer A/B integration verifies root isolation, fail-closed root loss, and daemon-restart root recovery instead.
    - Open review findings remain visible, not waived: inspector-tab overflow (D2), approval `alertdialog` focus semantics (F3), and the pre-existing frontend Vitest unhandled `invoke` rejection.
  - Verification:
    - `cargo build --bins`, `cargo build -p clay-desktop --bins`, `clay-agent npm run build`, and `frontend npm run build` passed.
    - `scripts/capture-ui-review.sh --fixture ui-review-coding-agent ...` passed; retained redacted evidence is under `test-plan/artifacts/119-editor-agent-remediation/live-agent/`.
    - `cargo test --test runtime large_document::`: 2 passed in 2.04 s.
    - `cargo test --test security agent_session_isolation`: 2 passed.
    - `clay-agent npm test`: 149 passed, 1 skipped.
    - `frontend npm test`: 49 files / 426 assertions passed, but Vitest exited 1 on two known unhandled `src/test/shell.test.tsx` `invoke` rejections; recorded without claiming a green frontend gate.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki workflow, quality bar, and archive policy.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code. (Chosen.)
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Plan 119 implementation map and links for each ownership boundary.
      - `docs/wiki/flows/document-chunked-loading.md`: P1-1 hoisted read buffer, UTF-8 carry, throughput guard, and real-server test paths.
      - `docs/wiki/modules/desktop-typed-bridge.md`: SC-1 ts-rs DTO source of truth, generated bindings, intentional narrowing, and staleness guard.
      - `docs/wiki/modules/server-ipc-skeleton.md`: SC-2 delivery policy, lane behavior, and unit-test coverage.
      - `docs/wiki/modules/agent-process-manager.md`: SC-3 module ownership, daemon-handle clearing, and respawn behavior.
      - `docs/wiki/modules/agent-protocol.md`, `docs/wiki/modules/clay-agent.md`, and `docs/wiki/modules/tabs-and-clients.md`: SC-6 session binding, workspace-root authority, MCP allow-list scope, and resume invariants.
      - `docs/wiki/modules/react-agui-chat-stream.md`, `docs/wiki/modules/react-client-bridge.md`, and `docs/wiki/modules/react-tabs-and-splits.md`: SC-1/SC-4/SC-6 webview contract, panel decomposition, lazy lane, and per-tab store lifecycle.
      - `docs/wiki/modules/server-file-workspace.md` and `docs/wiki/modules/ui-review-harness.md`: P1/P2 file authority and Plan 119 live UI review evidence.
  - Test Cases to Write:
    - Manual wiki review: the master index links relevant pages and updated pages explain what changed implementation does and how it works.
  - Outcome — complete:
    - Updated the master index with a Plan 119 implementation map covering chunked loading, generated bridge bindings, delivery lanes, workspace-scoped agent authority, daemon lifecycle, frontend session stores, panel decomposition, and UI review evidence.
    - Verified and tightened the relevant implementation pages. They document final data flow, performance ceilings, security boundaries, source/test paths, and the deliberate boundaries: full-rope loading, `State` replay versus `Advice` drop, ts-rs narrowing, `(agent type, workspace root)` ownership, fail-closed root loss, and lazy per-tab agent stores.
    - No public Clay JS API, configuration key, runtime authority, or executable code was added by this documentation task. No archive page was linked into the hot-path index.
  - Verification:
    - `cargo test --test protocol -- wiki_index_links_every_wiki_page` passed.
    - `cargo test --test protocol -- wiki_navigation_is_complete_and_current_page_paths_resolve` passed.
    - `cargo test --test protocol -- manual_smoke_docs` passed (26 tests).
    - `git diff --check` passed; artifact path scan found no retained host `/tmp`, `/home`, or `/run` paths.

## Compromises Made

- Known constraint at planning time: PF-1 (lsp-shared `VersionedDocument` O(n)-per-edit) is deferred — capped in practice by `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` (256 KiB) on the analysis route; upgrade path (port the incremental `BytePositionIndex`) recorded in the review.
- SC-5 (single AG-UI envelope end-to-end) is limited to what SC-1's generated contract gives; full envelope unification across stdio/rkyv/JSON hops stays out of scope.
- `frontend-budget` (new, SC-4): the panel decomposition costs ≈0.5 kB gzip
  and the total ceiling (400 kB, `frontend/scripts/bundle-budget.mjs`) had
  ≈0.1 kB headroom, so `check:budget` is red at 400.3 kB. Options: raise the
  ceiling by 0.5% (needs a decision-log entry — a policy number, not a
  mistake to paper over) or fold inspector leaves back into fewer files
  (measured: no saving; the cost is the new plumbing, not module count). Not
  re-scoped unilaterally; the shell budget (173.6 / 180 kB) still passes.
- Pre-existing red gates on the working tree at plan start (in-flight plan 118 work, not plan 119): `frontend-lint` exits 1 (12 errors / 4 warnings, including a real `rules-of-hooks` violation in `frontend/src/shell/WorkspacePanes.tsx:62`; the rest is the 2026-09-11 review's P1-5 class), and `frontend-test` exits 1 purely from Vitest's unhandled-rejection accounting (45 files / 391 tests pass; `src/test/shell.test.tsx` `invoke` undefined). Plan 119's final verification must not claim a green baseline without either fixing these or explicitly re-scoping them via a decision.

## Further Actions

All three items are complete (2026-09-15); evidence is in
`code-reviews/screenshots/2026-09-15-plan119-further-actions/review-log.md`.

- [x] P1: fix the pre-existing `src/test/shell.test.tsx` unhandled `invoke` rejections and rerun the frontend gate until its process exit matches its passing assertions.
  - Root cause: fire-and-forget bridge calls (`void adapters.send(...)`, layout persistence, SDUI intents, `void workspace.openX()`) attached no rejection handler, so a bridge without `invoke` produced unhandled rejections and production silently dropped the command.
  - Fix: one helper `frontend/src/lib/detached.ts` now wraps every intentional fire-and-forget call; waiting callers keep their own error paths. Also fixed the four remaining lint errors (test non-null assertions) and the `WorkspaceRail` memo (now keyed on the immutable CodeMirror `Text`).
  - Verified: `frontend npm test` 49 files / 426 tests, **exit 0**; `npm run lint` 0 errors / 0 warnings; `npx tsc --noEmit` clean.
- [x] P2: repeat the C44 live two-tab GUI leg with a configured provider and input-capable desktop host; resolve or explicitly re-approve inspector-strip overflow (D2) and approval `alertdialog` focus semantics (F3).
  - D2 resolved by re-approving the scroll the approved artifact itself declares (`.inspector .tabs { overflow-x: auto }`) and adding the missing affordance: the inspector tab strip now shows the host's standard thin scrollbar (measured 303/337 px overflow at 1440/1280/1024; `scrollbar-width: none → thin`).
  - F3 resolved: `ApprovalStrip` moves focus to the first action on appearance and returns it to the previously focused control on resolution, with a new panel test and a live fixture check (`document.activeElement` = Allow).
  - Live two-tab leg re-run with a configured mock provider (seeded vault + `book.json`), two tabs rooted at distinct scratch folders, real server + real daemon + real desktop client: both tabs restore with their own root (AT-SPI shows `ws-a`/`ws-b` and ws-a's `MARKER-A.txt`). Prompting and per-tab agent binding still need keyboard input, which this host cannot synthesize (`/dev/uinput` is root-only for ydotool; the portal grant is interactive), and one live `session.new` hit the server's 30 s ceiling although the same daemon creates the session in 8 ms standalone. Recorded, not waived, with a rerun recipe (`review-log.md` §4).
- [x] P2: revisit the 400 kB frontend gzip ceiling before adding more agent-surface plumbing; either reclaim the recorded ≈0.5 kB or make a policy decision with evidence.
  - Reclaim attempt found no dead code in the production graph (DEV-only fixture route and unused exports already tree-shaken; no `react-aria-components`; one unused CSS class removed during the audit).
  - Policy decision: `decision-logs/2026-09-15-1153-frontend-total-bundle-ceiling-404-kb.md` raises the total ceiling **400 → 404 kB (+1%)**, keeping the guard meaningful; mirrors updated (`bundle-budget.mjs`, `docs/development/performance.md`, `tests/performance_budgets.rs`, `test-plan/11-performance.md`, wiki, parity checklist).
  - Verified: fresh build + `check:budget` exit 0 (shell 173.7 / 180 kB, total 400.3 / 404 kB).
