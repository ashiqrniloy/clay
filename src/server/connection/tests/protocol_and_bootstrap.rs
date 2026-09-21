use super::*;

/// Plan 060 T4 (P0-2): save requires the editable lease and validates
/// `known_version`; status and list fail closed for documents the
/// connection never opened.
#[tokio::test]
async fn save_reload_status_list_enforce_connection_owned_access() {
    let root = temp_workspace("save-access");
    fs::write(root.join("note.md"), "hello\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();

    let mut connection_a = TestConnection::connect(
        99,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation.clone(),
        parse_coordinator.clone(),
        document_analysis.clone(),
        language_intelligence_coordinator(),
    )
    .await;
    let mut connection_b = TestConnection::connect(
        100,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;

    // A opens the document (editable lease).
    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    loop {
        match connection_a.receive().await {
            ServerMessage::DocumentOpened { .. } => break,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during A open: {other:?}"),
        }
    }
    connection_a.drain_until_quiet().await;

    // B has never opened document 1: resync, status, and list fail closed.
    connection_b
        .send(&ClientMessage::RequestResync {
            document_id: 1,
            client_id: 100,
            known_version: 0,
        })
        .await;
    let resync = connection_b.receive_response().await;
    assert!(
        matches!(
            resync,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "resync for an unopened document must not leak text, got {resync:?}"
    );
    connection_b
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 100,
            document_id: 1,
        })
        .await;
    let status = connection_b.receive_response().await;
    assert!(
        matches!(
            status,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "status for an unopened document must fail closed, got {status:?}"
    );
    connection_b
        .send(&ClientMessage::ListDocuments { client_id: 100 })
        .await;
    let list = connection_b.receive_response().await;
    assert!(
        matches!(list, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
        "list must not leak another connection's documents, got {list:?}"
    );

    // B opens the same document: read-only access. Save fails closed.
    connection_b
        .send(&ClientMessage::OpenDocument {
            client_id: 100,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    loop {
        match connection_b.receive().await {
            ServerMessage::DocumentOpened { metadata, .. } => {
                assert_eq!(metadata.access, DocumentAccess::ReadOnly);
                break;
            }
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during B open: {other:?}"),
        }
    }
    connection_b.drain_until_quiet().await;
    connection_b
        .send(&ClientMessage::SaveDocument {
            client_id: 100,
            document_id: 1,
            known_version: 1,
        })
        .await;
    let read_only_save = connection_b.receive_response().await;
    assert!(
        matches!(
            read_only_save,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::AccessDenied,
                ..
            }
        ),
        "read-only save must fail closed, got {read_only_save:?}"
    );

    // A saves with a future version claim: stale check fails closed.
    connection_a
        .send(&ClientMessage::SaveDocument {
            client_id: 99,
            document_id: 1,
            known_version: 99,
        })
        .await;
    let stale_save = connection_a.receive_response().await;
    assert!(
        matches!(
            stale_save,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::StaleFileMetadata,
                ..
            }
        ),
        "future-version save must fail closed, got {stale_save:?}"
    );

    // A saves at the current version: succeeds and clears dirty state.
    connection_a
        .send(&ClientMessage::SaveDocument {
            client_id: 99,
            document_id: 1,
            known_version: 1,
        })
        .await;
    let saved = connection_a.receive_response().await;
    assert!(
        matches!(
            saved,
            ServerMessage::DocumentSaved {
                document_id: 1,
                version: 1,
                dirty: false,
            }
        ),
        "lease-holder save at the current version must succeed, got {saved:?}"
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn protocol_v27_client_is_rejected_by_v28_server() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document_state(),
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: 26,
                client_name: "v27-client".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::Error {
            code: ProtocolErrorCode::UnsupportedProtocolVersion,
            message: "unsupported protocol version".to_string(),
        }
    );
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn live_typography_update_reaches_connection_once() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let runtime_generation = runtime_generation();
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document_state(),
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation.clone(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    loop {
        if matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ) {
            break;
        }
    }

    let mut typography = crate::protocol::ActiveTypography::default();
    typography.monospace.size = 16.0;
    runtime_generation
        .replace_typography(typography)
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::ActiveTypography(typography)
            if typography.revision == 1 && typography.monospace.size == 16.0
    ));
    assert!(
        timeout(
            Duration::from_millis(20),
            codec.read_server_message(&mut client),
        )
        .await
        .is_err(),
        "one replacement emits one live update"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_sends_minimal_behavior_manifest() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(1)))
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_does_not_send_default_workspace_sdui_snapshot_after_bootstrap() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hello from server".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        empty_sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    // Post-handshake file-open capability is always issued once.
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    let next = timeout(
        Duration::from_millis(25),
        codec.read_server_message(&mut client),
    )
    .await;
    assert!(next.is_err(), "unexpected default SDUI message: {next:?}");

    drop(client);
    server_task.await.unwrap().unwrap();
}
