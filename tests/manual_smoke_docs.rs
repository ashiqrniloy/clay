fn launch_smoke_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/launch-and-gui-smoke.md"
    ))
    .expect("read docs/development/launch-and-gui-smoke.md")
}

fn performance_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/performance.md"
    ))
    .expect("read docs/development/performance.md")
}

fn windows_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/windows.md"
    ))
    .expect("read docs/development/windows.md")
}

fn markdown_package_reference() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/reference/packages/markdown.md"
    ))
    .expect("read docs/reference/packages/markdown.md")
}

fn manual_file_browser_bug_contract() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/manual-file-browser-workflow-bug-contract.md"
    ))
    .expect("read docs/development/manual-file-browser-workflow-bug-contract.md")
}

fn wiki_doc(path: &str) -> String {
    std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("read {path}: {error}"))
}

#[test]
fn manual_file_browser_workflow_bug_contract_locks_reported_failures() {
    let contract = manual_file_browser_bug_contract();

    for expected in [
        "Manual File Browser Workflow Bug Contract",
        "cargo run",
        "~/.config/clay/init.js",
        "do not use `cargo run -- smoke-gui --config-fixture file-browser-workflow`",
        "Ctrl+Shift+O",
        "clientOpenFolderDialog()",
        "lowercase `o`",
        "uppercase `O`",
        "ActionSourceMismatch(SduiNodeId(5))",
        "row id is `main.rs`",
        "action source item id is `src/main.rs`",
        "UnknownActionCommand(\"workspace.openFile\")",
        "parse.open_activation_timeout",
        "second file contents do not replace the editor buffer",
        "editor region falls back to the full rect",
        "bottom-right purple circle",
        "inset card/padding region",
        "File browser cannot scroll",
        "no vertical scrollbar/thumb",
        "keybinding route / behavior manifest lookup",
        "SDUI list/action identity",
        "server `StaticSduiState` workspace-browser validation",
        "editor region computation / shell pane layout",
        "editor paint chrome",
        "SDUI scroll state / pointer scroll routing",
        "editor scroll chrome",
        "must not open a real GUI",
        "must not run package JavaScript, server IPC, filesystem work, shell commands, or full-document serialization",
        "Packages still cannot call raw `Deno.core.ops`",
        "keybinding_shifted_character_routes_client_ui_command",
        "file_browser_nested_file_action_source_matches_list_item_id",
        "file_browser_actions_still_validate_after_markdown_open_timeout",
        "opening_second_workspace_file_replaces_editor_snapshot",
        "file_browser_left_slot_still_reserves_editor_region_after_document_open",
    ] {
        assert!(
            contract.contains(expected),
            "manual file browser bug contract must lock `{expected}`"
        );
    }
}

#[test]
fn phase18_10_manual_syntax_smoke_has_runnable_fixture_contract() {
    let launch_doc = launch_smoke_doc();
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/configuration/syntax-grammars/init.js"
    ))
    .expect("read syntax-grammars smoke fixture");

    for expected in [
        "Phase 18.10 syntax grammar package smoke",
        "cargo run -- smoke-gui --config-fixture syntax-grammars",
        "loadPackage(\"@clay/rust\")",
        "loadPackage(\"@clay/typescript\")",
        "loadPackage(\"@clay/javascript\")",
        "loadPackage(\"@clay/markdown\")",
        "tests/fixtures/syntax/rust.rs",
        "tests/fixtures/syntax/typescript.ts",
        "tests/fixtures/syntax/typescript.tsx",
        "tests/fixtures/syntax/javascript.js",
        "tests/fixtures/syntax/markdown.md",
        "renders text immediately and remains editable under its active `core.code`/`core.text` fallback behavior",
        "Remove the language package load lines and relaunch",
        "Automated coverage (no manual execution needed)",
        "manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow",
        "first_party_language_fixtures_produce_themed_vocabulary_decorations",
        "first_party_artifact_provenance_is_recorded",
        "syntax_provider_selection_falls_back_to_no_highlighting_without_changing_mode",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 18.10 syntax smoke marker `{expected}`"
        );
    }

    assert!(
        fixture.contains("loadPackage(\"@clay/rust\")")
            && fixture.contains("loadPackage(\"@clay/typescript\")")
            && fixture.contains("loadPackage(\"@clay/javascript\")")
            && fixture.contains("loadPackage(\"@clay/markdown\")")
            && !fixture.contains("serverRegisterSyntaxGrammar")
            && !fixture.contains("Deno.core.ops"),
        "syntax-grammars smoke fixture must use only end-user loadPackage calls"
    );
}

