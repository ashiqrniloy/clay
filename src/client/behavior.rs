use crate::behavior::manifest::{ManifestValidationError, validate_manifest};
use crate::protocol::{
    BehaviorManifest, CommandAuthority, KeyBindingContext, KeyCode, KeyModifiers, KeyStroke,
    RoutingPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
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
    ) -> Option<AutocompleteTriggerRoute> {
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

        Some(AutocompleteTriggerRoute {
            trigger: trigger.trigger.clone(),
            routing_policy: trigger.routing_policy.clone(),
        })
    }

    pub(crate) fn route_key(&self, key: &KeyStroke) -> RoutedBehavior {
        let Some(rule) = self.active.keymaps.iter().find(|rule| {
            rule.context == KeyBindingContext::EditorTextFocus
                && rule.sequence.len() == 1
                && rule.sequence[0] == *key
        }) else {
            return self.route_unbound_key(key);
        };

        match &rule.routing_policy {
            RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck => {
                match rule.command_id.as_str() {
                    "text.insert_newline" => RoutedBehavior::ClientEdit(ClientLocalEdit::Newline),
                    "text.insert_tab" => RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText(
                        tab_text(&self.active).to_string(),
                    )),
                    _ => RoutedBehavior::Unhandled,
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
                    RoutedBehavior::ServerIntent(ServerIntentRoute {
                        command_id: rule.command_id.clone(),
                        routing_policy: rule.routing_policy.clone(),
                    })
                } else {
                    RoutedBehavior::Unhandled
                }
            }
        }
    }

    fn route_unbound_key(&self, key: &KeyStroke) -> RoutedBehavior {
        let _autocomplete_trigger = self.autocomplete_trigger_for_key(key);
        if key.modifiers == KeyModifiers::NONE {
            if let KeyCode::Character(text) = &key.key {
                if self
                    .active
                    .allows_client_first_edit(&crate::protocol::EditOperation::Insert {
                        byte_offset: 0,
                        text: text.clone(),
                    })
                {
                    return RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText(text.clone()));
                }
            }
        }

        RoutedBehavior::Unhandled
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
    ClientEdit(ClientLocalEdit),
    ServerIntent(ServerIntentRoute),
    Unhandled,
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
pub(crate) struct AutocompleteTriggerRoute {
    pub(crate) trigger: String,
    pub(crate) routing_policy: RoutingPolicy,
}

#[cfg(test)]
mod tests {
    use super::{
        AutocompleteTriggerRoute, ClientBehaviorState, ClientLocalEdit, RoutedBehavior,
        ServerIntentRoute,
    };
    use crate::protocol::{
        BehaviorManifest, CommandDeclaration, KeyBindingContext, KeyBindingRule, KeyCode,
        KeyStroke, RoutingPolicy, TabMode,
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
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText("x".to_string()))
        );
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
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText("  ".to_string()))
        );
    }

    #[test]
    fn autocomplete_trigger_declared_without_client_side_side_effect() {
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();

        let routed = state
            .autocomplete_trigger_for_key(&KeyStroke::new(KeyCode::Character(".".to_string())));

        assert_eq!(
            routed,
            Some(AutocompleteTriggerRoute {
                trigger: ".".to_string(),
                routing_policy: RoutingPolicy::UiReactivePriority,
            })
        );
        assert_eq!(
            state.route_key(&KeyStroke::new(KeyCode::Character(".".to_string()))),
            RoutedBehavior::ClientEdit(ClientLocalEdit::InsertText(".".to_string()))
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
}
