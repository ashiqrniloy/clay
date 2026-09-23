---
name: clay-execution
description: Execute tasks from plan documents and any plan-less code change in this repository. Router to project references: UI catalog and rules, Clay JS API naming/boundary/schema, package trust domains and authority boundaries, protocol/performance/behavior manifests, configuration and design-system constraints, documentation-as-code and wiki workflow, and the Clay planning checklist. Load only the reference files your task type needs.
---

# Clay Execution

Project-local execution router. Use whenever executing a task from a plan document **or** making/reviewing any code change in this repository without a plan (wiki upkeep, pattern memory, catalog currency).

Store no project content here; all rules live in `references/`.

## Plan Creation and Decision-Log Integration

- Creating/updating a plan: inspect `references/` and select only files relevant to the plan. Read them before writing task approaches, acceptance criteria, phase boundaries, or follow-up actions. Align the plan with the patterns, or explicitly call out any approved exception with a decision-log reference. Cite the pattern files in the task's references/approach notes.
- After an approved decision log: extract only durable planning guidance (ownership rules, architectural constraints, naming conventions, workflows, quality bars, testing expectations, policy defaults). Add a concise new reference file or update the smallest relevant existing one; one pattern per file, kebab-case names. If the decision changes required plan structure or recurring plan tasks, update `.agents/skills/create-plan/references/<git-root-basename>.md` (for Clay: `clay.md`). If the decision is too narrow to be reusable, add nothing and say so.
- Never copy full plan or decision-log text into references.

## Deterministic Execution Loop

1. Read the full plan.
2. Select the first unchecked task unless the user names a specific task.
3. If the task touches UI, theme, typography, tokens, components, layout, SDUI, or accessibility, read `references/ui.md` plus the component/token catalogs **before reviewing implementation or editing files**. Do not treat evidence recorded for another task as sufficient.
4. Re-read the routing-table references for this task, loaded project-specific plan requirements (`create-plan/references/<git-root-basename>.md`), relevant decision logs, and existing wiki pages for files/modules being changed.
5. Implement only the selected task unless dependencies require a small, explicitly noted prerequisite.
6. Run the task's listed tests/checks and any directly relevant validation.
7. Update the task checkbox to `- [x]` only after implementation and checks pass.
8. If the approach, files, or tests changed, update that task before continuing.
9. Repeat from step 2 until implementation and verification tasks are complete.
10. Execute any project-specific API/documentation maintenance task (e.g. Clay JS API verification) when present.
11. Execute the final code-wiki task when present (see `references/docs-as-code.md`).
12. Run final verification for the plan.
13. Fill `Compromises Made` and `Further Actions` with actual deviations, deferred work, rationale, and priority.

## Reading Model

Select by task type; read only what applies:

| Task touches | Read first |
|---|---|
| UI, theme, typography, tokens, layout, SDUI, accessibility | `references/ui.md`, `references/components.md`, `references/tokens.md` |
| Clay JS APIs, commands, key bindings, exports | `references/js-api.md` |
| Packages, modes, trust domains, authority, agent host, extension points | `references/packages.md` |
| Protocol, IPC, performance, behavior manifests, hot path | `references/protocol-perf.md` |
| Configuration, init.js, design-system packages, typography ownership | `references/config.md` |
| Docs, wiki pages, generated registries, examples/init.js | `references/docs-as-code.md` |
| Any plan creation/update | `references/planning-checklist.md` |

## Code Navigation: Graft First

- `graft` answers "where/how the code works now": auto-derived from source, always current, cheap queries (`graft ask`, `graft skeleton`, `graft callers`, `graft grep`). Query it before grepping or opening files.
- The wiki (`docs/wiki/`, per `references/docs-as-code.md`) answers evergreen "why/invariants/flows". Do not duplicate graft's code-map data in wiki pages.

## History Is Pull-Only

`plans/`, `decision-logs/`, `code-reviews/`, and `docs/wiki/archive/` hold historical records. Read them only when a task cites them or you explicitly need history. Never scan them proactively; nothing historical is linked from hot-path indexes. Keep them immutable; superseding decisions get new entries that reference earlier ones.