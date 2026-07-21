use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::{
    packages::{
        manifest::ClayPackageManifest,
        modes::{DocumentClassificationInput, ModeDeclaration, ModeDiagnostic},
    },
    protocol::{
        AutocompleteTrigger, CommentContinuationRule, EditorBehaviorRules, ElectricCharacterRule,
        ElectricEffect, EnterRule, PairRule, PairRuleContext, RoutingPolicy, TabMode, TabRule,
        TextEditCapability,
    },
};

use super::ClayOpState;

#[op2]
#[string]
pub(super) fn op_clay_modes_register_pattern(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context; the
    // declaration cannot override package name/version/prefix.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .require_current_package_capability(
            crate::packages::permissions::PackagePermission::ModeRegistration,
        )?;
    let declaration_value = parse_json(&declaration_json, "clay.modes.invalid_declaration")?;
    let declaration = parse_declaration(&declaration_value, &package.manifest)?;
    let response_identity = json!({
        "registered": true,
        "packagePrefix": declaration.api_prefix,
        "modeId": declaration.mode_id,
    });
    state
        .borrow::<Arc<ClayOpState>>()
        .register_mode(&package.manifest, declaration)
        .map_err(mode_error("clay.modes.registration_failed"))?;
    serde_json::to_string(&response_identity)
        .map_err(serialize_error("clay.modes.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_modes_classify_document(
    state: &mut OpState,
    #[string] input_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&input_json, "clay.modes.invalid_classification")?;
    let input = DocumentClassificationInput {
        document_id: value
            .get("documentId")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                JsErrorBox::generic(
                    "clay.modes.invalid_classification: documentId must be a number",
                )
            })?,
        path: value
            .get("path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        mime_type: value
            .get("mimeType")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        // Shebang and bounded leading-content probes are supplied by the open
        // path only. Oversize leading content is rejected by
        // `ModeRegistry::classify` (treated as absent), so probes can never
        // read unbounded content regardless of caller.
        shebang: value
            .get("shebang")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        leading_content: value
            .get("leadingContent")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    };
    let classification = state
        .borrow::<Arc<ClayOpState>>()
        .classify_document(&input)
        .map_err(mode_error("clay.modes.classification_failed"))?;
    serde_json::to_string(&json!({
        "documentId": classification.document_id,
        "packageName": classification.package_name,
        "packageVersion": classification.package_version,
        "apiPrefix": classification.api_prefix,
        "modeId": classification.mode_id,
        "matchedBy": format!("{:?}", classification.matched_by),
    }))
    .map_err(serialize_error("clay.modes.classification_failed"))
}

