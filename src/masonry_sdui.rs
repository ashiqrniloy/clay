use std::collections::{BTreeMap, BTreeSet};

use masonry::accesskit::{Node, NodeId, Role};
use masonry::core::{
    AccessCtx, BoxConstraints, BrushIndex, ChildrenIds, LayoutCtx, NoAction, PaintCtx,
    PropertiesMut, PropertiesRef, RegisterCtx, Widget, WidgetId, render_text,
};
use masonry::kurbo::{Affine, Point, Rect, Size};
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;

use crate::perf::metrics::global_recorder;
use crate::protocol::{
    DocumentId, SduiActionIntent, SduiActionSource, SduiEditorBinding, SduiFlexDirection, SduiNode,
    SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation, SduiTreeUpdate, SduiVersion,
};
use crate::shell::{
    FixedSlotId, FixedSlotState, PackageUiComponentTree, PackageUiOverlayObservation,
    PackageUiPanelObservation, PackageUiRuntimeError, PackageUiRuntimeState,
    PackageUiRuntimeUpdate, PaneSlotLayout, layout::PaneSlotId, theme::SduiThemeStyle,
};

#[cfg(test)]
use crate::shell::{
    FixedPackagePanel, PackageOverlayAnchor, PackagePanelVisibility, TransientPackageOverlay,
};

