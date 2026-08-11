//! Versioned extension-point declarations and structured package-relation
//! requests (Plan 061 task 6).
//!
//! First-party (owner) packages declare closed, versioned extension points in
//! `clay.extensionPoints`; adopting packages request exact
//! package/point/operation/scope relations through structured entries in
//! `clay.extends`, `clay.imports`, or `clay.overrides`. All parsing is
//! fail-closed: unknown fields, operations, contribution kinds, cross-package
//! prefixes, duplicate ids, and oversize values are rejected at manifest
//! validation time, before any package code executes. The schemas and limits
//! are locked in `docs/reference/primitives/package-security.md`
//! (`clay-extension-point-v1`, `clay-package-relation-v1`).

use serde_json::Value;

use crate::packages::manifest::{DiagnosticContext, PackageValidationRule};

/// Maximum extension points a single manifest may declare.
pub const MAX_EXTENSION_POINTS_PER_MANIFEST: usize = 64;
/// Maximum scopes per extension point or relation request.
pub const MAX_EXTENSION_SCOPES: usize = 32;
/// Maximum characters in one scope string.
pub const MAX_SCOPE_CHARS: usize = 128;
/// Maximum characters in an extension-point id.
pub const MAX_EXTENSION_POINT_ID_CHARS: usize = 64;
/// Maximum characters in display text (summary/justification).
pub const MAX_DISPLAY_TEXT_CHARS: usize = 280;
/// Maximum structured relation requests per manifest.
pub const MAX_RELATION_REQUESTS_PER_MANIFEST: usize = 64;

/// Closed mutation operation vocabulary for extension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationOperation {
    Append,
    Replace,
}

impl RelationOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Replace => "replace",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "append" => Some(Self::Append),
            "replace" => Some(Self::Replace),
            _ => None,
        }
    }
}

/// Closed contribution-kind vocabulary an extension point may govern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionContributionKind {
    ModePattern,
    Grammar,
    Command,
    KeyRoute,
    TextTransform,
    CompletionProvider,
    DecorationLayer,
    DiagnosticSource,
    Analyzer,
    IntelligenceProvider,
    PanelContribution,
    ComponentContribution,
    OverlayContribution,
    ThemeTokens,
    SduiRegion,
    StatusItem,
}

impl ExtensionContributionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModePattern => "modePattern",
            Self::Grammar => "grammar",
            Self::Command => "command",
            Self::KeyRoute => "keyRoute",
            Self::TextTransform => "textTransform",
            Self::CompletionProvider => "completionProvider",
            Self::DecorationLayer => "decorationLayer",
            Self::DiagnosticSource => "diagnosticSource",
            Self::Analyzer => "analyzer",
            Self::IntelligenceProvider => "intelligenceProvider",
            Self::PanelContribution => "panelContribution",
            Self::ComponentContribution => "componentContribution",
            Self::OverlayContribution => "overlayContribution",
            Self::ThemeTokens => "themeTokens",
            Self::SduiRegion => "sduiRegion",
            Self::StatusItem => "statusItem",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "modePattern" => Self::ModePattern,
            "grammar" => Self::Grammar,
            "command" => Self::Command,
            "keyRoute" => Self::KeyRoute,
            "textTransform" => Self::TextTransform,
            "completionProvider" => Self::CompletionProvider,
            "decorationLayer" => Self::DecorationLayer,
            "diagnosticSource" => Self::DiagnosticSource,
            "analyzer" => Self::Analyzer,
            "intelligenceProvider" => Self::IntelligenceProvider,
            "panelContribution" => Self::PanelContribution,
            "componentContribution" => Self::ComponentContribution,
            "overlayContribution" => Self::OverlayContribution,
            "themeTokens" => Self::ThemeTokens,
            "sduiRegion" => Self::SduiRegion,
            "statusItem" => Self::StatusItem,
            _ => return None,
        })
    }
}

/// One owner-declared, versioned extension point (`clay-extension-point-v1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionPointDeclaration {
    /// Package-prefixed `prefix.name` identifier.
    pub id: String,
    /// Positive version, bumped by the owner on incompatible change.
    pub version: u64,
    /// Non-empty subset of the closed operation enum.
    pub operations: Vec<RelationOperation>,
    /// Non-empty subset of the closed contribution-kind enum.
    pub contribution_kinds: Vec<ExtensionContributionKind>,
    /// Owner-prefixed contribution ids or `prefix.*` wildcards the point may
    /// touch; empty means the point does not withdraw owner contributions.
    pub scopes: Vec<String>,
    /// Optional display text; rendered alongside host facts, never authority.
    pub summary: Option<String>,
}

