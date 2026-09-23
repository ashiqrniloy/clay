//! Runtime family: SDUI actions, command intents (incl. settings persistence
//! and reload), generation-ack, completion and language-intelligence request
//! scheduling. Plan 090 task 2 extraction.

use std::sync::Arc;

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    packages::commands::CommandRegistry,
    perf::budgets::{COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES},
    protocol::{
        AgentClientCommand, AgentPickerKind, AgentServerMessage, BehaviorManifest, ClientId,
        CompletionProvenance, CompletionRequest, CompletionResultSet, CompletionStatus,
        CompletionTrigger, DocumentId, LanguageIntelligenceFeature, LanguageIntelligencePayload,
        LanguageIntelligenceResult, LanguageIntelligenceStatus, ProtocolErrorCode,
        SduiActionArgument, SduiActionIntent, SduiActionSource, SduiActionValue, ServerMessage,
        codec::CodecError, completion::estimated_result_payload_bytes,
    },
    server::{
        agent_picker::picker_kind_for_command,
        command_execution::{
            CONTROL_CENTER_COMMAND_ID, CommandExecutionRequest, CommandExecutionTarget,
            CommandExecutor, OPEN_PATH_BROWSER_COMMAND_ID,
        },
        completion::{
            CompletionProviderMeta, apply_exclusive_suppression, completion_prefix_matches,
            completion_recency_rank, score_completion_item,
        },
        document::DocumentState,
        language_intelligence::{
            LanguageIntelligenceCoordinatorError, LanguageIntelligenceDocumentWindow,
        },
        sdui::{StaticSduiState, sdui_action_response},
        workspace::WorkspaceState,
    },
};

use super::{
    ConnectionCtx,
    documents::{document_for_message, write_document_open_response},
    menus::open_command_centre_session,
    session_bound_message,
    workspace::workspace_command_result_message,
};

pub(super) async fn execute_command_intent(
    request: CommandExecutionRequest,
    workspace: Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    client_id: ClientId,
    reload_server: Option<&crate::server::IpcServer>,
    registry: &CommandRegistry,
) -> Option<ServerMessage> {
    let executor = CommandExecutor::new();

    if crate::server::command_execution::is_settings_command(&request.command_id) {
        // Phase 20.6: settings intents validate, then persist + reload so the
        // change applies live through the canonical apply path (persist →
        // reload → init.js re-eval + preferences apply → RuntimeStateSnapshot
        // fanout). `setTheme`/`setAppearance` carry their value as
        // `arguments.item_id`; `setTypography` carries one bounded complete
        // typography JSON argument that is revalidated before persistence.
        // `settings.reset` clears the persisted preferences store.
        let validated = match executor.execute_settings(request.clone()) {
            Ok(result) => result,
            Err(error) => {
                return Some(ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: format!(
                        "command execution rejected: {:?}: {}",
                        error.rule, error.message
                    ),
                });
            }
        };
        let client_projection = matches!(
            validated.command_id.as_str(),
            "settings.open" | "settings.close"
        );
        if let Some(server) = reload_server {
            match Box::pin(persist_settings_change(
                server,
                &validated.command_id,
                &request.arguments,
            ))
            .await
            {
                Ok(PersistOutcome::Reloaded(outcome)) => {
                    if !outcome.reloaded {
                        return outcome
                            .diagnostics
                            .into_iter()
                            .next()
                            .map(ServerMessage::RuntimeDiagnostic);
                    }
                    if let Some(diagnostic) = outcome.diagnostics.into_iter().next() {
                        return Some(ServerMessage::RuntimeDiagnostic(diagnostic));
                    }
                }
                Ok(PersistOutcome::Acknowledged) => {}
                Err(message) => {
                    return Some(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    });
                }
            }
        }
        return client_projection.then_some(ServerMessage::ShellClientCommandRequest {
            command_id: validated.command_id,
        });
    }

    // Phase 2 (plan 108 task 8): Coding Agent surface launch/close. The
    // intent is user-authorized (Command Centre catalogue entry or a declared
    // package action target); the toggle itself is client-local, so the
    // server answers with the shell-client request. Launching the surface
    // also makes its profile the book's active profile: prompts from the
    // surface must run with the coding tools, not the profile-less default
    // ("Chat"). Close leaves the book alone.
    if crate::server::command_execution::is_agent_surface_command(&request.command_id) {
        if request.command_id == "coding-agent.profile"
            && let Some(server) = reload_server
            && server
                .agent
                .profile_available(crate::server::command_execution::CODING_SURFACE_PROFILE_NAME)
                .await
        {
            // Resolve the connection's tab so the selection broadcast lands on
            // this tab's session instead of a session-less snapshot (which
            // wiped a live panel's transcript + branch).
            let tab = server
                .tab_registry
                .lock()
                .await
                .tab_for_client(client_id)
                .unwrap_or(client_id);
            server
                .agent
                .select_picker(
                    crate::protocol::AgentPickerKind::Agent,
                    crate::server::command_execution::CODING_SURFACE_PROFILE_ID,
                    Some(tab),
                )
                .await;
        }
        return Some(ServerMessage::ShellClientCommandRequest {
            command_id: request.command_id,
        });
    }

    if crate::server::command_execution::is_reload_command(&request.command_id) {
        let Some(server) = reload_server else {
            return Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: "runtime reload service is unavailable".to_string(),
            });
        };
        return match Box::pin(server.execute_reload_command(request)).await {
            Ok(outcome) if outcome.reloaded => {
                if let Some(diagnostic) = outcome.diagnostics.into_iter().next() {
                    Some(ServerMessage::RuntimeDiagnostic(diagnostic))
                } else {
                    Some(ServerMessage::RuntimeDiagnostic(
                        crate::protocol::RuntimeDiagnostic {
                            severity: crate::protocol::DiagnosticSeverity::Info,
                            code: "runtime.reload_succeeded".to_string(),
                            message: format!(
                                "Runtime configuration reloaded as generation {}.",
                                outcome.active_generation_id
                            ),
                        },
                    ))
                }
            }
            Ok(outcome) => outcome
                .diagnostics
                .into_iter()
                .next()
                .map(ServerMessage::RuntimeDiagnostic),
            Err(error) => Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!(
                    "command execution rejected: {:?}: {}",
                    error.rule, error.message
                ),
            }),
        };
    }

    if crate::server::command_execution::is_workspace_command(&request.command_id) {
        let result = {
            let mut workspace_guard = workspace.lock().await;
            executor
                .execute_workspace(registry, &mut workspace_guard, client_id, request)
                .await
        };
        match result {
            Ok(result) => {
                workspace_command_result_message(
                    result,
                    &workspace,
                    document,
                    sdui,
                    client_id,
                    reload_server,
                )
                .await
            }
            Err(error) => Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!(
                    "command execution rejected: {:?}: {}",
                    error.rule, error.message
                ),
            }),
        }
    } else {
        executor
            .execute(registry, request)
            .err()
            .map(|error| ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!(
                    "command execution rejected: {:?}: {}",
                    error.rule, error.message
                ),
            })
    }
}

