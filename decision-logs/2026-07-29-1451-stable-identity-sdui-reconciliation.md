---
date: 2026-07-29 14:51
status: approved
decision_about: "Stable-identity reconciliation for the SDUI retained Masonry subtree"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Stable-identity reconciliation for the SDUI Masonry subtree

## Decision

Plan 070 will add a dedicated stage that replaces the SDUI region's current **wholesale rebuild** with a **stable-identity reconciler**: on each SDUI update, diff the incoming tree against the live Masonry subtree and update widgets **in place**, so a node that still exists keeps its `WidgetId`. Because Masonry keys widget state by identity, persistent state — `Portal` scroll position (`viewport_pos`), keyboard focus (`RenderRoot` tracks `focused_widget` by `WidgetId`), and later dropdown/collapse/modal/textInput state — then survives server updates automatically. The stage begins with a throwaway spike that proves a stateful widget survives a region sync before any production refactor.

## Context

Since Step 8, `EditorWidget::sync_region` (`src/masonry_editor.rs:402`) feeds the reconciled sidebar by building a **brand-new** `SduiRegionWidget` and swapping its `WidgetPod` wholesale (`std::mem::replace` + `remove_child` + `children_changed`). Both `apply_snapshot` and `apply_update` call `mark_region_dirty`, so this rebuild runs on **every** server SDUI update. Every widget in the subtree therefore gets a fresh `WidgetId` on every update, and any state Masonry stores per-widget is discarded.

This is invisible for the kinds migrated so far: `button` (Step 9) and `list` rows (Step 10a) carry only transient hover/active state, so a mid-hover rebuild resetting it is unnoticeable. It becomes a defect the moment a widget holds **persistent** state:

- **Scroll (Step 10b).** Masonry `Portal` stores `viewport_pos` inside the widget. Under wholesale rebuild the sidebar would snap back to the top on every server SDUI update — a regression from today's hand-managed `scroll_offset`, which lives in `SduiNativeState` (outside the rebuilt subtree) and is re-read on every layout. This is why Step 10b was deferred.
- **Transient surfaces (Step 11).** A dropdown's open/closed + selected index, a collapse panel's expanded state, a modal's visibility, and a text input's cursor/selection/focus are all persistent. Every one of them resets under wholesale rebuild.

Stable-identity reconcile is therefore the gateway to both Step 10b and all of Step 11. Two facts make it tractable:

1. **The data model is already reconciler-ready.** `SduiNode` carries a stable `SduiNodeId(u64)` and the server already sends incremental ops (`SduiTreeOperation::ReplaceNode`/`RemoveNode`, plus root replacement) in `src/protocol/sdui.rs`. `SduiRegionWidget` already has `reconcile_snapshot` and an incremental `apply_update`. The diff *input* exists; the code currently discards it and rebuilds.
2. **Persistent state is already externalized.** `SduiNativeState` already stores `scroll_offset`, `focused_action`, `dropdown_selected: BTreeMap<u64, usize>`, and `collapse_expanded: BTreeSet<u64>` keyed by stable node id — the legacy solution to this exact problem, and a fallback lever.

## Approval

- Proposed by: agent (recommended "Option A" after presenting four options).
- Approved by user: Yes.
- Approval evidence: User replied "Let's go with Option A as you recommended. Before implementation update the plan document to reflect this separate stage with tasks and then order the rest of the tasks accordingly. Also create a decision log about this."

## Alternatives Considered

1. **External-state preservation (hybrid)** — keep each widget's persistent state in `SduiNativeState` (mostly already there) and restore it into freshly recreated widgets on rebuild (re-focus by node id, push `scroll_offset` into a new `Portal`, re-open the right dropdown). Smaller upfront, but it is per-widget plumbing that *fights* Masonry's internal state model (you must drive `Portal` programmatically and intercept its scroll rather than letting it own it), and it accumulates hack debt that the reconciler would later delete. Rejected as the primary path; retained only as a possible temporary unblock or fallback.
2. **Status quo — wholesale rebuild + hand-managed scroll** — works today and the sidebar is a stable plateau, but it provides no Masonry scrollbar, cannot serve Step 11's persistent-state widgets at all, and does not reach the plan's stated end-state ("one widget tree … reconciled from `SduiTree`"). Rejected.
3. **Accept the reset (no persistence)** — let scroll/focus/dropdown state reset on every update. A clear UX regression (sidebar jumps to top; open dropdowns collapse). Rejected.
4. **Stable-identity reconciler** — diff and update in place; persistent state survives because widgets are never recreated; no external bookkeeping or restore logic; pays off across Step 10b and every Step 11 surface; matches the approved Clay-owned retained-shell direction. Selected.

