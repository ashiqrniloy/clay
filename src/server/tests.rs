use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
    time::SystemTime,
};

use tokio::{io::duplex, net::UnixStream, sync::Mutex, task::JoinSet};

use super::{ActiveBehaviorManifest, IpcServer, RuntimeGenerationStore, ServerConfig};
use crate::server::{
    language_intelligence::LanguageIntelligenceCoordinator, locks::ScopedLockManager,
    parse_coordinator::ParseCoordinator, sdui::StaticSduiState,
};
use crate::{
    protocol::{
        ClientMessage, DocumentAccess, EditOperation, EditRejection, LockOwner, PROTOCOL_VERSION,
        SduiNodeKind, ServerMessage, TabCommand, codec::Codec,
    },
    server::document::DocumentState,
};

fn unique_socket_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("clay-{name}-{}-{unique}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    dir.join("clay.sock")
}

fn server_with_document(socket_path: &std::path::Path, document: DocumentState) -> IpcServer {
    server_with_document_and_registry(
        socket_path,
        document,
        Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new())),
    )
}

fn server_with_document_and_registry(
    socket_path: &std::path::Path,
    document: DocumentState,
    tab_registry: Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
) -> IpcServer {
    let document = Arc::new(Mutex::new(document));
    let workspace = {
        let mut workspace = crate::server::workspace::WorkspaceState::new();
        workspace
            .add_root(socket_path.parent().expect("socket parent"))
            .expect("test workspace root");
        workspace.reserve_document_ids_from(2);
        Arc::new(Mutex::new(workspace))
    };
    let bootstrap_state = super::TabServerState {
        welcome: Arc::clone(&document),
        workspace: Arc::clone(&workspace),
        workspace_pane_visible: Arc::new(AtomicBool::new(true)),
    };
    IpcServer {
        config: ServerConfig::new(socket_path),
        codec: Codec::default(),
        bootstrap_state,
        tab_states: Arc::new(Mutex::new(std::collections::HashMap::new())),
        document_id_allocator: Arc::new(AtomicU64::new(2)),
        bootstrap_consumed: Arc::new(AtomicBool::new(false)),
        document,
        behavior: Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        workspace,
        sdui: Arc::new(Mutex::new(StaticSduiState::empty_for_document(1))),
        active_theme: Arc::new(Mutex::new(None)),
        active_design_system: Arc::new(Mutex::new(
            crate::shell::design_system::ActiveDesignSystem::core_fallback(0),
        )),
        active_icon_pack: Arc::new(Mutex::new(None)),
        runtime_diagnostics: Arc::new(Mutex::new(
            super::connection::RuntimeDiagnosticStore::default(),
        )),
        connection_permits: Arc::new(tokio::sync::Semaphore::new(
            crate::perf::budgets::MAX_ACTIVE_CONNECTIONS,
        )),
        parse_coordinator: ParseCoordinator::default(),
        completion: crate::server::completion::CompletionCoordinator::new(),
        document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
        language_intelligence: LanguageIntelligenceCoordinator::new(),
        runtime_generation: RuntimeGenerationStore::initial(),
        scoped_locks: ScopedLockManager::default(),
        reload_attempt: Arc::new(Mutex::new(())),
        next_client_id: Arc::new(AtomicU64::new(1)),
        live_clients: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        tab_registry,
        tab_registry_tx: tokio::sync::broadcast::channel(16).0,
        agent: super::agent::AgentHost::inert(),
        #[cfg(test)]
        reload_barrier: super::ReloadCandidateBarrier::default(),
    }
}

