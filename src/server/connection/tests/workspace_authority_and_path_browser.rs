use super::*;

#[tokio::test]
async fn cross_tab_workspace_and_document_authority_is_fail_closed() {
    let root_a = temp_workspace("authority-alpha");
    let root_b = temp_workspace("authority-beta");
    let extra_root_a = temp_workspace("authority-alpha-extra");
    fs::write(root_a.join("alpha.txt"), "alpha").unwrap();
    fs::write(root_b.join("beta.txt"), "beta").unwrap();
    fs::write(extra_root_a.join("only-alpha.txt"), "only alpha").unwrap();

    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("cross-tab-authority"),
    ));
    let (alpha_snapshot, alpha_state) = server
        .create_tab_state(11, root_a.to_string_lossy().into_owned())
        .await
        .expect("alpha tab state is created");
    let (beta_snapshot, beta_state) = server
        .create_tab_state(22, root_b.to_string_lossy().into_owned())
        .await
        .expect("beta tab state is created");
    let alpha_tab = alpha_snapshot.tabs[0].tab_id;
    let beta_tab = beta_snapshot.tabs[1].tab_id;
    let alpha_root_id = alpha_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("alpha root")
        .workspace_root_id;
    let alpha_extra_root_id = alpha_state
        .workspace
        .lock()
        .await
        .add_root(&extra_root_a)
        .expect("alpha extra root");
    let beta_root_id = beta_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("beta root")
        .workspace_root_id;

    let mut connection_a = TestConnection::connect_with_server(11, server.clone()).await;
    connection_a.reclaim(11, alpha_tab).await;
    let mut connection_b = TestConnection::connect_with_server(22, server.clone()).await;
    connection_b.reclaim(22, beta_tab).await;
    connection_a.drain_bounded().await;

    let (alpha_metadata, alpha_behavior_version) = connection_a
        .open_document(11, alpha_root_id, "alpha.txt")
        .await;
    let (beta_metadata, _) = connection_b
        .open_document(22, beta_root_id, "beta.txt")
        .await;
    connection_a.drain_bounded().await;
    connection_b.drain_bounded().await;
    connection_a
        .send(&ClientMessage::ListDocuments { client_id: 11 })
        .await;
    let alpha_list = connection_a.receive_response().await;
    assert!(matches!(
        alpha_list,
        ServerMessage::DocumentList { ref documents }
            if documents.len() == 1 && documents[0].document_id == alpha_metadata.document_id
    ));

    connection_b
        .send(&ClientMessage::ListDocuments { client_id: 22 })
        .await;
    let beta_list = connection_b.receive_response().await;
    assert!(matches!(
        beta_list,
        ServerMessage::DocumentList { ref documents }
            if documents.len() == 1 && documents[0].document_id == beta_metadata.document_id
    ));

    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 11,
            workspace_root_id: beta_root_id,
            path: "beta.txt".to_string(),
        })
        .await;
    let foreign_open = connection_a.receive_response().await;
    assert!(matches!(
        foreign_open,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::NotFound,
            ..
        }
    ));

    connection_b
        .send(&ClientMessage::OpenDocument {
            client_id: 22,
            workspace_root_id: alpha_extra_root_id,
            path: "only-alpha.txt".to_string(),
        })
        .await;
    let foreign_root = connection_b.receive_response().await;
    assert!(matches!(
        foreign_root,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownWorkspaceRoot,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::RequestResync {
            client_id: 11,
            document_id: beta_metadata.document_id,
            known_version: 1,
        })
        .await;
    let foreign_resync = connection_a.receive_response().await;
    assert!(matches!(
        foreign_resync,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 11,
            document_id: beta_metadata.document_id,
        })
        .await;
    let foreign_status = connection_a.receive_response().await;
    assert!(matches!(
        foreign_status,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::SaveDocument {
            client_id: 11,
            document_id: beta_metadata.document_id,
            known_version: beta_metadata.version,
        })
        .await;
    let foreign_save = connection_a.receive_response().await;
    assert!(matches!(
        foreign_save,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::ReloadDocument {
            client_id: 11,
            document_id: beta_metadata.document_id,
            known_version: beta_metadata.version,
            force: true,
        })
        .await;
    let foreign_reload = connection_a.receive_response().await;
    assert!(matches!(
        foreign_reload,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::CloseDocument {
            client_id: 11,
            document_id: beta_metadata.document_id,
            force: true,
        })
        .await;
    let foreign_close = connection_a.receive_response().await;
    assert!(matches!(
        foreign_close,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::Edit {
            client_id: 11,
            document_id: beta_metadata.document_id,
            lease_id: beta_metadata.lease_id,
            base_version: beta_metadata.version,
            behavior_version: alpha_behavior_version,
            transaction_id: 7,
            operation: EditOperation::Insert {
                byte_offset: 0,
                text: "leak".to_string(),
            },
        })
        .await;
    let foreign_edit = connection_a.receive().await;
    assert!(matches!(
        foreign_edit,
        ServerMessage::EditRejected {
            reason: EditRejection::InvalidDocument { document_id },
            ..
        } if document_id == beta_metadata.document_id
    ));

    connection_a
        .send(&ClientMessage::OpenSelectedFile {
            client_id: 11,
            capability: connection_b.file_open_capability.clone(),
            selected_path: root_b.join("beta.txt").to_string_lossy().into_owned(),
        })
        .await;
    let capability_replenish = connection_a.receive().await;
    let capability_rejection = connection_a.receive().await;
    assert!(matches!(
        capability_replenish,
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    assert!(matches!(
        capability_rejection,
        ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { ref code, .. })
            if code == "client.selected_file_open.unauthorized"
    ));

    connection_b
        .send(&ClientMessage::RequestResync {
            client_id: 22,
            document_id: beta_metadata.document_id,
            known_version: beta_metadata.version,
        })
        .await;
    let beta_resync = connection_b.receive_response().await;
    assert!(matches!(
        beta_resync,
        ServerMessage::ResyncSnapshot { ref head, document_id, .. }
            if document_id == beta_metadata.document_id && head.first_chunk == "beta"
    ));
    connection_b
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 22,
            document_id: beta_metadata.document_id,
        })
        .await;
    let beta_status = connection_b.receive_response().await;
    assert!(matches!(
        beta_status,
        ServerMessage::DocumentStatus { ref metadata }
            if metadata.document_id == beta_metadata.document_id
                && metadata.version == beta_metadata.version
                && !metadata.dirty
    ));

    connection_a
        .send(&ClientMessage::TabCommand {
            client_id: 11,
            command: crate::protocol::TabCommand::Reclaim { tab_id: beta_tab },
        })
        .await;
    let reclaim_foreign = connection_a.receive().await;
    assert!(matches!(
        reclaim_foreign,
        ServerMessage::Error {
            code: ProtocolErrorCode::InvalidMessage,
            ..
        }
    ));
    let registry = server.tab_registry.lock().await.snapshot();
    assert!(
        registry
            .tabs
            .iter()
            .any(|entry| entry.tab_id == alpha_tab && entry.client_id == 11)
    );
    assert!(
        registry
            .tabs
            .iter()
            .any(|entry| entry.tab_id == beta_tab && entry.client_id == 22)
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(root_a);
    let _ = fs::remove_dir_all(root_b);
    let _ = fs::remove_dir_all(extra_root_a);
}

/// Phase 24.3: `controlCenter.openPath` opens the Path Browser through
/// the shared Command Centre helper — from its keybinding command and
/// from the Control Center catalogue — with the one-active-session
/// invariant enforced on every open.
#[tokio::test]
async fn path_browser_opens_from_keybinding_and_control_center_catalogue() {
    let root = temp_workspace("path-browser-open");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("README.md"), "# path browser").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-open"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();

    // Keybinding path: one seed resolution + one bounded listing, pushed
    // as the initial snapshot (the active document is the tab's welcome
    // document, so the seed falls back to the bound tab's workspace root).
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(first) = receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert_eq!(first.prompt, format!("Browse · {}", root.display()));
    assert_eq!(first.query, format!("{}/", root.display()));
    // Plan 124 task 7: the path browser is the palette's path mode, so it rides
    // the same composer anchor as the catalogue instead of a window sheet.
    assert_eq!(
        first.origin,
        crate::protocol::TransientMenuOriginData::CommandPalette
    );
    let names: Vec<_> = first.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(
        names,
        vec!["src", "README.md"],
        "empty filter keeps deterministic directory-first order"
    );
    let first_path_id = first.session_id;
    assert!(first_path_id & (1 << 63) != 0, "server-owned id partition");

    // Reopening replaces the active session and reports the closed id.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == first_path_id
    ));
    let ServerMessage::TransientMenuSnapshot(second) = receive_menu_message(&mut connection).await
    else {
        panic!("expected replacement TransientMenuSnapshot");
    };
    assert_ne!(second.session_id, first_path_id);
    let second_path_id = second.session_id;
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: second_path_id,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == second_path_id
    ));

    // Control Center path: the catalogue lists "Browse Filesystem";
    // activating it closes the Control Center and opens the Path Browser
    // through the same helper.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(control_center) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected Control Center snapshot");
    };
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: control_center.session_id,
            query: "Browse Filesystem".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered snapshot");
    };
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].id, "controlCenter.openPath");
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: control_center.session_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == control_center.session_id
    ));
    let ServerMessage::TransientMenuSnapshot(from_catalogue) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected Path Browser snapshot from catalogue activation");
    };
    assert_eq!(
        from_catalogue.prompt,
        format!("Browse · {}", root.display())
    );
    assert_eq!(from_catalogue.query, format!("{}/", root.display()));

    connection.drain_bounded().await;
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3: `controlCenter.openPath` opens the Path Browser through
/// the shared Command Centre helper — from its keybinding command and
/// from the Control Center catalogue — with the one-active-session
/// invariant enforced on every open.
#[tokio::test]
async fn path_browser_opens_with_sticky_error_for_unlistable_seed() {
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-error-seed"),
    ));
    let root = temp_workspace("path-browser-error-seed");
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    // The tab root vanishes after binding: the seed still resolves to it,
    // but the bounded listing fails. The command does not fail; the
    // session opens in its sticky error state (empty items, bounded
    // status) and stays cancellable.
    let _ = fs::remove_dir_all(&root);
    let behavior_version = server.behavior.lock().await.version();

    // A missing seed does not fail the command: the session opens in its
    // sticky error state (empty items, bounded status) and stays
    // cancellable.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert!(snapshot.prompt.starts_with("Browse · "));
    assert!(snapshot.items.is_empty(), "items suppressed under error");
    assert!(matches!(
        snapshot.status,
        crate::protocol::TransientMenuStatusData::Empty { .. }
    ));
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: snapshot.session_id,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == snapshot.session_id
    ));
    connection.drain_bounded().await;
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 8): directory descend (primary activation keeps the
/// session open and relists), empty-filter Backspace ascent, direct
/// absolute/relative path jumps, and invalid-path recovery — all with a
/// stable session id and exactly one snapshot per accepted transition.
#[tokio::test]
async fn path_browser_navigates_descend_ascend_and_direct_jump() {
    let root = temp_workspace("path-browser-navigate");
    fs::create_dir_all(root.join("a/b")).unwrap();
    fs::create_dir(root.join("c")).unwrap();
    fs::write(root.join("notes.txt"), "notes").unwrap();
    fs::write(root.join("a/a1.txt"), "a1").unwrap();
    fs::write(root.join("a/b/b1.txt"), "b1").unwrap();
    fs::write(root.join("c/c1.txt"), "c1").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-navigate"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();

    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    let path_id = snapshot.session_id;
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["a", "c", "notes.txt"]);

    // Direct relative jump: typing `c/` from the tab root relists
    // `/root/c`.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "c/".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected relist snapshot");
    };
    assert_eq!(snapshot.session_id, path_id, "session id stays stable");
    assert_eq!(snapshot.prompt, format!("Browse · {}/c", root.display()));
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["c1.txt"]);

    // Direct absolute jump into `a/b`.
    let a_b = format!("{}/a/b/", root.display());
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: a_b.clone(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected absolute jump snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}/a/b", root.display()));
    assert_eq!(snapshot.query, a_b);
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b1.txt"]);

    // Empty-filter Backspace ascends one level (relist, same session).
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected ascent snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}/a", root.display()));
    assert_eq!(snapshot.query, format!("{}/a/", root.display()));
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b", "a1.txt"]);

    // Primary activation on the selected directory (`b`, index 0)
    // descends: the session stays open and relists.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected descend snapshot");
    };
    assert_eq!(snapshot.session_id, path_id, "descend keeps the session");
    assert_eq!(snapshot.prompt, format!("Browse · {}/a/b", root.display()));
    assert_eq!(snapshot.query, format!("{}/a/b/", root.display()));
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b1.txt"]);

    // Filter-only edits never relist (no second snapshot): typing a
    // fuzzy fragment over the listing re-scores locally.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "b1".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filter-only snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.query, "b1");
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b1.txt"]);

    // Invalid direct jump: the menu stays open with a bounded error
    // status (items suppressed), and Backspace recovers by ascending to
    // the last canonical directory.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: format!("{}/missing/", root.display()),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected error-status snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert!(snapshot.items.is_empty(), "items suppressed under error");
    assert!(matches!(
        snapshot.status,
        crate::protocol::TransientMenuStatusData::Empty { .. }
    ));
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected recovery snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}/a", root.display()));
    assert!(!snapshot.items.is_empty(), "recovered listing reinstated");

    // Path mode browses the whole filesystem: ascents past the tab root
    // continue to the filesystem root, where Backspace is a no-op. The
    // recovery above left the session at `/root/a`.
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected tab-root ascent snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}", root.display()));
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected /tmp ascent snapshot");
    };
    assert_eq!(snapshot.prompt, "Browse · /tmp");
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filesystem-root snapshot");
    };
    assert_eq!(snapshot.prompt, "Browse · /");
    assert_eq!(snapshot.query, "/");
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected root no-op snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.query, "/", "filesystem root Backspace is a no-op");

    connection.drain_bounded().await;
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 9): primary activation on a selected file closes the
/// path session and runs the ordinary selected-file open — the browse
/// activation itself is the authorization event that converts to exactly
/// one `SingleFile` grant. The grant is strictly single-file (siblings
/// fail `OutsideRoot`), duplicate opens return the same document id with
/// no second view, and a file that disappeared between listing and
/// activation fails without a grant or document leak.
#[tokio::test]
async fn path_browser_open_file_converts_browse_to_single_file_grant() {
    let root = temp_workspace("path-browser-open-file");
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("sub/b.txt"), "b").unwrap();
    fs::write(root.join("a.txt"), "hello").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-open-file"),
    ));
    let (tab_snapshot, tab_state) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    // Open the Path Browser with a fresh behavior stamp; a stale stamp
    // (a concurrent manifest publish bumps the connection-wide version)
    // is answered with a bounded Error, so resync and retry exactly like
    // the real client would.
    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    // Read the next transient-menu frame, skipping parse/analysis noise
    // that can race an exchange; labeled so failures name the phase.
    async fn recv_menu_frame(
        label: &'static str,
        connection: &mut TestConnection,
    ) -> ServerMessage {
        loop {
            match timeout(Duration::from_secs(5), connection.receive()).await {
                Ok(
                    message @ (ServerMessage::TransientMenuSnapshot(_)
                    | ServerMessage::TransientMenuClosed { .. }),
                ) => return message,
                Ok(_) => continue,
                Err(_) => panic!("{label}: timed out awaiting transient menu frame"),
            }
        }
    }

    // Open the path browser and filter down to the file (directory-first
    // order puts `sub` at index 0).
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "a.txt".to_string(),
            scope: None,
        })
        .await;
    let snapshot = recv_menu_frame("filter", &mut connection).await;
    let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
        panic!("filter: expected snapshot, got closed");
    };
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["a.txt"]);

    // Primary activation: session closes first, then DocumentOpened with
    // the ordinary follow-up chain (no capability token involved).
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let closed = recv_menu_frame("close-before-open", &mut connection).await;
    let ServerMessage::TransientMenuClosed { session_id } = closed else {
        panic!("close-before-open: expected closed, got {closed:?}");
    };
    assert_eq!(session_id, path_id, "menu closes before the open response");
    let (opened_root_id, opened_document_id) =
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::DocumentOpened { metadata, head }) => {
                assert_eq!(head.first_chunk, "hello");
                assert_eq!(metadata.path, "a.txt");
                (metadata.workspace_root_id, metadata.document_id)
            }
            Ok(other) => panic!("expected DocumentOpened, got {other:?}"),
            Err(_) => panic!("timed out awaiting DocumentOpened"),
        };
    // Follow-ups arrive after the open frame; drain them.
    connection.drain_bounded().await;

    // The browse activation became a single-file grant: opening a
    // sibling document under that root fails OutsideRoot.
    connection
        .send(&ClientMessage::OpenDocument {
            client_id: 11,
            workspace_root_id: opened_root_id,
            path: "sub/b.txt".to_string(),
        })
        .await;
    let mut saw_outside_root = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::FileOperationFailed {
                code: FileErrorCode::OutsideRoot,
                workspace_root_id: Some(id),
                document_id: None,
                ..
            }) if id == opened_root_id => {
                saw_outside_root = true;
                break;
            }
            Ok(ServerMessage::DecorationSet(_))
            | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
            | Ok(ServerMessage::RuntimeDiagnostic(_))
            | Ok(ServerMessage::BehaviorManifest(_)) => {}
            Ok(other) => panic!("expected outside-root failure, got {other:?}"),
            Err(_) => panic!("timed out awaiting outside-root failure"),
        }
    }
    assert!(saw_outside_root, "single-file grant rejects siblings");
    connection.drain_bounded().await;

    // Duplicate open: reopening the same file returns the same document
    // id with no second view or grant.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "a.txt".to_string(),
            scope: None,
        })
        .await;
    let snapshot = recv_menu_frame("second-filter", &mut connection).await;
    let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
        panic!("second-filter: expected snapshot, got closed");
    };
    assert_eq!(snapshot.items.len(), 1);
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let closed = recv_menu_frame("close-duplicate", &mut connection).await;
    assert!(
        matches!(closed, ServerMessage::TransientMenuClosed { .. }),
        "close-duplicate: expected closed, got {closed:?}"
    );
    let mut duplicate_id = None;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::DocumentOpened { metadata, .. }) => {
                duplicate_id = Some(metadata.document_id);
                break;
            }
            Ok(ServerMessage::DecorationSet(_))
            | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
            | Ok(ServerMessage::RuntimeDiagnostic(_))
            | Ok(ServerMessage::BehaviorManifest(_)) => {}
            Ok(other) => panic!("expected duplicate DocumentOpened, got {other:?}"),
            Err(_) => panic!("timed out awaiting duplicate DocumentOpened"),
        }
    }
    assert_eq!(
        duplicate_id,
        Some(opened_document_id),
        "duplicate open returns the existing document"
    );
    connection.drain_bounded().await;
    let workspace = tab_state.workspace.lock().await;
    assert!(
        workspace
            .document_canonical_path(opened_document_id)
            .is_some()
    );
    assert!(
        workspace
            .document_canonical_path(opened_document_id + 1)
            .is_none(),
        "no second document created by the duplicate open"
    );
    drop(workspace);

    // Disappeared file: the listing was taken before deletion, so the
    // activation still resolves to the stale canonical path; the open
    // fails with a bounded FileOperationFailed, the session is closed,
    // and no grant or document appears.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "a.txt".to_string(),
            scope: None,
        })
        .await;
    let snapshot = recv_menu_frame("third-filter", &mut connection).await;
    let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
        panic!("third-filter: expected snapshot, got closed");
    };
    assert_eq!(snapshot.items.len(), 1, "listing predates the deletion");
    fs::remove_file(root.join("a.txt")).unwrap();
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let closed = recv_menu_frame("close-failed-open", &mut connection).await;
    assert!(
        matches!(closed, ServerMessage::TransientMenuClosed { .. }),
        "close-failed-open: expected closed, got {closed:?}"
    );
    let mut failed = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::FileOperationFailed { .. }) => {
                failed = true;
                break;
            }
            Ok(ServerMessage::DecorationSet(_))
            | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
            | Ok(ServerMessage::RuntimeDiagnostic(_))
            | Ok(ServerMessage::BehaviorManifest(_)) => {}
            Ok(other) => panic!("expected FileOperationFailed, got {other:?}"),
            Err(_) => panic!("timed out awaiting FileOperationFailed"),
        }
    }
    assert!(failed, "disappeared file fails without an open");
    connection.drain_bounded().await;
    let workspace = tab_state.workspace.lock().await;
    assert!(
        workspace
            .document_canonical_path(opened_document_id)
            .is_some()
    );
    assert!(
        workspace
            .document_canonical_path(opened_document_id + 1)
            .is_none(),
        "failed open allocates no document"
    );
    drop(workspace);

    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 10): secondary activation (Alt+Enter) on a selected
