use std::{
    collections::HashMap,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Instant,
};

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    packages::{permissions::PackagePermission, record::PackageRecord},
    perf::{
        budgets::{INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, SYNTAX_CACHE_BUDGET_BYTES},
        metrics::{
            MetricMetadata, MetricValue, PerfRecorder, SYNTAX_CANCELLED_SUPERSEDED,
            SYNTAX_DECORATION_CHUNKS, SYNTAX_EDIT_TO_PUBLISH, SYNTAX_LOGICAL_WORK_ITEMS,
            global_recorder,
        },
    },
    protocol::{
        BehaviorVersion, DocumentId, DocumentVersion, IncrementalParseUpdate, ParseByteRange,
        ParseEditNotification, ParseInputEdit, ParsePoint, ParsePolicy, ParseUnit,
        ParseWindowSnapshot, RuntimeDiagnostic, SyntaxMemoryBudget,
    },
    server::{decorations::validate_decoration_set, diagnostics::validate_diagnostic_set},
};

pub type ParseHandlerFuture =
    Pin<Box<dyn Future<Output = Result<IncrementalParseUpdate, ParseCoordinatorError>> + Send>>;

/// Typed server-side boundary for package parse handlers.
///
/// Implementations may call the constrained JavaScript runtime, but the Rust
/// client never receives or executes parser code.
pub trait ParseHandler: Send + Sync + 'static {
    fn parse(&self, notification: ParseEditNotification) -> ParseHandlerFuture;
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
    updates_tx: mpsc::UnboundedSender<IncrementalParseUpdate>,
    updates_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<IncrementalParseUpdate>>>,
    diagnostics_tx: mpsc::UnboundedSender<RuntimeDiagnostic>,
    diagnostics_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<RuntimeDiagnostic>>>,
}

struct ParseCoordinatorInner {
    handlers: HashMap<HandlerKey, RegisteredParseHandler>,
    active_tasks: HashMap<TaskKey, ActiveParseTask>,
    current_versions: HashMap<DocumentId, DocumentVersion>,
    accepted_native_edits: HashMap<DocumentId, (DocumentVersion, Instant, bool)>,
    stats: ParseCoordinatorStats,
}

struct ActiveParseTask {
    handle: JoinHandle<()>,
    document_version: DocumentVersion,
    native_edit: bool,
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
        let (updates_tx, updates_rx) = mpsc::unbounded_channel();
        let (diagnostics_tx, diagnostics_rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(Mutex::new(ParseCoordinatorInner {
                handlers: HashMap::new(),
                active_tasks: HashMap::new(),
                current_versions: HashMap::new(),
                accepted_native_edits: HashMap::new(),
                stats: ParseCoordinatorStats::default(),
            })),
            perf: global_recorder(),
            updates_tx,
            updates_rx: Arc::new(tokio::sync::Mutex::new(updates_rx)),
            diagnostics_tx,
            diagnostics_rx: Arc::new(tokio::sync::Mutex::new(diagnostics_rx)),
        }
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
        abort_tasks(&mut inner, stale_task_keys, &self.perf);
        inner.handlers.insert(
            key,
            RegisteredParseHandler {
                generation_id,
                handler: Arc::new(handler),
            },
        );
        Ok(meta)
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
        abort_tasks(&mut inner, task_keys, &self.perf);
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
        abort_tasks(&mut inner, task_keys, &self.perf);
        drop(inner);
        self.drain_pending_outputs();
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
        abort_tasks(&mut inner, task_keys, &self.perf);
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

