---
date: 2026-08-17 19:31
status: approved
decision_about: "Plan 090 visual review scope"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Waive visual review for Plan 090

## Decision

Treat Plan 090's visual screenshot review as unnecessary and complete that task as not applicable. Plan 090 is a responsibility-preserving refactor: it moves existing UI/shell/editor code into private modules without changing user-facing UI elements, layout, styling, labels, or interaction contracts.

## Context

The plan's UI-adjacent work extracted `masonry_shell`, editor surface, and app-driver responsibilities. The implementation preserved existing behavior and passed the full Linux test/performance/accessibility gates. A live screenshot attempt was also blocked by this host's compositor placing Clay off-screen, but the user confirmed that visual review should be skipped for this plan rather than extended.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: “Let's skip the visual review then and mark it as unnecessary for this plan and move on.”

## Alternatives Considered

1. **Continue repairing the host screenshot/window setup** — rejected for this plan; it adds environment work without a changed visual surface to inspect.
2. **Leave visual acceptance unresolved** — rejected after explicit user scope decision.
3. **Mark visual review not applicable for this pure refactor** — chosen; structural/accessibility evidence remains historical validation, not visual acceptance evidence.

## Rationale and Evidence

- Plan 090 explicitly requires preserving UI behavior and excludes visual redesign or dependency migration: `plans/090-Audit-Remediation-Responsibility-Preserving-Refactor.md`.
- The shell extraction retained `ClayShellWidget` as state owner and deferred overlay-state consolidation rather than changing presentation behavior.
- The editor extraction retained `EditorSurface` as the state owner and moved only cohesive helpers/state vocabulary.
- The completed refactor gates reported 2,289 tests passing, no clippy warnings, clean formatting, no benchmark regression, and no new public API surface.
- The prior visual-review artifact records the attempted live AT-SPI checks and the invalid off-screen screenshot capture: `code-reviews/screenshots/2026-08-14-plan090-refactor-parity/REVIEW.md`.

## References

- `plans/090-Audit-Remediation-Responsibility-Preserving-Refactor.md`
- `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md` — general rule; this is an explicit Plan 090 exception.
- `docs/development/architecture-ownership.md`
- `code-reviews/screenshots/2026-08-14-plan090-refactor-parity/REVIEW.md`
- Prior full Linux test/performance/security gate recorded in the Plan 090 task 9 evidence.

## Consequences

- Plan 090 can proceed to Clay JS API/configuration/manual-plan/wiki verification without another screenshot run.
- The existing UI visual-review requirement remains unchanged for later plans that add or alter user-facing UI; this exception does not weaken that default.
- No project-pattern update is needed because this is a one-plan scope exception, not a reusable UI policy.
