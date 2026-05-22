mod behavior;
mod connection;
mod document;
mod workspace;

use std::{
    error::Error,
    fmt, fs, io,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use tokio::{net::UnixListener, sync::Mutex, task::JoinSet};

use crate::protocol::codec::Codec;

use self::{
    behavior::ActiveBehaviorManifest, connection::handle_connection, document::DocumentState,
    workspace::WorkspaceState,
};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub socket_path: PathBuf,
    pub workspace_roots: Vec<PathBuf>,
}

impl ServerConfig {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            workspace_roots: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct IpcServer {
    config: ServerConfig,
    codec: Codec,
    document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    #[expect(
        dead_code,
        reason = "Phase 9 workspace state is validated at startup before protocol dispatch integration"
    )]
    workspace: Arc<Mutex<WorkspaceState>>,
    next_client_id: AtomicU64,
}

impl IpcServer {
    pub fn new(config: ServerConfig) -> Self {
        Self::try_new(config).expect("configured workspace roots must be valid")
    }

    pub fn try_new(config: ServerConfig) -> Result<Self, ServerError> {
        let mut workspace = WorkspaceState::new();
        for root in &config.workspace_roots {
            workspace
                .add_root(root)
                .map_err(|error| ServerError::InvalidWorkspaceRoot(error.to_string()))?;
        }

        Ok(Self {
            config,
            codec: Codec::default(),
            document: Arc::new(Mutex::new(DocumentState::default())),
            behavior: Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace: Arc::new(Mutex::new(workspace)),
            next_client_id: AtomicU64::new(1),
        })
    }

    pub async fn run(self) -> Result<(), ServerError> {
        let listener = bind_unix_listener(&self.config.socket_path)?;
        self.accept_loop(listener).await
    }

    async fn accept_loop(self, listener: UnixListener) -> Result<(), ServerError> {
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let (stream, _address) = accepted.map_err(ServerError::Accept)?;
                    let client_id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
                    let document = Arc::clone(&self.document);
                    let behavior = Arc::clone(&self.behavior);
                    let codec = self.codec;
                    connections.spawn(async move {
                        if let Err(error) = handle_connection(stream, client_id, document, behavior, codec).await {
                            eprintln!("clay server connection {client_id} closed with error: {error}");
                        }
                    });
                }
                Some(joined) = connections.join_next() => {
                    if let Err(error) = joined {
                        eprintln!("clay server connection task failed: {error}");
                    }
                }
            }
        }
    }
}

fn bind_unix_listener(socket_path: &Path) -> Result<UnixListener, ServerError> {
    validate_socket_path(socket_path)?;
    remove_stale_socket(socket_path)?;
    UnixListener::bind(socket_path).map_err(ServerError::Bind)
}

fn validate_socket_path(socket_path: &Path) -> Result<(), ServerError> {
    if socket_path.as_os_str().is_empty() {
        return Err(ServerError::InvalidSocketPath(
            "socket path must not be empty".to_string(),
        ));
    }

    let Some(parent) = socket_path.parent() else {
        return Err(ServerError::InvalidSocketPath(
            "socket path must have a parent directory".to_string(),
        ));
    };
    let metadata = fs::metadata(parent).map_err(ServerError::SocketDirectory)?;
    if !metadata.is_dir() {
        return Err(ServerError::InvalidSocketPath(format!(
            "socket parent {} is not a directory",
            parent.display()
        )));
    }

    Ok(())
}

fn remove_stale_socket(socket_path: &Path) -> Result<(), ServerError> {
    let Ok(metadata) = fs::symlink_metadata(socket_path) else {
        return Ok(());
    };

    if metadata.file_type().is_socket() {
        fs::remove_file(socket_path).map_err(ServerError::RemoveStaleSocket)?;
        return Ok(());
    }

    Err(ServerError::InvalidSocketPath(format!(
        "refusing to replace non-socket path {}",
        socket_path.display()
    )))
}

#[derive(Debug)]
pub enum ServerError {
    InvalidSocketPath(String),
    SocketDirectory(io::Error),
    RemoveStaleSocket(io::Error),
    Bind(io::Error),
    Accept(io::Error),
    InvalidWorkspaceRoot(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSocketPath(message) => write!(formatter, "invalid socket path: {message}"),
            Self::SocketDirectory(error) => {
                write!(formatter, "failed to inspect socket directory: {error}")
            }
            Self::RemoveStaleSocket(error) => {
                write!(formatter, "failed to remove stale socket: {error}")
            }
            Self::Bind(error) => write!(formatter, "failed to bind Unix socket: {error}"),
            Self::Accept(error) => write!(
                formatter,
                "failed to accept Unix socket connection: {error}"
            ),
            Self::InvalidWorkspaceRoot(message) => {
                write!(formatter, "invalid workspace root: {message}")
            }
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SocketDirectory(error)
            | Self::RemoveStaleSocket(error)
            | Self::Bind(error)
            | Self::Accept(error) => Some(error),
            Self::InvalidSocketPath(_) | Self::InvalidWorkspaceRoot(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{Arc, atomic::AtomicU64},
        time::SystemTime,
    };

    use tokio::{net::UnixStream, sync::Mutex};

    use super::{ActiveBehaviorManifest, IpcServer, ServerConfig};
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
            workspace: Arc::new(Mutex::new(crate::server::workspace::WorkspaceState::new())),
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
