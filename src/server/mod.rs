mod behavior;
pub mod command_execution;
pub mod completion;
mod configuration;
mod connection;
pub(crate) mod control_center;
pub mod decorations;
pub mod diagnostics;
pub(crate) mod document;
pub(crate) mod document_analysis;
#[allow(dead_code)]
pub(crate) mod git;
#[allow(dead_code)]
mod js_runtime;
#[doc(hidden)]
pub mod language_intelligence;
#[doc(hidden)]
pub mod language_server;
pub(crate) mod locks;
#[allow(dead_code)]
mod ops;
pub mod parse_coordinator;
#[doc(hidden)]
pub mod runtime_sandbox;
mod sdui;
pub mod syntax;
mod ui;
pub(crate) mod workspace;

use std::{
    error::Error,
    fmt, io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::{fs, os::unix::fs::FileTypeExt, path::Path};

use tokio::{
    sync::{Mutex, broadcast},
    task::JoinSet,
};

#[cfg(test)]
use tokio::sync::oneshot;

#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
#[cfg(windows)]
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, LocalFree},
    Security::{
        ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, AddAccessAllowedAce, GetLengthSid,
        GetTokenInformation, InitializeAcl, InitializeSecurityDescriptor, PSECURITY_DESCRIPTOR,
        PSID, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SetSecurityDescriptorDacl, TOKEN_QUERY,
        TOKEN_USER, TokenUser,
    },
    System::Memory::{LPTR, LocalAlloc},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

use crate::{
    ipc::IpcEndpoint,
    packages::commands::CommandRegistry,
    perf::budgets::RUNTIME_STATE_BROADCAST_CAPACITY,
    protocol::{
        DocumentId, LockOwner, RuntimeDiagnostic, RuntimeGenerationId, RuntimeStateSnapshot,
        ServerMessage, codec::Codec,
    },
    server::command_execution::{
        CommandExecutionDiagnostic, CommandExecutionRequest, CommandExecutionRule,
        CommandExecutionTarget, CommandExecutor, RELOAD_CONFIGURATION_COMMAND_ID,
        is_reload_command,
    },
};

use self::{
    behavior::{ActiveBehaviorManifest, BehaviorGraceState},
    connection::handle_connection_with_analysis,
    document::DocumentState,
    js_runtime::ClayJsRuntimeService,
    language_intelligence::LanguageIntelligenceCoordinator,
    locks::{ScopedLockManager, ScopedLockTarget},
    parse_coordinator::ParseCoordinator,
    sdui::StaticSduiState,
    workspace::WorkspaceState,
};

#[cfg(windows)]
const ERROR_PIPE_CONNECTED: i32 = 535;

#[cfg(test)]
pub(crate) static JS_RUNTIME_TEST_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub endpoint: IpcEndpoint,
    pub workspace_roots: Vec<PathBuf>,
    pub configuration_root: Option<PathBuf>,
}

