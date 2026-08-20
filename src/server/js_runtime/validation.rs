// Auto-extracted from js_runtime.rs (Plan 090 task 3). Private submodule: validation family.

use crate::protocol::{
    DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DiagnosticSet,
    DiagnosticSeverity, DiagnosticSpan, DocumentFontRole, IncrementalParseUpdate, Modifiers,
    ParseByteRange, ParseEditNotification, TokenType,
};

use super::error::ClayRuntimeError;

pub(super) fn document_analysis_event_json(
    registration: &crate::server::document_analysis::JsDocumentAnalyzerRegistration,
    event: &crate::server::document_analysis::DocumentAnalysisEvent,
) -> String {
    use crate::server::document_analysis::DocumentAnalysisEvent;
    let identity = serde_json::json!({
        "package": registration.package.manifest.name,
        "packageVersion": registration.package.manifest.version,
        "packagePrefix": registration.package.manifest.clay.api_prefix,
        "analyzerId": registration.id,
        "contribution": registration.contribution,
    });
    let value = match event {
        DocumentAnalysisEvent::Open {
            document_id,
            document_version,
            runtime_generation,
            active_mode,
            workspace_root_id,
            canonical_root_path,
            relative_path,
            text,
        } => serde_json::json!({
            "kind": "open",
            "identity": identity,
            "documentId": document_id,
            "documentVersion": document_version,
            "runtimeGeneration": runtime_generation,
            "activeMode": active_mode,
            "workspaceRootId": workspace_root_id,
            "canonicalRootPath": canonical_root_path,
            "relativePath": relative_path,
            "text": text,
        }),
        DocumentAnalysisEvent::Change {
            document_id,
            base_version,
            document_version,
            byte_start,
            byte_end,
            inserted_text,
        } => serde_json::json!({
            "kind": "change",
            "identity": identity,
            "documentId": document_id,
            "baseVersion": base_version,
            "documentVersion": document_version,
            "byteStart": byte_start,
            "byteEnd": byte_end,
            "insertedText": inserted_text,
        }),
        DocumentAnalysisEvent::Reset {
            document_id,
            document_version,
            text,
        } => serde_json::json!({
            "kind": "reset",
            "identity": identity,
            "documentId": document_id,
            "documentVersion": document_version,
            "text": text,
        }),
        DocumentAnalysisEvent::Close {
            document_id,
            document_version,
        } => serde_json::json!({
            "kind": "close",
            "identity": identity,
            "documentId": document_id,
            "documentVersion": document_version,
        }),
        DocumentAnalysisEvent::Completion { request, window } => serde_json::json!({
            "kind": "completion",
            "identity": identity,
            "request": serde_json::from_str::<serde_json::Value>(&completion_request_json(request)).unwrap_or(serde_json::Value::Null),
            "window": serde_json::from_str::<serde_json::Value>(&completion_window_json(window)).unwrap_or(serde_json::Value::Null),
        }),
        DocumentAnalysisEvent::LanguageIntelligence { request, window } => serde_json::json!({
            "kind": "languageIntelligence",
            "identity": identity,
            "request": serde_json::from_str::<serde_json::Value>(&language_intelligence_request_json(request)).unwrap_or(serde_json::Value::Null),
            "window": serde_json::from_str::<serde_json::Value>(&language_intelligence_window_json(window)).unwrap_or(serde_json::Value::Null),
        }),
        DocumentAnalysisEvent::Shutdown => serde_json::json!({
            "kind": "shutdown",
            "identity": identity,
        }),
    };
    value.to_string()
}

