// Auto-extracted from js_runtime.rs (Plan 090 task 3). Private submodule: worker family.
use std::{
    fmt,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, mpsc},
    time::Duration,
};

use deno_core::{JsRuntime, ModuleSpecifier, RuntimeOptions, v8};
use tokio::sync::oneshot;

use crate::perf::metrics::global_recorder;
use crate::protocol::{IncrementalParseUpdate, ParseEditNotification};
use crate::server::configuration::ConfigurationRuntime;
use crate::server::ops::{ClayOpState, PackageLoadEntryAllowlist, init_runtime_extension};
use crate::server::workspace::WorkspaceState;

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
    pub(super) sender: mpsc::Sender<RuntimeCommand>,
    pub(super) op_state: Arc<ClayOpState>,
    join: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
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
        let _ = self.sender.send(RuntimeCommand::Shutdown);
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
    Shutdown,
    /// Follow-up round (`editor-control`): host-replicated active editor mode
    /// snapshot pushed by the trusted worker after behavior-manifest
    /// replacements. The third-party worker stores it for its editor-op gate.
    UpdateActiveEditorMode(Option<String>),
}

pub(super) fn start_runtime_worker(
    timeout: Duration,
    heap_limit_bytes: usize,
    domain: crate::packages::bundled::RuntimeDomain,
    package_service: Arc<std::sync::Mutex<crate::packages::service::PackageService>>,
    load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
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
    )
}

pub(super) fn start_runtime_worker_with_state(
    timeout: Duration,
    heap_limit_bytes: usize,
    domain: crate::packages::bundled::RuntimeDomain,
    default_workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    op_state: Arc<ClayOpState>,
) -> Arc<RuntimeWorker> {
    let (sender, receiver) = mpsc::channel();
    let worker_state = Arc::clone(&op_state);
    let join = std::thread::Builder::new()
        .name(match domain {
            crate::packages::bundled::RuntimeDomain::Trusted => "clay-js-runtime".to_string(),
            crate::packages::bundled::RuntimeDomain::ThirdParty => {
                "clay-js-runtime-third-party".to_string()
            }
        })
        .spawn(move || {
            run_runtime_worker(
                receiver,
                timeout,
                heap_limit_bytes,
                domain,
                default_workspace,
                worker_state,
            )
        })
        .expect("failed to spawn persistent JS runtime worker");
    Arc::new(RuntimeWorker {
        sender,
        op_state,
        join: std::sync::Mutex::new(Some(join)),
    })
}

