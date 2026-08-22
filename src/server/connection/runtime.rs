//! Runtime family: SDUI actions, command intents (incl. settings persistence
//! and reload), generation-ack, completion and language-intelligence request
//! scheduling. Plan 090 task 2 extraction.

use std::sync::Arc;

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    packages::commands::CommandRegistry,
    perf::budgets::{COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES},
    protocol::{
        BehaviorManifest, ClientId, CompletionProvenance, CompletionRequest, CompletionResultSet,
        CompletionStatus, CompletionTrigger, DocumentId, LanguageIntelligenceFeature,
        LanguageIntelligencePayload, LanguageIntelligenceResult, LanguageIntelligenceStatus,
        ProtocolErrorCode, SduiActionArgument, SduiActionIntent, SduiActionSource, SduiActionValue,
        ServerMessage, TabId,
        codec::{Codec, CodecError},
        completion::estimated_result_payload_bytes,
    },
    server::{
        agent_picker::picker_kind_for_command,
        command_execution::{
            CONTROL_CENTER_COMMAND_ID, CommandExecutionRequest, CommandExecutionTarget,
            CommandExecutor, OPEN_PATH_BROWSER_COMMAND_ID,
        },
        completion::{
            CompletionCoordinator, CompletionProviderMeta, apply_exclusive_suppression,
            completion_prefix_matches, completion_recency_rank, score_completion_item,
        },
        document::DocumentState,
        document_analysis::DocumentAnalysisCoordinator,
        language_intelligence::{
            LanguageIntelligenceCoordinator, LanguageIntelligenceCoordinatorError,
            LanguageIntelligenceDocumentWindow,
        },
        menu_sessions::ServerMenuSessions,
        sdui::{StaticSduiState, sdui_action_response},
        tab_registry::TabRegistry,
        workspace::WorkspaceState,
    },
};

use crate::server::{IpcServer, RuntimeGenerationStore};

use super::{
    documents::{document_for_message, write_document_open_response},
    menus::open_command_centre_session,
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
        // `arguments.item_id`; `setTypography` has no value payload yet (free-
        // form textInput value carriage is a follow-up protocol task), so it
        // validates and acknowledges without persisting. `settings.reset`
        // clears the persisted preferences store.
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
        if let Some(server) = reload_server {
            match persist_settings_change(server, &validated.command_id, &request.arguments).await {
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
        return None;
    }

    if crate::server::command_execution::is_chat_command(&request.command_id) {
        return match executor.execute_chat(request) {
            Ok(_) => None,
            Err(error) => Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: format!(
                    "command execution rejected: {:?}: {}",
                    error.rule, error.message
                ),
            }),
        };
    }

    if crate::server::command_execution::is_reload_command(&request.command_id) {
        let Some(server) = reload_server else {
            return Some(ServerMessage::Error {
                code: ProtocolErrorCode::InvalidMessage,
                message: "runtime reload service is unavailable".to_string(),
            });
        };
        return match server.execute_reload_command(request).await {
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
    /// Command acknowledged without persistence (e.g. `settings.open`,
    /// `settings.close`, `settings.setTypography` which has no value payload).
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
    use crate::server::configuration::ConfigurationRuntime;
    let Some(config_root) = server.config.configuration_root.as_ref() else {
        return Err("settings persistence requires a configured configuration root".to_string());
    };
    let runtime = ConfigurationRuntime::from_config_root(config_root)
        .map_err(|error| format!("settings persistence root error: {error}"))?;
    let should_reload = match command_id {
        "settings.setTheme" => {
            let value = settings_value(arguments).ok_or_else(|| {
                "settings.setTheme requires an item_id/specifier argument".to_string()
            })?;
            runtime
                .persist_preference("theme", serde_json::Value::String(value))
                .map(|_| true)
                .map_err(|error| format!("settings.setTheme persistence failed: {error}"))?
        }
        "settings.setAppearance" => {
            let value = settings_value(arguments).ok_or_else(|| {
                "settings.setAppearance requires an item_id/appearance argument".to_string()
            })?;
            runtime
                .persist_preference("appearance", serde_json::Value::String(value))
                .map(|_| true)
                .map_err(|error| format!("settings.setAppearance persistence failed: {error}"))?
        }
        "settings.reset" => runtime
            .clear_preferences()
            .map(|_| true)
            .map_err(|error| format!("settings.reset failed: {error}"))?,
        // settings.open / settings.close / settings.setTypography: no
        // persistable value yet (setTypography free-form value carriage is a
        // follow-up protocol task). Acknowledge without reloading.
        "settings.open" | "settings.close" | "settings.setTypography" => false,
        _ => false,
    };
    if should_reload {
        let outcome = server.reload_runtime_generation().await;
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
    text: &str,
    package_prefix: &str,
) -> crate::server::completion::CompletionDocumentWindow {
    const WINDOW_BYTES: usize = 64 * 1024;
    let cursor = (request.cursor_byte_offset as usize).min(text.len());
    let mut start = cursor.saturating_sub(WINDOW_BYTES / 2);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + WINDOW_BYTES).min(text.len());
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    crate::server::completion::CompletionDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        package_prefix: package_prefix.to_string(),
        byte_start: start as u64,
        byte_end: end as u64,
        text: text[start..end].to_string(),
    }
}

pub(super) fn language_intelligence_document_window_for_behavior(
    request: &crate::protocol::LanguageIntelligenceRequest,
    text: &str,
    behavior: &BehaviorManifest,
) -> LanguageIntelligenceDocumentWindow {
    let manifest_id = &behavior.manifest_id;
    language_intelligence_document_window(
        request,
        text,
        manifest_id.rsplit('.').next().unwrap_or(manifest_id),
    )
}

