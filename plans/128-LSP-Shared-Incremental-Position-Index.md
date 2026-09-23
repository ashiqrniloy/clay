# 128 — LSP-Shared Incremental Position Index

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` item D3
(review §1); open PF-1 carried from `code-reviews/2026-09-14-editor-and-agent-architecture-review.md`.
Scope: `packages/lsp-shared/positions.js` and its consumers
(`packages/lsp-javascript`, `packages/lsp-typescript`, `packages/lsp-rust`,
`packages/lsp-markdown`). No Rust changes.

## Objectives

- Port the incremental UTF-16↔UTF-8 line/byte index (the frontend's
  `BytePositionIndex` pattern, `frontend/src/editor/position-index.ts`) into
  `lsp-shared`, replacing `VersionedDocument`'s full rebuild per edit.
- Keep the existing `VersionedDocument` public surface
  (`reset`, `applyIncremental`/`applyFull`, `byteToPosition`, `positionToByte`,
  `#lineBounds`) so all four language adapters and `bridge.js` work unchanged.
- Keep the edit path O(edit + log lines), conversions O(log lines + intra-line scan).

## Expected Outcome

- Typing latency on the LSP route is flat in document size: an edit on a 1 MiB
  document costs the same order as on a 64 KiB one (regression test with the
  2026-09-14 probe numbers as the "before" record).
- All four `packages/lsp-*` package suites and real-server smoke tests pass
  unchanged; `lsp-shared` adapter tests pass.
- No behavior change in LSP position fidelity (existing round-trip tests).

## Tasks

- [x] Baseline: package suites and per-edit cost table on the unmodified tree
  - Acceptance Criteria:
    - Functional: lsp-shared adapter tests + four language package suites + `lsp_real_servers` suite pass untouched; record exit codes.
    - Performance: reproduce the 2026-09-14 per-edit cost table (64 KiB / 256 KiB / 1 MiB / 8 MiB, median of repeated edits) in this repo's environment as the pre-change baseline.
    - Code Quality: no code changes.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §PF-1 (probe method + numbers).
      - `packages/lsp-shared/positions.js` (current `reset`-per-edit implementation).
    - Options Considered:
      - Reuse 2026-09-14 numbers only: different machine; fresh baseline chosen.
    - Chosen Approach:
      - Node one-shot probe mirroring the review's method; store under `code-reviews/` evidence path.
    - Files to Create/Edit:
      - None.
    - References:
      - `packages/lsp-shared/adapter.test.mjs`, `tests/suites/lsp_bridge.rs`, `tests/suites/lsp_real_servers.rs`.
  - Test Cases to Write:
    - None (evidence-recording task).
  - Outcome (2026-09-20):
    - Evidence: `code-reviews/2026-09-19-plan128-baseline/` (README with
      tables, per-suite logs, `logs/run-lsp-suites.sh`, probe source
      `probe-lsp-edit-cost.mjs.txt`).
    - Tree: HEAD `66648e3` + the 24-file plan 126/127 uncommitted diff
      (plan-128-untouched; no `packages/`/`tests/lsp_*` files in it);
      `git diff | sha256sum`
      `f45352fa…0d73a0` verified identical before and after the run — the probe
      ran from `/tmp`, so the tree never held it.
    - Suites: adapter 19/19, four package suites 3/3 each, `lsp_bridge` 11/11 —
      all exit 0. `lsp_real_servers` (gated) exit 101: the rust-analyzer real
      smoke fails deterministically (3/3 runs) at
      `rust-real-smoke.test.mjs:93`, because the bridge publishes an empty
      `kind: "inlayHint"` decoration set right after the 20-span semantic set
      and the test asserts on `decorations.at(-1)`; `inlayHint` and that test
      were last changed together in `a10cb72` (2026-08-21), so the red
      predates this plan and the 2026-09-14 review. Markdown real smoke passes
      standalone; TypeScript/JavaScript real smoke skipped —
      `typescript-language-server` is not installed on this host.
    - Cost (median of 7×20 `applyByteChange` edits, Node 24): 64 KiB
      **2.30** ms, 256 KiB **10.21**, 1 MiB **33.29**, 8 MiB **214.15**
      (max 336.42) — linear in document size; matches the 2026-09-14 review's
      2.5 / 12.4 / 45.3 / 214.6 shape (8 MiB within 0.3%). Full-sync
      `reset()` medians: 0.96 / 4.73 / 14.24 / 79.29 ms.

