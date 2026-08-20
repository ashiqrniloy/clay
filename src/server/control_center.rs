//! Control Center command workflow.
//!
//! The Control Center is a built-in transient menu that lists registered
//! commands, filters them by query, and produces inert activation actions.
//! It is not a bespoke command-palette dispatcher: it reuses
//! `TransientMenuSession`, the generation-stamped command catalogue snapshot,
//! and the existing server/shell execution paths (Phase 24.2).

use crate::{
    packages::commands::{CommandCatalogue, RegisteredCommand},
    protocol::{KeyCode, KeyStroke, RoutingPolicy},
    server::command_execution::{
        CommandExecutionDiagnostic, CommandExecutionRequest, CommandExecutionRule,
        CommandExecutionTarget,
    },
    shell::{
        fuzzy::fuzzy_score_fields,
        transient_menu::{
            TransientMenuAction, TransientMenuItem, TransientMenuItemProvenance,
            TransientMenuOrigin, TransientMenuSession, TransientMenuSessionId,
        },
    },
};

/// Typed result of activating one Control Center item (Phase 24.2).
/// Server/package commands produce a `CommandExecutionRequest` routed through
/// the same dispatcher as keybindings/SDUI; shell `ClientUiCommand` items
/// produce the narrow server-approved shell command id the client re-parses
/// deny-by-default. No generic arbitrary client-command channel exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ServerMenuActivation {
    Command(CommandExecutionRequest),
    ShellClientCommand(String),
}

/// Server-owned Control Center state.
///
/// Holds the full unfiltered command list (with routing policy for activation
/// typing) and the current query. The filtered `TransientMenuSession` is
/// produced on demand so Masonry only ever sees the bounded, filtered item
/// list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ControlCenter {
    session_id: TransientMenuSessionId,
    all_items: Vec<(TransientMenuItem, RoutingPolicy)>,
    query: String,
    selected_index: usize,
}

