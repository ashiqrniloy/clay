//! Server-owned runtime state: configuration, the live JS runtime
//! generation store, the behavior/typography runtime fanouts, and the
//! shared application of runtime outputs to server state.

use super::*;

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
    // `pub(super)`: connection tests assemble generations directly (this struct
    // was module-private in `server/mod.rs` before plan 133 task 3).
    pub(super) id: u64,
    pub(super) service: ClayJsRuntimeService,
    pub(super) evaluation: Option<Arc<js_runtime::ClayRuntimeEvaluation>>,
    pub(super) diagnostics: Vec<RuntimeDiagnostic>,
}

impl RuntimeGeneration {
    pub(super) fn initial(agent_host: crate::server::agent::AgentHostHandle) -> Self {
        let service = ClayJsRuntimeService::production();
        // Plan 130 A1: the server's own agent host rides into every lane op
        // state here — construction, not a process global.
        service.set_agent_host(agent_host);
        Self {
            id: 1,
            service,
            evaluation: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeGenerationStore {
    /// The live generation (service, evaluation, diagnostics). Not a lane: the
    /// runtime-state fanout signals generation changes, this holds the state.
    pub(super) current: Arc<Mutex<RuntimeGeneration>>,
    pub(super) typography: ActiveTypographyState,
    pub(super) runtime_state: ActiveRuntimeStateFanout,
    pub(super) behavior_grace: BehaviorGraceState,
}

/// Latest committed runtime snapshot and bounded live-update lane.
///
/// Deliberately not a `StateFanout`: the lane carries only the generation id
/// while the store holds the snapshot, which is narrowed per client on read
/// (`for_client`), so publish value and channel value differ by design. The
/// acknowledgement map is per-client state, not a current value either.
#[derive(Debug, Clone)]
pub(crate) struct ActiveRuntimeStateFanout {
    latest: Arc<Mutex<Option<RuntimeStateSnapshot>>>,
    updates: fanout::Fanout<RuntimeGenerationId>,
    acknowledgements:
        Arc<Mutex<std::collections::HashMap<crate::protocol::ClientId, RuntimeGenerationId>>>,
}

impl Default for ActiveRuntimeStateFanout {
    fn default() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
            updates: fanout::Fanout::new(RUNTIME_STATE_BROADCAST_CAPACITY),
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
        self.updates.publish(generation);
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

/// Server-owned active typography and bounded live-update lane.
///
/// The lane is a `Fanout`, but the store is deliberately not a `StateFanout`:
/// `IpcServer::commit_runtime_generation` holds this guard across its six-state
/// conflict check and broadcasts only after the other guards drop, so the
/// store's write and its send are two separate steps that a lane's
/// publish-and-record API cannot express. `replace` is the one caller that
/// pairs them, and it sends after dropping the guard for the same reason.
#[derive(Debug, Clone)]
pub(crate) struct ActiveTypographyState {
    pub(super) current: Arc<Mutex<crate::protocol::ActiveTypography>>,
    pub(super) updates: fanout::Fanout<crate::protocol::ActiveTypography>,
}

impl Default for ActiveTypographyState {
    fn default() -> Self {
        Self {
            current: Arc::new(Mutex::new(crate::protocol::ActiveTypography::default())),
            updates: fanout::Fanout::new(16),
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
    /// their revision and emit no duplicate client event. Validation, compare,
    /// revision bump, and store write all happen under the store lock, so two
    /// concurrent replacements cannot claim the same revision.
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
            && current.hierarchy == typography.hierarchy
        {
            return Ok(None);
        }
        typography.revision = current.revision.saturating_add(1);
        *current = typography.clone();
        drop(current);
        self.updates.publish(typography.clone());
        Ok(Some(typography))
    }
}

impl RuntimeGenerationStore {
    pub(super) fn initial(agent_host: crate::server::agent::AgentHostHandle) -> Self {
        Self {
            current: Arc::new(Mutex::new(RuntimeGeneration::initial(agent_host))),
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

    /// Follow-up round (`editor-control`): subscribe to gated programmatic
    /// editor-command execution requests. The channel is shared across
    /// runtime generations, so one subscription covers reloads.
    pub(crate) async fn subscribe_editor_commands(
        &self,
    ) -> broadcast::Receiver<crate::protocol::EditorCommandRequest> {
        self.current_service().await.subscribe_editor_commands()
    }

    /// Plan 071 caret-transport fix: subscribe to runtime caret override
    /// updates. The channel is shared across runtime generations.
    pub(crate) async fn subscribe_caret_styles(
        &self,
    ) -> broadcast::Receiver<Option<crate::protocol::CaretStyle>> {
        self.current_service().await.subscribe_caret_styles()
    }

    /// Current runtime caret override (connection initial sync / lag replay).
    pub(crate) async fn caret_style_override(&self) -> Option<crate::protocol::CaretStyle> {
        self.current_service().await.caret_style_override()
    }

    /// Phase 26: subscribe to user-owned editor wrap-policy override updates.
    /// The channel is shared across runtime generations.
    pub(crate) async fn subscribe_editor_layout(
        &self,
    ) -> broadcast::Receiver<Option<crate::protocol::WrapPolicy>> {
        self.current_service().await.subscribe_editor_layout()
    }

    /// Current editor wrap-policy override (connection initial sync / lag
    /// replay).
    pub(crate) async fn editor_layout_override(&self) -> Option<crate::protocol::WrapPolicy> {
        self.current_service().await.editor_layout_override()
    }

    /// Phase 22.1: subscribe to shell-preferences updates.
    pub(crate) async fn subscribe_shell_preferences(
        &self,
    ) -> broadcast::Receiver<crate::protocol::ShellPreferences> {
        self.current_service().await.subscribe_shell_preferences()
    }

    /// Current shell preferences (connection initial sync / lag replay).
    pub(crate) async fn shell_preferences(&self) -> crate::protocol::ShellPreferences {
        self.current_service().await.shell_preferences()
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

    /// Build one bounded inert command catalogue from the active runtime
    /// generation. The worker snapshots clone only Rust metadata; no runtime
    /// evaluation or V8 value crosses this boundary.
    pub(crate) async fn command_catalogue_snapshot(
        &self,
        active_manifest: &crate::protocol::BehaviorManifest,
    ) -> Result<(u64, CommandCatalogue), CommandCatalogueError> {
        let current = self.current.lock().await;
        let (trusted_commands, third_party_commands) = current.service.command_registry_snapshots();
        let builtins = crate::server::command_execution::builtin_server_command_ids()
            .iter()
            .filter_map(|command_id| {
                crate::server::command_execution::builtin_server_command(command_id)
            })
            .collect();
        let shell_commands = shell_command_catalogue();
        let catalogue = CommandCatalogue::from_sources(
            vec![
                builtins,
                shell_commands,
                trusted_commands,
                third_party_commands,
            ],
            active_manifest,
        )?;
        Ok((current.id, catalogue))
    }

    pub(super) async fn push_diagnostic(&self, diagnostic: RuntimeDiagnostic) {
        self.current.lock().await.diagnostics.push(diagnostic);
    }

    pub(super) async fn swap(&self, next: RuntimeGeneration) {
        *self.current.lock().await = next;
    }
}

fn shell_command_catalogue() -> Vec<RegisteredCommand> {
    crate::client_commands::SHELL_CLIENT_COMMAND_CATALOGUE
        .iter()
        .map(|(command_id, display_name)| RegisteredCommand {
            package_name: "clay".to_string(),
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            api_prefix: "shell".to_string(),
            command_id: (*command_id).to_string(),
            display_name: (*display_name).to_string(),
            routing_policy: crate::protocol::RoutingPolicy::ClientUiCommand,
            key_bindings: Vec::new(),
            custom_properties: Default::default(),
            permissions: Vec::new(),
        })
        .collect()
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
    /// (`behavior.invalid_manifest`, `sdui.invalid_tree`).
    #[cfg(test)]
    pub(crate) fn diagnostics(&self) -> Vec<RuntimeDiagnostic> {
        let mut diagnostics = Vec::new();
        if matches!(self.behavior, Some(Err(()))) {
            diagnostics.push(RuntimeDiagnostic::error(
                "behavior.invalid_manifest",
                "Runtime behavior manifest failed server validation.",
            ));
        }
        if matches!(self.sdui, Some(Err(()))) {
            diagnostics.push(RuntimeDiagnostic::error(
                "sdui.invalid_tree",
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
