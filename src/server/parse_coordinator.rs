use std::{
    collections::HashMap,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Instant,
};

use tokio::sync::mpsc;

use crate::{
    packages::{permissions::PackagePermission, record::PackageRecord},
    perf::{
        budgets::{
            INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
            INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES, SYNTAX_CACHE_BUDGET_BYTES,
        },
        metrics::{
            MetricMetadata, MetricValue, PerfRecorder, SYNTAX_CANCELLED_SUPERSEDED,
            SYNTAX_DECORATION_CHUNKS, SYNTAX_EDIT_TO_PUBLISH, SYNTAX_END,
            SYNTAX_LOGICAL_WORK_ITEMS, SYNTAX_QUEUE, SYNTAX_START, global_recorder,
        },
    },
    protocol::{
        BehaviorVersion, ClientId, DocumentId, DocumentVersion, IncrementalParseUpdate,
        ParseByteRange, ParseEditNotification, ParseInputEdit, ParsePoint, ParsePolicy, ParseUnit,
        ParseWindowSnapshot, RuntimeDiagnostic, SyntaxMemoryBudget,
    },
    server::{
        decorations::validate_decoration_set,
        diagnostics::validate_diagnostic_set,
        syntax_session::{self, SyntaxExecutor},
    },
};

pub type ParseHandlerFuture =
    Pin<Box<dyn Future<Output = Result<IncrementalParseUpdate, ParseCoordinatorError>> + Send>>;

/// Typed server-side boundary for package parse handlers.
///
/// Implementations may call the constrained JavaScript runtime, but the Rust
/// client never receives or executes parser code.
pub trait ParseHandler: Send + Sync + 'static {
    fn parse(&self, notification: ParseEditNotification) -> ParseHandlerFuture;

    /// Plan 099: synchronous CPU-bound parse for handlers that must run on
    /// the bounded blocking executor instead of a Tokio worker thread. Return
    /// `None` when the handler is async-only (package JavaScript); the
    /// session worker then awaits [`ParseHandler::parse`] normally.
    fn parse_blocking(
        &self,
        _notification: ParseEditNotification,
    ) -> Option<Result<IncrementalParseUpdate, ParseCoordinatorError>> {
        None
    }

    /// Whether this handler's CPU-bound work must stay off the async runtime.
    /// Blocking handlers run inside per-document syntax sessions on the
    /// shared bounded blocking executor.
    fn runs_on_blocking_executor(&self) -> bool {
        false
    }

    /// Plan 071 task 10: read-only tree-sitter text-object/smart-select byte
    /// ranges for one selection query. Default `None` = this handler cannot
    /// answer (JS parse handlers); the server then returns an empty result.
    /// Implementations must never mutate the document or spawn external work.
    fn selection_query_ranges(
        &self,
        _document_id: crate::protocol::DocumentId,
        _document_version: u64,
        _text: &str,
        _query: crate::protocol::SelectionQuery,
        _selections: &[crate::protocol::SelectionQueryCursor],
    ) -> Option<Vec<Option<crate::protocol::SelectionQueryRange>>> {
        None
    }
}

