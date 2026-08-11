use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::server::syntax::SyntaxEngineTier;

use super::{
    ClayOpState,
    decorations::{clay_error, parse_json},
};

#[op2]
#[string]
pub(super) fn op_clay_syntax_set_engine_preference(
    state: &mut OpState,
    #[string] target: String,
    #[string] tier: String,
) -> Result<String, JsErrorBox> {
    let tier = parse_engine_tier(&tier)?;
    state
        .borrow::<Arc<ClayOpState>>()
        .set_syntax_engine_preference(&target, tier)
        .map_err(|error| {
            clay_error(format!(
                "syntax.invalid_engine_preference: syntax engine preference rejected: {error:?}"
            ))
        })?;
    serde_json::to_string(&json!({
        "target": target,
        "tier": tier.as_str(),
    }))
    .map_err(|error| clay_error(format!("syntax.invalid_engine_preference: {error}")))
}

#[op2]
#[string]
pub(super) fn op_clay_syntax_register_syntax_grammar(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "syntax.invalid_grammar")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("syntax.invalid_grammar: options must be an object"))?;
    reject_prohibited_authority(options)?;

    // Grammar contributions come from the host-enabled record of the
    // executing package (its on-disk package.json); caller-supplied grammar
    // manifests are never consulted.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .require_current_package_capability(
            crate::packages::permissions::PackagePermission::ParseDocument,
        )?;
    if package.contributions.syntax_grammars.is_empty() {
        return Err(clay_error(
            "syntax.invalid_grammar: package must declare a syntaxGrammars contribution",
        ));
    }

    let registered = state
        .borrow::<Arc<ClayOpState>>()
        .register_syntax_grammar_package(&package)
        .map_err(|error| {
            clay_error(format!(
                "syntax.registration_failed: syntax grammar registry rejected contribution: {error:?}"
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
            "syntax.registration_failed: failed to serialize result ({error})"
        ))
    })
}

fn parse_engine_tier(tier: &str) -> Result<SyntaxEngineTier, JsErrorBox> {
    match tier {
        "native" => Ok(SyntaxEngineTier::Native),
        "wasm" => Ok(SyntaxEngineTier::Wasm),
        "javascript" | "js" => Ok(SyntaxEngineTier::JavaScriptFallback),
        _ => Err(clay_error(
            "syntax.invalid_engine_preference: tier must be native, wasm, or javascript",
        )),
    }
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
                "syntax.invalid_grammar: executable or raw authority field `{key}` is not accepted by the public registration contract"
            )));
        }
    }
    Ok(())
}