/// Activate a major mode for a document, optionally installing package-supplied
/// editor rules into the live behavior manifest.
///
/// The `input` object accepted from JavaScript may include an optional
/// `editorRules` field whose value is a JSON object with generic rule fields:
///
/// ```json
/// {
///   "documentId": 1,
///   "modeId": "markdown",
///   "editorRules": {
///     "enter": {
///       "kind": "continueLineMarkers",
///       "markers": ["-", "*", "+", "ordered-dot"],
///       "exitOnEmptyItem": true
///     },
///     "pairs": [
///       { "open": "(",  "close": ")" },
///       { "open": "[",  "close": "]" },
///       { "open": "**", "close": "**" },
///       { "open": "__", "close": "__" },
///       { "open": "`",  "close": "`"  }
///     ],
///     "comments": [],
///     "tabSpaces": 4
///   }
/// }
/// ```
///
/// The Rust op validates the shape of each rule, converts it into the generic
/// protocol types (`EnterRule`, `PairRule`, …), and publishes an updated
/// `BehaviorManifest`.  **No Markdown-specific logic lives in this op** — the
/// package JS is responsible for choosing which rule kinds and parameters to
/// supply.
#[op2]
#[string]
pub(super) fn op_clay_modes_activate_major_mode(
    state: &mut OpState,
    #[string] input_json: String,
) -> Result<String, JsErrorBox> {
    // The mode owner is resolved host-side from the classification (registered
    // declaration provenance + enabled set), never from a caller manifest.
    let value = parse_json(&input_json, "clay.modes.invalid_activation")?;
    let input = DocumentClassificationInput {
        document_id: value
            .get("documentId")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                JsErrorBox::generic("clay.modes.invalid_activation: documentId must be a number")
            })?,
        path: value
            .get("path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        mime_type: value
            .get("mimeType")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        shebang: value
            .get("shebang")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        leading_content: value
            .get("leadingContent")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    };

    // Parse optional package-supplied editor rules.  These are validated into
    // generic protocol types; no mode-specific Rust logic is involved.
    let editor_rules_override: Option<EditorBehaviorRules> = value
        .get("editorRules")
        .map(parse_editor_rules)
        .transpose()
        .map_err(|e: String| JsErrorBox::generic(e))?;

    let op_state = state.borrow::<Arc<ClayOpState>>();
    let activation = op_state
        .activate_major_mode(&input)
        .map_err(mode_error("clay.modes.activation_failed"))?;

    // If the package supplied editor rules, publish an updated behavior manifest
    // with those rules applied.  This is mode-agnostic: any package for any mode
    // can shape the manifest's enter/pair/comment/tab rules by passing editorRules.
    if let Some(rules) = editor_rules_override {
        let commands_for_activation: Vec<_> = value
            .get("commands")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|cmd| {
                        let id = cmd.get("id").and_then(Value::as_str)?;
                        let display_name =
                            cmd.get("displayName").and_then(Value::as_str).unwrap_or(id);
                        let routing = cmd
                            .get("routingPolicy")
                            .and_then(Value::as_str)
                            .unwrap_or("server-first");
                        let policy = parse_routing_policy_str(routing).ok()?;
                        Some(crate::protocol::CommandDeclaration {
                            command_id: id.to_string(),
                            display_name: display_name.to_string(),
                            routing_policy: policy,
                            authority: crate::protocol::CommandAuthority::ServerIntent,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let keymaps_for_activation: Vec<_> = value
            .get("keymaps")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(|km| parse_keymap(km).ok()).collect())
            .unwrap_or_default();

        op_state
            .publish_mode_behavior_manifest(
                &activation,
                rules,
                commands_for_activation,
                keymaps_for_activation,
            )
            .map_err(|e| {
                JsErrorBox::generic(format!(
                    "clay.modes.activation_failed: manifest validation: {e:?}"
                ))
            })?;
    }

    serde_json::to_string(&json!({
        "documentId": activation.document_id,
        "packageName": activation.package_name,
        "packageVersion": activation.package_version,
        "apiPrefix": activation.api_prefix,
        "modeId": activation.mode_id,
        "behaviorVersion": activation.behavior_version,
    }))
    .map_err(serialize_error("clay.modes.activation_failed"))
}

// ── Editor-rules JSON deserializer ────────────────────────────────────────────
//
// Converts the generic `editorRules` JSON object supplied by package JavaScript
// into `EditorBehaviorRules`.  All rule kinds are language-agnostic;
// no mode-specific names appear here.

fn parse_editor_rules(value: &Value) -> Result<EditorBehaviorRules, String> {
    let enter = match value.get("enter") {
        None => EnterRule::PreserveLeadingWhitespace,
        Some(enter_value) => parse_enter_rule(enter_value)?,
    };

    let pairs = match value.get("pairs") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(parse_pair_rule)
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(
                "clay.modes.invalid_activation: editorRules.pairs must be an array".to_string(),
            );
        }
    };

    let comments = match value.get("comments") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(parse_comment_rule)
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(
                "clay.modes.invalid_activation: editorRules.comments must be an array".to_string(),
            );
        }
    };

    let tab_spaces: u8 = value
        .get("tabSpaces")
        .and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
        .unwrap_or(4);

    let autocomplete_triggers: Vec<AutocompleteTrigger> = match value.get("autocompleteTriggers") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|t| {
                let trigger = t.get("trigger").and_then(Value::as_str)?.to_string();
                Some(AutocompleteTrigger {
                    trigger,
                    routing_policy: RoutingPolicy::UiReactivePriority,
                })
            })
            .collect(),
        _ => Vec::new(),
    };

    // Electric characters are declarative manifest data: a package names the
    // trigger character and a known effect. Only Rust-known effects are
    // accepted; unknown effects are dropped so packages can never introduce a
    // client-executed transform kind the engine does not recognise.
    let electric_characters: Vec<ElectricCharacterRule> = match value.get("electricCharacters") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|entry| {
                let trigger = entry.get("trigger").and_then(Value::as_str)?.to_string();
                let effect = entry.get("effect").and_then(Value::as_str)?;
                let effect = match effect {
                    "outdent-one-level" => ElectricEffect::OutdentOneLevel,
                    _ => return None,
                };
                Some(ElectricCharacterRule { trigger, effect })
            })
            .collect(),
        _ => Vec::new(),
    };

    Ok(EditorBehaviorRules {
        text_edits: vec![
            TextEditCapability::Insert,
            TextEditCapability::Delete,
            TextEditCapability::Replace,
        ],
        enter,
        tab: TabRule {
            mode: TabMode::InsertSpaces,
            spaces_per_tab: tab_spaces,
        },
        pairs,
        comments,
        electric_characters,
        autocomplete_triggers,
    })
}

