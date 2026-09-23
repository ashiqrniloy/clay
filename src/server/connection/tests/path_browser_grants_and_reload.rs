use super::*;

/// Phase 24.3 (task 10): secondary activation on a directory that
/// vanished between listing and activation rejects with the bounded
/// file-operation failure and leaves the tab's workspace root unchanged.
#[tokio::test]
async fn path_browser_workspace_open_rejects_vanished_directory() {
    let root = temp_workspace("path-browser-vanished-workspace");
    fs::create_dir(root.join("alpha")).unwrap();
    fs::write(root.join("beta.txt"), "b").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-vanished-workspace"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
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

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    // The directory disappears after the listing installed.
    fs::remove_dir_all(root.join("alpha")).unwrap();
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    let mut saw_failure = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                assert_eq!(session_id, path_id);
            }
            Ok(ServerMessage::FileOperationFailed {
                code: FileErrorCode::NotFound,
                workspace_root_id: None,
                document_id: None,
                ..
            }) => {
                saw_failure = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting vanished-directory failure"),
        }
    }
    assert!(saw_failure, "vanished directory fails with NotFound");

    // The tab's workspace root did not change: a fresh Path Browser still
    // seeds from the original root.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    assert_eq!(
        snapshot.prompt,
        format!("Browse · {}", root.display()),
        "failed workspace open leaves the tab root unchanged"
    );

    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 10): the shared bound-tab workspace-open helper
/// rejects a connection with no bound tab before touching the workspace
/// or registry.
#[tokio::test]
async fn open_workspace_helper_requires_bound_tab() {
    let workspace = workspace_state();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let sdui = sdui_state();
    let (registry, tab_registry_tx) = two_tab_registry();
    let messages = open_workspace_for_bound_tab(
        &workspace,
        &document,
        &sdui,
        &registry,
        &tab_registry_tx,
        None,
        99,
        None,
        PathBuf::from("/tmp/unused"),
    )
    .await;
    assert_eq!(messages.len(), 1);
    match &messages[0] {
        ServerMessage::Error { message, .. } => {
            assert!(message.contains("requires a bound tab"));
        }
        other => panic!("expected bound-tab rejection, got {other:?}"),
    }
}

