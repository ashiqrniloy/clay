// Auto-extracted from record.rs (Plan 090 task 4). Private submodule: documentation family.
use super::*;

use serde_json::Value;

use crate::packages::permissions::PackagePermission;
use crate::perf::budgets::BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES;

pub(super) fn parse_docs_metadata(
    value: Option<&Value>,
    ctx: &ErrorContext,
) -> Result<PackageDocsMetadata, PackageRecordError> {
    match value {
        Some(Value::String(path)) if !path.trim().is_empty() => Ok(PackageDocsMetadata {
            docs_path: path.clone(),
        }),
        _ => Err(ctx.error(
            PackageRecordRule::MissingRequiredField,
            None,
            "clay.docs must be a non-empty path to the package documentation index",
        )),
    }
}

pub(super) fn parse_performance_metadata(
    value: Option<&Value>,
    raw_manifest: &Value,
    ctx: &ErrorContext,
) -> Result<PackagePerformanceMetadata, PackageRecordError> {
    let estimated_manifest_bytes = match value {
        Some(Value::Object(perf)) => {
            match perf.get("estimatedManifestBytes") {
                Some(Value::Number(n)) => n
                    .as_u64()
                    .map(|v| v as usize)
                    .ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::MissingRequiredField,
                            None,
                            "clay.performance.estimatedManifestBytes must be a non-negative integer",
                        )
                    })?,
                _ => return Err(ctx.error(
                    PackageRecordRule::MissingRequiredField,
                    None,
                    "clay.performance.estimatedManifestBytes must be declared as a non-negative integer",
                )),
            }
        }
        // When no performance block is present, derive the estimate from the raw payload size.
        None => {
            serde_json::to_vec(raw_manifest)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX)
        }
        _ => return Err(ctx.error(
            PackageRecordRule::MissingRequiredField,
            None,
            "clay.performance must be an object when present",
        )),
    };

    if estimated_manifest_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
        return Err(ctx.error(
            PackageRecordRule::PayloadBudgetExceeded,
            None,
            format!(
                "package estimated manifest payload ({estimated_manifest_bytes} bytes) exceeds \
                 BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
            ),
        ));
    }

    Ok(PackagePerformanceMetadata {
        estimated_manifest_bytes,
    })
}

pub(super) fn parse_api_dependencies(
    value: Option<&Value>,
    ctx: &ErrorContext,
) -> Result<Vec<PackageApiDependency>, PackageRecordError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidApiDependency,
            None,
            "clay.apiDependencies must be an array of Clay JS API ID strings when present",
        ));
    };
    let mut deps = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(api_id) = entry.as_str() else {
            return Err(ctx.error(
                PackageRecordRule::InvalidApiDependency,
                None,
                "clay.apiDependencies entries must be strings",
            ));
        };
        if api_id.trim().is_empty() {
            return Err(ctx.error(
                PackageRecordRule::InvalidApiDependency,
                None,
                "clay.apiDependencies entries must be non-empty strings",
            ));
        }
        deps.push(PackageApiDependency {
            api_id: api_id.to_string(),
        });
    }
    Ok(deps)
}

pub(super) fn validate_api_dependency_permissions(
    dependencies: &[PackageApiDependency],
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    for dependency in dependencies {
        let required = match dependency.api_id.as_str() {
            "packages.serverLoadPackage" => None,
            "behavior.buildCodeEditingManifest" => None,
            "modes.serverRegisterModePattern" => Some(PackagePermission::ModeRegistration),
            "modes.serverActivateMajorMode" => Some(PackagePermission::ModeActivation),
            "commands.serverRegisterCommand" => Some(PackagePermission::CommandRegistration),
            "completion.serverRegisterCompletionProvider" => {
                Some(PackagePermission::CompletionProvider)
            }
            "completion.completionTriggerCharactersFromEditorRules" => None,
            "parse.serverRegisterParseHandler" | "language.serverRegisterDocumentAnalyzer" => {
                Some(PackagePermission::ParseDocument)
            }
            "language-server.startLanguageServerSession" => Some(PackagePermission::LanguageServer),
            "decorations.serverPublishDecorations" | "diagnostics.serverPublishDiagnostics" => {
                Some(PackagePermission::RenderDecorations)
            }
            "syntax.serverRegisterSyntaxGrammar" => Some(PackagePermission::ParseDocument),
            "ui.serverRegisterPanelContribution"
            | "ui.serverRegisterComponentContribution"
            | "ui.serverRegisterTransientOverlayContribution"
            | "ui.serverRegisterThemeToken"
            | "ui.serverRegisterInputContribution"
            | "ui.serverRegisterUiStateScope"
            | "ui.serverRegisterPaneContentContribution" => None,
            "ui.serverSetLayoutOverride" | "configuration.setPackageOption" => {
                Some(PackagePermission::PackageConfiguration)
            }
            // Read-only Git discovery and inert SDUI publication are
            // server-owned: they require no package permission because Git
            // authority never reaches package code.
            "git.serverListGitStatuses" | "git.serverRefreshGitStatus" | "sdui.publishTree" => None,
            _ => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidApiDependency,
                    Some(&dependency.api_id),
                    format!(
                        "unknown Clay JS API dependency `{}`; packages must list documented Clay API IDs",
                        dependency.api_id
                    ),
                ));
            }
        };

        if let Some(required) = required
            && !permissions.contains(&required)
        {
            return Err(ctx.error(
                    PackageRecordRule::UndeclaredPermissionForContribution,
                    Some(&dependency.api_id),
                    format!(
                        "API dependency `{}` requires the `{}` permission to be declared in clay.permissions",
                        dependency.api_id,
                        required.as_str()
                    ),
                ));
        }
    }

    Ok(())
}
