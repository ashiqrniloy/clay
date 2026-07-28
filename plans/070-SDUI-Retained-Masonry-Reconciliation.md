# SDUI Retained Masonry Reconciliation (Incremental, Component by Component)

## Context

The SDUI *data model* (`src/protocol/sdui.rs`) is a retained, versioned, incrementally-updated tree: stable `SduiNodeId`s, `SduiTree` snapshots, and `SduiTreeUpdate` ops (`ReplaceRoot`/`ReplaceNode`/`RemoveNode`). The *renderer* is not. `SduiNativeState` (`src/masonry_sdui.rs:84`) implements `Widget` with an empty `register_children` — it is a leaf that immediate-mode paints the entire panel/component tree into the Vello `Scene`, hand-rolling layout math, scroll, pointer hit-testing, keyboard nav, focus traps, and accesskit nodes (~161 manual paint/geometry/hit-test helpers). `EditorWidget` (`src/masonry_editor.rs:2268`) is likewise a leaf that paints editor surface + SDUI sidebar + status line and holds `sdui: SduiNativeState` as a field (`:293`).

Approved decision `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md` already chose a Clay-owned declarative shell over Masonry as substrate (use `Split`/`Flex`/`Grid`/`ZStack`/`Portal`) and explicitly rejected "keep `EditorWidget` as the whole app shell and overlay SDUI panels manually" as the long-term direction. The current hand-paint path is that rejected-as-long-term option. This plan closes that drift: converge the chrome/panel/component renderer onto a retained Masonry widget subtree reconciled from the SDUI tree, one component kind at a time, while leaving the bespoke editor text canvas (`src/editor/surface.rs`) untouched.

Authority model is preserved unchanged: server validates inert SDUI trees; no package JS in paint/layout/input; typed tokens not CSS; Clay owns native widgets. Only the client-internal rendering strategy changes.

## Migration Strategy Revision (recorded during execution)

Reading the renderer surfaced a constraint that reorders the work. `SduiNativeState::paint_node` (`src/masonry_sdui.rs`) lays out **every kind in one shared `cursor_y` flow**: `Panel` paints a title then recurses into children; `Label`/`Button`/`List`/`EditorView` each advance the same cursor; `Flex`/`Stack` recurse. Real trees intersperse kinds (e.g. `packages/settings/dist/load.js`: panel → label → control → label → … → button → button). Consequence: **a single kind cannot be extracted into a retained widget without breaking layout** — reconciling only labels would stack them contiguously while the legacy path still reserves gaps for them in the cursor flow (doubled/misplaced rows). The original leaf-first order (label → button → containers) is therefore inverted for this codebase.

Revised strategy (foundation-first):

1. **Container reconciler first** (tasks 5/6 foundations): reconcile the tree *structure* into a nested Masonry subtree — `panel`/`flex` → `Flex` (column/row), `stack` → `ZStack`, `label` → `Label` — so Masonry owns layout for the container skeleton.
2. **Rendering cutover as one dedicated step** (new task 6.5): host the reconciled region at the sidebar rect and move per-node rendering into the retained tree. Because the layout is monolithic, the cutover reconciles a *complete* subtree at once; leaf kinds not yet given real Masonry widgets are carried by a thin legacy-paint leaf widget that reports the exact legacy row size, so layout stays pixel-identical and leaves can then be swapped to real Masonry widgets one kind at a time (tasks 3/4/7) with no layout risk.
3. **Leaf migrations become safe** (tasks 3/4/7): each replaces one legacy-paint leaf with a real Masonry widget of the same size — layout-neutral, independently verifiable.

Tasks 5 and 6 below are split into a **Completed (foundation)** part and a **Remaining (cutover)** part accordingly; neither top-level task is fully closed until the cutover (task 6.5) lands.

## Objectives

- Replace immediate-mode SDUI painting with a retained Masonry widget subtree reconciled from `SduiTree`/`SduiTreeUpdate`, component kind by component kind.
- Recover Masonry-provided layout, hit-testing, focus traversal, pointer capture, and accessibility instead of hand-rolling them.
- Collapse the two parallel geometry systems (`WorkingAreaLayout` in `src/masonry_shell.rs` vs. `sdui_slot_layout`/`PaneSlotLayout` bridging inside the editor widget) into one Clay-owned slot geometry.
- Preserve the authority boundary, the no-package-JS-in-hot-path invariant, token-driven styling, payload budgets, and all existing conformance tests throughout.
- Delete the hand-painted code as each component kind migrates, shrinking the `SduiNativeState` god-object toward removal.

