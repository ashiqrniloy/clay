pub(crate) mod behavior;
pub mod clipboard;
pub mod file_dialog;
pub(crate) mod runtime_state;

pub use behavior::ClientUiCommandRoute;
pub use behavior::language_intelligence_feature_for_command;
pub use clipboard::{
    ClipboardError, ClipboardSink, SystemClipboard, copy_text_to_system_clipboard,
    read_text_from_system_clipboard,
};
pub use file_dialog::{
    FileDialogFilter, FileDialogResult, markdown_file_dialog_filters, open_folder_dialog,
    open_markdown_file_dialog,
};
pub(crate) use runtime_state::{ClientRuntimeStateCandidate, ClientRuntimeStateInstallError};

use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::editor::{
    EditorCompletionRequestEvent, EditorEditEvent, EditorLanguageIntelligenceRequestEvent,
};
use crate::ipc::IpcEndpoint;
use crate::perf::metrics::{MetricMetadata, global_recorder};
use crate::protocol::{
    ActiveTypography, BehaviorManifest, BehaviorVersion, ClientId, ClientMessage,
    CompletionRejection, CompletionRequest, CompletionRequestId, CompletionResultSet,
    DecorationSet, DiagnosticSet, DocumentAccess, DocumentId, DocumentMetadata, DocumentVersion,
    EditOperation, EditRejection, FileErrorCode, LanguageIntelligenceRejection,
    LanguageIntelligenceRequest, LanguageIntelligenceRequestId, LanguageIntelligenceResult,
    PROTOCOL_VERSION, ProtocolErrorCode, RuntimeDiagnostic, SduiActionIntent, SduiTree,
    SduiTreeUpdate, ServerMessage, TransactionId, WorkspaceRootId,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ClientInitialState {
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub text: String,
    pub access: DocumentAccess,
    pub behavior_manifest: BehaviorManifest,
    pub active_theme: crate::protocol::ActiveTheme,
    pub active_typography: ActiveTypography,
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
    /// Document currently owning live optimistic version tracking. Acks/rejects
    /// for other document ids are ignored so backgrounded multi-doc sessions
    /// cannot corrupt the active document's base version.
    document_id: Option<DocumentId>,
    confirmed_version: DocumentVersion,
    optimistic_version: DocumentVersion,
    pending: VecDeque<PendingEdit>,
    last_resync: Option<ClientResyncSnapshot>,
}

impl ClientSyncState {
    fn new(confirmed_version: DocumentVersion) -> Self {
        Self::for_document(None, confirmed_version)
    }

    fn for_document(document_id: Option<DocumentId>, confirmed_version: DocumentVersion) -> Self {
        Self {
            document_id,
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

    fn acknowledge(
        &mut self,
        document_id: DocumentId,
        confirmed_version: DocumentVersion,
        transaction_id: TransactionId,
    ) {
        if self.document_id.is_some() && self.document_id != Some(document_id) {
            return;
        }
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

    fn reject(&mut self, document_id: DocumentId, transaction_id: TransactionId) {
        if self.document_id.is_some() && self.document_id != Some(document_id) {
            return;
        }
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
        self.document_id = Some(snapshot.document_id);
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
    file_open_capability: Arc<Mutex<Option<String>>>,
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
                file_open_capability: Arc::new(Mutex::new(None)),
            },
            receiver,
        )
    }

    pub(crate) fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub fn from_sender(sender: mpsc::Sender<ClientMessage>) -> Self {
        Self {
            sender,
            client_id: 0,
            lease_id: None,
            sync_state: Arc::new(Mutex::new(ClientSyncState::new(0))),
            file_open_capability: Arc::new(Mutex::new(None)),
        }
    }

    pub fn enqueue_edit_event(
        &self,
        event: EditorEditEvent,
        transaction_id: TransactionId,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let recorder = global_recorder();
        let _scope = recorder.scope_with_metadata(
            "client.edit_queue.enqueue",
            MetricMetadata::document(event.document_id, event.base_version),
        );
        let operation = event.operation;
        let base_version = {
            let mut state = self.sync_state.lock().expect("client sync state poisoned");
            let base_version =
                state.reserve_pending(event.document_id, transaction_id, operation.clone());
            recorder.record_gauge(
                "client.edit_queue.pending_depth",
                state.pending.len() as u64,
            );
            base_version
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
            recorder.record_counter("client.edit_queue.enqueue_failed", 1);
            recorder.record_gauge(
                "client.edit_queue.pending_depth",
                state.pending.len() as u64,
            );
            return Err(error);
        }

        recorder.record_counter("client.edit_queue.enqueued", 1);
        Ok(())
    }

    pub(crate) fn enqueue_decoration_viewport_request(
        &self,
        document_id: DocumentId,
        document_version: DocumentVersion,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let message = ClientMessage::DecorationViewportRequest {
            client_id: self.client_id,
            document_id,
            document_version,
            byte_start,
            byte_end,
        };
        if self.sender.capacity() <= 1 {
            return Err(mpsc::error::TrySendError::Full(message));
        }
        self.sender.try_send(message)
    }

    pub fn enqueue_sdui_action(
        &self,
        ui_version: u64,
        intent: SduiActionIntent,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::SduiAction {
            client_id: self.client_id,
            ui_version,
            intent,
        })
    }

    pub(crate) fn enqueue_command_intent(
        &self,
        document_id: DocumentId,
        behavior_version: crate::protocol::BehaviorVersion,
        command_id: String,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::CommandIntent {
            client_id: self.client_id,
            document_id,
            behavior_version,
            command_id,
        })
    }

    /// Acknowledge a fully installed runtime generation after atomic client install.
    pub(crate) fn enqueue_runtime_generation_installed(
        &self,
        runtime_generation_id: crate::protocol::RuntimeGenerationId,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender
            .try_send(ClientMessage::RuntimeGenerationInstalled {
                client_id: self.client_id,
                runtime_generation_id,
            })
    }

    pub(crate) fn enqueue_completion_request(
        &self,
        event: EditorCompletionRequestEvent,
        request_id: CompletionRequestId,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let document_version = event.document_version.max(
            self.sync_state
                .lock()
                .expect("client sync state poisoned")
                .optimistic_version,
        );
        self.sender.try_send(ClientMessage::CompletionRequest {
            request: CompletionRequest {
                request_id,
                client_id: self.client_id,
                document_id: event.document_id,
                document_version,
                behavior_version: event.behavior_version,
                cursor_byte_offset: event.cursor_byte_offset,
                replacement_range: event.replacement_range,
                trigger: event.trigger,
                provider_generation: 0,
            },
        })
    }

    pub(crate) fn enqueue_language_intelligence_request(
        &self,
        event: EditorLanguageIntelligenceRequestEvent,
        request_id: LanguageIntelligenceRequestId,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender
            .try_send(ClientMessage::LanguageIntelligenceRequest {
                request: LanguageIntelligenceRequest {
                    request_id,
                    client_id: self.client_id,
                    document_id: event.document_id,
                    document_version: event.document_version,
                    behavior_version: event.behavior_version,
                    cursor_byte_offset: event.cursor_byte_offset,
                    feature: event.feature,
                    provider_generation: 0,
                },
            })
    }

    pub fn enqueue_open_selected_file(
        &self,
        selected_path: PathBuf,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let capability = self.take_selected_path_capability();
        self.sender.try_send(ClientMessage::OpenSelectedFile {
            client_id: self.client_id,
            capability,
            selected_path: selected_path.to_string_lossy().into_owned(),
        })
    }

    /// Notify the server that a document session is closed (explicit close or
    /// client LRU eviction), so server-side document state can be released
    /// when the last holder leaves. `force` discards unsaved edits.
    pub(crate) fn enqueue_close_document(
        &self,
        document_id: DocumentId,
        force: bool,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::CloseDocument {
            client_id: self.client_id,
            document_id,
            force,
        })
    }

    pub fn enqueue_add_selected_workspace_root(
        &self,
        selected_path: PathBuf,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let capability = self.take_selected_path_capability();
        self.sender
            .try_send(ClientMessage::AddSelectedWorkspaceRoot {
                client_id: self.client_id,
                capability,
                selected_path: selected_path.to_string_lossy().into_owned(),
            })
    }

    /// Request a server-first save for an open document. Never blocks paint.
    pub(crate) fn enqueue_save_document(
        &self,
        document_id: DocumentId,
        known_version: DocumentVersion,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::SaveDocument {
            client_id: self.client_id,
            document_id,
            known_version,
        })
    }

    /// Request a server-first reload. `force` discards dirty server text.
    pub(crate) fn enqueue_reload_document(
        &self,
        document_id: DocumentId,
        known_version: DocumentVersion,
        force: bool,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::ReloadDocument {
            client_id: self.client_id,
            document_id,
            known_version,
            force,
        })
    }

    /// Request a canonical document resync snapshot. Never blocks paint.
    pub(crate) fn enqueue_request_resync(
        &self,
        document_id: DocumentId,
        known_version: DocumentVersion,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::RequestResync {
            document_id,
            client_id: self.client_id,
            known_version,
        })
    }

    fn take_selected_path_capability(&self) -> String {
        // Take the pending server-issued capability token (single-use). If none
        // is available yet, send an empty capability; the server rejects it
        // with a typed diagnostic and re-issues a token for retry.
        self.file_open_capability
            .lock()
            .expect("client selected-path capability state poisoned")
            .take()
            .unwrap_or_default()
    }

    #[doc(hidden)]
    pub fn with_file_open_capability(self, capability: impl Into<String>) -> Self {
        *self
            .file_open_capability
            .lock()
            .expect("client file-open capability state poisoned") = Some(capability.into());
        self
    }

    pub fn sync_snapshot(&self) -> ClientSyncSnapshot {
        self.sync_state
            .lock()
            .expect("client sync state poisoned")
            .snapshot()
    }

    #[cfg(test)]
    pub(crate) fn acknowledge_for_test(
        &self,
        document_id: DocumentId,
        confirmed_version: DocumentVersion,
        transaction_id: TransactionId,
    ) {
        self.sync_state
            .lock()
            .expect("client sync state poisoned")
            .acknowledge(document_id, confirmed_version, transaction_id);
    }

    #[doc(hidden)]
    pub fn with_authority(mut self, client_id: ClientId, access: &DocumentAccess) -> Self {
        self.client_id = client_id;
        self.lease_id = access.lease_id();
        self
    }

    #[doc(hidden)]
    pub fn with_confirmed_version(mut self, confirmed_version: DocumentVersion) -> Self {
        self.sync_state = Arc::new(Mutex::new(ClientSyncState::new(confirmed_version)));
        self
    }

    #[doc(hidden)]
    pub fn update_opened_document_authority(
        &mut self,
        document_id: DocumentId,
        access: &DocumentAccess,
        confirmed_version: DocumentVersion,
    ) {
        self.lease_id = access.lease_id();
        *self.sync_state.lock().expect("client sync state poisoned") =
            ClientSyncState::for_document(Some(document_id), confirmed_version);
    }

    pub fn install_document_sync_state(
        &mut self,
        document_id: DocumentId,
        access: &DocumentAccess,
        confirmed_version: DocumentVersion,
        pending: Vec<PendingEdit>,
    ) {
        self.lease_id = access.lease_id();
        let mut state = ClientSyncState::for_document(Some(document_id), confirmed_version);
        for edit in pending {
            state.pending.push_back(edit);
        }
        if let Some(last) = state.pending.back() {
            state.optimistic_version = last.base_version.saturating_add(1);
        }
        *self.sync_state.lock().expect("client sync state poisoned") = state;
    }
}

