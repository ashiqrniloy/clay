//! Typed, bounded cross-domain extension invocation (Plan 061 task 7,
//! `clay-cross-domain-envelope-v1`).
//!
//! The two runtime trust domains never exchange V8 objects, functions,
//! promises, globals, or module instances. A third-party package may affect
//! first-party behavior only through Rust-mediated, typed, bounded, inert
//! extension-point requests; Rust registries and server state remain the
//! primary authority. This module is the canonical request/result contract
//! and the ingress validator: every field that names identity is
//! re-resolved against host-owned package state (never trusted from the
//! requester), current approval/grants are revalidated per request, and
//! payload size is checked before any allocation-heavy parsing.

use serde_json::Value;

use crate::packages::approvals::ApprovalMismatch;
use crate::packages::bundled::RuntimeDomain;
use crate::packages::extension_points::RelationOperation;
use crate::packages::service::PackageService;
use crate::perf::budgets::CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES;

/// Maximum pending cross-domain requests per lane.
pub(crate) const CROSS_DOMAIN_MAX_PENDING_REQUESTS: usize = 16;
/// Maximum deadline a requester may ask for (milliseconds).
pub(crate) const CROSS_DOMAIN_MAX_DEADLINE_MS: u64 = 250;
/// Maximum characters in an extension-point id on the envelope.
const MAX_ENVELOPE_POINT_CHARS: usize = 64;

/// Closed result-status vocabulary (`clay-cross-domain-envelope-v1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrossDomainStatus {
    Ok,
    Error,
    Denied,
    Stale,
    Revoked,
    Timeout,
}

impl CrossDomainStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Denied => "denied",
            Self::Stale => "stale",
            Self::Revoked => "revoked",
            Self::Timeout => "timeout",
        }
    }
}

/// Host-stamped requesting package identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrossDomainRequester {
    pub package: String,
    pub version: String,
    /// Requester's runtime-generation at dispatch; stale generations are
    /// rejected before the target is touched.
    pub generation: u64,
}

/// Host-stamped target identity: package + declared extension point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrossDomainTarget {
    pub package: String,
    pub extension_point: String,
    pub version: u64,
}

/// Typed, bounded, inert cross-domain extension request. All provenance
/// fields are stamped by the host at the bridge; requester-supplied identity
/// is never accepted.
#[derive(Debug, Clone)]
pub(crate) struct CrossDomainRequestEnvelope {
    pub request_id: u64,
    pub requester: CrossDomainRequester,
    pub target: CrossDomainTarget,
    pub operation: RelationOperation,
    pub scopes: Vec<String>,
    /// Approval binding (`package@version`) revalidated at ingress.
    pub approval_ref: String,
    pub deadline_ms: u64,
    /// Bounded inert JSON; no functions, handles, or class instances.
    pub payload: Value,
}

/// Typed cross-domain result; inert payloads only.
#[derive(Debug, Clone)]
pub(crate) struct CrossDomainResultEnvelope {
    pub request_id: u64,
    pub status: CrossDomainStatus,
    pub payload: Value,
}

/// Why a request was rejected at ingress, mapped to a result status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CrossDomainDenial {
    pub status: CrossDomainStatus,
    pub code: &'static str,
    pub detail: String,
}

impl CrossDomainDenial {
    fn new(status: CrossDomainStatus, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status,
            code,
            detail: detail.into(),
        }
    }
}

/// A validated route: the request passed every ingress check and names the
/// exact enabled records it may touch.
#[derive(Debug, Clone)]
pub(crate) struct ValidatedCrossDomainRoute {
    pub requester: CrossDomainRequester,
    pub target: CrossDomainTarget,
    pub operation: RelationOperation,
    pub scopes: Vec<String>,
    pub deadline_ms: u64,
    pub payload: Value,
}