impl<F, Fut> ParseHandler for F
where
    F: Fn(ParseEditNotification) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<IncrementalParseUpdate, ParseCoordinatorError>> + Send + 'static,
{
    fn parse(&self, notification: ParseEditNotification) -> ParseHandlerFuture {
        Box::pin(self(notification))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseCoordinatorError {
    MissingPermission {
        package_prefix: String,
    },
    HandlerAlreadyRegistered {
        package_prefix: String,
        mode_id: String,
    },
    HandlerNotRegistered {
        package_prefix: String,
        mode_id: String,
    },
    InvalidViewportRange,
    InvalidatedRangeInvalid {
        index: usize,
    },
    InvalidAcceptedEdit,
    InvalidParsePolicy,
    InvalidWindowRange {
        index: usize,
    },
    WindowMetadataMismatch {
        index: usize,
    },
    WindowTextLengthMismatch {
        index: usize,
        expected: usize,
        actual: usize,
    },
    WindowTooLarge {
        index: usize,
        bytes: usize,
        budget: usize,
    },
    WindowBudgetExceeded {
        bytes: usize,
        budget: usize,
    },
    StaleDocumentVersion {
        result_version: DocumentVersion,
        current_version: DocumentVersion,
    },
    ProvenanceMismatch,
    DecorationVersionMismatch {
        decoration_version: DocumentVersion,
        parse_version: DocumentVersion,
    },
    DiagnosticMetadataMismatch,
    PayloadBudgetExceeded {
        bytes: usize,
        budget: usize,
    },
    SerializationFailed,
    ResultChannelClosed,
    HandlerFailed(String),
    StaleRuntimeGeneration {
        task_generation: u64,
        active_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseHandlerMeta {
    pub package_prefix: String,
    pub mode_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsParseHandlerRegistration {
    pub(crate) package: PackageRecord,
    pub(crate) meta: ParseHandlerMeta,
    pub(crate) token: String,
    pub(crate) parse_unit: ParseUnit,
    pub(crate) timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseScheduleRequest {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub package_prefix: String,
    pub mode_id: String,
    pub viewport: ParseByteRange,
    pub invalidated_ranges: Vec<ParseByteRange>,
    pub accepted_edit: Option<ParseInputEdit>,
    pub trace_id: Option<crate::protocol::PerformanceTraceId>,
    /// Viewport-render request identity for atomic patch aggregation.
    pub request_id: Option<crate::protocol::ViewportRequestId>,
    /// Owning client for request-scoped parses (see
    /// `IncrementalParseUpdate::client_id`).
    pub client_id: Option<crate::protocol::ClientId>,
}

impl ParseScheduleRequest {
    fn into_notification(self) -> ParseEditNotification {
        let viewport = self.viewport;
        let mut invalidated_ranges = self.invalidated_ranges;
        invalidated_ranges.sort_by(|left, right| {
            let left_visible = left.intersects(viewport);
            let right_visible = right.intersects(viewport);
            right_visible
                .cmp(&left_visible)
                .then_with(|| left.start.cmp(&right.start))
                .then_with(|| left.end.cmp(&right.end))
        });
        ParseEditNotification {
            document_id: self.document_id,
            document_version: self.document_version,
            behavior_version: self.behavior_version,
            package_prefix: self.package_prefix,
            mode_id: self.mode_id,
            viewport,
            invalidated_ranges,
            accepted_edit: self.accepted_edit,
            parse_windows: Vec::new(),
            memory_budget: None,
            trace_id: self.trace_id,
            request_id: self.request_id,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParseCoordinatorStats {
    pub scheduled_tasks: usize,
    pub cancelled_superseded_tasks: usize,
    pub published_updates: usize,
    pub stale_results_rejected: usize,
    pub failed_tasks: usize,
}

#[derive(Clone)]
pub struct ParseCoordinator {
    inner: Arc<Mutex<ParseCoordinatorInner>>,
    perf: PerfRecorder,
    /// Bounded blocking executor shared by every per-document syntax session.
    syntax_executor: syntax_session::SyntaxExecutor,
    updates_tx: mpsc::Sender<IncrementalParseUpdate>,
    updates_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<IncrementalParseUpdate>>>,
    diagnostics_tx: mpsc::Sender<RuntimeDiagnostic>,
    diagnostics_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<RuntimeDiagnostic>>>,
}

struct ParseCoordinatorInner {
    handlers: HashMap<HandlerKey, RegisteredParseHandler>,
    active_tasks: HashMap<TaskKey, ActiveParseTask>,
    current_versions: HashMap<DocumentId, DocumentVersion>,
    accepted_native_edits: HashMap<
        DocumentId,
        (
            DocumentVersion,
            Instant,
            bool,
            Option<crate::protocol::PerformanceTraceId>,
        ),
    >,
    stats: ParseCoordinatorStats,
    /// Plan 060 T4 (P0-3): per-connection authorized subscriptions. Document
    /// updates route only to connections that opened the document; sanitized
    /// diagnostics broadcast to every subscribed connection.
    updates_router: crate::server::output_router::OutputRouter<IncrementalParseUpdate>,
    diagnostics_router: crate::server::output_router::OutputRouter<RuntimeDiagnostic>,
}

/// Plan 099: one persistent per-document syntax session. Unlike the old
/// per-task spawn, the entry (mailbox + worker) survives job completion until
/// the document closes, the grammar generation changes, or the package is
/// revoked; every schedule enqueues into the mailbox latest-wins.
struct ActiveParseTask {
    mailbox: Arc<syntax_session::SessionMailbox>,
    /// Latest scheduled version (the version a finishing job is compared
    /// against to detect supersession).
    document_version: DocumentVersion,
    native_edit: bool,
    /// Latest scheduled viewport-render request identity, plus its owning
    /// client, so a superseded pending job can publish an empty completion
    /// and keep the connection's pending-patch counter exact.
    request_id: Option<crate::protocol::ViewportRequestId>,
    request_client_id: Option<crate::protocol::ClientId>,
}

struct RegisteredParseHandler {
    generation_id: u64,
    handler: Arc<dyn ParseHandler>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HandlerKey {
    package_prefix: String,
    mode_id: String,
}

type TaskKey = HandlerKeyWithDocument;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HandlerKeyWithDocument {
    generation_id: u64,
    document_id: DocumentId,
    package_prefix: String,
    mode_id: String,
    window_id: Option<u64>,
}

impl ParseCoordinator {
    pub fn new() -> Self {
        // ponytail: bounded global drains retained for tests/internal tooling;
        // production delivery uses authorized per-connection subscriptions and
        // drops instead of growing when nobody drains the legacy channel.
        let (updates_tx, updates_rx) = mpsc::channel(4096);
        let (diagnostics_tx, diagnostics_rx) = mpsc::channel(4096);
        Self {
            inner: Arc::new(Mutex::new(ParseCoordinatorInner {
                handlers: HashMap::new(),
                active_tasks: HashMap::new(),
                current_versions: HashMap::new(),
                accepted_native_edits: HashMap::new(),
                stats: ParseCoordinatorStats::default(),
                updates_router: crate::server::output_router::OutputRouter::default(),
                diagnostics_router: crate::server::output_router::OutputRouter::default(),
            })),
            perf: global_recorder(),
            syntax_executor: SyntaxExecutor::new(),
            updates_tx,
            updates_rx: Arc::new(tokio::sync::Mutex::new(updates_rx)),
            diagnostics_tx,
            diagnostics_rx: Arc::new(tokio::sync::Mutex::new(diagnostics_rx)),
        }
    }

    /// Register one connection's authorized update/diagnostic channels
    /// (Plan 060 T4). Called once per accepted connection.
    pub(crate) fn subscribe_client(
        &self,
        client_id: ClientId,
    ) -> (
        mpsc::Receiver<IncrementalParseUpdate>,
        mpsc::Receiver<RuntimeDiagnostic>,
    ) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        let updates = inner.updates_router.subscribe_client(client_id);
        let diagnostics = inner.diagnostics_router.subscribe_client(client_id);
        (updates, diagnostics)
    }

    /// Authorize `client_id` to receive parse updates for `document_id`.
    pub(crate) fn subscribe_document(&self, document_id: DocumentId, client_id: ClientId) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner
            .updates_router
            .subscribe_document(document_id, client_id);
    }

    /// Withdraw `client_id`'s parse subscription for one document.
    pub(crate) fn unsubscribe_document(&self, document_id: DocumentId, client_id: ClientId) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner
            .updates_router
            .unsubscribe_document(document_id, client_id);
    }

    /// Remove every subscription held by one connection (disconnect).
    pub(crate) fn unsubscribe_client(&self, client_id: ClientId) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner.updates_router.unsubscribe_client(client_id);
        inner.diagnostics_router.unsubscribe_client(client_id);
    }

    pub fn register_handler(
        &self,
        package: &PackageRecord,
        mode_id: impl Into<String>,
        handler: impl ParseHandler,
    ) -> Result<ParseHandlerMeta, ParseCoordinatorError> {
        self.register_handler_for_generation(package, 0, mode_id, handler)
    }

    pub fn register_handler_for_generation(
        &self,
        package: &PackageRecord,
        generation_id: u64,
        mode_id: impl Into<String>,
        handler: impl ParseHandler,
    ) -> Result<ParseHandlerMeta, ParseCoordinatorError> {
        if !package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::ParseDocument)
        {
            return Err(ParseCoordinatorError::MissingPermission {
                package_prefix: package.manifest.clay.api_prefix.clone(),
            });
        }

        self.register_handler_meta_for_generation(
            generation_id,
            ParseHandlerMeta {
                package_prefix: package.manifest.clay.api_prefix.clone(),
                mode_id: mode_id.into(),
            },
            handler,
        )
    }

    pub(crate) fn register_handler_meta_for_generation(
        &self,
        generation_id: u64,
        meta: ParseHandlerMeta,
        handler: impl ParseHandler,
    ) -> Result<ParseHandlerMeta, ParseCoordinatorError> {
        self.register_handler_meta(generation_id, meta, handler, false)
    }

    pub(crate) fn replace_handler_meta_for_generation(
        &self,
        generation_id: u64,
        meta: ParseHandlerMeta,
        handler: impl ParseHandler,
    ) -> Result<ParseHandlerMeta, ParseCoordinatorError> {
        self.register_handler_meta(generation_id, meta, handler, true)
    }

    fn register_handler_meta(
        &self,
        generation_id: u64,
        meta: ParseHandlerMeta,
        handler: impl ParseHandler,
        replace_same_generation: bool,
    ) -> Result<ParseHandlerMeta, ParseCoordinatorError> {
        let key = HandlerKey {
            package_prefix: meta.package_prefix.clone(),
            mode_id: meta.mode_id.clone(),
        };
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        if inner
            .handlers
            .get(&key)
            .is_some_and(|registered| registered.generation_id == generation_id)
        {
            if replace_same_generation {
                inner.handlers.remove(&key);
            } else {
                return Err(ParseCoordinatorError::HandlerAlreadyRegistered {
                    package_prefix: key.package_prefix,
                    mode_id: key.mode_id,
                });
            }
        }
        let stale_task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|task_key| {
                task_key.package_prefix == key.package_prefix
                    && task_key.mode_id == key.mode_id
                    && task_key.generation_id != generation_id
            })
            .cloned()
            .collect();
        close_sessions(&mut inner, stale_task_keys, &self.perf, &self.updates_tx);
        inner.handlers.insert(
            key,
            RegisteredParseHandler {
                generation_id,
                handler: Arc::new(handler),
            },
        );
        Ok(meta)
    }

    /// Plan 071 task 10: look up the registered handler for a package/mode key
    /// so read-only selection queries can reuse its parsed tree. Returns the
    /// live handler regardless of generation; callers treat a miss as "no
    /// grammar".
    pub fn handler_for(
        &self,
        package_prefix: &str,
        mode_id: &str,
    ) -> Option<Arc<dyn ParseHandler>> {
        let inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner
            .handlers
            .get(&HandlerKey {
                package_prefix: package_prefix.to_string(),
                mode_id: mode_id.to_string(),
            })
            .map(|registered| registered.handler.clone())
    }

    /// Cancel handlers and active work for one exact generation.
    pub fn cancel_generation(&self, generation_id: u64) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner
            .handlers
            .retain(|_, registered| registered.generation_id != generation_id);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|task_key| task_key.generation_id == generation_id)
            .cloned()
            .collect();
        close_sessions(&mut inner, task_keys, &self.perf, &self.updates_tx);
    }

    /// After a successful runtime-generation commit, remove every handler and
    /// in-flight task older than `active_generation`, then drain any already
    /// queued parse outputs so late old-generation results cannot publish.
    pub fn cancel_older_generations(&self, active_generation: u64) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner
            .handlers
            .retain(|_, registered| registered.generation_id >= active_generation);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|task_key| task_key.generation_id < active_generation)
            .cloned()
            .collect();
        close_sessions(&mut inner, task_keys, &self.perf, &self.updates_tx);
        drop(inner);
        self.drain_pending_outputs();
    }

    /// Tear down every document-scoped registration when the final access
    /// holder closes a document: version tracking, native-edit acceptance
    /// state, and active parse work for the document (Plan 060 T6, P1-4).
    pub(crate) fn remove_document(&self, document_id: DocumentId) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner.current_versions.remove(&document_id);
        inner.accepted_native_edits.remove(&document_id);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|task_key| task_key.document_id == document_id)
            .cloned()
            .collect();
        close_sessions(&mut inner, task_keys, &self.perf, &self.updates_tx);
    }

    /// Cancel parse handlers and active work owned by one package prefix. This
    /// is the package-scoped disable/revoke hook; it reuses the same abort path
    /// as runtime generation replacement and never waits for handler completion.
    pub fn cancel_package(&self, package_prefix: &str) {
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        inner
            .handlers
            .retain(|key, _| key.package_prefix != package_prefix);
        let task_keys: Vec<_> = inner
            .active_tasks
            .keys()
            .filter(|task_key| task_key.package_prefix == package_prefix)
            .cloned()
            .collect();
        close_sessions(&mut inner, task_keys, &self.perf, &self.updates_tx);
    }

    /// Drop already-queued parse updates/diagnostics without waiting. Used by
    /// generation replacement so stale channel contents cannot reach clients.
    pub(crate) fn drain_pending_outputs(&self) {
        if let Ok(mut updates) = self.updates_rx.try_lock() {
            while updates.try_recv().is_ok() {}
        }
        if let Ok(mut diagnostics) = self.diagnostics_rx.try_lock() {
            while diagnostics.try_recv().is_ok() {}
        }
    }

    /// Snapshot of currently registered handler generations for tests/diagnostics.
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
            .expect("parse coordinator lock poisoned")
            .handlers
            .values()
            .map(|registered| registered.generation_id)
            .collect::<Vec<_>>();
        generations.sort_unstable();
        generations.dedup();
        generations
    }

    pub(crate) fn record_native_edit_accepted_with_trace(
        &self,
        document_id: DocumentId,
        document_version: DocumentVersion,
        trace_id: Option<crate::protocol::PerformanceTraceId>,
    ) {
        if !self.perf.is_enabled() {
            return;
        }
        self.inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .accepted_native_edits
            .insert(
                document_id,
                (document_version, Instant::now(), false, trace_id),
            );
        self.perf.record_with_metadata(
            SYNTAX_LOGICAL_WORK_ITEMS,
            MetricValue::Counter { amount: 1 },
            MetricMetadata::document(document_id, document_version).with_trace_id(trace_id),
        );
    }

    fn record_native_publication(&self, update: &IncrementalParseUpdate) {
        if !update
            .syntax_tree_delta
            .as_deref()
            .is_some_and(|delta| delta.starts_with("tree-sitter:"))
        {
            return;
        }
        let metadata = MetricMetadata::document(update.document_id, update.document_version)
            .with_trace_id(update.trace_id);
        if !update.decoration_updates.is_empty() {
            self.perf.record_with_metadata(
                SYNTAX_DECORATION_CHUNKS,
                MetricValue::Counter {
                    amount: update.decoration_updates.len() as u64,
                },
                metadata.clone(),
            );
        }
        let started = {
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            inner
                .accepted_native_edits
                .get_mut(&update.document_id)
                .and_then(|(version, started, published, _trace_id)| {
                    if *version == update.document_version && !*published {
                        *published = true;
                        Some(*started)
                    } else {
                        None
                    }
                })
        };
        if let Some(started) = started {
            self.perf.record_with_metadata(
                SYNTAX_EDIT_TO_PUBLISH,
                MetricValue::Duration {
                    nanos: started.elapsed().as_nanos(),
                },
                metadata,
            );
        }
    }

    /// Schedule parse work after an edit/viewport change has already been
    /// accepted. This method only records metadata, aborts superseded work, and
    /// spawns a background task; it does not wait for parse completion.
    pub fn schedule_parse(
        &self,
        request: ParseScheduleRequest,
    ) -> Result<(), ParseCoordinatorError> {
        self.schedule_parse_with_windows(request, Vec::new(), None)
    }

    /// Schedule parse work with server-prepared bounded document snapshots.
    ///
    /// The caller remains responsible for creating snapshots from already-open
    /// server-canonical document text. The coordinator validates package/mode,
    /// document/version, byte-range, and memory-budget metadata before the
    /// package handler can observe any text.
    pub fn schedule_parse_with_windows(
        &self,
        request: ParseScheduleRequest,
        parse_windows: Vec<ParseWindowSnapshot>,
        policy: Option<ParsePolicy>,
    ) -> Result<(), ParseCoordinatorError> {
        validate_request_ranges(&request)?;
        validate_parse_policy(policy)?;
        validate_window_snapshots(&request, &parse_windows, policy)?;
        let handler_key = HandlerKey {
            package_prefix: request.package_prefix.clone(),
            mode_id: request.mode_id.clone(),
        };
        // Plan 099: one session per document/grammar — the window identity
        // rides on the job, not the session, so every viewport change coalesces
        // into the same mailbox.
        let task_key = TaskKey {
            generation_id: 0,
            document_id: request.document_id,
            package_prefix: request.package_prefix.clone(),
            mode_id: request.mode_id.clone(),
            window_id: None,
        };
        let request_client_id = request.client_id;
        let mut notification = request.into_notification();
        let request_id = notification.request_id;
        let queued_at = Instant::now();
        if let Some(policy) = policy {
            notification.memory_budget =
                Some(SyntaxMemoryBudget::new(policy.memory_budget_bytes, 0));
        }
        notification.parse_windows = parse_windows;
        let notification_document_version = notification.document_version;

        let (handler, task_key, native_edit) = {
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            let registered = inner.handlers.get(&handler_key).ok_or_else(|| {
                ParseCoordinatorError::HandlerNotRegistered {
                    package_prefix: handler_key.package_prefix.clone(),
                    mode_id: handler_key.mode_id.clone(),
                }
            })?;
            let handler = registered.handler.clone();
            let task_key = TaskKey {
                generation_id: registered.generation_id,
                ..task_key
            };

            if let Some(session) = inner.active_tasks.get_mut(&task_key) {
                // Request-scoped schedules are idempotent per request id;
                // edit/viewport schedules without a request id always
                // enqueue (two same-version viewports are distinct jobs).
                if session.document_version == notification.document_version
                    && notification.request_id.is_some()
                    && session.request_id == notification.request_id
                {
                    return Ok(());
                }
                if session.document_version > notification.document_version {
                    return Err(ParseCoordinatorError::StaleDocumentVersion {
                        result_version: notification.document_version,
                        current_version: session.document_version,
                    });
                }
                // Plan 099: the session stays alive; the newer job replaces
                // the pending one latest-wins and the worker picks it up when
                // its current job finishes. A superseded pending viewport
                // request must still receive its empty completion so the
                // requesting connection's pending-patch counter reaches zero.
                let superseded =
                    session
                        .mailbox
                        .push(notification.clone(), request_client_id, Instant::now());
                if let Some(pending) = superseded {
                    publish_request_completion(&self.updates_tx, &task_key, &pending);
                }
                if session.native_edit {
                    record_superseded_cancellation(
                        &self.perf,
                        task_key.document_id,
                        session.document_version,
                    );
                }
                session.document_version = notification.document_version;
                session.request_id = notification.request_id;
                session.request_client_id = request_client_id;
                inner.stats.scheduled_tasks += 1;
                return Ok(());
            }
            let native_edit = inner
                .accepted_native_edits
                .get(&notification.document_id)
                .is_some_and(|(version, _, _, _)| *version == notification.document_version);
            inner.stats.scheduled_tasks += 1;
            (handler, task_key, native_edit)
        };

        // Plan 099: one persistent per-document session instead of a spawned
        // task per parse. The worker drains the latest-wins mailbox, runs
        // blocking handlers on the bounded executor, and publishes exactly
        // one validated update per job through `finish_task`.
        let mailbox = Arc::new(syntax_session::SessionMailbox::new());
        mailbox.push(notification.clone(), request_client_id, queued_at);
        let coordinator = self.clone();
        let executor = self.syntax_executor.clone();
        let session_task_key = task_key.clone();
        let worker_mailbox = Arc::clone(&mailbox);
        tokio::spawn(async move {
            coordinator
                .run_session(session_task_key, handler, worker_mailbox, executor)
                .await;
        });

        self.inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .active_tasks
            .insert(
                task_key,
                ActiveParseTask {
                    mailbox,
                    document_version: notification_document_version,
                    native_edit,
                    request_id,
                    request_client_id,
                },
            );
        Ok(())
    }

    /// Session worker body: drain the latest-wins mailbox job by job. Native
    /// CPU-bound handlers run on the bounded blocking executor; package
    /// JavaScript handlers await on the normal runtime. The worker exits when
    /// its mailbox is closed (document close, package revoke, generation
    /// reload) and publishes each job's terminal state through `finish_task`.
    async fn run_session(
        self,
        task_key: TaskKey,
        handler: Arc<dyn ParseHandler>,
        mailbox: Arc<syntax_session::SessionMailbox>,
        executor: syntax_session::SyntaxExecutor,
    ) {
        let mut receiver = mailbox.receiver();
        while let Some(job) = receiver.recv().await {
            let notification = job.notification;
            let job_version = notification.document_version;
            let trace_id = notification.trace_id;
            let request_id = notification.request_id;
            let client_id = job.client_id;
            let metadata = MetricMetadata::document(notification.document_id, job_version)
                .with_trace_id(trace_id);
            self.perf.record_with_metadata(
                SYNTAX_QUEUE,
                MetricValue::Duration {
                    nanos: job.queued_at.elapsed().as_nanos(),
                },
                metadata.clone(),
            );
            self.perf.record_with_metadata(
                SYNTAX_START,
                MetricValue::Counter { amount: 1 },
                metadata.clone(),
            );
            let parse_scope = self.perf.scope_with_metadata(SYNTAX_END, metadata);
            let mut result = if handler.runs_on_blocking_executor() {
                let permit = executor.acquire().await;
                let handler = Arc::clone(&handler);
                match tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    handler
                        .parse_blocking(notification)
                        .expect("blocking handlers must implement parse_blocking")
                })
                .await
                {
                    Ok(result) => result,
                    Err(join_error) => Err(ParseCoordinatorError::HandlerFailed(format!(
                        "syntax session worker join: {join_error}"
                    ))),
                }
            } else {
                handler.parse(notification).await
            };
            if let Ok(update) = &mut result {
                update.trace_id = trace_id;
                update.request_id = request_id;
                update.client_id = client_id;
                for set in &mut update.decoration_updates {
                    set.trace_id = trace_id;
                }
            }
            parse_scope.finish();
            self.finish_task(task_key.clone(), job_version, request_id, client_id, result);
        }
    }

    fn finish_task(
        &self,
        task_key: TaskKey,
        task_version: DocumentVersion,
        request_id: Option<crate::protocol::ViewportRequestId>,
        request_client_id: Option<crate::protocol::ClientId>,
        result: Result<IncrementalParseUpdate, ParseCoordinatorError>,
    ) {
        // Plan 099: the session entry persists across jobs, so a version
        // mismatch here means this job was superseded (or its session was
        // closed) — the job's output is stale and must not publish, but a
        // request-scoped job still owes the connection its empty completion.
        let superseded_or_closed = {
            let inner = self.inner.lock().expect("parse coordinator lock poisoned");
            match inner.active_tasks.get(&task_key) {
                None => true,
                Some(task) => task.document_version != task_version,
            }
        };
        if superseded_or_closed {
            self.publish_completion_if_requested(
                &task_key,
                task_version,
                request_id,
                request_client_id,
            );
            self.inner
                .lock()
                .expect("parse coordinator lock poisoned")
                .stats
                .stale_results_rejected += 1;
            return;
        }
        // Every terminal path of a request-scoped task must publish exactly
        // one update carrying the request id (empty on failure) so the
        // connection's pending-patch counter always reaches zero.
        let completion = |coordinator: &Self| {
            let Some(request_id) = request_id else { return };
            let _ = coordinator.updates_tx.try_send(IncrementalParseUpdate {
                document_id: task_key.document_id,
                document_version: task_version,
                behavior_version: 0,
                package_prefix: task_key.package_prefix.clone(),
                mode_id: task_key.mode_id.clone(),
                parse_unit: ParseUnit::Region,
                viewport: ParseByteRange::new(0, 0),
                invalidated_ranges: Vec::new(),
                syntax_tree_delta: None,
                decoration_updates: Vec::new(),
                diagnostic_update: None,
                folding_update: None,
                trace_id: None,
                request_id: Some(request_id),
                client_id: request_client_id,
            });
        };
        let Ok(update) = result else {
            let error = result.expect_err("parse result error present");
            let diagnostic = parse_failure_diagnostic(&task_key, &error);
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            inner.stats.failed_tasks += 1;
            inner.diagnostics_router.broadcast(&diagnostic);
            drop(inner);
            let _ = self.diagnostics_tx.try_send(diagnostic);
            completion(self);
            return;
        };

        if self.validate_task_generation(&task_key).is_err() {
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            inner.stats.stale_results_rejected += 1;
            drop(inner);
            completion(self);
            return;
        }

        match self.validate_update(&update) {
            Ok(()) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.stats.published_updates += 1;
                inner
                    .updates_router
                    .route_document(update.document_id, &update);
                drop(inner);
                self.record_native_publication(&update);
                let _ = self.updates_tx.try_send(update);
            }
            Err(ParseCoordinatorError::StaleDocumentVersion { .. }) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.stats.stale_results_rejected += 1;
                drop(inner);
                completion(self);
            }
            Err(error) => {
                let diagnostic = parse_failure_diagnostic(&task_key, &error);
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.stats.failed_tasks += 1;
                inner.diagnostics_router.broadcast(&diagnostic);
                drop(inner);
                let _ = self.diagnostics_tx.try_send(diagnostic);
                completion(self);
            }
        }
    }

    /// Publish the empty completion update a request-scoped job owes the
    /// connection whenever its output cannot publish (superseded, session
    /// closed, failed, stale) so the pending-patch counter reaches zero.
    fn publish_completion_if_requested(
        &self,
        task_key: &TaskKey,
        task_version: DocumentVersion,
        request_id: Option<crate::protocol::ViewportRequestId>,
        request_client_id: Option<crate::protocol::ClientId>,
    ) {
        let Some(request_id) = request_id else {
            return;
        };
        let _ = self.updates_tx.try_send(empty_request_completion(
            task_key,
            task_version,
            request_id,
            request_client_id,
        ));
    }

    fn validate_task_generation(&self, task_key: &TaskKey) -> Result<(), ParseCoordinatorError> {
        let handler_key = HandlerKey {
            package_prefix: task_key.package_prefix.clone(),
            mode_id: task_key.mode_id.clone(),
        };
        let active_generation = self
            .inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .handlers
            .get(&handler_key)
            .map(|registered| registered.generation_id);
        if active_generation == Some(task_key.generation_id) {
            Ok(())
        } else {
            Err(ParseCoordinatorError::StaleRuntimeGeneration {
                task_generation: task_key.generation_id,
                active_generation: active_generation.unwrap_or_default(),
            })
        }
    }

    pub fn validate_update(
        &self,
        update: &IncrementalParseUpdate,
    ) -> Result<(), ParseCoordinatorError> {
        if !update.viewport.is_valid() {
            return Err(ParseCoordinatorError::InvalidViewportRange);
        }
        for (index, range) in update.invalidated_ranges.iter().enumerate() {
            if !range.is_valid() {
                return Err(ParseCoordinatorError::InvalidatedRangeInvalid { index });
            }
        }
        let current_version = self
            .inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .current_versions
            .get(&update.document_id)
            .copied()
            .unwrap_or(update.document_version);
        if update.document_version != current_version {
            return Err(ParseCoordinatorError::StaleDocumentVersion {
                result_version: update.document_version,
                current_version,
            });
        }
        for decorations in &update.decoration_updates {
            if decorations.document_id != update.document_id
                || decorations.document_version != update.document_version
            {
                return Err(ParseCoordinatorError::DecorationVersionMismatch {
                    decoration_version: decorations.document_version,
                    parse_version: update.document_version,
                });
            }
            if decorations.viewport_byte_start < update.viewport.start
                || decorations.viewport_byte_end > update.viewport.end
                || decorations
                    .spans
                    .iter()
                    .any(|span| span.provenance.package_prefix != update.package_prefix)
            {
                return Err(ParseCoordinatorError::ProvenanceMismatch);
            }
            validate_decoration_set(current_version, decorations.clone(), None).map_err(
                |error| {
                    ParseCoordinatorError::HandlerFailed(format!(
                        "decoration validation failed: {error:?}"
                    ))
                },
            )?;
        }
        if let Some(diagnostics) = &update.diagnostic_update {
            if diagnostics.document_id != update.document_id
                || diagnostics.document_version != update.document_version
                || diagnostics.viewport_byte_start != update.viewport.start
                || diagnostics.viewport_byte_end != update.viewport.end
                || diagnostics.provenance.package_prefix != update.package_prefix
            {
                return Err(ParseCoordinatorError::DiagnosticMetadataMismatch);
            }
            validate_diagnostic_set(current_version, diagnostics.clone(), None).map_err(
                |error| {
                    ParseCoordinatorError::HandlerFailed(format!(
                        "diagnostic validation failed: {error:?}"
                    ))
                },
            )?;
        }
        let mut bounded_update = update.clone();
        let decoration_updates = std::mem::take(&mut bounded_update.decoration_updates);
        validate_update_payload(&bounded_update)?;
        for decoration in decoration_updates {
            bounded_update.decoration_updates.push(decoration);
            validate_update_payload(&bounded_update)?;
            bounded_update.decoration_updates.clear();
        }
        Ok(())
    }

    pub async fn next_update(&self) -> Option<IncrementalParseUpdate> {
        self.updates_rx.lock().await.recv().await
    }

    pub async fn next_diagnostic(&self) -> Option<RuntimeDiagnostic> {
        self.diagnostics_rx.lock().await.recv().await
    }

    pub fn stats(&self) -> ParseCoordinatorStats {
        self.inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .stats
            .clone()
    }
}