#[allow(
    clippy::too_many_arguments,
    reason = "completion JS bridge needs runtime, registration, request, window, timeout, and heap state together"
)]
pub(super) fn completion_request_json(request: &crate::protocol::CompletionRequest) -> String {
    let trigger = match &request.trigger {
        crate::protocol::CompletionTrigger::Manual => serde_json::json!({ "kind": "manual" }),
        crate::protocol::CompletionTrigger::Character(character) => {
            serde_json::json!({ "kind": "character", "character": character })
        }
    };
    serde_json::json!({
        "requestId": request.request_id,
        "clientId": request.client_id,
        "documentId": request.document_id,
        "documentVersion": request.document_version,
        "behaviorVersion": request.behavior_version,
        "cursorByteOffset": request.cursor_byte_offset,
        "replacementRange": {
            "byteStart": request.replacement_range.byte_start,
            "byteEnd": request.replacement_range.byte_end,
        },
        "trigger": trigger,
        "providerGeneration": request.provider_generation,
    })
    .to_string()
}

pub(super) fn completion_window_json(
    window: &crate::server::completion::CompletionDocumentWindow,
) -> String {
    serde_json::json!({
        "documentId": window.document_id,
        "documentVersion": window.document_version,
        "behaviorVersion": window.behavior_version,
        "packagePrefix": window.package_prefix,
        "byteStart": window.byte_start,
        "byteEnd": window.byte_end,
        "text": window.text,
    })
    .to_string()
}

pub(super) fn completion_result_from_json(
    result_json: &str,
    package: &crate::packages::record::PackageRecord,
    request: &crate::protocol::CompletionRequest,
) -> Result<crate::protocol::CompletionResultSet, ClayRuntimeError> {
    use crate::protocol::{
        CompletionItem, CompletionItemTextFormat, CompletionProvenance, CompletionResultSet,
        CompletionStatus,
    };

    let value: serde_json::Value = serde_json::from_str(result_json).map_err(|error| {
        ClayRuntimeError::Runtime(format!("completion.invalid_result: {error}"))
    })?;
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("completion.invalid_result: result must be an object".to_string())
    })?;
    let status = match object
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ok")
    {
        "ok" | "Ok" => CompletionStatus::Ok,
        "empty" | "Empty" => CompletionStatus::Empty,
        "timeout" | "Timeout" => CompletionStatus::Timeout,
        "providerError" | "ProviderError" | "error" => CompletionStatus::ProviderError,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "completion.invalid_result: unsupported status `{other}`"
            )));
        }
    };
    let provenance = CompletionProvenance {
        package_name: package.manifest.name.clone(),
        package_version: package.manifest.version.clone(),
        package_prefix: package.manifest.clay.api_prefix.clone(),
    };
    let items = object
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let item = item.as_object().ok_or_else(|| {
                        ClayRuntimeError::Runtime(
                            "completion.invalid_result: item must be an object".to_string(),
                        )
                    })?;
                    let label = item
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            ClayRuntimeError::Runtime(
                                "completion.invalid_result: item label is required".to_string(),
                            )
                        })?
                        .to_string();
                    Ok(CompletionItem {
                        insert_text: item
                            .get("insertText")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(&label)
                            .to_string(),
                        label,
                        detail: item
                            .get("detail")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        commit_characters: item
                            .get("commitCharacters")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        text_format: match item
                            .get("textFormat")
                            .and_then(serde_json::Value::as_str)
                        {
                            Some("snippet" | "Snippet") => CompletionItemTextFormat::Snippet,
                            _ => CompletionItemTextFormat::PlainText,
                        },
                        provenance: provenance.clone(),
                    })
                })
                .collect::<Result<Vec<_>, ClayRuntimeError>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(CompletionResultSet {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        replacement_range: request.replacement_range,
        status: if items.is_empty() && status == CompletionStatus::Ok {
            CompletionStatus::Empty
        } else {
            status
        },
        items,
        provenance,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "language-intelligence JS bridge mirrors the parse-handler worker path and needs request+window inputs together"
)]
pub(super) fn language_intelligence_request_json(
    request: &crate::protocol::LanguageIntelligenceRequest,
) -> String {
    serde_json::json!({
        "requestId": request.request_id,
        "clientId": request.client_id,
        "documentId": request.document_id,
        "documentVersion": request.document_version,
        "behaviorVersion": request.behavior_version,
        "cursorByteOffset": request.cursor_byte_offset,
        "feature": language_intelligence_feature_name(request.feature),
        "providerGeneration": request.provider_generation,
    })
    .to_string()
}

