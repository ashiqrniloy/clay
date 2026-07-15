//! Phase 18.20 language-intelligence provider registration ops.
//!
//! Packages register feature-tagged providers under `parse-document`. Registration
//! accepts package provenance plus a package-root-confined module/export
//! declaration and issues a runtime token. No function value, process handle,
//! filesystem, network, or shell authority crosses this boundary. Process use
//! separately requires the deny-by-default `language-server` grant.

use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    packages::record::PackageRecord,
    perf::budgets::{
        LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS, LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS,
    },
    protocol::{CompletionProvenance, LanguageIntelligenceFeature},
    server::language_intelligence::{
        JsLanguageIntelligenceProviderRegistration, LanguageIntelligenceProviderMeta,
    },
};

use super::{
    ClayOpState,
    decorations::{clay_error, optional_u64, package_from_options, parse_json, required_str},
};

#[op2]
#[string]
pub(super) fn op_clay_language_register_intelligence_provider(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "clay.language.invalid_provider")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("clay.language.invalid_provider: options must be an object"))?;
    reject_executable_fields(options)?;

    let package = package_from_options(options, "parse-document")?;
    let provider_options = options
        .get("provider")
        .and_then(Value::as_object)
        .unwrap_or(options);

    let meta = provider_meta_from_options(&package, provider_options)?;
    let export_name = provider_options
        .get("exportName")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .unwrap_or("provideLanguageIntelligence")
        .to_string();
    let runtime_bridge = options
        .get("runtimeBridge")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let token = format!(
        "{}:{}:{}",
        package.manifest.clay.api_prefix,
        meta.id,
        state
            .borrow::<Arc<ClayOpState>>()
            .language_intelligence_providers()
            .len()
    );

    let registration = JsLanguageIntelligenceProviderRegistration {
        package: package.clone(),
        meta: meta.clone(),
        token: token.clone(),
        export_name: export_name.clone(),
    };

    let clay = state.borrow::<Arc<ClayOpState>>();
    clay.register_language_intelligence_provider_metadata(meta.clone())
        .map_err(|message| clay_error(format!("clay.language.registration_failed: {message}")))?;
    if runtime_bridge {
        clay.register_js_language_intelligence_provider(registration);
    }

    serde_json::to_string(&json!({
        "packageName": package.manifest.name,
        "packageVersion": package.manifest.version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "providerId": meta.id,
        "features": meta.features.iter().map(feature_name).collect::<Vec<_>>(),
        "modes": meta.modes,
        "priority": meta.priority,
        "timeoutMs": meta.timeout_ms,
        "token": token,
        "exportName": export_name,
        "runtimeBridge": runtime_bridge,
        "languageServerRequired": false,
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.language.registration_failed: failed to serialize result ({error})"
        ))
    })
}