- [x] Port the incremental line index into lsp-shared
  - Acceptance Criteria:
    - Functional: `VersionedDocument` maintains an incremental index (chunked line widths with subtree sums; single-line segments for long lines optional if the simpler chunked treap already meets budget); `applyIncremental` rebuilds only touched lines; all existing adapter tests pass.
    - Performance: per-edit cost on 1 MiB ≤ 10× the 64 KiB cost is a failure of this task — target: same order (constant in document size); regression test asserts a ratio bound with margin (e.g. 1 MiB edit ≤ 3× 64 KiB edit).
    - Code Quality: no dependencies added (`Uint32Array`-based, mirroring the frontend implementation's shape); file stays under ~400 lines including tests; adapters unchanged.
    - Security: index operates on host-owned text only; no new parsing of untrusted input (line splitting on `\n` unchanged; CR handling preserved).
  - Approach:
    - Documentation Reviewed:
      - `frontend/src/editor/position-index.ts` (the reference implementation: `CHUNK_LINES`, treap/priority structure, `pushLine`, `replaceRange`).
      - `frontend/src/editor/position-map.ts` (`utf8Length`/`utf8Width` helpers to mirror in JS without CodeMirror deps).
      - `packages/lsp-shared/positions.js:109–210` (public surface to preserve); `packages/lsp-shared/utf8.js` (existing encode/decode helpers).
    - Options Considered:
      - Import the frontend module: drags `@codemirror/state` into lsp-shared — rejected.
      - Copy-and-adapt the frontend algorithm (2026-09-14 review's explicit recommendation: "copy it, don't design a new one"). (Chosen.)
      - Simpler per-line `Map<line, cumulative>` with rebuild-on-edit-of-prefix: still O(n) worst case on early edits — rejected.
    - Chosen Approach:
      - Self-contained `positions.js` internal index: chunked line array with a small order-statistic tree over (lines, bytes), path-copied on edit; `applyIncremental` splices only the changed line range.
    - API Notes and Examples:
      ```js
      // internal, not exported:
      // buildIndex(text), spliceLines(index, fromLine, removedCount, addedTexts),
      // lineOfByte(index, byte), byteOfLineStart(index, line)
      // public surface unchanged:
      doc.applyIncremental({ range, text }); doc.byteToPosition(offset)
      ```
    - Files to Create/Edit:
      - `packages/lsp-shared/positions.js`: internal rewrite, public surface unchanged.
      - `packages/lsp-shared/positions.test.mjs` (new or extend `adapter.test.mjs`).
    - References:
      - Review D3; PF-1 (2026-09-14).
  - Test Cases to Write:
    - `edit_cost_flat_in_document_size`: 64 KiB vs 1 MiB vs 4 MiB, ratio bound.
    - `position_roundtrip_after_incremental_edits`: randomized edits + round-trip byte→position→byte vs a naive re-implementation (test oracle).
    - `crlf_and_multibyte_lines_track`: CR-stripping and UTF-16 unit conversion match current behavior.
    - `long_single_line_does_not_regress`: 1 MiB single-line conversion cost bounded (mirror of the frontend P1-2 segment test, simplified).
  - Outcome (2026-09-20):
    - Implemented in `packages/lsp-shared/positions.js` (399 lines): a path-copied
      treap of ≤64-line chunks (`makeLeaf`/`join`/`split`/`replaceRange`, the
      frontend's `CHUNK_LINES`/priority/xorshift shape) whose leaves carry the
      line strings plus `Uint32Array` `l16`/`l8` widths and subtree sums. The
      adapted piece vs the frontend: leaves also own the line text because
      lsp-shared has no CodeMirror rope (`#text`/`bytes` are lazy caches built
      from the tree on demand; `bytes` is materialized only when a consumer
      reads it, as the bridge analysis refresh does).
      `applyByteChange` locates both byte offsets in O(log chunks + line scan),
      splices only the touched line range (`region.split("\n")`), and returns
      the same range payload; `reset` still rebuilds fully by design (full-sync
      path). No new dependency; adapters, `bridge.js`, `client.js`, and
      `mapping.js` unchanged.
    - Behavior: byte→position/position→byte parity is exact for CR-stripping
      (including a trailing CR at EOF), LF/CRLF line endings, split-scalar and
      line-ending rejections with identical messages, encodings utf-8/16/32, and
      version/boundary errors. Verified by a temporary side-by-side fuzz against
      the pre-change implementation (`fuzz-parity.mjs.txt`: 25 seeds × 60 random
      edits × 3 encodings, every offset/position plus error messages) and by the
      in-suite naive line-table oracle.
    - Tests: four new cases in `packages/lsp-shared/adapter.test.mjs` (the file
      the Rust harness already runs, so no Rust change): 23/23 pass, ~0.5 s
      total. Ratio bound green (1 MiB ≤ 3× 64 KiB, 4 MiB ≤ 4× 1 MiB); the
      randomized oracle does 120 edits × 3 encodings with full sweeps.
    - Performance: probe re-run (`logs/probe-edit-cost-post-task2.txt`):
      `applyByteChange` median 0.0143 / 0.0143 / 0.0167 / 0.0210 ms at 64 KiB /
      256 KiB / 1 MiB / 8 MiB (was 2.30 / 10.21 / 33.29 / 214.15) — flat within
      1.5× over a 128× size range, ~160× to ~10 000× faster. Full-sync `reset`
      also improved (0.27 / 1.17 / 5.20 / 40.61 ms).

- [x] Verify language adapters and real-server suites against the new index
  - Acceptance Criteria:
    - Functional: all four `packages/lsp-*` package tests + real-smoke tests pass; `bridge.js` consumers unchanged.
    - Performance: adapter-level edit/convert timings recorded post-change next to task-1 baseline.
    - Code Quality: no adapter source changes required (surface preserved); if an adapter hits a gap, the gap is fixed in lsp-shared, not worked around in the adapter.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `packages/lsp-shared/bridge.js` (`exactDocument` usage of `VersionedDocument`), `packages/lsp-shared/client.js`.
    - Options Considered:
      - Re-run only unit tests: real-server suites exercise the didChange→position path end-to-end — run them. (Chosen.)
    - Chosen Approach:
      - Full package suite set + `tests/suites/lsp_real_servers.rs`.
    - Files to Create/Edit:
      - None expected.
    - References:
      - `scripts/package-smoke.sh`.
  - Test Cases to Write:
    - None beyond task 2 (verification task).
  - Outcome (2026-09-20):
    - Evidence: `code-reviews/2026-09-19-plan128-task3/` (run script, per-suite
      logs, adapter probe before/after, README).
    - Suites: adapter 23/23, four package suites 3/3 each, `lsp_bridge` 11/11 —
      exit 0. Markdown real smoke (marksman) 1/1 pass when run directly (the
      cargo suite panics before reaching it). `lsp_real_servers` exit 101 for
      the rust-analyzer smoke — identical failure
      (`rust-real-smoke.test.mjs:93:12`, empty trailing `inlayHint` payload) to
      a HEAD-worktree run, so the red predates the index and is unchanged. JS/TS
      real smokes skipped: `typescript-language-server` absent on the host (the
      harness's documented skip path).
    - Adapter-level cost (probe drives `LspClient#changeDocument` + bridge
      viewport read + `semanticTokensToClay`/`diagnosticsToClay`; before from a
      HEAD worktree): edit+refresh median went 2.805 / 13.388 / 37.293 /
      289.619 ms → 0.029 / 0.039 / 0.030 / 0.026 ms at 64 KiB / 256 KiB / 1 MiB /
      8 MiB (flat in size); mapping conversions ~0.06–0.21 ms per payload.
    - Gap found and fixed: the first probe showed edit+refresh still linear
      (1.29 / 6.52 / 18.97 / 113.02 ms) because `bridge.js` and `mapping.js`
      read `document.bytes.length` on every refresh and the new lazy `bytes`
      getter re-encoded the whole document. Fixed in lsp-shared as the plan
      requires: `VersionedDocument` exposes O(1) `get byteLength()` (treap UTF-8
      subtree sum) and the four production call sites use it (3 in `bridge.js`,
      1 in `mapping.js`); the `bytes` array stays for callers that need
      contents, and the adapter oracle now also asserts
      `byteLength === encodeUtf8(expected).length`. `git diff HEAD --stat`:
      6 lines `bridge.js`, 2 lines `mapping.js`, plus the lsp-shared index.
    - Pre-existing ceiling recorded, not changed: `framing.js`
      `MAX_FRAME_BYTES` (1 MiB) rejects `didOpen`/full-sync `didChange` around
      and above 1 MiB with `lsp.frame_too_large`; the probe raises its own
      budget to measure the index cost past it.

- [x] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: language-feature modules re-run on a real Linux build (hover/completion via a language server on a ≥1 MiB source file); record pass/fail; add a numbered step "language features remain responsive while typing in a large file".
    - Performance: step records perceived latency.
    - Code Quality: `test-plan/index.md` updated if steps are added.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`.
    - Chosen Approach:
      - Manual verification of the user-visible latency improvement; automated suites carry correctness.
    - Files to Create/Edit:
      - `test-plan/08-syntax-and-textobjects.md` (tentative — exact module per index), `test-plan/index.md`.
    - References:
      - None beyond index.
  - Test Cases to Write:
    - Manual steps as described.
  - Outcome: completed 2026-09-20 with one leg blocked by the host, and with the
    plan's headline live criterion turning out to be unreachable on this build.
    - Harness: `test-plan/artifacts/128-lsp-incremental-index/` (isolated
      HOME/XDG/socket/TMPDIR + `~/.rustup`/`~/.cargo` symlinks, private perf
      report dir, generated Cargo crate; `lsp`/`mid`/`large` shapes). Screenshots
      are window crops; the full-desktop capture taken while diagnosing the input
      ceiling was discarded, not retained.
    - Step added as **S35** in module 08 and question **Q43** in module 11, with
      execution records in both modules plus a `## Plan 128 manual-test-plan
      execution record` section and a coverage-matrix row in
      `test-plan/index.md`.
    - Live results: (a) 4 KiB accepted document — PASS presence: rust-analyzer and
      `rust-analyzer-proc-macro-srv` spawn inside the private root, no analyzer
      failure, `bridge.patch_delivery` p50 0.089 ms / p95 0.121 ms; (b) ≈250 KiB
      open document — FAIL, analyzer dies on the first semantic payload with
      `lsp.invalid_semantic_tokens: bounded five-integer records required` and the
      UI falls back to "Document analyzer stopped; baseline language support
      remains active."; (c) 1,258,277-byte document — PASS fail-closed
      ("Document exceeds the package analysis limit…", no rust-analyzer process)
      but the plan's "hover/completion via a language server on a ≥1 MiB source
      file" is therefore **unreachable on this build**, and a hover there returns
      `providerError` ("Language provider failed" toast); (d) typing echo and
      `server.edit_ack` on the ≥1 MiB document — **UNRESOLVED (host input)**: no
      keystroke reached the document (`Ctrl+End`, `Ctrl+B`, 60-char bursts; file
      mtime, word count and `clean` state unchanged; no `edit_apply`/`edit_ack`
      samples) because the MCP readiness probe reports
      `can_send_development_input: false` — the portal session carries no
      remote-interaction permission, so the plan-126/127 typing recipe cannot be
      replayed until that consent is granted again.
    - Performance evidence is therefore the measured adapter probe from task 3
      (edit+refresh 2.805 / 13.388 / 37.293 / 289.619 ms at HEAD → 0.029 / 0.039 /
      0.030 / 0.026 ms at 64 KiB / 256 KiB / 1 MiB / 8 MiB), not a live
      keystroke→paint number; the AT-SPI probe's ~0.9 s tree walk could not
      resolve one anyway.
    - Extra observation recorded, not chased: the `lsp` fixture's rust-analyzer
      `mismatched types` (`let value: u32 = "text";`) produced no lint-gutter
      marker after ~30 s (pixel check + 5× zoom), although the bridge does carry
      a push/pull diagnostics path.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed (lsp-shared is internal bridge infrastructure); verify via diff; record "no JS API change".
    - Performance: none.
    - Code Quality: registry/doc-guard suites pass.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
    - Chosen Approach:
      - Verify-only.
    - Files to Create/Edit:
      - None expected.
    - References:
      - Decision log 2026-05-08-1509.
  - Test Cases to Write:
    - None.
  - Outcome: completed 2026-09-20 — **no JS API change**, verified three ways.
    - Exported surface is byte-identical to HEAD: `POSITION_ENCODINGS`,
      `rootPathToFileUri`, `pathToFileUri`, `fileUriToRelative`,
      `VersionedDocument`. Inside the class, the method set only *gained*
      `byteLength` (added in task 3, adapter-facing); `text`/`bytes` moved from
      eager data fields to getters with identical read semantics, and the removed
      `lineStarts` field plus the unexported `requireByteBoundary` helper have no
      reference anywhere outside `packages/lsp-shared/` (repo-wide grep: only the
      plan text mentions `lineStarts`).
    - No Clay JS API entry mentions `lsp-shared` or `lsp-*`: they are workspace
      bridge packages, not inventory surfaces, and `VersionedDocument` has no
      consumer outside `packages/lsp-shared/` (grep: `adapter.test.mjs`,
      `client.js`, `positions.js`). The byte-range surfaces that route through the
      index indirectly (`document.edit`, `renderDecorations`, `renderFolding`,
      `language.registerLanguageIntelligenceProvider`) are unchanged in
      inventory, generated registry, and behaviour — the parity fuzz (25 seeds ×
      60 edits × 3 encodings) plus the four package suites are the behavioural
      evidence, and `docs/reference/packages/creating-packages.md`'s module table
      (CRLF normalization, surrogate rejection) still describes the module
      accurately.
    - Guard suites pass: `cargo test --test protocol` → 224 passed, 0 failed
      (covers `clay_js_api_inventory`, `clay_js_doc_registry`,
      `clay_js_facade_layout`, `documentation_coverage`, `manual_smoke_docs`), and
      `generated_registry_is_current` confirms nothing needed regenerating.
    - Nothing under `docs/reference/clay-js-api/**`, `frontend/**`, or
      `runtime/**` was touched by this plan. The working tree does carry
      `api-inventory.toml` / `clay-js-api-registry.json` edits, but they are
      plan-127 completion/language-provider documentation (last written 00:53,
      before this plan's work began at 01:31, and they mention no LSP adapter
      module), so they are not attributable here.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: the LSP bridge wiki page documents the incremental index, its invariants (path-copied chunks, O(edit + log lines)), and the shared lineage with the frontend position index.
    - Performance: wiki states the flat-in-size edit cost and the test that guards it.
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: notes the index processes host-owned text only.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`; existing LSP wiki pages via `docs/wiki/index.md`.
    - Chosen Approach:
      - Update once after tests pass.
    - Files to Create/Edit:
      - `docs/wiki/modules/*.md` (LSP page — locate via index), `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.
  - Outcome: completed 2026-09-20 in `docs/wiki/modules/first-party-lsp-bridge-packages.md`
    (page already linked from `docs/wiki/index.md`) plus one line in that index.
    - `### positions.js` was rewritten around what the code now does: the implicit
      treap of 64-line chunks, path-shared `join`/`split` (only touched nodes are
      rebuilt, untouched subtrees are shared with the previous root;
      `replaceRange` is the only structural edit), the same deterministic
      xorshift32 priorities as `frontend/src/editor/position-index.ts`, and the
      real API (`reset`, `applyByteChange`, `byteToPosition`, `positionToByte`,
      `rangeToBytes`, lazy `text`/`bytes` getters, O(1) `byteLength`, module URI
      helpers).
    - Invariants recorded: the phantom trailing `\n` per line is what makes
      `byteLength` = root `w8 − 1` O(1); CR stripping in `#lineBounds` is what
      keeps CRLF positions correct; unpaired surrogates are rejected at leaf
      build, so the tree can never hold text `encodeUtf8` refuses; no version
      history is retained.
    - Performance: the page states O(log lines + edited line + inserted lines) per
      edit, the flat-in-size measurement (0.014–0.021 ms/edit at 64 KiB/256 KiB/
      1 MiB/8 MiB, `code-reviews/2026-09-19-plan128-task3/`), the per-conversion
      bound `O(log lines + line bytes)`, and the guarding tests
      (`edit_cost_flat_in_document_size`, `long_single_line_does_not_regress`,
      the randomized round-trip oracle, CRLF/multibyte, and parity with the
      pre-change rebuild implementation).
    - Security: a new Security Boundary bullet states the index handles
      host-provided document text only, rejects unpaired surrogates at ingest and
      edit time, and range-checks every offset.
    - Lineage: `Related` links
      `../flows/frontend-edit-synchronization.md` as the CodeMirror-side
      `position-index.ts` this treap mirrors, satisfying the "shared lineage"
      criterion in both directions of the repo.
    - Pre-existing wiki staleness fixed while verifying (none of these symbols
      exist in `packages/lsp-shared/`): the old subsection documented
      `byteOffsetToPosition`, `positionToByteRange`, `normalizeRoot`, and
      `isInRoot`, plus an `applyByteChange(byteOffset, oldEnd, newText)`
      signature; the 2026-08-21 note said "`VersionedDocument` exposes
      `bytes.length`, not `byteLength`", and the manifest contract said decoration
      viewports use `bytes.length` — both are now the opposite of the shipped
      contract (`byteLength` is the O(1) accessor; `bytes.length` re-encodes).
    - Guards: `cargo test --test protocol` → 224 passed, 0 failed, which includes
      the wiki/reference coverage tests (`primitives_docs`,
      `documentation_coverage`, `manual_smoke_docs`, `performance_budgets`), so
      the index link and the `BytePositionIndex` primitive row still hold.

## Compromises Made
- (Task 2) The frontend's long-line scan blocks (`SEGMENT_UNITS`) were not
  ported: a multi-megabyte single line still costs an O(line) scan per
  conversion, exactly as the pre-change implementation did. A `ponytail:` note
  marks the upgrade path, and `long_single_line_does_not_regress` bounds it.
- (Task 2) The plan's API names `applyIncremental`/`applyFull` do not exist in
  the code; the real surface (`applyByteChange` for incremental sync, `reset`
  for full sync) was kept unchanged instead of adding unused aliases.
- (Task 2) `VersionedDocument#lineStarts` and the eager `text`/`bytes` data
  fields were dropped (no consumer outside `positions.js` used `lineStarts`);
  `text`/`bytes` are now lazy getters that materialize from the tree, so callers
  that need the arrays still pay O(n) — the viewport/refresh path no longer does
  after the task-3 `byteLength` accessor.
- (Task 3) Four adapter call sites changed (`bridge.js` ×3, `mapping.js` ×1)
  from `document.bytes.length` to the new O(1) `document.byteLength`. This is
  more than the "no adapter source changes" intent of the task-3 criteria, but
  the edit+refresh probe proved that without it every refresh still paid a full
  UTF-8 re-encode (113 ms at 8 MiB), so the plan's own outcome — flat typing
  latency on the LSP route — would not have been met. The change stays in the
  shared accessor's shape: no ad-hoc workaround in the adapters.

## Further Actions

**Routed 2026-09-20.** The six actionable items below are now scheduled in
[`plans/142-LSP-Decoration-Bounds-Analyzer-Survival-and-Smoke-Honesty.md`](142-LSP-Decoration-Bounds-Analyzer-Survival-and-Smoke-Honesty.md):
the semantic-token bound + worker death → its tasks 1–4; the analysis and frame
caps → task 5; the diagnostics gap → task 6; the `rust-real-smoke` masking and
`typescript-language-server` provisioning → task 7; live verification → task 8.
This section stays as the finding record (evidence, priorities, and the exact
observed messages).

- (Task 5) No API work is implied by this plan. If the LSP bridge later exposes
  the position index to packages (e.g. a package-facing byte-offset helper), that
  becomes a real JS API change (`api-inventory.toml`, generated registry, and the
  facade guards) rather than an internal refactor. Priority: low.
- (Task 4) **High: the Rust language route is dead for real files.**
  `packages/lsp-shared/mapping.js:3` caps semantic payloads at
  `MAX_SEMANTIC_TOKENS = 128` (rejects more than 640 integers at :134), so any
  file big enough for rust-analyzer to return more than 128 tokens — observed
  live at ≈250 KiB, i.e. ordinary source files — makes the analyzer worker die
  with `lsp.invalid_semantic_tokens: bounded five-integer records required` and
  drops the session to "Document analyzer stopped". Raise the bound (window the
  token set per viewport instead of truncating/rejecting the whole payload) and
  fix the `rust-real-smoke` assertion that hides it. Priority: high.
- (Task 4) Below that cap, `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` (256 KiB,
  `src/perf/budgets.rs:33`) and `framing.js`'s 1 MiB `MAX_FRAME_BYTES` keep
  hover/completion off every document the plan's live step asked about. Both are
  now cheap to raise relative to a full index rebuild; decide the intended
  behaviour for large files (raise, or keep the non-LSP route) before claiming
  large-file language intelligence. Priority: medium.
- (Task 4) Diagnostics probe: a rust-analyzer `mismatched types` fixture showed
  no lint-gutter marker after ~30 s even though the bridge implements push/pull
  diagnostics and `diagnosticsToClay`. Separate investigation; no relation to the
  position index. Priority: low.
- (Task 4) Host input consent: live typing legs need the portal's Remote Desktop
  dialog answered with "Allow Remote Interaction" enabled before Share (or a
  working `wtype`/`ydotoold`). Until then, typing-latency steps in modules 04, 08,
  09, and 11 stay UNRESOLVED on this host. Priority: low (environment).
- (Task 2) Pre-existing, environment-gated red found while baselining: the
  rust-analyzer real smoke asserts `decorations.at(-1).spans.length > 0` but the
  bridge publishes an empty `kind: "inlayHint"` set right after the semantic
  set, so `lsp_real_servers` exits 101 on this host. Fix the assertion to filter
  by kind (or skip empty inlay payloads) when touching that suite. Priority: low
  (gated by `CLAY_LSP_REAL_SMOKE=1`).
- (Task 2) Port scan blocks for very long single lines only if a real workflow
  hits multi-megabyte single-line files in the LSP route. Priority: low.
- (Task 3) `framing.js` caps JSON frames at 1 MiB, so documents around/above
  1 MiB cannot `didOpen` (nor full-sync `didChange`) even though the index handles
  them in tens of microseconds; decide whether the frame budget should scale
  with document size or whether large files should stay on a non-LSP route.
  Priority: medium (pre-existing, but it now caps the payoff of this plan).
- (Task 3) Install `typescript-language-server` on the dev/CI host so the
  javascript/typescript real smokes stop skipping; also make
  `lsp_real_servers` continue past a failing server so one red does not hide the
  other smokes. Priority: low.