pub(super) enum PersistOutcome {
    /// Preference persisted and the runtime reloaded; the reload outcome is
    /// forwarded so the caller can surface any reload diagnostic.
    Reloaded(crate::server::RuntimeReloadOutcome),
    /// Command acknowledged without persistence (`settings.open`/`settings.close`).
    Acknowledged,
}

/// Persist a settings command to `preferences.json` and trigger a runtime
/// reload so the change applies live through the canonical apply path. Returns
/// `Acknowledged` for commands that do not carry a persistable value.
/// `settings.reset` clears the store and reloads.
pub(super) async fn persist_settings_change(
    server: &crate::server::IpcServer,
    command_id: &str,
    arguments: &serde_json::Value,
) -> Result<PersistOutcome, String> {
    // Plan 134 P3: `effective_configuration_root` (canonicalize + is_file) and
    // the preferences read/write are blocking std::fs; run the whole root
    // resolution and persistence step on Tokio's blocking pool so a slow/cold
    // filesystem cannot stall the connection reactor.
    let blocking_server = server.clone();
    let command_id = command_id.to_string();
    let arguments = arguments.clone();
    let should_reload = tokio::task::spawn_blocking(move || -> Result<bool, String> {
        use crate::server::configuration::ConfigurationRuntime;
        let Some(config_root) = blocking_server.effective_configuration_root() else {
            return Err("settings persistence requires an active configuration root".to_string());
        };
        let runtime = ConfigurationRuntime::from_config_root(&config_root)
            .map_err(|error| format!("settings persistence root error: {error}"))?;
        match command_id.as_str() {
            "settings.setTheme" => {
                let value = settings_value(&arguments).ok_or_else(|| {
                    "settings.setTheme requires an item_id/specifier argument".to_string()
                })?;
                runtime
                    .persist_preference("theme", serde_json::Value::String(value))
                    .map(|_| true)
                    .map_err(|error| format!("settings.setTheme persistence failed: {error}"))
            }
            "settings.setAppearance" => {
                let value = settings_value(&arguments).ok_or_else(|| {
                    "settings.setAppearance requires an item_id/appearance argument".to_string()
                })?;
                runtime
                    .persist_preference("appearance", serde_json::Value::String(value))
                    .map(|_| true)
                    .map_err(|error| format!("settings.setAppearance persistence failed: {error}"))
            }
            "settings.setDesignSystem" => {
                let value = settings_value(&arguments).ok_or_else(|| {
                    "settings.setDesignSystem requires an item_id/specifier argument".to_string()
                })?;
                runtime
                    .persist_preference("designSystem", serde_json::Value::String(value))
                    .map(|_| true)
                    .map_err(|error| {
                        format!("settings.setDesignSystem persistence failed: {error}")
                    })
            }
            "settings.setTypography" => {
                let raw = arguments
                    .as_object()
                    .and_then(|object| object.get("typography"))
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        "settings.setTypography requires a complete typography argument".to_string()
                    })?;
                let value: serde_json::Value = serde_json::from_str(raw).map_err(|_| {
                    "settings.setTypography typography is not valid JSON".to_string()
                })?;
                crate::server::ops::typography::validate_typography_request(&value)?;
                runtime
                    .persist_preference("typography", value)
                    .map(|_| true)
                    .map_err(|error| format!("settings.setTypography persistence failed: {error}"))
            }
            "settings.reset" => runtime
                .clear_preferences()
                .map(|_| true)
                .map_err(|error| format!("settings.reset failed: {error}")),
            _ => Ok(false),
        }
    })
    .await
    .map_err(|_| "settings persistence task failed".to_string())??;
    if should_reload {
        // Plan 129 P4: box the reload future so the settings-persistence,
        // command-intent, and connection-loop futures that await this helper
        // stay small (clippy::large_futures).
        let outcome = Box::pin(server.reload_runtime_generation()).await;
        Ok(PersistOutcome::Reloaded(outcome))
    } else {
        Ok(PersistOutcome::Acknowledged)
    }
}