pub(super) fn language_intelligence_window_json(
    window: &crate::server::language_intelligence::LanguageIntelligenceDocumentWindow,
) -> String {
    serde_json::json!({
        "documentId": window.document_id,
        "documentVersion": window.document_version,
        "behaviorVersion": window.behavior_version,
        "byteStart": window.byte_start,
        "byteEnd": window.byte_end,
        "text": window.text,
        "activeMode": window.active_mode,
    })
    .to_string()
}

pub(super) fn language_intelligence_feature_name(
    feature: crate::protocol::LanguageIntelligenceFeature,
) -> &'static str {
    match feature {
        crate::protocol::LanguageIntelligenceFeature::Hover => "hover",
        crate::protocol::LanguageIntelligenceFeature::GoToDefinition => "definition",
        crate::protocol::LanguageIntelligenceFeature::CodeAction => "codeAction",
        crate::protocol::LanguageIntelligenceFeature::SignatureHelp => "signatureHelp",
    }
}

pub(super) fn language_intelligence_result_from_json(
    result_json: &str,
    package: &crate::packages::record::PackageRecord,
    request: &crate::protocol::LanguageIntelligenceRequest,
) -> Result<crate::protocol::LanguageIntelligenceResult, ClayRuntimeError> {
    use crate::protocol::{
        CodeActionResult, GoToDefinitionResult, HoverResult, LanguageIntelligencePayload,
        LanguageIntelligenceResult, LanguageIntelligenceStatus, SignatureHelpResult,
    };

    let value: serde_json::Value = serde_json::from_str(result_json)
        .map_err(|error| ClayRuntimeError::Runtime(format!("language.invalid_result: {error}")))?;
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("language.invalid_result: result must be an object".to_string())
    })?;
    let status = match object
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ok")
    {
        "ok" | "Ok" => LanguageIntelligenceStatus::Ok,
        "empty" | "Empty" => LanguageIntelligenceStatus::Empty,
        "timeout" | "Timeout" => LanguageIntelligenceStatus::Timeout,
        "providerError" | "ProviderError" | "error" => LanguageIntelligenceStatus::ProviderError,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "language.invalid_result: unsupported status `{other}`"
            )));
        }
    };

    let payload = match request.feature {
        crate::protocol::LanguageIntelligenceFeature::Hover => {
            let hover = object
                .get("payload")
                .and_then(|value| value.get("hover"))
                .or_else(|| object.get("hover"))
                .unwrap_or(&value);
            let hover_object = hover.as_object().unwrap_or(object);
            LanguageIntelligencePayload::Hover(HoverResult {
                range: hover_object
                    .get("range")
                    .and_then(language_intelligence_range_from_value),
                markdown: hover_object
                    .get("markdown")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        }
        crate::protocol::LanguageIntelligenceFeature::GoToDefinition => {
            let definition = object
                .get("payload")
                .and_then(|value| {
                    value
                        .get("definition")
                        .or_else(|| value.get("goToDefinition"))
                })
                .or_else(|| {
                    object
                        .get("definition")
                        .or_else(|| object.get("goToDefinition"))
                })
                .unwrap_or(&value);
            let locations = definition
                .get("locations")
                .or_else(|| object.get("locations"))
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(language_intelligence_location_from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult { locations })
        }
        crate::protocol::LanguageIntelligenceFeature::CodeAction => {
            let actions_value = object
                .get("payload")
                .and_then(|value| value.get("codeAction").or_else(|| value.get("actions")))
                .or_else(|| object.get("codeAction").or_else(|| object.get("actions")))
                .unwrap_or(&value);
            let actions = actions_value
                .get("actions")
                .or(Some(actions_value))
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(|value| language_intelligence_code_action_from_value(value, request))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            LanguageIntelligencePayload::CodeAction(CodeActionResult { actions })
        }
        crate::protocol::LanguageIntelligenceFeature::SignatureHelp => {
            let help = object
                .get("payload")
                .and_then(|value| value.get("signatureHelp"))
                .or_else(|| object.get("signatureHelp"))
                .unwrap_or(&value);
            let help_object = help.as_object().unwrap_or(object);
            let signatures = help_object
                .get("signatures")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(language_intelligence_signature_from_value)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            LanguageIntelligencePayload::SignatureHelp(SignatureHelpResult {
                signatures,
                active_signature: help_object
                    .get("activeSignature")
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as u16),
                active_parameter: help_object
                    .get("activeParameter")
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as u16),
            })
        }
    };

    Ok(LanguageIntelligenceResult {
        request_id: request.request_id,
        client_id: request.client_id,
        document_id: request.document_id,
        document_version: request.document_version,
        behavior_version: request.behavior_version,
        provider_generation: request.provider_generation,
        feature: request.feature,
        status,
        payload,
        provenance: crate::protocol::CompletionProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
    })
}

