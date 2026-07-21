//! Bounded analyzer-neutral package document workers.
//!
//! Workers are keyed by package, fixed process contribution, workspace root,
//! and runtime generation. Rust sends only canonical open/reset snapshots,
//! accepted UTF-8 byte deltas, close events, and existing completion/language
//! request shapes. Package JavaScript owns protocol and analyzer policy.

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use tokio::sync::{Notify, mpsc, oneshot};

use crate::{
    packages::record::PackageRecord,
    perf::budgets::{
        DOCUMENT_ANALYSIS_INPUT_MAX_BYTES, DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS,
        DOCUMENT_ANALYSIS_MAX_DELTA_BYTES, DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES,
        DOCUMENT_ANALYSIS_MAX_DOCUMENTS_PER_WORKER, DOCUMENT_ANALYSIS_MAX_PENDING_REQUESTS,
        DOCUMENT_ANALYSIS_MAX_TEXT_BYTES_PER_WORKER, DOCUMENT_ANALYSIS_MAX_WORKERS,
        DOCUMENT_ANALYSIS_OUTPUT_MAX_BYTES, DOCUMENT_ANALYSIS_OUTPUT_MAX_EVENTS,
        DOCUMENT_ANALYSIS_TOTAL_SHUTDOWN_MS,
    },
    protocol::{
        CompletionItem, CompletionProvenance, CompletionRequest, CompletionResultSet,
        DecorationSet, DiagnosticSet, DocumentId, DocumentMetadata, DocumentVersion,
        LanguageIntelligenceRequest, LanguageIntelligenceResult, RuntimeDiagnostic,
        WorkspaceRootId,
    },
    server::{
        completion::{
            CompletionCoordinator, CompletionDocumentWindow, CompletionProvider,
            CompletionProviderError, CompletionProviderFuture, CompletionProviderMeta,
            CompletionTriggerMetadata, WordBoundaryRule,
        },
        decorations::validate_decoration_publication,
        diagnostics::validate_diagnostic_publication,
        js_runtime::{ClayJsRuntimeService, DocumentAnalysisInvocation},
        language_intelligence::{
            LanguageIntelligenceCoordinator, LanguageIntelligenceDocumentWindow,
            LanguageIntelligenceProvider, LanguageIntelligenceProviderError,
            LanguageIntelligenceProviderFuture, LanguageIntelligenceProviderMeta,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsDocumentAnalyzerRegistration {
    pub(crate) package: PackageRecord,
    pub(crate) id: String,
    pub(crate) contribution: String,
    pub(crate) modes: Vec<String>,
    pub(crate) module_specifier: String,
    pub(crate) export_name: String,
}

#[derive(Debug, Clone)]
pub(crate) enum DocumentAnalysisEvent {
    Open {
        document_id: DocumentId,
        document_version: DocumentVersion,
        runtime_generation: u64,
        active_mode: String,
        workspace_root_id: WorkspaceRootId,
        canonical_root_path: String,
        relative_path: String,
        text: String,
    },
    Change {
        document_id: DocumentId,
        base_version: DocumentVersion,
        document_version: DocumentVersion,
        byte_start: u64,
        byte_end: u64,
        inserted_text: String,
    },
    Reset {
        document_id: DocumentId,
        document_version: DocumentVersion,
        text: String,
    },
    Close {
        document_id: DocumentId,
        document_version: DocumentVersion,
    },
    Completion {
        request: CompletionRequest,
        window: CompletionDocumentWindow,
    },
    LanguageIntelligence {
        request: LanguageIntelligenceRequest,
        window: LanguageIntelligenceDocumentWindow,
    },
    Shutdown,
}

impl DocumentAnalysisEvent {
    pub(crate) fn document_id(&self) -> Option<DocumentId> {
        match self {
            Self::Open { document_id, .. }
            | Self::Change { document_id, .. }
            | Self::Reset { document_id, .. }
            | Self::Close { document_id, .. } => Some(*document_id),
            Self::Completion { request, .. } => Some(request.document_id),
            Self::LanguageIntelligence { request, .. } => Some(request.document_id),
            Self::Shutdown => None,
        }
    }

    fn estimated_bytes(&self) -> usize {
        match self {
            Self::Open {
                active_mode,
                canonical_root_path,
                relative_path,
                text,
                ..
            } => {
                active_mode.len()
                    + canonical_root_path.len()
                    + relative_path.len()
                    + text.len()
                    + 128
            }
            Self::Change { inserted_text, .. } => inserted_text.len() + 96,
            Self::Reset { text, .. } => text.len() + 64,
            Self::Completion { window, .. } => window.text.len() + 256,
            Self::LanguageIntelligence { window, .. } => window.text.len() + 256,
            Self::Close { .. } | Self::Shutdown => 64,
        }
    }

    fn can_coalesce(&self) -> bool {
        matches!(
            self,
            Self::Change { .. }
                | Self::Reset { .. }
                | Self::Completion { .. }
                | Self::LanguageIntelligence { .. }
        )
    }
}

#[derive(Debug)]
enum AnalysisReply {
    Completion(oneshot::Sender<Result<CompletionResultSet, CompletionProviderError>>),
    LanguageIntelligence(
        oneshot::Sender<Result<LanguageIntelligenceResult, LanguageIntelligenceProviderError>>,
    ),
}

#[derive(Debug)]
struct QueuedEvent {
    bytes: usize,
    event: DocumentAnalysisEvent,
    reply: Option<AnalysisReply>,
}

#[derive(Debug, Default)]
struct MailboxState {
    queue: VecDeque<QueuedEvent>,
    bytes: usize,
    closed: bool,
}

#[derive(Debug, Default)]
struct AnalysisMailbox {
    state: Mutex<MailboxState>,
    ready: Notify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnqueueError {
    Full,
    Closed,
}

impl AnalysisMailbox {
    fn push(
        &self,
        event: DocumentAnalysisEvent,
        reply: Option<AnalysisReply>,
    ) -> Result<(), EnqueueError> {
        let bytes = event.estimated_bytes();
        let mut state = self.state.lock().expect("analysis mailbox lock poisoned");
        if state.closed {
            return Err(EnqueueError::Closed);
        }
        if state.queue.len() >= DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS
            || state.bytes.saturating_add(bytes) > DOCUMENT_ANALYSIS_INPUT_MAX_BYTES
        {
            return Err(EnqueueError::Full);
        }
        state.bytes += bytes;
        state.queue.push_back(QueuedEvent {
            bytes,
            event,
            reply,
        });
        drop(state);
        self.ready.notify_one();
        Ok(())
    }

    fn coalesce_reset(&self, event: DocumentAnalysisEvent) -> Result<(), EnqueueError> {
        let Some(document_id) = event.document_id() else {
            return self.push(event, None);
        };
        let bytes = event.estimated_bytes();
        let mut state = self.state.lock().expect("analysis mailbox lock poisoned");
        if state.closed {
            return Err(EnqueueError::Closed);
        }
        let mut removed_bytes = 0;
        state.queue.retain(|queued| {
            let remove =
                queued.event.document_id() == Some(document_id) && queued.event.can_coalesce();
            if remove {
                removed_bytes += queued.bytes;
            }
            !remove
        });
        state.bytes = state.bytes.saturating_sub(removed_bytes);
        if state.queue.len() >= DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS
            || state.bytes.saturating_add(bytes) > DOCUMENT_ANALYSIS_INPUT_MAX_BYTES
        {
            return Err(EnqueueError::Full);
        }
        state.bytes += bytes;
        state.queue.push_back(QueuedEvent {
            bytes,
            event,
            reply: None,
        });
        drop(state);
        self.ready.notify_one();
        Ok(())
    }

    async fn pop(&self) -> Option<QueuedEvent> {
        loop {
            let notified = self.ready.notified();
            {
                let mut state = self.state.lock().expect("analysis mailbox lock poisoned");
                if let Some(event) = state.queue.pop_front() {
                    state.bytes = state.bytes.saturating_sub(event.bytes);
                    return Some(event);
                }
                if state.closed {
                    return None;
                }
            }
            notified.await;
        }
    }

    fn close(&self) {
        let mut state = self.state.lock().expect("analysis mailbox lock poisoned");
        state.closed = true;
        state.queue.clear();
        state.bytes = 0;
        drop(state);
        self.ready.notify_waiters();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WorkerKey {
    package_name: String,
    contribution: String,
    workspace_root_id: WorkspaceRootId,
    generation: u64,
}

#[derive(Debug, Clone)]
struct DocumentRoute {
    key: WorkerKey,
}

#[derive(Debug, Clone, Copy)]
struct ActiveDocument {
    version: DocumentVersion,
    text_bytes: usize,
}

#[derive(Clone)]
struct AnalysisWorker {
    mailbox: Arc<AnalysisMailbox>,
    active_documents: Arc<Mutex<HashMap<DocumentId, ActiveDocument>>>,
    active: Arc<AtomicBool>,
    pending_requests: Arc<AtomicUsize>,
}

impl fmt::Debug for AnalysisWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnalysisWorker")
            .field("active", &self.active.load(Ordering::Relaxed))
            .field(
                "pending_requests",
                &self.pending_requests.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

struct RegisteredAnalyzer {
    registration: JsDocumentAnalyzerRegistration,
    runtime: ClayJsRuntimeService,
    generation: u64,
}

impl fmt::Debug for RegisteredAnalyzer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegisteredAnalyzer")
            .field("registration", &self.registration)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct CoordinatorInner {
    registrations: Vec<RegisteredAnalyzer>,
    workers: HashMap<WorkerKey, AnalysisWorker>,
    routes: HashMap<DocumentId, Vec<DocumentRoute>>,
}

#[derive(Debug, Clone)]
pub(crate) enum DocumentAnalysisOutput {
    Decorations(DecorationSet),
    Diagnostics(DiagnosticSet),
    Diagnostic(RuntimeDiagnostic),
}

#[derive(Debug)]
pub(crate) enum DocumentAnalysisResponse {
    None,
    Completion(Result<CompletionResultSet, CompletionProviderError>),
    LanguageIntelligence(Result<LanguageIntelligenceResult, LanguageIntelligenceProviderError>),
}

#[derive(Clone)]
pub(crate) struct DocumentAnalysisCoordinator {
    inner: Arc<Mutex<CoordinatorInner>>,
    outputs_tx: mpsc::Sender<DocumentAnalysisOutput>,
    outputs_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<DocumentAnalysisOutput>>>,
}

impl fmt::Debug for DocumentAnalysisCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        formatter
            .debug_struct("DocumentAnalysisCoordinator")
            .field("registrations", &inner.registrations.len())
            .field("workers", &inner.workers.len())
            .field("documents", &inner.routes.len())
            .finish()
    }
}

impl Default for DocumentAnalysisCoordinator {
    fn default() -> Self {
        let max_validated_output_bytes = crate::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES
            .max(crate::perf::budgets::DIAGNOSTIC_PAYLOAD_BUDGET_BYTES);
        let output_capacity = DOCUMENT_ANALYSIS_OUTPUT_MAX_EVENTS
            .min(DOCUMENT_ANALYSIS_OUTPUT_MAX_BYTES / max_validated_output_bytes);
        let (outputs_tx, outputs_rx) = mpsc::channel(output_capacity);
        Self {
            inner: Arc::new(Mutex::new(CoordinatorInner {
                registrations: Vec::new(),
                workers: HashMap::new(),
                routes: HashMap::new(),
            })),
            outputs_tx,
            outputs_rx: Arc::new(tokio::sync::Mutex::new(outputs_rx)),
        }
    }
}

impl DocumentAnalysisCoordinator {
    pub(crate) fn register(
        &self,
        generation: u64,
        runtime: ClayJsRuntimeService,
        registration: JsDocumentAnalyzerRegistration,
        completion: &CompletionCoordinator,
        language_intelligence: &LanguageIntelligenceCoordinator,
    ) -> Result<(), String> {
        register_completion_providers(self, generation, &registration, completion)?;
        register_language_intelligence_providers(
            self,
            generation,
            &registration,
            language_intelligence,
        )?;
        let mut inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        if inner.registrations.iter().any(|registered| {
            registered.generation == generation
                && registered.registration.package.manifest.name
                    == registration.package.manifest.name
                && registered.registration.id == registration.id
        }) {
            return Err(format!(
                "analyzer `{}` is already registered",
                registration.id
            ));
        }
        inner.registrations.push(RegisteredAnalyzer {
            registration,
            runtime,
            generation,
        });
        Ok(())
    }

    pub(crate) fn open_document(
        &self,
        generation: u64,
        metadata: &DocumentMetadata,
        active_mode: &str,
        canonical_root_path: PathBuf,
        text: String,
    ) -> Vec<RuntimeDiagnostic> {
        if text.len() > DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES {
            return vec![analysis_status(
                "clay.analysis.document_too_large",
                "Document exceeds the package analysis limit; baseline language support remains active.",
            )];
        }
        let mut diagnostics = Vec::new();
        let mut inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        let matching = inner
            .registrations
            .iter()
            .filter(|registered| {
                registered.generation == generation
                    && (registered.registration.modes.is_empty()
                        || registered
                            .registration
                            .modes
                            .iter()
                            .any(|mode| mode == active_mode))
            })
            .map(|registered| {
                (
                    registered.registration.clone(),
                    registered.runtime.clone(),
                    registered.generation,
                )
            })
            .collect::<Vec<_>>();

        for (registration, runtime, generation) in matching {
            if !runtime.document_analysis_authorized(&registration, metadata.workspace_root_id) {
                diagnostics.push(analysis_status(
                    "clay.analysis.unauthorized",
                    "Document analyzer lacks a current package, parse, or language-server grant.",
                ));
                continue;
            }
            let key = WorkerKey {
                package_name: registration.package.manifest.name.clone(),
                contribution: registration.contribution.clone(),
                workspace_root_id: metadata.workspace_root_id,
                generation,
            };
            if inner
                .routes
                .get(&metadata.document_id)
                .is_some_and(|routes| routes.iter().any(|route| route.key == key))
            {
                continue;
            }
            if !inner.workers.contains_key(&key) {
                if inner.workers.len() >= DOCUMENT_ANALYSIS_MAX_WORKERS {
                    diagnostics.push(analysis_status(
                        "clay.analysis.worker_limit",
                        "Document analyzer worker limit reached; baseline language support remains active.",
                    ));
                    continue;
                }
                // Analysis invokes through the owning domain runtime (Plan
                // 061); no additional persistent JsRuntime is created per
                // analyzer/document. The mailbox/budget worker struct stays.
                let worker = spawn_worker(
                    runtime.clone(),
                    registration.clone(),
                    metadata.workspace_root_id,
                    self.outputs_tx.clone(),
                );
                inner.workers.insert(key.clone(), worker);
            }
            let worker = inner
                .workers
                .get(&key)
                .expect("analysis worker inserted above")
                .clone();
            let mut documents = worker
                .active_documents
                .lock()
                .expect("analysis document state lock poisoned");
            if documents.len() >= DOCUMENT_ANALYSIS_MAX_DOCUMENTS_PER_WORKER {
                diagnostics.push(analysis_status(
                    "clay.analysis.document_limit",
                    "Document analyzer document limit reached; baseline language support remains active.",
                ));
                continue;
            }
            let retained_bytes: usize =
                documents.values().map(|document| document.text_bytes).sum();
            if retained_bytes.saturating_add(text.len())
                > DOCUMENT_ANALYSIS_MAX_TEXT_BYTES_PER_WORKER
            {
                diagnostics.push(analysis_status(
                    "clay.analysis.text_limit",
                    "Document analyzer text limit reached; baseline language support remains active.",
                ));
                continue;
            }
            let event = DocumentAnalysisEvent::Open {
                document_id: metadata.document_id,
                document_version: metadata.version,
                runtime_generation: generation,
                active_mode: active_mode.to_string(),
                workspace_root_id: metadata.workspace_root_id,
                canonical_root_path: canonical_root_path.to_string_lossy().into_owned(),
                relative_path: metadata.path.clone(),
                text: text.clone(),
            };
            if worker.mailbox.push(event, None).is_err() {
                worker.active.store(false, Ordering::Release);
                worker.mailbox.close();
                diagnostics.push(analysis_status(
                    "clay.analysis.queue_limit",
                    "Document analyzer input queue is full; baseline language support remains active.",
                ));
                continue;
            }
            documents.insert(
                metadata.document_id,
                ActiveDocument {
                    version: metadata.version,
                    text_bytes: text.len(),
                },
            );
            drop(documents);
            inner
                .routes
                .entry(metadata.document_id)
                .or_default()
                .push(DocumentRoute { key });
        }
        diagnostics
    }

    pub(crate) fn change_document(
        &self,
        document_id: DocumentId,
        base_version: DocumentVersion,
        document_version: DocumentVersion,
        byte_start: u64,
        byte_end: u64,
        inserted_text: String,
    ) -> bool {
        let inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        let Some(routes) = inner.routes.get(&document_id) else {
            return false;
        };
        let mut needs_reset = inserted_text.len() > DOCUMENT_ANALYSIS_MAX_DELTA_BYTES;
        for route in routes {
            let Some(worker) = inner.workers.get(&route.key) else {
                continue;
            };
            if needs_reset {
                continue;
            }
            let event = DocumentAnalysisEvent::Change {
                document_id,
                base_version,
                document_version,
                byte_start,
                byte_end,
                inserted_text: inserted_text.clone(),
            };
            match worker.mailbox.push(event, None) {
                Ok(()) => {
                    if let Some(document) = worker
                        .active_documents
                        .lock()
                        .expect("analysis document state lock poisoned")
                        .get_mut(&document_id)
                    {
                        document.version = document_version;
                        document.text_bytes = document
                            .text_bytes
                            .saturating_sub((byte_end - byte_start) as usize)
                            .saturating_add(inserted_text.len());
                    }
                }
                Err(EnqueueError::Full) => needs_reset = true,
                Err(EnqueueError::Closed) => {}
            }
        }
        needs_reset
    }

    pub(crate) fn reset_document(
        &self,
        document_id: DocumentId,
        document_version: DocumentVersion,
        text: String,
    ) {
        if text.len() > DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES {
            self.close_document(document_id, document_version);
            return;
        }
        let inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        let Some(routes) = inner.routes.get(&document_id) else {
            return;
        };
        for route in routes {
            let Some(worker) = inner.workers.get(&route.key) else {
                continue;
            };
            let event = DocumentAnalysisEvent::Reset {
                document_id,
                document_version,
                text: text.clone(),
            };
            if worker.mailbox.coalesce_reset(event).is_ok() {
                if let Some(document) = worker
                    .active_documents
                    .lock()
                    .expect("analysis document state lock poisoned")
                    .get_mut(&document_id)
                {
                    document.version = document_version;
                    document.text_bytes = text.len();
                }
            } else {
                worker.active.store(false, Ordering::Release);
                worker.mailbox.close();
            }
        }
    }

    pub(crate) fn close_document(
        &self,
        document_id: DocumentId,
        document_version: DocumentVersion,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        let Some(routes) = inner.routes.remove(&document_id) else {
            return;
        };
        let mut empty_workers = Vec::new();
        for route in routes {
            let Some(worker) = inner.workers.get(&route.key) else {
                continue;
            };
            worker
                .active_documents
                .lock()
                .expect("analysis document state lock poisoned")
                .remove(&document_id);
            let _ = worker.mailbox.push(
                DocumentAnalysisEvent::Close {
                    document_id,
                    document_version,
                },
                None,
            );
            if worker
                .active_documents
                .lock()
                .expect("analysis document state lock poisoned")
                .is_empty()
            {
                let _ = worker.mailbox.push(DocumentAnalysisEvent::Shutdown, None);
                empty_workers.push(route.key);
            }
        }
        for key in empty_workers {
            inner.workers.remove(&key);
        }
    }

    pub(crate) fn cancel_package(&self, package_name: &str) {
        let keys = {
            let inner = self
                .inner
                .lock()
                .expect("analysis coordinator lock poisoned");
            inner
                .workers
                .keys()
                .filter(|key| key.package_name == package_name)
                .cloned()
                .collect::<Vec<_>>()
        };
        self.cancel_workers(keys);
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "workspace-root removal hook is retained for the approved lifecycle even though roots are append-only today"
        )
    )]
    pub(crate) fn cancel_root(&self, root_id: WorkspaceRootId) {
        let keys = {
            let inner = self
                .inner
                .lock()
                .expect("analysis coordinator lock poisoned");
            inner
                .workers
                .keys()
                .filter(|key| key.workspace_root_id == root_id)
                .cloned()
                .collect::<Vec<_>>()
        };
        self.cancel_workers(keys);
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "exact-generation cancel remains for tests; production uses cancel_older_generations"
        )
    )]
    pub(crate) fn cancel_generation(&self, generation: u64) {
        self.cancel_older_generations(generation.saturating_add(1));
    }

    /// After a successful runtime-generation commit, shut down every worker and
    /// registration older than `active_generation`, then drain already-queued
    /// outputs so late old-generation decorations/diagnostics cannot publish.
    pub(crate) fn cancel_older_generations(&self, active_generation: u64) {
        let keys = {
            let inner = self
                .inner
                .lock()
                .expect("analysis coordinator lock poisoned");
            inner
                .workers
                .keys()
                .filter(|key| key.generation < active_generation)
                .cloned()
                .collect::<Vec<_>>()
        };
        self.cancel_workers(keys);
        self.inner
            .lock()
            .expect("analysis coordinator lock poisoned")
            .registrations
            .retain(|registration| registration.generation >= active_generation);
        self.drain_pending_outputs();
    }

    /// Drop already-queued analysis outputs without waiting.
    pub(crate) fn drain_pending_outputs(&self) {
        if let Ok(mut outputs) = self.outputs_rx.try_lock() {
            while outputs.try_recv().is_ok() {}
        }
    }

    /// Snapshot of analyzer registration generations retained after cleanup.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "generation introspection is used by reload cleanup tests"
        )
    )]
    pub(crate) fn registered_generations(&self) -> Vec<u64> {
        let mut generations = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned")
            .registrations
            .iter()
            .map(|registration| registration.generation)
            .collect::<Vec<_>>();
        generations.sort_unstable();
        generations.dedup();
        generations
    }

    /// Snapshot of live worker generations retained after cleanup.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "generation introspection is used by reload cleanup tests"
        )
    )]
    pub(crate) fn worker_generations(&self) -> Vec<u64> {
        let mut generations = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned")
            .workers
            .keys()
            .map(|key| key.generation)
            .collect::<Vec<_>>();
        generations.sort_unstable();
        generations.dedup();
        generations
    }

    fn cancel_workers(&self, keys: Vec<WorkerKey>) {
        let mut inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        for key in &keys {
            if let Some(worker) = inner.workers.remove(key) {
                worker.active.store(false, Ordering::Release);
                if worker
                    .mailbox
                    .push(DocumentAnalysisEvent::Shutdown, None)
                    .is_err()
                {
                    worker.mailbox.close();
                }
            }
        }
        inner.routes.retain(|_, routes| {
            routes.retain(|route| !keys.contains(&route.key));
            !routes.is_empty()
        });
    }

    pub(crate) fn active_completion_provider_ids(&self, document_id: DocumentId) -> Vec<String> {
        let inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        let Some(routes) = inner.routes.get(&document_id) else {
            return Vec::new();
        };
        inner
            .registrations
            .iter()
            .filter(|registered| {
                routes.iter().any(|route| {
                    route.key.package_name == registered.registration.package.manifest.name
                        && route.key.contribution == registered.registration.contribution
                        && route.key.generation == registered.generation
                })
            })
            .flat_map(|registered| {
                registered
                    .registration
                    .package
                    .contributions
                    .completion_providers
                    .iter()
                    .map(|provider| provider.id.clone())
            })
            .collect()
    }

    pub(crate) async fn next_output(&self) -> Option<DocumentAnalysisOutput> {
        self.outputs_rx.lock().await.recv().await
    }

    async fn request_completion(
        &self,
        package_name: &str,
        request: CompletionRequest,
        window: CompletionDocumentWindow,
    ) -> Result<CompletionResultSet, CompletionProviderError> {
        let (worker, registration) = self.worker_for_document(package_name, request.document_id)?;
        acquire_request_slot(&worker.pending_requests)
            .map_err(CompletionProviderError::ProviderFailed)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        let pushed = worker.mailbox.push(
            DocumentAnalysisEvent::Completion { request, window },
            Some(AnalysisReply::Completion(reply_tx)),
        );
        if pushed.is_err() {
            worker.pending_requests.fetch_sub(1, Ordering::AcqRel);
            return Err(CompletionProviderError::ProviderFailed(
                "document analyzer input queue unavailable".to_string(),
            ));
        }
        let result = reply_rx.await.map_err(|_| {
            CompletionProviderError::ProviderFailed(format!(
                "document analyzer `{}` stopped",
                registration.id
            ))
        })?;
        worker.pending_requests.fetch_sub(1, Ordering::AcqRel);
        result
    }

    async fn request_language_intelligence(
        &self,
        package_name: &str,
        request: LanguageIntelligenceRequest,
        window: LanguageIntelligenceDocumentWindow,
    ) -> Result<LanguageIntelligenceResult, LanguageIntelligenceProviderError> {
        let (worker, registration) = self
            .worker_for_document(package_name, request.document_id)
            .map_err(|error| match error {
                CompletionProviderError::ProviderFailed(message) => {
                    LanguageIntelligenceProviderError::ProviderFailed(message)
                }
                CompletionProviderError::Timeout => LanguageIntelligenceProviderError::Timeout,
            })?;
        acquire_request_slot(&worker.pending_requests)
            .map_err(LanguageIntelligenceProviderError::ProviderFailed)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        let pushed = worker.mailbox.push(
            DocumentAnalysisEvent::LanguageIntelligence { request, window },
            Some(AnalysisReply::LanguageIntelligence(reply_tx)),
        );
        if pushed.is_err() {
            worker.pending_requests.fetch_sub(1, Ordering::AcqRel);
            return Err(LanguageIntelligenceProviderError::ProviderFailed(
                "document analyzer input queue unavailable".to_string(),
            ));
        }
        let result = reply_rx.await.map_err(|_| {
            LanguageIntelligenceProviderError::ProviderFailed(format!(
                "document analyzer `{}` stopped",
                registration.id
            ))
        })?;
        worker.pending_requests.fetch_sub(1, Ordering::AcqRel);
        result
    }

    fn worker_for_document(
        &self,
        package_name: &str,
        document_id: DocumentId,
    ) -> Result<(AnalysisWorker, JsDocumentAnalyzerRegistration), CompletionProviderError> {
        let inner = self
            .inner
            .lock()
            .expect("analysis coordinator lock poisoned");
        let route = inner
            .routes
            .get(&document_id)
            .and_then(|routes| {
                routes
                    .iter()
                    .find(|route| route.key.package_name == package_name)
            })
            .ok_or_else(|| {
                CompletionProviderError::ProviderFailed(
                    "document is not synchronized with this analyzer".to_string(),
                )
            })?;
        let worker = inner.workers.get(&route.key).cloned().ok_or_else(|| {
            CompletionProviderError::ProviderFailed("document analyzer stopped".to_string())
        })?;
        let registration = inner
            .registrations
            .iter()
            .find(|registration| {
                registration.generation == route.key.generation
                    && registration.registration.package.manifest.name == route.key.package_name
                    && registration.registration.contribution == route.key.contribution
            })
            .map(|registration| registration.registration.clone())
            .ok_or_else(|| {
                CompletionProviderError::ProviderFailed(
                    "document analyzer registration is stale".to_string(),
                )
            })?;
        Ok((worker, registration))
    }
}

