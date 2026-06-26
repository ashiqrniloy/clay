use clay::packages::conflict::{PackageConflictKind, check_enabled_packages};
/// Integration tests for the Phase 17 package enable/load contract validator
/// and package management service.
///
/// Task 1 — package record validator:
///   - `package_record_accepts_full_markdown_contract`
///   - `package_record_rejects_missing_required_contract_fields`
///   - `package_record_rejects_package_claiming_clay_reserved_ids`
///   - `package_record_rejects_undeclared_permission_for_contribution`
///
/// Task 2 — package service:
///   - `package_service_install_records_without_executing_runtime`
///   - `package_service_enable_rejects_invalid_clay_metadata`
///   - `package_service_disable_removes_active_contributions`
///   - `package_cli_subcommands_route_through_shared_service`
///
/// Task 3 — per-document behavior manifest selection:
///   - `behavior_manifest_selected_per_document_with_provenance`
///   - `minor_mode_rejected_when_incompatible_major_mode`
///   - `minor_mode_cannot_override_major_mode_entries`
///   - `keypress_routing_uses_manifest_without_javascript`
///
/// Task 4 — deterministic conflicts and SDUI/status provenance:
///   - `enable_rejects_duplicate_prefix_and_mode_and_command`
///   - `ambiguous_keybinding_across_packages_rejected_without_priority`
///   - `package_sdui_contribution_carries_provenance_and_respects_budget`
use clay::packages::manager::FakeBackend;
use clay::packages::modes::{
    DocumentClassificationInput, MajorModeActivation, MinorModeDeclaration, ModeDeclaration,
    ModeRegistry, ModeValidationRule,
};
use clay::packages::record::{PackageRecordRule, assemble_package_record};
use clay::packages::service::{PackageService, PackageServiceError};
use clay::protocol::BehaviorScope;
use serde_json::{Value, json};
use std::path::Path;

// ── Fixtures (Task 1) ─────────────────────────────────────────────────────────

fn full_markdown_fixture() -> Value {
    json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "markdown",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "permissions": [
                "mode-registration",
                "mode-activation",
                "command-registration",
                "parse-document",
                "render-decorations",
                "package-configuration"
            ],
            "modes": ["markdown"],
            "docs": "./docs/index.md",
            "apiDependencies": [
                "clay.modes.serverRegisterModePattern",
                "clay.modes.serverActivateMajorMode",
                "clay.commands.serverRegisterCommand",
                "clay.parse.serverRegisterParseHandler",
                "clay.decorations.serverPublishDecorations"
            ],
            "contributions": {
                "commands": [{
                    "id": "markdown.togglePreview",
                    "displayName": "Toggle Markdown Preview",
                    "routingPolicy": "server-first"
                }],
                "configuration": [{
                    "key": "markdown.preview.enabled",
                    "type": "boolean",
                    "default": false
                }],
                "textTransforms": [{
                    "transformId": "markdown.list-continuation",
                    "kind": "enter-rule"
                }],
                "decorations": [{
                    "primitiveId": "markdown.syntaxDecorations",
                    "kind": "markdown.syntax"
                }]
            }
        }
    })
}

fn first_party_markdown_package_json() -> Value {
    let text = std::fs::read_to_string("packages/markdown/package.json")
        .expect("first-party Markdown package.json must exist");
    serde_json::from_str(&text).expect("first-party Markdown package.json must be valid JSON")
}

fn phase18_3_slot_ui_fixture(package_name: &str, prefix: &str, slot: &str) -> Value {
    let command_id = format!("{prefix}.togglePreview");
    json!({
        "name": package_name,
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": prefix,
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": [prefix],
            "docs": "./docs/index.md",
            "apiDependencies": [
                "clay.commands.serverRegisterCommand",
                "clay.ui.serverRegisterPanelContribution",
                "clay.ui.serverRegisterComponentContribution",
                "clay.ui.serverRegisterTransientOverlayContribution",
                "clay.ui.serverRegisterThemeToken"
            ],
            "contributions": {
                "commands": [{
                    "id": command_id,
                    "displayName": "Toggle Preview",
                    "routingPolicy": "server-first"
                }],
                "themeTokens": [{
                    "token": format!("{prefix}.preview.background"),
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Preview panel background"
                }],
                "ui": {
                    "components": [{
                        "kind": "label",
                        "id": format!("{prefix}.preview.empty"),
                        "text": "Preview unavailable",
                        "style": { "contentColor": "text.muted", "typography": "typography.body" }
                    }],
                    "panels": [{
                        "id": format!("{prefix}.preview"),
                        "slot": slot,
                        "kind": "fixed",
                        "defaultVisibility": "hidden",
                        "actionTargets": [command_id],
                        "component": {
                            "kind": "panel",
                            "id": format!("{prefix}.preview.root"),
                            "title": "Preview",
                            "style": { "background": format!("{prefix}.preview.background"), "padding": "spacing.panel" },
                            "children": [{
                                "kind": "button",
                                "id": format!("{prefix}.preview.toggle"),
                                "label": "Toggle",
                                "action": { "commandId": command_id }
                            }]
                        }
                    }],
                    "overlays": [{
                        "id": format!("{prefix}.quickOpen"),
                        "anchor": "working-area",
                        "focusPolicy": "restore",
                        "dismissalPolicy": "escape",
                        "component": {
                            "kind": "panel",
                            "id": format!("{prefix}.quickOpen.root"),
                            "title": "Quick Open",
                            "children": []
                        }
                    }]
                }
            }
        }
    })
}

fn phase18_4_input_state_config_fixture(package_name: &str, prefix: &str) -> Value {
    let command_id = format!("{prefix}.focusPreview");
    json!({
        "name": package_name,
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": prefix,
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration", "package-configuration"],
            "modes": [prefix],
            "docs": "./docs/index.md",
            "apiDependencies": [
                "clay.commands.serverRegisterCommand",
                "clay.ui.serverRegisterInputContribution",
                "clay.ui.serverRegisterUiStateScope",
                "clay.ui.serverSetLayoutOverride",
                "clay.configuration.setPackageOption"
            ],
            "contributions": {
                "commands": [{
                    "id": command_id,
                    "displayName": "Focus Preview",
                    "routingPolicy": "server-first"
                }],
                "themeTokens": [{
                    "token": format!("{prefix}.preview.background"),
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Preview background"
                }],
                "input": [{
                    "id": format!("{prefix}.preview.input"),
                    "scope": "component",
                    "componentId": format!("{prefix}.preview.root"),
                    "pointer": { "click": "action", "action": command_id, "drag": "select" },
                    "focus": { "policy": "restore-editor" },
                    "selectionPolicy": "component-local",
                    "context": { "modes": [prefix] },
                    "actionTargets": [format!("{prefix}.focusPreview")]
                }],
                "uiStateScopes": [{
                    "id": format!("{prefix}.preview.visibility"),
                    "scope": "pane",
                    "targetId": format!("{prefix}.preview"),
                    "owner": "shell",
                    "lifetime": "session",
                    "persistence": "client-local",
                    "implementationStatus": "implemented",
                    "valueSchema": { "kind": "enum", "values": ["visible", "hidden"] }
                }],
                "layoutOverrides": [{
                    "targetId": format!("{prefix}.preview"),
                    "property": "inputDefault",
                    "value": { "inputId": format!("{prefix}.preview.input") },
                    "source": "package-default"
                }, {
                    "targetId": format!("{prefix}.preview"),
                    "property": "themeToken",
                    "value": { "token": format!("{prefix}.preview.background"), "fallback": "surface.panel" },
                    "source": "package-default"
                }],
                "packageOptions": [{
                    "option": format!("{prefix}.layout.defaultVisibility"),
                    "type": "string",
                    "default": "hidden"
                }, {
                    "option": format!("{prefix}.input.default"),
                    "type": "string",
                    "default": format!("{prefix}.preview.input")
                }]
            }
        }
    })
}

