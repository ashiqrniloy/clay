fn launch_smoke_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/launch-and-gui-smoke.md"
    ))
    .expect("read docs/development/launch-and-gui-smoke.md")
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
        "UnknownActionCommand(\"clay.workspace.openFile\")",
        "clay.parse.open_activation_timeout",
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
        "tests/fixtures/syntax/rust.rs",
        "tests/fixtures/syntax/typescript.ts",
        "tests/fixtures/syntax/javascript.js",
        "editable under its active `core.code`/`core.text` fallback behavior",
        "Remove the language package load lines and relaunch",
        "Automated coverage (no manual execution needed)",
        "manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow",
        "first_party_syntax_fixtures_produce_bounded_decoration_sets",
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
            && !fixture.contains("serverRegisterSyntaxGrammar")
            && !fixture.contains("Deno.core.ops"),
        "syntax-grammars smoke fixture must use only end-user loadPackage calls"
    );
}

#[test]
fn phase18_14_language_package_expansion_smoke_has_runnable_fixture_contract() {
    let launch_doc = launch_smoke_doc();
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/configuration/language-packages/init.js"
    ))
    .expect("read language-packages smoke fixture");

    for expected in [
        "Phase 18.14 language package expansion smoke",
        "cargo run -- smoke-gui --config-fixture language-packages",
        "loadPackage(\"@clay/rust\")",
        "loadPackage(\"@clay/typescript\")",
        "loadPackage(\"@clay/javascript\")",
        "tests/fixtures/configuration/language-packages/workspace/main.rs",
        "main.ts",
        "main.js",
        "classified into the package-declared major mode",
        "rust.status.mode",
        "typescript.status.mode",
        "javascript.status.mode",
        "bounded, metadata-only completion list",
        "Remove the language package load lines and relaunch",
        "Automated coverage (no manual execution needed)",
        "language_packages_config_fixture_loads_and_registers_all_contributions",
        "rust_package_expansion_registers_mode_command_completion_and_status",
        "typescript_package_expansion_registers_mode_command_completion_and_status",
        "javascript_package_expansion_registers_mode_command_completion_and_status",
    ] {
        assert!(
            launch_doc.contains(expected),
            "launch smoke docs must define Phase 18.14 language package smoke marker `{expected}`"
        );
    }

    assert!(
        fixture.contains("loadPackage(\"@clay/rust\")")
            && fixture.contains("loadPackage(\"@clay/typescript\")")
            && fixture.contains("loadPackage(\"@clay/javascript\")")
            && !fixture.contains("serverRegisterSyntaxGrammar")
            && !fixture.contains("serverRegisterModePattern")
            && !fixture.contains("Deno.core.ops"),
        "language-packages smoke fixture must use only end-user loadPackage calls"
    );
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
        "clay.workspace.openDirectory",
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
        "clay.workspace.openFuzzyFile",
        "clay.workspace.toggleFileBrowser",
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
    // the Plan 044 regressions (shifted folder picker, nested `.rs` open,
    // second-file replacement, file browser surviving Markdown activation,
    // file-browser scroll, editor scroller, copy) as a manual checklist.
    for expected in [
        "Product `cargo run` configuration path",
        "cargo run",
        "~/.config/clay/init.js",
        "Ctrl+Shift+O",
        "src/main.rs",
        "Opening a second file replaces the editor buffer",
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
fn phase19_manual_smoke_docs_define_open_dialog_scope() {
    let launch_doc = launch_smoke_doc();
    let windows_doc = windows_doc();

    for expected in [
        "Phase 19 Windows Markdown open-dialog smoke contract",
        "Windows native dialog backend",
        "Windows 11",
        "clay.documents.clientOpenFileDialog",
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
        markdown
            .contains("Phase 19 extends the same package-owned path to native selected-file opens")
            && markdown.contains("initial parse window capped at 64 KiB")
            && markdown.contains("Ordinary edits after open continue through existing delta IPC"),
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
        "bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\", { scope: \"editor\" });",
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
        "bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\", { scope: \"editor\" });",
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
    // selected-file save remains out of scope. These baseline invariants must
    // be recorded in the product docs.
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
        "Selected-file open is edit-only",
        "Saving a file picked through the dialog is out of scope until a later phase",
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
        "Selected-file open is edit-only",
        "Saving a file picked through the dialog is out of scope until a later phase",
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
