use super::*;

#[tokio::test]
async fn open_document_renders_before_background_parse_completes() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let mut text = "# Top\n\n".to_string();
    text.push_str(&"a".repeat(80 * 1024));
    text.push_str("\n# Outside initial window\n");
    let metadata = DocumentMetadata {
        document_id: 2,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: 1,
        path: "large.md".to_string(),
    };
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = empty_sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
    let activation = super::super::classify_open_document(
        1,
        &runtime,
        &coordinator,
        &metadata,
        &text,
        &behavior,
        &sdui,
    )
    .await
    .expect("loaded package should classify markdown path");

    let document = DocumentState::new(2, text, DocumentAccess::Editable { lease_id: 1 });
    let immediate = super::super::schedule_open_parse(
        &coordinator,
        &metadata,
        &document,
        &behavior,
        &activation,
    )
    .await
    .expect("open parse should schedule");
    assert!(
        immediate.is_none(),
        "open follow-up must not wait for parse output"
    );

    let native_window = crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES;
    let update = timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(update.document_id, 2);
    assert_eq!(
        (update.viewport.start, update.viewport.end),
        (0, native_window as u64)
    );
    assert!(update.decoration_updates.iter().any(|set| {
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Heading1)
    }));
}

#[tokio::test]
async fn connection_open_selected_file_sends_snapshot_and_single_file_grant() {
    let root = temp_workspace("selected-dispatch");
    let selected = root.join("note.md");
    let sibling = root.join("sibling.md");
    fs::write(&selected, "# selected\n").unwrap();
    fs::write(&sibling, "# sibling\n").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
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
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::FileOpenCapabilityIssued { token } => token,
        message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenSelectedFile {
                client_id: 99,
                capability: capability_token,
                selected_path: selected.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    let selected_root_id = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::DocumentOpened { metadata, head } => {
            assert_eq!(metadata.document_id, 1);
            assert_eq!(metadata.version, 1);
            assert_eq!(metadata.access, DocumentAccess::Editable { lease_id: 1 });
            assert_eq!(metadata.path, "note.md");
            assert_eq!(head.first_chunk, "# selected\n");
            metadata.workspace_root_id
        }
        message => panic!("expected selected DocumentOpened, got {message:?}"),
    };
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::BehaviorManifest(_)
    ));
    loop {
        if matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ) {
            break;
        }
    }

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: selected_root_id,
                path: sibling.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::OutsideRoot,
                workspace_root_id: Some(id),
                document_id: None,
                ..
            } if id == selected_root_id => break,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_) => {}
            other => panic!("expected outside-root failure, got {other:?}"),
        }
    }

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(selected);
    let _ = fs::remove_file(sibling);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn connection_add_selected_workspace_root_sends_file_browser_snapshot() {
    let root = temp_workspace("selected-folder-dispatch");
    fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        Arc::clone(&document),
        behavior,
        Arc::clone(&workspace),
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
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::FileOpenCapabilityIssued { token } => token,
        message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::AddSelectedWorkspaceRoot {
                client_id: 99,
                capability: capability_token,
                selected_path: root.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::SduiSnapshot { client_id: 99, .. }
    ));
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    assert_eq!(workspace.lock().await.list_root_metadata().len(), 1);

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(root.join("main.rs"));
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn connection_add_selected_workspace_root_rejects_stale_capability() {
    let root = temp_workspace("selected-folder-stale");
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
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
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::AddSelectedWorkspaceRoot {
                client_id: 99,
                capability: "stale".to_string(),
                selected_path: root.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::RuntimeDiagnostic(diagnostic)
            if diagnostic.code == "client.selected_folder_open.unauthorized"
    ));
    assert!(workspace.lock().await.list_root_metadata().is_empty());

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn file_io_errors_are_typed_protocol_failures() {
    let root = temp_workspace("typed-errors");
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace,
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
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "missing.txt".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::NotFound,
            workspace_root_id: Some(id),
            document_id: None,
            ..
        } if id == root_id
    ));

    let invalid_utf8 = root.join("invalid.txt");
    fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "invalid.txt".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::InvalidUtf8,
            workspace_root_id: Some(id),
            document_id: None,
            ..
        } if id == root_id
    ));

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn server_rejects_invalid_frame_without_panic() {
    let (mut client, server) = duplex(4096);
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

    tokio::io::AsyncWriteExt::write_all(&mut client, &[0, 0, 0, 4, 0xde, 0xad, 0xbe, 0xef])
        .await
        .unwrap();
    drop(client);

    let result = server_task.await.unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn fragmented_client_frame_survives_concurrent_server_write() {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    let (mut client, server) = duplex(4096);
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
    for _ in 0..9 {
        let _ = codec.read_server_message(&mut client).await.unwrap();
    }

    // Drip-feed a client frame start, then fire a typography broadcast so a
    // server write wins the select race mid-frame. The read pump must keep
    // frame alignment regardless of the interleaving.
    let frame = codec
        .encode_client_message(&ClientMessage::ListDocuments { client_id: 99 })
        .unwrap();
    let split = 6;
    client.write_all(&frame[..split]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut typography = crate::protocol::ActiveTypography::default();
    typography.monospace.size += 1.0;
    runtime_generation
        .replace_typography(typography)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    client.write_all(&frame[split..]).await.unwrap();

    let mut saw_typography = false;
    let mut saw_status = false;
    for _ in 0..4 {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::ActiveTypography(_) => saw_typography = true,
            ServerMessage::DocumentList { .. } => saw_status = true,
            other => panic!("unexpected message during fragmented read: {other:?}"),
        }
        if saw_typography && saw_status {
            break;
        }
    }
    assert!(saw_typography && saw_status);

    // A second full request proves the stream stayed aligned.
    codec
        .write_client_message(&mut client, &ClientMessage::ListDocuments { client_id: 99 })
        .await
        .unwrap();
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::DocumentList { .. }
    ));

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn open_selected_file_without_capability_is_rejected_with_diagnostic() {
    let root = temp_workspace("selected-unauthorized");
    let target = root.join("secret.md");
    fs::write(&target, "# secret\n").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
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
    // Consume handshake noise and the post-handshake capability so it is no
    // longer pending.
    loop {
        if matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ) {
            break;
        }
    }

    // Raw path with no valid capability: server must reject and must NOT
    // open or grant the file.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenSelectedFile {
                client_id: 99,
                capability: String::new(),
                selected_path: target.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
    // Re-issued pending capability first, then the rejection diagnostic.
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::RuntimeDiagnostic(diagnostic) => {
            assert_eq!(diagnostic.code, "client.selected_file_open.unauthorized");
        }
        message => panic!("expected unauthorized RuntimeDiagnostic, got {message:?}"),
    }
    // No document was registered for the rejected path.
    assert!(workspace.lock().await.document_handle(1).is_none());

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(target);
    let _ = fs::remove_dir(root);
}

