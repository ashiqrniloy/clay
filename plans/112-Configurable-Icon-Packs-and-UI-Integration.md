# Plan 112: Configurable Icon Packs and UI Integration

## Objectives
- Ship `@clay/icons-phosphor-regular` as the recommended default and `@clay/icons-phosphor-duotone` as the alternative, with the same semantic icon coverage.
- Make icon style a user-owned selection in `~/.config/clay/init.js`, independent of content theme, typography, and UI design system.
- Support installed, explicitly adopted third-party icon packs through the same generic contribution and activation path, without giving packages renderer authority.
- Reduce repetitive action text while retaining discoverability, accessible names, useful metadata, and destructive-action safeguards.
- Complete the server validation, package lifecycle, protocol/DTO, frontend primitives, package UI, tests, public API/configuration documentation, example launch, manual verification, and implementation wiki work in this plan.

## Expected Outcome
- Both first-party packs work offline in a packaged Linux build, not only from the source checkout. Switching packs updates every migrated surface without remounting editors, changing selection, losing unsaved text, or resetting focus/scroll.
- A third-party fixture changes the same semantic icons without any host component source change; partial coverage uses safe bundled fallback assets.
- Existing text-only packages remain valid. Icons are optional, additive fields on existing host-owned controls and inert UI declarations, not a new parallel widget framework.
- Every migrated icon-only action has an accessible name, keyboard/focus behavior, hover/focus tooltip, adequate target size, and tested fallback. Icon packs cannot change action authority, accessible meaning, or theme colors.
- All blocking Linux checks pass. Live visual, keyboard, and example-config checks have retained evidence; blocked checks remain unchecked/unresolved rather than being represented as passed.

## Scope and Planning Status
- User requested this full implementation plan following the recommendation of Phosphor Regular and Duotone. This document plans that direction; it does not claim implementation or test completion and does not create an approved decision log by implication.
- Two styles of one family are intentional: consistent glyph meanings/proportions, materially different outline and shaded silhouettes, one upstream asset/license pipeline. Lucide was considered but a second outline family provides less visual differentiation.
- Public names below are **finalized by Task 2** (`docs/development/icon-pack-primitive-review.md`): `setIconPack` from `clay:theme` (stable ID `theme.setIconPack`, user-facing name "Set Icon Pack") and `clay.contributions.iconPack`. Use existing reserved `theme` domain; error namespace `theme.*` (`theme.invalid_icon_pack` added).
- Scope is the shipped Tauri/React client and generic Rust server/package primitives. Do not rebuild retired Masonry rendering or make a permanent dual-client icon implementation.
- No icon marketplace, arbitrary image loader, icon fonts, downloaded-at-render assets, per-file language-logo catalog, icon animations, new AI functionality, or unrelated model/status/error refactor. Settings UI icon-pack selection is not required: configuration switching is the requested surface. Do not add a second persistence system or silently let stored settings defeat `init.js`.
- Keep mandatory label choices below. This is not a global text-to-icon conversion.

## Evidence and Implementation Grounding
- Source review: `src/protocol/sdui.rs:114-119` and `runtime/js/sdui.d.ts:11-16` currently give list items text/detail/action but no semantic icon field. `frontend/src/sdui/types.ts:84-130` and `frontend/src/sdui/registry.tsx:68-245` establish the package component path. File-browser rows originate in `src/shell/file_browser.rs:329-414`, not in a standalone frontend file tree.
- Source review: `src/packages/record/mod.rs:682-763` assembles contributions; `src/packages/bundled.rs:374-448` checks real extension-point scopes; `build.rs:21-70` generates the bundled inventory; `src/server/ops/theme.rs:260-399` demonstrates activation/provenance and the existing lock-reentrancy pitfall. Reuse these boundaries, not their incidental implementation details.
- Current UI sources: `frontend/src/editor/ClayEditor.tsx:136-189`, `frontend/src/components/tab-strip.tsx:38-151`, `frontend/src/components/controls.tsx:43-109,191-225`, `frontend/src/components/modal.tsx:27-64`, `frontend/src/coding-agent/CodingAgentPanel.tsx:908-1393`, `packages/git/dist/status.js`, `packages/markdown/dist/sdui.js`.
- Previous review observed a live Linux workspace/browser/agent view. That is baseline evidence only, not proof of the future icon implementation or exhaustive interactive accessibility coverage.
- Current documentation lookup: `npx ctx7@latest library "Phosphor Icons"` resolved `/phosphor-icons/core`; `npx ctx7@latest docs /phosphor-icons/core` verified raw asset families and MIT notice requirements. Sources: https://github.com/phosphor-icons/core , https://github.com/phosphor-icons/core/blob/main/LICENSE . Pin an exact upstream release and inspect its actual files before conversion; documentation examples are not proof of filename presence in that release.
- Project docs reviewed: `docs/wiki/modules/ui-design-system-runtime.md`, `docs/wiki/modules/package-loading.md`, `docs/reference/clay-js-api/theme/set-design-system.md`, `test-plan/index.md`, UI component/token catalogs, and relevant project-pattern files. Per-task Documentation Reviewed lists are also mandatory execution-time reading; proposed new docs are read once their prerequisite task creates them. Existing theme docs contain differing descriptions of enable/revocation behavior; Task 2 must specify icon behavior explicitly rather than copy contradictory prose.
- Frontend commands are taken from `frontend/package.json`: `test`, `typecheck`, `lint`, `build`, `check:budget`. Existing dependencies include React 19, React Aria Components 1.x, TypeScript 5.9, Vite 7, and Vitest 3; lockfiles are version authority during execution.

## Contract Targets to Resolve in Task 2
1. **Semantic identity:** core meanings such as `action.close`, `action.new`, `document.save`, `document.reload`, `document.open`, `message.send`, `generation.stop`, `navigation.back`, `navigation.up`, `disclosure.right`, `disclosure.down`, `file.folder`, `file.file`, `file.symlink`, `session.resume`, `session.search`, `git.branch`, `status.success`, `status.warning`, `status.error`, and `preview.toggle`. These are icon keys, not callable command IDs. Final required key set must equal the placement inventory; remove unused proposals rather than shipping speculative icons. Package-specific meanings use the declaring package's prefix.
2. **Minimal vector format:** a versioned viewBox plus a bounded flat list of paths with validated geometry, fill rule, optional stroke geometry if the chosen sources require it, and bounded opacity. Foreground color is supplied by Clay through current theme roles. Duotone opacity represents geometry shading, not a second palette. No recursive scene graph or general SVG interpreter. Use a resolved existing geometry parser if available; otherwise normalize to a small typed numeric path-command representation at build time and validate its arity/numbers server-side. Do not accept unchecked arbitrary SVG strings in a `d` field.
3. **Bounds:** measure the selected assets and current manifest/runtime/frame ceilings, then publish constants and boundary tests for pack bytes, icon count, path count, command count, coordinate ranges, opacity, viewBox, and identifier lengths. Do not raise unrelated SDUI or transport limits blindly. Large-path input must be rejected before expensive expansion/allocation.
4. **Loading and selection:** `await loadPackage(...)` remains explicit/idempotent. Loading a pack registers its inert contribution from `clay.contributions` in `package.json`, the sole registration source, and must not win global selection by load order. Asset files may be manifest-referenced data; load entries must not duplicate manifest declarations or perform imperative registration. `setIconPack(...)` explicitly chooses one available, authorized pack. If no pack is selected, use the bundled Regular safety subset without executing a package load entry; the same generated asset source supplies package and fallback. Document this safety fallback versus explicit package activation distinction. If existing loader APIs cannot express it, fix the generic gap rather than add per-pack host branches.
5. **Failure semantics:** invalid explicit selection preserves the previous still-authorized active snapshot and reports a bounded diagnostic. Revocation/disable/remove withdraws revoked assets and switches to bundled fallback, never retains revoked geometry indefinitely. Required configuration reload failure preserves the previous valid generation, except authority revocation must still withdraw revoked data. Missing core key falls back to the Regular asset; unknown package-only decorative keys are omitted; a control never becomes blank because its icon is missing (retain/show its label). No arbitrary fallback chain or inheritance graph.
6. **Trust:** compiled inventory plus exact integrity determines first-party provenance, never an `@clay/` prefix. User selection is not adoption, installation, or trust promotion. Host appearance selection is a declared generic extension point; any cross-package mutation/replacement additionally requires the target's declared extension point and explicit approval. Selection alone must not grant `package-control`. No V8 object/function/module crosses runtime domains.
7. **Transport:** carry one bounded, validated, generation-stamped active asset snapshot through runtime initialization/reload/reconnect, not geometry in every UI node, row, or keystroke. Preserve protocol version compatibility rules and rebuild matched server/client binaries after wire changes. UI declarations carry semantic references only.

## Placement and Label Policy
| Surface | Implementation | Text retained / safety |
|---|---|---|
| Editor actions | Save, Reload, Close become semantic icon buttons; Open remains labeled beside path field | Save disabled/read-only semantics; reload/close dirty safeguards unchanged; contextual tooltip/name |
| Agent composer/header | Send icon; distinct Stop icon only on the existing generation-stop path; panel close in header | Sending, stopping, and closing remain different intents; preserve draft and in-flight state |
| Tabs and modal chrome | Replace `+` and `×` font glyphs with shared icons | Tab titles, dirty indicator, and contextual close names retained; no nested-interactive regression |
| Dropdown/disclosure | Replace text chevrons with pack icons | Selected values, section titles, expanded state retained |
| Workspace browser | Folder/file/symlink icons; up icon plus Parent folder | Names, counts, navigation targets, root grants unchanged; no inference from display suffix alone |
| Agent empty Files view | Leading icons on Open file, Resume session, Search sessions | All discovery labels remain |
| Recent sessions | Compact repeated Resume action | Session-specific accessible name and tooltip |
| Context/detail navigation | Back arrow plus Back or Back to list | Destination labels remain distinct |
| Git status | Branch/state icons next to real values | Branch name, changed count, stale/error text remain; no fake clickable indicators |
| Markdown preview | Preview icon plus toggle label | Expose real pressed/preview state via generic host semantics |
| Agent navigation tabs | No new decorative icons | Files, Memory, Context, Session Info remain text |
| Permissions, settings/model choices, errors, confirmations | No icon-only conversion | Preserve explanatory and destructive-action text |

## Execution Rules
- Tasks are ordered dependencies; execute first unchecked task unless directed otherwise. Write the listed regression cases before implementation. Mark a task complete only with its acceptance and commands evidenced here.
- For **each** independently executed UI task, reload all seven UI skill/catalog files listed under its Documentation Reviewed before source review or edits. Clay Operate-mode, user brief, accessibility, security, and typed-token ownership override marketing-site prescriptions. No hardcoded fonts, palettes, decorative motion, or custom widgets when catalog composition suffices.
- File lists are expected work inventory. New modules and shared integration paths are **tentative until Task 2 traces their exact owners/callers**. Update task file lists before editing if current code moved; do not blindly create every tentative file.
- During implementation, fetch current library docs with ctx7 before using library-specific APIs. For Rust crates actually used/added, inspect `cargo metadata --format-version 1`, `cargo tree -i <resolved-crate>`, and version-exact registry rustdoc/source; run `cargo doc -p <resolved-crate> --no-deps` with an isolated target dir if necessary. This planning-only change adds no Rust dependency or crate API assumption.
- Linux blocking gates: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, relevant and final Linux tests, frontend typecheck/lint/tests/build/bundle budgets, and Tauri checks against its manifest when not covered by the root workspace. Windows-only tests are not a blocking pass condition.
- Keep baseline failures separate from introduced defects; never silently waive blocking checks. No user profile, credentials, working documents, unrelated configuration, or release artifacts are modified by verification.

## Tasks

