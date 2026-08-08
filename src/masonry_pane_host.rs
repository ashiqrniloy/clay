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
}

impl PaneContentHost {
    pub(crate) fn placeholder(pane_id: PaneId) -> Self {
        Self {
            pane_id,
            content: PaneContent::Placeholder,
            pane_count: 1,
            document_display_name: None,
        }
    }

    pub(crate) fn with_editor(pane_id: PaneId, editor: NewWidget<EditorWidget>) -> Self {
        Self {
            pane_id,
            content: PaneContent::Editor(editor.to_pod()),
            pane_count: 1,
            document_display_name: None,
        }
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
        let theme = ResolvedUiTheme::default();
        let background = theme.color("surface.panel").unwrap_or(Color::TRANSPARENT);
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