const SIDEBAR_WIDTH: f64 = 240.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SduiObservableListItem {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SduiAccessibleNode {
    pub role: Role,
    pub label: Option<String>,
}

// Internal test/agent observability surface only. If SDUI state becomes a public
// Clay JS API, expose it through a dedicated facade instead of widening this type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SduiObservableSnapshot {
    pub ui_version: SduiVersion,
    pub node_kinds: Vec<(SduiNodeId, &'static str)>,
    pub panel_titles: Vec<String>,
    pub label_texts: Vec<String>,
    pub button_labels: Vec<String>,
    pub list_items: Vec<SduiObservableListItem>,
    pub editor_bindings: Vec<SduiEditorBinding>,
    pub has_sidebar: bool,
    pub editor_region_non_empty: bool,
    pub package_fixed_panels: Vec<PackageUiPanelObservation>,
    pub package_transient_overlays: Vec<PackageUiOverlayObservation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SduiNativeState {
    ui_version: SduiVersion,
    root_id: Option<SduiNodeId>,
    nodes: BTreeMap<SduiNodeId, SduiNode>,
    editor_binding: Option<SduiEditorBinding>,
    actions: Vec<SduiVisibleAction>,
    package_ui: PackageUiRuntimeState,
}

impl SduiNativeState {
    pub fn empty() -> Self {
        Self {
            ui_version: 0,
            root_id: None,
            nodes: BTreeMap::new(),
            editor_binding: None,
            actions: Vec::new(),
            package_ui: PackageUiRuntimeState::new(),
        }
    }

    pub fn apply_snapshot(&mut self, tree: SduiTree) {
        let recorder = global_recorder();
        let _scope = recorder.scope("sdui.apply_snapshot");
        recorder.record_gauge("sdui.snapshot.node_count", tree.nodes.len() as u64);
        self.ui_version = tree.ui_version;
        self.root_id = Some(tree.root_id);
        self.nodes = tree.nodes.into_iter().map(|node| (node.id, node)).collect();
        self.rebuild_derived_state();
    }

    pub fn apply_update(&mut self, update: SduiTreeUpdate) -> bool {
        let recorder = global_recorder();
        let _scope = recorder.scope("sdui.apply_update");
        recorder.record_gauge(
            "sdui.update.operation_count",
            update.operations.len() as u64,
        );
        if update.base_ui_version != self.ui_version {
            return false;
        }

        for operation in update.operations {
            match operation {
                SduiTreeOperation::ReplaceRoot { root_id } => self.root_id = Some(root_id),
                SduiTreeOperation::ReplaceNode { node } => {
                    self.nodes.insert(node.id, node);
                }
                SduiTreeOperation::RemoveNode { node_id } => {
                    self.nodes.remove(&node_id);
                    if self.root_id == Some(node_id) {
                        self.root_id = None;
                    }
                }
            }
        }
        self.ui_version = update.new_ui_version;
        self.rebuild_derived_state();
        true
    }

    pub fn ui_version(&self) -> SduiVersion {
        self.ui_version
    }

    pub(crate) fn apply_package_ui_update(
        &mut self,
        update: PackageUiRuntimeUpdate,
    ) -> Result<(), PackageUiRuntimeError> {
        self.package_ui.apply_update(update)?;
        self.actions.clear();
        Ok(())
    }

    pub(crate) fn package_ui_version(&self) -> u64 {
        self.package_ui.version()
    }

    pub fn editor_binding(&self) -> Option<&SduiEditorBinding> {
        self.editor_binding.as_ref()
    }

    pub fn contains_node(&self, node_id: SduiNodeId) -> bool {
        self.nodes.contains_key(&node_id)
    }

    pub fn visible_texts(&self) -> Vec<String> {
        let mut texts = Vec::new();
        if let Some(root_id) = self.root_id {
            self.collect_visible_texts(root_id, &mut texts);
        }
        texts
    }

    pub fn action_for_point(&self, point: Point) -> Option<SduiActionIntent> {
        self.actions
            .iter()
            .find(|action| action.rect.contains(point))
            .map(|action| action.intent.clone())
    }

    pub(crate) fn observable_snapshot(&self, widget_size: Size) -> SduiObservableSnapshot {
        let mut snapshot = SduiObservableSnapshot {
            ui_version: self.ui_version,
            node_kinds: self
                .nodes
                .iter()
                .map(|(node_id, node)| (*node_id, sdui_node_kind_name(&node.kind)))
                .collect(),
            panel_titles: Vec::new(),
            label_texts: Vec::new(),
            button_labels: Vec::new(),
            list_items: Vec::new(),
            editor_bindings: Vec::new(),
            has_sidebar: false,
            editor_region_non_empty: false,
            package_fixed_panels: self
                .package_ui
                .fixed_panel_observations(widget_size.to_rect()),
            package_transient_overlays: self.package_overlay_observations(widget_size),
        };

        if let Some(root_id) = self.root_id {
            let mut visited = BTreeSet::new();
            self.collect_observable_snapshot(root_id, &mut visited, &mut snapshot);
        }

        let region = editor_region(widget_size, self);
        snapshot.editor_region_non_empty =
            self.editor_binding.is_some() && region.width() > 0.0 && region.height() > 0.0;
        snapshot
    }

    fn package_overlay_observations(&self, widget_size: Size) -> Vec<PackageUiOverlayObservation> {
        let slot_geometry =
            combined_slot_layout(widget_size, self).compute_geometry(widget_size.to_rect());
        self.package_ui
            .overlays()
            .map(|overlay| PackageUiOverlayObservation {
                id: overlay.id.clone(),
                anchor: overlay.anchor,
                rect: overlay
                    .anchor
                    .rect(widget_size.to_rect(), slot_geometry.main_rect),
                component_id: overlay.component.id.clone(),
                component_kind: overlay.component.kind.clone(),
                focus_policy: overlay.focus_policy.clone(),
                dismissal_policy: overlay.dismissal_policy.clone(),
            })
            .collect()
    }

    pub(crate) fn accessibility_nodes(&self) -> Vec<SduiAccessibleNode> {
        let mut nodes = Vec::new();
        if let Some(root_id) = self.root_id {
            let mut visited = BTreeSet::new();
            self.collect_accessibility_nodes(root_id, &mut visited, &mut nodes);
        }
        nodes
    }

    #[cfg(test)]
    fn rebuild_action_regions_for_test(&mut self, size: Size) {
        self.actions.clear();
        let package_panels: Vec<_> = self
            .package_ui
            .visible_fixed_panels(size.to_rect())
            .into_iter()
            .map(|(rect, panel)| (rect, panel.clone()))
            .collect();
        for (rect, panel) in package_panels {
            let mut cursor_y = rect.y0 + sdui_theme_style().panel_padding;
            self.collect_package_action_regions(
                &panel.component,
                0,
                &mut cursor_y,
                rect.width(),
                rect.x0,
            );
        }
        let Some(root_id) = self.root_id else {
            return;
        };
        let Some(sidebar) = sdui_panel_left_slot_rect(size, self) else {
            return;
        };
        let mut cursor_y = sidebar.y0 + sdui_theme_style().panel_padding;
        self.collect_action_regions(root_id, 0, &mut cursor_y, sidebar.width(), sidebar.x0);
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        self.actions.clear();
        self.paint_package_fixed_panels(ctx, scene);
        if let Some(root_id) = self.root_id {
            if let Some(sidebar) = sdui_panel_left_slot_rect(ctx.size(), self) {
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    sdui_theme_style().panel_background,
                    None,
                    &sidebar,
                );
                let mut cursor_y = sidebar.y0 + sdui_theme_style().panel_padding;
                self.paint_node(
                    ctx,
                    scene,
                    root_id,
                    0,
                    &mut cursor_y,
                    sidebar.width(),
                    sidebar.x0,
                );
            }
        }
        self.paint_package_overlays(ctx, scene);
    }

    fn rebuild_derived_state(&mut self) {
        self.editor_binding = None;
        if let Some(root_id) = self.root_id {
            self.find_editor_binding(root_id);
        }
        self.actions.clear();
    }

    fn find_editor_binding(&mut self, node_id: SduiNodeId) {
        let Some(node) = self.nodes.get(&node_id).cloned() else {
            return;
        };
        match node.kind {
            SduiNodeKind::EditorView { binding } => {
                if self.editor_binding.is_none() {
                    self.editor_binding = Some(binding);
                }
            }
            SduiNodeKind::Panel { children, .. }
            | SduiNodeKind::Stack { children }
            | SduiNodeKind::Flex { children, .. } => {
                for child_id in children {
                    self.find_editor_binding(child_id);
                }
            }
            SduiNodeKind::Label { .. }
            | SduiNodeKind::Button { .. }
            | SduiNodeKind::List { .. } => {}
        }
    }

    fn collect_visible_texts(&self, node_id: SduiNodeId, texts: &mut Vec<String>) {
        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };
        match &node.kind {
            SduiNodeKind::Panel { title, children } => {
                texts.push(title.clone());
                for child_id in children {
                    self.collect_visible_texts(*child_id, texts);
                }
            }
            SduiNodeKind::Label { text } => texts.push(text.clone()),
            SduiNodeKind::Button { label, .. } => texts.push(label.clone()),
            SduiNodeKind::List { items } => {
                for item in items {
                    texts.push(item.label.clone());
                    if let Some(detail) = &item.detail {
                        texts.push(detail.clone());
                    }
                }
            }
            SduiNodeKind::EditorView { binding } => {
                texts.push(format!("Editor document {}", binding.document_id));
            }
            SduiNodeKind::Flex { children, .. } | SduiNodeKind::Stack { children } => {
                for child_id in children {
                    self.collect_visible_texts(*child_id, texts);
                }
            }
        }
    }

    fn collect_observable_snapshot(
        &self,
        node_id: SduiNodeId,
        visited: &mut BTreeSet<SduiNodeId>,
        snapshot: &mut SduiObservableSnapshot,
    ) {
        if !visited.insert(node_id) {
            return;
        }
        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };
        match &node.kind {
            SduiNodeKind::Panel { title, children } => {
                snapshot.has_sidebar = true;
                snapshot.panel_titles.push(title.clone());
                for child_id in children {
                    self.collect_observable_snapshot(*child_id, visited, snapshot);
                }
            }
            SduiNodeKind::Label { text } => {
                snapshot.has_sidebar = true;
                snapshot.label_texts.push(text.clone());
            }
            SduiNodeKind::Button { label, .. } => {
                snapshot.has_sidebar = true;
                snapshot.button_labels.push(label.clone());
            }
            SduiNodeKind::List { items } => {
                snapshot.has_sidebar = true;
                snapshot
                    .list_items
                    .extend(items.iter().map(|item| SduiObservableListItem {
                        id: item.id.clone(),
                        label: item.label.clone(),
                    }));
            }
            SduiNodeKind::EditorView { binding } => {
                snapshot.editor_bindings.push(binding.clone());
            }
            SduiNodeKind::Flex { children, .. } | SduiNodeKind::Stack { children } => {
                for child_id in children {
                    self.collect_observable_snapshot(*child_id, visited, snapshot);
                }
            }
        }
    }

    fn collect_accessibility_nodes(
        &self,
        node_id: SduiNodeId,
        visited: &mut BTreeSet<SduiNodeId>,
        nodes: &mut Vec<SduiAccessibleNode>,
    ) {
        if !visited.insert(node_id) {
            return;
        }
        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };
        match &node.kind {
            SduiNodeKind::Panel { title, children } => {
                nodes.push(SduiAccessibleNode {
                    role: Role::Pane,
                    label: Some(title.clone()),
                });
                for child_id in children {
                    self.collect_accessibility_nodes(*child_id, visited, nodes);
                }
            }
            SduiNodeKind::Label { text } => nodes.push(SduiAccessibleNode {
                role: Role::Label,
                label: Some(text.clone()),
            }),
            SduiNodeKind::Button { label, .. } => nodes.push(SduiAccessibleNode {
                role: Role::Button,
                label: Some(label.clone()),
            }),
            SduiNodeKind::List { items } => {
                nodes.push(SduiAccessibleNode {
                    role: Role::List,
                    label: None,
                });
                for item in items {
                    nodes.push(SduiAccessibleNode {
                        role: Role::ListItem,
                        label: Some(item.label.clone()),
                    });
                }
            }
            SduiNodeKind::EditorView { binding } => nodes.push(SduiAccessibleNode {
                role: Role::MultilineTextInput,
                label: Some(format!("Editor document {}", binding.document_id)),
            }),
            SduiNodeKind::Flex { children, .. } | SduiNodeKind::Stack { children } => {
                nodes.push(SduiAccessibleNode {
                    role: Role::Pane,
                    label: None,
                });
                for child_id in children {
                    self.collect_accessibility_nodes(*child_id, visited, nodes);
                }
            }
        }
    }

    #[cfg(test)]
    fn collect_action_regions(
        &mut self,
        node_id: SduiNodeId,
        depth: usize,
        cursor_y: &mut f64,
        width: f64,
        origin_x: f64,
    ) {
        let Some(node) = self.nodes.get(&node_id).cloned() else {
            return;
        };
        match node.kind {
            SduiNodeKind::Panel { children, .. } => {
                *cursor_y += sdui_theme_style().row_height;
                for child_id in children {
                    self.collect_action_regions(child_id, depth + 1, cursor_y, width, origin_x);
                }
            }
            SduiNodeKind::Label { .. } | SduiNodeKind::EditorView { .. } => {
                *cursor_y += sdui_theme_style().row_height;
            }
            SduiNodeKind::Button { action, .. } => {
                self.actions.push(SduiVisibleAction {
                    rect: row_rect(depth, *cursor_y, width, origin_x),
                    intent: action,
                });
                *cursor_y += sdui_theme_style().row_height + 6.0;
            }
            SduiNodeKind::List { items } => {
                for item in items {
                    if let Some(action) = item.action {
                        self.actions.push(SduiVisibleAction {
                            rect: row_rect(depth, *cursor_y, width, origin_x),
                            intent: action,
                        });
                    }
                    *cursor_y += sdui_theme_style().row_height + 10.0;
                }
            }
            SduiNodeKind::Flex {
                direction,
                children,
            } => match direction {
                SduiFlexDirection::Row => {
                    for child_id in children {
                        if !matches!(
                            self.nodes.get(&child_id).map(|node| &node.kind),
                            Some(SduiNodeKind::EditorView { .. })
                        ) {
                            self.collect_action_regions(child_id, depth, cursor_y, width, origin_x);
                        }
                    }
                }
                SduiFlexDirection::Column => {
                    for child_id in children {
                        self.collect_action_regions(child_id, depth, cursor_y, width, origin_x);
                    }
                }
            },
            SduiNodeKind::Stack { children } => {
                for child_id in children {
                    self.collect_action_regions(child_id, depth, cursor_y, width, origin_x);
                }
            }
        }
    }

    #[cfg(test)]
    fn collect_package_action_regions(
        &mut self,
        component: &PackageUiComponentTree,
        depth: usize,
        cursor_y: &mut f64,
        width: f64,
        origin_x: f64,
    ) {
        match component.kind.as_str() {
            "panel" => {
                *cursor_y += sdui_theme_style().row_height;
                for child in &component.children {
                    self.collect_package_action_regions(
                        child,
                        depth + 1,
                        cursor_y,
                        width,
                        origin_x,
                    );
                }
            }
            "button" => {
                if let Some(command_id) = &component.action_command_id {
                    self.actions.push(SduiVisibleAction {
                        rect: row_rect(depth, *cursor_y, width, origin_x),
                        intent: package_action_intent(command_id, &component.id),
                    });
                }
                *cursor_y += sdui_theme_style().row_height + 6.0;
            }
            "list" => {
                for item in &component.items {
                    if let Some(command_id) = &item.action_command_id {
                        self.actions.push(SduiVisibleAction {
                            rect: row_rect(depth, *cursor_y, width, origin_x),
                            intent: package_action_intent(
                                command_id,
                                &format!("{}.{}", component.id, item.id),
                            ),
                        });
                    }
                    *cursor_y += sdui_theme_style().row_height + 10.0;
                }
            }
            "label" | "statusItem" | "editorView" => {
                *cursor_y += sdui_theme_style().row_height;
            }
            "flex" | "stack" | "overlay" | "scroll" | "portal" => {
                for child in &component.children {
                    self.collect_package_action_regions(child, depth, cursor_y, width, origin_x);
                }
            }
            _ => {}
        }
    }

    fn build_accessibility_subtree(
        &self,
        node_id: SduiNodeId,
        visited: &mut BTreeSet<SduiNodeId>,
        tree_update: &mut masonry::accesskit::TreeUpdate,
    ) -> Option<NodeId> {
        if !visited.insert(node_id) {
            return None;
        }
        let node = self.nodes.get(&node_id)?;
        let id = NodeId::from(WidgetId::next());
        let mut access_node = Node::new(sdui_accessibility_role(&node.kind));
        if let Some(label) = sdui_accessibility_label(&node.kind) {
            access_node.set_label(label);
        }

        let mut child_ids = Vec::new();
        match &node.kind {
            SduiNodeKind::Panel { children, .. }
            | SduiNodeKind::Flex { children, .. }
            | SduiNodeKind::Stack { children } => {
                for child_id in children {
                    if let Some(access_child_id) =
                        self.build_accessibility_subtree(*child_id, visited, tree_update)
                    {
                        child_ids.push(access_child_id);
                    }
                }
            }
            SduiNodeKind::List { items } => {
                for item in items {
                    let item_id = NodeId::from(WidgetId::next());
                    let mut item_node = Node::new(Role::ListItem);
                    item_node.set_label(item.label.clone());
                    tree_update.nodes.push((item_id, item_node));
                    child_ids.push(item_id);
                }
            }
            SduiNodeKind::Label { .. }
            | SduiNodeKind::Button { .. }
            | SduiNodeKind::EditorView { .. } => {}
        }
        access_node.set_children(child_ids);
        tree_update.nodes.push((id, access_node));
        Some(id)
    }

    fn paint_node(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut Scene,
        node_id: SduiNodeId,
        depth: usize,
        cursor_y: &mut f64,
        width: f64,
        origin_x: f64,
    ) {
        let Some(node) = self.nodes.get(&node_id).cloned() else {
            return;
        };
        match node.kind {
            SduiNodeKind::Panel { title, children } => {
                self.paint_text(
                    ctx,
                    scene,
                    &title,
                    depth,
                    *cursor_y,
                    sdui_theme_style().title_text_size,
                    sdui_theme_style().text_color,
                    origin_x,
                );
                *cursor_y += sdui_theme_style().row_height;
                for child_id in children {
                    self.paint_node(ctx, scene, child_id, depth + 1, cursor_y, width, origin_x);
                }
            }
            SduiNodeKind::Label { text } => {
                self.paint_text(
                    ctx,
                    scene,
                    &text,
                    depth,
                    *cursor_y,
                    sdui_theme_style().body_text_size,
                    sdui_theme_style().muted_text_color,
                    origin_x,
                );
                *cursor_y += sdui_theme_style().row_height;
            }
            SduiNodeKind::Button { label, action } => {
                let rect = row_rect(depth, *cursor_y, width, origin_x);
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    sdui_theme_style().button_background,
                    None,
                    &rect,
                );
                self.actions.push(SduiVisibleAction {
                    rect,
                    intent: action,
                });
                self.paint_text(
                    ctx,
                    scene,
                    &label,
                    depth,
                    *cursor_y + 4.0,
                    sdui_theme_style().body_text_size,
                    sdui_theme_style().text_color,
                    origin_x,
                );
                *cursor_y += sdui_theme_style().row_height + 6.0;
            }
            SduiNodeKind::List { items } => {
                for item in items {
                    let rect = row_rect(depth, *cursor_y, width, origin_x);
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        sdui_theme_style().list_background,
                        None,
                        &rect,
                    );
                    if let Some(action) = item.action {
                        self.actions.push(SduiVisibleAction {
                            rect,
                            intent: action,
                        });
                    }
                    self.paint_text(
                        ctx,
                        scene,
                        &item.label,
                        depth,
                        *cursor_y + 2.0,
                        sdui_theme_style().body_text_size,
                        sdui_theme_style().text_color,
                        origin_x,
                    );
                    if let Some(detail) = item.detail {
                        self.paint_text(
                            ctx,
                            scene,
                            &detail,
                            depth,
                            *cursor_y + 15.0,
                            10.0,
                            sdui_theme_style().muted_text_color,
                            origin_x,
                        );
                    }
                    *cursor_y += sdui_theme_style().row_height + 10.0;
                }
            }
            SduiNodeKind::EditorView { binding } => {
                self.paint_text(
                    ctx,
                    scene,
                    &format!("Editor view · doc {}", binding.document_id),
                    depth,
                    *cursor_y,
                    sdui_theme_style().body_text_size,
                    sdui_theme_style().muted_text_color,
                    origin_x,
                );
                *cursor_y += sdui_theme_style().row_height;
            }
            SduiNodeKind::Flex {
                direction,
                children,
            } => match direction {
                SduiFlexDirection::Row => {
                    for child_id in children {
                        if !matches!(
                            self.nodes.get(&child_id).map(|node| &node.kind),
                            Some(SduiNodeKind::EditorView { .. })
                        ) {
                            self.paint_node(ctx, scene, child_id, depth, cursor_y, width, origin_x);
                        }
                    }
                }
                SduiFlexDirection::Column => {
                    for child_id in children {
                        self.paint_node(ctx, scene, child_id, depth, cursor_y, width, origin_x);
                    }
                }
            },
            SduiNodeKind::Stack { children } => {
                for child_id in children {
                    self.paint_node(ctx, scene, child_id, depth, cursor_y, width, origin_x);
                }
            }
        }
    }

    fn paint_package_fixed_panels(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        let size = ctx.size();
        let fixed_panels: Vec<_> = self
            .package_ui
            .visible_fixed_panels(size.to_rect())
            .into_iter()
            .map(|(rect, panel)| (rect, panel.clone()))
            .collect();
        for (rect, panel) in fixed_panels {
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                sdui_theme_style().panel_background,
                None,
                &rect,
            );
            let mut cursor_y = rect.y0 + sdui_theme_style().panel_padding;
            self.paint_package_component(
                ctx,
                scene,
                &panel.component,
                0,
                &mut cursor_y,
                rect.width(),
                rect.x0,
            );
        }
    }

    fn paint_package_overlays(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        let size = ctx.size();
        let slot_geometry = combined_slot_layout(size, self).compute_geometry(size.to_rect());
        let overlays: Vec<_> = self.package_ui.overlays().cloned().collect();
        for overlay in overlays {
            let rect = overlay.anchor.rect(size.to_rect(), slot_geometry.main_rect);
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                sdui_theme_style().panel_background,
                None,
                &rect,
            );
            let mut cursor_y = rect.y0 + sdui_theme_style().panel_padding;
            self.paint_package_component(
                ctx,
                scene,
                &overlay.component,
                0,
                &mut cursor_y,
                rect.width(),
                rect.x0,
            );
        }
    }

    fn paint_package_component(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut Scene,
        component: &PackageUiComponentTree,
        depth: usize,
        cursor_y: &mut f64,
        width: f64,
        origin_x: f64,
    ) {
        match component.kind.as_str() {
            "panel" => {
                if let Some(title) = &component.title {
                    self.paint_text(
                        ctx,
                        scene,
                        title,
                        depth,
                        *cursor_y,
                        sdui_theme_style().title_text_size,
                        sdui_theme_style().text_color,
                        origin_x,
                    );
                    *cursor_y += sdui_theme_style().row_height;
                }
                for child in &component.children {
                    self.paint_package_component(
                        ctx,
                        scene,
                        child,
                        depth + 1,
                        cursor_y,
                        width,
                        origin_x,
                    );
                }
            }
            "label" | "statusItem" => {
                let text = component
                    .text
                    .as_deref()
                    .or(component.label.as_deref())
                    .unwrap_or(&component.id);
                self.paint_text(
                    ctx,
                    scene,
                    text,
                    depth,
                    *cursor_y,
                    sdui_theme_style().body_text_size,
                    sdui_theme_style().muted_text_color,
                    origin_x,
                );
                *cursor_y += sdui_theme_style().row_height;
            }
            "button" => {
                let rect = row_rect(depth, *cursor_y, width, origin_x);
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    sdui_theme_style().button_background,
                    None,
                    &rect,
                );
                if let Some(command_id) = &component.action_command_id {
                    self.actions.push(SduiVisibleAction {
                        rect,
                        intent: package_action_intent(command_id, &component.id),
                    });
                }
                let label = component.label.as_deref().unwrap_or(&component.id);
                self.paint_text(
                    ctx,
                    scene,
                    label,
                    depth,
                    *cursor_y + 4.0,
                    sdui_theme_style().body_text_size,
                    sdui_theme_style().text_color,
                    origin_x,
                );
                *cursor_y += sdui_theme_style().row_height + 6.0;
            }
            "list" => {
                for item in &component.items {
                    let rect = row_rect(depth, *cursor_y, width, origin_x);
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        sdui_theme_style().list_background,
                        None,
                        &rect,
                    );
                    if let Some(command_id) = &item.action_command_id {
                        self.actions.push(SduiVisibleAction {
                            rect,
                            intent: package_action_intent(
                                command_id,
                                &format!("{}.{}", component.id, item.id),
                            ),
                        });
                    }
                    self.paint_text(
                        ctx,
                        scene,
                        &item.label,
                        depth,
                        *cursor_y + 2.0,
                        sdui_theme_style().body_text_size,
                        sdui_theme_style().text_color,
                        origin_x,
                    );
                    if let Some(detail) = &item.detail {
                        self.paint_text(
                            ctx,
                            scene,
                            detail,
                            depth,
                            *cursor_y + 15.0,
                            10.0,
                            sdui_theme_style().muted_text_color,
                            origin_x,
                        );
                    }
                    *cursor_y += sdui_theme_style().row_height + 10.0;
                }
            }
            "editorView" => {
                self.paint_text(
                    ctx,
                    scene,
                    &format!("Editor view · {}", component.id),
                    depth,
                    *cursor_y,
                    sdui_theme_style().body_text_size,
                    sdui_theme_style().muted_text_color,
                    origin_x,
                );
                *cursor_y += sdui_theme_style().row_height;
            }
            "flex" | "stack" | "overlay" | "scroll" | "portal" => {
                for child in &component.children {
                    self.paint_package_component(
                        ctx, scene, child, depth, cursor_y, width, origin_x,
                    );
                }
            }
            _ => {}
        }
    }

    fn paint_text(
        &self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut Scene,
        text: &str,
        depth: usize,
        y: f64,
        size: f32,
        color: Color,
        origin_x: f64,
    ) {
        let max_width =
            (SIDEBAR_WIDTH - sdui_theme_style().panel_padding * 2.0 - depth as f64 * 10.0).max(1.0)
                as f32;
        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.2)));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(max_width));
        render_text(
            scene,
            Affine::translate((
                origin_x + sdui_theme_style().panel_padding + depth as f64 * 10.0,
                y,
            )),
            &layout,
            &[color.into()],
            true,
        );
    }
}