fn acquire_request_slot(pending: &AtomicUsize) -> Result<(), String> {
    pending
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < DOCUMENT_ANALYSIS_MAX_PENDING_REQUESTS).then_some(current + 1)
        })
        .map(|_| ())
        .map_err(|_| "document analyzer request limit reached".to_string())
}

fn spawn_worker(
    runtime: ClayJsRuntimeService,
    registration: JsDocumentAnalyzerRegistration,
    workspace_root_id: WorkspaceRootId,
    outputs: mpsc::Sender<DocumentAnalysisOutput>,
) -> AnalysisWorker {
    let mailbox = Arc::new(AnalysisMailbox::default());
    let active_documents = Arc::new(Mutex::new(HashMap::new()));
    let active = Arc::new(AtomicBool::new(true));
    let worker = AnalysisWorker {
        mailbox: Arc::clone(&mailbox),
        active_documents: Arc::clone(&active_documents),
        active: Arc::clone(&active),
        pending_requests: Arc::new(AtomicUsize::new(0)),
    };
    tokio::spawn(async move {
        while let Some(queued) = mailbox.pop().await {
            let shutdown = matches!(queued.event, DocumentAnalysisEvent::Shutdown);
            if !runtime.document_analysis_authorized(&registration, workspace_root_id) {
                send_reply_error(queued.reply, "document analyzer authority was revoked");
                active.store(false, Ordering::Release);
                break;
            }
            let invocation =
                runtime.invoke_document_analyzer(registration.clone(), queued.event.clone());
            let invocation = if shutdown {
                tokio::time::timeout(
                    std::time::Duration::from_millis(DOCUMENT_ANALYSIS_TOTAL_SHUTDOWN_MS),
                    invocation,
                )
                .await
                .map_err(|_| crate::server::js_runtime::ClayRuntimeError::Timeout)
                .and_then(|result| result)
            } else {
                invocation.await
            };
            match invocation {
                Ok(invocation) => {
                    let (response, output_failed) = publish_invocation_outputs(
                        &registration.package,
                        &active_documents,
                        &outputs,
                        invocation,
                    );
                    if output_failed {
                        send_reply_error(
                            queued.reply,
                            "document analyzer output queue limit reached",
                        );
                        active.store(false, Ordering::Release);
                        break;
                    }
                    send_reply(queued.reply, response);
                }
                Err(error) => {
                    send_reply_error(queued.reply, &error.to_string());
                    let _ = outputs.try_send(DocumentAnalysisOutput::Diagnostic(analysis_status(
                        "clay.analysis.worker_failed",
                        "Document analyzer stopped; baseline language support remains active.",
                    )));
                    active.store(false, Ordering::Release);
                    break;
                }
            }
            if shutdown {
                break;
            }
        }
        mailbox.close();
        active.store(false, Ordering::Release);
    });
    worker
}

