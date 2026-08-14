//! SDUI → Masonry reconciliation seam (plan 070, tasks 2/5/6/6.5).
//!
//! The SDUI data model (`crate::protocol::sdui`) is a retained, versioned tree
//! with incremental `SduiTreeUpdate` operations. Historically the client
//! immediate-mode painted that tree inside the leaf `SduiNativeState` widget,
//! laying out every kind in one shared `cursor_y` flow. This module introduces
//! a Clay-owned Masonry *container* that reconciles the SDUI tree into a real
//! nested Masonry widget subtree so Masonry provides layout, hit-testing,
//! focus, and accessibility instead of hand-rolled paint code.
//!
//! Mapping (the per-kind whitelist lives in [`SduiRegionWidget::build_node`]):
//! container kinds (`panel`/`flex`/`stack`) map to Masonry `Flex`/`ZStack`;
//! leaf kinds (`label`/`button`/`list`/`editorView`) map to [`SduiLabel`],
//! [`SduiButton`], [`SduiListRow`], and [`EditorViewWidget`] (plan 070 step 14),
//! a thin widget that reports the *exact* legacy row height and paints through
//! the *same* shared helpers (`paint_sdui_text`/`sdui_row_rect`/state fills) as
//! the legacy renderer. Because every kind maps to a widget, the reconciled
//! subtree is complete and renders identically to the legacy paint at Rest
//! state; the live routing (hosting at the sidebar rect, scroll clip, pointer
//! routing) is the rendering-cutover step (task 6.5 remainder).

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::mem::{Discriminant, discriminant};
use std::rc::Rc;

use masonry::accesskit::{Node, Role};
#[cfg(test)]
use masonry::core::WidgetId;
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, Axis, BoxConstraints, ChildrenIds, ComposeCtx, EventCtx, LayoutCtx,
    MutateCtx, NewWidget, NoAction, PaintCtx, PointerEvent, PropertiesMut, PropertiesRef,
    RegisterCtx, ScrollDelta, TextEvent, Update, UpdateCtx, Widget, WidgetMut, WidgetPod,
};
use masonry::kurbo::{Affine, Point, Rect, Size, Vec2};
use masonry::peniko::{Color, Fill};
use masonry::properties::types::{Length, UnitPoint};
use masonry::vello::Scene;
use masonry::widgets::{Flex, ZStack};

use crate::editor::typography::{TypographyRegistry, UiTextMetrics, UiTextVariant};
use crate::masonry_sdui::{paint_sdui_text, sdui_row_rect};
use crate::protocol::SduiVersion;
use crate::protocol::{
    FontRole, SduiActionIntent, SduiEditorBinding, SduiFlexDirection, SduiListItem, SduiNode,
    SduiNodeId, SduiNodeKind, SduiTree,
};
#[cfg(test)]
use crate::protocol::{SduiTreeOperation, SduiTreeUpdate};
use crate::shell::primitives::{PanelChrome, paint_panel_chrome, paint_scroll_chrome};
use crate::shell::theme::{ResolvedUiTheme, SduiThemeStyle};
use crate::shell::{
    InteractionState, component_state_color, disabled_text_color, list_row_fill_color,
    paint_focus_ring,
};

type ScrollSelectionTarget = Rc<Cell<Option<(f64, f64)>>>;

/// A child position key in a reconciled container's child list (plan 070 step
/// 11b). Real SDUI children key by [`SduiNodeId`]; synthetic children created by
/// a container's mapping (a panel's title row, a list's rows) key separately so
/// the keyed child-list diff can preserve their identity too.
#[derive(Clone, Debug, PartialEq)]
enum ChildKey {
    /// A real SDUI child node.
    Node(SduiNodeId),
    /// A panel's synthetic title row (always the panel container's first child).
    PanelTitle,
    /// A list's synthetic row, keyed by `SduiListItem::id`.
    ListRow(String),
}

/// The reconciled identity of one SDUI node (plan 070 step 11b): the Masonry
/// `WidgetId` its widget was built with (stable across in-place reconciles) plus
/// the `SduiNodeKind` discriminant it was built as (to detect kind changes,
/// which force a rebuild rather than an in-place prop update).
#[derive(Clone, Copy)]
struct PodRecord {
    #[cfg(test)]
    id: WidgetId,
    kind: Discriminant<SduiNodeKind>,
}

