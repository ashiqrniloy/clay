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
        active_design_system: clay::protocol::ActiveDesignSystem::core_fallback(generation),
        active_icon_pack: None,
        ui_choices: clay::protocol::UiChoicesSnapshot::default(),
        sdui_tree: SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![SduiNode::new(
                SduiNodeId(1),
                SduiNodeKind::Label {
                    text: "runtime".to_string(),
                    icon: None,
                },
            )],
        },
        package_ui: PackageUiSnapshot {
            version: generation,
            ..Default::default()
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

#[test]
fn runtime_snapshot_carries_bounded_active_icon_pack_and_round_trips() {
    use clay::shell::icons::{IconGeometry, IconPath, IconPathCommand};
    use std::collections::BTreeMap;

    let mut icons = BTreeMap::new();
    icons.insert(
        "action.close".to_string(),
        IconGeometry {
            view_box: [0.0, 0.0, 24.0, 24.0],
            paths: vec![IconPath {
                commands: vec![
                    IconPathCommand::MoveTo([4.0, 4.0]),
                    IconPathCommand::LineTo([20.0, 20.0]),
                    IconPathCommand::LineTo([4.0, 20.0]),
                    IconPathCommand::LineTo([20.0, 4.0]),
                    IconPathCommand::ClosePath,
                ],
                opacity: None,
            }],
        },
    );
    let pack = clay::protocol::ActiveIconPack {
        specifier: "@clay/icons-phosphor-regular".to_string(),
        schema_version: 1,
        generation: 5,
        provenance: clay::protocol::DesignSystemProvenance {
            package_name: "@clay/icons-phosphor-regular".to_string(),
            package_version: "2.0.8".to_string(),
            api_prefix: "icons-phosphor-regular".to_string(),
            trust_domain: clay::protocol::PackageUiTrustDomain::Trusted,
        },
        icons,
    };
    pack.validate().expect("fixture pack");

    let mut snapshot = valid_snapshot(5, 3);
    snapshot.active_icon_pack = Some(pack);
    snapshot
        .validate()
        .expect("snapshot with bounded pack validates");

    // Wire round-trip preserves exact identity, provenance, and geometry.
    let codec = Codec::default();
    let message = ServerMessage::RuntimeStateSnapshot(Box::new(snapshot.clone()));
    let frame = codec.encode_server_message(&message).unwrap();
    assert_eq!(codec.decode_server_message(&frame).unwrap(), message);

    // The duotone-style layered path serializes to the canonical d-string
    // contract shape with opacity preserved.
    let pack = snapshot.active_icon_pack.as_ref().unwrap();
    let geometry = &pack.icons["action.close"];
    let json = serde_json::to_value(geometry).unwrap();
    assert_eq!(json["viewBox"], serde_json::json!([0.0, 0.0, 24.0, 24.0]));
    assert_eq!(json["paths"][0]["d"], "M 4,4 L 20,20 L 4,20 L 20,4 Z");
    assert!(json["paths"][0].get("opacity").is_none());
    assert_eq!(pack.specifier, "@clay/icons-phosphor-regular");
    assert_eq!(pack.provenance.api_prefix, "icons-phosphor-regular");
    assert_eq!(pack.generation, 5);
}

#[test]
fn runtime_snapshot_rejects_invalid_active_icon_packs() {
    use clay::shell::icons::{IconGeometry, IconPath, IconPathCommand};
    use std::collections::BTreeMap;

    let mut snapshot = valid_snapshot(6, 3);
    let mut icons = BTreeMap::new();
    icons.insert(
        "action.close".to_string(),
        IconGeometry {
            view_box: [0.0, 0.0, 24.0, 24.0],
            paths: vec![IconPath {
                commands: vec![IconPathCommand::LineTo([20.0, 20.0])],
                opacity: None,
            }],
        },
    );
    snapshot.active_icon_pack = Some(clay::protocol::ActiveIconPack {
        specifier: "@clay/icons-phosphor-regular".to_string(),
        schema_version: 1,
        generation: 6,
        provenance: clay::protocol::DesignSystemProvenance {
            package_name: "@clay/icons-phosphor-regular".to_string(),
            package_version: "2.0.8".to_string(),
            api_prefix: "icons-phosphor-regular".to_string(),
            trust_domain: clay::protocol::PackageUiTrustDomain::Trusted,
        },
        icons,
    });

    // Malformed geometry (path not starting with M) is rejected pre-install.
    assert!(snapshot.validate().is_err());

    // Unsupported schema versions are rejected (stale/unknown packs).
    let mut stale = snapshot.clone();
    let pack = stale.active_icon_pack.as_mut().unwrap();
    pack.schema_version = 99;
    assert!(stale.validate().is_err());

    // Unknown specifiers are structurally invalid.
    let mut anonymous = snapshot.clone();
    anonymous.active_icon_pack.as_mut().unwrap().specifier = "  ".to_string();
    assert!(anonymous.validate().is_err());

    // Excessive per-icon payload is rejected before wire publication. Reuse
    // the valid close glyph from the round-trip fixture as the base.
    let mut oversized = valid_snapshot(6, 3);
    let mut oversized_icons = BTreeMap::new();
    oversized_icons.insert(
        "action.close".to_string(),
        IconGeometry {
            view_box: [0.0, 0.0, 24.0, 24.0],
            paths: vec![IconPath {
                commands: vec![
                    IconPathCommand::MoveTo([4.0, 4.0]),
                    IconPathCommand::LineTo([20.0, 20.0]),
                    IconPathCommand::ClosePath,
                ],
                opacity: None,
            }],
        },
    );
    let pack = &mut oversized_icons;
    pack.insert(
        "git.branch".to_string(),
        IconGeometry {
            view_box: [0.0, 0.0, 24.0, 24.0],
            paths: vec![IconPath {
                commands: (0..512)
                    .map(|i| {
                        let x = i as f32 * 0.05;
                        if i == 0 {
                            IconPathCommand::MoveTo([x, x + 1.0])
                        } else {
                            IconPathCommand::LineTo([x, x + 1.0])
                        }
                    })
                    .collect(),
                opacity: None,
            }],
        },
    );
    oversized.active_icon_pack = Some(clay::protocol::ActiveIconPack {
        specifier: "@clay/icons-phosphor-regular".to_string(),
        schema_version: 1,
        generation: 6,
        provenance: clay::protocol::DesignSystemProvenance {
            package_name: "@clay/icons-phosphor-regular".to_string(),
            package_version: "2.0.8".to_string(),
            api_prefix: "icons-phosphor-regular".to_string(),
            trust_domain: clay::protocol::PackageUiTrustDomain::Trusted,
        },
        icons: std::mem::take(pack),
    });
    let error = oversized
        .active_icon_pack
        .as_ref()
        .unwrap()
        .validate()
        .expect_err("512-command glyph exceeds the per-icon payload budget");
    assert!(
        matches!(
            error,
            clay::shell::icons::IconPackValidationError::GeometryTooLarge { .. }
        ),
        "unexpected error variant: {error:?}"
    );
    assert!(oversized.validate().is_err());
}
