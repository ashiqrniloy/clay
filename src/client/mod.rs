#[cfg(unix)]
use std::path::Path;

use crate::editor::EditorEditEvent;
use crate::protocol::{
    BehaviorManifest, ClientId, ClientMessage, DocumentAccess, DocumentId, DocumentVersion,
    PROTOCOL_VERSION, ProtocolErrorCode, ServerMessage, TransactionId,
    codec::{Codec, CodecError},
};

use tokio::sync::mpsc;

#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio::time::{Duration, timeout};

const CLIENT_NAME: &str = "clay-client";
const EDIT_QUEUE_CAPACITY: usize = 256;
#[cfg(unix)]
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInitialState {
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub text: String,
    pub access: DocumentAccess,
    pub behavior_manifest: BehaviorManifest,
}

#[derive(Debug, Clone)]
pub struct ClientEditQueue {
    sender: mpsc::Sender<ClientMessage>,
}

impl ClientEditQueue {
    pub fn bounded(capacity: usize) -> (Self, mpsc::Receiver<ClientMessage>) {
        let (sender, receiver) = mpsc::channel(capacity);
        (Self { sender }, receiver)
    }

    pub fn from_sender(sender: mpsc::Sender<ClientMessage>) -> Self {
        Self { sender }
    }

    pub fn enqueue_edit_event(
        &self,
        event: EditorEditEvent,
        transaction_id: TransactionId,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::Edit {
            document_id: event.document_id,
            base_version: event.base_version,
            behavior_version: event.behavior_version,
            transaction_id,
            operation: event.operation,
        })
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

#[cfg(unix)]
pub async fn connect(socket_path: impl AsRef<Path>) -> Result<ClientSession, ClientBootstrapError> {
    let stream = UnixStream::connect(socket_path)
        .await
        .map_err(CodecError::Io)?;
    timeout(
        SNAPSHOT_TIMEOUT,
        connect_from_stream(stream, Codec::default()),
    )
    .await
    .map_err(|_| ClientBootstrapError::Timeout)?
}

#[cfg(unix)]
pub async fn load_initial_state(
    socket_path: impl AsRef<Path>,
) -> Result<ClientInitialState, ClientBootstrapError> {
    Ok(connect(socket_path).await?.initial_state)
}

#[cfg(unix)]
pub async fn load_initial_state_from_stream(
    stream: UnixStream,
    codec: Codec,
) -> Result<ClientInitialState, ClientBootstrapError> {
    Ok(connect_from_stream(stream, codec).await?.initial_state)
}

#[cfg(unix)]
pub async fn connect_from_stream(
    mut stream: UnixStream,
    codec: Codec,
) -> Result<ClientSession, ClientBootstrapError> {
    let initial_state = handshake_initial_state(&mut stream, codec).await?;
    let (edit_queue, outgoing_edits) = ClientEditQueue::bounded(EDIT_QUEUE_CAPACITY);
    let (event_sender, events) = mpsc::channel(EDIT_QUEUE_CAPACITY);
    tokio::spawn(run_connection(stream, codec, outgoing_edits, event_sender));

    Ok(ClientSession {
        initial_state,
        edit_queue,
        events,
    })
}

#[cfg(unix)]
async fn handshake_initial_state(
    stream: &mut UnixStream,
    codec: Codec,
) -> Result<ClientInitialState, ClientBootstrapError> {
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
        ServerMessage::BehaviorManifest(manifest) => manifest,
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

#[cfg(unix)]
async fn run_connection(
    stream: UnixStream,
    codec: Codec,
    mut outgoing_edits: mpsc::Receiver<ClientMessage>,
    events: mpsc::Sender<ClientConnectionEvent>,
) {
    let (mut reader, mut writer) = stream.into_split();

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
                    Ok(ServerMessage::EditAck { document_id, version, transaction_id }) => {
                        let _ = events.send(ClientConnectionEvent::EditAck { document_id, version, transaction_id }).await;
                    }
                    Ok(message @ ServerMessage::EditTransaction { .. }) => {
                        let _ = events.send(ClientConnectionEvent::EditTransaction(message)).await;
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
    use std::{fs, time::SystemTime};

    use tokio::net::UnixStream;

    use super::{
        ClientConnectionEvent, ClientEditQueue, ClientSession, connect, connect_from_stream,
        load_initial_state_from_stream,
    };
    use crate::editor::EditorEditEvent;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, DocumentAccess, EditOperation, PROTOCOL_VERSION,
        ServerMessage, codec::Codec,
    };
    use crate::server::{IpcServer, ServerConfig};

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

    async fn connect_with_retry(socket_path: &std::path::Path) -> ClientSession {
        let mut last_error = None;
        for _ in 0..50 {
            match connect(socket_path).await {
                Ok(session) => return session,
                Err(error) => {
                    last_error = Some(error.to_string());
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            }
        }
        panic!("failed to connect to test socket: {:?}", last_error);
    }

    #[tokio::test]
    async fn client_handles_initial_document_message() {
        let (client, mut server) = UnixStream::pair().unwrap();
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
                        access: DocumentAccess::Editable,
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
        assert_eq!(state.access, DocumentAccess::Editable);
        assert_eq!(
            state.behavior_manifest,
            BehaviorManifest::minimal_text_editing(9)
        );
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn edit_event_is_enqueued_as_client_edit_message() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);

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
    async fn bounded_edit_queue_applies_backpressure() {
        let (queue, _receiver) = ClientEditQueue::bounded(1);
        let event = EditorEditEvent {
            document_id: 1,
            base_version: 2,
            behavior_version: 3,
            operation: EditOperation::Delete { start: 4, end: 5 },
        };

        assert!(queue.enqueue_edit_event(event.clone(), 1).is_ok());
        assert!(queue.enqueue_edit_event(event, 2).is_err());
    }

    #[tokio::test]
    async fn client_installs_minimal_behavior_manifest() {
        let (client, mut server) = UnixStream::pair().unwrap();
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
        let (client, mut server) = UnixStream::pair().unwrap();
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
                        access: DocumentAccess::Editable,
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
        let (client, mut server) = UnixStream::pair().unwrap();
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
                        access: DocumentAccess::Editable,
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

    #[tokio::test]
    async fn end_to_end_edit_gets_acknowledged() {
        let (client, mut server) = UnixStream::pair().unwrap();
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
                        access: DocumentAccess::Editable,
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
                        version: 2,
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
