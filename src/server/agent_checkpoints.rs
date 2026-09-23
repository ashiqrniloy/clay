//! Document version checkpoints for agent session trees.
//!
//! The server (document authority) snapshots the text and version of every
//! open document at a checkpoint point, keyed by `(sessionId, entryId)`.
//! Restoring a checkpoint routes the snapshotted text through
//! `open_existing_file_unlocked` + `apply_edit` — the same lease/CAS path as
//! every other server-authoritative mutation — so dirty-buffer ownership and
//! version authority are never bypassed. Leases held by other clients fail
//! the restore for that document instead of being silently overwritten.
//!
//! ponytail: snapshots live in memory keyed by (session, entry). Persisting
//! them across daemon restarts is the upgrade if crash-restore matters.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::workspace::{
    WorkspaceError, WorkspaceRootId, WorkspaceState, open_existing_file_unlocked,
};
use crate::protocol::{ClientId, EditOperation, ServerMessage};

const AGENT_CLIENT_ID: ClientId = 0;

/// In-memory checkpoint store: (sessionId, entryId) -> per-document snapshots.
#[derive(Default)]
pub(crate) struct AgentCheckpointStore {
    checkpoints: HashMap<(String, String), HashMap<String, DocumentCheckpoint>>,
}

#[derive(Clone)]
struct DocumentCheckpoint {
    path: String,
    text: String,
}

impl AgentCheckpointStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Snapshot the session's workspace: the open documents of the resolved
    /// workspace state that live inside `root_id` (plan 119 SC-6 — a state can
    /// hold several roots, and a session checkpoints only its own).
    pub(crate) async fn checkpoint(
        &mut self,
        workspace: &Arc<Mutex<WorkspaceState>>,
        root_id: WorkspaceRootId,
        session_id: &str,
        entry_id: &str,
    ) -> Result<Value, String> {
        let workspace = workspace.lock().await;
        let mut snapshots = HashMap::new();
        for (document_id, canonical_path) in workspace.open_document_canonical_paths() {
            let Some(handle) = workspace.document_handle(document_id) else {
                continue;
            };
            let path = canonical_path.to_string_lossy().into_owned();
            // Containment first: the lease-free read below must not snapshot
            // another root's document into this session's checkpoint.
            if workspace
                .contained_existing_path(root_id, &canonical_path)
                .await
                .is_err()
            {
                continue;
            }
            let document = handle.lock().await;
            snapshots.insert(
                path.clone(),
                DocumentCheckpoint {
                    path,
                    text: document.text(),
                },
            );
        }
        let count = snapshots.len();
        self.checkpoints
            .insert((session_id.to_string(), entry_id.to_string()), snapshots);
        Ok(json!({ "sessionId": session_id, "entryId": entry_id, "documents": count }))
    }

    /// Restore every document snapshotted at `(session_id, entry_id)`.
    /// No checkpoint at that key is a no-op (pure-conversation checkout);
    /// documents are mutated through the lease/CAS path and a document whose
    /// lease is held elsewhere fails the whole restore fail-closed.
    /** Bounded entry-id list for one session (plan 108 task 10 tree flags). */
    pub(crate) fn list_for(&self, session_id: &str) -> Vec<String> {
        let mut entry_ids: Vec<String> = self
            .checkpoints
            .keys()
            .filter(|(sid, _)| sid == session_id)
            .map(|(_, entry_id)| entry_id.clone())
            .collect();
        entry_ids.sort();
        entry_ids.truncate(200);
        entry_ids
    }

    pub(crate) async fn restore(
        &self,
        workspace: &Arc<Mutex<WorkspaceState>>,
        root_id: WorkspaceRootId,
        session_id: &str,
        entry_id: &str,
    ) -> Result<Value, String> {
        let Some(snapshots) = self
            .checkpoints
            .get(&(session_id.to_string(), entry_id.to_string()))
        else {
            return Ok(json!({
                "sessionId": session_id,
                "entryId": entry_id,
                "documents": 0,
            }));
        };
        for checkpoint in snapshots.values() {
            restore_document(workspace, root_id, checkpoint).await?;
        }
        Ok(json!({
            "sessionId": session_id,
            "entryId": entry_id,
            "documents": snapshots.len(),
        }))
    }
}

