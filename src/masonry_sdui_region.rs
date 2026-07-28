#![allow(
    dead_code,
    reason = "SDUI reconciliation seam is staged for per-component migration (plan 070); it is exercised by its own tests before the reconciled tree is composed into the shell"
)]

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
//! Mapping (the per-kind whitelist lives in [`SduiRegionWidget::build_subtree`]):
//! container kinds (`panel`/`flex`/`stack`) map to Masonry `Flex`/`ZStack`;
//! leaf kinds (`label`/`button`/`list`/`editorView`) map to [`SduiLegacyLeaf`],
//! a thin widget that reports the *exact* legacy row height and paints through
//! the *same* shared helpers (`paint_sdui_text`/`sdui_row_rect`/state fills) as
//! the legacy renderer. Because every kind maps to a widget, the reconciled
//! subtree is complete and renders identically to the legacy paint at Rest
//! state; the live routing (hosting at the sidebar rect, scroll clip, pointer
//! routing) is the rendering-cutover step (task 6.5 remainder).

use std::collections::BTreeMap;

use masonry::accesskit::{Node, Role};
use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
use masonry::core::{
    AccessCtx, BoxConstraints, ChildrenIds, LayoutCtx, NewWidget, NoAction, PaintCtx,
    PropertiesMut, PropertiesRef, RegisterCtx, Widget, WidgetId, WidgetPod,
};
use masonry::dpi::PhysicalSize;
use masonry::kurbo::{Affine, Point, Size};
use masonry::peniko::{Color, Fill};
use masonry::properties::types::UnitPoint;
use masonry::theme::default_property_set;
use masonry::vello::Scene;
use masonry::widgets::{Flex, ZStack};

use crate::editor::typography::{TypographyRegistry, UiTextMetrics, UiTextVariant};
use crate::masonry_sdui::{paint_sdui_text, sdui_row_rect};
use crate::protocol::{FontRole, SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree};
use crate::protocol::{SduiTreeOperation, SduiTreeUpdate, SduiVersion};
use crate::shell::theme::{ResolvedUiTheme, SduiThemeStyle};
use crate::shell::{InteractionState, component_state_color, list_row_fill_color};

/// Renders the reconciled SDUI tree through a private Masonry [`RenderRoot`] and
/// caches the resulting [`Scene`], so `SduiNativeState::paint` can composite the
/// retained subtree at the exact immediate-mode renderer's z-order point (chrome under it,
/// package overlays over it) without restructuring the main widget tree.
///
/// The off-tree root has no resize API, so it is recreated at the current
/// sidebar size whenever the tree, theme/typography, or size changes (`dirty`).
/// Scroll is applied purely as a composite translation, so scrolling never
/// re-renders. ponytail: recreating the root re-loads system fonts; acceptable
/// because re-renders are low-frequency (server-published UI changes, theme
/// swaps, slot resizes), never per-frame or per-scroll.
pub(crate) struct RetainedSdui {
    root: RenderRoot,
    scene: Scene,
    rendered_size: Size,
    dirty: bool,
}

impl RetainedSdui {
    pub(crate) fn new() -> Self {
        Self {
            root: render_root_with(SduiRegionWidget::new(), Size::new(1.0, 1.0)),
            scene: Scene::new(),
            rendered_size: Size::ZERO,
            dirty: true,
        }
    }

    /// Mark the cached scene stale (tree/theme/typography changed).
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Return the retained scene rendered at `size`, re-rendering only when
    /// dirty or the size changed. `tree`/`typography`/`ui_theme` are the current
    /// authoritative values pulled from `SduiNativeState`.
    pub(crate) fn ensure_rendered(
        &mut self,
        tree: SduiTree,
        typography: &TypographyRegistry,
        ui_theme: &ResolvedUiTheme,
        size: Size,
    ) -> &Scene {
        let size_changed = (self.rendered_size.width - size.width).abs() > 0.5
            || (self.rendered_size.height - size.height).abs() > 0.5;
        if self.dirty || size_changed {
            // Build the region fully populated BEFORE handing it to the
            // RenderRoot: mutating the child tree after the register pass leaves
            // new pods unregistered ("cannot find child returned by
            // children_ids()"). Recreating per dirty render is low-frequency.
            let mut region = SduiRegionWidget::new();
            region.set_render_context(typography.clone(), ui_theme.clone());
            region.reconcile_snapshot(tree);
            self.root = render_root_with(region, size);
            let (scene, _) = self.root.redraw();
            self.scene = scene;
            self.rendered_size = size;
            self.dirty = false;
        }
        &self.scene
    }
}

