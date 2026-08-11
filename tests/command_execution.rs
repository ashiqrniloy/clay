use clay::packages::commands::{CommandRegistry, CommandValidationRule, PackageCommandDeclaration};
use clay::packages::manifest::validate_manifest_value;
use clay::packages::permissions::PackagePermission;
use clay::protocol::{
    KeyBindingContext, KeyBindingRule, KeyCode, KeyModifiers, LockScope, RoutingPolicy,
};
use clay::server::command_execution::{
    CommandExecutionProvenance, CommandExecutionRequest, CommandExecutionRule,
    CommandExecutionStatus, CommandExecutionTarget, CommandExecutor,
};
use serde_json::json;
use std::collections::BTreeMap;

fn markdown_manifest() -> clay::packages::manifest::ClayPackageManifest {
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
    routing_policy: RoutingPolicy,
    permissions: Vec<PackagePermission>,
) {
    let manifest = markdown_manifest();
    registry
        .register_command(
            &manifest,
            PackageCommandDeclaration {
                package_name: "@clay/markdown".to_string(),
                package_version: "0.1.0".to_string(),
                api_prefix: "markdown".to_string(),
                command_id: command_id.to_string(),
                display_name: "Test Command".to_string(),
                routing_policy,
                key_bindings: vec![KeyBindingRule::single(
                    command_id,
                    clay::protocol::KeyCode::Character("x".to_string()),
                )],
                custom_properties: BTreeMap::new(),
                permissions,
            },
        )
        .expect("register command");
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
fn registered_server_command_executes_with_accepted_status() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.togglePreview",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let result = CommandExecutor::new()
        .execute(&registry, request("markdown.togglePreview"))
        .expect("execute command");

    assert_eq!(result.command_id, "markdown.togglePreview");
    assert_eq!(result.status, CommandExecutionStatus::Accepted);
}

#[test]
fn reload_command_is_server_first_behavior_locked_and_discoverable() {
    let command =
        clay::server::command_execution::builtin_server_command("runtime.reloadConfiguration")
            .expect("reload command is built in");

    assert_eq!(command.display_name, "Reload Configuration and Packages");
    assert_eq!(
        command.routing_policy,
        RoutingPolicy::ServerFirstWithLock {
            lock_scope: LockScope::Behavior,
        }
    );
    assert_eq!(command.key_bindings.len(), 1);
    let binding = &command.key_bindings[0];
    assert_eq!(binding.command_id, "runtime.reloadConfiguration");
    assert_eq!(binding.context, KeyBindingContext::Global);
    assert_eq!(binding.sequence[0].key, KeyCode::Character("r".to_string()));
    assert_eq!(
        binding.sequence[0].modifiers,
        KeyModifiers {
            control: true,
            shift: true,
            ..KeyModifiers::NONE
        }
    );
    assert!(command.permissions.is_empty());
    assert!(
        clay::server::command_execution::builtin_server_command_ids()
            .contains(&"runtime.reloadConfiguration")
    );

    let result = CommandExecutor::new()
        .execute(
            &CommandRegistry::new(),
            CommandExecutionRequest {
                command_id: command.command_id,
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
        )
        .expect("reload command validates through shared executor");
    assert_eq!(result.status, CommandExecutionStatus::Accepted);
}

#[test]
fn unknown_command_is_rejected_before_any_side_effect() {
    let registry = CommandRegistry::new();

    let error = CommandExecutor::new()
        .execute(&registry, request("markdown.unknownCommand"))
        .expect_err("unknown command rejected");

    assert_eq!(error.rule, CommandExecutionRule::UnknownCommand);
}

#[test]
fn client_first_predictable_command_is_rejected_as_invalid_routing() {
    let mut registry = CommandRegistry::new();
    let command = clay::packages::commands::RegisteredCommand {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        command_id: "markdown.clientEdit".to_string(),
        display_name: "Client Edit".to_string(),
        routing_policy: RoutingPolicy::ClientFirstPredictable,
        key_bindings: vec![],
        custom_properties: BTreeMap::new(),
        permissions: vec![PackagePermission::ParseDocument],
    };
    registry.insert_test_command(command);

    let error = CommandExecutor::new()
        .execute(&registry, request("markdown.clientEdit"))
        .expect_err("client-first command rejected");

    assert_eq!(error.rule, CommandExecutionRule::InvalidRoutingPolicy);
}

#[test]
fn client_ui_command_is_rejected_as_invalid_routing() {
    let mut registry = CommandRegistry::new();
    let command = clay::packages::commands::RegisteredCommand {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        command_id: "markdown.clientUi".to_string(),
        display_name: "Client UI".to_string(),
        routing_policy: RoutingPolicy::ClientUiCommand,
        key_bindings: vec![],
        custom_properties: BTreeMap::new(),
        permissions: vec![PackagePermission::ParseDocument],
    };
    registry.insert_test_command(command);

    let error = CommandExecutor::new()
        .execute(&registry, request("markdown.clientUi"))
        .expect_err("client-ui command rejected");

    assert_eq!(error.rule, CommandExecutionRule::InvalidRoutingPolicy);
}

#[test]
fn mismatched_provenance_is_rejected() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.togglePreview",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let mut bad_request = request("markdown.togglePreview");
    bad_request.provenance = Some(CommandExecutionProvenance {
        package_name: "@clay/other".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "other".to_string(),
    });

    let error = CommandExecutor::new()
        .execute(&registry, bad_request)
        .expect_err("mismatched provenance rejected");

    assert_eq!(error.rule, CommandExecutionRule::InvalidProvenance);
}

