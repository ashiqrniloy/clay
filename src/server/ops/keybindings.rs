use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::protocol::{KeyBindingContext, KeyBindingRule, KeyCode, KeyModifiers, KeyStroke};

use super::ClayOpState;

#[op2]
#[string]
pub(super) fn op_clay_keybindings_bind_key(
    state: &mut OpState,
    #[string] key: String,
    #[string] command_id: String,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options = parse_options(&options_json, "clay.keybindings.invalid_bind")?;
    let scope = parse_scope(options.get("scope"), "clay.keybindings.invalid_bind")?;
    reject_when_clause(options.get("when"), "clay.keybindings.invalid_bind")?;
    let rule = KeyBindingRule {
        command_id: validate_command_id(&command_id)?,
        sequence: vec![parse_key_chord(&key)?],
        context: scope,
        routing_policy: command_routing_policy(&command_id)?,
    };
    let manifest = state
        .borrow::<Arc<ClayOpState>>()
        .bind_key(rule)
        .map_err(manifest_error("clay.keybindings.bind_failed"))?;
    serialize_key_binding(
        manifest
            .keymaps
            .iter()
            .find(|candidate| candidate.command_id == command_id)
            .expect("bound keymap must exist"),
    )
}

#[op2]
#[string]
pub(super) fn op_clay_keybindings_unbind_key(
    state: &mut OpState,
    #[string] key: String,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options = parse_options(&options_json, "clay.keybindings.invalid_unbind")?;
    let scope = parse_scope(options.get("scope"), "clay.keybindings.invalid_unbind")?;
    reject_when_clause(options.get("when"), "clay.keybindings.invalid_unbind")?;
    let stroke = parse_key_chord(&key)?;
    let manifest = state
        .borrow::<Arc<ClayOpState>>()
        .unbind_key(&stroke, &scope)
        .map_err(manifest_error("clay.keybindings.unbind_failed"))?;
    serialize_bindings(&manifest.keymaps)
}

#[op2]
#[string]
pub(super) fn op_clay_keybindings_list_key_bindings(
    state: &mut OpState,
    #[string] scope: String,
) -> Result<String, JsErrorBox> {
    let manifest = state.borrow::<Arc<ClayOpState>>().behavior_manifest();
    let records = manifest
        .keymaps
        .iter()
        .filter(|rule| match scope.as_str() {
            "all" | "" => true,
            "editor" => rule.context == KeyBindingContext::EditorTextFocus,
            "global" => rule.context == KeyBindingContext::Global,
            _ => false,
        })
        .map(key_binding_json)
        .collect();
    serde_json::to_string(&Value::Array(records))
        .map_err(serialize_error("clay.keybindings.list_failed"))
}

fn parse_options(json: &str, code: &str) -> Result<Map<String, Value>, JsErrorBox> {
    if json.trim().is_empty() {
        return Ok(Map::new());
    }
    let value = serde_json::from_str::<Value>(json).map_err(|error| {
        JsErrorBox::generic(format!("{code}: options must be valid JSON ({error})"))
    })?;
    match value {
        Value::Object(object) => Ok(object),
        Value::Null => Ok(Map::new()),
        _ => Err(JsErrorBox::generic(format!(
            "{code}: options must be an object"
        ))),
    }
}

fn parse_scope(value: Option<&Value>, code: &str) -> Result<KeyBindingContext, JsErrorBox> {
    match value.and_then(Value::as_str).unwrap_or("editor") {
        "editor" => Ok(KeyBindingContext::EditorTextFocus),
        "global" => Ok(KeyBindingContext::Global),
        other => Err(JsErrorBox::generic(format!(
            "{code}: unsupported key binding scope `{other}`"
        ))),
    }
}

fn reject_when_clause(value: Option<&Value>, code: &str) -> Result<(), JsErrorBox> {
    if value.is_some() && !matches!(value, Some(Value::Null)) {
        return Err(JsErrorBox::generic(format!(
            "{code}: conditional `when` expressions are not runtime-backed yet"
        )));
    }
    Ok(())
}

