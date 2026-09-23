// Auto-extracted from record.rs (Plan 090 task 4). Private submodule: language family.
use super::*;

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::packages::permissions::PackagePermission;
use crate::perf::budgets::{
    BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS,
    COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS, COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS,
    COMPLETION_RESULT_MAX_ITEMS, LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS,
    LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS,
};

use crate::protocol::{
    CompletionItemTextFormat, DocumentFontRole, LanguageIntelligenceFeature, Modifiers, TokenType,
};

pub(super) fn parse_syntax_grammar_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<SyntaxGrammarContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.syntaxGrammars", ctx)?;
    if !entries.is_empty()
        && (!permissions.contains(&PackagePermission::ParseDocument)
            || !permissions.contains(&PackagePermission::RenderDecorations))
    {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "syntax grammar contributions require `parse-document` and `render-decorations` permissions",
        ));
    }
    if !entries.is_empty()
        && !ctx
            .package_name
            .as_deref()
            .is_some_and(|package_name| package_name.starts_with("@clay/"))
    {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "Phase 18.10 syntax grammar contributions are first-party-only; arbitrary third-party grammar packages are not accepted",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut seen_languages = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let estimated_payload_bytes = contribution_payload_size(entry);
        if estimated_payload_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "syntax grammar metadata payload ({estimated_payload_bytes} bytes) exceeds BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_syntax_grammar_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "syntax grammar contribution", ctx)?;
        let language_id = required_str_field(obj, "languageId", ctx)?;
        if !is_valid_language_id(language_id) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(language_id),
                "syntax grammar languageId must use lowercase letters, digits, hyphen, underscore, plus, or dot",
            ));
        }
        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("{api_prefix}.{language_id}"));
        if id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(&id),
                "syntax grammar IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(&id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "syntax grammar IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(id.clone()) || !seen_languages.insert(language_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                "syntax grammar IDs and languageIds must be unique within a package",
            ));
        }

        let patterns = obj
            .get("filePatterns")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "syntax grammar contribution must include filePatterns object",
                )
            })?;
        let extensions =
            optional_string_vec(patterns.get("extensions"), "filePatterns.extensions", ctx)?;
        for extension in &extensions {
            if extension.starts_with('.')
                || extension.contains('/')
                || extension.contains('\\')
                || extension.trim().is_empty()
            {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(extension),
                    "syntax grammar extensions must be bare extension names without path separators or leading dots",
                ));
            }
        }
        let file_names =
            optional_string_vec(patterns.get("fileNames"), "filePatterns.fileNames", ctx)?;
        if extensions.is_empty() && file_names.is_empty() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "syntax grammar filePatterns must declare extensions or fileNames",
            ));
        }

        let grammar = obj
            .get("grammar")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "syntax grammar contribution must include grammar object",
                )
            })?;
        let grammar_kind = required_str_field(grammar, "kind", ctx)?;
        let (grammar_path, grammar_source) = if grammar_kind == "native" {
            // Tier 1: the grammar is compiled into the server binary; packages
            // declare only the native crate identifier. No wasm asset path is
            // accepted, and the first-party registry is the sole source of
            // truth for which native crates exist.
            if grammar.get("path").is_some() {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "native (Tier 1) syntax grammars are compiled into the server; a `path` is not accepted",
                ));
            }
            let source = required_str_field(grammar, "source", ctx)?;
            (String::new(), source.to_string())
        } else if grammar_kind == "tree-sitter-wasm" {
            let grammar_path = required_str_field(grammar, "path", ctx)?;
            validate_package_asset_path(grammar_path, "grammar.path", Some(".wasm"), ctx)?;
            let source = grammar
                .get("source")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned);
            (grammar_path.to_string(), source.unwrap_or_default())
        } else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(grammar_kind),
                "Phase 18.18 syntax grammars support kind `native` (Tier 1 compiled-in) or `tree-sitter-wasm` (Tier 2 host-side); other kinds are not accepted",
            ));
        };
        let grammar_source = if grammar_source.is_empty() {
            None
        } else {
            Some(grammar_source)
        };

        let queries = obj
            .get("queries")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "syntax grammar contribution must include queries object",
                )
            })?;
        let highlights_query_path = required_str_field(queries, "highlights", ctx)?;
        validate_package_asset_path(
            highlights_query_path,
            "queries.highlights",
            Some(".scm"),
            ctx,
        )?;
        let locals_query_path = optional_asset_path(queries.get("locals"), "queries.locals", ctx)?;
        let injections_query_path =
            optional_asset_path(queries.get("injections"), "queries.injections", ctx)?;
        // Deny-by-default: only the known query roles are accepted. Plan 071
        // task 15 keeps text-object queries out of package-declared grammar
        // metadata entirely — they are first-party native-descriptor
        // contributions compiled into the binary, so an unknown key such as
        // `textobjects` must fail validation instead of being silently dropped.
        for key in queries.keys() {
            if !matches!(key.as_str(), "highlights" | "locals" | "injections") {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    format!(
                        "unknown syntax grammar query kind `queries.{key}`; allowed keys are highlights, locals, injections"
                    ),
                ));
            }
        }
        let style_map = parse_syntax_style_map(obj.get("styleMap"), &id, ctx)?;

        let budgets = obj.get("budgets").and_then(Value::as_object);
        let timeout_ms = optional_u64_budget(budgets, "timeoutMs", &id, ctx)?;
        let max_window_bytes = optional_usize_budget(budgets, "maxWindowBytes", &id, ctx)?;

        descriptors.push(SyntaxGrammarContributionDescriptor {
            id,
            language_id: language_id.to_string(),
            extensions,
            file_names,
            grammar_kind: grammar_kind.to_string(),
            grammar_path: grammar_path.to_string(),
            grammar_source,
            highlights_query_path: highlights_query_path.to_string(),
            locals_query_path,
            injections_query_path,
            style_map,
            timeout_ms,
            max_window_bytes,
            estimated_payload_bytes,
        });
    }
    Ok(descriptors)
}