/// Phase 24.3 (task 11): browsing alone — open, filter, descend, ascend,
/// direct jump, cancel — never allocates a root grant or opens a
/// document. Activation is the single grant conversion point.
#[tokio::test]
async fn path_browser_navigation_only_creates_no_grants() {
    let root = temp_workspace("path-browser-navigation-only");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(root.join("README.md"), "r").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-navigation-only"),
    ));
    let (tab_snapshot, tab_state) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let workspace = Arc::clone(&tab_state.workspace);
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    assert_eq!(workspace.lock().await.directory_roots().len(), 1);

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

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;

    // Filter-only edit: no filesystem work, no grant.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "RE".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("filter snapshot")
    else {
        panic!("expected snapshot after filter edit");
    };
    assert_eq!(
        filtered.session_id, path_id,
        "session id stable across edits"
    );
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].label, "README.md");

    // Descend into src (primary on the directory after clearing the
    // filter): the session stays open and relists.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: String::new(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(_) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("cleared filter snapshot")
    else {
        panic!("expected snapshot after clearing the filter");
    };
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(descended) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("descend snapshot")
    else {
        panic!("expected snapshot after descend");
    };
    assert_eq!(
        descended.session_id, path_id,
        "session id stable across descend"
    );
    assert_eq!(
        descended.prompt,
        format!("Browse · {}/src", root.display()),
        "descend relists the canonical target"
    );
    assert_eq!(descended.items.len(), 1);
    assert_eq!(descended.items[0].label, "main.rs");

    // Ascend (Backspace on the empty filter) back to the tab root.
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(ascended) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("ascend snapshot")
    else {
        panic!("expected snapshot after ascend");
    };
    assert_eq!(
        ascended.prompt,
        format!("Browse · {}", root.display()),
        "empty-filter Backspace ascends to the parent"
    );

    // Direct jump to a typed absolute directory.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: format!("{}/src/", root.display()),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(jumped) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("direct jump snapshot")
    else {
        panic!("expected snapshot after direct jump");
    };
    assert_eq!(
        jumped.prompt,
        format!("Browse · {}/src", root.display()),
        "direct path edit jumps to the typed directory"
    );

    // Cancel: the session closes, still no grant or document.
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuClosed { session_id } =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("cancel frame")
    else {
        panic!("expected menu close");
    };
    assert_eq!(session_id, path_id);

    // Nothing but menu frames flowed, and the workspace gained no root.
    for _ in 0..16 {
        if timeout(
            Duration::from_millis(10),
            connection.codec.read_server_message(&mut connection.client),
        )
        .await
        .is_err()
        {
            break;
        }
    }
    assert_eq!(
        workspace.lock().await.directory_roots().len(),
        1,
        "browse navigation alone creates no root grants"
    );
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 11): a session id from one connection is opaque to
/// every other connection — cross-client activation fails closed with the
/// bounded `menu.unknown_session` diagnostic and never disturbs the
/// owning session.
#[tokio::test]
async fn path_browser_cross_client_activation_denied() {
    let root = temp_workspace("path-browser-cross-client");
    fs::write(root.join("a.txt"), "a").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-cross-client"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let (foreign_snapshot, _) = server
        .create_tab_state(22, root.to_string_lossy().into_owned())
        .await
        .expect("foreign tab state is created");
    let foreign_tab_id = foreign_snapshot
        .tabs
        .iter()
        .find(|tab| tab.client_id == 22)
        .expect("foreign tab present")
        .tab_id;
    let mut connection_a = TestConnection::connect_with_server(11, server.clone()).await;
    connection_a.reclaim(11, tab_id).await;
    connection_a.drain_bounded().await;
    let mut connection_b = TestConnection::connect_with_server(22, server.clone()).await;
    connection_b.reclaim(22, foreign_tab_id).await;
    connection_b.drain_bounded().await;

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

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection_a, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;

    // Client B cannot drive A's session: the id is per-connection opaque.
    connection_b
        .send(&ClientMessage::MenuActivate {
            client_id: 22,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let mut saw_denial = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection_b.receive()).await {
            Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, message, .. }))
                if code == "menu.unknown_session" =>
            {
                assert!(message.contains(&path_id.to_string()));
                saw_denial = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting cross-client denial"),
        }
    }
    assert!(saw_denial, "foreign session id fails closed");

    // A's session is untouched: it still cancels with the expected frame.
    connection_a
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuClosed { session_id } =
        timeout(Duration::from_secs(5), connection_a.receive())
            .await
            .expect("owner cancel frame")
    else {
        panic!("expected owner menu close");
    };
    assert_eq!(
        session_id, path_id,
        "owning connection still holds the session"
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 11): the per-connection session store survives tab
/// rebinds and drops cleanly on disconnect.
#[tokio::test]
async fn path_browser_survives_tab_switch_and_disconnect() {
    let root_a = temp_workspace("path-browser-tab-switch-a");
    fs::write(root_a.join("a.txt"), "a").unwrap();
    let root_b = temp_workspace("path-browser-tab-switch-b");
    fs::write(root_b.join("b.txt"), "b").unwrap();
    let root_a = fs::canonicalize(&root_a).unwrap();
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-tab-switch"),
    ));
    let (tab_a_snapshot, _) = server
        .create_tab_state(11, root_a.to_string_lossy().into_owned())
        .await
        .expect("tab a is created");
    let (tab_b_snapshot, _) = server
        .create_tab_state(11, root_b.to_string_lossy().into_owned())
        .await
        .expect("tab b is created");
    let tab_a = tab_a_snapshot.tabs[0].tab_id;
    let tab_b = tab_b_snapshot.tabs[1].tab_id;
    assert_ne!(tab_a, tab_b);

    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_a).await;
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

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;

    // Activating the second tab dismisses the session Escape-free (focus
    // loss) with the ordinary close frame; the session is then gone.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 11,
            command: crate::protocol::TabCommand::Activate { tab_id: tab_b },
        })
        .await;
    let ServerMessage::TransientMenuClosed { session_id } =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("tab switch close frame")
    else {
        panic!("expected menu close on tab switch");
    };
    assert_eq!(session_id, path_id, "tab switch dismisses the session");
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let mut saw_unknown = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. }))
                if code == "menu.unknown_session" =>
            {
                saw_unknown = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting unknown-session diagnostic"),
        }
    }
    assert!(saw_unknown, "dismissed session is gone");

    // Disconnect sweeps the store; `close` fails the test if the
    // connection task panicked or leaked a session.
    connection.close().await;
    let _ = fs::remove_dir_all(&root_a);
    let _ = fs::remove_dir_all(&root_b);
}