impl ControlCenter {
    /// Opens a Control Center session from one generation-stamped catalogue.
    /// Client-first edit commands stay excluded; shell `ClientUiCommand`
    /// entries stay visible and are activated through the client shell bridge.
    pub(crate) fn open_catalogue(catalogue: &CommandCatalogue, session_id: u64) -> Self {
        let all_items = catalogue
            .commands()
            .iter()
            .filter(|command| {
                is_executable_from_control_center(&command.command_id, &command.routing_policy)
            })
            .map(|command| {
                (
                    command_to_menu_item(command),
                    command.routing_policy.clone(),
                )
            })
            .collect();

        Self {
            session_id: TransientMenuSessionId(session_id),
            all_items,
            query: String::new(),
            selected_index: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn open(
        registry: &crate::packages::commands::CommandRegistry,
        session_id: u64,
    ) -> Self {
        let builtins = crate::server::command_execution::builtin_server_command_ids()
            .iter()
            .filter_map(|command_id| {
                crate::server::command_execution::builtin_server_command(command_id)
            })
            .collect();
        let catalogue = CommandCatalogue::from_sources(
            vec![builtins, registry.snapshot()],
            &crate::protocol::BehaviorManifest::minimal_text_editing(1),
        )
        .expect("test catalogue should fit and have unique IDs");
        Self::open_catalogue(&catalogue, session_id)
    }

    /// Replaces the filter query and returns the filtered session. Filtering
    /// resets the selection to index 0 (the item set changed).
    pub(crate) fn set_query(&mut self, query: impl Into<String>) -> TransientMenuSession {
        self.query = query.into();
        self.selected_index = 0;
        self.session()
    }

    /// Generic semantic Backspace (Phase 24.3): delete the last query
    /// character. The Control Center has no path semantics; path mode
    /// overrides this with ascend-when-filter-empty behavior.
    pub(crate) fn backspace(&mut self) -> TransientMenuSession {
        self.query.pop();
        self.session()
    }

    /// Returns a fresh session reflecting the current query and persisted
    /// selection without mutating stored query state.
    pub(crate) fn session(&self) -> TransientMenuSession {
        let mut filtered: Vec<(usize, i32, &TransientMenuItem)> = self
            .all_items
            .iter()
            .enumerate()
            .filter_map(|(index, (item, _))| {
                query_score(item, &self.query).map(|score| (index, score, item))
            })
            .collect();
        if !self.query.is_empty() {
            filtered.sort_by(|left, right| {
                right
                    .1
                    .cmp(&left.1)
                    .then_with(|| left.2.label.cmp(&right.2.label))
                    .then_with(|| left.2.id.cmp(&right.2.id))
                    .then_with(|| left.0.cmp(&right.0))
            });
        }
        let filtered = filtered
            .into_iter()
            .map(|(_, _, item)| item.clone())
            .collect();
        TransientMenuSession::new(self.session_id, "Control Center")
            .with_items(filtered)
            .with_selected_index(self.selected_index)
            .with_query(&self.query)
            .with_origin(TransientMenuOrigin::Centered)
    }

    /// Moves the persisted selection by `delta` (relative steps, wrapping per
    /// `TransientMenuSession::select_next` semantics) and returns the session.
    pub(crate) fn move_selection(&mut self, delta: i64) -> TransientMenuSession {
        let mut session = self.session();
        let len = session.items().len();
        if len > 0 {
            let steps = delta.rem_euclid(len as i64) as usize;
            for _ in 0..steps {
                session.select_next();
            }
            self.selected_index = session.selected_index();
        }
        session
    }

    /// Produces the typed activation for the currently selected item: a
    /// `CommandExecutionRequest` for server/package commands (dispatched by
    /// the connection through the shared intent dispatcher) or the narrow
    /// shell command id for `ClientUiCommand` items (re-parsed by the client
    /// deny-by-default). Nothing executes here; the caller owns the session
    /// close and the response ordering.
    pub(crate) fn selected_activation(
        &self,
        target: CommandExecutionTarget,
    ) -> Result<ServerMenuActivation, CommandExecutionDiagnostic> {
        let session = self.session();
        let action = session
            .activate_selected()
            .ok_or_else(|| CommandExecutionDiagnostic {
                command_id: String::new(),
                rule: CommandExecutionRule::UnknownCommand,
                message: "no command selected in Control Center".to_string(),
            })?;
        let routing = self
            .all_items
            .iter()
            .find(|(item, _)| item.id == action.command_id)
            .map(|(_, routing)| routing)
            .ok_or_else(|| CommandExecutionDiagnostic {
                command_id: action.command_id.clone(),
                rule: CommandExecutionRule::UnknownCommand,
                message: "selected item is not in the Control Center catalogue".to_string(),
            })?;
        if *routing == RoutingPolicy::ClientUiCommand
            || crate::masonry_editor::EditorClientCommand::from_command_id(&action.command_id)
                .is_some()
        {
            return Ok(ServerMenuActivation::ShellClientCommand(
                action.command_id.clone(),
            ));
        }
        Ok(ServerMenuActivation::Command(CommandExecutionRequest {
            command_id: action.command_id.clone(),
            arguments: action.arguments.clone(),
            target,
            provenance: None,
            expected_permissions: Vec::new(),
        }))
    }
}

fn is_executable_from_control_center(command_id: &str, routing_policy: &RoutingPolicy) -> bool {
    crate::masonry_editor::EditorClientCommand::from_command_id(command_id).is_some()
        || !matches!(
            routing_policy,
            RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck
        )
}

fn command_to_menu_item(command: &RegisteredCommand) -> TransientMenuItem {
    let provenance = if command.package_name == "clay" {
        TransientMenuItemProvenance::BuiltIn
    } else {
        TransientMenuItemProvenance::Package {
            name: command.package_name.clone(),
            version: command.package_version.clone(),
        }
    };

    let binding_summary = format_key_bindings(&command.key_bindings);
    let detail = if binding_summary.is_empty() {
        format!(
            "{} — {}",
            routing_label(&command.routing_policy),
            provenance_label(&provenance)
        )
    } else {
        format!(
            "{} — {} — {}",
            binding_summary,
            routing_label(&command.routing_policy),
            provenance_label(&provenance)
        )
    };

    let accessibility_label = format!("{} {}", command.display_name, provenance_label(&provenance));

    TransientMenuItem::new(
        &command.command_id,
        &command.display_name,
        TransientMenuAction::new(&command.command_id),
    )
    .with_detail(&detail)
    .with_accessibility_label(&accessibility_label)
    .with_provenance(provenance)
}

fn query_score(item: &TransientMenuItem, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    fuzzy_score_fields(
        query,
        [
            item.label.as_str(),
            item.id.as_str(),
            item.detail.as_deref().unwrap_or_default(),
            item.accessibility_label.as_str(),
        ],
    )
}

fn routing_label(routing_policy: &RoutingPolicy) -> &'static str {
    match routing_policy {
        RoutingPolicy::ServerFirst => "server-first",
        RoutingPolicy::ServerFirstWithLock { .. } => "server-first-with-lock",
        RoutingPolicy::UiReactivePriority => "ui-reactive",
        RoutingPolicy::Background => "background",
        RoutingPolicy::ClientFirstPredictable
        | RoutingPolicy::ClientFirstRequiresAck
        | RoutingPolicy::ClientUiCommand => "client",
    }
}

fn provenance_label(provenance: &TransientMenuItemProvenance) -> String {
    match provenance {
        TransientMenuItemProvenance::BuiltIn => "built-in".to_string(),
        TransientMenuItemProvenance::Package { name, version } => {
            format!("{name}@{version}")
        }
    }
}

fn format_key_bindings(bindings: &[crate::protocol::KeyBindingRule]) -> String {
    if bindings.is_empty() {
        return String::new();
    }
    bindings
        .iter()
        .map(|binding| {
            binding
                .sequence
                .iter()
                .map(format_keystroke)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_keystroke(stroke: &KeyStroke) -> String {
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
        parts.push("Cmd".to_string());
    }
    parts.push(format_key(&stroke.key));
    parts.join("+")
}

fn format_key(key: &KeyCode) -> String {
    match key {
        KeyCode::Character(c) => c.to_ascii_uppercase(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Escape => "Esc".to_string(),
        KeyCode::ArrowUp => "↑".to_string(),
        KeyCode::ArrowDown => "↓".to_string(),
        KeyCode::ArrowLeft => "←".to_string(),
        KeyCode::ArrowRight => "→".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::{
        packages::{
            commands::{CommandRegistry, PackageCommandDeclaration, RegisteredCommand},
            manifest::validate_manifest_value,
            permissions::PackagePermission,
        },
        protocol::{KeyBindingContext, KeyBindingRule, KeyCode, KeyStroke, RoutingPolicy},
        server::command_execution::{CommandExecutionRule, CommandExecutionTarget},
    };

    fn package_manifest() -> crate::packages::manifest::ClayPackageManifest {
        validate_manifest_value(&json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "markdown",
                "permissions": ["command-registration", "parse-document"],
                "modes": ["markdown"],
                "entry": "./dist/index.js"
            }
        }))
        .expect("valid package manifest")
    }

    fn register_command(
        registry: &mut CommandRegistry,
        command_id: &str,
        display_name: &str,
        routing_policy: RoutingPolicy,
        permissions: Vec<PackagePermission>,
        key_bindings: Vec<KeyBindingRule>,
    ) -> RegisteredCommand {
        let manifest = package_manifest();
        registry
            .register_command(
                &manifest,
                PackageCommandDeclaration {
                    package_name: "@clay/markdown".to_string(),
                    package_version: "0.1.0".to_string(),
                    api_prefix: "markdown".to_string(),
                    command_id: command_id.to_string(),
                    display_name: display_name.to_string(),
                    routing_policy,
                    key_bindings,
                    custom_properties: BTreeMap::new(),
                    permissions,
                },
            )
            .expect("register command")
    }

    #[test]
    fn opening_control_center_lists_all_executable_commands() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            vec![KeyBindingRule::single(
                "markdown.togglePreview",
                KeyCode::Character("p".to_string()),
            )],
        );
        register_command(
            &mut registry,
            "markdown.refreshPreview",
            "Refresh Preview",
            RoutingPolicy::Background,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );
        register_command(
            &mut registry,
            "markdown.uiReactive",
            "UI Reactive",
            RoutingPolicy::UiReactivePriority,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );

        let center = ControlCenter::open(&registry, 1);
        let session = center.session();

        let ids: Vec<_> = session
            .items()
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert!(ids.contains(&"markdown.refreshPreview"));
        assert!(ids.contains(&"markdown.togglePreview"));
        assert!(ids.contains(&"markdown.uiReactive"));
    }