pub(super) fn parse_completion_provider_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<CompletionProviderContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.completionProviders", ctx)?;
    if !entries.is_empty() && !permissions.contains(&PackagePermission::CompletionProvider) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "completion provider contributions require `completion-provider` permission",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let estimated_payload_bytes = contribution_payload_size(entry);
        if estimated_payload_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "completion provider metadata payload ({estimated_payload_bytes} bytes) exceeds BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_completion_provider_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "completion provider contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?.to_string();
        if !seen_ids.insert(id.clone()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                "completion provider IDs must be unique within a package",
            ));
        }
        let priority = obj.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
        let exclusive = match obj.get("exclusive") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "completion provider exclusive must be a boolean",
                ));
            }
        };
        let trigger_characters =
            optional_string_vec(obj.get("triggerCharacters"), "triggerCharacters", ctx)?;
        let word_boundary_chars =
            optional_string_vec(obj.get("wordBoundaryChars"), "wordBoundaryChars", ctx)?;
        let items = parse_completion_items(obj.get("items"), &id, ctx)?;
        if items.len() > COMPLETION_RESULT_MAX_ITEMS {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "completion provider items must contain at most {COMPLETION_RESULT_MAX_ITEMS} entries"
                ),
            ));
        }
        let mut unique_labels = HashSet::new();
        if let Some(item) = items
            .iter()
            .find(|item| !unique_labels.insert(item.label.as_str()))
        {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                format!("completion provider item `{}` is duplicated", item.label),
            ));
        }
        let has_plain_text = items
            .iter()
            .any(|item| item.text_format == CompletionItemTextFormat::PlainText);
        let has_snippets = items
            .iter()
            .any(|item| item.text_format == CompletionItemTextFormat::Snippet);
        if has_plain_text && has_snippets {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "completion provider items cannot mix plainText and snippet formats; use separate providers",
            ));
        }
        let timeout_ms = optional_u64_budget(
            obj.get("budgets").and_then(Value::as_object),
            "timeoutMs",
            &id,
            ctx,
        )?
        .unwrap_or(500);
        let max_items = optional_usize_budget(
            obj.get("budgets").and_then(Value::as_object),
            "maxItems",
            &id,
            ctx,
        )?
        .unwrap_or(64);
        if timeout_ms == 0 || timeout_ms > 5_000 {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "completion provider timeoutMs must be within 1..=5000",
            ));
        }
        if max_items == 0 || max_items > COMPLETION_RESULT_MAX_ITEMS {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "completion provider maxItems must be within 1..={COMPLETION_RESULT_MAX_ITEMS}"
                ),
            ));
        }
        if items.len() > max_items {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "completion provider item count ({}) exceeds its maxItems budget ({max_items})",
                    items.len()
                ),
            ));
        }
        descriptors.push(CompletionProviderContributionDescriptor {
            id,
            priority,
            exclusive,
            trigger_characters,
            word_boundary_chars,
            items,
            timeout_ms,
            max_items,
            estimated_payload_bytes,
        });
    }
    Ok(descriptors)
}

