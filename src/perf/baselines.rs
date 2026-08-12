use crate::{
    client::ClientEditQueue,
    editor::{EditorCommand, EditorEditEvent, EditorSurface},
    masonry_sdui::SduiNativeState,
    perf::fixtures::{FixtureKind, FixtureSpec, generate_fixture},
    protocol::{
        ActiveTypography, BehaviorManifest, ClientMessage, DocumentAccess, EditOperation,
        PROTOCOL_VERSION, SduiActionIntent, SduiActionSource, SduiEditorBinding, SduiFlexDirection,
        SduiListItem, SduiNode, SduiNodeId, SduiNodeKind, SduiTree, SduiTreeOperation,
        SduiTreeUpdate, ServerMessage, codec::Codec,
    },
};

pub const SMALL_BENCH_BYTES: usize = 64 * 1024;
pub const LARGE_BENCH_BYTES: usize = 1024 * 1024;

pub fn fixture_text(kind: FixtureKind, size_bytes: usize) -> String {
    let mut bytes = Vec::with_capacity(size_bytes);
    let spec = FixtureSpec::new(kind, size_bytes);
    generate_fixture(&spec, &mut bytes).expect("in-memory fixture generation should succeed");
    String::from_utf8(bytes).expect("fixture generator emits valid UTF-8")
}

pub fn editor_surface_with_fixture(size_bytes: usize) -> EditorSurface {
    let mut surface = EditorSurface::default();
    surface.load_snapshot(
        7,
        1,
        fixture_text(FixtureKind::ManyShortLines, size_bytes),
        DocumentAccess::Editable { lease_id: 1 },
    );
    surface.update_visible_line_count_for_height(720.0);
    surface
}

pub fn editor_visible_text_len(size_bytes: usize) -> usize {
    editor_surface_with_fixture(size_bytes).visible_text().len()
}

pub fn editor_insert_at_end(size_bytes: usize) -> usize {
    let mut surface = editor_surface_with_fixture(size_bytes);
    surface.command(EditorCommand::DocumentEnd);
    surface
        .command_with_event(EditorCommand::Insert("x"))
        .edit_event
        .map_or(0, |event| match event.operation {
            EditOperation::Insert { byte_offset, text } => byte_offset as usize + text.len(),
            _ => 0,
        })
}

pub fn editor_scroll_visible_text_len(size_bytes: usize) -> usize {
    let mut surface = editor_surface_with_fixture(size_bytes);
    let _ = surface.scroll_lines(512);
    surface.visible_text().len()
}

pub fn editor_scroll_window_signature(size_bytes: usize, delta_lines: isize) -> usize {
    let mut surface = editor_surface_with_fixture(size_bytes);
    let _ = surface.scroll_lines(delta_lines);
    let visible = surface.visible_text();
    let line_count = visible.lines().count();
    visible.len() ^ line_count
}

pub fn editor_resize_viewport_visible_text_len(size_bytes: usize, height: f64) -> usize {
    let mut surface = editor_surface_with_fixture(size_bytes);
    let _ = surface.update_visible_line_count_for_height(height);
    surface.visible_text().len()
}

pub fn editor_typography_viewport_visible_text_len(size_bytes: usize, font_size: f32) -> usize {
    let mut surface = editor_surface_with_fixture(size_bytes);
    let mut typography = ActiveTypography {
        revision: 1,
        ..ActiveTypography::default()
    };
    typography.monospace.size = font_size;
    typography.proportional.size = font_size;
    let _ = surface.set_typography(typography);
    let _ = surface.update_visible_line_count_for_height(1080.0);
    surface.visible_text().len()
}

pub fn editor_render_adjacent_update(size_bytes: usize) -> usize {
    let mut surface = editor_surface_with_fixture(size_bytes);
    let _ = surface.command(EditorCommand::MoveDown);
    let _ = surface.command(EditorCommand::MoveUp);
    let _ = surface.command(EditorCommand::SelectRight);
    let _ = surface.command(EditorCommand::MoveRight);
    surface.visible_text().len()
}

pub fn client_edit_message(text_bytes: usize) -> ClientMessage {
    ClientMessage::Edit {
        document_id: 7,
        client_id: 11,
        lease_id: Some(1),
        base_version: 1,
        behavior_version: 1,
        transaction_id: 99,
        operation: EditOperation::Insert {
            byte_offset: 128,
            text: "x".repeat(text_bytes),
        },
    }
}

