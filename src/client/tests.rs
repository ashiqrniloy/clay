#[cfg(unix)]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(any(unix, windows))]
use std::time::SystemTime;

use tokio::io::duplex;
#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(windows)]
use super::connect_transport;
use super::{
    ClientConnectionEvent, ClientEditQueue, connect_for_reclaim as connect_for_reclaim_endpoint,
    connect_for_reclaim_or_new, connect_from_stream,
    connect_with_workspace_root as connect_with_workspace_root_endpoint,
    load_initial_state_from_stream,
};
#[cfg(any(unix, windows))]
use super::{ClientSession, connect};
use super::{EditorCompletionRequestEvent, EditorEditEvent};
#[cfg(any(unix, windows))]
use crate::ipc::IpcEndpoint;
#[cfg(any(unix, windows))]
use crate::protocol::EditRejection;
use crate::protocol::{
    ActiveTypography, BehaviorManifest, ClientMessage, CommandDeclaration,
    CompletionReplacementRange, CompletionTrigger, DocumentAccess, DocumentTextHead, EditOperation,
    FileErrorCode, PROTOCOL_VERSION, RuntimeDiagnostic, SduiActionIntent, SduiActionSource,
    SduiEditorBinding, SduiNode, SduiNodeId, SduiNodeKind, SduiTree, ServerMessage, codec::Codec,
};
#[cfg(any(unix, windows))]
use crate::server::{IpcServer, ServerConfig};

#[cfg(unix)]
fn unique_socket_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "clay-client-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    dir.join("clay.sock")
}