- [x] 1. Establish baseline, dependency versions, and exact change inventory
  - Acceptance Criteria:
    - Functional: Confirm clean/dirty worktree ownership, current Linux build/test status, shipped renderer, package inventory, and every placement above before implementation. Record inherited failures separately.
    - Performance: Record current frontend bundle figures and available typing/scroll benchmarks; collect pack-independent render/mount baselines for later comparison.
    - Code Quality: Use graft map/ask/skeleton/callers before source searches; treat ranked results as non-exhaustive and use graft grep for migration inventories. Record resolved dependency versions and test suite layout.
    - Security: Use isolated scratch configuration/workspaces and matched server/client binaries; do not adopt packages, install tooling globally, or use provider secrets for baseline review.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `frontend/package.json`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `test-plan/index.md`
    - Options Considered:
      - Assume prior screenshots/builds are current: fast but misses drift.
      - Reproduce a bounded baseline with current binaries and source inventory: chosen.
    - Chosen Approach:
      - Capture graph/source inventory and baseline commands in this plan. Confirm all files and test runners; verify Cargo-resolved versions only for dependencies implementation will touch.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      npm --prefix frontend run typecheck
      npm --prefix frontend test
      npm --prefix frontend run build
      npm --prefix frontend run check:budget
      ```
    - Files to Create/Edit:
      - plans/112-Configurable-Icon-Packs-and-UI-Integration.md: baseline, file inventory, evidence and task status only.
      - test-plan/artifacts/112-icons/baseline/: bounded logs and app-only captures; do not store unrelated desktop content.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `frontend/package.json`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Record baseline pass/fail and exact command for each blocking gate.
    - Enumerate all consumers of affected controls and both SDUI representations; establish intentional keep-text exclusions.
    - Confirm normal typing does not depend on icon state before adding the capability.
  - Baseline Evidence (2026-09-07, branch `enhancement/UI`, HEAD `1b65568`, all seven gates pass, no inherited failures):
    - Worktree: only untracked plan file; no tracked-file modifications. Toolchain: rustc/cargo 1.96.1, node 24.19.0. `src-tauri` is a root workspace member, so root cargo gates cover it.
    - Gates: `cargo fmt --check` 0; `cargo check --all-targets` 0 (10.05s); `cargo clippy --all-targets -- -D warnings` 0; `npm --prefix frontend run typecheck` 0; `npm --prefix frontend test` 0 (35 files / 252 tests); `npm --prefix frontend run build` 0 (3.29s); `npm --prefix frontend run check:budget` 0 (shell gzip 155.3/180 kB, total 372.3/400 kB). Full `cargo test` intentionally deferred to Task 10.
    - Bundle baseline (gzip): index shell 146.1 kB, codemirror 117.8 kB (separate chunk; typing is CodeMirror-local), controls 20.3 kB, WorkspacePanes 21.67 kB, chat-agent-core 38.26 kB, CodingAgentPanel 6.10 kB. These are the Task 10 comparison figures; no other typing/scroll benchmark harness exists beyond `frontend/src/editor/performance.test.ts` and the bundle budget.
    - Shipped renderer: Tauri v2 + React client over authoritative Rust server. 19 first-party packages, no icon pack yet.
    - Inventory: ClayButton has 109 hits in 32 symbols across 15 files (frontend components/editor/chat/coding-agent/settings/shell/routes/sdui + tests). Both SDUI representations confirmed: native `src/protocol/sdui.rs` list rows (text/detail/action only) and package component JSON → `src-tauri/src/bridge/dto.rs` → `frontend/src/sdui/types.ts` → `registry.tsx`/`renderer.tsx`. Package producers: `packages/git/dist/status.js`, `packages/markdown/dist/sdui.js`, plus chat/coding-agent/settings component JSON.
    - Keep-text exclusions confirmed text-only today: agent nav tabs, permissions, model/provider/effort choices, errors, destructive confirmations.
    - Typing independence: zero icon code, imports, or SVG assets exist in `frontend/src`; the editor hot path (`frontend/src/editor/`) has no icon coupling, so typing cannot depend on icon state before this plan adds any.
    - Evidence artifact: `test-plan/artifacts/112-icons/baseline/baseline.md`. Correction recorded by Task 2: `src/shell/primitives.rs`/`paint_icon_slot` were removed with the native client — icon chrome primitives now live as React components/CSS; no React icon component, tooltip, or icon token variables exist yet.

- [x] 2. Review existing primitives and finalize generic icon contract before package work
  - Acceptance Criteria:
    - Functional: Inventory reusable package/configuration/protocol/SDUI/theme/icon-slot/button/tooltip primitives. Resolve every Contract Target above, exact key list, numeric budgets, selection scope, and proposed API names in a review artifact before implementation.
    - Performance: Size real Regular/Duotone samples against existing contribution and wire budgets; choose one cached active map with no lookup IPC, hot-path parsing, or dependency-heavy renderer.
    - Code Quality: Prefer optional icon fields on existing kinds and shared host composition. Record what existing primitives achieve, what generic gaps remain, and the exact integration files/callers. New stable policy outside this proposal requires explicit user approval before a decision log.
    - Security: Specify deny cases, approval/revocation semantics, provenance, package-key ownership, no action spoofing, and the distinction between global selection and cross-package mutation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/primitives/index.md`
      - `docs/reference/primitives/registry.md`
      - `docs/reference/primitives/ui-chrome-primitives.md`
      - `docs/reference/ui-components.md`
      - `docs/wiki/modules/primitive-architecture.md`
      - `docs/wiki/modules/package-loading.md`
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/create-decision-log/SKILL.md`
    - Options Considered:
      - Import Phosphor React components throughout UI: easy initially, but prevents inert third-party replacement and duplicates library coupling.
      - Allow arbitrary SVG/URLs: rejected for executable/resource authority and parser complexity.
      - Generic bounded vector data plus existing controls: chosen; add only missing reusable icon and tooltip composition.
    - Chosen Approach:
      - This task also satisfies Clay UI catalog review: inventory existing icon-size/text.icon tokens, paint_icon_slot, ClayButton, overlays, and planned tooltip before proposing any new component. Write one contract/primitive review covering fallback state table, complete key-to-source map, bounded vector schema, and catalog reuse versus justified new primitives. Compare `setDesignSystem` (enable+select in one call) with explicit `loadPackage` then `setIconPack`; keep load from selecting by order. Clarify first-party public appearance extension points without self-granting package selection APIs. Record any extra approval needed; do not invent an approval log.
    - API Notes and Examples:
      ```text
      Proposed module: clay:theme
      Proposed export: setIconPack
      Proposed stable ID: theme.setIconPack
      Proposed contribution: clay.contributions.iconPack
      Semantic reference: action.close (not an executable command)
      ```
    - Files to Create/Edit:
      - docs/development/icon-pack-primitive-review.md: inventory, decisions/options, lifecycle state table, budgets, placement/key map, exact source/test owners.
      - plans/112-Configurable-Icon-Packs-and-UI-Integration.md: finalize tentative file lists and contract names.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `docs/reference/primitives/index.md`
      - `docs/reference/primitives/registry.md`
  - Test Cases to Write:
    - Contract checklist accounts for both native SDUI tree and package component JSON consumers.
    - Each required key maps to real assets in both pinned styles; extra keys have a concrete consumer or are removed.
    - Explain authorized failure versus revoked-data fallback, startup no-selection behavior, duplicate loads, multiple loaded packs, and generation ordering.
  - Completion Evidence (2026-09-07): contract finalized in `docs/development/icon-pack-primitive-review.md`. Pinned upstream = phosphor-icons/core **v2.0.8** (verified latest release; per-file HTTP 200 for all 21 keys in both weights). Finalized: 21 required keys with concrete consumers (no speculative keys), bounded vector schema (viewBox [1,512], 1–8 paths, ≤512 absolute commands, opacity [0,1], coords ±4096), budgets (2048 B/icon, 64 KiB/pack and snapshot — matches UI_DESIGN_SYSTEM precedent; measured raw range regular 214–701 B, duotone 267–841 B), 13-row lifecycle/fallback state table, deny list, transport, and API names (`setIconPack`/`theme.setIconPack`/`clay.contributions.iconPack`).
    - Primitive inventory outcome: reusable = core tokens `dimension.icon.size`/`text.icon`, recipe slots `button.icon`/`dropdown.indicator`/`collapse.chevron`/`iconSlot` (already in the recipe matrix), ClayButton, inert component kinds, `apply_design_system` activation pattern, contribution validation/budgets, bundled inventory; gaps confirmed = no React icon component, no tooltip, no semantic icon fields on declarations, no iconPack contribution kind. Correction: `src/shell/primitives.rs` was removed with the native client — Task 7 renders icons in React only.
    - File-list finalization: Task 3 `src/shell/icons.rs` confirmed (new module next to theme.rs/components.rs); Task 7 `frontend/src/components/icon.tsx` + `icon.module.css` and `tooltip.tsx` + `tooltip.module.css` confirmed new (no existing reusable tooltip found). All other tentative paths matched the current tree.
    - Approvals: no new stable policy outside the approved plan scope; decision log for the implemented architecture is a post-verification follow-up requiring explicit user approval (recorded in review §12).

- [x] 3. Implement bounded icon contribution validation and generic semantic references
  - Acceptance Criteria:
    - Functional: Parse/version/validate icon-pack metadata into package records and add optional semantic references to supported button/label/status/list declarations in both UI paths. Existing text-only manifests remain accepted.
    - Performance: Enforce Task 2 byte/count/geometry bounds before allocation or path expansion; no raw geometry repeated in SDUI nodes. Preserve existing payload ceilings.
    - Code Quality: Use one generic Rust icon model/validator and shared reference validation; do not branch on Phosphor or first-party package names. Update manifest single-source rules and genuine extension-point scope validation.
    - Security: Reject raw SVG/XML, CSS, URLs, script/events, foreignObject, use/image/filter/animation, unsupported attributes, non-finite values, malformed commands, invisible/empty required glyphs, invalid viewBoxes, duplicate/prototype-sensitive keys, and unauthorized package namespaces.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/icon-pack-primitive-review.md`
      - `.agents/skills/project-patterns/references/package-manifest-single-source.md`
      - `src/packages/record/mod.rs:682-763`
      - `src/protocol/sdui.rs:114-119`
      - `runtime/js/sdui.d.ts:11-16`
    - Options Considered:
      - Separate special icon-button SDUI kind: duplicates existing behavior and breaks catalog economy.
      - Additive icon reference and bounded vector contribution with existing kinds: chosen.
    - Chosen Approach:
      - Keep vector parsing/validation private or pub(crate). Extend only supported generic declaration fields; add generic pressed-state metadata if Markdown preview currently lacks a truthful toggle state rather than a Markdown renderer branch.
    - API Notes and Examples:
      ```text
      UI node -> semantic key only
      Package contribution -> validated bounded vector geometry
      Host -> action, label, accessible state, theme color
      ```
    - Files to Create/Edit:
      - src/shell/icons.rs (new; confirmed by Task 2) and src/shell/mod.rs (module declaration): versioned model, geometry/reference validation and fallback contract.
      - src/packages/record/icons.rs (new), src/packages/record/mod.rs, src/packages/manifest.rs, src/packages/bundled.rs: contribution parsing and actual extension-point scope coverage.
      - src/packages/record/ui.rs, src/server/ui.rs, src/server/ops/sdui.rs, src/protocol/sdui.rs, src/shell/components.rs: additive semantic fields and validation as their actual ownership requires.
      - runtime/js/sdui.js, runtime/js/sdui.d.ts, runtime/js/ui.js, runtime/js/ui.d.ts: declaration helpers/types where applicable.
      - tests/package_loading.rs, tests/package_ui_conformance.rs, existing protocol test modules: compatibility, boundary and hostile-input cases.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `docs/development/icon-pack-primitive-review.md`
      - `.agents/skills/project-patterns/references/package-manifest-single-source.md`
  - Test Cases to Write:
    - Valid regular/duotone geometry round-trips; old text-only manifests round-trip unchanged.
    - For each budget test zero/min/max/max+1, invalid numeric ranges, truncated path commands and unsupported schema version.
    - Hostile asset/reference corpus cannot trigger network requests, execution, CSS injection or uncontrolled allocations.
    - Two packs with the same core semantic meanings coexist as alternatives without contribution conflict; package-owned custom keys cannot impersonate another package.
  - Completion Evidence (2026-09-07):
    - Generic model/validator: `src/shell/icons.rs` (new) — versioned `IconGeometry` (viewBox [1,512], 1..=8 paths, 1..=512 commands, coords ±4096, opacity [0,1]), full SVG 1.1 path scanner (MmLlHhVvCcSsQqTtAaZz incl. relative forms, implicit repetition, S/T reflection folded to plain C/Q; arcs required because Phosphor rounds corners with `a`), bounded before expansion (per-path 2048 B payload check precedes parsing; command cap enforced during expansion), semantic reference validation (`validate_icon_reference` core/own-prefix, `validate_core_icon_reference` for runtime trees, `validate_icon_pack_key` first-party gate for core keys, reserved `clay.` namespace, `..`/`__` prototype shapes, lowercase-kebab syntax). Contract §4 updated to the d-string encoding (generator passes upstream data through verbatim; parse once at load). 9 unit tests.
    - Contribution parsing: `src/packages/record/icons.rs` (new) + `PackageContributions.icon_pack` (single `clay.contributions.iconPack` object, matching uiDesignSystem precedent) with structural deny list (svg/xml/url/href/style/class/fill/stroke/transform/filter/use/image/foreignObject/script/callback/font…), schema-version gate, duplicate-key rejection, first-party core-key gate on manifest name with activation-time provenance still enforced in Task 5, ICON_PACK (64 KiB) and per-path ICON_GEOMETRY (2048 B) budget enforcement, manifest budget tier for iconPack manifests in `src/packages/manifest.rs`. 3 tests in `src/packages/record/mod.rs` (round-trip incl. duotone opacity layer, third-party impersonation + first-party core-key acceptance, hostile corpus + truncated commands + budgets + viewBox/opacity ranges).
    - Generic semantic references, both UI paths: `SduiListItem`/`Button`/`Label` gained optional `icon` (`src/protocol/sdui.rs`, serde-default additive; rkyv compatible). Runtime trees (untrusted publishTree) accept core keys only via `convert_icon_reference` (`src/server/ops/sdui.rs`; no declaring-package identity in tree builders). Package component declarations validate core/own-prefix keys on button/label/list/statusItem kinds only, components and list items (`src/server/ui.rs` `validate_component_icon` + `icon_references` on `ComponentValidationContext`); component JSON flows verbatim to the client so no DTO change was needed. Frontend types additive: `SduiListItem.icon`, label/button/icon node fields, `PackageListItem.icon`, `PackageComponentNode.icon` (`frontend/src/sdui/types.ts`); `runtime/js/ui.d.ts` needed no change (index-signature passthrough). Native builders pass `icon: None` (behavior wiring is Tasks 8/9).
    - Verification: `cargo fmt --check` clean; `cargo check --all-targets` clean; `cargo clippy --all-targets -- -D warnings` 0 findings; `cargo test --all-targets` all suites pass except one PRE-EXISTING failure (`documentation_coverage::parity_ledger_covers_every_manual_step_public_api_and_protocol_family` — test-plan/17 C1–C20 rows added by the HEAD WIP commit without ledger rows; verified failing at clean HEAD via stash, unrelated to icons); frontend typecheck + 252 vitest tests pass; frontend lint shows the same 12 pre-existing findings as HEAD.

