// Plan 061 task 14: verify third-party facade allowlist matches the plan
// inventory and internal runtime/approval identifiers are not public.

use std::collections::BTreeSet;
use std::fs;

mod common;
use common::read_src;

fn repository_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Parse public third-party facade rows from the single runtime facade table.
fn parse_third_party_facades() -> BTreeSet<String> {
    let source = fs::read_to_string(repository_root().join("src/server/facades.rs"))
        .expect("read facades.rs");
    source
        .split("Facade::public(")
        .skip(1)
        .filter_map(|row| row.split('\"').nth(1).map(str::to_string))
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
    assert_eq!(code.len(), 14, "third-party facade count must be 14");
    assert_eq!(
        code, plan,
        "THIRD_PARTY_FACADES must exactly match the plan's Public-third-party classification"
    );
}

/// Regression: trust-domain, routing, lifecycle, queue, filesystem, and
/// scheduler mechanics are host implementation details, never bare-public Rust
/// API. `pub(crate)` remains acceptable for cross-module orchestration.
#[test]
fn internal_runtime_mechanics_are_not_public() {
    let root = repository_root();
    let declarations: &[(&str, &str)] = &[
        (
            "src/packages/approvals.rs",
            "pub struct PackageApprovalStore",
        ),
        ("src/packages/approvals.rs", "pub enum ApprovalMismatch"),
        ("src/packages/approvals.rs", "pub enum ApprovalStoreError"),
        ("src/packages/bundled.rs", "pub enum RuntimeDomain"),
        ("src/packages/bundled.rs", "pub enum BundledTrustError"),
        (
            "src/server/cross_domain.rs",
            "pub struct CrossDomainRequestEnvelope",
        ),
        ("src/server/ops/mod.rs", "pub struct PackageContext"),
        ("src/server/output_router.rs", "pub struct OutputRouter"),
        ("src/server/workspace.rs", "pub struct TargetIdentity"),
        ("src/server/workspace.rs", "pub struct DirectoryListingPlan"),
        (
            "src/server/workspace.rs",
            "pub struct ListingCancellationGuard",
        ),
        ("src/server/workspace.rs", "pub struct CloseDocumentOutcome"),
        (
            "src/server/connection/mod.rs",
            "pub struct RuntimeDiagnosticStore",
        ),
        (
            "src/server/connection/mod.rs",
            "pub struct ConnectionOutputSubscriptions",
        ),
        (
            "src/server/document_analysis.rs",
            "pub struct AnalysisOutputSink",
        ),
        (
            "src/packages/extension_points.rs",
            "pub enum RelationVerificationError",
        ),
    ];
    for (path, declaration) in declarations {
        let source = read_src(path);
        assert!(
            !source.contains(declaration),
            "{declaration} in {path} must be pub(crate) or private"
        );
    }

    let internal_functions: &[(&str, &str)] = &[
        ("src/client/mod.rs", "pub fn enqueue_close_document"),
        ("src/server/completion.rs", "pub fn remove_document"),
        (
            "src/server/language_intelligence.rs",
            "pub fn remove_document",
        ),
        ("src/server/parse_coordinator.rs", "pub fn subscribe_client"),
        (
            "src/server/parse_coordinator.rs",
            "pub fn subscribe_document",
        ),
        (
            "src/server/parse_coordinator.rs",
            "pub fn unsubscribe_document",
        ),
        (
            "src/server/parse_coordinator.rs",
            "pub fn unsubscribe_client",
        ),
        ("src/server/parse_coordinator.rs", "pub fn remove_document"),
    ];
    for (path, declaration) in internal_functions {
        let source = read_src(path);
        assert!(
            !source.contains(declaration),
            "{declaration} in {path} must be pub(crate) or private"
        );
    }

    let budgets = fs::read_to_string(root.join("src/perf/budgets.rs")).expect("read budgets.rs");
    for name in [
        "LANGUAGE_SERVER_SESSION_COMMAND_CAPACITY",
        "DIRECTORY_LISTING_MAX_CONCURRENCY",
        "GIT_ROOT_CONCURRENCY",
        "MAX_DOCUMENTS_PER_CLIENT",
        "MAX_ACTIVE_CONNECTIONS",
        "MAX_SERVER_DOCUMENTS",
        "CONNECTION_RESULT_LANE_CAPACITY",
        "RUNTIME_DIAGNOSTIC_CAPACITY",
        "MAX_AUXILIARY_READ_BYTES",
        "MAX_GITIGNORE_LINES",
        "MAX_GITIGNORE_PATTERNS",
        "MAX_GITIGNORE_PATTERN_CHARS",
    ] {
        assert!(
            !budgets.contains(&format!("pub const {name}")),
            "internal compiled budget {name} must not be bare-public"
        );
    }
}