fn validate_update_payload(update: &IncrementalParseUpdate) -> Result<(), ParseCoordinatorError> {
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(update)
        .map_err(|_| ParseCoordinatorError::SerializationFailed)?
        .len();
    let budget = if update.folding_update.is_some() {
        INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES
    } else {
        INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES
    };
    if bytes > budget {
        return Err(ParseCoordinatorError::PayloadBudgetExceeded { bytes, budget });
    }
    Ok(())
}

fn parse_failure_diagnostic(
    task_key: &TaskKey,
    error: &ParseCoordinatorError,
) -> RuntimeDiagnostic {
    let reason = match error {
        ParseCoordinatorError::HandlerFailed(_) => "handler failed",
        ParseCoordinatorError::PayloadBudgetExceeded { .. } => "payload budget exceeded",
        ParseCoordinatorError::WindowTooLarge { .. }
        | ParseCoordinatorError::WindowBudgetExceeded { .. } => "parse window budget exceeded",
        ParseCoordinatorError::StaleDocumentVersion { .. }
        | ParseCoordinatorError::StaleRuntimeGeneration { .. } => "stale parse result rejected",
        _ => "parse result rejected",
    };
    RuntimeDiagnostic::error(
        "parse.open_failed",
        format!(
            "Background parse for package '{}' mode '{}' on document {} failed: {}.",
            task_key.package_prefix, task_key.mode_id, task_key.document_id, reason
        ),
    )
}

