use serde_json::Value;

use crate::{
    packages::{
        commands::{CommandRegistry, RegisteredCommand},
        modes::ModeRegistry,
        permissions::PackagePermission,
    },
    perf::budgets::COMMAND_ARGUMENT_BUDGET_BYTES,
    protocol::{ClientId, DocumentId, KeyBindingRule, RoutingPolicy, WorkspaceRootId},
    server::{
        git::{GitCachedStatus, GitStatusCache},
        workspace::{WorkspaceError, WorkspaceState},
    },
};

pub use crate::server::workspace::OpenDocumentHead;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandExecutionRequest {
    pub command_id: String,
    pub arguments: Value,
    pub target: CommandExecutionTarget,
    pub provenance: Option<CommandExecutionProvenance>,
    pub expected_permissions: Vec<PackagePermission>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandExecutionProvenance {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandExecutionTarget {
    ActiveDocument { document_id: DocumentId },
    Workspace,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandExecutionResult {
    pub command_id: String,
    pub routing_policy: RoutingPolicy,
    pub target: CommandExecutionTarget,
    pub status: CommandExecutionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandExecutionStatus {
    Accepted,
    /// Phase 18.9 mode-discovery result payload. Built-in discovery commands
    /// (`modes.listActiveModes`, `modes.explainActiveMode`) resolve
    /// through [`CommandExecutor::execute_discovery`] by reading installed
    /// `ModeRegistry` state; the payload carries no execution, document, or
    /// workspace authority.
    Discovery(DiscoveryResult),
    /// Phase 18.12 workspace action result. Produced by
    /// [`CommandExecutor::execute_workspace`] when a built-in file-browser
    /// command opens, reveals, or requests a visibility toggle through
    /// server-authoritative workspace APIs. The connection handler applies the
    /// toggle to its bound tab state and publishes the resulting SDUI snapshot.
    Workspace(WorkspaceActionResult),
    /// Phase 18.13 read-only Git discovery command result, backed by
    /// server-owned workspace roots and the Git status cache.
    Git(GitCommandResult),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitCommandResult {
    Statuses(Vec<GitCachedStatus>),
    Refreshed(Box<GitCachedStatus>),
}

/// Phase 18.12 workspace action result. Carries the outcome of a validated
/// file-browser command that was executed against [`WorkspaceState`]. No raw
/// filesystem paths or client authority are returned; only bounded document
/// metadata already known to the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceActionResult {
    /// A file was opened under a known root or via a selected-file grant.
    Opened(OpenDocumentHead),
    /// A directory navigation request was accepted; the connection handler
    /// will publish a refreshed file-browser SDUI snapshot.
    Navigated {
        root_id: WorkspaceRootId,
        relative_path: std::path::PathBuf,
    },
    /// A reveal request was accepted; the server will update the focused tree
    /// state on the next SDUI snapshot.
    Revealed,
    /// A file-browser visibility toggle was accepted; the bound connection
    /// owns applying it to its per-tab shell state.
    Toggled,
}

/// Phase 18.9 mode-discovery result payload. Read-only registry state; carries
/// no execution/document/workspace authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryResult {
    /// One entry per open document with an active major mode.
    ActiveModes(Vec<crate::packages::modes::ActiveModeSummary>),
    /// Detailed explanation for a single document's active mode (or `None` if
    /// the document has no active mode recorded).
    ModeExplanation(Option<crate::packages::modes::ModeExplanation>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandExecutionDiagnostic {
    pub command_id: String,
    pub rule: CommandExecutionRule,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandExecutionRule {
    UnknownCommand,
    InvalidRoutingPolicy,
    InvalidProvenance,
    UndeclaredPermission,
    InvalidArguments,
    UnauthorizedTarget,
    ReloadInProgress,
    StaleRuntimeGeneration,
}

#[derive(Debug, Default)]
pub struct CommandExecutor;

impl CommandExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(
        &self,
        registry: &CommandRegistry,
        request: CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandExecutionDiagnostic> {
        if let Some(command) = registry.get(&request.command_id) {
            return self.execute_registered(command, request);
        }
        if let Some(command) = builtin_server_command(&request.command_id) {
            return self.execute_registered(&command, request);
        }
        Err(diagnostic(
            &request.command_id,
            CommandExecutionRule::UnknownCommand,
            "command ID is not registered",
        ))
    }

    pub fn execute_registered(
        &self,
        command: &RegisteredCommand,
        request: CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandExecutionDiagnostic> {
        if request.command_id != command.command_id {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "request command ID does not match registered command",
            ));
        }
        validate(command, &request)?;
        if crate::client_commands::EditorClientCommand::from_command_id(&command.command_id)
            .is_some()
        {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "command is client-mapped and not server-executed",
            ));
        }

        Ok(CommandExecutionResult {
            command_id: command.command_id.clone(),
            routing_policy: command.routing_policy.clone(),
            target: request.target,
            status: CommandExecutionStatus::Accepted,
        })
    }

    /// Resolve a built-in Phase 18.9 mode-discovery command against
    /// installed [`ModeRegistry`] state. The command is looked up among the
    /// built-in server commands, validated exactly like any registered command
    /// (server-owned routing, no undeclared permissions, valid arguments and
    /// target), and then resolved by reading already-installed registry state.
    /// Discovery grants no execution, document, or workspace authority: it
    /// never triggers filesystem scans, package evaluation, network, shell,
    /// AI, WASM, raw ops, or client-side JavaScript — it only reads the registry.
    pub fn execute_discovery(
        &self,
        mode_registry: &ModeRegistry,
        request: CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandExecutionDiagnostic> {
        let Some(command) = builtin_server_command(&request.command_id) else {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "command ID is not a built-in server command",
            ));
        };
        if !is_mode_discovery_command(&request.command_id) {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "built-in command is not a mode-discovery command",
            ));
        }
        validate(&command, &request)?;

        let status = match request.command_id.as_str() {
            LIST_ACTIVE_MODES_COMMAND_ID => CommandExecutionStatus::Discovery(
                DiscoveryResult::ActiveModes(mode_registry.list_active_modes()),
            ),
            EXPLAIN_ACTIVE_MODE_COMMAND_ID => {
                let document_id = discovery_document_id(&request.arguments, &request.command_id)?;
                CommandExecutionStatus::Discovery(DiscoveryResult::ModeExplanation(
                    mode_registry.explain_active_mode(document_id),
                ))
            }
            _ => {
                return Err(diagnostic(
                    &request.command_id,
                    CommandExecutionRule::UnknownCommand,
                    "built-in command is not a mode-discovery command",
                ));
            }
        };
        Ok(CommandExecutionResult {
            command_id: command.command_id.clone(),
            routing_policy: command.routing_policy.clone(),
            target: request.target,
            status,
        })
    }

    /// Execute a Phase 18.12 file-browser command against server-authoritative
    /// workspace APIs. The command is validated through the same pipeline as
    /// package and discovery commands, then resolved by calling
    /// [`WorkspaceState`] methods that enforce root boundaries and selected-file
    /// grants. No raw client path is trusted; all paths are re-canonicalized and
    /// re-validated server-side.
    pub(crate) async fn execute_workspace(
        &self,
        registry: &CommandRegistry,
        workspace: &mut WorkspaceState,
        client_id: ClientId,
        request: CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandExecutionDiagnostic> {
        let builtin = builtin_server_command(&request.command_id);
        let command = registry
            .get(&request.command_id)
            .or(builtin.as_ref())
            .ok_or_else(|| {
                diagnostic(
                    &request.command_id,
                    CommandExecutionRule::UnknownCommand,
                    "command ID is not registered",
                )
            })?;
        if !is_workspace_command(&request.command_id) {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "built-in command is not a workspace command",
            ));
        }
        validate(command, &request)?;

        let status = match request.command_id.as_str() {
            OPEN_FILE_COMMAND_ID | OPEN_FUZZY_FILE_COMMAND_ID => {
                execute_open(
                    workspace,
                    client_id,
                    &request.arguments,
                    &request.command_id,
                )
                .await?
            }
            OPEN_DIRECTORY_COMMAND_ID => {
                let (root_id, relative_path) =
                    navigate_directory_arguments(&request.arguments, &request.command_id)?;
                workspace
                    .list_directory(
                        crate::server::workspace::FileListRequest {
                            root_id,
                            relative_path: relative_path.clone(),
                            max_depth: 1,
                            max_entries: 1,
                        },
                        None,
                    )
                    .map_err(|error| workspace_diagnostic(&request.command_id, error))?;
                CommandExecutionStatus::Workspace(WorkspaceActionResult::Navigated {
                    root_id,
                    relative_path,
                })
            }
            REVEAL_IN_TREE_COMMAND_ID => {
                let document_id = reveal_document_id(&request.arguments, &request.command_id)?;
                workspace
                    .document_metadata(document_id, client_id)
                    .await
                    .map_err(|error| workspace_diagnostic(&request.command_id, error))?;
                CommandExecutionStatus::Workspace(WorkspaceActionResult::Revealed)
            }
            TOGGLE_FILE_BROWSER_COMMAND_ID => {
                CommandExecutionStatus::Workspace(WorkspaceActionResult::Toggled)
            }
            _ => {
                return Err(diagnostic(
                    &request.command_id,
                    CommandExecutionRule::UnknownCommand,
                    "built-in command is not a workspace command",
                ));
            }
        };

        Ok(CommandExecutionResult {
            command_id: command.command_id.clone(),
            routing_policy: command.routing_policy.clone(),
            target: request.target,
            status,
        })
    }

    pub(crate) async fn execute_git(
        &self,
        workspace: &WorkspaceState,
        git_status_cache: GitStatusCache,
        request: CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandExecutionDiagnostic> {
        let Some(command) = builtin_server_command(&request.command_id) else {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "command ID is not a built-in Git command",
            ));
        };
        if !is_git_command(&request.command_id) {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "built-in command is not a Git command",
            ));
        }
        validate(&command, &request)?;

        let status = match request.command_id.as_str() {
            LIST_GIT_STATUSES_COMMAND_ID => CommandExecutionStatus::Git(
                GitCommandResult::Statuses(git_status_cache.list_cached(workspace).await),
            ),
            REFRESH_GIT_STATUS_COMMAND_ID => {
                let root_id = git_workspace_root_id(&request.arguments, &request.command_id)?;
                let root = workspace
                    .directory_roots()
                    .into_iter()
                    .find(|root| root.workspace_root_id == root_id)
                    .ok_or_else(|| {
                        diagnostic(
                            &request.command_id,
                            CommandExecutionRule::InvalidArguments,
                            format!("unknown workspace root `{root_id}`"),
                        )
                    })?;
                CommandExecutionStatus::Git(GitCommandResult::Refreshed(Box::new(
                    git_status_cache
                        .refresh_root(root_id, root.canonical_path)
                        .await,
                )))
            }
            _ => {
                return Err(diagnostic(
                    &request.command_id,
                    CommandExecutionRule::UnknownCommand,
                    "built-in command is not a Git command",
                ));
            }
        };

        Ok(CommandExecutionResult {
            command_id: command.command_id.clone(),
            routing_policy: command.routing_policy.clone(),
            target: request.target,
            status,
        })
    }
}

