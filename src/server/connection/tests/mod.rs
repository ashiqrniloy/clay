//! Stay-in-crate unit tests for a single server connection: handshake/reclaim,
//! path-browser authority, menus and the control centre, tab lifecycle, edit
//! ack/resync, viewport rendering, extracted handlers, and runtime diagnostics.
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc, time::SystemTime};

use crate::packages::commands::CommandRegistry;
use crate::protocol::ViewportRenderStatus;
use crate::protocol::{AgentServerMessage, KeyBindingContext, KeyCode};

use tokio::{
    io::duplex,
    sync::Mutex,
    time::{Duration, timeout},
};

use super::{
    RuntimeDiagnosticStore, handle_connection, route_connection_tab_state, session_bound_message,
};
// Moved family helpers (Plan 090 task 2) are glob re-exported in the
// connection module scope; the few names tests also import explicitly are
// imported from their family modules for unambiguous unqualified use.
use super::runtime::{
    execute_command_intent, language_intelligence_document_window,
    language_intelligence_document_window_for_behavior, sdui_command_request,
    static_package_completion_result,
};
use super::tabs::open_workspace_for_bound_tab;
use crate::protocol::ParseByteRange;
use crate::server::command_execution::{CommandExecutionRequest, CommandExecutionTarget};

fn workspace_state() -> Arc<Mutex<WorkspaceState>> {
    Arc::new(Mutex::new(WorkspaceState::new()))
}

fn sdui_state() -> Arc<Mutex<StaticSduiState>> {
    Arc::new(Mutex::new(StaticSduiState::for_document(1, 1)))
}

fn document_state() -> Arc<Mutex<DocumentState>> {
    Arc::new(Mutex::new(DocumentState::new(
        1,
        "".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )))
}

/// Document under test for window builders: they read the rope, not the id, so
/// the request's document id is what matters.
fn document_with_text(text: &str) -> DocumentState {
    DocumentState::new(
        1,
        text.to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )
}

fn empty_sdui_state() -> Arc<Mutex<StaticSduiState>> {
    Arc::new(Mutex::new(StaticSduiState::empty_for_document(1)))
}

fn runtime_diagnostics() -> Arc<Mutex<RuntimeDiagnosticStore>> {
    Arc::new(Mutex::new(RuntimeDiagnosticStore::default()))
}

fn active_theme_state() -> Arc<Mutex<Option<crate::protocol::ActiveTheme>>> {
    Arc::new(Mutex::new(None))
}

fn js_runtime() -> ClayJsRuntimeService {
    ClayJsRuntimeService::default()
}

// Test suites (see each file for its scope).
mod connection_identity;
mod control_center_and_menus;
mod document_open_and_viewport;
mod document_rendering_and_lifecycle;
mod edit_ack_and_resync;
mod handshake_and_reclaim;
mod language_intelligence;
mod markdown_modes;
mod path_browser_grants_and_reload;
mod protocol_and_bootstrap;
mod runtime_diagnostics_and_extracted_handlers;
mod settings_and_sdui_actions;
mod tab_lifecycle;
mod workspace_authority_and_path_browser;
fn runtime_generation() -> super::RuntimeGenerationStore {
    runtime_generation_from(js_runtime())
}

fn runtime_generation_from(runtime: ClayJsRuntimeService) -> super::RuntimeGenerationStore {
    super::RuntimeGenerationStore {
        current: Arc::new(Mutex::new(
            crate::server::runtime_state::RuntimeGeneration {
                id: 1,
                service: runtime,
                evaluation: None,
                diagnostics: Vec::new(),
            },
        )),
        typography: crate::server::runtime_state::ActiveTypographyState::default(),
        runtime_state: crate::server::runtime_state::ActiveRuntimeStateFanout::default(),
        behavior_grace: super::super::behavior::BehaviorGraceState::new(),
    }
}

fn parse_coordinator() -> ParseCoordinator {
    ParseCoordinator::default()
}

