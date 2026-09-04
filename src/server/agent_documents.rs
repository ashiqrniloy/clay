//! Reverse-RPC document operations served to the `clay-agent` daemon.
//!
//! The daemon's coding tools call `document.read` / `document.write` /
//! `document.mkdir` / `document.stat` over the agent stdio channel. Every
//! mutation stays server-authoritative: version, lease, and range checks run
//! through the existing document registry (`apply_edit` + CAS save). The
//! daemon never opens workspace files behind the registry for open documents.
//!
//! User-approval methods (`approval.request` mutation gate and
//! `approval.askUserDecision`) surface to connected clients through the
//! agent host's pending-approval bridge and fail closed on timeout or
//! missing consumer. Phase 1 ceilings: operations bind to the bootstrap
//! workspace (single-tab host model; multi-tab routing arrives with the
//! Phase 2 agent UI).

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

use super::agent::{AgentHost, ReverseRpcHandler};
use super::agent_checkpoints::AgentCheckpointStore;
use super::workspace::{
    WorkspaceError, WorkspaceRootId, WorkspaceState, open_existing_file_unlocked,
    save_document_unlocked,
};
use crate::protocol::ApprovalRequestKind;
use crate::protocol::{ClientId, EditOperation, ServerMessage};

/// Shared server-runtime identity: the agent acts like the JS runtime client,
/// never like a remote editor connection.
const AGENT_CLIENT_ID: ClientId = 0;
/// Transaction ids are client bookkeeping; a distinct range avoids confusion
/// in diagnostics, uniqueness is not enforced by `apply_edit`.
static AGENT_TRANSACTION: AtomicU64 = AtomicU64::new(1);

/// Maximum bytes the daemon may pull through one `document.read`.
const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;

/// Build the Phase 1 reverse-RPC handler bound to one workspace, the
/// document checkpoint store (task-boundary / user checkpoints + restore),
/// and the agent host approval bridge (user-approval surfacing).
pub(crate) fn document_reverse_handler(
    workspace: Arc<Mutex<WorkspaceState>>,
    checkpoints: Arc<Mutex<AgentCheckpointStore>>,
    agent: AgentHost,
) -> ReverseRpcHandler {
    Arc::new(move |method: String, params: Value| {
        let workspace = Arc::clone(&workspace);
        let checkpoints = Arc::clone(&checkpoints);
        let agent = agent.clone();
        Box::pin(async move { serve(&workspace, &checkpoints, &agent, &method, &params).await })
    })
}

async fn serve(
    workspace: &Arc<Mutex<WorkspaceState>>,
    checkpoints: &Arc<Mutex<AgentCheckpointStore>>,
    agent: &AgentHost,
    method: &str,
    params: &Value,
) -> Result<Value, String> {
    match method {
        "document.read" => document_read(workspace, params).await,
        "document.write" => document_write(workspace, params).await,
        "document.mkdir" => document_mkdir(workspace, params).await,
        "document.stat" => document_stat(workspace, params).await,
        "checkpoint.capture" => {
            let session_id = param_string(params, "sessionId")?;
            let entry_id = param_string(params, "entryId")?;
            checkpoints
                .lock()
                .await
                .checkpoint(workspace, &session_id, &entry_id)
                .await
        }
        "checkpoint.restore" => {
            let session_id = param_string(params, "sessionId")?;
            let entry_id = param_string(params, "entryId")?;
            checkpoints
                .lock()
                .await
                .restore(workspace, &session_id, &entry_id)
                .await
        }
        "checkpoint.list" => {
            // Bounded advisory list: which entries carry a document
            // checkpoint for this session (tree UI flags; no content).
            let session_id = param_string(params, "sessionId")?;
            let entry_ids = checkpoints.lock().await.list_for(&session_id);
            Ok(json!({ "sessionId": session_id, "entryIds": entry_ids }))
        }
        "approval.request" => {
            // Mutation gate: surface the daemon-produced action payload and
            // wait bounded for a user answer. Deny, timeout, and missing
            // consumer all fail closed (the daemon maps any error to deny).
            let payload = params
                .get("action")
                .cloned()
                .unwrap_or_else(|| params.clone());
            let payload_json = serde_json::to_string(&payload)
                .map_err(|error| format!("approval action is not serializable: {error}"))?;
            match agent
                .request_user_approval(ApprovalRequestKind::Mutation, &payload_json)
                .await
            {
                Ok(_) => Ok(json!({ "allowed": true })),
                Err(message) => Err(message),
            }
        }
        "approval.askUserDecision" => {
            // ask_user_decision tool: forward the bounded question/options
            // request and pass the validated answer back to the daemon.
            let payload_json = serde_json::to_string(params)
                .map_err(|error| format!("ask request is not serializable: {error}"))?;
            agent
                .request_user_approval(ApprovalRequestKind::AskDecision, &payload_json)
                .await
        }
        _ => Err(format!("unknown reverse method: {method}")),
    }
}

