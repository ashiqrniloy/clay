use std::{cell::RefCell, rc::Rc, sync::Arc};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

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