fn parse_key_chord(chord: &str) -> Result<KeyStroke, JsErrorBox> {
    let trimmed = chord.trim();
    if trimmed.is_empty() {
        return Err(JsErrorBox::generic(
            "clay.keybindings.invalid_key: key chord must not be empty",
        ));
    }
    if trimmed.contains(' ') {
        return Err(JsErrorBox::generic(
            "clay.keybindings.invalid_key: multi-stroke key chords are not runtime-backed yet",
        ));
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut key_part = None;
    for part in trimmed.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.control = true,
            "alt" | "option" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            "super" | "cmd" | "command" | "meta" => modifiers.super_key = true,
            _ if key_part.is_none() => key_part = Some(part),
            _ => {
                return Err(JsErrorBox::generic(format!(
                    "clay.keybindings.invalid_key: malformed key chord `{chord}`"
                )));
            }
        }
    }

    let Some(key_part) = key_part else {
        return Err(JsErrorBox::generic(format!(
            "clay.keybindings.invalid_key: missing key in chord `{chord}`"
        )));
    };
    let key = match key_part.to_ascii_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "escape" | "esc" => KeyCode::Escape,
        "arrowup" | "up" => KeyCode::ArrowUp,
        "arrowdown" | "down" => KeyCode::ArrowDown,
        "arrowleft" | "left" => KeyCode::ArrowLeft,
        "arrowright" | "right" => KeyCode::ArrowRight,
        _ if key_part.chars().count() == 1 => KeyCode::Character(key_part.to_ascii_lowercase()),
        _ => {
            return Err(JsErrorBox::generic(format!(
                "clay.keybindings.invalid_key: unsupported key `{key_part}`"
            )));
        }
    };
    Ok(KeyStroke { key, modifiers })
}

fn validate_command_id(command_id: &str) -> Result<String, JsErrorBox> {
    if command_id.trim().is_empty()
        || command_id.contains("javascript:")
        || command_id.contains("=>")
    {
        return Err(JsErrorBox::generic(
            "clay.keybindings.invalid_command: command ID must be a non-empty registered command string",
        ));
    }
    if is_runtime_bindable_command(command_id) {
        Ok(command_id.to_string())
    } else {
        Err(JsErrorBox::generic(format!(
            "clay.keybindings.unknown_command: command `{command_id}` is not registered for behavior manifests"
        )))
    }
}

fn is_runtime_bindable_command(command_id: &str) -> bool {
    matches!(
        command_id,
        "text.insert_newline"
            | "text.insert_tab"
            | "completion.trigger"
            | "workspace.refresh"
            | "document.focus_active"
            | "document.open_recent"
            | "clay.documents.serverOpenDocument"
            | "clay.documents.clientOpenFileDialog"
            | "clay.documents.serverSaveDocument"
            | "clay.documents.serverReloadDocument"
            | "clay.documents.serverGetDocumentStatus"
            | "clay.documents.serverListDocuments"
            | "clay.workspace.serverListWorkspaceRoots"
    )
}

fn command_routing_policy(command_id: &str) -> Result<crate::protocol::RoutingPolicy, JsErrorBox> {
    if matches!(command_id, "text.insert_newline" | "text.insert_tab") {
        Ok(crate::protocol::RoutingPolicy::ClientFirstPredictable)
    } else if command_id == "completion.trigger" {
        Ok(crate::protocol::RoutingPolicy::UiReactivePriority)
    } else if command_id == "clay.documents.clientOpenFileDialog" {
        Ok(crate::protocol::RoutingPolicy::ClientUiCommand)
    } else {
        Ok(crate::protocol::RoutingPolicy::ServerFirst)
    }
}

fn serialize_key_binding(rule: &KeyBindingRule) -> Result<String, JsErrorBox> {
    serde_json::to_string(&key_binding_json(rule))
        .map_err(serialize_error("clay.keybindings.bind_failed"))
}

fn serialize_bindings(rules: &[KeyBindingRule]) -> Result<String, JsErrorBox> {
    serde_json::to_string(&Value::Array(rules.iter().map(key_binding_json).collect()))
        .map_err(serialize_error("clay.keybindings.unbind_failed"))
}

pub(super) fn key_binding_json(rule: &KeyBindingRule) -> Value {
    json!({
        "key": key_chord_string(&rule.sequence[0]),
        "command": rule.command_id,
        "scope": match rule.context {
            KeyBindingContext::Global => "global",
            _ => "editor",
        },
    })
}

pub(super) fn key_chord_string(stroke: &KeyStroke) -> String {
    let mut parts = Vec::new();
    if stroke.modifiers.control {
        parts.push("Ctrl".to_string());
    }
    if stroke.modifiers.alt {
        parts.push("Alt".to_string());
    }
    if stroke.modifiers.shift {
        parts.push("Shift".to_string());
    }
    if stroke.modifiers.super_key {
        parts.push("Super".to_string());
    }
    parts.push(match &stroke.key {
        KeyCode::Character(text) => text.to_ascii_uppercase(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Escape => "Escape".to_string(),
        KeyCode::ArrowUp => "ArrowUp".to_string(),
        KeyCode::ArrowDown => "ArrowDown".to_string(),
        KeyCode::ArrowLeft => "ArrowLeft".to_string(),
        KeyCode::ArrowRight => "ArrowRight".to_string(),
    });
    parts.join("+")
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}

fn manifest_error(
    code: &'static str,
) -> impl Fn(crate::behavior::manifest::ManifestValidationError) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: invalid behavior manifest ({error:?})"))
}
