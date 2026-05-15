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
        ],
    ),
    (
        "runtime/js/keybindings.ts",
        &["bindKey", "unbindKey", "listKeyBindings"],
    ),
    (
        "runtime/js/configuration.ts",
        &["loadConfigurationModule", "getConfigurationState"],
    ),
    (
        "runtime/js/documents.ts",
        &[
            "serverGetDocumentSnapshot",
            "serverGetDocumentLease",
            "serverListDocuments",
        ],
    ),
    (
        "runtime/js/behavior.ts",
        &["getActiveBehaviorManifest", "listBehaviorRoutes"],
    ),
    ("runtime/js/application.ts", &["quit"]),
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
