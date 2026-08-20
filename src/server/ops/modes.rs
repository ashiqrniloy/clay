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
        AutocompleteTrigger, BlinkStyle, CaretShape, CaretStyle, CommentContinuationRule,
        EditorBehaviorRules, ElectricCharacterRule, ElectricEffect, EnterRule, LineMovementStyle,
        MovementRules, PairRule, PairRuleContext, ParagraphStyle, RoutingPolicy, TabMode, TabRule,
        TextEditCapability, WordSeparatorPolicy,
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
    let declaration_value = parse_json(&declaration_json, "modes.invalid_declaration")?;
    let declaration = parse_declaration(&declaration_value, &package.manifest)?;
    let response_identity = json!({
        "registered": true,
        "packagePrefix": declaration.api_prefix,
        "modeId": declaration.mode_id,
    });
    state
        .borrow::<Arc<ClayOpState>>()
        .register_mode(&package.manifest, declaration)
        .map_err(mode_error("modes.registration_failed"))?;
    serde_json::to_string(&response_identity).map_err(serialize_error("modes.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_modes_classify_document(
    state: &mut OpState,
    #[string] input_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&input_json, "modes.invalid_classification")?;
    let input = DocumentClassificationInput {
        document_id: value
            .get("documentId")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                JsErrorBox::generic("modes.invalid_classification: documentId must be a number")
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
        .map_err(mode_error("modes.classification_failed"))?;
    serde_json::to_string(&json!({
        "documentId": classification.document_id,
        "packageName": classification.package_name,
        "packageVersion": classification.package_version,
        "apiPrefix": classification.api_prefix,
        "modeId": classification.mode_id,
        "matchedBy": format!("{:?}", classification.matched_by),
    }))
    .map_err(serialize_error("modes.classification_failed"))
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
    let value = parse_json(&input_json, "modes.invalid_activation")?;
    let input = DocumentClassificationInput {
        document_id: value
            .get("documentId")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                JsErrorBox::generic("modes.invalid_activation: documentId must be a number")
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
        .map_err(mode_error("modes.activation_failed"))?;

    // Caller-supplied editorRules win (init.js / registerModePattern cache).
    // Otherwise use the host-enabled package record so loadPackage apply-record
    // does not need a JS activationRegistry entry.
    let record_behavior = op_state.record_behavior_for_activation(&activation);
    let rules = editor_rules_override.or(record_behavior.as_ref().map(|b| b.0.clone()));
    if let Some(rules) = rules {
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
                        let policy = RoutingPolicy::parse(routing).ok()?;
                        Some(crate::protocol::CommandDeclaration {
                            command_id: id.to_string(),
                            display_name: display_name.to_string(),
                            routing_policy: policy,
                            authority: crate::protocol::CommandAuthority::ServerIntent,
                        })
                    })
                    .collect()
            })
            .or_else(|| record_behavior.as_ref().map(|b| b.1.clone()))
            .unwrap_or_default();

        let keymaps_for_activation: Vec<_> = value
            .get("keymaps")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().map(parse_keymap).collect::<Result<Vec<_>, _>>())
            .transpose()
            .map_err(JsErrorBox::generic)?
            .or_else(|| record_behavior.as_ref().map(|b| b.2.clone()))
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
                    "modes.activation_failed: manifest validation: {e:?}"
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
    .map_err(serialize_error("modes.activation_failed"))
}

// ── Editor-rules JSON deserializer ────────────────────────────────────────────
//
// Converts the generic `editorRules` JSON object supplied by package JavaScript
// into `EditorBehaviorRules`.  All rule kinds are language-agnostic;
// no mode-specific names appear here.