impl ServerConfig {
    pub fn new(endpoint: impl Into<IpcEndpoint>) -> Self {
        Self {
            endpoint: endpoint.into(),
            workspace_roots: Vec::new(),
            configuration_root: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeGeneration {
    id: u64,
    service: ClayJsRuntimeService,
    evaluation: Option<Arc<js_runtime::ClayRuntimeEvaluation>>,
    diagnostics: Vec<RuntimeDiagnostic>,
}

impl RuntimeGeneration {
    fn initial() -> Self {
        Self {
            id: 1,
            service: ClayJsRuntimeService::default(),
            evaluation: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeGenerationStore {
    current: Arc<Mutex<RuntimeGeneration>>,
    typography: ActiveTypographyState,
    runtime_state: ActiveRuntimeStateFanout,
    behavior_grace: BehaviorGraceState,
}

/// Latest committed runtime snapshot and bounded live-update channel.
#[derive(Debug, Clone)]
pub(crate) struct ActiveRuntimeStateFanout {
    latest: Arc<Mutex<Option<RuntimeStateSnapshot>>>,
    updates: broadcast::Sender<RuntimeGenerationId>,
    acknowledgements:
        Arc<Mutex<std::collections::HashMap<crate::protocol::ClientId, RuntimeGenerationId>>>,
}

impl Default for ActiveRuntimeStateFanout {
    fn default() -> Self {
        let (updates, _) = broadcast::channel(RUNTIME_STATE_BROADCAST_CAPACITY);
        Self {
            latest: Arc::new(Mutex::new(None)),
            updates,
            acknowledgements: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}

impl ActiveRuntimeStateFanout {
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<RuntimeGenerationId> {
        self.updates.subscribe()
    }

    pub(crate) async fn latest_for(
        &self,
        client_id: crate::protocol::ClientId,
    ) -> Option<RuntimeStateSnapshot> {
        self.latest
            .lock()
            .await
            .clone()
            .map(|snapshot| snapshot.for_client(client_id))
    }

    pub(crate) async fn publish(&self, snapshot: RuntimeStateSnapshot) {
        let generation = snapshot.runtime_generation_id;
        *self.latest.lock().await = Some(snapshot);
        let _ = self.updates.send(generation);
    }

    /// Record a client install acknowledgement. Spoofed client IDs, future
    /// generations, and generations older than the latest committed snapshot are
    /// ignored so acknowledgement never invents authority.
    pub(crate) async fn note_installed(
        &self,
        client_id: crate::protocol::ClientId,
        expected_client_id: crate::protocol::ClientId,
        runtime_generation_id: RuntimeGenerationId,
    ) -> bool {
        if client_id != expected_client_id {
            return false;
        }
        let latest = self
            .latest
            .lock()
            .await
            .as_ref()
            .map(|snapshot| snapshot.runtime_generation_id);
        let Some(latest_generation) = latest else {
            return false;
        };
        if runtime_generation_id == 0 || runtime_generation_id > latest_generation {
            return false;
        }
        self.acknowledgements
            .lock()
            .await
            .insert(client_id, runtime_generation_id);
        true
    }

    pub(crate) async fn acknowledged_generation(
        &self,
        client_id: crate::protocol::ClientId,
    ) -> Option<RuntimeGenerationId> {
        self.acknowledgements.lock().await.get(&client_id).copied()
    }
}

/// Server-owned active typography and bounded live-update channel.
#[derive(Debug, Clone)]
pub(crate) struct ActiveTypographyState {
    current: Arc<Mutex<crate::protocol::ActiveTypography>>,
    updates: broadcast::Sender<crate::protocol::ActiveTypography>,
}

impl Default for ActiveTypographyState {
    fn default() -> Self {
        let (updates, _) = broadcast::channel(16);
        Self {
            current: Arc::new(Mutex::new(crate::protocol::ActiveTypography::default())),
            updates,
        }
    }
}

impl ActiveTypographyState {
    pub(crate) async fn snapshot(&self) -> crate::protocol::ActiveTypography {
        self.current.lock().await.clone()
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<crate::protocol::ActiveTypography> {
        self.updates.subscribe()
    }

    /// Replace all profiles only after complete validation. Equal profiles keep
    /// their revision and emit no duplicate client event.
    #[cfg(test)]
    pub(crate) async fn replace(
        &self,
        mut typography: crate::protocol::ActiveTypography,
    ) -> Result<
        Option<crate::protocol::ActiveTypography>,
        crate::protocol::ActiveTypographyValidationError,
    > {
        typography.validate()?;
        let mut current = self.current.lock().await;
        if current.monospace == typography.monospace
            && current.proportional == typography.proportional
            && current.ui == typography.ui
        {
            return Ok(None);
        }
        typography.revision = current.revision.saturating_add(1);
        *current = typography.clone();
        drop(current);
        let _ = self.updates.send(typography.clone());
        Ok(Some(typography))
    }
}

impl RuntimeGenerationStore {
    fn initial() -> Self {
        Self {
            current: Arc::new(Mutex::new(RuntimeGeneration::initial())),
            typography: ActiveTypographyState::default(),
            runtime_state: ActiveRuntimeStateFanout::default(),
            behavior_grace: BehaviorGraceState::new(),
        }
    }

    pub(crate) async fn active_typography(&self) -> crate::protocol::ActiveTypography {
        self.typography.snapshot().await
    }

    pub(crate) fn subscribe_typography(
        &self,
    ) -> broadcast::Receiver<crate::protocol::ActiveTypography> {
        self.typography.subscribe()
    }

    pub(crate) fn subscribe_runtime_state(&self) -> broadcast::Receiver<RuntimeGenerationId> {
        self.runtime_state.subscribe()
    }

    pub(crate) fn behavior_grace(&self) -> &BehaviorGraceState {
        &self.behavior_grace
    }

    pub(crate) async fn latest_runtime_snapshot_for(
        &self,
        client_id: crate::protocol::ClientId,
    ) -> Option<RuntimeStateSnapshot> {
        self.runtime_state.latest_for(client_id).await
    }

    pub(crate) async fn publish_runtime_snapshot(&self, snapshot: RuntimeStateSnapshot) {
        self.runtime_state.publish(snapshot).await;
    }

    pub(crate) async fn note_runtime_generation_installed(
        &self,
        client_id: crate::protocol::ClientId,
        expected_client_id: crate::protocol::ClientId,
        runtime_generation_id: RuntimeGenerationId,
    ) -> bool {
        self.runtime_state
            .note_installed(client_id, expected_client_id, runtime_generation_id)
            .await
    }

    pub(crate) async fn acknowledged_runtime_generation(
        &self,
        client_id: crate::protocol::ClientId,
    ) -> Option<RuntimeGenerationId> {
        self.runtime_state.acknowledged_generation(client_id).await
    }

    #[cfg(test)]
    pub(crate) async fn replace_typography(
        &self,
        typography: crate::protocol::ActiveTypography,
    ) -> Result<
        Option<crate::protocol::ActiveTypography>,
        crate::protocol::ActiveTypographyValidationError,
    > {
        self.typography.replace(typography).await
    }

    pub(crate) async fn generation_id(&self) -> u64 {
        self.current.lock().await.id
    }

    pub(crate) async fn current(&self) -> RuntimeGeneration {
        self.current.lock().await.clone()
    }

    pub(crate) async fn current_service(&self) -> ClayJsRuntimeService {
        self.current.lock().await.service.clone()
    }

    async fn push_diagnostic(&self, diagnostic: RuntimeDiagnostic) {
        self.current.lock().await.diagnostics.push(diagnostic);
    }

    async fn swap(&self, next: RuntimeGeneration) {
        *self.current.lock().await = next;
    }
}

#[derive(Debug)]
struct RuntimeGenerationCandidate {
    expected_generation_id: u64,
    generation: RuntimeGeneration,
    expected_behavior: ActiveBehaviorManifest,
    behavior: ActiveBehaviorManifest,
    expected_sdui: StaticSduiState,
    sdui: StaticSduiState,
    expected_theme: Option<crate::protocol::ActiveTheme>,
    active_theme: Option<crate::protocol::ActiveTheme>,
    expected_typography: crate::protocol::ActiveTypography,
    active_typography: crate::protocol::ActiveTypography,
    open_documents: Vec<workspace::OpenDocumentSnapshot>,
    runtime_snapshot: RuntimeStateSnapshot,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct ReloadedDocumentRefresh {
    pub document_id: DocumentId,
    pub messages: Vec<ServerMessage>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeReloadOutcome {
    pub previous_generation_id: u64,
    pub active_generation_id: u64,
    pub reloaded: bool,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub refreshed_documents: Vec<ReloadedDocumentRefresh>,
}

#[derive(Debug, Clone)]
pub struct IpcServer {
    config: ServerConfig,
    codec: Codec,
    document: Arc<Mutex<DocumentState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    sdui: Arc<Mutex<StaticSduiState>>,
    /// Resolved active theme snapshot (Plan 046 task 7 `setTheme`) shipped to the
    /// client during the welcome handshake. `None` = Clay default theme.
    active_theme: Arc<Mutex<Option<crate::protocol::ActiveTheme>>>,
    runtime_diagnostics: Arc<Mutex<Vec<RuntimeDiagnostic>>>,
    #[allow(dead_code)]
    parse_coordinator: ParseCoordinator,
    completion: crate::server::completion::CompletionCoordinator,
    document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
    language_intelligence: LanguageIntelligenceCoordinator,
    runtime_generation: RuntimeGenerationStore,
    scoped_locks: ScopedLockManager,
    reload_attempt: Arc<Mutex<()>>,
    next_client_id: Arc<AtomicU64>,
    /// Test-only barrier that parks candidate evaluation until the test releases
    /// it. Production builds omit this field entirely.
    #[cfg(test)]
    reload_barrier: ReloadCandidateBarrier,
}

/// Parks a reload candidate between attempt-lock acquisition and configuration
/// evaluation so tests can prove ordinary edits continue without waiting.
#[cfg(test)]
#[derive(Clone, Default)]
struct ReloadCandidateBarrier {
    inner: Arc<Mutex<ReloadCandidateBarrierState>>,
}

#[cfg(test)]
impl std::fmt::Debug for ReloadCandidateBarrier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReloadCandidateBarrier")
    }
}

#[cfg(test)]
#[derive(Default)]
struct ReloadCandidateBarrierState {
    entered: Option<oneshot::Sender<()>>,
    release: Option<oneshot::Receiver<()>>,
}

#[cfg(test)]
impl ReloadCandidateBarrier {
    async fn arm(&self) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *self.inner.lock().await = ReloadCandidateBarrierState {
            entered: Some(entered_tx),
            release: Some(release_rx),
        };
        (entered_rx, release_tx)
    }

    async fn wait_if_armed(&self) {
        let (entered, release) = {
            let mut state = self.inner.lock().await;
            (state.entered.take(), state.release.take())
        };
        if let Some(entered) = entered {
            let _ = entered.send(());
        }
        if let Some(release) = release {
            let _ = release.await;
        }
    }
}

impl IpcServer {
    pub fn new(config: ServerConfig) -> Self {
        Self::try_new(config).expect("configured workspace roots must be valid")
    }

    pub fn try_new(config: ServerConfig) -> Result<Self, ServerError> {
        let mut workspace = WorkspaceState::new();
        for root in &config.workspace_roots {
            workspace.add_root(root).map_err(|error| {
                ServerError::InvalidWorkspaceRoot(error.diagnostic().to_string())
            })?;
        }
        if config.workspace_roots.is_empty() {
            workspace.add_root_from_cwd().map_err(|error| {
                ServerError::InvalidWorkspaceRoot(error.diagnostic().to_string())
            })?;
        }
        workspace.reserve_document_ids_from(2);

        Ok(Self {
            config,
            codec: Codec::default(),
            document: Arc::new(Mutex::new(DocumentState::default())),
            behavior: Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace: Arc::new(Mutex::new(workspace)),
            sdui: Arc::new(Mutex::new(StaticSduiState::empty_for_document(1))),
            active_theme: Arc::new(Mutex::new(None)),
            runtime_diagnostics: Arc::new(Mutex::new(Vec::new())),
            parse_coordinator: ParseCoordinator::default(),
            completion: crate::server::completion::CompletionCoordinator::new(),
            document_analysis:
                crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
            language_intelligence: LanguageIntelligenceCoordinator::new(),
            runtime_generation: RuntimeGenerationStore::initial(),
            scoped_locks: ScopedLockManager::default(),
            reload_attempt: Arc::new(Mutex::new(())),
            next_client_id: Arc::new(AtomicU64::new(1)),
            #[cfg(test)]
            reload_barrier: ReloadCandidateBarrier::default(),
        })
    }

    #[cfg(unix)]
    pub async fn run(self) -> Result<(), ServerError> {
        let listener = bind_unix_listener(self.config.endpoint.as_unix_socket_path())?;
        self.load_default_configuration().await;
        self.accept_unix_loop(listener).await
    }

    #[cfg(unix)]
    async fn accept_unix_loop(self, listener: UnixListener) -> Result<(), ServerError> {
        let mut connections = JoinSet::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let (stream, _address) = accepted.map_err(ServerError::Accept)?;
                    self.spawn_connection(stream, &mut connections);
                }
                Some(joined) = connections.join_next() => {
                    if let Err(error) = joined {
                        eprintln!("clay server connection task failed: {error}");
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    pub async fn run(self) -> Result<(), ServerError> {
        self.config
            .endpoint
            .validate_windows_named_pipe()
            .map_err(ServerError::InvalidEndpoint)?;
        self.load_default_configuration().await;
        let mut connections = JoinSet::new();
        loop {
            let pipe = create_named_pipe_server(self.config.endpoint.as_windows_named_pipe())?;
            tokio::select! {
                connected = connect_named_pipe_server(pipe) => {
                    let stream = connected.map_err(ServerError::Accept)?;
                    self.spawn_connection(stream, &mut connections);
                }
                Some(joined) = connections.join_next() => {
                    if let Err(error) = joined {
                        eprintln!("clay server connection task failed: {error}");
                    }
                }
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub async fn run(self) -> Result<(), ServerError> {
        Err(ServerError::InvalidEndpoint(format!(
            "Clay IPC is unsupported on this platform: {}",
            self.config.endpoint
        )))
    }

    async fn load_default_configuration(&self) {
        let generation_id = self.runtime_generation.generation_id().await;
        let service = self.runtime_generation.current_service().await;
        match self.load_configuration_for_service(&service).await {
            Ok(Some(evaluation)) => {
                match self
                    .prepare_runtime_generation_candidate(
                        generation_id,
                        generation_id,
                        service,
                        evaluation,
                    )
                    .await
                {
                    Ok(candidate) => {
                        if let Err(diagnostic) = self.commit_runtime_generation(candidate).await {
                            self.record_runtime_diagnostic(
                                "clay server configuration commit failed",
                                diagnostic,
                            )
                            .await;
                        }
                    }
                    Err(diagnostic) => {
                        self.record_runtime_diagnostic(
                            "clay server configuration validation failed",
                            diagnostic,
                        )
                        .await;
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.record_runtime_error("clay server configuration failed", error)
                    .await;
            }
        }
    }

    async fn load_configuration_for_service(
        &self,
        service: &ClayJsRuntimeService,
    ) -> Result<Option<js_runtime::ClayRuntimeEvaluation>, js_runtime::ClayRuntimeError> {
        if let Some(config_root) = self.config.configuration_root.clone() {
            service
                .load_configuration_from_root_with_workspace(
                    config_root,
                    Arc::clone(&self.workspace),
                )
                .await
                .map(Some)
        } else {
            service
                .load_default_configuration_with_workspace(Arc::clone(&self.workspace))
                .await
        }
    }

    async fn record_runtime_error(&self, context: &str, error: js_runtime::ClayRuntimeError) {
        self.record_runtime_diagnostic(context, error.diagnostic())
            .await;
    }

    async fn record_runtime_diagnostic(&self, context: &str, diagnostic: RuntimeDiagnostic) {
        eprintln!("{context} [{}]: {}", diagnostic.code, diagnostic.message);
        self.runtime_generation
            .push_diagnostic(diagnostic.clone())
            .await;
        self.runtime_diagnostics.lock().await.push(diagnostic);
    }

    #[doc(hidden)]
    pub async fn trigger_developer_hot_reload(&self) -> RuntimeReloadOutcome {
        self.reload_runtime_generation().await
    }

    /// Arm a test-only barrier that parks the next reload candidate after the
    /// attempt lock is held and before configuration evaluation begins.
    /// Returns `(entered, release)`: await `entered` before submitting edits,
    /// then drop/send on `release` to let the candidate continue.
    #[cfg(test)]
    pub(crate) async fn arm_reload_candidate_barrier(
        &self,
    ) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        self.reload_barrier.arm().await
    }

    pub(crate) async fn execute_reload_command(
        &self,
        request: CommandExecutionRequest,
    ) -> Result<RuntimeReloadOutcome, CommandExecutionDiagnostic> {
        if !is_reload_command(&request.command_id) {
            return Err(CommandExecutionDiagnostic {
                command_id: request.command_id,
                rule: CommandExecutionRule::UnknownCommand,
                message: "command is not the runtime reload command".to_string(),
            });
        }
        CommandExecutor::new().execute(&CommandRegistry::new(), request)?;
        let Ok(_attempt) = Arc::clone(&self.reload_attempt).try_lock_owned() else {
            return Err(CommandExecutionDiagnostic {
                command_id: RELOAD_CONFIGURATION_COMMAND_ID.to_string(),
                rule: CommandExecutionRule::ReloadInProgress,
                message: "runtime reload is already in progress".to_string(),
            });
        };
        Ok(self.reload_runtime_generation_inner().await)
    }

    pub(crate) async fn reload_runtime_generation(&self) -> RuntimeReloadOutcome {
        match self
            .execute_reload_command(CommandExecutionRequest {
                command_id: RELOAD_CONFIGURATION_COMMAND_ID.to_string(),
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            })
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                let generation_id = self.runtime_generation.generation_id().await;
                RuntimeReloadOutcome {
                    previous_generation_id: generation_id,
                    active_generation_id: generation_id,
                    reloaded: false,
                    diagnostics: vec![RuntimeDiagnostic::error(
                        "clay.runtime.reload_in_progress",
                        error.message,
                    )],
                    refreshed_documents: Vec::new(),
                }
            }
        }
    }

    async fn reload_runtime_generation_inner(&self) -> RuntimeReloadOutcome {
        #[cfg(test)]
        self.reload_barrier.wait_if_armed().await;

        let previous_generation_id = self.runtime_generation.generation_id().await;
        let next_generation_id = previous_generation_id.saturating_add(1);
        let next_service = ClayJsRuntimeService::default();

        let evaluation = match self.load_configuration_for_service(&next_service).await {
            Ok(evaluation) => evaluation.unwrap_or_default(),
            Err(error) => {
                let diagnostic = error.diagnostic();
                self.record_runtime_error("clay server runtime reload failed", error)
                    .await;
                return RuntimeReloadOutcome {
                    previous_generation_id,
                    active_generation_id: previous_generation_id,
                    reloaded: false,
                    diagnostics: vec![diagnostic],
                    refreshed_documents: Vec::new(),
                };
            }
        };

        let candidate = match self
            .prepare_runtime_generation_candidate(
                previous_generation_id,
                next_generation_id,
                next_service,
                evaluation,
            )
            .await
        {
            Ok(candidate) => candidate,
            Err(diagnostic) => {
                self.record_runtime_diagnostic(
                    "clay server runtime reload validation failed",
                    diagnostic.clone(),
                )
                .await;
                return RuntimeReloadOutcome {
                    previous_generation_id,
                    active_generation_id: previous_generation_id,
                    reloaded: false,
                    diagnostics: vec![diagnostic],
                    refreshed_documents: Vec::new(),
                };
            }
        };

        match self.commit_runtime_generation(candidate).await {
            Ok(refreshed_documents) => RuntimeReloadOutcome {
                previous_generation_id,
                active_generation_id: next_generation_id,
                reloaded: true,
                diagnostics: Vec::new(),
                refreshed_documents,
            },
            Err(diagnostic) => {
                self.record_runtime_diagnostic(
                    "clay server runtime reload commit failed",
                    diagnostic.clone(),
                )
                .await;
                RuntimeReloadOutcome {
                    previous_generation_id,
                    active_generation_id: previous_generation_id,
                    reloaded: false,
                    diagnostics: vec![diagnostic],
                    refreshed_documents: Vec::new(),
                }
            }
        }
    }

    async fn prepare_runtime_generation_candidate(
        &self,
        expected_generation_id: u64,
        generation_id: u64,
        service: ClayJsRuntimeService,
        evaluation: js_runtime::ClayRuntimeEvaluation,
    ) -> Result<RuntimeGenerationCandidate, RuntimeDiagnostic> {
        let expected_behavior = self.behavior.lock().await.clone();
        let mut behavior = expected_behavior.clone();
        if let Some(manifest) = evaluation.behavior_manifest.clone() {
            let staged = behavior.stage_replacement(manifest).map_err(|_| {
                runtime_candidate_error(
                    "clay.behavior.invalid_manifest",
                    "Runtime behavior manifest failed server validation.",
                )
            })?;
            behavior.install_staged(staged);
        }

        let expected_sdui = self.sdui.lock().await.clone();
        let mut sdui = expected_sdui.clone();
        if let Some(tree) = evaluation.published_sdui_tree.clone() {
            sdui.replace_for_document_with_runtime_tree(sdui.document_id(), tree)
                .map_err(|_| {
                    runtime_candidate_error(
                        "clay.sdui.invalid_tree",
                        "Runtime SDUI tree failed server validation.",
                    )
                })?;
        }

        evaluation.ui_contributions.validate().map_err(|_| {
            runtime_candidate_error(
                "clay.ui.invalid_snapshot",
                "Runtime package UI contributions failed server validation.",
            )
        })?;
        syntax::SyntaxGrammarRegistry::validate_snapshot(
            &evaluation.syntax_grammars,
            &evaluation.syntax_engine_preferences,
        )
        .map_err(|_| {
            runtime_candidate_error(
                "clay.syntax.invalid_snapshot",
                "Runtime syntax grammar contributions failed server validation.",
            )
        })?;
        if let Some(set) = &evaluation.published_decoration_set {
            decorations::validate_decoration_set(set.document_version, set.clone(), None).map_err(
                |_| {
                    runtime_candidate_error(
                        "clay.decorations.invalid_set",
                        "Runtime decoration set failed server validation.",
                    )
                },
            )?;
        }
        if let Some(set) = &evaluation.published_diagnostic_set {
            diagnostics::validate_diagnostic_set(set.document_version, set.clone(), None).map_err(
                |_| {
                    runtime_candidate_error(
                        "clay.diagnostics.invalid_set",
                        "Runtime diagnostic set failed server validation.",
                    )
                },
            )?;
        }

        let expected_typography = self.runtime_generation.active_typography().await;
        let active_typography = stage_typography(
            &expected_typography,
            evaluation.active_typography.clone().unwrap_or_default(),
        )?;
        self.validate_runtime_registrations(generation_id, &service, &evaluation)?;

        let open_documents = self
            .workspace
            .lock()
            .await
            .open_document_snapshots(0)
            .await
            .map_err(|_| {
                runtime_candidate_error(
                    "clay.runtime.reload_refresh_failed",
                    "Reload open-document refresh metadata could not be prepared.",
                )
            })?;
        let expected_theme = self.active_theme.lock().await.clone();
        let active_theme = evaluation
            .active_theme
            .clone()
            .or_else(|| expected_theme.clone())
            .unwrap_or_else(|| crate::protocol::ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
            });
        let runtime_diagnostics = self.runtime_diagnostics.lock().await.clone();
        let runtime_snapshot = build_runtime_state_snapshot(
            generation_id,
            behavior.manifest().clone(),
            active_theme.clone(),
            active_typography.clone(),
            sdui.cloned_tree_or_default(),
            &open_documents,
            evaluation.published_decoration_set.clone(),
            evaluation.published_diagnostic_set.clone(),
            runtime_diagnostics,
        )?;
        // Fail closed before commit when the complete snapshot cannot fit one
        // bounded IPC frame. Partial/live mutation must not begin.
        self.codec
            .encode_server_message(&ServerMessage::RuntimeStateSnapshot(Box::new(
                runtime_snapshot.clone(),
            )))
            .map_err(|_| {
                runtime_candidate_error(
                    "clay.runtime.snapshot_too_large",
                    "Runtime state snapshot exceeds the 1 MiB IPC frame ceiling.",
                )
            })?;
        let published_theme = evaluation.active_theme.clone();
        let evaluation = Arc::new(evaluation);
        Ok(RuntimeGenerationCandidate {
            expected_generation_id,
            generation: RuntimeGeneration {
                id: generation_id,
                service,
                evaluation: Some(evaluation),
                diagnostics: Vec::new(),
            },
            expected_behavior,
            behavior,
            expected_sdui,
            sdui,
            expected_theme,
            active_theme: published_theme,
            expected_typography,
            active_typography,
            open_documents,
            runtime_snapshot,
        })
    }

    fn validate_runtime_registrations(
        &self,
        generation_id: u64,
        service: &ClayJsRuntimeService,
        evaluation: &js_runtime::ClayRuntimeEvaluation,
    ) -> Result<(), RuntimeDiagnostic> {
        let parse = ParseCoordinator::new();
        let completion = completion::CompletionCoordinator::new();
        let document_analysis = document_analysis::DocumentAnalysisCoordinator::default();
        let language_intelligence = LanguageIntelligenceCoordinator::new();
        register_runtime_contributions(
            generation_id,
            service,
            evaluation,
            &parse,
            &completion,
            &document_analysis,
            &language_intelligence,
        )
    }

    async fn commit_runtime_generation(
        &self,
        candidate: RuntimeGenerationCandidate,
    ) -> Result<Vec<ReloadedDocumentRefresh>, RuntimeDiagnostic> {
        let behavior_lock = self
            .scoped_locks
            .try_acquire(ScopedLockTarget::Behavior, LockOwner::Server)
            .map_err(|_| {
                runtime_candidate_error(
                    "clay.runtime.behavior_locked",
                    "Runtime behavior state is locked by another server operation.",
                )
            })?;
        if self.runtime_generation.generation_id().await != candidate.expected_generation_id {
            return Err(runtime_candidate_error(
                "clay.runtime.generation_conflict",
                "Runtime generation changed before the prepared candidate could commit.",
            ));
        }

        let mut behavior = self.behavior.lock().await;
        let mut sdui = self.sdui.lock().await;
        let mut active_theme = self.active_theme.lock().await;
        let mut active_typography = self.runtime_generation.typography.current.lock().await;
        if *behavior != candidate.expected_behavior
            || *sdui != candidate.expected_sdui
            || *active_theme != candidate.expected_theme
            || *active_typography != candidate.expected_typography
        {
            return Err(runtime_candidate_error(
                "clay.runtime.active_state_conflict",
                "Active runtime state changed before the prepared candidate could commit.",
            ));
        }

        let Some(evaluation) = candidate.generation.evaluation.as_deref() else {
            return Err(runtime_candidate_error(
                "clay.runtime.incomplete_candidate",
                "Prepared runtime generation is missing validated evaluation state.",
            ));
        };
        register_runtime_contributions(
            candidate.generation.id,
            &candidate.generation.service,
            evaluation,
            &self.parse_coordinator,
            &self.completion,
            &self.document_analysis,
            &self.language_intelligence,
        )?;

        let previous_generation = self.runtime_generation.current().await;
        let previous_generation_id = candidate.expected_generation_id;
        let previous_behavior_manifest = behavior.manifest().clone();
        let typography_changed = *active_typography != candidate.active_typography;
        *behavior = candidate.behavior;
        *sdui = candidate.sdui;
        *active_theme = candidate.active_theme;
        *active_typography = candidate.active_typography.clone();
        self.runtime_generation
            .swap(candidate.generation.clone())
            .await;

        if candidate.generation.id != previous_generation_id {
            cancel_older_runtime_generations(
                candidate.generation.id,
                &self.parse_coordinator,
                &self.completion,
                &self.document_analysis,
                &self.language_intelligence,
            );
            // Old grants/process authority end at commit. Cleanup failure must
            // not restore previous-generation executable handlers.
            let _ = previous_generation
                .service
                .shutdown_generation_resources()
                .await;
            // Retain only the immediately previous inert manifest for bounded
            // stale Edit/EditorIntent acceptance. Executable authority is gone.
            self.runtime_generation
                .behavior_grace()
                .begin(previous_behavior_manifest, previous_generation_id)
                .await;
        } else {
            self.runtime_generation.behavior_grace().clear().await;
        }
        drop(active_typography);
        drop(active_theme);
        drop(sdui);
        drop(behavior);
        if typography_changed {
            let _ = self
                .runtime_generation
                .typography
                .updates
                .send(candidate.active_typography);
        }
        self.runtime_generation
            .publish_runtime_snapshot(candidate.runtime_snapshot)
            .await;
        drop(behavior_lock);

        Ok(self
            .refresh_open_documents_after_reload(
                candidate.generation.id,
                &candidate.generation.service,
                candidate.open_documents,
            )
            .await)
    }

    async fn refresh_open_documents_after_reload(
        &self,
        generation_id: u64,
        service: &ClayJsRuntimeService,
        snapshots: Vec<workspace::OpenDocumentSnapshot>,
    ) -> Vec<ReloadedDocumentRefresh> {
        let roots = self.workspace.lock().await.directory_roots();
        let mut refreshed = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            let mut messages = connection::open_document_followup_messages(
                &snapshot.metadata,
                &snapshot.text,
                &self.behavior,
                &self.sdui,
                generation_id,
                service,
                &self.parse_coordinator,
            )
            .await;
            if let Some(root) = roots
                .iter()
                .find(|root| root.workspace_root_id == snapshot.metadata.workspace_root_id)
            {
                let manifest_id = self.behavior.lock().await.manifest().manifest_id.clone();
                let active_mode = manifest_id.rsplit('.').next().unwrap_or(&manifest_id);
                messages.extend(
                    self.document_analysis
                        .open_document(
                            generation_id,
                            &snapshot.metadata,
                            active_mode,
                            root.canonical_path.clone(),
                            snapshot.text.clone(),
                        )
                        .into_iter()
                        .map(ServerMessage::RuntimeDiagnostic),
                );
            }
            refreshed.push(ReloadedDocumentRefresh {
                document_id: snapshot.metadata.document_id,
                messages,
            });
        }
        refreshed
    }

    fn spawn_connection<S>(&self, stream: S, connections: &mut JoinSet<()>)
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let client_id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
        let document = Arc::clone(&self.document);
        let behavior = Arc::clone(&self.behavior);
        let workspace = Arc::clone(&self.workspace);
        let sdui = Arc::clone(&self.sdui);
        let active_theme = Arc::clone(&self.active_theme);
        let runtime_diagnostics = Arc::clone(&self.runtime_diagnostics);
        let runtime_generation = self.runtime_generation.clone();
        let parse_coordinator = self.parse_coordinator.clone();
        let completion = self.completion.clone();
        let document_analysis = self.document_analysis.clone();
        let language_intelligence = self.language_intelligence.clone();
        let reload_server = IpcServer::clone(self);
        let codec = self.codec;
        connections.spawn(async move {
            if let Err(error) = handle_connection_with_analysis(
                stream,
                client_id,
                document,
                behavior,
                workspace,
                sdui,
                active_theme,
                runtime_diagnostics,
                runtime_generation,
                parse_coordinator,
                completion,
                document_analysis,
                language_intelligence,
                Some(reload_server),
                codec,
            )
            .await
            {
                eprintln!("clay server connection {client_id} closed with error: {error}");
            }
        });
    }
}

fn register_runtime_contributions(
    generation_id: u64,
    service: &ClayJsRuntimeService,
    evaluation: &js_runtime::ClayRuntimeEvaluation,
    parse: &ParseCoordinator,
    completion: &completion::CompletionCoordinator,
    document_analysis: &document_analysis::DocumentAnalysisCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
) -> Result<(), RuntimeDiagnostic> {
    service
        .register_parse_handlers(parse, generation_id, evaluation)
        .map_err(|_| {
            runtime_candidate_error(
                "clay.parse.registration_failed",
                "Runtime parse handler registration failed validation.",
            )
        })?;
    service
        .register_completion_providers(completion, generation_id, evaluation)
        .map_err(|_| {
            runtime_candidate_error(
                "clay.completion.registration_failed",
                "Runtime completion provider registration failed validation.",
            )
        })?;
    for registration in &evaluation.document_analyzers {
        if !service.document_analysis_registration_authorized(registration) {
            return Err(runtime_candidate_error(
                "clay.analysis.unauthorized",
                "Runtime document analyzer lacks an exact current package/process grant.",
            ));
        }
        document_analysis
            .register(
                generation_id,
                service.clone(),
                registration.clone(),
                completion,
                language_intelligence,
            )
            .map_err(|_| {
                runtime_candidate_error(
                    "clay.analysis.registration_failed",
                    "Runtime document analyzer registration failed validation.",
                )
            })?;
    }
    service
        .register_language_intelligence_providers(language_intelligence, generation_id, evaluation)
        .map_err(|_| {
            runtime_candidate_error(
                "clay.language.registration_failed",
                "Runtime language-intelligence provider registration failed validation.",
            )
        })?;
    Ok(())
}

/// Shared post-commit cleanup for every generation-owned coordinator registry.
/// Package disable/revoke uses [`withdraw_package_contributions`] for the
/// package-scoped variant of the same cancel primitives.
fn cancel_older_runtime_generations(
    active_generation: u64,
    parse: &ParseCoordinator,
    completion: &completion::CompletionCoordinator,
    document_analysis: &document_analysis::DocumentAnalysisCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
) {
    parse.cancel_older_generations(active_generation);
    completion.cancel_older_generations(active_generation);
    document_analysis.cancel_older_generations(active_generation);
    language_intelligence.cancel_older_generations(active_generation);
}

/// Shared package-scoped withdrawal used by disable/revoke and any future
/// mid-generation package removal. Reload uses generation cancel instead, but
/// both paths share these coordinator primitives.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "package-disable wiring reuses this helper when a live server disable path lands"
    )
)]
pub(crate) fn withdraw_package_contributions(
    package_name: &str,
    package_prefix: &str,
    parse: &ParseCoordinator,
    completion: &completion::CompletionCoordinator,
    document_analysis: &document_analysis::DocumentAnalysisCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
) {
    parse.cancel_package(package_prefix);
    completion.cancel_package(package_prefix);
    language_intelligence.cancel_package(package_prefix);
    document_analysis.cancel_package(package_name);
}

fn stage_typography(
    current: &crate::protocol::ActiveTypography,
    mut requested: crate::protocol::ActiveTypography,
) -> Result<crate::protocol::ActiveTypography, RuntimeDiagnostic> {
    requested.validate().map_err(|_| {
        runtime_candidate_error(
            "clay.typography.invalid_configuration",
            "Runtime typography failed server validation.",
        )
    })?;
    requested.revision = if current.monospace == requested.monospace
        && current.proportional == requested.proportional
        && current.ui == requested.ui
    {
        current.revision
    } else {
        current.revision.saturating_add(1)
    };
    Ok(requested)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one prepare helper keeps every snapshot field explicit before commit"
)]
fn build_runtime_state_snapshot(
    runtime_generation_id: RuntimeGenerationId,
    behavior: crate::protocol::BehaviorManifest,
    active_theme: crate::protocol::ActiveTheme,
    active_typography: crate::protocol::ActiveTypography,
    sdui_tree: crate::protocol::SduiTree,
    open_documents: &[workspace::OpenDocumentSnapshot],
    published_decorations: Option<crate::protocol::DecorationSet>,
    published_diagnostics: Option<crate::protocol::DiagnosticSet>,
    diagnostics: Vec<RuntimeDiagnostic>,
) -> Result<RuntimeStateSnapshot, RuntimeDiagnostic> {
    let package_ui = crate::protocol::PackageUiSnapshot {
        // Package UI payloads are not on the wire yet; advance the version with
        // the runtime generation so clients clear previous package UI under the
        // same atomic install boundary.
        version: runtime_generation_id,
    };
    let documents = open_documents
        .iter()
        .map(|document| {
            let document_id = document.metadata.document_id;
            crate::protocol::DocumentRuntimeRenderState {
                document_id,
                document_version: document.metadata.version,
                reset_decorations: true,
                reset_diagnostics: true,
                initial_decorations: published_decorations
                    .clone()
                    .filter(|set| set.document_id == document_id),
                initial_diagnostics: published_diagnostics
                    .clone()
                    .filter(|set| set.document_id == document_id),
            }
        })
        .collect();
    let snapshot = RuntimeStateSnapshot {
        runtime_generation_id,
        client_id: 0,
        behavior,
        active_theme,
        active_typography,
        sdui_tree,
        package_ui,
        documents,
        diagnostics,
    };
    snapshot.validate().map_err(|_| {
        runtime_candidate_error(
            "clay.runtime.invalid_snapshot",
            "Runtime state snapshot failed validation before commit.",
        )
    })?;
    Ok(snapshot)
}

fn runtime_candidate_error(code: &'static str, message: &'static str) -> RuntimeDiagnostic {
    RuntimeDiagnostic::error(code, message)
}

/// Outcome of applying a [`js_runtime::ClayRuntimeEvaluation`]'s shared
/// outputs to server state.
///
/// Built by [`apply_runtime_outputs`] for explicit runtime/config publication
/// and [`apply_runtime_outputs_without_sdui`] for open-document follow-ups.
/// This keeps Clay-owned workspace chrome separate from package/open-time SDUI
/// while preserving one result shape for behavior/decorations diagnostics.
#[derive(Default)]
pub(crate) struct RuntimeOutputApplication {
    /// `Some(Ok(installed))` when a behavior manifest replaced the active
    /// manifest; `Some(Err(()))` when the manifest failed validation; `None`
    /// when the evaluation carried no manifest.
    #[allow(
        dead_code,
        reason = "open-time publication ignores installed behavior metadata"
    )]
    pub(crate) behavior: Option<Result<crate::protocol::BehaviorManifest, ()>>,
    /// `Some(Ok(tree))` when a runtime tree replaced the per-document SDUI
    /// state — the caller builds the `SduiSnapshot` message with its own
    /// `client_id`. `Some(Err(()))` on validation failure; `None` when no
    /// tree was published.
    #[allow(
        dead_code,
        reason = "open-time publication intentionally skips shared SDUI"
    )]
    pub(crate) sdui: Option<Result<crate::protocol::SduiTree, ()>>,
    /// Published decoration set, passed through for the caller to emit. The
    /// config-eval boundary holds no per-document decoration store, so this is
    /// not applied to shared state here.
    #[allow(
        dead_code,
        reason = "selected-file activation consumes decoration output directly; startup config keeps it for future caller parity"
    )]
    pub(crate) decorations: Option<crate::protocol::DecorationSet>,
    /// Published range-diagnostic set, passed through for the caller to emit.
    #[allow(
        dead_code,
        reason = "package diagnostic publication is observed via evaluation; live delivery uses protocol DiagnosticSet"
    )]
    pub(crate) diagnostic_set: Option<crate::protocol::DiagnosticSet>,
}