pub(super) fn parse_language_server_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<LanguageServerContributionDescriptor>, PackageRecordError> {
    const MAX_SERVERS: usize = 8;
    const MAX_ARGS: usize = 32;
    const MAX_ENVIRONMENT_NAMES: usize = 32;
    const MAX_VALUE_BYTES: usize = 4096;

    let entries = array_field(value, "clay.contributions.languageServers", ctx)?;
    if entries.len() > MAX_SERVERS {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("languageServers must contain at most {MAX_SERVERS} entries"),
        ));
    }
    if !entries.is_empty() && !permissions.contains(&PackagePermission::LanguageServer) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "language-server contributions require the `language-server` capability",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let obj = object_field(entry, "language-server contribution", ctx)?;
        for field in obj.keys() {
            if !matches!(
                field.as_str(),
                "id" | "executable" | "args" | "inheritEnvironment"
            ) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!("language-server contribution field `{field}` is not allowed"),
                ));
            }
        }
        let id = package_owned_field(obj, "id", api_prefix, ctx)?.to_string();
        if id.len() > 128 || !seen_ids.insert(id.clone()) {
            return Err(ctx.error(
                if id.len() > 128 {
                    PackageRecordRule::InvalidContributionDescriptor
                } else {
                    PackageRecordRule::DuplicateContributionId
                },
                Some(&id),
                "language-server contribution IDs must be unique and at most 128 bytes",
            ));
        }
        let executable = obj
            .get("executable")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "language-server executable must be a non-empty string",
                )
            })?;
        if executable.len() > MAX_VALUE_BYTES || executable.chars().any(char::is_control) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                "language-server executable must be at most 4096 bytes without control characters",
            ));
        }
        let args =
            bounded_string_array(obj.get("args"), "args", MAX_ARGS, MAX_VALUE_BYTES, &id, ctx)?;
        let inherit_environment = bounded_string_array(
            obj.get("inheritEnvironment"),
            "inheritEnvironment",
            MAX_ENVIRONMENT_NAMES,
            128,
            &id,
            ctx,
        )?;
        let mut seen_environment = HashSet::new();
        for name in &inherit_environment {
            let valid = name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            });
            if !valid || !seen_environment.insert(name.as_str()) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "inheritEnvironment names must be unique ASCII environment variable names",
                ));
            }
        }
        descriptors.push(LanguageServerContributionDescriptor {
            id,
            executable: executable.to_string(),
            args,
            inherit_environment,
        });
    }
    Ok(descriptors)
}

