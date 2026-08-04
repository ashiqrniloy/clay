//! Clay editor client-executed ops (Plan 071 task 5 / E.1 part 2).
//!
//! `op_clay_editor_move_cursor` / `op_clay_editor_set_selection` are the
//! programmatic Clay JS API surface for client-local caret/selection state.
//! They validate typed arguments with a deny-by-default enum policy (unknown
//! values error) and return the validated payload. Key-driven movement is
//! served client-local by the direction-specific `clay.editor.clientMoveCursor.*`
//! / `clay.editor.clientSetSelection.*` command IDs (allowlisted in
//! `keybindings.rs`, routed `ClientUiCommand`, dispatched in `EditorWidget`).
//!
//! `ponytail:` these ops validate + return the command descriptor; live
//! programmatic execution that reaches the client caret is served by
//! `op_clay_editor_execute_command` over the bounded `EditorCommandRequest`
//! push channel (follow-up round). The keybinding route remains the execution
//! path for key-driven movement.
//!
//! ## `editor-control` trust gate (approved 2026-08-03)
//!
//! All editor ops are registered in BOTH runtime domains. Every call passes
//! [`require_editor_control`]: inside a package activation (a `loadPackage`
//! loadEntry or a host-invoked package callback) the package must hold
//! approved `editor-control` AND the active document's major mode must be one
//! of its declared `clay.editorControl.modes` (deny-by-default).
//! Trusted-domain callers outside any package activation (user configuration)
//! are allowed; package callers never bypass the mode gate. The attribution
//! stamp may outlive activations for later package-facing registrations, so
//! the gate keys on the activation scope, not on stamp presence.

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};
use std::sync::Arc;

/// Enforce the `editor-control` trust gate for every editor op. See the
/// module docs for the boundary rules.
pub(super) fn require_editor_control(state: &OpState) -> Result<(), JsErrorBox> {
    let op_state = state
        .borrow::<Arc<crate::server::ops::ClayOpState>>()
        .clone();
    if !op_state.in_package_activation() {
        // Outside package code: trusted user configuration only.
        if matches!(
            op_state.domain,
            crate::packages::bundled::RuntimeDomain::Trusted
        ) {
            return Ok(());
        }
        return Err(JsErrorBox::generic(
            "clay.editor.missing_permission: editor ops require an active package context",
        ));
    }
    let record = op_state.require_current_package_capability(
        crate::packages::permissions::PackagePermission::EditorControl,
    )?;
    let Some(active_mode) = op_state.active_editor_mode_id() else {
        return Err(JsErrorBox::generic(format!(
            "clay.editor.mode_not_active: package `{}` cannot use editor ops without an active document major mode",
            record.manifest.name
        )));
    };
    if !record
        .manifest
        .clay
        .editor_control_modes
        .contains(&active_mode)
    {
        return Err(JsErrorBox::generic(format!(
            "clay.editor.mode_not_declared: package `{}` declared editor-control for {:?}, not active mode `{active_mode}`",
            record.manifest.name, record.manifest.clay.editor_control_modes
        )));
    }
    Ok(())
}

/// Valid movement directions for `clientMoveCursor`. The vocabulary covers the
/// task-4 `EditorCommand` motion set plus the legacy Phase-7 axis names so the
/// documented `client-move-cursor.md` contract keeps working.
const MOVE_DIRECTIONS: &[&str] = &[
    "nextWordStart",
    "prevWordStart",
    "nextWordEnd",
    "prevWordEnd",
    "nextParagraph",
    "prevParagraph",
    "firstNonWhitespace",
    "lastNonWhitespace",
    "matchingPair",
    // Legacy axis aliases (kept for back-compat with the Phase-7 doc contract).
    "left",
    "right",
    "up",
    "down",
    "start",
    "end",
];

const MOVE_GRANULARITIES: &[&str] = &["word", "subword", "paragraph", "line", "character"];

const SELECTION_ACTIONS: &[&str] = &["selectWord", "selectLine", "selectParagraph"];

const SELECTION_DIRECTIONS: &[&str] = &["current", "next", "prev"];

const CURSOR_SHAPES: &[&str] = &["bar", "line", "block", "underline"];
const CURSOR_BLINKS: &[&str] = &["solid", "blink", "phase", "smooth"];