pub(crate) fn parse_editor_rules(value: &Value) -> Result<EditorBehaviorRules, String> {
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
            return Err("modes.invalid_activation: editorRules.pairs must be an array".to_string());
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
                "modes.invalid_activation: editorRules.comments must be an array".to_string(),
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
        heading_prefixes: string_array_field(value, "headingPrefixes")?,
        electric_characters,
        autocomplete_triggers,
        movement: parse_movement_rules(value.get("movement")),
        caret_style: parse_caret_style(value.get("caretStyle")),
        chrome: parse_chrome(value.get("chrome")),
        layout: parse_layout(value.get("layout")),
    })
}

fn parse_layout(value: Option<&Value>) -> Option<crate::protocol::EditorLayoutRules> {
    let Some(Value::Object(obj)) = value else {
        return None;
    };
    let policy = obj.get("wrapPolicy").and_then(Value::as_str)?;
    let wrap = match policy {
        "none" => crate::protocol::WrapPolicy::None,
        "viewport" => crate::protocol::WrapPolicy::Viewport,
        "column" => {
            let cap = obj
                .get("columnCap")
                .and_then(Value::as_u64)
                .and_then(|n| u16::try_from(n).ok())
                .unwrap_or(crate::protocol::WrapPolicy::DEFAULT_COLUMN);
            crate::protocol::WrapPolicy::Column(crate::protocol::WrapPolicy::clamp_column(cap))
        }
        _ => return None,
    };
    Some(crate::protocol::EditorLayoutRules { wrap })
}

