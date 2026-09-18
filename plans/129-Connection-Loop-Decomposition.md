# 129 — Connection Loop Decomposition and Future-Size Reduction

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
C1, P4 (review §4, §2). Continues plan 119's SC-2 (which extracted the delivery
policy) by extracting per-family message handling out of `handle_connection_loop`
(`src/server/connection/mod.rs` L514–L1587, 1,073 lines, 31 arms, ~68 awaits).
Pure refactor: observable protocol behavior unchanged.

## Objectives

- C1: `handle_connection_loop` becomes a thin select router; each of the 31
  `ClientMessage` arms calls a named, individually testable handler in its
  submodule (`documents.rs`, `menus.rs`, `tabs.rs`, `workspace.rs`, `runtime.rs`, …).
- P4: as a consequence (plus explicit `Box::pin` on rare arms if still needed),
  no async fn on the connection path has a future > ~8 KB; the 75 clippy
  `large_future` warnings (40×20,432 B / 21×21,608 B / 7×27,976 B …) drop to zero
  on this path.
- Zero behavior change: all connection suites
  (`src/server/connection/tests.rs`, 8,558 lines) pass unmodified.

## Expected Outcome

- `handle_connection_loop` is < ~250 lines: subscriptions + select + one match
  dispatching to `handle_<family>` functions; per-family handlers live in their
  existing submodules with unit-testable signatures.
- `cargo clippy --all-targets -- -W clippy::large_future` reports no connection-path
  warnings (record before/after counts).
- All Linux gates green.

## Tasks

- [ ] Baseline gates, loop metrics, and future-size inventory
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full set passes untouched; record exit codes.
    - Performance: record current clippy `large_future` count/locations for `src/server/connection/**` and `src/server/mod.rs`; record loop length/arms as static metrics.
    - Code Quality: no code changes.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §4 (C1), §2 (P4).
      - `plans/119-Editor-and-Agent-Architecture-Remediation.md` SC-2 task (delivery-policy extraction precedent and its evidence format).
    - Options Considered:
      - Skip metrics: refactor's success criteria are these numbers. Recorded.
    - Chosen Approach:
      - Static counts + clippy inventory stored with plan evidence.
    - Files to Create/Edit:
      - None.
    - References:
      - `src/server/connection/mod.rs:514–1587`.
  - Test Cases to Write:
    - None (evidence-recording task).

- [ ] Define the handler context object and extract the mechanical arms
  - Acceptance Criteria:
    - Functional: a `ConnectionCtx<'_>` (or small struct of the loop's shared state: stream handle, client_id, document/behavior/workspace Arcs, coordinators, codec, subscriptions as needed) is passed to extracted `handle_*` fns; at least the straightforward arms (ListDocuments, GetDocumentStatus, ListLauncherEntries, RemoveLauncherRecent, ListAgentSettingsFiles, OpenAgentSettingsFile, OpenSelectedFile, RequestResync, CloseDocument, SaveDocument, ReloadDocument, DocumentChunkRequest) are extracted.
    - Performance: none (refactor).
    - Code Quality: each handler ≤ ~120 lines; no `clone` introduced solely for extraction (Arc clones only where the handler outlives the borrow — prefer keeping handlers borrowing the ctx).
    - Security: authorization checks (`document_for_message`, tab binding checks, session binding) move with their arms unchanged — each extraction's diff shows the check preceding state access.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection/mod.rs` (loop + existing helpers `message_requires_tab_state`, `route_connection_tab_state`), submodules `documents.rs`/`menus.rs`/`tabs.rs`/`workspace.rs`/`runtime.rs` (existing extraction targets).
      - `.agents/skills/clay-execution/references/protocol-perf.md` (connection-path perf invariants).
    - Options Considered:
      - One giant `match` moved to a new file: moves the problem.
      - Context struct + per-family handlers in existing submodules. (Chosen.)
      - Message-handler registry (map from discriminant to fn): indirection without benefit at 31 arms — rejected.
    - Chosen Approach:
      - Incremental extraction, one family per commit-sized step, suites run between steps.
    - API Notes and Examples:
      ```rust
      // src/server/connection/documents.rs
      pub(super) async fn handle_save_document<S: AsyncWrite + Unpin>(
          ctx: &mut ConnectionCtx<'_, S>, msg: SaveDocument,
      ) -> Result<(), CodecError> { … }
      ```
    - Files to Create/Edit:
      - `src/server/connection/mod.rs` (ctx + router), `src/server/connection/documents.rs`, `menus.rs`, `tabs.rs`, `workspace.rs`, `runtime.rs`.
    - References:
      - Plan 119 SC-2 evidence (loop already thinned once; same method).
  - Test Cases to Write:
    - Existing suites unmodified and green after each extraction step (this is the regression net; no new tests for moved code).

- [ ] Extract the stateful arms (Edit, EditorIntent, CompletionRequest, LanguageIntelligenceRequest, SduiAction, ViewportRenderRequest, Agent, Hello, RuntimeGenerationInstalled, Menu* family)
  - Acceptance Criteria:
    - Functional: loop is a thin router (< ~250 lines); all 31 arms dispatch; subscription select arms stay in the loop.
    - Performance: clippy `large_future` warnings on the connection path eliminated (Box::pin only where a rare arm still oversized — justified in code comment).
    - Code Quality: handlers individually callable from tests; at least two new unit tests exercise extracted handlers directly (previously only reachable through the loop).
    - Security: per-arm authorization and budget checks unchanged (diff-level review recorded in evidence).
  - Approach:
    - Documentation Reviewed:
      - Same as task 2; `src/server/connection/runtime.rs` (completion/LI arms to extract, following plan 126's changes if already landed — coordinate ordering).
    - Options Considered:
      - Extract everything at once: high-risk single step.
      - Two-stage (mechanical first, stateful second). (Chosen.)
    - Chosen Approach:
      - Stateful arms last; run connection suite after each.
    - Files to Create/Edit:
      - `src/server/connection/mod.rs` + submodules as above.
    - References:
      - Review C1, P4.
  - Test Cases to Write:
    - `handler_direct_unit_tests`: two extracted handlers invoked with crafted ctx + message asserting response stream content.
    - Existing 8,558-line connection suite green (regression net).

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: core editing, multi-cursor, caret/typography, and command modules re-run on a real Linux build; record pass/fail (pure refactor — expected all-pass; any failure is a defect).
    - Performance: none beyond perceived-input-latency sanity note.
    - Code Quality: no new manual steps expected; record that explicitly with the reason (internal refactor with automated regression net) if none are added.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`; `.agents/skills/create-plan/references/clay.md` (explicit-record rule for internal changes).
    - Chosen Approach:
      - Regression-only manual pass.
    - Files to Create/Edit:
      - None expected; record executed steps in task evidence.
    - References:
      - None beyond index.
  - Test Cases to Write:
    - None.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed; all extracted handlers `pub(crate)`/`pub(super)`; verify via diff.
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
    - Functional: connection-handling wiki page(s) reflect the router/handler structure with the per-family map.
    - Performance: notes future-size reduction and its cause (per-arm state now boxed in smaller futures).
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: documents that authorization checks live at handler entry points.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`.
    - Chosen Approach:
      - Update once after tests pass.
    - Files to Create/Edit:
      - `docs/wiki/modules/*.md` (connection page — locate via index), `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
