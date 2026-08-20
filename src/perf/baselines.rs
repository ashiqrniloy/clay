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

/// Measure the production SDUI left-slot decision across pane widths and UI
/// typography. The returned flags keep Criterion from eliding the real
/// `sidebar_geometry` and `editor_region_for_document` work: bit 0 means the
/// sidebar is present, bit 1 means the editor is offset by it, and bit 2 means
/// the editor has usable width.
#[doc(hidden)]
pub fn responsive_layout_work(width: f64, ui_size: f32) -> usize {
    let mut state = SduiNativeState::empty();
    state.apply_snapshot(representative_sdui_tree());
    let default = ActiveTypography::default();
    let typography = ActiveTypography {
        revision: 1,
        ui: crate::protocol::FontProfile {
            size: ui_size,
            ..default.ui.clone()
        },
        ..default
    };
    state.set_typography(
        crate::editor::typography::TypographyRegistry::from_active_typography(typography)
            .expect("benchmark typography fixture is valid"),
    );
    let size = masonry::kurbo::Size::new(width, 600.0);
    let sidebar = state.sidebar_geometry(size);
    let editor = crate::masonry_sdui::editor_region_for_document(size, &state, 7);
    let flags = usize::from(sidebar.is_some())
        | (usize::from(editor.x0 > 0.0) << 1)
        | (usize::from(editor.width() > 0.0) << 2);
    std::hint::black_box(flags)
}

/// Geometry work a centered Command Centre open/theme update performs: the
/// full-window scrim rect plus the centered surface rect and one rect per
/// hosted overlay. Pure rect math over window bounds — no document text,
/// serialization, IPC, or paint work — so the count is O(overlay_count) and
/// independent of document size.
pub fn centered_overlay_geometry_work(overlay_count: usize) -> usize {
    let window = Rect::new(0.0, 0.0, 900.0, 600.0);
    let centered = crate::shell::package_ui::centered_rect(window, 640.0, 220.0);
    // One scrim fill rect + one centered surface rect + one rect per overlay.
    1 + 1 + overlay_count * usize::from(centered.width() > 0.0)
}

// ── Plan 087: focused completion/menu regression baselines ────────────────
//
// These helpers are benchmark-only projections of the real bounded menu,
// fuzzy-filter, and caret geometry paths. Their wall-clock results remain
// local/advisory; hard CI coverage lives in the row/geometry/source guards.

fn completion_session_for_benchmark(item_count: usize) -> crate::shell::TransientMenuSession {
    use crate::protocol::{
        CompletionItem, CompletionProvenance, CompletionReplacementRange, CompletionResultSet,
        CompletionStatus,
    };

    let provenance = CompletionProvenance::builtin_core();
    let items = (0..item_count.min(crate::perf::budgets::TRANSIENT_MENU_MAX_ITEMS))
        .map(|index| {
            CompletionItem::new(
                format!("completion-{index}"),
                format!("Completion item {index}"),
                provenance.clone(),
            )
        })
        .collect();
    let result = CompletionResultSet {
        request_id: 1,
        client_id: 1,
        document_id: 7,
        document_version: 1,
        behavior_version: 0,
        provider_generation: 1,
        replacement_range: CompletionReplacementRange::new(0, 0),
        status: CompletionStatus::Ok,
        items,
        provenance,
    };
    crate::shell::completion_result_to_menu_session(&result)
        .with_completion_anchor(Rect::new(320.0, 280.0, 321.0, 300.0))
}

/// Construct and project one bounded completion menu for Criterion.
#[doc(hidden)]
pub fn completion_open_projection_work(item_count: usize) -> usize {
    let session = completion_session_for_benchmark(item_count);
    let overlay = crate::shell::package_ui::TransientPackageOverlay::from_menu_session(&session);
    let semantic_items = overlay
        .menu_a11y
        .as_ref()
        .map_or(0, |menu| menu.items.len());
    std::hint::black_box(semantic_items + overlay.component.children.len())
}

/// Project one selected completion result through the production menu and
/// accessibility model. Selection changes reuse the bounded rows; no provider,
/// document, IPC, or package JavaScript work is included.
#[doc(hidden)]
pub fn completion_selection_work(item_count: usize, selected_index: usize) -> usize {
    let session = completion_session_for_benchmark(item_count).with_selected_index(selected_index);
    let overlay = crate::shell::package_ui::TransientPackageOverlay::from_menu_session(&session);
    let selected = overlay.menu_a11y.as_ref().map_or(0, |menu| {
        menu.items.iter().filter(|item| item.selected).count()
    });
    std::hint::black_box(selected + overlay.component.children.len())
}

fn command_centre_session_for_benchmark(item_count: usize) -> crate::shell::TransientMenuSession {
    use crate::shell::transient_menu::{
        TransientMenuAction, TransientMenuItem, TransientMenuOrigin, TransientMenuSession,
        TransientMenuSessionId,
    };

    let items = (0..item_count.min(crate::perf::budgets::TRANSIENT_MENU_MAX_ITEMS))
        .map(|index| {
            TransientMenuItem::new(
                format!("command-{index}"),
                format!("Command Centre action {index}"),
                TransientMenuAction::new(format!("shell.command{index}")),
            )
        })
        .collect();
    TransientMenuSession::new(TransientMenuSessionId(2), "Control Center")
        .with_origin(TransientMenuOrigin::Centered)
        .with_items(items)
}

