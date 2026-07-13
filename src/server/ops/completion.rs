use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    packages::record::{PackageRecord, assemble_package_record},
    protocol::{
        CompletionItem,
        completion::{CompletionProvenance, CompletionProviderGeneration},
    },
    server::completion::{CompletionProviderMeta, CompletionTriggerMetadata, WordBoundaryRule},
};

use super::{
    ClayOpState,
    decorations::{clay_error, optional_u64, parse_json, required_str},
};

fn serialize_error(prefix: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| clay_error(format!("{prefix}: failed to serialize result ({error})"))
}

#[op2]
#[string]
pub(super) fn op_clay_completion_register_completion_provider(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "clay.completion.invalid_provider")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("clay.completion.invalid_provider: options must be an object"))?;
    reject_prohibited_authority(options)?;

    let package = package_value_from_options(options).and_then(|value| {
        assemble_package_record(&value).map_err(|error| {
            clay_error(format!(
                "clay.completion.invalid_provider: {:?}: {}",
                error.rule, error.message
            ))
        })
    })?;
    if package.contributions.completion_providers.is_empty() {
        return Err(clay_error(
            "clay.completion.invalid_provider: package must declare a completionProviders contribution",
        ));
    }

    let metas = completion_provider_metas(&package);
    let registered = state
        .borrow::<Arc<ClayOpState>>()
        .register_completion_provider_metadata(metas)
        .map_err(|message| clay_error(format!("clay.completion.registration_failed: {message}")))?;

    serde_json::to_string(&json!({
        "packageName": package.manifest.name,
        "packageVersion": package.manifest.version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "registeredProviderCount": registered.len(),
        "providers": registered.iter().map(|meta| meta.id.clone()).collect::<Vec<_>>(),
        "runtimeBridge": false,
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.completion.registration_failed: failed to serialize result ({error})"
        ))
    })
}

fn package_value_from_options(options: &Map<String, Value>) -> Result<Value, JsErrorBox> {
    if let Some(manifest) = options.get("packageManifest") {
        return Ok(manifest.clone());
    }

    let package_name = required_str(options, "packageName", "clay.completion.invalid_provider")?;
    let package_version = options
        .get("packageVersion")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0");
    let api_prefix = required_str(options, "packagePrefix", "clay.completion.invalid_provider")
        .or_else(|_| required_str(options, "apiPrefix", "clay.completion.invalid_provider"))?;
    let permissions = options
        .get("permissions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            clay_error(
                "clay.completion.invalid_provider: permissions must include completion-provider",
            )
        })?;
    let contribution = options
        .get("completionProvider")
        .or_else(|| options.get("contribution"))
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "id": options.get("providerId").cloned().unwrap_or(Value::Null),
                "priority": options.get("priority").cloned().unwrap_or(Value::Null),
                "triggerCharacters": trigger_characters(options),
                "wordBoundaryChars": options
                    .get("wordBoundaryChars")
                    .cloned()
                    .unwrap_or(Value::Null),
                "items": options.get("items").cloned().unwrap_or(Value::Null),
                "budgets": {
                    "timeoutMs": options.get("timeoutMs").cloned().unwrap_or(Value::Null),
                    "maxItems": options.get("maxItems").cloned().unwrap_or(Value::Null),
                }
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
            "contributions": { "completionProviders": [contribution] }
        }
    }))
}

fn trigger_characters(options: &Map<String, Value>) -> Value {
    if let Some(value) = options.get("triggerCharacters") {
        return value.clone();
    }
    options
        .get("triggers")
        .and_then(Value::as_object)
        .and_then(|triggers| triggers.get("characters"))
        .cloned()
        .unwrap_or(Value::Null)
}

fn completion_provider_metas(package: &PackageRecord) -> Vec<CompletionProviderMeta> {
    package
        .contributions
        .completion_providers
        .iter()
        .map(|descriptor| {
            let provenance = CompletionProvenance {
                package_name: package.manifest.name.clone(),
                package_version: package.manifest.version.clone(),
                package_prefix: package.manifest.clay.api_prefix.clone(),
            };
            CompletionProviderMeta {
                id: descriptor.id.clone(),
                provenance: provenance.clone(),
                priority: descriptor.priority,
                trigger_metadata: CompletionTriggerMetadata {
                    trigger_characters: descriptor.trigger_characters.clone(),
                },
                word_boundary: if descriptor.word_boundary_chars.is_empty() {
                    WordBoundaryRule::default_buffer_word()
                } else {
                    WordBoundaryRule::new(descriptor.word_boundary_chars.clone())
                },
                items: descriptor
                    .items
                    .iter()
                    .map(|item| CompletionItem::new(item, item, provenance.clone()))
                    .collect(),
                timeout_ms: descriptor.timeout_ms,
                max_items: descriptor.max_items,
                generation: CompletionProviderGeneration::default(),
            }
        })
        .collect()
}

#[op2]
#[string]
pub(super) fn op_clay_completion_providers_for_trigger(
    state: &mut OpState,
    #[string] trigger: String,
) -> Result<String, JsErrorBox> {
    let providers = state
        .borrow::<Arc<ClayOpState>>()
        .completion_providers_for_trigger(&trigger);
    serde_json::to_string(&json!({
        "trigger": trigger,
        "providers": providers.iter().map(|meta| json!({
            "id": meta.id,
            "packageName": meta.provenance.package_name,
            "packageVersion": meta.provenance.package_version,
            "packagePrefix": meta.provenance.package_prefix,
            "priority": meta.priority,
            "triggerCharacters": meta.trigger_metadata.trigger_characters,
            "wordBoundaryChars": meta.word_boundary.boundary_chars,
            "items": meta.items.iter().map(|item| &item.label).collect::<Vec<_>>(),
            "timeoutMs": meta.timeout_ms,
            "maxItems": meta.max_items,
        })).collect::<Vec<_>>(),
    }))
    .map_err(serialize_error("clay.completion.list_failed"))
}

fn reject_prohibited_authority(options: &Map<String, Value>) -> Result<(), JsErrorBox> {
    for key in [
        "handler",
        "callback",
        "complete",
        "function",
        "clientJavaScript",
        "nativeHandle",
        "rawOps",
        "module",
    ] {
        if options.contains_key(key) {
            return Err(clay_error(format!(
                "clay.completion.invalid_provider: executable or raw authority field `{key}` is not accepted by the public registration contract"
            )));
        }
    }
    if optional_u64(options.get("timeoutMs"))?.is_some_and(|value| value == 0 || value > 5_000) {
        return Err(clay_error(
            "clay.completion.invalid_provider: timeoutMs must be between 1 and 5000",
        ));
    }
    Ok(())
}