pub(super) fn settings_value(arguments: &serde_json::Value) -> Option<String> {
    arguments
        .as_object()
        .and_then(|object| {
            object
                .get("item_id")
                .or_else(|| object.get("specifier"))
                .or_else(|| object.get("appearance"))
        })
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

fn intent_text(intent: &SduiActionIntent) -> String {
    intent
        .arguments
        .iter()
        .find(|argument| argument.name == "value" || argument.name == "text")
        .and_then(|argument| match &argument.value {
            SduiActionValue::String(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Optional plan 109 I4 argument: the prompt's portable thinking level.
/// Absent/non-string keeps the session's current effort.
fn intent_thinking_level(intent: &SduiActionIntent) -> Option<String> {
    intent
        .arguments
        .iter()
        .find(|argument| argument.name == "thinkingLevel")
        .and_then(|argument| match &argument.value {
            SduiActionValue::String(level) => Some(level.clone()),
            _ => None,
        })
}

/// Host-owned agent controls: the agent surface's composer is rendered by the
/// host, not by a package-declared SDUI node, so its submit/cancel/steer
/// intents are authorized by the active tab session below. Plan 118: the ids
/// carry agent naming (they were `chat.*` while the chat landing owned the
/// composer); the transport is their single owner.
fn is_agent_run_action(command_id: &str) -> bool {
    matches!(command_id, "agent.submit" | "agent.cancel" | "agent.steer")
}

pub(super) fn sdui_command_request(intent: &SduiActionIntent) -> CommandExecutionRequest {
    CommandExecutionRequest {
        command_id: intent.command_id.clone(),
        arguments: sdui_action_arguments_json(&intent.arguments, &intent.source),
        target: CommandExecutionTarget::Global,
        provenance: None,
        expected_permissions: Vec::new(),
    }
}

/// Phase 20.6: forward the originating `SduiActionSource` so command handlers
/// receive the selected list/dropdown item id (`arguments.item_id`) or the
/// originating node id (`arguments.node_id`). Package component declarations
/// carry no argument data, so without this the choice value never reaches the
/// handler. Additive: handlers that ignore `arguments` are unaffected.
pub(super) fn sdui_action_arguments_json(
    arguments: &[SduiActionArgument],
    source: &SduiActionSource,
) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for argument in arguments {
        object.insert(
            argument.name.clone(),
            sdui_action_value_json(&argument.value),
        );
    }
    match source {
        SduiActionSource::ListItem { item_id, .. } => {
            object
                .entry("item_id".to_string())
                .or_insert_with(|| serde_json::Value::String(item_id.clone()));
        }
        SduiActionSource::Button { node_id } => {
            object
                .entry("node_id".to_string())
                .or_insert_with(|| serde_json::Value::String(node_id.0.to_string()));
        }
    }
    serde_json::Value::Object(object)
}

pub(super) fn sdui_action_value_json(value: &SduiActionValue) -> serde_json::Value {
    match value {
        SduiActionValue::String(value) => serde_json::Value::String(value.clone()),
        SduiActionValue::Bool(value) => serde_json::Value::Bool(*value),
        SduiActionValue::I64(value) => serde_json::Value::Number((*value).into()),
        SduiActionValue::U64(value) => serde_json::Value::Number((*value).into()),
    }
}

pub(super) fn empty_language_intelligence_payload(
    feature: LanguageIntelligenceFeature,
) -> LanguageIntelligencePayload {
    match feature {
        LanguageIntelligenceFeature::Hover => {
            LanguageIntelligencePayload::Hover(crate::protocol::HoverResult {
                range: None,
                markdown: String::new(),
            })
        }
        LanguageIntelligenceFeature::GoToDefinition => {
            LanguageIntelligencePayload::GoToDefinition(crate::protocol::GoToDefinitionResult {
                locations: Vec::new(),
            })
        }
        LanguageIntelligenceFeature::CodeAction => {
            LanguageIntelligencePayload::CodeAction(crate::protocol::CodeActionResult {
                actions: Vec::new(),
            })
        }
        LanguageIntelligenceFeature::SignatureHelp => {
            LanguageIntelligencePayload::SignatureHelp(crate::protocol::SignatureHelpResult {
                signatures: Vec::new(),
                active_signature: None,
                active_parameter: None,
            })
        }
    }
}

pub(super) fn completion_document_window(
    request: &CompletionRequest,
    document: &DocumentState,
    package_prefix: &str,
) -> crate::server::completion::CompletionDocumentWindow {
    use crate::perf::budgets::COMPLETION_DOCUMENT_WINDOW_BUDGET_BYTES;

    // Rope window: costs the window, never the document (Plan 126 D1).
    let (byte_start, byte_end, text) = document.window_around(
        request.cursor_byte_offset,
        COMPLETION_DOCUMENT_WINDOW_BUDGET_BYTES,
    );
    crate::server::completion::CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: package_prefix.to_string(),
        byte_start,
        byte_end,
        text,
    }
}

pub(super) fn language_intelligence_document_window_for_behavior(
    request: &crate::protocol::LanguageIntelligenceRequest,
    document: &DocumentState,
    behavior: &BehaviorManifest,
) -> LanguageIntelligenceDocumentWindow {
    let manifest_id = &behavior.manifest_id;
    language_intelligence_document_window(
        request,
        document,
        manifest_id.rsplit('.').next().unwrap_or(manifest_id),
    )
}

pub(super) fn language_intelligence_document_window(
    request: &crate::protocol::LanguageIntelligenceRequest,
    document: &DocumentState,
    active_mode: &str,
) -> LanguageIntelligenceDocumentWindow {
    use crate::perf::budgets::LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES;

    // Rope window: costs the window, never the document (Plan 126 D1).
    let (byte_start, byte_end, text) = document.window_around(
        request.cursor_byte_offset,
        LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES,
    );

    LanguageIntelligenceDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        byte_start,
        byte_end,
        text,
        active_mode: active_mode.to_string(),
    }
}

/// Static (package-declared) completion for `request`'s replacement range.
/// `replacement_text` is the document text covered by
/// `request.replacement_range` — callers never materialize the document, so
/// this reads only those bytes (Plan 126 D1).
pub(super) fn static_package_completion_result(
    request: &CompletionRequest,
    manifest_id: &str,
    replacement_text: &str,
    providers: &[CompletionProviderMeta],
) -> Option<CompletionResultSet> {
    let package_prefix = manifest_id.split('.').next()?;
    let mut matched: Vec<_> = providers
        .iter()
        .filter(|provider| {
            provider.provenance.package_prefix == package_prefix
                && match &request.trigger {
                    CompletionTrigger::Manual => true,
                    CompletionTrigger::Character(character) => provider
                        .trigger_metadata
                        .trigger_characters
                        .iter()
                        .any(|trigger| trigger == character),
                }
        })
        .collect();
    matched.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
    apply_exclusive_suppression(&mut matched);
    let provenance = matched.first()?.provenance.clone();
    let prefix = replacement_text;
    let mut result = CompletionResultSet {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        replacement_range: request.replacement_range,
        status: CompletionStatus::Empty,
        items: Vec::new(),
        provenance,
    };

    let mut candidates = Vec::new();
    for provider in matched {
        let mut provider_items: Vec<_> = provider
            .items
            .iter()
            .filter(|item| completion_prefix_matches(&item.insert_text, prefix))
            .collect();
        provider_items.sort_by(|left, right| {
            score_completion_item(
                &right.label,
                prefix,
                completion_recency_rank(&right.insert_text, &request.recent_completions),
            )
            .cmp(&score_completion_item(
                &left.label,
                prefix,
                completion_recency_rank(&left.insert_text, &request.recent_completions),
            ))
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.insert_text.cmp(&right.insert_text))
        });
        candidates.extend(
            provider_items
                .into_iter()
                .take(provider.max_items)
                .map(|item| (provider, item)),
        );
    }
    candidates.sort_by(|(left_provider, left), (right_provider, right)| {
        right_provider
            .priority
            .cmp(&left_provider.priority)
            .then_with(|| {
                score_completion_item(
                    &right.label,
                    prefix,
                    completion_recency_rank(&right.insert_text, &request.recent_completions),
                )
                .cmp(&score_completion_item(
                    &left.label,
                    prefix,
                    completion_recency_rank(&left.insert_text, &request.recent_completions),
                ))
            })
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.insert_text.cmp(&right.insert_text))
            .then_with(|| left_provider.id.cmp(&right_provider.id))
    });

    for (_, item) in candidates {
        let mut candidate = result.clone();
        candidate.items.push(item.clone());
        if candidate.items.len() > COMPLETION_RESULT_MAX_ITEMS
            || estimated_result_payload_bytes(&candidate) > COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES
        {
            break;
        }
        result.items.push(item.clone());
    }
    if !result.items.is_empty() {
        result.status = CompletionStatus::Ok;
    }
    Some(result)
}