async fn restore_document(
    workspace: &Arc<Mutex<WorkspaceState>>,
    root_id: WorkspaceRootId,
    checkpoint: &DocumentCheckpoint,
) -> Result<(), String> {
    let lease = open_existing_file_unlocked(
        workspace,
        root_id,
        Path::new(&checkpoint.path),
        AGENT_CLIENT_ID,
    )
    .await
    .map_err(workspace_error_message)?;
    let document_id = lease.document_id;
    let Some(lease_id) = lease.access.lease_id() else {
        return Err(format!(
            "document lease held by another client: {}",
            checkpoint.path
        ));
    };
    let document = Arc::clone(&lease.document);
    let confirmed = {
        let mut document = document.lock().await;
        let base_version = document.version();
        let end = document.byte_len() as u64;
        match document.apply_edit(
            document_id,
            AGENT_CLIENT_ID,
            Some(lease_id),
            base_version,
            0,
            EditOperation::Replace {
                start: 0,
                end,
                text: checkpoint.text.clone(),
            },
        ) {
            ServerMessage::EditAck {
                confirmed_version, ..
            } => Ok(confirmed_version),
            ServerMessage::EditRejected { reason, .. } => Err(format!("edit rejected: {reason:?}")),
            other => Err(format!("unexpected edit response: {other:?}")),
        }
    }?;
    // Buffer-only restore: the checkpointed text becomes the live dirty
    // buffer. Saving to disk is a separate user action (decision 2200:
    // checkpoints restore document *versions*, not disk state).
    document.lock().await.mark_clean_if_version(confirmed);
    let mut workspace = workspace.lock().await;
    workspace
        .release_single_document_access(document_id, AGENT_CLIENT_ID)
        .await;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workspace::WorkspaceState;

    async fn workspace_with_file(dir: &Path, name: &str, text: &str) -> Arc<Mutex<WorkspaceState>> {
        tokio::fs::create_dir_all(dir).await.unwrap();
        tokio::fs::write(dir.join(name), text).await.unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(dir).expect("root");
        Arc::new(Mutex::new(workspace))
    }

    async fn root_id(workspace: &Arc<Mutex<WorkspaceState>>) -> WorkspaceRootId {
        workspace.lock().await.directory_roots()[0].workspace_root_id
    }

    #[tokio::test]
    async fn checkpoint_captures_open_documents_and_restore_reverts_text() {
        let dir = std::env::temp_dir().join(format!(
            "clay-agent-ckpt-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = workspace_with_file(&dir, "notes.txt", "before").await;
        let root_id = root_id(&workspace).await;
        // Open the file so it has a registry entry with a real version.
        let lease = open_existing_file_unlocked(
            &workspace,
            root_id,
            Path::new("notes.txt"),
            AGENT_CLIENT_ID,
        )
        .await
        .unwrap();
        let document_id = lease.document_id;
        {
            let document = lease.document.lock().await;
            assert_eq!(document.text(), "before");
        }

        let mut store = AgentCheckpointStore::new();
        let result = store
            .checkpoint(&workspace, root_id, "session-1", "entry-1")
            .await
            .unwrap();
        assert_eq!(result["documents"], 1);

        // Dirty edit after the checkpoint.
        {
            let document = Arc::clone(&lease.document);
            let mut document = document.lock().await;
            let base_version = document.version();
            let end = document.byte_len() as u64;
            match document.apply_edit(
                document_id,
                AGENT_CLIENT_ID,
                lease.access.lease_id(),
                base_version,
                7,
                EditOperation::Replace {
                    start: 0,
                    end,
                    text: "DIRTY".to_string(),
                },
            ) {
                ServerMessage::EditAck { .. } => {}
                other => panic!("expected ack, got {other:?}"),
            }
        }

        store
            .restore(&workspace, root_id, "session-1", "entry-1")
            .await
            .unwrap();
        {
            let document = lease.document.lock().await;
            assert_eq!(document.text(), "before");
        }
        workspace
            .lock()
            .await
            .release_single_document_access(document_id, AGENT_CLIENT_ID)
            .await;
    }

    #[tokio::test]
    async fn restore_without_checkpoint_is_noop() {
        let dir = std::env::temp_dir().join(format!(
            "clay-agent-ckpt-missing-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = workspace_with_file(&dir, "a.txt", "x").await;
        let root_id = root_id(&workspace).await;
        let store = AgentCheckpointStore::new();
        let result = store
            .restore(&workspace, root_id, "nope", "nope")
            .await
            .unwrap();
        assert_eq!(result["documents"], 0);
    }
}
