use super::*;

#[tokio::test]
async fn sdui_button_action_emits_server_intent() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);
    let intent = SduiActionIntent::command(
        "workspace.refresh",
        SduiActionSource::Button {
            node_id: SduiNodeId(5),
        },
    );

    queue.enqueue_sdui_action(3, intent.clone()).unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::SduiAction {
            client_id: 42,
            ui_version: 3,
            intent,
        }
    );
}

#[tokio::test]
async fn server_keybinding_emits_bounded_command_intent() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);

    queue
        .enqueue_command_intent(7, 3, "controlCenter.open".to_string())
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::CommandIntent {
            client_id: 42,
            document_id: 7,
            behavior_version: 3,
            command_id: "controlCenter.open".to_string(),
        }
    );
}

#[tokio::test]
async fn command_intent_hot_path_uses_try_send_backpressure() {
    let (queue, _receiver) = ClientEditQueue::bounded(1);
    queue
        .enqueue_command_intent(7, 3, "controlCenter.open".to_string())
        .unwrap();

    let error = queue
        .enqueue_command_intent(7, 3, "controlCenter.open".to_string())
        .unwrap_err();

    assert!(matches!(
        error,
        tokio::sync::mpsc::error::TrySendError::Full(_)
    ));
}

#[tokio::test]
async fn client_hot_path_does_not_await_full_ipc_queue() {
    let (queue, _receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(0, &DocumentAccess::Editable { lease_id: 1 });
    let event = EditorEditEvent {
        document_id: 1,
        base_version: 2,
        behavior_version: 3,
        operation: EditOperation::Insert {
            byte_offset: 0,
            text: "x".to_string(),
        },
    };
    queue.enqueue_edit_event(event.clone(), 1).unwrap();

    let started = std::time::Instant::now();
    let result = queue.enqueue_edit_event(event, 2);

    assert!(result.is_err());
    assert!(
        started.elapsed() < std::time::Duration::from_millis(50),
        "full queue should fail through try_send instead of awaiting capacity"
    );
    assert_eq!(queue.sync_snapshot().pending.len(), 1);
}

#[tokio::test]
async fn client_keeps_pending_edit_until_ack_or_rejection() {
    let (queue, mut receiver) = ClientEditQueue::bounded(2);
    let queue = queue
        .with_authority(0, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(10);
    let event = EditorEditEvent {
        document_id: 1,
        base_version: 0,
        behavior_version: 3,
        operation: EditOperation::Insert {
            byte_offset: 0,
            text: "a".to_string(),
        },
    };

    queue.enqueue_edit_event(event, 44).unwrap();
    let message = receiver.recv().await.unwrap();

    assert!(matches!(
        message,
        ClientMessage::Edit {
            base_version: 10,
            transaction_id: 44,
            ..
        }
    ));
    let snapshot = queue.sync_snapshot();
    assert_eq!(snapshot.confirmed_version, 10);
    assert_eq!(snapshot.optimistic_version, 11);
    assert_eq!(snapshot.pending.len(), 1);
}

#[tokio::test]
async fn client_ack_advances_confirmed_version() {
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
        let _edit = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::EditAck {
                    document_id: 7,
                    confirmed_version: 11,
                    transaction_id: 44,
                },
            )
            .await
            .unwrap();
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();
    session
        .edit_queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: 7,
                base_version: 0,
                behavior_version: 3,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "a".to_string(),
                },
            },
            44,
        )
        .unwrap();
    assert_eq!(session.edit_queue.sync_snapshot().pending.len(), 1);

    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::EditAck {
            document_id: 7,
            version: 11,
            transaction_id: 44,
        }
    );
    let snapshot = session.edit_queue.sync_snapshot();
    assert_eq!(snapshot.confirmed_version, 11);
    assert_eq!(snapshot.pending.len(), 0);
    server_task.await.unwrap();
}

#[tokio::test]
async fn client_requests_resync_after_stale_rejection() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let server_task = tokio::spawn(async move {
        let _hello = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::Welcome {
                    client_id: 12,
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
                    head: DocumentTextHead::complete("local".to_string()),
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
        let _edit = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::EditRejected {
                    document_id: 7,
                    transaction_id: 44,
                    reason: crate::protocol::EditRejection::StaleVersion {
                        client_base_version: 10,
                        server_version: 12,
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_client_message(&mut server).await.unwrap(),
            ClientMessage::RequestResync {
                document_id: 7,
                client_id: 12,
                known_version: 10,
            }
        );
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();
    session
        .edit_queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: 7,
                base_version: 0,
                behavior_version: 3,
                operation: EditOperation::Insert {
                    byte_offset: 5,
                    text: "!".to_string(),
                },
            },
            44,
        )
        .unwrap();

    assert!(matches!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::EditRejected {
            transaction_id: 44,
            ..
        }
    ));
    server_task.await.unwrap();
}

#[tokio::test]
async fn client_applies_resync_snapshot_and_clears_pending_edits() {
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
                    document_id: 7,
                    version: 10,
                    head: DocumentTextHead::complete("local".to_string()),
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
        let _edit = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::EditRejected {
                    document_id: 7,
                    transaction_id: 44,
                    reason: crate::protocol::EditRejection::StaleVersion {
                        client_base_version: 10,
                        server_version: 12,
                    },
                },
            )
            .await
            .unwrap();
        let _resync = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::ResyncSnapshot {
                    document_id: 7,
                    version: 12,
                    head: DocumentTextHead::complete("server 🦀".to_string()),
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                },
            )
            .await
            .unwrap();
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();
    session
        .edit_queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: 7,
                base_version: 0,
                behavior_version: 3,
                operation: EditOperation::Insert {
                    byte_offset: 5,
                    text: "!".to_string(),
                },
            },
            44,
        )
        .unwrap();
    let _rejection = session.events.recv().await.unwrap();

    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::ResyncSnapshot(super::super::ClientResyncSnapshot {
            document_id: 7,
            version: 12,
            head: DocumentTextHead::complete("server 🦀".to_string()),
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
        })
    );
    let snapshot = session.edit_queue.sync_snapshot();
    assert_eq!(snapshot.confirmed_version, 12);
    assert_eq!(snapshot.optimistic_version, 12);
    assert!(snapshot.pending.is_empty());
    assert_eq!(snapshot.last_resync.unwrap().head.first_chunk, "server 🦀");
    server_task.await.unwrap();
}
