use super::*;

#[tokio::test]
async fn client_installs_minimal_behavior_manifest() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let server_task = tokio::spawn(async move {
        let _hello = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::Welcome {
                    client_id: 1,
                    protocol_version: PROTOCOL_VERSION,
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::InitialDocument {
                    document_id: 2,
                    version: 1,
                    head: DocumentTextHead::complete(String::new()),
                    access: DocumentAccess::ReadOnly,
                    lease_id: None,
                    workspace_root: String::new(),
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(
                    5,
                ))),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                    specifier: "@clay/default".to_string(),
                    overrides: Vec::new(),
                    design_tokens: Vec::new(),
                }),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTypography(ActiveTypography::default()),
            )
            .await
            .unwrap();
    });

    let state = load_initial_state_from_stream(client, codec).await.unwrap();

    assert_eq!(state.behavior_manifest.behavior_version, 5);
    assert_eq!(state.access, DocumentAccess::ReadOnly);
    server_task.await.unwrap();
}

#[tokio::test]
async fn end_to_end_client_receives_initial_snapshot() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let server_task = tokio::spawn(async move {
        let _hello = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::Welcome {
                    client_id: 21,
                    protocol_version: PROTOCOL_VERSION,
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::InitialDocument {
                    document_id: 22,
                    version: 23,
                    head: DocumentTextHead::complete("snapshot".to_string()),
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    workspace_root: String::new(),
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(
                    24,
                ))),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                    specifier: "@clay/default".to_string(),
                    overrides: Vec::new(),
                    design_tokens: Vec::new(),
                }),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTypography(ActiveTypography::default()),
            )
            .await
            .unwrap();
    });

    let session = connect_from_stream(client, codec).await.unwrap();

    assert_eq!(session.initial_state.document_id, 22);
    assert_eq!(session.initial_state.document_version, 23);
    assert_eq!(session.initial_state.head.first_chunk, "snapshot");
    server_task.await.unwrap();
}

#[tokio::test]
async fn end_to_end_client_receives_behavior_manifest() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let server_task = tokio::spawn(async move {
        let _hello = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::Welcome {
                    client_id: 1,
                    protocol_version: PROTOCOL_VERSION,
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::InitialDocument {
                    document_id: 2,
                    version: 3,
                    head: DocumentTextHead::complete(String::new()),
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    workspace_root: String::new(),
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(
                    44,
                ))),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                    specifier: "@clay/default".to_string(),
                    overrides: Vec::new(),
                    design_tokens: Vec::new(),
                }),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTypography(ActiveTypography::default()),
            )
            .await
            .unwrap();
    });

    let session = connect_from_stream(client, codec).await.unwrap();

    assert_eq!(session.initial_state.behavior_manifest.behavior_version, 44);
    server_task.await.unwrap();
}

#[tokio::test]
async fn client_receives_sdui_snapshot_event() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let tree = SduiTree {
        ui_version: 1,
        root_id: SduiNodeId(1),
        nodes: vec![SduiNode::new(
            SduiNodeId(1),
            SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id: 7,
                    expected_version: Some(10),
                },
            },
        )],
    };
    let expected_tree = tree.clone();
    let server_task = tokio::spawn(async move {
        let _hello = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::Welcome {
                    client_id: 1,
                    protocol_version: PROTOCOL_VERSION,
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::InitialDocument {
                    document_id: 7,
                    version: 10,
                    head: DocumentTextHead::complete(String::new()),
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                    workspace_root: String::new(),
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(
                    3,
                ))),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                    specifier: "@clay/default".to_string(),
                    overrides: Vec::new(),
                    design_tokens: Vec::new(),
                }),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ActiveTypography(ActiveTypography::default()),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::SduiSnapshot { client_id: 1, tree },
            )
            .await
            .unwrap();
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();

    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::SduiSnapshot {
            client_id: 1,
            tree: expected_tree,
        }
    );
    server_task.await.unwrap();
}
