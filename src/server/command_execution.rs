use serde_json::Value;

use crate::{
    packages::{
        commands::{CommandRegistry, RegisteredCommand},
        modes::ModeRegistry,
        permissions::PackagePermission,
    },
    perf::budgets::COMMAND_ARGUMENT_BUDGET_BYTES,
    protocol::{ClientId, DocumentId, RoutingPolicy, WorkspaceRootId},
    server::{
        git::{GitCachedStatus, GitStatusCache},
        workspace::{WorkspaceError, WorkspaceState},
    },
};

pub use crate::server::workspace::OpenDocumentSnapshot;

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
    /// (`clay.modes.listActiveModes`, `clay.modes.explainActiveMode`) resolve
    /// through [`CommandExecutor::execute_discovery`] by reading installed
    /// `ModeRegistry` state; the payload carries no execution, document, or
    /// workspace authority.
    Discovery(DiscoveryResult),
    /// Phase 18.12 workspace action result. Produced by
    /// [`CommandExecutor::execute_workspace`] when a built-in file-browser
    /// command opens, reveals, or toggles the file browser through
    /// server-authoritative workspace APIs.
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
    Opened(OpenDocumentSnapshot),
    /// A directory navigation request was accepted; the connection handler
    /// will publish a refreshed file-browser SDUI snapshot.
    Navigated {
        root_id: WorkspaceRootId,
        relative_path: std::path::PathBuf,
    },
    /// A reveal request was accepted; the server will update the focused tree
    /// state on the next SDUI snapshot.
    Revealed,
    /// A file-browser visibility toggle was accepted.
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
            "clay.modes.listActiveModes" => CommandExecutionStatus::Discovery(
                DiscoveryResult::ActiveModes(mode_registry.list_active_modes()),
            ),
            "clay.modes.explainActiveMode" => {
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
            "clay.workspace.openFile" | "clay.workspace.openFuzzyFile" => {
                execute_open(
                    workspace,
                    client_id,
                    &request.arguments,
                    &request.command_id,
                )
                .await?
            }
            "clay.workspace.openDirectory" => {
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
            "clay.workspace.revealInTree" => {
                let document_id = reveal_document_id(&request.arguments, &request.command_id)?;
                workspace
                    .document_metadata(document_id, client_id)
                    .await
                    .map_err(|error| workspace_diagnostic(&request.command_id, error))?;
                CommandExecutionStatus::Workspace(WorkspaceActionResult::Revealed)
            }
            "clay.workspace.toggleFileBrowser" => {
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
            "clay.git.listStatuses" => CommandExecutionStatus::Git(GitCommandResult::Statuses(
                git_status_cache.list_cached(workspace).await,
            )),
            "clay.git.refreshStatus" => {
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

/// Phase 18.12 workspace command IDs. Routed through
/// [`CommandExecutor::execute_workspace`] so they resolve against
/// [`WorkspaceState`] root/grant APIs.
pub(crate) const WORKSPACE_COMMAND_IDS: &[&str] = &[
    "clay.workspace.openFile",
    "clay.workspace.revealInTree",
    "clay.workspace.openFuzzyFile",
    "clay.workspace.openDirectory",
    "clay.workspace.toggleFileBrowser",
];

pub(crate) fn is_workspace_command(command_id: &str) -> bool {
    WORKSPACE_COMMAND_IDS.contains(&command_id)
}

pub(crate) const GIT_COMMAND_IDS: &[&str] = &["clay.git.listStatuses", "clay.git.refreshStatus"];

pub(crate) fn is_git_command(command_id: &str) -> bool {
    GIT_COMMAND_IDS.contains(&command_id)
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

/// Phase 18.9 built-in mode-discovery command IDs, reachable through the
/// Control Center and the command execution path. Internal to the crate: the
/// routing decision lives in `ops/mod.rs` and command resolution here.
pub(crate) const MODE_DISCOVERY_COMMAND_IDS: &[&str] =
    &["clay.modes.listActiveModes", "clay.modes.explainActiveMode"];

/// Internal routing predicate: `ops/mod.rs` uses it to route discovery commands
/// through [`CommandExecutor::execute_discovery`] (ModeRegistry access) instead
/// of the plain `execute` path.
pub(crate) fn is_mode_discovery_command(command_id: &str) -> bool {
    MODE_DISCOVERY_COMMAND_IDS.contains(&command_id)
}

/// Extract the `documentId` argument for `clay.modes.explainActiveMode`.
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
            "clay.modes.explainActiveMode requires a non-negative integer `documentId` argument",
        ));
    };
    Ok(document_id)
}

pub fn builtin_server_command_ids() -> &'static [&'static str] {
    &[
        "clay.controlCenter.open",
        "workspace.refresh",
        "document.focus_active",
        "document.open_recent",
        "clay.modes.listActiveModes",
        "clay.modes.explainActiveMode",
        "clay.workspace.openFile",
        "clay.workspace.revealInTree",
        "clay.workspace.openFuzzyFile",
        "clay.workspace.openDirectory",
        "clay.workspace.toggleFileBrowser",
        "clay.git.listStatuses",
        "clay.git.refreshStatus",
        "clay.language.hover",
        "clay.language.goToDefinition",
        "clay.language.codeActions",
        "clay.language.signatureHelp",
        "clay.language.previewEdit",
        "clay.language.dismissResult",
        "clay.language.navigateDefinition",
    ]
}