pub(super) fn parse_language_intelligence_provider_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<LanguageIntelligenceProviderContributionDescriptor>, PackageRecordError> {
    const MAX_PROVIDERS: usize = 32;
    const MAX_MODES: usize = 32;
    const MAX_FEATURES: usize = 4;

    let entries = array_field(
        value,
        "clay.contributions.languageIntelligenceProviders",
        ctx,
    )?;
    if entries.len() > MAX_PROVIDERS {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("languageIntelligenceProviders must contain at most {MAX_PROVIDERS} entries"),
        ));
    }
    if !entries.is_empty() && !permissions.contains(&PackagePermission::ParseDocument) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "language-intelligence provider contributions require `parse-document` permission",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let estimated_payload_bytes = contribution_payload_size(entry);
        if estimated_payload_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "language-intelligence provider metadata payload ({estimated_payload_bytes} bytes) exceeds BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_language_intelligence_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "language-intelligence provider contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?.to_string();
        if !seen_ids.insert(id.clone()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&id),
                "language-intelligence provider IDs must be unique within a package",
            ));
        }
        let modes = optional_string_vec(obj.get("modes"), "modes", ctx)?;
        if modes.len() > MAX_MODES {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "language-intelligence provider modes must contain at most {MAX_MODES} entries"
                ),
            ));
        }
        let features = parse_language_intelligence_features(obj.get("features"), &id, ctx)?;
        if features.is_empty() || features.len() > MAX_FEATURES {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "language-intelligence provider features must contain 1..={MAX_FEATURES} entries"
                ),
            ));
        }
        let priority = obj.get("priority").and_then(Value::as_i64).unwrap_or(0) as i32;
        let module = match obj.get("module") {
            None | Some(Value::Null) => None,
            Some(Value::String(path)) => {
                if path.is_empty()
                    || path.len() > 512
                    || path.chars().any(char::is_control)
                    || path.contains("..")
                    || path.starts_with('/')
                {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(&id),
                        "language-intelligence provider module must be a bounded package-relative path",
                    ));
                }
                Some(path.clone())
            }
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "language-intelligence provider module must be a string path",
                ));
            }
        };
        let export_name = match obj.get("exportName") {
            None | Some(Value::Null) => "provideLanguageIntelligence".to_string(),
            Some(Value::String(name))
                if !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control) =>
            {
                name.clone()
            }
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&id),
                    "language-intelligence provider exportName must be a non-empty bounded string",
                ));
            }
        };
        let timeout_ms = optional_u64_budget(
            obj.get("budgets").and_then(Value::as_object),
            "timeoutMs",
            &id,
            ctx,
        )?
        .or_else(|| obj.get("timeoutMs").and_then(Value::as_u64))
        .unwrap_or(LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS);
        if timeout_ms == 0 || timeout_ms > LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&id),
                format!(
                    "language-intelligence provider timeoutMs must be within 1..={LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS}"
                ),
            ));
        }
        descriptors.push(LanguageIntelligenceProviderContributionDescriptor {
            id,
            modes,
            features,
            priority,
            module,
            export_name,
            timeout_ms,
            estimated_payload_bytes,
        });
    }
    Ok(descriptors)
}

pub(super) fn parse_language_intelligence_features(
    value: Option<&Value>,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Vec<LanguageIntelligenceFeature>, PackageRecordError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(values) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "language-intelligence provider features must be an array of strings",
        ));
    };
    let mut features = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let Some(name) = value.as_str() else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(contribution_id),
                "language-intelligence provider features must be strings",
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
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(contribution_id),
                    format!("unsupported language-intelligence feature `{other}`"),
                ));
            }
        };
        if seen.insert(feature) {
            features.push(feature);
        }
    }
    Ok(features)
}

pub(super) fn reject_language_intelligence_prohibited_authority(
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "handler"
                        | "callback"
                        | "function"
                        | "clientJavaScript"
                        | "nativeHandle"
                        | "rawOps"
                        | "shellCommand"
                        | "executable"
                        | "languageServer"
                        | "process"
                ) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        None,
                        format!(
                            "language-intelligence provider metadata must not include executable or process authority field `{key}`"
                        ),
                    ));
                }
                reject_language_intelligence_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_language_intelligence_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn bounded_string_array(
    value: Option<&Value>,
    field: &str,
    max_items: usize,
    max_bytes: usize,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Vec<String>, PackageRecordError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(values) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!("language-server {field} must be an array of strings"),
        ));
    };
    if values.len() > max_items {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!("language-server {field} must contain at most {max_items} entries"),
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|text| !text.is_empty() && text.len() <= max_bytes && !text.chars().any(char::is_control))
                .map(str::to_string)
                .ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(contribution_id),
                        format!("language-server {field} entries must be non-empty bounded strings without control characters"),
                    )
                })
        })
        .collect()
}

