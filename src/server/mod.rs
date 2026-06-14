mod behavior;
mod configuration;
mod connection;
pub mod decorations;
pub(crate) mod document;
#[allow(dead_code)]
mod js_runtime;
#[allow(dead_code)]
mod ops;
pub mod parse_coordinator;
mod sdui;
mod ui;
mod workspace;

use std::{
    error::Error,
    fmt, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::{fs, os::unix::fs::FileTypeExt, path::Path};

use tokio::{sync::Mutex, task::JoinSet};

#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

use crate::{
    ipc::IpcEndpoint,
    protocol::{RuntimeDiagnostic, codec::Codec},
};

use self::{
    behavior::ActiveBehaviorManifest, connection::handle_connection, document::DocumentState,
    js_runtime::ClayJsRuntimeService, parse_coordinator::ParseCoordinator, sdui::StaticSduiState,
    workspace::WorkspaceState,
};

#[cfg(windows)]
const ERROR_PIPE_CONNECTED: i32 = 535;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub endpoint: IpcEndpoint,
    pub workspace_roots: Vec<PathBuf>,
    pub configuration_root: Option<PathBuf>,
}

impl ServerConfig {
    pub fn new(endpoint: impl Into<IpcEndpoint>) -> Self {
        Self {
            endpoint: endpoint.into(),
            workspace_roots: Vec::new(),
            configuration_root: None,
        }
    }
}

#[derive(Debug)]
pub struct IpcServer {
    config: ServerConfig,
    codec: Codec,
    document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    sdui: Arc<Mutex<StaticSduiState>>,
    runtime_diagnostics: Arc<Mutex<Vec<RuntimeDiagnostic>>>,
    #[allow(dead_code)]
    parse_coordinator: ParseCoordinator,
    #[allow(dead_code)]
    js_runtime: ClayJsRuntimeService,
    next_client_id: AtomicU64,
}

impl IpcServer {
    pub fn new(config: ServerConfig) -> Self {
        Self::try_new(config).expect("configured workspace roots must be valid")
    }

    pub fn try_new(config: ServerConfig) -> Result<Self, ServerError> {
        let mut workspace = WorkspaceState::new();
        for root in &config.workspace_roots {
            workspace.add_root(root).map_err(|error| {
                ServerError::InvalidWorkspaceRoot(error.diagnostic().to_string())
            })?;
        }
        workspace.reserve_document_ids_from(2);

        Ok(Self {
            config,
            codec: Codec::default(),
            document: Arc::new(Mutex::new(DocumentState::default())),
            behavior: Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace: Arc::new(Mutex::new(workspace)),
            sdui: Arc::new(Mutex::new(StaticSduiState::for_document(1, 1))),
            runtime_diagnostics: Arc::new(Mutex::new(Vec::new())),
            parse_coordinator: ParseCoordinator::default(),
            js_runtime: ClayJsRuntimeService::default(),
            next_client_id: AtomicU64::new(1),
        })
    }

    #[cfg(unix)]
    pub async fn run(self) -> Result<(), ServerError> {
        let listener = bind_unix_listener(self.config.endpoint.as_unix_socket_path())?;
        self.load_default_configuration().await;
        self.accept_unix_loop(listener).await
    }

