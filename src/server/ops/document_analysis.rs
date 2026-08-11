use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::{
    packages::permissions::PackagePermission,
    server::document_analysis::JsDocumentAnalyzerRegistration,
};

use super::{
    ClayOpState,
    decorations::{clay_error, parse_json, required_str},
};

#[op2]
#[string]
pub(super) fn op_clay_language_register_document_analyzer(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&options_json, "language.invalid_analyzer")?;
    let options = value
        .as_object()
        .ok_or_else(|| clay_error("language.invalid_analyzer: options must be an object"))?;
    for key in [
        "handler",
        "callback",
        "function",
        "executable",
        "args",
        "cwd",
        "environment",
        "process",
        "rawOps",
    ] {
        if options.contains_key(key) {
            return Err(clay_error(format!(
                "language.invalid_analyzer: authority field `{key}` is not accepted"
            )));
        }
    }

    let clay_state = state.borrow::<Arc<ClayOpState>>().clone();
    let package =
        clay_state.require_current_package_capability(PackagePermission::ParseDocument)?;
    if !clay_state
        .package_service()
        .lock()
        .expect("package service mutex poisoned")
        .has_approved_capability(&package.manifest.name, PackagePermission::LanguageServer)
    {
        return Err(clay_error(
            "language.invalid_analyzer: language-server permission is required",
        ));
    }
    let analyzer = options
        .get("analyzer")
        .and_then(Value::as_object)
        .unwrap_or(options);
    for key in [
        "handler",
        "callback",
        "function",
        "executable",
        "args",
        "cwd",
        "environment",
        "process",
        "rawOps",
    ] {
        if analyzer.contains_key(key) {
            return Err(clay_error(format!(
                "language.invalid_analyzer: authority field `{key}` is not accepted"
            )));
        }
    }
    let id = required_str(analyzer, "id", "language.invalid_analyzer")?.to_string();
    let api_prefix = package.manifest.clay.api_prefix.as_str();
    if id.starts_with("clay.")
        || !(id == api_prefix
            || id
                .strip_prefix(api_prefix)
                .is_some_and(|suffix| suffix.starts_with('.')))
    {
        return Err(clay_error(format!(
            "language.invalid_analyzer: id `{id}` must use package apiPrefix `{api_prefix}`"
        )));
    }
    let contribution =
        required_str(analyzer, "contribution", "language.invalid_analyzer")?.to_string();
    let Some(descriptor) = package
        .contributions
        .language_servers
        .iter()
        .find(|descriptor| descriptor.id == contribution)
    else {
        return Err(clay_error(
            "language.invalid_analyzer: contribution must name a fixed package language server",
        ));
    };
    let module_specifier =
        required_str(analyzer, "moduleSpecifier", "language.invalid_analyzer")?.to_string();
    let export_name = analyzer
        .get("exportName")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty() && name.len() <= 128)
        .unwrap_or("handleDocumentAnalysis")
        .to_string();
    let modes = match analyzer.get("modes") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) if values.len() <= 32 => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|mode| !mode.is_empty() && mode.len() <= 128)
                    .map(str::to_string)
                    .ok_or_else(|| {
                        clay_error("language.invalid_analyzer: modes must contain bounded strings")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(clay_error(
                "language.invalid_analyzer: modes must be an array with at most 32 entries",
            ));
        }
    };

    let clay = state.borrow::<Arc<ClayOpState>>();
    if !clay
        .load_entry_allowlist()
        .is_package_module(&module_specifier, &package.manifest.name)
    {
        return Err(clay_error(
            "language.invalid_analyzer: moduleSpecifier must resolve to a loaded module owned by the package",
        ));
    }
    let service = clay
        .package_service()
        .lock()
        .expect("package service mutex poisoned");
    let enabled = service.enabled_records().any(|record| record == &package);
    let current_grant = service
        .language_server_grant(&package.manifest.name, &contribution)
        .is_some_and(|grant| {
            grant.descriptor_fingerprint
                == crate::packages::authorization::language_server_descriptor_fingerprint(
                    descriptor,
                )
                && !grant.workspace_root_ids.is_empty()
        });
    drop(service);
    if !enabled || !current_grant {
        return Err(clay_error(
            "language.invalid_analyzer: package must be enabled with a current exact language-server grant before analyzer registration",
        ));
    }
    let registration = JsDocumentAnalyzerRegistration {
        package: package.clone(),
        id: id.clone(),
        contribution: contribution.clone(),
        modes,
        module_specifier,
        export_name: export_name.clone(),
    };
    clay.register_document_analyzer(registration)
        .map_err(|message| clay_error(format!("language.registration_failed: {message}")))?;

    serde_json::to_string(&json!({
        "packageName": package.manifest.name,
        "packageVersion": package.manifest.version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "analyzerId": id,
        "contribution": contribution,
        "exportName": export_name,
        "runtimeBridge": true,
    }))
    .map_err(|error| {
        clay_error(format!(
            "language.registration_failed: failed to serialize analyzer registration ({error})"
        ))
    })
}
