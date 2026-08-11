use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::{
    packages::{
        commands::{CommandDiagnostic, PackageCommandDeclaration},
        manifest::ClayPackageManifest,
        permissions::parse_permission,
    },
    protocol::RoutingPolicy,
    server::command_execution::{
        CommandExecutionProvenance, CommandExecutionRequest, CommandExecutionTarget,
    },
};

use super::ClayOpState;

#[op2]
#[string]
pub(super) fn op_clay_commands_register_command(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .require_current_package_capability(
            crate::packages::permissions::PackagePermission::CommandRegistration,
        )?;
    let value = parse_json(&declaration_json, "commands.invalid_declaration")?;
    let declaration = parse_declaration(&value, &package.manifest)?;
    let registered = state
        .borrow::<Arc<ClayOpState>>()
        .register_command(&package.manifest, declaration)
        .map_err(command_error("commands.registration_failed"))?;
    serde_json::to_string(&json!({
        "packageName": registered.package_name,
        "packageVersion": registered.package_version,
        "apiPrefix": registered.api_prefix,
        "commandId": registered.command_id,
        "displayName": registered.display_name,
        "routingPolicy": routing_policy_name(&registered.routing_policy),
        "permissions": registered.permissions.iter().map(|permission| permission.as_str()).collect::<Vec<_>>(),
    }))
    .map_err(serialize_error("commands.registration_failed"))
}

