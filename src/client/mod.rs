pub(crate) mod behavior;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::editor::EditorEditEvent;
use crate::ipc::IpcEndpoint;
use crate::protocol::{
    BehaviorManifest, BehaviorVersion, ClientId, ClientMessage, DocumentAccess, DocumentId,
    DocumentVersion, EditOperation, EditRejection, PROTOCOL_VERSION, ProtocolErrorCode,
    ServerMessage, TransactionId,
    codec::{Codec, CodecError},
};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc,
    time::{Duration, timeout},
};

#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;

const CLIENT_NAME: &str = "clay-client";
const EDIT_QUEUE_CAPACITY: usize = 256;
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(windows)]
const PIPE_BUSY_RETRY_DELAY: Duration = Duration::from_millis(20);
#[cfg(windows)]
const PIPE_BUSY_RETRY_ATTEMPTS: usize = 50;
#[cfg(windows)]
const ERROR_PIPE_BUSY: i32 = 231;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInitialState {
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub text: String,
    pub access: DocumentAccess,
    pub behavior_manifest: BehaviorManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEdit {
    pub document_id: DocumentId,
    pub base_version: DocumentVersion,
    pub transaction_id: TransactionId,
    pub operation: EditOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientResyncSnapshot {
    pub document_id: DocumentId,
    pub version: DocumentVersion,
    pub text: String,
    pub access: DocumentAccess,
    pub lease_id: Option<crate::protocol::LeaseId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSyncSnapshot {
    pub confirmed_version: DocumentVersion,
    pub optimistic_version: DocumentVersion,
    pub pending: Vec<PendingEdit>,
    pub last_resync: Option<ClientResyncSnapshot>,
}

#[derive(Debug)]
struct ClientSyncState {
    confirmed_version: DocumentVersion,
    optimistic_version: DocumentVersion,
    pending: VecDeque<PendingEdit>,
    last_resync: Option<ClientResyncSnapshot>,
}

impl ClientSyncState {
    fn new(confirmed_version: DocumentVersion) -> Self {
        Self {
            confirmed_version,
            optimistic_version: confirmed_version,
            pending: VecDeque::new(),
            last_resync: None,
        }
    }

    fn reserve_pending(
        &mut self,
        document_id: DocumentId,
        transaction_id: TransactionId,
        operation: EditOperation,
    ) -> DocumentVersion {
        let base_version = self.optimistic_version;
        self.optimistic_version = self.optimistic_version.saturating_add(1);
        self.pending.push_back(PendingEdit {
            document_id,
            base_version,
            transaction_id,
            operation,
        });
        base_version
    }

    fn rollback_pending_reservation(&mut self, transaction_id: TransactionId) {
        if let Some(position) = self
            .pending
            .iter()
            .position(|pending| pending.transaction_id == transaction_id)
        {
            self.pending.remove(position);
            self.optimistic_version = self
                .pending
                .back()
                .map_or(self.confirmed_version, |pending| pending.base_version + 1);
        }
    }

    fn acknowledge(&mut self, confirmed_version: DocumentVersion, transaction_id: TransactionId) {
        self.confirmed_version = confirmed_version;
        if let Some(position) = self
            .pending
            .iter()
            .position(|pending| pending.transaction_id == transaction_id)
        {
            self.pending.remove(position);
        }
        if self.optimistic_version < confirmed_version {
            self.optimistic_version = confirmed_version;
        }
    }

    fn reject(&mut self, transaction_id: TransactionId) {
        if let Some(position) = self
            .pending
            .iter()
            .position(|pending| pending.transaction_id == transaction_id)
        {
            self.pending.remove(position);
        }
        self.optimistic_version = self
            .pending
            .back()
            .map_or(self.confirmed_version, |pending| pending.base_version + 1);
    }

    fn apply_resync_snapshot(&mut self, snapshot: ClientResyncSnapshot) {
        self.confirmed_version = snapshot.version;
        self.optimistic_version = snapshot.version;
        self.pending.clear();
        self.last_resync = Some(snapshot);
    }

    fn snapshot(&self) -> ClientSyncSnapshot {
        ClientSyncSnapshot {
            confirmed_version: self.confirmed_version,
            optimistic_version: self.optimistic_version,
            pending: self.pending.iter().cloned().collect(),
            last_resync: self.last_resync.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClientEditQueue {
    sender: mpsc::Sender<ClientMessage>,
    client_id: ClientId,
    lease_id: Option<crate::protocol::LeaseId>,
    sync_state: Arc<Mutex<ClientSyncState>>,
}

impl ClientEditQueue {
    pub fn bounded(capacity: usize) -> (Self, mpsc::Receiver<ClientMessage>) {
        let (sender, receiver) = mpsc::channel(capacity);
        (
            Self {
                sender,
                client_id: 0,
                lease_id: None,
                sync_state: Arc::new(Mutex::new(ClientSyncState::new(0))),
            },
            receiver,
        )
    }

    pub fn from_sender(sender: mpsc::Sender<ClientMessage>) -> Self {
        Self {
            sender,
            client_id: 0,
            lease_id: None,
            sync_state: Arc::new(Mutex::new(ClientSyncState::new(0))),
        }
    }

    pub fn enqueue_edit_event(
        &self,
        event: EditorEditEvent,
        transaction_id: TransactionId,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let operation = event.operation;
        let base_version = {
            let mut state = self.sync_state.lock().expect("client sync state poisoned");
            state.reserve_pending(event.document_id, transaction_id, operation.clone())
        };
        let message = ClientMessage::Edit {
            document_id: event.document_id,
            client_id: self.client_id,
            lease_id: self.lease_id,
            base_version,
            behavior_version: event.behavior_version,
            transaction_id,
            operation,
        };

        if self.lease_id.is_none() {
            let mut state = self.sync_state.lock().expect("client sync state poisoned");
            state.rollback_pending_reservation(transaction_id);
            return Err(mpsc::error::TrySendError::Closed(message));
        }

        if let Err(error) = self.sender.try_send(message) {
            let mut state = self.sync_state.lock().expect("client sync state poisoned");
            state.rollback_pending_reservation(transaction_id);
            return Err(error);
        }

        Ok(())
    }

    pub fn sync_snapshot(&self) -> ClientSyncSnapshot {
        self.sync_state
            .lock()
            .expect("client sync state poisoned")
            .snapshot()
    }

    fn with_authority(mut self, client_id: ClientId, access: &DocumentAccess) -> Self {
        self.client_id = client_id;
        self.lease_id = access.lease_id();
        self
    }

    fn with_confirmed_version(mut self, confirmed_version: DocumentVersion) -> Self {
        self.sync_state = Arc::new(Mutex::new(ClientSyncState::new(confirmed_version)));
        self
    }
}

#[derive(Debug)]
pub struct ClientSession {
    pub initial_state: ClientInitialState,
    pub edit_queue: ClientEditQueue,
    pub events: mpsc::Receiver<ClientConnectionEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientConnectionEvent {
    EditAck {
        document_id: DocumentId,
        version: DocumentVersion,
        transaction_id: TransactionId,
    },
    EditRejected {
        document_id: DocumentId,
        transaction_id: TransactionId,
        reason: EditRejection,
    },
    BehaviorManifestInstalled {
        behavior_version: BehaviorVersion,
        manifest: BehaviorManifest,
    },
    BehaviorManifestRejected {
        behavior_version: BehaviorVersion,
        reason: String,
    },
    ResyncSnapshot(ClientResyncSnapshot),
    EditTransaction(ServerMessage),
    ServerError {
        code: ProtocolErrorCode,
        message: String,
    },
    Disconnected,
    ConnectionError(String),
}

#[derive(Debug)]
pub enum ClientBootstrapError {
    Codec(CodecError),
    UnexpectedMessage(&'static str),
    ServerError {
        code: ProtocolErrorCode,
        message: String,
    },
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientBootstrapErrorKind {
    TransportUnavailable,
    EndpointInvalid,
    HandshakeFailed,
    ServerRejected,
    TimedOut,
    ProtocolInvalid,
}

impl ClientBootstrapError {
    pub fn kind(&self) -> ClientBootstrapErrorKind {
        match self {
            Self::Codec(CodecError::Io(error))
                if error.kind() == std::io::ErrorKind::InvalidInput =>
            {
                ClientBootstrapErrorKind::EndpointInvalid
            }
            Self::Codec(CodecError::Io(_)) => ClientBootstrapErrorKind::TransportUnavailable,
            Self::Codec(_) => ClientBootstrapErrorKind::ProtocolInvalid,
            Self::UnexpectedMessage(_) => ClientBootstrapErrorKind::HandshakeFailed,
            Self::ServerError { .. } => ClientBootstrapErrorKind::ServerRejected,
            Self::Timeout => ClientBootstrapErrorKind::TimedOut,
        }
    }
}

impl From<CodecError> for ClientBootstrapError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl std::fmt::Display for ClientBootstrapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(error) => write!(formatter, "client IPC codec failed: {error}"),
            Self::UnexpectedMessage(message) => {
                write!(formatter, "unexpected server message: {message}")
            }
            Self::ServerError { code, message } => {
                write!(formatter, "server returned {code:?}: {message}")
            }
            Self::Timeout => formatter.write_str("timed out waiting for server snapshot"),
        }
    }
}

impl std::error::Error for ClientBootstrapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            _ => None,
        }
    }
}

pub async fn connect(endpoint: &IpcEndpoint) -> Result<ClientSession, ClientBootstrapError> {
    let stream = connect_transport(endpoint).await.map_err(CodecError::Io)?;
    timeout(
        SNAPSHOT_TIMEOUT,
        connect_from_stream(stream, Codec::default()),
    )
    .await
    .map_err(|_| ClientBootstrapError::Timeout)?
}

pub async fn load_initial_state(
    endpoint: &IpcEndpoint,
) -> Result<ClientInitialState, ClientBootstrapError> {
    Ok(connect(endpoint).await?.initial_state)
}

#[cfg(unix)]
async fn connect_transport(endpoint: &IpcEndpoint) -> std::io::Result<UnixStream> {
    UnixStream::connect(endpoint.as_unix_socket_path()).await
}

#[cfg(windows)]
async fn connect_transport(
    endpoint: &IpcEndpoint,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    endpoint
        .validate_windows_named_pipe()
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let pipe_name = endpoint.as_windows_named_pipe();
    let mut last_busy = None;

    for _ in 0..PIPE_BUSY_RETRY_ATTEMPTS {
        match ClientOptions::new().open(pipe_name) {
            Ok(client) => return Ok(client),
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                last_busy = Some(error);
                tokio::time::sleep(PIPE_BUSY_RETRY_DELAY).await;
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_busy.expect("pipe-busy retry loop records the last busy error"))
}

#[cfg(not(any(unix, windows)))]
async fn connect_transport(_endpoint: &IpcEndpoint) -> std::io::Result<tokio::io::DuplexStream> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Clay IPC is unsupported on this platform",
    ))
}

pub async fn load_initial_state_from_stream<S>(
    stream: S,
    codec: Codec,
) -> Result<ClientInitialState, ClientBootstrapError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    Ok(connect_from_stream(stream, codec).await?.initial_state)
}

