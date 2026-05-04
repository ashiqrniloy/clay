---
name: clay-planning
description: Create and maintain numbered implementation plan documents for work that requires documenting executable task lists. Use whenever an agent is asked to write, update, or execute an implementation plan, roadmap, task breakdown, phase plan, or plan document with objectives, expected outcomes, acceptance criteria, approaches, tests, compromises, and follow-up actions.
---

# Clay Planning

## Overview

Use this skill to create or update rigorous implementation plans in `plans/` at the repository root. Plans must be actionable, numbered in execution order, documentation-backed, and easy to update as tasks are completed.

## Workflow

1. Locate the repository root from the current working directory.
2. Ensure `plans/` exists at the repository root.
3. Inspect existing `plans/` filenames to choose the next three-digit sequence number.
   - Use `001-Topic.md`, `002-Another-Topic.md`, etc.
   - Preserve ordering by incrementing the highest existing prefix.
   - Use a concise PascalCase or Title-Case slug that reflects the plan topic, e.g. `001-Setup.md`, `002-Window-Creation.md`.
4. Before writing task approaches, read current documentation for all relevant libraries, frameworks, SDKs, packages, and Rust crates involved in the planned implementation.
   - Prefer project-local docs first when available.
   - Use documentation lookup tools for external dependencies.
   - Record specific APIs, commands, examples, and references in the plan.
5. Write the plan using the required structure below.
6. When executing an existing plan, update checkbox status as tasks complete. After implementation and passing tests, fill in `Compromises Made` and `Further Actions`.

## Required Plan Structure

Each plan must use this structure:

```markdown
# <Plan Title>

## Objectives
- <Objective 1>
- <Objective 2>

## Expected Outcome
- <What should be true after this plan is executed>
- <Observable behavior, deliverables, or system state>

## Tasks

- [ ] <Task title>
  - Acceptance Criteria:
    - Functional: <task-specific behavior that must work>
    - Performance: <task-specific latency, resource, scale, or non-regression expectation>
    - Code Quality: <task-specific maintainability, architecture, typing, linting, error handling, or idiomatic requirements>
    - Security: <task-specific safety, validation, permissions, secrets, input handling, or dependency requirements>
  - Approach:
    - Documentation Reviewed:
      - <Library/package/crate docs and relevant version, section, or URL/tool reference>
    - Options Considered:
      - <Option A and tradeoff>
      - <Option B and tradeoff>
    - Chosen Approach:
      - <Why this approach fits the current codebase and constraints>
    - API Notes and Examples:
      ```<language>
      <minimal relevant API example or command>
      ```
    - Files to Create/Edit:
      - `<path>`: <planned change>
    - References:
      - <Where the approach was derived or inferred from: docs, code paths, examples, ADRs, issues>
  - Test Cases to Write:
    - <test name or scenario>: <what it validates and which acceptance criteria it covers>

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
```

## Task Writing Rules

- Make every task independently checkable with `- [ ]` or `- [x]`.
- Keep acceptance criteria distinct and specific to the task; do not use generic placeholders.
- Include functional, performance, code quality, and security criteria for every task.
- Treat `Approach` as mandatory and evidence-based, not speculative.
- Include documentation-derived API examples when a task depends on a library, framework, SDK, package, crate, CLI, or external service.
- List every file expected to be created or edited. If the list is uncertain, mark it as tentative and explain why.
- Write test cases before implementation, derived from acceptance criteria.
- Do not fill `Compromises Made` or `Further Actions` before executing tasks unless known constraints already exist; otherwise leave the provided placeholder.

## Execution Updates

When executing a plan:

- Update each task checkbox from `- [ ]` to `- [x]` only after implementation and relevant tests/checks pass.
- Add any discovered deviations to `Compromises Made` after completing tasks.
- Add future improvements to `Further Actions`, including why they were deferred.
- If the implementation requires a changed approach, update the task's `Approach`, `Files to Create/Edit`, and `Test Cases to Write` before continuing.
