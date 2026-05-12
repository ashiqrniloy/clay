use std::collections::HashSet;

use crate::protocol::{
    BehaviorManifest, CommandAuthority, CommandDeclaration, KeyBindingRule, RoutingPolicy,
};

pub fn validate_manifest(manifest: &BehaviorManifest) -> Result<(), ManifestValidationError> {
    if manifest.manifest_id.trim().is_empty() {
        return Err(ManifestValidationError::EmptyManifestId);
    }

    let mut command_ids = HashSet::new();
    for command in &manifest.commands {
        validate_command(command)?;
        if !command_ids.insert(command.command_id.as_str()) {
            return Err(ManifestValidationError::DuplicateCommandId {
                command_id: command.command_id.clone(),
            });
        }
    }

    let mut key_rules = HashSet::new();
    for keymap in &manifest.keymaps {
        validate_key_binding(keymap, &command_ids)?;
        let key = (&keymap.context, &keymap.sequence);
        if !key_rules.insert(format!("{:?}:{:?}", key.0, key.1)) {
            return Err(ManifestValidationError::AmbiguousKeyBinding {
                command_id: keymap.command_id.clone(),
            });
        }
    }

    if manifest.editor_rules.tab.spaces_per_tab == 0 {
        return Err(ManifestValidationError::InvalidTabWidth);
    }

    for pair in &manifest.editor_rules.pairs {
        if pair.open.is_empty() || pair.close.is_empty() {
            return Err(ManifestValidationError::InvalidPairRule);
        }
    }

    for trigger in &manifest.editor_rules.autocomplete_triggers {
        if trigger.trigger.is_empty() {
            return Err(ManifestValidationError::InvalidAutocompleteTrigger);
        }
        if !matches!(trigger.routing_policy, RoutingPolicy::UiReactivePriority) {
            return Err(ManifestValidationError::InvalidAutocompleteRouting);
        }
    }

    Ok(())
}

fn validate_command(command: &CommandDeclaration) -> Result<(), ManifestValidationError> {
    if command.command_id.trim().is_empty() {
        return Err(ManifestValidationError::EmptyCommandId);
    }

    match (&command.routing_policy, &command.authority) {
        (
            RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck,
            CommandAuthority::BuiltInClientEdit,
        ) => Ok(()),
        (
            RoutingPolicy::ServerFirst
            | RoutingPolicy::ServerFirstWithLock { .. }
            | RoutingPolicy::UiReactivePriority
            | RoutingPolicy::Background,
            CommandAuthority::ServerIntent,
        ) => Ok(()),
        _ => Err(ManifestValidationError::ExecutableOrSideEffectAuthority {
            command_id: command.command_id.clone(),
        }),
    }
}

fn validate_key_binding(
    keymap: &KeyBindingRule,
    command_ids: &HashSet<&str>,
) -> Result<(), ManifestValidationError> {
    if !command_ids.contains(keymap.command_id.as_str()) {
        return Err(ManifestValidationError::UnknownCommandId {
            command_id: keymap.command_id.clone(),
        });
    }
    if keymap.sequence.is_empty() {
        return Err(ManifestValidationError::EmptyKeySequence {
            command_id: keymap.command_id.clone(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestValidationError {
    EmptyManifestId,
    EmptyCommandId,
    DuplicateCommandId { command_id: String },
    UnknownCommandId { command_id: String },
    EmptyKeySequence { command_id: String },
    AmbiguousKeyBinding { command_id: String },
    InvalidTabWidth,
    InvalidPairRule,
    InvalidAutocompleteTrigger,
    InvalidAutocompleteRouting,
    ExecutableOrSideEffectAuthority { command_id: String },
}

#[cfg(test)]
mod tests {
    use super::{ManifestValidationError, validate_manifest};
    use crate::protocol::{
        BehaviorManifest, CommandAuthority, CommandDeclaration, KeyBindingContext, KeyBindingRule,
        KeyCode, KeyStroke, LockScope, RoutingPolicy,
    };

    #[test]
    fn manifest_rejects_executable_behavior_payloads() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration {
            command_id: "workspace.delete-file".to_string(),
            display_name: "Delete File".to_string(),
            routing_policy: RoutingPolicy::ClientFirstPredictable,
            authority: CommandAuthority::ServerIntent,
        });

        let error = validate_manifest(&manifest).unwrap_err();

        assert_eq!(
            error,
            ManifestValidationError::ExecutableOrSideEffectAuthority {
                command_id: "workspace.delete-file".to_string()
            }
        );
    }

    #[test]
    fn manifest_requires_unique_command_ids_and_key_rules() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::client_edit(
            "text.insert",
            "Duplicate Insert",
        ));

        let error = validate_manifest(&manifest).unwrap_err();
        assert_eq!(
            error,
            ManifestValidationError::DuplicateCommandId {
                command_id: "text.insert".to_string()
            }
        );

        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::client_edit(
            "text.insert_newline_copy",
            "Insert Newline Copy",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "text.insert_newline_copy".to_string(),
            sequence: vec![KeyStroke::new(KeyCode::Enter)],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientFirstPredictable,
        });

        let error = validate_manifest(&manifest).unwrap_err();
        assert_eq!(
            error,
            ManifestValidationError::AmbiguousKeyBinding {
                command_id: "text.insert_newline_copy".to_string()
            }
        );
    }

    #[test]
    fn manifest_declares_all_routing_policy_variants() {
        let policies = vec![
            RoutingPolicy::ClientFirstPredictable,
            RoutingPolicy::ClientFirstRequiresAck,
            RoutingPolicy::ServerFirst,
            RoutingPolicy::ServerFirstWithLock {
                lock_scope: LockScope::Document,
            },
            RoutingPolicy::UiReactivePriority,
            RoutingPolicy::Background,
        ];

        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        for (index, policy) in policies.into_iter().enumerate() {
            let command_id = format!("test.command.{index}");
            let authority = match policy {
                RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck => {
                    CommandAuthority::BuiltInClientEdit
                }
                _ => CommandAuthority::ServerIntent,
            };
            manifest.commands.push(CommandDeclaration {
                command_id,
                display_name: format!("Test Command {index}"),
                routing_policy: policy,
                authority,
            });
        }

        validate_manifest(&manifest).unwrap();
    }
}
