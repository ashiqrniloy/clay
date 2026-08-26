use std::{cell::RefCell, rc::Rc, sync::Arc};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::{
    protocol::{DocumentAccess, DocumentMetadata},
    server::workspace::WorkspaceError,
};

use super::ClayOpState;

const RUNTIME_CLIENT_ID: u64 = 0;

#[op2]
#[string]
pub(super) async fn op_clay_documents_open_document(
    state: Rc<RefCell<OpState>>,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options = parse_object(&options_json, "documents.invalid_open_options")?;
    let workspace_root_id = parse_required_u64(
        options.get("workspaceRootId"),
        "workspaceRootId",
        "documents.invalid_open_options",
    )?;
    let path = parse_required_string(
        options.get("path"),
        "path",
        "documents.invalid_open_options",
    )?;
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let opened = crate::server::workspace::open_existing_file_unlocked(
        &workspace,
        workspace_root_id,
        path,
        RUNTIME_CLIENT_ID,
    )
    .await
    .map_err(workspace_error("documents.open_failed"))?;
    let document = opened.document.lock().await;
    let metadata = DocumentMetadata {
        document_id: opened.document_id,
        version: document.version(),
        lease_id: opened.access.lease_id(),
        access: opened.access,
        dirty: document.is_dirty(),
        workspace_root_id,
        path: opened.file_state.display_path(),
    };
    serialize_result(
        json!({
            "metadata": metadata_json(&metadata),
            "text": document.text(),
        }),
        "documents.open_failed",
    )
}

#[op2]
#[string]
pub(super) async fn op_clay_documents_save_document(
    state: Rc<RefCell<OpState>>,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options = parse_object(&options_json, "documents.invalid_save_options")?;
    let document_id = parse_required_u64(
        options.get("documentId"),
        "documentId",
        "documents.invalid_save_options",
    )?;
    let known_version = options
        .get("knownVersion")
        .map(|value| parse_u64_value(value, "knownVersion", "documents.invalid_save_options"))
        .transpose()?
        .unwrap_or(0);
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    // Runtime identity 0 bypasses an editable client lease, but an explicit
    // caller version still cannot claim state newer than the canonical server.
    let outcome = crate::server::workspace::save_document_unlocked(
        &workspace,
        document_id,
        RUNTIME_CLIENT_ID,
        known_version,
    )
    .await
    .map_err(workspace_error("documents.save_failed"))?;
    serialize_result(
        json!({
            "documentId": outcome.document_id.to_string(),
            "version": outcome.version,
            "dirty": outcome.dirty,
        }),
        "documents.save_failed",
    )
}

#[op2]
#[string]
pub(super) async fn op_clay_documents_reload_document(
    state: Rc<RefCell<OpState>>,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options = parse_object(&options_json, "documents.invalid_reload_options")?;
    let document_id = parse_required_u64(
        options.get("documentId"),
        "documentId",
        "documents.invalid_reload_options",
    )?;
    let force = options
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let (metadata, text) = {
        crate::server::workspace::reload_document_unlocked(
            &workspace,
            document_id,
            RUNTIME_CLIENT_ID,
            force,
        )
        .await
        .map_err(workspace_error("documents.reload_failed"))?;
        let workspace = workspace.lock().await;
        let metadata = workspace
            .document_metadata(document_id, RUNTIME_CLIENT_ID)
            .await
            .map_err(workspace_error("documents.reload_failed"))?;
        let document = workspace.document_handle(document_id).ok_or_else(|| {
            workspace_error("documents.reload_failed")(WorkspaceError::UnknownDocument {
                document_id,
            })
        })?;
        let text = document.lock().await.text();
        (metadata, text)
    };
    serialize_result(
        json!({
            "metadata": metadata_json(&metadata),
            "text": text,
        }),
        "documents.reload_failed",
    )
}

#[op2]
#[string]
pub(super) async fn op_clay_documents_get_document_status(
    state: Rc<RefCell<OpState>>,
    #[string] document_id_json: String,
) -> Result<String, JsErrorBox> {
    let document_id = parse_u64_value(
        &serde_json::from_str::<Value>(&document_id_json)
            .unwrap_or(Value::String(document_id_json)),
        "documentId",
        "documents.invalid_status_options",
    )?;
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let metadata = workspace
        .lock()
        .await
        .document_metadata(document_id, RUNTIME_CLIENT_ID)
        .await
        .map_err(workspace_error("documents.status_failed"))?;
    serialize_result(metadata_json(&metadata), "documents.status_failed")
}

#[op2]
#[string]
pub(super) async fn op_clay_documents_list_documents(
    state: Rc<RefCell<OpState>>,
) -> Result<String, JsErrorBox> {
    let workspace = state.borrow().borrow::<Arc<ClayOpState>>().workspace();
    let documents = workspace
        .lock()
        .await
        .list_documents(RUNTIME_CLIENT_ID)
        .await
        .map_err(workspace_error("documents.list_failed"))?;
    serialize_result(
        Value::Array(documents.iter().map(metadata_json).collect()),
        "documents.list_failed",
    )
}

fn parse_object(json: &str, code: &str) -> Result<serde_json::Map<String, Value>, JsErrorBox> {
    let value = serde_json::from_str::<Value>(json)
        .map_err(|error| JsErrorBox::generic(format!("{code}: invalid JSON options: {error}")))?;
    let Value::Object(object) = value else {
        return Err(JsErrorBox::generic(format!(
            "{code}: options must be an object"
        )));
    };
    Ok(object)
}

fn parse_required_string<'a>(
    value: Option<&'a Value>,
    field: &str,
    code: &str,
) -> Result<&'a str, JsErrorBox> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| JsErrorBox::generic(format!("{code}: {field} must be a non-empty string")))
}

fn parse_required_u64(value: Option<&Value>, field: &str, code: &str) -> Result<u64, JsErrorBox> {
    let Some(value) = value else {
        return Err(JsErrorBox::generic(format!("{code}: {field} is required")));
    };
    parse_u64_value(value, field, code)
}

fn parse_u64_value(value: &Value, field: &str, code: &str) -> Result<u64, JsErrorBox> {
    match value {
        Value::Number(number) => number.as_u64().ok_or_else(|| {
            JsErrorBox::generic(format!("{code}: {field} must be an unsigned integer"))
        }),
        Value::String(text) => text.parse::<u64>().map_err(|_| {
            JsErrorBox::generic(format!(
                "{code}: {field} must be an unsigned integer string"
            ))
        }),
        _ => Err(JsErrorBox::generic(format!(
            "{code}: {field} must be an unsigned integer or string"
        ))),
    }
}

fn metadata_json(metadata: &DocumentMetadata) -> Value {
    json!({
        "documentId": metadata.document_id.to_string(),
        "version": metadata.version,
        "readOnly": matches!(metadata.access, DocumentAccess::ReadOnly),
        "leaseId": metadata.lease_id.map(|lease_id| lease_id.to_string()),
        "dirty": metadata.dirty,
        "workspaceRootId": metadata.workspace_root_id.to_string(),
        "path": metadata.path,
    })
}

fn serialize_result(value: Value, code: &str) -> Result<String, JsErrorBox> {
    serde_json::to_string(&value).map_err(|error| {
        JsErrorBox::generic(format!("{code}: failed to serialize result: {error}"))
    })
}

fn workspace_error(code: &'static str) -> impl FnOnce(WorkspaceError) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {}", error.diagnostic()))
}