pub(super) fn run_runtime_worker(
    receiver: mpsc::Receiver<RuntimeCommand>,
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

    for command in receiver {
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
                let result = prepare_runtime_entry(entry, controlled_evaluation_id).and_then(
                    |loaded_entry| {
                        op_state.set_runtime_context(
                            workspace.unwrap_or_else(|| Arc::clone(&default_workspace)),
                            runtime_document_id,
                            configuration_evaluation,
                        );
                        op_state.begin_evaluation();
                        if let Some(context) = package_context {
                            op_state.set_current_package(Some(context));
                            op_state.enter_package_activation();
                        }
                        heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
                        loader.set_entry(
                            loaded_entry.main_specifier.clone(),
                            loaded_entry.main_source.clone(),
                            loaded_entry.configuration.clone(),
                        );
                        let recorder = global_recorder();
                        let _scope = recorder.scope(metric);
                        let result = tokio_runtime.block_on(evaluate_loaded_module(
                            &mut runtime,
                            &op_state,
                            loaded_entry,
                            timeout,
                            !main_module_loaded,
                            &heap_limit_hit,
                        ));
                        main_module_loaded = true;
                        result
                    },
                );
                let timed_out = matches!(result, Err(ClayRuntimeError::Timeout));
                let heap_limited = matches!(result, Err(ClayRuntimeError::HeapLimit));
                let _ = response.send(result);
                if timed_out || heap_limited {
                    break;
                }
            }
            RuntimeCommand::Parse {
                registration,
                notification,
                response,
            } => {
                op_state.begin_evaluation();
                // Stamp host-owned handler provenance so publications inside
                // the callback resolve to the registration's package.
                op_state.set_current_package(Some(
                    crate::server::ops::PackageContext::from_record(&registration.package),
                ));
                op_state.enter_package_activation();
                heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
                let result = tokio_runtime.block_on(evaluate_js_parse_handler(
                    &mut runtime,
                    &op_state,
                    &loader,
                    &registration,
                    notification,
                    timeout.min(Duration::from_millis(registration.timeout_ms)),
                    &heap_limit_hit,
                ));
                let timed_out = matches!(result, Err(ClayRuntimeError::Timeout));
                let heap_limited = matches!(result, Err(ClayRuntimeError::HeapLimit));
                let _ = response.send(result);
                if timed_out || heap_limited {
                    break;
                }
            }
            RuntimeCommand::Completion {
                registration,
                request,
                window,
                response,
            } => {
                op_state.begin_evaluation();
                op_state.set_current_package(Some(
                    crate::server::ops::PackageContext::from_record(&registration.package),
                ));
                op_state.enter_package_activation();
                heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
                let result = tokio_runtime.block_on(evaluate_js_completion_provider(
                    &mut runtime,
                    &op_state,
                    &loader,
                    &registration,
                    request,
                    window,
                    timeout.min(Duration::from_millis(registration.meta.timeout_ms)),
                    &heap_limit_hit,
                ));
                let timed_out = matches!(result, Err(ClayRuntimeError::Timeout));
                let heap_limited = matches!(result, Err(ClayRuntimeError::HeapLimit));
                let _ = response.send(result);
                if timed_out || heap_limited {
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
                op_state.begin_evaluation();
                op_state.set_current_package(Some(
                    crate::server::ops::PackageContext::from_record(&registration.package),
                ));
                op_state.enter_package_activation();
                heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
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
                let result = tokio_runtime.block_on(evaluate_js_document_analyzer(
                    &mut runtime,
                    &op_state,
                    &loader,
                    &registration,
                    event,
                    invocation_id,
                    analysis_timeout,
                    &heap_limit_hit,
                ));
                let timed_out = matches!(result, Err(ClayRuntimeError::Timeout));
                let heap_limited = matches!(result, Err(ClayRuntimeError::HeapLimit));
                let _ = response.send(result);
                if timed_out || heap_limited {
                    break;
                }
            }
            RuntimeCommand::LanguageIntelligence {
                registration,
                request,
                window,
                response,
            } => {
                op_state.begin_evaluation();
                op_state.set_current_package(Some(
                    crate::server::ops::PackageContext::from_record(&registration.package),
                ));
                op_state.enter_package_activation();
                heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
                let result = tokio_runtime.block_on(evaluate_js_language_intelligence_provider(
                    &mut runtime,
                    &op_state,
                    &loader,
                    &registration,
                    request,
                    window,
                    timeout.min(Duration::from_millis(registration.meta.timeout_ms)),
                    &heap_limit_hit,
                ));
                let timed_out = matches!(result, Err(ClayRuntimeError::Timeout));
                let heap_limited = matches!(result, Err(ClayRuntimeError::HeapLimit));
                let _ = response.send(result);
                if timed_out || heap_limited {
                    break;
                }
            }
            RuntimeCommand::Shutdown => break,
            RuntimeCommand::UpdateActiveEditorMode(mode_id) => {
                op_state.set_replicated_active_editor_mode(mode_id);
            }
        }
    }
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
    let callback_flag = Arc::clone(&heap_limit_hit);
    let terminate_handle = runtime.v8_isolate().thread_safe_handle();
    runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
        callback_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        terminate_handle.terminate_execution();
        current_limit.saturating_mul(2)
    });
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
        configuration_diagnostics: Vec::new(),
    }
}

pub(super) struct LoadedRuntimeEntry {
    pub(super) main_specifier: ModuleSpecifier,
    pub(super) main_source: Option<String>,
    pub(super) configuration: Option<Arc<ConfigurationRuntime>>,
}
