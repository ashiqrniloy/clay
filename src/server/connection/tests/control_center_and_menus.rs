use super::*;

/// Phase 24.1: menu intents naming sessions this connection does not hold
/// are dropped with a bounded `menu.unknown_session` diagnostic — never an
/// error or disconnect. Sessions are per-tab (per-connection), so the
/// connection must be tab-bound first.
#[tokio::test]
async fn menu_intents_for_unknown_sessions_produce_bounded_diagnostics() {
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("menu-unknown-session"),
    ));
    let (snapshot, _state) = server
        .create_tab_state(
            11,
            temp_workspace("menu-unknown")
                .to_string_lossy()
                .into_owned(),
        )
        .await
        .expect("tab state is created");
    let tab_id = snapshot.tabs[0].tab_id;

    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    for message in [
        ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: 1 << 63 | 7,
            query: "reload".to_string(),
            scope: None,
        },
        ClientMessage::MenuSelectionMove {
            client_id: 11,
            session_id: 1 << 63 | 7,
            delta: 1,
        },
        ClientMessage::MenuActivate {
            client_id: 11,
            session_id: 1 << 63 | 7,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        },
        ClientMessage::MenuCancel {
            client_id: 11,
            session_id: 1 << 63 | 7,
        },
    ] {
        connection.send(&message).await;
        let response = connection.receive().await;
        assert!(
            matches!(
                response,
                ServerMessage::RuntimeDiagnostic(ref diagnostic)
                    if diagnostic.code == "menu.unknown_session"
            ),
            "unexpected response: {response:?}"
        );
    }

    // The connection is still alive and functional (never a disconnect).
    connection
        .send(&ClientMessage::ListDocuments { client_id: 11 })
        .await;
    assert!(matches!(
        connection.receive_response().await,
        ServerMessage::DocumentList { .. }
    ));
    connection.close().await;
}

#[tokio::test]
async fn control_center_opens_filters_activates_and_cancels() {
    // Plan 086 task 7: whole-workflow bound. A hang here means pending
    // session cleanup, not a slow machine (measured ~0.03s); the timeout
    // names the failure instead of waiting indefinitely.
    timeout(
        Duration::from_secs(5),
        control_center_opens_filters_activates_and_cancels_scenario(),
    )
    .await
    .expect(
        "control_center_opens_filters_activates_and_cancels exceeded its 5s whole-workflow bound; \
         look for pending session or reply-receiver cleanup",
    );
}

#[tokio::test]
async fn control_center_shell_activation_sends_shell_command_request() {
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("control-center-shell"),
    ));
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
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
    assert!(
        snapshot
            .items
            .iter()
            .any(|item| item.id == "shell.clientSplitPaneVertical"),
        "shell.client* entries must appear in the Control Center listing"
    );

    // Fuzzy query narrows to exactly the shell entry.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id,
            query: "clientSplitPaneVertical".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered TransientMenuSnapshot");
    };
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].id, "shell.clientSplitPaneVertical");

    // Activation closes the menu, then ships the narrow shell-command
    // request frame the client re-parses deny-by-default.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == session_id
    ));
    assert_eq!(
        connection.receive().await,
        ServerMessage::ShellClientCommandRequest {
            command_id: "shell.clientSplitPaneVertical".to_string(),
        }
    );
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn menu_backspace_deletes_one_char_and_secondary_activation_matches_primary() {
    let server = super::super::super::IpcServer::new(super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("menu-backspace-secondary"),
    ));
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
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

    // Backspace on an empty query is a bounded no-op snapshot.
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(after_empty_backspace) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert_eq!(after_empty_backspace.query, "");

    // Backspace deletes exactly one query character (Control Center
    // semantics; path mode overrides with ascend in task 8).
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id,
            query: "clientSplitPaneVertical".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered TransientMenuSnapshot");
    };
    assert_eq!(filtered.items.len(), 1);
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(after_backspace) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert_eq!(after_backspace.query, "clientSplitPaneVertica");

    // Restore the exact query, then activate with the secondary kind:
    // the Control Center executes the same selection as primary.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id,
            query: "clientSplitPaneVertical".to_string(),
            scope: None,
        })
        .await;
    let _ = receive_menu_message(&mut connection).await;
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == session_id
    ));
    assert_eq!(
        connection.receive().await,
        ServerMessage::ShellClientCommandRequest {
            command_id: "shell.clientSplitPaneVertical".to_string(),
        }
    );
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn runtime_generation_replacement_cancels_open_control_center() {
    // Plan 086 task 7: whole-workflow bound. A hang here means the
    // replacement left a pending session or reply receiver; the timeout
    // names that instead of waiting indefinitely.
    timeout(
        Duration::from_secs(5),
        runtime_generation_replacement_cancels_open_control_center_scenario(),
    )
    .await
    .expect(
        "runtime_generation_replacement_cancels_open_control_center exceeded its 5s whole-workflow bound; \
         look for pending session or reply-receiver cleanup",
    );
}