#[test]
fn internal_runtime_names_are_absent_from_public_facades() {
    let root = repository_root();
    let internal_names = [
        "RuntimeDomain",
        "PackageContext",
        "OutputRouter",
        "TargetIdentity",
        "DirectoryListingPlan",
        "ListingCancellationGuard",
        "CloseDocumentOutcome",
        "RuntimeDiagnosticStore",
        "ConnectionOutputSubscriptions",
        "AnalysisOutputSink",
        "LANGUAGE_SERVER_SESSION_COMMAND_CAPACITY",
        "MAX_ACTIVE_CONNECTIONS",
        "serverCloseDocument",
    ];
    for entry in fs::read_dir(root.join("runtime/js")).expect("read runtime/js") {
        let path = entry.expect("runtime/js entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "ts")
        ) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read facade source");
        for name in internal_names {
            assert!(
                !source.contains(name),
                "{} must not expose internal runtime name {name}",
                path.display()
            );
        }
    }
}

/// Plan 079 task 10: per-tab server state and routing are implementation
/// details. Public workspace/document/tab behavior keeps using the existing
/// curated Clay JS APIs; no JS surface may accept an arbitrary tab-state
/// handle or bypass the connection's bound-tab authority.
#[test]
fn phase22_8_per_tab_state_has_no_new_public_programmatic_surface() {
    let root = repository_root();
    let server = fs::read_to_string(root.join("src/server/mod.rs")).expect("read server/mod.rs");
    for method in [
        "create_tab_state",
        "ensure_tab_state",
        "tab_state",
        "tab_state_for_client",
        "unbound_bootstrap_state",
        "state_for_client",
        "remove_tab_state",
    ] {
        assert!(
            server.contains(&format!("pub(crate) async fn {method}")),
            "IpcServer::{method} must stay pub(crate)"
        );
        assert!(
            !server.contains(&format!("pub async fn {method}")),
            "IpcServer::{method} must not become a bare-public API"
        );
    }
    assert!(
        server.contains("pub(crate) struct TabServerState"),
        "TabServerState must stay server-internal"
    );
    for method in ["workspace_pane_visible", "toggle_workspace_pane"] {
        assert!(
            server.contains(&format!("pub(crate) fn {method}")),
            "TabServerState::{method} must stay pub(crate)"
        );
    }

    let connection_mod = fs::read_to_string(root.join("src/server/connection/mod.rs"))
        .expect("read connection/mod.rs");
    let documents = fs::read_to_string(root.join("src/server/connection/documents.rs"))
        .expect("read connection/documents.rs");
    let workspace = fs::read_to_string(root.join("src/server/connection/workspace.rs"))
        .expect("read connection/workspace.rs");
    for (file, function) in [
        (&connection_mod, "route_connection_tab_state"),
        (&connection_mod, "message_requires_tab_state"),
        (&documents, "document_for_message"),
        (&workspace, "workspace_command_result_message"),
    ] {
        let private = file.contains(&format!("async fn {function}"))
            || file.contains(&format!("fn {function}"))
            || file.contains(&format!("pub(super) async fn {function}"));
        assert!(private, "connection::{function} must remain crate-private");
        assert!(
            !file.contains(&format!("pub async fn {function}"))
                && !file.contains(&format!("pub fn {function}")),
            "connection::{function} must not become a public API"
        );
    }

    let registry = fs::read_to_string(root.join("src/server/tab_registry.rs"))
        .expect("read server/tab_registry.rs");
    for method in [
        "entry",
        "tab_for_client",
        "create_tab",
        "open_workspace",
        "reclaim",
        "sweep_expired",
    ] {
        assert!(
            registry.contains(&format!("pub(crate) fn {method}")),
            "TabRegistry::{method} must stay pub(crate)"
        );
        assert!(
            !registry.contains(&format!("pub fn {method}")),
            "TabRegistry::{method} must not become a public API"
        );
    }

    let workspace =
        fs::read_to_string(root.join("src/server/workspace.rs")).expect("read server/workspace.rs");
    assert!(
        workspace.contains("pub(crate) fn with_document_id_allocator"),
        "WorkspaceState allocator plumbing must stay pub(crate)"
    );
    assert!(
        !workspace.contains("pub fn with_document_id_allocator"),
        "WorkspaceState allocator plumbing must not become a public API"
    );

    let facades = fs::read_dir(root.join("runtime/js"))
        .expect("read runtime/js")
        .filter_map(Result::ok)
        .filter(|entry| {
            matches!(
                entry.path().extension().and_then(|value| value.to_str()),
                Some("js" | "ts")
            )
        })
        .map(|entry| fs::read_to_string(entry.path()).expect("read facade"))
        .collect::<Vec<_>>()
        .join("\n");
    for internal_name in [
        "createTabState",
        "ensureTabState",
        "tabStateForClient",
        "serverSelectTabWorkspace",
        "serverOpenDocumentInTab",
        "serverListDocumentsForTab",
        "serverToggleFileBrowser",
    ] {
        assert!(
            !facades.contains(internal_name),
            "runtime JS must not expose per-tab internal name {internal_name}"
        );
    }
}