pub(crate) fn is_workspace_command(command_id: &str) -> bool {
    builtin_definition(command_id)
        .is_some_and(|command| command.kind == BuiltinCommandKind::Workspace)
}

pub(crate) fn is_git_command(command_id: &str) -> bool {
    builtin_definition(command_id).is_some_and(|command| command.kind == BuiltinCommandKind::Git)
}

/// Phase 20.6: settings command ids dispatched from the `@clay/settings`
/// package's catalog-composed surface. Live application (persist → reload →
/// runtime-state fanout) is owned by the configuration-precedence task; this
/// executor validates the inert intent values only.
pub(crate) fn is_settings_command(command_id: &str) -> bool {
    command_id.starts_with("settings.")
}

/// `@clay/chat` landing intents. Pickers are acknowledged here; Command
/// Centre session kinds come in the next Phase 25 task. Empty submit is a no-op.
pub(crate) fn is_chat_command(command_id: &str) -> bool {
    command_id.starts_with("chat.")
}

/// Bounded appearance values accepted by `settings.setAppearance`.
const SETTINGS_APPEARANCE_VALUES: &[&str] = &["light", "dark", "system"];

impl CommandExecutor {
    /// Validate a settings intent. `settings.setTheme` requires a
    /// first-party `@clay/theme-*` bundled specifier (carried as
    /// `arguments.item_id` from the dropdown list source);
    /// `settings.setAppearance` requires a bounded light/dark/system
    /// value; other `settings.*` intents (open, close, reset,
    /// setTypography) are accepted — typography bounds are enforced at apply
    /// time by the existing `setTypography` op.
    pub fn execute_settings(
        &self,
        request: CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandExecutionDiagnostic> {
        match request.command_id.as_str() {
            "settings.setTheme" => {
                let Some(specifier) = argument_string(&request.arguments, "item_id")
                    .or_else(|| argument_string(&request.arguments, "specifier"))
                else {
                    return Err(diagnostic(
                        &request.command_id,
                        CommandExecutionRule::InvalidArguments,
                        "settings.setTheme requires an item_id/specifier argument",
                    ));
                };
                if !specifier.starts_with("@clay/theme-")
                    || crate::packages::bundled::bundled_entry(&specifier).is_none()
                {
                    return Err(diagnostic(
                        &request.command_id,
                        CommandExecutionRule::InvalidArguments,
                        format!(
                            "settings.setTheme requires a first-party @clay/theme-* specifier, got `{specifier}`"
                        ),
                    ));
                }
            }
            "settings.setAppearance" => {
                let Some(value) = argument_string(&request.arguments, "item_id")
                    .or_else(|| argument_string(&request.arguments, "appearance"))
                else {
                    return Err(diagnostic(
                        &request.command_id,
                        CommandExecutionRule::InvalidArguments,
                        "settings.setAppearance requires an item_id/appearance argument",
                    ));
                };
                if !SETTINGS_APPEARANCE_VALUES.contains(&value.as_str()) {
                    return Err(diagnostic(
                        &request.command_id,
                        CommandExecutionRule::InvalidArguments,
                        format!("settings.setAppearance requires light/dark/system, got `{value}`"),
                    ));
                }
            }
            "settings.setTypography" => {
                let Some(raw) = argument_string(&request.arguments, "typography") else {
                    return Err(diagnostic(
                        &request.command_id,
                        CommandExecutionRule::InvalidArguments,
                        "settings.setTypography requires a complete typography argument",
                    ));
                };
                let value = serde_json::from_str(&raw).map_err(|_| {
                    diagnostic(
                        &request.command_id,
                        CommandExecutionRule::InvalidArguments,
                        "settings.setTypography typography argument is not valid JSON",
                    )
                })?;
                crate::server::ops::typography::validate_typography_request(&value).map_err(
                    |message| {
                        diagnostic(
                            &request.command_id,
                            CommandExecutionRule::InvalidArguments,
                            message,
                        )
                    },
                )?;
            }
            "settings.open" | "settings.close" | "settings.reset" => {}
            _ => {
                return Err(diagnostic(
                    &request.command_id,
                    CommandExecutionRule::UnknownCommand,
                    "unknown settings.* command",
                ));
            }
        }
        Ok(CommandExecutionResult {
            command_id: request.command_id.clone(),
            routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
            target: request.target,
            status: CommandExecutionStatus::Accepted,
        })
    }

    /// Chat landing intents. Empty/whitespace `chat.submit` is a no-op.
    /// Picker commands acknowledge; they do not open a second overlay here.
    pub fn execute_chat(
        &self,
        request: CommandExecutionRequest,
    ) -> Result<CommandExecutionResult, CommandExecutionDiagnostic> {
        if !is_chat_command(&request.command_id) {
            return Err(diagnostic(
                &request.command_id,
                CommandExecutionRule::UnknownCommand,
                "command is not a chat.* command",
            ));
        }
        if request.command_id == "chat.submit" {
            let value = argument_string(&request.arguments, "value")
                .or_else(|| argument_string(&request.arguments, "text"))
                .unwrap_or_default();
            if value.trim().is_empty() {
                return Ok(CommandExecutionResult {
                    command_id: request.command_id,
                    routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
                    target: request.target,
                    status: CommandExecutionStatus::Accepted,
                });
            }
        }
        Ok(CommandExecutionResult {
            command_id: request.command_id,
            routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
            target: request.target,
            status: CommandExecutionStatus::Accepted,
        })
    }
}

fn argument_string(arguments: &serde_json::Value, name: &str) -> Option<String> {
    arguments
        .as_object()
        .and_then(|object| object.get(name))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

/// Execute an open command: in-root files use [`WorkspaceState::open_existing_file`];
/// out-of-root files use [`WorkspaceState::open_selected_file`] and therefore the
/// selected-file single-file grant flow.
async fn execute_open(
    workspace: &mut WorkspaceState,
    client_id: ClientId,
    arguments: &Value,
    command_id: &str,
) -> Result<CommandExecutionStatus, CommandExecutionDiagnostic> {
    let args = OpenFileArguments::extract(arguments, command_id)?;

    let lease = if let Some((root_id, relative_path)) = args.in_root_path() {
        workspace
            .open_existing_file(root_id, relative_path, client_id)
            .await
            .map_err(|error| workspace_diagnostic(command_id, error))?
    } else if let Some(absolute_path) = args.absolute_path() {
        workspace
            .open_selected_file(absolute_path, client_id)
            .await
            .map_err(|error| workspace_diagnostic(command_id, error))?
    } else {
        return Err(diagnostic(
            command_id,
            CommandExecutionRule::InvalidArguments,
            "open command requires either (workspaceRootId + relativePath) or absolutePath",
        ));
    };

    let snapshot = lease.snapshot(client_id).await;
    Ok(CommandExecutionStatus::Workspace(
        WorkspaceActionResult::Opened(snapshot),
    ))
}

/// Bounded arguments for an open command. Accepts either an in-root path
/// reference or an absolute path for selected-file grant fallback.
#[derive(Debug, Default)]
struct OpenFileArguments {
    workspace_root_id: Option<WorkspaceRootId>,
    relative_path: Option<String>,
    absolute_path: Option<std::path::PathBuf>,
}

impl OpenFileArguments {
    fn extract(arguments: &Value, command_id: &str) -> Result<Self, CommandExecutionDiagnostic> {
        let root_id = arguments.get("workspaceRootId").and_then(Value::as_u64);
        let relative_path = arguments
            .get("relativePath")
            .and_then(Value::as_str)
            .map(String::from);
        let absolute_path = arguments
            .get("absolutePath")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from);

        let has_in_root = root_id.is_some() || relative_path.is_some();
        let has_absolute = absolute_path.is_some();

        if !has_in_root && !has_absolute {
            return Err(diagnostic(
                command_id,
                CommandExecutionRule::InvalidArguments,
                "open command requires workspaceRootId/relativePath or absolutePath",
            ));
        }

        if root_id.is_some() != relative_path.is_some() {
            return Err(diagnostic(
                command_id,
                CommandExecutionRule::InvalidArguments,
                "open command requires both workspaceRootId and relativePath together",
            ));
        }

        Ok(Self {
            workspace_root_id: root_id,
            relative_path,
            absolute_path,
        })
    }

    fn in_root_path(&self) -> Option<(WorkspaceRootId, &str)> {
        self.workspace_root_id.zip(self.relative_path.as_deref())
    }

    fn absolute_path(&self) -> Option<&std::path::Path> {
        self.absolute_path.as_deref()
    }
}

fn navigate_directory_arguments(
    arguments: &Value,
    command_id: &str,
) -> Result<(WorkspaceRootId, std::path::PathBuf), CommandExecutionDiagnostic> {
    let root_id = arguments
        .get("workspaceRootId")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            diagnostic(
                command_id,
                CommandExecutionRule::InvalidArguments,
                "directory command requires a non-negative integer `workspaceRootId` argument",
            )
        })?;
    let relative_path = arguments
        .get("relativePath")
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            diagnostic(
                command_id,
                CommandExecutionRule::InvalidArguments,
                "directory command requires a string `relativePath` argument",
            )
        })?;
    Ok((root_id, relative_path))
}