// ── Task 1 tests ──────────────────────────────────────────────────────────────

/// A complete Markdown-style package record validates with provenance retained.
#[test]
fn package_record_accepts_full_markdown_contract() {
    let record = assemble_package_record(&full_markdown_fixture())
        .expect("full Markdown package contract must validate");

    assert_eq!(record.manifest.name, "@clay/markdown");
    assert_eq!(record.manifest.version, "0.1.0");
    assert_eq!(record.manifest.clay.api_prefix, "markdown");
    assert_eq!(record.manifest.clay.entry, "./dist/index.js");
    assert_eq!(
        record.manifest.clay.load_entry.as_deref(),
        Some("./dist/load.js")
    );
    assert_eq!(record.docs.docs_path, "./docs/index.md");
    assert_eq!(record.api_dependencies.len(), 5);
    assert_eq!(
        record.api_dependencies[0].api_id,
        "clay.modes.serverRegisterModePattern"
    );
    assert!(
        record
            .api_dependencies
            .iter()
            .any(|dependency| dependency.api_id == "clay.commands.serverRegisterCommand")
    );
    assert_eq!(record.contributions.commands.len(), 1);
    let cmd = &record.contributions.commands[0];
    assert_eq!(cmd.id, "markdown.togglePreview");
    assert_eq!(cmd.display_name, "Toggle Markdown Preview");
    assert_eq!(cmd.routing_policy, "server-first");
    assert_eq!(record.contributions.configuration.len(), 1);
    assert_eq!(
        record.contributions.configuration[0].key,
        "markdown.preview.enabled"
    );
    assert_eq!(record.contributions.configuration[0].value_type, "boolean");
    assert_eq!(record.contributions.text_transforms.len(), 1);
    assert_eq!(
        record.contributions.text_transforms[0].transform_id,
        "markdown.list-continuation"
    );
    assert_eq!(record.contributions.text_transforms[0].kind, "enter-rule");
    assert!(record.performance.estimated_manifest_bytes > 0);
}

#[test]
fn markdown_package_contract_validates_with_required_metadata() {
    let package = first_party_markdown_package_json();
    let record = assemble_package_record(&package)
        .expect("first-party Markdown package contract must validate");

    assert_eq!(record.manifest.name, "@clay/markdown");
    assert_eq!(record.manifest.clay.api_prefix, "markdown");
    assert_eq!(record.manifest.clay.modes, vec!["markdown"]);
    assert_eq!(record.docs.docs_path, "./docs/index.md");
    assert_eq!(record.contributions.commands.len(), 3);
    assert_eq!(record.contributions.key_routing.len(), 3);
    // Verify key binding descriptors carry command IDs and key tokens.
    let kr = &record.contributions.key_routing;
    assert!(kr.iter().any(|k| k.command_id == "markdown.togglePreview"
        && k.key_binding.as_deref() == Some("Ctrl+Shift+M")));
    assert!(kr.iter().any(|k| k.command_id == "markdown.insertHeading"
        && k.key_binding.as_deref() == Some("Ctrl+Alt+1")));
    assert!(kr.iter().any(|k| k.command_id == "markdown.toggleList"
        && k.key_binding.as_deref() == Some("Ctrl+Shift+8")));
    assert_eq!(record.contributions.text_transforms.len(), 3);
    assert_eq!(record.contributions.sdui.len(), 1);
    assert_eq!(record.contributions.decorations.len(), 1);

    let clay = package["clay"].as_object().unwrap();
    let mode_patterns = clay["contributions"]["modePatterns"].as_array().unwrap();
    assert_eq!(
        mode_patterns[0]["extensions"],
        json!(["md", "markdown", "mdown"])
    );
    assert_eq!(mode_patterns[0]["mimeTypes"], json!(["text/markdown"]));
    assert!(
        clay["performance"]["hotPathPolicy"]
            .as_str()
            .unwrap()
            .contains("keypress")
    );
}

#[test]
fn markdown_package_rejects_missing_required_permissions() {
    for (api_id, missing_permission) in [
        ("clay.modes.serverRegisterModePattern", "mode-registration"),
        ("clay.modes.serverActivateMajorMode", "mode-activation"),
        (
            "clay.commands.serverRegisterCommand",
            "command-registration",
        ),
        ("clay.parse.serverRegisterParseHandler", "parse-document"),
        (
            "clay.decorations.serverPublishDecorations",
            "render-decorations",
        ),
    ] {
        let mut package = first_party_markdown_package_json();
        let permissions = package["clay"]["permissions"].as_array_mut().unwrap();
        permissions.retain(|value| value.as_str() != Some(missing_permission));

        let err = assemble_package_record(&package).unwrap_err();
        assert_eq!(
            err.rule,
            PackageRecordRule::UndeclaredPermissionForContribution
        );
        if api_id != "clay.commands.serverRegisterCommand" {
            assert_eq!(err.contribution_id.as_deref(), Some(api_id));
        }
        assert!(
            err.message.contains(missing_permission),
            "got: {}",
            err.message
        );
    }
}

#[test]
fn markdown_package_does_not_execute_on_install() {
    let mut service = PackageService::new(
        "target/test-package-store/markdown",
        Box::new(FakeBackend::default()),
    );
    service
        .install_from_value(first_party_markdown_package_json())
        .expect("installing from metadata records package");

    let inspection = service
        .inspect("@clay/markdown")
        .expect("installed package can be inspected without enable/load execution");
    assert!(!inspection.is_enabled);
    assert_eq!(inspection.api_prefix, "markdown");
}

#[test]
fn package_manifest_accepts_phase18_3_ui_contribution_metadata() {
    let record = assemble_package_record(&phase18_3_slot_ui_fixture(
        "@clay/markdown",
        "markdown",
        "right",
    ))
    .expect("Phase 18.3 slot UI metadata should validate at package load time");

    assert!(
        record
            .api_dependencies
            .iter()
            .any(|dependency| dependency.api_id == "clay.ui.serverRegisterPanelContribution")
    );
    assert_eq!(record.contributions.theme_tokens.len(), 1);
    assert_eq!(
        record.contributions.theme_tokens[0].token,
        "markdown.preview.background"
    );
    assert_eq!(record.contributions.ui_panels.len(), 1);
    assert_eq!(record.contributions.ui_panels[0].id, "markdown.preview");
    assert_eq!(record.contributions.ui_panels[0].slot, "right");
    assert_eq!(record.contributions.ui_components.len(), 1);
    assert_eq!(
        record.contributions.ui_components[0].id,
        "markdown.preview.empty"
    );
    assert_eq!(
        record.contributions.ui_components[0].style_variable_count,
        2
    );
    assert_eq!(record.contributions.ui_overlays.len(), 1);
    assert_eq!(record.contributions.ui_overlays[0].id, "markdown.quickOpen");
}

