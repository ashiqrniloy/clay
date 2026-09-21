//! `IpcServer` construction and the persistent-runtime reload pipeline:
//! configuration load, runtime generation candidate preparation, install,
//! and open-document refresh.

use super::*;

/// Daemon data location fallback: when the server was started without an
/// explicit configuration root, keep agent state (sessions, credentials,
/// book) under the user's default config root when it exists, instead of
/// dropping it in the temp dir.
fn effective_agent_root() -> Option<PathBuf> {
    let root = ConfigurationRuntime::default_config_root()?;
    root.join("init.js").is_file().then_some(root)
}

#[derive(Debug)]
pub(super) struct RuntimeGenerationCandidate {
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
    expected_design_system: crate::shell::design_system::ActiveDesignSystem,
    active_design_system: crate::shell::design_system::ActiveDesignSystem,
    expected_icon_pack: Option<crate::shell::icons::ActiveIconPack>,
    active_icon_pack: Option<crate::shell::icons::ActiveIconPack>,
    open_documents: Vec<workspace::OpenDocumentRefresh>,
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
                "parse.registration_failed",
                "Runtime parse handler registration failed validation.",
            )
        })?;
    service
        .register_completion_providers(completion, generation_id, evaluation)
        .map_err(|_| {
            runtime_candidate_error(
                "completion.registration_failed",
                "Runtime completion provider registration failed validation.",
            )
        })?;
    for registration in &evaluation.document_analyzers {
        // Phase 24.5 decision (2026-08-13-2223): an analyzer registered
        // without a language-server grant stays inactive rather than failing
        // the generation — grantLanguageServer degrades independently when
        // tooling/roots are absent. It is not registered for this generation
        // and re-registers on a later reload once the grant lands; the
        // per-document route check keeps denying invocation until then.
        if !service.document_analysis_registration_authorized(registration) {
            continue;
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
                    "analysis.registration_failed",
                    "Runtime document analyzer registration failed validation.",
                )
            })?;
    }
    service
        .register_language_intelligence_providers(language_intelligence, generation_id, evaluation)
        .map_err(|_| {
            runtime_candidate_error(
                "language.registration_failed",
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
            "typography.invalid_configuration",
            "Runtime typography failed server validation.",
        )
    })?;
    requested.revision = if current.monospace == requested.monospace
        && current.proportional == requested.proportional
        && current.ui == requested.ui
        && current.hierarchy == requested.hierarchy
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
    behavior: &ActiveBehaviorManifest,
    active_theme: crate::protocol::ActiveTheme,
    active_typography: crate::protocol::ActiveTypography,
    active_design_system: crate::shell::design_system::ActiveDesignSystem,
    active_icon_pack: Option<crate::shell::icons::ActiveIconPack>,
    sdui_tree: crate::protocol::SduiTree,
    open_documents: &[workspace::OpenDocumentRefresh],
    published_decorations: Option<crate::protocol::DecorationSet>,
    published_diagnostics: Option<crate::protocol::DiagnosticSet>,
    diagnostics: Vec<RuntimeDiagnostic>,
    package_ui: crate::protocol::PackageUiSnapshot,
    ui_choices: crate::protocol::UiChoicesSnapshot,
) -> Result<RuntimeStateSnapshot, RuntimeDiagnostic> {
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
                behavior_manifest: Some(behavior.manifest_for(document_id).clone()).filter(
                    |manifest| {
                        matches!(
                            manifest.scope,
                            crate::protocol::BehaviorScope::Document {
                                document_id: scope_document_id
                            } if scope_document_id == document_id
                        )
                    },
                ),
            }
        })
        .collect();
    let snapshot = RuntimeStateSnapshot {
        runtime_generation_id,
        client_id: 0,
        behavior: behavior.manifest().clone(),
        active_theme,
        active_typography,
        active_design_system,
        active_icon_pack,
        sdui_tree,
        package_ui,
        documents,
        diagnostics,
        ui_choices,
    };
    snapshot.validate().map_err(|_| {
        runtime_candidate_error(
            "runtime.invalid_snapshot",
            "Runtime state snapshot failed validation before commit.",
        )
    })?;
    Ok(snapshot)
}

fn runtime_candidate_error(code: &'static str, message: &'static str) -> RuntimeDiagnostic {
    RuntimeDiagnostic::error(code, message)
}

