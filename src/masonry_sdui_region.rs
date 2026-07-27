#![allow(
    dead_code,
    reason = "SDUI reconciliation seam is staged for per-component migration (plan 070); it is exercised by its own tests before each kind is composed into the shell"
)]

//! SDUI → Masonry reconciliation seam (plan 070, task 2).
//!
//! The SDUI data model (`crate::protocol::sdui`) is a retained, versioned tree
//! with incremental `SduiTreeUpdate` operations. Historically the client
//! immediate-mode painted that tree inside the leaf `SduiNativeState` widget.
//! This module introduces a Clay-owned Masonry *container* that reconciles the
//! SDUI tree into real child `WidgetPod`s so Masonry provides layout,
//! hit-testing, focus, and accessibility instead of hand-rolled paint code.
//!
//! Migration is per-component-kind and controlled by [`is_reconciled`]. Kinds
//! not yet reconciled produce no pods here and keep rendering through the
//! legacy immediate-mode path, so the app stays fully functional at every step.
//! The region is not yet composed into the shell; composing it and deleting the
//! legacy paint for a kind happen together in that kind's migration task.

use std::collections::BTreeMap;

use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, BoxConstraints, ChildrenIds, LayoutCtx, NewWidget, NoAction, PaintCtx,
    PropertiesMut, PropertiesRef, RegisterCtx, Widget, WidgetPod,
};
use masonry::kurbo::{Point, Size};
use masonry::vello::Scene;
use masonry::widgets::Label;

use crate::protocol::{
    SduiNode, SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation, SduiTreeUpdate, SduiVersion,
};

/// Clay-owned container reconciling an SDUI tree into Masonry child widgets.
///
/// Internal native implementation detail: packages never receive Masonry widget
/// IDs or handles. `pub(crate)` only; not a Clay JS API and has no facade/op.
pub(crate) struct SduiRegionWidget {
    ui_version: SduiVersion,
    root_id: Option<SduiNodeId>,
    nodes: BTreeMap<SduiNodeId, SduiNode>,
    pods: BTreeMap<SduiNodeId, WidgetPod<dyn Widget>>,
}

impl SduiRegionWidget {
    pub(crate) fn new() -> Self {
        Self {
            ui_version: 0,
            root_id: None,
            nodes: BTreeMap::new(),
            pods: BTreeMap::new(),
        }
    }

    /// Replace the retained tree and rebuild child pods for reconciled kinds.
    ///
    /// Runs outside paint (called from the SDUI snapshot application path).
    /// Non-reconciled kinds produce no pod and remain on the legacy paint path.
    pub(crate) fn reconcile_snapshot(&mut self, tree: SduiTree) {
        self.ui_version = tree.ui_version;
        self.root_id = Some(tree.root_id);
        self.nodes.clear();
        self.pods.clear();
        for node in tree.nodes {
            if is_reconciled(&node.kind)
                && let Some(pod) = build_pod(&node)
            {
                self.pods.insert(node.id, pod);
            }
            self.nodes.insert(node.id, node);
        }
    }

    /// Apply incremental tree operations as Masonry pod mutations.
    ///
    /// Returns `false` and leaves state untouched when `base_ui_version` does
    /// not match the current version (stale update rejection). Runs outside
    /// paint.
    pub(crate) fn apply_update(&mut self, update: SduiTreeUpdate) -> bool {
        if update.base_ui_version != self.ui_version {
            return false;
        }
        for operation in update.operations {
            match operation {
                SduiTreeOperation::ReplaceRoot { root_id } => {
                    self.root_id = Some(root_id);
                }
                SduiTreeOperation::ReplaceNode { node } => {
                    if is_reconciled(&node.kind) {
                        if let Some(pod) = build_pod(&node) {
                            self.pods.insert(node.id, pod);
                        }
                    } else {
                        // Kind migrated back to the legacy path (or not yet
                        // reconciled): drop any stale pod so it stops rendering
                        // through the retained tree.
                        self.pods.remove(&node.id);
                    }
                    self.nodes.insert(node.id, node);
                }
                SduiTreeOperation::RemoveNode { node_id } => {
                    self.pods.remove(&node_id);
                    self.nodes.remove(&node_id);
                }
            }
        }
        self.ui_version = update.new_ui_version;
        true
    }

    pub(crate) fn ui_version(&self) -> SduiVersion {
        self.ui_version
    }

    pub(crate) fn root_id(&self) -> Option<SduiNodeId> {
        self.root_id
    }

    pub(crate) fn pod_count(&self) -> usize {
        self.pods.len()
    }

    pub(crate) fn has_pod(&self, node_id: SduiNodeId) -> bool {
        self.pods.contains_key(&node_id)
    }

    /// The region is inert (takes no space, intercepts no input) until it has
    /// reconciled children. Used by `layout` so an empty, not-yet-hosted region
    /// cannot overlap or shadow the editor.
    fn is_inert(&self) -> bool {
        self.pods.is_empty()
    }
}

/// Per-kind reconciliation whitelist. Grows one entry per migration task.
///
/// Migration order (plan 070): label/statusItem → button → flex/stack →
/// panel+slot-geometry → list/scroll → overlay/portal → dropdown/collapse/
/// modal/textInput → EditorView-as-child. Only `label` is reconciled here to
/// prove the container machinery end-to-end; it is not yet composed into the
/// shell, so visible rendering is unchanged.
fn is_reconciled(kind: &SduiNodeKind) -> bool {
    matches!(kind, SduiNodeKind::Label { .. })
}

