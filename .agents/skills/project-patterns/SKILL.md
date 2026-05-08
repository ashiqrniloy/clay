---
name: project-patterns
description: Maintain and apply project-level pattern memory for repositories that also use create-plan and create-decision-log. Use this skill only at project level, never as a system-level skill. Use whenever creating, reviewing, or updating a plan document so the plan aligns with project patterns in references/. Use after an approved decision log is written so stable reusable guidance from the decision is added to new pattern files or folded into existing pattern files under references/.
---

# Project Patterns

Use this skill as a project-local pattern router. Keep `SKILL.md` generic; store all project-specific guidance as concise files in `references/`.

## Convention

- This skill assumes the project also has `create-plan` and `create-decision-log` skills.
- Use this skill only from a project repository. Do not install or invoke it as a system-level skill.
- Treat `references/` as appendable project memory. A new project may start with an empty `references/` folder.
- Keep each pattern file short, stable, and reusable. Do not copy full plan or decision-log text.

## When Creating or Updating a Plan

1. Inspect `references/` and select only pattern files relevant to the plan.
2. Read those files before writing task approaches, acceptance criteria, phase boundaries, or follow-up actions.
3. Align the plan with the selected patterns, or explicitly call out any approved exception with a decision-log reference.
4. In the plan's task references or approach notes, cite the pattern files used.

## When Logging an Approved Decision

1. First complete the `create-decision-log` workflow and write the approved decision log.
2. Read the new decision log and inspect `references/` for related pattern files.
3. Extract only durable planning guidance: ownership rules, architectural constraints, naming conventions, workflows, quality bars, testing expectations, or policy defaults.
4. Add a concise new pattern file or update the smallest relevant existing file.
5. Prefer one pattern per file. Use clear kebab-case filenames, e.g. `api-versioning.md`.
6. If the decision is too narrow or not reusable, do not add a pattern; mention that no pattern update was needed.

## Pattern File Style

Each file in `references/` should be concise and scannable:

```markdown
# <Pattern Name>

- <Stable rule or default>
- <When it applies>
- <Required plan/implementation implication>
- <Decision log source, if known: decision-logs/...>
```

Avoid project-specific instructions in this `SKILL.md`; put them in `references/` instead.