fn reveal_document_id(
    arguments: &Value,
    command_id: &str,
) -> Result<DocumentId, CommandExecutionDiagnostic> {
    arguments
        .get("documentId")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            diagnostic(
                command_id,
                CommandExecutionRule::InvalidArguments,
                "reveal command requires a non-negative integer `documentId` argument",
            )
        })
}

fn git_workspace_root_id(
    arguments: &Value,
    command_id: &str,
) -> Result<WorkspaceRootId, CommandExecutionDiagnostic> {
    arguments
        .get("workspaceRootId")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .or_else(|| arguments.get("workspaceRootId").and_then(Value::as_u64))
        .ok_or_else(|| {
            diagnostic(
                command_id,
                CommandExecutionRule::InvalidArguments,
                "Git refresh command requires workspaceRootId",
            )
        })
}

fn workspace_diagnostic(command_id: &str, error: WorkspaceError) -> CommandExecutionDiagnostic {
    let message = error.diagnostic().message;
    diagnostic(command_id, CommandExecutionRule::InvalidArguments, message)
}

/// Validate a registered/built-in command request: routing policy, provenance,
/// declared permissions, argument shape/budget, and target authority. Shared
/// between [`CommandExecutor::execute_registered`] and discovery execution so
/// discovery commands pass the exact same validation pipeline as package
/// commands.
fn validate(
    command: &RegisteredCommand,
    request: &CommandExecutionRequest,
) -> Result<(), CommandExecutionDiagnostic> {
    validate_routing(command)?;
    validate_provenance(command, request)?;
    validate_permissions(command, request)?;
    validate_arguments(request)?;
    validate_target(command, request)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuiltinCommandKind {
    General,
    Reload,
    ModeDiscovery,
    Workspace,
    Git,
}

#[derive(Clone, Copy, Debug)]
struct BuiltinCommandDefinition {
    id: &'static str,
    display_name: &'static str,
    kind: BuiltinCommandKind,
}

macro_rules! builtin_commands {
    ($($name:ident => ($id:literal, $display_name:literal, $kind:ident)),+ $(,)?) => {
        $(pub(crate) const $name: &str = $id;)+
        const BUILTIN_COMMAND_DEFINITIONS: &[BuiltinCommandDefinition] = &[
            $(BuiltinCommandDefinition {
                id: $name,
                display_name: $display_name,
                kind: BuiltinCommandKind::$kind,
            }),+
        ];
        const BUILTIN_SERVER_COMMAND_IDS: &[&str] = &[$($name),+];
    };
}

builtin_commands! {
    CONTROL_CENTER_COMMAND_ID => ("controlCenter.open", "Open Control Center", General),
    OPEN_PATH_BROWSER_COMMAND_ID => ("controlCenter.openPath", "Browse Filesystem", General),
    OPEN_AGENT_PICKER_COMMAND_ID => ("agent.clientOpenAgentPicker", "Choose Agent", General),
    OPEN_PROVIDER_PICKER_COMMAND_ID => ("agent.clientOpenProviderPicker", "Choose Provider", General),
    OPEN_MODEL_PICKER_COMMAND_ID => ("agent.clientOpenModelPicker", "Choose Model", General),
    OPEN_PROVIDER_SETUP_COMMAND_ID => ("agent.clientOpenProviderSetup", "Configure Provider", General),
    OPEN_SESSION_PICKER_COMMAND_ID => ("agent.clientOpenSessionPicker", "Resume Session", General),
    RELOAD_CONFIGURATION_COMMAND_ID => ("runtime.reloadConfiguration", "Reload Configuration and Packages", Reload),
    REFRESH_WORKSPACE_COMMAND_ID => ("workspace.refresh", "Refresh Workspace", General),
    FOCUS_ACTIVE_DOCUMENT_COMMAND_ID => ("document.focus_active", "Focus Active Document", General),
    OPEN_RECENT_DOCUMENT_COMMAND_ID => ("document.open_recent", "Open Recent Document", General),
    LIST_ACTIVE_MODES_COMMAND_ID => ("modes.listActiveModes", "List Active Modes", ModeDiscovery),
    EXPLAIN_ACTIVE_MODE_COMMAND_ID => ("modes.explainActiveMode", "Explain Active Mode", ModeDiscovery),
    OPEN_FILE_COMMAND_ID => ("workspace.openFile", "Open File", Workspace),
    REVEAL_IN_TREE_COMMAND_ID => ("workspace.revealInTree", "Reveal in File Tree", Workspace),
    OPEN_FUZZY_FILE_COMMAND_ID => ("workspace.openFuzzyFile", "Open File by Name", Workspace),
    OPEN_DIRECTORY_COMMAND_ID => ("workspace.openDirectory", "Open Directory", Workspace),
    TOGGLE_FILE_BROWSER_COMMAND_ID => ("workspace.toggleFileBrowser", "Toggle File Browser", Workspace),
    LIST_GIT_STATUSES_COMMAND_ID => ("git.listStatuses", "List Git Statuses", Git),
    REFRESH_GIT_STATUS_COMMAND_ID => ("git.refreshStatus", "Refresh Git Status", Git),
    HOVER_COMMAND_ID => ("language.hover", "Hover", General),
    GO_TO_DEFINITION_COMMAND_ID => ("language.goToDefinition", "Go to Definition", General),
    CODE_ACTIONS_COMMAND_ID => ("language.codeActions", "Code Actions", General),
    SIGNATURE_HELP_COMMAND_ID => ("language.signatureHelp", "Signature Help", General),
    PREVIEW_EDIT_COMMAND_ID => ("language.previewEdit", "Preview Edit (Deferred)", General),
    DISMISS_LANGUAGE_RESULT_COMMAND_ID => ("language.dismissResult", "Dismiss Language Result", General),
    NAVIGATE_DEFINITION_COMMAND_ID => ("language.navigateDefinition", "Navigate Definition", General),
}

fn builtin_definition(command_id: &str) -> Option<&'static BuiltinCommandDefinition> {
    BUILTIN_COMMAND_DEFINITIONS
        .iter()
        .find(|command| command.id == command_id)
}

