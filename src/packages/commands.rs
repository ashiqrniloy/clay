use std::collections::{BTreeMap, HashMap, HashSet};

use crate::behavior::manifest::{ManifestValidationError, validate_manifest};
use crate::packages::manifest::{ClayPackageManifest, is_valid_api_prefix};
use crate::packages::permissions::PackagePermission;
use crate::perf::budgets::KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS;
use crate::protocol::{
    BehaviorManifest, BehaviorScope, BehaviorVersion, CommandAuthority, CommandDeclaration,
    EditorBehaviorRules, KeyBindingRule, RoutingPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCommandDeclaration {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub command_id: String,
    pub display_name: String,
    pub routing_policy: RoutingPolicy,
    pub key_bindings: Vec<KeyBindingRule>,
    pub custom_properties: BTreeMap<String, String>,
    pub permissions: Vec<PackagePermission>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageBehaviorContribution {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub manifest_id: String,
    pub behavior_version: BehaviorVersion,
    pub scope: BehaviorScope,
    pub commands: Vec<PackageCommandDeclaration>,
    pub keymaps: Vec<KeyBindingRule>,
    pub editor_rules: EditorBehaviorRules,
    pub text_transforms: Vec<PackageTextTransformDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTextTransformDeclaration {
    pub transform_id: String,
    pub kind: TextTransformKind,
    pub javascript_callback: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextTransformKind {
    EnterRule,
    TabRule,
    PairRule,
    CommentContinuation,
    AutocompleteTrigger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredCommand {
    pub package_name: String,
    pub package_version: String,
    pub api_prefix: String,
    pub command_id: String,
    pub display_name: String,
    pub routing_policy: RoutingPolicy,
    pub key_bindings: Vec<KeyBindingRule>,
    pub custom_properties: BTreeMap<String, String>,
    pub permissions: Vec<PackagePermission>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDiagnostic {
    pub package_name: Option<Box<str>>,
    pub package_version: Option<Box<str>>,
    pub api_prefix: Option<Box<str>>,
    pub command_id: Option<Box<str>>,
    pub rule: CommandValidationRule,
    pub message: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandValidationRule {
    MissingPermission,
    InvalidPrefix,
    InvalidCommandId,
    DuplicateCommandId,
    InvalidMetadata,
    UndeclaredPermission,
    AmbiguousKeyBinding,
    InvalidBehaviorContribution,
    ExecutableTextTransform,
}

#[derive(Debug, Default)]
pub struct CommandRegistry {
    commands: HashMap<String, RegisteredCommand>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn keypress_to_local_paint_budget_ms(&self) -> u64 {
        KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS
    }

    pub fn register_command(
        &mut self,
        package: &ClayPackageManifest,
        declaration: PackageCommandDeclaration,
    ) -> Result<RegisteredCommand, CommandDiagnostic> {
        let context = CommandDiagnosticContext::from_declaration(&declaration);
        validate_package_command(package, &declaration, &context)?;
        if self.commands.contains_key(&declaration.command_id) {
            return Err(context.diagnostic(
                CommandValidationRule::DuplicateCommandId,
                "command IDs must be unique among enabled package and built-in commands",
            ));
        }

        let registered = RegisteredCommand {
            package_name: declaration.package_name,
            package_version: declaration.package_version,
            api_prefix: declaration.api_prefix,
            command_id: declaration.command_id.clone(),
            display_name: declaration.display_name,
            routing_policy: declaration.routing_policy,
            key_bindings: declaration.key_bindings,
            custom_properties: declaration.custom_properties,
            permissions: declaration.permissions,
        };
        self.commands
            .insert(registered.command_id.clone(), registered.clone());
        Ok(registered)
    }

    pub fn validate_behavior_contribution(
        &self,
        package: &ClayPackageManifest,
        contribution: PackageBehaviorContribution,
    ) -> Result<BehaviorManifest, CommandDiagnostic> {
        let context = CommandDiagnosticContext::from_contribution(&contribution);
        if package.name != contribution.package_name
            || package.version != contribution.package_version
            || package.clay.api_prefix != contribution.api_prefix
        {
            return Err(context.diagnostic(
                CommandValidationRule::InvalidPrefix,
                "behavior contribution provenance must match the validated package manifest",
            ));
        }
        if contribution.manifest_id.trim().is_empty() {
            return Err(context.diagnostic(
                CommandValidationRule::InvalidBehaviorContribution,
                "behavior contribution manifest_id must be non-empty",
            ));
        }
        for transform in &contribution.text_transforms {
            validate_text_transform(transform, &context)?;
        }

        let mut manifest = BehaviorManifest::minimal_text_editing(contribution.behavior_version);
        manifest.manifest_id = contribution.manifest_id;
        manifest.scope = contribution.scope;
        manifest.editor_rules = contribution.editor_rules;
        for command in contribution.commands {
            validate_package_command(
                package,
                &command,
                &CommandDiagnosticContext::from_declaration(&command),
            )?;
            manifest.commands.push(to_behavior_command(&command));
            manifest.keymaps.extend(command.key_bindings);
        }
        manifest.keymaps.extend(contribution.keymaps);
        for registered in self.commands.values() {
            manifest.commands.push(CommandDeclaration {
                command_id: registered.command_id.clone(),
                display_name: registered.display_name.clone(),
                routing_policy: registered.routing_policy.clone(),
                authority: CommandAuthority::ServerIntent,
            });
        }

        validate_manifest(&manifest)
            .map_err(|error| manifest_error_to_diagnostic(error, &context))?;
        Ok(manifest)
    }

    pub fn get(&self, command_id: &str) -> Option<&RegisteredCommand> {
        self.commands.get(command_id)
    }

    pub fn list(&self) -> impl Iterator<Item = &RegisteredCommand> {
        self.commands.values()
    }

    /// Test-only helper to insert a command directly without package manifest
    /// validation. Used by integration tests that need to exercise routing or
    /// permission combinations that package registration intentionally rejects.
    #[doc(hidden)]
    pub fn insert_test_command(&mut self, command: RegisteredCommand) {
        self.commands.insert(command.command_id.clone(), command);
    }
}

fn validate_package_command(
    package: &ClayPackageManifest,
    declaration: &PackageCommandDeclaration,
    context: &CommandDiagnosticContext,
) -> Result<(), CommandDiagnostic> {
    if !package
        .clay
        .permissions
        .contains(&PackagePermission::CommandRegistration)
    {
        return Err(context.diagnostic(
            CommandValidationRule::MissingPermission,
            "package must declare command-registration before registering package commands",
        ));
    }
    if package.name != declaration.package_name
        || package.version != declaration.package_version
        || package.clay.api_prefix != declaration.api_prefix
    {
        return Err(context.diagnostic(
            CommandValidationRule::InvalidPrefix,
            "command declaration provenance must match the validated package manifest",
        ));
    }
    if !is_valid_api_prefix(&declaration.api_prefix) {
        return Err(context.diagnostic(
            CommandValidationRule::InvalidPrefix,
            "command declaration api_prefix must match package prefix rules",
        ));
    }
    if declaration.command_id.starts_with("clay.")
        || !is_package_owned_id(&declaration.command_id, &declaration.api_prefix)
    {
        return Err(context.diagnostic(
            CommandValidationRule::InvalidCommandId,
            "command IDs must use the package apiPrefix or apiPrefix.* namespace",
        ));
    }
    if declaration.display_name.trim().is_empty() {
        return Err(context.diagnostic(
            CommandValidationRule::InvalidMetadata,
            "command display_name must be non-empty",
        ));
    }
    if matches!(
        declaration.routing_policy,
        RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck
    ) {
        return Err(context.diagnostic(
            CommandValidationRule::InvalidMetadata,
            "package command registration cannot grant built-in client edit authority",
        ));
    }
    if matches!(declaration.routing_policy, RoutingPolicy::ClientUiCommand) {
        return Err(context.diagnostic(
            CommandValidationRule::InvalidMetadata,
            "package command registration cannot grant native client UI authority",
        ));
    }

    let mut seen_permissions = HashSet::new();
    for permission in &declaration.permissions {
        if !seen_permissions.insert(permission.as_str())
            || !package.clay.permissions.contains(permission)
        {
            return Err(context.diagnostic(
                CommandValidationRule::UndeclaredPermission,
                "command permissions must be unique and already declared by the package manifest",
            ));
        }
    }
    validate_key_bindings_reference_command(declaration, context)?;
    Ok(())
}

fn validate_key_bindings_reference_command(
    declaration: &PackageCommandDeclaration,
    context: &CommandDiagnosticContext,
) -> Result<(), CommandDiagnostic> {
    for key_binding in &declaration.key_bindings {
        if key_binding.command_id != declaration.command_id {
            return Err(context.diagnostic(
                CommandValidationRule::InvalidBehaviorContribution,
                "package command key bindings must target their owning command ID",
            ));
        }
    }
    Ok(())
}

fn validate_text_transform(
    transform: &PackageTextTransformDeclaration,
    context: &CommandDiagnosticContext,
) -> Result<(), CommandDiagnostic> {
    if transform.transform_id.trim().is_empty() {
        return Err(context.diagnostic(
            CommandValidationRule::InvalidBehaviorContribution,
            "text transform declarations must include a non-empty transform_id",
        ));
    }
    if transform.javascript_callback.is_some() || transform.code.is_some() {
        return Err(context.diagnostic(
            CommandValidationRule::ExecutableTextTransform,
            "text transform contributions are inert manifest data and cannot include JavaScript callback or code fields",
        ));
    }
    Ok(())
}

fn to_behavior_command(declaration: &PackageCommandDeclaration) -> CommandDeclaration {
    CommandDeclaration {
        command_id: declaration.command_id.clone(),
        display_name: declaration.display_name.clone(),
        routing_policy: declaration.routing_policy.clone(),
        authority: CommandAuthority::ServerIntent,
    }
}

fn manifest_error_to_diagnostic(
    error: ManifestValidationError,
    context: &CommandDiagnosticContext,
) -> CommandDiagnostic {
    match error {
        ManifestValidationError::DuplicateCommandId { command_id } => CommandDiagnosticContext {
            command_id: Some(command_id),
            ..context.clone()
        }
        .diagnostic(
            CommandValidationRule::DuplicateCommandId,
            "behavior manifest contribution contains a duplicate command ID",
        ),
        ManifestValidationError::AmbiguousKeyBinding { command_id } => CommandDiagnosticContext {
            command_id: Some(command_id),
            ..context.clone()
        }
        .diagnostic(
            CommandValidationRule::AmbiguousKeyBinding,
            "behavior manifest contribution contains an ambiguous key binding",
        ),
        other => context.diagnostic(
            CommandValidationRule::InvalidBehaviorContribution,
            format!("behavior manifest contribution failed validation: {other:?}"),
        ),
    }
}

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

#[derive(Clone)]
struct CommandDiagnosticContext {
    package_name: Option<String>,
    package_version: Option<String>,
    api_prefix: Option<String>,
    command_id: Option<String>,
}

impl CommandDiagnosticContext {
    fn from_declaration(declaration: &PackageCommandDeclaration) -> Self {
        Self {
            package_name: Some(declaration.package_name.clone()),
            package_version: Some(declaration.package_version.clone()),
            api_prefix: Some(declaration.api_prefix.clone()),
            command_id: Some(declaration.command_id.clone()),
        }
    }

    fn from_contribution(contribution: &PackageBehaviorContribution) -> Self {
        Self {
            package_name: Some(contribution.package_name.clone()),
            package_version: Some(contribution.package_version.clone()),
            api_prefix: Some(contribution.api_prefix.clone()),
            command_id: None,
        }
    }

    fn diagnostic(
        &self,
        rule: CommandValidationRule,
        message: impl Into<Box<str>>,
    ) -> CommandDiagnostic {
        CommandDiagnostic {
            package_name: self.package_name.clone().map(String::into_boxed_str),
            package_version: self.package_version.clone().map(String::into_boxed_str),
            api_prefix: self.api_prefix.clone().map(String::into_boxed_str),
            command_id: self.command_id.clone().map(String::into_boxed_str),
            rule,
            message: message.into(),
        }
    }
}
