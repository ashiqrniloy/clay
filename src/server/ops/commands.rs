use std::{collections::BTreeMap, sync::Arc};

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::{
    packages::{
        commands::{CommandDiagnostic, PackageCommandDeclaration},
        manifest::{ClayPackageManifest, validate_manifest_value},
        permissions::parse_permission,
    },
    protocol::RoutingPolicy,
};

use super::ClayOpState;

#[op2]
#[string]
pub(super) fn op_clay_commands_register_command(
    state: &mut OpState,
    #[string] manifest_json: String,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    let package = parse_manifest(&manifest_json)?;
    let value = parse_json(&declaration_json, "clay.commands.invalid_declaration")?;
    let declaration = parse_declaration(&value, &package)?;
    let registered = state
        .borrow::<Arc<ClayOpState>>()
        .register_command(&package, declaration)
        .map_err(command_error("clay.commands.registration_failed"))?;
    serde_json::to_string(&json!({
        "packageName": registered.package_name,
        "packageVersion": registered.package_version,
        "apiPrefix": registered.api_prefix,
        "commandId": registered.command_id,
        "displayName": registered.display_name,
        "routingPolicy": routing_policy_name(&registered.routing_policy),
        "permissions": registered.permissions.iter().map(|permission| permission.as_str()).collect::<Vec<_>>(),
    }))
    .map_err(serialize_error("clay.commands.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_commands_list_commands(state: &mut OpState) -> Result<String, JsErrorBox> {
    let commands = state.borrow::<Arc<ClayOpState>>().list_package_commands();
    serde_json::to_string(&Value::Array(commands))
        .map_err(serialize_error("clay.commands.list_failed"))
}

fn parse_manifest(json_text: &str) -> Result<ClayPackageManifest, JsErrorBox> {
    let value = parse_json(json_text, "clay.packages.invalid_manifest")?;
    validate_manifest_value(&value).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.packages.invalid_manifest: {:?}: {}",
            error.rule, error.message
        ))
    })
}

fn parse_declaration(
    value: &Value,
    package: &ClayPackageManifest,
) -> Result<PackageCommandDeclaration, JsErrorBox> {
    let permissions =
        match value.get("permissions") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(values)) => values
                .iter()
                .map(|value| {
                    let permission = value.as_str().ok_or_else(|| JsErrorBox::generic(
                    "clay.commands.invalid_declaration: permissions entries must be strings",
                ))?;
                    parse_permission(permission).map_err(|_| JsErrorBox::generic(format!(
                    "clay.commands.invalid_declaration: unsupported permission `{permission}`"
                )))
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(JsErrorBox::generic(
                    "clay.commands.invalid_declaration: permissions must be an array",
                ));
            }
        };

    Ok(PackageCommandDeclaration {
        package_name: string_or(value, "packageName", &package.name),
        package_version: string_or(value, "packageVersion", &package.version),
        api_prefix: string_or(value, "apiPrefix", &package.clay.api_prefix),
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
            "clay.commands.invalid_declaration: unsupported routingPolicy `{other}`"
        ))),
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
                "clay.commands.invalid_declaration: {key} must be a string"
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