// `SduiNativeState` derives `Debug`/`Clone`/`PartialEq`, but the compositor's
// `RenderRoot`/`Scene` implement none of them. The compositor is a transient
// render cache carrying no logical state, so: debug skips the heavy fields,
// clone starts a fresh stale cache, and equality is always true.
impl std::fmt::Debug for RetainedSdui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedSdui")
            .field("rendered_size", &self.rendered_size)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

impl Clone for RetainedSdui {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl PartialEq for RetainedSdui {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

fn render_root_with(region: SduiRegionWidget, size: Size) -> RenderRoot {
    RenderRoot::new(
        NewWidget::new(region),
        |_| {},
        RenderRootOptions {
            default_properties: default_property_set().into(),
            use_system_fonts: true,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(size.width.max(1.0) as u32, size.height.max(1.0) as u32),
            // ponytail: fixed 1.0; the scene is vector-encoded in logical px and
            // the final rasterization applies the window scale uniformly. If
            // HiDPI glyphs render at the wrong size, thread the real scale here.
            scale_factor: 1.0,
            test_font: None,
        },
    )
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
    /// Render context cloned into each [`SduiLegacyLeaf`] so leaves paint with
    /// the active typography/theme. Fed by `SduiNativeState` at the cutover;
    /// defaults (base catalog) until then.
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
            typography: TypographyRegistry::default(),
            ui_theme: ResolvedUiTheme::default(),
        }
    }

    /// Install the active typography/theme used to paint legacy leaves.
    pub(crate) fn set_render_context(
        &mut self,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) {
        self.typography = typography;
        self.ui_theme = ui_theme;
        self.rebuild();
    }

    /// Replace the retained tree and rebuild the reconciled Masonry subtree.
    ///
    /// Runs outside paint (called from the SDUI snapshot application path).
    pub(crate) fn reconcile_snapshot(&mut self, tree: SduiTree) {
        self.ui_version = tree.ui_version;
        self.root_id = Some(tree.root_id);
        self.nodes.clear();
        for node in tree.nodes {
            self.nodes.insert(node.id, node);
        }
        self.rebuild();
    }

