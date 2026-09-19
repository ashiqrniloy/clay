// Auto-extracted from js_runtime.rs (Plan 090 task 3). Private submodule: worker family.
use std::{
    collections::VecDeque,
    fmt,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use deno_core::{JsRuntime, ModuleSpecifier, RuntimeOptions, v8};
use tokio::sync::oneshot;

use crate::perf::metrics::global_recorder;
use crate::protocol::{IncrementalParseUpdate, ParseEditNotification};
use crate::server::configuration::ConfigurationRuntime;
use crate::server::ops::{ClayOpState, PackageLoadEntryAllowlist, init_runtime_extension};
use crate::server::workspace::WorkspaceState;

use super::RuntimeLane;
use super::error::{ClayRuntimeError, ClayRuntimeEvaluation, DocumentAnalysisInvocation};
use super::evaluation::{
    evaluate_js_completion_provider, evaluate_js_document_analyzer,
    evaluate_js_language_intelligence_provider, evaluate_js_parse_handler, evaluate_loaded_module,
};
use super::source::{CONTROLLED_MAIN_SPECIFIER, ClayModuleLoader};

pub(crate) enum RuntimeEntry {
    ControlledSource(String),
    ConfigurationRoot(PathBuf),
}

pub(super) struct RuntimeWorker {
    pub(super) sender: RuntimeCommandSender,
    pub(super) op_state: Arc<ClayOpState>,
    join: Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// Producer handle for one lane's command mailbox. Cheap to clone; every clone
/// shares the lane's queue, capacity, and supersede policy.
#[derive(Clone)]
pub(crate) struct RuntimeCommandSender {
    mailbox: Arc<CommandMailbox>,
}

/// A command the mailbox refused because the lane's worker is gone. Carries the
/// command back (boxed: the command enum is large) so the caller can replace
/// the lane and retry it.
pub(crate) struct CommandSendError(pub(crate) Box<RuntimeCommand>);

impl RuntimeCommandSender {
    pub(crate) fn send(&self, command: RuntimeCommand) -> Result<(), CommandSendError> {
        self.mailbox.enqueue(command).map_err(CommandSendError)
    }

    #[cfg(test)]
    pub(crate) fn queue_stats(&self) -> CommandQueueStats {
        self.mailbox.stats()
    }

    fn close(&self) {
        self.mailbox.close();
    }
}

impl fmt::Debug for RuntimeWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeWorker")
            .finish_non_exhaustive()
    }
}

impl Drop for RuntimeWorker {
    fn drop(&mut self) {
        // Closing drains whatever is still queued (bounded by the mailbox
        // capacity) and then stops the command thread; no sentinel command is
        // needed. Callers holding a sender clone keep failing until the lane is
        // rewired to the replacement worker.
        self.sender.close();
        if let Some(join) = self
            .join
            .lock()
            .expect("Clay runtime worker join mutex poisoned")
            .take()
        {
            let _ = join.join();
        }
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "runtime worker commands stay on a single internal channel; boxing parse payloads is unnecessary until profiling says otherwise"
)]
pub(crate) enum RuntimeCommand {
    Evaluate {
        entry: RuntimeEntry,
        workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
        runtime_document_id: crate::protocol::DocumentId,
        /// Host-stamped package provenance for package-entry evaluations
        /// (package adoption/loading driven from Rust, and tests). `None`
        /// for configuration/orchestration evaluations; the package-load op
        /// stamps provenance itself mid-evaluation.
        package_context: Option<crate::server::ops::PackageContext>,
        metric: &'static str,
        response: oneshot::Sender<Result<ClayRuntimeEvaluation, ClayRuntimeError>>,
    },
    Parse {
        registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
        notification: ParseEditNotification,
        response: oneshot::Sender<Result<IncrementalParseUpdate, ClayRuntimeError>>,
    },
    Completion {
        registration: crate::server::completion::JsCompletionProviderRegistration,
        request: crate::protocol::CompletionRequest,
        window: crate::server::completion::CompletionDocumentWindow,
        response: oneshot::Sender<Result<crate::protocol::CompletionResultSet, ClayRuntimeError>>,
    },
    DocumentAnalysis {
        registration: crate::server::document_analysis::JsDocumentAnalyzerRegistration,
        event: crate::server::document_analysis::DocumentAnalysisEvent,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
        invocation_id: u64,
        response: oneshot::Sender<Result<DocumentAnalysisInvocation, ClayRuntimeError>>,
    },
    LanguageIntelligence {
        registration:
            crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration,
        request: crate::protocol::LanguageIntelligenceRequest,
        window: crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
        response:
            oneshot::Sender<Result<crate::protocol::LanguageIntelligenceResult, ClayRuntimeError>>,
    },
    /// Follow-up round (`editor-control`): host-replicated active editor mode
    /// snapshot pushed by the trusted worker after behavior-manifest
    /// replacements. The third-party worker stores it for its editor-op gate.
    UpdateActiveEditorMode(Option<String>),
}

