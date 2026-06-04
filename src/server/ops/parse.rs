use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::protocol::ParseUnit;

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
    reject_executable_handler(options)?;

    let registration = crate::server::parse_coordinator::ParseHandlerMeta {
        package_prefix: package.manifest.clay.api_prefix.clone(),
        mode_id: mode_id.clone(),
    };
    state
        .borrow::<Arc<ClayOpState>>()
        .register_parse_handler(registration.clone());

    serde_json::to_string(&json!({
        "packageName": package.manifest.name,
        "packageVersion": package.manifest.version,
        "packagePrefix": registration.package_prefix,
        "mode": mode_id,
        "parseUnit": parse_unit_name(parse_unit),
        "viewportPriority": viewport_priority,
        "timeoutMs": timeout_ms,
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.parse.registration_failed: failed to serialize result ({error})"
        ))
    })
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
