use std::fs;
use std::path::Path;

const FACADE_MODULES: &[(&str, &[&str])] = &[
    (
        "runtime/js/editor.js",
        &[
            "serverInsertText",
            "serverDeleteRange",
            "serverInsertNewline",
            "clientMoveCursor",
            "clientSetSelection",
            "clientScrollTo",
            "clientSetCursorStyle",
            "clientSetViewport",
            "clientCopySelection",
            "clientCutSelection",
            "clientPasteClipboard",
            "clientUndo",
            "clientRedo",
            "clientShowOpenDocuments",
            "clientRequestResync",
            "clientDismissRecovery",
            "toggleComment",
            "toggleListMarker",
            "rotateHeading",
            "clientToggleFold",
            "toggleInlayHints",
            "clientAddCursor",
            "clientColumnSelect",
            "clientSelectNextMatch",
            "clientSelectPrevMatch",
            "clientSelectAllMatches",
            "clientCancelMultipleSelections",
            "clientKeepSelection",
            "clientRemoveSelection",
            "clientUndoCursorMove",
            "clientSelectTextobject",
            "clientSmartSelect",
            "clientExecuteEditorCommand",
        ],
    ),
    (
        "runtime/js/keybindings.js",
        &["bindKey", "unbindKey", "listKeyBindings"],
    ),
    (
        "runtime/js/configuration.js",
        &[
            "loadConfigurationModule",
            "getConfigurationState",
            "setPackageOption",
            "setModePreference",
            "setDecorationTheme",
            "setParsePolicy",
        ],
    ),
    (
        "runtime/js/documents.js",
        &[
            "serverGetDocumentSnapshot",
            "serverGetDocumentLease",
            "clientOpenFileDialog",
            "serverOpenDocument",
            "serverSaveDocument",
            "serverReloadDocument",
            "serverGetDocumentStatus",
            "serverListDocuments",
        ],
    ),
    (
        "runtime/js/workspace.js",
        &[
            "serverListWorkspaceRoots",
            "serverAddWorkspaceRoot",
            "serverDiscoverWorkspaceRootForPath",
            "serverListDirectory",
            "serverCreateListingCancelToken",
            "serverCancelListing",
            "clientOpenFolderDialog",
        ],
    ),
    (
        "runtime/js/behavior.js",
        &["getActiveBehaviorManifest", "listBehaviorRoutes"],
    ),
    (
        "runtime/js/git.js",
        &["serverListGitStatuses", "serverRefreshGitStatus"],
    ),
    (
        "runtime/js/sdui.js",
        &[
            "definePanel",
            "defineLabel",
            "defineButton",
            "defineList",
            "defineEditorView",
            "defineFlex",
            "defineStack",
            "publishTree",
        ],
    ),
    (
        "runtime/js/ui.js",
        &[
            "serverRegisterPaneContentContribution",
            "serverRegisterPanelContribution",
            "serverRegisterComponentContribution",
            "serverRegisterTransientOverlayContribution",
            "serverRegisterInputContribution",
            "serverRegisterUiStateScope",
            "serverRegisterThemeToken",
            "serverRequestLayoutIntent",
        ],
    ),
    ("runtime/js/application.js", &["quit"]),
    (
        "runtime/js/packages.js",
        &[
            "serverValidatePackageManifest",
            "serverValidatePackagePermissions",
            "serverLoadPackage",
            "loadPackage",
        ],
    ),
    (
        "runtime/js/modes.js",
        &[
            "serverRegisterModePattern",
            "serverClassifyDocument",
            "serverActivateMajorMode",
            "serverSelectDocumentManifest",
            "serverRegisterDecorationProvider",
            "serverRegisterParseProvider",
            "serverRegisterFoldingProvider",
        ],
    ),
    (
        "runtime/js/commands.js",
        &[
            "serverRegisterCommand",
            "serverListCommands",
            "serverExecuteCommand",
            "serverOpenFile",
            "serverOpenDirectory",
            "serverRevealInTree",
        ],
    ),
    ("runtime/js/decorations.js", &["serverPublishDecorations"]),
    ("runtime/js/folding.js", &["serverPublishFoldingRanges"]),
    ("runtime/js/diagnostics.js", &["serverPublishDiagnostics"]),
    (
        "runtime/js/language-server.js",
        &["authorizeLanguageServer", "startLanguageServerSession"],
    ),
    (
        "runtime/js/language.js",
        &[
            "serverRegisterDocumentAnalyzer",
            "serverRegisterLanguageIntelligenceProvider",
        ],
    ),
    ("runtime/js/parse.js", &["serverRegisterParseHandler"]),
    ("runtime/js/syntax.js", &["serverRegisterSyntaxGrammar"]),
    (
        "runtime/js/completion.js",
        &[
            "serverRegisterCompletionProvider",
            "serverDisableCompletion",
        ],
    ),
    (
        "runtime/js/theme.js",
        &["setTheme", "setAppearance", "setTypography"],
    ),
    (
        "runtime/js/shell.js",
        &[
            "clientSplitPaneVertical",
            "clientSplitPaneHorizontal",
            "clientAddEqualPane",
            "clientClosePane",
            "clientFocusPaneNext",
            "clientFocusPanePrev",
            "clientResizePaneLeft",
            "clientResizePaneRight",
            "clientResizePaneUp",
            "clientResizePaneDown",
            "clientMovePaneNext",
            "clientMovePanePrev",
            "clientTabNext",
            "clientTabPrev",
            "clientTabNew",
            "clientTabClose",
            "clientTabMoveLeft",
            "clientTabMoveRight",
            "clientTabActivate",
            "clientTabMoveTo",
            "setPaneFocusPolicy",
        ],
    ),
];

