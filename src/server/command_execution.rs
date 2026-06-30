use serde_json::Value;

use crate::{
    packages::{
        commands::{CommandRegistry, RegisteredCommand},
        modes::ModeRegistry,
        permissions::PackagePermission,
    },
    perf::budgets::COMMAND_ARGUMENT_BUDGET_BYTES,
    protocol::{DocumentId, RoutingPolicy},
};

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
    validate_provenance(command, &request)?;
    validate_permissions(command, &request)?;
    validate_arguments(&request)?;
    validate_target(command, &request)?;
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
    ]
}

pub fn builtin_server_command(command_id: &str) -> Option<RegisteredCommand> {
    match command_id {
        "clay.controlCenter.open"
        | "workspace.refresh"
        | "document.focus_active"
        | "document.open_recent"
        | "clay.modes.listActiveModes"
        | "clay.modes.explainActiveMode" => Some(RegisteredCommand {
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
}