impl RuntimeOutputApplication {
    /// Unified diagnostics for outputs that failed validation. Both call sites
    /// surface these so the diagnostic codes stay identical across flows
    /// (`clay.behavior.invalid_manifest`, `clay.sdui.invalid_tree`).
    #[cfg(test)]
    pub(crate) fn diagnostics(&self) -> Vec<RuntimeDiagnostic> {
        let mut diagnostics = Vec::new();
        if matches!(self.behavior, Some(Err(()))) {
            diagnostics.push(RuntimeDiagnostic::error(
                "clay.behavior.invalid_manifest",
                "Runtime behavior manifest failed server validation.",
            ));
        }
        if matches!(self.sdui, Some(Err(()))) {
            diagnostics.push(RuntimeDiagnostic::error(
                "clay.sdui.invalid_tree",
                "Published SDUI tree failed server validation.",
            ));
        }
        diagnostics
    }
}

/// Apply explicit runtime/config outputs to shared server state: behavior and
/// per-document SDUI tree.
///
/// `ui_contributions` on the evaluation are intentionally not applied here:
/// the shell owns the package-UI registry; `IpcServer` does not hold one to
/// merge a `PackageUiRegistrySnapshot` into. JS parse handlers are registered
/// separately by the runtime-generation candidate because they need the
/// persistent runtime service, not just this open-document output primitive.
#[cfg(test)]
pub(crate) async fn apply_runtime_outputs(
    evaluation: &js_runtime::ClayRuntimeEvaluation,
    document_id: crate::protocol::DocumentId,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
) -> RuntimeOutputApplication {
    let behavior_out = match &evaluation.behavior_manifest {
        Some(manifest) => Some(
            behavior
                .lock()
                .await
                .publish_replacement(manifest.clone())
                .map_err(|_| ()),
        ),
        None => None,
    };

    let sdui_out = match evaluation.published_sdui_tree.clone() {
        Some(tree) => {
            let applied = sdui
                .lock()
                .await
                .replace_for_document_with_runtime_tree(document_id, tree.clone())
                .map(|_| tree)
                .map_err(|_| ());
            Some(applied)
        }
        None => None,
    };

    RuntimeOutputApplication {
        behavior: behavior_out,
        sdui: sdui_out,
        decorations: evaluation.published_decoration_set.clone(),
        diagnostic_set: evaluation.published_diagnostic_set.clone(),
    }
}

/// Apply open-document runtime outputs without replacing shared SDUI state.
///
/// Open-time classification may load packages or activate modes, but that path
/// must not erase Clay-owned file-browser validation state. Explicit runtime
/// SDUI publication still goes through [`apply_runtime_outputs`].
pub(crate) async fn apply_runtime_outputs_without_sdui(
    evaluation: &js_runtime::ClayRuntimeEvaluation,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
) -> RuntimeOutputApplication {
    let behavior_out = match &evaluation.behavior_manifest {
        Some(manifest) => Some(
            behavior
                .lock()
                .await
                .publish_replacement(manifest.clone())
                .map_err(|_| ()),
        ),
        None => None,
    };

    RuntimeOutputApplication {
        behavior: behavior_out,
        sdui: None,
        decorations: evaluation.published_decoration_set.clone(),
        diagnostic_set: evaluation.published_diagnostic_set.clone(),
    }
}

#[cfg(unix)]
fn bind_unix_listener(socket_path: &Path) -> Result<UnixListener, ServerError> {
    validate_socket_path(socket_path)?;
    remove_stale_socket(socket_path)?;
    let listener = UnixListener::bind(socket_path).map_err(ServerError::Bind)?;
    restrict_unix_socket_permissions(socket_path)?;
    Ok(listener)
}

#[cfg(unix)]
fn restrict_unix_socket_permissions(socket_path: &Path) -> Result<(), ServerError> {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    let permissions = Permissions::from_mode(0o600);
    fs::set_permissions(socket_path, permissions).map_err(ServerError::EndpointPermissions)
}

#[cfg(unix)]
fn validate_socket_path(socket_path: &Path) -> Result<(), ServerError> {
    if socket_path.as_os_str().is_empty() {
        return Err(ServerError::InvalidEndpoint(
            "socket path must not be empty".to_string(),
        ));
    }

    let Some(parent) = socket_path.parent() else {
        return Err(ServerError::InvalidEndpoint(
            "socket path must have a parent directory".to_string(),
        ));
    };
    let metadata = fs::metadata(parent).map_err(ServerError::EndpointDirectory)?;
    if !metadata.is_dir() {
        return Err(ServerError::InvalidEndpoint(format!(
            "socket parent {} is not a directory",
            parent.display()
        )));
    }

    validate_parent_directory_ownership(parent, &metadata)?;

    Ok(())
}