    pub(crate) fn record_native_edit_accepted(
        &self,
        document_id: DocumentId,
        document_version: DocumentVersion,
    ) {
        if !self.perf.is_enabled() {
            return;
        }
        self.inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .accepted_native_edits
            .insert(document_id, (document_version, Instant::now(), false));
        self.perf.record_with_metadata(
            SYNTAX_LOGICAL_WORK_ITEMS,
            MetricValue::Counter { amount: 1 },
            MetricMetadata::document(document_id, document_version),
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
        let metadata = MetricMetadata::document(update.document_id, update.document_version);
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
                .and_then(|(version, started, published)| {
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
        let window_id = parse_windows.first().map(|window| window.window_id);
        let task_key = TaskKey {
            generation_id: 0,
            document_id: request.document_id,
            package_prefix: request.package_prefix.clone(),
            mode_id: request.mode_id.clone(),
            window_id: window_id.or(Some(request.viewport.start)),
        };
        let mut notification = request.into_notification();
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

            if inner
                .active_tasks
                .get(&task_key)
                .is_some_and(|task| task.document_version == notification.document_version)
            {
                return Ok(());
            }
            if let Some(current_version) = inner.current_versions.get(&notification.document_id)
                && *current_version > notification.document_version
            {
                return Err(ParseCoordinatorError::StaleDocumentVersion {
                    result_version: notification.document_version,
                    current_version: *current_version,
                });
            }
            inner
                .current_versions
                .insert(notification.document_id, notification.document_version);
            let superseded = inner
                .active_tasks
                .iter()
                .filter(|(key, task)| {
                    key.generation_id == task_key.generation_id
                        && key.document_id == task_key.document_id
                        && key.package_prefix == task_key.package_prefix
                        && key.mode_id == task_key.mode_id
                        && (*key == &task_key
                            || task.document_version < notification.document_version)
                })
                .map(|(key, _)| key.clone())
                .collect();
            abort_tasks(&mut inner, superseded, &self.perf);
            let native_edit = inner
                .accepted_native_edits
                .get(&notification.document_id)
                .is_some_and(|(version, _, _)| *version == notification.document_version);
            inner.stats.scheduled_tasks += 1;
            (handler, task_key, native_edit)
        };

        let coordinator = self.clone();
        let spawned_task_key = task_key.clone();
        let (start_tx, start_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            if start_rx.await.is_err() {
                return;
            }
            let result = handler.parse(notification).await;
            coordinator.finish_task(spawned_task_key, notification_document_version, result);
        });

        self.inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .active_tasks
            .insert(
                task_key,
                ActiveParseTask {
                    handle: task,
                    document_version: notification_document_version,
                    native_edit,
                },
            );
        let _ = start_tx.send(());
        Ok(())
    }

    fn finish_task(
        &self,
        task_key: TaskKey,
        task_version: DocumentVersion,
        result: Result<IncrementalParseUpdate, ParseCoordinatorError>,
    ) {
        if self
            .inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .active_tasks
            .get(&task_key)
            .is_none_or(|task| task.document_version != task_version)
        {
            return;
        }
        let Ok(update) = result else {
            let error = result.expect_err("parse result error present");
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            inner.active_tasks.remove(&task_key);
            inner.stats.failed_tasks += 1;
            drop(inner);
            let _ = self
                .diagnostics_tx
                .send(parse_failure_diagnostic(&task_key, &error));
            return;
        };

        if self.validate_task_generation(&task_key).is_err() {
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            inner.active_tasks.remove(&task_key);
            inner.stats.stale_results_rejected += 1;
            return;
        }

        match self.validate_update(&update) {
            Ok(()) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.published_updates += 1;
                drop(inner);
                self.record_native_publication(&update);
                let _ = self.updates_tx.send(update);
            }
            Err(ParseCoordinatorError::StaleDocumentVersion { .. }) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.stale_results_rejected += 1;
            }
            Err(error) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.failed_tasks += 1;
                drop(inner);
                let _ = self
                    .diagnostics_tx
                    .send(parse_failure_diagnostic(&task_key, &error));
            }
        }
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
    if bytes > INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES {
        return Err(ParseCoordinatorError::PayloadBudgetExceeded {
            bytes,
            budget: INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
        });
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
        "clay.parse.open_failed",
        format!(
            "Background parse for package '{}' mode '{}' on document {} failed: {}.",
            task_key.package_prefix, task_key.mode_id, task_key.document_id, reason
        ),
    )
}

fn abort_tasks(inner: &mut ParseCoordinatorInner, task_keys: Vec<TaskKey>, perf: &PerfRecorder) {
    for task_key in task_keys {
        if let Some(task) = inner.active_tasks.remove(&task_key) {
            task.handle.abort();
            if task.native_edit {
                record_superseded_cancellation(perf, task_key.document_id, task.document_version);
            }
            inner.stats.cancelled_superseded_tasks += 1;
        }
    }
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
        protocol::{DecorationSet, ParseUnit},
    };

    #[test]
    fn accepted_native_edit_records_one_logical_item_and_one_latency_sample() {
        let perf = PerfRecorder::for_test(true);
        let mut coordinator = ParseCoordinator::new();
        coordinator.perf = perf.clone();
        coordinator.record_native_edit_accepted(7, 2);
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
            }],
            diagnostic_update: None,
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
                .filter(|snapshot| snapshot.name == SYNTAX_DECORATION_CHUNKS)
                .count(),
            2
        );
        assert_eq!(
            snapshots
                .iter()
                .filter(|snapshot| snapshot.name == SYNTAX_EDIT_TO_PUBLISH)
                .count(),
            1
        );
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
                handle: tokio::spawn(std::future::pending()),
                document_version: 3,
                native_edit: true,
            },
        );
        abort_tasks(&mut inner, vec![task_key], &perf);
        drop(inner);

        let snapshot = perf.snapshots().pop().expect("cancellation metric");
        assert_eq!(snapshot.name, SYNTAX_CANCELLED_SUPERSEDED);
        assert_eq!(snapshot.metadata, MetricMetadata::document(7, 3));
    }
}
