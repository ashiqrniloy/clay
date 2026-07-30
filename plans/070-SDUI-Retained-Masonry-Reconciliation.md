# SDUI Retained Masonry Reconciliation (Incremental, Component by Component)

## Context

The SDUI *data model* (`src/protocol/sdui.rs`) is a retained, versioned, incrementally-updated tree: stable `SduiNodeId`s, `SduiTree` snapshots, and `SduiTreeUpdate` ops (`ReplaceRoot`/`ReplaceNode`/`RemoveNode`). The *renderer* is not. `SduiNativeState` (`src/masonry_sdui.rs:84`) implements `Widget` with an empty `register_children` — it is a leaf that immediate-mode paints the entire panel/component tree into the Vello `Scene`, hand-rolling layout math, scroll, pointer hit-testing, keyboard nav, focus traps, and accesskit nodes (~161 manual paint/geometry/hit-test helpers). `EditorWidget` (`src/masonry_editor.rs:2268`) is likewise a leaf that paints editor surface + SDUI sidebar + status line and holds `sdui: SduiNativeState` as a field (`:293`).

Approved decision `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md` already chose a Clay-owned declarative shell over Masonry as substrate (use `Split`/`Flex`/`Grid`/`ZStack`/`Portal`) and explicitly rejected "keep `EditorWidget` as the whole app shell and overlay SDUI panels manually" as the long-term direction. The current hand-paint path is that rejected-as-long-term option. This plan closes that drift: converge the chrome/panel/component renderer onto a retained Masonry widget subtree reconciled from the SDUI tree, one component kind at a time, while leaving the bespoke editor text canvas (`src/editor/surface.rs`) untouched.

Authority model is preserved unchanged: server validates inert SDUI trees; no package JS in paint/layout/input; typed tokens not CSS; Clay owns native widgets. Only the client-internal rendering strategy changes.

## Migration Strategy (v2 — corrected sequencing)

Two constraints discovered during execution reshape the order.

**Constraint 1 — monolithic legacy layout (foundation-first, already executed).** `paint_node` laid out every kind in one shared `cursor_y` flow, so a single kind could not be extracted without breaking layout. This drove the foundation-first sequencing already completed: container reconciler (task 5) → rendering cutover (task 6.5) → leaf handling. Containers (`panel`/`flex`/`stack`) now reconcile to real Masonry `Flex`/`ZStack`; leaf kinds render through the retained tree via a parley-direct `SduiLegacyLeaf` that reports exact legacy row geometry.

**Constraint 2 — the rendering cutover used a render-only nested compositor (interactivity ceiling).** Task 6.5 hosted the reconciled region via a *nested* `RenderRoot` (`RetainedSdui`) composited with `Scene::append`, chosen to preserve z-order (tree above editor chrome, below package overlays) without restructuring `EditorWidget`. That compositor is **render-only**: it receives no pointer/key events, discards widget actions (`|_| {}`), and is recreated wholesale on every tree/theme/size change (so focus/input state cannot persist). Consequence: **real interactive Masonry widgets cannot work under it** — a real `Button`/dropdown/`textInput` would render but never receive a click. This blocks every interactive kind (button, list/scroll, dropdown/collapse/modal/textInput) and would force per-widget hand-built interactivity on the legacy hit-test path — exactly what this plan migrates away from.

**Constraint 3 — the wholesale region rebuild resets widget identity (state-persistence ceiling).** Step 8 feeds the region by rebuilding it *wholesale* on every `apply_snapshot`/`apply_update` (`sync_region` swaps the whole `WidgetPod`). Every widget gets a fresh `WidgetId` per update, so Masonry-managed state is discarded. This is invisible for the transient button/list hover migrated in Steps 9–10, but it resets *persistent* state — a `Portal`'s scroll position (Step 12) and the open/focus/selection state of the transient surfaces (Step 13). The fix is a stable-identity reconciler (Step 11) that diffs the tree and updates widgets in place so surviving nodes keep their `WidgetId`. The SDUI data model is already reconciler-ready (stable `SduiNodeId` + incremental `SduiTreeOperation`s), and `SduiNativeState` already externalizes the persistent state (`scroll_offset`, `dropdown_selected`, `collapse_expanded`, `focused_action`) as a fallback. Decision: `decision-logs/2026-07-29-1451-stable-identity-sdui-reconciliation.md`.

**Corrected sequence.** Reach the target architecture by making the region a *live* part of the main widget tree first, then migrating interactive kinds to real Masonry widgets on that live tree, then retiring the god-object (step numbers refer to the Execution Order table below):

1. Geometry shell-ownership (Step 6) — prerequisite for hosting the region as a child.
2. Z-order de-risk spike (Step 7) — prove overlay z-order with the region as a real child (`ZStack`/`Portal`); gate for Step 8.
3. Structural enabler (Step 8) — host the region as a real child, solve z-order, **delete the nested compositor**; legacy hit-test retained temporarily so buttons keep working. Rendering parity verified.
4. First interactive kinds as real Masonry widgets (Steps 9–10) — button (establishes event routing + action→`enqueue_sdui_action`), then list rows. Each deletes its legacy hit-test/interaction code.
5. Stable-identity reconciler (Step 11) — replace the wholesale region rebuild with in-place diffing so widgets keep their `WidgetId` (and thus Masonry-managed scroll/focus/transient state) across server updates: spike (11a) → reconciler core (11b) → production switchover (11c). Gateway for Steps 12–13 (Constraint 3).
6. Stateful kinds (Steps 12–13) — scroll via a Clay-owned `SduiScrollViewport` (12), then the transient family (dropdown/collapse/modal/textInput + overlay/portal) split into stages 13a–13f (13f is the hosted menu a11y-parity follow-up); both rely on Step 11 to persist state across updates. Each deletes its legacy hit-test/interaction code.
7. Retire the god-object (Step 14) — host `EditorView`/editor surface as a child component; with all kinds real, delete `SduiNativeState::paint` + `SduiLegacyLeaf`; shrink `masonry_sdui.rs`.
8. Docs / verify / wiki (Steps 15–18).

The nested compositor (Step 5 / task 6.5) is therefore a **temporary stepping stone**, removed in Step 8. `SduiLegacyLeaf` is the carrying mechanism for not-yet-migrated leaves and is deleted in Step 14. Labels stay parley-direct (Step 4 resolution): Clay renders all text parley-direct, so a real Masonry `Label` is rejected as a divergent, unverifiable, no-deletion change.

**Target state.** One widget tree, one event tree, one render pass: `ClayShellWidget` → Clay-owned slot layout → real Masonry children (`Flex`/`ZStack`/`Portal`/`Label`-as-leaf/`Button`/list-scroll/overlay/dropdown/collapse/modal/textInput) reconciled from `SduiTree`; the bespoke editor text canvas hosted as a child component via `EditorView`; events routed by Masonry, actions emitted as inert `SduiActionIntent`s; no nested compositor, no `SduiNativeState` paint path, no per-widget hand-built interactivity.

## Execution Order (authoritative)

Follow this sequence one step at a time; each step is independently verifiable. Where the detailed task definitions below differ in ordering or scope, **this table is authoritative** (the detailed blocks are reference material and were written before v2).

| Step | Task | Status | Depends on | Verifiable by |
|---|---|---|---|---|
| 1 | Baseline, decision alignment, catalog review | done | — | conformance gates |
| 2 | Reconciliation seam (`SduiRegionWidget`) | done | 1 | unit + RenderRoot contract tests |
| 3 | `flex`/`stack` containers → `Flex`/`ZStack` | done | 2 | reconciliation + geometry-parity tests |
| 4 | `label`/`statusItem` (parley-direct leaf) | done | 2 | leaf height-parity test |
| 5 | Rendering cutover — nested compositor (**temporary**) | done | 3,4 | geometry-parity + manual visual |
| 6 | Unify SDUI slot geometry to one source (shell-vs-package_ui reclassified as separate concerns; region-placement → Step 8) | done | 5 | headless geometry-parity tests |
| 7 | Z-order de-risk spike — PASSED; chose Composition B (chrome=`paint()`, region=child, overlays=`post_paint()`); `Portal` is scroll not z-layer | done | 6 | two committed `spike_*` tests |
| 8 | Structural enabler: host region as real child + z-order + **delete compositor** | done | 7 | mechanism proven headlessly; **manual visual gate pending** |
| 9 | `button` → real Masonry button widget; route events; action→`enqueue_sdui_action`; delete legacy button hit-test | done | 8 | button action-intent parity test (RenderRoot click) + legacy hit-test removed; **manual visual gate pending** |
| 10 | `list` → retained Masonry row widgets; route row action; delete legacy list hit-test | done | 8 | list-row action-intent parity test (RenderRoot click) + legacy hit-test removed; **manual visual gate pending** |
| 11a | Reconciler de-risk **spike**: prove a stateful widget survives a region sync via in-place identity preservation (throwaway gate) | done | 10 | two committed `spike_*` tests: in-place `get_mut` preserves state + `WidgetId`; wholesale rebuild resets both |
| 11b | Stable-identity reconciler **core**: `SduiNodeId`→`WidgetPod` map; diff snapshot/update; reuse pods for surviving nodes; in-place prop updates; dynamic `Flex`/`ZStack` child-list mutation | done | 11a | four committed tests: identity + focus preserved across in-place update; Flex add/remove/reorder; in-place prop update |
| 11c | Production **switchover**: rewire `sync_region`/`main.rs` to in-place reconcile; delete the wholesale region rebuild | done | 11b | full suite green + existing interactive tests pass; production wholesale swap deleted (manual visual pending) |
| 12 | `scroll` → Clay-owned `SduiScrollViewport` (theme-driven scrollbar); delete manual scroll state | done | 11c | scroll position persists across SDUI updates; scroll-bounds parity; manual scroll fields removed; scrollbar follows `surface.scrollbar` theme |
| 13a | Package-component reconciliation foundation: `PackageRegionWidget` reconciles `PackageUiComponentTree` → retained widgets (stable identity); `panel`/`label`/`button`/`list` pixel parity | done | 11c | standalone geometry/paint-parity + stable-identity + kind-change tests (no production paint change) |
| 13b | Fixed-panel widget hosting (`PackagePanelHost` child) + `collapse` → real `PackageCollapse` widget (retained expanded state); delete `collapse_expanded` + legacy fixed-panel paint | done | 13a | collapse toggle/retention + panel-host reconcile tests; legacy fixed-panel paint removed |
| 13c | `textInput` → real editable field (`PackageTextInput` wrapping Masonry `TextArea<true>` + Clay-owned chrome) + optimistic value-sync | done | 13b | editing/placeholder/validation-border/commit/server-adoption tests; committed value→server intent; legacy paint deleted |
| 13d | `dropdown` → real widget (ComboBox role, ArrowUp/Down, Enter/Space confirm); delete `dropdown_selected` | done | 13b | keyboard-nav + selection-persists + open-list hover/click tests; legacy paint + `dropdown_selected` + `dropdownToggle` route deleted |
| 13e | Overlay hosting (`EditorWidget` multi-child, above region) + `modal` focus trap + `overlay`/`portal` containers; delete `focused_action`/`modal_focusable_intents` | done | 13a,13b | modal Tab/Shift+Tab trap + z-order (z.overlay<z.modal<z.tooltip) + pointer routing; menu nav re-sync via `MenuStateChanged` action |
| 13f | Hosted menu a11y parity (`MenuItem` role + selected state + custom labels on the hosted overlay path); delete the legacy overlay/menu a11y branch + re-include `overlay_host` in a11y children | open | 13e | hosted menu a11y test asserts `MenuItem` roles + selected suffix + custom labels; legacy `collect_active_menu_accessibility_entries` deleted; no double-report |
| 14 | Retire god-object: host `EditorView`/editor surface as child; delete `SduiNativeState::paint` + `SduiLegacyLeaf`; shrink `masonry_sdui.rs` | open | 13f | compile-time absence of retired fns + full suite + visual |
| 15 | Update package UI/layout/rendering docs | open | 14 | doc/catalog drift gates |
| 16 | Verify Clay JS APIs (no new surface) | open | 14 | visibility audit |
| 17 | Verify configuration APIs (no new surface) | open | 14 | config conformance gates |
| 18 | Update code wiki | open | 15–17 | manual wiki review |

## Objectives

- Replace immediate-mode SDUI painting with a retained Masonry widget subtree reconciled from `SduiTree`/`SduiTreeUpdate`, component kind by component kind.
- Recover Masonry-provided layout, hit-testing, focus traversal, pointer capture, and accessibility instead of hand-rolling them.
- Collapse the two parallel geometry systems (`WorkingAreaLayout` in `src/masonry_shell.rs` vs. `sdui_slot_layout`/`PaneSlotLayout` bridging inside the editor widget) into one Clay-owned slot geometry.
- Preserve the authority boundary, the no-package-JS-in-hot-path invariant, token-driven styling, payload budgets, and all existing conformance tests throughout.
- Delete the hand-painted code as each component kind migrates, shrinking the `SduiNativeState` god-object toward removal.
- Remove the temporary nested compositor (`RetainedSdui`) by making the reconciled region a live child of the main widget tree, so window events route through Masonry and interactive kinds use real Masonry widgets (focus/keyboard/press/a11y) instead of per-widget hand-built hit-test/focus/scroll.

## Expected Outcome

