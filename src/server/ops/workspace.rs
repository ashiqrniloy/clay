use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::server::workspace::{
    FileListEntryKind, FileListRequest, cancel_listing, create_listing_cancel_token,
    register_listing_cancel_token, remove_listing_cancel_token,
};

use super::ClayOpState;

#[op2]
#[string]
pub(super) async fn op_clay_workspace_list_roots(
    state: Rc<RefCell<OpState>>,
) -> Result<String, JsErrorBox> {
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let roots = workspace.lock().await.list_root_metadata();
    let value = Value::Array(
        roots
            .iter()
            .map(|root| {
                json!({
                    "workspaceRootId": root.workspace_root_id.to_string(),
                    "displayName": root.display_name,
                    "displayPath": root.display_path,
                })
            })
            .collect(),
    );
    serde_json::to_string(&value).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.workspace.list_roots_failed: failed to serialize result: {error}"
        ))
    })
}

#[op2]
#[string]
pub(super) async fn op_clay_workspace_add_root(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let root_id = workspace
        .lock()
        .await
        .add_explicit_user_grant(PathBuf::from(path))
        .map_err(|error| JsErrorBox::generic(error.diagnostic().to_string()))?;
    let value = json!({
        "workspaceRootId": root_id.to_string(),
    });
    serde_json::to_string(&value).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.workspace.add_root_failed: failed to serialize result: {error}"
        ))
    })
}

#[op2]
#[string]
pub(super) async fn op_clay_workspace_discover_root_for_path(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let root_id = workspace
        .lock()
        .await
        .discover_root_for_path(PathBuf::from(path))
        .map_err(|error| JsErrorBox::generic(error.diagnostic().to_string()))?;
    let value = match root_id {
        Some(id) => json!({
            "workspaceRootId": id.to_string(),
            "discovered": true,
        }),
        None => json!({
            "workspaceRootId": None::<String>,
            "discovered": false,
        }),
    };
    serde_json::to_string(&value).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.workspace.discover_root_for_path_failed: failed to serialize result: {error}"
        ))
    })
}

#[op2]
#[string]
pub(super) async fn op_clay_workspace_list_directory(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
    #[string] cancel_token_id: Option<String>,
) -> Result<String, JsErrorBox> {
    let request = parse_file_list_request(&request_json)?;
    let (token_id, token) = match cancel_token_id {
        Some(id) => (id.clone(), register_listing_cancel_token(id)),
        None => create_listing_cancel_token(),
    };

    let result = {
        let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
        let guard = workspace.lock().await;
        guard.list_directory(request, Some(&token))
    };

    remove_listing_cancel_token(&token_id);

    let page = result.map_err(|error| JsErrorBox::generic(error.diagnostic().to_string()))?;
    serde_json::to_string(&serialize_file_list_page(&page)).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.workspace.list_directory_failed: failed to serialize result: {error}"
        ))
    })
}

#[op2]
#[string]
pub(super) async fn op_clay_workspace_create_listing_cancel_token() -> Result<String, JsErrorBox> {
    let (id, _) = create_listing_cancel_token();
    Ok(id)
}

#[op2]
pub(super) async fn op_clay_workspace_cancel_listing(
    #[string] token_id: String,
) -> Result<bool, JsErrorBox> {
    Ok(cancel_listing(&token_id))
}

fn parse_file_list_request(json: &str) -> Result<FileListRequest, JsErrorBox> {
    let value: Value = serde_json::from_str(json).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.workspace.list_directory_failed: invalid request JSON: {error}"
        ))
    })?;
    let defaults = FileListRequest::default();
    Ok(FileListRequest {
        root_id: value
            .get("rootId")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok())
            .unwrap_or(defaults.root_id),
        relative_path: value
            .get("relativePath")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or(defaults.relative_path),
        max_depth: value
            .get("maxDepth")
            .and_then(Value::as_u64)
            .map(|d| d as usize)
            .unwrap_or(defaults.max_depth),
        max_entries: value
            .get("maxEntries")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(defaults.max_entries),
    })
}

fn serialize_file_list_page(page: &crate::server::workspace::FileListPage) -> Value {
    json!({
        "rootId": page.root_id.to_string(),
        "entries": page.entries.iter().map(|entry| {
            let kind = match entry.kind {
                FileListEntryKind::Directory => "directory",
                FileListEntryKind::File => "file",
                FileListEntryKind::Symlink => "symlink",
                FileListEntryKind::Other => "other",
            };
            json!({
                "name": entry.name,
                "kind": kind,
                "relativePath": entry.relative_path.to_string_lossy().replace('\\', "/"),
                "sizeHint": entry.size_hint,
                "childCount": entry.child_count,
                "diagnostic": entry.diagnostic.as_ref().map(|diagnostic| json!({
                    "code": format!("{:?}", diagnostic.code),
                    "message": diagnostic.message,
                })),
            })
        }).collect::<Vec<_>>(),
        "truncated": page.truncated,
        "cancelled": page.cancelled,
        "diagnostics": page.diagnostics.iter().map(|diagnostic| json!({
            "code": format!("{:?}", diagnostic.code),
            "message": diagnostic.message,
            "hint": diagnostic.hint,
        })).collect::<Vec<_>>(),
    })
}