impl Default for SduiNativeState {
    fn default() -> Self {
        Self::empty()
    }
}

impl Widget for SduiNativeState {
    type Action = NoAction;

    fn register_children(&mut self, _ctx: &mut RegisterCtx<'_>) {}

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(SIDEBAR_WIDTH, 600.0))
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        SduiNativeState::paint(self, ctx, scene);
    }

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label("Server-driven UI");
        let mut visited = BTreeSet::new();
        let mut children = Vec::new();
        if let Some(root_id) = self.root_id {
            if let Some(root_access_id) =
                self.build_accessibility_subtree(root_id, &mut visited, ctx.tree_update())
            {
                children.push(root_access_id);
            }
        }
        node.set_children(children);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SduiVisibleAction {
    rect: Rect,
    intent: SduiActionIntent,
}

pub fn editor_region(size: Size, sdui: &SduiNativeState) -> Rect {
    sdui_slot_layout(size, sdui)
        .compute_geometry(size.to_rect())
        .main_rect
}

pub fn editor_region_for_document(
    size: Size,
    sdui: &SduiNativeState,
    document_id: DocumentId,
) -> Rect {
    let full_rect = size.to_rect();
    let package_main_rect = sdui
        .package_ui
        .slot_layout()
        .compute_geometry(full_rect)
        .main_rect;
    match sdui.editor_binding() {
        Some(binding) if binding.document_id == document_id => editor_region(size, sdui),
        _ if package_main_rect != full_rect => package_main_rect,
        _ => full_rect,
    }
}

