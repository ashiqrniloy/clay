use super::*;

#[tokio::test]
async fn server_accepts_hello_and_sends_snapshot() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hello from server".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
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

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::Welcome {
            client_id: 99,
            protocol_version: PROTOCOL_VERSION,
        }
    );
    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::InitialDocument {
            document_id: 7,
            version: 1,
            head: crate::protocol::DocumentTextHead::complete("Hello from server".to_string(),),
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            workspace_root: String::new(),
        }
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn handshake_replays_committed_runtime_snapshot_with_pane_surfaces() {
    let generation = runtime_generation();
    let snapshot = crate::protocol::RuntimeStateSnapshot {
        runtime_generation_id: 2,
        client_id: 0,
        behavior: BehaviorManifest::minimal_text_editing(2),
        active_theme: crate::protocol::ActiveTheme {
            specifier: "@clay/default".into(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: crate::protocol::ActiveTypography::default(),
        active_design_system: crate::shell::design_system::ActiveDesignSystem::core_fallback(2),
        active_icon_pack: None,
        ui_choices: crate::protocol::UiChoicesSnapshot::default(),
        sdui_tree: crate::server::sdui::default_document_tree(1, 1),
        package_ui: crate::protocol::PackageUiSnapshot {
            version: 2,
            surfaces: vec![crate::protocol::EmptyTabContent {
                id: "coding-agent.surface".into(),
                package_name: "@clay/coding-agent".into(),
                component_json: r#"{"id":"coding-agent.root","kind":"panel","children":[]}"#.into(),
                action_targets: vec!["coding-agent.profile".into()],
                provenance: crate::protocol::PackageUiProvenance {
                    package_name: "@clay/coding-agent".into(),
                    package_version: "0.1.0".into(),
                    api_prefix: "coding-agent".into(),
                    trust_domain: crate::protocol::PackageUiTrustDomain::Trusted,
                },
            }],
            ..Default::default()
        },
        documents: Vec::new(),
        diagnostics: Vec::new(),
    };
    snapshot.validate().expect("handshake fixture snapshot");
    generation.publish_runtime_snapshot(snapshot).await;

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hello from server".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
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
        generation,
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

    let mut saw_surface = false;
    for _ in 0..64 {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::RuntimeStateSnapshot(snapshot) => {
                assert_eq!(snapshot.client_id, 99);
                assert_eq!(snapshot.package_ui.surfaces.len(), 1);
                assert_eq!(snapshot.package_ui.surfaces[0].id, "coding-agent.surface");
                saw_surface = true;
                break;
            }
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            _ => {}
        }
    }
    assert!(
        saw_surface,
        "handshake must replay the committed runtime snapshot so pane surfaces reach a connecting client"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn connection_state_route_follows_reclaim_and_fails_closed() {
    let root_a = temp_workspace("route-alpha");
    let root_b = temp_workspace("route-beta");
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("connection-state-route"),
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

    let alpha_route = route_connection_tab_state(
        11,
        Some(&server),
        &alpha_state.welcome,
        &alpha_state.workspace,
    )
    .await
    .expect("bound alpha route");
    assert_eq!(alpha_route.tab_id, Some(alpha_tab));
    assert!(Arc::ptr_eq(
        &alpha_route.state.workspace,
        &alpha_state.workspace
    ));
    assert!(!Arc::ptr_eq(
        &alpha_route.state.workspace,
        &beta_state.workspace
    ));

    fs::write(root_a.join("alpha.txt"), "alpha").expect("alpha file is written");
    fs::write(root_b.join("beta.txt"), "beta").expect("beta file is written");
    let alpha_root_id = alpha_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("alpha root")
        .workspace_root_id;
    let beta_root_id = beta_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("beta root")
        .workspace_root_id;
    let alpha_document = crate::server::workspace::open_existing_file_unlocked(
        &alpha_route.state.workspace,
        alpha_root_id,
        "alpha.txt",
        11,
    )
    .await
    .expect("alpha document opens in alpha state");
    let beta_document = crate::server::workspace::open_existing_file_unlocked(
        &beta_state.workspace,
        beta_root_id,
        "beta.txt",
        22,
    )
    .await
    .expect("beta document opens in beta state");
    let alpha_response = alpha_document.document.lock().await.apply_edit(
        alpha_document.document_id,
        11,
        alpha_document.access.lease_id(),
        1,
        1,
        crate::protocol::EditOperation::Insert {
            byte_offset: 5,
            text: "!".to_string(),
        },
    );
    assert!(matches!(alpha_response, ServerMessage::EditAck { .. }));
    let beta_document_state = beta_document.document.lock().await;
    assert_eq!(beta_document_state.version(), 1);
    assert!(!beta_document_state.is_dirty());
    assert_eq!(beta_document_state.text(), "beta");

    assert!(server.tab_registry.lock().await.reclaim(alpha_tab, 33));
    assert!(
        route_connection_tab_state(
            11,
            Some(&server),
            &alpha_state.welcome,
            &alpha_state.workspace,
        )
        .await
        .is_none()
    );
    let reclaimed_route = route_connection_tab_state(
        33,
        Some(&server),
        &alpha_state.welcome,
        &alpha_state.workspace,
    )
    .await
    .expect("reclaimed route");
    assert_eq!(reclaimed_route.tab_id, Some(alpha_tab));
    assert!(Arc::ptr_eq(
        &reclaimed_route.state.workspace,
        &alpha_state.workspace
    ));

    server.remove_tab_state(beta_tab).await;
    assert!(
        route_connection_tab_state(
            22,
            Some(&server),
            &beta_state.welcome,
            &beta_state.workspace,
        )
        .await
        .is_none()
    );

    let _ = fs::remove_dir_all(root_a);
    let _ = fs::remove_dir_all(root_b);
}