fn language_intelligence_coordinator() -> LanguageIntelligenceCoordinator {
    LanguageIntelligenceCoordinator::new()
}

async fn load_markdown_runtime(
    runtime: &ClayJsRuntimeService,
    coordinator: &ParseCoordinator,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
) {
    let evaluation = runtime
        .evaluate_controlled_module(
            r#"import { loadPackage } from "clay:packages";
import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
await loadPackage("@clay/markdown");
const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
serverActivateClassifiedMode(classification, { path: "README.md" });"#,
        )
        .await
        .expect("Markdown package load should evaluate");
    runtime
        .register_parse_handlers(coordinator, 1, &evaluation)
        .expect("Markdown parse handler should register");
    crate::server::runtime_state::apply_runtime_outputs(&evaluation, 1, behavior, sdui).await;
}

fn temp_workspace(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "clay-connection-workspace-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    dir
}
use crate::{
    protocol::{
        BehaviorManifest, BehaviorScope, ClientMessage, DocumentAccess, DocumentMetadata,
        EditOperation, EditRejection, FileErrorCode, PROTOCOL_VERSION, ProtocolErrorCode,
        RuntimeDiagnostic, SduiActionArgument, SduiActionIntent, SduiActionSource, SduiActionValue,
        SduiNodeId, SduiNodeKind, ServerMessage, TokenType, codec::Codec,
    },
    server::{
        behavior::ActiveBehaviorManifest, document::DocumentState,
        js_runtime::ClayJsRuntimeService, language_intelligence::LanguageIntelligenceCoordinator,
        parse_coordinator::ParseCoordinator, sdui::StaticSduiState, workspace::WorkspaceState,
    },
    shell::file_browser::FileBrowserState,
};

/// Plan 060 T4 test helpers: drain the bootstrap sequence through the
/// always-terminal capability issue so tests start from a clean cursor.
async fn drain_bootstrap(client: &mut tokio::io::DuplexStream, codec: Codec) -> String {
    loop {
        if let ServerMessage::FileOpenCapabilityIssued { token } =
            codec.read_server_message(client).await.unwrap()
        {
            return token;
        }
    }
}

struct TestConnection {
    client: tokio::io::DuplexStream,
    server_task: tokio::task::JoinHandle<Result<(), crate::protocol::codec::CodecError>>,
    codec: Codec,
    file_open_capability: String,
}

