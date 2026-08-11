use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    packages::record::PackageRecord,
    protocol::{
        CompletionItem, CompletionItemTextFormat,
        completion::{CompletionProvenance, CompletionProviderGeneration},
    },
    server::completion::{
        CompletionProviderMeta, CompletionTriggerMetadata, JsCompletionProviderRegistration,
        WordBoundaryRule,
    },
};

use super::{
    ClayOpState,
    decorations::{clay_error, optional_u64, parse_json},
};

const COMPLETION_DISABLE_TARGET_MAX_CHARS: usize = 128;

#[op2(fast)]
pub(super) fn op_clay_completion_store_result(
    state: &mut OpState,
    #[string] result_json: String,
) -> Result<(), JsErrorBox> {
    if result_json.len() > crate::perf::budgets::COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES {
        return Err(clay_error(
            "completion.invalid_result: result exceeds payload budget",
        ));
    }
    // Bridge ingress revalidation (Plan 061 task 7): the executing package
    // must still be enabled; stale/revoked provider results are rejected
    // before they reach host state.
    let clay_state = state.borrow::<Arc<ClayOpState>>().clone();
    clay_state.current_package_record()?;
    clay_state.store_completion_result_json(result_json);
    Ok(())
}

fn serialize_error(prefix: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| clay_error(format!("{prefix}: failed to serialize result ({error})"))
}

#[op2]
#[string]
pub(super) fn op_clay_completion_disable(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&options_json, "completion.invalid_disable")?;
    let options = value
        .as_object()
        .ok_or_else(|| clay_error("completion.invalid_disable: options must be an object"))?;
    if options
        .keys()
        .any(|key| key != "provider" && key != "packagePrefix")
    {
        return Err(clay_error(
            "completion.invalid_disable: only provider or packagePrefix is accepted",
        ));
    }
    let provider = optional_disable_target(options, "provider")?;
    let package_prefix = optional_disable_target(options, "packagePrefix")?;
    let target = match (provider, package_prefix) {
        (Some(target), None) | (None, Some(target)) => target,
        _ => {
            return Err(clay_error(
                "completion.invalid_disable: provide exactly one non-empty provider or packagePrefix",
            ));
        }
    };
    let (disabled, generation) = state
        .borrow::<Arc<ClayOpState>>()
        .disable_completion(target.to_string());
    serde_json::to_string(&json!({
        "target": target,
        "disabled": disabled,
        "providerGeneration": generation,
    }))
    .map_err(serialize_error("completion.disable_failed"))
}

fn optional_disable_target<'a>(
    options: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, JsErrorBox> {
    let Some(value) = options.get(key) else {
        return Ok(None);
    };
    let target = value
        .as_str()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .ok_or_else(|| {
            clay_error(format!(
                "completion.invalid_disable: {key} must be a non-empty string"
            ))
        })?;
    if target.chars().count() > COMPLETION_DISABLE_TARGET_MAX_CHARS {
        return Err(clay_error(format!(
            "completion.invalid_disable: {key} exceeds {COMPLETION_DISABLE_TARGET_MAX_CHARS} characters"
        )));
    }
    Ok(Some(target))
}

