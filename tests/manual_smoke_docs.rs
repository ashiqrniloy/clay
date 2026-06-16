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

fn wiki_doc(path: &str) -> String {
    std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("read {path}: {error}"))
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
        "Windows Markdown Open Dialog Smoke",
        "edit-only",
        "Do not test save for this phase",
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
        "Out of scope for Phase 19 smoke",
        "Windows Explorer file associations",
        "double-click-to-open behavior",
        "non-Windows native dialogs",
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
        "Editor-only main slot",
        "mandatory `main` slot of `PaneSlotLayout`",
        "No default `PanelContribution`",
        "defaultVisibility: \"hidden\"",
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