/// Plan 071 task 9 multi-cursor directions.
const ADD_CURSOR_DIRECTIONS: &[&str] = &["below", "above"];
const COLUMN_SELECT_DIRECTIONS: &[&str] = &["down", "up", "left", "right"];

/// Plan 071 task 10 text-object vocabulary (deny-by-default; mirrors
/// `TextobjectKind`/`TextobjectDirection`/`SmartSelectAction`).
const TEXTOBJECT_KINDS: &[&str] = &[
    "function",
    "class",
    "argument",
    "comment",
    "loop",
    "conditional",
    "call",
    "statement",
];
const TEXTOBJECT_DIRECTIONS: &[&str] = &["current", "next", "previous"];
const SMART_SELECT_ACTIONS: &[&str] = &["expand", "shrink"];

fn parse_options(options_json: &str, error_code: &str) -> Result<Value, JsErrorBox> {
    serde_json::from_str::<Value>(options_json).map_err(|error| {
        JsErrorBox::generic(format!(
            "{error_code}: options must be a JSON object ({error})"
        ))
    })
}

fn require_string(
    value: &Value,
    key: &str,
    allowed: &[&str],
    error_code: &str,
) -> Result<String, JsErrorBox> {
    let raw = value
        .get(key)
        .and_then(|entry| entry.as_str())
        .ok_or_else(|| {
            JsErrorBox::generic(format!(
                "{error_code}: missing required string `{key}` (one of {})",
                allowed.join(", ")
            ))
        })?;
    if !allowed.contains(&raw) {
        return Err(JsErrorBox::generic(format!(
            "{error_code}: unknown `{key}` value `{raw}` (expected one of {})",
            allowed.join(", ")
        )));
    }
    Ok(raw.to_string())
}

fn optional_string(value: &Value, key: &str, allowed: &[&str]) -> Option<String> {
    value
        .get(key)
        .and_then(|entry| entry.as_str())
        .and_then(|raw| allowed.contains(&raw).then(|| raw.to_string()))
}

fn optional_bool(value: &Value, key: &str, default: bool) -> bool {
    value
        .get(key)
        .and_then(|entry| entry.as_bool())
        .unwrap_or(default)
}

fn optional_count(value: &Value, key: &str) -> u32 {
    value
        .get(key)
        .and_then(|entry| entry.as_u64())
        .map(|raw| raw.clamp(1, u32::MAX as u64) as u32)
        .unwrap_or(1)
}

/// Optional string that is deny-by-default when present: an absent/`null` key
/// yields `None`, a present-but-unknown value errors.
fn optional_string_strict(
    value: &Value,
    key: &str,
    allowed: &[&str],
    error_code: &str,
) -> Result<Option<String>, JsErrorBox> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(entry) => {
            let raw = entry.as_str().ok_or_else(|| {
                JsErrorBox::generic(format!("{error_code}: `{key}` must be a string"))
            })?;
            if !allowed.contains(&raw) {
                return Err(JsErrorBox::generic(format!(
                    "{error_code}: unknown `{key}` value `{raw}` (expected one of {})",
                    allowed.join(", ")
                )));
            }
            Ok(Some(raw.to_string()))
        }
    }
}

/// Validate `clientMoveCursor` options (deny-by-default enum). Returns the
/// validated descriptor as a JSON object string. Plain (non-`op2`) so it is
/// unit-testable.
pub(super) fn validate_move_cursor(options_json: &str) -> Result<String, JsErrorBox> {
    let value = parse_options(options_json, "clay.editor.invalid_move_cursor")?;
    let direction = require_string(
        &value,
        "direction",
        MOVE_DIRECTIONS,
        "clay.editor.invalid_move_cursor",
    )?;
    let granularity = optional_string(&value, "granularity", MOVE_GRANULARITIES);
    let extend = optional_bool(&value, "extend", false);
    let count = optional_count(&value, "count");
    serde_json::to_string(&json!({
        "commandId": "clay.editor.clientMoveCursor",
        "direction": direction,
        "granularity": granularity,
        "extend": extend,
        "count": count,
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.editor.invalid_move_cursor: failed to serialize result ({error})"
        ))
    })
}

