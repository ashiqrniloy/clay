//! Phase 22.1 `setPaneFocusPolicy` Clay JS op (`clay:shell` facade).
//!
//! Validates the pane-focus policy string (`"click"` or `"cursor"`) and
//! publishes the updated [`ShellPreferences`] to connected clients. The
//! setting is inert configuration data: it controls only client-side pointer
//! event handling in `ClayShellWidget` and grants no filesystem, network,
//! shell, extension, AI, workspace, or package authority.

use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::protocol::ShellPreferences;

use super::ClayOpState;

const VALID_PANE_FOCUS_POLICIES: &[&str] = &["click", "cursor"];

/// Validate and publish the pane-focus policy preference. Called from
/// `clay:shell`'s `setPaneFocusPolicy` facade during `init.js` evaluation.
#[op2]
#[string]
pub(super) fn op_clay_shell_set_pane_focus_policy(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let value: Value = serde_json::from_str(&options_json).map_err(|error| {
        JsErrorBox::generic(format!(
            "shell.invalid_pane_focus_policy: input must be valid JSON ({error})"
        ))
    })?;
    let policy = value
        .get("paneFocusPolicy")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            JsErrorBox::generic(
                "shell.invalid_pane_focus_policy: requires { paneFocusPolicy: \"click\" | \"cursor\" }",
            )
        })?;
    if !VALID_PANE_FOCUS_POLICIES.contains(&policy) {
        return Err(JsErrorBox::generic(format!(
            "shell.invalid_pane_focus_policy: unknown value `{policy}`; expected \"click\" or \"cursor\""
        )));
    }
    let preferences = ShellPreferences {
        pane_focus_policy: policy.to_string(),
    };
    let clay_state = state.borrow::<Arc<ClayOpState>>();
    clay_state.publish_shell_preferences(preferences.clone());
    serde_json::to_string(&json!({ "paneFocusPolicy": preferences.pane_focus_policy })).map_err(
        |error| {
            JsErrorBox::generic(format!(
                "shell.invalid_pane_focus_policy: failed to serialize result ({error})"
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_policies_are_click_and_cursor_only() {
        assert_eq!(VALID_PANE_FOCUS_POLICIES, &["click", "cursor"]);
    }
}