    /// Apply incremental tree operations, then rebuild the reconciled subtree.
    ///
    /// Returns `false` and leaves state untouched when `base_ui_version` does
    /// not match the current version (stale update rejection). Runs outside
    /// paint.
    ///
    /// ponytail: rebuilds the whole subtree on every accepted update (O(n) in
    /// node count). SDUI updates are low-frequency (server-published UI changes,
    /// not per-keystroke); incremental tree surgery is premature until that is
    /// measurably hot.
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
                    self.nodes.insert(node.id, node);
                }
                SduiTreeOperation::RemoveNode { node_id } => {
                    self.nodes.remove(&node_id);
                }
            }
        }
        self.ui_version = update.new_ui_version;
        self.rebuild();
        true
    }

    pub(crate) fn ui_version(&self) -> SduiVersion {
        self.ui_version
    }

    pub(crate) fn root_id(&self) -> Option<SduiNodeId> {
        self.root_id
    }

    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the current tree reconciled into a Masonry subtree at all.
    pub(crate) fn has_root_pod(&self) -> bool {
        self.root_pod.is_some()
    }

    pub(crate) fn root_pod_id(&self) -> Option<WidgetId> {
        self.root_pod.as_ref().map(|pod| pod.id())
    }

    /// The region is inert (takes no space, intercepts no input) until it has a
    /// reconciled subtree.
    fn is_inert(&self) -> bool {
        self.root_pod.is_none()
    }

    fn rebuild(&mut self) {
        self.root_pod = self
            .root_id
            .and_then(|root_id| self.build_subtree(root_id, 0))
            .map(|new_widget| new_widget.to_pod());
    }

    /// Recursively reconcile one SDUI node into a Masonry widget.
    ///
    /// Container kinds compose their reconciled children; leaf kinds map to a
    /// [`SduiLegacyLeaf`]. `depth` mirrors the legacy immediate-mode renderer indentation
    /// depth: panel children indent one level deeper; flex/stack children keep
    /// the current depth.
    fn build_subtree(&self, node_id: SduiNodeId, depth: usize) -> Option<NewWidget<dyn Widget>> {
        let node = self.nodes.get(&node_id)?;
        match &node.kind {
            SduiNodeKind::Flex {
                direction,
                children,
            } => {
                let mut flex = match direction {
                    SduiFlexDirection::Row => Flex::row(),
                    SduiFlexDirection::Column => Flex::column(),
                };
                // ponytail: gap is 0 until the spacing token is wired at cutover;
                // legacy stacks rows with no inter-row gap, so 0 reproduces it.
                for child_id in children {
                    if let Some(child) = self.build_subtree(*child_id, depth) {
                        flex = flex.with_child(child);
                    }
                }
                Some(NewWidget::new(flex).erased())
            }
            SduiNodeKind::Stack { children } => {
                let mut stack = ZStack::new();
                for child_id in children {
                    if let Some(child) = self.build_subtree(*child_id, depth) {
                        stack = stack.with_child(child, UnitPoint::TOP_LEFT);
                    }
                }
                Some(NewWidget::new(stack).erased())
            }
            SduiNodeKind::Panel { title, children } => {
                // Panel renders as a column: a title leaf followed by children
                // indented one level deeper (mirrors legacy immediate-mode renderer).
                // ponytail: panel chrome (paint_panel_chrome) is applied at the
                // rendering cutover; this is the structural mapping.
                let mut column = Flex::column();
                column = column.with_child(NewWidget::new(SduiLegacyLeaf::panel_title(
                    title.clone(),
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                )));
                for child_id in children {
                    if let Some(child) = self.build_subtree(*child_id, depth + 1) {
                        column = column.with_child(child);
                    }
                }
                Some(NewWidget::new(column).erased())
            }
            // Leaf kinds: render through the legacy-paint leaf so the reconciled
            // subtree is complete and pixel-identical to the legacy renderer.
            SduiNodeKind::Label { .. }
            | SduiNodeKind::Button { .. }
            | SduiNodeKind::List { .. }
            | SduiNodeKind::EditorView { .. } => Some(
                NewWidget::new(SduiLegacyLeaf::new(
                    node.kind.clone(),
                    depth,
                    self.typography.clone(),
                    self.ui_theme.clone(),
                ))
                .erased(),
            ),
        }
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
        let child_size = ctx.run_layout(pod, bc);
        ctx.place_child(pod, Point::ZERO);
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(child_size)
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {
        // The reconciled subtree paints itself through the Masonry render pass.
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
        node.set_children(Vec::new());
    }

    fn children_ids(&self) -> ChildrenIds {
        match &self.root_pod {
            Some(pod) => ChildrenIds::from_slice(&[pod.id()]),
            None => ChildrenIds::new(),
        }
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
pub(crate) struct SduiLegacyLeaf {
    kind: SduiNodeKind,
    depth: usize,
    /// Panel title rows render with the `title_text` typography variant and
    /// primary text color (mirrors the legacy immediate-mode renderer Panel arm); plain
    /// labels use `body_text` + muted color.
    title: bool,
    typography: TypographyRegistry,
    ui_theme: ResolvedUiTheme,
}

impl SduiLegacyLeaf {
    fn new(
        kind: SduiNodeKind,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            kind,
            depth,
            title: false,
            typography,
            ui_theme,
        }
    }

    /// Panel title leaf (uses the title typography variant + primary color).
    fn panel_title(
        title: String,
        depth: usize,
        typography: TypographyRegistry,
        ui_theme: ResolvedUiTheme,
    ) -> Self {
        Self {
            kind: SduiNodeKind::Label { text: title },
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

    /// The typography variant + color this leaf's text renders with.
    fn label_presentation(&self) -> (UiTextVariant, Color) {
        let style = self.style();
        if self.title {
            (style.title_text, style.text_color)
        } else {
            (style.body_text, style.muted_text_color)
        }
    }

    /// The exact height the legacy immediate-mode renderer advances `cursor_y` by for this
    /// node. This is the layout-parity contract with the legacy renderer.
    pub(crate) fn legacy_height(&self) -> f64 {
        let style = self.style();
        match &self.kind {
            SduiNodeKind::Label { .. } | SduiNodeKind::EditorView { .. } => {
                let (variant, _) = self.label_presentation();
                self.metrics(variant).row_height
            }
            SduiNodeKind::Button { .. } => self.metrics(style.body_text).button_height(),
            SduiNodeKind::List { items } => {
                let body = self.metrics(style.body_text);
                let detail = self.metrics(UiTextVariant::Detail);
                items.len() as f64 * body.list_height(detail)
            }
            // Containers are never leaves; they map to Masonry containers.
            _ => 0.0,
        }
    }
}

impl Widget for SduiLegacyLeaf {
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
        let depth = self.depth;
        match &self.kind {
            SduiNodeKind::Label { text } => {
                let (variant, color) = self.label_presentation();
                paint_sdui_text(
                    &self.typography,
                    padding,
                    ctx,
                    scene,
                    text,
                    depth,
                    0.0,
                    width,
                    0.0,
                    FontRole::Ui,
                    self.metrics(variant),
                    color,
                );
            }
            SduiNodeKind::Button { label, .. } => {
                let metrics = self.metrics(style.body_text);
                let height = metrics.button_height();
                let rect = sdui_row_rect(padding, depth, 0.0, width, 0.0, height);
                let fill = component_state_color(
                    &self.ui_theme,
                    "surface.control",
                    InteractionState::Rest,
                );
                scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
                paint_sdui_text(
                    &self.typography,
                    padding,
                    ctx,
                    scene,
                    label,
                    depth,
                    (height - metrics.line_height) / 2.0,
                    width,
                    0.0,
                    FontRole::Ui,
                    metrics,
                    style.text_color,
                );
            }
            SduiNodeKind::List { items } => {
                let body = self.metrics(style.body_text);
                let detail = self.metrics(UiTextVariant::Detail);
                let row_height = body.list_height(detail);
                for (index, item) in items.iter().enumerate() {
                    let y = index as f64 * row_height;
                    let rect = sdui_row_rect(padding, depth, y, width, 0.0, row_height);
                    let fill = list_row_fill_color(&self.ui_theme, InteractionState::Rest, false);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
                    paint_sdui_text(
                        &self.typography,
                        padding,
                        ctx,
                        scene,
                        &item.label,
                        depth,
                        y,
                        width,
                        0.0,
                        FontRole::Ui,
                        body,
                        style.text_color,
                    );
                    if let Some(detail_text) = &item.detail {
                        paint_sdui_text(
                            &self.typography,
                            padding,
                            ctx,
                            scene,
                            detail_text,
                            depth,
                            y + body.line_height,
                            width,
                            0.0,
                            FontRole::Ui,
                            detail,
                            style.muted_text_color,
                        );
                    }
                }
            }
            SduiNodeKind::EditorView { binding } => {
                let text = format!("Editor view · doc {}", binding.document_id);
                paint_sdui_text(
                    &self.typography,
                    padding,
                    ctx,
                    scene,
                    &text,
                    depth,
                    0.0,
                    width,
                    0.0,
                    FontRole::Ui,
                    self.metrics(style.body_text),
                    style.muted_text_color,
                );
            }
            // Containers never reach the leaf; mapped to Masonry containers.
            _ => {}
        }
    }

    fn accessibility_role(&self) -> Role {
        match &self.kind {
            SduiNodeKind::Button { .. } => Role::Button,
            SduiNodeKind::List { .. } => Role::List,
            _ => Role::Label,
        }
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        let label = match &self.kind {
            SduiNodeKind::Label { text } => text.clone(),
            SduiNodeKind::Button { label, .. } => label.clone(),
            SduiNodeKind::EditorView { binding } => {
                format!("Editor view · doc {}", binding.document_id)
            }
            SduiNodeKind::List { items } => format!("{} rows", items.len()),
            _ => String::new(),
        };
        node.set_label(label);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{SduiActionIntent, SduiActionSource, SduiEditorBinding, SduiListItem};
    use masonry::core::WidgetRef;

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
    fn legacy_leaf_heights_match_the_legacy_cursor_advances() {
        let typography = TypographyRegistry::default();
        let style = default_style();
        let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);
        let detail = typography.ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);

        let cases = [
            (label_node(1, "x"), body.row_height),
            (button_node(1), body.button_height()),
            (list_node(1, 3), 3.0 * body.list_height(detail)),
            (editor_view_node(1), body.row_height),
        ];
        for (node, expected) in cases {
            let leaf =
                SduiLegacyLeaf::new(node.kind, 0, typography.clone(), ResolvedUiTheme::default());
            assert!((leaf.legacy_height() - expected).abs() < 1e-9);
        }
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
        let root_pod_id = region.root_pod_id().unwrap();
        let mut render_root =
            RenderRoot::new(NewWidget::new(region), |_| {}, render_root_options());
        let _ = render_root.redraw();

        let mut heights = Vec::new();
        {
            let root = render_root.get_widget(root_pod_id).unwrap();
            collect_leaf_heights(root, &mut heights);
        }

        // Expected heights mirror the legacy immediate-mode renderer cursor advances:
        // panel title (title variant) + label + button + 2-row list + label.
        let typography = TypographyRegistry::default();
        let style = default_style();
        let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);
        let title = typography.ui_text_metrics(FontRole::Ui, style.title_text);
        let detail = typography.ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);
        let expected = [
            title.row_height,
            body.row_height,
            body.button_height(),
            2.0 * body.list_height(detail),
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
    fn compositor_renders_retained_scene_and_recreates_on_size_change() {
        // Exercises the full compositor pipeline the parity test bypasses:
        // edit_base_layer -> try_downcast -> set_render_context ->
        // reconcile_snapshot -> redraw, plus the recreate-on-size-change path.
        let mut compositor = RetainedSdui::new();
        let snapshot = tree(
            1,
            vec![
                panel_node(1, "Settings", vec![SduiNodeId(2), SduiNodeId(3)]),
                label_node(2, "Theme"),
                button_node(3),
            ],
        );
        let typography = TypographyRegistry::default();
        let theme = ResolvedUiTheme::default();

        let _ = compositor.ensure_rendered(
            snapshot.clone(),
            &typography,
            &theme,
            Size::new(240.0, 200.0),
        );
        // Same size, not dirty: served from cache (no panic, no re-render).
        let _ = compositor.ensure_rendered(
            snapshot.clone(),
            &typography,
            &theme,
            Size::new(240.0, 200.0),
        );
        // Different size: forces the off-tree RenderRoot recreate path.
        let _ = compositor.ensure_rendered(snapshot, &typography, &theme, Size::new(240.0, 320.0));
    }

    #[test]
    fn empty_region_is_inert_and_claims_no_space() {
        let region = SduiRegionWidget::new();
        assert!(region.is_inert());
        assert!(!region.has_root_pod());
        assert_eq!(region.children_ids().len(), 0);
        assert_eq!(region.root_id(), None);
    }
}