#[tokio::test]
async fn live_command_catalogue_contains_builtins_and_exact_shell_surface() {
    let socket_path = unique_socket_path("command-catalogue");
    let server = server_with_document(
        &socket_path,
        DocumentState::new(
            1,
            "welcome".to_string(),
            crate::protocol::DocumentAccess::Editable { lease_id: 1 },
        ),
    );
    let manifest = server.behavior.lock().await.manifest_for(1).clone();
    let (generation_id, catalogue) = server
        .runtime_generation
        .command_catalogue_snapshot(&manifest)
        .await
        .expect("live catalogue should validate");
    let ids: std::collections::HashSet<_> = catalogue
        .commands()
        .iter()
        .map(|command| command.command_id.as_str())
        .collect();

    assert_eq!(generation_id, 1);
    for command_id in crate::server::command_execution::builtin_server_command_ids() {
        assert!(ids.contains(command_id));
    }
    for (command_id, _) in crate::client_commands::SHELL_CLIENT_COMMAND_CATALOGUE {
        assert!(ids.contains(command_id));
        assert!(crate::client_commands::ShellClientCommand::from_command_id(command_id).is_some());
    }

    fs::remove_dir_all(socket_path.parent().expect("socket directory")).unwrap();
}

#[tokio::test]
async fn deferred_initial_state_waits_for_tab_binding() {
    let socket_path = unique_socket_path("deferred-handshake");
    let selected_root = socket_path.parent().unwrap().join("selected");
    fs::create_dir(&selected_root).unwrap();
    fs::write(selected_root.join("selected.txt"), "selected\n").unwrap();
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let (client, server_stream) = duplex(64 * 1024);
    let mut connections = JoinSet::new();
    server.spawn_connection(server_stream, &mut connections);
    let mut client = client;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "deferred-handshake".to_string(),
            },
        )
        .await
        .unwrap();
    let client_id = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::Welcome { client_id, .. } => client_id,
        message => panic!("expected Welcome, got {message:?}"),
    };
    let mut behavior_version = 1;
    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            ServerMessage::InitialDocument { .. } | ServerMessage::SduiSnapshot { .. } => {
                panic!("document/SDUI leaked before tab binding")
            }
            ServerMessage::BehaviorManifest(manifest) => {
                behavior_version = manifest.behavior_version;
            }
            ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::RuntimeStateSnapshot(_)
            | ServerMessage::TabRegistry(_) => {}
            message => panic!("unexpected pre-bind message: {message:?}"),
        }
    }
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::TabCommand {
                client_id,
                command: TabCommand::New {
                    workspace_root: selected_root.to_string_lossy().into_owned(),
                },
            },
        )
        .await
        .unwrap();
    let (document_id, workspace_root) = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::InitialDocument {
                document_id,
                workspace_root,
                ..
            } => break (document_id, workspace_root),
            ServerMessage::SduiSnapshot { .. }
            | ServerMessage::RuntimeStateSnapshot(_)
            | ServerMessage::TabRegistry(_) => {}
            message => panic!("expected bound InitialDocument, got {message:?}"),
        }
    };
    assert_eq!(
        workspace_root,
        fs::canonicalize(&selected_root)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    );
    // The workspace browser is visible by default (the empty pane offers
    // Open File/Folder and an explicit workspace root must be shown).
    let tree = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => break tree,
            ServerMessage::TabRegistry(_) => {}
            message => panic!("expected bound workspace SDUI, got {message:?}"),
        }
    };
    assert!(tree.nodes.iter().any(|node| matches!(
        &node.kind,
        SduiNodeKind::List { items } if items.iter().any(|item| item.label == "selected.txt")
    )));

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::CommandIntent {
                client_id,
                document_id,
                behavior_version,
                command_id: "workspace.toggleFileBrowser".to_string(),
            },
        )
        .await
        .unwrap();
    let hidden_tree = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => break tree,
            ServerMessage::TabRegistry(_) => {}
            message => panic!("expected hidden workspace SDUI, got {message:?}"),
        }
    };
    assert!(hidden_tree.nodes.iter().all(|node| {
        !matches!(
            &node.kind,
            SduiNodeKind::Panel { .. } | SduiNodeKind::List { .. }
        )
    }));

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::CommandIntent {
                client_id,
                document_id,
                behavior_version,
                command_id: "workspace.toggleFileBrowser".to_string(),
            },
        )
        .await
        .unwrap();
    let tree = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => break tree,
            ServerMessage::TabRegistry(_) => {}
            message => panic!("expected toggled workspace SDUI, got {message:?}"),
        }
    };
    assert!(tree.nodes.iter().any(|node| matches!(
        &node.kind,
        SduiNodeKind::List { items } if items.iter().any(|item| item.label == "selected.txt")
    )));

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::CommandIntent {
                client_id,
                document_id,
                behavior_version,
                command_id: "workspace.toggleFileBrowser".to_string(),
            },
        )
        .await
        .unwrap();
    let hidden_tree = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::SduiSnapshot { tree, .. } => break tree,
            ServerMessage::TabRegistry(_) => {}
            message => panic!("expected hidden workspace SDUI, got {message:?}"),
        }
    };
    assert!(hidden_tree.nodes.iter().all(|node| {
        !matches!(
            &node.kind,
            SduiNodeKind::Panel { .. } | SduiNodeKind::List { .. }
        )
    }));

    drop(client);
    connections.abort_all();
    let _ = fs::remove_dir_all(socket_path.parent().unwrap());
}

