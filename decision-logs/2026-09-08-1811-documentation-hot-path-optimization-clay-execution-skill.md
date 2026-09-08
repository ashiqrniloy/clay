---
date: 2026-09-08 18:11
status: approved
decision_about: "Agent documentation hot-path optimization: clay-execution skill, progressive disclosure, historical archive"
proposed_by: "both"
explicitly_approved_by_user: true
---

# Decision: Agent documentation hot-path optimization

## Decision

Restructure Clay's agent-facing documentation into a progressive-disclosure model: merge `.agents/skills/project-wiki/`, `.agents/skills/project-patterns/`, and `.agents/skills/clay-ui/` into one router skill `clay-execution` (triggered by plan-task execution AND plan-less code changes), whose references hold condensed, deduplicated, drift-corrected rules; `create-plan` keeps plan structure and per-task requirement descriptions only (execution loop moves to `clay-execution`); historical phase/review wiki pages move to `docs/wiki/archive/` (pull-only, preserved); the per-task four-design-skill load mandate is replaced by distilled binding rules in `clay-execution/references/ui.md` (the four skills load only for substantial new-surface design tasks); catalog files move to `.agents/skills/clay-execution/references/` with all pinned test paths updated.

Implementation order and full acceptance criteria are recorded in `plans/114-Doc-Hot-Path-Optimization-Clay-Execution-Skill.md`.

## Context

Clay's project knowledge is maintained in a segregated manner across many locations: three skill directories with overlapping triggers, seven restatements of the same UI-skill-stack mandate, an execution loop duplicated between `create-plan/SKILL.md` and the routing duties of `project-wiki`, and a 2.5MB wiki whose `docs/wiki/index.md` mixes 43 historical phase/review pages (~810KB, 39% of module bytes, largest pages documenting the deleted native Masonry client) into the evergreen index. Documentation drift compounds the cost: `clay-ui/SKILL.md` still presents the native Rust GUI (Masonry/winit + Vello/Parley) as current though Plan 097 Phase 12 removed it; catalog pages inventory deleted widgets; `create-plan/references/clay.md` and `project-patterns/references/package-ui-layout.md` carry "migration inventory" lines for code already removed. The user's optimization goal: tighten token usage on the agent hot path, prevent context rot, surface history only when explicitly requested, and never delete history.

## Approval

- Proposed by: both
- Approved by user: Yes
- Approval evidence: "1. Design-skill mandate: go with the recommended option 2. Archive location is fine. 3. Go with the recommended structure 4. Yes. clay-execution should also fire on planless code changes. With these 4 decisions create a plan document in the proposed order of execution for all items" followed by "Complete first task... and update the plan once done."

## Alternatives Considered

1. **Keep per-task four-design-skill load mandate** — Rejected: it is the single biggest hot-path cost; the four skills (`impeccable`, `high-end-visual-design`, `design-taste-frontend`, `full-output-enforcement`) are description-listed every session and fully loaded per UI task, with their advice partly restated seven times across repo skills. Replaced by Clay-adapted binding rules in `ui.md`; the skills still load for substantial new-surface design tasks.
2. **Top-level `archive/` directory** — Rejected: archive inside `docs/wiki/archive/`, near the content it documents, with a single pull-only pointer in the master index.
3. **Keep `components.md`/`tokens.md` at their old `.agents/skills/clay-ui/` paths to avoid touching tests** — Rejected: paths must reflect the single new skill; the pinned-path update (~25 references across `tests/primitives_docs.rs`, `tests/package_ui_conformance.rs`, `tests/documentation_coverage.rs`, `src/shell/theme.rs:2454`, `docs/reference/ui-components.md`) is mechanical and keeps `cargo test` enforcing catalog currency.
4. **`clay-execution` triggers only on plan-task execution** — Rejected: plan-less code changes would lose the wiki/pattern upkeep trigger inherited from `project-wiki`; the router fires on both.
5. **Delete historical content / leave history in the evergreen index** — Rejected: user requires preservation and pull-only exposure; the 43 pages move byte-identical via `git mv`, with evergreen content migrated first for any phase page that is the sole live doc of a surface.
6. **Status quo: keep three skills, tighten in place** — Rejected: does not remove the duplicated triggers, restated mandates, and split routing that constitute the token waste.

