use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{perf::budgets::SYNTAX_CACHE_BUDGET_BYTES, protocol::ParseUnit};

use super::{
    ClayOpState,
    decorations::{clay_error, optional_u64, package_from_options, parse_json, required_str},
};

#[op2]
#[string]
pub(super) fn op_clay_parse_register_parse_handler(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "clay.parse.invalid_handler")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("clay.parse.invalid_handler: options must be an object"))?;
    let package = package_from_options(options, "parse-document")?;
    let mode_id = required_str(options, "mode", "clay.parse.invalid_handler")?.to_string();
    let parse_unit = parse_unit(
        options
            .get("parseUnit")
            .or_else(|| options.get("parseUnits"))
            .and_then(Value::as_str)
            .unwrap_or("line-group"),
    )?;
    let viewport_priority = options
        .get("viewportPriority")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let timeout_ms = optional_u64(options.get("timeoutMs"))?.unwrap_or(50);
    if timeout_ms == 0 || timeout_ms > 5_000 {
        return Err(clay_error(
            "clay.parse.invalid_handler: timeoutMs must be between 1 and 5000",
        ));
    }
    let max_window_bytes = optional_u64(
        options
            .get("maxWindowBytes")
            .or_else(|| options.get("parseWindowBytes")),
    )?
    .unwrap_or(64 * 1024);
    let guard_bytes = optional_u64(options.get("guardBytes"))?.unwrap_or(4 * 1024);
    let memory_budget_bytes =
        optional_u64(options.get("memoryBudgetBytes"))?.unwrap_or(SYNTAX_CACHE_BUDGET_BYTES as u64);
    if max_window_bytes == 0
        || memory_budget_bytes == 0
        || max_window_bytes > memory_budget_bytes
        || memory_budget_bytes > SYNTAX_CACHE_BUDGET_BYTES as u64
    {
        return Err(clay_error(
            "clay.parse.invalid_handler: window and memory budgets must be non-zero, bounded, and within the syntax cache budget",
        ));
    }
    reject_executable_handler(options)?;

    let token = format!(
        "{}:{}:{}",
        package.manifest.clay.api_prefix,
        mode_id,
        state.borrow::<Arc<ClayOpState>>().parse_handlers().len()
    );
    let meta = crate::server::parse_coordinator::ParseHandlerMeta {
        package_prefix: package.manifest.clay.api_prefix.clone(),
        mode_id: mode_id.clone(),
    };
    if options
        .get("runtimeBridge")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        state
            .borrow::<Arc<ClayOpState>>()
            .register_js_parse_handler(
                crate::server::parse_coordinator::JsParseHandlerRegistration {
                    package: package.clone(),
                    meta: meta.clone(),
                    token: token.clone(),
                    parse_unit,
                    timeout_ms,
                },
            );
    } else {
        state
            .borrow::<Arc<ClayOpState>>()
            .register_parse_handler_meta(meta.clone());
    }

    serde_json::to_string(&json!({
        "packageName": package.manifest.name,
        "packageVersion": package.manifest.version,
        "packagePrefix": meta.package_prefix,
        "mode": mode_id,
        "token": token,
        "parseUnit": parse_unit_name(parse_unit),
        "viewportPriority": viewport_priority,
        "timeoutMs": timeout_ms,
        "parsePolicy": {
            "maxWindowBytes": max_window_bytes,
            "guardBytes": guard_bytes,
            "memoryBudgetBytes": memory_budget_bytes,
        },
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.parse.registration_failed: failed to serialize result ({error})"
        ))
    })
}

#[op2(fast)]
pub(super) fn op_clay_parse_store_update(
    state: &mut OpState,
    #[string] update_json: String,
) -> Result<(), JsErrorBox> {
    let value = parse_json(&update_json, "clay.parse.invalid_update")?;
    if !value.is_object() {
        return Err(clay_error(
            "clay.parse.invalid_update: update must be an object",
        ));
    }
    state
        .borrow::<Arc<ClayOpState>>()
        .store_parse_update_json(update_json);
    Ok(())
}

fn parse_unit(value: &str) -> Result<ParseUnit, JsErrorBox> {
    match value {
        "file" | "File" => Ok(ParseUnit::File),
        "region" | "Region" => Ok(ParseUnit::Region),
        "line-group" | "lineGroup" | "LineGroup" => Ok(ParseUnit::LineGroup),
        other => Err(clay_error(format!(
            "clay.parse.invalid_handler: unsupported parseUnit `{other}`"
        ))),
    }
}

fn parse_unit_name(parse_unit: ParseUnit) -> &'static str {
    match parse_unit {
        ParseUnit::File => "file",
        ParseUnit::Region => "region",
        ParseUnit::LineGroup => "line-group",
    }
}

fn reject_executable_handler(options: &Map<String, Value>) -> Result<(), JsErrorBox> {
    for key in ["handler", "callback", "onParse", "function"] {
        if options.contains_key(key) {
            return Err(clay_error(format!(
                "clay.parse.invalid_handler: executable `{key}` callbacks are not accepted by the public registration contract"
            )));
        }
    }
    Ok(())
}