- SDUI panels and components render as real Masonry child widgets (`Flex`/`ZStack`/`Portal`/`Label`/`Button`/etc.) under a Clay-owned reconciliation container; `SduiTreeUpdate` ops apply as Masonry widget mutations outside paint.
- One widget tree, one event tree, one render pass: the nested `RetainedSdui` compositor is gone; the reconciled region is a live child of the main tree, pointer/key events route through Masonry, and widget actions emit inert `SduiActionIntent`s through the existing server-first command path.
- The editor text surface remains bespoke immediate-mode rendering (unchanged), hosted as a child component in the reconciled tree via `EditorView`.
- Hand-rolled scroll/hit-test/focus/a11y code for migrated kinds is deleted; Masonry's passes provide those behaviors.
- `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and the Linux test suite (including `tests/ui_primitive_conformance.rs` and `tests/package_ui_conformance.rs`) pass at every task boundary.
- No new package-facing authority, raw CSS, client JS, or public op surface is introduced; new client internals stay `pub(crate)`.

## Tasks

- [x] **(Step 1)** Baseline, decision alignment, and Clay UI catalog review before UI work
  - Acceptance Criteria:
    - Functional: Current SDUI render path, geometry bridging, and per-kind paint code paths are inventoried; the decision-log drift (implemented hand-paint vs. approved Masonry-substrate direction) is documented as the basis for this plan; the Clay UI catalog and token catalog are read and the migration order is confirmed against implemented kinds.
    - Performance: Baseline render/scroll metrics captured (existing `masonry.render_prepare.paint` scope in `global_recorder`) so later tasks can prove non-regression.
    - Code Quality: No code change in this task beyond this plan being checked off; migration order recorded with rationale (leaf kinds first, transient/focus family last).
    - Security: Confirm the authority boundary and conformance guards (`tests/ui_primitive_conformance.rs`, `tests/package_ui_conformance.rs`) currently pass and are listed as must-not-regress.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md`: approved Clay-owned shell over Masonry; rejected editor-as-shell + manual SDUI overlay as long-term.
      - `docs/reference/primitives/shell-layout-strategy.md`: shell vocabulary, Masonry-as-substrate, "temporary compatibility bridge" status of the SDUI sidebar.
      - `docs/reference/primitives/rendering-strategy.md`: client rendering attachment points; no package JS in paint/layout/input.
      - `.agents/skills/clay-ui/references/components.md` and `.agents/skills/clay-ui/references/tokens.md`: implemented component kinds, typed style variables, chrome primitives, token catalog.
      - `docs/reference/ui-components.md`: UI navigation/contract entry point.
      - Masonry 0.4.0 local crate source (`~/.cargo/registry/src/*/masonry-0.4.0/`): `src/widgets/{flex,zstack,portal,label,button,split}.rs`, `src/doc/implementing_container_widget.md`.
    - Options Considered:
      - Big-bang rewrite of `masonry_sdui.rs`: rejected — high risk, breaks working behavior, violates incremental adoption.
      - Status quo (keep hand-paint): rejected — conflicts with approved decision; cost rises with the implemented transient/focus component family.
      - Incremental reconciliation seam with per-kind migration and fallback coexistence: chosen.
    - Chosen Approach:
      - Record baseline and migration order: label/statusItem → button → flex/stack → panel+slot-geometry → list/scroll → overlay/portal → dropdown/collapse/modal/textInput → EditorView-as-child → retire legacy paint. Leaf kinds first (no children, simplest), transient/focus family last (highest value, carries hand-rolled focus/a11y/keyboard code).
    - API Notes and Examples:
      ```bash
      cargo fmt --check && cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --test ui_primitive_conformance --test package_ui_conformance
      ```
    - Files to Create/Edit:
      - `plans/070-SDUI-Retained-Masonry-Reconciliation.md`: this plan (baseline section checked off).
    - References:
      - `src/masonry_sdui.rs:84` (`SduiNativeState`), `:2156` (`impl Widget`, empty `register_children`).
      - `src/masonry_editor.rs:293` (`sdui` field), `:2646` (paint), `:509`/`:513` (`apply_snapshot`/`apply_update`).
      - `.agents/skills/project-patterns/references/package-ui-layout.md`, `planning-checklist.md`, `authority-boundaries.md`, `protocol-and-performance.md`.
  - Test Cases to Write:
    - Baseline conformance run: `ui_primitive_conformance` and `package_ui_conformance` pass before any render change (guard for all later tasks).
  - Baseline Captured (completed):
    - Conformance guards pass: `cargo test --test editor conformance` → 21 passed, 0 failed (`ui_primitive_conformance` + `package_ui_conformance`; note `autotests=false`, these live in the `editor` suite via `tests/suites/editor.rs`, not standalone targets). `cargo fmt --check` clean.
    - Implemented component kinds confirmed from `src/shell/components.rs::ComponentKind` (15): `editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, `statusItem`, `dropdown`, `collapse`, `modal`, `textInput`; `table` reserved. Matches catalog; migration order validated against this set.
    - Render path inventory: root `ClayShellWidget::single_editor` (`src/main.rs:1631`) → `EditorWidget` leaf (`src/masonry_editor.rs:2268`, paints at `:2646`) → embedded `sdui: SduiNativeState` field (`:293`), itself a leaf `impl Widget` (`src/masonry_sdui.rs:2156`, empty `register_children` `:2159`). SDUI applied outside paint at `masonry_editor.rs:509`/`:513`.
    - Hand-rolled surface to retire: `src/masonry_sdui.rs` = 4002 lines, 70 `SduiNodeKind::` match sites, 7 paint fns; `masonry_editor.rs` = 5129; `masonry_shell.rs` = 909.
    - Duplicate geometry bridge fns flagged for Task 6 unification (`src/masonry_sdui.rs`): `editor_region` `:2206`, `editor_region_for_document` `:2212`, `sdui_slot_layout` `:2234`, `sdui_panel_slot_layout` `:2246`, `combined_slot_layout` `:2255`, `fixed_sdui_left_slot` `:2259`, `sdui_panel_left_slot_rect` `:2271`.
    - Client-local interaction state to retire (`src/masonry_sdui.rs`): `scroll_offset` `:101`, `content_height` `:104`, `viewport_height` `:105`, `pointer_pos` `:112`, `pointer_pressed` `:113`, `focused_action` `:114`, `dropdown_selected` `:116`, `collapse_expanded` `:118`.
    - Perf scopes for non-regression tracking: `masonry.render_prepare.paint` (`masonry_editor.rs:2648`), `sdui.apply_snapshot` (`masonry_sdui.rs:368`), `sdui.apply_update` (`masonry_sdui.rs:379`) via `global_recorder`.
    - Decision drift documented: implemented hand-paint = the option `decision-logs/2026-06-09-1431-...md` rejected as the long-term direction; `shell-layout-strategy.md` labels the SDUI sidebar a "temporary compatibility bridge". This plan is the convergence.

- [x] **(Step 2)** Introduce the SDUI→Masonry reconciliation seam with legacy fallback coexistence
  - Acceptance Criteria:
    - Functional: A Clay-owned container widget reconciles an `SduiTree` into Masonry child `WidgetPod`s keyed by `SduiNodeId`, and applies `SduiTreeUpdate` ops (`ReplaceRoot`/`ReplaceNode`/`RemoveNode`) as widget mutations outside paint; an explicit per-kind whitelist controls which kinds render through the retained tree, with non-whitelisted kinds falling back to the existing immediate-mode paint so the app stays fully functional at every step.
    - Performance: Reconciliation runs in `apply_snapshot`/`apply_update` (outside paint); paint reads placed children only; no per-frame allocation proportional to node count; scroll/adjacent render stays within `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`.
    - Code Quality: Container follows Masonry container invariants (`register_children`/`children_ids`/`run_layout`/`place_child` for every child); new types `pub(crate)`; no duplication of SDUI validation (reuse `src/server/ops/sdui.rs`-validated inert trees).
    - Security: No package JS, raw CSS, raw ops, or client-side callbacks enter the seam; reconciliation consumes only server-validated inert `SduiNode`s; payload budgets unchanged.
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/doc/implementing_container_widget.md`: containers must `register_child` each child and call `run_layout` then `place_child` per child during layout.
      - Masonry `src/widgets/flex.rs` (`Flex::row/column/with_child/with_gap`, `Flex::add_child`/`remove_child` via `WidgetMut`), `zstack.rs` (`ZStack::with_child/insert_child/remove_child`), `portal.rs` (`Portal::new`).
    - Options Considered:
      - Reconcile into `EditorWidget` directly as child pods: rejected — `EditorWidget` is a leaf that must stay focused on the text canvas; mixing container duties back in recreates the god-object.
      - A dedicated `SduiRegionWidget` container owned by the shell, sibling to the editor: chosen — matches shell-layout-strategy's "Clay-owned container widgets where invariants require it".
      - React-style virtual-DOM differ: rejected — SDUI ops are already coarse-grained; Masonry update passes plus op application suffice.
    - Chosen Approach:
      - Add `SduiRegionWidget` (container) holding `BTreeMap<SduiNodeId, WidgetPod<dyn Widget>>` plus the current `SduiTree`. `reconcile_snapshot(tree)` builds pods for whitelisted kinds; `apply_update(update)` maps each op to add/replace/remove on the pod map (with `base_ui_version` stale-rejection). Layout runs/places children; paint is Masonry's. Non-whitelisted kinds delegate to the existing `SduiNativeState` paint temporarily.
      - REFINEMENT (implemented): the whitelist starts with `label` only (not strictly empty) so the container machinery — heterogeneous `WidgetPod<dyn Widget>` via `NewWidget::new(w).erased().to_pod()`, registration, `run_layout`/`place_child`, `children_ids`, op application — is proven end-to-end against a real Masonry `Label`. The region is NOT yet composed into the shell, so visible rendering is unchanged (zero behavior change). Shell hosting is deferred to the label migration task (task 3) to avoid breaking the shell's `children_ids == [editor]` assertions and to avoid an empty/overlapping child intercepting editor pointer events; `layout` returns `Size::ZERO` while inert as a guard.
    - API Notes and Examples:
      ```rust
      // Masonry container contract (masonry-0.4.0/src/doc/implementing_container_widget.md)
      fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
          for pod in self.pods.values_mut() { ctx.register_child(pod); }
      }
      fn layout(&mut self, ctx: &mut LayoutCtx<'_>, _p: &mut PropertiesMut<'_>, bc: &BoxConstraints) -> Size {
          // for each child: let s = ctx.run_layout(pod, &child_bc); ctx.place_child(pod, pos);
          bc.max()
      }
      // Flex substrate for flex/panel kinds:
      // Flex::column().with_gap(gap).with_child(NewWidget::new(Label::new(text)))
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs` (new module): `SduiRegionWidget`, `reconcile_snapshot`/`apply_update`, `is_reconciled` whitelist, `build_pod`, full `Widget` container impl, 7 unit tests. Created.
      - `src/lib.rs`: register module (`pub(crate) mod masonry_sdui_region;`). Done.
      - `src/masonry_shell.rs`: hosting DEFERRED to task 3 (see Chosen Approach refinement); not edited in this task.
    - References:
      - `src/protocol/sdui.rs` (`SduiTree`, `SduiTreeUpdate`, `SduiTreeOperation`).
      - `src/masonry_sdui.rs:366`/`:377` (`apply_snapshot`/`apply_update` current home).
      - `.agents/skills/project-patterns/references/protocol-and-performance.md` (no IPC/JS in paint).
  - Test Cases to Write:
    - Non-reconciled kinds (panel/flex) produce zero pods and stay on the legacy path; region inert.
    - Reconciled `label` produces exactly one child pod; `children_ids` len 1.
    - `ReplaceNode` updates the pod for a stable node id; `RemoveNode` drops it; version advances.
    - Stale `base_ui_version` rejected with no state/version change.
    - `ReplaceRoot` updates the root binding.
    - Full Masonry container contract runs through `RenderRoot::redraw()` (register/layout/place/paint/a11y) without panic; reconciled child present via `has_widget`.
    - Empty region is inert and claims no space.
  - Completed:
    - `src/masonry_sdui_region.rs` created (module-level `#![allow(dead_code)]` with staging reason, matching `masonry_sdui.rs` precedent); `pub(crate)` struct, no public/JS surface.
    - `cargo test --lib masonry_sdui_region` → 7 passed, 0 failed.
    - `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings` clean.
    - No regression: `cargo test --test editor conformance` → 21 passed.
    - Zero visible behavior change: region not composed into the shell; legacy `SduiNativeState` paint path untouched.

- [x] **(Step 3)** Migrate `flex` and `stack` containers to Masonry `Flex`/`ZStack`
  - Acceptance Criteria:
    - Functional: `flex` (row/column + `gap` token) maps to Masonry `Flex`; `stack` maps to `ZStack`; children reconcile recursively; containers carry no chrome of their own.
    - Performance: Layout computed by Masonry's pass (taffy-backed) instead of manual `cursor_y` stacking; no regression in panel layout/scroll budgets.
    - Code Quality: Manual child-stacking geometry for flex/stack removed from `SduiNativeState`.
    - Security: Containers remain inert layout; no authority change.
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/widgets/flex.rs` (`Flex::row/column/with_gap/with_child`), `src/widgets/zstack.rs` (`ZStack::with_child/insert_child`).
    - Options Considered:
      - Keep manual vertical stacking: rejected — Masonry layout is the point of adopting the substrate.
      - `Flex`/`ZStack` direct mapping: chosen (1:1 with SDUI kinds).
    - Chosen Approach:
      - Recursive reconciliation: container kinds build Masonry containers whose children are the reconciled child pods; `gap` from spacing token.
    - API Notes and Examples:
      ```rust
      Flex::column().with_gap(Length::Pt(gap)).with_child(child_pod)
      ZStack::new().with_child(child_pod, UnitPoint::TOP_LEFT)
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: flex/stack recursive mapping.
      - `src/masonry_sdui.rs`: remove migrated container geometry code.
    - References:
      - `.agents/skills/clay-ui/references/tokens.md` (spacing tokens).
  - Test Cases to Write:
    - Nested flex/stack snapshot parity: child order/geometry matches legacy observable snapshot.
    - `gap` token honored; no chrome on containers.
  - Progress (done — closed after the 6.5 cutover landed):
    - `SduiRegionWidget::build_subtree` (`src/masonry_sdui_region.rs`) reconciles the tree recursively into a nested Masonry subtree — `flex` → `Flex::row/column`, `stack` → `ZStack`, `panel` → `Flex::column` (+ a `panel_title` leaf); leaf kinds map to `SduiLegacyLeaf`. Containers carry no chrome of their own. `SduiTreeUpdate` ops apply then rebuild the subtree (rebuild-on-update, O(n), ponytail-noted). Tests: nested panel>flex>label reconciliation, a settings-shaped all-leaf-kinds tree, and a `RenderRoot::redraw()` no-panic container-contract test; geometry parity proven by `retained_layout_matches_legacy_row_geometry`.
    - Cutover (the former "Remaining" bullet) landed via task 6.5: the retained compositor is the sole render path and the manual `cursor_y` flex/stack stacking (`paint_node`) was deleted, so layout is Masonry's taffy-backed pass and the manual child-stacking geometry is gone from `SduiNativeState`.
    - `gap`: N/A — `SduiNodeKind::Flex { direction, children }` / `Stack { children }` carry **no gap field** in the protocol, and legacy stacked rows with zero inter-row gap, so the hardcoded `with_gap`-less build (gap 0) reproduces it exactly. If a gap surface is ever added to the protocol, wire the spacing token then (ponytail note in `build_subtree`).

- [x] **(Step 4)** Migrate `label` and `statusItem` to retained Masonry `Label`
  - Acceptance Criteria:
    - Functional: `label` and `statusItem` SDUI kinds render through Masonry `Label` children in the reconciled region; text font role, `text.muted` rest color, and `Disabled → text.disabled × opacity.disabled` behavior preserved via typed tokens.
    - Performance: No regression in paint scope; text shaping cached by Masonry/Parley as before.
    - Code Quality: Corresponding hand-painted label/status text code and its manual a11y node construction removed from `SduiNativeState` once the kind is fully served by the retained tree.
    - Security: Styling stays token-driven (`ResolvedUiTheme`); no raw colors introduced outside `primitives.rs`/`theme.rs` (conformance test enforced).
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/widgets/label.rs`: `Label::new(text)`, `with_style(StyleProperty)`, `set_text` via `WidgetMut`.
      - `.agents/skills/clay-ui/references/components.md`: label/statusItem interaction notes; `typography` style variable variants.
    - Options Considered:
      - Keep hand-painted labels, migrate only interactive kinds: rejected — labels are the simplest leaf and prove the seam end-to-end with real text shaping.
      - Masonry `Label` with token-derived `StyleProperty`s: chosen.
    - Chosen Approach:
      - Map `SduiNodeKind::Label`/status items to `Label::new(text)` styled from `TypographyRegistry`/`ResolvedUiTheme`. The structural `label` → plain `Label` mapping already exists (task 2/5 foundation); this task upgrades it to token-styled (font role, `text.muted`, disabled × opacity) and deletes the replaced legacy label paint.
      - REVISED (see Migration Strategy Revision): hosting is now task 6.5 (rendering cutover), not this task. After the cutover, labels are carried by a `SduiLegacyLeaf`; this task swaps that leaf for the real token-styled Masonry `Label` — a layout-neutral change because the leaf reported the same row height. `statusItem` is a package_ui component kind (painted via the package_ui path, not `paint_node`); its migration follows the same leaf-swap once the package_ui panel path is reconciled.
    - API Notes and Examples:
      ```rust
      Label::new(text).with_style(StyleProperty::FontSize(ui_metrics.size))
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: upgrade provisional label `build_pod` to token-styled; add `statusItem` mapping + whitelist entry.
      - `src/masonry_shell.rs`: host `SduiRegionWidget` as a child pod in the left slot (inherited from task 2); update `children_ids` test assertions.
      - `src/masonry_sdui.rs`: remove migrated label/status paint + manual a11y for those kinds.
    - References:
      - `.agents/skills/clay-ui/references/tokens.md` (typography, color roles).
  - Test Cases to Write:
    - Label snapshot parity: retained render produces same observable text/role as legacy snapshot (`SduiObservableSnapshot.label_texts`).
    - Shell now registers the region child: `children_ids` includes editor + region pod (updated assertions).
    - Disabled label: dimmed via token, action gated.
    - Conformance: no raw color literals outside allowed files.
  - Resolution (closed as satisfied-by-leaf, real Masonry `Label` rejected):
    - Finding: Clay renders **all** text parley-direct (`ranged_builder` → `render_text` into the vello `Scene`) — editor buffer, status line, panel-chrome titles. Grep confirms **zero** Masonry `Label`/`Prose`/`VariableLabel` usage in `src/`; the Masonry widgets in use are containers (`Flex`/`ZStack`) plus custom leaves that paint parley-direct.
    - `SduiLegacyLeaf` already renders labels **through the retained Masonry tree** (real `Flex`/`ZStack` layout, real a11y `Role::Label` + accessible name) on the **same parley-direct path as the rest of the app** — i.e. it is consistent with the codebase rendering model, not a stopgap.
    - A real Masonry `Label` would make SDUI labels the only text on a different pipeline (pixel-inconsistency risk: baseline/hinting/line-box), is unverifiable headlessly, deletes no code (`paint_sdui_text` is shared by button/list/editorView leaves and the package-UI paint path), and adds nothing for an inert text node that the leaf lacks.
    - Decision (user-approved): keep the parley-direct leaf for labels; the retained-layout + a11y goal of this task is met. Real Masonry widget swaps are reserved for **interactive** kinds (button/dropdown/modal/textInput) where Masonry's focus/keyboard/action plumbing is the actual payoff. `statusItem` (a package-UI component painted via the package path, not `build_subtree`) follows the same leaf approach until the package-UI panel path is reconciled.

