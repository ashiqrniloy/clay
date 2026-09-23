use super::*;

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
            if let Ok(Ok(message)) = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                codec.read_client_message(&mut server),
            )
            .await
            {
                panic!("unexpected client message before install: {message:?}")
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
            if let Ok(Ok(message)) = tokio::time::timeout(
                std::time::Duration::from_millis(50),
                codec.read_client_message(&mut server),
            )
            .await
            {
                panic!("invalid snapshot must not acknowledge: {message:?}");
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