/// Build the Masonry child pod for a reconciled node.
///
/// Provisional plain `Label` (no token styling yet); token-backed styling is
/// added in the label migration task. Returns `None` for kinds not reconciled.
fn build_pod(node: &SduiNode) -> Option<WidgetPod<dyn Widget>> {
    match &node.kind {
        SduiNodeKind::Label { text } => {
            Some(NewWidget::new(Label::new(text.clone())).erased().to_pod())
        }
        _ => None,
    }
}

impl Widget for SduiRegionWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        for pod in self.pods.values_mut() {
            ctx.register_child(pod);
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        if self.is_inert() {
            // Inert: claim no space so the not-yet-hosted region cannot overlap
            // or shadow the editor main region.
            return Size::ZERO;
        }
        // ponytail: provisional vertical stack at the origin. Real layout policy
        // is owned by the flex/stack/panel container kinds (plan 070 task 5/6);
        // this only proves the Masonry container contract (run_layout +
        // place_child for every registered child).
        let mut cursor_y = 0.0;
        let mut max_width: f64 = 0.0;
        for pod in self.pods.values_mut() {
            let child_size = ctx.run_layout(pod, bc);
            ctx.place_child(pod, Point::new(0.0, cursor_y));
            cursor_y += child_size.height;
            max_width = max_width.max(child_size.width);
        }
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(max_width, cursor_y))
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {
        // Children paint themselves through the Masonry render pass; the region
        // adds no chrome of its own.
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
        // Child accessibility nodes are derived by Masonry from the registered
        // child widgets; richer SDUI a11y composition arrives with hosting.
        node.set_children(Vec::new());
    }

    fn children_ids(&self) -> ChildrenIds {
        let ids: Vec<_> = self.pods.values().map(|pod| pod.id()).collect();
        ChildrenIds::from_slice(&ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{SduiFlexDirection, SduiTreeOperation};
    use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
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

    fn panel_node(id: u64, children: Vec<SduiNodeId>) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::Panel {
                title: "Panel".to_string(),
                children,
            },
        )
    }

    fn flex_node(id: u64, children: Vec<SduiNodeId>) -> SduiNode {
        SduiNode::new(
            SduiNodeId(id),
            SduiNodeKind::Flex {
                direction: SduiFlexDirection::Column,
                children,
            },
        )
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
    fn non_reconciled_kinds_produce_no_pods_and_stay_on_legacy_path() {
        let mut region = SduiRegionWidget::new();
        let tree = SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![
                panel_node(1, vec![SduiNodeId(2)]),
                flex_node(2, vec![SduiNodeId(3)]),
            ],
        };

        region.reconcile_snapshot(tree);

        assert_eq!(region.pod_count(), 0, "panel/flex not reconciled yet");
        assert!(region.is_inert());
        assert_eq!(region.children_ids().len(), 0);
        assert_eq!(region.ui_version(), 1);
    }

    #[test]
    fn reconciled_label_produces_exactly_one_child_pod() {
        let mut region = SduiRegionWidget::new();
        let tree = SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![label_node(1, "Open files")],
        };

        region.reconcile_snapshot(tree);

        assert_eq!(region.pod_count(), 1);
        assert!(region.has_pod(SduiNodeId(1)));
        assert_eq!(region.children_ids().len(), 1);
        assert!(!region.is_inert());
    }

    #[test]
    fn replace_node_updates_pod_in_place_and_remove_node_drops_it() {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![label_node(1, "before")],
        });

        // ReplaceNode keeps the stable SDUI node id; a fresh Masonry pod is
        // built for the updated content.
        assert!(region.apply_update(SduiTreeUpdate {
            base_ui_version: 1,
            new_ui_version: 2,
            operations: vec![SduiTreeOperation::ReplaceNode {
                node: label_node(1, "after"),
            }],
        }));
        assert_eq!(region.ui_version(), 2);
        assert_eq!(region.pod_count(), 1);
        assert!(region.has_pod(SduiNodeId(1)));

        assert!(region.apply_update(SduiTreeUpdate {
            base_ui_version: 2,
            new_ui_version: 3,
            operations: vec![SduiTreeOperation::RemoveNode {
                node_id: SduiNodeId(1),
            }],
        }));
        assert_eq!(region.pod_count(), 0);
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
        assert_eq!(region.ui_version(), 5, "version unchanged on stale update");
        assert_eq!(region.pod_count(), 1, "pod untouched on stale update");
    }

    #[test]
    fn replace_root_updates_root_binding() {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![label_node(1, "a"), label_node(2, "b")],
        });

        assert!(region.apply_update(SduiTreeUpdate {
            base_ui_version: 1,
            new_ui_version: 2,
            operations: vec![SduiTreeOperation::ReplaceRoot {
                root_id: SduiNodeId(2),
            }],
        }));
        assert_eq!(region.root_id(), Some(SduiNodeId(2)));
        assert_eq!(region.pod_count(), 2);
    }

    #[test]
    fn region_registers_label_child_and_lays_out_without_panic() {
        let mut region = SduiRegionWidget::new();
        region.reconcile_snapshot(SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![label_node(1, "Open files")],
        });
        let child_id = {
            let ids = region.children_ids();
            ids.iter().next().copied().unwrap()
        };

        let mut render_root =
            RenderRoot::new(NewWidget::new(region), |_| {}, render_root_options());

        // The full Masonry container contract (register_children, layout,
        // place_child, paint, accessibility) runs through the render passes
        // without panicking, and the reconciled child is present in the tree.
        let _ = render_root.redraw();
        assert!(render_root.has_widget(child_id));
    }

    #[test]
    fn empty_region_is_inert_and_claims_no_space() {
        let region = SduiRegionWidget::new();
        assert!(region.is_inert());
        assert_eq!(region.pod_count(), 0);
        assert_eq!(region.children_ids().len(), 0);
        assert_eq!(region.root_id(), None);
    }
}