/// Validate `clientSetSelection` options (deny-by-default enum).
pub(super) fn validate_set_selection(options_json: &str) -> Result<String, JsErrorBox> {
    let value = parse_options(options_json, "clay.editor.invalid_set_selection")?;
    let action = require_string(
        &value,
        "action",
        SELECTION_ACTIONS,
        "clay.editor.invalid_set_selection",
    )?;
    let extend = optional_bool(&value, "extend", false);
    let direction = optional_string(&value, "direction", SELECTION_DIRECTIONS);
    serde_json::to_string(&json!({
        "commandId": "clay.editor.clientSetSelection",
        "action": action,
        "extend": extend,
        "direction": direction,
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.editor.invalid_set_selection: failed to serialize result ({error})"
        ))
    })
}

/// Validate `clientSetCursorStyle` options (deny-by-default enum). All fields
/// are optional; present-but-unknown `shape`/`blink` values error.
pub(super) fn validate_set_cursor_style(options_json: &str) -> Result<String, JsErrorBox> {
    let value = parse_options(options_json, "clay.editor.invalid_set_cursor_style")?;
    validate_set_cursor_style_value(&value)
}

fn validate_set_cursor_style_value(value: &Value) -> Result<String, JsErrorBox> {
    let shape = optional_string_strict(
        value,
        "shape",
        CURSOR_SHAPES,
        "clay.editor.invalid_set_cursor_style",
    )?;
    let blink = optional_string_strict(
        value,
        "blink",
        CURSOR_BLINKS,
        "clay.editor.invalid_set_cursor_style",
    )?;
    let width_px = value.get("widthPx").and_then(Value::as_f64);
    let height_pct = value.get("heightPct").and_then(Value::as_f64);
    let hollow = optional_bool(value, "hollow", false);
    let stop_blink_on_typing = optional_bool(value, "stopBlinkOnTyping", true);
    serde_json::to_string(&json!({
        "commandId": "clay.editor.clientSetCursorStyle",
        "shape": shape,
        "blink": blink,
        "widthPx": width_px,
        "heightPct": height_pct,
        "hollow": hollow,
        "stopBlinkOnTyping": stop_blink_on_typing,
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.editor.invalid_set_cursor_style: failed to serialize result ({error})"
        ))
    })
}

/// Build the runtime caret override from partial options: recognized fields
/// merge over the active below-override style (manifest `caret_style`, else
/// the Clay default); no recognized field clears the override (`None`) so the
/// layers below show through again.
fn build_cursor_style_override(
    state: &OpState,
    value: &Value,
) -> Result<Option<crate::protocol::CaretStyle>, JsErrorBox> {
    const RECOGNIZED: &[&str] = &[
        "shape",
        "blink",
        "widthPx",
        "heightPct",
        "hollow",
        "smoothAnimationMs",
        "stopBlinkOnTyping",
    ];
    if !RECOGNIZED.iter().any(|key| value.get(key).is_some()) {
        return Ok(None);
    }
    let base = state
        .borrow::<Arc<crate::server::ops::ClayOpState>>()
        .behavior_manifest()
        .editor_rules
        .caret_style
        .unwrap_or_default();
    let style = super::modes::merge_caret_style(base, value);
    style.validate().map_err(|_| {
        JsErrorBox::generic(
            "clay.editor.invalid_set_cursor_style: caret style fields out of bounds",
        )
    })?;
    Ok(Some(style))
}

#[op2]
#[string]
pub(super) fn op_clay_editor_move_cursor(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    validate_move_cursor(&options_json)
}

#[op2]
#[string]
pub(super) fn op_clay_editor_set_selection(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    validate_set_selection(&options_json)
}

#[op2]
#[string]
pub(super) fn op_clay_editor_set_cursor_style(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    let value = parse_options(&options_json, "clay.editor.invalid_set_cursor_style")?;
    let descriptor = validate_set_cursor_style_value(&value)?;
    // Plan 071 caret-transport fix: validation alone never reached the
    // client; publish the merged override (or `None` to clear) so the
    // running editor applies it.
    let override_style = build_cursor_style_override(state, &value)?;
    state
        .borrow::<Arc<crate::server::ops::ClayOpState>>()
        .publish_caret_style_override(override_style);
    Ok(descriptor)
}

