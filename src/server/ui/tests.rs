//! Unit tests for the package UI registry (`super`).
//!
//! Extracted from `ui.rs` by plan 133 task 5; the module is declared there as
//! `#[cfg(test)] mod tests;`.

use serde_json::json;

use super::*;
use crate::packages::{manifest::validate_manifest_value, permissions::PackagePermission};

fn package() -> ClayPackageManifest {
    validate_manifest_value(&json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "clay": {
            "apiPrefix": "markdown",
            "entry": "./dist/index.js",
            "permissions": ["command-registration"],
            "modes": ["markdown"]
        }
    }))
    .unwrap()
}

#[test]
fn component_icon_references_validate_namespace_and_kind() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec!["markdown.togglePreview".to_string()];

    // Core key and own-prefix keys accepted on supported kinds.
    let component = registry
        .register_component(
            &package,
            &json!({
                "kind": "button",
                "id": "markdown.preview.toggle",
                "label": "Toggle",
                "icon": "preview.toggle",
                "action": { "commandId": "markdown.togglePreview" }
            }),
            &commands,
        )
        .expect("core icon reference accepted");
    assert_eq!(component.root_kind, "button");

    registry
        .register_component(
            &package,
            &json!({
                "kind": "label",
                "id": "markdown.preview.eye",
                "text": "Preview",
                "icon": "markdown.eye"
            }),
            &commands,
        )
        .expect("own-prefix icon reference accepted");

    // Impersonating another package's namespace rejected.
    let error = registry
        .register_component(
            &package,
            &json!({
                "kind": "label",
                "id": "markdown.preview.steal",
                "text": "Steal",
                "icon": "git.branch.steal"
            }),
            &commands,
        )
        .unwrap_err();
    assert!(error.message.contains("namespace"));

    // Icons on unsupported kinds rejected.
    let error = registry
        .register_component(
            &package,
            &json!({
                "kind": "flex",
                "id": "markdown.preview.layout",
                "direction": "row",
                "icon": "preview.toggle",
                "children": []
            }),
            &commands,
        )
        .unwrap_err();
    assert!(
        error
            .message
            .contains("only supported by button, label, list, and statusItem")
    );

    // List item icons accepted through the items loop.
    registry
        .register_component(
            &package,
            &json!({
                "kind": "list",
                "id": "markdown.preview.rows",
                "items": [{
                    "id": "row",
                    "label": "Row",
                    "icon": "file.file"
                }]
            }),
            &commands,
        )
        .expect("list item icon reference accepted");
}

#[test]
fn ui_registry_accepts_valid_panel_component_overlay_and_theme_token() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec!["markdown.togglePreview".to_string()];

    let panel = registry
        .register_panel(
            &package,
            &json!({
                "id": "markdown.preview",
                "slot": "right",
                "kind": "fixed",
                "defaultVisibility": "hidden",
                "actionTargets": ["markdown.togglePreview"],
                "component": {
                    "kind": "panel",
                    "id": "markdown.preview.root",
                    "title": "Preview",
                    "children": [{
                        "kind": "button",
                        "id": "markdown.preview.toggle",
                        "label": "Toggle",
                        "action": { "commandId": "markdown.togglePreview" }
                    }]
                }
            }),
            &commands,
        )
        .unwrap();
    assert_eq!(panel.slot, "right");
    assert_eq!(panel.provenance.api_prefix, "markdown");

    let component = registry
        .register_component(
            &package,
            &json!({
                "kind": "label",
                "id": "markdown.preview.empty",
                "text": "Preview unavailable"
            }),
            &commands,
        )
        .unwrap();
    assert_eq!(component.root_kind, "label");

    let overlay = registry
        .register_overlay(
            &package,
            &json!({
                "id": "markdown.preview.quickOpen",
                "anchor": "working-area",
                "focusPolicy": "restore",
                "dismissalPolicy": "escape",
                "component": {
                    "kind": "panel",
                    "id": "markdown.preview.quickOpen.root",
                    "title": "Quick Open",
                    "children": []
                }
            }),
            &commands,
        )
        .unwrap();
    assert_eq!(overlay.focus_policy, "restore");

    let token = registry
        .register_theme_token(
            &package,
            &json!({
                "token": "markdown.preview.background",
                "type": "color-role",
                "fallback": "surface.panel",
                "description": "Markdown preview background"
            }),
        )
        .unwrap();
    assert_eq!(token.token_type, "color-role");
    assert_eq!(token.resolved_core_token, "surface.panel");

    let styled_component = registry
        .register_component(
            &package,
            &json!({
                "kind": "panel",
                "id": "markdown.preview.styled",
                "style": {
                    "background": "markdown.preview.background",
                    "padding": "spacing.panel",
                    "typography": "typography.body"
                },
                "children": []
            }),
            &commands,
        )
        .unwrap();
    assert_eq!(styled_component.style_variable_count, 3);
    let snapshot = registry.snapshot();
    assert_eq!(snapshot.panels.len(), 1);
    let mut runtime = crate::shell::PackageUiRuntimeState::new();
    runtime
        .apply_update(snapshot.runtime_update(0))
        .expect("registered package UI contributions should compose into runtime state");
    assert!(runtime.has_fixed_panels());
    assert!(runtime.fixed_panel_for_slot(FixedSlotId::Right).is_some());
    assert_eq!(runtime.transient_overlay_count(), 1);
    let wire = snapshot
        .wire_snapshot(8, |_| crate::protocol::PackageUiTrustDomain::Trusted)
        .expect("one pane-content winner");
    assert_eq!(wire.version, 8);
    assert_eq!(wire.panels.len(), 1);
    assert_eq!(wire.overlays.len(), 1);
    assert_eq!(wire.components.len(), 2);
    assert_eq!(
        wire.panels[0].provenance.trust_domain,
        crate::protocol::PackageUiTrustDomain::Trusted
    );
    assert!(serde_json::from_str::<Value>(&wire.panels[0].component_json).is_ok());
    assert!(
        package
            .clay
            .permissions
            .contains(&PackagePermission::CommandRegistration)
    );
}

