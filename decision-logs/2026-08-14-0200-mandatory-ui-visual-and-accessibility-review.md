---
date: 2026-08-14 02:00
status: approved
decision_about: "UI completion evidence"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Require visual and accessibility review for UI work

## Decision

Every Clay plan and `clay-ui` task that changes UI must require post-implementation screenshot review. When computer-use capability is available, it must also inspect the accessibility tree and keyboard interaction before the work is accepted.

## Context

Structural tests and token/component conformance checks cannot prove rendered layout, visual hierarchy, focus visibility, or real assistive-technology behavior. The user requested durable plan- and skill-level enforcement.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: “update @.agents/skills/create-plan/ to make sure to always add UI review task by visual review with screenshot and accessibility check with computer use capability if any UI is changed or modified as part of the plan” and “update @.agents/skills/clay-ui/ skill to make sure that during UI design always the agent takes visual input for review of the implemented UI by taking screenshots”.

## Alternatives Considered

1. **Automated structural/accessibility checks only** — retained as regression coverage, rejected as UI acceptance evidence because they cannot inspect rendered output.
2. **Optional manual UI review** — rejected because it is easily skipped and does not produce evidence.
3. **Mandatory screenshots plus computer-use a11y checks when available** — chosen; it makes gaps and unavailable tooling explicit without blocking non-UI work.

## Rationale and Evidence

Clay's own UI-observability documentation records that structural snapshots do not exercise production GPU rendering and that pixel snapshot coverage is deferred. During this review, live screenshots exposed an oversized completion surface and an accessibility-triggered crash that static source inspection did not establish.

## References

- `docs/development/ui-observability.md` — limits of structural/headless UI coverage.
- `.agents/skills/create-plan/SKILL.md` — required plan workflow.
- `.agents/skills/clay-ui/SKILL.md` — UI implementation workflow.
- `code-reviews/screenshots/2026-08-14-clay-audit/` — live review evidence.

## Consequences

- UI plans add one explicit visual/a11y acceptance task before final documentation work.
- Screenshot paths and limitations become completion evidence.
- Unavailable GUI/computer-use tooling is recorded as an unresolved manual acceptance gap, not silently waived.