/// Plan 099: close sessions instead of aborting parse work. The mailbox is
/// closed (pending job dropped with its request completion) and the worker
/// exits gracefully after its current job; that job's output is discarded by
/// `finish_task` because the session entry is gone, and its request-scoped
/// completion still publishes. Stale output can never publish after close.
fn close_sessions(
    inner: &mut ParseCoordinatorInner,
    task_keys: Vec<TaskKey>,
    perf: &PerfRecorder,
    updates_tx: &mpsc::Sender<IncrementalParseUpdate>,
) {
    for task_key in task_keys {
        if let Some(task) = inner.active_tasks.remove(&task_key) {
            if let Some(pending) = task.mailbox.close() {
                publish_request_completion(updates_tx, &task_key, &pending);
            }
            if task.native_edit {
                record_superseded_cancellation(perf, task_key.document_id, task.document_version);
            }
            inner.stats.cancelled_superseded_tasks += 1;
        }
    }
}

/// The one empty terminal update a request-scoped job publishes when it can
/// never deliver real members.
#[allow(clippy::too_many_arguments)]
fn empty_request_completion(
    task_key: &TaskKey,
    task_version: DocumentVersion,
    request_id: crate::protocol::ViewportRequestId,
    client_id: Option<crate::protocol::ClientId>,
) -> IncrementalParseUpdate {
    IncrementalParseUpdate {
        document_id: task_key.document_id,
        document_version: task_version,
        behavior_version: 0,
        package_prefix: task_key.package_prefix.clone(),
        mode_id: task_key.mode_id.clone(),
        parse_unit: ParseUnit::Region,
        viewport: ParseByteRange::new(0, 0),
        invalidated_ranges: Vec::new(),
        syntax_tree_delta: None,
        decoration_updates: Vec::new(),
        diagnostic_update: None,
        folding_update: None,
        trace_id: None,
        request_id: Some(request_id),
        client_id,
    }
}

