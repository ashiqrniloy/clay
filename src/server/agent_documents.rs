//! Reverse-RPC document operations served to the `clay-agent` daemon.
//!
//! The daemon's coding tools call `document.read` / `document.write` /
//! `document.mkdir` / `document.stat` over the agent stdio channel. Every
//! mutation stays server-authoritative: version, lease, and range checks run
//! through the existing document registry (`apply_edit` + CAS save). The
//! daemon never opens workspace files behind the registry for open documents.
//!
//! **Workspace authority (plan 119 SC-6, decision 2026-09-14-1705).** Every
//! call names its session, and the session's recorded workspace root decides
//! which folder the operation may touch: the root is resolved to the tab
//! state that has that folder open, so a session's tools can never address a
//! root it does not own. When the session's root (or the root's state)
//! cannot be resolved the call fails closed with an `agent.workspace_unresolved`
//! diagnostic — there is no launch-root or first-root fallback, and no
//! `workspaceRootId` parameter to address a root the session does not own.
//!
//! User-approval methods (`approval.request` mutation gate and
//! `approval.askUserDecision`) surface to connected clients through the
//! agent host's pending-approval bridge and fail closed on timeout or
//! missing consumer.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

use super::agent::{AgentHost, ReverseRpcHandler};
use super::agent_checkpoints::AgentCheckpointStore;
use super::tab_registry::TabRegistry;
use super::workspace::{
    WorkspaceError, WorkspaceRootId, WorkspaceState, atomic_create_file,
    open_existing_file_unlocked, save_document_unlocked,
};
use crate::protocol::AgentServerMessage;
use crate::protocol::ApprovalRequestKind;
use crate::protocol::{ClientId, EditOperation, ServerMessage, TabId};
use crate::server::TabServerState;

/// Shared server-runtime identity: the agent acts like the JS runtime client,
/// never like a remote editor connection.
const AGENT_CLIENT_ID: ClientId = 0;
/// Transaction ids are client bookkeeping; a distinct range avoids confusion
/// in diagnostics, uniqueness is not enforced by `apply_edit`.
static AGENT_TRANSACTION: AtomicU64 = AtomicU64::new(1);

/// Maximum bytes the daemon may pull through one `document.read`.
const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;

/// Resolves a session's workspace root to the workspace state that has that
/// folder open (plan 119 SC-6). Root ids are per tab state, so the root
/// *path* is the cross-state identity: the tab registry is the single source
/// of tab→root truth, and the state map holds the state each tab edits
/// through. `None` = no tab has that folder open → the caller fails closed.
#[derive(Clone)]
pub(crate) struct SessionWorkspaces {
    tab_registry: Arc<Mutex<TabRegistry>>,
    tab_states: Arc<Mutex<HashMap<TabId, TabServerState>>>,
}

/// One resolved agent workspace: the state to operate through and the state's
/// own root id for that folder.
struct AgentWorkspace {
    workspace: Arc<Mutex<WorkspaceState>>,
    root_id: WorkspaceRootId,
}

impl SessionWorkspaces {
    pub(crate) fn new(
        tab_registry: Arc<Mutex<TabRegistry>>,
        tab_states: Arc<Mutex<HashMap<TabId, TabServerState>>>,
    ) -> Self {
        Self {
            tab_registry,
            tab_states,
        }
    }

    /// The state + root id a session's recorded root is open in. Locks are
    /// taken one at a time (registry, then states, then the workspace) and
    /// never nested.
    async fn resolve(&self, root_path: &str) -> Option<AgentWorkspace> {
        let canonical = tokio::fs::canonicalize(root_path).await.ok()?;
        let tab_ids: Vec<TabId> = {
            let registry = self.tab_registry.lock().await;
            let mut tabs: Vec<TabId> = registry
                .snapshot()
                .tabs
                .iter()
                .filter(|tab| tab.workspace_root == root_path)
                .map(|tab| tab.tab_id)
                .collect();
            tabs.sort_unstable();
            tabs
        };
        let states: Vec<Arc<Mutex<WorkspaceState>>> = {
            let states = self.tab_states.lock().await;
            tab_ids
                .iter()
                .filter_map(|tab| states.get(tab))
                .map(|state| Arc::clone(&state.workspace))
                .collect()
        };
        for workspace in states {
            let root_id = {
                let workspace = workspace.lock().await;
                workspace.root_id_for_canonical_path(&canonical)
            };
            if let Some(root_id) = root_id {
                return Some(AgentWorkspace { workspace, root_id });
            }
        }
        None
    }
}