pub async fn connect_from_stream<S>(
    mut stream: S,
    codec: Codec,
) -> Result<ClientSession, ClientBootstrapError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let initial_state = handshake_initial_state(&mut stream, codec).await?;
    let (edit_queue, outgoing_edits) = ClientEditQueue::bounded(EDIT_QUEUE_CAPACITY);
    let edit_queue = edit_queue
        .with_authority(initial_state.client_id, &initial_state.access)
        .with_confirmed_version(initial_state.document_version);
    let sync_state = Arc::clone(&edit_queue.sync_state);
    let behavior_state = Arc::new(Mutex::new(
        behavior::ClientBehaviorState::new(initial_state.behavior_manifest.clone())
            .map_err(|_| ClientBootstrapError::UnexpectedMessage("invalid BehaviorManifest"))?,
    ));
    let (event_sender, events) = mpsc::channel(EDIT_QUEUE_CAPACITY);
    tokio::spawn(run_connection(
        stream,
        codec,
        outgoing_edits,
        event_sender,
        sync_state,
        behavior_state,
        initial_state.client_id,
    ));

    Ok(ClientSession {
        initial_state,
        edit_queue,
        events,
    })
}

async fn handshake_initial_state<S>(
    stream: &mut S,
    codec: Codec,
) -> Result<ClientInitialState, ClientBootstrapError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    codec
        .write_client_message(
            &mut *stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: CLIENT_NAME.to_string(),
            },
        )
        .await?;

    let client_id = match codec.read_server_message(&mut *stream).await? {
        ServerMessage::Welcome {
            client_id,
            protocol_version,
        } if protocol_version == PROTOCOL_VERSION => client_id,
        ServerMessage::Error { code, message } => {
            return Err(ClientBootstrapError::ServerError { code, message });
        }
        _ => return Err(ClientBootstrapError::UnexpectedMessage("expected Welcome")),
    };

    let (document_id, document_version, text, access) =
        match codec.read_server_message(&mut *stream).await? {
            ServerMessage::InitialDocument {
                document_id,
                version,
                text,
                access,
                lease_id: _,
            } => (document_id, version, text, access),
            ServerMessage::Error { code, message } => {
                return Err(ClientBootstrapError::ServerError { code, message });
            }
            _ => {
                return Err(ClientBootstrapError::UnexpectedMessage(
                    "expected InitialDocument",
                ));
            }
        };

    let behavior_manifest = match codec.read_server_message(&mut *stream).await? {
        ServerMessage::BehaviorManifest(manifest) => {
            behavior::ClientBehaviorState::new(manifest.clone())
                .map_err(|_| ClientBootstrapError::UnexpectedMessage("invalid BehaviorManifest"))?;
            manifest
        }
        ServerMessage::Error { code, message } => {
            return Err(ClientBootstrapError::ServerError { code, message });
        }
        _ => {
            return Err(ClientBootstrapError::UnexpectedMessage(
                "expected BehaviorManifest",
            ));
        }
    };

    Ok(ClientInitialState {
        client_id,
        document_id,
        document_version,
        text,
        access,
        behavior_manifest,
    })
}