/// Host-stamped identity of one removable unit of work (Plan 127 P2). Built
/// only from host-owned fields: the protocol request envelope (client +
/// document) and the runtime's own registration token — never a
/// package-supplied string, so one package can neither claim nor evict
/// another's queued work.
#[derive(Clone, PartialEq, Eq)]
struct SupersedeKey {
    kind: CommandKind,
    client_id: crate::protocol::ClientId,
    document_id: crate::protocol::DocumentId,
    provider_token: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandKind {
    Completion,
    LanguageIntelligence,
}

/// Observable state of one lane's command mailbox.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandQueueStats {
    pub(crate) capacity: usize,
    pub(crate) pending_supersedable: usize,
    pub(crate) peak_pending: usize,
    pub(crate) superseded: u64,
    pub(crate) evicted: u64,
}

impl RuntimeCommand {
    /// Identity used to drop an undelivered command in favour of a newer one.
    /// `None` for commands that must always run: evaluations and parse/
    /// analysis deltas carry side effects or incremental ranges a newer
    /// sibling does not re-encode, and mode replication is host state. Their
    /// producers already gate them (one pending job per document).
    fn supersede_key(&self) -> Option<SupersedeKey> {
        match self {
            Self::Completion {
                registration,
                request,
                ..
            } => Some(SupersedeKey {
                kind: CommandKind::Completion,
                client_id: request.client_id,
                document_id: request.document_id,
                provider_token: registration.token.clone(),
            }),
            Self::LanguageIntelligence {
                registration,
                request,
                ..
            } => Some(SupersedeKey {
                kind: CommandKind::LanguageIntelligence,
                client_id: request.client_id,
                document_id: request.document_id,
                provider_token: registration.token.clone(),
            }),
            Self::Evaluate { .. }
            | Self::Parse { .. }
            | Self::DocumentAnalysis { .. }
            | Self::UpdateActiveEditorMode(_) => None,
        }
    }

    fn is_supersedable(&self) -> bool {
        matches!(
            self,
            Self::Completion { .. } | Self::LanguageIntelligence { .. }
        )
    }

    /// Answer a command the mailbox dropped before it ran.
    fn fail(self, error: ClayRuntimeError) {
        match self {
            Self::Evaluate { response, .. } => {
                let _ = response.send(Err(error));
            }
            Self::Parse { response, .. } => {
                let _ = response.send(Err(error));
            }
            Self::Completion { response, .. } => {
                let _ = response.send(Err(error));
            }
            Self::DocumentAnalysis { response, .. } => {
                let _ = response.send(Err(error));
            }
            Self::LanguageIntelligence { response, .. } => {
                let _ = response.send(Err(error));
            }
            Self::UpdateActiveEditorMode(_) => {}
        }
    }
}

/// Bounded, supersede-aware command queue for one runtime lane (Plan 127 P2).
///
/// The previous `mpsc::channel()` was unbounded, so a lane busy to its
/// evaluation timeout accumulated every client-driven request that arrived
/// meanwhile. This mailbox caps the supersedable backlog at
/// `JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY`: a newer command for the same
/// `SupersedeKey` replaces the undelivered older one, and at capacity the
/// oldest supersedable command is evicted. Both dropped commands answer
/// `ClayRuntimeError::Superseded` instead of delivering a stale result.
pub(crate) struct CommandMailbox {
    capacity: usize,
    state: Mutex<MailboxState>,
    ready: Condvar,
}

