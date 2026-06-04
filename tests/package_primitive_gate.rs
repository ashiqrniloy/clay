use std::collections::BTreeMap;

use clay::packages::commands::{
    CommandRegistry, CommandValidationRule, PackageBehaviorContribution, PackageCommandDeclaration,
    PackageTextTransformDeclaration, TextTransformKind,
};
use clay::packages::manifest::{
    PackageValidationRule, validate_manifest_value, validate_manifest_values,
};
use clay::packages::modes::{
    DocumentClassificationInput, ModeDeclaration, ModePatternKind, ModeRegistry, ModeValidationRule,
};
use clay::packages::permissions::PackagePermission;
use clay::protocol::{
    BehaviorManifest, BehaviorScope, KeyBindingContext, KeyBindingRule, KeyCode, KeyModifiers,
    KeyStroke, RoutingPolicy,
};
use serde_json::{Value, json};

fn markdown_fixture() -> Value {
    json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "clay": {
            "apiPrefix": "markdown",
            "permissions": ["mode-registration", "mode-activation"],
            "modes": ["markdown"],
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js"
        }
    })
}

fn command_fixture() -> Value {
    let mut fixture = markdown_fixture();
    fixture["clay"]["permissions"] = json!([
        "mode-registration",
        "mode-activation",
        "command-registration",
        "parse-document"
    ]);
    fixture
}

fn markdown_command_declaration(command_id: &str) -> PackageCommandDeclaration {
    PackageCommandDeclaration {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        command_id: command_id.to_string(),
        display_name: "Toggle Markdown Preview".to_string(),
        routing_policy: RoutingPolicy::ServerFirst,
        key_bindings: vec![KeyBindingRule {
            command_id: command_id.to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("p".to_string()),
                modifiers: KeyModifiers {
                    shift: false,
                    control: true,
                    alt: false,
                    super_key: false,
                },
            }],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ServerFirst,
        }],
        custom_properties: BTreeMap::from([("category".to_string(), "Markdown".to_string())]),
        permissions: vec![PackagePermission::ParseDocument],
    }
}

fn markdown_behavior_contribution(
    commands: Vec<PackageCommandDeclaration>,
) -> PackageBehaviorContribution {
    PackageBehaviorContribution {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        manifest_id: "markdown.behavior".to_string(),
        behavior_version: 1,
        scope: BehaviorScope::Language {
            language_id: "markdown".to_string(),
        },
        commands,
        keymaps: Vec::new(),
        editor_rules: BehaviorManifest::minimal_text_editing(1).editor_rules,
        text_transforms: vec![PackageTextTransformDeclaration {
            transform_id: "markdown.list-continuation".to_string(),
            kind: TextTransformKind::EnterRule,
            javascript_callback: None,
            code: None,
        }],
    }
}

fn markdown_mode_declaration() -> ModeDeclaration {
    ModeDeclaration {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        display_name: "Markdown".to_string(),
        extensions: vec![
            "md".to_string(),
            "markdown".to_string(),
            "mdown".to_string(),
        ],
        mime_types: vec!["text/markdown".to_string()],
        file_names: Vec::new(),
        file_name_patterns: Vec::new(),
    }
}

#[test]
fn package_manifest_accepts_minimal_markdown_fixture() {
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");

    assert_eq!(manifest.name, "@clay/markdown");
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.clay.api_prefix, "markdown");
    assert_eq!(
        manifest.clay.permissions,
        vec![
            PackagePermission::ModeRegistration,
            PackagePermission::ModeActivation
        ]
    );
    assert_eq!(manifest.clay.modes, vec!["markdown".to_string()]);
    assert_eq!(manifest.clay.entry, "./dist/index.js");
    assert_eq!(manifest.clay.load_entry.as_deref(), Some("./dist/load.js"));
}

#[test]
fn package_manifest_rejects_invalid_prefix_and_reserved_clay_ids() {
    let mut invalid_prefix = markdown_fixture();
    invalid_prefix["clay"]["apiPrefix"] = json!("Markdown");

    let error = validate_manifest_value(&invalid_prefix).unwrap_err();
    assert_eq!(error.package_name.as_deref(), Some("@clay/markdown"));
    assert_eq!(error.package_version.as_deref(), Some("0.1.0"));
    assert_eq!(error.api_prefix.as_deref(), Some("Markdown"));
    assert_eq!(error.rule, PackageValidationRule::InvalidPrefix);

    let mut reserved_id = markdown_fixture();
    reserved_id["clay"]["modes"] = json!(["clay.markdown"]);

    let error = validate_manifest_value(&reserved_id).unwrap_err();
    assert_eq!(error.rule, PackageValidationRule::ReservedClayId);
    assert!(error.message.contains("clay.*"));
}

