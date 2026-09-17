//! Control Center command workflow — the composer's `/` palette.
//!
//! The Control Center is a built-in transient menu that lists registered
//! commands, filters them by query, and produces inert activation actions.
//! It is not a bespoke command-palette dispatcher: it reuses
//! `TransientMenuSession`, the generation-stamped command catalogue snapshot,
//! and the existing server/shell execution paths (Phase 24.2).
//!
//! Plan 124 task 7: since the Control Centre is the agent lane's `/` palette,
//! the session declares the bottom-anchored `CommandPalette` origin — the
//! client draws it as the composer's own menu, 6px above the box (DESIGN.md
//! §12) — and names itself `Commands`, the artifact's accessible name. The
//! session's query is the *filter* the field holds after the `/` sigil: the
//! field is the input, so opening the palette shows the whole catalogue and
//! every later keystroke arrives as a `MenuQueryUpdate`. One session holds the
//! whole unfiltered catalogue for its lifetime, so a keystroke re-filters and
//! never rebuilds it.

use crate::{
    packages::commands::{CommandCatalogue, RegisteredCommand},
    protocol::{KeyCode, KeyStroke, RoutingPolicy},
    server::command_execution::{
        CommandExecutionDiagnostic, CommandExecutionRequest, CommandExecutionRule,
        CommandExecutionTarget, OPEN_PATH_BROWSER_COMMAND_ID,
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

/// The palette's **closed scope vocabulary** (DESIGN.md §12's chips): the
/// server owns the words, the client renders a chip per word it is handed and
/// sends the word back untouched. `session` is the agent package's own
/// commands, `shell` the app's (chrome, panes, tabs, editor, workspace),
/// `files` the palette's path mode. A command outside the three shows under
/// `All` only — never under a guessed scope.
const SCOPE_SESSION: &str = "session";
const SCOPE_SHELL: &str = "shell";
const SCOPE_FILES: &str = "files";
const PALETTE_SCOPES: [&str; 3] = [SCOPE_SESSION, SCOPE_SHELL, SCOPE_FILES];

/// The package that owns the agent session's commands (`/compact`, `/new`, …);
/// its declarations are what the palette groups under `Session`.
const AGENT_PACKAGE_NAME: &str = "@clay/coding-agent";

/// Server-owned Control Center state.
///
/// Holds the full unfiltered command list (with routing policy for activation
/// typing), the current query, and the palette's scope chip. The filtered
/// `TransientMenuSession` is produced on demand so Masonry only ever sees the
/// bounded, filtered item list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ControlCenter {
    session_id: TransientMenuSessionId,
    all_items: Vec<(TransientMenuItem, RoutingPolicy)>,
    query: String,
    /// `None` = the `All` chip. A known [`PALETTE_SCOPES`] word else.
    scope: Option<&'static str>,
    selected_index: usize,
}

impl ControlCenter {
    /// Opens a palette session from one generation-stamped catalogue.
    /// Client-first edit commands stay excluded; shell `ClientUiCommand`
    /// entries stay visible and are activated through the client shell bridge.
    /// The query starts empty: the lane's field owns it and every keystroke
    /// arrives as a `MenuQueryUpdate` against this same session.
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
            scope: None,
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

    /// Replaces the filter query and returns the filtered session. A
    /// genuinely changed filter resets the selection to index 0 (the item
    /// set changed); an unchanged query keeps it — the webview flushes the
    /// same draft on Enter before activating, and that flush must not
    /// clobber the arrow-selected item.
    pub(crate) fn set_query(&mut self, query: impl Into<String>) -> TransientMenuSession {
        let query = query.into();
        if query != self.query {
            self.selected_index = 0;
        }
        self.query = query;
        self.session()
    }

    /// Plan 124: selects a scope chip (`All` = `None`) and returns the filtered
    /// session. Unknown words fall back to `All`: the vocabulary is closed, so
    /// a client cannot invent a scope, and none of them carries authority.
    /// A genuinely changed scope resets the selection to index 0 for the same
    /// reason a changed query does — the item set changed.
    pub(crate) fn set_scope(&mut self, scope: Option<&str>) -> TransientMenuSession {
        let scope = scope.and_then(known_scope);
        if scope != self.scope {
            self.selected_index = 0;
        }
        self.scope = scope;
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
            .filter(|(_, (item, _))| self.in_scope(item))
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
        TransientMenuSession::new(self.session_id, "Commands")
            .with_items(filtered)
            .with_selected_index(self.selected_index)
            .with_query(&self.query)
            // Plan 124 task 7: the composer's palette, not a window sheet. The
            // Bottom anchor is the shipped plumbing (`PackageOverlayAnchor::
            // Bottom`); the client anchors it to the lane's composer box.
            .with_origin(TransientMenuOrigin::CommandPalette)
    }

    /// The `All` chip (`scope == None`) shows every row; a scope chip shows the
    /// rows tagged with it, so an untagged package command is `All`-only.
    fn in_scope(&self, item: &TransientMenuItem) -> bool {
        match self.scope {
            None => true,
            Some(scope) => item.scope.as_deref() == Some(scope),
        }
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
            || crate::client_commands::EditorClientCommand::from_command_id(&action.command_id)
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

/// The closed vocabulary's membership test: a word outside it is not a scope.
fn known_scope(scope: &str) -> Option<&'static str> {
    PALETTE_SCOPES.iter().copied().find(|known| *known == scope)
}

fn is_executable_from_control_center(command_id: &str, routing_policy: &RoutingPolicy) -> bool {
    crate::client_commands::EditorClientCommand::from_command_id(command_id).is_some()
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

    // Plan 124: the chords and the scope are item fields now, so the detail
    // line stops restating the chords — the row's chips own them, and the
    // palette finds a command by chord because `query_score` reads `bindings`.
    let detail = format!(
        "{} — {}",
        routing_label(&command.routing_policy),
        provenance_label(&provenance)
    );

    let accessibility_label = format!("{} {}", command.display_name, provenance_label(&provenance));

    let mut item = TransientMenuItem::new(
        &command.command_id,
        &command.display_name,
        TransientMenuAction::new(&command.command_id),
    )
    .with_detail(&detail)
    .with_accessibility_label(&accessibility_label)
    .with_provenance(provenance)
    .with_bindings(binding_chords(&command.key_bindings));
    if let Some(scope) = command_scope(command) {
        item = item.with_scope(scope);
    }
    item
}

/// The closed scope a command belongs to (DESIGN.md §12's chips). Checked in
/// precedence order: the palette's own path mode is `files` even though the
/// built-in is otherwise a shell command, and the client-UI spellings are
/// `shell` whatever package declares them.
fn command_scope(command: &RegisteredCommand) -> Option<&'static str> {
    if command.command_id == OPEN_PATH_BROWSER_COMMAND_ID {
        return Some(SCOPE_FILES);
    }
    if crate::client_commands::ShellClientCommand::from_command_id(&command.command_id).is_some()
        || crate::client_commands::EditorClientCommand::from_command_id(&command.command_id)
            .is_some()
        || command.package_name == "clay"
    {
        return Some(SCOPE_SHELL);
    }
    if command.package_name == AGENT_PACKAGE_NAME {
        return Some(SCOPE_SESSION);
    }
    None
}

pub(crate) fn score_menu_item(item: &TransientMenuItem, query: &str) -> Option<i32> {
    query_score(item, query)
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
        ]
        .into_iter()
        // A command is findable by its chord (plan 124 moved the chords out of
        // `detail` into `bindings`, so the search follows them).
        .chain(item.bindings.iter().map(String::as_str)),
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

/// One chord string per registered binding, in the app's own spelling
/// (`"Ctrl+X Ctrl+P"`): the palette's per-row chip groups (plan 124).
fn binding_chords(bindings: &[crate::protocol::KeyBindingRule]) -> Vec<String> {
    bindings.iter().map(format_chord).collect()
}

fn format_chord(binding: &crate::protocol::KeyBindingRule) -> String {
    binding
        .sequence
        .iter()
        .map(format_keystroke)
        .collect::<Vec<_>>()
        .join(" ")
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
        assert!(ids.contains(&"agent.clientOpenAgentPicker"));
        assert!(ids.contains(&"agent.clientOpenProviderPicker"));
        assert!(ids.contains(&"agent.clientOpenModelPicker"));
        assert!(ids.contains(&"agent.clientOpenProviderSetup"));
        assert!(ids.contains(&"agent.clientOpenSessionPicker"));

        let reload = session
            .items()
            .iter()
            .find(|item| item.id == "runtime.reloadConfiguration")
            .expect("reload command is listed");
        // Plan 124: the chord is the row's `bindings` (the palette's chips), not
        // text inside the detail line, and a built-in is in the shell scope.
        assert_eq!(reload.bindings, ["Ctrl+Shift+R"]);
        assert_eq!(reload.scope.as_deref(), Some("shell"));
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
    fn unchanged_query_keeps_arrow_selection_flush_before_activate() {
        // The webview flush pattern re-sends the same draft via menuQuery
        // right before menuActivate; that flush must not clobber the
        // arrow-selected item. A genuinely changed query still resets.
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );
        register_command(
            &mut registry,
            "markdown.toggleList",
            "Toggle List",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );

        let mut center = ControlCenter::open(&registry, 4);
        center.set_query("toggle");
        let session = center.move_selection(1);
        assert_eq!(session.selected_index(), 1);
        let session = center.set_query("toggle");
        assert_eq!(
            session.selected_index(),
            1,
            "flush of the unchanged draft must keep the selected item"
        );
        let session = center.set_query("toggleList");
        assert_eq!(session.selected_index(), 0);
    }

    #[test]
    fn palette_session_opens_full_and_keeps_the_catalogue_across_queries() {
        // Plan 124 task 7: the catalogue session is the composer's `/` palette.
        // The lane's field owns the query, so opening shows the whole
        // route-filtered catalogue and every keystroke only re-filters the
        // session's own held list — nothing is rebuilt per query.
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );
        register_command(
            &mut registry,
            "markdown.toggleList",
            "Toggle List",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );

        let mut center = ControlCenter::open(&registry, 9);
        let opened = center.session();
        assert_eq!(opened.origin(), TransientMenuOrigin::CommandPalette);
        assert_eq!(opened.prompt(), "Commands");
        assert_eq!(opened.query(), "");
        let ids = |session: &TransientMenuSession| -> Vec<String> {
            session.items().iter().map(|item| item.id.clone()).collect()
        };
        let all = ids(&opened);
        assert!(all.contains(&"markdown.togglePreview".to_string()));
        assert!(all.contains(&"markdown.toggleList".to_string()));

        let narrowed = center.set_query("toggleList");
        assert_eq!(narrowed.items().len(), 1);
        assert_eq!(narrowed.origin(), TransientMenuOrigin::CommandPalette);

        // Backspace is the palette's own gesture: it deletes one filter
        // character from the same session.
        let shorter = center.backspace();
        assert_eq!(shorter.query(), "toggleLis");
        assert_eq!(shorter.items().len(), 1);

        // Clearing the filter restores the open-time set: one session held the
        // whole catalogue the entire time; no query rebuilt it.
        let cleared = center.set_query("");
        assert_eq!(ids(&cleared), all);
        assert_eq!(cleared.prompt(), "Commands");
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
        let shell_commands = crate::client_commands::SHELL_CLIENT_COMMAND_CATALOGUE
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
        let shell_commands = crate::client_commands::SHELL_CLIENT_COMMAND_CATALOGUE
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
            crate::client_commands::SHELL_CLIENT_COMMAND_CATALOGUE.len()
        );
        for (command_id, _) in crate::client_commands::SHELL_CLIENT_COMMAND_CATALOGUE {
            assert!(ids.contains(command_id));
            assert!(
                crate::client_commands::ShellClientCommand::from_command_id(command_id).is_some()
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
        // Plan 124 task 7: the palette's origin rides the same session, so a
        // query update re-filters the held catalogue and never re-anchors or
        // rebuilds it.
        assert_eq!(
            center.session().origin(),
            TransientMenuOrigin::CommandPalette
        );
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
    fn item_states_its_chords_and_scope_while_the_detail_keeps_provenance() {
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

        assert_eq!(
            item.bindings,
            ["P"],
            "the chord is an item field, not prose"
        );
        let detail = item.detail.as_ref().unwrap();
        assert!(detail.contains("server-first"));
        assert!(detail.contains("@clay/markdown"));
        assert!(
            !detail.contains('P'),
            "the detail line stops restating the chord: {detail}"
        );
        // A package outside the app and the agent has no scope of its own: it
        // shows under `All` only.
        assert_eq!(item.scope, None);
    }

    #[test]
    fn items_carry_the_closed_scope_vocabulary() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );
        let agent_manifest = crate::packages::manifest::validate_manifest_value(&json!({
            "name": "@clay/coding-agent",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "coding-agent",
                "permissions": ["command-registration"],
                "modes": ["coding-agent"],
                "entry": "./dist/index.js"
            }
        }))
        .expect("valid agent manifest");
        registry
            .register_command(
                &agent_manifest,
                PackageCommandDeclaration {
                    package_name: "@clay/coding-agent".to_string(),
                    package_version: "0.1.0".to_string(),
                    api_prefix: "coding-agent".to_string(),
                    command_id: "coding-agent.compact".to_string(),
                    display_name: "/compact".to_string(),
                    routing_policy: RoutingPolicy::ServerFirst,
                    key_bindings: Vec::new(),
                    custom_properties: BTreeMap::new(),
                    permissions: vec![PackagePermission::CommandRegistration],
                },
            )
            .expect("register agent command");

        let session = ControlCenter::open(&registry, 9).session();
        let scope_of = |id: &str| {
            session
                .items()
                .iter()
                .find(|item| item.id == id)
                .unwrap_or_else(|| panic!("{id} is listed"))
                .scope
                .clone()
        };

        // The app's own commands (built-ins and shell client commands) are
        // `shell`; the palette's path mode is `files` even though it is a
        // built-in; the agent package's commands are `session`; a third-party
        // package's command has no scope.
        assert_eq!(scope_of("workspace.refresh").as_deref(), Some("shell"));
        assert_eq!(scope_of("controlCenter.openPath").as_deref(), Some("files"));
        assert_eq!(scope_of("coding-agent.compact").as_deref(), Some("session"));
        assert_eq!(scope_of("markdown.togglePreview"), None);

        // The vocabulary follows the command family, not the declaring package:
        // a shell/editor client-UI spelling is the shell scope wherever it is
        // declared (registration namespaces real packages, so classify a
        // synthetic spelling directly).
        let foreign_shell = RegisteredCommand {
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            command_id: "shell.clientTabNew".to_string(),
            display_name: "New Tab".to_string(),
            routing_policy: RoutingPolicy::ClientUiCommand,
            key_bindings: Vec::new(),
            custom_properties: BTreeMap::new(),
            permissions: Vec::new(),
        };
        assert_eq!(command_scope(&foreign_shell), Some("shell"));
        let editor_command = RegisteredCommand {
            command_id: "editor.clientToggleFold".to_string(),
            ..foreign_shell
        };
        assert_eq!(command_scope(&editor_command), Some("shell"));
    }

    #[test]
    fn scope_chip_narrows_the_catalogue_and_resets_the_selection() {
        let mut registry = CommandRegistry::new();
        register_command(
            &mut registry,
            "markdown.togglePreview",
            "Toggle Preview",
            RoutingPolicy::ServerFirst,
            vec![PackagePermission::ParseDocument],
            Vec::new(),
        );
        let shell_ids = |session: &TransientMenuSession| {
            session
                .items()
                .iter()
                .filter(|item| item.scope.as_deref() == Some("shell"))
                .count()
        };

        let mut center = ControlCenter::open(&registry, 4);
        let all = center.session();
        assert!(all.items().len() > shell_ids(&all));
        center.move_selection(2);
        assert_eq!(center.selected_index, 2);

        let shell = center.set_scope(Some("shell"));
        assert_eq!(shell.items().len(), shell_ids(&shell));
        assert!(
            shell
                .items()
                .iter()
                .all(|item| item.scope.as_deref() == Some("shell")),
            "a scope chip shows only its own rows"
        );
        assert_eq!(
            shell.selected_index(),
            0,
            "a changed scope resets the selection onto the new item set"
        );

        // The path mode is the only `files` row, and `All` restores everything.
        let files = center.set_scope(Some("files"));
        assert_eq!(
            files
                .items()
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["controlCenter.openPath"]
        );
        assert_eq!(center.set_scope(None).items().len(), all.items().len());

        // The vocabulary is closed: an unknown word is not a scope.
        assert_eq!(
            center.set_scope(Some("not-a-scope")).items().len(),
            all.items().len()
        );
    }

    #[test]
    fn a_command_is_findable_by_its_chord() {
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

        // The chord used to be searchable only because the detail line repeated
        // it; it is an item field now, so the filter reads that field.
        let session = ControlCenter::open(&registry, 5).set_query("ctrl+x ctrl+p");
        assert!(session.items().is_empty());
        let session = ControlCenter::open(&registry, 5).set_query("P");
        assert!(
            session
                .items()
                .iter()
                .any(|item| item.id == "markdown.togglePreview"),
            "typing a chord finds the command that owns it"
        );
    }
}