pub fn encode_decode_client_edit(text_bytes: usize) -> usize {
    let codec = Codec::default();
    let frame = codec
        .encode_client_message(&client_edit_message(text_bytes))
        .expect("representative client edit should encode");
    let decoded = codec
        .decode_client_message(&frame)
        .expect("representative client edit should decode");
    match decoded {
        ClientMessage::Edit {
            operation: EditOperation::Insert { text, .. },
            ..
        } => text.len(),
        _ => 0,
    }
}

pub fn encode_decode_initial_document(size_bytes: usize) -> usize {
    let codec =
        Codec::new((size_bytes + 64 * 1024).max(crate::protocol::codec::DEFAULT_MAX_FRAME_SIZE));
    let message = ServerMessage::InitialDocument {
        document_id: 7,
        version: 1,
        text: fixture_text(FixtureKind::MixedUnicode, size_bytes),
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        workspace_root: "/tmp/root".to_string(),
    };
    let frame = codec
        .encode_server_message(&message)
        .expect("representative initial document should encode");
    let decoded = codec
        .decode_server_message(&frame)
        .expect("representative initial document should decode");
    match decoded {
        ServerMessage::InitialDocument { text, .. } => text.len(),
        _ => 0,
    }
}

pub fn client_enqueue_edit_batch(edit_count: usize) -> usize {
    let capacity = edit_count.max(1);
    let (queue, mut receiver) = ClientEditQueue::bounded(capacity + 1);
    let queue = queue
        .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(1);

    let mut enqueued = 0;
    for transaction_id in 1..=edit_count as u64 {
        let event = EditorEditEvent {
            document_id: 7,
            base_version: 1,
            behavior_version: 1,
            operation: EditOperation::Insert {
                byte_offset: 0,
                text: "x".to_string(),
            },
        };
        if queue.enqueue_edit_event(event, transaction_id).is_ok() {
            enqueued += 1;
        }
    }

    let mut drained = 0;
    while receiver.try_recv().is_ok() {
        drained += 1;
    }
    debug_assert_eq!(drained, enqueued);
    enqueued
}

#[cfg(any(unix, windows))]
pub fn server_apply_edit_ack_count(edit_count: usize) -> usize {
    let mut document = crate::server::document::DocumentState::new(
        7,
        fixture_text(FixtureKind::ManyShortLines, SMALL_BENCH_BYTES),
        DocumentAccess::Editable { lease_id: 1 },
    );
    let access = document.acquire_access(11);
    let mut ack_count = 0;
    for transaction_id in 1..=edit_count as u64 {
        let response = document.apply_edit(
            7,
            11,
            access.lease_id(),
            document.version(),
            transaction_id,
            EditOperation::Insert {
                byte_offset: 0,
                text: "x".to_string(),
            },
        );
        if matches!(response, ServerMessage::EditAck { .. }) {
            ack_count += 1;
        }
    }
    ack_count
}

#[cfg(any(unix, windows))]
pub fn server_rejects_stale_edit_count(edit_count: usize) -> usize {
    let mut document = crate::server::document::DocumentState::new(
        7,
        fixture_text(FixtureKind::ManyShortLines, SMALL_BENCH_BYTES),
        DocumentAccess::Editable { lease_id: 1 },
    );
    let access = document.acquire_access(11);

    let _ = document.apply_edit(
        7,
        11,
        access.lease_id(),
        document.version(),
        1,
        EditOperation::Insert {
            byte_offset: 0,
            text: "x".to_string(),
        },
    );

    let stale_base_version = document.version().saturating_sub(1);
    let mut rejection_count = 0;
    for transaction_id in 2..=(edit_count as u64 + 1) {
        let response = document.apply_edit(
            7,
            11,
            access.lease_id(),
            stale_base_version,
            transaction_id,
            EditOperation::Insert {
                byte_offset: 0,
                text: "y".to_string(),
            },
        );
        if matches!(
            response,
            ServerMessage::EditRejected {
                reason: crate::protocol::EditRejection::StaleVersion { .. },
                ..
            }
        ) {
            rejection_count += 1;
        }
    }
    rejection_count
}

pub fn behavior_manifest_route_count() -> usize {
    BehaviorManifest::minimal_text_editing(1).commands.len()
}