/// Validate `clientAddCursor` options (deny-by-default enum). Returns the
/// direction-specific command ID descriptor (Plan 071 task 9).
pub(super) fn validate_add_cursor(options_json: &str) -> Result<String, JsErrorBox> {
    let value = parse_options(options_json, "clay.editor.invalid_add_cursor")?;
    let direction = require_string(
        &value,
        "direction",
        ADD_CURSOR_DIRECTIONS,
        "clay.editor.invalid_add_cursor",
    )?;
    let command_id = format!("clay.editor.clientAddCursor.{direction}");
    serde_json::to_string(&json!({
        "commandId": command_id,
        "direction": direction,
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.editor.invalid_add_cursor: failed to serialize result ({error})"
        ))
    })
}

/// Validate `clientColumnSelect` options (deny-by-default enum).
pub(super) fn validate_column_select(options_json: &str) -> Result<String, JsErrorBox> {
    let value = parse_options(options_json, "clay.editor.invalid_column_select")?;
    let direction = require_string(
        &value,
        "direction",
        COLUMN_SELECT_DIRECTIONS,
        "clay.editor.invalid_column_select",
    )?;
    let command_id = format!("clay.editor.clientColumnSelect.{direction}");
    serde_json::to_string(&json!({
        "commandId": command_id,
        "direction": direction,
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.editor.invalid_column_select: failed to serialize result ({error})"
        ))
    })
}

#[op2]
#[string]
pub(super) fn op_clay_editor_add_cursor(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    validate_add_cursor(&options_json)
}

#[op2]
#[string]
pub(super) fn op_clay_editor_column_select(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    validate_column_select(&options_json)
}

#[op2]
#[string]
pub(super) fn op_clay_editor_select_textobject(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    validate_select_textobject(&options_json)
}

#[op2]
#[string]
pub(super) fn op_clay_editor_smart_select(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    validate_smart_select(&options_json)
}

/// Follow-up round (`editor-control`): gated programmatic execution channel.
/// Validates a KNOWN editor command ID (deny-by-default re-parse), passes the
/// same trust gate as every editor op, then publishes an advisory
/// `EditorCommandRequest` that connection loops forward to the client. The
/// client dispatches the ID through the same path as keybinding-routed
/// command IDs; unknown IDs are dropped client-side too.
#[op2]
#[string]
pub(super) fn op_clay_editor_execute_command(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    require_editor_control(state)?;
    let op_state = state
        .borrow::<Arc<crate::server::ops::ClayOpState>>()
        .clone();

    let value = parse_options(&options_json, "clay.editor.invalid_execute_command")?;
    let Some(command_id) = value.get("commandId").and_then(Value::as_str) else {
        return Err(JsErrorBox::generic(
            "clay.editor.invalid_execute_command: commandId must be a string",
        ));
    };
    if command_id.is_empty()
        || command_id.len() > crate::protocol::MAX_EDITOR_COMMAND_REQUEST_ID_BYTES
    {
        return Err(JsErrorBox::generic(
            "clay.editor.invalid_execute_command: commandId must be a bounded string",
        ));
    }
    // Known-command allowlist: only IDs the client can dispatch as editor
    // commands (movement/selection/caret/multi-cursor/textobject/smart-select).
    let known = crate::masonry_editor::EditorClientCommand::from_command_id(command_id).is_some()
        || crate::protocol::SelectionQuery::from_command_id(command_id).is_some();
    if !known {
        return Err(JsErrorBox::generic(format!(
            "clay.editor.invalid_execute_command: `{command_id}` is not a known editor command ID"
        )));
    }

    let package_prefix = if op_state.in_package_activation() {
        op_state
            .current_package_record()
            .map_err(|_| {
                JsErrorBox::generic(
                    "clay.editor.invalid_execute_command: executing package is no longer enabled",
                )
            })?
            .manifest
            .clay
            .api_prefix
    } else {
        "clay.config".to_string()
    };
    let mode_id = op_state
        .active_editor_mode_id()
        .unwrap_or_else(|| "clay.default".to_string());

    let published = op_state.publish_editor_command(crate::protocol::EditorCommandRequest {
        command_id: command_id.to_string(),
        package_prefix,
        mode_id,
    });
    serde_json::to_string(&json!({
        "requested": true,
        "published": published,
        "commandId": command_id
    }))
    .map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.editor.invalid_execute_command: failed to serialize result ({error})"
        ))
    })
}