pub(super) fn language_intelligence_range_from_value(
    value: &serde_json::Value,
) -> Option<crate::protocol::TextByteRange> {
    let object = value.as_object()?;
    let byte_start = object
        .get("byteStart")
        .and_then(serde_json::Value::as_u64)?;
    let byte_end = object.get("byteEnd").and_then(serde_json::Value::as_u64)?;
    Some(crate::protocol::TextByteRange::new(byte_start, byte_end))
}

pub(super) fn language_intelligence_location_from_value(
    value: &serde_json::Value,
) -> Result<crate::protocol::TextLocation, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("language.invalid_result: location must be an object".to_string())
    })?;
    let range = object
        .get("range")
        .and_then(language_intelligence_range_from_value)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: location.range requires byteStart/byteEnd".to_string(),
            )
        })?;
    if let Some(document_id) = object.get("documentId").and_then(serde_json::Value::as_u64) {
        return Ok(crate::protocol::TextLocation::OpenDocument { document_id, range });
    }
    let workspace_root_id = object
        .get("workspaceRootId")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: location requires documentId or workspaceRootId"
                    .to_string(),
            )
        })?;
    let relative_path = object
        .get("relativePath")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "language.invalid_result: workspace location requires relativePath".to_string(),
            )
        })?
        .to_string();
    Ok(crate::protocol::TextLocation::WorkspaceFile {
        workspace_root_id,
        relative_path,
        range,
    })
}

pub(super) fn language_intelligence_code_action_from_value(
    value: &serde_json::Value,
    request: &crate::protocol::LanguageIntelligenceRequest,
) -> Result<crate::protocol::CodeAction, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "language.invalid_result: code action must be an object".to_string(),
        )
    })?;
    let range = object
        .get("range")
        .and_then(language_intelligence_range_from_value)
        .unwrap_or_else(|| {
            crate::protocol::TextByteRange::new(
                request.cursor_byte_offset,
                request.cursor_byte_offset,
            )
        });
    let title = object
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let command_id = object
        .get("commandId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let edit = object.get("edit").and_then(|edit_value| {
        let edit_object = edit_value.as_object()?;
        let edits = edit_object
            .get("edits")
            .and_then(serde_json::Value::as_array)?
            .iter()
            .filter_map(|entry| {
                let entry_object = entry.as_object()?;
                Some(crate::protocol::RangeEdit {
                    range: entry_object
                        .get("range")
                        .and_then(language_intelligence_range_from_value)?,
                    replacement: entry_object
                        .get("replacement")
                        .and_then(serde_json::Value::as_str)?
                        .to_string(),
                })
            })
            .collect::<Vec<_>>();
        Some(crate::protocol::EditPreview {
            document_id: edit_object
                .get("documentId")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(request.document_id),
            document_version: edit_object
                .get("documentVersion")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(request.document_version),
            edits,
        })
    });
    Ok(crate::protocol::CodeAction {
        range,
        title,
        command_id,
        edit,
    })
}

