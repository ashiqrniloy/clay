---
name: create-plan
description: Create and maintain numbered plan documents for work that requires documenting executable task lists. Use anytime a user asks an agent to create, write, update, execute, or maintain any plan, including an implementation plan, roadmap, task breakdown, phase plan, project plan, or plan document with objectives, expected outcomes, acceptance criteria, approaches, tests, compromises, and follow-up actions.
---

# Create Plan

Create or update actionable, numbered, documentation-backed implementation plans in `plans/` at the repository root.

## Plan Creation Workflow

1. Determine the repository root: Git root if available, otherwise current working directory.
2. Ensure `plans/` exists.
3. Choose the next filename by incrementing the highest existing three-digit prefix, e.g. `001-Setup.md`, `002-Window-Creation.md`.
4. Read current docs for relevant libraries, frameworks, SDKs, packages, crates, CLIs, or services. Prefer project-local docs, then documentation lookup tools. Record exact docs/API references in the plan.
5. If `.agents/skills/project-patterns/` exists, use it before writing task approaches and cite relevant pattern files.
6. Load project-specific plan requirements deterministically:
   - Read `.agents/skills/create-plan/references/default.md` if it exists.
   - Read `.agents/skills/create-plan/references/<git-root-basename>.md` if it exists.
   - Apply all loaded requirements before finalizing tasks.
7. If `.agents/skills/project-wiki/` exists, include exactly one final code-wiki task after implementation/verification and project-specific maintenance tasks. Use `.agents/skills/create-plan/references/wiki-task.md` when present.
8. Write the plan using the structure below.

## Required Plan Structure

```markdown
# <Plan Title>

## Objectives
- <Objective 1>
- <Objective 2>

## Expected Outcome
- <Observable deliverable or behavior>
- <System state after completion>

## Tasks

- [ ] <Task title>
  - Acceptance Criteria:
    - Functional: <task-specific behavior>
    - Performance: <latency/resource/scale/non-regression expectation>
    - Code Quality: <maintainability/architecture/typing/linting/error-handling expectation>
    - Security: <safety/validation/permissions/secrets/dependency expectation>
  - Approach:
    - Documentation Reviewed:
      - <docs, versions, sections, URLs, or tool references>
    - Options Considered:
      - <Option and tradeoff>
      - <Option and tradeoff>
    - Chosen Approach:
      - <why this approach fits>
    - API Notes and Examples:
      ```<language>
      <minimal relevant API example or command>
      ```
    - Files to Create/Edit:
      - `<path>`: <planned change>
    - References:
      - <docs, code paths, examples, decisions, issues, or patterns>
  - Test Cases to Write:
    - <test/scenario>: <what it validates>

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
```

## Task Writing Rules

- Make every task independently checkable with `- [ ]` or `- [x]`.
- Keep acceptance criteria specific; include functional, performance, code quality, and security criteria for every task.
- Treat `Approach` as mandatory and evidence-based.
- Include documentation-derived API examples for library/framework/SDK/package/crate/CLI/service usage.
- List every expected file. If uncertain, mark the list tentative and explain why.
- Write test cases before implementation, derived from acceptance criteria.
- Apply loaded project-specific requirements before finalizing the task list.
- Do not fill `Compromises Made` or `Further Actions` before execution unless known constraints already exist.

## Deterministic Execution Loop

When executing a plan:

1. Read the full plan.
2. Select the first unchecked task unless the user names a specific task.
3. Re-read relevant project patterns, loaded project-specific plan requirements, decision logs, and existing wiki pages for files/modules being changed.
4. Implement only the selected task unless dependencies require a small, explicitly noted prerequisite.
5. Run the task's listed tests/checks and any directly relevant validation.
6. Update the task checkbox to `- [x]` only after implementation and checks pass.
7. If the approach, files, or tests changed, update that task before continuing.
8. Repeat from step 2 until implementation and verification tasks are complete.
9. Execute any project-specific API/documentation maintenance task, such as Clay JS API verification, when present.
10. Execute the final code-wiki task when present.
11. Run final verification for the plan.
12. Fill `Compromises Made` and `Further Actions` with actual deviations, deferred work, rationale, and priority.
