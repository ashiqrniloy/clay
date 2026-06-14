use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn package_loading_doc_linked_from_indexes_and_marks_phase17_ready() {
    let docs_index = read("docs/index.md");
    let primitives_index = read("docs/reference/primitives/index.md");
    let backlog = read("docs/reference/primitives/backlog.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");

    assert!(
        docs_index.contains("reference/primitives/package-loading.md"),
        "docs/index.md must link the Phase 17 package loading reference"
    );
    assert!(
        primitives_index.contains("package-loading.md"),
        "primitives index must link package-loading.md"
    );
    for checklist in [
        "Package manifest validation supports package identity",
        "Package enable/load rejects invalid prefixes",
        "DocumentClassification",
        "MajorModeActivation",
        "CommandDeclaration",
        "Phase 17 explicitly hands off `DecorationRange` and `IncrementalParseUpdate`",
    ] {
        assert!(
            backlog.contains(&format!("- [x] {checklist}")) || backlog.contains(checklist),
            "Phase 17 backlog checklist must mark/readiness-cover {checklist}"
        );
    }
    assert!(package_loading.contains("Install, Enable, and Runtime Boundary"));
    assert!(package_loading.contains("Conflict Handling"));
    assert!(package_loading.contains("Phase 18 Handoff"));
}

#[test]
fn package_loading_keeps_validation_and_parsing_out_of_typing_hot_path() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_service = read("src/packages/service.rs");
    let parse_coordinator = read("src/server/parse_coordinator.rs");

    for phrase in ["typing", "paint", "layout", "scroll", "text-event"] {
        assert!(
            package_loading.contains(phrase),
            "package loading reference must document {phrase} hot-path exclusion"
        );
    }
    assert!(
        package_loading.contains("outside typing, paint, layout, scroll, and text-event handlers"),
        "package loading reference must keep validation/loading outside editor hot paths"
    );
    assert!(
        package_service.contains("none of these operations may be called from typing"),
        "package service source comment should preserve enable/install hot-path policy"
    );
    assert!(
        parse_coordinator.contains("does not wait for parse completion"),
        "parse coordinator must keep parsing off edit acknowledgement/typing paths"
    );
}

#[test]
fn package_default_init_js_loading_documents_one_line_path_or_current_gap() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let facade = read("runtime/js/packages.ts");
    let embedded_facade = read("src/server/js_runtime.rs");
    let inventory = read("docs/reference/clay-js-api/api-inventory.toml");

    for source in [&package_guide, &package_loading, &wiki] {
        assert!(
            source.contains("loadPackage(\"@clay/markdown\")"),
            "package loading docs/wiki must preserve the one-line explicit init.js target"
        );
    }

    let one_line_loader_is_implemented = facade.contains("export function loadPackage(")
        || embedded_facade.contains("export function loadPackage(")
        || inventory.contains("clay.packages.loadPackage");
    assert!(
        !one_line_loader_is_implemented,
        "Phase 18.4 verified the generic one-line package loader remains unimplemented; update this test when loadPackage ships"
    );

    for source in [&package_guide, &package_loading, &wiki] {
        for phrase in [
            "generic one-line loader is not implemented yet",
            "generic loader/API gap",
            "resolve an installed package specifier",
            "enable the package",
            "loadEntry",
            "temporary validation/loading gap",
        ] {
            assert!(
                source.contains(phrase),
                "docs/wiki must identify current one-line loader gap phrase `{phrase}`"
            );
        }
    }

    assert!(
        package_loading.contains("serverLoadPackage(packageJson)")
            && package_loading.contains("rather than end-user package installation"),
        "package loading reference must document serverLoadPackage as a validation helper/gap, not the default loader"
    );
    assert!(
        package_guide.contains("Do not present `serverLoadPackage` as ordinary end-user setup"),
        "package guide must keep the fixture-only serverLoadPackage fallback clear"
    );
    assert!(
        wiki.contains("not the end-user one-line package loader")
            && wiki.contains("fixture scripts")
            && wiki.contains("explicitly temporary"),
        "package loading wiki must document the default-loader gap"
    );
}