pub(crate) fn is_mode_discovery_command(command_id: &str) -> bool {
    builtin_definition(command_id)
        .is_some_and(|command| command.kind == BuiltinCommandKind::ModeDiscovery)
}

/// Extract the `documentId` argument for `modes.explainActiveMode`.
/// Accepts an optional `{ "documentId": <non-negative integer> }` object;
/// missing or invalid arguments are rejected with `InvalidArguments`.
fn discovery_document_id(
    arguments: &Value,
    command_id: &str,
) -> Result<DocumentId, CommandExecutionDiagnostic> {
    let Some(document_id) = arguments.get("documentId").and_then(Value::as_u64) else {
        return Err(diagnostic(
            command_id,
            CommandExecutionRule::InvalidArguments,
            "modes.explainActiveMode requires a non-negative integer `documentId` argument",
        ));
    };
    Ok(document_id)
}

pub(crate) fn is_reload_command(command_id: &str) -> bool {
    builtin_definition(command_id).is_some_and(|command| command.kind == BuiltinCommandKind::Reload)
}

pub fn builtin_server_command_ids() -> &'static [&'static str] {
    BUILTIN_SERVER_COMMAND_IDS
}

pub fn builtin_server_command(command_id: &str) -> Option<RegisteredCommand> {
    let definition = builtin_definition(command_id)?;
    Some(RegisteredCommand {
        package_name: "clay".to_string(),
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        api_prefix: "clay".to_string(),
        command_id: definition.id.to_string(),
        display_name: definition.display_name.to_string(),
        routing_policy: if definition.kind == BuiltinCommandKind::Reload {
            RoutingPolicy::ServerFirstWithLock {
                lock_scope: crate::protocol::LockScope::Behavior,
            }
        } else {
            RoutingPolicy::ServerFirst
        },
        key_bindings: if definition.kind == BuiltinCommandKind::Reload {
            vec![KeyBindingRule::default_reload_configuration()]
        } else {
            Vec::new()
        },
        custom_properties: Default::default(),
        permissions: Vec::new(),
    })
}

