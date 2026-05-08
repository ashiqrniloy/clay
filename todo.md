# Temporary Todo

## Workflow Determinism and Skill Consistency Review

- [ ] 1. Make `create-plan` generic again by removing hardcoded Clay wording
  - Current issue:
    - `.agents/skills/create-plan/SKILL.md` says: `For this Clay repository, read references/clay.md`.
    - This makes the generic skill project-specific.
  - Proposed change:
    - Replace with deterministic project reference lookup, e.g. read `.agents/skills/create-plan/references/<git-root-basename>.md` when it exists.
    - Optionally support `.agents/skills/create-plan/references/default.md` for non-project-specific defaults.
  - Expected result:
    - The skill remains reusable across projects.
    - In this repository, the git root basename resolves to `clay`, so `references/clay.md` is still loaded deterministically.

- [ ] 2. Make the final code-wiki task conditional instead of part of the always-required generic plan template
  - Current issue:
    - `create-plan` says every plan must use the required structure, and that structure includes `Update the code wiki after implementation`.
    - This conflicts with the earlier condition that the task is only required when `.agents/skills/project-wiki/` exists.
  - Proposed change:
    - Move the full wiki task template out of `create-plan/SKILL.md` into a reference file such as `.agents/skills/create-plan/references/wiki-task.md`.
    - Keep only a short rule in `SKILL.md`: if `project-wiki` exists, append exactly one final wiki task from the reference template.
  - Expected result:
    - Non-wiki projects do not get inappropriate wiki tasks.
    - Wiki-enabled projects still get the final wiki update/verification task deterministically.

- [ ] 3. Add a strict deterministic execution loop to `create-plan`
  - Current issue:
    - `create-plan` says to update checkboxes as tasks complete, but does not define a strict task-by-task execution algorithm.
    - Agents may batch tasks, skip final documentation/wiki tasks, or update plan state inconsistently.
  - Proposed change:
    - Add an execution loop such as:
      1. Read the full plan.
      2. Select the first unchecked task unless the user specifies another.
      3. Re-read relevant project patterns, project-specific plan requirements, and wiki pages.
      4. Implement only that task.
      5. Run the task's listed tests/checks.
      6. Update the checkbox only after checks pass.
      7. Repeat until implementation/verification tasks are complete.
      8. Execute the Clay JS API task if present.
      9. Execute the final wiki task if present.
      10. Run final verification.
      11. Fill `Compromises Made` and `Further Actions`.
  - Expected result:
    - Plan execution becomes more predictable for AI agents.
    - Final maintenance tasks are less likely to be skipped.

- [ ] 4. Update `project-patterns/references/planning-checklist.md` to match the Clay JS API decision
  - Current issue:
    - The checklist still says any new public protocol/API/command/manifest/permission/tool surface must have a registry/documentation path.
    - Newer architecture says documentation-as-code registry coverage applies to Clay JS APIs, while internal implementation details belong in the code wiki.
  - Proposed change:
    - Replace broad public-surface registry language with Clay JS API-specific language.
    - Clarify that server-side Rust public functions must have Clay JS APIs, and internal implementation details go to the project wiki.
  - Expected result:
    - Agents do not document raw Rust/protocol internals directly as public registry docs when the correct public surface is the Clay JS API facade.

- [ ] 5. Clarify boundary between `project-wiki` and documentation-as-code registry docs
  - Current issue:
    - `project-wiki` says it documents public APIs, protocols, commands, etc.
    - `documentation-as-code.md` says public programmatic docs should be Clay JS API registry docs.
    - This can cause duplication or drift between `docs/wiki/` and `docs/reference/`.
  - Proposed change:
    - Update `project-wiki/SKILL.md` to say:
      - For public programmatic APIs, link to the authoritative Clay JS API registry docs instead of duplicating them.
      - The wiki should explain implementation internals behind those APIs.
  - Expected result:
    - `docs/reference/` remains authoritative for Clay JS API public docs.
    - `docs/wiki/` remains authoritative for educational implementation knowledge.

- [ ] 6. Add a decision-log feedback rule for plan-structure requirements
  - Current issue:
    - `create-decision-log` already says approved decisions should update `project-patterns` when reusable patterns change.
    - But if a decision changes required plan structure, there is no explicit instruction to update `.agents/skills/create-plan/references/<project>.md`.
  - Proposed change:
    - Update `create-decision-log/SKILL.md` and/or `project-patterns/SKILL.md` to say:
      - If an approved decision changes required plan structure or recurring plan tasks, also update `.agents/skills/create-plan/references/<project>.md` when present.
  - Expected result:
    - Project pattern memory and project-specific plan requirements stay synchronized.

- [ ] 7. Decide whether to keep or split `documentation-as-code.md`
  - Current issue:
    - `.agents/skills/project-patterns/references/documentation-as-code.md` is still readable, but it mixes several concerns:
      - Clay JS API boundary
      - required documentation fields
      - registry generation/tests
      - planning guidance
      - anti-patterns
  - Proposed options:
    - Keep as one file for simplicity.
    - Split into smaller files, for example:
      - `documentation-as-code.md`
      - `clay-js-api-boundary.md`
      - `doc-registry-tests.md`
  - Expected result:
    - Pattern files remain concise and easier for agents to load selectively.

- [ ] 8. Reduce `project-wiki/SKILL.md` size by moving the page template to a reference file
  - Current issue:
    - The skill is valid but includes a large inline page template.
  - Proposed change:
    - Move the template to `.agents/skills/project-wiki/references/page-template.md`.
    - Keep only workflow, scope, and quality bar in `SKILL.md`.
  - Expected result:
    - The skill becomes more concise without losing detail.
    - Agents can load the template only when writing wiki pages.

- [ ] 9. Reduce `create-plan/SKILL.md` size by moving detailed optional task templates to references
  - Current issue:
    - `create-plan/SKILL.md` includes a full final wiki task template inline.
    - Future project-specific recurring tasks could make the generic skill even larger.
  - Proposed change:
    - Move detailed recurring task templates into reference files.
    - Keep `SKILL.md` as a concise workflow/router.
  - Expected result:
    - `create-plan` stays generic and concise.
    - Project-specific and optional details remain available through references.

- [ ] 10. Consider future automated validation for wiki and docs maintenance
  - Current issue:
    - Some maintenance expectations are currently manual, especially wiki index completeness.
  - Proposed future checks:
    - Wiki index links every wiki page.
    - Clay JS API docs appear in `docs/index.md`.
    - Generated documentation registry is current.
    - Server-side Rust public functions have Clay JS APIs.
    - Clay JS APIs have Markdown docs and generated registry entries.
  - Expected result:
    - The intended AI-maintained workflow becomes verifiable through tests instead of relying only on instruction-following.