pub(super) fn language_intelligence_signature_from_value(
    value: &serde_json::Value,
) -> Result<crate::protocol::SignatureInformation, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "language.invalid_result: signature must be an object".to_string(),
        )
    })?;
    let parameters = object
        .get("parameters")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|parameter| {
                    let parameter_object = parameter.as_object()?;
                    Some(crate::protocol::ParameterInformation {
                        label: parameter_object
                            .get("label")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        documentation: parameter_object
                            .get("documentation")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(crate::protocol::SignatureInformation {
        label: object
            .get("label")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        documentation: object
            .get("documentation")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        parameters,
    })
}

pub(super) fn parse_notification_json(notification: &ParseEditNotification) -> String {
    serde_json::json!({
        "documentId": notification.document_id,
        "documentVersion": notification.document_version,
        "behaviorVersion": notification.behavior_version,
        "packagePrefix": notification.package_prefix,
        "mode": notification.mode_id,
        "viewport": range_json(notification.viewport),
        "invalidatedRanges": notification.invalidated_ranges.iter().map(|range| range_json(*range)).collect::<Vec<_>>(),
        "acceptedEdit": notification.accepted_edit.map(|edit| serde_json::json!({
            "baseDocumentVersion": edit.base_document_version,
            "documentVersion": edit.document_version,
            "startByte": edit.start_byte,
            "oldEndByte": edit.old_end_byte,
            "newEndByte": edit.new_end_byte,
            "startPosition": { "row": edit.start_position.row, "column": edit.start_position.column },
            "oldEndPosition": { "row": edit.old_end_position.row, "column": edit.old_end_position.column },
            "newEndPosition": { "row": edit.new_end_position.row, "column": edit.new_end_position.column },
        })),
        "parseWindows": notification.parse_windows.iter().map(|window| serde_json::json!({
            "documentId": window.document_id,
            "documentVersion": window.document_version,
            "packagePrefix": window.package_prefix,
            "mode": window.mode_id,
            "windowId": window.window_id,
            "byteStart": window.byte_start,
            "byteEnd": window.byte_end,
            "baseLine": window.base_line,
            "baseColumn": window.base_column,
            "incrementalEdit": window.incremental_edit,
            "text": window.text,
        })).collect::<Vec<_>>(),
        "memoryBudget": notification.memory_budget.map(|budget| serde_json::json!({
            "budgetBytes": budget.budget_bytes,
            "retainedBytes": budget.retained_bytes,
        })),
    })
    .to_string()
}

pub(super) fn range_json(range: ParseByteRange) -> serde_json::Value {
    serde_json::json!({ "byteStart": range.start, "byteEnd": range.end })
}

pub(super) fn parse_update_json(
    update_json: &str,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    fallback: ParseEditNotification,
) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
    let value: serde_json::Value = serde_json::from_str(update_json)
        .map_err(|error| ClayRuntimeError::Runtime(format!("parse.invalid_update: {error}")))?;
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: update must be an object".to_string())
    })?;
    let viewport = object
        .get("viewport")
        .and_then(parse_range_value)
        .unwrap_or(fallback.viewport);
    let spans: Option<Vec<DecorationSpan>> = object
        .get("spans")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| span_from_value(value, registration))
                .collect()
        })
        .transpose()?;
    let diagnostics = object
        .get("diagnostics")
        .map(|value| diagnostic_set_from_value(value, registration, &fallback, viewport))
        .transpose()?;
    Ok(IncrementalParseUpdate {
        document_id: object
            .get("documentId")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.document_id),
        document_version: object
            .get("documentVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.document_version),
        behavior_version: object
            .get("behaviorVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.behavior_version),
        package_prefix: object
            .get("packagePrefix")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&registration.meta.package_prefix)
            .to_string(),
        mode_id: object
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&registration.meta.mode_id)
            .to_string(),
        parse_unit: registration.parse_unit,
        viewport,
        invalidated_ranges: fallback.invalidated_ranges,
        syntax_tree_delta: object
            .get("syntaxTreeDelta")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        decoration_updates: spans
            .map(|spans| DecorationSet {
                document_id: fallback.document_id,
                document_version: fallback.document_version,
                package_prefix: registration.meta.package_prefix.clone(),
                kind: spans
                    .first()
                    .map_or(DecorationKind::Syntax, |span| span.kind),
                viewport_byte_start: viewport.start,
                viewport_byte_end: viewport.end,
                spans,
            })
            .into_iter()
            .collect(),
        diagnostic_update: diagnostics,
        folding_update: None,
    })
}

