mod behavior;
pub mod command_execution;
pub mod completion;
mod configuration;
mod connection;
pub(crate) mod control_center;
pub mod decorations;
pub(crate) mod document;
#[allow(dead_code)]
pub(crate) mod git;
#[allow(dead_code)]
mod js_runtime;
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

use tokio::{sync::Mutex, task::JoinSet};

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
    protocol::{DocumentId, RuntimeDiagnostic, ServerMessage, codec::Codec},
};

use self::{
    behavior::ActiveBehaviorManifest, connection::handle_connection, document::DocumentState,
    js_runtime::ClayJsRuntimeService, parse_coordinator::ParseCoordinator, sdui::StaticSduiState,
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
    diagnostics: Vec<RuntimeDiagnostic>,
}

impl RuntimeGeneration {
    fn initial() -> Self {
        Self {
            id: 1,
            service: ClayJsRuntimeService::default(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeGenerationStore {
    current: Arc<Mutex<RuntimeGeneration>>,
}

impl RuntimeGenerationStore {
    fn initial() -> Self {
        Self {
            current: Arc::new(Mutex::new(RuntimeGeneration::initial())),
        }
    }

    pub(crate) async fn generation_id(&self) -> u64 {
        self.current.lock().await.id
    }

    pub(crate) async fn current(&self) -> RuntimeGeneration {
        self.current.lock().await.clone()
    }

    pub(crate) async fn current_service(&self) -> ClayJsRuntimeService {
        self.current().await.service
    }

    async fn push_diagnostic(&self, diagnostic: RuntimeDiagnostic) {
        self.current.lock().await.diagnostics.push(diagnostic);
    }

    async fn swap(&self, next: RuntimeGeneration) {
        *self.current.lock().await = next;
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadedDocumentRefresh {
    pub document_id: DocumentId,
    pub messages: Vec<ServerMessage>,
}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReloadOutcome {
    pub previous_generation_id: u64,
    pub active_generation_id: u64,
    pub reloaded: bool,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub refreshed_documents: Vec<ReloadedDocumentRefresh>,
}

#[derive(Debug)]
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
    runtime_generation: RuntimeGenerationStore,
    next_client_id: AtomicU64,
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
            runtime_generation: RuntimeGenerationStore::initial(),
            next_client_id: AtomicU64::new(1),
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
        let service = self.runtime_generation.current_service().await;
        let evaluation = self.load_configuration_for_service(&service).await;

        match evaluation {
            Ok(Some(evaluation)) => {
                self.apply_runtime_evaluation(
                    self.runtime_generation.generation_id().await,
                    &service,
                    evaluation,
                )
                .await
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
        let diagnostic = error.diagnostic();
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

    pub(crate) async fn reload_runtime_generation(&self) -> RuntimeReloadOutcome {
        let previous_generation_id = self.runtime_generation.generation_id().await;
        let next_generation_id = previous_generation_id.saturating_add(1);
        let next_service = ClayJsRuntimeService::default();

        match self.load_configuration_for_service(&next_service).await {
            Ok(evaluation) => {
                let diagnostics = Vec::new();
                if let Some(evaluation) = evaluation {
                    self.apply_runtime_evaluation(next_generation_id, &next_service, evaluation)
                        .await;
                }
                self.parse_coordinator
                    .cancel_generation(previous_generation_id);
                self.runtime_generation
                    .swap(RuntimeGeneration {
                        id: next_generation_id,
                        service: next_service.clone(),
                        diagnostics: Vec::new(),
                    })
                    .await;
                let refreshed_documents = self
                    .refresh_open_documents_after_reload(next_generation_id, &next_service)
                    .await;
                RuntimeReloadOutcome {
                    previous_generation_id,
                    active_generation_id: next_generation_id,
                    reloaded: true,
                    diagnostics,
                    refreshed_documents,
                }
            }
            Err(error) => {
                let diagnostic = error.diagnostic();
                self.record_runtime_error("clay server runtime reload failed", error)
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

    async fn refresh_open_documents_after_reload(
        &self,
        generation_id: u64,
        service: &ClayJsRuntimeService,
    ) -> Vec<ReloadedDocumentRefresh> {
        let snapshots = match self.workspace.lock().await.open_document_snapshots(0).await {
            Ok(snapshots) => snapshots,
            Err(error) => {
                self.runtime_diagnostics
                    .lock()
                    .await
                    .push(RuntimeDiagnostic::error(
                        "clay.runtime.reload_refresh_failed",
                        format!(
                            "Reload open-document refresh failed: {:?}",
                            error.diagnostic()
                        ),
                    ));
                return Vec::new();
            }
        };

        let mut refreshed = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            let messages = connection::open_document_followup_messages(
                &snapshot.metadata,
                &snapshot.text,
                &self.behavior,
                &self.sdui,
                generation_id,
                service,
                &self.parse_coordinator,
            )
            .await;
            refreshed.push(ReloadedDocumentRefresh {
                document_id: snapshot.metadata.document_id,
                messages,
            });
        }
        refreshed
    }

    async fn apply_runtime_evaluation(
        &self,
        generation_id: u64,
        service: &ClayJsRuntimeService,
        evaluation: js_runtime::ClayRuntimeEvaluation,
    ) {
        let application = apply_runtime_outputs(
            &evaluation,
            self.sdui.lock().await.document_id(),
            &self.behavior,
            &self.sdui,
        )
        .await;

        // Plan 046 task 7: resolve the active theme selected by `setTheme` in
        // `init.js` onto the shared slot the welcome handshake ships to the
        // client. `None` clears any previously selected theme on reload.
        *self.active_theme.lock().await = evaluation.active_theme.clone();

        if let Err(error) =
            service.register_parse_handlers(&self.parse_coordinator, generation_id, &evaluation)
        {
            self.runtime_diagnostics
                .lock()
                .await
                .push(RuntimeDiagnostic::error(
                    "clay.parse.registration_failed",
                    format!("Runtime parse handler registration failed: {error:?}"),
                ));
        }

        // Startup reads the shared behavior/SDUI state lazily during the
        // welcome handshake, so only validation failures produce diagnostics
        // here. Decorations are pass-through (no startup client / decoration store).
        for diagnostic in application.diagnostics() {
            eprintln!(
                "clay server rejected runtime output [{}]: {}",
                diagnostic.code, diagnostic.message
            );
            self.runtime_diagnostics.lock().await.push(diagnostic);
        }
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
        let codec = self.codec;
        connections.spawn(async move {
            if let Err(error) = handle_connection(
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
                codec,
            )
            .await
            {
                eprintln!("clay server connection {client_id} closed with error: {error}");
            }
        });
    }
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
    pub(crate) behavior: Option<Result<crate::protocol::BehaviorManifest, ()>>,
    /// `Some(Ok(tree))` when a runtime tree replaced the per-document SDUI
    /// state — the caller builds the `SduiSnapshot` message with its own
    /// `client_id`. `Some(Err(()))` on validation failure; `None` when no
    /// tree was published.
    pub(crate) sdui: Option<Result<crate::protocol::SduiTree, ()>>,
    /// Published decoration set, passed through for the caller to emit. The
    /// config-eval boundary holds no per-document decoration store, so this is
    /// not applied to shared state here.
    #[allow(
        dead_code,
        reason = "selected-file activation consumes decoration output directly; startup config keeps it for future caller parity"
    )]
    pub(crate) decorations: Option<crate::protocol::DecorationSet>,
}

impl RuntimeOutputApplication {
    /// Unified diagnostics for outputs that failed validation. Both call sites
    /// surface these so the diagnostic codes stay identical across flows
    /// (`clay.behavior.invalid_manifest`, `clay.sdui.invalid_tree`).
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
/// separately by `IpcServer::apply_runtime_evaluation` because they need the
/// persistent runtime service, not just this output-application primitive.
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
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: Some(valid_manifest()),
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            completion_providers: vec![],
            active_theme: None,
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
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: Some(valid_manifest()),
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            completion_providers: vec![],
            active_theme: None,
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
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: None,
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            completion_providers: vec![],
            active_theme: None,
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
            parse_handlers: vec![],
            js_parse_handlers: vec![],
            behavior_manifest: None,
            ui_contributions: Default::default(),
            syntax_grammars: vec![],
            completion_providers: vec![],
            active_theme: None,
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
    use std::{fs, time::SystemTime};

    use crate::{ipc::IpcEndpoint, protocol::ServerMessage};

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
                .any(|message| matches!(message, ServerMessage::DecorationSet(_)))
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
    use crate::server::{ParseCoordinator, sdui::StaticSduiState};
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
            runtime_generation: RuntimeGenerationStore::initial(),
            next_client_id: AtomicU64::new(1),
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
        let _manifest = codec.read_server_message(&mut stream).await.unwrap();
        let _active_theme = codec.read_server_message(&mut stream).await.unwrap();
        assert!(matches!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ));

        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Edit {
                    document_id,
                    client_id,
                    lease_id: Some(lease_id),
                    base_version: version,
                    behavior_version: 1,
                    transaction_id: 12,
                    operation: EditOperation::Insert {
                        byte_offset: 1,
                        text: "x".to_string(),
                    },
                },
            )
            .await
            .unwrap();

        assert!(matches!(
            codec.read_server_message(&mut stream).await.unwrap(),
            ServerMessage::EditRejected {
                document_id: rejected_document_id,
                transaction_id: 12,
                reason: EditRejection::RegionLocked { conflict },
            } if rejected_document_id == document_id
                && conflict.lock_id == lock_id
                && conflict.start == 0
                && conflict.end == 7
                && conflict.owner == LockOwner::Server
                && conflict.created_at_version == version
        ));

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
