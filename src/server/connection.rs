use std::sync::Arc;

use tokio::{net::UnixStream, sync::Mutex};

use crate::protocol::{
    BehaviorManifest, ClientMessage, PROTOCOL_VERSION, ProtocolErrorCode, ServerMessage,
    codec::{Codec, CodecError},
};

use super::document::DocumentState;

pub(crate) async fn handle_connection(
    mut stream: UnixStream,
    client_id: u64,
    document: Arc<Mutex<DocumentState>>,
    codec: Codec,
) -> Result<(), CodecError> {
    let first_message = codec.read_client_message(&mut stream).await?;
    match first_message {
        ClientMessage::Hello {
            protocol_version,
            client_name: _,
        } if protocol_version == PROTOCOL_VERSION => {
            send_welcome_snapshot_and_manifest(&mut stream, client_id, &document, codec).await?;
        }
        ClientMessage::Hello { .. } => {
            codec
                .write_server_message(
                    &mut stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::UnsupportedProtocolVersion,
                        message: "unsupported protocol version".to_string(),
                    },
                )
                .await?;
            return Ok(());
        }
        _ => {
            codec
                .write_server_message(
                    &mut stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message: "first client message must be Hello".to_string(),
                    },
                )
                .await?;
            return Ok(());
        }
    }

    loop {
        let message = match codec.read_client_message(&mut stream).await {
            Ok(message) => message,
            Err(CodecError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                document.lock().await.release_access(client_id);
                return Ok(());
            }
            Err(error) => {
                document.lock().await.release_access(client_id);
                return Err(error);
            }
        };

        match message {
            ClientMessage::Edit {
                document_id,
                client_id,
                lease_id,
                base_version,
                behavior_version: _,
                transaction_id,
                operation,
            } => {
                let response = {
                    let mut document = document.lock().await;
                    document.apply_edit(
                        document_id,
                        client_id,
                        lease_id,
                        base_version,
                        transaction_id,
                        operation,
                    )
                };
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::EditorIntent {
                document_id,
                client_id,
                lease_id,
                base_version,
                behavior_version: _,
                transaction_id,
                intent,
            } => {
                let operation = match intent {
                    crate::protocol::EditorIntent::InsertText { byte_offset, text } => {
                        crate::protocol::EditOperation::Insert { byte_offset, text }
                    }
                    crate::protocol::EditorIntent::DeleteRange { start, end } => {
                        crate::protocol::EditOperation::Delete { start, end }
                    }
                };
                let response = {
                    let mut document = document.lock().await;
                    document.apply_edit(
                        document_id,
                        client_id,
                        lease_id,
                        base_version,
                        transaction_id,
                        operation,
                    )
                };
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::RequestResync {
                document_id,
                client_id,
                known_version: _,
            } => {
                let response = {
                    let document = document.lock().await;
                    document.resync_snapshot_message_for_client(document_id, client_id)
                };
                codec.write_server_message(&mut stream, &response).await?;
            }
            ClientMessage::Hello { .. } => {
                codec
                    .write_server_message(
                        &mut stream,
                        &ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message: "duplicate Hello message".to_string(),
                        },
                    )
                    .await?;
            }
        }
    }
}

async fn send_welcome_snapshot_and_manifest(
    stream: &mut UnixStream,
    client_id: u64,
    document: &Arc<Mutex<DocumentState>>,
    codec: Codec,
) -> Result<(), CodecError> {
    codec
        .write_server_message(
            stream,
            &ServerMessage::Welcome {
                client_id,
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await?;

    let initial_document = {
        let mut document = document.lock().await;
        let access = document.acquire_access(client_id);
        document.initial_document_message(access)
    };
    codec
        .write_server_message(stream, &initial_document)
        .await?;

    codec
        .write_server_message(
            stream,
            &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1)),
        )
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::{net::UnixStream, sync::Mutex};

    use super::handle_connection;
    use crate::{
        protocol::{
            BehaviorManifest, ClientMessage, DocumentAccess, EditOperation, PROTOCOL_VERSION,
            ServerMessage, codec::Codec,
        },
        server::document::DocumentState,
    };

    #[tokio::test]
    async fn server_accepts_hello_and_sends_snapshot() {
        let (client, server) = UnixStream::pair().unwrap();
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "Hello from server".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let server_task = tokio::spawn(handle_connection(server, 99, document, codec));
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

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::Welcome {
                client_id: 99,
                protocol_version: PROTOCOL_VERSION,
            }
        );
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::InitialDocument {
                document_id: 7,
                version: 1,
                text: "Hello from server".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_sends_minimal_behavior_manifest() {
        let (client, server) = UnixStream::pair().unwrap();
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let server_task = tokio::spawn(handle_connection(server, 99, document, codec));
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

        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1))
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_acknowledges_insert_edit() {
        let (client, server) = UnixStream::pair().unwrap();
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "Hi".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let server_task = tokio::spawn(handle_connection(server, 99, document, codec));
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
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Edit {
                    document_id: 7,
                    client_id: 99,
                    lease_id: Some(1),
                    base_version: 1,
                    behavior_version: 1,
                    transaction_id: 123,
                    operation: EditOperation::Insert {
                        byte_offset: 2,
                        text: " Clay".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::EditAck {
                document_id: 7,
                confirmed_version: 2,
                transaction_id: 123,
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_sends_resync_snapshot_after_request() {
        let (client, server) = UnixStream::pair().unwrap();
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::new(
            7,
            "server 🦀".to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        )));
        let server_task = tokio::spawn(handle_connection(server, 99, document, codec));
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
        let _welcome = codec.read_server_message(&mut client).await.unwrap();
        let _snapshot = codec.read_server_message(&mut client).await.unwrap();
        let _manifest = codec.read_server_message(&mut client).await.unwrap();

        codec
            .write_client_message(
                &mut client,
                &ClientMessage::RequestResync {
                    document_id: 7,
                    client_id: 99,
                    known_version: 0,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::ResyncSnapshot {
                document_id: 7,
                version: 1,
                text: "server 🦀".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
            }
        );

        drop(client);
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn server_rejects_invalid_frame_without_panic() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let codec = Codec::default();
        let document = Arc::new(Mutex::new(DocumentState::default()));
        let server_task = tokio::spawn(handle_connection(server, 99, document, codec));

        tokio::io::AsyncWriteExt::write_all(&mut client, &[0, 0, 0, 4, 0xde, 0xad, 0xbe, 0xef])
            .await
            .unwrap();
        drop(client);

        let result = server_task.await.unwrap();
        assert!(result.is_err());
    }
}
