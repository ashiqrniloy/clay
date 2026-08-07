//! Phase 19 runtime-generation snapshot protocol and fan-out coverage.

use clay::protocol::{
    ActiveTheme, ActiveTypography, BehaviorManifest, ClientMessage, DocumentRuntimeRenderState,
    PackageUiSnapshot, RuntimeStateSnapshot, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
    ServerMessage, codec::Codec,
};

fn valid_snapshot(generation: u64, client_id: u64) -> RuntimeStateSnapshot {
    let snapshot = RuntimeStateSnapshot {
        runtime_generation_id: generation,
        client_id,
        behavior: BehaviorManifest::minimal_text_editing(generation),
        active_theme: ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: ActiveTypography::default(),
        sdui_tree: SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![SduiNode::new(
                SduiNodeId(1),
                SduiNodeKind::Label {
                    text: "runtime".to_string(),
                },
            )],
        },
        package_ui: PackageUiSnapshot {
            version: generation,
        },
        documents: vec![DocumentRuntimeRenderState {
            document_id: 1,
            document_version: 1,
            reset_decorations: true,
            reset_diagnostics: true,
            initial_decorations: None,
            initial_diagnostics: None,
            behavior_manifest: None,
        }],
        diagnostics: Vec::new(),
    };
    snapshot.validate().expect("fixture snapshot");
    snapshot
}

#[test]
fn runtime_state_snapshot_round_trips_with_generation_and_bounded_payload() {
    let codec = Codec::default();
    let snapshot = valid_snapshot(2, 7);
    let message = ServerMessage::RuntimeStateSnapshot(Box::new(snapshot));
    let frame = codec.encode_server_message(&message).unwrap();
    assert!(frame.len() < 1024 * 1024);
    assert_eq!(codec.decode_server_message(&frame).unwrap(), message);

    let ack = ClientMessage::RuntimeGenerationInstalled {
        client_id: 7,
        runtime_generation_id: 2,
    };
    let ack_frame = codec.encode_client_message(&ack).unwrap();
    assert_eq!(codec.decode_client_message(&ack_frame).unwrap(), ack);
}

#[test]
fn oversized_or_invalid_runtime_snapshot_is_rejected_before_install() {
    let codec = Codec::new(128);
    let snapshot = valid_snapshot(2, 1);
    let error = codec
        .encode_server_message(&ServerMessage::RuntimeStateSnapshot(Box::new(
            snapshot.clone(),
        )))
        .unwrap_err();
    assert!(matches!(
        error,
        clay::protocol::codec::CodecError::FrameTooLarge { max: 128, .. }
    ));

    let mut invalid = snapshot;
    invalid.behavior.manifest_id.clear();
    assert!(invalid.validate().is_err());
}

#[test]
fn runtime_snapshot_carries_complete_install_surface_for_atomic_client_swap() {
    let snapshot = valid_snapshot(3, 9);
    assert_eq!(snapshot.runtime_generation_id, 3);
    assert_eq!(snapshot.client_id, 9);
    assert_eq!(snapshot.behavior.behavior_version, 3);
    assert_eq!(snapshot.package_ui.version, 3);
    assert_eq!(snapshot.documents.len(), 1);
    assert!(snapshot.documents[0].reset_decorations);
    assert!(snapshot.documents[0].reset_diagnostics);
    assert!(snapshot.documents[0].initial_decorations.is_none());
    assert!(snapshot.documents[0].initial_diagnostics.is_none());
    assert!(snapshot.diagnostics.is_empty());
    snapshot
        .validate()
        .expect("complete snapshot remains installable");
}

#[test]
fn runtime_snapshot_payload_reports_diff_review_threshold_under_hard_ceiling() {
    use clay::perf::budgets::{
        RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS, RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES,
    };
    use clay::protocol::codec::DEFAULT_MAX_FRAME_SIZE;

    let codec = Codec::default();
    let snapshot = valid_snapshot(4, 11);
    let frame = codec
        .encode_server_message(&ServerMessage::RuntimeStateSnapshot(Box::new(snapshot)))
        .expect("representative snapshot encodes");
    let payload = frame.len().saturating_sub(4);
    assert!(
        payload < DEFAULT_MAX_FRAME_SIZE,
        "snapshot payload {payload} must stay under hard ceiling {DEFAULT_MAX_FRAME_SIZE}"
    );
    assert!(
        payload < RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES,
        "representative snapshot {payload} should remain under the 768 KiB diff-review threshold"
    );
    assert_eq!(RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS, 16);
}