/// Phase 22.7 verification pass: a LIVE connected client's tab never
/// expires. Client A opens a tab, its registry entry is aged far beyond
/// the TTL, then client B connects — B's arrival sweep must skip A's
/// stale-but-live entry (tab-idle presence, e.g. an hour of document
/// editing, never touches the registry; sweeping it would unmount a live
/// client's tab out from under its window).
#[tokio::test]
async fn sweep_skips_tabs_of_live_connected_clients() {
    let socket_path = unique_socket_path("sweep-liveness");
    let registry = Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new()));
    let server = server_with_document_and_registry(
        &socket_path,
        DocumentState::default(),
        Arc::clone(&registry),
    );
    let server_task = tokio::spawn(server.run());

    // Client A connects and opens a tab.
    let codec = Codec::default();
    let mut stream_a = connect_with_retry(&socket_path).await;
    codec
        .write_client_message(
            &mut stream_a,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "sweep-a".to_string(),
            },
        )
        .await
        .unwrap();
    let client_id_a = loop {
        if let ServerMessage::Welcome { client_id, .. } =
            codec.read_server_message(&mut stream_a).await.unwrap()
        {
            break client_id;
        }
    };
    codec
        .write_client_message(
            &mut stream_a,
            &ClientMessage::TabCommand {
                client_id: client_id_a,
                command: crate::protocol::TabCommand::New {
                    workspace_root: socket_path
                        .parent()
                        .expect("socket dir")
                        .to_string_lossy()
                        .to_string(),
                },
            },
        )
        .await
        .unwrap();
    let created = loop {
        match codec.read_server_message(&mut stream_a).await.unwrap() {
            ServerMessage::TabRegistry(snapshot) if snapshot.tabs.len() == 1 => {
                break snapshot;
            }
            _ => {}
        }
    };
    let tab_id = created.tabs[0].tab_id;

    // Age A's entry far beyond the TTL while A stays connected.
    registry
        .lock()
        .await
        .age_all_entries_for_test(std::time::Duration::from_secs(7200));

    // Client B connects: its arrival sweep runs before its handshake, so
    // reading B's Welcome proves the sweep already ran.
    let mut stream_b = connect_with_retry(&socket_path).await;
    codec
        .write_client_message(
            &mut stream_b,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "sweep-b".to_string(),
            },
        )
        .await
        .unwrap();
    loop {
        if let ServerMessage::Welcome { .. } =
            codec.read_server_message(&mut stream_b).await.unwrap()
        {
            break;
        }
    }

    // A's stale tab survived the arrival sweep: same entry, unchanged
    // revision (a removal would bump it and broadcast).
    let after = registry.lock().await.snapshot();
    assert_eq!(after.tabs.len(), 1, "live client's tab must not expire");
    assert_eq!(after.tabs[0].tab_id, tab_id);
    assert_eq!(after.active, Some(tab_id));
    assert_eq!(
        after.revision, created.revision,
        "sweep removed nothing: revision unchanged"
    );

    drop(stream_a);
    drop(stream_b);
    server_task.abort();
}