impl TestConnection {
    #[allow(
        clippy::too_many_arguments,
        reason = "test connection harness mirrors the server's explicit authority parameters"
    )]
    async fn connect(
        client_id: u64,
        document: Arc<Mutex<DocumentState>>,
        behavior: Arc<Mutex<ActiveBehaviorManifest>>,
        workspace: Arc<Mutex<WorkspaceState>>,
        runtime_generation: super::RuntimeGenerationStore,
        parse_coordinator: ParseCoordinator,
        document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
        language_intelligence: LanguageIntelligenceCoordinator,
    ) -> Self {
        let registry = Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new()));
        let (tab_registry_tx, _) = tokio::sync::broadcast::channel(16);
        Self::connect_with_registry(
            client_id,
            document,
            behavior,
            workspace,
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence,
            registry,
            tab_registry_tx,
        )
        .await
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "test connection harness mirrors the server's explicit authority parameters"
    )]
    async fn connect_with_registry(
        client_id: u64,
        document: Arc<Mutex<DocumentState>>,
        behavior: Arc<Mutex<ActiveBehaviorManifest>>,
        workspace: Arc<Mutex<WorkspaceState>>,
        runtime_generation: super::RuntimeGenerationStore,
        parse_coordinator: ParseCoordinator,
        document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
        language_intelligence: LanguageIntelligenceCoordinator,
        tab_registry: Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
        tab_registry_tx: tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
    ) -> Self {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let server_task = tokio::spawn(super::handle_connection_with_analysis(
            server,
            client_id,
            document,
            behavior,
            workspace,
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation,
            parse_coordinator,
            crate::server::completion::CompletionCoordinator::new(),
            document_analysis,
            language_intelligence,
            None,
            tab_registry,
            tab_registry_tx,
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
        let file_open_capability = drain_bootstrap(&mut client, codec).await;
        Self {
            client,
            server_task,
            codec,
            file_open_capability,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "test connection harness mirrors the production IpcServer connection wiring"
    )]
    async fn connect_with_server(client_id: u64, server: super::super::IpcServer) -> Self {
        let (client, server_stream) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::clone(&server.bootstrap_state.welcome);
        let behavior = Arc::clone(&server.behavior);
        let workspace = Arc::clone(&server.bootstrap_state.workspace);
        let sdui = Arc::clone(&server.sdui);
        let active_theme = Arc::clone(&server.active_theme);
        let runtime_diagnostics = Arc::clone(&server.runtime_diagnostics);
        let runtime_generation = server.runtime_generation.clone();
        let parse_coordinator = server.parse_coordinator.clone();
        let completion = server.completion.clone();
        let document_analysis = server.document_analysis.clone();
        let language_intelligence = server.language_intelligence.clone();
        let tab_registry = Arc::clone(&server.tab_registry);
        let tab_registry_tx = server.tab_registry_tx.clone();
        let server_task = tokio::spawn(super::handle_connection_with_analysis(
            server_stream,
            client_id,
            document,
            behavior,
            workspace,
            sdui,
            active_theme,
            runtime_diagnostics,
            runtime_generation,
            parse_coordinator,
            completion,
            document_analysis,
            language_intelligence,
            Some(server),
            tab_registry,
            tab_registry_tx,
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
        let file_open_capability = drain_bootstrap(&mut client, codec).await;
        Self {
            client,
            server_task,
            codec,
            file_open_capability,
        }
    }

    async fn reclaim(&mut self, client_id: u64, tab_id: crate::protocol::TabId) {
        self.send(&ClientMessage::TabCommand {
            client_id,
            command: crate::protocol::TabCommand::Reclaim { tab_id },
        })
        .await;
        let mut received_initial_document = false;
        loop {
            match self.receive().await {
                ServerMessage::InitialDocument { .. } => received_initial_document = true,
                ServerMessage::TabRegistry(_) if received_initial_document => return,
                ServerMessage::SduiSnapshot { .. }
                | ServerMessage::TabRegistry(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::FileOpenCapabilityIssued { .. }
                | ServerMessage::BehaviorManifest(_) => {}
                other => panic!("unexpected message during tab reclaim: {other:?}"),
            }
        }
    }

    async fn open_document(
        &mut self,
        client_id: u64,
        workspace_root_id: crate::protocol::WorkspaceRootId,
        path: &str,
    ) -> (DocumentMetadata, crate::protocol::BehaviorVersion) {
        self.send(&ClientMessage::OpenDocument {
            client_id,
            workspace_root_id,
            path: path.to_string(),
        })
        .await;
        let mut behavior_version = 1;
        loop {
            match self.receive().await {
                message @ ServerMessage::BehaviorManifest(_) => {
                    let ServerMessage::BehaviorManifest(manifest) = message else {
                        unreachable!();
                    };
                    behavior_version = manifest.behavior_version;
                }
                message @ ServerMessage::DocumentOpened { .. } => {
                    let ServerMessage::DocumentOpened { metadata, .. } = message else {
                        unreachable!();
                    };
                    return (metadata, behavior_version);
                }
                ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. }
                | ServerMessage::TabRegistry(_) => {}
                other => panic!("unexpected message during document open: {other:?}"),
            }
        }
    }

    async fn send(&mut self, message: &ClientMessage) {
        self.codec
            .write_client_message(&mut self.client, message)
            .await
            .unwrap();
    }

    async fn receive(&mut self) -> ServerMessage {
        self.codec
            .read_server_message(&mut self.client)
            .await
            .unwrap()
    }

    async fn close(self) {
        drop(self.client);
        self.server_task.await.unwrap().unwrap();
    }

    /// Drain open/activation follow-ups until the stream goes quiet so the
    /// next read observes the response to the next request, not a queued
    /// BehaviorManifest/decoration frame.
    async fn drain_until_quiet(&mut self) {
        while timeout(
            Duration::from_millis(50),
            self.codec.read_server_message(&mut self.client),
        )
        .await
        .is_ok()
        {}
    }

    /// Drain a bounded amount of asynchronous output. Some parser lanes
    /// can continuously publish while a test is intentionally not asserting
    /// every advisory frame.
    async fn drain_bounded(&mut self) {
        for _ in 0..32 {
            if timeout(
                Duration::from_millis(10),
                self.codec.read_server_message(&mut self.client),
            )
            .await
            .is_err()
            {
                break;
            }
        }
    }

    /// Read until the response frame arrives, skipping asynchronous parse
    /// and activation output that can race a request/response exchange.
    async fn receive_response(&mut self) -> ServerMessage {
        loop {
            let frame = self.receive().await;
            if matches!(
                frame,
                ServerMessage::Error { .. }
                    | ServerMessage::FileOperationFailed { .. }
                    | ServerMessage::DocumentSaved { .. }
                    | ServerMessage::DocumentReloaded { .. }
                    | ServerMessage::DocumentClosed { .. }
                    | ServerMessage::DocumentStatus { .. }
                    | ServerMessage::DocumentList { .. }
                    | ServerMessage::ResyncSnapshot { .. }
            ) {
                return frame;
            }
        }
    }
}

/// Read until a transient-menu frame arrives, skipping parse/activation
/// noise that can race a menu exchange.
async fn receive_menu_message(connection: &mut TestConnection) -> ServerMessage {
    loop {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(
                message @ (ServerMessage::TransientMenuSnapshot(_)
                | ServerMessage::TransientMenuClosed { .. }),
            ) => return message,
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting transient menu frame"),
        }
    }
}

async fn control_center_opens_filters_activates_and_cancels_scenario() {
    let root = temp_workspace("control-center");
    // Hermetic configuration root (Phase 24.5, task 8): without an
    // explicit root this test fell back to the real ~/.clay and
    // hung whenever that directory contains an init.js (reload evaluates
    // the live user config). The sentinel typography proves the hermetic
    // root — not ambient ~/.clay — is the generation source: an
    // ambient fallback would load the default 20px monospace, not 21px.
    let config_root = temp_workspace("control-center-config");
    fs::write(
        config_root.join("init.js"),
        "import { setTypography } from \"clay:theme\"; setTypography({ monospace: { families: [\"MartianMono Nerd Font\", \"monospace\"], size: 21 }, proportional: { families: [\"Noto Sans\", \"sans-serif\"], size: 17 }, ui: { families: [\"system-ui\"], size: 13 } });",
    )
    .unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "control-center-open",
    ));
    config.workspace_roots.push(root.clone());
    config.configuration_root = Some(config_root);
    let server = super::super::IpcServer::new(config);
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    let open = |behavior_version| ClientMessage::CommandIntent {
        client_id: 11,
        document_id: 1,
        behavior_version,
        command_id: "controlCenter.open".to_string(),
    };
    let mut behavior_version = server.behavior.lock().await.version();

    // Opening replaces any active session: the first open delivers a
    // snapshot, a second open closes the old session and returns a
    // distinct new session id.
    connection.send(&open(behavior_version)).await;
    let ServerMessage::TransientMenuSnapshot(first_snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected first TransientMenuSnapshot");
    };
    let first_session_id = first_snapshot.session_id;
    connection.send(&open(behavior_version)).await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id }
            if session_id == first_session_id
    ));
    let ServerMessage::TransientMenuSnapshot(second_snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected replacement TransientMenuSnapshot");
    };
    let second_session_id = second_snapshot.session_id;
    assert_ne!(first_session_id, second_session_id);
    // Plan 124 task 7: the catalogue session is the composer's `/` palette — the
    // client anchors it to the lane's composer box (bottom anchor), names it
    // "Commands", and opens it unfiltered (the field below is the query).
    assert_eq!(
        second_snapshot.origin,
        crate::protocol::TransientMenuOriginData::CommandPalette,
        "the palette must declare the bottom/composer anchor, not a window sheet"
    );
    assert_eq!(second_snapshot.prompt, "Commands");
    assert!(!second_snapshot.items.is_empty());
    let opened_item_count = second_snapshot.items.len();

    // A stale selection move against the replaced session is a bounded
    // diagnostic, never an error or disconnect.
    connection
        .send(&ClientMessage::MenuSelectionMove {
            client_id: 11,
            session_id: first_session_id,
            delta: 1,
        })
        .await;
    assert!(matches!(
        connection.receive().await,
        ServerMessage::RuntimeDiagnostic(ref diagnostic)
            if diagnostic.code == "menu.unknown_session"
    ));

    // Query filtering narrows the live session's items.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: "reload".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered TransientMenuSnapshot");
    };
    assert!(
        !filtered.items.is_empty() && filtered.items.iter().all(|item| item.id.contains("reload")),
        "filtered items must all match the query: {:?}",
        filtered
            .items
            .iter()
            .map(|item| &item.id)
            .collect::<Vec<_>>()
    );
    // The palette rides one session across keystrokes: the filter update keeps
    // the same id and the same anchor, and clearing it restores the whole
    // open-time catalogue (the session holds it — no query rebuilds it).
    assert_eq!(filtered.session_id, second_session_id);
    assert_eq!(
        filtered.origin,
        crate::protocol::TransientMenuOriginData::CommandPalette
    );
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: String::new(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(restored) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected restored TransientMenuSnapshot");
    };
    assert_eq!(restored.session_id, second_session_id);
    assert_eq!(restored.items.len(), opened_item_count);

    // Plan 124: the palette's rows carry the server's own scope vocabulary and
    // their chords, so the sheet draws chips from data instead of guessing.
    let toggle_lane = restored
        .items
        .iter()
        .find(|item| item.id == "shell.toggleAgentLane")
        .expect("the lane toggle is a palette row");
    assert_eq!(toggle_lane.scope.as_deref(), Some("shell"));
    assert_eq!(toggle_lane.bindings, ["Ctrl+X Ctrl+P"]);

    // The scope chip rides the same update as the query: the *session* filters
    // by it, so the client never hides a row the server still selects over.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: String::new(),
            scope: Some("files".to_string()),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(scoped) = receive_menu_message(&mut connection).await
    else {
        panic!("expected scoped TransientMenuSnapshot");
    };
    assert_eq!(scoped.session_id, second_session_id);
    assert_eq!(
        scoped
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["controlCenter.openPath"],
        "the Files chip shows the palette's own path mode and nothing else"
    );
    assert_eq!(scoped.selected_index, 0);

    // The vocabulary is closed: an unknown word is not a scope (the chip falls
    // back to All), and the session survives it.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: String::new(),
            scope: Some("not-a-scope".to_string()),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(unscoped) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected unscoped TransientMenuSnapshot");
    };
    assert_eq!(unscoped.session_id, second_session_id);
    assert_eq!(unscoped.items.len(), opened_item_count);

    // Leave the palette filtered for the activation below.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: "reload".to_string(),
            scope: None,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuSnapshot(_)
    ));

    // Activating the selected item closes the menu and executes the
    // server command; the reload fanout (diagnostic + snapshot) arrives
    // asynchronously after the close.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: second_session_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id }
            if session_id == second_session_id
    ));
    let mut saw_reload_diagnostic = false;
    let mut saw_runtime_snapshot = false;
    for _ in 0..64 {
        match connection.receive().await {
            ServerMessage::RuntimeDiagnostic(ref diagnostic)
                if diagnostic.code == "runtime.reload_succeeded" =>
            {
                saw_reload_diagnostic = true;
            }
            ServerMessage::RuntimeStateSnapshot(snapshot) => {
                saw_runtime_snapshot = true;
                behavior_version = snapshot.behavior.behavior_version;
            }
            ServerMessage::BehaviorManifest(manifest) => {
                behavior_version = manifest.behavior_version;
            }
            _ => {}
        }
        if saw_reload_diagnostic && saw_runtime_snapshot {
            break;
        }
    }
    assert!(
        saw_reload_diagnostic && saw_runtime_snapshot,
        "reload fanout must deliver the diagnostic and snapshot"
    );
    // The hermetic root (not ambient ~/.clay) was the reload
    // source: the sentinel typography from its init.js is now live.
    assert!(
        (server
            .runtime_generation
            .active_typography()
            .await
            .monospace
            .size
            - 21.0)
            .abs()
            < f32::EPSILON,
        "reloaded generation must come from the hermetic config root"
    );

    // Reopening after the generation replacement yields a fresh session
    // id (the old generation's session is gone).
    connection.send(&open(behavior_version)).await;
    let ServerMessage::TransientMenuSnapshot(third_snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected reopened TransientMenuSnapshot");
    };
    assert_ne!(third_snapshot.session_id, second_session_id);

    // Escape (MenuCancel) closes the active session with a close frame.
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: third_snapshot.session_id,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id }
            if session_id == third_snapshot.session_id
    ));

    // The connection is still alive and functional.
    connection
        .send(&ClientMessage::ListDocuments { client_id: 11 })
        .await;
    assert!(matches!(
        connection.receive_response().await,
        ServerMessage::DocumentList { .. }
    ));
    connection.drain_bounded().await;
    connection.close().await;
}