/// Clay-owned container reconciling an SDUI tree into a nested Masonry subtree.
///
/// Internal native implementation detail: packages never receive Masonry widget
/// IDs or handles. `pub(crate)` only; not a Clay JS API and has no facade/op.
pub(crate) struct SduiRegionWidget {
    ui_version: SduiVersion,
    root_id: Option<SduiNodeId>,
    nodes: BTreeMap<SduiNodeId, SduiNode>,
    root_pod: Option<WidgetPod<dyn Widget>>,
    /// Stable-identity map (plan 070 step 11b): every reconciled SDUI node → its
    /// widget identity + built kind. Surviving nodes keep their `WidgetId` across
    /// in-place reconciles, so Masonry-managed state (focus, and the
    /// `SduiScrollViewport` scroll / later transient open state) persists across
    /// server updates.
    pods: BTreeMap<SduiNodeId, PodRecord>,
    /// Per-container ordered child keys, parallel to each reconciled container's
    /// `Flex`/`ZStack` children vec. Drives the keyed child-list diff so
    /// add/remove (and same-order survival) reuse child pods in place.
    child_keys: BTreeMap<SduiNodeId, Vec<ChildKey>>,
    /// Render context cloned into each retained leaf (`SduiLabel`/`SduiButton`/
    /// `SduiListRow`/`EditorViewWidget`) so leaves paint with the active
    /// typography/theme. Fed by `SduiNativeState` at the cutover; defaults (base
    /// catalog) until then.
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl SduiRegionWidget {
    pub(crate) fn new() -> Self {
        Self {
            ui_version: 0,
            root_id: None,
            nodes: BTreeMap::new(),
            root_pod: None,
            pods: BTreeMap::new(),
            child_keys: BTreeMap::new(),
            typography: TypographyRegistry::default(),
            ui_theme: ResolvedUiTheme::default(),
        }
    }

    /// Install the active typography/theme fields. The live reconcile path
    /// (plan 070 step 11c) applies them to surviving widgets in place; the
    /// wholesale [`Self::reconcile_snapshot`] rebuilds separately. No rebuild
    /// here — rebuilding on every theme change would discard widget identity.
    pub(crate) fn set_render_context(
        &mut self,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) {
        self.typography = typography;
        self.ui_theme = ui_theme;
    }

    /// Replace the retained tree (data only) — shared by the fresh and live paths.
    fn set_snapshot_data(&mut self, tree: SduiTree) {
        self.ui_version = tree.ui_version;
        self.root_id = Some(tree.root_id);
        self.nodes.clear();
        for node in tree.nodes {
            self.nodes.insert(node.id, node);
        }
    }

    /// Apply incremental operations to the retained tree (data only). Returns
    /// `false` and leaves state untouched when `base_ui_version` is stale.
    #[cfg(test)]
    fn apply_update_data(&mut self, update: SduiTreeUpdate) -> bool {
        if update.base_ui_version != self.ui_version {
            return false;
        }
        for operation in update.operations {
            match operation {
                SduiTreeOperation::ReplaceRoot { root_id } => {
                    self.root_id = Some(root_id);
                }
                SduiTreeOperation::ReplaceNode { node } => {
                    self.nodes.insert(node.id, node);
                }
                SduiTreeOperation::RemoveNode { node_id } => {
                    self.nodes.remove(&node_id);
                }
            }
        }
        self.ui_version = update.new_ui_version;
        true
    }

    /// Replace the retained tree and rebuild the reconciled subtree wholesale
    /// (no live tree required). Standalone-test path and the current
    /// `sync_region` wholesale swap; the stable-identity in-place equivalent is
    /// [`Self::reconcile_snapshot_live`].
    #[cfg(test)]
    pub(crate) fn reconcile_snapshot(&mut self, tree: SduiTree) {
        self.set_snapshot_data(tree);
        self.rebuild();
    }

    /// Apply incremental operations, then rebuild the reconciled subtree
    /// wholesale. Returns `false` on a stale `base_ui_version`. Standalone-test
    /// path; the in-place equivalent is [`Self::apply_update_live`].
    #[cfg(test)]
    pub(crate) fn apply_update(&mut self, update: SduiTreeUpdate) -> bool {
        if !self.apply_update_data(update) {
            return false;
        }
        self.rebuild();
        true
    }

    /// Stable-identity snapshot reconcile against a live tree (plan 070 step
    /// 11b): reuses surviving widgets' `WidgetId`s instead of rebuilding.
    pub(crate) fn reconcile_snapshot_live(&mut self, ctx: &mut MutateCtx<'_>, tree: SduiTree) {
        self.set_snapshot_data(tree);
        self.reconcile(ctx);
    }

    /// Stable-identity incremental reconcile against a live tree (plan 070 step
    /// 11b): surviving nodes keep their `WidgetId` (and thus Masonry-managed
    /// state); only added/removed/changed nodes are touched. Returns `false` on
    /// a stale `base_ui_version`.
    #[cfg(test)]
    pub(crate) fn apply_update_live(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        update: SduiTreeUpdate,
    ) -> bool {
        if !self.apply_update_data(update) {
            return false;
        }
        self.reconcile(ctx);
        true
    }

    /// Drop the reconciled subtree, returning the region to its inert empty
    /// state (plan 070 step 11c). Used when the SDUI root disappears; a no-op
    /// when the region is already empty.
    pub(crate) fn clear_live(&mut self, ctx: &mut MutateCtx<'_>) {
        self.ui_version = 0;
        self.root_id = None;
        self.nodes.clear();
        self.reconcile(ctx);
    }

    #[cfg(test)]
    pub(crate) fn ui_version(&self) -> SduiVersion {
        self.ui_version
    }

    #[cfg(test)]
    pub(crate) fn root_id(&self) -> Option<SduiNodeId> {
        self.root_id
    }

    /// Whether the current tree reconciled into a Masonry subtree at all.
    #[cfg(test)]
    pub(crate) fn has_root_pod(&self) -> bool {
        self.root_pod.is_some()
    }

    /// The reconciled content root's `WidgetId` (the tree root inside the
    /// `SduiScrollViewport`), used by tests to traverse the reconciled subtree
    /// without picking up the scroll viewport wrapper itself.
    #[cfg(test)]
    pub(crate) fn content_root_pod_id(&self) -> Option<WidgetId> {
        self.root_id.and_then(|root_id| self.pod_id_for(root_id))
    }

    #[cfg(test)]
    pub(crate) fn root_pod_id(&self) -> Option<WidgetId> {
        self.root_pod.as_ref().map(|pod| pod.id())
    }

    /// The reconciled `WidgetId` for an SDUI node, if it reconciled (plan 070
    /// step 11b; tests assert stable identity across updates via this).
    #[cfg(test)]
    pub(crate) fn pod_id_for(&self, node_id: SduiNodeId) -> Option<WidgetId> {
        self.pods.get(&node_id).map(|record| record.id)
    }

    /// The region is inert (takes no space, intercepts no input) until it has a
    /// reconciled subtree.
    #[cfg(test)]
    fn is_inert(&self) -> bool {
        self.root_pod.is_none()
    }

    /// Wholesale fresh rebuild (no live tree): clears the identity maps and
    /// re-creates every pod via [`Self::build_node`], which repopulates them.
    #[cfg(test)]
    fn rebuild(&mut self) {
        self.pods.clear();
        self.child_keys.clear();
        let ui_theme = self.ui_theme.clone();
        self.root_pod = self
            .root_id
            .and_then(|root_id| self.build_node(root_id, 0))
            .map(|content| wrap_in_viewport(content, ui_theme).to_pod());
    }

    /// Recursively build one SDUI node into a fresh Masonry widget, recording
    /// each node's [`PodRecord`] in [`Self::pods`] and each container's child
    /// order in [`Self::child_keys`]. Shared by the wholesale rebuild and the
    /// "added node" branch of the in-place reconciler. `depth` mirrors the
    /// legacy renderer indentation: panel children indent one level deeper;
    /// flex/stack children keep the current depth.
    fn build_node(&mut self, node_id: SduiNodeId, depth: usize) -> Option<NewWidget<dyn Widget>> {
        let node = self.nodes.get(&node_id)?.clone();
        let kind = discriminant(&node.kind);
        let (new_widget, keys): (NewWidget<dyn Widget>, Vec<ChildKey>) = match &node.kind {
            SduiNodeKind::Flex {
                direction,
                children,
            } => {
                let mut flex = match direction {
                    SduiFlexDirection::Row => Flex::row(),
                    SduiFlexDirection::Column => Flex::column(),
                };
                // Masonry's `Flex` defaults to a 10px gap (`DEFAULT_GAP`); the
                // legacy renderer stacks children with no inter-child gap, and
                // the scroll-bounds walk (`collect_action_regions`) assumes zero
                // gap. A non-zero gap made the reconciled subtree taller than
                // the computed content height, so listings near the viewport
                // height overflowed but could not scroll. Zero it until a spacing
                // token is wired at cutover.
                flex = flex.with_gap(Length::ZERO);
                let mut keys = Vec::new();
                for child_id in children {
                    if let Some(child) = self.build_node(*child_id, depth) {
                        flex = flex.with_child(child);
                        keys.push(ChildKey::Node(*child_id));
                    }
                }
                (NewWidget::new(flex).erased(), keys)
            }
            SduiNodeKind::Stack { children } => {
                let mut stack = ZStack::new();
                let mut keys = Vec::new();
                for child_id in children {
                    if let Some(child) = self.build_node(*child_id, depth) {
                        stack = stack.with_child(child, UnitPoint::TOP_LEFT);
                        keys.push(ChildKey::Node(*child_id));
                    }
                }
                (NewWidget::new(stack).erased(), keys)
            }
            SduiNodeKind::Panel { title, children } => {
                // Panel renders as a column: a title leaf followed by children
                // indented one level deeper (mirrors legacy immediate-mode renderer).
                let mut column = Flex::column().with_gap(Length::ZERO);
                column = column.with_child(NewWidget::new(SduiLabel::panel_title(
                    title.clone(),
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                )));
                let mut keys = vec![ChildKey::PanelTitle];
                for child_id in children {
                    if let Some(child) = self.build_node(*child_id, depth + 1) {
                        column = column.with_child(child);
                        keys.push(ChildKey::Node(*child_id));
                    }
                }
                (NewWidget::new(column).erased(), keys)
            }
            // Button: a real interactive Masonry widget (plan 070 step 9).
            SduiNodeKind::Button { label, action } => (
                NewWidget::new(SduiButton::new(
                    label.clone(),
                    action.clone(),
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                ))
                .erased(),
                Vec::new(),
            ),
            // List: a column of retained row widgets (plan 070 step 10), keyed
            // by item id so rows keep their identity across in-place reconciles.
            SduiNodeKind::List { items } => {
                let mut column = Flex::column().with_gap(Length::ZERO);
                let mut keys = Vec::new();
                for item in items {
                    column = column.with_child(NewWidget::new(SduiListRow::new(
                        item.label.clone(),
                        item.detail.clone(),
                        item.action.clone(),
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    )));
                    keys.push(ChildKey::ListRow(item.id.clone()));
                }
                (NewWidget::new(column).erased(), keys)
            }
            // Leaf kinds: `Label` -> `SduiLabel`, `EditorView` ->
            // `EditorViewWidget` (plan 070 step 14). The editor canvas itself
            // stays bespoke-painted by `EditorWidget` (hot path unchanged);
            // `EditorViewWidget` is the binding/slot component in the tree.
            SduiNodeKind::Label { text } => (
                NewWidget::new(SduiLabel::new(
                    text.clone(),
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                ))
                .erased(),
                Vec::new(),
            ),
            SduiNodeKind::EditorView { binding } => (
                NewWidget::new(EditorViewWidget::new(
                    binding.clone(),
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                ))
                .erased(),
                Vec::new(),
            ),
        };
        self.pods.insert(
            node_id,
            PodRecord {
                #[cfg(test)]
                id: new_widget.id(),
                kind,
            },
        );
        self.child_keys.insert(node_id, keys);
        Some(new_widget)
    }

    /// Stable-identity reconcile against a live tree (plan 070 step 11b).
    ///
    /// Reuses the existing root pod when the root node's kind is unchanged
    /// (mutating it in place via [`Self::reconcile_node`]); otherwise builds a
    /// fresh subtree and swaps it. Finishes by garbage-collecting identity
    /// records for nodes no longer reachable from the root.
    fn reconcile(&mut self, ctx: &mut MutateCtx<'_>) {
        let Some(root_id) = self.root_id else {
            if let Some(old) = self.root_pod.take() {
                ctx.remove_child(old);
                ctx.children_changed();
            }
            self.pods.clear();
            self.child_keys.clear();
            return;
        };
        let reusable = self.root_pod.is_some()
            && self.pods.get(&root_id).map(|record| record.kind)
                == self
                    .nodes
                    .get(&root_id)
                    .map(|node| discriminant(&node.kind));
        if reusable {
            // Take the pod out so `reconcile_node` can borrow `self` freely; the
            // reused pod is restored afterwards (its `WidgetId` is unchanged).
            // The root pod is the `SduiScrollViewport`, so reconcile the content
            // subtree through its child.
            let mut pod = self.root_pod.take().expect("root pod present");
            {
                let mut viewport_widget = ctx.get_mut(&mut pod);
                if let Some(mut viewport) = viewport_widget.try_downcast::<SduiScrollViewport>() {
                    // Sync the theme so the scrollbar repaints on theme change;
                    // the viewport is scroll chrome, not part of the reconciled
                    // tree, so `reconcile_node` never visits it.
                    viewport.widget.ui_theme = self.ui_theme.clone();
                    viewport.ctx.request_render();
                    let content = SduiScrollViewport::content_mut(&mut viewport);
                    self.reconcile_node(content, root_id, 0);
                }
            }
            self.root_pod = Some(pod);
        } else {
            if let Some(old) = self.root_pod.take() {
                ctx.remove_child(old);
            }
            self.pods.clear();
            self.child_keys.clear();
            let ui_theme = self.ui_theme.clone();
            self.root_pod = self
                .build_node(root_id, 0)
                .map(|content| wrap_in_viewport(content, ui_theme).to_pod());
            ctx.children_changed();
        }
        self.gc(root_id);
    }

    /// Reconcile one surviving node in place, given a `WidgetMut` to its widget.
    /// Leaf kinds update their props; container kinds reconcile their child list.
    fn reconcile_node(
        &mut self,
        mut widget: WidgetMut<'_, dyn Widget>,
        node_id: SduiNodeId,
        depth: usize,
    ) {
        let Some(node) = self.nodes.get(&node_id).cloned() else {
            return;
        };
        match &node.kind {
            SduiNodeKind::Label { text } => {
                if let Some(mut label) = widget.try_downcast::<SduiLabel>() {
                    label.widget.update_from(
                        text.clone(),
                        depth,
                        false,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    );
                    label.ctx.request_layout();
                }
            }
            SduiNodeKind::EditorView { binding } => {
                if let Some(mut view) = widget.try_downcast::<EditorViewWidget>() {
                    view.widget.update_from(
                        binding.clone(),
                        depth,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    );
                    view.ctx.request_layout();
                }
            }
            SduiNodeKind::Button { label, action } => {
                if let Some(mut button) = widget.try_downcast::<SduiButton>() {
                    button.widget.label = label.clone();
                    button.widget.intent = action.clone();
                    button.widget.depth = depth;
                    button.widget.typography = self.typography.clone();
                    button.widget.ui_theme = self.ui_theme.clone();
                    button.ctx.request_layout();
                }
            }
            SduiNodeKind::Flex {
                direction,
                children,
            } => {
                if let Some(mut flex) = widget.try_downcast::<Flex>() {
                    let axis = match direction {
                        SduiFlexDirection::Row => Axis::Horizontal,
                        SduiFlexDirection::Column => Axis::Vertical,
                    };
                    Flex::set_direction(&mut flex, axis);
                    let keys = children.iter().map(|c| ChildKey::Node(*c)).collect();
                    self.reconcile_flex_children(&mut flex, node_id, keys, depth, false);
                }
            }
            SduiNodeKind::Stack { children } => {
                if let Some(mut stack) = widget.try_downcast::<ZStack>() {
                    let keys = children.iter().map(|c| ChildKey::Node(*c)).collect();
                    self.reconcile_zstack_children(&mut stack, node_id, keys, depth);
                }
            }
            SduiNodeKind::Panel { children, .. } => {
                if let Some(mut flex) = widget.try_downcast::<Flex>() {
                    let mut keys = vec![ChildKey::PanelTitle];
                    keys.extend(children.iter().map(|c| ChildKey::Node(*c)));
                    self.reconcile_flex_children(&mut flex, node_id, keys, depth, true);
                }
            }
            SduiNodeKind::List { items } => {
                if let Some(mut flex) = widget.try_downcast::<Flex>() {
                    let keys = items
                        .iter()
                        .map(|item| ChildKey::ListRow(item.id.clone()))
                        .collect();
                    self.reconcile_flex_children(&mut flex, node_id, keys, depth, false);
                }
            }
        }
    }

    /// Reconcile a `Flex` container's child list in place, reusing surviving
    /// child pods. Same-order survivors mutate in place; added keys insert
    /// fresh pods; removed keys are destroyed.
    ///
    /// ponytail: a survivor that must *move* (reorder) is removed and re-inserted
    /// fresh — Masonry's `Flex` has no move-child API, so reorder loses that
    /// node's identity. Pure add/remove preserves identity for every survivor.
    fn reconcile_flex_children(
        &mut self,
        flex: &mut WidgetMut<'_, Flex>,
        container_id: SduiNodeId,
        new_keys: Vec<ChildKey>,
        depth: usize,
        container_is_panel: bool,
    ) {
        let old_keys = self
            .child_keys
            .get(&container_id)
            .cloned()
            .unwrap_or_default();

        if old_keys == new_keys {
            for (i, key) in new_keys.iter().enumerate() {
                let child_depth = Self::child_depth(key, depth, container_is_panel);
                if let Some(child) = Flex::child_mut(flex, i) {
                    self.reconcile_child_in_place(child, container_id, key, child_depth);
                }
            }
            return;
        }

        let mut current = old_keys;
        // Remove children absent from the new list (reverse index order).
        for i in (0..current.len()).rev() {
            if !new_keys.contains(&current[i]) {
                Flex::remove_child(flex, i);
                current.remove(i);
            }
        }
        // Walk the new list: keep in-place survivors, insert/move the rest.
        for (target, key) in new_keys.iter().enumerate() {
            let child_depth = Self::child_depth(key, depth, container_is_panel);
            if current.get(target) == Some(key) {
                if let Some(child) = Flex::child_mut(flex, target) {
                    self.reconcile_child_in_place(child, container_id, key, child_depth);
                }
            } else if let Some(from) = current.iter().position(|k| k == key) {
                // Reordered survivor: Masonry cannot move it, so rebuild it.
                Flex::remove_child(flex, from);
                current.remove(from);
                let child = self.build_child(container_id, key, child_depth);
                Flex::insert_child(flex, target, child);
                current.insert(target, key.clone());
            } else {
                let child = self.build_child(container_id, key, child_depth);
                Flex::insert_child(flex, target, child);
                current.insert(target, key.clone());
            }
        }
        self.child_keys.insert(container_id, new_keys);
    }

    /// Reconcile a `ZStack` container's child list.
    ///
    /// ponytail: Masonry's `ZStack` only appends (`insert_child` has no index),
    /// so a positional diff is impossible. Same-order survivors reconcile in
    /// place; any structural change rebuilds the stack's children wholesale
    /// (identity lost). `stack` is rare in SDUI (the sidebar is Flex/Panel/List),
    /// so this stays simple rather than emulating inserts via remove-tail.
    fn reconcile_zstack_children(
        &mut self,
        stack: &mut WidgetMut<'_, ZStack>,
        container_id: SduiNodeId,
        new_keys: Vec<ChildKey>,
        depth: usize,
    ) {
        let old_keys = self
            .child_keys
            .get(&container_id)
            .cloned()
            .unwrap_or_default();
        if old_keys == new_keys {
            for (i, key) in new_keys.iter().enumerate() {
                if let Some(child) = ZStack::child_mut(stack, i) {
                    self.reconcile_child_in_place(child, container_id, key, depth);
                }
            }
            return;
        }
        for i in (0..old_keys.len()).rev() {
            ZStack::remove_child(stack, i);
        }
        for key in &new_keys {
            let child = self.build_child(container_id, key, depth);
            ZStack::insert_child(stack, child, UnitPoint::TOP_LEFT);
        }
        self.child_keys.insert(container_id, new_keys);
    }

    /// The layout depth for a container child key: a panel's title sits at the
    /// panel's depth, its node children one deeper; everything else keeps depth.
    fn child_depth(key: &ChildKey, depth: usize, container_is_panel: bool) -> usize {
        match key {
            ChildKey::PanelTitle => depth,
            ChildKey::Node(_) if container_is_panel => depth + 1,
            ChildKey::Node(_) | ChildKey::ListRow(_) => depth,
        }
    }

    /// Reconcile one container child in place: a real node recurses into
    /// [`Self::reconcile_node`]; a synthetic child (panel title / list row)
    /// updates its props from the parent node's data.
    fn reconcile_child_in_place(
        &mut self,
        mut child: WidgetMut<'_, dyn Widget>,
        container_id: SduiNodeId,
        key: &ChildKey,
        depth: usize,
    ) {
        match key {
            ChildKey::Node(id) => self.reconcile_node(child, *id, depth),
            ChildKey::PanelTitle => {
                let title = match self.nodes.get(&container_id) {
                    Some(SduiNode {
                        kind: SduiNodeKind::Panel { title, .. },
                        ..
                    }) => title.clone(),
                    _ => return,
                };
                if let Some(mut label) = child.try_downcast::<SduiLabel>() {
                    label.widget.update_from(
                        title,
                        depth,
                        true,
                        self.typography.clone(),
                        self.ui_theme.clone(),
                    );
                    label.ctx.request_layout();
                }
            }
            ChildKey::ListRow(item_id) => {
                let item = match self.nodes.get(&container_id) {
                    Some(SduiNode {
                        kind: SduiNodeKind::List { items, .. },
                        ..
                    }) => items.iter().find(|item| &item.id == item_id).cloned(),
                    _ => None,
                };
                let Some(item) = item else { return };
                if let Some(mut row) = child.try_downcast::<SduiListRow>() {
                    row.widget.label = item.label;
                    row.widget.detail = item.detail;
                    row.widget.action = item.action;
                    row.widget.depth = depth;
                    row.widget.typography = self.typography.clone();
                    row.widget.ui_theme = self.ui_theme.clone();
                    row.ctx.request_layout();
                }
            }
        }
    }

    /// Build a fresh child widget for a container child key: a real node
    /// recurses into [`Self::build_node`]; a synthetic child builds from the
    /// parent node's data.
    fn build_child(
        &mut self,
        container_id: SduiNodeId,
        key: &ChildKey,
        depth: usize,
    ) -> NewWidget<dyn Widget> {
        match key {
            ChildKey::Node(id) => self
                .build_node(*id, depth)
                .expect("child node present in its container"),
            ChildKey::PanelTitle => {
                let title = match self.nodes.get(&container_id) {
                    Some(SduiNode {
                        kind: SduiNodeKind::Panel { title, .. },
                        ..
                    }) => title.clone(),
                    _ => String::new(),
                };
                NewWidget::new(SduiLabel::panel_title(
                    title,
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                ))
                .erased()
            }
            ChildKey::ListRow(item_id) => {
                let item = match self.nodes.get(&container_id) {
                    Some(SduiNode {
                        kind: SduiNodeKind::List { items, .. },
                        ..
                    }) => items.iter().find(|item| &item.id == item_id).cloned(),
                    _ => None,
                };
                let item = item.unwrap_or(SduiListItem {
                    id: item_id.clone(),
                    label: String::new(),
                    detail: None,
                    action: None,
                });
                NewWidget::new(SduiListRow::new(
                    item.label,
                    item.detail,
                    item.action,
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                ))
                .erased()
            }
        }
    }

    /// Drop identity records for nodes no longer reachable from the root. The
    /// widgets themselves leave the live tree via the container child-list
    /// reconcile; this only cleans up bookkeeping (including subtrees orphaned
    /// by a `RemoveNode`).
    fn gc(&mut self, root_id: SduiNodeId) {
        let mut reachable = BTreeSet::new();
        let mut stack = vec![root_id];
        while let Some(id) = stack.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(node) = self.nodes.get(&id) {
                match &node.kind {
                    SduiNodeKind::Flex { children, .. }
                    | SduiNodeKind::Stack { children }
                    | SduiNodeKind::Panel { children, .. } => {
                        stack.extend(children.iter().copied());
                    }
                    SduiNodeKind::List { .. }
                    | SduiNodeKind::Button { .. }
                    | SduiNodeKind::Label { .. }
                    | SduiNodeKind::EditorView { .. } => {}
                }
            }
        }
        self.pods.retain(|id, _| reachable.contains(id));
        self.child_keys.retain(|id, _| reachable.contains(id));
    }
}

