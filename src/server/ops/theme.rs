//! `setTheme` Clay JS op (Plan 046 task 7, `clay:theme` facade).
//!
//! Selects the one active theme by resolving a first-party `@clay/*` theme
//! package's inert `clay.contributions.textStyles` overrides into a
//! [`crate::protocol::ActiveTheme`] snapshot. The snapshot is carried out in
//! [`crate::server::js_runtime::ClayRuntimeEvaluation`] and applied to the
//! shared server slot so the welcome handshake ships it to the client, which
//! reconstructs the [`crate::editor::theme::StyleRegistry`] before startup
//! paint. This is pure inert style data: no code/widgets/ops/CSS. Deny-by-
//! default for non-`@clay/*` specifiers (decision 2026-07-09-0352).

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use super::ClayOpState;
use super::packages::ensure_first_party_record;

/// Resolve `specifier` to an enabled package record's inert `textStyles`
/// overrides and record it as the active theme. Returns `{ specifier,
/// overrideCount }`. Denies any specifier that is not a first-party `@clay/*`
/// package so arbitrary theme specifiers grant no filesystem/network/extension
/// authority beyond loading a bundled package.
#[op2]
#[string]
pub(super) fn op_clay_theme_set_theme(
    state: &mut OpState,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let request: Value = serde_json::from_str(&request_json).map_err(|_| {
        JsErrorBox::generic("clay.theme.invalid_request: setTheme requires { specifier: string }")
    })?;
    let Some(specifier) = request.get("specifier").and_then(Value::as_str) else {
        return Err(JsErrorBox::generic(
            "clay.theme.invalid_request: setTheme requires a `specifier` string",
        ));
    };
    if specifier.trim().is_empty() {
        return Err(JsErrorBox::generic(
            "clay.theme.invalid_request: setTheme requires a non-empty `specifier`",
        ));
    }
    // Deny-by-default: only bundled first-party theme packages may be selected.
    // Theme packages are inert style-data packages (empty permissions/modes),
    // so loading them grants no executable authority beyond package load.
    if !specifier.starts_with("@clay/") {
        return Err(JsErrorBox::generic(format!(
            "clay.theme.unauthorized: setTheme denies non-first-party specifier `{specifier}`"
        )));
    }

    // Ensure the theme package is installed, authorized, and enabled via the
    // shared `loadPackage` resolution path. The returned validated record is
    // the single source of the package's inert `clay.contributions.textStyles`
    // overrides; theme packages declare no modes/parse handlers.
    let clay_state = state.borrow::<std::sync::Arc<ClayOpState>>();
    let (record, _package_root, _resolved_name) = ensure_first_party_record(clay_state, specifier)?;

    // Convert the RGBA-byte inert descriptors into the wire form the client
    // reconstructs a `StyleRegistry` from. Pure style data: no code/widgets/ops.
    let overrides = record
        .contributions
        .text_styles
        .iter()
        .map(|descriptor| crate::protocol::TextThemeOverride {
            token: descriptor.token.clone(),
            color: descriptor.color,
            bold: descriptor.bold,
            italic: descriptor.italic,
            underline: descriptor.underline,
            strike: descriptor.strike,
            provenance: descriptor.provenance.clone(),
        })
        .collect::<Vec<_>>();

    let snapshot = crate::protocol::ActiveTheme {
        specifier: specifier.to_string(),
        overrides: overrides.clone(),
    };
    clay_state.set_active_theme(snapshot);

    serde_json::to_string(&json!({
        "specifier": specifier,
        "overrideCount": overrides.len(),
    }))
    .map_err(|_| JsErrorBox::generic("clay.theme.invalid_request: serialization failed"))
}