fn parse_chrome(value: Option<&Value>) -> Option<crate::protocol::EditorChrome> {
    let Some(Value::Object(obj)) = value else {
        return None;
    };
    Some(crate::protocol::EditorChrome {
        gutter: obj.get("gutter").and_then(Value::as_bool).unwrap_or(false),
        active_line: obj
            .get("activeLine")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        indent_guides: obj
            .get("indentGuides")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        bracket_match: obj
            .get("bracketMatch")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        inlay_hints: obj
            .get("inlayHints")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Parse the optional `editorRules.movement` override; absent or partial values
/// fall back to [`MovementRules::default`] so existing modes gain the new
/// primitives with no behaviour change.
fn parse_movement_rules(value: Option<&Value>) -> MovementRules {
    let Some(value) = value else {
        return MovementRules::default();
    };
    let word_separators = match value.get("wordSeparators") {
        Some(Value::String(s)) if s == "prose" => WordSeparatorPolicy::Prose,
        Some(Value::String(s)) if s == "code" => WordSeparatorPolicy::Code,
        Some(Value::Object(_)) => {
            let separators = value
                .get("wordSeparators")
                .and_then(|v| v.get("custom"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().and_then(|s| s.chars().next()))
                .collect::<Vec<char>>();
            WordSeparatorPolicy::Custom(separators)
        }
        _ => WordSeparatorPolicy::Code,
    };
    MovementRules {
        word_separators,
        treat_underscore_as_word: value
            .get("treatUnderscoreAsWord")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        camel_case_sub_word: value
            .get("camelCaseSubWord")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        paragraph_style: match value.get("paragraphStyle").and_then(Value::as_str) {
            Some("blankLine") => ParagraphStyle::BlankLine,
            _ => ParagraphStyle::BlankLineOrWhitespace,
        },
        stop_at_eol_word_end: value
            .get("stopAtEolWordEnd")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        line_movement: match value.get("lineMovement").and_then(Value::as_str) {
            Some("screenLine") => LineMovementStyle::ScreenLine,
            _ => LineMovementStyle::Character,
        },
        sticky_column: value
            .get("stickyColumn")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    }
}

/// Parse the optional `editorRules.caretStyle` override. Returns `None` when
/// absent so the editor `StyleRegistry` default applies; partial objects fall
/// back field-by-field to [`CaretStyle::default`].
pub(super) fn parse_caret_style(value: Option<&Value>) -> Option<CaretStyle> {
    let value = value?;
    Some(merge_caret_style(CaretStyle::default(), value))
}

/// Merge the caret-style fields present in `value` over `style`; absent
/// fields keep the base. Shared by manifest parsing (base = Clay default)
/// and the `clientSetCursorStyle` runtime override (base = active
/// manifest/theme style), so both honour "absent fields fall back".
pub(super) fn merge_caret_style(mut style: CaretStyle, value: &Value) -> CaretStyle {
    style.shape = match value.get("shape").and_then(Value::as_str) {
        Some("bar") => CaretShape::Bar,
        Some("line") => CaretShape::Line,
        Some("block") => CaretShape::Block,
        Some("underline") => CaretShape::Underline,
        _ => style.shape,
    };
    style.blink = match value.get("blink").and_then(Value::as_str) {
        Some("solid") => BlinkStyle::Solid,
        Some("blink") => BlinkStyle::Blink {
            on_ms: 500,
            off_ms: 500,
            wait_ms: 500,
        },
        Some("phase") => BlinkStyle::Phase { period_ms: 1000 },
        Some("smooth") => BlinkStyle::Smooth { period_ms: 1000 },
        _ => style.blink,
    };
    if let Some(width_px) = value.get("widthPx").and_then(Value::as_f64) {
        style.width_px = width_px as f32;
    }
    if let Some(height_pct) = value.get("heightPct").and_then(Value::as_f64) {
        style.height_pct = height_pct as f32;
    }
    if let Some(hollow) = value.get("hollow").and_then(Value::as_bool) {
        style.hollow = hollow;
    }
    if let Some(ms) = value.get("smoothAnimationMs").and_then(Value::as_u64) {
        style.smooth_animation_ms = ms as u32;
    }
    if let Some(stop) = value.get("stopBlinkOnTyping").and_then(Value::as_bool) {
        style.stop_blink_on_typing = stop;
    }
    style
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
                return Err(
                    "modes.invalid_activation: continueLineMarkers requires at least one marker"
                        .to_string(),
                );
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
                return Err("modes.invalid_activation: preserveFenceBodyIndent requires at least one fenceMarker".to_string());
            }
            Ok(EnterRule::PreserveFenceBodyIndent { fence_markers })
        }
        other => Err(format!(
            "modes.invalid_activation: unknown enter rule kind '{other}'; \
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
            "modes.invalid_activation: pair rule requires non-empty 'open'".to_string()
        })?;
    let close = value
        .get("close")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "modes.invalid_activation: pair rule requires non-empty 'close'".to_string()
        })?;
    // Reject executable-sounding field names.
    for forbidden in &["callback", "code", "javascript", "hook"] {
        if value.get(forbidden).is_some() {
            return Err(format!(
                "modes.invalid_activation: pair rule must not include executable field '{forbidden}'"
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
            "modes.invalid_activation: comment rule requires non-empty 'linePrefix'".to_string()
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
                    format!("modes.invalid_activation: '{key}' entries must be strings")
                })
            })
            .collect(),
        _ => Err(format!(
            "modes.invalid_activation: '{key}' must be an array"
        )),
    }
}

pub(crate) fn parse_keymap(value: &Value) -> Result<crate::protocol::KeyBindingRule, String> {
    use crate::protocol::{KeyBindingContext, KeyBindingRule};
    let command_id = value
        .get("commandId")
        .and_then(Value::as_str)
        .ok_or_else(|| "modes.invalid_activation: keymap requires 'commandId'".to_string())?;
    let key_str = value
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| "modes.invalid_activation: keymap requires 'key'".to_string())?;
    let routing = value
        .get("routingPolicy")
        .and_then(Value::as_str)
        .unwrap_or("server-first");
    let routing_policy = RoutingPolicy::parse(routing)
        .map_err(|e| format!("modes.invalid_activation: keymap {e}"))?;
    let sequence = super::keybindings::parse_key_sequence(key_str)
        .map_err(|e| format!("modes.invalid_activation: keymap {e}"))?;
    Ok(KeyBindingRule {
        command_id: command_id.to_string(),
        sequence,
        context: KeyBindingContext::EditorTextFocus,
        routing_policy,
    })
}

