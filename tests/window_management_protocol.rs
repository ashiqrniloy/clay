//! Phase 22.6 (plan 077 task 7): protocol compatibility for the Phase 22
//! window-management messages — tab commands, the tab registry snapshot,
//! and the handshake protocol version.
//!
//! This suite guards, it does not extend: every Phase 22.3/22.4 addition
//! must survive the rkyv length-prefixed codec unchanged, the wire version
//! is pinned, and malformed frames are rejected without panic (archived
//! bytes are validated before access by the codec contract).

use clay::protocol::{
    BehaviorManifest, ClientMessage, KeyBindingContext, KeyBindingRule, KeyCode, KeyModifiers,
    KeyStroke, PROTOCOL_VERSION, RoutingPolicy, ServerMessage, TabCommand, TabEntry,
    TabRegistrySnapshot, codec::Codec,
};

fn registry_snapshot() -> TabRegistrySnapshot {
    TabRegistrySnapshot {
        tabs: vec![
            TabEntry {
                tab_id: 1,
                workspace_root_id: 10,
                client_id: 1,
                workspace_root: "/tmp/alpha".to_string(),
            },
            TabEntry {
                tab_id: 2,
                workspace_root_id: 20,
                client_id: 2,
                workspace_root: "/tmp/beta".to_string(),
            },
        ],
        active: Some(2),
        revision: 7,
    }
}

/// The handshake wire version is pinned. Bump `PROTOCOL_VERSION`
/// deliberately when the wire changes; this test fails loudly otherwise.
#[test]
fn multi_stroke_key_binding_rules_round_trip_the_archive_identically() {
    // Phase 24.5: the multi-stroke router adds no wire shape —
    // `KeyBindingRule.sequence` already carried N strokes. Prove a
    // multi-stroke rule survives the codec identically (mixed with a
    // single-stroke rule, since both share the manifest snapshot).
    let codec = Codec::default();
    let mut manifest = BehaviorManifest::minimal_text_editing(1);
    manifest.keymaps.push(KeyBindingRule {
        command_id: "controlCenter.open".to_string(),
        sequence: vec![
            KeyStroke {
                key: KeyCode::Character("x".to_string()),
                modifiers: KeyModifiers {
                    shift: false,
                    control: true,
                    alt: false,
                    super_key: false,
                },
            },
            KeyStroke {
                key: KeyCode::Character("p".to_string()),
                modifiers: KeyModifiers {
                    shift: false,
                    control: true,
                    alt: false,
                    super_key: false,
                },
            },
        ],
        context: KeyBindingContext::Global,
        routing_policy: RoutingPolicy::ServerFirst,
    });
    manifest.keymaps.push(KeyBindingRule::single(
        "text.insert_newline",
        KeyCode::Enter,
    ));

    let frame = codec
        .encode_server_message(&ServerMessage::BehaviorManifest(Box::new(manifest.clone())))
        .expect("manifest encodes");
    assert!(
        frame.len() < 1024 * 1024,
        "behavior manifest frame must stay bounded"
    );
    let decoded = codec
        .decode_server_message(&frame)
        .expect("manifest decodes");
    assert_eq!(
        decoded,
        ServerMessage::BehaviorManifest(Box::new(manifest)),
        "multi-stroke rules must round-trip the archive identically"
    );
}

#[test]
fn protocol_version_is_pinned() {
    assert_eq!(PROTOCOL_VERSION, 17);
}

/// The handshake itself round-trips: `Hello` carries the pinned version to
/// the server, which must not accept an older client silently.
#[test]
fn handshake_hello_round_trips_with_pinned_protocol_version() {
    let codec = Codec::default();
    let hello = ClientMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        client_name: "clay-window-management-test".to_string(),
    };
    let frame = codec.encode_client_message(&hello).unwrap();
    assert!(frame.len() < 1024 * 1024);
    assert_eq!(codec.decode_client_message(&frame).unwrap(), hello);
}

