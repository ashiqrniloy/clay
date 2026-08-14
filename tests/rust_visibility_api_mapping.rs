// Plan 061 task 14: verify third-party facade allowlist matches the plan
// inventory and internal runtime/approval identifiers are not public.

use std::collections::BTreeSet;
use std::fs;

fn repository_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn non_test_body(src: &str) -> &str {
    if let Some(index) = src.find("\nmod tests") {
        return &src[..index];
    }
    if let Some(index) = src.find("\n#[cfg(test)]") {
        return &src[..index];
    }
    src
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
        "paint_scrim",
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

/// Plan 084 task 9: Phase 24.4 adds no public programmatic surface. The
/// centered Command Centre is presentation/accessibility work: `paint_scrim`
/// stays `pub(crate)` (pinned by `phase20_2_primitives_are_not_exposed_to_javascript`),
/// the centered anchor/geometry helpers stay `pub(crate)`, and the only new
/// bare `pub` is the `#[doc(hidden)]` `EditorWidget::reconcile_centered_overlay_layer`
/// bridge the driver bin crate needs to mount/remove its window-level layer.
/// Nothing from the centered surface may be wrapped by a deno_core op or
/// appear in a Clay JS facade, and the generated JS API registry must not gain
/// any Phase 24.4 entry (no `setCommandCenterPosition`, no scrim API).
#[test]
fn phase24_5_command_centre_sessions_are_not_a_package_programmatic_surface() {
    // Phase 24.5 authority review: the built-in browse grant (path-mode
    // traversal outside workspace roots) is reachable only from the
    // user-driven built-in path-mode surface. The session-opening helper is
    // defined in the connection layer and has exactly two call sites — the
    // client CommandIntent special case for the two builtin ids and menu
    // activation of the built-in openPath id — both fed by user-driven
    // client messages. Package JavaScript runs in the op layer, which never
    // calls the helper; the package executeCommand facade validates and
    // acknowledges without opening a session; package registerCommand cannot
    // claim either id (tests/command_execution.rs
    // control_center_command_ids_are_not_registerable_by_packages).
    let root = repository_root();
    let connection =
        fs::read_to_string(root.join("src/server/connection.rs")).expect("connection readable");
    assert_eq!(
        connection.matches("open_command_centre_session(").count(),
        3,
        "definition + exactly two call sites (CommandIntent, menu activation)"
    );

    for file in [
        "src/server/ops/mod.rs",
        "src/server/ops/commands.rs",
        "src/packages/commands.rs",
        "src/packages/service.rs",
        "src/packages/modes.rs",
        "src/server/control_center.rs",
    ] {
        let src = fs::read_to_string(root.join(file))
            .unwrap_or_else(|error| panic!("read {file}: {error}"));
        let body = non_test_body(&src);
        for forbidden in [
            "open_command_centre_session",
            "PathBrowserSession",
            "UserBrowseListingPlan",
            "execute_user_browse_listing",
        ] {
            assert!(
                !body.contains(forbidden),
                "{file} must not open command centre/browse sessions: {forbidden}"
            );
        }
    }
    // Session state legitimately flows through the menu-session store, but
    // opening a browse session must stay in the connection layer.
    let sessions =
        fs::read_to_string(root.join("src/server/menu_sessions.rs")).expect("sessions readable");
    assert!(
        !non_test_body(&sessions).contains("open_command_centre_session"),
        "menu_sessions.rs must not open command centre/browse sessions"
    );
}

#[test]
fn phase24_4_centered_surface_is_not_a_public_programmatic_surface() {
    let root = repository_root();

    // The reconcile bridge joins the established `#[doc(hidden)] pub fn`
    // widget-method allowlist (the bin crate calls shell widget methods), so
    // any new bare-pub helper outside the allowlist fails this pin.
    let editor =
        fs::read_to_string(root.join("src/masonry_editor.rs")).expect("read src/masonry_editor.rs");
    assert!(
        editor.contains("#[doc(hidden)]\n    pub fn reconcile_centered_overlay_layer"),
        "masonry_editor.rs must keep the doc(hidden) reconcile bridge"
    );
    let mut doc_hidden_pub_fns: Vec<String> = std::fs::read_dir(root.join("src"))
        .expect("read src")
        .filter_map(|entry| entry.ok())
        .flat_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                return Vec::new();
            }
            fs::read_to_string(&path)
                .expect("read source file")
                .split("#[doc(hidden)]")
                .skip(1)
                .filter(|tail| tail.trim_start().starts_with("pub fn"))
                .map(|_| format!("{}::pub fn", path.display()))
                .collect()
        })
        .collect();
    doc_hidden_pub_fns.sort();
    assert_eq!(
        doc_hidden_pub_fns,
        vec![
            format!("{}::pub fn", root.join("src/masonry_editor.rs").display()),
            format!("{}::pub fn", root.join("src/masonry_shell.rs").display()),
            format!("{}::pub fn", root.join("src/masonry_shell.rs").display()),
        ],
        "doc(hidden) pub fn allowlist: reconcile bridge + shell widget methods only"
    );

    // No Phase 24.4 implementation name may be wrapped by a deno_core op.
    for ops in ["src/server/ops/ui.rs", "src/server/ops/theme.rs"] {
        let ops_source = fs::read_to_string(root.join(ops)).expect("read ops source");
        for name in [
            "reconcile_centered_overlay_layer",
            "paint_scrim",
            "PackageOverlayAnchor::Centered",
        ] {
            assert!(
                !ops_source.contains(name),
                "{ops} must not wrap {name} in a deno_core op"
            );
        }
    }

    // No Phase 24.4 implementation name may appear in a Clay JS facade.
    for entry in fs::read_dir(root.join("runtime/js")).expect("read runtime/js") {
        let path = entry.expect("runtime/js entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "ts")
        ) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read facade source");
        for name in [
            "reconcile_centered_overlay_layer",
            "paint_scrim",
            "setCommandCenterPosition",
        ] {
            assert!(
                !source.contains(name),
                "{} must not expose {name} to JavaScript",
                path.display()
            );
        }
    }

    // The generated registry must not gain a Phase 24.4 API entry.
    let registry = fs::read_to_string(root.join("docs/generated/clay-js-api-registry.json"))
        .expect("read generated registry");
    assert!(
        !registry.contains("setCommandCenterPosition") && !registry.contains("scrim"),
        "generated registry must not contain a Phase 24.4 API entry"
    );
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

    let connection = fs::read_to_string(root.join("src/server/connection.rs"))
        .expect("read server/connection.rs");
    for function in [
        "route_connection_tab_state",
        "message_requires_tab_state",
        "document_for_message",
        "workspace_command_result_message",
    ] {
        assert!(
            connection.contains(&format!("async fn {function}"))
                || connection.contains(&format!("fn {function}")),
            "connection::{function} must remain private"
        );
        assert!(
            !connection.contains(&format!("pub async fn {function}"))
                && !connection.contains(&format!("pub fn {function}")),
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

/// Plan 085 task 10: Phase 24.5 keybinding internals stay crate-private.
/// The multi-stroke extension is a chord-string FORMAT change to the existing
/// bindKey/unbindKey APIs — no new op, facade function, command id, or public
/// Rust capability. The sequence matcher (route_key_sequence/ChordRouteOutcome),
/// the pending-chord input-routing state (PendingChord), the sequence parser
/// (parse_key_sequence/key_sequence_string), and the prefix validator
/// (is_strict_prefix) are internal router/parser machinery: pub(crate) or
/// private, absent from runtime/js facades, and never wrapped in a deno_core
/// op. The generated registry stores API shape only — it contains no default
/// keybinding metadata, so the chord defaults (controlCenter.open =
/// Ctrl+X Ctrl+P, controlCenter.openPath = Ctrl+X Ctrl+F) cannot go stale
/// there; their documentation lives in bind-key.md (updated by task 9) and
/// the protocol default_keymaps (task 6), pinned by
/// default_keymaps_are_prefix_collision_free.
#[test]
fn phase24_5_keybinding_internals_stay_crate_private() {
    let root = repository_root();

    // (1) New router/parser internals are pub(crate) or private — never `pub fn`.
    let cases = [
        (
            "src/client/behavior.rs",
            "pub(crate) fn route_key_sequence(",
            "route_key_sequence must stay pub(crate): it is the pure sequence matcher, not a public capability",
        ),
        (
            "src/client/behavior.rs",
            "pub(crate) enum ChordRouteOutcome {",
            "ChordRouteOutcome must stay pub(crate)",
        ),
        (
            "src/editor/surface.rs",
            "pub(crate) struct PendingChord {",
            "PendingChord must stay pub(crate): it is internal input-routing state",
        ),
        (
            "src/server/ops/keybindings.rs",
            "fn parse_key_sequence(chord: &str) -> Result<Vec<KeyStroke>, JsErrorBox> {",
            "parse_key_sequence must stay private to the ops module (configuration path only)",
        ),
        (
            "src/server/ops/keybindings.rs",
            "pub(super) fn key_sequence_string(sequence: &[KeyStroke]) -> String {",
            "key_sequence_string must stay pub(super)",
        ),
        (
            "src/behavior/manifest.rs",
            "fn is_strict_prefix(a: &[KeyStroke], b: &[KeyStroke]) -> bool {",
            "is_strict_prefix must stay private to the manifest validator",
        ),
    ];
    for (file, needle, message) in cases {
        let src = fs::read_to_string(root.join(file)).expect("read {file}");
        assert!(
            non_test_body(&src).contains(needle),
            "{message} (missing `{needle}` in {file})"
        );
    }

    // (2) No new public Rust capability: the parser module exposes no `pub fn`
    // at all, and no new op is registered.
    let keybindings_ops = fs::read_to_string(root.join("src/server/ops/keybindings.rs"))
        .expect("read keybindings ops");
    assert_eq!(
        keybindings_ops.matches("pub fn ").count(),
        0,
        "keybindings ops module must not add public Rust functions"
    );
    let ops_mod = fs::read_to_string(root.join("src/server/ops/mod.rs")).expect("read ops mod");
    for op in [
        "op_clay_keybindings_bind_key,",
        "op_clay_keybindings_bind_keys,",
        "op_clay_keybindings_unbind_key,",
        "op_clay_keybindings_unbind_keys,",
        "op_clay_keybindings_list_key_bindings,",
    ] {
        assert_eq!(
            ops_mod.matches(op).count(),
            2,
            "op {op} must be imported and registered exactly once (no new keybinding ops)"
        );
    }

    // (3) The runtime/js facade stays the complete public keybinding surface:
    // exactly the three pre-existing functions, referencing only the five
    // pre-existing ops, with no internal routing symbols.
    let facade = fs::read_to_string(root.join("runtime/js/keybindings.js")).expect("read facade");
    for export in [
        "export function bindKey(",
        "export function unbindKey(",
        "export function listKeyBindings(",
    ] {
        assert!(facade.contains(export), "facade must keep {export}");
    }
    for internal_name in [
        "PendingChord",
        "pending_chord",
        "route_key_sequence",
        "parse_key_sequence",
    ] {
        assert!(
            !facade.contains(internal_name),
            "runtime JS must not expose keybinding internal {internal_name}"
        );
    }
    for op in [
        "op_clay_keybindings_bind_key",
        "op_clay_keybindings_bind_keys",
        "op_clay_keybindings_unbind_key",
        "op_clay_keybindings_unbind_keys",
        "op_clay_keybindings_list_key_bindings",
    ] {
        assert!(
            facade.contains(op),
            "facade must reference the pre-existing op {op}"
        );
    }
    // No new op names leak into the facade beyond the pre-existing five.
    for line in facade.lines().filter(|l| l.contains("op_clay_keybindings")) {
        let known = [
            "op_clay_keybindings_bind_key",
            "op_clay_keybindings_bind_keys",
            "op_clay_keybindings_unbind_key",
            "op_clay_keybindings_unbind_keys",
            "op_clay_keybindings_list_key_bindings",
        ];
        assert!(
            known.iter().any(|op| line.contains(op)),
            "facade references unknown keybinding op: {line}"
        );
    }

    // (4) The generated registry stores API shape only: it must not carry any
    // default keybinding metadata that could go stale when defaults change.
    let registry = fs::read_to_string(root.join("docs/generated/clay-js-api-registry.json"))
        .expect("read generated registry");
    for stale in ["Ctrl+Shift+P", "Ctrl+Alt+P", "controlCenter.openPath"] {
        assert!(
            !registry.contains(stale),
            "generated registry must not embed default keybinding metadata ({stale})"
        );
    }
}

#[test]
fn plan086_virtual_accessibility_helpers_are_not_public_programmatic_surfaces() {
    let root = repository_root();
    let accessibility =
        fs::read_to_string(root.join("src/editor/accessibility.rs")).expect("read accessibility");
    assert!(
        accessibility.contains("pub(crate) fn virtual_a11y_node_id"),
        "virtual a11y ID derivation must stay crate-private"
    );
    assert!(
        accessibility.contains("pub(crate) mod virtual_a11y_slots"),
        "virtual a11y slot namespace must stay crate-private"
    );
    assert!(
        !accessibility.contains("pub fn virtual_a11y_node_id")
            && !accessibility.contains("pub mod virtual_a11y_slots"),
        "virtual a11y helpers must not become bare-public Rust APIs"
    );

    for entry in fs::read_dir(root.join("src/server/ops")).expect("read server ops") {
        let path = entry.expect("server ops entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read server ops source");
        for internal_name in [
            "virtual_a11y_node_id",
            "VIRTUAL_A11Y_NODE_PREFIX",
            "virtual_a11y_slots",
        ] {
            assert!(
                !source.contains(internal_name),
                "{} must not wrap accessibility internal {internal_name} in a deno_core op",
                path.display()
            );
        }
    }

    for entry in fs::read_dir(root.join("runtime/js")).expect("read runtime/js") {
        let path = entry.expect("runtime/js entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("js" | "ts")
        ) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read facade source");
        for internal_name in [
            "virtual_a11y_node_id",
            "VIRTUAL_A11Y_NODE_PREFIX",
            "virtual_a11y_slots",
        ] {
            assert!(
                !source.contains(internal_name),
                "{} must not expose accessibility internal {internal_name} to JavaScript",
                path.display()
            );
        }
    }

    let registry = fs::read_to_string(root.join("docs/generated/clay-js-api-registry.json"))
        .expect("read generated registry");
    for internal_name in [
        "virtual_a11y_node_id",
        "VIRTUAL_A11Y_NODE_PREFIX",
        "virtual_a11y_slots",
    ] {
        assert!(
            !registry.contains(internal_name),
            "generated registry must not contain accessibility internal {internal_name}"
        );
    }
}