fn publish_invocation_outputs(
    package: &PackageRecord,
    active_documents: &Mutex<HashMap<DocumentId, ActiveDocument>>,
    outputs: &mpsc::Sender<DocumentAnalysisOutput>,
    invocation: DocumentAnalysisInvocation,
) -> (DocumentAnalysisResponse, bool) {
    let mut output_failed = false;
    if let Some(set) = invocation.decorations {
        let current = active_documents
            .lock()
            .expect("analysis document state lock poisoned")
            .get(&set.document_id)
            .copied();
        if current.is_some_and(|document| document.version == set.document_version)
            && let Ok(set) = validate_decoration_publication(package, set.document_version, set)
        {
            output_failed |= outputs
                .try_send(DocumentAnalysisOutput::Decorations(set))
                .is_err();
        }
    }
    if let Some(set) = invocation.diagnostics {
        let current = active_documents
            .lock()
            .expect("analysis document state lock poisoned")
            .get(&set.document_id)
            .copied();
        if current.is_some_and(|document| document.version == set.document_version)
            && let Ok(set) = validate_diagnostic_publication(package, set.document_version, set)
        {
            output_failed |= outputs
                .try_send(DocumentAnalysisOutput::Diagnostics(set))
                .is_err();
        }
    }
    (invocation.response, output_failed)
}