#[op2]
#[string]
pub(super) async fn op_clay_commands_execute_command(
    state: Rc<RefCell<OpState>>,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request = parse_execute_request(&request_json)?;
    let op_state = state.borrow().borrow::<Arc<ClayOpState>>().clone();
    let result = op_state
        .execute_command(request)
        .await
        .map_err(command_execution_error("commands.execute_failed"))?;
    serde_json::to_string(&json!({
        "commandId": result.command_id,
        "routingPolicy": routing_policy_name(&result.routing_policy),
        "target": command_target_json(&result.target),
        "status": command_status_json(&result.status),
    }))
    .map_err(serialize_error("commands.execute_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_commands_list_commands(state: &mut OpState) -> Result<String, JsErrorBox> {
    let commands = state.borrow::<Arc<ClayOpState>>().list_package_commands();
    serde_json::to_string(&Value::Array(commands)).map_err(serialize_error("commands.list_failed"))
}

fn parse_declaration(
    value: &Value,
    package: &ClayPackageManifest,
) -> Result<PackageCommandDeclaration, JsErrorBox> {
    let permissions = match value.get("permissions") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                let permission = value.as_str().ok_or_else(|| {
                    JsErrorBox::generic(
                        "commands.invalid_declaration: permissions entries must be strings",
                    )
                })?;
                parse_permission(permission).map_err(|_| {
                    JsErrorBox::generic(format!(
                        "commands.invalid_declaration: unsupported permission `{permission}`"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(JsErrorBox::generic(
                "commands.invalid_declaration: permissions must be an array",
            ));
        }
    };

    Ok(PackageCommandDeclaration {
        package_name: package.name.clone(),
        package_version: package.version.clone(),
        api_prefix: package.clay.api_prefix.clone(),
        command_id: required_string(value, "commandId")?,
        display_name: required_string(value, "displayName")?,
        routing_policy: parse_routing_policy(
            value
                .get("routingPolicy")
                .and_then(Value::as_str)
                .unwrap_or("server-first"),
        )?,
        key_bindings: Vec::new(),
        custom_properties: BTreeMap::new(),
        permissions,
    })
}

fn parse_routing_policy(value: &str) -> Result<RoutingPolicy, JsErrorBox> {
    match value {
        "server-first" | "ServerFirst" => Ok(RoutingPolicy::ServerFirst),
        "background" | "Background" => Ok(RoutingPolicy::Background),
        "ui-reactive-priority" | "UiReactivePriority" => Ok(RoutingPolicy::UiReactivePriority),
        other => Err(JsErrorBox::generic(format!(
            "commands.invalid_declaration: unsupported routingPolicy `{other}`"
        ))),
    }
}

fn parse_execute_request(json_text: &str) -> Result<CommandExecutionRequest, JsErrorBox> {
    let value = parse_json(json_text, "commands.invalid_execute_request")?;
    let command_id = required_string(&value, "commandId")?;
    let arguments = value.get("arguments").cloned().unwrap_or(Value::Null);
    let target = value
        .get("target")
        .map(parse_execute_target)
        .transpose()?
        .unwrap_or(CommandExecutionTarget::Global);
    let provenance = value
        .get("provenance")
        .filter(|value| !value.is_null())
        .map(parse_provenance)
        .transpose()?;
    let expected_permissions = match value.get("expectedPermissions") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                let permission = value.as_str().ok_or_else(|| {
                    JsErrorBox::generic(
                        "commands.invalid_execute_request: expectedPermissions entries must be strings",
                    )
                })?;
                parse_permission(permission).map_err(|_| {
                    JsErrorBox::generic(format!(
                        "commands.invalid_execute_request: unsupported permission `{permission}`"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(JsErrorBox::generic(
                "commands.invalid_execute_request: expectedPermissions must be an array",
            ));
        }
    };
    Ok(CommandExecutionRequest {
        command_id,
        arguments,
        target,
        provenance,
        expected_permissions,
    })
}

fn parse_execute_target(value: &Value) -> Result<CommandExecutionTarget, JsErrorBox> {
    if let Some(document_id) = value
        .get("activeDocument")
        .and_then(|v| v.get("documentId"))
        .and_then(Value::as_u64)
    {
        return Ok(CommandExecutionTarget::ActiveDocument { document_id });
    }
    if value.get("workspace").is_some() {
        return Ok(CommandExecutionTarget::Workspace);
    }
    if value.get("global").is_some() || value.is_null() {
        return Ok(CommandExecutionTarget::Global);
    }
    Err(JsErrorBox::generic(
        "commands.invalid_execute_request: unsupported command target",
    ))
}

fn parse_provenance(value: &Value) -> Result<CommandExecutionProvenance, JsErrorBox> {
    Ok(CommandExecutionProvenance {
        package_name: required_string(value, "packageName")?,
        package_version: required_string(value, "packageVersion")?,
        api_prefix: required_string(value, "apiPrefix")?,
    })
}

fn command_execution_error(
    code: &'static str,
) -> impl Fn(crate::server::command_execution::CommandExecutionDiagnostic) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn command_target_json(target: &CommandExecutionTarget) -> Value {
    match target {
        CommandExecutionTarget::ActiveDocument { document_id } => json!({
            "activeDocument": { "documentId": document_id }
        }),
        CommandExecutionTarget::Workspace => json!({ "workspace": {} }),
        CommandExecutionTarget::Global => json!({ "global": {} }),
    }
}

fn command_status_json(status: &crate::server::command_execution::CommandExecutionStatus) -> Value {
    use crate::server::command_execution::{
        CommandExecutionStatus, GitCommandResult, WorkspaceActionResult,
    };
    match status {
        CommandExecutionStatus::Accepted => json!({ "kind": "accepted" }),
        CommandExecutionStatus::Discovery(result) => json!({
            "kind": "discovery",
            "result": format!("{result:?}"),
        }),
        CommandExecutionStatus::Workspace(WorkspaceActionResult::Opened(snapshot)) => json!({
            "kind": "workspace",
            "action": "opened",
            "documentId": snapshot.metadata.document_id,
            "version": snapshot.metadata.version,
            "path": snapshot.metadata.path,
        }),
        CommandExecutionStatus::Workspace(WorkspaceActionResult::Navigated {
            root_id,
            relative_path,
        }) => json!({
            "kind": "workspace",
            "action": "navigated",
            "workspaceRootId": root_id,
            "relativePath": relative_path,
        }),
        CommandExecutionStatus::Workspace(WorkspaceActionResult::Revealed) => json!({
            "kind": "workspace",
            "action": "revealed",
        }),
        CommandExecutionStatus::Workspace(WorkspaceActionResult::Toggled) => json!({
            "kind": "workspace",
            "action": "toggled",
        }),
        CommandExecutionStatus::Git(GitCommandResult::Statuses(statuses)) => json!({
            "kind": "git",
            "action": "listed",
            "statuses": statuses.iter().map(super::git::git_cached_status_json).collect::<Vec<_>>(),
        }),
        CommandExecutionStatus::Git(GitCommandResult::Refreshed(status)) => json!({
            "kind": "git",
            "action": "refreshed",
            "status": super::git::git_cached_status_json(status),
        }),
    }
}

fn parse_json(json_text: &str, code: &str) -> Result<Value, JsErrorBox> {
    serde_json::from_str(json_text)
        .map_err(|error| JsErrorBox::generic(format!("{code}: input must be valid JSON ({error})")))
}

fn required_string(value: &Value, key: &str) -> Result<String, JsErrorBox> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            JsErrorBox::generic(format!(
                "commands.invalid_declaration: {key} must be a string"
            ))
        })
}

fn string_or(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn routing_policy_name(policy: &RoutingPolicy) -> &'static str {
    match policy {
        RoutingPolicy::ServerFirst => "server-first",
        RoutingPolicy::Background => "background",
        RoutingPolicy::UiReactivePriority => "ui-reactive-priority",
        RoutingPolicy::ClientUiCommand => "client-ui-command",
        RoutingPolicy::ClientFirstPredictable => "client-first-predictable",
        RoutingPolicy::ClientFirstRequiresAck => "client-first-requires-ack",
        RoutingPolicy::ServerFirstWithLock { .. } => "server-first-with-lock",
    }
}

fn command_error(code: &'static str) -> impl Fn(CommandDiagnostic) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}