## Rationale and Evidence

- Hot-path measurements (bytes): `clay-ui/SKILL.md` 8.7K + `references/` ~569 lines; `project-wiki/SKILL.md` 5.7K; `project-patterns/SKILL.md` 3.1K + 26 references ~750 lines; `create-plan/SKILL.md` 7.9K + `references/clay.md` 28.7K; `docs/wiki/index.md` 156 lines with 37 phase-named entries; `AGENTS.md` 5.6K. Historical module pages: 43 files, 810KB total, e.g. `masonry-shell.md` 74KB and `masonry-editor.md` 42KB documenting removed code.
- Duplication evidence: the "load clay-ui + 4 design skills per UI task" mandate appears in `create-plan/SKILL.md` step 7, `create-plan/references/clay.md` (two task types), `project-patterns/references/ui-skill-stack.md`, `ui-modernization.md`, `package-ui-layout.md`, `planning-checklist.md`, and `clay-ui/SKILL.md` Step 0 — seven restatements. The plan-duties duplication: primitive-review template in SKILL step 6 and `clay.md`; wiki-task template in SKILL step 9, `clay.md`, and `references/wiki-task.md`.
- Graft (`graft/`, deterministic, auto-derived, queryable) already routes code-navigation through `AGENTS.md`; the restructure sharpens the boundary — graft answers "where/how the code works now", the wiki answers evergreen "why/invariants/flows" — so new wiki duty shrinks to cross-cutting flows and token savings compound over time.
- Catalog path pins are a real migration constraint (tests enforce catalog-vs-code currency); updating them is part of the approved scope, not a risk.

## References

- `plans/114-Doc-Hot-Path-Optimization-Clay-Execution-Skill.md` — approved implementation order with acceptance criteria per task.
- `docs/wiki/modules/tauri-react-cutover.md` — native client deletion record (Plan 097 Phase 12) that the corrected docs align with.
- `tests/primitives_docs.rs`, `tests/package_ui_conformance.rs`, `tests/documentation_coverage.rs`, `src/shell/theme.rs:2454`, `docs/reference/ui-components.md` — pinned catalog paths updated by the plan.
- `graft/` — auto-derived code map; AGENTS.md routing entry. `AGENTS.md` verified lean and current; no edit planned.
- Prior decision log `2026-05-08-1419-markdown-authoritative-documentation-registry.md` — documentation-as-code foundations that this decision extends, not supersedes.

## Consequences

- Positive outcomes expected: an executing agent reads 2–3 reference files instead of 6+; always-listened skill descriptions drop from 3 entries to 1; wiki index shrinks ~35% with zero historical noise; document text (all three merged skills + `create-plan` + catalogs) roughly halves while every durable rule survives; drift about the deleted native client is eliminated from every surviving document.
- Risks or follow-up work: any missed pinned path breaks `cargo test` (mitigated by repo-wide sweeps in the plan); historical pages must be classified before moving (7 candidate pages flagged as possible sole live docs of surfaces); `create-decision-log` step 6 and `docs/reference/ui-components.md` links retarget; `graft build` refresh required after moves.
- Pattern extraction: the durable guidance from this decision (historical records live pull-only under `docs/wiki/archive/`; hot-path docs are progressive-disclosure with single-source mandates; docs must be corrected at removal time, not at audit time) is folded into `clay-execution/references/docs-as-code.md` by plan Task 2, which replaces `project-patterns/` before any per-skill pattern file would be read — the create-decision-log step 6 update is therefore scheduled inside the plan (Task 2) rather than executed against the outgoing directory structure.
- Conditions that would cause revisiting: if the distilled `ui.md` rules prove insufficient for UI quality in practice (agents regress without the four-skill context), the load mandate returns; if token savings fail to materialize, reconsider merge granularity.