fn parse_declaration(
    value: &Value,
    package: &ClayPackageManifest,
) -> Result<ModeDeclaration, JsErrorBox> {
    Ok(ModeDeclaration {
        package_name: package.name.clone(),
        package_version: package.version.clone(),
        api_prefix: package.clay.api_prefix.clone(),
        mode_id: required_string(value, "modeId", "modes.invalid_declaration")?,
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
                    "modes.invalid_declaration: defaultFontRole must be `monospace` or `proportional`",
                ));
            }
        },
        extensions: string_array(
            value.get("extensions"),
            "extensions",
            "modes.invalid_declaration",
        )?,
        mime_types: string_array(
            value.get("mimeTypes"),
            "mimeTypes",
            "modes.invalid_declaration",
        )?,
        file_names: string_array(
            value.get("fileNames"),
            "fileNames",
            "modes.invalid_declaration",
        )?,
        file_name_patterns: string_array(
            value.get("fileNamePatterns"),
            "fileNamePatterns",
            "modes.invalid_declaration",
        )?,
        shebang_patterns: string_array(
            value.get("shebangPatterns"),
            "shebangPatterns",
            "modes.invalid_declaration",
        )?,
        content_probes: string_array(
            value.get("contentProbes"),
            "contentProbes",
            "modes.invalid_declaration",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{BlinkStyle, CaretShape, CaretStyle};

    #[test]
    fn parse_caret_style_absent_is_none() {
        assert_eq!(parse_caret_style(None), None);
    }

    #[test]
    fn parse_caret_style_maps_shape_and_blink() {
        let value: Value = serde_json::from_str(
            r#"{"shape":"block","blink":"blink","widthPx":2.0,"hollow":true}"#,
        )
        .unwrap();
        let style = parse_caret_style(Some(&value)).expect("present object parses");
        assert_eq!(style.shape, CaretShape::Block);
        assert!(matches!(style.blink, BlinkStyle::Blink { .. }));
        assert_eq!(style.width_px, 2.0);
        assert!(style.hollow);
    }

    #[test]
    fn parse_caret_style_partial_falls_back_to_defaults() {
        let value: Value = serde_json::from_str(r#"{"shape":"underline"}"#).unwrap();
        let style = parse_caret_style(Some(&value)).expect("partial parses");
        assert_eq!(style.shape, CaretShape::Underline);
        assert_eq!(style.blink, BlinkStyle::Solid);
        assert_eq!(style.width_px, CaretStyle::default().width_px);
        assert!(!style.hollow);
    }

    fn stroke(json: &str) -> crate::protocol::KeyBindingRule {
        parse_keymap(&serde_json::from_str(json).unwrap()).unwrap()
    }

    #[test]
    fn parse_keymap_ctrl_shift_m_has_modifiers() {
        let rule = stroke(
            r#"{"commandId":"markdown.togglePreview","key":"Ctrl+Shift+M","routingPolicy":"server-first"}"#,
        );
        assert_eq!(rule.sequence.len(), 1);
        assert_eq!(
            rule.sequence[0].key,
            crate::protocol::KeyCode::Character("m".into())
        );
        assert!(rule.sequence[0].modifiers.control);
        assert!(rule.sequence[0].modifiers.shift);
        assert!(!rule.sequence[0].modifiers.alt);
    }

    #[test]
    fn parse_keymap_multi_stroke_sequence() {
        let rule = stroke(r#"{"commandId":"x","key":"Ctrl+X Ctrl+F"}"#);
        assert_eq!(rule.sequence.len(), 2);
        assert_eq!(
            rule.sequence[0],
            super::super::keybindings::parse_key_chord("Ctrl+X").unwrap()
        );
        assert_eq!(
            rule.sequence[1],
            super::super::keybindings::parse_key_chord("Ctrl+F").unwrap()
        );
    }

    #[test]
    fn parse_keymap_rejects_empty_and_malformed() {
        for json in [
            r#"{"commandId":"x","key":""}"#,
            r#"{"commandId":"x","key":"Ctrl+"}"#,
            r#"{"commandId":"x","key":"Ctrl+Shift+Moo"}"#,
            r#"{"commandId":"x","key":"Ctrl+Shift+M","routingPolicy":"not-a-policy"}"#,
        ] {
            assert!(
                parse_keymap(&serde_json::from_str(json).unwrap()).is_err(),
                "{json}"
            );
        }
    }

    #[test]
    fn markdown_default_keymaps_match_key_events() {
        let preview = stroke(
            r#"{"commandId":"markdown.togglePreview","key":"Ctrl+Shift+M","routingPolicy":"server-first"}"#,
        );
        let heading = stroke(
            r#"{"commandId":"markdown.insertHeading","key":"Ctrl+Alt+1","routingPolicy":"server-first"}"#,
        );
        let list = stroke(
            r#"{"commandId":"markdown.toggleList","key":"Ctrl+Shift+8","routingPolicy":"server-first"}"#,
        );
        assert_eq!(
            preview.sequence[0].key,
            crate::protocol::KeyCode::Character("m".into())
        );
        assert!(preview.sequence[0].modifiers.control && preview.sequence[0].modifiers.shift);
        assert_eq!(
            heading.sequence[0].key,
            crate::protocol::KeyCode::Character("1".into())
        );
        assert!(heading.sequence[0].modifiers.control && heading.sequence[0].modifiers.alt);
        assert_eq!(
            list.sequence[0].key,
            crate::protocol::KeyCode::Character("8".into())
        );
        assert!(list.sequence[0].modifiers.control && list.sequence[0].modifiers.shift);
    }

    #[test]
    fn first_party_package_keymaps_match_parsed_sequences_and_key_events() {
        let package: Value =
            serde_json::from_str(include_str!("../../../packages/markdown/package.json"))
                .expect("Markdown package manifest must be valid JSON");
        let keymaps = package["clay"]["contributions"]["keyRouting"]
            .as_array()
            .expect("Markdown package must declare keyRouting");

        for keymap in keymaps {
            let command_id = keymap["commandId"]
                .as_str()
                .expect("keyRouting commandId must be a string");
            let key = keymap["key"]
                .as_str()
                .expect("keyRouting key must be a string");
            let parsed = parse_keymap(keymap).expect("package keymap must parse");
            let expected = super::super::keybindings::parse_key_sequence(key)
                .expect("shared key sequence parser must accept package key");
            assert_eq!(
                parsed.sequence, expected,
                "sequence parity for {command_id}"
            );

            let mut manifest = crate::protocol::BehaviorManifest::minimal_text_editing(1);
            manifest
                .commands
                .push(crate::protocol::CommandDeclaration::server_intent(
                    command_id, command_id,
                ));
            manifest.keymaps.push(parsed);
            let state = crate::client::behavior::ClientBehaviorState::new(manifest)
                .expect("package keymap must produce a valid manifest");
            let mut pending = Vec::new();
            for (index, stroke) in expected.iter().enumerate() {
                let event = match &stroke.key {
                    crate::protocol::KeyCode::Character(text) => crate::protocol::KeyStroke {
                        key: crate::protocol::KeyCode::Character(text.to_uppercase()),
                        modifiers: stroke.modifiers,
                    },
                    key => crate::protocol::KeyStroke {
                        key: key.clone(),
                        modifiers: stroke.modifiers,
                    },
                };
                let outcome = state.route_key_sequence(&pending, &event);
                if index + 1 == expected.len() {
                    assert!(
                        matches!(
                            outcome,
                            crate::client::behavior::ChordRouteOutcome::Matched(_)
                        ),
                        "parsed {key:?} must match its key event for {command_id}"
                    );
                } else {
                    assert_eq!(
                        outcome,
                        crate::client::behavior::ChordRouteOutcome::Pending,
                        "parsed {key:?} must hold a prefix for {command_id}"
                    );
                    pending.push(stroke.clone());
                }
            }
        }
    }
}