fn validate_routing(command: &RegisteredCommand) -> Result<(), CommandExecutionDiagnostic> {
    if matches!(
        command.routing_policy,
        RoutingPolicy::ClientFirstPredictable
            | RoutingPolicy::ClientFirstRequiresAck
            | RoutingPolicy::ClientUiCommand
    ) {
        return Err(diagnostic(
            &command.command_id,
            CommandExecutionRule::InvalidRoutingPolicy,
            "package command execution must be server-owned",
        ));
    }
    Ok(())
}

fn validate_provenance(
    command: &RegisteredCommand,
    request: &CommandExecutionRequest,
) -> Result<(), CommandExecutionDiagnostic> {
    let Some(provenance) = &request.provenance else {
        return Ok(());
    };
    if provenance.package_name != command.package_name
        || provenance.package_version != command.package_version
        || provenance.api_prefix != command.api_prefix
    {
        return Err(diagnostic(
            &command.command_id,
            CommandExecutionRule::InvalidProvenance,
            "command execution provenance must match the registered command",
        ));
    }
    Ok(())
}

fn validate_permissions(
    command: &RegisteredCommand,
    request: &CommandExecutionRequest,
) -> Result<(), CommandExecutionDiagnostic> {
    for permission in &request.expected_permissions {
        if !command.permissions.contains(permission) {
            return Err(diagnostic(
                &command.command_id,
                CommandExecutionRule::UndeclaredPermission,
                "command execution expected permissions must be declared by the registered command",
            ));
        }
    }
    Ok(())
}

fn validate_arguments(request: &CommandExecutionRequest) -> Result<(), CommandExecutionDiagnostic> {
    if !(request.arguments.is_null() || request.arguments.is_object()) {
        return Err(diagnostic(
            &request.command_id,
            CommandExecutionRule::InvalidArguments,
            "command arguments must be null or a JSON object",
        ));
    }
    let bytes = serde_json::to_vec(&request.arguments).map_err(|_| {
        diagnostic(
            &request.command_id,
            CommandExecutionRule::InvalidArguments,
            "command arguments must be JSON serializable",
        )
    })?;
    if bytes.len() > COMMAND_ARGUMENT_BUDGET_BYTES {
        return Err(diagnostic(
            &request.command_id,
            CommandExecutionRule::InvalidArguments,
            "command arguments exceed the command execution payload budget",
        ));
    }
    Ok(())
}

fn validate_target(
    command: &RegisteredCommand,
    request: &CommandExecutionRequest,
) -> Result<(), CommandExecutionDiagnostic> {
    match &request.target {
        CommandExecutionTarget::ActiveDocument { document_id } if *document_id == 0 => {
            Err(diagnostic(
                &command.command_id,
                CommandExecutionRule::UnauthorizedTarget,
                "active-document command target must name an open document",
            ))
        }
        CommandExecutionTarget::Workspace
            if !command
                .permissions
                .contains(&PackagePermission::WorkspaceMutation) =>
        {
            Err(diagnostic(
                &command.command_id,
                CommandExecutionRule::UnauthorizedTarget,
                "workspace command target requires workspace-mutation permission",
            ))
        }
        _ => Ok(()),
    }
}

