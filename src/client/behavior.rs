use crate::behavior::manifest::{ManifestValidationError, validate_manifest};
use crate::protocol::{
    BehaviorManifest, CommandAuthority, CompletionTrigger, KeyBindingContext, KeyBindingRule,
    KeyCode, KeyModifiers, KeyStroke, RoutingPolicy,
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
        // Phase 24.5: the single-stroke path delegates to the pure sequence
        // matcher with an empty pending buffer. A pending prefix cannot
        // dispatch here (callers with chord state use `route_key_sequence`);
        // it must not insert text, so it maps to Unhandled.
        match self.route_key_sequence(&[], key) {
            ChordRouteOutcome::Matched(behavior) => behavior,
            ChordRouteOutcome::Pending => RoutedBehavior::Unhandled,
            ChordRouteOutcome::Mismatch => self.route_unbound_key(key),
        }
    }

    /// Pure multi-stroke matcher (Phase 24.5): given the accumulated pending
    /// strokes and the incoming key, decide whether the extended candidate
    /// exactly matches a rule (`Matched`), is a strict prefix of some rule
    /// (`Pending`), or matches nothing (`Mismatch`). Contexts are considered
    /// in the Phase 22.1 order (EditorTextFocus before Global), and within a
    /// context an exact match wins over a longer rule's prefix. Allocation-
    /// free: rules are compared slice-wise against `pending` + `key`.
    pub(crate) fn route_key_sequence(
        &self,
        pending: &[KeyStroke],
        key: &KeyStroke,
    ) -> ChordRouteOutcome {
        self.route_key_sequence_in_contexts(
            pending,
            key,
            &[
                KeyBindingContext::EditorTextFocus,
                KeyBindingContext::Global,
            ],
        )
    }

    /// Match only global bindings. Welcome is not an editor text surface, but
    /// global commands must still be reachable while it is visible.
    pub(crate) fn route_global_key_sequence(
        &self,
        pending: &[KeyStroke],
        key: &KeyStroke,
    ) -> ChordRouteOutcome {
        self.route_key_sequence_in_contexts(pending, key, &[KeyBindingContext::Global])
    }

    fn route_key_sequence_in_contexts(
        &self,
        pending: &[KeyStroke],
        key: &KeyStroke,
        contexts: &[KeyBindingContext],
    ) -> ChordRouteOutcome {
        let candidate_len = pending.len() + 1;
        for context in contexts {
            let mut saw_prefix = false;
            for rule in self.active.keymaps.iter() {
                if &rule.context != context {
                    continue;
                }
                let sequence = &rule.sequence;
                if sequence.len() == candidate_len
                    && sequence[..pending.len()] == *pending
                    && key_matches_binding(&sequence[pending.len()], key)
                {
                    return ChordRouteOutcome::Matched(self.dispatch_rule(rule));
                }
                if sequence.len() > candidate_len
                    && sequence[..pending.len()] == *pending
                    && key_matches_binding(&sequence[pending.len()], key)
                {
                    saw_prefix = true;
                }
            }
            if saw_prefix {
                return ChordRouteOutcome::Pending;
            }
        }
        ChordRouteOutcome::Mismatch
    }

    /// Dispatch a matched keymap rule to its routed behavior.
    fn dispatch_rule(&self, rule: &KeyBindingRule) -> RoutedBehavior {
        if crate::masonry_editor::EditorClientCommand::from_command_id(&rule.command_id).is_some() {
            return RoutedBehavior::ClientUiCommand(ClientUiCommandRoute {
                command_id: rule.command_id.clone(),
                routing_policy: rule.routing_policy.clone(),
            });
        }
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

/// Phase 24.5: non-mutating probe used by widget key handling to give the
/// behavior manifest precedence over hard-coded platform shortcuts. True
/// when `key` is the first stroke of any bound rule — a single-stroke exact
/// match or the prefix of a multi-stroke chord — in any context. That is
/// exactly the condition under which `route_key_sequence(&[], key)` would
/// return `Matched` or `Pending` from a fresh pending buffer, so this lets the
/// widget divert a manifest-claimed first stroke (e.g. `Ctrl+X`, the prefix
/// of the `Ctrl+X Ctrl+P` Command Centre chord) to the manifest instead of
/// letting a hard-coded shortcut (e.g. Ctrl+X cut) swallow it. Borrowed only:
/// no manifest clone or validation, so it is cheap to run on every keystroke.
pub(crate) fn manifest_claims_chord(manifest: &BehaviorManifest, key: &KeyStroke) -> bool {
    manifest
        .keymaps
        .iter()
        .any(|rule| !rule.sequence.is_empty() && key_matches_binding(&rule.sequence[0], key))
}

fn tab_text(manifest: &BehaviorManifest) -> String {
    use crate::protocol::TabMode;

    match manifest.editor_rules.tab.mode {
        TabMode::InsertSpaces => " ".repeat(manifest.editor_rules.tab.spaces_per_tab as usize),
        TabMode::InsertTabCharacter => "\t".to_string(),
    }
}

/// Phase 24.5: outcome of routing an incoming key against the accumulated
/// pending chord. `#[must_use]`: dropping the outcome would swallow either a
/// dispatch or a cancel decision.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChordRouteOutcome {
    /// The extended candidate exactly matched a rule; dispatch it.
    Matched(RoutedBehavior),
    /// The extended candidate is a strict prefix of some rule; keep waiting.
    Pending,
    /// No rule has the extended candidate as a prefix; cancel and re-evaluate
    /// the incoming key as a fresh stroke.
    Mismatch,
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

/// Maps built-in `language.*` command IDs to language-intelligence features.
pub fn language_intelligence_feature_for_command(
    command_id: &str,
) -> Option<crate::protocol::LanguageIntelligenceFeature> {
    use crate::protocol::LanguageIntelligenceFeature;
    match command_id {
        "language.hover" => Some(LanguageIntelligenceFeature::Hover),
        "language.goToDefinition" => Some(LanguageIntelligenceFeature::GoToDefinition),
        "language.codeActions" => Some(LanguageIntelligenceFeature::CodeAction),
        "language.signatureHelp" => Some(LanguageIntelligenceFeature::SignatureHelp),
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
        ChordRouteOutcome, ClientBehaviorState, ClientLocalEdit, ClientUiCommandRoute,
        CompletionTriggerRoute, RoutedBehavior, ServerIntentRoute,
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
    fn manifest_claims_chord_prefix_without_swallowing_unbound_shortcuts() {
        // Phase 24.5: the predicate that lets widget key handling give the
        // behavior manifest precedence over hard-coded platform shortcuts.
        // The shipped default manifest binds the Command Centre (`Ctrl+X
        // Ctrl+P`) and Path Browser (`Ctrl+X Ctrl+F`) Global chords, so their
        // shared first stroke `Ctrl+X` must be claimed — otherwise the
        // hard-coded Ctrl+X cut shortcut swallows it and the chord never
        // starts. Single-stroke hard-coded shortcuts (Ctrl+C copy, Ctrl+V
        // paste, Ctrl+Z undo) are NOT in the manifest, so they stay unclaimed
        // and fall through to those shortcuts unchanged.
        let manifest = BehaviorManifest::minimal_text_editing(1);
        let ctrl = |c: char| KeyStroke {
            key: KeyCode::Character(c.to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        };
        assert!(
            super::manifest_claims_chord(&manifest, &ctrl('x')),
            "Ctrl+X is the prefix of the Ctrl+X Ctrl+P / Ctrl+X Ctrl+F chords and must be claimed"
        );
        assert!(
            !super::manifest_claims_chord(&manifest, &ctrl('c')),
            "Ctrl+C copy is hard-coded, not manifest-bound; stay unclaimed"
        );
        assert!(
            !super::manifest_claims_chord(&manifest, &ctrl('v')),
            "Ctrl+V paste is hard-coded, not manifest-bound; stay unclaimed"
        );
        assert!(
            !super::manifest_claims_chord(&manifest, &ctrl('z')),
            "Ctrl+Z undo is hard-coded, not manifest-bound; stay unclaimed"
        );
        // The completing stroke of the chord is not a *first* stroke of any
        // rule, so it is unclaimed from a fresh buffer (it only matches once
        // the pending Ctrl+X prefix is in place, which `local_key` handles).
        assert!(
            !super::manifest_claims_chord(&manifest, &ctrl('p')),
            "Ctrl+P is only the second stroke; unclaimed from a fresh buffer"
        );
        assert!(
            !super::manifest_claims_chord(&manifest, &ctrl('f')),
            "Ctrl+F is only the second stroke; unclaimed from a fresh buffer"
        );
        // A plain typing key is never claimed, so the guard does not divert
        // ordinary insertion.
        assert!(
            !super::manifest_claims_chord(
                &manifest,
                &KeyStroke::new(KeyCode::Character("a".to_string()))
            ),
            "plain 'a' is not a chord first stroke"
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
            command_id: "language.goToDefinition".to_string(),
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
            language_intelligence_feature_for_command("language.hover"),
            Some(LanguageIntelligenceFeature::Hover)
        );
    }

    #[test]
    fn route_key_sequence_matches_single_stroke_without_regression() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::server_intent(
            "documents.serverSaveDocument",
            "Save Document",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "documents.serverSaveDocument".to_string(),
            sequence: vec![KeyStroke::new(KeyCode::Character("s".to_string()))],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();
        let key = KeyStroke::new(KeyCode::Character("s".to_string()));

        // Phase 24.5: `route_key` delegates to the pure matcher, so the
        // single-stroke dispatch is identical to the pre-change fast path.
        let routed = state.route_key(&key);
        assert_eq!(
            state.route_key_sequence(&[], &key),
            ChordRouteOutcome::Matched(routed.clone())
        );
        assert!(matches!(
            routed,
            RoutedBehavior::ServerIntent(ServerIntentRoute {
                command_id,
                ..
            }) if command_id == "documents.serverSaveDocument"
        ));
    }

    #[test]
    fn route_key_sequence_tracks_a_two_stroke_chord() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        // `controlCenter.open` is already declared in the default manifest.
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "controlCenter.open".to_string(),
            sequence: vec![g.clone(), g.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        // First stroke: strict prefix of the bound sequence.
        assert_eq!(
            state.route_key_sequence(&[], &g),
            ChordRouteOutcome::Pending
        );
        // Second stroke: exact match dispatches.
        assert_eq!(
            state.route_key_sequence(std::slice::from_ref(&g), &g),
            ChordRouteOutcome::Matched(RoutedBehavior::ServerIntent(ServerIntentRoute {
                command_id: "controlCenter.open".to_string(),
                routing_policy: RoutingPolicy::ServerFirst,
            }))
        );
        // A third stroke is no longer a prefix of the two-stroke rule.
        assert_eq!(
            state.route_key_sequence(&[g.clone(), g.clone()], &g),
            ChordRouteOutcome::Mismatch
        );
    }

    #[test]
    fn route_key_sequence_mismatch_clears_the_chord() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::server_intent(
            "workspace.refresh",
            "Refresh Workspace",
        ));
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        let x = KeyStroke::new(KeyCode::Character("x".to_string()));
        let z = KeyStroke::new(KeyCode::Character("z".to_string()));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.refresh".to_string(),
            sequence: vec![g.clone(), x.clone()],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        assert_eq!(
            state.route_key_sequence(&[], &g),
            ChordRouteOutcome::Pending
        );
        // The completing stroke dispatches.
        assert!(matches!(
            state.route_key_sequence(std::slice::from_ref(&g), &x),
            ChordRouteOutcome::Matched(_)
        ));
        // An unrelated key after the prefix mismatches: the surface cancels
        // and re-evaluates it fresh, which the matcher reports as unbound.
        assert_eq!(
            state.route_key_sequence(&[g], &z),
            ChordRouteOutcome::Mismatch
        );
        assert_eq!(
            state.route_key_sequence(&[], &z),
            ChordRouteOutcome::Mismatch
        );
    }

    #[test]
    fn route_key_sequence_keeps_editor_context_precedence_for_exact_matches() {
        // Same first stroke in both contexts: the EditorTextFocus exact rule
        // wins even though the Global rule could continue a chord.
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest
            .commands
            .push(CommandDeclaration::server_intent("editor.save", "Save"));
        manifest
            .commands
            .push(CommandDeclaration::server_intent("global.save", "Save"));
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "editor.save".to_string(),
            sequence: vec![g.clone()],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        manifest.keymaps.push(KeyBindingRule {
            command_id: "global.save".to_string(),
            sequence: vec![g.clone(), g.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        assert!(matches!(
            state.route_key_sequence(&[], &g),
            ChordRouteOutcome::Matched(RoutedBehavior::ServerIntent(ServerIntentRoute {
                command_id,
                ..
            })) if command_id == "editor.save"
        ));
    }

    #[test]
    fn route_key_sequence_keeps_editor_context_precedence_for_prefixes() {
        // A pending continuation in EditorTextFocus wins over an exact
        // Global match for the same candidate strokes.
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest
            .commands
            .push(CommandDeclaration::server_intent("editor.save", "Save"));
        manifest
            .commands
            .push(CommandDeclaration::server_intent("global.save", "Save"));
        let g = KeyStroke::new(KeyCode::Character("g".to_string()));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "editor.save".to_string(),
            sequence: vec![g.clone(), g.clone(), g.clone()],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        manifest.keymaps.push(KeyBindingRule {
            command_id: "global.save".to_string(),
            sequence: vec![g.clone(), g.clone()],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        // After "g", the next "g" extends the EditorTextFocus prefix instead
        // of dispatching Global's exact "g g" rule.
        assert_eq!(
            state.route_key_sequence(std::slice::from_ref(&g), &g),
            ChordRouteOutcome::Pending
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
    fn client_routes_control_center_open_default_binding_as_server_intent() {
        // Phase 24.5: the default manifest's Global `Ctrl+X Ctrl+P` chord
        // dispatches through the server-intent lane (no hard-coded chord in
        // widget event handling); the first stroke is pending, the second
        // dispatches.
        let state = ClientBehaviorState::new(BehaviorManifest::minimal_text_editing(1)).unwrap();
        let ctrl_x = KeyStroke {
            key: KeyCode::Character("x".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        };
        let ctrl_p = KeyStroke {
            key: KeyCode::Character("p".to_string()),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::NONE
            },
        };

        assert_eq!(
            state.route_key_sequence(&[], &ctrl_x),
            ChordRouteOutcome::Pending
        );
        assert_eq!(
            state.route_key_sequence(&[ctrl_x], &ctrl_p),
            ChordRouteOutcome::Matched(RoutedBehavior::ServerIntent(ServerIntentRoute {
                command_id: "controlCenter.open".to_string(),
                routing_policy: RoutingPolicy::ServerFirst,
            }))
        );
    }

    #[test]
    fn editor_text_focus_rule_wins_over_global_for_same_chord() {
        // Precedence: an EditorTextFocus rule shadows a Global rule for the
        // same chord (more specific context first).
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest
            .commands
            .push(CommandDeclaration::client_ui("editor.clientUndo", "Undo"));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "editor.clientUndo".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("p".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientUiCommand,
        });
        let state = ClientBehaviorState::new(manifest).unwrap();

        let routed = state.route_key(&KeyStroke {
            key: KeyCode::Character("p".to_string()),
            modifiers: KeyModifiers {
                control: true,
                shift: true,
                ..KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            RoutedBehavior::ClientUiCommand(ClientUiCommandRoute {
                command_id: "editor.clientUndo".to_string(),
                routing_policy: RoutingPolicy::ClientUiCommand,
            })
        );
    }

    #[test]
    fn client_routes_open_file_dialog_as_client_ui_intent() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::client_ui(
            "documents.clientOpenFileDialog",
            "Open File Dialog",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "documents.clientOpenFileDialog".to_string(),
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
                command_id: "documents.clientOpenFileDialog".to_string(),
                routing_policy: RoutingPolicy::ClientUiCommand,
            })
        );
    }

    #[test]
    fn shifted_character_key_binding_matches_lowercase_manifest_rule() {
        let mut manifest = BehaviorManifest::minimal_text_editing(1);
        manifest.commands.push(CommandDeclaration::client_ui(
            "workspace.clientOpenFolderDialog",
            "Open Folder Dialog",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.clientOpenFolderDialog".to_string(),
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
                command_id: "workspace.clientOpenFolderDialog".to_string(),
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
            "workspace.clientOpenFolderDialog",
            "Open Folder Dialog",
        ));
        manifest.keymaps.push(KeyBindingRule {
            command_id: "workspace.clientOpenFolderDialog".to_string(),
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
                command_id: "workspace.clientOpenFolderDialog".to_string(),
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