pub(super) fn reject_completion_provider_prohibited_authority(
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match value {
        Value::String(text)
            if text.contains("://")
                || text.contains("Deno.core.ops")
                || text.contains("nativeHandle")
                || text.contains("drawCallback")
                || text.contains("clientJavaScript")
                || text.contains("rawOps")
                || text.contains("css")
                || text.contains("rawColor") =>
        {
            Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "completion provider metadata must not contain URLs, raw ops, native handles, client JavaScript, CSS, or raw colors",
            ))
        }
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "nativeHandle"
                        | "nativeLibrary"
                        | "dynamicLibrary"
                        | "downloadUrl"
                        | "packageManager"
                        | "shellCommand"
                        | "clientJavaScript"
                        | "drawCallback"
                        | "rawOps"
                        | "css"
                        | "rawColor"
                        | "snippet"
                        | "command"
                ) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        None,
                        format!(
                            "completion provider metadata must not include executable or external authority field `{key}`"
                        ),
                    ));
                }
                reject_completion_provider_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_completion_provider_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn reject_syntax_grammar_prohibited_authority(
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match value {
        Value::String(text)
            if text.contains("://")
                || text.contains("Deno.core.ops")
                || text.contains("nativeHandle")
                || text.contains("drawCallback")
                || text.contains("clientJavaScript")
                || text.contains("rawOps")
                || text.contains("css")
                || text.contains("rawColor") =>
        {
            Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "syntax grammar metadata must not contain URLs, raw ops, native handles, client JavaScript, CSS, raw colors, or concrete typography",
            ))
        }
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "nativeHandle"
                        | "nativeLibrary"
                        | "dynamicLibrary"
                        | "downloadUrl"
                        | "packageManager"
                        | "shellCommand"
                        | "clientJavaScript"
                        | "drawCallback"
                        | "rawOps"
                        | "css"
                        | "rawColor"
                        | "fontFamily"
                        | "fontFamilies"
                        | "fontSize"
                        | "fontStack"
                ) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        None,
                        format!(
                            "syntax grammar metadata must not include executable, external, or concrete typography field `{key}`"
                        ),
                    ));
                }
                reject_syntax_grammar_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_syntax_grammar_prohibited_authority(nested, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn is_valid_language_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_' | '+' | '.')
        })
}

pub(super) fn validate_package_asset_path(
    path: &str,
    field: &str,
    required_suffix: Option<&str>,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let valid = path.starts_with("./")
        && !path.contains('\\')
        && !path.contains("://")
        && !path.contains("Deno.core.ops")
        && path[2..]
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if !valid {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(path),
            format!("{field} must be a package-root-confined relative ./ path without traversal, URLs, or raw ops"),
        ));
    }
    if let Some(suffix) = required_suffix
        && !path.ends_with(suffix)
    {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(path),
            format!("{field} must end with {suffix}"),
        ));
    }
    Ok(())
}

pub(super) fn optional_asset_path(
    value: Option<&Value>,
    field: &str,
    ctx: &ErrorContext,
) -> Result<Option<String>, PackageRecordError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(path)) if !path.trim().is_empty() => {
            validate_package_asset_path(path, field, Some(".scm"), ctx)?;
            Ok(Some(path.clone()))
        }
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{field} must be a non-empty string path when present"),
        )),
    }
}