#[test]
fn plan056_linux_syntax_smoke_and_measurements_are_recorded() {
    let launch_doc = launch_smoke_doc();
    let performance_doc = performance_doc();

    for expected in [
        "Plan 056 low-latency syntax Linux smoke (2026-07-19)",
        "cargo run -- smoke-gui --config-fixture language-packages --profile-perf",
        "managed smoke cleanup left no `clay-smoke-gui` server process",
        "rapid keyword/identifier/punctuation/comment/string/prose/code edits",
        "syntax-plus-semantic layering",
        "save, undo/redo, and document switching",
        "syntax_grammar` (58 tests)",
        "parse_coordinator` (29)",
        "decoration_transport` (15)",
        "performance_protocol` (19)",
        "editor_performance_invariants` (22)",
        "language_intelligence` (31)",
    ] {
        assert!(
            launch_doc.contains(expected),
            "Plan 056 launch record must contain `{expected}`"
        );
    }

    for expected in [
        "Plan 056 low-latency syntax Linux verification (2026-07-19)",
        "cargo test --all-targets",
        "cargo bench --no-run",
        "first_party_incremental_edit",
        "168.91 µs",
        "356.50 µs",
        "125.02 µs",
        "124.91 µs",
        "217.54 µs",
        "syntax.parse.logical_work_items",
        "syntax.edit_to_publish",
        "syntax_pipeline_metrics_are_source_safe_and_retention_bounded",
        "Metrics remain numeric-only and never include source text or paths",
    ] {
        assert!(
            performance_doc.contains(expected),
            "Plan 056 performance record must contain `{expected}`"
        );
    }
}

#[test]
fn plan057_linux_syntax_continuity_smoke_and_measurements_are_recorded() {
    let launch_doc = launch_smoke_doc();
    let performance_doc = performance_doc();

    for expected in [
        "Plan 057 syntax-continuity Linux smoke (2026-07-19)",
        "cargo run -- smoke-gui --config-fixture language-packages --profile-perf",
        "appending `x` to the already classified `greet` declaration",
        "complete `greetx` run in the function color",
        "all-white newline regression were not observed",
        "plan057_first_party_languages_keep_continuity_across_edit_boundaries",
        "plan057_authoritative_queries_correct_inherited_code_keywords",
        "rapid_local_versions_reject_stale_authority_without_losing_provisional_geometry",
        "plan057_utf8_scalar_at_nominal_chunk_boundary_is_never_split",
        "Rust, TypeScript, TSX, JavaScript, and Markdown",
    ] {
        assert!(
            launch_doc.contains(expected),
            "Plan 057 launch record must contain `{expected}`"
        );
    }

    for expected in [
        "Plan 057 syntax-continuity Linux verification (2026-07-19)",
        "first_party_continuity_edits_keep_one_bounded_parse_and_query",
        "accepted_native_edit_records_one_logical_item_and_one_latency_sample",
        "140.268 µs",
        "167.95 µs",
        "361.10 µs",
        "125.92 µs",
        "122.93 µs",
        "199.86 µs",
        "no statistically significant performance change",
        "machine-local and advisory",
    ] {
        assert!(
            performance_doc.contains(expected),
            "Plan 057 performance record must contain `{expected}`"
        );
    }
}