fn rejection_requests_resync(reason: &EditRejection) -> bool {
    matches!(
        reason,
        EditRejection::StaleVersion { .. }
            | EditRejection::FutureVersion { .. }
            | EditRejection::LeaseRequired
            | EditRejection::LeaseExpired { .. }
            | EditRejection::ReadOnlyDocument
            | EditRejection::RegionLocked { .. }
    )
}

async fn run_connection<S>(
    stream: S,
    codec: Codec,
    mut outgoing_edits: mpsc::Receiver<ClientMessage>,
    events: mpsc::Sender<ClientConnectionEvent>,
    sync_state: Arc<Mutex<ClientSyncState>>,
    behavior_state: Arc<Mutex<behavior::ClientBehaviorState>>,
    client_id: ClientId,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut reader, mut writer) = tokio::io::split(stream);

    loop {
        tokio::select! {
            outgoing = outgoing_edits.recv() => {
                let Some(message) = outgoing else {
                    let _ = events.send(ClientConnectionEvent::Disconnected).await;
                    return;
                };
                if let Err(error) = codec.write_client_message(&mut writer, &message).await {
                    let _ = events.send(ClientConnectionEvent::ConnectionError(error.to_string())).await;
                    return;
                }
            }
            incoming = codec.read_server_message(&mut reader) => {
                match incoming {
                    Ok(ServerMessage::EditAck { document_id, confirmed_version, transaction_id }) => {
                        sync_state
                            .lock()
                            .expect("client sync state poisoned")
                            .acknowledge(confirmed_version, transaction_id);
                        let _ = events.send(ClientConnectionEvent::EditAck { document_id, version: confirmed_version, transaction_id }).await;
                    }
                    Ok(ServerMessage::EditRejected { document_id, transaction_id, reason }) => {
                        let known_version = {
                            let mut state = sync_state
                                .lock()
                                .expect("client sync state poisoned");
                            state.reject(transaction_id);
                            state.confirmed_version
                        };
                        let should_resync = rejection_requests_resync(&reason);
                        let _ = events.send(ClientConnectionEvent::EditRejected { document_id, transaction_id, reason }).await;
                        if should_resync {
                            let request = ClientMessage::RequestResync {
                                document_id,
                                client_id,
                                known_version,
                            };
                            if let Err(error) = codec.write_client_message(&mut writer, &request).await {
                                let _ = events.send(ClientConnectionEvent::ConnectionError(error.to_string())).await;
                                return;
                            }
                        }
                    }
                    Ok(ServerMessage::ResyncSnapshot { document_id, version, text, access, lease_id }) => {
                        let snapshot = ClientResyncSnapshot { document_id, version, text, access, lease_id };
                        sync_state
                            .lock()
                            .expect("client sync state poisoned")
                            .apply_resync_snapshot(snapshot.clone());
                        let _ = events.send(ClientConnectionEvent::ResyncSnapshot(snapshot)).await;
                    }
                    Ok(message @ ServerMessage::EditTransaction { .. }) => {
                        let _ = events.send(ClientConnectionEvent::EditTransaction(message)).await;
                    }
                    Ok(ServerMessage::BehaviorManifest(manifest)) => {
                        let behavior_version = manifest.behavior_version;
                        let install_result = behavior_state
                            .lock()
                            .expect("client behavior state poisoned")
                            .install_replacement(manifest.clone());
                        match install_result {
                            Ok(()) => {
                                let _ = events.send(ClientConnectionEvent::BehaviorManifestInstalled { behavior_version, manifest }).await;
                            }
                            Err(error) => {
                                let _ = events.send(ClientConnectionEvent::BehaviorManifestRejected {
                                    behavior_version,
                                    reason: format!("{error:?}"),
                                }).await;
                            }
                        }
                    }
                    Ok(ServerMessage::Error { code, message }) => {
                        let _ = events.send(ClientConnectionEvent::ServerError { code, message }).await;
                    }
                    Ok(_) => {}
                    Err(CodecError::Io(error)) if matches!(
                        error.kind(),
                        std::io::ErrorKind::UnexpectedEof
                            | std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::BrokenPipe
                    ) => {
                        let _ = events.send(ClientConnectionEvent::Disconnected).await;
                        return;
                    }
                    Err(error) => {
                        let _ = events.send(ClientConnectionEvent::ConnectionError(error.to_string())).await;
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::fs;
    #[cfg(any(unix, windows))]
    use std::time::SystemTime;

    use tokio::io::duplex;
    #[cfg(unix)]
    use tokio::net::UnixStream;

    #[cfg(windows)]
    use super::connect_transport;
    use super::{
        ClientConnectionEvent, ClientEditQueue, connect_from_stream, load_initial_state_from_stream,
    };
    #[cfg(any(unix, windows))]
    use super::{ClientSession, connect};
    use crate::editor::EditorEditEvent;
    #[cfg(any(unix, windows))]
    use crate::ipc::IpcEndpoint;
    #[cfg(any(unix, windows))]
    use crate::protocol::EditRejection;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, CommandDeclaration, DocumentAccess, EditOperation,
        PROTOCOL_VERSION, ServerMessage, codec::Codec,
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

    #[tokio::test]
    async fn client_handles_initial_document_message() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 11,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 7,
                        version: 3,
                        text: "Loaded from server 🦀".to_string(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(9)),
                )
                .await
                .unwrap();
        });

        let state = load_initial_state_from_stream(client, codec).await.unwrap();

        assert_eq!(state.client_id, 11);
        assert_eq!(state.document_id, 7);
        assert_eq!(state.document_version, 3);
        assert_eq!(state.text, "Loaded from server 🦀");
        assert_eq!(state.access, DocumentAccess::Editable { lease_id: 1 });
        assert_eq!(
            state.behavior_manifest,
            BehaviorManifest::minimal_text_editing(9)
        );
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn edit_event_is_enqueued_as_client_edit_message() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue
            .with_authority(0, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(5);

        queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: 4,
                    base_version: 5,
                    behavior_version: 6,
                    operation: EditOperation::Insert {
                        byte_offset: 2,
                        text: "x".to_string(),
                    },
                },
                7,
            )
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            crate::protocol::ClientMessage::Edit {
                document_id: 4,
                client_id: 0,
                lease_id: Some(1),
                base_version: 5,
                behavior_version: 6,
                transaction_id: 7,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: "x".to_string()
                }
            }
        );
    }

    #[tokio::test]
    async fn read_only_client_queue_does_not_emit_edit_message() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_confirmed_version(5);

        let result = queue.enqueue_edit_event(
            EditorEditEvent {
                document_id: 4,
                base_version: 5,
                behavior_version: 6,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: "x".to_string(),
                },
            },
            7,
        );

        assert!(result.is_err());
        assert!(receiver.try_recv().is_err());
        assert!(queue.sync_snapshot().pending.is_empty());
    }

    #[tokio::test]
    async fn bounded_edit_queue_applies_backpressure() {
        let (queue, _receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_authority(0, &DocumentAccess::Editable { lease_id: 1 });
        let event = EditorEditEvent {
            document_id: 1,
            base_version: 2,
            behavior_version: 3,
            operation: EditOperation::Delete { start: 4, end: 5 },
        };

        assert!(queue.enqueue_edit_event(event.clone(), 1).is_ok());
        assert!(queue.enqueue_edit_event(event, 2).is_err());
        assert_eq!(queue.sync_snapshot().pending.len(), 1);
    }

    #[tokio::test]
    async fn client_hot_path_does_not_await_full_ipc_queue() {
        let (queue, _receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_authority(0, &DocumentAccess::Editable { lease_id: 1 });
        let event = EditorEditEvent {
            document_id: 1,
            base_version: 2,
            behavior_version: 3,
            operation: EditOperation::Insert {
                byte_offset: 0,
                text: "x".to_string(),
            },
        };
        queue.enqueue_edit_event(event.clone(), 1).unwrap();

        let started = std::time::Instant::now();
        let result = queue.enqueue_edit_event(event, 2);

        assert!(result.is_err());
        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "full queue should fail through try_send instead of awaiting capacity"
        );
        assert_eq!(queue.sync_snapshot().pending.len(), 1);
    }

    #[tokio::test]
    async fn client_keeps_pending_edit_until_ack_or_rejection() {
        let (queue, mut receiver) = ClientEditQueue::bounded(2);
        let queue = queue
            .with_authority(0, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(10);
        let event = EditorEditEvent {
            document_id: 1,
            base_version: 0,
            behavior_version: 3,
            operation: EditOperation::Insert {
                byte_offset: 0,
                text: "a".to_string(),
            },
        };

        queue.enqueue_edit_event(event, 44).unwrap();
        let message = receiver.recv().await.unwrap();

        assert!(matches!(
            message,
            ClientMessage::Edit {
                base_version: 10,
                transaction_id: 44,
                ..
            }
        ));
        let snapshot = queue.sync_snapshot();
        assert_eq!(snapshot.confirmed_version, 10);
        assert_eq!(snapshot.optimistic_version, 11);
        assert_eq!(snapshot.pending.len(), 1);
    }

    #[tokio::test]
    async fn client_ack_advances_confirmed_version() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 1,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 7,
                        version: 10,
                        text: String::new(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(3)),
                )
                .await
                .unwrap();
            let _edit = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::EditAck {
                        document_id: 7,
                        confirmed_version: 11,
                        transaction_id: 44,
                    },
                )
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();
        session
            .edit_queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: 7,
                    base_version: 0,
                    behavior_version: 3,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "a".to_string(),
                    },
                },
                44,
            )
            .unwrap();
        assert_eq!(session.edit_queue.sync_snapshot().pending.len(), 1);

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::EditAck {
                document_id: 7,
                version: 11,
                transaction_id: 44,
            }
        );
        let snapshot = session.edit_queue.sync_snapshot();
        assert_eq!(snapshot.confirmed_version, 11);
        assert_eq!(snapshot.pending.len(), 0);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_requests_resync_after_stale_rejection() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 12,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 7,
                        version: 10,
                        text: "local".to_string(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(3)),
                )
                .await
                .unwrap();
            let _edit = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::EditRejected {
                        document_id: 7,
                        transaction_id: 44,
                        reason: crate::protocol::EditRejection::StaleVersion {
                            client_base_version: 10,
                            server_version: 12,
                        },
                    },
                )
                .await
                .unwrap();

            assert_eq!(
                codec.read_client_message(&mut server).await.unwrap(),
                ClientMessage::RequestResync {
                    document_id: 7,
                    client_id: 12,
                    known_version: 10,
                }
            );
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();
        session
            .edit_queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: 7,
                    base_version: 0,
                    behavior_version: 3,
                    operation: EditOperation::Insert {
                        byte_offset: 5,
                        text: "!".to_string(),
                    },
                },
                44,
            )
            .unwrap();

        assert!(matches!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::EditRejected {
                transaction_id: 44,
                ..
            }
        ));
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_applies_resync_snapshot_and_clears_pending_edits() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 1,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 7,
                        version: 10,
                        text: "local".to_string(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(3)),
                )
                .await
                .unwrap();
            let _edit = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::EditRejected {
                        document_id: 7,
                        transaction_id: 44,
                        reason: crate::protocol::EditRejection::StaleVersion {
                            client_base_version: 10,
                            server_version: 12,
                        },
                    },
                )
                .await
                .unwrap();
            let _resync = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ResyncSnapshot {
                        document_id: 7,
                        version: 12,
                        text: "server 🦀".to_string(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();
        session
            .edit_queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: 7,
                    base_version: 0,
                    behavior_version: 3,
                    operation: EditOperation::Insert {
                        byte_offset: 5,
                        text: "!".to_string(),
                    },
                },
                44,
            )
            .unwrap();
        let _rejection = session.events.recv().await.unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::ResyncSnapshot(super::ClientResyncSnapshot {
                document_id: 7,
                version: 12,
                text: "server 🦀".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
            })
        );
        let snapshot = session.edit_queue.sync_snapshot();
        assert_eq!(snapshot.confirmed_version, 12);
        assert_eq!(snapshot.optimistic_version, 12);
        assert!(snapshot.pending.is_empty());
        assert_eq!(snapshot.last_resync.unwrap().text, "server 🦀");
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_installs_minimal_behavior_manifest() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 1,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 2,
                        version: 1,
                        text: String::new(),
                        access: DocumentAccess::ReadOnly,
                        lease_id: None,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(5)),
                )
                .await
                .unwrap();
        });

        let state = load_initial_state_from_stream(client, codec).await.unwrap();

        assert_eq!(state.behavior_manifest.behavior_version, 5);
        assert_eq!(state.access, DocumentAccess::ReadOnly);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn end_to_end_client_receives_initial_snapshot() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 21,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 22,
                        version: 23,
                        text: "snapshot".to_string(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(24)),
                )
                .await
                .unwrap();
        });

        let session = connect_from_stream(client, codec).await.unwrap();

        assert_eq!(session.initial_state.document_id, 22);
        assert_eq!(session.initial_state.document_version, 23);
        assert_eq!(session.initial_state.text, "snapshot");
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn end_to_end_client_receives_behavior_manifest() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 1,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 2,
                        version: 3,
                        text: String::new(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(44)),
                )
                .await
                .unwrap();
        });

        let session = connect_from_stream(client, codec).await.unwrap();

        assert_eq!(session.initial_state.behavior_manifest.behavior_version, 44);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_installs_behavior_manifest_replacement_event() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 1,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 2,
                        version: 3,
                        text: String::new(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(4)),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(5)),
                )
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::BehaviorManifestInstalled {
                behavior_version: 5,
                manifest: BehaviorManifest::minimal_text_editing(5),
            }
        );
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_rejects_invalid_behavior_manifest_replacement_event() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 1,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 2,
                        version: 3,
                        text: String::new(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(4)),
                )
                .await
                .unwrap();
            let mut invalid = BehaviorManifest::minimal_text_editing(5);
            invalid
                .commands
                .push(CommandDeclaration::client_edit("text.insert", "Duplicate"));
            codec
                .write_server_message(&mut server, &ServerMessage::BehaviorManifest(invalid))
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();

        assert!(matches!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::BehaviorManifestRejected {
                behavior_version: 5,
                ..
            }
        ));
        server_task.await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn end_to_end_second_client_is_read_only() {
        let socket_path = unique_socket_path("read-only");
        let server = IpcServer::new(ServerConfig::new(&socket_path));
        let server_task = tokio::spawn(server.run());

        let first = connect_with_retry(&socket_path).await;
        let second = connect_with_retry(&socket_path).await;

        assert!(matches!(
            first.initial_state.access,
            DocumentAccess::Editable { lease_id: 1 }
        ));
        assert_eq!(second.initial_state.access, DocumentAccess::ReadOnly);

        drop(first);
        drop(second);
        server_task.abort();
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_dir(socket_path.parent().unwrap());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn real_server_end_to_end_edit_gets_acknowledged() {
        let socket_path = unique_socket_path("ack");
        let server = IpcServer::new(ServerConfig::new(&socket_path));
        let server_task = tokio::spawn(server.run());

        let mut session = connect_with_retry(&socket_path).await;
        session
            .edit_queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: session.initial_state.document_id,
                    base_version: session.initial_state.document_version,
                    behavior_version: session.initial_state.behavior_manifest.behavior_version,
                    operation: EditOperation::Insert {
                        byte_offset: session.initial_state.text.len() as u64,
                        text: "manual".to_string(),
                    },
                },
                77,
            )
            .unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::EditAck {
                document_id: session.initial_state.document_id,
                version: session.initial_state.document_version + 1,
                transaction_id: 77,
            }
        );

        server_task.abort();
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_dir(socket_path.parent().unwrap());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_named_pipe_client_receives_initial_snapshot() {
        let endpoint = unique_named_pipe("snapshot");
        let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
        let server_task = tokio::spawn(server.run());

        let session = connect_with_retry(&endpoint).await;

        assert_eq!(
            session.initial_state.text,
            "Welcome to Clay's Phase 4 IPC server.\n"
        );
        assert!(matches!(
            session.initial_state.access,
            DocumentAccess::Editable { lease_id: 1 }
        ));
        assert_eq!(session.initial_state.behavior_manifest.behavior_version, 1);

        server_task.abort();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_named_pipe_edit_gets_acknowledged() {
        let endpoint = unique_named_pipe("ack");
        let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
        let server_task = tokio::spawn(server.run());

        let mut session = connect_with_retry(&endpoint).await;
        session
            .edit_queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: session.initial_state.document_id,
                    base_version: session.initial_state.document_version,
                    behavior_version: session.initial_state.behavior_manifest.behavior_version,
                    operation: EditOperation::Insert {
                        byte_offset: session.initial_state.text.len() as u64,
                        text: "pipe".to_string(),
                    },
                },
                88,
            )
            .unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::EditAck {
                document_id: session.initial_state.document_id,
                version: session.initial_state.document_version + 1,
                transaction_id: 88,
            }
        );

        server_task.abort();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_second_client_is_read_only() {
        let endpoint = unique_named_pipe("read-only");
        let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
        let server_task = tokio::spawn(server.run());

        let first = connect_with_retry(&endpoint).await;
        let second = connect_with_retry(&endpoint).await;

        assert!(matches!(
            first.initial_state.access,
            DocumentAccess::Editable { lease_id: 1 }
        ));
        assert_eq!(second.initial_state.access, DocumentAccess::ReadOnly);

        server_task.abort();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_named_pipe_stale_edit_rejected_then_resynced() {
        let endpoint = unique_named_pipe("stale-resync");
        let server = IpcServer::new(ServerConfig::new(endpoint.clone()));
        let server_task = tokio::spawn(server.run());

        let mut stream = {
            let mut last_error = None;
            let mut stream = None;
            for _ in 0..50 {
                match connect_transport(&endpoint).await {
                    Ok(connected) => {
                        stream = Some(connected);
                        break;
                    }
                    Err(error) => {
                        last_error = Some(error);
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }
            }
            stream.unwrap_or_else(|| panic!("failed to connect to test named pipe: {last_error:?}"))
        };
        let codec = Codec::default();
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "stale-test".to_string(),
                },
            )
            .await
            .unwrap();

        let client_id = match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::Welcome { client_id, .. } => client_id,
            message => panic!("expected Welcome, got {message:?}"),
        };
        let (document_id, version, text, lease_id) =
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::InitialDocument {
                    document_id,
                    version,
                    text,
                    access: DocumentAccess::Editable { lease_id },
                    lease_id: Some(snapshot_lease_id),
                } => {
                    assert_eq!(lease_id, snapshot_lease_id);
                    (document_id, version, text, lease_id)
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
                    base_version: version - 1,
                    behavior_version: 1,
                    transaction_id: 99,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "stale".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::EditRejected {
                document_id,
                transaction_id: 99,
                reason: EditRejection::StaleVersion {
                    client_base_version: version - 1,
                    server_version: version,
                },
            }
        );

        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::RequestResync {
                    document_id,
                    client_id,
                    known_version: version - 1,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::ResyncSnapshot {
                document_id,
                version,
                text,
                access: DocumentAccess::Editable { lease_id },
                lease_id: Some(lease_id),
            }
        );

        server_task.abort();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn real_server_end_to_end_stale_edit_rejected_then_resynced() {
        let socket_path = unique_socket_path("stale-resync");
        let server = IpcServer::new(ServerConfig::new(&socket_path));
        let server_task = tokio::spawn(server.run());

        let mut stream = connect_stream_with_retry(&socket_path).await;
        let codec = Codec::default();
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "stale-test".to_string(),
                },
            )
            .await
            .unwrap();

        let client_id = match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::Welcome { client_id, .. } => client_id,
            message => panic!("expected Welcome, got {message:?}"),
        };
        let (document_id, version, text, lease_id) =
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::InitialDocument {
                    document_id,
                    version,
                    text,
                    access: DocumentAccess::Editable { lease_id },
                    lease_id: Some(snapshot_lease_id),
                } => {
                    assert_eq!(lease_id, snapshot_lease_id);
                    (document_id, version, text, lease_id)
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
                    base_version: version - 1,
                    behavior_version: 1,
                    transaction_id: 99,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "stale".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::EditRejected {
                document_id,
                transaction_id: 99,
                reason: EditRejection::StaleVersion {
                    client_base_version: version - 1,
                    server_version: version,
                },
            }
        );

        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::RequestResync {
                    document_id,
                    client_id,
                    known_version: version - 1,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::ResyncSnapshot {
                document_id,
                version,
                text,
                access: DocumentAccess::Editable { lease_id },
                lease_id: Some(lease_id),
            }
        );

        server_task.abort();
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_dir(socket_path.parent().unwrap());
    }

    #[tokio::test]
    async fn end_to_end_edit_gets_acknowledged() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let server_task = tokio::spawn(async move {
            let _hello = codec.read_client_message(&mut server).await.unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::Welcome {
                        client_id: 1,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::InitialDocument {
                        document_id: 7,
                        version: 1,
                        text: "Hi".to_string(),
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                    },
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1)),
                )
                .await
                .unwrap();

            assert_eq!(
                codec.read_client_message(&mut server).await.unwrap(),
                ClientMessage::Edit {
                    document_id: 7,
                    client_id: 1,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 1,
                    transaction_id: 9,
                    operation: EditOperation::Insert {
                        byte_offset: 2,
                        text: "!".to_string()
                    }
                }
            );
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::EditAck {
                        document_id: 7,
                        confirmed_version: 2,
                        transaction_id: 9,
                    },
                )
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();
        session
            .edit_queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: 7,
                    base_version: 1,
                    behavior_version: 1,
                    operation: EditOperation::Insert {
                        byte_offset: 2,
                        text: "!".to_string(),
                    },
                },
                9,
            )
            .unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::EditAck {
                document_id: 7,
                version: 2,
                transaction_id: 9,
            }
        );
        server_task.await.unwrap();
    }
}
