//! `agent` domain ops (Phase 1): user-facing host controls forwarded to the
//! clay-agent daemon over its stdio RPC. Package JavaScript never receives a
//! daemon handle; these ops fail closed when no agent host is attached and
//! never grant filesystem/network/shell authority by existing.

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

/// This lane's agent authority, injected by server construction (Plan 130 A1).
/// No handle wired → fail closed naming the missing wiring; neither a process
/// global nor a missing host ever grants authority.
fn lane_host(
    state: &Rc<RefCell<OpState>>,
) -> Result<crate::server::agent::AgentHostHandle, JsErrorBox> {
    state
        .borrow()
        .borrow::<std::sync::Arc<super::ClayOpState>>()
        .agent_host()
        .map_err(|_| {
            JsErrorBox::generic("agent.unavailable: no agent host is attached to this runtime")
        })
}

async fn agent_rpc(
    state: &Rc<RefCell<OpState>>,
    method: &str,
    params: Value,
) -> Result<String, JsErrorBox> {
    let host = lane_host(state)?;
    let result = host
        .rpc(method, params)
        .await
        .map_err(|error| JsErrorBox::generic(format!("agent.rpc_failed: {error}")))?;
    serde_json::to_string(&result)
        .map_err(|error| JsErrorBox::generic(format!("agent.encode_failed: {error}")))
}

/// Structural pre-validation shared by both registration paths: the daemon
/// still owns semantic validation (duplicate names, tool/skill resolution),
/// but malformed declarations never enter a queue.
fn validate_registration_shape(method: &str, params: &Value) -> Result<(), JsErrorBox> {
    let invalid =
        |detail: &str| JsErrorBox::generic(format!("agent.invalid_params: {method} {detail}"));
    if method == "knowledge.setOptions" {
        // Knowledge options carry a workspace root instead of a name; the
        // wiki/graft flags are validated daemon-side (fail closed there).
        if params
            .get("workspaceRoot")
            .and_then(Value::as_str)
            .is_none_or(|root| root.trim().is_empty())
        {
            return Err(invalid("requires a non-empty string `workspaceRoot`"));
        }
        let wiki = params.get("wiki");
        let graft = params.get("graft");
        for (name, value) in [("wiki", wiki), ("graft", graft)] {
            if !value.is_none_or(Value::is_boolean) {
                return Err(invalid(&format!("`{name}` must be a boolean when present")));
            }
        }
        if wiki.is_none() && graft.is_none() {
            return Err(invalid("requires a boolean `wiki` and/or `graft` flag"));
        }
        // Plan 121 follow-up: `graftDeepModel` configures the graft extension,
        // so it only exists beside `graft: true`. Shape only — the daemon owns
        // provider-id and credential resolution.
        if let Some(deep) = params.get("graftDeepModel") {
            if graft != Some(&Value::Bool(true)) {
                return Err(invalid("`graftDeepModel` requires `graft: true`"));
            }
            let Some(deep) = deep.as_object() else {
                return Err(invalid("`graftDeepModel` must be an object"));
            };
            for name in ["provider", "model"] {
                let ok = deep
                    .get(name)
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty());
                if !ok {
                    return Err(invalid(&format!(
                        "`graftDeepModel.{name}` must be a non-empty string"
                    )));
                }
            }
            for name in ["apiKey", "baseUrl"] {
                let Some(value) = deep.get(name) else {
                    continue;
                };
                let ok = value
                    .as_str()
                    .is_some_and(|value| name == "baseUrl" || !value.is_empty());
                if !ok {
                    return Err(invalid(&format!(
                        "`graftDeepModel.{name}` must be a string{}",
                        if name == "baseUrl" {
                            ""
                        } else {
                            " (non-empty)"
                        }
                    )));
                }
            }
            return Ok(());
        }
        return Ok(());
    }
    if method == "run.setOptions" {
        let compaction = params.get("compaction");
        let compact_after = params.get("compactAfterTokens");
        const POLICY: &[&str] = &[
            "maxInputTokens",
            "maxOutputTokens",
            "maxTurns",
            "maxToolRounds",
            "maxToolCalls",
            "maxWallTimeMs",
        ];
        if POLICY.iter().all(|name| params.get(*name).is_none())
            && compact_after.is_none()
            && compaction.is_none()
        {
            return Err(invalid(
                "requires a policy cap, compactAfterTokens, and/or compaction",
            ));
        }
        for name in POLICY {
            let Some(value) = params.get(*name) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let Some(n) = value.as_u64() else {
                return Err(invalid(&format!(
                    "`{name}` must be a positive integer or null"
                )));
            };
            if n < 1 {
                return Err(invalid(&format!(
                    "`{name}` must be a positive integer or null"
                )));
            }
        }
        if let Some(value) = compact_after {
            let Some(n) = value.as_u64() else {
                return Err(invalid("`compactAfterTokens` must be a positive integer"));
            };
            if n < 1 {
                return Err(invalid("`compactAfterTokens` must be a positive integer"));
            }
        }
        if let Some(value) = compaction {
            let Some(name) = value.as_str() else {
                return Err(invalid("`compaction` must be one of default|llm|om"));
            };
            if !matches!(name, "default" | "llm" | "om") {
                return Err(invalid("`compaction` must be one of default|llm|om"));
            }
        }
        return Ok(());
    }
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| invalid("requires a non-empty string `name`"))?;
    for field in ["tools", "skills", "toolNames"] {
        if let Some(list) = params.get(field)
            && !list
                .as_array()
                .is_some_and(|items| items.iter().all(|item| item.is_string()))
        {
            return Err(invalid(&format!(
                "`{field}` must be an array of strings (profile `{name}`)"
            )));
        }
    }
    Ok(())
}