/// Every Phase 22.3 (new/open/close/activate/reclaim) and Phase 22.4
/// (move-left/right/to) tab command survives encode/decode equal, with
/// `client_id` validated like any other client message at decode time.
#[test]
fn tab_commands_round_trip_the_codec_unchanged() {
    let codec = Codec::default();
    let commands = [
        TabCommand::New {
            workspace_root: "/tmp/alpha".to_string(),
        },
        TabCommand::OpenWorkspace {
            tab_id: 1,
            root: "/tmp/beta".to_string(),
        },
        TabCommand::Close { tab_id: 2 },
        TabCommand::Activate { tab_id: 1 },
        TabCommand::Reclaim { tab_id: 2 },
        TabCommand::MoveLeft { tab_id: 2 },
        TabCommand::MoveRight { tab_id: 1 },
        TabCommand::MoveTo {
            tab_id: 2,
            position: 1,
        },
    ];
    for command in commands {
        let message = ClientMessage::TabCommand {
            client_id: 7,
            command,
        };
        let frame = codec.encode_client_message(&message).unwrap();
        assert!(
            frame.len() < 1024 * 1024,
            "tab command frame must stay bounded"
        );
        assert_eq!(codec.decode_client_message(&frame).unwrap(), message);
    }
}

/// The server-authoritative registry snapshot (tab order, active tab,
/// per-tab identity bindings) survives encode/decode equal with a bounded
/// payload, in both directions.
#[test]
fn tab_registry_snapshot_round_trips_with_bounded_payload() {
    let codec = Codec::default();
    let snapshot = registry_snapshot();
    let message = ServerMessage::TabRegistry(snapshot.clone());
    let frame = codec.encode_server_message(&message).unwrap();
    assert!(
        frame.len() < 1024 * 1024,
        "registry snapshot frame must stay bounded"
    );
    assert_eq!(codec.decode_server_message(&frame).unwrap(), message);
}

/// Malformed tab frames are rejected without panic: short frames, declared
/// length mismatches, oversize declarations, and corrupt payload bytes all
/// fail closed at the codec boundary before any archived access.
#[test]
fn malformed_tab_frames_are_rejected_without_panic() {
    let codec = Codec::default();
    let snapshot = registry_snapshot();
    let valid = codec
        .encode_server_message(&ServerMessage::TabRegistry(snapshot))
        .unwrap();
    let payload_len = valid.len() - 4;

    // Shorter than the length prefix.
    assert!(
        matches!(
            codec.decode_server_message(&[]),
            Err(clay::protocol::codec::CodecError::IncompleteFrame)
        ),
        "empty frame must fail closed"
    );
    assert!(
        matches!(
            codec.decode_server_message(&[0, 0, 0]),
            Err(clay::protocol::codec::CodecError::IncompleteFrame)
        ),
        "sub-prefix frame must fail closed"
    );

    // Declared length larger than the actual payload.
    let truncated = valid[..valid.len() - 8].to_vec();
    assert!(
        matches!(
            codec.decode_server_message(&truncated),
            Err(clay::protocol::codec::CodecError::LengthMismatch {
                declared,
                actual
            }) if declared == payload_len && actual == truncated.len() - 4
        ),
        "declared/actual length mismatch must fail closed"
    );

    // Declared length beyond the frame ceiling is rejected before read.
    let mut oversize = vec![0; 4];
    oversize[..4].copy_from_slice(&(1024 * 1024 + 1u32).to_be_bytes());
    assert!(
        matches!(
            codec.decode_server_message(&oversize),
            Err(clay::protocol::codec::CodecError::FrameTooLarge { .. })
        ),
        "oversize declared length must fail closed"
    );

    // Corrupt payload bytes with a valid length: validated deserialization
    // rejects, never panics.
    let mut corrupted = valid.clone();
    // Flip a byte inside the payload proper (past the length prefix and
    // message discriminant), not trailing alignment padding.
    corrupted[8] ^= 0xFF;
    assert!(
        matches!(
            codec.decode_server_message(&corrupted),
            Err(clay::protocol::codec::CodecError::Deserialize(_))
        ),
        "corrupt archived bytes must fail validated access"
    );
}