## Rationale and Evidence

- `src/masonry_editor.rs:402` (`sync_region`) confirms the wholesale swap: fresh `SduiRegionWidget`, `std::mem::replace` of the pod, `remove_child(old)`, `children_changed()`.
- `src/masonry_sdui.rs` `apply_snapshot` (`:371`) and `apply_update` (`:383`) both call `mark_region_dirty` (`:1109`), so a rebuild follows every server SDUI update.
- Masonry `widgets/portal.rs` stores `viewport_pos` internally and clips to its viewport; `RenderRoot` tracks the focused widget by `WidgetId`. Both are lost when the widget is recreated.
- `src/protocol/sdui.rs`: `SduiNodeId(pub u64)` (`:17`) is stable; `SduiTreeOperation::ReplaceNode`/`RemoveNode` (`:138`-`:139`) give incremental, id-keyed diffs.
- `src/masonry_sdui_region.rs` already exposes `reconcile_snapshot` (`:95`) and `apply_update` (`:115`); the incremental machinery is present but unused by the production wholesale path.
- `src/masonry_sdui.rs` already externalizes persistent interaction state: `scroll_offset` (`:98`), `focused_action` (`:111`), `dropdown_selected` (`:113`), `collapse_expanded` (`:115`).
- The reconciler is the single remaining piece that turns "mostly-reconciled SDUI" into a fully retained, Masonry-native tree, and it unblocks two deferred milestones at once.

Known costs (why a spike precedes the refactor):

- In-place prop updates need `WidgetMut` per descendant (i.e. `RenderRoot::edit_widget(child_id)`), orchestrated from `main.rs` — a different control flow than building a subtree inside the region, touching the ~15 update sites.
- Dynamic container children: `Flex`/`ZStack` take a fixed child list at construction; mutating a live container's child list has known Masonry gotchas (post-construction child-registration panics were hit earlier in this plan).
- Mutating the live widget graph is fiddly and must be proven cheaply before committing.

## References

- `plans/070-SDUI-Retained-Masonry-Reconciliation.md` — the plan this stage extends (Steps 10b and 11 were gated on it).
- `decision-logs/2026-06-09-1431-clay-owned-shell-layout-and-package-ui-contribution-model.md` — approved Clay-owned retained shell over Masonry as substrate; this decision furthers that direction.
- `src/masonry_editor.rs` (`sync_region`), `src/masonry_sdui.rs` (dirty-tracking + externalized state), `src/masonry_sdui_region.rs` (reconcile seam), `src/protocol/sdui.rs` (stable ids + incremental ops).
- Masonry 0.4.0 `widgets/portal.rs`, `core/widget.rs` (`find_widget_under_pointer`, focus by `WidgetId`).

## Consequences

- Positive: scroll position, focus, and transient-surface state persist across server SDUI updates; Steps 10b and 11 are unblocked; the plan reaches its "one widget tree, one event tree, one render pass" end-state; the externalized state maps in `SduiNativeState` become removable once the widgets own their state.
- Risks / follow-up: in-place mutation of the live Masonry graph is the main risk; a spike must prove a stateful widget survives a region sync and that container child-list mutation cooperates. The refactor changes how updates flow from `main.rs` (per-descendant `edit_widget`).
- Conditions to revisit: if Masonry's in-place mutation API proves too restrictive or fragile, fall back to Alternative 1 (external-state preservation) for the affected widgets; if measurement shows wholesale rebuild + external state is materially cheaper for the real update rates, reconsider scope.