- [x] **(Step 5 — temporary, removed in Step 8)** Rendering cutover: host the reconciled region and move per-node rendering into the retained tree
  - Acceptance Criteria:
    - Functional: `SduiRegionWidget` is composed at the sidebar rect and renders the reconciled subtree; leaf kinds not yet given real Masonry widgets render through a thin legacy-paint leaf widget reporting the exact legacy row size, so the sidebar is pixel-identical to the legacy paint; `SduiNativeState::paint_node` no longer paints kinds owned by the region.
    - Performance: One layout pass; no double paint; scroll/adjacent render within `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`.
    - Code Quality: The shared `cursor_y` flow is replaced by Masonry layout for reconciled subtrees; the legacy-paint leaf is the only remaining immediate-mode island and is the seam tasks 3/4/7 delete kind-by-kind.
    - Security: No package JS/raw CSS in the leaf; actions stay registered-command intents; payload budgets unchanged.
  - Approach:
    - Extract per-node paint + size from `paint_node` into a `SduiLegacyLeaf` widget (reuses `paint_text`/`row_rect`/`interaction_state`/`actions` registration, depth indentation, scroll) so a leaf reports the exact legacy height and paints itself.
    - Host the region where the sidebar geometry already lives (first inside `EditorWidget`, which computes `sdui_panel_left_slot_rect`); shell-ownership of geometry (task 6 remainder) follows once the region provably renders the sidebar.
    - Switch sidebar rendering to the region for reconciled subtrees; keep legacy paint only inside `SduiLegacyLeaf`.
    - ponytail: rebuild-on-update already in place; absolute-position hybrid (option B) rejected as throwaway.
  - Files to Create/Edit:
    - `src/masonry_sdui_region.rs`: `SduiLegacyLeaf` widget; wire leaf kinds to it.
    - `src/masonry_editor.rs`: host the region child at the sidebar rect; stop calling legacy `paint_node` for reconciled subtrees.
    - `src/masonry_sdui.rs`: extract per-node paint/size helpers used by the leaf.
  - Test Cases to Write:
    - Observable-snapshot parity: `label_texts`/`list_items`/package panel rects identical before/after cutover for the settings + markdown fixtures.
    - Sidebar geometry identical (`sdui_panel_left_slot_rect`, `editor_region_for_document`).
    - Conformance + full Linux suite green.
  - Progress (rendering mechanism done + verified this pass; live integration remaining):
    - Done: shared paint extracted behavior-preserving into free fns `paint_sdui_text`/`sdui_row_rect` (`src/masonry_sdui.rs`); `SduiLegacyLeaf` (`src/masonry_sdui_region.rs`) paints `label`/`button`/`list`/`editorView` through those exact helpers and reports the exact legacy row height (`legacy_height()`); reconciler now maps **every** kind to a widget (containers → `Flex`/`ZStack`, leaves → `SduiLegacyLeaf`), so the reconciled subtree is complete and renders identically to legacy at Rest state. Verified: `legacy_leaf_heights_match_the_legacy_cursor_advances`, `settings_shaped_tree_with_all_leaf_kinds_reconciles_and_paints_without_panic` (RenderRoot redraw exercises every leaf paint path), full suite green (lib 1121, editor 149, clippy/fmt clean).
    - Geometry parity PROVEN this pass: `retained_layout_matches_legacy_row_geometry` lays a settings-shaped tree out through a real `RenderRoot` and asserts every retained leaf's Masonry-laid-out height equals the legacy `paint_node` cursor advance (within 1px — Masonry pixel-snaps `QueryCtx::size`, legacy advances fractionally, so exact parity is impossible by design). This caught + fixed a real modeling bug: the panel-title leaf used the `body_text` variant/muted color, but legacy paints panel titles with `title_text` + primary color (`SduiLegacyLeaf::label_presentation`).
    - Remaining (live integration) — approach CORRECTED this pass: the earlier "make `EditorWidget` a container hosting the region child" plan is FLAWED. Masonry paints children in a post-parent pass, but `SduiNativeState::paint` is a manual z-ordered sequence (chrome → clip → `paint_node` tree → `paint_package_overlays`); a region child would paint AFTER `paint_package_overlays`, putting package dropdown/modal overlays UNDER the retained tree (a real regression). The z-order-correct flip is a nested `RenderRoot` compositor inside `SduiNativeState`: render the region via Masonry off-tree, then `Scene::append` its output into `paint()` at the exact `paint_node` point (inside the existing `push_clip_layer(sidebar)` + scroll translation), leaving chrome/overlays/status/`collect_action_regions` hit-testing untouched.
    - Compositor friction found (resolve during the visually-verified step): `RenderRoot` has NO resize API (size only changes via `handle_window_event`, so the off-tree root must be recreated or fed a synthetic resize when the sidebar rect changes); data feed is via `edit_base_layer` + `try_downcast::<SduiRegionWidget>`; the region tree must be kept in sync with `SduiNativeState`'s own tree (still used for hit-testing/overlays/editor-binding); cache the rendered `Scene` and re-render only on data/size change (never per-frame).
    - COMPOSITOR IMPLEMENTED + GATED this pass (default OFF): `RetainedSdui` (`src/masonry_sdui_region.rs`) renders the reconciled tree through a private off-tree `RenderRoot` and caches the `Scene`; `SduiNativeState::paint_retained` (`src/masonry_sdui.rs`) rebuilds hit-test rects + content height via the now-production `collect_action_regions` (un-gated from `#[cfg(test)]`), then `Scene::append`s the retained scene at the exact `paint_node` z-order point (chrome under, `paint_package_overlays` over), translated for panel position + scroll. Gate: default **ON**, opt-out kill-switch `CLAY_SDUI_RETAINED=0`/`false` (`retained_sdui_enabled()`, `OnceLock`-cached). `SduiNativeState` gained a `retained: Option<RetainedSdui>` cache field (manual `Debug`/`Clone`/`PartialEq` \u2014 cache is transient). Verified: `compositor_renders_retained_scene_and_recreates_on_size_change` exercises the full off-tree pipeline; that test caught + fixed a real bug (mutating the region's child tree after the `RenderRoot` register pass \u2014 fix: build the region fully populated before handing it to the root). Full suite green gate-OFF and gate-ON (lib 1123, editor 149; clippy/fmt clean).
    - USER VISUAL CONFIRMATION DONE: with the gate ON the Workspace/folders/files sidebar renders identically to legacy ("works as it worked before"). Two issues seen are PRE-EXISTING, not from the flip: (1) editor text scrollbar thumb stuck/doesn't move \u2014 lives in `editor/surface.rs` `scrollbar_thumb_rect` (untouched by this diff; the SDUI sidebar paints no thumb, the only production `paint_scroll_chrome` is the editor's); (2) workspace sidebar ignores theme (user-confirmed pre-existing). Gate flipped to default ON this pass; full suite green default-ON and opt-out (lib 1123, editor 149; clippy/fmt clean).
    - DONE (this pass): user dogfooded default-ON and confirmed fine; the legacy immediate-mode `paint_node` path (~180 lines) AND the `CLAY_SDUI_RETAINED` gate are DELETED. The retained compositor (`paint_retained` + `RetainedSdui`) is now the ONLY sidebar rendering path; `collect_action_regions` (un-gated from `#[cfg(test)]`) supplies hit-test rects + content height. Shared helpers (`paint_text`/`interaction_state`/`is_focused`/state fills) remain \u2014 still used by the package-component paint path. `masonry_sdui.rs` net \u221285 lines. Full suite green (lib 1123, editor 149; clippy/fmt clean). Task 6.5 CLOSED.
    - FOLLOW-UP (out of scope for 6.5): three reported bugs.
      - ESC DISCONNECT — FIXED: `on_text_event` submitted `EditorAction::ExitRequested` (→ `ctx.exit()`) on every bare Esc, dropping the IPC connection. Fix: Esc now routes through `local_key` (cancels package component / menu / snippet; bare Esc is a no-op), mirroring Enter/Tab. Deleted the now-dead `ExitRequested` variant + its `main.rs` handler (`on_close_requested` already exits directly); `has_active_snippet_session` gated `#[cfg(test)]`. Gate green (lib 1123, editor 149; clippy/fmt clean). Regression guard is compile-time: no exit action exists to wire Esc to.
      - EDITOR SCROLLBAR THUMB (dark in light mode) — FIXED, shared root cause with the sidebar bug. `paint_scroll_chrome` reads `surface.scrollbar`/`surface.scrollbar.track` from `ResolvedUiTheme`, which was built ONLY from `design_tokens` and fell through to the hardcoded DARK core catalog (`surface.scrollbar` core default etc.) whenever a legacy theme shipped no `designTokens`. The editor text went light via a *separate* palette (`BaseUiColors` from `TextThemeOverride`), so chrome and text disagreed. Fix: `ResolvedUiTheme` gained a base-palette layer (`with_base_ui` + `base_color`) that resolves color tokens from the editor's `BaseUiColors` between the design-token overrides and the core catalog; `EditorSurface::set_active_theme` now installs it. The thumb math was always correct (verified earlier) — it only *looked* stuck because a dark thumb on a light track read as a static groove. **Visibility half (second pass):** the base layer fixed the *color source*, but the thumb was still near-invisible because `paint_scroll_chrome` multiplied the theme's already-baked resting alpha (e.g. modus-operandi `#9f9f9faa` = 67%) by `opacity.disabled` (0.5) → ~33% → a faint smudge on a near-white track. No theme ships a `surface.scrollbar` designToken, so the theme's `scrollbar` override alpha *is* the intended resting look; the framework was double-dimming it. Fix: extracted `scrollbar_thumb_paint_color(base, state)` — Rest/Disabled use the theme color verbatim; Hover/Active/Focus lift toward opaque (`apply_alpha(base, 1.5)`, saturating) for perceptible feedback. Regression test `scrollbar_thumb_rest_keeps_theme_alpha_and_hover_lifts` pins rest alpha == theme alpha (no halving) and hover > rest. Gate green (lib 1125, editor 149). Visual confirmation pending.
      - SIDEBAR THEME (dark sidebar on light theme) — FIXED, same root cause. `paint_panel_chrome` / `SduiThemeStyle` read `surface.panel`/`surface.list`/`text.primary`/`text.muted` from `ResolvedUiTheme`, which (no `designTokens`) resolved to the dark core catalog while the editor text was light. The same base-palette layer fixes it. The three SDUI theme-install sites in `masonry_editor.rs` now clone `editor.ui_theme()` onto the SDUI, so the sidebar is *structurally* unable to diverge from the editor theme (single source of truth, no second `from_active_theme` parse). Regression test `base_palette_layers_under_design_tokens_for_legacy_themes` proves: empty design_tokens + light base → shell colors track the base; a design-token override still beats the base; non-color tokens still fall through to the core catalog. Gate green (lib 1124, editor 149; clippy/fmt clean). Visual confirmation: sidebar/workspace now follow the theme (user-confirmed).
      - WINDOW TITLE BAR (dark, ignores OS theme) — NOT a Clay code bug; pending user validation before any change. Investigated: the window is created with `Window::default_attributes().with_title(WINDOW_TITLE)` and **no `with_decorations(...)`**; Clay draws no title-bar widget (root is `ClayShellWidget`). winit 0.30.13 pulls in `sctk-adwaita` (CSD headerbar via smithay-client-toolkit). On Wayland compositors without server-side decorations (GNOME/Mutter) winit draws that CSD headerbar with its own fixed Adwaita look → dark bar + dark/white title text that cannot follow the OS theme. The only Clay-hardcoded piece is the title *string* `WINDOW_TITLE = "Clay Phase 4"` (main.rs:28); the bar + title-text color come from sctk-adwaita. This is the CSD-vs-SSD implementation choice the user asked to validate — see report for options (theme the CSD via winit `set_theme`, force SSD where available, or a custom frameless in-app title bar). No code changed. **Resolved:** user chose to keep CSD as-is; only the hardcoded title string changed `"Clay Phase 4"` → `"Clay"` (main.rs:28). Frame theming (option 1/3) deferred.
    - Verification wall: the compositor is hot-path and **not headlessly verifiable** — the suite checks structure/geometry/observable snapshots (computed from the data tree, not rendered pixels); there is no golden/pixel or SDUI-interaction test. Shared paint code + the proven geometry parity mean pixels match legacy by construction (modulo sub-pixel snap), so the only unverified surfaces are the compositing mechanics (transform/clip/resize/sync). A human must eyeball the running app (glyphs, scroll clip, button hover/focus, chrome + overlay layering) to close this task. Interaction fidelity (hover/active/focus fills, focus ring, action emission) stays on the legacy hit-test path here and migrates per-kind in tasks 3/4/7.