#[cfg(unix)]
async fn connect_with_retry(socket_path: &std::path::Path) -> ClientSession {
    let mut last_error = None;
    for _ in 0..50 {
        match connect(&IpcEndpoint::from(socket_path)).await {
            Ok(session) => return session,
            Err(error) => {
                last_error = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
    panic!("failed to connect to test socket: {:?}", last_error);
}

#[cfg(unix)]
async fn connect_stream_with_retry(socket_path: &std::path::Path) -> UnixStream {
    let mut last_error = None;
    for _ in 0..50 {
        match UnixStream::connect(socket_path).await {
            Ok(stream) => return stream,
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
    panic!("failed to connect to test socket: {:?}", last_error);
}

#[cfg(unix)]
async fn connect_with_workspace_root(
    socket_path: &Path,
    workspace_root: String,
) -> Result<ClientSession, super::ClientBootstrapError> {
    connect_with_workspace_root_endpoint(&IpcEndpoint::from(socket_path), workspace_root).await
}

#[cfg(unix)]
async fn connect_for_reclaim(
    socket_path: &Path,
    tab_id: crate::protocol::TabId,
) -> Result<ClientSession, super::ClientBootstrapError> {
    connect_for_reclaim_endpoint(&IpcEndpoint::from(socket_path), tab_id).await
}

#[test]
fn invalid_behavior_version_rejection_requests_resync() {
    assert!(super::rejection_requests_resync(
        &crate::protocol::EditRejection::InvalidBehaviorVersion {
            behavior_version: 1,
            server_behavior_version: 2,
        }
    ));
}

#[cfg(windows)]
fn unique_named_pipe(name: &str) -> IpcEndpoint {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    IpcEndpoint::WindowsNamedPipe(format!(
        r"\\.\pipe\clay-client-{name}-{}-{unique}",
        std::process::id()
    ))
}

#[cfg(windows)]
async fn connect_with_retry(endpoint: &IpcEndpoint) -> ClientSession {
    let mut last_error = None;
    for _ in 0..50 {
        match connect(endpoint).await {
            Ok(session) => return session,
            Err(error) => {
                last_error = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
    panic!("failed to connect to test named pipe: {:?}", last_error);
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
async fn completion_request_is_enqueued_as_non_blocking_message() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(42, &DocumentAccess::Editable { lease_id: 1 });

    queue
        .enqueue_completion_request(
            EditorCompletionRequestEvent {
                document_id: 4,
                document_version: 5,
                behavior_version: 6,
                cursor_byte_offset: 9,
                replacement_range: CompletionReplacementRange::new(7, 9),
                trigger: CompletionTrigger::Manual,
            },
            11,
        )
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::CompletionRequest {
            request: crate::protocol::CompletionRequest {
                request_id: 11,
                client_id: 42,
                document_id: 4,
                document_version: 5,
                behavior_version: 6,
                cursor_byte_offset: 9,
                replacement_range: CompletionReplacementRange::new(7, 9),
                trigger: CompletionTrigger::Manual,
                provider_generation: 0,
                recent_completions: Vec::<String>::new().into_boxed_slice(),
            }
        }
    );
}

#[tokio::test]
async fn completion_request_carries_bounded_accept_recency() {
    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(42, &DocumentAccess::Editable { lease_id: 1 });
    queue.record_completion_accept("recent");
    queue
        .enqueue_completion_request(
            EditorCompletionRequestEvent {
                document_id: 4,
                document_version: 5,
                behavior_version: 6,
                cursor_byte_offset: 9,
                replacement_range: CompletionReplacementRange::new(7, 9),
                trigger: CompletionTrigger::Manual,
            },
            12,
        )
        .unwrap();
    let ClientMessage::CompletionRequest { request } = receiver.recv().await.unwrap() else {
        panic!("expected completion request");
    };
    assert_eq!(
        request
            .recent_completions
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["recent"]
    );
}

#[tokio::test]
async fn completion_after_local_edit_uses_optimistic_document_version() {
    let (queue, mut receiver) = ClientEditQueue::bounded(2);
    let queue = queue
        .with_authority(42, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(5);

    queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: 4,
                base_version: 5,
                behavior_version: 6,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "p".to_string(),
                },
            },
            10,
        )
        .unwrap();
    queue
        .enqueue_completion_request(
            EditorCompletionRequestEvent {
                document_id: 4,
                document_version: 5,
                behavior_version: 6,
                cursor_byte_offset: 1,
                replacement_range: CompletionReplacementRange::new(0, 1),
                trigger: CompletionTrigger::Manual,
            },
            11,
        )
        .unwrap();

    assert!(matches!(
        receiver.recv().await,
        Some(ClientMessage::Edit { .. })
    ));
    let Some(ClientMessage::CompletionRequest { request }) = receiver.recv().await else {
        panic!("expected completion request after edit");
    };
    assert_eq!(request.document_version, 6);
}

#[tokio::test]
async fn language_intelligence_request_is_enqueued_as_non_blocking_message() {
    use super::EditorLanguageIntelligenceRequestEvent;
    use crate::protocol::LanguageIntelligenceFeature;

    let (queue, mut receiver) = ClientEditQueue::bounded(1);
    let queue = queue.with_authority(42, &DocumentAccess::Editable { lease_id: 1 });

    queue
        .enqueue_language_intelligence_request(
            EditorLanguageIntelligenceRequestEvent {
                document_id: 4,
                document_version: 5,
                behavior_version: 6,
                cursor_byte_offset: 9,
                feature: LanguageIntelligenceFeature::Hover,
            },
            13,
        )
        .unwrap();

    assert_eq!(
        receiver.recv().await.unwrap(),
        ClientMessage::LanguageIntelligenceRequest {
            request: crate::protocol::LanguageIntelligenceRequest {
                request_id: 13,
                client_id: 42,
                document_id: 4,
                document_version: 5,
                behavior_version: 6,
                cursor_byte_offset: 9,
                feature: LanguageIntelligenceFeature::Hover,
                provider_generation: 0,
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

    let (edit_queue, outgoing) = ClientEditQueue::bounded(super::EDIT_QUEUE_CAPACITY);
    let edit_queue = edit_queue.with_authority(7, &DocumentAccess::ReadOnly);
    let sync_state = std::sync::Arc::clone(&edit_queue.sync_state);
    let file_open_capability = std::sync::Arc::clone(&edit_queue.file_open_capability);
    let behavior_state = std::sync::Arc::new(std::sync::Mutex::new(
        super::behavior::ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1))
            .unwrap(),
    ));
    let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(super::EDIT_QUEUE_CAPACITY);
    let connection = tokio::spawn(super::run_connection(
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

    let (edit_queue, outgoing) = ClientEditQueue::bounded(super::EDIT_QUEUE_CAPACITY);
    let edit_queue = edit_queue.with_authority(7, &DocumentAccess::ReadOnly);
    let sync_state = std::sync::Arc::clone(&edit_queue.sync_state);
    let file_open_capability = std::sync::Arc::clone(&edit_queue.file_open_capability);
    let behavior_state = std::sync::Arc::new(std::sync::Mutex::new(
        super::behavior::ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1))
            .unwrap(),
    ));
    let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(super::EDIT_QUEUE_CAPACITY);
    let connection = tokio::spawn(super::run_connection(
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
        ClientConnectionEvent::ResyncSnapshot(super::ClientResyncSnapshot {
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

#[tokio::test]
async fn client_forwards_document_opened_without_replacing_live_sync_state() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let metadata = crate::protocol::DocumentMetadata {
        document_id: 42,
        version: 5,
        access: DocumentAccess::Editable { lease_id: 8 },
        lease_id: Some(8),
        dirty: false,
        workspace_root_id: 77,
        path: "note.md".to_string(),
    };
    let expected_metadata = metadata.clone();
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
                    head: DocumentTextHead::complete("scratch".to_string()),
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
                    4,
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
                &ServerMessage::DocumentOpened {
                    metadata,
                    head: DocumentTextHead::complete("# opened\n".to_string()),
                },
            )
            .await
            .unwrap();
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();

    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::DocumentOpened {
            metadata: expected_metadata,
            head: DocumentTextHead::complete("# opened\n".to_string()),
        }
    );
    // Multi-document: connection layer forwards DocumentOpened only. The
    // editor widget retains the prior session and installs authority for
    // the newly active document, so live sync stays on the initial doc
    // until that install runs.
    let snapshot = session.edit_queue.sync_snapshot();
    assert_eq!(snapshot.confirmed_version, 3);
    assert_eq!(snapshot.optimistic_version, 3);
    assert!(snapshot.pending.is_empty());
    server_task.await.unwrap();
}

#[tokio::test]
async fn client_receives_file_operation_failed_event() {
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
                    4,
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
                &ServerMessage::FileOperationFailed {
                    code: FileErrorCode::InvalidUtf8,
                    message: "workspace file <requested path> is not valid UTF-8 text".to_string(),
                    workspace_root_id: None,
                    document_id: None,
                },
            )
            .await
            .unwrap();
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();

    assert!(matches!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::FileOperationFailed {
            code: FileErrorCode::InvalidUtf8,
            workspace_root_id: None,
            document_id: None,
            ..
        }
    ));
    server_task.await.unwrap();
}

#[tokio::test]
async fn client_receives_runtime_diagnostic_event() {
    let (client, mut server) = duplex(4096);
    let codec = Codec::default();
    let expected = RuntimeDiagnostic::error(
        "runtime.syntax_error",
        "JavaScript syntax error while evaluating server-side configuration.",
    );
    let mut live_typography = ActiveTypography {
        revision: 1,
        ..ActiveTypography::default()
    };
    live_typography.ui.size = 13.0;
    let server_task = tokio::spawn({
        let expected = expected.clone();
        let live_typography = live_typography.clone();
        async move {
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
                    &ServerMessage::BehaviorManifest(Box::new(
                        BehaviorManifest::minimal_text_editing(4),
                    )),
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
                    &ServerMessage::ActiveTypography(live_typography),
                )
                .await
                .unwrap();
            codec
                .write_server_message(&mut server, &ServerMessage::RuntimeDiagnostic(expected))
                .await
                .unwrap();
        }
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();

    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::ActiveTypography(live_typography)
    );
    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::RuntimeDiagnostic(expected)
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn client_installs_behavior_manifest_replacement_event() {
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
                    4,
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

    let mut session = connect_from_stream(client, codec).await.unwrap();

    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::BehaviorManifestInstalled {
            behavior_version: 5,
            manifest: BehaviorManifest::minimal_text_editing(5),
        }
    );
    server_task.await.unwrap();
}

fn sample_runtime_snapshot(
    generation: u64,
    client_id: u64,
) -> crate::protocol::RuntimeStateSnapshot {
    let snapshot = crate::protocol::RuntimeStateSnapshot {
        runtime_generation_id: generation,
        client_id,
        behavior: BehaviorManifest::minimal_text_editing(generation),
        active_theme: crate::protocol::ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: ActiveTypography::default(),
        active_design_system: crate::shell::design_system::ActiveDesignSystem::core_fallback(
            generation,
        ),
        active_icon_pack: None,
        ui_choices: crate::protocol::UiChoicesSnapshot::default(),
        sdui_tree: SduiTree {
            ui_version: generation,
            root_id: SduiNodeId(1),
            nodes: vec![SduiNode::new(
                SduiNodeId(1),
                SduiNodeKind::Label {
                    text: format!("gen-{generation}"),
                    icon: None,
                },
            )],
        },
        package_ui: crate::protocol::PackageUiSnapshot {
            version: generation,
            ..Default::default()
        },
        documents: Vec::new(),
        diagnostics: Vec::new(),
    };
    snapshot.validate().expect("fixture");
    snapshot
}

async fn write_minimal_bootstrap(
    codec: &Codec,
    server: &mut tokio::io::DuplexStream,
    client_id: u64,
) {
    let _hello = codec.read_client_message(server).await.unwrap();
    codec
        .write_server_message(
            server,
            &ServerMessage::Welcome {
                client_id,
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await
        .unwrap();
    codec
        .write_server_message(
            server,
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
            server,
            &ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(1))),
        )
        .await
        .unwrap();
    codec
        .write_server_message(
            server,
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
            server,
            &ServerMessage::ActiveTypography(ActiveTypography::default()),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn runtime_state_snapshot_is_staged_without_immediate_ack() {
    let (client, mut server) = duplex(8192);
    let codec = Codec::default();
    let snapshot = sample_runtime_snapshot(2, 1);
    let server_task = tokio::spawn({
        let snapshot = snapshot.clone();
        async move {
            write_minimal_bootstrap(&codec, &mut server, 1).await;
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::RuntimeStateSnapshot(Box::new(snapshot)),
                )
                .await
                .unwrap();
            // Receive loop must not acknowledge before editor install.
            match tokio::time::timeout(
                std::time::Duration::from_millis(50),
                codec.read_client_message(&mut server),
            )
            .await
            {
                Err(_) => {}
                Ok(Ok(message)) => {
                    panic!("unexpected client message before install: {message:?}")
                }
                Ok(Err(_)) => {}
            }
        }
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();
    match session.events.recv().await.unwrap() {
        ClientConnectionEvent::RuntimeStateSnapshot(received) => {
            assert_eq!(received.runtime_generation_id, 2);
            assert_eq!(received.client_id, 1);
        }
        other => panic!("expected staged runtime snapshot, got {other:?}"),
    }
    server_task.await.unwrap();
}

#[tokio::test]
async fn invalid_runtime_state_snapshot_fail_closes_without_ack_or_event() {
    let (client, mut server) = duplex(8192);
    let codec = Codec::default();
    let server_task = tokio::spawn({
        async move {
            write_minimal_bootstrap(&codec, &mut server, 1).await;
            let mut invalid = sample_runtime_snapshot(2, 1);
            invalid.behavior.manifest_id.clear();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::RuntimeStateSnapshot(Box::new(invalid)),
                )
                .await
                .unwrap();
            match tokio::time::timeout(
                std::time::Duration::from_millis(50),
                codec.read_client_message(&mut server),
            )
            .await
            {
                Err(_) => {}
                Ok(Ok(message)) => panic!("invalid snapshot must not acknowledge: {message:?}"),
                Ok(Err(_)) => {}
            }
        }
    });

    let mut session = connect_from_stream(client, codec).await.unwrap();
    match session.events.recv().await.unwrap() {
        ClientConnectionEvent::ConnectionError(message) => {
            assert!(message.contains("invalid runtime state snapshot"));
        }
        other => panic!("expected fail-closed connection error, got {other:?}"),
    }
    assert!(session.events.recv().await.is_none());
    server_task.await.unwrap();
}

#[tokio::test]
async fn client_rejects_invalid_behavior_manifest_replacement_event() {
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
                    4,
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
        let mut invalid = BehaviorManifest::minimal_text_editing(5);
        invalid
            .commands
            .push(CommandDeclaration::client_edit("text.insert", "Duplicate"));
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::BehaviorManifest(Box::new(invalid)),
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

    let mut session = connect_from_stream(client, codec).await.unwrap();

    assert!(matches!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::BehaviorManifestRejected {
            behavior_version: 5,
            ..
        }
    ));
    server_task.await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn end_to_end_second_client_gets_independent_welcome_document() {
    let socket_path = unique_socket_path("read-only");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    let first = connect_with_retry(&socket_path).await;
    let second = connect_with_retry(&socket_path).await;
    let startup_root = std::fs::canonicalize(std::env::current_dir().unwrap())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(first.initial_state.workspace_root, startup_root);
    assert_eq!(second.initial_state.workspace_root, startup_root);

    assert!(matches!(
        first.initial_state.access,
        DocumentAccess::Editable { lease_id: 1 }
    ));
    assert!(matches!(
        second.initial_state.access,
        DocumentAccess::Editable { lease_id: 1 }
    ));
    assert_ne!(
        first.initial_state.document_id, second.initial_state.document_id,
        "per-tab welcome documents are independent"
    );

    drop(first);
    drop(second);
    server_task.abort();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn real_server_end_to_end_edit_gets_acknowledged() {
    let socket_path = unique_socket_path("ack");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    let mut session = connect_with_retry(&socket_path).await;
    session
        .edit_queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: session.initial_state.document_id,
                base_version: session.initial_state.document_version,
                behavior_version: session.initial_state.behavior_manifest.behavior_version,
                operation: EditOperation::Insert {
                    byte_offset: session.initial_state.head.first_chunk.len() as u64,
                    text: "manual".to_string(),
                },
            },
            77,
        )
        .unwrap();

    let mut event = session.events.recv().await.unwrap();
    // Handshake extras and asynchronous server broadcasts (SDUI snapshot,
    // runtime caret override, the mode-activation document-layer manifest,
    // theme/typography fanout, parse decorations) are legitimate at any
    // point; skip past them to the event under test.
    while matches!(
        event,
        ClientConnectionEvent::SduiSnapshot { .. }
            | ClientConnectionEvent::RuntimeStateSnapshot(_)
            | ClientConnectionEvent::CaretStyleOverride(_)
            | ClientConnectionEvent::EditorLayoutOverride(_)
            | ClientConnectionEvent::ShellPreferences(_)
            | ClientConnectionEvent::TabRegistry(_)
            | ClientConnectionEvent::BehaviorManifestInstalled { .. }
            | ClientConnectionEvent::ActiveTheme(_)
            | ClientConnectionEvent::ActiveTypography(_)
            | ClientConnectionEvent::RuntimeDiagnostic(_)
            | ClientConnectionEvent::DecorationSet(_)
            | ClientConnectionEvent::DecorationBatch(_)
            | ClientConnectionEvent::DiagnosticSet(_)
            | ClientConnectionEvent::FoldingRangeSet(_)
            | ClientConnectionEvent::ViewportRenderPatch(_)
    ) {
        event = session.events.recv().await.unwrap();
    }

    assert_eq!(
        event,
        ClientConnectionEvent::EditAck {
            document_id: session.initial_state.document_id,
            version: session.initial_state.document_version + 1,
            transaction_id: 77,
        }
    );

    server_task.abort();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn selected_file_edit_then_save_persists_and_reports_clean() {
    let socket_path = unique_socket_path("selected-save");
    let file_path = socket_path.parent().unwrap().join("save.rs");
    let config_root = socket_path.parent().unwrap().join("config");
    fs::create_dir(&config_root).unwrap();
    fs::write(&file_path, "fn main() {}\n").unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
import { bindKey } from "clay:keybindings";
bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
"#,
    )
    .unwrap();
    let mut config = ServerConfig::new(&socket_path);
    config.configuration_root = Some(config_root.clone());
    let server = IpcServer::new(config);
    let server_task = tokio::spawn(server.run());

    let mut session = connect_with_retry(&socket_path).await;
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if session
                .edit_queue
                .file_open_capability
                .lock()
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("file-open capability timed out");
    session
        .edit_queue
        .enqueue_open_selected_file(file_path.clone())
        .unwrap();

    let (metadata, behavior_manifest) =
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let metadata = loop {
                let event = session.events.recv().await.unwrap();
                if let ClientConnectionEvent::DocumentOpened { metadata, .. } = event {
                    break metadata;
                }
            };
            let behavior_manifest = loop {
                let event = session.events.recv().await.unwrap();
                if let ClientConnectionEvent::BehaviorManifestInstalled { manifest, .. } = event {
                    break manifest;
                }
            };
            (metadata, behavior_manifest)
        })
        .await
        .expect("selected file open timed out");
    assert!(
        behavior_manifest
            .keymaps
            .iter()
            .any(|rule| { rule.command_id == "documents.serverSaveDocument" }),
        "selected-file activation must preserve configured save binding"
    );
    let behavior_version = behavior_manifest.behavior_version;
    session.edit_queue.update_opened_document_authority(
        metadata.document_id,
        &metadata.access,
        metadata.version,
    );
    let inserted = "// saved\n";
    session
        .edit_queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: metadata.document_id,
                base_version: metadata.version,
                behavior_version,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: inserted.to_string(),
                },
            },
            91,
        )
        .unwrap();
    session
        .edit_queue
        .enqueue_save_document(metadata.document_id, metadata.version + 1)
        .unwrap();

    let saved = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let event = session.events.recv().await.unwrap();
            if let ClientConnectionEvent::DocumentSaved {
                document_id,
                version,
                dirty,
            } = event
            {
                break (document_id, version, dirty);
            }
        }
    })
    .await
    .expect("selected file save timed out");

    assert_eq!(saved, (metadata.document_id, metadata.version + 1, false));
    assert_eq!(
        fs::read_to_string(&file_path).unwrap(),
        format!("{inserted}fn main() {{}}\n")
    );

    server_task.abort();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_file(config_root.join("init.js"));
    let _ = fs::remove_dir(&config_root);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[cfg(windows)]