pub(super) fn parse_syntax_style_map(
    value: Option<&Value>,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<BTreeMap<String, SyntaxStyleMapEntry>, PackageRecordError> {
    let Some(Value::Object(map)) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "syntax grammar styleMap must map captures to vocabulary objects or known legacy Clay style tokens",
        ));
    };
    if map.is_empty() {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "syntax grammar styleMap must not be empty",
        ));
    }

    let mut style_map = BTreeMap::new();
    for (capture, value) in map {
        if capture.trim().is_empty()
            || capture.starts_with('@')
            || capture.contains('{')
            || capture.contains('}')
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(capture),
                "syntax grammar capture names must be non-empty names without @, braces, CSS, or query payloads",
            ));
        }

        let priority = match value {
            Value::Object(entry) => match entry.get("priority") {
                None => DEFAULT_SYNTAX_STYLE_PRIORITY,
                Some(Value::Number(number)) => match number
                    .as_u64()
                    .and_then(|value| u16::try_from(value).ok())
                    .filter(|value| *value <= MAX_SYNTAX_STYLE_PRIORITY)
                {
                    Some(priority) => priority,
                    None => {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(capture),
                            "syntax grammar styleMap `priority` must be an integer between 0 and 100",
                        ));
                    }
                },
                Some(_) => {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(capture),
                        "syntax grammar styleMap `priority` must be an integer between 0 and 100",
                    ));
                }
            },
            _ => DEFAULT_SYNTAX_STYLE_PRIORITY,
        };

        let (token_type, modifiers, scope, font_role) = match value {
            Value::String(style_token) => {
                if !is_known_syntax_style_token(style_token) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(style_token),
                        "syntax grammar styleMap legacy values must be known Clay style tokens, not raw CSS or colors",
                    ));
                }
                let (token_type, modifiers) = TokenType::classify_style_token(style_token);
                (token_type, modifiers, Some(style_token.clone()), None)
            }
            Value::Object(entry) => {
                if entry.contains_key("fontFamily")
                    || entry.contains_key("fontFamilies")
                    || entry.contains_key("fontSize")
                    || entry.contains_key("fontStack")
                    || entry.contains_key("color")
                {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(capture),
                        "syntax grammar styleMap entries may select vocabulary and a semantic fontRole, never font families, sizes, stacks, or colors",
                    ));
                }
                let font_role = match entry.get("fontRole") {
                    None => None,
                    Some(Value::String(role)) => match DocumentFontRole::from_name(role) {
                        Some(
                            role @ (DocumentFontRole::Monospace | DocumentFontRole::Proportional),
                        ) => Some(role),
                        _ => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(role),
                                "syntax grammar styleMap fontRole must be `monospace` or `proportional`",
                            ));
                        }
                    },
                    Some(_) => {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(capture),
                            "syntax grammar styleMap fontRole must be a semantic role string",
                        ));
                    }
                };

                if let Some(style_token) = entry.get("styleToken").and_then(Value::as_str) {
                    if entry.contains_key("type") || entry.contains_key("modifiers") {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(capture),
                            "syntax grammar styleMap entries must use either legacy `styleToken` or vocabulary `type` + `modifiers`, never both",
                        ));
                    }
                    if !is_known_syntax_style_token(style_token) {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(style_token),
                            "syntax grammar styleMap legacy values must be known Clay style tokens, not raw CSS or colors",
                        ));
                    }
                    let (token_type, modifiers) = TokenType::classify_style_token(style_token);
                    (
                        token_type,
                        modifiers,
                        Some(style_token.to_string()),
                        font_role,
                    )
                } else {
                    let type_name = entry
                        .get("type")
                        .and_then(Value::as_str)
                        .filter(|value| !value.trim().is_empty())
                        .ok_or_else(|| {
                            ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(capture),
                                "syntax grammar styleMap entry requires a known `type` naming a TokenType variant (e.g. `Keyword`, `String`, `Heading1`)",
                            )
                        })?;
                    let token_type = TokenType::from_name(type_name).ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(type_name),
                            "syntax grammar styleMap `type` must name a known TokenType variant, not raw CSS, a color, or a free-form string",
                        )
                    })?;
                    let modifiers = match entry.get("modifiers") {
                        None => Modifiers::NONE,
                        Some(Value::Array(names)) => {
                            let parsed: Vec<&str> = names
                                .iter()
                                .map(|value| value.as_str().unwrap_or(""))
                                .collect();
                            Modifiers::from_names(&parsed).ok_or_else(|| {
                                ctx.error(
                                    PackageRecordRule::InvalidContributionDescriptor,
                                    Some(capture),
                                    "syntax grammar styleMap `modifiers` must be an array of known Modifiers variant names (e.g. `Declaration`, `Bold`)",
                                )
                            })?
                        }
                        Some(_) => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(capture),
                                "syntax grammar styleMap `modifiers` must be an array of known Modifiers variant names",
                            ));
                        }
                    };
                    (token_type, modifiers, None, font_role)
                }
            }
            _ => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(capture),
                    "syntax grammar styleMap values must be vocabulary objects or known legacy Clay style-token strings",
                ));
            }
        };

        style_map.insert(
            capture.clone(),
            SyntaxStyleMapEntry {
                token_type,
                modifiers,
                scope,
                font_role,
                priority,
            },
        );
    }
    Ok(style_map)
}

pub(super) fn is_known_syntax_style_token(token: &str) -> bool {
    matches!(
        token,
        "markup.heading.1"
            | "markup.heading.2"
            | "markup.heading.3"
            | "markup.heading.4"
            | "markup.heading.5"
            | "markup.heading.6"
            | "markup.strong"
            | "markup.emphasis"
            | "markup.inline-code"
            | "markup.code-block"
            | "markup.list-marker"
            | "keyword.control"
            | "string.quoted"
            | "comment.line"
            | "punctuation.definition"
            | "diagnostic.error"
            | "diagnostic.warning"
            | "diagnostic.info"
            | "search.match"
            | "text"
    )
}

