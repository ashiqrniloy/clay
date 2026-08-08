// Plan 061 task 14: verify third-party facade allowlist matches the plan
// inventory and internal runtime/approval identifiers are not public.

use std::collections::BTreeSet;
use std::fs;

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
    assert_eq!(code.len(), 13, "third-party facade count must be 13");
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
            "src/server/connection.rs",
            "pub struct RuntimeDiagnosticStore",
        ),
        (
            "src/server/connection.rs",
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
        let source =
            fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"));
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
        let source =
            fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"));
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

/// Plan 063 task 8: verify Phase 20.2 native chrome primitives are not exposed
/// to JavaScript. Primitives are `pub(crate)` inert paint helpers, not
/// programmatic behavior. They must not be wrapped by `deno_core` ops or
/// exposed through Clay JS facades.
#[test]
fn phase20_2_primitives_are_not_exposed_to_javascript() {
    let root = repository_root();
    let primitives_source =
        fs::read_to_string(root.join("src/shell/primitives.rs")).expect("read primitives.rs");

    // Assert all primitive functions are `pub(crate)`, not bare `pub`.
    let primitive_functions = [
        "paint_divider",
        "paint_focus_ring",
        "paint_panel_chrome",
        "paint_scroll_chrome",
        "paint_badge",
        "paint_kbd_hint",
        "paint_icon_slot",
        "paint_tooltip_shell",
    ];
    for func in primitive_functions {
        let pub_crate_marker = format!("pub(crate) fn {func}");
        let pub_marker = format!("pub fn {func}");
        assert!(
            primitives_source.contains(&pub_crate_marker),
            "primitive {func} must be pub(crate), not bare pub or private"
        );
        assert!(
            !primitives_source.contains(&pub_marker),
            "primitive {func} must not be bare pub (exposed to JS)"
        );
    }

    // Assert no primitive is wrapped by a deno_core op.
    let ops_source =
        fs::read_to_string(root.join("src/server/ops/ui.rs")).expect("read src/server/ops/ui.rs");
    for func in primitive_functions {
        assert!(
            !ops_source.contains(func),
            "primitive {func} must not be wrapped by a deno_core op in src/server/ops/ui.rs"
        );
    }

    // Assert no primitive is exposed through Clay JS facades.
    for entry in fs::read_dir(root.join("runtime/js")).expect("read runtime/js") {
        let path = entry.expect("runtime/js entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "ts")
        ) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read facade source");
        for func in primitive_functions {
            assert!(
                !source.contains(func),
                "{} must not expose primitive {func} to JavaScript",
                path.display()
            );
        }
    }
}

#[test]
fn phase20_4_introduces_no_unexposed_public_rust_function() {
    // Plan 065 task 10: Phase 20.4 is pub(crate) paint/interaction work. Every
    // new helper must be pub(crate) or private — none may be a bare `pub fn`
    // that becomes a public JS surface, and none may be wrapped by a deno_core
    // op or exposed through a runtime/js facade.
    let root = repository_root();

    // (source_file, [function names that must be pub(crate) or private])
    let new_helpers: &[(&str, &[&str])] = &[
        (
            "src/shell/primitives.rs",
            &[
                "component_state_color",
                "list_row_fill_color",
                "disabled_text_color",
            ],
        ),
        ("src/shell/theme.rs", &["from_ui_theme", "typography"]),
        (
            "src/masonry_sdui.rs",
            // Plan 070 step 13e deleted the legacy immediate-mode interaction
            // model (`set_focused_action`/`focused_action`/`is_focused`/
            // `interaction_state`/`set_pointer_pos`/`set_pointer_pressed`/
            // `clear_pointer_state`) — interactive + pointer state now lives in
            // the retained host widgets, which derive it from Masonry's ctx.
            &["theme_style"],
        ),
        (
            "src/editor/surface.rs",
            &[
                "ui_theme",
                "set_pointer_pos",
                "set_pointer_pressed",
                "clear_pointer_chrome_state",
                "scrollbar_interaction_state",
            ],
        ),
    ];

    // Each new helper must appear as `pub(crate) fn` (or a private `fn`) and
    // must NOT appear as a bare `pub fn`.
    for (file, funcs) in new_helpers {
        let source = fs::read_to_string(root.join(file))
            .unwrap_or_else(|error| panic!("read {file}: {error}"));
        for func in *funcs {
            let pub_crate_marker = format!("pub(crate) fn {func}");
            let private_marker = format!("    fn {func}");
            let pub_marker = format!("pub fn {func}");
            assert!(
                source.contains(&pub_crate_marker) || source.contains(&private_marker),
                "{file}::{func} must be pub(crate) or private, not bare pub"
            );
            assert!(
                !source.contains(&pub_marker),
                "{file}::{func} must not be bare pub (would expose to JS)"
            );
        }
    }

    // No new helper may be wrapped by a deno_core op. Only check the
    // Rust-internal snake_case names; generic words like `typography`/`ui_theme`
    // appear legitimately in facade prose and are not Rust helper exports.
    let facade_ops_funcs = [
        "component_state_color",
        "list_row_fill_color",
        "disabled_text_color",
        "from_ui_theme",
        "theme_style",
        "set_pointer_pos",
        "set_pointer_pressed",
        "clear_pointer_state",
        "set_focused_action",
        "focused_action",
        "is_focused",
        "interaction_state",
        "clear_pointer_chrome_state",
        "scrollbar_interaction_state",
    ];
    let ops_text =
        fs::read_to_string(root.join("src/server/ops/ui.rs")).expect("read src/server/ops/ui.rs");
    for func in facade_ops_funcs {
        assert!(
            !ops_text.contains(func),
            "{func} must not be wrapped by a deno_core op in src/server/ops/ui.rs"
        );
    }

    // No new helper may be exposed through a Clay JS facade.
    for entry in fs::read_dir(root.join("runtime/js")).expect("read runtime/js") {
        let path = entry.expect("runtime/js entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "ts")
        ) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read facade source");
        for func in facade_ops_funcs {
            assert!(
                !source.contains(func),
                "{} must not expose {func} to JavaScript",
                path.display()
            );
        }
    }
}

/// Plan 077 task 9: Phase 22.6 window-model accessibility additions are not
/// public programmatic capabilities. The a11y metadata (announcement builder,
/// pane counts/display names, per-pane chrome geometry proxies, window budget
/// constants) is internal chrome state — none of it is a Clay JS API, and none
/// of it may be wrapped by a deno_core op or exposed through a runtime/js
/// facade. The shell methods the driver bin crate calls (`announce`,
/// `announce_tab_activated`, `announce_tab_created`, `set_pane_document_name`,
/// `metadata_path`) are bare `pub` by the established widget-method convention
/// (the bin crate cannot see `pub(crate)` lib items; `set_active_tab`/
/// `remove_tab`/`mount_tab` follow the same rule), but they are Masonry widget
/// methods, never server-side capabilities.
#[test]
fn phase22_6_window_model_a11y_additions_are_not_public_programmatic_surfaces() {
    let root = repository_root();

    // Internal helpers must be pub(crate) or private, never bare `pub`.
    let internal_items: &[(&str, &str)] = &[
        ("src/masonry_shell.rs", "pub(crate) enum AnnouncementKind"),
        (
            "src/masonry_shell.rs",
            "pub(crate) const ANNOUNCEMENT_MAX_CHARS",
        ),
        ("src/masonry_shell.rs", "pub(crate) fn compose_announcement"),
        ("src/masonry_shell.rs", "    fn announce_pane_change"),
        ("src/masonry_pane_host.rs", "pub(crate) fn with_pane_count"),
        ("src/masonry_pane_host.rs", "pub(crate) fn set_pane_count"),
        (
            "src/masonry_pane_host.rs",
            "pub(crate) fn set_document_display_name",
        ),
        (
            "src/perf/baselines.rs",
            "pub(crate) fn pane_split_tree_with",
        ),
        (
            "src/perf/baselines.rs",
            "pub(crate) fn working_area_layout_with",
        ),
    ];
    for (path, declaration) in internal_items {
        let source =
            fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"));
        assert!(
            source.contains(declaration),
            "{declaration} in {path} must be pub(crate) or private"
        );
    }

    // None of the Phase 22.6 additions may be wrapped by a deno_core op.
    let ops_names = [
        "announce",
        "announce_tab_activated",
        "announce_tab_created",
        "set_pane_document_name",
        "set_pane_count",
        "set_document_display_name",
        "with_pane_count",
        "metadata_path",
        "compose_announcement",
        "AnnouncementKind",
        "pane_chrome_piece_count",
        "tab_switch_geometry_work",
        "pane_split_tree_with",
        "working_area_layout_with",
        "PANE_PAINT_P95_BUDGET_MS",
        "TAB_SWITCH_P95_BUDGET_MS",
        "MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES",
    ];
    for entry in fs::read_dir(root.join("src/server/ops")).expect("read src/server/ops") {
        let path = entry.expect("src/server/ops entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read ops source");
        for name in ops_names {
            assert!(
                !source.contains(name),
                "{} must not wrap Phase 22.6 internal {name} in a deno_core op",
                path.display()
            );
        }
    }

    // None of the additions may be exposed through a Clay JS facade.
    for entry in fs::read_dir(root.join("runtime/js")).expect("read runtime/js") {
        let path = entry.expect("runtime/js entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "ts")
        ) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read facade source");
        for name in ops_names {
            assert!(
                !source.contains(name),
                "{} must not expose Phase 22.6 internal {name} to JavaScript",
                path.display()
            );
        }
    }
}