fn provider_meta_from_options(
    package: &PackageRecord,
    options: &Map<String, Value>,
) -> Result<LanguageIntelligenceProviderMeta, JsErrorBox> {
    if let Some(descriptor) = package
        .contributions
        .language_intelligence_providers
        .first()
        && options.get("id").is_none()
        && options.get("features").is_none()
    {
        return Ok(LanguageIntelligenceProviderMeta {
            id: descriptor.id.clone(),
            provenance: CompletionProvenance {
                package_name: package.manifest.name.clone(),
                package_version: package.manifest.version.clone(),
                package_prefix: package.manifest.clay.api_prefix.clone(),
            },
            modes: descriptor.modes.clone(),
            features: descriptor.features.clone(),
            priority: descriptor.priority,
            timeout_ms: descriptor.timeout_ms,
            generation: 0,
        });
    }

    let id = required_str(options, "id", "clay.language.invalid_provider")?.to_string();
    let api_prefix = package.manifest.clay.api_prefix.as_str();
    if id.starts_with("clay.") {
        return Err(clay_error(format!(
            "clay.language.invalid_provider: provider id `{id}` claims the reserved clay.* namespace"
        )));
    }
    if !(id == api_prefix
        || id
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.')))
    {
        return Err(clay_error(format!(
            "clay.language.invalid_provider: provider id `{id}` must be owned by apiPrefix `{api_prefix}`"
        )));
    }

    let features = parse_features(options.get("features"))?;
    if features.is_empty() {
        return Err(clay_error(
            "clay.language.invalid_provider: features must contain at least one entry",
        ));
    }
    let modes = match options.get("modes") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|mode| !mode.is_empty() && mode.len() <= 128)
                    .map(str::to_string)
                    .ok_or_else(|| {
                        clay_error(
                            "clay.language.invalid_provider: modes entries must be non-empty strings",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(clay_error(
                "clay.language.invalid_provider: modes must be an array of strings",
            ));
        }
    };
    let priority = options.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
    let timeout_ms = optional_u64(options.get("timeoutMs"))?
        .or_else(|| {
            options
                .get("budgets")
                .and_then(Value::as_object)
                .and_then(|budgets| budgets.get("timeoutMs").and_then(Value::as_u64))
        })
        .unwrap_or(LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS);
    if timeout_ms == 0 || timeout_ms > LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS {
        return Err(clay_error(format!(
            "clay.language.invalid_provider: timeoutMs must be within 1..={LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS}"
        )));
    }

    Ok(LanguageIntelligenceProviderMeta {
        id,
        provenance: CompletionProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
        modes,
        features,
        priority,
        timeout_ms,
        generation: 0,
    })
}

fn parse_features(value: Option<&Value>) -> Result<Vec<LanguageIntelligenceFeature>, JsErrorBox> {
    let Some(Value::Array(values)) = value else {
        return Ok(Vec::new());
    };
    let mut features = Vec::new();
    for value in values {
        let Some(name) = value.as_str() else {
            return Err(clay_error(
                "clay.language.invalid_provider: features entries must be strings",
            ));
        };
        let feature = match name {
            "hover" | "Hover" => LanguageIntelligenceFeature::Hover,
            "definition" | "goToDefinition" | "GoToDefinition" => {
                LanguageIntelligenceFeature::GoToDefinition
            }
            "codeAction" | "CodeAction" => LanguageIntelligenceFeature::CodeAction,
            "signatureHelp" | "SignatureHelp" => LanguageIntelligenceFeature::SignatureHelp,
            other => {
                return Err(clay_error(format!(
                    "clay.language.invalid_provider: unsupported feature `{other}`"
                )));
            }
        };
        if !features.contains(&feature) {
            features.push(feature);
        }
    }
    Ok(features)
}

fn feature_name(feature: &LanguageIntelligenceFeature) -> &'static str {
    match feature {
        LanguageIntelligenceFeature::Hover => "hover",
        LanguageIntelligenceFeature::GoToDefinition => "definition",
        LanguageIntelligenceFeature::CodeAction => "codeAction",
        LanguageIntelligenceFeature::SignatureHelp => "signatureHelp",
    }
}

fn reject_executable_fields(options: &Map<String, Value>) -> Result<(), JsErrorBox> {
    for key in [
        "handler",
        "callback",
        "function",
        "clientJavaScript",
        "nativeHandle",
        "rawOps",
        "shellCommand",
        "executable",
        "process",
        "languageServer",
    ] {
        if options.contains_key(key) {
            return Err(clay_error(format!(
                "clay.language.invalid_provider: executable or process authority field `{key}` is not accepted by the public registration contract"
            )));
        }
        if let Some(provider) = options.get("provider").and_then(Value::as_object)
            && provider.contains_key(key)
        {
            return Err(clay_error(format!(
                "clay.language.invalid_provider: executable or process authority field `{key}` is not accepted by the public registration contract"
            )));
        }
    }
    Ok(())
}

#[op2(fast)]
pub(super) fn op_clay_language_store_intelligence_result(
    state: &mut OpState,
    #[string] result_json: String,
) -> Result<(), JsErrorBox> {
    let value = parse_json(&result_json, "clay.language.invalid_result")?;
    if !value.is_object() {
        return Err(clay_error(
            "clay.language.invalid_result: result must be an object",
        ));
    }
    state
        .borrow::<Arc<ClayOpState>>()
        .store_language_intelligence_result_json(result_json);
    Ok(())
}
