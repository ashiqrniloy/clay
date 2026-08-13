use std::collections::HashSet;

use crate::protocol::{
    BehaviorManifest, CommandAuthority, CommandDeclaration, KeyBindingRule, KeyStroke,
    RoutingPolicy,
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

    // Phase 24.5: within one context, a rule whose sequence is a strict
    // prefix of another rule's sequence is ambiguous: the prefix always
    // fires first (its stroke completes on the earlier key), so the longer
    // chord can never be reached. The shorter (prefix) rule's command is
    // reported. Divergent rules sharing a common prefix (e.g. `Ctrl+X
    // Ctrl+A` and `Ctrl+X Ctrl+B`) stay valid; the pending-chord matcher
    // resolves them by the next stroke.
    let rules: Vec<&KeyBindingRule> = manifest.keymaps.iter().collect();
    for (index, rule) in rules.iter().enumerate() {
        for other in &rules[index + 1..] {
            if rule.context != other.context {
                continue;
            }
            if is_strict_prefix(&rule.sequence, &other.sequence) {
                return Err(ManifestValidationError::AmbiguousKeyBinding {
                    command_id: rule.command_id.clone(),
                });
            }
            if is_strict_prefix(&other.sequence, &rule.sequence) {
                return Err(ManifestValidationError::AmbiguousKeyBinding {
                    command_id: other.command_id.clone(),
                });
            }
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

    for electric in &manifest.editor_rules.electric_characters {
        if electric.trigger.is_empty() {
            return Err(ManifestValidationError::InvalidElectricCharacterRule);
        }
    }

    if manifest.editor_rules.autocomplete_triggers.len() > 32 {
        return Err(ManifestValidationError::InvalidAutocompleteTrigger);
    }
    let mut autocomplete_triggers = HashSet::new();
    for trigger in &manifest.editor_rules.autocomplete_triggers {
        if trigger.trigger.chars().count() != 1 || !autocomplete_triggers.insert(&trigger.trigger) {
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
        (RoutingPolicy::ClientUiCommand, CommandAuthority::ClientUi) => Ok(()),
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

/// True when `a` is a strict prefix of `b` (Phase 24.5 prefix-collision
/// check). Slice comparison over `&[KeyStroke]`; no string formatting.
fn is_strict_prefix(a: &[KeyStroke], b: &[KeyStroke]) -> bool {
    a.len() < b.len() && b[..a.len()] == *a
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestValidationError {
    EmptyManifestId,
    EmptyCommandId,
    DuplicateCommandId {
        command_id: String,
    },
    UnknownCommandId {
        command_id: String,
    },
    EmptyKeySequence {
        command_id: String,
    },
    /// A key rule collides with another rule in the same context: either an
    /// identical (context, sequence) pair, or a rule whose sequence is a
    /// strict prefix of another's. A prefix collision is ambiguous because
    /// the prefix fires on the earlier stroke and the longer chord becomes
    /// unreachable; the shorter (prefix) rule's command is reported. Cross-
    /// context collisions and divergent rules sharing a common prefix are
    /// valid (the pending-chord matcher resolves them by the next stroke).
    AmbiguousKeyBinding {
        command_id: String,
    },
    InvalidTabWidth,
    InvalidPairRule,
    InvalidElectricCharacterRule,
    InvalidAutocompleteTrigger,
    InvalidAutocompleteRouting,
    ExecutableOrSideEffectAuthority {
        command_id: String,
    },
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
    fn manifest_rejects_prefix_collisions_within_a_context() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.refresh",
            "Refresh",
        ));
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.refresh.save",
            "Refresh and Save",
        ));
        let ctrl_x = KeyStroke {
            key: KeyCode::Character("x".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        };
        let ctrl_s = KeyStroke {
            key: KeyCode::Character("s".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        };
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.refresh".to_string(),
            sequence: vec![ctrl_x.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.refresh.save".to_string(),
            sequence: vec![ctrl_x.clone(), ctrl_s.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });

        // The single-stroke rule is the strict prefix: it is reported.
        assert_eq!(
            validate_manifest(&manifest).unwrap_err(),
            ManifestValidationError::AmbiguousKeyBinding {
                command_id: "workspace.refresh".to_string()
            }
        );
    }

    #[test]
    fn manifest_accepts_divergent_rules_sharing_a_common_prefix() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.refresh.all",
            "Refresh All",
        ));
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.refresh.one",
            "Refresh One",
        ));
        let ctrl_x = KeyStroke {
            key: KeyCode::Character("x".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        };
        let ctrl_a = KeyStroke {
            key: KeyCode::Character("a".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        };
        let ctrl_b = KeyStroke {
            key: KeyCode::Character("b".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        };
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.refresh.all".to_string(),
            sequence: vec![ctrl_x.clone(), ctrl_a],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.refresh.one".to_string(),
            sequence: vec![ctrl_x.clone(), ctrl_b],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });

        // Neither rule is a strict prefix of the other: valid, and the
        // pending-chord matcher resolves them by the second stroke.
        validate_manifest(&manifest).unwrap();
    }

    #[test]
    fn manifest_accepts_prefix_collisions_across_contexts() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest
            .commands
            .push(CommandDeclaration::client_edit("text.insert_g", "Insert G"));
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        // `g` (EditorTextFocus) and `g g` (Global) are independent contexts.
        manifest.keymaps.push(KeyBindingRule {
            command_id: "text.insert_g".to_string(),
            sequence: vec![g.clone()],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientFirstPredictable,
        });
        manifest.keymaps.push(KeyBindingRule {
            command_id: "controlCenter.open".to_string(),
            sequence: vec![g.clone(), g.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });

        validate_manifest(&manifest).unwrap();
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
            RoutingPolicy::ClientUiCommand,
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
                RoutingPolicy::ClientUiCommand => CommandAuthority::ClientUi,
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

    #[test]
    fn manifest_rejects_malformed_autocomplete_triggers() {
        use crate::protocol::AutocompleteTrigger;

        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.autocomplete_triggers = vec![AutocompleteTrigger {
            trigger: "..".to_string(),
            routing_policy: RoutingPolicy::UiReactivePriority,
        }];
        assert_eq!(
            validate_manifest(&manifest).unwrap_err(),
            ManifestValidationError::InvalidAutocompleteTrigger
        );

        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.autocomplete_triggers = (0..33)
            .map(|index| AutocompleteTrigger {
                trigger: char::from_u32(0x21 + index).unwrap().to_string(),
                routing_policy: RoutingPolicy::UiReactivePriority,
            })
            .collect();
        assert_eq!(
            validate_manifest(&manifest).unwrap_err(),
            ManifestValidationError::InvalidAutocompleteTrigger
        );
    }

    #[test]
    fn manifest_rejects_malformed_electric_character_rules() {
        use crate::protocol::ElectricCharacterRule;

        // A trigger must be non-empty; an empty trigger is a malformed rule set
        // and must be rejected before the manifest can be installed.
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest
            .editor_rules
            .electric_characters
            .push(ElectricCharacterRule {
                trigger: String::new(),
                effect: crate::protocol::ElectricEffect::OutdentOneLevel,
            });
        assert_eq!(
            validate_manifest(&manifest).unwrap_err(),
            ManifestValidationError::InvalidElectricCharacterRule
        );

        // A well-formed electric rule (`core.code` default set) validates fine.
        let manifest = BehaviorManifest::core_code_editing(1);
        validate_manifest(&manifest).unwrap();
    }
}
