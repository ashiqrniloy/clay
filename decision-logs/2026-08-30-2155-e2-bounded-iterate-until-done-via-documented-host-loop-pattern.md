---
date: 2026-08-30 21:55
status: approved
decision_about: "Bounded iterate-until-done orchestration pattern (E2)"
proposed_by: both
explicitly_approved_by_user: true
---

# Decision: E2 resolved — adopt the documented Prism host-loop pattern

## Decision

Clay's bounded iterate-until-done orchestration (the `st` build/fix
loops) adopts the **documented Prism 0.3.2 host-loop pattern**: one
`runWorkflow` per iteration (task / implement / test / validate),
iteration state in workflow inputs, explicit termination predicates and
budgets with typed fail-closed `BudgetExhaustedError`, and
`replayWorkflow` per iteration run id for audit and resume.
`examples/autonomous-coding-loop.ts` in the Prism repo is the
conformance reference. An in-graph `loop` node is explicitly deferred
upstream (Prism Plan 045); Clay does not wait for it.

## Context

Prism workflows are acyclic and revision-fingerprinted by design; `st`
needs "loop until the goal is achieved" semantics. Open decision 3 asked
whether this lands as a documented host pattern or a new Prism
primitive. Prism 0.3.2 (plan 050) shipped FEATURE-2 (bounded
iterate-until-done docs) + FEATURE-6 (composite conformance example
matching st's exact shape: goal → roadmap → per-task supervisor children
→ goal-verify → OM attach/compact/recall → human gate with simulated
restart → host-side bounded loop with deterministic budget exhaustion).

## Approval

- Proposed by: both (agent analysis, user adoption)
- Approved by user: Yes
- Approval evidence: user requested the E2 analysis against latest
  Prism updates to be documented in the roadmap; the analysis
  recommended the documented host-loop pattern (Option A) over waiting
  for the in-graph primitive (Option B); user accepted and moved to the
  next decisions. Approved for logging: "Go ahead and log the decisions
  as decided already" (2026-08-30).

## Alternatives Considered

1. **In-graph `loop` node / `iterateUntil` primitive** — rejected for
   now: explicitly deferred upstream (Prism Plan 045, no ship date);
   would block Phase 5 on unreleased work; even when shipped it cannot
   own Clay's loop-internal policy (git branch/commit, frozen-plan
   reruns, typed feedback tasks, decision gates). Revisit only if Prism
   ships it and the host loop shows measured pain.
2. **Undocumented ad-hoc host loop** — superseded by the documented
   pattern with typed budget semantics; the historical risk (sloppy host
   loops that hang or overrun) is closed by `BudgetExhaustedError`.

## Rationale and Evidence

- Prism 0.3.2 `docs/workflows.md` documents the pattern; the changelog
  states Plan 045's loop node "remains the future in-graph primitive;
  this intake ships the docs+example minimum only."
- Available today at 0.3.2 — Phase 5 unblocked immediately.
- Termination/budget semantics have a typed contract (fail-closed, never
  a hang); audit/resume granularity is per-iteration run ids plus
  `replayWorkflow`, complementing Clay's checkpoints +
  `definitionRevision`.
- Cost accepted: the daemon owns the iteration cursor and double-run
  guards.

## References

- `roadmap.md` — Phase 0 (verification), Phase 5 (orchestrator pattern),
  resolved item 3.
- `@arnilo/prism-workflows@0.3.1`, `docs/workflows.md`,
  `examples/autonomous-coding-loop.ts` (Prism 0.3.2).
- `docs/clay-integration-findings.md` — FEATURE-2/6 intake items.

## Consequences

- Positive: no upstream dependency; vendor-endorsed pattern with
  conformance example; deterministic budget exhaustion.
- Risks: host-owned cursor state; mitigated by checkpoints and
  `definitionRevision` fingerprinting.
- Revisit if: Prism ships the loop node and iteration latency or state
  serialization overhead measurably hurts.