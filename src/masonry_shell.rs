use masonry::accesskit::{Node, Role};
use masonry::core::{
    AccessCtx, BoxConstraints, ChildrenIds, LayoutCtx, NewWidget, NoAction, PaintCtx,
    PropertiesMut, PropertiesRef, RegisterCtx, Widget, WidgetId, WidgetPod,
};
use masonry::kurbo::{Point, Rect, Size};
use masonry::vello::Scene;

use crate::masonry_editor::EditorWidget;
use crate::shell::{
    ShellComponentKind, WorkingAreaLayout, WorkingAreaLayoutObservation, WorkingAreaLayoutUpdate,
    WorkingAreaLayoutUpdateError,
};

#[cfg(test)]
use crate::shell::{
    FixedSlotId, FixedSlotState, PaneId, PaneSlotId, PaneSlotLayout, PaneSlotLayoutAssignment,
    PaneSplitNode, PaneSplitTree, PaneTreeObservation, ShellComponentId, ShellLayoutVersion,
    SplitOrientation, SplitRatio, WorkingAreaId,
};

/// Internal structural shell snapshot for tests and agent inspection.
///
/// The snapshot deliberately omits Masonry/native widget IDs, document text,
/// source snippets, raw action payload authority, raw filesystem paths, raw CSS,
/// raw ops, and executable package code. Public shell APIs must be introduced
/// separately through Clay JS facade/op/reference-doc coverage.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShellObservableSnapshot {
    pub(crate) layout: WorkingAreaLayoutObservation,
    pub(crate) editor_component_bound: bool,
    pub(crate) sdui_state_present: bool,
    pub(crate) status_present: bool,
}

/// Clay-owned native shell root for a window working area.
///
/// The shell is a Masonry container that keeps the editor as a child component
/// instead of making `EditorWidget` the top-level application layout. It is an
/// internal native implementation detail; packages never receive Masonry widget
/// IDs, widget handles, raw callbacks, or layout mutation authority. The type is
/// Rust-public only so the package's `clay` binary target can construct the
/// library-owned widget; it is not a Clay JS API and has no facade/op/registry
/// entry.
#[doc(hidden)]
pub struct ClayShellWidget {
    layout: WorkingAreaLayout,
    editor: WidgetPod<EditorWidget>,
    editor_widget_id: WidgetId,
}

impl ClayShellWidget {
    pub fn single_editor(editor: EditorWidget) -> Self {
        Self::single_editor_with_layout(editor, WorkingAreaLayout::single_editor())
    }

    fn single_editor_with_layout(editor: EditorWidget, layout: WorkingAreaLayout) -> Self {
        let editor = NewWidget::new(editor);
        let editor_widget_id = editor.id();
        Self {
            layout,
            editor: editor.to_pod(),
            editor_widget_id,
        }
    }

    pub fn editor_widget_id(&self) -> WidgetId {
        self.editor_widget_id
    }

    pub fn focus_fallback_widget_id(&self) -> WidgetId {
        self.editor_widget_id
    }

    #[allow(dead_code)]
    pub(crate) fn apply_layout_update(
        &mut self,
        update: WorkingAreaLayoutUpdate,
    ) -> Result<(), WorkingAreaLayoutUpdateError> {
        self.layout.apply_update(update)
    }

    #[allow(dead_code)]
    pub(crate) fn observable_snapshot(&self, size: Size) -> ShellObservableSnapshot {
        ShellObservableSnapshot {
            layout: self
                .layout
                .observable_snapshot(Rect::new(0.0, 0.0, size.width, size.height)),
            editor_component_bound: self.layout.editor_component().kind
                == ShellComponentKind::Editor,
            sdui_state_present: true,
            status_present: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn working_area_layout(&self) -> &WorkingAreaLayout {
        &self.layout
    }

    #[cfg(test)]
    pub(crate) fn editor_component_rect_for_size(&self, size: Size) -> Rect {
        self.editor_component_rect(size)
    }

    fn layout_size(bc: &BoxConstraints) -> Size {
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(900.0, 600.0))
        }
    }

    fn editor_component_rect(&self, size: Size) -> Rect {
        self.layout
            .editor_component_rect(Rect::new(0.0, 0.0, size.width, size.height))
    }
}