/// Complete a superseded pending request-scoped job at schedule time.
fn publish_request_completion(
    updates_tx: &mpsc::Sender<IncrementalParseUpdate>,
    task_key: &TaskKey,
    pending: &syntax_session::SessionJob,
) {
    let Some(request_id) = pending.notification.request_id else {
        return;
    };
    let _ = updates_tx.try_send(empty_request_completion(
        task_key,
        pending.notification.document_version,
        request_id,
        pending.client_id,
    ));
}

fn record_superseded_cancellation(
    perf: &PerfRecorder,
    document_id: DocumentId,
    document_version: DocumentVersion,
) {
    perf.record_with_metadata(
        SYNTAX_CANCELLED_SUPERSEDED,
        MetricValue::Counter { amount: 1 },
        MetricMetadata::document(document_id, document_version),
    );
}

impl Default for ParseCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ParseCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParseCoordinator")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

fn validate_request_ranges(request: &ParseScheduleRequest) -> Result<(), ParseCoordinatorError> {
    if !request.viewport.is_valid() {
        return Err(ParseCoordinatorError::InvalidViewportRange);
    }
    for (index, range) in request.invalidated_ranges.iter().enumerate() {
        if !range.is_valid() {
            return Err(ParseCoordinatorError::InvalidatedRangeInvalid { index });
        }
    }
    if request
        .accepted_edit
        .is_some_and(|edit| !edit.is_valid() || edit.document_version != request.document_version)
    {
        return Err(ParseCoordinatorError::InvalidAcceptedEdit);
    }
    Ok(())
}