fn param_string(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("params.{key} is required"))
}

fn workspace_error_message(error: WorkspaceError) -> String {
    match error {
        WorkspaceError::OutsideRoot => "path outside workspace roots".to_string(),
        WorkspaceError::UnknownRoot { root_id } => format!("unknown workspace root {root_id}"),
        WorkspaceError::UnknownDocument { .. } => "document not open".to_string(),
        WorkspaceError::FileUnavailable { path, .. } => {
            format!("file unavailable: {}", path.display())
        }
        WorkspaceError::InvalidUtf8 { .. } => "file is not valid UTF-8".to_string(),
        other => format!("workspace error: {other}"),
    }
}

fn resolve_root(params: &Value, workspace: &WorkspaceState) -> Result<WorkspaceRootId, String> {
    if let Some(root_id) = params.get("workspaceRootId").and_then(Value::as_u64) {
        return Ok(root_id);
    }
    workspace
        .first_root_id()
        .ok_or_else(|| "no workspace roots configured".to_string())
}

async fn document_read(
    workspace: &Arc<Mutex<WorkspaceState>>,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let max_bytes = params
        .get("maxBytes")
        .and_then(Value::as_u64)
        .unwrap_or(MAX_READ_BYTES)
        .min(MAX_READ_BYTES);
    let (root_id, open_handle) = {
        let workspace = workspace.lock().await;
        let root_id = resolve_root(params, &workspace)?;
        let canonical = workspace
            .contained_existing_path(root_id, Path::new(&path))
            .map_err(workspace_error_message)?;
        let open_handle = workspace
            .find_open_document_by_canonical_path(&canonical)
            .and_then(|document_id| workspace.document_handle(document_id));
        (root_id, open_handle)
    };
    if let Some(handle) = open_handle {
        // Open document: the dirty buffer wins over disk. Read without
        // acquiring a lease so agent reads never block a user editor.
        let document = handle.lock().await;
        return Ok(json!({
            "text": document.text(),
            "version": document.version(),
            "dirty": document.is_dirty(),
            "open": true,
        }));
    }
    // Not open: read from disk, bounded, inside the workspace root.
    let bytes = read_contained_bytes(workspace, root_id, &path, max_bytes).await?;
    let text = String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8".to_string())?;
    Ok(json!({
        "text": text,
        "version": 1,
        "dirty": false,
        "open": false,
    }))
}

async fn read_contained_bytes(
    workspace: &Arc<Mutex<WorkspaceState>>,
    root_id: WorkspaceRootId,
    path: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let canonical = {
        let workspace = workspace.lock().await;
        workspace
            .contained_existing_path(root_id, Path::new(path))
            .map_err(workspace_error_message)?
    };
    let file = tokio::fs::File::open(&canonical)
        .await
        .map_err(|error| format!("file unavailable: {}: {error}", canonical.display()))?;
    let mut limited = file.take(max_bytes.saturating_add(1));
    let mut buffer = Vec::new();
    limited
        .read_to_end(&mut buffer)
        .await
        .map_err(|error| format!("read failed: {error}"))?;
    if buffer.len() as u64 > max_bytes {
        return Err(format!("file exceeds {max_bytes} byte read cap"));
    }
    Ok(buffer)
}