/// One structured relation request from an adopting package
/// (`clay-package-relation-v1`). `relation_key` records which manifest field
/// carried the entry (`extends`/`imports`/`overrides`) for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredRelationRequest {
    pub package: String,
    pub extension_point: String,
    pub version: u64,
    pub operation: RelationOperation,
    /// Requester-prefixed contribution ids (or `prefix.*`) the requester will
    /// append/replace through this relation.
    pub scopes: Vec<String>,
    pub justification: Option<String>,
    /// Manifest field that carried this entry (`extends`/`imports`/`overrides`).
    pub relation_key: String,
}

/// Deterministic rejection codes for host-side relation verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RelationVerificationError {
    UnknownExtensionPoint {
        point: String,
    },
    VersionMismatch {
        point: String,
        declared: u64,
        requested: u64,
    },
    OperationNotOffered {
        point: String,
        operation: String,
    },
}

impl RelationVerificationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownExtensionPoint { .. } => "package_relation.unknown_extension_point",
            Self::VersionMismatch { .. } => "package_relation.version_mismatch",
            Self::OperationNotOffered { .. } => "package_relation.operation_not_offered",
        }
    }
}

/// Verify a relation request against the target package's declared extension
/// points. Returns the matching declaration on success. Scope and prefix
/// well-formedness was already enforced at manifest parse time.
pub(crate) fn verify_relation_request<'a>(
    target_points: &'a [ExtensionPointDeclaration],
    request: &StructuredRelationRequest,
) -> Result<&'a ExtensionPointDeclaration, RelationVerificationError> {
    let declaration = target_points
        .iter()
        .find(|point| point.id == request.extension_point)
        .ok_or_else(|| RelationVerificationError::UnknownExtensionPoint {
            point: request.extension_point.clone(),
        })?;
    if declaration.version != request.version {
        return Err(RelationVerificationError::VersionMismatch {
            point: request.extension_point.clone(),
            declared: declaration.version,
            requested: request.version,
        });
    }
    if !declaration.operations.contains(&request.operation) {
        return Err(RelationVerificationError::OperationNotOffered {
            point: request.extension_point.clone(),
            operation: request.operation.as_str().to_string(),
        });
    }
    Ok(declaration)
}

// ── Manifest parsing ─────────────────────────────────────────────────────────

type ParseResult<T> = Result<T, crate::packages::manifest::PackageDiagnostic>;

fn fail<T>(
    context: &DiagnosticContext,
    rule: PackageValidationRule,
    message: impl Into<String>,
) -> ParseResult<T> {
    Err(context.diagnostic(rule, message))
}

fn is_valid_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.len() <= 32
        && prefix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A scope is `prefix.contribution-id` or a single trailing `prefix.*`
/// wildcard. The prefix is lowercase/digits/hyphens; the contribution-id
/// segment is alphanumeric/dots/hyphens (real contribution ids use camelCase).
fn parse_scope(raw: &str) -> Option<(String, bool)> {
    if raw.is_empty() || raw.chars().count() > MAX_SCOPE_CHARS {
        return None;
    }
    if let Some(prefix) = raw.strip_suffix(".*") {
        if is_valid_prefix(prefix) {
            return Some((prefix.to_string(), true));
        }
        return None;
    }
    let (prefix, id) = raw.split_once('.')?;
    if !is_valid_prefix(prefix)
        || id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return None;
    }
    Some((prefix.to_string(), false))
}

fn read_scopes(
    value: Option<&Value>,
    owner_prefix: &str,
    field: &str,
    context: &DiagnosticContext,
) -> ParseResult<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value.as_array().ok_or_else(|| {
        context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            format!("{field} must be an array of scope strings"),
        )
    })?;
    if array.len() > MAX_EXTENSION_SCOPES {
        return fail(
            context,
            PackageValidationRule::InvalidPackageGraph,
            format!("{field} supports at most {MAX_EXTENSION_SCOPES} scopes"),
        );
    }
    let mut scopes = Vec::with_capacity(array.len());
    for entry in array {
        let raw = entry.as_str().ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::InvalidPackageGraph,
                format!("{field} entries must be strings"),
            )
        })?;
        let Some((prefix, _wildcard)) = parse_scope(raw) else {
            return fail(
                context,
                PackageValidationRule::InvalidPackageGraph,
                format!(
                    "{field} scope `{raw}` must be `prefix.contribution-id` or a single trailing `prefix.*` wildcard (max {MAX_SCOPE_CHARS} chars)"
                ),
            );
        };
        if prefix != owner_prefix {
            return fail(
                context,
                PackageValidationRule::InvalidPackageGraph,
                format!(
                    "{field} scope `{raw}` uses prefix `{prefix}`; only the owning package prefix `{owner_prefix}` is allowed"
                ),
            );
        }
        scopes.push(raw.to_string());
    }
    Ok(scopes)
}