#[test]
fn package_component_font_role_is_semantic_and_text_only() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = Vec::new();

    let accepted = registry
        .register_component(
            &package,
            &json!({
                "kind": "label",
                "id": "markdown.preview.code",
                "text": "cargo test",
                "style": { "fontRole": "monospace" }
            }),
            &commands,
        )
        .unwrap();
    assert_eq!(accepted.style_variable_count, 1);

    for style in [
        json!({ "fontRole": "serif" }),
        json!({ "fontFamily": "JetBrains Mono" }),
        json!({ "fontSize": 18 }),
    ] {
        assert!(
            registry
                .register_component(
                    &package,
                    &json!({
                        "kind": "label",
                        "id": format!("markdown.preview.invalid{}", registry.components.len()),
                        "text": "bad",
                        "style": style
                    }),
                    &commands,
                )
                .is_err()
        );
    }
    assert!(
        registry
            .register_component(
                &package,
                &json!({
                    "kind": "stack",
                    "id": "markdown.preview.stack",
                    "style": { "fontRole": "monospace" }
                }),
                &commands,
            )
            .is_err()
    );
}

#[test]
fn ui_registry_rejects_invalid_prefix_unregistered_actions_raw_css_and_duplicate_ids() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec!["markdown.togglePreview".to_string()];

    let invalid_prefix = registry
        .register_component(
            &package,
            &json!({ "kind": "label", "id": "other.preview", "text": "bad" }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(invalid_prefix.rule, UiContributionRule::InvalidId);

    let invalid_action = registry
            .register_panel(
                &package,
                &json!({
                    "id": "markdown.preview",
                    "slot": "right",
                    "component": { "kind": "button", "id": "markdown.preview.button", "label": "Run", "action": { "commandId": "markdown.missing" } }
                }),
                &commands,
            )
            .unwrap_err();
    assert_eq!(invalid_action.rule, UiContributionRule::InvalidActionTarget);

    let raw_css = registry
            .register_component(
                &package,
                &json!({ "kind": "label", "id": "markdown.preview.raw", "text": "bad", "style": "color: red" }),
                &commands,
            )
            .unwrap_err();
    assert_eq!(raw_css.rule, UiContributionRule::ProhibitedAuthority);

    registry
        .register_theme_token(
            &package,
            &json!({
                "token": "markdown.preview.background",
                "type": "color-role",
                "fallback": "surface.panel",
                "description": "Markdown preview background"
            }),
        )
        .unwrap();
    let duplicate = registry
        .register_theme_token(
            &package,
            &json!({
                "token": "markdown.preview.background",
                "type": "color-role",
                "fallback": "surface.panel",
                "description": "Duplicate"
            }),
        )
        .unwrap_err();
    assert_eq!(duplicate.rule, UiContributionRule::DuplicateId);
}

#[test]
fn component_catalog_rejects_unknown_kinds_duplicate_ids_and_unregistered_actions() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec!["markdown.togglePreview".to_string()];

    let unknown_kind = registry
        .register_component(
            &package,
            &json!({ "kind": "table", "id": "markdown.preview.table" }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(unknown_kind.rule, UiContributionRule::InvalidComponent);
    assert!(unknown_kind.message.contains("reserved for a later"));

    let duplicate_ids = registry
        .register_component(
            &package,
            &json!({
                "kind": "panel",
                "id": "markdown.preview.root",
                "children": [
                    { "kind": "label", "id": "markdown.preview.duplicate", "text": "First" },
                    { "kind": "label", "id": "markdown.preview.duplicate", "text": "Second" }
                ]
            }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(duplicate_ids.rule, UiContributionRule::DuplicateId);

    let unregistered_action = registry
        .register_component(
            &package,
            &json!({
                "kind": "button",
                "id": "markdown.preview.run",
                "label": "Run",
                "action": { "commandId": "markdown.missing" }
            }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(
        unregistered_action.rule,
        UiContributionRule::InvalidActionTarget
    );
}

#[test]
fn input_contributions_accept_component_scoped_pointer_focus_and_actions() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec![
        "markdown.focusPreview".to_string(),
        "markdown.togglePreview".to_string(),
    ];

    let input = registry
        .register_input(
            &package,
            &json!({
                "id": "markdown.preview.input",
                "scope": "component",
                "componentId": "markdown.preview.root",
                "pointer": {
                    "click": "action",
                    "action": "markdown.focusPreview",
                    "drag": "select"
                },
                "focus": { "policy": "restore-editor" },
                "selectionPolicy": "component-local",
                "context": { "modes": ["markdown"] },
                "actionTargets": ["markdown.togglePreview"]
            }),
            &commands,
        )
        .unwrap();

    assert_eq!(input.scope, "component");
    assert_eq!(
        input.pointer_action.as_deref(),
        Some("markdown.focusPreview")
    );
    assert_eq!(input.action_targets.len(), 2);
    let snapshot = registry.snapshot();
    let mut runtime = crate::shell::PackageUiRuntimeState::new();
    runtime
        .apply_update(snapshot.runtime_update(0))
        .expect("input routing should compose into inert runtime state");
    let routes: Vec<_> = runtime.input_routes().collect();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].focus_policy, "restore-editor");
}

#[test]
fn input_contributions_reject_raw_callbacks_key_routing_and_unregistered_actions() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec!["markdown.focusPreview".to_string()];

    let raw_callback = registry
        .register_input(
            &package,
            &json!({
                "id": "markdown.preview.input",
                "scope": "component",
                "componentId": "markdown.preview.root",
                "pointer": { "click": "focus" },
                "clientJavaScript": "window.alert(1)"
            }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(raw_callback.rule, UiContributionRule::ProhibitedAuthority);

    let key_route = registry
        .register_input(
            &package,
            &json!({
                "id": "markdown.preview.keys",
                "scope": "component",
                "componentId": "markdown.preview.root",
                "keys": ["Enter"]
            }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(key_route.rule, UiContributionRule::ProhibitedAuthority);

    let missing_action = registry
        .register_input(
            &package,
            &json!({
                "id": "markdown.preview.missingAction",
                "scope": "component",
                "componentId": "markdown.preview.root",
                "pointer": { "click": "action", "action": "markdown.missing" }
            }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(missing_action.rule, UiContributionRule::InvalidActionTarget);
}

#[test]
fn ui_state_scope_registration_accepts_supported_scopes_and_lifecycles() {
    let mut registry = PackageUiRegistry::new();
    let package = package();

    let scope = registry
        .register_ui_state_scope(
            &package,
            &json!({
                "id": "markdown.preview.visibility",
                "scope": "pane",
                "targetId": "markdown.preview",
                "owner": "shell",
                "lifetime": "session",
                "persistence": "client-local",
                "implementationStatus": "implemented",
                "valueSchema": { "kind": "enum", "values": ["visible", "hidden"] }
            }),
        )
        .unwrap();

    assert_eq!(scope.scope, "pane");
    assert_eq!(scope.persistence, "client-local");
    assert_eq!(scope.implementation_status, "implemented");
    assert_eq!(scope.value_schema_kind, "enum");
    assert_eq!(scope.provenance.api_prefix, "markdown");
    let snapshot = registry.snapshot();
    assert_eq!(snapshot.ui_state_scopes.len(), 1);
    assert_eq!(
        snapshot.ui_state_scopes[0].target_id.as_deref(),
        Some("markdown.preview")
    );
}

#[test]
fn ui_state_scope_registration_rejects_hidden_globals_unsupported_scopes_and_payloads() {
    let mut registry = PackageUiRegistry::new();
    let package = package();

    let hidden = registry
        .register_ui_state_scope(
            &package,
            &json!({
                "id": "markdown._hidden",
                "scope": "package-global",
                "owner": "package",
                "lifetime": "session",
                "persistence": "none",
                "valueSchema": { "kind": "boolean" }
            }),
        )
        .unwrap_err();
    assert_eq!(hidden.rule, UiContributionRule::InvalidId);

    let unsupported = registry
        .register_ui_state_scope(
            &package,
            &json!({
                "id": "markdown.preview.unsupported",
                "scope": "masonry-widget",
                "owner": "shell",
                "lifetime": "session",
                "persistence": "client-local",
                "valueSchema": { "kind": "boolean" }
            }),
        )
        .unwrap_err();
    assert_eq!(unsupported.rule, UiContributionRule::InvalidStateScope);

    let raw_value = registry
        .register_ui_state_scope(
            &package,
            &json!({
                "id": "markdown.preview.raw",
                "scope": "component",
                "targetId": "markdown.preview.root",
                "owner": "shell",
                "lifetime": "session",
                "persistence": "client-local",
                "valueSchema": { "kind": "string", "defaultValue": "hidden" }
            }),
        )
        .unwrap_err();
    assert_eq!(raw_value.rule, UiContributionRule::ProhibitedAuthority);

    let raw_ops = registry
        .register_ui_state_scope(
            &package,
            &json!({
                "id": "markdown.preview.ops",
                "scope": "component",
                "targetId": "markdown.preview.root",
                "owner": "shell",
                "lifetime": "session",
                "persistence": "client-local",
                "valueSchema": { "kind": "string", "code": "Deno.core.ops.op_clay_runtime_ping()" }
            }),
        )
        .unwrap_err();
    assert_eq!(raw_ops.rule, UiContributionRule::ProhibitedAuthority);
}

#[test]
fn layout_override_applies_user_precedence_and_validates_theme_input_and_actions() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec!["markdown.togglePreview".to_string()];

    registry
        .register_panel(
            &package,
            &json!({
                "id": "markdown.preview",
                "slot": "right",
                "component": {
                    "kind": "button",
                    "id": "markdown.preview.button",
                    "label": "Toggle",
                    "action": { "commandId": "markdown.togglePreview" }
                }
            }),
            &commands,
        )
        .unwrap();
    registry
        .register_theme_token(
            &package,
            &json!({
                "token": "markdown.preview.background",
                "type": "color-role",
                "fallback": "surface.panel",
                "description": "Markdown preview background"
            }),
        )
        .unwrap();
    registry
        .register_input(
            &package,
            &json!({
                "id": "markdown.preview.input",
                "scope": "component",
                "componentId": "markdown.preview.button",
                "pointer": { "click": "action", "action": "markdown.togglePreview" }
            }),
            &commands,
        )
        .unwrap();

    let visibility = registry
        .set_layout_override(&json!({
            "targetId": "markdown.preview",
            "property": "visibility",
            "value": "hidden",
            "source": "user-config"
        }))
        .unwrap();
    assert_eq!(visibility.precedence_rank, 1);

    let token_remap = registry
        .set_layout_override(&json!({
            "targetId": "markdown.preview",
            "property": "themeToken",
            "value": { "token": "markdown.preview.background", "fallback": "surface.overlay" },
            "source": "user-config"
        }))
        .unwrap();
    assert_eq!(token_remap.property, "themeToken");

    let input_default = registry
        .set_layout_override(&json!({
            "targetId": "markdown.preview",
            "property": "inputDefault",
            "value": { "inputId": "markdown.preview.input" },
            "source": "active-major-mode"
        }))
        .unwrap();
    assert_eq!(input_default.precedence_rank, 2);

    let action_default = registry
        .set_layout_override(&json!({
            "targetId": "markdown.preview",
            "property": "actionDefault",
            "value": "markdown.togglePreview",
            "source": "package-default"
        }))
        .unwrap();
    assert_eq!(action_default.precedence_rank, 5);
    assert_eq!(registry.snapshot().layout_overrides.len(), 4);
}

#[test]
fn layout_override_rejects_hidden_keys_unknown_tokens_raw_values_and_bad_slots() {
    let mut registry = PackageUiRegistry::new();

    let hidden_target = registry
        .set_layout_override(&json!({
            "targetId": "markdown._hidden",
            "property": "visibility",
            "value": "hidden"
        }))
        .unwrap_err();
    assert_eq!(hidden_target.rule, UiContributionRule::InvalidId);

    let bad_slot = registry
        .set_layout_override(&json!({
            "targetId": "markdown.preview",
            "property": "slot",
            "value": "main"
        }))
        .unwrap_err();
    assert_eq!(bad_slot.rule, UiContributionRule::InvalidSlot);

    let unknown_token = registry
        .set_layout_override(&json!({
            "targetId": "markdown.preview",
            "property": "themeToken",
            "value": { "token": "markdown.preview.background", "fallback": "surface.overlay" }
        }))
        .unwrap_err();
    assert_eq!(unknown_token.rule, UiContributionRule::InvalidThemeToken);

    let raw_value = registry
        .set_layout_override(&json!({
            "targetId": "markdown.preview",
            "property": "fallback",
            "value": { "rawOps": "Deno.core.ops.op_clay_runtime_ping" }
        }))
        .unwrap_err();
    assert_eq!(raw_value.rule, UiContributionRule::ProhibitedAuthority);
}

#[test]
fn theme_token_registry_rejects_raw_css_raw_colors_and_type_mismatches() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let commands = vec!["markdown.togglePreview".to_string()];

    let raw_color_token = registry
        .register_theme_token(
            &package,
            &json!({
                "token": "markdown.preview.raw",
                "type": "color-role",
                "fallback": "surface.panel",
                "description": "Raw color should be rejected",
                "rawColor": "#ff00aa"
            }),
        )
        .unwrap_err();
    assert_eq!(
        raw_color_token.rule,
        UiContributionRule::ProhibitedAuthority
    );

    let type_mismatch = registry
        .register_theme_token(
            &package,
            &json!({
                "token": "markdown.preview.padding",
                "type": "spacing",
                "fallback": "surface.panel",
                "description": "Spacing cannot fall back to a color token"
            }),
        )
        .unwrap_err();
    assert_eq!(type_mismatch.rule, UiContributionRule::InvalidThemeToken);

    registry
        .register_theme_token(
            &package,
            &json!({
                "token": "markdown.preview.background",
                "type": "color-role",
                "fallback": "surface.panel",
                "description": "Markdown preview background"
            }),
        )
        .unwrap();
    let raw_component_color = registry
        .register_component(
            &package,
            &json!({
                "kind": "label",
                "id": "markdown.preview.rawColor",
                "text": "bad",
                "style": { "background": "#ff00aa" }
            }),
            &commands,
        )
        .unwrap_err();
    assert_eq!(
        raw_component_color.rule,
        UiContributionRule::ProhibitedAuthority
    );
}

/// Compile-time size guard for the boxed UI contribution diagnostic.
/// Mirrors the guard in `packages::record`; keeps `UiContributionDiagnostic`
/// under clippy's `result_large_err` 128-byte threshold.
#[test]
fn ui_contribution_diagnostic_size_remains_under_large_err_threshold() {
    const _: () = assert!(std::mem::size_of::<UiContributionDiagnostic>() <= 128);
    assert!(std::mem::size_of::<UiContributionDiagnostic>() <= 128);
}

// -- Phase 20.3: layout intent validation tests --

#[test]
fn layout_intent_valid_request_accepted() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let registered = registry
        .request_layout_intent(
            &package,
            &json!({
                "id": "markdown.splitPreview",
                "targetPane": "active",
                "orientation": "horizontal",
                "ratio": 0.5,
                "position": "second"
            }),
        )
        .unwrap();
    assert_eq!(registered.id, "markdown.splitPreview");
    assert_eq!(registered.target_pane, "active");
    assert_eq!(registered.orientation, "horizontal");
    assert!((registered.ratio - 0.5).abs() < 1e-9);
    assert_eq!(registered.position, "second");
    assert_eq!(registered.source, "markdown");
    assert_eq!(registry.snapshot().layout_intents.len(), 1);
}

#[test]
fn layout_intent_invalid_ratio_rejected() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let err = registry
        .request_layout_intent(
            &package,
            &json!({
                "id": "markdown.badRatio",
                "targetPane": "active",
                "orientation": "horizontal",
                "ratio": 1.5
            }),
        )
        .unwrap_err();
    assert_eq!(err.rule, UiContributionRule::InvalidLayoutIntent);
}

#[test]
fn layout_intent_invalid_orientation_rejected() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let err = registry
        .request_layout_intent(
            &package,
            &json!({
                "id": "markdown.badOrientation",
                "targetPane": "active",
                "orientation": "diagonal",
                "ratio": 0.5
            }),
        )
        .unwrap_err();
    assert_eq!(err.rule, UiContributionRule::InvalidLayoutIntent);
}

#[test]
fn layout_intent_invalid_provenance_rejected() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    // ID not owned by the package's apiPrefix.
    let err = registry
        .request_layout_intent(
            &package,
            &json!({
                "id": "other.splitPreview",
                "targetPane": "active",
                "orientation": "horizontal",
                "ratio": 0.5
            }),
        )
        .unwrap_err();
    assert_eq!(err.rule, UiContributionRule::InvalidId);
}

#[test]
fn layout_intent_duplicate_id_rejected() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let declaration = json!({
        "id": "markdown.splitPreview",
        "targetPane": "active",
        "orientation": "horizontal",
        "ratio": 0.5
    });
    registry
        .request_layout_intent(&package, &declaration)
        .unwrap();
    let err = registry
        .request_layout_intent(&package, &declaration)
        .unwrap_err();
    assert_eq!(err.rule, UiContributionRule::DuplicateId);
}

#[test]
fn layout_intent_default_position_is_second() {
    let mut registry = PackageUiRegistry::new();
    let package = package();
    let registered = registry
        .request_layout_intent(
            &package,
            &json!({
                "id": "markdown.noPosition",
                "targetPane": "active",
                "orientation": "vertical",
                "ratio": 0.4
            }),
        )
        .unwrap();
    assert_eq!(registered.position, "second");
}

fn other_package() -> ClayPackageManifest {
    validate_manifest_value(&json!({
        "name": "@other/landing",
        "version": "0.1.0",
        "clay": {
            "apiPrefix": "other",
            "entry": "./dist/index.js",
            "permissions": ["command-registration"],
            "modes": ["other"]
        }
    }))
    .unwrap()
}

fn entry_declaration(id: &str, command: &str) -> serde_json::Value {
    json!({
        "id": id,
        "activation": "empty-tab",
        "actionTargets": [command],
        "component": {
            "kind": "panel",
            "id": format!("{id}.root"),
            "title": "Entry",
            "children": [{
                "kind": "button",
                "id": format!("{id}.open"),
                "label": "Open File",
                "action": { "commandId": command }
            }]
        }
    })
}

#[test]
fn pane_content_one_winner_two_conflict_and_replacement_withdraws() {
    let mut registry = PackageUiRegistry::new();
    let first = package();
    let second = other_package();
    let commands = vec!["markdown.openFile".to_string(), "other.open".to_string()];

    let registered = registry
        .register_pane_content(
            &first,
            &entry_declaration("markdown.entry", "markdown.openFile"),
            &commands,
        )
        .unwrap();
    assert_eq!(registered.activation, "empty-tab");
    let winner = registry.snapshot().empty_tab().unwrap().unwrap();
    assert_eq!(winner.id, "markdown.entry");
    assert_eq!(winner.package_name, "@clay/markdown");

    registry
        .register_pane_content(
            &second,
            &entry_declaration("other.entry", "other.open"),
            &commands,
        )
        .unwrap();
    let conflict = registry.snapshot().empty_tab().unwrap_err();
    assert_eq!(conflict, vec!["markdown.entry", "other.entry"]);

    registry.pane_contents.remove("markdown.entry");
    let restored = registry.snapshot().empty_tab().unwrap().unwrap();
    assert_eq!(restored.id, "other.entry");
}

/// Plan 118 Part D shipped split: one package claims the landing
/// (`empty-tab`) and another its named surface (`pane`) — no conflict, and
/// a second landing still fails closed.
#[test]
fn launcher_landing_and_agent_pane_coexist_without_competing() {
    let mut registry = PackageUiRegistry::new();
    let launcher = package();
    let agent = other_package();
    let commands = vec!["markdown.openFile".to_string(), "other.open".to_string()];

    registry
        .register_pane_content(
            &launcher,
            &entry_declaration("markdown.start", "markdown.openFile"),
            &commands,
        )
        .unwrap();
    let registered = registry
        .register_pane_content(
            &agent,
            &surface_declaration("other.surface", "other.open"),
            &commands,
        )
        .unwrap();
    assert_eq!(registered.activation, "pane");

    let winner = registry.snapshot().empty_tab().unwrap().unwrap();
    assert_eq!(winner.id, "markdown.start");
    let snapshot = registry.snapshot();
    let surfaces: Vec<&str> = snapshot
        .pane_contents
        .iter()
        .filter(|entry| entry.activation == "pane")
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(surfaces, vec!["other.surface"]);

    // A second landing candidate conflicts, in sorted id order, and
    // withdrawing one restores the single winner.
    registry
        .register_pane_content(
            &agent,
            &entry_declaration("other.start", "other.open"),
            &commands,
        )
        .unwrap();
    assert_eq!(
        registry.snapshot().empty_tab().unwrap_err(),
        vec!["markdown.start", "other.start"]
    );
    registry.pane_contents.remove("other.start");
    assert_eq!(
        registry.snapshot().empty_tab().unwrap().unwrap().id,
        "markdown.start"
    );
}

fn surface_declaration(id: &str, command: &str) -> serde_json::Value {
    json!({
        "id": id,
        "activation": "pane",
        "actionTargets": [command],
        "component": {
            "kind": "panel",
            "id": format!("{id}.root"),
            "title": "Agent",
            "children": [{
                "kind": "button",
                "id": format!("{id}.launch"),
                "label": "Launch",
                "action": { "commandId": command }
            }]
        }
    })
}

#[test]
fn pane_activation_surface_wires_as_named_surface_and_keeps_empty_tab_election() {
    let mut registry = PackageUiRegistry::new();
    let owner = package();
    let commands = vec!["markdown.openFile".to_string()];

    registry
        .register_pane_content(
            &owner,
            &entry_declaration("markdown.entry", "markdown.openFile"),
            &commands,
        )
        .unwrap();
    registry
        .register_pane_content(
            &owner,
            &surface_declaration("markdown.surface", "markdown.openFile"),
            &commands,
        )
        .unwrap();

    // The empty-tab election ignores `pane` surfaces entirely.
    let winner = registry.snapshot().empty_tab().unwrap().unwrap();
    assert_eq!(winner.id, "markdown.entry");

    // The wire snapshot exposes `pane` surfaces with their targets, and
    // version-stamped actions against them validate.
    let wire = registry
        .snapshot()
        .wire_snapshot(7, |_| crate::protocol::PackageUiTrustDomain::Trusted)
        .unwrap();
    assert_eq!(wire.surfaces.len(), 1);
    assert_eq!(wire.surfaces[0].id, "markdown.surface");
    assert_eq!(wire.surfaces[0].package_name, "@clay/markdown");
    assert!(wire.allows_action(7, "markdown.openFile"));
    assert!(!wire.allows_action(6, "markdown.openFile"));
    crate::protocol::PackageUiSnapshot::validate(&wire).unwrap();

    // Unknown activations still fail closed.
    let mut bad = surface_declaration("markdown.bad", "markdown.openFile");
    bad["activation"] = json!("agent-surface");
    let error = registry
        .register_pane_content(&owner, &bad, &commands)
        .unwrap_err();
    assert!(error.message.contains("empty-tab or pane"));
}

/// Every closed-choice field is declared exactly once and still points at the
/// allowed values that shipped before plan 133 task 5 made the checks
/// table-driven; no `VALID_*` slice lost its validator.
#[test]
fn choice_fields_pin_every_allowed_set() {
    const EXPECTED: &[(&str, &[&str])] = &[
        ("slot", VALID_SLOTS),
        ("defaultVisibility", VALID_VISIBILITY),
        ("anchor", VALID_OVERLAY_ANCHORS),
        ("focusPolicy", VALID_FOCUS_POLICIES),
        ("dismissalPolicy", VALID_DISMISSAL_POLICIES),
        ("input.scope", VALID_INPUT_SCOPES),
        ("pointer.click", VALID_POINTER_CLICK_POLICIES),
        ("pointer.drag", VALID_POINTER_DRAG_POLICIES),
        ("focus.policy", VALID_COMPONENT_FOCUS_POLICIES),
        ("selectionPolicy", VALID_SELECTION_POLICIES),
        ("state.scope", VALID_UI_STATE_SCOPES),
        ("state.owner", VALID_UI_STATE_OWNERS),
        ("state.lifetime", VALID_UI_STATE_LIFETIMES),
        ("state.persistence", VALID_UI_STATE_PERSISTENCE),
        ("implementationStatus", VALID_UI_STATE_STATUSES),
        ("valueSchema.kind", VALID_UI_STATE_SCHEMA_KINDS),
        ("layoutOverride.property", VALID_LAYOUT_OVERRIDE_PROPERTIES),
        ("layoutOverride.source", VALID_LAYOUT_OVERRIDE_SOURCES),
        ("layoutOverride.slot", VALID_SLOTS),
        ("layoutOverride.visibility", VALID_VISIBILITY),
        ("layoutOverride.fallback", VALID_FALLBACK_BEHAVIORS),
    ];
    assert_eq!(
        CHOICE_FIELDS.len(),
        EXPECTED.len(),
        "choice field count changed"
    );
    for (key, allowed) in EXPECTED {
        let (row_key, row_allowed, _, _) = choice_field(key);
        assert_eq!(row_key, key, "missing choice field");
        assert_eq!(row_allowed, allowed, "allowed values for {key} changed");
    }
    let mut keys: Vec<&str> = CHOICE_FIELDS.iter().map(|(key, ..)| *key).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), CHOICE_FIELDS.len(), "duplicate choice keys");
    const SLICES: &[&[&str]] = &[
        VALID_SLOTS,
        VALID_VISIBILITY,
        VALID_OVERLAY_ANCHORS,
        VALID_FOCUS_POLICIES,
        VALID_DISMISSAL_POLICIES,
        VALID_INPUT_SCOPES,
        VALID_POINTER_CLICK_POLICIES,
        VALID_POINTER_DRAG_POLICIES,
        VALID_COMPONENT_FOCUS_POLICIES,
        VALID_SELECTION_POLICIES,
        VALID_UI_STATE_SCOPES,
        VALID_UI_STATE_OWNERS,
        VALID_UI_STATE_LIFETIMES,
        VALID_UI_STATE_PERSISTENCE,
        VALID_UI_STATE_STATUSES,
        VALID_UI_STATE_SCHEMA_KINDS,
        VALID_LAYOUT_OVERRIDE_PROPERTIES,
        VALID_LAYOUT_OVERRIDE_SOURCES,
        VALID_FALLBACK_BEHAVIORS,
    ];
    for slice in SLICES {
        assert!(
            CHOICE_FIELDS
                .iter()
                .any(|(_, allowed, _, _)| allowed == slice),
            "no choice row validates {slice:?}"
        );
    }
}