// ---------- coordinator loop handlers (Plan 090 task 2 extraction) ----------

pub(super) async fn handle_runtime_generation_installed<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    ack_client_id: ClientId,
    runtime_generation_id: u64,
) where
    S: AsyncWrite + Unpin,
{
    let _ = ctx
        .runtime_generation
        .note_runtime_generation_installed(ack_client_id, ctx.client_id, runtime_generation_id)
        .await;
}

/// A second `Hello` on an established connection is a client protocol error:
/// answer with one bound diagnostic and keep serving (the identity boundary
/// and handshake already ran).
pub(super) async fn handle_duplicate_hello<S>(
    ctx: &mut ConnectionCtx<'_, S>,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    ctx.codec
        .write_server_message(
            ctx.stream,
            &ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: "duplicate Hello message".to_string(),
            },
        )
        .await?;
    Ok(())
}

pub(super) async fn handle_sdui_action<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    ui_version: u64,
    intent: SduiActionIntent,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let codec = ctx.codec;
    let stream: &mut S = &mut *ctx.stream;
    let sdui = ctx.sdui;
    let workspace = &*ctx.workspace;
    let document = &*ctx.document;
    let behavior = ctx.behavior;
    let runtime_generation = ctx.runtime_generation;
    let parse_coordinator = ctx.parse_coordinator;
    let document_analysis = ctx.document_analysis;
    let menu_sessions = &mut *ctx.menu_sessions;
    let tab_registry = ctx.tab_registry;
    let reload_server = ctx.reload_server;
    let client_id = ctx.client_id;
    let bound_tab_id = *ctx.bound_tab_id;
    let package_action = runtime_generation
        .latest_runtime_snapshot_for(client_id)
        .await
        .is_some_and(|snapshot| {
            snapshot
                .package_ui
                .allows_action(ui_version, &intent.command_id)
        });
    // Plan 109 review fix: panel-rendered (non-SDUI) surfaces send client
    // intents through the same action lane. Two classes must bypass SDUI-tree
    // validation: (1) Command Centre picker opens — the picker branch below
    // answers with a server-scoped TransientMenuSnapshot (composer /model and
    // /resume intercepts + the Files-tab Resume session button);
    // (2) client-UI commands the command lane already trusts (native file /
    // folder dialogs) — they answer with a ShellClientCommandRequest that the
    // shell dispatches locally. Without this, the panel buttons surfaced
    // `invalid SDUI message: UnknownActionCommand(...)` and no picker opened.
    let manifest_client_ui = {
        let document_id = document.lock().await.document_id();
        manifest_allows_client_ui(
            behavior.lock().await.manifest_for(document_id),
            &intent.command_id,
        )
    };
    if !package_action
        && !is_agent_run_action(&intent.command_id)
        && picker_kind_for_command(&intent.command_id).is_none()
        && !manifest_client_ui
    {
        let validation_response = {
            let state = sdui.lock().await;
            if state.cloned_tree_or_default().ui_version != ui_version {
                Some(ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: "SDUI action version is stale".to_string(),
                })
            } else {
                sdui_action_response(&state, &intent)
            }
        };
        if let Some(response) = validation_response {
            codec.write_server_message(stream, &response).await?;
            return Ok(());
        }
    }

    if is_agent_run_action(&intent.command_id) {
        if let Some(host) = reload_server.map(|server| &server.agent) {
            let tab = bound_tab_id.unwrap_or(client_id);
            let message = if intent.command_id == "agent.cancel" {
                host.cancel_tab(tab).await
            } else if intent.command_id == "agent.steer" {
                host.steer_tab(tab, &intent_text(&intent)).await
            } else {
                host.begin_prompt_with_effort(
                    tab,
                    &intent_text(&intent),
                    intent_thinking_level(&intent),
                )
                .await
            };
            eprintln!(
                "[agent] sdui {} from client {client_id} tab {tab:?} -> {}",
                intent.command_id,
                match &message {
                    AgentServerMessage::Snapshot(_) => "snapshot".to_string(),
                    AgentServerMessage::Event { .. } => "event".to_string(),
                    AgentServerMessage::Inventory(_) => "inventory".to_string(),
                    AgentServerMessage::Picker { .. } => "picker".to_string(),
                    AgentServerMessage::Diagnostic { code, .. } => format!("diag:{code}"),
                    _ => "other".to_string(),
                }
            );
            // Plan 119 SC-6: a prompt is what creates the tab's session, and
            // the run's events are session-tagged. The binding is written on
            // this connection *first* (before the answer's own snapshot and
            // long before the daemon's first event), so the store has adopted
            // its session by the time any of them arrive.
            let session_id = host.tab_session_id(tab).await.unwrap_or_default();
            let bound = session_bound_message(client_id, tab, &session_id);
            codec
                .write_server_message(stream, &ServerMessage::Agent(Box::new(bound)))
                .await?;
            codec
                .write_server_message(stream, &ServerMessage::Agent(Box::new(message)))
                .await?;
        }
        return Ok(());
    }
    if picker_kind_for_command(&intent.command_id).is_some() {
        match open_command_centre_session(
            &intent.command_id,
            menu_sessions,
            behavior,
            runtime_generation,
            document,
            workspace,
            tab_registry,
            bound_tab_id,
            reload_server.map(|server| &server.agent),
        )
        .await
        {
            Ok((replaced_id, snapshot)) => {
                if let Some(replaced_id) = replaced_id {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::TransientMenuClosed {
                                session_id: replaced_id,
                            },
                        )
                        .await?;
                }
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::TransientMenuSnapshot(Box::new(snapshot)),
                    )
                    .await?;
            }
            Err(message) => {
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        },
                    )
                    .await?;
            }
        }
        return Ok(());
    }
    let response = execute_command_intent(
        sdui_command_request(&intent),
        Arc::clone(workspace),
        document,
        sdui,
        client_id,
        reload_server,
        &CommandRegistry::new(),
    )
    .await;
    if let Some(response) = response {
        write_document_open_response(
            &codec,
            stream,
            response,
            behavior,
            runtime_generation,
            workspace,
            sdui,
            parse_coordinator,
            document_analysis,
            client_id,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn handle_command_intent<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    document_id: DocumentId,
    behavior_version: crate::protocol::BehaviorVersion,
    command_id: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let codec = ctx.codec;
    let stream: &mut S = &mut *ctx.stream;
    let menu_sessions = &mut *ctx.menu_sessions;
    let behavior = ctx.behavior;
    let runtime_generation = ctx.runtime_generation;
    let document = &*ctx.document;
    let workspace = &*ctx.workspace;
    let sdui = ctx.sdui;
    let tab_registry = ctx.tab_registry;
    let reload_server = ctx.reload_server;
    let client_id = ctx.client_id;
    let bound_tab_id = *ctx.bound_tab_id;
    // Commands never receive previous-generation grace. The gate protects
    // manifest-coupled routing below (client-UI + document commands): the
    // client must not act on a stale manifest view. Server-owned catalogue
    // commands (Control Centre, Path Browser, agent pickers) re-resolve
    // everything server-side at open time, so a stale client version is
    // harmless there — and rejecting them silently bricked the chord
    // whenever the client's version lagged a manifest publish.
    if command_id == CONTROL_CENTER_COMMAND_ID
        || command_id == OPEN_PATH_BROWSER_COMMAND_ID
        || picker_kind_for_command(&command_id).is_some()
    {
        match open_command_centre_session(
            &command_id,
            menu_sessions,
            behavior,
            runtime_generation,
            document,
            workspace,
            tab_registry,
            bound_tab_id,
            reload_server.map(|server| &server.agent),
        )
        .await
        {
            Ok((replaced_id, snapshot)) => {
                if let Some(replaced_id) = replaced_id {
                    codec
                        .write_server_message(
                            stream,
                            &ServerMessage::TransientMenuClosed {
                                session_id: replaced_id,
                            },
                        )
                        .await?;
                }
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::TransientMenuSnapshot(Box::new(snapshot)),
                    )
                    .await?;
            }
            Err(message) => {
                codec
                    .write_server_message(
                        stream,
                        &ServerMessage::Error {
                            code: ProtocolErrorCode::InvalidMessage,
                            message,
                        },
                    )
                    .await?;
            }
        }
        return Ok(());
    }
    // Everything below couples to the client's manifest view — previous-
    // generation intents are rejected (the client surfaces the error).
    if behavior.lock().await.version() != behavior_version {
        codec
            .write_server_message(
                stream,
                &ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: "command intent behavior version is stale".to_string(),
                },
            )
            .await?;
        return Ok(());
    }
    let is_client_ui =
        manifest_allows_client_ui(behavior.lock().await.manifest_for(document_id), &command_id);
    if is_client_ui {
        codec
            .write_server_message(
                stream,
                &ServerMessage::ShellClientCommandRequest { command_id },
            )
            .await?;
        return Ok(());
    }
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id,
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::ActiveDocument { document_id },
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(workspace),
        document,
        sdui,
        client_id,
        reload_server,
        &CommandRegistry::new(),
    )
    .await;
    if let Some(response) = response {
        codec.write_server_message(stream, &response).await?;
    }
    Ok(())
}