#[tokio::test]
async fn windows_named_pipe_client_receives_initial_snapshot() {
    let endpoint = unique_named_pipe("snapshot");
    let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
    let server_task = tokio::spawn(server.run());

    let session = connect_with_retry(&endpoint).await;

    assert_eq!(session.initial_state.head.first_chunk, "");
    assert!(matches!(
        session.initial_state.access,
        DocumentAccess::Editable { lease_id: 1 }
    ));
    // The ambient default configuration (e.g. ~/.config/clay/init.js) may
    // publish a behavior manifest, so the exact version is not fixed.
    assert!(
        session.initial_state.behavior_manifest.behavior_version >= 1,
        "expected a loaded behavior manifest, got version {}",
        session.initial_state.behavior_manifest.behavior_version
    );

    server_task.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn windows_named_pipe_edit_gets_acknowledged() {
    let endpoint = unique_named_pipe("ack");
    let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
    let server_task = tokio::spawn(server.run());

    let mut session = connect_with_retry(&endpoint).await;
    session
        .edit_queue
        .enqueue_edit_event(
            EditorEditEvent {
                document_id: session.initial_state.document_id,
                base_version: session.initial_state.document_version,
                behavior_version: session.initial_state.behavior_manifest.behavior_version,
                operation: EditOperation::Insert {
                    byte_offset: session.initial_state.head.first_chunk.len() as u64,
                    text: "pipe".to_string(),
                },
            },
            88,
        )
        .unwrap();

    let mut event = session.events.recv().await.unwrap();
    // Handshake extras (SDUI snapshot, runtime caret override) are
    // legitimate at any point; skip past them to the event under test.
    while matches!(
        event,
        ClientConnectionEvent::SduiSnapshot { .. }
            | ClientConnectionEvent::CaretStyleOverride(_)
            | ClientConnectionEvent::EditorLayoutOverride(_)
            | ClientConnectionEvent::ShellPreferences(_)
            | ClientConnectionEvent::TabRegistry(_)
    ) {
        event = session.events.recv().await.unwrap();
    }

    assert_eq!(
        event,
        ClientConnectionEvent::EditAck {
            document_id: session.initial_state.document_id,
            version: session.initial_state.document_version + 1,
            transaction_id: 88,
        }
    );

    server_task.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn windows_second_client_gets_independent_welcome_document() {
    let endpoint = unique_named_pipe("read-only");
    let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
    let server_task = tokio::spawn(server.run());

    let first = connect_with_retry(&endpoint).await;
    let second = connect_with_retry(&endpoint).await;

    assert!(matches!(
        first.initial_state.access,
        DocumentAccess::Editable { lease_id: 1 }
    ));
    assert!(matches!(
        second.initial_state.access,
        DocumentAccess::Editable { lease_id: 1 }
    ));
    assert_ne!(
        first.initial_state.document_id, second.initial_state.document_id,
        "per-tab welcome documents are independent"
    );

    server_task.abort();
}

#[cfg(windows)]
#[tokio::test]
async fn windows_named_pipe_stale_edit_rejected_then_resynced() {
    let endpoint = unique_named_pipe("stale-resync");
    let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
    let server_task = tokio::spawn(server.run());

    let mut stream = {
        let mut last_error = None;
        let mut stream = None;
        for _ in 0..50 {
            match connect_transport(&endpoint).await {
                Ok(connected) => {
                    stream = Some(connected);
                    break;
                }
                Err(error) => {
                    last_error = Some(error);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
        stream.unwrap_or_else(|| panic!("failed to connect to test named pipe: {last_error:?}"))
    };
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "stale-test".to_string(),
            },
        )
        .await
        .unwrap();

    let client_id = match codec.read_server_message(&mut stream).await.unwrap() {
        ServerMessage::Welcome { client_id, .. } => client_id,
        message => panic!("expected Welcome, got {message:?}"),
    };
    let server_behavior_version = match codec.read_server_message(&mut stream).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
        ServerMessage::ActiveTheme(_) => panic!("expected BehaviorManifest before ActiveTheme"),
        message => panic!("expected BehaviorManifest, got {message:?}"),
    };
    let _active_theme = codec.read_server_message(&mut stream).await.unwrap();
    let _active_typography = codec.read_server_message(&mut stream).await.unwrap();
    loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::TabRegistry(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected file-open capability, got {message:?}"),
        }
    }
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::TabCommand {
                client_id,
                command: crate::protocol::TabCommand::New {
                    workspace_root: String::new(),
                },
            },
        )
        .await
        .unwrap();
    let (document_id, version, text, lease_id) = loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::InitialDocument {
                document_id,
                version,
                head,
                access: DocumentAccess::Editable { lease_id },
                lease_id: Some(snapshot_lease_id),
                workspace_root: _,
            } => {
                assert_eq!(lease_id, snapshot_lease_id);
                break (document_id, version, head.first_chunk, lease_id);
            }
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::TabRegistry(_)
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected editable InitialDocument, got {message:?}"),
        }
    };

    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Edit {
                document_id,
                client_id,
                lease_id: Some(lease_id),
                base_version: version - 1,
                behavior_version: server_behavior_version,
                transaction_id: 99,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "stale".to_string(),
                },
            },
        )
        .await
        .unwrap();

    let reason = loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::EditRejected {
                document_id: rejected_document_id,
                transaction_id: 99,
                reason,
            } if rejected_document_id == document_id => break reason,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::FileOpenCapabilityIssued { .. }
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::TabRegistry(_)
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => continue,
            message => panic!("expected EditRejected, got {message:?}"),
        }
    };

    assert_eq!(
        reason,
        EditRejection::StaleVersion {
            client_base_version: version - 1,
            server_version: version,
        }
    );

    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::RequestResync {
                document_id,
                client_id,
                known_version: version - 1,
            },
        )
        .await
        .unwrap();

    loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::ResyncSnapshot {
                document_id: resynced_document_id,
                version: resynced_version,
                head: resynced_head,
                access:
                    DocumentAccess::Editable {
                        lease_id: resynced_lease_id,
                    },
                lease_id: Some(resynced_snapshot_lease_id),
            } => {
                assert_eq!(resynced_document_id, document_id);
                assert_eq!(resynced_version, version);
                assert_eq!(resynced_head.first_chunk, text);
                assert_eq!(resynced_lease_id, lease_id);
                assert_eq!(resynced_snapshot_lease_id, lease_id);
                break;
            }
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::FileOpenCapabilityIssued { .. }
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::TabRegistry(_)
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected ResyncSnapshot, got {message:?}"),
        }
    }

    server_task.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn real_server_tab_command_new_registers_and_rejected_activate_pushes_reconcile() {
    let socket_path = unique_socket_path("tabcmd");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());
    let workspace_root = std::env::temp_dir().join(format!("clay-tabcmd-{}", std::process::id()));
    tokio::fs::create_dir_all(&workspace_root).await.unwrap();
    let mut session =
        connect_with_workspace_root(&socket_path, workspace_root.to_string_lossy().into_owned())
            .await
            .unwrap();
    let client_id = session.initial_state.client_id;

    let mut event = session.events.recv().await.unwrap();
    // The handshake replays the (empty) registry snapshot; skip it and
    // wait for the snapshot carrying our `New` registration.
    loop {
        if matches!(&event, ClientConnectionEvent::TabRegistry(snapshot) if !snapshot.tabs.is_empty())
        {
            break;
        }
        event = session.events.recv().await.unwrap();
    }
    let ClientConnectionEvent::TabRegistry(snapshot) = event else {
        unreachable!("matched TabRegistry")
    };
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].client_id, client_id);
    assert_eq!(
        snapshot.tabs[0].workspace_root,
        workspace_root.to_string_lossy().into_owned()
    );
    let created_tab_id = snapshot.tabs[0].tab_id;
    assert_eq!(snapshot.active, Some(created_tab_id));

    // A rejected `Activate` (unknown tab) still pushes the current
    // snapshot: the optimistic client reverts to the server's active tab.
    session
        .edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::Activate { tab_id: 999 })
        .unwrap();
    let mut event = session.events.recv().await.unwrap();
    // The handshake replays the (empty) registry snapshot; skip it and
    // wait for the snapshot carrying our `New` registration.
    loop {
        if matches!(&event, ClientConnectionEvent::TabRegistry(snapshot) if !snapshot.tabs.is_empty())
        {
            break;
        }
        event = session.events.recv().await.unwrap();
    }
    let ClientConnectionEvent::TabRegistry(snapshot) = event else {
        unreachable!("matched TabRegistry")
    };
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(
        snapshot.active,
        Some(created_tab_id),
        "reject leaves active unchanged"
    );

    server_task.abort();
    let _ = tokio::fs::remove_dir_all(&workspace_root).await;
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[tokio::test]
async fn real_server_tab_close_ends_connection_and_removes_registry_entry() {
    let socket_path = unique_socket_path("tabclose");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());
    let workspace_root = std::env::temp_dir().join(format!("clay-tabclose-{}", std::process::id()));
    tokio::fs::create_dir_all(&workspace_root).await.unwrap();
    let mut session =
        connect_with_workspace_root(&socket_path, workspace_root.to_string_lossy().into_owned())
            .await
            .unwrap();

    // Wait for the registration snapshot and grab the tab id.
    let tab_id = loop {
        let event = session.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 1
        {
            break snapshot.tabs[0].tab_id;
        }
    };

    // Close: the server removes the registry entry and ends the
    // connection (permit + leases release via the disconnect cleanup).
    // The closing connection never reads its own broadcast update (the
    // handler returns before the select loop can), so removal is observed
    // on a fresh connection's handshake replay.
    session
        .edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::Close { tab_id })
        .unwrap();
    let mut saw_disconnect = false;
    for _ in 0..4 {
        if matches!(
            session.events.recv().await,
            Some(ClientConnectionEvent::Disconnected)
        ) {
            saw_disconnect = true;
            break;
        }
    }
    assert!(saw_disconnect, "close must end the connection");

    let mut fresh = connect_stream_with_retry(&socket_path).await;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut fresh,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "close-replay".to_string(),
            },
        )
        .await
        .unwrap();
    let replayed = loop {
        match codec.read_server_message(&mut fresh).await.unwrap() {
            ServerMessage::TabRegistry(snapshot) => break snapshot.tabs,
            ServerMessage::Welcome { .. } => {}
            ServerMessage::FileOpenCapabilityIssued { .. }
            | ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected registry replay, got {message:?}"),
        }
    };
    drop(fresh);
    assert!(
        replayed.is_empty(),
        "close must remove the registry entry (replay on a fresh connection)"
    );

    server_task.abort();
    let _ = tokio::fs::remove_dir_all(&workspace_root).await;
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[tokio::test]
async fn real_server_tab_move_commands_reorder_broadcast_and_reject() {
    let socket_path = unique_socket_path("tabmove");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    // Two tabs on two connections (distinct roots). Registry order after
    // setup: [alpha, beta], active = beta.
    let alpha_root =
        std::env::temp_dir().join(format!("clay-tabmove-alpha-{}", std::process::id()));
    tokio::fs::create_dir_all(&alpha_root).await.unwrap();
    let mut alpha =
        connect_with_workspace_root(&socket_path, alpha_root.to_string_lossy().into_owned())
            .await
            .unwrap();
    let alpha_tab = loop {
        let event = alpha.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 1
        {
            break snapshot.tabs[0].tab_id;
        }
    };

    let beta_root = std::env::temp_dir().join(format!("clay-tabmove-beta-{}", std::process::id()));
    tokio::fs::create_dir_all(&beta_root).await.unwrap();
    let mut beta =
        connect_with_workspace_root(&socket_path, beta_root.to_string_lossy().into_owned())
            .await
            .unwrap();
    let (beta_tab, setup_active) = loop {
        let event = beta.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 2
        {
            break (snapshot.tabs[1].tab_id, snapshot.active);
        }
    };
    assert_ne!(alpha_tab, beta_tab);
    assert_eq!(setup_active, Some(beta_tab));

    // A move mutation broadcasts to every connection: alpha's own
    // connection observes its MoveRight as the new order, and the active
    // tab keeps its status.
    alpha
        .edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::MoveRight { tab_id: alpha_tab })
        .unwrap();
    let (order, active) = loop {
        let event = alpha.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs[0].tab_id == beta_tab
        {
            break (
                snapshot
                    .tabs
                    .iter()
                    .map(|entry| entry.tab_id)
                    .collect::<Vec<_>>(),
                snapshot.active,
            );
        }
    };
    assert_eq!(order, vec![beta_tab, alpha_tab]);
    assert_eq!(active, Some(beta_tab), "moves preserve the active tab");

    // Drain beta's stream past the same broadcast so the next registry
    // event on it is the no-op broadcast, not a stale one.
    loop {
        let event = beta.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs[0].tab_id == beta_tab
        {
            break;
        }
    }

    // Boundary no-op (beta is already first): still broadcasts, order
    // unchanged — no wraparound.
    beta.edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::MoveLeft { tab_id: beta_tab })
        .unwrap();
    let (order, _) = loop {
        let event = beta.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 2
        {
            break (
                snapshot
                    .tabs
                    .iter()
                    .map(|entry| entry.tab_id)
                    .collect::<Vec<_>>(),
                snapshot.active,
            );
        }
    };
    assert_eq!(order, vec![beta_tab, alpha_tab]);

    // Rejected MoveTo (position 3 with 2 tabs): broadcasts the unchanged
    // registry so the executing client reconciles.
    alpha
        .edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::MoveTo {
            tab_id: alpha_tab,
            position: 3,
        })
        .unwrap();
    let (order, _) = loop {
        let event = alpha.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 2
        {
            break (
                snapshot
                    .tabs
                    .iter()
                    .map(|entry| entry.tab_id)
                    .collect::<Vec<_>>(),
                snapshot.active,
            );
        }
    };
    assert_eq!(order, vec![beta_tab, alpha_tab]);

    // Foreign-client move (beta moves alpha's tab): rejected, unchanged.
    beta.edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::MoveLeft { tab_id: alpha_tab })
        .unwrap();
    let (order, _) = loop {
        let event = beta.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 2
        {
            break (
                snapshot
                    .tabs
                    .iter()
                    .map(|entry| entry.tab_id)
                    .collect::<Vec<_>>(),
                snapshot.active,
            );
        }
    };
    assert_eq!(order, vec![beta_tab, alpha_tab]);

    // Valid MoveTo reorders and preserves the active tab: [alpha, beta].
    alpha
        .edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::MoveTo {
            tab_id: alpha_tab,
            position: 1,
        })
        .unwrap();
    let (order, active) = loop {
        let event = alpha.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs[0].tab_id == alpha_tab
        {
            break (
                snapshot
                    .tabs
                    .iter()
                    .map(|entry| entry.tab_id)
                    .collect::<Vec<_>>(),
                snapshot.active,
            );
        }
    };
    assert_eq!(order, vec![alpha_tab, beta_tab]);
    assert_eq!(active, Some(beta_tab));

    // Handshake replay carries the reordered registry to a fresh
    // unbound connection. It must not create a new tab just to observe
    // the replay.
    let mut fresh = connect_stream_with_retry(&socket_path).await;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut fresh,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "move-replay".to_string(),
            },
        )
        .await
        .unwrap();
    let replayed = loop {
        match codec.read_server_message(&mut fresh).await.unwrap() {
            ServerMessage::TabRegistry(snapshot) => break snapshot.tabs,
            ServerMessage::Welcome { .. } => {}
            ServerMessage::FileOpenCapabilityIssued { .. }
            | ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected registry replay, got {message:?}"),
        }
    };
    drop(fresh);
    assert_eq!(replayed.len(), 2);
    assert_eq!(replayed[0].tab_id, alpha_tab);
    assert_eq!(replayed[1].tab_id, beta_tab);

    server_task.abort();
    let _ = tokio::fs::remove_dir_all(&alpha_root).await;
    let _ = tokio::fs::remove_dir_all(&beta_root).await;
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