async fn runtime_generation_replacement_cancels_open_control_center_scenario() {
    // Hermetic configuration root (Phase 24.5, task 8): same real-config
    // fallback hazard as control_center_opens_filters_activates_and_cancels.
    // Sentinel typography proves the hermetic root is the generation
    // source (ambient ~/.clay must never load).
    let config_root = temp_workspace("control-center-generation-config");
    fs::write(
        config_root.join("init.js"),
        "import { setTypography } from \"clay:theme\"; setTypography({ monospace: { families: [\"MartianMono Nerd Font\", \"monospace\"], size: 21 }, proportional: { families: [\"Noto Sans\", \"sans-serif\"], size: 17 }, ui: { families: [\"system-ui\"], size: 13 } });",
    )
    .unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "control-center-generation",
    ));
    config.configuration_root = Some(config_root);
    let server = super::super::IpcServer::new(config);
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    let mut behavior_version = server.behavior.lock().await.version();
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

    // A direct reload while the menu is open replaces the runtime
    // generation; the broadcast cancels the open menu session before
    // replaying the replacement state.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "runtime.reloadConfiguration".to_string(),
        })
        .await;
    let mut saw_reload_diagnostic = false;
    let mut saw_menu_close = false;
    let mut saw_runtime_snapshot = false;
    for _ in 0..64 {
        match connection.receive().await {
            ServerMessage::RuntimeDiagnostic(ref diagnostic)
                if diagnostic.code == "runtime.reload_succeeded" =>
            {
                saw_reload_diagnostic = true;
            }
            ServerMessage::TransientMenuClosed { session_id: closed } if closed == session_id => {
                saw_menu_close = true;
            }
            ServerMessage::RuntimeStateSnapshot(snapshot) => {
                saw_runtime_snapshot = true;
                behavior_version = snapshot.behavior.behavior_version;
            }
            ServerMessage::BehaviorManifest(manifest) => {
                behavior_version = manifest.behavior_version;
            }
            _ => {}
        }
        if saw_reload_diagnostic && saw_menu_close && saw_runtime_snapshot {
            break;
        }
    }
    assert!(
        saw_reload_diagnostic && saw_menu_close && saw_runtime_snapshot,
        "generation replacement must close the open menu and replay state"
    );
    // The hermetic root (not ambient ~/.clay) was the reload
    // source: the sentinel typography from its init.js is now live.
    assert!(
        (server
            .runtime_generation
            .active_typography()
            .await
            .monospace
            .size
            - 21.0)
            .abs()
            < f32::EPSILON,
        "replaced generation must come from the hermetic config root"
    );

    // The reopened menu is stamped with the replacement generation and
    // gets a distinct session id.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(reopened) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected reopened TransientMenuSnapshot");
    };
    assert_ne!(reopened.session_id, session_id);

    // No pending session survives the replacement: a stale selection
    // against the cancelled session id is a bounded diagnostic, never a
    // reply from a live session or a hang.
    connection
        .send(&ClientMessage::MenuSelectionMove {
            client_id: 11,
            session_id,
            delta: 1,
        })
        .await;
    assert!(matches!(
        connection.receive().await,
        ServerMessage::RuntimeDiagnostic(ref diagnostic)
            if diagnostic.code == "menu.unknown_session"
    ));
    connection.drain_bounded().await;
    connection.close().await;
}