## Expected Outcome

- SDUI panels and components render as real Masonry child widgets (`Flex`/`ZStack`/`Portal`/`Label`/`Button`/etc.) under a Clay-owned reconciliation container; `SduiTreeUpdate` ops apply as Masonry widget mutations outside paint.
- The editor text surface remains bespoke immediate-mode rendering (unchanged), hosted as a child component in the reconciled tree via `EditorView`.
- Hand-rolled scroll/hit-test/focus/a11y code for migrated kinds is deleted; Masonry's passes provide those behaviors.
- `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and the Linux test suite (including `tests/ui_primitive_conformance.rs` and `tests/package_ui_conformance.rs`) pass at every task boundary.
- No new package-facing authority, raw CSS, client JS, or public op surface is introduced; new client internals stay `pub(crate)`.

## Tasks

- [x] Baseline, decision alignment, and Clay UI catalog review before UI work
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

- [x] Introduce the SDUI→Masonry reconciliation seam with legacy fallback coexistence
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

- [x] Migrate `label` and `statusItem` to retained Masonry `Label`
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

- [ ] Migrate `button` to retained Masonry `Button`
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

- [x] Migrate `flex` and `stack` containers to Masonry `Flex`/`ZStack`
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

- [ ] Migrate `panel` and unify slot geometry into one Clay-owned layout
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
  - Progress (foundation done this pass; shell-ownership pending task 6.5):
    - Done: SDUI-side slot geometry deduplicated to one mechanical source — `with_default_left_slot(layout, defaults, want_left)` in `src/masonry_sdui.rs`; `editor_region_for_document`, `sdui_slot_layout`, and `sdui_panel_slot_layout` now share it (each keeps its intentional gate). Behavior-preserving: all geometry tests green (`editor_region_*`, `slot_panel_contribution_*`, `transient_overlay_*`, `workspace_browser_reserves_left_slot_*`).
    - Remaining (task 6.5): make the shell's `WorkingAreaLayout` the single slot-geometry owner that places the region in fixed slots; delete the SDUI-side parallel slot helpers; `editor_main_rect` reads unified geometry. Blocked on the region rendering the sidebar (cutover).
    - Note: the three gates differ intentionally (editor main reserves on root-or-binding + width guard; panel sidebar on root only). Unifying the *gates* (not just the mechanics) is an edge-case behavior change needing a product decision; deliberately not done here.

- [x] Rendering cutover: host the reconciled region and move per-node rendering into the retained tree
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

- [ ] Migrate `list` and `scroll` to retained Masonry scrolling
  - Acceptance Criteria:
    - Functional: `list` renders rows (title + detail, selection, per-row interaction states) as retained children; `scroll` uses Masonry scrolling (`Portal`/scrollbar) instead of the hand-managed `scroll_offset`/`content_height`/`viewport_height` clamping; row activation emits the same inert intent.
    - Performance: Scroll within `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`; viewport-bounded; no full-list repaint per scroll tick.
    - Code Quality: Manual scroll state fields and clamp logic removed from `SduiNativeState` once served by Masonry scrolling.
    - Security: Row actions remain registered-command intents; no authority change.
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/widgets/portal.rs` (`Portal::new`, scrollbar mutators), `scroll_bar.rs`, `virtual_scroll.rs` (for large lists if needed).
      - `.agents/skills/clay-ui/references/components.md`: list row fill (`list_row_fill_color(state, selected)`), scroll chrome (`paint_scroll_chrome`).
    - Options Considered:
      - Keep manual scroll math: rejected — it is fragile and duplicates Masonry scrolling.
      - `Portal`-backed scrolling with `paint_scroll_chrome`-equivalent Masonry scrollbar: chosen; `virtual_scroll` only if a list proves large (ponytail: defer until measured).
    - Chosen Approach:
      - Map `list` to a scrollable column of row widgets; map `scroll` to `Portal`; reuse existing row styling tokens.
    - API Notes and Examples:
      ```rust
      Portal::new(NewWidget::new(rows_flex)) // Masonry-owned scroll + scrollbars
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: list/scroll mapping.
      - `src/masonry_sdui.rs`: remove `scroll_offset`/`content_height`/`viewport_height` manual path.
    - References:
      - `src/masonry_sdui.rs` (scroll fields at `:104`-`:110` region), `src/shell/primitives.rs` (`paint_scroll_chrome`).
  - Test Cases to Write:
    - List snapshot parity: rows/labels/details match `SduiObservableSnapshot.list_items`.
    - Scroll clamps at content bounds; row action intent unchanged.

- [ ] Migrate transient surfaces: `overlay`/`portal`, then `dropdown`/`collapse`/`modal`/`textInput`
  - Acceptance Criteria:
    - Functional: `overlay`/`portal` render through Masonry `Portal`/`ZStack` with anchor + dismissal + focus policy; `dropdown` (ComboBox role, ArrowUp/Down/Enter/Space), `collapse` (Group role, Enter/Space toggle), `modal` (Dialog role, Tab/Shift+Tab focus trap), and `textInput` (TextInput role, placeholder, validation-state border) keep their documented keyboard/focus behavior — now provided/composed via Masonry focus + Clay intent routing rather than hand-rolled `dropdown_selected`/`collapse_expanded`/`modal_focusable_intents` maps.
    - Performance: Transient paint/overlay z-order (`z.overlay`<`z.modal`<`z.tooltip`) preserved; no hot-path regression.
    - Code Quality: The client-local interaction maps and manual focus-trap/keyboard handling in `SduiNativeState` are removed as each kind migrates; this is the largest deletion of hand-rolled behavior.
    - Security: Focus trapping/dismissal/z-order remain Clay-owned; actions stay registered-command intents; `textInput` never grants authority via its value.
  - Approach:
    - Documentation Reviewed:
      - Masonry `src/widgets/portal.rs`, `zstack.rs`, `text_input.rs`, `checkbox.rs`; focus/event bubbling in `src/doc/implementing_widget.md`.
      - `.agents/skills/clay-ui/references/components.md`: Phase 20.5 overlay/menu/input notes (z-level stacking, `TransientMenuOrigin`, focus policies).
    - Options Considered:
      - Migrate transient family first (highest value): rejected as *first* step — depends on containers/scroll seam being stable; do last among component kinds.
      - Keep hand-rolled focus traps: rejected — this is the most bug-prone hand-rolled code and the strongest reason to adopt the substrate.
      - Masonry `Portal`/`ZStack`/`TextInput` + Clay focus/intent routing: chosen.
    - Chosen Approach:
      - Split into two sub-steps: (a) `overlay`/`portal` containers; (b) `dropdown`/`collapse`/`modal`/`textInput`. Reuse `TransientMenuSession`/`TransientPackageOverlay` state; replace manual keyboard/focus maps with Masonry focus traversal + existing intent emission.
    - API Notes and Examples:
      ```rust
      // overlay composition: ZStack layer + Portal-anchored content; focus via Masonry focus chain
      ```
    - Files to Create/Edit:
      - `src/masonry_sdui_region.rs`: overlay/portal/dropdown/collapse/modal/textInput mapping.
      - `src/masonry_sdui.rs`: remove `dropdown_selected`/`collapse_expanded`/`focused_action`/`modal_focusable_intents` manual handling as kinds migrate.
      - `src/shell/package_ui.rs` / `src/shell/transient_menu.rs`: keep state model; hand rendering to the region.
    - References:
      - `src/masonry_sdui.rs` (interaction maps at `:113`-`:120`), `src/shell/transient_menu.rs`.
  - Test Cases to Write:
    - Dropdown keyboard nav parity (cycle + confirm clears focus).
    - Modal Tab/Shift+Tab focus-trap cycles focusable intents.
    - Collapse toggle expands/collapses children.
    - textInput validation-state border + placeholder parity.
    - Overlay z-order stacking preserved.

- [ ] Host `EditorView` as a real child component and retire the `SduiNativeState` god-object
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

- [ ] Update the package UI/layout authoring contract and package guide
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

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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

- [ ] Create or verify Clay configuration APIs
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

- [ ] Update or verify the code wiki after implementation
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
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
