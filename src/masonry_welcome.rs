//! Clay-owned first-launch entry surface.
//!
//! The server still owns the tab's empty welcome document. This module owns
//! only its native presentation and emits the existing client UI command
//! routes for user-mediated file/folder dialogs.

use std::cell::RefCell;
use std::rc::Rc;

use masonry::accesskit::{Action, Live, Node, Role};
use masonry::core::keyboard::{Key, NamedKey};
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, PaintCtx,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx, Widget,
    WidgetPod,
};
use masonry::kurbo::{Affine, Point, Rect, Size};
use masonry::peniko::{Color, Fill};
use masonry::vello::Scene;

use crate::client::ClientUiCommandRoute;
use crate::editor::accessibility::{sanitize_document_display_name, sanitize_recovery_summary};
use crate::editor::typography::{TypographyRegistry, UiTextVariant};
use crate::masonry_editor::{EditorAction, EditorConnectionStatus, EditorStatus};
use crate::masonry_sdui::paint_sdui_text;
use crate::protocol::{DocumentAccess, RoutingPolicy};
use crate::shell::ResolvedUiTheme;
use crate::shell::primitives::{
    InteractionState, PanelChrome, component_state_color, paint_focus_ring, paint_panel_chrome,
};

const OPEN_FILE_COMMAND: &str = "documents.clientOpenFileDialog";
const OPEN_FOLDER_COMMAND: &str = "workspace.clientOpenFolderDialog";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WelcomeState {
    workspace_name: String,
    connection: EditorConnectionStatus,
    access: String,
    diagnostic: Option<String>,
}

impl Default for WelcomeState {
    fn default() -> Self {
        Self::from_status(&EditorStatus::local_fallback(), "Workspace")
    }
}

impl WelcomeState {
    pub(crate) fn from_status(status: &EditorStatus, workspace_path: &str) -> Self {
        let workspace_name = if workspace_path.trim().is_empty() {
            "Workspace".to_string()
        } else {
            sanitize_document_display_name(workspace_path)
        };
        let access = match status.access.as_ref() {
            Some(DocumentAccess::Editable { .. }) => "Editable",
            Some(DocumentAccess::ReadOnly) => "Read-only",
            None => "No server",
        }
        .to_string();
        let diagnostic = status.runtime_diagnostic.as_ref().and_then(|diagnostic| {
            sanitize_recovery_summary(&format!(
                "Runtime {}: {}",
                diagnostic.code, diagnostic.message
            ))
        });
        Self {
            workspace_name,
            connection: status.connection.clone(),
            access,
            diagnostic,
        }
    }