pub(super) fn language_intelligence_document_window(
    request: &crate::protocol::LanguageIntelligenceRequest,
    text: &str,
    active_mode: &str,
) -> LanguageIntelligenceDocumentWindow {
    use crate::perf::budgets::LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES;

    let cursor = (request.cursor_byte_offset as usize).min(text.len());
    let half = LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES / 2;
    let mut start = cursor.saturating_sub(half);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES).min(text.len());
    while end < text.len() && !text.is_char_boundary(end) {
        end += 1;
    }
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end < start {
        end = start;
    }

    LanguageIntelligenceDocumentWindow {
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        byte_start: start as u64,
        byte_end: end as u64,
        text: text[start..end].to_string(),
        active_mode: active_mode.to_string(),
    }
}

pub(super) fn static_package_completion_result(
    request: &CompletionRequest,
    manifest_id: &str,
    document_text: &str,
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
    let start = usize::try_from(request.replacement_range.byte_start).ok()?;
    let end = usize::try_from(request.replacement_range.byte_end).ok()?;
    let prefix = document_text.get(start..end)?;
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

pub(super) async fn handle_runtime_generation_installed(
    runtime_generation: &RuntimeGenerationStore,
    ack_client_id: ClientId,
    client_id: ClientId,
    runtime_generation_id: u64,
) {
    let _ = runtime_generation
        .note_runtime_generation_installed(ack_client_id, client_id, runtime_generation_id)
        .await;
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_sdui_action<S>(
    codec: Codec,
    stream: &mut S,
    sdui: &Arc<Mutex<StaticSduiState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    behavior: &Arc<Mutex<crate::server::behavior::ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    parse_coordinator: &crate::server::parse_coordinator::ParseCoordinator,
    document_analysis: &DocumentAnalysisCoordinator,
    menu_sessions: &mut ServerMenuSessions,
    tab_registry: &Arc<Mutex<TabRegistry>>,
    reload_server: Option<&IpcServer>,
    client_id: ClientId,
    intent: SduiActionIntent,
    bound_tab_id: Option<TabId>,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    if intent.command_id == "chat.submit" || intent.command_id == "chat.cancel" {
        if let Some(host) = reload_server.map(|server| &server.agent) {
            let tab = bound_tab_id.unwrap_or(client_id);
            let message = if intent.command_id == "chat.cancel" {
                host.cancel_tab(tab).await
            } else {
                host.begin_prompt(tab, &intent_text(&intent)).await
            };
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
    let validation_response = {
        let state = sdui.lock().await;
        sdui_action_response(&state, &intent)
    };
    if let Some(response) = validation_response {
        codec.write_server_message(stream, &response).await?;
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

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_command_intent<S>(
    codec: Codec,
    stream: &mut S,
    menu_sessions: &mut ServerMenuSessions,
    behavior: &Arc<Mutex<crate::server::behavior::ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    tab_registry: &Arc<Mutex<TabRegistry>>,
    reload_server: Option<&IpcServer>,
    client_id: ClientId,
    document_id: DocumentId,
    behavior_version: crate::protocol::BehaviorVersion,
    command_id: String,
    bound_tab_id: Option<TabId>,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    // Commands never receive previous-generation grace.
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
    // Phase 24.1-24.3: the built-in Command Centre commands are a
    // command-lane special case mirroring the workspace-command precedent —
    // the bounded snapshot IS the response. Generic execution of these ids
    // yields nothing on the wire, so bare `Accepted` accounting (the JS op
    // path) is unchanged. Opening replaces any active server session; the
    // client is told about the closed one.
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
    if command_id == "chat.cancel"
        && let Some(host) = reload_server.map(|server| &server.agent)
    {
        let message = host.cancel_tab(bound_tab_id.unwrap_or(client_id)).await;
        codec
            .write_server_message(stream, &ServerMessage::Agent(Box::new(message)))
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

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_completion_request<S>(
    codec: Codec,
    stream: &mut S,
    behavior: &Arc<Mutex<crate::server::behavior::ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    completion: &CompletionCoordinator,
    document_analysis: &DocumentAnalysisCoordinator,
    completion_tx: &tokio::sync::mpsc::Sender<ServerMessage>,
    dropped_results: &Arc<std::sync::atomic::AtomicU64>,
    client_id: ClientId,
    request: &mut CompletionRequest,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
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
    let document_text = target_document.lock().await.text();
    let providers = runtime_generation
        .current()
        .await
        .service
        .completion_providers();
    let fallback =
        static_package_completion_result(request, &manifest_id, &document_text, &providers)
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
    if let Some(provider) = dynamic_provider {
        request.provider_generation = provider.generation;
        let window = completion_document_window(
            request,
            &document_text,
            &provider.provenance.package_prefix,
        );
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

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_language_intelligence_request<S>(
    codec: Codec,
    stream: &mut S,
    behavior: &Arc<Mutex<crate::server::behavior::ActiveBehaviorManifest>>,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    language_intelligence: &LanguageIntelligenceCoordinator,
    language_intelligence_tx: &tokio::sync::mpsc::Sender<ServerMessage>,
    dropped_results: &Arc<std::sync::atomic::AtomicU64>,
    client_id: ClientId,
    request: &mut crate::protocol::LanguageIntelligenceRequest,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
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
    let document_text = target_document.lock().await.text();
    let window = {
        let behavior = behavior.lock().await;
        let manifest = behavior.manifest_for(request.document_id).clone();
        language_intelligence_document_window_for_behavior(request, &document_text, &manifest)
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