/// Read until a `TabRegistry` snapshot arrives (skipping unrelated
/// frames that can race the tab-command exchange).
async fn receive_tab_registry_snapshot(
    connection: &mut TestConnection,
) -> crate::protocol::TabRegistrySnapshot {
    loop {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TabRegistry(snapshot)) => return snapshot,
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting TabRegistry snapshot"),
        }
    }
}

/// A shared registry seeded with two tabs: tab 1 bound to client 99 (the
/// test connection's identity) and tab 2 bound to a foreign client (7).
fn two_tab_registry() -> (
    Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
    tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
) {
    let mut registry = crate::server::tab_registry::TabRegistry::new();
    registry.create_tab(99, 1, "/workspaces/alpha".to_string());
    registry.create_tab(7, 2, "/workspaces/beta".to_string());
    let registry = Arc::new(Mutex::new(registry));
    let (tab_registry_tx, _) = tokio::sync::broadcast::channel(16);
    (registry, tab_registry_tx)
}

/// Open `path` and return the granted document id, skipping open-time
/// follow-up noise (behavior manifest, diagnostics, SDUI snapshot).
async fn open_document_until_opened(
    connection: &mut TestConnection,
    client_id: u64,
    root_id: crate::protocol::WorkspaceRootId,
    path: &str,
) -> crate::protocol::DocumentId {
    connection
        .send(&ClientMessage::OpenDocument {
            client_id,
            workspace_root_id: root_id,
            path: path.to_string(),
        })
        .await;
    loop {
        match connection.receive().await {
            ServerMessage::DocumentOpened { metadata, .. } => return metadata.document_id,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during open: {other:?}"),
        }
    }
}

