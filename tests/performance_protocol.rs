use std::time::{Duration, Instant};

use clay::{
    client::ClientEditQueue,
    editor::{EditorCommand, EditorEditEvent, EditorSurface},
    perf::{
        baselines::representative_sdui_tree,
        budgets::{
            BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, CLIENT_EDIT_PAYLOAD_BUDGET_BYTES,
            EDIT_ACK_PAYLOAD_BUDGET_BYTES, SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
            SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
        },
        metrics::{PerfConfig, install_global_recorder},
    },
    protocol::{
        BehaviorManifest, ClientMessage, DocumentAccess, EditOperation, ServerMessage,
        codec::{Codec, CodecError},
    },
};

const FRAME_PREFIX_BYTES: usize = 4;

fn edit_event(byte_offset: u64, text: &str) -> EditorEditEvent {
    EditorEditEvent {
        document_id: 7,
        base_version: 1,
        behavior_version: 1,
        operation: EditOperation::Insert {
            byte_offset,
            text: text.to_string(),
        },
    }
}

fn payload_len(frame: &[u8]) -> usize {
    frame.len().saturating_sub(FRAME_PREFIX_BYTES)
}

#[test]
fn ordinary_edit_updates_shadow_before_ack() {
    let mut surface = EditorSurface::default();
    surface.load_snapshot(
        7,
        1,
        "abc".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    surface.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
    surface.command(EditorCommand::DocumentEnd);

    let (queue, _receiver) = ClientEditQueue::bounded(1);
    let queue = queue
        .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(1);

    let first = surface.command_with_event(EditorCommand::Insert("!"));
    assert!(first.changed);
    queue
        .enqueue_edit_event(first.edit_event.expect("edit event"), 1)
        .expect("first edit should fit queue");

    let second = surface.command_with_event(EditorCommand::Insert("?"));
    assert!(second.changed);
    assert_eq!(surface.visible_text(), "abc!?");

    let second_send = queue.enqueue_edit_event(second.edit_event.expect("edit event"), 2);
    assert!(matches!(
        second_send,
        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
    ));
    assert_eq!(surface.visible_text(), "abc!?");
}

#[test]
fn client_edit_queue_reports_depth_without_blocking_input() {
    let (queue, _receiver) = ClientEditQueue::bounded(1);
    let queue = queue
        .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(1);

    queue
        .enqueue_edit_event(edit_event(0, "x"), 1)
        .expect("first edit should enqueue");

    let started = Instant::now();
    let second = queue.enqueue_edit_event(edit_event(1, "y"), 2);
    let elapsed = started.elapsed();

    assert!(matches!(
        second,
        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
    ));
    assert!(
        elapsed < Duration::from_millis(50),
        "full queue must fail fast via try_send; observed {elapsed:?}"
    );
    assert_eq!(queue.sync_snapshot().pending.len(), 1);
}

#[test]
fn representative_protocol_payloads_fit_phase14_budgets() {
    let codec = Codec::default();

    let client_edit = ClientMessage::Edit {
        document_id: 7,
        client_id: 11,
        lease_id: Some(1),
        base_version: 1,
        behavior_version: 3,
        transaction_id: 99,
        operation: EditOperation::Insert {
            byte_offset: 128,
            text: "x".repeat(96),
        },
    };
    let client_edit_payload = payload_len(&codec.encode_client_message(&client_edit).unwrap());
    assert!(
        client_edit_payload <= CLIENT_EDIT_PAYLOAD_BUDGET_BYTES,
        "client edit payload {client_edit_payload} exceeds budget {CLIENT_EDIT_PAYLOAD_BUDGET_BYTES}"
    );

    let edit_ack = ServerMessage::EditAck {
        document_id: 7,
        confirmed_version: 2,
        transaction_id: 99,
    };
    let edit_ack_payload = payload_len(&codec.encode_server_message(&edit_ack).unwrap());
    assert!(
        edit_ack_payload <= EDIT_ACK_PAYLOAD_BUDGET_BYTES,
        "edit ack payload {edit_ack_payload} exceeds budget {EDIT_ACK_PAYLOAD_BUDGET_BYTES}"
    );

    let manifest = ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(3));
    let manifest_payload = payload_len(&codec.encode_server_message(&manifest).unwrap());
    assert!(
        manifest_payload <= BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES,
        "behavior manifest payload {manifest_payload} exceeds budget {BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES}"
    );

    let sdui_snapshot = ServerMessage::SduiSnapshot {
        client_id: 11,
        tree: representative_sdui_tree(),
    };
    let sdui_snapshot_payload = payload_len(&codec.encode_server_message(&sdui_snapshot).unwrap());
    assert!(
        sdui_snapshot_payload <= SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
        "SDUI snapshot payload {sdui_snapshot_payload} exceeds budget {SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES}"
    );

    let sdui_update = ServerMessage::SduiUpdate {
        update: clay::perf::baselines::representative_panel_update(),
    };
    let sdui_update_payload = payload_len(&codec.encode_server_message(&sdui_update).unwrap());
    assert!(
        sdui_update_payload <= SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
        "SDUI update payload {sdui_update_payload} exceeds budget {SDUI_UPDATE_PAYLOAD_BUDGET_BYTES}"
    );
}

#[test]
fn oversized_and_invalid_frames_still_rejected_with_metrics_enabled() {
    install_global_recorder(PerfConfig::enabled());

    let codec = Codec::new(32);
    let oversized = ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1));
    assert!(matches!(
        codec.encode_server_message(&oversized),
        Err(CodecError::FrameTooLarge { max: 32, .. })
    ));

    let invalid = [0, 0, 0, 4, 0xde, 0xad, 0xbe, 0xef];
    assert!(matches!(
        codec.decode_client_message(&invalid),
        Err(CodecError::Deserialize(_))
    ));

    let mut oversize_declared = vec![];
    oversize_declared.extend_from_slice(&64_u32.to_be_bytes());
    oversize_declared.extend_from_slice(&[0; 64]);
    assert!(matches!(
        codec.decode_server_message(&oversize_declared),
        Err(CodecError::FrameTooLarge { len: 64, max: 32 })
    ));
}
