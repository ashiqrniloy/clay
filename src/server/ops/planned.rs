use deno_core::{OpState, op2};
use deno_error::JsErrorBox;

#[op2(fast)]
pub(super) fn op_clay_runtime_unavailable(
    _state: &mut OpState,
    #[string] api: String,
) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic(format!(
        "{api} is planned; Clay JS runtime op wiring is not implemented yet"
    )))
}
