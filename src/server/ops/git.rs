use std::{cell::RefCell, rc::Rc, sync::Arc};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::{
    protocol::WorkspaceRootId,
    server::git::{GitCachedStatus, GitHeadState, GitRefreshState, GitRefreshStatus},
};

use super::ClayOpState;

#[op2]
#[string]
pub(super) async fn op_clay_git_list_statuses(
    state: Rc<RefCell<OpState>>,
) -> Result<String, JsErrorBox> {
    let op_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    let workspace = op_state.workspace();
    let workspace = workspace.lock().await;
    serialize_statuses(op_state.git_status_cache().list_cached(&workspace).await)
}

#[op2]
#[string]
pub(super) async fn op_clay_git_refresh_status(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let root_id = parse_workspace_root_id(&request_json)?;
    let op_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    let workspace = op_state.workspace();
    let root = {
        let workspace = workspace.lock().await;
        workspace
            .directory_roots()
            .into_iter()
            .find(|root| root.workspace_root_id == root_id)
    }
    .ok_or_else(|| {
        JsErrorBox::generic(format!(
            "clay.git.refresh_status_failed: unknown workspace root `{root_id}`"
        ))
    })?;

    serialize_status(
        op_state
            .git_status_cache()
            .refresh_root(root_id, root.canonical_path)
            .await,
    )
}

pub(super) fn git_cached_status_json(status: &GitCachedStatus) -> Value {
    json!({
        "workspaceRootId": status.workspace_root_id.to_string(),
        "workspaceRoot": status.workspace_root.to_string_lossy(),
        "snapshot": status.snapshot.as_ref().map(git_snapshot_json),
        "refreshState": git_refresh_state_json(&status.refresh_state),
    })
}

fn parse_workspace_root_id(json_text: &str) -> Result<WorkspaceRootId, JsErrorBox> {
    let value: Value = serde_json::from_str(json_text).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.git.refresh_status_failed: invalid request JSON: {error}"
        ))
    })?;
    value
        .get("workspaceRootId")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .or_else(|| value.get("workspaceRootId").and_then(Value::as_u64))
        .ok_or_else(|| {
            JsErrorBox::generic(
                "clay.git.refresh_status_failed: workspaceRootId must be a string or integer",
            )
        })
}

fn serialize_statuses(statuses: Vec<GitCachedStatus>) -> Result<String, JsErrorBox> {
    serde_json::to_string(&Value::Array(
        statuses.iter().map(git_cached_status_json).collect(),
    ))
    .map_err(serialize_error("clay.git.list_statuses_failed"))
}

fn serialize_status(status: GitCachedStatus) -> Result<String, JsErrorBox> {
    serde_json::to_string(&git_cached_status_json(&status))
        .map_err(serialize_error("clay.git.refresh_status_failed"))
}

fn git_snapshot_json(snapshot: &crate::server::git::GitStatusSnapshot) -> Value {
    json!({
        "workspaceRootId": snapshot.workspace_root_id.to_string(),
        "workspaceRoot": snapshot.workspace_root.to_string_lossy(),
        "repositoryRoot": snapshot.repository_root.as_ref().map(|path| path.to_string_lossy().to_string()),
        "head": git_head_json(&snapshot.head),
        "dirty": snapshot.dirty,
        "changedFileCount": snapshot.changed_file_count,
        "lastRefresh": git_refresh_status_json(&snapshot.last_refresh),
    })
}

fn git_head_json(head: &GitHeadState) -> Value {
    match head {
        GitHeadState::Branch(name) => json!({ "kind": "branch", "name": name }),
        GitHeadState::Detached(short_sha) => json!({ "kind": "detached", "shortSha": short_sha }),
        GitHeadState::Unborn => json!({ "kind": "unborn" }),
        GitHeadState::Unknown => json!({ "kind": "unknown" }),
    }
}

fn git_refresh_state_json(state: &GitRefreshState) -> Value {
    match state {
        GitRefreshState::Idle => json!({ "kind": "idle" }),
        GitRefreshState::Refreshing { started_at } => json!({
            "kind": "refreshing",
            "startedAtMillis": millis_since_epoch(*started_at),
        }),
        GitRefreshState::LastSuccess { finished_at } => json!({
            "kind": "last-success",
            "finishedAtMillis": millis_since_epoch(*finished_at),
        }),
        GitRefreshState::LastError {
            finished_at,
            status,
        } => json!({
            "kind": "last-error",
            "finishedAtMillis": millis_since_epoch(*finished_at),
            "status": git_refresh_status_json(status),
        }),
    }
}

fn git_refresh_status_json(status: &GitRefreshStatus) -> Value {
    match status {
        GitRefreshStatus::Success => json!({ "kind": "success" }),
        GitRefreshStatus::NonRepository => json!({ "kind": "non-repository" }),
        GitRefreshStatus::Timeout => json!({ "kind": "timeout" }),
        GitRefreshStatus::BoundaryRejected => json!({ "kind": "boundary-rejected" }),
        GitRefreshStatus::CommandError { command, message } => json!({
            "kind": "command-error",
            "command": command,
            "message": message,
        }),
        GitRefreshStatus::InvalidOutput { command, message } => json!({
            "kind": "invalid-output",
            "command": command,
            "message": message,
        }),
    }
}

fn millis_since_epoch(time: std::time::SystemTime) -> u128 {
    time.duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}