/// Validate an envelope against current host package state. This is the
/// single ingress check for cross-domain extension traffic; handlers receive
/// only the validated route. Validation re-checks:
///
/// - payload budget (before any handler allocation),
/// - requester is an enabled third-domain package at the exact version,
/// - target is enabled, declares the point at the exact version, and offers
///   the operation,
/// - the durable approval still covers this exact edge (identity, scopes),
/// - deadline within the fixed ceiling.
pub(crate) fn validate_cross_domain_request(
    service: &PackageService,
    envelope: &CrossDomainRequestEnvelope,
) -> Result<ValidatedCrossDomainRoute, CrossDomainDenial> {
    // Payload budget first: reject oversize before any further work.
    let payload_bytes = serde_json::to_vec(&envelope.payload)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX);
    if payload_bytes > CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES {
        return Err(CrossDomainDenial::new(
            CrossDomainStatus::Denied,
            "cross_domain.payload_too_large",
            format!(
                "payload is {payload_bytes} bytes (budget {CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES})"
            ),
        ));
    }
    if envelope.deadline_ms == 0 || envelope.deadline_ms > CROSS_DOMAIN_MAX_DEADLINE_MS {
        return Err(CrossDomainDenial::new(
            CrossDomainStatus::Denied,
            "cross_domain.invalid_deadline",
            format!(
                "deadline {}ms outside 1..={CROSS_DOMAIN_MAX_DEADLINE_MS}",
                envelope.deadline_ms
            ),
        ));
    }
    if envelope.target.extension_point.chars().count() > MAX_ENVELOPE_POINT_CHARS {
        return Err(CrossDomainDenial::new(
            CrossDomainStatus::Denied,
            "cross_domain.invalid_target",
            "extension point id exceeds bounds",
        ));
    }

    // Requester: enabled, exact version, third-party domain. A package whose
    // approval was revoked and then disabled shows up as not-enabled.
    let requester_record = service
        .enabled_record(&envelope.requester.package, &envelope.requester.version)
        .ok_or_else(|| {
            CrossDomainDenial::new(
                CrossDomainStatus::Stale,
                "cross_domain.requester_stale",
                format!(
                    "requester `{}@{}` is not enabled at that version",
                    envelope.requester.package, envelope.requester.version
                ),
            )
        })?;
    if requester_record.runtime_domain != RuntimeDomain::ThirdParty {
        return Err(CrossDomainDenial::new(
            CrossDomainStatus::Denied,
            "cross_domain.requester_not_third_party",
            "cross-domain requests originate only from the third-party domain",
        ));
    }

    // Approval binding: envelope's approval_ref must name the requester at
    // the exact approved version.
    if envelope.approval_ref
        != format!(
            "{}@{}",
            envelope.requester.package, envelope.requester.version
        )
    {
        return Err(CrossDomainDenial::new(
            CrossDomainStatus::Denied,
            "cross_domain.approval_ref_mismatch",
            "approvalRef does not bind the requester identity",
        ));
    }

    // Target: enabled, declares the exact point/version/operation.
    let target_record = service
        .enabled_records()
        .find(|record| record.manifest.name == envelope.target.package)
        .ok_or_else(|| {
            CrossDomainDenial::new(
                CrossDomainStatus::Denied,
                "cross_domain.target_not_enabled",
                format!("target `{}` is not enabled", envelope.target.package),
            )
        })?;
    let request = crate::packages::extension_points::StructuredRelationRequest {
        package: envelope.target.package.clone(),
        extension_point: envelope.target.extension_point.clone(),
        version: envelope.target.version,
        operation: envelope.operation,
        scopes: envelope.scopes.clone(),
        justification: None,
        relation_key: "cross-domain".to_string(),
    };
    crate::packages::extension_points::verify_relation_request(
        &target_record.manifest.clay.extension_points,
        &request,
    )
    .map_err(|error| {
        CrossDomainDenial::new(
            CrossDomainStatus::Denied,
            error.code(),
            format!("{error:?}"),
        )
    })?;

    // User consent: the durable approval must currently cover this exact
    // edge. Identity drift maps to stale; revocation to revoked; any
    // expansion/absence to denied.
    let installed = service
        .installed_package_for_specifier(&envelope.requester.package)
        .map(|(_, installed)| installed)
        .ok_or_else(|| {
            CrossDomainDenial::new(
                CrossDomainStatus::Stale,
                "cross_domain.requester_stale",
                "requester is no longer installed",
            )
        })?;
    let mut relations = crate::packages::manifest::PackageGraphRelations::default();
    relations.relation_requests.push(request);
    service
        .approval_store()
        .approval_covers(
            &installed.provenance,
            &requester_record.manifest.clay.api_prefix,
            &[],
            &[],
            &relations,
        )
        .map_err(|mismatch| {
            let (status, code) = match &mismatch {
                ApprovalMismatch::Revoked => {
                    (CrossDomainStatus::Revoked, "package_approval.revoked")
                }
                ApprovalMismatch::IdentityChanged { .. } => {
                    (CrossDomainStatus::Stale, mismatch.code())
                }
                _ => (CrossDomainStatus::Denied, mismatch.code()),
            };
            CrossDomainDenial::new(status, code, format!("{mismatch:?}"))
        })?;

    Ok(ValidatedCrossDomainRoute {
        requester: envelope.requester.clone(),
        target: envelope.target.clone(),
        operation: envelope.operation,
        scopes: envelope.scopes.clone(),
        deadline_ms: envelope.deadline_ms,
        payload: envelope.payload.clone(),
    })
}

