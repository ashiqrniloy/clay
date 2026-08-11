use std::{
    error::Error,
    fmt,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use deno_core::{
    JsRuntime, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind, RuntimeOptions,
    error::ModuleLoaderError, v8,
};
use deno_error::JsErrorBox;
use tokio::{sync::oneshot, task};

use crate::perf::budgets::{JS_RUNTIME_EVALUATION_TIMEOUT_MS, JS_RUNTIME_HEAP_LIMIT_BYTES};
use crate::perf::metrics::global_recorder;
use crate::protocol::{
    DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DiagnosticSet,
    DiagnosticSeverity, DiagnosticSpan, IncrementalParseUpdate, ParseByteRange,
    ParseEditNotification, RuntimeDiagnostic,
};

use super::{
    completion::CompletionProviderError,
    configuration::{ConfigurationError, ConfigurationRuntime},
    ops::{ClayOpState, PackageLoadEntryAllowlist, init_runtime_extension},
    workspace::WorkspaceState,
};

const CONTROLLED_MAIN_SPECIFIER: &str = "clay://runtime/main.js";
const MARKDOWN_IT_MODULE_SPECIFIER: &str = "clay://vendor/markdown-it.js";

/// One persistent domain worker plus its per-domain generation state
/// (poison flag, evaluation metric, replaceable worker handle).
#[derive(Debug, Clone)]
struct DomainRuntime {
    poisoned: Arc<std::sync::atomic::AtomicBool>,
    evaluations: Arc<AtomicU64>,
    /// Per-domain runtime generation, bumped on every worker replacement
    /// (poison recovery, trusted reload). Registration ownership metadata
    /// per Plan 061 task 12.
    generation: Arc<AtomicU64>,
    worker: Arc<std::sync::Mutex<Arc<RuntimeWorker>>>,
}

/// Isolated server-side Clay JavaScript runtime boundary. Owns exactly two
/// persistent application runtimes (Plan 061 trust domains): one trusted
/// runtime for configuration and bundled first-party packages, and one shared
/// third-party runtime for adopted packages. Each domain has its own op
/// extension, module-loader allowlist, facade export set, poison/restart
/// path, and evaluation metric; both share the host-owned `PackageService`
/// and package load-entry allowlist.
#[derive(Clone)]
pub(crate) struct ClayJsRuntimeService {
    evaluations: Arc<AtomicU64>,
    timeout: Duration,
    heap_limit_bytes: usize,
    completion_providers:
        Arc<std::sync::Mutex<Vec<crate::server::completion::CompletionProviderMeta>>>,
    native_syntax_handlers: Arc<std::sync::Mutex<std::collections::HashSet<(u64, String, String)>>>,
    package_service: Arc<std::sync::Mutex<crate::packages::service::PackageService>>,
    load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
    workers_started: Arc<AtomicU64>,
    trusted: DomainRuntime,
    third_party: DomainRuntime,
    /// Follow-up round (`editor-control`): bounded publisher for gated
    /// programmatic editor-command execution requests. Shared by both domain
    /// workers (each op state holds a sender clone) and subscribed by every
    /// connection loop. Survives `production_reload`.
    editor_commands: tokio::sync::broadcast::Sender<crate::protocol::EditorCommandRequest>,
    /// Plan 071 caret-transport fix: runtime caret override channel plus the
    /// current-value store shared by both domain workers. Survives
    /// `production_reload` so one subscription covers reloads; the store
    /// feeds connection initial sync and lag replay.
    caret_styles: tokio::sync::broadcast::Sender<Option<crate::protocol::CaretStyle>>,
    caret_style_state: std::sync::Arc<std::sync::Mutex<Option<crate::protocol::CaretStyle>>>,
    /// Phase 22.1: shell-preferences channel plus the current-value store.
    /// Same lifetime semantics as `caret_styles`: survives reloads, feeds
    /// connection initial sync and lag replay.
    shell_preferences: tokio::sync::broadcast::Sender<crate::protocol::ShellPreferences>,
    shell_preferences_state: std::sync::Arc<std::sync::Mutex<crate::protocol::ShellPreferences>>,
}

impl std::fmt::Debug for ClayJsRuntimeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClayJsRuntimeService")
            .field("evaluations", &self.evaluations)
            .field("timeout", &self.timeout)
            .field("heap_limit_bytes", &self.heap_limit_bytes)
            .field("workers_started", &self.workers_started)
            .field("trusted", &self.trusted)
            .field("third_party", &self.third_party)
            .finish_non_exhaustive()
    }
}

impl Default for ClayJsRuntimeService {
    fn default() -> Self {
        Self::new(Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS))
    }
}

impl ClayJsRuntimeService {
    fn new(timeout: Duration) -> Self {
        Self::new_with_heap_limit(timeout, JS_RUNTIME_HEAP_LIMIT_BYTES)
    }

    fn new_with_heap_limit(timeout: Duration, heap_limit_bytes: usize) -> Self {
        Self::with_package_service(
            timeout,
            heap_limit_bytes,
            crate::packages::service::PackageService::new(
                PathBuf::new(),
                Box::new(crate::packages::manager::FakeBackend::new()),
            ),
        )
    }

    /// Production reload (Plan 061 task 12): a trusted-generation reload
    /// shares the CURRENT third-party domain (worker, poison/generation
    /// state, package authority, load-entry allowlist) so adopted
    /// third-party packages, their providers, and their language-server
    /// sessions survive a configuration reload untouched. Only the trusted
    /// worker is rebuilt and re-runs configuration.
    pub(crate) fn production_reload(current: &Self) -> Self {
        let workers_started = Arc::new(AtomicU64::new(0));
        let trusted = Self::start_domain_runtime(
            crate::packages::bundled::RuntimeDomain::Trusted,
            current.timeout,
            current.heap_limit_bytes,
            &current.package_service,
            &current.load_entry_allowlist,
            &workers_started,
        );
        trusted
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_third_party_commands(
                current
                    .domain_worker(crate::packages::bundled::RuntimeDomain::ThirdParty)
                    .sender
                    .clone(),
            );
        trusted
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_editor_command_publisher(current.editor_commands.clone());
        trusted
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_caret_style_publisher(
                current.caret_styles.clone(),
                std::sync::Arc::clone(&current.caret_style_state),
            );
        Self {
            evaluations: Arc::new(AtomicU64::new(0)),
            timeout: current.timeout,
            heap_limit_bytes: current.heap_limit_bytes,
            completion_providers: Arc::new(std::sync::Mutex::new(Vec::new())),
            native_syntax_handlers: Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
            package_service: Arc::clone(&current.package_service),
            load_entry_allowlist: Arc::clone(&current.load_entry_allowlist),
            workers_started,
            trusted,
            third_party: current.third_party.clone(),
            editor_commands: current.editor_commands.clone(),
            caret_styles: current.caret_styles.clone(),
            caret_style_state: std::sync::Arc::clone(&current.caret_style_state),
            shell_preferences: current.shell_preferences.clone(),
            shell_preferences_state: std::sync::Arc::clone(&current.shell_preferences_state),
        }
    }

    /// Production server runtime: durable approval store at the default
    /// package store root so CLI/native approvals take effect (Plan 061 task
    /// 10). A corrupt or unreadable approval store fails closed with an empty
    /// in-memory store — no third-party package can be approved into
    /// execution until the store is repaired.
    pub(crate) fn production() -> Self {
        let store_root = crate::packages::service::default_store_root();
        let service = crate::packages::service::PackageService::open(
            store_root,
            Box::new(crate::packages::manager::FakeBackend::new()),
        )
        .unwrap_or_else(|error| {
            eprintln!(
                "clay: package approval store unavailable ({error}); third-party packages stay unadopted"
            );
            crate::packages::service::PackageService::new(
                PathBuf::new(),
                Box::new(crate::packages::manager::FakeBackend::new()),
            )
        });
        Self::with_package_service(
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            service,
        )
    }

    fn with_package_service(
        timeout: Duration,
        heap_limit_bytes: usize,
        package_service: crate::packages::service::PackageService,
    ) -> Self {
        let package_service = Arc::new(std::sync::Mutex::new(package_service));
        let load_entry_allowlist = Arc::new(PackageLoadEntryAllowlist::default());
        let workers_started = Arc::new(AtomicU64::new(0));
        let trusted = Self::start_domain_runtime(
            crate::packages::bundled::RuntimeDomain::Trusted,
            timeout,
            heap_limit_bytes,
            &package_service,
            &load_entry_allowlist,
            &workers_started,
        );
        let third_party = Self::start_domain_runtime(
            crate::packages::bundled::RuntimeDomain::ThirdParty,
            timeout,
            heap_limit_bytes,
            &package_service,
            &load_entry_allowlist,
            &workers_started,
        );
        // Wire the cross-domain bridge: trusted config `loadPackage` of an
        // approved third-party package dispatches its load evaluation to the
        // third-party worker (Plan 061 task 12).
        let (editor_commands, _) = tokio::sync::broadcast::channel(16);
        // Plan 071 caret-transport fix: runtime caret override lane.
        let (caret_styles, _) = tokio::sync::broadcast::channel(4);
        let caret_style_state =
            std::sync::Arc::new(std::sync::Mutex::new(None::<crate::protocol::CaretStyle>));
        // Phase 22.1: shell-preferences lane.
        let (shell_preferences, _) = tokio::sync::broadcast::channel(4);
        let shell_preferences_state =
            std::sync::Arc::new(std::sync::Mutex::new(crate::protocol::ShellPreferences {
                pane_focus_policy: "click".to_string(),
            }));
        trusted
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_third_party_commands(
                third_party
                    .worker
                    .lock()
                    .expect("Clay runtime service worker mutex poisoned")
                    .sender
                    .clone(),
            );
        trusted
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_editor_command_publisher(editor_commands.clone());
        trusted
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_caret_style_publisher(
                caret_styles.clone(),
                std::sync::Arc::clone(&caret_style_state),
            );
        trusted
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_shell_preferences_publisher(
                shell_preferences.clone(),
                std::sync::Arc::clone(&shell_preferences_state),
            );
        third_party
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_editor_command_publisher(editor_commands.clone());
        third_party
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_caret_style_publisher(
                caret_styles.clone(),
                std::sync::Arc::clone(&caret_style_state),
            );
        third_party
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned")
            .op_state
            .set_shell_preferences_publisher(
                shell_preferences.clone(),
                std::sync::Arc::clone(&shell_preferences_state),
            );
        Self {
            evaluations: Arc::new(AtomicU64::new(0)),
            timeout,
            heap_limit_bytes,
            completion_providers: Arc::new(std::sync::Mutex::new(Vec::new())),
            native_syntax_handlers: Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
            package_service,
            load_entry_allowlist,
            workers_started,
            trusted,
            third_party,
            editor_commands,
            caret_styles,
            caret_style_state,
            shell_preferences,
            shell_preferences_state,
        }
    }

    fn start_domain_runtime(
        domain: crate::packages::bundled::RuntimeDomain,
        timeout: Duration,
        heap_limit_bytes: usize,
        package_service: &Arc<std::sync::Mutex<crate::packages::service::PackageService>>,
        load_entry_allowlist: &Arc<PackageLoadEntryAllowlist>,
        workers_started: &Arc<AtomicU64>,
    ) -> DomainRuntime {
        workers_started.fetch_add(1, Ordering::Relaxed);
        DomainRuntime {
            poisoned: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            evaluations: Arc::new(AtomicU64::new(0)),
            generation: Arc::new(AtomicU64::new(1)),
            worker: Arc::new(std::sync::Mutex::new(start_runtime_worker(
                timeout,
                heap_limit_bytes,
                domain,
                Arc::clone(package_service),
                Arc::clone(load_entry_allowlist),
            ))),
        }
    }

    /// Follow-up round (`editor-control`): subscribe to gated programmatic
    /// editor-command execution requests. Connection loops forward each
    /// request as `ServerMessage::EditorCommandRequest`.
    pub(crate) fn subscribe_editor_commands(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::protocol::EditorCommandRequest> {
        self.editor_commands.subscribe()
    }

    /// Plan 071 caret-transport fix: subscribe to runtime caret override
    /// updates (`None` clears). Shared across generations.
    pub(crate) fn subscribe_caret_styles(
        &self,
    ) -> tokio::sync::broadcast::Receiver<Option<crate::protocol::CaretStyle>> {
        self.caret_styles.subscribe()
    }

    /// Current runtime caret override for connection initial sync and lag
    /// replay.
    pub(crate) fn caret_style_override(&self) -> Option<crate::protocol::CaretStyle> {
        *self
            .caret_style_state
            .lock()
            .expect("caret style state mutex poisoned")
    }

    /// Phase 22.1: subscribe to shell-preferences updates.
    pub(crate) fn subscribe_shell_preferences(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::protocol::ShellPreferences> {
        self.shell_preferences.subscribe()
    }

    /// Current shell preferences for connection initial sync and lag replay.
    pub(crate) fn shell_preferences(&self) -> crate::protocol::ShellPreferences {
        self.shell_preferences_state
            .lock()
            .expect("shell preferences state mutex poisoned")
            .clone()
    }
    fn domain(&self, domain: crate::packages::bundled::RuntimeDomain) -> &DomainRuntime {
        match domain {
            crate::packages::bundled::RuntimeDomain::Trusted => &self.trusted,
            crate::packages::bundled::RuntimeDomain::ThirdParty => &self.third_party,
        }
    }

    fn replace_domain_worker(&self, domain: crate::packages::bundled::RuntimeDomain) {
        self.workers_started.fetch_add(1, Ordering::Relaxed);
        self.domain(domain)
            .generation
            .fetch_add(1, Ordering::Relaxed);
        let replacement = start_runtime_worker(
            self.timeout,
            self.heap_limit_bytes,
            domain,
            Arc::clone(&self.package_service),
            Arc::clone(&self.load_entry_allowlist),
        );
        // Rewire the cross-domain bridge: the trusted worker dispatches
        // third-party load evaluations to the CURRENT third-party worker.
        if domain == crate::packages::bundled::RuntimeDomain::ThirdParty {
            self.domain_worker(crate::packages::bundled::RuntimeDomain::Trusted)
                .op_state
                .set_third_party_commands(replacement.sender.clone());
        }
        // Replacement workers start unwired: restore the editor-command
        // publisher so the `editor-control` execution channel survives.
        replacement
            .op_state
            .set_editor_command_publisher(self.editor_commands.clone());
        replacement.op_state.set_caret_style_publisher(
            self.caret_styles.clone(),
            std::sync::Arc::clone(&self.caret_style_state),
        );
        replacement.op_state.set_shell_preferences_publisher(
            self.shell_preferences.clone(),
            std::sync::Arc::clone(&self.shell_preferences_state),
        );
        *self
            .domain(domain)
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned") = replacement;
    }

    /// Host-owned package authority shared by both domain runtimes.
    pub(crate) fn package_service(
        &self,
    ) -> &Arc<std::sync::Mutex<crate::packages::service::PackageService>> {
        &self.package_service
    }

    /// Runtime domain hosting an enabled package's code, resolved from the
    /// host-owned enabled record (never from caller-supplied manifests).
    fn enabled_record_domain(
        &self,
        package_name: &str,
        package_version: &str,
    ) -> crate::packages::bundled::RuntimeDomain {
        self.package_service
            .lock()
            .expect("package service mutex poisoned")
            .enabled_records()
            .find(|record| {
                record.manifest.name == package_name && record.manifest.version == package_version
            })
            .map(|record| record.runtime_domain)
            .unwrap_or(crate::packages::bundled::RuntimeDomain::ThirdParty)
    }

    /// Sets a custom evaluation timeout. The default is
    /// [`JS_RUNTIME_EVALUATION_TIMEOUT_MS`]; tests use a short timeout to
    /// exercise the termination path quickly.
    #[cfg(test)]
    pub(crate) fn with_timeout(timeout: Duration) -> Self {
        Self::new(timeout)
    }

    #[cfg(test)]
    pub(crate) fn with_timeout_and_heap_limit(timeout: Duration, heap_limit_bytes: usize) -> Self {
        Self::new_with_heap_limit(timeout, heap_limit_bytes)
    }

    /// Evaluates a controlled server-owned ES module on the persistent runtime worker.
    pub(crate) async fn evaluate_controlled_module(
        &self,
        source: impl Into<String> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_controlled_module_for_document(source, 1)
            .await
    }

    /// Controlled-source evaluation stamped with the runtime document the
    /// evaluation serves. Classification/open-path evaluations use the opened
    /// document's id so harvested per-document mode manifests resolve to the
    /// right layer (Phase 22.2).
    pub(crate) async fn evaluate_controlled_module_for_document(
        &self,
        source: impl Into<String> + Send + 'static,
        runtime_document_id: crate::protocol::DocumentId,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ControlledSource(source.into()),
            None,
            runtime_document_id,
            "runtime.evaluate_controlled_module",
        )
        .await
    }

    pub(crate) async fn load_configuration_from_root(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            None,
            1,
            "runtime.load_configuration",
        )
        .await
    }

    pub(crate) async fn load_configuration_from_root_with_workspace(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            Some(workspace),
            1,
            "runtime.load_configuration_with_workspace",
        )
        .await
    }

    pub(crate) async fn load_configuration_from_root_for_document(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        runtime_document_id: crate::protocol::DocumentId,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            None,
            runtime_document_id,
            "runtime.load_configuration_for_document",
        )
        .await
    }

    async fn evaluate_entry(
        &self,
        entry: RuntimeEntry,
        workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
        runtime_document_id: crate::protocol::DocumentId,
        metric: &'static str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry_for_domain(
            crate::packages::bundled::RuntimeDomain::Trusted,
            entry,
            workspace,
            runtime_document_id,
            None,
            metric,
        )
        .await
    }

    /// Evaluate a module with host-stamped package provenance: package-facing
    /// registration/publication ops inside the evaluation resolve to
    /// `package`'s host-enabled record. Used by Rust-driven package
    /// loading/adoption and by tests; orchestration code that loads packages
    /// through the `loadPackage` op gets provenance stamped by the op itself.
    #[cfg(test)]
    pub(crate) async fn evaluate_entry_as_package(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        package: &crate::packages::record::PackageRecord,
        entry: RuntimeEntry,
        metric: &'static str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry_for_domain(
            domain,
            entry,
            None,
            0,
            Some(crate::server::ops::PackageContext::from_record(package)),
            metric,
        )
        .await
    }

    async fn evaluate_entry_for_domain(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        entry: RuntimeEntry,
        workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
        runtime_document_id: crate::protocol::DocumentId,
        package_context: Option<crate::server::ops::PackageContext>,
        metric: &'static str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let domain_runtime = self.domain(domain);
        if domain_runtime.poisoned.swap(false, Ordering::Relaxed) {
            self.replace_domain_worker(domain);
        }
        let (response, receiver) = oneshot::channel();
        let command = RuntimeCommand::Evaluate {
            entry,
            workspace,
            runtime_document_id,
            package_context,
            metric,
            response,
        };
        if let Err(error) = self.domain_worker(domain).sender.send(command) {
            self.replace_domain_worker(domain);
            self.domain_worker(domain)
                .sender
                .send(error.0)
                .map_err(|_| {
                    ClayRuntimeError::Runtime(
                        "persistent JavaScript runtime worker stopped".to_string(),
                    )
                })?;
        }
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("persistent JavaScript runtime worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            domain_runtime.poisoned.store(true, Ordering::Relaxed);
        } else if let Ok(evaluation) = &result {
            self.evaluations.fetch_add(1, Ordering::Relaxed);
            domain_runtime.evaluations.fetch_add(1, Ordering::Relaxed);
            *self
                .completion_providers
                .lock()
                .expect("completion provider snapshot lock poisoned") =
                evaluation.completion_providers.clone();
        }
        result
    }

    /// Test-only evaluation entry for the third-party domain, used by trust
    /// boundary tests. Production third-party loading arrives with the
    /// adoption/activation tasks.
    #[cfg(test)]
    pub(crate) async fn evaluate_third_party_module(
        &self,
        source: impl Into<String> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry_for_domain(
            crate::packages::bundled::RuntimeDomain::ThirdParty,
            RuntimeEntry::ControlledSource(source.into()),
            None,
            1,
            None,
            "runtime.evaluate_third_party_module",
        )
        .await
    }

    pub(crate) fn completion_providers(
        &self,
    ) -> Vec<crate::server::completion::CompletionProviderMeta> {
        self.completion_providers
            .lock()
            .expect("completion provider snapshot lock poisoned")
            .clone()
    }

    pub(crate) async fn load_default_configuration(
        &self,
    ) -> Result<Option<ClayRuntimeEvaluation>, ClayRuntimeError> {
        let Some(config_root) = ConfigurationRuntime::default_config_root() else {
            return Ok(None);
        };
        if !config_root.join("init.js").is_file() {
            return Ok(None);
        }
        self.load_configuration_from_root(config_root)
            .await
            .map(Some)
    }

    pub(crate) async fn load_default_configuration_with_workspace(
        &self,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    ) -> Result<Option<ClayRuntimeEvaluation>, ClayRuntimeError> {
        let Some(config_root) = ConfigurationRuntime::default_config_root() else {
            return Ok(None);
        };
        if !config_root.join("init.js").is_file() {
            return Ok(None);
        }
        self.load_configuration_from_root_with_workspace(config_root, workspace)
            .await
            .map(Some)
    }

    pub(crate) fn register_parse_handlers(
        &self,
        coordinator: &crate::server::parse_coordinator::ParseCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
    ) -> Result<
        Vec<crate::server::parse_coordinator::ParseHandlerMeta>,
        crate::server::parse_coordinator::ParseCoordinatorError,
    > {
        let mut registered = Vec::new();
        for registration in &evaluation.js_parse_handlers {
            if registration.meta.package_prefix != registration.package.manifest.clay.api_prefix {
                return Err(
                    crate::server::parse_coordinator::ParseCoordinatorError::ProvenanceMismatch,
                );
            }
            match coordinator.register_handler_for_generation(
                &registration.package,
                generation_id,
                registration.meta.mode_id.clone(),
                JsParseHandler {
                    runtime: self.clone(),
                    registration: registration.clone(),
                },
            ) {
                Ok(meta) => registered.push(meta),
                Err(crate::server::parse_coordinator::ParseCoordinatorError::HandlerAlreadyRegistered { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(registered)
    }

    pub(crate) fn register_completion_providers(
        &self,
        coordinator: &crate::server::completion::CompletionCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
    ) -> Result<
        Vec<crate::server::completion::CompletionProviderMeta>,
        crate::server::completion::CompletionProviderRegistryError,
    > {
        let mut registered = Vec::new();
        for registration in &evaluation.js_completion_providers {
            let mut registration = registration.clone();
            registration.meta.generation = generation_id;
            let meta = registration.meta.clone();
            let package = registration.package.clone();
            match coordinator.register_package_for_generation(
                &package,
                meta.clone(),
                JsCompletionProvider {
                    runtime: self.clone(),
                    registration,
                },
            ) {
                Ok(()) => registered.push(meta),
                Err(crate::server::completion::CompletionProviderRegistryError::ProviderAlreadyRegistered { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(registered)
    }

    pub(crate) fn register_language_intelligence_providers(
        &self,
        coordinator: &crate::server::language_intelligence::LanguageIntelligenceCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
    ) -> Result<
        Vec<crate::server::language_intelligence::LanguageIntelligenceProviderMeta>,
        crate::server::language_intelligence::LanguageIntelligenceProviderRegistryError,
    > {
        let mut registered = Vec::new();
        for registration in &evaluation.js_language_intelligence_providers {
            let mut meta = registration.meta.clone();
            meta.generation = generation_id;
            match coordinator.register_package_for_generation(
                &registration.package,
                meta.clone(),
                JsLanguageIntelligenceProvider {
                    runtime: self.clone(),
                    registration: registration.clone(),
                },
            ) {
                Ok(()) => registered.push(meta),
                Err(
                    crate::server::language_intelligence::LanguageIntelligenceProviderRegistryError::ProviderAlreadyRegistered {
                        ..
                    },
                ) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(registered)
    }

    pub(crate) fn register_native_syntax_handler(
        &self,
        coordinator: &crate::server::parse_coordinator::ParseCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
        path: &str,
        package_prefix: &str,
        _mode_id: &str,
    ) -> Result<
        Option<(
            crate::server::parse_coordinator::ParseHandlerMeta,
            crate::protocol::ParsePolicy,
        )>,
        crate::server::parse_coordinator::ParseCoordinatorError,
    > {
        let Some(contribution) = crate::server::syntax::select_grammar_for_path(
            &evaluation.syntax_grammars,
            &evaluation.syntax_engine_preferences,
            path,
        ) else {
            return Ok(None);
        };
        if contribution.engine_tier != crate::server::syntax::SyntaxEngineTier::Native {
            return Ok(None);
        }
        let policy = crate::protocol::ParsePolicy::new(
            contribution
                .max_window_bytes
                .unwrap_or(crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
                as u64,
            4 * 1024,
            30 * 1024 * 1024,
            contribution.timeout_ms.unwrap_or(5_000),
        );
        let key = (
            generation_id,
            package_prefix.to_string(),
            contribution.id.clone(),
        );
        let mut native_syntax_handlers = self
            .native_syntax_handlers
            .lock()
            .expect("native syntax handler set lock poisoned");
        if native_syntax_handlers.contains(&key) {
            return Ok(Some((
                crate::server::parse_coordinator::ParseHandlerMeta {
                    package_prefix: package_prefix.to_string(),
                    mode_id: contribution.id.clone(),
                },
                policy,
            )));
        }
        let handler = crate::server::syntax::native_handler(contribution).map_err(|error| {
            crate::server::parse_coordinator::ParseCoordinatorError::HandlerFailed(
                error.to_string(),
            )
        })?;
        let Some(handler) = handler else {
            return Ok(None);
        };
        match coordinator.replace_handler_meta_for_generation(
            generation_id,
            crate::server::parse_coordinator::ParseHandlerMeta {
                package_prefix: package_prefix.to_string(),
                mode_id: contribution.id.clone(),
            },
            handler,
        ) {
            Ok(meta) => {
                native_syntax_handlers.insert(key);
                Ok(Some((meta, policy)))
            }
            Err(
                crate::server::parse_coordinator::ParseCoordinatorError::HandlerAlreadyRegistered {
                    ..
                },
            ) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn registered_native_syntax_handler(
        &self,
        generation_id: u64,
        path: &str,
    ) -> Option<(
        crate::server::parse_coordinator::ParseHandlerMeta,
        crate::protocol::ParsePolicy,
    )> {
        let registry = crate::server::syntax::SyntaxGrammarRegistry::with_first_party_native();
        let contribution = registry.find_candidate_for_path(path)?.1;
        let key = (
            generation_id,
            contribution.package_prefix.clone(),
            contribution.id.clone(),
        );
        if !self
            .native_syntax_handlers
            .lock()
            .expect("native syntax handler set lock poisoned")
            .contains(&key)
        {
            return None;
        }
        Some((
            crate::server::parse_coordinator::ParseHandlerMeta {
                package_prefix: contribution.package_prefix.clone(),
                mode_id: contribution.id.clone(),
            },
            crate::protocol::ParsePolicy::new(
                contribution
                    .max_window_bytes
                    .unwrap_or(crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
                    as u64,
                4 * 1024,
                30 * 1024 * 1024,
                contribution.timeout_ms.unwrap_or(5_000),
            ),
        ))
    }

    /// Dispatch one provider command to the runtime domain that owns the
    /// registration's package (Plan 061 task 7): third-party callbacks run in
    /// the third-party worker, trusted callbacks in the trusted worker, so a
    /// slow or hostile sibling can never block the trusted runtime. On a
    /// poisoned worker the domain worker is replaced once and the command is
    /// retried; timeout/heap results poison the owning domain only.
    async fn dispatch_to_domain<T>(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
        command: RuntimeCommand,
        receiver: oneshot::Receiver<Result<T, ClayRuntimeError>>,
    ) -> Result<T, ClayRuntimeError> {
        let domain_runtime = self.domain(domain);
        if domain_runtime.poisoned.swap(false, Ordering::Relaxed) {
            self.replace_domain_worker(domain);
            self.replay_third_party_domain(domain).await?;
        }
        if let Err(error) = self.domain_worker(domain).sender.send(command) {
            self.replace_domain_worker(domain);
            self.replay_third_party_domain(domain).await?;
            self.domain_worker(domain)
                .sender
                .send(error.0)
                .map_err(|_| {
                    ClayRuntimeError::Runtime(
                        "persistent JavaScript runtime worker stopped".to_string(),
                    )
                })?;
        }
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("persistent JavaScript runtime worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            domain_runtime.poisoned.store(true, Ordering::Relaxed);
        } else if result.is_ok() {
            domain_runtime.evaluations.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// Replay every currently enabled, approved third-party package's load
    /// entry into a freshly replaced third-party worker (Plan 061 task 12).
    /// Only the current host-approved graph replays; deterministic
    /// registration tokens (`apiPrefix:id:index`) restore the host-side
    /// coordinator registrations in the fresh runtime. One bounded pass: a
    /// failing replay evaluation poisons the domain again instead of
    /// looping. No-op for the trusted domain (its replay is configuration
    /// reload).
    async fn replay_third_party_domain(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
    ) -> Result<(), ClayRuntimeError> {
        if domain != crate::packages::bundled::RuntimeDomain::ThirdParty {
            return Ok(());
        }
        let mut packages: Vec<(crate::server::ops::PackageContext, String)> = {
            let service = self
                .package_service
                .lock()
                .expect("package service mutex poisoned");
            let mut enabled: Vec<_> = service
                .enabled_records()
                .filter(|record| {
                    record.runtime_domain == crate::packages::bundled::RuntimeDomain::ThirdParty
                })
                .map(|record| {
                    (
                        crate::server::ops::PackageContext::from_record(record),
                        record.manifest.name.clone(),
                    )
                })
                .collect();
            enabled.sort_by(|a, b| a.1.cmp(&b.1));
            enabled
                .into_iter()
                .filter_map(|(context, name)| {
                    self.load_entry_allowlist
                        .specifier_for_package(&name)
                        .map(|specifier| (context, specifier))
                })
                .collect()
        };
        packages.dedup_by(|a, b| a.0.package_name == b.0.package_name);
        for (context, specifier) in packages {
            let source = format!(
                "const module = await import({specifier:?});\nif (typeof module.default === 'function') {{ await module.default(); }}"
            );
            let (response, receiver) = oneshot::channel();
            self.domain_worker(domain)
                .sender
                .send(RuntimeCommand::Evaluate {
                    entry: RuntimeEntry::ControlledSource(source),
                    workspace: None,
                    runtime_document_id: 1,
                    package_context: Some(context),
                    metric: "runtime.replay_third_party_domain",
                    response,
                })
                .map_err(|_| {
                    ClayRuntimeError::Runtime(
                        "persistent JavaScript runtime worker stopped".to_string(),
                    )
                })?;
            let result = receiver.await.map_err(|_| {
                ClayRuntimeError::Runtime(
                    "persistent JavaScript runtime worker stopped".to_string(),
                )
            })?;
            if let Err(error) = result {
                if matches!(
                    error,
                    ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit
                ) {
                    self.domain(domain).poisoned.store(true, Ordering::Relaxed);
                }
                return Err(error);
            }
        }
        Ok(())
    }

    /// Resolve the runtime domain owning a provider registration from the
    /// host-enabled package record (never from caller-supplied identity).
    fn registration_domain(
        &self,
        package: &crate::packages::record::PackageRecord,
    ) -> crate::packages::bundled::RuntimeDomain {
        self.enabled_record_domain(&package.manifest.name, &package.manifest.version)
    }

    async fn invoke_parse_handler(
        &self,
        registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
        notification: ParseEditNotification,
    ) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
        let domain = self.registration_domain(&registration.package);
        let (response, receiver) = oneshot::channel();
        self.dispatch_to_domain(
            domain,
            RuntimeCommand::Parse {
                registration,
                notification,
                response,
            },
            receiver,
        )
        .await
    }

    async fn invoke_completion_provider(
        &self,
        registration: crate::server::completion::JsCompletionProviderRegistration,
        request: crate::protocol::CompletionRequest,
        window: crate::server::completion::CompletionDocumentWindow,
    ) -> Result<crate::protocol::CompletionResultSet, ClayRuntimeError> {
        let domain = self.registration_domain(&registration.package);
        let (response, receiver) = oneshot::channel();
        self.dispatch_to_domain(
            domain,
            RuntimeCommand::Completion {
                registration,
                request,
                window,
                response,
            },
            receiver,
        )
        .await
    }

    pub(crate) fn document_analysis_registration_authorized(
        &self,
        registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
    ) -> bool {
        use crate::packages::{
            authorization::language_server_descriptor_fingerprint, permissions::PackagePermission,
        };

        if !registration
            .package
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::ParseDocument)
            || !registration
                .package
                .manifest
                .clay
                .permissions
                .contains(&PackagePermission::LanguageServer)
        {
            return false;
        }
        let Some(descriptor) = registration
            .package
            .contributions
            .language_servers
            .iter()
            .find(|descriptor| descriptor.id == registration.contribution)
        else {
            return false;
        };
        let service = self
            .package_service
            .lock()
            .expect("package service mutex poisoned");
        service.enabled_records().any(|record| {
            record.manifest.name == registration.package.manifest.name
                && record.manifest.version == registration.package.manifest.version
                && record.manifest.clay.api_prefix == registration.package.manifest.clay.api_prefix
        }) && service
            .language_server_grant(
                &registration.package.manifest.name,
                &registration.contribution,
            )
            .is_some_and(|grant| {
                grant.descriptor_fingerprint == language_server_descriptor_fingerprint(descriptor)
            })
    }

    pub(crate) fn document_analysis_authorized(
        &self,
        registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
        workspace_root_id: crate::protocol::WorkspaceRootId,
    ) -> bool {
        self.document_analysis_registration_authorized(registration)
            && self
                .package_service
                .lock()
                .expect("package service mutex poisoned")
                .language_server_grant(
                    &registration.package.manifest.name,
                    &registration.contribution,
                )
                .is_some_and(|grant| grant.workspace_root_ids.contains(&workspace_root_id))
    }

    pub(crate) async fn invoke_document_analyzer(
        &self,
        registration: crate::server::document_analysis::JsDocumentAnalyzerRegistration,
        event: crate::server::document_analysis::DocumentAnalysisEvent,
    ) -> Result<DocumentAnalysisInvocation, ClayRuntimeError> {
        // Route analysis through the owning domain runtime (Plan 061): no
        // additional persistent runtimes are created per analyzer/document.
        let domain = self.enabled_record_domain(
            &registration.package.manifest.name,
            &registration.package.manifest.version,
        );
        let domain_runtime = self.domain(domain);
        if domain_runtime.poisoned.swap(false, Ordering::Relaxed) {
            self.replace_domain_worker(domain);
        }
        let (response, receiver) = oneshot::channel();
        let invocation_id = self.evaluations.fetch_add(1, Ordering::Relaxed);
        let command = RuntimeCommand::DocumentAnalysis {
            registration,
            event,
            invocation_id,
            response,
        };
        if let Err(error) = self.domain_worker(domain).sender.send(command) {
            self.replace_domain_worker(domain);
            self.domain_worker(domain)
                .sender
                .send(error.0)
                .map_err(|_| {
                    ClayRuntimeError::Runtime("document analysis worker stopped".to_string())
                })?;
        }
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("document analysis worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            domain_runtime.poisoned.store(true, Ordering::Relaxed);
        } else if result.is_ok() {
            domain_runtime.evaluations.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    async fn invoke_language_intelligence_provider(
        &self,
        registration: crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration,
        request: crate::protocol::LanguageIntelligenceRequest,
        window: crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
    ) -> Result<crate::protocol::LanguageIntelligenceResult, ClayRuntimeError> {
        let domain = self.registration_domain(&registration.package);
        let (response, receiver) = oneshot::channel();
        self.dispatch_to_domain(
            domain,
            RuntimeCommand::LanguageIntelligence {
                registration,
                request,
                window,
                response,
            },
            receiver,
        )
        .await
    }

    /// Trusted-domain worker.
    fn worker(&self) -> Arc<RuntimeWorker> {
        self.domain_worker(crate::packages::bundled::RuntimeDomain::Trusted)
    }

    fn domain_worker(&self, domain: crate::packages::bundled::RuntimeDomain) -> Arc<RuntimeWorker> {
        Arc::clone(
            &self
                .domain(domain)
                .worker
                .lock()
                .expect("Clay runtime service worker mutex poisoned"),
        )
    }

    /// Revoke previous-generation executable process authority after commit.
    /// Coordinator registrations are cancelled separately; this tears down any
    /// language-server children still owned by either domain of this service.
    pub(crate) async fn shutdown_generation_resources(&self) -> usize {
        let trusted = self
            .worker()
            .op_state
            .shutdown_language_server_sessions()
            .await;
        let third_party = self
            .domain_worker(crate::packages::bundled::RuntimeDomain::ThirdParty)
            .op_state
            .shutdown_language_server_sessions()
            .await;
        trusted + third_party
    }

    /// Snapshot of the third-party worker's current registration payload
    /// (Plan 061 task 12): the worker survives trusted reloads, so at
    /// generation commit the server re-registers these under the new
    /// generation instead of canceling live third-party providers.
    pub(crate) fn third_party_registrations_snapshot(&self) -> ClayRuntimeEvaluation {
        harvest_op_state_evaluation(
            &self
                .domain_worker(crate::packages::bundled::RuntimeDomain::ThirdParty)
                .op_state,
        )
    }

    /// Trusted-domain-only generation shutdown (Plan 061 task 12): reload
    /// shares the third-party worker across generations, so only the old
    /// trusted worker's language-server sessions end at commit.
    pub(crate) async fn shutdown_trusted_generation_resources(&self) -> usize {
        self.worker()
            .op_state
            .shutdown_language_server_sessions()
            .await
    }

    #[cfg(test)]
    pub(crate) async fn language_server_session_count(&self) -> usize {
        let trusted = self.worker().op_state.language_server_session_count().await;
        let third_party = self
            .domain_worker(crate::packages::bundled::RuntimeDomain::ThirdParty)
            .op_state
            .language_server_session_count()
            .await;
        trusted + third_party
    }

    /// Test-only handle to the trusted generation's language-server process service.
    #[cfg(test)]
    pub(crate) fn language_server_process_for_test(
        &self,
    ) -> crate::server::language_server::LanguageServerProcessService {
        self.worker().op_state.language_server_process()
    }

    #[cfg(test)]
    pub(crate) fn evaluation_count(&self) -> u64 {
        self.evaluations.load(Ordering::Relaxed)
    }

    /// Total persistent application runtimes started by this service
    /// (initial two plus any poison replacements). Trust-domain tests assert
    /// package/document/analyzer activity never raises it above two.
    #[cfg(test)]
    pub(crate) fn workers_started(&self) -> u64 {
        self.workers_started.load(Ordering::Relaxed)
    }

    /// Per-domain successful evaluation count (trust-domain dispatch tests).
    #[cfg(test)]
    pub(crate) fn domain_evaluations(
        &self,
        domain: crate::packages::bundled::RuntimeDomain,
    ) -> u64 {
        self.domain(domain).evaluations.load(Ordering::Relaxed)
    }

    /// Per-domain runtime generation (bumped on every worker replacement).
    #[cfg(test)]
    pub(crate) fn domain_generation(&self, domain: crate::packages::bundled::RuntimeDomain) -> u64 {
        self.domain(domain).generation.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn test_op_state(&self) -> Arc<ClayOpState> {
        Arc::clone(&self.worker().op_state)
    }
}

struct JsParseHandler {
    runtime: ClayJsRuntimeService,
    registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
}

impl crate::server::parse_coordinator::ParseHandler for JsParseHandler {
    fn parse(
        &self,
        notification: ParseEditNotification,
    ) -> crate::server::parse_coordinator::ParseHandlerFuture {
        let runtime = self.runtime.clone();
        let registration = self.registration.clone();
        Box::pin(async move {
            runtime
                .invoke_parse_handler(registration, notification)
                .await
                .map_err(|error| {
                    crate::server::parse_coordinator::ParseCoordinatorError::HandlerFailed(
                        error.to_string(),
                    )
                })
        })
    }
}

struct JsCompletionProvider {
    runtime: ClayJsRuntimeService,
    registration: crate::server::completion::JsCompletionProviderRegistration,
}

impl crate::server::completion::CompletionProvider for JsCompletionProvider {
    fn complete(
        &self,
        request: crate::protocol::CompletionRequest,
        window: crate::server::completion::CompletionDocumentWindow,
    ) -> crate::server::completion::CompletionProviderFuture {
        let runtime = self.runtime.clone();
        let registration = self.registration.clone();
        Box::pin(async move {
            runtime
                .invoke_completion_provider(registration, request, window)
                .await
                .map_err(|error| {
                    crate::server::completion::CompletionProviderError::ProviderFailed(
                        error.to_string(),
                    )
                })
        })
    }
}

struct JsLanguageIntelligenceProvider {
    runtime: ClayJsRuntimeService,
    registration: crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration,
}

impl crate::server::language_intelligence::LanguageIntelligenceProvider
    for JsLanguageIntelligenceProvider
{
    fn provide(
        &self,
        request: crate::protocol::LanguageIntelligenceRequest,
        window: crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
    ) -> crate::server::language_intelligence::LanguageIntelligenceProviderFuture {
        let runtime = self.runtime.clone();
        let registration = self.registration.clone();
        Box::pin(async move {
            runtime
                .invoke_language_intelligence_provider(registration, request, window)
                .await
                .map_err(|error| {
                    crate::server::language_intelligence::LanguageIntelligenceProviderError::ProviderFailed(
                        error.to_string(),
                    )
                })
        })
    }
}

/// Result of one JavaScript evaluation returned across the Rust boundary.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ClayRuntimeEvaluation {
    pub(crate) op_records: Vec<String>,
    pub(crate) published_sdui_tree: Option<crate::protocol::SduiTree>,
    pub(crate) published_decoration_set: Option<crate::protocol::DecorationSet>,
    pub(crate) published_diagnostic_set: Option<crate::protocol::DiagnosticSet>,
    pub(crate) parse_handlers: Vec<crate::server::parse_coordinator::ParseHandlerMeta>,
    pub(crate) js_parse_handlers: Vec<crate::server::parse_coordinator::JsParseHandlerRegistration>,
    pub(crate) behavior_manifest: Option<crate::protocol::BehaviorManifest>,
    pub(crate) ui_contributions: crate::server::ui::PackageUiRegistrySnapshot,
    pub(crate) syntax_grammars: Vec<crate::server::syntax::SyntaxGrammarContribution>,
    pub(crate) syntax_engine_preferences:
        std::collections::BTreeMap<String, crate::server::syntax::SyntaxEngineTier>,
    pub(crate) completion_providers: Vec<crate::server::completion::CompletionProviderMeta>,
    pub(crate) js_completion_providers:
        Vec<crate::server::completion::JsCompletionProviderRegistration>,
    pub(crate) language_intelligence_providers:
        Vec<crate::server::language_intelligence::LanguageIntelligenceProviderMeta>,
    pub(crate) js_language_intelligence_providers:
        Vec<crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration>,
    pub(crate) document_analyzers:
        Vec<crate::server::document_analysis::JsDocumentAnalyzerRegistration>,
    /// Resolved active theme snapshot from `setTheme` (`clay:theme` facade). `None`
    /// when `init.js` did not select a theme (Clay default applies). Applied to
    /// the shared server slot at load/reload so the welcome handshake ships it.
    pub(crate) active_theme: Option<crate::protocol::ActiveTheme>,
    /// Complete typography candidate from `setTypography`, if this evaluation
    /// configured one. The server assigns its authoritative revision only after
    /// the evaluation succeeds.
    pub(crate) active_typography: Option<crate::protocol::ActiveTypography>,
    /// Warnings emitted by optional configuration-module imports. These are
    /// drained from the configuration runtime before the evaluation is
    /// returned so reload callers can retain and report them.
    pub(crate) configuration_diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Debug)]
pub(crate) struct DocumentAnalysisInvocation {
    pub(crate) decorations: Option<crate::protocol::DecorationSet>,
    pub(crate) diagnostics: Option<crate::protocol::DiagnosticSet>,
    pub(crate) response: crate::server::document_analysis::DocumentAnalysisResponse,
}

#[derive(Debug)]
pub(crate) enum ClayRuntimeError {
    Configuration(ConfigurationError),
    InvalidMainSpecifier(String),
    Runtime(String),
    Timeout,
    HeapLimit,
    Join(task::JoinError),
}

impl fmt::Display for ClayRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(error) => write!(formatter, "configuration error: {error}"),
            Self::InvalidMainSpecifier(message) => {
                write!(formatter, "invalid main module: {message}")
            }
            Self::Runtime(message) => write!(formatter, "JavaScript runtime error: {message}"),
            Self::Timeout => write!(
                formatter,
                "JavaScript runtime evaluation exceeded the configured timeout"
            ),
            Self::HeapLimit => write!(formatter, "JavaScript runtime exceeded the heap limit"),
            Self::Join(error) => write!(formatter, "JavaScript runtime task failed: {error}"),
        }
    }
}

impl Error for ClayRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::Join(error) => Some(error),
            Self::InvalidMainSpecifier(_) | Self::Runtime(_) | Self::Timeout | Self::HeapLimit => {
                None
            }
        }
    }
}

impl ClayRuntimeError {
    pub(crate) fn diagnostic(&self) -> RuntimeDiagnostic {
        match self {
            Self::Configuration(error) => RuntimeDiagnostic::error(
                "configuration.invalid_module",
                configuration_diagnostic_message(&error.to_string()),
            ),
            Self::InvalidMainSpecifier(_) => RuntimeDiagnostic::error(
                "runtime.invalid_main",
                "Runtime configuration entry point could not be parsed.",
            ),
            Self::Runtime(message) => runtime_error_diagnostic(message),
            Self::Timeout => RuntimeDiagnostic::error(
                "runtime.timeout",
                "JavaScript runtime evaluation timed out and was terminated.",
            ),
            Self::HeapLimit => RuntimeDiagnostic::error(
                "runtime.heap_limit",
                "JavaScript runtime exceeded its heap budget and was terminated.",
            ),
            Self::Join(_) => RuntimeDiagnostic::error(
                "runtime.task_failed",
                "JavaScript runtime worker failed before configuration completed.",
            ),
        }
    }
}

fn runtime_error_diagnostic(message: &str) -> RuntimeDiagnostic {
    let code = extract_clay_error_code(message).unwrap_or_else(|| {
        if message.contains("SyntaxError") {
            "runtime.syntax_error".to_string()
        } else {
            "runtime.exception".to_string()
        }
    });
    let detail = match code.as_str() {
        "runtime.invalid_import" => {
            "Only clay:* facades and relative local configuration modules are allowed."
        }
        "configuration.invalid_module" => {
            // Secure but actionable: name the allowed import families (clay:*
            // facades + relative local .js) so a typo (e.g. `clay:themes` vs
            // `clay:theme`) is diagnosable without echoing the rejected
            // specifier/URL/path (which must not leak).
            "Configuration import rejected: only clay:* facades (clay:theme, clay:configuration, clay:keybindings, clay:packages, clay:ui, clay:commands, ...) and explicit relative .js files under the configuration root are allowed. Check the import specifier spelling."
        }
        "runtime.syntax_error" => {
            "JavaScript syntax error while evaluating server-side configuration."
        }
        "runtime.invalid_record" => "Runtime op validation rejected an empty record.",
        "sdui.invalid_tree" => "Published SDUI tree failed server validation.",
        "sdui.invalid_action" => "Published SDUI action contains unsupported command authority.",
        "keybindings.unknown_command" => {
            "Key binding references an unknown or unsupported command."
        }
        code if code.starts_with("ui.") => {
            "Package UI contribution registration failed server validation."
        }
        code if code.starts_with("documents.") => {
            "Document/workspace operation failed server validation."
        }
        code if code.starts_with("workspace.") => "Workspace operation failed server validation.",
        _ => "JavaScript runtime evaluation failed.",
    };

    RuntimeDiagnostic::error(code, detail)
}

fn configuration_diagnostic_message(message: &str) -> String {
    // Secure but actionable: do not echo the rejected specifier/URL/path
    // (which must not leak). Name the allowed import families so a config
    // typo is diagnosable. `message` is only inspected to distinguish the
    // entry-point case; its contents are never surfaced.
    if message.contains("init.js") {
        "Configuration entry point init.js could not be loaded.".to_string()
    } else {
        "Configuration import rejected: only clay:* facades (clay:theme, clay:configuration, clay:keybindings, clay:packages, clay:ui, clay:commands, ...) and explicit relative .js files under the configuration root are allowed. Check the import specifier spelling.".to_string()
    }
}

/// Extract the human-readable detail from a `configuration.invalid_module: <detail>`
/// JS error string so runtime-routed configuration errors name the rejected
/// module/path instead of an opaque generic message.
fn configuration_runtime_detail(message: &str) -> Option<&str> {
    let prefix = "configuration.invalid_module:";
    message
        .strip_prefix(prefix)
        .map(str::trim)
        .filter(|d| !d.is_empty())
}

fn extract_clay_error_code(message: &str) -> Option<String> {
    message
        .split(|character: char| character.is_whitespace() || character == ':' || character == '`')
        .find(|part| {
            part.contains('.')
                && part.chars().all(is_error_code_character)
                && part.split('.').next().is_some_and(|domain| {
                    crate::packages::manifest::RESERVED_CORE_API_DOMAINS.contains(&domain)
                })
        })
        .map(ToOwned::to_owned)
}

fn is_error_code_character(character: char) -> bool {
    character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || character == '.'
        || character == '_'
}

pub(crate) enum RuntimeEntry {
    ControlledSource(String),
    ConfigurationRoot(PathBuf),
}

struct RuntimeWorker {
    sender: mpsc::Sender<RuntimeCommand>,
    op_state: Arc<ClayOpState>,
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

fn start_runtime_worker(
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

fn start_runtime_worker_with_state(
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

fn run_runtime_worker(
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
                invocation_id,
                response,
            } => {
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

fn create_js_runtime(
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

fn prepare_runtime_entry(
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
fn harvest_op_state_evaluation(op_state: &Arc<ClayOpState>) -> ClayRuntimeEvaluation {
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

async fn evaluate_loaded_module(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loaded_configuration: LoadedRuntimeEntry,
    timeout: Duration,
    use_main_module: bool,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    if let Some(configuration) = &loaded_configuration.configuration {
        runtime
            .op_state()
            .borrow_mut()
            .put(Arc::clone(configuration));
    }
    let terminate_handle = runtime.v8_isolate().thread_safe_handle();
    let timer = TerminationTimer::start(timeout, terminate_handle);

    let evaluation_result: Result<ClayRuntimeEvaluation, ClayRuntimeError> = async {
        let module_id = if use_main_module {
            if let Some(source) = loaded_configuration.main_source {
                runtime
                    .load_main_es_module_from_code(&loaded_configuration.main_specifier, source)
                    .await
            } else {
                runtime
                    .load_main_es_module(&loaded_configuration.main_specifier)
                    .await
            }
        } else if let Some(source) = loaded_configuration.main_source {
            runtime
                .load_side_es_module_from_code(&loaded_configuration.main_specifier, source)
                .await
        } else {
            runtime
                .load_side_es_module(&loaded_configuration.main_specifier)
                .await
        }
        .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
        let result = runtime.mod_evaluate(module_id);
        runtime
            .run_event_loop(Default::default())
            .await
            .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
        result
            .await
            .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
        // Phase 20.6: apply persisted user preferences (`preferences.json`,
        // source `ui-session`) AFTER init.js so the documented precedence
        // holds — package/canonical defaults < init.js < UI session. Validation
        // reuses the same Rust functions as the `setTheme`/`setAppearance`/
        // `setTypography` ops so no bounds/authority logic is duplicated. A
        // corrupted preference field is dropped with a diagnostic record so a
        // bad file never breaks startup.
        if let Some(configuration) = &loaded_configuration.configuration {
            apply_persisted_preferences(op_state, configuration);
        }
        let configuration_diagnostics = loaded_configuration
            .configuration
            .as_ref()
            .map(|configuration| {
                configuration
                    .take_module_errors()
                    .into_iter()
                    .map(|error| {
                        RuntimeDiagnostic::warning(
                            "configuration.module_failed",
                            format!(
                                "Optional configuration module {} failed: {}",
                                error.path, error.message
                            ),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut evaluation = harvest_op_state_evaluation(op_state);
        evaluation.configuration_diagnostics = configuration_diagnostics;
        Ok(evaluation)
    }
    .await;

    if timer.did_fire() {
        let _ = runtime.v8_isolate().cancel_terminate_execution();
        return Err(ClayRuntimeError::Timeout);
    }
    if heap_limit_hit.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = runtime.v8_isolate().cancel_terminate_execution();
        return Err(ClayRuntimeError::HeapLimit);
    }
    evaluation_result
}

/// Apply persisted UI-session preferences to `op_state` after init.js has run.
/// Theme is applied first (marks an explicit theme active), then appearance
/// (no canonical re-resolve over the explicit theme), then typography. Absent
/// fields leave init.js / canonical defaults in place. Failures (corrupted
/// field, unresolvable theme package) are recorded as diagnostics and skipped.
fn apply_persisted_preferences(op_state: &Arc<ClayOpState>, configuration: &ConfigurationRuntime) {
    let prefs = configuration.load_preferences();
    for diagnostic in &prefs.diagnostics {
        op_state.record(format!("preferences: {diagnostic}"));
    }
    if let Some(specifier) = &prefs.theme
        && let Err(error) = crate::server::ops::theme::apply_theme(op_state, specifier)
    {
        op_state.record(format!(
            "preferences: theme `{specifier}` rejected: {error}"
        ));
    }
    if let Some(appearance) = prefs.appearance {
        crate::server::ops::theme::apply_appearance(op_state, appearance, true);
    }
    if let Some(typography_value) = &prefs.typography {
        match serde_json::to_string(typography_value) {
            Ok(json) => {
                if let Err(error) =
                    crate::server::ops::typography::apply_typography(op_state, &json)
                {
                    op_state.record(format!("preferences: typography rejected: {error}"));
                }
            }
            Err(error) => {
                op_state.record(format!(
                    "preferences: typography serialization failed: {error}"
                ));
            }
        }
    }
}

async fn evaluate_js_parse_handler(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    notification: ParseEditNotification,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
    let source = format!(
        r#"
const registry = globalThis.__clayParseHandlers ?? Object.create(null);
const handler = registry[{token:?}];
if (typeof handler !== "function") {{
  throw new Error("parse.handler_missing: registered parse handler is unavailable");
}}
const notification = {notification};
const update = await handler(notification);
Deno.core.ops.op_clay_parse_store_update(JSON.stringify(update ?? null));
"#,
        token = registration.token,
        notification = parse_notification_json(&notification),
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/parse-{}.js",
            registration.token.replace(':', "-")
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let update_json = op_state.take_parse_update_json().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: handler produced no update".to_string())
    })?;
    parse_update_json(&update_json, registration, notification)
}

#[expect(
    clippy::too_many_arguments,
    reason = "document analyzer invocation keeps runtime, registration, event, timeout, and heap containment explicit"
)]
async fn evaluate_js_document_analyzer(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
    event: crate::server::document_analysis::DocumentAnalysisEvent,
    invocation_id: u64,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<DocumentAnalysisInvocation, ClayRuntimeError> {
    let module_specifier = serde_json::to_string(&registration.module_specifier)
        .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
    let export_name = serde_json::to_string(&registration.export_name)
        .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
    let event_json = document_analysis_event_json(registration, &event);
    let source = format!(
        r#"
import * as analyzerModule from {module_specifier};
const handler = analyzerModule[{export_name}];
if (typeof handler !== "function") {{
  throw new Error("analysis.handler_missing: registered analyzer export is unavailable");
}}
const result = await handler({event_json});
Deno.core.ops.op_clay_runtime_record(JSON.stringify(result ?? null));
"#,
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/document-analysis-{}-{invocation_id}.js",
            registration.id.replace([':', '/'], "-")
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    let evaluation =
        evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let response_json = evaluation
        .op_records
        .last()
        .map(String::as_str)
        .unwrap_or("null");
    let response = match &event {
        crate::server::document_analysis::DocumentAnalysisEvent::Completion { request, .. } => {
            crate::server::document_analysis::DocumentAnalysisResponse::Completion(
                completion_result_from_json(response_json, &registration.package, request)
                    .map_err(|error| CompletionProviderError::ProviderFailed(error.to_string())),
            )
        }
        crate::server::document_analysis::DocumentAnalysisEvent::LanguageIntelligence {
            request,
            ..
        } => crate::server::document_analysis::DocumentAnalysisResponse::LanguageIntelligence(
            language_intelligence_result_from_json(
                response_json,
                &registration.package,
                request,
            )
            .map_err(|error| {
                crate::server::language_intelligence::LanguageIntelligenceProviderError::ProviderFailed(
                    error.to_string(),
                )
            }),
        ),
        _ => crate::server::document_analysis::DocumentAnalysisResponse::None,
    };
    Ok(DocumentAnalysisInvocation {
        decorations: evaluation.published_decoration_set,
        diagnostics: evaluation.published_diagnostic_set,
        response,
    })
}

fn document_analysis_event_json(
    registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
    event: &crate::server::document_analysis::DocumentAnalysisEvent,
) -> String {
    use crate::server::document_analysis::DocumentAnalysisEvent;
    let identity = serde_json::json!({
        "package": registration.package.manifest.name,
        "packageVersion": registration.package.manifest.version,
        "packagePrefix": registration.package.manifest.clay.api_prefix,
        "analyzerId": registration.id,
        "contribution": registration.contribution,
    });
    let value = match event {
        DocumentAnalysisEvent::Open {
            document_id,
            document_version,
            runtime_generation,
            active_mode,
            workspace_root_id,
            canonical_root_path,
            relative_path,
            text,
        } => serde_json::json!({
            "kind": "open",
            "identity": identity,
            "documentId": document_id,
            "documentVersion": document_version,
            "runtimeGeneration": runtime_generation,
            "activeMode": active_mode,
            "workspaceRootId": workspace_root_id,
            "canonicalRootPath": canonical_root_path,
            "relativePath": relative_path,
            "text": text,
        }),
        DocumentAnalysisEvent::Change {
            document_id,
            base_version,
            document_version,
            byte_start,
            byte_end,
            inserted_text,
        } => serde_json::json!({
            "kind": "change",
            "identity": identity,
            "documentId": document_id,
            "baseVersion": base_version,
            "documentVersion": document_version,
            "byteStart": byte_start,
            "byteEnd": byte_end,
            "insertedText": inserted_text,
        }),
        DocumentAnalysisEvent::Reset {
            document_id,
            document_version,
            text,
        } => serde_json::json!({
            "kind": "reset",
            "identity": identity,
            "documentId": document_id,
            "documentVersion": document_version,
            "text": text,
        }),
        DocumentAnalysisEvent::Close {
            document_id,
            document_version,
        } => serde_json::json!({
            "kind": "close",
            "identity": identity,
            "documentId": document_id,
            "documentVersion": document_version,
        }),
        DocumentAnalysisEvent::Completion { request, window } => serde_json::json!({
            "kind": "completion",
            "identity": identity,
            "request": serde_json::from_str::<serde_json::Value>(&completion_request_json(request)).unwrap_or(serde_json::Value::Null),
            "window": serde_json::from_str::<serde_json::Value>(&completion_window_json(window)).unwrap_or(serde_json::Value::Null),
        }),
        DocumentAnalysisEvent::LanguageIntelligence { request, window } => serde_json::json!({
            "kind": "languageIntelligence",
            "identity": identity,
            "request": serde_json::from_str::<serde_json::Value>(&language_intelligence_request_json(request)).unwrap_or(serde_json::Value::Null),
            "window": serde_json::from_str::<serde_json::Value>(&language_intelligence_window_json(window)).unwrap_or(serde_json::Value::Null),
        }),
        DocumentAnalysisEvent::Shutdown => serde_json::json!({
            "kind": "shutdown",
            "identity": identity,
        }),
    };
    value.to_string()
}

#[allow(
    clippy::too_many_arguments,
    reason = "completion JS bridge needs runtime, registration, request, window, timeout, and heap state together"
)]
async fn evaluate_js_completion_provider(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::completion::JsCompletionProviderRegistration,
    request: crate::protocol::CompletionRequest,
    window: crate::server::completion::CompletionDocumentWindow,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<crate::protocol::CompletionResultSet, ClayRuntimeError> {
    let source = format!(
        r#"
const registry = globalThis.__clayCompletionHandlers ?? Object.create(null);
const handler = registry[{token:?}];
if (typeof handler !== "function") {{
  throw new Error("completion.handler_missing: registered completion handler is unavailable");
}}
const result = await handler({request}, {window});
Deno.core.ops.op_clay_completion_store_result(JSON.stringify(result ?? null));
"#,
        token = registration.token,
        request = completion_request_json(&request),
        window = completion_window_json(&window),
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/completion-{}-{}.js",
            registration.token.replace(':', "-"),
            request.request_id,
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let result_json = op_state.take_completion_result_json().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "completion.invalid_result: handler produced no result".to_string(),
        )
    })?;
    completion_result_from_json(&result_json, &registration.package, &request)
}

fn completion_request_json(request: &crate::protocol::CompletionRequest) -> String {
    let trigger = match &request.trigger {
        crate::protocol::CompletionTrigger::Manual => serde_json::json!({ "kind": "manual" }),
        crate::protocol::CompletionTrigger::Character(character) => {
            serde_json::json!({ "kind": "character", "character": character })
        }
    };
    serde_json::json!({
        "requestId": request.request_id,
        "clientId": request.client_id,
        "documentId": request.document_id,
        "documentVersion": request.document_version,
        "behaviorVersion": request.behavior_version,
        "cursorByteOffset": request.cursor_byte_offset,
        "replacementRange": {
            "byteStart": request.replacement_range.byte_start,
            "byteEnd": request.replacement_range.byte_end,
        },
        "trigger": trigger,
        "providerGeneration": request.provider_generation,
    })
    .to_string()
}

fn completion_window_json(window: &crate::server::completion::CompletionDocumentWindow) -> String {
    serde_json::json!({
        "documentId": window.document_id,
        "documentVersion": window.document_version,
        "behaviorVersion": window.behavior_version,
        "packagePrefix": window.package_prefix,
        "byteStart": window.byte_start,
        "byteEnd": window.byte_end,
        "text": window.text,
    })
    .to_string()
}

fn completion_result_from_json(
    result_json: &str,
    package: &crate::packages::record::PackageRecord,
    request: &crate::protocol::CompletionRequest,
) -> Result<crate::protocol::CompletionResultSet, ClayRuntimeError> {
    use crate::protocol::{
        CompletionItem, CompletionItemTextFormat, CompletionProvenance, CompletionResultSet,
        CompletionStatus,
    };

    let value: serde_json::Value = serde_json::from_str(result_json).map_err(|error| {
        ClayRuntimeError::Runtime(format!("completion.invalid_result: {error}"))
    })?;
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("completion.invalid_result: result must be an object".to_string())
    })?;
    let status = match object
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ok")
    {
        "ok" | "Ok" => CompletionStatus::Ok,
        "empty" | "Empty" => CompletionStatus::Empty,
        "timeout" | "Timeout" => CompletionStatus::Timeout,
        "providerError" | "ProviderError" | "error" => CompletionStatus::ProviderError,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "completion.invalid_result: unsupported status `{other}`"
            )));
        }
    };
    let provenance = CompletionProvenance {
        package_name: package.manifest.name.clone(),
        package_version: package.manifest.version.clone(),
        package_prefix: package.manifest.clay.api_prefix.clone(),
    };
    let items = object
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let item = item.as_object().ok_or_else(|| {
                        ClayRuntimeError::Runtime(
                            "completion.invalid_result: item must be an object".to_string(),
                        )
                    })?;
                    let label = item
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            ClayRuntimeError::Runtime(
                                "completion.invalid_result: item label is required".to_string(),
                            )
                        })?
                        .to_string();
                    Ok(CompletionItem {
                        insert_text: item
                            .get("insertText")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(&label)
                            .to_string(),
                        label,
                        detail: item
                            .get("detail")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        commit_characters: item
                            .get("commitCharacters")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        text_format: match item
                            .get("textFormat")
                            .and_then(serde_json::Value::as_str)
                        {
                            Some("snippet" | "Snippet") => CompletionItemTextFormat::Snippet,
                            _ => CompletionItemTextFormat::PlainText,
                        },
                        provenance: provenance.clone(),
                    })
                })
                .collect::<Result<Vec<_>, ClayRuntimeError>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(CompletionResultSet {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        replacement_range: request.replacement_range,
        status: if items.is_empty() && status == CompletionStatus::Ok {
            CompletionStatus::Empty
        } else {
            status
        },
        items,
        provenance,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "language-intelligence JS bridge mirrors the parse-handler worker path and needs request+window inputs together"
)]
async fn evaluate_js_language_intelligence_provider(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::language_intelligence::JsLanguageIntelligenceProviderRegistration,
    request: crate::protocol::LanguageIntelligenceRequest,
    window: crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<crate::protocol::LanguageIntelligenceResult, ClayRuntimeError> {
    let source = format!(
        r#"
const registry = globalThis.__clayLanguageIntelligenceHandlers ?? Object.create(null);
const handler = registry[{token:?}];
if (typeof handler !== "function") {{
  throw new Error("language.handler_missing: registered language-intelligence handler is unavailable");
}}
const request = {request};
const window = {window};
const result = await handler(request, window);
Deno.core.ops.op_clay_language_store_intelligence_result(JSON.stringify(result ?? null));
"#,
        token = registration.token,
        request = language_intelligence_request_json(&request),
        window = language_intelligence_window_json(&window),
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/language-intelligence-{}-{}.js",
            registration.token.replace(':', "-"),
            request.request_id,
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let result_json = op_state
        .take_language_intelligence_result_json()
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: handler produced no result".to_string(),
            )
        })?;
    language_intelligence_result_from_json(&result_json, &registration.package, &request)
}

fn language_intelligence_request_json(
    request: &crate::protocol::LanguageIntelligenceRequest,
) -> String {
    serde_json::json!({
        "requestId": request.request_id,
        "clientId": request.client_id,
        "documentId": request.document_id,
        "documentVersion": request.document_version,
        "behaviorVersion": request.behavior_version,
        "cursorByteOffset": request.cursor_byte_offset,
        "feature": language_intelligence_feature_name(request.feature),
        "providerGeneration": request.provider_generation,
    })
    .to_string()
}

fn language_intelligence_window_json(
    window: &crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
) -> String {
    serde_json::json!({
        "documentId": window.document_id,
        "documentVersion": window.document_version,
        "behaviorVersion": window.behavior_version,
        "byteStart": window.byte_start,
        "byteEnd": window.byte_end,
        "text": window.text,
        "activeMode": window.active_mode,
    })
    .to_string()
}

fn language_intelligence_feature_name(
    feature: crate::protocol::LanguageIntelligenceFeature,
) -> &'static str {
    match feature {
        crate::protocol::LanguageIntelligenceFeature::Hover => "hover",
        crate::protocol::LanguageIntelligenceFeature::GoToDefinition => "definition",
        crate::protocol::LanguageIntelligenceFeature::CodeAction => "codeAction",
        crate::protocol::LanguageIntelligenceFeature::SignatureHelp => "signatureHelp",
    }
}

fn language_intelligence_result_from_json(
    result_json: &str,
    package: &crate::packages::record::PackageRecord,
    request: &crate::protocol::LanguageIntelligenceRequest,
) -> Result<crate::protocol::LanguageIntelligenceResult, ClayRuntimeError> {
    use crate::protocol::{
        CodeActionResult, GoToDefinitionResult, HoverResult, LanguageIntelligencePayload,
        LanguageIntelligenceResult, LanguageIntelligenceStatus, SignatureHelpResult,
    };

    let value: serde_json::Value = serde_json::from_str(result_json)
        .map_err(|error| ClayRuntimeError::Runtime(format!("language.invalid_result: {error}")))?;
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("language.invalid_result: result must be an object".to_string())
    })?;
    let status = match object
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ok")
    {
        "ok" | "Ok" => LanguageIntelligenceStatus::Ok,
        "empty" | "Empty" => LanguageIntelligenceStatus::Empty,
        "timeout" | "Timeout" => LanguageIntelligenceStatus::Timeout,
        "providerError" | "ProviderError" | "error" => LanguageIntelligenceStatus::ProviderError,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "language.invalid_result: unsupported status `{other}`"
            )));
        }
    };

    let payload = match request.feature {
        crate::protocol::LanguageIntelligenceFeature::Hover => {
            let hover = object
                .get("payload")
                .and_then(|value| value.get("hover"))
                .or_else(|| object.get("hover"))
                .unwrap_or(&value);
            let hover_object = hover.as_object().unwrap_or(object);
            LanguageIntelligencePayload::Hover(HoverResult {
                range: hover_object
                    .get("range")
                    .and_then(language_intelligence_range_from_value),
                markdown: hover_object
                    .get("markdown")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        }
        crate::protocol::LanguageIntelligenceFeature::GoToDefinition => {
            let definition = object
                .get("payload")
                .and_then(|value| {
                    value
                        .get("definition")
                        .or_else(|| value.get("goToDefinition"))
                })
                .or_else(|| {
                    object
                        .get("definition")
                        .or_else(|| object.get("goToDefinition"))
                })
                .unwrap_or(&value);
            let locations = definition
                .get("locations")
                .or_else(|| object.get("locations"))
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(language_intelligence_location_from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult { locations })
        }
        crate::protocol::LanguageIntelligenceFeature::CodeAction => {
            let actions_value = object
                .get("payload")
                .and_then(|value| value.get("codeAction").or_else(|| value.get("actions")))
                .or_else(|| object.get("codeAction").or_else(|| object.get("actions")))
                .unwrap_or(&value);
            let actions = actions_value
                .get("actions")
                .or(Some(actions_value))
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(|value| language_intelligence_code_action_from_value(value, request))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            LanguageIntelligencePayload::CodeAction(CodeActionResult { actions })
        }
        crate::protocol::LanguageIntelligenceFeature::SignatureHelp => {
            let help = object
                .get("payload")
                .and_then(|value| value.get("signatureHelp"))
                .or_else(|| object.get("signatureHelp"))
                .unwrap_or(&value);
            let help_object = help.as_object().unwrap_or(object);
            let signatures = help_object
                .get("signatures")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(language_intelligence_signature_from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            LanguageIntelligencePayload::SignatureHelp(SignatureHelpResult {
                signatures,
                active_signature: help_object
                    .get("activeSignature")
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as u16),
                active_parameter: help_object
                    .get("activeParameter")
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as u16),
            })
        }
    };

    Ok(LanguageIntelligenceResult {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        feature: request.feature,
        status,
        payload,
        provenance: crate::protocol::CompletionProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
    })
}

fn language_intelligence_range_from_value(
    value: &serde_json::Value,
) -> Option<crate::protocol::TextByteRange> {
    let object = value.as_object()?;
    let byte_start = object
        .get("byteStart")
        .and_then(serde_json::Value::as_u64)?;
    let byte_end = object.get("byteEnd").and_then(serde_json::Value::as_u64)?;
    Some(crate::protocol::TextByteRange::new(byte_start, byte_end))
}

fn language_intelligence_location_from_value(
    value: &serde_json::Value,
) -> Result<crate::protocol::TextLocation, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("language.invalid_result: location must be an object".to_string())
    })?;
    let range = object
        .get("range")
        .and_then(language_intelligence_range_from_value)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: location.range requires byteStart/byteEnd".to_string(),
            )
        })?;
    if let Some(document_id) = object.get("documentId").and_then(serde_json::Value::as_u64) {
        return Ok(crate::protocol::TextLocation::OpenDocument { document_id, range });
    }
    let workspace_root_id = object
        .get("workspaceRootId")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: location requires documentId or workspaceRootId"
                    .to_string(),
            )
        })?;
    let relative_path = object
        .get("relativePath")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: workspace location requires relativePath".to_string(),
            )
        })?
        .to_string();
    Ok(crate::protocol::TextLocation::WorkspaceFile {
        workspace_root_id,
        relative_path,
        range,
    })
}

fn language_intelligence_code_action_from_value(
    value: &serde_json::Value,
    request: &crate::protocol::LanguageIntelligenceRequest,
) -> Result<crate::protocol::CodeAction, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "language.invalid_result: code action must be an object".to_string(),
        )
    })?;
    let range = object
        .get("range")
        .and_then(language_intelligence_range_from_value)
        .unwrap_or_else(|| {
            crate::protocol::TextByteRange::new(
                request.cursor_byte_offset,
                request.cursor_byte_offset,
            )
        });
    let title = object
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let command_id = object
        .get("commandId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let edit = object.get("edit").and_then(|edit_value| {
        let edit_object = edit_value.as_object()?;
        let edits = edit_object
            .get("edits")
            .and_then(serde_json::Value::as_array)?
            .iter()
            .filter_map(|entry| {
                let entry_object = entry.as_object()?;
                Some(crate::protocol::RangeEdit {
                    range: entry_object
                        .get("range")
                        .and_then(language_intelligence_range_from_value)?,
                    replacement: entry_object
                        .get("replacement")
                        .and_then(serde_json::Value::as_str)?
                        .to_string(),
                })
            })
            .collect::<Vec<_>>();
        Some(crate::protocol::EditPreview {
            document_id: edit_object
                .get("documentId")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(request.document_id),
            document_version: edit_object
                .get("documentVersion")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(request.document_version),
            edits,
        })
    });
    Ok(crate::protocol::CodeAction {
        range,
        title,
        command_id,
        edit,
    })
}

fn language_intelligence_signature_from_value(
    value: &serde_json::Value,
) -> Result<crate::protocol::SignatureInformation, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "language.invalid_result: signature must be an object".to_string(),
        )
    })?;
    let parameters = object
        .get("parameters")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|parameter| {
                    let parameter_object = parameter.as_object()?;
                    Some(crate::protocol::ParameterInformation {
                        label: parameter_object
                            .get("label")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        documentation: parameter_object
                            .get("documentation")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(crate::protocol::SignatureInformation {
        label: object
            .get("label")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        documentation: object
            .get("documentation")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        parameters,
    })
}

fn parse_notification_json(notification: &ParseEditNotification) -> String {
    serde_json::json!({
        "documentId": notification.document_id,
        "documentVersion": notification.document_version,
        "behaviorVersion": notification.behavior_version,
        "packagePrefix": notification.package_prefix,
        "mode": notification.mode_id,
        "viewport": range_json(notification.viewport),
        "invalidatedRanges": notification.invalidated_ranges.iter().map(|range| range_json(*range)).collect::<Vec<_>>(),
        "acceptedEdit": notification.accepted_edit.map(|edit| serde_json::json!({
            "baseDocumentVersion": edit.base_document_version,
            "documentVersion": edit.document_version,
            "startByte": edit.start_byte,
            "oldEndByte": edit.old_end_byte,
            "newEndByte": edit.new_end_byte,
            "startPosition": { "row": edit.start_position.row, "column": edit.start_position.column },
            "oldEndPosition": { "row": edit.old_end_position.row, "column": edit.old_end_position.column },
            "newEndPosition": { "row": edit.new_end_position.row, "column": edit.new_end_position.column },
        })),
        "parseWindows": notification.parse_windows.iter().map(|window| serde_json::json!({
            "documentId": window.document_id,
            "documentVersion": window.document_version,
            "packagePrefix": window.package_prefix,
            "mode": window.mode_id,
            "windowId": window.window_id,
            "byteStart": window.byte_start,
            "byteEnd": window.byte_end,
            "baseLine": window.base_line,
            "baseColumn": window.base_column,
            "incrementalEdit": window.incremental_edit,
            "text": window.text,
        })).collect::<Vec<_>>(),
        "memoryBudget": notification.memory_budget.map(|budget| serde_json::json!({
            "budgetBytes": budget.budget_bytes,
            "retainedBytes": budget.retained_bytes,
        })),
    })
    .to_string()
}

fn range_json(range: ParseByteRange) -> serde_json::Value {
    serde_json::json!({ "byteStart": range.start, "byteEnd": range.end })
}

fn parse_update_json(
    update_json: &str,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    fallback: ParseEditNotification,
) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
    let value: serde_json::Value = serde_json::from_str(update_json)
        .map_err(|error| ClayRuntimeError::Runtime(format!("parse.invalid_update: {error}")))?;
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: update must be an object".to_string())
    })?;
    let viewport = object
        .get("viewport")
        .and_then(parse_range_value)
        .unwrap_or(fallback.viewport);
    let spans: Option<Vec<DecorationSpan>> = object
        .get("spans")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| span_from_value(value, registration))
                .collect()
        })
        .transpose()?;
    let diagnostics = object
        .get("diagnostics")
        .map(|value| diagnostic_set_from_value(value, registration, &fallback, viewport))
        .transpose()?;
    Ok(IncrementalParseUpdate {
        document_id: object
            .get("documentId")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.document_id),
        document_version: object
            .get("documentVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.document_version),
        behavior_version: object
            .get("behaviorVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.behavior_version),
        package_prefix: object
            .get("packagePrefix")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&registration.meta.package_prefix)
            .to_string(),
        mode_id: object
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&registration.meta.mode_id)
            .to_string(),
        parse_unit: registration.parse_unit,
        viewport,
        invalidated_ranges: fallback.invalidated_ranges,
        syntax_tree_delta: object
            .get("syntaxTreeDelta")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        decoration_updates: spans
            .map(|spans| DecorationSet {
                document_id: fallback.document_id,
                document_version: fallback.document_version,
                package_prefix: registration.meta.package_prefix.clone(),
                kind: spans
                    .first()
                    .map_or(DecorationKind::Syntax, |span| span.kind),
                viewport_byte_start: viewport.start,
                viewport_byte_end: viewport.end,
                spans,
            })
            .into_iter()
            .collect(),
        diagnostic_update: diagnostics,
    })
}

fn diagnostic_set_from_value(
    value: &serde_json::Value,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    fallback: &ParseEditNotification,
    viewport: ParseByteRange,
) -> Result<DiagnosticSet, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: diagnostics must be an object".to_string())
    })?;
    let source = required_string(object, "source", "diagnostics")?;
    let provenance = DecorationProvenance {
        package_name: registration.package.manifest.name.clone(),
        package_version: registration.package.manifest.version.clone(),
        package_prefix: registration.package.manifest.clay.api_prefix.clone(),
    };
    let spans = object
        .get("spans")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "parse.invalid_update: diagnostics.spans must be an array".to_string(),
            )
        })?
        .iter()
        .map(|value| diagnostic_span_from_value(value, source, &provenance))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DiagnosticSet {
        document_id: fallback.document_id,
        document_version: fallback.document_version,
        viewport_byte_start: viewport.start,
        viewport_byte_end: viewport.end,
        source: source.to_string(),
        provenance,
        spans,
    })
}

fn diagnostic_span_from_value(
    value: &serde_json::Value,
    source: &str,
    provenance: &DecorationProvenance,
) -> Result<DiagnosticSpan, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "parse.invalid_update: diagnostic span must be an object".to_string(),
        )
    })?;
    let severity = match required_string(object, "severity", "diagnostic span")? {
        "error" => DiagnosticSeverity::Error,
        "warning" => DiagnosticSeverity::Warning,
        "info" => DiagnosticSeverity::Info,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: unsupported diagnostic severity `{other}`"
            )));
        }
    };
    Ok(DiagnosticSpan {
        byte_start: required_u64(object, "byteStart", "diagnostic span")?,
        byte_end: required_u64(object, "byteEnd", "diagnostic span")?,
        severity,
        code: required_string(object, "code", "diagnostic span")?.to_string(),
        message: required_string(object, "message", "diagnostic span")?.to_string(),
        source: source.to_string(),
        provenance: provenance.clone(),
    })
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, ClayRuntimeError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: {context}.{field} must be a string"
            ))
        })
}

fn required_u64(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    context: &str,
) -> Result<u64, ClayRuntimeError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: {context}.{field} must be an unsigned integer"
            ))
        })
}

fn parse_range_value(value: &serde_json::Value) -> Option<ParseByteRange> {
    let object = value.as_object()?;
    Some(ParseByteRange::new(
        object
            .get("byteStart")
            .or_else(|| object.get("start"))?
            .as_u64()?,
        object
            .get("byteEnd")
            .or_else(|| object.get("end"))?
            .as_u64()?,
    ))
}

fn span_from_value(
    value: &serde_json::Value,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
) -> Result<DecorationSpan, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: span must be an object".to_string())
    })?;
    let kind = match object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("syntax")
    {
        "syntax" | "Syntax" => DecorationKind::Syntax,
        "semantic" | "Semantic" => DecorationKind::Semantic,
        "diagnostic" | "Diagnostic" => DecorationKind::Diagnostic,
        "search-match" | "searchMatch" | "SearchMatch" => DecorationKind::SearchMatch,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: unsupported decoration kind `{other}`"
            )));
        }
    };
    let style_token = object
        .get("styleToken")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("markup.plain");
    Ok(DecorationSpan::from_style_token(
        object
            .get("byteStart")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        object
            .get("byteEnd")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        kind,
        style_token,
        object
            .get("priority")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u16,
        DecorationProvenance {
            package_name: registration.package.manifest.name.clone(),
            package_version: registration.package.manifest.version.clone(),
            package_prefix: registration.package.manifest.clay.api_prefix.clone(),
        },
    ))
}

/// Watchdog that terminates a V8 isolate when an evaluation exceeds a budget.
///
/// Spawns a lightweight OS thread that sleeps in 10 ms ticks until either the
/// timeout elapses (then calls `terminate_execution`) or [`did_fire`] cancels
/// it. `did_fire` is called on the happy path after evaluation completes and
/// atomically reports whether the watchdog already fired.
///
/// ponytail: one thread per evaluation. Ceiling: evaluations are infrequent
/// (startup config load, per-document loadEntry) so a polling thread per
/// evaluation is cheap; if evaluation frequency rises, switch to a shared
/// timer wheel or `tokio::time::timeout` on a `LocalSet`-spawned task.
struct TerminationTimer {
    fired: Arc<std::sync::atomic::AtomicBool>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl TerminationTimer {
    fn start(timeout: Duration, handle: deno_core::v8::IsolateHandle) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};

        let fired = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let (fired_clone, cancel_clone) = (Arc::clone(&fired), Arc::clone(&cancel));
        let join = std::thread::Builder::new()
            .name("clay-js-runtime-timeout".to_string())
            .spawn(move || {
                let start = std::time::Instant::now();
                while start.elapsed() < timeout {
                    if cancel_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                // Timeout elapsed before cancellation: terminate the isolate so
                // the blocked evaluation returns control.
                fired_clone.store(true, Ordering::Relaxed);
                handle.terminate_execution();
            })
            .expect("failed to spawn JS runtime timeout watchdog thread");
        Self {
            fired,
            cancel,
            join: Some(join),
        }
    }

    /// Cancels the watchdog and returns whether it had already fired.
    fn did_fire(mut self) -> bool {
        use std::sync::atomic::Ordering;

        self.cancel.store(true, Ordering::Relaxed);
        let fired = self.fired.load(Ordering::Relaxed);
        // Detach rather than join: the thread observes `cancel` and exits within
        // a 10 ms tick. Joining is safe (terminate is non-blocking) but detaching
        // keeps the happy path off any thread-synchronization latency.
        self.join.take();
        fired
    }
}

struct LoadedRuntimeEntry {
    main_specifier: ModuleSpecifier,
    main_source: Option<String>,
    configuration: Option<Arc<ConfigurationRuntime>>,
}

#[derive(Debug)]
struct ClayModuleLoader {
    state: std::sync::Mutex<ClayModuleLoaderState>,
    // Shared validated package loadEntry gate. Populated by
    // `op_clay_packages_load_package_by_specifier`, checked in resolve/load.
    // Ceiling: one entry per loaded package module.
    package_load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
    // Trust domain this loader serves; gates facade and configuration-module
    // resolution.
    domain: crate::packages::bundled::RuntimeDomain,
}

#[derive(Debug)]
struct ClayModuleLoaderState {
    main_specifier: ModuleSpecifier,
    main_source: Option<String>,
    configuration: Option<Arc<ConfigurationRuntime>>,
}

impl ClayModuleLoader {
    fn new(
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
        package_load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
        domain: crate::packages::bundled::RuntimeDomain,
    ) -> Self {
        Self {
            state: std::sync::Mutex::new(ClayModuleLoaderState {
                main_specifier,
                main_source,
                configuration,
            }),
            package_load_entry_allowlist,
            domain,
        }
    }

    fn set_entry(
        &self,
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
    ) {
        *self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned") = ClayModuleLoaderState {
            main_specifier,
            main_source,
            configuration,
        };
    }

    fn denied(specifier: &str) -> JsErrorBox {
        JsErrorBox::generic(format!(
            "runtime.invalid_import: module specifier `{specifier}` is not allowed in the server runtime boundary"
        ))
    }
}

impl ModuleLoader for ClayModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        let state = self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned");
        if specifier == state.main_specifier.as_str() {
            return Ok(state.main_specifier.clone());
        }
        if super::facades::source(specifier).is_some() {
            if !super::facades::allowed(self.domain, specifier) {
                return Err(Self::denied(specifier));
            }
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        if specifier == "markdown-it" {
            return ModuleSpecifier::parse(MARKDOWN_IT_MODULE_SPECIFIER)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        // Validated package `loadEntry`: opaque `clay://packages/...`
        // specifiers recorded by `op_clay_packages_load_package_by_specifier`.
        // This branch sits BEFORE the config-root branch because
        // `reject_non_local_specifier` would otherwise deny `clay://` URLs; the
        // shared allowlist is the single gate, so only a package module the
        // resolver op validated and recorded ever resolves here. Every other
        // `clay://packages/...` URL falls through to config-root confinement
        // (which rejects non-local specifiers) and the deny fallback.
        if self
            .package_load_entry_allowlist
            .absolute_path(specifier)
            .is_some()
        {
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        // Transitive relative imports from a validated package loadEntry are
        // confined to the validated package root by the allowlist and recorded
        // on first resolution. This lets a loadEntry import its own sibling
        // modules (e.g. `./index.js`) without weakening the config-root
        // boundary for any non-package specifier. ponytail: ceiling is the
        // validated package root; `resolve_relative` denies anything escaping it.
        if (specifier.starts_with("./") || specifier.starts_with("../"))
            && let Some(new_specifier) = self
                .package_load_entry_allowlist
                .resolve_relative(referrer, specifier)
        {
            return ModuleSpecifier::parse(&new_specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        if self.domain == crate::packages::bundled::RuntimeDomain::Trusted
            && let Some(configuration) = &state.configuration
        {
            return configuration
                .resolve_module(specifier, referrer)
                .map_err(|error| error.to_js_error());
        }

        Err(Self::denied(&format!("{specifier} from {referrer}")))
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let state = self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned");
        if module_specifier == &state.main_specifier
            && let Some(source) = &state.main_source
        {
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(source.clone().into()),
                module_specifier,
                None,
            )));
        }

        if let Some(source) = super::facades::source(module_specifier.as_str()) {
            if !super::facades::allowed(self.domain, module_specifier.as_str()) {
                return ModuleLoadResponse::Sync(Err(Self::denied(module_specifier.as_str())));
            }
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(source.to_string().into()),
                module_specifier,
                None,
            )));
        }

        if module_specifier.as_str() == MARKDOWN_IT_MODULE_SPECIFIER {
            return ModuleLoadResponse::Sync(markdown_it_module_source().map(|source| {
                ModuleSource::new(
                    ModuleType::JavaScript,
                    ModuleSourceCode::String(source.into()),
                    module_specifier,
                    None,
                )
            }));
        }
        // Validated package `loadEntry`: read the on-disk source the resolver op
        // recorded for this exact opaque specifier. Single gate, same allowlist
        // as `resolve`; no path outside the validated package root is ever read.
        if let Some(absolute_path) = self
            .package_load_entry_allowlist
            .absolute_path(module_specifier.as_str())
        {
            return ModuleLoadResponse::Sync(
                std::fs::read_to_string(&absolute_path)
                    .map_err(|error| {
                        Self::denied(&format!(
                            "package loadEntry {module_specifier} could not be loaded ({error})"
                        ))
                    })
                    .map(|source| {
                        ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(source.into()),
                            module_specifier,
                            None,
                        )
                    }),
            );
        }
        if self.domain == crate::packages::bundled::RuntimeDomain::Trusted
            && let Some(configuration) = &state.configuration
        {
            return ModuleLoadResponse::Sync(
                configuration
                    .load_module_source(module_specifier)
                    .map(|source| {
                        ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(source.into()),
                            module_specifier,
                            None,
                        )
                    })
                    .map_err(|error| error.to_js_error()),
            );
        }

        ModuleLoadResponse::Sync(Err(Self::denied(module_specifier.as_str())))
    }
}

fn markdown_it_module_source() -> Result<String, ModuleLoaderError> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages")
        .join("markdown")
        .join("node_modules")
        .join("markdown-it")
        .join("dist")
        .join("markdown-it.js");
    let bundled = std::fs::read_to_string(&path).map_err(|error| {
        ClayModuleLoader::denied(&format!(
            "markdown-it bundle could not be loaded from {} ({error})",
            path.display()
        ))
    })?;
    Ok(format!(
        "{bundled}\nconst MarkdownIt = globalThis.markdownit;\nexport default MarkdownIt;\nexport {{ MarkdownIt }};\n"
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        path::PathBuf,
        rc::Rc,
        sync::Arc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use tokio::sync::Mutex;

    use deno_core::{
        ModuleLoadOptions, ModuleLoadResponse, ModuleLoader, ModuleSpecifier, ModuleType,
        RequestedModuleType, ResolutionKind,
    };

    use super::{
        CONTROLLED_MAIN_SPECIFIER, ClayJsRuntimeService, ClayModuleLoader, ClayRuntimeError,
        ClayRuntimeEvaluation, PackageLoadEntryAllowlist, RuntimeEntry, create_js_runtime,
        evaluate_loaded_module, prepare_runtime_entry, start_runtime_worker,
    };
    use crate::perf::budgets::{
        BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, JS_RUNTIME_EVALUATION_TIMEOUT_MS,
        JS_RUNTIME_HEAP_LIMIT_BYTES,
    };
    use crate::protocol::{
        BehaviorVersion, DiagnosticSeverity, EnterRule, ParseByteRange, ParseEditNotification,
        ParsePolicy, ParseWindowSnapshot,
    };

    /// Raw-runtime tests that exercise `loadPackage` need a third-party
    /// worker sharing the test op state's package authority and load-entry
    /// allowlist for the cross-domain bridge (Plan 061 task 12). The returned
    /// worker must outlive the evaluations it serves.
    fn wire_test_third_party_bridge(
        op_state: &Arc<crate::server::ops::ClayOpState>,
    ) -> Arc<super::RuntimeWorker> {
        let worker = start_runtime_worker(
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            crate::packages::bundled::RuntimeDomain::ThirdParty,
            op_state.package_service_arc(),
            op_state.load_entry_allowlist(),
        );
        op_state.set_third_party_commands(worker.sender.clone());
        worker
    }
    use crate::server::configuration::ConfigurationRuntime;
    use crate::server::parse_coordinator::{ParseCoordinator, ParseScheduleRequest};
    use crate::server::workspace::WorkspaceState;

    fn init_git_repo(root: &Path) {
        git(root, ["init", "-b", "main"]);
        git(root, ["config", "user.email", "clay@example.invalid"]);
        git(root, ["config", "user.name", "Clay Test"]);
    }

    fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .status()
            .unwrap();
        assert!(status.success(), "git command failed: {args:?}");
    }

    fn config_fixture(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("clay-{name}-{suffix}"));
        fs::create_dir_all(&root).expect("create configuration fixture root");
        root
    }

    /// Build a synthetic package manifest for provenance tests.
    fn test_package_json(
        name: &str,
        api_prefix: &str,
        permissions: &[&str],
        contributions: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": api_prefix,
                "entry": "./dist/index.js",
                "permissions": permissions,
                "modes": [api_prefix],
                "docs": "./docs/index.md",
                "contributions": contributions,
            }
        })
    }

    /// Host-side install + authorize + enable of a synthetic package, then
    /// evaluate `source` with that package's host-stamped provenance. This is
    /// the same flow production package adoption uses: authority comes from
    /// the enabled set and authorization record, never caller manifests.
    async fn evaluate_as_package(
        service: &ClayJsRuntimeService,
        package_json: serde_json::Value,
        approved: Vec<crate::packages::permissions::PackagePermission>,
        source: &str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        evaluate_as_package_with_ls_grant(service, package_json, approved, None, source).await
    }

    /// Same as [`evaluate_as_package`], additionally recording a
    /// language-server grant (which approves the `language-server`
    /// capability) for analyzer/session provenance tests.
    /// Host-side adoption flow for synthetic test packages: install, approve
    /// the exact capability set, optionally grant a language-server
    /// contribution, enable. Idempotent per package name/version.
    fn ensure_synthetic_package_enabled(
        service: &ClayJsRuntimeService,
        package_json: serde_json::Value,
        approved: Vec<crate::packages::permissions::PackagePermission>,
        language_server_grant: Option<(&str, std::path::PathBuf)>,
    ) -> crate::packages::record::PackageRecord {
        let record = crate::packages::record::assemble_package_record(&package_json)
            .expect("synthetic package record must assemble");
        let root = config_fixture("package-provenance");
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        if locked
            .enabled_record(&record.manifest.name, &record.manifest.version)
            .is_none()
        {
            locked
                .install_from_value_at_root_with_spec(package_json, root, "local:provenance-test")
                .expect("synthetic package installs");
            locked
                .authorize_package(
                    &record.manifest.name,
                    approved.clone(),
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .expect("synthetic package authorizes");
            // LS capability must be granted before enable: declared
            // capabilities require a current grant at enable time.
            if let Some((contribution, executable)) = &language_server_grant {
                locked
                    .authorize_language_server(
                        &record.manifest.name,
                        contribution,
                        executable.clone(),
                        vec![1],
                        "test",
                    )
                    .expect("language-server grant authorizes");
            }
            // Pre-execution adoption gate: synthetic third-party packages
            // need an exact durable approval before enable.
            locked
                .approve_package(&record.manifest.name, "test")
                .expect("synthetic package approves");
            locked
                .enable(&record.manifest.name)
                .expect("synthetic package enables");
        }
        record
    }

    async fn evaluate_as_package_with_ls_grant(
        service: &ClayJsRuntimeService,
        package_json: serde_json::Value,
        approved: Vec<crate::packages::permissions::PackagePermission>,
        language_server_grant: Option<(&str, std::path::PathBuf)>,
        source: &str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let record = ensure_synthetic_package_enabled(
            service,
            package_json,
            approved,
            language_server_grant,
        );
        // Evaluate in the runtime domain that owns the package (Plan 061 task
        // 7): third-party packages run their callbacks in the third-party
        // worker, which is where provider dispatch sends their commands.
        let domain = service
            .test_op_state()
            .package_service()
            .lock()
            .expect("package service mutex poisoned")
            .enabled_record(&record.manifest.name, &record.manifest.version)
            .map(|enabled| enabled.runtime_domain)
            .unwrap_or(crate::packages::bundled::RuntimeDomain::ThirdParty);
        service
            .evaluate_entry_as_package(
                domain,
                &record,
                RuntimeEntry::ControlledSource(source.to_string()),
                "runtime.evaluate_as_package",
            )
            .await
    }

    /// Same as [`evaluate_as_package`] but stamps the synthetic package into
    /// the trusted domain: for tests exercising trusted-only ops (mode
    /// activation, package loading) that third-party runtimes cannot reach.
    async fn evaluate_as_trusted_package(
        service: &ClayJsRuntimeService,
        package_json: serde_json::Value,
        approved: Vec<crate::packages::permissions::PackagePermission>,
        source: &str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let record = ensure_synthetic_package_enabled(service, package_json, approved, None);
        service
            .test_op_state()
            .package_service()
            .lock()
            .expect("package service mutex poisoned")
            .force_enabled_runtime_domain_for_test(
                &record.manifest.name,
                &record.manifest.version,
                crate::packages::bundled::RuntimeDomain::Trusted,
            );
        service
            .evaluate_entry_as_package(
                crate::packages::bundled::RuntimeDomain::Trusted,
                &record,
                RuntimeEntry::ControlledSource(source.to_string()),
                "runtime.evaluate_as_trusted_package",
            )
            .await
    }

    #[tokio::test]
    async fn js_runtime_evaluates_controlled_module() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                const ping = Deno.core.ops.op_clay_runtime_ping();
                if (ping !== "clay-runtime-ready") {
                    throw new Error(`unexpected ping: ${ping}`);
                }
                Deno.core.ops.op_clay_runtime_record("configured");
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["configured"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn persistent_js_runtime_retains_global_state_between_evaluations() {
        let service = ClayJsRuntimeService::default();
        service
            .evaluate_controlled_module(r#"globalThis.__clayPersistentRuntime = 41;"#)
            .await
            .unwrap();
        let result = service
            .evaluate_controlled_module(
                r#"
                if (globalThis.__clayPersistentRuntime !== 41) {
                    throw new Error("persistent runtime state missing");
                }
                Deno.core.ops.op_clay_runtime_record("persistent");
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["persistent"]);
        assert_eq!(service.evaluation_count(), 2);
    }

    // ── Plan 061 task 4: two-domain trust boundary tests ────────────────────

    // ── Plan 061 task 5: package-scoped provenance adversarial tests ────────

    #[tokio::test]
    async fn package_provenance_ignores_caller_supplied_identity_fields() {
        let service = ClayJsRuntimeService::default();
        // Options still carry forged identity fields naming another package;
        // publication provenance must come from the executing-package context.
        let result = evaluate_as_package(
            &service,
            test_package_json(
                "@vendor/alpha",
                "alpha",
                &["render-decorations"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::RenderDecorations],
            r#"
            import { serverPublishDiagnostics } from "clay:diagnostics";
            serverPublishDiagnostics({
              packageName: "@vendor/beta",
              packageManifest: { name: "@vendor/beta", version: "9.9.9", clay: { apiPrefix: "beta" } },
              packagePrefix: "beta",
              permissions: ["render-decorations", "raw-ops"],
              documentId: 1,
              documentVersion: 1,
              viewport: { byteStart: 0, byteEnd: 8 },
              source: "s",
              spans: [{ byteStart: 0, byteEnd: 1, severity: "error", code: "x", message: "y" }],
            });
            "#,
        )
        .await
        .unwrap();
        let set = result.published_diagnostic_set.expect("diagnostic set");
        assert_eq!(set.provenance.package_name, "@vendor/alpha");
        assert_eq!(set.provenance.package_prefix, "alpha");
    }

    #[tokio::test]
    async fn disabled_package_callback_publications_fail_closed() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@vendor/stale",
                "stale",
                &["parse-document", "render-decorations"],
                serde_json::json!({}),
            ),
            vec![
                crate::packages::permissions::PackagePermission::ParseDocument,
                crate::packages::permissions::PackagePermission::RenderDecorations,
            ],
            r#"
            import { serverPublishDiagnostics } from "clay:diagnostics";
            import { serverRegisterParseHandler } from "clay:parse";
            serverRegisterParseHandler({
              mode: "stale",
              module: { default: async (notification) => {
                serverPublishDiagnostics({
                  documentId: notification.documentId,
                  documentVersion: notification.documentVersion,
                  viewport: notification.viewport,
                  source: "stale",
                  spans: [{ byteStart: 0, byteEnd: 1, severity: "error", code: "x", message: "y" }],
                });
                return { viewport: notification.viewport };
              } }
            });
            "#,
        )
        .await
        .unwrap();
        let registration = evaluation
            .js_parse_handlers
            .first()
            .expect("handler registered")
            .clone();

        // Disable the package host-side; the stale registration's callback
        // must fail closed at op ingress (enabled-set lookup), not publish.
        service
            .test_op_state()
            .package_service()
            .lock()
            .expect("package service mutex poisoned")
            .disable("@vendor/stale")
            .expect("package disables");
        let notification = ParseEditNotification {
            document_id: 1,
            document_version: 1,
            behavior_version: 1,
            package_prefix: "stale".to_string(),
            mode_id: "stale".to_string(),
            viewport: ParseByteRange::new(0, 4),
            invalidated_ranges: vec![ParseByteRange::new(0, 4)],
            accepted_edit: None,
            parse_windows: Vec::new(),
            memory_budget: None,
        };
        let error = service
            .invoke_parse_handler(registration, notification)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("packages.package_not_enabled"),
            "stale package callback must fail closed, got {error}"
        );
    }

    #[tokio::test]
    async fn language_server_session_io_requires_executing_owner_package() {
        let service = ClayJsRuntimeService::default();
        // Package B knows A's package/contribution names and a session id, but
        // session IO is bound to the host-stamped executing package.
        let error = evaluate_as_package_with_ls_grant(
            &service,
            serde_json::json!({
                "name": "@vendor/b",
                "version": "0.1.0",
                "type": "module",
                "exports": { ".": "./dist/index.js" },
                "clay": {
                    "apiPrefix": "beta",
                    "entry": "./dist/index.js",
                    "permissions": ["parse-document"],
                    "capabilities": ["language-server"],
                    "modes": [],
                    "docs": "./docs/index.md",
                    "contributions": {
                        "languageServers": [{
                            "id": "beta.server",
                            "executable": "/bin/true",
                            "args": []
                        }]
                    }
                }
            }),
            vec![crate::packages::permissions::PackagePermission::ParseDocument],
            Some((
                "beta.server",
                std::fs::canonicalize("/bin/true").expect("canonical /bin/true"),
            )),
            r#"
            const identity = { sessionId: 1, package: "@vendor/a", contribution: "a.server" };
            try {
              await Deno.core.ops.op_clay_language_server_send_message(
                JSON.stringify({ ...identity, message: "x" }));
              throw new Error("cross-package session write must not succeed");
            } catch (error) {
              Deno.core.ops.op_clay_runtime_record(String(error));
            }
            "#,
        )
        .await
        .unwrap();
        assert!(
            evaluation_contains(&error, "language_server.session_owner_mismatch"),
            "cross-package session IO must fail, got {:?}",
            error.op_records
        );
    }

    fn evaluation_contains(evaluation: &ClayRuntimeEvaluation, needle: &str) -> bool {
        evaluation
            .op_records
            .iter()
            .any(|record| record.contains(needle))
    }

    #[tokio::test]
    async fn third_party_provider_executes_in_third_party_runtime_only() {
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@vendor/domain-dynamic",
                "domaindyn",
                &["completion-provider"],
                serde_json::json!({
                    "completionProviders": [{
                        "id": "domaindyn.provider",
                        "triggerCharacters": ["."],
                        "budgets": { "timeoutMs": 500, "maxItems": 8 }
                    }]
                }),
            ),
            vec![crate::packages::permissions::PackagePermission::CompletionProvider],
            r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({
                  module: {
                    provideCompletion: async (_request, window) => ({
                      status: "ok",
                      items: [{ label: "dynamic", insertText: "dynamic", detail: window.text }]
                    })
                  }
                });
                "#,
        )
        .await
        .unwrap();
        let coordinator = crate::server::completion::CompletionCoordinator::new();
        service
            .register_completion_providers(&coordinator, 4, &evaluation)
            .unwrap();
        let trusted_before =
            service.domain_evaluations(crate::packages::bundled::RuntimeDomain::Trusted);
        let third_party_before =
            service.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty);
        let reply_rx = coordinator
            .schedule_completion(
                "domaindyn.provider",
                crate::protocol::CompletionRequest {
                    request_id: 92,
                    client_id: 2,
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    cursor_byte_offset: 2,
                    replacement_range: crate::protocol::CompletionReplacementRange {
                        byte_start: 2,
                        byte_end: 2,
                    },
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 4,
                },
                crate::server::completion::CompletionDocumentWindow {
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    package_prefix: "domaindyn".to_string(),
                    byte_start: 0,
                    byte_end: 2,
                    text: "fn".to_string(),
                },
            )
            .unwrap();
        let result = tokio::time::timeout(Duration::from_secs(2), reply_rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.items[0].detail, "fn");
        assert!(
            service.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty)
                > third_party_before,
            "third-party provider must execute in the third-party runtime"
        );
        assert_eq!(
            service.domain_evaluations(crate::packages::bundled::RuntimeDomain::Trusted),
            trusted_before,
            "third-party provider invocation must not touch the trusted runtime"
        );
    }

    #[tokio::test]
    async fn slow_third_party_provider_poisons_only_third_party_domain() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@vendor/slow-dynamic",
                "slowdyn",
                &["completion-provider"],
                serde_json::json!({
                    "completionProviders": [{
                        "id": "slowdyn.provider",
                        "triggerCharacters": ["."],
                        "budgets": { "timeoutMs": 50, "maxItems": 8 }
                    }]
                }),
            ),
            vec![crate::packages::permissions::PackagePermission::CompletionProvider],
            r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({
                  module: {
                    provideCompletion: async () => { for (;;) {} }
                  }
                });
                "#,
        )
        .await
        .unwrap();
        let coordinator = crate::server::completion::CompletionCoordinator::new();
        service
            .register_completion_providers(&coordinator, 4, &evaluation)
            .unwrap();
        let reply_rx = coordinator
            .schedule_completion(
                "slowdyn.provider",
                crate::protocol::CompletionRequest {
                    request_id: 93,
                    client_id: 2,
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    cursor_byte_offset: 2,
                    replacement_range: crate::protocol::CompletionReplacementRange {
                        byte_start: 2,
                        byte_end: 2,
                    },
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 4,
                },
                crate::server::completion::CompletionDocumentWindow {
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    package_prefix: "slowdyn".to_string(),
                    byte_start: 0,
                    byte_end: 2,
                    text: "fn".to_string(),
                },
            )
            .unwrap();
        // The busy-loop provider times out; only the third-party domain is
        // poisoned. The trusted runtime keeps answering immediately.
        // The busy-loop provider times out inside the coordinator; the
        // request-scoped reply is dropped and the receiver observes
        // cancellation instead of a result.
        let outcome = tokio::time::timeout(Duration::from_secs(1), reply_rx).await;
        assert!(matches!(outcome, Ok(Err(_))));
        let trusted = service
            .evaluate_controlled_module(
                r#"Deno.core.ops.op_clay_runtime_record(Deno.core.ops.op_clay_runtime_ping());"#,
            )
            .await
            .expect("trusted runtime survives third-party provider timeout");
        assert_eq!(trusted.op_records, vec!["clay-runtime-ready"]);
    }

    /// Plan 061 task 12: a trusted-generation reload shares the third-party
    /// domain — providers keep answering in the SAME worker (generation and
    /// evaluation counters unchanged), and the reload snapshot re-registers
    /// them under a new generation.
    #[tokio::test]
    async fn trusted_reload_preserves_third_party_providers() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@vendor/reload-survivor",
                "survivor",
                &["completion-provider"],
                serde_json::json!({
                    "completionProviders": [{
                        "id": "survivor.provider",
                        "triggerCharacters": ["."],
                        "budgets": { "timeoutMs": 500, "maxItems": 8 }
                    }]
                }),
            ),
            vec![crate::packages::permissions::PackagePermission::CompletionProvider],
            r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({
                  module: {
                    provideCompletion: async () => ({
                      status: "ok",
                      items: [{ label: "survivor", insertText: "survivor" }]
                    })
                  }
                });
                "#,
        )
        .await
        .unwrap();

        let third_party_generation_before =
            service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty);
        let reloaded = ClayJsRuntimeService::production_reload(&service);
        assert_eq!(
            reloaded.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty),
            third_party_generation_before,
            "trusted reload must not replace the third-party worker"
        );

        // Re-register the surviving third-party registrations under the new
        // generation exactly like the reload commit path does.
        let snapshot = reloaded.third_party_registrations_snapshot();
        assert_eq!(
            snapshot.js_completion_providers.len(),
            evaluation.js_completion_providers.len(),
            "surviving worker must expose its registration payload"
        );
        let coordinator = crate::server::completion::CompletionCoordinator::new();
        reloaded
            .register_completion_providers(&coordinator, 5, &snapshot)
            .unwrap();
        let third_party_evals_before =
            reloaded.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty);
        let reply_rx = coordinator
            .schedule_completion(
                "survivor.provider",
                crate::protocol::CompletionRequest {
                    request_id: 95,
                    client_id: 2,
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    cursor_byte_offset: 2,
                    replacement_range: crate::protocol::CompletionReplacementRange {
                        byte_start: 2,
                        byte_end: 2,
                    },
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 5,
                },
                crate::server::completion::CompletionDocumentWindow {
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    package_prefix: "survivor".to_string(),
                    byte_start: 0,
                    byte_end: 2,
                    text: "su".to_string(),
                },
            )
            .unwrap();
        let result = tokio::time::timeout(Duration::from_secs(2), reply_rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.items[0].label, "survivor");
        assert!(
            reloaded.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty)
                > third_party_evals_before,
            "provider must answer in the shared third-party worker after reload"
        );
    }

    /// Plan 061 task 12: a poisoned third-party domain is replaced once and
    /// replays ONLY the current approved graph; deterministic registration
    /// tokens make the pre-poison coordinator registrations valid again.
    #[tokio::test]
    async fn third_party_poison_replays_approved_graph_and_restores_providers() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(100));
        let root = config_fixture("third-party-replay").join("replayd");
        write_loadable_package(
            &root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => ({
                  status: "ok",
                  items: [{ label: "replayed", insertText: "replayed" }]
                })
              }
            });
            export default function load() {}
            "#,
        );
        let package_json = test_package_json(
            "@vendor/replayd",
            "replayd",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "replayd.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 50, "maxItems": 8 }
                }]
            }),
        );
        let approved = vec![crate::packages::permissions::PackagePermission::CompletionProvider];
        ensure_synthetic_package_enabled(&service, package_json.clone(), approved.clone(), None);
        // Record the load entry so the poison-recovery replay can re-import it.
        service
            .test_op_state()
            .load_entry_allowlist()
            .record_for_package(
                "clay://packages/@vendor/replayd/dist/load.js",
                root.join("dist/load.js"),
                root.clone(),
                Some("@vendor/replayd"),
            );
        let evaluation = evaluate_as_package(
            &service,
            package_json.clone(),
            approved.clone(),
            r#"const m = await import("clay://packages/@vendor/replayd/dist/load.js"); await m.default();"#,
        )
        .await
        .unwrap();
        let coordinator = crate::server::completion::CompletionCoordinator::new();
        service
            .register_completion_providers(&coordinator, 4, &evaluation)
            .unwrap();

        // Poison the third-party domain with a busy-loop evaluation.
        let _ = evaluate_as_package(&service, package_json, approved, "for (;;) {}").await;
        let generation_after_poison =
            service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty);

        // The next third-party dispatch replaces the worker and replays the
        // approved graph: the provider answers again under the same token.
        let reply_rx = coordinator
            .schedule_completion(
                "replayd.provider",
                crate::protocol::CompletionRequest {
                    request_id: 96,
                    client_id: 2,
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    cursor_byte_offset: 2,
                    replacement_range: crate::protocol::CompletionReplacementRange {
                        byte_start: 2,
                        byte_end: 2,
                    },
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 4,
                },
                crate::server::completion::CompletionDocumentWindow {
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    package_prefix: "replayd".to_string(),
                    byte_start: 0,
                    byte_end: 2,
                    text: "re".to_string(),
                },
            )
            .unwrap();
        let result = tokio::time::timeout(Duration::from_secs(3), reply_rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.items[0].label, "replayed");
        assert!(
            service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty)
                > generation_after_poison,
            "poison recovery must bump the third-party domain generation"
        );
        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    /// Plan 061 task 13 adversarial: the cross-domain load bridge rejects a
    /// TRUSTED record — config must never route bundled packages through the
    /// third-party runtime via the bridge op.
    #[tokio::test]
    async fn cross_domain_load_bridge_rejects_trusted_records() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                const result = JSON.parse(
                    await Deno.core.ops.op_clay_packages_load_package_by_specifier(
                        JSON.stringify({ specifier: "@clay/markdown" })
                    )
                );
                await Deno.core.ops.op_clay_packages_load_in_package_domain(
                    JSON.stringify(result)
                );
                "#,
            )
            .await
            .expect_err("bridge must reject a trusted-domain record");
        let message = error.to_string();
        assert!(
            message.contains("is not a third-party package"),
            "unexpected bridge denial message: {message}"
        );
        assert_eq!(
            service.domain_evaluations(crate::packages::bundled::RuntimeDomain::ThirdParty),
            0,
            "denied bridge call must not touch the third-party runtime"
        );
    }

    /// Plan 061 task 13 adversarial: poison recovery replays ONLY the current
    /// approved graph — a package disabled after poisoning is not replayed,
    /// and its pre-poison coordinator token fails closed.
    #[tokio::test]
    async fn third_party_poison_replay_skips_disabled_packages() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(100));
        let root = config_fixture("third-party-replay-skip").join("skipd");
        write_loadable_package(
            &root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => ({
                  status: "ok",
                  items: [{ label: "skipped", insertText: "skipped" }]
                })
              }
            });
            export default function load() {}
            "#,
        );
        let package_json = test_package_json(
            "@vendor/skipd",
            "skipd",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "skipd.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 50, "maxItems": 8 }
                }]
            }),
        );
        let approved = vec![crate::packages::permissions::PackagePermission::CompletionProvider];
        ensure_synthetic_package_enabled(&service, package_json.clone(), approved.clone(), None);
        service
            .test_op_state()
            .load_entry_allowlist()
            .record_for_package(
                "clay://packages/@vendor/skipd/dist/load.js",
                root.join("dist/load.js"),
                root.clone(),
                Some("@vendor/skipd"),
            );
        let evaluation = evaluate_as_package(
            &service,
            package_json.clone(),
            approved.clone(),
            r#"const m = await import("clay://packages/@vendor/skipd/dist/load.js"); await m.default();"#,
        )
        .await
        .unwrap();
        let coordinator = crate::server::completion::CompletionCoordinator::new();
        service
            .register_completion_providers(&coordinator, 4, &evaluation)
            .unwrap();
        // Poison, then disable the package before any dispatch replays it.
        let _ = evaluate_as_package(&service, package_json, approved, "for (;;) {}").await;
        service
            .test_op_state()
            .package_service()
            .lock()
            .unwrap()
            .disable("@vendor/skipd")
            .unwrap();
        let reply_rx = coordinator
            .schedule_completion(
                "skipd.provider",
                crate::protocol::CompletionRequest {
                    request_id: 97,
                    client_id: 2,
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    cursor_byte_offset: 2,
                    replacement_range: crate::protocol::CompletionReplacementRange {
                        byte_start: 2,
                        byte_end: 2,
                    },
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 4,
                },
                crate::server::completion::CompletionDocumentWindow {
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    package_prefix: "skipd".to_string(),
                    byte_start: 0,
                    byte_end: 2,
                    text: "sk".to_string(),
                },
            )
            .unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(1), reply_rx).await;
        // A dropped request-scoped reply or silent drop are both fail-closed:
        // no completion is ever produced from the disabled package.
        if let Ok(Ok(result)) = outcome {
            assert_ne!(
                result.status,
                crate::protocol::CompletionStatus::Ok,
                "disabled package must not be replayed; provider must fail closed"
            );
        }
        assert!(
            service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty) > 1,
            "poison recovery must have replaced the third-party worker"
        );
        service
            .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('alive');")
            .await
            .expect("third-party domain stays alive after replay skip");
        assert!(
            !service
                .test_op_state()
                .package_service()
                .lock()
                .unwrap()
                .inspect("@vendor/skipd")
                .unwrap()
                .is_enabled,
            "replay must not re-enable the disabled package"
        );
        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    /// Plan 061 task 13 adversarial: a third-party package cannot call
    /// loadPackage — the loader op is trusted-only, so the public
    /// clay:packages facade fails closed by op absence in the third-party
    /// runtime.
    #[tokio::test]
    async fn third_party_package_cannot_load_other_packages() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let error = evaluate_as_package(
            &service,
            test_package_json("@vendor/loader", "loaderd", &[], serde_json::json!({})),
            Vec::new(),
            r#"
                import { loadPackage } from "clay:packages";
                await loadPackage("@clay/markdown");
                "#,
        )
        .await
        .expect_err("third-party loadPackage must fail closed");
        let message = error.to_string();
        // clay:packages is not in the third-party facade allowlist: denial at
        // the import boundary, before any op is reachable.
        assert!(
            message.contains("not allowed in the server runtime boundary"),
            "expected import-boundary denial, got: {message}"
        );
        assert!(
            service
                .test_op_state()
                .package_service()
                .lock()
                .unwrap()
                .enabled_records()
                .all(|record| record.manifest.name != "@clay/markdown"),
            "denied loadPackage must not enable the target package"
        );
    }

    #[tokio::test]
    async fn third_party_runtime_cannot_see_trusted_ops_or_admin_modules() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();

        // Trusted-only/admin op names are not even enumerable in the
        // third-party isolate; public contribution ops exist in both. The
        // seven editor ops are shared (follow-up round `editor-control`) but
        // gated per call by permission + declared mode; visibility alone
        // grants nothing.
        let probe = service
            .evaluate_third_party_module(
                r#"
                const ops = Deno.core.ops;
                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    ping: typeof ops.op_clay_runtime_ping,
                    configGet: typeof ops.op_clay_configuration_get_state,
                    openDocument: typeof ops.op_clay_documents_open_document,
                    loadPackage: typeof ops.op_clay_packages_load_package,
                    authorizeLs: typeof ops.op_clay_language_server_authorize,
                    classify: typeof ops.op_clay_modes_classify_document,
                    setTheme: typeof ops.op_clay_theme_set_theme,
                    editorMove: typeof ops.op_clay_editor_move_cursor,
                    editorSelect: typeof ops.op_clay_editor_set_selection,
                    editorCaret: typeof ops.op_clay_editor_set_cursor_style,
                    editorAddCursor: typeof ops.op_clay_editor_add_cursor,
                    editorColumnSelect: typeof ops.op_clay_editor_column_select,
                    editorTextobject: typeof ops.op_clay_editor_select_textobject,
                    editorSmartSelect: typeof ops.op_clay_editor_smart_select,
                    publicRegister: typeof ops.op_clay_commands_register_command,
                    publicPublish: typeof ops.op_clay_decorations_publish_decorations,
                }));
                "#,
            )
            .await
            .expect("third-party op probe evaluation");
        assert_eq!(
            probe.op_records,
            vec![
                r#"{"ping":"undefined","configGet":"undefined","openDocument":"undefined","loadPackage":"undefined","authorizeLs":"undefined","classify":"undefined","setTheme":"undefined","editorMove":"function","editorSelect":"function","editorCaret":"function","editorAddCursor":"function","editorColumnSelect":"function","editorTextobject":"function","editorSmartSelect":"function","publicRegister":"function","publicPublish":"function"}"#
            ]
        );

        // The same names resolve to real functions in the trusted isolate.
        let trusted_probe = service
            .evaluate_controlled_module(
                r#"
                const ops = Deno.core.ops;
                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    ping: typeof ops.op_clay_runtime_ping,
                    configGet: typeof ops.op_clay_configuration_get_state,
                    loadPackage: typeof ops.op_clay_packages_load_package,
                    editorMove: typeof ops.op_clay_editor_move_cursor,
                    editorTextobject: typeof ops.op_clay_editor_select_textobject,
                }));
                "#,
            )
            .await
            .expect("trusted op probe evaluation");
        assert_eq!(
            trusted_probe.op_records,
            vec![
                r#"{"ping":"function","configGet":"function","loadPackage":"function","editorMove":"function","editorTextobject":"function"}"#
            ]
        );

        // Admin/internal facade modules do not resolve in the third-party
        // domain; public facades do.
        for specifier in [
            "clay:configuration",
            "clay:documents",
            "clay:workspace",
            "clay:keybindings",
            "clay:packages",
            "clay:theme",
            "clay:application",
            "clay:editor",
            "clay:shell",
        ] {
            let result = service
                .evaluate_third_party_module(format!(r#"import "{specifier}";"#))
                .await;
            assert!(
                result.is_err(),
                "third-party domain must reject admin facade {specifier}"
            );
        }
        for specifier in ["clay:commands", "clay:decorations", "clay:sdui"] {
            service
                .evaluate_third_party_module(format!(r#"import "{specifier}";"#))
                .await
                .unwrap_or_else(|error| panic!("third-party public facade {specifier}: {error}"));
        }
    }

    #[tokio::test]
    async fn domain_globals_and_module_state_do_not_cross() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        service
            .evaluate_controlled_module("globalThis.__clayDomainProbe = 'trusted';")
            .await
            .expect("trusted global write");
        let third_party = service
            .evaluate_third_party_module(
                "Deno.core.ops.op_clay_runtime_record(String(globalThis.__clayDomainProbe));",
            )
            .await
            .expect("third-party global read");
        assert_eq!(third_party.op_records, vec!["undefined"]);
        service
            .evaluate_third_party_module("globalThis.__clayDomainProbe = 'third-party';")
            .await
            .expect("third-party global write");
        let trusted = service
            .evaluate_controlled_module(
                "Deno.core.ops.op_clay_runtime_record(String(globalThis.__clayDomainProbe));",
            )
            .await
            .expect("trusted global read");
        assert_eq!(trusted.op_records, vec!["trusted"]);
    }

    #[tokio::test]
    async fn third_party_termination_replaces_only_third_party_generation() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
        // Runaway third-party evaluation terminates only its own domain.
        let timed_out = service.evaluate_third_party_module("while (true) {}").await;
        assert!(
            matches!(timed_out, Err(ClayRuntimeError::Timeout)),
            "third-party runaway must time out, got {timed_out:?}"
        );
        // Trusted domain stays responsive without any worker replacement.
        let workers_after_timeout = service.workers_started();
        service
            .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
            .await
            .expect("trusted runtime remains responsive after third-party termination");
        assert_eq!(workers_after_timeout, 2);
        assert_eq!(service.workers_started(), 2);
        // Third-party domain recovers by replacing only its own worker.
        service
            .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('recovered');")
            .await
            .expect("third-party domain restarts after termination");
        assert_eq!(service.workers_started(), 3);
    }

    #[cfg(target_os = "linux")]
    fn process_rss_kib_and_threads() -> (u64, usize) {
        let status = fs::read_to_string("/proc/self/status").expect("read process status");
        let rss_kib = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse().ok())
            .expect("parse VmRSS from process status");
        let threads = fs::read_dir("/proc/self/task")
            .expect("read process task directory")
            .count();
        (rss_kib, threads)
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    #[ignore = "manual Plan 061 before/after resource baseline"]
    async fn runtime_resource_baseline_probe() {
        let (rss_before_kib, threads_before) = process_rss_kib_and_threads();
        let startup_started = Instant::now();
        let service = ClayJsRuntimeService::default();
        service
            .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
            .await
            .expect("first runtime evaluation");
        let startup_us = startup_started.elapsed().as_micros();
        tokio::time::sleep(Duration::from_millis(25)).await;
        let (rss_after_start_kib, threads_after_start) = process_rss_kib_and_threads();

        let mut warm_evaluation_us = Vec::with_capacity(20);
        for _ in 0..20 {
            let started = Instant::now();
            service
                .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
                .await
                .expect("warm runtime evaluation");
            warm_evaluation_us.push(started.elapsed().as_micros());
        }
        warm_evaluation_us.sort_unstable();
        let warm_evaluation_median_us = warm_evaluation_us[warm_evaluation_us.len() / 2];

        let mut package_load_us = Vec::new();
        for specifier in [
            "@clay/rust",
            "@clay/markdown",
            "@clay/git",
            "@clay/theme-gruvbox-material-dark",
        ] {
            let started = Instant::now();
            service
                .evaluate_controlled_module(format!(
                    "import {{ loadPackage }} from 'clay:packages'; await loadPackage({specifier:?});"
                ))
                .await
                .unwrap_or_else(|error| panic!("load {specifier}: {error}"));
            package_load_us.push((specifier, started.elapsed().as_micros()));
        }
        let enabled_packages = service
            .test_op_state()
            .package_service()
            .lock()
            .expect("package service mutex poisoned")
            .enabled_records()
            .count();
        tokio::time::sleep(Duration::from_millis(25)).await;
        let (rss_after_packages_kib, threads_after_packages) = process_rss_kib_and_threads();

        let reload_started = Instant::now();
        let candidate = ClayJsRuntimeService::default();
        candidate
            .evaluate_controlled_module("Deno.core.ops.op_clay_runtime_ping();")
            .await
            .expect("candidate runtime evaluation");
        let candidate_reload_us = reload_started.elapsed().as_micros();
        tokio::time::sleep(Duration::from_millis(25)).await;
        let (rss_with_candidate_kib, threads_with_candidate) = process_rss_kib_and_threads();
        drop(candidate);

        // Plan 061 task 4: analysis invokes route through the two domain
        // runtimes; no per-analyzer persistent runtimes exist anymore.
        let (rss_with_max_analysis_kib, threads_with_max_analysis) =
            (rss_with_candidate_kib, threads_with_candidate);

        assert_eq!(enabled_packages, 4);
        assert_eq!(service.workers_started(), 2);
        eprintln!(
            "PLAN061_RUNTIME_BASELINE rss_before_kib={rss_before_kib} rss_after_start_kib={rss_after_start_kib} rss_after_packages_kib={rss_after_packages_kib} rss_with_candidate_kib={rss_with_candidate_kib} rss_with_max_analysis_kib={rss_with_max_analysis_kib} threads_before={threads_before} threads_after_start={threads_after_start} threads_after_packages={threads_after_packages} threads_with_candidate={threads_with_candidate} threads_with_max_analysis={threads_with_max_analysis} startup_us={startup_us} warm_evaluation_median_us={warm_evaluation_median_us} candidate_reload_us={candidate_reload_us} enabled_packages={enabled_packages} package_load_us={package_load_us:?} main_heap_limit_bytes={JS_RUNTIME_HEAP_LIMIT_BYTES} persistent_workers_started={}",
            service.workers_started()
        );

        // Plan 061 task 13: third-party provider latency + bridge
        // saturation (serial cross-domain dispatches through the completion
        // coordinator).
        let provider_evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@vendor/probe-provider",
                "probeprov",
                &["completion-provider"],
                serde_json::json!({
                    "completionProviders": [{
                        "id": "probeprov.provider",
                        "triggerCharacters": ["."],
                        "budgets": { "timeoutMs": 500, "maxItems": 8 }
                    }]
                }),
            ),
            vec![crate::packages::permissions::PackagePermission::CompletionProvider],
            r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({
                  module: {
                    provideCompletion: async (_request, window) => ({
                      status: "ok",
                      items: [{ label: "probe", insertText: "probe", detail: window.text }]
                    })
                  }
                });
                "#,
        )
        .await
        .unwrap();
        let coordinator = crate::server::completion::CompletionCoordinator::new();
        service
            .register_completion_providers(&coordinator, 4, &provider_evaluation)
            .unwrap();
        let completion_window = crate::server::completion::CompletionDocumentWindow {
            document_id: 7,
            document_version: 3,
            behavior_version: 5,
            package_prefix: "probeprov".to_string(),
            byte_start: 0,
            byte_end: 2,
            text: "pr".to_string(),
        };
        let mut provider_invoke_us = Vec::with_capacity(20);
        let saturation_started = Instant::now();
        for index in 0..20u32 {
            let started = Instant::now();
            let reply_rx = coordinator
                .schedule_completion(
                    "probeprov.provider",
                    crate::protocol::CompletionRequest {
                        request_id: u64::from(100 + index),
                        client_id: 2,
                        document_id: 7,
                        document_version: 3,
                        behavior_version: 5,
                        cursor_byte_offset: 2,
                        replacement_range: crate::protocol::CompletionReplacementRange {
                            byte_start: 2,
                            byte_end: 2,
                        },
                        trigger: crate::protocol::CompletionTrigger::Manual,
                        provider_generation: 4,
                    },
                    completion_window.clone(),
                )
                .unwrap();
            tokio::time::timeout(Duration::from_secs(2), reply_rx)
                .await
                .unwrap()
                .unwrap();
            provider_invoke_us.push(started.elapsed().as_micros());
        }
        provider_invoke_us.sort_unstable();
        let provider_invoke_median_us = provider_invoke_us[provider_invoke_us.len() / 2];
        let bridge_saturation_20_serial_us = saturation_started.elapsed().as_micros();

        // Third-party recovery: poison the domain with a busy-loop, then time
        // the first successful provider answer (replace + replay + invoke).
        let recovery_service = ClayJsRuntimeService::with_timeout(Duration::from_millis(50));
        let recovery_root = config_fixture("third-party-recovery-probe").join("recoverd");
        write_loadable_package(
            &recovery_root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => ({
                  status: "ok",
                  items: [{ label: "recovered", insertText: "recovered" }]
                })
              }
            });
            export default function load() {}
            "#,
        );
        let recovery_json = test_package_json(
            "@vendor/recoverd",
            "recoverd",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "recoverd.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 50, "maxItems": 8 }
                }]
            }),
        );
        let recovery_permissions =
            vec![crate::packages::permissions::PackagePermission::CompletionProvider];
        ensure_synthetic_package_enabled(
            &recovery_service,
            recovery_json.clone(),
            recovery_permissions.clone(),
            None,
        );
        recovery_service
            .test_op_state()
            .load_entry_allowlist()
            .record_for_package(
                "clay://packages/@vendor/recoverd/dist/load.js",
                recovery_root.join("dist/load.js"),
                recovery_root.clone(),
                Some("@vendor/recoverd"),
            );
        let recovery_evaluation = evaluate_as_package(
            &recovery_service,
            recovery_json.clone(),
            recovery_permissions.clone(),
            r#"const m = await import("clay://packages/@vendor/recoverd/dist/load.js"); await m.default();"#,
        )
        .await
        .unwrap();
        let recovery_coordinator = crate::server::completion::CompletionCoordinator::new();
        recovery_service
            .register_completion_providers(&recovery_coordinator, 4, &recovery_evaluation)
            .unwrap();
        let _ = evaluate_as_package(
            &recovery_service,
            recovery_json,
            recovery_permissions,
            "for (;;) {}",
        )
        .await;
        let recovery_started = Instant::now();
        let reply_rx = recovery_coordinator
            .schedule_completion(
                "recoverd.provider",
                crate::protocol::CompletionRequest {
                    request_id: 200,
                    client_id: 2,
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    cursor_byte_offset: 2,
                    replacement_range: crate::protocol::CompletionReplacementRange {
                        byte_start: 2,
                        byte_end: 2,
                    },
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 4,
                },
                crate::server::completion::CompletionDocumentWindow {
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    package_prefix: "recoverd".to_string(),
                    byte_start: 0,
                    byte_end: 2,
                    text: "re".to_string(),
                },
            )
            .unwrap();
        let recovered = tokio::time::timeout(Duration::from_secs(3), reply_rx)
            .await
            .unwrap()
            .unwrap();
        let third_party_recovery_us = recovery_started.elapsed().as_micros();
        assert_eq!(recovered.items[0].label, "recovered");
        let _ = fs::remove_dir_all(recovery_root.parent().unwrap());

        eprintln!(
            "PLAN061_CROSS_DOMAIN provider_invoke_median_us={provider_invoke_median_us} bridge_saturation_20_serial_us={bridge_saturation_20_serial_us} third_party_recovery_us={third_party_recovery_us}"
        );
    }

    #[tokio::test]
    async fn js_parse_handler_bridge_runs_registered_markdown_handler() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                await loadPackage("@clay/markdown");
                "#,
            )
            .await
            .unwrap();
        assert_eq!(evaluation.js_parse_handlers.len(), 1);

        let coordinator = ParseCoordinator::new();
        service
            .register_parse_handlers(&coordinator, 1, &evaluation)
            .unwrap();

        let text = "# Title\n";
        let request = ParseScheduleRequest {
            document_id: 1,
            document_version: 1,
            behavior_version: 1 as BehaviorVersion,
            package_prefix: "markdown".to_string(),
            mode_id: "markdown".to_string(),
            viewport: ParseByteRange::new(0, text.len() as u64),
            invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
            accepted_edit: None,
        };
        let windows = vec![ParseWindowSnapshot {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            mode_id: "markdown".to_string(),
            window_id: 0,
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            base_column: 0,
            incremental_edit: false,
            text: text.to_string(),
        }];
        coordinator
            .schedule_parse_with_windows(
                request,
                windows,
                Some(ParsePolicy::new(
                    64 * 1024,
                    4 * 1024,
                    30 * 1024 * 1024,
                    5_000,
                )),
            )
            .unwrap();

        let update = tokio::time::timeout(Duration::from_secs(6), coordinator.next_update())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(update.package_prefix, "markdown");
        assert!(
            update
                .decoration_updates
                .iter()
                .any(|set| !set.spans.is_empty()),
            "markdown parser produced syntax decorations"
        );
    }

    #[tokio::test]
    async fn js_parse_handler_bridge_accepts_inert_diagnostic_records() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/fixture-parser",
                "fixture",
                &["parse-document"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::ParseDocument],
            r#"
                import { serverRegisterParseHandler } from "clay:parse";
                const parser = { default: async (notification) => ({
                  diagnostics: {
                    source: "fixture-parser",
                    spans: [{
                      byteStart: 0,
                      byteEnd: 1,
                      severity: "error",
                      code: "syntax.error",
                      message: "syntax error",
                    }],
                  },
                  viewport: notification.viewport,
                }) };
                serverRegisterParseHandler({
                  module: parser,
                  mode: "fixture",
                });
                "#,
        )
        .await
        .unwrap();
        let coordinator = ParseCoordinator::new();
        service
            .register_parse_handlers(&coordinator, 1, &evaluation)
            .unwrap();
        coordinator
            .schedule_parse(ParseScheduleRequest {
                document_id: 9,
                document_version: 2,
                behavior_version: 1,
                package_prefix: "fixture".to_string(),
                mode_id: "fixture".to_string(),
                viewport: ParseByteRange::new(0, 8),
                invalidated_ranges: vec![ParseByteRange::new(0, 1)],
                accepted_edit: None,
            })
            .unwrap();

        let update = tokio::time::timeout(Duration::from_secs(1), coordinator.next_update())
            .await
            .unwrap()
            .unwrap();
        let diagnostics = update.diagnostic_update.unwrap();

        assert_eq!(diagnostics.source, "fixture-parser");
        assert_eq!(diagnostics.spans[0].severity, DiagnosticSeverity::Error);
    }

    #[tokio::test]
    async fn parse_registration_rejects_executable_callbacks_and_missing_permissions() {
        let service = ClayJsRuntimeService::default();
        // Executable callback fields are rejected by the facade before any op.
        let error = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterParseHandler } from "clay:parse";
                serverRegisterParseHandler({
                  mode: "evil",
                  handler() {}
                });
                "#,
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("parse.invalid_handler"),
            "unexpected registration error: {error}"
        );
        // Missing approved parse-document capability fails closed.
        let error = evaluate_as_package(
            &service,
            test_package_json("@clay/no-parse", "noparse", &[], serde_json::json!({})),
            vec![],
            r#"
            import { serverRegisterParseHandler } from "clay:parse";
            serverRegisterParseHandler({
              mode: "noparse"
            });
            "#,
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("packages.missing_permission"),
            "unexpected registration error: {error}"
        );
    }

    #[tokio::test]
    async fn syntax_facade_registers_grammar_metadata_without_raw_ops() {
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/rust-grammar",
                "rust",
                &["parse-document", "render-decorations"],
                serde_json::json!({
                    "syntaxGrammars": [{
                        "languageId": "rust",
                        "filePatterns": { "extensions": ["rs"] },
                        "grammar": { "kind": "native", "source": "tree-sitter-rust" },
                        "queries": { "highlights": "./queries/highlights.scm" },
                        "styleMap": {
                          "keyword": { "type": "Keyword" },
                          "string": { "type": "String" },
                          "comment": { "type": "Comment" },
                          "punctuation": { "type": "Operator" }
                        },
                        "budgets": { "timeoutMs": 5000, "maxWindowBytes": 4096 }
                    }]
                }),
            ),
            vec![
                crate::packages::permissions::PackagePermission::ParseDocument,
                crate::packages::permissions::PackagePermission::RenderDecorations,
            ],
            r#"
                import { serverRegisterSyntaxGrammar } from "clay:syntax";
                const result = serverRegisterSyntaxGrammar({});
                Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.languages[0]}:${result.registeredGrammarCount}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(evaluation.op_records, vec!["rust:rust:0"]);
        assert!(evaluation.syntax_grammars.iter().any(|grammar| {
            grammar.language_id == "rust"
                && grammar.engine_tier == crate::server::syntax::SyntaxEngineTier::Native
        }));
    }

    #[tokio::test]
    async fn syntax_facade_engine_preference_allows_explicit_wasm_override() {
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_trusted_package(
            &service,
            test_package_json(
                "@clay/rust-grammar",
                "rust",
                &["parse-document", "render-decorations"],
                serde_json::json!({
                    "syntaxGrammars": [{
                        "languageId": "rust",
                        "filePatterns": { "extensions": ["rs"] },
                        "grammar": { "kind": "tree-sitter-wasm", "path": "./grammars/rust.wasm" },
                        "queries": { "highlights": "./queries/highlights.scm" },
                        "styleMap": { "keyword": { "type": "Keyword" } }
                    }]
                }),
            ),
            vec![
                crate::packages::permissions::PackagePermission::ParseDocument,
                crate::packages::permissions::PackagePermission::RenderDecorations,
            ],
            r#"
                import { setSyntaxEnginePreference, serverRegisterSyntaxGrammar } from "clay:syntax";
                setSyntaxEnginePreference("rust", "wasm");
                const result = serverRegisterSyntaxGrammar({});
                Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.registeredGrammarCount}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(evaluation.op_records, vec!["rust:1"]);
        assert!(evaluation.syntax_grammars.iter().any(|grammar| {
            grammar.language_id == "rust"
                && grammar.engine_tier == crate::server::syntax::SyntaxEngineTier::Wasm
        }));
    }

    #[tokio::test]
    async fn javascript_engine_preference_keeps_markdown_tier3_fallback_selected() {
        let root = config_fixture("markdown-javascript-fallback");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            import { setSyntaxEnginePreference } from "clay:syntax";
            setSyntaxEnginePreference("markdown", "javascript");
            await loadPackage("@clay/markdown");
            "#,
        )
        .unwrap();
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .load_configuration_from_root(root)
            .await
            .expect("Markdown JavaScript preference loads");
        assert_eq!(
            evaluation.syntax_engine_preferences.get("markdown"),
            Some(&crate::server::syntax::SyntaxEngineTier::JavaScriptFallback)
        );
        assert_eq!(evaluation.js_parse_handlers.len(), 1);
        assert!(
            service
                .register_native_syntax_handler(
                    &ParseCoordinator::new(),
                    1,
                    &evaluation,
                    "note.md",
                    "markdown",
                    "markdown",
                )
                .expect("engine selection")
                .is_none(),
            "explicit JavaScript preference must suppress native handler installation"
        );
    }

    #[tokio::test]
    async fn typescript_and_tsx_install_their_selected_native_handlers() {
        let root = config_fixture("typescript-native-handlers");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/typescript");
            "#,
        )
        .unwrap();
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .load_configuration_from_root(root)
            .await
            .expect("TypeScript package loads");
        let coordinator = ParseCoordinator::new();

        let typescript = service
            .register_native_syntax_handler(
                &coordinator,
                1,
                &evaluation,
                "app.ts",
                "typescript",
                "typescript",
            )
            .expect("TypeScript native selection")
            .expect("TypeScript native handler");
        let tsx = service
            .register_native_syntax_handler(
                &coordinator,
                1,
                &evaluation,
                "app.tsx",
                "typescript",
                "typescript",
            )
            .expect("TSX native selection")
            .expect("TSX native handler");

        assert_eq!(typescript.0.mode_id, "typescript.typescript");
        assert_eq!(tsx.0.mode_id, "typescript.tsx");
    }

    #[tokio::test]
    async fn syntax_facade_rejects_raw_authority_and_third_party_grammars() {
        let service = ClayJsRuntimeService::default();
        // Raw authority fields are rejected by the facade before any op.
        let error = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterSyntaxGrammar } from "clay:syntax";
                serverRegisterSyntaxGrammar({ rawOps: true });
                "#,
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("syntax.invalid_grammar"),
            "unexpected syntax registration error: {error}"
        );
        // Third-party grammar contributions are rejected at host-side manifest
        // validation, before any package code runs.
        let error = crate::packages::record::assemble_package_record(&test_package_json(
            "@vendor/rust",
            "vendor-rust",
            &["parse-document", "render-decorations"],
            serde_json::json!({
                "syntaxGrammars": [{
                    "languageId": "rust",
                    "filePatterns": { "extensions": ["rs"] },
                    "grammar": { "kind": "tree-sitter-wasm", "path": "./grammars/rust.wasm" },
                    "queries": { "highlights": "./queries/highlights.scm" },
                    "styleMap": { "keyword": { "type": "Keyword" } }
                }]
            }),
        ))
        .unwrap_err();
        assert!(
            error.message.contains("first-party-only"),
            "unexpected manifest validation error: {error:?}"
        );
    }

    #[tokio::test]
    async fn completion_facade_registers_provider_metadata_without_raw_ops() {
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_trusted_package(
            &service,
            test_package_json(
                "@vendor/words",
                "words",
                &["completion-provider"],
                serde_json::json!({
                    "completionProviders": [{
                        "id": "words.buffer",
                        "priority": 2,
                        "exclusive": true,
                        "triggerCharacters": ["."],
                        "wordBoundaryChars": [".", ","],
                        "budgets": { "timeoutMs": 50, "maxItems": 20 }
                    }]
                }),
            ),
            vec![crate::packages::permissions::PackagePermission::CompletionProvider],
            r#"
                import { serverListCompletionProvidersForTrigger, serverRegisterCompletionProvider } from "clay:completion";
                const result = serverRegisterCompletionProvider({});
                const listed = serverListCompletionProvidersForTrigger({ trigger: "." });
                Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.providers[0]}:${result.registeredProviderCount}:${result.runtimeBridge}:${listed.providers[0].exclusive}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(
            evaluation.op_records,
            vec!["words:words.buffer:1:false:true"]
        );
        assert_eq!(evaluation.completion_providers.len(), 1);
        assert_eq!(evaluation.completion_providers[0].id, "words.buffer");
        assert!(evaluation.completion_providers[0].exclusive);
        assert_eq!(
            evaluation.completion_providers[0].provenance.package_prefix,
            "words"
        );
    }

    #[tokio::test]
    async fn completion_facade_invokes_token_backed_dynamic_provider() {
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@vendor/dynamic",
                "dynamic",
                &["completion-provider"],
                serde_json::json!({
                    "completionProviders": [{
                        "id": "dynamic.provider",
                        "triggerCharacters": ["."],
                        "budgets": { "timeoutMs": 500, "maxItems": 8 }
                    }]
                }),
            ),
            vec![crate::packages::permissions::PackagePermission::CompletionProvider],
            r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({
                  module: {
                    provideCompletion: async (_request, window) => ({
                      status: "ok",
                      items: [{ label: "dynamic", insertText: "dynamic", detail: window.text }]
                    })
                  }
                });
                "#,
        )
        .await
        .unwrap();
        let coordinator = crate::server::completion::CompletionCoordinator::new();
        service
            .register_completion_providers(&coordinator, 4, &evaluation)
            .unwrap();
        let request = crate::protocol::CompletionRequest {
            request_id: 91,
            client_id: 2,
            document_id: 7,
            document_version: 3,
            behavior_version: 5,
            cursor_byte_offset: 2,
            replacement_range: crate::protocol::CompletionReplacementRange {
                byte_start: 2,
                byte_end: 2,
            },
            trigger: crate::protocol::CompletionTrigger::Manual,
            provider_generation: 4,
        };
        let reply_rx = coordinator
            .schedule_completion(
                "dynamic.provider",
                request,
                crate::server::completion::CompletionDocumentWindow {
                    document_id: 7,
                    document_version: 3,
                    behavior_version: 5,
                    package_prefix: "dynamic".to_string(),
                    byte_start: 0,
                    byte_end: 2,
                    text: "fn".to_string(),
                },
            )
            .unwrap();

        let result = tokio::time::timeout(Duration::from_secs(2), reply_rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.items[0].detail, "fn");
    }

    #[tokio::test]
    async fn language_intelligence_facade_registers_token_backed_provider_without_process_authority()
     {
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json("@org/intel", "intel", &["parse-document"], serde_json::json!({})),
            vec![crate::packages::permissions::PackagePermission::ParseDocument],
            r#"
                import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
                const result = serverRegisterLanguageIntelligenceProvider({
                  provider: {
                    id: "intel.intelligence",
                    modes: ["intel"],
                    features: ["hover", "definition", "codeAction", "signatureHelp"],
                    priority: 10,
                    timeoutMs: 500
                  },
                  module: {
                    provideLanguageIntelligence: async () => ({ status: "ok" })
                  }
                });
                Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.providerId}:${result.runtimeBridge}:${result.languageServerRequired}:${typeof result.token}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(
            evaluation.op_records,
            vec!["intel:intel.intelligence:true:false:string"]
        );
        assert_eq!(evaluation.language_intelligence_providers.len(), 1);
        assert_eq!(
            evaluation.language_intelligence_providers[0].id,
            "intel.intelligence"
        );
        assert_eq!(evaluation.js_language_intelligence_providers.len(), 1);
        assert_eq!(
            evaluation.js_language_intelligence_providers[0].export_name,
            "provideLanguageIntelligence"
        );
        assert!(
            !evaluation.language_intelligence_providers[0]
                .provenance
                .package_prefix
                .is_empty()
        );
    }

    #[tokio::test]
    async fn document_analyzer_registration_rejects_unowned_module_and_runtime_process_fields() {
        let service = ClayJsRuntimeService::default();
        for analyzer in [
            r#"{ id: "analysis.worker", contribution: "analysis.server", moduleSpecifier: "file:///tmp/escape.js" }"#,
            r#"{ id: "analysis.worker", contribution: "analysis.server", moduleSpecifier: "clay://packages/other/worker.js", executable: "/bin/true" }"#,
        ] {
            let source = format!(
                r#"
                import {{ serverRegisterDocumentAnalyzer }} from "clay:language";
                serverRegisterDocumentAnalyzer({{
                  analyzer: {analyzer}
                }});
                "#
            );
            let error = evaluate_as_package_with_ls_grant(
                &service,
                serde_json::json!({
                    "name": "@vendor/analysis",
                    "version": "0.1.0",
                    "type": "module",
                    "exports": { ".": "./dist/index.js" },
                    "clay": {
                        "apiPrefix": "analysis",
                        "entry": "./dist/index.js",
                        "permissions": ["parse-document"],
                        "capabilities": ["language-server"],
                        "modes": [],
                        "docs": "./docs/index.md",
                        "contributions": {
                            "languageServers": [{
                                "id": "analysis.server",
                                "executable": "/bin/true",
                                "args": []
                            }]
                        }
                    }
                }),
                vec![crate::packages::permissions::PackagePermission::ParseDocument],
                Some((
                    "analysis.server",
                    std::fs::canonicalize("/bin/true").expect("canonical /bin/true"),
                )),
                &source,
            )
            .await
            .unwrap_err();
            assert!(
                error.to_string().contains("language.invalid_analyzer"),
                "unexpected analyzer registration error: {error}"
            );
        }
    }

    #[tokio::test]
    async fn language_intelligence_facade_rejects_executable_and_process_fields() {
        let service = ClayJsRuntimeService::default();
        for source in [
            r#"
            import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
            serverRegisterLanguageIntelligenceProvider({
              packageName: "@org/intel",
              packagePrefix: "intel",
              permissions: ["parse-document"],
              id: "intel.intelligence",
              features: ["hover"],
              handler: () => {}
            });
            "#,
            r#"
            import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
            serverRegisterLanguageIntelligenceProvider({
              packageName: "@org/intel",
              packagePrefix: "intel",
              permissions: ["parse-document"],
              id: "intel.intelligence",
              features: ["hover"],
              languageServer: true
            });
            "#,
        ] {
            let error = service
                .evaluate_controlled_module_for_document(source, 88)
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("language.invalid_provider"),
                "unexpected language registration error: {error}"
            );
        }
    }

    #[tokio::test]
    async fn language_intelligence_js_bridge_publishes_validated_hover_result() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@org/intel",
                "intel",
                &["parse-document"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::ParseDocument],
            r#"
                import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
                serverRegisterLanguageIntelligenceProvider({
                  provider: {
                    id: "intel.intelligence",
                    modes: ["intel"],
                    features: ["hover"],
                    priority: 10,
                    timeoutMs: 500
                  },
                  module: {
                    provideLanguageIntelligence: async (request, window) => ({
                      status: "ok",
                      markdown: `hover:${request.feature}:${window.text}`,
                      range: { byteStart: 0, byteEnd: window.text.length }
                    })
                  }
                });
                "#,
        )
        .await
        .unwrap();
        assert_eq!(evaluation.js_language_intelligence_providers.len(), 1);

        let coordinator =
            crate::server::language_intelligence::LanguageIntelligenceCoordinator::new();
        service
            .register_language_intelligence_providers(&coordinator, 1, &evaluation)
            .unwrap();

        let request = crate::protocol::LanguageIntelligenceRequest {
            request_id: 7,
            client_id: 1,
            document_id: 1,
            document_version: 1,
            behavior_version: 1,
            cursor_byte_offset: 0,
            feature: crate::protocol::LanguageIntelligenceFeature::Hover,
            provider_generation: 1,
        };
        let window = crate::server::language_intelligence::LanguageIntelligenceDocumentWindow {
            document_id: 1,
            document_version: 1,
            behavior_version: 1,
            byte_start: 0,
            byte_end: 4,
            text: "fn()".to_string(),
            active_mode: "intel".to_string(),
        };
        let reply_rx = coordinator
            .schedule(Some("intel.intelligence"), request.clone(), window)
            .unwrap();
        let result = reply_rx.await.expect("js hover result");
        crate::server::language_intelligence::validate_result(&result).unwrap();
        assert_eq!(
            result.status,
            crate::protocol::LanguageIntelligenceStatus::Ok
        );
        assert_eq!(result.request_id, 7);
        assert_eq!(result.provenance.package_prefix, "intel");
        match result.payload {
            crate::protocol::LanguageIntelligencePayload::Hover(hover) => {
                assert_eq!(hover.markdown, "hover:hover:fn()");
                assert_eq!(hover.range, Some(crate::protocol::TextByteRange::new(0, 4)));
            }
            other => panic!("expected hover payload, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn completion_facade_disables_provider_and_filters_trigger_listing() {
        let service = ClayJsRuntimeService::default();
        for (name, prefix) in [("@vendor/words", "words"), ("@vendor/other", "other")] {
            evaluate_as_package(
                &service,
                test_package_json(
                    name,
                    prefix,
                    &["completion-provider"],
                    serde_json::json!({
                        "completionProviders": [{
                            "id": format!("{prefix}.buffer"),
                            "triggerCharacters": ["."]
                        }]
                    }),
                ),
                vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({});
                "#,
            )
            .await
            .unwrap();
        }
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { serverDisableCompletion, serverListCompletionProvidersForTrigger } from "clay:completion";
                const disabled = serverDisableCompletion({ provider: "words.buffer" });
                const repeated = serverDisableCompletion({ provider: "words.buffer" });
                const packageDisabled = serverDisableCompletion({ packagePrefix: "@vendor/other" });
                const listed = serverListCompletionProvidersForTrigger({ trigger: "." });
                Deno.core.ops.op_clay_runtime_record(`${disabled.target}:${disabled.disabled}:${disabled.providerGeneration}:${repeated.disabled}:${packageDisabled.providerGeneration}:${listed.providers.length}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(evaluation.op_records, vec!["words.buffer:true:1:false:2:0"]);
        assert!(evaluation.completion_providers.is_empty());
    }

    #[tokio::test]
    async fn completion_disable_facade_rejects_empty_ambiguous_and_authority_fields() {
        let service = ClayJsRuntimeService::default();
        for source in [
            r#"
            import { serverDisableCompletion } from "clay:completion";
            serverDisableCompletion({});
            "#,
            r#"
            import { serverDisableCompletion } from "clay:completion";
            serverDisableCompletion({ provider: "words.buffer", packagePrefix: "words" });
            "#,
            r#"
            import { serverDisableCompletion } from "clay:completion";
            serverDisableCompletion({ provider: "words.buffer", handler() {} });
            "#,
            r#"
            import { serverDisableCompletion } from "clay:completion";
            serverDisableCompletion({ provider: "x".repeat(129) });
            "#,
        ] {
            let error = service
                .evaluate_controlled_module_for_document(source, 88)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("completion.invalid_disable"));
        }
    }

    #[tokio::test]
    async fn completion_facade_rejects_callbacks_missing_permission_and_bad_prefix() {
        let service = ClayJsRuntimeService::default();
        // Executable callback fields are rejected by the facade before any op.
        let error = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({ handler() {} });
                "#,
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("completion.invalid_provider"),
            "unexpected completion registration error: {error}"
        );
        // Approved-capability check: enable with the capability granted, then
        // shrink the authorization record; the enabled package's registration
        // now fails closed against the current approved set.
        let package_json = test_package_json(
            "@vendor/nope",
            "nope",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{ "id": "nope.words" }]
            }),
        );
        let root = config_fixture("package-provenance");
        let record = {
            let op_state = service.test_op_state();
            let mut locked = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            locked
                .install_from_value_at_root_with_spec(package_json, root, "local:provenance-test")
                .expect("synthetic package installs");
            locked
                .authorize_package(
                    "@vendor/nope",
                    vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .expect("synthetic package authorizes");
            locked
                .approve_package("@vendor/nope", "test")
                .expect("approves");
            locked.enable("@vendor/nope").expect("enables");
            locked
                .authorize_package(
                    "@vendor/nope",
                    vec![],
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .expect("capability shrink authorizes");
            crate::packages::record::assemble_package_record(&serde_json::json!({
                "name": "@vendor/nope",
                "version": "0.1.0",
                "type": "module",
                "exports": { ".": "./dist/index.js" },
                "clay": {
                    "apiPrefix": "nope",
                    "entry": "./dist/index.js",
                    "permissions": ["completion-provider"],
                    "modes": ["nope"],
                    "docs": "./docs/index.md",
                    "contributions": { "completionProviders": [{ "id": "nope.words" }] },
                }
            }))
            .expect("record assembles")
        };
        let error = service
            .evaluate_entry_as_package(
                crate::packages::bundled::RuntimeDomain::Trusted,
                &record,
                RuntimeEntry::ControlledSource(
                    r#"
                    import { serverRegisterCompletionProvider } from "clay:completion";
                    serverRegisterCompletionProvider({});
                    "#
                    .to_string(),
                ),
                "runtime.evaluate_as_package",
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("packages.missing_permission")
                && error.to_string().contains("completion-provider"),
            "unexpected completion registration error: {error}"
        );
        // Provider ids outside the host package prefix are rejected at
        // host-side manifest validation, before any package code runs.
        let error = crate::packages::record::assemble_package_record(&test_package_json(
            "@vendor/bad",
            "bad",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{ "id": "other.words" }]
            }),
        ))
        .unwrap_err();
        assert!(
            error.message.contains("apiPrefix"),
            "unexpected manifest validation error: {error:?}"
        );
    }

    #[tokio::test]
    async fn language_package_completion_trigger_metadata_is_queryable() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r##"
                import { loadPackage } from "clay:packages";
                import { serverDisableCompletion, serverListCompletionProvidersForTrigger } from "clay:completion";

                await loadPackage("@clay/rust");
                await loadPackage("@clay/typescript");
                await loadPackage("@clay/javascript");
                await loadPackage("@clay/markdown");

                const dotProviders = serverListCompletionProvidersForTrigger({ trigger: "." });
                const rustScopeProviders = serverListCompletionProvidersForTrigger({ trigger: ":" });
                const markdownProviders = serverListCompletionProvidersForTrigger({ trigger: "#" });
                const noProviders = serverListCompletionProvidersForTrigger({ trigger: "?" });
                serverDisableCompletion({ packagePrefix: "@clay/rust" });
                const dotProvidersAfterRustDisable = serverListCompletionProvidersForTrigger({ trigger: "." });

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                  dotIds: dotProviders.providers.map((p) => p.id).sort(),
                  rustScopeIds: rustScopeProviders.providers.map((p) => p.id).sort(),
                  noCount: noProviders.providers.length,
                  rustIdsAfterDisable: dotProvidersAfterRustDisable.providers.filter((p) => p.packagePrefix === "rust").map((p) => p.id),
                  rustTriggerCharacters: dotProviders.providers.find((p) => p.id === "rust.keywords")?.triggerCharacters ?? [],
                  typescriptTriggerCharacters: dotProviders.providers.find((p) => p.id === "typescript.keywords")?.triggerCharacters ?? [],
                  rustPriority: dotProviders.providers.find((p) => p.id === "rust.keywords")?.priority,
                  rustItems: dotProviders.providers.find((p) => p.id === "rust.keywords")?.items ?? [],
                  rustSnippetItems: dotProviders.providers.find((p) => p.id === "rust.snippets")?.items ?? [],
                  typescriptSnippetItems: dotProviders.providers.find((p) => p.id === "typescript.snippets")?.items ?? [],
                  markdownIds: markdownProviders.providers.map((p) => p.id),
                  markdownItems: markdownProviders.providers.find((p) => p.id === "markdown.keywords")?.items ?? [],
                }));
                "##,
            )
            .await
            .unwrap();

        assert!(
            evaluation
                .completion_providers
                .iter()
                .all(|provider| provider.provenance.package_prefix != "rust")
        );
        let record = evaluation
            .op_records
            .into_iter()
            .next()
            .expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(
            parsed["dotIds"],
            serde_json::json!([
                "javascript.keywords",
                "rust.keywords",
                "rust.snippets",
                "typescript.keywords",
                "typescript.snippets"
            ])
        );
        assert_eq!(
            parsed["rustScopeIds"],
            serde_json::json!(["rust.keywords", "rust.snippets"])
        );
        assert_eq!(parsed["noCount"], 0);
        assert_eq!(parsed["rustIdsAfterDisable"], serde_json::json!([]));
        assert_eq!(
            parsed["rustTriggerCharacters"],
            serde_json::json!([".", ":"])
        );
        assert_eq!(
            parsed["typescriptTriggerCharacters"],
            serde_json::json!(["."])
        );
        assert_eq!(parsed["rustPriority"], 0);
        assert!(parsed["rustItems"].as_array().unwrap().iter().any(|item| {
            item["label"] == "fn" && item["insertText"] == "fn" && item["textFormat"] == "plainText"
        }));
        assert!(
            parsed["rustSnippetItems"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "fn" && item["textFormat"] == "snippet")
        );
        assert!(
            parsed["typescriptSnippetItems"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "interface" && item["textFormat"] == "snippet")
        );
        assert_eq!(
            parsed["markdownIds"],
            serde_json::json!(["markdown.keywords"])
        );
        assert!(
            parsed["markdownItems"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "# " && item["textFormat"] == "plainText")
        );
    }

    #[tokio::test]
    async fn load_package_registers_first_party_syntax_grammars() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                await loadPackage("@clay/rust");
                await loadPackage("@clay/typescript");
                await loadPackage("@clay/javascript");
                "#,
            )
            .await
            .unwrap();

        let languages = evaluation
            .syntax_grammars
            .iter()
            .map(|grammar| (grammar.language_id.as_str(), grammar.engine_tier))
            .collect::<Vec<_>>();
        assert_eq!(
            languages,
            vec![
                (
                    "javascript",
                    crate::server::syntax::SyntaxEngineTier::Native
                ),
                ("markdown", crate::server::syntax::SyntaxEngineTier::Native),
                ("rust", crate::server::syntax::SyntaxEngineTier::Native),
                ("tsx", crate::server::syntax::SyntaxEngineTier::Native),
                (
                    "typescript",
                    crate::server::syntax::SyntaxEngineTier::Native
                ),
            ]
        );
    }

    #[tokio::test]
    async fn lsp_rust_package_loads_after_exact_grant_without_starting_child() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        if crate::packages::authorization::resolve_language_server_executable("rustup").is_none() {
            return;
        }
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/configuration/lsp-rust");
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp/rust");
        let mut workspace = WorkspaceState::new();
        let registered = workspace.add_root(&workspace_root).unwrap();
        assert_eq!(registered, 1);

        let runtime = ClayJsRuntimeService::default();
        let evaluation = runtime
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        assert_eq!(evaluation.document_analyzers.len(), 1);
        let analyzer = &evaluation.document_analyzers[0];
        assert_eq!(analyzer.package.manifest.name, "@clay/lsp-rust");
        assert_eq!(analyzer.id, "lsp-rust.bridge");
        assert_eq!(analyzer.contribution, "lsp-rust.server");
        assert_eq!(analyzer.modes, ["rust"]);
        assert_eq!(
            evaluation.op_records,
            ["@clay/lsp-rust:1"],
            "load must register metadata only; analyzer starts lazily on document open"
        );

        let invocation = runtime
            .evaluate_controlled_module(
                r#"
                import { createRustAnalyzerBridge } from "clay://packages/@clay/lsp-rust/dist/server.js";
                import { FrameDecoder, encodeFrame } from "clay://packages/@clay/lsp-rust/dist/shared/framing.js";
                const decoder = new FrameDecoder();
                const reads = [];
                const methods = [];
                const session = {
                  async sendBytes(bytes) {
                    for (const message of decoder.push(bytes)) {
                      methods.push(message.method);
                      if (message.method === "initialize") reads.push(encodeFrame({
                        jsonrpc: "2.0",
                        id: message.id,
                        result: { capabilities: { textDocumentSync: { openClose: true, change: 2 } } },
                      }));
                    }
                  },
                  async readBytes() { return reads.shift() ?? new Uint8Array(); },
                  async stop() {},
                };
                const bridge = createRustAnalyzerBridge({
                  startSession: async () => session,
                  publishDecorations() {},
                  publishDiagnostics() {},
                  packageManifest: {},
                });
                await bridge.handle({
                  kind: "open",
                  identity: { package: "@clay/lsp-rust", contribution: "lsp-rust.server" },
                  documentId: 7,
                  documentVersion: 1,
                  workspaceRootId: 1,
                  canonicalRootPath: "/tmp",
                  relativePath: "main.rs",
                  text: "fn main() {}\n",
                });
                Deno.core.ops.op_clay_runtime_record(methods.join(","));
                "#,
            )
            .await
            .unwrap();
        assert_eq!(
            invocation.op_records,
            ["initialize,initialized,textDocument/didOpen"]
        );
    }

    #[tokio::test]
    async fn lsp_typescript_and_javascript_packages_load_after_exact_grants_without_starting_children()
     {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        if crate::packages::authorization::resolve_language_server_executable(
            "typescript-language-server",
        )
        .is_none()
        {
            return;
        }

        let mut typescript_runtime = None;
        for (config_dir, workspace_dir, package_name, analyzer_id, contribution, mode) in [
            (
                "tests/fixtures/configuration/lsp-typescript",
                "tests/fixtures/lsp/typescript",
                "@clay/lsp-typescript",
                "lsp-typescript.bridge",
                "lsp-typescript.server",
                "typescript",
            ),
            (
                "tests/fixtures/configuration/lsp-javascript",
                "tests/fixtures/lsp/javascript",
                "@clay/lsp-javascript",
                "lsp-javascript.bridge",
                "lsp-javascript.server",
                "javascript",
            ),
        ] {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(config_dir);
            let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(workspace_dir);
            let mut workspace = WorkspaceState::new();
            let registered = workspace.add_root(&workspace_root).unwrap();
            assert_eq!(registered, 1);

            let runtime = ClayJsRuntimeService::default();
            let evaluation = runtime
                .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
                .await
                .unwrap();

            assert_eq!(evaluation.document_analyzers.len(), 1);
            let analyzer = &evaluation.document_analyzers[0];
            assert_eq!(analyzer.package.manifest.name, package_name);
            assert_eq!(analyzer.id, analyzer_id);
            assert_eq!(analyzer.contribution, contribution);
            assert_eq!(analyzer.modes, [mode]);
            assert_eq!(
                evaluation.op_records,
                [format!("{package_name}:1")],
                "load must register metadata only; analyzer starts lazily on document open"
            );
            if package_name == "@clay/lsp-typescript" {
                typescript_runtime = Some(runtime);
            }
        }

        let runtime = typescript_runtime.expect("typescript bridge runtime loaded");
        let invocation = runtime
            .evaluate_controlled_module(
                r#"
                import { createTypescriptBridge } from "clay://packages/@clay/lsp-typescript/dist/server.js";
                import { FrameDecoder, encodeFrame } from "clay://packages/@clay/lsp-typescript/dist/shared/framing.js";
                const decoder = new FrameDecoder();
                const reads = [];
                const methods = [];
                const session = {
                  async sendBytes(bytes) {
                    for (const message of decoder.push(bytes)) {
                      methods.push(message.method);
                      if (message.method === "initialize") reads.push(encodeFrame({
                        jsonrpc: "2.0",
                        id: message.id,
                        result: { capabilities: { textDocumentSync: { openClose: true, change: 2 } } },
                      }));
                    }
                  },
                  async readBytes() { return reads.shift() ?? new Uint8Array(); },
                  async stop() {},
                };
                const bridge = createTypescriptBridge({
                  startSession: async () => session,
                  publishDecorations() {},
                  publishDiagnostics() {},
                  packageManifest: {},
                });
                await bridge.handle({
                  kind: "open",
                  identity: { package: "@clay/lsp-typescript", contribution: "lsp-typescript.server" },
                  documentId: 7,
                  documentVersion: 1,
                  workspaceRootId: 1,
                  canonicalRootPath: "/tmp",
                  relativePath: "main.ts",
                  text: "export const value = 1;\n",
                });
                Deno.core.ops.op_clay_runtime_record(methods.join(","));
                "#,
            )
            .await
            .unwrap();
        assert_eq!(
            invocation.op_records,
            ["initialize,initialized,textDocument/didOpen"]
        );
    }

    #[tokio::test]
    async fn lsp_markdown_package_loads_after_exact_grant_without_starting_child() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        if crate::packages::authorization::resolve_language_server_executable("marksman").is_none()
        {
            return;
        }
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/configuration/lsp-markdown");
        let workspace_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp/markdown");
        let mut workspace = WorkspaceState::new();
        let registered = workspace.add_root(&workspace_root).unwrap();
        assert_eq!(registered, 1);

        let runtime = ClayJsRuntimeService::default();
        let evaluation = runtime
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        assert_eq!(evaluation.document_analyzers.len(), 1);
        let analyzer = &evaluation.document_analyzers[0];
        assert_eq!(analyzer.package.manifest.name, "@clay/lsp-markdown");
        assert_eq!(analyzer.id, "lsp-markdown.bridge");
        assert_eq!(analyzer.contribution, "lsp-markdown.server");
        assert_eq!(analyzer.modes, ["markdown"]);
        assert_eq!(
            evaluation.op_records,
            ["@clay/lsp-markdown:1"],
            "load must register metadata only; analyzer starts lazily on document open"
        );

        let invocation = runtime
            .evaluate_controlled_module(
                r##"
                import { createMarksmanBridge } from "clay://packages/@clay/lsp-markdown/dist/server.js";
                import { FrameDecoder, encodeFrame } from "clay://packages/@clay/lsp-markdown/dist/shared/framing.js";
                const decoder = new FrameDecoder();
                const reads = [];
                const methods = [];
                const session = {
                  async sendBytes(bytes) {
                    for (const message of decoder.push(bytes)) {
                      methods.push(message.method);
                      if (message.method === "initialize") reads.push(encodeFrame({
                        jsonrpc: "2.0",
                        id: message.id,
                        result: { capabilities: { textDocumentSync: { openClose: true, change: 1 } } },
                      }));
                    }
                  },
                  async readBytes() { return reads.shift() ?? new Uint8Array(); },
                  async stop() {},
                };
                const bridge = createMarksmanBridge({
                  startSession: async () => session,
                  publishDecorations() {},
                  publishDiagnostics() {},
                  packageManifest: {},
                });
                await bridge.handle({
                  kind: "open",
                  identity: { package: "@clay/lsp-markdown", contribution: "lsp-markdown.server" },
                  documentId: 7,
                  documentVersion: 1,
                  workspaceRootId: 1,
                  canonicalRootPath: "/tmp",
                  relativePath: "README.md",
                  text: "# Title\n",
                });
                Deno.core.ops.op_clay_runtime_record(methods.join(","));
                "##,
            )
            .await
            .unwrap();
        assert_eq!(
            invocation.op_records,
            ["initialize,initialized,textDocument/didOpen"]
        );
    }

    #[tokio::test]
    async fn lsp_language_packages_fixture_grants_before_load_without_starting_children() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let required = ["rustup", "typescript-language-server", "marksman"];
        if required.iter().any(|executable| {
            crate::packages::authorization::resolve_language_server_executable(executable).is_none()
        }) {
            return;
        }

        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/configuration/lsp-language-packages");
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lsp/rust");
        let mut workspace = WorkspaceState::new();
        assert_eq!(workspace.add_root(&workspace_root).unwrap(), 1);

        let evaluation = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        let analyzer_names: Vec<_> = evaluation
            .document_analyzers
            .iter()
            .map(|analyzer| analyzer.package.manifest.name.as_str())
            .collect();
        assert_eq!(
            analyzer_names,
            [
                "@clay/lsp-rust",
                "@clay/lsp-typescript",
                "@clay/lsp-javascript",
                "@clay/lsp-markdown",
            ]
        );
        assert!(
            evaluation.op_records.is_empty(),
            "representative fixture must register only; it must not start language-server children: {:?}",
            evaluation.op_records
        );

        let completion_ids: Vec<_> = evaluation
            .completion_providers
            .iter()
            .map(|provider| provider.id.as_str())
            .collect();
        for required_id in [
            "rust.keywords",
            "typescript.keywords",
            "javascript.keywords",
            "markdown.keywords",
        ] {
            assert!(
                completion_ids.contains(&required_id),
                "fixture must register base provider {required_id}; got {completion_ids:?}"
            );
        }
        assert!(
            completion_ids.iter().all(|id| !id.starts_with("lsp-")),
            "LSP completion providers register lazily through analyzers; load must not eagerly start them: {completion_ids:?}"
        );

        for package in [
            "lsp-rust",
            "lsp-typescript",
            "lsp-javascript",
            "lsp-markdown",
        ] {
            let manifest: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(format!(
                    "{}/packages/{package}/package.json",
                    env!("CARGO_MANIFEST_DIR")
                ))
                .unwrap(),
            )
            .unwrap();
            let provider = &manifest["clay"]["contributions"]["completionProviders"][0];
            assert_eq!(provider["priority"].as_i64(), Some(100));
            assert!(
                provider.get("exclusive").is_none()
                    || provider["exclusive"] == serde_json::Value::Bool(false)
            );
        }
    }

    #[tokio::test]
    async fn js_parse_handler_timeout_uses_registered_budget() {
        let service = ClayJsRuntimeService::default();
        let evaluation = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/loop",
                "loop",
                &["parse-document"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::ParseDocument],
            r#"
                import { serverRegisterParseHandler } from "clay:parse";
                const parser = { parse() { while (true) {} } };
                serverRegisterParseHandler({
                  mode: "loop",
                  parseUnit: "line-group",
                  timeoutMs: 50,
                  module: parser,
                  exportName: "parse"
                });
                "#,
        )
        .await
        .expect("malicious handler registration itself should be bounded metadata work");
        let registration = evaluation
            .js_parse_handlers
            .first()
            .expect("handler registered")
            .clone();
        let notification = ParseEditNotification {
            document_id: 1,
            document_version: 1,
            behavior_version: 1,
            package_prefix: "loop".to_string(),
            mode_id: "loop".to_string(),
            viewport: ParseByteRange::new(0, 4),
            invalidated_ranges: vec![ParseByteRange::new(0, 4)],
            accepted_edit: None,
            parse_windows: Vec::new(),
            memory_budget: None,
        };
        let started = std::time::Instant::now();
        let error = service
            .invoke_parse_handler(registration, notification)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Timeout));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "registered handler timeout budget should beat global 5s guard"
        );
        assert_eq!(error.diagnostic().code, "runtime.timeout");
    }

    #[tokio::test]
    async fn runtime_boundary_does_not_expose_platform_authorities() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                const exposed = [
                  ["fetch", typeof fetch],
                  ["WebSocket", typeof WebSocket],
                  ["Worker", typeof Worker],
                  ["process", typeof process],
                  ["require", typeof require],
                  ["Deno.readTextFile", typeof Deno.readTextFile],
                  ["Deno.Command", typeof Deno.Command],
                ].filter(([, type]) => type !== "undefined");
                Deno.core.ops.op_clay_runtime_record(JSON.stringify(exposed));
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["[]"]);
    }

    #[tokio::test]
    async fn js_runtime_infinite_loop_is_terminated_with_timeout() {
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
        let start = std::time::Instant::now();
        let error = service
            .evaluate_controlled_module(r#"while (true) {}"#)
            .await
            .unwrap_err();
        let elapsed = start.elapsed();

        assert!(
            matches!(error, ClayRuntimeError::Timeout),
            "expected Timeout, got {error:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1000),
            "timeout test should finish quickly, took {elapsed:?}"
        );
        assert_eq!(
            error.diagnostic().code,
            "runtime.timeout",
            "timeout should surface the runtime.timeout diagnostic"
        );
        // Timed-out evaluations are not counted as successful completions.
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn js_runtime_timeout_recovery_uses_fresh_worker() {
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
        let error = service
            .evaluate_controlled_module(
                r#"
                globalThis.__clayRecoveryMarker = "stale";
                while (true) {}
                "#,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ClayRuntimeError::Timeout));

        let result = service
            .evaluate_controlled_module(
                r#"
                Deno.core.ops.op_clay_runtime_record(typeof globalThis.__clayRecoveryMarker);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["undefined"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic() {
        let service = ClayJsRuntimeService::with_timeout_and_heap_limit(
            Duration::from_secs(3),
            8 * 1024 * 1024,
        );
        let error = service
            .evaluate_controlled_module(
                r#"
                const values = [];
                while (true) {
                  values.push({ text: "Hello", number: values.length });
                }
                "#,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(error, ClayRuntimeError::HeapLimit),
            "expected heap limit, got {error:?}"
        );
        assert_eq!(error.diagnostic().code, "runtime.heap_limit");
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn js_runtime_heap_limit_recovery_uses_fresh_worker() {
        let service = ClayJsRuntimeService::with_timeout_and_heap_limit(
            Duration::from_secs(3),
            8 * 1024 * 1024,
        );
        let error = service
            .evaluate_controlled_module(
                r#"
                globalThis.__clayRecoveryMarker = "stale";
                const values = [];
                while (true) {
                  values.push({ text: "Hello", number: values.length });
                }
                "#,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ClayRuntimeError::HeapLimit));

        let result = service
            .evaluate_controlled_module(
                r#"
                Deno.core.ops.op_clay_runtime_record(typeof globalThis.__clayRecoveryMarker);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["undefined"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn js_runtime_short_timeout_does_not_break_fast_evaluation() {
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
        let result = service
            .evaluate_controlled_module(
                r#"
                Deno.core.ops.op_clay_runtime_record("fast");
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["fast"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn js_runtime_rejects_unsafe_or_unknown_imports() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(r#"import "https://example.invalid/module.js";"#)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("runtime.invalid_import"));
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn configuration_runtime_loads_init_js_fixture() {
        let root = config_fixture("init");
        fs::write(
            root.join("init.js"),
            r#"Deno.core.ops.op_clay_runtime_record("init-loaded");"#,
        )
        .unwrap();

        let service = ClayJsRuntimeService::default();
        let result = service.load_configuration_from_root(root).await.unwrap();

        assert_eq!(result.op_records, vec!["init-loaded"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn configuration_runtime_loads_relative_module() {
        let root = config_fixture("relative");
        fs::write(
            root.join("init.js"),
            r#"
            import { getConfigurationState, loadConfigurationModule } from "clay:configuration";
            await loadConfigurationModule({ path: "./ui.js" });
            const state = getConfigurationState();
            Deno.core.ops.op_clay_runtime_record(state.entryPoint);
            Deno.core.ops.op_clay_runtime_record(state.loadedModules.join(","));
            "#,
        )
        .unwrap();
        fs::write(
            root.join("ui.js"),
            r#"Deno.core.ops.op_clay_runtime_record("ui-loaded");"#,
        )
        .unwrap();

        let service = ClayJsRuntimeService::default();
        let result = service.load_configuration_from_root(root).await.unwrap();

        assert_eq!(result.op_records, vec!["ui-loaded", "./init.js", "./ui.js"]);
    }

    #[tokio::test]
    async fn configuration_optional_module_failure_isolated_and_reported() {
        let root = config_fixture("optional-syntax");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadConfigurationModule } from "clay:configuration";
            const result = await loadConfigurationModule({ path: "./broken.js", optional: true });
            Deno.core.ops.op_clay_runtime_record(`${result.loaded}:${typeof result.error}`);
            Deno.core.ops.op_clay_runtime_record("after");
            "#,
        )
        .unwrap();
        fs::write(root.join("broken.js"), "export const = ;").unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("optional syntax failure must not fail configuration evaluation");

        assert_eq!(result.op_records, vec!["false:string", "after"]);
        let diagnostic = result
            .configuration_diagnostics
            .first()
            .expect("optional module failure diagnostic");
        assert_eq!(diagnostic.code, "configuration.module_failed");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
        assert!(diagnostic.message.contains("./broken.js"));
    }

    #[tokio::test]
    async fn configuration_optional_missing_module_failure_isolated_and_reported() {
        let root = config_fixture("optional-missing");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadConfigurationModule } from "clay:configuration";
            const result = await loadConfigurationModule({ path: "./missing.js", optional: true });
            Deno.core.ops.op_clay_runtime_record(`${result.loaded}:${typeof result.error}`);
            Deno.core.ops.op_clay_runtime_record("after");
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("optional missing module must not fail configuration evaluation");

        assert_eq!(result.op_records, vec!["false:string", "after"]);
        let diagnostic = result
            .configuration_diagnostics
            .first()
            .expect("optional missing module diagnostic");
        assert_eq!(diagnostic.code, "configuration.module_failed");
        assert!(diagnostic.message.contains("./missing.js"));
    }

    #[tokio::test]
    async fn configuration_required_module_failure_still_fails_evaluation() {
        let root = config_fixture("required-module");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadConfigurationModule } from "clay:configuration";
            await loadConfigurationModule({ path: "./broken.js" });
            "#,
        )
        .unwrap();
        fs::write(root.join("broken.js"), "export const = ;").unwrap();

        let error = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect_err("required module failure must preserve fail-evaluation behavior");

        assert!(error.to_string().contains("SyntaxError"));
    }

    #[tokio::test]
    async fn configuration_optional_module_path_escape_still_fails_before_catch() {
        let parent = config_fixture("optional-escape");
        let root = parent.join("config");
        fs::create_dir(&root).unwrap();
        fs::write(parent.join("outside.js"), "export const outside = true;").unwrap();
        fs::write(
            root.join("init.js"),
            r#"
            import { loadConfigurationModule } from "clay:configuration";
            await loadConfigurationModule({ path: "../outside.js", optional: true });
            "#,
        )
        .unwrap();

        let error = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect_err("optional path escape must remain a hard failure");

        assert!(error.to_string().contains("configuration.invalid_module"));
    }

    #[tokio::test]
    async fn runtime_imports_clay_sdui_facade() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { definePanel } from "clay:sdui";
                const panel = definePanel({ id: "root", title: "Runtime", children: [] });
                Deno.core.ops.op_clay_runtime_record(`${panel.kind}:${panel.title}:${panel.id}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["panel:Runtime:root"]);
    }

    #[tokio::test]
    async fn runtime_imports_clay_ui_facade_and_registers_contributions() {
        let service = ClayJsRuntimeService::default();
        let result = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/markdown-ui",
                "markdown",
                &["command-registration"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::CommandRegistration],
            r#"
                import { serverRegisterCommand } from "clay:commands";
                import {
                  serverRegisterComponentContribution,
                  serverRegisterPanelContribution,
                  serverRegisterThemeToken,
                  serverRegisterTransientOverlayContribution,
                } from "clay:ui";

                serverRegisterCommand({
                  commandId: "markdown.togglePreview",
                  displayName: "Toggle Markdown Preview",
                  routingPolicy: "server-first",
                });
                const token = serverRegisterThemeToken({
                  token: "markdown.preview.background",
                  type: "color-role",
                  fallback: "surface.panel",
                  description: "Markdown preview background",
                });
                const component = serverRegisterComponentContribution({
                  kind: "label",
                  id: "markdown.preview.empty",
                  text: "Preview unavailable",
                });
                const panel = serverRegisterPanelContribution({
                  id: "markdown.preview",
                  slot: "right",
                  kind: "fixed",
                  defaultVisibility: "hidden",
                  actionTargets: ["markdown.togglePreview"],
                  component: {
                    kind: "panel",
                    id: "markdown.preview.root",
                    title: "Preview",
                    children: [{
                      kind: "button",
                      id: "markdown.preview.toggle",
                      label: "Toggle",
                      action: { commandId: "markdown.togglePreview" },
                    }],
                  },
                });
                const overlay = serverRegisterTransientOverlayContribution({
                  id: "markdown.preview.overlay",
                  anchor: "working-area",
                  focusPolicy: "restore",
                  dismissalPolicy: "escape",
                  component: { kind: "panel", id: "markdown.preview.overlay.root", title: "Overlay", children: [] },
                });
                Deno.core.ops.op_clay_runtime_record(`${panel.slot}:${component.rootKind}:${overlay.focusPolicy}:${token.type}:${panel.provenance.apiPrefix}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(
            result.op_records,
            vec!["right:label:restore:color-role:markdown"]
        );
        assert_eq!(result.ui_contributions.panels.len(), 1);
        assert_eq!(result.ui_contributions.components.len(), 1);
        assert_eq!(result.ui_contributions.overlays.len(), 1);
        assert_eq!(result.ui_contributions.theme_tokens.len(), 1);
        assert_eq!(
            result.ui_contributions.panels[0].provenance.package_name,
            "@clay/markdown-ui"
        );
    }

    #[tokio::test]
    async fn runtime_clay_ui_rejects_invalid_prefix_unregistered_action_and_raw_css() {
        let service = ClayJsRuntimeService::default();
        let error = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/markdown-ui",
                "markdown",
                &["command-registration"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::CommandRegistration],
            r#"
                import { serverRegisterPanelContribution } from "clay:ui";
                serverRegisterPanelContribution({
                  id: "other.preview",
                  slot: "right",
                  rawCss: "color: red",
                  component: { kind: "button", id: "other.preview.button", label: "Run", action: { commandId: "markdown.missing" } },
                });
                "#,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("ui.registration_failed"));
    }

    #[tokio::test]
    async fn runtime_facades_do_not_require_raw_ops() {
        let root = config_fixture("facade-no-raw-ops");
        fs::write(
            root.join("init.js"),
            r#"
            import { defineLabel } from "clay:sdui";
            import { getConfigurationState } from "clay:configuration";
            const label = defineLabel({ text: "Ready" });
            const state = getConfigurationState();
            if (label.kind !== "label" || state.entryPoint !== "./init.js") {
              throw new Error("facade import failed");
            }
            "#,
        )
        .unwrap();

        ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn unsupported_facade_returns_planned_error() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { serverGetDocumentSnapshot } from "clay:documents";
                await serverGetDocumentSnapshot("1");
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error
                .to_string()
                .contains("documents.serverGetDocumentSnapshot is planned")
        );
    }

    #[tokio::test]
    async fn facade_op_mapping_matches_inventory() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { loadConfigurationModule } from "clay:configuration";
                import { defineStack } from "clay:sdui";
                const stack = defineStack({ children: [] });
                Deno.core.ops.op_clay_runtime_record(`${typeof loadConfigurationModule}:${stack.kind}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["function:stack"]);
    }

    #[tokio::test]
    async fn smoke_config_fixture_publishes_runtime_sdui_snapshot() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("runtime-sdui");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
        let tree = result.published_sdui_tree.expect("published SDUI tree");

        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Panel { title, .. } if title == "Runtime Smoke Workspace"
        )));
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::EditorView { binding }
                if binding.document_id == 1 && binding.expected_version == Some(1)
        )));
    }

    #[tokio::test]
    async fn markdown_config_fixture_opens_workspace_without_default_panel() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("markdown-mode");
        let workspace_root = root.join("workspace");
        let mut workspace = WorkspaceState::new();
        workspace
            .add_root(&workspace_root)
            .expect("markdown workspace fixture root must register");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        // Phase 20 task 4: the fixture uses the default load path and publishes
        // NO default side panel — only behavior/decorations state. The optional
        // preview is a package PanelContribution, validated separately by
        // `markdown_optional_preview_is_valid_panel_contribution`.
        assert!(
            result.published_sdui_tree.is_none(),
            "markdown-mode fixture must not publish a default side panel SDUI tree"
        );
        assert_eq!(result.parse_handlers.len(), 1);
        assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
        // Decorations publish only through package callbacks now; the fixture
        // (configuration code) no longer publishes directly.
        assert!(result.published_decoration_set.is_none());
        let manifest = result
            .behavior_manifest
            .expect("markdown behavior manifest");
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview")
        );
    }

    #[tokio::test]
    async fn windows_markdown_open_config_fixture_loads_without_default_panel() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("windows-markdown-open");
        let workspace_root = root.join("workspace");
        let mut workspace = WorkspaceState::new();
        workspace
            .add_root(&workspace_root)
            .expect("Windows Markdown open fixture root must register");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        // Phase 20 task 4: the fixture uses the default load path and publishes
        // NO default side panel — only behavior/decorations state.
        assert!(
            result.published_sdui_tree.is_none(),
            "windows-markdown-open fixture must not publish a default side panel SDUI tree"
        );
        assert_eq!(result.parse_handlers.len(), 1);
        assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
        // Decorations publish only through package callbacks now; the fixture
        // (configuration code) no longer publishes directly.
        assert!(result.published_decoration_set.is_none());
        let manifest = result
            .behavior_manifest
            .expect("Windows Markdown open behavior manifest");
        assert!(manifest.keymaps.iter().any(|rule| {
            rule.sequence
                == vec![crate::protocol::KeyStroke {
                    key: crate::protocol::KeyCode::Character("o".to_string()),
                    modifiers: crate::protocol::KeyModifiers {
                        control: true,
                        ..crate::protocol::KeyModifiers::NONE
                    },
                }]
                && rule.command_id == "documents.clientOpenFileDialog"
                && rule.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
        }));
        assert!(manifest.commands.iter().any(|command| {
            command.command_id == "documents.clientOpenFileDialog"
                && command.authority == crate::protocol::CommandAuthority::ClientUi
        }));
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview")
        );
    }

    #[tokio::test]
    async fn markdown_package_runtime_loads_markdown_it_workflow() {
        let root = config_fixture("markdown-package-runtime");
        for file_name in ["index.js", "load.js", "parser.js", "sdui.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r##"
            import * as commands from "clay:commands";
            import * as decorations from "clay:decorations";
            import * as modes from "clay:modes";
            import * as packages from "clay:packages";
            import * as parse from "clay:parse";
            import * as sdui from "clay:sdui";
            import { loadPackage } from "clay:packages";
            import { markdownPackageContract } from "./load.js";
            import { publishMarkdownDecorations } from "./parser.js";
            import { publishMarkdownPreviewStatus } from "./sdui.js";

            const clay = { commands, decorations, modes, packages, parse, sdui };
            // Load through the real loadPackage path (host-stamped provenance
            // for this evaluation), then drive the parser/sdui workflow.
            await loadPackage("@clay/markdown");
            const contract = markdownPackageContract();

            const text = "# Runtime package\n\n- item\n";
            const tokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Runtime package", children: [{ type: "text", content: "Runtime package" }] },
              { type: "heading_close" },
              { type: "bullet_list_open", map: [2, 3] },
              { type: "list_item_open", map: [2, 3] },
              { type: "inline", map: [2, 3], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "list_item_close" },
              { type: "bullet_list_close" },
            ];
            const update = await publishMarkdownDecorations(clay, {
              text,
              tokens,
              documentId: 1,
              documentVersion: 1,
              behaviorVersion: 2,
              viewport: { byteStart: 0, byteEnd: 64 },
            });
            await publishMarkdownPreviewStatus(clay, {
              documentId: 1,
              documentVersion: 1,
              documentPath: "sample.md",
            });
            Deno.core.ops.op_clay_runtime_record(`${contract.parse.adapter}:${contract.sdui.adapter}:${update.publishedSpanCount}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["./dist/parser.js:./dist/sdui.js:2"]);
        assert_eq!(result.parse_handlers.len(), 1);
        assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
        assert!(result.published_decoration_set.is_some());
        let tree = result.published_sdui_tree.expect("Markdown SDUI tree");
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Label { text } if text == "Parse: markdown-it registered"
        )));
        let manifest = result
            .behavior_manifest
            .expect("Markdown behavior manifest");
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview")
        );
    }

    #[tokio::test]
    async fn language_packages_config_fixture_loads_and_registers_all_contributions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("language-packages");

        let service = ClayJsRuntimeService::default();
        let result = service.load_configuration_from_root(root).await.unwrap();
        assert_eq!(
            service.completion_providers(),
            result.completion_providers,
            "runtime service must retain an inert Rust snapshot for completion requests"
        );

        for (provider_id, expected_item) in [
            ("rust.keywords", "fn"),
            ("typescript.keywords", "interface"),
            ("javascript.keywords", "function"),
            ("markdown.keywords", "# "),
        ] {
            let provider = result
                .completion_providers
                .iter()
                .find(|provider| provider.id == provider_id)
                .unwrap_or_else(|| panic!("fixture must register {provider_id}"));
            assert_eq!(provider.priority, 0);
            assert!(
                provider
                    .items
                    .iter()
                    .any(|item| item.label == expected_item),
                "{provider_id} must carry `{expected_item}` as inert text replacement data"
            );
            assert!(provider.items.len() <= provider.max_items);
        }

        let component_ids: Vec<_> = result
            .ui_contributions
            .components
            .iter()
            .map(|component| component.id.clone())
            .collect();
        assert!(
            component_ids.iter().any(|id| id == "rust.status.mode"),
            "fixture must register rust.status.mode status item"
        );
        assert!(
            component_ids
                .iter()
                .any(|id| id == "typescript.status.mode"),
            "fixture must register typescript.status.mode status item"
        );
        assert!(
            component_ids
                .iter()
                .any(|id| id == "javascript.status.mode"),
            "fixture must register javascript.status.mode status item"
        );
        assert!(
            component_ids.iter().any(|id| id == "markdown.status.mode"),
            "fixture must register markdown.status.mode status item"
        );

        let grammar_ids: Vec<_> = result
            .syntax_grammars
            .iter()
            .map(|grammar| grammar.language_id.clone())
            .collect();
        assert!(
            grammar_ids.iter().any(|id| id == "rust"),
            "fixture must register rust syntax grammar"
        );
        assert!(
            grammar_ids.iter().any(|id| id == "typescript"),
            "fixture must register typescript syntax grammar"
        );
        assert!(
            grammar_ids.iter().any(|id| id == "javascript"),
            "fixture must register javascript syntax grammar"
        );

        // Phase 18.18: the Markdown package also loads through a one-line
        // `loadPackage("@clay/markdown")` and registers its JS parse handler
        // (decoration/preview path) alongside the three code-language packages.
        assert!(
            grammar_ids.iter().any(|id| id == "markdown"),
            "fixture must register markdown syntax grammar"
        );
        assert_eq!(
            result.js_parse_handlers.len(),
            1,
            "fixture must register the Markdown parse handler"
        );
        assert_eq!(
            result.js_parse_handlers[0].package.manifest.name, "@clay/markdown",
            "Markdown parse handler must come from @clay/markdown"
        );
    }

    #[tokio::test]
    async fn first_party_language_packages_are_not_silent_defaults() {
        // No `loadPackage` call in init.js: no first-party package may register
        // its mode, commands, completion providers, parse handlers, or UI. The
        // compiled-in Tier 1 native grammars remain (engine capability, not
        // package activation); only an explicit `loadPackage("@clay/*")` opts a
        // package's contributions in.
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let root = config_fixture("no-silent-defaults");
        fs::write(
            root.join("init.js"),
            "// empty init.js: no language packages loaded\n",
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert!(
            result.completion_providers.is_empty(),
            "no completion provider may register without an explicit loadPackage"
        );
        assert!(
            result.js_parse_handlers.is_empty(),
            "no parse handler may register without an explicit loadPackage"
        );
        assert!(
            result.ui_contributions.components.is_empty(),
            "no package UI contribution may register without an explicit loadPackage"
        );
        assert!(
            result.ui_contributions.panels.is_empty(),
            "no package panel may register without an explicit loadPackage"
        );
        // The five compiled-in first-party native grammars are engine
        // capability (registered by `with_first_party_native`), not silent
        // package defaults: they only highlight when an explicit `loadPackage`
        // has registered a matching major mode that selects them.
        assert_eq!(
            result.syntax_grammars.len(),
            5,
            "only the compiled-in native grammars exist with no package loaded"
        );
        for grammar in &result.syntax_grammars {
            assert_eq!(
                grammar.engine_tier,
                crate::server::syntax::SyntaxEngineTier::Native,
                "unloaded-package grammars must be native engine capability only"
            );
        }
    }

    #[tokio::test]
    async fn file_browser_workflow_config_fixture_loads_packages_and_bindings() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("file-browser-workflow");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("fixture must publish configured keybindings");

        for provider_id in [
            "rust.keywords",
            "typescript.keywords",
            "javascript.keywords",
        ] {
            assert!(
                result
                    .completion_providers
                    .iter()
                    .any(|provider| provider.id == provider_id),
                "fixture must load completion provider {provider_id}"
            );
        }
        for command_id in [
            "workspace.clientOpenFolderDialog",
            "workspace.openFuzzyFile",
            "workspace.toggleFileBrowser",
            "documents.serverSaveDocument",
            "editor.clientCopySelection",
            "editor.clientCutSelection",
            "editor.clientPasteClipboard",
            "editor.clientShowOpenDocuments",
        ] {
            assert!(
                manifest
                    .keymaps
                    .iter()
                    .any(|rule| rule.command_id == command_id),
                "fixture must bind {command_id}"
            );
        }
        for command_id in [
            "workspace.clientOpenFolderDialog",
            "editor.clientCopySelection",
            "editor.clientCutSelection",
            "editor.clientPasteClipboard",
            "editor.clientShowOpenDocuments",
        ] {
            assert!(manifest.commands.iter().any(|command| {
                command.command_id == command_id
                    && command.authority == crate::protocol::CommandAuthority::ClientUi
            }));
        }
    }

    #[tokio::test]
    async fn configuration_can_publish_sdui_snapshot() {
        let root = config_fixture("sdui-publish");
        fs::write(
            root.join("init.js"),
            r#"
            import {
              defineButton,
              defineEditorView,
              defineFlex,
              defineLabel,
              defineList,
              definePanel,
              defineStack,
              publishTree,
            } from "clay:sdui";

            const tree = defineFlex({
              id: "root",
              direction: "row",
              children: [
                definePanel({
                  id: "panel",
                  title: "Runtime Workspace",
                  children: [defineStack({
                    id: "stack",
                    children: [
                      defineLabel({ id: "label", text: "Ready" }),
                      defineButton({
                        id: "refresh",
                        label: "Refresh",
                        action: { commandId: "workspace.refresh", arguments: { force: true } },
                      }),
                      defineList({
                        id: "documents",
                        items: [{
                          id: "active",
                          label: "Document 1",
                          detail: "Runtime generated",
                          action: { commandId: "document.open_recent" },
                        }],
                      }),
                    ],
                  })],
                }),
                defineEditorView({ id: "editor", documentId: 1, expectedVersion: 1 }),
              ],
            });
            await publishTree(tree);
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
        let tree = result.published_sdui_tree.expect("published SDUI tree");

        assert_eq!(tree.ui_version, 1);
        assert_eq!(tree.nodes.len(), 7);
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Panel { title, .. } if title == "Runtime Workspace"
        )));
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::EditorView { binding }
                if binding.document_id == 1 && binding.expected_version == Some(1)
        )));
    }

    #[tokio::test]
    async fn js_generated_sdui_rejects_unknown_document_binding() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { defineEditorView, publishTree } from "clay:sdui";
                await publishTree(defineEditorView({ documentId: 999 }));
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("sdui.invalid_tree"));
    }

    #[tokio::test]
    async fn js_generated_sdui_rejects_executable_action_payloads() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { defineButton, publishTree } from "clay:sdui";
                await publishTree(defineButton({
                  label: "Run",
                  action: { commandId: "shell.run", arguments: { code: "rm -rf /" } },
                }));
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("sdui.invalid_action"));
    }

    #[tokio::test]
    async fn document_facade_open_status_list_round_trip() {
        let config_root = config_fixture("document-facade");
        let workspace_root = config_root.join("workspace");
        fs::create_dir(&workspace_root).unwrap();
        fs::write(workspace_root.join("note.txt"), "hello").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import {
              serverGetDocumentStatus,
              serverListDocuments,
              serverOpenDocument,
              serverReloadDocument,
              serverSaveDocument,
            } from "clay:documents";

            const opened = await serverOpenDocument({ workspaceRootId: "1", path: "note.txt" });
            const status = await serverGetDocumentStatus(opened.metadata.documentId);
            const saved = await serverSaveDocument({ documentId: opened.metadata.documentId });
            const reloaded = await serverReloadDocument({ documentId: opened.metadata.documentId });
            const documents = await serverListDocuments();
            Deno.core.ops.op_clay_runtime_record(`${opened.text}:${status.path}:${saved.dirty}:${reloaded.text}:${documents.length}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&workspace_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["hello:note.txt:false:hello:1"]);
    }

    #[tokio::test]
    async fn document_facade_save_rejects_future_known_version() {
        let config_root = config_fixture("document-save-version");
        let workspace_root = config_root.join("workspace");
        fs::create_dir(&workspace_root).unwrap();
        fs::write(workspace_root.join("note.txt"), "hello").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { serverOpenDocument, serverSaveDocument } from "clay:documents";
            const opened = await serverOpenDocument({ workspaceRootId: "1", path: "note.txt" });
            try {
              await serverSaveDocument({
                documentId: opened.metadata.documentId,
                knownVersion: opened.metadata.version + 1,
              });
              Deno.core.ops.op_clay_runtime_record("accepted");
            } catch (error) {
              Deno.core.ops.op_clay_runtime_record(String(error).includes("claims version") ? "rejected" : String(error));
            }
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&workspace_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["rejected"]);
    }

    #[tokio::test]
    async fn workspace_roots_facade_reports_authorized_roots() {
        let config_root = config_fixture("workspace-facade");
        let workspace_root = config_root.join("project");
        fs::create_dir(&workspace_root).unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { serverListWorkspaceRoots } from "clay:workspace";
            const roots = await serverListWorkspaceRoots();
            Deno.core.ops.op_clay_runtime_record(`${roots.length}:${roots[0].workspaceRootId}:${roots[0].displayName}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&workspace_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["1:1:project"]);
    }

    #[tokio::test]
    async fn git_facade_lists_refreshes_and_commands_statuses() {
        let config_root = config_fixture("git-facade");
        let repo_root = config_root.join("repo");
        let plain_root = config_root.join("plain");
        fs::create_dir(&repo_root).unwrap();
        fs::create_dir(&plain_root).unwrap();
        init_git_repo(&repo_root);
        fs::write(repo_root.join("tracked.txt"), "base").unwrap();
        git(&repo_root, ["add", "."]);
        git(&repo_root, ["commit", "-m", "initial"]);
        fs::write(repo_root.join("tracked.txt"), "changed").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { serverListGitStatuses, serverRefreshGitStatus } from "clay:git";
            import { serverExecuteCommand } from "clay:commands";
            const cold = await serverListGitStatuses();
            const repo = await serverRefreshGitStatus({ workspaceRootId: cold[0].workspaceRootId });
            const plain = await serverRefreshGitStatus({ workspaceRootId: cold[1].workspaceRootId });
            const listed = await serverExecuteCommand("git.listStatuses");
            const refreshed = await serverExecuteCommand("git.refreshStatus", { workspaceRootId: cold[0].workspaceRootId });
            Deno.core.ops.op_clay_runtime_record(`${cold.length}:${cold[0].refreshState.kind}:${repo.snapshot.head.kind}:${repo.snapshot.dirty}:${plain.snapshot.lastRefresh.kind}:${listed.status.kind}:${listed.status.statuses.length}:${refreshed.status.action}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&repo_root).unwrap();
        workspace.add_root(&plain_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["2:idle:branch:true:non-repository:git:2:refreshed"]
        );
    }

    #[tokio::test]
    async fn git_package_loads_and_publishes_read_only_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let config_root = config_fixture("git-package-load");
        let repo_root = config_root.join("repo");
        let plain_root = config_root.join("plain");
        fs::create_dir(&repo_root).unwrap();
        fs::create_dir(&plain_root).unwrap();
        init_git_repo(&repo_root);
        fs::write(repo_root.join("tracked.txt"), "base").unwrap();
        git(&repo_root, ["add", "."]);
        git(&repo_root, ["commit", "-m", "initial"]);
        fs::write(repo_root.join("tracked.txt"), "changed").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            import { serverListGitStatuses, serverRefreshGitStatus } from "clay:git";

            // Warm the cache first so the package status panel renders branch state.
            const cold = await serverListGitStatuses();
            await serverRefreshGitStatus({ workspaceRootId: cold[0].workspaceRootId });
            // `loadPackage("@clay/git")` runs the load entry, which publishes a
            // read-only status tree from cached clay:git data. No throw => the
            // status data path works against a repo + plain root.
            const summary = await loadPackage("@clay/git");
            const warm = await serverListGitStatuses();
            Deno.core.ops.op_clay_runtime_record(`${summary.name}:${summary.apiPrefix}:${summary.permissions.length}:${summary.contributions.sdui}:${warm.length}:${warm[0].snapshot.head.kind}:${warm[0].snapshot.dirty}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&repo_root).unwrap();
        workspace.add_root(&plain_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["@clay/git:git:0:1:2:branch:true"]);
    }

    #[tokio::test]
    async fn git_package_declares_no_mutation_or_network_authority() {
        // Phase 18.13: prove @clay/git is read-only. It declares no permissions
        // (no network/shell/filesystem/mutation), registers no package commands,
        // and exposes no configuration/package options (fixed safe defaults).
        // Mutating Git operations and config knobs are intentionally out of scope.
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let config_root = config_fixture("git-package-authority");
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";

            const summary = await loadPackage("@clay/git");
            const perms = summary.permissions.join(",");
            const mutating = ["filesystem", "network", "shell", "wasm", "ai-tools",
              "workspace-mutation", "native-ui", "client-runtime", "raw-ops",
              "package-control", "package-import"];
            const leaked = mutating.filter((m) => perms.includes(m)).join(",");
            Deno.core.ops.op_clay_runtime_record(
              `${perms.length}:${summary.contributions.commands}:` +
              `${summary.contributions.configuration}:${summary.contributions.packageOptions}:${leaked}`
            );
            "#,
        )
        .unwrap();
        let workspace = WorkspaceState::new();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        // perms:commands:configuration:packageOptions:leaked — all zero/empty
        assert_eq!(result.op_records, vec!["0:0:0:0:"]);
    }

    #[tokio::test]
    async fn document_facade_rejects_unauthorized_paths() {
        let parent = config_fixture("document-facade-reject");
        let config_root = parent.join("config");
        let workspace_root = parent.join("workspace");
        fs::create_dir(&config_root).unwrap();
        fs::create_dir(&workspace_root).unwrap();
        fs::write(parent.join("outside.txt"), "secret").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { serverOpenDocument } from "clay:documents";
            await serverOpenDocument({ workspaceRootId: "1", path: "../outside.txt" });
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&workspace_root).unwrap();

        let error = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("documents.open_failed"));
        assert!(error.to_string().contains("outside the authorized root"));
    }

    #[tokio::test]
    async fn configuration_runtime_rejects_traversal_and_urls() {
        for rejected_path in [
            "../outside.js",
            "https://example.invalid/config.js",
            "npm:pkg",
            "package",
        ] {
            let root = config_fixture("reject");
            fs::write(
                root.join("init.js"),
                format!(
                    r#"
                    import {{ loadConfigurationModule }} from "clay:configuration";
                    await loadConfigurationModule({{ path: "{rejected_path}" }});
                    "#
                ),
            )
            .unwrap();
            let error = ClayJsRuntimeService::default()
                .load_configuration_from_root(root)
                .await
                .unwrap_err();

            assert!(matches!(error, ClayRuntimeError::Runtime(_)));
            assert!(error.to_string().contains("configuration.invalid_module"));
        }
    }

    #[tokio::test]
    async fn configuration_error_diagnostic_names_the_rejected_specifier() {
        // Phase 20.6: a config typo (e.g. `clay:themes` instead of `clay:theme`)
        // must produce a diagnostic that names the rejected specifier so the
        // user can fix it, not an opaque generic string.
        let root = config_fixture("bad-facade-import");
        fs::write(
            root.join("init.js"),
            r#"
            import { setAppearance } from "clay:themes";
            setAppearance("light");
            "#,
        )
        .unwrap();
        let error = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();
        assert_eq!(diagnostic.code, "configuration.invalid_module");
        // Secure: the rejected specifier (`clay:themes`) must NOT leak...
        assert!(
            !diagnostic.message.contains("clay:themes"),
            "diagnostic must not leak the rejected specifier, got: {}",
            diagnostic.message
        );
        // ...but the message must be actionable: name a real facade example
        // so the user can spot the typo.
        assert!(
            diagnostic.message.contains("clay:theme")
                && diagnostic.message.contains("specifier spelling"),
            "diagnostic must name an allowed facade and hint at spelling, got: {}",
            diagnostic.message
        );
    }

    #[tokio::test]
    async fn configuration_bind_key_updates_behavior_manifest() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings } from "clay:keybindings";
                import { getActiveBehaviorManifest, listBehaviorRoutes } from "clay:behavior";
                const bound = bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
                const bindings = listKeyBindings("editor");
                const manifest = await getActiveBehaviorManifest();
                const routes = await listBehaviorRoutes();
                Deno.core.ops.op_clay_runtime_record(`${bound.key}:${bound.command}:${manifest.version}:${bindings.length}:${routes.some((route) => route.apiId === "documents.serverSaveDocument")}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(
            result.op_records,
            vec!["Ctrl+S:documents.serverSaveDocument:2:3:true"]
        );
        assert_eq!(manifest.behavior_version, 2);
        assert!(manifest.keymaps.iter().any(|rule| {
            rule.command_id == "documents.serverSaveDocument"
                && rule.routing_policy == crate::protocol::RoutingPolicy::ServerFirst
        }));
    }

    #[tokio::test]
    async fn configuration_default_reload_binding_is_present_and_overridable() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings, unbindKey } from "clay:keybindings";
                const defaultBinding = listKeyBindings("global").find(
                  (binding) => binding.command === "runtime.reloadConfiguration"
                );
                unbindKey("Ctrl+Shift+R", { scope: "global" });
                bindKey("Ctrl+Alt+R", "runtime.reloadConfiguration", { scope: "global" });
                const bindings = listKeyBindings("global");
                Deno.core.ops.op_clay_runtime_record(
                  `${defaultBinding?.key}:${bindings.some((binding) => binding.key === "Ctrl+Shift+R")}:${bindings.some((binding) => binding.key === "Ctrl+Alt+R")}`
                );
                "#,
            )
            .await
            .expect("override reload command");
        let manifest = result.behavior_manifest.expect("bound behavior manifest");
        let rule = manifest
            .keymaps
            .iter()
            .find(|rule| rule.command_id == "runtime.reloadConfiguration")
            .expect("overridden reload binding");

        assert_eq!(result.op_records, vec!["Ctrl+Shift+R:false:true"]);
        assert_eq!(rule.context, crate::protocol::KeyBindingContext::Global);
        assert_eq!(
            rule.sequence,
            vec![crate::protocol::KeyStroke {
                key: crate::protocol::KeyCode::Character("r".to_string()),
                modifiers: crate::protocol::KeyModifiers {
                    control: true,
                    alt: true,
                    ..crate::protocol::KeyModifiers::NONE
                },
            }]
        );
        assert_eq!(
            rule.routing_policy,
            crate::protocol::RoutingPolicy::ServerFirstWithLock {
                lock_scope: crate::protocol::LockScope::Behavior,
            }
        );
    }

    #[tokio::test]
    async fn package_javascript_cannot_directly_execute_reload_command() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverExecuteCommand } from "clay:commands";
                try {
                  await serverExecuteCommand("runtime.reloadConfiguration");
                } catch (error) {
                  Deno.core.ops.op_clay_runtime_record(String(error));
                }
                "#,
            )
            .await
            .expect("reload denial remains a handled JS error");

        assert!(result.op_records.iter().any(|record| {
            record.contains("UnauthorizedTarget")
                && record.contains("runtime reload requires a user command intent")
        }));
    }

    #[tokio::test]
    async fn configuration_binds_client_ui_file_folder_and_copy_commands() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings } from "clay:keybindings";
                import { listBehaviorRoutes } from "clay:behavior";
                import { clientOpenFileDialog } from "clay:documents";
                import { clientOpenFolderDialog } from "clay:workspace";
                import { clientCopySelection } from "clay:editor";
                const file = bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
                const folder = bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
                const copy = bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
                const bindings = listKeyBindings("editor");
                const routes = await listBehaviorRoutes();
                const fileRoute = routes.find((candidate) => candidate.apiId === "documents.clientOpenFileDialog");
                const folderRoute = routes.find((candidate) => candidate.apiId === "workspace.clientOpenFolderDialog");
                const copyRoute = routes.find((candidate) => candidate.apiId === "editor.clientCopySelection");
                Deno.core.ops.op_clay_runtime_record(`${file.key}:${file.command}:${folder.key}:${folder.command}:${copy.key}:${copy.command}:${bindings.length}:${fileRoute.runtimePath}:${fileRoute.authority}:${folderRoute.runtimePath}:${folderRoute.authority}:${copyRoute.runtimePath}:${copyRoute.authority}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(
            result.op_records,
            vec![
                "Ctrl+O:documents.clientOpenFileDialog:Ctrl+Shift+O:workspace.clientOpenFolderDialog:Ctrl+Shift+C:editor.clientCopySelection:5:client-ui-command:client-ui:client-ui-command:client-ui:client-ui-command:client-ui"
            ]
        );
        for command_id in [
            "documents.clientOpenFileDialog",
            "workspace.clientOpenFolderDialog",
            "editor.clientCopySelection",
        ] {
            assert!(manifest.keymaps.iter().any(|rule| {
                rule.command_id == command_id
                    && rule.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
            }));
            assert!(manifest.commands.iter().any(|command| {
                command.command_id == command_id
                    && command.authority == crate::protocol::CommandAuthority::ClientUi
            }));
        }
    }

    #[tokio::test]
    async fn configuration_unbind_key_updates_behavior_manifest() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey, unbindKey, listKeyBindings } from "clay:keybindings";
                bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
                unbindKey("Ctrl+S", { scope: "editor" });
                Deno.core.ops.op_clay_runtime_record(`${listKeyBindings("editor").some((binding) => binding.key === "Ctrl+S")}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(result.op_records, vec!["false"]);
        assert_eq!(manifest.behavior_version, 3);
        assert!(
            !manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "documents.serverSaveDocument")
        );
    }

    #[tokio::test]
    async fn configuration_bind_key_table_form_binds_multiple_commands() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings } from "clay:keybindings";
                const bound = bindKey({
                  scope: "editor",
                  bindings: {
                    "Ctrl+O": "documents.clientOpenFileDialog",
                    "Alt+I": "editor.clientSelectTextobject.function.inner.current",
                    "Ctrl+S": "documents.serverSaveDocument",
                  },
                });
                const bindings = listKeyBindings("editor");
                Deno.core.ops.op_clay_runtime_record(`${bound.length}:${bound[0].key}:${bound[1].command}:${bindings.some((binding) => binding.key === "Ctrl+O")}:${bindings.some((binding) => binding.key === "Alt+I")}:${bindings.some((binding) => binding.key === "Ctrl+S")}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(
            result.op_records,
            vec!["3:Ctrl+O:editor.clientSelectTextobject.function.inner.current:true:true:true"]
        );
        for command_id in [
            "documents.clientOpenFileDialog",
            "editor.clientSelectTextobject.function.inner.current",
            "documents.serverSaveDocument",
        ] {
            assert!(
                manifest
                    .keymaps
                    .iter()
                    .any(|rule| rule.command_id == command_id),
                "{command_id} must be bound by the table form"
            );
        }
        assert_eq!(
            manifest
                .keymaps
                .iter()
                .filter(|rule| {
                    rule.context == crate::protocol::KeyBindingContext::EditorTextFocus
                        && matches!(
                            rule.command_id.as_str(),
                            "documents.clientOpenFileDialog"
                                | "editor.clientSelectTextobject.function.inner.current"
                                | "documents.serverSaveDocument"
                        )
                })
                .count(),
            3
        );
    }

    #[tokio::test]
    async fn configuration_bind_key_table_form_is_all_or_nothing() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings } from "clay:keybindings";
                const bound = bindKey({
                  scope: "editor",
                  bindings: {
                    "Ctrl+O": "documents.clientOpenFileDialog",
                    "PgDn": "editor.clientUndo",
                  },
                });
                Deno.core.ops.op_clay_runtime_record(`${bound.length}`);
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        let message = error.to_string();
        assert!(
            message.contains("entry 2") && message.contains("unsupported key `PgDn`"),
            "diagnostic must name the failing table entry: {message}"
        );
        // All-or-nothing: the valid first entry must not be applied. The
        // manifest is not reachable from the error, so verify via a fresh
        // evaluation that no partial binding leaked into the shared service.
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { listKeyBindings } from "clay:keybindings";
                Deno.core.ops.op_clay_runtime_record(`${listKeyBindings("editor").some((binding) => binding.key === "Ctrl+O")}`);
                "#,
            )
            .await
            .unwrap();
        assert_eq!(result.op_records, vec!["false"]);
    }

    #[tokio::test]
    async fn configuration_unbind_key_table_form_unbinds_multiple_keys() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey, unbindKey, listKeyBindings } from "clay:keybindings";
                bindKey({
                  scope: "editor",
                  bindings: {
                    "Ctrl+O": "documents.clientOpenFileDialog",
                    "Alt+I": "editor.clientSelectTextobject.function.inner.current",
                  },
                });
                unbindKey({ scope: "editor", keys: ["Ctrl+O", "Alt+I"] });
                const bindings = listKeyBindings("editor");
                Deno.core.ops.op_clay_runtime_record(`${bindings.length}:${bindings.some((binding) => binding.key === "Ctrl+O")}:${bindings.some((binding) => binding.key === "Alt+I")}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(result.op_records, vec!["2:false:false"]);
        assert!(!manifest.keymaps.iter().any(|rule| {
            matches!(
                rule.command_id.as_str(),
                "documents.clientOpenFileDialog"
                    | "editor.clientSelectTextobject.function.inner.current"
            )
        }));
    }

    #[tokio::test]
    async fn unknown_command_binding_is_rejected() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey } from "clay:keybindings";
                bindKey("Ctrl+X", "shell.run");
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("keybindings.unknown_command"));
    }

    #[tokio::test]
    async fn raw_clipboard_and_dialog_command_bindings_are_rejected() {
        for command_id in [
            "clipboard.writeText",
            "dialog.openRawPath",
            "Deno.core.ops.op_clipboard_write",
        ] {
            let source = format!(
                r#"
                import {{ bindKey }} from "clay:keybindings";
                bindKey("Ctrl+Alt+C", {command_id:?});
                "#
            );
            let error = ClayJsRuntimeService::default()
                .evaluate_controlled_module_for_document(source, 88)
                .await
                .unwrap_err();

            assert!(matches!(error, ClayRuntimeError::Runtime(_)));
            assert!(
                error.to_string().contains("keybindings.unknown_command"),
                "{command_id} must stay rejected: {error}"
            );
        }
    }

    #[tokio::test]
    async fn runtime_imports_modes_commands_and_packages_facades() {
        let service = ClayJsRuntimeService::default();
        let result = evaluate_as_trusted_package(
            &service,
            test_package_json(
                "@clay/markdown-facade",
                "markdown",
                &[
                    "mode-registration",
                    "mode-activation",
                    "command-registration",
                    "parse-document",
                ],
                serde_json::json!({
                    "commands": [{ "id": "markdown.togglePreview", "displayName": "Toggle Markdown Preview", "routingPolicy": "server-first" }]
                }),
            ),
            vec![
                crate::packages::permissions::PackagePermission::ModeRegistration,
                crate::packages::permissions::PackagePermission::ModeActivation,
                crate::packages::permissions::PackagePermission::CommandRegistration,
                crate::packages::permissions::PackagePermission::ParseDocument,
            ],
            r#"
                import { serverRegisterModePattern, serverActivateMajorMode } from "clay:modes";
                import { serverRegisterCommand, serverListCommands } from "clay:commands";
                import { serverLoadPackage, serverValidatePackagePermissions } from "clay:packages";
                import { serverPublishDecorations } from "clay:decorations";
                import { serverRegisterParseHandler } from "clay:parse";

                if (typeof serverPublishDecorations !== "function" || typeof serverRegisterParseHandler !== "function") {
                  throw new Error("decoration/parse facade export missing");
                }
                // serverLoadPackage validates a manifest shape only; it grants
                // no authority and sets no provenance.
                const manifest = {
                  name: "@clay/markdown-facade",
                  version: "0.1.0",
                  clay: {
                    apiPrefix: "markdown",
                    permissions: ["mode-registration", "mode-activation", "command-registration", "parse-document", "package-configuration"],
                    modes: ["markdown"],
                    entry: "./dist/index.js",
                    loadEntry: "./dist/load.js",
                    docs: "./docs/index.md",
                    performance: { estimatedManifestBytes: 2048 },
                    apiDependencies: ["modes.serverRegisterModePattern", "commands.serverRegisterCommand"],
                    contributions: {
                      commands: [{ id: "markdown.togglePreview", displayName: "Toggle Markdown Preview", routingPolicy: "server-first" }],
                      configuration: [{ key: "markdown.preview.enabled", type: "boolean", default: false }]
                    }
                  }
                };
                const loaded = serverLoadPackage(manifest);
                const permissions = serverValidatePackagePermissions(manifest.clay.permissions);
                serverRegisterModePattern({
                  modeId: "markdown",
                  displayName: "Markdown",
                  extensions: ["md"],
                  mimeTypes: ["text/markdown"]
                });
                const activation = serverActivateMajorMode({ documentId: 5, path: "README.md" });
                const command = serverRegisterCommand({
                  commandId: "markdown.togglePreview",
                  displayName: "Toggle Markdown Preview",
                  permissions: ["parse-document"]
                });
                const commands = serverListCommands();
                Deno.core.ops.op_clay_runtime_record(`${loaded.contributions.commands}:${permissions.permissions.length}:${activation.modeId}:${activation.behaviorVersion}:${command.commandId}:${commands.length}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(
            result.op_records,
            vec!["1:5:markdown:1:markdown.togglePreview:1"]
        );
    }

    #[tokio::test]
    async fn syntax_grammar_packages_default_load_from_init_js() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let root = config_fixture("syntax-grammar-init-load");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";

            const loaded = [];
            for (const specifier of ["@clay/rust", "@clay/typescript", "@clay/javascript", "@clay/markdown"]) {
              const summary = await loadPackage(specifier);
              loaded.push(`${summary.name}:${summary.apiPrefix}:${summary.modes.length}:${summary.permissions.join("+")}:${summary.contributions.syntaxGrammars}`);
            }
            Deno.core.ops.op_clay_runtime_record(loaded.join("|"));
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec![
                "@clay/rust:rust:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:1|@clay/typescript:typescript:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:1|@clay/javascript:javascript:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:1|@clay/markdown:markdown:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:1"
            ]
        );
    }

    #[tokio::test]
    async fn invalid_mode_font_role_fails_before_registration_and_keeps_core_fallback() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverClassifyDocument, serverRegisterModePattern } from "clay:modes";
                const manifest = {
                  name: "@clay/example", version: "0.1.0", type: "module",
                  exports: { ".": "./index.js" },
                  clay: {
                    apiPrefix: "example", entry: "./index.js",
                    permissions: ["mode-registration", "mode-activation"],
                    modes: ["example"], docs: "./docs/index.md"
                  }
                };
                try {
                  serverRegisterModePattern(manifest, {
                    modeId: "example", extensions: ["rs"], defaultFontRole: "serif"
                  });
                } catch {}
                const classification = serverClassifyDocument({ documentId: 9, path: "main.rs" });
                Deno.core.ops.op_clay_runtime_record(classification.modeId);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["core.code"]);
    }

    #[tokio::test]
    async fn rust_package_expansion_registers_mode_command_completion_and_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";

                const summary = await loadPackage("@clay/rust");
                const classification = serverClassifyDocument({ documentId: 42, path: "src/main.rs" });
                const commands = serverListCommands();
                const rustCommand = commands.find((command) => command.commandId === "rust.toggleLineComment");

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    apiPrefix: summary.apiPrefix,
                    modes: summary.modes,
                    commands: summary.contributions.commands,
                    uiComponents: summary.contributions.uiComponents,
                    classification,
                    rustCommandRegistered: Boolean(rustCommand)
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["apiPrefix"], "rust");
        assert_eq!(parsed["modes"], serde_json::json!(["rust"]));
        assert_eq!(parsed["classification"]["modeId"], "rust");
        assert_eq!(parsed["classification"]["apiPrefix"], "rust");
        assert!(parsed["rustCommandRegistered"].as_bool().unwrap());
        assert_eq!(parsed["commands"], 1);
        assert_eq!(parsed["uiComponents"], 1);
    }

    #[tokio::test]
    async fn typescript_package_expansion_registers_mode_command_completion_and_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";

                const summary = await loadPackage("@clay/typescript");
                const classification = serverClassifyDocument({ documentId: 42, path: "src/index.ts" });
                const commands = serverListCommands();
                const tsCommand = commands.find((command) => command.commandId === "typescript.toggleLineComment");

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    apiPrefix: summary.apiPrefix,
                    modes: summary.modes,
                    commands: summary.contributions.commands,
                    uiComponents: summary.contributions.uiComponents,
                    classification,
                    tsCommandRegistered: Boolean(tsCommand)
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["apiPrefix"], "typescript");
        assert_eq!(parsed["modes"], serde_json::json!(["typescript"]));
        assert_eq!(parsed["classification"]["modeId"], "typescript");
        assert_eq!(parsed["classification"]["apiPrefix"], "typescript");
        assert!(parsed["tsCommandRegistered"].as_bool().unwrap());
        assert_eq!(parsed["commands"], 1);
        assert_eq!(parsed["uiComponents"], 1);
    }

    #[tokio::test]
    async fn javascript_package_expansion_registers_mode_command_completion_and_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";

                const summary = await loadPackage("@clay/javascript");
                const classification = serverClassifyDocument({ documentId: 42, path: "src/index.js" });
                const commands = serverListCommands();
                const jsCommand = commands.find((command) => command.commandId === "javascript.toggleLineComment");

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    apiPrefix: summary.apiPrefix,
                    modes: summary.modes,
                    commands: summary.contributions.commands,
                    uiComponents: summary.contributions.uiComponents,
                    classification,
                    jsCommandRegistered: Boolean(jsCommand)
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["apiPrefix"], "javascript");
        assert_eq!(parsed["modes"], serde_json::json!(["javascript"]));
        assert_eq!(parsed["classification"]["modeId"], "javascript");
        assert_eq!(parsed["classification"]["apiPrefix"], "javascript");
        assert!(parsed["jsCommandRegistered"].as_bool().unwrap());
        assert_eq!(parsed["commands"], 1);
        assert_eq!(parsed["uiComponents"], 1);
    }

    #[tokio::test]
    async fn each_language_mode_registers_indent_electric_pairs_comment_triggers() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let cases = [
            (
                "@clay/rust",
                "src/main.rs",
                4,
                5,
                1,
                vec!["}"],
                vec![".", ":"],
                false,
                6400,
            ),
            (
                "@clay/typescript",
                "src/main.ts",
                2,
                6,
                1,
                vec!["}", ")", "]"],
                vec!["."],
                false,
                6400,
            ),
            (
                "@clay/javascript",
                "src/main.js",
                2,
                6,
                1,
                vec!["}", ")", "]"],
                vec!["."],
                false,
                6400,
            ),
            (
                "@clay/markdown",
                "README.md",
                2,
                5,
                0,
                vec![],
                vec!["#", "[", "`"],
                true,
                6400,
            ),
        ];

        for (
            specifier,
            path,
            indent,
            pair_count,
            comment_count,
            electric,
            triggers,
            markdown,
            estimated_bytes,
        ) in cases
        {
            let source = format!(
                r#"
                import {{ loadPackage }} from "clay:packages";
                import {{ serverActivateClassifiedMode, serverClassifyDocument }} from "clay:modes";
                await loadPackage("{specifier}");
                const classification = serverClassifyDocument({{ documentId: 88, path: "{path}" }});
                serverActivateClassifiedMode(classification, {{ path: "{path}" }});
                "#,
            );
            let result = ClayJsRuntimeService::default()
                .evaluate_controlled_module_for_document(source, 88)
                .await
                .unwrap_or_else(|error| panic!("{specifier} should activate: {error}"));
            let manifest = result
                .behavior_manifest
                .unwrap_or_else(|| panic!("{specifier} should publish a behavior manifest"));
            let rules = &manifest.editor_rules;

            assert_eq!(rules.tab.spaces_per_tab, indent, "{specifier} indent");
            assert_eq!(rules.pairs.len(), pair_count, "{specifier} pairs");
            assert_eq!(rules.comments.len(), comment_count, "{specifier} comments");
            assert_eq!(
                rules
                    .electric_characters
                    .iter()
                    .map(|rule| rule.trigger.as_str())
                    .collect::<Vec<_>>(),
                electric,
                "{specifier} electric characters"
            );
            assert_eq!(
                rules
                    .autocomplete_triggers
                    .iter()
                    .map(|rule| rule.trigger.as_str())
                    .collect::<Vec<_>>(),
                triggers,
                "{specifier} autocomplete triggers"
            );
            if markdown {
                assert!(matches!(
                    &rules.enter,
                    EnterRule::ContinueLineMarkers {
                        markers,
                        exit_on_empty_item: true,
                    } if markers == &["-", "*", "+", "ordered-dot"]
                ));
                // Plan 071 task 11: markdown ships prose movement via its
                // manifest — underscore and camelCase carry no meaning in
                // prose. Caret defers to the editor default bar.
                assert_eq!(
                    rules.movement.word_separators,
                    crate::protocol::WordSeparatorPolicy::Prose,
                    "{specifier} prose word separators"
                );
                assert!(
                    !rules.movement.treat_underscore_as_word,
                    "{specifier} prose underscore policy"
                );
                assert!(
                    !rules.movement.camel_case_sub_word,
                    "{specifier} prose camelCase policy"
                );
            } else {
                assert!(matches!(rules.enter, EnterRule::PreserveLeadingWhitespace));
                assert_eq!(rules.comments[0].line_prefix, "//");
                // Plan 071 task 11: code packages declare the code movement
                // policy explicitly (identical to the built-in default).
                assert_eq!(
                    rules.movement.word_separators,
                    crate::protocol::WordSeparatorPolicy::Code,
                    "{specifier} code word separators"
                );
                assert!(
                    rules.movement.treat_underscore_as_word,
                    "{specifier} code underscore policy"
                );
                assert!(
                    rules.movement.camel_case_sub_word,
                    "{specifier} code camelCase policy"
                );
            }
            // No package ships a caret override today: the reduced-motion-safe
            // editor default bar applies to every mode (customization is opt-in).
            assert_eq!(
                rules.caret_style, None,
                "{specifier} caret defers to default"
            );
            let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&manifest)
                .expect("behavior manifest serializes")
                .len();
            assert!(
                payload <= estimated_bytes,
                "{specifier} payload {payload} exceeds package estimate {estimated_bytes}"
            );
            assert!(payload <= BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES);
        }
    }

    /// Plan 071 task 11: `loadPackage("@clay/markdown")` yields prose movement
    /// for Markdown documents (asserted in
    /// `each_language_mode_registers_indent_electric_pairs_comment_triggers`)
    /// and leaves unrelated document types on the built-in fallback defaults —
    /// no silent behaviour change. The built-in `core.code`/`core.text` modes
    /// ship movement/caret/ligature defaults with no owning package.
    #[tokio::test]
    async fn markdown_load_yields_prose_movement_without_touching_code_defaults() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                await loadPackage("@clay/markdown");
                const markdown = serverClassifyDocument({ documentId: 1, path: "README.md" });
                const code = serverClassifyDocument({ documentId: 2, path: "src/main.rs" });
                const text = serverClassifyDocument({ documentId: 3, path: "notes" });
                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    markdown: markdown.modeId, code: code.modeId, text: text.modeId
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["markdown"], "markdown");
        // Markdown must not claim unrelated document types: with no language
        // package loaded, code-like and plain files still resolve to the
        // built-in core.code/core.text fallbacks.
        assert_eq!(parsed["code"], "core.code");
        assert_eq!(parsed["text"], "core.text");

        // Built-in fallback manifests ship the defaults without any package:
        // code movement for core.code, caret deferred to the editor default
        // bar, and role-selected typography ligatures from the baseline.
        let code = crate::protocol::BehaviorManifest::core_code_editing(1);
        assert_eq!(code.manifest_id, "default.code");
        assert_eq!(
            code.editor_rules.movement.word_separators,
            crate::protocol::WordSeparatorPolicy::Code
        );
        assert!(code.editor_rules.movement.treat_underscore_as_word);
        assert!(code.editor_rules.movement.camel_case_sub_word);
        assert_eq!(code.editor_rules.caret_style, None);
        assert_eq!(
            code.document_font_role,
            crate::protocol::DocumentFontRole::Monospace
        );

        let text = crate::protocol::BehaviorManifest::minimal_text_editing(1);
        assert_eq!(text.manifest_id, "default.text");
        assert_eq!(text.editor_rules.caret_style, None);
        assert_eq!(
            text.document_font_role,
            crate::protocol::DocumentFontRole::Proportional
        );

        // Ligature baseline ships with every font role (standard + contextual
        // on), so both fallback roles resolve ligatures at install time.
        let typography = crate::protocol::ActiveTypography::default();
        for profile in [&typography.monospace, &typography.proportional] {
            assert!(profile.ligatures.enable_standard);
            assert!(profile.ligatures.enable_contextual);
        }
    }

    /// Plan 071 task 11: a package may customize its mode's movement and caret
    /// through documented `editorRules` manifest data (validated server-side);
    /// absent fields keep the defaults.
    #[tokio::test]
    async fn package_manifest_can_customize_movement_and_caret_style() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let result = evaluate_as_trusted_package(
            &service,
            test_package_json(
                "@clay/fixture-prose",
                "fixtureprose",
                &["mode-registration", "mode-activation"],
                serde_json::json!({}),
            ),
            vec![
                crate::packages::permissions::PackagePermission::ModeRegistration,
                crate::packages::permissions::PackagePermission::ModeActivation,
            ],
            r#"
            import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
            serverRegisterModePattern({
                modeId: "fixtureprose",
                displayName: "Fixture Prose",
                defaultFontRole: "proportional",
                extensions: ["fxp"],
                editorRules: {
                    movement: {
                        wordSeparators: "prose",
                        treatUnderscoreAsWord: false,
                        camelCaseSubWord: false,
                        stickyColumn: false
                    },
                    caretStyle: { shape: "block", blink: "blink", stopBlinkOnTyping: false }
                }
            });
            const classification = serverClassifyDocument({ documentId: 7, path: "notes.fxp" });
            serverActivateClassifiedMode(classification, { path: "notes.fxp" });
            "#,
        )
        .await
        .unwrap();

        let manifest = result
            .behavior_manifest
            .expect("custom mode activation publishes a manifest");
        let rules = &manifest.editor_rules;
        assert_eq!(
            rules.movement.word_separators,
            crate::protocol::WordSeparatorPolicy::Prose
        );
        assert!(!rules.movement.treat_underscore_as_word);
        assert!(!rules.movement.camel_case_sub_word);
        assert!(!rules.movement.sticky_column);
        // Absent movement fields keep the defaults.
        assert_eq!(
            rules.movement.paragraph_style,
            crate::protocol::ParagraphStyle::BlankLineOrWhitespace
        );
        let caret = rules.caret_style.expect("caret override applies");
        assert_eq!(caret.shape, crate::protocol::CaretShape::Block);
        assert!(matches!(
            caret.blink,
            crate::protocol::BlinkStyle::Blink { .. }
        ));
        assert!(!caret.stop_blink_on_typing);
        // Absent caret fields keep the defaults.
        assert_eq!(
            caret.width_px,
            crate::protocol::CaretStyle::default().width_px
        );
    }

    /// Follow-up round (`editor-control`): package callers may use the editor
    /// ops only with approved `editor-control` AND an active major mode named
    /// in their `clay.editorControl.modes` declaration. Deny-by-default.
    #[tokio::test]
    async fn editor_control_gate_enforces_permission_and_declared_mode() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let editor_control = crate::packages::permissions::PackagePermission::EditorControl;
        let register = crate::packages::permissions::PackagePermission::ModeRegistration;
        let activate = crate::packages::permissions::PackagePermission::ModeActivation;

        // (a) Approved editor-control + declared active mode: allowed.
        let mut package_json = test_package_json(
            "@clay/fixture-editor-ok",
            "fixtureeditorok",
            &["mode-registration", "mode-activation", "editor-control"],
            serde_json::json!({}),
        );
        package_json["clay"]["editorControl"] = serde_json::json!({ "modes": ["fixtureeditorok"] });
        let evaluation = evaluate_as_trusted_package(
            &service,
            package_json,
            vec![register, activate, editor_control],
            r#"
            import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
            serverRegisterModePattern({
                modeId: "fixtureeditorok",
                displayName: "Fixture Editor OK",
                defaultFontRole: "proportional",
                extensions: ["feo"],
                editorRules: { tabSpaces: 4 }
            });
            const classification = serverClassifyDocument({ documentId: 71, path: "a.feo" });
            serverActivateClassifiedMode(classification, { path: "a.feo" });
            const moved = JSON.parse(Deno.core.ops.op_clay_editor_move_cursor(
                JSON.stringify({ direction: "nextWordStart" })));
            if (moved.commandId !== "editor.clientMoveCursor" || moved.direction !== "nextWordStart") {
                throw new Error("unexpected descriptor: " + moved.commandId + "/" + moved.direction);
            }
            "#,
        )
        .await;
        assert!(
            evaluation.is_ok(),
            "declared mode + approved editor-control must pass the gate: {evaluation:?}"
        );

        // (b) Missing `editor-control` permission: denied.
        let package_json = test_package_json(
            "@clay/fixture-editor-noperm",
            "fixtureeditornoperm",
            &["mode-registration", "mode-activation"],
            serde_json::json!({}),
        );
        let evaluation = evaluate_as_trusted_package(
            &service,
            package_json,
            vec![register, activate],
            r#"
            import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
            serverRegisterModePattern({
                modeId: "fixtureeditornoperm",
                displayName: "Fixture Editor NoPerm",
                defaultFontRole: "proportional",
                extensions: ["fen"],
                editorRules: { tabSpaces: 4 }
            });
            const classification = serverClassifyDocument({ documentId: 72, path: "a.fen" });
            serverActivateClassifiedMode(classification, { path: "a.fen" });
            let error = "";
            try {
                Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
            } catch (e) {
                error = String(e && e.message ? e.message : e);
            }
            if (!error.includes("editor-control")) {
                throw new Error("expected editor-control denial, got: " + (error || "allowed"));
            }
            "#,
        )
        .await;
        assert!(
            evaluation.is_ok(),
            "missing-permission case must deny inside JS: {evaluation:?}"
        );

        // (c) Approved editor-control but the active mode is not declared:
        // denied deny-by-default.
        let mut package_json = test_package_json(
            "@clay/fixture-editor-wrongmode",
            "fixtureeditorwrongmode",
            &["mode-registration", "mode-activation", "editor-control"],
            serde_json::json!({}),
        );
        package_json["clay"]["editorControl"] = serde_json::json!({ "modes": ["some.other.mode"] });
        let evaluation = evaluate_as_trusted_package(
            &service,
            package_json,
            vec![register, activate, editor_control],
            r#"
            import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
            serverRegisterModePattern({
                modeId: "fixtureeditorwrongmode",
                displayName: "Fixture Editor WrongMode",
                defaultFontRole: "proportional",
                extensions: ["few"],
                editorRules: { tabSpaces: 4 }
            });
            const classification = serverClassifyDocument({ documentId: 73, path: "a.few" });
            serverActivateClassifiedMode(classification, { path: "a.few" });
            let error = "";
            try {
                Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
            } catch (e) {
                error = String(e && e.message ? e.message : e);
            }
            if (!error.includes("mode_not_declared")) {
                throw new Error("expected mode_not_declared denial, got: " + (error || "allowed"));
            }
            "#,
        )
        .await;
        assert!(
            evaluation.is_ok(),
            "undeclared-mode case must deny inside JS: {evaluation:?}"
        );
    }

    /// Follow-up round (`editor-control`): the programmatic execution op
    /// publishes gated known editor command IDs to the connection lane
    /// with host-stamped provenance, and denies unknown IDs deny-by-default.
    #[tokio::test]
    async fn editor_control_execute_publishes_gated_known_commands_only() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let mut receiver = service.subscribe_editor_commands();
        let editor_control = crate::packages::permissions::PackagePermission::EditorControl;
        let register = crate::packages::permissions::PackagePermission::ModeRegistration;
        let activate = crate::packages::permissions::PackagePermission::ModeActivation;

        let mut package_json = test_package_json(
            "@clay/fixture-editor-exec",
            "fixtureeditorexec",
            &["mode-registration", "mode-activation", "editor-control"],
            serde_json::json!({}),
        );
        package_json["clay"]["editorControl"] =
            serde_json::json!({ "modes": ["fixtureeditorexec"] });
        let evaluation = evaluate_as_trusted_package(
            &service,
            package_json,
            vec![register, activate, editor_control],
            r#"
            import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
            serverRegisterModePattern({
                modeId: "fixtureeditorexec",
                displayName: "Fixture Editor Exec",
                defaultFontRole: "proportional",
                extensions: ["fex"],
                editorRules: { tabSpaces: 4 }
            });
            const classification = serverClassifyDocument({ documentId: 75, path: "a.fex" });
            serverActivateClassifiedMode(classification, { path: "a.fex" });
            const executed = JSON.parse(Deno.core.ops.op_clay_editor_execute_command(
                JSON.stringify({ commandId: "editor.clientMoveCursor.nextWordStart" })));
            if (!executed.requested) {
                throw new Error("expected requested=true");
            }
            let error = "";
            try {
                Deno.core.ops.op_clay_editor_execute_command(
                    JSON.stringify({ commandId: "application.quit" }));
            } catch (e) {
                error = String(e && e.message ? e.message : e);
            }
            if (!error.includes("not a known editor command")) {
                throw new Error("expected unknown-ID denial, got: " + (error || "allowed"));
            }
            "#,
        )
        .await;
        assert!(
            evaluation.is_ok(),
            "execute op must publish known IDs and deny unknown ones: {evaluation:?}"
        );

        let request = tokio::time::timeout(std::time::Duration::from_millis(500), receiver.recv())
            .await
            .expect("execution request reaches the connection lane")
            .expect("editor command channel stays open");
        assert_eq!(request.command_id, "editor.clientMoveCursor.nextWordStart");
        assert_eq!(request.package_prefix, "fixtureeditorexec");
        assert_eq!(request.mode_id, "fixtureeditorexec");
    }

    /// Plan 071 caret-transport fix: `clientSetCursorStyle` from user
    /// configuration publishes the merged runtime caret override on the
    /// connection lane. Before the fix the op only validated and returned a
    /// descriptor, so blink/phase settings never reached the client.
    #[tokio::test]
    async fn set_cursor_style_publishes_runtime_caret_override() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let mut receiver = service.subscribe_caret_styles();
        let root = config_fixture("set-cursor-style");
        fs::write(
            root.join("init.js"),
            r#"
            import { clientSetCursorStyle } from "clay:editor";
            clientSetCursorStyle({ shape: "underline", blink: "phase" });
            "#,
        )
        .unwrap();
        service
            .load_configuration_from_root(root)
            .await
            .expect("caret style configuration loads");
        let style = receiver
            .recv()
            .await
            .expect("caret override lane delivers")
            .expect("override is set, not cleared");
        assert_eq!(style.shape, crate::protocol::CaretShape::Underline);
        assert!(matches!(
            style.blink,
            crate::protocol::BlinkStyle::Phase { .. }
        ));
        // Current-value store feeds connection initial sync / lag replay.
        assert_eq!(service.caret_style_override(), Some(style));
    }

    /// Phase 22.1 task 10: `setPaneFocusPolicy` from user configuration
    /// publishes the validated preference on the shell-preferences lane.
    #[tokio::test]
    async fn set_pane_focus_policy_publishes_shell_preferences() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let mut receiver = service.subscribe_shell_preferences();
        let root = config_fixture("set-pane-focus-policy");
        fs::write(
            root.join("init.js"),
            r#"
            import { setPaneFocusPolicy } from "clay:shell";
            const summary = setPaneFocusPolicy({ paneFocusPolicy: "cursor" });
            if (summary.paneFocusPolicy !== "cursor") {
                throw new Error("expected cursor summary");
            }
            "#,
        )
        .unwrap();
        service
            .load_configuration_from_root(root)
            .await
            .expect("pane focus policy configuration loads");
        let preferences = receiver
            .recv()
            .await
            .expect("shell preferences lane delivers");
        assert_eq!(preferences.pane_focus_policy, "cursor");
        // Current-value store feeds connection initial sync / lag replay.
        assert_eq!(service.shell_preferences().pane_focus_policy, "cursor");
    }

    /// Phase 22.1 task 10: unknown pane-focus values are rejected at the
    /// configuration boundary with an actionable diagnostic.
    #[tokio::test]
    async fn set_pane_focus_policy_rejects_unknown_values() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let root = config_fixture("set-pane-focus-policy-invalid");
        fs::write(
            root.join("init.js"),
            r#"
            import { setPaneFocusPolicy } from "clay:shell";
            setPaneFocusPolicy({ paneFocusPolicy: "hover" });
            "#,
        )
        .unwrap();
        let result = service.load_configuration_from_root(root).await;
        assert!(
            result.is_err(),
            "unknown pane focus value must not silently evaluate, got {result:?}"
        );
    }

    /// Phase 22.1 task 10: with no `setPaneFocusPolicy` call the store keeps
    /// the `click` default for connection initial sync.
    #[tokio::test]
    async fn shell_preferences_default_to_click_when_unset() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let root = config_fixture("shell-preferences-default");
        fs::write(root.join("init.js"), "// no shell configuration\n").unwrap();
        service
            .load_configuration_from_root(root)
            .await
            .expect("empty configuration loads");
        assert_eq!(service.shell_preferences().pane_focus_policy, "click");
    }

    /// Follow-up round (`editor-control`): third-party packages pass the same
    /// gate (shared op state, mode-scoped); callers without any package
    /// context are denied in the third-party domain.
    #[tokio::test]
    async fn third_party_editor_control_gate_requires_declared_mode() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let mut receiver = service.subscribe_editor_commands();
        let editor_control = crate::packages::permissions::PackagePermission::EditorControl;
        let register = crate::packages::permissions::PackagePermission::ModeRegistration;
        let activate = crate::packages::permissions::PackagePermission::ModeActivation;

        // A trusted package owns and activates the mode first.
        let owner = test_package_json(
            "@clay/fixture-tp-owner",
            "fixturetpowner",
            &["mode-registration", "mode-activation"],
            serde_json::json!({}),
        );
        evaluate_as_trusted_package(
            &service,
            owner,
            vec![register, activate],
            r#"
            import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
            serverRegisterModePattern({
                modeId: "fixturetpowner",
                displayName: "Fixture TP",
                defaultFontRole: "proportional",
                extensions: ["ftp"],
                editorRules: { tabSpaces: 4 }
            });
            const classification = serverClassifyDocument({ documentId: 74, path: "a.ftp" });
            serverActivateClassifiedMode(classification, { path: "a.ftp" });
            "#,
        )
        .await
        .expect("trusted owner activates the fixture mode");

        // Third-party package declaring the active mode: allowed.
        let mut user = test_package_json(
            "@tp/editor-user",
            "editoruser",
            &["editor-control"],
            serde_json::json!({}),
        );
        user["clay"]["editorControl"] = serde_json::json!({ "modes": ["fixturetpowner"] });
        let evaluation = evaluate_as_package(
            &service,
            user,
            vec![editor_control],
            r#"
            const moved = JSON.parse(Deno.core.ops.op_clay_editor_move_cursor(
                JSON.stringify({ direction: "prevWordStart" })));
            if (moved.commandId !== "editor.clientMoveCursor" || moved.direction !== "prevWordStart") {
                throw new Error("unexpected descriptor: " + moved.commandId + "/" + moved.direction);
            }
            const executed = JSON.parse(Deno.core.ops.op_clay_editor_execute_command(
                JSON.stringify({ commandId: "editor.clientSetSelection.selectLine" })));
            if (!executed.requested) {
                throw new Error("expected requested=true");
            }
            "#,
        )
        .await;
        assert!(
            evaluation.is_ok(),
            "third-party caller in declared mode must pass the gate: {evaluation:?}"
        );
        let request = tokio::time::timeout(std::time::Duration::from_millis(500), receiver.recv())
            .await
            .expect("third-party execution request reaches the connection lane")
            .expect("editor command channel stays open");
        assert_eq!(request.command_id, "editor.clientSetSelection.selectLine");
        assert_eq!(request.package_prefix, "editoruser");
        assert_eq!(request.mode_id, "fixturetpowner");

        // Third-party package declaring a different mode: denied.
        let mut other = test_package_json(
            "@tp/editor-other",
            "editorother",
            &["editor-control"],
            serde_json::json!({}),
        );
        other["clay"]["editorControl"] = serde_json::json!({ "modes": ["other.mode"] });
        let evaluation = evaluate_as_package(
            &service,
            other,
            vec![editor_control],
            r#"
            let error = "";
            try {
                Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
            } catch (e) {
                error = String(e && e.message ? e.message : e);
            }
            if (!error.includes("mode_not_declared")) {
                throw new Error("expected mode_not_declared denial, got: " + (error || "allowed"));
            }
            "#,
        )
        .await;
        assert!(
            evaluation.is_ok(),
            "undeclared-mode third-party case must deny inside JS: {evaluation:?}"
        );

        // Third-party evaluation without any package context: denied.
        let evaluation = service
            .evaluate_third_party_module(
                r#"
                let error = "";
                try {
                    Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
                } catch (e) {
                    error = String(e && e.message ? e.message : e);
                }
                if (!error.includes("package context")) {
                    throw new Error("expected package-context denial, got: " + (error || "allowed"));
                }
                "#,
            )
            .await;
        assert!(
            evaluation.is_ok(),
            "package-less third-party call must deny inside JS: {evaluation:?}"
        );
    }

    #[tokio::test]
    async fn language_commands_are_package_prefixed_and_server_first_with_provenance() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverListCommands } from "clay:commands";
                await loadPackage("@clay/rust");
                await loadPackage("@clay/typescript");
                await loadPackage("@clay/javascript");
                await loadPackage("@clay/markdown");
                Deno.core.ops.op_clay_runtime_record(JSON.stringify(serverListCommands()));
                "#,
            )
            .await
            .unwrap();

        let commands: serde_json::Value = serde_json::from_str(&result.op_records[0]).unwrap();
        for (command_id, package_name, api_prefix, declaration_source, load_source) in [
            (
                "rust.toggleLineComment",
                "@clay/rust",
                "rust",
                include_str!("../../packages/rust/dist/index.js"),
                include_str!("../../packages/rust/dist/load.js"),
            ),
            (
                "typescript.toggleLineComment",
                "@clay/typescript",
                "typescript",
                include_str!("../../packages/typescript/dist/index.js"),
                include_str!("../../packages/typescript/dist/load.js"),
            ),
            (
                "javascript.toggleLineComment",
                "@clay/javascript",
                "javascript",
                include_str!("../../packages/javascript/dist/index.js"),
                include_str!("../../packages/javascript/dist/load.js"),
            ),
            (
                "markdown.toggleComment",
                "@clay/markdown",
                "markdown",
                include_str!("../../packages/markdown/dist/index.js"),
                include_str!("../../packages/markdown/dist/load.js"),
            ),
        ] {
            let command = commands
                .as_array()
                .unwrap()
                .iter()
                .find(|command| command["commandId"] == command_id)
                .unwrap_or_else(|| panic!("missing {command_id}"));
            assert_eq!(command["packageName"], package_name);
            assert_eq!(command["packageVersion"], "0.1.0");
            assert_eq!(command["apiPrefix"], api_prefix);
            assert!(declaration_source.contains(command_id));
            assert!(
                declaration_source.contains("routingPolicy: \"server-first\"")
                    || declaration_source.contains("routingPolicy: \"ServerFirst\"")
            );
            assert!(declaration_source.contains("permissions: []"));
            assert!(load_source.contains("routingPolicy:"));
            assert!(load_source.contains("permissions:"));
        }

        for (component_id, package_name, api_prefix) in [
            ("rust.status.mode", "@clay/rust", "rust"),
            ("typescript.status.mode", "@clay/typescript", "typescript"),
            ("javascript.status.mode", "@clay/javascript", "javascript"),
            ("markdown.status.mode", "@clay/markdown", "markdown"),
        ] {
            let component = result
                .ui_contributions
                .components
                .iter()
                .find(|component| component.id == component_id)
                .unwrap_or_else(|| panic!("missing {component_id}"));
            assert_eq!(component.root_kind, "statusItem");
            assert_eq!(component.provenance.package_name, package_name);
            assert_eq!(component.provenance.package_version, "0.1.0");
            assert_eq!(component.provenance.api_prefix, api_prefix);
        }
    }

    #[test]
    fn language_mode_registration_has_no_per_language_rust_branch() {
        let sources = [
            include_str!("ops/modes.rs"),
            include_str!("../packages/modes.rs"),
            include_str!("../packages/commands.rs"),
        ];
        for source in sources {
            for mode in ["rust", "typescript", "javascript", "markdown"] {
                assert!(!source.contains(&format!("mode_id == \"{mode}\"")));
                assert!(!source.contains(&format!("mode_id == {mode:?}")));
            }
        }
    }

    #[tokio::test]
    async fn build_code_editing_manifest_produces_valid_editor_rules() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { buildCodeEditingManifest } from "clay:behavior";
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";

                // @clay/javascript now uses buildCodeEditingManifest for its editor rules.
                // Loading the package exercises the manifest validator; classifying a
                // matching document proves the mode pattern (built from helper output)
                // was registered successfully.
                const summary = await loadPackage("@clay/javascript");
                const classification = serverClassifyDocument({ documentId: 7, path: "src/index.js" });

                const rules = buildCodeEditingManifest({
                  indentSize: 4,
                  lineComment: "//",
                  enter: { kind: "continueLineMarkers", markers: ["-"], exitOnEmptyItem: true },
                  pairs: [{ open: "(", close: ")" }],
                  electricOutdentCharacters: ["}", ")", "]", "xx", "}"],
                  autocompleteTriggers: [".", "::", ":", ":"]
                });

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                  modeId: classification.modeId,
                  apiPrefix: classification.apiPrefix,
                  packageName: classification.packageName,
                  packageVersion: classification.packageVersion,
                  summaryModes: summary.modes,
                  rulesEnterKind: rules.enter.kind,
                  rulesTabSpaces: rules.tabSpaces,
                  rulesPairCount: rules.pairs.length,
                  rulesCommentCount: rules.comments.length,
                  rulesElectricCount: rules.electricCharacters.length,
                  rulesAutocompleteCount: rules.autocompleteTriggers.length
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["modeId"], "javascript");
        assert_eq!(parsed["apiPrefix"], "javascript");
        assert_eq!(parsed["packageName"], "@clay/javascript");
        assert_eq!(parsed["packageVersion"], "0.1.0");
        assert_eq!(parsed["rulesEnterKind"], "continueLineMarkers");
        assert_eq!(parsed["rulesTabSpaces"], 4);
        assert_eq!(parsed["rulesPairCount"], 1);
        assert_eq!(parsed["rulesCommentCount"], 1);
        assert_eq!(parsed["rulesElectricCount"], 3);
        assert_eq!(parsed["rulesAutocompleteCount"], 2);
    }

    #[tokio::test]
    async fn language_packages_classify_with_core_fallbacks_and_no_conflicts() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";
                import { serverListCompletionProvidersForTrigger } from "clay:completion";

                await loadPackage("@clay/rust");
                await loadPackage("@clay/typescript");
                await loadPackage("@clay/javascript");

                const classifications = {
                  rust: serverClassifyDocument({ documentId: 1, path: "src/main.rs" }),
                  typescript: serverClassifyDocument({ documentId: 2, path: "src/index.ts" }),
                  javascript: serverClassifyDocument({ documentId: 3, path: "src/index.js" }),
                  plainText: serverClassifyDocument({ documentId: 4, path: "README.txt" }),
                  unknownCode: serverClassifyDocument({ documentId: 5, path: "prog.py" }),
                };

                const commands = serverListCommands();
                const commandIds = commands.map((command) => command.commandId).sort();
                const dotProviders = serverListCompletionProvidersForTrigger({ trigger: "." });
                const providerIds = dotProviders.providers.map((provider) => provider.id).sort();

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                  classifications,
                  commandIds,
                  providerIds,
                  commandCount: commands.length,
                  providerCount: dotProviders.providers.length
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");

        // Package-declared modes win over core.code for known extensions.
        assert_eq!(parsed["classifications"]["rust"]["modeId"], "rust");
        assert_eq!(
            parsed["classifications"]["typescript"]["modeId"],
            "typescript"
        );
        assert_eq!(
            parsed["classifications"]["javascript"]["modeId"],
            "javascript"
        );

        // Plain text falls back to core.text; unmatched code-like extension falls back to core.code.
        assert_eq!(
            parsed["classifications"]["plainText"]["modeId"],
            "core.text"
        );
        assert_eq!(
            parsed["classifications"]["unknownCode"]["modeId"],
            "core.code"
        );

        // No duplicate command or provider IDs across packages.
        assert_eq!(parsed["commandCount"], 3);
        assert_eq!(
            parsed["commandIds"],
            serde_json::json!([
                "javascript.toggleLineComment",
                "rust.toggleLineComment",
                "typescript.toggleLineComment"
            ])
        );
        assert_eq!(parsed["providerCount"], 5);
        assert_eq!(
            parsed["providerIds"],
            serde_json::json!([
                "javascript.keywords",
                "rust.keywords",
                "rust.snippets",
                "typescript.keywords",
                "typescript.snippets"
            ])
        );
    }

    #[tokio::test]
    async fn language_package_classification_is_deterministic_across_load_orders() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;

        for (first, second, third) in [
            ("@clay/rust", "@clay/typescript", "@clay/javascript"),
            ("@clay/javascript", "@clay/rust", "@clay/typescript"),
            ("@clay/typescript", "@clay/javascript", "@clay/rust"),
        ] {
            let source = format!(
                r#"
                import {{ loadPackage }} from "clay:packages";
                import {{ serverClassifyDocument }} from "clay:modes";

                await loadPackage("{}");
                await loadPackage("{}");
                await loadPackage("{}");

                const rust = serverClassifyDocument({{ documentId: 10, path: "lib.rs" }});
                const ts = serverClassifyDocument({{ documentId: 11, path: "app.ts" }});
                const js = serverClassifyDocument({{ documentId: 12, path: "app.js" }});

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({{
                  rust: rust.modeId,
                  typescript: ts.modeId,
                  javascript: js.modeId
                }}));
                "#,
                first, second, third
            );
            let result = ClayJsRuntimeService::default()
                .evaluate_controlled_module_for_document(source, 88)
                .await
                .unwrap();

            let record = result.op_records.into_iter().next().expect("one record");
            let parsed: serde_json::Value =
                serde_json::from_str(&record).expect("valid JSON record");
            assert_eq!(parsed["rust"], "rust");
            assert_eq!(parsed["typescript"], "typescript");
            assert_eq!(parsed["javascript"], "javascript");
        }
    }

    #[tokio::test]
    async fn language_package_rejects_unauthorized_completion_provider() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverRegisterCompletionProvider } from "clay:completion";

                serverRegisterCompletionProvider({
                  providerId: "evil.keywords",
                  triggerCharacters: ["."]
                });
                "#,
            )
            .await
            .unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("packages.no_active_package"),
            "expected no-active-package provenance error, got: {message}"
        );
    }

    #[tokio::test]
    async fn primitive_facades_return_actionable_validation_errors() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverValidatePackagePermissions } from "clay:packages";
                serverValidatePackagePermissions(["network"]);
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("packages.prohibited_authority"));
        assert!(error.to_string().contains("network"));
    }

    #[tokio::test]
    async fn primitive_configuration_facades_promote_package_options_only() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { setPackageOption, setModePreference, setDecorationTheme, setParsePolicy } from "clay:configuration";
                if ([setPackageOption, setModePreference, setDecorationTheme, setParsePolicy].some((api) => typeof api !== "function")) {
                  throw new Error("configuration primitive facade export missing");
                }
                setModePreference({ modeId: "markdown", source: "init-js" });
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error
                .to_string()
                .contains("configuration.setModePreference is planned")
        );
    }

    #[tokio::test]
    async fn markdown_large_file_parse_policy_rejects_unsafe_values() {
        for (name, policy_fields, expected) in [
            (
                "zero timeout",
                "timeoutMs: 0, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 30 * 1024 * 1024",
                "timeoutMs must be between 1 and 5000",
            ),
            (
                "oversized timeout",
                "timeoutMs: 5001, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 30 * 1024 * 1024",
                "timeoutMs must be between 1 and 5000",
            ),
            (
                "zero cache budget",
                "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 0",
                "window and memory budgets must be non-zero",
            ),
            (
                "window larger than cache budget",
                "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 1024",
                "window and memory budgets must be non-zero",
            ),
            (
                "unbounded cache budget",
                "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 64 * 1024 * 1024",
                "window and memory budgets must be non-zero",
            ),
        ] {
            let source = format!(
                r#"
                import {{ serverRegisterParseHandler }} from "clay:parse";
                serverRegisterParseHandler({{
                  mode: "markdown",
                  parseUnit: "line-group",
                  viewportPriority: true,
                  {policy_fields}
                }});
                "#
            );
            let service = ClayJsRuntimeService::default();
            let error = evaluate_as_package(
                &service,
                test_package_json(
                    "@clay/markdown-policy",
                    "markdown",
                    &["parse-document"],
                    serde_json::json!({}),
                ),
                vec![crate::packages::permissions::PackagePermission::ParseDocument],
                &source,
            )
            .await
            .unwrap_err();

            assert!(
                matches!(error, ClayRuntimeError::Runtime(_)),
                "{name} should fail in the runtime"
            );
            assert!(
                error.to_string().contains(expected),
                "{name} should reject unsafe parse policy with `{expected}`, got {error}"
            );
        }
    }

    #[tokio::test]
    async fn phase18_parse_and_decoration_facades_are_runtime_backed() {
        let service = ClayJsRuntimeService::default();
        let result = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/markdown-phase18",
                "markdown",
                &["parse-document", "render-decorations"],
                serde_json::json!({}),
            ),
            vec![
                crate::packages::permissions::PackagePermission::ParseDocument,
                crate::packages::permissions::PackagePermission::RenderDecorations,
            ],
            r#"
                import { serverPublishDecorations } from "clay:decorations";
                import { serverRegisterParseHandler } from "clay:parse";
                const handler = serverRegisterParseHandler({
                  mode: "markdown",
                  parseUnit: "line-group",
                  viewportPriority: true,
                });
                const decorations = serverPublishDecorations({
                  documentId: 1,
                  documentVersion: 1,
                  behaviorVersion: 1,
                  viewport: { byteStart: 0, byteEnd: 12 },
                  spans: [{ byteStart: 0, byteEnd: 5, kind: "syntax", styleToken: "markup.inline-code", fontRole: "monospace", priority: 10 }],
                });
                Deno.core.ops.op_clay_runtime_record(`${handler.mode}:${decorations.publishedSpanCount}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(result.op_records, vec!["markdown:1"]);
        assert_eq!(result.parse_handlers.len(), 1);
        assert_eq!(
            result.published_decoration_set.unwrap().spans[0].font_role,
            Some(crate::protocol::DocumentFontRole::Monospace)
        );
    }

    #[tokio::test]
    async fn semantic_two_axis_publication_accepts_token_type_and_modifiers() {
        use crate::protocol::{DecorationKind, Modifiers, TokenType};

        let service = ClayJsRuntimeService::default();
        let result = evaluate_as_package(
            &service,
            test_package_json(
                "@org/semantic",
                "semanticpkg",
                &["render-decorations"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::RenderDecorations],
            r#"
                import { serverPublishDecorations } from "clay:decorations";
                const decorations = serverPublishDecorations({
                  documentId: 7,
                  documentVersion: 3,
                  viewport: { byteStart: 0, byteEnd: 16 },
                  spans: [{
                    byteStart: 2,
                    byteEnd: 10,
                    kind: "semantic",
                    tokenType: "Function",
                    modifiers: ["Declaration", "Readonly"],
                    priority: 20,
                  }],
                });
                Deno.core.ops.op_clay_runtime_record(`${decorations.publishedSpanCount}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(result.op_records, vec!["1"]);
        let set = result
            .published_decoration_set
            .expect("semantic set published");
        assert_eq!(set.spans.len(), 1);
        assert_eq!(set.spans[0].kind, DecorationKind::Semantic);
        assert_eq!(set.spans[0].token_type, TokenType::Function);
        assert!(set.spans[0].modifiers.contains(Modifiers::DECLARATION));
        assert!(set.spans[0].modifiers.contains(Modifiers::READONLY));
        assert!(set.spans[0].scope.is_none());
        assert_eq!(set.spans[0].provenance.package_prefix, "semanticpkg");
    }

    #[tokio::test]
    async fn diagnostics_facade_publishes_validated_range_diagnostics() {
        let service = ClayJsRuntimeService::default();
        let result = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/rust-diag",
                "rust",
                &["render-decorations"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::RenderDecorations],
            r#"
                import { serverPublishDiagnostics } from "clay:diagnostics";
                const published = serverPublishDiagnostics({
                  documentId: 7,
                  documentVersion: 3,
                  viewport: { byteStart: 0, byteEnd: 64 },
                  source: "my-parser",
                  spans: [{
                    byteStart: 4,
                    byteEnd: 5,
                    severity: "error",
                    code: "parser.syntax-error",
                    message: "Syntax error",
                  }],
                });
                Deno.core.ops.op_clay_runtime_record(`${published.source}:${published.publishedSpanCount}`);
                "#,
        )
        .await
        .unwrap();

        assert_eq!(result.op_records, vec!["my-parser:1"]);
        let set = result.published_diagnostic_set.expect("diagnostic set");
        assert_eq!(set.source, "my-parser");
        assert_eq!(set.spans.len(), 1);
        assert_eq!(
            set.spans[0].severity,
            crate::protocol::DiagnosticSeverity::Error
        );
        assert_eq!(set.provenance.package_prefix, "rust");
    }

    #[tokio::test]
    async fn diagnostics_publication_rejects_missing_permission_or_bad_provenance() {
        // No executing-package context at all: raw/config code cannot publish.
        let missing = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverPublishDiagnostics } from "clay:diagnostics";
                serverPublishDiagnostics({
                  documentId: 1,
                  documentVersion: 1,
                  viewport: { byteStart: 0, byteEnd: 8 },
                  source: "my-parser",
                  spans: [{ byteStart: 1, byteEnd: 2, severity: "error", code: "x", message: "y" }],
                });
                "#,
            )
            .await
            .unwrap_err();
        assert!(
            missing.to_string().contains("packages.no_active_package"),
            "publication without package context must fail, got {missing}"
        );

        // Enabled package whose approved capabilities were shrunk below
        // render-decorations.
        let service = ClayJsRuntimeService::default();
        let error = evaluate_as_package(
            &service,
            test_package_json("@org/unapproved", "unapproved", &[], serde_json::json!({})),
            vec![],
            r#"
            import { serverPublishDiagnostics } from "clay:diagnostics";
            serverPublishDiagnostics({
              documentId: 1,
              documentVersion: 1,
              viewport: { byteStart: 0, byteEnd: 8 },
              source: "my-parser",
              spans: [{ byteStart: 1, byteEnd: 2, severity: "error", code: "x", message: "y" }],
            });
            "#,
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("packages.missing_permission"),
            "missing render-decorations approval must fail, got {error}"
        );
    }

    #[tokio::test]
    async fn diagnostics_publication_rejects_stale_oversized_or_executable_data() {
        let service = ClayJsRuntimeService::default();
        let package_json = test_package_json(
            "@clay/rust-diag",
            "rust",
            &["render-decorations"],
            serde_json::json!({}),
        );
        let approved = vec![crate::packages::permissions::PackagePermission::RenderDecorations];
        let stale = evaluate_as_package(
            &service,
            package_json.clone(),
            approved.clone(),
            r#"
                import { serverPublishDiagnostics } from "clay:diagnostics";
                serverPublishDiagnostics({
                  documentId: 1,
                  documentVersion: 1,
                  currentDocumentVersion: 2,
                  viewport: { byteStart: 0, byteEnd: 8 },
                  source: "my-parser",
                  spans: [{ byteStart: 1, byteEnd: 2, severity: "error", code: "x", message: "y" }],
                });
                "#,
        )
        .await
        .unwrap_err();
        assert!(
            stale.to_string().contains("diagnostics.publish_failed"),
            "stale version must fail, got {stale}"
        );

        // Executable callback fields are rejected by the facade before any op.
        let executable = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverPublishDiagnostics } from "clay:diagnostics";
                serverPublishDiagnostics({
                  documentId: 1,
                  documentVersion: 1,
                  viewport: { byteStart: 0, byteEnd: 8 },
                  source: "my-parser",
                  spans: [],
                  callback: () => {},
                });
                "#,
            )
            .await
            .unwrap_err();
        assert!(
            executable
                .to_string()
                .contains("diagnostics.invalid_publication"),
            "executable callback must fail, got {executable}"
        );

        let oversized_source = format!(
            r#"
                import {{ serverPublishDiagnostics }} from "clay:diagnostics";
                serverPublishDiagnostics({{
                  documentId: 1,
                  documentVersion: 1,
                  viewport: {{ byteStart: 0, byteEnd: 8 }},
                  source: "my-parser",
                  spans: [{{
                    byteStart: 1,
                    byteEnd: 2,
                    severity: "error",
                    code: "x",
                    message: "{}",
                  }}],
                }});
                "#,
            "m".repeat(2048)
        );
        let oversized = evaluate_as_package(&service, package_json, approved, &oversized_source)
            .await
            .unwrap_err();
        assert!(
            oversized.to_string().contains("diagnostics.publish_failed"),
            "oversized message must fail, got {oversized}"
        );
    }

    #[tokio::test]
    async fn markdown_parser_adapter_publishes_viewport_bounded_decorations() {
        let root = config_fixture("markdown-parser-adapter");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { serverPublishDecorations } from "clay:decorations";
            import { parseMarkdownDecorations, publishMarkdownDecorations } from "clay://packages/@clay/markdown-adapter/parser.js";

            const text = "# Hé 🦀\n\nSome **bold** and *em* and `code`.\n\n```js\nx\n```\n\n1. item\n";
            const markdownTokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Hé 🦀", children: [] },
              { type: "heading_close" },
              { type: "paragraph_open", map: [2, 3] },
              {
                type: "inline",
                map: [2, 3],
                content: "Some **bold** and *em* and `code`.",
                children: [
                  { type: "text", content: "Some " },
                  { type: "strong_open", markup: "**" },
                  { type: "text", content: "bold" },
                  { type: "strong_close", markup: "**" },
                  { type: "text", content: " and " },
                  { type: "em_open", markup: "*" },
                  { type: "text", content: "em" },
                  { type: "em_close", markup: "*" },
                  { type: "text", content: " and " },
                  { type: "code_inline", markup: "`", content: "code" }
                ]
              },
              { type: "paragraph_close" },
              { type: "fence", tag: "code", map: [4, 7], markup: "```", info: "js" },
              { type: "ordered_list_open", map: [8, 9] },
              { type: "list_item_open", map: [8, 9] },
              { type: "paragraph_open", map: [8, 9] },
              { type: "inline", map: [8, 9], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "paragraph_close" },
              { type: "list_item_close" },
              { type: "ordered_list_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            function byteRangeFor(needle, from = 0) {
              const codeUnitStart = text.indexOf(needle, from);
              if (codeUnitStart < 0) throw new Error(`missing fixture needle: ${needle}`);
              return {
                byteStart: utf8ByteLength(text.slice(0, codeUnitStart)),
                byteEnd: utf8ByteLength(text.slice(0, codeUnitStart + needle.length)),
                codeUnitEnd: codeUnitStart + needle.length,
              };
            }
            function requireSpan(styleToken) {
              const span = spans.find((candidate) => candidate.styleToken === styleToken);
              if (!span) throw new Error(`missing span ${styleToken} in ${JSON.stringify(spans)}`);
              return span;
            }
            function assertSpan(styleToken, expected) {
              const span = requireSpan(styleToken);
              if (span.byteStart !== expected.byteStart || span.byteEnd !== expected.byteEnd) {
                throw new Error(`${styleToken} expected ${expected.byteStart}:${expected.byteEnd}, got ${span.byteStart}:${span.byteEnd}`);
              }
            }

            const fullViewport = { byteStart: 0, byteEnd: utf8ByteLength(text) };
            const spans = await parseMarkdownDecorations({ text, tokens: markdownTokens, viewport: fullViewport });
            assertSpan("markup.heading.1", { byteStart: 0, byteEnd: utf8ByteLength("# Hé 🦀") });
            assertSpan("markup.strong", byteRangeFor("**bold**"));
            assertSpan("markup.emphasis", byteRangeFor("*em*"));
            assertSpan("markup.inline-code", byteRangeFor("`code`"));
            assertSpan("markup.list-marker", byteRangeFor("1."));
            const fenceStart = byteRangeFor("```js");
            const fenceTerminator = byteRangeFor("\n\n1. item");
            assertSpan("markup.code-block", { byteStart: fenceStart.byteStart, byteEnd: utf8ByteLength(text.slice(0, fenceTerminator.codeUnitEnd - "\n1. item".length)) });
            if (requireSpan("markup.inline-code").fontRole !== "monospace" || requireSpan("markup.code-block").fontRole !== "monospace") {
              throw new Error("Markdown code spans must declare the generic monospace role");
            }

            const listMarker = requireSpan("markup.list-marker");
            const viewportOnlyList = await parseMarkdownDecorations({
              text,
              tokens: markdownTokens,
              viewport: { byteStart: listMarker.byteStart, byteEnd: listMarker.byteEnd },
            });
            if (viewportOnlyList.length !== 1 || viewportOnlyList[0].styleToken !== "markup.list-marker") {
              throw new Error(`viewport filter leaked spans: ${JSON.stringify(viewportOnlyList)}`);
            }

            let parseCalls = 0;
            const fakeMarkdownIt = {
              parse(source, env) {
                parseCalls += 1;
                if (source !== text || !env) throw new Error("parse received unexpected arguments");
                return markdownTokens;
              },
              render() {
                throw new Error("adapter must not render HTML");
              }
            };
            await parseMarkdownDecorations({ text, markdownIt: fakeMarkdownIt, viewport: fullViewport });
            if (parseCalls !== 1) throw new Error(`expected one markdown-it parse call, got ${parseCalls}`);

            const tokens = spans.map((span) => span.styleToken).sort().join(",");
            const heading = requireSpan("markup.heading.1");
            const published = await publishMarkdownDecorations({ decorations: { serverPublishDecorations } }, {
              text,
              tokens: markdownTokens,
              documentId: 7,
              documentVersion: 3,
              behaviorVersion: 2,
              viewport: fullViewport,
            });
            Deno.core.ops.op_clay_runtime_record(tokens);
            Deno.core.ops.op_clay_runtime_record(`${heading.byteStart}:${heading.byteEnd}:${published.publishedSpanCount}:parseCalls=${parseCalls}`);
            "##,
        )
        .unwrap();

        // The adapter publishes through a package context: register the
        // parser module in the load-entry allowlist for a synthetic package
        // and evaluate the script with that package's host-stamped provenance.
        let service = ClayJsRuntimeService::default();
        service
            .test_op_state()
            .load_entry_allowlist()
            .record_for_package(
                "clay://packages/@clay/markdown-adapter/parser.js",
                fs::canonicalize(root.join("parser.js")).unwrap(),
                fs::canonicalize(&root).unwrap(),
                Some("@clay/markdown-adapter"),
            );
        let source = fs::read_to_string(root.join("init.js")).unwrap();
        let result = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/markdown-adapter",
                "markdown",
                &["parse-document", "render-decorations"],
                serde_json::json!({}),
            ),
            vec![
                crate::packages::permissions::PackagePermission::ParseDocument,
                crate::packages::permissions::PackagePermission::RenderDecorations,
            ],
            &source,
        )
        .await
        .unwrap();

        let tokens = &result.op_records[0];
        for expected in [
            "markup.heading.1",
            "markup.strong",
            "markup.emphasis",
            "markup.inline-code",
            "markup.code-block",
            "markup.list-marker",
        ] {
            assert!(tokens.contains(expected), "missing {expected} in {tokens}");
        }
        assert_eq!(result.op_records[1], "0:10:6:parseCalls=1");
        assert_eq!(result.published_decoration_set.unwrap().spans.len(), 6);
    }

    #[tokio::test]
    async fn markdown_windowed_adapter_offsets_ranges_to_absolute_document_bytes() {
        let root = config_fixture("markdown-windowed-absolute-ranges");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorations } from "./parser.js";

            const windowText = "# Hé 🦀\n\nParagraph **dé** and `cø`.\n";
            const absoluteByteStart = 4096;
            const tokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Hé 🦀", children: [{ type: "text", content: "Hé 🦀" }] },
              { type: "heading_close" },
              { type: "paragraph_open", map: [2, 3] },
              {
                type: "inline",
                map: [2, 3],
                content: "Paragraph **dé** and `cø`.",
                children: [
                  { type: "text", content: "Paragraph " },
                  { type: "strong_open", markup: "**" },
                  { type: "text", content: "dé" },
                  { type: "strong_close", markup: "**" },
                  { type: "text", content: " and " },
                  { type: "code_inline", markup: "`", content: "cø" },
                  { type: "text", content: "." }
                ]
              },
              { type: "paragraph_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            function absoluteRangeFor(needle, from = 0) {
              const start = windowText.indexOf(needle, from);
              if (start < 0) throw new Error(`missing ${needle}`);
              return {
                byteStart: absoluteByteStart + utf8ByteLength(windowText.slice(0, start)),
                byteEnd: absoluteByteStart + utf8ByteLength(windowText.slice(0, start + needle.length))
              };
            }
            function span(styleToken) {
              const found = spans.find((candidate) => candidate.styleToken === styleToken);
              if (!found) throw new Error(`missing ${styleToken} in ${JSON.stringify(spans)}`);
              return found;
            }
            function assertRange(styleToken, range) {
              const found = span(styleToken);
              if (found.byteStart !== range.byteStart || found.byteEnd !== range.byteEnd) {
                throw new Error(`${styleToken} expected ${range.byteStart}:${range.byteEnd}, got ${found.byteStart}:${found.byteEnd}`);
              }
            }

            let parseCalls = 0;
            const fakeMarkdownIt = {
              parse(source, env) {
                parseCalls += 1;
                if (source !== windowText || !env) throw new Error("markdown-it must receive only window text");
                return tokens;
              },
              render() {
                throw new Error("windowed adapter must not render HTML");
              }
            };
            const spans = await parseMarkdownDecorations({
              text: windowText,
              absoluteByteStart,
              baseLine: 120,
              parseWindow: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText), baseLine: 120 },
              viewport: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
              markdownIt: fakeMarkdownIt
            });

            assertRange("markup.heading.1", { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength("# Hé 🦀") });
            assertRange("markup.strong", absoluteRangeFor("**dé**"));
            assertRange("markup.inline-code", absoluteRangeFor("`cø`"));
            Deno.core.ops.op_clay_runtime_record(`${spans.length}:parseCalls=${parseCalls}:${span("markup.strong").byteStart}:${span("markup.inline-code").byteEnd}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["3:parseCalls=1:4118:4135"]);
    }

    #[tokio::test]
    async fn markdown_windowed_adapter_does_not_parse_full_large_document() {
        let root = config_fixture("markdown-windowed-no-full-doc");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorationUpdate } from "./parser.js";

            const windowText = "# Visible\n\n- item\n";
            const absoluteByteStart = 8 * 1024 * 1024;
            const largeDocumentSentinel = "x".repeat(16 * 1024 * 1024);
            const tokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Visible", children: [{ type: "text", content: "Visible" }] },
              { type: "heading_close" },
              { type: "bullet_list_open", map: [2, 3] },
              { type: "list_item_open", map: [2, 3] },
              { type: "paragraph_open", map: [2, 3] },
              { type: "inline", map: [2, 3], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "paragraph_close" },
              { type: "list_item_close" },
              { type: "bullet_list_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            let parseCalls = 0;
            const fakeMarkdownIt = {
              parse(source) {
                parseCalls += 1;
                if (source === largeDocumentSentinel || source.length !== windowText.length) {
                  throw new Error(`received unbounded source length ${source.length}`);
                }
                return tokens;
              }
            };
            const update = await parseMarkdownDecorationUpdate({
              documentId: 7,
              documentVersion: 3,
              behaviorVersion: 2,
              viewport: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
              parseWindows: [{
                text: windowText,
                byteStart: absoluteByteStart,
                byteEnd: absoluteByteStart + utf8ByteLength(windowText),
                baseLine: 900
              }],
              markdownIt: fakeMarkdownIt
            });
            if (update.spans.length !== 2) throw new Error(`expected heading and list marker spans, got ${JSON.stringify(update.spans)}`);
            Deno.core.ops.op_clay_runtime_record(`${update.viewport.byteStart}:${update.viewport.byteEnd}:${update.spans.length}:parseCalls=${parseCalls}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["8388608:8388626:2:parseCalls=1"]);
    }

    #[tokio::test]
    async fn markdown_windowed_adapter_preserves_fence_and_list_context() {
        let root = config_fixture("markdown-windowed-fence-list-context");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorations } from "./parser.js";

            const windowText = "```js\nconst visible = 1;\n```\n\n- item\n";
            const absoluteByteStart = 2048;
            const tokens = [
              { type: "fence", tag: "code", map: [0, 3], markup: "```", info: "js" },
              { type: "bullet_list_open", map: [4, 5] },
              { type: "list_item_open", map: [4, 5] },
              { type: "paragraph_open", map: [4, 5] },
              { type: "inline", map: [4, 5], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "paragraph_close" },
              { type: "list_item_close" },
              { type: "bullet_list_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            const visibleStart = absoluteByteStart + utf8ByteLength("```js\n");
            const visibleEnd = absoluteByteStart + utf8ByteLength(windowText.slice(0, windowText.indexOf(" item")));
            const spans = await parseMarkdownDecorations({
              text: windowText,
              tokens,
              absoluteByteStart,
              parseWindow: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
              viewport: { byteStart: visibleStart, byteEnd: visibleEnd }
            });
            const fence = spans.find((span) => span.styleToken === "markup.code-block");
            const list = spans.find((span) => span.styleToken === "markup.list-marker");
            if (!fence || fence.byteStart !== visibleStart || fence.byteEnd > visibleEnd) {
              throw new Error(`fence span was not clipped to the visible viewport: ${JSON.stringify(spans)}`);
            }
            if (!list || list.byteStart !== visibleEnd - 1 || list.byteEnd !== visibleEnd) {
              throw new Error(`list marker did not survive guard-window parsing: ${JSON.stringify(spans)}`);
            }
            Deno.core.ops.op_clay_runtime_record(`${spans.length}:${fence.byteStart}:${fence.byteEnd}:${list.byteStart}:${list.byteEnd}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["2:2054:2077:2078:2079"]);
    }

    #[tokio::test]
    async fn markdown_large_file_status_reports_windowed_highlighting() {
        let root = config_fixture("markdown-large-file-windowed-status");
        // index.js re-exports from ./load.js (markdownLoadMode fallback entry),
        // so the whole dist module graph must be copied for sdui.js to load.
        for file_name in ["index.js", "sdui.js", "load.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r##"
            import { markdownPreviewStatusModel } from "./sdui.js";

            const model = markdownPreviewStatusModel({
              documentByteLength: 16 * 1024 * 1024,
              documentPath: "C:/Users/alice/work/large.md"
            });
            if (model.status.highlightingState !== "windowed") throw new Error(JSON.stringify(model));
            if (model.status.fileTier !== "large") throw new Error(JSON.stringify(model));
            Deno.core.ops.op_clay_runtime_record(`${model.documentPath}:${model.status.parse}:${model.status.decorations}:${model.status.highlightingState}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec![
                "large.md:windowed visible syntax current:visible and near-viewport chunks current:windowed"
            ]
        );
    }

    #[tokio::test]
    async fn markdown_large_file_budget_exhaustion_falls_back_to_plain_text() {
        let root = config_fixture("markdown-large-file-plain-text-fallback");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorationUpdate } from "./parser.js";

            const windowText = "# Visible\n\n- item\n";
            const fakeMarkdownIt = {
              parse() {
                throw new Error("plain-text fallback must not invoke markdown-it");
              }
            };
            const update = await parseMarkdownDecorationUpdate({
              documentId: 9,
              documentVersion: 4,
              behaviorVersion: 2,
              viewport: { byteStart: 0, byteEnd: 18 },
              parseWindows: [{ text: windowText, byteStart: 0, byteEnd: 18, baseLine: 0 }],
              memoryBudgetBytes: 1,
              markdownIt: fakeMarkdownIt
            });
            if (update.spans.length !== 0) throw new Error(`fallback must clear spans: ${JSON.stringify(update.spans)}`);
            if (update.status.highlightingState !== "plain-text-fallback") throw new Error(JSON.stringify(update.status));
            Deno.core.ops.op_clay_runtime_record(`${update.spans.length}:${update.status.highlightingState}:${update.status.reason}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["0:plain-text-fallback:budget-exceeded"]
        );
    }

    #[tokio::test]
    async fn markdown_degraded_status_contains_no_document_text_or_paths() {
        let root = config_fixture("markdown-degraded-status-sanitized");
        for file_name in ["index.js", "sdui.js", "load.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r##"
            import { markdownPreviewStatusModel } from "./sdui.js";

            const model = markdownPreviewStatusModel({
              documentByteLength: 6 * 1024 * 1024,
              parserTimedOut: true,
              documentPath: "C:/Users/alice/secrets/project.md",
              diagnostic: "C:/Users/alice/secrets/project.md first line SECRET_DOCUMENT_TEXT"
            });
            const encoded = JSON.stringify(model);
            for (const forbidden of ["C:/", "Users/alice", "secrets/project.md", "SECRET_DOCUMENT_TEXT"]) {
              if (encoded.includes(forbidden)) throw new Error(`unsanitized status: ${encoded}`);
            }
            if (model.status.highlightingState !== "degraded") throw new Error(encoded);
            Deno.core.ops.op_clay_runtime_record(`${model.documentPath}:${model.status.parse}:${model.status.highlightingState}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["project.md:degraded; visible syntax refresh delayed:degraded"]
        );
    }

    #[tokio::test]
    async fn markdown_it_adapter_large_fixture_span_counts_are_stable() {
        let root = config_fixture("markdown-adapter-large-counts");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorations } from "./parser.js";

            const blockCount = 192;
            let text = "";
            const tokens = [];
            for (let index = 0; index < blockCount; index += 1) {
              const startLine = text.split("\n").length - 1;
              text += `# Heading ${index}\n\n`;
              text += `Paragraph ${index} has **strong**, *emphasis*, and \`code\`.\n\n`;
              text += "```js\nconst value = 1;\n```\n\n";
              text += `- bullet ${index}\n1. ordered ${index}\n\n`;
              tokens.push(
                { type: "heading_open", tag: "h1", map: [startLine, startLine + 1] },
                { type: "inline", map: [startLine, startLine + 1], content: `Heading ${index}`, children: [{ type: "text", content: `Heading ${index}` }] },
                { type: "heading_close" },
                { type: "paragraph_open", map: [startLine + 2, startLine + 3] },
                {
                  type: "inline",
                  map: [startLine + 2, startLine + 3],
                  content: `Paragraph ${index} has **strong**, *emphasis*, and \`code\`.`,
                  children: [
                    { type: "text", content: `Paragraph ${index} has ` },
                    { type: "strong_open", markup: "**" },
                    { type: "text", content: "strong" },
                    { type: "strong_close", markup: "**" },
                    { type: "text", content: ", " },
                    { type: "em_open", markup: "*" },
                    { type: "text", content: "emphasis" },
                    { type: "em_close", markup: "*" },
                    { type: "text", content: ", and " },
                    { type: "code_inline", markup: "`", content: "code" },
                    { type: "text", content: "." }
                  ]
                },
                { type: "paragraph_close" },
                { type: "fence", tag: "code", map: [startLine + 4, startLine + 7], markup: "```", info: "js" },
                { type: "bullet_list_open", map: [startLine + 8, startLine + 9] },
                { type: "list_item_open", map: [startLine + 8, startLine + 9] },
                { type: "paragraph_open", map: [startLine + 8, startLine + 9] },
                { type: "inline", map: [startLine + 8, startLine + 9], content: `bullet ${index}`, children: [{ type: "text", content: `bullet ${index}` }] },
                { type: "paragraph_close" },
                { type: "list_item_close" },
                { type: "bullet_list_close" },
                { type: "ordered_list_open", map: [startLine + 9, startLine + 10] },
                { type: "list_item_open", map: [startLine + 9, startLine + 10] },
                { type: "paragraph_open", map: [startLine + 9, startLine + 10] },
                { type: "inline", map: [startLine + 9, startLine + 10], content: `ordered ${index}`, children: [{ type: "text", content: `ordered ${index}` }] },
                { type: "paragraph_close" },
                { type: "list_item_close" },
                { type: "ordered_list_close" }
              );
            }
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            const viewport = { byteStart: 0, byteEnd: utf8ByteLength(text) };
            const first = await parseMarkdownDecorations({ text, tokens, viewport });
            const second = await parseMarkdownDecorations({ text, tokens, viewport });
            if (first.length !== second.length) throw new Error(`unstable span counts: ${first.length} != ${second.length}`);
            if (first.length !== blockCount * 7) throw new Error(`expected ${blockCount * 7} spans, got ${first.length}`);
            const byToken = new Map();
            for (const span of first) byToken.set(span.styleToken, (byToken.get(span.styleToken) ?? 0) + 1);
            for (const [token, expected] of [
              ["markup.heading.1", blockCount],
              ["markup.strong", blockCount],
              ["markup.emphasis", blockCount],
              ["markup.inline-code", blockCount],
              ["markup.code-block", blockCount],
              ["markup.list-marker", blockCount * 2],
            ]) {
              if (byToken.get(token) !== expected) throw new Error(`${token} expected ${expected}, got ${byToken.get(token)}`);
            }
            Deno.core.ops.op_clay_runtime_record(`${first.length}:${byToken.get("markup.list-marker")}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["1344:384"]);
    }

    // Plan 061 task 15: trusted init.js configuration APIs for third-party
    // package loading, replacement, and adoption-state diagnostics.

    /// Plan 061 task 15: one-line `loadPackage` from a trusted configuration
    /// must fail with a clear pending-adoption diagnostic when no durable
    /// approval exists — init.js cannot bypass the pre-execution gate.
    #[tokio::test]
    async fn third_party_config_load_fails_with_pending_adoption_diagnostic() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let root = config_fixture("third-party-config-adoption")
            .join(format!("pending-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        write_loadable_package(
            &root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => ({
                  status: "ok",
                  items: [{ label: "config-loaded", insertText: "config-loaded" }]
                })
              }
            });
            export default function load() {}
            "#,
        );
        let mut package_json = loadable_package_fixture("@vendor/config-adopt", "cfgadopt");
        package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
        package_json["clay"]["contributions"] = serde_json::json!({
            "completionProviders": [{
                "id": "cfgadopt.provider",
                "triggerCharacters": ["."],
                "budgets": { "timeoutMs": 500, "maxItems": 8 }
            }]
        });
        {
            let op_state = service.test_op_state();
            let mut locked = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            locked
                .install_from_value_at_root_with_spec(
                    package_json,
                    root.clone(),
                    "local:config-adopt-test",
                )
                .expect("synthetic package installs");
            // Intentionally skip approve_package — leaving it in Pending state.
            locked
                .authorize_package(
                    "@vendor/config-adopt",
                    vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .expect("synthetic package authorizes");
        }
        let config_root = config_fixture("third-party-config-adoption").join("config");
        fs::create_dir_all(&config_root).unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@vendor/config-adopt");
            "#,
        )
        .unwrap();
        let error = service
            .load_configuration_from_root(config_root)
            .await
            .expect_err("unadopted third-party package must not execute via config");
        let message = error.to_string();
        assert!(
            message.contains("adoption") || message.contains("missing"),
            "expected adoption diagnostic, got: {message}"
        );
        assert!(
            !service
                .test_op_state()
                .package_service()
                .lock()
                .unwrap()
                .inspect("@vendor/config-adopt")
                .unwrap()
                .is_enabled,
            "pending package must stay disabled after failed config load"
        );
        let _ = fs::remove_dir_all(
            config_fixture("third-party-config-adoption")
                .join(format!("pending-{}", std::process::id())),
        );
    }

    /// Plan 061 task 15: after CLI adoption, a one-line `loadPackage` from
    /// `init.js` succeeds — the package executes in the third-party runtime
    /// and its registration payload is absorbed into the trusted worker.
    #[tokio::test]
    async fn third_party_config_load_succeeds_after_cli_adoption() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let root = config_fixture("third-party-config-adoption")
            .join(format!("adopted-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        write_loadable_package(
            &root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => ({
                  status: "ok",
                  items: [{ label: "config-loaded", insertText: "config-loaded" }]
                })
              }
            });
            export default function load() {}
            "#,
        );
        let mut package_json = loadable_package_fixture("@vendor/config-adopt-ok", "cfgadok");
        package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
        package_json["clay"]["contributions"] = serde_json::json!({
            "completionProviders": [{
                "id": "cfgadok.provider",
                "triggerCharacters": ["."],
                "budgets": { "timeoutMs": 500, "maxItems": 8 }
            }]
        });
        {
            let op_state = service.test_op_state();
            let mut locked = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            locked
                .install_from_value_at_root_with_spec(
                    package_json,
                    root.clone(),
                    "local:config-adopt-ok",
                )
                .expect("synthetic package installs");
            locked
                .authorize_package(
                    "@vendor/config-adopt-ok",
                    vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .expect("synthetic package authorizes");
            // Simulate the CLI adoption step that must precede config load.
            locked
                .approve_package("@vendor/config-adopt-ok", "cli")
                .expect("CLI adoption succeeds");
        }
        let config_root = config_fixture("third-party-config-adoption").join("config-ok");
        fs::create_dir_all(&config_root).unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@vendor/config-adopt-ok");
            Deno.core.ops.op_clay_runtime_record("package-loaded");
            "#,
        )
        .unwrap();
        let result = service
            .load_configuration_from_root(config_root)
            .await
            .expect("adopted third-party package must load via config");
        assert!(
            result.op_records.contains(&"package-loaded".to_string()),
            "config evaluation must complete after third-party load"
        );
        assert!(
            !result.js_completion_providers.is_empty(),
            "absorbed cross-domain registration must include completion provider"
        );
        let _ = fs::remove_dir_all(
            config_fixture("third-party-config-adoption")
                .join(format!("adopted-{}", std::process::id())),
        );
    }

    /// Plan 061 task 15: a stale approval (version drift, scope expansion, or
    /// target replacement) blocks config-load of an approved package — the
    /// adoption gate fails closed and a diagnostic is produced.
    #[tokio::test]
    async fn stale_approval_blocks_config_load_with_clear_diagnostic() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let root = config_fixture("third-party-config-stale")
            .join(format!("stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        write_loadable_package(
            &root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async () => ({
                  status: "ok",
                  items: [{ label: "stale", insertText: "stale" }]
                })
              }
            });
            export default function load() {}
            "#,
        );
        let package_json_v1 = loadable_package_fixture("@vendor/config-stale", "cfgstale");
        {
            let op_state = service.test_op_state();
            let mut locked = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            locked
                .install_from_value_at_root_with_spec(
                    package_json_v1,
                    root.clone(),
                    "local:config-stale",
                )
                .expect("synthetic package v1 installs");
            locked
                .authorize_package(
                    "@vendor/config-stale",
                    Vec::new(),
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .expect("synthetic package authorizes");
            locked
                .approve_package("@vendor/config-stale", "cli")
                .expect("initial adoption succeeds");
        }
        // Update the install to a different version, staling the approval.
        let mut package_json_v2 = loadable_package_fixture("@vendor/config-stale", "cfgstale");
        package_json_v2["version"] = serde_json::json!("0.2.0");
        {
            let op_state = service.test_op_state();
            let mut locked = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            locked
                .install_from_value_at_root_with_spec(
                    package_json_v2,
                    root.clone(),
                    "local:config-stale-v2",
                )
                .expect("synthetic package v2 installs");
        }
        let config_root = config_fixture("third-party-config-stale").join("config");
        fs::create_dir_all(&config_root).unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@vendor/config-stale");
            "#,
        )
        .unwrap();
        let error = service
            .load_configuration_from_root(config_root)
            .await
            .expect_err("stale approval must block config load (version drift beyond adopted)");
        let message = error.to_string();
        assert!(
            message.contains("adoption")
                || message.contains("stale")
                || message.contains("missing"),
            "expected stale-adoption diagnostic, got: {message}"
        );
        let _ = fs::remove_dir_all(
            config_fixture("third-party-config-stale")
                .join(format!("stale-{}", std::process::id())),
        );
    }

    #[test]
    fn keypress_routing_uses_manifest_not_js() {
        let manifest = {
            let service = ClayJsRuntimeService::default();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime
                .block_on(service.evaluate_controlled_module(
                    r#"
                    import { bindKey } from "clay:keybindings";
                    bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
                    "#,
                ))
                .unwrap()
                .behavior_manifest
                .unwrap()
        };
        let state = crate::client::behavior::ClientBehaviorState::new(manifest).unwrap();
        let routed = state.route_key(&crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("s".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            crate::client::behavior::RoutedBehavior::ServerIntent(
                crate::client::behavior::ServerIntentRoute {
                    command_id: "documents.serverSaveDocument".to_string(),
                    routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
                }
            )
        );
    }

    #[test]
    fn keypress_routing_can_reach_client_ui_command_without_js() {
        let manifest = {
            let service = ClayJsRuntimeService::default();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime
                .block_on(service.evaluate_controlled_module(
                    r#"
                    import { bindKey } from "clay:keybindings";
                    bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
                    "#,
                ))
                .unwrap()
                .behavior_manifest
                .unwrap()
        };
        let state = crate::client::behavior::ClientBehaviorState::new(manifest).unwrap();
        let routed = state.route_key(&crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("o".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            crate::client::behavior::RoutedBehavior::ClientUiCommand(
                crate::client::behavior::ClientUiCommandRoute {
                    command_id: "documents.clientOpenFileDialog".to_string(),
                    routing_policy: crate::protocol::RoutingPolicy::ClientUiCommand,
                }
            )
        );
    }

    #[test]
    fn ordinary_typing_does_not_enter_js_runtime() {
        let service = ClayJsRuntimeService::default();

        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn js_runtime_errors_are_typed_not_panics() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(r#"Deno.core.ops.op_clay_runtime_record("");"#)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("runtime.invalid_record"));
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn runtime_syntax_error_reports_diagnostic() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(r#"const broken = ;"#)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, "runtime.syntax_error");
        assert_eq!(
            diagnostic.message,
            "JavaScript syntax error while evaluating server-side configuration."
        );
    }

    #[tokio::test]
    async fn runtime_permission_error_reports_sanitized_diagnostic() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(r#"import "file:///home/example/.config/clay/secret.js";"#)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, "runtime.invalid_import");
        assert!(!diagnostic.message.contains("/home/example"));
        assert!(!diagnostic.message.contains("secret.js"));
    }

    #[tokio::test]
    async fn runtime_op_validation_error_reports_diagnostic() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(r#"Deno.core.ops.op_clay_runtime_record("");"#)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, "runtime.invalid_record");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    }

    /// Helper: call the raw resolver op from a controlled module. The public
    /// `loadPackage` facade is wired in Phase 18.6 task 5; these op-level
    /// tests exercise the resolver directly so the security boundary is
    /// covered before the facade lands.
    async fn resolve_by_specifier(specifier: &str) -> Result<String, String> {
        let source = format!(
            r#"
            const result = Deno.core.ops.op_clay_packages_load_package_by_specifier(
              JSON.stringify({{ specifier: {specifier:?} }})
            );
            globalThis.__clay_result = result;
            "#
        );
        match ClayJsRuntimeService::default()
            .evaluate_controlled_module(source)
            .await
        {
            Ok(_) => Ok("ok".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn loadable_package_fixture(name: &str, api_prefix: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "version": "0.1.0",
            "type": "module",
            "clay": {
                "apiPrefix": api_prefix,
                "entry": "./dist/index.js",
                "loadEntry": "./dist/load.js",
                "capabilities": [],
                "modes": [],
                "docs": "./docs/index.md",
                "apiDependencies": [],
                "performance": {
                    "estimatedManifestBytes": 256,
                    "hotPathPolicy": "no hot-path JS on keypress/paint"
                },
                "contributions": {}
            }
        })
    }

    fn write_loadable_package(root: &Path, load_source: &str) {
        fs::create_dir_all(root.join("dist")).expect("create package dist directory");
        fs::create_dir_all(root.join("docs")).expect("create package docs directory");
        fs::write(root.join("dist/index.js"), "export {};\n").expect("write package entry");
        fs::write(root.join("dist/load.js"), load_source).expect("write package loadEntry");
        fs::write(
            root.join("dist/helper.js"),
            "Deno.core.ops.op_clay_runtime_record(\"helper loaded\"); export {};\n",
        )
        .expect("write package helper");
        fs::write(root.join("docs/index.md"), "# Fixture\n").expect("write package docs");
    }

    async fn evaluate_with_seeded_package(
        specifier: &str,
        package_name: &str,
        api_prefix: &str,
        package_root: PathBuf,
        load_source: &str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        evaluate_with_seeded_package_adoption(
            specifier,
            package_name,
            api_prefix,
            package_root,
            load_source,
            true,
        )
        .await
    }

    async fn evaluate_with_seeded_package_adoption(
        specifier: &str,
        package_name: &str,
        api_prefix: &str,
        package_root: PathBuf,
        load_source: &str,
        adopt: bool,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        write_loadable_package(&package_root, load_source);
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::new(Mutex::new(WorkspaceState::new())),
            1,
        ));
        let package_json = loadable_package_fixture(package_name, api_prefix);
        {
            let mut service = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            service
                .install_from_value_at_root_with_spec(package_json, package_root, specifier)
                .expect("seed package install succeeds");
            service
                .authorize_package(
                    package_name,
                    Vec::new(),
                    crate::packages::authorization::RuntimeProfile::NativeTrust,
                    "test-user",
                )
                .expect("seed package authorization succeeds");
            if adopt {
                service
                    .approve_package(package_name, "test")
                    .expect("seed package adoption approval succeeds");
            }
        }
        // Third-party packages load through the cross-domain bridge; the
        // worker must outlive the evaluations below.
        let _third_party_worker = wire_test_third_party_bridge(&op_state);
        let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            main_specifier,
            None,
            None,
            op_state.load_entry_allowlist(),
            crate::packages::bundled::RuntimeDomain::Trusted,
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            crate::packages::bundled::RuntimeDomain::Trusted,
        );
        let source = format!(
            r#"
            import {{ loadPackage }} from "clay:packages";
            await loadPackage({specifier:?});
            "#
        );
        let loaded = prepare_runtime_entry(RuntimeEntry::ControlledSource(source), 1).unwrap();
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
    }

    /// Plan 035 task 8: prove the one-line `init.js` default loads an installed,
    /// authorized, *user-installed* (non-`@clay/*`) package the same way it
    /// loads `@clay/markdown`. Mirrors [`evaluate_with_seeded_package`] but
    /// evaluates a real `~/.config/clay/init.js`-shaped config root instead of
    /// a controlled module source, so the loadEntry import + default-export
    /// invocation is exercised through the configuration runtime path.
    async fn evaluate_init_js_with_seeded_package(
        config_root: PathBuf,
        specifier: &str,
        package_name: &str,
        api_prefix: &str,
        package_root: PathBuf,
        load_source: &str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        write_loadable_package(&package_root, load_source);
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::new(Mutex::new(WorkspaceState::new())),
            1,
        ));
        let _third_party_worker = wire_test_third_party_bridge(&op_state);
        let package_json = loadable_package_fixture(package_name, api_prefix);
        {
            let mut service = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            service
                .install_from_value_at_root_with_spec(package_json, package_root, specifier)
                .expect("seed package install succeeds");
            service
                .authorize_package(
                    package_name,
                    Vec::new(),
                    crate::packages::authorization::RuntimeProfile::NativeTrust,
                    "test-user",
                )
                .expect("seed package authorization succeeds");
            service
                .approve_package(package_name, "test")
                .expect("seed package adoption approval succeeds");
        }
        let loaded =
            prepare_runtime_entry(RuntimeEntry::ConfigurationRoot(config_root), 1).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
            op_state.load_entry_allowlist(),
            crate::packages::bundled::RuntimeDomain::Trusted,
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            crate::packages::bundled::RuntimeDomain::Trusted,
        );
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
    }

    #[tokio::test]
    async fn init_js_authorizes_exact_language_server_before_load_and_package_cannot_self_grant() {
        let config_root = config_fixture("language-server-authority");
        let package_root = config_root.join("package");
        let workspace_root = config_root.join("workspace");
        fs::create_dir_all(&workspace_root).unwrap();
        let executable = std::env::current_exe().unwrap();
        let specifier = "local:language-server-authority";
        let package_name = "@vendor/lsp-authority";
        let contribution = "lsp-authority.server";
        let load_source = format!(
            r#"
            import {{ authorizeLanguageServer }} from "clay:language-server";
            export default async function load() {{
              try {{
                await authorizeLanguageServer({{
                  package: {package_name:?},
                  contribution: {contribution:?},
                  workspaceRootIds: [1],
                }});
                throw new Error("loaded package unexpectedly self-authorized");
              }} catch (error) {{
                // Trusted-domain runtimes reject with authorization_sealed;
                // third-party runtimes fail closed by op absence (Plan 061).
                if (!String(error).includes("authorization_sealed") && !String(error).includes("is not a function")) throw error;
                Deno.core.ops.op_clay_runtime_record("package grant sealed");
              }}
            }}
            "#
        );
        write_loadable_package(&package_root, &load_source);

        let mut package_json = loadable_package_fixture(package_name, "lsp-authority");
        package_json["clay"]["capabilities"] = serde_json::json!(["language-server"]);
        package_json["clay"]["contributions"]["languageServers"] = serde_json::json!([{
            "id": contribution,
            "executable": executable,
            "args": ["--stdio"],
            "inheritEnvironment": ["HOME"]
        }]);

        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
        let workspace_root_id = workspace.lock().await.add_root(&workspace_root).unwrap();
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::clone(&workspace),
            1,
        ));
        let _third_party_worker = wire_test_third_party_bridge(&op_state);
        op_state.set_runtime_context(Arc::clone(&workspace), 1, true);
        {
            let mut locked = op_state.package_service().lock().unwrap();
            locked
                .install_from_value_at_root_with_spec(package_json, package_root, specifier)
                .unwrap();
            locked.approve_package(package_name, "test").unwrap();
        }

        fs::write(
            config_root.join("init.js"),
            format!(
                r#"
                import {{ authorizeLanguageServer }} from "clay:language-server";
                import {{ loadPackage }} from "clay:packages";
                try {{
                  await authorizeLanguageServer({{
                    package: {package_name:?},
                    contribution: {contribution:?},
                    workspaceRootIds: [999999],
                  }});
                  throw new Error("unknown workspace root unexpectedly authorized");
                }} catch (error) {{
                  if (!String(error).includes("unknown_workspace_root")) throw error;
                  Deno.core.ops.op_clay_runtime_record("unknown root rejected");
                }}
                const grant = await authorizeLanguageServer({{
                  package: {package_name:?},
                  contribution: {contribution:?},
                  workspaceRootIds: [{workspace_root_id}],
                }});
                Deno.core.ops.op_clay_runtime_record(`granted:${{grant.contribution}}`);
                await loadPackage({specifier:?});
                "#
            ),
        )
        .unwrap();

        let loaded =
            prepare_runtime_entry(RuntimeEntry::ConfigurationRoot(config_root), 1).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
            op_state.load_entry_allowlist(),
            crate::packages::bundled::RuntimeDomain::Trusted,
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            crate::packages::bundled::RuntimeDomain::Trusted,
        );
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        let evaluation = evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
        .unwrap();

        assert_eq!(
            evaluation.op_records,
            [
                "unknown root rejected",
                "granted:lsp-authority.server",
                "package grant sealed"
            ]
        );
        let service = op_state.package_service().lock().unwrap();
        let grant = service
            .language_server_grant(package_name, contribution)
            .unwrap();
        assert_eq!(grant.workspace_root_ids, [workspace_root_id]);
        assert_eq!(
            grant.canonical_executable,
            std::fs::canonicalize(executable).unwrap()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn language_server_facade_round_trips_exact_uint8array_bytes() {
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt as _;

        let config_root = config_fixture("language-server-bytes");
        let package_root = config_root.join("package");
        let workspace_root = config_root.join("workspace");
        fs::create_dir_all(&workspace_root).unwrap();
        write_loadable_package(&package_root, "export default function load() {}\n");

        let executable = config_root.join("fake-byte-echo");
        let mut file = fs::File::create(&executable).unwrap();
        file.write_all(b"#!/bin/sh\nexec /bin/cat\n").unwrap();
        file.sync_all().unwrap();
        drop(file);
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

        let package_name = "@vendor/lsp-bytes";
        let contribution = "lspbytes.server";
        let mut package_json = loadable_package_fixture(package_name, "lspbytes");
        package_json["clay"]["capabilities"] = serde_json::json!(["language-server"]);
        package_json["clay"]["contributions"]["languageServers"] = serde_json::json!([{
            "id": contribution,
            "executable": executable,
            "args": [],
            "inheritEnvironment": []
        }]);

        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
        let workspace_root_id = workspace.lock().await.add_root(&workspace_root).unwrap();
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::clone(&workspace),
            1,
        ));
        let _third_party_worker = wire_test_third_party_bridge(&op_state);
        op_state.set_runtime_context(Arc::clone(&workspace), 1, true);
        {
            let mut service = op_state.package_service().lock().unwrap();
            service
                .install_from_value_at_root_with_spec(
                    package_json,
                    package_root,
                    "local:language-server-bytes",
                )
                .unwrap();
            // Base capabilities first: the init.js language-server grant below
            // augments this record with the `language-server` capability.
            service
                .authorize_package(
                    package_name,
                    vec![
                        crate::packages::permissions::PackagePermission::ParseDocument,
                        crate::packages::permissions::PackagePermission::RenderDecorations,
                        crate::packages::permissions::PackagePermission::CompletionProvider,
                    ],
                    crate::packages::authorization::RuntimeProfile::Restricted,
                    "test",
                )
                .unwrap();
        }

        // Phase 1 (configuration): approve the contribution/root grant.
        fs::write(
            config_root.join("init.js"),
            format!(
                r#"
                import {{ authorizeLanguageServer }} from "clay:language-server";
                await authorizeLanguageServer({{
                  package: {package_name:?},
                  contribution: {contribution:?},
                  workspaceRootIds: [{workspace_root_id}],
                }});
                "#
            ),
        )
        .unwrap();

        let loaded =
            prepare_runtime_entry(RuntimeEntry::ConfigurationRoot(config_root), 1).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
            op_state.load_entry_allowlist(),
            crate::packages::bundled::RuntimeDomain::Trusted,
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            crate::packages::bundled::RuntimeDomain::Trusted,
        );
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
        .unwrap();

        // Enable with the grant in place, then run the session phase under the
        // package's host-stamped context: sessions are owned by the executing
        // package, never by caller-supplied names.
        let enabled = {
            let mut locked = op_state.package_service().lock().unwrap();
            locked.approve_package(package_name, "test").unwrap();
            locked.enable(package_name).unwrap().clone()
        };
        op_state.set_current_package(Some(crate::server::ops::PackageContext::from_record(
            &enabled,
        )));
        let session_source = format!(
            r#"
            import {{ startLanguageServerSession }} from "clay:language-server";
            const session = await startLanguageServerSession({{
              contribution: {contribution:?},
              workspaceRootId: {workspace_root_id},
            }});
            const sent = new Uint8Array([0, 240, 159, 166, 128, 255]);
            await session.sendBytes(sent);
            const received = [];
            while (received.length < sent.length) {{
              received.push(...await session.readBytes(sent.length - received.length, 2000));
            }}
            Deno.core.ops.op_clay_runtime_record(`bytes:${{received.join(",")}}`);
            await session.stop();
            "#
        );
        let session_entry =
            prepare_runtime_entry(RuntimeEntry::ControlledSource(session_source), 2).unwrap();
        loader.set_entry(
            session_entry.main_specifier.clone(),
            session_entry.main_source.clone(),
            session_entry.configuration.clone(),
        );
        let evaluation = evaluate_loaded_module(
            &mut runtime,
            &op_state,
            session_entry,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            false,
            &heap_limit_hit,
        )
        .await
        .unwrap();

        assert_eq!(evaluation.op_records, ["bytes:0,240,159,166,128,255"]);
    }

    #[tokio::test]
    async fn load_package_user_installed_default_loads_from_init_js() {
        // Plan 035 task 8: the one-line end-user default loads an installed,
        // authorized, user-installed package from a genuine `init.js` config
        // root. No inline manifest, no per-primitive registration, and no
        // manual facade plumbing in user config — `loadPackage` owns all of it.
        let config_root = config_fixture("init-js-user-package");
        let package_root = config_root
            .join("node_modules")
            .join("@vendor")
            .join("mode");
        let init_js = r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("github:vendor/mode");
            "#;
        fs::write(config_root.join("init.js"), init_js).unwrap();

        // The user config carries no manifest object and no per-primitive
        // registration calls — loadPackage does all of it.
        for forbidden in [
            "contributions",
            "modePattern",
            "serverRegisterCommand",
            "serverRegisterParseHandler",
            "serverActivateMajorMode",
            "markdownPackageManifest",
        ] {
            assert!(
                !init_js.contains(forbidden),
                "default init.js must not carry `{forbidden}` for a user-installed package"
            );
        }

        let result = evaluate_init_js_with_seeded_package(
            config_root.clone(),
            "github:vendor/mode",
            "@vendor/mode",
            "vendormode",
            package_root.clone(),
            r#"Deno.core.ops.op_clay_runtime_record("user-installed init.js load"); export default function load() {}"#,
        )
        .await
        .expect("one-line init.js load must succeed for installed user package");

        // The package loadEntry default export ran (it recorded an op), proving
        // activation went through the shared resolver + enable + authorize +
        // loadEntry import path from a real init.js file.
        assert_eq!(result.op_records, vec!["user-installed init.js load"]);
        let _ = fs::remove_dir_all(config_root);
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_rejects_uninstalled_specifier() {
        // Source-aware loading no longer categorically rejects npm/GitHub/local
        // shapes. They must still exist in the package service's installed and
        // authorized registry before runtime loading can proceed.
        for denied in [
            "left-pad",
            "github:user/mode",
            "./local-package",
            "../escape",
            "/absolute/package",
        ] {
            let err = resolve_by_specifier(denied).await.unwrap_err();
            assert!(
                err.contains("packages.not_installed"),
                "uninstalled specifier `{denied}` must be not_installed, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_rejects_invalid_bundled_specifier() {
        for denied in [
            "@clay/",
            "@clay/../escape",
            "@clay/foo/bar",
            "@clay/markdown?tag=latest",
            "@clay/markdown#hash",
        ] {
            let err = resolve_by_specifier(denied).await.unwrap_err();
            assert!(
                err.contains("packages.invalid_specifier"),
                "invalid bundled specifier `{denied}` must be invalid_specifier, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn load_package_loads_authorized_npm_style_fixture() {
        let root = config_fixture("npm-package-load")
            .join("node_modules")
            .join("left-pad");
        let result = evaluate_with_seeded_package(
            "left-pad",
            "left-pad",
            "leftpad",
            root.clone(),
            r#"Deno.core.ops.op_clay_runtime_record("npm fixture loaded"); export default function load() {}"#,
        )
        .await
        .expect("authorized npm-style package must load through shared package path");

        assert_eq!(result.op_records, vec!["npm fixture loaded"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_loads_authorized_github_requested_spec_fixture() {
        let root = config_fixture("github-package-load")
            .join("node_modules")
            .join("@vendor")
            .join("mode");
        let result = evaluate_with_seeded_package(
            "github:vendor/mode",
            "@vendor/mode",
            "vendormode",
            root.clone(),
            r#"import "./helper.js"; export default function load() {}"#,
        )
        .await
        .expect("authorized scoped package must load through shared package path");

        assert_eq!(result.op_records, vec!["helper loaded"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_completion_provider_fixture_registers_metadata() {
        let root = config_fixture("completion-provider-package-load").join("completion-provider");
        write_loadable_package(
            &root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            export default function load() {
              serverRegisterCompletionProvider({});
            }
            "#,
        );
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::new(Mutex::new(WorkspaceState::new())),
            1,
        ));
        let _third_party_worker = wire_test_third_party_bridge(&op_state);
        let mut package_json =
            loadable_package_fixture("completion-provider", "completionprovider");
        package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
        package_json["clay"]["contributions"]["completionProviders"] = serde_json::json!([{
            "id": "completionprovider.words",
            "triggerCharacters": ["."],
            "budgets": { "timeoutMs": 50, "maxItems": 20 }
        }]);
        {
            let mut service = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            service
                .install_from_value_at_root_with_spec(
                    package_json,
                    root.clone(),
                    "completion-provider",
                )
                .expect("seed completion package install succeeds");
            service
                .authorize_package(
                    "completion-provider",
                    vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                    crate::packages::authorization::RuntimeProfile::NativeTrust,
                    "test-user",
                )
                .expect("seed completion package authorization succeeds");
            service
                .approve_package("completion-provider", "test")
                .expect("seed completion package adoption approval succeeds");
        }
        let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            main_specifier,
            None,
            None,
            op_state.load_entry_allowlist(),
            crate::packages::bundled::RuntimeDomain::Trusted,
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
            crate::packages::bundled::RuntimeDomain::Trusted,
        );
        let loaded = prepare_runtime_entry(
            RuntimeEntry::ControlledSource(
                r#"
                import { loadPackage } from "clay:packages";
                await loadPackage("completion-provider");
                "#
                .to_string(),
            ),
            1,
        )
        .unwrap();
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        let result = evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
        .expect("completion provider loadPackage path succeeds");

        assert_eq!(result.completion_providers.len(), 1);
        assert_eq!(
            result.completion_providers[0].id,
            "completionprovider.words"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_loads_authorized_local_requested_spec_fixture() {
        let root = config_fixture("local-package-load").join("local-package");
        let result = evaluate_with_seeded_package(
            "./local-package",
            "local-package",
            "localpackage",
            root.clone(),
            r#"Deno.core.ops.op_clay_runtime_record("local fixture loaded"); export default function load() {}"#,
        )
        .await
        .expect("authorized local package spec must load through shared package path");

        assert_eq!(result.op_records, vec!["local fixture loaded"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_rejects_escaping_relative_import_from_package_root() {
        let root = config_fixture("escaping-package-load").join("evil-mode");
        fs::create_dir_all(root.parent().unwrap()).expect("create parent fixture root");
        fs::write(root.parent().unwrap().join("escape.js"), "export {};\n")
            .expect("write outside escape module");
        let err = evaluate_with_seeded_package(
            "evil-mode",
            "evil-mode",
            "evilmode",
            root.clone(),
            r#"import "../escape.js"; export default function load() {}"#,
        )
        .await
        .unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("runtime.invalid_import"),
            "escaping relative import must fail at module loader boundary, got: {message}"
        );
        let _ = fs::remove_dir_all(root.parent().unwrap());
    }

    /// Plan 061 task 10 sentinel: an unapproved third-party package's
    /// JavaScript never executes — `loadPackage` fails at the adoption gate
    /// before the load entry module is imported/evaluated.
    #[tokio::test]
    async fn unapproved_third_party_package_never_executes_before_adoption() {
        let root = config_fixture("unapproved-package-load").join("stealth-package");
        let err = evaluate_with_seeded_package_adoption(
            "stealth-package",
            "stealth-package",
            "stealthpackage",
            root.clone(),
            r#"Deno.core.ops.op_clay_runtime_record("SENTINEL-EXECUTED"); export default function load() {}"#,
            false,
        )
        .await
        .unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("package_approval.missing"),
            "unapproved loadPackage must fail at the adoption gate, got: {message}"
        );
        assert!(
            !message.contains("SENTINEL-EXECUTED"),
            "package load entry must never execute before approval, got: {message}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_rejects_unknown_package() {
        // `@clay/*` shape but no installed package on disk.
        let err = resolve_by_specifier("@clay/does-not-exist")
            .await
            .unwrap_err();
        assert!(
            err.contains("packages.not_installed"),
            "unknown first-party package must be not_installed, got: {err}"
        );
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_resolves_and_enables_first_party_markdown()
    {
        // The real shipped `@clay/markdown` package validates/enables through
        // PackageService and returns an opaque loadEntrySpecifier. The module
        // import itself is task 4/5; here we prove resolve + enable works and
        // the opaque specifier is recorded in the allowlist via the returned
        // summary shape.
        let source = r#"
            const raw = Deno.core.ops.op_clay_packages_load_package_by_specifier(
              JSON.stringify({ specifier: "@clay/markdown" })
            );
            const summary = JSON.parse(raw);
            globalThis.__clay_summary = summary;
        "#;
        let evaluation = ClayJsRuntimeService::default()
            .evaluate_controlled_module(source)
            .await
            .expect("@clay/markdown must resolve and enable");

        // The op returns the typed summary as a JSON string; we cannot read
        // `globalThis` after the runtime tears down, so we assert the op ran
        // without error and that subsequent resolver calls for the same
        // package succeed (idempotent enable via AlreadyEnabled fallback).
        assert!(evaluation.behavior_manifest.is_none());
        let second = resolve_by_specifier("@clay/markdown").await;
        assert!(
            second.is_ok(),
            "resolving an already-enabled package must be idempotent, got: {second:?}"
        );
    }

    /// Build an isolated `ClayModuleLoader` with a manually-populated allowlist
    /// (no resolver op, no real runtime) so the resolve/load gate is tested in
    /// isolation. `configuration` mirrors the runtime's config-root branch.
    fn loader_with_allowlist(
        entries: &[(&str, PathBuf, PathBuf)],
        configuration: Option<Arc<ConfigurationRuntime>>,
    ) -> ClayModuleLoader {
        let allowlist = Arc::new(PackageLoadEntryAllowlist::default());
        for (specifier, path, package_root) in entries {
            allowlist.record(specifier, path.clone(), package_root.clone());
        }
        let main_specifier = ModuleSpecifier::parse("clay://runtime/main.js").unwrap();
        ClayModuleLoader::new(
            main_specifier,
            None,
            configuration,
            allowlist,
            crate::packages::bundled::RuntimeDomain::Trusted,
        )
    }

    fn default_load_options() -> ModuleLoadOptions {
        ModuleLoadOptions {
            is_dynamic_import: false,
            is_synchronous: false,
            requested_module_type: RequestedModuleType::None,
        }
    }

    #[test]
    fn clay_module_loader_loads_allowlisted_first_party_load_entry() {
        // A real on-disk loadEntry OUTSIDE any config root, recorded in the
        // allowlist (what the resolver op does), must resolve and load.
        let outside_root = config_fixture("loader-loadentry");
        let loadentry_path = outside_root.join("load.js");
        fs::write(&loadentry_path, "export const clayLoadedEntry = true;\n").unwrap();

        let opaque = "clay://packages/@clay/example/dist/load.js";
        let loader = loader_with_allowlist(&[(opaque, loadentry_path, outside_root)], None);

        let resolved = loader
            .resolve(opaque, "clay://runtime/main.js", ResolutionKind::Import)
            .expect("allowlisted loadEntry must resolve");
        assert_eq!(resolved.as_str(), opaque);

        let source = match loader.load(&resolved, None, default_load_options()) {
            ModuleLoadResponse::Sync(Ok(source)) => source,
            ModuleLoadResponse::Sync(Err(error)) => panic!("load failed: {error:?}"),
            _ => panic!("expected sync response, got async"),
        };
        assert_eq!(source.module_type, ModuleType::JavaScript);
        assert!(
            std::str::from_utf8(source.code.as_bytes())
                .unwrap()
                .contains("clayLoadedEntry"),
            "load must return the recorded on-disk loadEntry source"
        );
    }

    #[test]
    fn package_load_entry_allowlist_revokes_owned_entries() {
        let root = config_fixture("loader-revoke-package");
        let loadentry_path = root.join("load.js");
        let helper_path = root.join("helper.js");
        fs::write(&loadentry_path, "import './helper.js';\n").unwrap();
        fs::write(&helper_path, "export const helper = true;\n").unwrap();
        let allowlist = PackageLoadEntryAllowlist::default();
        let opaque = "clay://packages/@vendor/example/dist/load.js";
        let canonical_root = std::fs::canonicalize(&root).unwrap();
        let canonical_loadentry = std::fs::canonicalize(&loadentry_path).unwrap();
        allowlist.record_for_package(
            opaque,
            canonical_loadentry,
            canonical_root,
            Some("@vendor/example"),
        );
        let helper = allowlist
            .resolve_relative(opaque, "./helper.js")
            .expect("relative helper import is recorded with same owner");

        assert_eq!(allowlist.revoke_package("@vendor/example"), 2);
        assert!(allowlist.absolute_path(opaque).is_none());
        assert!(allowlist.absolute_path(&helper).is_none());
    }

    #[test]
    fn clay_module_loader_denies_unallowlisted_first_party_url() {
        // Empty allowlist: every `clay://packages/...` URL is denied exactly
        // like any other untrusted specifier, even loadEntry-shaped ones.
        let loader = loader_with_allowlist(&[], None);
        for url in [
            "clay://packages/@clay/markdown/dist/load.js",
            "clay://packages/@clay/evil/x.js",
            "clay://packages/anything",
        ] {
            let error = loader
                .resolve(url, "clay://runtime/main.js", ResolutionKind::Import)
                .expect_err("unallowlisted package URL must be denied");
            assert!(
                error.to_string().contains("runtime.invalid_import"),
                "unallowlisted `{url}` must be denied, got: {error:?}"
            );
        }
    }

    #[test]
    fn clay_module_loader_preserves_config_root_confinement_for_non_package_imports() {
        // A real config root exercises the configuration branch. The allowlist
        // addition must NOT relax config-root confinement: escaping imports are
        // still rejected, while an allowlisted package loadEntry still loads.
        let parent = config_fixture("loader-configroot-parent");
        let root = parent.join("config");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("init.js"), "export const ready = true;\n").unwrap();
        // `escape.js` lives in the parent (a real file OUTSIDE config root) so
        // `canonicalize` succeeds and the `starts_with(config_root)` check is
        // the thing that rejects it.
        fs::write(parent.join("escape.js"), "export const escape = true;\n").unwrap();
        let configuration = Arc::new(ConfigurationRuntime::from_config_root(&root).unwrap());

        // Allowlisted loadEntry lives OUTSIDE the config root but still loads.
        let outside = config_fixture("loader-configroot-loadentry");
        let loadentry_path = outside.join("load.js");
        fs::write(&loadentry_path, "export const ok = true;\n").unwrap();
        let opaque = "clay://packages/@clay/example/dist/load.js";
        let loader = loader_with_allowlist(
            &[(opaque, loadentry_path, outside.clone())],
            Some(configuration),
        );

        let resolved = loader
            .resolve(opaque, "clay:configuration", ResolutionKind::Import)
            .expect("allowlisted loadEntry loads even with a config root present");

        // Escaping relative imports (not validated loadEntries) stay confined.
        let escape_err = loader
            .resolve("../escape.js", "clay:configuration", ResolutionKind::Import)
            .expect_err("escaping import must be denied by config-root confinement");
        assert!(
            escape_err.to_string().contains("configuration directory"),
            "config-root confinement must reject escaping imports, got: {escape_err:?}"
        );

        // And the allowlisted entry still returns its on-disk source alongside.
        let source = match loader.load(&resolved, None, default_load_options()) {
            ModuleLoadResponse::Sync(Ok(source)) => source,
            ModuleLoadResponse::Sync(Err(error)) => panic!("load failed: {error:?}"),
            _ => panic!("expected sync response, got async"),
        };
        assert!(
            std::str::from_utf8(source.code.as_bytes())
                .unwrap()
                .contains("ok = true"),
            "allowlisted loadEntry must load alongside config-root confinement"
        );
    }

    #[test]
    fn clay_module_loader_denies_arbitrary_file_url_or_https_specifier() {
        // `file://`, `https://`, `http://`, bare, and scheme-bearing specifiers
        // that are not curated facades or allowlisted loadEntries stay denied.
        let loader = loader_with_allowlist(&[], None);
        for specifier in [
            "file:///etc/passwd",
            "https://example.com/evil.js",
            "http://example.com/x.js",
            "react",
            "node:fs",
            "npm:lodash",
        ] {
            let error = loader
                .resolve(specifier, "clay://runtime/main.js", ResolutionKind::Import)
                .expect_err("non-allowlisted specifier must be denied");
            assert!(
                error.to_string().contains("runtime.invalid_import"),
                "specifier `{specifier}` must be denied, got: {error:?}"
            );
        }
    }

    #[test]
    fn clay_module_loader_denies_load_entry_imports_outside_package_root() {
        // Phase 18.6 task 7 security boundary: a validated package loadEntry
        // may import its own sibling modules (e.g. `./index.js`) — those are
        // confined to the validated package root by `resolve_relative`. But an
        // import that ESCAPES the package root (e.g. `../escape.js` landing
        // outside it) must be denied so a package cannot read arbitrary files
        // outside its validated root. This is the transitive-load confinement
        // gate added in task 5.
        let outside = config_fixture("pkg-escape-root");
        let package_root = outside.join("pkg");
        let dist = package_root.join("dist");
        fs::create_dir_all(&dist).unwrap();
        let load_entry = dist.join("load.js");
        fs::write(&load_entry, "// loadEntry").unwrap();
        // A legitimate sibling inside the package root.
        let sibling = dist.join("index.js");
        fs::write(&sibling, "// sibling").unwrap();
        // An escape file OUTSIDE the package root (in the fixture parent).
        let escape = outside.join("escape.js");
        fs::write(&escape, "// secret").unwrap();

        let opaque = "clay://packages/@clay/example/dist/load.js";
        let allowlist = Arc::new(PackageLoadEntryAllowlist::default());
        allowlist.record(
            opaque,
            load_entry.canonicalize().unwrap(),
            package_root.canonicalize().unwrap(),
        );

        // Legitimate sibling import inside the package root resolves.
        let ok = allowlist.resolve_relative(opaque, "./index.js");
        assert!(
            ok.is_some(),
            "a sibling import inside the validated package root must resolve"
        );
        // An import that escapes the package root is denied (returns None).
        assert_eq!(
            allowlist.resolve_relative(opaque, "../escape.js"),
            None,
            "an import escaping the validated package root must be denied"
        );
        // A deep escape attempt is also denied.
        assert_eq!(
            allowlist.resolve_relative(opaque, "../../escape.js"),
            None,
            "a deep-escape import must be denied"
        );
        // A relative import from an unknown referrer (not in the allowlist) is
        // denied — the confinement gate only fires for validated package modules.
        assert_eq!(
            allowlist.resolve_relative("clay://packages/@clay/unknown/dist/x.js", "./y.js"),
            None,
            "a relative import from a non-validated referrer must be denied"
        );
    }

    #[tokio::test]
    async fn set_theme_resolves_first_party_gruvbox_theme() {
        // Gruvbox stays opt-in: both Gruvbox Material variants are selectable by
        // a one-line `setTheme` call; neither is a canonical default.
        for specifier in [
            "@clay/theme-gruvbox-material-dark",
            "@clay/theme-gruvbox-material-light",
        ] {
            let root = config_fixture(&format!(
                "set-theme-gruvbox-e2e-{}",
                specifier.trim_start_matches("@clay/theme-gruvbox-material-")
            ));
            fs::write(
                root.join("init.js"),
                format!(
                    r#"
                    import {{ setTheme }} from "clay:theme";
                    const summary = setTheme("{specifier}");
                    Deno.core.ops.op_clay_runtime_record(
                      `theme:${{summary.specifier}}:overrides:${{summary.overrideCount}}`
                    );
                    "#
                ),
            )
            .unwrap();

            let result = ClayJsRuntimeService::default()
                .load_configuration_from_root(root)
                .await
                .unwrap_or_else(|err| panic!("setTheme('{specifier}') must succeed: {err:?}"));

            let theme = result.active_theme.expect("active theme snapshot emitted");
            assert_eq!(theme.specifier, specifier);
            assert_eq!(theme.overrides.len(), 48);
            assert!(
                result
                    .op_records
                    .iter()
                    .any(|record| *record == format!("theme:{specifier}:overrides:48")),
                "setTheme('{specifier}') summary must reach init.js"
            );
        }
    }

    #[tokio::test]
    async fn default_config_resolves_canonical_dark_modus_vivendi() {
        // No explicit setTheme: appearance defaults to System, which with no OS
        // signal falls back to dark → canonical default Modus Vivendi.
        let root = config_fixture("default-theme-appearance-e2e");
        fs::write(root.join("init.js"), "// no setTheme call\n").unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("default config must evaluate");
        let theme = result
            .active_theme
            .as_ref()
            .expect("canonical default theme must be resolved when no explicit theme is set");
        assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
        assert_eq!(theme.overrides.len(), 48);
    }

    #[tokio::test]
    async fn set_appearance_light_resolves_canonical_modus_operandi() {
        let root = config_fixture("set-appearance-light-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { setAppearance } from "clay:theme";
            const summary = setAppearance("light");
            Deno.core.ops.op_clay_runtime_record(
              `appearance:${summary.appearance}:theme:${summary.resolvedTheme}`
            );
            "#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("setAppearance('light') must succeed");
        let theme = result
            .active_theme
            .as_ref()
            .expect("canonical light default must be resolved");
        assert_eq!(theme.specifier, "@clay/theme-modus-operandi");
        assert!(
            result
                .op_records
                .iter()
                .any(|r| r == "appearance:light:theme:@clay/theme-modus-operandi"),
            "setAppearance summary must reach init.js"
        );
    }

    #[tokio::test]
    async fn explicit_set_theme_wins_over_appearance() {
        let root = config_fixture("explicit-theme-wins-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { setTheme, setAppearance } from "clay:theme";
            setTheme("@clay/theme-gruvbox-material-dark");
            const summary = setAppearance("light");
            Deno.core.ops.op_clay_runtime_record(
              `appearance:${summary.appearance}:resolved:${summary.resolvedTheme}`
            );
            "#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("config must evaluate");
        let theme = result
            .active_theme
            .as_ref()
            .expect("explicit theme must remain active");
        assert_eq!(
            theme.specifier, "@clay/theme-gruvbox-material-dark",
            "explicit setTheme must win over appearance-derived default"
        );
        // setAppearance reports no re-resolution once an explicit theme is active.
        assert!(
            result
                .op_records
                .iter()
                .any(|r| r == "appearance:light:resolved:null"),
            "setAppearance must not re-resolve over an explicit theme"
        );
    }

    #[tokio::test]
    async fn set_appearance_rejects_unknown_value() {
        let root = config_fixture("set-appearance-invalid-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { setAppearance } from "clay:theme";
            try {
              setAppearance("nope");
              Deno.core.ops.op_clay_runtime_record("appearance:accepted");
            } catch (err) {
              Deno.core.ops.op_clay_runtime_record(`appearance:rejected:${err.message.split(":")[0]}`);
            }
            "#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("config must evaluate");
        assert!(
            result
                .op_records
                .iter()
                .any(|r| r == "appearance:rejected:theme.invalid_request"),
            "unknown appearance must be rejected with theme.invalid_request"
        );
    }

    fn collect_kinds<'a>(
        component: &'a crate::shell::PackageUiComponentTree,
        out: &mut Vec<&'a str>,
    ) {
        out.push(component.kind.as_str());
        for item in &component.items {
            let _ = item;
        }
        for child in &component.children {
            collect_kinds(child, out);
        }
    }

    #[tokio::test]
    async fn settings_package_registers_catalog_only_panel() {
        let root = config_fixture("settings-package-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/settings");
            "#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("@clay/settings must load");
        let panel = result
            .ui_contributions
            .panels
            .iter()
            .find(|panel| panel.id == "settings.surface")
            .expect("settings.surface panel contribution must register");
        // Every action target is a settings.* command intent.
        for target in &panel.action_targets {
            assert!(
                target.starts_with("settings."),
                "settings panel action target `{target}` must be a settings.* intent"
            );
        }
        // Every component kind in the tree is an implemented catalog kind.
        let mut kinds: Vec<&str> = Vec::new();
        collect_kinds(&panel.component_tree, &mut kinds);
        assert!(
            kinds.iter().all(|kind| matches!(
                *kind,
                "panel"
                    | "label"
                    | "button"
                    | "list"
                    | "flex"
                    | "stack"
                    | "overlay"
                    | "scroll"
                    | "portal"
                    | "statusItem"
                    | "dropdown"
                    | "collapse"
                    | "modal"
                    | "textInput"
                    | "editorView"
            )),
            "settings surface must use only catalog kinds, got {kinds:?}"
        );
        // Theme and appearance dropdowns plus typography textInputs are present.
        assert!(
            kinds.contains(&"dropdown"),
            "theme/appearance dropdowns present"
        );
        assert!(
            kinds.contains(&"textInput"),
            "typography textInputs present"
        );
        assert!(kinds.contains(&"collapse"), "collapsible sections present");
        assert!(kinds.contains(&"button"), "action buttons present");
    }

    #[tokio::test]
    async fn set_theme_resolves_first_party_modus_themes() {
        for specifier in ["@clay/theme-modus-operandi", "@clay/theme-modus-vivendi"] {
            let root = config_fixture(&format!(
                "set-theme-modus-e2e-{}",
                specifier.trim_start_matches("@clay/theme-")
            ));
            fs::write(
                root.join("init.js"),
                format!(
                    r#"
                    import {{ setTheme }} from "clay:theme";
                    const summary = setTheme("{specifier}");
                    Deno.core.ops.op_clay_runtime_record(
                      `theme:${{summary.specifier}}:overrides:${{summary.overrideCount}}`
                    );
                    "#
                ),
            )
            .unwrap();

            let result = ClayJsRuntimeService::default()
                .load_configuration_from_root(root)
                .await
                .unwrap_or_else(|err| panic!("setTheme('{specifier}') must succeed: {err:?}"));

            let theme = result.active_theme.expect("active theme snapshot emitted");
            assert_eq!(theme.specifier, specifier);
            assert_eq!(theme.overrides.len(), 48);
            assert!(
                result
                    .op_records
                    .iter()
                    .any(|record| record == &format!("theme:{specifier}:overrides:48")),
                "setTheme summary must reach init.js for {specifier}"
            );
        }
    }

    #[tokio::test]
    async fn canonical_default_is_modus_not_gruvbox() {
        // Gruvbox stays opt-in: a silent init.js resolves the Modus canonical
        // default (dark / Modus Vivendi), never a Gruvbox theme. There is no
        // promotion-by-naming for Gruvbox.
        let root = config_fixture("canonical-default-not-gruvbox");
        fs::write(root.join("init.js"), "// silent\n").unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("silent config must evaluate");
        let theme = result.active_theme.expect("canonical default emitted");
        assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
        assert_ne!(theme.specifier, "@clay/theme-gruvbox-material-dark");
        assert_ne!(theme.specifier, "@clay/theme-gruvbox-material-light");
    }

    #[tokio::test]
    async fn explicit_set_theme_wins_over_canonical_default() {
        // An explicit `setTheme` for a non-default bundled theme overrides the
        // appearance-derived canonical default without any `loadPackage` call.
        let root = config_fixture("explicit-theme-beats-canonical-default");
        fs::write(
            root.join("init.js"),
            r#"
            import { setTheme } from "clay:theme";
            setTheme("@clay/theme-gruvbox-material-light");
            "#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("explicit setTheme config must evaluate");
        let theme = result.active_theme.expect("active theme emitted");
        assert_eq!(theme.specifier, "@clay/theme-gruvbox-material-light");
    }

    #[tokio::test]
    async fn absent_init_js_loads_no_runtime_theme() {
        // Boundary: with no init.js at all the default-config loader returns
        // None and resolves no runtime theme (the editor/shell core default
        // applies; the canonical Modus default requires an init.js entry
        // point to run). This documents the loading-experience boundary:
        // canonical defaults need no `loadPackage`, but they do need the
        // `init.js` entry point to evaluate.
        let root = config_fixture("absent-init-js");
        // Deliberately do NOT create init.js.
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await;
        assert!(
            result.is_err(),
            "absent init.js must not silently evaluate, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn set_typography_replaces_all_profiles_atomically() {
        let root = config_fixture("set-typography-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { setTypography } from "clay:theme";
            const summary = setTypography({
              monospace: { families: ["JetBrains Mono", "monospace"], size: 16 },
              proportional: { families: ["Inter", "sans-serif"], size: 17 },
              ui: { families: ["system-ui"], size: 13 },
            });
            Deno.core.ops.op_clay_runtime_record(`typography:${summary.revision}`);
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("setTypography must accept one complete valid replacement");
        let typography = result
            .active_typography
            .expect("complete typography candidate emitted");
        assert_eq!(typography.revision, 1);
        assert_eq!(typography.monospace.families[0], "JetBrains Mono");
        assert_eq!(typography.proportional.size, 17.0);
        assert_eq!(typography.ui.families, ["system-ui"]);
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "typography:1")
        );
    }

    #[tokio::test]
    async fn set_typography_failure_preserves_previous_revision() {
        let service = ClayJsRuntimeService::default();
        let first = service
            .evaluate_controlled_module(
                r#"import { setTypography } from "clay:theme";
                setTypography({
                  monospace: { families: ["monospace"], size: 16 },
                  proportional: { families: ["sans-serif"], size: 17 },
                  ui: { families: ["system-ui"], size: 13 },
                });"#,
            )
            .await
            .expect("initial typography succeeds");
        assert_eq!(first.active_typography.unwrap().revision, 1);

        assert!(
            service
                .evaluate_controlled_module(
                    r#"import { setTypography } from "clay:theme";
                    setTypography({
                      monospace: { families: ["monospace"], size: 16 },
                      proportional: { families: ["sans-serif"], size: 17 },
                    });"#,
                )
                .await
                .is_err(),
            "incomplete candidate fails before state replacement"
        );

        let second = service
            .evaluate_controlled_module(
                r#"import { setTypography } from "clay:theme";
                setTypography({
                  monospace: { families: ["monospace"], size: 18 },
                  proportional: { families: ["sans-serif"], size: 19 },
                  ui: { families: ["system-ui"], size: 14 },
                });"#,
            )
            .await
            .expect("valid replacement after failure succeeds");
        let typography = second.active_typography.unwrap();
        assert_eq!(typography.revision, 2);
        assert_eq!(typography.monospace.size, 18.0);
    }

    #[tokio::test]
    async fn typography_configuration_rejects_oversized_snapshot() {
        let service = ClayJsRuntimeService::default();
        assert!(
            service
                .evaluate_controlled_module(
                    r#"import { setTypography } from "clay:theme";
                    const named = "x".repeat(128);
                    setTypography({
                      monospace: { families: [named, named, named, named, named, named, named, "monospace"], size: 16 },
                      proportional: { families: [named, named, named, named, named, named, named, "sans-serif"], size: 17 },
                      ui: { families: [named, named, named, named, named, named, named, "system-ui"], size: 13 },
                    });"#,
                )
                .await
                .is_err(),
            "one typography update remains bounded even when individual fields are valid"
        );
    }

    #[tokio::test]
    async fn typography_configuration_grants_no_additional_authority() {
        let service = ClayJsRuntimeService::default();
        assert!(
            service
                .evaluate_controlled_module(
                    r#"import { setTypography } from "clay:theme";
                    setTypography({
                      monospace: { families: ["monospace"], size: 16, fontUrl: "https://example.com/font" },
                      proportional: { families: ["sans-serif"], size: 17 },
                      ui: { families: ["system-ui"], size: 13 },
                    });"#,
                )
                .await
                .is_err(),
            "font URLs and all unrecognized authority fields are rejected"
        );
    }

    #[tokio::test]
    async fn load_package_resolves_and_activates_first_party_markdown_end_to_end() {
        // The one-line default end-user path: a configuration module that does
        // `await loadPackage("@clay/markdown")` activates the package — the
        // loadEntry imports curated clay:* facades and registers its mode,
        // commands, and parse handler under Clay's authority.
        let root = config_fixture("loadpackage-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            const summary = await loadPackage("@clay/markdown");
            Deno.core.ops.op_clay_runtime_record(
              `loaded:${summary.name}:modes:${summary.modes.join(",")}`
            );
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage('@clay/markdown') must succeed end-to-end");

        // The resolver summary reaches the caller.
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "loaded:@clay/markdown:modes:markdown"),
            "loadPackage must return the typed summary with name + modes, got {:?}",
            result.op_records
        );
        // The loadEntry's default activation registered a parse handler.
        assert!(
            !result.parse_handlers.is_empty(),
            "loadPackage must activate the markdown parse handler, got none"
        );
        // Modes/commands/keymaps surfaced through the behavior manifest.
        assert!(
            result.behavior_manifest.is_some(),
            "loadPackage must register mode/commands/keymaps into the behavior manifest"
        );
    }

    #[tokio::test]
    async fn load_package_is_idempotent_per_persistent_runtime() {
        let root = config_fixture("loadpackage-idempotent");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            await loadPackage("@clay/markdown");
            Deno.core.ops.op_clay_runtime_record("loaded-once");
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("repeated loadPackage calls must reuse the already-loaded package");

        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "loaded-once")
        );
        assert_eq!(result.js_parse_handlers.len(), 1);
    }

    #[tokio::test]
    async fn load_package_remains_idempotent_inside_one_generation() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                const first = await loadPackage("@clay/markdown");
                const second = await loadPackage("@clay/markdown");
                const third = await loadPackage("@clay/rust");
                const fourth = await loadPackage("@clay/rust");
                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                  markdownSame: first === second,
                  rustSame: third === fourth,
                  markdownCached: Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"]),
                  rustCached: Boolean(globalThis.__clayLoadedPackages?.["@clay/rust"]),
                }));
                "#,
            )
            .await
            .expect("in-generation repeated loads must succeed");

        assert_eq!(
            evaluation
                .js_parse_handlers
                .iter()
                .filter(|handler| handler.package.manifest.name == "@clay/markdown")
                .count(),
            1,
            "markdown parse handler must register once per generation"
        );
        assert_eq!(
            evaluation
                .completion_providers
                .iter()
                .filter(|provider| provider.id == "rust.keywords")
                .count(),
            1,
            "rust completion provider must register once per generation"
        );
        let record = evaluation
            .op_records
            .into_iter()
            .next()
            .expect("idempotency record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON");
        assert_eq!(parsed["markdownSame"], true);
        assert_eq!(parsed["rustSame"], true);
        assert_eq!(parsed["markdownCached"], true);
        assert_eq!(parsed["rustCached"], true);
    }

    #[tokio::test]
    async fn load_package_rejects_non_string_specifier() {
        // The facade validates the specifier type before touching the op,
        // mirroring bindKey/serverLoadPackage validation.
        for invalid in ["loadPackage(123)", "loadPackage()", "loadPackage(null)"] {
            let root = config_fixture("loadpackage-invalid");
            fs::write(
                root.join("init.js"),
                format!(
                    r#"
                    import {{ loadPackage }} from "clay:packages";
                    try {{
                      await {invalid};
                      Deno.core.ops.op_clay_runtime_record("no-throw");
                    }} catch (error) {{
                      Deno.core.ops.op_clay_runtime_record(String(error));
                    }}
                    "#
                ),
            )
            .unwrap();
            let result = ClayJsRuntimeService::default()
                .load_configuration_from_root(root)
                .await
                .expect("the invalid-specifier facade call must not crash the runtime");
            assert!(
                result
                    .op_records
                    .iter()
                    .any(|record| record.contains("packages.invalid_specifier")),
                "`{invalid}` must throw packages.invalid_specifier, got {:?}",
                result.op_records
            );
            assert!(
                !result.op_records.iter().any(|record| record == "no-throw"),
                "`{invalid}` must throw, not return normally"
            );
        }
    }

    #[tokio::test]
    async fn markdown_optional_preview_is_valid_panel_contribution() {
        // Phase 20 task 4: the optional Markdown preview helper registers a
        // valid clay:ui PanelContribution (hidden right slot, toggle action
        // target, package provenance) — but ONLY when called explicitly. The
        // default load path never invokes it (guarded separately by the
        // `load_package_markdown_default_activates_full_mode_from_init_js`
        // test, which asserts no panel contribution is published by default).
        let root = config_fixture("markdown-optional-preview-panel");
        // load.js imports the `clay:ui` facade and `markdownPackageManifest`
        // from index.js, so the dist module graph must be copied.
        for file_name in ["index.js", "load.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            import { registerMarkdownPreview } from "./load.js";

            // Realistic opt-in order: load the package first (registers the
            // markdown.togglePreview command and stamps host provenance for
            // this evaluation), THEN publish the optional panel.
            await loadPackage("@clay/markdown");
            const panel = registerMarkdownPreview();
            Deno.core.ops.op_clay_runtime_record(`${panel.id}:${panel.slot}:${panel.defaultVisibility}`);
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("registerMarkdownPreview must succeed");

        // The returned declaration reached the caller with the contract shape.
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "markdown.preview:right:hidden"),
            "registerMarkdownPreview must return the hidden right-slot panel, got {:?}",
            result.op_records
        );
        // The server-side PackageUiRegistry validated and recorded it with
        // package provenance.
        let panel = result
            .ui_contributions
            .panels
            .iter()
            .find(|panel| panel.id == "markdown.preview")
            .expect("the optional preview must register as a validated PanelContribution");
        assert_eq!(panel.slot, "right");
        assert_eq!(panel.default_visibility, "hidden");
        assert_eq!(panel.provenance.api_prefix, "markdown");
        assert!(
            panel
                .action_targets
                .iter()
                .any(|target| target == "markdown.togglePreview"),
            "preview panel must target the toggle command, got {:?}",
            panel.action_targets
        );
    }

    #[tokio::test]
    async fn load_package_markdown_default_activates_full_mode_from_init_js() {
        // Phase 18.6 task 6: the one-line default end-user path activates the
        // FULL markdown setup (parse handler + commands + mode) from a genuinely
        // minimal init.js — no inline manifest, no per-primitive registration,
        // no manual clay facade plumbing in user config.
        let root = config_fixture("loadpackage-default");
        let init_js = r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#;
        fs::write(root.join("init.js"), init_js).unwrap();

        // The user config carries no manifest object and no per-primitive
        // registration calls — loadPackage does all of it.
        for forbidden in [
            "contributions",
            "modePattern",
            "serverRegisterCommand",
            "serverRegisterParseHandler",
            "serverActivateMajorMode",
            "markdownPackageManifest",
        ] {
            assert!(
                !init_js.contains(forbidden),
                "default init.js must not carry `{forbidden}` — loadPackage owns activation"
            );
        }

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage('@clay/markdown') default must succeed");

        // The markdown parse handler registered (mode_id `markdown`).
        assert!(
            result
                .parse_handlers
                .iter()
                .any(|handler| handler.mode_id == "markdown"),
            "default load must register the markdown parse handler, got {:?}",
            result.parse_handlers
        );
        // The markdown commands surfaced into the behavior manifest.
        let manifest = result
            .behavior_manifest
            .as_ref()
            .expect("default load must activate the major mode into the behavior manifest");
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview"),
            "default load must register the markdown.togglePreview command, got {:?}",
            manifest
                .commands
                .iter()
                .map(|c| &c.command_id)
                .collect::<Vec<_>>()
        );
        // The markdown keymap surfaced into the behavior manifest (distinct from
        // any Ctrl+O file-open binding, which loadPackage must NOT install).
        assert!(
            manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "markdown.togglePreview"),
            "default load must register the markdown togglePreview keymap, got {:?}",
            manifest
                .keymaps
                .iter()
                .map(|k| &k.command_id)
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn default_loading_preserves_explicit_ctrl_o_separation() {
        // Phase 18.6 task 6: loadPackage must NOT install the Ctrl+O file-open
        // binding. That binding stays a separate explicit bindKey call so the
        // package never owns a global file-open key. This test verifies both
        // halves: loadPackage alone installs no clientOpenFileDialog keymap, and
        // adding the documented separate bindKey call does install it.
        let root = config_fixture("loadpackage-no-ctrlo");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#,
        )
        .unwrap();
        let without_binding = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage-only config must load");
        let manifest = without_binding
            .behavior_manifest
            .as_ref()
            .expect("loadPackage must still produce a behavior manifest");
        assert!(
            !manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "documents.clientOpenFileDialog"),
            "loadPackage must NOT install the Ctrl+O file-open keymap; it stays a separate bindKey call, got {:?}",
            manifest
                .keymaps
                .iter()
                .map(|k| &k.command_id)
                .collect::<Vec<_>>()
        );

        // The documented default adds the Ctrl+O binding as a separate explicit
        // bindKey call after loadPackage, and it lands in the manifest.
        let root = config_fixture("loadpackage-with-ctrlo");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            import { bindKey } from "clay:keybindings";
            await loadPackage("@clay/markdown");
            bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
            "#,
        )
        .unwrap();
        let with_binding = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage + bindKey config must load");
        let manifest = with_binding
            .behavior_manifest
            .as_ref()
            .expect("config with bindKey must produce a behavior manifest");
        assert!(
            manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "documents.clientOpenFileDialog"),
            "the separate bindKey call must install the Ctrl+O file-open keymap, got {:?}",
            manifest
                .keymaps
                .iter()
                .map(|k| &k.command_id)
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn preferences_override_init_js_theme_on_reload() {
        // Precedence: init.js setTheme < persisted UI-session theme. The
        // preferences.json theme is applied AFTER init.js so the UI choice wins.
        let root = config_fixture("preferences-override-init-theme");
        fs::write(
            root.join("init.js"),
            r#"
            import { setTheme } from "clay:theme";
            setTheme("@clay/theme-gruvbox-material-light");
            "#,
        )
        .unwrap();
        fs::write(
            root.join("preferences.json"),
            r#"{"theme":"@clay/theme-modus-vivendi"}"#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("preferences + init.js config must load");
        let theme = result.active_theme.expect("active theme emitted");
        assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
        assert_eq!(theme.overrides.len(), 48);
    }

    #[tokio::test]
    async fn preferences_appearance_applies_when_init_js_is_silent() {
        // No init.js theme; preferences.appearance drives the canonical default.
        let root = config_fixture("preferences-appearance-only");
        fs::write(
            root.join("init.js"),
            "// silent
",
        )
        .unwrap();
        fs::write(root.join("preferences.json"), r#"{"appearance":"light"}"#).unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("appearance preference config must load");
        let theme = result.active_theme.expect("canonical default emitted");
        assert_eq!(theme.specifier, "@clay/theme-modus-operandi");
    }

    #[tokio::test]
    async fn preferences_theme_beats_appearance_canonical_default() {
        // Both theme and appearance persisted: explicit theme wins (applied
        // first, marks explicit; appearance apply does not re-resolve).
        let root = config_fixture("preferences-theme-over-appearance");
        fs::write(
            root.join("init.js"),
            "// silent
",
        )
        .unwrap();
        fs::write(
            root.join("preferences.json"),
            r#"{"theme":"@clay/theme-gruvbox-material-dark","appearance":"light"}"#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("theme+appearance preference config must load");
        let theme = result.active_theme.expect("active theme emitted");
        assert_eq!(theme.specifier, "@clay/theme-gruvbox-material-dark");
    }

    #[tokio::test]
    async fn preferences_typography_round_trips_through_reload() {
        let root = config_fixture("preferences-typography-roundtrip");
        fs::write(
            root.join("init.js"),
            "// silent
",
        )
        .unwrap();
        fs::write(
            root.join("preferences.json"),
            r#"{"typography":{"monospace":{"families":["JetBrains Mono","monospace"],"size":18},
               "proportional":{"families":["Inter","sans-serif"],"size":17},
               "ui":{"families":["system-ui"],"size":13}}}"#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("typography preference config must load");
        let typography = result.active_typography.expect("typography emitted");
        assert!(typography.revision >= 1, "revision assigned on apply");
        assert_eq!(typography.monospace.families[0], "JetBrains Mono");
        assert_eq!(typography.monospace.size, 18.0);
        assert_eq!(typography.proportional.size, 17.0);
        assert_eq!(typography.ui.families, ["system-ui"]);
    }

    #[tokio::test]
    async fn invalid_preferences_theme_falls_back_safely_with_diagnostic() {
        // A corrupted theme field is dropped; init.js / canonical default applies.
        let root = config_fixture("preferences-invalid-theme-fallback");
        fs::write(
            root.join("init.js"),
            "// silent
",
        )
        .unwrap();
        fs::write(
            root.join("preferences.json"),
            r#"{"theme":"@vendor/evil","appearance":"dark"}"#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("invalid-preference config must still load");
        // Invalid theme dropped; appearance=dark resolves the canonical default.
        let theme = result.active_theme.expect("canonical dark default emitted");
        assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record.contains("preferences:"))
                || result
                    .op_records
                    .iter()
                    .any(|record| record.contains("preferences.json")),
            "invalid preference field must record a diagnostic, got {:?}",
            result.op_records
        );
    }

    #[tokio::test]
    async fn no_preferences_lets_init_js_win() {
        let root = config_fixture("preferences-absent-init-wins");
        fs::write(
            root.join("init.js"),
            r#"
            import { setTheme } from "clay:theme";
            setTheme("@clay/theme-gruvbox-material-light");
            "#,
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("init.js-only config must load");
        let theme = result.active_theme.expect("active theme emitted");
        assert_eq!(theme.specifier, "@clay/theme-gruvbox-material-light");
    }
}