- [x] **(Step 6 — prerequisite for Step 8)** Migrate `panel` and unify slot geometry into one Clay-owned layout
  - Acceptance Criteria:
    - Functional: `panel` renders through `paint_panel_chrome`-backed Masonry composition in the correct fixed slot (`left`/`right`/`top`/`bottom`); the duplicate geometry systems (`sdui_slot_layout`/`combined_slot_layout`/`sdui_panel_slot_layout` in `masonry_sdui.rs` vs. `WorkingAreaLayout` in `masonry_shell.rs`) collapse to a single Clay-owned slot geometry source; panel size stays user-configurable (min/max/collapse/resize).
    - Performance: One geometry computation per layout pass; no double layout; persistence (`~/.config/clay/layout.json`) unchanged.
    - Code Quality: The "temporary compatibility bridge" called out in `shell-layout-strategy.md` is removed; `FixedSlotState`/`PaneSlotLayout` remain the single source.
    - Security: Slot ownership/visibility/collapse validation in `src/shell/layout.rs` unchanged.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/shell-layout-strategy.md`: working area → pane/split tree → slots; Phase 20.1 token-backed panel defaults (`ResolvedUiTheme::panel_defaults()`).
      - `src/shell/layout.rs` (`PaneSlotLayout`, `FixedSlotState`), `src/shell/theme.rs` (`panel_defaults`).
    - Options Considered:
      - Leave the two geometry systems bridged: rejected — it is the documented temporary state and a standing source of overlap bugs (e.g. `editor_region_for_document` reserving the left slot by panel presence).
      - Move panel placement under the shell's `WorkingAreaLayout`, SDUI region fills slot content: chosen.
    - Chosen Approach:
      - The shell computes slot geometry; `SduiRegionWidget` is placed in fixed slots and reconciles panel content inside them; delete the SDUI-side parallel slot helpers.
    - API Notes and Examples:
      ```rust
      // single source: shell WorkingAreaLayout -> slot rects -> place SduiRegionWidget pods
      ```
    - Files to Create/Edit:
      - `src/masonry_shell.rs`: place SDUI region pods in computed slots.
      - `src/masonry_sdui.rs`: remove `sdui_slot_layout`/`combined_slot_layout`/`sdui_panel_slot_layout`/`fixed_sdui_left_slot` bridging.
      - `src/masonry_editor.rs`: `editor_main_rect` reads unified geometry.
    - References:
      - `src/masonry_sdui.rs` (`editor_region_for_document`, slot helpers), `src/shell/layout.rs`.
  - Test Cases to Write:
    - Editor main region identical across migration for representative panel layouts.
    - Slot resize/collapse/persistence still works; no geometry double-compute.
  - Progress (DONE):
    - Done (prior pass): SDUI-side slot geometry deduplicated to one mechanical source — `with_default_left_slot(layout, defaults, want_left)` in `src/masonry_sdui.rs`; `editor_region_for_document`, `sdui_slot_layout`, and `sdui_panel_slot_layout` share it (each keeps its intentional gate).
    - Done (this pass): deleted the last residual redundancy — `combined_slot_layout` was a pure alias of `sdui_slot_layout`; removed it and inlined `sdui_slot_layout` at its 3 call sites (`src/masonry_sdui.rs`). Behavior-preserving: fmt + clippy `-D warnings` clean, lib 1125 + editor 149 green (all `editor_region_*`/`slot_panel_contribution_*`/`transient_overlay_*`/`workspace_browser_reserves_left_slot_*` geometry tests pass).
    - **Resolution — the literal "collapse the two geometry systems into one source" criterion is reclassified after investigation. The two systems are a correct concern separation, not a duplication:**
      - Shell `WorkingAreaLayout.pane_slots` (`src/shell/layout.rs`) is fed by the server `WorkingAreaLayoutUpdate` protocol, is **persisted** (`~/.config/clay/layout.json`) and **user-resizable/collapsible** (`resize_slot_live`/`commit_slot_resize`). In the default `single_editor()` config it is `main_only()` (no slots) and returns the full pane.
      - The SDUI sidebar geometry is **package/theme-driven transient** state: `package_ui.slot_layout(&defaults)` (from package-contributed fixed panels, `src/shell/package_ui.rs:275`) plus the Clay-owned default left slot (`fixed_sdui_left_slot` from `panel_defaults()`), gated on `root_id`/`editor_binding`/width. It is never persisted and not user-resizable (`min == max == sidebar_width`).
      - They operate at different layers without conflict: the shell places the `EditorWidget` child at the pane; `EditorWidget` carves its sidebar internally via `editor_main_rect`. Folding the theme-driven transient sidebar into the persisted user-slot model would violate the "persistence unchanged" criterion.
    - The three SDUI gate entry points encode genuinely different questions and were left intact: `editor_region` (binding+width, observation), `editor_region_for_document`/`editor_main_rect` (root-or-binding+width), `sdui_panel_slot_layout`/sidebar rect (root-only, no width guard). Merging them risks a behavior change that is not headlessly verifiable.
    - **Folded into Step 8:** "the shell places `SduiRegionWidget` in fixed slots" is hosting work (visual + z-order), not a geometry refactor. The SDUI-side helpers remain the package/theme geometry source until Step 8/12 retire them with the god-object.
    - Note: the three gates differ intentionally (editor main reserves on root-or-binding + width guard; panel sidebar on root only). Unifying the *gates* (not just the mechanics) is an edge-case behavior change needing a product decision; deliberately not done here.

- [x] **(Step 7)** Z-order de-risk spike: prove the region as a real child can sit below package overlays and above editor chrome
  - Acceptance Criteria:
    - Functional: A throwaway proof (not production code) demonstrates that hosting `SduiRegionWidget` as a real child of the main tree can reproduce the compositor's z-order — tree painted **above** editor chrome and **below** package transient overlays — using Masonry `ZStack` (ordered children) and/or `Portal` (overlay layer). Identifies concretely which widget hosts the stack and where overlays attach.
    - Performance: N/A (spike).
    - Code Quality: No production change; findings recorded in this plan (chosen composition + any blocker). This is the gate for Step 8.
    - Security: No authority change.
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/widgets/zstack.rs` (`ZStack::with_child` paints children in order, later = on top), `src/widgets/portal.rs` (`Portal::new`, layer hoisting), `src/doc/implementing_container_widget.md`.
      - `src/masonry_sdui.rs` `paint`/`paint_retained`: current z-order is package fixed panels (under) → panel chrome (under tree) → tree → package overlays (over); `EditorWidget::paint` is bg → editor text → `sdui.paint()` → status line.
    - Options Considered:
      - `ZStack` at the shell/editor level ordering `[editor chrome, region, overlays]`: candidate.
      - `Portal` hoisting overlays to a top layer above the region: candidate.
      - Keep the compositor: the fallback if neither composes cleanly.
    - Chosen Approach:
      - Build a minimal `RenderRoot` proof placing a region child + an overlay child in a `ZStack`/`Portal` and confirm paint order + pointer routing to the region child. Record the composition that works.
    - Files to Create/Edit:
      - Scratch/throwaway only; this plan updated with the outcome and the gate decision for Step 8.
    - References:
      - `src/masonry_editor.rs:2646` (`EditorWidget::paint` z-order), `src/masonry_sdui.rs` (`paint_retained`).
  - Test Cases to Write:
    - Spike-only assertions (paint order / region receives a pointer event); not committed as production tests unless reusable.
  - Findings & gate decision (DONE — gate PASSED):
    - **Paint order is authoritative** (masonry_core 0.4.0 `src/passes/paint.rs::paint_widget`): a widget appends its own `paint()` first, then its children in `children_ids` order, then its `post_paint()`. Hit-testing walks the same tree front-first (`core/widget.rs`: "the last child as determined by `children_ids` is chosen"), so **hit-test order == paint order**. A runtime hit-test probe therefore stands in for paint z-order (no pixel/golden test needed — consistent with the verification wall).
    - **`ZStack`** stacks children back-to-front in insertion order (later = on top). Proven by `spike_zstack_child_order_is_z_order_and_routes_pointer_to_top` (`src/masonry_sdui_region.rs`): with `[chrome, region, overlay]` the overlay (last) is topmost; with `[chrome, region]` the region is topmost; the pointer routes to the top child.
    - **`post_paint()`** paints above all children and adds no widget, so it never intercepts pointers. Proven by `spike_region_child_receives_pointer_under_post_paint_overlay`: a real `SduiRegionWidget` child receives a pointer probe even under a full-area `post_paint()` overlay, with `paint()` chrome below.
    - **`Portal` correction:** `Portal` in Masonry 0.4.0 is a **scroll viewport** (`viewport_pos`/scrollbars/`pan_viewport`), NOT a z-layer hoist. It is the mechanism for Step 10 (list/scroll), not overlay z-order. The pre-spike "Portal overlay layer" option is withdrawn.
    - **Gate decision — Step 8 uses "Composition B":** `EditorWidget` becomes a container; editor chrome (bg / editor text / panel borders) in `paint()` (below children); the reconciled `SduiRegionWidget` as the sole child (middle); package transient overlays in `post_paint()` (above children). Legacy hit-test (`collect_action_regions`/`action_for_point`) is retained, so overlays need no widget hit-test yet. This reproduces the compositor's chrome < tree < overlay z-order with the region as a live event-tree child. No `ZStack`/`Portal` needed for the cutover.
    - **Reserved for Step 13e (Composition A):** when overlays become real interactive widgets, move them to a later `ZStack` child so they paint AND hit-test above the region. Caveat proven relevant: Masonry hit-tests by widget **bounding rect**, so a full-size overlay child would block the region everywhere — overlay children must be sized to their rect (or stashed when absent).
    - Two reusable regression tests committed (not throwaway): the two `spike_*` tests above. Gate green: fmt + clippy `-D warnings` clean, lib 1127 (+2) / editor 149.

- [x] **(Step 8)** Structural enabler: host the reconciled region as a real child, solve z-order, delete the nested compositor
  - Acceptance Criteria:
    - Functional: `SduiRegionWidget` is a live child of the main widget tree (hosted by `EditorWidget`/`ClayShellWidget` in the computed slot), painted in the correct z-order via the composition proven in Step 7; the nested `RetainedSdui` compositor (`Scene::append`, recreate-on-dirty) is **removed**. Leaves not yet given real widgets remain `SduiLegacyLeaf` children; the legacy hit-test path (`collect_action_regions`/`action_for_point`) is retained temporarily so buttons stay clickable until Step 9.
    - Performance: Editor text canvas hot path unchanged; one render pass (no nested `RenderRoot`); no per-frame allocation proportional to node count.
    - Code Quality: `EditorWidget` becomes a container that hosts the region child; `RetainedSdui` and its `Scene`/`rendered_size`/`dirty` machinery deleted; shell `children_ids` test assertions updated.
    - Security: Authority boundary unchanged; no package JS in paint/layout/input.
  - Approach:
    - Documentation Reviewed:
      - Step 7 outcome — **Composition B**: chrome in `paint()` (below), region as the sole child (middle), package overlays in `post_paint()` (above); no `ZStack`/`Portal` needed for the cutover (`Portal` is a scroll viewport, reserved for Step 10).
      - `src/shell/layout.rs` (`WorkingAreaLayout`, `PaneSlotLayout`, `FixedSlotState`) for slot placement of the region child.
    - Options Considered:
      - Host region as a child of `EditorWidget` vs. `ClayShellWidget`: decided by Step 7 (whichever gives correct z-order with overlays/status).
      - Keep the compositor and bridge events into it: rejected — throwaway plumbing that Step 14 would delete; the region-as-child is the durable target.
    - Chosen Approach:
      - Using the unified SDUI package/theme slot geometry (Step 6 source) → place `SduiRegionWidget` pod in the sidebar slot → compose overlays/status per Step 7 → delete `RetainedSdui`. `SduiNativeState` keeps data + legacy hit-test for now; its `paint` no longer composites. (Step 8 absorbs the "shell places the region in fixed slots" work reclassified out of Step 6.)
    - API Notes and Examples:
      ```rust
      // EditorWidget registers the region pod as its sole child; layout runs/places it in the slot.
      // paint() = chrome (below child); post_paint() = package overlays (above child). RetainedSdui deleted.
      ```
    - Files to Create/Edit:
      - `src/masonry_editor.rs` / `src/masonry_shell.rs`: host + place the region child; container conversion; update `children_ids` tests.
      - `src/masonry_sdui_region.rs`: remove `RetainedSdui` + `render_root_with`; region becomes a normal child.
      - `src/masonry_sdui.rs`: `paint` stops compositing (legacy hit-test collection stays until Step 9).
    - References:
      - `src/masonry_sdui_region.rs` (`RetainedSdui`), `src/masonry_editor.rs:2646`, `src/shell/layout.rs`.
  - Test Cases to Write:
    - Region is a registered child of the main tree (`children_ids` includes it); full snapshot renders through the live tree with no nested `RenderRoot`.
    - Geometry parity: sidebar/editor rects identical to the compositor era (`editor_region_for_document` tests).
    - Rendering parity (manual visual): z-order (chrome < tree < overlays), scroll clip, theme, button still clickable via the retained legacy hit-test.
  - Progress (DONE — pending manual visual gate):
    - **Hosted as a real child:** `EditorWidget` is now a container whose sole child is a `WidgetPod<dyn Widget>` holding `SduiRegionWidget`. `register_children`/`children_ids` register/report it; `layout` runs/places it at the sidebar slot. The dead `impl Widget for SduiNativeState` is deleted.
    - **Z-order (Composition B):** `EditorWidget::paint` paints bg + editor text + `sdui.paint_chrome` (fixed panels + panel chrome) BELOW the child; the region child paints the tree; `EditorWidget::post_paint` paints `sdui.paint_overlays` + the status line ABOVE. Order is identical to the compositor era (bg, editor, fixed panels, panel chrome, tree, overlays, status).
    - **Scroll + clip:** scroll is baked into the region's placement origin (`sidebar.y0 + padding - scroll_offset`, the compositor's exact translation); `LayoutCtx::set_clip_path(sidebar)` clips the over-tall content to the viewport for paint AND pointer. Sidebar scroll now calls `request_layout` (re-places the child) instead of render-only.
    - **Data feed:** the compositor-dirty flag became `region_dirty` (set by `set_typography`/`set_ui_theme`/`apply_snapshot`/`apply_update`). `EditorWidget::sync_region(&mut MutateCtx)` rebuilds the region wholesale when dirty — take old pod → reconcile fresh → `ctx.remove_child(old)` → `ctx.children_changed()` — called from the three `main.rs` paths that apply a `ClientConnectionEvent` (the `ClientConnection` action arm, the `ConnectionEvent` UI-command arm, and dialog completion). Wholesale rebuild is fine while leaves are inert (no state to lose); stable-identity incremental reconcile is now its own stage (Step 11), gating the stateful kinds (Steps 12–13).
    - **Compositor deleted:** `RetainedSdui`, `render_root_with`, `ensure_rendered`, the nested `RenderRoot`/`Scene::append`, and `SduiNativeState::paint`/`paint_retained` are gone. `paint` split into `paint_chrome`/`paint_overlays`; geometry/actions moved to `sidebar_geometry` (called from layout). One render pass.
    - **Clicks:** legacy hit-test retained per plan — a pointer event targets a region leaf and bubbles to `EditorWidget` (Masonry bubbles target→ancestors; the event position is window-space and `EditorWidget` sits at the window origin, so `action_for_point` is unchanged).
    - **De-risked headlessly:** Step 7 spikes (z-order + pointer routing) plus a new `spike_persistent_region_rebuilds_across_redraws_without_panic` (child lifecycle: `remove_child` + `children_changed` across redraws). Gate green: fmt + clippy `-D warnings` clean; lib 1126 / main 34 / editor 149 / runtime 196; only the two pre-existing failures remain (security `package_manifest_rejects_invalid_slot_ui_contribution_metadata`, protocol `plan061_...rebaseline`).
    - **VISUAL GATE (needs eyeball):** z-order (chrome < tree < overlays), scroll clip + thumb movement, theme parity (light/dark sidebar), and button/list clicks via the retained legacy hit-test. Headless tests prove geometry/lifecycle/z-order mechanism, not final pixels.