#[cfg(unix)]
fn validate_parent_directory_ownership(
    parent: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ServerError> {
    use std::os::unix::fs::MetadataExt;

    let dir_uid = metadata.uid();
    // SAFETY: getuid has no failure mode and is async-signal-safe.
    let process_uid = unsafe { libc::getuid() };
    if dir_uid != process_uid {
        return Err(ServerError::EndpointOwnership(format!(
            "socket parent {} is owned by uid {}, but this process runs as uid {}. \
             Refusing to create an IPC endpoint in a directory not owned by the current user.",
            parent.display(),
            dir_uid,
            process_uid
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn remove_stale_socket(socket_path: &Path) -> Result<(), ServerError> {
    let Ok(metadata) = fs::symlink_metadata(socket_path) else {
        return Ok(());
    };

    if metadata.file_type().is_socket() {
        fs::remove_file(socket_path).map_err(ServerError::RemoveStaleSocket)?;
        return Ok(());
    }

    Err(ServerError::InvalidEndpoint(format!(
        "refusing to replace non-socket path {}",
        socket_path.display()
    )))
}

#[cfg(windows)]
fn create_named_pipe_server(pipe_name: &str) -> Result<NamedPipeServer, ServerError> {
    let mut security =
        CurrentUserSecurityAttributes::new().map_err(ServerError::InvalidEndpoint)?;
    // SAFETY: `security.attributes` points at heap-allocated descriptor/ACL
    // owned by `security`, which outlives this synchronous CreateNamedPipe call.
    unsafe {
        ServerOptions::new()
            .create_with_security_attributes_raw(
                pipe_name,
                &mut security.attributes as *mut _ as *mut std::ffi::c_void,
            )
            .map_err(ServerError::Bind)
    }
    // `security` drops here, freeing the descriptor and ACL after the pipe is created.
}

#[cfg(windows)]
struct CurrentUserSecurityAttributes {
    token_user: windows::Win32::Foundation::HLOCAL,
    acl: windows::Win32::Foundation::HLOCAL,
    #[allow(dead_code)]
    security_descriptor: Box<SECURITY_DESCRIPTOR>,
    attributes: SECURITY_ATTRIBUTES,
}

#[cfg(windows)]
impl CurrentUserSecurityAttributes {
    fn new() -> Result<Self, String> {
        // Standard access-mask constants; the `windows` crate does not expose
        // GENERIC_ALL as a standalone constant in this version.
        const GENERIC_ALL: u32 = 0x1000_0000;
        const SECURITY_DESCRIPTOR_REVISION: u32 = 1;

        unsafe {
            // Open the current process token to read the user SID.
            let mut token = HANDLE(std::ptr::null_mut());
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|error| format!("OpenProcessToken failed: {error}"))?;

            // Query the size required for TokenUser.
            let mut required = 0u32;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut required);

            // Allocate the TokenUser buffer.
            let token_user = LocalAlloc(LPTR, required as usize).map_err(|error| {
                let _ = CloseHandle(token);
                format!("LocalAlloc failed for token user buffer: {error}")
            })?;
            if token_user.0.is_null() {
                let _ = CloseHandle(token);
                return Err("LocalAlloc returned null for token user buffer".to_string());
            }

            // Read TokenUser.
            if let Err(error) = GetTokenInformation(
                token,
                TokenUser,
                Some(token_user.0),
                required,
                &mut required,
            ) {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                return Err(format!("GetTokenInformation failed: {error}"));
            }

            let user = &*(token_user.0 as *const TOKEN_USER);
            let user_sid: PSID = user.User.Sid;

            // Build a DACL containing one ACE that grants the current user full access.
            let sid_length = GetLengthSid(user_sid);
            let acl_size = std::mem::size_of::<ACL>() as u32
                + sid_length
                + (std::mem::size_of::<ACCESS_ALLOWED_ACE>() as u32)
                - (std::mem::size_of::<u32>() as u32);
            let acl = LocalAlloc(LPTR, acl_size as usize).map_err(|error| {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                format!("LocalAlloc failed for ACL: {error}")
            })?;
            if acl.0.is_null() {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                return Err("LocalAlloc returned null for ACL".to_string());
            }

            InitializeAcl(acl.0 as *mut ACL, acl_size, ACL_REVISION).map_err(|error| {
                let _ = CloseHandle(token);
                let _ = LocalFree(Some(token_user));
                let _ = LocalFree(Some(acl));
                format!("InitializeAcl failed: {error}")
            })?;

            // GENERIC_ALL is broader than needed for a pipe, but it mirrors the
            // creator-owner rights the default DACL grants. For a least-privilege
            // refinement, use FILE_GENERIC_READ | FILE_GENERIC_WRITE.
            AddAccessAllowedAce(acl.0 as *mut ACL, ACL_REVISION, GENERIC_ALL, user_sid).map_err(
                |error| {
                    let _ = CloseHandle(token);
                    let _ = LocalFree(Some(token_user));
                    let _ = LocalFree(Some(acl));
                    format!("AddAccessAllowedAce failed: {error}")
                },
            )?;

            let _ = CloseHandle(token);

            // Build a security descriptor owning the DACL.
            let mut security_descriptor = Box::new(std::mem::zeroed());
            let sd_ptr =
                PSECURITY_DESCRIPTOR(&mut *security_descriptor as *mut _ as *mut std::ffi::c_void);
            InitializeSecurityDescriptor(sd_ptr, SECURITY_DESCRIPTOR_REVISION).map_err(
                |error| {
                    let _ = LocalFree(Some(token_user));
                    let _ = LocalFree(Some(acl));
                    format!("InitializeSecurityDescriptor failed: {error}")
                },
            )?;

            SetSecurityDescriptorDacl(sd_ptr, true, Some(acl.0 as *mut ACL), false).map_err(
                |error| {
                    let _ = LocalFree(Some(token_user));
                    let _ = LocalFree(Some(acl));
                    format!("SetSecurityDescriptorDacl failed: {error}")
                },
            )?;

            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd_ptr.0,
                bInheritHandle: false.into(),
            };

            Ok(Self {
                token_user,
                acl,
                security_descriptor,
                attributes,
            })
        }
    }
}

#[cfg(windows)]
impl Drop for CurrentUserSecurityAttributes {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(self.token_user));
            let _ = LocalFree(Some(self.acl));
            // self.security_descriptor is freed by Box::drop.
        }
    }
}

#[cfg(windows)]
async fn connect_named_pipe_server(pipe: NamedPipeServer) -> io::Result<NamedPipeServer> {
    match pipe.connect().await {
        Ok(()) => Ok(pipe),
        Err(error) if error.raw_os_error() == Some(ERROR_PIPE_CONNECTED) => Ok(pipe),
        Err(error) => Err(error),
    }
}

#[derive(Debug)]
pub enum ServerError {
    InvalidEndpoint(String),
    EndpointDirectory(io::Error),
    EndpointOwnership(String),
    EndpointPermissions(io::Error),
    RemoveStaleSocket(io::Error),
    Bind(io::Error),
    Accept(io::Error),
    InvalidWorkspaceRoot(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint(message) => write!(formatter, "invalid IPC endpoint: {message}"),
            Self::EndpointDirectory(error) => {
                write!(
                    formatter,
                    "failed to inspect IPC endpoint directory: {error}"
                )
            }
            Self::EndpointOwnership(message) => {
                write!(
                    formatter,
                    "IPC endpoint directory ownership check failed: {message}"
                )
            }
            Self::EndpointPermissions(error) => {
                write!(
                    formatter,
                    "failed to restrict IPC endpoint permissions: {error}"
                )
            }
            Self::RemoveStaleSocket(error) => {
                write!(formatter, "failed to remove stale socket: {error}")
            }
            Self::Bind(error) => write!(formatter, "failed to bind IPC endpoint: {error}"),
            Self::Accept(error) => write!(formatter, "failed to accept IPC connection: {error}"),
            Self::InvalidWorkspaceRoot(message) => {
                write!(formatter, "invalid workspace root: {message}")
            }
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::EndpointDirectory(error)
            | Self::EndpointPermissions(error)
            | Self::RemoveStaleSocket(error)
            | Self::Bind(error)
            | Self::Accept(error) => Some(error),
            Self::InvalidEndpoint(_)
            | Self::EndpointOwnership(_)
            | Self::InvalidWorkspaceRoot(_) => None,
        }
    }
}

#[cfg(test)]
mod runtime_outputs_tests {
    use std::sync::Arc;

    use tokio::sync::Mutex;

    use super::{
        ActiveBehaviorManifest, StaticSduiState, apply_runtime_outputs,
        apply_runtime_outputs_without_sdui,
    };
    use crate::{
        protocol::{
            BehaviorManifest, DecorationSet, DocumentId, SduiActionIntent, SduiActionSource,
            SduiNodeId,
        },
        server::{js_runtime::ClayRuntimeEvaluation, sdui::default_document_tree},
    };

    fn valid_manifest() -> BehaviorManifest {
        let mut manifest = BehaviorManifest::minimal_text_editing(99);
        manifest.manifest_id = "clay.test.manifest".to_string();
        manifest
    }

    fn empty_decoration_set(document_id: DocumentId) -> DecorationSet {
        DecorationSet {
            document_id,
            document_version: 1,
            viewport_byte_start: 0,
            viewport_byte_end: 0,
            spans: vec![],
        }
    }

    fn harness(
        document_id: DocumentId,
    ) -> (
        Arc<Mutex<ActiveBehaviorManifest>>,
        Arc<Mutex<StaticSduiState>>,
    ) {
        (
            Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            Arc::new(Mutex::new(StaticSduiState::empty_for_document(document_id))),
        )
    }

    /// Behavior manifest and SDUI tree are applied to shared state in one
    /// primitive, and no diagnostics are produced when both are valid.
    #[tokio::test]
    async fn apply_runtime_outputs_applies_behavior_and_sdui_to_shared_state() {
        let (behavior, sdui) = harness(1);
        let evaluation = ClayRuntimeEvaluation {
            op_records: vec![],
            published_sdui_tree: Some(default_document_tree(1, 1)),
            published_decoration_set: None,
            published_diagnostic_set: None,
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: Some(valid_manifest()),
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            syntax_engine_preferences: Default::default(),
            completion_providers: vec![],
            js_completion_providers: vec![],
            language_intelligence_providers: vec![],
            js_language_intelligence_providers: vec![],
            document_analyzers: vec![],
            active_theme: None,
            active_typography: None,
        };

        let application = apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;

        assert!(
            application.diagnostics().is_empty(),
            "no diagnostics for valid outputs"
        );
        assert!(
            application.behavior.is_some_and(|r| r.is_ok()),
            "behavior applied"
        );
        assert!(application.sdui.is_some_and(|r| r.is_ok()), "sdui applied");
        assert_eq!(
            behavior.lock().await.version(),
            2,
            "shared behavior advanced"
        );
    }

    #[tokio::test]
    async fn open_time_runtime_sdui_output_does_not_replace_workspace_browser_state() {
        let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
        let sdui = Arc::new(Mutex::new(StaticSduiState::for_document(1, 1)));
        let evaluation = ClayRuntimeEvaluation {
            op_records: vec![],
            published_sdui_tree: Some(default_document_tree(2, 1)),
            published_decoration_set: Some(empty_decoration_set(1)),
            published_diagnostic_set: None,
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: Some(valid_manifest()),
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            syntax_engine_preferences: Default::default(),
            completion_providers: vec![],
            js_completion_providers: vec![],
            language_intelligence_providers: vec![],
            js_language_intelligence_providers: vec![],
            document_analyzers: vec![],
            active_theme: None,
            active_typography: None,
        };

        let application = apply_runtime_outputs_without_sdui(&evaluation, &behavior).await;

        assert!(application.sdui.is_none(), "open-time SDUI is ignored");
        assert!(application.behavior.is_some_and(|result| result.is_ok()));
        assert!(application.decorations.is_some());
        sdui.lock()
            .await
            .validate_action(&SduiActionIntent::command(
                "workspace.refresh",
                SduiActionSource::Button {
                    node_id: SduiNodeId(5),
                },
            ))
            .expect("original workspace browser action still validates");
    }

    /// An SDUI tree bound to a different document fails per-document
    /// validation and surfaces the unified `clay.sdui.invalid_tree` diagnostic
    /// regardless of which flow called the primitive.
    #[tokio::test]
    async fn apply_runtime_outputs_reports_unified_diagnostic_for_invalid_sdui() {
        let (behavior, sdui) = harness(1);
        // Tree built for document 2, applied against document 1 -> binding
        // validation fails.
        let evaluation = ClayRuntimeEvaluation {
            op_records: vec![],
            published_sdui_tree: Some(default_document_tree(2, 1)),
            published_decoration_set: None,
            published_diagnostic_set: None,
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: None,
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            syntax_engine_preferences: Default::default(),
            completion_providers: vec![],
            js_completion_providers: vec![],
            language_intelligence_providers: vec![],
            js_language_intelligence_providers: vec![],
            document_analyzers: vec![],
            active_theme: None,
            active_typography: None,
        };

        let application = apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;

        assert!(
            matches!(application.sdui, Some(Err(()))),
            "sdui failed validation"
        );
        let diagnostics = application.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "clay.sdui.invalid_tree");
        assert_eq!(
            behavior.lock().await.version(),
            1,
            "behavior untouched when absent"
        );
    }

    /// Decoration sets are passed through for the caller to emit; the
    /// config-eval boundary holds no decoration store, so they are not applied
    /// to shared state here. This makes the previously-silent drop explicit.
    #[tokio::test]
    async fn apply_runtime_outputs_passes_decorations_through() {
        let (behavior, sdui) = harness(1);
        let set = empty_decoration_set(1);
        let evaluation = ClayRuntimeEvaluation {
            op_records: vec![],
            published_sdui_tree: None,
            published_decoration_set: Some(set.clone()),
            published_diagnostic_set: None,
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: None,
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            syntax_engine_preferences: Default::default(),
            completion_providers: vec![],
            js_completion_providers: vec![],
            language_intelligence_providers: vec![],
            js_language_intelligence_providers: vec![],
            document_analyzers: vec![],
            active_theme: None,
            active_typography: None,
        };

        let application = apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;

        assert_eq!(
            application.decorations,
            Some(set),
            "decorations passed through"
        );
        assert!(application.behavior.is_none(), "no manifest present");
        assert!(application.sdui.is_none(), "no tree present");
        assert!(application.diagnostics().is_empty());
    }
}

#[cfg(test)]
mod runtime_generation_tests {
    use std::{
        fs,
        time::{Duration, SystemTime},
    };

    use crate::{
        ipc::IpcEndpoint,
        packages::record::assemble_package_record,
        protocol::{
            ActiveTheme, BehaviorManifest, FontProfile, IncrementalParseUpdate, ParseByteRange,
            ParseEditNotification, ServerMessage,
        },
        server::{
            command_execution::{
                CommandExecutionRequest, CommandExecutionRule, CommandExecutionTarget,
                RELOAD_CONFIGURATION_COMMAND_ID,
            },
            completion::BufferWordCompletionProvider,
            js_runtime::ClayRuntimeEvaluation,
            language_intelligence::LanguageIntelligenceProviderMeta,
            parse_coordinator::ParseScheduleRequest,
            sdui::default_document_tree,
            withdraw_package_contributions,
        },
    };

    use super::{IpcServer, ServerConfig};

    fn temp_config_root(name: &str, init_js: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "clay-runtime-generation-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("init.js"), init_js).unwrap();
        root
    }

    fn server_with_config(root: std::path::PathBuf) -> IpcServer {
        let mut config = ServerConfig::new(IpcEndpoint::from_argument("runtime-generation-test"));
        config.configuration_root = Some(root);
        IpcServer::new(config)
    }

    fn reload_request() -> CommandExecutionRequest {
        CommandExecutionRequest {
            command_id: RELOAD_CONFIGURATION_COMMAND_ID.to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::Global,
            provenance: None,
            expected_permissions: Vec::new(),
        }
    }

    #[tokio::test]
    async fn concurrent_reload_commands_commit_at_most_one_candidate_at_a_time() {
        let root = temp_config_root("reload-in-progress", "");
        let server = server_with_config(root);
        let active_attempt = server.reload_attempt.lock().await;

        let error = server
            .execute_reload_command(reload_request())
            .await
            .expect_err("concurrent reload must not queue");
        assert_eq!(error.rule, CommandExecutionRule::ReloadInProgress);
        assert_eq!(server.runtime_generation.generation_id().await, 1);

        drop(active_attempt);
        assert!(
            server
                .execute_reload_command(reload_request())
                .await
                .expect("next reload runs after release")
                .reloaded
        );
    }

    #[tokio::test]
    async fn failed_reload_releases_attempt_lock() {
        let root = temp_config_root("reload-lock-release", "const = broken;");
        let server = server_with_config(root.clone());

        assert!(
            !server
                .execute_reload_command(reload_request())
                .await
                .expect("invalid configuration is a completed reload attempt")
                .reloaded
        );
        fs::write(root.join("init.js"), "").unwrap();
        assert!(
            server
                .execute_reload_command(reload_request())
                .await
                .expect("failed attempt released reload locks")
                .reloaded
        );
    }

