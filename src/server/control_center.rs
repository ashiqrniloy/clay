//! Control Center command workflow.
//!
//! The Control Center is a built-in transient menu that lists registered
//! commands, filters them by query, and produces inert activation actions that
//! route through the server-owned `CommandExecutor`. It is not a bespoke
//! command-palette dispatcher: it reuses `TransientMenuSession`, the command
//! registry snapshot, and the existing command execution path.

#![allow(dead_code)]

use crate::{
    packages::commands::{CommandRegistry, RegisteredCommand},
    protocol::{KeyCode, KeyStroke, RoutingPolicy},
    server::command_execution::{
        CommandExecutionRequest, CommandExecutionResult, CommandExecutionTarget, CommandExecutor,
        builtin_server_command_ids,
    },
    shell::transient_menu::{
        TransientMenuAction, TransientMenuItem, TransientMenuItemProvenance, TransientMenuSession,
        TransientMenuSessionId,
    },
};

/// Server-owned Control Center state.
///
/// Holds the full unfiltered command list and the current query. The filtered
/// `TransientMenuSession` is produced on demand so Masonry only ever sees the
/// bounded, filtered item list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ControlCenter {
    session_id: TransientMenuSessionId,
    all_items: Vec<TransientMenuItem>,
    query: String,
}

impl ControlCenter {
    /// Opens a Control Center session from the current command registry.
    ///
    /// Only commands with server-owned or background routing policies are
    /// exposed. Client-first and native-client-UI commands are not executable
    /// from the Control Center because they require client-side edit authority
    /// or native widget coordination.
    pub(crate) fn open(registry: &CommandRegistry, session_id: u64) -> Self {
        let mut all_items: Vec<TransientMenuItem> = registry
            .list()
            .filter(|command| is_executable_from_control_center(&command.routing_policy))
            .map(command_to_menu_item)
            .collect();

        for command_id in builtin_server_command_ids() {
            if registry.get(command_id).is_none()
                && let Some(command) =
                    crate::server::command_execution::builtin_server_command(command_id)
            {
                all_items.push(command_to_menu_item(&command));
            }
        }

        all_items.sort_by(|a, b| a.label.cmp(&b.label));

        Self {
            session_id: TransientMenuSessionId(session_id),
            all_items,
            query: String::new(),
        }
    }

    pub(crate) fn session_id(&self) -> TransientMenuSessionId {
        self.session_id
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    /// Replaces the filter query and returns the filtered session.
    pub(crate) fn set_query(&mut self, query: impl Into<String>) -> TransientMenuSession {
        self.query = query.into();
        let filtered: Vec<TransientMenuItem> = self
            .all_items
            .iter()
            .filter(|item| matches_query(item, &self.query))
            .cloned()
            .collect();
        TransientMenuSession::new(self.session_id, "Control Center").with_items(filtered)
    }

    /// Returns a fresh session reflecting the current query without mutating
    /// stored query state. Useful for tests and idempotent renders.
    pub(crate) fn session(&self) -> TransientMenuSession {
        let filtered: Vec<TransientMenuItem> = self
            .all_items
            .iter()
            .filter(|item| matches_query(item, &self.query))
            .cloned()
            .collect();
        TransientMenuSession::new(self.session_id, "Control Center").with_items(filtered)
    }

    /// Executes the currently selected item from the filtered session through
    /// the shared command executor.
    pub(crate) fn execute_selected(
        &self,
        executor: &CommandExecutor,
        registry: &CommandRegistry,
        target: CommandExecutionTarget,
    ) -> Result<CommandExecutionResult, crate::server::command_execution::CommandExecutionDiagnostic>
    {
        let session = self.session();
        let action = session.activate_selected().ok_or_else(|| {
            crate::server::command_execution::CommandExecutionDiagnostic {
                command_id: String::new(),
                rule: crate::server::command_execution::CommandExecutionRule::UnknownCommand,
                message: "no command selected in Control Center".to_string(),
            }
        })?;
        executor.execute(
            registry,
            CommandExecutionRequest {
                command_id: action.command_id.clone(),
                arguments: action.arguments.clone(),
                target,
                provenance: None,
                expected_permissions: Vec::new(),
            },
        )
    }
}

fn is_executable_from_control_center(routing_policy: &RoutingPolicy) -> bool {
    !matches!(
        routing_policy,
        RoutingPolicy::ClientFirstPredictable
            | RoutingPolicy::ClientFirstRequiresAck
            | RoutingPolicy::ClientUiCommand
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

fn matches_query(item: &TransientMenuItem, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    item.label.to_lowercase().contains(&query)
        || item.id.to_lowercase().contains(&query)
        || item
            .detail
            .as_ref()
            .map(|d| d.to_lowercase().contains(&query))
            .unwrap_or(false)
        || item.accessibility_label.to_lowercase().contains(&query)
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
            commands::{PackageCommandDeclaration, RegisteredCommand},
            manifest::validate_manifest_value,
            permissions::PackagePermission,
        },
        protocol::{KeyBindingContext, KeyBindingRule, KeyCode, KeyStroke, RoutingPolicy},
        server::command_execution::{
            CommandExecutionRule, CommandExecutionStatus, CommandExecutionTarget,
        },
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
    fn selected_command_executes_through_command_executor() {
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

        let result = center
            .execute_selected(
                &CommandExecutor::new(),
                &registry,
                CommandExecutionTarget::ActiveDocument { document_id: 1 },
            )
            .expect("execute selected command");

        assert_eq!(result.command_id, "markdown.togglePreview");
        assert_eq!(result.status, CommandExecutionStatus::Accepted);
    }

    #[test]
    fn empty_filtered_session_rejects_execution() {
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
            .execute_selected(
                &CommandExecutor::new(),
                &registry,
                CommandExecutionTarget::Global,
            )
            .expect_err("no selected command");

        assert!(matches!(error.rule, CommandExecutionRule::UnknownCommand));
    }

    #[test]
    fn client_first_command_is_not_executable_from_control_center() {
        // Package registration already rejects client-first and client-ui
        // routing policies, so this test exercises the filtering defense
        // directly with raw RegisteredCommand values.
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
        assert!(!is_executable_from_control_center(&command.routing_policy));

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
        assert!(!is_executable_from_control_center(&command.routing_policy));
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