#[test]
fn plan058_linux_exact_range_smoke_and_measurements_are_recorded() {
    let launch_doc = launch_smoke_doc();
    let performance_doc = performance_doc();

    for expected in [
        "Plan 058 exact-range replacement Linux smoke (2026-07-20)",
        "cargo run -- smoke-gui --config-fixture language-packages --profile-perf",
        "Eight letters were typed one at a time inside that comment",
        "no one-byte-per-keypress white gap",
        "Backspace and Enter",
        "plan058_first_party_languages_preserve_shifted_boundary_continuity",
        "plan058_repeated_insert_delete_authority_cycles_preserve_boundary_geometry",
        "repeated_authority_keeps_local_residual_cache_bounded",
        "Temporary fixture text was restored",
    ] {
        assert!(
            launch_doc.contains(expected),
            "Plan 058 launch record must contain `{expected}`"
        );
    }

    for expected in [
        "Plan 058 exact-range replacement Linux verification (2026-07-20)",
        "one parser call, one query range, and one emitted member",
        "20 queried bytes for Rust",
        "26 for TypeScript/TSX/JavaScript",
        "17 for Markdown",
        "512 authoritative applications",
        "first_party_authoritative_replacement/apply_and_coalesce_residual",
        "1.8150 µs",
        "no statistically significant regression",
        "machine-local and advisory",
    ] {
        assert!(
            performance_doc.contains(expected),
            "Plan 058 performance record must contain `{expected}`"
        );
    }
}

#[test]
fn phase18_16_tiered_syntax_smoke_documents_engine_selection() {
    let launch_doc = launch_smoke_doc();

    for expected in [
        "Phase 18.16 tiered syntax engine smoke",
        "setSyntaxEnginePreference(\"rust\", \"wasm\")",
        "setSyntaxEnginePreference(\"markdown\", \"javascript\")",
        "Tier 1 vocabulary highlighting",
        "Tier 2 package assets",
        "Tier 3 package parser",
        "parse.open_failed",
        "cargo test --test runtime syntax_grammar::",
        "cargo test --test runtime parse_coordinator::",
        "cargo test --test protocol manual_smoke_docs::",
        "cargo test --test editor editor_performance_invariants::",
        "No network fetch",
        "third-party grammar trust is deferred to Phase 23",
        "tests/fixtures/syntax/typescript.tsx",
        "packages/*/grammars/PROVENANCE.md",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 18.16 tiered syntax marker `{expected}`"
        );
    }
}

#[test]
fn phase18_16_5_typography_smoke_covers_fallback_geometry_and_authority() {
    let launch_doc = launch_smoke_doc();

    for expected in [
        "Phase 18.16.5 typography smoke",
        "Gruvbox Material dark and light",
        "6 px, defaults, and 40 px",
        "Unicode (`Hé`, `漢字`)",
        "emoji (`🦀`)",
        "unavailable name followed by its generic fallback",
        "must not fetch or open font files/URLs",
        "caret, selection, wrapping, hit testing, and scrolling",
        "status text, Workspace file browser, runtime SDUI, package status items",
        "accessibility bounds must scale together",
        "Remove typography configuration and reconnect",
        "previous complete typography remains active",
        "typography_updates_do_not_enter_editor_hot_paths",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define typography marker `{expected}`"
        );
    }
}

#[test]
fn phase18_17_manual_smoke_documents_analyzer_only_diagnostics() {
    let launch_doc = launch_smoke_doc();

    for expected in [
        "Phase 18.17 range diagnostic transport smoke",
        "DiagnosticSet",
        "explicit analyzer packages",
        "serverPublishDiagnostics",
        "RuntimeDiagnostic",
        "cargo run -- smoke-gui --config-fixture syntax-grammars",
        "without red squiggles from Tree-sitter",
        "bounded-fragment recovery nodes are not correctness authority",
        "Local typing/scroll remain responsive",
        "tree_sitter_highlighting_does_not_emit_range_diagnostics",
        "first_party_invalid_fixtures_do_not_masquerade_as_analyzer_diagnostics",
        "runtime_diagnostics_remain_status_level_and_range_diagnostics_remain_inline",
        "range_diagnostics_do_not_enter_editor_hot_paths",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 18.17 range-diagnostic marker `{expected}`"
        );
    }
}

