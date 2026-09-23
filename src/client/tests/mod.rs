//! Stay-in-crate unit tests for the IPC client: edit queue and acks, events,
//! viewport/framing, and real-server end-to-end flows over Unix sockets and
//! Windows named pipes.
#[cfg(unix)]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(any(unix, windows))]
use std::time::SystemTime;

use tokio::io::duplex;
#[cfg(unix)]
use tokio::net::UnixStream;

use super::EditorEditEvent;
#[cfg(windows)]
use super::connect_transport;
use super::{
    ClientConnectionEvent, ClientEditQueue, connect_for_reclaim as connect_for_reclaim_endpoint,
    connect_for_reclaim_or_new, connect_from_stream,
    connect_with_workspace_root as connect_with_workspace_root_endpoint,
    load_initial_state_from_stream,
};
#[cfg(any(unix, windows))]
use super::{ClientSession, connect};
#[cfg(any(unix, windows))]
use crate::ipc::IpcEndpoint;
#[cfg(any(unix, windows))]
use crate::protocol::EditRejection;
use crate::protocol::{
    ActiveTypography, BehaviorManifest, ClientMessage, CommandDeclaration, DocumentAccess,
    DocumentTextHead, EditOperation, FileErrorCode, PROTOCOL_VERSION, RuntimeDiagnostic,
    SduiActionIntent, SduiActionSource, SduiEditorBinding, SduiNode, SduiNodeId, SduiNodeKind,
    SduiTree, ServerMessage, codec::Codec,
};
#[cfg(any(unix, windows))]
use crate::server::{IpcServer, ServerConfig};

#[cfg(unix)]
fn unique_socket_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "clay-client-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    dir.join("clay.sock")
}

#[cfg(unix)]
async fn connect_with_retry(socket_path: &std::path::Path) -> ClientSession {
    let mut last_error = None;
    for _ in 0..50 {
        match connect(&IpcEndpoint::from(socket_path)).await {
            Ok(session) => return session,
            Err(error) => {
                last_error = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
    panic!("failed to connect to test socket: {:?}", last_error);
}

#[cfg(unix)]
async fn connect_stream_with_retry(socket_path: &std::path::Path) -> UnixStream {
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

#[cfg(unix)]
async fn connect_with_workspace_root(
    socket_path: &Path,
    workspace_root: String,
) -> Result<ClientSession, super::ClientBootstrapError> {
    connect_with_workspace_root_endpoint(&IpcEndpoint::from(socket_path), workspace_root).await
}

#[cfg(unix)]
async fn connect_for_reclaim(
    socket_path: &Path,
    tab_id: crate::protocol::TabId,
) -> Result<ClientSession, super::ClientBootstrapError> {
    connect_for_reclaim_endpoint(&IpcEndpoint::from(socket_path), tab_id).await
}

// Test suites (see each file for its scope).
mod client_events;
mod edit_queue_and_resync;
mod end_to_end_handshake;
mod intents_and_acks;
mod real_server_end_to_end;
mod real_server_tabs;
mod viewport_and_framing;
#[cfg(windows)]
mod windows_named_pipe;
#[cfg(windows)]
fn unique_named_pipe(name: &str) -> IpcEndpoint {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    IpcEndpoint::WindowsNamedPipe(format!(
        r"\\.\pipe\clay-client-{name}-{}-{unique}",
        std::process::id()
    ))
}

#[cfg(windows)]
async fn connect_with_retry(endpoint: &IpcEndpoint) -> ClientSession {
    let mut last_error = None;
    for _ in 0..50 {
        match connect(endpoint).await {
            Ok(session) => return session,
            Err(error) => {
                last_error = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
    panic!("failed to connect to test named pipe: {:?}", last_error);
}

fn sample_runtime_snapshot(
    generation: u64,
    client_id: u64,
) -> crate::protocol::RuntimeStateSnapshot {
    let snapshot = crate::protocol::RuntimeStateSnapshot {
        runtime_generation_id: generation,
        client_id,
        behavior: BehaviorManifest::minimal_text_editing(generation),
        active_theme: crate::protocol::ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: ActiveTypography::default(),
        active_design_system: crate::shell::design_system::ActiveDesignSystem::core_fallback(
            generation,
        ),
        active_icon_pack: None,
        ui_choices: crate::protocol::UiChoicesSnapshot::default(),
        sdui_tree: SduiTree {
            ui_version: generation,
            root_id: SduiNodeId(1),
            nodes: vec![SduiNode::new(
                SduiNodeId(1),
                SduiNodeKind::Label {
                    text: format!("gen-{generation}"),
                    icon: None,
                },
            )],
        },
        package_ui: crate::protocol::PackageUiSnapshot {
            version: generation,
            ..Default::default()
        },
        documents: Vec::new(),
        diagnostics: Vec::new(),
    };
    snapshot.validate().expect("fixture");
    snapshot
}

async fn write_minimal_bootstrap(
    codec: &Codec,
    server: &mut tokio::io::DuplexStream,
    client_id: u64,
) {
    let _hello = codec.read_client_message(server).await.unwrap();
    codec
        .write_server_message(
            server,
            &ServerMessage::Welcome {
                client_id,
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await
        .unwrap();
    codec
        .write_server_message(
            server,
            &ServerMessage::InitialDocument {
                document_id: 2,
                version: 3,
                head: DocumentTextHead::complete(String::new()),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
                workspace_root: String::new(),
            },
        )
        .await
        .unwrap();
    codec
        .write_server_message(
            server,
            &ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(1))),
        )
        .await
        .unwrap();
    codec
        .write_server_message(
            server,
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
            server,
            &ServerMessage::ActiveTypography(ActiveTypography::default()),
        )
        .await
        .unwrap();
}