#[test]
fn undeclared_expected_permission_is_rejected() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.togglePreview",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let mut bad_request = request("markdown.togglePreview");
    bad_request.expected_permissions = vec![PackagePermission::WorkspaceMutation];

    let error = CommandExecutor::new()
        .execute(&registry, bad_request)
        .expect_err("undeclared permission rejected");

    assert_eq!(error.rule, CommandExecutionRule::UndeclaredPermission);
}

#[test]
fn malformed_arguments_are_rejected() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.togglePreview",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let mut bad_request = request("markdown.togglePreview");
    bad_request.arguments = json!("not an object");

    let error = CommandExecutor::new()
        .execute(&registry, bad_request)
        .expect_err("malformed arguments rejected");

    assert_eq!(error.rule, CommandExecutionRule::InvalidArguments);
}

#[test]
fn oversize_arguments_are_rejected_before_heavy_work() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.togglePreview",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let mut bad_request = request("markdown.togglePreview");
    bad_request.arguments = json!({ "text": "x".repeat(8 * 1024) });

    let error = CommandExecutor::new()
        .execute(&registry, bad_request)
        .expect_err("oversize arguments rejected");

    assert_eq!(error.rule, CommandExecutionRule::InvalidArguments);
    assert!(
        error.message.contains("budget"),
        "diagnostic should mention payload budget: {}",
        error.message
    );
}

#[test]
fn invalid_active_document_target_is_rejected() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.togglePreview",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let mut bad_request = request("markdown.togglePreview");
    bad_request.target = CommandExecutionTarget::ActiveDocument { document_id: 0 };

    let error = CommandExecutor::new()
        .execute(&registry, bad_request)
        .expect_err("invalid document target rejected");

    assert_eq!(error.rule, CommandExecutionRule::UnauthorizedTarget);
}