fn sdui_slot_layout(size: Size, sdui: &SduiNativeState) -> PaneSlotLayout {
    let mut layout = sdui.package_ui.slot_layout();
    if sdui.editor_binding().is_some()
        && size.width > SIDEBAR_WIDTH + 100.0
        && !layout.contains_slot(PaneSlotId::Left)
    {
        layout = layout.with_fixed_slot(fixed_sdui_left_slot());
    }
    layout
}

fn sdui_panel_slot_layout(sdui: &SduiNativeState) -> PaneSlotLayout {
    let mut layout = sdui.package_ui.slot_layout();
    if sdui.root_id.is_some() && !layout.contains_slot(PaneSlotId::Left) {
        layout = layout.with_fixed_slot(fixed_sdui_left_slot());
    }
    layout
}

fn combined_slot_layout(size: Size, sdui: &SduiNativeState) -> PaneSlotLayout {
    sdui_slot_layout(size, sdui)
}

fn fixed_sdui_left_slot() -> FixedSlotState {
    FixedSlotState {
        slot_id: FixedSlotId::Left,
        size: SIDEBAR_WIDTH,
        min_size: SIDEBAR_WIDTH,
        max_size: SIDEBAR_WIDTH,
        visible: true,
        collapsed: false,
        resized_by_user: false,
    }
}

