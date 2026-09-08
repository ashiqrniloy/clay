---
name: create-plan
description: Create and maintain numbered plan documents for work that requires documenting executable task lists. Use anytime a user asks an agent to create, write, update, execute, or maintain any plan, including an implementation plan, roadmap, task breakdown, phase plan, project plan, or plan document with objectives, expected outcomes, acceptance criteria, approaches, tests, compromises, and follow-up actions.
---

# Create Plan

Create or update actionable, numbered, documentation-backed implementation plans in `plans/` at the repository root. Executing a plan is routed through `.agents/skills/clay-execution/` (deterministic execution loop, per-task references); this skill defines the plan document itself.

## Plan Creation Workflow

1. Determine the repository root (git root, else cwd); ensure `plans/` exists.
2. Choose the next filename by incrementing the highest existing three-digit prefix, e.g. `001-Setup.md`.
3. Read current docs for relevant libraries/frameworks/SDKs/packages/crates/CLIs/services (project-local docs first, then documentation lookup tools); record exact references in the plan.
4. If `.agents/skills/clay-execution/` exists, use it before writing task approaches (router: plan-creation integration, pattern references) and cite relevant reference files.
5. For phase plans adding/changing an editor mode, language mode, JS package, extension point, or reusable capability, include a dedicated primitive-review task before implementation (inventory existing primitives, document what they achieve, plan only generic reusable new primitives, build package/mode functionality on top).
6. For any plan touching Clay app UI (components, panels, overlays, pop-ups, dropdowns, menus, text inputs, multi-selects, completion pop-ups, theme, typography, tokens, layout), apply the UI gate: every UI task reads `.agents/skills/clay-execution/references/ui.md` plus the catalog references (`references/components.md`, `references/tokens.md`) and lists them under `Approach -> Documentation Reviewed`; plan-level mention alone is insufficient. Substantial new-surface design tasks additionally load the four project-local design skills (`impeccable`, `full-output-enforcement`, `high-end-visual-design`, `design-taste-frontend`). Reuse cataloged primitives/components first; custom components outside the catalog require explicit justification in `Options Considered`. Include one post-implementation visual/accessibility review task before final documentation work (duty details: `clay-execution/references/planning-checklist.md`).
7. Load project-specific plan requirements deterministically: `.agents/skills/create-plan/references/default.md` and `references/<git-root-basename>.md` when present, and apply them before finalizing tasks. For Clay this includes, for plans touching a user-facing configuration surface, the example-config maintenance task plus a separate live launch-test task.
8. Include exactly one final code-wiki task after implementation/verification and project-specific maintenance tasks (when the wiki workflow applies per `.agents/skills/clay-execution/references/docs-as-code.md`); use the template in `references/clay.md`.
9. Write the plan using the structure below.

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

- Make every task independently checkable (`- [ ]` / `- [x]`); acceptance criteria must cover functional, performance, code quality, and security.
- Treat `Approach` as mandatory and evidence-based; include documentation-derived API examples for library/framework/SDK/package/crate/CLI/service usage.
- For new JS packages or mode implementations, add a primitive-first task before package work: read primitive docs/wiki, assess existing Rust-side primitives, identify generic primitive gaps, reject mode-specific Rust logic, include tests/docs so the primitive library becomes easier to reuse.
- List every expected file (mark tentative lists as such, with reason). Write test cases before implementation, derived from acceptance criteria.
- Apply loaded project-specific requirements before finalizing the task list.
- Do not fill `Compromises Made` or `Further Actions` before execution unless known constraints already exist.