/// Project one bounded Command Centre catalogue through the production menu
/// and accessibility model. Filtering uses the same matcher as completion;
/// this helper measures the open/projection side without server or filesystem
/// authority.
#[doc(hidden)]
pub fn command_centre_open_projection_work(item_count: usize) -> usize {
    let session = command_centre_session_for_benchmark(item_count);
    let overlay = crate::shell::package_ui::TransientPackageOverlay::from_menu_session(&session);
    let semantic_items = overlay
        .menu_a11y
        .as_ref()
        .map_or(0, |menu| menu.items.len());
    let result_count = overlay
        .menu_a11y
        .as_ref()
        .and_then(|menu| menu.result_count.as_deref())
        .map_or(0, str::len);
    std::hint::black_box(semantic_items + result_count + overlay.component.children.len())
}

/// Score one bounded transient-menu candidate set with the production fuzzy
/// matcher. Command Centre filtering uses the same matcher and caps.
#[doc(hidden)]
pub fn transient_menu_filter_work(candidate_count: usize, query: &str) -> usize {
    let matches = (0..candidate_count.min(crate::perf::budgets::TRANSIENT_MENU_MAX_ITEMS))
        .filter(|index| {
            let label = format!("Command {index:03} split pane");
            let command_id = format!("shell.command{index}");
            crate::shell::fuzzy::fuzzy_score_fields(query, [label.as_str(), command_id.as_str()])
                .is_some()
        })
        .count();
    std::hint::black_box(matches)
}

/// Resolve one completion popup rect through the production caret geometry
/// helper. The returned bit pattern keeps Criterion from eliding the work.
#[doc(hidden)]
pub fn completion_layout_work(item_count: usize, caret_y: f64) -> usize {
    let main = Rect::new(0.0, 0.0, 900.0, 600.0);
    let caret = Rect::new(320.0, caret_y, 321.0, caret_y + 20.0);
    let typography = crate::editor::typography::TypographyRegistry::default();
    let ui_theme = crate::shell::theme::ResolvedUiTheme::default();
    let rect = crate::shell::package_ui::completion_overlay_rect(
        main,
        Some(caret),
        item_count,
        &typography,
        &ui_theme,
    );
    std::hint::black_box(rect.width().to_bits() as usize ^ rect.height().to_bits() as usize)
}

/// Reusable optimized benchmark fixture for retained accessibility updates.
/// The initial tree is built outside Criterion's timed closure; `update`
/// changes labels while preserving the owner/client-derived virtual IDs.
#[doc(hidden)]
pub struct AccessibilityTreeBench {
    root: masonry::app::RenderRoot,
    shell_id: masonry::core::WidgetId,
    tab_count: usize,
    revision: usize,
}

impl AccessibilityTreeBench {
    /// Build a bounded shell tree with stable tab accessibility IDs.
    #[doc(hidden)]
    pub fn new(tab_count: usize) -> Self {
        use masonry::app::{RenderRoot, RenderRootOptions, WindowSizePolicy};
        use masonry::core::{NewWidget, WindowEvent};
        use masonry::dpi::PhysicalSize;

        let tab_count = tab_count.clamp(2, crate::perf::budgets::MAX_ACTIVE_CONNECTIONS);
        let root_widget = NewWidget::new(crate::masonry_shell::ClayShellWidget::single_editor(
            0,
            crate::masonry_editor::EditorWidget::default(),
        ));
        let shell_id = root_widget.id();
        let mut root = RenderRoot::new(
            root_widget,
            |_| {},
            RenderRootOptions {
                default_properties: masonry::theme::default_property_set().into(),
                use_system_fonts: false,
                size_policy: WindowSizePolicy::User,
                size: PhysicalSize::new(900, 600),
                scale_factor: 1.0,
                test_font: None,
            },
        );
        root.handle_window_event(WindowEvent::EnableAccessTree);
        let _ = root.redraw();
        root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget
                .try_downcast::<crate::masonry_shell::ClayShellWidget>()
                .expect("benchmark root is ClayShellWidget");
            for client_id in 1..tab_count as u64 {
                shell.widget.install_tab(
                    &mut shell.ctx,
                    client_id,
                    crate::masonry_shell::TabChrome::single_editor(
                        crate::masonry_editor::EditorWidget::default(),
                        false,
                    ),
                );
            }
            shell
                .widget
                .set_tab_cards(&mut shell.ctx, benchmark_tab_cards(tab_count, 0));
        });
        let _ = root.redraw();
        Self {
            root,
            shell_id,
            tab_count,
            revision: 0,
        }
    }

    /// Apply one stable-ID label update and return emitted accessibility-node
    /// count. This is the timed operation; setup and first tree construction
    /// stay outside the benchmark closure.
    #[doc(hidden)]
    pub fn update(&mut self) -> usize {
        self.revision = self.revision.wrapping_add(1);
        let revision = self.revision;
        self.root.edit_widget(self.shell_id, |mut widget| {
            let mut shell = widget
                .try_downcast::<crate::masonry_shell::ClayShellWidget>()
                .expect("benchmark root is ClayShellWidget");
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                benchmark_tab_cards(self.tab_count, revision),
            );
        });
        let (_, update) = self.root.redraw();
        std::hint::black_box(update.map_or(0, |tree| tree.nodes.len()))
    }
}

fn benchmark_tab_cards(tab_count: usize, revision: usize) -> Vec<crate::masonry_shell::TabCard> {
    (0..tab_count)
        .map(|client_id| crate::masonry_shell::TabCard {
            client_id: client_id as u64,
            name: format!("workspace-{revision}-{client_id}"),
            closable: true,
        })
        .collect()
}

/// Build and update one retained accessibility tree when a caller needs a
/// single-shot smoke value rather than a reusable Criterion fixture.
#[doc(hidden)]
pub fn accessibility_tree_update_work(tab_count: usize) -> usize {
    let mut fixture = AccessibilityTreeBench::new(tab_count);
    fixture.update()
}