#[derive(Default)]
struct MailboxState {
    queue: VecDeque<RuntimeCommand>,
    closed: bool,
    pending_supersedable: usize,
    peak_pending: usize,
    superseded: u64,
    evicted: u64,
}

impl CommandMailbox {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            state: Mutex::new(MailboxState::default()),
            ready: Condvar::new(),
        }
    }

    /// Queue one command, newest-wins for supersedable work. Fails only when
    /// the lane's worker is gone; the command comes back (boxed) for a retry on
    /// the replacement lane.
    fn enqueue(&self, command: RuntimeCommand) -> Result<(), Box<RuntimeCommand>> {
        let mut state = self
            .state
            .lock()
            .expect("Clay runtime command mailbox poisoned");
        if state.closed {
            return Err(Box::new(command));
        }
        let mut dropped_metric = None;
        // One predicate gates the accounting and the supersede decision, so the
        // pending count stays balanced with `recv()`. The work key is found by
        // scanning the queue: it is bounded by `capacity`, so no side index map
        // (and no second bookkeeping structure to keep consistent) is needed.
        if command.is_supersedable() {
            let replacement = command.supersede_key().as_ref().and_then(|key| {
                state
                    .queue
                    .iter()
                    .position(|queued| queued.supersede_key().as_ref() == Some(key))
            });
            // Same work key wins first; otherwise make room by dropping the
            // oldest supersedable command (stale-first) once the cap is hit.
            let evicted = match replacement {
                Some(index) => Some(index),
                None if state.pending_supersedable >= self.capacity => {
                    state.queue.iter().position(RuntimeCommand::is_supersedable)
                }
                None => None,
            };
            let same_key = evicted.is_some() && evicted == replacement;
            if let Some(dropped) = evicted.and_then(|index| state.queue.remove(index)) {
                state.pending_supersedable -= 1;
                if same_key {
                    state.superseded += 1;
                    dropped_metric = Some(crate::perf::metrics::JS_RUNTIME_COMMAND_SUPERSEDED);
                } else {
                    state.evicted += 1;
                    dropped_metric = Some(crate::perf::metrics::JS_RUNTIME_COMMAND_EVICTED);
                }
                dropped.fail(ClayRuntimeError::Superseded);
            }
            state.pending_supersedable += 1;
            state.peak_pending = state.peak_pending.max(state.pending_supersedable);
        }
        state.queue.push_back(command);
        drop(state);
        if let Some(metric) = dropped_metric {
            global_recorder().record_counter(metric, 1);
        }
        self.ready.notify_one();
        Ok(())
    }

    /// Next command in FIFO order, blocking until one arrives. `None` once the
    /// mailbox is closed and drained.
    fn recv(&self) -> Option<RuntimeCommand> {
        let mut state = self
            .state
            .lock()
            .expect("Clay runtime command mailbox poisoned");
        loop {
            if let Some(command) = state.queue.pop_front() {
                if command.is_supersedable() {
                    state.pending_supersedable -= 1;
                }
                return Some(command);
            }
            if state.closed {
                return None;
            }
            state = self
                .ready
                .wait(state)
                .expect("Clay runtime command mailbox poisoned");
        }
    }

    /// Stop accepting commands and wake the worker so it drains and exits.
    fn close(&self) {
        let mut state = self
            .state
            .lock()
            .expect("Clay runtime command mailbox poisoned");
        state.closed = true;
        drop(state);
        self.ready.notify_all();
    }

    #[cfg(test)]
    fn stats(&self) -> CommandQueueStats {
        let state = self
            .state
            .lock()
            .expect("Clay runtime command mailbox poisoned");
        CommandQueueStats {
            capacity: self.capacity,
            pending_supersedable: state.pending_supersedable,
            peak_pending: state.peak_pending,
            superseded: state.superseded,
            evicted: state.evicted,
        }
    }
}