fn read_string_list(
    value: &Value,
    field: &str,
    context: &DiagnosticContext,
) -> ParseResult<Vec<String>> {
    let array = value.as_array().ok_or_else(|| {
        context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            format!("{field} must be a non-empty array of strings"),
        )
    })?;
    if array.is_empty() {
        return fail(
            context,
            PackageValidationRule::InvalidPackageGraph,
            format!("{field} must be a non-empty array of strings"),
        );
    }
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let raw = entry.as_str().ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::InvalidPackageGraph,
                format!("{field} entries must be strings"),
            )
        })?;
        out.push(raw.to_string());
    }
    Ok(out)
}

fn reject_unknown_fields(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    what: &str,
    context: &DiagnosticContext,
) -> ParseResult<()> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return fail(
                context,
                PackageValidationRule::InvalidPackageGraph,
                format!("{what} does not allow unknown field `{key}`"),
            );
        }
    }
    Ok(())
}

fn read_optional_display_text(
    value: Option<&Value>,
    field: &str,
    context: &DiagnosticContext,
) -> ParseResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let raw = value.as_str().ok_or_else(|| {
        context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            format!("{field} must be a string"),
        )
    })?;
    if raw.chars().count() > MAX_DISPLAY_TEXT_CHARS {
        return fail(
            context,
            PackageValidationRule::InvalidPackageGraph,
            format!("{field} supports at most {MAX_DISPLAY_TEXT_CHARS} chars"),
        );
    }
    Ok(Some(raw.to_string()))
}

fn read_positive_version(
    value: Option<&Value>,
    field: &str,
    context: &DiagnosticContext,
) -> ParseResult<u64> {
    let version = value
        .and_then(Value::as_u64)
        .filter(|version| *version > 0)
        .ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::InvalidPackageGraph,
                format!("{field} must be a positive integer"),
            )
        })?;
    Ok(version)
}