#[test]
fn workspace_target_requires_workspace_mutation_permission() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.workspaceCommand",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let mut bad_request = request("markdown.workspaceCommand");
    bad_request.target = CommandExecutionTarget::Workspace;

    let error = CommandExecutor::new()
        .execute(&registry, bad_request)
        .expect_err("workspace target without permission rejected");

    assert_eq!(error.rule, CommandExecutionRule::UnauthorizedTarget);

    let mut allowed_registry = CommandRegistry::new();
    let allowed_command = clay::packages::commands::RegisteredCommand {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        command_id: "markdown.workspaceAllowed".to_string(),
        display_name: "Workspace Allowed".to_string(),
        routing_policy: RoutingPolicy::ServerFirst,
        key_bindings: vec![],
        custom_properties: BTreeMap::new(),
        permissions: vec![PackagePermission::WorkspaceMutation],
    };
    allowed_registry.insert_test_command(allowed_command);

    let mut allowed_request = request("markdown.workspaceAllowed");
    allowed_request.target = CommandExecutionTarget::Workspace;
    allowed_request.expected_permissions = vec![PackagePermission::WorkspaceMutation];

    let result = CommandExecutor::new()
        .execute(&allowed_registry, allowed_request)
        .expect("workspace command with permission accepted");
    assert_eq!(result.status, CommandExecutionStatus::Accepted);
}

#[test]
fn duplicate_command_id_rejected_at_registration() {
    let mut registry = CommandRegistry::new();
    register_command(
        &mut registry,
        "markdown.togglePreview",
        RoutingPolicy::ServerFirst,
        vec![PackagePermission::ParseDocument],
    );

    let manifest = markdown_manifest();
    let duplicate = PackageCommandDeclaration {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        command_id: "markdown.togglePreview".to_string(),
        display_name: "Duplicate".to_string(),
        routing_policy: RoutingPolicy::ServerFirst,
        key_bindings: vec![],
        custom_properties: BTreeMap::new(),
        permissions: vec![PackagePermission::ParseDocument],
    };

    let error = registry
        .register_command(&manifest, duplicate)
        .expect_err("duplicate command rejected");

    assert_eq!(error.rule, CommandValidationRule::DuplicateCommandId);
}

// ── Phase 18.9 Task 6: mode discovery/listing commands ──

use clay::packages::modes::{
    DocumentClassificationInput, ModePatternKind, ModeProvenance, ModeRegistry,
};
use clay::server::command_execution::DiscoveryResult;

fn discovery_request(command_id: &str, arguments: serde_json::Value) -> CommandExecutionRequest {
    CommandExecutionRequest {
        command_id: command_id.to_string(),
        arguments,
        target: CommandExecutionTarget::Global,
        provenance: None,
        expected_permissions: Vec::new(),
    }
}

/// Register a markdown package mode claiming `.md`, then classify+activate it
/// for `note.md` so discovery has a package-owned active mode to contrast
/// with the built-in `core.code` fallback (no language package).
fn registry_with_package_md_mode(document_id: u64) -> ModeRegistry {
    let manifest = validate_manifest_value(&json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "clay": {
            "apiPrefix": "markdown",
            "permissions": ["mode-registration", "mode-activation"],
            "modes": ["markdown"],
            "entry": "./dist/index.js"
        }
    }))
    .expect("valid package manifest");
    let mut registry = ModeRegistry::new();
    let decl = clay::packages::modes::ModeDeclaration {
        package_name: manifest.name.clone(),
        package_version: manifest.version.clone(),
        api_prefix: manifest.clay.api_prefix.clone(),
        mode_id: "markdown".to_string(),
        display_name: "Markdown".to_string(),
        document_font_role: clay::protocol::DocumentFontRole::Proportional,
        extensions: vec!["md".to_string()],
        mime_types: vec![],
        file_names: vec![],
        file_name_patterns: vec![],
        shebang_patterns: vec![],
        content_probes: vec![],
    };
    registry
        .register_mode(&manifest, decl)
        .expect("register package mode");
    let input = DocumentClassificationInput {
        document_id,
        path: Some("note.md".to_string()),
        mime_type: None,
        shebang: None,
        leading_content: None,
    };
    let classification = registry
        .classify(&input)
        .expect("md classifies to package mode");
    registry
        .activate_major_mode(&manifest, classification)
        .expect("activate package major mode");
    registry
}

