# 132 — Fanout and Protocol-Contract Deduplication

Source: `code-reviews/2026-09-18-comprehensive-implementation-review.md` items
U1, U2, C5 (review §5, §4). Two deduplications: the five reimplemented
"broadcast + current-value store + lag replay" lanes, and the remaining
hand-mapped Tauri DTO layer. No wire-format or behavior changes.

## Objectives

- U2/C5: one generic `StateFanout<T>` (bounded broadcast + current-value store +
  subscribe + override getter + lag-replay semantics) replaces the five
  hand-rolled lanes (`editor_commands`, `caret_styles`, `editor_layouts`,
  `shell_preferences` in `src/server/js_runtime/mod.rs`; `ActiveTypographyState`
  and the state lanes of `RuntimeGenerationStore` in `src/server/mod.rs`), and
  collapses the repeated `.lock().expect(...)` ladders (20+ in `js_runtime/mod.rs`,
  ~30-line `production_reload` wiring).
- U1: extend the plan-119 ts-rs codegen (DTO → TS) to eliminate the remaining
  hand-maintained mapping inside `src-tauri/src/bridge/dto.rs`, so each protocol
  message family is defined once in Rust and projected, not transcribed.

## Expected Outcome

- One `StateFanout<T>` implementation with unit tests; the five lanes become
  field declarations; connection-loop subscription code unchanged in shape
  (`subscribe_*` methods remain, now delegating).
- `dto.rs` shrinks to projections/renames only (or is generated); a staleness
  check fails CI when Rust DTOs and generated TS drift (extending the existing
  `scripts/check-bindings.sh` pattern).
- All Linux gates green (root + clay-desktop + frontend typecheck/suite).

## Tasks

- [ ] Baseline gates and duplication inventory
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` full set + clay-desktop gates pass untouched; record exit codes.
    - Performance: none.
    - Code Quality: record the five lanes' current implementations (file:line spans) and `dto.rs`'s hand-mapped surface (which families still manual).
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-18-comprehensive-implementation-review.md` §5 (U1, U2), §4 (C5).
      - `plans/119-…` SC-1 task evidence (ts-rs codegen decision: DTO layer, not protocol layer — this plan extends, not revisits).
    - Options Considered:
      - Skip inventory: the lane list is the work order. Recorded.
    - Chosen Approach:
      - Static inventory into task evidence.
    - Files to Create/Edit:
      - None.
    - References:
      - `src/server/js_runtime/mod.rs:57–100`, `src/server/mod.rs:166–304`.
  - Test Cases to Write:
    - None (evidence-recording task).

- [ ] Introduce `StateFanout<T>` and migrate the four js_runtime lanes
  - Acceptance Criteria:
    - Functional: a `StateFanout<T>` type (tokio broadcast + `Mutex<Option<T>>`/`Mutex<T>` state + subscribe + current + publish) with unit tests for lag-replay semantics (late subscriber gets current value; lagged receiver replays current); `editor_commands`, `caret_styles`, `editor_layouts`, `shell_preferences` become `StateFanout<…>` fields; all `subscribe_*`/`*_override` accessors delegate; `production_reload` shares fanouts by clone, no ladders.
    - Performance: no change in per-message cost (same broadcast primitive); publish path identical allocations.
    - Code Quality: js_runtime/mod.rs loses the 20+ lock ladders on these lanes; clippy clean; no behavior change in connection initial-sync/lag-replay (connection suites green).
    - Security: channel capacities and semantics (advisory drop vs replay) preserved per lane exactly — capacity constants unchanged.
  - Approach:
    - Documentation Reviewed:
      - `src/server/js_runtime/mod.rs` (five lanes + wiring), `src/server/connection/mod.rs` (consumer side: initial sync + lag replay comments per lane).
    - Options Considered:
      - Per-lane wrapper structs: five more copies of the same shape.
      - One generic with explicit replay policy (`Replay::Current` vs `Drop`), capacity in constructor. (Chosen.)
    - Chosen Approach:
      - `struct StateFanout<T> { tx: broadcast::Sender<T>, state: Arc<Mutex<T>> }` — constructors `with_capacity`, `with_initial`; `publish`, `subscribe`, `current`, `update`.
    - API Notes and Examples:
      ```rust
      // src/server/fanout.rs (tentative location)
      pub(crate) struct StateFanout<T: Clone> { … }
      impl<T: Clone> StateFanout<T> {
          pub(crate) fn publish(&self, value: T);      // send + store
          pub(crate) fn subscribe(&self) -> broadcast::Receiver<T>;
          pub(crate) fn current(&self) -> T;            // replay source
      }
      ```
    - Files to Create/Edit:
      - `src/server/fanout.rs` (new), `src/server/mod.rs` (module line), `src/server/js_runtime/mod.rs` (migrate four lanes + reload wiring).
    - References:
      - Review U2, C5.
  - Test Cases to Write:
    - `late_subscriber_replays_current`: subscribe after publish → initial sync sees latest.
    - `lagged_receiver_replays_current` (the Plan 071 semantics).
    - `capacity_preserved_per_lane`: channel capacity assertions matching current constants (16/4/4/4).