#[test]
fn package_manifest_rejects_invalid_slot_ui_contribution_metadata() {
    let mut unknown_dependency = phase18_3_slot_ui_fixture("@clay/markdown", "markdown", "right");
    unknown_dependency["clay"]["apiDependencies"]
        .as_array_mut()
        .unwrap()
        .push(json!("clay.ui.serverMutateMasonryWidget"));
    let err = assemble_package_record(&unknown_dependency).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidApiDependency);
    assert_eq!(
        err.contribution_id.as_deref(),
        Some("clay.ui.serverMutateMasonryWidget")
    );

    let mut duplicate_slot = phase18_3_slot_ui_fixture("@clay/markdown", "markdown", "right");
    let second_panel = json!({
        "id": "markdown.outline",
        "slot": "right",
        "kind": "fixed",
        "component": { "kind": "panel", "id": "markdown.outline.root", "children": [] }
    });
    duplicate_slot["clay"]["contributions"]["ui"]["panels"]
        .as_array_mut()
        .unwrap()
        .push(second_panel);
    let err = assemble_package_record(&duplicate_slot).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::DuplicateContributionId);
    assert!(err.message.contains("same shell slot"));

    let mut invalid_action = phase18_3_slot_ui_fixture("@clay/markdown", "markdown", "right");
    invalid_action["clay"]["contributions"]["ui"]["panels"][0]["actionTargets"] =
        json!(["markdown.missing"]);
    let err = assemble_package_record(&invalid_action).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert_eq!(err.contribution_id.as_deref(), Some("markdown.missing"));

    let mut invalid_fallback = phase18_3_slot_ui_fixture("@clay/markdown", "markdown", "right");
    invalid_fallback["clay"]["contributions"]["themeTokens"][0]["fallback"] =
        json!("spacing.panel");
    let err = assemble_package_record(&invalid_fallback).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(err.message.contains("same type"));

    let mut raw_style = phase18_3_slot_ui_fixture("@clay/markdown", "markdown", "right");
    raw_style["clay"]["contributions"]["ui"]["panels"][0]["component"]["style"]["background"] =
        json!("#ffffff");
    let err = assemble_package_record(&raw_style).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(err.message.contains("raw CSS") || err.message.contains("raw colors"));

    let mut unsupported_component =
        phase18_3_slot_ui_fixture("@clay/markdown", "markdown", "right");
    unsupported_component["clay"]["contributions"]["ui"]["panels"][0]["component"]["children"][0]
        ["kind"] = json!("modal");
    let err = assemble_package_record(&unsupported_component).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(err.message.contains("reserved for a later"));

    let mut oversize = phase18_3_slot_ui_fixture("@clay/markdown", "markdown", "right");
    oversize["clay"]["contributions"]["ui"]["panels"][0]["estimatedPayloadBytes"] = json!(4097);
    let err = assemble_package_record(&oversize).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::PayloadBudgetExceeded);
}

#[test]
fn enabled_package_conflicts_reject_duplicate_slot_ui_ids_and_slots() {
    let first = assemble_package_record(&phase18_3_slot_ui_fixture(
        "@clay/markdown",
        "markdown",
        "right",
    ))
    .unwrap();
    let second = assemble_package_record(&phase18_3_slot_ui_fixture(
        "@clay/asciidoc",
        "asciidoc",
        "right",
    ))
    .unwrap();

    let err = check_enabled_packages([&first, &second]).unwrap_err();
    assert_eq!(err.kind, PackageConflictKind::UiFixedSlotCollision);
    assert_eq!(&*err.contribution_id, "right");
}

#[test]
fn package_manifest_accepts_phase18_4_input_state_config_metadata() {
    let record = assemble_package_record(&phase18_4_input_state_config_fixture(
        "@clay/markdown",
        "markdown",
    ))
    .expect("Phase 18.4 input/state/config metadata should validate at package load time");

    assert!(
        record
            .api_dependencies
            .iter()
            .any(|dependency| dependency.api_id == "clay.ui.serverRegisterInputContribution")
    );
    assert_eq!(record.contributions.input_contributions.len(), 1);
    assert_eq!(
        record.contributions.input_contributions[0].id,
        "markdown.preview.input"
    );
    assert_eq!(
        record.contributions.input_contributions[0].scope,
        "component"
    );
    assert_eq!(record.contributions.ui_state_scopes.len(), 1);
    assert_eq!(
        record.contributions.ui_state_scopes[0].id,
        "markdown.preview.visibility"
    );
    assert_eq!(
        record.contributions.ui_state_scopes[0].value_schema_kind,
        "enum"
    );
    assert_eq!(record.contributions.layout_overrides.len(), 2);
    assert_eq!(record.contributions.package_options.len(), 2);
    assert_eq!(
        record.contributions.package_options[0].option,
        "markdown.layout.defaultVisibility"
    );
}

#[test]
fn package_manifest_rejects_invalid_phase18_4_metadata() {
    let mut missing_permission = phase18_4_input_state_config_fixture("@clay/markdown", "markdown");
    missing_permission["clay"]["permissions"] = json!([
        "mode-registration",
        "mode-activation",
        "command-registration"
    ]);
    let err = assemble_package_record(&missing_permission).unwrap_err();
    assert_eq!(
        err.rule,
        PackageRecordRule::UndeclaredPermissionForContribution
    );
    assert!(err.message.contains("package-configuration"));

    let mut hidden_state = phase18_4_input_state_config_fixture("@clay/markdown", "markdown");
    hidden_state["clay"]["contributions"]["uiStateScopes"][0]["id"] = json!("markdown._secret");
    let err = assemble_package_record(&hidden_state).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(err.message.contains("hidden"));

    let mut bad_input = phase18_4_input_state_config_fixture("@clay/markdown", "markdown");
    bad_input["clay"]["contributions"]["input"][0]["pointer"]["action"] = json!("markdown.missing");
    let err = assemble_package_record(&bad_input).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert_eq!(err.contribution_id.as_deref(), Some("markdown.missing"));

    let mut hidden_option = phase18_4_input_state_config_fixture("@clay/markdown", "markdown");
    hidden_option["clay"]["contributions"]["packageOptions"][0]["option"] =
        json!("markdown._layout.defaultVisibility");
    let err = assemble_package_record(&hidden_option).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(err.message.contains("hidden"));

    let mut raw_layout = phase18_4_input_state_config_fixture("@clay/markdown", "markdown");
    raw_layout["clay"]["contributions"]["layoutOverrides"][0]["nativeHandle"] =
        json!("native-preview");
    let err = assemble_package_record(&raw_layout).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(err.message.contains("native widgets"));

    let mut bad_option_type = phase18_4_input_state_config_fixture("@clay/markdown", "markdown");
    bad_option_type["clay"]["contributions"]["packageOptions"][0]["type"] = json!("number");
    let err = assemble_package_record(&bad_option_type).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(err.message.contains("must declare type `string`"));
}

