use super::*;

#[tokio::test]
async fn tab_switch_cancels_the_active_server_menu_session() {
    let root_a = temp_workspace("menu-tab-alpha");
    let root_b = temp_workspace("menu-tab-beta");
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("menu-tab-switch"),
    ));
    let (first_snapshot, _) = server
        .create_tab_state(11, root_a.to_string_lossy().into_owned())
        .await
        .expect("first tab state is created");
    let first_tab = first_snapshot.tabs[0].tab_id;
    let (second_snapshot, _) = server
        .create_tab_state(11, root_b.to_string_lossy().into_owned())
        .await
        .expect("second tab state is created");
    let second_tab = second_snapshot.tabs[0].tab_id;

    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, first_tab).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    let session_id = snapshot.session_id;

    // Switching to the second tab dismisses the open menu (Escape-free
    // dismissal on focus loss) with an explicit close frame.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 11,
            command: crate::protocol::TabCommand::Activate { tab_id: second_tab },
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == session_id
    ));
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn agent_lane_toggle_projects_a_shell_client_request() {
    // Plan 124: `shell.toggleAgentLane` is declared ClientUi in the default
    // manifest, so its Global ServerFirst `Ctrl+X Ctrl+P` intent must come
    // back as the narrow shell-client request the shell executes (the lane's
    // visibility is client-local per-tab layout state) — not a wire error
    // from the server command executor, and no runtime generation bump.
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("agent-lane-toggle"),
    ));
    let generation_before = server.runtime_generation.generation_id().await;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "shell.toggleAgentLane".to_string(),
        })
        .await;
    assert_eq!(
        connection.receive().await,
        ServerMessage::ShellClientCommandRequest {
            command_id: "shell.toggleAgentLane".to_string(),
        }
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before,
        "a client-local toggle must not advance the runtime generation"
    );
    connection.close().await;
}

#[tokio::test]
async fn package_command_dispatchs_through_shared_dispatcher_with_live_registry() {
    // A validated package command resolves through the live aggregated
    // registry passed by the menu-activation path (not the empty registry
    // SDUI/CommandIntent use): the dispatcher returns `None` (Accepted —
    // the JS side effect runs in the package runtime, no wire message).
    let mut registry = CommandRegistry::new();
    registry.insert_test_command(crate::packages::commands::RegisteredCommand {
        command_id: "markdown.togglePreview".to_string(),
        display_name: "Toggle Markdown Preview".to_string(),
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
        key_bindings: Vec::new(),
        permissions: vec![crate::packages::permissions::PackagePermission::ParseDocument],
        custom_properties: BTreeMap::new(),
    });
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "markdown.togglePreview".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
            provenance: None,
            expected_permissions: Vec::new(),
        },
        workspace_state(),
        &document_state(),
        &sdui_state(),
        1,
        None,
        &registry,
    )
    .await;
    assert_eq!(response, None, "validated package commands accept silently");
}

/// Phase 22.7 (task 3): a rejected `Close` (foreign tab) pushes the
/// reconciling snapshot and the sender's connection keeps serving.
#[tokio::test]
async fn rejected_close_keeps_connection_serving() {
    let root = temp_workspace("rejected-close");
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.add_root(&root).unwrap();
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
    let (registry, tab_registry_tx) = two_tab_registry();
    let mut connection = TestConnection::connect_with_registry(
        99,
        document,
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
        Arc::clone(&registry),
        tab_registry_tx,
    )
    .await;

    // A (client 99) tries to close B's tab (tab 2): rejected.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 99,
            command: crate::protocol::TabCommand::Close { tab_id: 2 },
        })
        .await;
    let snapshot = receive_tab_registry_snapshot(&mut connection).await;
    // Registry unchanged: both tabs still bound, tab 2 still owned by 7.
    assert_eq!(snapshot.tabs.len(), 2);
    assert!(
        snapshot
            .tabs
            .iter()
            .any(|entry| entry.tab_id == 2 && entry.client_id == 7)
    );
    assert!(
        snapshot
            .tabs
            .iter()
            .any(|entry| entry.tab_id == 1 && entry.client_id == 99)
    );

    // A's next command still processes: activate its own tab succeeds and
    // pushes another snapshot (the connection never ended).
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 99,
            command: crate::protocol::TabCommand::Activate { tab_id: 1 },
        })
        .await;
    let snapshot = receive_tab_registry_snapshot(&mut connection).await;
    assert_eq!(snapshot.active, Some(1));

    assert!(registry.lock().await.snapshot().tabs.len() == 2);
    connection.close().await;
}

/// Phase 22.7 (task 3): an accepted `Close` (own tab) still ends the
/// connection (EOF on the client stream) and removes the tab.
#[tokio::test]
async fn accepted_close_still_ends_connection() {
    let root = temp_workspace("accepted-close");
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.add_root(&root).unwrap();
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
    let (registry, tab_registry_tx) = two_tab_registry();
    let mut connection = TestConnection::connect_with_registry(
        99,
        document,
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
        Arc::clone(&registry),
        tab_registry_tx,
    )
    .await;

    // A closes its own tab (tab 1): accepted, the connection ends.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 99,
            command: crate::protocol::TabCommand::Close { tab_id: 1 },
        })
        .await;
    // The server task resolves and the client stream reaches EOF.
    timeout(
        Duration::from_secs(2),
        connection.codec.read_server_message(&mut connection.client),
    )
    .await
    .expect("EOF expected within the timeout")
    .expect_err("accepted close must end the connection (EOF)");
    // The tab is gone from the shared registry; the foreign tab remains.
    let snapshot = registry.lock().await.snapshot();
    assert_eq!(snapshot.tabs.len(), 1);
    assert!(
        snapshot
            .tabs
            .iter()
            .any(|entry| entry.tab_id == 2 && entry.client_id == 7)
    );
}