#[derive(Debug)]
pub struct ClientSession {
    pub initial_state: ClientInitialState,
    pub edit_queue: ClientEditQueue,
    pub events: mpsc::Receiver<ClientConnectionEvent>,
}

#[derive(Debug, Clone, PartialEq)]
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
    ActiveTheme(crate::protocol::ActiveTheme),
    ActiveTypography(ActiveTypography),
    /// Phase 19 complete runtime-generation snapshot staged for atomic install.
    RuntimeStateSnapshot(Box<crate::protocol::RuntimeStateSnapshot>),
    ResyncSnapshot(ClientResyncSnapshot),
    DocumentOpened {
        metadata: DocumentMetadata,
        text: String,
    },
    DocumentSaved {
        document_id: DocumentId,
        version: DocumentVersion,
        dirty: bool,
    },
    DocumentClosed {
        document_id: DocumentId,
        closed: bool,
    },
    DocumentReloaded {
        metadata: DocumentMetadata,
        text: String,
    },
    FileOperationFailed {
        code: FileErrorCode,
        message: String,
        workspace_root_id: Option<WorkspaceRootId>,
        document_id: Option<DocumentId>,
    },
    SduiSnapshot {
        client_id: ClientId,
        tree: SduiTree,
    },
    SduiUpdate(SduiTreeUpdate),
    DecorationSet(DecorationSet),
    DecorationBatch(Vec<DecorationSet>),
    DiagnosticSet(DiagnosticSet),
    CompletionResult(CompletionResultSet),
    CompletionRejected {
        request_id: CompletionRequestId,
        reason: CompletionRejection,
    },
    LanguageIntelligenceResult(LanguageIntelligenceResult),
    LanguageIntelligenceRejected {
        request_id: LanguageIntelligenceRequestId,
        reason: LanguageIntelligenceRejection,
    },
    RuntimeDiagnostic(RuntimeDiagnostic),
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
    let file_open_capability = Arc::clone(&edit_queue.file_open_capability);
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
        file_open_capability,
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

    let active_theme = match codec.read_server_message(&mut *stream).await? {
        ServerMessage::ActiveTheme(theme) => theme,
        ServerMessage::Error { code, message } => {
            return Err(ClientBootstrapError::ServerError { code, message });
        }
        _ => {
            return Err(ClientBootstrapError::UnexpectedMessage(
                "expected ActiveTheme",
            ));
        }
    };

    let active_typography = match codec.read_server_message(&mut *stream).await? {
        ServerMessage::ActiveTypography(typography) if typography.validate().is_ok() => typography,
        ServerMessage::ActiveTypography(_) => {
            return Err(ClientBootstrapError::UnexpectedMessage(
                "invalid ActiveTypography",
            ));
        }
        ServerMessage::Error { code, message } => {
            return Err(ClientBootstrapError::ServerError { code, message });
        }
        _ => {
            return Err(ClientBootstrapError::UnexpectedMessage(
                "expected ActiveTypography",
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
        active_theme,
        active_typography,
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
            | EditRejection::InvalidBehaviorVersion { .. }
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "connection task entrypoint wires independent async channels and shared states explicitly"
)]
async fn run_connection<S>(
    stream: S,
    codec: Codec,
    mut outgoing_edits: mpsc::Receiver<ClientMessage>,
    events: mpsc::Sender<ClientConnectionEvent>,
    sync_state: Arc<Mutex<ClientSyncState>>,
    behavior_state: Arc<Mutex<behavior::ClientBehaviorState>>,
    file_open_capability: Arc<Mutex<Option<String>>>,
    client_id: ClientId,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut reader, mut writer) = tokio::io::split(stream);
    // Cancellation-safety: framed reads run in a dedicated pump task so a
    // winning select branch can never strand a partially-read frame
    // (`AsyncReadExt::read_exact` is not cancellation-safe). The loop below
    // selects only over channels; `writer` is the single owned write half.
    let (incoming_tx, mut incoming_rx) =
        mpsc::channel::<Result<ServerMessage, CodecError>>(EDIT_QUEUE_CAPACITY);
    let read_pump = tokio::spawn(async move {
        loop {
            match codec.read_server_message(&mut reader).await {
                Ok(message) => {
                    if incoming_tx.send(Ok(message)).await.is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = incoming_tx.send(Err(error)).await;
                    return;
                }
            }
        }
    });
    let _read_pump_guard = crate::protocol::codec::ReadPumpGuard::new(read_pump.abort_handle());

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
            incoming = incoming_rx.recv() => {
                let Some(incoming) = incoming else {
                    let _ = events.send(ClientConnectionEvent::Disconnected).await;
                    return;
                };
                match incoming {
                    Ok(ServerMessage::EditAck { document_id, confirmed_version, transaction_id }) => {
                        let recorder = global_recorder();
                        let _scope = recorder.scope_with_metadata(
                            "client.edit_ack.apply",
                            MetricMetadata::transaction(document_id, client_id, transaction_id, confirmed_version),
                        );
                        let pending_depth = {
                            let mut state = sync_state
                                .lock()
                                .expect("client sync state poisoned");
                            state.acknowledge(document_id, confirmed_version, transaction_id);
                            state.pending.len()
                        };
                        recorder.record_gauge("client.edit_queue.pending_depth", pending_depth as u64);
                        let _ = events.send(ClientConnectionEvent::EditAck { document_id, version: confirmed_version, transaction_id }).await;
                    }
                    Ok(ServerMessage::EditRejected { document_id, transaction_id, reason }) => {
                        let known_version = {
                            let mut state = sync_state
                                .lock()
                                .expect("client sync state poisoned");
                            state.reject(document_id, transaction_id);
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
                    Ok(ServerMessage::DocumentOpened { metadata, text }) => {
                        // Multi-document: do not wipe live sync state here. The
                        // editor widget retains the prior session and then
                        // installs authority for the newly active document.
                        let _ = events.send(ClientConnectionEvent::DocumentOpened { metadata, text }).await;
                    }
                    Ok(ServerMessage::DocumentSaved {
                        document_id,
                        version,
                        dirty,
                    }) => {
                        let _ = events
                            .send(ClientConnectionEvent::DocumentSaved {
                                document_id,
                                version,
                                dirty,
                            })
                            .await;
                    }
                    Ok(ServerMessage::DocumentClosed { document_id, closed }) => {
                        let _ = events
                            .send(ClientConnectionEvent::DocumentClosed { document_id, closed })
                            .await;
                    }
                    Ok(ServerMessage::DocumentReloaded { metadata, text }) => {
                        // Only rewrite live sync state when the reloaded document
                        // is the currently installed owner. Background sessions
                        // are updated by the editor widget without touching the
                        // active queue.
                        {
                            let mut state = sync_state
                                .lock()
                                .expect("client sync state poisoned");
                            if state.document_id.is_none()
                                || state.document_id == Some(metadata.document_id)
                            {
                                state.apply_resync_snapshot(ClientResyncSnapshot {
                                    document_id: metadata.document_id,
                                    version: metadata.version,
                                    text: text.clone(),
                                    access: metadata.access.clone(),
                                    lease_id: metadata.lease_id,
                                });
                            }
                        }
                        let _ = events
                            .send(ClientConnectionEvent::DocumentReloaded { metadata, text })
                            .await;
                    }
                    Ok(ServerMessage::FileOperationFailed { code, message, workspace_root_id, document_id }) => {
                        let _ = events.send(ClientConnectionEvent::FileOperationFailed { code, message, workspace_root_id, document_id }).await;
                    }
                    Ok(ServerMessage::SduiSnapshot { client_id, tree }) => {
                        let _ = events.send(ClientConnectionEvent::SduiSnapshot { client_id, tree }).await;
                    }
                    Ok(ServerMessage::FileOpenCapabilityIssued { token }) => {
                        // Store the latest single-use token; replaces any unused
                        // pending token so only the most recently issued one is
                        // valid.
                        *file_open_capability
                            .lock()
                            .expect("client file-open capability state poisoned") = Some(token);
                    }
                    Ok(ServerMessage::SduiUpdate { update }) => {
                        let _ = events.send(ClientConnectionEvent::SduiUpdate(update)).await;
                    }
                    Ok(ServerMessage::DecorationSet(set)) => {
                        let _ = events.send(ClientConnectionEvent::DecorationSet(set)).await;
                    }
                    Ok(ServerMessage::DecorationBatch(sets)) => {
                        let _ = events.send(ClientConnectionEvent::DecorationBatch(sets)).await;
                    }
                    Ok(ServerMessage::DiagnosticSet(set)) => {
                        let _ = events.send(ClientConnectionEvent::DiagnosticSet(set)).await;
                    }
                    Ok(ServerMessage::CompletionResult { result }) => {
                        let _ = events.send(ClientConnectionEvent::CompletionResult(result)).await;
                    }
                    Ok(ServerMessage::CompletionRejected { request_id, reason }) => {
                        let _ = events.send(ClientConnectionEvent::CompletionRejected { request_id, reason }).await;
                    }
                    Ok(ServerMessage::LanguageIntelligenceResult { result }) => {
                        let _ = events.send(ClientConnectionEvent::LanguageIntelligenceResult(result)).await;
                    }
                    Ok(ServerMessage::LanguageIntelligenceRejected { request_id, reason }) => {
                        let _ = events.send(ClientConnectionEvent::LanguageIntelligenceRejected { request_id, reason }).await;
                    }
                    Ok(ServerMessage::RuntimeDiagnostic(diagnostic)) => {
                        let _ = events.send(ClientConnectionEvent::RuntimeDiagnostic(diagnostic)).await;
                    }
                    Ok(message @ ServerMessage::EditTransaction { .. }) => {
                        let _ = events.send(ClientConnectionEvent::EditTransaction(message)).await;
                    }
                    Ok(ServerMessage::ActiveTheme(theme)) => {
                        let _ = events.send(ClientConnectionEvent::ActiveTheme(theme)).await;
                    }
                    Ok(ServerMessage::ActiveTypography(typography)) if typography.validate().is_ok() => {
                        let _ = events.send(ClientConnectionEvent::ActiveTypography(typography)).await;
                    }
                    Ok(ServerMessage::ActiveTypography(_)) => {}
                    Ok(ServerMessage::RuntimeStateSnapshot(snapshot)) => {
                        // Protocol-level gate only. Full candidate validation and
                        // atomic install happen in the editor; acknowledgement is
                        // sent only after that install succeeds.
                        if snapshot.client_id != client_id || snapshot.validate().is_err() {
                            let _ = events
                                .send(ClientConnectionEvent::ConnectionError(
                                    "invalid runtime state snapshot".to_string(),
                                ))
                                .await;
                            return;
                        }
                        let _ = events
                            .send(ClientConnectionEvent::RuntimeStateSnapshot(snapshot))
                            .await;
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
    use std::path::PathBuf;
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
    use crate::editor::{EditorCompletionRequestEvent, EditorEditEvent};
    #[cfg(any(unix, windows))]
    use crate::ipc::IpcEndpoint;
    #[cfg(any(unix, windows))]
    use crate::protocol::EditRejection;
    use crate::protocol::{
        ActiveTypography, BehaviorManifest, ClientMessage, CommandDeclaration,
        CompletionReplacementRange, CompletionTrigger, DocumentAccess, EditOperation,
        FileErrorCode, PROTOCOL_VERSION, RuntimeDiagnostic, SduiActionIntent, SduiActionSource,
        SduiEditorBinding, SduiNode, SduiNodeId, SduiNodeKind, SduiTree, ServerMessage,
        codec::Codec,
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

    #[test]
    fn invalid_behavior_version_rejection_requests_resync() {
        assert!(super::rejection_requests_resync(
            &crate::protocol::EditRejection::InvalidBehaviorVersion {
                behavior_version: 1,
                server_behavior_version: 2,
            }
        ));
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
        assert_eq!(state.active_typography, ActiveTypography::default());
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
    async fn completion_request_is_enqueued_as_non_blocking_message() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_authority(42, &DocumentAccess::Editable { lease_id: 1 });

        queue
            .enqueue_completion_request(
                EditorCompletionRequestEvent {
                    document_id: 4,
                    document_version: 5,
                    behavior_version: 6,
                    cursor_byte_offset: 9,
                    replacement_range: CompletionReplacementRange::new(7, 9),
                    trigger: CompletionTrigger::Manual,
                },
                11,
            )
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::CompletionRequest {
                request: crate::protocol::CompletionRequest {
                    request_id: 11,
                    client_id: 42,
                    document_id: 4,
                    document_version: 5,
                    behavior_version: 6,
                    cursor_byte_offset: 9,
                    replacement_range: CompletionReplacementRange::new(7, 9),
                    trigger: CompletionTrigger::Manual,
                    provider_generation: 0,
                }
            }
        );
    }

    #[tokio::test]
    async fn completion_after_local_edit_uses_optimistic_document_version() {
        let (queue, mut receiver) = ClientEditQueue::bounded(2);
        let queue = queue
            .with_authority(42, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(5);

        queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: 4,
                    base_version: 5,
                    behavior_version: 6,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "p".to_string(),
                    },
                },
                10,
            )
            .unwrap();
        queue
            .enqueue_completion_request(
                EditorCompletionRequestEvent {
                    document_id: 4,
                    document_version: 5,
                    behavior_version: 6,
                    cursor_byte_offset: 1,
                    replacement_range: CompletionReplacementRange::new(0, 1),
                    trigger: CompletionTrigger::Manual,
                },
                11,
            )
            .unwrap();

        assert!(matches!(
            receiver.recv().await,
            Some(ClientMessage::Edit { .. })
        ));
        let Some(ClientMessage::CompletionRequest { request }) = receiver.recv().await else {
            panic!("expected completion request after edit");
        };
        assert_eq!(request.document_version, 6);
    }

    #[tokio::test]
    async fn language_intelligence_request_is_enqueued_as_non_blocking_message() {
        use crate::editor::EditorLanguageIntelligenceRequestEvent;
        use crate::protocol::LanguageIntelligenceFeature;

        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_authority(42, &DocumentAccess::Editable { lease_id: 1 });

        queue
            .enqueue_language_intelligence_request(
                EditorLanguageIntelligenceRequestEvent {
                    document_id: 4,
                    document_version: 5,
                    behavior_version: 6,
                    cursor_byte_offset: 9,
                    feature: LanguageIntelligenceFeature::Hover,
                },
                13,
            )
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::LanguageIntelligenceRequest {
                request: crate::protocol::LanguageIntelligenceRequest {
                    request_id: 13,
                    client_id: 42,
                    document_id: 4,
                    document_version: 5,
                    behavior_version: 6,
                    cursor_byte_offset: 9,
                    feature: LanguageIntelligenceFeature::Hover,
                    provider_generation: 0,
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

    #[test]
    fn opened_document_reset_keeps_connection_and_editor_sync_state_shared() {
        let (mut editor_queue, _receiver) = ClientEditQueue::bounded(1);
        let connection_queue = editor_queue.clone();

        editor_queue.update_opened_document_authority(
            42,
            &DocumentAccess::Editable { lease_id: 8 },
            5,
        );

        assert_eq!(connection_queue.sync_snapshot().confirmed_version, 5);
        assert_eq!(connection_queue.sync_snapshot().optimistic_version, 5);
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
    async fn decoration_viewport_request_emits_bounded_range_metadata() {
        let (queue, mut receiver) = ClientEditQueue::bounded(2);
        let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);

        queue
            .enqueue_decoration_viewport_request(7, 3, 1_024, 2_048)
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::DecorationViewportRequest {
                client_id: 42,
                document_id: 7,
                document_version: 3,
                byte_start: 1_024,
                byte_end: 2_048,
            }
        );
    }

    #[tokio::test]
    async fn viewport_requests_reserve_queue_capacity_for_workspace_actions() {
        let (queue, mut receiver) = ClientEditQueue::bounded(2);
        let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);

        queue
            .enqueue_decoration_viewport_request(7, 3, 1_024, 2_048)
            .unwrap();
        assert!(matches!(
            queue.enqueue_decoration_viewport_request(7, 3, 2_048, 3_072),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_))
        ));
        queue
            .enqueue_sdui_action(
                1,
                SduiActionIntent {
                    command_id: "clay.workspace.openDirectory".to_string(),
                    source: SduiActionSource::Button {
                        node_id: SduiNodeId(5),
                    },
                    arguments: Vec::new(),
                },
            )
            .unwrap();

        assert!(matches!(
            receiver.recv().await.unwrap(),
            ClientMessage::DecorationViewportRequest { .. }
        ));
        assert!(matches!(
            receiver.recv().await.unwrap(),
            ClientMessage::SduiAction { .. }
        ));
    }

    #[tokio::test]
    async fn fragmented_frame_survives_concurrent_outgoing_message() {
        use std::time::Duration;
        use tokio::io::AsyncWriteExt;

        let codec = Codec::default();
        let (client_end, mut server_end) = duplex(64 * 1024);

        let (edit_queue, outgoing) = ClientEditQueue::bounded(super::EDIT_QUEUE_CAPACITY);
        let edit_queue = edit_queue.with_authority(7, &DocumentAccess::ReadOnly);
        let sync_state = std::sync::Arc::clone(&edit_queue.sync_state);
        let file_open_capability = std::sync::Arc::clone(&edit_queue.file_open_capability);
        let behavior_state = std::sync::Arc::new(std::sync::Mutex::new(
            super::behavior::ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1))
                .unwrap(),
        ));
        let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(super::EDIT_QUEUE_CAPACITY);
        let connection = tokio::spawn(super::run_connection(
            client_end,
            codec,
            outgoing,
            events_tx,
            sync_state,
            behavior_state,
            file_open_capability,
            7,
        ));

        let provenance = crate::protocol::DecorationProvenance {
            package_name: "@clay/markdown".to_string(),
            package_version: "builtin".to_string(),
            package_prefix: "markdown".to_string(),
        };
        let spans = (0..256)
            .map(|index| {
                crate::protocol::DecorationSpan::from_vocabulary(
                    index * 16,
                    index * 16 + 8,
                    crate::protocol::DecorationKind::Syntax,
                    crate::protocol::TokenType::Paragraph,
                    crate::protocol::Modifiers::NONE,
                    70,
                    provenance.clone(),
                )
            })
            .collect();
        let set = crate::protocol::DecorationSet {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: crate::protocol::DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 4096,
            spans,
        };
        let frame = codec
            .encode_server_message(&ServerMessage::DecorationSet(set))
            .unwrap();

        // Drip-feed the frame start, then win the select race with an outgoing
        // message before the rest of the frame arrives. The read pump must keep
        // frame alignment regardless of the interleaving.
        let split = 6;
        server_end.write_all(&frame[..split]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        edit_queue
            .enqueue_decoration_viewport_request(1, 1, 0, 4096)
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        server_end.write_all(&frame[split..]).await.unwrap();
        codec
            .write_server_message(
                &mut server_end,
                &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                    specifier: "@clay/default".to_string(),
                    overrides: Vec::new(),
                }),
            )
            .await
            .unwrap();

        let first = tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(first, ClientConnectionEvent::DecorationSet(_)),
            "fragmented frame must decode as one DecorationSet, got {first:?}"
        );
        let second = tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(second, ClientConnectionEvent::ActiveTheme(_)),
            "next frame must stay aligned, got {second:?}"
        );

        drop(edit_queue);
        connection.await.unwrap();
    }

    #[tokio::test]
    async fn decoration_batch_frame_dispatches_single_event() {
        use std::time::Duration;

        let codec = Codec::default();
        let (client_end, mut server_end) = duplex(64 * 1024);

        let (edit_queue, outgoing) = ClientEditQueue::bounded(super::EDIT_QUEUE_CAPACITY);
        let edit_queue = edit_queue.with_authority(7, &DocumentAccess::ReadOnly);
        let sync_state = std::sync::Arc::clone(&edit_queue.sync_state);
        let file_open_capability = std::sync::Arc::clone(&edit_queue.file_open_capability);
        let behavior_state = std::sync::Arc::new(std::sync::Mutex::new(
            super::behavior::ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1))
                .unwrap(),
        ));
        let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(super::EDIT_QUEUE_CAPACITY);
        let connection = tokio::spawn(super::run_connection(
            client_end,
            codec,
            outgoing,
            events_tx,
            sync_state,
            behavior_state,
            file_open_capability,
            7,
        ));

        let provenance = crate::protocol::DecorationProvenance {
            package_name: "@clay/markdown".to_string(),
            package_version: "builtin".to_string(),
            package_prefix: "markdown".to_string(),
        };
        let chunk = |start: u64| crate::protocol::DecorationSet {
            document_id: 1,
            document_version: 2,
            package_prefix: "markdown".to_string(),
            kind: crate::protocol::DecorationKind::Syntax,
            viewport_byte_start: start,
            viewport_byte_end: start + 128,
            spans: vec![crate::protocol::DecorationSpan::from_vocabulary(
                start,
                start + 8,
                crate::protocol::DecorationKind::Syntax,
                crate::protocol::TokenType::Paragraph,
                crate::protocol::Modifiers::NONE,
                70,
                provenance.clone(),
            )],
        };
        codec
            .write_server_message(
                &mut server_end,
                &ServerMessage::DecorationBatch(vec![chunk(0), chunk(128), chunk(256)]),
            )
            .await
            .unwrap();

        let event = tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let ClientConnectionEvent::DecorationBatch(sets) = event else {
            panic!("batch frame must dispatch one batch event, got {event:?}");
        };
        assert_eq!(sets.len(), 3);
        assert!(
            sets.windows(2)
                .all(|pair| pair[0].viewport_byte_start < pair[1].viewport_byte_start),
            "chunk order preserved"
        );

        drop(edit_queue);
        connection.await.unwrap();
    }

    #[tokio::test]
    async fn selected_file_open_request_emits_non_edit_message() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue
            .with_authority(42, &DocumentAccess::ReadOnly)
            .with_file_open_capability("foc-test-token");
        let selected_path = PathBuf::from("C:/Users/test/Documents/note.md");

        queue
            .enqueue_open_selected_file(selected_path.clone())
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::OpenSelectedFile {
                client_id: 42,
                capability: "foc-test-token".to_string(),
                selected_path: selected_path.to_string_lossy().into_owned(),
            }
        );
    }

    #[tokio::test]
    async fn selected_folder_root_request_emits_non_edit_message() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue
            .with_authority(42, &DocumentAccess::ReadOnly)
            .with_file_open_capability("folder-token");
        let selected_path = PathBuf::from("/home/test/project");

        queue
            .enqueue_add_selected_workspace_root(selected_path.clone())
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::AddSelectedWorkspaceRoot {
                client_id: 42,
                capability: "folder-token".to_string(),
                selected_path: selected_path.to_string_lossy().into_owned(),
            }
        );
    }

    #[tokio::test]
    async fn selected_file_open_without_capability_sends_empty_token() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);
        let selected_path = PathBuf::from("C:/Users/test/Documents/note.md");

        queue
            .enqueue_open_selected_file(selected_path.clone())
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::OpenSelectedFile {
                client_id: 42,
                capability: String::new(),
                selected_path: selected_path.to_string_lossy().into_owned(),
            }
        );
    }

    #[tokio::test]
    async fn sdui_button_action_emits_server_intent() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);
        let intent = SduiActionIntent::command(
            "workspace.refresh",
            SduiActionSource::Button {
                node_id: SduiNodeId(5),
            },
        );

        queue.enqueue_sdui_action(3, intent.clone()).unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::SduiAction {
                client_id: 42,
                ui_version: 3,
                intent,
            }
        );
    }

    #[tokio::test]
    async fn server_keybinding_emits_bounded_command_intent() {
        let (queue, mut receiver) = ClientEditQueue::bounded(1);
        let queue = queue.with_authority(42, &DocumentAccess::ReadOnly);

        queue
            .enqueue_command_intent(7, 3, "clay.controlCenter.open".to_string())
            .unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            ClientMessage::CommandIntent {
                client_id: 42,
                document_id: 7,
                behavior_version: 3,
                command_id: "clay.controlCenter.open".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn command_intent_hot_path_uses_try_send_backpressure() {
        let (queue, _receiver) = ClientEditQueue::bounded(1);
        queue
            .enqueue_command_intent(7, 3, "clay.controlCenter.open".to_string())
            .unwrap();

        let error = queue
            .enqueue_command_intent(7, 3, "clay.controlCenter.open".to_string())
            .unwrap_err();

        assert!(matches!(
            error,
            tokio::sync::mpsc::error::TrySendError::Full(_)
        ));
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
                )
                .await
                .unwrap();
        });

        let session = connect_from_stream(client, codec).await.unwrap();

        assert_eq!(session.initial_state.behavior_manifest.behavior_version, 44);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_receives_sdui_snapshot_event() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let tree = SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![SduiNode::new(
                SduiNodeId(1),
                SduiNodeKind::EditorView {
                    binding: SduiEditorBinding {
                        document_id: 7,
                        expected_version: Some(10),
                    },
                },
            )],
        };
        let expected_tree = tree.clone();
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::SduiSnapshot { client_id: 1, tree },
                )
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::SduiSnapshot {
                client_id: 1,
                tree: expected_tree,
            }
        );
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_forwards_document_opened_without_replacing_live_sync_state() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let metadata = crate::protocol::DocumentMetadata {
            document_id: 42,
            version: 5,
            access: DocumentAccess::Editable { lease_id: 8 },
            lease_id: Some(8),
            dirty: false,
            workspace_root_id: 77,
            path: "note.md".to_string(),
        };
        let expected_metadata = metadata.clone();
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
                        text: "scratch".to_string(),
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
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::DocumentOpened {
                        metadata,
                        text: "# opened\n".to_string(),
                    },
                )
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::DocumentOpened {
                metadata: expected_metadata,
                text: "# opened\n".to_string(),
            }
        );
        // Multi-document: connection layer forwards DocumentOpened only. The
        // editor widget retains the prior session and installs authority for
        // the newly active document, so live sync stays on the initial doc
        // until that install runs.
        let snapshot = session.edit_queue.sync_snapshot();
        assert_eq!(snapshot.confirmed_version, 3);
        assert_eq!(snapshot.optimistic_version, 3);
        assert!(snapshot.pending.is_empty());
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_receives_file_operation_failed_event() {
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
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::FileOperationFailed {
                        code: FileErrorCode::InvalidUtf8,
                        message: "workspace file <requested path> is not valid UTF-8 text"
                            .to_string(),
                        workspace_root_id: None,
                        document_id: None,
                    },
                )
                .await
                .unwrap();
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();

        assert!(matches!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::FileOperationFailed {
                code: FileErrorCode::InvalidUtf8,
                workspace_root_id: None,
                document_id: None,
                ..
            }
        ));
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn client_receives_runtime_diagnostic_event() {
        let (client, mut server) = duplex(4096);
        let codec = Codec::default();
        let expected = RuntimeDiagnostic::error(
            "clay.runtime.syntax_error",
            "JavaScript syntax error while evaluating server-side configuration.",
        );
        let mut live_typography = ActiveTypography {
            revision: 1,
            ..ActiveTypography::default()
        };
        live_typography.ui.size = 13.0;
        let server_task = tokio::spawn({
            let expected = expected.clone();
            let live_typography = live_typography.clone();
            async move {
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
                        &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                            specifier: "@clay/default".to_string(),
                            overrides: Vec::new(),
                        }),
                    )
                    .await
                    .unwrap();
                codec
                    .write_server_message(
                        &mut server,
                        &ServerMessage::ActiveTypography(ActiveTypography::default()),
                    )
                    .await
                    .unwrap();
                codec
                    .write_server_message(
                        &mut server,
                        &ServerMessage::ActiveTypography(live_typography),
                    )
                    .await
                    .unwrap();
                codec
                    .write_server_message(&mut server, &ServerMessage::RuntimeDiagnostic(expected))
                    .await
                    .unwrap();
            }
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();

        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::ActiveTypography(live_typography)
        );
        assert_eq!(
            session.events.recv().await.unwrap(),
            ClientConnectionEvent::RuntimeDiagnostic(expected)
        );
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
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            },
            active_typography: ActiveTypography::default(),
            sdui_tree: SduiTree {
                ui_version: generation,
                root_id: SduiNodeId(1),
                nodes: vec![SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Label {
                        text: format!("gen-{generation}"),
                    },
                )],
            },
            package_ui: crate::protocol::PackageUiSnapshot {
                version: generation,
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
                    text: String::new(),
                    access: DocumentAccess::Editable { lease_id: 1 },
                    lease_id: Some(1),
                },
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                server,
                &ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1)),
            )
            .await
            .unwrap();
        codec
            .write_server_message(
                server,
                &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                    specifier: "@clay/default".to_string(),
                    overrides: Vec::new(),
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

    #[tokio::test]
    async fn runtime_state_snapshot_is_staged_without_immediate_ack() {
        let (client, mut server) = duplex(8192);
        let codec = Codec::default();
        let snapshot = sample_runtime_snapshot(2, 1);
        let server_task = tokio::spawn({
            let snapshot = snapshot.clone();
            async move {
                write_minimal_bootstrap(&codec, &mut server, 1).await;
                codec
                    .write_server_message(
                        &mut server,
                        &ServerMessage::RuntimeStateSnapshot(Box::new(snapshot)),
                    )
                    .await
                    .unwrap();
                // Receive loop must not acknowledge before editor install.
                match tokio::time::timeout(
                    std::time::Duration::from_millis(50),
                    codec.read_client_message(&mut server),
                )
                .await
                {
                    Err(_) => {}
                    Ok(Ok(message)) => {
                        panic!("unexpected client message before install: {message:?}")
                    }
                    Ok(Err(_)) => {}
                }
            }
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();
        match session.events.recv().await.unwrap() {
            ClientConnectionEvent::RuntimeStateSnapshot(received) => {
                assert_eq!(received.runtime_generation_id, 2);
                assert_eq!(received.client_id, 1);
            }
            other => panic!("expected staged runtime snapshot, got {other:?}"),
        }
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn invalid_runtime_state_snapshot_fail_closes_without_ack_or_event() {
        let (client, mut server) = duplex(8192);
        let codec = Codec::default();
        let server_task = tokio::spawn({
            async move {
                write_minimal_bootstrap(&codec, &mut server, 1).await;
                let mut invalid = sample_runtime_snapshot(2, 1);
                invalid.behavior.manifest_id.clear();
                codec
                    .write_server_message(
                        &mut server,
                        &ServerMessage::RuntimeStateSnapshot(Box::new(invalid)),
                    )
                    .await
                    .unwrap();
                match tokio::time::timeout(
                    std::time::Duration::from_millis(50),
                    codec.read_client_message(&mut server),
                )
                .await
                {
                    Err(_) => {}
                    Ok(Ok(message)) => panic!("invalid snapshot must not acknowledge: {message:?}"),
                    Ok(Err(_)) => {}
                }
            }
        });

        let mut session = connect_from_stream(client, codec).await.unwrap();
        match session.events.recv().await.unwrap() {
            ClientConnectionEvent::ConnectionError(message) => {
                assert!(message.contains("invalid runtime state snapshot"));
            }
            other => panic!("expected fail-closed connection error, got {other:?}"),
        }
        assert!(session.events.recv().await.is_none());
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
                )
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

        let mut event = session.events.recv().await.unwrap();
        if matches!(event, ClientConnectionEvent::SduiSnapshot { .. }) {
            event = session.events.recv().await.unwrap();
        }

        assert_eq!(
            event,
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

    #[cfg(unix)]
    #[tokio::test]
    async fn selected_file_edit_then_save_persists_and_reports_clean() {
        let socket_path = unique_socket_path("selected-save");
        let file_path = socket_path.parent().unwrap().join("save.rs");
        let config_root = socket_path.parent().unwrap().join("config");
        fs::create_dir(&config_root).unwrap();
        fs::write(&file_path, "fn main() {}\n").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
import { bindKey } from "clay:keybindings";
bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
"#,
        )
        .unwrap();
        let mut config = ServerConfig::new(&socket_path);
        config.configuration_root = Some(config_root.clone());
        let server = IpcServer::new(config);
        let server_task = tokio::spawn(server.run());

        let mut session = connect_with_retry(&socket_path).await;
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if session
                    .edit_queue
                    .file_open_capability
                    .lock()
                    .unwrap()
                    .is_some()
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("file-open capability timed out");
        session
            .edit_queue
            .enqueue_open_selected_file(file_path.clone())
            .unwrap();

        let (metadata, behavior_manifest) =
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                let metadata = loop {
                    let event = session.events.recv().await.unwrap();
                    if let ClientConnectionEvent::DocumentOpened { metadata, .. } = event {
                        break metadata;
                    }
                };
                let behavior_manifest = loop {
                    let event = session.events.recv().await.unwrap();
                    if let ClientConnectionEvent::BehaviorManifestInstalled { manifest, .. } = event
                    {
                        break manifest;
                    }
                };
                (metadata, behavior_manifest)
            })
            .await
            .expect("selected file open timed out");
        assert!(
            behavior_manifest
                .keymaps
                .iter()
                .any(|rule| { rule.command_id == "clay.documents.serverSaveDocument" }),
            "selected-file activation must preserve configured save binding"
        );
        let behavior_version = behavior_manifest.behavior_version;
        session.edit_queue.update_opened_document_authority(
            metadata.document_id,
            &metadata.access,
            metadata.version,
        );
        let inserted = "// saved\n";
        session
            .edit_queue
            .enqueue_edit_event(
                EditorEditEvent {
                    document_id: metadata.document_id,
                    base_version: metadata.version,
                    behavior_version,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: inserted.to_string(),
                    },
                },
                91,
            )
            .unwrap();
        session
            .edit_queue
            .enqueue_save_document(metadata.document_id, metadata.version + 1)
            .unwrap();

        let saved = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let event = session.events.recv().await.unwrap();
                if let ClientConnectionEvent::DocumentSaved {
                    document_id,
                    version,
                    dirty,
                } = event
                {
                    break (document_id, version, dirty);
                }
            }
        })
        .await
        .expect("selected file save timed out");

        assert_eq!(saved, (metadata.document_id, metadata.version + 1, false));
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            format!("{inserted}fn main() {{}}\n")
        );

        server_task.abort();
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_file(config_root.join("init.js"));
        let _ = fs::remove_dir(&config_root);
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
        // The ambient default configuration (e.g. ~/.config/clay/init.js) may
        // publish a behavior manifest, so the exact version is not fixed.
        assert!(
            session.initial_state.behavior_manifest.behavior_version >= 1,
            "expected a loaded behavior manifest, got version {}",
            session.initial_state.behavior_manifest.behavior_version
        );

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

        let mut event = session.events.recv().await.unwrap();
        if matches!(event, ClientConnectionEvent::SduiSnapshot { .. }) {
            event = session.events.recv().await.unwrap();
        }

        assert_eq!(
            event,
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
        let server_behavior_version = match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
            message => panic!("expected BehaviorManifest, got {message:?}"),
        };
        // The server may also send an SDUI snapshot, runtime diagnostics, and a
        // post-handshake FileOpenCapabilityIssued token before entering the
        // request loop. We send the edit and then skip those messages when
        // reading the response.

        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Edit {
                    document_id,
                    client_id,
                    lease_id: Some(lease_id),
                    base_version: version - 1,
                    behavior_version: server_behavior_version,
                    transaction_id: 99,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "stale".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        let reason = loop {
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::EditRejected {
                    document_id: rejected_document_id,
                    transaction_id: 99,
                    reason,
                } if rejected_document_id == document_id => break reason,
                ServerMessage::SduiSnapshot { .. }
                | ServerMessage::ActiveTheme(_)
                | ServerMessage::ActiveTypography(_)
                | ServerMessage::FileOpenCapabilityIssued { .. }
                | ServerMessage::RuntimeDiagnostic(_) => continue,
                message => panic!("expected EditRejected, got {message:?}"),
            }
        };

        assert_eq!(
            reason,
            EditRejection::StaleVersion {
                client_base_version: version - 1,
                server_version: version,
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
        let server_behavior_version = match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
            message => panic!("expected BehaviorManifest, got {message:?}"),
        };
        // The server may also send an SDUI snapshot, runtime diagnostics, and a
        // post-handshake FileOpenCapabilityIssued token before entering the
        // request loop. We send the edit and then skip those messages when
        // reading the response.

        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Edit {
                    document_id,
                    client_id,
                    lease_id: Some(lease_id),
                    base_version: version - 1,
                    behavior_version: server_behavior_version,
                    transaction_id: 99,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "stale".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        let reason = loop {
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::EditRejected {
                    document_id: rejected_document_id,
                    transaction_id: 99,
                    reason,
                } if rejected_document_id == document_id => break reason,
                ServerMessage::SduiSnapshot { .. }
                | ServerMessage::ActiveTheme(_)
                | ServerMessage::ActiveTypography(_)
                | ServerMessage::FileOpenCapabilityIssued { .. }
                | ServerMessage::RuntimeDiagnostic(_) => continue,
                message => panic!("expected EditRejected, got {message:?}"),
            }
        };

        assert_eq!(
            reason,
            EditRejection::StaleVersion {
                client_base_version: version - 1,
                server_version: version,
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
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTheme(crate::protocol::ActiveTheme {
                        specifier: "@clay/default".to_string(),
                        overrides: Vec::new(),
                    }),
                )
                .await
                .unwrap();
            codec
                .write_server_message(
                    &mut server,
                    &ServerMessage::ActiveTypography(ActiveTypography::default()),
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