/// Registration path for package load entries. Validation first (fail
/// closed, typed error). With a host attached: queue server-side while the
/// daemon is down instead of spawning it or blocking on its boot. Without a
/// host (hostless runtimes): queue on this lane's own state so the declaration
/// is handed over when a host is attached to the lane — a load entry never
/// fails just because this runtime has no agent subsystem (plan 130 A1: no
/// process-global authority or queue).
async fn agent_registration_rpc(
    state: &Rc<RefCell<OpState>>,
    method: &str,
    params: Value,
) -> Result<String, JsErrorBox> {
    validate_registration_shape(method, &params)?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match lane_host(state) {
        Ok(host) => {
            let result = host.rpc_or_queue(method, params).await;
            eprintln!("[agent-reg] {method} '{name}' host=live -> {result:?}");
            let result = result
                .map_err(|error| JsErrorBox::generic(format!("agent.rpc_failed: {error}")))?;
            serde_json::to_string(&result)
                .map_err(|error| JsErrorBox::generic(format!("agent.encode_failed: {error}")))
        }
        Err(_) => {
            eprintln!("[agent-reg] {method} '{name}' host=absent -> queued");
            state
                .borrow()
                .borrow::<std::sync::Arc<super::ClayOpState>>()
                .queue_agent_registration(method, params)
                .map_err(|error| JsErrorBox::generic(format!("agent.rpc_failed: {error}")))?;
            Ok(r#"{"queued":true}"#.to_string())
        }
    }
}

fn require_session(params: &Value) -> Result<String, JsErrorBox> {
    Ok(params
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| JsErrorBox::generic("agent.invalid_params: sessionId is required"))?
        .to_string())
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_set_autonomy(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    let session_id = require_session(&params)?;
    let enabled = params
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| JsErrorBox::generic("agent.invalid_params: enabled must be a boolean"))?;
    if !enabled {
        // Disabling autonomy on an unknown/unlive session is a no-op success:
        // the fail-closed default already applies.
        return agent_rpc(
            &state,
            "session.setAutonomy",
            json!({ "sessionId": session_id, "enabled": false }),
        )
        .await;
    }
    agent_rpc(
        &state,
        "session.setAutonomy",
        json!({ "sessionId": session_id, "enabled": true }),
    )
    .await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_compact_session(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    let session_id = require_session(&params)?;
    let mut rpc_params = json!({ "sessionId": session_id });
    if let Some(strategy) = params.get("strategy") {
        if !strategy.is_string() {
            return Err(JsErrorBox::generic(
                "agent.invalid_params: strategy must be a string",
            ));
        }
        rpc_params["strategy"] = strategy.clone();
    }
    if let Some(threshold) = params.get("compactAfterTokens") {
        let Some(threshold) = threshold.as_u64().filter(|threshold| *threshold > 0) else {
            return Err(JsErrorBox::generic(
                "agent.invalid_params: compactAfterTokens must be a positive integer",
            ));
        };
        rpc_params["compactAfterTokens"] = json!(threshold);
    }
    agent_rpc(&state, "session.compact", rpc_params).await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_search_sessions(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    let session_id = require_session(&params)?;
    let mut rpc_params = json!({ "sessionId": session_id });
    if let Some(query) = params.get("query") {
        if !query.is_string() {
            return Err(JsErrorBox::generic(
                "agent.invalid_params: query must be a string",
            ));
        }
        rpc_params["query"] = query.clone();
    }
    if let Some(limit) = params.get("limit") {
        let Some(limit) = limit.as_u64() else {
            return Err(JsErrorBox::generic(
                "agent.invalid_params: limit must be a non-negative integer",
            ));
        };
        rpc_params["limit"] = json!(limit);
    }
    // Workspace scoping is server-owned: the daemon filters by the session's
    // stamped workspaceRoot; hits are metadata, never injected into context.
    agent_rpc(&state, "session.search", rpc_params).await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_resume_run(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    let session_id = require_session(&params)?;
    let run_id = params
        .get("runId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| JsErrorBox::generic("agent.invalid_params: runId is required"))?
        .to_string();
    let decision = params
        .get("decision")
        .filter(|decision| decision.is_object())
        .ok_or_else(|| {
            JsErrorBox::generic(
                "agent.invalid_params: decision must be an object (daemon validates its shape fail-closed)",
            )
        })?
        .clone();
    // The daemon validates decision shape/version and refuses stale resumes;
    // this op only forwards user intent and adds no authority.
    agent_rpc(
        &state,
        "run.resume",
        json!({ "sessionId": session_id, "runId": run_id, "decision": decision }),
    )
    .await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_session_tree(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    let session_id = require_session(&params)?;
    let entry_id = params
        .get("entryId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| JsErrorBox::generic("agent.invalid_params: entryId is required"))?
        .to_string();
    let method = params
        .get("method")
        .and_then(Value::as_str)
        .filter(|method| matches!(*method, "checkout" | "fork" | "clone" | "checkpoint"))
        .ok_or_else(|| {
            JsErrorBox::generic(
                "agent.invalid_params: method must be checkout, fork, clone, or checkpoint",
            )
        })?
        .to_string();
    agent_rpc(
        &state,
        &format!("session.{method}"),
        json!({ "sessionId": session_id, "entryId": entry_id }),
    )
    .await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_profile_register(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    // The daemon validates the profile shape and fails closed on duplicates;
    // this op only forwards the package's inert profile declaration. Queued
    // (never spawned) while the daemon is down so load entries stay fast.
    agent_registration_rpc(&state, "agentProfile.register", params).await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_skill_register(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    // The daemon validates the skill shape (duplicate names fail closed);
    // this op only forwards the package's inert skill declaration. Queued
    // (never spawned) while the daemon is down so load entries stay fast.
    agent_registration_rpc(&state, "skill.register", params).await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_knowledge_set_options(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    // The daemon validates the flags and owns the opt-in activation
    // (decision 2156); this op only forwards the configuration intent.
    // Queued (never spawned) while the daemon is down so a load entry
    // configuring knowledge options applies after the host initializes.
    agent_registration_rpc(&state, "knowledge.setOptions", params).await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_run_set_options(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    // Daemon owns policy caps (Prism 0.5.4: number | null, no product HARD).
    // Queued while the daemon is down so init.js never blocks on boot.
    agent_registration_rpc(&state, "run.setOptions", params).await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_command_register(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    // The daemon validates the handler name and duplicate names; this op only
    // forwards the package's inert command declaration. Queued (never
    // spawned) while the daemon is down so load entries stay fast.
    agent_registration_rpc(&state, "command.register", params).await
}

#[op2]
#[string]
pub(super) async fn op_clay_agent_command_dispatch(
    state: Rc<RefCell<OpState>>,
    #[string] params_json: String,
) -> Result<String, JsErrorBox> {
    let params: Value = serde_json::from_str(&params_json)
        .map_err(|error| JsErrorBox::generic(format!("agent.invalid_params: {error}")))?;
    if params
        .get("name")
        .and_then(Value::as_str)
        .is_none_or(|name| name.trim().is_empty())
    {
        return Err(JsErrorBox::generic(
            "agent.invalid_params: command.dispatch requires a non-empty string `name`",
        ));
    }
    if let Some(args) = params.get("args")
        && !args.is_object()
    {
        return Err(JsErrorBox::generic(
            "agent.invalid_params: command.dispatch `args` must be an object",
        ));
    }
    if let Some(session_id) = params.get("sessionId")
        && !session_id.is_string()
    {
        return Err(JsErrorBox::generic(
            "agent.invalid_params: command.dispatch `sessionId` must be a string",
        ));
    }
    // Live dispatch only: commands act on the daemon's session state, so this
    // op never queues — an unavailable daemon is a typed failure, not a
    // deferred execution (unlike inert registrations).
    agent_rpc(&state, "command.dispatch", params).await
}

#[cfg(test)]
mod tests {
    use super::validate_registration_shape;
    use serde_json::json;

    fn rejects(params: serde_json::Value, needle: &str) {
        let error = validate_registration_shape("knowledge.setOptions", &params)
            .expect_err("must fail closed");
        let message = error.to_string();
        assert!(
            message.contains(needle),
            "expected `{needle}` in `{message}` for {params}"
        );
    }

    #[test]
    fn graft_deep_model_shape_fails_closed_before_queueing() {
        // Plan 121 follow-up: the deep model is graft-extension configuration,
        // and the key/model identity must never enter the registration queue
        // half-formed (the daemon still owns provider and credential checks).
        rejects(
            json!({
                "workspaceRoot": "/tmp/ws",
                "wiki": true,
                "graftDeepModel": { "provider": "anthropic", "model": "m" }
            }),
            "requires `graft: true`",
        );
        rejects(
            json!({
                "workspaceRoot": "/tmp/ws",
                "graft": true,
                "graftDeepModel": "anthropic"
            }),
            "must be an object",
        );
        rejects(
            json!({
                "workspaceRoot": "/tmp/ws",
                "graft": true,
                "graftDeepModel": { "provider": " ", "model": "m" }
            }),
            "`graftDeepModel.provider` must be a non-empty string",
        );
        rejects(
            json!({
                "workspaceRoot": "/tmp/ws",
                "graft": true,
                "graftDeepModel": { "provider": "openai", "model": "" }
            }),
            "`graftDeepModel.model` must be a non-empty string",
        );
        rejects(
            json!({
                "workspaceRoot": "/tmp/ws",
                "graft": true,
                "graftDeepModel": { "provider": "openai", "model": "m", "apiKey": 7 }
            }),
            "`graftDeepModel.apiKey` must be a string (non-empty)",
        );
        // A well-formed shape (with or without the optional key) still passes:
        // an omitted key is the vault path, not a malformed one.
        for extra in [json!({}), json!({ "apiKey": "k" })] {
            let mut deep = json!({ "provider": "anthropic", "model": "m" });
            deep.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            validate_registration_shape(
                "knowledge.setOptions",
                &json!({
                    "workspaceRoot": "/tmp/ws",
                    "graft": true,
                    "graftDeepModel": deep
                }),
            )
            .expect("well-formed deep model passes shape validation");
        }
    }
}
