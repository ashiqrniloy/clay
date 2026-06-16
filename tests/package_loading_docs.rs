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

    // Phase 18.5 added the planned inventory entry clay.packages.loadPackage to track
    // the preferred target. Phase 18.6 task 5 ships the loader as a runtime-backed
    // facade export. While the loader is unimplemented the test pins the doc gap;
    // once it ships the gap-phrase assertions are skipped (task 8 updates the docs
    // to describe the implemented loader, then replaces this branch).
    let one_line_loader_is_implemented = facade.contains("export function loadPackage(")
        || facade.contains("export async function loadPackage(")
        || embedded_facade.contains("export function loadPackage(")
        || embedded_facade.contains("export async function loadPackage(");
    assert!(
        one_line_loader_is_implemented,
        "The generic one-line package loader must ship as a runtime-backed facade export (`export ... function loadPackage`)"
    );

    // The planned inventory entry must exist to track the loader.
    assert!(
        inventory.contains("clay.packages.loadPackage"),
        "clay.packages.loadPackage must have a planned inventory entry tracking the preferred one-line target"
    );

    // The loader is implemented (Phase 18.6). The docs must describe the
    // implemented resolver, the deny-by-default boundary, and the carried-
    // forward deferrals (non-@clay/*, hot reload, persistent enable state).
    // The old gap phrases are no longer present.
    if one_line_loader_is_implemented {
        // Authoritative reference docs must describe the resolver mechanics.
        for source in [&package_guide, &package_loading] {
            for phrase in [
                "Phase 18.6 shipped",
                "deny-by-default",
                "FirstPartyLoadEntryAllowlist",
                "PackageService",
                "loadEntry",
                "runtime-backed",
            ] {
                assert!(
                    source.contains(phrase),
                    "package docs must describe the implemented resolver with phrase `{phrase}`"
                );
            }
        }
        // The implementation wiki summarizes the resolver at a higher level.
        assert!(
            wiki.contains("Phase 18.6 implemented")
                && wiki.contains("loadPackage")
                && wiki.contains("deny-by-default"),
            "package loading wiki must summarize the Phase 18.6 implementation"
        );

        for source in [&package_guide, &package_loading, &wiki] {
            // Old gap phrases must be gone or replaced.
            assert!(
                !source.contains("generic one-line loader is not implemented yet"),
                "docs must not claim the loader is unimplemented after Phase 18.6"
            );
            assert!(
                !source.contains("generic loader/API gap"),
                "docs must not describe the resolver as a gap after Phase 18.6"
            );
            assert!(
                !source.contains("temporary validation/loading gap"),
                "docs must not describe the resolver as a temporary gap after Phase 18.6"
            );
        }

        // The inventory entry must be promoted to runtime-backed.
        assert!(
            inventory.contains("status = \"runtime-backed\"")
                && inventory.contains("registry_public = true")
                && inventory.contains("op_clay_packages_load_package_by_specifier"),
            "clay.packages.loadPackage inventory entry must be promoted to runtime-backed with concrete paths"
        );

        // The resolver op must be documented as the concrete implementation.
        assert!(
            package_loading.contains("op_clay_packages_load_package_by_specifier")
                && package_loading.contains("src/server/ops/packages.rs"),
            "package loading reference must document the concrete resolver op"
        );
        assert!(
            package_loading.contains("constrained first-party")
                && (package_loading.contains("Non-`@clay/*`")
                    || package_loading.contains("non-`@clay/*`")
                    || package_loading.contains("non-@clay/*"))
                && (package_loading.contains("Hot-reload")
                    || package_loading.contains("hot reload"))
                && (package_loading.contains("Persistent shared enable state")
                    || package_loading.contains("persistent shared enable state")),
            "package loading reference must document the carried-forward deferrals"
        );
        assert!(
            package_loading.contains("serverLoadPackage")
                && package_loading.contains("remains a lower-level validation helper"),
            "package loading reference must reframe serverLoadPackage as a helper, not a gap"
        );
    }
}