/// Validate `clientSelectTextobject` options (deny-by-default enums) and
/// return the direction-specific command ID for `bindKey` (Plan 071 task 10).
/// `around` defaults to `false` (inner), `direction` to `current`.
pub(super) fn validate_select_textobject(options_json: &str) -> Result<String, JsErrorBox> {
    const CODE: &str = "clay.editor.invalid_select_textobject";
    let value = parse_options(options_json, CODE)?;
    let object = require_string(&value, "object", TEXTOBJECT_KINDS, CODE)?;
    let around = optional_bool(&value, "around", false);
    let direction = optional_string_strict(&value, "direction", TEXTOBJECT_DIRECTIONS, CODE)?
        .unwrap_or_else(|| "current".to_string());
    let kind = crate::protocol::TextobjectKind::parse(&object).expect("validated kind");
    let parsed_direction =
        crate::protocol::TextobjectDirection::parse(&direction).expect("validated direction");
    let command_id = crate::protocol::SelectionQuery::Textobject {
        kind,
        around,
        direction: parsed_direction,
    }
    .command_id();
    serde_json::to_string(&json!({
        "commandId": command_id,
        "object": object,
        "around": around,
        "direction": direction,
    }))
    .map_err(|error| JsErrorBox::generic(format!("{CODE}: failed to serialize result ({error})")))
}

