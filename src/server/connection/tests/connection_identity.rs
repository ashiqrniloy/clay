use super::*;

/// Plan 060 T4 (P0-2): one pre-dispatch boundary rejects every legacy
/// message whose `client_id` does not match the handshake-assigned
/// connection identity. Table covers every post-Hello family.
#[tokio::test]
async fn forged_client_identity_is_rejected_for_every_message_family() {
    let root = temp_workspace("forged-identity");
    fs::write(root.join("note.md"), "# secret\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();
    let mut connection = TestConnection::connect(
        99,
        document,
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;

    let forged: Vec<(&str, ClientMessage)> = vec![
        (
            "Edit",
            ClientMessage::Edit {
                document_id: 7,
                client_id: 1,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 1,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "x".to_string(),
                },
            },
        ),
        (
            "EditorIntent",
            ClientMessage::EditorIntent {
                document_id: 7,
                client_id: 1,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 2,
                intent: crate::protocol::EditorIntent::InsertText {
                    byte_offset: 0,
                    text: "x".to_string(),
                },
            },
        ),
        (
            "RequestResync",
            ClientMessage::RequestResync {
                document_id: 7,
                client_id: 1,
                known_version: 0,
            },
        ),
        (
            "ViewportRenderRequest",
            ClientMessage::ViewportRenderRequest {
                client_id: 1,
                document_id: 7,
                document_version: 1,
                request_id: 1,
                byte_start: 0,
                byte_end: 1,
                trace_id: None,
            },
        ),
        (
            "OpenDocument",
            ClientMessage::OpenDocument {
                client_id: 1,
                workspace_root_id: root_id,
                path: "note.md".to_string(),
            },
        ),
        (
            "OpenSelectedFile",
            ClientMessage::OpenSelectedFile {
                client_id: 1,
                capability: "forged".to_string(),
                selected_path: root.join("note.md").to_string_lossy().into_owned(),
            },
        ),
        (
            "AddSelectedWorkspaceRoot",
            ClientMessage::AddSelectedWorkspaceRoot {
                client_id: 1,
                capability: "forged".to_string(),
                selected_path: root.to_string_lossy().into_owned(),
            },
        ),
        (
            "SaveDocument",
            ClientMessage::SaveDocument {
                client_id: 1,
                document_id: 7,
                known_version: 1,
            },
        ),
        (
            "ReloadDocument",
            ClientMessage::ReloadDocument {
                client_id: 1,
                document_id: 7,
                known_version: 1,
                force: true,
            },
        ),
        (
            "GetDocumentStatus",
            ClientMessage::GetDocumentStatus {
                client_id: 1,
                document_id: 7,
            },
        ),
        (
            "ListDocuments",
            ClientMessage::ListDocuments { client_id: 1 },
        ),
        (
            "SduiAction",
            ClientMessage::SduiAction {
                client_id: 1,
                ui_version: 1,
                intent: SduiActionIntent::command(
                    "controlCenter.open",
                    SduiActionSource::Button {
                        node_id: SduiNodeId(1),
                    },
                ),
            },
        ),
        (
            "CommandIntent",
            ClientMessage::CommandIntent {
                client_id: 1,
                document_id: 7,
                behavior_version: 1,
                command_id: "controlCenter.open".to_string(),
            },
        ),
        (
            "CompletionRequest",
            ClientMessage::CompletionRequest {
                request: crate::protocol::CompletionRequest {
                    request_id: 1,
                    client_id: 1,
                    document_id: 7,
                    document_version: 1,
                    behavior_version: 1,
                    cursor_byte_offset: 0,
                    replacement_range: crate::protocol::CompletionReplacementRange::new(0, 0),
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 1,
                    recent_completions: Vec::<String>::new().into_boxed_slice(),
                },
            },
        ),
        (
            "LanguageIntelligenceRequest",
            ClientMessage::LanguageIntelligenceRequest {
                request: crate::protocol::LanguageIntelligenceRequest {
                    request_id: 1,
                    client_id: 1,
                    document_id: 7,
                    document_version: 1,
                    behavior_version: 1,
                    cursor_byte_offset: 0,
                    feature: crate::protocol::LanguageIntelligenceFeature::Hover,
                    provider_generation: 1,
                },
            },
        ),
        (
            "RuntimeGenerationInstalled",
            ClientMessage::RuntimeGenerationInstalled {
                client_id: 1,
                runtime_generation_id: 1,
            },
        ),
    ];

    for (family, message) in forged {
        connection.send(&message).await;
        let response = connection.receive().await;
        assert!(
            matches!(
                response,
                ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    ..
                }
            ),
            "forged {family} must be rejected at the identity boundary, got {response:?}"
        );
    }

    // The forged OpenDocument must have had no effect: the connection's own
    // document list stays empty, and the connection survives rejections.
    connection
        .send(&ClientMessage::ListDocuments { client_id: 99 })
        .await;
    let response = connection.receive().await;
    assert!(
        matches!(response, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
        "forged open must not register documents, got {response:?}"
    );

    connection.close().await;
    let _ = fs::remove_dir_all(root);
}

/// Plan 060 T4 (P0-3): two connections share the server coordinators; a
/// parse update for a document opened by one connection never reaches the
/// other connection's stream.
#[tokio::test]
async fn two_client_parse_updates_are_isolated_to_the_subscribed_connection() {
    let root = temp_workspace("parse-isolation");
    fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
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

    // A opens and edits the document; only A may see its parse output.
    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "main.rs".to_string(),
        })
        .await;
    let behavior_version = loop {
        match connection_a.receive().await {
            ServerMessage::BehaviorManifest(manifest) => break manifest.behavior_version,
            ServerMessage::DocumentOpened { .. }
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during open: {other:?}"),
        }
    };
    connection_a.drain_until_quiet().await;
    connection_a
        .send(&ClientMessage::Edit {
            document_id: 1,
            client_id: 99,
            lease_id: Some(1),
            base_version: 1,
            behavior_version,
            transaction_id: 900,
            operation: EditOperation::Insert {
                byte_offset: 13,
                text: "// owned by A\n".to_string(),
            },
        })
        .await;
    loop {
        match connection_a.receive().await {
            ServerMessage::DecorationSet(set)
                if set.document_id == 1 && set.document_version == 2 =>
            {
                break;
            }
            ServerMessage::EditAck { .. }
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_) => {}
            other => panic!("unexpected message awaiting decorations: {other:?}"),
        }
    }

    // B never subscribed to document 1: its stream stays silent.
    let leaked = timeout(
        Duration::from_millis(150),
        connection_b
            .codec
            .read_server_message(&mut connection_b.client),
    )
    .await;
    assert!(
        leaked.is_err(),
        "unsubscribed connection must receive no parse output, got {leaked:?}"
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(root);
}
