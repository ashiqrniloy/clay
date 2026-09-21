use super::*;

#[tokio::test]
async fn bounded_edit_queue_applies_backpressure() {
    let (queue, _receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(0, &DocumentAccess::Editable { lease_id: 1 });
    let event = EditorEditEvent {
        document_id: 1,
        base_version: 2,
        behavior_version: 3,
        operation: EditOperation::Delete { start: 4, end: 5 },
    };

    assert!(queue.enqueue_edit_event(event.clone(), 1).is_ok());
    assert!(queue.enqueue_edit_event(event, 2).is_err());
    assert_eq!(queue.sync_snapshot().pending.len(), 1);
}

#[tokio::test]
async fn viewport_render_request_emits_bounded_range_metadata() {
    let (queue, mut receiver) = ClientEditQueue::bounded(2);
    let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);

    queue
        .enqueue_viewport_render_request(7, 3, 1, 1_024, 2_048)
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::ViewportRenderRequest {
            client_id: 42,
            document_id: 7,
            document_version: 3,
            request_id: 1,
            byte_start: 1_024,
            byte_end: 2_048,
            trace_id: None,
        }
    );
}

#[tokio::test]
async fn viewport_requests_reserve_queue_capacity_for_workspace_actions() {
    let (queue, mut receiver) = ClientEditQueue::bounded(2);
    let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);

    queue
        .enqueue_viewport_render_request(7, 3, 1, 1_024, 2_048)
        .unwrap();
    assert!(matches!(
        queue.enqueue_viewport_render_request(7, 3, 2, 2_048, 3_072),
        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
    ));
    queue
        .enqueue_sdui_action(
            1,
            SduiActionIntent {
                command_id: "workspace.openDirectory".to_string(),
                source: SduiActionSource::Button {
                    node_id: SduiNodeId(5),
                },
                arguments: Vec::new(),
            },
        )
        .unwrap();

    assert!(matches!(
        receiver.recv().await.unwrap(),
        ClientMessage::ViewportRenderRequest { .. }
    ));
    assert!(matches!(
        receiver.recv().await.unwrap(),
        ClientMessage::SduiAction { .. }
    ));
}

#[tokio::test]
async fn fragmented_frame_survives_concurrent_outgoing_message() {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    let codec = Codec::default();
    let (client_end, mut server_end) = duplex(64 * 1024);

    let (edit_queue, outgoing) = ClientEditQueue::bounded(super::super::EDIT_QUEUE_CAPACITY);
    let edit_queue = edit_queue.with_authority(7, &DocumentAccess::ReadOnly);
    let sync_state = std::sync::Arc::clone(&edit_queue.sync_state);
    let file_open_capability = std::sync::Arc::clone(&edit_queue.file_open_capability);
    let behavior_state = std::sync::Arc::new(std::sync::Mutex::new(
        super::super::behavior::ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1))
            .unwrap(),
    ));
    let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(super::super::EDIT_QUEUE_CAPACITY);
    let connection = tokio::spawn(super::super::run_connection(
        client_end,
        codec,
        outgoing,
        events_tx,
        sync_state,
        behavior_state,
        file_open_capability,
        7,
    ));

    let provenance = crate::protocol::DecorationProvenance {
        package_name: "@clay/markdown".to_string(),
        package_version: "builtin".to_string(),
        package_prefix: "markdown".to_string(),
    };
    let spans = (0..256)
        .map(|index| {
            crate::protocol::DecorationSpan::from_vocabulary(
                index * 16,
                index * 16 + 8,
                crate::protocol::DecorationKind::Syntax,
                crate::protocol::TokenType::Paragraph,
                crate::protocol::Modifiers::NONE,
                70,
                provenance.clone(),
            )
        })
        .collect();
    let set = crate::protocol::DecorationSet {
        document_id: 1,
        document_version: 1,
        package_prefix: "markdown".to_string(),
        kind: crate::protocol::DecorationKind::Syntax,
        viewport_byte_start: 0,
        viewport_byte_end: 4096,
        spans,
        trace_id: None,
    };
    let frame = codec
        .encode_server_message(&ServerMessage::DecorationSet(set))
        .unwrap();

    // Drip-feed the frame start, then win the select race with an outgoing
    // message before the rest of the frame arrives. The read pump must keep
    // frame alignment regardless of the interleaving.
    let split = 6;
    server_end.write_all(&frame[..split]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    edit_queue
        .enqueue_viewport_render_request(1, 1, 1, 0, 4096)
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    server_end.write_all(&frame[split..]).await.unwrap();
    codec
        .write_server_message(
            &mut server_end,
            &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            }),
        )
        .await
        .unwrap();

    let first = tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(first, ClientConnectionEvent::DecorationSet(_)),
        "fragmented frame must decode as one DecorationSet, got {first:?}"
    );
    let second = tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(second, ClientConnectionEvent::ActiveTheme(_)),
        "next frame must stay aligned, got {second:?}"
    );

    drop(edit_queue);
    connection.await.unwrap();
}