fn parse_enter_rule(value: &Value) -> Result<EnterRule, String> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("preserveLeadingWhitespace");
    match kind {
        "preserveLeadingWhitespace" => Ok(EnterRule::PreserveLeadingWhitespace),
        "insertNewlineOnly" => Ok(EnterRule::InsertNewlineOnly),
        "continueLineMarkers" => {
            let markers = string_array_field(value, "markers")?;
            if markers.is_empty() {
                return Err("clay.modes.invalid_activation: continueLineMarkers requires at least one marker".to_string());
            }
            let exit_on_empty_item = value
                .get("exitOnEmptyItem")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            Ok(EnterRule::ContinueLineMarkers {
                markers,
                exit_on_empty_item,
            })
        }
        "preserveFenceBodyIndent" => {
            let fence_markers = string_array_field(value, "fenceMarkers")?;
            if fence_markers.is_empty() {
                return Err("clay.modes.invalid_activation: preserveFenceBodyIndent requires at least one fenceMarker".to_string());
            }
            Ok(EnterRule::PreserveFenceBodyIndent { fence_markers })
        }
        other => Err(format!(
            "clay.modes.invalid_activation: unknown enter rule kind '{other}'; \
             valid kinds: preserveLeadingWhitespace, insertNewlineOnly, \
             continueLineMarkers, preserveFenceBodyIndent"
        )),
    }
}

fn parse_pair_rule(value: &Value) -> Result<PairRule, String> {
    let open = value
        .get("open")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "clay.modes.invalid_activation: pair rule requires non-empty 'open'".to_string()
        })?;
    let close = value
        .get("close")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "clay.modes.invalid_activation: pair rule requires non-empty 'close'".to_string()
        })?;
    // Reject executable-sounding field names.
    for forbidden in &["callback", "code", "javascript", "hook"] {
        if value.get(forbidden).is_some() {
            return Err(format!(
                "clay.modes.invalid_activation: pair rule must not include executable field '{forbidden}'"
            ));
        }
    }
    Ok(PairRule {
        open: open.to_string(),
        close: close.to_string(),
        when: PairRuleContext::CaretOrSelection,
    })
}

fn parse_comment_rule(value: &Value) -> Result<CommentContinuationRule, String> {
    let line_prefix = value
        .get("linePrefix")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "clay.modes.invalid_activation: comment rule requires non-empty 'linePrefix'"
                .to_string()
        })?;
    let continue_prefix = value
        .get("continuePrefix")
        .and_then(Value::as_str)
        .unwrap_or(line_prefix);
    Ok(CommentContinuationRule {
        line_prefix: line_prefix.to_string(),
        continue_prefix: continue_prefix.to_string(),
    })
}

fn string_array_field(value: &Value, key: &str) -> Result<Vec<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|v| {
                v.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                    format!("clay.modes.invalid_activation: '{key}' entries must be strings")
                })
            })
            .collect(),
        _ => Err(format!(
            "clay.modes.invalid_activation: '{key}' must be an array"
        )),
    }
}

