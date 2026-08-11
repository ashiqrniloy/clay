use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use tokio::sync::{Mutex, Semaphore};

use crate::{
    perf::budgets::DIRECTORY_LISTING_MAX_CONCURRENCY,
    server::workspace::{
        FileListEntryKind, FileListPage, FileListRequest, ListingCancelToken,
        ListingCancellationGuard, WorkspaceState, cancel_listing, create_listing_cancel_token,
        create_listing_cancel_token_id, register_listing_cancel_token, traverse_directory,
    },
};

static DIRECTORY_LISTING_PERMITS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(DIRECTORY_LISTING_MAX_CONCURRENCY)));

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
            "workspace.list_roots_failed: failed to serialize result: {error}"
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
            "workspace.add_root_failed: failed to serialize result: {error}"
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
            "workspace.discover_root_for_path_failed: failed to serialize result: {error}"
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

    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let page = run_directory_listing(workspace, request, token_id, token).await?;
    serde_json::to_string(&serialize_file_list_page(&page)).map_err(|error| {
        JsErrorBox::generic(format!(
            "workspace.list_directory_failed: failed to serialize result: {error}"
        ))
    })
}

async fn run_directory_listing(
    workspace: Arc<Mutex<WorkspaceState>>,
    request: FileListRequest,
    token_id: String,
    token: ListingCancelToken,
) -> Result<FileListPage, JsErrorBox> {
    let _cleanup = ListingCancellationGuard::new(token_id, Arc::clone(&token));
    let plan = workspace
        .lock()
        .await
        .prepare_directory_listing(request)
        .map_err(|error| JsErrorBox::generic(error.diagnostic().to_string()))?;
    let permit = Arc::clone(&DIRECTORY_LISTING_PERMITS)
        .acquire_owned()
        .await
        .map_err(|_| JsErrorBox::generic("workspace.list_directory_failed: service stopped"))?;
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        traverse_directory(plan, Some(&token))
    })
    .await
    .map_err(|_| JsErrorBox::generic("workspace.list_directory_failed: traversal task failed"))?;
    result.map_err(|error| JsErrorBox::generic(error.diagnostic().to_string()))
}

#[op2]
#[string]
pub(super) async fn op_clay_workspace_create_listing_cancel_token() -> Result<String, JsErrorBox> {
    Ok(create_listing_cancel_token_id())
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
            "workspace.list_directory_failed: invalid request JSON: {error}"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workspace::{
        cancel_listing, open_existing_file_unlocked, save_document_unlocked,
    };
    use std::{fs, io::Write, panic::AssertUnwindSafe, time::Duration};

    #[cfg(unix)]
    #[tokio::test]
    async fn slow_listing_does_not_hold_workspace_lock_and_cleans_token() {
        let root = std::env::temp_dir().join(format!(
            "clay-slow-listing-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("note.txt"), "hello").unwrap();
        let fifo = root.join(".gitignore");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );

        let mut state = WorkspaceState::new();
        let root_id = state.add_root(&root).unwrap();
        let workspace = Arc::new(Mutex::new(state));
        let (token_id, token) = create_listing_cancel_token();
        let token_for_check = token_id.clone();
        let listing_workspace = Arc::clone(&workspace);
        let listing = tokio::spawn(async move {
            run_directory_listing(
                listing_workspace,
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 100,
                },
                token_id,
                token,
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !listing.is_finished(),
            "FIFO-backed listing must be blocked"
        );

        let opened = tokio::time::timeout(
            Duration::from_secs(1),
            open_existing_file_unlocked(&workspace, root_id, "note.txt", 7),
        )
        .await
        .expect("open must not wait for listing")
        .expect("open succeeds");
        tokio::time::timeout(
            Duration::from_secs(1),
            save_document_unlocked(&workspace, opened.document_id, 7, 1),
        )
        .await
        .expect("save must not wait for listing")
        .expect("save succeeds");

        assert!(cancel_listing(&token_for_check));
        let fifo_writer = tokio::task::spawn_blocking(move || {
            let mut writer = fs::OpenOptions::new().write(true).open(fifo).unwrap();
            writer.write_all(b"# release\n").unwrap();
        });
        let page = listing
            .await
            .unwrap()
            .expect("listing completes after release");
        fifo_writer.await.unwrap();
        assert!(page.cancelled);
        assert!(
            !cancel_listing(&token_for_check),
            "successful completion must remove its token"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn listing_error_removes_cancel_token() {
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
        let (token_id, token) = create_listing_cancel_token();
        let token_for_check = token_id.clone();
        let result = run_directory_listing(
            workspace,
            FileListRequest {
                root_id: 999,
                ..FileListRequest::default()
            },
            token_id,
            token,
        )
        .await;
        assert!(result.is_err());
        assert!(!cancel_listing(&token_for_check));
    }

    #[test]
    fn listing_guard_removes_cancel_token_during_unwind() {
        let (token_id, token) = create_listing_cancel_token();
        let token_for_check = token_id.clone();
        let unwind = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = ListingCancellationGuard::new(token_id, token);
            panic!("staged listing panic");
        }));
        assert!(unwind.is_err());
        assert!(!cancel_listing(&token_for_check));
    }
}
