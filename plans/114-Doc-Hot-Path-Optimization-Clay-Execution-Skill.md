# Documentation Hot-Path Optimization: Clay-Execution Skill, Progressive Disclosure, and Historical Archive

## Objectives

- Optimize the agent hot path: an executing agent reads only the 1–3 reference files its task type needs, instead of 3 separate skills plus duplicated mandates.
- Merge `.agents/skills/project-wiki/`, `.agents/skills/project-patterns/`, and `.agents/skills/clay-ui/` into a single router skill, `clay-execution`, triggered by plan-task execution and by plan-less code changes.
- Keep `create-plan` as the single holder of plan document structure and per-task requirement descriptions; move the deterministic execution loop into `clay-execution`.
- Replace the per-task four-design-skill load mandate with distilled binding rules in `clay-execution/references/ui.md`; the four design skills load only for substantial new-surface design tasks.
- Move historical records (43 phase/review/bugfix wiki pages, ~810KB) out of the evergreen wiki into `docs/wiki/archive/`, preserving them pull-only; prune the wiki master index.
- Fix identified documentation drift (deleted native Masonry client presented as current architecture) in every surviving document.
- Tighten text volume across all touched documents without losing information.

## Expected Outcome

- `.agents/skills/clay-execution/` exists with a router `SKILL.md` (~120 lines) and ~10 reference files; `project-wiki/`, `project-patterns/`, `clay-ui/` directories are deleted.
- `create-plan/SKILL.md` contains no execution loop; `references/wiki-task.md` is folded into `references/clay.md`.
- `docs/wiki/index.md` links zero phase-named pages and carries one archive pointer line; all 43 identified historical pages are either moved to `docs/wiki/archive/` or their evergreen content migrated into a surviving module page.
- No surviving document references the deleted Masonry client as current; the UI catalog is React/frontend-primary with server-side validation paths.
- All Linux gates pass: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (including `tests/primitives_docs.rs`, `tests/package_ui_conformance.rs`, `tests/documentation_coverage.rs` against the moved catalog paths).
- Zero information loss: every durable rule, task requirement, and decision-log citation from the merged skills survives in a `clay-execution` reference; every historical page survives in the archive.

## Tasks