#[test]
fn package_default_load_gap_is_decision_log_backed_with_package_owned_fallback() {
    // Phase 18.5 (plans/028 Task 4) defers the generic loadPackage("@clay/*")
    // resolver with a decision-log-backed rationale and ships a clean
    // package-owned fallback entry. This test pins both halves so the gap
    // cannot drift back to fixture-only copied manifests.
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let wiki = read("docs/wiki/modules/package-loading.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let markdown_docs = read("packages/markdown/docs/index.md");
    let decision_log =
        read("decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md");
    let load_entry = read("packages/markdown/dist/load.js");
    let package_index = read("packages/markdown/dist/index.js");

    // The deferral is decision-log-backed.
    for source in [&package_loading, &wiki, &package_guide, &markdown_docs] {
        assert!(
            source.contains("2026-06-15-1015-defer-generic-loadpackage-first-party-resolver"),
            "docs must reference the loadPackage deferral decision log"
        );
    }
    for phrase in [
        "Defer the generic",
        "loadPackage(\"@clay/*\")",
        "ClayModuleLoader",
        "canonical_local_file",
        "security-critical",
        "markdownLoadMode",
        "Alternatives Considered",
        "explicitly_approved_by_user",
    ] {
        assert!(
            decision_log.contains(phrase),
            "loadPackage deferral decision log must record `{phrase}`"
        );
    }

    // The package-owned fallback entry exists, imports Clay facades directly,
    // and reuses loadMarkdownPackage without an inline manifest.
    assert!(
        load_entry.contains("export async function markdownLoadMode(options = {})"),
        "package load entry must export markdownLoadMode"
    );
    for facade in [
        "import { serverRegisterCommand } from \"clay:commands\"",
        "import { serverActivateMajorMode, serverRegisterModePattern } from \"clay:modes\"",
        "import { serverLoadPackage } from \"clay:packages\"",
        "import { serverRegisterParseHandler } from \"clay:parse\"",
    ] {
        assert!(
            load_entry.contains(facade),
            "package load entry must import Clay facade directly: {facade}"
        );
    }
    assert!(
        load_entry.contains("return loadMarkdownPackage(clay, options);")
            && !load_entry.contains("const markdownPackage = {"),
        "markdownLoadMode must reuse loadMarkdownPackage and must not declare an inline manifest"
    );
    assert!(
        package_index
            .contains("export { loadMarkdownPackage, markdownLoadMode } from \"./load.js\""),
        "package root index must re-export markdownLoadMode so the @clay/markdown fallback import resolves"
    );

    // The documented fallback is concise and uses implemented generic primitives.
    for source in [&package_guide, &markdown_docs] {
        assert!(
            source.contains("import { markdownLoadMode } from \"@clay/markdown\""),
            "docs must show the concise package-owned fallback import"
        );
        assert!(
            source.contains("await markdownLoadMode();"),
            "docs must show the concise package-owned fallback call"
        );
    }
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

#[test]
fn package_loading_docs_describe_implemented_resolver_and_carried_forward_deferrals() {
    // Phase 18.6 task 8: after the docs transition, the authoritative docs must
    // describe the implemented resolver (not a gap) and the carried-forward
    // deferrals (non-@clay/*, hot reload, persistent enable state).
    let package_loading = read("docs/reference/primitives/package-loading.md");
    let package_guide = read("docs/reference/packages/creating-packages.md");
    let wiki = read("docs/wiki/modules/package-loading.md");

    for source in [&package_loading, &package_guide, &wiki] {
        assert!(
            source.contains("Phase 18.6 shipped") || source.contains("Phase 18.6 implemented"),
            "docs must describe the Phase 18.6 implementation"
        );
        assert!(
            source.contains("deny-by-default"),
            "docs must document the deny-by-default boundary"
        );
        assert!(
            source.contains("loadPackage"),
            "docs must mention the loadPackage facade"
        );
    }
    // Carried-forward deferrals are explicitly stated as future work, not
    // current gaps.
    let has_non_clay_deferral = package_loading.contains("Non-`@clay/*`")
        || package_loading.contains("non-`@clay/*`")
        || package_loading.contains("non-@clay/*");
    assert!(
        has_non_clay_deferral
            && (package_loading.contains("Hot-reload") || package_loading.contains("hot reload"))
            && (package_loading.contains("Persistent shared enable state")
                || package_loading.contains("persistent shared enable state")),
        "package loading reference must document the carried-forward deferrals"
    );
    // Old gap language must not appear.
    assert!(
        !package_loading.contains("generic one-line loader is not implemented yet"),
        "package loading reference must not claim the loader is unimplemented"
    );
    assert!(
        !package_loading.contains("generic loader/API gap"),
        "package loading reference must not describe the resolver as a gap"
    );
    assert!(
        !package_loading.contains("temporary validation/loading gap"),
        "package loading reference must not describe the resolver as a temporary gap"
    );
}

#[test]
fn clay_packages_load_package_registry_entry_is_runtime_backed() {
    // Phase 18.6 task 8: the inventory entry must be promoted from planned to
    // runtime-backed with concrete paths and registry_public = true.
    let inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    let entry = inventory
        .split("[[api]]")
        .find(|block| block.contains("id = \"clay.packages.loadPackage\""))
        .expect("clay.packages.loadPackage inventory entry must exist");

    assert!(
        entry.contains("status = \"runtime-backed\""),
        "loadPackage inventory entry must be runtime-backed, got:\n{entry}"
    );
    assert!(
        entry.contains("registry_public = true"),
        "loadPackage inventory entry must be registry_public, got:\n{entry}"
    );
    // Concrete paths replace the old "planned:" placeholders.
    assert!(
        entry.contains("facade_path = \"runtime/js/packages.ts::loadPackage\""),
        "loadPackage inventory entry must have a concrete facade path"
    );
    assert!(
        entry.contains("src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier"),
        "loadPackage inventory entry must have a concrete deno_op_path"
    );
    assert!(
        entry.contains("src/server/ops/packages.rs::op_clay_packages_load_package_by_specifier"),
        "loadPackage inventory entry must have a concrete backing_rust path"
    );
    assert!(
        entry.contains("src/server/js_runtime.rs::ClayModuleLoader"),
        "loadPackage inventory entry must reference the module-loader gate"
    );
    assert!(
        entry.contains("docs/reference/clay-js-api/packages/load-package.md"),
        "loadPackage inventory entry must point to the dedicated Markdown doc"
    );
}

#[test]
fn load_package_introduces_no_hidden_configuration_key() {
    // configuration verification: loadPackage is an explicit action, not a config key.
    let facade = read("runtime/js/packages.ts");
    let resolver_op = read("src/server/ops/packages.rs");

    // The loadPackage facade takes only a specifier; no config/options/setting key.
    assert!(
        facade.contains("specifier: string"),
        "loadPackage facade must accept only specifier, not config/options/setting"
    );
    assert!(
        !facade.contains("defaultPackages"),
        "loadPackage facade must not introduce a defaultPackages config key"
    );

    // The resolver op takes only specifier JSON; no config/preferences/settings key.
    assert!(
        !resolver_op.contains("defaultPackages"),
        "resolver op must not introduce a defaultPackages config key"
    );

    // Package customization uses documented APIs, not loadPackage config keys.
    let package_loading = read("docs/reference/primitives/package-loading.md");
    assert!(
        package_loading.contains("setPackageOption")
            || package_loading.contains("clay.configuration.setPackageOption"),
        "package-loading.md must reference setPackageOption for customization"
    );
}
