#![allow(
    dead_code,
    reason = "SduiNativeState retains a test/agent observability surface (observable_snapshot, package_ui introspection, a11y walker) that is not wired into production rendering after plan 070 step 14 retired the god-object paint path; the live tree flows through the hosted SduiRegionWidget/PackageRegionWidget children. This is test infrastructure, not a migration staging block — full removal/gating is future cleanup"
)]
#![allow(
    clippy::too_many_arguments,
    reason = "Masonry paint/layout helpers pass explicit render context and geometry instead of hiding hot-path state in heap structs"
)]

use std::collections::{BTreeMap, BTreeSet};

use masonry::core::{BrushIndex, PaintCtx, render_text};
use masonry::kurbo::{Affine, Point, Rect, Size};
use masonry::parley::style::{LineHeight, StyleProperty};
use masonry::peniko::Color;
use masonry::vello::Scene;

use crate::{
    editor::typography::{TypographyRegistry, UiTextMetrics, UiTextVariant},
    perf::metrics::global_recorder,
    protocol::{
        DocumentId, FontRole, SduiActionIntent, SduiActionSource, SduiEditorBinding, SduiNode,
        SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation, SduiTreeUpdate, SduiVersion,
    },
    shell::{
        CompletionMenuAcceptAction, FixedSlotId, FixedSlotState, PackageUiComponentTree,
        PackageUiOverlayObservation, PackageUiPanelObservation, PackageUiRuntimeError,
        PackageUiRuntimeState, PackageUiRuntimeUpdate, PaneSlotLayout, TransientMenuSession,
        layout::PaneSlotId,
        theme::{PanelDefaults, SduiThemeStyle},
    },
};

#[cfg(test)]
use crate::shell::{
    FixedPackagePanel, PackageOverlayAnchor, PackagePanelVisibility, TransientPackageOverlay,
};

#[cfg(test)]
const SIDEBAR_WIDTH: f64 = 240.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SduiObservableListItem {
    pub id: String,
    pub label: String,
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
    package_ui: PackageUiRuntimeState,
    typography: TypographyRegistry,
    // Phase 20.1: cached resolved UI design-token registry layered over the
    // core fallback catalog. Installed atomically with the editor
    // `StyleRegistry` when the active theme changes; paint reads cached typed
    // values via the accessors without parsing strings or allocating maps.
    ui_theme: crate::shell::theme::ResolvedUiTheme,
    active_menu: Option<TransientMenuSession>,
    /// Plan 070 step 8: set whenever the SDUI tree/theme/typography changes so
    /// the host (`EditorWidget::sync_region`) rebuilds the reconciled
    /// `SduiRegionWidget` child on the next event-loop edit. Replaces the
    /// deleted nested `RetainedSdui` compositor dirty flag.
    region_dirty: bool,
    /// Plan 070 step 13b: set whenever package_ui/theme/typography changes so the
    /// host (`EditorWidget::sync_panels`) reconciles the retained fixed-panel
    /// children on the next event-loop edit.
    panels_dirty: bool,
    /// Plan 070 step 13e: set whenever the transient overlays (package overlays
    /// or the active menu) or the render context changes so the host
    /// (`EditorWidget::sync_overlays`) reconciles the retained overlay children
    /// on the next event-loop edit.
    overlays_dirty: bool,
}