impl Widget for ClayShellWidget {
    type Action = NoAction;

    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        ctx.register_child(&mut self.editor);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = Self::layout_size(bc);
        let editor_rect = self.editor_component_rect(size);
        let child_size = Size::new(editor_rect.width(), editor_rect.height());
        let child_constraints = BoxConstraints::tight(child_size);
        ctx.run_layout(&mut self.editor, &child_constraints);
        ctx.place_child(&mut self.editor, Point::new(editor_rect.x0, editor_rect.y0));
        size
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, _scene: &mut Scene) {}

    fn accessibility_role(&self) -> Role {
        Role::GenericContainer
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        node.set_label(format!(
            "Clay working area shell, active pane {}",
            self.layout.active_pane_id().0
        ));
    }

    fn children_ids(&self) -> ChildrenIds {
        ChildrenIds::from_slice(&[self.editor.id()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{
        ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientResyncSnapshot,
    };
    use crate::masonry_editor::EditorWidget;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, DocumentAccess, EditOperation, SduiEditorBinding,
        SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
    };
    use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
    use masonry::core::keyboard::{Code, Key, KeyState, KeyboardEvent, NamedKey};
    use masonry::core::{Ime, TextEvent};
    use masonry::dpi::PhysicalSize;
    use masonry::theme::default_property_set;

    fn initial_state(access: DocumentAccess, version: u64) -> ClientInitialState {
        ClientInitialState {
            client_id: 11,
            document_id: 7,
            document_version: version,
            text: "server text".to_string(),
            access,
            behavior_manifest: BehaviorManifest::minimal_text_editing(3),
            active_theme: crate::protocol::ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
            },
        }
    }

    fn sdui_tree(label_text: &str) -> SduiTree {
        SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![
                SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Flex {
                        direction: SduiFlexDirection::Row,
                        children: vec![SduiNodeId(2), SduiNodeId(3)],
                    },
                ),
                SduiNode::new(
                    SduiNodeId(2),
                    SduiNodeKind::Label {
                        text: label_text.to_string(),
                    },
                ),
                SduiNode::new(
                    SduiNodeId(3),
                    SduiNodeKind::EditorView {
                        binding: SduiEditorBinding {
                            document_id: 7,
                            expected_version: Some(3),
                        },
                    },
                ),
            ],
        }
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

    fn render_root_for_shell(editor: EditorWidget) -> (RenderRoot, WidgetId) {
        let shell = ClayShellWidget::single_editor(editor);
        let editor_widget_id = shell.editor_widget_id();
        let mut render_root = RenderRoot::new(NewWidget::new(shell), |_| {}, render_root_options());

        assert!(render_root.has_widget(editor_widget_id));
        assert!(render_root.set_focus_fallback(Some(editor_widget_id)));
        assert!(render_root.focus_on(Some(editor_widget_id)));
        assert_eq!(render_root.focused_widget(), Some(editor_widget_id));

        (render_root, editor_widget_id)
    }

    fn with_shell_editor<R>(
        render_root: &mut RenderRoot,
        editor_widget_id: WidgetId,
        f: impl FnOnce(&mut EditorWidget) -> R,
    ) -> R {
        render_root.edit_widget(editor_widget_id, |mut widget| {
            let editor = widget
                .try_downcast::<EditorWidget>()
                .expect("shell editor child downcasts to EditorWidget");
            f(editor.widget)
        })
    }

    #[test]
    fn shell_observable_snapshot_captures_default_working_area() {
        let shell = ClayShellWidget::single_editor(EditorWidget::default());

        let observation = shell.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(observation.layout.layout_version, ShellLayoutVersion(1));
        assert_eq!(observation.layout.working_area_id, WorkingAreaId(1));
        assert_eq!(observation.layout.root_pane_id, PaneId(1));
        assert_eq!(observation.layout.active_pane_id, PaneId(1));
        assert_eq!(observation.layout.editor_component.id, ShellComponentId(1));
        assert_eq!(
            observation.layout.editor_component.kind,
            ShellComponentKind::Editor
        );
        assert_eq!(observation.layout.editor_component.pane_id, PaneId(1));
        assert!(matches!(
            observation.layout.pane_tree,
            PaneTreeObservation::Leaf { pane_id: PaneId(1) }
        ));
        assert_eq!(observation.layout.pane_count, 1);
        assert_eq!(observation.layout.split_count, 0);
        assert_eq!(observation.layout.slots.len(), 1);
        assert_eq!(observation.layout.slots[0].slot_id, PaneSlotId::Main);
        assert!(observation.layout.editor_region_non_empty);
        assert!(observation.editor_component_bound);
        assert!(observation.sdui_state_present);
        assert!(observation.status_present);
    }

    #[test]
    fn shell_root_registers_editor_child_and_focus_fallback() {
        let shell = ClayShellWidget::single_editor(EditorWidget::default());

        assert_eq!(
            shell.children_ids(),
            ChildrenIds::from_slice(&[shell.editor_widget_id()])
        );
        assert_eq!(shell.focus_fallback_widget_id(), shell.editor_widget_id());
    }

    #[test]
    fn shell_editor_text_input_remains_client_first() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(3);
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ))
        .with_edit_queue(queue);
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        render_root.handle_text_event(TextEvent::Ime(Ime::Commit("!".to_string())));

        let visible_text = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.visible_text_for_test()
        });
        assert_eq!(visible_text, "!server text");
        assert_eq!(
            receiver.try_recv().expect("local edit is queued"),
            ClientMessage::Edit {
                document_id: 7,
                client_id: 11,
                lease_id: Some(1),
                base_version: 3,
                behavior_version: 3,
                transaction_id: 1,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "!".to_string(),
                },
            }
        );
    }

    #[test]
    fn shell_editor_keyboard_routing_uses_installed_behavior_manifest() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(3);
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ))
        .with_edit_queue(queue);
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        render_root.handle_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key: Key::Named(NamedKey::Enter),
            code: Code::Enter,
            ..KeyboardEvent::default()
        }));

        let visible_text = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.visible_text_for_test()
        });
        assert_eq!(visible_text, "\nserver text");
        assert!(matches!(
            receiver.try_recv().expect("manifest-routed edit is queued"),
            ClientMessage::Edit {
                operation: EditOperation::Insert { byte_offset: 0, text },
                behavior_version: 3,
                ..
            } if text == "\n"
        ));
    }

    #[test]
    fn shell_editor_read_only_observer_blocks_local_edit_queue() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(11, &DocumentAccess::ReadOnly)
            .with_confirmed_version(3);
        let editor = EditorWidget::with_initial_state(initial_state(DocumentAccess::ReadOnly, 3))
            .with_edit_queue(queue);
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        render_root.handle_text_event(TextEvent::Ime(Ime::Commit("!".to_string())));

        let visible_text = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.visible_text_for_test()
        });
        assert_eq!(visible_text, "server text");
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn shell_routes_edit_ack_and_resync_to_editor() {
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ));
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            assert!(
                editor.apply_connection_event(ClientConnectionEvent::EditAck {
                    document_id: 7,
                    version: 4,
                    transaction_id: 1,
                })
            );
        });
        let status_after_ack = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.status_text()
        });
        assert_eq!(status_after_ack, "Clay — Connected — Editable — doc 7 — v4");

        with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            assert!(
                editor.apply_connection_event(ClientConnectionEvent::ResyncSnapshot(
                    ClientResyncSnapshot {
                        document_id: 7,
                        version: 9,
                        text: "server resync".to_string(),
                        access: DocumentAccess::ReadOnly,
                        lease_id: None,
                    },
                ))
            );
        });
        let (visible_text, status_after_resync) =
            with_shell_editor(&mut render_root, editor_widget_id, |editor| {
                (editor.visible_text_for_test(), editor.status_text())
            });
        assert_eq!(visible_text, "server resync");
        assert_eq!(
            status_after_resync,
            "Clay — Connected — Read-only Observer — doc 7 — v9"
        );
    }

    #[test]
    fn shell_routes_sdui_snapshots_to_editor_component() {
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ));
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            assert!(
                editor.apply_connection_event(ClientConnectionEvent::SduiSnapshot {
                    client_id: 11,
                    tree: sdui_tree("Ready"),
                })
            );
        });

        let visible_texts = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.sdui_visible_texts()
        });
        assert!(visible_texts.contains(&"Ready".to_string()));
    }

    #[test]
    fn shell_root_delegates_connection_events_to_editor_component() {
        let shell = ClayShellWidget::single_editor(EditorWidget::default());
        let layout = shell.working_area_layout();
        let observation = shell.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(layout.editor_component().kind, ShellComponentKind::Editor);
        assert_eq!(layout.editor_component().pane_id, layout.active_pane_id());
        assert_eq!(
            observation.layout.editor_component.pane_id,
            layout.editor_component().pane_id
        );
        assert!(observation.editor_component_bound);
    }

    #[test]
    fn shell_places_editor_child_in_main_slot_rect() {
        let slot_layout = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 240.0, 120.0, 320.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Bottom, 80.0, 40.0, 120.0).unwrap());
        let layout = WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout);
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);

        let editor_rect = shell.editor_component_rect_for_size(Size::new(900.0, 600.0));

        assert_eq!(editor_rect, Rect::new(240.0, 0.0, 900.0, 520.0));
        assert_eq!(
            shell.children_ids(),
            ChildrenIds::from_slice(&[shell.editor_widget_id()])
        );
    }

    #[test]
    fn shell_observable_snapshot_captures_split_and_slots() {
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::new(0.25).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(2),
        )
        .unwrap();
        let mut layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(2)).unwrap();
        layout
            .apply_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: PaneSplitTree::new(
                    PaneSplitNode::split(
                        SplitOrientation::Horizontal,
                        SplitRatio::new(0.25).unwrap(),
                        PaneSplitNode::leaf(PaneId(1)),
                        PaneSplitNode::leaf(PaneId(2)),
                    ),
                    PaneId(2),
                )
                .unwrap(),
                editor_pane_id: PaneId(2),
                pane_slots: vec![PaneSlotLayoutAssignment {
                    pane_id: PaneId(2),
                    layout: PaneSlotLayout::main_only().with_fixed_slot(
                        FixedSlotState::new(FixedSlotId::Left, 120.0, 80.0, 200.0).unwrap(),
                    ),
                }],
            })
            .unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);

        let snapshot = shell.observable_snapshot(Size::new(1000.0, 600.0));

        assert_eq!(snapshot.layout.layout_version, ShellLayoutVersion(2));
        assert_eq!(snapshot.layout.active_pane_id, PaneId(2));
        assert_eq!(snapshot.layout.editor_component.pane_id, PaneId(2));
        assert_eq!(snapshot.layout.pane_count, 2);
        assert_eq!(snapshot.layout.split_count, 1);
        assert!(matches!(
            snapshot.layout.pane_tree,
            PaneTreeObservation::Split { .. }
        ));
        assert!(snapshot.layout.slots.iter().any(|slot| {
            slot.pane_id == PaneId(2)
                && slot.slot_id == PaneSlotId::Left
                && slot.visible
                && slot.rect == Rect::new(250.0, 0.0, 370.0, 600.0)
        }));
        assert!(snapshot.layout.slots.iter().any(|slot| {
            slot.pane_id == PaneId(2)
                && slot.slot_id == PaneSlotId::Main
                && slot.rect == Rect::new(370.0, 0.0, 1000.0, 600.0)
        }));
    }

    #[test]
    fn shell_observation_does_not_expose_document_text_or_native_handles() {
        let shell = ClayShellWidget::single_editor(EditorWidget::default());

        let snapshot = shell.observable_snapshot(Size::new(900.0, 600.0));
        let debug = format!("{snapshot:?}");

        assert!(!debug.contains("hello from a document"));
        assert!(!debug.contains("WidgetId"));
        assert!(!debug.contains("Deno.core.ops"));
        assert!(!debug.contains("raw_css"));
    }

    #[test]
    fn shell_layout_update_rejects_stale_or_oversize_payload() {
        let mut shell = ClayShellWidget::single_editor(EditorWidget::default());

        assert!(matches!(
            shell.apply_layout_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(0),
                working_area_id: WorkingAreaId(1),
                pane_tree: PaneSplitTree::default(),
                editor_pane_id: PaneId(1),
                pane_slots: Vec::new(),
            }),
            Err(WorkingAreaLayoutUpdateError::StaleVersion { .. })
        ));

        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap();
        assert!(matches!(
            shell.apply_layout_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: tree,
                editor_pane_id: PaneId(1),
                pane_slots: vec![
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(1),
                        layout: PaneSlotLayout::main_only(),
                    },
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(2),
                        layout: PaneSlotLayout::main_only(),
                    },
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(3),
                        layout: PaneSlotLayout::main_only(),
                    },
                ],
            }),
            Err(WorkingAreaLayoutUpdateError::TooManyPaneSlotLayouts { .. })
        ));
    }

    #[test]
    fn pane_split_tree_layout_does_not_mutate_children() {
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap();
        let layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);

        let child_ids_before: Vec<_> = shell.children_ids().iter().copied().collect();
        let editor_rect = shell.editor_component_rect_for_size(Size::new(1000.0, 600.0));
        let child_ids_after: Vec<_> = shell.children_ids().iter().copied().collect();

        assert_eq!(editor_rect, Rect::new(0.0, 0.0, 500.0, 600.0));
        assert_eq!(child_ids_after, child_ids_before);
        assert_eq!(child_ids_after, vec![shell.editor_widget_id()]);
    }
}
