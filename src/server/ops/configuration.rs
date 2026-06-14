use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::Value;

use crate::server::configuration::{ConfigurationRuntime, RegisteredPackageOption};

#[op2]
#[string]
pub(super) fn op_clay_configuration_load_module(
    state: &mut OpState,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    state
        .try_borrow::<Arc<ConfigurationRuntime>>()
        .ok_or_else(|| JsErrorBox::generic("clay.configuration.runtime_unavailable: configuration runtime is unavailable in this context"))?
        .validate_module_path(&path)
        .map_err(|error| error.to_js_error())?;
    Ok(path)
}

#[op2]
#[string]
pub(super) fn op_clay_configuration_get_state(state: &mut OpState) -> Result<String, JsErrorBox> {
    Ok(state
        .try_borrow::<Arc<ConfigurationRuntime>>()
        .ok_or_else(|| JsErrorBox::generic("clay.configuration.runtime_unavailable: configuration runtime is unavailable in this context"))?
        .state_json())
}

#[op2]
#[string]
pub(super) fn op_clay_configuration_set_package_option(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let value: Value = serde_json::from_str(&options_json).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.configuration.invalid_package_option: input must be valid JSON ({error})"
        ))
    })?;
    let registered = state
        .try_borrow::<Arc<ConfigurationRuntime>>()
        .ok_or_else(|| JsErrorBox::generic("clay.configuration.runtime_unavailable: configuration runtime is unavailable in this context"))?
        .set_package_option(&value)
        .map_err(|error| error.to_js_error())?;
    serde_json::to_string(&package_option_result(&registered)).map_err(|error| {
        JsErrorBox::generic(format!(
            "clay.configuration.invalid_package_option: failed to serialize result ({error})"
        ))
    })
}

fn package_option_result(registered: &RegisteredPackageOption) -> serde_json::Value {
    serde_json::json!({
        "registered": true,
        "packagePrefix": registered.package_prefix,
        "option": registered.option,
        "value": registered.value,
        "source": registered.source,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
    })
}