/// Build the reverse-RPC handler: session-scoped workspace resolution (plan
/// 119 SC-6), the document checkpoint store (task-boundary / user checkpoints
/// + restore), and the agent host approval bridge.
pub(crate) fn document_reverse_handler(
    workspaces: SessionWorkspaces,
    checkpoints: Arc<Mutex<AgentCheckpointStore>>,
    agent: AgentHost,
) -> ReverseRpcHandler {
    Arc::new(move |method: String, params: Value| {
        let workspaces = workspaces.clone();
        let checkpoints = Arc::clone(&checkpoints);
        let agent = agent.clone();
        Box::pin(async move { serve(&workspaces, &checkpoints, &agent, &method, &params).await })
    })
}

async fn serve(
    workspaces: &SessionWorkspaces,
    checkpoints: &Arc<Mutex<AgentCheckpointStore>>,
    agent: &AgentHost,
    method: &str,
    params: &Value,
) -> Result<Value, String> {
    match method {
        "document.read" => document_read(workspaces, agent, params).await,
        "document.write" => document_write(workspaces, agent, params).await,
        "document.mkdir" => document_mkdir(workspaces, agent, params).await,
        "document.stat" => document_stat(workspaces, agent, params).await,
        "checkpoint.capture" => {
            let entry_id = param_string(params, "entryId")?;
            let (workspace, root_id) = session_workspace(workspaces, agent, params).await?;
            let session_id = param_string(params, "sessionId")?;
            checkpoints
                .lock()
                .await
                .checkpoint(&workspace, root_id, &session_id, &entry_id)
                .await
        }
        "checkpoint.restore" => {
            let entry_id = param_string(params, "entryId")?;
            let (workspace, root_id) = session_workspace(workspaces, agent, params).await?;
            let session_id = param_string(params, "sessionId")?;
            checkpoints
                .lock()
                .await
                .restore(&workspace, root_id, &session_id, &entry_id)
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

/// Resolve one call's workspace: the session's recorded root, in the tab state
/// that has that folder open (plan 119 SC-6). Fails closed — with an
/// `agent.workspace_unresolved` diagnostic — when the session or its root
/// cannot be resolved, rather than falling back to any other root.
async fn session_workspace(
    workspaces: &SessionWorkspaces,
    agent: &AgentHost,
    params: &Value,
) -> Result<(Arc<Mutex<WorkspaceState>>, WorkspaceRootId), String> {
    let session_id = param_string(params, "sessionId")?;
    let Some(root) = agent.session_workspace_root(&session_id).await else {
        return Err(unresolved(agent, &session_id, None));
    };
    match workspaces.resolve(&root).await {
        Some(resolved) => Ok((resolved.workspace, resolved.root_id)),
        None => Err(unresolved(agent, &session_id, Some(&root))),
    }
}

/// The fail-closed answer for an unresolvable workspace: the tool call errors
/// (the daemon turns that into the model's tool result) and the view gets a
/// diagnostic so the failure is visible rather than silent.
fn unresolved(agent: &AgentHost, session_id: &str, root: Option<&str>) -> String {
    let message = match root {
        Some(root) => format!(
            "agent.workspace_unresolved: session {session_id} workspace root is not open: {root}"
        ),
        None => format!(
            "agent.workspace_unresolved: session {session_id} has no recorded workspace root"
        ),
    };
    agent.broadcast(AgentServerMessage::Diagnostic {
        code: "agent.workspace_unresolved".to_string(),
        message: message.clone(),
    });
    message
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

async fn document_read(
    workspaces: &SessionWorkspaces,
    agent: &AgentHost,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let (workspace, root_id) = session_workspace(workspaces, agent, params).await?;
    let max_bytes = params
        .get("maxBytes")
        .and_then(Value::as_u64)
        .unwrap_or(MAX_READ_BYTES)
        .min(MAX_READ_BYTES);
    let open_handle = {
        let workspace = workspace.lock().await;
        let canonical = workspace
            .contained_existing_path(root_id, Path::new(&path))
            .await
            .map_err(workspace_error_message)?;
        workspace
            .find_open_document_by_canonical_path(&canonical)
            .and_then(|document_id| workspace.document_handle(document_id))
    };
    if let Some(handle) = open_handle {
        // Open document: the dirty buffer wins over disk. Read without
        // acquiring a lease so agent reads never block a user editor.
        // Bounded like the disk path (plan 119 P2-1): slice the rope at the cap
        // instead of materializing a whole 256MB resident buffer only to cut it,
        // and tell the agent when the text it gets is not the whole document.
        let document = handle.lock().await;
        let total_bytes = document.byte_len();
        let cap = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        let truncated = total_bytes > cap;
        let text = if truncated {
            document.bounded_prefix(cap)
        } else {
            document.text()
        };
        return Ok(json!({
            "text": text,
            "version": document.version(),
            "dirty": document.is_dirty(),
            "open": true,
            "truncated": truncated,
            "totalBytes": total_bytes,
        }));
    }
    // Not open: read from disk, bounded, inside the workspace root.
    let bytes = read_contained_bytes(&workspace, root_id, &path, max_bytes).await?;
    let total_bytes = bytes.len();
    let text = String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8".to_string())?;
    Ok(json!({
        "text": text,
        "version": 1,
        "dirty": false,
        "open": false,
        // The disk path refuses oversize files outright, so this response is
        // never a prefix; both paths report the fields the agent checks.
        "truncated": false,
        "totalBytes": total_bytes,
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
            .await
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
    workspaces: &SessionWorkspaces,
    agent: &AgentHost,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let content = param_string(params, "content")?;
    let (workspace, root_id) = session_workspace(workspaces, agent, params).await?;
    let canonical = {
        let workspace = workspace.lock().await;
        match workspace
            .contained_existing_path(root_id, Path::new(&path))
            .await
        {
            Ok(canonical) => Ok(Some(canonical)),
            Err(WorkspaceError::FileUnavailable { .. }) => Ok(None),
            Err(error) => Err(workspace_error_message(error)),
        }
    }?;
    if canonical.is_some() {
        // Existing file: route through open -> apply_edit -> save so version
        // and lease authority stay with the server.
        let lease = open_existing_file_unlocked(&workspace, root_id, &path, AGENT_CLIENT_ID)
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
        save_document_unlocked(&workspace, document_id, AGENT_CLIENT_ID, confirmed)
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
            .await
            .map_err(workspace_error_message)?
    };
    if let Some(parent) = canonical.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("mkdir failed: {error}"))?;
    }
    // Atomic (temp + fsync + rename), same primitive as the open/save path: a
    // crash mid-write must not leave a partial file behind a `saved: true` the
    // agent already reported, and a power loss must not lose it (plan 119 P2-2).
    atomic_create_file(&canonical, content.as_bytes())
        .await
        .map_err(|error| format!("write failed: {error}"))?;
    Ok(json!({ "version": 1, "saved": true, "open": false }))
}

async fn document_mkdir(
    workspaces: &SessionWorkspaces,
    agent: &AgentHost,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let (workspace, root_id) = session_workspace(workspaces, agent, params).await?;
    let canonical = {
        let workspace = workspace.lock().await;
        workspace
            .contained_new_file_path(root_id, Path::new(&path))
            .await
            .map_err(workspace_error_message)?
    };
    tokio::fs::create_dir_all(&canonical)
        .await
        .map_err(|error| format!("mkdir failed: {error}"))?;
    Ok(json!({ "ok": true }))
}

async fn document_stat(
    workspaces: &SessionWorkspaces,
    agent: &AgentHost,
    params: &Value,
) -> Result<Value, String> {
    let path = param_string(params, "path")?;
    let (workspace, root_id) = session_workspace(workspaces, agent, params).await?;
    let open_handle = {
        let workspace = workspace.lock().await;
        workspace
            .contained_existing_path(root_id, Path::new(&path))
            .await
            .ok()
            .and_then(|canonical| {
                workspace
                    .find_open_document_by_canonical_path(&canonical)
                    .and_then(|document_id| workspace.document_handle(document_id))
            })
    };
    if let Some(handle) = open_handle {
        let document = handle.lock().await;
        return Ok(json!({ "size": document.byte_len(), "open": true }));
    }
    let canonical = {
        let workspace = workspace.lock().await;
        match workspace
            .contained_existing_path(root_id, Path::new(&path))
            .await
        {
            Ok(canonical) => canonical,
            Err(_) => workspace
                .contained_new_file_path(root_id, Path::new(&path))
                .await
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
    use crate::server::{DocumentAccess, DocumentState};
    use std::sync::atomic::AtomicBool;

    async fn workspace_with_file(dir: &Path, name: &str, text: &str) -> Arc<Mutex<WorkspaceState>> {
        tokio::fs::create_dir_all(dir).await.unwrap();
        tokio::fs::write(dir.join(name), text).await.unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(dir).expect("root");
        Arc::new(Mutex::new(workspace))
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "clay-agent-docs-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// Register `dir` as a root and open `<dir>/<name>` with its buffer
    /// replaced by `content`: a dirty resident document, the state
    /// `document_read` has to bound (plan 119 P2-1).
    async fn open_dirty_document(
        dir: &Path,
        name: &str,
        content: &str,
    ) -> (Arc<Mutex<WorkspaceState>>, std::path::PathBuf) {
        let workspace = workspace_with_file(dir, name, "disk bytes").await;
        let file = dir.join(name);
        {
            // Simulate a user holding the document open with unsaved edits.
            let root_id = workspace.lock().await.directory_roots()[0].workspace_root_id;
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
                    text: content.to_string(),
                },
            ) {
                ServerMessage::EditAck { .. } => {}
                other => panic!("expected edit ack, got {other:?}"),
            }
        }
        (workspace, file)
    }

    /// The session every single-tab test call names (plan 119 SC-6: a
    /// `document.*` call is scoped by the session's recorded workspace root).
    const SESSION: &str = "session-1";

    fn tab_state(workspace: Arc<Mutex<WorkspaceState>>) -> TabServerState {
        TabServerState {
            workspace,
            welcome: Arc::new(Mutex::new(DocumentState::new(
                1,
                String::new(),
                DocumentAccess::Editable { lease_id: 1 },
            ))),
            workspace_pane_visible: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Multi-tab harness (plan 119 SC-6): each `(tab, root, workspace)` is a
    /// live tab the resolver can find a session's root in.
    async fn handler_for_tabs(
        host: &AgentHost,
        tabs: &[(TabId, &Path, Arc<Mutex<WorkspaceState>>)],
    ) -> ReverseRpcHandler {
        let registry = Arc::new(Mutex::new(TabRegistry::new()));
        let states = Arc::new(Mutex::new(HashMap::new()));
        for (tab, root, workspace) in tabs {
            registry
                .lock()
                .await
                .create_tab(*tab, 7, root.to_string_lossy().into_owned());
            states
                .lock()
                .await
                .insert(*tab, tab_state(Arc::clone(workspace)));
        }
        document_reverse_handler(
            SessionWorkspaces::new(registry, states),
            Arc::new(Mutex::new(AgentCheckpointStore::new())),
            host.clone(),
        )
    }

    /// One tab whose workspace root is `dir`, with `SESSION` recorded against
    /// that root — the shape a real tab + session has.
    async fn handler_for(
        dir: &Path,
        workspace: Arc<Mutex<WorkspaceState>>,
    ) -> (ReverseRpcHandler, AgentHost) {
        let host = AgentHost::inert();
        host.record_session_root_for_test(SESSION, &dir.to_string_lossy())
            .await;
        let handler = handler_for_tabs(&host, &[(1, dir, workspace)]).await;
        (handler, host)
    }

    /// Dispatch one call with the harness session stamped on it.
    async fn call(
        handler: &ReverseRpcHandler,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        call_as(handler, SESSION, method, params).await
    }

    async fn call_as(
        handler: &ReverseRpcHandler,
        session_id: &str,
        method: &str,
        mut params: Value,
    ) -> Result<Value, String> {
        params["sessionId"] = json!(session_id);
        handler(method.to_string(), params).await
    }

    #[tokio::test]
    async fn document_write_creates_new_files_through_the_atomic_path() {
        // Nested missing parents (mkdir) plus a brand-new file (plan 119 P2-2).
        let dir = temp_dir("newfile");
        let workspace = workspace_with_file(&dir, "existing.txt", "x").await;
        let (handler, _host) = handler_for(&dir, workspace).await;
        let target = dir.join("nested/deeper/new.md");

        let result = call(
            &handler,
            "document.write",
            json!({ "path": target.to_str().unwrap(), "content": "fresh content" }),
        )
        .await
        .expect("write");
        assert_eq!(result["saved"], true);
        assert_eq!(result["open"], false);
        assert_eq!(result["version"], 1);
        assert_eq!(
            tokio::fs::read_to_string(&target).await.unwrap(),
            "fresh content"
        );

        // The temp file lands next to the target and is renamed over it: no
        // `.clay-save-*` litter may survive a successful write.
        let mut entries = tokio::fs::read_dir(target.parent().unwrap()).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().into_owned();
            assert!(!name.contains(".clay-save-"), "leftover temp file: {name}");
        }

        // The atomic path's temp starts `0o600` and a brand-new file keeps it;
        // a plain `fs::write` would have created the file via umask (0o644).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = tokio::fs::metadata(&target)
                .await
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "new files must not be group/world readable");
        }
    }

    #[tokio::test]
    async fn document_read_caps_the_dirty_buffer_and_flags_the_cut() {
        let dir = temp_dir("cap");
        let (workspace, file) = open_dirty_document(&dir, "big.txt", &"x".repeat(4096)).await;
        let (handler, _host) = handler_for(&dir, workspace).await;
        let path = file.to_str().unwrap();

        let read = call(
            &handler,
            "document.read",
            json!({ "path": path, "maxBytes": 1024 }),
        )
        .await
        .expect("read");
        assert_eq!(read["text"].as_str().unwrap().len(), 1024);
        assert_eq!(read["truncated"], true);
        assert_eq!(read["totalBytes"], 4096);
        assert_eq!(read["open"], true);
        assert_eq!(read["dirty"], true);
        // Same document under the cap: whole text, no flag.
        let full = call(
            &handler,
            "document.read",
            json!({ "path": path, "maxBytes": 8192 }),
        )
        .await
        .expect("read");
        assert_eq!(full["text"].as_str().unwrap().len(), 4096);
        assert_eq!(full["truncated"], false);
        assert_eq!(full["totalBytes"], 4096);
    }

    #[tokio::test]
    async fn document_read_never_splits_a_scalar_at_the_dirty_buffer_cap() {
        // 1023 ASCII bytes then a 4-byte scalar: a naive 1024-byte cut would
        // land inside the scalar and ship invalid UTF-8.
        let total = 1023 + "😀".len() + 64;
        let dir = temp_dir("scalar");
        let (workspace, file) = open_dirty_document(
            &dir,
            "scalar.txt",
            &format!("{}😀{}", "a".repeat(1023), "b".repeat(64)),
        )
        .await;
        let (handler, _host) = handler_for(&dir, workspace).await;
        let read = call(
            &handler,
            "document.read",
            json!({ "path": file.to_str().unwrap(), "maxBytes": 1024 }),
        )
        .await
        .expect("read");
        let text = read["text"].as_str().unwrap();
        assert_eq!(text.len(), 1023, "cut must floor to the scalar boundary");
        assert!(text.chars().all(|c| c == 'a'));
        assert_eq!(read["truncated"], true);
        assert_eq!(read["totalBytes"], total);
    }

    #[tokio::test]
    async fn document_read_clamps_the_requested_cap_to_its_ceiling() {
        // The agent cannot raise its own bound: MAX_READ_BYTES wins over the ask.
        let content = "y".repeat(MAX_READ_BYTES as usize + 1024);
        let dir = temp_dir("clamp");
        let (workspace, file) = open_dirty_document(&dir, "huge.txt", &content).await;
        let (handler, _host) = handler_for(&dir, workspace).await;
        let read = call(
            &handler,
            "document.read",
            json!({ "path": file.to_str().unwrap(), "maxBytes": 512 * 1024 * 1024 }),
        )
        .await
        .expect("read");
        assert_eq!(
            read["text"].as_str().unwrap().len(),
            MAX_READ_BYTES as usize
        );
        assert_eq!(read["truncated"], true);
        assert_eq!(read["totalBytes"], MAX_READ_BYTES + 1024);
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
        let (handler, _host) = handler_for(&dir, workspace).await;
        let path = dir.join("notes.txt");

        let read = call(
            &handler,
            "document.read",
            json!({ "path": path.to_str().unwrap() }),
        )
        .await
        .expect("read");
        assert_eq!(read["text"], "hello");
        assert_eq!(read["open"], false);
        // Disk reads refuse oversize files rather than cutting them, and say so
        // in the same shape the dirty path reports (plan 119 P2-1).
        assert_eq!(read["truncated"], false);
        assert_eq!(read["totalBytes"], 5);

        let written = call(
            &handler,
            "document.write",
            json!({ "path": path.to_str().unwrap(), "content": "replaced" }),
        )
        .await
        .expect("write");
        assert_eq!(written["version"], 2);
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "replaced");

        let stat = call(
            &handler,
            "document.stat",
            json!({ "path": path.to_str().unwrap() }),
        )
        .await
        .expect("stat");
        assert_eq!(stat["size"], 8);
        assert_eq!(stat["open"], false);
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
        let (handler, _host) = handler_for(&dir, workspace).await;
        let error = call(
            &handler,
            "document.write",
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
        let (handler, _host) = handler_for(&dir, workspace).await;
        let path = dir.join("nested/new.txt");
        let written = call(
            &handler,
            "document.write",
            json!({ "path": path.to_str().unwrap(), "content": "fresh" }),
        )
        .await
        .expect("write");
        assert_eq!(written["version"], 1);
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "fresh");
    }

    /// Plan 119 SC-6: two sessions in two workspaces — each writes (and
    /// reads) inside its own root only; a path under the other root fails
    /// closed instead of resolving through the other tab's state.
    #[tokio::test]
    async fn sessions_touch_only_their_own_workspace_root() {
        let dir_a = temp_dir("iso-a");
        let dir_b = temp_dir("iso-b");
        let workspace_a = workspace_with_file(&dir_a, "a.txt", "a").await;
        let workspace_b = workspace_with_file(&dir_b, "b.txt", "b").await;
        let host = AgentHost::inert();
        host.record_session_root_for_test("session-a", &dir_a.to_string_lossy())
            .await;
        host.record_session_root_for_test("session-b", &dir_b.to_string_lossy())
            .await;
        let handler = handler_for_tabs(
            &host,
            &[
                (1, &dir_a, Arc::clone(&workspace_a)),
                (2, &dir_b, Arc::clone(&workspace_b)),
            ],
        )
        .await;

        let new_a = dir_a.join("from-a.txt");
        let new_b = dir_b.join("from-b.txt");
        for (session, path, content) in [("session-a", &new_a, "A"), ("session-b", &new_b, "B")] {
            let written = call_as(
                &handler,
                session,
                "document.write",
                json!({ "path": path.to_str().unwrap(), "content": content }),
            )
            .await
            .expect("write inside the session's own root");
            assert_eq!(written["saved"], true);
        }
        assert_eq!(tokio::fs::read_to_string(&new_a).await.unwrap(), "A");
        assert_eq!(tokio::fs::read_to_string(&new_b).await.unwrap(), "B");

        // Session A may not write or read through B's root.
        let sneak = dir_b.join("sneak.txt");
        let crossed = call_as(
            &handler,
            "session-a",
            "document.write",
            json!({ "path": sneak.to_str().unwrap(), "content": "no" }),
        )
        .await;
        assert!(crossed.is_err(), "cross-root write must fail closed");
        assert!(!sneak.exists(), "cross-root write must create nothing");
        let crossed_read = call_as(
            &handler,
            "session-a",
            "document.read",
            json!({ "path": dir_b.join("b.txt").to_str().unwrap() }),
        )
        .await;
        assert!(crossed_read.is_err(), "cross-root read must fail closed");
    }

    /// Plan 119 SC-6: an unrecorded or unregistered session root fails closed
    /// with an `agent.workspace_unresolved` diagnostic — never the root of
    /// whichever tab happens to be first.
    #[tokio::test]
    async fn unresolved_session_root_fails_closed_with_a_diagnostic() {
        let dir = temp_dir("unresolved");
        let workspace = workspace_with_file(&dir, "a.txt", "a").await;
        let host = AgentHost::inert();
        let mut diagnostics = host.subscribe();
        // A session whose recorded folder no tab has open.
        let gone = temp_dir("unresolved-gone");
        host.record_session_root_for_test("session-foreign", &gone.to_string_lossy())
            .await;
        let handler = handler_for_tabs(&host, &[(1, &dir, workspace)]).await;

        let unrecorded_path = dir.join("x.txt");
        let unrecorded = call_as(
            &handler,
            "session-unknown",
            "document.write",
            json!({ "path": unrecorded_path.to_str().unwrap(), "content": "no" }),
        )
        .await
        .expect_err("a session with no recorded root must fail");
        assert!(
            unrecorded.contains("agent.workspace_unresolved"),
            "{unrecorded}"
        );
        let foreign_path = dir.join("y.txt");
        let foreign = call_as(
            &handler,
            "session-foreign",
            "document.write",
            json!({ "path": foreign_path.to_str().unwrap(), "content": "no" }),
        )
        .await
        .expect_err("a root no tab has open must fail");
        assert!(foreign.contains("agent.workspace_unresolved"), "{foreign}");
        assert!(!unrecorded_path.exists() && !foreign_path.exists());

        // Visible, not silent: each refusal broadcasts a diagnostic.
        let mut codes = Vec::new();
        while let Ok(message) = diagnostics.try_recv() {
            if let AgentServerMessage::Diagnostic { code, .. } = &*message {
                codes.push(code.clone());
            }
        }
        assert_eq!(codes, vec!["agent.workspace_unresolved"; 2]);
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
            let root_id = workspace.lock().await.directory_roots()[0].workspace_root_id;
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
        let (handler, _host) = handler_for(&dir, workspace).await;
        let read = call(
            &handler,
            "document.read",
            json!({ "path": file.to_str().unwrap() }),
        )
        .await
        .expect("read");
        assert_eq!(read["text"], "dirty buffer");
        assert_eq!(read["dirty"], true);
        assert_eq!(read["open"], true);
    }
}