#[test]
fn phase18_18_manual_smoke_documents_first_party_language_matrix() {
    let launch_doc = launch_smoke_doc();
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/configuration/language-packages/init.js"
    ))
    .expect("read language-packages smoke fixture");

    for expected in [
        "Phase 18.18 first-party language package smoke",
        "cargo run -- smoke-gui --config-fixture language-packages",
        "tests/fixtures/syntax/rust.rs",
        "tests/fixtures/syntax/typescript.tsx",
        "tests/fixtures/syntax/javascript.jsx",
        "tests/fixtures/syntax/javascript.mjs",
        "tests/fixtures/syntax/javascript.cjs",
        "tests/fixtures/syntax/markdown-invalid.md",
        "Gruvbox-themed native vocabulary highlighting",
        "keyword completion",
        "indent, pairs, and comment behavior",
        "without a grammar-produced squiggle",
        "Typing/scroll remain responsive",
        "graceful fallback to `core.code` (Rust/TypeScript/JavaScript) or `core.text` (Markdown)",
        "no secrets, real paths, or executable authority",
        "first_party_syntax_fixtures_exist_per_language",
        "language_packages_config_fixture_loads_and_registers_all_contributions",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 18.18 language package smoke marker `{expected}`"
        );
    }

    for specifier in [
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "@clay/markdown",
    ] {
        assert!(
            fixture.contains(&format!("loadPackage(\"{specifier}\")")),
            "language package smoke fixture must explicitly load {specifier}"
        );
    }
    for forbidden in [
        "serverRegisterSyntaxGrammar",
        "serverRegisterModePattern",
        "Deno.core.ops",
    ] {
        assert!(
            !fixture.contains(forbidden),
            "language package smoke fixture must not use hidden/raw authority `{forbidden}`"
        );
    }
}

#[test]
fn first_party_syntax_fixtures_exist_per_language() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "tests/fixtures/syntax/rust.rs",
        "tests/fixtures/syntax/rust-invalid.rs",
        "tests/fixtures/syntax/typescript.ts",
        "tests/fixtures/syntax/typescript.tsx",
        "tests/fixtures/syntax/typescript-invalid.ts",
        "tests/fixtures/syntax/javascript.js",
        "tests/fixtures/syntax/javascript.jsx",
        "tests/fixtures/syntax/javascript.mjs",
        "tests/fixtures/syntax/javascript.cjs",
        "tests/fixtures/syntax/javascript-invalid.js",
        "tests/fixtures/syntax/markdown.md",
        "tests/fixtures/syntax/markdown-invalid.md",
    ] {
        let path = manifest_dir.join(relative);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        assert!(
            !text.trim().is_empty(),
            "{} must not be empty",
            path.display()
        );
        assert!(
            !text.contains("/home/") && !text.to_ascii_lowercase().contains("secret"),
            "{} must remain synthetic and path/secret-free",
            path.display()
        );
    }
}

#[test]
fn end_to_end_file_browser_workflow_smoke_has_runnable_fixture_contract() {
    let launch_doc = launch_smoke_doc();
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/configuration/file-browser-workflow/init.js"
    ))
    .expect("read file-browser-workflow smoke fixture");

    for expected in [
        "End-to-end file browser workflow smoke",
        "cargo run -- smoke-gui --config-fixture file-browser-workflow",
        "1. Open the Clay app.",
        "2. See the Clay-owned Workspace file browser.",
        "3. Select a folder from the system.",
        "4. Navigate different folders and files.",
        "5. See file contents when the selected file is Rust, TypeScript, or JavaScript.",
        "6. Copy text snippets from opened files to the OS clipboard.",
        "clientOpenFolderDialog()",
        "clientCopySelection()",
        "bindKey(\"Ctrl+Shift+O\", clientOpenFolderDialog(), { scope: \"editor\" });",
        "bindKey(\"Ctrl+Shift+C\", clientCopySelection(), { scope: \"editor\" });",
        "workspace.openDirectory",
        "../` parent row",
        "tests/fixtures/configuration/file-browser-workflow/workspace/main.rs",
        "main.ts",
        "main.js",
        "Rust/TypeScript/JavaScript language package",
        "only the selected UTF-8 text is copied",
        "Copy selection is write-only",
        "Automated coverage (no manual execution needed)",
        "file_browser_workflow_config_fixture_loads_packages_and_bindings",
        "workspace_directory_action_sends_refreshed_file_browser_snapshot",
        "file_browser_open_uses_generic_open_document_followups",
        "copy_selection_writes_selected_text_without_edit_event",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define end-to-end file browser workflow marker `{expected}`"
        );
    }

    for expected in [
        "loadPackage(\"@clay/rust\")",
        "loadPackage(\"@clay/typescript\")",
        "loadPackage(\"@clay/javascript\")",
        "clientOpenFolderDialog()",
        "clientCopySelection()",
        "clientCutSelection()",
        "clientPasteClipboard()",
        "clientShowOpenDocuments()",
        "workspace.openFuzzyFile",
        "workspace.toggleFileBrowser",
        "documents.serverSaveDocument",
    ] {
        assert!(
            fixture.contains(expected),
            "file-browser-workflow fixture must contain `{expected}`"
        );
    }

    for forbidden in [
        "Deno.core.ops",
        "serverRegisterSyntaxGrammar",
        "serverRegisterModePattern",
        "serverPublishDecorations",
        "clipboard.writeText",
        "readText",
    ] {
        assert!(
            !fixture.contains(forbidden),
            "file-browser-workflow fixture must not use hidden/raw authority `{forbidden}`"
        );
    }
}

