use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::packages::record::assemble_package_record;

use super::{
    ClayOpState,
    decorations::{clay_error, parse_json, required_str},
};

#[op2]
#[string]
pub(super) fn op_clay_syntax_register_syntax_grammar(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "clay.syntax.invalid_grammar")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("clay.syntax.invalid_grammar: options must be an object"))?;
    reject_prohibited_authority(options)?;

    let package_value = package_value_from_options(options)?;
    let package = assemble_package_record(&package_value).map_err(|error| {
        clay_error(format!(
            "clay.syntax.invalid_grammar: {:?}: {}",
            error.rule, error.message
        ))
    })?;
    if package.contributions.syntax_grammars.is_empty() {
        return Err(clay_error(
            "clay.syntax.invalid_grammar: package must declare a syntaxGrammars contribution",
        ));
    }

    let registered = state
        .borrow::<Arc<ClayOpState>>()
        .register_syntax_grammar_package(&package)
        .map_err(|error| {
            clay_error(format!(
                "clay.syntax.registration_failed: syntax grammar registry rejected contribution: {error:?}"
            ))
        })?;

    serde_json::to_string(&json!({
        "packageName": package.manifest.name,
        "packageVersion": package.manifest.version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "registeredGrammarCount": registered,
        "languages": package
            .contributions
            .syntax_grammars
            .iter()
            .map(|grammar| grammar.language_id.clone())
            .collect::<Vec<_>>(),
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.syntax.registration_failed: failed to serialize result ({error})"
        ))
    })
}

fn package_value_from_options(options: &Map<String, Value>) -> Result<Value, JsErrorBox> {
    if let Some(manifest) = options.get("packageManifest") {
        return Ok(manifest.clone());
    }

    let package_name = required_str(options, "packageName", "clay.syntax.invalid_grammar")?;
    let package_version = options
        .get("packageVersion")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0");
    let api_prefix = required_str(options, "packagePrefix", "clay.syntax.invalid_grammar")
        .or_else(|_| required_str(options, "apiPrefix", "clay.syntax.invalid_grammar"))?;
    let permissions = options
        .get("permissions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            clay_error(
                "clay.syntax.invalid_grammar: permissions must include parse-document and render-decorations",
            )
        })?;
    let syntax_grammar = options
        .get("syntaxGrammar")
        .or_else(|| options.get("contribution"))
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "languageId": options.get("languageId").cloned().unwrap_or(Value::Null),
                "filePatterns": options.get("filePatterns").cloned().unwrap_or(Value::Null),
                "grammar": options.get("grammar").cloned().unwrap_or(Value::Null),
                "queries": options.get("queries").cloned().unwrap_or(Value::Null),
                "styleMap": options.get("styleMap").cloned().unwrap_or(Value::Null),
                "budgets": options.get("budgets").cloned().unwrap_or(Value::Null),
            })
        });

    Ok(json!({
        "name": package_name,
        "version": package_version,
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "permissions": permissions,
            "modes": [],
            "docs": "./docs/index.md",
            "apiDependencies": ["clay.syntax.serverRegisterSyntaxGrammar"],
            "contributions": { "syntaxGrammars": [syntax_grammar] }
        }
    }))
}

fn reject_prohibited_authority(options: &Map<String, Value>) -> Result<(), JsErrorBox> {
    for key in [
        "handler",
        "callback",
        "onParse",
        "function",
        "clientJavaScript",
        "nativeHandle",
        "rawOps",
    ] {
        if options.contains_key(key) {
            return Err(clay_error(format!(
                "clay.syntax.invalid_grammar: executable or raw authority field `{key}` is not accepted by the public registration contract"
            )));
        }
    }
    Ok(())
}
