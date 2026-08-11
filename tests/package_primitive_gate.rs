use std::collections::BTreeMap;

use clay::packages::commands::{
    CommandRegistry, CommandValidationRule, PackageBehaviorContribution, PackageCommandDeclaration,
    PackageTextTransformDeclaration, TextTransformKind,
};
use clay::packages::manifest::{
    PackageValidationRule, validate_manifest_value, validate_manifest_values,
};
use clay::packages::modes::{
    DocumentClassificationInput, MAX_LEADING_CONTENT_BYTES, ModeDeclaration, ModePatternKind,
    ModeRegistry, ModeValidationRule, core_code_mode, core_text_mode,
};
use clay::packages::permissions::PackagePermission;
use clay::packages::record::{PackageRecordRule, assemble_package_record};
use clay::perf::budgets::BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES;
use clay::protocol::{
    BehaviorManifest, BehaviorScope, KeyBindingContext, KeyBindingRule, KeyCode, KeyModifiers,
    KeyStroke, RoutingPolicy, ServerMessage, codec::Codec,
};
use serde_json::{Value, json};

/// Encoded server-frame length minus the 4-byte length prefix, matching the
/// payload budget enforcement used by the markdown manifest budget test.
fn protocol_payload_len(frame: &[u8]) -> usize {
    frame.len().saturating_sub(4)
}

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

fn completion_provider_fixture() -> Value {
    json!({
        "name": "@vendor/words",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "words",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "permissions": ["completion-provider"],
            "modes": [],
            "docs": "./docs/index.md",
            "contributions": {
                "completionProviders": [{
                    "id": "words.buffer",
                    "priority": 10,
                    "triggerCharacters": ["."],
                    "wordBoundaryChars": [".", ","],
                    "items": ["alpha", "await"],
                    "budgets": { "timeoutMs": 50, "maxItems": 32 }
                }]
            }
        }
    })
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
        document_font_role: clay::protocol::DocumentFontRole::Proportional,
        extensions: vec![
            "md".to_string(),
            "markdown".to_string(),
            "mdown".to_string(),
        ],
        mime_types: vec!["text/markdown".to_string()],
        file_names: Vec::new(),
        file_name_patterns: Vec::new(),
        shebang_patterns: Vec::new(),
        content_probes: Vec::new(),
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

/// Third-party packages must not claim a reserved core API domain (e.g.
/// `shell`, `editor`) as their `clay.apiPrefix` — core IDs are bare
/// `<domain>.<name>` and a squatted domain would make core and package IDs
/// indistinguishable. Bundled first-party packages from the compiled
/// inventory are exempt (`@clay/git` owns the `git` domain).
#[test]
fn package_manifest_rejects_third_party_api_prefix_squatting_core_domain() {
    let mut squatter = markdown_fixture();
    squatter["name"] = json!("@vendor/shell");
    squatter["clay"]["apiPrefix"] = json!("shell");
    let error = validate_manifest_value(&squatter).unwrap_err();
    assert_eq!(error.rule, PackageValidationRule::InvalidPrefix);
    assert!(error.message.contains("reserved Clay core API domain"));

    // Bundled first-party package keeps its core-domain prefix.
    let mut bundled_git = markdown_fixture();
    bundled_git["name"] = json!("@clay/git");
    bundled_git["clay"]["apiPrefix"] = json!("git");
    bundled_git["clay"]["modes"] = json!([]);
    bundled_git["clay"]["permissions"] = json!([]);
    validate_manifest_value(&bundled_git).expect("bundled @clay/git keeps the git domain");
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
fn completion_provider_contributions_require_permission_and_inert_metadata() {
    let valid = assemble_package_record(&completion_provider_fixture())
        .expect("completion provider metadata fixture validates");
    assert_eq!(valid.contributions.completion_providers.len(), 1);
    assert_eq!(
        valid.contributions.completion_providers[0].id,
        "words.buffer"
    );
    assert_eq!(
        valid.contributions.completion_providers[0]
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "await"]
    );

    let mut structured = completion_provider_fixture();
    structured["clay"]["contributions"]["completionProviders"][0]["items"] = json!([{
        "label": "fn",
        "insertText": "fn ${1:name}() {$0}",
        "detail": "function snippet",
        "textFormat": "snippet"
    }]);
    let structured = assemble_package_record(&structured).unwrap();
    let item = &structured.contributions.completion_providers[0].items[0];
    assert_eq!(item.label, "fn");
    assert_eq!(item.insert_text, "fn ${1:name}() {$0}");
    assert_eq!(item.detail, "function snippet");
    assert_eq!(
        item.text_format,
        clay::protocol::CompletionItemTextFormat::Snippet
    );

    let mut missing_permission = completion_provider_fixture();
    missing_permission["clay"]["permissions"] = json!([]);
    let error = assemble_package_record(&missing_permission).unwrap_err();
    assert_eq!(
        error.rule,
        PackageRecordRule::UndeclaredPermissionForContribution
    );

    for prohibited in [
        ("rawOps", json!(["op_secret"])),
        ("command", json!("workspace.delete")),
        ("snippet", json!("${1:run}")),
        ("shellCommand", json!("rm -rf .")),
        ("downloadUrl", json!("https://example.invalid/provider.js")),
        ("clientJavaScript", json!("window.alert(1)")),
    ] {
        let mut fixture = completion_provider_fixture();
        fixture["clay"]["contributions"]["completionProviders"][0][prohibited.0] = prohibited.1;
        let error = assemble_package_record(&fixture).unwrap_err();
        assert!(
            matches!(
                error.rule,
                PackageRecordRule::InvalidContributionDescriptor
                    | PackageRecordRule::ManifestValidationFailed
            ),
            "field `{}` must be rejected as executable/external authority, got {:?}",
            prohibited.0,
            error.rule
        );
    }
}