pub(super) fn diagnostic_set_from_value(
    value: &serde_json::Value,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    fallback: &ParseEditNotification,
    viewport: ParseByteRange,
) -> Result<DiagnosticSet, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: diagnostics must be an object".to_string())
    })?;
    let source = required_string(object, "source", "diagnostics")?;
    let provenance = DecorationProvenance {
        package_name: registration.package.manifest.name.clone(),
        package_version: registration.package.manifest.version.clone(),
        package_prefix: registration.package.manifest.clay.api_prefix.clone(),
    };
    let spans = object
        .get("spans")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(
                "parse.invalid_update: diagnostics.spans must be an array".to_string(),
            )
        })?
        .iter()
        .map(|value| diagnostic_span_from_value(value, source, &provenance))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DiagnosticSet {
        document_id: fallback.document_id,
        document_version: fallback.document_version,
        viewport_byte_start: viewport.start,
        viewport_byte_end: viewport.end,
        source: source.to_string(),
        provenance,
        spans,
    })
}

pub(super) fn diagnostic_span_from_value(
    value: &serde_json::Value,
    source: &str,
    provenance: &DecorationProvenance,
) -> Result<DiagnosticSpan, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "parse.invalid_update: diagnostic span must be an object".to_string(),
        )
    })?;
    let severity = match required_string(object, "severity", "diagnostic span")? {
        "error" => DiagnosticSeverity::Error,
        "warning" => DiagnosticSeverity::Warning,
        "info" => DiagnosticSeverity::Info,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: unsupported diagnostic severity `{other}`"
            )));
        }
    };
    Ok(DiagnosticSpan {
        byte_start: required_u64(object, "byteStart", "diagnostic span")?,
        byte_end: required_u64(object, "byteEnd", "diagnostic span")?,
        severity,
        code: required_string(object, "code", "diagnostic span")?.to_string(),
        message: required_string(object, "message", "diagnostic span")?.to_string(),
        source: source.to_string(),
        provenance: provenance.clone(),
    })
}

pub(super) fn required_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, ClayRuntimeError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: {context}.{field} must be a string"
            ))
        })
}

pub(super) fn required_u64(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    context: &str,
) -> Result<u64, ClayRuntimeError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: {context}.{field} must be an unsigned integer"
            ))
        })
}

pub(super) fn parse_range_value(value: &serde_json::Value) -> Option<ParseByteRange> {
    let object = value.as_object()?;
    Some(ParseByteRange::new(
        object
            .get("byteStart")
            .or_else(|| object.get("start"))?
            .as_u64()?,
        object
            .get("byteEnd")
            .or_else(|| object.get("end"))?
            .as_u64()?,
    ))
}