impl IpcServer {
    #[cfg(test)]
    pub fn new(config: ServerConfig) -> Self {
        Self::try_new(config).expect("test server config must be valid")
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
        let document_id_allocator = Arc::new(AtomicU64::new(1));
        let bootstrap_state =
            TabServerState::from_workspace(workspace, Arc::clone(&document_id_allocator));
        let tab_registry = Arc::new(Mutex::new(tab_registry::TabRegistry::new()));
        let agent = agent::AgentHost::for_server(
            config
                .configuration_root
                .clone()
                .or_else(effective_agent_root)
                .as_deref(),
            // Repo-root .mcp.json rides the launch workspace (plan 117);
            // the user mcp.json comes from the configuration root. The
            // cwd fallback mirrors add_root_from_cwd below.
            config
                .workspace_roots
                .first()
                .cloned()
                .or_else(|| std::env::current_dir().ok())
                .as_deref(),
        );
        // Plan 119 SC-6: agent tool calls resolve their workspace from the
        // session (its recorded root, in the tab state that has that folder
        // open) — never from the bootstrap state's first root.
        let tab_states = Arc::new(Mutex::new(HashMap::new()));
        let session_workspaces = agent_documents::SessionWorkspaces::new(
            Arc::clone(&tab_registry),
            Arc::clone(&tab_states),
        );
        agent.set_reverse_handler(agent_documents::document_reverse_handler(
            session_workspaces,
            Arc::new(Mutex::new(agent_checkpoints::AgentCheckpointStore::new())),
            agent.clone(),
        ));
        // Plan 109 I1: the agent host reads each tab's current workspace
        // root from the registry so agent sessions bind to the tab's
        // workspace (and rebind when it changes), never the launch cwd.
        agent.set_tab_registry(Arc::clone(&tab_registry));
        // Plan 118 task 35: per-agent config roots resolve from the Clay data
        // root (`<root>/agents/<agent type>`); each session's MCP allow-list
        // merges that agent's own `mcp.json` with the *session's* workspace
        // `.mcp.json` (plan 119 SC-6).
        agent.set_agent_roots(
            config
                .configuration_root
                .clone()
                .or_else(effective_agent_root),
        );
        // Phase 1 `agent` domain: server-owned RPC authority for the
        // user-facing agent facades (`clay:agent`). Package JS cannot reach
        // the daemon except through these validated ops, and a second server
        // in the same process owns its own host (Plan 130 A1).
        let agent_host = agent::AgentHostHandle::new(agent.clone());

        Ok(Self {
            config,
            codec: Codec::default(),
            #[cfg(test)]
            document: Arc::clone(&bootstrap_state.welcome),
            #[cfg(test)]
            workspace: Arc::clone(&bootstrap_state.workspace),
            bootstrap_state,
            tab_states,
            document_id_allocator,
            bootstrap_consumed: Arc::new(AtomicBool::new(false)),
            behavior: Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
            sdui: Arc::new(Mutex::new(StaticSduiState::empty_for_document(1))),
            active_theme: Arc::new(Mutex::new(None)),
            active_design_system: Arc::new(Mutex::new(
                crate::shell::design_system::ActiveDesignSystem::core_fallback(0),
            )),
            active_icon_pack: Arc::new(Mutex::new(None)),
            runtime_diagnostics: Arc::new(
                Mutex::new(connection::RuntimeDiagnosticStore::default()),
            ),
            connection_permits: Arc::new(tokio::sync::Semaphore::new(
                crate::perf::budgets::MAX_ACTIVE_CONNECTIONS,
            )),
            parse_coordinator: ParseCoordinator::default(),
            completion: crate::server::completion::CompletionCoordinator::new(),
            document_analysis:
                crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
            language_intelligence: LanguageIntelligenceCoordinator::new(),
            runtime_generation: RuntimeGenerationStore::initial(agent_host),
            scoped_locks: ScopedLockManager::default(),
            reload_attempt: Arc::new(Mutex::new(())),
            next_client_id: Arc::new(AtomicU64::new(1)),
            live_clients: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            tab_registry,
            tab_registry_tx: broadcast::channel(
                crate::perf::budgets::RUNTIME_STATE_BROADCAST_CAPACITY,
            )
            .0,
            agent,
            #[cfg(test)]
            reload_barrier: ReloadCandidateBarrier::default(),
        })
    }