async fn document_write(
    workspace: &Arc<Mutex<WorkspaceState>>,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let content = param_string(params, "content")?;
    let root_id = {
        let workspace = workspace.lock().await;
        resolve_root(params, &workspace)?
    };
    let canonical = {
        let workspace = workspace.lock().await;
        match workspace.contained_existing_path(root_id, Path::new(&path)) {
            Ok(canonical) => Ok(Some(canonical)),
            Err(WorkspaceError::FileUnavailable { .. }) => Ok(None),
            Err(error) => Err(workspace_error_message(error)),
        }
    }?;
    if canonical.is_some() {
        // Existing file: route through open -> apply_edit -> save so version
        // and lease authority stay with the server.
        let lease = open_existing_file_unlocked(workspace, root_id, &path, AGENT_CLIENT_ID)
            .await
            .map_err(workspace_error_message)?;
        let document_id = lease.document_id;
        let Some(lease_id) = lease.access.lease_id() else {
            return Err("document lease held by another client".to_string());
        };
        let document = Arc::clone(&lease.document);
        let confirmed = {
            let mut document = document.lock().await;
            let base_version = document.version();
            let end = document.byte_len() as u64;
            let transaction_id = AGENT_TRANSACTION.fetch_add(1, Ordering::Relaxed);
            match document.apply_edit(
                document_id,
                AGENT_CLIENT_ID,
                Some(lease_id),
                base_version,
                transaction_id,
                EditOperation::Replace {
                    start: 0,
                    end,
                    text: content,
                },
            ) {
                ServerMessage::EditAck {
                    confirmed_version, ..
                } => Ok(confirmed_version),
                ServerMessage::EditRejected { reason, .. } => {
                    Err(format!("edit rejected: {reason:?}"))
                }
                other => Err(format!("unexpected edit response: {other:?}")),
            }
        }?;
        save_document_unlocked(workspace, document_id, AGENT_CLIENT_ID, confirmed)
            .await
            .map_err(workspace_error_message)?;
        let mut workspace = workspace.lock().await;
        workspace
            .release_single_document_access(document_id, AGENT_CLIENT_ID)
            .await;
        return Ok(json!({ "version": confirmed, "saved": true, "open": true }));
    }
    // New file: create parents and write inside the root. No registry entry;
    // the document does not exist yet.
    let canonical = {
        let workspace = workspace.lock().await;
        workspace
            .contained_new_file_path(root_id, Path::new(&path))
            .map_err(workspace_error_message)?
    };
    if let Some(parent) = canonical.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("mkdir failed: {error}"))?;
    }
    tokio::fs::write(&canonical, content.as_bytes())
        .await
        .map_err(|error| format!("write failed: {error}"))?;
    Ok(json!({ "version": 1, "saved": true, "open": false }))
}

async fn document_mkdir(
    workspace: &Arc<Mutex<WorkspaceState>>,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let root_id = {
        let workspace = workspace.lock().await;
        resolve_root(params, &workspace)?
    };
    let canonical = {
        let workspace = workspace.lock().await;
        workspace
            .contained_new_file_path(root_id, Path::new(&path))
            .map_err(workspace_error_message)?
    };
    tokio::fs::create_dir_all(&canonical)
        .await
        .map_err(|error| format!("mkdir failed: {error}"))?;
    Ok(json!({ "ok": true }))
}

async fn document_stat(
    workspace: &Arc<Mutex<WorkspaceState>>,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let canonical = {
        let workspace = workspace.lock().await;
        let root_id = resolve_root(params, &workspace)?;
        let open = workspace
            .contained_existing_path(root_id, Path::new(&path))
            .ok()
            .and_then(|canonical| {
                workspace
                    .find_open_document_by_canonical_path(&canonical)
                    .and_then(|document_id| workspace.document_handle(document_id))
            });
        if let Some(handle) = open {
            let document = handle.lock().await;
            return Ok(json!({ "size": document.byte_len(), "open": true }));
        }
        match workspace.contained_existing_path(root_id, Path::new(&path)) {
            Ok(canonical) => canonical,
            Err(_) => workspace
                .contained_new_file_path(root_id, Path::new(&path))
                .map_err(workspace_error_message)?,
        }
    };
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|error| format!("stat failed: {error}"))?;
    Ok(json!({ "size": metadata.len(), "open": false }))
}
#[cfg(test)]
mod tests {
    use super::*;

