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

- [ ] Baseline: package suites and per-edit cost table on the unmodified tree
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

- [ ] Port the incremental line index into lsp-shared
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

- [ ] Verify language adapters and real-server suites against the new index
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

- [ ] Execute and update the manual test plan (test-plan/)
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

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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

- [ ] Update or verify the code wiki after implementation
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

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
