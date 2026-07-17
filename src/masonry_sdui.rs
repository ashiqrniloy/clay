#![allow(
    dead_code,
    reason = "SDUI observability/package UI bridge structs are staged for runtime wiring and covered by docs/tests before every callsite is live"
)]
#![allow(
    clippy::too_many_arguments,
    reason = "Masonry paint/layout helpers pass explicit render context and geometry instead of hiding hot-path state in heap structs"
)]

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

use crate::{
    editor::typography::{TypographyRegistry, UiTextMetrics, UiTextVariant},
    perf::metrics::global_recorder,
    protocol::{
        DocumentId, FontRole, SduiActionIntent, SduiActionSource, SduiEditorBinding,
        SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation,
        SduiTreeUpdate, SduiVersion,
    },
    shell::{
        CompletionMenuAcceptAction, FixedSlotId, FixedSlotState, PackageUiComponentTree,
        PackageUiOverlayObservation, PackageUiPanelObservation, PackageUiRuntimeError,
        PackageUiRuntimeState, PackageUiRuntimeUpdate, PaneSlotLayout, TransientMenuSession,
        layout::PaneSlotId, theme::SduiThemeStyle,
    },
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

#[derive(Debug, Clone, PartialEq)]
struct SduiAccessibilityEntry {
    role: Role,
    label: Option<String>,
    bounds: Rect,
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
    typography: TypographyRegistry,
    active_menu: Option<TransientMenuSession>,
    // Client-local vertical scroll offset (pixels) for the Clay-owned left
    // file-browser panel. Scroll reveals already-listed rows only; it never
    // relists directories or calls the server.
    scroll_offset: f64,
    // Last measured content (rows) and viewport heights of the left panel,
    // captured during paint so scroll clamping can run without repainting.
    content_height: f64,
    viewport_height: f64,
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
            typography: TypographyRegistry::default(),
            active_menu: None,
            scroll_offset: 0.0,
            content_height: 0.0,
            viewport_height: 0.0,
        }
    }

    pub(crate) fn active_menu(&self) -> Option<&TransientMenuSession> {
        self.active_menu.as_ref()
    }

    pub(crate) fn set_typography(&mut self, typography: TypographyRegistry) {
        if self.typography == typography {
            return;
        }
        self.typography = typography;
        self.scroll_offset = 0.0;
        self.content_height = 0.0;
        self.viewport_height = 0.0;
        self.actions.clear();
    }

    fn text_metrics(&self, role: FontRole, variant: UiTextVariant) -> UiTextMetrics {
        self.typography.ui_text_metrics(role, variant)
    }

    fn body_metrics(&self) -> UiTextMetrics {
        self.text_metrics(FontRole::Ui, sdui_theme_style().body_text)
    }

    fn component_variant(
        component: &PackageUiComponentTree,
        fallback: UiTextVariant,
    ) -> UiTextVariant {
        component.text_variant.unwrap_or(fallback)
    }

    fn component_metrics(
        &self,
        component: &PackageUiComponentTree,
        fallback: UiTextVariant,
    ) -> UiTextMetrics {
        self.text_metrics(
            component.font_role,
            Self::component_variant(component, fallback),
        )
    }

    pub(crate) fn set_active_menu(&mut self, menu: TransientMenuSession) {
        self.active_menu = Some(menu);
    }

    pub(crate) fn clear_active_menu(&mut self) {
        self.active_menu = None;
    }

    pub(crate) fn menu_select_next(&mut self) {
        if let Some(menu) = &mut self.active_menu {
            menu.select_next();
        }
    }

    pub(crate) fn menu_select_previous(&mut self) {
        if let Some(menu) = &mut self.active_menu {
            menu.select_previous();
        }
    }

    pub(crate) fn menu_activate_selected(&mut self) -> Option<crate::protocol::SduiActionIntent> {
        let menu = self.active_menu.as_ref()?;
        let action = menu.activate_selected()?;
        if action.completion_accept.is_some() {
            return None;
        }
        Some(crate::protocol::SduiActionIntent {
            command_id: action.command_id.clone(),
            source: crate::protocol::SduiActionSource::ListItem {
                node_id: crate::protocol::SduiNodeId(menu.session_id().0),
                item_id: menu.selected_index().to_string(),
            },
            arguments: json_object_to_sdui_arguments(&action.arguments),
        })
    }

    /// Returns the selected transient-menu action without converting to SDUI,
    /// so local language-intelligence handlers can inspect typed arguments.
    pub(crate) fn menu_selected_action(
        &self,
    ) -> Option<crate::shell::transient_menu::TransientMenuAction> {
        self.active_menu
            .as_ref()
            .and_then(crate::shell::TransientMenuSession::activate_selected)
            .cloned()
    }

    pub(crate) fn menu_activate_completion(&mut self) -> Option<CompletionMenuAcceptAction> {
        let menu = self.active_menu.as_ref()?;
        menu.activate_selected()?.completion_accept.clone()
    }

    pub(crate) fn menu_cancel(&mut self) {
        if let Some(menu) = &mut self.active_menu {
            menu.cancel();
        }
    }

    pub fn apply_snapshot(&mut self, tree: SduiTree) {
        let recorder = global_recorder();
        let _scope = recorder.scope("sdui.apply_snapshot");
        recorder.record_gauge("sdui.snapshot.node_count", tree.nodes.len() as u64);
        self.ui_version = tree.ui_version;
        self.root_id = Some(tree.root_id);
        self.nodes = tree.nodes.into_iter().map(|node| (node.id, node)).collect();
        self.scroll_offset = 0.0;
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
        self.scroll_offset = 0.0;
        self.rebuild_derived_state();
        true
    }

    pub fn ui_version(&self) -> SduiVersion {
        self.ui_version
    }

    pub(crate) fn typography_revision(&self) -> u64 {
        self.typography.revision()
    }

    pub(crate) fn apply_package_ui_update(
        &mut self,
        update: PackageUiRuntimeUpdate,
    ) -> Result<(), PackageUiRuntimeError> {
        self.package_ui.apply_update(update)?;
        self.actions.clear();
        Ok(())
    }

    pub(crate) fn install_package_ui_snapshot(
        &mut self,
        snapshot: &crate::protocol::PackageUiSnapshot,
    ) {
        self.package_ui.install_runtime_snapshot(snapshot);
        self.actions.clear();
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

    /// True when `point` lies inside the Clay-owned left file-browser panel,
    /// i.e. scroll events here should scroll the file browser, not the editor.
    pub(crate) fn scrolls_point(&self, size: Size, point: Point) -> bool {
        sdui_panel_left_slot_rect(size, self).is_some_and(|rect| rect.contains(point))
    }

    /// Scroll the left file browser by `delta` pixels (positive = down). Returns
    /// true when the scroll offset changed. Client-local paint math only.
    pub(crate) fn scroll_vertical_pixels(&mut self, size: Size, delta: f64) -> bool {
        let viewport = sdui_panel_left_slot_rect(size, self).map_or(0.0, |r| r.height());
        self.viewport_height = viewport;
        let max_scroll = (self.content_height - self.viewport_height).max(0.0);
        let next = (self.scroll_offset + delta).clamp(0.0, max_scroll);
        if next == self.scroll_offset {
            return false;
        }
        self.scroll_offset = next;
        true
    }

    /// Scroll the left file browser by whole rows (positive = down).
    pub(crate) fn scroll_lines(&mut self, size: Size, lines: isize) -> bool {
        self.scroll_vertical_pixels(size, lines as f64 * self.body_metrics().row_height)
    }

    pub(crate) fn scroll_offset(&self) -> f64 {
        self.scroll_offset
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
        let mut overlays: Vec<PackageUiOverlayObservation> = self
            .package_ui
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
            .collect();
        if let Some(menu) = &self.active_menu
            && menu.is_active()
        {
            let overlay = crate::shell::TransientPackageOverlay::from_menu_session(menu);
            overlays.push(PackageUiOverlayObservation {
                id: overlay.id,
                anchor: overlay.anchor,
                rect: overlay
                    .anchor
                    .rect(widget_size.to_rect(), slot_geometry.main_rect),
                component_id: overlay.component.id,
                component_kind: overlay.component.kind,
                focus_policy: overlay.focus_policy,
                dismissal_policy: overlay.dismissal_policy,
            });
        }
        overlays
    }

    pub(crate) fn accessibility_nodes(&self) -> Vec<SduiAccessibleNode> {
        let mut nodes = Vec::new();
        if let Some(root_id) = self.root_id {
            let mut visited = BTreeSet::new();
            self.collect_accessibility_nodes(root_id, &mut visited, &mut nodes);
        }
        nodes
    }

    pub(crate) fn append_accessibility_children(&self, ctx: &mut AccessCtx<'_>) -> Vec<NodeId> {
        self.accessibility_entries(ctx.size())
            .into_iter()
            .map(|entry| {
                let id = NodeId::from(WidgetId::next());
                let mut node = Node::new(entry.role);
                if let Some(label) = entry.label {
                    node.set_label(label);
                }
                node.set_bounds(masonry::accesskit::Rect {
                    x0: entry.bounds.x0,
                    y0: entry.bounds.y0,
                    x1: entry.bounds.x1,
                    y1: entry.bounds.y1,
                });
                ctx.tree_update().nodes.push((id, node));
                id
            })
            .collect()
    }

    fn accessibility_entries(&self, size: Size) -> Vec<SduiAccessibilityEntry> {
        let mut entries = Vec::new();
        if let Some(root_id) = self.root_id
            && let Some(sidebar) = sdui_panel_left_slot_rect(size, self)
        {
            let mut cursor_y = sidebar.y0 + sdui_theme_style().panel_padding - self.scroll_offset;
            self.collect_accessibility_entries(
                root_id,
                0,
                &mut cursor_y,
                sidebar.width(),
                sidebar.x0,
                &mut entries,
            );
        }
        for (rect, panel) in self.package_ui.visible_fixed_panels(size.to_rect()) {
            let mut cursor_y = rect.y0 + sdui_theme_style().panel_padding;
            self.collect_package_accessibility_entries(
                &panel.component,
                0,
                &mut cursor_y,
                rect.width(),
                rect.x0,
                &mut entries,
            );
        }
        let slot_geometry = combined_slot_layout(size, self).compute_geometry(size.to_rect());
        for overlay in self.package_ui.overlays() {
            let rect = overlay.anchor.rect(size.to_rect(), slot_geometry.main_rect);
            let mut cursor_y = rect.y0 + sdui_theme_style().panel_padding;
            self.collect_package_accessibility_entries(
                &overlay.component,
                0,
                &mut cursor_y,
                rect.width(),
                rect.x0,
                &mut entries,
            );
        }
        if let Some(menu) = &self.active_menu
            && menu.is_active()
        {
            let overlay = crate::shell::TransientPackageOverlay::from_menu_session(menu);
            let rect = overlay.anchor.rect(size.to_rect(), slot_geometry.main_rect);
            let mut cursor_y = rect.y0 + sdui_theme_style().panel_padding;
            self.collect_active_menu_accessibility_entries(menu, rect, &mut cursor_y, &mut entries);
        }
        entries
    }

    fn collect_active_menu_accessibility_entries(
        &self,
        menu: &crate::shell::TransientMenuSession,
        rect: Rect,
        cursor_y: &mut f64,
        entries: &mut Vec<SduiAccessibilityEntry>,
    ) {
        let body = self.body_metrics();
        let prompt = crate::editor::accessibility::sanitize_recovery_summary(menu.prompt())
            .unwrap_or_else(|| "Transient menu".to_string());
        entries.push(SduiAccessibilityEntry {
            role: Role::Menu,
            label: Some(prompt.clone()),
            bounds: Rect::new(rect.x0, rect.y0, rect.x1, rect.y1),
        });
        *cursor_y += body.row_height;
        for (index, item) in menu.items().iter().enumerate() {
            let selected = index == menu.selected_index();
            let base = if item.accessibility_label.trim().is_empty() {
                item.label.clone()
            } else {
                item.accessibility_label.clone()
            };
            let label = if selected {
                format!("{base} selected")
            } else {
                base
            };
            entries.push(SduiAccessibilityEntry {
                role: Role::MenuItem,
                label: Some(label),
                bounds: row_rect(0, *cursor_y, rect.width(), rect.x0, body.row_height),
            });
            *cursor_y += body.row_height;
        }
        if let crate::shell::TransientMenuStatus::Empty { message } = menu.status()
            && let Some(summary) = crate::editor::accessibility::sanitize_recovery_summary(message)
        {
            entries.push(SduiAccessibilityEntry {
                role: Role::Status,
                label: Some(summary),
                bounds: row_rect(0, *cursor_y, rect.width(), rect.x0, body.row_height),
            });
        }
    }

    fn collect_accessibility_entries(
        &self,
        node_id: SduiNodeId,
        depth: usize,
        cursor_y: &mut f64,
        width: f64,
        origin_x: f64,
        entries: &mut Vec<SduiAccessibilityEntry>,
    ) {
        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };
        let body = self.body_metrics();
        match &node.kind {
            SduiNodeKind::Panel { title, children } => {
                let height = self
                    .text_metrics(FontRole::Ui, sdui_theme_style().title_text)
                    .row_height;
                entries.push(SduiAccessibilityEntry {
                    role: Role::Pane,
                    label: Some(title.clone()),
                    bounds: row_rect(depth, *cursor_y, width, origin_x, height),
                });
                *cursor_y += height;
                for child_id in children {
                    self.collect_accessibility_entries(
                        *child_id,
                        depth + 1,
                        cursor_y,
                        width,
                        origin_x,
                        entries,
                    );
                }
            }
            SduiNodeKind::Label { text } => {
                entries.push(SduiAccessibilityEntry {
                    role: Role::Label,
                    label: Some(text.clone()),
                    bounds: row_rect(depth, *cursor_y, width, origin_x, body.row_height),
                });
                *cursor_y += body.row_height;
            }
            SduiNodeKind::Button { label, .. } => {
                let height = body.button_height();
                entries.push(SduiAccessibilityEntry {
                    role: Role::Button,
                    label: Some(label.clone()),
                    bounds: row_rect(depth, *cursor_y, width, origin_x, height),
                });
                *cursor_y += height;
            }
            SduiNodeKind::List { items } => {
                let row_height =
                    body.list_height(self.text_metrics(FontRole::Ui, UiTextVariant::Detail));
                let list_start = *cursor_y;
                for item in items {
                    entries.push(SduiAccessibilityEntry {
                        role: Role::ListItem,
                        label: Some(item.label.clone()),
                        bounds: row_rect(depth, *cursor_y, width, origin_x, row_height),
                    });
                    *cursor_y += row_height;
                }
                entries.push(SduiAccessibilityEntry {
                    role: Role::List,
                    label: None,
                    bounds: row_rect(depth, list_start, width, origin_x, *cursor_y - list_start),
                });
            }
            SduiNodeKind::EditorView { binding } => {
                entries.push(SduiAccessibilityEntry {
                    role: Role::MultilineTextInput,
                    label: Some(format!("Editor view for document {}", binding.document_id)),
                    bounds: row_rect(depth, *cursor_y, width, origin_x, body.row_height),
                });
                *cursor_y += body.row_height;
            }
            SduiNodeKind::Flex {
                direction,
                children,
            } => match direction {
                SduiFlexDirection::Row => {
                    for child_id in children {
                        if !matches!(
                            self.nodes.get(child_id).map(|node| &node.kind),
                            Some(SduiNodeKind::EditorView { .. })
                        ) {
                            self.collect_accessibility_entries(
                                *child_id, depth, cursor_y, width, origin_x, entries,
                            );
                        }
                    }
                }
                SduiFlexDirection::Column => {
                    for child_id in children {
                        self.collect_accessibility_entries(
                            *child_id, depth, cursor_y, width, origin_x, entries,
                        );
                    }
                }
            },
            SduiNodeKind::Stack { children } => {
                for child_id in children {
                    self.collect_accessibility_entries(
                        *child_id, depth, cursor_y, width, origin_x, entries,
                    );
                }
            }
        }
    }

    fn collect_package_accessibility_entries(
        &self,
        component: &PackageUiComponentTree,
        depth: usize,
        cursor_y: &mut f64,
        width: f64,
        origin_x: f64,
        entries: &mut Vec<SduiAccessibilityEntry>,
    ) {
        let body = self.component_metrics(component, sdui_theme_style().body_text);
        match component.kind.as_str() {
            "panel" => {
                if let Some(title) = &component.title {
                    let variant = Self::component_variant(component, sdui_theme_style().title_text);
                    let height = self.text_metrics(component.font_role, variant).row_height;
                    entries.push(SduiAccessibilityEntry {
                        role: Role::Pane,
                        label: Some(title.clone()),
                        bounds: row_rect(depth, *cursor_y, width, origin_x, height),
                    });
                    *cursor_y += height;
                }
                for child in &component.children {
                    self.collect_package_accessibility_entries(
                        child,
                        depth + 1,
                        cursor_y,
                        width,
                        origin_x,
                        entries,
                    );
                }
            }
            "label" | "statusItem" => {
                let label = component
                    .text
                    .as_deref()
                    .or(component.label.as_deref())
                    .unwrap_or(&component.id)
                    .to_string();
                entries.push(SduiAccessibilityEntry {
                    role: if component.kind == "statusItem" {
                        Role::Status
                    } else {
                        Role::Label
                    },
                    label: Some(label),
                    bounds: row_rect(depth, *cursor_y, width, origin_x, body.row_height),
                });
                *cursor_y += body.row_height;
            }
            "button" => {
                let height = body.button_height();
                entries.push(SduiAccessibilityEntry {
                    role: Role::Button,
                    label: Some(
                        component
                            .label
                            .clone()
                            .unwrap_or_else(|| component.id.clone()),
                    ),
                    bounds: row_rect(depth, *cursor_y, width, origin_x, height),
                });
                *cursor_y += height;
            }
            "list" => {
                let row_height =
                    body.list_height(self.text_metrics(component.font_role, UiTextVariant::Detail));
                let list_start = *cursor_y;
                for item in &component.items {
                    entries.push(SduiAccessibilityEntry {
                        role: Role::ListItem,
                        label: Some(item.label.clone()),
                        bounds: row_rect(depth, *cursor_y, width, origin_x, row_height),
                    });
                    *cursor_y += row_height;
                }
                entries.push(SduiAccessibilityEntry {
                    role: Role::List,
                    label: None,
                    bounds: row_rect(depth, list_start, width, origin_x, *cursor_y - list_start),
                });
            }
            "editorView" => {
                entries.push(SduiAccessibilityEntry {
                    role: Role::MultilineTextInput,
                    label: Some(format!("Editor view · {}", component.id)),
                    bounds: row_rect(depth, *cursor_y, width, origin_x, body.row_height),
                });
                *cursor_y += body.row_height;
            }
            "flex" | "stack" | "overlay" | "scroll" | "portal" => {
                for child in &component.children {
                    self.collect_package_accessibility_entries(
                        child, depth, cursor_y, width, origin_x, entries,
                    );
                }
            }
            _ => {}
        }
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
        self.viewport_height = sidebar.height();
        let mut cursor_y = sidebar.y0 + sdui_theme_style().panel_padding - self.scroll_offset;
        self.collect_action_regions(root_id, 0, &mut cursor_y, sidebar.width(), sidebar.x0);
        self.content_height = (cursor_y - sidebar.y0 + self.scroll_offset).max(0.0);
        let max_scroll = (self.content_height - self.viewport_height).max(0.0);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
    }

    pub fn paint(&mut self, ctx: &mut PaintCtx<'_>, scene: &mut Scene) {
        self.actions.clear();
        self.paint_package_fixed_panels(ctx, scene);
        if let Some(root_id) = self.root_id
            && let Some(sidebar) = sdui_panel_left_slot_rect(ctx.size(), self)
        {
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                sdui_theme_style().panel_background,
                None,
                &sidebar,
            );
            self.viewport_height = sidebar.height();
            // Clip panel content to the sidebar so scrolled-out rows do not
            // paint over the editor main region.
            scene.push_clip_layer(Affine::IDENTITY, &sidebar);
            let mut cursor_y = sidebar.y0 + sdui_theme_style().panel_padding - self.scroll_offset;
            self.paint_node(
                ctx,
                scene,
                root_id,
                0,
                &mut cursor_y,
                sidebar.width(),
                sidebar.x0,
            );
            // Content height is independent of the current scroll offset: add
            // it back so clamping reflects the full row extent.
            self.content_height = (cursor_y - sidebar.y0 + self.scroll_offset).max(0.0);
            scene.pop_layer();
            let max_scroll = (self.content_height - self.viewport_height).max(0.0);
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
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
                label: Some(format!("Editor view for document {}", binding.document_id)),
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
        let body = self.body_metrics();
        match node.kind {
            SduiNodeKind::Panel { children, .. } => {
                *cursor_y += self
                    .text_metrics(FontRole::Ui, sdui_theme_style().title_text)
                    .row_height;
                for child_id in children {
                    self.collect_action_regions(child_id, depth + 1, cursor_y, width, origin_x);
                }
            }
            SduiNodeKind::Label { .. } | SduiNodeKind::EditorView { .. } => {
                *cursor_y += body.row_height;
            }
            SduiNodeKind::Button { action, .. } => {
                self.actions.push(SduiVisibleAction {
                    rect: row_rect(depth, *cursor_y, width, origin_x, body.button_height()),
                    intent: action,
                });
                *cursor_y += body.button_height();
            }
            SduiNodeKind::List { items } => {
                let row_height =
                    body.list_height(self.text_metrics(FontRole::Ui, UiTextVariant::Detail));
                for item in items {
                    if let Some(action) = item.action {
                        self.actions.push(SduiVisibleAction {
                            rect: row_rect(depth, *cursor_y, width, origin_x, row_height),
                            intent: action,
                        });
                    }
                    *cursor_y += row_height;
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
        let metrics = self.component_metrics(component, sdui_theme_style().body_text);
        match component.kind.as_str() {
            "panel" => {
                if component.title.is_some() {
                    *cursor_y += self
                        .component_metrics(component, sdui_theme_style().title_text)
                        .row_height;
                }
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
                        rect: row_rect(depth, *cursor_y, width, origin_x, metrics.button_height()),
                        intent: package_action_intent(command_id, &component.id),
                    });
                }
                *cursor_y += metrics.button_height();
            }
            "list" => {
                let row_height = metrics
                    .list_height(self.text_metrics(component.font_role, UiTextVariant::Detail));
                for item in &component.items {
                    if let Some(command_id) = &item.action_command_id {
                        self.actions.push(SduiVisibleAction {
                            rect: row_rect(depth, *cursor_y, width, origin_x, row_height),
                            intent: package_action_intent(
                                command_id,
                                &format!("{}.{}", component.id, item.id),
                            ),
                        });
                    }
                    *cursor_y += row_height;
                }
            }
            "label" | "statusItem" | "editorView" => {
                *cursor_y += metrics.row_height;
            }
            "flex" | "stack" | "overlay" | "scroll" | "portal" => {
                for child in &component.children {
                    self.collect_package_action_regions(child, depth, cursor_y, width, origin_x);
                }
            }
            _ => {}
        }
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
                let variant = sdui_theme_style().title_text;
                self.paint_text(
                    ctx,
                    scene,
                    &title,
                    depth,
                    *cursor_y,
                    width,
                    origin_x,
                    FontRole::Ui,
                    variant,
                    sdui_theme_style().text_color,
                );
                *cursor_y += self.text_metrics(FontRole::Ui, variant).row_height;
                for child_id in children {
                    self.paint_node(ctx, scene, child_id, depth + 1, cursor_y, width, origin_x);
                }
            }
            SduiNodeKind::Label { text } => {
                let variant = sdui_theme_style().body_text;
                self.paint_text(
                    ctx,
                    scene,
                    &text,
                    depth,
                    *cursor_y,
                    width,
                    origin_x,
                    FontRole::Ui,
                    variant,
                    sdui_theme_style().muted_text_color,
                );
                *cursor_y += self.text_metrics(FontRole::Ui, variant).row_height;
            }
            SduiNodeKind::Button { label, action } => {
                let variant = sdui_theme_style().body_text;
                let metrics = self.text_metrics(FontRole::Ui, variant);
                let rect = row_rect(depth, *cursor_y, width, origin_x, metrics.button_height());
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
                    *cursor_y + (metrics.button_height() - metrics.line_height) / 2.0,
                    width,
                    origin_x,
                    FontRole::Ui,
                    variant,
                    sdui_theme_style().text_color,
                );
                *cursor_y += metrics.button_height();
            }
            SduiNodeKind::List { items } => {
                let variant = sdui_theme_style().body_text;
                let metrics = self.text_metrics(FontRole::Ui, variant);
                let detail_metrics = self.text_metrics(FontRole::Ui, UiTextVariant::Detail);
                let row_height = metrics.list_height(detail_metrics);
                for item in items {
                    let rect = row_rect(depth, *cursor_y, width, origin_x, row_height);
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
                        *cursor_y,
                        width,
                        origin_x,
                        FontRole::Ui,
                        variant,
                        sdui_theme_style().text_color,
                    );
                    if let Some(detail) = item.detail {
                        self.paint_text(
                            ctx,
                            scene,
                            &detail,
                            depth,
                            *cursor_y + metrics.line_height,
                            width,
                            origin_x,
                            FontRole::Ui,
                            UiTextVariant::Detail,
                            sdui_theme_style().muted_text_color,
                        );
                    }
                    *cursor_y += row_height;
                }
            }
            SduiNodeKind::EditorView { binding } => {
                let variant = sdui_theme_style().body_text;
                self.paint_text(
                    ctx,
                    scene,
                    &format!("Editor view · doc {}", binding.document_id),
                    depth,
                    *cursor_y,
                    width,
                    origin_x,
                    FontRole::Ui,
                    variant,
                    sdui_theme_style().muted_text_color,
                );
                *cursor_y += self.text_metrics(FontRole::Ui, variant).row_height;
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
        let mut overlays: Vec<crate::shell::TransientPackageOverlay> =
            self.package_ui.overlays().cloned().collect();
        if let Some(menu) = &self.active_menu
            && menu.is_active()
        {
            overlays.push(crate::shell::TransientPackageOverlay::from_menu_session(
                menu,
            ));
        }
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
                    let variant = Self::component_variant(component, sdui_theme_style().title_text);
                    self.paint_text(
                        ctx,
                        scene,
                        title,
                        depth,
                        *cursor_y,
                        width,
                        origin_x,
                        component.font_role,
                        variant,
                        sdui_theme_style().text_color,
                    );
                    *cursor_y += self.text_metrics(component.font_role, variant).row_height;
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
                let fallback = if component.kind == "statusItem" {
                    sdui_theme_style().status_text
                } else {
                    sdui_theme_style().body_text
                };
                let variant = Self::component_variant(component, fallback);
                let metrics = self.text_metrics(component.font_role, variant);
                self.paint_text(
                    ctx,
                    scene,
                    text,
                    depth,
                    *cursor_y,
                    width,
                    origin_x,
                    component.font_role,
                    variant,
                    sdui_theme_style().muted_text_color,
                );
                *cursor_y += metrics.row_height;
            }
            "button" => {
                let variant = Self::component_variant(component, sdui_theme_style().body_text);
                let metrics = self.text_metrics(component.font_role, variant);
                let rect = row_rect(depth, *cursor_y, width, origin_x, metrics.button_height());
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
                    *cursor_y + (metrics.button_height() - metrics.line_height) / 2.0,
                    width,
                    origin_x,
                    component.font_role,
                    variant,
                    sdui_theme_style().text_color,
                );
                *cursor_y += metrics.button_height();
            }
            "list" => {
                let variant = Self::component_variant(component, sdui_theme_style().body_text);
                let metrics = self.text_metrics(component.font_role, variant);
                let detail_metrics = self.text_metrics(component.font_role, UiTextVariant::Detail);
                let row_height = metrics.list_height(detail_metrics);
                for item in &component.items {
                    let rect = row_rect(depth, *cursor_y, width, origin_x, row_height);
                    let background = if item.selected {
                        sdui_theme_style().selected_background
                    } else {
                        sdui_theme_style().list_background
                    };
                    scene.fill(Fill::NonZero, Affine::IDENTITY, background, None, &rect);
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
                        *cursor_y,
                        width,
                        origin_x,
                        component.font_role,
                        variant,
                        sdui_theme_style().text_color,
                    );
                    if let Some(detail) = &item.detail {
                        self.paint_text(
                            ctx,
                            scene,
                            detail,
                            depth,
                            *cursor_y + metrics.line_height,
                            width,
                            origin_x,
                            component.font_role,
                            UiTextVariant::Detail,
                            sdui_theme_style().muted_text_color,
                        );
                    }
                    *cursor_y += row_height;
                }
            }
            "editorView" => {
                let variant = sdui_theme_style().body_text;
                self.paint_text(
                    ctx,
                    scene,
                    &format!("Editor view · {}", component.id),
                    depth,
                    *cursor_y,
                    width,
                    origin_x,
                    FontRole::Ui,
                    variant,
                    sdui_theme_style().muted_text_color,
                );
                *cursor_y += self.text_metrics(FontRole::Ui, variant).row_height;
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
        width: f64,
        origin_x: f64,
        role: FontRole,
        variant: UiTextVariant,
        color: Color,
    ) {
        let max_width =
            (width - sdui_theme_style().panel_padding * 2.0 - depth as f64 * 10.0).max(1.0) as f32;
        let metrics = self.text_metrics(role, variant);
        let (font_context, layout_context) = ctx.text_contexts();
        let mut builder = layout_context.ranged_builder(font_context, text, 1.0, true);
        builder.push_default(StyleProperty::FontStack(
            self.typography.profile(role).font_stack(),
        ));
        builder.push_default(StyleProperty::FontSize(metrics.font_size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            UiTextMetrics::LINE_HEIGHT_MULTIPLIER as f32,
        )));
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
        Role::Group
    }

    fn accessibility(
        &mut self,
        ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label("Server-driven UI panels");
        node.set_children(self.append_accessibility_children(ctx));
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
    _document_id: DocumentId,
) -> Rect {
    // Reserve the Clay-owned left file-browser slot whenever a Clay-owned SDUI
    // panel exists, even if its editor binding still points at the bootstrap
    // document while a freshly opened workspace document is active. Reserving
    // by panel presence (not binding match) keeps the editor main region from
    // overlapping the left file browser after a workspace file opens.
    let full_rect = size.to_rect();
    let mut layout = sdui.package_ui.slot_layout();
    if (sdui.root_id.is_some() || sdui.editor_binding().is_some())
        && size.width > SIDEBAR_WIDTH + 100.0
        && !layout.contains_slot(PaneSlotId::Left)
    {
        layout = layout.with_fixed_slot(fixed_sdui_left_slot());
    }
    layout.compute_geometry(full_rect).main_rect
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

fn row_rect(depth: usize, y: f64, width: f64, origin_x: f64, height: f64) -> Rect {
    let x0 = origin_x + sdui_theme_style().panel_padding + depth as f64 * 10.0;
    Rect::new(
        x0,
        y,
        (origin_x + width - sdui_theme_style().panel_padding).max(x0),
        y + height,
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

fn json_object_to_sdui_arguments(
    value: &serde_json::Value,
) -> Vec<crate::protocol::SduiActionArgument> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    object
        .iter()
        .filter_map(|(name, value)| {
            let sdui_value = match value {
                serde_json::Value::String(text) => {
                    crate::protocol::SduiActionValue::String(text.clone())
                }
                serde_json::Value::Bool(flag) => crate::protocol::SduiActionValue::Bool(*flag),
                serde_json::Value::Number(number) => {
                    if let Some(v) = number.as_u64() {
                        crate::protocol::SduiActionValue::U64(v)
                    } else if let Some(v) = number.as_i64() {
                        crate::protocol::SduiActionValue::I64(v)
                    } else {
                        return None;
                    }
                }
                _ => return None,
            };
            Some(crate::protocol::SduiActionArgument {
                name: name.clone(),
                value: sdui_value,
            })
        })
        .collect()
}

fn stable_package_source_id(source_id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in source_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash.max(1)
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
    use crate::shell::transient_menu::{
        TransientMenuItem, TransientMenuSession, TransientMenuSessionId,
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
        assert_eq!(style.title_text, UiTextVariant::Title);
        assert_eq!(style.body_text, UiTextVariant::Body);
        assert_eq!(style.status_text, UiTextVariant::Status);
        assert_eq!(style.panel_background, Color::from_rgb8(0x21, 0x20, 0x2b));
        assert_eq!(style.button_background, Color::from_rgb8(0x39, 0x35, 0x4a));
        assert_eq!(style.list_background, Color::from_rgb8(0x29, 0x28, 0x35));
    }

    #[test]
    fn ui_size_change_scales_row_hit_and_accessibility_bounds_together() {
        let size = Size::new(900.0, 600.0);
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());
        state.rebuild_action_regions_for_test(size);
        let before_action = state.actions[0].rect;
        let before_accessibility = state
            .accessibility_entries(size)
            .into_iter()
            .find(|entry| entry.role == Role::Button && entry.label.as_deref() == Some("Refresh"))
            .expect("Refresh accessibility entry")
            .bounds;

        let mut active = crate::protocol::ActiveTypography {
            revision: 1,
            ..crate::protocol::ActiveTypography::default()
        };
        active.ui.size = 24.0;
        state.set_typography(TypographyRegistry::from_active_typography(active).unwrap());
        state.rebuild_action_regions_for_test(size);
        let after_action = state.actions[0].rect;
        let after_accessibility = state
            .accessibility_entries(size)
            .into_iter()
            .find(|entry| entry.role == Role::Button && entry.label.as_deref() == Some("Refresh"))
            .expect("Refresh accessibility entry after typography update")
            .bounds;

        assert!(after_action.height() > before_action.height());
        assert_eq!(after_action, after_accessibility);
        assert_eq!(before_action, before_accessibility);
        assert!(
            (after_action.height() - state.body_metrics().button_height()).abs() < 0.001,
            "paint, hit test, and accessibility use one UI metric"
        );
    }

    #[test]
    fn package_component_font_role_uses_selected_profile_without_concrete_sizes() {
        let component = PackageUiComponentTree::from_declaration(&json!({
            "kind": "panel",
            "id": "markdown.preview.root",
            "title": "Preview",
            "children": [{
                "kind": "button",
                "id": "markdown.preview.toggle",
                "label": "Toggle",
                "style": { "fontRole": "monospace", "typography": "typography.body" },
                "action": { "commandId": "markdown.togglePreview" }
            }]
        }))
        .unwrap();
        assert_eq!(component.children[0].font_role, FontRole::Monospace);
        assert_eq!(
            component.children[0].text_variant,
            Some(UiTextVariant::Body)
        );

        let size = Size::new(900.0, 600.0);
        let mut state = SduiNativeState::empty();
        state
            .apply_package_ui_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Right,
                    PackagePanelVisibility::Visible,
                    component,
                    vec!["markdown.togglePreview".to_string()],
                )],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .unwrap();
        let mut active = crate::protocol::ActiveTypography {
            revision: 1,
            ..crate::protocol::ActiveTypography::default()
        };
        active.ui.size = 10.0;
        active.monospace.size = 24.0;
        state.set_typography(TypographyRegistry::from_active_typography(active).unwrap());
        state.rebuild_action_regions_for_test(size);

        let action = state.actions.first().expect("package button action").rect;
        assert!(
            (action.height()
                - state
                    .text_metrics(FontRole::Monospace, UiTextVariant::Body)
                    .button_height())
            .abs()
                < 0.001
        );
    }

    #[test]
    fn workspace_browser_reserves_left_slot_after_document_id_changes() {
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(sample_tree());

        // The active document ID (999) differs from the SDUI editor binding
        // (document 7), as happens when a freshly opened workspace file becomes
        // active while the Clay-owned file browser still binds the bootstrap
        // document. The editor main region must still exclude the left
        // file-browser slot instead of falling back to the full rect and
        // overlapping the panel.
        let region = editor_region_for_document(Size::new(900.0, 600.0), &state, 999);

        assert_eq!(region.x0, SIDEBAR_WIDTH);
        assert_eq!(region.x1, 900.0);
        assert_eq!(region.y0, 0.0);
        assert_eq!(region.y1, 600.0);
    }

    fn browser_tree_with_rows(row_count: usize) -> SduiTree {
        let root = SduiNodeId(1);
        let panel = SduiNodeId(2);
        let list = SduiNodeId(3);
        let items: Vec<SduiListItem> = (0..row_count)
            .map(|index| SduiListItem {
                id: format!("item-{index}"),
                label: format!("Row {index}"),
                detail: None,
                action: Some(SduiActionIntent::command(
                    "clay.workspace.openFile",
                    SduiActionSource::ListItem {
                        node_id: list,
                        item_id: format!("item-{index}"),
                    },
                )),
            })
            .collect();
        SduiTree {
            ui_version: 1,
            root_id: root,
            nodes: vec![
                SduiNode::new(
                    root,
                    SduiNodeKind::Flex {
                        direction: SduiFlexDirection::Row,
                        children: vec![panel],
                    },
                ),
                SduiNode::new(
                    panel,
                    SduiNodeKind::Panel {
                        title: "Workspace".to_string(),
                        children: vec![list],
                    },
                ),
                SduiNode::new(list, SduiNodeKind::List { items }),
            ],
        }
    }

    #[test]
    fn file_browser_scroll_reveals_later_rows_without_relisting() {
        // A bounded snapshot with more rows than the viewport height. Scrolling
        // is client-local paint/action math: it never relists directories,
        // calls the server, runs JS, or enqueues workspace actions.
        let size = Size::new(900.0, 120.0);
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(browser_tree_with_rows(30));
        state.rebuild_action_regions_for_test(size);

        assert!(state.scroll_offset() == 0.0);
        assert!(state.content_height > state.viewport_height);

        // Positive delta scrolls down (reveals later rows): offset increases.
        assert!(state.scroll_vertical_pixels(size, 72.0));
        assert_eq!(state.scroll_offset(), 72.0);
        // Scrolling back up decreases the offset.
        assert!(state.scroll_vertical_pixels(size, -30.0));
        assert_eq!(state.scroll_offset(), 42.0);
        // Scrolling far past the bottom clamps to the max scroll, then stays.
        let max_scroll = (state.content_height - state.viewport_height).max(0.0);
        assert!(state.scroll_vertical_pixels(size, 100_000.0));
        assert_eq!(state.scroll_offset(), max_scroll);
        assert!(!state.scroll_vertical_pixels(size, 100_000.0));
    }

    #[test]
    fn file_browser_scrolled_action_hits_visible_row() {
        // After scrolling, clicking a screen position must activate the row
        // currently under the pointer, not the row that occupied that pixel
        // before scrolling.
        let size = Size::new(900.0, 120.0);
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(browser_tree_with_rows(30));
        state.rebuild_action_regions_for_test(size);

        let sidebar = sdui_panel_left_slot_rect(size, &state).expect("left file-browser panel");
        let row_height = state.body_metrics().row_height;
        let click_point = Point::new(
            sidebar.x0 + sdui_theme_style().panel_padding + 10.0 + 4.0,
            sidebar.y0 + sdui_theme_style().panel_padding + row_height + 4.0,
        );

        let before = state
            .action_for_point(click_point)
            .expect("first visible row");
        let SduiActionSource::ListItem { item_id, .. } = &before.source else {
            panic!("expected list-item action source");
        };
        assert_eq!(item_id, "item-0");

        // Scroll down ~2 rows. The pixel that showed item-0 now shows item-2.
        let row_pitch = state
            .body_metrics()
            .list_height(state.text_metrics(FontRole::Ui, UiTextVariant::Detail));
        assert!(state.scroll_vertical_pixels(size, row_pitch * 2.0));
        state.rebuild_action_regions_for_test(size);

        let after = state
            .action_for_point(click_point)
            .expect("scrolled-in row");
        let SduiActionSource::ListItem { item_id, .. } = &after.source else {
            panic!("expected list-item action source after scroll");
        };
        assert_eq!(item_id, "item-2");
    }

    #[test]
    fn scrolls_point_routes_scroll_to_file_browser_only_inside_left_pane() {
        // Scroll routing boundary: only points inside the left file-browser
        // panel route to the SDUI scroll path; everything else keeps the
        // existing editor scroll behavior.
        let size = Size::new(900.0, 600.0);
        let mut state = SduiNativeState::empty();
        state.apply_snapshot(browser_tree_with_rows(3));

        let sidebar = sdui_panel_left_slot_rect(size, &state).expect("left file-browser panel");
        assert!(state.scrolls_point(size, Point::new(sidebar.x0 + 8.0, sidebar.y0 + 8.0)));
        // Just to the right of the file browser belongs to the editor.
        assert!(!state.scrolls_point(size, Point::new(sidebar.x1 + 8.0, sidebar.y0 + 8.0)));
        assert!(!state.scrolls_point(size, Point::new(size.width - 8.0, size.height - 8.0)));
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
    fn sdui_accessibility_role_is_group_container() {
        let state = SduiNativeState::empty();

        assert_eq!(state.accessibility_role(), Role::Group);
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
            label: Some("Editor view for document 7".to_string()),
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

    #[test]
    fn active_menu_appears_in_overlay_observation() {
        use crate::shell::transient_menu::{TransientMenuAction, TransientMenuItem};

        let mut state = SduiNativeState::empty();
        let menu = TransientMenuSession::new(TransientMenuSessionId(3), "Control Center")
            .with_items(vec![
                TransientMenuItem::new("a", "Alpha", TransientMenuAction::new("clay.alpha")),
                TransientMenuItem::new("b", "Beta", TransientMenuAction::new("clay.beta")),
            ]);
        state.set_active_menu(menu);

        let overlays = state
            .observable_snapshot(Size::new(900.0, 600.0))
            .package_transient_overlays;
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].id, "clay.menu.3");
        assert_eq!(overlays[0].anchor, PackageOverlayAnchor::Bottom);
        assert_eq!(overlays[0].component_kind, "stack");
    }

    #[test]
    fn active_menu_exposes_menu_role_and_item_accessibility_labels() {
        use crate::shell::transient_menu::{TransientMenuAction, TransientMenuItem};

        let mut state = SduiNativeState::empty();
        let menu = TransientMenuSession::new(TransientMenuSessionId(9), "Conflict recovery")
            .with_items(vec![
                TransientMenuItem::new(
                    "reload",
                    "Reload",
                    TransientMenuAction::new("clay.documents.serverReloadDocument"),
                )
                .with_accessibility_label("Reload from disk"),
                TransientMenuItem::new(
                    "keep",
                    "Keep editing",
                    TransientMenuAction::new("clay.documents.dismissConflict"),
                )
                .with_accessibility_label("Keep dirty buffer"),
            ]);
        state.set_active_menu(menu);

        let entries = state.accessibility_entries(Size::new(900.0, 600.0));
        assert!(entries.iter().any(|entry| {
            entry.role == Role::Menu && entry.label.as_deref() == Some("Conflict recovery")
        }));
        assert!(entries.iter().any(|entry| {
            entry.role == Role::MenuItem
                && entry.label.as_deref() == Some("Reload from disk selected")
        }));
        assert!(entries.iter().any(|entry| {
            entry.role == Role::MenuItem && entry.label.as_deref() == Some("Keep dirty buffer")
        }));
    }

    #[test]
    fn cancelled_menu_does_not_appear_in_overlay_observation() {
        use crate::shell::transient_menu::{TransientMenuAction, TransientMenuItem};

        let mut state = SduiNativeState::empty();
        let mut menu = TransientMenuSession::new(TransientMenuSessionId(4), "Control Center")
            .with_items(vec![TransientMenuItem::new(
                "a",
                "Alpha",
                TransientMenuAction::new("clay.alpha"),
            )]);
        menu.cancel();
        state.set_active_menu(menu);

        let overlays = state
            .observable_snapshot(Size::new(900.0, 600.0))
            .package_transient_overlays;
        assert!(overlays.is_empty());
    }

    #[test]
    fn menu_overlay_does_not_change_editor_region() {
        use crate::shell::transient_menu::{TransientMenuAction, TransientMenuItem};

        let mut state = SduiNativeState::empty();
        let menu = TransientMenuSession::new(TransientMenuSessionId(5), "Control Center")
            .with_items(vec![TransientMenuItem::new(
                "a",
                "Alpha",
                TransientMenuAction::new("clay.alpha"),
            )]);
        state.set_active_menu(menu);

        let region_with_menu = editor_region(Size::new(900.0, 600.0), &state);
        let region_without_menu = editor_region(Size::new(900.0, 600.0), &SduiNativeState::empty());
        assert_eq!(region_with_menu, region_without_menu);
        assert_eq!(region_with_menu, Rect::new(0.0, 0.0, 900.0, 600.0));
    }

    #[test]
    fn menu_navigation_updates_selection() {
        use crate::shell::transient_menu::TransientMenuAction;

        let mut state = SduiNativeState::empty();
        let menu =
            TransientMenuSession::new(TransientMenuSessionId(6), "Commands").with_items(vec![
                TransientMenuItem::new("a", "Alpha", TransientMenuAction::new("clay.alpha")),
                TransientMenuItem::new("b", "Beta", TransientMenuAction::new("clay.beta")),
                TransientMenuItem::new("c", "Gamma", TransientMenuAction::new("clay.gamma")),
            ]);
        state.set_active_menu(menu);
        assert_eq!(state.active_menu().unwrap().selected_index(), 0);

        state.menu_select_next();
        assert_eq!(state.active_menu().unwrap().selected_index(), 1);

        state.menu_select_previous();
        assert_eq!(state.active_menu().unwrap().selected_index(), 0);

        state.menu_select_previous();
        assert_eq!(state.active_menu().unwrap().selected_index(), 2);
    }

    #[test]
    fn menu_activate_selected_returns_inert_action_intent() {
        use crate::shell::transient_menu::TransientMenuAction;

        let mut state = SduiNativeState::empty();
        let menu =
            TransientMenuSession::new(TransientMenuSessionId(7), "Commands").with_items(vec![
                TransientMenuItem::new("a", "Alpha", TransientMenuAction::new("clay.alpha")),
                TransientMenuItem::new("b", "Beta", TransientMenuAction::new("clay.beta")),
            ]);
        state.set_active_menu(menu);
        state.menu_select_next();

        let intent = state
            .menu_activate_selected()
            .expect("selected item action");
        assert_eq!(intent.command_id, "clay.beta");
    }
}