    pub(super) async fn load_default_configuration(&self) {
        // Plan 129 P4: boxed reload stages keep `run`/`main` futures small too
        // (same reason as `reload_runtime_generation_inner`).
        let generation_id = self.runtime_generation.generation_id().await;
        let service = self.runtime_generation.current_service().await;
        match Box::pin(self.load_configuration_for_service(&service)).await {
            Ok(Some(evaluation)) => {
                self.record_configuration_diagnostics(&evaluation.configuration_diagnostics)
                    .await;
                match Box::pin(self.prepare_runtime_generation_candidate(
                    generation_id,
                    generation_id,
                    service,
                    evaluation,
                ))
                .await
                {
                    Ok(candidate) => {
                        if let Err(diagnostic) =
                            Box::pin(self.commit_runtime_generation(candidate)).await
                        {
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

    pub(super) async fn load_configuration_for_service(
        &self,
        service: &ClayJsRuntimeService,
    ) -> Result<Option<js_runtime::ClayRuntimeEvaluation>, js_runtime::ClayRuntimeError> {
        if let Some(config_root) = self.config.configuration_root.clone() {
            service
                .load_configuration_from_root_with_workspace(
                    config_root,
                    Arc::clone(&self.bootstrap_state.workspace),
                )
                .await
                .map(Some)
        } else {
            service
                .load_default_configuration_with_workspace(Arc::clone(
                    &self.bootstrap_state.workspace,
                ))
                .await
        }
    }

    async fn record_runtime_error(&self, context: &str, error: js_runtime::ClayRuntimeError) {
        self.record_runtime_diagnostic(context, error.diagnostic())
            .await;
    }

    async fn record_configuration_diagnostics(&self, diagnostics: &[RuntimeDiagnostic]) {
        for diagnostic in diagnostics {
            self.record_runtime_diagnostic(
                "clay server optional configuration module failed",
                diagnostic.clone(),
            )
            .await;
        }
    }

    async fn record_runtime_diagnostic(&self, context: &str, diagnostic: RuntimeDiagnostic) {
        eprintln!("{context} [{}]: {}", diagnostic.code, diagnostic.message);
        self.runtime_generation
            .push_diagnostic(diagnostic.clone())
            .await;
        self.runtime_diagnostics.lock().await.publish(diagnostic);
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
        Ok(Box::pin(self.reload_runtime_generation_inner()).await)
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
                        "runtime.reload_in_progress",
                        error.message,
                    )],
                    refreshed_documents: Vec::new(),
                }
            }
        }
    }

    // Plan 129 P4: the three heavy reload stages are boxed so this future (and
    // every caller's: settings persistence, command intents, the connection
    // loop) stays under clippy's large-futures threshold. Reload is rare; the
    // allocation is cold-path only.

    async fn reload_runtime_generation_inner(&self) -> RuntimeReloadOutcome {
        #[cfg(test)]
        self.reload_barrier.wait_if_armed().await;
        let previous_generation_id = self.runtime_generation.generation_id().await;
        let next_generation_id = previous_generation_id.saturating_add(1);
        // Trusted-generation reload shares the live third-party domain so
        // adopted third-party packages, providers, and language-server
        // sessions survive untouched (Plan 061 task 12).
        let current_service = self.runtime_generation.current_service().await;
        let next_service = ClayJsRuntimeService::production_reload(&current_service);
        let (evaluation, configuration_diagnostics) =
            match Box::pin(self.load_configuration_for_service(&next_service)).await {
                Ok(evaluation) => {
                    let evaluation = evaluation.unwrap_or_default();
                    let diagnostics = evaluation.configuration_diagnostics.clone();
                    self.record_configuration_diagnostics(&diagnostics).await;
                    (evaluation, diagnostics)
                }
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
        let candidate = match Box::pin(self.prepare_runtime_generation_candidate(
            previous_generation_id,
            next_generation_id,
            next_service,
            evaluation,
        ))
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
        match Box::pin(self.commit_runtime_generation(candidate)).await {
            Ok(refreshed_documents) => RuntimeReloadOutcome {
                previous_generation_id,
                active_generation_id: next_generation_id,
                reloaded: true,
                diagnostics: configuration_diagnostics,
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

    pub(super) async fn prepare_runtime_generation_candidate(
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
                    "behavior.invalid_manifest",
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
                        "sdui.invalid_tree",
                        "Runtime SDUI tree failed server validation.",
                    )
                })?;
        }

        evaluation.ui_contributions.validate().map_err(|_| {
            runtime_candidate_error(
                "ui.invalid_snapshot",
                "Runtime package UI contributions failed server validation.",
            )
        })?;
        syntax::SyntaxGrammarRegistry::validate_snapshot(
            &evaluation.syntax_grammars,
            &evaluation.syntax_engine_preferences,
        )
        .map_err(|_| {
            runtime_candidate_error(
                "syntax.invalid_snapshot",
                "Runtime syntax grammar contributions failed server validation.",
            )
        })?;
        if let Some(set) = &evaluation.published_decoration_set {
            decorations::validate_decoration_set(set.document_version, set.clone(), None).map_err(
                |_| {
                    runtime_candidate_error(
                        "decorations.invalid_set",
                        "Runtime decoration set failed server validation.",
                    )
                },
            )?;
        }
        if let Some(set) = &evaluation.published_diagnostic_set {
            diagnostics::validate_diagnostic_set(set.document_version, set.clone(), None).map_err(
                |_| {
                    runtime_candidate_error(
                        "diagnostics.invalid_set",
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
            .bootstrap_state
            .workspace
            .lock()
            .await
            .open_document_refreshes(0)
            .await
            .map_err(|_| {
                runtime_candidate_error(
                    "runtime.reload_refresh_failed",
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
                design_tokens: Vec::new(),
            });
        let expected_design_system = self.active_design_system.lock().await.clone();
        let mut active_design_system = if let Some(selected) =
            evaluation.active_design_system.clone()
        {
            let packages = service
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            if selected.provenance.package_name != "core" {
                let is_valid = packages.enabled_records().any(|r| {
                    r.manifest.name == selected.provenance.package_name
                        && r.manifest.version == selected.provenance.package_version
                        && r.contributions.ui_design_system.is_some()
                });
                if is_valid {
                    selected
                } else {
                    crate::shell::design_system::ActiveDesignSystem::core_fallback(generation_id)
                }
            } else {
                selected
            }
        } else {
            let packages = service
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            if expected_design_system.provenance.package_name != "core" {
                let is_valid = packages.enabled_records().any(|r| {
                    r.manifest.name == expected_design_system.provenance.package_name
                        && r.manifest.version == expected_design_system.provenance.package_version
                        && r.contributions.ui_design_system.is_some()
                });
                if is_valid {
                    expected_design_system.clone()
                } else {
                    crate::shell::design_system::ActiveDesignSystem::core_fallback(generation_id)
                }
            } else {
                expected_design_system.clone()
            }
        };
        active_design_system.generation = generation_id;
        // Plan 112 (state table rows 6/7/13): resolve the active icon pack for
        // this generation. A freshly selected pack must still be enabled with
        // its iconPack contribution intact; a previously active pack that lost
        // its record (disable/remove/revoke) falls back to the bundled Regular
        // subset. The generation stamp makes stale snapshots rejectable.
        let expected_icon_pack = self.active_icon_pack.lock().await.clone();
        let mut active_icon_pack = if let Some(selected) = evaluation.active_icon_pack.clone() {
            let icon_pack_valid = {
                let packages = service
                    .package_service()
                    .lock()
                    .expect("package service mutex poisoned");
                packages.enabled_records().any(|r| {
                    r.manifest.name == selected.provenance.package_name
                        && r.manifest.version == selected.provenance.package_version
                        && r.contributions.icon_pack.is_some()
                })
            };
            icon_pack_valid.then_some(selected)
        } else if let Some(previous) = expected_icon_pack.clone() {
            let icon_pack_valid = {
                let packages = service
                    .package_service()
                    .lock()
                    .expect("package service mutex poisoned");
                packages.enabled_records().any(|r| {
                    r.manifest.name == previous.provenance.package_name
                        && r.manifest.version == previous.provenance.package_version
                        && r.contributions.icon_pack.is_some()
                })
            };
            icon_pack_valid.then_some(previous)
        } else {
            None
        };
        if let Some(active_icon_pack) = active_icon_pack.as_mut() {
            active_icon_pack.generation = generation_id;
        }
        let mut runtime_diagnostics = self.runtime_diagnostics.lock().await.snapshot();
        let package_ui = {
            let packages = service
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            evaluation
                .ui_contributions
                .wire_snapshot(generation_id, |provenance| {
                    packages
                        .enabled_records()
                        .find(|record| {
                            record.manifest.name == provenance.package_name
                                && record.manifest.version == provenance.package_version
                                && record.manifest.clay.api_prefix == provenance.api_prefix
                        })
                        .map_or(
                            crate::protocol::PackageUiTrustDomain::ThirdParty,
                            |record| match record.runtime_domain {
                                crate::packages::bundled::RuntimeDomain::Trusted => {
                                    crate::protocol::PackageUiTrustDomain::Trusted
                                }
                                crate::packages::bundled::RuntimeDomain::ThirdParty => {
                                    crate::protocol::PackageUiTrustDomain::ThirdParty
                                }
                            },
                        )
                })
        };
        let package_ui = match package_ui {
            Ok(snapshot) => snapshot,
            Err(ids) => {
                runtime_diagnostics.push(RuntimeDiagnostic::error(
                    "ui.pane_content_conflict",
                    format!(
                        "multiple empty-tab pane-content contributions: {}",
                        ids.join(", ")
                    ),
                ));
                evaluation
                    .ui_contributions
                    .wire_snapshot(generation_id, |_| {
                        crate::protocol::PackageUiTrustDomain::ThirdParty
                    })
                    .unwrap_or_default()
            }
        };
        let runtime_snapshot = build_runtime_state_snapshot(
            generation_id,
            &behavior,
            active_theme.clone(),
            active_typography.clone(),
            active_design_system.clone(),
            active_icon_pack.clone(),
            sdui.cloned_tree_or_default(),
            &open_documents,
            evaluation.published_decoration_set.clone(),
            evaluation.published_diagnostic_set.clone(),
            runtime_diagnostics,
            package_ui,
            self.enumerate_ui_choices(&service),
        )?;
        // Fail closed before commit when the complete snapshot cannot fit one
        // bounded IPC frame. Partial/live mutation must not begin.
        self.codec
            .encode_server_message(&ServerMessage::RuntimeStateSnapshot(Box::new(
                runtime_snapshot.clone(),
            )))
            .map_err(|_| {
                runtime_candidate_error(
                    "runtime.snapshot_too_large",
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
            expected_design_system,
            active_design_system,
            expected_icon_pack,
            active_icon_pack,
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

    pub(super) async fn commit_runtime_generation(
        &self,
        candidate: RuntimeGenerationCandidate,
    ) -> Result<Vec<ReloadedDocumentRefresh>, RuntimeDiagnostic> {
        let behavior_lock = self.scoped_locks.try_acquire().map_err(|_| {
            runtime_candidate_error(
                "runtime.behavior_locked",
                "Runtime behavior state is locked by another server operation.",
            )
        })?;
        if self.runtime_generation.generation_id().await != candidate.expected_generation_id {
            return Err(runtime_candidate_error(
                "runtime.generation_conflict",
                "Runtime generation changed before the prepared candidate could commit.",
            ));
        }

        let mut behavior = self.behavior.lock().await;
        let mut sdui = self.sdui.lock().await;
        let mut active_theme = self.active_theme.lock().await;
        let mut active_typography = self.runtime_generation.typography.current.lock().await;
        let mut active_design_system = self.active_design_system.lock().await;
        let mut active_icon_pack = self.active_icon_pack.lock().await;
        if *behavior != candidate.expected_behavior
            || *sdui != candidate.expected_sdui
            || *active_theme != candidate.expected_theme
            || *active_typography != candidate.expected_typography
            || *active_design_system != candidate.expected_design_system
            || *active_icon_pack != candidate.expected_icon_pack
        {
            return Err(runtime_candidate_error(
                "runtime.active_state_conflict",
                "Active runtime state changed before the prepared candidate could commit.",
            ));
        }

        let Some(evaluation) = candidate.generation.evaluation.as_deref() else {
            return Err(runtime_candidate_error(
                "runtime.incomplete_candidate",
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
        // The third-party worker survives trusted reloads; re-register its
        // live registrations under the new generation so the post-commit
        // generation cancel never withdraws them (Plan 061 task 12).
        let third_party_snapshot = candidate
            .generation
            .service
            .third_party_registrations_snapshot();
        register_runtime_contributions(
            candidate.generation.id,
            &candidate.generation.service,
            &third_party_snapshot,
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
        *active_design_system = candidate.active_design_system;
        *active_icon_pack = candidate.active_icon_pack;
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
            // Old trusted grants/process authority end at commit; the shared
            // third-party domain (workers, providers, language-server
            // sessions) survives a trusted reload (Plan 061 task 12). Cleanup
            // failure must not restore previous-generation executable
            // handlers.
            let _ = previous_generation
                .service
                .shutdown_trusted_generation_resources()
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
        drop(active_design_system);
        drop(active_typography);
        drop(active_theme);
        drop(sdui);
        drop(behavior);
        if typography_changed {
            self.runtime_generation
                .typography
                .updates
                .publish(candidate.active_typography);
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
        snapshots: Vec<workspace::OpenDocumentRefresh>,
    ) -> Vec<ReloadedDocumentRefresh> {
        let documents = {
            let workspace = self.bootstrap_state.workspace.lock().await;
            let mut documents = Vec::with_capacity(snapshots.len());
            for snapshot in snapshots {
                let Some(handle) = workspace.document_handle(snapshot.metadata.document_id) else {
                    continue;
                };
                documents.push((snapshot.metadata, handle));
            }
            documents
        };
        let mut refreshed = Vec::with_capacity(documents.len());
        for (metadata, handle) in documents {
            let mut messages = connection::open_document_followup_messages(
                &metadata,
                &handle,
                &self.behavior,
                &self.sdui,
                generation_id,
                service,
                &self.parse_coordinator,
            )
            .await;
            messages.extend(
                connection::start_document_analysis(
                    &self.document_analysis,
                    &self.bootstrap_state.workspace,
                    &self.behavior,
                    generation_id,
                    &metadata,
                    &handle,
                )
                .await,
            );
            refreshed.push(ReloadedDocumentRefresh {
                document_id: metadata.document_id,
                messages,
            });
        }
        refreshed
    }

    /// Plan 110 task 10: enumerate installable Settings selections from the
    /// enabled package inventory plus the persisted appearance preference —
    /// one inventory pass, no new scan, so the Settings dropdowns render from
    /// the snapshot the client already receives. Deterministic (sorted) so
    /// snapshot equality stays stable across reloads.
    fn enumerate_ui_choices(
        &self,
        service: &ClayJsRuntimeService,
    ) -> crate::protocol::UiChoicesSnapshot {
        let option =
            |record: &crate::packages::record::PackageRecord| crate::protocol::UiChoiceOption {
                specifier: record.manifest.name.clone(),
                display_name: record
                    .contributions
                    .ui_design_system
                    .as_ref()
                    .map(|ds| ds.display_name.clone()),
            };
        let package_service = service
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        let records: Vec<&crate::packages::record::PackageRecord> =
            package_service.enabled_records().collect();
        let mut themes: Vec<_> = records
            .iter()
            .filter(|record| record.manifest.name.starts_with("@clay/theme-"))
            .map(|record| option(record))
            .collect();
        let mut design_systems: Vec<_> = records
            .iter()
            .filter(|record| record.contributions.ui_design_system.is_some())
            .map(|record| option(record))
            .collect();
        // Plan 118 task 20: a bundled design-system package is selectable without
        // a prior `loadPackage` — `settings.setDesignSystem` accepts it and the
        // apply path enables the record on demand — so the panel must offer it,
        // not only enumerate what happens to be enabled. Without this pass the
        // shipped system is unreachable from the Settings dropdown on a fresh
        // install. Bundled manifests are read, never installed or enabled.
        for name in crate::packages::bundled::bundled_package_names() {
            if design_systems.iter().any(|option| option.specifier == name) {
                continue;
            }
            if let Some(display_name) =
                crate::packages::bundled::bundled_design_system_display_name(name)
            {
                design_systems.push(crate::protocol::UiChoiceOption {
                    specifier: name.to_string(),
                    display_name: Some(display_name),
                });
            }
        }
        themes.sort_by(|a, b| a.specifier.cmp(&b.specifier));
        design_systems.sort_by(|a, b| a.specifier.cmp(&b.specifier));
        // The built-in core baseline is always selectable and never a record.
        design_systems.insert(
            0,
            crate::protocol::UiChoiceOption {
                specifier: "@clay/core".to_string(),
                display_name: Some("Core baseline".to_string()),
            },
        );
        let appearance = self
            .effective_configuration_root()
            .and_then(|root| {
                crate::server::configuration::ConfigurationRuntime::from_config_root(&root).ok()
            })
            .map(|runtime| runtime.load_preferences().appearance)
            .and_then(|appearance| appearance)
            .map(|appearance| appearance.as_str().to_string());
        crate::protocol::UiChoicesSnapshot {
            themes,
            design_systems,
            appearance,
        }
    }
}