fn parse_keymap(value: &Value) -> Result<crate::protocol::KeyBindingRule, String> {
    use crate::protocol::{KeyBindingContext, KeyBindingRule, KeyCode, KeyModifiers, KeyStroke};
    let command_id = value
        .get("commandId")
        .and_then(Value::as_str)
        .ok_or_else(|| "clay.modes.invalid_activation: keymap requires 'commandId'".to_string())?;
    let key_str = value
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| "clay.modes.invalid_activation: keymap requires 'key'".to_string())?;
    let routing = value
        .get("routingPolicy")
        .and_then(Value::as_str)
        .unwrap_or("server-first");
    let routing_policy = parse_routing_policy_str(routing)
        .map_err(|e| format!("clay.modes.invalid_activation: keymap {e}"))?;
    Ok(KeyBindingRule {
        command_id: command_id.to_string(),
        sequence: vec![KeyStroke {
            key: KeyCode::Character(key_str.to_string()),
            modifiers: KeyModifiers::NONE,
        }],
        context: KeyBindingContext::EditorTextFocus,
        routing_policy,
    })
}

fn parse_routing_policy_str(value: &str) -> Result<RoutingPolicy, String> {
    match value {
        "server-first" | "ServerFirst" => Ok(RoutingPolicy::ServerFirst),
        "background" | "Background" => Ok(RoutingPolicy::Background),
        "ui-reactive-priority" | "UiReactivePriority" => Ok(RoutingPolicy::UiReactivePriority),
        other => Err(format!("unsupported routingPolicy '{other}'")),
    }
}

fn parse_declaration(
    value: &Value,
    package: &ClayPackageManifest,
) -> Result<ModeDeclaration, JsErrorBox> {
    Ok(ModeDeclaration {
        package_name: package.name.clone(),
        package_version: package.version.clone(),
        api_prefix: package.clay.api_prefix.clone(),
        mode_id: required_string(value, "modeId", "clay.modes.invalid_declaration")?,
        display_name: string_or(value, "displayName", "Mode"),
        document_font_role: match value
            .get("defaultFontRole")
            .and_then(Value::as_str)
            .unwrap_or("proportional")
        {
            "monospace" => crate::protocol::DocumentFontRole::Monospace,
            "proportional" => crate::protocol::DocumentFontRole::Proportional,
            _ => {
                return Err(JsErrorBox::generic(
                    "clay.modes.invalid_declaration: defaultFontRole must be `monospace` or `proportional`",
                ));
            }
        },
        extensions: string_array(
            value.get("extensions"),
            "extensions",
            "clay.modes.invalid_declaration",
        )?,
        mime_types: string_array(
            value.get("mimeTypes"),
            "mimeTypes",
            "clay.modes.invalid_declaration",
        )?,
        file_names: string_array(
            value.get("fileNames"),
            "fileNames",
            "clay.modes.invalid_declaration",
        )?,
        file_name_patterns: string_array(
            value.get("fileNamePatterns"),
            "fileNamePatterns",
            "clay.modes.invalid_declaration",
        )?,
        shebang_patterns: string_array(
            value.get("shebangPatterns"),
            "shebangPatterns",
            "clay.modes.invalid_declaration",
        )?,
        content_probes: string_array(
            value.get("contentProbes"),
            "contentProbes",
            "clay.modes.invalid_declaration",
        )?,
    })
}

fn parse_json(json_text: &str, code: &str) -> Result<Value, JsErrorBox> {
    serde_json::from_str(json_text)
        .map_err(|error| JsErrorBox::generic(format!("{code}: input must be valid JSON ({error})")))
}

fn required_string(value: &Value, key: &str, code: &str) -> Result<String, JsErrorBox> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| JsErrorBox::generic(format!("{code}: {key} must be a string")))
}

fn string_or(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn string_array(value: Option<&Value>, key: &str, code: &str) -> Result<Vec<String>, JsErrorBox> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                    JsErrorBox::generic(format!("{code}: {key} entries must be strings"))
                })
            })
            .collect(),
        _ => Err(JsErrorBox::generic(format!(
            "{code}: {key} must be an array"
        ))),
    }
}

fn mode_error(code: &'static str) -> impl Fn(ModeDiagnostic) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}