#[test]
fn package_permissions_reject_unknown_or_prohibited_authority() {
    let mut unknown = markdown_fixture();
    unknown["clay"]["permissions"] = json!(["mode-registration", "telepathy"]);

    let error = validate_manifest_value(&unknown).unwrap_err();
    assert_eq!(error.rule, PackageValidationRule::UnknownPermission);
    assert_eq!(error.api_prefix.as_deref(), Some("markdown"));

    let mut prohibited = markdown_fixture();
    prohibited["clay"]["permissions"] = json!(["mode-registration", "network"]);

    let error = validate_manifest_value(&prohibited).unwrap_err();
    assert_eq!(error.rule, PackageValidationRule::ProhibitedAuthority);
    assert!(error.message.contains("network"));
}

#[test]
fn package_manifest_rejects_duplicate_prefixes_raw_ops_and_client_hooks() {
    let mut second = markdown_fixture();
    second["name"] = json!("@clay/markdown-alt");
    second["version"] = json!("0.2.0");

    let error = validate_manifest_values(&[markdown_fixture(), second]).unwrap_err();
    assert_eq!(error.rule, PackageValidationRule::DuplicatePrefix);
    assert_eq!(error.api_prefix.as_deref(), Some("markdown"));

    let mut raw_ops = markdown_fixture();
    raw_ops["clay"]["facade"] = json!("Deno.core.ops.op_secret");
    let error = validate_manifest_value(&raw_ops).unwrap_err();
    assert_eq!(error.rule, PackageValidationRule::RawDenoOpsExposure);

    let mut client_hook = markdown_fixture();
    client_hook["clay"]["clientJavaScript"] = json!("window.alert('nope')");
    let error = validate_manifest_value(&client_hook).unwrap_err();
    assert_eq!(error.rule, PackageValidationRule::ClientJavaScriptHook);
}

#[test]
fn mode_registry_classifies_markdown_extension() {
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(&manifest, markdown_mode_declaration())
        .expect("mode registration succeeds");

    let classification = registry
        .classify(&DocumentClassificationInput {
            document_id: 42,
            path: Some("notes/README.md".to_string()),
            mime_type: None,
        })
        .expect("markdown extension classifies");

    assert_eq!(classification.document_id, 42);
    assert_eq!(classification.package_name, "@clay/markdown");
    assert_eq!(classification.api_prefix, "markdown");
    assert_eq!(classification.mode_id, "markdown");
    assert_eq!(classification.matched_by, ModePatternKind::Extension);
    assert_eq!(registry.activation_budget_ms(), 100);
}

#[test]
fn mode_registry_rejects_duplicate_mode_name_and_malformed_patterns() {
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(&manifest, markdown_mode_declaration())
        .expect("first mode registration succeeds");

    let error = registry
        .register_mode(&manifest, markdown_mode_declaration())
        .unwrap_err();
    assert_eq!(error.rule, ModeValidationRule::DuplicateModeId);
    assert_eq!(error.package_name.as_deref(), Some("@clay/markdown"));
    assert_eq!(error.mode_id.as_deref(), Some("markdown"));

    let mut malformed = markdown_mode_declaration();
    malformed.mode_id = "markdown.preview".to_string();
    malformed.extensions = vec![".md".to_string()];
    let mut manifest_with_preview = markdown_fixture();
    manifest_with_preview["clay"]["modes"] = json!(["markdown", "markdown.preview"]);
    let manifest_with_preview =
        validate_manifest_value(&manifest_with_preview).expect("valid two-mode fixture");
    let error = ModeRegistry::new()
        .register_mode(&manifest_with_preview, malformed)
        .unwrap_err();
    assert_eq!(error.rule, ModeValidationRule::MalformedPattern);
}

#[test]
fn mode_activation_keeps_one_major_mode_per_document() {
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(&manifest, markdown_mode_declaration())
        .expect("mode registration succeeds");
    let classification = registry
        .classify(&DocumentClassificationInput {
            document_id: 7,
            path: Some("README.md".to_string()),
            mime_type: Some("text/markdown".to_string()),
        })
        .expect("markdown document classifies");

    let first = registry
        .activate_major_mode(&manifest, classification.clone())
        .expect("first activation succeeds");
    let second = registry
        .activate_major_mode(&manifest, classification)
        .expect("second activation replaces active major mode deterministically");

    assert_eq!(first.document_id, 7);
    assert_eq!(first.mode_id, "markdown");
    assert_eq!(first.behavior_version, 1);
    assert_eq!(second.behavior_version, 2);
    assert_eq!(registry.active_major_mode(7), Some(&second));
}

