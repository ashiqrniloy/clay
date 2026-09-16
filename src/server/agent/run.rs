//! The agent run pipeline: prompts, streaming, approvals, and cancellation
//! (plan 119 SC-3).
//!
//! Split out of `src/server/agent.rs` unchanged. Every `session.prompt` /
//! `session.cancel` / `session.steer` / `session.resume` and the daemon
//! bootstrap commands (`ListSessions`, pickers, vault, package registration)
//! enter through `AgentHost::run_inner`; daemon events are mapped to
//! `AgentServerMessage` here, with secrets redacted on the way. Durable-run
//! tool approvals are brokered to connected clients and fail closed on
//! timeout or missing consumer.

use super::book::{
    context_tokens, json_om_workers, json_usage, selection_for, snapshot_from_load,
    snapshot_from_new,
};
use super::mcp::picker_items;
use super::{
    AGENT_MAX_ENTRY_TEXT_BYTES, AGENT_MAX_PROMPT_BYTES, AgentClientCommand, AgentError, AgentHost,
    AgentOmWorkerKind, AgentOmWorkerModel, AgentSecret, AgentServerMessage, AgentToolPhase,
    AgentTranscriptEntry, AgentTranscriptFile, AgentTranscriptKind, AgentWireEvent,
    ApprovalRequestKind, Ordering, TabId, agent_rpc, diagnostic, error_code, json_string,
    json_text, redact_text, transcript_file_for_tool, truncate_transcript_text,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

/// How long a pending user-approval request waits for a client answer
/// before failing closed (deny). Generous: a human is deciding.
pub(super) const APPROVAL_WAIT: Duration = Duration::from_secs(300);
/// Upper bound on one daemon-produced approval request payload.
pub(super) const MAX_APPROVAL_PAYLOAD_BYTES: usize = 16 * 1024;

/// Pending daemon-initiated approval requests, keyed by request id.
pub(super) type PendingApprovals = HashMap<String, oneshot::Sender<Result<Value, String>>>;

pub(super) fn map_event(params: &Value, secrets: &[String]) -> Option<AgentServerMessage> {
    let session_id = params
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let event = params.get("event").unwrap_or(params);
    let event_type = event.get("type").and_then(Value::as_str)?;
    let run_id = event
        .get("runId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mapped = match event_type {
        "agent_started" => AgentWireEvent::Started {
            session_id: session_id.clone(),
            run_id,
        },
        "agent_finished" => AgentWireEvent::Finished {
            session_id: session_id.clone(),
            run_id,
            usage: json_usage(event),
            // Run-total usage is the wrong meter numerator (it sums every
            // turn's prompt); occupancy books per provider turn below.
            context_tokens: None,
        },
        // Plan 117 token meter: the last provider round's prompt tokens
        // (input + cache reads + cache writes) IS the context occupancy
        // the next round starts from. Later turns overwrite — latest wins.
        // Usage-unreported turns fall to the catch-all (heuristic rules).
        "provider_turn_finished" if context_tokens(event).is_some() => {
            AgentWireEvent::ContextTokens {
                session_id: session_id.clone(),
                run_id,
                tokens: context_tokens(event).unwrap_or_default(),
            }
        }
        "message_delta"
            if json_string(event, &["content", "type"]) == "thinking"
                || json_string(event, &["content", "type"]) == "reasoning" =>
        {
            AgentWireEvent::ThinkingDelta {
                session_id: session_id.clone(),
                run_id,
                text: redact_text(&json_text(event.get("content").unwrap_or(event)), secrets),
            }
        }
        "message_delta" => AgentWireEvent::MessageDelta {
            session_id: session_id.clone(),
            run_id,
            text: redact_text(&json_text(event.get("content").unwrap_or(event)), secrets),
        },
        "tool_execution_started" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Started,
            name: json_string(event, &["call", "name"]),
            tool_call_id: json_string(event, &["call", "id"]),
            args_digest: tool_args_digest(event, secrets),
            output_digest: None,
            // load_skill args are {"name": "<skill>"} (plan 109 I5).
            skill_name: skill_name_from_args(event),
            file: tool_file(event, secrets),
        },
        "tool_execution_progress" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Progress,
            name: json_string(event, &["name"]),
            tool_call_id: json_string(event, &["toolCallId"]),
            args_digest: None,
            output_digest: None,
            skill_name: None,
            file: None,
        },
        "tool_execution_finished" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Finished,
            name: json_string(event, &["result", "name"]),
            tool_call_id: json_string(event, &["result", "toolCallId"]),
            args_digest: None,
            output_digest: tool_output_digest(event, secrets),
            skill_name: None,
            file: None,
        },
        "tool_execution_error" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Error,
            name: json_string(event, &["call", "name"]),
            tool_call_id: json_string(event, &["call", "id"]),
            args_digest: tool_args_digest(event, secrets),
            output_digest: Some(redact_text(
                &json_string(event, &["error", "message"]),
                secrets,
            ))
            .filter(|output| !output.is_empty()),
            skill_name: skill_name_from_args(event),
            // An errored file tool still names the file it tried to touch.
            file: tool_file(event, secrets),
        },
        "tool_execution_blocked" => AgentWireEvent::Tool {
            session_id: session_id.clone(),
            run_id,
            phase: AgentToolPhase::Blocked,
            name: json_string(event, &["name"]),
            tool_call_id: json_string(event, &["toolCallId"]),
            args_digest: None,
            output_digest: Some(redact_text(&json_string(event, &["reason"]), secrets))
                .filter(|output| !output.is_empty()),
            skill_name: None,
            file: None,
        },
        "permission_requested" | "permission_request" => AgentWireEvent::Permission {
            session_id: session_id.clone(),
            run_id,
            request_id: json_string(event, &["requestId"]),
            tool_name: json_string(event, &["toolName"]),
            allowed: None,
        },
        "permission_resolved" => AgentWireEvent::Permission {
            session_id: session_id.clone(),
            run_id,
            request_id: json_string(event, &["requestId"]),
            tool_name: json_string(event, &["toolName"]),
            allowed: event.get("allowed").and_then(Value::as_bool),
        },
        "event_subscriber_overflow" => AgentWireEvent::Overflow,
        // Plan 122: supervisor subagent lifecycle (the daemon's
        // `observeSupervisorLifecycle` bridge) maps onto the existing Tool
        // wire event so children render as ordinary tool rows — no new AG-UI
        // event type or panel. `name` is the stable `childId` (fallback
        // `spawn_agent`); the delegation id pairs Started/Finished rows.
        // Events without any identifying id stay dropped (plan 120's
        // unknown-drop contract still governs `delegation_*` and the rest).
        "subagent_started" | "subagent_stopped" => {
            let child_id = json_string(event, &["childId"]);
            let delegation_id = json_string(event, &["delegationId"]);
            if child_id.is_empty() && delegation_id.is_empty() {
                return None;
            }
            let name = if child_id.is_empty() {
                "spawn_agent".to_string()
            } else {
                child_id
            };
            let status = json_string(event, &["status"]);
            AgentWireEvent::Tool {
                session_id: session_id.clone(),
                run_id,
                phase: if event_type == "subagent_started" {
                    AgentToolPhase::Started
                } else {
                    AgentToolPhase::Finished
                },
                name,
                tool_call_id: delegation_id,
                args_digest: None,
                output_digest: (!status.is_empty()).then(|| {
                    truncate_transcript_text(
                        &redact_text(&status, secrets),
                        AGENT_MAX_ENTRY_TEXT_BYTES,
                    )
                }),
                skill_name: None,
                file: None,
            }
        }
        // Durable-run suspension (tool approval): surface the pending
        // approval as Permission (request_id = first pending approvalId)
        // so the panel can render Allow/Deny and resume the run. Without
        // this arm the suspension fell into the catch-all below and the
        // client saw a spurious Started, leaving the run "streaming"
        // forever. Suspensions without a client-resumable approval close
        // the AG-UI run instead of parking it.
        "agent_suspended" => {
            let pending = event
                .get("interruption")
                .and_then(|interruption| interruption.get("pendingDecisions"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let tool_name = event
                .get("interruption")
                .and_then(|interruption| interruption.get("toolName"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let approval = pending
                .first()
                .and_then(|decision| decision.get("approvalId"))
                .and_then(Value::as_str)
                .map(str::to_string);
            match approval {
                Some(approval_id) if !tool_name.is_empty() => AgentWireEvent::Permission {
                    session_id: session_id.clone(),
                    run_id,
                    request_id: approval_id,
                    tool_name: tool_name.to_string(),
                    allowed: None,
                },
                _ => AgentWireEvent::Finished {
                    session_id: session_id.clone(),
                    run_id,
                    usage: String::new(),
                    context_tokens: None,
                },
            }
        }
        // Prism error events nest the details under `error`: {error: {name,
        // message, code}}; a top-level `message` string is the legacy shape.
        "error" => {
            let detail = event
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .or_else(|| event.get("message").and_then(Value::as_str))
                .unwrap_or("error");
            AgentWireEvent::Error {
                session_id: session_id.clone(),
                message: redact_text(detail, secrets),
            }
        }
        // Unknown event types are dropped, never forwarded. The catch-all
        // used to synthesize `Started`, which left the client believing a
        // run was streaming forever (`agent_suspended` incident); Prism 0.7
        // adds `attention_compiled`, `subagent_*`, and `delegation_*`
        // telemetry that must not reopen a run. A lifecycle type Clay has to
        // render gets an explicit arm instead (subagent lifecycle maps onto
        // the existing Tool events).
        _ => return None,
    };
    Some(AgentServerMessage::Event {
        session_id,
        event: mapped,
    })
}

/// The file a tool call touches (plan 118 task 36): its verb plus the call's
/// `path` argument, redacted and bounded like the args digest. Only the tools
/// that name one file carry a record — search, shell, git and move name none.
pub(super) fn tool_file(event: &Value, secrets: &[String]) -> Option<AgentTranscriptFile> {
    let name = json_string(event, &["call", "name"]);
    let arguments = event.get("call").and_then(|call| call.get("arguments"))?;
    let file = transcript_file_for_tool(&name, arguments)?;
    let path = truncate_transcript_text(
        &redact_text(&file.path, secrets),
        AGENT_MAX_ENTRY_TEXT_BYTES,
    );
    if path.is_empty() {
        return None;
    }
    Some(AgentTranscriptFile { path, op: file.op })
}

/// Bounded, redacted argument summary for tool rows (plan 109 I5): the
/// daemon's `call.arguments` compacted, secret-redacted, and truncated to
/// the per-entry budget before the wire. `None` when the call carries no
/// arguments object.
pub(super) fn tool_args_digest(event: &Value, secrets: &[String]) -> Option<String> {
    let arguments = event.get("call").and_then(|call| call.get("arguments"))?;
    if !arguments.is_object() {
        return None;
    }
    let text = redact_text(&arguments.to_string(), secrets);
    (!text.is_empty()).then(|| truncate_transcript_text(&text, AGENT_MAX_ENTRY_TEXT_BYTES))
}

/// Bounded, redacted output excerpt for terminal tool rows (plan 109 I5):
/// finished rows project the result's text content blocks (falling back to
/// the stringified `value`); the caller handles error/blocked reasons.
pub(super) fn tool_output_digest(event: &Value, secrets: &[String]) -> Option<String> {
    let result = event.get("result")?;
    let mut text = String::new();
    if let Some(blocks) = result.get("content").and_then(Value::as_array) {
        for block in blocks {
            if block.get("type").and_then(Value::as_str) == Some("text")
                && let Some(chunk) = block.get("text").and_then(Value::as_str)
            {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(chunk);
            }
        }
    }
    if text.is_empty()
        && let Some(value) = result.get("value")
        && !value.is_null()
    {
        text = value.to_string();
    }
    let text = redact_text(&text, secrets);
    (!text.is_empty()).then(|| truncate_transcript_text(&text, AGENT_MAX_ENTRY_TEXT_BYTES))
}

/// `load_skill` rows carry the loaded skill's name from the call arguments
/// (`{"name": "<skill>"}`); other tools have none (plan 109 I5).
pub(super) fn skill_name_from_args(event: &Value) -> Option<String> {
    let name = json_string(event, &["call", "name"]);
    if name != "load_skill" {
        return None;
    }
    let skill = json_string(event, &["call", "arguments", "name"]);
    (!skill.is_empty()).then_some(skill)
}

impl AgentHost {
    pub async fn request_user_approval(
        &self,
        kind: ApprovalRequestKind,
        payload_json: &str,
    ) -> Result<Value, String> {
        self.request_user_approval_for(kind, payload_json, APPROVAL_WAIT)
            .await
    }

    pub(super) async fn request_user_approval_for(
        &self,
        kind: ApprovalRequestKind,
        payload_json: &str,
        wait: Duration,
    ) -> Result<Value, String> {
        if payload_json.len() > MAX_APPROVAL_PAYLOAD_BYTES {
            return Err("approval payload exceeds the wire budget".into());
        }
        let request_id = format!(
            "approval-{}",
            self.inner.approval_seq.fetch_add(1, Ordering::Relaxed)
        );
        let (reply, answer) = oneshot::channel();
        self.inner
            .approvals
            .lock()
            .await
            .insert(request_id.clone(), reply);
        let request = Arc::new(AgentServerMessage::ApprovalRequest {
            request_id: request_id.clone(),
            kind,
            payload_json: payload_json.to_string(),
        });
        if self.inner.events.send(request).is_err() {
            self.inner.approvals.lock().await.remove(&request_id);
            return Err("no approval consumer is connected".into());
        }
        let result = match timeout(wait, answer).await {
            Ok(Ok(result)) => result,
            Ok(Err(_receiver_gone)) => Err("no approval consumer is connected".into()),
            Err(_elapsed) => Err("approval request timed out".into()),
        };
        self.inner.approvals.lock().await.remove(&request_id);
        result
    }

    /// Answer a pending approval request. Returns false for unknown/stale
    /// ids; the requester sees exactly one answer either way.
    pub async fn resolve_approval(&self, request_id: &str, result: Result<Value, String>) -> bool {
        match self.inner.approvals.lock().await.remove(request_id) {
            Some(reply) => reply.send(result).is_ok(),
            None => false,
        }
    }

    pub async fn begin_prompt(&self, tab: TabId, text: &str) -> AgentServerMessage {
        self.begin_prompt_with_effort(tab, text, None).await
    }

    /// Plan 109 I4: prompt with the session's portable thinking level
    /// (`None` keeps the current effort). The daemon fail-closes invalid
    /// level strings at its boundary.
    pub async fn begin_prompt_with_effort(
        &self,
        tab: TabId,
        text: &str,
        thinking_level: Option<String>,
    ) -> AgentServerMessage {
        if text.trim().is_empty() {
            return self.emit_agent(diagnostic("agent.empty_prompt", "empty prompt"));
        }
        if text.len() > AGENT_MAX_PROMPT_BYTES {
            return self.emit_agent(diagnostic(
                "agent.prompt_too_large",
                "prompt exceeds AGENT_MAX_PROMPT_BYTES",
            ));
        }
        let Some(session_id) = self.ensure_tab_session(tab).await else {
            return self.emit_agent(AgentServerMessage::Snapshot(
                self.unconfigured_snapshot().await,
            ));
        };
        {
            let mut book = self.inner.book.lock().await;
            book.push_row(
                &session_id,
                AgentTranscriptEntry::new(AgentTranscriptKind::User, text),
            );
            book.cancelled.remove(&session_id);
            book.running.insert(session_id.clone());
            // Plan 109 I4: the prompt's level becomes the session's active
            // effort; STATE echoes it until the next prompt changes it.
            if let Some(level) = &thinking_level {
                book.effort.insert(session_id.clone(), level.clone());
            }
        }
        let snapshot = self.snapshot_for(&session_id).await;
        // Run-scoped selection override (plan 108 task 9, per-workspace in
        // plan 109 I2): a picker switch between runs applies at the next
        // prompt without a new session. The workspace's last-used selection
        // resolves first; no inventory re-check here — the entry was written
        // from an already-configured picker selection.
        let workspace_root = self.tab_workspace_root(tab).await;
        let agent_type = self.tab_agent_type(tab).await;
        let (provider, model) = {
            let book = self.inner.book.lock().await;
            let resolved = selection_for(
                &book,
                agent_type.as_deref(),
                workspace_root.as_deref(),
                &|_| true,
            );
            (resolved.provider, resolved.model)
        };
        self.dispatch(AgentClientCommand::Prompt {
            session_id,
            text: text.to_string(),
            provider: Some(provider),
            model: Some(model),
            thinking_level,
        });
        self.emit_agent(AgentServerMessage::Snapshot(snapshot))
    }

    pub(crate) async fn cancel_tab(&self, tab: TabId) -> AgentServerMessage {
        let session_id = self.tab_session_id(tab).await;
        let Some(session_id) = session_id else {
            return diagnostic("agent.idle", "no running session");
        };
        {
            let mut book = self.inner.book.lock().await;
            book.cancelled.insert(session_id.clone());
            book.running.remove(&session_id);
        }
        self.dispatch(AgentClientCommand::Cancel { session_id });
        diagnostic("agent.cancelled", "cancelled")
    }

    /// Queues a mid-run user message on the tab's session (pi-parity steer,
    /// plan 108 task 9). User-initiated only: reachable solely through the
    /// validated chat intent path with the composer's text.
    pub async fn steer_tab(&self, tab: TabId, text: &str) -> AgentServerMessage {
        if text.trim().is_empty() || text.len() > AGENT_MAX_PROMPT_BYTES {
            return diagnostic("agent.steer_rejected", "empty or oversized steer");
        }
        let session_id = self.tab_session_id(tab).await;
        let Some(session_id) = session_id else {
            return diagnostic("agent.idle", "no running session");
        };
        // Plan 109 I5: mid-run user input is visible in the transcript
        // (pi parity) — record the steer as a user-kind entry and publish
        // the snapshot so the live run reconciles around it.
        let snapshot = {
            let mut book = self.inner.book.lock().await;
            book.push_row(
                &session_id,
                AgentTranscriptEntry::new(AgentTranscriptKind::User, text),
            );
            self.snapshot_for(&session_id).await
        };
        self.dispatch(AgentClientCommand::Steer {
            session_id,
            text: text.to_string(),
            soft_interrupt: false,
        });
        self.emit_agent(AgentServerMessage::Snapshot(snapshot));
        diagnostic("agent.steered", "steered")
    }

    pub async fn resume_tab(
        &self,
        tab: TabId,
        session_id: &str,
        entry_id: Option<&str>,
    ) -> AgentServerMessage {
        let root = self.tab_workspace_root(tab).await;
        let tab_agent = self.tab_agent_type(tab).await;
        self.inner.book.lock().await.bind_workspace_session(
            tab_agent.as_deref(),
            root.as_deref(),
            session_id,
        );
        // The root binding is load-bearing, not bookkeeping: agent tool calls
        // resolve their workspace from the session (`session_workspace_root`),
        // and `session_for_workspace` looks the resume up by that same key —
        // so a resume that left the root unset made the next prompt start a
        // brand-new session and silently abandon the one just opened. The
        // agent binding is symmetric (plan 118 task 35): resuming a session
        // written by another agent type adopts *that* agent, so the next run
        // re-reads the config the transcript was produced under.
        if let Some(root) = root.clone() {
            let mut book = self.inner.book.lock().await;
            book.session_root.insert(session_id.to_string(), root);
        }
        if let Some(agent) = tab_agent.clone().or(self.session_agent(session_id).await) {
            let mut book = self.inner.book.lock().await;
            book.session_agent
                .insert(session_id.to_string(), agent.clone());
            // A resumed session adopts the tab's agent key when the tab
            // declares one; otherwise the daemon's record decides.
            if tab_agent.is_none() {
                book.bind_workspace_session(Some(&agent), root.as_deref(), session_id);
            }
        }
        let loaded = self
            .run(AgentClientCommand::LoadSession {
                session_id: session_id.to_string(),
                entry_id: entry_id.map(str::to_string),
            })
            .await;
        if let AgentServerMessage::Snapshot(snapshot) = &loaded {
            let mut book = self.inner.book.lock().await;
            book.transcripts
                .insert(session_id.to_string(), snapshot.entries.clone());
            if let Some(tokens) = snapshot.context_tokens {
                book.context_tokens
                    .insert(session_id.to_string(), Some(tokens));
            }
            if !snapshot.profile.is_empty() {
                book.profile = snapshot.profile.clone();
            }
            if !snapshot.provider.is_empty() {
                book.provider = snapshot.provider.clone();
            }
            if !snapshot.model.is_empty() {
                book.model = snapshot.model.clone();
            }
        }
        // Plan 117: no trailing dispatch here. The old
        // `dispatch(ResumeSession)` broadcast an entry-less snapshot that
        // raced the rich load snapshot and wiped the restored transcript;
        // the daemon creates the live session lazily at the next prompt.
        loaded
    }

    /// Plan 109 I9: workspace-scoped resumable session list for the
    /// /resume picker. `workspace_root` comes from the tab registry
    /// (server-derived, never webview input); the daemon scopes the query
    /// and bounds the page, most-recent first.
    pub async fn run(&self, command: AgentClientCommand) -> AgentServerMessage {
        if self.inner.config.inert {
            return diagnostic("agent.unavailable", "agent host is not started");
        }
        match self.run_inner(command).await {
            Ok(message) => message,
            Err(error) => {
                let message = redact_text(&error.to_string(), &self.secrets().await);
                diagnostic(error_code(&error), &message)
            }
        }
    }

    pub(super) async fn secrets(&self) -> Vec<String> {
        self.inner.secrets.lock().await.clone()
    }

    pub(super) async fn remember_secret(&self, secret: &str) {
        if secret.is_empty() {
            return;
        }
        let mut secrets = self.inner.secrets.lock().await;
        if !secrets.iter().any(|item| item == secret) {
            secrets.push(secret.to_string());
        }
    }

    pub(super) async fn run_inner(
        &self,
        command: AgentClientCommand,
    ) -> Result<AgentServerMessage, AgentError> {
        match command {
            AgentClientCommand::Prompt {
                session_id,
                text,
                provider,
                model,
                thinking_level,
            } => {
                if text.len() > AGENT_MAX_PROMPT_BYTES {
                    return Ok(diagnostic(
                        "agent.prompt_too_large",
                        "prompt exceeds AGENT_MAX_PROMPT_BYTES",
                    ));
                }
                let mut params = json!({ "sessionId": session_id, "text": text });
                if let Some(provider) = provider {
                    params["provider"] = json!(provider);
                }
                if let Some(model) = model {
                    params["model"] = json!(model);
                }
                if let Some(level) = thinking_level {
                    params["thinkingLevel"] = json!(level);
                }
                self.rpc("session.prompt", params).await?;
                // Refreshed snapshot: after a switch the state carries the
                // session's persisted provider/model so every client tab and
                // the status row agree.
                Ok(AgentServerMessage::Snapshot(
                    self.snapshot_for(&session_id).await,
                ))
            }
            AgentClientCommand::Cancel { session_id } => {
                self.rpc("session.cancel", json!({ "sessionId": session_id }))
                    .await?;
                Ok(diagnostic("agent.cancelled", "cancelled"))
            }
            AgentClientCommand::Steer {
                session_id,
                text,
                soft_interrupt,
            } => {
                self.rpc(
                    "session.steer",
                    json!({
                        "sessionId": session_id,
                        "text": text,
                        "softInterrupt": soft_interrupt,
                    }),
                )
                .await?;
                Ok(diagnostic("agent.steered", "steered"))
            }
            AgentClientCommand::NewSession {
                profile,
                provider,
                model,
                workspace_root,
                full_autonomy,
                om_observation,
                om_reflection,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("profile".into(), json!(profile));
                params.insert("provider".into(), json!(provider));
                params.insert("model".into(), json!(model));
                if let Some(root) = workspace_root {
                    params.insert("workspaceRoot".into(), json!(root));
                }
                if let Some(enabled) = full_autonomy {
                    params.insert("fullAutonomy".into(), json!(enabled));
                }
                // Plan 109 I8: the workspace book's OM worker defaults ride
                // session creation (per-session retention is daemon metadata).
                let om_workers = json_om_workers(om_observation, om_reflection);
                if !om_workers.is_null() {
                    params.insert("observationalMemoryWorkers".into(), om_workers);
                }
                let result = self.rpc("session.new", Value::Object(params)).await?;
                Ok(AgentServerMessage::Snapshot(
                    self.decorate_snapshot(snapshot_from_new(&result)).await,
                ))
            }
            AgentClientCommand::LoadSession {
                session_id,
                entry_id,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(entry) = entry_id {
                    params.insert("entryId".into(), json!(entry));
                }
                let result = self.rpc("session.load", Value::Object(params)).await?;
                Ok(AgentServerMessage::Snapshot(
                    self.decorate_snapshot(snapshot_from_load(&result)).await,
                ))
            }
            // Plan 109 I7: context inspector — the daemon's bounded,
            // redacted response rides the generic agent-RPC custom event
            // (`clay.agentRpc`), so the panel renders it without new
            // snapshot state.
            AgentClientCommand::Context {
                session_id,
                item_id,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(item) = item_id {
                    params.insert("itemId".into(), json!(item));
                }
                let result = self.rpc("session.context", Value::Object(params)).await?;
                Ok(agent_rpc("session.context", &result))
            }
            // Plan 109 I8: direct dispatch (no connection intercept) still
            // applies the daemon half; the book half rides select_worker.
            AgentClientCommand::SelectWorker {
                worker,
                id,
                session_id,
            } => {
                if let Some(session_id) = session_id {
                    let parsed = id
                        .strip_prefix("model:")
                        .and_then(|rest| rest.split_once('/'))
                        .map(|(provider, model)| AgentOmWorkerModel {
                            provider: provider.to_string(),
                            model: model.to_string(),
                        });
                    let params = json!({
                        "sessionId": session_id,
                        "workers": json_om_workers(
                            matches!(worker, AgentOmWorkerKind::Observation).then(|| parsed.clone()).flatten(),
                            matches!(worker, AgentOmWorkerKind::Reflection).then(|| parsed.clone()).flatten(),
                        ),
                    });
                    let result = self.rpc("session.om.set", params).await?;
                    Ok(agent_rpc("session.om.set", &result))
                } else {
                    Ok(diagnostic("agent.omWorkers", "no session bound"))
                }
            }
            // Plan 109 I8: per-session OM worker selection — validated +
            // persisted daemon-side; the response (with the effective
            // selection) rides the generic agent-RPC custom event.
            AgentClientCommand::SetOmWorkers {
                session_id,
                observation,
                reflection,
            } => {
                let params = json!({
                    "sessionId": session_id,
                    "workers": json_om_workers(observation, reflection),
                });
                let result = self.rpc("session.om.set", params).await?;
                Ok(agent_rpc("session.om.set", &result))
            }
            AgentClientCommand::OmActivity { session_id } => {
                let result = self
                    .rpc("session.om.activity", json!({ "sessionId": session_id }))
                    .await?;
                Ok(agent_rpc("session.om.activity", &result))
            }
            AgentClientCommand::ResumeSession { session_id } => {
                let result = self
                    .rpc("session.resume", json!({ "sessionId": session_id }))
                    .await?;
                Ok(AgentServerMessage::Snapshot(
                    self.decorate_snapshot(snapshot_from_new(&result)).await,
                ))
            }
            AgentClientCommand::DeleteSession { session_id } => {
                self.rpc("session.delete", json!({ "sessionId": session_id }))
                    .await?;
                Ok(diagnostic("agent.deleted", "deleted"))
            }
            AgentClientCommand::ListSessions => {
                Ok(AgentServerMessage::Inventory(self.inventory().await?))
            }
            // Plan 117: served by the connection layer (tab-resolved, then
            // broadcast); the dispatch path has no tab and never reaches
            // this arm.
            AgentClientCommand::ResumableSessions => {
                Ok(diagnostic("agent.unavailable", "no tab binding"))
            }
            // Plan 117 follow-up: same tab-resolved shape as the resume
            // list — the connection layer broadcasts the snapshot.
            AgentClientCommand::TabState => Ok(diagnostic("agent.unavailable", "no tab binding")),
            // Plan 117 @-mentions: bounded workspace listing rides the
            // generic agent-RPC custom event like session.context.
            AgentClientCommand::WorkspaceFiles { session_id } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                let result = self.rpc("workspace.files", Value::Object(params)).await?;
                Ok(agent_rpc("workspace.files", &result))
            }
            AgentClientCommand::OpenPicker { kind } => {
                let inventory = self.inventory().await?;
                Ok(AgentServerMessage::Picker {
                    kind,
                    items: picker_items(kind, &inventory),
                })
            }
            AgentClientCommand::Select { kind, id } => {
                Ok(diagnostic("agent.selected", &format!("{kind:?}:{id}")))
            }
            AgentClientCommand::CredentialPut {
                provider,
                name,
                secret: AgentSecret(secret),
            } => {
                self.remember_secret(&secret).await;
                self.rpc(
                    "credential.put",
                    json!({ "provider": provider, "name": name, "secret": secret }),
                )
                .await?;
                Ok(AgentServerMessage::CredentialAck {
                    provider,
                    name,
                    stored: true,
                })
            }
            AgentClientCommand::CredentialDelete { provider, name } => {
                self.rpc(
                    "credential.delete",
                    json!({ "provider": provider, "name": name }),
                )
                .await?;
                Ok(AgentServerMessage::CredentialAck {
                    provider,
                    name,
                    stored: false,
                })
            }
            AgentClientCommand::Compact {
                session_id,
                strategy,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(name) = strategy {
                    params.insert("strategy".into(), json!(name));
                }
                self.rpc("session.compact", Value::Object(params)).await?;
                Ok(diagnostic("agent.compacted", "compacted"))
            }
            AgentClientCommand::SessionTree {
                session_id,
                method,
                entry_id,
            } => {
                let rpc_method = match method.as_str() {
                    "checkout" | "fork" | "clone" | "checkpoint" => method.clone(),
                    other => return Ok(diagnostic("agent.tree_invalid_method", other)),
                };
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                params.insert("entryId".into(), json!(entry_id));
                self.rpc(&format!("session.{rpc_method}"), Value::Object(params))
                    .await?;
                Ok(diagnostic(
                    "agent.tree_commanded",
                    &format!("{rpc_method}:{session_id} at {entry_id}"),
                ))
            }
            AgentClientCommand::SetAutonomy {
                session_id,
                enabled,
            } => {
                self.rpc(
                    "session.setAutonomy",
                    json!({ "sessionId": session_id, "enabled": enabled }),
                )
                .await?;
                Ok(diagnostic(
                    "agent.autonomy_set",
                    &format!("{session_id}: {enabled}"),
                ))
            }
            AgentClientCommand::SearchSessions {
                session_id,
                query,
                limit,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                if let Some(q) = query {
                    params.insert("query".into(), json!(q));
                }
                if let Some(l) = limit {
                    params.insert("limit".into(), json!(l));
                }
                let result = self.rpc("session.search", Value::Object(params)).await?;
                Ok(agent_rpc("agent.search_result", &result))
            }
            AgentClientCommand::RunResume {
                session_id,
                run_id,
                decision_json,
            } => {
                let decision: Value = serde_json::from_str(&decision_json)
                    .map_err(|error| AgentError::Rpc(format!("invalid decision JSON: {error}")))?;
                if !decision.is_object() {
                    return Err(AgentError::Rpc(
                        "invalid decision: expected a JSON object".into(),
                    ));
                }
                // Fail-closed passthrough: the daemon re-validates every
                // field. expectedVersion may be omitted — the daemon then
                // applies its stashed suspension version.
                let mut params = serde_json::Map::new();
                params.insert("sessionId".into(), json!(session_id));
                params.insert("runId".into(), json!(run_id));
                for key in ["expectedVersion", "decision", "decisions"] {
                    if let Some(value) = decision.get(key) {
                        params.insert(key.into(), value.clone());
                    }
                }
                let result = self.rpc("run.resume", Value::Object(params)).await?;
                Ok(agent_rpc("agent.run_resume_result", &result))
            }
            AgentClientCommand::SkillRegister {
                name,
                description,
                instructions,
                tool_names,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("name".into(), json!(name));
                if let Some(text) = description {
                    params.insert("description".into(), json!(text));
                }
                if let Some(text) = instructions {
                    params.insert("instructions".into(), json!(text));
                }
                if !tool_names.is_empty() {
                    params.insert("toolNames".into(), json!(tool_names));
                }
                self.rpc("skill.register", Value::Object(params)).await?;
                Ok(diagnostic("agent.skill_registered", "registered"))
            }
            AgentClientCommand::CommandRegister {
                name,
                handler,
                description,
            } => {
                let mut params = serde_json::Map::new();
                params.insert("name".into(), json!(name));
                if let Some(handler) = handler {
                    params.insert("handler".into(), json!(handler));
                }
                if let Some(text) = description {
                    params.insert("description".into(), json!(text));
                }
                self.rpc("command.register", Value::Object(params)).await?;
                Ok(diagnostic("agent.command_registered", "registered"))
            }
            AgentClientCommand::CommandDispatch {
                name,
                session_id,
                args_json,
            } => {
                let args = match args_json {
                    Some(text) => {
                        let value: Value = serde_json::from_str(&text).map_err(|error| {
                            AgentError::Rpc(format!("invalid command args JSON: {error}"))
                        })?;
                        if !value.is_object() {
                            return Err(AgentError::Rpc(
                                "invalid command args: expected a JSON object".into(),
                            ));
                        }
                        value
                    }
                    None => json!({}),
                };
                let mut params = serde_json::Map::new();
                params.insert("name".into(), json!(name));
                if let Some(session_id) = session_id {
                    params.insert("sessionId".into(), json!(session_id));
                }
                params.insert("args".into(), args);
                let result = self.rpc("command.dispatch", Value::Object(params)).await?;
                Ok(agent_rpc("agent.command_result", &result))
            }
            AgentClientCommand::ApprovalResolve {
                request_id,
                allowed,
            } => {
                let delivered = self
                    .resolve_approval(&request_id, Ok(json!({ "allowed": allowed })))
                    .await;
                Ok(diagnostic(
                    if delivered {
                        "agent.approval_resolved"
                    } else {
                        // Unknown/stale request id: nothing is mutated.
                        "agent.approval_unknown"
                    },
                    &request_id,
                ))
            }
            AgentClientCommand::AskDecisionResolve {
                request_id,
                answer_json,
            } => {
                let answer: Value = serde_json::from_str(&answer_json)
                    .map_err(|error| AgentError::Rpc(format!("invalid answer JSON: {error}")))?;
                if !answer.is_object() {
                    return Err(AgentError::Rpc(
                        "invalid answer: expected a JSON object".into(),
                    ));
                }
                let delivered = self.resolve_approval(&request_id, Ok(answer)).await;
                Ok(diagnostic(
                    if delivered {
                        "agent.ask_decision_resolved"
                    } else {
                        "agent.approval_unknown"
                    },
                    &request_id,
                ))
            }
            AgentClientCommand::RegisterProfile {
                name,
                description,
                instructions,
            } => {
                self.rpc(
                    "agentProfile.register",
                    json!({
                        "name": name,
                        "description": description,
                        "instructions": instructions,
                    }),
                )
                .await?;
                Ok(diagnostic("agent.profile_registered", &name))
            }
        }
    }
}
