// Plan 061 task 14: verify third-party facade allowlist matches the plan
// inventory and internal runtime/approval identifiers are not public.

use std::collections::BTreeSet;
use std::fs;

fn repository_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Parse the `THIRD_PARTY_FACADES` compile-time array from the JS runtime
/// source. The constant is an `&[&str]` of `"clay:*"` specifiers.
fn parse_third_party_facades() -> BTreeSet<String> {
    let source = fs::read_to_string(repository_root().join("src/server/js_runtime.rs"))
        .expect("read js_runtime.rs");
    // The constant declaration:
    //   const THIRD_PARTY_FACADES: &[&str] = &[
    //        "clay:sdui",
    //        ...
    //   ];
    let body = source
        .split_once("const THIRD_PARTY_FACADES: &[&str] = &[")
        .and_then(|(_, remaining)| remaining.split_once("];"))
        .map(|(body, _)| body)
        .expect("find THIRD_PARTY_FACADES in js_runtime.rs");
    body.lines()
        .map(str::trim)
        .filter_map(|line| {
            line.trim_start_matches('"')
                .split_once('"')
                .map(|(specifier, _)| specifier.to_string())
        })
        .filter(|specifier| specifier.starts_with("clay:"))
        .collect()
}

/// Parse the Plan 061 public-third-party facade list from its inventory
/// marker section.
fn parse_plan_public_third_party_facades() -> BTreeSet<String> {
    let plan = fs::read_to_string(
        repository_root()
            .join("plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md"),
    )
    .expect("read Plan 061");
    let section = plan
        .split_once("<!-- plan061-task1-facade-inventory:start -->")
        .and_then(|(_, remaining)| {
            remaining.split_once("<!-- plan061-task1-facade-inventory:end -->")
        })
        .map(|(section, _)| section)
        .expect("find plan facade-inventory section");
    // Extract every `clay:*` specifier from the public-third-party row.
    let mut facades = BTreeSet::new();
    for line in section.lines() {
        if !line.contains("Public-third-party") {
            continue;
        }
        let mut remaining = line;
        while let Some(start) = remaining.find('`') {
            remaining = &remaining[start + 1..];
            let Some(end) = remaining.find('`') else {
                break;
            };
            let specifier = &remaining[..end];
            if specifier.starts_with("clay:") {
                facades.insert(specifier.to_string());
            }
            remaining = &remaining[end + 1..];
        }
    }
    facades
}

#[test]
fn third_party_facade_allowlist_exactly_matches_plan_public_inventory() {
    let code = parse_third_party_facades();
    let plan = parse_plan_public_third_party_facades();
    assert_eq!(code.len(), 13, "third-party facade count must be 13");
    assert_eq!(
        code, plan,
        "THIRD_PARTY_FACADES must exactly match the plan's Public-third-party classification"
    );
}

/// Regression: internal types that handle approval storage, cross-domain
/// routing, package context, and domain classification must remain
/// `pub(crate)` or private — never `pub`.
#[test]
fn internal_trust_domain_types_are_not_public() {
    let root = repository_root();
    // For each internal type, scan the defining file for a `pub struct` or
    // `pub enum` line that would expose it publicly.  `pub(crate)` is
    // acceptable; bare `pub ` is a violation.
    let checks: &[(&str, &str)] = &[
        ("src/packages/approvals.rs", "PackageApprovalStore"),
        ("src/packages/approvals.rs", "ApprovalMismatch"),
        ("src/packages/approvals.rs", "ApprovalStoreError"),
        ("src/packages/bundled.rs", "BundledTrustError"),
        ("src/server/cross_domain.rs", "CrossDomainRequestEnvelope"),
        ("src/server/ops/mod.rs", "PackageContext"),
        (
            "src/packages/extension_points.rs",
            "RelationVerificationError",
        ),
    ];
    for (path, type_name) in checks {
        let source =
            fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let public_pattern = format!("pub struct {type_name}");
        let public_enum_pat = format!("pub enum {type_name}");
        if source.contains(&public_pattern) || source.contains(&public_enum_pat) {
            // Allow pub struct if the doc states it is part of the public
            // package-authoring manifest (ExtensionContributionKind,
            // ExtensionPointDeclaration, StructuredRelationRequest,
            // RelationOperation) or the adoption record model
            // (PackageApprovalRecord, ApprovedRelation, ApprovedReplacement).
            let allowed_public = [
                "ExtensionPointDeclaration",
                "StructuredRelationRequest",
                "ExtensionContributionKind",
                "RelationOperation",
                "PackageApprovalRecord",
                "ApprovedRelation",
                "ApprovedReplacement",
            ];
            if !allowed_public.contains(type_name) {
                panic!(
                    "{type_name} in {path} must be pub(crate) or private, \
                     found bare `pub`"
                );
            }
        }
    }
}