#[test]
fn mode_registration_and_activation_require_declared_authority() {
    let mut fixture = markdown_fixture();
    fixture["clay"]["permissions"] = json!(["mode-activation"]);
    let registration_only_missing = validate_manifest_value(&fixture).expect("valid fixture");
    let error = ModeRegistry::new()
        .register_mode(&registration_only_missing, markdown_mode_declaration())
        .unwrap_err();
    assert_eq!(error.rule, ModeValidationRule::MissingPermission);

    let mut fixture = markdown_fixture();
    fixture["clay"]["permissions"] = json!(["mode-registration"]);
    let activation_missing = validate_manifest_value(&fixture).expect("valid fixture");
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(&activation_missing, markdown_mode_declaration())
        .expect("registration permission present");
    let classification = registry
        .classify(&DocumentClassificationInput {
            document_id: 9,
            path: Some("README.md".to_string()),
            mime_type: None,
        })
        .expect("classification succeeds");
    let error = registry
        .activate_major_mode(&activation_missing, classification)
        .unwrap_err();
    assert_eq!(error.rule, ModeValidationRule::MissingPermission);
}

#[test]
fn package_command_registry_rejects_duplicate_command_id() {
    let manifest = validate_manifest_value(&command_fixture()).expect("valid command fixture");
    let mut registry = CommandRegistry::new();
    let command = markdown_command_declaration("markdown.togglePreview");
    let registered = registry
        .register_command(&manifest, command.clone())
        .expect("first command registration succeeds");

    assert_eq!(registered.package_name, "@clay/markdown");
    assert_eq!(registered.api_prefix, "markdown");
    assert_eq!(registered.command_id, "markdown.togglePreview");
    assert_eq!(
        registered.permissions,
        vec![PackagePermission::ParseDocument]
    );
    assert_eq!(registry.keypress_to_local_paint_budget_ms(), 16);

    let error = registry.register_command(&manifest, command).unwrap_err();
    assert_eq!(error.rule, CommandValidationRule::DuplicateCommandId);
    assert_eq!(error.command_id.as_deref(), Some("markdown.togglePreview"));
}

#[test]
fn package_keybindings_reject_ambiguous_bindings() {
    let manifest = validate_manifest_value(&command_fixture()).expect("valid command fixture");
    let mut first = markdown_command_declaration("markdown.togglePreview");
    let mut second = markdown_command_declaration("markdown.openPreview");
    second.display_name = "Open Markdown Preview".to_string();
    second.permissions = Vec::new();
    second.key_bindings[0].command_id = second.command_id.clone();
    first.key_bindings[0].sequence = vec![KeyStroke::new(KeyCode::Enter)];
    second.key_bindings[0].sequence = vec![KeyStroke::new(KeyCode::Enter)];

    let contribution = markdown_behavior_contribution(vec![first, second]);
    let error = CommandRegistry::new()
        .validate_behavior_contribution(&manifest, contribution)
        .unwrap_err();

    assert_eq!(error.rule, CommandValidationRule::AmbiguousKeyBinding);
}

#[test]
fn package_text_transforms_are_inert_manifest_data() {
    let manifest = validate_manifest_value(&command_fixture()).expect("valid command fixture");
    let command = markdown_command_declaration("markdown.togglePreview");
    let contribution = markdown_behavior_contribution(vec![command]);
    let manifest_candidate = CommandRegistry::new()
        .validate_behavior_contribution(&manifest, contribution)
        .expect("inert behavior contribution validates");
    assert_eq!(manifest_candidate.manifest_id, "markdown.behavior");
    assert!(
        manifest_candidate
            .commands
            .iter()
            .any(|command| command.command_id == "markdown.togglePreview")
    );

    let command = markdown_command_declaration("markdown.togglePreview");
    let mut contribution = markdown_behavior_contribution(vec![command]);
    contribution.text_transforms[0].javascript_callback = Some("() => true".to_string());
    let error = CommandRegistry::new()
        .validate_behavior_contribution(&manifest, contribution)
        .unwrap_err();
    assert_eq!(error.rule, CommandValidationRule::ExecutableTextTransform);
    assert!(error.message.contains("cannot include JavaScript"));
}