fn sdui_panel_left_slot_rect(size: Size, sdui: &SduiNativeState) -> Option<Rect> {
    sdui_panel_slot_layout(sdui)
        .compute_geometry(size.to_rect())
        .fixed_slots
        .into_iter()
        .find(|slot| slot.slot_id == FixedSlotId::Left)
        .map(|slot| slot.rect)
}

fn sdui_theme_style() -> SduiThemeStyle {
    SduiThemeStyle::default()
}

fn row_rect(depth: usize, y: f64, width: f64, origin_x: f64) -> Rect {
    let x0 = origin_x + sdui_theme_style().panel_padding + depth as f64 * 10.0;
    Rect::new(
        x0,
        y,
        (origin_x + width - sdui_theme_style().panel_padding).max(x0),
        y + sdui_theme_style().row_height,
    )
}

fn package_action_intent(command_id: &str, source_id: &str) -> SduiActionIntent {
    SduiActionIntent::command(
        command_id.to_string(),
        SduiActionSource::Button {
            node_id: SduiNodeId(stable_package_source_id(source_id)),
        },
    )
}

fn stable_package_source_id(source_id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in source_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash.max(1)
}

fn sdui_accessibility_role(kind: &SduiNodeKind) -> Role {
    match kind {
        SduiNodeKind::Panel { .. } | SduiNodeKind::Flex { .. } | SduiNodeKind::Stack { .. } => {
            Role::Pane
        }
        SduiNodeKind::Label { .. } => Role::Label,
        SduiNodeKind::Button { .. } => Role::Button,
        SduiNodeKind::List { .. } => Role::List,
        SduiNodeKind::EditorView { .. } => Role::MultilineTextInput,
    }
}