#[test]
fn package_customization_uses_documented_configuration_apis() {
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let configuration_wiki = read("docs/wiki/modules/configuration-runtime.md");

    for source in [&package_guide, &package_loading, &wiki, &configuration_wiki] {
        for phrase in [
            "setPackageOption",
            "serverSetLayoutOverride",
            "documented Clay JS APIs",
            "hidden JSON/TOML/ad hoc",
            "startup",
            "package-load",
            "configuration-change",
            "Masonry",
        ] {
            assert!(
                source.contains(phrase),
                "package customization docs/wiki must mention `{phrase}`"
            );
        }
    }

    for phrase in [
        "layout.defaultVisibility",
        "layout.defaultSlot",
        "input.default",
        "action.default",
        "themeTokenRemap",
        "slot",
        "visibility",
        "themeToken",
    ] {
        assert!(
            package_guide.contains(phrase) || configuration_wiki.contains(phrase),
            "customization docs must cover supported option/override `{phrase}`"
        );
    }
}

#[test]
fn phase18_3_package_loading_docs_cover_slot_ui_metadata_validation() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    for source in [&package_loading, &wiki] {
        for phrase in [
            "ui.panels",
            "ui.components",
            "ui.overlays",
            "themeTokens",
            "typed style variables",
            "action targets",
            "same-type core token fallbacks",
            "duplicate fixed slot claims",
            "bounded payload",
        ] {
            assert!(
                source.contains(phrase),
                "package loading docs/wiki must mention Phase 18.3 package UI validation phrase `{phrase}`"
            );
        }
        for prohibition in [
            "raw CSS",
            "client JavaScript",
            "direct Masonry",
            "native handles",
        ] {
            assert!(
                source.contains(prohibition),
                "package loading docs/wiki must preserve package UI non-authority `{prohibition}`"
            );
        }
    }
}

#[test]
fn phase18_4_package_loading_docs_cover_input_state_config_metadata_validation() {
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_security = read("docs/reference/primitives/package-security.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    for source in [&package_loading, &package_security, &package_guide, &wiki] {
        for phrase in [
            "input",
            "uiStateScopes",
            "layoutOverrides",
            "packageOptions",
            "registered actions",
            "package-configuration",
            "hidden-key rejection",
            "state-value rejection",
            "duplicate input",
            "duplicate UI state scope",
            "duplicate layout override",
            "duplicate package option",
            "package provenance",
        ] {
            assert!(
                source.contains(phrase),
                "package loading/security docs must mention Phase 18.4 metadata phrase `{phrase}`"
            );
        }
    }
}

#[test]
fn phase18_parse_decoration_apis_are_documented_without_raw_op_exposure() {
    let runtime = read("src/server/js_runtime.rs");
    let decorations = read("runtime/js/decorations.ts");
    let parse = read("runtime/js/parse.ts");
    let package_loading = read("docs/reference/primitives/package-loading.md");

    assert!(runtime.contains("\"clay:decorations\" => Some(CLAY_FACADE_DECORATIONS)"));
    assert!(runtime.contains("\"clay:parse\" => Some(CLAY_FACADE_PARSE)"));
    assert!(decorations.contains("serverPublishDecorations"));
    assert!(parse.contains("serverRegisterParseHandler"));
    assert!(
        package_loading.contains("planned-unavailable errors")
            || read("docs/reference/clay-js-api/api-inventory.toml")
                .contains("clay.decorations.serverPublishDecorations")
    );

    for (path, source) in [
        ("runtime/js/decorations.ts", decorations),
        ("runtime/js/parse.ts", parse),
    ] {
        assert!(
            !source.contains("Deno.core.ops."),
            "{path} must not expose raw Deno.core.ops dot calls"
        );
        for line in source
            .lines()
            .filter(|line| line.trim_start().starts_with("export "))
        {
            assert!(
                !line.contains("op_"),
                "{path} public exports must not expose raw op-shaped names: {line}"
            );
        }
    }
}
