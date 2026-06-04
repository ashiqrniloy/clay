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
    DocumentId, SduiActionIntent, SduiEditorBinding, SduiFlexDirection, SduiNode, SduiNodeId,
    SduiNodeKind, SduiTree, SduiTreeOperation, SduiTreeUpdate, SduiVersion,
};

const SIDEBAR_WIDTH: f64 = 240.0;
const PANEL_PADDING: f64 = 14.0;
const ROW_HEIGHT: f64 = 26.0;
const TITLE_TEXT_SIZE: f32 = 14.0;
const BODY_TEXT_SIZE: f32 = 12.0;
const PANEL_BACKGROUND: Color = Color::from_rgb8(0x21, 0x20, 0x2b);
const BUTTON_BACKGROUND: Color = Color::from_rgb8(0x39, 0x35, 0x4a);
const LIST_BACKGROUND: Color = Color::from_rgb8(0x29, 0x28, 0x35);
const TEXT_COLOR: Color = Color::from_rgb8(0xee, 0xea, 0xff);
const MUTED_TEXT_COLOR: Color = Color::from_rgb8(0xb9, 0xb2, 0xcf);

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
}

#[derive(Debug, Clone, PartialEq)]
pub struct SduiNativeState {
    ui_version: SduiVersion,
    root_id: Option<SduiNodeId>,
    nodes: BTreeMap<SduiNodeId, SduiNode>,
    editor_binding: Option<SduiEditorBinding>,
    actions: Vec<SduiVisibleAction>,
}

impl SduiNativeState {
    pub fn empty() -> Self {
        Self {
            ui_version: 0,
            root_id: None,
            nodes: BTreeMap::new(),
            editor_binding: None,
            actions: Vec::new(),
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

    pub(crate) fn accessibility_nodes(&self) -> Vec<SduiAccessibleNode> {
        let mut nodes = Vec::new();
        if let Some(root_id) = self.root_id {
            let mut visited = BTreeSet::new();
            self.collect_accessibility_nodes(root_id, &mut visited, &mut nodes);
        }
        nodes
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        self.actions.clear();
        let Some(root_id) = self.root_id else {
            return;
        };
        let size = ctx.size();
        let sidebar = Rect::new(0.0, 0.0, SIDEBAR_WIDTH.min(size.width), size.height);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            PANEL_BACKGROUND,
            None,
            &sidebar,
        );
        let mut cursor_y = PANEL_PADDING;
        self.paint_node(ctx, scene, root_id, 0, &mut cursor_y, sidebar.width());
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
                    TITLE_TEXT_SIZE,
                    TEXT_COLOR,
                );
                *cursor_y += ROW_HEIGHT;
                for child_id in children {
                    self.paint_node(ctx, scene, child_id, depth + 1, cursor_y, width);
                }
            }
            SduiNodeKind::Label { text } => {
                self.paint_text(
                    ctx,
                    scene,
                    &text,
                    depth,
                    *cursor_y,
                    BODY_TEXT_SIZE,
                    MUTED_TEXT_COLOR,
                );
                *cursor_y += ROW_HEIGHT;
            }
            SduiNodeKind::Button { label, action } => {
                let rect = row_rect(depth, *cursor_y, width);
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    BUTTON_BACKGROUND,
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
                    BODY_TEXT_SIZE,
                    TEXT_COLOR,
                );
                *cursor_y += ROW_HEIGHT + 6.0;
            }
            SduiNodeKind::List { items } => {
                for item in items {
                    let rect = row_rect(depth, *cursor_y, width);
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        LIST_BACKGROUND,
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
                        BODY_TEXT_SIZE,
                        TEXT_COLOR,
                    );
                    if let Some(detail) = item.detail {
                        self.paint_text(
                            ctx,
                            scene,
                            &detail,
                            depth,
                            *cursor_y + 15.0,
                            10.0,
                            MUTED_TEXT_COLOR,
                        );
                    }
                    *cursor_y += ROW_HEIGHT + 10.0;
                }
            }
            SduiNodeKind::EditorView { binding } => {
                self.paint_text(
                    ctx,
                    scene,
                    &format!("Editor view · doc {}", binding.document_id),
                    depth,
                    *cursor_y,
                    BODY_TEXT_SIZE,
                    MUTED_TEXT_COLOR,
                );
                *cursor_y += ROW_HEIGHT;
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
                            self.paint_node(ctx, scene, child_id, depth, cursor_y, width);
                        }
                    }
                }
                SduiFlexDirection::Column => {
                    for child_id in children {
                        self.paint_node(ctx, scene, child_id, depth, cursor_y, width);
                    }
                }
            },
            SduiNodeKind::Stack { children } => {
                for child_id in children {
                    self.paint_node(ctx, scene, child_id, depth, cursor_y, width);
                }
            }
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
    ) {
        let max_width = (SIDEBAR_WIDTH - PANEL_PADDING * 2.0 - depth as f64 * 10.0).max(1.0) as f32;
        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.2)));
        builder.push_default(StyleProperty::Brush(BrushIndex(0)));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(max_width));
        render_text(
            scene,
            Affine::translate((PANEL_PADDING + depth as f64 * 10.0, y)),
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
    if sdui.editor_binding().is_some() && size.width > SIDEBAR_WIDTH + 100.0 {
        Rect::new(SIDEBAR_WIDTH, 0.0, size.width, size.height)
    } else {
        size.to_rect()
    }
}

pub fn editor_region_for_document(
    size: Size,
    sdui: &SduiNativeState,
    document_id: DocumentId,
) -> Rect {
    match sdui.editor_binding() {
        Some(binding) if binding.document_id == document_id => editor_region(size, sdui),
        _ => size.to_rect(),
    }
}

fn row_rect(depth: usize, y: f64, width: f64) -> Rect {
    let x0 = PANEL_PADDING + depth as f64 * 10.0;
    Rect::new(x0, y, (width - PANEL_PADDING).max(x0), y + ROW_HEIGHT)
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