fn manifest_allows_client_ui(manifest: &BehaviorManifest, command_id: &str) -> bool {
    if matches!(
        command_id,
        "documents.clientOpenFileDialog" | "workspace.clientOpenFolderDialog"
    ) {
        return true;
    }
    manifest.commands.iter().any(|command| {
        command.command_id == command_id
            && command.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
    })
}

pub(super) async fn handle_completion_request<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    request: &mut CompletionRequest,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let codec = ctx.codec;
    let stream: &mut S = &mut *ctx.stream;
    let behavior = ctx.behavior;
    let runtime_generation = ctx.runtime_generation;
    let document = &*ctx.document;
    let workspace = &*ctx.workspace;
    let completion = ctx.completion;
    let document_analysis = ctx.document_analysis;
    let completion_tx = ctx.completion_tx;
    let dropped_results = ctx.dropped_results;
    let client_id = ctx.client_id;
    request.client_id = client_id;
    if let Err(rejection) = request.validate() {
        codec
            .write_server_message(
                stream,
                &ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: format!("completion request rejected: {rejection:?}"),
                },
            )
            .await?;
        return Ok(());
    }
    let Some(target_document) =
        document_for_message(request.document_id, client_id, document, workspace).await
    else {
        codec
            .write_server_message(
                stream,
                &ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: "completion document is not authorized for this connection"
                        .to_string(),
                },
            )
            .await?;
        return Ok(());
    };
    let manifest_id = behavior
        .lock()
        .await
        .manifest_for(request.document_id)
        .manifest_id
        .clone();
    let package_prefix = manifest_id.split('.').next().unwrap_or("");
    let providers = runtime_generation
        .current()
        .await
        .service
        .completion_providers();
    let analysis_provider_ids =
        document_analysis.active_completion_provider_ids(request.document_id);
    let dynamic_provider = completion.providers().into_iter().find(|provider| {
        (provider.provenance.package_prefix == package_prefix
            || analysis_provider_ids.contains(&provider.id))
            && match &request.trigger {
                CompletionTrigger::Manual => true,
                CompletionTrigger::Character(character) => provider
                    .trigger_metadata
                    .trigger_characters
                    .iter()
                    .any(|trigger| trigger == character),
            }
    });
    // Provider matching runs before any text access: the document lock is held
    // only to read the replacement range and, when a JS provider matched, one
    // bounded window (Plan 126 D1).
    let (fallback, window) = {
        let document = target_document.lock().await;
        let fallback = document
            .text_range(
                request.replacement_range.byte_start,
                request.replacement_range.byte_end,
            )
            .and_then(|replacement_text| {
                static_package_completion_result(
                    request,
                    &manifest_id,
                    &replacement_text,
                    &providers,
                )
            })
            .unwrap_or_else(|| CompletionResultSet {
                request_id: request.request_id,
                client_id: request.client_id,
                document_id: request.document_id,
                document_version: request.document_version,
                behavior_version: request.behavior_version,
                provider_generation: request.provider_generation,
                replacement_range: request.replacement_range,
                status: CompletionStatus::Empty,
                items: Vec::new(),
                provenance: CompletionProvenance::builtin_core(),
            });
        let window = dynamic_provider.as_ref().map(|provider| {
            completion_document_window(request, &document, &provider.provenance.package_prefix)
        });
        (fallback, window)
    };
    if let Some((provider, window)) = dynamic_provider.zip(window) {
        request.provider_generation = provider.generation;
        if let Ok(reply_rx) = completion.schedule_completion(&provider.id, request.clone(), window)
        {
            let tx = completion_tx.clone();
            let dropped = std::sync::Arc::clone(dropped_results);
            tokio::spawn(async move {
                let message = match tokio::time::timeout(
                    std::time::Duration::from_millis(provider.timeout_ms.saturating_add(50)),
                    reply_rx,
                )
                .await
                {
                    Ok(Ok(result)) => ServerMessage::CompletionResult { result },
                    // Provider timeout/failure/supersede: fall back to the
                    // static result so the client never waits on a dropped
                    // request-scoped reply.
                    _ => ServerMessage::CompletionResult { result: fallback },
                };
                if tx.try_send(message).is_err() {
                    let count = dropped
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        .saturating_add(1);
                    eprintln!(
                        "clay server: completion result lane full; dropped {count} result(s)"
                    );
                }
            });
            return Ok(());
        }
    }

    codec
        .write_server_message(
            stream,
            &ServerMessage::CompletionResult { result: fallback },
        )
        .await?;
    Ok(())
}