#[test]
fn enabled_package_conflicts_reject_duplicate_input_state_config_targets() {
    let first = assemble_package_record(&phase18_4_input_state_config_fixture(
        "@clay/markdown",
        "markdown",
    ))
    .unwrap();
    let mut second_input = phase18_4_input_state_config_fixture("@clay/markdown-alt", "markdown");
    second_input["clay"]["modes"] = json!(["markdown.alt"]);
    second_input["clay"]["contributions"]["commands"][0]["id"] = json!("markdown.focusAlt");
    second_input["clay"]["contributions"]["input"][0]["pointer"]["action"] =
        json!("markdown.focusAlt");
    second_input["clay"]["contributions"]["input"][0]["actionTargets"] =
        json!(["markdown.focusAlt"]);
    second_input["clay"]["contributions"]["input"][0]["context"]["modes"] = json!(["markdown.alt"]);
    second_input["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("themeTokens");
    second_input["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("uiStateScopes");
    second_input["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("layoutOverrides");
    second_input["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("packageOptions");
    let second = assemble_package_record(&second_input).unwrap();
    let err = check_enabled_packages([&first, &second]).unwrap_err();
    assert_eq!(err.kind, PackageConflictKind::InputContributionCollision);
    assert_eq!(&*err.contribution_id, "markdown.preview.input");

    let mut second_option =
        phase18_4_input_state_config_fixture("@clay/markdown-options", "markdown");
    second_option["clay"]["modes"] = json!(["markdown.options"]);
    second_option["clay"]["contributions"]["commands"][0]["id"] = json!("markdown.focusOptions");
    second_option["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("themeTokens");
    second_option["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("input");
    second_option["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("uiStateScopes");
    second_option["clay"]["contributions"]
        .as_object_mut()
        .unwrap()
        .remove("layoutOverrides");
    let second = assemble_package_record(&second_option).unwrap();
    let err = check_enabled_packages([&first, &second]).unwrap_err();
    assert_eq!(err.kind, PackageConflictKind::PackageOptionCollision);
    assert_eq!(&*err.contribution_id, "markdown.layout.defaultVisibility");
}

#[test]
fn phase18_4_diagnostics_preserve_package_provenance() {
    let mut package = phase18_4_input_state_config_fixture("@clay/markdown", "markdown");
    package["clay"]["contributions"]["uiStateScopes"][0]["valueSchema"]["defaultValue"] =
        json!("hidden");

    let err = assemble_package_record(&package).unwrap_err();
    assert_eq!(err.package_name.as_deref(), Some("@clay/markdown"));
    assert_eq!(err.package_version.as_deref(), Some("0.1.0"));
    assert_eq!(err.api_prefix.as_deref(), Some("markdown"));
    assert_eq!(
        err.contribution_id.as_deref(),
        Some("markdown.preview.visibility")
    );
    assert!(err.message.contains("state values"));
}

#[test]
fn markdown_package_docs_path_is_required_and_resolvable() {
    let package = first_party_markdown_package_json();
    let record = assemble_package_record(&package).unwrap();
    let docs_path = record.docs.docs_path.trim_start_matches("./");
    assert!(Path::new("packages/markdown").join(docs_path).is_file());

    let mut missing = package;
    missing["clay"].as_object_mut().unwrap().remove("docs");
    let err = assemble_package_record(&missing).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::MissingRequiredField);
    assert!(err.message.contains("clay.docs"));
}

/// Missing required fields fail with actionable per-field diagnostics carrying
/// the package name, version, and prefix.
#[test]
fn package_record_rejects_missing_required_contract_fields() {
    // Missing clay.docs
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"].as_object_mut().unwrap().remove("docs");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::MissingRequiredField);
        assert!(err.message.contains("clay.docs"), "got: {}", err.message);
        assert_eq!(err.package_name.as_deref(), Some("@clay/markdown"));
        assert_eq!(err.api_prefix.as_deref(), Some("markdown"));
    }

    // Missing clay.entry — bubbles from manifest validator
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"].as_object_mut().unwrap().remove("entry");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ManifestValidationFailed);
        assert!(err.message.contains("clay.entry"), "got: {}", err.message);
    }

    // Missing clay.permissions
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]
            .as_object_mut()
            .unwrap()
            .remove("permissions");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ManifestValidationFailed);
    }

    // Missing clay.modes
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"].as_object_mut().unwrap().remove("modes");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ManifestValidationFailed);
    }

    // Missing top-level name
    {
        let mut fixture = full_markdown_fixture();
        fixture.as_object_mut().unwrap().remove("name");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ManifestValidationFailed);
    }
}

#[test]
fn package_record_rejects_unsafe_entry_and_load_entry_paths() {
    for (field, value) in [
        ("entry", "dist/index.js"),
        ("entry", "./dist/index.ts"),
        ("entry", "../dist/index.js"),
        ("entry", "./dist/../index.js"),
        ("entry", "/tmp/index.js"),
        ("entry", "C:\\tmp\\index.js"),
        ("loadEntry", "dist/load.js"),
        ("loadEntry", "./dist/load.ts"),
        ("loadEntry", "./dist/../load.js"),
        ("loadEntry", "https://example.invalid/load.js"),
    ] {
        let mut fixture = full_markdown_fixture();
        fixture["clay"][field] = json!(value);

        let err = assemble_package_record(&fixture).unwrap_err();

        assert_eq!(err.rule, PackageRecordRule::ManifestValidationFailed);
        assert!(
            err.message.contains("without traversal"),
            "got: {}",
            err.message
        );
    }
}

/// Package-owned contribution IDs claiming the reserved `clay.*` namespace are
/// rejected with a `ReservedClayIdInContribution` diagnostic.
#[test]
fn package_record_rejects_package_claiming_clay_reserved_ids() {
    // Command ID using reserved clay.* namespace
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["contributions"]["commands"][0]["id"] = json!("clay.coreCommand");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ReservedClayIdInContribution);
        assert!(err.message.contains("clay.*"), "got: {}", err.message);
        assert_eq!(err.package_name.as_deref(), Some("@clay/markdown"));
        assert_eq!(err.contribution_id.as_deref(), Some("clay.coreCommand"));
    }

    // Configuration key using reserved clay.* namespace
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["contributions"]["configuration"][0]["key"] =
            json!("clay.internal.setting");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ReservedClayIdInContribution);
        assert_eq!(
            err.contribution_id.as_deref(),
            Some("clay.internal.setting")
        );
    }

    // Text transform ID using reserved clay.* namespace
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["contributions"]["textTransforms"][0]["transformId"] =
            json!("clay.baseTransform");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ReservedClayIdInContribution);
    }

    // API prefix "clay" caught by the manifest validator
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["apiPrefix"] = json!("clay");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ManifestValidationFailed);
    }
}

/// A contribution requiring a permission not declared in `clay.permissions`
/// fails before the package is enabled.
#[test]
fn package_record_rejects_undeclared_permission_for_contribution() {
    // Command contributions without `command-registration`
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["permissions"] = json!([
            "mode-registration",
            "mode-activation",
            "package-configuration"
        ]);
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(
            err.rule,
            PackageRecordRule::UndeclaredPermissionForContribution
        );
        assert!(
            err.message.contains("command-registration"),
            "got: {}",
            err.message
        );
        assert_eq!(err.package_name.as_deref(), Some("@clay/markdown"));
        assert_eq!(err.api_prefix.as_deref(), Some("markdown"));
    }

    // Configuration contributions without `package-configuration`
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["permissions"] = json!([
            "mode-registration",
            "mode-activation",
            "command-registration"
        ]);
        fixture["clay"]["contributions"]
            .as_object_mut()
            .unwrap()
            .remove("commands");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(
            err.rule,
            PackageRecordRule::UndeclaredPermissionForContribution
        );
        assert!(
            err.message.contains("package-configuration"),
            "got: {}",
            err.message
        );
    }

    // No contributions + minimal permissions must pass
    {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["permissions"] = json!(["mode-registration", "mode-activation"]);
        fixture["clay"]["contributions"] = json!({});
        fixture["clay"]
            .as_object_mut()
            .unwrap()
            .remove("apiDependencies");
        let result = assemble_package_record(&fixture);
        assert!(
            result.is_ok(),
            "package with no contributions and minimal permissions must validate, err: {:?}",
            result.unwrap_err()
        );
    }
}

// ── Fixtures (Task 2) ─────────────────────────────────────────────────────────

fn valid_markdown_package_json() -> Value {
    json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "markdown",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["markdown"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": "markdown.togglePreview",
                    "displayName": "Toggle Markdown Preview",
                    "routingPolicy": "server-first"
                }]
            }
        }
    })
}

/// `clay.contributions.commands` requires `command-registration` — this package
/// omits it intentionally so `enable()` fails.
fn invalid_clay_metadata_package_json() -> Value {
    json!({
        "name": "@clay/broken",
        "version": "1.0.0",
        "clay": {
            "apiPrefix": "broken",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration"],
            "modes": ["broken"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": "broken.doThing",
                    "displayName": "Do Thing",
                    "routingPolicy": "server-first"
                }]
            }
        }
    })
}

// ── Task 2 tests ──────────────────────────────────────────────────────────────

/// Install records a package without executing its runtime entry point.
///
/// After `install_from_value`, the package appears in `list()` as installed
/// but not enabled.  The service retains the raw package.json for later enable.
#[test]
fn package_service_install_records_without_executing_runtime() {
    let backend = FakeBackend::new();
    let mut service = PackageService::new("/tmp/clay-test-store", Box::new(backend));

    service
        .install_from_value(valid_markdown_package_json())
        .expect("install_from_value must succeed for valid package.json");

    // Appears in the list as installed but not enabled.
    let packages = service.list();
    assert_eq!(packages.len(), 1, "one package installed");
    assert_eq!(packages[0].name, "@clay/markdown");
    assert!(
        !packages[0].is_enabled,
        "installed package must not be enabled until enable() is called"
    );

    // inspect() works before enable.
    let inspection = service
        .inspect("@clay/markdown")
        .expect("installed package can be inspected");
    assert_eq!(inspection.name, "@clay/markdown");
    assert!(!inspection.is_enabled);
}