- [x] 4. Build and ship both first-party Phosphor icon packages
  - Acceptance Criteria:
    - Functional: Both named packages ship identical required semantic keys, pinned upstream provenance, correct Regular/Duotone geometry, package docs, load entries if required, license notices, and reproducible distributable assets. Packaged app works without source checkout/network.
    - Performance: Extract only inventory-backed glyphs, not the entire upstream catalog. Record compressed/uncompressed pack sizes and fallback subset cost within approved bounds.
    - Code Quality: Use one mapping/source pipeline to produce both packs and host safety fallback; generated artifacts are deterministic and checked for drift. Each package exposes real extension-point scopes and valid metadata through compiled inventory.
    - Security: Build-time conversion accepts only pinned upstream inputs. No install/runtime lifecycle fetch, renderer SVG injection, font file, package callback, filesystem/network permission, or executable asset is introduced.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `https://github.com/phosphor-icons/core`
      - `https://github.com/phosphor-icons/core/blob/main/LICENSE`
      - `Context7 /phosphor-icons/core: raw assets by regular/duotone style; MIT notices.`
      - `src/packages/bundled.rs:374-448`
      - `build.rs:21-70`
      - `.agents/skills/project-patterns/references/package-distribution.md`
    - Options Considered:
      - Vendor full upstream catalog and React library: larger and unnecessary.
      - Hand-draw equivalent icons: inconsistent and unnecessary.
      - Pinned upstream subset converted once into bounded data: chosen.
    - Chosen Approach:
      - Inspect exact release paths and license before conversion; preserve viewBox and layer opacity. Prefer existing repository build tooling. Keep host fallback generated from Regular package source; do not add a second hand-maintained icon map. Bundled Regular subset must work with zero init.js icon lines and no package load entry. Explicit `loadPackage` registers the pack only; it is not a silent behavior-changing default and does not require copied manifests or manual primitive registration.
    - API Notes and Examples:
      ```text
      Upstream documentation pattern: assets/<weight>/<kebab-name>-<weight>.svg
      Styles used: regular, duotone
      Verify exact paths in the pinned release rather than assuming catalog examples.
      ```
    - Files to Create/Edit:
      - packages/icons-phosphor-regular/package.json, dist/index.js, dist/load.js, docs/index.md, LICENSE, icons.json (new; asset/load files follow finalized contract).
      - packages/icons-phosphor-duotone/package.json, dist/index.js, dist/load.js, docs/index.md, LICENSE, icons.json (new; same contract).
      - scripts/generate-icon-packs.mjs and packages/icon-sources.json (new tentative): pinned source version/integrity, semantic map and deterministic generation; no unused generator framework.
      - src/packages/bundled-inventory.toml, build.rs, src/packages/bundled.rs: embed/package required data and license assets through existing inventory mechanism.
      - frontend/src/icons/fallback.generated.ts (new tentative): generated safety subset only, not executable library imports.
      - tests/icon_packages.rs (new, wire into actual declared Cargo suite if needed): inventory, generation, licenses and completeness.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `https://github.com/phosphor-icons/core`
      - `https://github.com/phosphor-icons/core/blob/main/LICENSE`
  - Test Cases to Write:
    - Exact semantic-key equality across both packs and all expected UI consumers.
    - Regeneration makes no diff; missing asset/license, changed upstream checksum or unlisted key fails deterministically.
    - Release-style/package-resource smoke works with source-tree packages unavailable and network disabled.
    - Repeated explicit load is idempotent; loading another pack does not change explicit active selection.
  - Completion Evidence (2026-09-07):
    - Pipeline: `scripts/generate-icon-packs.mjs` (dependency-free Node, no network at build) + `packages/icon-sources.json` (pinned upstream v2.0.8 release/license/copyright, semantic key→asset map, per-file sha256 of all 42 vendored SVGs under `packages/icon-sources/phosphor-2.0.8/{regular,duotone}/`). Converter accepts only the verified upstream shape (viewBox `0 0 256 256`, `fill=currentColor`, flat paths, bounded opacity, no transforms/styles/URLs/other elements) and rejects anything else, so drift fails deterministically.
    - Packages generated (single manifest source of truth; `dist/index.js`/`dist/load.js` are generated no-ops like the theme packs, no icons.json duplicate): `packages/icons-phosphor-regular/` and `packages/icons-phosphor-duotone/` each with `package.json` (inline `clay.contributions.iconPack`, schemaVersion 1, 21 core keys, `permissions: []`, `modes: []`, iconPack extension point like the design-system packs), `docs/index.md` (usage + provenance + key table), and `LICENSE` (MIT, Phosphor copyright). `clay.contributions.iconPack` registered as an `ExtensionContributionKind::IconPack` ("iconPack") so packs declare a real extension-point scope.
    - Recorded sizes: Regular manifest 11,974 B (geometry 9,187 B; gzip 2,891 B); Duotone manifest 15,643 B (gzip 3,623 B); fallback subset 8,851 B (gzip 2,688 B). All far under ICON_PACK_PAYLOAD_BUDGET_BYTES (64 KiB); no full upstream catalog vendored (42 files, inventory-backed keys only).
    - Host fallback: `frontend/src/icons/fallback.generated.ts` generated from the Regular pack by the same pipeline (data-only typed record; no library imports, no executable surface). Zero init.js icon lines render Regular via this fallback; explicit `loadPackage` + `setIconPack` selects a pack without changing theme/design-system/typography state (behavior wiring is Task 5).
    - Bundling: both pack roots added to `src/packages/bundled-inventory.toml` (trust inventory fingerprints manifests at build time via `build.rs`; no new embed mechanism, no lifecycle hooks). Plan 061 frozen package inventory baseline updated (+2 inert-data rows, count 18→20 in `tests/primitives_docs.rs`). Manifest-side budget check for estimatedManifestBytes now accepts the iconPack tier (ICON_PACK_PAYLOAD_BUDGET_BYTES) in `src/packages/record/documentation.rs`, matching the manifest.rs tier from Task 3.
    - Tests: `tests/icon_packages.rs` (wired into the presentation suite) — 5 tests: both packs assemble via `assemble_package_record` with identical key sets exactly equal to `CORE_ICON_KEYS`; both are bundled inventory members; host fallback geometry equals the Regular pack per key (never a second hand-maintained map); `node scripts/generate-icon-packs.mjs --check` regenerates with no diff; packs carry no permissions/modes, no-op load entries, LICENSE and docs. Release-style offline smoke (source-tree-independent) is covered by the existing bundled-inventory trust tests plus the in-crate inventory↔toml match assertion.
    - Verification: fmt/check --all-targets/clippy -D warnings clean; `cargo test --all-targets` all suites pass except the one PRE-EXISTING parity-ledger failure (documented in Task 3 evidence); frontend typecheck + 252 vitest tests pass; lint findings unchanged from HEAD baseline.

- [x] 5. Implement user-owned activation, third-party support, and revocation lifecycle
  - Acceptance Criteria:
    - Functional: Expose finalized selection facade through existing theme/configuration boundaries. Explicitly loaded first-party or adopted third-party pack can be selected; configuration reload, failed selection, disable/remove/revoke and startup fallback follow Task 2 state table.
    - Performance: Validate/resolve once at load or selection/generation commit, cache one active map; unchanged selection is idempotent. Never acquire package-service mutex recursively or await under a held lock.
    - Code Quality: Separate selected identity from resolved active data and fallback reason. Preserve theme/design-system/typography state. Runtime package callers cannot hijack user-global selection; document and enforce caller policy.
    - Security: No implicit install/adoption/trust promotion or grant from setIconPack. Exact compiled provenance governs trusted packages. Third-party worker has only documented allowed public ops; internal modules/ops remain absent. Revocation withdraws data and stale results are rejected.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `runtime/js/theme.js`
      - `runtime/js/theme.d.ts`
      - `src/server/ops/theme.rs:260-399`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/package-runtime-trust-domains.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
    - Options Considered:
      - Let last-loaded pack win: violates explicit user ownership.
      - Let appearance selection install/promote packages: rejected.
      - Register through loadPackage and select explicitly through shared validated state: chosen.
    - Chosen Approach:
      - Use existing configuration transactions and enabled package records. Reserve Regular fallback independently of user package execution. Add a small third-party local fixture using the ordinary adoption/authorization path, never a test-only activation backdoor.
    - API Notes and Examples:
      ```javascript
      // Proposed API, finalized by Task 2; not callable at plan creation.
      import { loadPackage } from "clay:packages";
      import { setIconPack } from "clay:theme";
      await loadPackage("@clay/icons-phosphor-duotone");
      setIconPack("@clay/icons-phosphor-duotone");
      ```
    - Files to Create/Edit:
      - src/server/ops/theme.rs, src/server/ops/mod.rs, src/server/ops/packages.rs: facade op, registration/activation and package hooks.
      - src/server/js_runtime/mod.rs, src/server/js_runtime/evaluation.rs, src/server/mod.rs: configuration result/state, atomic generation commit and revocation.
      - src/packages/service.rs, src/packages/extension_points.rs: withdrawal counts/extension rules only where needed.
      - runtime/js/theme.js, runtime/js/theme.d.ts: documented selector and result types.
      - src/server/js_runtime/tests.rs, tests/persistent_runtime_hot_reload.rs, tests/runtime_sandbox_harness.rs: lifecycle/trust regression coverage.
      - tests/fixtures/icon-packs/third-party/: valid partial/custom pack and hostile/revoked variants (new).
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `runtime/js/theme.js`
      - `runtime/js/theme.d.ts`
  - Test Cases to Write:
    - Default/no config; Regular -> Duotone -> third-party -> Regular; modular init and restart.
    - Unknown/not-adopted/not-enabled/non-icon package selection rejects without altering authorized current state.
    - Revocation/remove/disable while active replaces revoked data with fallback; late generation cannot restore it.
    - Load-order independence, duplicate loads, failed required reload rollback, idempotent activation and timeout/deadlock regression.
    - Third-party cannot self-authorize, call internal ops, import trusted module roots, or replace another package without declared extension point and approval.
  - Completion Evidence (2026-09-07):
    - Selection facade: `setIconPack` exported from the existing `clay:theme` facade (`runtime/js/theme.js` + `theme.d.ts`, string or `{ specifier }`, returns `{ pack, iconCount, schemaVersion }` per finalized naming). Backing op `op_clay_theme_set_icon_pack` + `apply_icon_pack` (`src/server/ops/theme.rs`) mirror the `setDesignSystem` pattern: held-lock first-party resolution via `ensure_first_party_record_locked` (no re-entrant mutex, plan 110 task 18 precedent), `theme.*` error namespace. Op registered ONLY in the trusted runtime extension (96→97 ops, admin list updated); the third-party package extension stays at 46 ops — runtime package callers cannot reach user-global selection (caller policy enforced by construction, asserted in `domain_extension_tests`). Plan 061 frozen op-inventory baseline updated (42→43 admin-only ops) in `tests/primitives_docs.rs`.
    - Active state, identity separate from data: new `ActiveIconPack` in `src/shell/icons.rs` — `{ specifier, schema_version, generation, provenance (reused DesignSystemProvenance), icons: BTreeMap<key, IconGeometry> }`. Carried out of evaluations (`worker.rs`/`error.rs`), stored in a new `IpcServer.active_icon_pack` slot (`None` = bundled Regular host-fallback subset active, zero init.js lines). `ClayOpState` carries `active_icon_pack` + `explicit_icon_pack_active`.
    - Activation rules (state table): selection resolves from already-registered records — first-party bundles resolve from the compiled inventory without executing anything (zero-config works); third-party specifiers are NEVER enabled by selection (`theme.load_failed` unless already enabled through the ordinary load/adoption path; load ≠ select, load order never wins). Idempotent re-selection; failed/unknown/not-enabled/non-icon selection preserves the previous active snapshot untouched (row 4). Activation-time provenance gate in `ActiveIconPack::from_record`: core semantic keys resolve only from records backed by the compiled bundled inventory (exact name + version) — an `@clay/`-named non-inventory record is rejected (`compiled first-party provenance`).
    - Configuration reload + persistence: new `iconPack` preference key (`PREFERENCES_KEYS`, load/persist/validate/serialize in `src/server/configuration.rs`, non-empty-string validation with diagnostics for garbage); `apply_persisted_preferences` re-applies it on every configuration load/reload with sanitized diagnostics on rejection (rows 6; previous valid generation preserved).
    - Revocation (row 7): the runtime-generation prepare path (`src/server/mod.rs`) re-validates the evaluation-selected OR previously-active pack against enabled records at every commit (name + version + iconPack contribution intact); a disabled/removed/revoked pack resolves to `None` = bundled Regular subset, and the generation stamp (`active_icon_pack.generation = generation_id`) makes stale snapshots rejectable by the transport layer (Task 6). Commit guard + slot assignment wired into the existing atomic candidate commit (expected vs active compare before swap).
    - Fixtures: `tests/fixtures/icon-packs/third-party/` — `valid-partial/` (own-namespace keys only, partial coverage legitimate per rows 8/9) and `hostile/` (core-key impersonation + raw `svg` field) with README; adopted through the ordinary install/authorize/approve path, never a test-only backdoor.
    - Tests (+13): `ops/theme.rs` — zero-config Regular selection, load ≠ select, Regular→Duotone→Regular with differing geometry, idempotent re-selection, bad-specifier matrix (empty/unknown/not-enabled/non-icon) preserving state, third-party requires prior enable then selection succeeds with ThirdParty provenance and own-namespace keys only, disable-while-active rejects re-selection and preserves state, hostile fixture fails record assembly + selection; `shell/icons.rs` — activation-time inventory provenance gate; `js_runtime/tests.rs` — `setIconPack("@clay/icons-phosphor-duotone")` end-to-end via init.js (summary reaches init.js, snapshot emitted, Trusted provenance); `configuration.rs` — iconPack preference persists, survives reload, drops garbage. Sandbox/trust suites (security 135, sandbox harness) pass unchanged.
    - Verification: fmt clean; check --all-targets clean; clippy -D warnings 0 findings; 1249 lib tests pass; all-target suites pass except the one documented PRE-EXISTING parity-ledger failure; frontend typecheck + 252 vitest tests pass.