pub(super) fn span_from_value(
    value: &serde_json::Value,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
) -> Result<DecorationSpan, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: span must be an object".to_string())
    })?;
    let kind = match object.get("kind").and_then(serde_json::Value::as_str) {
        Some(name) => DecorationKind::from_name(name).ok_or_else(|| {
            ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: unsupported decoration kind `{name}`"
            ))
        })?,
        None => DecorationKind::Syntax,
    };
    let provenance = DecorationProvenance {
        package_name: registration.package.manifest.name.clone(),
        package_version: registration.package.manifest.version.clone(),
        package_prefix: registration.package.manifest.clay.api_prefix.clone(),
    };
    let byte_start = object
        .get("byteStart")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let byte_end = object
        .get("byteEnd")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let priority = object
        .get("priority")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u16;
    let mut span = if let Some(token_type_name) = object
        .get("tokenType")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let token_type = TokenType::from_name(token_type_name).ok_or_else(|| {
            ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: unknown tokenType `{token_type_name}`"
            ))
        })?;
        let modifiers = object
            .get("modifiers")
            .and_then(serde_json::Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let modifiers = Modifiers::from_names(&modifiers).unwrap_or(Modifiers::NONE);
        DecorationSpan::from_vocabulary(
            byte_start, byte_end, kind, token_type, modifiers, priority, provenance,
        )
    } else {
        let style_token = object
            .get("styleToken")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("markup.plain");
        DecorationSpan::from_style_token(
            byte_start,
            byte_end,
            kind,
            style_token,
            priority,
            provenance,
        )
    };
    if let Some(role) = object.get("fontRole").and_then(serde_json::Value::as_str) {
        span.font_role = DocumentFontRole::from_name(role);
    }
    span.target = parse_decoration_target(object.get("target"))?;
    span.inlay = parse_inlay(object.get("inlay"))?;
    Ok(span)
}

fn parse_inlay(
    value: Option<&serde_json::Value>,
) -> Result<Option<crate::protocol::InlayHintPayload>, ClayRuntimeError> {
    use crate::protocol::InlayHintPayload;
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return Ok(None);
    };
    let label = object
        .get("label")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime("parse.invalid_update: inlay.label must be a string".into())
        })?
        .to_string();
    let placement = object
        .get("placement")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("after");
    InlayHintPayload::from_name(placement, label)
        .map(Some)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime("parse.invalid_update: inlay payload rejected".into())
        })
}

fn parse_decoration_target(
    value: Option<&serde_json::Value>,
) -> Result<Option<crate::protocol::DecorationTarget>, ClayRuntimeError> {
    use crate::protocol::{DecorationTarget, TextByteRange};
    let Some(object) = value.and_then(serde_json::Value::as_object) else {
        return Ok(None);
    };
    let kind = object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            ClayRuntimeError::Runtime("parse.invalid_update: target.kind must be a string".into())
        })?;
    let range = match (
        object.get("byteStart").and_then(serde_json::Value::as_u64),
        object.get("byteEnd").and_then(serde_json::Value::as_u64),
    ) {
        (Some(byte_start), Some(byte_end)) => Some(TextByteRange::new(byte_start, byte_end)),
        (None, None) => None,
        _ => {
            return Err(ClayRuntimeError::Runtime(
                "parse.invalid_update: target range requires byteStart and byteEnd".into(),
            ));
        }
    };
    let target = match kind {
        "workspacePath" | "WorkspacePath" => DecorationTarget::WorkspacePath {
            relative_path: object
                .get("relativePath")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    ClayRuntimeError::Runtime(
                        "parse.invalid_update: target.relativePath must be a string".into(),
                    )
                })?
                .to_string(),
            range,
        },
        "documentRange" | "DocumentRange" => DecorationTarget::DocumentRange {
            range: range.ok_or_else(|| {
                ClayRuntimeError::Runtime(
                    "parse.invalid_update: documentRange requires byteStart and byteEnd".into(),
                )
            })?,
        },
        "displayOnly" | "DisplayOnly" => DecorationTarget::DisplayOnly {
            text: object
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        },
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "parse.invalid_update: unknown target kind `{other}`"
            )));
        }
    };
    target.sanitized().map(Some).ok_or_else(|| {
        ClayRuntimeError::Runtime("parse.invalid_update: target payload rejected".into())
    })
}
