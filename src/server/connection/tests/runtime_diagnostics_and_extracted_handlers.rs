use super::*;

/// Runtime-diagnostic retention: consecutive duplicates collapse, the
/// deque never exceeds its capacity, and drops are counted (Plan 060 T6,
/// P1-8).
#[test]
fn runtime_diagnostic_store_deduplicates_and_bounds() {
    let mut store = RuntimeDiagnosticStore::default();
    let duplicate = RuntimeDiagnostic::warning("test.dup", "same");
    store.push(duplicate.clone());
    store.push(duplicate);
    assert_eq!(
        store.snapshot().len(),
        1,
        "consecutive duplicate must collapse"
    );
    assert_eq!(store.dropped_count(), 0);

    for index in 0..crate::perf::budgets::RUNTIME_DIAGNOSTIC_CAPACITY + 8 {
        store.push(RuntimeDiagnostic::warning(
            "test.flood",
            format!("diagnostic {index}"),
        ));
    }
    let snapshot = store.snapshot();
    assert_eq!(
        snapshot.len(),
        crate::perf::budgets::RUNTIME_DIAGNOSTIC_CAPACITY,
        "retention must stay within the snapshot cap"
    );
    // 41 total entries (1 duplicate survivor + 40 flood) minus the 32
    // retained = 9 dropped; "diagnostic 8" is the oldest survivor.
    assert_eq!(store.dropped_count(), 9);
    assert_eq!(
        snapshot.first().map(|d| d.message.as_str()),
        Some("diagnostic 8"),
        "oldest entries drop first"
    );
}

#[test]
fn sdui_command_request_forwards_list_item_id_as_argument() {
    let intent = SduiActionIntent {
        command_id: "settings.setTheme".to_string(),
        source: SduiActionSource::ListItem {
            node_id: SduiNodeId(7),
            item_id: "@clay/theme-modus-vivendi".to_string(),
        },
        arguments: Vec::new(),
    };
    let request = sdui_command_request(&intent);
    assert_eq!(request.command_id, "settings.setTheme");
    assert_eq!(
        request.arguments,
        serde_json::json!({ "item_id": "@clay/theme-modus-vivendi" })
    );
}

#[test]
fn sdui_command_request_forwards_button_node_id_as_argument() {
    let intent = SduiActionIntent {
        command_id: "settings.close".to_string(),
        source: SduiActionSource::Button {
            node_id: SduiNodeId(42),
        },
        arguments: Vec::new(),
    };
    let request = sdui_command_request(&intent);
    assert_eq!(request.arguments, serde_json::json!({ "node_id": "42" }));
}

#[test]
fn sdui_command_request_preserves_explicit_arguments() {
    let intent = SduiActionIntent {
        command_id: "workspace.openFile".to_string(),
        source: SduiActionSource::Button {
            node_id: SduiNodeId(1),
        },
        arguments: vec![SduiActionArgument {
            name: "path".to_string(),
            value: SduiActionValue::String("/tmp/a.md".to_string()),
        }],
    };
    let request = sdui_command_request(&intent);
    // Explicit arguments are preserved; source node_id is added additively.
    assert_eq!(
        request.arguments,
        serde_json::json!({ "path": "/tmp/a.md", "node_id": "1" })
    );
}

#[tokio::test]
async fn agent_surface_commands_project_client_toggle_without_server_state() {
    // Plan 108 task 8: `coding-agent.profile` / `coding-agent.close` are
    // user-authorized presentation toggles. The dispatcher answers with the
    // narrow shell-client request (the client re-parses deny-by-default);
    // no runtime generation advances and no server state changes.
    // Plan 117: the agent settings page toggle rides the same lane.
    let workspace = workspace_state();
    let document = document_state();
    let sdui = sdui_state();
    let empty_registry = CommandRegistry::new();

    for command_id in [
        "coding-agent.profile",
        "coding-agent.close",
        "coding-agent.agentSettings.open",
        "coding-agent.agentSettings.close",
    ] {
        let response = execute_command_intent(
            CommandExecutionRequest {
                command_id: command_id.to_string(),
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&workspace),
            &document,
            &sdui,
            1,
            None,
            &empty_registry,
        )
        .await;
        assert!(
            matches!(
                response,
                Some(ServerMessage::ShellClientCommandRequest { command_id: ref id })
                    if id == command_id
            ),
            "{command_id} should project one shell-client request"
        );
    }
}