impl Widget for SduiRegionWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        if let Some(pod) = &mut self.root_pod {
            ctx.register_child(pod);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let Some(pod) = &mut self.root_pod else {
            return Size::ZERO;
        };
        // The region is the fixed sidebar frame: paint the panel chrome across
        // its full rect (see `paint`) and place the scrolling content below the
        // top `panel_padding` (plan 070 step 14 — chrome moved here from
        // `EditorWidget`/`SduiNativeState::paint_chrome`).
        let size = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(240.0, 600.0))
        };
        let padding = SduiThemeStyle::from_ui_theme(&self.ui_theme).panel_padding;
        let viewport_size = Size::new(size.width, (size.height - padding).max(1.0));
        let _ = ctx.run_layout(pod, &BoxConstraints::new(viewport_size, viewport_size));
        ctx.place_child(pod, Point::new(0.0, padding));
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        // Sidebar panel chrome (bg/border) paints as the fixed frame BEHIND the
        // scrolling content child (plan 070 step 14 — moved from
        // `SduiNativeState::paint_chrome`/`EditorWidget`). The scrollbar paints
        // in the scroll viewport's `post_paint` above the content.
        if self.root_pod.is_some() {
            paint_panel_chrome(
                scene,
                ctx.size().to_rect(),
                &PanelChrome {
                    title: None,
                    collapse: InteractionState::Rest,
                    resize: InteractionState::Rest,
                },
                &self.ui_theme,
            );
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Group
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label("Server-driven UI region");
        // Children flow from `children_ids` (the scroll-viewport subtree) so the
        // reconciled SDUI tree is reachable in the access tree with
        // Masonry-computed (scroll-aware) bounds.
    }

    fn children_ids(&self) -> ChildrenIds {
        match &self.root_pod {
            Some(pod) => ChildrenIds::from_slice(&[pod.id()]),
            None => ChildrenIds::new(),
        }
    }
}

/// Wrap the reconciled content subtree in a [`SduiScrollViewport`] (plan 070
/// step 12 rework). The region is laid out as a fixed viewport (sidebar width ×
/// height); the viewport owns the scroll position, clips the content, and paints
/// a theme-driven vertical scrollbar when the content overflows.
fn wrap_in_viewport(
    content: NewWidget<dyn Widget>,
    ui_theme: ResolvedUiTheme,
) -> NewWidget<dyn Widget> {
    NewWidget::new(SduiScrollViewport::new(content, ui_theme)).erased()
}

/// Scrollbar metrics mirroring the editor chrome (`editor/surface.rs`) so the
/// sidebar scrollbar renders with identical proportions.
const SDUI_SCROLLBAR_WIDTH: f64 = 8.0;
const SDUI_SCROLLBAR_MARGIN: f64 = 4.0;
const SDUI_SCROLLBAR_MIN_THUMB: f64 = 24.0;

/// A self-contained vertical scroll viewport that owns its scroll position and
/// paints a theme-driven scrollbar (plan 070 step 12 rework). Replaces the stock
/// Masonry `Portal` so the sidebar scrollbar follows Clay's active theme
/// (`surface.scrollbar` / `surface.scrollbar.track`) instead of Masonry's fixed
/// colors — the same "adopt Masonry for behavior, own the paint for theme"
/// pattern the other reconciled widgets (`SduiButton`, `SduiListRow`) already
/// use. Wheel scroll updates the clamped scroll offset; the content is clipped
/// to the viewport and translated in `compose`. Thumb-drag is deferred, matching
/// the editor scrollbar (`editor/surface.rs`).
pub(crate) struct SduiScrollViewport {
    child: WidgetPod<dyn Widget>,
    scroll_offset: f64,
    content_size: Size,
    ui_theme: ResolvedUiTheme,
    pointer_pos: Option<Point>,
    pointer_pressed: bool,
    selection_target: Option<ScrollSelectionTarget>,
}

impl SduiScrollViewport {
    pub(crate) fn new(child: NewWidget<dyn Widget>, ui_theme: ResolvedUiTheme) -> Self {
        Self::with_selection_target_option(child, ui_theme, None)
    }

    pub(crate) fn with_selection_target(
        child: NewWidget<dyn Widget>,
        ui_theme: ResolvedUiTheme,
        selection_target: ScrollSelectionTarget,
    ) -> Self {
        Self::with_selection_target_option(child, ui_theme, Some(selection_target))
    }

    fn with_selection_target_option(
        child: NewWidget<dyn Widget>,
        ui_theme: ResolvedUiTheme,
        selection_target: Option<ScrollSelectionTarget>,
    ) -> Self {
        Self {
            child: child.to_pod(),
            scroll_offset: 0.0,
            content_size: Size::ZERO,
            ui_theme,
            pointer_pos: None,
            pointer_pressed: false,
            selection_target,
        }
    }

    pub(crate) fn set_ui_theme(&mut self, ui_theme: ResolvedUiTheme) {
        self.ui_theme = ui_theme;
    }

    /// Mutable access to the content child, mirroring `Portal::child_mut`, so
    /// the reconciler can reach the subtree inside the viewport.
    pub(crate) fn content_mut<'w>(this: &'w mut WidgetMut<'_, Self>) -> WidgetMut<'w, dyn Widget> {
        this.ctx.get_mut(&mut this.widget.child)
    }

    #[cfg(test)]
    pub(crate) fn scroll_offset_for_test(&self) -> f64 {
        self.scroll_offset
    }

    fn max_scroll(&self, viewport_height: f64) -> f64 {
        (self.content_size.height - viewport_height).max(0.0)
    }

    fn track_rect(&self, viewport: Size) -> Rect {
        let x1 = viewport.width - SDUI_SCROLLBAR_MARGIN;
        let x0 = x1 - SDUI_SCROLLBAR_WIDTH;
        Rect::new(x0, 0.0, x1, viewport.height)
    }

    fn thumb_rect(&self, track: Rect) -> Option<Rect> {
        let viewport_height = track.height();
        let max_scroll = self.max_scroll(viewport_height);
        if max_scroll <= 0.0 || self.content_size.height <= 0.0 {
            return None;
        }
        let frac = (self.scroll_offset / max_scroll).clamp(0.0, 1.0);
        let ratio = (viewport_height / self.content_size.height).clamp(0.0, 1.0);
        let thumb_height =
            (ratio * viewport_height).max(SDUI_SCROLLBAR_MIN_THUMB.min(viewport_height));
        let thumb_y0 = track.y0 + frac * (viewport_height - thumb_height).max(0.0);
        Some(Rect::new(
            track.x0,
            thumb_y0,
            track.x1,
            thumb_y0 + thumb_height,
        ))
    }

    fn interaction_state(&self, track: Rect) -> InteractionState {
        let Some(point) = self.pointer_pos else {
            return InteractionState::Rest;
        };
        if !track.contains(point) {
            return InteractionState::Rest;
        }
        if self.pointer_pressed
            && self
                .thumb_rect(track)
                .is_some_and(|thumb| thumb.contains(point))
        {
            return InteractionState::Active;
        }
        InteractionState::Hover
    }
}

