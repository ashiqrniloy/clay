use std::{
    collections::HashMap,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    packages::{permissions::PackagePermission, record::PackageRecord},
    perf::budgets::{INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, SYNTAX_CACHE_BUDGET_BYTES},
    protocol::{
        BehaviorVersion, DocumentId, DocumentVersion, IncrementalParseUpdate, ParseByteRange,
        ParseEditNotification, ParsePolicy, ParseWindowSnapshot, SyntaxMemoryBudget,
    },
    server::decorations::validate_decoration_set,
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
    PayloadBudgetExceeded {
        bytes: usize,
        budget: usize,
    },
    SerializationFailed,
    ResultChannelClosed,
    HandlerFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseHandlerMeta {
    pub package_prefix: String,
    pub mode_id: String,
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
}

#[derive(Clone)]
pub struct ParseCoordinator {
    inner: Arc<Mutex<ParseCoordinatorInner>>,
    updates_tx: mpsc::UnboundedSender<IncrementalParseUpdate>,
    updates_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<IncrementalParseUpdate>>>,
}

struct ParseCoordinatorInner {
    handlers: HashMap<HandlerKey, Arc<dyn ParseHandler>>,
    active_tasks: HashMap<TaskKey, JoinHandle<()>>,
    current_versions: HashMap<DocumentId, DocumentVersion>,
    stats: ParseCoordinatorStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HandlerKey {
    package_prefix: String,
    mode_id: String,
}

type TaskKey = HandlerKeyWithDocument;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HandlerKeyWithDocument {
    document_id: DocumentId,
    package_prefix: String,
    mode_id: String,
}

impl ParseCoordinator {
    pub fn new() -> Self {
        let (updates_tx, updates_rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(Mutex::new(ParseCoordinatorInner {
                handlers: HashMap::new(),
                active_tasks: HashMap::new(),
                current_versions: HashMap::new(),
                stats: ParseCoordinatorStats::default(),
            })),
            updates_tx,
            updates_rx: Arc::new(tokio::sync::Mutex::new(updates_rx)),
        }
    }

    pub fn register_handler(
        &self,
        package: &PackageRecord,
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

        let meta = ParseHandlerMeta {
            package_prefix: package.manifest.clay.api_prefix.clone(),
            mode_id: mode_id.into(),
        };
        let key = HandlerKey {
            package_prefix: meta.package_prefix.clone(),
            mode_id: meta.mode_id.clone(),
        };
        let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
        if inner.handlers.contains_key(&key) {
            return Err(ParseCoordinatorError::HandlerAlreadyRegistered {
                package_prefix: key.package_prefix,
                mode_id: key.mode_id,
            });
        }
        inner.handlers.insert(key, Arc::new(handler));
        Ok(meta)
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
        let task_key = TaskKey {
            document_id: request.document_id,
            package_prefix: request.package_prefix.clone(),
            mode_id: request.mode_id.clone(),
        };
        let mut notification = request.into_notification();
        if let Some(policy) = policy {
            notification.memory_budget =
                Some(SyntaxMemoryBudget::new(policy.memory_budget_bytes, 0));
        }
        notification.parse_windows = parse_windows;

        let handler = {
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            let handler = inner.handlers.get(&handler_key).cloned().ok_or_else(|| {
                ParseCoordinatorError::HandlerNotRegistered {
                    package_prefix: handler_key.package_prefix.clone(),
                    mode_id: handler_key.mode_id.clone(),
                }
            })?;

            inner
                .current_versions
                .insert(notification.document_id, notification.document_version);
            if let Some(previous) = inner.active_tasks.remove(&task_key) {
                previous.abort();
                inner.stats.cancelled_superseded_tasks += 1;
            }
            inner.stats.scheduled_tasks += 1;
            handler
        };

        let coordinator = self.clone();
        let spawned_task_key = task_key.clone();
        let task = tokio::spawn(async move {
            let result = handler.parse(notification).await;
            coordinator.finish_task(spawned_task_key, result);
        });

        self.inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .active_tasks
            .insert(task_key, task);
        Ok(())
    }

    fn finish_task(
        &self,
        task_key: TaskKey,
        result: Result<IncrementalParseUpdate, ParseCoordinatorError>,
    ) {
        let Ok(update) = result else {
            let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
            inner.active_tasks.remove(&task_key);
            return;
        };

        match self.validate_update(&update) {
            Ok(()) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.published_updates += 1;
                drop(inner);
                let _ = self.updates_tx.send(update);
            }
            Err(ParseCoordinatorError::StaleDocumentVersion { .. }) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
                inner.stats.stale_results_rejected += 1;
            }
            Err(_) => {
                let mut inner = self.inner.lock().expect("parse coordinator lock poisoned");
                inner.active_tasks.remove(&task_key);
            }
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
        if let Some(decorations) = &update.decoration_update {
            if decorations.document_id != update.document_id
                || decorations.document_version != update.document_version
            {
                return Err(ParseCoordinatorError::DecorationVersionMismatch {
                    decoration_version: decorations.document_version,
                    parse_version: update.document_version,
                });
            }
            validate_decoration_set(current_version, decorations.clone(), None).map_err(
                |error| {
                    ParseCoordinatorError::HandlerFailed(format!(
                        "decoration validation failed: {error:?}"
                    ))
                },
            )?;
        }
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

    pub async fn next_update(&self) -> Option<IncrementalParseUpdate> {
        self.updates_rx.lock().await.recv().await
    }

    pub fn stats(&self) -> ParseCoordinatorStats {
        self.inner
            .lock()
            .expect("parse coordinator lock poisoned")
            .stats
            .clone()
    }
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