impl CrossDomainResultEnvelope {
    pub(crate) fn denied(request_id: u64, denial: &CrossDomainDenial) -> Self {
        Self {
            request_id,
            status: denial.status,
            payload: serde_json::json!({
                "code": denial.code,
                "detail": denial.detail,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::packages::authorization::RuntimeProfile;
    use crate::packages::manager::FakeBackend;
    use crate::packages::record::assemble_package_record;

    fn owner_json() -> Value {
        json!({
            "name": "@vendor/base",
            "version": "0.1.0",
            "type": "module",
            "clay": {
                "apiPrefix": "base",
                "entry": "./dist/index.js",
                "permissions": [],
                "modes": [],
                "docs": "./docs/index.md",
                "extensionPoints": [{
                    "id": "base.completionProviders",
                    "version": 1,
                    "operations": ["append"],
                    "contributionKinds": ["completionProvider"]
                }]
            }
        })
    }

    fn requester_json() -> Value {
        json!({
            "name": "@vendor/ext",
            "version": "0.1.0",
            "type": "module",
            "clay": {
                "apiPrefix": "ext",
                "entry": "./dist/index.js",
                "permissions": [],
                "modes": [],
                "docs": "./docs/index.md",
                "extends": [{
                    "package": "@vendor/base",
                    "extensionPoint": "base.completionProviders",
                    "version": 1,
                    "operation": "append",
                    "scopes": ["ext.wikilinks"]
                }]
            }
        })
    }

    fn service_with_pair() -> PackageService {
        let mut service =
            PackageService::new("target/test-cross-domain", Box::<FakeBackend>::default());
        for package_json in [owner_json(), requester_json()] {
            let record = assemble_package_record(&package_json).unwrap();
            service.install_from_value(package_json).unwrap();
            service
                .authorize_package(
                    &record.manifest.name,
                    record.manifest.clay.permissions.clone(),
                    RuntimeProfile::Restricted,
                    "test-user",
                )
                .unwrap();
        }
        service.approve_package("@vendor/base", "test").unwrap();
        service.approve_package("@vendor/ext", "test").unwrap();
        service.enable("@vendor/ext").unwrap();
        service
    }

    fn envelope() -> CrossDomainRequestEnvelope {
        CrossDomainRequestEnvelope {
            request_id: 42,
            requester: CrossDomainRequester {
                package: "@vendor/ext".to_string(),
                version: "0.1.0".to_string(),
                generation: 1,
            },
            target: CrossDomainTarget {
                package: "@vendor/base".to_string(),
                extension_point: "base.completionProviders".to_string(),
                version: 1,
            },
            operation: RelationOperation::Append,
            scopes: vec!["ext.wikilinks".to_string()],
            approval_ref: "@vendor/ext@0.1.0".to_string(),
            deadline_ms: 250,
            payload: json!({ "items": [] }),
        }
    }

    #[test]
    fn exact_approved_request_validates() {
        let service = service_with_pair();
        let route = validate_cross_domain_request(&service, &envelope()).expect("validates");
        assert_eq!(route.target.extension_point, "base.completionProviders");
        assert_eq!(route.requester.package, "@vendor/ext");
    }

    #[test]
    fn wrong_target_point_and_operation_denied() {
        let service = service_with_pair();
        let mut bad_point = envelope();
        bad_point.target.extension_point = "base.other".to_string();
        let denial = validate_cross_domain_request(&service, &bad_point).unwrap_err();
        assert_eq!(denial.status, CrossDomainStatus::Denied);
        assert_eq!(denial.code, "package_relation.unknown_extension_point");

        let mut bad_op = envelope();
        bad_op.operation = RelationOperation::Replace;
        let denial = validate_cross_domain_request(&service, &bad_op).unwrap_err();
        assert_eq!(denial.code, "package_relation.operation_not_offered");

        let mut bad_version = envelope();
        bad_version.target.version = 2;
        let denial = validate_cross_domain_request(&service, &bad_version).unwrap_err();
        assert_eq!(denial.code, "package_relation.version_mismatch");
    }

    #[test]
    fn stale_requester_and_revoked_approval_rejected() {
        let mut service = service_with_pair();
        let mut stale = envelope();
        stale.requester.version = "9.9.9".to_string();
        stale.approval_ref = "@vendor/ext@9.9.9".to_string();
        let denial = validate_cross_domain_request(&service, &stale).unwrap_err();
        assert_eq!(denial.status, CrossDomainStatus::Stale);

        service.revoke_package_approval("@vendor/ext").unwrap();
        let denial = validate_cross_domain_request(&service, &envelope()).unwrap_err();
        assert_eq!(denial.status, CrossDomainStatus::Revoked);
    }

    #[test]
    fn oversize_payload_and_bad_deadline_denied_before_target_lookup() {
        let service = service_with_pair();
        let mut oversize = envelope();
        oversize.payload = json!({ "blob": "x".repeat(CROSS_DOMAIN_PAYLOAD_BUDGET_BYTES) });
        oversize.target.extension_point = "nonexistent.point".to_string();
        let denial = validate_cross_domain_request(&service, &oversize).unwrap_err();
        assert_eq!(denial.code, "cross_domain.payload_too_large");

        let mut bad_deadline = envelope();
        bad_deadline.deadline_ms = 0;
        let denial = validate_cross_domain_request(&service, &bad_deadline).unwrap_err();
        assert_eq!(denial.code, "cross_domain.invalid_deadline");
    }

    #[test]
    fn mismatched_result_provenance_denied() {
        let service = service_with_pair();
        let mut forged = envelope();
        forged.approval_ref = "@vendor/other@0.1.0".to_string();
        let denial = validate_cross_domain_request(&service, &forged).unwrap_err();
        assert_eq!(denial.code, "cross_domain.approval_ref_mismatch");
    }

    #[test]
    fn enabled_requester_with_expanded_scope_denied() {
        let service = service_with_pair();
        let mut expanded = envelope();
        expanded.scopes.push("ext.other".to_string());
        let denial = validate_cross_domain_request(&service, &expanded).unwrap_err();
        assert_eq!(denial.code, "package_approval.relation_expansion");
    }
}
