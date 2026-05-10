mod connection;
mod document;

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

use self::{connection::handle_connection, document::DocumentState};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub socket_path: PathBuf,
}

impl ServerConfig {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }
}

#[derive(Debug)]
pub struct IpcServer {
    config: ServerConfig,
    codec: Codec,
    document: Arc<Mutex<DocumentState>>,
    next_client_id: AtomicU64,
}

impl IpcServer {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            codec: Codec::default(),
            document: Arc::new(Mutex::new(DocumentState::default())),
            next_client_id: AtomicU64::new(1),
        }
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
                    let codec = self.codec;
                    connections.spawn(async move {
                        if let Err(error) = handle_connection(stream, client_id, document, codec).await {
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
            Self::InvalidSocketPath(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use tokio::net::UnixStream;

    use super::{IpcServer, ServerConfig};
    use crate::protocol::{
        ClientMessage, DocumentAccess, PROTOCOL_VERSION, ServerMessage, codec::Codec,
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
                access: DocumentAccess::Editable,
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