pub(super) fn start_runtime_worker(
    timeout: Duration,
    heap_limit_bytes: usize,
    domain: crate::packages::bundled::RuntimeDomain,
    package_service: Arc<std::sync::Mutex<crate::packages::service::PackageService>>,
    load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
    lane: RuntimeLane,
) -> Arc<RuntimeWorker> {
    let default_workspace = Arc::new(tokio::sync::Mutex::new(WorkspaceState::new()));
    let op_state = Arc::new(ClayOpState::new_for_domain(
        Arc::clone(&default_workspace),
        1,
        domain,
        package_service,
        load_entry_allowlist,
    ));
    start_runtime_worker_with_state(
        timeout,
        heap_limit_bytes,
        domain,
        default_workspace,
        op_state,
        lane,
    )
}

pub(super) fn start_runtime_worker_with_state(
    timeout: Duration,
    heap_limit_bytes: usize,
    domain: crate::packages::bundled::RuntimeDomain,
    default_workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    op_state: Arc<ClayOpState>,
    lane: RuntimeLane,
) -> Arc<RuntimeWorker> {
    let mailbox = Arc::new(CommandMailbox::new(
        crate::perf::budgets::JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY,
    ));
    let worker_mailbox = Arc::clone(&mailbox);
    let worker_state = Arc::clone(&op_state);
    let join = std::thread::Builder::new()
        .name(match (domain, lane) {
            (crate::packages::bundled::RuntimeDomain::Trusted, RuntimeLane::General) => {
                "clay-js-runtime".to_string()
            }
            (crate::packages::bundled::RuntimeDomain::Trusted, RuntimeLane::Latency) => {
                "clay-js-runtime-latency".to_string()
            }
            (crate::packages::bundled::RuntimeDomain::ThirdParty, RuntimeLane::General) => {
                "clay-js-runtime-third-party".to_string()
            }
            (crate::packages::bundled::RuntimeDomain::ThirdParty, RuntimeLane::Latency) => {
                "clay-js-runtime-third-party-latency".to_string()
            }
        })
        .spawn(move || {
            run_runtime_worker(
                worker_mailbox,
                timeout,
                heap_limit_bytes,
                domain,
                default_workspace,
                worker_state,
            )
        })
        .expect("failed to spawn persistent JS runtime worker");
    Arc::new(RuntimeWorker {
        sender: RuntimeCommandSender { mailbox },
        op_state,
        join: std::sync::Mutex::new(Some(join)),
    })
}

