//! Phase 22.1: generic pane content host.
//!
//! Every pane leaf of the working-area split tree owns one [`PaneContentHost`]
//! with a stable `WidgetId`. Content is mounted inside the host (`EditorWidget`
//! today; later phases may mount other workspace apps), so split/close
//! reconciliation never rebuilds surviving panes. The host is content-type
//! agnostic and workspace-bound by construction: it carries its pane identity,
//! never a file path.

use masonry::accesskit::{Node, Role};
#[cfg(test)]
use masonry::core::WidgetId;
use masonry::core::{
    AccessCtx, BoxConstraints, ChildrenIds, LayoutCtx, MutateCtx, NewWidget, NoAction, PaintCtx,
    PropertiesMut, PropertiesRef, RegisterCtx, Widget, WidgetPod,
};
use masonry::kurbo::{Point, Rect, Size};
use masonry::peniko::Color;
use masonry::vello::Scene;

use crate::masonry_editor::EditorWidget;
use crate::masonry_pane_document::PaneDocumentView;
use crate::shell::{PaneId, ResolvedUiTheme};

/// Content mounted inside a [`PaneContentHost`].
///
/// Phase 22.1 ships `Placeholder` for newly split panes; the initial editor
/// pane hosts the client's connection owner (`EditorWidget`). Phase 22.2 adds
/// `Document` for panes hosting a live [`PaneDocumentView`] (mounted by the
/// app driver when a document opens into a placeholder pane).
///
/// EXTENSION SEAM: this is a closed enum by design — a future pane kind
/// (e.g. a terminal) would add a variant here and a matching branch in the
/// host's paint/register/layout paths; whether a package-facing `Custom`
/// variant (arbitrary package widgets mounted into a pane) is ever added is
/// a Plan 079+ decision. Nothing outside the host may assume a variant set
/// beyond these three.
#[doc(hidden)]
pub enum PaneContent {
    Placeholder,
    Editor(WidgetPod<EditorWidget>),
    Document(WidgetPod<PaneDocumentView>),
}

/// Generic workspace-bound content host for one pane leaf.
#[doc(hidden)]
pub struct PaneContentHost {
    pane_id: PaneId,
    content: PaneContent,
    /// Phase 22.6: total pane count of the owning tab's split tree, for the
    /// "Pane N of M" accessibility label (kept current by the shell's
    /// `reconcile_pane_hosts`).
    pane_count: usize,
    /// Phase 22.6: sanitized display name of the mounted document (set by the
    /// app driver when a document open/reload lands in this pane; `None` for
    /// placeholder/editor panes and after `clear_content`).
    document_display_name: Option<String>,
    /// Resolved UI theme for the placeholder fill. Stamped by the shell at
    /// creation and on active-theme changes so split panes follow the theme.
    ui_theme: ResolvedUiTheme,
}

impl PaneContentHost {
    pub(crate) fn placeholder(pane_id: PaneId) -> Self {
        Self {
            pane_id,
            content: PaneContent::Placeholder,
            pane_count: 1,
            document_display_name: None,
            ui_theme: ResolvedUiTheme::default(),
        }
    }

    pub(crate) fn with_editor(pane_id: PaneId, editor: NewWidget<EditorWidget>) -> Self {
        Self {
            pane_id,
            content: PaneContent::Editor(editor.to_pod()),
            pane_count: 1,
            document_display_name: None,
            ui_theme: ResolvedUiTheme::default(),
        }
    }

    /// Builder-set resolved UI theme (stamped before the first register pass).
    pub(crate) fn with_ui_theme(mut self, ui_theme: ResolvedUiTheme) -> Self {
        self.ui_theme = ui_theme;
        self
    }

    /// Install a newer resolved UI theme for the placeholder fill.
    pub(crate) fn set_ui_theme(&mut self, ui_theme: ResolvedUiTheme) {
        self.ui_theme = ui_theme;
    }

    /// Phase 22.6: builder-set pane count for hosts created from an existing
    /// multi-pane tree (restore path), before the first reconcile.
    pub(crate) fn with_pane_count(mut self, count: usize) -> Self {
        self.pane_count = count.max(1);
        self
    }

    /// Phase 22.6: keep the "Pane N of M" accessibility label current.
    pub(crate) fn set_pane_count(&mut self, ctx: &mut MutateCtx<'_>, count: usize) {
        let count = count.max(1);
        if self.pane_count == count {
            return;
        }
        self.pane_count = count;
        ctx.request_accessibility_update();
    }

    /// Phase 22.6: sanitized document display name for the pane's
    /// accessibility label (`None` clears it, e.g. when the pane empties).
    pub(crate) fn set_document_display_name(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        name: Option<String>,
    ) {
        if self.document_display_name == name {
            return;
        }
        self.document_display_name = name;
        ctx.request_accessibility_update();
    }