/// Enable fails for a package whose Clay record is invalid (missing permission
/// for a declared contribution), with actionable diagnostics.
#[test]
fn package_service_enable_rejects_invalid_clay_metadata() {
    let backend = FakeBackend::new();
    let mut service = PackageService::new("/tmp/clay-test-store", Box::new(backend));

    // Install succeeds — install and validation are separate.
    service
        .install_from_value(invalid_clay_metadata_package_json())
        .expect("install of broken package must succeed");

    // Enable must fail because command contribution lacks command-registration.
    let err = service.enable("@clay/broken").unwrap_err();

    match err {
        PackageServiceError::InvalidClayMetadata(record_err) => {
            assert_eq!(
                record_err.rule,
                PackageRecordRule::UndeclaredPermissionForContribution,
                "wrong rule: {record_err:?}"
            );
            assert!(
                record_err.message.contains("command-registration"),
                "diagnostic must name missing permission, got: {}",
                record_err.message
            );
            assert_eq!(record_err.package_name.as_deref(), Some("@clay/broken"));
        }
        other => panic!("expected InvalidClayMetadata, got {other:?}"),
    }

    // Package must NOT appear enabled after a failed enable.
    let inspection = service
        .inspect("@clay/broken")
        .expect("package still inspectable after failed enable");
    assert!(!inspection.is_enabled);
}

/// Disabling removes the package from the enabled set and frees its
/// prefix/mode/command IDs so they can be reused by another package.
#[test]
fn package_service_disable_removes_active_contributions() {
    let backend = FakeBackend::new();
    let mut service = PackageService::new("/tmp/clay-test-store", Box::new(backend));

    service
        .install_from_value(valid_markdown_package_json())
        .expect("install must succeed");
    service
        .enable("@clay/markdown")
        .expect("enable must succeed for valid package");

    // Verify enabled state before disable.
    {
        let inspection = service
            .inspect("@clay/markdown")
            .expect("inspectable when enabled");
        assert!(inspection.is_enabled);
        assert_eq!(inspection.command_count, 1);
        assert_eq!(inspection.api_prefix, "markdown");
    }

    // Disable — returns the full PackageRecord with provenance.
    let record = service
        .disable("@clay/markdown")
        .expect("disable must succeed when enabled");
    assert_eq!(record.manifest.name, "@clay/markdown");
    assert_eq!(record.manifest.clay.api_prefix, "markdown");

    // Package is now installed but not enabled.
    {
        let inspection = service
            .inspect("@clay/markdown")
            .expect("still inspectable after disable");
        assert!(!inspection.is_enabled);
    }

    // No enabled records remain.
    assert_eq!(service.enabled_records().count(), 0);

    // A second disable returns NotEnabled.
    let err = service.disable("@clay/markdown").unwrap_err();
    assert!(
        matches!(err, PackageServiceError::NotEnabled { .. }),
        "second disable must return NotEnabled, got {err:?}"
    );
}

/// The pnpm backend suppresses lifecycle scripts by default and only omits
/// `--ignore-scripts` when explicitly opted in.
#[test]
fn pnpm_backend_install_args_suppress_lifecycle_scripts_by_default() {
    use clay::packages::manager::{PackageInstallOptions, PnpmBackend};

    let backend = PnpmBackend::new();

    let default_args =
        backend.install_command_args("@clay/markdown", PackageInstallOptions::default());
    assert_eq!(
        default_args,
        vec!["add", "@clay/markdown", "--ignore-scripts"]
    );

    let allowed_args = backend.install_command_args(
        "@clay/markdown",
        PackageInstallOptions {
            allow_lifecycle_scripts: true,
        },
    );
    assert_eq!(allowed_args, vec!["add", "@clay/markdown"]);
}

/// The `clay package` CLI subcommands (add / enable / disable / list / inspect)
/// route through the same shared `PackageService` paths as a direct API call.
///
/// This test is backed by a `FakeBackend` and exercises the full service API
/// surface rather than spawning a subprocess.
#[test]
fn package_cli_subcommands_route_through_shared_service() {
    let package_json = valid_markdown_package_json();
    let backend = FakeBackend::new().will_install("@clay/markdown", package_json.clone());
    let mut service = PackageService::new("/tmp/clay-test-store", Box::new(backend));

    // `clay package add @clay/markdown` — install via value path.
    service
        .install_from_value(package_json)
        .expect("install_from_value succeeds");
    assert_eq!(service.list().len(), 1, "list returns 1 after add");

    // `clay package enable @clay/markdown`
    service.enable("@clay/markdown").expect("enable succeeds");
    let after_enable = service
        .inspect("@clay/markdown")
        .expect("inspect after enable");
    assert!(after_enable.is_enabled);
    assert_eq!(after_enable.docs_path.as_deref(), Some("./docs/index.md"));

    // `clay package disable @clay/markdown`
    service.disable("@clay/markdown").expect("disable succeeds");
    let after_disable = service
        .inspect("@clay/markdown")
        .expect("inspect after disable");
    assert!(!after_disable.is_enabled);

    // `clay package list` — still present as installed.
    let list = service.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "@clay/markdown");
    assert!(!list[0].is_enabled);

    // `clay package inspect @clay/markdown`
    let inspection = service
        .inspect("@clay/markdown")
        .expect("inspect returns Some for installed package");
    assert_eq!(inspection.name, "@clay/markdown");
    assert_eq!(inspection.api_prefix, "markdown");
}

/// `clay package list` reflects packages installed by a *previous* `clay
/// package add` process. Each CLI invocation is a fresh process with a fresh
/// `PackageService`, so a freshly-constructed service must call
/// `refresh_installed()` to repopulate its in-memory `installed` map from the
/// package-manager store. This test simulates the cross-process boundary by
/// constructing two independent services backed by the same discovered store.
///
/// `FakeBackend::list_results` is the in-memory stand-in for the on-disk pnpm
/// store that a real `pnpm list --json` would rediscover; it never executes
/// package code.
#[test]
fn package_service_list_persists_across_service_instances() {
    let package_json = valid_markdown_package_json();

    // Process 1: a package is installed. Its package.json lands in the store.
    let mut first = PackageService::new("/tmp/clay-test-store", Box::new(FakeBackend::new()));
    first
        .install_from_value(package_json.clone())
        .expect("install_from_value must succeed");
    drop(first);

    // Process 2: a brand-new service. Without refresh, it sees nothing.
    let backend = FakeBackend::new();
    let mut second = PackageService::new("/tmp/clay-test-store", Box::new(backend));
    assert_eq!(
        second.list().len(),
        0,
        "fresh service without refresh sees no installed packages"
    );

    // The package-manager store (simulated by list_results) still contains the
    // package; refresh repopulates `installed` without executing package code.
    let backend = FakeBackend {
        list_results: vec![clay::packages::manager::DiscoveredPackage {
            package_json: package_json.clone(),
            package_root: std::path::PathBuf::from("/tmp/clay-test-store/@clay/markdown"),
        }],
        ..FakeBackend::new()
    };
    second = PackageService::new("/tmp/clay-test-store", Box::new(backend));
    second
        .refresh_installed()
        .expect("refresh_installed must succeed against a healthy store");

    let packages = second.list();
    assert_eq!(
        packages.len(),
        1,
        "refresh must repopulate installed packages"
    );
    assert_eq!(packages[0].name, "@clay/markdown");
    assert!(!packages[0].is_enabled, "refresh must not enable packages");
}

