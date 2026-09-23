---
date: 2026-08-23 01:15
status: approved
decision_about: "Mandatory project-local UI skill stack"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Replace external UI skill routing with a fixed project-local stack

## Decision

Remove the external UI skill-selection command from Clay's repository workflow. Every UI planning, implementation, and review task must load `clay-ui` and all four project-local skills: `impeccable`, `full-output-enforcement`, `high-end-visual-design`, and `design-taste-frontend`. Clay's layout and spatial-engineering directives are part of `clay-ui` itself.

Every UI task in a plan must name these skills under `Approach -> Documentation Reviewed`; a plan-level mention or prior task's evidence is insufficient.

## Context

Clay previously required agents to invoke an external skill router, inspect categories, and select one to three skills per UI task. The user reported that this selection workflow was not working well and requested a deterministic project-local stack instead.

The required skills contain overlapping and sometimes page-specific aesthetic rules. Clay therefore uses all of them as quality lenses while preserving the user brief, existing product identity, accessibility, security, authority boundaries, component compatibility, and typed theme/token ownership as hard constraints.

## Approval

- Proposed by: User
- Approved by user: Yes
- Approval evidence: The user explicitly instructed removal of the existing command everywhere and required the named skill stack for each UI task, `create-plan`, `clay-ui`, Plan 097, and the overall repository.

## Alternatives Considered

1. **Keep external category-based selection** — rejected because the user found its results unreliable and it made task execution dependent on external routing.
2. **Select a smaller project-local subset per task** — rejected because the user explicitly requires the full fixed stack for every UI task.
3. **Require the fixed project-local stack and reconcile conflicts through Clay constraints** — selected because it is deterministic, repository-owned, and keeps specialist review lenses available without surrendering product authority.

## Rationale and Evidence

- `.agents/skills/create-plan/SKILL.md` previously imposed the external routing command both during planning and execution.
- `.agents/skills/clay-ui/SKILL.md` repeated the same per-task gate.
- Project patterns and numerous plans inherited that command, creating duplicated and inconsistent routing evidence.
- All required skills exist under `.agents/skills/`, so no external lookup or network-dependent selection is necessary.
- `high-end-visual-design` and `design-taste-frontend` contain potentially conflicting aesthetic defaults. Treating them as complementary review lenses, subordinate to Clay's brief and platform constraints, avoids blindly imposing marketing-page patterns on an operating desktop UI.

## References

- `.agents/skills/create-plan/SKILL.md`
- `.agents/skills/create-plan/references/clay.md`
- `.agents/skills/clay-ui/SKILL.md`
- `.agents/skills/impeccable/SKILL.md`
- `.agents/skills/full-output-enforcement/SKILL.md`
- `.agents/skills/high-end-visual-design/SKILL.md`
- `.agents/skills/design-taste-frontend/SKILL.md`
- `plans/097-Tauri-React-Architecture-Migration.md`

## Consequences

- UI work no longer invokes or depends on the external router.
- Every UI task carries a larger fixed context cost in exchange for deterministic quality coverage.
- Plans become more explicit because each UI task lists the complete skill stack.
- Agents must reconcile contradictory stylistic rules instead of applying every aesthetic literally.
- Revisit only if the fixed stack causes measurable context or instruction-conflict failures; any replacement remains project-local and requires a superseding decision.