/// The published diagnostics are unchanged: each row's label plus its
/// allowed list reproduces the exact message the hand-written checks raised.
#[test]
fn choice_messages_match_the_shipped_diagnostics() {
    const EXPECTED: &[(&str, &str)] = &[
        (
            "slot",
            "panel slot must be one of left, right, top, or bottom",
        ),
        (
            "defaultVisibility",
            "panel defaultVisibility must be visible, hidden, or collapsed",
        ),
        (
            "anchor",
            "overlay anchor must be one of working-area, active-pane, main, or pointer",
        ),
        (
            "focusPolicy",
            "overlay focusPolicy must be none, restore, or trap",
        ),
        (
            "dismissalPolicy",
            "overlay dismissalPolicy must be manual, escape, outside, or escape-or-outside",
        ),
        (
            "input.scope",
            "input scope must be component, panel, or overlay",
        ),
        (
            "pointer.click",
            "pointer.click must be none, focus, action, or select",
        ),
        ("pointer.drag", "pointer.drag must be none, select, or pan"),
        (
            "focus.policy",
            "focus.policy must be none, restore-editor, focus-component, or trap",
        ),
        (
            "selectionPolicy",
            "selectionPolicy must be preserve-editor, component-local, or disabled",
        ),
        (
            "state.scope",
            "UI state scope must be package-global, user-config, workspace, document, pane, component, or transient-overlay",
        ),
        (
            "state.owner",
            "UI state owner must be package, shell, or server",
        ),
        (
            "state.lifetime",
            "UI state lifetime must be session, workspace, document, or transient",
        ),
        (
            "state.persistence",
            "UI state persistence must be none, client-local, server-canonical, or deferred",
        ),
        (
            "implementationStatus",
            "implementationStatus must be implemented or deferred",
        ),
        (
            "valueSchema.kind",
            "valueSchema.kind must be boolean, number, string, enum, or object",
        ),
        (
            "layoutOverride.property",
            "layout override property must be slot, visibility, splitRatio, themeToken, inputDefault, actionDefault, or fallback",
        ),
        (
            "layoutOverride.source",
            "layout override source must be user-config, active-major-mode, compatible-minor-mode, global-package, or package-default",
        ),
        (
            "layoutOverride.slot",
            "slot override value must be left, right, top, or bottom",
        ),
        (
            "layoutOverride.visibility",
            "visibility override value must be visible, hidden, or collapsed",
        ),
        (
            "layoutOverride.fallback",
            "fallback override value must be package-default, hide, or ignore",
        ),
    ];
    assert_eq!(
        CHOICE_FIELDS.len(),
        EXPECTED.len(),
        "choice field count changed"
    );
    for (key, message) in EXPECTED {
        let (_, allowed, _, label) = choice_field(key);
        assert_eq!(
            choice_message(allowed, label),
            *message,
            "diagnostic for {key} changed"
        );
    }
}