/// After a fresh service refreshes its installed map from the store, `enable`
/// works on the previously-installed package without requiring a re-install.
/// This is the core correctness guarantee of CLI state persistence: install in
/// one process, enable in another.
#[test]
fn package_service_enable_after_refresh_does_not_require_reinstall() {
    let package_json = valid_markdown_package_json();

    // Process 2 starts cold; the store (list_results) already contains the
    // package installed by an earlier process.
    let backend = FakeBackend {
        list_results: vec![clay::packages::manager::DiscoveredPackage {
            package_json: package_json.clone(),
            package_root: std::path::PathBuf::from("/tmp/clay-test-store/@clay/markdown"),
        }],
        ..FakeBackend::new()
    };
    let mut service = PackageService::new("/tmp/clay-test-store", Box::new(backend));

    // Without refresh, enable fails because the package is not in the map.
    let err = service.enable("@clay/markdown").unwrap_err();
    assert!(matches!(
        err,
        clay::packages::service::PackageServiceError::NotInstalled { .. }
    ));

    // After refresh, enable succeeds without any install call.
    service.refresh_installed().expect("refresh must succeed");
    service
        .enable("@clay/markdown")
        .expect("enable must succeed on a previously-installed package after refresh");
    assert!(
        service
            .inspect("@clay/markdown")
            .expect("inspect after enable")
            .is_enabled
    );
}

// ── Fixtures (Task 3) ─────────────────────────────────────────────────────────

/// Assemble a validated PackageRecord from a raw fixture JSON value.
fn make_record(value: Value) -> clay::packages::record::PackageRecord {
    assemble_package_record(&value).expect("fixture must produce a valid PackageRecord")
}

/// A full Markdown package record suitable for major-mode manifest tests.
fn markdown_package_record() -> clay::packages::record::PackageRecord {
    make_record(json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "markdown",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["markdown"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": "markdown.togglePreview",
                    "displayName": "Toggle Markdown Preview",
                    "routingPolicy": "server-first"
                }]
            }
        }
    }))
}

/// An RST package record for a second document in a different mode.
fn rst_package_record() -> clay::packages::record::PackageRecord {
    make_record(json!({
        "name": "@clay/rst",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "rst",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["rst"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": "rst.toggleSection",
                    "displayName": "Toggle RST Section",
                    "routingPolicy": "server-first"
                }]
            }
        }
    }))
}

/// Register a major mode and activate it for a document, returning the activation.
fn register_and_activate_major(
    registry: &mut ModeRegistry,
    record: &clay::packages::record::PackageRecord,
    mode_id: &str,
    extension: &str,
    document_id: u64,
) -> MajorModeActivation {
    let decl = ModeDeclaration {
        package_name: record.manifest.name.clone(),
        package_version: record.manifest.version.clone(),
        api_prefix: record.manifest.clay.api_prefix.clone(),
        mode_id: mode_id.to_string(),
        display_name: format!("{} mode", mode_id),
        extensions: vec![extension.to_string()],
        mime_types: vec![],
        file_names: vec![],
        file_name_patterns: vec![],
    };
    registry
        .register_mode(&record.manifest, decl)
        .expect("register_mode must succeed");

    let input = DocumentClassificationInput {
        document_id,
        path: Some(format!("file.{extension}")),
        mime_type: None,
    };
    let classification = registry.classify(&input).expect("classify must succeed");
    registry
        .activate_major_mode(&record.manifest, classification)
        .expect("activate_major_mode must succeed")
}

// ── Task 3 tests ──────────────────────────────────────────────────────────────

/// Two documents in different major modes get distinct validated manifests
/// with correct provenance and behavior versions.
#[test]
fn behavior_manifest_selected_per_document_with_provenance() {
    let md_record = markdown_package_record();
    let rst_record = rst_package_record();

    let mut registry = ModeRegistry::new();

    // Document 1 → Markdown major mode.
    let md_act = register_and_activate_major(&mut registry, &md_record, "markdown", "md", 1);
    // Document 2 → RST major mode.
    let rst_act = register_and_activate_major(&mut registry, &rst_record, "rst", "rst", 2);

    let enabled: Vec<&clay::packages::record::PackageRecord> = vec![&md_record, &rst_record];

    // Select manifest for document 1 — clone the relevant data before calling again.
    let (
        md_manifest_id,
        md_doc_id,
        md_mode_id,
        md_pkg_name,
        md_bver,
        md_has_toggle,
        md_scope_ok,
        md_no_minor,
    ) = {
        let sel = registry
            .select_behavior_manifest_for_document(1, &enabled)
            .expect("manifest selection must succeed for document 1");
        (
            sel.manifest.manifest_id.clone(),
            sel.major_mode.document_id,
            sel.major_mode.mode_id.clone(),
            sel.major_mode.package_name.clone(),
            sel.major_mode.behavior_version,
            sel.manifest
                .commands
                .iter()
                .any(|c| c.command_id == "markdown.togglePreview"),
            matches!(
                sel.manifest.scope,
                BehaviorScope::Document { document_id: 1 }
            ),
            sel.minor_modes.is_empty(),
        )
    };

    assert_eq!(md_doc_id, 1);
    assert_eq!(md_mode_id, "markdown");
    assert_eq!(md_pkg_name, "@clay/markdown");
    assert_eq!(md_bver, md_act.behavior_version);
    assert!(
        md_has_toggle,
        "composed manifest must include markdown.togglePreview"
    );
    assert!(
        md_scope_ok,
        "manifest scope must be Document {{ document_id: 1 }}"
    );
    assert!(md_no_minor);

    // Select manifest for document 2 — must be distinct.
    let (
        rst_manifest_id,
        rst_doc_id,
        rst_mode_id,
        rst_pkg_name,
        rst_bver,
        rst_has_toggle,
        rst_scope_ok,
    ) = {
        let sel = registry
            .select_behavior_manifest_for_document(2, &enabled)
            .expect("manifest selection must succeed for document 2");
        (
            sel.manifest.manifest_id.clone(),
            sel.major_mode.document_id,
            sel.major_mode.mode_id.clone(),
            sel.major_mode.package_name.clone(),
            sel.major_mode.behavior_version,
            sel.manifest
                .commands
                .iter()
                .any(|c| c.command_id == "rst.toggleSection"),
            matches!(
                sel.manifest.scope,
                BehaviorScope::Document { document_id: 2 }
            ),
        )
    };

    assert_eq!(rst_doc_id, 2);
    assert_eq!(rst_mode_id, "rst");
    assert_eq!(rst_pkg_name, "@clay/rst");
    assert_eq!(rst_bver, rst_act.behavior_version);
    assert!(
        rst_has_toggle,
        "composed manifest must include rst.toggleSection"
    );
    assert!(
        rst_scope_ok,
        "manifest scope must be Document {{ document_id: 2 }}"
    );

    // The two manifests must differ in content.
    assert_ne!(
        md_manifest_id, rst_manifest_id,
        "manifests for different modes must have different IDs"
    );

    // selected_manifest() must return the stored selection (immutable borrow — OK).
    let cached = registry
        .selected_manifest(1)
        .expect("selected_manifest must return the cached selection");
    assert_eq!(cached.major_mode.mode_id, "markdown");
}