#[tokio::test]
async fn decoration_batch_frame_dispatches_single_event() {
    use std::time::Duration;

    let codec = Codec::default();
    let (client_end, mut server_end) = duplex(64 * 1024);

    let (edit_queue, outgoing) = ClientEditQueue::bounded(super::super::EDIT_QUEUE_CAPACITY);
    let edit_queue = edit_queue.with_authority(7, &DocumentAccess::ReadOnly);
    let sync_state = std::sync::Arc::clone(&edit_queue.sync_state);
    let file_open_capability = std::sync::Arc::clone(&edit_queue.file_open_capability);
    let behavior_state = std::sync::Arc::new(std::sync::Mutex::new(
        super::super::behavior::ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1))
            .unwrap(),
    ));
    let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(super::super::EDIT_QUEUE_CAPACITY);
    let connection = tokio::spawn(super::super::run_connection(
        client_end,
        codec,
        outgoing,
        events_tx,
        sync_state,
        behavior_state,
        file_open_capability,
        7,
    ));

    let provenance = crate::protocol::DecorationProvenance {
        package_name: "@clay/markdown".to_string(),
        package_version: "builtin".to_string(),
        package_prefix: "markdown".to_string(),
    };
    let chunk = |start: u64| crate::protocol::DecorationSet {
        document_id: 1,
        document_version: 2,
        package_prefix: "markdown".to_string(),
        kind: crate::protocol::DecorationKind::Syntax,
        viewport_byte_start: start,
        viewport_byte_end: start + 128,
        spans: vec![crate::protocol::DecorationSpan::from_vocabulary(
            start,
            start + 8,
            crate::protocol::DecorationKind::Syntax,
            crate::protocol::TokenType::Paragraph,
            crate::protocol::Modifiers::NONE,
            70,
            provenance.clone(),
        )],
        trace_id: None,
    };
    codec
        .write_server_message(
            &mut server_end,
            &ServerMessage::DecorationBatch(vec![chunk(0), chunk(128), chunk(256)]),
        )
        .await
        .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .unwrap()
        .unwrap();
    let ClientConnectionEvent::DecorationBatch(sets) = event else {
        panic!("batch frame must dispatch one batch event, got {event:?}");
    };
    assert_eq!(sets.len(), 3);
    assert!(
        sets.windows(2)
            .all(|pair| pair[0].viewport_byte_start < pair[1].viewport_byte_start),
        "chunk order preserved"
    );

    drop(edit_queue);
    connection.await.unwrap();
}

#[tokio::test]
async fn selected_file_open_request_emits_non_edit_message() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue
        .with_authority(42, &DocumentAccess::ReadOnly)
        .with_file_open_capability("foc-test-token");
    let selected_path = PathBuf::from("C:/Users/test/Documents/note.md");

    queue
        .enqueue_open_selected_file(selected_path.clone())
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::OpenSelectedFile {
            client_id: 42,
            capability: "foc-test-token".to_string(),
            selected_path: selected_path.to_string_lossy().into_owned(),
        }
    );
}

#[tokio::test]
async fn selected_folder_root_request_emits_non_edit_message() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue
        .with_authority(42, &DocumentAccess::ReadOnly)
        .with_file_open_capability("folder-token");
    let selected_path = PathBuf::from("/home/test/project");

    queue
        .enqueue_add_selected_workspace_root(selected_path.clone())
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::AddSelectedWorkspaceRoot {
            client_id: 42,
            capability: "folder-token".to_string(),
            selected_path: selected_path.to_string_lossy().into_owned(),
        }
    );
}

#[tokio::test]
async fn selected_file_open_without_capability_sends_empty_token() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);
    let selected_path = PathBuf::from("C:/Users/test/Documents/note.md");

    queue
        .enqueue_open_selected_file(selected_path.clone())
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::OpenSelectedFile {
            client_id: 42,
            capability: String::new(),
            selected_path: selected_path.to_string_lossy().into_owned(),
        }
    );
}