#[test]
fn end_to_end_file_browser_workflow_smoke_covers_cargo_run_config_path() {
    let launch_doc = launch_smoke_doc();

    // The real product workflow path must be documented alongside the
    // fixture: a bare `cargo run` driven by `~/.config/clay/init.js`, with
    // the Plan 044/Phase 20 regressions (shifted folder picker, nested `.rs` open,
    // multi-document retain/switch, dirty/save/conflict UX, file browser surviving
    // Markdown activation, file-browser scroll, editor scroller, copy) as a manual checklist.
    for expected in [
        "Product `cargo run` configuration path",
        "cargo run",
        "~/.config/clay/init.js",
        "Ctrl+Shift+O",
        "src/main.rs",
        "Opening a second file retains the prior document session",
        "documents.serverSaveDocument",
        "Dirty",
        "The file browser scrolls when there are many rows",
        "The editor shows a slim vertical scrollbar thumb",
        "copies only the selected UTF-8 text",
        "Typing, paint, layout, pointer, and scroll stay client-local",
        "selected-folder grants are server-validated",
        "clipboard copy is write-only",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must cover the cargo run config path marker `{expected}`"
        );
    }
}

#[test]
fn phase20_daily_editing_platform_matrix_and_linux_verification_are_documented() {
    let launch_doc = launch_smoke_doc();
    let workflow_doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/file-open-save-reload-workflow.md"
    ))
    .expect("read file-open-save-reload workflow doc");

    for expected in [
        "Phase 20 daily-editing platform matrix and Linux verification",
        "Platform capability matrix",
        "Shortcut matrix (native editor chords)",
        "Linux verification evidence (Plan 055 Task 17)",
        "cargo fmt --check",
        "cargo clippy --all-targets -- -D warnings",
        "cargo test --all-targets",
        "xdg-desktop-portal",
        "Ctrl+C",
        "Cmd+C",
        "Ctrl+Z",
        "Cmd+Z",
        "Ctrl+Y",
        "Cmd+Shift+Z",
        "clientShowOpenDocuments",
        "clientRequestResync",
        "clientDismissRecovery",
        "ibus/fcitx",
        "DocumentSessionStore",
        "Linux-primary",
        "Windows/macOS host checklists",
        "Live boot check",
        "Ime::Enabled",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must record Phase 20 platform matrix marker `{expected}`"
        );
    }

    for expected in [
        "Clipboard copy/cut/paste",
        "IME preedit overlay",
        "Pending-edit / disconnect recovery",
        "launch-and-gui-smoke.md#phase-20-daily-editing-platform-matrix-and-linux-verification",
    ] {
        assert!(
            workflow_doc.contains(expected),
            "file workflow docs must record Phase 20 platform matrix marker `{expected}`"
        );
    }
}

