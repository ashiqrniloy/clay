use super::*;

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