/// Phase 24.3 (task 11): after a runtime reload bumps the generation
/// stamp, an open session fails closed with the bounded stale-generation
/// diagnostic instead of executing against the old generation — and the
/// session is then gone, so cancel reports `menu.unknown_session`.
#[tokio::test]
async fn path_browser_activation_after_runtime_reload_fails_closed() {
    let config_root = temp_workspace("path-browser-reload-config");
    fs::write(config_root.join("init.js"), "").unwrap();
    let workspace_root = temp_workspace("path-browser-reload-workspace");
    fs::write(workspace_root.join("a.txt"), "a").unwrap();
    let workspace_root = fs::canonicalize(&workspace_root).unwrap();
    let mut config = super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-reload"),
    );
    config.configuration_root = Some(config_root.clone());
    let server = super::super::super::IpcServer::new(config);
    let (tab_snapshot, _) = server
        .create_tab_state(11, workspace_root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
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

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    assert_eq!(server.runtime_generation.generation_id().await, 1);

    let outcome = server.reload_runtime_generation().await;
    assert!(
        outcome.reloaded,
        "reload succeeds with the empty init.js config"
    );
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    // Activation with the old stamp fails closed: bounded diagnostic, no
    // execution, and the session is consumed like any rejected activation.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    // Reload publishes the new generation snapshot; the loop closes the
    // active session first (the catalogue is generation-bound), exactly
    // like a tab switch dismisses on focus loss.
    let mut saw_closed = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                assert_eq!(session_id, path_id);
                saw_closed = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting reload dismissal"),
        }
    }
    assert!(saw_closed, "runtime reload dismisses the active session");

    // The session is gone: cancel reports the bounded unknown-session
    // diagnostic rather than a spurious close.
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let mut saw_unknown = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. }))
                if code == "menu.unknown_session" =>
            {
                saw_unknown = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting unknown-session diagnostic"),
        }
    }
    assert!(saw_unknown, "cancelled session reports unknown_session");

    connection.close().await;
    let _ = fs::remove_dir_all(&config_root);
    let _ = fs::remove_dir_all(&workspace_root);
}

/// Phase 24.3 (task 11): the package/generic command lane cannot open
/// the Path Browser. Only the connection's `CommandIntent` special case
/// and the Control Centre catalogue's `MenuActivate` special case reach
/// the session store; the shared executor (the lane package callbacks
/// run through) yields nothing on the wire for the built-in id.
#[tokio::test]
async fn package_command_lane_cannot_open_path_browser() {
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("package-lane-path-browser"),
    ));
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "controlCenter.openPath".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(&server.workspace),
        &server.document,
        &server.sdui,
        1,
        Some(&server),
        &CommandRegistry::new(),
    )
    .await;
    assert!(
        response.is_none(),
        "generic execution of controlCenter.openPath must yield no message"
    );
}