pub(super) fn run_runtime_worker(
    mailbox: Arc<CommandMailbox>,
    timeout: Duration,
    heap_limit_bytes: usize,
    domain: crate::packages::bundled::RuntimeDomain,
    default_workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    op_state: Arc<ClayOpState>,
) {
    let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER)
        .expect("controlled runtime specifier must parse");
    let loader = Rc::new(ClayModuleLoader::new(
        main_specifier,
        None,
        None,
        op_state.load_entry_allowlist(),
        domain,
    ));
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("persistent JS runtime tokio runtime must build");
    let (mut runtime, heap_limit_hit) = create_js_runtime(
        Arc::clone(&op_state),
        Rc::clone(&loader),
        heap_limit_bytes,
        domain,
    );
    let mut controlled_evaluation_id = 0_u64;
    let mut main_module_loaded = false;

    while let Some(command) = mailbox.recv() {
        match command {
            RuntimeCommand::Evaluate {
                entry,
                workspace,
                runtime_document_id,
                package_context,
                metric,
                response,
            } => {
                controlled_evaluation_id = controlled_evaluation_id.saturating_add(1);
                let configuration_evaluation = matches!(&entry, RuntimeEntry::ConfigurationRoot(_));
                if configuration_evaluation
                    && domain != crate::packages::bundled::RuntimeDomain::Trusted
                {
                    let _ = response.send(Err(ClayRuntimeError::Runtime(
                        "domain.trusted_only: configuration evaluation requires the trusted runtime domain"
                            .to_string(),
                    )));
                    continue;
                }
                let loaded_entry = match prepare_runtime_entry(entry, controlled_evaluation_id) {
                    Ok(loaded_entry) => loaded_entry,
                    Err(error) => {
                        let _ = response.send(Err(error));
                        continue;
                    }
                };
                op_state.set_runtime_context(
                    workspace.unwrap_or_else(|| Arc::clone(&default_workspace)),
                    runtime_document_id,
                    configuration_evaluation,
                );
                let poisoned = run_bounded_evaluation(
                    &mut runtime,
                    &op_state,
                    heap_limit_bytes,
                    &heap_limit_hit,
                    package_context,
                    |runtime| {
                        loader.set_entry(
                            loaded_entry.main_specifier.clone(),
                            loaded_entry.main_source.clone(),
                            loaded_entry.configuration.clone(),
                        );
                        let recorder = global_recorder();
                        let _scope = recorder.scope(metric);
                        let result = tokio_runtime.block_on(evaluate_loaded_module(
                            runtime,
                            &op_state,
                            loaded_entry,
                            timeout,
                            !main_module_loaded,
                            &heap_limit_hit,
                        ));
                        main_module_loaded = true;
                        result
                    },
                    response,
                );
                if poisoned {
                    break;
                }
            }
            RuntimeCommand::Parse {
                registration,
                notification,
                response,
            } => {
                // Stamp host-owned handler provenance so publications inside
                // the callback resolve to the registration's package.
                let poisoned = run_bounded_evaluation(
                    &mut runtime,
                    &op_state,
                    heap_limit_bytes,
                    &heap_limit_hit,
                    Some(crate::server::ops::PackageContext::from_record(
                        &registration.package,
                    )),
                    |runtime| {
                        tokio_runtime.block_on(evaluate_js_parse_handler(
                            runtime,
                            &op_state,
                            &loader,
                            &registration,
                            notification,
                            timeout.min(Duration::from_millis(registration.timeout_ms)),
                            &heap_limit_hit,
                        ))
                    },
                    response,
                );
                if poisoned {
                    break;
                }
            }
            RuntimeCommand::Completion {
                registration,
                request,
                window,
                response,
            } => {
                let poisoned = run_bounded_evaluation(
                    &mut runtime,
                    &op_state,
                    heap_limit_bytes,
                    &heap_limit_hit,
                    Some(crate::server::ops::PackageContext::from_record(
                        &registration.package,
                    )),
                    |runtime| {
                        tokio_runtime.block_on(evaluate_js_completion_provider(
                            runtime,
                            &op_state,
                            &loader,
                            &registration,
                            request,
                            window,
                            timeout.min(Duration::from_millis(registration.meta.timeout_ms)),
                            &heap_limit_hit,
                        ))
                    },
                    response,
                );
                if poisoned {
                    break;
                }
            }
            RuntimeCommand::DocumentAnalysis {
                registration,
                event,
                workspace,
                invocation_id,
                response,
            } => {
                let runtime_document_id = event.document_id().unwrap_or(1);
                op_state.set_runtime_context(workspace, runtime_document_id, false);
                let analysis_timeout = if matches!(
                    &event,
                    crate::server::document_analysis::DocumentAnalysisEvent::Shutdown
                ) {
                    Duration::from_millis(
                        crate::perf::budgets::DOCUMENT_ANALYSIS_GRACEFUL_SHUTDOWN_MS,
                    )
                } else {
                    timeout
                };
                let poisoned = run_bounded_evaluation(
                    &mut runtime,
                    &op_state,
                    heap_limit_bytes,
                    &heap_limit_hit,
                    Some(crate::server::ops::PackageContext::from_record(
                        &registration.package,
                    )),
                    |runtime| {
                        tokio_runtime.block_on(evaluate_js_document_analyzer(
                            runtime,
                            &op_state,
                            &loader,
                            &registration,
                            event,
                            invocation_id,
                            analysis_timeout,
                            &heap_limit_hit,
                        ))
                    },
                    response,
                );
                if poisoned {
                    break;
                }
            }
            RuntimeCommand::LanguageIntelligence {
                registration,
                request,
                window,
                response,
            } => {
                let poisoned = run_bounded_evaluation(
                    &mut runtime,
                    &op_state,
                    heap_limit_bytes,
                    &heap_limit_hit,
                    Some(crate::server::ops::PackageContext::from_record(
                        &registration.package,
                    )),
                    |runtime| {
                        tokio_runtime.block_on(evaluate_js_language_intelligence_provider(
                            runtime,
                            &op_state,
                            &loader,
                            &registration,
                            request,
                            window,
                            timeout.min(Duration::from_millis(registration.meta.timeout_ms)),
                            &heap_limit_hit,
                        ))
                    },
                    response,
                );
                if poisoned {
                    break;
                }
            }
            RuntimeCommand::UpdateActiveEditorMode(mode_id) => {
                op_state.set_replicated_active_editor_mode(mode_id);
            }
        }
    }
}