- [x] **(Step 9 — requires Step 8)** Migrate `button` to retained Masonry `Button`; route events to the region; delete legacy button hit-test
  - v2 note: this is the first real interactive widget and establishes the event-routing mechanism (window pointer/key events → the live region tree; widget action → `enqueue_sdui_action`). It deletes the legacy button hit-test (`collect_action_regions` button arm / `action_for_point`) once the real `Button` handles its own events. The legacy hit-test was retained in Step 8 only to keep buttons clickable during the structural change.
  - Acceptance Criteria:
    - Functional: `button` renders as Masonry `Button` with variants `default`/`muted`/`primary`/`danger`; all five interaction states (`Rest`/`Hover`/`Active`/`Focus`/`Disabled`) styled from tokens; activation emits the same inert `SduiActionIntent` through the existing server-first command path.
    - Performance: Pointer/focus handled by Masonry passes; no per-frame hit-test allocation for buttons.
    - Code Quality: Manual button rect hit-testing and `SduiVisibleAction` bookkeeping for buttons removed once served by the retained tree.
    - Security: Action intents carry only registered command id + bounded primitive args; no callback/op/native handle; disabled gates the action.
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/widgets/button.rs`: `Button::with_text(text)`, `Button::new(child)`, action submission to the app driver.
      - `.agents/skills/clay-ui/references/components.md`: button variant/state notes (`component_state_color("surface.control", state)`, focus ring on `Focus`).
    - Options Considered:
      - Custom button widget: rejected — Masonry `Button` gives press/focus/action plumbing free.
      - Masonry `Button` + Clay token-backed properties: chosen.
    - Chosen Approach:
      - Map `SduiNodeKind::Button` to `Button`, wire its action to enqueue the existing `SduiActionIntent`; style via typed properties from `ResolvedUiTheme`; add to whitelist; delete replaced hit-test/paint.
    - API Notes and Examples:
      ```rust
      Button::with_text(label) // action routed to enqueue_sdui_action(intent)
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: button mapping, action wiring.
      - `src/masonry_editor.rs`/`src/client/mod.rs`: route Masonry button action to `enqueue_sdui_action` (reuse existing path).
      - `src/masonry_sdui.rs`: remove migrated button paint/hit-test.
    - References:
      - `src/client/mod.rs:323` (`enqueue_sdui_action`), `src/protocol/sdui.rs` (`SduiActionIntent`).
  - Test Cases to Write:
    - Button action emits identical server intent as legacy (`sdui_button_action_emits_server_intent` parity).
    - Disabled button: no action emitted.
    - Focus ring/variant styling from tokens (conformance).
  - Progress (DONE — pending manual visual gate):
    - **Deviation from the stock `Button` (documented, deliberate).** The plan's "Masonry `Button` + token properties" was rejected during implementation for two hard reasons: (1) the stock `Button`'s `ButtonPress` action carries **no payload**, so routing to an SDUI intent would need a `widget_id → intent` side channel that the wholesale region rebuild (Step 8) would have to republish on every update; (2) its property set has **no hovered/focus background** (only `HoveredBorderColor`), so it cannot reproduce Clay's per-state `surface.control` fills. Instead `SduiButton` (in `masonry_sdui_region.rs`) is a thin retained Masonry widget that reuses Masonry's pointer/focus/keyboard plumbing — the *same* `ctx` calls as the stock `Button` (`capture_pointer` on press, submit on release-while-hovered, Enter/Space when focused, click focus via `accepts_focus`, `Role::Button` + `accesskit::Action::Click`) — but paints through the legacy shared helpers (`sdui_row_rect`/`component_state_color`/`paint_sdui_text`/`paint_focus_ring`) for pixel parity and carries the intent in its action. No hand-rolled hit-test: Masonry still hit-tests the bounding rect and dispatches events.
    - **Event routing established (the Step 9 mechanism).** `SduiButton::type Action = SduiButtonPress { intent }` (the node's inert `SduiActionIntent`, `pub`, re-exported via `masonry_editor` so the binary driver can name it without exposing the `pub(crate)` region module). `Driver::on_action` downcasts `SduiButtonPress` *before* `EditorAction`, targets the editor via `editor_action_target`, and calls the new `EditorWidget::enqueue_sdui_button_action(intent)` → `edit_queue.enqueue_sdui_action(ui_version, intent)` — the identical server-first path the legacy click used.
    - **Legacy button hit-test deleted.** The `collect_action_regions` button arm no longer pushes a `SduiVisibleAction` (the cursor still advances by `button_height` so content-height/scroll-bounds stay pixel-identical to the reconciled layout). Deleting it is *required*, not just cleanup: a click otherwise double-enqueues (legacy on `Down` via the bubbled `action_for_point` + Masonry on `Up`). `action_for_point` stays for not-yet-migrated kinds (list) and package-UI.
    - **Five states from tokens:** `Rest`/`Hover`/`Active`/`Focus`/`Disabled` all derive `component_state_color("surface.control", state)` from Masonry-tracked state (`is_disabled > is_active > is_focus_target > is_hovered > Rest`); `Focus` adds `paint_focus_ring`; `Disabled` swaps to `disabled_text_color`. `update()` repaints on hover/active/focus/disabled changes.
    - **Deferred (protocol can't express yet):** the `default`/`muted`/`primary`/`danger` *variants* and the *disabled gate* — `SduiNodeKind::Button` is only `{ label, action }` (no variant/disabled field), so the default `surface.control` look is rendered and a button is never disabled. Both await a protocol field; the widget's state machine already handles them.
    - **`SduiLegacyLeaf`'s button arm** is now unreachable (buttons map to `SduiButton`); it is deleted with the whole leaf in Step 14 rather than churned now.
    - **Tests:** `masonry_sdui_region::sdui_button_action_emits_server_intent` drives a real `RenderRoot` click (Move→Down→Up) and asserts exactly one `SduiButtonPress` carrying the node's exact intent. The two obsolete legacy-button geometry tests were rewritten: `sdui_button_is_served_by_retained_widget_not_legacy_hit_test` (button is no longer a legacy rect; intent preserved on the node) and `ui_size_change_scales_row_hit_and_accessibility_bounds_together` (repointed to an actionable list row via a new `actionable_list_tree`, since the button is no longer legacy). Gate green: fmt + clippy `-D warnings` clean; lib 1127 / editor 149 / runtime 196; only the two pre-existing failures remain.
    - **VISUAL GATE (needs eyeball):** button hover/active/focus fills + focus ring, click activates the command, keyboard (Tab focus + Enter/Space) activation, and no double-fire. Headless test proves action-intent parity + routing, not final pixels.

- [x] **(Step 10 — requires Step 8)** Migrate `list` to retained Masonry row widgets; route row action; delete legacy list hit-test (scroll split out to **Step 12**, gated on the **Step 11** reconciler)
  - Acceptance Criteria:
    - Functional: `list` renders rows (title + detail, per-row interaction states) as retained children; row activation emits the same inert intent.
    - Performance: a row state change repaints that row, not the whole list; row paint reuses the shared parley helpers.
    - Code Quality: legacy list hit-test removed from `collect_action_regions`; `SduiListRow` mirrors the `SduiButton` event pattern (capture-on-press, submit-on-release-while-hovered, Enter/Space when focused).
    - Security: Row actions remain registered-command intents; no authority change.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/references/components.md`: list row fill (`list_row_fill_color(state, selected)`).
      - Masonry `src/widgets/button.rs`: pointer/focus/keyboard plumbing reused by `SduiListRow`.
    - Options Considered:
      - One `SduiLegacyLeaf` per list (former behavior): rejected — no per-row interaction or focus.
      - `Flex::column` of `SduiListRow` widgets: chosen — per-row hover/active/focus via real Masonry events.
    - Chosen Approach:
      - Map `list` to a `Flex::column` of `SduiListRow` widgets; reuse row styling tokens; emit `SduiListRowPress { intent }`.
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: `SduiListRow` + `SduiListRowPress`, list mapping.
      - `src/masonry_editor.rs`: `enqueue_sdui_intent` + re-export.
      - `src/main.rs`: `route_sdui_intent`.
      - `src/masonry_sdui.rs`: delete the legacy list hit-test arm.
  - Test Cases to Write:
    - List-row action-intent parity: a `RenderRoot` click emits the item's exact intent.
    - Geometry parity: the list contributes one leaf height per row.
  - Progress (DONE — pending manual visual gate; scroll moved to Step 12):
    - **10a — list → retained rows (done).** `SduiNodeKind::List` now reconciles to a `Flex::column` of `SduiListRow` widgets (one per item) instead of a single `SduiLegacyLeaf`. `SduiListRow` mirrors `SduiButton`: it reuses Masonry's pointer/focus/keyboard plumbing (capture-on-press, submit-on-release-while-hovered, Enter/Space when focused, `Role::ListItem` + a11y `Click`) and paints through the legacy shared helpers (`sdui_row_rect`/`list_row_fill_color`/`paint_sdui_text`) for pixel parity at Rest, while adding per-row `Hover`/`Active`/`Focus` fills (`list_row_fill_color(state, …)` gives `surface.hover`/`surface.active`) + a focus ring. Rows without an action are inert (no focus, no activation) but still repaint on hover. `SduiListItem` has no `selected` field, so the `selected` fill stays `false` (selection deferred pending a protocol field, like button variants).
    - **10a — routing.** `SduiListRowPress { intent }` (pub, re-exported via `masonry_editor`) carries the item's inert intent. `Driver::on_action` downcasts `SduiButtonPress` then `SduiListRowPress`, both routing through a new shared `Driver::route_sdui_intent` helper → `EditorWidget::enqueue_sdui_intent` (renamed from `enqueue_sdui_button_action`, now generic for button + list row + future transient surfaces).
    - **10a — legacy list hit-test deleted.** The `collect_action_regions` list arm no longer pushes action rects (the cursor still advances by the full list height for content-height/scroll-bounds parity). Deleting it is required to avoid double-fire (legacy on `Down` + Masonry on `Up`). With button (step 9) + list rows now Masonry widgets, **no actionable SDUI element remains in the legacy hit-test** — `collect_action_regions` now only walks the cursor for content height, so its dead `depth`/`width`/`origin_x` params were removed (clippy `only_used_in_recursion`). Package-UI actions are a separate walker, unaffected.
    - **10a — tests:** `masonry_sdui_region::sdui_list_row_action_emits_server_intent` drives a real `RenderRoot` click and asserts exactly one `SduiListRowPress` carrying the item's exact intent. `retained_layout_matches_legacy_row_geometry` updated (the list now contributes one leaf height *per row*). Two obsolete legacy-hit-test tests deleted (`file_browser_scrolled_action_hits_visible_row`, `ui_size_change_scales_row_hit_and_accessibility_bounds_together`) plus the now-dead `actionable_list_tree` helper; the scroll-*math* and scroll-*routing* tests (`file_browser_scroll_reveals_later_rows…`, `scrolls_point_routes…`) remain green. Gate green: fmt + clippy `-D warnings` clean; lib 1126 / editor 149 / runtime 196; only the two pre-existing failures remain.
    - **10a — VISUAL GATE (needs eyeball):** per-row hover/active/focus fills + focus ring, click activates the row's command once, keyboard (Tab focus + Enter/Space) activation, inert rows don't activate, and no double-fire.
    - **scroll → `Portal` (now Step 12, unblocked by the Step 11 reconciler).** Originally deferred here as "10b" because the wholesale region rebuild reset the `Portal`'s `viewport_pos` on every SDUI update. The full deferral reasoning and its resolution live in Constraint 3, Steps 11–12, and `decision-logs/2026-07-29-1451-stable-identity-sdui-reconciliation.md`.

- [x] **(Step 11a — requires Step 10)** Reconciler de-risk spike: prove a stateful widget survives a region sync via in-place identity preservation
  - Acceptance Criteria:
    - Functional: a throwaway stateful test widget (holds a counter/scroll offset) keeps its state across a reconcile update that reuses its `WidgetId`, and loses it under the current wholesale rebuild (demonstrating the difference).
    - Performance: spike-only; no production perf bar.
    - Code Quality: disposable proof, not production code; records which Masonry APIs cooperate (`edit_widget`, `get_raw_mut`, child-list mutation, `NewWidget::with_id`) and which fight back.
    - Security: no authority change.
  - Approach:
    - Documentation Reviewed:
      - Masonry `core/widget.rs` (`WidgetId`, focus by id), `core/contexts.rs` (`MutateCtx::get_raw_mut`, `remove_child`, `children_changed`), `widgets/portal.rs` (`viewport_pos`).
      - `decision-logs/2026-07-29-1451-stable-identity-sdui-reconciliation.md`.
    - Options Considered:
      - Skip the spike, build the reconciler directly: rejected — in-place mutation of the live widget graph has known gotchas (post-construction child-registration panics were hit in Step 8); prove the mechanism cheaply first.
      - Spike with a real `Portal` vs. a minimal custom stateful widget: the minimal widget isolates the identity question without Portal's scroll complexity (use Portal only if the minimal spike is inconclusive).
    - Chosen Approach:
      - Build a minimal stateful widget in a test; reconcile a tree twice (second pass reusing the node's `WidgetId`); assert state survives; contrast with a wholesale rebuild resetting it.
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: test-only spike widgets + `spike_*` tests (disposable).
  - Test Cases to Write:
    - `spike_stateful_widget_survives_inplace_reconcile`: state intact when the pod/`WidgetId` is reused.
    - `spike_wholesale_rebuild_resets_state`: state lost when the widget is recreated (documents the bug being fixed).
  - Progress (DONE):
    - Two committed tests in `src/masonry_sdui_region.rs`: `spike_stateful_widget_survives_inplace_reconcile` (reusing the pod keeps the counter at 5 and the `WidgetId` stable across a reconcile + redraw, no panic) and `spike_wholesale_rebuild_resets_state` (a fresh pod mints a new `WidgetId` and discards the counter — the bug Step 11 fixes). Test-only widgets `SpikeStateful` (stateful leaf) + `ReconcileSpikeHost` (parent mirroring `SduiRegionWidget`) kept as regression guards, matching the repo's convention of retaining `spike_*` tests.
    - **APIs that cooperate:** `MutateCtx::get_mut(&mut pod) -> WidgetMut<W>` mutates a *registered* child in place with no `children_changed` and no register-pass panic — this is the reconciler's core primitive. Driving it via `RenderRoot::edit_widget(host_id, |mut w| { let mut host = w.try_downcast::<T>().expect(…); host.widget.reconcile(&mut host.ctx, …) })` works exactly like the production `sync_region` path.
    - **Constraints found:** `get_mut` does `children.item_mut(id).expect("child not found")`, so in-place mutation only works on *already-registered* children — added nodes still need `to_pod()` + `children_changed` (already proven panic-free by `spike_persistent_region_rebuilds_across_redraws_without_panic`). `get_raw_mut`/`AllowRawMut` was *not* needed: a typed `WidgetPod<SpikeStateful>` downcasts via the blanket `impl<T: Widget> FromDynWidget for T`. `NewWidget::with_id` was not tested (the chosen approach reuses pods, not ids on fresh pods).
    - Gate green: fmt + clippy `-D warnings` clean; lib 1128 / editor 149 / runtime 196.

- [x] **(Step 11b — requires Step 11a)** Stable-identity reconciler core: diff the SDUI tree and update the Masonry subtree in place
  - Acceptance Criteria:
    - Functional: `SduiRegionWidget` keeps a `SduiNodeId → WidgetPod` map; on `reconcile_snapshot`/`apply_update`, surviving nodes reuse their pod (same `WidgetId`), added nodes create pods, removed nodes are destroyed; changed props (label text, action, theme) update in place; `Flex`/`ZStack` child lists mutate (add/remove/reorder) to match the tree.
    - Performance: an update that changes one node does not recreate unrelated subtrees; reconcile cost scales with changed nodes, not tree size.
    - Code Quality: the diff reuses the existing incremental `SduiTreeOperation`s; no per-kind special-casing beyond prop mapping; container child-list mutation uses Masonry's documented child-mutation API.
    - Security: no authority change; reconciled widgets remain inert declarations.
  - Approach:
    - Documentation Reviewed:
      - `src/protocol/sdui.rs` (`SduiNodeId`, `SduiTreeOperation::ReplaceNode`/`RemoveNode`).
      - Masonry child-mutation API (`MutateCtx::remove_child`, `children_changed`, `get_raw_mut`); spike findings from Step 11a.
    - Options Considered:
      - Recreate the whole subtree but reassign stable `WidgetId`s via `NewWidget::with_id`: only viable if Masonry treats a reused id on a fresh pod as the same widget (state kept) — confirm in the spike; otherwise rejected.
      - True in-place diff with a `SduiNodeId → WidgetPod` map: chosen — widgets are never recreated, so Masonry keeps their state.
    - Chosen Approach:
      - Maintain the id→pod map; walk the incoming tree; reuse/create/destroy pods; push prop changes through `get_raw_mut`/`edit_widget`; mutate container child lists.
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: id→pod map + diff + in-place prop update + container child mutation.
  - Test Cases to Write:
    - Identity preservation: a node present before and after an `apply_update` keeps the same `WidgetId`.
    - State preservation: a stateful widget's state survives an unrelated tree update.
    - Container mutation: add/remove/reorder children of a `Flex`/`ZStack` reconciles correctly.
    - Prop update: changing a label's text updates in place without recreating the widget.
  - Progress (DONE):
    - `SduiRegionWidget` gained two maps: `pods: BTreeMap<SduiNodeId, PodRecord>` (node → stable `WidgetId` + built-kind discriminant) and `child_keys: BTreeMap<SduiNodeId, Vec<ChildKey>>` (per-container ordered child keys, parallel to the `Flex`/`ZStack` children vec; `ChildKey` covers real nodes plus the synthetic panel-title and list-row children). `build_node` (the fresh builder, renamed from `build_subtree`) populates both maps as it builds.
    - Two reconcile paths share the data-update helpers: the no-ctx `reconcile_snapshot`/`apply_update` still rebuild wholesale (standalone tests + the current `sync_region` swap, both unchanged); the new ctx-taking `reconcile_snapshot_live`/`apply_update_live` run `reconcile(ctx)`, which reuses the root pod when its kind is unchanged and recurses via `reconcile_node`. Production wiring of the `_live` path is Step 11c (the module's `#![allow(dead_code)]` covers the not-yet-wired methods).
    - **`Flex` children** reconcile incrementally (`reconcile_flex_children`): same-order survivors mutate in place via `Flex::child_mut`; added keys `Flex::insert_child`; removed keys `Flex::remove_child`. **Ceiling:** Masonry's `Flex` has no move-child API, so a *reordered* survivor is removed + re-inserted fresh (identity lost); pure add/remove preserves every survivor's identity.
    - **`ZStack` children** (`reconcile_zstack_children`): Masonry's `ZStack` only appends (`insert_child` has no index), so same-order survivors reconcile in place but any structural change rebuilds the stack's children wholesale. `stack` is rare in SDUI, so this stays simple.
    - Prop updates mutate leaf fields in place through `WidgetMut` (same-module field access): `SduiLegacyLeaf.{kind,depth,typography,ui_theme}`, `SduiButton.{label,intent,…}`, `SduiListRow.{label,detail,action,…}`, plus the synthetic panel-title/list-row leaves from their parent node; each requests layout. `gc` drops identity records for nodes unreachable from the root after each reconcile.
    - Four committed tests (driven through a live `RenderRoot` via `edit_widget`): `stable_identity_nodes_keep_widget_ids_across_inplace_update`, `stable_identity_preserves_focus_across_unrelated_update` (focus is keyed by `WidgetId`, so a stable id keeps focus — a wholesale rebuild would drop it), `container_child_list_add_remove_reorder_reconciles_correctly`, `prop_update_changes_label_text_without_recreating_the_widget`.
    - Gate green: fmt + clippy `-D warnings` clean; lib 1132 / editor 149 / runtime 196; existing standalone reconcile tests unchanged.

- [x] **(Step 11c — requires Step 11b)** Production switchover: feed the live region by in-place reconcile; delete the wholesale rebuild
  - Acceptance Criteria:
    - Functional: `EditorWidget::sync_region` (and the `main.rs` update sites) drive the persistent region via in-place reconcile; the wholesale `std::mem::replace` + `remove_child` + `children_changed` rebuild is deleted; button/list focus survives server SDUI updates (and scroll once Step 12 lands).
    - Performance: a server SDUI update reconciles in place; no full subtree teardown/re-register per update.
    - Code Quality: one data-feed path (in-place reconcile); wholesale-rebuild code gone; existing interactive tests (button/list action parity) pass unchanged.
    - Security: no authority change.
  - Approach:
    - Documentation Reviewed:
      - `src/masonry_editor.rs` (`sync_region`), `src/main.rs` (the `edit_widget(editor)` update sites).
    - Options Considered:
      - Keep the wholesale rebuild behind a fallback flag: rejected (ponytail) — two paths is debt; the spike + core already prove in-place works.
      - Switch over directly once Step 11b's tests pass: chosen.
    - Chosen Approach:
      - Replace the wholesale swap in `sync_region` with a call into the in-place reconciler; remove rebuild-only helpers.
    - Files to Create/Edit:
      - `src/masonry_editor.rs`: `sync_region` → in-place reconcile.
      - `src/masonry_sdui_region.rs`: remove wholesale-rebuild-only code.
  - Test Cases to Write:
    - Full suite green (lib/editor/runtime) with existing button/list parity tests unchanged.
    - Region-level test: focus on a button survives an `apply_update` that does not remove it.
  - Progress (DONE):
    - `EditorWidget::sync_region` no longer rebuilds wholesale. It takes a `WidgetMut` to the persistent region child (`ctx.get_mut(&mut self.region)` → `try_downcast::<SduiRegionWidget>()`) and drives it in place: `set_render_context` (typography/theme fields) + `reconcile_snapshot_live(ctx, tree)` when there is a sidebar tree, or `clear_live(ctx)` when the root is gone. The `std::mem::replace` + `remove_child` + `children_changed` swap is deleted; the region pod is created once in `new_region_pod` and never replaced, so its `pods`/`child_keys` identity maps persist across updates and the reconciler diffs against the previous tree.
    - `set_render_context` dropped its internal `rebuild()` (now fields only) — rebuilding on every theme change would have discarded the identity the reconciler preserves; the in-place reconcile applies the new typography/theme to surviving leaves instead.
    - `clear_live` added for the no-root case (initial empty state / root removed): zeroes the data model and lets `reconcile(ctx)` tear down the subtree (a no-op when already empty).
    - Production data-feed is now a single in-place path. The no-ctx `reconcile_snapshot`/`apply_update`/`rebuild` remain only as standalone-test scaffolding for the geometry/data-model tests (which build a tree without a live `RenderRoot`); they are not a UI data-feed path and sit under the module's `#![allow(dead_code)]`.
    - `main.rs` needed no change: `sync_region`'s signature is unchanged, so the three `edit_widget(editor)` call sites drive the new in-place path as-is.
    - Focus-survives-update coverage is the Step 11b test `stable_identity_preserves_focus_across_unrelated_update` (focus is keyed by `WidgetId`, so the persistent identity keeps it across a server update).
    - Gate green: fmt + clippy `-D warnings` clean; lib 1132 / editor 149 / runtime 196; existing button/list action-parity tests unchanged. Manual visual gate pending.

- [x] **(Step 12 — requires Step 11c)** Migrate `scroll` to a Masonry `Portal`; delete the hand-managed scroll state
  - **Status: done (2026-07-29).** The reconciled region root is now a Masonry `Portal` (`wrap_in_portal`, `constrain_horizontal(true)`) that owns scroll position, clips content to the viewport, and renders a vertical scrollbar on overflow. `EditorWidget::layout` places the region as a *fixed viewport* (sidebar width × sidebar height below the top padding) at the sidebar origin — no scroll-offset placement math. The in-place reconciler (Step 11c) unwraps the `Portal` via `Portal::child_mut` to reconcile the content subtree, so the `Portal` (and its `viewport_pos`) survives SDUI updates. Supporting changes: the vestigial `EditorView` leaf is now zero-width so the root `Flex(Row)[panel, editor]` fits the viewport horizontally (no unwanted horizontal scroll range); the region/`EditorWidget` accessibility overrides were opened up so the tree a11y flows through the `Portal` subtree with Masonry-computed scroll-aware bounds (this also retires the known dual-a11y debt — the legacy `collect_accessibility_entries` tree walk is deleted). Deleted from `SduiNativeState`: `scroll_offset`/`content_height`/`viewport_height` fields, `scroll_lines`/`scroll_vertical_pixels`/`scroll_offset` methods, the `collect_action_regions` content-height walk, and the obsolete hand-managed scroll test.
  - **Deviation from the written criteria (robustness):** `scrolls_point` was *kept* (the criteria listed it for removal). The `Portal` does not call `set_handled` on scroll, so the event still bubbles to `EditorWidget`; `scrolls_point` remains the clean way to distinguish sidebar scroll (Portal-handled, `EditorWidget` returns `(false,false)`) from editor scroll. Removing it would require a more fragile target-tracking mechanism for no deletion win.
  - **Known minor limitation:** scrolling over the ~`panel_padding`-tall strip at the very top of the sidebar (above the region viewport) does not scroll, because the region viewport starts below the top padding (preserving the legacy scroll-range algebra). Scrolling over the content itself — the common case — works through the scroll viewport.
  - **Rework (2026-07-29, theme-driven scrollbar):** the stock Masonry `Portal` was replaced by a Clay-owned `SduiScrollViewport` (`wrap_in_viewport`). Masonry's `ScrollBar` paints with a hardcoded `theme::SCROLLBAR_COLOR` const (not themeable per-widget), so the sidebar scrollbar could not follow Clay's active theme. `SduiScrollViewport` is the same "adopt Masonry for behavior, own the paint for theme" pattern the other reconciled widgets (`SduiButton`/`SduiListRow`) already use: it owns the clamped `scroll_offset`, clips content to the viewport (`set_clip_path`), scrolls via `compose` translation, and paints the scrollbar in `post_paint` via `paint_scroll_chrome` (the same primitive as the editor scrollbar) so thumb/track follow `surface.scrollbar`/`surface.scrollbar.track`. Interaction matches the editor scrollbar (wheel scroll + hover/active visual; thumb-drag deferred). The reconciler reaches the content via `SduiScrollViewport::content_mut` (mirrors `Portal::child_mut`); scroll position still persists across SDUI updates because the viewport survives in-place reconcile (Step 11c). Theme sync: the reusable reconcile branch pushes `ui_theme` into the surviving viewport so theme changes repaint the scrollbar. Renamed tests: `viewport_scrolls_sidebar_content_through_masonry_path`, `viewport_scroll_position_persists_across_sdui_updates`; added `viewport_scrollbar_thumb_tracks_scroll_and_overflow` + `viewport_scrollbar_interaction_state_tracks_pointer`.
  - Gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean; lib 1136 / editor 149 / runtime 196 green (protocol `plan061_...` + security `package_manifest_...` failures are pre-existing on clean tree, verified via `git stash`).
  - Test Cases (committed): `viewport_scrolls_sidebar_content_through_masonry_path` (wheel scroll pans content through the viewport), `viewport_scroll_position_persists_across_sdui_updates` (scroll position survives an in-place `apply_update` — the blocking regression), `viewport_scrollbar_thumb_tracks_scroll_and_overflow` + `viewport_scrollbar_interaction_state_tracks_pointer` (themed-scrollbar math + interaction state).
  - Acceptance Criteria:
    - Functional: the sidebar scrolls via a `Portal` wrapping the reconciled tree; scroll position persists across server SDUI updates (the `Portal` survives via Step 11's stable identity); row activation intent unchanged.
    - Performance: scroll within `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`; viewport-bounded; no full-list repaint per scroll tick.
    - Code Quality: `scroll_offset`/`content_height`/`viewport_height` + `scroll_lines`/`scroll_vertical_pixels`/`scrolls_point` removed from `SduiNativeState` once the `Portal` owns scroll.
    - Security: no authority change.
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/widgets/portal.rs` (`Portal::new`, `constrain_horizontal`, `viewport_pos`, scrollbar mutators), `scroll_bar.rs`.
      - `.agents/skills/clay-ui/references/components.md`: scroll chrome (`paint_scroll_chrome`).
    - Options Considered:
      - Keep hand-managed scroll: rejected — duplicates Masonry scrolling and blocks the retained end-state.
      - External `scroll_offset` pushed into a fresh `Portal` on rebuild (hybrid): rejected — more complex than status quo while keeping the hand-managed state; superseded by Step 11's stable identity.
      - `Portal`-backed scrolling on a stably-identified region: chosen — the `Portal` owns scroll and survives updates.
    - Chosen Approach:
      - Wrap the reconciled tree in a vertical `Portal` (constrain horizontal); let it own scroll + scrollbar; delete the hand-managed scroll path.
    - API Notes and Examples:
      ```rust
      Portal::new(child).constrain_horizontal(true) // vertical-only Masonry scroll
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: wrap subtree in `Portal`.
      - `src/masonry_editor.rs`: place the region at the sidebar viewport (no scroll offset); drop the sidebar scroll routing.
      - `src/masonry_sdui.rs`: remove `scroll_offset`/`content_height`/`viewport_height` + scroll helpers.
    - References:
      - `src/shell/primitives.rs` (`paint_scroll_chrome`) if the `Portal` scrollbar is restyled to match Clay tokens.
  - Test Cases to Write:
    - Scroll position persists across an `apply_update` (the regression that blocked this step).
    - Scroll clamps at content bounds; row action intent unchanged.
  - History: originally paired with `list` as "Step 10b"; deferred because the wholesale region rebuild reset the `Portal`'s `viewport_pos` on every SDUI update. Step 11's stable-identity reconciler removes that blocker (see the Step 10 progress note for the original deferral reasoning).

**Step 13 — the transient family (full real migration), split into six dependency-ordered stages (13a–13f; 13f is the hosted menu a11y-parity follow-up).** The transient kinds (`overlay`/`portal`/`dropdown`/`collapse`/`modal`/`textInput`) are **package_ui components** (`ComponentKind` in `src/shell/components.rs`), **not** SDUI-tree nodes — `SduiNodeKind` has only `Panel`/`Label`/`Button`/`List`/`EditorView`/`Flex`/`Stack`. Today they are painted immediate-mode by `paint_package_component` (`src/masonry_sdui.rs`) and driven by hand-rolled client-local state in `SduiNativeState`: `dropdown_selected`, `collapse_expanded`, `focused_action`, and `modal_focusable_intents()` (keyboard-routed in `src/masonry_editor.rs::route_package_component_key`).

- **Scope boundary:** the editor text canvas (`src/editor/surface.rs`, where file content renders) is **out of scope** and stays bespoke. `textInput` here is only the *package-contributed single-line field* (search/rename/filter boxes in panels/overlays), a different component entirely.
- **Decision (2026-07-29, user-approved): full real migration, not parity.** `textInput` becomes genuinely editable (Masonry `TextInput` + Clay optimistic value-sync — the same client-local-editing / server-authority pattern the editor text already uses), because a non-editing input is useless to a package and parity would only defer the rework. Retained stable-identity widgets are also the *performant* substrate for transient surfaces: immediate-mode re-walks/repaints the whole component tree per keystroke/hover, while retained widgets repaint only the dirty subtree. Substrate pattern throughout: **Masonry for behavior, Clay owns the paint** (theme tokens, validation border, placeholder color) — same as `SduiButton`/`SduiListRow`/`SduiScrollViewport`.
- **Shared security invariant (all stages):** focus trapping/dismissal/z-order stay Clay-owned; actions stay registered-command intents; `textInput` never grants authority via its value; reconciliation consumes only server-validated inert declarations (no package JS in layout/paint/input).

- [x] **(Step 13a — requires Step 11c)** Package-component reconciliation foundation
  - **Status: done (2026-07-29).** New module `src/masonry_package_region.rs` hosts `PackageRegionWidget` — a stable-identity reconciler for *nested* `PackageUiComponentTree`s (unlike the SDUI flat-node + incremental-ops model, package trees re-provide whole, so the reconciler diffs the new tree against the retained widget subtree by `stable_package_source_id`). Mirrors the Step-11 `pods`/`child_keys` keyed-diff: survivors reuse pods, `Flex` child lists diff in place, reorders rebuild (no move-child API), kind changes force a fresh subtree (the `PackageChildKey::Component` key carries the kind so a nested kind change reads as remove+add, not a survivor whose downcast would silently fail), `gc` prunes unreachable records. Reconciled kinds: `panel`/`flex`/`stack`/`overlay`/`scroll`/`portal` containers (all zero-gap `Flex` columns — package containers flow children vertically; package `stack` is *not* a z-stack), `label`/`statusItem`/`editorView` via `PackageLeaf`, `button` via `PackageButton`, `list` rows via `PackageListRow`. Transient kinds (`dropdown`/`collapse`/`modal`/`textInput`) reconcile to an inert placeholder leaf pending 13c–13e.
  - **Widget reuse deviation (from the plan's "reuse `SduiButton`/`SduiListRow`/`SduiLegacyLeaf" suggestion):** package components carry package-specific data (`selected`/`disabled`/`validation_state`) the SDUI widgets don't model, and `statusItem` has no `SduiNodeKind` analogue, so the leaves are package-specific widgets (`PackageLeaf`/`PackageButton`/`PackageListRow`) that reuse the shared **paint helpers** (`paint_sdui_text`/`sdui_row_rect`/`component_state_color`/`list_row_fill_color`/`disabled_text_color`/`paint_focus_ring`) — parity by construction, zero changes to the production SDUI widgets. Interaction mirrors the `SduiButton`/`SduiListRow` Masonry pointer/focus/keyboard pattern; actions (`PackageButtonPress`/`PackageListRowPress`) carry the inert intent and are routed to the server in 13b.
  - Supporting change: `stable_package_source_id`/`package_action_intent` made `pub(crate)` in `src/masonry_sdui.rs`; module registered in `src/lib.rs`.
  - Gate: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` clean; lib 1143 (+7) / editor 149 / runtime 196 green (protocol `plan061_...` + security `package_manifest_...` failures are the known pre-existing ones on clean tree).
  - Test Cases (committed): `every_nontransient_kind_reconciles_to_a_root_pod`, `panel_children_reconcile_at_geometry_parity` (title/label/button/list-row heights match legacy cursor advances), `stable_identity_across_prop_update`, `kind_change_forces_rebuild`, `child_list_add_remove_reconciles_survivors`, `package_button_action_emits_server_intent`, `package_list_row_action_emits_server_intent`.
  - Goal: a retained-widget reconciliation path for `PackageUiComponentTree`, mirroring the `SduiRegionWidget`/Step-11 stable-identity reconciler. **No production paint change yet** — proven standalone.
  - Acceptance Criteria:
    - Functional: `PackageRegionWidget` reconciles a `PackageUiComponentTree` into retained Masonry widgets keyed by stable component id (`stable_package_source_id`); surviving components keep `WidgetId` across updates (Step-11 pattern). `panel`/`label`/`button`/`list` reconcile at pixel parity with `paint_package_component` (reuse `SduiButton`/`SduiListRow`/`SduiLegacyLeaf`/`paint_sdui_text` where possible).
    - Performance: no production hot-path change (standalone reconciler only).
    - Code Quality: the keyed-diff core is shared with / factored from the Step-11 SDUI reconciler, not duplicated.
    - Security: shared Step-13 invariant.
  - Approach:
    - Chosen Approach: build `PackageRegionWidget` (new module or in `src/masonry_sdui_region.rs`) reusing the Step-11 `pods`/`child_keys` keyed-diff; map each `ComponentKind` to a widget; validate `panel`/`label`/`button`/`list` first (they already have SDUI-tree analogs to reuse).
    - Documentation Reviewed: Step 11 reconciler (this plan), `src/shell/components.rs` (`ComponentKind`), `src/masonry_sdui.rs::paint_package_component` (parity target).
    - Files to Create/Edit: `src/masonry_sdui_region.rs` (or `src/masonry_package_region.rs`) — `PackageRegionWidget` + per-kind builders; `src/shell/package_ui.rs` — expose a stable tree snapshot for reconciliation.
  - Test Cases to Write: panel/label/button/list reconcile at pixel parity (geometry + glyph positions); stable identity across a prop-only update; kind-change forces a fresh subtree.

- [x] **(Step 13b — requires Step 13a)** Fixed-panel widget hosting + `collapse` migration
  - **Status: done (2026-07-29).** First production cutover. **`PackagePanelHost`** (`src/masonry_package_region.rs`) is a retained `EditorWidget` child that fills the working area, paints only the panel chrome (`paint_panel_chrome`), and hosts one `PackageRegionWidget` per *visible* fixed panel, reconciled in place by `FixedSlotId` (surviving panels keep widget identity → `collapse` state persists). Panel rects are computed in layout from the working-area size (`PackageUiRuntimeState::visible_fixed_panels`); a new `visible_fixed_panel_components` accessor feeds the no-size reconcile. `EditorWidget` hosts `[panel_host, region]` (panel_host first so the SDUI sidebar stays topmost/hit-priority); `sync_panels` mirrors `sync_region` and is wired at the same three `main.rs` event sites, gated by a new `panels_dirty` flag (set in `apply_package_ui_update`/`install_package_ui_snapshot`/`set_ui_theme`/`set_typography`). `PackageButtonPress`/`PackageListRowPress` are re-exported via `masonry_editor` and downcast in `main.rs` to the shared server-first `route_sdui_intent`.
  - **`PackageCollapse`** (same module) holds `expanded` in the widget (retained across reconcile via stable identity), paints its title row, hosts its children in a content `Flex`, and hides collapsed children via a `set_clip_path` (title-row height) — no show/hide re-registration. Toggle on title-row click / Enter / Space / a11y click.
  - **Deletions:** `collapse_expanded` + `is_collapse_expanded` + `collapse_toggle` from `SduiNativeState`; `paint_package_fixed_panels`; the `collapse` branch of `paint_package_component`; the `clay.ui.collapseToggle` arm of `route_package_component_key` (Masonry widget owns focus/keyboard now).
  - **Deviations from the acceptance criteria (documented):** (1) `PackageCollapse`/`PackagePanelHost` live in `masonry_package_region.rs`, not a `SduiCollapse` in `masonry_sdui_region.rs` — consistent with the 13a package-specific-widget decision. (2) The collapse toggle is **client-local UI state**, *not* a `clay.ui.collapseToggle` server intent — that matches the actual legacy behavior (`collapse_expanded` was always client-local; the intent only fed hand-rolled focus routing, now replaced by Masonry focus). (3) No chevron — the legacy collapse paint had none (title row only), so parity keeps it chevron-free. (4) Known staging gap: `dropdown`/`textInput` inside a fixed panel (e.g. the settings surface) render as dimmed placeholders until 13c/13d migrate them.
  - Gate: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` clean; lib 1146 (+3) / editor 149 / runtime 196 green (protocol `plan061_...` + security `package_manifest_...` are the known pre-existing failures).
  - Test Cases (committed): `collapse_toggles_children_visibility` (title-only ↔ title+children via click), `collapse_expanded_state_survives_reconcile` (retained state + identity across prop-only update), `panel_host_hosts_and_removes_panels` (visible panel hosted, reconciled in place, dropped when hidden).
  - Goal: first production cutover — host fixed-panel content as real Masonry children (replacing the `paint_package_fixed_panels` immediate-mode walk), and migrate the simplest interactive inline kind (`collapse`).
  - Acceptance Criteria:
    - Functional: fixed panels render through the reconciled subtree (chrome via `paint_panel_chrome` retained); `collapse` is a real widget (Group role, Enter/Space toggle, chevron, children shown/hidden) via Masonry focus + Clay intent routing, not `collapse_expanded`.
    - Performance: fixed-panel paint is incremental (dirty subtree only), not a full per-frame re-walk.
    - Code Quality: `collapse_expanded` + the fixed-panel branch of `paint_package_component` are deleted; the `paint_package_fixed_panels` immediate-mode walk is removed.
    - Security: collapse toggle emits a registered-command intent (`clay.ui.collapseToggle`); shared Step-13 invariant.
  - Approach:
    - Chosen Approach: host the reconciled fixed-panel subtree as a Masonry child of `EditorWidget` positioned at each fixed-panel rect (below the region child, matching current chrome z-order); `collapse` becomes a `SduiCollapse` widget (header row + conditional children) using Masonry focus.
    - Files to Create/Edit: `src/masonry_editor.rs` — host fixed-panel child(ren) at slot rects; `src/masonry_sdui.rs` — delete `collapse_expanded` + fixed-panel paint; `src/masonry_sdui_region.rs` — `SduiCollapse` widget.
    - References: Steps 9/10 (per-kind migration + legacy-hit-test deletion), Step 8 (hosting a child of `EditorWidget`).
  - Test Cases to Write: collapse toggle expands/collapses via keyboard (Enter/Space) and pointer; fixed-panel rendering parity (chrome + label/button/list); `collapse_expanded` compile-time absence.

- [x] **(Step 13c — requires Step 13b)** `textInput` → real editable field
  - **Status: done (2026-07-30).** `PackageTextInput` (`src/masonry_package_region.rs`) is a genuinely-editable single-line field: a retained Masonry **`TextArea<true>`** (editing/selection/clipboard/IME, reports `Role::TextInput`) wrapped in **Clay-owned chrome** — background (`surface.control`), a full 1px border (`validation state > focus > subtle`), and the placeholder (shown only when empty) all painted from theme tokens. Editing is **optimistic-local** (the `TextArea` updates itself per keystroke; `TextAction::Changed` is not synced per-key, bounding cost). The **committed value** (Enter → `TextAction::Entered`) is routed to the server: the region keeps a `text_input_intents` map (inner `TextArea` `WidgetId` → base intent) and `text_input_commit` appends the value as a `"value"` argument; `main.rs` downcasts `TextAction` and routes via `EditorWidget::package_text_input_commit` → `PackagePanelHost` → region. **Server authority:** a changed `component.text` is adopted on reconcile (`TextArea::reset_text`) only when the field is **not** focused (revert-on-reject without clobbering an in-progress edit); `is_focused` is tracked via `Update::ChildFocusChanged`.
  - **Design deviation (from the plan's "Masonry `TextInput`"):** we wrap `TextArea<true>` directly, **not** Masonry `TextInput`. `TextInput` hard-codes a white focus border in its own paint; `TextArea` paints *only* text/selection/caret, so Clay owns 100% of the chrome (consistent with the adopt-Masonry-for-behavior-own-the-paint principle). The commit command is the component's `action_command_id`, else the internal `clay.ui.textInputCommit`.
  - **Deletions:** the `textInput` branch of `paint_package_component` (bordered field + placeholder + `clay.ui.textInputFocus` focus ring + `actions.push`); the now-redundant **fixed-panel a11y loop** in `accessibility_entries` (fixed-panel a11y flows through the hosted `PackagePanelHost`/`PackageRegionWidget` Masonry subtree since 13b — this also fixes a latent 13b a11y double-count; the overlay/menu a11y paths remain for not-yet-hosted transient chrome).
  - **Real bug caught by tests:** an empty `TextArea` computes a **zero content height**, leaving the field with no hit area to click into; the layout pins the `TextArea` to the line height so the field is always clickable.
  - **Known gap (documented):** a `disabled` textInput records no commit intent (never routes a command) but the field remains editable; no current package uses a disabled textInput.
  - Gate: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` clean; lib 1150 (+4) / editor 149 / runtime 196 green (protocol `plan061_...` + security `package_manifest_...` are the known pre-existing failures).
  - Test Cases (committed): `text_input_typing_is_optimistic_and_commit_emits_intent` (type → `TextAction::Entered("ab")` + commit intent carries the `"value"` argument), `text_input_border_color_resolves_validation_over_focus_over_subtle` (all 3 validation states + focus + subtle precedence), `text_input_placeholder_shown_only_when_empty`, `text_input_adopts_changed_server_value_when_unfocused` (server authority).
  - Goal: replace the placeholder-only paint with a genuinely editable single-line input — the only outcome consistent with "packages modify what's shown."
  - Acceptance Criteria:
    - Functional: `textInput` is a real editable field (TextInput role) on Masonry `TextInput` (editing, selection, clipboard, IME, a11y), wrapped Clay-owns-the-paint: theme tokens, validation-state border (`diagnostic.error`/`warning`/`success` > focus > `border.subtle`), placeholder color. Editing is optimistic-local; the committed value is emitted to the server as a registered-command intent and the server stays the authority — the same optimistic-sync model the editor text already uses.
    - Performance: per-keystroke cost is bounded to the input widget (retained), not a full component-tree re-walk.
    - Code Quality: the `textInput` branch of `paint_package_component` + its hand-rolled `is_focused`/`actions.push` focus are deleted.
    - Security: value syncs as an inert intent; validation/border are style-only; shared Step-13 invariant.
  - Approach:
    - Chosen Approach: `SduiTextInput` wrapping Masonry `TextInput`/`TextArea<true>`; Clay sets placeholder/validation/theme via properties + border paint; a value-sync bridge stages local edits and emits the committed value as an intent (server authority). Confirm the committed-value → server channel during implementation.
    - Documentation Reviewed: Masonry `src/widgets/text_input.rs`, `text_area.rs`; editor optimistic text-sync (shadow copy + version tracking).
    - Files to Create/Edit: `src/masonry_sdui_region.rs` — `SduiTextInput` + theme/validation bridge; `src/masonry_sdui.rs` — delete the `textInput` paint branch; value-sync intent plumbing (existing `enqueue_*_intent` channel).
  - Test Cases to Write: typing updates the field optimistically + committed value emitted as an intent; validation-state border resolves error/warning/success > focus > subtle; placeholder shown when empty/unfocused.

- [x] **(Step 13d — requires Step 13b)** `dropdown` → real widget
  - **Status: done (2026-07-30).** `PackageDropdown` (`src/masonry_package_region.rs`) is a real ComboBox-role widget: the closed trigger shows the selected item's label; clicking (or Enter/Space) opens the inline item list; ArrowUp/Down cycles the highlight; Enter/Space confirms (emitting the item's command via `PackageDropdownSelect`, routed through the same server-first path); Escape closes. Selection + open state are **widget-local** and survive reconcile (stable identity; clamped if the item list shrinks); the initial selection honors the server-marked `selected` item (else 0). Clicking takes keyboard focus (`request_focus` on Down) so arrow/Enter nav reaches the widget.
  - **Design: single self-contained widget.** The open list is painted inline by the widget (matching the legacy `paint_package_component` behavior); row hover/active derive from the tracked pointer position (the `SduiScrollViewport` pattern), not from per-row child widgets — so there is no parent↔child coordination for "row click updates selection + closes."
  - **Two legacy gaps fixed (not regressions):** (1) the legacy open list painted `component.children`, but the real dropdowns (settings) declare `items`, so the legacy open list was **never visible** for the actual usage — the new widget renders the `items`; (2) the legacy trigger only opened when the dropdown had a top-level `action_command_id` (settings dropdowns have none → the legacy trigger painted `Disabled` and never opened) — the new trigger toggles open regardless, since the *items* carry the commands.
  - **Deletions:** the `dropdown_selected` map + `dropdown_selected_index`/`dropdown_cycle`; the `dropdown` paint branch of `paint_package_component`; the `dropdown` a11y branch of `collect_package_accessibility_entries`; and the `clay.ui.dropdownToggle` routing in `route_package_component_key` (collapsed to the modal-only Tab trap). The `component_state_palette` `"dropdown"` arm + its validation tests are **kept** (consistent with the `collapse` precedent from 13b — the palette is a state-table validation helper, not the production paint path).
  - Gate: `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` clean; lib 1153 (+3) / editor 149 / runtime 196 green (protocol `plan061_...` + security `package_manifest_...` are the known pre-existing failures).
  - Test Cases (committed): `dropdown_arrow_keys_cycle_and_enter_confirms` (ArrowDown cycles, Enter confirms + emits the item's command), `dropdown_selection_persists_across_unrelated_update` (widget id + selection survive an unrelated reconcile), `dropdown_open_list_row_hover_active_and_click_emits_intent` (row hover/active state + click confirms + closes).
  - Goal: migrate the single-select dropdown to a real widget with keyboard nav, deleting `dropdown_selected`.
  - Acceptance Criteria:
    - Functional: `dropdown` is a real widget (ComboBox role): closed trigger shows the selected item label; ArrowUp/Down cycles the selection, Enter/Space confirms; the open list renders (inline below the trigger, matching current `paint_package_component` behavior) with hover/active feedback. Keyboard/focus via Masonry focus + Clay intent routing, not `dropdown_selected`.
    - Performance: open/close + nav repaint only the dropdown subtree.
    - Code Quality: `dropdown_selected` + the `dropdown` paint branch + its hand-rolled `interaction_state`/`is_focused` are deleted; the `clay.ui.dropdownToggle` routing in `route_package_component_key` is removed.
    - Security: selection change emits a registered-command intent; disabled state honored; shared Step-13 invariant.
  - Approach:
    - Chosen Approach: `SduiDropdown` widget (trigger row + expandable list) using Masonry focus for keyboard; selection index held as widget-local state (survives via Step-11 stable identity). Open-list rendered inline (current behavior); hoisting it to a true overlay is part of 13e if desired.
    - Files to Create/Edit: `src/masonry_sdui_region.rs` — `SduiDropdown`; `src/masonry_sdui.rs` — delete `dropdown_selected` + dropdown paint branch; `src/masonry_editor.rs` — remove the `clay.ui.dropdownToggle` keyboard routing.
    - References: Steps 9/10 (per-kind pattern), `src/masonry_editor.rs::route_package_component_key` (current dropdown routing).
  - Test Cases to Write: ArrowUp/Down cycles the selected label + Enter/Space confirms/closes; selected index persists across an unrelated update (stable identity); open-list row hover/active + selection intent emitted.

- [x] **(Step 13e — requires Step 13a, 13b)** Overlay hosting + `modal` focus trap + `overlay`/`portal` containers
  - **Status: done (2026-07-30).** `EditorWidget` hosts three children in registration order `[panel_host, region, overlay_host]` (reverse hit-test gives `overlay_host` topmost). `PackageOverlayHost` reconciles package `transient_overlays()` (package overlays + `TransientPackageOverlay::from_menu_session`) by id with z-order (`z.overlay`=0 < `z.modal`=1 < `z.tooltip`=2), sizes each overlay to its anchor rect, paints `tooltip_shell` chrome behind the children, and returns `accepts_pointer_interaction()=false` so it is transparent to hit-testing (clicks outside an overlay's rect fall through to the region/editor; overlay-region children handle their own clicks) — resolving the Step-7 bounding-rect caveat. `PackageModal` is a `Role::Dialog` widget that traps focus via a manually-tracked `focus_index` cycling among focusable descendants (`ctx.set_focus` + `set_handled`), dismisses on Escape via a `PackageModalDismiss` action. `overlay`/`portal` container kinds already map to zero-gap `Flex::column` in the `PackageRegionWidget` reconciler (13a).
  - **Menu hosting + keyboard re-sync:** the menu (the only real runtime overlay) renders through the same hosted overlay pipeline. Menu keyboard navigation is client-local (`route_menu_key` → `menu_select_next`/`previous`/`cancel`), so it produces no server connection event to drive `sync_overlays`. Added `EditorAction::MenuStateChanged`: `local_key` submits it after `route_menu_key` returns true; `main.rs` handles it by calling `sync_overlays` (the only path to a `MutateCtx` — `EventCtx` cannot reach one), which reconciles the menu overlay in place and updates the hosted rows' `selected` highlight. Verified by `overlay_host_reconcile_updates_menu_selection`.
  - **Deletions:** `focused_action`/`set_focused_action`/`focused_action` getter/`is_focused`/`interaction_state`/`modal_focusable_intents`; `paint_package_component` (whole function, ~280 lines); `paint_package_overlays` + `paint_overlays`; `route_package_component_key` (modal Tab trap) + its `local_key` call; the `action_for_point` branch in `EditorWidget::PointerDown`; the SDUI `pointer_pos`/`pointer_pressed` state + setters/clearers (dead write-only state). `set_active_menu`/`menu_select_*`/`menu_cancel` set `overlays_dirty`; `apply_package_ui_update`/`install_package_ui_snapshot` now set `panels_dirty`+`overlays_dirty` (latent bug fix — they previously reconciled neither).
  - **Deviation (documented — tracked as Step 13f):** the legacy overlay/menu a11y branch (`collect_active_menu_accessibility_entries` + the overlay arm of `accessibility_entries`) is **kept**, not deleted. It provides rich `Menu`/`MenuItem` roles with custom accessibility labels ("Reload from disk", "Keep dirty buffer") and a "selected" suffix that the generic hosted `Group`/`ListItem` a11y from `PackageRegionWidget`/`PackageListRow` cannot replicate. `overlay_host` is intentionally **excluded** from `EditorWidget`'s a11y children so the menu is not double-reported; the legacy `Menu`/`MenuItem` tree is the single a11y source. Deleting the legacy branch awaits hosted menu a11y parity (Step 13f).
  - **Also kept (not in the plan's deletion list):** the legacy hit-test model (`self.actions`/`action_for_point`/`rebuild_action_regions_for_test`/`collect_package_action_regions`) — test-only, used by migration-verification tests asserting migrated kinds are no longer served by the legacy hit-test.
  - **`PackageModalDismiss`** is emitted on Escape as the dismissal mechanism but is **not downcast in `main.rs`** (zero packages declare `modal` components at runtime); production routing to close an overlay is future work when a real package modal ships. Menu dismissal stays with `route_menu_key` (Escape → `menu_cancel`).
  - Acceptance Criteria:
    - Functional: package overlays render through real Masonry children layered above the region child (each sized to its anchor rect so it doesn't block the region — the Step-7 bounding-rect caveat); `modal` is a real widget (Dialog role, Tab/Shift+Tab focus trap via Masonry's focus chain, dismissal) at `z.modal`; `overlay`/`portal` are real container widgets with anchor + dismissal + focus policy. Transient z-order (`z.overlay` < `z.modal` < `z.tooltip`) preserved.
    - Performance: overlays repaint only when their own state changes (not per-frame `post_paint`).
    - Code Quality: `focused_action`, `modal_focusable_intents()`, the overlay branch of `paint_package_component`, and the `paint_package_overlays` immediate-mode walk are deleted; the modal Tab-trap routing in `route_package_component_key` is removed. **Deferred to 13f:** the overlay branch of `collect_accessibility_entries` (kept for menu a11y quality).
    - Security: focus trap/dismissal/z-order stay Clay-owned; shared Step-13 invariant.
  - Approach:
    - Chosen Approach: `EditorWidget` hosts overlay children after the region child (later = higher in Masonry's paint + reverse hit-test order); each overlay child is an anchored container sized to its rect; `modal` traps focus via Masonry's focus chain (`accepts_focus` + Tab traversal scoped to the dialog). Reuse `TransientMenuSession`/`TransientPackageOverlay` state model; hand rendering to widgets.
    - Documentation Reviewed: Masonry `src/widgets/zstack.rs`, `portal.rs`; focus/event bubbling in `src/doc/implementing_widget.md`; `.agents/skills/clay-ui/references/components.md` (z-level stacking, `TransientMenuOrigin`, focus policies).
    - Files to Create/Edit: `src/masonry_editor.rs` — multi-child overlay hosting + remove modal Tab-trap routing; `src/masonry_sdui.rs` — delete `focused_action`/`modal_focusable_intents` + overlay paint paths; `src/masonry_sdui_region.rs` — overlay/modal container widgets; `src/shell/package_ui.rs` / `src/shell/transient_menu.rs` — keep state model, hand rendering to widgets.
    - References: Step 7 (Composition A + bounding-rect caveat), `src/shell/transient_menu.rs`.
  - Test Cases to Write: modal Tab/Shift+Tab cycles focus within the dialog + Escape/dismissal; overlay z-order stacking preserved (z.overlay < z.modal < z.tooltip); overlay child does not block pointer to the region outside its rect; dropdown-open-list/modal route pointer correctly. **Written:** `modal_tab_trap_cycles_focus_and_escape_dismisses`; `overlay_host_reconciles_by_id_with_z_order`/`_anchor_layout`/`_transparent_to_pointer`/`_reconcile_updates_menu_selection`.

- [ ] **(Step 13f — requires Step 13e)** Hosted menu a11y parity; delete the legacy overlay/menu a11y branch
  - Goal: give the hosted overlay path (`PackageOverlayHost` → `PackageRegionWidget` → `PackageListRow`) screen-reader output equivalent to the legacy `Menu`/`MenuItem` a11y, then delete the legacy overlay/menu a11y branch and re-include `overlay_host` in `EditorWidget`'s a11y children.
  - Motivation: Step 13e kept the legacy `collect_active_menu_accessibility_entries` + overlay arm of `accessibility_entries` because the hosted `Group`/`ListItem` a11y is semantically inferior (no `MenuItem` role, no "selected" suffix, no custom accessibility labels). The menu is the only real runtime overlay and is heavily used (completion/command-palette/context menu), so the regression is real. This step closes that gap.
  - Acceptance Criteria:
    - Functional: the hosted menu reports `Role::Menu` (container) and `Role::MenuItem` (rows), carries the active item's "selected" state, and uses the custom accessibility labels (e.g. "Reload from disk") where the menu session provides them; non-menu overlays report appropriate roles.
    - Code Quality: `collect_active_menu_accessibility_entries` + the overlay arm of `accessibility_entries` are deleted; `overlay_host` is **re-included** in `EditorWidget`'s a11y children (the hosted subtree becomes the single a11y source); the two legacy menu a11y tests (`active_menu_exposes_menu_role_and_item_accessibility_labels`, `accessibility_recovery_summary_uses_active_menu_prompt`) are either migrated to assert against the hosted tree or replaced by hosted-parity tests.
    - No double-report: only one menu a11y tree reaches the root node.
  - Approach (options to confirm at execution time):
    - Extend `PackageListRow::accessibility` with a menu-item mode (role `MenuItem`, selected state, custom label) driven by a flag/prop set during the menu's `build_component`/`reconcile_component`, and set the menu overlay region's container role to `Menu` — reuses the existing hosted widgets with additive menu-specific a11y.
    - Or: a dedicated menu overlay widget with its own a11y — divergent, only justified if the row widget's generic responsibilities conflict with menu a11y.
  - References: Step 13e deviation note; `collect_active_menu_accessibility_entries` in `src/masonry_sdui.rs`; `PackageListRow::accessibility` in `src/masonry_package_region.rs`.

- [ ] **(Step 14 — requires Step 13f)** Host `EditorView` as a real child component and retire the `SduiNativeState` god-object
  - v2 note: the *structural* hosting (region as a real child, z-order, compositor removal) moved to Step 8. This step is the final retirement: host the editor surface as a child component via `EditorView`, and — with all kinds now served by real Masonry widgets — delete `SduiNativeState::paint`, `SduiLegacyLeaf`, and the residual legacy hit-test/interaction state; `EditorWidget` no longer paints SDUI chrome and `masonry_sdui.rs` shrinks dramatically.
  - Acceptance Criteria:
    - Functional: `SduiNodeKind::EditorView` binds the existing editor surface as a child component in the reconciled tree (one editor binding per working area); with all kinds migrated, the immediate-mode `SduiNativeState::paint` path and its leaf `impl Widget` are removed; `EditorWidget` no longer paints SDUI chrome.
    - Performance: Editor text canvas hot path unchanged (bespoke virtualized rendering preserved); no new IPC/JS in paint.
    - Code Quality: `SduiNativeState` reduced to inert reconciled state or removed; `masonry_sdui.rs` shrinks dramatically; no dead `#![allow(dead_code)]` staging blocks remain for migrated code.
    - Security: Editor surface authority unchanged; no package JS enters the editor canvas.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/rendering-strategy.md`: editor surface attachment points; SDUI is the panel path, not the inline-decoration path.
      - `concept.md` Phase 3: bespoke virtualized canvas is intentional and stays.
    - Options Considered:
      - Rewrite the editor surface on Masonry text widgets: rejected — bespoke rope/parley virtualization is correct and normal for editors.
      - Keep editor surface bespoke, host it as a child component: chosen.
    - Chosen Approach:
      - `EditorView` reconciliation places the existing editor surface pod; delete the legacy paint path and the god-object once nothing references it.
    - API Notes and Examples:
      ```rust
      // EditorView -> existing EditorSurface hosted as a child pod in the pane main slot
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: EditorView child binding.
      - `src/masonry_editor.rs`: remove `self.sdui.paint(...)`; editor surface hosted by region/shell.
      - `src/masonry_sdui.rs`: delete retired paint/leaf-widget code.
    - References:
      - `src/editor/surface.rs`, `src/masonry_editor.rs:2646`.
  - Test Cases to Write:
    - Editor region geometry parity (`editor_region`/`editor_region_for_document`).
    - Full SDUI snapshot renders entirely through retained tree; legacy paint path absent (compile-time: removed functions).
    - Typing hot path non-regression (existing edit/scroll tests + perf scope).

- [ ] **(Step 15 — requires Step 14)** Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md`, `docs/reference/primitives/shell-layout-strategy.md`, and `docs/reference/primitives/rendering-strategy.md` reflect that SDUI kinds now render through a retained reconciled Masonry subtree (not immediate-mode paint), with the "temporary compatibility bridge" status removed; package-facing contract (inert declarations, Clay-owned widgets) unchanged.
    - Performance: Docs note the no-hot-path invariant is preserved.
    - Code Quality: Catalog drift guards still pass; `.agents/skills/clay-ui/references/components.md`/`tokens.md` updated if any kind note changed (implementation substrate changed, package-facing kinds did not).
    - Security: Docs reaffirm no raw CSS/client JS/native widget authority for packages.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`, `documentation-as-code.md`.
      - `docs/reference/packages/creating-packages.md`, `docs/reference/ui-components.md`.
    - Options Considered:
      - Document as a new package API: rejected — no package-facing change; this is client-internal substrate.
      - Update strategy/rendering docs to describe the retained reconciliation substrate: chosen.
    - Chosen Approach:
      - Edit strategy/rendering docs + catalog notes to match implementation; keep package contract stable.
    - API Notes and Examples:
      ```text
      docs/reference/primitives/shell-layout-strategy.md  # remove "temporary bridge" wording
      docs/reference/packages/creating-packages.md        # rendering substrate note
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/shell-layout-strategy.md`, `docs/reference/primitives/rendering-strategy.md`, `docs/reference/packages/creating-packages.md`, `.agents/skills/clay-ui/references/components.md`.
    - References:
      - `decision-logs/2026-06-09-1431-...md`.
  - Test Cases to Write:
    - Doc/catalog drift checks pass (`cargo test` documentation-contract gates).

- [ ] **(Step 16 — requires Step 14)** Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Confirm this plan introduces no new public programmatic capability; all new reconciliation types are `pub(crate)`; any inadvertently-public client function is made `pub(crate)`; no new op/facade required.
    - Performance: N/A (no new API surface).
    - Code Quality: Rust visibility audit — reconciliation container and helpers not exposed across the lib/bin boundary beyond what `main.rs` needs; `#[doc(hidden)]` native-only types stay non-registry.
    - Security: No raw `Deno.core.ops`, native widget handle, or client JS surface added; registry unchanged.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (Clay JS API Task), `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`.
    - Options Considered:
      - Expose reconciliation state for agents: rejected — observability stays internal test/agent surface (`SduiObservableSnapshot`), not a public API.
      - Verify-and-no-op with a visibility audit: chosen.
    - Chosen Approach:
      - Audit visibility; keep internals `pub(crate)`; document in the plan that no Clay JS API is added.
    - API Notes and Examples:
      ```rust
      pub(crate) struct SduiRegionWidget { /* ... */ } // not a JS-facing surface
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs` / `src/lib.rs`: enforce `pub(crate)` visibility.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
  - Test Cases to Write:
    - Visibility audit: `cargo test` doc-registry/lookup gates pass with no new public API rows.

- [ ] **(Step 17 — requires Step 14)** Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Confirm no new user-visible command, key binding, or configuration option is introduced; panel size/visibility configuration continues through existing documented surfaces; no hidden config keys added.
    - Performance: N/A.
    - Code Quality: No undocumented config keys; layout persistence path (`~/.config/clay/layout.json`) unchanged.
    - Security: No configuration path grants filesystem/network/shell/extension/AI/workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` (Clay Configuration Task), `.agents/skills/project-patterns/references/configuration-system.md`.
    - Options Considered:
      - Add a reconciliation debug config toggle: rejected (YAGNI) — observability stays internal.
      - Verify-and-no-op: chosen.
    - Chosen Approach:
      - Verify existing configuration surfaces remain the only ones; document no-op in plan.
    - API Notes and Examples:
      ```text
      ~/.config/clay/init.js  # unchanged entry point; no new options
      ```
    - Files to Create/Edit:
      - None expected (verify only).
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
  - Test Cases to Write:
    - Configuration conformance gates pass with no new undocumented keys.

- [ ] **(Step 18 — requires Steps 15–17)** Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages (new `masonry_sdui_region` reconciliation module, retired immediate-mode path, unified slot geometry).
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made
- **Step 13e — legacy overlay/menu a11y kept (tracked as Step 13f).** The hosted `PackageOverlayHost` → `PackageRegionWidget` → `PackageListRow` path reports generic `Group`/`ListItem` a11y, which is semantically inferior to the legacy `Menu`/`MenuItem` roles (no selected suffix, no custom accessibility labels). The menu is the only real runtime overlay and is heavily used, so deleting the legacy a11y would be a real screen-reader regression. The legacy branch is kept as the single a11y source (`overlay_host` excluded from `EditorWidget`'s a11y children to avoid double-report); hosted menu a11y parity + legacy deletion is scheduled as Step 13f.
- **Step 13e — legacy hit-test model kept (test-only).** `self.actions`/`action_for_point`/`rebuild_action_regions_for_test`/`collect_package_action_regions` remain because migration-verification tests use them to assert migrated kinds are no longer served by the legacy hit-test. Not in the plan's deletion list; cleanup belongs with Step 14 god-object retirement.
- **Step 13e — `PackageModalDismiss` not production-wired.** Zero packages declare `modal` components at runtime, so the modal's Escape-dismiss action is emitted but not downcast in `main.rs`; routing is future work when a real package modal ships.

## Further Actions
- **Step 13f (open):** build hosted menu a11y parity (`MenuItem` role + selected state + custom labels), then delete the legacy overlay/menu a11y branch and re-include `overlay_host` in `EditorWidget`'s a11y children. Unblocks a clean Step 14 god-object retirement (which would otherwise delete the menu's only a11y source).
