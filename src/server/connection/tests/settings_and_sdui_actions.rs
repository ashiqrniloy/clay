use super::*;

#[tokio::test]
async fn sdui_actions_and_keybinding_intents_share_command_execution_path() {
    let sdui_request = sdui_command_request(&SduiActionIntent::command(
        "controlCenter.open",
        SduiActionSource::Button {
            node_id: SduiNodeId(5),
        },
    ));
    let keybinding_request = CommandExecutionRequest {
        command_id: "controlCenter.open".to_string(),
        arguments: serde_json::Value::Null,
        target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
        provenance: None,
        expected_permissions: Vec::new(),
    };

    let document = document_state();
    let sdui = sdui_state();
    assert_eq!(
        execute_command_intent(
            sdui_request,
            workspace_state(),
            &document,
            &sdui,
            1,
            None,
            &CommandRegistry::new(),
        )
        .await,
        None
    );
    assert_eq!(
        execute_command_intent(
            keybinding_request,
            workspace_state(),
            &document,
            &sdui,
            1,
            None,
            &CommandRegistry::new(),
        )
        .await,
        None
    );
}

#[tokio::test]
async fn reload_command_intent_uses_shared_server_reload_service() {
    let root = temp_workspace("reload-command-intent");
    fs::write(root.join("init.js"), "").unwrap();
    let mut config = super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("reload-command-intent"),
    );
    config.configuration_root = Some(root.clone());
    let server = super::super::super::IpcServer::new(config);

    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "runtime.reloadConfiguration".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::Global,
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
    .await
    .expect("reload command returns status");

    assert!(matches!(
        response,
        ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. })
            if code == "runtime.reload_succeeded"
    ));
    assert_eq!(server.runtime_generation.generation_id().await, 2);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn settings_live_switch_persists_and_reloads_end_to_end() {
    // Plan 067 task 12: full live-switch + persistence matrix through the
    // real command executor + persist + reload path. From a clean config:
    // settings.setTheme selects Gruvbox (proving Gruvbox remains
    // selectable), persists to preferences.json, and advances the runtime
    // generation so the change applies live via reload→fanout;
    // settings.setAppearance persists appearance; settings.reset clears the
    // store and reloads; a non-bundled @clay/theme-* specifier is rejected
    // by execute_settings without advancing the generation.
    let root = temp_workspace("settings-live-switch-e2e");
    fs::write(root.join("init.js"), "").unwrap();
    let mut config = super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("settings-live-switch-e2e"),
    );
    config.configuration_root = Some(root.clone());
    let server = super::super::super::IpcServer::new(config);
    let preferences = root.join("preferences.json");
    let workspace = workspace_state();
    let document = document_state();
    let sdui = sdui_state();

    let settings_registry = CommandRegistry::new();
    let settings_request = |command_id: &str, item_id: &str| {
        execute_command_intent(
            CommandExecutionRequest {
                command_id: command_id.to_string(),
                arguments: serde_json::json!({ "item_id": item_id }),
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&workspace),
            &document,
            &sdui,
            1,
            Some(&server),
            &settings_registry,
        )
    };

    // 0. Open/close remain validated server commands but project one
    //    narrow client UI request without reloading.
    assert!(matches!(
        settings_request("settings.open", "").await,
        Some(ServerMessage::ShellClientCommandRequest { command_id })
            if command_id == "settings.open"
    ));
    assert_eq!(server.runtime_generation.generation_id().await, 1);

    // 1. settings.setTheme selects Gruvbox Material Light (opt-in theme
    //    remains selectable), persists, and reloads live.
    let response =
        settings_request("settings.setTheme", "@clay/theme-gruvbox-material-light").await;
    assert!(
        response.is_none(),
        "settings.setTheme returns no error on success"
    );
    assert_eq!(server.runtime_generation.generation_id().await, 2);
    let persisted = fs::read_to_string(&preferences).expect("preferences.json written");
    assert!(
        persisted.contains("@clay/theme-gruvbox-material-light"),
        "preferences.json persists the selected theme: {persisted}"
    );

    // 2. settings.setAppearance persists appearance and reloads again.
    let response = settings_request("settings.setAppearance", "light").await;
    assert!(
        response.is_none(),
        "settings.setAppearance returns no error on success"
    );
    assert_eq!(server.runtime_generation.generation_id().await, 3);
    let persisted = fs::read_to_string(&preferences).expect("preferences.json updated");
    assert!(
        persisted.contains("\"appearance\"") && persisted.contains("light"),
        "preferences.json persists appearance: {persisted}"
    );

    // 3. Complete typography persists and reloads through the same parser.
    let typography = serde_json::json!({
        "monospace": { "families": ["Mono"], "size": 16 },
        "proportional": { "families": ["Sans"], "size": 17 },
        "ui": { "families": ["UI"], "size": 14 },
        "hierarchy": {
            "display": 1.5, "title": 1.16, "section": 1.08,
            "body": 1.0, "status": 1.0, "detail": 0.83, "caption": 0.75
        }
    });
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "settings.setTypography".to_string(),
            arguments: serde_json::json!({ "typography": typography.to_string() }),
            target: CommandExecutionTarget::Global,
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(&workspace),
        &document,
        &sdui,
        1,
        Some(&server),
        &settings_registry,
    )
    .await;
    assert!(response.is_none(), "complete typography applies");
    assert_eq!(server.runtime_generation.generation_id().await, 4);
    let persisted = fs::read_to_string(&preferences).expect("typography persisted");
    assert!(persisted.contains("\"typography\"") && persisted.contains("\"UI\""));

    // 4. A non-bundled @clay/theme-* specifier is rejected by execute_settings
    //    and does not advance the generation (authority denial).
    let generation_before = server.runtime_generation.generation_id().await;
    let response = settings_request("settings.setTheme", "@clay/theme-evil").await;
    assert!(
        matches!(response, Some(ServerMessage::Error { .. })),
        "non-bundled theme specifier is rejected"
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before,
        "rejected settings intent does not reload"
    );

    // 5. settings.reset clears the persisted store and reloads.
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "settings.reset".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::Global,
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(&workspace),
        &document,
        &sdui,
        1,
        Some(&server),
        &CommandRegistry::new(),
    )
    .await;
    assert!(
        response.is_none(),
        "settings.reset returns no error on success"
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before + 1
    );
    let reset = fs::read_to_string(&preferences).unwrap_or_default();
    assert!(
        !reset.contains("@clay/theme-"),
        "preferences.json cleared after reset: {reset}"
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn settings_set_design_system_persists_and_snapshot_lists_choices() {
    // Plan 110 task 10: settings.setDesignSystem persists the designSystem
    // preference and reloads live; the committed runtime snapshot enumerates
    // the installed theme/design-system packages plus the persisted appearance
    // so the Settings panel renders real choice lists.
    let root = temp_workspace("settings-design-system-e2e");
    fs::write(root.join("init.js"), "").unwrap();
    let mut config = super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("settings-design-system-e2e"),
    );
    config.configuration_root = Some(root.clone());
    let server = super::super::super::IpcServer::new(config);
    let preferences = root.join("preferences.json");
    let workspace = workspace_state();
    let document = document_state();
    let sdui = sdui_state();

    let registry = CommandRegistry::new();
    let settings_request = |command_id: &str, arguments: serde_json::Value| {
        execute_command_intent(
            CommandExecutionRequest {
                command_id: command_id.to_string(),
                arguments,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&workspace),
            &document,
            &sdui,
            1,
            Some(&server),
            &registry,
        )
    };
    let item = |specifier: &str| serde_json::json!({ "item_id": specifier });

    // 1. Appearance persists and shows up in the snapshot choices.
    let response = settings_request("settings.setAppearance", item("light")).await;
    assert!(response.is_none(), "setAppearance accepted");
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    // 2. Design system selection persists and reloads. A real bundled DS
    //    package now applies through the shared enable path (plan 110 task 18
    //    fixed the package-service double-lock deadlock); the specifier is
    //    suffix-built to stay plan-104 source-independence-guard-proof.
    let ds_suffix = "instrument";
    let ds_specifier = format!("@clay/design-{ds_suffix}");
    let response = settings_request("settings.setDesignSystem", item(&ds_specifier)).await;
    assert!(response.is_none(), "setDesignSystem accepted");
    assert_eq!(server.runtime_generation.generation_id().await, 3);
    let persisted = fs::read_to_string(&preferences).expect("preferences written");
    assert!(persisted.contains("designSystem"));

    // 3. Theme selection enables the theme record; the snapshot enumerates it.
    let response = settings_request(
        "settings.setTheme",
        item("@clay/theme-gruvbox-material-light"),
    )
    .await;
    assert!(response.is_none(), "setTheme accepted");
    assert_eq!(server.runtime_generation.generation_id().await, 4);

    let snapshot = server
        .runtime_generation
        .latest_runtime_snapshot_for(1)
        .await
        .expect("committed runtime snapshot");
    let choices = snapshot.ui_choices;
    assert_eq!(choices.appearance.as_deref(), Some("light"));
    assert!(
        choices
            .themes
            .iter()
            .any(|theme| theme.specifier == "@clay/theme-gruvbox-material-light"),
        "enabled theme is enumerated: {:?}",
        choices.themes
    );
    // Plan 118 task 20: the choice set is exactly what ships — the built-in core
    // baseline first, then the enabled design-system packages, sorted — so the
    // Settings dropdown shows the shipped set and nothing else.
    assert_eq!(
        choices
            .design_systems
            .iter()
            .map(|option| option.specifier.as_str())
            .collect::<Vec<_>>(),
        vec!["@clay/core", ds_specifier.as_str()],
        "exactly the shipped design-system choices, core baseline first"
    );
    assert_eq!(
        snapshot.active_design_system.specifier.as_str(),
        ds_specifier,
        "committed snapshot carries the enabled design system"
    );
    assert!(
        !snapshot.active_design_system.recipes.is_empty(),
        "active design system carries the package's resolved recipes"
    );

    // 4. Invalid specifiers are rejected without persisting or reloading.
    let generation_before = server.runtime_generation.generation_id().await;
    let response = settings_request(
        "settings.setDesignSystem",
        item("@clay/theme-modus-vivendi"),
    )
    .await;
    assert!(
        matches!(response, Some(ServerMessage::Error { .. })),
        "non-design-system specifier is rejected"
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn persisted_design_system_preference_applies_at_startup_reload() {
    // Plan 110 task 18 regression: a persisted non-core design-system choice
    // is applied by `apply_persisted_preferences` during the startup reload
    // without deadlocking the package service. The bounded timeout makes a
    // regression fail the test instead of hanging CI.
    let ds_suffix = "instrument";
    let ds_specifier = format!("@clay/design-{ds_suffix}");
    let root = temp_workspace("persisted-design-system-startup");
    fs::write(root.join("init.js"), "").unwrap();
    fs::write(
        root.join("preferences.json"),
        serde_json::json!({ "designSystem": ds_specifier }).to_string(),
    )
    .unwrap();
    let mut config = super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("persisted-design-system-startup"),
    );
    config.configuration_root = Some(root.clone());
    let server = super::super::super::IpcServer::new(config);

    let outcome = timeout(Duration::from_secs(5), server.reload_runtime_generation())
        .await
        .expect("startup reload with a persisted non-core DS choice must not hang");
    assert!(outcome.reloaded, "startup reload succeeds");
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    let snapshot = server
        .runtime_generation
        .latest_runtime_snapshot_for(1)
        .await
        .expect("committed runtime snapshot");
    assert_eq!(
        snapshot.active_design_system.specifier.as_str(),
        ds_specifier,
        "persisted design system applied at startup"
    );
    assert!(
        !snapshot.active_design_system.recipes.is_empty(),
        "applied design system carries the package's resolved recipes"
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn persisted_removed_design_system_preference_commits_the_core_baseline() {
    // Plan 118 task 20: a preference naming a design system this generation
    // removed must not fail the generation or leave a half-installed snapshot.
    // The committed generation carries the core baseline — the host-consumed
    // subset of the shipped language — and the shipped choice set is unchanged.
    // The specifier is assembled so the plan-118 absence guard sees no literal of
    // a removed package name.
    let removed = format!("@clay/design-{}", "neobrutal");
    let shipped = format!("@clay/design-{}", "instrument");
    let root = temp_workspace("persisted-removed-design-system-startup");
    fs::write(root.join("init.js"), "").unwrap();
    fs::write(
        root.join("preferences.json"),
        serde_json::json!({ "designSystem": removed }).to_string(),
    )
    .unwrap();
    let mut config = super::super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("persisted-removed-design-system-startup"),
    );
    config.configuration_root = Some(root.clone());
    let server = super::super::super::IpcServer::new(config);

    let outcome = timeout(Duration::from_secs(5), server.reload_runtime_generation())
        .await
        .expect("startup reload with a removed design-system preference must not hang");
    assert!(outcome.reloaded, "startup reload succeeds");
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    let snapshot = server
        .runtime_generation
        .latest_runtime_snapshot_for(1)
        .await
        .expect("committed runtime snapshot");
    assert_eq!(
        snapshot.active_design_system.specifier.as_str(),
        "@clay/core",
        "a removed design system falls back to the core baseline"
    );
    assert_eq!(
        snapshot.active_design_system.provenance.package_name,
        "core"
    );
    assert!(
        !snapshot.active_design_system.recipes.is_empty(),
        "the fallback carries the shipped language's recipes"
    );
    assert_eq!(
        snapshot
            .ui_choices
            .design_systems
            .iter()
            .map(|option| option.specifier.as_str())
            .collect::<Vec<_>>(),
        vec!["@clay/core", shipped.as_str()],
        "the rejected preference leaves the shipped choice set intact"
    );
    // The shipped system is offered (with its declared display name) without a
    // prior loadPackage, so the dropdown matches what the command accepts.
    assert_eq!(
        snapshot.ui_choices.design_systems[1]
            .display_name
            .as_deref(),
        Some("Quiet Instrument")
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn package_ui_unregistered_action_is_rejected_by_command_execution() {
    let response = execute_command_intent(
        sdui_command_request(&SduiActionIntent::command(
            "markdown.missingCommand",
            SduiActionSource::Button {
                node_id: SduiNodeId(5),
            },
        )),
        workspace_state(),
        &document_state(),
        &sdui_state(),
        1,
        None,
        &CommandRegistry::new(),
    )
    .await
    .expect("unknown package UI action returns protocol error");

    assert!(matches!(response, ServerMessage::Error { .. }));
    if let ServerMessage::Error { message, .. } = response {
        assert!(message.contains("UnknownCommand"));
    }
}

#[tokio::test]
async fn workspace_directory_action_sends_refreshed_file_browser_snapshot() {
    let root = temp_workspace("navigate-snapshot");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let workspace = workspace_state();
    let root_id = workspace.lock().await.add_root(&root).unwrap();
    let document = document_state();
    let sdui = sdui_state();
    let mut intent = SduiActionIntent::command(
        "workspace.openDirectory",
        SduiActionSource::ListItem {
            node_id: SduiNodeId(5),
            item_id: "src".to_string(),
        },
    );
    intent.arguments = vec![
        SduiActionArgument {
            name: "workspaceRootId".to_string(),
            value: SduiActionValue::U64(root_id),
        },
        SduiActionArgument {
            name: "relativePath".to_string(),
            value: SduiActionValue::String("src".to_string()),
        },
    ];

    let response = execute_command_intent(
        sdui_command_request(&intent),
        workspace,
        &document,
        &sdui,
        42,
        None,
        &CommandRegistry::new(),
    )
    .await
    .expect("directory navigation sends a snapshot");

    let ServerMessage::SduiSnapshot { client_id, tree } = response else {
        panic!("expected SduiSnapshot");
    };
    assert_eq!(client_id, 42);
    let labels: Vec<String> = tree
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            SduiNodeKind::List { items, .. } => {
                Some(items.iter().map(|item| item.label.clone()).collect())
            }
            _ => None,
        })
        .unwrap();
    assert!(labels.iter().any(|label| label == "Parent folder"));
    assert!(labels.iter().any(|label| label == "main.rs"));

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn file_browser_action_survives_markdown_open_followup_diagnostic() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = temp_workspace("browser-survives-open-followup");
    fs::write(root.join("note.md"), "# note\n").unwrap();

    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let browser = FileBrowserState::from_workspace(&workspace_state_value, root_id).unwrap();
    let tree = browser.to_sdui_tree(1u64, 1u64);
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
    let sdui = empty_sdui_state();
    sdui.lock()
        .await
        .replace_for_document_with_runtime_tree(1, tree)
        .unwrap();

    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
    let metadata = DocumentMetadata {
        document_id: 2,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: root_id,
        path: "note.md".to_string(),
    };

    let document = Arc::new(Mutex::new(DocumentState::new(
        2,
        "# note\n".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let messages = super::super::open_document_followup_messages(
        &metadata,
        &document,
        &behavior,
        &sdui,
        1,
        &runtime,
        &coordinator,
    )
    .await;
    assert!(messages.iter().any(|message| {
        matches!(
            message,
            ServerMessage::BehaviorManifest(_) | ServerMessage::RuntimeDiagnostic(_)
        )
    }));
    sdui.lock()
        .await
        .validate_action(&action)
        .expect("file-browser action remains valid after open-time follow-up");

    let _ = fs::remove_dir_all(root);
}