    fn headline(&self) -> &'static str {
        match (&self.connection, self.diagnostic.is_some()) {
            (EditorConnectionStatus::Connecting, _) => "Loading Clay",
            (EditorConnectionStatus::Connected, true) => "Clay needs attention",
            (EditorConnectionStatus::Connected, false) => "Ready to edit",
            (EditorConnectionStatus::LocalFallback, _) => "No server connected",
            (EditorConnectionStatus::Disconnected, _) => "Connection lost",
        }
    }

    fn detail(&self) -> String {
        match (&self.connection, self.diagnostic.as_deref()) {
            (EditorConnectionStatus::Connecting, _) => {
                "Clay is connecting to its server. Your workspace will appear when ready."
                    .to_string()
            }
            (EditorConnectionStatus::Connected, Some(diagnostic)) => {
                format!("{diagnostic}. Reload configuration from Command Centre or restart Clay.")
            }
            (EditorConnectionStatus::Connected, None) => {
                "Open a file or folder to start editing.".to_string()
            }
            (EditorConnectionStatus::LocalFallback, _) => {
                "Start or connect to a Clay server, then open a workspace.".to_string()
            }
            (EditorConnectionStatus::Disconnected, _) => {
                "Restart Clay, then open a file or folder again.".to_string()
            }
        }
    }

    fn connection_label(&self) -> &'static str {
        match self.connection {
            EditorConnectionStatus::Connecting => "Connecting",
            EditorConnectionStatus::Connected => "Connected",
            EditorConnectionStatus::LocalFallback => "Local fallback",
            EditorConnectionStatus::Disconnected => "Disconnected",
        }
    }

    pub(crate) fn accessibility_label(&self) -> String {
        let label = format!(
            "{}; {}; Workspace: {}; Connection: {}; Access: {}.",
            self.headline(),
            self.detail(),
            self.workspace_name,
            self.connection_label(),
            self.access
        );
        sanitize_recovery_summary(&label).unwrap_or_else(|| "Welcome to Clay".to_string())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WelcomeRenderContext {
    pub(crate) typography: TypographyRegistry,
    pub(crate) ui_theme: ResolvedUiTheme,
}

#[derive(Clone, Copy)]
struct WelcomeGeometry {
    card: Rect,
    padding: f64,
    title_y: f64,
    intro_y: f64,
    workspace_y: f64,
    open_file_y: f64,
    open_folder_y: f64,
    shortcut_y: f64,
    state_y: f64,
    button_width: f64,
    button_height: f64,
}

pub(crate) struct WelcomeWidget {
    state: Rc<RefCell<WelcomeState>>,
    render: Rc<RefCell<WelcomeRenderContext>>,
    open_file: WidgetPod<WelcomeButton>,
    open_folder: WidgetPod<WelcomeButton>,
}

impl WelcomeWidget {
    pub(crate) fn new(
        state: Rc<RefCell<WelcomeState>>,
        render: Rc<RefCell<WelcomeRenderContext>>,
    ) -> Self {
        Self {
            open_file: WidgetPod::new(WelcomeButton::new(
                "Open File",
                OPEN_FILE_COMMAND,
                render.clone(),
            )),
            open_folder: WidgetPod::new(WelcomeButton::new(
                "Open Folder",
                OPEN_FOLDER_COMMAND,
                render.clone(),
            )),
            state,
            render,
        }
    }

    fn geometry(&self, size: Size) -> WelcomeGeometry {
        let render = self.render.borrow();
        let scale = f64::from(render.ui_theme.spacing_scale());
        let outer = render.ui_theme.scalar_f64("spacing.lg").unwrap_or(24.0) * scale;
        let inset = outer
            .min(size.width.max(0.0) / 4.0)
            .min(size.height.max(0.0) / 4.0);
        let card = Rect::new(
            inset,
            inset,
            (size.width - inset).max(inset),
            (size.height - inset).max(inset),
        );
        let padding = render.ui_theme.scalar_f64("spacing.md").unwrap_or(16.0) * scale;
        let gap = render.ui_theme.scalar_f64("spacing.sm").unwrap_or(12.0) * scale;
        let tight_gap = render.ui_theme.scalar_f64("spacing.xs").unwrap_or(8.0) * scale;
        let title = render
            .typography
            .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Title);
        let body = render
            .typography
            .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Body);
        let detail = render
            .typography
            .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Detail);
        let button_height = body.button_height();
        let title_y = card.y0 + padding;
        let intro_y = title_y + title.row_height;
        let workspace_y = intro_y + body.row_height;
        let open_file_y = workspace_y + detail.row_height + gap;
        let open_folder_y = open_file_y + button_height + tight_gap;
        let shortcut_y = open_folder_y + button_height + gap;
        let state_y = shortcut_y + detail.row_height + tight_gap;
        Self::geometry_from_parts(
            card,
            padding,
            title_y,
            intro_y,
            workspace_y,
            open_file_y,
            open_folder_y,
            shortcut_y,
            state_y,
            (card.width() - padding * 2.0).max(0.0),
            button_height,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn geometry_from_parts(
        card: Rect,
        padding: f64,
        title_y: f64,
        intro_y: f64,
        workspace_y: f64,
        open_file_y: f64,
        open_folder_y: f64,
        shortcut_y: f64,
        state_y: f64,
        button_width: f64,
        button_height: f64,
    ) -> WelcomeGeometry {
        WelcomeGeometry {
            card,
            padding,
            title_y,
            intro_y,
            workspace_y,
            open_file_y,
            open_folder_y,
            shortcut_y,
            state_y,
            button_width,
            button_height,
        }
    }

    fn text_color(theme: &ResolvedUiTheme) -> Color {
        theme.color("text.primary").unwrap_or(Color::BLACK)
    }

    fn muted_color(theme: &ResolvedUiTheme) -> Color {
        theme.color("text.muted").unwrap_or(Color::BLACK)
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_line(
        &self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut Scene,
        text: &str,
        variant: UiTextVariant,
        x: f64,
        y: f64,
        width: f64,
        color: Color,
    ) {
        let render = self.render.borrow();
        let metrics = render
            .typography
            .ui_text_metrics(crate::protocol::FontRole::Ui, variant);
        paint_sdui_text(
            &render.typography,
            0.0,
            ctx,
            scene,
            text,
            0,
            y,
            width,
            x,
            crate::protocol::FontRole::Ui,
            metrics,
            color,
        );
    }
}

impl Widget for WelcomeWidget {
    type Action = EditorAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.open_file);
        ctx.register_child(&mut self.open_folder);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = bc.max();
        let geometry = self.geometry(size);
        let constraints =
            BoxConstraints::tight(Size::new(geometry.button_width, geometry.button_height));
        let _ = ctx.run_layout(&mut self.open_file, &constraints);
        ctx.place_child(
            &mut self.open_file,
            Point::new(geometry.card.x0 + geometry.padding, geometry.open_file_y),
        );
        let _ = ctx.run_layout(&mut self.open_folder, &constraints);
        ctx.place_child(
            &mut self.open_folder,
            Point::new(geometry.card.x0 + geometry.padding, geometry.open_folder_y),
        );
        size
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let render = self.render.borrow();
        paint_panel_chrome(
            scene,
            self.geometry(_ctx.size()).card,
            &PanelChrome {
                title: None,
                collapse: InteractionState::Rest,
                resize: InteractionState::Rest,
            },
            &render.ui_theme,
        );
    }

    fn post_paint(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        scene: &mut Scene,
    ) {
        let geometry = self.geometry(ctx.size());
        let render = self.render.borrow();
        let state = self.state.borrow().clone();
        let text_color = Self::text_color(&render.ui_theme);
        let muted_color = Self::muted_color(&render.ui_theme);
        drop(render);

        self.paint_line(
            ctx,
            scene,
            "Welcome to Clay",
            UiTextVariant::Title,
            geometry.card.x0 + geometry.padding,
            geometry.title_y,
            geometry.card.width() - geometry.padding * 2.0,
            text_color,
        );
        self.paint_line(
            ctx,
            scene,
            "Open a file or folder to start editing.",
            UiTextVariant::Body,
            geometry.card.x0 + geometry.padding,
            geometry.intro_y,
            geometry.card.width() - geometry.padding * 2.0,
            muted_color,
        );
        self.paint_line(
            ctx,
            scene,
            &format!("Workspace: {}", state.workspace_name),
            UiTextVariant::Detail,
            geometry.card.x0 + geometry.padding,
            geometry.workspace_y,
            geometry.card.width() - geometry.padding * 2.0,
            muted_color,
        );
        self.paint_line(
            ctx,
            scene,
            "Shortcuts: Ctrl+X Ctrl+P Command Centre · Ctrl+\\ split pane · Ctrl+T new tab",
            UiTextVariant::Caption,
            geometry.card.x0 + geometry.padding,
            geometry.shortcut_y,
            geometry.card.width() - geometry.padding * 2.0,
            muted_color,
        );
        self.paint_line(
            ctx,
            scene,
            state.headline(),
            UiTextVariant::Section,
            geometry.card.x0 + geometry.padding,
            geometry.state_y,
            geometry.card.width() - geometry.padding * 2.0,
            text_color,
        );
        self.paint_line(
            ctx,
            scene,
            &state.detail(),
            UiTextVariant::Detail,
            geometry.card.x0 + geometry.padding,
            geometry.state_y
                + self
                    .render
                    .borrow()
                    .typography
                    .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Section)
                    .row_height,
            geometry.card.width() - geometry.padding * 2.0,
            muted_color,
        );
        self.paint_line(
            ctx,
            scene,
            &format!(
                "Connection: {} · Access: {}",
                state.connection_label(),
                state.access
            ),
            UiTextVariant::Detail,
            geometry.card.x0 + geometry.padding,
            geometry.state_y
                + self
                    .render
                    .borrow()
                    .typography
                    .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Section)
                    .row_height
                + self
                    .render
                    .borrow()
                    .typography
                    .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Detail)
                    .row_height,
            geometry.card.width() - geometry.padding * 2.0,
            muted_color,
        );
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
        let state = self.state.borrow();
        node.set_label("Welcome to Clay");
        let status_id = crate::editor::accessibility::virtual_a11y_node_id(
            ctx.widget_id(),
            crate::editor::accessibility::virtual_a11y_slots::STATUS,
        );
        let mut status = Node::new(Role::Status);
        status.set_label(state.accessibility_label());
        status.set_live(Live::Polite);
        ctx.tree_update().nodes.push((status_id, status));
        node.set_children(vec![
            status_id,
            self.open_file.id().into(),
            self.open_folder.id().into(),
        ]);
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.open_file.id(), self.open_folder.id()])
    }
}