#[test]
fn phase19_manual_smoke_docs_define_open_dialog_scope() {
    let launch_doc = launch_smoke_doc();
    let windows_doc = windows_doc();

    for expected in [
        "Phase 19 Windows Markdown open-dialog smoke contract",
        "Windows native dialog backend",
        "Windows 11",
        "documents.clientOpenFileDialog",
        "filters for `.md`, `.markdown`, and `.mdown`",
        "regular UTF-8 Markdown file",
        "cargo run -- smoke-gui --config-fixture windows-markdown-open",
        "Windows Markdown open-dialog smoke",
        "edit-only",
        "Do not test save in Phase 19",
        "A selected path is an explicit user-mediated open request only",
        "not unrestricted client filesystem authority",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 19 scope marker `{expected}`"
        );
    }

    assert!(
        windows_doc.contains("Phase 19")
            && windows_doc
                .contains("cargo run -- smoke-gui --config-fixture windows-markdown-open")
            && windows_doc.contains("activate Markdown mode")
            && windows_doc.contains("viewport-bounded Markdown decorations/status")
            && windows_doc.contains("cancellation is a non-error no-op"),
        "Windows docs must link the Phase 19 smoke contract and document selected-file Markdown activation"
    );
}

#[test]
fn phase19_code_wiki_documents_open_dialog_path() {
    let index = wiki_doc("docs/wiki/index.md");
    let client_dialog = wiki_doc("docs/wiki/modules/client-file-dialog.md");
    let workspace = wiki_doc("docs/wiki/modules/server-file-workspace.md");
    let edit_ack = wiki_doc("docs/wiki/flows/client-server-edit-ack.md");
    let markdown = wiki_doc("docs/wiki/modules/first-party-markdown-package.md");

    for linked_page in [
        "modules/client-file-dialog.md",
        "modules/server-file-workspace.md",
        "flows/client-server-edit-ack.md",
        "modules/first-party-markdown-package.md",
    ] {
        assert!(
            index.contains(linked_page),
            "wiki index must link Phase 19 implementation page `{linked_page}`"
        );
    }

    for expected in [
        "docs/reference/clay-js-api/documents/client-open-file-dialog.md",
        "Shell COM APIs",
        "FileDialogResult::Selected(PathBuf)",
        "Cancellation is a non-error no-op",
        "WorkspaceState::open_selected_file",
    ] {
        assert!(
            client_dialog.contains(expected),
            "client dialog wiki must document marker `{expected}`"
        );
    }

    assert!(
        workspace.contains("selected-file single-file grants")
            && workspace.contains("without authorizing sibling paths")
            && workspace.contains("Invalid UTF-8 files do not create or poison registry entries"),
        "workspace wiki must document selected-file grant validation and authority boundaries"
    );
    assert!(
        edit_ack.contains("ClientMessage::OpenSelectedFile")
            && edit_ack.contains("without reserving an edit transaction")
            && edit_ack.contains("DocumentOpened"),
        "edit-ack wiki must document selected-file request and opened-document snapshot flow"
    );
    assert!(
        markdown.contains("Selected-file open now follows one generic path")
            && markdown.contains("`schedule_open_parse` returns immediately after enqueue")
            && markdown.contains(
                "query/decor authority remains capped to the existing 4 KiB viewport output budget"
            ),
        "Markdown package wiki must document selected-file activation and hot-path boundaries"
    );
}

#[test]
fn phase19_manual_smoke_docs_reject_file_association_requirement() {
    let launch_doc = launch_smoke_doc();

    for expected in [
        "Out of scope for the Phase 19 Windows Markdown open-dialog smoke only",
        "Windows Explorer file associations",
        "double-click-to-open behavior",
        "Linux folder selection, directory navigation, and workspace-root expansion are covered separately",
        "full HTML preview or browser/webview rendering",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must exclude out-of-scope marker `{expected}`"
        );
    }

    for forbidden in [
        "requires Windows Explorer file associations",
        "requires file associations",
        "Double-click a `.md` file in Explorer to run the Phase 19 smoke",
    ] {
        assert!(
            !launch_doc.contains(forbidden),
            "Phase 19 docs must not require file association behavior via `{forbidden}`"
        );
    }
}

