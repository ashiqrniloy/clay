use super::*;

#[tokio::test]
async fn connection_open_document_sends_snapshot_and_manifest_without_full_document_on_edit_ack() {
    let root = temp_workspace("open-dispatch");
    let file = root.join("main.rs");
    fs::write(&file, "fn main() {}\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

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
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::DocumentOpened {
            metadata: DocumentMetadata {
                document_id: 1,
                version: 1,
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
                dirty: false,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
            head: crate::protocol::DocumentTextHead::complete("fn main() {}\n".to_string()),
        }
    );
    let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => {
            assert_eq!(manifest.manifest_id, "rust.rust");
            assert_eq!(
                manifest.scope,
                crate::protocol::BehaviorScope::Document { document_id: 1 }
            );
            assert_eq!(manifest.editor_rules.tab.spaces_per_tab, 4);
            assert_eq!(
                manifest
                    .editor_rules
                    .autocomplete_triggers
                    .iter()
                    .map(|trigger| trigger.trigger.as_str())
                    .collect::<Vec<_>>(),
                vec![".", ":"]
            );
            manifest.behavior_version
        }
        other => panic!("expected Rust behavior manifest after open, got {other:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 1,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version,
                transaction_id: 444,
                operation: EditOperation::Insert {
                    byte_offset: 13,
                    text: "// ok\n".to_string(),
                },
            },
        )
        .await
        .unwrap();

    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::EditAck {
                document_id: 1,
                confirmed_version: 2,
                transaction_id: 444,
            } => break,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            other => panic!("expected edit acknowledgement, got {other:?}"),
        }
    }

    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DecorationSet(set)
                if set.document_id == 1 && set.document_version == 2 =>
            {
                assert!(!set.spans.is_empty());
                break;
            }
            ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            other => panic!("expected refreshed syntax decorations, got {other:?}"),
        }
    }

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::GetDocumentStatus {
                client_id: 99,
                document_id: 1,
            },
        )
        .await
        .unwrap();
    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DocumentStatus {
                metadata:
                    DocumentMetadata {
                        document_id: 1,
                        version: 2,
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                        dirty: true,
                        workspace_root_id: status_root_id,
                        path,
                    },
            } => {
                assert_eq!(status_root_id, root_id);
                assert_eq!(path, "main.rs");
                break;
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            other => panic!("expected document status, got {other:?}"),
        }
    }

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn viewport_render_requests_answer_one_patch_per_request_id() {
    let root = temp_workspace("viewport-patch-protocol");
    let file = root.join("main.rs");
    fs::write(&file, "fn main() {} // tail\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

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
    for _ in 0..9 {
        let _ = codec.read_server_message(&mut client).await.unwrap();
    }
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
        )
        .await
        .unwrap();
    let opened_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::DocumentOpened { metadata, .. } => metadata.version,
        message => panic!("expected DocumentOpened, got {message:?}"),
    };
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    // Trailing connection-wide manifest follows the document's mode layer.
    let _global_manifest = codec.read_server_message(&mut client).await.unwrap();
    // Drain the open-driven parse output (edit-driven frames) so the
    // later request-scoped assertions see a quiet connection.
    loop {
        let message = timeout(
            Duration::from_secs(2),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("open parse output timed out")
        .unwrap();
        if matches!(
            message,
            ServerMessage::DecorationSet(_)
                | ServerMessage::DecorationBatch(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
        ) {
            break;
        }
    }

    // Stale version: one rejection patch, nothing else scheduled.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::ViewportRenderRequest {
                client_id: 99,
                document_id: 1,
                document_version: opened_version + 5,
                request_id: 1,
                byte_start: 0,
                byte_end: 16,
                trace_id: None,
            },
        )
        .await
        .unwrap();
    let stale_patch = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::ViewportRenderPatch(patch) => break patch,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::BehaviorManifest(_) => {}
            message => panic!("expected stale-version rejection patch, got {message:?}"),
        }
    };
    assert_eq!(stale_patch.request_id, 1);
    assert_eq!(stale_patch.status, ViewportRenderStatus::Rejected);
    assert_eq!(stale_patch.reason.as_deref(), Some("staleVersion"));
    assert!(stale_patch.decorations.is_empty());

    // Invalid range: rejected before any allocation.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::ViewportRenderRequest {
                client_id: 99,
                document_id: 1,
                document_version: opened_version,
                request_id: 2,
                byte_start: 32,
                byte_end: 16,
                trace_id: None,
            },
        )
        .await
        .unwrap();
    let range_patch = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::ViewportRenderPatch(patch) => break patch,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::BehaviorManifest(_) => {}
            message => panic!("expected invalid-range rejection patch, got {message:?}"),
        }
    };
    assert_eq!(range_patch.request_id, 2);
    assert_eq!(range_patch.status, ViewportRenderStatus::Rejected);
    assert_eq!(range_patch.reason.as_deref(), Some("invalidRange"));

    // Valid request (clamped past the document end): exactly one complete
    // patch aggregates every scheduled window member, in viewport-key
    // order, with no per-member frames after the request.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::ViewportRenderRequest {
                client_id: 99,
                document_id: 1,
                document_version: opened_version,
                request_id: 3,
                byte_start: 0,
                byte_end: 1 << 20,
                trace_id: None,
            },
        )
        .await
        .unwrap();
    let mut member_frames = 0usize;
    let patch = loop {
        let message = timeout(
            Duration::from_secs(2),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("viewport patch timed out")
        .unwrap();
        match message {
            ServerMessage::ViewportRenderPatch(patch) if patch.request_id == 3 => break patch,
            ServerMessage::ViewportRenderPatch(_) => {
                panic!("no second patch may answer one request id")
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_) => member_frames += 1,
            _ => {}
        }
    };
    assert_eq!(patch.status, ViewportRenderStatus::Complete);
    assert!(patch.reason.is_none());
    assert!(!patch.decorations.is_empty());
    assert!(
        patch
            .decorations
            .windows(2)
            .all(|pair| pair[0].viewport_byte_start <= pair[1].viewport_byte_start),
        "patch members arrive in viewport-key order"
    );
    assert!(
        patch
            .covered_ranges
            .iter()
            .all(|range| range.byte_end <= 21),
        "covered ranges stay clamped to the document, got {:?}",
        patch.covered_ranges
    );
    assert_eq!(
        member_frames, 0,
        "viewport replies must not fan out per-member frames"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn file_browser_open_uses_generic_open_document_followups() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = temp_workspace("file-browser-open-followups");
    let selected = root.join("note.md");
    fs::write(&selected, "# Browser note\n\n- item\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.reserve_document_ids_from(2);
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();

    let (client, server) = duplex(16 * 1024 * 1024);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        Arc::clone(&sdui),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation_from(runtime),
        coordinator,
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
    let tree = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::SduiSnapshot { tree, .. } => tree,
        message => panic!("expected file browser SduiSnapshot, got {message:?}"),
    };
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();
    let action = tree
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            SduiNodeKind::List { items, .. } => items
                .iter()
                .find(|item| item.label == "note.md")
                .and_then(|item| item.action.clone()),
            _ => None,
        })
        .expect("note.md file-browser action");

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::SduiAction {
                client_id: 99,
                ui_version: tree.ui_version,
                intent: action,
            },
        )
        .await
        .unwrap();

    let (opened_version, opened_lease_id) =
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DocumentOpened { metadata, head } => {
                assert_eq!(metadata.document_id, 2);
                assert_eq!(metadata.workspace_root_id, root_id);
                assert_eq!(metadata.path, "note.md");
                assert_eq!(head.first_chunk, "# Browser note\n\n- item\n");
                let DocumentAccess::Editable { lease_id } = metadata.access else {
                    panic!("file-browser opener must receive editable access");
                };
                (metadata.version, lease_id)
            }
            message => panic!("expected file-browser DocumentOpened, got {message:?}"),
        };
    let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => {
            assert_eq!(manifest.manifest_id, "markdown.markdown");
            assert!(matches!(
                manifest.scope,
                BehaviorScope::Document { document_id: 2 }
            ));
            manifest.behavior_version
        }
        message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 2,
                client_id: 99,
                lease_id: Some(opened_lease_id),
                base_version: opened_version,
                behavior_version,
                transaction_id: 7,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "!".to_string(),
                },
            },
        )
        .await
        .unwrap();
    loop {
        match timeout(
            Duration::from_secs(1),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("opened-file edit acknowledgement timed out")
        .unwrap()
        {
            ServerMessage::EditAck {
                document_id: 2,
                confirmed_version,
                transaction_id: 7,
            } => {
                assert_eq!(confirmed_version, opened_version + 1);
                break;
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            message => panic!("expected opened-file EditAck, got {message:?}"),
        }
    }

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(selected);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn multi_chunk_parse_update_ships_as_single_decoration_batch() {
    let root = temp_workspace("decoration-batch");
    let file = root.join("main.rs");
    // Well past one 128-byte authority chunk.
    let source = "fn main() { let value = 1; }\n".repeat(16);
    fs::write(&file, &source).unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let (client, server) = duplex(64 * 1024);
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
    for _ in 0..8 {
        let _ = codec.read_server_message(&mut client).await.unwrap();
    }
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
        )
        .await
        .unwrap();
    // DocumentOpened, BehaviorManifest, replenished capability.
    let _opened = codec.read_server_message(&mut client).await.unwrap();
    let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
        message => panic!("expected behavior manifest, got {message:?}"),
    };
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    // Register/cache the native handler with one edit, then request the
    // whole visible region so this test isolates multi-chunk wire batching
    // from the edit's expected one-chunk incremental update.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 1,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version,
                transaction_id: 555,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "// batch\n".to_string(),
                },
            },
        )
        .await
        .unwrap();

    let mut confirmed_version = None;
    let mut edit_update_seen = false;
    let mut viewport_requested = false;
    let mut viewport_patches = 0usize;
    let mut member_frames_after_request = 0usize;
    let batch = loop {
        let message = timeout(
            Duration::from_secs(2),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("viewport patch timed out")
        .unwrap();
        match message {
            ServerMessage::ViewportRenderPatch(patch)
                if viewport_requested && patch.request_id == 1 =>
            {
                assert_eq!(patch.document_id, 1);
                assert_eq!(patch.document_version, 2);
                viewport_patches += 1;
                break patch.decorations;
            }
            ServerMessage::DecorationBatch(chunks)
                if !viewport_requested && chunks[0].document_version == 2 =>
            {
                edit_update_seen = true;
            }
            ServerMessage::EditAck {
                confirmed_version: version,
                ..
            } => confirmed_version = Some(version),
            ServerMessage::DecorationSet(set)
                if set.document_id == 1 && set.document_version == 2 =>
            {
                if viewport_requested {
                    member_frames_after_request += 1;
                } else {
                    edit_update_seen = true;
                }
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_) => {}
            message => panic!("expected viewport render patch, got {message:?}"),
        }
        if !viewport_requested
            && edit_update_seen
            && let Some(document_version) = confirmed_version
        {
            codec
                .write_client_message(
                    &mut client,
                    &ClientMessage::ViewportRenderRequest {
                        client_id: 99,
                        document_id: 1,
                        document_version,
                        request_id: 1,
                        byte_start: 0,
                        byte_end: (source.len() + "// batch\n".len()) as u64,
                        trace_id: None,
                    },
                )
                .await
                .unwrap();
            viewport_requested = true;
        }
    };

    assert!(
        batch.len() > 1,
        "multi-chunk window must arrive as one patch with ordered members, got {} members",
        batch.len()
    );
    assert!(batch.iter().all(|set| set.document_id == 1));
    assert!(
        batch
            .windows(2)
            .all(|pair| pair[0].viewport_byte_start <= pair[1].viewport_byte_start),
        "patch members arrive in viewport-key order"
    );
    assert!(batch.iter().all(|set| !set.spans.is_empty()));
    assert_eq!(viewport_patches, 1, "exactly one patch per request id");
    assert_eq!(
        member_frames_after_request, 0,
        "viewport replies must not fan out per-chunk frames after the request"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn selected_markdown_file_publishes_manifest_and_decorations() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = temp_workspace("selected-markdown-runtime");
    let selected = root.join("note.md");
    fs::write(
        &selected,
        "# Opened note\n\n- item with `code`\n\n**strong** and *emphasis*\n",
    )
    .unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.reserve_document_ids_from(2);
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();

    let (client, server) = duplex(16 * 1024 * 1024);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        Arc::clone(&sdui),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation_from(runtime),
        coordinator,
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

    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::DocumentOpened { metadata, head } => {
            assert_eq!(metadata.document_id, 2);
            assert_eq!(metadata.path, "note.md");
            assert_eq!(
                head.first_chunk,
                "# Opened note\n\n- item with `code`\n\n**strong** and *emphasis*\n"
            );
        }
        message => panic!("expected selected Markdown DocumentOpened, got {message:?}"),
    }
    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => {
            assert_eq!(manifest.manifest_id, "markdown.markdown");
            assert!(matches!(
                manifest.scope,
                BehaviorScope::Document { document_id: 2 }
            ));
            assert!(
                manifest
                    .commands
                    .iter()
                    .any(|command| { command.command_id == "markdown.togglePreview" })
            );
        }
        message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
    }
    // Server re-issues one pending capability after the open attempt; parse
    // decorations are scheduled in the background instead of blocking open.
    // Phase 22.2: the follow-up also carries the connection-wide manifest
    // after the document's mode layer; consume it before the capability.
    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(_) => {}
        message => panic!("expected trailing global manifest, got {message:?}"),
    }
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));

    // Selected-file activation publishes behavior only on the open path;
    // optional package UI panels stay opt-in, and highlights arrive later
    // through the parse coordinator rather than before the replenished
    // capability.

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(selected);
    let _ = fs::remove_dir(root);
}
