use super::*;

#[test]
fn invalid_behavior_version_rejection_requests_resync() {
    assert!(super::super::rejection_requests_resync(
        &crate::protocol::EditRejection::InvalidBehaviorVersion {
            behavior_version: 1,
            server_behavior_version: 2,
        }
    ));
}

#[tokio::test]
async fn client_handles_initial_document_message() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let server_task = tokio::spawn(async move {
        let _hello = codec.read_client_message(&mut server).await.unwrap();
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::Welcome {
                    client_id: 11,
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
                    version: 3,
                    head: DocumentTextHead::complete("Loaded from server 🦀".to_string()),
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
                    9,
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

    assert_eq!(state.client_id, 11);
    assert_eq!(state.document_id, 7);
    assert_eq!(state.document_version, 3);
    assert_eq!(state.head.first_chunk, "Loaded from server 🦀");
    assert_eq!(state.access, DocumentAccess::Editable { lease_id: 1 });
    assert_eq!(state.active_typography, ActiveTypography::default());
    assert_eq!(
        state.behavior_manifest,
        BehaviorManifest::minimal_text_editing(9)
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn edit_event_is_enqueued_as_client_edit_message() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue
        .with_authority(0, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(5);

    queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: 4,
                base_version: 5,
                behavior_version: 6,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: "x".to_string(),
                },
            },
            7,
        )
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        crate::protocol::ClientMessage::Edit {
            document_id: 4,
            client_id: 0,
            lease_id: Some(1),
            base_version: 5,
            behavior_version: 6,
            transaction_id: 7,
            operation: EditOperation::Insert {
                byte_offset: 2,
                text: "x".to_string()
            }
        }
    );
}

#[tokio::test]
async fn read_only_client_queue_does_not_emit_edit_message() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_confirmed_version(5);

    let result = queue.enqueue_edit_event(
        EditorEditEvent {
            document_id: 4,
            base_version: 5,
            behavior_version: 6,
            operation: EditOperation::Insert {
                byte_offset: 2,
                text: "x".to_string(),
            },
        },
        7,
    );

    assert!(result.is_err());
    assert!(receiver.try_recv().is_err());
    assert!(queue.sync_snapshot().pending.is_empty());
}

#[test]
fn opened_document_reset_keeps_connection_and_editor_sync_state_shared() {
    let (mut editor_queue, _receiver) = ClientEditQueue::bounded(1);
    let connection_queue = editor_queue.clone();

    editor_queue.update_opened_document_authority(42, &DocumentAccess::Editable { lease_id: 8 }, 5);

    assert_eq!(connection_queue.sync_snapshot().confirmed_version, 5);
    assert_eq!(connection_queue.sync_snapshot().optimistic_version, 5);
}
