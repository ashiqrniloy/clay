use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;

use crate::server::configuration::ConfigurationRuntime;

#[op2]
#[string]
pub(super) fn op_clay_configuration_load_module(
    state: &mut OpState,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    state
        .borrow::<Arc<ConfigurationRuntime>>()
        .validate_module_path(&path)
        .map_err(|error| error.to_js_error())?;
    Ok(path)
}

#[op2]
#[string]
pub(super) fn op_clay_configuration_get_state(state: &mut OpState) -> Result<String, JsErrorBox> {
    Ok(state.borrow::<Arc<ConfigurationRuntime>>().state_json())
}
