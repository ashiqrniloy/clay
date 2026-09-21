use super::*;

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
    // The ambient default configuration (e.g. ~/.clay/init.js) may
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