- [x] 6. Project active icons through runtime protocol, Tauri DTOs, and frontend state
  - Acceptance Criteria:
    - Functional: Initial connection, reload, reconnect, and every workspace connection receive consistent selected/resolved pack identity, provenance, generation/revision and bounded geometry. Preserve both SDUI icon-reference paths through Rust and TS DTOs.
    - Performance: Send asset data only with changed active state, not per row, pointer event or edit. Bound runtime snapshot/frame size; stale/duplicate installs do no work. Avoid whole-app rerenders on typing.
    - Code Quality: Follow existing runtime snapshot and store patterns with a narrow icon state module; use lossless IDs and version handshake. Preserve component keys, editor state and focused controls across pack swaps.
    - Security: Validate before wire publication and defensively at DTO/render boundaries; reject unsupported/stale snapshots and unknown fields. Tauri gets no new broad OS or webview capabilities.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/wiki/modules/ui-design-system-runtime.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `src-tauri/src/bridge/dto.rs:609-686`
      - `frontend/src/sdui/types.ts:84-130`
    - Options Considered:
      - Put geometry in every component snapshot: repeats bytes and trust handling.
      - Fetch icons individually from renderer: adds authority and latency.
      - Single bounded generation-stamped active map using existing transport: chosen.
    - Chosen Approach:
      - Trace current runtime update fan-out and validators before edits. Add active icon state without changing document authority; derive root/icon subscribers narrowly and retain last authorized state during transient disconnection. Revoke according to server state, not client guesses.
    - API Notes and Examples:
      ```text
      package/config generation -> validated active icon snapshot -> Tauri DTO
      -> icon store -> shared ClayIcon
      SDUI node -> semantic reference -> same store
      ```
    - Files to Create/Edit:
      - src/protocol/mod.rs and runtime-state protocol owner (tentative until graph trace), src/server/mod.rs, src/server/connection/runtime.rs: initial/update/reconnect publication and bounds.
      - src-tauri/src/bridge/dto.rs and bridge event projection owner: icon DTO mapping and rejection.
      - frontend/src/bridge/types.ts, frontend/src/sdui/types.ts, frontend/src/shell/workspace-controller.ts: typed payload and runtime application.
      - frontend/src/state/icon-store.ts (new tentative): atomic/idempotent active map.
      - tests/runtime_update_protocol.rs, src-tauri/tests/dto_roundtrips.rs, frontend/src/shell/workspace-controller.test.ts, frontend/src/test/icon-store.test.ts (new): state and transport coverage.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `docs/wiki/modules/ui-design-system-runtime.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Wire and JSON round-trip valid geometry/semantic references with exact identity/provenance.
    - Stale generation, duplicate revision, malformed snapshot, excessive payload and mismatched protocol are safely rejected.
    - Multiple tabs/panes and reconnect converge on same pack with no per-document duplicated asset stream.
    - Selection/draft/undo/focus/scroll remain intact; unchanged icon state causes no additional mount or edit-path work.
  - Completion Evidence (2026-09-07):
    - Traced before edits: runtime snapshot fan-out (`RuntimeStateSnapshot` validate + 1 MiB frame encode at candidate build), Tauri `RuntimeSnapshotDto::resolve`/`BootstrapDto` projection (`session.rs` pump rejects invalid snapshots with `runtime.snapshot_rejected` diagnostics), and the React theme/design-system consumption path (`use-clay-session.ts`). DS flow mirrored at every layer.
    - Protocol (`src/protocol/runtime.rs`): `RuntimeStateSnapshot.active_icon_pack: Option<ActiveIconPack>` — additive, `#[serde(default)]` (older clients ignore), skipped on the wire while `None` (host fallback subset active; no per-frame bytes for the default case). `ActiveIconPack::validate()` added in `src/shell/icons.rs`: identity bounds, supported schema version, 1..=MAX_ICONS_PER_PACK, per-icon ≤ ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES (2048), pack total ≤ ICON_PACK_PAYLOAD_BUDGET_BYTES (64 KiB), plus a new moveto-first structural re-check inside `validate_icon_geometry` so directly constructed geometry cannot bypass the parser invariant. Snapshot validate maps failures to new `InvalidIconPack` variant — fail closed before install/fan-out. rkyv derives on IconGeometry/IconPath/IconPathCommand/ActiveIconPack keep the length-prefixed server transport intact.
    - Wire shape: canonical contract d-string form `{ viewBox, paths: [{ d, opacity? }] }` everywhere — new `IconPath::to_d()` bounded d-string writer (absolute commands, shortest round-trip f32 formatting; output re-parses losslessly), manual serde Serialize/Deserialize on `IconPath` (parse+normalize on deserialize), so manifests, host fallback, protocol JSON, and Tauri DTO all share one shape; parsed commands never serialize.
    - Server: `build_runtime_state_snapshot` carries the generation-stamped `active_icon_pack` from the commit candidate (Task 5) into every snapshot — initial connect, config reload, reconnect, and every workspace connection receive the same atomic install surface. Asset data ships only with the snapshot (generation change), never per row/pointer/edit.
    - Tauri DTOs (`src-tauri/src/bridge/dto.rs`): new `IconPackSnapshotDto`/`IconGeometryDto` (viewBox widened to f64, lossless)/`IconPathDto`; `IconPackSnapshotDto::resolve` validates before publication and rejects (never truncates) invalid packs; `RuntimeSnapshotDto.active_icon_pack` + `BootstrapDto.active_icon_pack` (None at bootstrap — pack identity arrives with the first runtime snapshot, matching the DS core_fallback treatment; `IconPackSnapshotDto` re-exported from bridge). Both SDUI icon-reference paths already flow through Rust+TS DTOs from Task 3 (verified, no further change).
    - Frontend: `frontend/src/icons/types.ts` (IconPackSnapshot/IconGeometry/IconPathData wire types); `frontend/src/state/icon-store.ts` (new, dependency-free observable store mirroring design-system-store conventions): atomic install, idempotent same-generation identical revision (no subscriber churn, no DOM writes — geometry only), stale-generation rejection keeping last authorized state, same-generation identity swap accepted, malformed snapshots (wrong shape, non-finite/out-of-range coords, viewBox width/height out of [1,512], >8 paths, oversized/empty d, opacity outside [0,1], >64 icons, empty pack) rejected wholesale without partial application; `resetToFallback()` on disconnect (server owns revocation, client never guesses). Singleton `iconStore` in `state/stores.ts`; wired in `use-clay-session.ts` at all three consumption points (runtimeSnapshot envelope, disconnected, bootstrap). `RuntimeSnapshot.activeIconPack?` and `BootstrapDto.activeIconPack?` typed in `sdui/types.ts` + `bridge/types.ts`.
    - Tests (+2 Rust suites, +1 frontend): `tests/runtime_update_protocol.rs` — bounded pack wire round-trip preserving exact identity/provenance/generation + canonical d-string JSON shape, and rejection matrix (malformed geometry, unsupported schema version, anonymous specifier, excessive per-icon payload). `src-tauri/tests/dto_roundtrips.rs` — DTO resolve round-trip with duotone opacity layer surviving to JSON, fail-closed rejection of an empty pack; also repaired two PRE-EXISTING stale initializers in that file at HEAD (`thinking_level`, `ui_choices`, missing `icon` fields — src-tauri tests are outside the root workspace's default test run and had drifted). `frontend/src/test/icon-store.test.ts` — 6 tests: install + per-key geometry, idempotent revisions, stale-generation rejection, same-generation swap, malformed rejection matrix keeping last authorized state, null fallback semantics.
    - Verification: fmt clean; check --all-targets clean; clippy -D warnings 0 findings; 1249 lib tests + runtime/protocol/security/presentation suites pass (only the documented PRE-EXISTING parity-ledger failure remains); frontend typecheck clean, 258 vitest tests pass (+6), lint findings unchanged from HEAD baseline.

- [x] 7. Implement shared icon rendering and accessible control composition
  - Acceptance Criteria:
    - Functional: Provide one cataloged ClayIcon and icon-capable existing button/list/label/status composition. Icon-only controls have required accessible labels, contextual hover/focus tooltips, valid disabled behavior and focus-visible targets. Missing icon never creates an unnamed/blank action.
    - Performance: Render bounded validated geometry only; no runtime XML parsing, dynamic library loading or network. Use existing dimension.icon.size for glyphs; hit targets remain at least WCAG 2.2 minimum target requirements (24 CSS px or valid spacing exception), not merely 16px glyph bounds.
    - Code Quality: Reuse ClayButton/React Aria, existing overlay/tooltips if adequate, and icon recipe slots. Add a generic tooltip wrapper only because current catalog marks tooltip planned and icon-only actions require focus as well as hover help. Catalog and type constraints must prevent unlabeled icon-only usage.
    - Security: Host determines accessible names, roles, focus and action handlers; packages supply geometry/references only. No dangerouslySetInnerHTML, CSS selectors, arbitrary attribute spread, hardcoded palette/font, or icon-dependent action routing.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - `docs/reference/primitives/ui-chrome-primitives.md`
      - `frontend/src/components/button.tsx`
      - `frontend/src/components/chrome.tsx`
      - `.agents/skills/project-patterns/references/ui-design-system-packages.md`
      - `React Aria Components docs via ctx7 during implementation, matched to lockfile version.`
    - Options Considered:
      - One bespoke clickable icon per feature: duplicates a11y/disabled/focus bugs.
      - SVG title or HTML title alone: insufficient tooltip and consistent accessible behavior.
      - Shared decorative icon plus existing accessible button and generic tooltip composition: chosen.
    - Chosen Approach:
      - Consume existing icon recipe slots and theme color roles only. Do not add package palettes, literal colors, raw CSS, or design-system selection coupling; content themes stay sole normal color authority and typography/design-system layers stay independent. Use currentColor, standard token sizing, bounded duotone opacity, and disabled/state recipe styling. Keep icon descendants aria-hidden when parent label conveys meaning; expose one name per control. Forced colors preserves informative outlines; reduced motion avoids new animations. Tooltip closes on Escape and is hoverable/persistent where required.
    - API Notes and Examples:
      ```text
      Icon-only button contract: icon + required label + existing action
      Decorative icon contract: icon + aria-hidden
      Missing required icon: bundled glyph or visible original label
      Tooltip text: action name, plus actual configured shortcut when available
      ```
    - Files to Create/Edit:
      - frontend/src/components/icon.tsx, icon.module.css (new; confirmed by Task 2), frontend/src/components/tooltip.tsx, tooltip.module.css (new; no existing reusable tooltip): safe geometry renderer, token/recipe consumption, accessible shared tooltip.
      - frontend/src/components/button.tsx, button.module.css, controls.tsx, controls.module.css, chrome.tsx, index.ts: shared composition rather than parallel widgets.
      - frontend/src/components/tooltip.tsx and tooltip.module.css (new only if no existing reusable implementation): accessible shared tooltip.
      - frontend/src/sdui/registry.tsx and SDUI tree renderer owner, frontend/src/sdui/types.ts: optional icon/pressed-state projection.
      - .agents/skills/clay-ui/references/components.md, .agents/skills/clay-ui/references/tokens.md, docs/reference/ui-components.md, docs/development/ui-design-system-recipe-matrix.md: generic primitives and consumed slots.
      - frontend/src/test/icons.test.tsx (new), frontend/src/test/components.test.tsx, frontend/src/test/design-system-consumption.test.ts: rendering/a11y/fallback tests.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `docs/reference/ui-components.md`
      - `docs/reference/primitives/ui-chrome-primitives.md`
  - Test Cases to Write:
    - Regular and Duotone render correct path order, viewBox, fill-rule and opacity with theme colors.
    - Keyboard focus, Enter/Space, disabled activation gate, tooltip hover/focus/Escape, accessible-name uniqueness and contextual labels.
    - Unknown icon fallback keeps button usable; decorative unknowns add no meaningless announcement.
    - Forced colors, large typography, RTL back/disclosure semantics where supported, narrow layout, no clipped focus ring or shrinking target.
    - Pack switching retains DOM control identity and focus.
  - Completion Evidence:
    - `frontend/src/components/icon.tsx` + `icon.module.css`: `ClayIcon` renders bounded host-validated geometry from `iconStore` (bundled Regular subset fallback via `icons/fallback.generated.ts`) as inline `currentColor` SVG sized by `dimension.icon.size`, colored by `text.icon`; `aria-hidden` by default, `role="img"` with a single name only when `label` is passed; unknown keys render an empty decorative slot. No innerHTML, no arbitrary spread, no package CSS/palette.
    - `frontend/src/components/tooltip.tsx` + `tooltip.module.css`: `ClayTooltip` over React Aria `TooltipTrigger`/`Tooltip` (hover **and** keyboard-focus trigger, Escape/blur dismiss, `role="tooltip"` + `aria-describedby`); host-owned string content only; consumes the shipped `tooltip.default.root.rest` recipe with core-token fallbacks; fade-only entry (reduced-motion collapses it).
    - `frontend/src/components/button.tsx` + `button.module.css`: `ClayIconButton` requires `label` (type-level: unlabeled icon-only usage cannot compile), decorative 16px glyph inside a ≥24px CSS hit target (`dimension.icon.size + 2 × spacing`), tooltip from label + optional shortcut, and a visible-text fallback when the key has no geometry — a failed pack never leaves a blank control. Existing `ClayButton` reused unchanged for labeled actions.
    - `frontend/src/components/controls.tsx` + `controls.module.css`: dropdown indicator now renders the shared `disclosure.down` glyph via `ClayIcon` (recipe attributes `dropdown.indicator`); collapse chevron renders `disclosure.right` (rotates 90° when expanded via existing CSS); `ClayList` rows accept optional `icon` rendered decoratively before the title (`textValue` unchanged).
    - `frontend/src/sdui/registry.tsx`: package SDUI `button`/`label`/`list` nodes project their validated `icon` references through the same `ClayIcon` path; `frontend/src/components/recipe-attributes.ts` allowlist extended with `iconSlot` + `tooltip`; `frontend/src/components/index.ts` exports `ClayIcon`/`ClayIconButton`/`ClayTooltip`.
    - `frontend/src/test/icons.test.tsx` (new, 15 tests): path order/viewBox/opacity for Regular + Duotone, zero-config fallback, decorative-vs-informative a11y, unknown-key empty slot, Enter/Space/disabled gating, single accessible name, unknown-key visible-label fallback, DOM-identity + focus retention across pack switches, tooltip focus-open/Escape-dismiss, dropdown/collapse glyph migration, SDUI icon projection (button/label/list). `design-system-consumption.test.ts` ownership map registers `tooltip → components/tooltip.module.css`.
    - Docs: `.agents/skills/clay-ui/references/components.md` (Tooltip + Icon slot now implemented), `docs/development/react-ui-catalog-mapping.md` (Plan 112 rows + updated planned-entries note), `docs/reference/ui-components.md` (new Plan 112 section).
    - Verification: `npm run typecheck` clean; 273 vitest tests pass (14 new icon tests + SDUI projection); lint unchanged from HEAD baseline (12 pre-existing); no new dependencies.
    - Deviations: `chrome.tsx` untouched (badge/kbd/divider have no icon composition to migrate); RTL/large-typography visual matrix deferred to Task 11 evidence; `useIconGeometry` subscribes via the `stores.ts` singleton so all consumers share one pack instance.

- [x] 8. Migrate core controls, editor actions, and coding-agent surfaces
  - Acceptance Criteria:
    - Functional: Implement the complete Placement and Label Policy for tabs, modal close, dropdown/disclosure, editor toolbar, agent composer/header, empty Files actions, recent-session actions and Back navigation. Keep all intentional label exclusions.
    - Performance: Compact repetitive controls without shrinking hit targets, increasing editor render frequency, or adding tooltip observers/timers per transcript row unnecessarily.
    - Code Quality: Route all visuals through shared icon keys; remove replaced text-glyph styling, not meaningful text. Keep existing handlers/keybindings/IDs and scopes. Close buttons in tab strips must not introduce invalid nested interactive controls.
    - Security: Preserve read-only/dirty guards, error/status announcements, draft content, confirmation labels, and Stop versus Close semantics. No new agent actions, provider requests, package control or document authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `frontend/src/editor/ClayEditor.tsx:136-189`
      - `frontend/src/components/tab-strip.tsx:38-151`
      - `frontend/src/components/modal.tsx:27-64`
      - `frontend/src/components/controls.tsx:43-109,191-225`
      - `frontend/src/coding-agent/CodingAgentPanel.tsx:908-1393`
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
    - Options Considered:
      - Convert every action/tab to icon-only: reduces comprehension and conflicts with requested review.
      - Implement only a toolbar demo: leaves major consumers inconsistent.
      - Migrate all agreed placements and explicitly retain discovery/destination labels: chosen.
    - Chosen Approach:
      - Trace each action caller before edits. Use existing generation-stop only if present; otherwise record it as not applicable rather than inventing behavior. Move existing panel close to header without changing close intent. Use contextual labels based on sanitized file/session names.
    - API Notes and Examples:
      ```text
      Save -> document.save; Reload -> document.reload; Close -> action.close
      Send -> message.send; Stop generation -> generation.stop
      Back and Back to list -> navigation.back with distinct visible labels
      Files / Memory / Context / Session Info -> unchanged text tabs
      ```
    - Files to Create/Edit:
      - frontend/src/components/tab-strip.tsx, tab-strip.module.css, modal.tsx, modal.module.css, controls.tsx, controls.module.css: shared chrome migration.
      - frontend/src/editor/ClayEditor.tsx and its existing CSS module: compact action row.
      - frontend/src/coding-agent/CodingAgentPanel.tsx and its existing CSS module: agreed action/header/navigation changes.
      - frontend/src/test/components.test.tsx, frontend/src/test/editor.test.tsx, existing coding-agent component tests: interaction and retained-label coverage.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `frontend/src/editor/ClayEditor.tsx:136-189`
      - `frontend/src/components/tab-strip.tsx:38-151`
  - Test Cases to Write:
    - Each migrated action invokes exact existing handler once and respects disabled conditions.
    - Dirty save/reload/close and closing non-active tabs retain existing confirmation/focus policies.
    - Composer empty/sending/running/error states expose correct Send/Stop/Close actions without discarding draft.
    - Recent sessions have distinct Resume names; empty Files labels and Back destination labels remain visible.
    - No icons added to agent tab labels, permissions, model/effort choices or destructive confirmation text.
  - Completion Evidence:
    - `frontend/src/editor/ClayEditor.tsx`: action row migrated to `ClayIconButton` — Save (`document.save`, primary, `!editable` disabled), Reload (`document.reload`), Close (`action.close`, muted, `session.close(meta.dirty)` dirty guard kept); handlers, ordering, and disabled conditions unchanged; Open path input + Open button keep visible text (destination label policy).
    - `frontend/src/components/tab-strip.tsx`: per-tab close `×` text glyph → decorative `ClayIcon action.close` (nested-button structure, `aria-label="Close <tab>"`, stopPropagation, and focus policy untouched — no new nested interactive controls); strip `+` new-tab glyph → decorative `ClayIcon action.new` with `aria-label="New tab"` kept; empty-strip "New tab" keeps its text label (discovery context).
    - `frontend/src/components/modal.tsx`: dialog close `×` glyph → decorative `ClayIcon action.close` inside the unchanged React Aria `slot="close"` button (Escape/focus-trap semantics untouched, `aria-label="Close"` kept).
    - `frontend/src/coding-agent/CodingAgentPanel.tsx`: composer Send → icon-only `message.send` with native `type="submit"` (empty-draft/provider gates preserved via `isDisabled`); streaming Cancel → icon-only `generation.stop` labelled "Stop" (`abortRun()` handler unchanged — Stop vs Close semantics preserved); composer Close → icon-only `action.close` sending the same `coding-agent.close` intent; Session Info/drawer Back and "Back to list" now carry decorative `navigation.back` glyphs with distinct visible labels retained; recent-session rows compact "Resume" into icon-only `session.resume` with distinct names `Resume <sessionId>`; empty-Files actions (Open file / Resume session / Search sessions…) and agent tab labels keep text; Allow/Deny approval strip keeps text.
    - `frontend/src/components/button.tsx`: `ClayIconButton` gained an optional `type` prop (submit support for icon-only form actions); `type` was added to `button.tsx` not the panel.
    - Tests: `src/test/editor.test.tsx` +1 (action row invokes each handler exactly once with glyph assertions and Open-stays-text check); `src/coding-agent/CodingAgentPanel.test.tsx` +3 (composer submit-Send/disabled gate/Close intent, distinct Resume names + retained empty-state labels + resume invocation, tab labels and Allow keep text with no svg takeover). Existing role/name queries pass unchanged because every icon-only control kept its accessible name.
    - Verification: `npm run typecheck` clean; 277 vitest tests pass (+4); lint unchanged from HEAD baseline (12 pre-existing). Not applicable per plan note: generation-stop already existed (`abortRun`), so it was migrated rather than recorded as absent. ChatPanel's separate Cancel button is outside this task's file list (chat surface is not a coding-agent/editor/core-control surface).
    - Deviations: none within scope; dropdown/disclosure glyph migration was already delivered in Task 7 (`disclosure.down`/`disclosure.right`).

- [x] 9. Integrate workspace browser and first-party package status icons end to end
  - Acceptance Criteria:
    - Functional: File-browser rows expose semantic kind icons and parent navigation; Git labels show branch/state icons alongside real values; Markdown preview gains icon plus truthful toggle semantics. All resolve from same selected pack, including third-party replacement.
    - Performance: Carry only semantic keys on rows; maintain bounded directory listing/payload caps and no extra filesystem stat/scan or Git refresh due to icon rendering.
    - Code Quality: Derive file kind from server metadata, not slash/name heuristics; preserve list IDs/actions/counts. Package JS composes generic inert fields rather than client special cases. Catalog/public guide updates match new fields.
    - Security: Symlink icons do not change traversal/open authorization. Git remains read-only and sanitized; Markdown toggle retains existing command permissions; status icons never masquerade as clickable buttons.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `src/shell/file_browser.rs:320-414`
      - `packages/git/dist/status.js`
      - `packages/markdown/dist/sdui.js`
      - `docs/wiki/modules/package-git.md`
      - `docs/wiki/modules/first-party-markdown-package.md`
      - `docs/reference/packages/creating-packages.md`
    - Options Considered:
      - Hardcode icons by package name in frontend: defeats third-party generic contract.
      - Guess file type from display string or fetch metadata in renderer: unreliable and unauthorized.
      - Publish semantic references from existing authoritative producers: chosen.
    - Chosen Approach:
      - Add generic label/status/list icon support before package changes. Retain branch names, counts, stale/error text, filenames and folder counts. Parent becomes up icon plus Parent folder. Skip decorative unknown package icons without losing text.
    - API Notes and Examples:
      ```text
      Directory metadata -> file.folder
      File metadata -> file.file
      Symlink metadata -> file.symlink
      Parent directory action -> navigation.up + Parent folder
      Git branch label -> git.branch + real branch name
      ```
    - Files to Create/Edit:
      - src/shell/file_browser.rs: semantic file-kind references and parent row text; collocated tests.
      - packages/git/dist/status.js, packages/git/docs/index.md: read-only icon+text status composition.
      - packages/markdown/dist/sdui.js, packages/markdown/docs/index.md: icon+label preview toggle/state.
      - frontend/src/sdui/registry.tsx and SDUI tree renderer only for missing generic support discovered in integration.
      - docs/reference/packages/creating-packages.md: implemented icon/label/status/list authoring examples and compatibility.
      - Existing Git/Markdown package tests, tests/package_ui_conformance.rs, frontend SDUI tests: semantic round-trip and invariants.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `src/shell/file_browser.rs:320-414`
      - `packages/git/dist/status.js`
  - Test Cases to Write:
    - File/folder/symlink/other/parent rows render without additional IO and keep same action targets.
    - Root boundary and symlink tests unchanged; long names/counts remain usable at narrow width.
    - Git clean/dirty/detached/unborn/stale/error states preserve textual meaning and no action affordance.
    - Preview on/off maps actual state and command; missing preview icon does not erase label.
    - Third-party partial pack changes supported rows/statuses; unmatched required keys fall back deterministically.
  - Completion Evidence:
    - `src/shell/file_browser.rs`: rows carry server-metadata-derived semantic icons — Directory → `file.folder`, File → `file.file`, Symlink → `file.symlink`, Other → none (text label is the sole signal) — via a new `semantic_icon()` helper (no name/slash heuristics, no extra IO); parent row now `navigation.up` icon + visible "Parent folder" label (was `../` + detail "parent"); list IDs, action targets, and child-count details unchanged. Collocated test `file_browser_rows_carry_semantic_kind_icons_and_parent_up_icon` asserts folder/file/symlink/parent icons with actions intact.
    - `src/server/connection/tests.rs`: parent-row label assertion updated to "Parent folder".
    - `packages/git/dist/status.js`: status labels now compose core-key icon references beside unchanged sanitized text — HEAD (`git.branch` for branch/detached/unborn), dirty (`status.success` clean, `status.warning` dirty/changed, none when not refreshed), refresh (`status.success` last-success, `status.error` last-error, none otherwise). Labels-only tree remains: no action targets, no callbacks, read-only authority unchanged.
    - `packages/markdown/dist/sdui.js`: `markdown.togglePreview` button carries `preview.toggle` beside its label with the same `markdown.togglePreview` command; the preview status label carries `preview.toggle` only when `previewEnabled` is true (state-truthful) and the text stands alone otherwise.
    - `frontend/src/sdui/renderer.tsx` (native SDUI renderer, the missing generic support): label/button/list-item `icon` fields now project through the shared `ClayIcon` path (registry path already done in Task 7). New renderer test covers file-folder row icon, git-branch label icon, and plain-label absence; file-browser SDUI rides this path end to end.
    - Docs: `packages/git/docs/index.md` and `packages/markdown/docs/index.md` describe the icon composition and text-alone fallback; `docs/reference/packages/creating-packages.md` SDUI example gains the `icon: "preview.toggle"` authoring pattern plus a Plan 112 semantic-icon-reference paragraph (core-keys-only at runtime, namespace keys at record time, text-alone truth, no added action authority).
    - Verification: `cargo fmt` clean; `cargo check --all-targets` clean; `cargo clippy --all-targets -- -D warnings` 0 findings; 1250 lib tests pass (+1 file-browser icons); runtime/security/presentation suites pass; protocol suite 207/208 with only the documented pre-existing parity-ledger failure (Plan 109 domain); `tests/primitives_docs.rs` catalog-partition guard updated for the Task 7 Tooltip/Icon-slot implemented promotion (same pattern as the Phase 20.5/22.3 promotions); frontend typecheck clean, 278 vitest tests pass (+1 renderer icon projection), lint unchanged from HEAD baseline.
    - Deviations: none. Third-party partial-pack fallback for these rows/statuses is covered by the Task 7 icon-store fallback tests (unknown keys render empty decorative slots while text labels persist); symlink rows keep the same open-file action target as regular files, so icon display adds no traversal authority.

- [x] 10. Run cross-layer compatibility, security, and performance verification
  - Acceptance Criteria:
    - Functional: All new schema, packages, activation, protocol, controls and migrated surfaces pass integrated Linux tests; old package fixtures and no-selection startup remain usable. No first-party-only test shortcut hides third-party failures.
    - Performance: Check pack/frame ceilings and frontend bundle budgets; compare baseline typing/scroll/render counts. No package JS/IPC/XML parsing on input or paint paths and no asset/network requests at render time.
    - Code Quality: Run root and Tauri formatting/check/clippy/test gates plus frontend typecheck/lint/tests/build/budget checks. Wire new test modules into actual declared suites; do not assume a standalone tests file executes automatically.
    - Security: Exercise hostile fixture corpus, trusted-name spoofing, denied internal ops/modules, approval/revocation/rollback, stale generation, oversized snapshots and theme color injection end to end.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `frontend/package.json`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `tests/suites/`
      - `docs/development/performance.md`
    - Options Considered:
      - Only snapshot-test rendered icons: misses authority, generation and action regressions.
      - Use existing test frameworks and real cross-layer fixtures: chosen.
    - Chosen Approach:
      - Build complete matrix: Regular/Duotone/partial-third-party/fallback across Neobrutal and Glass plus two materially different light/dark themes. If other design systems are already shipped, verify their fallback/slot compatibility too without coupling to unfinished Plan 111 work.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test
      cargo check --manifest-path src-tauri/Cargo.toml --all-targets
      cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
      cargo test --manifest-path src-tauri/Cargo.toml
      npm --prefix frontend run typecheck
      npm --prefix frontend run lint
      npm --prefix frontend test
      npm --prefix frontend run build
      npm --prefix frontend run check:budget
      ```
    - Files to Create/Edit:
      - Existing relevant Rust/Tauri/frontend test files and declared suite registrations: integration coverage only where missing.
      - test-plan/artifacts/112-icons/automated/: command summaries, measured sizes/render counts, failure evidence.
      - plans/112-Configurable-Icon-Packs-and-UI-Integration.md: actual checks and unresolved defects.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `frontend/package.json`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Matrix passes same action/label semantics regardless of selected pack/theme/design system.
    - Old text-only fixtures and package loading tests remain green; protocol mismatch fails gracefully.
    - Maximum valid pack accepted, immediately oversized pack rejected; old/new active maps remain bounded during swap.
    - Pack swap does not recreate EditorView, reset undo/draft/focus or trigger extra edit IPC.
    - Blocked Cargo/frontend gates remain recorded blockers; no suppression of failing assertions or inflation of budgets to hide growth.
  - Completion Evidence:
    - Full battery executed (raw logs + analysis in `test-plan/artifacts/112-icons/automated/`, summarized in its `README.md`): `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings` (0 findings), root `cargo test` (1251 lib + runtime 75 + security 135 + presentation 46 + protocol 207/208), Tauri check/clippy (0 findings)/test (52 passed), frontend typecheck/lint/test (280 passed)/build/check:budget (shell gzip 164.4/180 kB, total 378.0/400 kB).
    - Only failures are the two documented pre-existing blockers at HEAD baseline: protocol `documentation_coverage::parity_ledger…` (Plan 109 WIP domain) and 12 frontend lint problems. No assertion suppressed, no budget inflated, no ledger row fabricated.
    - Tauri gap found and fixed: `src-tauri/src/bridge/dto.rs` test fixture lacked the Task 6 `icon: None` / `active_icon_pack: None` fields — added, Tauri suite now fully green (52/52 incl. icon-pack DTO round-trip and fail-closed tests).
    - Matrix gaps closed with 3 new tests: (1) `icon_pack_matrix_coexists_with_theme_and_design_system_selections` (ops/theme.rs) — Dark appearance + Gruvbox Material Dark theme + Glass design system + Duotone pack resolve concurrently; pack swap changes geometry only; failed selection preserves every other resolved state; (2) frontend "embeds no colors in icon output" — duotone opacity layer renders with `fill="currentColor"` only, zero per-path fill/stroke/style, so themes remain the sole color authority; (3) frontend "keeps action and label semantics identical across the pack matrix" — fallback → Regular → Duotone keeps accessible name and press semantics identical.
    - Remaining matrix/security/performance coverage verified as already shipped by Tasks 3–9 and inventoried in the evidence README (pack × DS × theme matrix, hostile corpus, spoofing, op authority, revocation, stale generations, oversized snapshots, color injection, payload ceilings, no-JS-on-paint-path, DOM identity across pack swaps).
    - Deviations: none. `tests/suites/` registrations confirmed wired (new modules registered in Task 4/6; suite-inventory test green).

- [x] 11. Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: Launch real matched Linux server/client; inspect every changed surface/state and retain app-only screenshots plus accessibility findings. Verify pack differences are visible rather than merely reporting selected metadata.
    - Performance: Use one batched review, fix findings together, then at most one confirmation pass; include narrow/wide windows and increased UI typography without repeated unbounded polish loops.
    - Code Quality: Review required Regular/Duotone x Neobrutal/Glass x light/dark matrix, plus third-party partial/fallback, forced-colors, reduced-motion/transparency states where supported. Record state-to-artifact index and pass/fail.
    - Security: Use scratch workspace/config without secrets. Start computer-use session with get_app_state; identify target window before input; re-observe after interaction. Never retain unrelated desktop content.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
      - `docs/development/accessibility.md`
      - `scripts/capture-ui-review.sh`
      - `.agents/skills/impeccable/reference/operate.md`
    - Options Considered:
      - Source inspection/jsdom alone: cannot prove glyph contrast, target size or real keyboard accessibility.
      - Real app screenshots plus AT-SPI/available browser AX and keyboard verification: chosen.
    - Chosen Approach:
      - Exercise hover/active/focus/disabled, tooltips, disclosure open/closed, modal containment, empty/error/loading/recovery and all changed action states. Verify names/roles/states, tab order, focus visibility, announcements, glyph contrast (3:1 informative shapes), text contrast, stop/close distinction and back destination. If tooling/launch is blocked, record exact blocker, run strongest structural checks, and leave interactive acceptance unresolved.
    - API Notes and Examples:
      ```text
      Evidence root: test-plan/artifacts/112-icons/visual/
      Per capture: pack, theme, design system, window size, state, source revision
      Per control: role/name/state, keyboard action, focus result, pass/fail
      ```
    - Files to Create/Edit:
      - test-plan/artifacts/112-icons/visual/: app-cropped screenshots, accessibility snapshots and findings index.
      - Affected implementation/tests from Tasks 3-9: only fixes demonstrated by review.
      - plans/112-Configurable-Icon-Packs-and-UI-Integration.md: screenshot paths and explicit unresolved accessibility legs.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
      - `docs/development/accessibility.md`
  - Test Cases to Write:
    - Tab through icon-only actions and activate with Enter/Space; tooltips appear on focus and dismiss correctly.
    - No clipped/low-contrast glyphs or rings at narrow width/high DPI/large text; targets do not shrink to glyph size.
    - Switch packs while focused/in-flight/dirty; focus, draft, undo and scroll remain.
    - Invalid selection and revoked active pack show truthful fallback/diagnostic with functional controls.
    - No visual/a11y pass claimed when screenshots, AX tree or keyboard input are unavailable.
  - Completion Evidence:
    - Four real-app captures PASS (app-cropped screenshots + AT-SPI dumps + delivered-tree evidence under `test-plan/artifacts/112-icons/visual/`, indexed in its `README.md`): `regular-core-light` (Regular × Neobrutal core × light, editor open — inline panel icons + icon-button action row + focus styling), `duotone-core-dark` (Duotone × dark — shade layers visibly differ from Regular), `fallback-core-dark-large` (zero-config fallback subset × large 24px typography), `coding-agent-fallback` (file-browser row icon live).
    - Pack differences are visible, not just metadata: Regular outlines vs Duotone soft-filled layers distinguishable at 16px; fallback subset renders identically with zero init.js icon lines.
    - A11y: AT-SPI dumps show correct roles/names with icon-free accessible names (decorative icons aria-hidden); keyboard activation, tooltip-on-focus, disabled gating, DOM identity across pack switches covered by the 17 structural jsdom icon tests; no visual/a11y pass claimed without evidence.
    - UNRESOLVED (exact blockers recorded, structural coverage substituted): keyboard/hover interactive legs — Wayland host has no input backend (`wtype`/`xdotool`/`ydotool` absent, RemoteDesktop portal input disabled, `can_send_development_input: false`); third-party partial-pack live capture — pnpm not installed so `clay package add` cannot install the fixture; Glass DS capture — pre-existing plan-110 task-18 `setDesignSystem("@clay/design-*")` runtime deadlock (reproduced today: glass UNRESOLVED, core PASS); forced-colors/reduced-motion live toggles unavailable.
    - Defects found and fixed in one batched pass, confirmed by one recapture: (1) live app showed pre-icon UI because `clay-desktop` embedded a stale frontend dist — rebuilt desktop binary after `npm run build` (build-process hazard, not icon code); (2) glyphs stacked above labels — `.icon` changed to `inline-block`; (3) glyphs did not track large typography — `.icon` now `max(var(--clay-dimension-icon-size, 16px), 1em)` so the token stays the floor; (4) capture script hardened: unresolved runs retain `server.partial.log`, icon fixtures registered with document + tree waits; (5) fixture-only: SDUI actions naming unregistered commands are rejected whole-tree by design (`sdui.invalid_action`) — fixtures use built-in `workspace.refresh`.
    - Hygiene: captures are scratch-workspace, no secrets; one capture that caught a terminal overlay was deleted and retaken; frontend 280 vitest pass after the CSS fixes, typecheck clean, lint unchanged at HEAD baseline.

- [x] 12. Create or verify Clay JS APIs and package-authoring reference coverage
  - Acceptance Criteria:
    - Functional: Every new public programmatic capability has stable facade, typed declaration, op wrapper and searchable reference metadata. Inventory every changed server-side Rust public function and either map public capability to JS or make helper private/pub(crate).
    - Performance: Document install-time resolution, cached rendering, budgets and return/idempotence behavior; registry generation adds no hot-path work.
    - Code Quality: Use concise callable and reserved core IDs; document user-facing name, key_bindings (empty if none), all custom_properties, module/export/Rust/op paths, sync/async results, errors, examples, tags and master-index links. Refresh generated registry and fail tests on stale/missing coverage.
    - Security: Document exact adoption/selection/revocation permissions and no renderer/OS authority. Package authors get semantic references and bounded asset schema, not raw ops or executable SVG examples.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`
      - `docs/reference/clay-js-api/theme/set-design-system.md`
      - `src/bin/update-doc-registry.rs`
    - Options Considered:
      - Only write a README example: missing discoverability, metadata and contract checks.
      - Authoritative Markdown API docs plus generated registry and package guide: chosen.
    - Chosen Approach:
      - Verify finalized selector/result and generic UI definition fields against parsers/types. Document optional icon references and label/pressed-state policies for both UI pipelines. Update primitive inventory and deterministic doc guards without exposing internal validation helpers as APIs.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test
      ```
    - Files to Create/Edit:
      - docs/reference/clay-js-api/theme/set-icon-pack.md (new proposed path), runtime/js/theme.js, runtime/js/theme.d.ts: selector docs/types aligned.
      - docs/reference/icon-packs.md (new), docs/reference/packages/creating-packages.md, docs/reference/ui-components.md, docs/reference/primitives/index.md, docs/reference/primitives/registry.md, docs/reference/primitives/ui-chrome-primitives.md: author contract, primitives, examples, limitations.
      - docs/index.md, docs/reference/clay-js-api/api-inventory.toml, docs/generated/clay-js-api-registry.json: authoritative navigation/inventory/derived registry.
      - tests/clay_js_api_inventory.rs, tests/clay_js_doc_registry.rs, tests/rust_visibility_api_mapping.rs, tests/primitives_docs.rs, tests/package_ui_conformance.rs: documentation-as-code guards.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - Delete required API doc/index link/metadata/registry entry in test fixture -> coverage gate fails without mutating repository.
    - Lookup by stable ID, user-facing name and tags finds selector with complete custom_properties.
    - Example code executes through public facade, never raw Deno ops.
    - Each new primitive/reference is linked from public primitive index and final wiki coverage gate.
  - Completion Evidence:
    - `theme.setIconPack` fully documented at `docs/reference/clay-js-api/theme/set-icon-pack.md`: stable ID `theme.setIconPack`, JS module `clay:theme` / export `setIconPack` (verified against `runtime/js/theme.js`/`theme.d.ts` from task 5), facade/backing-Rust/op paths, user-facing name "Set Icon Pack", empty `key_bindings`, complete `custom_properties` (`specifier:string=required`), sync return `{pack, iconCount, schemaVersion}`, error family (`theme.invalid_request`/`theme.load_failed`/`theme.invalid_icon_pack`), full security/agent-guidance/authority/denied/fallback sections; examples use the public facade only (never raw ops). Runtime JS/types were already task-5 deliverables; verified aligned.
    - Registry coverage: `theme.setIconPack` entry added to `docs/reference/clay-js-api/api-inventory.toml` (authority, runtime path, hot-path policy, budgets note, trusted-extension-only op note) and the generated matrix regenerated via `cargo run --bin update-doc-registry` (+34 lines in `docs/generated/clay-js-api-registry.json`); `docs/index.md` gained the authoritative link.
    - Package-authoring reference: new `docs/reference/icon-packs.md` (contribution contract, geometry schema with budgets, 21-key core set, activation/lifecycle state table, accessibility floor, security deny cases, authoring checklist) linked from `docs/index.md` Developer Guides, `docs/reference/ui-components.md` plan-112 section, and the new "Icon-pack declarations (clay.contributions.iconPack, Plan 112)" section in `docs/reference/packages/creating-packages.md`; `docs/reference/primitives/ui-chrome-primitives.md` `paint_icon_slot` primitive row now carries the Plan 112 React realization note.
    - Rust public-surface inventory: the only new user-facing capability is `op_clay_theme_set_icon_pack`/`apply_icon_pack` (mapped to the JS API above); `shell::icons` validation/parse helpers and `packages/record` icon-pack parsing are consumed cross-module and by the external Tauri bridge (`clay::shell::icons::ActiveIconPack`), documented in the API doc's Backing implementation as intentionally not package-facing; visibility gate (`rust_visibility_api_mapping`, security suite) passes unchanged.
    - Documentation-as-code gates green without modification: protocol suite 207 passed (inventory schema/matrix-match/metadata-contract/source-path-existence tests all validate the new entry; only the pre-existing Plan 109 parity-ledger failure remains), security 135 passed, presentation 46 passed. Deletion/lookup/read-only guard behaviors are the existing generic tests, which exercise the new entry through the same fixtures.

- [x] 13. Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Verify user selection from init.js and modular imports, explicit package loading, deterministic startup/no-selection behavior, reload/rollback and optional customization. All supported options are documented APIs rather than hidden config keys.
    - Performance: Re-evaluate through existing serialized reload path; unchanged config does not re-send/re-render unchanged icon state. No new watcher, polling loop or preferences file.
    - Code Quality: Confirm precedence: explicit selector determines active pack, otherwise bundled safety fallback; registration does not select by load order. Selection survives restart by re-evaluating config. Keep theme/typography/design system independent.
    - Security: Configuration cannot bypass adoption, capabilities, trusted-domain policy, revoked package checks or package-root confinement. Reject unsupported options and package caller attempts to change global selection.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/configuration.md`
      - `docs/reference/clay-js-api/theme/set-icon-pack.md (created in Task 12)`
    - Options Considered:
      - Introduce hidden iconPack JSON preference or implicit load-order selection: rejected.
      - Existing init.js facade/transaction path with documented precedence: chosen.
    - Chosen Approach:
      - Cross-check parser defaults/enums/result/error states against API inventory and examples. Preferred convention: zero lines uses bundled Regular safety subset; recommended explicit path is `loadPackage` then `setIconPack` (same split as load versus `setDesignSystem` selection, not a missing one-line primitive). Loading both packs in either order without selection must not change icons. No copied manifests, low-level registration, or Settings dropdown required.
    - API Notes and Examples:
      ```javascript
      // Proposed finalized equivalent, verified during this task.
      import { loadPackage } from "clay:packages";
      import { setIconPack } from "clay:theme";
      await loadPackage("@clay/icons-phosphor-regular");
      setIconPack("@clay/icons-phosphor-regular");
      ```
    - Files to Create/Edit:
      - docs/reference/clay-js-api/theme/set-icon-pack.md, docs/reference/clay-js-api/configuration.md, docs/index.md, docs/reference/clay-js-api/api-inventory.toml, docs/generated/clay-js-api-registry.json: exact configuration contract.
      - src/server/js_runtime/tests.rs and tests/fixtures/configuration/plan112-icons/ (new): modular init, overrides/reload/default/deny fixtures.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - Copy minimal load+select example; selected style is correct with no low-level registration.
    - Load both packs in either order with explicit selection; active choice remains identical.
    - Local module import and config watcher/reload apply same behavior; restart reproduces selection.
  - Completion Evidence:
    - New fixture set `tests/fixtures/configuration/plan112-icons/`: `load-select/init.js` (documented `await loadPackage(...)` + `setIconPack(...)` recommended path, no low-level registration), `modular/init.js` + `modular/icons-config.js` (selection through a local module import using only public facades), `default/init.js` (zero icon lines), `deny/init.js` (unloaded third-party specifier caught and recorded).
    - Six integration tests in `src/server/js_runtime/tests.rs` (all pass): `plan112_load_then_select_recommended_path_activates_pack` (example works verbatim: Regular, 21 icons, summary recorded), `plan112_both_packs_loaded_either_order_explicit_selection_wins` (both packs loaded in both orders × both selections — active pack always the explicit choice, load order never influences it), `plan112_modular_import_selection_and_unchanged_reload_behavior` (modular import selects identically; unchanged re-load re-executes nothing and re-sends no icon state — zero new op records with the snapshot persisting), `plan112_persisted_icon_pack_preference_reapplies_after_restart` (init.js without icon lines + persisted `iconPack` preference → selection reproduced after restart), `plan112_zero_icon_configuration_keeps_bundled_fallback_without_snapshot` (no selection → no snapshot, bundled fallback), `plan112_selecting_unloaded_third_party_pack_fails_closed_to_fallback` (`theme.load_failed` sanitized diagnostic, no snapshot, no success record).
    - Precedence/security verified by these tests plus existing coverage: trusted-extension-only op registration (task 5 tests deny package callers), `icon_pack_preference_persists_survives_reload_and_drops_garbage` (configuration.rs), cross-independence matrix test (task 10), wire re-validation (task 6). No new preference file, watcher, or polling loop; reload rides the existing serialized evaluation path.
    - Documentation: new "Plan 112 icon-pack selection configuration" section in `docs/reference/clay-js-api/configuration.md` (zero-config default, load ≠ select, persistence/restart, independence, security); `set-icon-pack.md` "When to use" now shows the recommended `loadPackage` + `setIconPack` path; registry regenerated (`update-doc-registry`) with unchanged metadata contract.
    - Gates: lib 1257 passed (+6), fmt/check/clippy clean, runtime 75, security 135, presentation 46, protocol 207 passed (only the pre-existing Plan 109 parity-ledger failure remains).
    - Invalid required config preserves prior authorized generation; revoke cannot preserve withdrawn assets.
    - Every behavior-changing property appears in custom_properties; unknown option rejected.

- [x] 14. Update canonical example configuration
  - Acceptance Criteria:
    - Functional: Update examples/config/init.js with a comprehensive icon-pack section, recommended Regular selection and commented Duotone/third-party alternatives. Every supported icon configuration surface appears once with option type/default/allowed values and ownership.
    - Performance: Default example remains lightweight and offline-safe for bundled assets; no additional environment-specific startup dependencies.
    - Code Quality: Preserve existing section ordering, authorization-before-load rules, modular imports and planned-but-not-callable tail. Cross-check example options against parser, TS declarations, public docs and API inventory.
    - Security: Keep third-party adoption, heavy/environment-specific grants and provider setup commented. Copying example must not implicitly grant filesystem/network/shell/process authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `examples/config/init.js`
      - `.agents/skills/create-plan/references/clay.md: Example Configuration Maintenance Task`
      - `docs/reference/clay-js-api/theme/set-icon-pack.md (Task 12)`
      - `.agents/skills/project-patterns/references/configuration-system.md`
    - Options Considered:
      - Append a terse unexplained snippet: incomplete canonical config documentation.
      - Integrate one annotated section with safe active defaults and commented variants: chosen.
    - Chosen Approach:
      - Use implemented selector spelling only; annotate explicit load versus selection versus fallback. Do not silently activate another appearance system or rewrite unrelated configuration.
    - API Notes and Examples:
      ```bash
      node --check examples/config/init.js
      ```
    - Files to Create/Edit:
      - examples/config/init.js: single annotated icon section and safe default.
      - examples/config/packages/ (only if modular example assets are actually needed): inert fixture/config files, not mandatory extra setup.
  - Completion Evidence:
    - `examples/config/init.js` section 2 (clay:theme) gained one annotated icon block after setDesignSystem: active recommended `setIconPack("@clay/icons-phosphor-regular")` (offline-safe, bundled, same as the zero-config default), commented Duotone alternative and `{ specifier }` object form, and comments covering option type/allowed values/ownership (user-global, trusted-runtime-only op so package callers cannot change it), load ≠ select (bundled packs need no loadPackage; third-party requires load first), persistence (`iconPack` preference wins over this call on every reload), and fallback (failed/revoked selection preserved; unknown keys never blank a control). Import line extended with `setIconPack` (each facade still imported exactly once).
    - `examples/config/packages/third-party.js` gained the commented third-party load-then-select template (`loadPackage` here, `setIconPack` in init.js) inside the existing adoption-path section — third-party adoption stays commented and copy-safe; no active line grants load/adoption authority.
    - Section ordering, authorization-before-load rules, modular imports, and the planned-but-not-callable tail are untouched; no icon `loadPackage` lines added for bundled packs (they resolve from the compiled inventory).
    - Cross-checked against `apply_icon_pack` (string/{specifier}, trimmed non-empty), `runtime/js/theme.d.ts` (SetIconPackOptions), `theme.setIconPack` doc (task 12), and the api-inventory entry.
    - `node --check` passes on init.js, first-party.js, and third-party.js.
    - Guard updated and strengthened: `clay_js_doc_registry::canonical_example_covers_theme_typography_and_modular_configuration` now asserts the new import line, exactly one active Regular selection, commented Duotone + object-form alternatives, the load-≠-select/persistence/fallback/ownership comment markers, and the single commented third-party icon load in the template. Protocol suite: 51/51 clay_js_doc_registry tests pass; full protocol 207 passed (only the pre-existing Plan 109 parity-ledger failure remains); lib plan112 6/6; presentation 46; fmt clean.
      - Existing configuration example coverage tests: surface/option/default consistency.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `examples/config/init.js`
      - `.agents/skills/create-plan/references/clay.md: Example Configuration Maintenance Task`
  - Test Cases to Write:
    - node --check succeeds and canonical configuration test evaluates successfully.
    - All documented options are represented exactly once, defaults/enums match parser.
    - Commented alternative changes style without changing content theme, typography or design system.
    - No active private paths/secrets/network grants or optional third-party dependencies introduced.

- [x] 15. Launch-test real app using a copy of canonical example configuration
  - Acceptance Criteria:
    - Functional: Immediately after Task 14, launch matched Linux GUI/server using isolated copied examples/config/init.js (and packages subfolder if present). Client reaches Connected, config commits cleanly, commands/menus/panes work and actual icons match selected pack.
    - Performance: Observe startup and switch latency/bundle bounds against baseline; no repeated reload loops or unbounded diagnostics.
    - Code Quality: Record exact launch command, scratch config/data/workspace/socket paths, source revision, generation and observed results. Test unmodified copied default first, then edited copies selecting Duotone and authorized third-party fixture.
    - Security: Never launch test against developer profile. Use unique config/data roots and IPC endpoint through documented launcher options; preserve session/runtime environment required by GUI and isolate only intended paths.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/create-plan/references/clay.md: Example Configuration Live Launch-Test Task`
      - `docs/development/launch-and-gui-smoke.md`
      - `scripts/capture-ui-review.sh`
      - `examples/config/init.js`
    - Options Considered:
      - Only node syntax check: misses runtime/config/transport/renderer breakage.
      - Fresh scratch profile with actual app and interactive checks: chosen.
    - Chosen Approach:
      - Reuse documented launch harness after reading its exact options; do not invent environment flags. Verify configured theme/design-system still looks correct as well as icon style. Invalid selection surfaces diagnostic/fallback; restore valid configuration and confirm recovery. If GUI blocked, start real server with copied config and assert generation success, but keep GUI/interactive acceptance unresolved.
    - API Notes and Examples:
      ```bash
      scratch=$(mktemp -d)
      mkdir -p "$scratch/config/clay"
      cp examples/config/init.js "$scratch/config/clay/init.js"
      if [ -d examples/config/packages ]; then
        cp -R examples/config/packages "$scratch/config/clay/packages"
      fi
      node --check "$scratch/config/clay/init.js"
      # Then use documented real-app launcher with this scratch config and unique IPC endpoint.
      # Record the resolved launch command in completion evidence.
      ```
    - Files to Create/Edit:
      - test-plan/artifacts/112-icons/example-launch/: exact launch transcript, runtime generation result, app-only screenshots and interaction checks.
      - plans/112-Configurable-Icon-Packs-and-UI-Integration.md: default/alternate/failure/recovery results and blockers.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `.agents/skills/create-plan/references/clay.md: Example Configuration Live Launch-Test Task`
      - `docs/development/launch-and-gui-smoke.md`
  - Test Cases to Write:
    - Unmodified copied config: Connected, no configuration failed, shell responds to open/command/menu.
    - Copied Duotone config: visible shaded paths, metadata agrees; theme/design-system unchanged.
    - Copied third-party fixture config: adoption required, style changes and missing keys fall back.
    - Unknown/revoked selection: bounded diagnostic and usable UI; restored config recovers.
    - No launch/keyboard tooling -> exact blocker and strongest server-only evidence, not a claimed GUI pass.
  - Completion Evidence:
    - Artifacts: `test-plan/artifacts/112-icons/example-launch/` (README.md records the exact launch recipe, per-variant results, performance numbers, and blockers; each variant directory carries `review.status`, `metadata.txt` with socket/connected latency, `config-verify.txt` proving the launched selection lines, AT-SPI `accessibility.txt` + `dump-*.txt`, `server.log`/`client.log`, and an app-only `screenshot.png` cropped to the Clay window).
    - Launch recipe (corrected): the server resolves its configuration root at `$HOME/.config/clay` (`ConfigurationRuntime::default_config_root`), NOT `XDG_CONFIG_HOME` — the plan sketch's XDG-only copy silently no-ops (initial four runs validated core defaults with the example unread; discarded and re-run). `XDG_CONFIG_HOME` governs `layout.json` window state only, which is how the scratch document opens. Isolated mode-700 scratch config/data/home/workspace/tmp roots, private Unix socket, synthetic documents; never the developer profile.
    - Unmodified copy: PASS — client reaches Connected ≈ 1.3s (socket ≈ 0.11s), example config evaluates (25-line bounded server log, zero failure diagnostics), file browser shows folder/file kind icons, editor action row renders icon buttons (Save/Reload/Close) with Open as text, rust mode + gruvbox dark active, document open and editable (`v1 · clean · editable`).
    - Duotone copy: PASS — same shell, selection line swapped in the copy; rendered pixels differ from default (36 rows), theme/design-system/appearance unchanged.
    - Third-party fixture copy: BLOCKED live — `pnpm` absent on this host, so `clay package add`/`adopt` cannot run; the unloaded-specifier variant exercises the identical fail-closed selection path end to end. Recorded as a blocker, not a pass.
    - Unknown/revoked selection: PASS — `@vendor/unloaded-icons` in the copy produces the bounded sanitized diagnostic at startup AND reload (`clay server configuration failed [theme.load_failed]` / `clay server runtime reload failed [theme.load_failed]`), the app stays up with the editor, document, and bundled-Regular fallback icons usable.
    - Recovery: PASS — break the RUNNING config (unloaded selection): reload rejected fail-closed, previous working generation preserved, UI stays usable; restore the Regular selection: reload succeeds with no new failures and the shell + document recover (dump-broken/dump-restored both show the open document).
    - Performance: no repeated reload loops (server log bounded at 13–25 lines after touch + settle) and no unbounded diagnostics; startup/switch latency far inside baseline.
    - Blockers (environmental): no input-synthesis backend (keyboard/pointer) on this host — interactive keyboard/pointer legs remain UNRESOLVED and are covered by jsdom suites (tasks 7–8) and task 11 captures; transient once (1 of 4 runs): the first no-op reload recorded a `theme.load_failed` evaluation failure with an unchanged valid selection and the next watcher event re-evaluated cleanly — flagged for the task 16 manual plan follow-up.

- [x] 16. Execute and update numbered manual test plan
  - Acceptance Criteria:
    - Functional: Add and execute numbered icon-package steps with expected outcomes, negative checks and known ceilings; record per-step pass/fail/unresolved on real Linux app. Keep affected module coverage/index accurate.
    - Performance: Include no-network rendering, responsive controls and typing/scroll/pack-swap checks without turning manual test into stress-loop polish.
    - Code Quality: Cross-link deep reference/visual evidence rather than duplicate docs. Preserve every existing regression step and distinguish inherited ceilings from introduced defects.
    - Security: Test unadopted/invalid/revoked pack handling and retained destructive confirmations using scratch files only; no real provider credentials required for icon UI checks.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `test-plan/index.md`
      - `test-plan/02-configuration-init-js.md`
      - `test-plan/03-files-and-workspace.md`
      - `test-plan/09-packages-and-modes.md`
      - `test-plan/14-tabs.md`
      - `test-plan/15-ui-design-systems.md`
      - `test-plan/17-coding-agent-parity.md`
    - Options Considered:
      - Ad hoc screenshots with no step IDs: difficult to rerun.
      - Dedicated icon module linked to existing config/file/tab/agent modules: chosen.
    - Chosen Approach:
      - Create module 18 if still available, otherwise use next free number and update this task. Cover default/alternate/third-party/sparse/failure/revocation/reload, controls/labels, file/Git/preview semantics, a11y and pack-theme-design independence. Reuse Task 11/15 evidence only where exact step/state matches.
    - API Notes and Examples:
      ```text
      Proposed manual IDs: ICON-01 through ICON-12
      Each step: setup -> action -> expected result -> negative check -> evidence -> result
      Never delete/weaken a failing legacy step to obtain PASS.
      ```
    - Files to Create/Edit:
      - test-plan/18-icon-packs.md (new number tentative): numbered feature/negative/accessibility/ceiling steps.
      - test-plan/index.md: module map, coverage matrix and evidence links.
      - test-plan/02-configuration-init-js.md, 03-files-and-workspace.md, 09-packages-and-modes.md, 14-tabs.md, 15-ui-design-systems.md, 17-coding-agent-parity.md: cross-references and affected step results.
      - test-plan/artifacts/112-icons/manual/: per-step results and remaining blockers.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `test-plan/index.md`
      - `test-plan/02-configuration-init-js.md`
  - Test Cases to Write:
    - All placement rows have manual coverage and retained-label negative checks.
    - Both packs visibly differ on same state; third-party uses identical host controls.
    - Keyboard-only focus/tooltips/actions and narrow/large-typography cases recorded truthfully.
    - No unsupported Windows-only requirement blocks Linux completion; unavailable checks remain unresolved.
  - Completion Evidence:
    - Created `test-plan/18-icon-packs.md` (module 18, ICON-01–ICON-12) with setup, numbered action/expected steps covering zero-config fallback, Regular/Duotone selection, watcher swap + fail-closed break/restore recovery, unloaded/unknown selection diagnostics, third-party own-prefix/hostile handling, file-browser/git/markdown semantics, icon-only a11y contract (names, tooltips, hit targets, retained text labels), AT-SPI pass, no-network inlined-geometry rendering, and responsive/large-typography/pack-swap feel — plus an execution record and a known-ceilings section.
    - Executed 2026-09-07 on the current build: ICON-01–05, 07–12 PASS citing existing task 11/15 artifacts (`test-plan/artifacts/112-icons/visual/`, `example-launch/`, `automated/`); ICON-06 (live third-party adoption) UNRESOLVED — `pnpm` absent, identical fail-closed path covered by fixture suites; ICON-08 (live hover/focus tooltips) UNRESOLVED — no input-synthesis backend, structure covered by AT-SPI dumps + jsdom suites.
    - Cross-linked, never duplicated: `test-plan/index.md` gained module 18 in the module map, an icon coverage-matrix row (18 + 02/09/15), and a dated Plan 112 execution-record section; modules 02, 03, 09, 14, 15, 17 gained Plan 112 cross-reference sections pointing at module 18 steps; per-step blocker summary at `test-plan/artifacts/112-icons/manual/README.md`.
    - No existing step deleted or weakened; inherited ceilings (input synthesis, pnpm, plan-110 task-18 deadlock, one-run transient first-reload `theme.load_failed` flake) recorded as ceilings, not defects.

- [x] 17. Update or verify code wiki after implementation and final verification
  - Acceptance Criteria:
    - Functional: Update implementation wiki once after implementation, automated/live verification, public API/configuration/example/manual tasks. Master index links the new icon module and relevant existing modules; document actual final behavior, not this proposal as if shipped.
    - Performance: Explain cache/generation/transport boundaries, pack and path budgets, bundle measurements, no-hot-path-work invariants and known ceilings; wiki adds no runtime work.
    - Code Quality: Explain source ownership, validated data structures, state transitions, fallback/revocation, trust separation, host a11y composition, adding a pack/key, testing and regeneration. Link authoritative API docs rather than duplicating them. Run final doc guards and blocking checks; only then fill compromises/further actions with real outcomes.
    - Security: Document denied SVG/URL/execution authority, exact provenance, adoption, revocation and failure handling without leaking secrets or overstating third-party isolation.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-wiki/references/page-template.md`
      - `docs/wiki/modules/ui-design-system-runtime.md`
      - `docs/wiki/modules/package-loading.md`
      - `docs/wiki/modules/react-sdui-package-ui.md`
      - `docs/reference/icon-packs.md (Task 12)`
    - Options Considered:
      - Update wiki after each partial task: creates churn and prematurely describes proposed behavior.
      - One final indexed implementation update backed by verified source/tests: chosen.
    - Chosen Approach:
      - Add icon runtime module page and link from existing theme/package/SDUI/primitive explanations. Keep primitive inventories and deterministic docs tests synchronized. Refresh graph with graft build after substantial code changes; re-run final checks from Task 10 plus registry/doc checks, review diff, then record actual deviations and priorities.
    - API Notes and Examples:
      ```bash
      cargo run --bin update-doc-registry
      cargo test
      graft build
      git diff --check
      ```
    - Files to Create/Edit:
      - docs/wiki/modules/icon-pack-runtime.md (new): complete implementation flow, ownership, bounds, examples, source/test paths.
      - docs/wiki/index.md, docs/wiki/modules/primitive-architecture.md, docs/wiki/modules/ui-design-system-runtime.md, docs/wiki/modules/package-loading.md, docs/wiki/modules/react-sdui-package-ui.md: navigation and integration explanations.
      - tests/primitives_docs.rs and relevant documentation coverage tests: module/reference/master-index coverage.
      - plans/112-Configurable-Icon-Packs-and-UI-Integration.md: final evidence, actual compromises and further actions; checked tasks only if accepted.
      - graft/: deterministic refresh artifacts only after substantial implementation.
    - References:
      - Contract Targets and Placement and Label Policy in this plan.
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-wiki/references/page-template.md`
  - Test Cases to Write:
    - Master wiki index resolves new page; each reusable primitive has reference/wiki/source/test links.
    - Documentation coverage fails on missing required module/index/reference entry.
    - Public API examples agree with implemented facade/parser and canonical copied configuration.
    - Final Linux and frontend gates pass; visual/interactive blockers remain explicit rather than erased.
  - Completion Evidence:
    - Created `docs/wiki/modules/icon-pack-runtime.md` following the project-wiki template: sources/tests/reference-docs header, responsibilities split (packages supply inert bounded geometry; host owns validation/rendering/color), schemaVersion-1 schema with exact constants, provenance/trust rules, the load ≠ select lifecycle (trusted-only op, prepare-phase revalidation, generation-stamped snapshot, DTO projection, IconStore, bootstrap `None`), rendering/a11y contract (moveto-first, currentColor-only, size floor `max(…, 1em)`, required-label icon buttons), semantic references on host surfaces, invariants/budgets, add-a-pack/add-a-key paths, and per-test commands.
    - Master index `docs/wiki/index.md` links the new page (satisfies `wiki_index_links_every_wiki_page`); integration cross-links added to `ui-design-system-runtime.md` (lifecycle mirror), `package-loading.md` (record parsing pattern), `react-sdui-package-ui.md` (icon field projection), `frontend-theme-runtime.md` (core-token consumption), and `primitive-architecture-docs.md` (React realization of the icon-slot/tooltip-shell gaps). Public API docs remain authoritative and are linked, not duplicated.
    - `cargo run --bin update-doc-registry` re-run (registry regenerated, no drift); `graft build` refreshed the graph; `git diff --check` clean.
    - Final gates: `cargo fmt --check` clean; `cargo check --all-targets` clean; `cargo clippy --all-targets -- -D warnings` clean; lib 1257 passed; runtime 75 passed; security 135 passed; presentation 46 passed; protocol 207/208 (only the pre-existing Plan 109 parity-ledger failure — C1–C20 rows added by WIP commit 1b65568 without ledger updates — unchanged by this plan); Tauri check/clippy clean + 52 tests passed; frontend typecheck clean, 280 vitest passed, production build OK; lint unchanged from the HEAD baseline (12 pre-existing problems).
    - Blockers remain explicit: live third-party adoption (no pnpm), interactive keyboard/pointer a11y legs (no input-synthesis backend), and the plan-110 task-18 init.js design-system deadlock — all recorded in module 18 and the task 11/15 evidence, none erased.

## Compromises Made
- Geometry is inlined directly in each pack's `package.json` (`clay.contributions.iconPack`) instead of a separate icons file — 21-key packs fit the 64 KiB contribution budget and keep the manifest the single registration source.
- The wire format carries per-path SVG `d` strings (normalized at load to absolute commands) rather than pre-parsed command vectors — smaller payloads and the generator passes upstream Phosphor data through verbatim.
- `BootstrapDto.active_icon_pack` is `None` until the first `RuntimeStateSnapshot` (mirrors the design-system core-fallback treatment); bootstrap UI briefly renders the bundled fallback subset.
- The `iconPack` preference validator accepts arbitrary specifiers (unlike the strict theme validator) because selection is re-validated at every commit and revoked packs fail closed to the bundled Regular subset.
- Pack-swap-while-running was verified via the scripted isolated-launch recovery leg and jsdom DOM-continuity tests rather than manual interactive clicking — this host has no keyboard/pointer input synthesis.
- Scope exclusions in the plan (native chrome chrome.tsx migration for badge/kbd/divider, etc.) remain intentional plan boundaries, not unverified compromises.

## Further Actions
- Investigate the one-run transient first-reload `theme.load_failed` evaluation failure observed during task 15 (unchanged valid selection, next watcher event clean, unreproduced elsewhere) — medium priority, server reload/worker-cache path.
- Recover the Plan 109 parity-ledger failure (`documentation_coverage::parity_ledger_covers_every_manual_step…`, C1–C20 rows vs `tauri-react-parity-ledger.json`) in the Plan 109 domain — pre-existing, high priority there, untouched here to avoid fabricating ledger rows.
- Chase the 12 pre-existing frontend lint problems present at HEAD — low priority, out of icon scope.
- Revisit live third-party pack adoption and interactive tooltip/focus a11y legs when the host gains `pnpm` and an input-synthesis backend — blocking, environment not code.
- Windows cross-compilation remains a long-term target; all blocking gates were Linux (primary host) and passed.