impl Widget for SduiScrollViewport {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.child);
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Scroll(scroll) => {
                let delta_y = match scroll.delta {
                    ScrollDelta::PixelDelta(position) => {
                        -position.to_logical::<f64>(ctx.get_scale_factor()).y
                    }
                    ScrollDelta::LineDelta(_, y) => -(y as f64) * 120.0,
                    ScrollDelta::PageDelta(_, y) => -(y as f64) * ctx.size().height,
                };
                let max = self.max_scroll(ctx.size().height);
                let new_offset = (self.scroll_offset + delta_y).clamp(0.0, max);
                if (new_offset - self.scroll_offset).abs() > f64::EPSILON {
                    self.scroll_offset = new_offset;
                    ctx.request_compose();
                    ctx.request_render();
                    ctx.set_handled();
                }
            }
            PointerEvent::Move(update) => {
                self.pointer_pos = Some(ctx.local_position(update.current.position));
                ctx.request_render();
            }
            PointerEvent::Down(..) => {
                self.pointer_pressed = true;
                ctx.request_render();
            }
            PointerEvent::Up(..) => {
                self.pointer_pressed = false;
                ctx.request_render();
            }
            PointerEvent::Cancel(..) | PointerEvent::Leave(..) => {
                self.pointer_pos = None;
                self.pointer_pressed = false;
                ctx.request_render();
            }
            _ => (),
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let viewport = bc.max();
        let child_bc = BoxConstraints::new(Size::ZERO, viewport);
        self.content_size = ctx.run_layout(&mut self.child, &child_bc);
        let max = self.max_scroll(viewport.height);
        if let Some((target_y0, target_y1)) = self
            .selection_target
            .as_ref()
            .and_then(|target| target.get())
        {
            if target_y0 < self.scroll_offset {
                self.scroll_offset = target_y0;
            } else if target_y1 > self.scroll_offset + viewport.height {
                self.scroll_offset = target_y1 - viewport.height;
            }
        }
        self.scroll_offset = self.scroll_offset.clamp(0.0, max);
        ctx.set_clip_path(viewport.to_rect());
        ctx.place_child(&mut self.child, Point::ZERO);
        viewport
    }

    fn compose(&mut self, ctx: &mut ComposeCtx<'_>) {
        ctx.set_child_scroll_translation(&mut self.child, Vec2::new(0.0, -self.scroll_offset));
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {
        // The content child paints itself; the scrollbar overlays it in
        // `post_paint`.
    }

    fn post_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        scene: &mut Scene,
    ) {
        let track = self.track_rect(ctx.size());
        let Some(thumb) = self.thumb_rect(track) else {
            return;
        };
        let state = self.interaction_state(track);
        paint_scroll_chrome(scene, track, thumb, state, &self.ui_theme);
    }

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_clips_children();
        // Children flow from `children_ids` (the content subtree).
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.child.id()])
    }
}

/// Action emitted when a reconciled SDUI button is activated (click, Enter/Space,
/// or accessibility click).
///
/// Carries the inert [`SduiActionIntent`] declared on the SDUI `button` node, so
/// the app driver routes it through the existing server-first command path
/// (`enqueue_sdui_action`) with no widget-id → intent side channel. The intent is
/// unchanged from the legacy hit-test path: registered command id + bounded args,
/// no callback/op/native handle.
#[derive(Debug, Clone)]
pub struct SduiButtonPress {
    pub intent: SduiActionIntent,
}

/// A retained Masonry button reconciled from an SDUI `button` node (plan 070
/// step 9 — the first real interactive widget).
///
/// Reuses Masonry's pointer/focus/keyboard plumbing — the same `ctx` calls as the
/// stock `Button` (capture on press, submit on release-while-hovered, Enter/Space
/// when focused, click focus, `Role::Button`) — but paints through the legacy
/// shared helpers (`sdui_row_rect`/`component_state_color`/`paint_sdui_text`) so it
/// is pixel-identical to the immediate-mode renderer and styles all five
/// interaction states from tokens.
///
/// The stock `masonry::widgets::Button` was rejected for this seat: its
/// `ButtonPress` action carries no payload (forcing a widget-id → intent map that
/// the wholesale region rebuild would have to republish on every update), and its
/// property set has no hovered/focus background, so it cannot reproduce Clay's
/// per-state `surface.control` fills. This widget keeps Masonry's event/a11y
/// passes (no hand-rolled hit-test) while carrying the intent in its action.
pub(crate) struct SduiButton {
    label: String,
    intent: SduiActionIntent,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl SduiButton {
    fn new(
        label: String,
        intent: SduiActionIntent,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            label,
            intent,
            depth,
            typography,
            ui_theme,
        }
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(FontRole::Ui, self.style().body_text)
    }

    fn press(&self, ctx: &mut EventCtx<'_>) {
        ctx.submit_action::<SduiButtonPress>(SduiButtonPress {
            intent: self.intent.clone(),
        });
    }

    /// The interaction state painted this frame, derived from Masonry's tracked
    /// pointer/focus state (priority mirrors the legacy renderer:
    /// Disabled > Active > Focus > Hover > Rest).
    fn interaction_state(ctx: &PaintCtx<'_>) -> InteractionState {
        if ctx.is_disabled() {
            InteractionState::Disabled
        } else if ctx.is_active() {
            InteractionState::Active
        } else if ctx.is_focus_target() {
            InteractionState::Focus
        } else if ctx.is_hovered() {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        }
    }
}

impl Widget for SduiButton {
    type Action = SduiButtonPress;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(..) => {
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(..) => {
                if ctx.is_active() && ctx.is_hovered() {
                    self.press(ctx);
                }
                ctx.request_paint_only();
            }
            _ => (),
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(event)
                if event.state.is_up()
                    && (matches!(&event.key, Key::Character(c) if c == " ")
                        || event.key == Key::Named(NamedKey::Enter)) =>
            {
                self.press(ctx);
            }
            _ => (),
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if event.action == masonry::accesskit::Action::Click {
            self.press(ctx);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
            | Update::DisabledChanged(_) => {
                ctx.request_paint_only();
            }
            _ => {}
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        // Exact legacy button row height (layout-parity contract).
        let height = self.metrics().button_height();
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let padding = style.panel_padding;
        let width = ctx.size().width;
        let metrics = self.metrics();
        let height = metrics.button_height();
        let state = Self::interaction_state(ctx);
        let rect = sdui_row_rect(padding, self.depth, 0.0, width, 0.0, height);
        let fill = component_state_color(&self.ui_theme, "surface.control", state);
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
        let text_color = if state == InteractionState::Disabled {
            disabled_text_color(&self.ui_theme)
        } else {
            style.text_color
        };
        paint_sdui_text(
            &self.typography,
            padding,
            ctx,
            scene,
            &self.label,
            self.depth,
            (height - metrics.line_height) / 2.0,
            width,
            0.0,
            FontRole::Ui,
            metrics,
            text_color,
        );
        if state == InteractionState::Focus {
            paint_focus_ring(scene, rect, &self.ui_theme);
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::Button
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.label.clone());
        node.add_action(masonry::accesskit::Action::Click);
    }

    fn accepts_focus(&self) -> bool {
        // Buttons are tab/click focusable so Enter/Space activate them and the
        // focus ring shows; they are not text inputs.
        true
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// Action emitted when a reconciled SDUI list row is activated (click,
/// Enter/Space, or accessibility click).
///
/// Carries the inert [`SduiActionIntent`] declared on the list item, routed
/// through the same server-first command path as [`SduiButtonPress`].
#[derive(Debug, Clone)]
pub struct SduiListRowPress {
    pub intent: SduiActionIntent,
}

/// A retained Masonry list row reconciled from an SDUI `list` item (plan 070
/// step 10).
///
/// Mirrors [`SduiButton`]: reuses Masonry's pointer/focus/keyboard plumbing and
/// paints through the legacy shared helpers
/// (`sdui_row_rect`/`list_row_fill_color`/`paint_sdui_text`) so it is
/// pixel-identical to the immediate-mode renderer at Rest while adding per-row
/// hover/active/focus feedback. Rows without an action are inert (no focus, no
/// activation) but still repaint on hover.
pub(crate) struct SduiListRow {
    label: String,
    detail: Option<String>,
    action: Option<SduiActionIntent>,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl SduiListRow {
    fn new(
        label: String,
        detail: Option<String>,
        action: Option<SduiActionIntent>,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            label,
            detail,
            action,
            depth,
            typography,
            ui_theme,
        }
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn body_metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(FontRole::Ui, self.style().body_text)
    }

    fn detail_metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Detail)
    }

    fn row_height(&self) -> f64 {
        self.body_metrics().list_height(self.detail_metrics())
    }

    fn press(&self, ctx: &mut EventCtx<'_>) {
        if let Some(intent) = &self.action {
            ctx.submit_action::<SduiListRowPress>(SduiListRowPress {
                intent: intent.clone(),
            });
        }
    }

    /// The interaction state painted this frame (priority mirrors the legacy
    /// renderer: Disabled > Active > Focus > Hover > Rest).
    fn interaction_state(ctx: &PaintCtx<'_>) -> InteractionState {
        if ctx.is_disabled() {
            InteractionState::Disabled
        } else if ctx.is_active() {
            InteractionState::Active
        } else if ctx.is_focus_target() {
            InteractionState::Focus
        } else if ctx.is_hovered() {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        }
    }
}

impl Widget for SduiListRow {
    type Action = SduiListRowPress;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        // Inert rows (no action) still repaint on hover via `update`, but only
        // actionable rows capture/activate.
        if self.action.is_none() {
            return;
        }
        match event {
            PointerEvent::Down(..) => {
                ctx.capture_pointer();
                ctx.request_paint_only();
            }
            PointerEvent::Up(..) => {
                if ctx.is_active() && ctx.is_hovered() {
                    self.press(ctx);
                }
                ctx.request_paint_only();
            }
            _ => (),
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        match event {
            TextEvent::Keyboard(event)
                if event.state.is_up()
                    && (matches!(&event.key, Key::Character(c) if c == " ")
                        || event.key == Key::Named(NamedKey::Enter)) =>
            {
                self.press(ctx);
            }
            _ => (),
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if event.action == masonry::accesskit::Action::Click {
            self.press(ctx);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        match event {
            Update::HoveredChanged(_)
            | Update::ActiveChanged(_)
            | Update::FocusChanged(_)
            | Update::DisabledChanged(_) => {
                ctx.request_paint_only();
            }
            _ => {}
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        // Exact legacy list row height (layout-parity contract).
        let height = self.row_height();
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let padding = style.panel_padding;
        let width = ctx.size().width;
        let body = self.body_metrics();
        let detail = self.detail_metrics();
        let height = body.list_height(detail);
        let state = Self::interaction_state(ctx);
        let rect = sdui_row_rect(padding, self.depth, 0.0, width, 0.0, height);
        let fill = list_row_fill_color(&self.ui_theme, state, false);
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
        paint_sdui_text(
            &self.typography,
            padding,
            ctx,
            scene,
            &self.label,
            self.depth,
            0.0,
            width,
            0.0,
            FontRole::Ui,
            body,
            style.text_color,
        );
        if let Some(detail_text) = &self.detail {
            paint_sdui_text(
                &self.typography,
                padding,
                ctx,
                scene,
                detail_text,
                self.depth,
                body.line_height,
                width,
                0.0,
                FontRole::Ui,
                detail,
                style.muted_text_color,
            );
        }
        if state == InteractionState::Focus {
            paint_focus_ring(scene, rect, &self.ui_theme);
        }
    }

    fn accessibility_role(&self) -> Role {
        Role::ListItem
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.label.clone());
        if self.action.is_some() {
            node.add_action(masonry::accesskit::Action::Click);
        }
    }

    fn accepts_focus(&self) -> bool {
        // Only actionable rows are tab/click focusable (Enter/Space activate).
        self.action.is_some()
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// A leaf SDUI node rendered through the legacy paint helpers.
///
/// Reports the exact legacy row height in `layout` and paints via the same
/// shared code paths (`paint_sdui_text`/`sdui_row_rect`/state fills) as
/// the legacy immediate-mode renderer, so a reconciled subtree is pixel-identical to
/// the legacy renderer at Rest state. Interaction (hover/active/focus fills,
/// focus ring, action registration) stays on the legacy hit-test path until
/// each kind is swapped for a real Masonry widget (tasks 3/4/7).
pub(crate) struct SduiLabel {
    text: String,
    depth: usize,
    /// Panel title rows render with the `title_text` typography variant and
    /// primary text color (mirrors the legacy immediate-mode renderer Panel arm);
    /// plain labels use `body_text` + muted color.
    title: bool,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl SduiLabel {
    pub(crate) fn new(
        text: String,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            text,
            depth,
            title: false,
            typography,
            ui_theme,
        }
    }

    /// Panel title leaf (title typography variant + primary color).
    pub(crate) fn panel_title(
        text: String,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            text,
            depth,
            title: true,
            typography,
            ui_theme,
        }
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn metrics(&self, variant: UiTextVariant) -> UiTextMetrics {
        self.typography.ui_text_metrics(FontRole::Ui, variant)
    }

    fn label_presentation(&self) -> (UiTextVariant, Color) {
        let style = self.style();
        if self.title {
            (style.title_text, style.text_color)
        } else {
            (style.body_text, style.muted_text_color)
        }
    }

    /// The exact height the legacy immediate-mode renderer advances `cursor_y`
    /// by for a label row (the layout-parity contract with the legacy renderer).
    pub(crate) fn legacy_height(&self) -> f64 {
        let (variant, _) = self.label_presentation();
        self.metrics(variant).row_height
    }

    fn update_from(
        &mut self,
        text: String,
        depth: usize,
        title: bool,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) {
        self.text = text;
        self.depth = depth;
        self.title = title;
        self.typography = typography;
        self.ui_theme = ui_theme;
    }
}

impl Widget for SduiLabel {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let height = self.legacy_height();
        let width = if bc.is_width_bounded() {
            bc.max().width
        } else {
            0.0
        };
        bc.constrain(Size::new(width, height))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let style = self.style();
        let padding = style.panel_padding;
        let width = ctx.size().width;
        let (variant, color) = self.label_presentation();
        paint_sdui_text(
            &self.typography,
            padding,
            ctx,
            scene,
            &self.text,
            self.depth,
            0.0,
            width,
            0.0,
            FontRole::Ui,
            self.metrics(variant),
            color,
        );
    }

    fn accessibility_role(&self) -> Role {
        Role::Label
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.text.clone());
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

/// The `EditorView` SDUI node rendered as a retained widget (plan 070 step 14).
///
/// The real editor surface is bespoke-virtualized and stays painted by
/// `EditorWidget` (hot path unchanged — `concept.md` Phase 3). This widget is
/// the binding/slot component in the reconciled tree: it carries the
/// [`SduiEditorBinding`] (which document this view is), reports the editor a11y
/// label, and reserves zero width so the sidebar `Flex(Row)[panel, editor]` fits
/// the sidebar viewport (the editor canvas paints over the editor area to the
/// right of the sidebar).
pub(crate) struct EditorViewWidget {
    binding: SduiEditorBinding,
    depth: usize,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl EditorViewWidget {
    pub(crate) fn new(
        binding: SduiEditorBinding,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            binding,
            depth,
            typography,
            ui_theme,
        }
    }

    fn style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    fn metrics(&self) -> UiTextMetrics {
        self.typography
            .ui_text_metrics(FontRole::Ui, self.style().body_text)
    }

    /// The exact height the legacy immediate-mode renderer advances `cursor_y`
    /// by for an `EditorView` row (the layout-parity contract).
    pub(crate) fn legacy_height(&self) -> f64 {
        self.metrics().row_height
    }

    fn label(&self) -> String {
        format!("Editor view · doc {}", self.binding.document_id)
    }

    fn update_from(
        &mut self,
        binding: SduiEditorBinding,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) {
        self.binding = binding;
        self.depth = depth;
        self.typography = typography;
        self.ui_theme = ui_theme;
    }
}

impl Widget for EditorViewWidget {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        // Zero width: the real editor canvas is painted by `EditorWidget` to the
        // right of the sidebar; a non-zero width would make the reconciled root
        // `Flex(Row)[panel, editor]` wider than the sidebar viewport and enable
        // an unwanted horizontal scroll range.
        bc.constrain(Size::new(0.0, self.legacy_height()))
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {
        // Placeholder — the editor canvas is painted by `EditorWidget`.
    }

    fn accessibility_role(&self) -> Role {
        Role::Label
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(self.label());
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{SduiActionIntent, SduiActionSource, SduiEditorBinding, SduiListItem};
    use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
    use masonry::core::{MutateCtx, WidgetRef};
    use masonry::dpi::PhysicalSize;
    use masonry::theme::default_property_set;

    fn label_node(id: u64, text: &str) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::Label {
                text: text.to_string(),
            },
        )
    }

    fn panel_node(id: u64, title: &str, children: Vec<SduiNodeId>) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::Panel {
                title: title.to_string(),
                children,
            },
        )
    }

    fn flex_node(id: u64, direction: SduiFlexDirection, children: Vec<SduiNodeId>) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::Flex {
                direction,
                children,
            },
        )
    }