pub(super) async fn handle_language_intelligence_request<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    request: &mut crate::protocol::LanguageIntelligenceRequest,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let codec = ctx.codec;
    let stream: &mut S = &mut *ctx.stream;
    let behavior = ctx.behavior;
    let document = &*ctx.document;
    let workspace = &*ctx.workspace;
    let language_intelligence = ctx.language_intelligence;
    let language_intelligence_tx = ctx.language_intelligence_tx;
    let dropped_results = ctx.dropped_results;
    let client_id = ctx.client_id;
    // Stamp the connection's client identity; ignore any client-supplied
    // client_id so results cannot be forged across clients.
    request.client_id = client_id;
    if let Err(rejection) = request.validate() {
        codec
            .write_server_message(
                stream,
                &ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: format!("language-intelligence request rejected: {rejection:?}"),
                },
            )
            .await?;
        return Ok(());
    }

    let Some(target_document) =
        document_for_message(request.document_id, client_id, document, workspace).await
    else {
        codec
            .write_server_message(
                stream,
                &ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: "language-intelligence document is not authorized for this connection"
                        .to_string(),
                },
            )
            .await?;
        return Ok(());
    };
    let window = {
        // Clone the manifest under the behavior lock only; the document lock is
        // never nested behind it.
        let manifest = {
            let behavior = behavior.lock().await;
            behavior.manifest_for(request.document_id).clone()
        };
        let document = target_document.lock().await;
        language_intelligence_document_window_for_behavior(request, &document, &manifest)
    };
    match language_intelligence.schedule(None, request.clone(), window) {
        Ok(reply_rx) => {
            let tx = language_intelligence_tx.clone();
            let dropped = std::sync::Arc::clone(dropped_results);
            tokio::spawn(async move {
                match reply_rx.await {
                    Ok(result) => {
                        if tx
                            .try_send(ServerMessage::LanguageIntelligenceResult { result })
                            .is_err()
                        {
                            let count = dropped
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                                .saturating_add(1);
                            eprintln!(
                                "clay server: language-intelligence result lane full; dropped {count} result(s)"
                            );
                        }
                    }
                    Err(_canceled) => {
                        // Stale/canceled work drops silently so a newer
                        // cursor/edit request can replace it without a late
                        // empty/error flash.
                    }
                }
            });
        }
        Err(LanguageIntelligenceCoordinatorError::NoProviderForFeature) => {
            let empty_payload = empty_language_intelligence_payload(request.feature);
            let result = LanguageIntelligenceResult {
                request_id: request.request_id,
                client_id: request.client_id,
                document_id: request.document_id,
                document_version: request.document_version,
                behavior_version: request.behavior_version,
                provider_generation: request.provider_generation,
                feature: request.feature,
                status: LanguageIntelligenceStatus::Empty,
                payload: empty_payload,
                provenance: CompletionProvenance::builtin_core(),
            };
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::LanguageIntelligenceResult { result },
                )
                .await?;
        }
        Err(LanguageIntelligenceCoordinatorError::OutstandingRequestLimit { .. }) => {
            let result = LanguageIntelligenceResult {
                request_id: request.request_id,
                client_id: request.client_id,
                document_id: request.document_id,
                document_version: request.document_version,
                behavior_version: request.behavior_version,
                provider_generation: request.provider_generation,
                feature: request.feature,
                status: LanguageIntelligenceStatus::ProviderError,
                payload: empty_language_intelligence_payload(request.feature),
                provenance: CompletionProvenance::builtin_core(),
            };
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::LanguageIntelligenceResult { result },
                )
                .await?;
        }
        Err(error) => {
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message: format!("language-intelligence schedule rejected: {error}"),
                    },
                )
                .await?;
        }
    }
    Ok(())
}