#[test]
fn clay_js_facade_modules_exist_with_expected_exports() {
    for (path, exports) in FACADE_MODULES {
        let source =
            fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));

        let declaration_path = path.replace(".js", ".d.ts");
        let declarations = fs::read_to_string(&declaration_path)
            .unwrap_or_else(|err| panic!("failed to read {declaration_path}: {err}"));
        for export_name in *exports {
            let function_export = format!("export function {export_name}");
            let async_function_export = format!("export async function {export_name}");
            assert!(
                source.contains(&function_export) || source.contains(&async_function_export),
                "{path} must export planned facade function {export_name}"
            );
            assert!(
                declarations.contains(&format!("function {export_name}")),
                "{declaration_path} must declare facade function {export_name}"
            );
        }
    }

    assert!(
        Path::new("runtime/js/mod.ts").exists(),
        "aggregate facade module is missing"
    );
    assert!(
        Path::new("runtime/js/README.md").exists(),
        "facade README is missing"
    );
}

#[test]
fn clay_js_facade_exports_follow_naming_and_boundary_rules() {
    for (path, _) in FACADE_MODULES {
        let source =
            fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));

        assert!(
            !source.contains("Deno.core.ops."),
            "{path} must not call raw Deno core ops from the public facade"
        );

        for line in source
            .lines()
            .filter(|line| line.trim_start().starts_with("export "))
        {
            assert!(
                !line.contains(" op_") && !line.contains(" opClay") && !line.contains("Deno"),
                "{path} exposes an implementation-shaped export: {line}"
            );
            assert!(
                !line.contains("clayEditor") && !line.contains("editorInsert"),
                "{path} repeats module/project context in an export: {line}"
            );
        }
    }
}

#[test]
fn runtime_facades_are_included_from_authoritative_js_files() {
    let table = fs::read_to_string("src/server/facades.rs").unwrap();
    let runtime = fs::read_to_string("src/server/js_runtime/mod.rs").unwrap();
    let mut expected: Vec<_> = FACADE_MODULES.iter().map(|(path, _)| *path).collect();
    let mut executable: Vec<_> = fs::read_dir("runtime/js")
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "js"))
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    expected.sort_unstable();
    executable.sort_unstable();
    assert_eq!(executable, expected);

    for (path, _) in FACADE_MODULES {
        assert_eq!(
            table
                .matches(&format!("include_str!(\"../../{path}\")"))
                .count(),
            1,
            "{path} must be included exactly once by the runtime facade table"
        );
    }
    assert!(!runtime.contains("const CLAY_FACADE_"));
}

#[test]
fn load_package_does_not_expose_raw_op_names() {
    // `loadPackage` is the only public symbol added; the packages facade must
    // not expose raw `op_`-shaped exports. (The general boundary rule is enforced
    // for all facades above; this pins it explicitly for the new loader.)
    let ts_facade = fs::read_to_string("runtime/js/packages.js").unwrap();
    for line in ts_facade
        .lines()
        .filter(|line| line.trim_start().starts_with("export "))
    {
        assert!(
            !line.contains(" op_") && !line.contains("Deno"),
            "runtime/js/packages.js must not expose an implementation-shaped export: {line}"
        );
    }
}

#[test]
fn shell_tab_facade_helpers_lock_stable_command_ids() {
    // The tab helpers return the same dotted IDs the client maps
    // (masonry_shell ShellClientCommand::from_command_id): flat IDs are
    // string constants, numbered families are a template over 1..9. Lock the
    // exact strings so the facade and the client cannot drift.
    let source = fs::read_to_string("runtime/js/shell.js").unwrap();
    for id in [
        "shell.clientTabNext",
        "shell.clientTabPrev",
        "shell.clientTabNew",
        "shell.clientTabClose",
        "shell.clientTabMoveLeft",
        "shell.clientTabMoveRight",
    ] {
        assert!(source.contains(id), "shell.js must reference {id}");
    }
    assert!(source.contains("`${family}.${n}`"));
    assert!(source.contains("shell.clientTabActivate"));
    assert!(source.contains("shell.clientTabMoveTo"));
    assert!(
        source.contains("n > 9"),
        "the facade must cap positions at 9"
    );
}