fn diagnostic(
    command_id: &str,
    rule: CommandExecutionRule,
    message: impl Into<String>,
) -> CommandExecutionDiagnostic {
    CommandExecutionDiagnostic {
        command_id: command_id.to_string(),
        rule,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashSet};

    use serde_json::json;

    use super::*;
    use crate::{
        packages::{
            commands::{PackageCommandDeclaration, RegisteredCommand},
            manifest::validate_manifest_value,
        },
        protocol::RoutingPolicy,
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

    fn declaration(command_id: &str) -> PackageCommandDeclaration {
        PackageCommandDeclaration {
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            command_id: command_id.to_string(),
            display_name: "Toggle Markdown Preview".to_string(),
            routing_policy: RoutingPolicy::ServerFirst,
            key_bindings: Vec::new(),
            custom_properties: BTreeMap::new(),
            permissions: vec![PackagePermission::ParseDocument],
        }
    }

    fn request(command_id: &str) -> CommandExecutionRequest {
        CommandExecutionRequest {
            command_id: command_id.to_string(),
            arguments: json!({ "preview": true }),
            target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
            provenance: Some(CommandExecutionProvenance {
                package_name: "@clay/markdown".to_string(),
                package_version: "0.1.0".to_string(),
                api_prefix: "markdown".to_string(),
            }),
            expected_permissions: vec![PackagePermission::ParseDocument],
        }
    }

    #[test]
    fn builtin_command_table_owns_unique_ids_and_registration_fields() {
        let ids: HashSet<_> = BUILTIN_COMMAND_DEFINITIONS
            .iter()
            .map(|command| command.id)
            .collect();
        assert_eq!(ids.len(), BUILTIN_COMMAND_DEFINITIONS.len());
        for definition in BUILTIN_COMMAND_DEFINITIONS {
            let command = builtin_server_command(definition.id).expect("defined command resolves");
            assert_eq!(command.display_name, definition.display_name);
            assert_eq!(
                matches!(
                    command.routing_policy,
                    RoutingPolicy::ServerFirstWithLock { .. }
                ),
                definition.kind == BuiltinCommandKind::Reload
            );
        }
    }

    #[test]
    fn known_builtin_server_command_executes_with_typed_result() {
        let result = CommandExecutor::new()
            .execute(
                &CommandRegistry::new(),
                CommandExecutionRequest {
                    command_id: CONTROL_CENTER_COMMAND_ID.to_string(),
                    arguments: Value::Null,
                    target: CommandExecutionTarget::Global,
                    provenance: None,
                    expected_permissions: Vec::new(),
                },
            )
            .expect("execute built-in command");

        assert_eq!(result.command_id, CONTROL_CENTER_COMMAND_ID);
        assert_eq!(result.routing_policy, RoutingPolicy::ServerFirst);
        assert_eq!(result.status, CommandExecutionStatus::Accepted);
    }

    #[test]
    fn registered_server_command_executes_with_typed_result() {
        let manifest = package_manifest();
        let mut registry = CommandRegistry::new();
        registry
            .register_command(&manifest, declaration("markdown.togglePreview"))
            .expect("register command");

        let result = CommandExecutor::new()
            .execute(&registry, request("markdown.togglePreview"))
            .expect("execute command");

        assert_eq!(result.command_id, "markdown.togglePreview");
        assert_eq!(result.routing_policy, RoutingPolicy::ServerFirst);
        assert_eq!(result.status, CommandExecutionStatus::Accepted);
    }

    #[test]
    fn unknown_command_is_rejected() {
        let error = CommandExecutor::new()
            .execute(&CommandRegistry::new(), request("markdown.missing"))
            .unwrap_err();

        assert_eq!(error.rule, CommandExecutionRule::UnknownCommand);
    }

    #[test]
    fn unbacked_package_command_is_not_accepted() {
        let manifest = package_manifest();
        let mut registry = CommandRegistry::new();
        registry
            .register_command(&manifest, declaration("markdown.toggleComment"))
            .expect("register command");
        let error = CommandExecutor::new()
            .execute(&registry, request("markdown.toggleComment"))
            .unwrap_err();
        assert_eq!(error.rule, CommandExecutionRule::UnknownCommand);
    }

    #[test]
    fn execution_rejects_mismatched_provenance_and_permissions() {
        let manifest = package_manifest();
        let mut registry = CommandRegistry::new();
        registry
            .register_command(&manifest, declaration("markdown.togglePreview"))
            .expect("register command");

        let mut bad_provenance = request("markdown.togglePreview");
        bad_provenance.provenance.as_mut().unwrap().package_version = "9.9.9".to_string();
        let error = CommandExecutor::new()
            .execute(&registry, bad_provenance)
            .unwrap_err();
        assert_eq!(error.rule, CommandExecutionRule::InvalidProvenance);

        let mut bad_permission = request("markdown.togglePreview");
        bad_permission.expected_permissions = vec![PackagePermission::RenderDecorations];
        let error = CommandExecutor::new()
            .execute(&registry, bad_permission)
            .unwrap_err();
        assert_eq!(error.rule, CommandExecutionRule::UndeclaredPermission);
    }

    #[test]
    fn git_command_ids_are_read_only_list_and_refresh() {
        // Phase 18.13: the only server-owned Git Control Center commands are
        // status list and refresh. Prove no mutating Git operation
        // (checkout/stage/commit/reset/rebase/stash/push/pull/fetch) is wired.
        const MUTATING_WORDS: &[&str] = &[
            "checkout", "switch", "stage", "add", "commit", "reset", "rebase", "stash", "push",
            "pull", "fetch", "merge", "revert", "tag",
        ];
        let commands: Vec<_> = BUILTIN_COMMAND_DEFINITIONS
            .iter()
            .filter(|command| command.kind == BuiltinCommandKind::Git)
            .map(|command| command.id)
            .collect();
        assert_eq!(
            commands,
            [LIST_GIT_STATUSES_COMMAND_ID, REFRESH_GIT_STATUS_COMMAND_ID]
        );
        for command in commands {
            for &word in MUTATING_WORDS {
                assert!(
                    !command.contains(word),
                    "server Git command {command} must not name a mutating operation"
                );
            }
        }
    }

    #[test]
    fn execution_rejects_client_first_routes_malformed_args_and_workspace_target() {
        let mut command = RegisteredCommand {
            package_name: "@clay/markdown".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "markdown".to_string(),
            command_id: "markdown.togglePreview".to_string(),
            display_name: "Toggle Markdown Preview".to_string(),
            routing_policy: RoutingPolicy::ClientFirstPredictable,
            key_bindings: Vec::new(),
            custom_properties: BTreeMap::new(),
            permissions: vec![PackagePermission::ParseDocument],
        };
        let executor = CommandExecutor::new();

        let error = executor
            .execute_registered(&command, request("markdown.togglePreview"))
            .unwrap_err();
        assert_eq!(error.rule, CommandExecutionRule::InvalidRoutingPolicy);

        command.routing_policy = RoutingPolicy::ServerFirst;
        let mut bad_args = request("markdown.togglePreview");
        bad_args.arguments = json!("not an object");
        let error = executor.execute_registered(&command, bad_args).unwrap_err();
        assert_eq!(error.rule, CommandExecutionRule::InvalidArguments);

        let mut oversized_args = request("markdown.togglePreview");
        oversized_args.arguments = json!({ "text": "x".repeat(COMMAND_ARGUMENT_BUDGET_BYTES) });
        let error = executor
            .execute_registered(&command, oversized_args)
            .unwrap_err();
        assert_eq!(error.rule, CommandExecutionRule::InvalidArguments);

        let mut workspace = request("markdown.togglePreview");
        workspace.target = CommandExecutionTarget::Workspace;
        let error = executor
            .execute_registered(&command, workspace)
            .unwrap_err();
        assert_eq!(error.rule, CommandExecutionRule::UnauthorizedTarget);
    }

    mod workspace_commands {
        use std::{fs, path::PathBuf, time::SystemTime};

        use serde_json::{Value, json};

        use super::super::{
            CommandExecutionRequest, CommandExecutionRule, CommandExecutionStatus,
            CommandExecutionTarget, CommandExecutor, OPEN_FILE_COMMAND_ID,
            REVEAL_IN_TREE_COMMAND_ID, TOGGLE_FILE_BROWSER_COMMAND_ID, WorkspaceActionResult,
        };
        use crate::packages::commands::CommandRegistry;
        use crate::server::workspace::WorkspaceState;

        fn temp_workspace(name: &str) -> PathBuf {
            let unique = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "clay-cmd-workspace-{name}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&dir).unwrap();
            dir
        }

        fn open_request(root_id: u64, relative_path: &str) -> CommandExecutionRequest {
            CommandExecutionRequest {
                command_id: OPEN_FILE_COMMAND_ID.to_string(),
                arguments: json!({
                    "workspaceRootId": root_id,
                    "relativePath": relative_path,
                }),
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            }
        }

        #[tokio::test]
        async fn open_in_root_file_returns_opened_result() {
            let root = temp_workspace("open-in-root");
            fs::write(root.join("lib.rs"), "fn main() {}").unwrap();

            let mut workspace = WorkspaceState::new();
            let root_id = workspace.add_root(root.clone()).unwrap();

            let result = CommandExecutor::new()
                .execute_workspace(
                    &CommandRegistry::new(),
                    &mut workspace,
                    41,
                    open_request(root_id, "lib.rs"),
                )
                .await
                .expect("open command should execute");

            assert_eq!(result.command_id, OPEN_FILE_COMMAND_ID);
            let CommandExecutionStatus::Workspace(WorkspaceActionResult::Opened(snapshot)) =
                result.status
            else {
                panic!("expected Opened workspace result, got {:?}", result.status);
            };
            assert_eq!(snapshot.metadata.path, "lib.rs");
            assert_eq!(
                snapshot.metadata.access,
                crate::protocol::DocumentAccess::Editable { lease_id: 1 }
            );
            assert_eq!(snapshot.head.first_chunk, "fn main() {}");

            let _ = fs::remove_dir_all(root);
        }

        #[tokio::test]
        async fn open_out_of_root_file_uses_selected_file_grant() {
            let root = temp_workspace("open-out-root");
            let outside = temp_workspace("outside");
            fs::write(outside.join("external.txt"), "external content").unwrap();

            let mut workspace = WorkspaceState::new();
            workspace.add_root(root).unwrap();

            let request = CommandExecutionRequest {
                command_id: OPEN_FILE_COMMAND_ID.to_string(),
                arguments: json!({ "absolutePath": outside.join("external.txt").to_string_lossy() }),
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            };

            let result = CommandExecutor::new()
                .execute_workspace(&CommandRegistry::new(), &mut workspace, 1, request)
                .await
                .expect("open command should create selected-file grant");

            let CommandExecutionStatus::Workspace(WorkspaceActionResult::Opened(snapshot)) =
                result.status
            else {
                panic!("expected Opened workspace result, got {:?}", result.status);
            };
            assert!(snapshot.metadata.path.contains("external.txt"));
            assert_eq!(snapshot.head.first_chunk, "external content");

            let _ = fs::remove_dir_all(outside);
        }

        #[tokio::test]
        async fn reveal_command_returns_revealed_result() {
            let root = temp_workspace("reveal");
            fs::write(root.join("main.rs"), "fn main() {}").unwrap();

            let mut workspace = WorkspaceState::new();
            let root_id = workspace.add_root(&root).unwrap();
            let opened = workspace
                .open_existing_file(root_id, "main.rs", 1)
                .await
                .unwrap();

            let result = CommandExecutor::new()
                .execute_workspace(
                    &CommandRegistry::new(),
                    &mut workspace,
                    1,
                    CommandExecutionRequest {
                        command_id: REVEAL_IN_TREE_COMMAND_ID.to_string(),
                        arguments: json!({ "documentId": opened.document_id }),
                        target: CommandExecutionTarget::Global,
                        provenance: None,
                        expected_permissions: Vec::new(),
                    },
                )
                .await
                .expect("reveal command should execute");

            assert_eq!(result.command_id, REVEAL_IN_TREE_COMMAND_ID);
            assert_eq!(
                result.status,
                CommandExecutionStatus::Workspace(WorkspaceActionResult::Revealed)
            );

            let _ = fs::remove_dir_all(root);
        }

        #[tokio::test]
        async fn toggle_command_returns_toggled_result() {
            let mut workspace = WorkspaceState::new();
            let result = CommandExecutor::new()
                .execute_workspace(
                    &CommandRegistry::new(),
                    &mut workspace,
                    1,
                    CommandExecutionRequest {
                        command_id: TOGGLE_FILE_BROWSER_COMMAND_ID.to_string(),
                        arguments: Value::Null,
                        target: CommandExecutionTarget::Global,
                        provenance: None,
                        expected_permissions: Vec::new(),
                    },
                )
                .await
                .expect("toggle command should execute");

            assert_eq!(result.command_id, TOGGLE_FILE_BROWSER_COMMAND_ID);
            assert_eq!(
                result.status,
                CommandExecutionStatus::Workspace(WorkspaceActionResult::Toggled)
            );
        }

        #[tokio::test]
        async fn open_command_rejects_missing_arguments() {
            let mut workspace = WorkspaceState::new();
            let error = CommandExecutor::new()
                .execute_workspace(
                    &CommandRegistry::new(),
                    &mut workspace,
                    1,
                    CommandExecutionRequest {
                        command_id: OPEN_FILE_COMMAND_ID.to_string(),
                        arguments: Value::Null,
                        target: CommandExecutionTarget::Global,
                        provenance: None,
                        expected_permissions: Vec::new(),
                    },
                )
                .await
                .unwrap_err();

            assert_eq!(error.rule, CommandExecutionRule::InvalidArguments);
        }

        #[tokio::test]
        async fn save_related_command_is_not_registered() {
            let mut workspace = WorkspaceState::new();
            let error = CommandExecutor::new()
                .execute_workspace(
                    &CommandRegistry::new(),
                    &mut workspace,
                    1,
                    CommandExecutionRequest {
                        command_id: "workspace.saveFile".to_string(),
                        arguments: Value::Null,
                        target: CommandExecutionTarget::Global,
                        provenance: None,
                        expected_permissions: Vec::new(),
                    },
                )
                .await
                .unwrap_err();

            assert_eq!(error.rule, CommandExecutionRule::UnknownCommand);
        }
    }
    fn settings_request(command_id: &str, args: serde_json::Value) -> CommandExecutionRequest {
        CommandExecutionRequest {
            command_id: command_id.to_string(),
            arguments: args,
            target: CommandExecutionTarget::Global,
            provenance: None,
            expected_permissions: Vec::new(),
        }
    }

    #[test]
    fn settings_set_theme_accepts_first_party_modus_specifier() {
        let executor = CommandExecutor::new();
        let result = executor
            .execute_settings(settings_request(
                "settings.setTheme",
                json!({ "item_id": "@clay/theme-modus-vivendi" }),
            ))
            .expect("first-party @clay/theme-* specifier must validate");
        assert_eq!(result.status, CommandExecutionStatus::Accepted);
        assert_eq!(result.command_id, "settings.setTheme");
    }

    #[test]
    fn settings_set_theme_rejects_non_first_party_specifier() {
        let executor = CommandExecutor::new();
        let err = executor
            .execute_settings(settings_request(
                "settings.setTheme",
                json!({ "item_id": "@vendor/evil" }),
            ))
            .expect_err("non-first-party specifier must be rejected");
        assert_eq!(err.rule, CommandExecutionRule::InvalidArguments);
    }

    #[test]
    fn settings_set_theme_rejects_non_theme_first_party_specifier() {
        let executor = CommandExecutor::new();
        let err = executor
            .execute_settings(settings_request(
                "settings.setTheme",
                json!({ "item_id": "@clay/markdown" }),
            ))
            .expect_err("first-party non-theme specifier must be rejected");
        assert_eq!(err.rule, CommandExecutionRule::InvalidArguments);
    }

    #[test]
    fn settings_set_appearance_accepts_bounded_enum() {
        let executor = CommandExecutor::new();
        for value in ["light", "dark", "system"] {
            let result = executor
                .execute_settings(settings_request(
                    "settings.setAppearance",
                    json!({ "item_id": value }),
                ))
                .expect("bounded appearance value must validate");
            assert_eq!(result.status, CommandExecutionStatus::Accepted);
        }
    }

    #[test]
    fn settings_set_appearance_rejects_unknown_value() {
        let executor = CommandExecutor::new();
        let err = executor
            .execute_settings(settings_request(
                "settings.setAppearance",
                json!({ "item_id": "auto" }),
            ))
            .expect_err("unknown appearance must be rejected");
        assert_eq!(err.rule, CommandExecutionRule::InvalidArguments);
    }

    #[test]
    fn settings_set_typography_and_lifecycle_commands_accept() {
        let executor = CommandExecutor::new();
        let typography = json!({
            "monospace": { "families": ["monospace"], "size": 16 },
            "proportional": { "families": ["sans-serif"], "size": 16 },
            "ui": { "families": ["system-ui"], "size": 12 },
            "hierarchy": {
                "display": 1.5, "title": 1.16, "section": 1.08,
                "body": 1.0, "status": 1.0, "detail": 0.83, "caption": 0.75
            }
        });
        let result = executor
            .execute_settings(settings_request(
                "settings.setTypography",
                json!({ "typography": typography.to_string() }),
            ))
            .expect("complete typography accepts");
        assert_eq!(result.status, CommandExecutionStatus::Accepted);
        for command_id in ["settings.open", "settings.close", "settings.reset"] {
            let result = executor
                .execute_settings(settings_request(command_id, json!({})))
                .expect("lifecycle settings commands accept");
            assert_eq!(
                result.status,
                CommandExecutionStatus::Accepted,
                "{command_id}"
            );
        }
        let rejected = executor
            .execute_settings(settings_request(
                "settings.setTypography",
                json!({ "typography": "{}" }),
            ))
            .expect_err("partial typography fails closed");
        assert_eq!(rejected.rule, CommandExecutionRule::InvalidArguments);
    }

    #[test]
    fn is_settings_command_recognizes_prefix() {
        assert!(is_settings_command("settings.setTheme"));
        assert!(is_settings_command("settings.open"));
        assert!(!is_settings_command("controlCenter.open"));
        assert!(!is_settings_command("markdown.togglePreview"));
    }

    #[test]
    fn chat_empty_submit_is_noop_and_pickers_accept() {
        let executor = CommandExecutor::new();
        for args in [json!({}), json!({ "value": "" }), json!({ "value": "   " })] {
            let result = executor
                .execute_chat(settings_request("chat.submit", args))
                .expect("empty submit is a no-op");
            assert_eq!(result.status, CommandExecutionStatus::Accepted);
        }
        for command_id in [
            "chat.profile",
            "chat.openAgentPicker",
            "chat.openProviderPicker",
            "chat.openModelPicker",
        ] {
            let result = executor
                .execute_chat(settings_request(command_id, json!({})))
                .expect("chat chrome commands accept");
            assert_eq!(
                result.status,
                CommandExecutionStatus::Accepted,
                "{command_id}"
            );
        }
        assert!(is_chat_command("chat.submit"));
        assert!(!is_chat_command("settings.open"));
    }
}