#[op2]
#[string]
pub(super) fn op_clay_completion_register_completion_provider(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "completion.invalid_provider")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("completion.invalid_provider: options must be an object"))?;
    reject_prohibited_authority(options)?;
    let runtime_bridge = options
        .get("runtimeBridge")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let export_name = options
        .get("exportName")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .unwrap_or("provideCompletion")
        .to_string();

    // Provider contributions come from the host-enabled record of the
    // executing package; caller-supplied manifests are never consulted.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .require_current_package_capability(
            crate::packages::permissions::PackagePermission::CompletionProvider,
        )?;
    if package.contributions.completion_providers.is_empty() {
        return Err(clay_error(
            "completion.invalid_provider: package must declare a completionProviders contribution",
        ));
    }

    let metas = completion_provider_metas(&package);
    let clay = state.borrow::<Arc<ClayOpState>>();
    let registered = clay
        .register_completion_provider_metadata(metas)
        .map_err(|message| clay_error(format!("completion.registration_failed: {message}")))?;
    let registrations = if runtime_bridge {
        registered
            .iter()
            .enumerate()
            .map(|(index, meta)| JsCompletionProviderRegistration {
                package: package.clone(),
                meta: meta.clone(),
                token: super::registration_token(
                    &package.manifest.clay.api_prefix,
                    &meta.id,
                    index,
                ),
                export_name: export_name.clone(),
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    for registration in &registrations {
        clay.register_js_completion_provider(registration.clone());
    }

    serde_json::to_string(&json!({
        "packageName": package.manifest.name,
        "packageVersion": package.manifest.version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "registeredProviderCount": registered.len(),
        "providers": registered.iter().map(|meta| meta.id.clone()).collect::<Vec<_>>(),
        "tokens": registrations.iter().map(|registration| registration.token.clone()).collect::<Vec<_>>(),
        "exportName": export_name,
        "runtimeBridge": runtime_bridge,
    }))
    .map_err(|error| {
        clay_error(format!(
            "completion.registration_failed: failed to serialize result ({error})"
        ))
    })
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

pub(crate) fn completion_provider_metas(package: &PackageRecord) -> Vec<CompletionProviderMeta> {
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
                exclusive: descriptor.exclusive,
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
                    .map(|item| CompletionItem {
                        label: item.label.clone(),
                        insert_text: item.insert_text.clone(),
                        detail: item.detail.clone(),
                        commit_characters: String::new(),
                        text_format: item.text_format,
                        provenance: provenance.clone(),
                    })
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
            "exclusive": meta.exclusive,
            "triggerCharacters": meta.trigger_metadata.trigger_characters,
            "wordBoundaryChars": meta.word_boundary.boundary_chars,
            "items": meta.items.iter().map(|item| json!({
                "label": item.label,
                "insertText": item.insert_text,
                "detail": item.detail,
                "textFormat": completion_item_text_format_name(item.text_format),
            })).collect::<Vec<_>>(),
            "timeoutMs": meta.timeout_ms,
            "maxItems": meta.max_items,
        })).collect::<Vec<_>>(),
    }))
    .map_err(serialize_error("completion.list_failed"))
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
    ] {
        if options.contains_key(key) {
            return Err(clay_error(format!(
                "completion.invalid_provider: executable or raw authority field `{key}` is not accepted by the public registration contract"
            )));
        }
    }
    if optional_u64(options.get("timeoutMs"))?.is_some_and(|value| value == 0 || value > 5_000) {
        return Err(clay_error(
            "completion.invalid_provider: timeoutMs must be between 1 and 5000",
        ));
    }
    Ok(())
}

fn completion_item_text_format_name(format: CompletionItemTextFormat) -> &'static str {
    match format {
        CompletionItemTextFormat::PlainText => "plainText",
        CompletionItemTextFormat::Snippet => "snippet",
    }
}

#[cfg(test)]
mod tests {
    use super::completion_provider_metas;
    use crate::packages::record::assemble_package_record;
    use crate::protocol::CompletionItemTextFormat;
    use crate::server::{
        completion::{
            BufferWordCompletionProvider, CompletionProviderError, CompletionProviderRegistry,
        },
        ops::ClayOpState,
    };

    /// Phase 18.19: a package ships more than one completion provider (e.g.
    /// `@clay/rust` registering a keyword provider and a snippet provider).
    /// Both must register through the same one-line `loadPackage("@clay/rust")`
    /// path with no loader/op change: the load entry submits its package
    /// manifest once, `completion_provider_metas` maps the full contribution
    /// array, and the state layer registers all distinct IDs together while
    /// still rejecting duplicates.
    #[test]
    fn a_package_can_register_multiple_completion_providers_through_one_load() {
        let path = format!("{}/packages/rust/package.json", env!("CARGO_MANIFEST_DIR"));
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let record = assemble_package_record(&value).expect("parse @clay/rust package.json");

        let providers = completion_provider_metas(&record);
        assert_eq!(providers.len(), 2);
        assert!(providers.iter().all(|provider| !provider.exclusive));
        let trigger = providers[0]
            .trigger_metadata
            .trigger_characters
            .first()
            .expect("package provider has a trigger character")
            .clone();
        let duplicate = providers[1].clone();

        let state = ClayOpState::default();
        state
            .register_completion_provider_metadata(providers)
            .expect("register keyword and snippet providers together");

        let listed = state.completion_providers_for_trigger(&trigger);
        assert_eq!(
            listed
                .iter()
                .map(|meta| meta.id.clone())
                .collect::<Vec<_>>(),
            vec![
                format!("{}.keywords", record.manifest.clay.api_prefix),
                format!("{}.snippets", record.manifest.clay.api_prefix),
            ]
        );
        assert!(
            listed[1]
                .items
                .iter()
                .all(|item| item.text_format == CompletionItemTextFormat::Snippet)
        );

        // Duplicate ID is still rejected (no silent overwrite/merge).
        let error = state
            .register_completion_provider_metadata(vec![duplicate])
            .unwrap_err();
        assert!(
            error.contains("already registered"),
            "duplicate provider id must be rejected: {error}"
        );
    }