- [ ] Migrate typography/runtime-state lanes and remaining connection wiring
  - Acceptance Criteria:
    - Functional: `ActiveTypographyState` and the state lanes of `RuntimeGenerationStore` use `StateFanout` where their semantics match (replay-current); generation signaling that is *not* current-value shaped (e.g. `RuntimeGenerationId` notifications) stays as-is with a comment saying why.
    - Performance: none.
    - Code Quality: one pattern for all current-value lanes; connection-loop subscription code compiles unchanged or with mechanical delegation edits; suites green.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `src/server/mod.rs:166–304` (`RuntimeGenerationStore`, `ActiveTypographyState`, fanout structs).
    - Options Considered:
      - Force-fit non-replay lanes: wrong semantics; keep the distinction explicit. (Chosen: migrate only matching lanes.)
    - Chosen Approach:
      - Migrate matching lanes; document the non-matching ones.
    - Files to Create/Edit:
      - `src/server/mod.rs`, `src/server/fanout.rs`.
    - References:
      - Review U2.
  - Test Cases to Write:
    - Existing runtime-state/typography subscription suites green.

- [ ] Eliminate hand-mapped DTO transcription in the Tauri bridge (U1)
  - Acceptance Criteria:
    - Functional: every message family currently hand-transcribed in `src-tauri/src/bridge/dto.rs` is either generated from the Rust DTO types via the existing ts-rs pipeline or reduced to a mechanical projection (field rename/wrap) with the mapping expressed as code (serde attributes / From impls) rather than duplicated struct definitions; generated TS under `frontend/src/bridge/generated/` remains the single TS truth.
    - Performance: no runtime cost (codegen-only path; `ts-bindings` feature stays off in shipped builds).
    - Code Quality: `scripts/check-bindings.sh` (or its successor) fails on drift for the migrated families; DTO struct count in `dto.rs` reduced (record before/after).
    - Security: projection boundary unchanged — no new fields cross the bridge; the trust boundary between protocol types and webview DTOs stays explicit.
  - Approach:
    - Documentation Reviewed:
      - `plans/119-…` SC-1 task + its decision log (`2026-09-14` ts-rs-from-DTO approval) — this plan extends the same approved approach, no new decision needed unless a family's shape forces one.
      - `src-tauri/src/bridge/dto.rs`, `frontend/src/bridge/generated/bridge.ts`, `scripts/check-bindings.sh`, `Cargo.toml` `ts-bindings` feature.
    - Options Considered:
      - Hand-fix drift as found: keeps N duplicate definitions.
      - Generate/derive the remaining families with the established pipeline. (Chosen.)
    - Chosen Approach:
      - Family-by-family migration, `check-bindings` green between steps.
    - Files to Create/Edit:
      - `src-tauri/src/bridge/dto.rs` (shrink), `frontend/src/bridge/generated/bridge.ts` (regenerated), `scripts/check-bindings.sh` (extend coverage).
    - References:
      - Review U1; plan 119 SC-1 evidence.
  - Test Cases to Write:
    - Existing bridge staleness/typecheck suites green per step; add a drift test for any family that lacked one.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: internal dedup with no user-visible change — record explicitly which regression modules were re-run (core editing + theme/caret apply paths, since fanout lanes carry those) and that no new steps are needed, with the reason.
    - Performance: none.
    - Code Quality: record per `.agents/skills/create-plan/references/clay.md` explicit-record rule.
    - Security: none.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`.
    - Chosen Approach:
      - Regression-only pass on caret/typography/theme modules.
    - Files to Create/Edit:
      - None expected.
    - References:
      - None beyond index.
  - Test Cases to Write:
    - None.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: no public programmatic surface changed; verify via diff; record "no JS API change".
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
    - Functional: `docs/wiki/modules/desktop-typed-bridge.md` (and the runtime fanout page if separate) reflect `StateFanout` and the generated DTO contract.
    - Performance: notes replay semantics and capacities.
    - Code Quality: linked from `docs/wiki/index.md`.
    - Security: documents the unchanged projection boundary.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`; `docs/wiki/modules/desktop-typed-bridge.md`.
    - Chosen Approach:
      - Update once after gates pass.
    - Files to Create/Edit:
      - `docs/wiki/modules/desktop-typed-bridge.md`, fanout page, `docs/wiki/index.md`.
  - Test Cases to Write:
    - Manual wiki review.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