fn send_reply(reply: Option<AnalysisReply>, response: DocumentAnalysisResponse) {
    match (reply, response) {
        (Some(AnalysisReply::Completion(reply)), DocumentAnalysisResponse::Completion(result)) => {
            let _ = reply.send(result);
        }
        (
            Some(AnalysisReply::LanguageIntelligence(reply)),
            DocumentAnalysisResponse::LanguageIntelligence(result),
        ) => {
            let _ = reply.send(result);
        }
        (Some(reply), _) => send_reply_error(
            Some(reply),
            "document analyzer returned the wrong response kind",
        ),
        (None, _) => {}
    }
}

fn send_reply_error(reply: Option<AnalysisReply>, message: &str) {
    match reply {
        Some(AnalysisReply::Completion(reply)) => {
            let _ = reply.send(Err(CompletionProviderError::ProviderFailed(
                message.to_string(),
            )));
        }
        Some(AnalysisReply::LanguageIntelligence(reply)) => {
            let _ = reply.send(Err(LanguageIntelligenceProviderError::ProviderFailed(
                message.to_string(),
            )));
        }
        None => {}
    }
}

fn analysis_status(code: &'static str, message: &'static str) -> RuntimeDiagnostic {
    RuntimeDiagnostic::error(code, message)
}

fn register_completion_providers(
    analysis: &DocumentAnalysisCoordinator,
    generation: u64,
    registration: &JsDocumentAnalyzerRegistration,
    coordinator: &CompletionCoordinator,
) -> Result<(), String> {
    for descriptor in &registration.package.contributions.completion_providers {
        let provenance = CompletionProvenance {
            package_name: registration.package.manifest.name.clone(),
            package_version: registration.package.manifest.version.clone(),
            package_prefix: registration.package.manifest.clay.api_prefix.clone(),
        };
        let meta = CompletionProviderMeta {
            id: descriptor.id.clone(),
            provenance: provenance.clone(),
            priority: descriptor.priority,
            exclusive: descriptor.exclusive,
            trigger_metadata: CompletionTriggerMetadata {
                trigger_characters: descriptor.trigger_characters.clone(),
            },
            word_boundary: if descriptor.word_boundary_chars.is_empty() {
                WordBoundaryRule::default_buffer_word()
            } else {
                WordBoundaryRule::new(descriptor.word_boundary_chars.clone())
            },
            items: descriptor
                .items
                .iter()
                .map(|item| CompletionItem {
                    label: item.label.clone(),
                    insert_text: item.insert_text.clone(),
                    detail: item.detail.clone(),
                    commit_characters: String::new(),
                    text_format: item.text_format,
                    provenance: provenance.clone(),
                })
                .collect(),
            timeout_ms: descriptor.timeout_ms,
            max_items: descriptor.max_items,
            generation,
        };
        coordinator
            .register_package_for_generation(
                &registration.package,
                meta,
                AnalysisCompletionProvider {
                    coordinator: analysis.clone(),
                    package_name: registration.package.manifest.name.clone(),
                },
            )
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

fn register_language_intelligence_providers(
    analysis: &DocumentAnalysisCoordinator,
    generation: u64,
    registration: &JsDocumentAnalyzerRegistration,
    coordinator: &LanguageIntelligenceCoordinator,
) -> Result<(), String> {
    for descriptor in &registration
        .package
        .contributions
        .language_intelligence_providers
    {
        let meta = LanguageIntelligenceProviderMeta {
            id: descriptor.id.clone(),
            provenance: CompletionProvenance {
                package_name: registration.package.manifest.name.clone(),
                package_version: registration.package.manifest.version.clone(),
                package_prefix: registration.package.manifest.clay.api_prefix.clone(),
            },
            modes: descriptor.modes.clone(),
            features: descriptor.features.clone(),
            priority: descriptor.priority,
            timeout_ms: descriptor.timeout_ms,
            generation,
        };
        coordinator
            .register_package_for_generation(
                &registration.package,
                meta,
                AnalysisLanguageIntelligenceProvider {
                    coordinator: analysis.clone(),
                    package_name: registration.package.manifest.name.clone(),
                },
            )
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

struct AnalysisCompletionProvider {
    coordinator: DocumentAnalysisCoordinator,
    package_name: String,
}

impl CompletionProvider for AnalysisCompletionProvider {
    fn complete(
        &self,
        request: CompletionRequest,
        window: CompletionDocumentWindow,
    ) -> CompletionProviderFuture {
        let coordinator = self.coordinator.clone();
        let package_name = self.package_name.clone();
        Box::pin(async move {
            coordinator
                .request_completion(&package_name, request, window)
                .await
        })
    }
}

struct AnalysisLanguageIntelligenceProvider {
    coordinator: DocumentAnalysisCoordinator,
    package_name: String,
}

impl LanguageIntelligenceProvider for AnalysisLanguageIntelligenceProvider {
    fn provide(
        &self,
        request: LanguageIntelligenceRequest,
        window: LanguageIntelligenceDocumentWindow,
    ) -> LanguageIntelligenceProviderFuture {
        let coordinator = self.coordinator.clone();
        let package_name = self.package_name.clone();
        Box::pin(async move {
            coordinator
                .request_language_intelligence(&package_name, request, window)
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

    use serde_json::json;

    use super::*;
    use crate::{
        packages::{
            authorization::RuntimeProfile, permissions::PackagePermission,
            record::assemble_package_record,
        },
        protocol::{
            CompletionReplacementRange, CompletionTrigger, DocumentAccess,
            LanguageIntelligenceFeature, LanguageIntelligencePayload,
        },
    };

    fn fixture_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-document-analysis-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn package_value(executable: &str) -> serde_json::Value {
        json!({
            "name": "@vendor/analysis",
            "version": "1.0.0",
            "type": "module",
            "exports": { ".": "./analyzer.js" },
            "clay": {
                "apiPrefix": "analysis",
                "entry": "./analyzer.js",
                "loadEntry": "./analyzer.js",
                "permissions": [
                    "parse-document",
                    "completion-provider",
                    "render-decorations"
                ],
                "capabilities": ["language-server"],
                "modes": [],
                "docs": "./docs.md",
                "contributions": {
                    "languageServers": [{
                        "id": "analysis.server",
                        "executable": executable,
                        "args": [],
                        "inheritEnvironment": []
                    }],
                    "completionProviders": [{
                        "id": "analysis.completion",
                        "priority": 100,
                        "triggerCharacters": ["."],
                        "items": [],
                        "budgets": { "timeoutMs": 1000, "maxItems": 16 }
                    }],
                    "languageIntelligenceProviders": [{
                        "id": "analysis.intelligence",
                        "modes": ["test"],
                        "features": ["hover", "definition", "codeAction", "signatureHelp"],
                        "priority": 100,
                        "module": "./analyzer.js",
                        "exportName": "handleDocumentAnalysis",
                        "timeoutMs": 1000
                    }]
                }
            }
        })
    }

    fn analyzer_source(package: &serde_json::Value) -> String {
        format!(
            r#"
import {{ serverPublishDecorations }} from "clay:decorations";
import {{ serverPublishDiagnostics }} from "clay:diagnostics";
const manifest = {package};
const documents = new Map();
export async function handleDocumentAnalysis(event) {{
  if (event.kind === "open" || event.kind === "reset") {{
    documents.set(event.documentId, {{ text: event.text, version: event.documentVersion }});
  }} else if (event.kind === "change") {{
    const document = documents.get(event.documentId);
    document.text = document.text.slice(0, event.byteStart) + event.insertedText + document.text.slice(event.byteEnd);
    document.version = event.documentVersion;
    serverPublishDiagnostics({{
      packageManifest: manifest,
      documentId: event.documentId,
      documentVersion: event.documentVersion,
      currentDocumentVersion: event.documentVersion,
      viewport: {{ byteStart: 0, byteEnd: Math.max(1, document.text.length) }},
      source: "fake-analyzer",
      spans: [{{ byteStart: 0, byteEnd: 1, severity: "warning", code: "fake", message: "fake warning" }}],
    }});
  }} else if (event.kind === "close") {{
    documents.delete(event.documentId);
  }}
  if (event.kind === "reset" && event.text === "stale") {{
    serverPublishDecorations({{
      packageManifest: manifest,
      documentId: event.documentId,
      documentVersion: event.documentVersion - 1,
      currentDocumentVersion: event.documentVersion - 1,
      viewport: {{ byteStart: 0, byteEnd: event.text.length }},
      spans: [{{ byteStart: 0, byteEnd: 1, kind: "semantic", tokenType: "Function", priority: 100 }}],
    }});
  }}
  if (event.kind === "open" && event.text.length > 0) {{
    serverPublishDecorations({{
      packageManifest: manifest,
      documentId: event.documentId,
      documentVersion: event.documentVersion,
      currentDocumentVersion: event.documentVersion,
      viewport: {{ byteStart: 0, byteEnd: event.text.length }},
      spans: [{{ byteStart: 0, byteEnd: 1, kind: "semantic", tokenType: "Function", priority: 100 }}],
    }});
  }}
  if (event.kind === "completion") {{
    const document = documents.get(event.request.documentId);
    return {{ status: "ok", items: [{{ label: "dynamic", insertText: "dynamic", detail: document.text }}] }};
  }}
  if (event.kind === "languageIntelligence") {{
    const document = documents.get(event.request.documentId);
    return {{ status: "ok", hover: {{ markdown: document.text }} }};
  }}
  return null;
}}
"#,
            package = package
        )
    }

    fn configured_runtime(
        name: &str,
    ) -> (
        ClayJsRuntimeService,
        JsDocumentAnalyzerRegistration,
        PathBuf,
    ) {
        let root = fixture_root(name);
        let executable = root.join("fake-server");
        fs::write(&executable, "#!/bin/sh\nexec /bin/cat\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let canonical_executable = fs::canonicalize(&executable).unwrap();
        let package_json = package_value(executable.to_str().unwrap());
        let package = assemble_package_record(&package_json).unwrap();
        let module_path = root.join("analyzer.js");
        fs::write(&module_path, analyzer_source(&package_json)).unwrap();
        fs::write(root.join("docs.md"), "# fixture\n").unwrap();

        let runtime = ClayJsRuntimeService::default();
        let op_state = runtime.test_op_state();
        {
            let mut service = op_state.package_service().lock().unwrap();
            service
                .install_from_value_at_root_with_spec(
                    package_json,
                    root.clone(),
                    "local:analysis-fixture",
                )
                .unwrap();
            service
                .authorize_package(
                    "@vendor/analysis",
                    vec![
                        PackagePermission::ParseDocument,
                        PackagePermission::CompletionProvider,
                        PackagePermission::RenderDecorations,
                    ],
                    RuntimeProfile::Restricted,
                    "test",
                )
                .unwrap();
            service
                .authorize_language_server(
                    "@vendor/analysis",
                    "analysis.server",
                    canonical_executable,
                    vec![1],
                    "test",
                )
                .unwrap();
            service.approve_package("@vendor/analysis", "test").unwrap();
            service.enable("@vendor/analysis").unwrap();
        }
        let module_specifier = "clay://packages/@vendor/analysis/analyzer.js";
        op_state.load_entry_allowlist().record_for_package(
            module_specifier,
            fs::canonicalize(&module_path).unwrap(),
            fs::canonicalize(&root).unwrap(),
            Some("@vendor/analysis"),
        );
        (
            runtime,
            JsDocumentAnalyzerRegistration {
                package,
                id: "analysis.worker".to_string(),
                contribution: "analysis.server".to_string(),
                modes: vec!["test".to_string()],
                module_specifier: module_specifier.to_string(),
                export_name: "handleDocumentAnalysis".to_string(),
            },
            root,
        )
    }

    fn metadata(version: u64) -> DocumentMetadata {
        DocumentMetadata {
            document_id: 7,
            version,
            access: DocumentAccess::ReadOnly,
            lease_id: None,
            dirty: false,
            workspace_root_id: 1,
            path: "main.test".to_string(),
        }
    }

    fn completion_request(version: u64) -> CompletionRequest {
        CompletionRequest {
            request_id: 9,
            client_id: 2,
            document_id: 7,
            document_version: version,
            behavior_version: 3,
            cursor_byte_offset: 4,
            replacement_range: CompletionReplacementRange {
                byte_start: 0,
                byte_end: 0,
            },
            trigger: CompletionTrigger::Manual,
            provider_generation: 1,
        }
    }

    fn completion_window(version: u64, text: &str) -> CompletionDocumentWindow {
        CompletionDocumentWindow {
            document_id: 7,
            document_version: version,
            behavior_version: 3,
            package_prefix: "analysis".to_string(),
            byte_start: 0,
            byte_end: text.len() as u64,
            text: text.to_string(),
        }
    }

    #[tokio::test]
    async fn resolver_owned_module_registers_through_language_facade() {
        let (runtime, registration, _root) = configured_runtime("registration");
        // Host-stamped provenance: the evaluation runs under the enabled
        // package's context; no caller manifest is involved.
        let source = r#"
            import { serverRegisterDocumentAnalyzer } from "clay:language";
            const registration = serverRegisterDocumentAnalyzer({
              analyzer: {
                id: "analysis.worker",
                contribution: "analysis.server",
                modes: ["test"],
                moduleSpecifier: "clay://packages/@vendor/analysis/analyzer.js",
                exportName: "handleDocumentAnalysis"
              }
            });
            Deno.core.ops.op_clay_runtime_record(registration.analyzerId);
            "#
        .to_string();

        let evaluation = runtime
            .evaluate_entry_as_package(
                crate::packages::bundled::RuntimeDomain::Trusted,
                &registration.package,
                crate::server::js_runtime::RuntimeEntry::ControlledSource(source),
                "runtime.evaluate_as_package",
            )
            .await
            .unwrap();

        assert_eq!(evaluation.op_records, ["analysis.worker"]);
        assert_eq!(evaluation.document_analyzers.len(), 1);
    }

    #[tokio::test]
    async fn worker_preserves_open_change_outputs_requests_and_close_lifecycle() {
        let (runtime, registration, root) = configured_runtime("lifecycle");
        let runtime_probe = runtime.clone();
        let coordinator = DocumentAnalysisCoordinator::default();
        let completion = CompletionCoordinator::new();
        let intelligence = LanguageIntelligenceCoordinator::new();
        coordinator
            .register(1, runtime, registration, &completion, &intelligence)
            .unwrap();
        assert!(
            coordinator
                .open_document(1, &metadata(1), "test", root, "fn".to_string())
                .is_empty()
        );
        let output = tokio::time::timeout(Duration::from_secs(2), coordinator.next_output())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(output, DocumentAnalysisOutput::Decorations(_)));
        // Plan 061 task 4: analyzer registration, document open, and analysis
        // invocation never create additional persistent runtimes beyond the
        // two domain workers.
        assert_eq!(runtime_probe.workers_started(), 2);

        assert!(!coordinator.change_document(7, 1, 2, 2, 2, " x".to_string()));
        let output = tokio::time::timeout(Duration::from_secs(2), coordinator.next_output())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(output, DocumentAnalysisOutput::Diagnostics(_)));

        let completion_result = coordinator
            .request_completion(
                "@vendor/analysis",
                completion_request(2),
                completion_window(2, "fn x"),
            )
            .await
            .unwrap();
        assert_eq!(completion_result.items[0].detail, "fn x");

        let request = LanguageIntelligenceRequest {
            request_id: 10,
            client_id: 2,
            document_id: 7,
            document_version: 2,
            behavior_version: 3,
            cursor_byte_offset: 1,
            feature: LanguageIntelligenceFeature::Hover,
            provider_generation: 1,
        };
        let result = coordinator
            .request_language_intelligence(
                "@vendor/analysis",
                request,
                LanguageIntelligenceDocumentWindow {
                    document_id: 7,
                    document_version: 2,
                    behavior_version: 3,
                    byte_start: 0,
                    byte_end: 4,
                    text: "fn x".to_string(),
                    active_mode: "test".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(matches!(
            result.payload,
            LanguageIntelligencePayload::Hover(ref hover) if hover.markdown == "fn x"
        ));

        coordinator.close_document(7, 2);
        assert!(
            coordinator
                .request_completion(
                    "@vendor/analysis",
                    completion_request(2),
                    completion_window(2, "fn x"),
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn stale_live_output_is_dropped_after_newer_reset() {
        let (runtime, registration, root) = configured_runtime("stale-output");
        let coordinator = DocumentAnalysisCoordinator::default();
        coordinator
            .register(
                1,
                runtime,
                registration,
                &CompletionCoordinator::new(),
                &LanguageIntelligenceCoordinator::new(),
            )
            .unwrap();
        coordinator.open_document(1, &metadata(1), "test", root, "fn".to_string());
        let _ = tokio::time::timeout(Duration::from_secs(2), coordinator.next_output())
            .await
            .unwrap();

        coordinator.reset_document(7, 2, "stale".to_string());

        assert!(
            tokio::time::timeout(Duration::from_millis(100), coordinator.next_output())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn grant_revocation_stops_worker_and_prevents_reopen() {
        let (runtime, registration, root) = configured_runtime("revocation");
        let coordinator = DocumentAnalysisCoordinator::default();
        coordinator
            .register(
                1,
                runtime.clone(),
                registration,
                &CompletionCoordinator::new(),
                &LanguageIntelligenceCoordinator::new(),
            )
            .unwrap();
        assert!(
            coordinator
                .open_document(1, &metadata(1), "test", root.clone(), "fn".to_string())
                .is_empty()
        );
        let _ = tokio::time::timeout(Duration::from_secs(2), coordinator.next_output())
            .await
            .unwrap();

        runtime
            .test_op_state()
            .package_service()
            .lock()
            .unwrap()
            .revoke_language_server_grants("@vendor/analysis");
        coordinator.cancel_package("@vendor/analysis");
        let diagnostics =
            coordinator.open_document(1, &metadata(1), "test", root, "fn".to_string());

        assert_eq!(diagnostics[0].code, "clay.analysis.unauthorized");
        assert!(coordinator.active_completion_provider_ids(7).is_empty());
    }

    #[tokio::test]
    async fn root_and_generation_cancellation_remove_document_routes() {
        for cancel_generation in [false, true] {
            let (runtime, registration, root) = configured_runtime(if cancel_generation {
                "generation-cancellation"
            } else {
                "root-cancellation"
            });
            let coordinator = DocumentAnalysisCoordinator::default();
            coordinator
                .register(
                    1,
                    runtime,
                    registration,
                    &CompletionCoordinator::new(),
                    &LanguageIntelligenceCoordinator::new(),
                )
                .unwrap();
            assert!(
                coordinator
                    .open_document(1, &metadata(1), "test", root, "fn".to_string())
                    .is_empty()
            );
            let _ = tokio::time::timeout(Duration::from_secs(2), coordinator.next_output())
                .await
                .unwrap();
            assert!(!coordinator.active_completion_provider_ids(7).is_empty());

            if cancel_generation {
                coordinator.cancel_generation(1);
            } else {
                coordinator.cancel_root(1);
            }

            assert!(coordinator.active_completion_provider_ids(7).is_empty());
            assert!(
                coordinator
                    .request_completion(
                        "@vendor/analysis",
                        completion_request(1),
                        completion_window(1, "fn"),
                    )
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn oversize_document_never_starts_worker_or_request_route() {
        let (runtime, registration, root) = configured_runtime("oversize");
        let coordinator = DocumentAnalysisCoordinator::default();
        coordinator
            .register(
                1,
                runtime,
                registration,
                &CompletionCoordinator::new(),
                &LanguageIntelligenceCoordinator::new(),
            )
            .unwrap();
        let diagnostics = coordinator.open_document(
            1,
            &metadata(1),
            "test",
            root,
            "x".repeat(DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES + 1),
        );
        assert_eq!(diagnostics[0].code, "clay.analysis.document_too_large");
        assert!(
            coordinator
                .request_completion(
                    "@vendor/analysis",
                    completion_request(1),
                    completion_window(1, "x"),
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn mailbox_pressure_coalesces_document_work_to_latest_reset() {
        let mailbox = AnalysisMailbox::default();
        for version in 1..=DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS {
            mailbox
                .push(
                    DocumentAnalysisEvent::Change {
                        document_id: 7,
                        base_version: version.saturating_sub(1) as u64,
                        document_version: version as u64,
                        byte_start: 0,
                        byte_end: 0,
                        inserted_text: "x".to_string(),
                    },
                    None,
                )
                .unwrap();
        }
        mailbox
            .coalesce_reset(DocumentAnalysisEvent::Reset {
                document_id: 7,
                document_version: 65,
                text: "latest".to_string(),
            })
            .unwrap();
        let queued = mailbox.pop().await.unwrap();
        assert!(matches!(
            queued.event,
            DocumentAnalysisEvent::Reset {
                document_version: 65,
                ref text,
                ..
            } if text == "latest"
        ));
    }
}