    #[tokio::test]
    async fn failed_candidate_commit_releases_behavior_lock() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "candidate-lock-release",
        )));
        let candidate = server
            .prepare_runtime_generation_candidate(
                1,
                2,
                super::ClayJsRuntimeService::default(),
                ClayRuntimeEvaluation::default(),
            )
            .await
            .expect("prepare candidate");
        *server.active_theme.lock().await = Some(ActiveTheme {
            specifier: "@clay/conflict".to_string(),
            overrides: Vec::new(),
        });

        assert!(server.commit_runtime_generation(candidate).await.is_err());

        let next = server
            .prepare_runtime_generation_candidate(
                1,
                2,
                super::ClayJsRuntimeService::default(),
                ClayRuntimeEvaluation::default(),
            )
            .await
            .expect("prepare replacement candidate");
        assert!(server.commit_runtime_generation(next).await.is_ok());
    }

    #[tokio::test]
    async fn candidate_validation_failure_changes_no_active_state() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "candidate-validation-rollback",
        )));
        let generation_before = server.runtime_generation.current().await;
        let behavior_before = server.behavior.lock().await.clone();
        let sdui_before = server.sdui.lock().await.clone();
        let theme_before = server.active_theme.lock().await.clone();
        let typography_before = server.runtime_generation.active_typography().await;
        let completion_before = server.completion.providers();
        let intelligence_before = server.language_intelligence.providers();

        let mut evaluation = ClayRuntimeEvaluation::default();
        let mut manifest = BehaviorManifest::minimal_text_editing(99);
        manifest.manifest_id = "clay.test.candidate".to_string();
        evaluation.behavior_manifest = Some(manifest);
        evaluation.published_sdui_tree = Some(default_document_tree(2, 1));

        let result = server
            .prepare_runtime_generation_candidate(
                1,
                2,
                super::ClayJsRuntimeService::default(),
                evaluation,
            )
            .await;

        assert!(result.is_err());
        assert_eq!(
            server.runtime_generation.current().await.id,
            generation_before.id
        );
        assert_eq!(*server.behavior.lock().await, behavior_before);
        assert_eq!(*server.sdui.lock().await, sdui_before);
        assert_eq!(*server.active_theme.lock().await, theme_before);
        assert_eq!(
            server.runtime_generation.active_typography().await,
            typography_before
        );
        assert_eq!(server.completion.providers(), completion_before);
        assert_eq!(
            server.language_intelligence.providers(),
            intelligence_before
        );
    }

    #[tokio::test]
    async fn candidate_commit_advances_all_server_generation_state_once() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "candidate-commit",
        )));
        let mut evaluation = ClayRuntimeEvaluation::default();
        let mut manifest = BehaviorManifest::minimal_text_editing(99);
        manifest.manifest_id = "clay.test.committed".to_string();
        evaluation.behavior_manifest = Some(manifest);
        evaluation.published_sdui_tree = Some(default_document_tree(1, 1));
        evaluation.active_theme = Some(ActiveTheme {
            specifier: "@clay/theme-test".to_string(),
            overrides: Vec::new(),
        });
        evaluation.active_typography = Some(crate::protocol::ActiveTypography {
            revision: 99,
            monospace: FontProfile {
                families: vec!["monospace".to_string()],
                size: 17.0,
            },
            proportional: FontProfile {
                families: vec!["sans-serif".to_string()],
                size: 18.0,
            },
            ui: FontProfile {
                families: vec!["system-ui".to_string()],
                size: 14.0,
            },
        });
        let candidate = server
            .prepare_runtime_generation_candidate(
                1,
                2,
                super::ClayJsRuntimeService::default(),
                evaluation,
            )
            .await
            .expect("valid candidate");

        server
            .commit_runtime_generation(candidate)
            .await
            .expect("candidate commit");

        let current = server.runtime_generation.current().await;
        assert_eq!(current.id, 2);
        assert_eq!(server.behavior.lock().await.version(), 2);
        assert_eq!(
            server.behavior.lock().await.manifest().manifest_id,
            "clay.test.committed"
        );
        assert!(server.sdui.lock().await.snapshot_message(1).is_some());
        assert_eq!(
            server
                .active_theme
                .lock()
                .await
                .as_ref()
                .map(|theme| theme.specifier.as_str()),
            Some("@clay/theme-test")
        );
        assert_eq!(
            server.runtime_generation.active_typography().await.revision,
            1
        );
        assert!(current.evaluation.is_some());
    }

    #[tokio::test]
    async fn typography_defaults_exist_without_init_configuration() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "typography-defaults",
        )));

        assert_eq!(
            server.runtime_generation.active_typography().await,
            crate::protocol::ActiveTypography::default()
        );
    }

    #[tokio::test]
    async fn typography_update_reaches_connected_clients_once() {
        let root = temp_config_root(
            "typography-live-update",
            r#"import { setTypography } from "clay:theme";
            setTypography({
              monospace: { families: ["monospace"], size: 16 },
              proportional: { families: ["sans-serif"], size: 17 },
              ui: { families: ["system-ui"], size: 13 },
            });"#,
        );
        let server = server_with_config(root.clone());
        let mut updates = server.runtime_generation.subscribe_typography();

        assert!(server.reload_runtime_generation().await.reloaded);
        let update = tokio::time::timeout(std::time::Duration::from_millis(100), updates.recv())
            .await
            .expect("connected client receives typography update")
            .expect("typography channel remains open");
        assert_eq!(update.revision, 1);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), updates.recv())
                .await
                .is_err(),
            "one atomic replacement emits one update"
        );

        fs::write(
            root.join("init.js"),
            r#"import { setTypography } from "clay:theme";
            setTypography({
              monospace: { families: ["monospace"], size: 16 },
              proportional: { families: ["sans-serif"], size: 17 },
            });"#,
        )
        .unwrap();
        assert!(!server.reload_runtime_generation().await.reloaded);
        assert_eq!(
            server.runtime_generation.active_typography().await.revision,
            1
        );
    }

    #[tokio::test]
    async fn reload_runtime_generation_swaps_only_after_successful_configuration_load() {
        let root = temp_config_root(
            "success",
            r#"Deno.core.ops.op_clay_runtime_record("reload ok");"#,
        );
        let server = server_with_config(root);
        let opened_path = temp_config_root("opened-doc", "").join("note.md");
        fs::write(&opened_path, "# kept open\n").unwrap();
        let opened = server
            .workspace
            .lock()
            .await
            .open_selected_file(&opened_path, 77)
            .await
            .unwrap();
        let original_service = server.runtime_generation.current_service().await;
        original_service
            .evaluate_controlled_module("globalThis.__reloadMarker = 41;")
            .await
            .unwrap();

        let outcome = server.reload_runtime_generation().await;

        assert!(outcome.reloaded);
        assert_eq!(outcome.previous_generation_id, 1);
        assert_eq!(outcome.active_generation_id, 2);
        let documents = server
            .workspace
            .lock()
            .await
            .list_documents(77)
            .await
            .unwrap();
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].document_id, opened.document_id);
        assert_eq!(documents[0].lease_id, opened.access.lease_id());
        let current_service = server.runtime_generation.current_service().await;
        let evaluation = current_service
            .evaluate_controlled_module(
                r#"Deno.core.ops.op_clay_runtime_record(String(globalThis.__reloadMarker ?? "empty"));"#,
            )
            .await
            .unwrap();
        assert_eq!(evaluation.op_records.last().unwrap(), "empty");
    }

    #[tokio::test]
    async fn successful_reload_refreshes_open_documents_without_full_snapshots() {
        let root = temp_config_root(
            "open-refresh",
            r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");"#,
        );
        let server = server_with_config(root);
        let file_root = temp_config_root("open-refresh-docs", "");
        let markdown_path = file_root.join("note.md");
        let text_path = file_root.join("plain.txt");
        fs::write(&markdown_path, "# Reloaded\n").unwrap();
        fs::write(&text_path, "plain text\n").unwrap();
        let markdown = server
            .workspace
            .lock()
            .await
            .open_selected_file(&markdown_path, 77)
            .await
            .unwrap();
        let text = server
            .workspace
            .lock()
            .await
            .open_selected_file(&text_path, 77)
            .await
            .unwrap();

        let outcome = server.reload_runtime_generation().await;

        assert!(outcome.reloaded);
        assert_eq!(outcome.refreshed_documents.len(), 2);
        let markdown_refresh = outcome
            .refreshed_documents
            .iter()
            .find(|refresh| refresh.document_id == markdown.document_id)
            .unwrap();
        assert!(markdown_refresh.messages.iter().any(|message| matches!(
            message,
            ServerMessage::BehaviorManifest(manifest)
                if manifest.manifest_id == "markdown.markdown"
                    && matches!(manifest.scope, crate::protocol::BehaviorScope::Document { document_id } if document_id == markdown.document_id)
        )));
        assert!(
            markdown_refresh
                .messages
                .iter()
                .all(|message| !matches!(message, ServerMessage::DecorationSet(_))),
            "reload refresh should not block on background parse decorations"
        );
        assert!(
            outcome
                .refreshed_documents
                .iter()
                .all(|refresh| refresh.messages.iter().all(|message| !matches!(
                    message,
                    ServerMessage::DocumentOpened { .. } | ServerMessage::DocumentReloaded { .. }
                )))
        );
        let text_refresh = outcome
            .refreshed_documents
            .iter()
            .find(|refresh| refresh.document_id == text.document_id)
            .unwrap();
        assert!(
            text_refresh
                .messages
                .iter()
                .all(|message| !matches!(message, ServerMessage::DecorationSet(_)))
        );
    }

    #[tokio::test]
    async fn reload_reruns_init_js_package_load_in_fresh_generation_and_preserves_old_on_failure() {
        let root = temp_config_root(
            "package-cache",
            r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
await loadPackage("@clay/markdown");"#,
        );
        let server = server_with_config(root.clone());

        let loaded = server.reload_runtime_generation().await;
        assert!(loaded.reloaded);
        assert_eq!(loaded.active_generation_id, 2);
        let current_service = server.runtime_generation.current_service().await;
        let cached = current_service
            .evaluate_controlled_module(
                r#"import { loadPackage } from "clay:packages";
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"])));
await loadPackage("@clay/markdown");
Deno.core.ops.op_clay_runtime_record("cached");"#,
            )
            .await
            .unwrap();
        assert!(cached.op_records.iter().any(|record| record == "true"));
        assert_eq!(cached.op_records.last().unwrap(), "cached");

        fs::write(
            root.join("init.js"),
            r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/not-installed");"#,
        )
        .unwrap();
        let failed = server.reload_runtime_generation().await;
        assert!(!failed.reloaded);
        assert_eq!(failed.active_generation_id, 2);
        let still_cached = server
            .runtime_generation
            .current_service()
            .await
            .evaluate_controlled_module(
                r#"import { loadPackage } from "clay:packages";
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"])));
await loadPackage("@clay/markdown");
Deno.core.ops.op_clay_runtime_record("still-cached");"#,
            )
            .await
            .unwrap();
        assert!(
            still_cached
                .op_records
                .iter()
                .any(|record| record == "true")
        );
        assert_eq!(still_cached.op_records.last().unwrap(), "still-cached");
        assert!(
            server
                .runtime_diagnostics
                .lock()
                .await
                .iter()
                .any(|diagnostic| diagnostic.code == "clay.packages.not_installed")
        );
    }

    #[tokio::test]
    async fn reload_reruns_one_line_loads_and_rebuilds_representative_contributions() {
        let root = temp_config_root(
            "one-line-rebuild",
            r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");"#,
        );
        let server = server_with_config(root);

        let outcome = server.reload_runtime_generation().await;
        assert!(outcome.reloaded);
        assert_eq!(outcome.active_generation_id, 2);

        let evaluation = server
            .runtime_generation
            .current()
            .await
            .evaluation
            .expect("committed generation must retain evaluation snapshot");

        assert!(
            evaluation
                .js_parse_handlers
                .iter()
                .any(|handler| handler.package.manifest.name == "@clay/markdown"),
            "markdown parse handler must rebuild in G2"
        );
        assert!(
            evaluation.behavior_manifest.is_some(),
            "language/command behavior contributions must rebuild in G2"
        );
        for language in ["rust", "typescript", "javascript", "markdown"] {
            assert!(
                evaluation
                    .syntax_grammars
                    .iter()
                    .any(|grammar| grammar.language_id == language),
                "{language} syntax grammar must rebuild in G2"
            );
        }
        for provider_id in [
            "markdown.keywords",
            "rust.keywords",
            "typescript.keywords",
            "javascript.keywords",
        ] {
            assert!(
                evaluation
                    .completion_providers
                    .iter()
                    .any(|provider| provider.id == provider_id),
                "{provider_id} completion metadata must rebuild in G2"
            );
        }
        assert!(
            !evaluation.ui_contributions.components.is_empty()
                || !evaluation.ui_contributions.panels.is_empty(),
            "package UI contributions must rebuild in G2"
        );

        let cached = server
            .runtime_generation
            .current_service()
            .await
            .evaluate_controlled_module(
                r#"import { loadPackage } from "clay:packages";
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"])));
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/rust"])));
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
Deno.core.ops.op_clay_runtime_record("idempotent");"#,
            )
            .await
            .unwrap();
        assert!(cached.op_records.iter().any(|record| record == "true"));
        assert_eq!(cached.op_records.last().unwrap(), "idempotent");
    }

    #[tokio::test]
    async fn runtime_timeout_drops_candidate_service_and_keeps_old_generation() {
        let root = temp_config_root("candidate-timeout", "while (true) {}");
        let server = server_with_config(root);
        let original = server.runtime_generation.current().await;
        let candidate_service = super::ClayJsRuntimeService::with_timeout_and_heap_limit(
            Duration::from_millis(10),
            crate::perf::budgets::JS_RUNTIME_HEAP_LIMIT_BYTES,
        );

        let error = server
            .load_configuration_for_service(&candidate_service)
            .await
            .expect_err("candidate evaluation must time out");

        assert!(matches!(
            error,
            super::js_runtime::ClayRuntimeError::Timeout
        ));
        assert_eq!(server.runtime_generation.current().await.id, original.id);
        assert!(
            server
                .runtime_generation
                .current()
                .await
                .evaluation
                .is_none()
        );
    }

    #[tokio::test]
    async fn failed_reload_keeps_previous_runtime_generation_active() {
        let root = temp_config_root("failure", "export const = ;");
        let server = server_with_config(root);
        let original_service = server.runtime_generation.current_service().await;
        original_service
            .evaluate_controlled_module("globalThis.__reloadMarker = 7;")
            .await
            .unwrap();

        let outcome = server.reload_runtime_generation().await;

        assert!(!outcome.reloaded);
        assert_eq!(outcome.previous_generation_id, 1);
        assert_eq!(outcome.active_generation_id, 1);
        let current_service = server.runtime_generation.current_service().await;
        let evaluation = current_service
            .evaluate_controlled_module(
                r#"Deno.core.ops.op_clay_runtime_record(String(globalThis.__reloadMarker));"#,
            )
            .await
            .unwrap();
        assert_eq!(evaluation.op_records.last().unwrap(), "7");
        assert!(
            server
                .runtime_diagnostics
                .lock()
                .await
                .iter()
                .any(|diagnostic| diagnostic.code == "clay.runtime.syntax_error")
        );
    }

    fn seed_package() -> crate::packages::record::PackageRecord {
        assemble_package_record(&serde_json::json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "markdown",
                "entry": "./dist/index.js",
                "permissions": ["parse-document", "completion-provider"],
                "modes": ["markdown"],
                "docs": "./docs/index.md"
            }
        }))
        .expect("seed package validates")
    }

    fn seed_old_generation_contributions(server: &IpcServer) {
        let package = seed_package();
        server
            .parse_coordinator
            .register_handler_for_generation(
                &package,
                1,
                "markdown",
                |_notification: ParseEditNotification| async move {
                    Ok(IncrementalParseUpdate {
                        document_id: 7,
                        document_version: 1,
                        behavior_version: 1,
                        package_prefix: "markdown".to_string(),
                        mode_id: "markdown".to_string(),
                        parse_unit: crate::protocol::ParseUnit::File,
                        viewport: ParseByteRange::new(0, 1),
                        invalidated_ranges: Vec::new(),
                        syntax_tree_delta: None,
                        decoration_update: None,
                        diagnostic_update: None,
                    })
                },
            )
            .expect("seed parse handler");
        server
            .completion
            .register_builtin_buffer_words(1)
            .expect("seed completion");
        server
            .language_intelligence
            .register_builtin(
                LanguageIntelligenceProviderMeta::builtin_core(
                    "hover",
                    vec![crate::protocol::LanguageIntelligenceFeature::Hover],
                    1,
                    500,
                    1,
                ),
                |_request, _window| async move {
                    Err(
                        crate::server::language_intelligence::LanguageIntelligenceProviderError::ProviderFailed(
                            "seed".to_string(),
                        ),
                    )
                },
            )
            .expect("seed language intelligence");
    }

    #[tokio::test]
    async fn successful_reload_replaces_all_provider_registries_and_cancels_old_work() {
        let root = temp_config_root(
            "replace-all",
            r#"Deno.core.ops.op_clay_runtime_record("generation-two");"#,
        );
        let server = server_with_config(root);
        seed_old_generation_contributions(&server);
        assert_eq!(server.parse_coordinator.registered_generations(), vec![1]);
        assert_eq!(server.completion.registered_generations(), vec![1]);
        assert_eq!(
            server.language_intelligence.registered_generations(),
            vec![1]
        );

        let outcome = server.reload_runtime_generation().await;

        assert!(outcome.reloaded);
        assert_eq!(outcome.active_generation_id, 2);
        assert!(
            server
                .parse_coordinator
                .registered_generations()
                .iter()
                .all(|&generation| generation >= 2)
        );
        assert!(
            server
                .completion
                .registered_generations()
                .iter()
                .all(|&generation| generation >= 2)
        );
        assert!(
            server
                .language_intelligence
                .registered_generations()
                .iter()
                .all(|&generation| generation >= 2)
        );
        assert!(
            server.document_analysis.registered_generations().is_empty()
                || server
                    .document_analysis
                    .registered_generations()
                    .iter()
                    .all(|&generation| generation >= 2)
        );
        assert!(
            server.document_analysis.worker_generations().is_empty()
                || server
                    .document_analysis
                    .worker_generations()
                    .iter()
                    .all(|&generation| generation >= 2)
        );
    }

    #[tokio::test]
    async fn failed_reload_keeps_workers_sessions_and_outputs_on_previous_generation() {
        let root = temp_config_root("keep-old", "export const = ;");
        let server = server_with_config(root);
        seed_old_generation_contributions(&server);
        let sessions_before = server
            .runtime_generation
            .current_service()
            .await
            .language_server_session_count()
            .await;

        let outcome = server.reload_runtime_generation().await;

        assert!(!outcome.reloaded);
        assert_eq!(outcome.active_generation_id, 1);
        assert_eq!(server.parse_coordinator.registered_generations(), vec![1]);
        assert_eq!(server.completion.registered_generations(), vec![1]);
        assert_eq!(
            server.language_intelligence.registered_generations(),
            vec![1]
        );
        assert_eq!(
            server
                .runtime_generation
                .current_service()
                .await
                .language_server_session_count()
                .await,
            sessions_before
        );
    }

    #[tokio::test]
    async fn late_old_generation_parse_completion_diagnostic_and_intelligence_output_is_dropped() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "stale-output-drop",
        )));
        let package = seed_package();
        server
            .parse_coordinator
            .register_handler_for_generation(
                &package,
                1,
                "markdown",
                |notification: ParseEditNotification| async move {
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    Ok(IncrementalParseUpdate {
                        document_id: notification.document_id,
                        document_version: notification.document_version,
                        behavior_version: notification.behavior_version,
                        package_prefix: notification.package_prefix,
                        mode_id: notification.mode_id,
                        parse_unit: crate::protocol::ParseUnit::File,
                        viewport: notification.viewport,
                        invalidated_ranges: notification.invalidated_ranges,
                        syntax_tree_delta: None,
                        decoration_update: None,
                        diagnostic_update: None,
                    })
                },
            )
            .unwrap();
        server
            .completion
            .register_builtin(
                BufferWordCompletionProvider::meta(1),
                BufferWordCompletionProvider,
            )
            .unwrap();

        server
            .parse_coordinator
            .schedule_parse(ParseScheduleRequest {
                document_id: 7,
                document_version: 1,
                behavior_version: 1,
                package_prefix: "markdown".to_string(),
                mode_id: "markdown".to_string(),
                viewport: ParseByteRange::new(0, 8),
                invalidated_ranges: vec![ParseByteRange::new(0, 8)],
            })
            .unwrap();
        server.parse_coordinator.cancel_older_generations(2);
        server.completion.cancel_older_generations(2);
        server.language_intelligence.cancel_older_generations(2);
        server.document_analysis.cancel_older_generations(2);

        assert!(
            tokio::time::timeout(
                Duration::from_millis(300),
                server.parse_coordinator.next_update()
            )
            .await
            .is_err(),
            "late old-generation parse output must be drained/dropped"
        );
        assert!(server.parse_coordinator.registered_generations().is_empty());
        assert!(server.completion.registered_generations().is_empty());
        assert_eq!(server.completion.stats().stale_results_rejected, 0);
    }

    #[tokio::test]
    async fn removed_language_package_reclassifies_to_core_fallback() {
        use crate::packages::modes::{DocumentClassificationInput, ModeDeclaration, ModeRegistry};

        let package = assemble_package_record(&serde_json::json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "markdown",
                "entry": "./dist/index.js",
                "permissions": ["mode-registration", "mode-activation"],
                "modes": ["markdown"],
                "docs": "./docs/index.md"
            }
        }))
        .expect("mode package validates");
        let mut registry = ModeRegistry::new();
        registry
            .register_mode(
                &package.manifest,
                ModeDeclaration {
                    package_name: package.manifest.name.clone(),
                    package_version: package.manifest.version.clone(),
                    api_prefix: package.manifest.clay.api_prefix.clone(),
                    mode_id: "markdown".to_string(),
                    display_name: "Markdown".to_string(),
                    document_font_role: crate::protocol::DocumentFontRole::Proportional,
                    extensions: vec!["md".to_string()],
                    mime_types: Vec::new(),
                    file_names: Vec::new(),
                    file_name_patterns: Vec::new(),
                    shebang_patterns: Vec::new(),
                    content_probes: Vec::new(),
                },
            )
            .expect("register markdown mode");
        let classified = registry
            .classify(&DocumentClassificationInput {
                document_id: 7,
                path: Some("note.md".to_string()),
                mime_type: None,
                shebang: None,
                leading_content: None,
            })
            .expect("markdown claims .md");
        assert_eq!(classified.mode_id, "markdown");
        assert_eq!(registry.unregister_package_modes("markdown"), 1);

        let fallback = registry
            .classify(&DocumentClassificationInput {
                document_id: 7,
                path: Some("note.md".to_string()),
                mime_type: None,
                shebang: None,
                leading_content: None,
            })
            .expect("core fallback remains after package withdrawal");
        assert_eq!(fallback.mode_id, "core.text");
        assert_eq!(fallback.api_prefix, "core");
    }

    #[tokio::test]
    async fn withdraw_package_contributions_reuses_generation_cancel_primitives() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "withdraw-package",
        )));
        seed_old_generation_contributions(&server);
        withdraw_package_contributions(
            "@clay/markdown",
            "markdown",
            &server.parse_coordinator,
            &server.completion,
            &server.document_analysis,
            &server.language_intelligence,
        );
        assert!(server.parse_coordinator.registered_generations().is_empty());
        // Built-in buffer-word completion uses clay.core provenance, so package
        // withdraw leaves it; language-intelligence seed uses clay.core too.
        assert!(
            server
                .completion
                .providers()
                .iter()
                .all(|meta| meta.provenance.package_prefix != "markdown")
        );

        let mut commands = crate::packages::commands::CommandRegistry::new();
        commands.insert_test_command(crate::packages::commands::RegisteredCommand {
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            command_id: "markdown.togglePreview".to_string(),
            display_name: "Toggle Preview".to_string(),
            routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
            key_bindings: Vec::new(),
            custom_properties: Default::default(),
            permissions: Vec::new(),
        });
        assert_eq!(commands.remove_package_commands("@clay/markdown"), 1);
        assert!(commands.list().next().is_none());
    }

    fn sample_runtime_snapshot(generation: u64) -> crate::protocol::RuntimeStateSnapshot {
        let snapshot = crate::protocol::RuntimeStateSnapshot {
            runtime_generation_id: generation,
            client_id: 0,
            behavior: BehaviorManifest::minimal_text_editing(generation),
            active_theme: ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
            },
            active_typography: crate::protocol::ActiveTypography::default(),
            sdui_tree: default_document_tree(1, 1),
            package_ui: crate::protocol::PackageUiSnapshot {
                version: generation,
            },
            documents: Vec::new(),
            diagnostics: Vec::new(),
        };
        snapshot.validate().expect("sample snapshot");
        snapshot
    }

    #[tokio::test]
    async fn successful_reload_publishes_runtime_state_snapshot_to_subscribers() {
        let root = temp_config_root(
            "snapshot-fanout",
            r#"Deno.core.ops.op_clay_runtime_record("fanout");"#,
        );
        let server = server_with_config(root.clone());
        let mut updates = server.runtime_generation.subscribe_runtime_state();

        let outcome = server.reload_runtime_generation().await;
        assert!(outcome.reloaded);
        assert_eq!(outcome.active_generation_id, 2);

        let generation = tokio::time::timeout(Duration::from_millis(200), updates.recv())
            .await
            .expect("subscriber receives generation notice")
            .expect("runtime-state channel remains open");
        assert_eq!(generation, 2);
        let snapshot = server
            .runtime_generation
            .latest_runtime_snapshot_for(42)
            .await
            .expect("latest snapshot retained after commit");
        assert_eq!(snapshot.runtime_generation_id, 2);
        assert_eq!(snapshot.client_id, 42);
        assert!(snapshot.package_ui.version >= 2);
        snapshot.validate().expect("published snapshot validates");

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn lagged_connection_receives_latest_snapshot_not_intermediate_generations() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "lagged-runtime-snapshot",
        )));
        let mut updates = server.runtime_generation.subscribe_runtime_state();

        for generation in 1..=20 {
            server
                .runtime_generation
                .publish_runtime_snapshot(sample_runtime_snapshot(generation))
                .await;
        }

        match updates.recv().await {
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Ok(generation) => {
                // Capacity may still deliver the newest notice without lagging
                // when the runtime coalesces; either way recovery uses latest.
                assert_eq!(generation, 20);
            }
            Err(other) => panic!("unexpected broadcast error: {other:?}"),
        }

        let latest = server
            .runtime_generation
            .latest_runtime_snapshot_for(9)
            .await
            .expect("latest complete snapshot");
        assert_eq!(latest.runtime_generation_id, 20);
        assert_eq!(latest.client_id, 9);
        assert_ne!(latest.runtime_generation_id, 19);
    }

    #[tokio::test]
    async fn spoofed_or_future_install_ack_is_ignored() {
        let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
            "spoofed-runtime-ack",
        )));
        server
            .runtime_generation
            .publish_runtime_snapshot(sample_runtime_snapshot(3))
            .await;

        assert!(
            !server
                .runtime_generation
                .note_runtime_generation_installed(99, 7, 3)
                .await,
            "spoofed client id must be ignored"
        );
        assert!(
            !server
                .runtime_generation
                .note_runtime_generation_installed(7, 7, 4)
                .await,
            "future generation must be ignored"
        );
        assert!(
            !server
                .runtime_generation
                .note_runtime_generation_installed(7, 7, 0)
                .await,
            "zero generation must be ignored"
        );
        assert!(
            server
                .runtime_generation
                .note_runtime_generation_installed(7, 7, 3)
                .await
        );
        assert_eq!(
            server
                .runtime_generation
                .acknowledged_runtime_generation(7)
                .await,
            Some(3)
        );
    }

    #[tokio::test]
    async fn successful_reload_reaches_two_connected_clients() {
        use std::sync::Arc;
        use tokio::io::duplex;

        let root = temp_config_root(
            "two-clients",
            r#"Deno.core.ops.op_clay_runtime_record("two clients");"#,
        );
        let server = server_with_config(root.clone());
        let codec = crate::protocol::codec::Codec::default();

        async fn bootstrap_client(
            server: &IpcServer,
            client_id: u64,
            codec: crate::protocol::codec::Codec,
        ) -> (
            tokio::io::DuplexStream,
            tokio::task::JoinHandle<Result<(), crate::protocol::codec::CodecError>>,
        ) {
            let (client, server_stream) = duplex(64 * 1024);
            let connection_server = server.clone();
            let handle = tokio::spawn(async move {
                crate::server::connection::handle_connection_with_analysis(
                    server_stream,
                    client_id,
                    Arc::clone(&connection_server.document),
                    Arc::clone(&connection_server.behavior),
                    Arc::clone(&connection_server.workspace),
                    Arc::clone(&connection_server.sdui),
                    Arc::clone(&connection_server.active_theme),
                    Arc::clone(&connection_server.runtime_diagnostics),
                    connection_server.runtime_generation.clone(),
                    connection_server.parse_coordinator.clone(),
                    connection_server.completion.clone(),
                    connection_server.document_analysis.clone(),
                    connection_server.language_intelligence.clone(),
                    Some(connection_server),
                    codec,
                )
                .await
            });
            let mut client = client;
            codec
                .write_client_message(
                    &mut client,
                    &crate::protocol::ClientMessage::Hello {
                        protocol_version: crate::protocol::PROTOCOL_VERSION,
                        client_name: format!("client-{client_id}"),
                    },
                )
                .await
                .unwrap();
            loop {
                match codec.read_server_message(&mut client).await.unwrap() {
                    ServerMessage::Welcome {
                        client_id: welcome_id,
                        ..
                    } => {
                        assert_eq!(welcome_id, client_id);
                        break;
                    }
                    ServerMessage::Error { code, message } => {
                        panic!("bootstrap failed: {code:?} {message}");
                    }
                    _ => {}
                }
            }
            // Drain remaining bootstrap messages so reload fan-out is next.
            for _ in 0..16 {
                match tokio::time::timeout(
                    Duration::from_millis(10),
                    codec.read_server_message(&mut client),
                )
                .await
                {
                    Ok(Ok(_)) => {}
                    Ok(Err(_)) | Err(_) => break,
                }
            }
            (client, handle)
        }

        let (mut client_a, task_a) = bootstrap_client(&server, 21, codec).await;
        let (mut client_b, task_b) = bootstrap_client(&server, 22, codec).await;

        assert!(server.reload_runtime_generation().await.reloaded);

        async fn read_snapshot(
            codec: &crate::protocol::codec::Codec,
            client: &mut tokio::io::DuplexStream,
            expected_client_id: u64,
        ) -> crate::protocol::RuntimeStateSnapshot {
            loop {
                match tokio::time::timeout(
                    Duration::from_millis(500),
                    codec.read_server_message(client),
                )
                .await
                .expect("client receives fan-out")
                .unwrap()
                {
                    ServerMessage::RuntimeStateSnapshot(snapshot) => {
                        assert_eq!(snapshot.client_id, expected_client_id);
                        assert_eq!(snapshot.runtime_generation_id, 2);
                        return *snapshot;
                    }
                    ServerMessage::ActiveTypography(_)
                    | ServerMessage::BehaviorManifest(_)
                    | ServerMessage::DecorationSet(_)
                    | ServerMessage::DiagnosticSet(_)
                    | ServerMessage::RuntimeDiagnostic(_)
                    | ServerMessage::SduiSnapshot { .. }
                    | ServerMessage::SduiUpdate { .. } => {}
                    other => panic!("unexpected fan-out message: {other:?}"),
                }
            }
        }

        let snapshot_a = read_snapshot(&codec, &mut client_a, 21).await;
        let snapshot_b = read_snapshot(&codec, &mut client_b, 22).await;
        assert_eq!(
            snapshot_a.runtime_generation_id,
            snapshot_b.runtime_generation_id
        );

        codec
            .write_client_message(
                &mut client_a,
                &crate::protocol::ClientMessage::RuntimeGenerationInstalled {
                    client_id: 21,
                    runtime_generation_id: 2,
                },
            )
            .await
            .unwrap();
        codec
            .write_client_message(
                &mut client_b,
                &crate::protocol::ClientMessage::RuntimeGenerationInstalled {
                    client_id: 22,
                    runtime_generation_id: 2,
                },
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            server
                .runtime_generation
                .acknowledged_runtime_generation(21)
                .await,
            Some(2)
        );
        assert_eq!(
            server
                .runtime_generation
                .acknowledged_runtime_generation(22)
                .await,
            Some(2)
        );

        drop(client_a);
        drop(client_b);
        let _ = task_a.await;
        let _ = task_b.await;
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn edit_sent_before_snapshot_install_is_accepted_once_under_previous_generation() {
        use std::sync::Arc;
        use tokio::io::duplex;

        let root = temp_config_root(
            "grace-accept",
            r#"
            import { bindKey } from "clay:keybindings";
            bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
            Deno.core.ops.op_clay_runtime_record("grace accept");
            "#,
        );
        let server = server_with_config(root.clone());
        let previous_behavior = server.behavior.lock().await.manifest().clone();
        assert_eq!(previous_behavior.behavior_version, 1);

        let outcome = server.reload_runtime_generation().await;
        assert!(outcome.reloaded);
        assert_eq!(server.runtime_generation.generation_id().await, 2);
        assert_eq!(server.behavior.lock().await.version(), 2);

        let (client, server_stream) = duplex(64 * 1024);
        let codec = crate::protocol::codec::Codec::default();
        let connection_server = server.clone();
        let server_task = tokio::spawn(async move {
            crate::server::connection::handle_connection_with_analysis(
                server_stream,
                31,
                Arc::clone(&connection_server.document),
                Arc::clone(&connection_server.behavior),
                Arc::clone(&connection_server.workspace),
                Arc::clone(&connection_server.sdui),
                Arc::clone(&connection_server.active_theme),
                Arc::clone(&connection_server.runtime_diagnostics),
                connection_server.runtime_generation.clone(),
                connection_server.parse_coordinator.clone(),
                connection_server.completion.clone(),
                connection_server.document_analysis.clone(),
                connection_server.language_intelligence.clone(),
                Some(connection_server),
                codec,
            )
            .await
        });
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Hello {
                    protocol_version: crate::protocol::PROTOCOL_VERSION,
                    client_name: "grace-client".to_string(),
                },
            )
            .await
            .unwrap();
        for _ in 0..7 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }

        let document = server.document.lock().await;
        let document_id = document.document_id();
        let version = document.version();
        let lease_id = match document.access_for_client(31) {
            crate::protocol::DocumentAccess::Editable { lease_id } => Some(lease_id),
            _ => None,
        };
        drop(document);

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Edit {
                    document_id,
                    client_id: 31,
                    lease_id,
                    base_version: version,
                    behavior_version: previous_behavior.behavior_version,
                    transaction_id: 501,
                    operation: crate::protocol::EditOperation::Insert {
                        byte_offset: 0,
                        text: "g1".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        match codec.read_server_message(&mut client).await.unwrap() {
            crate::protocol::ServerMessage::EditAck {
                document_id: ack_document_id,
                transaction_id,
                confirmed_version,
            } => {
                assert_eq!(ack_document_id, document_id);
                assert_eq!(transaction_id, 501);
                assert_eq!(confirmed_version, version + 1);
            }
            other => panic!("expected EditAck under grace, got {other:?}"),
        }

        drop(client);
        let _ = server_task.await;
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn previous_generation_edit_after_ack_or_expiry_is_rejected_and_snapshot_resent() {
        use std::sync::Arc;
        use tokio::io::duplex;

        let root = temp_config_root(
            "grace-reject",
            r#"
            import { bindKey } from "clay:keybindings";
            bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
            Deno.core.ops.op_clay_runtime_record("grace reject");
            "#,
        );
        let server = server_with_config(root.clone());
        let previous_version = server.behavior.lock().await.version();
        assert!(server.reload_runtime_generation().await.reloaded);

        let (client, server_stream) = duplex(64 * 1024);
        let codec = crate::protocol::codec::Codec::default();
        let connection_server = server.clone();
        let server_task = tokio::spawn(async move {
            crate::server::connection::handle_connection_with_analysis(
                server_stream,
                32,
                Arc::clone(&connection_server.document),
                Arc::clone(&connection_server.behavior),
                Arc::clone(&connection_server.workspace),
                Arc::clone(&connection_server.sdui),
                Arc::clone(&connection_server.active_theme),
                Arc::clone(&connection_server.runtime_diagnostics),
                connection_server.runtime_generation.clone(),
                connection_server.parse_coordinator.clone(),
                connection_server.completion.clone(),
                connection_server.document_analysis.clone(),
                connection_server.language_intelligence.clone(),
                Some(connection_server),
                codec,
            )
            .await
        });
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Hello {
                    protocol_version: crate::protocol::PROTOCOL_VERSION,
                    client_name: "grace-reject-client".to_string(),
                },
            )
            .await
            .unwrap();
        for _ in 0..7 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::RuntimeGenerationInstalled {
                    client_id: 32,
                    runtime_generation_id: 2,
                },
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;

        let document = server.document.lock().await;
        let document_id = document.document_id();
        let version = document.version();
        let lease_id = match document.access_for_client(32) {
            crate::protocol::DocumentAccess::Editable { lease_id } => Some(lease_id),
            _ => None,
        };
        let text_before = document.text();
        drop(document);

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Edit {
                    document_id,
                    client_id: 32,
                    lease_id,
                    base_version: version,
                    behavior_version: previous_version,
                    transaction_id: 777,
                    operation: crate::protocol::EditOperation::Insert {
                        byte_offset: 0,
                        text: "stale".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        match codec.read_server_message(&mut client).await.unwrap() {
            crate::protocol::ServerMessage::EditRejected {
                reason:
                    crate::protocol::EditRejection::InvalidBehaviorVersion {
                        behavior_version,
                        server_behavior_version,
                    },
                ..
            } => {
                assert_eq!(behavior_version, previous_version);
                assert_eq!(server_behavior_version, 2);
            }
            other => panic!("expected InvalidBehaviorVersion, got {other:?}"),
        }
        match codec.read_server_message(&mut client).await.unwrap() {
            crate::protocol::ServerMessage::RuntimeStateSnapshot(snapshot) => {
                assert_eq!(snapshot.runtime_generation_id, 2);
                assert_eq!(snapshot.client_id, 32);
            }
            other => panic!("expected RuntimeStateSnapshot republish, got {other:?}"),
        }
        assert_eq!(server.document.lock().await.text(), text_before);

        server
            .runtime_generation
            .behavior_grace()
            .expire_for_test()
            .await;
        assert!(
            server
                .runtime_generation
                .behavior_grace()
                .validate_edit_version(
                    &*server.behavior.lock().await,
                    99,
                    document_id,
                    1,
                    previous_version,
                    2,
                    None,
                    std::time::Instant::now(),
                )
                .await
                .is_err()
        );

        drop(client);
        let _ = server_task.await;
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn grace_never_bypasses_lease_validation() {
        use std::sync::Arc;
        use tokio::io::duplex;

        let root = temp_config_root(
            "grace-lease",
            r#"
            import { bindKey } from "clay:keybindings";
            bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
            Deno.core.ops.op_clay_runtime_record("grace lease");
            "#,
        );
        let server = server_with_config(root.clone());
        let previous_version = server.behavior.lock().await.version();
        assert!(server.reload_runtime_generation().await.reloaded);

        let (client, server_stream) = duplex(64 * 1024);
        let codec = crate::protocol::codec::Codec::default();
        let connection_server = server.clone();
        let server_task = tokio::spawn(async move {
            crate::server::connection::handle_connection_with_analysis(
                server_stream,
                33,
                Arc::clone(&connection_server.document),
                Arc::clone(&connection_server.behavior),
                Arc::clone(&connection_server.workspace),
                Arc::clone(&connection_server.sdui),
                Arc::clone(&connection_server.active_theme),
                Arc::clone(&connection_server.runtime_diagnostics),
                connection_server.runtime_generation.clone(),
                connection_server.parse_coordinator.clone(),
                connection_server.completion.clone(),
                connection_server.document_analysis.clone(),
                connection_server.language_intelligence.clone(),
                Some(connection_server),
                codec,
            )
            .await
        });
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Hello {
                    protocol_version: crate::protocol::PROTOCOL_VERSION,
                    client_name: "grace-lease-client".to_string(),
                },
            )
            .await
            .unwrap();
        for _ in 0..7 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }

        let document = server.document.lock().await;
        let document_id = document.document_id();
        let version = document.version();
        let text_before = document.text();
        drop(document);

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Edit {
                    document_id,
                    client_id: 33,
                    lease_id: Some(u64::MAX),
                    base_version: version,
                    behavior_version: previous_version,
                    transaction_id: 808,
                    operation: crate::protocol::EditOperation::Insert {
                        byte_offset: 0,
                        text: "nope".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        match codec.read_server_message(&mut client).await.unwrap() {
            crate::protocol::ServerMessage::EditRejected {
                reason:
                    crate::protocol::EditRejection::LeaseExpired { .. }
                    | crate::protocol::EditRejection::LeaseRequired,
                ..
            } => {}
            other => panic!("grace must not bypass lease checks, got {other:?}"),
        }
        assert_eq!(server.document.lock().await.text(), text_before);

        drop(client);
        let _ = server_task.await;
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn typing_and_edit_ack_continue_while_candidate_runtime_is_blocked_on_test_barrier() {
        use std::sync::Arc;
        use tokio::io::duplex;

        let root = temp_config_root(
            "barrier-typing",
            r#"Deno.core.ops.op_clay_runtime_record("barrier ok");"#,
        );
        let server = server_with_config(root.clone());
        let (entered_rx, release_tx) = server.arm_reload_candidate_barrier().await;

        let (client, server_stream) = duplex(64 * 1024);
        let codec = crate::protocol::codec::Codec::default();
        let connection_server = server.clone();
        let server_task = tokio::spawn(async move {
            crate::server::connection::handle_connection_with_analysis(
                server_stream,
                41,
                Arc::clone(&connection_server.document),
                Arc::clone(&connection_server.behavior),
                Arc::clone(&connection_server.workspace),
                Arc::clone(&connection_server.sdui),
                Arc::clone(&connection_server.active_theme),
                Arc::clone(&connection_server.runtime_diagnostics),
                connection_server.runtime_generation.clone(),
                connection_server.parse_coordinator.clone(),
                connection_server.completion.clone(),
                connection_server.document_analysis.clone(),
                connection_server.language_intelligence.clone(),
                Some(connection_server),
                codec,
            )
            .await
        });
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Hello {
                    protocol_version: crate::protocol::PROTOCOL_VERSION,
                    client_name: "barrier-client".to_string(),
                },
            )
            .await
            .unwrap();
        for _ in 0..7 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }

        let reload_server = server.clone();
        let reload_task =
            tokio::spawn(async move { reload_server.reload_runtime_generation().await });
        entered_rx
            .await
            .expect("reload candidate must reach the test barrier");
        assert_eq!(
            server.runtime_generation.generation_id().await,
            1,
            "generation must stay on G1 while candidate is blocked"
        );

        let document = server.document.lock().await;
        let document_id = document.document_id();
        let version = document.version();
        let lease_id = match document.access_for_client(41) {
            crate::protocol::DocumentAccess::Editable { lease_id } => Some(lease_id),
            _ => None,
        };
        let behavior_version = server.behavior.lock().await.version();
        drop(document);

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Edit {
                    document_id,
                    client_id: 41,
                    lease_id,
                    base_version: version,
                    behavior_version,
                    transaction_id: 7001,
                    operation: crate::protocol::EditOperation::Insert {
                        byte_offset: 0,
                        text: "typed".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        match tokio::time::timeout(
            Duration::from_millis(250),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("edit ack must not wait for blocked reload")
        .unwrap()
        {
            crate::protocol::ServerMessage::EditAck {
                document_id: ack_document_id,
                transaction_id,
                confirmed_version,
            } => {
                assert_eq!(ack_document_id, document_id);
                assert_eq!(transaction_id, 7001);
                assert_eq!(confirmed_version, version + 1);
            }
            other => panic!("expected EditAck while reload is blocked, got {other:?}"),
        }

        release_tx.send(()).expect("release blocked reload");
        let outcome = reload_task.await.expect("reload task joins");
        assert!(outcome.reloaded);
        assert_eq!(outcome.active_generation_id, 2);

        drop(client);
        let _ = server_task.await;
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn failed_reload_broadcasts_diagnostic_but_no_generation_snapshot() {
        use std::sync::Arc;
        use tokio::io::duplex;

        let root = temp_config_root(
            "failed-no-snapshot",
            r#"Deno.core.ops.op_clay_runtime_record("baseline");"#,
        );
        let server = server_with_config(root.clone());
        assert_eq!(server.runtime_generation.generation_id().await, 1);

        let (client, server_stream) = duplex(64 * 1024);
        let codec = crate::protocol::codec::Codec::default();
        let connection_server = server.clone();
        let server_task = tokio::spawn(async move {
            crate::server::connection::handle_connection_with_analysis(
                server_stream,
                42,
                Arc::clone(&connection_server.document),
                Arc::clone(&connection_server.behavior),
                Arc::clone(&connection_server.workspace),
                Arc::clone(&connection_server.sdui),
                Arc::clone(&connection_server.active_theme),
                Arc::clone(&connection_server.runtime_diagnostics),
                connection_server.runtime_generation.clone(),
                connection_server.parse_coordinator.clone(),
                connection_server.completion.clone(),
                connection_server.document_analysis.clone(),
                connection_server.language_intelligence.clone(),
                Some(connection_server),
                codec,
            )
            .await
        });
        let mut client = client;

        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Hello {
                    protocol_version: crate::protocol::PROTOCOL_VERSION,
                    client_name: "failed-reload-client".to_string(),
                },
            )
            .await
            .unwrap();
        for _ in 0..7 {
            let _ = codec.read_server_message(&mut client).await.unwrap();
        }

        let mut updates = server.runtime_generation.subscribe_runtime_state();
        fs::write(root.join("init.js"), "export const = ;").unwrap();

        let outcome = server
            .execute_reload_command(reload_request())
            .await
            .expect("failed reload still returns a command outcome");
        assert!(!outcome.reloaded);
        assert_eq!(outcome.active_generation_id, 1);
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "clay.runtime.syntax_error")
        );
        assert!(
            server
                .runtime_diagnostics
                .lock()
                .await
                .iter()
                .any(|diagnostic| diagnostic.code == "clay.runtime.syntax_error")
        );
        assert!(
            server
                .runtime_generation
                .latest_runtime_snapshot_for(42)
                .await
                .is_none()
        );
        assert!(
            matches!(
                updates.try_recv(),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
            ),
            "failed reload must not publish a runtime generation id"
        );

        // Live connection must stay on G1: no RuntimeStateSnapshot appears.
        match tokio::time::timeout(
            Duration::from_millis(80),
            codec.read_server_message(&mut client),
        )
        .await
        {
            Err(_) => {}
            Ok(Ok(ServerMessage::RuntimeStateSnapshot(_))) => {
                panic!("failed reload must not fan out a generation snapshot")
            }
            Ok(Ok(ServerMessage::RuntimeDiagnostic(diagnostic))) => {
                assert_ne!(diagnostic.code, "clay.runtime.reload_succeeded");
            }
            Ok(Ok(other)) => panic!("unexpected live message after failed reload: {other:?}"),
            Ok(Err(error)) => panic!("client read failed: {error}"),
        }

        drop(client);
        let _ = server_task.await;
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn successful_reload_is_observed_as_one_generation_by_all_clients() {
        use std::sync::Arc;
        use tokio::io::duplex;

        let root = temp_config_root(
            "one-generation",
            r#"
            import { bindKey } from "clay:keybindings";
            bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
            Deno.core.ops.op_clay_runtime_record("one generation");
            "#,
        );
        let server = server_with_config(root.clone());
        let codec = crate::protocol::codec::Codec::default();

        async fn bootstrap_client(
            server: &IpcServer,
            client_id: u64,
            codec: crate::protocol::codec::Codec,
        ) -> (tokio::io::DuplexStream, tokio::task::JoinHandle<()>) {
            let (client, server_stream) = duplex(64 * 1024);
            let connection_server = server.clone();
            let handle = tokio::spawn(async move {
                let _ = crate::server::connection::handle_connection_with_analysis(
                    server_stream,
                    client_id,
                    Arc::clone(&connection_server.document),
                    Arc::clone(&connection_server.behavior),
                    Arc::clone(&connection_server.workspace),
                    Arc::clone(&connection_server.sdui),
                    Arc::clone(&connection_server.active_theme),
                    Arc::clone(&connection_server.runtime_diagnostics),
                    connection_server.runtime_generation.clone(),
                    connection_server.parse_coordinator.clone(),
                    connection_server.completion.clone(),
                    connection_server.document_analysis.clone(),
                    connection_server.language_intelligence.clone(),
                    Some(connection_server),
                    codec,
                )
                .await;
            });
            let mut client = client;
            codec
                .write_client_message(
                    &mut client,
                    &crate::protocol::ClientMessage::Hello {
                        protocol_version: crate::protocol::PROTOCOL_VERSION,
                        client_name: format!("client-{client_id}"),
                    },
                )
                .await
                .unwrap();
            for _ in 0..16 {
                match tokio::time::timeout(
                    Duration::from_millis(10),
                    codec.read_server_message(&mut client),
                )
                .await
                {
                    Ok(Ok(_)) => {}
                    Ok(Err(_)) | Err(_) => break,
                }
            }
            (client, handle)
        }

        let (mut client_a, task_a) = bootstrap_client(&server, 51, codec).await;
        let (mut client_b, task_b) = bootstrap_client(&server, 52, codec).await;

        assert!(server.reload_runtime_generation().await.reloaded);
        assert_eq!(server.runtime_generation.generation_id().await, 2);

        async fn read_complete_snapshot(
            codec: &crate::protocol::codec::Codec,
            client: &mut tokio::io::DuplexStream,
            expected_client_id: u64,
        ) -> crate::protocol::RuntimeStateSnapshot {
            loop {
                match tokio::time::timeout(
                    Duration::from_millis(500),
                    codec.read_server_message(client),
                )
                .await
                .expect("client receives one complete generation")
                .unwrap()
                {
                    ServerMessage::RuntimeStateSnapshot(snapshot) => {
                        assert_eq!(snapshot.client_id, expected_client_id);
                        assert_eq!(snapshot.runtime_generation_id, 2);
                        assert_eq!(snapshot.behavior.behavior_version, 2);
                        snapshot.validate().expect("fan-out snapshot validates");
                        return *snapshot;
                    }
                    ServerMessage::ActiveTypography(_)
                    | ServerMessage::BehaviorManifest(_)
                    | ServerMessage::DecorationSet(_)
                    | ServerMessage::DiagnosticSet(_)
                    | ServerMessage::RuntimeDiagnostic(_)
                    | ServerMessage::SduiSnapshot { .. }
                    | ServerMessage::SduiUpdate { .. } => {}
                    other => panic!("unexpected fan-out message: {other:?}"),
                }
            }
        }

        let snapshot_a = read_complete_snapshot(&codec, &mut client_a, 51).await;
        let snapshot_b = read_complete_snapshot(&codec, &mut client_b, 52).await;
        assert_eq!(
            snapshot_a.runtime_generation_id,
            snapshot_b.runtime_generation_id
        );
        assert_eq!(
            snapshot_a.behavior.behavior_version,
            snapshot_b.behavior.behavior_version
        );
        assert_eq!(snapshot_a.active_theme, snapshot_b.active_theme);
        assert_eq!(
            snapshot_a.active_typography.revision,
            snapshot_b.active_typography.revision
        );

        // Neither client may observe a second generation id for this commit.
        for (client, client_id) in [(&mut client_a, 51u64), (&mut client_b, 52)] {
            match tokio::time::timeout(Duration::from_millis(40), codec.read_server_message(client))
                .await
            {
                Err(_) => {}
                Ok(Ok(ServerMessage::RuntimeStateSnapshot(snapshot))) => {
                    panic!(
                        "client {client_id} observed a second snapshot generation {}",
                        snapshot.runtime_generation_id
                    );
                }
                Ok(Ok(_)) | Ok(Err(_)) => {}
            }
        }

        drop(client_a);
        drop(client_b);
        let _ = task_a.await;
        let _ = task_b.await;
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reload_preserves_authority_denials_and_cleans_old_lsp_worker() {
        use std::{
            io::Write,
            os::unix::fs::PermissionsExt,
            path::{Path, PathBuf},
        };

        use crate::server::language_server::LanguageServerSpawn;

        fn fake_echo_child(root: &Path) -> PathBuf {
            let path = root.join("fake-echo");
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(
                b"#!/bin/sh\nwhile IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done\n",
            )
            .unwrap();
            file.sync_all().unwrap();
            drop(file);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        }

        let root = temp_config_root(
            "lsp-cleanup",
            r#"Deno.core.ops.op_clay_runtime_record("lsp cleanup");"#,
        );
        let server = server_with_config(root.clone());
        let previous = server.runtime_generation.current_service().await;
        let process = previous.language_server_process_for_test();
        let executable = fake_echo_child(&root);
        let session = process
            .start(LanguageServerSpawn {
                package_name: "example".to_string(),
                contribution_id: "example.echo".to_string(),
                descriptor_fingerprint: 0,
                canonical_executable: executable,
                args: Vec::new(),
                inherit_environment: Vec::new(),
                cwd: root.clone(),
            })
            .await
            .expect("seed previous-generation language-server session");
        assert_eq!(previous.language_server_session_count().await, 1);
        let _ = session;

        let outcome = server.reload_runtime_generation().await;
        assert!(outcome.reloaded);
        assert_eq!(outcome.active_generation_id, 2);
        assert_eq!(
            previous.language_server_session_count().await,
            0,
            "successful commit must shut down previous-generation language-server sessions"
        );

        // Authority denials remain deny-by-default after a successful swap.
        fs::write(
            root.join("init.js"),
            r#"import "https://example.com/not-allowed.js";"#,
        )
        .unwrap();
        let denied = server.reload_runtime_generation().await;
        assert!(!denied.reloaded);
        assert_eq!(denied.active_generation_id, 2);
        let diagnostic = denied.diagnostics.last().expect("denial diagnostic");
        assert_eq!(diagnostic.code, "clay.configuration.invalid_module");
        assert!(
            !diagnostic
                .message
                .contains("https://example.com/not-allowed.js"),
            "diagnostics must not leak denied module URLs"
        );
        assert!(
            !diagnostic
                .message
                .contains(&root.to_string_lossy().to_string()),
            "diagnostics must not leak configuration paths"
        );

        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        sync::{Arc, atomic::AtomicU64},
        time::SystemTime,
    };

    use tokio::{net::UnixStream, sync::Mutex};

    use super::{ActiveBehaviorManifest, IpcServer, RuntimeGenerationStore, ServerConfig};
    use crate::server::{
        language_intelligence::LanguageIntelligenceCoordinator, locks::ScopedLockManager,
        parse_coordinator::ParseCoordinator, sdui::StaticSduiState,
    };
    use crate::{
        protocol::{
            ClientMessage, DocumentAccess, EditOperation, EditRejection, LockOwner,
            PROTOCOL_VERSION, ServerMessage, codec::Codec,
        },
        server::document::DocumentState,
    };

    fn unique_socket_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("clay-{name}-{}-{unique}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        dir.join("clay.sock")
    }

    fn server_with_document(socket_path: &std::path::Path, document: DocumentState) -> IpcServer {
        IpcServer {
            config: ServerConfig::new(socket_path),
            codec: Codec::default(),
            document: Arc::new(Mutex::new(document)),
            behavior: Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            workspace: {
                let mut workspace = crate::server::workspace::WorkspaceState::new();
                workspace.reserve_document_ids_from(2);
                Arc::new(Mutex::new(workspace))
            },
            sdui: Arc::new(Mutex::new(StaticSduiState::empty_for_document(1))),
            active_theme: Arc::new(Mutex::new(None)),
            runtime_diagnostics: Arc::new(Mutex::new(Vec::new())),
            parse_coordinator: ParseCoordinator::default(),
            completion: crate::server::completion::CompletionCoordinator::new(),
            document_analysis:
                crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
            language_intelligence: LanguageIntelligenceCoordinator::new(),
            runtime_generation: RuntimeGenerationStore::initial(),
            scoped_locks: ScopedLockManager::default(),
            reload_attempt: Arc::new(Mutex::new(())),
            next_client_id: Arc::new(AtomicU64::new(1)),
            #[cfg(test)]
            reload_barrier: super::ReloadCandidateBarrier::default(),
        }
    }

    #[tokio::test]
    async fn real_server_end_to_end_region_locked_edit_rejected() {
        let socket_path = unique_socket_path("region-lock");
        let mut document = DocumentState::default();
        let lock_id = document
            .register_region_lock(0, 7, LockOwner::Server)
            .unwrap();
        let server = server_with_document(&socket_path, document);
        let server_task = tokio::spawn(server.run());

        let mut stream = connect_with_retry(&socket_path).await;
        let codec = Codec::default();
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "region-lock-test".to_string(),
                },
            )
            .await
            .unwrap();

        let client_id = match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::Welcome { client_id, .. } => client_id,
            message => panic!("expected Welcome, got {message:?}"),
        };
        let (document_id, version, lease_id) =
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::InitialDocument {
                    document_id,
                    version,
                    access: DocumentAccess::Editable { lease_id },
                    lease_id: Some(snapshot_lease_id),
                    ..
                } => {
                    assert_eq!(lease_id, snapshot_lease_id);
                    (document_id, version, lease_id)
                }
                message => panic!("expected editable InitialDocument, got {message:?}"),
            };
        let behavior_version = match codec.read_server_message(&mut stream).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
            message => panic!("expected BehaviorManifest, got {message:?}"),
        };
        let _active_theme = codec.read_server_message(&mut stream).await.unwrap();
        let _active_typography = codec.read_server_message(&mut stream).await.unwrap();
        loop {
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::FileOpenCapabilityIssued { .. } => break,
                ServerMessage::SduiSnapshot { .. } | ServerMessage::RuntimeDiagnostic(_) => {}
                message => panic!("expected file-open capability, got {message:?}"),
            }
        }

        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Edit {
                    document_id,
                    client_id,
                    lease_id: Some(lease_id),
                    base_version: version,
                    behavior_version,
                    transaction_id: 12,
                    operation: EditOperation::Insert {
                        byte_offset: 1,
                        text: "x".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        loop {
            match codec.read_server_message(&mut stream).await.unwrap() {
                ServerMessage::EditRejected {
                    document_id: rejected_document_id,
                    transaction_id: 12,
                    reason: EditRejection::RegionLocked { conflict },
                } if rejected_document_id == document_id
                    && conflict.lock_id == lock_id
                    && conflict.start == 0
                    && conflict.end == 7
                    && conflict.owner == LockOwner::Server
                    && conflict.created_at_version == version =>
                {
                    break;
                }
                ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::RuntimeDiagnostic(_) => {}
                other => panic!("expected region-lock rejection, got {other:?}"),
            }
        }

        server_task.abort();
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_dir(socket_path.parent().unwrap());
    }

    #[test]
    fn server_accepts_configured_workspace_roots_and_reports_invalid_roots() {
        let socket_path = unique_socket_path("configured-workspace");
        let root = socket_path.parent().unwrap().join("workspace");
        fs::create_dir(&root).unwrap();

        let mut config = ServerConfig::new(&socket_path);
        config.workspace_roots = vec![root.clone()];
        let server = IpcServer::try_new(config).unwrap();
        assert_eq!(server.config.workspace_roots, vec![root]);

        let missing_root = socket_path.parent().unwrap().join("missing");
        let mut invalid_config = ServerConfig::new(&socket_path);
        invalid_config.workspace_roots = vec![missing_root];
        let error = IpcServer::try_new(invalid_config).unwrap_err();
        assert!(matches!(error, super::ServerError::InvalidWorkspaceRoot(_)));
        assert!(error.to_string().contains("invalid workspace root"));

        let _ = fs::remove_dir(server.config.workspace_roots[0].clone());
        let _ = fs::remove_dir(socket_path.parent().unwrap());
    }

    #[tokio::test]
    async fn default_server_starts_without_workspace_sdui_snapshot() {
        let socket_path = unique_socket_path("no-default-sdui");
        let server = IpcServer::new(ServerConfig::new(&socket_path));

        assert!(server.sdui.lock().await.snapshot_message(1).is_none());

        let _ = fs::remove_dir(socket_path.parent().unwrap());
    }

    #[tokio::test]
    async fn server_listener_accepts_client_hello() {
        let socket_path = unique_socket_path("listener-hello");
        let server = IpcServer::new(ServerConfig::new(&socket_path));
        let server_task = tokio::spawn(server.run());

        let mut stream = connect_with_retry(&socket_path).await;
        let codec = Codec::default();
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "listener-test".to_string(),
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::Welcome { .. }
        ));
        assert!(matches!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::InitialDocument {
                access: DocumentAccess::Editable { lease_id: 1 },
                ..
            }
        ));

        server_task.abort();
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_dir(socket_path.parent().unwrap());
    }

    async fn connect_with_retry(socket_path: &std::path::Path) -> UnixStream {
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

    #[tokio::test]
    async fn unix_socket_is_created_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let socket_path = unique_socket_path("owner-only");
        let server = IpcServer::new(ServerConfig::new(&socket_path));
        let server_task = tokio::spawn(server.run());

        let stream = connect_with_retry(&socket_path).await;
        drop(stream);

        let metadata = std::fs::metadata(&socket_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "socket must be created with owner-only permissions, got {mode:o}"
        );

        server_task.abort();
        let _ = std::fs::remove_file(&socket_path);
        let _ = std::fs::remove_dir(socket_path.parent().unwrap());
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use std::os::windows::io::AsRawHandle;

    use windows::Win32::{
        Foundation::HANDLE,
        Security::{
            ACL,
            Authorization::{GetSecurityInfo, SE_KERNEL_OBJECT},
            DACL_SECURITY_INFORMATION,
        },
    };

    use super::create_named_pipe_server;

    #[tokio::test]
    async fn windows_pipe_creation_applies_current_user_security_descriptor() {
        let pipe_name = format!(
            r"\\.\pipe\clay-test-security-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let pipe = create_named_pipe_server(&pipe_name)
            .expect("pipe creation with current-user-only security descriptor should succeed");

        // Read back the DACL and verify the pipe no longer uses the default
        // descriptor (which has multiple ACEs for LocalSystem/Admins/Owner/
        // Everyone/Anonymous). A single-ACE DACL confirms we installed a
        // custom, restricted descriptor.
        unsafe {
            let mut dacl: *mut ACL = std::ptr::null_mut();
            GetSecurityInfo(
                HANDLE(pipe.as_raw_handle()),
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut dacl),
                None,
                None,
            )
            .ok()
            .expect("GetSecurityInfo should succeed on the pipe we created");

            assert!(
                !dacl.is_null(),
                "pipe must have a DACL after custom descriptor is applied"
            );
            assert_eq!(
                (*dacl).AceCount,
                1,
                "current-user-only DACL must have exactly one ACE"
            );

            // LocalFree expects the descriptor returned by GetSecurityInfo, but
            // because we passed null for ppSecurityDescriptor we only need to
            // free the DACL if GetSecurityInfo allocated it. In practice
            // GetSecurityInfo returns a self-relative descriptor whose DACL is
            // internal; passing the handle-owned descriptor pointer to LocalFree
            // is undefined. We therefore do not free `dacl` here — it is valid
            // only while the pipe handle remains open.
        }

        drop(pipe);
    }
}