pub fn representative_sdui_tree() -> SduiTree {
    let root_id = SduiNodeId(1);
    let sidebar_id = SduiNodeId(2);
    let stack_id = SduiNodeId(3);
    let label_id = SduiNodeId(4);
    let button_id = SduiNodeId(5);
    let list_id = SduiNodeId(6);
    let editor_id = SduiNodeId(7);

    SduiTree {
        ui_version: 1,
        root_id,
        nodes: vec![
            SduiNode::new(
                root_id,
                SduiNodeKind::Flex {
                    direction: SduiFlexDirection::Row,
                    children: vec![sidebar_id, editor_id],
                },
            ),
            SduiNode::new(
                sidebar_id,
                SduiNodeKind::Panel {
                    title: "Workspace".to_string(),
                    children: vec![stack_id],
                },
            ),
            SduiNode::new(
                stack_id,
                SduiNodeKind::Stack {
                    children: vec![label_id, button_id, list_id],
                },
            ),
            SduiNode::new(
                label_id,
                SduiNodeKind::Label {
                    text: "Document 7 · version 3".to_string(),
                },
            ),
            SduiNode::new(
                button_id,
                SduiNodeKind::Button {
                    label: "Refresh".to_string(),
                    action: SduiActionIntent::command(
                        "workspace.refresh",
                        SduiActionSource::Button { node_id: button_id },
                    ),
                },
            ),
            SduiNode::new(
                list_id,
                SduiNodeKind::List {
                    items: vec![SduiListItem {
                        id: "active-document".to_string(),
                        label: "Document 7".to_string(),
                        detail: Some("Server-generated editor view".to_string()),
                        action: Some(SduiActionIntent::command(
                            "document.open_recent",
                            SduiActionSource::ListItem {
                                node_id: list_id,
                                item_id: "active-document".to_string(),
                            },
                        )),
                    }],
                },
            ),
            SduiNode::new(
                editor_id,
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

pub fn representative_panel_update() -> SduiTreeUpdate {
    SduiTreeUpdate {
        base_ui_version: 1,
        new_ui_version: 2,
        operations: vec![SduiTreeOperation::ReplaceNode {
            node: SduiNode::new(
                SduiNodeId(4),
                SduiNodeKind::Label {
                    text: "Document 7 · version 4".to_string(),
                },
            ),
        }],
    }
}

pub fn apply_sdui_snapshot_and_update() -> usize {
    let mut state = SduiNativeState::empty();
    state.apply_snapshot(representative_sdui_tree());
    let update_applied = state.apply_update(representative_panel_update());
    state.visible_texts().len() + usize::from(update_applied)
}

pub fn encode_decode_sdui_snapshot() -> usize {
    let codec = Codec::default();
    let message = ServerMessage::SduiSnapshot {
        client_id: 11,
        tree: representative_sdui_tree(),
    };
    let frame = codec
        .encode_server_message(&message)
        .expect("representative SDUI snapshot should encode");
    let decoded = codec
        .decode_server_message(&frame)
        .expect("representative SDUI snapshot should decode");
    match decoded {
        ServerMessage::SduiSnapshot { tree, .. } => tree.nodes.len(),
        _ => 0,
    }
}

/// Phase 24.1: worst-case server-owned transient-menu snapshot (max items ×
/// max label/detail/accessibility/query strings) must encode/decode and stay
/// far under the 1 MiB frame cap. The DTO clamps at construction, so the
/// worst case is bounded by `TRANSIENT_MENU_MAX_*`; this asserts the wire
/// size stays small enough that per-keystroke snapshot pushes on local IPC
/// are negligible. Returns the encoded frame length in bytes.
pub fn encode_decode_max_transient_menu_snapshot() -> usize {
    use crate::protocol::{
        TransientMenuFocusPolicyData, TransientMenuItemData, TransientMenuOriginData,
        TransientMenuSnapshotData, TransientMenuStatusData,
    };
    let max_string = "x".repeat(
        crate::perf::budgets::TRANSIENT_MENU_MAX_LABEL_CHARS
            .max(crate::perf::budgets::TRANSIENT_MENU_MAX_DETAIL_CHARS),
    );
    let items = (0..crate::perf::budgets::TRANSIENT_MENU_MAX_ITEMS)
        .map(|i| {
            TransientMenuItemData::new(
                format!("item-{i}"),
                max_string.clone(),
                Some(max_string.clone()),
                max_string.clone(),
            )
        })
        .collect();
    let snapshot = TransientMenuSnapshotData::new(
        1 << 63 | 1,
        max_string.clone(),
        max_string.clone(),
        items,
        0,
        TransientMenuStatusData::Active,
        TransientMenuFocusPolicyData::Modal,
        TransientMenuOriginData::CommandPalette,
    );
    let codec = Codec::default();
    let message = ServerMessage::TransientMenuSnapshot(Box::new(snapshot));
    let frame = codec
        .encode_server_message(&message)
        .expect("max transient menu snapshot should encode");
    assert!(
        frame.len() < crate::protocol::codec::DEFAULT_MAX_FRAME_SIZE,
        "max snapshot frame {} bytes exceeds the {} byte cap",
        frame.len(),
        crate::protocol::codec::DEFAULT_MAX_FRAME_SIZE
    );
    let decoded = codec
        .decode_server_message(&frame)
        .expect("max transient menu snapshot should decode");
    match decoded {
        ServerMessage::TransientMenuSnapshot(snapshot) => snapshot.items.len(),
        _ => 0,
    }
}

pub fn protocol_hello_roundtrip() -> u32 {
    let codec = Codec::default();
    let message = ClientMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        client_name: "clay-bench".to_string(),
    };
    let frame = codec
        .encode_client_message(&message)
        .expect("hello should encode");
    match codec
        .decode_client_message(&frame)
        .expect("hello should decode")
    {
        ClientMessage::Hello {
            protocol_version, ..
        } => protocol_version,
        _ => 0,
    }
}

// ── Phase 22.6 (plan 077 task 5): window-model geometry baselines ──
//
// Advisory pane-paint / tab-switch baselines. The shell's paint pass
// (dividers, fixed-slot chrome, focus ring) and a tab switch's layout pass
// (same geometry for the newly active tab) are pure geometry math over the
// pane tree, so the benchable proxy is the chrome piece count and the time
// to compute it. Editor-surface paint is viewport-bounded and separately
// benched (`editor_baselines`). Results feed the advisory
// `PANE_PAINT_P95_BUDGET_MS` / `TAB_SWITCH_P95_BUDGET_MS` constants
// (docs/development/performance.md, Phase 22.6 section).

use crate::shell::layout::{
    PaneId, PaneSplitTree, WorkingAreaId, WorkingAreaLayout, WorkingAreaLayoutUpdate,
};
use masonry::kurbo::Rect;

/// A balanced `pane_count`-leaf split tree (active pane 1).
pub(crate) fn pane_split_tree_with(pane_count: usize) -> PaneSplitTree {
    let mut tree = PaneSplitTree::single_leaf(PaneId(1));
    for _ in 1..pane_count {
        tree = tree
            .add_equal_pane()
            .expect("adding a pane to a valid tree succeeds");
    }
    tree
}

/// A working-area layout with `pane_count` panes and no fixed slots.
pub(crate) fn working_area_layout_with(pane_count: usize) -> WorkingAreaLayout {
    let mut layout = WorkingAreaLayout::single_editor();
    if pane_count > 1 {
        layout
            .apply_update(WorkingAreaLayoutUpdate {
                base_version: layout.version(),
                working_area_id: WorkingAreaId(1),
                pane_tree: pane_split_tree_with(pane_count),
                editor_pane_id: PaneId(1),
                pane_slots: Vec::new(),
            })
            .expect("valid pane tree update");
    }
    layout
}

/// Chrome pieces the shell paints for an N-pane window: split dividers
/// (N-1), fixed-slot handles (none in the default layout), and the focus
/// ring (1 when N > 1). Linear in pane count: 0, N, N for 1, 2+, N panes.
pub fn pane_chrome_piece_count(pane_count: usize) -> usize {
    let layout = working_area_layout_with(pane_count);
    let area = Rect::new(0.0, 0.0, 1200.0, 800.0);
    let dividers = layout.pane_tree().divider_rects(area).len();
    let slots = layout
        .pane_slot_geometry(layout.active_pane_id(), area)
        .map_or(0, |geometry| geometry.fixed_slots.len());
    // The shell paints the focus ring only while more than one pane exists.
    let focus = if pane_count > 1 {
        usize::from(layout.focused_pane_rect(area).is_some())
    } else {
        0
    };
    dividers + slots + focus
}

/// Geometry work a tab switch triggers on the newly active tab's layout
/// pass: the same chrome pieces as a paint pass plus the editor component
/// rect. No document text, serialization, or IPC is involved.
pub fn tab_switch_geometry_work(pane_count: usize) -> usize {
    let layout = working_area_layout_with(pane_count);
    let area = Rect::new(0.0, 0.0, 1200.0, 800.0);
    let dividers = layout.pane_tree().divider_rects(area).len();
    let slots = layout
        .pane_slot_geometry(layout.active_pane_id(), area)
        .map_or(0, |geometry| geometry.fixed_slots.len());
    let focus = if pane_count > 1 {
        usize::from(layout.focused_pane_rect(area).is_some())
    } else {
        0
    };
    let editor = usize::from(layout.editor_component_rect(area).width() > 0.0);
    dividers + slots + focus + editor
}
