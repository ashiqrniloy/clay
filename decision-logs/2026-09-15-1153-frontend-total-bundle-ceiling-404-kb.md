---
date: 2026-09-15 11:53
status: approved
decision_about: "Frontend total gzip bundle ceiling: 400 kB -> 404 kB (1%)"
proposed_by: "agent (plan 119 further-action P2), completed on user instruction"
explicitly_approved_by_user: true
---

# Decision: Raise the frontend total gzip bundle ceiling to 404 kB

## Decision

The production frontend **total** gzip budget in `frontend/scripts/bundle-budget.mjs`
moves from **400 kB to 404 kB** (+1%). The startup **shell** budget stays at 180 kB.
The ceiling keeps its meaning as an unintentional-creep detector: 404 kB leaves 3.7 kB
of headroom above the current 400.3 kB build, enough for roughly two or three small
feature increments, while a real regression (tens of kB) still fails the gate.

## Context

- `npm --prefix frontend run check:budget` is **red on the plan-119 final tree:
  400.3 kB total / 400 kB**. The shell lane passes (173.7 kB / 180 kB).
- The 400 kB ceiling was set for plan 099 when the total was ≈347 kB (≈15% headroom).
  The deterministic growth since then is approved feature work, not waste:
  plan 118 (Quiet Instrument migration, design-system runtime, host fallback block)
  moved the total 381.0 → 390.9 → 392.9 kB, and plan 119's agent work
  (SC-4 panel decomposition ≈0.5 kB, SC-6 generated-contract/per-tab-store plumbing)
  reached 399.0 → 400.3 kB.
- Plan 119 recorded this at planning time: "the next editor feature has ~1 kB of
  headroom before the ceiling must move", and its further actions list the ceiling
  review as a policy question, not a code fix.

## Approval

- Proposed by: plan-119 further action P2 ("revisit the 400 kB frontend gzip ceiling …
  either reclaim the recorded ≈0.5 kB or make a policy decision with evidence").
- Approved by user: Yes — the user instructed "Complete all of the tasks from the
  Further Actions section", i.e. to take this branch of the task with evidence.
- The chosen number (404 kB) is the agent's proposal inside that instruction; the user
  may revise it without re-opening the analysis.

## Alternatives Considered

1. **Reclaim ≈0.5 kB and leave the ceiling at 400 kB** — not achievable without
   deleting user-facing code. Evidence from this pass: the DEV-only fixture route
   (`frontend/src/routes/fixture.tsx`, 38 kB source) and unused exports are already
   tree-shaken out of the production graph (verified by grepping the built chunks);
   `react-aria-components` is not bundled; the unreferenced-CSS scan across all
   `*.module.css` files finds one leftover class (`sessionEmpty`, tens of bytes) plus
   CodeMirror global selectors that are used implicitly. Even a successful 0.4 kB
   reclaim would leave 0.0 kB headroom, so the next feature re-opens the same question.
2. **Raise by 0.5% (402 kB)** — 1.7 kB headroom, about one feature increment; the same
   review would return within one or two plans.
3. **Large raise (e.g. 420 kB)** — weakens the guard: a 20 kB accidental regression
   would ship unnoticed.
4. **Drop the total budget and keep only the shell budget** — loses the only guard that
   covers the lazy editor/agent/workflow lanes, which are the ones that have grown.

## Rationale and Evidence

- Measured build (2026-09-15, `frontend/scripts/bundle-budget.mjs`, fresh `vite build`):
  shell 173.7 kB / 180 kB; total 400.3 kB / 400 kB. Lane split: shell ≈173.7,
  editor (CodeMirror) 115.0, agent-core 37.2, workflow ≈45.3, package ≈8.1 kB gzip.
- Ceiling history: plan 099 347.0–350.1 kB / 400; plan 118 381.0 → 392.9 / 400;
  plan 119 baseline 399.0 / 400 → now 400.3 / 400.
- A 1% raise restores a ratio comparable to the one the ceiling was set with (≈1% vs the
  original 15%, deliberately tighter), so it stays a creep guard rather than a freeze.
- No runtime cost, no code path, and no user-visible behaviour change: this is a number
  in a check script plus its documentation mirrors.

## References

- `frontend/scripts/bundle-budget.mjs` — the gate (`SHELL_BUDGET_GZIP_KB`,
  `TOTAL_BUDGET_GZIP_KB`) and its lane classification.
- `docs/development/performance.md:1071` — the documented budget table
  (`Total frontend | <= 400 kB gzip`).
- `tests/performance_budgets.rs:878` (`tauri_react_bundle_budgets_are_documented`) —
  docs-as-code guard that fails if the documented numbers drift from the script.
- `plans/118-Quiet-Instrument-Migration-Component-and-Surface-Adoption.md` — 390.9/392.6/
  392.9 kB totals during the design-system migration.
- `plans/119-Editor-and-Agent-Architecture-Remediation.md:220,223,1093,1196` — 399.0 kB
  baseline, the "headroom nearly gone" note, and the 400.3 kB SC-4 finding.
- Commands: `npm --prefix frontend run build && npm --prefix frontend run check:budget`.

## Reusable guidance

None added: this is a single policy number specific to the frontend budget, not a
reusable planning pattern (`.agents/skills/clay-execution/references/` unchanged).

## Consequences

- Positive: the frontend gate can be green again on a tree whose content is intentional;
  the ceiling still catches large accidental growth.
- Positive: the budget number now has a written rationale and history, so the next
  proposal to move it starts from evidence instead of rediscovery.
- Risk: if the agent lane keeps growing a few kB per plan, the ceiling returns to zero
  headroom quickly. Revisit with a deliberate re-derivation (or real reclaim) rather
  than an incidental bump each time.
- Follow-up: the shell budget stays at 180 kB and is untouched; code-splitting rules
  (lazy agent/editor lanes) remain the first-line tool for new weight.