#[test]
fn phase20_end_user_markdown_setup_is_one_line_load_plus_bind_key() {
    // Guard: the actual-app Markdown instructions show a minimal package load
    // plus an explicit bindKey, not the smoke fixture manifest block. The
    // end-user product baseline must be documented and clearly separated from
    // the dev-only fixtures.
    let launch_doc = launch_smoke_doc();
    let markdown_ref = markdown_package_reference();

    for required in [
        "Default End-User Configuration",
        "import { loadPackage } from \"clay:packages\";",
        "await loadPackage(\"@clay/markdown\");",
        "bindKey(\"Ctrl+O\", \"documents.clientOpenFileDialog\", { scope: \"editor\" });",
        "Smoke-only (dev validation, never the product path)",
        "End-user (product baseline)",
        "inline a full `markdownPackage` manifest object",
        "Pasting the smoke fixture manifest block into `~/.config/clay/init.js` is not supported",
    ] {
        assert!(
            launch_doc.contains(required),
            "launch smoke docs must document the Phase 20 end-user Markdown baseline marker `{required}`"
        );
    }

    // The reference doc must carry the same baseline contract.
    for required in [
        "End-User UX Baseline",
        "import { loadPackage } from \"clay:packages\";",
        "await loadPackage(\"@clay/markdown\");",
        "bindKey(\"Ctrl+O\", \"documents.clientOpenFileDialog\", { scope: \"editor\" });",
        "product baseline",
        "are dev validation, never the documented end-user path",
    ] {
        assert!(
            markdown_ref.contains(required),
            "docs/reference/packages/markdown.md must document the Phase 20 end-user Markdown baseline marker `{required}`"
        );
    }
}

#[test]
fn phase20_markdown_baseline_no_default_panel_and_edit_only_selected_file() {
    // Guard: default Markdown mode does not require a PanelContribution, and
    // selected-file save/conflict UX is Clay-owned (Ctrl+S + recovery menu).
    // These baseline invariants must be recorded in the product docs.
    let launch_doc = launch_smoke_doc();
    let markdown_ref = markdown_package_reference();

    for required in [
        "Editor-only Markdown main slot",
        "mandatory `main` slot of `PaneSlotLayout`",
        "package-owned default `PanelContribution`",
        "does not forbid the Clay-owned Workspace file browser",
        "Clay-owned workspace chrome is separate",
        "defaultVisibility: \"hidden\"",
        "Fixed panels resize the editor",
        "transient overlays may cover content by design",
        "Selected-file open supports save/conflict UX",
        "documents.serverSaveDocument",
        "Configuration/open time only",
        "No authority broadened",
    ] {
        assert!(
            launch_doc.contains(required),
            "launch smoke docs must record the Phase 20 Markdown baseline invariant `{required}`"
        );
    }

    for required in [
        "Editor-only main slot",
        "mandatory `main` slot of `PaneSlotLayout`",
        "No default `PanelContribution`",
        "defaultVisibility: \"hidden\"",
        "Selected-file open supports save/conflict UX",
        "documents.serverSaveDocument",
    ] {
        assert!(
            markdown_ref.contains(required),
            "docs/reference/packages/markdown.md must record the Phase 20 Markdown baseline invariant `{required}`"
        );
    }
}

#[test]
fn phase18_11_manual_completion_smoke_has_runnable_contract() {
    let launch_doc = launch_smoke_doc();

    for expected in [
        "Phase 18.11 completion provider smoke",
        "bindKey(\"Ctrl+Space\", \"completion.trigger\", { scope: \"editor\" })",
        "core.bufferWords",
        "TransientMenuSession",
        "ArrowUp",
        "ArrowDown",
        "Enter",
        "Tab",
        "Escape",
        "commit character",
        "autocomplete trigger character",
        "edits locally first",
        "bounded non-blocking channel",
        "UiReactivePriority",
        "cancellable",
        "stale result is dropped",
        "Disable/reload a package provider",
        "built-in `core.bufferWords` provider should still produce completions",
        "inert text-replacement data only",
        "Automated coverage (no manual execution needed)",
        "completion_hot_paths_use_inert_state_and_nonblocking_enqueue_only",
        "representative_completion_result_payload_stays_bounded",
        "tests/completion_provider.rs",
        "tests/editor_performance_invariants.rs",
        "tests/performance_protocol.rs",
        "tests/package_primitive_gate.rs",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 18.11 completion smoke marker `{expected}`"
        );
    }
}