fn sdui_accessibility_label(kind: &SduiNodeKind) -> Option<String> {
    match kind {
        SduiNodeKind::Panel { title, .. } => Some(title.clone()),
        SduiNodeKind::Label { text } => Some(text.clone()),
        SduiNodeKind::Button { label, .. } => Some(label.clone()),
        SduiNodeKind::EditorView { binding } => {
            Some(format!("Editor document {}", binding.document_id))
        }
        SduiNodeKind::List { .. } | SduiNodeKind::Flex { .. } | SduiNodeKind::Stack { .. } => None,
    }
}

fn sdui_node_kind_name(kind: &SduiNodeKind) -> &'static str {
    match kind {
        SduiNodeKind::Panel { .. } => "Panel",
        SduiNodeKind::Label { .. } => "Label",
        SduiNodeKind::Button { .. } => "Button",
        SduiNodeKind::List { .. } => "List",
        SduiNodeKind::EditorView { .. } => "EditorView",
        SduiNodeKind::Flex { .. } => "Flex",
        SduiNodeKind::Stack { .. } => "Stack",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        SduiActionSource, SduiEditorBinding, SduiFlexDirection, SduiListItem, SduiNodeKind,
        representative_panel_update, representative_sdui_tree,
    };
    use serde_json::json;

    fn package_component(id: &str) -> PackageUiComponentTree {
        PackageUiComponentTree::from_declaration(&json!({
            "kind": "panel",
            "id": id,
            "title": "Preview",
            "children": [{
                "kind": "button",
                "id": format!("{id}.toggle"),
                "label": "Toggle Preview",
                "action": { "commandId": "markdown.togglePreview" }
            }]
        }))
        .unwrap()
    }

    fn sample_tree() -> SduiTree {
        let root = SduiNodeId(1);
        let panel = SduiNodeId(2);
        let label = SduiNodeId(3);
        let button = SduiNodeId(4);
        let list = SduiNodeId(5);
        let editor = SduiNodeId(6);
        SduiTree {
            ui_version: 1,
            root_id: root,
            nodes: vec![
                SduiNode::new(
                    root,
                    SduiNodeKind::Flex {
                        direction: SduiFlexDirection::Row,
                        children: vec![panel, editor],
                    },
                ),
                SduiNode::new(
                    panel,
                    SduiNodeKind::Panel {
                        title: "Workspace".to_string(),
                        children: vec![label, button, list],
                    },
                ),
                SduiNode::new(
                    label,
                    SduiNodeKind::Label {
                        text: "Ready".to_string(),
                    },
                ),
                SduiNode::new(
                    button,
                    SduiNodeKind::Button {
                        label: "Refresh".to_string(),
                        action: SduiActionIntent::command(
                            "workspace.refresh",
                            SduiActionSource::Button { node_id: button },
                        ),
                    },
                ),
                SduiNode::new(
                    list,
                    SduiNodeKind::List {
                        items: vec![SduiListItem {
                            id: "active".to_string(),
                            label: "Document 7".to_string(),
                            detail: Some("Server-generated editor view".to_string()),
                            action: None,
                        }],
                    },
                ),
                SduiNode::new(
                    editor,
                    SduiNodeKind::EditorView {
                        binding: SduiEditorBinding {
                            document_id: 7,
                            expected_version: Some(1),
                        },
                    },
                ),
            ],
        }
    }

    #[test]
    fn sdui_snapshot_replaces_native_tree_state() {
        let mut state = SduiNativeState::empty();

        state.apply_snapshot(sample_tree());

        assert_eq!(state.ui_version(), 1);
        assert_eq!(state.editor_binding().unwrap().document_id, 7);
        assert!(state.visible_texts().contains(&"Workspace".to_string()));
        assert!(state.visible_texts().contains(&"Refresh".to_string()));
    }

    #[test]
    fn sdui_update_preserves_editor_document_state() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());
        let before = state.editor_binding().cloned();

        assert!(state.apply_update(SduiTreeUpdate {
            base_ui_version: 1,
            new_ui_version: 2,
            operations: vec![SduiTreeOperation::ReplaceNode {
                node: SduiNode::new(
                    SduiNodeId(3),
                    SduiNodeKind::Label {
                        text: "Updated".to_string(),
                    },
                ),
            }],
        }));

        assert_eq!(state.ui_version(), 2);
        assert_eq!(state.editor_binding().cloned(), before);
        assert!(state.visible_texts().contains(&"Updated".to_string()));
    }

    #[test]
    fn editor_region_is_bounded_when_document_bound_editor_view_is_present() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        let region = editor_region_for_document(Size::new(900.0, 600.0), &state, 7);

        assert_eq!(region.x0, SIDEBAR_WIDTH);
        assert_eq!(region.x1, 900.0);
    }

    #[test]
    fn slot_panel_contribution_places_panel_in_requested_slot_and_preserves_main_editor() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());
        state
            .apply_package_ui_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Right,
                    PackagePanelVisibility::Visible,
                    package_component("markdown.preview.root"),
                    vec!["markdown.togglePreview".to_string()],
                )],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .unwrap();

        let region = editor_region_for_document(Size::new(900.0, 600.0), &state, 7);
        let snapshot = state.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(region, Rect::new(SIDEBAR_WIDTH, 0.0, 660.0, 600.0));
        assert_eq!(snapshot.package_fixed_panels.len(), 1);
        assert_eq!(snapshot.package_fixed_panels[0].slot_id, FixedSlotId::Right);
        assert_eq!(
            snapshot.package_fixed_panels[0].rect,
            Rect::new(660.0, 0.0, 900.0, 600.0)
        );
        assert_eq!(state.package_ui_version(), 1);
    }

    #[test]
    fn transient_overlay_renders_without_consuming_fixed_slot_geometry() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());
        state
            .apply_package_ui_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Bottom,
                    PackagePanelVisibility::Visible,
                    package_component("markdown.preview.root"),
                    Vec::new(),
                )],
                transient_overlays: vec![TransientPackageOverlay::new(
                    "markdown.preview.quickOpen",
                    PackageOverlayAnchor::Main,
                    "restore",
                    "escape",
                    package_component("markdown.preview.quickOpen.root"),
                    Vec::new(),
                )],
                input_routing: Vec::new(),
            })
            .unwrap();

        let region = editor_region_for_document(Size::new(900.0, 600.0), &state, 7);
        let snapshot = state.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(region, Rect::new(SIDEBAR_WIDTH, 0.0, 900.0, 480.0));
        assert_eq!(snapshot.package_transient_overlays.len(), 1);
        assert_eq!(snapshot.package_transient_overlays[0].rect, region);
    }

    #[test]
    fn slot_ui_actions_emit_registered_command_intents_only() {
        let mut state = SduiNativeState::empty();
        state
            .apply_package_ui_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Left,
                    PackagePanelVisibility::Visible,
                    package_component("markdown.preview.root"),
                    vec!["markdown.togglePreview".to_string()],
                )],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .unwrap();
        state.rebuild_action_regions_for_test(Size::new(900.0, 600.0));

        let action = state
            .action_for_point(Point::new(40.0, 45.0))
            .expect("package button should install an inert command hit region");

        assert_eq!(action.command_id, "markdown.togglePreview");
        assert!(matches!(action.source, SduiActionSource::Button { .. }));
    }

    #[test]
    fn slot_ui_observation_omits_document_text_native_handles_and_raw_authority() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());
        state
            .apply_package_ui_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Top,
                    PackagePanelVisibility::Visible,
                    package_component("markdown.preview.root"),
                    vec!["markdown.togglePreview".to_string()],
                )],
                transient_overlays: vec![TransientPackageOverlay::new(
                    "markdown.preview.quickOpen",
                    PackageOverlayAnchor::Pointer,
                    "restore",
                    "escape-or-outside",
                    package_component("markdown.preview.quickOpen.root"),
                    Vec::new(),
                )],
                input_routing: Vec::new(),
            })
            .unwrap();

        let debug = format!("{:?}", state.observable_snapshot(Size::new(900.0, 600.0)));
        for forbidden in [
            "WidgetId",
            "nativeHandle",
            "masonryWidget",
            "Deno.core.ops",
            "op_clay_",
            "rendererCallback",
            "clientJavaScript",
            "secret",
        ] {
            assert!(!debug.contains(forbidden), "observation leaked {forbidden}");
        }
    }

    #[test]
    fn sdui_actions_still_emit_server_intents_from_slot_geometry() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        state.rebuild_action_regions_for_test(Size::new(900.0, 600.0));
        let intent = state
            .action_for_point(Point::new(30.0, 70.0))
            .expect("button action should be installed in left slot geometry");

        assert_eq!(intent.command_id, "workspace.refresh");
    }

    #[test]
    fn sdui_renderer_uses_resolved_theme_tokens_for_panel_styles() {
        let style = sdui_theme_style();

        assert_eq!(style.panel_padding, 14.0);
        assert_eq!(style.row_height, 26.0);
        assert_eq!(style.title_text_size, 14.0);
        assert_eq!(style.body_text_size, 12.0);
        assert_eq!(style.panel_background, Color::from_rgb8(0x21, 0x20, 0x2b));
        assert_eq!(style.button_background, Color::from_rgb8(0x39, 0x35, 0x4a));
        assert_eq!(style.list_background, Color::from_rgb8(0x29, 0x28, 0x35));
    }

    #[test]
    fn unknown_editor_view_document_uses_safe_full_editor_region() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        let region = editor_region_for_document(Size::new(900.0, 600.0), &state, 999);

        assert_eq!(region, Size::new(900.0, 600.0).to_rect());
    }

    #[test]
    fn stale_sdui_update_is_ignored() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        assert!(!state.apply_update(SduiTreeUpdate {
            base_ui_version: 99,
            new_ui_version: 100,
            operations: vec![SduiTreeOperation::RemoveNode {
                node_id: SduiNodeId(3),
            }],
        }));
        assert!(state.contains_node(SduiNodeId(3)));
    }

    #[test]
    fn sdui_observable_snapshot_empty_state_is_well_formed() {
        let state = SduiNativeState::empty();

        let snapshot = state.observable_snapshot(Size::new(800.0, 600.0));

        assert_eq!(snapshot.ui_version, 0);
        assert!(snapshot.node_kinds.is_empty());
        assert!(snapshot.panel_titles.is_empty());
        assert!(snapshot.label_texts.is_empty());
        assert!(snapshot.button_labels.is_empty());
        assert!(snapshot.list_items.is_empty());
        assert!(snapshot.editor_bindings.is_empty());
        assert!(!snapshot.has_sidebar);
        assert!(!snapshot.editor_region_non_empty);
    }

    #[test]
    fn sdui_observable_snapshot_captures_representative_tree() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());

        let snapshot = state.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(snapshot.ui_version, 1);
        assert_eq!(snapshot.panel_titles, vec!["Workspace".to_string()]);
        assert_eq!(
            snapshot.label_texts,
            vec!["Document 7 · version 3".to_string()]
        );
        assert_eq!(snapshot.button_labels, vec!["Refresh".to_string()]);
        assert_eq!(
            snapshot.list_items,
            vec![SduiObservableListItem {
                id: "active-document".to_string(),
                label: "Document 7".to_string(),
            }]
        );
        assert_eq!(
            snapshot.editor_bindings,
            vec![SduiEditorBinding {
                document_id: 7,
                expected_version: Some(3),
            }]
        );
        assert!(snapshot.has_sidebar);
        assert!(snapshot.editor_region_non_empty);
        assert!(snapshot.node_kinds.contains(&(SduiNodeId(1), "Flex")));
        assert!(snapshot.node_kinds.contains(&(SduiNodeId(2), "Panel")));
        assert!(snapshot.node_kinds.contains(&(SduiNodeId(3), "Stack")));
        assert!(snapshot.node_kinds.contains(&(SduiNodeId(4), "Label")));
        assert!(snapshot.node_kinds.contains(&(SduiNodeId(5), "Button")));
        assert!(snapshot.node_kinds.contains(&(SduiNodeId(6), "List")));
        assert!(snapshot.node_kinds.contains(&(SduiNodeId(7), "EditorView")));
    }

    #[test]
    fn sdui_observable_snapshot_changes_after_update() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());
        let before = state.observable_snapshot(Size::new(900.0, 600.0));

        assert!(state.apply_update(representative_panel_update()));
        let after = state.observable_snapshot(Size::new(900.0, 600.0));

        assert_ne!(after, before);
        assert_eq!(
            after.label_texts,
            vec!["Document 7 · version 4".to_string()]
        );
        assert_eq!(after.panel_titles, before.panel_titles);
        assert_eq!(after.button_labels, before.button_labels);
        assert_eq!(after.list_items, before.list_items);
        assert_eq!(after.editor_bindings, before.editor_bindings);
        assert_eq!(after.node_kinds, before.node_kinds);
    }

    #[test]
    fn sdui_observable_snapshot_node_kinds_sorted_by_id() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(10),
            nodes: vec![
                SduiNode::new(
                    SduiNodeId(10),
                    SduiNodeKind::Stack {
                        children: vec![SduiNodeId(2), SduiNodeId(7)],
                    },
                ),
                SduiNode::new(
                    SduiNodeId(7),
                    SduiNodeKind::Label {
                        text: "Second".to_string(),
                    },
                ),
                SduiNode::new(
                    SduiNodeId(2),
                    SduiNodeKind::Label {
                        text: "First".to_string(),
                    },
                ),
            ],
        });

        let snapshot = state.observable_snapshot(Size::new(800.0, 600.0));
        let ids: Vec<SduiNodeId> = snapshot
            .node_kinds
            .iter()
            .map(|(node_id, _)| *node_id)
            .collect();

        assert_eq!(ids, vec![SduiNodeId(2), SduiNodeId(7), SduiNodeId(10)]);
    }

    #[test]
    fn sdui_layout_regression_representative_tree() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());

        let snapshot = state.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(snapshot.ui_version, 1);
        assert_eq!(
            snapshot.node_kinds,
            vec![
                (SduiNodeId(1), "Flex"),
                (SduiNodeId(2), "Panel"),
                (SduiNodeId(3), "Stack"),
                (SduiNodeId(4), "Label"),
                (SduiNodeId(5), "Button"),
                (SduiNodeId(6), "List"),
                (SduiNodeId(7), "EditorView"),
            ]
        );
        assert_eq!(snapshot.panel_titles, vec!["Workspace".to_string()]);
        assert_eq!(
            snapshot.label_texts,
            vec!["Document 7 · version 3".to_string()]
        );
        assert_eq!(snapshot.button_labels, vec!["Refresh".to_string()]);
        assert_eq!(
            snapshot.list_items,
            vec![SduiObservableListItem {
                id: "active-document".to_string(),
                label: "Document 7".to_string(),
            }]
        );
        assert_eq!(
            snapshot.editor_bindings,
            vec![SduiEditorBinding {
                document_id: 7,
                expected_version: Some(3),
            }]
        );
        assert!(snapshot.has_sidebar);
        assert!(snapshot.editor_region_non_empty);
    }

    #[test]
    fn sdui_layout_regression_panel_update_changes_label_only() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());
        let before = state.observable_snapshot(Size::new(900.0, 600.0));

        assert!(state.apply_update(representative_panel_update()));
        let after = state.observable_snapshot(Size::new(900.0, 600.0));

        let mut expected = before;
        expected.ui_version = 2;
        expected.label_texts = vec!["Document 7 · version 4".to_string()];
        assert_eq!(after, expected);
    }

    #[test]
    fn sdui_layout_regression_stale_update_leaves_snapshot_unchanged() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());
        let before = state.observable_snapshot(Size::new(900.0, 600.0));
        let mut stale_update = representative_panel_update();
        stale_update.base_ui_version = 99;

        assert!(!state.apply_update(stale_update));

        assert_eq!(state.observable_snapshot(Size::new(900.0, 600.0)), before);
    }

    #[test]
    fn sdui_layout_regression_snapshot_replaces_prior_tree() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());

        state.apply_snapshot(SduiTree {
            ui_version: 8,
            root_id: SduiNodeId(20),
            nodes: vec![
                SduiNode::new(
                    SduiNodeId(20),
                    SduiNodeKind::Stack {
                        children: vec![SduiNodeId(21)],
                    },
                ),
                SduiNode::new(
                    SduiNodeId(21),
                    SduiNodeKind::Label {
                        text: "Replacement status".to_string(),
                    },
                ),
            ],
        });
        let snapshot = state.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(snapshot.ui_version, 8);
        assert_eq!(
            snapshot.node_kinds,
            vec![(SduiNodeId(20), "Stack"), (SduiNodeId(21), "Label")]
        );
        assert_eq!(snapshot.label_texts, vec!["Replacement status".to_string()]);
        assert!(snapshot.panel_titles.is_empty());
        assert!(snapshot.button_labels.is_empty());
        assert!(snapshot.list_items.is_empty());
        assert!(snapshot.editor_bindings.is_empty());
        assert!(!snapshot.editor_region_non_empty);
    }

    #[test]
    fn sdui_layout_regression_empty_after_root_remove() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());

        assert!(state.apply_update(SduiTreeUpdate {
            base_ui_version: 1,
            new_ui_version: 2,
            operations: vec![SduiTreeOperation::RemoveNode {
                node_id: SduiNodeId(1),
            }],
        }));
        let snapshot = state.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(snapshot.ui_version, 2);
        assert!(!snapshot.has_sidebar);
        assert!(!snapshot.editor_region_non_empty);
        assert!(snapshot.panel_titles.is_empty());
        assert!(snapshot.label_texts.is_empty());
        assert!(snapshot.button_labels.is_empty());
        assert!(snapshot.list_items.is_empty());
        assert!(snapshot.editor_bindings.is_empty());
    }

    #[test]
    fn sdui_accessibility_role_is_generic_container() {
        let state = SduiNativeState::empty();

        assert_eq!(state.accessibility_role(), Role::GenericContainer);
    }

    #[test]
    fn sdui_accessibility_panel_label_matches_title() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        assert!(state.accessibility_nodes().contains(&SduiAccessibleNode {
            role: Role::Pane,
            label: Some("Workspace".to_string()),
        }));
    }

    #[test]
    fn sdui_accessibility_button_label_matches_button_label() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        assert!(state.accessibility_nodes().contains(&SduiAccessibleNode {
            role: Role::Button,
            label: Some("Refresh".to_string()),
        }));
    }

    #[test]
    fn sdui_accessibility_list_items_match_item_labels() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        assert!(state.accessibility_nodes().contains(&SduiAccessibleNode {
            role: Role::ListItem,
            label: Some("Document 7".to_string()),
        }));
    }

    #[test]
    fn sdui_accessibility_representative_tree_covers_all_node_kinds() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(representative_sdui_tree());

        let nodes = state.accessibility_nodes();
        assert_eq!(
            nodes.iter().filter(|node| node.role == Role::Pane).count(),
            3
        );
        assert!(nodes.iter().any(|node| node.role == Role::Label));
        assert!(nodes.iter().any(|node| node.role == Role::Button));
        assert!(nodes.iter().any(|node| node.role == Role::List));
        assert!(nodes.iter().any(|node| node.role == Role::ListItem));
        assert!(
            nodes
                .iter()
                .any(|node| node.role == Role::MultilineTextInput)
        );
    }

    #[test]
    fn sdui_accessibility_editor_view_label_includes_document_id() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        assert!(state.accessibility_nodes().contains(&SduiAccessibleNode {
            role: Role::MultilineTextInput,
            label: Some("Editor document 7".to_string()),
        }));
    }

    #[test]
    fn sdui_accessibility_empty_state_does_not_panic() {
        let state = SduiNativeState::empty();

        assert!(state.accessibility_nodes().is_empty());
    }

    #[test]
    fn sdui_accessibility_labels_are_stable_for_equivalent_trees() {
        let mut first = SduiNativeState::empty();
        let mut second = SduiNativeState::empty();
        first.apply_snapshot(sample_tree());
        second.apply_snapshot(sample_tree());

        assert_eq!(first.accessibility_nodes(), second.accessibility_nodes());
    }
}