fn validate_parse_policy(policy: Option<ParsePolicy>) -> Result<(), ParseCoordinatorError> {
    let Some(policy) = policy else {
        return Ok(());
    };
    if policy.max_window_bytes == 0
        || policy.memory_budget_bytes == 0
        || policy.memory_budget_bytes > SYNTAX_CACHE_BUDGET_BYTES as u64
        || policy.max_window_bytes > policy.memory_budget_bytes
        || policy.timeout_ms == 0
        || policy.timeout_ms > 5_000
    {
        return Err(ParseCoordinatorError::InvalidParsePolicy);
    }
    Ok(())
}

fn validate_window_snapshots(
    request: &ParseScheduleRequest,
    parse_windows: &[ParseWindowSnapshot],
    policy: Option<ParsePolicy>,
) -> Result<(), ParseCoordinatorError> {
    let max_window_bytes = policy
        .map(|policy| policy.max_window_bytes as usize)
        .unwrap_or(SYNTAX_CACHE_BUDGET_BYTES);
    let memory_budget_bytes = policy
        .map(|policy| policy.memory_budget_bytes as usize)
        .unwrap_or(SYNTAX_CACHE_BUDGET_BYTES);
    let mut total_bytes = 0usize;

    for (index, snapshot) in parse_windows.iter().enumerate() {
        if snapshot.document_id != request.document_id
            || snapshot.document_version != request.document_version
            || snapshot.package_prefix != request.package_prefix
            || snapshot.mode_id != request.mode_id
            || snapshot.window_id != snapshot.byte_start
        {
            return Err(ParseCoordinatorError::WindowMetadataMismatch { index });
        }
        let range = snapshot.byte_range();
        if !range.is_valid() {
            return Err(ParseCoordinatorError::InvalidWindowRange { index });
        }
        let expected = usize::try_from(range.len()).map_err(|_| {
            ParseCoordinatorError::WindowTextLengthMismatch {
                index,
                expected: usize::MAX,
                actual: snapshot.text_len_bytes(),
            }
        })?;
        let actual = snapshot.text_len_bytes();
        if actual != expected {
            return Err(ParseCoordinatorError::WindowTextLengthMismatch {
                index,
                expected,
                actual,
            });
        }
        if actual > max_window_bytes {
            return Err(ParseCoordinatorError::WindowTooLarge {
                index,
                bytes: actual,
                budget: max_window_bytes,
            });
        }
        if snapshot.incremental_edit
            && !request
                .accepted_edit
                .is_some_and(|edit| edit_fits_window(edit, snapshot, actual, max_window_bytes))
        {
            return Err(ParseCoordinatorError::InvalidAcceptedEdit);
        }
        total_bytes = total_bytes.saturating_add(actual);
    }

    if total_bytes > memory_budget_bytes {
        return Err(ParseCoordinatorError::WindowBudgetExceeded {
            bytes: total_bytes,
            budget: memory_budget_bytes,
        });
    }

    Ok(())
}