    fn button_node(id: u64) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::Button {
                label: "Apply".to_string(),
                action: SduiActionIntent::command(
                    "settings.setTypography".to_string(),
                    SduiActionSource::Button {
                        node_id: SduiNodeId(id),
                    },
                ),
            },
        )
    }

    fn list_node(id: u64, rows: usize) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::List {
                items: (0..rows)
                    .map(|i| SduiListItem {
                        id: format!("row{i}"),
                        label: format!("Row {i}"),
                        detail: Some(format!("detail {i}")),
                        action: None,
                    })
                    .collect(),
            },
        )
    }

    fn editor_view_node(id: u64) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id: 7,
                    expected_version: None,
                },
            },
        )
    }

    fn tree(root_id: u64, nodes: Vec<SduiNode>) -> SduiTree {
        SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(root_id),
            nodes,
        }
    }

    fn default_style() -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&ResolvedUiTheme::default())
    }

    fn render_root_options() -> RenderRootOptions {
        RenderRootOptions {
            default_properties: default_property_set().into(),
            use_system_fonts: false,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(900, 600),
            scale_factor: 1.0,
            test_font: None,
        }
    }

    #[test]
    fn every_kind_reconciles_into_a_root_pod() {
        for root in [
            label_node(1, "Open files"),
            panel_node(1, "Explorer", vec![]),
            flex_node(1, SduiFlexDirection::Column, vec![]),
            button_node(1),
            list_node(1, 3),
            editor_view_node(1),
        ] {
            let mut region = SduiRegionWidget::new();
            region.reconcile_snapshot(tree(1, vec![root]));
            assert!(region.has_root_pod(), "kind should reconcile");
            assert_eq!(region.children_ids().len(), 1);
        }
    }

    #[test]
    fn label_and_editor_view_heights_match_the_legacy_cursor_advances() {
        let typography = TypographyRegistry::default();
        let style = default_style();
        let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);

        let label = SduiLabel::new(
            "x".to_string(),
            0,
            typography.clone(),
            ResolvedUiTheme::default(),
        );
        assert!((label.legacy_height() - body.row_height).abs() < 1e-9);

        let title = SduiLabel::panel_title(
            "T".to_string(),
            0,
            typography.clone(),
            ResolvedUiTheme::default(),
        );
        let title_variant = SduiThemeStyle::from_ui_theme(&ResolvedUiTheme::default()).title_text;
        let title_metrics = typography.ui_text_metrics(FontRole::Ui, title_variant);
        assert!((title.legacy_height() - title_metrics.row_height).abs() < 1e-9);

        let view = EditorViewWidget::new(
            SduiEditorBinding {
                document_id: 7,
                expected_version: None,
            },
            0,
            typography.clone(),
            ResolvedUiTheme::default(),
        );
        assert!((view.legacy_height() - body.row_height).abs() < 1e-9);
    }

    #[test]
    fn settings_shaped_tree_with_all_leaf_kinds_reconciles_and_paints_without_panic() {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(
            1,
            vec![
                panel_node(
                    1,
                    "Settings",
                    vec![SduiNodeId(2), SduiNodeId(3), SduiNodeId(4), SduiNodeId(5)],
                ),
                label_node(2, "Theme"),
                button_node(3),
                list_node(4, 2),
                editor_view_node(5),
            ],
        ));
        assert!(region.has_root_pod());
        let root_pod_id = region.root_pod_id().unwrap();

        let mut render_root =
            RenderRoot::new(NewWidget::new(region), |_| {}, render_root_options());
        // Exercises the full container contract plus every legacy-leaf paint
        // path (label/button/list/editorView) through the render passes.
        let _ = render_root.redraw();
        assert!(render_root.has_widget(root_pod_id));
    }

    #[test]
    fn replace_node_rebuilds_the_subtree_and_version_advances() {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(1, vec![label_node(1, "before")]));
        assert!(region.has_root_pod());

        assert!(region.apply_update(SduiTreeUpdate {
            base_ui_version: 1,
            new_ui_version: 2,
            operations: vec![SduiTreeOperation::ReplaceNode {
                node: panel_node(1, "After", vec![]),
            }],
        }));
        assert_eq!(region.ui_version(), 2);
        assert!(region.has_root_pod());
    }

    #[test]
    fn remove_root_node_leaves_region_inert() {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(1, vec![label_node(1, "x")]));
        assert!(region.has_root_pod());

        assert!(region.apply_update(SduiTreeUpdate {
            base_ui_version: 1,
            new_ui_version: 2,
            operations: vec![SduiTreeOperation::RemoveNode {
                node_id: SduiNodeId(1),
            }],
        }));
        assert!(!region.has_root_pod());
        assert!(region.is_inert());
    }

    #[test]
    fn stale_update_base_version_is_rejected_without_state_change() {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(SduiTree {
            ui_version: 5,
            root_id: SduiNodeId(1),
            nodes: vec![label_node(1, "current")],
        });

        let accepted = region.apply_update(SduiTreeUpdate {
            base_ui_version: 4,
            new_ui_version: 6,
            operations: vec![SduiTreeOperation::RemoveNode {
                node_id: SduiNodeId(1),
            }],
        });

        assert!(!accepted);
        assert_eq!(region.ui_version(), 5);
        assert!(region.has_root_pod());
    }

    /// Recursively collect the laid-out heights of every leaf widget (a widget
    /// with no children) in traversal order.
    fn collect_leaf_heights(widget: WidgetRef<'_, dyn masonry::core::Widget>, out: &mut Vec<f64>) {
        let children = widget.children();
        if children.is_empty() {
            out.push(widget.ctx().size().height);
        } else {
            for child in children {
                collect_leaf_heights(child, out);
            }
        }
    }

    #[test]
    fn retained_layout_matches_legacy_row_geometry() {
        // Settings-shaped column: panel(title) > [label, button, list(2), label].
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(
            1,
            vec![
                panel_node(
                    1,
                    "Settings",
                    vec![SduiNodeId(2), SduiNodeId(3), SduiNodeId(4), SduiNodeId(5)],
                ),
                label_node(2, "Theme"),
                button_node(3),
                list_node(4, 2),
                label_node(5, "Footer"),
            ],
        ));
        // Traverse the reconciled *content* root (inside the `SduiScrollViewport`)
        // so the scroll-viewport wrapper doesn't pollute the leaf-height walk.
        let root_pod_id = region.content_root_pod_id().unwrap();
        let mut render_root =
            RenderRoot::new(NewWidget::new(region), |_| {}, render_root_options());
        let _ = render_root.redraw();

        let mut heights = Vec::new();
        {
            let root = render_root.get_widget(root_pod_id).unwrap();
            collect_leaf_heights(root, &mut heights);
        }

        // Expected heights mirror the legacy immediate-mode renderer cursor advances:
        // panel title (title variant) + label + button + two list rows + label.
        // The list reconciles to a column of per-row widgets (plan 070 step 10),
        // so it contributes one leaf height per row (each `list_height`), not a
        // single combined leaf.
        let typography = TypographyRegistry::default();
        let style = default_style();
        let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);
        let title = typography.ui_text_metrics(FontRole::Ui, style.title_text);
        let detail = typography.ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);
        let expected = [
            title.row_height,
            body.row_height,
            body.button_height(),
            body.list_height(detail),
            body.list_height(detail),
            body.row_height,
        ];

        assert_eq!(heights.len(), expected.len(), "leaf count mismatch");
        // Masonry pixel-snaps laid-out sizes/positions to integer device pixels
        // (`QueryCtx::size` is "adjusted for pixel snapping"), whereas the legacy
        // renderer advances `cursor_y` fractionally. Exact fractional parity is
        // therefore impossible by design; a 1px tolerance still catches real
        // modeling bugs (wrong variant/metric) while accepting sub-pixel snap.
        for (got, want) in heights.iter().zip(expected.iter()) {
            assert!((got - want).abs() <= 1.0, "got {got}, want {want}");
        }
    }

    #[test]
    fn empty_region_is_inert_and_claims_no_space() {
        let region = SduiRegionWidget::new();
        assert!(region.is_inert());
        assert!(!region.has_root_pod());
        assert_eq!(region.children_ids().len(), 0);
        assert_eq!(region.root_id(), None);
    }

    // ---- Step 7 (plan 070) z-order de-risk spike ----------------------------
    //
    // Proves the make-or-break constraint for Step 8 (host the reconciled region
    // as a real child of the main tree): that the compositor's z-order
    // (editor chrome < SDUI tree < package overlays) and pointer routing survive
    // the move off the nested render-only compositor.
    //
    // Authoritative source for the paint order (masonry_core 0.4.0
    // `src/passes/paint.rs::paint_widget`): a widget appends its own `paint()`
    // first, then its children in `children_ids` order, then its `post_paint()`.
    // Hit-testing walks the same tree front-first (`core/widget.rs`: "the last
    // child as determined by children_ids is chosen"), so hit-test order == paint
    // order. Consequences used below:
    //   * `ZStack` children stack back-to-front in insertion order (later = top).
    //   * A parent's `paint()` is below all children; `post_paint()` is above all
    //     children. `post_paint()` adds no widget, so it never intercepts pointers.
    //   * `Portal` is a scroll viewport (viewport_pos/scrollbars), NOT a z-layer —
    //     it is for Step 10 (list/scroll), not overlay z-order.

    /// Minimal fixed-size colored leaf used to observe stacking/hit-test order.
    struct ColorBox {
        color: Color,
        size: Size,
    }

    impl ColorBox {
        fn new(color: Color, size: Size) -> Self {
            Self { color, size }
        }
    }

    impl Widget for ColorBox {
        type Action = NoAction;

        fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

        fn layout(
            &mut self,
            _ctx: &mut LayoutCtx<'_>,
            _props: &mut PropertiesMut<'_>,
            bc: &BoxConstraints,
        ) -> Size {
            bc.constrain(self.size)
        }

        fn paint(
            &mut self,
            _ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            scene: &mut Scene,
        ) {
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                self.color,
                None,
                &self.size.to_rect(),
            );
        }

        fn accessibility_role(&self) -> Role {
            Role::GenericContainer
        }

        fn children_ids(&self) -> ChildrenIds {
            ChildrenIds::new()
        }

        fn accessibility(
            &mut self,
            _ctx: &mut AccessCtx<'_>,
            _props: &PropertiesRef<'_>,
            _node: &mut Node,
        ) {
        }
    }

    /// Step 8 host model: chrome in `paint()` (below the child), the reconciled
    /// region as the sole child, package overlays in `post_paint()` (above the
    /// child). Proves the child still receives pointers.
    struct SpikeHost {
        child: WidgetPod<dyn Widget>,
        overlay: Size,
    }

    impl Widget for SpikeHost {
        type Action = NoAction;

        fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
            ctx.register_child(&mut self.child);
        }

        fn layout(
            &mut self,
            ctx: &mut LayoutCtx<'_>,
            _props: &mut PropertiesMut<'_>,
            bc: &BoxConstraints,
        ) -> Size {
            let _ = ctx.run_layout(&mut self.child, bc);
            ctx.place_child(&mut self.child, Point::ZERO);
            if bc.is_width_bounded() && bc.is_height_bounded() {
                bc.max()
            } else {
                bc.constrain(Size::new(400.0, 400.0))
            }
        }

        fn paint(
            &mut self,
            _ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            scene: &mut Scene,
        ) {
            // Editor chrome: painted BELOW the region child.
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgb8(0x10, 0x10, 0x10),
                None,
                &Size::new(900.0, 600.0).to_rect(),
            );
        }

        fn post_paint(
            &mut self,
            _ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            scene: &mut Scene,
        ) {
            // Package overlay: painted ABOVE the region child, full-area so it
            // covers the probe point — yet must not intercept the pointer.
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgb8(0xff, 0x00, 0x00),
                None,
                &self.overlay.to_rect(),
            );
        }

        fn accessibility_role(&self) -> Role {
            Role::GenericContainer
        }

        fn children_ids(&self) -> ChildrenIds {
            ChildrenIds::from_slice(&[self.child.id()])
        }

        fn accessibility(
            &mut self,
            _ctx: &mut AccessCtx<'_>,
            _props: &PropertiesRef<'_>,
            _node: &mut Node,
        ) {
        }
    }

    #[test]
    fn spike_zstack_child_order_is_z_order_and_routes_pointer_to_top() {
        // Back-to-front: chrome, region, overlay. At an interior overlap point the
        // last-added child (overlay) must be topmost; with only [chrome, region]
        // the region must be topmost. Hit-test order == paint order, so this
        // proves the z-order the compositor relied on.
        let area = Size::new(400.0, 400.0);
        let probe = Point::new(50.0, 50.0);

        // Three children: overlay (last) is topmost.
        let chrome = NewWidget::new(ColorBox::new(Color::from_rgb8(0x10, 0x10, 0x10), area));
        let region = NewWidget::new(ColorBox::new(Color::from_rgb8(0x00, 0xff, 0x00), area));
        let overlay = NewWidget::new(ColorBox::new(Color::from_rgb8(0xff, 0x00, 0x00), area));
        let region_id = region.id();
        let overlay_id = overlay.id();
        let stack = ZStack::new()
            .with_child(chrome, UnitPoint::TOP_LEFT)
            .with_child(region, UnitPoint::TOP_LEFT)
            .with_child(overlay, UnitPoint::TOP_LEFT);
        let stack_new = NewWidget::new(stack);
        let stack_id = stack_new.id();
        let mut rr = RenderRoot::new(stack_new, |_| {}, render_root_options());
        let _ = rr.redraw();
        let hit = rr
            .get_widget(stack_id)
            .unwrap()
            .find_widget_under_pointer(probe)
            .expect("pointer over the stack must hit a child");
        assert_eq!(
            hit.id(),
            overlay_id,
            "last child (overlay) must paint/hit-test on top"
        );

        // Two children: region (last) is topmost over chrome.
        let chrome2 = NewWidget::new(ColorBox::new(Color::from_rgb8(0x10, 0x10, 0x10), area));
        let region2 = NewWidget::new(ColorBox::new(Color::from_rgb8(0x00, 0xff, 0x00), area));
        let region2_id = region2.id();
        let stack2 = ZStack::new()
            .with_child(chrome2, UnitPoint::TOP_LEFT)
            .with_child(region2, UnitPoint::TOP_LEFT);
        let stack2_new = NewWidget::new(stack2);
        let stack2_id = stack2_new.id();
        let mut rr2 = RenderRoot::new(stack2_new, |_| {}, render_root_options());
        let _ = rr2.redraw();
        let hit2 = rr2
            .get_widget(stack2_id)
            .unwrap()
            .find_widget_under_pointer(probe)
            .expect("pointer over the stack must hit a child");
        assert_eq!(hit2.id(), region2_id, "region must sit above chrome");
        let _ = region_id; // (used above via overlay ordering; keeps ids explicit)
    }

    #[test]
    fn spike_region_child_receives_pointer_under_post_paint_overlay() {
        // The exact Step 8 composition: chrome in paint() (below), the reconciled
        // SduiRegionWidget as the sole child, package overlays in post_paint()
        // (above, full-area). The pointer must still route INTO the region child —
        // proving neither the paint() chrome nor the post_paint() overlay blocks it
        // (post_paint adds no widget, so it never intercepts hit-testing).
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(
            1,
            vec![
                panel_node(1, "Settings", vec![SduiNodeId(2)]),
                button_node(2),
            ],
        ));
        let host = SpikeHost {
            child: NewWidget::new(region).erased().to_pod(),
            overlay: Size::new(900.0, 600.0),
        };
        let host_new = NewWidget::new(host);
        let host_id = host_new.id();
        let mut rr = RenderRoot::new(host_new, |_| {}, render_root_options());
        let _ = rr.redraw();

        // Probe inside the first leaf row (panel title spans the full width).
        let hit = rr
            .get_widget(host_id)
            .unwrap()
            .find_widget_under_pointer(Point::new(10.0, 10.0))
            .expect("pointer over the region must hit a descendant leaf");
        assert_ne!(
            hit.id(),
            host_id,
            "pointer must descend into the region child, not stop at the host \
             (chrome paint below / overlay post_paint above must not block it)"
        );
    }

    #[test]
    fn spike_persistent_region_rebuilds_across_redraws_without_panic() {
        // The Step 8 child-lifecycle gate: a persistent region child swaps its
        // reconciled subtree (brand-new pods) on every data update. Masonry only
        // re-runs register_children when `children_changed` is set, and orphaned
        // pods must go through `remove_child` (not be dropped). Prove the swap
        // through the live edit API does not panic the register pass and that the
        // rebuilt subtree still hit-tests.
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(
            1,
            vec![
                panel_node(1, "A", vec![SduiNodeId(2)]),
                label_node(2, "alpha"),
            ],
        ));
        let region_new = NewWidget::new(region);
        let region_id = region_new.id();
        let mut rr = RenderRoot::new(region_new, |_| {}, render_root_options());
        let _ = rr.redraw();

        // Data update #1: swap to a differently-shaped tree through the live tree.
        rr.edit_widget(region_id, |mut w| {
            let mut region = w.try_downcast::<SduiRegionWidget>().expect("region");
            let old = region.widget.root_pod.take();
            region.widget.reconcile_snapshot(tree(
                1,
                vec![
                    panel_node(1, "B", vec![SduiNodeId(2), SduiNodeId(3)]),
                    label_node(2, "beta"),
                    button_node(3),
                ],
            ));
            if let Some(old) = old {
                region.ctx.remove_child(old);
            }
            region.ctx.children_changed();
        });
        let _ = rr.redraw(); // must not panic on the register pass

        // Data update #2: swap back to a smaller tree (exercises pod removal).
        rr.edit_widget(region_id, |mut w| {
            let mut region = w.try_downcast::<SduiRegionWidget>().expect("region");
            let old = region.widget.root_pod.take();
            region
                .widget
                .reconcile_snapshot(tree(1, vec![label_node(1, "gamma")]));
            if let Some(old) = old {
                region.ctx.remove_child(old);
            }
            region.ctx.children_changed();
        });
        let _ = rr.redraw(); // must not panic

        // The rebuilt subtree still receives pointers.
        let hit = rr
            .get_widget(region_id)
            .unwrap()
            .find_widget_under_pointer(Point::new(10.0, default_style().panel_padding + 10.0))
            .expect("rebuilt region must still hit-test");
        assert_ne!(hit.id(), region_id, "must descend into a rebuilt leaf");
    }

    // --- Plan 070 step 11a: reconciler de-risk spike (disposable) -------------
    //
    // Proves the one thing the stable-identity reconciler (step 11b) depends on:
    // reusing a `WidgetPod` (stable `WidgetId`) across a reconcile keeps the
    // widget's state, while the current wholesale rebuild (fresh pod) discards
    // it. `SpikeHost` mirrors `SduiRegionWidget` (a parent holding child pods);
    // its two methods are the two strategies the reconciler chooses between.

    /// A leaf widget holding mutable state (the stand-in for Masonry-managed
    /// state such as a `Portal`'s `viewport_pos` or keyboard focus).
    struct SpikeStateful {
        counter: u32,
    }

    impl Widget for SpikeStateful {
        type Action = NoAction;

        fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

        fn layout(
            &mut self,
            _ctx: &mut LayoutCtx<'_>,
            _props: &mut PropertiesMut<'_>,
            _bc: &BoxConstraints,
        ) -> Size {
            Size::new(10.0, 10.0)
        }

        fn paint(
            &mut self,
            _ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            _scene: &mut Scene,
        ) {
        }

        fn accessibility_role(&self) -> Role {
            Role::Group
        }

        fn accessibility(
            &mut self,
            _ctx: &mut AccessCtx<'_>,
            _props: &PropertiesRef<'_>,
            _node: &mut Node,
        ) {
        }

        fn children_ids(&self) -> ChildrenIds {
            ChildrenIds::new()
        }
    }

    /// Minimal parent holding one `SpikeStateful` child.
    struct ReconcileSpikeHost {
        child: WidgetPod<SpikeStateful>,
    }

    impl ReconcileSpikeHost {
        fn new(counter: u32) -> Self {
            Self {
                child: NewWidget::new(SpikeStateful { counter }).to_pod(),
            }
        }

        /// Step 11b's chosen strategy: reuse the existing pod (stable `WidgetId`)
        /// and mutate the child in place. No new pod, no `children_changed`.
        fn reconcile_in_place(&mut self, ctx: &mut MutateCtx<'_>, add: u32) {
            let child = ctx.get_mut(&mut self.child);
            child.widget.counter += add;
        }

        /// The current production strategy: build a fresh pod (fresh `WidgetId`)
        /// and swap it in, discarding the old widget's accumulated state.
        fn rebuild_wholesale(&mut self, ctx: &mut MutateCtx<'_>) {
            let fresh = NewWidget::new(SpikeStateful { counter: 0 }).to_pod();
            let old = std::mem::replace(&mut self.child, fresh);
            ctx.remove_child(old);
            ctx.children_changed();
        }
    }

    impl Widget for ReconcileSpikeHost {
        type Action = NoAction;

        fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
            ctx.register_child(&mut self.child);
        }

        fn layout(
            &mut self,
            ctx: &mut LayoutCtx<'_>,
            _props: &mut PropertiesMut<'_>,
            bc: &BoxConstraints,
        ) -> Size {
            let size = ctx.run_layout(&mut self.child, bc);
            ctx.place_child(&mut self.child, Point::ZERO);
            size
        }

        fn paint(
            &mut self,
            _ctx: &mut PaintCtx<'_>,
            _props: &PropertiesRef<'_>,
            _scene: &mut Scene,
        ) {
        }

        fn accessibility_role(&self) -> Role {
            Role::Group
        }

        fn accessibility(
            &mut self,
            _ctx: &mut AccessCtx<'_>,
            _props: &PropertiesRef<'_>,
            _node: &mut Node,
        ) {
        }

        fn children_ids(&self) -> ChildrenIds {
            ChildrenIds::from_slice(&[self.child.id()])
        }
    }

    #[test]
    fn spike_stateful_widget_survives_inplace_reconcile() {
        // Reusing the pod (stable WidgetId) across a reconcile keeps the child's
        // state and does not panic the render passes.
        let host = ReconcileSpikeHost::new(0);
        let child_id_before = host.child.id();
        let host_new = NewWidget::new(host);
        let host_id = host_new.id();
        let mut rr = RenderRoot::new(host_new, |_| {}, render_root_options());
        let _ = rr.redraw();

        rr.edit_widget(host_id, |mut w| {
            let mut host = w.try_downcast::<ReconcileSpikeHost>().expect("host");
            host.widget.reconcile_in_place(&mut host.ctx, 5);
        });
        let _ = rr.redraw(); // must not panic

        rr.edit_widget(host_id, |mut w| {
            let mut host = w.try_downcast::<ReconcileSpikeHost>().expect("host");
            assert_eq!(
                host.widget.child.id(),
                child_id_before,
                "reused pod must keep its WidgetId"
            );
            let counter = host.ctx.get_mut(&mut host.widget.child).widget.counter;
            assert_eq!(counter, 5, "in-place reconcile must preserve state");
        });
    }

    #[test]
    fn spike_wholesale_rebuild_resets_state() {
        // The current production strategy: a fresh pod (fresh WidgetId) discards
        // the old widget's accumulated state. This is the bug step 11 fixes.
        let host = ReconcileSpikeHost::new(7);
        let child_id_before = host.child.id();
        let host_new = NewWidget::new(host);
        let host_id = host_new.id();
        let mut rr = RenderRoot::new(host_new, |_| {}, render_root_options());
        let _ = rr.redraw();

        rr.edit_widget(host_id, |mut w| {
            let mut host = w.try_downcast::<ReconcileSpikeHost>().expect("host");
            host.widget.rebuild_wholesale(&mut host.ctx);
        });
        let _ = rr.redraw(); // fresh pod registered via children_changed; must not panic

        rr.edit_widget(host_id, |mut w| {
            let mut host = w.try_downcast::<ReconcileSpikeHost>().expect("host");
            assert_ne!(
                host.widget.child.id(),
                child_id_before,
                "wholesale rebuild must mint a fresh WidgetId"
            );
            let counter = host.ctx.get_mut(&mut host.widget.child).widget.counter;
            assert_eq!(counter, 0, "wholesale rebuild discards accumulated state");
        });
    }

    // --- Plan 070 step 11b: stable-identity reconciler tests ----------------

    /// Host a freshly-reconciled region in a `RenderRoot` so the in-place
    /// `_live` reconcile runs against a live tree (the fresh `reconcile_snapshot`
    /// populates the identity maps; `redraw` registers the subtree).
    fn hosted_region(tree: SduiTree) -> (RenderRoot, WidgetId) {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree);
        let region_new = NewWidget::new(region);
        let region_id = region_new.id();
        let mut rr = RenderRoot::new(region_new, |_| {}, render_root_options());
        let _ = rr.redraw();
        (rr, region_id)
    }

    #[test]
    fn reconciled_containers_use_zero_gap_for_scroll_parity() {
        // Regression: Masonry `Flex` defaults to a 10px gap (`DEFAULT_GAP`). The
        // legacy scroll-bounds walk (`collect_action_regions`) stacks rows with
        // no gap, so a non-zero gap made the reconciled subtree taller than the
        // computed content height — a Workspace-sidebar listing near the viewport
        // height overflowed but could not scroll. The reconciled column height
        // must equal the sum of its row heights (zero gap).
        let rows = 10;
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(
            1,
            vec![
                panel_node(1, "Workspace", vec![SduiNodeId(2)]),
                list_node(2, rows),
            ],
        ));
        // Traverse the reconciled *content* root (inside the `SduiScrollViewport`)
        // — the panel — not the scroll-viewport wrapper itself.
        let root_pod_id = region.content_root_pod_id().unwrap();
        let mut render_root =
            RenderRoot::new(NewWidget::new(region), |_| {}, render_root_options());
        let _ = render_root.redraw();

        // The panel reconciles to `Flex(column)[panel_title, list]`; the list is
        // its second child. A `Flex` child gets `bc.min = 0` along the main axis,
        // so the list column reports its natural height — the sum of its rows.
        let panel = render_root.get_widget(root_pod_id).unwrap();
        let list_column = panel.children().into_iter().nth(1).expect("list column");
        let height = list_column.ctx().size().height;

        let typography = TypographyRegistry::default();
        let style = default_style();
        let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);
        let detail = typography.ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);
        let expected = rows as f64 * body.list_height(detail);

        assert!(
            (height - expected).abs() <= 1.0,
            "list column height {height} != zero-gap expectation {expected} (a Flex gap leaked in)"
        );
    }

    #[test]
    fn viewport_scrolls_sidebar_content_through_masonry_path() {
        // Plan 070 step 12 rework: the reconciled sidebar is a
        // `SduiScrollViewport`. A wheel scroll over the content pans it (the
        // content's window origin moves up), with no hand-managed
        // `SduiNativeState` scroll offset remaining.
        use masonry::core::{
            PointerEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState, PointerType,
            ScrollDelta,
        };
        use masonry::dpi::PhysicalPosition;

        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(
            1,
            vec![
                panel_node(1, "Workspace", vec![SduiNodeId(2)]),
                list_node(2, 50),
            ],
        ));
        let content_root = region.content_root_pod_id().unwrap();
        let mut options = render_root_options();
        options.size = PhysicalSize::new(240, 200);
        let mut rr = RenderRoot::new(NewWidget::new(region), |_| {}, options);
        let _ = rr.redraw();

        let y_before = rr
            .get_widget(content_root)
            .unwrap()
            .ctx()
            .to_window(Point::ZERO)
            .y;

        // The viewport negates the wheel delta, so a negative pixel delta pans
        // the viewport down (revealing later rows); the content translates up.
        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        rr.handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
            pointer: info,
            delta: ScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -120.0)),
            state: PointerState {
                position: PhysicalPosition::new(120.0, 100.0),
                ..Default::default()
            },
        }));
        let _ = rr.redraw();

        let y_after = rr
            .get_widget(content_root)
            .unwrap()
            .ctx()
            .to_window(Point::ZERO)
            .y;
        assert!(
            y_after < y_before - 1.0,
            "scroll should pan the content up (before {y_before}, after {y_after})"
        );
    }

    #[test]
    fn viewport_scroll_position_persists_across_sdui_updates() {
        // Plan 070 step 12 rework: the `SduiScrollViewport` wraps the reconciled
        // tree and survives in-place reconciles (Step 11c stable identity), so
        // its scroll position persists across server SDUI updates — the
        // regression that blocked this step under the wholesale region rebuild.
        use masonry::core::{
            PointerEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState, PointerType,
            ScrollDelta,
        };
        use masonry::dpi::PhysicalPosition;

        let (mut rr, region_id) = hosted_region(tree(
            1,
            vec![
                panel_node(1, "Workspace", vec![SduiNodeId(2), SduiNodeId(3)]),
                list_node(2, 50),
                label_node(3, "footer"),
            ],
        ));
        let content_root = region_pod_id(&mut rr, region_id, SduiNodeId(1)).unwrap();

        // Scroll down (negative pixel delta pans the viewport down).
        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        rr.handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
            pointer: info,
            delta: ScrollDelta::PixelDelta(PhysicalPosition::new(0.0, -120.0)),
            state: PointerState {
                position: PhysicalPosition::new(120.0, 100.0),
                ..Default::default()
            },
        }));
        let _ = rr.redraw();
        let y_scrolled = rr
            .get_widget(content_root)
            .unwrap()
            .ctx()
            .to_window(Point::ZERO)
            .y;
        assert!(
            y_scrolled < -1.0,
            "content should be translated up after scrolling, got {y_scrolled}"
        );

        // An unrelated update (replace the footer label) reconciles in place;
        // the viewport and its scroll position survive.
        let accepted = apply_live(
            &mut rr,
            region_id,
            SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: label_node(3, "footer v2"),
                }],
            },
        );
        assert!(accepted);

        let y_after = rr
            .get_widget(content_root)
            .unwrap()
            .ctx()
            .to_window(Point::ZERO)
            .y;
        assert!(
            (y_after - y_scrolled).abs() <= 1.0,
            "scroll position should persist across the update (scrolled {y_scrolled}, after {y_after})"
        );
    }

    #[test]
    fn viewport_scrollbar_thumb_tracks_scroll_and_overflow() {
        let mut viewport = SduiScrollViewport::new(
            NewWidget::new(SduiLabel::new(
                "x".to_string(),
                0,
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            ))
            .erased(),
            ResolvedUiTheme::default(),
        );
        let track = viewport.track_rect(Size::new(240.0, 200.0));

        // Content fits the viewport: no thumb.
        viewport.content_size = Size::new(240.0, 100.0);
        assert!(viewport.thumb_rect(track).is_none());

        // Content overflows: the thumb moves down as the scroll offset grows.
        viewport.content_size = Size::new(240.0, 1000.0);
        viewport.scroll_offset = 0.0;
        let top = viewport.thumb_rect(track).expect("scrollable");
        viewport.scroll_offset = 400.0;
        let scrolled = viewport.thumb_rect(track).expect("scrollable");
        assert!(
            scrolled.y0 > top.y0,
            "thumb should move down as scroll increases"
        );
        // At max scroll (1000 - 200 = 800) the thumb reaches the track bottom.
        viewport.scroll_offset = 800.0;
        let bottom = viewport.thumb_rect(track).expect("scrollable");
        assert!(
            (bottom.y1 - track.y1).abs() <= 1.0,
            "thumb should reach the track bottom at max scroll"
        );
    }

    #[test]
    fn viewport_scrollbar_interaction_state_tracks_pointer() {
        let mut viewport = SduiScrollViewport::new(
            NewWidget::new(SduiLabel::new(
                "x".to_string(),
                0,
                TypographyRegistry::default(),
                ResolvedUiTheme::default(),
            ))
            .erased(),
            ResolvedUiTheme::default(),
        );
        viewport.content_size = Size::new(240.0, 1000.0);
        viewport.scroll_offset = 0.0;
        let track = viewport.track_rect(Size::new(240.0, 200.0));

        assert_eq!(viewport.interaction_state(track), InteractionState::Rest);
        // Hover over the track.
        viewport.pointer_pos = Some(Point::new(track.x0 + 2.0, 100.0));
        assert_eq!(viewport.interaction_state(track), InteractionState::Hover);
        // Press over the thumb: Active.
        let thumb = viewport.thumb_rect(track).expect("scrollable");
        viewport.pointer_pos = Some(Point::new(thumb.x0 + 2.0, thumb.y0 + 2.0));
        viewport.pointer_pressed = true;
        assert_eq!(viewport.interaction_state(track), InteractionState::Active);
        // Pointer off the track: Rest.
        viewport.pointer_pos = Some(Point::new(10.0, 100.0));
        assert_eq!(viewport.interaction_state(track), InteractionState::Rest);
    }

    fn region_pod_id(
        rr: &mut RenderRoot,
        region_id: WidgetId,
        node_id: SduiNodeId,
    ) -> Option<WidgetId> {
        let mut result = None;
        rr.edit_widget(region_id, |mut w| {
            let region = w.try_downcast::<SduiRegionWidget>().expect("region");
            result = region.widget.pod_id_for(node_id);
        });
        result
    }

    fn apply_live(rr: &mut RenderRoot, region_id: WidgetId, update: SduiTreeUpdate) -> bool {
        let mut accepted = false;
        rr.edit_widget(region_id, |mut w| {
            let mut region = w.try_downcast::<SduiRegionWidget>().expect("region");
            accepted = region.widget.apply_update_live(&mut region.ctx, update);
        });
        let _ = rr.redraw();
        accepted
    }

    /// Read the root leaf's `SduiNodeKind` (test-only): takes the root pod out
    /// and restores it to obtain a live `WidgetMut`.
    fn root_leaf_kind(rr: &mut RenderRoot, region_id: WidgetId) -> Option<SduiNodeKind> {
        let mut kind = None;
        rr.edit_widget(region_id, |mut w| {
            let mut region = w.try_downcast::<SduiRegionWidget>().expect("region");
            let mut pod = region.widget.root_pod.take().expect("root pod");
            {
                // The root pod is the `SduiScrollViewport`; the reconciled tree
                // root is its content child.
                let mut viewport_widget = region.ctx.get_mut(&mut pod);
                if let Some(mut viewport) = viewport_widget.try_downcast::<SduiScrollViewport>() {
                    let mut content = SduiScrollViewport::content_mut(&mut viewport);
                    if let Some(label) = content.try_downcast::<SduiLabel>() {
                        kind = Some(SduiNodeKind::Label {
                            text: label.widget.text.clone(),
                        });
                    } else if let Some(view) = content.try_downcast::<EditorViewWidget>() {
                        kind = Some(SduiNodeKind::EditorView {
                            binding: view.widget.binding.clone(),
                        });
                    }
                }
            }
            region.widget.root_pod = Some(pod);
        });
        kind
    }

    #[test]
    fn stable_identity_nodes_keep_widget_ids_across_inplace_update() {
        let (mut rr, region_id) = hosted_region(tree(
            1,
            vec![
                panel_node(1, "Explorer", vec![SduiNodeId(2), SduiNodeId(3)]),
                label_node(2, "alpha"),
                button_node(3),
            ],
        ));
        let panel_before = region_pod_id(&mut rr, region_id, SduiNodeId(1)).unwrap();
        let label_before = region_pod_id(&mut rr, region_id, SduiNodeId(2)).unwrap();
        let button_before = region_pod_id(&mut rr, region_id, SduiNodeId(3)).unwrap();

        // Change only the label's text (same id, same kind).
        assert!(apply_live(
            &mut rr,
            region_id,
            SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: label_node(2, "beta"),
                }],
            },
        ));

        assert_eq!(
            region_pod_id(&mut rr, region_id, SduiNodeId(1)).unwrap(),
            panel_before,
            "untouched panel keeps its WidgetId"
        );
        assert_eq!(
            region_pod_id(&mut rr, region_id, SduiNodeId(2)).unwrap(),
            label_before,
            "label updated in place keeps its WidgetId"
        );
        assert_eq!(
            region_pod_id(&mut rr, region_id, SduiNodeId(3)).unwrap(),
            button_before,
            "untouched button keeps its WidgetId"
        );
    }

    #[test]
    fn stable_identity_preserves_focus_across_unrelated_update() {
        let (mut rr, region_id) = hosted_region(tree(
            1,
            vec![
                panel_node(1, "Explorer", vec![SduiNodeId(2), SduiNodeId(3)]),
                label_node(2, "alpha"),
                button_node(3),
            ],
        ));
        let button_id = region_pod_id(&mut rr, region_id, SduiNodeId(3)).unwrap();
        assert!(rr.focus_on(Some(button_id)), "button accepts focus");
        let _ = rr.redraw();
        assert_eq!(rr.focused_widget(), Some(button_id));

        // Unrelated update: change the label, not the button.
        assert!(apply_live(
            &mut rr,
            region_id,
            SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: label_node(2, "beta"),
                }],
            },
        ));

        // The button's WidgetId survived, so Masonry's focus (keyed by WidgetId)
        // is still on it — a wholesale rebuild would have dropped this.
        assert_eq!(
            region_pod_id(&mut rr, region_id, SduiNodeId(3)).unwrap(),
            button_id
        );
        assert_eq!(
            rr.focused_widget(),
            Some(button_id),
            "focus survives an unrelated update"
        );
    }

    #[test]
    fn container_child_list_add_remove_reorder_reconciles_correctly() {
        // Flex(1) > [label(2), label(3)].
        let (mut rr, region_id) = hosted_region(tree(
            1,
            vec![
                flex_node(
                    1,
                    SduiFlexDirection::Column,
                    vec![SduiNodeId(2), SduiNodeId(3)],
                ),
                label_node(2, "a"),
                label_node(3, "b"),
            ],
        ));
        let label2 = region_pod_id(&mut rr, region_id, SduiNodeId(2)).unwrap();

        // Remove label(3), add label(4): children become [2, 4].
        assert!(apply_live(
            &mut rr,
            region_id,
            SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![
                    SduiTreeOperation::ReplaceNode {
                        node: flex_node(
                            1,
                            SduiFlexDirection::Column,
                            vec![SduiNodeId(2), SduiNodeId(4)],
                        ),
                    },
                    SduiTreeOperation::RemoveNode {
                        node_id: SduiNodeId(3)
                    },
                    SduiTreeOperation::ReplaceNode {
                        node: label_node(4, "c")
                    },
                ],
            },
        ));
        assert_eq!(
            region_pod_id(&mut rr, region_id, SduiNodeId(2)).unwrap(),
            label2,
            "surviving label keeps its WidgetId through add/remove"
        );
        assert!(
            region_pod_id(&mut rr, region_id, SduiNodeId(4)).is_some(),
            "added label reconciled"
        );
        assert!(
            region_pod_id(&mut rr, region_id, SduiNodeId(3)).is_none(),
            "removed label's identity is garbage-collected"
        );

        // Reorder: children become [4, 2] — still correct, both present.
        assert!(apply_live(
            &mut rr,
            region_id,
            SduiTreeUpdate {
                base_ui_version: 2,
                new_ui_version: 3,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: flex_node(
                        1,
                        SduiFlexDirection::Column,
                        vec![SduiNodeId(4), SduiNodeId(2)],
                    ),
                }],
            },
        ));
        assert!(region_pod_id(&mut rr, region_id, SduiNodeId(4)).is_some());
        assert!(region_pod_id(&mut rr, region_id, SduiNodeId(2)).is_some());
    }

    #[test]
    fn prop_update_changes_label_text_without_recreating_the_widget() {
        let (mut rr, region_id) = hosted_region(tree(1, vec![label_node(1, "before")]));
        let label_before = region_pod_id(&mut rr, region_id, SduiNodeId(1)).unwrap();
        assert_eq!(
            root_leaf_kind(&mut rr, region_id),
            Some(SduiNodeKind::Label {
                text: "before".to_string()
            })
        );

        assert!(apply_live(
            &mut rr,
            region_id,
            SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: label_node(1, "after"),
                }],
            },
        ));

        // Same WidgetId (not recreated)...
        assert_eq!(
            region_pod_id(&mut rr, region_id, SduiNodeId(1)).unwrap(),
            label_before
        );
        // ...and the text actually changed in place.
        assert_eq!(
            root_leaf_kind(&mut rr, region_id),
            Some(SduiNodeKind::Label {
                text: "after".to_string()
            })
        );
    }

    #[test]
    fn sdui_button_action_emits_server_intent() {
        // Plan 070 step 9 parity gate: clicking the reconciled Masonry button
        // emits `SduiButtonPress` carrying the exact inert intent declared on the
        // SDUI node — the same intent the retired legacy hit-test enqueued.
        use masonry::app::RenderRootSignal;
        use masonry::core::{
            PointerButton, PointerButtonEvent, PointerEvent, PointerId, PointerInfo, PointerState,
            PointerType, PointerUpdate,
        };
        use masonry::dpi::PhysicalPosition;
        use std::cell::RefCell;
        use std::rc::Rc;

        let expected = SduiActionIntent::command(
            "settings.setTypography".to_string(),
            SduiActionSource::Button {
                node_id: SduiNodeId(1),
            },
        );
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(1, vec![button_node(1)]));

        let captured: Rc<RefCell<Vec<SduiActionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(press) = action.downcast::<SduiButtonPress>()
                {
                    sink.borrow_mut().push(press.intent.clone());
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();

        // The button is the region's root pod, filling the width at the top; click
        // its center row (Move sets hover, Down captures/activates, Up submits).
        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        let body =
            TypographyRegistry::default().ui_text_metrics(FontRole::Ui, default_style().body_text);
        let state = PointerState {
            position: PhysicalPosition::new(450.0, body.button_height() / 2.0),
            ..Default::default()
        };
        rr.handle_pointer_event(PointerEvent::Move(PointerUpdate {
            pointer: info,
            current: state.clone(),
            coalesced: vec![],
            predicted: vec![],
        }));
        rr.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state: state.clone(),
        }));
        rr.handle_pointer_event(PointerEvent::Up(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state,
        }));

        let captured = captured.borrow();
        assert_eq!(captured.len(), 1, "exactly one button action emitted");
        assert_eq!(captured[0], expected, "intent identical to the SDUI node's");
    }

    #[test]
    fn sdui_list_row_action_emits_server_intent() {
        // Plan 070 step 10 parity gate: clicking an actionable reconciled list
        // row emits `SduiListRowPress` carrying the exact inert intent declared
        // on the list item — the same intent the retired legacy hit-test
        // enqueued.
        use masonry::app::RenderRootSignal;
        use masonry::core::{
            PointerButton, PointerButtonEvent, PointerEvent, PointerId, PointerInfo, PointerState,
            PointerType, PointerUpdate,
        };
        use masonry::dpi::PhysicalPosition;
        use std::cell::RefCell;
        use std::rc::Rc;

        let expected = SduiActionIntent::command(
            "document.open".to_string(),
            SduiActionSource::ListItem {
                node_id: SduiNodeId(1),
                item_id: "row0".to_string(),
            },
        );
        let list = SduiNode::new(
            SduiNodeId(1),
            SduiNodeKind::List {
                items: vec![SduiListItem {
                    id: "row0".to_string(),
                    label: "Row 0".to_string(),
                    detail: Some("detail 0".to_string()),
                    action: Some(expected.clone()),
                }],
            },
        );
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(tree(1, vec![list]));

        let captured: Rc<RefCell<Vec<SduiActionIntent>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut rr = RenderRoot::new(
            NewWidget::new(region),
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(press) = action.downcast::<SduiListRowPress>()
                {
                    sink.borrow_mut().push(press.intent.clone());
                }
            },
            render_root_options(),
        );
        let _ = rr.redraw();

        // The list is the region's root pod; its single row fills the width at
        // the top. Click the row's label band (Move sets hover, Down
        // captures/activates, Up submits).
        let info = PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        };
        let body =
            TypographyRegistry::default().ui_text_metrics(FontRole::Ui, default_style().body_text);
        let state = PointerState {
            position: PhysicalPosition::new(
                450.0,
                default_style().panel_padding + body.line_height / 2.0,
            ),
            ..Default::default()
        };
        rr.handle_pointer_event(PointerEvent::Move(PointerUpdate {
            pointer: info,
            current: state.clone(),
            coalesced: vec![],
            predicted: vec![],
        }));
        rr.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state: state.clone(),
        }));
        rr.handle_pointer_event(PointerEvent::Up(PointerButtonEvent {
            pointer: info,
            button: Some(PointerButton::Primary),
            state,
        }));

        let captured = captured.borrow();
        assert_eq!(captured.len(), 1, "exactly one list row action emitted");
        assert_eq!(captured[0], expected, "intent identical to the list item's");
    }
}