pub(super) fn optional_u64_budget(
    budgets: Option<&serde_json::Map<String, Value>>,
    field: &str,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Option<u64>, PackageRecordError> {
    match budgets.and_then(|budgets| budgets.get(field)) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number.as_u64().map(Some).ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(contribution_id),
                format!("budgets.{field} must be a non-negative integer"),
            )
        }),
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!("budgets.{field} must be a non-negative integer"),
        )),
    }
}

pub(super) fn optional_usize_budget(
    budgets: Option<&serde_json::Map<String, Value>>,
    field: &str,
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<Option<usize>, PackageRecordError> {
    optional_u64_budget(budgets, field, contribution_id, ctx).map(|value| value.map(|n| n as usize))
}

pub(super) fn parse_completion_items(
    value: Option<&Value>,
    provider_id: &str,
    ctx: &ErrorContext,
) -> Result<Vec<CompletionItemContributionDescriptor>, PackageRecordError> {
    let values = match value {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(values)) => values,
        Some(_) => {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(provider_id),
                "completion provider items must be an array",
            ));
        }
    };

    values
        .iter()
        .map(|value| {
            let item = match value {
                Value::String(text) if !text.trim().is_empty() => {
                    CompletionItemContributionDescriptor {
                        label: text.clone(),
                        insert_text: text.clone(),
                        detail: String::new(),
                        text_format: CompletionItemTextFormat::PlainText,
                    }
                }
                Value::Object(object) => {
                    if object.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            "label" | "insertText" | "detail" | "textFormat"
                        )
                    }) {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(provider_id),
                            "completion provider item objects accept only label, insertText, detail, and textFormat",
                        ));
                    }
                    let label = required_str_field(object, "label", ctx)?.to_string();
                    let insert_text =
                        required_str_field(object, "insertText", ctx)?.to_string();
                    let detail = match object.get("detail") {
                        None | Some(Value::Null) => String::new(),
                        Some(Value::String(detail)) => detail.clone(),
                        Some(_) => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(provider_id),
                                "completion provider item detail must be a string",
                            ));
                        }
                    };
                    let text_format = match object.get("textFormat") {
                        None | Some(Value::Null) => CompletionItemTextFormat::PlainText,
                        Some(Value::String(value)) if value == "plainText" => {
                            CompletionItemTextFormat::PlainText
                        }
                        Some(Value::String(value)) if value == "snippet" => {
                            CompletionItemTextFormat::Snippet
                        }
                        _ => {
                            return Err(ctx.error(
                                PackageRecordRule::InvalidContributionDescriptor,
                                Some(provider_id),
                                "completion provider item textFormat must be `plainText` or `snippet`",
                            ));
                        }
                    };
                    CompletionItemContributionDescriptor {
                        label,
                        insert_text,
                        detail,
                        text_format,
                    }
                }
                _ => {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(provider_id),
                        "completion provider items must be non-empty strings or structured item objects",
                    ));
                }
            };

            for (field, chars, max) in [
                (
                    "label",
                    item.label.chars().count(),
                    COMPLETION_RESULT_MAX_ITEM_LABEL_CHARS,
                ),
                (
                    "insertText",
                    item.insert_text.chars().count(),
                    COMPLETION_RESULT_MAX_ITEM_INSERT_TEXT_CHARS,
                ),
                (
                    "detail",
                    item.detail.chars().count(),
                    COMPLETION_RESULT_MAX_ITEM_DETAIL_CHARS,
                ),
            ] {
                if chars > max {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(provider_id),
                        format!("completion provider item {field} exceeds {max} characters"),
                    ));
                }
            }
            Ok(item)
        })
        .collect()
}

pub(super) fn optional_string_vec(
    value: Option<&Value>,
    key: &str,
    ctx: &ErrorContext,
) -> Result<Vec<String>, PackageRecordError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            None,
                            format!("{key} entries must be non-empty strings"),
                        )
                    })
            })
            .collect(),
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{key} must be an array"),
        )),
    }
}