/// Shared epilogue for the five bounded worker commands: reset per-evaluation
/// host state, run `evaluate`, send the response, and report whether the
/// isolate must be discarded. The poison decision (timeout or heap limit), the
/// heap-limit restore for a recovered evaluation, and the "response before
/// break" ordering live only here.
pub(super) fn run_bounded_evaluation<T>(
    runtime: &mut JsRuntime,
    op_state: &ClayOpState,
    heap_limit_bytes: usize,
    heap_limit_hit: &Arc<std::sync::atomic::AtomicBool>,
    package_context: Option<crate::server::ops::PackageContext>,
    evaluate: impl FnOnce(&mut JsRuntime) -> Result<T, ClayRuntimeError>,
    response: oneshot::Sender<Result<T, ClayRuntimeError>>,
) -> bool {
    op_state.begin_evaluation();
    if let Some(context) = package_context {
        op_state.set_current_package(Some(context));
        op_state.enter_package_activation();
    }
    heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
    let result = evaluate(runtime);
    let poisoned = matches!(
        result,
        Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
    );
    if !poisoned {
        // The evaluation completed, so any termination request it armed (near-heap
        // callback or watchdog racing the completion) is moot, and v8 keeps the
        // raised limit the near-heap callback returned. Both must be undone before
        // another command runs.
        restore_heap_limit(runtime, heap_limit_hit, heap_limit_bytes);
    }
    let _ = response.send(result);
    poisoned
}

/// Install the near-heap-limit supervision: report the hit to the worker,
/// request termination so the offending evaluation cannot keep allocating, and
/// hand v8 a doubled limit so the termination has room to unwind.
fn install_heap_limit_callback(
    runtime: &mut JsRuntime,
    heap_limit_hit: &Arc<std::sync::atomic::AtomicBool>,
) {
    let callback_flag = Arc::clone(heap_limit_hit);
    let terminate_handle = runtime.v8_isolate().thread_safe_handle();
    runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
        callback_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        terminate_handle.terminate_execution();
        current_limit.saturating_mul(2)
    });
}

/// Undo the limit the near-heap callback raised. v8 latches the value the
/// callback returns until the limit is set again, so without this a recovered
/// evaluation would leave the isolate running with doubled (and re-doubling on
/// every later hit) package memory authority. `remove_near_heap_limit_callback`
/// restores the configured budget, clamped down to what the live heap needs, and
/// uninstalls the callback, so supervision is re-armed afterwards.
fn restore_heap_limit(
    runtime: &mut JsRuntime,
    heap_limit_hit: &Arc<std::sync::atomic::AtomicBool>,
    heap_limit_bytes: usize,
) {
    let _ = runtime.v8_isolate().cancel_terminate_execution();
    runtime.remove_near_heap_limit_callback(heap_limit_bytes);
    install_heap_limit_callback(runtime, heap_limit_hit);
}