    /// Mount a live document view in this pane (placeholder → document).
    pub fn set_document_view(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        view: NewWidget<PaneDocumentView>,
    ) {
        self.content = PaneContent::Document(view.to_pod());
        ctx.children_changed();
    }

    /// Remove the mounted document view (document → placeholder). The view pod
    /// is detached immediately so Masonry's canonical children list stays
    /// consistent.
    pub fn clear_content(&mut self, ctx: &mut MutateCtx<'_>) {
        if let PaneContent::Document(view) =
            std::mem::replace(&mut self.content, PaneContent::Placeholder)
        {
            ctx.remove_child(view);
        }
        self.document_display_name = None;
        ctx.children_changed();
        ctx.request_accessibility_update();
    }

    /// The mounted document view's widget id, if any (pane routing).
    pub fn document_view_widget_id(&self) -> Option<masonry::core::WidgetId> {
        match &self.content {
            PaneContent::Document(view) => Some(view.id()),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn editor_widget_id(&self) -> Option<WidgetId> {
        match &self.content {
            PaneContent::Editor(pod) => Some(pod.id()),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn is_placeholder(&self) -> bool {
        matches!(self.content, PaneContent::Placeholder)
    }

    /// Test accessor for the placeholder fill color.
    #[cfg(test)]
    pub(crate) fn placeholder_background(&self) -> Color {
        self.ui_theme
            .color("surface.panel")
            .unwrap_or(Color::TRANSPARENT)
    }
}

impl Widget for PaneContentHost {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        match &mut self.content {
            PaneContent::Editor(editor) => ctx.register_child(editor),
            PaneContent::Document(view) => ctx.register_child(view),
            PaneContent::Placeholder => {}
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(400.0, 300.0))
        };
        if let PaneContent::Editor(editor) = &mut self.content {
            ctx.run_layout(editor, &BoxConstraints::tight(size));
            ctx.place_child(editor, Point::ZERO);
        } else if let PaneContent::Document(view) = &mut self.content {
            ctx.run_layout(view, &BoxConstraints::tight(size));
            ctx.place_child(view, Point::ZERO);
        }
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        if !matches!(self.content, PaneContent::Placeholder) {
            return;
        }
        // Inert empty-pane surface: theme token fill only (no JS, no IPC).
        let background = self
            .ui_theme
            .color("surface.panel")
            .unwrap_or(Color::TRANSPARENT);
        let rect = Rect::new(0.0, 0.0, ctx.size().width, ctx.size().height);
        scene.fill(
            masonry::vello::peniko::Fill::NonZero,
            masonry::kurbo::Affine::IDENTITY,
            background,
            None,
            &rect,
        );
    }

    fn accessibility_role(&self) -> Role {
        Role::Pane
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        // Phase 22.6: "Pane N of M" plus the mounted content kind; document
        // panes carry the sanitized document display name (never a host
        // path — the driver sanitizes before setting it).
        node.set_label(match &self.content {
            PaneContent::Placeholder => {
                format!("Empty pane {} of {}", self.pane_id.0, self.pane_count)
            }
            PaneContent::Editor(_) => {
                format!("Pane {} of {}: editor", self.pane_id.0, self.pane_count)
            }
            PaneContent::Document(_) => match &self.document_display_name {
                Some(name) => format!("Pane {} of {}: {}", self.pane_id.0, self.pane_count, name),
                None => format!("Pane {} of {}: document", self.pane_id.0, self.pane_count),
            },
        });
    }

    fn children_ids(&self) -> ChildrenIds {
        match &self.content {
            PaneContent::Editor(pod) => ChildrenIds::from_slice(&[pod.id()]),
            PaneContent::Document(pod) => ChildrenIds::from_slice(&[pod.id()]),
            PaneContent::Placeholder => ChildrenIds::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use masonry::accesskit::{Node, NodeId, Role};
    use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
    use masonry::core::{NewWidget, WindowEvent};
    use masonry::dpi::PhysicalSize;
    use masonry::theme::default_property_set;

    use super::*;
    use crate::protocol::{UiDesignTokenOverride, WireDesignTokenValue};
    use crate::shell::ResolvedUiTheme;

    fn theme_with_panel(color: [u8; 4]) -> ResolvedUiTheme {
        ResolvedUiTheme::from_active_theme(&[UiDesignTokenOverride {
            token: "surface.panel".to_string(),
            value: WireDesignTokenValue::Color(color),
            provenance: "pane-host-test".to_string(),
        }])
        .expect("surface.panel is a valid color-role override")
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

    fn access_tree(render_root: &mut RenderRoot) -> masonry::accesskit::TreeUpdate {
        render_root.handle_window_event(WindowEvent::EnableAccessTree);
        let (_, update) = render_root.redraw();
        update.expect("access tree is active after EnableAccessTree")
    }

    fn pane_nodes(update: &masonry::accesskit::TreeUpdate) -> Vec<(NodeId, &Node)> {
        update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::Pane)
            .map(|(id, node)| (*id, node))
            .collect()
    }

    fn label_of(update: &masonry::accesskit::TreeUpdate) -> Option<&str> {
        pane_nodes(update)
            .first()
            .and_then(|(_, node)| node.label())
    }

    /// Phase 22.7 (finding D5): the placeholder fill follows the installed
    /// theme in both stamping paths — the creation-time builder and the
    /// active-theme-change setter. This is the logic behind the dark-split
    /// bug (split panes painted with the wrong surface).
    #[test]
    fn host_stamps_theme_on_placeholder_at_creation_and_via_setter() {
        let dark = theme_with_panel([0x10, 0x20, 0x30, 0xff]);
        let light = theme_with_panel([0xe0, 0xe0, 0xe0, 0xff]);

        // Creation-time stamping via with_ui_theme.
        let host = PaneContentHost::placeholder(PaneId(3)).with_ui_theme(dark.clone());
        assert!(host.is_placeholder());
        assert_eq!(
            host.placeholder_background(),
            dark.color("surface.panel").unwrap(),
            "creation-time theme stamps the placeholder fill"
        );

        // Setter path (active-theme change) restamps the fill.
        let mut host = host;
        host.set_ui_theme(light.clone());
        assert_eq!(
            host.placeholder_background(),
            light.color("surface.panel").unwrap(),
            "set_ui_theme restamps the placeholder fill"
        );

        // A host without an installed theme resolves the core fallback (never
        // a stale or transparent fill).
        let bare = PaneContentHost::placeholder(PaneId(4));
        assert_eq!(
            bare.placeholder_background(),
            ResolvedUiTheme::default()
                .color("surface.panel")
                .unwrap_or(Color::TRANSPARENT)
        );
    }

    /// Phase 22.7 (finding D5): content swaps (Editor → Document →
    /// Placeholder) preserve the host's pane identity, and the pane-count /
    /// document-name labels stay current through the same transitions.
    #[test]
    fn host_content_transitions_preserve_pane_identity() {
        let menu_session_ids = Rc::new(Cell::new(0u64));
        let sdui_ui_version = Rc::new(Cell::new(0u64));
        let host = PaneContentHost::with_editor(PaneId(3), NewWidget::new(EditorWidget::default()))
            .with_pane_count(2)
            .with_ui_theme(theme_with_panel([0x10, 0x20, 0x30, 0xff]));
        let mut render_root = RenderRoot::new(NewWidget::new(host), |_| {}, render_root_options());

        // Editor content: identity label carries pane id + count.
        let update = access_tree(&mut render_root);
        assert_eq!(pane_nodes(&update).len(), 1, "one Pane node for the host");
        assert_eq!(
            label_of(&update),
            Some("Pane 3 of 2: editor"),
            "editor host exposes pane identity"
        );

        // Pane-count update (reconcile) flows into the label.
        render_root.edit_base_layer(|mut widget| {
            let mut host = widget.try_downcast::<PaneContentHost>().expect("host");
            host.widget.set_pane_count(&mut host.ctx, 4);
        });
        let update = access_tree(&mut render_root);
        assert_eq!(label_of(&update), Some("Pane 3 of 4: editor"));

        // Editor → Document swap: the document view becomes the child, the
        // editor pod leaves the content, and the identity label survives.
        render_root.edit_base_layer(|mut widget| {
            let mut host = widget.try_downcast::<PaneContentHost>().expect("host");
            host.widget.set_document_view(
                &mut host.ctx,
                NewWidget::new(PaneDocumentView::new(
                    PaneId(3),
                    menu_session_ids.clone(),
                    sdui_ui_version.clone(),
                )),
            );
            assert_eq!(
                host.widget.editor_widget_id(),
                None,
                "editor pod is gone after the Document swap"
            );
            assert!(
                host.widget.document_view_widget_id().is_some(),
                "document view is mounted"
            );
        });
        let update = access_tree(&mut render_root);
        assert_eq!(
            label_of(&update),
            Some("Pane 3 of 4: document"),
            "document swap keeps pane identity"
        );

        // Document display name (driver sanitized) lands in the label.
        render_root.edit_base_layer(|mut widget| {
            let mut host = widget.try_downcast::<PaneContentHost>().expect("host");
            host.widget
                .set_document_display_name(&mut host.ctx, Some("report.md".to_string()));
        });
        let update = access_tree(&mut render_root);
        assert_eq!(label_of(&update), Some("Pane 3 of 4: report.md"));

        // Document → Placeholder: view detached, name cleared, identity kept.
        render_root.edit_base_layer(|mut widget| {
            let mut host = widget.try_downcast::<PaneContentHost>().expect("host");
            host.widget.clear_content(&mut host.ctx);
            assert!(host.widget.is_placeholder());
            assert_eq!(host.widget.document_view_widget_id(), None);
        });
        let update = access_tree(&mut render_root);
        assert_eq!(
            label_of(&update),
            Some("Empty pane 3 of 4"),
            "placeholder keeps pane identity"
        );
    }
}