/// Phase 22.5: the client restore flow's server-side shape — sequential
/// `TabCommand::New` in persisted order (the restore mounts exactly
/// these commands) yields a registry whose order equals the persisted
/// order with no `MoveTo`; per-pane `OpenDocument` reopens land in the
/// tab's own root; a missing workspace root is rejected without a tab
/// (the backstop behind the client's is_dir pre-check and restore
/// deadline); the persisted active tab activates via `Activate`.
#[cfg(unix)]
#[tokio::test]
async fn real_server_restore_sequence_orders_tabs_and_opens_documents() {
    let socket_path = unique_socket_path("tabrestore");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    // Three roots, each with a document the restore reopens.
    let mut roots = Vec::new();
    for name in ["restore-alpha", "restore-beta", "restore-gamma"] {
        let root = std::env::temp_dir().join(format!("clay-{name}-{}", std::process::id()));
        tokio::fs::create_dir_all(&root).await.unwrap();
        tokio::fs::write(root.join("notes.md"), "# restored\n")
            .await
            .unwrap();
        roots.push(root);
    }

    // Persisted order [alpha, beta, gamma]: each connection binds its
    // selected root before returning its session.
    eprintln!("step 1: connect alpha");
    let mut alpha =
        connect_with_workspace_root(&socket_path, roots[0].to_string_lossy().into_owned())
            .await
            .unwrap();
    let mut beta =
        connect_with_workspace_root(&socket_path, roots[1].to_string_lossy().into_owned())
            .await
            .unwrap();
    let _gamma = connect_with_workspace_root(&socket_path, roots[2].to_string_lossy().into_owned())
        .await
        .unwrap();
    assert_eq!(
        alpha.initial_state.workspace_root,
        roots[0].to_string_lossy()
    );
    assert_eq!(
        beta.initial_state.workspace_root,
        roots[1].to_string_lossy()
    );
    // The workspace browser is visible by default: the bind snapshot
    // carries the root's listing (toggle mechanics are pinned server-side
    // by deferred_initial_state_waits_for_tab_binding).
    let initial_alpha_browser = loop {
        if let ClientConnectionEvent::SduiSnapshot { tree, .. } = alpha.events.recv().await.unwrap()
        {
            break tree;
        }
    };
    assert!(initial_alpha_browser.nodes.iter().any(|node| matches!(
        &node.kind,
        SduiNodeKind::List { items } if items.iter().any(|item| item.label == "notes.md")
    )));

    // Registry order equals persisted order — no `MoveTo` needed.
    let snapshot = loop {
        if let ClientConnectionEvent::TabRegistry(snapshot) = alpha.events.recv().await.unwrap()
            && snapshot.tabs.len() == 3
        {
            break snapshot;
        }
    };
    let alpha_client = alpha.initial_state.client_id;
    let alpha_entry = snapshot
        .tabs
        .iter()
        .find(|entry| entry.client_id == alpha_client)
        .expect("alpha registry entry");
    let order = snapshot
        .tabs
        .iter()
        .map(|entry| entry.workspace_root.clone())
        .collect::<Vec<_>>();
    let alpha_root_id = alpha_entry.workspace_root_id;
    let alpha_tab = alpha_entry.tab_id;
    let expected = roots
        .iter()
        .map(|root| root.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(order, expected);

    // Per-pane document reopen: the exact `OpenDocument` the restore
    // enqueues lands in the tab's own root, workspace-relative path
    // echoed back (the pending-open attribution match key).
    alpha
        .edit_queue
        .enqueue_open_document(alpha_root_id, "notes.md".to_string())
        .unwrap();
    let opened = loop {
        let event = alpha.events.recv().await.unwrap();
        if let ClientConnectionEvent::DocumentOpened { metadata, .. } = &event {
            break metadata.clone();
        }
    };
    assert_eq!(opened.path, "notes.md");
    assert_eq!(opened.workspace_root_id, alpha_root_id);

    // A missing workspace root is rejected: `FileOperationFailed` answer
    // and no registry entry (the client's is_dir pre-check normally
    // prevents this; the rejection feeds the restore deadline).
    let missing = std::env::temp_dir().join(format!("clay-restore-missing-{}", std::process::id()));
    beta.edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::New {
            workspace_root: missing.to_string_lossy().into_owned(),
        })
        .unwrap();
    loop {
        let event = beta.events.recv().await.unwrap();
        if matches!(&event, ClientConnectionEvent::FileOperationFailed { .. }) {
            break;
        }
    }

    // The persisted active tab (alpha) activates via the shared path.
    alpha
        .edit_queue
        .enqueue_tab_command(crate::protocol::TabCommand::Activate { tab_id: alpha_tab })
        .unwrap();
    let active = loop {
        let event = alpha.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 3
        {
            break snapshot.active;
        }
    };
    assert_eq!(active, Some(alpha_tab));

    server_task.abort();
    for root in &roots {
        let _ = tokio::fs::remove_dir_all(root).await;
    }
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[tokio::test]
async fn real_server_reconnect_reclaims_tab_binding() {
    let socket_path = unique_socket_path("tabreclaim");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());
    let workspace_root =
        std::env::temp_dir().join(format!("clay-tabreclaim-{}", std::process::id()));
    tokio::fs::create_dir_all(&workspace_root).await.unwrap();

    // First connection binds the tab during its handshake.
    let mut first_session =
        connect_with_workspace_root(&socket_path, workspace_root.to_string_lossy().into_owned())
            .await
            .unwrap();
    let first_client_id = first_session.initial_state.client_id;
    let first_document_id = first_session.initial_state.document_id;
    let first_workspace_root = first_session.initial_state.workspace_root.clone();
    let tab_id = loop {
        let event = first_session.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 1
        {
            break snapshot.tabs[0].tab_id;
        }
    };

    // The connection drops (no Close): the in-memory registry entry
    // survives with the old client binding.
    drop(first_session);

    // A reconnecting client (new ClientId) reclaims the same TabId during
    // its handshake.
    let mut second_session = connect_for_reclaim(&socket_path, tab_id).await.unwrap();
    let second_client_id = second_session.initial_state.client_id;
    assert_ne!(first_client_id, second_client_id);
    assert_eq!(second_session.initial_state.document_id, first_document_id);
    assert_eq!(
        second_session.initial_state.workspace_root,
        first_workspace_root
    );
    // The handshake replay already carries the surviving entry (bound to
    // the old client id); the rebind snapshot is the one whose entry names
    // THIS connection.
    let rebound = loop {
        let event = second_session.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = &event
            && snapshot.tabs.len() == 1
            && snapshot.tabs[0].client_id == second_client_id
        {
            break snapshot.tabs[0].clone();
        }
    };
    assert_eq!(rebound.tab_id, tab_id, "reclaim keeps the tab identity");
    assert_eq!(
        rebound.client_id, second_client_id,
        "reclaim rebinds the client"
    );
    assert_eq!(
        rebound.workspace_root,
        workspace_root.to_string_lossy().into_owned()
    );

    server_task.abort();
    let _ = tokio::fs::remove_dir_all(&workspace_root).await;
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn real_server_restart_rebuilds_reconnect_from_persisted_workspace_root() {
    let socket_path = unique_socket_path("tab-restart-reconnect");
    let workspace_root = socket_path.parent().unwrap().join("persisted-root");
    tokio::fs::create_dir_all(&workspace_root).await.unwrap();

    let first_server = IpcServer::new(ServerConfig::new(&socket_path));
    let first_server_task = tokio::spawn(first_server.run());
    let mut first_session = loop {
        match connect_with_workspace_root(
            &socket_path,
            workspace_root.to_string_lossy().into_owned(),
        )
        .await
        {
            Ok(session) => break session,
            Err(error) if error.kind() == super::ClientBootstrapErrorKind::TransportUnavailable => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Err(error) => panic!("initial server connection failed: {error}"),
        }
    };
    let old_tab_id = loop {
        let event = first_session.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = event
            && snapshot.tabs.len() == 1
        {
            break snapshot.tabs[0].tab_id;
        }
    };
    drop(first_session);
    first_server_task.abort();
    let _ = fs::remove_file(&socket_path);

    // The new server has an empty in-memory registry. Reclaim must not
    // resurrect that stale entry; the reconnect helper rebuilds a fresh
    // tab from the persisted root with `New`.
    let second_server = IpcServer::new(ServerConfig::new(&socket_path));
    let second_server_task = tokio::spawn(second_server.run());
    let endpoint = IpcEndpoint::from(&socket_path);
    let mut second_session = loop {
        match connect_for_reclaim_or_new(
            &endpoint,
            old_tab_id,
            workspace_root.to_string_lossy().into_owned(),
        )
        .await
        {
            Ok(session) => break session,
            Err(error) if error.kind() == super::ClientBootstrapErrorKind::TransportUnavailable => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Err(error) => panic!("server restart reconnect failed: {error}"),
        }
    };
    assert_eq!(
        second_session.initial_state.workspace_root,
        fs::canonicalize(&workspace_root)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    );
    let rebound = loop {
        let event = second_session.events.recv().await.unwrap();
        if let ClientConnectionEvent::TabRegistry(snapshot) = event
            && snapshot.tabs.len() == 1
        {
            break snapshot.tabs[0].clone();
        }
    };
    assert_eq!(
        rebound.workspace_root,
        workspace_root.to_string_lossy().into_owned()
    );

    drop(second_session);
    second_server_task.abort();
    let _ = fs::remove_dir_all(socket_path.parent().unwrap());
}

#[tokio::test]
async fn real_server_connection_cap_refuses_excess_connections() {
    let socket_path = unique_socket_path("tabcap");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    // Fill the connection cap with idle sessions; each occupies a permit.
    let mut sessions = Vec::new();
    for _ in 0..crate::perf::budgets::MAX_ACTIVE_CONNECTIONS {
        sessions.push(connect_with_retry(&socket_path).await);
    }
    // The next connect is refused gracefully (the server drops the
    // stream): the open-tab path surfaces this as a diagnostic and never
    // mounts a tab.
    let refused = connect(&IpcEndpoint::from(&socket_path)).await;
    assert!(
        refused.is_err(),
        "the connection cap must refuse excess connections"
    );

    drop(sessions);
    server_task.abort();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[tokio::test]
async fn real_server_end_to_end_stale_edit_rejected_then_resynced() {
    let socket_path = unique_socket_path("stale-resync");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    let mut stream = connect_stream_with_retry(&socket_path).await;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "stale-test".to_string(),
            },
        )
        .await
        .unwrap();

    let client_id = match codec.read_server_message(&mut stream).await.unwrap() {
        ServerMessage::Welcome { client_id, .. } => client_id,
        message => panic!("expected Welcome, got {message:?}"),
    };
    let server_behavior_version = match codec.read_server_message(&mut stream).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
        ServerMessage::ActiveTheme(_) => panic!("expected BehaviorManifest before ActiveTheme"),
        message => panic!("expected BehaviorManifest, got {message:?}"),
    };
    let _active_theme = codec.read_server_message(&mut stream).await.unwrap();
    let _active_typography = codec.read_server_message(&mut stream).await.unwrap();
    loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::TabRegistry(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected file-open capability, got {message:?}"),
        }
    }
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::TabCommand {
                client_id,
                command: crate::protocol::TabCommand::New {
                    workspace_root: String::new(),
                },
            },
        )
        .await
        .unwrap();
    let (document_id, version, text, lease_id) = loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::InitialDocument {
                document_id,
                version,
                head,
                access: DocumentAccess::Editable { lease_id },
                lease_id: Some(snapshot_lease_id),
                workspace_root: _,
            } => {
                assert_eq!(lease_id, snapshot_lease_id);
                break (document_id, version, head.first_chunk, lease_id);
            }
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::TabRegistry(_)
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected editable InitialDocument, got {message:?}"),
        }
    };

    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Edit {
                document_id,
                client_id,
                lease_id: Some(lease_id),
                base_version: version - 1,
                behavior_version: server_behavior_version,
                transaction_id: 99,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "stale".to_string(),
                },
            },
        )
        .await
        .unwrap();

    let reason = loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::EditRejected {
                document_id: rejected_document_id,
                transaction_id: 99,
                reason,
            } if rejected_document_id == document_id => break reason,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::FileOpenCapabilityIssued { .. }
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::TabRegistry(_)
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => continue,
            message => panic!("expected EditRejected, got {message:?}"),
        }
    };

    assert_eq!(
        reason,
        EditRejection::StaleVersion {
            client_base_version: version - 1,
            server_version: version,
        }
    );

    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::RequestResync {
                document_id,
                client_id,
                known_version: version - 1,
            },
        )
        .await
        .unwrap();

    loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::ResyncSnapshot {
                document_id: resynced_document_id,
                version: resynced_version,
                head: resynced_head,
                access:
                    DocumentAccess::Editable {
                        lease_id: resynced_lease_id,
                    },
                lease_id: Some(resynced_snapshot_lease_id),
            } => {
                assert_eq!(resynced_document_id, document_id);
                assert_eq!(resynced_version, version);
                assert_eq!(resynced_head.first_chunk, text);
                assert_eq!(resynced_lease_id, lease_id);
                assert_eq!(resynced_snapshot_lease_id, lease_id);
                break;
            }
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::FileOpenCapabilityIssued { .. }
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::TabRegistry(_)
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected ResyncSnapshot, got {message:?}"),
        }
    }

    server_task.abort();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[tokio::test]
async fn end_to_end_edit_gets_acknowledged() {
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
                    version: 1,
                    head: DocumentTextHead::complete("Hi".to_string()),
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
                    1,
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

        assert_eq!(
            codec.read_client_message(&mut server).await.unwrap(),
            ClientMessage::Edit {
                document_id: 7,
                client_id: 1,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 9,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: "!".to_string()
                }
            }
        );
        codec
            .write_server_message(
                &mut server,
                &ServerMessage::EditAck {
                    document_id: 7,
                    confirmed_version: 2,
                    transaction_id: 9,
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
                base_version: 1,
                behavior_version: 1,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: "!".to_string(),
                },
            },
            9,
        )
        .unwrap();

    assert_eq!(
        session.events.recv().await.unwrap(),
        ClientConnectionEvent::EditAck {
            document_id: 7,
            version: 2,
            transaction_id: 9,
        }
    );
    server_task.await.unwrap();
}