/// Plan 129 task 3: crafted state for driving extracted handlers directly,
/// without a socket, a server, or the connection loop. Handlers take only the
/// context, so each one is independently callable from a test.
struct DirectHandlerState {
    codec: Codec,
    client_id: u64,
    document: Arc<Mutex<DocumentState>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: super::RuntimeGenerationStore,
    sdui: Arc<Mutex<StaticSduiState>>,
    parse_coordinator: ParseCoordinator,
    completion: crate::server::completion::CompletionCoordinator,
    language_intelligence: LanguageIntelligenceCoordinator,
    document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
    menu_sessions: crate::server::menu_sessions::ServerMenuSessions,
    bound_tab_id: Option<crate::protocol::TabId>,
    bound_state: Arc<std::sync::Mutex<Option<super::TabServerState>>>,
    tab_registry: Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
    tab_registry_tx: tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
    _tab_registry_rx: tokio::sync::broadcast::Receiver<crate::protocol::TabRegistrySnapshot>,
    pending_viewport_patches: std::collections::HashMap<
        (
            crate::protocol::DocumentId,
            crate::protocol::ViewportRequestId,
        ),
        super::PendingViewportPatch,
    >,
    completion_tx: tokio::sync::mpsc::Sender<ServerMessage>,
    _completion_rx: tokio::sync::mpsc::Receiver<ServerMessage>,
    language_intelligence_tx: tokio::sync::mpsc::Sender<ServerMessage>,
    _language_intelligence_rx: tokio::sync::mpsc::Receiver<ServerMessage>,
    dropped_results: Arc<std::sync::atomic::AtomicU64>,
    file_open_capabilities: super::FileOpenCapabilityPool,
}