#[test]
fn completion_provider_contributions_reject_conflicts_and_oversize_metadata() {
    let mut duplicate = completion_provider_fixture();
    duplicate["clay"]["contributions"]["completionProviders"] = json!([
        { "id": "words.buffer", "triggerCharacters": ["."], "wordBoundaryChars": ["."] },
        { "id": "words.buffer", "triggerCharacters": [":"], "wordBoundaryChars": [":"] }
    ]);
    let error = assemble_package_record(&duplicate).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::DuplicateContributionId);

    let mut bad_prefix = completion_provider_fixture();
    bad_prefix["clay"]["contributions"]["completionProviders"][0]["id"] = json!("other.buffer");
    let error = assemble_package_record(&bad_prefix).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);

    let mut duplicate_item = completion_provider_fixture();
    duplicate_item["clay"]["contributions"]["completionProviders"][0]["items"] =
        json!(["await", "await"]);
    let error = assemble_package_record(&duplicate_item).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::DuplicateContributionId);

    let mut mixed_formats = completion_provider_fixture();
    mixed_formats["clay"]["contributions"]["completionProviders"][0]["items"] = json!([
        "await",
        { "label": "fn", "insertText": "fn ${1:name}() {$0}", "textFormat": "snippet" }
    ]);
    let error = assemble_package_record(&mixed_formats).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(error.message.contains("separate providers"));

    let mut bad_text_format = completion_provider_fixture();
    bad_text_format["clay"]["contributions"]["completionProviders"][0]["items"] = json!([{
        "label": "fn", "insertText": "fn", "textFormat": "transform"
    }]);
    let error = assemble_package_record(&bad_text_format).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(error.message.contains("plainText` or `snippet"));

    let mut too_many_items = completion_provider_fixture();
    too_many_items["clay"]["contributions"]["completionProviders"][0]["items"] = json!(
        (0..33)
            .map(|index| format!("item{index}"))
            .collect::<Vec<_>>()
    );
    let error = assemble_package_record(&too_many_items).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);

    let mut oversized_item = completion_provider_fixture();
    oversized_item["clay"]["contributions"]["completionProviders"][0]["items"] =
        json!(["x".repeat(129)]);
    let error = assemble_package_record(&oversized_item).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);

    let mut oversized_insert_text = completion_provider_fixture();
    oversized_insert_text["clay"]["contributions"]["completionProviders"][0]["items"] = json!([{
        "label": "snippet", "insertText": "x".repeat(257), "textFormat": "snippet"
    }]);
    let error = assemble_package_record(&oversized_insert_text).unwrap_err();
    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(error.message.contains("insertText exceeds 256"));

    let mut oversize = completion_provider_fixture();
    oversize["clay"]["contributions"]["completionProviders"][0]["detail"] =
        json!("x".repeat(BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES + 1));
    let error = assemble_package_record(&oversize).unwrap_err();
    assert!(matches!(
        error.rule,
        PackageRecordRule::PayloadBudgetExceeded | PackageRecordRule::ManifestValidationFailed
    ));
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
            shebang: None,
            leading_content: None,
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
            shebang: None,
            leading_content: None,
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
            shebang: None,
            leading_content: None,
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

// ── Phase 18.9 Task 3: built-in core.text / core.code fallback modes ──────────