impl SduiNativeState {
    pub fn empty() -> Self {
        Self {
            ui_version: 0,
            root_id: None,
            nodes: BTreeMap::new(),
            editor_binding: None,
            package_ui: PackageUiRuntimeState::new(),
            typography: TypographyRegistry::default(),
            ui_theme: crate::shell::theme::ResolvedUiTheme::default(),
            active_menu: None,
            region_dirty: true,
            panels_dirty: true,
            overlays_dirty: true,
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
        self.mark_region_dirty();
        self.panels_dirty = true;
        self.overlays_dirty = true;
    }

    /// Install a resolved UI design-token registry built from the active
    /// theme's validated overrides. Called atomically with the editor
    /// `StyleRegistry` install; cached for paint/layout reads. Invalid overrides
    /// (a malformed snapshot) fall back to core fallbacks rather than crash.
    pub(crate) fn set_ui_theme(&mut self, ui_theme: crate::shell::theme::ResolvedUiTheme) {
        self.ui_theme = ui_theme;
        self.mark_region_dirty();
        self.panels_dirty = true;
        self.overlays_dirty = true;
    }

    /// Resolve the SDUI paint style from the active `ui_theme` so package
    /// component fills, typography variants, and the spacing rhythm honor the
    /// user theme (Phase 20.4). Reads cached resolved token values only.
    fn theme_style(&self) -> SduiThemeStyle {
        SduiThemeStyle::from_ui_theme(&self.ui_theme)
    }

    /// Return and clear the panels-dirty flag (plan 070 step 13b). The host
    /// calls this from the event loop to decide whether to reconcile the
    /// retained fixed-panel children.
    pub(crate) fn take_panels_dirty(&mut self) -> bool {
        std::mem::take(&mut self.panels_dirty)
    }

    pub(crate) fn take_overlays_dirty(&mut self) -> bool {
        std::mem::take(&mut self.overlays_dirty)
    }

    /// Snapshot the transient overlays + render context for the overlay host to
    /// reconcile its retained overlay children (plan 070 step 13e).
    pub(crate) fn overlays_render_input(
        &self,
    ) -> (
        Vec<crate::shell::TransientPackageOverlay>,
        TypographyRegistry,
        crate::shell::theme::ResolvedUiTheme,
    ) {
        (
            self.transient_overlays(),
            self.typography.clone(),
            self.ui_theme.clone(),
        )
    }

    /// The package_ui runtime state the panel host reconciles against.
    pub(crate) fn package_ui(&self) -> &PackageUiRuntimeState {
        &self.package_ui
    }

    /// Snapshot the package_ui state + render context for the panel host to
    /// reconcile its retained fixed-panel children (plan 070 step 13b).
    pub(crate) fn panels_render_input(
        &self,
    ) -> (
        PackageUiRuntimeState,
        TypographyRegistry,
        crate::shell::theme::ResolvedUiTheme,
    ) {
        (
            self.package_ui.clone(),
            self.typography.clone(),
            self.ui_theme.clone(),
        )
    }

    fn text_metrics(&self, role: FontRole, variant: UiTextVariant) -> UiTextMetrics {
        self.typography.ui_text_metrics(role, variant)
    }

    fn body_metrics(&self) -> UiTextMetrics {
        self.text_metrics(FontRole::Ui, self.theme_style().body_text)
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
        self.overlays_dirty = true;
    }

    pub(crate) fn clear_active_menu(&mut self) {
        self.active_menu = None;
        self.overlays_dirty = true;
    }

    pub(crate) fn menu_select_next(&mut self) {
        if let Some(menu) = &mut self.active_menu {
            menu.select_next();
            self.overlays_dirty = true;
        }
    }

    pub(crate) fn menu_select_previous(&mut self) {
        if let Some(menu) = &mut self.active_menu {
            menu.select_previous();
            self.overlays_dirty = true;
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
            self.overlays_dirty = true;
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
        self.mark_region_dirty();
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
        self.mark_region_dirty();
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
        // package_ui drives both the fixed panels and the transient overlays.
        self.panels_dirty = true;
        self.overlays_dirty = true;
        Ok(())
    }

    pub(crate) fn install_package_ui_snapshot(
        &mut self,
        snapshot: &crate::protocol::PackageUiSnapshot,
    ) {
        self.package_ui.install_runtime_snapshot(snapshot);
        self.panels_dirty = true;
        self.overlays_dirty = true;
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

    /// True when `point` lies inside the Clay-owned left file-browser panel,
    /// i.e. scroll events here should scroll the file browser, not the editor.
    /// The actual scrolling is handled by the reconciled region's `Portal`
    /// scroll viewport (plan 070 step 12); this only distinguishes the sidebar
    /// from the editor for the `EditorWidget` scroll fall-through.
    pub(crate) fn scrolls_point(&self, size: Size, point: Point) -> bool {
        sdui_panel_left_slot_rect(size, self).is_some_and(|rect| rect.contains(point))
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
                .fixed_panel_observations(widget_size.to_rect(), &self.ui_theme.panel_defaults()),
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
            sdui_slot_layout(widget_size, self).compute_geometry(widget_size.to_rect());
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

    /// Compute the sidebar slot geometry for the current size. Called from
    /// `EditorWidget::layout` to place the reconciled region child as a fixed
    /// scroll viewport. Scroll position/content height are owned by the
    /// region's `Portal` (plan 070 step 12), so this only reports the slot rect
    /// and its content padding.
    pub(crate) fn sidebar_geometry(&self, size: Size) -> Option<SduiSidebarGeometry> {
        self.root_id?;
        let sidebar = sdui_panel_left_slot_rect(size, self)?;
        let padding = self.theme_style().panel_padding;
        Some(SduiSidebarGeometry {
            rect: sidebar,
            padding,
        })
    }

    /// Snapshot the reconciled tree + render context for the host to feed the
    /// `SduiRegionWidget` child. `None` when there is no sidebar tree (the
    /// region stays inert).
    pub(crate) fn region_render_input(&self) -> Option<SduiRenderInput> {
        let root_id = self.root_id?;
        Some(SduiRenderInput {
            tree: self.current_tree(root_id),
            typography: self.typography.clone(),
            ui_theme: self.ui_theme.clone(),
        })
    }

    /// Snapshot the current protocol tree for the reconciled region child.
    fn current_tree(&self, root_id: SduiNodeId) -> SduiTree {
        SduiTree {
            ui_version: self.ui_version,
            root_id,
            nodes: self.nodes.values().cloned().collect(),
        }
    }

    fn mark_region_dirty(&mut self) {
        self.region_dirty = true;
    }

    /// Return and clear the region-dirty flag (plan 070 step 8). The host calls
    /// this from the event loop to decide whether to rebuild the region child.
    pub(crate) fn take_region_dirty(&mut self) -> bool {
        std::mem::take(&mut self.region_dirty)
    }

    fn rebuild_derived_state(&mut self) {
        self.editor_binding = None;
        if let Some(root_id) = self.root_id {
            self.find_editor_binding(root_id);
        }
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

    /// Transient overlays in stacking order (`z.overlay` < `z.modal` <
    /// `z.tooltip`): the package transient overlays plus the active menu
    /// projected as an overlay. Shared by the retained overlay host (step 13e)
    /// and the legacy immediate-mode paint walk below.
    pub(crate) fn transient_overlays(&self) -> Vec<crate::shell::TransientPackageOverlay> {
        let mut overlays: Vec<crate::shell::TransientPackageOverlay> =
            self.package_ui.overlays().cloned().collect();
        if let Some(menu) = &self.active_menu
            && menu.is_active()
        {
            overlays.push(crate::shell::TransientPackageOverlay::from_menu_session(
                menu,
            ));
        }
        overlays.sort_by_key(|o| overlay_z_order(o.z_level_token));
        overlays
    }
}

/// Shared SDUI text paint used by the package-component paint path (via
/// `paint_text`) and the retained `SduiLegacyLeaf` reconciliation widget (plan
/// 070 task 6.5), so both render glyphs through one code path.
pub(crate) fn paint_sdui_text(
    typography: &TypographyRegistry,
    panel_padding: f64,
    ctx: &mut PaintCtx<'_>,
    scene: &mut Scene,
    text: &str,
    depth: usize,
    y: f64,
    width: f64,
    origin_x: f64,
    role: FontRole,
    metrics: UiTextMetrics,
    color: Color,
) {
    let max_width = (width - panel_padding * 2.0 - depth as f64 * 10.0).max(1.0) as f32;
    let (font_context, layout_context) = ctx.text_contexts();
    let mut builder = layout_context.ranged_builder(font_context, text, 1.0, true);
    builder.push_default(StyleProperty::FontStack(
        typography.profile(role).font_stack(),
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
        Affine::translate((origin_x + panel_padding + depth as f64 * 10.0, y)),
        &layout,
        &[color.into()],
        true,
    );
}

/// Shared SDUI row-rect geometry (see [`SduiNativeState::row_rect`]).
pub(crate) fn sdui_row_rect(
    panel_padding: f64,
    depth: usize,
    y: f64,
    width: f64,
    origin_x: f64,
    height: f64,
) -> Rect {
    let x0 = origin_x + panel_padding + depth as f64 * 10.0;
    Rect::new(
        x0,
        y,
        (origin_x + width - panel_padding).max(x0),
        y + height,
    )
}

impl Default for SduiNativeState {
    fn default() -> Self {
        Self::empty()
    }
}

/// Sidebar slot geometry computed by [`SduiNativeState::sidebar_geometry`] for
/// placing + clipping the reconciled region child (plan 070 step 8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SduiSidebarGeometry {
    pub(crate) rect: Rect,
    pub(crate) padding: f64,
}

/// Reconciled tree + render context fed to the `SduiRegionWidget` child by the
/// host (plan 070 step 8).
#[derive(Clone, Debug)]
pub(crate) struct SduiRenderInput {
    pub(crate) tree: SduiTree,
    pub(crate) typography: TypographyRegistry,
    pub(crate) ui_theme: crate::shell::theme::ResolvedUiTheme,
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
    let defaults = sdui.ui_theme.panel_defaults();
    let full_rect = size.to_rect();
    let layout = sdui.package_ui.slot_layout(&defaults);
    let want_left = (sdui.root_id.is_some() || sdui.editor_binding().is_some())
        && size.width > defaults.sidebar_width + 100.0;
    with_default_left_slot(layout, &defaults, want_left)
        .compute_geometry(full_rect)
        .main_rect
}

/// Single mechanical source for adding the default Clay-owned left slot.
///
/// Task 6 (plan 070): the three slot-layout entry points previously each
/// open-coded the same `contains_slot` + `with_fixed_slot(fixed_sdui_left_slot)`
/// block. They keep their distinct *gates* (which differ intentionally: the
/// editor main region reserves on root-or-binding plus a width guard, the panel
/// sidebar on root only) but share this one application site so the default-left
/// construction cannot drift between them.
fn with_default_left_slot(
    layout: PaneSlotLayout,
    defaults: &PanelDefaults,
    want_left: bool,
) -> PaneSlotLayout {
    if want_left && !layout.contains_slot(PaneSlotId::Left) {
        layout.with_fixed_slot(fixed_sdui_left_slot(defaults))
    } else {
        layout
    }
}

fn sdui_slot_layout(size: Size, sdui: &SduiNativeState) -> PaneSlotLayout {
    let defaults = sdui.ui_theme.panel_defaults();
    let layout = sdui.package_ui.slot_layout(&defaults);
    let want_left = sdui.editor_binding().is_some() && size.width > defaults.sidebar_width + 100.0;
    with_default_left_slot(layout, &defaults, want_left)
}

fn sdui_panel_slot_layout(sdui: &SduiNativeState) -> PaneSlotLayout {
    let defaults = sdui.ui_theme.panel_defaults();
    let layout = sdui.package_ui.slot_layout(&defaults);
    with_default_left_slot(layout, &defaults, sdui.root_id.is_some())
}

fn fixed_sdui_left_slot(defaults: &PanelDefaults) -> FixedSlotState {
    FixedSlotState {
        slot_id: FixedSlotId::Left,
        size: defaults.sidebar_width,
        min_size: defaults.sidebar_width,
        max_size: defaults.sidebar_width,
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

pub(crate) fn package_action_intent(command_id: &str, source_id: &str) -> SduiActionIntent {
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

/// Stacking order for transient overlays: `z.overlay` (0) < `z.modal` (1) <
/// `z.tooltip` (2). Unknown tokens sort as `z.overlay`. Shared by the retained
/// overlay host (step 13e) and the legacy overlay paint walk.
/// The kind name for an SDUI node (test/observability surface).
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

pub(crate) fn overlay_z_order(token: &str) -> u8 {
    match token {
        "z.modal" => 1,
        "z.tooltip" => 2,
        _ => 0,
    }
}

pub(crate) fn stable_package_source_id(source_id: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in source_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash.max(1)
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
                    "z.overlay",
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
                    "z.overlay",
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
    fn sdui_renderer_uses_resolved_theme_tokens_for_panel_styles() {
        // Phase 20.4: SDUI paint resolves from the active ResolvedUiTheme, not
        // the core-fallback-only resolver. Default theme (no overrides) resolves
        // core tokens; panel_padding is on the 4pt spacing.md rhythm (16) and
        // fills come from core surface tokens.
        let state = SduiNativeState::empty();
        let style = state.theme_style();

        assert_eq!(style.panel_padding, 16.0);
        assert_eq!(style.title_text, UiTextVariant::Title);
        assert_eq!(style.body_text, UiTextVariant::Body);
        assert_eq!(style.status_text, UiTextVariant::Status);
        assert_eq!(style.panel_background, Color::from_rgb8(0x21, 0x20, 0x2b));
        assert_eq!(style.button_background, Color::from_rgb8(0x39, 0x35, 0x4a));
        assert_eq!(style.list_background, Color::from_rgb8(0x29, 0x28, 0x35));
    }

    #[test]
    fn sdui_paint_uses_active_theme_not_core_fallbacks() {
        // Phase 20.4: installing a ResolvedUiTheme with design-token overrides
        // must change what SDUI paint reads. The core-fallback path
        // (SduiThemeStyle::default / from_resolver) is no longer in the paint
        // path, so user/theme-package overrides reach component fills.
        use crate::protocol::{UiDesignTokenOverride, WireDesignTokenValue};
        use crate::shell::theme::ResolvedUiTheme;

        let overrides = vec![
            UiDesignTokenOverride {
                token: "surface.panel".to_string(),
                value: WireDesignTokenValue::Color([0x11, 0x22, 0x33, 0xff]),
                provenance: "test".to_string(),
            },
            UiDesignTokenOverride {
                token: "surface.control".to_string(),
                value: WireDesignTokenValue::Color([0x44, 0x55, 0x66, 0xff]),
                provenance: "test".to_string(),
            },
            UiDesignTokenOverride {
                token: "spacing.md".to_string(),
                value: WireDesignTokenValue::Scalar(20.0),
                provenance: "test".to_string(),
            },
        ];
        let mut state = SduiNativeState::empty();
        state
            .set_ui_theme(ResolvedUiTheme::from_active_theme(&overrides).expect("valid overrides"));

        let style = state.theme_style();
        assert_eq!(style.panel_background, Color::from_rgb8(0x11, 0x22, 0x33));
        assert_eq!(style.button_background, Color::from_rgb8(0x44, 0x55, 0x66));
        // spacing.md (20) scaled by default density (1.0).
        assert_eq!(style.panel_padding, 20.0);
    }

    #[test]
    fn sdui_spacing_rhythm_scales_with_density() {
        // Phase 20.4: panel_padding is spacing.md x spacing_scale(); density
        // scales the UI spacing rhythm only. compact=0.875, default=1.0,
        // spacious=1.125.
        use crate::protocol::{UiDesignTokenOverride, WireDesignTokenValue};
        use crate::shell::theme::ResolvedUiTheme;

        let compact = UiDesignTokenOverride {
            token: "density.default".to_string(),
            value: WireDesignTokenValue::Level("compact".to_string()),
            provenance: "test".to_string(),
        };
        let spacious = UiDesignTokenOverride {
            token: "density.default".to_string(),
            value: WireDesignTokenValue::Level("spacious".to_string()),
            provenance: "test".to_string(),
        };

        let mut state = SduiNativeState::empty();
        // core spacing.md = 16.
        assert_eq!(state.theme_style().panel_padding, 16.0);

        state
            .set_ui_theme(ResolvedUiTheme::from_active_theme(&[compact]).expect("compact density"));
        assert_eq!(state.theme_style().panel_padding, 16.0 * 0.875);

        let mut state = SduiNativeState::empty();
        state.set_ui_theme(
            ResolvedUiTheme::from_active_theme(&[spacious]).expect("spacious density"),
        );
        assert_eq!(state.theme_style().panel_padding, 16.0 * 1.125);
    }

    /// Plan 065 task 7: per-component-per-state structural observability
    /// palette. Captures the resolved fill/border/text colors the SDUI paint
    /// path derives for a component kind in a given InteractionState from the
    /// active ResolvedUiTheme — no pixel rendering. Containers
    /// (flex/stack/scroll/portal) and `editorView` carry no SDUI state-driven
    /// chrome (editorView chrome is editor-theme-driven; see task 5).
    #[derive(Debug, Clone, PartialEq)]
    struct ComponentStatePalette {
        fill: Option<Color>,
        border: Option<Color>,
        text: Option<Color>,
    }

    fn component_state_palette(
        theme: &crate::shell::theme::ResolvedUiTheme,
        kind: &str,
        state: crate::shell::primitives::InteractionState,
    ) -> ComponentStatePalette {
        use crate::shell::primitives::InteractionState;
        use crate::shell::primitives::{
            component_state_color, disabled_text_color, list_row_fill_color,
        };
        let primary = || theme.color("text.primary").unwrap_or(Color::TRANSPARENT);
        let muted = || theme.color("text.muted").unwrap_or(Color::TRANSPARENT);
        let text_for = |default_muted: bool| {
            if state == InteractionState::Disabled {
                disabled_text_color(theme)
            } else if default_muted {
                muted()
            } else {
                primary()
            }
        };
        match kind {
            "button" => ComponentStatePalette {
                fill: Some(component_state_color(theme, "surface.control", state)),
                border: (state == InteractionState::Focus)
                    .then(|| theme.color("border.focus").unwrap_or(Color::TRANSPARENT)),
                text: Some(text_for(false)),
            },
            "list" => ComponentStatePalette {
                fill: Some(list_row_fill_color(theme, state, false)),
                border: None,
                text: Some(text_for(false)),
            },
            "label" | "statusItem" => ComponentStatePalette {
                fill: None,
                border: None,
                text: Some(text_for(true)),
            },
            "panel" => ComponentStatePalette {
                fill: theme.color("surface.panel"),
                border: theme.color("border.subtle"),
                text: theme.color("text.primary"),
            },
            "overlay" => ComponentStatePalette {
                fill: theme.color("surface.overlay"),
                border: theme.color("border.subtle"),
                text: theme.color("text.primary"),
            },
            // Containers recurse children; editorView chrome is editor-theme.
            "flex" | "stack" | "scroll" | "portal" | "editorView" => ComponentStatePalette {
                fill: None,
                border: None,
                text: None,
            },
            // Phase 20.5: dropdown trigger — same state model as button.
            "dropdown" => ComponentStatePalette {
                fill: Some(component_state_color(theme, "surface.control", state)),
                border: (state == InteractionState::Focus)
                    .then(|| theme.color("border.focus").unwrap_or(Color::TRANSPARENT)),
                text: Some(text_for(false)),
            },
            // Phase 20.5: collapse — title text, no fill of its own.
            "collapse" => ComponentStatePalette {
                fill: None,
                border: None,
                text: Some(text_for(false)),
            },
            // Phase 20.5: modal — overlay-surface chrome.
            "modal" => ComponentStatePalette {
                fill: theme.color("surface.overlay"),
                border: theme.color("border.subtle"),
                text: theme.color("text.primary"),
            },
            // Phase 20.5: text input — control fill, validation-state border.
            "textInput" => ComponentStatePalette {
                fill: Some(component_state_color(theme, "surface.control", state)),
                border: (state == InteractionState::Focus)
                    .then(|| theme.color("border.focus").unwrap_or(Color::TRANSPARENT))
                    .or_else(|| theme.color("border.subtle")),
                text: Some(text_for(true)),
            },
            _ => ComponentStatePalette {
                fill: None,
                border: None,
                text: None,
            },
        }
    }

    #[test]
    fn each_component_kind_renders_all_five_states() {
        // Plan 065 task 7 + Phase 20.5: 14 kinds × 5 states snapshot matrix.
        // Pins the resolved palette per kind per state against the core token
        // values so regressions in token routing or state mapping fail
        // deterministically.
        use crate::shell::primitives::InteractionState;
        use crate::shell::theme::ResolvedUiTheme;
        let theme = ResolvedUiTheme::from_active_theme(&[]).unwrap();
        let kinds = [
            "editorView",
            "panel",
            "label",
            "button",
            "list",
            "flex",
            "stack",
            "overlay",
            "scroll",
            "portal",
            "statusItem",
            // Phase 20.5
            "dropdown",
            "collapse",
            "modal",
            "textInput",
        ];
        let states = [
            InteractionState::Rest,
            InteractionState::Hover,
            InteractionState::Active,
            InteractionState::Focus,
            InteractionState::Disabled,
        ];
        for kind in kinds {
            for state in states {
                let palette = component_state_palette(&theme, kind, state);
                // Every kind must produce a palette for every state (no panic,
                // no None where a fill is expected).
                match kind {
                    "button" | "dropdown" | "textInput" => {
                        assert!(palette.fill.is_some(), "{kind} needs a fill for {state:?}")
                    }
                    "list" => assert!(palette.fill.is_some(), "list needs a fill for {state:?}"),
                    "panel" | "overlay" | "modal" => {
                        assert!(
                            palette.fill.is_some(),
                            "{kind} needs a chrome fill for {state:?}"
                        );
                        assert!(
                            palette.border.is_some(),
                            "{kind} needs a chrome border for {state:?}"
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn component_state_colors_are_token_derived() {
        // Plan 065 task 7: the per-state palette matches the exact core token
        // values (state mapping + token values pinned), not literals from paint.
        use crate::shell::primitives::InteractionState;
        use crate::shell::theme::ResolvedUiTheme;
        let theme = ResolvedUiTheme::from_active_theme(&[]).unwrap();

        // Button fill across all five states.
        assert_eq!(
            component_state_palette(&theme, "button", InteractionState::Rest).fill,
            Some(Color::from_rgb8(0x39, 0x35, 0x4a))
        );
        assert_eq!(
            component_state_palette(&theme, "button", InteractionState::Hover).fill,
            Some(Color::from_rgb8(0x2d, 0x2b, 0x3d))
        );
        assert_eq!(
            component_state_palette(&theme, "button", InteractionState::Active).fill,
            Some(Color::from_rgb8(0x34, 0x31, 0x47))
        );
        assert_eq!(
            component_state_palette(&theme, "button", InteractionState::Focus).fill,
            Some(Color::from_rgb8(0x7c, 0x6f, 0xff))
        );
        assert_eq!(
            component_state_palette(&theme, "button", InteractionState::Focus).border,
            Some(Color::from_rgb8(0x7c, 0x6f, 0xff))
        );
        assert_eq!(
            component_state_palette(&theme, "button", InteractionState::Disabled).fill,
            Some(Color::from_rgba8(0x1b, 0x1a, 0x24, 140))
        );
        assert_eq!(
            component_state_palette(&theme, "button", InteractionState::Disabled).text,
            Some(Color::from_rgba8(0x6f, 0x6a, 0x87, 140))
        );

        // List row fill: Rest unselected → surface.list; selected-style states.
        assert_eq!(
            component_state_palette(&theme, "list", InteractionState::Rest).fill,
            Some(Color::from_rgb8(0x29, 0x28, 0x35))
        );
        assert_eq!(
            component_state_palette(&theme, "list", InteractionState::Hover).fill,
            Some(Color::from_rgb8(0x2d, 0x2b, 0x3d))
        );
        assert_eq!(
            component_state_palette(&theme, "list", InteractionState::Active).fill,
            Some(Color::from_rgb8(0x34, 0x31, 0x47))
        );

        // Label/statusItem: no fill, muted text at Rest, disabled text when Disabled.
        assert_eq!(
            component_state_palette(&theme, "label", InteractionState::Rest).fill,
            None
        );
        assert_eq!(
            component_state_palette(&theme, "label", InteractionState::Rest).text,
            Some(Color::from_rgb8(0xb9, 0xb2, 0xcf))
        );
        assert_eq!(
            component_state_palette(&theme, "statusItem", InteractionState::Disabled).text,
            Some(Color::from_rgba8(0x6f, 0x6a, 0x87, 140))
        );

        // Panel/overlay chrome: token-driven fill + border, state-independent.
        assert_eq!(
            component_state_palette(&theme, "panel", InteractionState::Rest).fill,
            Some(Color::from_rgb8(0x21, 0x20, 0x2b))
        );
        assert_eq!(
            component_state_palette(&theme, "panel", InteractionState::Rest).border,
            Some(Color::from_rgb8(0x2f, 0x2c, 0x40))
        );
        assert_eq!(
            component_state_palette(&theme, "overlay", InteractionState::Rest).fill,
            Some(Color::from_rgb8(0x18, 0x17, 0x20))
        );

        // Containers + editorView: no SDUI state-driven chrome.
        for kind in ["flex", "stack", "scroll", "portal", "editorView"] {
            let palette = component_state_palette(&theme, kind, InteractionState::Hover);
            assert_eq!(
                palette,
                ComponentStatePalette {
                    fill: None,
                    border: None,
                    text: None
                },
                "{kind} has no SDUI state chrome"
            );
        }
    }

    /// Phase 20.7 task 5: ground-truth tie between `applicable_states` (Plan
    /// 068 task 4) and the `component_state_palette` paint path. For each
    /// `ComponentKind` category, asserts the applicable-state set matches the
    /// documented per-kind notes AND the palette renders token-driven output
    /// (Some fill or text) for every applicable state of interactive kinds.
    /// Catches drift between the `applicable_states` table and the paint path
    /// — the ground truth the task-4 `ponytail:` note deferred to this matrix.
    #[test]
    fn applicable_states_match_component_state_palette() {
        use crate::shell::components::{ComponentKind as K, applicable_states};
        use crate::shell::primitives::InteractionState;
        use crate::shell::theme::ResolvedUiTheme;
        use InteractionState as S;
        let theme = ResolvedUiTheme::from_active_theme(&[]).unwrap();
        let all_five = [S::Rest, S::Hover, S::Active, S::Focus, S::Disabled];

        // Interactive triggers: all five states applicable; palette renders a
        // fill (button/list/dropdown/textInput) or text (collapse) for every
        // state — state-complete from tokens.
        for kind in [K::Button, K::List, K::Dropdown, K::Collapse, K::TextInput] {
            assert_eq!(
                applicable_states(kind),
                all_five,
                "{kind:?} applicable = all five"
            );
            for state in all_five {
                let p = component_state_palette(&theme, kind.as_str(), state);
                assert!(
                    p.fill.is_some() || p.text.is_some(),
                    "{kind:?} must render a fill or text for {state:?}"
                );
            }
        }

        // Chrome containers: Rest only; palette renders a chrome fill+border.
        for kind in [K::Panel, K::Overlay, K::Modal] {
            assert_eq!(
                applicable_states(kind),
                [S::Rest],
                "{kind:?} applicable = Rest"
            );
            let p = component_state_palette(&theme, kind.as_str(), S::Rest);
            assert!(
                p.fill.is_some() && p.border.is_some(),
                "{kind:?} chrome fill+border"
            );
        }

        // Text-no-fill: Rest/Focus/Disabled; palette text renders, fill None.
        for kind in [K::Label, K::StatusItem] {
            assert_eq!(
                applicable_states(kind),
                [S::Rest, S::Focus, S::Disabled],
                "{kind:?} applicable = Rest/Focus/Disabled"
            );
            for state in [S::Rest, S::Focus, S::Disabled] {
                let p = component_state_palette(&theme, kind.as_str(), state);
                assert!(
                    p.text.is_some() && p.fill.is_none(),
                    "{kind:?} renders text not fill for {state:?}"
                );
            }
        }

        // Scrollbar-bearing: Rest/Hover/Active; no SDUI state-token chrome
        // (scrollbar chrome is editor-side via paint_scroll_chrome).
        for kind in [K::EditorView, K::Scroll] {
            assert_eq!(
                applicable_states(kind),
                [S::Rest, S::Hover, S::Active],
                "{kind:?} applicable = Rest/Hover/Active"
            );
            let p = component_state_palette(&theme, kind.as_str(), S::Hover);
            assert_eq!(
                p,
                ComponentStatePalette {
                    fill: None,
                    border: None,
                    text: None
                },
                "{kind:?} has no SDUI state chrome"
            );
        }

        // Layout containers: Rest only; no chrome of their own.
        for kind in [K::Flex, K::Stack, K::Portal] {
            assert_eq!(
                applicable_states(kind),
                [S::Rest],
                "{kind:?} applicable = Rest"
            );
            let p = component_state_palette(&theme, kind.as_str(), S::Rest);
            assert_eq!(
                p,
                ComponentStatePalette {
                    fill: None,
                    border: None,
                    text: None
                },
                "{kind:?} has no chrome"
            );
        }
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