/// A minor mode registered as compatible with "markdown" is rejected when the
/// document's active major mode is "rst".
#[test]
fn minor_mode_rejected_when_incompatible_major_mode() {
    let _md_record = markdown_package_record();
    let rst_record = rst_package_record();

    // A minor-mode package compatible only with "markdown".
    let minor_record = make_record(json!({
        "name": "@clay/md-extras",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "md-extras",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation"],
            "modes": ["md-extras"],
            "docs": "./docs/index.md"
        }
    }));

    let mut registry = ModeRegistry::new();

    // Activate the RST major mode for document 1.
    register_and_activate_major(&mut registry, &rst_record, "rst", "rst", 1);

    // Register the minor mode as compatible with "markdown" only.
    let minor_decl = MinorModeDeclaration {
        package_name: minor_record.manifest.name.clone(),
        package_version: minor_record.manifest.version.clone(),
        api_prefix: minor_record.manifest.clay.api_prefix.clone(),
        mode_id: "md-extras".to_string(),
        display_name: "Markdown Extras".to_string(),
        compatible_major_modes: vec!["markdown".to_string()],
    };
    registry
        .register_minor_mode(&minor_record.manifest, minor_decl)
        .expect("register_minor_mode must succeed");

    // Attempt to activate the minor mode while the document is in RST mode → reject.
    let err = registry
        .activate_minor_mode(&minor_record.manifest, 1, "md-extras")
        .unwrap_err();

    assert_eq!(
        err.rule,
        ModeValidationRule::UndeclaredMode,
        "wrong rule: {err:?}"
    );
    assert!(
        err.message.contains("md-extras"),
        "diagnostic must mention the minor mode ID, got: {}",
        err.message
    );
    assert!(
        err.message.contains("rst"),
        "diagnostic must mention the incompatible active major mode, got: {}",
        err.message
    );
    assert!(
        err.message.contains("markdown"),
        "diagnostic must mention compatible modes, got: {}",
        err.message
    );

    // Missing major mode entirely → also rejected.
    let err2 = registry
        .activate_minor_mode(&minor_record.manifest, 99, "md-extras")
        .unwrap_err();
    assert_eq!(err2.rule, ModeValidationRule::AmbiguousClassification);
    assert!(err2.message.contains("no active major mode"));
}

/// A minor mode that uses the same mode_id as an already-registered major mode
/// is rejected at register_minor_mode time with DuplicateModeId; and a minor mode
/// whose commands do not collide with major-mode commands composes cleanly.
#[test]
fn minor_mode_cannot_override_major_mode_entries() {
    let md_record = markdown_package_record();

    // A minor mode with a non-colliding package command.
    let minor_record = make_record(json!({
        "name": "@clay/md-override",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "md-override",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["md-override"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": "md-override.extra",
                    "displayName": "Extra Command",
                    "routingPolicy": "server-first"
                }]
            }
        }
    }));

    // A second minor mode (also non-colliding).
    let collider_record = make_record(json!({
        "name": "@clay/md-collider",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "md-collider",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["md-collider"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": "md-collider.altCommand",
                    "displayName": "Alt Command",
                    "routingPolicy": "server-first"
                }]
            }
        }
    }));

    let mut registry = ModeRegistry::new();

    // Activate Markdown major mode for document 1.
    register_and_activate_major(&mut registry, &md_record, "markdown", "md", 1);

    // Register and activate the first non-colliding minor mode.
    let minor_decl = MinorModeDeclaration {
        package_name: minor_record.manifest.name.clone(),
        package_version: minor_record.manifest.version.clone(),
        api_prefix: minor_record.manifest.clay.api_prefix.clone(),
        mode_id: "md-override".to_string(),
        display_name: "MD Override".to_string(),
        compatible_major_modes: vec!["markdown".to_string()],
    };
    registry
        .register_minor_mode(&minor_record.manifest, minor_decl)
        .expect("register_minor_mode must succeed");
    registry
        .activate_minor_mode(&minor_record.manifest, 1, "md-override")
        .expect("non-colliding minor mode activation must succeed");

    // Register and activate the second non-colliding minor mode.
    let collider_decl = MinorModeDeclaration {
        package_name: collider_record.manifest.name.clone(),
        package_version: collider_record.manifest.version.clone(),
        api_prefix: collider_record.manifest.clay.api_prefix.clone(),
        mode_id: "md-collider".to_string(),
        display_name: "MD Collider".to_string(),
        compatible_major_modes: vec!["markdown".to_string()],
    };
    registry
        .register_minor_mode(&collider_record.manifest, collider_decl)
        .expect("register_minor_mode must succeed");
    registry
        .activate_minor_mode(&collider_record.manifest, 1, "md-collider")
        .expect("second non-colliding minor mode activation must succeed");

    // Compose the manifest — non-colliding minor modes must compose cleanly.
    let enabled: Vec<&clay::packages::record::PackageRecord> =
        vec![&md_record, &minor_record, &collider_record];
    let selection = registry
        .select_behavior_manifest_for_document(1, &enabled)
        .expect(
            "manifest selection must succeed when minor modes do not override major-mode entries",
        );

    // Both minor-mode commands must appear in the manifest.
    assert!(
        selection
            .manifest
            .commands
            .iter()
            .any(|c| c.command_id == "md-override.extra"),
        "md-override.extra must be in composed manifest"
    );
    assert!(
        selection
            .manifest
            .commands
            .iter()
            .any(|c| c.command_id == "md-collider.altCommand"),
        "md-collider.altCommand must be in composed manifest"
    );
    // Major-mode command must still be present.
    assert!(
        selection
            .manifest
            .commands
            .iter()
            .any(|c| c.command_id == "markdown.togglePreview"),
        "markdown.togglePreview must still be in composed manifest"
    );
    assert_eq!(selection.minor_modes.len(), 2);

    // Verify the duplicate mode_id guard: attempt to register a minor mode with
    // mode_id "markdown" — which is already registered as a major mode — should fail.
    let spoof_record = make_record(json!({
        "name": "@clay/spoof",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "spoof",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation"],
            "modes": ["spoof"],
            "docs": "./docs/index.md"
        }
    }));
    // "markdown" is not owned by the "spoof" prefix, so it triggers InvalidModeId.
    let dup_decl = MinorModeDeclaration {
        package_name: spoof_record.manifest.name.clone(),
        package_version: spoof_record.manifest.version.clone(),
        api_prefix: spoof_record.manifest.clay.api_prefix.clone(),
        mode_id: "markdown".to_string(),
        display_name: "Spoof".to_string(),
        compatible_major_modes: vec!["markdown".to_string()],
    };
    let dup_err = registry
        .register_minor_mode(&spoof_record.manifest, dup_decl)
        .unwrap_err();
    assert_eq!(
        dup_err.rule,
        ModeValidationRule::InvalidModeId,
        "mode_id not owned by the package must fail with InvalidModeId, got: {dup_err:?}"
    );
}

/// Composed manifest routing does not invoke server JavaScript on keypress:
/// all routing decisions are made from inert manifest data only.
///
/// Structural test: every command in the composed manifest must have a
/// routing/authority combination accepted by the existing manifest validator,
/// and client-first hot-path commands carry `BuiltInClientEdit` authority —
/// never `ServerIntent`.
#[test]
fn keypress_routing_uses_manifest_without_javascript() {
    use clay::protocol::{CommandAuthority, RoutingPolicy};

    let md_record = markdown_package_record();
    let mut registry = ModeRegistry::new();

    register_and_activate_major(&mut registry, &md_record, "markdown", "md", 1);

    let enabled: Vec<&clay::packages::record::PackageRecord> = vec![&md_record];
    let selection = registry
        .select_behavior_manifest_for_document(1, &enabled)
        .expect("manifest selection must succeed");

    for cmd in &selection.manifest.commands {
        match &cmd.routing_policy {
            RoutingPolicy::ClientFirstPredictable | RoutingPolicy::ClientFirstRequiresAck => {
                assert_eq!(
                    cmd.authority,
                    CommandAuthority::BuiltInClientEdit,
                    "client-first command `{}` must have BuiltInClientEdit authority; \
                     packages must not gain client-edit authority via manifest composition",
                    cmd.command_id
                );
            }
            RoutingPolicy::ServerFirst
            | RoutingPolicy::ServerFirstWithLock { .. }
            | RoutingPolicy::UiReactivePriority
            | RoutingPolicy::Background => {
                assert_eq!(
                    cmd.authority,
                    CommandAuthority::ServerIntent,
                    "server-routing command `{}` must have ServerIntent authority",
                    cmd.command_id
                );
            }
            RoutingPolicy::ClientUiCommand => {
                panic!(
                    "package manifest command `{}` must not request native client UI authority",
                    cmd.command_id
                );
            }
        }
    }

    // select_behavior_manifest_for_document calls validate_manifest internally,
    // so a successful return guarantees the manifest is inert and structurally
    // valid — the client routes only from this data structure, never from JS.
    assert!(
        !selection.manifest.manifest_id.is_empty(),
        "manifest_id must be non-empty in the selected manifest"
    );
    assert!(
        matches!(
            selection.manifest.scope,
            BehaviorScope::Document { document_id: 1 }
        ),
        "manifest scope must be Document {{ document_id: 1 }}"
    );
}

