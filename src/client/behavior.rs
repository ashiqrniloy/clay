use crate::behavior::manifest::{ManifestValidationError, validate_manifest};
use crate::protocol::{
    BehaviorManifest, CommandAuthority, CompletionTrigger, KeyBindingContext, KeyCode,
    KeyModifiers, KeyStroke, RoutingPolicy,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClientBehaviorState {
    active: BehaviorManifest,
}

impl ClientBehaviorState {
    pub(crate) fn new(manifest: BehaviorManifest) -> Result<Self, ManifestValidationError> {
        validate_manifest(&manifest)?;
        Ok(Self { active: manifest })
    }

    #[cfg(test)]
    pub(crate) fn active_manifest(&self) -> &BehaviorManifest {
        &self.active
    }

    #[cfg(test)]
    pub(crate) fn behavior_version(&self) -> crate::protocol::BehaviorVersion {
        self.active.behavior_version
    }

    pub(crate) fn install_replacement(
        &mut self,
        manifest: BehaviorManifest,
    ) -> Result<(), ManifestValidationError> {
        validate_manifest(&manifest)?;
        self.active = manifest;
        Ok(())
    }

    pub(crate) fn autocomplete_trigger_for_key(
        &self,
        key: &KeyStroke,
    ) -> Option<CompletionTriggerRoute> {
        if key.modifiers != KeyModifiers::NONE {
            return None;
        }
        let KeyCode::Character(text) = &key.key else {
            return None;
        };
        let trigger = self
            .active
            .editor_rules
            .autocomplete_triggers
            .iter()
            .find(|trigger| trigger.trigger == *text)?;

        Some(CompletionTriggerRoute {
            trigger: CompletionTrigger::Character(trigger.trigger.clone()),
            routing_policy: trigger.routing_policy.clone(),
        })
    }

    pub(crate) fn route_key(&self, key: &KeyStroke) -> RoutedBehavior {
        let Some(rule) = self.active.keymaps.iter().find(|rule| {
            rule.context == KeyBindingContext::EditorTextFocus
                && rule.sequence.len() == 1
                && key_matches_binding(&rule.sequence[0], key)
        }) else {
            return self.route_unbound_key(key);
        };

        match &rule.routing_policy {
            RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck => {
                match rule.command_id.as_str() {
                    "text.insert_newline" => {
                        RoutedBehavior::ClientEdit(ClientLocalEdit::Newline, None)
                    }
                    "text.insert_tab" => RoutedBehavior::ClientEdit(
                        ClientLocalEdit::InsertText(tab_text(&self.active).to_string()),
                        None,
                    ),
                    _ => RoutedBehavior::Unhandled,
                }
            }
            RoutingPolicy::ClientUiCommand => {
                let authority = self
                    .active
                    .commands
                    .iter()
                    .find(|command| command.command_id == rule.command_id)
                    .map(|command| command.authority.clone());
                if authority == Some(CommandAuthority::ClientUi) {
                    RoutedBehavior::ClientUiCommand(ClientUiCommandRoute {
                        command_id: rule.command_id.clone(),
                        routing_policy: rule.routing_policy.clone(),
                    })
                } else {
                    RoutedBehavior::Unhandled
                }
            }
            RoutingPolicy::ServerFirst
            | RoutingPolicy::ServerFirstWithLock { .. }
            | RoutingPolicy::UiReactivePriority
            | RoutingPolicy::Background => {
                let authority = self
                    .active
                    .commands
                    .iter()
                    .find(|command| command.command_id == rule.command_id)
                    .map(|command| command.authority.clone());
                if authority == Some(CommandAuthority::ServerIntent) {
                    if rule.command_id == "completion.trigger"
                        && matches!(rule.routing_policy, RoutingPolicy::UiReactivePriority)
                    {
                        RoutedBehavior::Completion(CompletionTriggerRoute {
                            trigger: CompletionTrigger::Manual,
                            routing_policy: rule.routing_policy.clone(),
                        })
                    } else if let Some(feature) =
                        language_intelligence_feature_for_command(&rule.command_id)
                        && matches!(rule.routing_policy, RoutingPolicy::UiReactivePriority)
                    {
                        RoutedBehavior::LanguageIntelligence(LanguageIntelligenceTriggerRoute {
                            feature,
                            routing_policy: rule.routing_policy.clone(),
                        })
                    } else {
                        RoutedBehavior::ServerIntent(ServerIntentRoute {
                            command_id: rule.command_id.clone(),
                            routing_policy: rule.routing_policy.clone(),
                        })
                    }
                } else {
                    RoutedBehavior::Unhandled
                }
            }
        }
    }

    fn route_unbound_key(&self, key: &KeyStroke) -> RoutedBehavior {
        let completion_trigger = self.autocomplete_trigger_for_key(key);
        if !key.modifiers.control
            && !key.modifiers.alt
            && !key.modifiers.super_key
            && let KeyCode::Character(text) = &key.key
            && self
                .active
                .allows_client_first_edit(&crate::protocol::EditOperation::Insert {
                    byte_offset: 0,
                    text: text.clone(),
                })
        {
            return RoutedBehavior::ClientEdit(
                ClientLocalEdit::InsertText(text.clone()),
                completion_trigger,
            );
        }

        RoutedBehavior::Unhandled
    }
}

fn key_matches_binding(binding: &KeyStroke, event: &KeyStroke) -> bool {
    if binding.modifiers != event.modifiers {
        return false;
    }
    match (&binding.key, &event.key) {
        (KeyCode::Character(binding), KeyCode::Character(event)) => {
            binding == event || binding.eq_ignore_ascii_case(event)
        }
        _ => binding.key == event.key,
    }
}

fn tab_text(manifest: &BehaviorManifest) -> String {
    use crate::protocol::TabMode;

    match manifest.editor_rules.tab.mode {
        TabMode::InsertSpaces => " ".repeat(manifest.editor_rules.tab.spaces_per_tab as usize),
        TabMode::InsertTabCharacter => "\t".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RoutedBehavior {
    ClientEdit(ClientLocalEdit, Option<CompletionTriggerRoute>),
    Completion(CompletionTriggerRoute),
    LanguageIntelligence(LanguageIntelligenceTriggerRoute),
    ServerIntent(ServerIntentRoute),
    ClientUiCommand(ClientUiCommandRoute),
    Unhandled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LanguageIntelligenceTriggerRoute {
    pub(crate) feature: crate::protocol::LanguageIntelligenceFeature,
    pub(crate) routing_policy: RoutingPolicy,
}

/// Maps built-in `clay.language.*` command IDs to language-intelligence features.
pub fn language_intelligence_feature_for_command(
    command_id: &str,
) -> Option<crate::protocol::LanguageIntelligenceFeature> {
    use crate::protocol::LanguageIntelligenceFeature;
    match command_id {
        "clay.language.hover" => Some(LanguageIntelligenceFeature::Hover),
        "clay.language.goToDefinition" => Some(LanguageIntelligenceFeature::GoToDefinition),
        "clay.language.codeActions" => Some(LanguageIntelligenceFeature::CodeAction),
        "clay.language.signatureHelp" => Some(LanguageIntelligenceFeature::SignatureHelp),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClientLocalEdit {
    InsertText(String),
    Newline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerIntentRoute {
    pub(crate) command_id: String,
    pub(crate) routing_policy: RoutingPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientUiCommandRoute {
    pub command_id: String,
    pub routing_policy: RoutingPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionTriggerRoute {
    pub(crate) trigger: CompletionTrigger,
    pub(crate) routing_policy: RoutingPolicy,
}

#[cfg(test)]
mod tests {
    use super::{
        ClientBehaviorState, ClientLocalEdit, ClientUiCommandRoute, CompletionTriggerRoute,
        RoutedBehavior, ServerIntentRoute,
    };
    use crate::protocol::{
        BehaviorManifest, CommandDeclaration, CompletionTrigger, KeyBindingContext, KeyBindingRule,
        KeyCode, KeyModifiers, KeyStroke, RoutingPolicy, TabMode,
    };

    #[test]
    fn client_installs_valid_manifest_atomically() {
        let mut state =
            ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();
        let replacement = BehaviorManifest::minimal_text_editing(2);

        state.install_replacement(replacement.clone()).unwrap();

        assert_eq!(state.behavior_version(), 2);
        assert_eq!(state.active_manifest(), &replacement);
    }

    #[test]
    fn client_keeps_previous_manifest_when_replacement_invalid() {
        let mut state =
            ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();
        let previous = state.active_manifest().clone();
        let mut invalid = BehaviorManifest::minimal_text_editing(2);
        invalid
            .commands
            .push(CommandDeclaration::client_edit("text.insert", "Duplicate"));

        assert!(state.install_replacement(invalid).is_err());

        assert_eq!(state.behavior_version(), 1);
        assert_eq!(state.active_manifest(), &previous);
    }

    #[test]
    fn client_routes_client_first_key_without_ipc_wait() {
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();

        let routed = state.route_key(&KeyStroke::new(KeyCode::Character("x".to_string())));

        assert_eq!(
            routed,
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText("x".to_string()), None)
        );
    }

    #[test]
    fn client_routes_shifted_printable_key_as_text_input() {
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();
        let shifted_a = KeyStroke {
            key: KeyCode::Character("A".to_string()),
            modifiers: KeyModifiers {
                shift: true,
                ..KeyModifiers::NONE
            },
        };

        let routed = state.route_key(&shifted_a);

        assert_eq!(
            routed,
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText("A".to_string()), None)
        );
    }

    #[test]
    fn shifted_printable_unbound_character_still_inserts_shifted_text() {
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character("!".to_string()),
            modifiers: KeyModifiers {
                shift: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText("!".to_string()), None)
        );
    }

    #[test]
    fn client_does_not_route_control_character_as_text_input() {
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();
        let control_a = KeyStroke {
            key: KeyCode::Character("a".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        };

        let routed = state.route_key(&control_a);

        assert_eq!(routed, RoutedBehavior::Unhandled);
    }

    #[test]
    fn client_routes_tab_from_manifest_rules() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.editor_rules.tab.mode = TabMode::InsertSpaces;
        manifest.editor_rules.tab.spaces_per_tab = 2;
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke::new(KeyCode::Tab));

        assert_eq!(
            routed,
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText("  ".to_string()), None)
        );
    }

    #[test]
    fn autocomplete_trigger_declared_without_client_side_side_effect() {
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();

        let routed = state
            .autocomplete_trigger_for_key(&KeyStroke::new(KeyCode::Character(".".to_string())));

        assert_eq!(
            routed,
            Some(CompletionTriggerRoute {
                trigger: CompletionTrigger::Character(".".to_string()),
                routing_policy: RoutingPolicy::UiReactivePriority,
            })
        );
        assert_eq!(
            state.route_key(&KeyStroke::new(KeyCode::Character(".".to_string()))),
            RoutedBehavior::ClientEdit(
                ClientLocalEdit::InsertText(".".to_string()),
                Some(CompletionTriggerRoute {
                    trigger: CompletionTrigger::Character(".".to_string()),
                    routing_policy: RoutingPolicy::UiReactivePriority,
                }),
            )
        );
    }

    #[test]
    fn client_routes_manual_completion_as_first_class_completion_route() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.keymaps.push(KeyBindingRule {
            command_id: "completion.trigger".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character(" ".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::UiReactivePriority,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character(" ".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            RoutedBehavior::Completion(CompletionTriggerRoute {
                trigger: CompletionTrigger::Manual,
                routing_policy: RoutingPolicy::UiReactivePriority,
            })
        );
    }

    #[test]
    fn client_routes_language_intelligence_commands_as_ui_reactive_triggers() {
        use super::{LanguageIntelligenceTriggerRoute, language_intelligence_feature_for_command};
        use crate::protocol::LanguageIntelligenceFeature;

        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        // Commands are already discoverable via default_commands(); only bind a key.
        manifest.keymaps.push(KeyBindingRule {
            command_id: "clay.language.goToDefinition".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("d".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::UiReactivePriority,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character("d".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            RoutedBehavior::LanguageIntelligence(LanguageIntelligenceTriggerRoute {
                feature: LanguageIntelligenceFeature::GoToDefinition,
                routing_policy: RoutingPolicy::UiReactivePriority,
            })
        );
        assert_eq!(
            language_intelligence_feature_for_command("clay.language.hover"),
            Some(LanguageIntelligenceFeature::Hover)
        );
    }

    #[test]
    fn client_routes_server_first_command_as_intent() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.save",
            "Save Workspace File",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.save".to_string(),
            sequence: vec![KeyStroke::new(KeyCode::Character("s".to_string()))],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke::new(KeyCode::Character("s".to_string())));

        assert_eq!(
            routed,
            RoutedBehavior::ServerIntent(ServerIntentRoute {
                command_id: "workspace.save".to_string(),
                routing_policy: RoutingPolicy::ServerFirst,
            })
        );
    }

    #[test]
    fn client_routes_open_file_dialog_as_client_ui_intent() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::client_ui(
            "clay.documents.clientOpenFileDialog",
            "Open File Dialog",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "clay.documents.clientOpenFileDialog".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("o".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientUiCommand,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character("o".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            RoutedBehavior::ClientUiCommand(ClientUiCommandRoute {
                command_id: "clay.documents.clientOpenFileDialog".to_string(),
                routing_policy: RoutingPolicy::ClientUiCommand,
            })
        );
    }

    #[test]
    fn shifted_character_key_binding_matches_lowercase_manifest_rule() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::client_ui(
            "clay.workspace.clientOpenFolderDialog",
            "Open Folder Dialog",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "clay.workspace.clientOpenFolderDialog".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("o".to_string()),
                modifiers: KeyModifiers {
                    shift: true,
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientUiCommand,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character("O".to_string()),
            modifiers: KeyModifiers {
                shift: true,
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            RoutedBehavior::ClientUiCommand(ClientUiCommandRoute {
                command_id: "clay.workspace.clientOpenFolderDialog".to_string(),
                routing_policy: RoutingPolicy::ClientUiCommand,
            })
        );
    }

    #[test]
    fn configuration_shifted_folder_binding_routes_on_linux_key_event() {
        // Locks the end-to-end configuration contract for the file-browser
        // workflow: configuration publishes `Ctrl+Shift+O` with a lowercase
        // manifest chord, and a Linux/GNOME key event reporting uppercase `O`
        // (because Shift is held) must still route to the folder picker. This
        // mirrors the workflow fixture's `bindKey("Ctrl+Shift+O",
        // clientOpenFolderDialog())` contract.
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::client_ui(
            "clay.workspace.clientOpenFolderDialog",
            "Open Folder Dialog",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "clay.workspace.clientOpenFolderDialog".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("o".to_string()),
                modifiers: KeyModifiers {
                    shift: true,
                    control: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientUiCommand,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character("O".to_string()),
            modifiers: KeyModifiers {
                shift: true,
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            RoutedBehavior::ClientUiCommand(ClientUiCommandRoute {
                command_id: "clay.workspace.clientOpenFolderDialog".to_string(),
                routing_policy: RoutingPolicy::ClientUiCommand,
            })
        );
    }

    #[test]
    fn open_file_dialog_binding_is_not_hard_coded() {
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character("o".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(routed, RoutedBehavior::Unhandled);
    }
}
