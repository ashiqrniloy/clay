pub(crate) mod behavior;

pub use behavior::ClientUiCommandRoute;
pub use behavior::language_intelligence_feature_for_command;
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::ipc::IpcEndpoint;
use crate::perf::{
    budgets::{COMPLETION_RECENCY_MAX_ITEM_CHARS, COMPLETION_RECENCY_MAX_ITEMS},
    metrics::{
        BRIDGE_CLIENT_DELIVERY, BRIDGE_SERVER_DELIVERY, MetricMetadata, MetricValue,
        global_recorder,
    },
};
use crate::protocol::{
    ActiveTypography, BehaviorManifest, BehaviorScope, BehaviorVersion, CaretStyle, ClientId,
    ClientMessage, CompletionRejection, CompletionRequest, CompletionRequestId,
    CompletionResultSet, DecorationSet, DiagnosticSet, DocumentAccess, DocumentChunkRejection,
    DocumentId, DocumentMetadata, DocumentTextHead, DocumentVersion, EditOperation, EditRejection,
    EditorCommandRequest, FileErrorCode, LanguageIntelligenceRejection,
    LanguageIntelligenceRequest, LanguageIntelligenceRequestId, LanguageIntelligenceResult,
    PROTOCOL_VERSION, ProtocolErrorCode, RuntimeDiagnostic, SduiActionIntent, SduiTree,
    SduiTreeUpdate, SelectionQueryRequest, SelectionQueryResult, ServerMessage, ShellPreferences,
    TabCommand, TabId, TabRegistrySnapshot, TransactionId, WorkspaceRootId,
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
pub struct EditorEditEvent {
    pub document_id: DocumentId,
    pub base_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub operation: EditOperation,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorCompletionRequestEvent {
    pub(crate) document_id: DocumentId,
    pub(crate) document_version: DocumentVersion,
    pub(crate) behavior_version: BehaviorVersion,
    pub(crate) cursor_byte_offset: u64,
    pub(crate) replacement_range: crate::protocol::CompletionReplacementRange,
    pub(crate) trigger: crate::protocol::CompletionTrigger,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorLanguageIntelligenceRequestEvent {
    pub(crate) document_id: DocumentId,
    pub(crate) document_version: DocumentVersion,
    pub(crate) behavior_version: BehaviorVersion,
    pub(crate) cursor_byte_offset: u64,
    pub(crate) feature: crate::protocol::LanguageIntelligenceFeature,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorSelectionQueryRequestEvent {
    pub(crate) document_id: DocumentId,
    pub(crate) document_version: DocumentVersion,
    pub(crate) behavior_version: BehaviorVersion,
    pub(crate) query: crate::protocol::SelectionQuery,
    pub(crate) selections: Vec<crate::protocol::SelectionQueryCursor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientInitialState {
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub head: DocumentTextHead,
    pub access: DocumentAccess,
    pub behavior_manifest: BehaviorManifest,
    pub active_theme: crate::protocol::ActiveTheme,
    pub active_typography: ActiveTypography,
    /// Workspace root path for the bound initial document. The client binds
    /// with `TabCommand::New`/`Reclaim` before receiving this state.
    pub workspace_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEdit {
    pub document_id: DocumentId,
    pub base_version: DocumentVersion,
    pub transaction_id: TransactionId,
    pub operation: EditOperation,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientResyncSnapshot {
    pub document_id: DocumentId,
    pub version: DocumentVersion,
    pub head: DocumentTextHead,
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
    /// Per-document live optimistic-version tracking (Phase 22.2). Every pane's
    /// document tracks its own confirmed/optimistic version and pending-edit
    /// queue concurrently; acks/rejects route by `document_id`. All
    /// `ClientEditQueue` clones (one per pane view plus the connection loop)
    /// share this state, so edits from any pane reserve against the right
    /// document's base version.
    documents: HashMap<DocumentId, DocumentSyncState>,
    /// Confirmed version assumed for documents reserved before any authority
    /// install (test bootstrap path; production opens install per-document).
    default_confirmed_version: DocumentVersion,
    /// Most recently touched document (compat surface for `snapshot()`).
    last_touched: Option<DocumentId>,
}

#[derive(Debug)]
struct DocumentSyncState {
    confirmed_version: DocumentVersion,
    optimistic_version: DocumentVersion,
    pending: VecDeque<PendingEdit>,
    last_resync: Option<ClientResyncSnapshot>,
    /// Lease of the open access for this document; sent with outgoing edits.
    /// Stored per document because the server leases per (client, document).
    lease_id: Option<crate::protocol::LeaseId>,
}

impl DocumentSyncState {
    fn new(confirmed_version: DocumentVersion, lease_id: Option<crate::protocol::LeaseId>) -> Self {
        Self {
            confirmed_version,
            optimistic_version: confirmed_version,
            pending: VecDeque::new(),
            last_resync: None,
            lease_id,
        }
    }
}

impl ClientSyncState {
    fn new(confirmed_version: DocumentVersion) -> Self {
        Self {
            documents: HashMap::new(),
            default_confirmed_version: confirmed_version,
            last_touched: None,
        }
    }

    /// Get-or-create the tracking state for `document_id`, marking it most
    /// recently touched.
    fn entry(&mut self, document_id: DocumentId) -> &mut DocumentSyncState {
        self.last_touched = Some(document_id);
        self.documents
            .entry(document_id)
            .or_insert_with(|| DocumentSyncState::new(self.default_confirmed_version, None))
    }

    fn reserve_pending(
        &mut self,
        document_id: DocumentId,
        transaction_id: TransactionId,
        operation: EditOperation,
    ) -> DocumentVersion {
        let state = self.entry(document_id);
        let base_version = state.optimistic_version;
        state.optimistic_version = state.optimistic_version.saturating_add(1);
        state.pending.push_back(PendingEdit {
            document_id,
            base_version,
            transaction_id,
            operation,
        });
        base_version
    }

    fn rollback_pending_reservation(
        &mut self,
        document_id: DocumentId,
        transaction_id: TransactionId,
    ) {
        if let Some(state) = self.documents.get_mut(&document_id)
            && let Some(position) = state
                .pending
                .iter()
                .position(|pending| pending.transaction_id == transaction_id)
        {
            state.pending.remove(position);
            state.optimistic_version = state
                .pending
                .back()
                .map_or(state.confirmed_version, |pending| pending.base_version + 1);
        }
    }

    fn acknowledge(
        &mut self,
        document_id: DocumentId,
        confirmed_version: DocumentVersion,
        transaction_id: TransactionId,
    ) {
        if let Some(state) = self.documents.get_mut(&document_id) {
            self.last_touched = Some(document_id);
            state.confirmed_version = confirmed_version;
            if let Some(position) = state
                .pending
                .iter()
                .position(|pending| pending.transaction_id == transaction_id)
            {
                state.pending.remove(position);
            }
            if state.optimistic_version < confirmed_version {
                state.optimistic_version = confirmed_version;
            }
        }
    }

    fn reject(&mut self, document_id: DocumentId, transaction_id: TransactionId) {
        if let Some(state) = self.documents.get_mut(&document_id) {
            self.last_touched = Some(document_id);
            if let Some(position) = state
                .pending
                .iter()
                .position(|pending| pending.transaction_id == transaction_id)
            {
                state.pending.remove(position);
            }
            state.optimistic_version = state
                .pending
                .back()
                .map_or(state.confirmed_version, |pending| pending.base_version + 1);
        }
    }

    fn apply_resync_snapshot(&mut self, snapshot: ClientResyncSnapshot) {
        self.last_touched = Some(snapshot.document_id);
        let state = self
            .documents
            .entry(snapshot.document_id)
            .or_insert_with(|| DocumentSyncState::new(snapshot.version, snapshot.lease_id));
        state.confirmed_version = snapshot.version;
        state.optimistic_version = snapshot.version;
        state.pending.clear();
        state.last_resync = Some(snapshot.clone());
        state.lease_id = snapshot.lease_id;
    }

    /// Replace one document's tracking state (fresh open or retained-session
    /// activation) without disturbing other documents' states.
    fn install_document_state(
        &mut self,
        document_id: DocumentId,
        confirmed_version: DocumentVersion,
        pending: Vec<PendingEdit>,
        lease_id: Option<crate::protocol::LeaseId>,
    ) {
        self.last_touched = Some(document_id);
        let mut state = DocumentSyncState::new(confirmed_version, lease_id);
        for edit in pending {
            state.pending.push_back(edit);
        }
        if let Some(last) = state.pending.back() {
            state.optimistic_version = last.base_version.saturating_add(1);
        }
        self.documents.insert(document_id, state);
    }

    fn confirmed_version_for(&self, document_id: DocumentId) -> DocumentVersion {
        self.documents
            .get(&document_id)
            .map(|state| state.confirmed_version)
            .unwrap_or(self.default_confirmed_version)
    }

    fn lease_for(&self, document_id: DocumentId) -> Option<crate::protocol::LeaseId> {
        self.documents
            .get(&document_id)
            .and_then(|state| state.lease_id)
    }

    fn pending_len(&self) -> usize {
        self.documents
            .values()
            .map(|state| state.pending.len())
            .sum()
    }

    fn snapshot_for(&self, document_id: DocumentId) -> ClientSyncSnapshot {
        match self.documents.get(&document_id) {
            Some(state) => ClientSyncSnapshot {
                confirmed_version: state.confirmed_version,
                optimistic_version: state.optimistic_version,
                pending: state.pending.iter().cloned().collect(),
                last_resync: state.last_resync.clone(),
            },
            None => ClientSyncSnapshot {
                confirmed_version: self.default_confirmed_version,
                optimistic_version: self.default_confirmed_version,
                pending: Vec::new(),
                last_resync: None,
            },
        }
    }

    fn snapshot(&self) -> ClientSyncSnapshot {
        match self.last_touched {
            Some(document_id) => self.snapshot_for(document_id),
            None => ClientSyncSnapshot {
                confirmed_version: self.default_confirmed_version,
                optimistic_version: self.default_confirmed_version,
                pending: Vec::new(),
                last_resync: None,
            },
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
    #[allow(dead_code)]
    completion_recency: Arc<Mutex<VecDeque<String>>>,
}

#[allow(dead_code)]
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
                completion_recency: Arc::new(Mutex::new(VecDeque::new())),
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
            completion_recency: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn enqueue_raw(
        &self,
        message: ClientMessage,
    ) -> Result<(), tokio::sync::mpsc::error::TrySendError<ClientMessage>> {
        // Bridge path: send a validated client message verbatim. Edits and
        // editor intents MUST use the dedicated enqueue methods so optimistic
        // version bookkeeping runs.
        self.sender.try_send(message)
    }

    pub fn enqueue_edit_event(
        &self,
        event: EditorEditEvent,
        transaction_id: TransactionId,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let recorder = global_recorder();
        let _scope = recorder.scope_with_metadata(
            "client.edit_queue.enqueue",
            MetricMetadata::document(event.document_id, event.base_version)
                .with_trace_id(Some(transaction_id)),
        );
        let operation = event.operation;
        let (base_version, lease_id) = {
            let mut state = self.sync_state.lock().expect("client sync state poisoned");
            let base_version =
                state.reserve_pending(event.document_id, transaction_id, operation.clone());
            recorder.record_gauge(
                "client.edit_queue.pending_depth",
                state.pending_len() as u64,
            );
            // Phase 22.2: each document carries its own lease; fall back to the
            // queue-level lease (pre-open test bootstrap) when unknown.
            let lease_id = state.lease_for(event.document_id).or(self.lease_id);
            (base_version, lease_id)
        };
        let message = ClientMessage::Edit {
            document_id: event.document_id,
            client_id: self.client_id,
            lease_id,
            base_version,
            behavior_version: event.behavior_version,
            transaction_id,
            operation,
        };

        if lease_id.is_none() {
            let mut state = self.sync_state.lock().expect("client sync state poisoned");
            state.rollback_pending_reservation(event.document_id, transaction_id);
            return Err(mpsc::error::TrySendError::Closed(message));
        }

        if let Err(error) = self.sender.try_send(message) {
            let mut state = self.sync_state.lock().expect("client sync state poisoned");
            state.rollback_pending_reservation(event.document_id, transaction_id);
            recorder.record_counter("client.edit_queue.enqueue_failed", 1);
            recorder.record_gauge(
                "client.edit_queue.pending_depth",
                state.pending_len() as u64,
            );
            return Err(error);
        }

        recorder.record_counter("client.edit_queue.enqueued", 1);
        Ok(())
    }

    pub(crate) fn enqueue_viewport_render_request(
        &self,
        document_id: DocumentId,
        document_version: DocumentVersion,
        request_id: crate::protocol::ViewportRequestId,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        let message = ClientMessage::ViewportRenderRequest {
            client_id: self.client_id,
            document_id,
            document_version,
            request_id,
            byte_start,
            byte_end,
            trace_id: None,
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

    // Phase 24.1: server-owned menu intents. The client forwards keystrokes
    // only; the server is authoritative for query, items, selection, and
    // activation payloads (`session_id` is the opaque server handle).
    pub(crate) fn enqueue_menu_query_update(
        &self,
        session_id: u64,
        query: String,
        scope: Option<String>,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::MenuQueryUpdate {
            client_id: self.client_id,
            session_id,
            query,
            scope,
        })
    }

    /// Phase 24.3: semantic Backspace; the server session decides whether it
    /// deletes query text or ascends (path mode).
    pub(crate) fn enqueue_menu_backspace(
        &self,
        session_id: u64,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::MenuBackspace {
            client_id: self.client_id,
            session_id,
        })
    }

    pub(crate) fn enqueue_menu_selection_move(
        &self,
        session_id: u64,
        delta: i64,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::MenuSelectionMove {
            client_id: self.client_id,
            session_id,
            delta,
        })
    }

    pub(crate) fn enqueue_menu_activate(
        &self,
        session_id: u64,
        kind: crate::protocol::TransientMenuActivationData,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::MenuActivate {
            client_id: self.client_id,
            session_id,
            kind,
        })
    }

    pub(crate) fn enqueue_menu_cancel(
        &self,
        session_id: u64,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::MenuCancel {
            client_id: self.client_id,
            session_id,
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

    pub(crate) fn record_completion_accept(&self, insert_text: &str) {
        // ponytail: keep only four 64-char entries; send the full ring only if
        // completion quality proves this request-budget ceiling too restrictive.
        let text: String = insert_text
            .chars()
            .take(COMPLETION_RECENCY_MAX_ITEM_CHARS)
            .collect();
        if text.is_empty() {
            return;
        }
        let mut recency = self
            .completion_recency
            .lock()
            .expect("completion recency poisoned");
        if let Some(index) = recency.iter().position(|item| item == &text) {
            recency.remove(index);
        }
        recency.push_front(text);
        recency.truncate(COMPLETION_RECENCY_MAX_ITEMS);
    }

    fn recent_completion_texts(&self) -> Box<[String]> {
        self.completion_recency
            .lock()
            .expect("completion recency poisoned")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .into_boxed_slice()
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
                .snapshot_for(event.document_id)
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
                recent_completions: self.recent_completion_texts(),
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

    pub(crate) fn enqueue_selection_query_request(
        &self,
        event: EditorSelectionQueryRequestEvent,
        request_id: u64,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::SelectionQueryRequest {
            request: SelectionQueryRequest {
                request_id,
                client_id: self.client_id,
                document_id: event.document_id,
                document_version: event.document_version,
                behavior_version: event.behavior_version,
                query: event.query,
                selections: event.selections,
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

    /// Phase 22.3: send a server-authoritative tab lifecycle command (New /
    /// OpenWorkspace / Close / Activate / Reclaim) on this connection.
    /// Phase 22.3: re-open a document on this connection after a reconnect.
    /// The plain `OpenDocument` path (workspace root id + relative path) is
    /// used because a fresh connection holds no selected-file capability for
    /// its tab's previously open documents.
    pub fn enqueue_open_document(
        &self,
        workspace_root_id: crate::protocol::WorkspaceRootId,
        path: String,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::OpenDocument {
            client_id: self.client_id,
            workspace_root_id,
            path,
        })
    }

    pub fn enqueue_tab_command(
        &self,
        command: crate::protocol::TabCommand,
    ) -> Result<(), mpsc::error::TrySendError<ClientMessage>> {
        self.sender.try_send(ClientMessage::TabCommand {
            client_id: self.client_id,
            command,
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

    /// Snapshot of one document's tracking state (Phase 22.2). Pane views use
    /// this when stashing a session so only that document's pending edits are
    /// retained.
    pub(crate) fn sync_snapshot_for(&self, document_id: DocumentId) -> ClientSyncSnapshot {
        self.sync_state
            .lock()
            .expect("client sync state poisoned")
            .snapshot_for(document_id)
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
        let mut state = self.sync_state.lock().expect("client sync state poisoned");
        // Phase 22.2: replace only this document's state; other panes' live
        // documents keep their own confirmed/optimistic tracking.
        state.install_document_state(
            document_id,
            confirmed_version,
            Vec::new(),
            access.lease_id(),
        );
    }

    pub fn install_document_sync_state(
        &mut self,
        document_id: DocumentId,
        access: &DocumentAccess,
        confirmed_version: DocumentVersion,
        pending: Vec<PendingEdit>,
    ) {
        self.lease_id = access.lease_id();
        let mut state = self.sync_state.lock().expect("client sync state poisoned");
        state.install_document_state(document_id, confirmed_version, pending, access.lease_id());
    }
}

#[derive(Debug)]
pub struct ClientSession {
    pub initial_state: ClientInitialState,
    pub edit_queue: ClientEditQueue,
    pub events: mpsc::Receiver<ClientConnectionEvent>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
        head: DocumentTextHead,
    },
    DocumentChunk {
        document_id: DocumentId,
        document_version: DocumentVersion,
        offset: u64,
        text: String,
    },
    DocumentChunkRejected {
        document_id: DocumentId,
        document_version: DocumentVersion,
        offset: u64,
        reason: DocumentChunkRejection,
    },
    DocumentSaved {
        document_id: DocumentId,
        version: DocumentVersion,
        dirty: bool,
    },
    /// Reply to `GetDocumentStatus`; carries the authoritative metadata
    /// (including `workspace_root_id`) the React client needs before it can
    /// open persisted documents during layout restore.
    DocumentStatus {
        metadata: DocumentMetadata,
    },
    DocumentClosed {
        document_id: DocumentId,
        closed: bool,
    },
    DocumentReloaded {
        metadata: DocumentMetadata,
        head: DocumentTextHead,
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
    FoldingRangeSet(crate::protocol::FoldingRangeSet),
    /// Protocol v29: one complete atomic answer to a `ViewportRenderRequest`.
    ViewportRenderPatch(crate::protocol::ViewportRenderPatch),
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
    /// Plan 071 task 10: read-only text-object/smart-select ranges for one
    /// selection query; the widget applies them as selections.
    SelectionQueryResult(SelectionQueryResult),
    /// Plan 071 follow-up round (`editor-control`): gated programmatic
    /// editor-command execution request pushed by the server.
    EditorCommandRequest(EditorCommandRequest),
    /// Plan 071 caret-transport fix: runtime caret appearance override from
    /// `clientSetCursorStyle`; `None` clears back to manifest/theme layers.
    CaretStyleOverride(Option<CaretStyle>),
    /// Phase 26 user-owned editor wrap-policy override from `setEditorLayout`;
    /// `None` clears back to the per-mode manifest then `from_font_role`.
    EditorLayoutOverride(Option<crate::protocol::WrapPolicy>),
    /// Phase 22.1 shell-level user preferences from `setPaneFocusPolicy`.
    ShellPreferences(ShellPreferences),
    /// Phase 22.3: server-authoritative tab registry snapshot. Broadcast to
    /// every connection on any registry mutation and replayed on handshake;
    /// the `Driver` applies it to the tab bar and per-tab connection map.
    TabRegistry(TabRegistrySnapshot),
    RuntimeDiagnostic(RuntimeDiagnostic),
    EditTransaction(ServerMessage),
    /// Phase 24.1: server-authoritative transient-menu display copy. Keys
    /// route back as menu intents; visuals update only from snapshots.
    TransientMenuSnapshot(Box<crate::protocol::TransientMenuSnapshotData>),
    /// Phase 24.1: the server closed a menu session (Escape, activation,
    /// tab switch, replacement open, or generation replacement).
    TransientMenuClosed {
        session_id: u64,
    },
    /// Phase 24.2: server-approved shell command from menu activation. The
    /// client re-parses the id deny-by-default via
    /// `ShellClientCommand::from_command_id`; unknown/forged ids are dropped.
    ShellClientCommandRequest {
        command_id: String,
    },
    Agent(Box<crate::protocol::AgentServerMessage>),
    /// Reply to `ListAgentSettingsFiles`: the server-resolved listing of the
    /// agent's delivered config files. The webview renders it in the coding
    /// agent's Settings tab; paths stay server-side (display only).
    AgentSettingsFiles {
        client_id: ClientId,
        files: Vec<crate::protocol::AgentSettingsFileInfo>,
    },
    /// Reply to `ListLauncherEntries` / `RemoveLauncherRecent`: the start
    /// surface's server-resolved rows (recent workspaces + agent types).
    LauncherEntries {
        client_id: ClientId,
        entries: Box<crate::protocol::LauncherEntries>,
    },
    ServerError {
        code: ProtocolErrorCode,
        message: String,
    },
    Disconnected,
    ConnectionError(String),
}

impl ClientConnectionEvent {
    /// The document a connection event is scoped to, if any (Phase 22.2 pane
    /// routing). Request-scoped events (completion/language-intelligence
    /// rejects, selection queries carry their own document) return `None` and
    /// route by request id instead.
    pub fn document_id(&self) -> Option<DocumentId> {
        match self {
            ClientConnectionEvent::EditAck { document_id, .. }
            | ClientConnectionEvent::EditRejected { document_id, .. }
            | ClientConnectionEvent::DocumentSaved { document_id, .. }
            | ClientConnectionEvent::DocumentClosed { document_id, .. }
            | ClientConnectionEvent::DocumentChunk { document_id, .. }
            | ClientConnectionEvent::DocumentChunkRejected { document_id, .. } => {
                Some(*document_id)
            }
            ClientConnectionEvent::ResyncSnapshot(snapshot) => Some(snapshot.document_id),
            ClientConnectionEvent::DocumentOpened { metadata, .. }
            | ClientConnectionEvent::DocumentReloaded { metadata, .. }
            | ClientConnectionEvent::DocumentStatus { metadata } => Some(metadata.document_id),
            ClientConnectionEvent::FileOperationFailed { document_id, .. } => *document_id,
            ClientConnectionEvent::DecorationSet(set) => Some(set.document_id),
            ClientConnectionEvent::DecorationBatch(sets) => sets.first().map(|set| set.document_id),
            ClientConnectionEvent::DiagnosticSet(set) => Some(set.document_id),
            ClientConnectionEvent::FoldingRangeSet(set) => Some(set.document_id),
            ClientConnectionEvent::ViewportRenderPatch(patch) => Some(patch.document_id),
            ClientConnectionEvent::CompletionResult(result) => Some(result.document_id),
            ClientConnectionEvent::LanguageIntelligenceResult(result) => Some(result.document_id),
            ClientConnectionEvent::SelectionQueryResult(result) => Some(result.document_id),
            _ => None,
        }
    }

    /// Phase 22.6: the document path carried by open/reload events, for the
    /// driver to keep pane accessibility labels in sync (sanitized by the
    /// caller before any announcement).
    pub fn metadata_path(&self) -> Option<&str> {
        match self {
            ClientConnectionEvent::DocumentOpened { metadata, .. }
            | ClientConnectionEvent::DocumentReloaded { metadata, .. } => Some(&metadata.path),
            _ => None,
        }
    }
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

/// Desktop-side adoption probe: is the listener on this endpoint a Clay
/// server speaking the current protocol version?
///
/// The desktop supervisor must not adopt (or trust) an endpoint it has not
/// handshaken with: an old server from a previous build otherwise answers
/// every later handshake with `UnsupportedProtocolVersion` and reconnect can
/// never recover. The probe sends one `Hello` and reads exactly one reply;
/// dropping the stream afterwards ends the probe connection cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolProbe {
    /// Listener completed Hello/Welcome at [`PROTOCOL_VERSION`].
    Compatible,
    /// Something answered but refused (or could not speak) the current
    /// protocol version; the endpoint is occupied by an incompatible server.
    Incompatible,
    /// Nothing accepted the transport connection.
    NotListening,
}

pub async fn probe_protocol(endpoint: &IpcEndpoint) -> ProtocolProbe {
    let Ok(mut stream) = connect_transport(endpoint).await else {
        return ProtocolProbe::NotListening;
    };
    let codec = Codec::default();
    let hello = codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "clay-probe".to_string(),
            },
        )
        .await;
    if hello.is_err() {
        // Listener vanished between connect and write: treat as empty.
        return ProtocolProbe::NotListening;
    }
    match timeout(SNAPSHOT_TIMEOUT, codec.read_server_message(&mut stream)).await {
        Ok(Ok(ServerMessage::Welcome {
            protocol_version, ..
        })) if protocol_version == PROTOCOL_VERSION => ProtocolProbe::Compatible,
        _ => ProtocolProbe::Incompatible,
    }
}

pub async fn connect(endpoint: &IpcEndpoint) -> Result<ClientSession, ClientBootstrapError> {
    connect_with_binding(
        endpoint,
        TabCommand::New {
            workspace_root: String::new(),
        },
    )
    .await
}

/// Connect and bind a new tab to `workspace_root` before returning the
/// session's initial document. Empty root requests server bootstrap-root
/// selection for the first tab.
#[doc(hidden)]
pub async fn connect_with_workspace_root(
    endpoint: &IpcEndpoint,
    workspace_root: impl Into<String>,
) -> Result<ClientSession, ClientBootstrapError> {
    connect_with_binding(
        endpoint,
        TabCommand::New {
            workspace_root: workspace_root.into(),
        },
    )
    .await
}

/// Connect and reclaim an existing server tab before returning its initial
/// document.
#[doc(hidden)]
pub async fn connect_for_reclaim(
    endpoint: &IpcEndpoint,
    tab_id: TabId,
) -> Result<ClientSession, ClientBootstrapError> {
    connect_with_binding(endpoint, TabCommand::Reclaim { tab_id }).await
}

/// Rebind a tab when its server still has the registry entry; after a server
/// restart or TTL eviction, rebuild the tab from its persisted workspace root.
/// The driver cancels this path when the tab is explicitly removed.
#[doc(hidden)]
pub async fn connect_for_reclaim_or_new(
    endpoint: &IpcEndpoint,
    tab_id: TabId,
    workspace_root: String,
) -> Result<ClientSession, ClientBootstrapError> {
    match connect_for_reclaim(endpoint, tab_id).await {
        Ok(session) => Ok(session),
        Err(error) if error.kind() == ClientBootstrapErrorKind::ServerRejected => {
            connect_with_workspace_root(endpoint, workspace_root).await
        }
        Err(error) => Err(error),
    }
}

async fn connect_with_binding(
    endpoint: &IpcEndpoint,
    binding: TabCommand,
) -> Result<ClientSession, ClientBootstrapError> {
    let stream = connect_transport(endpoint).await.map_err(CodecError::Io)?;
    timeout(
        SNAPSHOT_TIMEOUT,
        connect_from_stream_with_binding(stream, Codec::default(), binding),
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
    stream: S,
    codec: Codec,
) -> Result<ClientSession, ClientBootstrapError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    connect_from_stream_with_binding(
        stream,
        codec,
        TabCommand::New {
            workspace_root: String::new(),
        },
    )
    .await
}

async fn connect_from_stream_with_binding<S>(
    mut stream: S,
    codec: Codec,
    binding: TabCommand,
) -> Result<ClientSession, ClientBootstrapError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let handshake = handshake_initial_state(&mut stream, codec, binding).await?;
    let initial_state = handshake.initial_state;
    let (edit_queue, outgoing_edits) = ClientEditQueue::bounded(EDIT_QUEUE_CAPACITY);
    let mut edit_queue = edit_queue
        .with_authority(initial_state.client_id, &initial_state.access)
        .with_confirmed_version(initial_state.document_version);
    if let Some(capability) = handshake.file_open_capability {
        edit_queue = edit_queue.with_file_open_capability(capability);
    }
    let sync_state = Arc::clone(&edit_queue.sync_state);
    let file_open_capability = Arc::clone(&edit_queue.file_open_capability);
    let behavior_state = Arc::new(Mutex::new(
        behavior::ClientBehaviorState::new(initial_state.behavior_manifest.clone())
            .map_err(|_| ClientBootstrapError::UnexpectedMessage("invalid BehaviorManifest"))?,
    ));
    let (event_sender, events) = mpsc::channel(EDIT_QUEUE_CAPACITY);
    for message in handshake.pending_messages {
        if let Some(event) =
            pending_handshake_event(message, &behavior_state, initial_state.client_id)
        {
            let _ = event_sender.send(event).await;
        }
    }
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

struct HandshakeResult {
    initial_state: ClientInitialState,
    pending_messages: Vec<ServerMessage>,
    file_open_capability: Option<String>,
}

async fn handshake_initial_state<S>(
    stream: &mut S,
    codec: Codec,
    binding: TabCommand,
) -> Result<HandshakeResult, ClientBootstrapError>
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

    // Keep scripted/unit-server compatibility: older handshakes sent the
    // document before the manifest. Production servers now send the manifest
    // first and wait for the tab-binding command before sending document text.
    let first_after_welcome = codec.read_server_message(&mut *stream).await?;
    if let ServerMessage::InitialDocument {
        document_id,
        version,
        head,
        access,
        workspace_root,
        ..
    } = first_after_welcome
    {
        let behavior_manifest = match codec.read_server_message(&mut *stream).await? {
            ServerMessage::BehaviorManifest(manifest) => *manifest,
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
        let active_typography = read_active_typography(stream, codec).await?;
        return Ok(HandshakeResult {
            initial_state: ClientInitialState {
                client_id,
                document_id,
                document_version: version,
                head,
                access,
                behavior_manifest,
                active_theme,
                active_typography,
                workspace_root,
            },
            pending_messages: Vec::new(),
            file_open_capability: None,
        });
    }

    let mut manifests = match first_after_welcome {
        ServerMessage::BehaviorManifest(manifest) => vec![*manifest],
        ServerMessage::Error { code, message } => {
            return Err(ClientBootstrapError::ServerError { code, message });
        }
        _ => {
            return Err(ClientBootstrapError::UnexpectedMessage(
                "expected BehaviorManifest",
            ));
        }
    };
    let active_theme = loop {
        match codec.read_server_message(&mut *stream).await? {
            ServerMessage::BehaviorManifest(manifest) => manifests.push(*manifest),
            ServerMessage::ActiveTheme(theme) => break theme,
            ServerMessage::Error { code, message } => {
                return Err(ClientBootstrapError::ServerError { code, message });
            }
            _ => {
                return Err(ClientBootstrapError::UnexpectedMessage(
                    "expected ActiveTheme",
                ));
            }
        }
    };
    let active_typography = read_active_typography(stream, codec).await?;

    codec
        .write_client_message(
            &mut *stream,
            &ClientMessage::TabCommand {
                client_id,
                command: binding,
            },
        )
        .await?;

    let mut pending_messages = Vec::new();
    let mut file_open_capability = None;
    let (document_id, document_version, head, access, workspace_root) = loop {
        match codec.read_server_message(&mut *stream).await? {
            ServerMessage::InitialDocument {
                document_id,
                version,
                head,
                access,
                workspace_root,
                ..
            } => break (document_id, version, head, access, workspace_root),
            ServerMessage::FileOpenCapabilityIssued { token } => {
                file_open_capability = Some(token);
            }
            ServerMessage::Error { code, message } => {
                return Err(ClientBootstrapError::ServerError { code, message });
            }
            message => pending_messages.push(message),
        }
    };

    let behavior_manifest = manifests
        .iter()
        .find(|manifest| matches!(manifest.scope, BehaviorScope::GlobalDefault))
        .cloned()
        .or_else(|| manifests.last().cloned())
        .ok_or(ClientBootstrapError::UnexpectedMessage(
            "expected BehaviorManifest",
        ))?;

    Ok(HandshakeResult {
        initial_state: ClientInitialState {
            client_id,
            document_id,
            document_version,
            head,
            access,
            behavior_manifest,
            active_theme,
            active_typography,
            workspace_root,
        },
        pending_messages: manifests
            .into_iter()
            .filter(|manifest| !matches!(manifest.scope, BehaviorScope::GlobalDefault))
            .map(|manifest| ServerMessage::BehaviorManifest(Box::new(manifest)))
            .chain(pending_messages)
            .collect(),
        file_open_capability,
    })
}

async fn read_active_typography<S>(
    stream: &mut S,
    codec: Codec,
) -> Result<ActiveTypography, ClientBootstrapError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match codec.read_server_message(&mut *stream).await? {
        ServerMessage::ActiveTypography(typography) if typography.validate().is_ok() => {
            Ok(typography)
        }
        ServerMessage::ActiveTypography(_) => Err(ClientBootstrapError::UnexpectedMessage(
            "invalid ActiveTypography",
        )),
        ServerMessage::Error { code, message } => {
            Err(ClientBootstrapError::ServerError { code, message })
        }
        _ => Err(ClientBootstrapError::UnexpectedMessage(
            "expected ActiveTypography",
        )),
    }
}

fn pending_handshake_event(
    message: ServerMessage,
    behavior_state: &Arc<Mutex<behavior::ClientBehaviorState>>,
    client_id: ClientId,
) -> Option<ClientConnectionEvent> {
    match message {
        ServerMessage::BehaviorManifest(manifest) => {
            let behavior_version = manifest.behavior_version;
            match behavior_state
                .lock()
                .expect("client behavior state poisoned")
                .install_replacement(manifest.as_ref().clone())
            {
                Ok(()) => Some(ClientConnectionEvent::BehaviorManifestInstalled {
                    behavior_version,
                    manifest: *manifest,
                }),
                Err(error) => Some(ClientConnectionEvent::BehaviorManifestRejected {
                    behavior_version,
                    reason: format!("{error:?}"),
                }),
            }
        }
        ServerMessage::CaretStyleOverride(style)
            if style.as_ref().is_none_or(|style| style.validate().is_ok()) =>
        {
            Some(ClientConnectionEvent::CaretStyleOverride(style))
        }
        ServerMessage::EditorLayoutOverride(wrap) => {
            Some(ClientConnectionEvent::EditorLayoutOverride(wrap))
        }
        ServerMessage::ShellPreferences(preferences) => {
            Some(ClientConnectionEvent::ShellPreferences(preferences))
        }
        ServerMessage::SduiSnapshot { client_id, tree } => {
            Some(ClientConnectionEvent::SduiSnapshot { client_id, tree })
        }
        ServerMessage::SduiUpdate { update } => Some(ClientConnectionEvent::SduiUpdate(update)),
        ServerMessage::TabRegistry(snapshot) => Some(ClientConnectionEvent::TabRegistry(snapshot)),
        ServerMessage::RuntimeDiagnostic(diagnostic) => {
            Some(ClientConnectionEvent::RuntimeDiagnostic(diagnostic))
        }
        ServerMessage::ActiveTheme(theme) => Some(ClientConnectionEvent::ActiveTheme(theme)),
        ServerMessage::ActiveTypography(typography) if typography.validate().is_ok() => {
            Some(ClientConnectionEvent::ActiveTypography(typography))
        }
        ServerMessage::RuntimeStateSnapshot(snapshot)
            if snapshot.client_id == client_id && snapshot.validate().is_ok() =>
        {
            Some(ClientConnectionEvent::RuntimeStateSnapshot(snapshot))
        }
        ServerMessage::Error { code, message } => {
            Some(ClientConnectionEvent::ServerError { code, message })
        }
        _ => None,
    }
}

fn client_message_trace_id(message: &ClientMessage) -> Option<u64> {
    match message {
        ClientMessage::Edit { transaction_id, .. } => Some(*transaction_id),
        ClientMessage::ViewportRenderRequest { trace_id, .. } => *trace_id,
        _ => None,
    }
}

fn server_message_trace_id(message: &ServerMessage) -> Option<u64> {
    match message {
        ServerMessage::EditAck { transaction_id, .. }
        | ServerMessage::EditRejected { transaction_id, .. } => Some(*transaction_id),
        ServerMessage::DecorationSet(set) => set.trace_id,
        ServerMessage::DecorationBatch(sets) => sets.first().and_then(|set| set.trace_id),
        ServerMessage::ViewportRenderPatch(patch) => patch.trace_id,
        _ => None,
    }
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
                let recorder = global_recorder();
                let delivery = recorder
                    .is_enabled()
                    .then(|| client_message_trace_id(&message))
                    .flatten()
                    .map(|trace_id| {
                        recorder.scope_with_metadata(
                            BRIDGE_CLIENT_DELIVERY,
                            MetricMetadata::default().with_trace_id(Some(trace_id)),
                        )
                    });
                if let Err(error) = codec.write_client_message(&mut writer, &message).await {
                    if let Some(delivery) = delivery {
                        delivery.finish();
                    }
                    let _ = events.send(ClientConnectionEvent::ConnectionError(error.to_string())).await;
                    return;
                }
                if let Some(delivery) = delivery {
                    delivery.finish();
                }
            }
            incoming = incoming_rx.recv() => {
                let Some(incoming) = incoming else {
                    let _ = events.send(ClientConnectionEvent::Disconnected).await;
                    return;
                };
                let recorder = global_recorder();
                if recorder.is_enabled()
                    && let Ok(message) = &incoming
                    && let Some(trace_id) = server_message_trace_id(message)
                {
                    recorder.record_with_metadata(
                        BRIDGE_SERVER_DELIVERY,
                        MetricValue::Counter { amount: 1 },
                        MetricMetadata::default().with_trace_id(Some(trace_id)),
                    );
                }
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
                            state.pending_len()
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
                            state.confirmed_version_for(document_id)
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
                    Ok(ServerMessage::ResyncSnapshot { document_id, version, head, access, lease_id }) => {
                        let snapshot = ClientResyncSnapshot { document_id, version, head, access, lease_id };
                        sync_state
                            .lock()
                            .expect("client sync state poisoned")
                            .apply_resync_snapshot(snapshot.clone());
                        let _ = events.send(ClientConnectionEvent::ResyncSnapshot(snapshot)).await;
                    }
                    Ok(ServerMessage::DocumentOpened { metadata, head }) => {
                        // Multi-document: do not wipe live sync state here. The
                        // editor widget retains the prior session and then
                        // installs authority for the newly active document.
                        let _ = events.send(ClientConnectionEvent::DocumentOpened { metadata, head }).await;
                    }
                    Ok(ServerMessage::DocumentChunk { document_id, document_version, offset, text }) => {
                        let _ = events.send(ClientConnectionEvent::DocumentChunk { document_id, document_version, offset, text }).await;
                    }
                    Ok(ServerMessage::DocumentChunkRejected { document_id, document_version, offset, reason }) => {
                        let _ = events.send(ClientConnectionEvent::DocumentChunkRejected { document_id, document_version, offset, reason }).await;
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
                    Ok(ServerMessage::DocumentStatus { metadata }) => {
                        let _ = events
                            .send(ClientConnectionEvent::DocumentStatus { metadata })
                            .await;
                    }
                    Ok(ServerMessage::DocumentReloaded { metadata, head }) => {
                        // Phase 22.2: each document owns its tracking state, so
                        // reloads rewrite only the reloaded document's state
                        // regardless of which pane is active.
                        {
                            let mut state = sync_state
                                .lock()
                                .expect("client sync state poisoned");
                            state.apply_resync_snapshot(ClientResyncSnapshot {
                                document_id: metadata.document_id,
                                version: metadata.version,
                                head: head.clone(),
                                access: metadata.access.clone(),
                                lease_id: metadata.lease_id,
                            });
                        }
                        let _ = events
                            .send(ClientConnectionEvent::DocumentReloaded { metadata, head })
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
                    Ok(ServerMessage::FoldingRangeSet(set)) => {
                        let _ = events.send(ClientConnectionEvent::FoldingRangeSet(set)).await;
                    }
                    Ok(ServerMessage::ViewportRenderPatch(patch)) => {
                        let _ = events.send(ClientConnectionEvent::ViewportRenderPatch(patch)).await;
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
                    Ok(ServerMessage::SelectionQueryResult { result }) => {
                        let _ = events.send(ClientConnectionEvent::SelectionQueryResult(result)).await;
                    }
                    Ok(ServerMessage::EditorCommandRequest(request)) => {
                        let _ = events
                            .send(ClientConnectionEvent::EditorCommandRequest(*request))
                            .await;
                    }
                    Ok(ServerMessage::CaretStyleOverride(style))
                        if style.as_ref().is_none_or(|style| style.validate().is_ok()) =>
                    {
                        let _ = events
                            .send(ClientConnectionEvent::CaretStyleOverride(style))
                            .await;
                    }
                    Ok(ServerMessage::CaretStyleOverride(_)) => {}
                    Ok(ServerMessage::EditorLayoutOverride(wrap)) => {
                        let _ = events
                            .send(ClientConnectionEvent::EditorLayoutOverride(wrap))
                            .await;
                    }
                    Ok(ServerMessage::ShellPreferences(prefs)) => {
                        let _ = events.send(ClientConnectionEvent::ShellPreferences(prefs)).await;
                    }
                    Ok(ServerMessage::TabRegistry(snapshot)) => {
                        let _ = events.send(ClientConnectionEvent::TabRegistry(snapshot)).await;
                    }
                    Ok(ServerMessage::RuntimeDiagnostic(diagnostic)) => {
                        let _ = events.send(ClientConnectionEvent::RuntimeDiagnostic(diagnostic)).await;
                    }
                    Ok(message @ ServerMessage::EditTransaction { .. }) => {
                        let _ = events.send(ClientConnectionEvent::EditTransaction(message)).await;
                    }
                    Ok(ServerMessage::Agent(payload)) => {
                        let _ = events.send(ClientConnectionEvent::Agent(payload)).await;
                    }
                    Ok(ServerMessage::AgentSettingsFiles { client_id, files }) => {
                        let _ = events
                            .send(ClientConnectionEvent::AgentSettingsFiles {
                                client_id,
                                files,
                            })
                            .await;
                    }
                    Ok(ServerMessage::LauncherEntries { client_id, entries }) => {
                        let _ = events
                            .send(ClientConnectionEvent::LauncherEntries { client_id, entries })
                            .await;
                    }
                    Ok(ServerMessage::TransientMenuSnapshot(snapshot)) => {
                        let _ = events
                            .send(ClientConnectionEvent::TransientMenuSnapshot(snapshot))
                            .await;
                    }
                    Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                        let _ = events
                            .send(ClientConnectionEvent::TransientMenuClosed { session_id })
                            .await;
                    }
                    Ok(ServerMessage::ShellClientCommandRequest { command_id }) => {
                        let _ = events
                            .send(ClientConnectionEvent::ShellClientCommandRequest { command_id })
                            .await;
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
                            .install_replacement(manifest.as_ref().clone());
                        match install_result {
                            Ok(()) => {
                                let _ = events.send(ClientConnectionEvent::BehaviorManifestInstalled { behavior_version, manifest: *manifest }).await;
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
mod tests;