#[tokio::test]
async fn real_server_end_to_end_region_locked_edit_rejected() {
    let socket_path = unique_socket_path("region-lock");
    let mut document = DocumentState::new(
        1,
        "welcome".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    let lock_id = document
        .register_region_lock(0, 7, LockOwner::Server)
        .unwrap();
    let server = server_with_document(&socket_path, document);
    let server_task = tokio::spawn(server.run());

    let mut stream = connect_with_retry(&socket_path).await;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "region-lock-test".to_string(),
            },
        )
        .await
        .unwrap();

    let client_id = match codec.read_server_message(&mut stream).await.unwrap() {
        ServerMessage::Welcome { client_id, .. } => client_id,
        message => panic!("expected Welcome, got {message:?}"),
    };
    let behavior_version = match codec.read_server_message(&mut stream).await.unwrap() {
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
    let (document_id, version, lease_id) = loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::InitialDocument {
                document_id,
                version,
                access: DocumentAccess::Editable { lease_id },
                lease_id: Some(snapshot_lease_id),
                ..
            } => {
                assert_eq!(lease_id, snapshot_lease_id);
                break (document_id, version, lease_id);
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
                base_version: version,
                behavior_version,
                transaction_id: 12,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "x".to_string(),
                },
            },
        )
        .await
        .unwrap();

    loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::EditRejected {
                document_id: rejected_document_id,
                transaction_id: 12,
                reason: EditRejection::RegionLocked { conflict },
            } if rejected_document_id == document_id
                && conflict.lock_id == lock_id
                && conflict.start == 0
                && conflict.end == 7
                && conflict.owner == LockOwner::Server
                && conflict.created_at_version == version =>
            {
                break;
            }
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. }
            | ServerMessage::TabRegistry(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::FileOpenCapabilityIssued { .. } => {}
            other => panic!("expected region-lock rejection, got {other:?}"),
        }
    }

    server_task.abort();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[test]
