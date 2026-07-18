use std::fs;
use std::path::Path;

const FACADE_MODULES: &[(&str, &[&str])] = &[
    (
        "runtime/js/editor.ts",
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
        ],
    ),
    (
        "runtime/js/keybindings.ts",
        &["bindKey", "unbindKey", "listKeyBindings"],
    ),
    (
        "runtime/js/configuration.ts",
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
        "runtime/js/documents.ts",
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
        "runtime/js/workspace.ts",
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
        "runtime/js/behavior.ts",
        &["getActiveBehaviorManifest", "listBehaviorRoutes"],
    ),
    (
        "runtime/js/sdui.ts",
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
        "runtime/js/ui.ts",
        &[
            "serverRegisterPanelContribution",
            "serverRegisterComponentContribution",
            "serverRegisterTransientOverlayContribution",
            "serverRegisterInputContribution",
            "serverRegisterUiStateScope",
            "serverRegisterThemeToken",
        ],
    ),
    ("runtime/js/application.ts", &["quit"]),
    (
        "runtime/js/packages.ts",
        &[
            "serverValidatePackageManifest",
            "serverValidatePackagePermissions",
            "serverLoadPackage",
            "loadPackage",
        ],
    ),
    (
        "runtime/js/modes.ts",
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
        "runtime/js/commands.ts",
        &[
            "serverRegisterCommand",
            "serverListCommands",
            "serverExecuteCommand",
            "serverOpenFile",
            "serverOpenDirectory",
            "serverRevealInTree",
        ],
    ),
    ("runtime/js/decorations.ts", &["serverPublishDecorations"]),
    ("runtime/js/parse.ts", &["serverRegisterParseHandler"]),
    ("runtime/js/syntax.ts", &["serverRegisterSyntaxGrammar"]),
    (
        "runtime/js/completion.ts",
        &[
            "serverRegisterCompletionProvider",
            "serverDisableCompletion",
        ],
    ),
];

#[test]
fn clay_js_facade_modules_exist_with_expected_exports() {
    for (path, exports) in FACADE_MODULES {
        let source =
            fs::read_to_string(path).unwrap_or_else(|err| panic!("failed to read {path}: {err}"));

        for export_name in *exports {
            let function_export = format!("export function {export_name}");
            let async_function_export = format!("export async function {export_name}");
            assert!(
                source.contains(&function_export) || source.contains(&async_function_export),
                "{path} must export planned facade function {export_name}"
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
fn load_package_facade_stays_in_sync_between_ts_and_embedded_constant() {
    // Phase 18.6 task 5: `loadPackage` ships on BOTH `runtime/js/packages.ts`
    // (the TypeScript facade) and the embedded `CLAY_FACADE_PACKAGES` constant
    // in `src/server/js_runtime.rs`. The runtime embeds facades as constants, so
    // both must export `loadPackage` with the same shape or the runtime will not
    // resolve it from a configuration module.
    let ts_facade = fs::read_to_string("runtime/js/packages.ts").unwrap();
    let embedded = fs::read_to_string("src/server/js_runtime.rs").unwrap();

    for needle in [
        "export async function loadPackage",
        "clay.packages.invalid_specifier: loadPackage requires a string specifier",
        "op_clay_packages_load_package_by_specifier",
        "loadEntrySpecifier",
    ] {
        assert!(
            ts_facade.contains(needle),
            "runtime/js/packages.ts must include the loadPackage facade piece: `{needle}`"
        );
        assert!(
            embedded.contains(needle),
            "the embedded CLAY_FACADE_PACKAGES constant must mirror the same loadPackage piece: `{needle}`"
        );
    }
}

#[test]
fn load_package_does_not_expose_raw_op_names() {
    // `loadPackage` is the only public symbol added; the packages facade must
    // not expose raw `op_`-shaped exports. (The general boundary rule is enforced
    // for all facades above; this pins it explicitly for the new loader.)
    let ts_facade = fs::read_to_string("runtime/js/packages.ts").unwrap();
    for line in ts_facade
        .lines()
        .filter(|line| line.trim_start().starts_with("export "))
    {
        assert!(
            !line.contains(" op_") && !line.contains("Deno"),
            "runtime/js/packages.ts must not expose an implementation-shaped export: {line}"
        );
    }
}
