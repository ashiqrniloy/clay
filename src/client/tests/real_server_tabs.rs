use super::*;

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
            ServerMessage::Welcome { .. }
            | ServerMessage::FileOpenCapabilityIssued { .. }
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
            ServerMessage::Welcome { .. }
            | ServerMessage::FileOpenCapabilityIssued { .. }
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
        SduiNodeKind::List { items, .. } if items.iter().any(|item| item.label == "notes.md")
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
            Err(error)
                if error.kind() == super::super::ClientBootstrapErrorKind::TransportUnavailable =>
            {
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
            Err(error)
                if error.kind() == super::super::ClientBootstrapErrorKind::TransportUnavailable =>
            {
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