/// Plan 109/117: coding-agent panel messages. Tab-scoped commands (picker
/// selection, worker model, resume, mount STATE, recent sessions) resolve the
/// connection's tab through the registry and apply through the agent book
/// paths; everything else dispatches to the host unchanged. A connection with
/// no agent host attached gets one bounded diagnostic.
pub(super) async fn handle_agent_command<S>(
    ctx: &mut ConnectionCtx<'_, S>,
    command: Box<AgentClientCommand>,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let codec = ctx.codec;
    let mut stream: &mut S = &mut *ctx.stream;
    let client_id = ctx.client_id;
    let reload_server = ctx.reload_server;
    if let Some(server) = reload_server.as_ref() {
        // Plan 109 I3: a panel model/provider/agent selection
        // (dropdown) applies through the same book path as the
        // Command Centre picker, with the connection's bound tab
        // so the per-workspace selection (I2) is written.
        match &*command {
            AgentClientCommand::Select { kind, id }
                if matches!(
                    kind,
                    AgentPickerKind::Model | AgentPickerKind::Provider | AgentPickerKind::Agent
                ) =>
            {
                // `unwrap_or(client_id)` is the same tab the
                // panel's own mount (TabState) resolves, so the
                // selection lands on the tab its STATE belongs to.
                let tab = server
                    .tab_registry
                    .lock()
                    .await
                    .tab_for_client(client_id)
                    .unwrap_or(client_id);
                server.agent.select_picker(*kind, id, Some(tab)).await;
            }
            // Plan 109 I8: OM worker model selection — the same
            // per-workspace book path plus the daemon's
            // per-session `session.om.set`.
            AgentClientCommand::SelectWorker {
                worker,
                id,
                session_id,
            } => {
                // Same tab resolution as the panel's own mount: a
                // bare `tab_for_client` can be `None` while the
                // panel still holds this tab's session, which sent
                // the book broadcast at a session-less snapshot and
                // wiped the Memory tab.
                let tab = server
                    .tab_registry
                    .lock()
                    .await
                    .tab_for_client(client_id)
                    .unwrap_or(client_id);
                server
                    .agent
                    .select_worker(*worker, id, session_id.clone(), Some(tab))
                    .await;
            }
            // Plan 117: the panel's resume — the same rich
            // load the picker uses (full transcript + trio +
            // tab rebind), tab-resolved client-side then
            // broadcast so the view's relay applies it. The
            // old dispatch path had no tab (no rebind) and
            // broadcast an entry-less snapshot that wiped the
            // restored transcript.
            AgentClientCommand::ResumeSession { session_id } => {
                let tab = server
                    .tab_registry
                    .lock()
                    .await
                    .tab_for_client(client_id)
                    .unwrap_or(client_id);
                let snapshot = server.agent.resume_tab(tab, session_id, None).await;
                server.agent.broadcast(snapshot);
            }
            // Plan 117 follow-up: the coding-agent pane's mount
            // STATE — the tab's branch + daemon environment
            // (skills, MCP outcomes) before any prompt, so the
            // status row and the pinned cards are populated on a
            // freshly opened surface.
            AgentClientCommand::TabState => {
                let tab = server
                    .tab_registry
                    .lock()
                    .await
                    .tab_for_client(client_id)
                    .unwrap_or(client_id);
                let snapshot = server.agent.tab_state_snapshot(tab).await;
                // Written on this connection before the broadcast
                // snapshot, so the tab's store has adopted its
                // session by the time the snapshot's STATE and
                // MESSAGES events arrive.
                let bound = session_bound_message(client_id, tab, &snapshot.session_id);
                codec
                    .write_server_message(&mut stream, &ServerMessage::Agent(Box::new(bound)))
                    .await?;
                server
                    .agent
                    .broadcast(AgentServerMessage::Snapshot(snapshot));
            }
            // Plan 117: the panel's recent-sessions list —
            // labeled, workspace-scoped, bounded.
            AgentClientCommand::ResumableSessions => {
                let tab = server
                    .tab_registry
                    .lock()
                    .await
                    .tab_for_client(client_id)
                    .unwrap_or(client_id);
                let sessions = server.agent.resumable_for_tab(tab, 5).await;
                server.agent.broadcast(AgentServerMessage::AgentRpc {
                    code: "session.resumable".into(),
                    result_json: serde_json::json!({ "sessions": sessions }).to_string(),
                });
            }
            _ => server.agent.dispatch(*command),
        }
    } else {
        codec
            .write_server_message(
                &mut stream,
                &ServerMessage::Agent(Box::new(AgentServerMessage::Diagnostic {
                    code: "agent.unavailable".to_string(),
                    message: "agent host is not attached to this connection".to_string(),
                })),
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_ui_projection_requires_exact_manifest_declaration() {
        let manifest = BehaviorManifest::minimal_text_editing(1);
        assert!(manifest_allows_client_ui(
            &manifest,
            "documents.clientOpenFileDialog"
        ));
        assert!(!manifest_allows_client_ui(
            &manifest,
            "documents.clientOpenFileDialog.evil"
        ));
        assert!(!manifest_allows_client_ui(
            &manifest,
            "runtime.reloadConfiguration"
        ));
    }

    #[test]
    fn host_agent_actions_do_not_require_a_static_sdui_node() {
        assert!(is_agent_run_action("agent.submit"));
        assert!(is_agent_run_action("agent.cancel"));
        assert!(is_agent_run_action("agent.steer"));
        assert!(!is_agent_run_action("shell.run"));
        // Plan 118: the retired chat ids are not an authorization path.
        assert!(!is_agent_run_action("chat.submit"));
        assert!(!is_agent_run_action("chat.cancel"));
        assert!(!is_agent_run_action("chat.steer"));
    }

    #[test]
    fn agent_submit_webview_payload_deserializes_with_prompt_text() {
        let json = r#"{
            "family":"sduiAction",
            "payload":{
                "clientId":0,
                "uiVersion":4,
                "intent":{
                    "commandId":"agent.submit",
                    "source":{"button":{"nodeId":1}},
                    "arguments":[{"name":"value","value":{"string":"hello"}}]
                }
            }
        }"#;
        let message: crate::protocol::ClientMessage =
            serde_json::from_str(json).expect("webview agent.submit payload");
        let crate::protocol::ClientMessage::SduiAction { intent, .. } = message else {
            panic!("expected sduiAction");
        };
        assert_eq!(intent.command_id, "agent.submit");
        assert_eq!(intent_text(&intent), "hello");
    }

    #[test]
    fn agent_submit_intent_carries_optional_thinking_level() {
        // Plan 109 I4: the prompt's portable thinking level rides the
        // intent as a second named argument; absent keeps the current
        // effort, non-string arguments are ignored (the daemon fail-closes
        // empty/non-string level strings at its boundary).
        let with_level = r#"{
            "family":"sduiAction",
            "payload":{
                "clientId":0,
                "uiVersion":4,
                "intent":{
                    "commandId":"agent.submit",
                    "source":{"button":{"nodeId":1}},
                    "arguments":[
                        {"name":"value","value":{"string":"hello"}},
                        {"name":"thinkingLevel","value":{"string":"high"}}
                    ]
                }
            }
        }"#;
        let message: crate::protocol::ClientMessage =
            serde_json::from_str(with_level).expect("webview agent.submit payload");
        let crate::protocol::ClientMessage::SduiAction { intent, .. } = message else {
            panic!("expected sduiAction");
        };
        assert_eq!(intent_text(&intent), "hello");
        assert_eq!(intent_thinking_level(&intent).as_deref(), Some("high"));

        let without_level = r#"{
            "family":"sduiAction",
            "payload":{
                "clientId":0,
                "uiVersion":4,
                "intent":{
                    "commandId":"agent.submit",
                    "source":{"button":{"nodeId":1}},
                    "arguments":[{"name":"value","value":{"string":"hello"}}]
                }
            }
        }"#;
        let message: crate::protocol::ClientMessage =
            serde_json::from_str(without_level).expect("webview agent.submit payload");
        let crate::protocol::ClientMessage::SduiAction { intent, .. } = message else {
            panic!("expected sduiAction");
        };
        assert_eq!(intent_thinking_level(&intent), None);
    }
}