fn edit_fits_window(
    edit: ParseInputEdit,
    window: &ParseWindowSnapshot,
    current_window_bytes: usize,
    max_window_bytes: usize,
) -> bool {
    let Some(relative) = edit.relative_to_window(window) else {
        return false;
    };
    let delta = relative.new_end_byte as i128 - relative.old_end_byte as i128;
    let old_window_bytes = current_window_bytes as i128 - delta;
    if !(0..=max_window_bytes as i128).contains(&old_window_bytes)
        || relative.old_end_byte > old_window_bytes as u64
        || relative.new_end_byte > current_window_bytes as u64
    {
        return false;
    }
    point_at_window_offset(window, relative.start_byte) == Some(edit.start_position)
        && point_at_window_offset(window, relative.new_end_byte) == Some(edit.new_end_position)
}

fn point_at_window_offset(window: &ParseWindowSnapshot, offset: u64) -> Option<ParsePoint> {
    let offset = usize::try_from(offset).ok()?;
    let prefix = window.text.get(..offset)?;
    let newline_count = prefix.bytes().filter(|byte| *byte == b'\n').count() as u64;
    if newline_count == 0 {
        Some(ParsePoint::new(
            window.base_line,
            window.base_column + offset as u64,
        ))
    } else {
        Some(ParsePoint::new(
            window.base_line + newline_count,
            prefix.rsplit_once('\n')?.1.len() as u64,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        perf::metrics::PerfRecorder,
        protocol::{DecorationSet, FoldingRangeSet, ParseUnit},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn accepted_native_edit_records_one_logical_item_and_one_latency_sample() {
        let perf = PerfRecorder::for_test(true);
        let mut coordinator = ParseCoordinator::new();
        coordinator.perf = perf.clone();
        coordinator.record_native_edit_accepted_with_trace(7, 2, Some(41));
        let update = IncrementalParseUpdate {
            document_id: 7,
            document_version: 2,
            behavior_version: 1,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            parse_unit: ParseUnit::Region,
            viewport: ParseByteRange::new(0, 1),
            invalidated_ranges: vec![ParseByteRange::new(0, 1)],
            syntax_tree_delta: Some("tree-sitter:rust:incremental".to_string()),
            decoration_updates: vec![DecorationSet {
                document_id: 7,
                document_version: 2,
                package_prefix: "rust".to_string(),
                kind: crate::protocol::DecorationKind::Syntax,
                viewport_byte_start: 0,
                viewport_byte_end: 1,
                spans: Vec::new(),
                trace_id: Some(41),
            }],
            diagnostic_update: None,
            folding_update: None,
            trace_id: Some(41),
            request_id: None,
            client_id: None,
        };

        coordinator.record_native_publication(&update);
        coordinator.record_native_publication(&update);

        let snapshots = perf.snapshots();
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_LOGICAL_WORK_ITEMS)
                .count(),
            1
        );
        assert_eq!(
            snapshots
                .iter()
                .find(|snapshot| snapshot.name == SYNTAX_LOGICAL_WORK_ITEMS)
                .and_then(|snapshot| snapshot.metadata.trace_id),
            Some(41)
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_DECORATION_CHUNKS)
                .count(),
            2
        );
        let edit_to_publish = snapshots
            .iter()
            .filter(|snapshot| snapshot.name == SYNTAX_EDIT_TO_PUBLISH)
            .collect::<Vec<_>>();
        assert_eq!(edit_to_publish.len(), 1);
        eprintln!("edit_to_publish={:?}", edit_to_publish[0].value);
    }

    // ── Plan 099 bounded per-document syntax session tests ──────────────────

    /// Test handler whose "parse" blocks on a std channel until released, so
    /// tests can hold the session worker mid-job deterministically.
    #[derive(Clone)]
    struct GateHandler {
        release: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<()>>>,
        started: Arc<AtomicUsize>,
        blocking: bool,
    }

    struct GateControl {
        release: std::sync::mpsc::Sender<()>,
        started: Arc<AtomicUsize>,
    }

    impl GateControl {
        /// Async wait so the current-thread test runtime keeps polling the
        /// session worker while the job has not started yet.
        async fn wait_until_started(&self) {
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            while self.started.load(Ordering::SeqCst) == 0 {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "syntax session never started the gated job"
                );
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
        }
    }

    impl GateHandler {
        /// (handler, control). Release sends one token; every `started` count
        /// means one job parked mid-parse.
        fn gated(blocking: bool) -> (Self, GateControl) {
            let (tx, rx) = std::sync::mpsc::channel::<()>();
            let started = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    release: Arc::new(std::sync::Mutex::new(rx)),
                    started: Arc::clone(&started),
                    blocking,
                },
                GateControl {
                    release: tx,
                    started,
                },
            )
        }
    }

    impl ParseHandler for GateHandler {
        fn parse(&self, notification: ParseEditNotification) -> ParseHandlerFuture {
            let result = self
                .parse_blocking(notification)
                .expect("gate handler implements parse_blocking");
            Box::pin(async move { result })
        }

        fn parse_blocking(
            &self,
            notification: ParseEditNotification,
        ) -> Option<Result<IncrementalParseUpdate, ParseCoordinatorError>> {
            self.started.fetch_add(1, Ordering::SeqCst);
            // Deterministic park: a std channel recv blocks the worker (or
            // the blocking thread) until the test releases the job.
            let _ = self
                .release
                .lock()
                .expect("gate release mutex poisoned")
                .recv();
            Some(Ok(IncrementalParseUpdate {
                document_id: notification.document_id,
                document_version: notification.document_version,
                behavior_version: notification.behavior_version,
                package_prefix: notification.package_prefix,
                mode_id: notification.mode_id,
                parse_unit: ParseUnit::Region,
                viewport: notification.viewport,
                invalidated_ranges: notification.invalidated_ranges,
                syntax_tree_delta: Some("gate".to_string()),
                decoration_updates: Vec::new(),
                diagnostic_update: None,
                folding_update: None,
                trace_id: notification.trace_id,
                request_id: notification.request_id,
                client_id: None,
            }))
        }

        fn runs_on_blocking_executor(&self) -> bool {
            self.blocking
        }
    }

    fn gate_request(
        document_id: u64,
        version: u64,
        request_id: Option<u64>,
    ) -> ParseScheduleRequest {
        ParseScheduleRequest {
            document_id,
            document_version: version,
            behavior_version: 1,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            viewport: ParseByteRange::new(0, 64),
            invalidated_ranges: vec![ParseByteRange::new(0, 64)],
            accepted_edit: None,
            trace_id: None,
            request_id,
            client_id: request_id.map(|_| 9),
        }
    }

    /// Plan 099: one CPU-bound native job parked on the blocking executor
    /// must not delay an unrelated timer task on the async runtime.
    #[tokio::test]
    async fn blocking_syntax_job_does_not_starve_tokio_timer() {
        let coordinator = ParseCoordinator::new();
        let (handler, gate) = GateHandler::gated(true);
        coordinator
            .register_handler_meta_for_generation(
                1,
                ParseHandlerMeta {
                    package_prefix: "rust".to_string(),
                    mode_id: "rust.rust".to_string(),
                },
                handler,
            )
            .unwrap();
        coordinator
            .schedule_parse(gate_request(7, 1, None))
            .unwrap();
        gate.wait_until_started().await;

        // The runtime stays responsive while the parse thread is parked.
        tokio::time::timeout(std::time::Duration::from_millis(200), async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        })
        .await
        .expect("tokio timer must fire while syntax job blocks");

        gate.release.send(()).unwrap();
        let update = coordinator.next_update().await.unwrap();
        assert_eq!(update.document_version, 1);
    }

    /// Plan 099: 100 same-document updates coalesce latest-wins into one
    /// published patch (the running job's stale output is dropped).
    #[tokio::test]
    async fn session_mailbox_coalesces_hundred_updates_to_latest() {
        let coordinator = ParseCoordinator::new();
        let (handler, gate) = GateHandler::gated(true);
        coordinator
            .register_handler_meta_for_generation(
                1,
                ParseHandlerMeta {
                    package_prefix: "rust".to_string(),
                    mode_id: "rust.rust".to_string(),
                },
                handler,
            )
            .unwrap();
        coordinator
            .schedule_parse(gate_request(7, 1, None))
            .unwrap();
        gate.wait_until_started().await;
        for version in 2..=100u64 {
            coordinator
                .schedule_parse(gate_request(7, version, None))
                .unwrap();
        }
        // One token releases the parked first job; the second token releases
        // the coalesced latest job (v100).
        gate.release.send(()).unwrap();
        gate.release.send(()).unwrap();
        let update =
            tokio::time::timeout(std::time::Duration::from_secs(2), coordinator.next_update())
                .await
                .unwrap()
                .unwrap();
        assert_eq!(
            update.document_version, 100,
            "only the latest coalesced job publishes"
        );
        assert_eq!(coordinator.stats().published_updates, 1);
        assert_eq!(coordinator.stats().stale_results_rejected, 1);
    }

    /// Plan 099: same-language documents progress independently — each
    /// document owns a session, and both jobs publish within the executor
    /// bound.
    #[tokio::test]
    async fn same_language_two_documents_progress_independently() {
        let coordinator = ParseCoordinator::new();
        let (handler, gate) = GateHandler::gated(false);
        coordinator
            .register_handler_meta_for_generation(
                1,
                ParseHandlerMeta {
                    package_prefix: "rust".to_string(),
                    mode_id: "rust.rust".to_string(),
                },
                handler,
            )
            .unwrap();
        coordinator
            .schedule_parse(gate_request(7, 1, None))
            .unwrap();
        coordinator
            .schedule_parse(gate_request(8, 1, None))
            .unwrap();
        // One release token per running job; both sessions run to completion.
        gate.release.send(()).unwrap();
        gate.release.send(()).unwrap();
        let mut documents = Vec::new();
        for _ in 0..2 {
            let update =
                tokio::time::timeout(std::time::Duration::from_secs(2), coordinator.next_update())
                    .await
                    .unwrap()
                    .unwrap();
            documents.push(update.document_id);
        }
        documents.sort();
        assert_eq!(documents, vec![7, 8]);
    }

    /// Plan 099: a superseded running request-scoped job publishes its empty
    /// completion (the pending-patch counter must reach zero), and a closed
    /// document's pending request-scoped job completes at close time.
    #[tokio::test]
    async fn superseded_and_closed_request_jobs_publish_completions() {
        let coordinator = ParseCoordinator::new();
        let (handler, gate) = GateHandler::gated(true);
        coordinator
            .register_handler_meta_for_generation(
                1,
                ParseHandlerMeta {
                    package_prefix: "rust".to_string(),
                    mode_id: "rust.rust".to_string(),
                },
                handler,
            )
            .unwrap();
        // Request 11 (v1) runs and gets parked; a newer schedule supersedes
        // the session.
        coordinator
            .schedule_parse(gate_request(7, 1, Some(11)))
            .unwrap();
        gate.wait_until_started().await;
        coordinator
            .schedule_parse(gate_request(7, 2, None))
            .unwrap();
        gate.release.send(()).unwrap();

        let completion = coordinator.next_update().await.unwrap();
        assert_eq!(completion.request_id, Some(11));
        assert!(completion.decoration_updates.is_empty());
        assert_eq!(
            coordinator.stats().stale_results_rejected,
            1,
            "superseded running job output is discarded"
        );

        // v2 runs unblocked; release and let it publish.
        gate.release.send(()).unwrap();
        let update = coordinator.next_update().await.unwrap();
        assert_eq!(update.document_version, 2);

        // Close the document with a pending request-scoped job: the close
        // path owes that request its empty completion.
        coordinator
            .schedule_parse(gate_request(7, 3, Some(12)))
            .unwrap();
        gate.release.send(()).unwrap();
        let running = coordinator.next_update().await.unwrap();
        assert_eq!(running.document_version, 3);
        coordinator
            .schedule_parse(gate_request(7, 4, Some(13)))
            .unwrap();
        coordinator.remove_document(7);
        let closed = coordinator.next_update().await.unwrap();
        assert_eq!(closed.request_id, Some(13));
        assert!(closed.decoration_updates.is_empty());
        assert_eq!(coordinator.stats().published_updates, 2);
    }

    #[tokio::test]
    async fn superseded_native_task_records_only_document_version_metadata() {
        let perf = PerfRecorder::for_test(true);
        let coordinator = ParseCoordinator::new();
        let task_key = TaskKey {
            generation_id: 1,
            document_id: 7,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            window_id: Some(0),
        };
        let mut inner = coordinator
            .inner
            .lock()
            .expect("parse coordinator lock poisoned");
        inner.active_tasks.insert(
            task_key.clone(),
            ActiveParseTask {
                mailbox: Arc::new(syntax_session::SessionMailbox::new()),
                document_version: 3,
                native_edit: true,
                request_id: None,
                request_client_id: None,
            },
        );
        close_sessions(&mut inner, vec![task_key], &perf, &coordinator.updates_tx);
        drop(inner);

        let snapshot = perf.snapshots().pop().expect("cancellation metric");
        assert_eq!(snapshot.name, SYNTAX_CANCELLED_SUPERSEDED);
        assert_eq!(snapshot.metadata, MetricMetadata::document(7, 3));
    }

    #[test]
    fn folded_parse_update_uses_additive_budget_without_changing_ordinary_cap() {
        let ordinary = IncrementalParseUpdate {
            document_id: 7,
            document_version: 2,
            behavior_version: 1,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            parse_unit: ParseUnit::Region,
            viewport: ParseByteRange::new(0, 1),
            invalidated_ranges: vec![ParseByteRange::new(0, 1)],
            syntax_tree_delta: Some("x".repeat(4000)),
            decoration_updates: Vec::new(),
            diagnostic_update: None,
            folding_update: None,
            trace_id: None,
            request_id: None,
            client_id: None,
        };
        assert!(matches!(
            validate_update_payload(&ordinary),
            Err(ParseCoordinatorError::PayloadBudgetExceeded {
                budget: INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
                ..
            })
        ));

        let mut folded = ordinary;
        folded.folding_update = Some(FoldingRangeSet {
            document_id: 7,
            document_version: 2,
            package_prefix: "core".to_string(),
            ranges: Vec::new(),
        });
        assert!(validate_update_payload(&folded).is_ok());
    }
}