/// Parse `clay.extensionPoints` (owner declarations). The id must be
/// package-prefixed with the manifest's own api prefix and ids must be unique.
pub(crate) fn parse_extension_points(
    value: Option<&Value>,
    api_prefix: &str,
    context: &DiagnosticContext,
) -> ParseResult<Vec<ExtensionPointDeclaration>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value.as_array().ok_or_else(|| {
        context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            "clay.extensionPoints must be an array",
        )
    })?;
    if array.len() > MAX_EXTENSION_POINTS_PER_MANIFEST {
        return fail(
            context,
            PackageValidationRule::InvalidPackageGraph,
            format!(
                "clay.extensionPoints supports at most {MAX_EXTENSION_POINTS_PER_MANIFEST} entries"
            ),
        );
    }
    let mut points = Vec::with_capacity(array.len());
    for entry in array {
        let object = entry.as_object().ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::InvalidPackageGraph,
                "clay.extensionPoints entries must be objects",
            )
        })?;
        reject_unknown_fields(
            object,
            &[
                "id",
                "version",
                "operations",
                "contributionKinds",
                "scopes",
                "summary",
            ],
            "clay.extensionPoints entry",
            context,
        )?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.chars().count() <= MAX_EXTENSION_POINT_ID_CHARS)
            .ok_or_else(|| {
                context.diagnostic(
                    PackageValidationRule::InvalidPackageGraph,
                    format!(
                        "clay.extensionPoints id must be a non-empty string of at most {MAX_EXTENSION_POINT_ID_CHARS} chars"
                    ),
                )
            })?;
        let Some((point_prefix, point_name)) = id.split_once('.') else {
            return fail(
                context,
                PackageValidationRule::InvalidPackageGraph,
                format!("clay.extensionPoints id `{id}` must be package-prefixed `prefix.name`"),
            );
        };
        // The point name follows contribution-id conventions (camelCase
        // allowed, e.g. `markdown.completionProviders`); the prefix segment
        // must equal the owning api prefix.
        if point_prefix != api_prefix
            || point_name.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return fail(
                context,
                PackageValidationRule::InvalidPackageGraph,
                format!(
                    "clay.extensionPoints id `{id}` must use the owning package prefix `{api_prefix}` and lowercase/digits/dots/hyphens"
                ),
            );
        }
        if points
            .iter()
            .any(|point: &ExtensionPointDeclaration| point.id == id)
        {
            return fail(
                context,
                PackageValidationRule::InvalidPackageGraph,
                format!("clay.extensionPoints declares duplicate id `{id}`"),
            );
        }
        let operations = read_string_list(
            object.get("operations").unwrap_or(&Value::Null),
            "clay.extensionPoints operations",
            context,
        )?
        .into_iter()
        .map(|raw| {
            RelationOperation::parse(&raw).ok_or_else(|| {
                context.diagnostic(
                    PackageValidationRule::InvalidPackageGraph,
                    format!("clay.extensionPoints operation `{raw}` is not append/replace"),
                )
            })
        })
        .collect::<ParseResult<Vec<_>>>()?;
        let contribution_kinds = read_string_list(
            object.get("contributionKinds").unwrap_or(&Value::Null),
            "clay.extensionPoints contributionKinds",
            context,
        )?
        .into_iter()
        .map(|raw| {
            ExtensionContributionKind::parse(&raw).ok_or_else(|| {
                context.diagnostic(
                    PackageValidationRule::InvalidPackageGraph,
                    format!(
                        "clay.extensionPoints contribution kind `{raw}` is not in the closed kind enum"
                    ),
                )
            })
        })
        .collect::<ParseResult<Vec<_>>>()?;
        let scopes = read_scopes(
            object.get("scopes"),
            api_prefix,
            "clay.extensionPoints scopes",
            context,
        )?;
        let summary = read_optional_display_text(
            object.get("summary"),
            "clay.extensionPoints summary",
            context,
        )?;
        points.push(ExtensionPointDeclaration {
            id: id.to_string(),
            version: read_positive_version(
                object.get("version"),
                "clay.extensionPoints version",
                context,
            )?,
            operations,
            contribution_kinds,
            scopes,
            summary,
        });
    }
    Ok(points)
}