#[test]
fn builtin_core_modes_are_present_and_classify_with_zero_packages() {
    // `ModeRegistry::new()` registers the always-on Clay-owned built-in
    // fallback modes with no package, no init.js line, and no loadPackage.
    let registry = ModeRegistry::new();

    // core.code claims common code extensions; with no language package
    // installed, a .rs file classifies as core.code via an Extension match.
    let code = registry
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some("src/main.rs".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("code-like file classifies as core.code with no packages");
    assert_eq!(code.mode_id, "core.code");
    assert_eq!(code.api_prefix, "core");
    assert_eq!(code.package_name, "clay");
    assert_eq!(code.matched_by, ModePatternKind::Extension);

    // core.text is the universal fallback for files no pattern claims.
    let text = registry
        .classify(&DocumentClassificationInput {
            document_id: 2,
            path: Some("notes.txt".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("unknown extension falls back to core.text");
    assert_eq!(text.mode_id, "core.text");
    assert_eq!(text.matched_by, ModePatternKind::Fallback);

    let no_ext = registry
        .classify(&DocumentClassificationInput {
            document_id: 3,
            path: Some("README".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("extensionless file falls back to core.text");
    assert_eq!(no_ext.mode_id, "core.text");
    assert_eq!(no_ext.matched_by, ModePatternKind::Fallback);
}

#[test]
fn builtin_core_modes_activate_and_remain_editable_without_packages() {
    let mut registry = ModeRegistry::new();
    let classification = registry
        .classify(&DocumentClassificationInput {
            document_id: 7,
            path: Some("script.py".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("python file classifies as core.code");
    assert_eq!(classification.mode_id, "core.code");

    let activation = registry
        .activate_builtin_major_mode(classification.clone())
        .expect("built-in major mode activates without a package");
    assert_eq!(activation.document_id, 7);
    assert_eq!(activation.mode_id, "core.code");
    assert_eq!(activation.behavior_version, 1);
    assert_eq!(registry.active_major_mode(7), Some(&activation));

    // Re-activation bumps the behavior version deterministically.
    let second = registry
        .activate_builtin_major_mode(classification)
        .expect("re-activation replaces active built-in major mode");
    assert_eq!(second.behavior_version, 2);
}

#[test]
fn package_declaring_core_mode_id_is_rejected() {
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");

    for mode_id in ["core.text", "core.code", "core.foo"] {
        let mut declaration = markdown_mode_declaration();
        declaration.mode_id = mode_id.to_string();
        let error = ModeRegistry::new()
            .register_mode(&manifest, declaration)
            .unwrap_err();
        assert_eq!(
            error.rule,
            ModeValidationRule::InvalidModeId,
            "package-declared {mode_id} must be rejected"
        );
        assert_eq!(error.mode_id.as_deref(), Some(mode_id));
    }
}

#[test]
fn disabling_language_packages_yields_editable_fallback_mode() {
    // With a language package registered, its declared extension wins on ties
    // (package > built-in precedence).
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");
    let mut with_markdown = ModeRegistry::new();
    with_markdown
        .register_mode(&manifest, markdown_mode_declaration())
        .expect("markdown mode registers");
    let md = with_markdown
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some("README.md".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("markdown classifies");
    assert_eq!(md.mode_id, "markdown");

    // Simulate every language package disabled: a registry with no package
    // modes registered still yields an editable built-in fallback mode.
    let without_any_package = ModeRegistry::new();
    let fallback_md = without_any_package
        .classify(&DocumentClassificationInput {
            document_id: 2,
            path: Some("README.md".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("markdown file falls back to core.text when no package is loaded");
    assert_eq!(fallback_md.mode_id, "core.text");
    assert_eq!(fallback_md.matched_by, ModePatternKind::Fallback);

    // A code-like file still resolves to core.code even with no packages.
    let fallback_code = without_any_package
        .classify(&DocumentClassificationInput {
            document_id: 3,
            path: Some("main.rs".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("rust file falls back to core.code when no package is loaded");
    assert_eq!(fallback_code.mode_id, "core.code");
}

#[test]
fn register_builtin_mode_rejects_non_core_namespace_and_duplicates() {
    let mut registry = ModeRegistry::new();

    // Non-core IDs are rejected on the built-in registration path.
    let mut alien = core_text_mode();
    alien.mode_id = "markdown.text".to_string();
    let error = registry.register_builtin_mode(alien).unwrap_err();
    assert_eq!(error.rule, ModeValidationRule::InvalidModeId);

    // Duplicate built-in registration is rejected.
    let error = registry
        .register_builtin_mode(core_text_mode())
        .unwrap_err();
    assert_eq!(error.rule, ModeValidationRule::DuplicateModeId);
    let error = registry
        .register_builtin_mode(core_code_mode())
        .unwrap_err();
    assert_eq!(error.rule, ModeValidationRule::DuplicateModeId);

    // Package precedence beats built-in on a tie without raising ambiguity.
    let mut rustish = markdown_mode_declaration();
    rustish.mode_id = "rust".to_string();
    rustish.api_prefix = "rust".to_string();
    rustish.package_name = "@clay/rust".to_string();
    rustish.extensions = vec!["rs".to_string()];
    let mut rust_fixture = markdown_fixture();
    rust_fixture["name"] = json!("@clay/rust");
    rust_fixture["clay"]["apiPrefix"] = json!("rust");
    rust_fixture["clay"]["modes"] = json!(["rust"]);
    let rust_manifest = validate_manifest_value(&rust_fixture).expect("valid rust fixture");
    registry
        .register_mode(&rust_manifest, rustish)
        .expect("package rust mode registers alongside built-ins");
    let winner = registry
        .classify(&DocumentClassificationInput {
            document_id: 42,
            path: Some("lib.rs".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("rs classifies");
    assert_eq!(winner.mode_id, "rust");
    assert_eq!(winner.matched_by, ModePatternKind::Extension);
}

// ── Phase 18.9 Task 4: shebang + bounded leading-content probes ───────────────

/// Build a minimal valid package fixture whose single declared mode is named
/// after its `api_prefix`. Used by the probe tests to register package-declared
/// shebang/content-probe modes.
fn probe_fixture(api_prefix: &str, package_name: &str) -> Value {
    json!({
        "name": package_name,
        "version": "0.1.0",
        "clay": {
            "apiPrefix": api_prefix,
            "permissions": ["mode-registration", "mode-activation"],
            "modes": [api_prefix],
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js"
        }
    })
}

/// A mode declaration carrying only the given probe metadata (everything else
/// empty), so the test exercises the probe signal in isolation.
fn probe_mode(
    prefix: &str,
    display: &str,
    shebang: Vec<String>,
    content: Vec<String>,
) -> ModeDeclaration {
    ModeDeclaration {
        package_name: format!("@clay/{prefix}"),
        package_version: "0.1.0".to_string(),
        api_prefix: prefix.to_string(),
        mode_id: prefix.to_string(),
        display_name: display.to_string(),
        document_font_role: clay::protocol::DocumentFontRole::Proportional,
        extensions: Vec::new(),
        mime_types: Vec::new(),
        file_names: Vec::new(),
        file_name_patterns: Vec::new(),
        shebang_patterns: shebang,
        content_probes: content,
    }
}

#[test]
fn shebang_routes_script_to_owning_package_then_core_code() {
    let mut registry = ModeRegistry::new();

    // python package declares only a shebang pattern (no extensions).
    let py_manifest =
        validate_manifest_value(&probe_fixture("python", "@clay/python")).expect("valid python");
    registry
        .register_mode(
            &py_manifest,
            probe_mode("python", "Python", vec!["python*".to_string()], Vec::new()),
        )
        .expect("python mode registers");

    // A .txt-named script with a python shebang routes to the python package
    // via the shebang signal, NOT core.text.
    let scripted = registry
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some("scripts/run.txt".to_string()),
            mime_type: None,
            shebang: Some("#!/usr/bin/env python3".to_string()),
            leading_content: None,
        })
        .expect("shebang classifies to python");
    assert_eq!(scripted.mode_id, "python");
    assert_eq!(scripted.matched_by, ModePatternKind::Shebang);

    // With no owning package, the same shebang script falls back to core.code
    // (any shebang marks a document as code-like), never core.text.
    let no_package = ModeRegistry::new();
    let fallback = no_package
        .classify(&DocumentClassificationInput {
            document_id: 2,
            path: Some("scripts/run.txt".to_string()),
            mime_type: None,
            shebang: Some("#!/usr/bin/env python3".to_string()),
            leading_content: None,
        })
        .expect("shebang falls back to core.code");
    assert_eq!(fallback.mode_id, "core.code");
    assert_eq!(fallback.matched_by, ModePatternKind::Shebang);

    // The same .txt file WITHOUT a shebang is plain text (core.text), proving
    // the shebang — not the extension — drove the core.code result above.
    let plain = no_package
        .classify(&DocumentClassificationInput {
            document_id: 3,
            path: Some("scripts/run.txt".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("plain txt falls back to core.text");
    assert_eq!(plain.mode_id, "core.text");
    assert_eq!(plain.matched_by, ModePatternKind::Fallback);
}

#[test]
fn bounded_content_probe_matches_and_oversize_slice_is_rejected() {
    let mut registry = ModeRegistry::new();
    let xml_manifest =
        validate_manifest_value(&probe_fixture("xml", "@clay/xml")).expect("valid xml");
    registry
        .register_mode(
            &xml_manifest,
            probe_mode("xml", "XML", Vec::new(), vec!["<?xml".to_string()]),
        )
        .expect("xml mode registers");

    // Within-bound leading content starting with the marker classifies as xml.
    let matched = registry
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some("data.bin".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: Some("<?xml version=\"1.0\"?>".to_string()),
        })
        .expect("content probe classifies xml");
    assert_eq!(matched.mode_id, "xml");
    assert_eq!(matched.matched_by, ModePatternKind::ContentProbe);

    // Oversize slice is rejected (treated as absent); classification still
    // succeeds via the fallback ladder (core.text for a .bin file).
    let oversize = format!("<?xml{}", "x".repeat(MAX_LEADING_CONTENT_BYTES));
    assert!(oversize.len() > MAX_LEADING_CONTENT_BYTES);
    let fallback = registry
        .classify(&DocumentClassificationInput {
            document_id: 2,
            path: Some("data.bin".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: Some(oversize),
        })
        .expect("oversize content still classifies via fallback");
    assert_eq!(fallback.mode_id, "core.text");
    assert_eq!(fallback.matched_by, ModePatternKind::Fallback);

    // Within-bound content that does NOT start with the marker falls back too.
    let nomatch = registry
        .classify(&DocumentClassificationInput {
            document_id: 3,
            path: Some("data.bin".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: Some("not xml at all".to_string()),
        })
        .expect("non-matching content falls back");
    assert_eq!(nomatch.mode_id, "core.text");
}

#[test]
fn ambiguous_equal_priority_probes_are_rejected() {
    // Two packages both claim the same shebang interpreter.
    let mut registry = ModeRegistry::new();
    let alpha = validate_manifest_value(&probe_fixture("alpha", "@clay/alpha")).expect("valid");
    let beta = validate_manifest_value(&probe_fixture("beta", "@clay/beta")).expect("valid");
    registry
        .register_mode(
            &alpha,
            probe_mode("alpha", "Alpha", vec!["python*".to_string()], Vec::new()),
        )
        .unwrap();
    registry
        .register_mode(
            &beta,
            probe_mode("beta", "Beta", vec!["python*".to_string()], Vec::new()),
        )
        .unwrap();
    let err = registry
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some("s".to_string()),
            mime_type: None,
            shebang: Some("#!/usr/bin/env python3".to_string()),
            leading_content: None,
        })
        .unwrap_err();
    assert_eq!(err.rule, ModeValidationRule::AmbiguousClassification);

    // Two packages both claim the same content marker.
    let mut registry2 = ModeRegistry::new();
    let gamma = validate_manifest_value(&probe_fixture("gamma", "@clay/gamma")).expect("valid");
    let delta = validate_manifest_value(&probe_fixture("delta", "@clay/delta")).expect("valid");
    registry2
        .register_mode(
            &gamma,
            probe_mode("gamma", "Gamma", Vec::new(), vec!["<?xml".to_string()]),
        )
        .unwrap();
    registry2
        .register_mode(
            &delta,
            probe_mode("delta", "Delta", Vec::new(), vec!["<?xml".to_string()]),
        )
        .unwrap();
    let err = registry2
        .classify(&DocumentClassificationInput {
            document_id: 2,
            path: Some("s".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: Some("<?xml version".to_string()),
        })
        .unwrap_err();
    assert_eq!(err.rule, ModeValidationRule::AmbiguousClassification);
}

#[test]
fn classification_precedence_extension_beats_shebang_beats_content_probe() {
    let mut registry = ModeRegistry::new();
    // ext claims `.foo`; sheb claims shebang `foo*`; probe claims `<FOO>` marker.
    let mut ext = probe_mode("ext", "Ext", Vec::new(), Vec::new());
    ext.extensions = vec!["foo".to_string()];
    let sheb = probe_mode("sheb", "Sheb", vec!["foo*".to_string()], Vec::new());
    let probe = probe_mode("probe", "Probe", Vec::new(), vec!["<FOO>".to_string()]);
    let ext_m = validate_manifest_value(&probe_fixture("ext", "@clay/ext")).expect("valid");
    let sheb_m = validate_manifest_value(&probe_fixture("sheb", "@clay/sheb")).expect("valid");
    let probe_m = validate_manifest_value(&probe_fixture("probe", "@clay/probe")).expect("valid");
    registry.register_mode(&ext_m, ext).unwrap();
    registry.register_mode(&sheb_m, sheb).unwrap();
    registry.register_mode(&probe_m, probe).unwrap();

    let input = DocumentClassificationInput {
        document_id: 1,
        path: Some("x.foo".to_string()),
        mime_type: None,
        shebang: Some("#!/usr/bin/env foo".to_string()),
        leading_content: Some("<FOO> body".to_string()),
    };
    // Extension wins over shebang and content probe.
    let winner = registry.classify(&input).expect("extension wins");
    assert_eq!(winner.mode_id, "ext");
    assert_eq!(winner.matched_by, ModePatternKind::Extension);

    // Drop the claimed extension: shebang now wins over the content probe.
    let mut no_ext = input.clone();
    no_ext.path = Some("x.bin".to_string());
    let sheb_winner = registry.classify(&no_ext).expect("shebang wins");
    assert_eq!(sheb_winner.mode_id, "sheb");
    assert_eq!(sheb_winner.matched_by, ModePatternKind::Shebang);

    // Drop the shebang too: the content probe is the only remaining signal.
    let mut no_shebang = no_ext.clone();
    no_shebang.shebang = None;
    let probe_winner = registry.classify(&no_shebang).expect("content probe wins");
    assert_eq!(probe_winner.mode_id, "probe");
    assert_eq!(probe_winner.matched_by, ModePatternKind::ContentProbe);
}

#[test]
fn classification_reads_no_filesystem() {
    // classify uses only the supplied input fields; a path that does not exist
    // on disk still classifies via the shebang signal without any file read,
    // proving probes perform no filesystem scan.
    let registry = ModeRegistry::new();
    let res = registry
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some("/nonexistent/path/that/does/not/exist/script".to_string()),
            mime_type: None,
            shebang: Some("#!/bin/bash".to_string()),
            leading_content: None,
        })
        .expect("classifies without reading the filesystem");
    assert_eq!(res.mode_id, "core.code");
    assert_eq!(res.matched_by, ModePatternKind::Shebang);
}

// ── Phase 18.9 Task 7: reclassification, stale behavior-version, budgets ──

#[test]
fn disabling_language_package_mid_session_reclassifies_to_fallback_and_increments_behavior_version()
{
    // Pre-reload: a language package owns README.md.
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(&manifest, markdown_mode_declaration())
        .expect("markdown mode registers");
    let input = |id| DocumentClassificationInput {
        document_id: id,
        path: Some("README.md".to_string()),
        mime_type: None,
        shebang: None,
        leading_content: None,
    };
    let md = registry.classify(&input(1)).expect("markdown classifies");
    assert_eq!(md.mode_id, "markdown");
    let first = registry
        .activate_major_mode(&manifest, md)
        .expect("activate markdown");
    assert_eq!(first.behavior_version, 1);

    // Mid-session: the markdown package is disabled. `unregister_mode` drops
    // the declaration, so the centralized `select_behavior_manifest_for_document`
    // can no longer serve a manifest for `markdown` (its owning package is no
    // longer enabled) — this is the no-bypass protection: a disabled package
    // cannot leave a stale active mode that bypasses validation. Built-in
    // `core.*` modes are Clay-owned and cannot be removed. The prior active
    // entry is deliberately retained so reactivation bumps the version.
    assert!(registry.unregister_mode("markdown"));
    assert!(!registry.unregister_mode("markdown"), "already removed");
    assert!(
        !registry.unregister_mode("core.text"),
        "built-in core.* modes cannot be removed mid-session"
    );
    // The stale activation remains until reclassification replaces it, but it
    // is inert: manifest selection errors (safe), proving no validation bypass.
    assert_eq!(
        registry.active_major_mode(1).map(|a| a.mode_id.as_str()),
        Some("markdown"),
        "stale entry retained for version bump on reactivation"
    );
    assert!(
        registry
            .select_behavior_manifest_for_document(1, &[])
            .is_err(),
        "disabled-package stale activation cannot serve a manifest (no bypass)"
    );

    // Reclassification reuses the centralized `classify` + activation path
    // (no parallel reclassification primitive) and falls back deterministically
    // to the always-available built-in `core.text` because no package now claims
    // `.md`.
    let fallback = registry
        .classify(&input(1))
        .expect("reclassifies deterministically");
    assert_eq!(fallback.mode_id, "core.text");
    assert_eq!(fallback.matched_by, ModePatternKind::Fallback);
    let second = registry
        .activate_builtin_major_mode(fallback)
        .expect("activate built-in core.text");

    // The reclassified activation is strictly newer, so any client-stashed
    // behavior_version from the pre-reload activation is now stale.
    assert!(
        second.behavior_version > first.behavior_version,
        "reclassification must activate a new behavior version (got {} then {})",
        first.behavior_version,
        second.behavior_version
    );
    assert_eq!(
        registry.active_major_mode(1).map(|a| a.mode_id.as_str()),
        Some("core.text")
    );

    // The document remains editable end-to-end: the centralized manifest
    // selection path composes an inert manifest for the fallback mode with the
    // new behavior version and no owning package record.
    let selection = registry
        .select_behavior_manifest_for_document(1, &[])
        .expect("fallback behavior manifest composes for editable document");
    assert_eq!(selection.manifest.behavior_version, second.behavior_version);
    assert_eq!(
        selection.manifest.scope,
        BehaviorScope::Document { document_id: 1 }
    );
    assert!(selection.major_mode.mode_id == "core.text");
    assert!(selection.minor_modes.is_empty());
}

#[test]
fn manifest_served_after_reactivation_carries_strictly_newer_behavior_version() {
    // Stale behavior-version rejection is enforced at the connection layer
    // (`EditRejection::InvalidBehaviorVersion`, covered by
    // `server::connection::tests::server_rejects_edit_with_stale_behavior_version_without_mutating_document`).
    // Its registry-level precondition is that the latest activation is strictly
    // newer than any prior one for the same document, so the manifest the
    // client built edits against is provably stale after reactivation. Verified
    // here on a built-in `core.code` fallback so no package record is needed.
    let mut registry = ModeRegistry::new();
    let input = DocumentClassificationInput {
        document_id: 5,
        path: Some("main.rs".to_string()),
        mime_type: None,
        shebang: None,
        leading_content: None,
    };
    let first = registry
        .activate_builtin_major_mode(registry.classify(&input).expect("classify core.code"))
        .expect("activate");
    let first_manifest_version = registry
        .select_behavior_manifest_for_document(5, &[])
        .expect("first manifest composes")
        .manifest
        .clone()
        .behavior_version;
    assert_eq!(first_manifest_version, first.behavior_version);

    // Reactivation (e.g. forced reclassify after a touched package or reload)
    // reuses the same centralized activation path and bumps the recorded
    // version, so the manifest served afterwards is strictly newer.
    let second = registry
        .activate_builtin_major_mode(registry.classify(&input).expect("classify core.code"))
        .expect("reactivate");
    let second_manifest_version = registry
        .select_behavior_manifest_for_document(5, &[])
        .expect("second manifest composes")
        .manifest
        .clone()
        .behavior_version;

    assert!(second.behavior_version > first.behavior_version);
    assert!(second_manifest_version > first_manifest_version);
    assert_eq!(second_manifest_version, second.behavior_version);
    // A client edit carrying the prior behavior_version is stale by
    // construction: the served manifest version is strictly newer.
    assert_ne!(first_manifest_version, second_manifest_version);
}

#[test]
fn fallback_activation_manifest_fits_payload_budget() {
    // Reclassification to a built-in fallback must still ship an inert manifest
    // under BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES so fallback activation never
    // blows the protocol budget. core.text and core.code are the always-available
    // fallbacks, so measure both.
    let mut registry = ModeRegistry::new();
    let codec = Codec::default();

    let inputs = [
        ("README.txt", "core.text", ModePatternKind::Fallback),
        ("main.rs", "core.code", ModePatternKind::Extension),
    ];
    for (idx, (path, expected_mode, expected_kind)) in inputs.iter().enumerate() {
        let document_id = (idx + 1) as u64;
        let classification = registry
            .classify(&DocumentClassificationInput {
                document_id,
                path: Some((*path).to_string()),
                mime_type: None,
                shebang: None,
                leading_content: None,
            })
            .expect("fallback classifies");
        assert_eq!(classification.mode_id, *expected_mode);
        assert_eq!(classification.matched_by, *expected_kind);
        registry
            .activate_builtin_major_mode(classification)
            .expect("activate built-in fallback");
        let selection = registry
            .select_behavior_manifest_for_document(document_id, &[])
            .expect("fallback manifest composes");
        let frame = codec
            .encode_server_message(&ServerMessage::BehaviorManifest(Box::new(
                selection.manifest.clone(),
            )))
            .expect("fallback behavior manifest must encode");
        let payload = protocol_payload_len(&frame);
        assert!(
            payload <= BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES,
            "fallback ({expected_mode}) behavior manifest payload {payload} exceeds budget {BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES}"
        );
    }
}

// ── Phase 18.9 Task 8: package default init.js loading experience ──
//
// Verifies the always-available guarantee: `core.text`/`core.code` are
// Clay-owned built-in modes registered at `ModeRegistry::new()` (server
// startup) with no `~/.config/clay/init.js` line, no `loadPackage` step, and
// no package enable/load authority. A fresh registry with zero packages is
// the faithful simulation of an absent/empty `init.js`: no package has been
// loaded, yet every file still classifies and activates an editable built-in
// fallback mode through the centralized activation path used on open.

#[test]
fn empty_init_js_opens_txt_and_rs_into_core_fallbacks_and_remains_editable() {
    // No init.js, no loadPackage: a fresh registry simulates the absent-config
    // open path. Built-in core.text/core.code are always-on at server startup.
    let mut registry = ModeRegistry::new();

    // .txt → core.text (universal Fallback). Activate via the centralized
    // built-in path (no package manifest, no mode-activation permission).
    let text_classification = registry
        .classify(&DocumentClassificationInput {
            document_id: 11,
            path: Some("notes.txt".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("plaintext file classifies as core.text with no init.js");
    assert_eq!(text_classification.mode_id, "core.text");
    assert_eq!(text_classification.matched_by, ModePatternKind::Fallback);
    let text_activation = registry
        .activate_builtin_major_mode(text_classification)
        .expect("core.text activates without init.js or loadPackage");

    // .rs → core.code (Extension). Same centralized built-in activation path.
    let code_classification = registry
        .classify(&DocumentClassificationInput {
            document_id: 12,
            path: Some("src/main.rs".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("rust file classifies as core.code with no init.js");
    assert_eq!(code_classification.mode_id, "core.code");
    assert_eq!(code_classification.matched_by, ModePatternKind::Extension);
    let code_activation = registry
        .activate_builtin_major_mode(code_classification)
        .expect("core.code activates without init.js or loadPackage");

    // Both documents must remain editable: the served behavior manifest is
    // scoped per-document and carries a real editor rule set. core.text ships
    // minimal_text_editing (no electric chars); core.code ships core_code_editing
    // (default_text + electric outdent rules for closing braces/brackets/parens).
    let text_selection = registry
        .select_behavior_manifest_for_document(11, &[])
        .expect("core.text manifest composes with no enabled packages");
    assert_eq!(text_selection.manifest.manifest_id, "core.core.text");
    assert_eq!(
        text_selection.manifest.behavior_version,
        text_activation.behavior_version
    );
    assert!(
        matches!(
            text_selection.manifest.scope,
            BehaviorScope::Document { document_id: 11 }
        ),
        "core.text manifest is scoped to the document"
    );
    assert!(
        text_selection
            .manifest
            .editor_rules
            .electric_characters
            .is_empty(),
        "core.text ships no electric characters"
    );

    let code_selection = registry
        .select_behavior_manifest_for_document(12, &[])
        .expect("core.code manifest composes with no enabled packages");
    assert_eq!(code_selection.manifest.manifest_id, "core.core.code");
    assert_eq!(
        code_selection.manifest.behavior_version,
        code_activation.behavior_version
    );
    assert!(
        matches!(
            code_selection.manifest.scope,
            BehaviorScope::Document { document_id: 12 }
        ),
        "core.code manifest is scoped to the document"
    );
    assert!(
        !code_selection
            .manifest
            .editor_rules
            .electric_characters
            .is_empty(),
        "core.code ships electric outdent rules so closing braces reflow"
    );
    assert!(
        code_selection
            .manifest
            .editor_rules
            .electric_characters
            .iter()
            .any(|rule| rule.trigger == "}"),
        "core.code electric set includes closing brace"
    );
}

#[test]
fn load_package_markdown_extends_core_code_for_md_while_rs_still_uses_core_code() {
    // The one-line `loadPackage("@clay/markdown")` init.js convention loads the
    // Markdown language package, which declares its own `.md` pattern. That
    // package-declared pattern wins precedence over the built-in core.code
    // fallback (package > built-in on the classification ladder), while `.rs`
    // still has no package claiming it and stays on core.code. This proves
    // language packages extend core.code rather than replacing it, and remain
    // explicit opt-in: with no loadPackage, .md would fall back to core.text.
    let manifest = validate_manifest_value(&markdown_fixture()).expect("valid markdown fixture");
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(&manifest, markdown_mode_declaration())
        .expect("markdown mode registers (simulates loadPackage @clay/markdown)");

    // .md → markdown package mode (package-declared Extension wins over the
    // built-in core.code/core.text fallbacks).
    let md = registry
        .classify(&DocumentClassificationInput {
            document_id: 21,
            path: Some("README.md".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("markdown classifies");
    assert_eq!(md.mode_id, "markdown");
    assert_eq!(md.api_prefix, "markdown");
    assert_eq!(md.matched_by, ModePatternKind::Extension);
    assert_ne!(md.mode_id, "core.code");

    // .rs → core.code (still the built-in fallback; the markdown package only
    // claims .md and does not extend to .rs).
    let code = registry
        .classify(&DocumentClassificationInput {
            document_id: 22,
            path: Some("main.rs".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("rust file still classifies while markdown package is loaded");
    assert_eq!(code.mode_id, "core.code");
    assert_eq!(code.api_prefix, "core");
    assert_eq!(code.matched_by, ModePatternKind::Extension);
}
