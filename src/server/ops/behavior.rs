use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::protocol::{CommandAuthority, RoutingPolicy};

use super::{ClayOpState, keybindings::key_chord_string};

#[op2]
#[string]
pub(super) fn op_clay_behavior_get_active_manifest(
    state: &mut OpState,
    #[string] _document_id: String,
) -> Result<String, JsErrorBox> {
    let manifest = state.borrow::<Arc<ClayOpState>>().behavior_manifest();
    serde_json::to_string(&json!({
        "id": manifest.manifest_id,
        "version": manifest.behavior_version,
        "clientFirstBehaviors": manifest.commands.iter()
            .filter(|command| matches!(command.routing_policy, RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck))
            .map(|command| command.command_id.clone())
            .collect::<Vec<_>>(),
    }))
    .map_err(serialize_error("clay.behavior.manifest_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_behavior_list_routes(
    state: &mut OpState,
    #[string] _document_id: String,
) -> Result<String, JsErrorBox> {
    let manifest = state.borrow::<Arc<ClayOpState>>().behavior_manifest();
    let routes = manifest
        .keymaps
        .iter()
        .map(|rule| {
            json!({
                "input": key_chord_string(&rule.sequence[0]),
                "runtimePath": runtime_path(&rule.routing_policy),
                "apiId": if rule.command_id.starts_with("clay.") { Some(rule.command_id.clone()) } else { None },
                "commandId": rule.command_id,
                "authority": manifest.commands.iter()
                    .find(|command| command.command_id == rule.command_id)
                    .map(|command| authority_name(&command.authority)),
            })
        })
        .collect::<Vec<Value>>();
    serde_json::to_string(&Value::Array(routes))
        .map_err(serialize_error("clay.behavior.routes_failed"))
}

fn runtime_path(policy: &RoutingPolicy) -> &'static str {
    match policy {
        RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck => {
            "client-first"
        }
        RoutingPolicy::ServerFirst | RoutingPolicy::ServerFirstWithLock { .. } => "server-first",
        RoutingPolicy::ClientUiCommand => "client-ui-command",
        RoutingPolicy::UiReactivePriority | RoutingPolicy::Background => "background",
    }
}

fn authority_name(authority: &CommandAuthority) -> &'static str {
    match authority {
        CommandAuthority::BuiltInClientEdit => "built-in-client-edit",
        CommandAuthority::ServerIntent => "server-intent",
        CommandAuthority::ClientUi => "client-ui",
    }
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}