    async fn workspace_with_file(dir: &Path, name: &str, text: &str) -> Arc<Mutex<WorkspaceState>> {
        tokio::fs::create_dir_all(dir).await.unwrap();
        tokio::fs::write(dir.join(name), text).await.unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(dir).expect("root");
        Arc::new(Mutex::new(workspace))
    }

    #[tokio::test]
    async fn document_read_returns_disk_text_and_write_bumps_version() {
        let dir = std::env::temp_dir().join(format!(
            "clay-agent-docs-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = workspace_with_file(&dir, "notes.txt", "hello").await;
        let handler = document_reverse_handler(
            Arc::clone(&workspace),
            Arc::new(Mutex::new(AgentCheckpointStore::new())),
            AgentHost::inert(),
        );
        let path = dir.join("notes.txt");

        let read = handler(
            "document.read".to_string(),
            json!({ "path": path.to_str().unwrap() }),
        )
        .await
        .expect("read");
        assert_eq!(read["text"], "hello");
        assert_eq!(read["open"], false);

        let written = handler(
            "document.write".to_string(),
            json!({ "path": path.to_str().unwrap(), "content": "replaced" }),
        )
        .await
        .expect("write");
        assert_eq!(written["version"], 2);
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "replaced");
    }

    #[tokio::test]
    async fn document_write_rejects_paths_outside_roots() {
        let dir = std::env::temp_dir().join(format!(
            "clay-agent-docs-out-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&dir).expect("root");
        let workspace = Arc::new(Mutex::new(workspace));
        let handler = document_reverse_handler(
            workspace,
            Arc::new(Mutex::new(AgentCheckpointStore::new())),
            AgentHost::inert(),
        );
        let error = handler(
            "document.write".to_string(),
            json!({ "path": "/etc/passwd", "content": "no" }),
        )
        .await
        .expect_err("must be rejected");
        assert!(error.contains("outside workspace roots") || error.contains("unavailable"));
    }

    #[tokio::test]
    async fn document_write_creates_new_files_inside_root() {
        let dir = std::env::temp_dir().join(format!(
            "clay-agent-docs-new-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&dir).expect("root");
        let workspace = Arc::new(Mutex::new(workspace));
        let handler = document_reverse_handler(
            workspace,
            Arc::new(Mutex::new(AgentCheckpointStore::new())),
            AgentHost::inert(),
        );
        let path = dir.join("nested/new.txt");
        let written = handler(
            "document.write".to_string(),
            json!({ "path": path.to_str().unwrap(), "content": "fresh" }),
        )
        .await
        .expect("write");
        assert_eq!(written["version"], 1);
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "fresh");
    }

    #[tokio::test]
    async fn document_read_prefers_dirty_buffer_of_open_document() {
        let dir = std::env::temp_dir().join(format!(
            "clay-agent-docs-dirty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let file = dir.join("open.txt");
        tokio::fs::write(&file, "disk bytes").await.unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&dir).expect("root");
        let workspace = Arc::new(Mutex::new(workspace));
        {
            // Simulate a user holding the document open with unsaved edits.
            let root_id = workspace.lock().await.first_root_id().unwrap();
            let lease = open_existing_file_unlocked(&workspace, root_id, &file, 7)
                .await
                .expect("open");
            let document = Arc::clone(&lease.document);
            let mut document = document.lock().await;
            let version = document.version();
            let end = document.byte_len() as u64;
            match document.apply_edit(
                lease.document_id,
                7,
                lease.access.lease_id(),
                version,
                1,
                EditOperation::Replace {
                    start: 0,
                    end,
                    text: "dirty buffer".to_string(),
                },
            ) {
                ServerMessage::EditAck { .. } => {}
                other => panic!("expected edit ack, got {other:?}"),
            }
        }
        let handler = document_reverse_handler(
            workspace,
            Arc::new(Mutex::new(AgentCheckpointStore::new())),
            AgentHost::inert(),
        );
        let read = handler(
            "document.read".to_string(),
            json!({ "path": file.to_str().unwrap() }),
        )
        .await
        .expect("read");
        assert_eq!(read["text"], "dirty buffer");
        assert_eq!(read["dirty"], true);
        assert_eq!(read["open"], true);
    }
}