    #[cfg(unix)]
    async fn accept_unix_loop(self, listener: UnixListener) -> Result<(), ServerError> {
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let (stream, _address) = accepted.map_err(ServerError::Accept)?;
                    self.spawn_connection(stream, &mut connections);
                }
                Some(joined) = connections.join_next() => {
                    if let Err(error) = joined {
                        eprintln!("clay server connection task failed: {error}");
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    pub async fn run(self) -> Result<(), ServerError> {
        self.config
            .endpoint
            .validate_windows_named_pipe()
            .map_err(ServerError::InvalidEndpoint)?;
        self.load_default_configuration().await;
        let mut connections = JoinSet::new();
        loop {
            let pipe = create_named_pipe_server(self.config.endpoint.as_windows_named_pipe())?;
            tokio::select! {
                connected = connect_named_pipe_server(pipe) => {
                    let stream = connected.map_err(ServerError::Accept)?;
                    self.spawn_connection(stream, &mut connections);
                }
                Some(joined) = connections.join_next() => {
                    if let Err(error) = joined {
                        eprintln!("clay server connection task failed: {error}");
                    }
                }
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub async fn run(self) -> Result<(), ServerError> {
        Err(ServerError::InvalidEndpoint(format!(
            "Clay IPC is unsupported on this platform: {}",
            self.config.endpoint
        )))
    }

    async fn load_default_configuration(&self) {
        let evaluation = if let Some(config_root) = self.config.configuration_root.clone() {
            self.js_runtime
                .load_configuration_from_root_with_workspace(
                    config_root,
                    Arc::clone(&self.workspace),
                )
                .await
                .map(Some)
        } else {
            self.js_runtime
                .load_default_configuration_with_workspace(Arc::clone(&self.workspace))
                .await
        };

        match evaluation {
            Ok(Some(evaluation)) => self.apply_runtime_evaluation(evaluation).await,
            Ok(None) => {}
            Err(error) => {
                let diagnostic = error.diagnostic();
                eprintln!(
                    "clay server configuration failed [{}]: {}",
                    diagnostic.code, diagnostic.message
                );
                self.runtime_diagnostics.lock().await.push(diagnostic);
            }
        }
    }

    async fn apply_runtime_evaluation(&self, evaluation: js_runtime::ClayRuntimeEvaluation) {
        if let Some(tree) = evaluation.published_sdui_tree {
            if let Err(error) = self.sdui.lock().await.replace_with_runtime_tree(tree) {
                let diagnostic = RuntimeDiagnostic::error(
                    "clay.sdui.invalid_tree",
                    "Published SDUI tree failed server validation.",
                );
                eprintln!(
                    "clay server rejected runtime SDUI tree [{}]: {} ({error:?})",
                    diagnostic.code, diagnostic.message
                );
                self.runtime_diagnostics.lock().await.push(diagnostic);
            }
        }
        if let Some(manifest) = evaluation.behavior_manifest {
            if let Err(error) = self.behavior.lock().await.publish_replacement(manifest) {
                let diagnostic = RuntimeDiagnostic::error(
                    "clay.behavior.invalid_manifest",
                    "Runtime behavior manifest failed server validation.",
                );
                eprintln!(
                    "clay server rejected runtime behavior manifest [{}]: {} ({error:?})",
                    diagnostic.code, diagnostic.message
                );
                self.runtime_diagnostics.lock().await.push(diagnostic);
            }
        }
    }

    fn spawn_connection<S>(&self, stream: S, connections: &mut JoinSet<()>)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let client_id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
        let document = Arc::clone(&self.document);
        let behavior = Arc::clone(&self.behavior);
        let workspace = Arc::clone(&self.workspace);
        let sdui = Arc::clone(&self.sdui);
        let runtime_diagnostics = Arc::clone(&self.runtime_diagnostics);
        let codec = self.codec;
        connections.spawn(async move {
            if let Err(error) = handle_connection(
                stream,
                client_id,
                document,
                behavior,
                workspace,
                sdui,
                runtime_diagnostics,
                codec,
            )
            .await
            {
                eprintln!("clay server connection {client_id} closed with error: {error}");
            }
        });
    }
}

#[cfg(unix)]
fn bind_unix_listener(socket_path: &Path) -> Result<UnixListener, ServerError> {
    validate_socket_path(socket_path)?;
    remove_stale_socket(socket_path)?;
    UnixListener::bind(socket_path).map_err(ServerError::Bind)
}

#[cfg(unix)]
fn validate_socket_path(socket_path: &Path) -> Result<(), ServerError> {
    if socket_path.as_os_str().is_empty() {
        return Err(ServerError::InvalidEndpoint(
            "socket path must not be empty".to_string(),
        ));
    }

    let Some(parent) = socket_path.parent() else {
        return Err(ServerError::InvalidEndpoint(
            "socket path must have a parent directory".to_string(),
        ));
    };
    let metadata = fs::metadata(parent).map_err(ServerError::EndpointDirectory)?;
    if !metadata.is_dir() {
        return Err(ServerError::InvalidEndpoint(format!(
            "socket parent {} is not a directory",
            parent.display()
        )));
    }

    Ok(())
}

#[cfg(unix)]
fn remove_stale_socket(socket_path: &Path) -> Result<(), ServerError> {
    let Ok(metadata) = fs::symlink_metadata(socket_path) else {
        return Ok(());
    };

    if metadata.file_type().is_socket() {
        fs::remove_file(socket_path).map_err(ServerError::RemoveStaleSocket)?;
        return Ok(());
    }

    Err(ServerError::InvalidEndpoint(format!(
        "refusing to replace non-socket path {}",
        socket_path.display()
    )))
}

#[cfg(windows)]
fn create_named_pipe_server(pipe_name: &str) -> Result<NamedPipeServer, ServerError> {
    ServerOptions::new()
        .create(pipe_name)
        .map_err(ServerError::Bind)
}

#[cfg(windows)]
async fn connect_named_pipe_server(pipe: NamedPipeServer) -> io::Result<NamedPipeServer> {
    match pipe.connect().await {
        Ok(()) => Ok(pipe),
        Err(error) if error.raw_os_error() == Some(ERROR_PIPE_CONNECTED) => Ok(pipe),
        Err(error) => Err(error),
    }
}

#[derive(Debug)]
pub enum ServerError {
    InvalidEndpoint(String),
    EndpointDirectory(io::Error),
    RemoveStaleSocket(io::Error),
    Bind(io::Error),
    Accept(io::Error),
    InvalidWorkspaceRoot(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint(message) => write!(formatter, "invalid IPC endpoint: {message}"),
            Self::EndpointDirectory(error) => {
                write!(
                    formatter,
                    "failed to inspect IPC endpoint directory: {error}"
                )
            }
            Self::RemoveStaleSocket(error) => {
                write!(formatter, "failed to remove stale socket: {error}")
            }
            Self::Bind(error) => write!(formatter, "failed to bind IPC endpoint: {error}"),
            Self::Accept(error) => write!(formatter, "failed to accept IPC connection: {error}"),
            Self::InvalidWorkspaceRoot(message) => {
                write!(formatter, "invalid workspace root: {message}")
            }
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EndpointDirectory(error)
            | Self::RemoveStaleSocket(error)
            | Self::Bind(error)
            | Self::Accept(error) => Some(error),
            Self::InvalidEndpoint(_) | Self::InvalidWorkspaceRoot(_) => None,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        sync::{Arc, atomic::AtomicU64},
        time::SystemTime,
    };

    use tokio::{net::UnixStream, sync::Mutex};

    use super::{ActiveBehaviorManifest, ClayJsRuntimeService, IpcServer, ServerConfig};
    use crate::{
        protocol::{
            ClientMessage, DocumentAccess, EditOperation, EditRejection, LockOwner,
            PROTOCOL_VERSION, ServerMessage, codec::Codec,
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
        IpcServer {
            config: ServerConfig::new(socket_path),
            codec: Codec::default(),
            document: Arc::new(Mutex::new(document)),
            behavior: Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace: {
                let mut workspace = crate::server::workspace::WorkspaceState::new();
                workspace.reserve_document_ids_from(2);
                Arc::new(Mutex::new(workspace))
            },
            sdui: Arc::new(Mutex::new(StaticSduiState::for_document(1, 1))),
            runtime_diagnostics: Arc::new(Mutex::new(Vec::new())),
            parse_coordinator: ParseCoordinator::default(),
            js_runtime: ClayJsRuntimeService::default(),
            next_client_id: AtomicU64::new(1),
        }
    }

    #[tokio::test]
    async fn real_server_end_to_end_region_locked_edit_rejected() {
        let socket_path = unique_socket_path("region-lock");
        let mut document = DocumentState::default();
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
        let (document_id, version, lease_id) =
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::InitialDocument {
                    document_id,
                    version,
                    access: DocumentAccess::Editable { lease_id },
                    lease_id: Some(snapshot_lease_id),
                    ..
                } => {
                    assert_eq!(lease_id, snapshot_lease_id);
                    (document_id, version, lease_id)
                }
                message => panic!("expected editable InitialDocument, got {message:?}"),
            };
        let _manifest = codec.read_server_message(&mut stream).await.unwrap();
        let _sdui = codec.read_server_message(&mut stream).await.unwrap();

        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Edit {
                    document_id,
                    client_id,
                    lease_id: Some(lease_id),
                    base_version: version,
                    behavior_version: 1,
                    transaction_id: 12,
                    operation: EditOperation::Insert {
                        byte_offset: 1,
                        text: "x".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::EditRejected {
                document_id: rejected_document_id,
                transaction_id: 12,
                reason: EditRejection::RegionLocked { conflict },
            } if rejected_document_id == document_id
                && conflict.lock_id == lock_id
                && conflict.start == 0
                && conflict.end == 7
                && conflict.owner == LockOwner::Server
                && conflict.created_at_version == version
        ));

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
    fn ordinary_typing_does_not_enter_js_runtime() {
        let socket_path = unique_socket_path("runtime-not-hot-path");
        let server = IpcServer::new(ServerConfig::new(&socket_path));

        assert_eq!(server.js_runtime.evaluation_count(), 0);

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

        assert!(matches!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::Welcome { .. }
        ));
        assert!(matches!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::InitialDocument {
                access: DocumentAccess::Editable { lease_id: 1 },
                ..
            }
        ));

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
}