struct WelcomeButton {
    label: &'static str,
    command_id: &'static str,
    render: Rc<RefCell<WelcomeRenderContext>>,
}

impl WelcomeButton {
    fn new(
        label: &'static str,
        command_id: &'static str,
        render: Rc<RefCell<WelcomeRenderContext>>,
    ) -> Self {
        Self {
            label,
            command_id,
            render,
        }
    }

    fn interaction_state(&self, ctx: &PaintCtx<'_>) -> InteractionState {
        if ctx.is_active() {
            InteractionState::Active
        } else if ctx.is_focus_target() {
            InteractionState::Focus
        } else if ctx.is_hovered() {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        }
    }

    fn action(&self) -> EditorAction {
        EditorAction::ClientUiCommand(ClientUiCommandRoute {
            command_id: self.command_id.to_string(),
            routing_policy: RoutingPolicy::ClientUiCommand,
        })
    }

    fn press(&self, ctx: &mut EventCtx<'_>) {
        ctx.submit_action::<EditorAction>(self.action());
    }
}

impl Widget for WelcomeButton {
    type Action = EditorAction;

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
            _ => {}
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if let TextEvent::Keyboard(event) = event
            && event.state.is_up()
            && (matches!(&event.key, Key::Character(text) if text == " ")
                || event.key == Key::Named(NamedKey::Enter))
        {
            self.press(ctx);
        }
    }

    fn on_access_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &AccessEvent,
    ) {
        if event.action == Action::Click {
            self.press(ctx);
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx<'_>, _props: &mut PropertiesMut<'_>, event: &Update) {
        if matches!(
            event,
            Update::HoveredChanged(_) | Update::ActiveChanged(_) | Update::FocusChanged(_)
        ) {
            ctx.request_paint_only();
        }
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let render = self.render.borrow();
        let metrics = render
            .typography
            .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Body);
        bc.constrain(Size::new(bc.max().width, metrics.button_height()))
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let render = self.render.borrow();
        let metrics = render
            .typography
            .ui_text_metrics(crate::protocol::FontRole::Ui, UiTextVariant::Body);
        let padding = render.ui_theme.scalar_f64("spacing.md").unwrap_or(16.0)
            * f64::from(render.ui_theme.spacing_scale());
        let state = self.interaction_state(ctx);
        let rect = ctx.size().to_rect();
        let fill = component_state_color(&render.ui_theme, "surface.control", state);
        let text = render
            .ui_theme
            .color("text.primary")
            .unwrap_or(Color::BLACK);
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &rect);
        paint_sdui_text(
            &render.typography,
            padding,
            ctx,
            scene,
            self.label,
            0,
            (metrics.button_height() - metrics.line_height) / 2.0,
            rect.width(),
            0.0,
            crate::protocol::FontRole::Ui,
            metrics,
            text,
        );
        if state == InteractionState::Focus {
            paint_focus_ring(scene, rect, &render.ui_theme);
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
        node.set_label(self.label);
        node.add_action(Action::Click);
    }

    fn accepts_focus(&self) -> bool {
        true
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_state_uses_basename_and_actionable_recovery_copy() {
        let mut status = EditorStatus::connected(7, 1, DocumentAccess::Editable { lease_id: 3 });
        status.runtime_diagnostic = Some(crate::protocol::RuntimeDiagnostic::error(
            "runtime.invalid",
            "configuration failed",
        ));
        let state = WelcomeState::from_status(&status, "/home/user/project");
        assert_eq!(state.workspace_name, "project");
        assert_eq!(state.headline(), "Clay needs attention");
        assert!(state.detail().contains("Reload configuration"));
        assert!(!state.accessibility_label().contains("/home/user"));
    }

    #[test]
    fn welcome_state_matrix_has_actionable_accessible_statuses() {
        let mut diagnostic =
            EditorStatus::connected(7, 1, DocumentAccess::Editable { lease_id: 3 });
        diagnostic.runtime_diagnostic = Some(crate::protocol::RuntimeDiagnostic::error(
            "runtime.invalid",
            "configuration failed",
        ));
        let mut disconnected = EditorStatus::local_fallback();
        disconnected.connection = EditorConnectionStatus::Disconnected;

        let cases = [
            (EditorStatus::connecting(), "Loading Clay", "Connecting"),
            (
                EditorStatus::connected(7, 1, DocumentAccess::Editable { lease_id: 3 }),
                "Ready to edit",
                "Connected",
            ),
            (diagnostic, "Clay needs attention", "Connected"),
            (
                EditorStatus::local_fallback(),
                "No server connected",
                "Local fallback",
            ),
            (disconnected, "Connection lost", "Disconnected"),
        ];

        for (status, headline, connection) in cases {
            let state = WelcomeState::from_status(&status, "/home/user/project");
            let label = state.accessibility_label();
            assert_eq!(state.headline(), headline);
            assert_eq!(state.connection_label(), connection);
            assert!(label.contains(headline));
            assert!(label.contains("Workspace: project"));
            assert!(!label.contains("/home/user"));
        }
    }

    #[test]
    fn welcome_button_routes_existing_client_command_without_server_intent() {
        let render = Rc::new(RefCell::new(WelcomeRenderContext::default()));
        for (label, command_id) in [
            ("Open File", OPEN_FILE_COMMAND),
            ("Open Folder", OPEN_FOLDER_COMMAND),
        ] {
            let button = WelcomeButton::new(label, command_id, render.clone());
            match button.action() {
                EditorAction::ClientUiCommand(route) => {
                    assert_eq!(route.command_id, command_id);
                    assert_eq!(route.routing_policy, RoutingPolicy::ClientUiCommand);
                }
                action => panic!("welcome action must stay client-local: {action:?}"),
            }
        }
    }

    #[test]
    fn welcome_state_is_bounded_and_narrow_layout_stays_valid() {
        let mut status = EditorStatus::connected(7, 1, DocumentAccess::Editable { lease_id: 3 });
        status.runtime_diagnostic = Some(crate::protocol::RuntimeDiagnostic::error(
            "runtime.invalid",
            "x".repeat(1024),
        ));
        let state = WelcomeState::from_status(&status, &format!("/home/user/{}", "x".repeat(512)));
        assert!(state.accessibility_label().chars().count() <= 256);
        assert!(!state.accessibility_label().contains("/home/user"));

        let render = Rc::new(RefCell::new(WelcomeRenderContext::default()));
        let widget = WelcomeWidget::new(Rc::new(RefCell::new(state)), render);
        let geometry = widget.geometry(Size::new(80.0, 64.0));
        assert!(geometry.button_width >= 0.0);
        assert!(geometry.button_height >= 0.0);
    }
}