fn server_accepts_configured_workspace_roots_and_reports_invalid_roots() {
    let socket_path = unique_socket_path("configured-workspace");
    let root = socket_path.parent().unwrap().join("workspace");
    fs::create_dir(&root).unwrap();

    let mut config = ServerConfig::new(&socket_path);
    config.workspace_roots = vec![root.clone()];
    let server = IpcServer::try_new(config).unwrap();
    assert_eq!(server.config.workspace_roots, vec![root]);

    let missing_root = socket_path.parent().unwrap().join("missing");
    let mut invalid_config = ServerConfig::new(&socket_path);
    invalid_config.workspace_roots = vec![missing_root];
    let error = IpcServer::try_new(invalid_config).unwrap_err();
    assert!(matches!(error, super::ServerError::InvalidWorkspaceRoot(_)));
    assert!(error.to_string().contains("invalid workspace root"));

    let _ = fs::remove_dir(server.config.workspace_roots[0].clone());
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[test]
fn production_server_binaries_use_fallible_constructor() {
    for path in ["src/launch.rs", "src/bin/clay-server.rs"] {
        let source =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
                .expect("read production binary source");
        assert!(
            source.contains("IpcServer::try_new"),
            "{path} must use try_new"
        );
        assert!(
            !source.contains("IpcServer::new"),
            "{path} must not use the panic constructor"
        );
    }
}

#[tokio::test]
async fn default_server_starts_without_workspace_sdui_snapshot() {
    let socket_path = unique_socket_path("no-default-sdui");
    let server = IpcServer::new(ServerConfig::new(&socket_path));

    assert!(server.sdui.lock().await.snapshot_message(1).is_none());

    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

/// Unix endpoint parent policy: group/world-writable parents are rejected
/// unless the sticky bit marks a sanctioned shared-temp policy; the
/// default owner-only runtime directory stays valid (Plan 060 T6, P1-10).
#[cfg(unix)]
#[test]
fn socket_parent_mode_policy() {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!(
        "clay-parent-mode-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let socket = root.join("clay.sock");

    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    super::validate_socket_path(&socket).expect("owner-only parent must be accepted");

    fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
    let error = super::validate_socket_path(&socket)
        .expect_err("world-writable parent without sticky bit must be rejected");
    assert!(
        error.to_string().contains("group/world-writable"),
        "unexpected error: {error}"
    );

    fs::set_permissions(&root, fs::Permissions::from_mode(0o1777)).unwrap();
    super::validate_socket_path(&socket).expect("sticky shared parent must be accepted");

    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let _ = fs::remove_dir(root);
}

/// The accept loop refuses connections once every permit is held, and a
/// released permit lets the next connection spawn (disconnected clients
/// release capacity) (Plan 060 T6, P1-10).
#[tokio::test]
async fn connection_limit_refuses_excess_and_recovers() {
    let socket_path = unique_socket_path("connection-limit");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let permits: Vec<_> = (0..crate::perf::budgets::MAX_ACTIVE_CONNECTIONS)
        .map(|_| {
            server
                .connection_permits
                .clone()
                .try_acquire_owned()
                .expect("permits available")
        })
        .collect();

    let (stream, _client) = tokio::io::duplex(1024);
    let mut connections = tokio::task::JoinSet::new();
    server.spawn_connection(stream, &mut connections);
    assert!(
        connections.is_empty(),
        "exhausted permits must refuse the connection without spawning"
    );

    drop(permits);
    let (stream, _client) = tokio::io::duplex(1024);
    server.spawn_connection(stream, &mut connections);
    assert!(
        !connections.is_empty(),
        "a released permit must let the next connection spawn"
    );
    connections.abort_all();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

#[tokio::test]
async fn server_listener_accepts_client_hello() {
    let socket_path = unique_socket_path("listener-hello");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    let mut stream = connect_with_retry(&socket_path).await;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "listener-test".to_string(),
            },
        )
        .await
        .unwrap();

    let client_id = match codec.read_server_message(&mut stream).await.unwrap() {
        ServerMessage::Welcome { client_id, .. } => client_id,
        message => panic!("expected Welcome, got {message:?}"),
    };
    loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::RuntimeStateSnapshot(_)
            | ServerMessage::TabRegistry(_) => {}
            message => panic!("expected handshake message, got {message:?}"),
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
    loop {
        match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::InitialDocument {
                access: DocumentAccess::Editable { lease_id: 1 },
                ..
            } => break,
            ServerMessage::SduiSnapshot { .. }
            | ServerMessage::RuntimeStateSnapshot(_)
            | ServerMessage::TabRegistry(_) => {}
            message => panic!("expected InitialDocument, got {message:?}"),
        }
    }

    server_task.abort();
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(socket_path.parent().unwrap());
}

async fn connect_with_retry(socket_path: &std::path::Path) -> UnixStream {
    let mut last_error = None;
    for _ in 0..50 {
        match UnixStream::connect(socket_path).await {
            Ok(stream) => return stream,
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
    panic!("failed to connect to test socket: {:?}", last_error);
}

#[tokio::test]
async fn unix_socket_is_created_with_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let socket_path = unique_socket_path("owner-only");
    let server = IpcServer::new(ServerConfig::new(&socket_path));
    let server_task = tokio::spawn(server.run());

    let stream = connect_with_retry(&socket_path).await;
    drop(stream);

    let metadata = std::fs::metadata(&socket_path).unwrap();
    let mode = metadata.permissions().mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "socket must be created with owner-only permissions, got {mode:o}"
    );

    server_task.abort();
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_dir(socket_path.parent().unwrap());
}