/// CloseDocument: final-holder close acknowledges and tears down the
/// document; a shared document survives until the last holder closes
/// (Plan 060 T6, P1-4).
#[tokio::test]
async fn close_document_acknowledges_and_tears_down_final_document() {
    let root = temp_workspace("close-document");
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

    // A opens the shared document, then B opens it too.
    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    connection_a.drain_until_quiet().await;
    connection_b
        .send(&ClientMessage::OpenDocument {
            client_id: 100,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    connection_b.drain_until_quiet().await;

    // A closes: not the final holder, document survives for B.
    connection_a
        .send(&ClientMessage::CloseDocument {
            client_id: 99,
            document_id: 1,
            force: false,
        })
        .await;
    let closed_a = connection_a.receive_response().await;
    assert!(
        matches!(
            closed_a,
            ServerMessage::DocumentClosed {
                document_id: 1,
                closed: false
            }
        ),
        "non-final close must report closed=false, got {closed_a:?}"
    );
    connection_b
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 100,
            document_id: 1,
        })
        .await;
    let status_b = connection_b.receive_response().await;
    assert!(
        matches!(status_b, ServerMessage::DocumentStatus { .. }),
        "remaining holder must still see the document, got {status_b:?}"
    );
    // A no longer has access.
    connection_a
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 99,
            document_id: 1,
        })
        .await;
    let status_a = connection_a.receive_response().await;
    assert!(
        matches!(
            status_a,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "closed connection must lose access, got {status_a:?}"
    );

    // B closes: final holder, document is torn down.
    connection_b
        .send(&ClientMessage::CloseDocument {
            client_id: 100,
            document_id: 1,
            force: false,
        })
        .await;
    let closed_b = connection_b.receive_response().await;
    assert!(
        matches!(
            closed_b,
            ServerMessage::DocumentClosed {
                document_id: 1,
                closed: true
            }
        ),
        "final close must report closed=true, got {closed_b:?}"
    );
    assert!(workspace.lock().await.document_handle(1).is_none());

    let _ = fs::remove_file(root.join("note.md"));
    let _ = fs::remove_dir(root);
}