/// Parse structured relation entries from one manifest field (`clay.extends`,
/// `clay.imports`, `clay.overrides`). String entries are legacy name targets
/// handled by the caller; only object entries arrive here. Request scopes must
/// name only the requester's own api prefix.
pub(crate) fn parse_structured_relation(
    object: &serde_json::Map<String, Value>,
    relation_key: &str,
    requester_prefix: &str,
    context: &DiagnosticContext,
) -> ParseResult<StructuredRelationRequest> {
    let field = format!("clay.{relation_key} entry");
    reject_unknown_fields(
        object,
        &[
            "package",
            "extensionPoint",
            "version",
            "operation",
            "scopes",
            "justification",
        ],
        &field,
        context,
    )?;
    let package = object
        .get("package")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::MissingField,
                format!("{field} must include non-empty package"),
            )
        })?;
    let extension_point = object
        .get("extensionPoint")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.chars().count() <= MAX_EXTENSION_POINT_ID_CHARS)
        .ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::MissingField,
                format!("{field} must include non-empty extensionPoint"),
            )
        })?;
    let operation_raw = object
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::MissingField,
                format!("{field} must include operation"),
            )
        })?;
    let operation = RelationOperation::parse(operation_raw).ok_or_else(|| {
        context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            format!("{field} operation `{operation_raw}` is not append/replace"),
        )
    })?;
    let scopes = read_scopes(
        object.get("scopes"),
        requester_prefix,
        &format!("{field} scopes"),
        context,
    )?;
    Ok(StructuredRelationRequest {
        package: package.to_string(),
        extension_point: extension_point.to_string(),
        version: read_positive_version(
            object.get("version"),
            &format!("{field} version"),
            context,
        )?,
        operation,
        scopes,
        justification: read_optional_display_text(
            object.get("justification"),
            &format!("{field} justification"),
            context,
        )?,
        relation_key: relation_key.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::packages::manifest::DiagnosticContext;

    fn ctx() -> DiagnosticContext {
        DiagnosticContext::new(
            Some("@clay/markdown".to_string()),
            Some("0.1.0".to_string()),
            Some("markdown".to_string()),
        )
    }

    #[test]
    fn extension_point_declaration_round_trips_valid_entry() {
        let value = json!([{
            "id": "markdown.completionProviders",
            "version": 1,
            "operations": ["append", "replace"],
            "contributionKinds": ["completionProvider"],
            "scopes": ["markdown.*"],
            "summary": "Add or replace Markdown completion providers."
        }]);
        let points = parse_extension_points(Some(&value), "markdown", &ctx()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].id, "markdown.completionProviders");
        assert_eq!(points[0].version, 1);
        assert_eq!(
            points[0].operations,
            vec![RelationOperation::Append, RelationOperation::Replace]
        );
        assert_eq!(
            points[0].contribution_kinds,
            vec![ExtensionContributionKind::CompletionProvider]
        );
        assert_eq!(points[0].scopes, vec!["markdown.*"]);
    }

    #[test]
    fn extension_point_rejects_unknown_field_operation_kind_duplicate_and_prefix() {
        for (mut entry, expected) in [
            (
                json!({"id":"markdown.p","version":1,"operations":["append"],"contributionKinds":["command"],"extra":true}),
                "unknown field",
            ),
            (
                json!({"id":"markdown.p","version":1,"operations":["mutate"],"contributionKinds":["command"]}),
                "not append/replace",
            ),
            (
                json!({"id":"markdown.p","version":1,"operations":["append"],"contributionKinds":["process"]}),
                "closed kind enum",
            ),
            (
                json!({"id":"other.p","version":1,"operations":["append"],"contributionKinds":["command"]}),
                "owning package prefix",
            ),
            (
                json!({"id":"markdown.p","version":0,"operations":["append"],"contributionKinds":["command"]}),
                "positive integer",
            ),
        ] {
            let expected: &str = expected;
            let _ = entry.as_object_mut().unwrap();
            let value = json!([entry]);
            let error = parse_extension_points(Some(&value), "markdown", &ctx())
                .unwrap_err()
                .message;
            assert!(
                error.contains(expected),
                "expected `{expected}`, got {error}"
            );
        }
        let duplicate = json!([
            {"id":"markdown.p","version":1,"operations":["append"],"contributionKinds":["command"]},
            {"id":"markdown.p","version":2,"operations":["append"],"contributionKinds":["command"]},
        ]);
        let error = parse_extension_points(Some(&duplicate), "markdown", &ctx())
            .unwrap_err()
            .message;
        assert!(error.contains("duplicate id"), "got {error}");
    }

    #[test]
    fn structured_relation_validates_scopes_and_operations() {
        let object = json!({
            "package": "@clay/markdown",
            "extensionPoint": "markdown.completionProviders",
            "version": 1,
            "operation": "append",
            "scopes": ["vendor-markdown.wikilinks"],
            "justification": "Wikilink completion."
        })
        .as_object()
        .unwrap()
        .clone();
        let request =
            parse_structured_relation(&object, "extends", "vendor-markdown", &ctx()).unwrap();
        assert_eq!(request.package, "@clay/markdown");
        assert_eq!(request.operation, RelationOperation::Append);
        assert_eq!(request.scopes, vec!["vendor-markdown.wikilinks"]);

        // Scope naming another package's prefix fails closed.
        let mut bad = object.clone();
        bad.insert("scopes".to_string(), json!(["markdown.wikilinks"]));
        let error = parse_structured_relation(&bad, "extends", "vendor-markdown", &ctx())
            .unwrap_err()
            .message;
        assert!(
            error.contains("only the owning package prefix"),
            "got {error}"
        );
    }

    #[test]
    fn verify_relation_request_matches_exact_declaration() {
        let value = json!([{
            "id": "markdown.completionProviders",
            "version": 2,
            "operations": ["append"],
            "contributionKinds": ["completionProvider"]
        }]);
        let points = parse_extension_points(Some(&value), "markdown", &ctx()).unwrap();
        let request = StructuredRelationRequest {
            package: "@clay/markdown".to_string(),
            extension_point: "markdown.completionProviders".to_string(),
            version: 1,
            operation: RelationOperation::Append,
            scopes: vec![],
            justification: None,
            relation_key: "extends".to_string(),
        };
        let error = verify_relation_request(&points, &request).unwrap_err();
        assert_eq!(error.code(), "package_relation.version_mismatch");
        let request = StructuredRelationRequest {
            version: 2,
            ..request
        };
        verify_relation_request(&points, &request).expect("exact match verifies");
        let request = StructuredRelationRequest {
            operation: RelationOperation::Replace,
            ..request
        };
        let error = verify_relation_request(&points, &request).unwrap_err();
        assert_eq!(error.code(), "package_relation.operation_not_offered");
    }
}