/// Validate `clientSmartSelect` options (deny-by-default enum) and return the
/// action-specific command ID for `bindKey` (Plan 071 task 10).
pub(super) fn validate_smart_select(options_json: &str) -> Result<String, JsErrorBox> {
    const CODE: &str = "clay.editor.invalid_smart_select";
    let value = parse_options(options_json, CODE)?;
    let action = require_string(&value, "action", SMART_SELECT_ACTIONS, CODE)?;
    let parsed_action =
        crate::protocol::SmartSelectAction::parse(&action).expect("validated action");
    let command_id = crate::protocol::SelectionQuery::SmartSelect {
        action: parsed_action,
    }
    .command_id();
    serde_json::to_string(&json!({
        "commandId": command_id,
        "action": action,
    }))
    .map_err(|error| JsErrorBox::generic(format!("{CODE}: failed to serialize result ({error})")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_cursor_validates_known_direction() {
        let result =
            validate_move_cursor(r#"{"direction":"nextWordStart","extend":true,"count":3}"#)
                .unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["direction"], "nextWordStart");
        assert_eq!(parsed["extend"], true);
        assert_eq!(parsed["count"], 3);
        assert_eq!(parsed["commandId"], "clay.editor.clientMoveCursor");
    }

    #[test]
    fn move_cursor_rejects_unknown_direction_deny_by_default() {
        let err = validate_move_cursor(r#"{"direction":"sideways"}"#).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown `direction` value `sideways`")
        );
    }

    #[test]
    fn move_cursor_rejects_missing_direction() {
        let err = validate_move_cursor("{}").unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required string `direction`")
        );
    }

    #[test]
    fn move_cursor_rejects_malformed_json() {
        let err = validate_move_cursor("{not json").unwrap_err();
        assert!(err.to_string().contains("options must be a JSON object"));
    }

    #[test]
    fn move_cursor_clamps_count_to_one() {
        let result = validate_move_cursor(r#"{"direction":"right"}"#).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["count"], 1);
        assert_eq!(parsed["extend"], false);
        assert_eq!(parsed["granularity"], serde_json::Value::Null);
    }

    #[test]
    fn set_selection_validates_known_action() {
        let result = validate_set_selection(r#"{"action":"selectLine"}"#).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["action"], "selectLine");
        assert_eq!(parsed["commandId"], "clay.editor.clientSetSelection");
    }

    #[test]
    fn set_selection_rejects_unknown_action_deny_by_default() {
        let err = validate_set_selection(r#"{"action":"selectSentence"}"#).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown `action` value `selectSentence`")
        );
    }

    #[test]
    fn set_cursor_style_validates_known_shape_and_blink() {
        let result =
            validate_set_cursor_style(r#"{"shape":"block","blink":"solid","hollow":true}"#)
                .unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["shape"], "block");
        assert_eq!(parsed["blink"], "solid");
        assert_eq!(parsed["hollow"], true);
        assert_eq!(parsed["commandId"], "clay.editor.clientSetCursorStyle");
    }

    #[test]
    fn set_cursor_style_all_fields_optional() {
        let result = validate_set_cursor_style(r#"{"widthPx":2.5}"#).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["shape"], serde_json::Value::Null);
        assert_eq!(parsed["blink"], serde_json::Value::Null);
        assert_eq!(parsed["widthPx"], 2.5);
        assert_eq!(parsed["stopBlinkOnTyping"], true);
    }

    #[test]
    fn set_cursor_style_rejects_unknown_shape_deny_by_default() {
        let err = validate_set_cursor_style(r#"{"shape":"diamond"}"#).unwrap_err();
        assert!(err.to_string().contains("unknown `shape` value `diamond`"));
    }

    #[test]
    fn set_cursor_style_rejects_unknown_blink_deny_by_default() {
        let err = validate_set_cursor_style(r#"{"blink":"strobe"}"#).unwrap_err();
        assert!(err.to_string().contains("unknown `blink` value `strobe`"));
    }

    #[test]
    fn add_cursor_maps_direction_to_command_id() {
        let below = validate_add_cursor(r#"{"direction":"below"}"#).unwrap();
        let parsed: Value = serde_json::from_str(&below).unwrap();
        assert_eq!(parsed["commandId"], "clay.editor.clientAddCursor.below");

        let above = validate_add_cursor(r#"{"direction":"above"}"#).unwrap();
        let parsed: Value = serde_json::from_str(&above).unwrap();
        assert_eq!(parsed["commandId"], "clay.editor.clientAddCursor.above");
    }

    #[test]
    fn add_cursor_rejects_unknown_direction_deny_by_default() {
        let err = validate_add_cursor(r#"{"direction":"sideways"}"#).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown `direction` value `sideways`")
        );
    }

    #[test]
    fn column_select_maps_direction_to_command_id() {
        for direction in ["down", "up", "left", "right"] {
            let result =
                validate_column_select(&format!(r#"{{"direction":"{direction}"}}"#)).unwrap();
            let parsed: Value = serde_json::from_str(&result).unwrap();
            assert_eq!(
                parsed["commandId"],
                format!("clay.editor.clientColumnSelect.{direction}")
            );
        }
    }

    #[test]
    fn column_select_rejects_unknown_direction_deny_by_default() {
        let err = validate_column_select(r#"{"direction":"diagonal"}"#).unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown `direction` value `diagonal`")
        );
    }

    #[test]
    fn select_textobject_maps_to_direction_specific_command_id() {
        let result =
            validate_select_textobject(r#"{"object":"function","around":true,"direction":"next"}"#)
                .unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            parsed["commandId"],
            "clay.editor.clientSelectTextobject.function.around.next"
        );
        // Defaults: inner + current.
        let result = validate_select_textobject(r#"{"object":"comment"}"#).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            parsed["commandId"],
            "clay.editor.clientSelectTextobject.comment.inner"
        );
        assert_eq!(parsed["around"], false);
        assert_eq!(parsed["direction"], "current");
    }

    #[test]
    fn select_textobject_rejects_unknown_values_deny_by_default() {
        let err = validate_select_textobject(r#"{"object":"widget"}"#).unwrap_err();
        assert!(err.to_string().contains("unknown `object` value `widget`"));
        let err = validate_select_textobject(r#"{"object":"function","direction":"sideways"}"#)
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown `direction` value `sideways`")
        );
        let err = validate_select_textobject(r#"{}"#).unwrap_err();
        assert!(err.to_string().contains("missing required string `object`"));
    }

    #[test]
    fn smart_select_maps_action_to_command_id() {
        for (action, expected) in [
            ("expand", "clay.editor.clientSmartSelect.expand"),
            ("shrink", "clay.editor.clientSmartSelect.shrink"),
        ] {
            let result = validate_smart_select(&format!(r#"{{"action":"{action}"}}"#)).unwrap();
            let parsed: Value = serde_json::from_str(&result).unwrap();
            assert_eq!(parsed["commandId"], expected);
        }
    }

    #[test]
    fn smart_select_rejects_unknown_action_deny_by_default() {
        let err = validate_smart_select(r#"{"action":"grow"}"#).unwrap_err();
        assert!(err.to_string().contains("unknown `action` value `grow`"));
    }
}