/// Disconnect releases every access grant; documents with no remaining
/// holders are removed from the workspace registry (Plan 060 T6, P1-4).
#[tokio::test]
async fn disconnect_finalizes_documents_with_no_remaining_holders() {
    let root = temp_workspace("disconnect-finalize");
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

    let connection = TestConnection::connect(
        99,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation(),
        parse_coordinator(),
        crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
        language_intelligence_coordinator(),
    )
    .await;
    let mut connection = connection;
    connection
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    connection.drain_until_quiet().await;
    assert!(workspace.lock().await.document_handle(1).is_some());

    // Disconnect: the server task exits and finalizes the document.
    drop(connection.client);
    connection.server_task.await.unwrap().unwrap();
    assert!(
        workspace.lock().await.document_handle(1).is_none(),
        "disconnect must finalize documents with no remaining holders"
    );

    let _ = fs::remove_file(root.join("note.md"));
    let _ = fs::remove_dir(root);
}

/// Phase 22.6 (plan 077 task 6): a reconnected/reclaimed tab regains
/// only its own grants. Tab A's disconnect releases every document
/// grant; a fresh connection re-opening one of the tab's documents
/// inherits nothing — the tab's other document stays unknown until
/// explicitly re-opened.
#[tokio::test]
async fn reconnected_tab_regains_only_its_own_reopened_grants() {
    let root = temp_workspace("tab-reclaim-grants");
    fs::write(root.join("note.md"), "hello\n").unwrap();
    fs::write(root.join("second.md"), "second\n").unwrap();
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

    // Tab A's connection (99) opens both of the tab's documents.
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
    let first_document =
        open_document_until_opened(&mut connection_a, 99, root_id, "note.md").await;
    let second_document =
        open_document_until_opened(&mut connection_a, 99, root_id, "second.md").await;
    assert_eq!(first_document, 1);
    assert_eq!(second_document, 2);
    connection_a.drain_until_quiet().await;
    assert!(workspace.lock().await.document_handle(1).is_some());
    assert!(workspace.lock().await.document_handle(2).is_some());

    // Disconnect: every grant is released and both documents finalize.
    connection_a.close().await;
    assert!(
        workspace.lock().await.document_handle(1).is_none(),
        "disconnect must release the tab's first document grant"
    );
    assert!(
        workspace.lock().await.document_handle(2).is_none(),
        "disconnect must release the tab's second document grant"
    );

    // The reconnected tab (fresh connection 101) inherits nothing.
    let mut connection_c = TestConnection::connect(
        101,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;
    connection_c
        .send(&ClientMessage::ListDocuments { client_id: 101 })
        .await;
    let list = connection_c.receive_response().await;
    assert!(
        matches!(list, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
        "reconnected tab must inherit no grants, got {list:?}"
    );

    // Re-opening the tab's own document grants only the new connection:
    // the old grant was finalized, so the file re-opens as a fresh
    // document with a fresh lease, not as a restored one.
    let reopened = open_document_until_opened(&mut connection_c, 101, root_id, "note.md").await;
    assert_ne!(
        reopened, first_document,
        "a finalized grant must not be re-attached; re-open is a fresh grant"
    );
    connection_c.drain_until_quiet().await;
    connection_c
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 101,
            document_id: second_document,
        })
        .await;
    let status = connection_c.receive_response().await;
    assert!(
        matches!(
            status,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "the tab's second document stays ungranted until re-opened, got {status:?}"
    );
    connection_c
        .send(&ClientMessage::ListDocuments { client_id: 101 })
        .await;
    let list = connection_c.receive_response().await;
    assert!(
        matches!(list, ServerMessage::DocumentList { ref documents }
            if documents.len() == 1 && documents[0].document_id == reopened),
        "re-opened grant is the only grant, got {list:?}"
    );

    connection_c.close().await;
    let _ = fs::remove_file(root.join("note.md"));
    let _ = fs::remove_file(root.join("second.md"));
    let _ = fs::remove_dir(root);
}
