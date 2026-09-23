---
date: 2026-08-30 21:54
status: approved
decision_about: "Initial st workflow types and the loop/skill execution model"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Initial `st` workflow types are `build` + `fix`; host loop auto-prompts skills

## Decision

`st` ships two initial workflow types: `build` (goal → phased roadmap →
per-phase plans → tasks) and `fix` (one or more issues → one plan doc with
tasks mapped to issues → per-issue loop). Both ride the same per-task
implementation → test child → validation child → commit cycle. The host
orchestrator **auto-prompts** every required step — skills are bodies, not
hope: an agent that never loads a skill still hits the step because the
loop injects it. Project patterns are not a skill; they live as
`execute-plan/references/` grouped by unit. `create-decision-log` is
embedded in create-plan and execute-plan to surface critical decisions
for human review. Compromises/further-actions are always surfaced and can
be enqueued onto the workflow roadmap for a later plan.

## Context

The roadmap needed the workflow-type set beyond `build` and the execution
model that guarantees skill-loaded behavior. Discussion settled the fix
loop shape (issues in, plan, one-by-one solve/test/validate, done when
all issues resolved), the auto-prompt principle, the project-patterns
placement, decision-log embedding, and the compromise/further-action
surfacing rules.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: user specified the fix workflow steps, the loop and
  loop-done conditions, separate test/validation sub-agents, the
  skill-system additions (documented/test/validate child flows, typed
  feedback tasks appended to the main plan, full-suite reruns), project
  patterns as execution references with no generic base, decision-log
  embedding, and compromises/further-actions surfacing (2026-08-29/30).
  Approved for logging: "Go ahead and log the decisions as decided
  already" (2026-08-30).

## Alternatives Considered

1. **Single generic workflow type** — rejected; build and fix have
   different entry contracts (goal vs issues) though the same inner loop.
2. **Skills as passive markdown** — rejected; the loop injects required
   steps so behavior does not depend on the agent choosing to load a
   skill.
3. **Project patterns as a skill with generic base** — rejected; always
   project-built, grouped by unit inside execute references.
4. **Test/validation by the implementing agent** — rejected; separate
   child agents keep implementation clean and provide independent
   verification; failures append typed tasks (bug/enhancement/validation)
   to the main plan, which the main agent fixes, then both children
   re-run the full frozen suites.

## Rationale and Evidence

- Per-task test + validation children match the Phase 6 exit-gate
  design (children authored from implementation plan only, store
  inspection proving isolation).
- ponytail-review is always part of validation (user decision).
- Skill shape (generic SKILL.md + project `references/`, load
  `references/default.md` then `references/<git-root-basename>.md`) is
  the proven pi/clay progressive-disclosure model.

## References

- `roadmap.md` — Phase 5 (skill shape, loop model), Phase 6 (sub-agent
  validation loop), "Resolved this iteration" items 2.
- pi skills documentation (progressive disclosure, load order).

## Consequences

- Positive: deterministic step coverage regardless of agent behavior;
  typed feedback loop between children and main agent; decisions and
  compromises never silently dropped.
- Risks: auto-prompting adds per-step overhead; acceptable for autonomous
  runs.
- Revisit if: more workflow types are added (post-roadmap by design —
  each is a registered loop implementation).