#[test]
fn phase18_20_language_intelligence_smoke_marks_phase18_21_compatibility() {
    let launch_doc = launch_smoke_doc();

    for expected in [
        "Phase 18.20 language intelligence / Phase 18.21 LSP bridge smoke markers",
        "language.hover",
        "language.goToDefinition",
        "language.codeActions",
        "language.signatureHelp",
        "TransientMenuSession",
        "workspace.openFile",
        "CommandExecution",
        "authorizeLanguageServer",
        "@clay/lsp-rust",
        "@clay/lsp-typescript",
        "@clay/lsp-javascript",
        "@clay/lsp-markdown",
        "rust-analyzer",
        "typescript-language-server",
        "marksman",
        "tests/language_intelligence.rs",
        "language_intelligence_provider_work_is_absent_from_editor_hot_paths",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 18.20/18.21 language-intelligence smoke marker `{expected}`"
        );
    }
}

#[test]
fn plan086_live_atspi_smoke_command_and_prerequisites_are_documented() {
    let launch_doc = launch_smoke_doc();

    for expected in [
        "CLAY_LIVE_A11Y_SMOKE=1",
        "cargo test --test security live_atspi_smoke::live_atspi_accessibility_smoke -- --ignored --exact --test-threads=1",
        "org.a11y.Bus",
        "python3-gi",
        "gir1.2-atspi-2.0",
        "mode-700 temporary IPC/config home",
        "Workspace tabs",
        "tests/live_atspi_smoke.rs",
        "CLAY_LIVE_A11Y_SMOKE",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define the plan 086 live AT-SPI smoke marker `{expected}`"
        );
    }

    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/configuration/runtime-sdui/init.js"
    ))
    .expect("read runtime-sdui smoke fixture");
    assert!(
        fixture.contains("publishTree"),
        "runtime-sdui fixture must keep publishing an SDUI tree for the live AT-SPI smoke"
    );
}

#[test]
fn plan087_ui_review_harness_command_and_prerequisites_are_documented() {
    let launch_doc = launch_smoke_doc();
    let observability_doc = wiki_doc("docs/development/ui-observability.md");
    let script = wiki_doc("scripts/capture-ui-review.sh");

    for expected in [
        "scripts/capture-ui-review.sh --fixture ui-review-default",
        "scripts/capture-ui-review.sh --fixture ui-review-completion",
        "ui-review-default",
        "ui-review-loading",
        "ui-review-error",
        "ui-review-recovery",
        "ui-review-completion",
        "ui-review-command-centre",
        "screenshot.png",
        "accessibility.txt",
        "review.status",
        "mode-700 temporary",
        "xdg-desktop-portal",
        "UNRESOLVED",
        "exits 2",
        "900×600",
        "not GPU goldens",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Plan 087 review marker `{expected}`"
        );
    }

    assert!(
        observability_doc.contains("scripts/capture-ui-review.sh")
            && observability_doc.contains("review.status`=`UNRESOLVED")
            && observability_doc.contains("exit 2"),
        "UI observability docs must link the unresolved-safe Plan 087 harness"
    );

    for fixture in [
        "ui-review-default",
        "ui-review-loading",
        "ui-review-error",
        "ui-review-recovery",
        "ui-review-completion",
        "ui-review-command-centre",
    ] {
        let path = format!(
            "{}/tests/fixtures/configuration/{fixture}/init.js",
            env!("CARGO_MANIFEST_DIR")
        );
        assert!(
            std::path::Path::new(&path).is_file(),
            "Plan 087 review fixture is missing: {path}"
        );
    }

    for expected in [
        "target/debug/clay",
        "HOME=",
        "XDG_CONFIG_HOME=",
        "XDG_DATA_HOME=",
        "TMPDIR=",
        "chmod 700",
        "atspi_probe.py",
        "portal_capture.py",
        "timeout 2s",
        "UNRESOLVED",
        "review.status",
        "tests/fixtures/configuration/",
        "Ctrl+Space",
        "Ctrl+Alt+P",
        "900×600",
    ] {
        assert!(
            script.contains(expected),
            "UI review harness must retain safety/fixture marker `{expected}`"
        );
    }
    assert!(
        !script.contains("cargo run -- smoke-gui"),
        "UI review wrapper must not create a second Cargo/build target path"
    );
}