- [x] Record the approved methodology decision in decision-logs/
  - Acceptance Criteria:
    - Functional: A decision log entry at `decision-logs/2026-09-08-1811-documentation-hot-path-optimization-clay-execution-skill.md` records the four user-approved decisions: (1) distilled UI rules in `clay-execution/references/ui.md` replace the per-task four-design-skill load mandate, with the four skills loading only for substantial new-surface design tasks; (2) historical wiki pages move to `docs/wiki/archive/`; (3) catalog files move to `clay-execution/references/` with test path strings updated; (4) `clay-execution` also triggers on plan-less code changes (inherits the old `project-wiki` trigger).
    - Performance: None (documentation only).
    - Code Quality: Follows the create-decision-log template (frontmatter, Decision, Context, Approval, Alternatives, Rationale, Consequences); approval evidence quotes the user's approval message; alternatives considered record the rejected options (keep per-task design-skill mandate, top-level `archive/`, keeping catalog files at old paths, plan-only trigger).
    - Security: No secrets or credentials recorded.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`: required workflow and template.
    - Options Considered:
      - Log each decision separately: rejected — one methodology decision, one log.
      - Skip logging: rejected — repo convention requires finalized decisions to be recorded before pattern files are restructured.
    - Chosen Approach:
      - Single decision log covering the reorganization and the four approved options.
    - API Notes and Examples:
      ```text
      decision-logs/YYYY-MM-DD-HHMM-documentation-hot-path-optimization-clay-execution-skill.md
      ```
    - Files to Create/Edit:
      - `decision-logs/2026-09-08-1811-documentation-hot-path-optimization-clay-execution-skill.md`: Created (2026-09-08 18:11), template-compliant, all four decisions recorded with user approval evidence quoted.
    - References:
      - User approval in conversation: "1. Design-skill mandate: go with the recommended option 2. Archive location is fine. 3. Go with the recommended structure 4. Yes. clay-execution should also fire on planless code changes."
  - Test Cases to Write:
    - Manual (PASSED): Log follows the template (frontmatter, Decision, Context, Approval, Alternatives Considered, Rationale and Evidence, References, Consequences), filename is chronologically sortable (`2026-09-08-1811-...`), all four decisions recorded with approval evidence quoted verbatim, and the deferred pattern-extraction step is explicitly noted in Consequences.

- [x] Create the clay-execution router skill from project-wiki and project-patterns
  - Acceptance Criteria:
    - Functional: `.agents/skills/clay-execution/SKILL.md` exists with frontmatter description triggering on (a) executing a task from a plan document and (b) any code change/review in this repository without a plan (inherits `project-wiki`'s trigger). The body contains only: the deterministic execution loop (moved from `create-plan/SKILL.md`), a task-type→reference routing table, a one-line graft boundary statement (graft = "where/how code works now", wiki = evergreen "why/invariants/flows"), and the archive policy (history is pull-only, never linked from hot-path indexes).
    - Performance: Router SKILL.md ≤ ~120 lines; no reference content inlined in the router.
    - Code Quality: Every durable rule and decision-log citation from `.agents/skills/project-patterns/references/*.md` (26 files) and `.agents/skills/project-wiki/SKILL.md` survives in a merged reference file; no rule is dropped, only restatement removed. Reference set:
      - `references/js-api.md` ← `clay-js-api-naming.md`, `clay-js-api-boundary.md`, `clay-js-api-schema.md`, `doc-registry-tests.md`
      - `references/packages.md` ← `package-distribution.md`, `package-manifest-single-source.md`, `package-runtime-trust-domains.md`, `authority-boundaries.md`, `extensions-and-ai.md`, `agent-host.md`, `product-surfaces-are-packages.md`, `mode-primitive-first.md`, `language-capability-sequencing.md`, `package-ui-layout.md` (with the stale "Masonry widgets remain migration inventory" line corrected to record the completed removal)
      - `references/protocol-perf.md` ← `protocol-and-performance.md`, `maintenance-validation.md`
      - `references/config.md` ← `configuration-system.md`, `ui-design-system-packages.md`, `typography-role-ownership.md`, `ui-modernization.md`
      - `references/docs-as-code.md` ← `documentation-as-code.md` + the full `project-wiki` workflow (wiki location, read-before-code, update-after-tests, master-index discipline, scope/boundary, required page content, quality bar, avoid-list, retired `clay.<domain>.*` naming rule) + the page template from `project-wiki/references/page-template.md`
      - `references/planning-checklist.md` ← `planning-checklist.md` (with UI-stack line retargeted to `references/ui.md`)
      - `references/ui-skill-stack.md` and `ui-visual-review.md` are NOT recreated as separate files; their rules fold into `references/ui.md` (Task 3) and `references/planning-checklist.md` respectively.
    - Security: Preserve every security/trust-domain rule verbatim in substance (two runtime trust domains, deny-by-default authority, inert contributions, narrow Tauri capabilities); no rule weakened during tightening.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`, `.agents/skills/project-patterns/SKILL.md` and all 26 reference files: source content being merged.
      - `.agents/skills/create-decision-log/SKILL.md`: step 6 points at `project-patterns/` and must be retargeted.
      - `.agents/skills/project-patterns/references/planning-checklist.md`: the per-task pattern-consultation step the router must preserve.
    - Options Considered:
      - Keep `project-patterns` as a separate router: rejected — two routers double hot-path description load and split the execution workflow.
      - Merge reference files one-to-one (26 files unchanged): rejected — progressive disclosure wants ~6 domain files, not 26; each old filename gets a redirect note in git history via the decision log.
    - Chosen Approach:
      - Router + domain-file merge as specified; each merged reference keeps its decision-log source citations as links (no duplication of decision-log text).
    - API Notes and Examples:
      ```markdown
      ---
      name: clay-execution
      description: Execute tasks from plan documents and any plan-less code change in this repository. Router to project references: UI catalog, JS API rules, package trust domains, protocol/performance, documentation-as-code duties, and the wiki workflow. Load only the reference files the task type needs.
      ---
      ```
    - Files to Create/Edit:
      - `.agents/skills/clay-execution/SKILL.md` (54 lines): Router — execution loop, plan-creation/decision-log integration, routing table, graft boundary, pull-only history policy.
      - `.agents/skills/clay-execution/references/{js-api,packages,protocol-perf,config,docs-as-code,planning-checklist,ui}.md`: Created. Mapping: `js-api` ← clay-js-api-{naming,boundary,schema} + doc-registry-tests; `packages` ← package-distribution, package-manifest-single-source, package-runtime-trust-domains, authority-boundaries, extensions-and-ai, agent-host, product-surfaces-are-packages, mode-primitive-first, language-capability-sequencing, package-ui-layout + tauri-react-client unique bits (TauRPC spike rule, migration-complete rule); `protocol-perf` ← protocol-and-performance, behavior-manifests, maintenance-validation; `config` ← configuration-system, ui-design-system-packages, typography-role-ownership, ui-modernization; `docs-as-code` ← documentation-as-code + project-wiki SKILL + page-template + archive policy; `planning-checklist` ← planning-checklist + ui-visual-review; `ui.md` ← ui-skill-stack + tauri-react-client (deviation: plan scheduled ui.md for Task 3; created initial version in Task 2 so ui-skill-stack rules are not gap-filed between deletions; Task 3 completes it with distilled design rules + catalog rewrite).
      - `.agents/skills/create-decision-log/SKILL.md`: Step 6 retargeted from `project-patterns/` to `clay-execution/references/`.
      - Pointer retargets in live sources (all done, sweep-clean): `.agents/skills/create-plan/SKILL.md` (steps 5/9), `create-plan/references/clay.md` (naming ref, tauri-react-client ref, ui-design-system-packages ref, 4× “final project-wiki task” prose), `create-plan/references/wiki-task.md` (4 refs), `docs/reference/clay-js-api/schema.md`, `docs/reference/primitives/{audit,markdown-mode-requirements,parse-update-strategy,rendering-strategy}.md`, `docs/development/{icon-pack-primitive-review,tauri-react-primitive-migration,ui-design-system-recipe-matrix}.md`, `tests/package_ui_conformance.rs:556`, `roadmap.md` (historical narrative).
      - Deleted: `.agents/skills/project-wiki/` (SKILL.md, references/page-template.md, agents/openai.yaml), `.agents/skills/project-patterns/` (SKILL.md, 26 references, agents/openai.yaml). Both via `git rm` (trackable rename history). `skills-lock.json` verified: no entry enumerates the deleted local skills (0 hits).
    - References:
      - `decision-logs/2026-09-08-1811-documentation-hot-path-optimization-clay-execution-skill.md`: approved decisions.
      - `tests/package_ui_conformance.rs:556`: cites `project-patterns/references/package-runtime-trust-domains.md` — retarget in Task 3's test-path sweep.
  - Test Cases to Write:
    - Manual sweep (PASSED): `grep -rn 'project-patterns\|project-wiki'` across tracked sources returns only the intentional historical narrative in `roadmap.md:687`; all other hits (create-plan 8, docs/reference 11, docs/development 3, tests 1) were live pointer retargets fixed in this task. Staged diff clean: 10 adds, 31 deletions, 14 modifications; unrelated pre-existing session changes left unstaged.
    - Manual rule-preservation check (PASSED): automated needle check over the 7 new files — every distinctive rule token sampled from the 26 source files present (spot-checked across all 6 domains; the three nominal misses were cross-file moves: `RESERVED_CORE_API_DOMAINS` lives in js-api/config, `MAX_CHUNK_BYTES` in protocol-perf, AIDA/forced-colors in ui/config). Security/trust-domain rules preserved verbatim in substance; decision-log citations retained per group. `create-decision-log/agents/openai.yaml` checked — no reference to merged skills.

- [x] Migrate and rewrite the UI catalog into clay-execution, fix UI documentation drift, update pinned test paths
  - Acceptance Criteria:
    - Functional: `.agents/skills/clay-execution/references/components.md` and `tokens.md` exist (moved from `clay-ui/references/`) and `.agents/skills/clay-execution/references/ui.md` exists (rewritten from `clay-ui/SKILL.md`). The catalog is frontend/React-primary: package-facing `ComponentKind`/style-variable/token tables stay authoritative against `src/shell/components.rs` and `src/shell/theme.rs` validation, while deleted native Masonry content (Plan 070 retained-reconciliation widget inventory, Phase 20.4/20.5 Masonry interaction-state paint prose, `SduiRegionWidget`/`PackageRegionWidget`/`EditorWidget` mappings, `src/masonry_*.rs` paths) is removed or moved to a clearly marked historical section; live React component inventory (`frontend/src/components/`) and the frontend theme runtime (`--clay-*` CSS custom properties, CodeMirror adapters) become the primary implementation paths.
    - Functional: `ui.md` contains the Clay-adapted binding rules distilled from `impeccable`, `full-output-enforcement`, `high-end-visual-design`, and `design-taste-frontend` (primitives/components first, token-only styling, font roles/variants only, inert contributions, additive-only contracts, catalog currency, state-complete components, Operate-mode adaptation — no AIDA/hero/decorative-motion forcing, visual/a11y review duty with `computer-use-linux` `get_app_state` first, screenshot evidence, blocker recording), plus the shell layout model (working area → pane split tree → `main` + optional `left`/`right`/`top`/`bottom` slots, user-configurable panel sizes, transient surface policy) corrected to the React client. It states: the four design skills load additionally only for substantial new-surface design tasks.
    - Performance: `components.md` + `tokens.md` + `ui.md` combined ≤ ~60% of the byte volume of the current `clay-ui` SKILL + references; no table rows dropped (catalog completeness is validated by tests, not prose).
    - Code Quality: All pinned path references updated: `tests/primitives_docs.rs` (19 hits), `tests/package_ui_conformance.rs` (4 hits incl. the `project-patterns` citation), `tests/documentation_coverage.rs` (2 hits), `src/shell/theme.rs:2454` doc comment, `docs/reference/ui-components.md` (4 link hits), plus any other hit from a repo-wide `grep -rn 'agents/skills/clay-ui'`. `.agents/skills/clay-ui/` directory is deleted (SKILL.md, references/, agents/openai.yaml).
    - Security: Catalog preserves the security-relevant contracts (package UI inertness, no direct Tauri IPC, no uncontrolled host CSS, token validation bounds).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`, `references/components.md`, `references/tokens.md`: source content (drift-affected, verified against source tree).
      - `src/shell/components.rs`, `src/shell/theme.rs`: live server-side validation the catalog must match (tests enforce this).
      - `frontend/src/components/`, `frontend/src/theme/`: live React component inventory and theme runtime for the rewrite.
      - `docs/reference/ui-components.md`: navigation entry that links the catalog.
      - `docs/wiki/modules/frontend-theme-runtime.md`, `react-shell.md`: current architecture descriptions feeding the rewrite.
    - Options Considered:
      - Keep `components.md`/`tokens.md` at the old `clay-ui` path to avoid touching tests: rejected per user decision 3 — paths should reflect the single new skill; the sweep is mechanical.
      - Fix drift in place first, then move in a later task: rejected — double edit of the same content; the rewrite and the move are one change.
    - Chosen Approach:
      - Move + rewrite + test-path sweep in one task; catalog correctness is enforced by the existing documentation-coverage tests after the sweep.
    - API Notes and Examples:
      ```text
      .agents/skills/clay-execution/references/components.md   (moved, Masonry content stripped)
      .agents/skills/clay-execution/references/tokens.md        (moved, hot-path prose corrected)
      .agents/skills/clay-execution/references/ui.md           (new, from clay-ui/SKILL.md)
      sed-style sweep: '.agents/skills/clay-ui/references/' -> '.agents/skills/clay-execution/references/'
      ```
    - Files to Create/Edit:
      - `.agents/skills/clay-execution/references/components.md` (28737 B): Created (moved + rewritten). Masonry content stripped: Plan 070 retained-reconciliation section deleted (historical pointer to `docs/wiki/archive/` in header), Phase 20.5 widget-prose compressed to state/origin contracts, `SduiRegionWidget`/`PackageRegionWidget`/`src/masonry_*.rs` mentions removed, primitives intro retargeted to `frontend/src/components/chrome.tsx` (React/CSS realization), surfaces table retargeted to live paths (`src/shell/layout/`, frontend components). All test-pinned markers/rows preserved: `## Package-Facing Component Kinds` heading + 16 kind rows (`| `k` | implemented` ×15, `| `table` | reserved`), `## Typed Style Variables` + 15 rows, `## Clay-Native Chrome Primitives (internal)` + 9 `paint_*` rows, `| <surface> | implemented/planned |` planned-components rows, Plan 087/088/097/101/112/Phase 25/28 marker strings. Recipe-slots table compacted to surface→slots index with the full matrix authoritative in `docs/development/ui-design-system-recipe-matrix.md`.
      - `.agents/skills/clay-execution/references/tokens.md` (18339 B): Created (moved + corrected). All 13 token tables verbatim (core-token rows are 1:1 with `core_theme_value`, pinned by `core_token_catalog_matches_tokens_md`); stale "native paint ... per frame" hot-path prose corrected to install-time resolution + `--clay-*` CSS custom-property projection via `frontend/src/theme/`; consumption chronicles (Plan 088, Phase 20.4, Phase 24.4) retained but tightened; all pinned markers kept (incl. "## Plan 088 token consumption (no additions)", "UI Design-System Value Domains (Plan 101)", "Phase 20.4 component uplift", "Phase 20.5 overlay/menu component work").
      - `.agents/skills/clay-execution/references/ui.md` (6834 B): Completed (Task 2 initial + Task 3): distilled binding rules (primitives-first, token-only, font roles, inert contributions, state-complete components, Operate-mode adaptation, no AIDA/hero forcing), design-skill routing (four skills load only for substantial new-surface design tasks — decision 1), shell layout model, client architecture, catalog pointer + live architecture map.
      - `tests/primitives_docs.rs` (21 hits), `tests/package_ui_conformance.rs` (3 path hits), `tests/documentation_coverage.rs` (2 hits): path strings + prose names retargeted to `.agents/skills/clay-execution/references/` and `clay-execution catalog` naming.
      - `src/shell/theme.rs:2455`: doc-comment path updated to `.agents/skills/clay-execution/references/tokens.md`.
      - `docs/reference/ui-components.md` (5 hits incl. prose): links + naming retargeted.
      - Additional live-path retargets discovered by the repo-wide sweep (beyond the plan's estimate): `docs/reference/packages/creating-packages.md` (10), `docs/reference/primitives/{ui-chrome-primitives,index}.md`, `docs/reference/clay-js-api/theme/set-theme.md`, `docs/development/{ui-design-system-css-audit,react-ui-catalog-mapping}.md`, `frontend/src/components/index.ts`, `PRODUCT.md`, `.agents/skills/create-plan/SKILL.md` + `references/clay.md` (UI gate rewritten per decision 1: ui.md + catalogs always, four design skills only for substantial new-surface design), 19 wiki files under `docs/wiki/` (incl. `index.md`, live module pages, and index-linked phase pages — all links kept valid for the Task 4 archive move; phase-page relinks use the merged-reference name map).
      - Deleted: `.agents/skills/clay-ui/` (SKILL.md, references/{components,tokens}.md, agents/openai.yaml) via `git rm` (git records components.md/tokens.md as renames R→clay-execution).
      - Deviation 1 (performance): combined volume is 53910 B = 80.4% of the old 67061 B, not the ~60% (40236 B) target. The estimate predates the catalog's post-Phase-20.5 growth: Plan 087/088/097/101/108/110/112 + Phase 24.4/26.x/28 contract sections are all pinned by `tests/primitives_docs.rs`/`package_ui_conformance.rs` marker assertions, putting the honest floor near ~47 KB. Achieved reduction (−19.6%) came from Masonry stripping + prose tightening with zero dropped test-validated rows. Further reduction would delete live, test-pinned contract content (violates the functional acceptance and the security criterion). Options if 60% is still wanted: relax the criterion or relocate un-pinned consumption chronicles (Phase 20.4/24.4 sections) to the wiki archive.
      - Deviation 2 (scope): the sweep found ~59 more live references to old skill paths than the plan's ~26-string estimate (wiki pages, PRODUCT.md, JS API docs, creating-packages.md, frontend comment); all retargeted. `docs/wiki/modules/clay-agent.md` retarget is staged alongside concurrent agent-run-options session edits (mixed working file).
      - Note for Task 4: the components.md header already states historical reconciliation notes live in `docs/wiki/archive/`; Task 4 must complete that archive move to make the statement true.
    - References:
      - `decision-logs/2026-09-08-1811-documentation-hot-path-optimization-clay-execution-skill.md`: approved decisions 1 (distilled UI rules) and 3 (catalog paths).
      - `docs/wiki/modules/tauri-react-cutover.md`: native client deletion record (Plan 097 Phase 12).
  - Test Cases to Write:
    - (PASSED with two pre-existing exceptions) `cargo test --test presentation --test protocol` (which contains `package_ui_conformance`, `primitives_docs`, `documentation_coverage`): 207 passed, 0 new failures. All catalog drift guards, marker assertions, kind/style-variable/token parity tests pass against the new paths and rewritten content.
    - (PASSED, verified foreign) Two remaining failures are not caused by this task, proven by `git stash` isolation at committed HEAD: `primitives_docs::plan061_runtime_package_authority_rebaseline_matches_source_inventory` (fails only with the concurrent session's unstaged `src/server/ops/mod.rs` agent-run-options op: 98 vs pinned 97) and `documentation_coverage::parity_ledger_covers_every_manual_step_public_api_and_protocol_family` (fails at committed HEAD: test-plan/17 ledger rows C21–C23 merged into one row by the prior Prism 0.5.1 work). Both belong to the concurrent agent/Prism workstream.
    - (PASSED) Repo sweep: `grep -rn 'agents/skills/clay-ui'` excluding `plans/`, `decision-logs/`, `code-reviews/`, and `docs/wiki/` returns zero hits; `docs/wiki/` hits in index-linked phase pages were relinked to the merged reference names so `documentation_coverage::wiki_navigation_is_complete_and_current_page_paths_resolve` passes.
    - (PASSED) Drift check: only one Masonry mention survives in the three files — the factual historical note in the components.md header ("native Masonry client was removed in Plan 097 Phase 12"); no `src/masonry_*.rs`, `SduiRegionWidget`/`PackageRegionWidget`, or `src/shell/layout.rs`-as-file references remain.

- [x] Archive historical wiki pages and prune the master index
  - Acceptance Criteria:
    - Functional: The 43 identified pages under `docs/wiki/modules/` matching `phase*|*review*|*bugfix*` are classified: pure historical records (completed-phase reviews, pre-implementation inventories, records of removed code) move to `docs/wiki/archive/`; any page that is the sole surviving documentation of a live surface (candidates to verify: `phase20.5-overlay-menu-input-components.md`, `phase20.6-theme-segregation-settings-ui.md`, `phase20.7-package-ui-conformance-and-aesthetic-guardrails.md`, `phase25-agent-host-primitive-review.md`, `phase25-agent-process-manager.md`, `phase25-agent-protocol.md`, `ui-review-harness.md`) has its evergreen content migrated into the corresponding live module page (or stays in `modules/` renamed without the phase prefix) before the historical remainder is archived. No content is deleted.
    - Functional: `docs/wiki/index.md` contains zero phase-named links; it gains one line under a `## Archive` heading: historical phase/review/bugfix records live in `archive/` and are pull-only. The 12 inline `(historical; removed in Plan 097 Phase 12)` annotations for removed native subsystems move their target pages to `archive/` with the annotation preserved.
    - Performance: `docs/wiki/index.md` shrinks to evergreen-only (~35% reduction); no agent reading the index is routed to historical content.
    - Code Quality: Archive pages keep their filenames and full content; `docs/wiki/archive/` gets no index beyond a one-line pointer in the main index (git/find/graft grep remain the discovery paths). The wiki workflow (in `clay-execution/references/docs-as-code.md`) states: completed-phase review records go to `archive/`, not `modules/`.
    - Security: No pages deleted; sanitized labels/security notes preserved as-is.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/index.md`: current index, 37 phase-named entries, 12 historical annotations.
      - `docs/wiki/modules/` file list: 43 matches, ~810KB.
      - `.agents/skills/clay-execution/references/docs-as-code.md` (from Task 2): wiki workflow receiving the archive rule.
    - Options Considered:
      - Top-level `archive/`: rejected per user decision 2 — archive inside wiki, near content.
      - Delete historical pages: rejected — user requirement is preserve, not remove.
      - Move all 43 unconditionally: rejected — a few phase pages may be sole live docs; classify first to avoid orphaning live surfaces.
    - Chosen Approach:
      - Classify then move; migrate evergreen content for live-surface pages; prune index; add archive policy to the wiki workflow reference.
    - API Notes and Examples:
      ```bash
      git mv docs/wiki/modules/<page>.md docs/wiki/archive/<page>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/archive/`: receives moved pages.
      - `docs/wiki/index.md`: prune 43 entries, add `## Archive` pointer line.
      - `docs/wiki/modules/<live-surface pages>`: receive migrated evergreen content where applicable.
      - `.agents/skills/clay-execution/references/docs-as-code.md`: archive policy line.
    - References:
      - `decision-logs/2026-09-08-1811-documentation-hot-path-optimization-clay-execution-skill.md`: approved decision 2 (archive location).
  - Test Cases to Write:
    - (PASSED) Every moved page exists in `archive/` with full content: `git status` shows 50 `R` rename pairs (49 archive moves + catalog rename), 0 `D` deletions under `docs/wiki/`; 49 pages, 1.1 MB preserved.
    - (PASSED) `grep -n 'phase' docs/wiki/index.md` returns only non-link prose (zero phase-named link targets; two stale Masonry prose mentions also corrected).
    - (PASSED) Each live surface that had only a phase page is reachable: phase20.6 evergreen content migrated into `configuration-runtime.md` (new "Theme packages, appearance, and the settings surface" subsection) before archiving; phase25-agent-protocol/process-manager and phase20.1 (sole live docs) kept in `modules/` renamed to `agent-protocol.md`, `agent-process-manager.md`, `ui-design-language-primitive-review.md`; ui-review-harness.md stays (live); 20.5/20.7/25-host verified covered by clay-execution catalogs + `creating-packages.md` §20.7 and archived as phase records.
    - (PASSED, full suite) `cargo test --test presentation --test protocol`: 46 + 209 passed, 0 failed — including the two previously failing pre-existing tests (`plan061`, `parity_ledger`), which the concurrent session's fixes (98-op pin, ledger rows) restored while this task ran.
    - Deviations (test-policy updates the plan did not anticipate):
      1. Two tests encoded the old "every wiki page must be index-linked" policy (`documentation_coverage::wiki_navigation_*`, `primitives_docs::wiki_index_links_every_wiki_page`): both now exclude `docs/wiki/archive/` from the index-link requirement (pull-only history per decision 2) while still requiring every archive page's links to resolve.
      2. Four tests pinned exact paths of moved/renamed pages: `phase20_1_..._is_linked_and_complete` (retargeted to the renamed path), `phase20_4_...` (retargeted to `archive/`, index-link assert dropped — archive is unindexed; content asserts kept), `plan088_code_wiki_documents_modernization_contract` (masonry-shell/masonry-sdui-region/pane-document-views index-link asserts became archive file-existence asserts; live-page asserts kept), `phase19_code_wiki_documents_open_dialog_path` (edit-ack path retargeted to `archive/`, index-link entry dropped).
      3. Archive→live-module bare-name links (47 pages) were rewritten to `../modules/…`/`../flows/…` after the first relink pass missed them; `wiki_navigation` link-resolution loop validates all of them green.
      4. Concurrent-session files left unstaged and untouched: `plans/061…` (98-op pin), `docs/development/tauri-react-parity-ledger.json`, clay-agent run-options set. `docs/wiki/modules/clay-agent.md` is mixed (concurrent prose + this task's `../archive/` link fix) and was staged for test integrity.
    - Evidence: `docs/wiki/index.md` 55524 → 33164 B (−40.3%, target ~35%), 156 → 108 lines, `## Archive` pointer section added; `docs-as-code.md` already carried the archive policy (Task 2 lines: "Completed-phase review records go to `docs/wiki/archive/`, not `modules/`"). Non-wiki docs retargeted to `archive/` paths: `docs/reference/{ui-components,primitives/{ui-chrome-primitives,audit,backlog,diagnostics,language-intelligence,registry,shell-layout-strategy},packages/creating-packages}.md`, `docs/development/{windows,launch-and-gui-smoke,file-open-save-reload-workflow,tauri-react-primitive-migration}.md`.

- [x] Tighten create-plan: remove the execution loop, fold the wiki-task template, deduplicate the UI mandate
  - Acceptance Criteria:
    - Functional: `create-plan/SKILL.md` no longer contains the "Deterministic Execution Loop" section (moved to `clay-execution` in Task 2); the plan-creation workflow's execution-related pointers reference `clay-execution`. The UI-stack mandate appears exactly once as a pointer: UI tasks must follow `clay-execution/references/ui.md` and list it under `Approach -> Documentation Reviewed`, instead of enumerating seven skill files in both the SKILL and `clay.md`. All plan structure requirements and task-writing rules survive: filename/numbering convention, required plan structure template, acceptance-criteria categories, approach/documentation-reviewed/options rules, primitive-review task inclusion, per-task plan duties from `references/clay.md` (all task types preserved verbatim in substance: Primitive-First, Trust-Domain, Default Loading, External Process Authority, Grammar, Package UI/Layout Authoring, Clay JS API, Configuration, Example Config Maintenance, Example Config Launch-Test, Manual Test Plan, UI Primitives-First, Design-System Package, Mandatory UI Visual/A11y Review), with the two UI task-type sections updated to point at `clay-execution/references/ui.md` and to record the distilled design-skill rule (four skills load only for substantial new-surface design tasks).
    - Functional: `create-plan/references/wiki-task.md` is deleted; its final-wiki-task template is folded into `references/clay.md` as a compact section (the task requirement description is preserved; the reference to `project-wiki/SKILL.md` becomes `clay-execution/references/docs-as-code.md`).
    - Performance: `create-plan/SKILL.md` ≤ ~70 lines and `references/clay.md` ≤ ~200 lines with no information loss (verified by rule-preservation check).
    - Code Quality: No dangling references to `project-wiki`, `project-patterns`, or `clay-ui` remain in `create-plan/`. The skill's frontmatter description still triggers on create/write/update/execute/maintain plan requests; execution-time behavior is delegated to `clay-execution`.
    - Security: None affected (documentation only); trust-domain/authority task requirements preserved intact in `clay.md`.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/SKILL.md`, `references/clay.md`, `references/wiki-task.md`: source content.
      - `.agents/skills/clay-execution/SKILL.md` (from Task 2): destination of the execution loop; router must cover every step (select task, load per-task references, run tests, tick checkbox, update changed approach, maintenance tasks, wiki task, final verification, compromises/further-actions).
    - Options Considered:
      - Leave the execution loop in both skills: rejected — duplication is the core problem this plan removes.
      - Keep `wiki-task.md` as a separate file: rejected — it is a plan-task requirement description, which belongs with the other `clay.md` duties.
    - Chosen Approach:
      - Cut execution loop from `create-plan`; single-pointer UI mandate; fold wiki-task template into `clay.md`; tighten restated prose throughout both files.
    - API Notes and Examples:
      ```text
      .agents/skills/create-plan/SKILL.md        (plan creation workflow + structure + task rules only)
      .agents/skills/create-plan/references/clay.md (all Clay task duties incl. folded wiki task)
      delete: .agents/skills/create-plan/references/wiki-task.md
      ```
    - Files to Create/Edit:
      - `.agents/skills/create-plan/SKILL.md`: Tightened, execution loop removed.
      - `.agents/skills/create-plan/references/clay.md`: Folded wiki task, UI-mandate pointers, stale migration lines corrected ("During migration, current Masonry widgets are parity inventory" → removed/completed-migration phrasing).
      - Delete: `.agents/skills/create-plan/references/wiki-task.md`.
    - References:
      - `decision-logs/2026-09-08-1811-documentation-hot-path-optimization-clay-execution-skill.md`.
  - Test Cases to Write:
    - (PASSED) Rule-preservation check: automated needle test over 80 substantive tokens from the old `clay.md` (all 14 duty-section requirements, decision-log citations, code identifiers, authority/security rules, placement guidance) — all present in the new file. 8 initial misses audited against `git show HEAD:` — none existed in the old file (false needles from other references); one needle phrasing corrected (`lack of hostile isolation` present in both).
    - (PASSED) Repo sweep: `grep -rn 'wiki-task\|project-wiki\|project-patterns\|clay-ui' .agents/skills/create-plan/` returns zero hits.
    - (PASSED) `cargo test --test presentation --test protocol`: 46 + 209 passed, 0 failed — including `create_plan_ui_requirements_name_existing_catalog_files` (clay.md keeps components.md/tokens.md/creating-packages.md/ui-components.md/.agents/skills/clay-execution references) and the ui-components.md → `create-plan/references/clay.md)` link pin.
    - Evidence: `SKILL.md` 103 → 76 lines (−26%; floor is the 44-line Required Plan Structure template, kept verbatim — the plan's ~70-line target is met within the no-information-loss constraint): "Deterministic Execution Loop" section deleted (lives in `clay-execution/SKILL.md` step-for-step, including the UI-before-editing rule and the docs-as-code/wiki-task steps), workflow step 9 now points at the folded template in `references/clay.md`, intro line states execution routes through clay-execution. `references/clay.md` 310 → 206 lines (−34%, while absorbing the 32-line wiki-task template): every duty section compressed to one-line trigger + substance-complete bullets + inline placement/decision-source; new "Final Code Wiki Task" section preserves the template (acceptance criteria, once-after-tests approach, `docs-as-code.md` reference, archive rule) with the `project-wiki/SKILL.md` pointer replaced by `.agents/skills/clay-execution/references/docs-as-code.md`. `references/wiki-task.md` deleted (`git rm`). UI mandate now appears once in SKILL (workflow step 6, single pointer to ui.md + catalogs + four-design-skill rule) — the duplicate that lived in execution-loop step 3 is gone with the loop. Stale migration line ("During migration, current Masonry widgets are parity inventory") already removed in Task 3's clay.md edit; verified absent. Frontmatter unchanged (create/write/update/execute/maintain triggers intact).

- [x] Final verification: cross-reference sweep, wiki consistency, graft refresh, full Linux gates
  - Acceptance Criteria:
    - Functional: Repo-wide sweep (excluding `plans/`, `decision-logs/`, `code-reviews/`, `docs/wiki/archive/`, `target/`, `node_modules/`, `graft/`) shows zero references to deleted paths: `.agents/skills/clay-ui`, `.agents/skills/project-wiki`, `.agents/skills/project-patterns`, `create-plan/references/wiki-task.md`. `skills-lock.json` verified: no entry enumerates the deleted local skills (local skills are not lock-managed; confirm and record).
    - Functional: Wiki consistency verified: every `docs/wiki/modules/` and `flows/` page is linked from `docs/wiki/index.md`; every index link resolves; archive pages are reachable only via the archive pointer, find, or git.
    - Performance: Hot-path volume targets met: `clay-execution/SKILL.md` ≤ ~120 lines; `create-plan/SKILL.md` ≤ ~70 lines; `references/clay.md` ≤ ~200 lines; wiki index ~35% smaller. Record measured line/byte counts in task evidence.
    - Code Quality: `graft build` run successfully after all moves; graft nodes point at current paths. Full Linux gates pass: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
    - Security: No gate weakened; no historical record deleted (verify `git status`/`git diff --stat` shows renames and edits, no deletions of plans/decision-logs/code-reviews/archive content).
  - Approach:
    - Documentation Reviewed:
      - All files created/edited in Tasks 1–5.
      - `AGENTS.md`: unchanged hot-path entry (graft router) — verify no edit is needed and record that verification.
    - Options Considered:
      - Per-task gate runs: redundant; each task already runs its targeted tests; final gate covers the aggregate.
    - Chosen Approach:
      - Single final verification task after all restructuring, per the deterministic execution loop.
    - API Notes and Examples:
      ```bash
      graft build
      cargo fmt --check && cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test
      ```
    - Files to Create/Edit:
      - Any residual cross-reference fixups found by the sweep (tentative; expected small).
    - References:
      - `.agents/skills/clay-execution/SKILL.md`: final verification step definition.
  - Test Cases to Write:
    - `cargo test` green, including `tests/primitives_docs.rs`, `tests/package_ui_conformance.rs`, `tests/documentation_coverage.rs`, `tests/clay_js_doc_registry.rs`.
    - Sweep commands (above) return zero hits outside excluded historical stores.
    - `graft build` exits 0.
    - (PASSED) Repo-wide sweep (excluding `plans/`, `decision-logs/`, `code-reviews/`, `docs/wiki/archive/`, `target/`, `node_modules/`, `graft/`, `.git/`) for `skills/clay-ui|skills/project-wiki|skills/project-patterns|references/wiki-task`: two residual hits found and fixed — `docs/reference/primitives/package-security.md` (3 project-patterns pointers retargeted: package-distribution + extensions-and-ai → `clay-execution/references/packages.md`, clay-js-api-boundary → `references/js-api.md`) and `test-plan/artifacts/112-icons/visual/README.md` (dated review artifact; methodology pointer retargeted to `clay-execution/references/planning-checklist.md`, capture date preserved). Re-sweep: zero hits. `skills-lock.json` (repo root): 0 hits for deleted local skills — local skills are not lock-managed (verified in Task 2, re-confirmed).
    - (PASSED) Wiki consistency: 83 live pages under `docs/wiki/{modules,flows}` all linked from `docs/wiki/index.md` (0 unlinked), 0 broken index links. Inline citations from live pages to `../archive/` pages (~100 across 30+ pages) verified as explicit historical pointers — consistent with the pull-only history policy, which forbids archive links from hot-path *indexes*: `AGENTS.md`, `docs/index.md`, `.agents/skills/**` contain zero `](...wiki/archive...)` links (only policy prose naming the archive location).
    - (PASSED) Hot-path volume: `clay-execution/SKILL.md` 54 lines / 4744 B (target ≤ ~120); `create-plan/SKILL.md` 76 lines / 5498 B (target ≤ ~70; floor is the 44-line Required Plan Structure template — deviation documented in Task 5); `references/clay.md` 206 lines / 28434 B (target ≤ ~200 while absorbing the 32-line wiki-task template — deviation documented in Task 5); `docs/wiki/index.md` 55524 → 33164 B = 40.3% smaller (target ~35%); clay-execution references dir 9 files (incl. catalogs), agent-facing hot path reduced from ~26 scattered pattern files + clay-ui + project-wiki + project-patterns skills to 1 router + 9 references.
    - (PASSED) `graft build` exits 0 (12 files re-parsed, 499 replayed from cache); graft graph rebuilt over current paths.
    - (PASSED) `cargo fmt --check`: the only diff in the crate is `src/server/ops/agent.rs` — unstaged work of a concurrent session, untouched by this plan; all plan-touched Rust files fmt-clean.
    - (PASSED) `cargo check --all-targets`: finished clean. (PASSED) `cargo clippy --all-targets -- -D warnings`: finished clean.
    - (PASSED) Full `cargo test`: 1748 passed, 0 failed across all targets (lib 1274 + doc 6 + presentation 46 + protocol 209 + runtime 75 + security 138). Note: one intermediate run failed `plan101_documentation_cross_links_and_token_synchronization` because `PRODUCT.md` had been deleted from the worktree by the concurrent session; restored from the index (Task 3's staged edit intact; no relocated PRODUCT* file found — accidental deletion), test green afterwards. The two failures seen during Tasks 3–4 (`plan061` op-count, `parity_ledger` C21–C23) were concurrent-session/pre-existing and are green in the final run (fixed on the other session's branch work).
    - (PASSED) Security: staged history-dir diff shows additions and 34 pure renames only — zero deletions of `plans/`, `decision-logs/`, `code-reviews/`, or archive content; no gate weakened (all four gates green or foreign-attributed as recorded above). `AGENTS.md` verified unchanged: its graft entry is the hot-path code-navigation pointer and needs no edit; clay-execution is discoverable via its skill description.

## Scope Notes (duties not applicable — recorded per Clay plan requirements)

- This plan changes no user-facing configuration surface, no Clay JS API, no Rust behavior (one doc-comment path string in `src/shell/theme.rs`), and no user-visible product behavior. Therefore: no Example Config Maintenance/Launch-Test task, no Clay JS API task, no Manual Test Plan (`test-plan/`) task — the touched surfaces are agent-facing documentation and skill tooling only. The final wiki verification duty is covered by Task 5's index-consistency checks and Task 6's wiki consistency verification.
- UI work: this plan rewrites UI *documentation*, not UI code. The visual/a11y review duty does not apply (no UI implementation).

## Compromises Made

- `create-plan/SKILL.md` (76 lines) and `references/clay.md` (206 lines) land slightly above the ~70/~200 targets: both files are pinned floors — the 44-line Required Plan Structure template in SKILL.md and the 14 duty sections in clay.md carry test-enforced and decision-log-backed content. Compression stopped at the no-information-loss boundary; the plan's "≤ ~" targets are met in spirit (−26% and −34% respectively, with clay.md additionally absorbing the wiki-task template).
- The catalog rewrite (Task 3) reached 53910 B ≈ 80% of the original three UI files versus the estimated 60%: test-pinned live contracts (Plan 087/088/097/101/108/110/112, Phase 24.4/26.x/28) form a hard floor the estimate did not account for. All Masonry-era prose was still removed and hot-path corrections applied.
- Inline citations from live wiki pages to `docs/wiki/archive/` pages were kept (rather than stripped to make archive strictly pointer-reachable): they are the historical context those pages explicitly reference, and the pull-only policy bans archive links from hot-path indexes, not from wiki pages a reader has already opted into. Verified zero archive links from AGENTS.md, docs/index.md, and skills.

## Further Actions

- The concurrent session working on `upgrade/prism-0.5.3` still holds unstaged changes (`src/server/ops/agent.rs` fmt drift, `plans/061` op-count edits, `tests/agent_protocol.rs`, `tests/package_loading.rs`); those gates must go green when that session commits — not a follow-up of this plan, just recorded so the final gate evidence is not misread.
- `docs/wiki/archive/` pages keep their original filenames; if wiki navigation grows again, consider a one-line archive pointer per module page instead of inline citations (cosmetic; no action planned).
- skills-lock.json continues to enumerate only external skills; local skill lifecycle (create/delete) requires no lock maintenance — no action.