/// directory closes the path session and opens that directory as the
/// current tab's workspace — the browse activation itself is the
/// authorization event that converts ephemeral browse authority into a
/// `Directory` root grant. The bound tab's registry row rebinds to the
/// canonical directory root and the file browser refreshes to the new
/// root; a foreign tab's row is untouched; reopening the same directory
/// deduplicates to the same root id; secondary activation on a file
/// rejects without mutation.
#[tokio::test]
async fn path_browser_workspace_open_rebinds_only_bound_tab() {
    let root = temp_workspace("path-browser-open-workspace");
    fs::create_dir(root.join("alpha")).unwrap();
    fs::create_dir(root.join("alpha/inner")).unwrap();
    fs::write(root.join("alpha/file.txt"), "hi").unwrap();
    fs::write(root.join("beta.txt"), "b").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let alpha = fs::canonicalize(root.join("alpha")).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-open-workspace"),
    ));
    let (tab_snapshot, _tab_state) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    // The workspace pane is visible by default in fresh tab state, so the
    // rebind refresh carries the file-browser listing.
    // A second tab owned by a foreign client must stay untouched by the
    // bound tab's workspace open.
    let (foreign_snapshot, _) = server
        .create_tab_state(7, root.to_string_lossy().into_owned())
        .await
        .expect("foreign tab state is created");
    let foreign_tab_id = foreign_snapshot
        .tabs
        .iter()
        .find(|tab| tab.client_id == 7)
        .expect("foreign tab present")
        .tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    // Seed listing: directory-first order puts `alpha` at index 0.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["alpha", "beta.txt"]);

    // Alt+Enter on the selected directory: the session closes first,
    // then the bound tab rebinds to the canonical directory root and the
    // file browser refreshes to the new root's listing.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    let mut closed_id = None;
    let mut registry_snapshot = None;
    let mut saw_browser_refresh = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                assert_eq!(session_id, path_id, "workspace open closes the session");
                closed_id = Some(session_id);
            }
            Ok(ServerMessage::TabRegistry(snapshot)) => registry_snapshot = Some(snapshot),
            Ok(ServerMessage::SduiSnapshot { tree, .. }) => {
                // The refresh is the file-browser tree for the new root.
                // Other snapshots (late reclaim follow-ups with the
                // hidden editor-only tree) are noise; keep scanning.
                // Directory labels carry a trailing separator ("inner/").
                let labels: Vec<String> = tree
                    .nodes
                    .iter()
                    .find_map(|node| match &node.kind {
                        SduiNodeKind::List { items, .. } => {
                            Some(items.iter().map(|item| item.label.clone()).collect())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();
                if labels.iter().any(|label| label == "inner/")
                    && labels.iter().any(|label| label == "file.txt")
                {
                    saw_browser_refresh = true;
                }
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting workspace rebind frames"),
        }
        if closed_id.is_some() && registry_snapshot.is_some() && saw_browser_refresh {
            break;
        }
    }
    assert_eq!(closed_id, Some(path_id));
    assert!(
        saw_browser_refresh,
        "file browser refresh for the new root never arrived"
    );
    let registry_snapshot = registry_snapshot.expect("TabRegistry snapshot after workspace open");
    let bound = registry_snapshot
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .expect("bound tab present");
    assert_eq!(bound.workspace_root, alpha.to_string_lossy().as_ref());
    let foreign = registry_snapshot
        .tabs
        .iter()
        .find(|tab| tab.tab_id == foreign_tab_id)
        .expect("foreign tab present");
    assert_eq!(
        foreign.workspace_root,
        root.to_string_lossy().as_ref(),
        "other tabs' roots are untouched"
    );
    let bound_root_id = registry_snapshot
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .expect("bound tab present")
        .workspace_root_id;
    connection.drain_bounded().await;

    // Reopening the same directory deduplicates to the same root id: the
    // tab's seed is now the rebound root, so ascend back to the original
    // root and activate `alpha` again.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    assert_eq!(
        snapshot.prompt,
        format!("Browse · {}", alpha.display()),
        "seed follows the rebound tab workspace root"
    );
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected ascent snapshot");
    };
    assert_eq!(snapshot.prompt, format!("Browse · {}", root.display()));
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    loop {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TabRegistry(snapshot)) => {
                let bound = snapshot
                    .tabs
                    .iter()
                    .find(|tab| tab.tab_id == tab_id)
                    .expect("bound tab present");
                assert_eq!(
                    bound.workspace_root_id, bound_root_id,
                    "same canonical directory deduplicates to the same root id"
                );
                break;
            }
            Ok(ServerMessage::TransientMenuClosed { .. }) => {}
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting deduplicated TabRegistry snapshot"),
        }
    }
    connection.drain_bounded().await;

    // Secondary activation on a file is not a workspace open: the
    // session closes and the bounded diagnostic names the rejection.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "file.txt".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filter snapshot");
    };
    assert_eq!(snapshot.items.len(), 1);
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    let mut saw_rejection = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { .. }) => {}
            Ok(ServerMessage::Error { message, .. }) => {
                assert!(
                    message.contains("no activation"),
                    "file has no secondary activation: {message}"
                );
                saw_rejection = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting file-activation rejection"),
        }
    }
    assert!(saw_rejection);

    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}