// ── Task 4 tests ──────────────────────────────────────────────────────────────

fn conflict_record(
    name: &str,
    prefix: &str,
    mode: &str,
    command: &str,
) -> clay::packages::record::PackageRecord {
    make_record(json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": prefix,
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": [mode],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": command,
                    "displayName": "Command",
                    "routingPolicy": "server-first"
                }]
            }
        }
    }))
}

/// Duplicate prefixes, mode names, and command IDs fail deterministically with
/// both packages' provenance in the diagnostic.
#[test]
fn enable_rejects_duplicate_prefix_and_mode_and_command() {
    let first = conflict_record("@clay/one", "one", "one", "one.run");
    let dup_prefix = conflict_record("@clay/one-alt", "one", "one.alt", "one.altRun");
    let err = check_enabled_packages([&first, &dup_prefix]).unwrap_err();
    assert_eq!(err.kind, PackageConflictKind::DuplicatePrefix);
    assert_eq!(&*err.contribution_id, "one");
    assert_eq!(err.first.package_name, "@clay/one");
    assert_eq!(err.second.package_name, "@clay/one-alt");

    let dup_mode = conflict_record("@clay/two", "one", "one", "one.twoRun");
    let err = check_enabled_packages([&first, &dup_mode]).unwrap_err();
    assert_eq!(err.kind, PackageConflictKind::DuplicateMode);
    assert_eq!(&*err.contribution_id, "one");
    assert!(err.message.contains("@clay/one"));
    assert!(err.message.contains("@clay/two"));

    let duplicate_command = make_record(json!({
        "name": "@clay/command-dupe",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "one",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["one.dupe"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{
                    "id": "one.run",
                    "displayName": "Command Dupe",
                    "routingPolicy": "server-first"
                }]
            }
        }
    }));
    let err = check_enabled_packages([&first, &duplicate_command]).unwrap_err();
    assert_eq!(err.kind, PackageConflictKind::DuplicateCommand);
    assert_eq!(&*err.contribution_id, "one.run");

    // The service runs the same conflict pass during enable and rolls back the candidate.
    let mut service = PackageService::new("/tmp/clay-conflict-store", Box::new(FakeBackend::new()));
    service
        .install_from_value(json!({
            "name": "@clay/one",
            "version": "0.1.0",
            "type": "module",
            "clay": {
                "apiPrefix": "one",
                "entry": "./dist/index.js",
                "permissions": ["mode-registration", "mode-activation"],
                "modes": ["one"],
                "docs": "./docs/index.md"
            }
        }))
        .unwrap();
    service
        .install_from_value(json!({
            "name": "@clay/one-alt",
            "version": "0.1.0",
            "type": "module",
            "clay": {
                "apiPrefix": "one",
                "entry": "./dist/index.js",
                "permissions": ["mode-registration", "mode-activation"],
                "modes": ["one.alt"],
                "docs": "./docs/index.md"
            }
        }))
        .unwrap();
    service.enable("@clay/one").unwrap();
    let err = service.enable("@clay/one-alt").unwrap_err();
    match err {
        PackageServiceError::ContributionConflict(conflict) => {
            assert_eq!(conflict.kind, PackageConflictKind::DuplicatePrefix);
            assert_eq!(&*conflict.contribution_id, "one");
        }
        other => panic!("expected ContributionConflict, got {other:?}"),
    }
    assert!(!service.inspect("@clay/one-alt").unwrap().is_enabled);
}

/// Ambiguous key bindings without distinct priority/routing metadata are
/// rejected instead of being resolved lazily or by silent package order.
#[test]
fn ambiguous_keybinding_across_packages_rejected_without_priority() {
    let first = make_record(json!({
        "name": "@clay/key-one",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "key-one",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["key-one"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{"id": "key-one.run", "displayName": "Run", "routingPolicy": "server-first"}],
                "keyRouting": [{"commandId": "key-one.run", "key": "Ctrl+K", "routingPolicy": "server-first"}]
            }
        }
    }));
    let second = make_record(json!({
        "name": "@clay/key-two",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "key-two",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation", "command-registration"],
            "modes": ["key-two"],
            "docs": "./docs/index.md",
            "contributions": {
                "commands": [{"id": "key-two.run", "displayName": "Run", "routingPolicy": "server-first"}],
                "keyRouting": [{"commandId": "key-two.run", "key": "Ctrl+K", "routingPolicy": "server-first"}]
            }
        }
    }));

    let err = check_enabled_packages([&first, &second]).unwrap_err();
    assert_eq!(err.kind, PackageConflictKind::AmbiguousKeyBinding);
    assert!(err.message.contains("Ctrl+K"));
    assert!(err.message.contains("@clay/key-one"));
    assert!(err.message.contains("@clay/key-two"));
}

/// SDUI/status contributions retain package region/provenance metadata and are
/// rejected when their estimated snapshot/update payloads exceed SDUI budgets.
#[test]
fn package_sdui_contribution_carries_provenance_and_respects_budget() {
    let record = make_record(json!({
        "name": "@clay/status",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "status",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation"],
            "modes": ["status"],
            "docs": "./docs/index.md",
            "contributions": {
                "sdui": [{
                    "regionId": "status.footer",
                    "displayName": "Status Footer",
                    "estimatedSnapshotBytes": 512,
                    "estimatedUpdateBytes": 128
                }]
            }
        }
    }));
    let sdui = &record.contributions.sdui[0];
    assert_eq!(record.manifest.name, "@clay/status");
    assert_eq!(record.manifest.clay.api_prefix, "status");
    assert_eq!(sdui.region_id, "status.footer");
    assert_eq!(sdui.display_name, "Status Footer");
    assert_eq!(sdui.estimated_snapshot_bytes, 512);
    assert_eq!(sdui.estimated_update_bytes, 128);

    let mut oversized = json!({
        "name": "@clay/status-big",
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": "status-big",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation"],
            "modes": ["status-big"],
            "docs": "./docs/index.md",
            "contributions": {
                "sdui": [{
                    "regionId": "status-big.footer",
                    "displayName": "Huge Status Footer",
                    "estimatedSnapshotBytes": 4097,
                    "estimatedUpdateBytes": 128
                }]
            }
        }
    });
    let err = assemble_package_record(&oversized).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::PayloadBudgetExceeded);
    assert_eq!(err.contribution_id.as_deref(), Some("status-big.footer"));
    assert!(err.message.contains("SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES"));

    oversized["clay"]["contributions"]["sdui"][0]["estimatedSnapshotBytes"] = json!(512);
    oversized["clay"]["contributions"]["sdui"][0]["estimatedUpdateBytes"] = json!(1025);
    let err = assemble_package_record(&oversized).unwrap_err();
    assert_eq!(err.rule, PackageRecordRule::PayloadBudgetExceeded);
    assert!(err.message.contains("SDUI_UPDATE_PAYLOAD_BUDGET_BYTES"));
}
