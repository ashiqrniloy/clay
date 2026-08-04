---
date: 2026-08-04 16:45
status: approved
decision_about: "Manual test plan folder (test-plan/) with per-plan manual verification duty"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: `test-plan/` manual verification guide maintained by every behavior-changing plan

## Decision

Clay keeps a module-segregated manual test plan under `test-plan/` (index plus per-module/workflow files). Every plan document that changes user-visible behavior must include a dedicated task that executes the affected manual steps and maintains the `test-plan/` files and index.

## Context

Automated suites gate every change, but rendering, blink rhythm, ligature glyphs, IME feel, native dialogs, and latency feel are only verifiable by a human. Manual procedures existed scattered across `docs/development/*.md` with no single guide and no process guarantee that new features get manual coverage. The user requested a dedicated folder with a guiding index and a per-plan enforcement task in the create-plan skill.

## Approval

- Proposed by: user
- Approved by user: Yes — direct instruction: "create a test-plan folder and create documentation on how to do manual test of the whole application. This should be also a task in each plan using @.agents/skills/create-plan/ so that it is enforced as a task that a manual check is always done… at each plan there should be task to maintain this."

## Alternatives Considered

1. **Single monolithic test-plan.md** — rejected: 12 distinct workflows in one file makes per-change maintenance and targeted runs impractical.
2. **Rewrite existing docs/development/ content into test-plan/** — rejected: duplication drifts; module files reference the deep docs (`launch-and-gui-smoke.md`, `file-open-save-reload-workflow.md`, `manual-editor-capabilities-test-plan.md`, `performance.md`, `windows.md`) and carry checklists instead.
3. **Fold the duty into the existing verification task of each plan ad hoc** — rejected: the user explicitly asked for a dedicated enforced task so manual coverage cannot be silently skipped.

## Rationale and Evidence

- `test-plan/index.md` provides how-to-use, prerequisites, a 12-module map with deep-reference links, and a change→minimum-modules coverage matrix.
- Module files 01–12 cover launch/connection, configuration, files/workspace, core editing, movement/selection, multi-cursor, caret/typography, syntax/textobjects, packages/modes, keybindings/commands, performance, Windows; each has setup, numbered steps with expected results, negative checks, and known ceilings (matching plan 071 documented ceilings).
- Enforcement lives in `.agents/skills/create-plan/references/clay.md` ("Manual Test Plan Task" section) — the file SKILL.md already routes Clay plans through. The task permits an explicit recorded N/A for automated-only changes and forbids weakening steps to pass.
- Discoverability: `docs/index.md` Developer Guides section links the plan (outside the registry section, so the doc-registry generator is unaffected).

## References

- `test-plan/index.md` + `test-plan/01-…` through `test-plan/12-…` module files.
- `.agents/skills/create-plan/references/clay.md` — "Manual Test Plan Task".
- `docs/development/` deep-reference docs.

## Consequences

- Positive: one guided entry point for human verification; coverage is plan-gated; step IDs (e.g. X3, K12) give defect reports precise anchors.
- Cost: module files must be maintained per plan (the task exists exactly for this).
- Revisit if module count grows past ~15 or a workflow needs sub-modules.
