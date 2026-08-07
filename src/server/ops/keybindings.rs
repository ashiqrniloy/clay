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
        "space" => KeyCode::Character(" ".to_string()),
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
    // Plan 071 task 10: the text-object/smart-select command-ID surface is
    // generated (kind x scope x direction), so parse instead of enumerating.
    if crate::protocol::SelectionQuery::from_command_id(command_id).is_some() {
        return true;
    }
    // Phase 22.4: the numbered tab families parse the same way (1..=9 only).
    if tab_family_variant(command_id).is_some() {
        return true;
    }
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
            | "clay.workspace.clientOpenFolderDialog"
            | "clay.workspace.openFuzzyFile"
            | "clay.workspace.toggleFileBrowser"
            | "clay.editor.clientCopySelection"
            | "clay.editor.clientCutSelection"
            | "clay.editor.clientPasteClipboard"
            | "clay.editor.clientUndo"
            | "clay.editor.clientRedo"
            | "clay.editor.clientShowOpenDocuments"
            | "clay.editor.clientRequestResync"
            | "clay.editor.clientDismissRecovery"
            | "clay.editor.clientMoveCursor.nextWordStart"
            | "clay.editor.clientMoveCursor.prevWordStart"
            | "clay.editor.clientMoveCursor.nextParagraph"
            | "clay.editor.clientMoveCursor.prevParagraph"
            | "clay.editor.clientSetSelection.selectWord"
            | "clay.editor.clientSetSelection.selectLine"
            | "clay.editor.clientAddCursor.below"
            | "clay.editor.clientAddCursor.above"
            | "clay.editor.clientColumnSelect.down"
            | "clay.editor.clientColumnSelect.up"
            | "clay.editor.clientColumnSelect.left"
            | "clay.editor.clientColumnSelect.right"
            | "clay.editor.clientSelectNextMatch"
            | "clay.editor.clientSelectPrevMatch"
            | "clay.editor.clientSelectAllMatches"
            | "clay.editor.clientCancelMultipleSelections"
            | "clay.editor.clientKeepSelection"
            | "clay.editor.clientRemoveSelection"
            | "clay.editor.clientUndoCursorMove"
            | "clay.language.hover"
            | "clay.language.goToDefinition"
            | "clay.language.codeActions"
            | "clay.language.signatureHelp"
            | "clay.documents.serverSaveDocument"
            | "clay.runtime.reloadConfiguration"
            | "clay.documents.serverReloadDocument"
            | "clay.documents.serverGetDocumentStatus"
            | "clay.documents.serverListDocuments"
            | "clay.workspace.serverListWorkspaceRoots"
            | "clay.shell.clientSplitPaneVertical"
            | "clay.shell.clientSplitPaneHorizontal"
            | "clay.shell.clientAddEqualPane"
            | "clay.shell.clientClosePane"
            | "clay.shell.clientFocusPaneNext"
            | "clay.shell.clientFocusPanePrev"
            | "clay.shell.clientResizePaneLeft"
            | "clay.shell.clientResizePaneRight"
            | "clay.shell.clientResizePaneUp"
            | "clay.shell.clientResizePaneDown"
            | "clay.shell.clientMovePaneNext"
            | "clay.shell.clientMovePanePrev"
            | "clay.shell.clientTabNext"
            | "clay.shell.clientTabPrev"
            | "clay.shell.clientTabNew"
            | "clay.shell.clientTabClose"
            | "clay.shell.clientTabMoveLeft"
            | "clay.shell.clientTabMoveRight"
    )
}

/// Phase 22.4: numbered tab command families. `clay.shell.clientTabActivate.N`
/// and `clay.shell.clientTabMoveTo.N` exist for N in 1..=9 only — "numbered
/// switch beyond 9" is the policy that no such command ID exists (the dotted
/// family rides the Plan 071 `SelectionQuery` parse precedent).
fn tab_family_variant(command_id: &str) -> Option<u32> {
    let (family, suffix) = command_id.rsplit_once('.')?;
    if !matches!(
        family,
        "clay.shell.clientTabActivate" | "clay.shell.clientTabMoveTo"
    ) {
        return None;
    }
    let n: u32 = suffix.parse().ok()?;
    (1..=9).contains(&n).then_some(n)
}