#[test]
fn explain_active_mode_returns_core_code_fallback_rationale_without_language_package() {
    let mut registry = ModeRegistry::new();
    // A rust file with no language package installed: core.code claims the .rs
    // extension as a built-in fallback.
    let input = DocumentClassificationInput {
        document_id: 7,
        path: Some("script.rs".to_string()),
        mime_type: None,
        shebang: None,
        leading_content: None,
    };
    let classification = registry.classify(&input).expect("core.code classifies .rs");
    assert_eq!(classification.mode_id, "core.code");
    registry
        .activate_builtin_major_mode(classification)
        .expect("activate built-in core.code");

    let result = CommandExecutor::new()
        .execute_discovery(
            &registry,
            discovery_request("modes.explainActiveMode", json!({ "documentId": 7 })),
        )
        .expect("explainActiveMode resolves");

    let CommandExecutionStatus::Discovery(DiscoveryResult::ModeExplanation(Some(explanation))) =
        result.status
    else {
        panic!(
            "expected resolved ModeExplanation payload, got {:?}",
            result.status
        );
    };
    assert_eq!(explanation.document_id, 7);
    assert_eq!(explanation.active_mode, "core.code");
    assert_eq!(explanation.provenance, ModeProvenance::CoreBuiltIn);
    assert_eq!(
        explanation.classification_source,
        ModePatternKind::Extension
    );
    assert!(
        !explanation.fallback_used,
        "core.code via extension is not the universal fallback"
    );
    // The rationale must explain that no language package matched and that the
    // built-in core.code claimed the document, naming the matched signal.
    assert!(
        explanation.why.contains("no language package matched"),
        "rationale should mention no language package matched: {}",
        explanation.why
    );
    assert!(
        explanation.why.contains("built-in core.code"),
        "rationale should name built-in core.code: {}",
        explanation.why
    );
    assert!(
        explanation.why.contains("extension"),
        "rationale should name the classification signal: {}",
        explanation.why
    );
}

#[test]
fn explain_active_mode_reports_core_text_universal_fallback_for_plain_text() {
    let mut registry = ModeRegistry::new();
    let input = DocumentClassificationInput {
        document_id: 9,
        path: Some("README.txt".to_string()),
        mime_type: None,
        shebang: None,
        leading_content: None,
    };
    let classification = registry
        .classify(&input)
        .expect("core.text universal fallback");
    assert_eq!(classification.mode_id, "core.text");
    registry
        .activate_builtin_major_mode(classification)
        .expect("activate built-in core.text");

    let result = CommandExecutor::new()
        .execute_discovery(
            &registry,
            discovery_request("modes.explainActiveMode", json!({ "documentId": 9 })),
        )
        .expect("explainActiveMode resolves");

    let CommandExecutionStatus::Discovery(DiscoveryResult::ModeExplanation(Some(explanation))) =
        result.status
    else {
        panic!("expected resolved ModeExplanation payload");
    };
    assert_eq!(explanation.active_mode, "core.text");
    assert_eq!(explanation.provenance, ModeProvenance::CoreBuiltIn);
    assert_eq!(explanation.classification_source, ModePatternKind::Fallback);
    assert!(explanation.fallback_used);
    assert!(explanation.why.contains("core.text"));
    assert!(explanation.why.contains("fallback"));
}

#[test]
fn list_active_modes_reports_package_and_built_in_provenance_with_classification_source() {
    // One package-owned mode (markdown.markdown via extension) plus one
    // built-in core.code fallback (rust file, no language package).
    let mut registry = registry_with_package_md_mode(11);
    let input = DocumentClassificationInput {
        document_id: 12,
        path: Some("main.rs".to_string()),
        mime_type: None,
        shebang: None,
        leading_content: None,
    };
    let classification = registry.classify(&input).expect("core.code classifies .rs");
    registry
        .activate_builtin_major_mode(classification)
        .expect("activate built-in core.code");

    let result = CommandExecutor::new()
        .execute_discovery(
            &registry,
            discovery_request("modes.listActiveModes", serde_json::Value::Null),
        )
        .expect("listActiveModes resolves");

    let CommandExecutionStatus::Discovery(DiscoveryResult::ActiveModes(entries)) = result.status
    else {
        panic!("expected resolved ActiveModes payload");
    };
    assert_eq!(entries.len(), 2, "two documents have active modes");

    let by_doc: std::collections::HashMap<u64, _> =
        entries.iter().map(|e| (e.document_id, e.clone())).collect();

    let md = by_doc.get(&11).expect("markdown document listed");
    assert_eq!(md.mode_id, "markdown");
    assert_eq!(md.provenance, ModeProvenance::Package);
    assert_eq!(md.classification_source, ModePatternKind::Extension);

    let rs = by_doc.get(&12).expect("rust document listed");
    assert_eq!(rs.mode_id, "core.code");
    assert_eq!(rs.provenance, ModeProvenance::CoreBuiltIn);
    assert_eq!(rs.classification_source, ModePatternKind::Extension);
}