/// Plan 119 SC-6: the tab's own `TabState` answer must name its session *and*
/// the requesting client, because the relay is a process-wide fan-out — a
/// store can only adopt the binding that carries its own connection identity.
#[test]
fn tab_state_binding_names_the_requesting_client_and_session() {
    let message = session_bound_message(7, 3, "sess-42");
    let AgentServerMessage::AgentRpc { code, result_json } = message else {
        panic!("a tab binding is an agent-RPC answer");
    };
    assert_eq!(code, "session.bound");
    let value: serde_json::Value = serde_json::from_str(&result_json).expect("json payload");
    assert_eq!(value["clientId"], 7);
    assert_eq!(value["tabId"], 3);
    assert_eq!(value["sessionId"], "sess-42");

    // A tab with no session yet still adopts: the empty id means "this tab
    // owns nothing", never "unknown owner" (which stays a client-side state).
    let message = session_bound_message(7, 3, "");
    let AgentServerMessage::AgentRpc { result_json, .. } = message else {
        panic!("a tab binding is an agent-RPC answer");
    };
    let value: serde_json::Value = serde_json::from_str(&result_json).expect("json payload");
    assert_eq!(value["sessionId"], "");
}

#[tokio::test]
async fn extracted_list_documents_handler_answers_without_the_loop() {
    let mut state = DirectHandlerState::new();
    let (mut server_side, mut peer) = duplex(64 * 1024);
    {
        let mut ctx = state.ctx(&mut server_side);
        super::super::documents::handle_list_documents(&mut ctx)
            .await
            .expect("list documents writes its response");
    }
    let ServerMessage::DocumentList { documents } = state
        .codec
        .read_server_message(&mut peer)
        .await
        .expect("the handler wrote one response")
    else {
        panic!("list documents answers with the document list");
    };
    assert!(
        documents.is_empty(),
        "a fresh connection owns no documents: {documents:?}"
    );
}

#[tokio::test]
async fn extracted_launcher_handler_answers_without_the_loop() {
    let mut state = DirectHandlerState::new();
    let (mut server_side, mut peer) = duplex(64 * 1024);
    {
        let mut ctx = state.ctx(&mut server_side);
        super::super::workspace::handle_list_launcher_entries(&mut ctx)
            .await
            .expect("launcher listing writes its response");
    }
    let ServerMessage::LauncherEntries { client_id, entries } = state
        .codec
        .read_server_message(&mut peer)
        .await
        .expect("the handler wrote one response")
    else {
        panic!("launcher listing answers with the launcher entries");
    };
    assert_eq!(client_id, state.client_id);
    assert!(
        entries.workspaces.is_empty() && entries.agents.is_empty(),
        "no configuration root means the first-run empty launcher: {entries:?}"
    );
    assert_eq!(entries.pruned, 0);
}

#[tokio::test]
async fn extracted_duplicate_hello_handler_rejects_without_the_loop() {
    let mut state = DirectHandlerState::new();
    let (mut server_side, mut peer) = duplex(64 * 1024);
    {
        let mut ctx = state.ctx(&mut server_side);
        super::super::runtime::handle_duplicate_hello(&mut ctx)
            .await
            .expect("the duplicate-hello reply writes");
    }
    let ServerMessage::Error { code, message } = state
        .codec
        .read_server_message(&mut peer)
        .await
        .expect("the handler wrote one response")
    else {
        panic!("a second Hello answers with a protocol error");
    };
    assert_eq!(code, ProtocolErrorCode::InvalidMessage);
    assert_eq!(message, "duplicate Hello message");
}