    #[test]
    fn control_center_includes_built_in_commands() {
        let registry = CommandRegistry::new();
        let center = ControlCenter::open(&registry, 2);
        let session = center.session();

        let ids: Vec<_> = session
            .items()
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert!(ids.contains(&"controlCenter.open"));
        assert!(ids.contains(&"runtime.reloadConfiguration"));
        assert!(ids.contains(&"workspace.refresh"));

        let reload = session
            .items()
            .iter()
            .find(|item| item.id == "runtime.reloadConfiguration")
            .expect("reload command is listed");
        assert!(
            reload
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("Ctrl+Shift+R")),
            "Control Center should show the default reload chord"
        );
    }

    #[test]
    fn filtering_matches_label_id_binding_and_provenance() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            vec![KeyBindingRule::single(
                "markdown.togglePreview",
                KeyCode::Character("p".to_string()),
            )],
        );
        register_command(
            &mut registry,
            "markdown.refreshPreview",
            "Refresh Preview",
            RoutingPolicy::Background,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );

        let mut center = ControlCenter::open(&registry, 3);
        let session = center.set_query("togglePreview");
        assert_eq!(session.items().len(), 1);
        assert_eq!(session.items()[0].id, "markdown.togglePreview");

        let session = center.set_query("Toggle Preview");
        assert_eq!(session.items().len(), 1);
        assert_eq!(session.items()[0].id, "markdown.togglePreview");

        let session = center.set_query("markdown");
        assert_eq!(session.items().len(), 2);

        let session = center.set_query("Refresh Preview");
        assert_eq!(session.items().len(), 1);
        assert_eq!(session.items()[0].id, "markdown.refreshPreview");

        let session = center.set_query("@clay/markdown");
        assert_eq!(session.items().len(), 2);
    }

    #[test]
    fn selected_command_produces_command_activation() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );

        let mut center = ControlCenter::open(&registry, 4);
        let session = center.set_query("togglePreview");
        assert_eq!(session.selected_index(), 0);

        let activation = center
            .selected_activation(CommandExecutionTarget::ActiveDocument { document_id: 1 })
            .expect("selected command activation");
        let ServerMenuActivation::Command(request) = activation else {
            panic!("expected command activation")
        };
        assert_eq!(request.command_id, "markdown.togglePreview");
        assert_eq!(
            request.target,
            CommandExecutionTarget::ActiveDocument { document_id: 1 }
        );
    }

    #[test]
    fn empty_filtered_session_rejects_activation() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );
        let mut center = ControlCenter::open(&registry, 5);
        center.set_query("zzzz-no-match");

        let error = center
            .selected_activation(CommandExecutionTarget::Global)
            .expect_err("no selected command");

        assert!(matches!(error.rule, CommandExecutionRule::UnknownCommand));
    }

    #[test]
    fn selected_shell_client_item_produces_shell_activation() {
        let shell_commands = crate::masonry_shell::SHELL_CLIENT_COMMAND_CATALOGUE
            .iter()
            .map(|(command_id, display_name)| RegisteredCommand {
                package_name: "clay".to_string(),
                package_version: env!("CARGO_PKG_VERSION").to_string(),
                api_prefix: "shell".to_string(),
                command_id: (*command_id).to_string(),
                display_name: (*display_name).to_string(),
                routing_policy: RoutingPolicy::ClientUiCommand,
                key_bindings: Vec::new(),
                custom_properties: BTreeMap::new(),
                permissions: Vec::new(),
            })
            .collect();
        let catalogue = CommandCatalogue::from_sources(
            vec![shell_commands],
            &crate::protocol::BehaviorManifest::minimal_text_editing(1),
        )
        .expect("shell catalogue should validate");
        let mut center = ControlCenter::open_catalogue(&catalogue, 8);

        let session = center.set_query("clientSplitPaneVertical");
        assert_eq!(session.items().len(), 1);

        let activation = center
            .selected_activation(CommandExecutionTarget::Global)
            .expect("selected shell item activation");
        assert_eq!(
            activation,
            ServerMenuActivation::ShellClientCommand("shell.clientSplitPaneVertical".to_string())
        );
    }

    #[test]
    fn client_first_command_is_not_executable_from_control_center() {
        // Package registration already rejects client-first and client-ui
        // routing policies. Client-first remains hidden; shell client-ui
        // entries are intentionally visible for task 6's activation bridge.
        let command = RegisteredCommand {
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            command_id: "markdown.clientEdit".to_string(),
            display_name: "Client Edit".to_string(),
            routing_policy: RoutingPolicy::ClientFirstPredictable,
            key_bindings: Vec::new(),
            custom_properties: std::collections::BTreeMap::new(),
            permissions: vec![PackagePermission::ParseDocument],
        };
        assert!(!is_executable_from_control_center(
            &command.command_id,
            &command.routing_policy
        ));

        let command = RegisteredCommand {
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            command_id: "markdown.clientUi".to_string(),
            display_name: "Client UI".to_string(),
            routing_policy: RoutingPolicy::ClientUiCommand,
            key_bindings: Vec::new(),
            custom_properties: std::collections::BTreeMap::new(),
            permissions: vec![PackagePermission::ParseDocument],
        };
        assert!(is_executable_from_control_center(
            &command.command_id,
            &command.routing_policy
        ));
    }

    #[test]
    fn shell_client_catalogue_entries_are_visible_and_parser_allowlisted() {
        let shell_commands = crate::masonry_shell::SHELL_CLIENT_COMMAND_CATALOGUE
            .iter()
            .map(|(command_id, display_name)| RegisteredCommand {
                package_name: "clay".to_string(),
                package_version: env!("CARGO_PKG_VERSION").to_string(),
                api_prefix: "shell".to_string(),
                command_id: (*command_id).to_string(),
                display_name: (*display_name).to_string(),
                routing_policy: RoutingPolicy::ClientUiCommand,
                key_bindings: Vec::new(),
                custom_properties: BTreeMap::new(),
                permissions: Vec::new(),
            })
            .collect();
        let catalogue = CommandCatalogue::from_sources(
            vec![shell_commands],
            &crate::protocol::BehaviorManifest::minimal_text_editing(1),
        )
        .expect("shell catalogue should fit");
        let center = ControlCenter::open_catalogue(&catalogue, 8);
        let session = center.session();
        let ids: std::collections::HashSet<_> = session
            .items()
            .iter()
            .map(|item| item.id.as_str())
            .collect();

        assert_eq!(
            ids.len(),
            crate::masonry_shell::SHELL_CLIENT_COMMAND_CATALOGUE.len()
        );
        for (command_id, _) in crate::masonry_shell::SHELL_CLIENT_COMMAND_CATALOGUE {
            assert!(ids.contains(command_id));
            assert!(
                crate::masonry_shell::ShellClientCommand::from_command_id(command_id).is_some()
            );
        }
    }

    #[test]
    fn catalogue_snapshot_is_not_rebuilt_for_query_updates() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );
        let mut center = ControlCenter::open(&registry, 9);
        registry.insert_test_command(RegisteredCommand {
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            command_id: "markdown.addedAfterOpen".to_string(),
            display_name: "Added After Open".to_string(),
            routing_policy: RoutingPolicy::ServerFirst,
            key_bindings: Vec::new(),
            custom_properties: BTreeMap::new(),
            permissions: vec![PackagePermission::ParseDocument],
        });

        assert!(center.set_query("addedAfterOpen").items().is_empty());
    }

    #[test]
    fn fuzzy_query_matches_subsequence_in_command_label() {
        let command = RegisteredCommand {
            package_name: "clay".to_string(),
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            api_prefix: "clay".to_string(),
            command_id: "controlCenter.open".to_string(),
            display_name: "Control Center Open".to_string(),
            routing_policy: RoutingPolicy::ServerFirst,
            key_bindings: Vec::new(),
            custom_properties: BTreeMap::new(),
            permissions: Vec::new(),
        };
        let catalogue = CommandCatalogue::from_sources(
            vec![vec![command]],
            &crate::protocol::BehaviorManifest::minimal_text_editing(1),
        )
        .expect("fuzzy test catalogue should validate");
        let mut center = ControlCenter::open_catalogue(&catalogue, 10);

        let session = center.set_query("ccop");

        assert_eq!(
            session.items().first().map(|item| item.label.as_str()),
            Some("Control Center Open")
        );
    }

    #[test]
    fn fuzzy_ties_follow_catalogue_label_and_id_order() {
        let command = |command_id: &str| RegisteredCommand {
            package_name: "clay".to_string(),
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            api_prefix: "clay".to_string(),
            command_id: command_id.to_string(),
            display_name: "Same Label".to_string(),
            routing_policy: RoutingPolicy::ServerFirst,
            key_bindings: Vec::new(),
            custom_properties: BTreeMap::new(),
            permissions: Vec::new(),
        };
        let catalogue = CommandCatalogue::from_sources(
            vec![vec![command("test.b"), command("test.a")]],
            &crate::protocol::BehaviorManifest::minimal_text_editing(1),
        )
        .expect("tie test catalogue should validate");
        let mut center = ControlCenter::open_catalogue(&catalogue, 11);

        let session = center.set_query("same");
        let ids: Vec<_> = session
            .items()
            .iter()
            .map(|item| item.id.as_str())
            .collect();

        assert_eq!(ids, ["test.a", "test.b"]);
    }

    #[test]
    fn item_detail_includes_key_binding_and_provenance() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            vec![KeyBindingRule {
                command_id: "markdown.togglePreview".to_string(),
                sequence: vec![KeyStroke::new(KeyCode::Character("p".to_string()))],
                context: KeyBindingContext::EditorTextFocus,
                routing_policy: RoutingPolicy::ServerFirst,
            }],
        );

        let center = ControlCenter::open(&registry, 7);
        let session = center.session();
        let item = session
            .items()
            .iter()
            .find(|item| item.id == "markdown.togglePreview")
            .expect("toggle preview item");

        assert!(
            item.detail
                .as_ref()
                .unwrap()
                .to_ascii_lowercase()
                .contains("p")
        );
        assert!(item.detail.as_ref().unwrap().contains("server-first"));
        assert!(item.detail.as_ref().unwrap().contains("@clay/markdown"));
    }
}