#[test]
fn explain_active_mode_for_unknown_document_returns_none_explanation() {
    let registry = ModeRegistry::new(); // no documents activated

    let result = CommandExecutor::new()
        .execute_discovery(
            &registry,
            discovery_request("modes.explainActiveMode", json!({ "documentId": 404 })),
        )
        .expect("explainActiveMode resolves for unknown document");

    let CommandExecutionStatus::Discovery(DiscoveryResult::ModeExplanation(None)) = result.status
    else {
        panic!("expected None explanation for unknown document");
    };
}

#[test]
fn discovery_commands_are_reachable_from_control_center_listing() {
    // Built-in discovery command IDs appear in the built-in command list so the
    // Control Center surfaces them (reachable through the Phase 18.8 command
    // execution path). They are server-first with no permissions.
    let ids = clay::server::command_execution::builtin_server_command_ids();
    assert!(ids.contains(&"modes.listActiveModes"));
    assert!(ids.contains(&"modes.explainActiveMode"));

    for command_id in ["modes.listActiveModes", "modes.explainActiveMode"] {
        let command = clay::server::command_execution::builtin_server_command(command_id)
            .expect("discovery command is built-in");
        assert_eq!(command.routing_policy, RoutingPolicy::ServerFirst);
        assert!(
            command.permissions.is_empty(),
            "discovery commands carry no permissions/authority"
        );
    }
}

#[test]
fn discovery_commands_reject_no_authority_violations() {
    let registry = ModeRegistry::new();

    // explainActiveMode requires a non-negative integer documentId argument;
    // missing/malformed arguments are rejected via the shared validation path.
    let err = CommandExecutor::new()
        .execute_discovery(
            &registry,
            discovery_request("modes.explainActiveMode", json!({ "notDocumentId": 1 })),
        )
        .expect_err("missing documentId rejected");
    assert_eq!(err.rule, CommandExecutionRule::InvalidArguments);

    // A workspace target is not authorized for a discovery command (no
    // workspace-mutation permission declared): the shared target validator
    // rejects it the same as any other command.
    let mut workspace_request =
        discovery_request("modes.explainActiveMode", json!({ "documentId": 1 }));
    workspace_request.target = CommandExecutionTarget::Workspace;
    let err = CommandExecutor::new()
        .execute_discovery(&registry, workspace_request)
        .expect_err("workspace target rejected for discovery command");
    assert_eq!(err.rule, CommandExecutionRule::UnauthorizedTarget);

    // A non-discovery built-in command cannot be resolved through the
    // discovery entry point: rejected as UnknownCommand.
    let err = CommandExecutor::new()
        .execute_discovery(
            &registry,
            discovery_request("workspace.refresh", serde_json::Value::Null),
        )
        .expect_err("non-discovery command rejected by discovery path");
    assert_eq!(err.rule, CommandExecutionRule::UnknownCommand);

    // A bogus command ID is rejected (cannot be resolved through discovery).
    let err = CommandExecutor::new()
        .execute_discovery(
            &registry,
            discovery_request("modes.bogus", serde_json::Value::Null),
        )
        .expect_err("unknown discovery command rejected");
    assert_eq!(err.rule, CommandExecutionRule::UnknownCommand);
}