    #[test]
    fn disabling_provider_id_filters_native_provider_and_bumps_generation() {
        let state = ClayOpState::default();
        let mut native = BufferWordCompletionProvider::meta(0);
        native.trigger_metadata.trigger_characters = vec![".".to_string()];
        let mut peer = native.clone();
        peer.id = "core.peer".to_string();
        state
            .register_completion_provider_metadata(vec![native, peer])
            .unwrap();

        let (disabled, generation) =
            state.disable_completion(BufferWordCompletionProvider::ID.to_string());
        let listed = state.completion_providers_for_trigger(".");

        assert!(disabled);
        assert_eq!(generation, 1);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "core.peer");
        assert_eq!(listed[0].generation, generation);
    }

    #[test]
    fn disabling_package_name_filters_every_provider_from_that_package() {
        let path = format!("{}/packages/rust/package.json", env!("CARGO_MANIFEST_DIR"));
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let record = assemble_package_record(&value).expect("parse @clay/rust package.json");
        let metas = completion_provider_metas(&record);
        assert_eq!(metas.len(), 2);
        let trigger = metas[0].trigger_metadata.trigger_characters[0].clone();
        let state = ClayOpState::default();
        state.register_completion_provider_metadata(metas).unwrap();

        state.disable_completion(record.manifest.name.clone());

        assert!(state.completion_providers_for_trigger(&trigger).is_empty());
        assert!(state.completion_providers().is_empty());
    }

    #[test]
    fn descriptor_rejects_non_boolean_exclusive_claim() {
        let path = format!("{}/packages/rust/package.json", env!("CARGO_MANIFEST_DIR"));
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["clay"]["contributions"]["completionProviders"][0]["exclusive"] =
            serde_json::Value::String("yes".to_string());

        let error = assemble_package_record(&value).unwrap_err();

        assert!(error.message.contains("exclusive must be a boolean"));
    }

    #[test]
    fn descriptor_and_selection_paths_apply_the_same_exclusive_claim() {
        let path = format!("{}/packages/rust/package.json", env!("CARGO_MANIFEST_DIR"));
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        value["clay"]["contributions"]["completionProviders"][0]["exclusive"] =
            serde_json::Value::Bool(true);
        let record = assemble_package_record(&value).expect("parse exclusive provider descriptor");
        let mut exclusive = completion_provider_metas(&record).remove(0);
        assert!(exclusive.exclusive);
        let trigger = exclusive.trigger_metadata.trigger_characters[0].clone();
        exclusive.priority = 10;
        exclusive.id = format!("{}.exclusive", record.manifest.clay.api_prefix);
        let mut peer = exclusive.clone();
        peer.id = format!("{}.peer", record.manifest.clay.api_prefix);
        peer.exclusive = false;
        let mut low = peer.clone();
        low.id = format!("{}.low", record.manifest.clay.api_prefix);
        low.priority = 1;
        let metas = vec![low, peer, exclusive];

        let state = ClayOpState::default();
        state
            .register_completion_provider_metadata(metas.clone())
            .unwrap();
        let state_ids: Vec<_> = state
            .completion_providers_for_trigger(&trigger)
            .into_iter()
            .map(|meta| meta.id)
            .collect();

        let mut coordinator_registry = CompletionProviderRegistry::new();
        for meta in metas {
            coordinator_registry
                .register_package(&record, meta, |_request, _window| async {
                    Err(CompletionProviderError::ProviderFailed(
                        "unused".to_string(),
                    ))
                })
                .unwrap();
        }
        let coordinator_ids: Vec<_> = coordinator_registry
            .providers_for_trigger_character(&trigger)
            .into_iter()
            .map(|meta| meta.id.clone())
            .collect();

        assert_eq!(state_ids, coordinator_ids);
        assert_eq!(state_ids.len(), 2);
    }
}