pub fn builtin_server_command(command_id: &str) -> Option<RegisteredCommand> {
    match command_id {
        "clay.controlCenter.open"
        | "workspace.refresh"
        | "document.focus_active"
        | "document.open_recent"
        | "clay.modes.listActiveModes"
        | "clay.modes.explainActiveMode"
        | "clay.workspace.openFile"
        | "clay.workspace.revealInTree"
        | "clay.workspace.openFuzzyFile"
        | "clay.workspace.openDirectory"
        | "clay.workspace.toggleFileBrowser"
        | "clay.git.listStatuses"
        | "clay.git.refreshStatus"
        | "clay.language.hover"
        | "clay.language.goToDefinition"
        | "clay.language.codeActions"
        | "clay.language.signatureHelp"
        | "clay.language.previewEdit"
        | "clay.language.dismissResult"
        | "clay.language.navigateDefinition" => Some(RegisteredCommand {
            package_name: "clay".to_string(),
            package_version: env!("CARGO_PKG_VERSION").to_string(),
            api_prefix: "clay".to_string(),
            command_id: command_id.to_string(),
            display_name: builtin_display_name(command_id).to_string(),
            routing_policy: RoutingPolicy::ServerFirst,
            key_bindings: Vec::new(),
            custom_properties: Default::default(),
            permissions: Vec::new(),
        }),
        _ => None,
    }
}

fn builtin_display_name(command_id: &str) -> &'static str {
    match command_id {
        "clay.controlCenter.open" => "Open Control Center",
        "workspace.refresh" => "Refresh Workspace",
        "document.focus_active" => "Focus Active Document",
        "document.open_recent" => "Open Recent Document",
        "clay.modes.listActiveModes" => "List Active Modes",
        "clay.modes.explainActiveMode" => "Explain Active Mode",
        "clay.workspace.openFile" => "Open File",
        "clay.workspace.revealInTree" => "Reveal in File Tree",
        "clay.workspace.openFuzzyFile" => "Open File by Name",
        "clay.workspace.openDirectory" => "Open Directory",
        "clay.workspace.toggleFileBrowser" => "Toggle File Browser",
        "clay.git.listStatuses" => "List Git Statuses",
        "clay.git.refreshStatus" => "Refresh Git Status",
        "clay.language.hover" => "Hover",
        "clay.language.goToDefinition" => "Go to Definition",
        "clay.language.codeActions" => "Code Actions",
        "clay.language.signatureHelp" => "Signature Help",
        "clay.language.previewEdit" => "Preview Edit (Deferred)",
        "clay.language.dismissResult" => "Dismiss Language Result",
        "clay.language.navigateDefinition" => "Navigate Definition",
        _ => "Built-in Command",
    }
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
    use std::collections::BTreeMap;

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
    fn known_builtin_server_command_executes_with_typed_result() {
        let result = CommandExecutor::new()
            .execute(
                &CommandRegistry::new(),
                CommandExecutionRequest {
                    command_id: "clay.controlCenter.open".to_string(),
                    arguments: Value::Null,
                    target: CommandExecutionTarget::Global,
                    provenance: None,
                    expected_permissions: Vec::new(),
                },
            )
            .expect("execute built-in command");

        assert_eq!(result.command_id, "clay.controlCenter.open");
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
        assert_eq!(
            GIT_COMMAND_IDS,
            &["clay.git.listStatuses", "clay.git.refreshStatus"]
        );
        for &command in GIT_COMMAND_IDS {
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
            CommandExecutionTarget, CommandExecutor, WorkspaceActionResult,
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
                command_id: "clay.workspace.openFile".to_string(),
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

            assert_eq!(result.command_id, "clay.workspace.openFile");
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
            assert_eq!(snapshot.text, "fn main() {}");

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
                command_id: "clay.workspace.openFile".to_string(),
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
            assert_eq!(snapshot.text, "external content");

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
                        command_id: "clay.workspace.revealInTree".to_string(),
                        arguments: json!({ "documentId": opened.document_id }),
                        target: CommandExecutionTarget::Global,
                        provenance: None,
                        expected_permissions: Vec::new(),
                    },
                )
                .await
                .expect("reveal command should execute");

            assert_eq!(result.command_id, "clay.workspace.revealInTree");
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
                        command_id: "clay.workspace.toggleFileBrowser".to_string(),
                        arguments: Value::Null,
                        target: CommandExecutionTarget::Global,
                        provenance: None,
                        expected_permissions: Vec::new(),
                    },
                )
                .await
                .expect("toggle command should execute");

            assert_eq!(result.command_id, "clay.workspace.toggleFileBrowser");
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
                        command_id: "clay.workspace.openFile".to_string(),
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
                        command_id: "clay.workspace.saveFile".to_string(),
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
}