fn command_routing_policy(command_id: &str) -> Result<crate::protocol::RoutingPolicy, JsErrorBox> {
    if matches!(command_id, "text.insert_newline" | "text.insert_tab") {
        Ok(crate::protocol::RoutingPolicy::ClientFirstPredictable)
    } else if command_id == "clay.runtime.reloadConfiguration" {
        Ok(crate::protocol::RoutingPolicy::ServerFirstWithLock {
            lock_scope: crate::protocol::LockScope::Behavior,
        })
    } else if command_id == "completion.trigger"
        || crate::client::behavior::language_intelligence_feature_for_command(command_id).is_some()
    {
        Ok(crate::protocol::RoutingPolicy::UiReactivePriority)
    } else if crate::protocol::SelectionQuery::from_command_id(command_id).is_some() {
        // Text-object/smart-select: UI-reactive read-only server query; the
        // client captures its selection set and applies returned ranges.
        Ok(crate::protocol::RoutingPolicy::UiReactivePriority)
    } else if tab_family_variant(command_id).is_some() {
        // Phase 22.4: numbered tab commands are client-routed like the flat
        // tab IDs (driver executes the tab operation locally).
        Ok(crate::protocol::RoutingPolicy::ClientUiCommand)
    } else if matches!(
        command_id,
        "clay.documents.clientOpenFileDialog"
            | "clay.workspace.clientOpenFolderDialog"
            | "clay.editor.clientCopySelection"
            | "clay.editor.clientCutSelection"
            | "clay.editor.clientPasteClipboard"
            | "clay.editor.clientUndo"
            | "clay.editor.clientRedo"
            | "clay.editor.clientShowOpenDocuments"
            | "clay.editor.clientRequestResync"
            | "clay.editor.clientDismissRecovery"
            | "clay.editor.clientMoveCursor.nextWordStart"
            | "clay.editor.clientMoveCursor.prevWordStart"
            | "clay.editor.clientMoveCursor.nextParagraph"
            | "clay.editor.clientMoveCursor.prevParagraph"
            | "clay.editor.clientSetSelection.selectWord"
            | "clay.editor.clientSetSelection.selectLine"
            | "clay.editor.clientAddCursor.below"
            | "clay.editor.clientAddCursor.above"
            | "clay.editor.clientColumnSelect.down"
            | "clay.editor.clientColumnSelect.up"
            | "clay.editor.clientColumnSelect.left"
            | "clay.editor.clientColumnSelect.right"
            | "clay.editor.clientSelectNextMatch"
            | "clay.editor.clientSelectPrevMatch"
            | "clay.editor.clientSelectAllMatches"
            | "clay.editor.clientCancelMultipleSelections"
            | "clay.editor.clientKeepSelection"
            | "clay.editor.clientRemoveSelection"
            | "clay.editor.clientUndoCursorMove"
            | "clay.shell.clientSplitPaneVertical"
            | "clay.shell.clientSplitPaneHorizontal"
            | "clay.shell.clientAddEqualPane"
            | "clay.shell.clientClosePane"
            | "clay.shell.clientFocusPaneNext"
            | "clay.shell.clientFocusPanePrev"
            | "clay.shell.clientResizePaneLeft"
            | "clay.shell.clientResizePaneRight"
            | "clay.shell.clientResizePaneUp"
            | "clay.shell.clientResizePaneDown"
            | "clay.shell.clientMovePaneNext"
            | "clay.shell.clientMovePanePrev"
            | "clay.shell.clientTabNext"
            | "clay.shell.clientTabPrev"
            | "clay.shell.clientTabNew"
            | "clay.shell.clientTabClose"
            | "clay.shell.clientTabMoveLeft"
            | "clay.shell.clientTabMoveRight"
    ) {
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

#[cfg(test)]
mod tests {
    use super::{command_routing_policy, is_runtime_bindable_command, validate_command_id};
    use crate::protocol::RoutingPolicy;

    #[test]
    fn documented_space_chord_parses_for_manual_completion() {
        assert_eq!(
            super::parse_key_chord("Ctrl+Space").unwrap(),
            crate::protocol::KeyStroke {
                key: crate::protocol::KeyCode::Character(" ".to_string()),
                modifiers: crate::protocol::KeyModifiers {
                    control: true,
                    ..crate::protocol::KeyModifiers::NONE
                },
            }
        );
    }

    #[test]
    fn language_intelligence_commands_are_runtime_bindable_ui_reactive_routes() {
        for command in [
            "clay.language.hover",
            "clay.language.goToDefinition",
            "clay.language.codeActions",
            "clay.language.signatureHelp",
        ] {
            assert!(is_runtime_bindable_command(command));
            assert_eq!(
                command_routing_policy(command).unwrap(),
                RoutingPolicy::UiReactivePriority
            );
        }
    }

    #[test]
    fn undo_redo_commands_are_runtime_bindable_client_ui_routes() {
        for command in [
            "clay.editor.clientUndo",
            "clay.editor.clientRedo",
            "clay.editor.clientShowOpenDocuments",
            "clay.editor.clientRequestResync",
            "clay.editor.clientDismissRecovery",
        ] {
            assert!(is_runtime_bindable_command(command));
            assert_eq!(
                command_routing_policy(command).unwrap(),
                RoutingPolicy::ClientUiCommand
            );
        }
    }

    #[test]
    fn textobject_and_smart_select_commands_are_bindable_ui_reactive() {
        // Plan 071 task 10: the generated command-ID surface is bindable and
        // routes UI-reactive; unknown kinds/scopes/directions stay unbindable
        // (deny-by-default).
        for command in [
            "clay.editor.clientSelectTextobject.function.inner",
            "clay.editor.clientSelectTextobject.function.around.next",
            "clay.editor.clientSelectTextobject.comment.around.previous",
            "clay.editor.clientSmartSelect.expand",
            "clay.editor.clientSmartSelect.shrink",
        ] {
            assert!(is_runtime_bindable_command(command));
            assert_eq!(
                command_routing_policy(command).unwrap(),
                RoutingPolicy::UiReactivePriority
            );
        }
        for command in [
            "clay.editor.clientSelectTextobject.widget.inner",
            "clay.editor.clientSelectTextobject.function.side",
            "clay.editor.clientSmartSelect.grow",
        ] {
            assert!(!is_runtime_bindable_command(command));
        }
    }

    #[test]
    fn phase_22_1_shell_commands_are_bindable_and_client_ui_routed() {
        let shell_commands = [
            "clay.shell.clientSplitPaneVertical",
            "clay.shell.clientSplitPaneHorizontal",
            "clay.shell.clientAddEqualPane",
            "clay.shell.clientClosePane",
            "clay.shell.clientFocusPaneNext",
            "clay.shell.clientFocusPanePrev",
            "clay.shell.clientResizePaneLeft",
            "clay.shell.clientResizePaneRight",
            "clay.shell.clientResizePaneUp",
            "clay.shell.clientResizePaneDown",
            "clay.shell.clientMovePaneNext",
            "clay.shell.clientMovePanePrev",
        ];
        for command in shell_commands {
            assert!(
                is_runtime_bindable_command(command),
                "{} should be bindable",
                command
            );
            assert_eq!(
                command_routing_policy(command).unwrap(),
                RoutingPolicy::ClientUiCommand,
                "{} should be ClientUiCommand-routed",
                command
            );
        }
        // Unknown clay.shell.* IDs are rejected.
        assert!(!is_runtime_bindable_command("clay.shell.clientUnknown"));
        assert!(!is_runtime_bindable_command(
            "clay.shell.clientSplitPane.diagonal"
        ));
    }

    #[test]
    fn phase_22_4_tab_commands_are_bindable_and_client_ui_routed() {
        for command in [
            "clay.shell.clientTabNext",
            "clay.shell.clientTabPrev",
            "clay.shell.clientTabNew",
            "clay.shell.clientTabClose",
            "clay.shell.clientTabMoveLeft",
            "clay.shell.clientTabMoveRight",
        ] {
            assert!(
                is_runtime_bindable_command(command),
                "{command} should be bindable"
            );
            assert_eq!(
                command_routing_policy(command).unwrap(),
                RoutingPolicy::ClientUiCommand,
                "{command} should be ClientUiCommand-routed"
            );
        }
        // Numbered families: every 1..=9 variant is bindable and client-routed.
        for n in 1..=9 {
            for command in [
                format!("clay.shell.clientTabActivate.{n}"),
                format!("clay.shell.clientTabMoveTo.{n}"),
            ] {
                assert!(
                    is_runtime_bindable_command(&command),
                    "{command} should be bindable"
                );
                assert_eq!(
                    command_routing_policy(&command).unwrap(),
                    RoutingPolicy::ClientUiCommand,
                    "{command} should be ClientUiCommand-routed"
                );
            }
        }
        // Deny-by-default: only 1..=9 exist. 0, 10, non-numeric suffixes, and
        // unknown tab-ish IDs reject; the "beyond 9" policy is that no such
        // command ID is declared.
        for command in [
            "clay.shell.clientTabActivate.0",
            "clay.shell.clientTabActivate.10",
            "clay.shell.clientTabMoveTo.0",
            "clay.shell.clientTabMoveTo.10",
            "clay.shell.clientTabActivate.1a",
            "clay.shell.clientTabBogus",
        ] {
            assert!(
                !is_runtime_bindable_command(command),
                "{command} must not be bindable"
            );
            assert!(
                validate_command_id(command).is_err(),
                "{command} must reject through the bindKey validation gate"
            );
        }
        // Representative IDs pass the full bindKey validation gate (the op
        // calls validate_command_id, which allow-lists + routes + rejects
        // when-clauses).
        for command in [
            "clay.shell.clientTabClose",
            "clay.shell.clientTabActivate.5",
            "clay.shell.clientTabMoveTo.9",
        ] {
            assert_eq!(
                validate_command_id(command).unwrap(),
                command,
                "{command} must pass the bindKey validation gate"
            );
        }
    }
}