impl DirectHandlerState {
    fn new() -> Self {
        let (tab_registry_tx, _tab_registry_rx) = tokio::sync::broadcast::channel(8);
        let (completion_tx, _completion_rx) =
            tokio::sync::mpsc::channel(crate::perf::budgets::CONNECTION_RESULT_LANE_CAPACITY);
        let (language_intelligence_tx, _language_intelligence_rx) =
            tokio::sync::mpsc::channel(crate::perf::budgets::CONNECTION_RESULT_LANE_CAPACITY);
        Self {
            codec: Codec::default(),
            client_id: 1,
            document: document_state(),
            workspace: workspace_state(),
            behavior: Arc::new(Mutex::new(
                ActiveBehaviorManifest::new(BehaviorManifest::minimal_text_editing(1))
                    .expect("minimal text editing manifest is valid"),
            )),
            runtime_generation: runtime_generation(),
            sdui: sdui_state(),
            parse_coordinator: parse_coordinator(),
            completion: crate::server::completion::CompletionCoordinator::new(),
            language_intelligence: language_intelligence_coordinator(),
            document_analysis:
                crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
            menu_sessions: crate::server::menu_sessions::ServerMenuSessions::new(),
            bound_tab_id: None,
            bound_state: Arc::new(std::sync::Mutex::new(None)),
            tab_registry: Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new())),
            tab_registry_tx,
            _tab_registry_rx,
            pending_viewport_patches: std::collections::HashMap::new(),
            completion_tx,
            _completion_rx,
            language_intelligence_tx,
            _language_intelligence_rx,
            dropped_results: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            file_open_capabilities: super::FileOpenCapabilityPool::new(),
        }
    }

    fn ctx<'a>(
        &'a mut self,
        stream: &'a mut tokio::io::DuplexStream,
    ) -> super::ConnectionCtx<'a, tokio::io::DuplexStream> {
        super::ConnectionCtx {
            codec: self.codec,
            stream,
            client_id: self.client_id,
            document: &mut self.document,
            workspace: &mut self.workspace,
            behavior: &self.behavior,
            runtime_generation: &self.runtime_generation,
            sdui: &self.sdui,
            parse_coordinator: &self.parse_coordinator,
            completion: &self.completion,
            language_intelligence: &self.language_intelligence,
            document_analysis: &self.document_analysis,
            reload_server: None,
            file_open_capabilities: &mut self.file_open_capabilities,
            menu_sessions: &mut self.menu_sessions,
            bound_tab_id: &mut self.bound_tab_id,
            bound_state: &self.bound_state,
            tab_registry: &self.tab_registry,
            tab_registry_tx: &self.tab_registry_tx,
            pending_viewport_patches: &mut self.pending_viewport_patches,
            completion_tx: &self.completion_tx,
            language_intelligence_tx: &self.language_intelligence_tx,
            dropped_results: &self.dropped_results,
        }
    }
}