pub(super) fn create_js_runtime(
    op_state: Arc<ClayOpState>,
    loader: Rc<ClayModuleLoader>,
    heap_limit_bytes: usize,
    domain: crate::packages::bundled::RuntimeDomain,
) -> (JsRuntime, Arc<std::sync::atomic::AtomicBool>) {
    let create_params = v8::Isolate::create_params().heap_limits(0, heap_limit_bytes);
    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        extensions: vec![init_runtime_extension(domain)],
        create_params: Some(create_params),
        ..Default::default()
    });
    let heap_limit_hit = Arc::new(std::sync::atomic::AtomicBool::new(false));
    install_heap_limit_callback(&mut runtime, &heap_limit_hit);
    runtime.op_state().borrow_mut().put(op_state);
    (runtime, heap_limit_hit)
}

pub(super) fn prepare_runtime_entry(
    entry: RuntimeEntry,
    controlled_evaluation_id: u64,
) -> Result<LoadedRuntimeEntry, ClayRuntimeError> {
    match entry {
        RuntimeEntry::ControlledSource(source) => Ok(LoadedRuntimeEntry {
            main_specifier: ModuleSpecifier::parse(&format!(
                "clay://runtime/main-{controlled_evaluation_id}.js"
            ))
            .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
            main_source: Some(source),
            configuration: None,
        }),
        RuntimeEntry::ConfigurationRoot(config_root) => {
            let configuration = Arc::new(
                ConfigurationRuntime::from_config_root(config_root)
                    .map_err(ClayRuntimeError::Configuration)?,
            );
            Ok(LoadedRuntimeEntry {
                main_specifier: configuration
                    .entry_specifier()
                    .map_err(ClayRuntimeError::Configuration)?,
                main_source: None,
                configuration: Some(configuration),
            })
        }
    }
}

/// Harvest the registration payload accumulated in a worker's op state into
/// an evaluation snapshot. Used at every evaluation end, and at trusted
/// reload commit to re-register the surviving third-party worker's
/// registrations under the new generation (Plan 061 task 12).
pub(super) fn harvest_op_state_evaluation(op_state: &Arc<ClayOpState>) -> ClayRuntimeEvaluation {
    let behavior_manifest = op_state.behavior_manifest();
    // Phase 20.6: if no explicit `setTheme` ran, resolve the canonical default
    // theme from the appearance preference so a fresh config ships Modus
    // Operandi/Vivendi instead of the bare Clay default. `os_dark = true` is
    // the no-OS-signal fallback (`System` → dark). An explicit theme, once set,
    // is already in `active_theme` and wins. Resolution failure (missing
    // canonical package) leaves `None` so startup never breaks.
    let active_theme = op_state.active_theme().or_else(|| {
        crate::server::ops::theme::resolve_canonical_default_theme(
            op_state,
            op_state.appearance(),
            true,
        )
    });
    ClayRuntimeEvaluation {
        op_records: op_state.records(),
        published_sdui_tree: op_state.published_sdui_tree(),
        published_decoration_set: op_state.published_decoration_set(),
        published_diagnostic_set: op_state.published_diagnostic_set(),
        published_folding_set: op_state.published_folding_set(),
        parse_handlers: op_state.parse_handlers(),
        js_parse_handlers: op_state.js_parse_handlers(),
        behavior_manifest: (behavior_manifest.behavior_version > 1).then_some(behavior_manifest),
        ui_contributions: op_state.ui_contributions(),
        syntax_grammars: op_state.syntax_grammars(),
        syntax_engine_preferences: op_state.syntax_engine_preferences(),
        completion_providers: op_state.completion_providers(),
        js_completion_providers: op_state.js_completion_providers(),
        language_intelligence_providers: op_state.language_intelligence_providers(),
        js_language_intelligence_providers: op_state.js_language_intelligence_providers(),
        document_analyzers: op_state.document_analyzers(),
        active_theme,
        active_typography: op_state.active_typography(),
        active_design_system: op_state.active_design_system(),
        active_icon_pack: op_state.active_icon_pack(),
        configuration_diagnostics: Vec::new(),
    }
}

pub(super) struct LoadedRuntimeEntry {
    pub(super) main_specifier: ModuleSpecifier,
    pub(super) main_source: Option<String>,
    pub(super) configuration: Option<Arc<ConfigurationRuntime>>,
}
