use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct InventoryEntry {
    fields: BTreeMap<String, String>,
}

impl InventoryEntry {
    fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }

    fn has_key(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    fn is_public_registry_api(&self) -> bool {
        self.get("registry_public") == "true"
    }
}

fn inventory_entries() -> Vec<InventoryEntry> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/reference/clay-js-api/api-inventory.toml"
    );
    let text = fs::read_to_string(path).expect("read api inventory");
    let mut entries = Vec::new();
    let mut current: Option<BTreeMap<String, String>> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[api]]" {
            if let Some(fields) = current.take() {
                entries.push(InventoryEntry { fields });
            }
            current = Some(BTreeMap::new());
            continue;
        }
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        let fields = current
            .as_mut()
            .expect("inventory key/value appears inside an [[api]] table");
        fields.insert(key.to_string(), value.trim().trim_matches('"').to_string());
    }

    if let Some(fields) = current {
        entries.push(InventoryEntry { fields });
    }

    assert!(!entries.is_empty(), "inventory must contain API entries");
    entries
}

fn public_inventory_entries() -> Vec<InventoryEntry> {
    inventory_entries()
        .into_iter()
        .filter(InventoryEntry::is_public_registry_api)
        .collect()
}

fn markdown_frontmatter(path: &Path) -> BTreeMap<String, String> {
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("---"),
        "{path:?} must start with YAML frontmatter"
    );

    let mut fields = BTreeMap::new();
    for line in lines.by_ref() {
        if line == "---" {
            return fields;
        }
        if line.starts_with("  - ") || line.starts_with("    ") {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.to_string(), value.trim().trim_matches('"').to_string());
        }
    }
    panic!("{path:?} is missing closing frontmatter delimiter");
}

fn docs_index_registry_links() -> BTreeSet<String> {
    let index_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/index.md");
    let text = fs::read_to_string(&index_path).expect("read docs/index.md");
    let section = text
        .split("## Clay JS API Registry Source Files")
        .nth(1)
        .expect("docs/index.md has registry source section")
        .split("## Registry Rules")
        .next()
        .expect("docs/index.md has registry rules section");

    section
        .lines()
        .filter_map(|line| {
            line.split_once("](")
                .and_then(|(_, rest)| rest.split_once(')'))
        })
        .map(|(path, _)| format!("docs/{path}"))
        .collect()
}

fn parse_toml_string_list(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed == "[]" || !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Vec::new();
    }

    trimmed
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| item.trim().trim_matches('"'))
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn inventory_custom_property_names(value: &str) -> Vec<String> {
    parse_toml_string_list(value)
        .into_iter()
        .filter_map(|property| {
            property
                .split_once(':')
                .map(|(name, _)| name.to_string())
                .or_else(|| property.split_once('=').map(|(name, _)| name.to_string()))
        })
        .collect()
}

fn denied_configuration_authorities() -> [&'static str; 9] {
    [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package",
        "WASM",
        "client-side JavaScript",
    ]
}

fn is_configuration_security_relevant(entry: &InventoryEntry) -> bool {
    entry.get("authority").contains("configuration")
        || entry.get("category").contains("configuration")
        || entry.get("category") == "key-binding-management"
        || !parse_toml_string_list(entry.get("custom_properties")).is_empty()
}

fn contains_permission_validation_note(text: &str) -> bool {
    text.contains("server-side validation")
        || text.contains("server validation")
        || text.contains("runtime permission checks")
        || text.contains("required permissions are absent")
        || text.contains("valid lease")
        || text.contains("editable lease")
}

fn is_lower_camel_case(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_alphanumeric())
        && !name.contains('_')
}

fn facade_exports_function(facade_path: &str, export_name: &str) -> bool {
    let Some((path, symbol)) = facade_path.split_once("::") else {
        return false;
    };
    if symbol != export_name {
        return false;
    }
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| panic!("read facade source {source_path:?}: {err}"));
    source.contains(&format!("export function {export_name}"))
        || source.contains(&format!("export async function {export_name}"))
}

#[test]
fn phase15_sdui_observability_surfaces_remain_internal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sdui_source =
        fs::read_to_string(root.join("src/masonry_sdui.rs")).expect("read src/masonry_sdui.rs");
    let editor_source =
        fs::read_to_string(root.join("src/masonry_editor.rs")).expect("read src/masonry_editor.rs");

    assert!(
        sdui_source.contains("pub(crate) struct SduiObservableSnapshot"),
        "Phase 15 SDUI structural observations must stay crate-internal until a dedicated Clay JS API is designed"
    );
    assert!(
        sdui_source.contains("pub(crate) fn observable_snapshot"),
        "SduiNativeState::observable_snapshot must stay crate-internal"
    );
    assert!(
        editor_source.contains("pub(crate) struct SduiStatusObservation"),
        "Phase 15 GUI status observations must stay crate-internal until a dedicated Clay JS API is designed"
    );
    assert!(
        editor_source.contains("pub(crate) fn status_observation"),
        "EditorWidget::status_observation must stay crate-internal"
    );

    let public_sdui_ids: BTreeSet<_> = public_inventory_entries()
        .into_iter()
        .map(|entry| entry.get("id").to_string())
        .filter(|id| id.starts_with("clay.sdui."))
        .collect();
    let expected_public_sdui_ids = BTreeSet::from([
        "clay.sdui.defineButton".to_string(),
        "clay.sdui.defineEditorView".to_string(),
        "clay.sdui.defineFlex".to_string(),
        "clay.sdui.defineLabel".to_string(),
        "clay.sdui.defineList".to_string(),
        "clay.sdui.definePanel".to_string(),
        "clay.sdui.defineStack".to_string(),
        "clay.sdui.publishTree".to_string(),
    ]);
    assert_eq!(
        public_sdui_ids, expected_public_sdui_ids,
        "Phase 15 must not add a public SDUI observability/configuration API without docs, facade, op, and registry metadata"
    );
}

#[test]
fn phase17_configuration_apis_cover_reviewed_package_surfaces() {
    let expected = [
        (
            "clay.configuration.setPackageOption",
            ["packagePrefix", "option", "value", "source"].as_slice(),
        ),
        (
            "clay.configuration.setModePreference",
            [
                "modeId",
                "extensions",
                "mimeTypes",
                "defaultActivation",
                "source",
            ]
            .as_slice(),
        ),
        (
            "clay.configuration.setDecorationTheme",
            ["theme", "styleTokens", "contrastMode", "source"].as_slice(),
        ),
        (
            "clay.configuration.setParsePolicy",
            [
                "timeoutMs",
                "maxTimeoutMs",
                "parseUnits",
                "viewportPriority",
                "source",
            ]
            .as_slice(),
        ),
    ];
    let entries = inventory_entries();
    let overview = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/reference/clay-js-api/configuration.md"),
    )
    .expect("read configuration overview");

    for (id, custom_properties) in expected {
        let entry = entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing Phase 17 configuration API review entry {id}"));
        let expected_status = if id == "clay.configuration.setPackageOption" {
            "runtime-backed"
        } else {
            "planned"
        };
        assert_eq!(
            entry.get("status"),
            expected_status,
            "{id} status must match promoted validators/settings"
        );
        assert_eq!(entry.get("js_module"), "clay:configuration");
        let expected_registry_public = if id == "clay.configuration.setPackageOption" {
            "true"
        } else {
            "false"
        };
        assert_eq!(
            entry.get("registry_public"),
            expected_registry_public,
            "{id} registry visibility must match docs/op/runtime promotion"
        );
        assert!(
            entry.get("hot_path_policy").contains("not")
                || entry.get("hot_path_policy").contains("never"),
            "{id} must document that configuration stays off typing/rendering hot paths"
        );
        if id == "clay.configuration.setPackageOption" {
            assert!(
                entry.get("runtime_path").contains("runtime"),
                "{id} runtime path must record promoted runtime metadata"
            );
        } else {
            assert!(
                entry.get("runtime_path").contains("planned"),
                "{id} runtime path must remain explicit planned metadata"
            );
        }
        for property in custom_properties {
            assert!(
                inventory_custom_property_names(entry.get("custom_properties"))
                    .contains(&property.to_string()),
                "{id} custom_properties must include {property}"
            );
        }
        assert!(
            overview.contains(id),
            "configuration overview must record Phase 17 review result for {id}"
        );
    }

    assert!(
        overview.contains(
            "Package enable/disable remains a privileged package service or CLI operation"
        ),
        "configuration overview must record enable/disable deferral"
    );
}

#[test]
fn sdui_query_ui_state_decision_is_recorded() {
    let entries = inventory_entries();
    assert!(
        entries
            .iter()
            .all(|entry| entry.get("id") != "clay.sdui.queryUiState"),
        "clay:sdui.queryUiState must stay absent until promoted with full docs/registry/tests"
    );

    let overview = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/reference/clay-js-api/configuration.md"),
    )
    .expect("read configuration overview");
    assert!(overview.contains("`clay:sdui.queryUiState` remains deferred"));
    assert!(
        overview.contains("`SduiObservableSnapshot` and `SduiStatusObservation` stay internal")
    );
}

#[test]
fn package_configuration_cannot_grant_prohibited_authority() {
    let entries = inventory_entries();
    let configuration_ids = [
        "clay.configuration.setPackageOption",
        "clay.configuration.setModePreference",
        "clay.configuration.setDecorationTheme",
        "clay.configuration.setParsePolicy",
    ];

    for id in configuration_ids {
        let entry = entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing configuration entry {id}"));
        let security_notes = entry.get("security_notes");
        for denied in denied_configuration_authorities() {
            assert!(
                security_notes.contains(denied),
                "{id} security_notes must deny {denied} authority"
            );
        }
        for denied in ["raw Deno ops", "package installation", "enable/disable"] {
            assert!(
                security_notes.contains(denied),
                "{id} security_notes must deny {denied} authority"
            );
        }
    }
}

#[test]
fn phase18_2_shell_layout_configuration_surfaces_are_planned_or_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing shell/layout configuration inventory entry {id}"))
    };
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let shell_layout_doc =
        fs::read_to_string(root.join("docs/reference/primitives/shell-layout-strategy.md"))
            .expect("read shell/layout strategy");
    let docs_index = fs::read_to_string(root.join("docs/index.md")).expect("read docs index");
    let registry_links = docs_index_registry_links();

    assert!(
        docs_index.contains("reference/clay-js-api/configuration.md"),
        "docs/index.md must link the configuration overview"
    );
    assert!(
        docs_index.contains("reference/primitives/shell-layout-strategy.md"),
        "docs/index.md must link the shell/layout strategy"
    );

    for id in [
        "clay.configuration.setPackageOption",
        "clay.ui.serverSetLayoutOverride",
    ] {
        let entry = entry_by_id(id);
        assert_eq!(entry.get("status"), "runtime-backed", "{id} is promoted");
        assert_eq!(
            entry.get("registry_public"),
            "true",
            "{id} must enter the public registry after runtime validation and API docs ship"
        );
        assert!(
            configuration_doc.contains(id) || shell_layout_doc.contains(id),
            "configuration or shell/layout docs must record planned surface {id}"
        );
    }

    let layout_override = entry_by_id("clay.ui.serverSetLayoutOverride");
    assert_eq!(layout_override.get("js_module"), "clay:ui");
    assert_eq!(
        layout_override.get("documentation_path"),
        "docs/reference/clay-js-api/ui/server-set-layout-override.md"
    );
    assert_eq!(
        layout_override.get("deno_op"),
        "op_clay_ui_set_layout_override"
    );
    assert!(
        layout_override
            .get("runtime_path")
            .contains("configuration")
            && layout_override
                .get("hot_path_policy")
                .contains("no-hot-path"),
        "layout override planning metadata must keep configuration off Masonry/editor hot paths"
    );
    assert!(
        registry_links.contains("docs/reference/clay-js-api/ui/server-set-layout-override.md"),
        "runtime-backed clay:ui layout override docs must be linked as public registry docs after implementation"
    );
    assert!(
        registry_links.contains("docs/reference/clay-js-api/configuration/set-package-option.md"),
        "runtime-backed setPackageOption docs must be linked as public registry docs after concrete shell/layout settings ship"
    );
    for planned_ui_doc in [
        "docs/reference/clay-js-api/ui/server-register-working-area-layout.md",
        "docs/reference/clay-js-api/ui/server-register-pane-split-tree.md",
        "docs/reference/clay-js-api/ui/server-set-pane-slot-layout.md",
    ] {
        assert!(
            !registry_links.contains(planned_ui_doc),
            "planned clay:ui layout/state/config docs must not be linked as public registry docs before implementation: {planned_ui_doc}"
        );
    }
    for implemented_ui_doc in [
        "docs/reference/clay-js-api/ui/server-register-panel-contribution.md",
        "docs/reference/clay-js-api/ui/server-register-component-contribution.md",
        "docs/reference/clay-js-api/ui/server-register-transient-overlay-contribution.md",
        "docs/reference/clay-js-api/ui/server-register-input-contribution.md",
        "docs/reference/clay-js-api/ui/server-register-ui-state-scope.md",
        "docs/reference/clay-js-api/ui/server-register-theme-token.md",
    ] {
        assert!(
            registry_links.contains(implemented_ui_doc),
            "runtime-backed Phase 18.3 clay:ui contribution docs must be linked: {implemented_ui_doc}"
        );
    }

    assert!(
        root.join("runtime/js/ui.ts").exists(),
        "Phase 18.3 adds a clay:ui facade for contribution declarations while configuration/override APIs remain planned"
    );
    let js_runtime =
        fs::read_to_string(root.join("src/server/js_runtime.rs")).expect("read server JS runtime");
    assert!(
        js_runtime.contains("\"clay:ui\""),
        "Phase 18.3 runtime must allow importing runtime-backed clay:ui contribution APIs"
    );

    for required in [
        "Phase 18.2/18.3 shell/layout and package UI configuration review",
        "Phase 18.2 does **not** promote any new runtime-backed or user-visible shell/layout configuration API",
        "Phase 18.3 promotes package UI declaration APIs",
        "does not promote user-visible panel visibility, default-slot, component-style, theme-token override, or layout behavior configuration APIs",
        "`clay.ui.serverSetLayoutOverride` is the planned `PackageLayoutOverride` surface",
        "`clay.configuration.setPackageOption` remains the planned package-owned option surface",
        "Phase 18.3 promotes `clay.ui.serverRegisterThemeToken` to a runtime-backed package declaration API",
    ] {
        assert!(
            configuration_doc.contains(required),
            "configuration overview must record shell/layout planned-vs-implemented status text: {required}"
        );
    }
    for required in [
        "Configuration and User Override Surfaces",
        "Phase 18.2 still does not introduce a callable shell/layout configuration API",
        "`clay.ui.serverSetLayoutOverride` / `PackageLayoutOverride`",
        "`clay.configuration.setPackageOption`",
        "`clay.ui.serverRegisterThemeToken` / `PackageThemeTokenDeclaration`",
        "`clay.ui.serverRegisterUiStateScope` / `PackageUiStateScope`",
    ] {
        assert!(
            shell_layout_doc.contains(required),
            "shell/layout strategy must record planned configuration surface text: {required}"
        );
    }
}

#[test]
fn shell_layout_configuration_inventory_records_metadata() {
    let entries = inventory_entries();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing shell/layout configuration inventory entry {id}"))
    };

    for (id, required_properties) in [
        (
            "clay.ui.serverSetLayoutOverride",
            ["targetId", "property", "value", "source"].as_slice(),
        ),
        (
            "clay.configuration.setPackageOption",
            ["packagePrefix", "option", "value", "source"].as_slice(),
        ),
    ] {
        let entry = entry_by_id(id);
        assert_eq!(entry.get("status"), "runtime-backed");
        assert_eq!(entry.get("key_bindings"), "[]");
        assert_eq!(entry.get("registry_public"), "true");
        assert!(
            entry.get("authority").contains("configuration")
                || entry.get("category").contains("configuration"),
            "{id} must be classified as configuration-relevant"
        );
        assert!(
            entry.get("hot_path_policy").contains("not")
                || entry.get("hot_path_policy").contains("never")
                || entry.get("hot_path_policy").contains("no-hot-path"),
            "{id} must keep configuration work off hot paths"
        );
        for property in required_properties {
            assert!(
                inventory_custom_property_names(entry.get("custom_properties"))
                    .contains(&property.to_string()),
                "{id} custom_properties must include {property}"
            );
        }
        for denied in denied_configuration_authorities() {
            assert!(
                entry.get("security_notes").contains(denied),
                "{id} security_notes must deny {denied} authority"
            );
        }
        for denied in ["raw Deno ops", "enable/disable"] {
            assert!(
                entry.get("security_notes").contains(denied),
                "{id} security_notes must deny {denied} authority"
            );
        }
    }

    let layout_override = entry_by_id("clay.ui.serverSetLayoutOverride");
    assert_eq!(
        parse_toml_string_list(layout_override.get("permissions")),
        vec!["package-configuration".to_string()],
        "behavior-changing shell/layout overrides require package-configuration permission metadata"
    );
    for denied in [
        "hidden JSON/TOML layout keys",
        "native widget handles",
        "direct Masonry widgets",
        "raw CSS",
        "renderer callback",
    ] {
        assert!(
            layout_override.get("security_notes").contains(denied),
            "layout override inventory security notes must deny {denied}"
        );
    }
}

#[test]
fn phase18_4_clay_ui_and_configuration_api_inventory_status_matches_runtime() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let registry_links = docs_index_registry_links();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing Phase 18.4 API inventory entry {id}"))
    };

    for (id, module, export, op, rust, required_properties) in [
        (
            "clay.ui.serverRegisterInputContribution",
            "clay:ui",
            "serverRegisterInputContribution",
            "op_clay_ui_register_input_contribution",
            "src/server/ui.rs::PackageUiRegistry::register_input",
            [
                "id",
                "scope",
                "componentId",
                "pointer.click",
                "actionTargets",
            ]
            .as_slice(),
        ),
        (
            "clay.ui.serverRegisterUiStateScope",
            "clay:ui",
            "serverRegisterUiStateScope",
            "op_clay_ui_register_ui_state_scope",
            "src/server/ui.rs::PackageUiRegistry::register_ui_state_scope",
            [
                "id",
                "scope",
                "owner",
                "lifetime",
                "persistence",
                "valueSchema.kind",
            ]
            .as_slice(),
        ),
        (
            "clay.ui.serverSetLayoutOverride",
            "clay:ui",
            "serverSetLayoutOverride",
            "op_clay_ui_set_layout_override",
            "src/server/ui.rs::PackageUiRegistry::set_layout_override",
            ["targetId", "property", "value", "source"].as_slice(),
        ),
        (
            "clay.configuration.setPackageOption",
            "clay:configuration",
            "setPackageOption",
            "op_clay_configuration_set_package_option",
            "src/server/configuration.rs::ConfigurationRuntime::set_package_option",
            ["packagePrefix", "option", "value", "source"].as_slice(),
        ),
    ] {
        let entry = entry_by_id(id);
        assert_eq!(entry.get("status"), "runtime-backed", "{id} status");
        assert_eq!(entry.get("registry_public"), "true", "{id} visibility");
        assert_eq!(entry.get("js_module"), module);
        assert_eq!(entry.get("js_export"), export);
        assert_eq!(entry.get("deno_op"), op);
        assert_eq!(entry.get("backing_rust"), rust);
        assert_eq!(entry.get("key_bindings"), "[]");
        assert!(facade_exports_function(entry.get("facade_path"), export));
        assert!(registry_links.contains(entry.get("documentation_path")));
        assert!(
            root.join(entry.get("documentation_path")).exists(),
            "{id} documentation_path must exist"
        );
        assert!(
            entry.get("hot_path_policy").contains("no-hot-path")
                || entry.get("hot_path_policy").contains("not")
                || entry.get("hot_path_policy").contains("never")
        );
        for property in required_properties {
            assert!(
                inventory_custom_property_names(entry.get("custom_properties"))
                    .contains(&property.to_string()),
                "{id} custom_properties must include {property}"
            );
        }
        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "AI mutation",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
            "direct Masonry widgets",
            "native widget handles",
        ] {
            assert!(
                entry.get("security_notes").contains(denied),
                "{id} security_notes must deny {denied}"
            );
        }
    }

    for id in [
        "clay.ui.serverRegisterWorkingAreaLayout",
        "clay.ui.serverRegisterPaneSplitTree",
        "clay.ui.serverSetPaneSlotLayout",
    ] {
        let entry = entry_by_id(id);
        assert_eq!(entry.get("status"), "planned", "{id} remains deferred");
        assert_eq!(entry.get("registry_public"), "false");
        assert_eq!(entry.get("deno_op"), "op_clay_runtime_unavailable");
        assert_eq!(
            entry.get("documentation_path"),
            "docs/reference/primitives/shell-layout-strategy.md"
        );
    }
}

#[test]
fn shell_layout_planned_api_inventory_status_is_explicit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let strategy =
        fs::read_to_string(root.join("docs/reference/primitives/shell-layout-strategy.md"))
            .expect("read shell/layout strategy");
    let docs_index = fs::read_to_string(root.join("docs/index.md")).expect("read docs index");
    let registry_links = docs_index_registry_links();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing shell/layout planned API inventory entry {id}"))
    };

    let planned = [
        (
            "WorkingAreaLayout",
            "clay.ui.serverRegisterWorkingAreaLayout",
            "serverRegisterWorkingAreaLayout",
            "Register Working Area Layout",
        ),
        (
            "PaneSplitTree",
            "clay.ui.serverRegisterPaneSplitTree",
            "serverRegisterPaneSplitTree",
            "Register Pane Split Tree",
        ),
        (
            "PaneSlotLayout",
            "clay.ui.serverSetPaneSlotLayout",
            "serverSetPaneSlotLayout",
            "Set Pane Slot Layout",
        ),
    ];

    for (primitive, id, js_export, user_facing_name) in planned {
        let entry = entry_by_id(id);
        assert_eq!(entry.get("status"), "planned", "{id} remains planned");
        assert_eq!(
            entry.get("visibility"),
            "public",
            "{id} is a future public surface"
        );
        assert_eq!(
            entry.get("registry_public"),
            "false",
            "{id} must not be lookup-visible before implementation"
        );
        assert_eq!(
            entry.get("js_module"),
            "clay:ui",
            "{id} keeps the planned module specifier"
        );
        assert_eq!(
            entry.get("js_export"),
            js_export,
            "{id} keeps its planned callable/export name"
        );
        assert!(
            is_lower_camel_case(js_export),
            "{id} export {js_export} must be lower camel case"
        );
        assert_eq!(
            entry.get("user_facing_name"),
            user_facing_name,
            "{id} keeps its searchable user-facing name"
        );
        assert_eq!(
            entry.get("deno_op"),
            "op_clay_runtime_unavailable",
            "{id} must not claim a runtime op yet"
        );
        assert_eq!(
            entry.get("documentation_path"),
            "docs/reference/primitives/shell-layout-strategy.md"
        );
        assert_eq!(
            entry.get("key_bindings"),
            "[]",
            "planned {id} has no default key binding"
        );
        assert_ne!(
            entry.get("custom_properties"),
            "[]",
            "planned {id} must list future metadata/custom properties"
        );
        assert!(
            entry
                .get("facade_path")
                .contains(&format!("runtime/js/ui.ts::{js_export}"))
        );
        assert!(
            entry.get("backing_rust").starts_with("planned:"),
            "{id} must identify backing Rust as planned only"
        );
        assert!(
            entry.get("current_rust_owner").contains("planned:"),
            "{id} must not imply implemented Rust ownership"
        );
        assert!(
            entry.get("hot_path_policy").contains("no-hot-path"),
            "{id} must preserve no-hot-path policy"
        );
        assert!(
            entry.get("hot_path_policy").contains("hot"),
            "{id} must explain why planned shell/layout work stays off runtime hot paths"
        );
        assert!(
            strategy.contains(primitive),
            "strategy must link planned API {id} to primitive {primitive}"
        );
        assert!(
            strategy.contains(id),
            "strategy must name planned API stable ID {id}"
        );
        assert!(
            strategy.contains(js_export),
            "strategy must name planned API export {js_export}"
        );

        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "AI mutation",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
            "direct Masonry widgets",
            "native widget handles",
            "raw CSS",
        ] {
            assert!(
                entry.get("security_notes").contains(denied),
                "{id} security notes must deny {denied} authority"
            );
        }
        for forbidden_name in ["opClay", "op_clay_", "Rust", "Masonry"] {
            assert!(
                !js_export.contains(forbidden_name),
                "{id} JavaScript export must not expose implementation naming layer {forbidden_name}"
            );
        }
    }

    for (id, deno_op, backing) in [
        (
            "clay.ui.serverRegisterPanelContribution",
            "op_clay_ui_register_panel_contribution",
            "src/server/ui.rs::PackageUiRegistry::register_panel",
        ),
        (
            "clay.ui.serverRegisterComponentContribution",
            "op_clay_ui_register_component_contribution",
            "src/server/ui.rs::PackageUiRegistry::register_component",
        ),
        (
            "clay.ui.serverRegisterTransientOverlayContribution",
            "op_clay_ui_register_transient_overlay_contribution",
            "src/server/ui.rs::PackageUiRegistry::register_overlay",
        ),
        (
            "clay.ui.serverRegisterThemeToken",
            "op_clay_ui_register_theme_token",
            "src/server/ui.rs::PackageUiRegistry::register_theme_token",
        ),
    ] {
        let entry = entry_by_id(id);
        assert_eq!(
            entry.get("status"),
            "runtime-backed",
            "{id} is runtime-backed in Phase 18.3"
        );
        assert_eq!(
            entry.get("registry_public"),
            "true",
            "{id} is registry-public after the Phase 18.3 API docs task"
        );
        assert!(
            entry
                .get("documentation_path")
                .starts_with("docs/reference/clay-js-api/ui/"),
            "{id} must point at its public clay:ui API Markdown page"
        );
        assert_eq!(entry.get("deno_op"), deno_op);
        assert_eq!(
            entry.get("deno_op_path"),
            format!("src/server/ops/ui.rs::{deno_op}")
        );
        assert_eq!(entry.get("backing_rust"), backing);
        assert!(facade_exports_function(
            entry.get("facade_path"),
            entry.get("js_export")
        ));
        assert!(
            entry
                .get("security_notes")
                .contains("Runtime-backed Clay JS API")
        );
        assert!(entry.get("hot_path_policy").contains("no-hot-path"));
    }

    for required in [
        "Clay JS API Inventory Status",
        "runtime-backed `clay:ui` contribution facade",
        "module specifier groups imports",
        "lower-camel-case export",
        "stable registry ID",
        "user-facing name",
        "Package-owned shell/layout IDs",
        "must use package prefixes",
        "raw-op denial",
        "native-widget denial",
        "client-JS denial",
        "style-token constraint",
        "action-target validation",
    ] {
        assert!(
            strategy.contains(required),
            "strategy must record public API inventory status detail: {required}"
        );
    }
    for id in [
        "clay.ui.serverRegisterPanelContribution",
        "clay.ui.serverRegisterComponentContribution",
        "clay.ui.serverRegisterTransientOverlayContribution",
        "clay.ui.serverRegisterThemeToken",
    ] {
        let entry = entry_by_id(id);
        assert!(registry_links.contains(entry.get("documentation_path")));
        assert!(docs_index.contains(entry.get("documentation_path").trim_start_matches("docs/")));
    }
}

#[test]
fn phase18_2_shell_layout_api_inventory_status_matches_runtime() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing Phase 18.2 shell/layout inventory entry {id}"))
    };
    let strategy =
        fs::read_to_string(root.join("docs/reference/primitives/shell-layout-strategy.md"))
            .expect("read shell/layout strategy");
    let lib_source = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let shell_mod_source =
        fs::read_to_string(root.join("src/shell/mod.rs")).expect("read src/shell/mod.rs");
    let shell_layout_source =
        fs::read_to_string(root.join("src/shell/layout.rs")).expect("read src/shell/layout.rs");
    let masonry_shell_source =
        fs::read_to_string(root.join("src/masonry_shell.rs")).expect("read src/masonry_shell.rs");

    for (id, rust_owner) in [
        (
            "clay.ui.serverRegisterWorkingAreaLayout",
            "src/shell/layout.rs::WorkingAreaLayout",
        ),
        (
            "clay.ui.serverRegisterPaneSplitTree",
            "src/shell/layout.rs::PaneSplitTree",
        ),
        (
            "clay.ui.serverSetPaneSlotLayout",
            "src/shell/layout.rs::PaneSlotLayout",
        ),
    ] {
        let entry = entry_by_id(id);
        assert_eq!(entry.get("status"), "planned", "{id} remains planned");
        assert_eq!(entry.get("registry_public"), "false");
        assert_eq!(entry.get("deno_op"), "op_clay_runtime_unavailable");
        assert_eq!(
            entry.get("deno_op_path"),
            "src/server/ops/planned.rs::op_clay_runtime_unavailable"
        );
        assert_eq!(
            entry.get("documentation_path"),
            "docs/reference/primitives/shell-layout-strategy.md"
        );
        assert!(
            entry.get("facade_path").starts_with("runtime/js/ui.ts::"),
            "{id} must keep only a planned facade namespace"
        );
        assert!(
            entry.get("backing_rust").contains(rust_owner),
            "{id} must point planned backing metadata at the actual internal runtime owner {rust_owner}"
        );
        assert!(
            entry
                .get("current_rust_owner")
                .contains("internal runtime implemented"),
            "{id} current_rust_owner must say the Rust runtime exists while the public API is unavailable"
        );
        for required in [
            "server validation",
            "bounded",
            "client-side JavaScript",
            "raw Deno ops",
            "direct Masonry widgets",
            "native widget handles",
            "raw CSS",
        ] {
            assert!(
                entry.get("security_notes").contains(required)
                    || entry.get("hot_path_policy").contains(required),
                "{id} inventory metadata must preserve {required:?}"
            );
        }
    }

    assert!(
        root.join("runtime/js/ui.ts").exists(),
        "Phase 18.3 promotes callable clay:ui contribution facade while layout override APIs stay planned"
    );
    assert!(
        root.join("docs/reference/clay-js-api/ui").exists(),
        "Phase 18.3 API docs task adds public docs for runtime-backed clay:ui contribution APIs"
    );
    assert!(
        lib_source.contains("#[doc(hidden)]") && lib_source.contains("pub mod masonry_shell;"),
        "masonry_shell is Rust-public only for the package binary boundary and must stay hidden from public Rust docs"
    );
    assert!(shell_mod_source.contains("pub(crate) mod layout"));
    for forbidden_public_layout in [
        "pub struct WorkingAreaLayout",
        "pub enum PaneSplitNode",
        "pub struct PaneSplitTree",
        "pub struct PaneSlotLayout",
        "pub struct WorkingAreaLayoutUpdate",
        "pub struct WorkingAreaLayoutObservation",
    ] {
        assert!(
            !shell_layout_source.contains(forbidden_public_layout),
            "{forbidden_public_layout} must stay crate-private unless a Clay JS API is promoted"
        );
    }

    assert!(masonry_shell_source.contains("#[doc(hidden)]"));
    assert!(masonry_shell_source.contains("pub struct ClayShellWidget"));
    for binary_only_method in [
        "pub fn single_editor",
        "pub fn editor_widget_id",
        "pub fn focus_fallback_widget_id",
    ] {
        assert!(
            masonry_shell_source.contains(binary_only_method),
            "expected binary-only native shell method {binary_only_method}"
        );
    }
    for internal_method in ["apply_layout_update", "observable_snapshot"] {
        assert!(
            masonry_shell_source.contains(&format!("pub(crate) fn {internal_method}")),
            "{internal_method} must remain crate-internal"
        );
        assert!(
            !masonry_shell_source.contains(&format!("pub fn {internal_method}")),
            "{internal_method} must not become a public Rust API without Clay JS facade/docs/registry coverage"
        );
    }

    for required in [
        "Rust visibility audit",
        "introduces no new public server-side Rust shell/layout functions",
        "Rust-public only for the Cargo package's binary/library boundary",
        "not backed by a `deno_core` op",
        "generated registry entry",
        "remain `pub(crate)`",
    ] {
        assert!(
            strategy.contains(required),
            "shell/layout strategy must record Phase 18.2 API audit rationale: {required}"
        );
    }
}

#[test]
fn phase18_2_shell_docs_reject_hidden_layout_config_keys() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let shell_layout_doc =
        fs::read_to_string(root.join("docs/reference/primitives/shell-layout-strategy.md"))
            .expect("read shell/layout strategy");
    let package_guide =
        fs::read_to_string(root.join("docs/reference/packages/creating-packages.md"))
            .expect("read package guide");

    for (name, text) in [
        ("configuration overview", configuration_doc.as_str()),
        ("shell/layout strategy", shell_layout_doc.as_str()),
        ("package guide", package_guide.as_str()),
    ] {
        assert!(
            text.contains("hidden JSON/TOML/ad hoc")
                || text.contains("Do not add hidden JSON/TOML/ad hoc keys"),
            "{name} must reject hidden JSON/TOML/ad hoc shell/layout configuration keys"
        );
        assert!(
            text.contains("documented Clay JS APIs"),
            "{name} must route shell/layout configuration through documented Clay JS APIs"
        );
        for denied in ["raw CSS", "native widget", "client-side JavaScript"] {
            assert!(
                text.contains(denied),
                "{name} must deny {denied} authority for shell/layout configuration"
            );
        }
    }

    for required_key in [
        "layout.preview.defaultSlot",
        "layout.preview.defaultVisibility",
        "preview.position",
        "preview.defaultVisibility",
        "theme.markdown.heading.1",
    ] {
        assert!(
            configuration_doc.contains(required_key) && shell_layout_doc.contains(required_key),
            "configuration and shell/layout docs must identify planned key {required_key} as API-mediated, not hidden config"
        );
    }
    for denied in [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace mutation",
        "package enable/disable",
        "WASM",
        "raw Deno ops",
        "native widget handles",
        "direct Masonry widgets",
        "renderer callbacks",
        "client-side JavaScript",
    ] {
        assert!(
            configuration_doc.contains(denied) && shell_layout_doc.contains(denied),
            "shell/layout configuration docs must deny {denied} authority"
        );
    }
}

#[test]
fn open_file_dialog_keybinding_is_configured_through_init_js() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture =
        fs::read_to_string(root.join("tests/fixtures/configuration/windows-markdown-open/init.js"))
            .expect("read Windows Markdown open fixture");
    let bind_key_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/keybindings/bind-key.md"))
            .expect("read bindKey docs");
    let editor_widget_source =
        fs::read_to_string(root.join("src/masonry_editor.rs")).expect("read editor widget");
    let keybinding_source = fs::read_to_string(root.join("src/server/ops/keybindings.rs"))
        .expect("read keybinding ops");

    for text in [&fixture, &bind_key_doc] {
        assert!(text.contains("import { bindKey } from \"clay:keybindings\";"));
        assert!(text.contains(
            "bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\", { scope: \"editor\" });"
        ));
    }
    assert!(keybinding_source.contains("clay.documents.clientOpenFileDialog"));
    assert!(keybinding_source.contains("RoutingPolicy::ClientUiCommand"));
    assert!(
        !editor_widget_source.contains("Ctrl+O"),
        "EditorWidget must not hard-code Ctrl+O; the binding must come from init.js/behavior manifests"
    );
}

#[test]
fn open_file_dialog_configuration_does_not_grant_broad_filesystem_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let bind_key_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/keybindings/bind-key.md"))
            .expect("read bindKey docs");
    let fixture =
        fs::read_to_string(root.join("tests/fixtures/configuration/windows-markdown-open/init.js"))
            .expect("read Windows Markdown open fixture");

    for text in [&configuration_doc, &bind_key_doc] {
        for denied in denied_configuration_authorities() {
            assert!(
                text.contains(denied),
                "open-dialog configuration docs must deny {denied} authority"
            );
        }
    }
    for required in [
        "selected-file-only server validation/granting",
        "does not grant arbitrary filesystem authority",
        "workspace expansion",
        "raw Deno ops",
    ] {
        assert!(
            configuration_doc.contains(required) || bind_key_doc.contains(required),
            "Phase 19 configuration docs must cover `{required}`"
        );
    }
    assert!(
        !fixture.contains("Deno.core.ops")
            && !fixture.contains("rawOp")
            && !fixture.contains("clientOpenFileDialog(")
            && !fixture.contains("dialogFilter")
            && !fixture.contains("defaultDirectory"),
        "fixture must not expose hidden dialog configuration keys or callable client hooks"
    );
}

#[test]
fn phase19_hot_reload_configuration_review_rejects_hidden_reload_keys() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let runtime_doc = fs::read_to_string(root.join("docs/wiki/modules/embedded-js-runtime.md"))
        .expect("read embedded runtime wiki");
    let entries = inventory_entries();

    for required in [
        "Phase 19 persistent-runtime hot reload configuration review",
        "did **not** promote a new user-facing reload setting",
        "internal/developer-only server lifecycle primitive",
        "not callable from `~/.config/clay/init.js`",
        "hidden JSON/TOML/ad hoc keys",
        "startup or explicit developer-triggered reload work only",
        "deny-by-default module resolution",
        "sanitized diagnostics",
    ] {
        assert!(
            configuration_doc.contains(required),
            "configuration overview must document Phase 19 hot reload config rule `{required}`"
        );
    }
    for forbidden_key in [
        "hotReload",
        "hot_reload",
        "reloadOnSave",
        "autoReload",
        "reloadPackages",
        "reloadConfiguration",
    ] {
        assert!(
            configuration_doc.contains(forbidden_key),
            "configuration overview must explicitly reject hidden key `{forbidden_key}`"
        );
        assert!(
            entries
                .iter()
                .all(|entry| !entry.get("id").contains(forbidden_key)
                    && !entry.get("custom_properties").contains(forbidden_key)),
            "API inventory must not add hidden reload config key `{forbidden_key}`"
        );
    }
    assert!(runtime_doc.contains("IpcServer::trigger_developer_hot_reload"));
    assert!(entries.iter().all(|entry| {
        !(entry.get("js_module") == "clay:configuration"
            && entry.get("id").to_ascii_lowercase().contains("reload"))
    }));
    for denied in denied_configuration_authorities() {
        assert!(
            configuration_doc.contains(denied),
            "hot reload configuration docs must deny {denied} authority"
        );
    }
}

#[test]
fn phase19_hot_reload_has_no_public_clay_js_api_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let server_mod = fs::read_to_string(root.join("src/server/mod.rs")).expect("read server mod");
    let runtime_js = fs::read_to_string(root.join("runtime/js/application.ts"))
        .unwrap_or_else(|_| String::new())
        + &fs::read_to_string(root.join("runtime/js/packages.ts")).expect("read packages facade")
        + &fs::read_to_string(root.join("runtime/js/configuration.ts"))
            .expect("read config facade");
    let registry = fs::read_to_string(root.join("docs/generated/clay-js-api-registry.json"))
        .expect("read generated registry");
    let entries = inventory_entries();

    for rust_surface in [
        "pub struct ReloadedDocumentRefresh",
        "pub struct RuntimeReloadOutcome",
        "pub async fn trigger_developer_hot_reload",
    ] {
        let position = server_mod
            .find(rust_surface)
            .unwrap_or_else(|| panic!("server mod must contain {rust_surface}"));
        let prefix_start = position.saturating_sub(80);
        let prefix = &server_mod[prefix_start..position];
        assert!(
            prefix.contains("#[doc(hidden)]"),
            "{rust_surface} must remain doc-hidden until promoted to a public Clay JS API"
        );
    }
    assert!(server_mod.contains("pub(crate) async fn reload_runtime_generation"));
    assert!(!runtime_js.contains("triggerDeveloperHotReload"));
    assert!(!runtime_js.contains("reloadConfiguration"));
    assert!(!runtime_js.contains("reloadPackages"));
    assert!(entries.iter().all(|entry| {
        !(entry.get("id").starts_with("clay.runtime.")
            || (entry.get("id").to_ascii_lowercase().contains("reload")
                && entry.get("id") != "clay.documents.serverReloadDocument"))
    }));
    assert!(!registry.contains("triggerDeveloperHotReload"));
    assert!(!registry.contains("clay.runtime.reload"));
}

#[test]
fn configuration_docs_cover_open_file_dialog_defaults_or_options() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let bind_key_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/keybindings/bind-key.md"))
            .expect("read bindKey docs");
    let dialog_source =
        fs::read_to_string(root.join("src/client/file_dialog.rs")).expect("read dialog source");
    let entries = inventory_entries();
    let bind_key = entries
        .iter()
        .find(|entry| entry.get("id") == "clay.keybindings.bindKey")
        .expect("bindKey inventory entry");

    for property in ["key", "command", "scope", "when"] {
        assert!(
            inventory_custom_property_names(bind_key.get("custom_properties"))
                .contains(&property.to_string()),
            "bindKey custom_properties must include {property}"
        );
    }
    for required in [
        "Phase 19 Windows open-dialog configuration review",
        "did **not** promote a new dialog-settings configuration API",
        "fixed defaults, not hidden `init.js` keys",
        ".md",
        ".markdown",
        ".mdown",
        "all-files fallback",
        "No default `Ctrl+O` shortcut in Rust",
    ] {
        assert!(
            configuration_doc.contains(required),
            "configuration overview must document `{required}`"
        );
    }
    assert!(bind_key_doc.contains("fixed Markdown/all-files filter defaults"));
    assert!(dialog_source.contains("*.md"));
    assert!(dialog_source.contains("*.markdown"));
    assert!(dialog_source.contains("*.mdown"));
    assert!(dialog_source.contains("*.*"));
    assert!(
        entries.iter().all(|entry| {
            !(entry.get("id").starts_with("clay.configuration.")
                && entry.get("id").contains("FileDialog"))
        }),
        "Phase 19 must not add hidden clay.configuration FileDialog settings before real user-tunable settings exist"
    );
}

#[test]
fn client_open_file_dialog_api_is_documented_indexed_and_facade_backed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = public_inventory_entries();
    let entry = entries
        .iter()
        .find(|entry| entry.get("id") == "clay.documents.clientOpenFileDialog")
        .expect("client open file dialog API inventory entry");
    let linked_paths = docs_index_registry_links();
    let doc_path = entry.get("documentation_path");
    let doc_text = fs::read_to_string(root.join(doc_path)).expect("read client dialog API doc");

    assert_eq!(entry.get("js_module"), "clay:documents");
    assert_eq!(entry.get("js_export"), "clientOpenFileDialog");
    assert_eq!(entry.get("status"), "runtime-backed-command");
    assert_eq!(entry.get("key_bindings"), "[]");
    assert_eq!(entry.get("custom_properties"), "[]");
    assert!(linked_paths.contains(doc_path));
    assert!(facade_exports_function(
        entry.get("facade_path"),
        entry.get("js_export")
    ));

    for required in [
        "Stable ID: `clay.documents.clientOpenFileDialog`",
        "Module/export: `clay:documents` / `clientOpenFileDialog`",
        "bindKey(\"Ctrl+O\", clientOpenFileDialog(), { scope: \"editor\" })",
        "fixed Markdown filters",
        "Windows-only native dialog support",
        "selected-file-only server validation",
        "ordinary editing remains delta-based",
        "background, viewport-bounded work",
    ] {
        assert!(
            doc_text.contains(required),
            "client open file dialog API docs must mention {required:?}"
        );
    }
}

#[test]
fn open_dialog_api_security_notes_cover_selected_file_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc_text = fs::read_to_string(
        root.join("docs/reference/clay-js-api/documents/client-open-file-dialog.md"),
    )
    .expect("read client open file dialog API doc");
    let entry = public_inventory_entries()
        .into_iter()
        .find(|entry| entry.get("id") == "clay.documents.clientOpenFileDialog")
        .expect("client open file dialog API inventory entry");

    for required in [
        "native dialog execution requires explicit user key routing",
        "server-validated as single-file grants",
        "sanitizes diagnostics",
        "grants at most that selected file",
        "raw Deno ops",
        "broad filesystem/workspace authority",
    ] {
        assert!(
            doc_text.contains(required) || entry.get("security_notes").contains(required),
            "open-dialog API security notes must cover {required:?}"
        );
    }
    for denied in denied_configuration_authorities() {
        assert!(doc_text.contains(denied));
        assert!(entry.get("security_notes").contains(denied));
    }
}

#[test]
fn api_inventory_has_required_fields() {
    let entries = inventory_entries();
    let required_fields = [
        "id",
        "category",
        "visibility",
        "status",
        "js_module",
        "js_export",
        "user_facing_name",
        "authority",
        "runtime_path",
        "hot_path_policy",
        "facade_path",
        "backing_rust",
        "deno_op",
        "deno_op_path",
        "documentation_path",
        "key_bindings",
        "custom_properties",
        "permissions",
        "security_notes",
        "current_rust_owner",
        "registry_public",
    ];

    let mut ids = BTreeSet::new();
    for entry in &entries {
        let id = entry.get("id");
        assert!(!id.is_empty(), "inventory entry is missing id: {entry:?}");
        assert!(ids.insert(id.to_string()), "duplicate inventory id {id}");

        for field in required_fields {
            assert!(
                entry.has_key(field),
                "{id} is missing required field {field}"
            );
        }

        if entry.is_public_registry_api() {
            for field in required_fields {
                let value = entry.get(field);
                let may_be_empty_list =
                    matches!(field, "key_bindings" | "custom_properties" | "permissions");
                assert!(
                    may_be_empty_list || !value.trim().is_empty(),
                    "public inventory entry {id} has empty required field {field}"
                );
            }
            assert!(
                entry.get("id").starts_with("clay."),
                "public inventory entry {id} must use the clay.* stable ID namespace"
            );
            assert!(
                entry
                    .get("documentation_path")
                    .starts_with("docs/reference/clay-js-api/"),
                "public inventory entry {id} must point at Clay JS API reference docs"
            );
            assert!(
                entry
                    .get("security_notes")
                    .contains("does not grant filesystem"),
                "public inventory entry {id} must explicitly state authority not granted"
            );
        }
    }
}

#[test]
fn api_inventory_classifies_current_editor_behavior() {
    let entries = inventory_entries();
    let categories: BTreeSet<_> = entries
        .iter()
        .filter(|entry| entry.is_public_registry_api())
        .map(|entry| entry.get("category").to_string())
        .collect();
    let required_categories = [
        "text-insertion",
        "newline",
        "backspace-delete",
        "cursor-movement",
        "selection",
        "scrolling",
        "resize-viewport",
        "cursor-style-customization",
        "key-binding-management",
        "configuration-entrypoint",
        "behavior-manifest-routing",
        "lease-read-only-state",
        "escape-quit-application-actions",
    ];

    for category in required_categories {
        assert!(
            categories.contains(category),
            "inventory is missing required Phase 7 functionality category {category}"
        );
    }

    let hot_path_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.get("runtime_path").contains("hot-path"))
        .collect();
    assert!(
        hot_path_entries
            .iter()
            .any(|entry| entry.get("hot_path_policy").contains("asynchronously")),
        "hot-path inventory must record that ordinary editing is async to the server"
    );
}

#[test]
fn api_inventory_primitive_gate_entries_are_implemented_or_planned() {
    let entries = inventory_entries();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing primitive gate API inventory entry {id}"))
    };

    for id in [
        "clay.packages.serverValidatePackageManifest",
        "clay.packages.serverValidatePackagePermissions",
        "clay.packages.serverLoadPackage",
        "clay.modes.serverRegisterModePattern",
        "clay.modes.serverClassifyDocument",
        "clay.modes.serverActivateMajorMode",
        "clay.commands.serverRegisterCommand",
        "clay.commands.serverListCommands",
        "clay.decorations.serverPublishDecorations",
        "clay.parse.serverRegisterParseHandler",
    ] {
        let entry = entry_by_id(id);
        assert_eq!(
            entry.get("status"),
            "runtime-backed",
            "{id} must be marked implemented"
        );
        assert_eq!(
            entry.get("registry_public"),
            "true",
            "{id} must be public in the generated registry"
        );
        assert!(
            entry.get("hot_path_policy").contains("not")
                || entry.get("hot_path_policy").contains("never")
                || entry.get("hot_path_policy").contains("must not"),
            "{id} must document that primitive gate work is outside ordinary hot paths"
        );
    }

    let select_manifest = entry_by_id("clay.modes.serverSelectDocumentManifest");
    assert_eq!(select_manifest.get("status"), "planned");
    assert_eq!(select_manifest.get("registry_public"), "false");
    assert_eq!(
        select_manifest.get("deno_op"),
        "op_clay_runtime_unavailable"
    );

    {
        let id = "clay.folding.serverPublishFoldingRanges";
        let entry = entry_by_id(id);
        assert_eq!(
            entry.get("status"),
            "planned",
            "{id} must remain a planned API"
        );
        assert_eq!(
            entry.get("registry_public"),
            "false",
            "{id} must not be generated before implementation"
        );
    }
}

#[test]
fn api_inventory_does_not_mark_internal_details_public() {
    let entries = inventory_entries();
    let internal_ids = [
        "internal.editor.buffer",
        "internal.editor.layoutPaint",
        "internal.protocol.dto",
        "internal.server.ipcRuntime",
    ];

    for internal_id in internal_ids {
        let entry = entries
            .iter()
            .find(|entry| entry.get("id") == internal_id)
            .unwrap_or_else(|| panic!("missing internal inventory entry {internal_id}"));
        assert_eq!(
            entry.get("visibility"),
            "internal",
            "{internal_id} must be marked internal"
        );
        assert_eq!(
            entry.get("registry_public"),
            "false",
            "{internal_id} must not be included in public registry generation"
        );
        assert!(
            entry.get("js_module").is_empty() && entry.get("js_export").is_empty(),
            "{internal_id} must not expose a Clay JS module/export"
        );
    }
}

#[test]
fn inventory_future_ops_are_not_user_facing_exports() {
    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let js_export = entry.get("js_export");
        let deno_op = entry.get("deno_op");

        assert!(
            !js_export.starts_with("op_") && !js_export.starts_with("opClay"),
            "public inventory entry {id} exposes raw op-shaped JS export {js_export}"
        );
        assert!(
            deno_op.starts_with("op_clay_"),
            "public inventory entry {id} must map to an explicit future op_clay_* wrapper, got {deno_op}"
        );
        assert_ne!(
            js_export, deno_op,
            "public inventory entry {id} must not make the future op wrapper the user-facing JS export"
        );
    }
}

#[test]
fn clay_js_api_docs_have_required_frontmatter_and_body_sections() {
    let required_frontmatter = [
        "id",
        "kind",
        "js_module",
        "js_export",
        "js_facade",
        "backing_rust",
        "deno_op",
        "deno_op_path",
        "name",
        "user_facing_name",
        "summary",
        "owner",
        "phase",
        "visibility",
        "permissions",
        "key_bindings",
        "custom_properties",
        "security",
        "agent_guidance",
        "lookup_tags",
        "app_visible",
        "help_visible",
        "stability",
        "async",
    ];
    let required_sections = [
        "## Summary",
        "## Description",
        "## When to use",
        "## JavaScript usage",
        "## Example",
        "## Options",
        "## Key bindings",
        "## Custom properties",
        "## Return and async behavior",
        "## Errors",
        "## Permissions and security",
        "## Agent guidance",
        "## Backing implementation",
        "## Lookup metadata",
    ];

    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        assert!(
            doc_path.exists(),
            "{id} documentation file is missing: {doc_path:?}"
        );

        let fields = markdown_frontmatter(&doc_path);
        for field in required_frontmatter {
            assert!(
                fields.contains_key(field),
                "{id} documentation is missing frontmatter field {field}"
            );
        }
        assert_eq!(fields.get("kind").map(String::as_str), Some("clay-js-api"));
        assert_eq!(fields.get("visibility").map(String::as_str), Some("public"));
        assert_eq!(
            fields.get("stability").map(String::as_str),
            Some(entry.get("status")),
            "{id} documentation stability must match inventory status"
        );
        assert!(
            fields
                .get("security")
                .is_some_and(|security| security.contains("does not grant filesystem")),
            "{id} documentation must state authority not granted"
        );
        assert!(
            fields.get("lookup_tags").is_some_and(|tags| tags != "[]"),
            "{id} documentation must include lookup tags"
        );

        let text = fs::read_to_string(&doc_path).expect("read API doc");
        assert!(
            text.contains(&format!("# {}", entry.get("js_export"))),
            "{id} documentation must title the JS export"
        );
        for section in required_sections {
            assert!(
                text.contains(section),
                "{id} documentation is missing {section}"
            );
        }
        assert!(
            text.contains("```ts") && text.contains(entry.get("js_module")),
            "{id} documentation must include a TypeScript usage example"
        );
    }
}

#[test]
fn docs_index_links_all_public_inventory_docs() {
    let linked_paths = docs_index_registry_links();
    for entry in public_inventory_entries() {
        let doc_path = entry.get("documentation_path");
        assert!(
            linked_paths.contains(doc_path),
            "docs/index.md must link public API documentation for {} at {doc_path}",
            entry.get("id")
        );
    }
}

#[test]
fn api_docs_match_inventory_ids_and_exports() {
    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let fields = markdown_frontmatter(&doc_path);
        for (doc_field, inventory_field) in [
            ("id", "id"),
            ("js_module", "js_module"),
            ("js_export", "js_export"),
            ("js_facade", "facade_path"),
            ("backing_rust", "backing_rust"),
            ("deno_op", "deno_op"),
            ("deno_op_path", "deno_op_path"),
            ("user_facing_name", "user_facing_name"),
        ] {
            assert_eq!(
                fields.get(doc_field).map(String::as_str),
                Some(entry.get(inventory_field)),
                "{id} documentation frontmatter field {doc_field} must match inventory field {inventory_field}"
            );
        }
    }
}

#[test]
fn clay_js_api_inventory_docs_and_index_are_consistent() {
    let public_entries = public_inventory_entries();
    let inventory_doc_paths: BTreeSet<_> = public_entries
        .iter()
        .map(|entry| entry.get("documentation_path").to_string())
        .collect();
    let linked_paths = docs_index_registry_links();

    assert_eq!(
        linked_paths, inventory_doc_paths,
        "docs/index.md Clay JS API Registry Source Files must exactly match public api-inventory.toml entries; add/remove the named link instead of relying on generated artifacts"
    );

    for entry in public_entries {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let doc_text = fs::read_to_string(&doc_path).expect("read API doc");
        assert!(
            facade_exports_function(entry.get("facade_path"), entry.get("js_export")),
            "{id} facade_path {} must point at a file exporting {}",
            entry.get("facade_path"),
            entry.get("js_export")
        );
        assert!(
            doc_text.contains(entry.get("facade_path"))
                && doc_text.contains(entry.get("deno_op_path"))
                && doc_text.contains(entry.get("backing_rust")),
            "{} must document facade, future op path, and backing Rust owner in {}",
            id,
            entry.get("documentation_path")
        );
    }
}

#[test]
fn clay_js_api_names_follow_project_conventions() {
    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let js_module = entry.get("js_module");
        let js_export = entry.get("js_export");
        let expected_id = format!(
            "clay.{}.{}",
            js_module
                .strip_prefix("clay:")
                .unwrap_or_else(|| panic!("{id} js_module must start with clay:, got {js_module}")),
            js_export
        );

        assert_eq!(
            id, expected_id,
            "{id} stable ID must be clay.<module>.<export>"
        );
        assert!(
            is_lower_camel_case(js_export),
            "{id} js_export {js_export} must be flat lower-camel-case"
        );
        assert!(
            !js_export.contains("clay")
                && !js_export.contains("Clay")
                && (!js_export.contains("op") || js_export == "serverRegisterUiStateScope")
                && !js_export.contains("Rust"),
            "{id} js_export {js_export} must not expose Clay/project, raw op, or Rust implementation names"
        );

        if matches!(
            entry.get("category"),
            "text-insertion"
                | "newline"
                | "backspace-delete"
                | "cursor-movement"
                | "selection"
                | "scrolling"
                | "resize-viewport"
                | "cursor-style-customization"
                | "lease-read-only-state"
        ) {
            assert!(
                js_export.starts_with("server") || js_export.starts_with("client"),
                "{id} editor/document state API export {js_export} must carry server/client authority marker"
            );
        }
    }
}

#[test]
fn public_api_docs_include_security_keybinding_and_custom_properties() {
    let denied_authorities = denied_configuration_authorities();

    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let fields = markdown_frontmatter(&doc_path);
        let doc_text = fs::read_to_string(&doc_path).expect("read API doc");

        assert!(
            fields.contains_key("key_bindings"),
            "{id} is missing key_bindings frontmatter"
        );
        assert!(
            fields.contains_key("custom_properties"),
            "{id} is missing custom_properties frontmatter"
        );
        assert!(
            doc_text.contains("## Key bindings") && doc_text.contains("## Custom properties"),
            "{id} must include discoverability sections for key bindings and custom properties"
        );

        for key_binding in parse_toml_string_list(entry.get("key_bindings")) {
            assert!(
                doc_text.contains(&key_binding),
                "{id} documentation must mention inventory key binding {key_binding}"
            );
        }
        for property in inventory_custom_property_names(entry.get("custom_properties")) {
            assert!(
                doc_text.contains(&format!("- `{property}`"))
                    && doc_text.contains(&format!("- name: {property}")),
                "{id} documentation must include custom property metadata for {property} in frontmatter and body"
            );
        }

        let security = fields
            .get("security")
            .map(String::as_str)
            .unwrap_or_default();
        for denied in denied_authorities {
            assert!(
                security.contains(denied) && doc_text.contains(denied),
                "{id} security metadata/body must explicitly say it does not grant {denied} authority"
            );
        }
    }
}

#[test]
fn configuration_docs_deny_implicit_external_authority() {
    for entry in public_inventory_entries()
        .into_iter()
        .filter(is_configuration_security_relevant)
    {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let fields = markdown_frontmatter(&doc_path);
        let doc_text = fs::read_to_string(&doc_path).expect("read API doc");
        let frontmatter_security = fields
            .get("security")
            .map(String::as_str)
            .unwrap_or_default();
        let inventory_security = entry.get("security_notes");

        for denied in denied_configuration_authorities() {
            assert!(
                frontmatter_security.contains(denied),
                "{id} {} frontmatter security is missing no-authority language for {denied}",
                entry.get("documentation_path")
            );
            assert!(
                inventory_security.contains(denied),
                "{id} {} inventory security_notes is missing no-authority language for {denied}",
                entry.get("documentation_path")
            );
            assert!(
                doc_text.contains(denied),
                "{id} {} body is missing no-authority language for {denied}",
                entry.get("documentation_path")
            );
        }
    }
}

#[test]
fn clay_js_api_docs_cover_primitive_gate_surfaces() {
    let linked_paths = docs_index_registry_links();
    for id in [
        "clay.packages.serverValidatePackageManifest",
        "clay.packages.serverValidatePackagePermissions",
        "clay.packages.serverLoadPackage",
        "clay.modes.serverRegisterModePattern",
        "clay.modes.serverClassifyDocument",
        "clay.modes.serverActivateMajorMode",
        "clay.commands.serverRegisterCommand",
        "clay.commands.serverListCommands",
    ] {
        let entry = public_inventory_entries()
            .into_iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing public primitive gate entry {id}"));
        assert!(
            linked_paths.contains(entry.get("documentation_path")),
            "{id} must be linked from docs/index.md"
        );
        assert!(
            facade_exports_function(entry.get("facade_path"), entry.get("js_export")),
            "{id} facade export must exist"
        );
        let doc_text = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path")),
        )
        .expect("read primitive gate API doc");
        assert!(
            doc_text.contains(entry.get("deno_op_path")),
            "{id} docs must name the op wrapper path"
        );
        assert!(
            doc_text.contains("raw op names") || doc_text.contains("Deno.core.ops"),
            "{id} docs must steer users away from raw ops"
        );
        assert!(
            doc_text.contains("does not grant filesystem"),
            "{id} docs must include security no-authority notes"
        );
    }
}

#[test]
fn phase9_file_workspace_apis_are_documented_indexed_and_security_scoped() {
    let expected = [
        "clay.documents.serverOpenDocument",
        "clay.documents.serverSaveDocument",
        "clay.documents.serverReloadDocument",
        "clay.documents.serverGetDocumentStatus",
        "clay.documents.serverListDocuments",
        "clay.workspace.serverListWorkspaceRoots",
    ];
    let entries = public_inventory_entries();
    let linked_paths = docs_index_registry_links();

    for expected_id in expected {
        let entry = entries
            .iter()
            .find(|entry| entry.get("id") == expected_id)
            .unwrap_or_else(|| panic!("missing Phase 9 file/workspace API {expected_id}"));
        let doc_path = entry.get("documentation_path");
        assert!(
            linked_paths.contains(doc_path),
            "{expected_id} must be linked from docs/index.md"
        );
        assert!(
            facade_exports_function(entry.get("facade_path"), entry.get("js_export")),
            "{expected_id} facade export is missing"
        );

        let full_doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(doc_path);
        let doc_text = fs::read_to_string(&full_doc_path).expect("read Phase 9 API doc");
        for required in [
            "server-side validation",
            "workspace root authorization",
            "path traversal",
            "typed file errors",
            "do not receive raw host filesystem authority",
        ] {
            assert!(
                doc_text.contains(required),
                "{expected_id} documentation must include security note {required:?}"
            );
        }
        assert!(
            entry.get("hot_path_policy").contains("not")
                || entry.get("hot_path_policy").contains("never")
                || entry.get("hot_path_policy").contains("asynchronous"),
            "{expected_id} must document that file/workspace API use is outside ordinary hot paths"
        );
    }
}

#[test]
fn phase18_4_configuration_audit_closes_unchecked_task() {
    // Closes the gap from plans/027 Phase 18.4 "Create or verify Clay configuration APIs".
    // All Phase 18.4 configuration/override surfaces have docs, index links,
    // inventory entries, custom_properties, security notes, and runtime-backed status.
    let entries = inventory_entries();
    let registry_links = docs_index_registry_links();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing Phase 18.4 configuration API inventory entry {id}"))
    };

    // The four Phase 18.4 configuration/override surfaces
    for (id, doc_path) in [
        (
            "clay.configuration.setPackageOption",
            "docs/reference/clay-js-api/configuration/set-package-option.md",
        ),
        (
            "clay.ui.serverSetLayoutOverride",
            "docs/reference/clay-js-api/ui/server-set-layout-override.md",
        ),
        (
            "clay.ui.serverRegisterUiStateScope",
            "docs/reference/clay-js-api/ui/server-register-ui-state-scope.md",
        ),
        (
            "clay.ui.serverRegisterInputContribution",
            "docs/reference/clay-js-api/ui/server-register-input-contribution.md",
        ),
    ] {
        let entry = entry_by_id(id);
        // Status and visibility
        assert_eq!(
            entry.get("status"),
            "runtime-backed",
            "{id} must be runtime-backed"
        );
        assert_eq!(
            entry.get("registry_public"),
            "true",
            "{id} must be registry public"
        );
        // Per-API docs exist
        let full_doc_path = root.join(doc_path);
        assert!(
            full_doc_path.exists(),
            "{id} per-API doc must exist at {doc_path}"
        );
        // docs/index.md link
        assert!(
            registry_links.contains(doc_path),
            "{id} must be linked from docs/index.md registry section"
        );
        // custom_properties exist and are non-empty
        let custom_props = inventory_custom_property_names(entry.get("custom_properties"));
        assert!(!custom_props.is_empty(), "{id} must have custom_properties");
        // Security notes deny prohibited authorities
        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
            "direct Masonry widgets",
        ] {
            assert!(
                entry.get("security_notes").contains(denied),
                "{id} security_notes must deny {denied}"
            );
        }
        // Facade export is wired
        assert!(
            facade_exports_function(entry.get("facade_path"), entry.get("js_export")),
            "{id} facade must export the function"
        );
    }
}

#[test]
fn phase18_4_configuration_audit_rejects_hidden_keys() {
    // Verifies no undocumented configuration keys were introduced.
    // Phase 18.4 docs explicitly reject hidden JSON/TOML/ad hoc layout, style,
    // input, theme, and package option keys.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let configuration_wiki =
        fs::read_to_string(root.join("docs/wiki/modules/configuration-runtime.md"))
            .expect("read configuration runtime wiki");

    // The configuration.md and wiki must contain rejection language
    for rejection_phrase in [
        "hidden JSON/TOML/ad hoc",
        "documented Clay JS APIs",
        "raw CSS",
        "client-side JavaScript",
        "package enable/disable",
    ] {
        assert!(
            configuration_doc.contains(rejection_phrase),
            "configuration.md must reject hidden/ad hoc keys: {rejection_phrase}"
        );
        assert!(
            configuration_wiki.contains(rejection_phrase),
            "configuration-runtime.md must reject hidden/ad hoc keys: {rejection_phrase}"
        );
    }

    // The per-API docs for setPackageOption and serverSetLayoutOverride must
    // explicitly reject hidden keys
    let set_option_doc = fs::read_to_string(
        root.join("docs/reference/clay-js-api/configuration/set-package-option.md"),
    )
    .expect("read set-package-option.md");
    let layout_override_doc = fs::read_to_string(
        root.join("docs/reference/clay-js-api/ui/server-set-layout-override.md"),
    )
    .expect("read server-set-layout-override.md");

    for rejection_phrase in ["hidden", "raw ops", "client-side JavaScript"] {
        assert!(
            set_option_doc.contains(rejection_phrase),
            "set-package-option.md must reject {rejection_phrase}"
        );
        assert!(
            layout_override_doc.contains(rejection_phrase),
            "server-set-layout-override.md must reject {rejection_phrase}"
        );
    }

    // Named hidden keys that must be identified as rejected in configuration.md
    for hidden_key in [
        "preview.position",
        "layout.preview.defaultSlot",
        "preview.defaultVisibility",
        "layout.preview.defaultVisibility",
    ] {
        assert!(
            configuration_doc.contains(hidden_key),
            "configuration.md must identify hidden key `{hidden_key}` as rejected or planned-only"
        );
    }
}

#[test]
fn phase18_4_configuration_audit_records_deferrals() {
    // Verifies intentionally deferred surfaces have explicit rationale in docs.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let entries = inventory_entries();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing inventory entry {id}"))
    };

    // Deferred surfaces: setModePreference, setDecorationTheme, setParsePolicy
    // must remain planned in the inventory
    for deferred_id in [
        "clay.configuration.setModePreference",
        "clay.configuration.setDecorationTheme",
        "clay.configuration.setParsePolicy",
    ] {
        let entry = entry_by_id(deferred_id);
        assert_eq!(
            entry.get("status"),
            "planned",
            "{deferred_id} must remain planned (not runtime-backed)"
        );
        assert_eq!(
            entry.get("registry_public"),
            "false",
            "{deferred_id} must not be registry public"
        );
    }

    // Deferred shell/layout mutation APIs remain planned
    for deferred_id in [
        "clay.ui.serverRegisterWorkingAreaLayout",
        "clay.ui.serverRegisterPaneSplitTree",
        "clay.ui.serverSetPaneSlotLayout",
    ] {
        let entry = entry_by_id(deferred_id);
        assert_eq!(
            entry.get("status"),
            "planned",
            "{deferred_id} must remain planned"
        );
    }

    // configuration.md must explicitly document what remains deferred
    for deferred_mention in [
        "setModePreference",
        "setDecorationTheme",
        "setParsePolicy",
        "durable state-value mutation",
        "pane selector",
        "package enable/disable",
    ] {
        assert!(
            configuration_doc.contains(deferred_mention),
            "configuration.md must document deferred surface: {deferred_mention}"
        );
    }
}

#[test]
fn permission_bearing_configuration_requires_validation_notes() {
    for entry in public_inventory_entries()
        .into_iter()
        .filter(is_configuration_security_relevant)
        .filter(|entry| !parse_toml_string_list(entry.get("permissions")).is_empty())
    {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let fields = markdown_frontmatter(&doc_path);
        let doc_text = fs::read_to_string(&doc_path).expect("read API doc");
        let frontmatter_security = fields
            .get("security")
            .map(String::as_str)
            .unwrap_or_default();
        let combined_validation_notes = format!(
            "{}\n{}\n{}",
            entry.get("security_notes"),
            frontmatter_security,
            doc_text
        );

        assert!(
            doc_text.contains("Requires:") || doc_text.contains("required permissions"),
            "{id} {} must document explicit required permissions in the body",
            entry.get("documentation_path")
        );
        assert!(
            contains_permission_validation_note(&combined_validation_notes),
            "{id} {} lists permissions but is missing server-side validation notes",
            entry.get("documentation_path")
        );
    }
}

#[test]
fn phase18_5_clay_js_api_inventory_status_matches_runtime() {
    // Phase 18.5 Task 7: verifies that implemented Phase 18.5-relevant APIs
    // have runtime metadata and deferred APIs remain planned in the inventory.
    let entries = inventory_entries();
    let registry_links = docs_index_registry_links();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Implemented runtime-backed APIs that Phase 18.5 relies on must have
    // status = "runtime-backed" (or "runtime-backed-command"), registry_public = true,
    // per-API docs, facade exports, and generated registry/index links.
    let implemented_ids = [
        "clay.ui.serverRegisterPanelContribution",
        "clay.ui.serverRegisterComponentContribution",
        "clay.ui.serverRegisterTransientOverlayContribution",
        "clay.ui.serverRegisterInputContribution",
        "clay.ui.serverRegisterUiStateScope",
        "clay.ui.serverRegisterThemeToken",
        "clay.ui.serverSetLayoutOverride",
        "clay.configuration.setPackageOption",
        "clay.documents.clientOpenFileDialog",
    ];

    for id in implemented_ids {
        let entry = entries
            .iter()
            .find(|e| e.get("id") == id)
            .unwrap_or_else(|| panic!("missing inventory entry for {id}"));
        let status = entry.get("status");
        assert!(
            status == "runtime-backed" || status == "runtime-backed-command",
            "{id} must be runtime-backed, got {status}"
        );
        assert_eq!(
            entry.get("registry_public"),
            "true",
            "{id} must be registry public"
        );
        let doc_path = entry.get("documentation_path");
        assert!(!doc_path.is_empty(), "{id} must have a documentation_path");
        let full_path = root.join(doc_path);
        assert!(
            full_path.exists(),
            "{id} per-API doc must exist at {doc_path}"
        );
        assert!(
            registry_links.contains(doc_path),
            "{id} must be linked from docs/index.md registry section"
        );
        // Facade must export the function
        assert!(
            facade_exports_function(entry.get("facade_path"), entry.get("js_export")),
            "{id} facade must export the function"
        );
    }

    // Deferred APIs from Phase 18.5 must remain planned
    let deferred_ids = [
        "clay.ui.serverRegisterWorkingAreaLayout",
        "clay.ui.serverRegisterPaneSplitTree",
        "clay.ui.serverSetPaneSlotLayout",
        "clay.configuration.setModePreference",
        "clay.configuration.setDecorationTheme",
        "clay.configuration.setParsePolicy",
    ];
    for id in deferred_ids {
        let entry = entries
            .iter()
            .find(|e| e.get("id") == id)
            .unwrap_or_else(|| panic!("missing inventory entry for deferred {id}"));
        assert_eq!(
            entry.get("status"),
            "planned",
            "deferred {id} must be planned, got {}",
            entry.get("status")
        );
        assert_eq!(
            entry.get("registry_public"),
            "false",
            "deferred {id} must not be registry public"
        );
    }
}

#[test]
fn generated_registry_contains_phase18_5_public_apis() {
    // Phase 18.5 Task 7: verifies that the generated Clay JS API registry
    // contains entries for all Phase 18.5-relevant public APIs with docs, index
    // links, user-facing names, and custom properties.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry_path = root.join("docs/generated/clay-js-api-registry.json");
    let registry_text = fs::read_to_string(&registry_path).expect("read generated registry");
    let registry: serde_json::Value =
        serde_json::from_str(&registry_text).expect("parse generated registry");
    let entries = registry["entries"]
        .as_array()
        .expect("registry entries array");

    let registry_by_id: BTreeMap<String, &serde_json::Value> = entries
        .iter()
        .map(|e| {
            let id = e["id"].as_str().expect("entry id");
            (id.to_string(), e)
        })
        .collect();

    // Every Phase 18.5-relevant implemented public API must be in the registry
    let public_api_ids = [
        "clay.ui.serverRegisterPanelContribution",
        "clay.ui.serverRegisterComponentContribution",
        "clay.ui.serverRegisterTransientOverlayContribution",
        "clay.ui.serverRegisterInputContribution",
        "clay.ui.serverRegisterUiStateScope",
        "clay.ui.serverRegisterThemeToken",
        "clay.ui.serverSetLayoutOverride",
        "clay.configuration.setPackageOption",
        "clay.documents.clientOpenFileDialog",
    ];
    for id in &public_api_ids {
        let entry = registry_by_id
            .get(*id)
            .unwrap_or_else(|| panic!("generated registry missing {id}"));
        // Must have user_facing_name
        assert!(
            !entry
                .get("user_facing_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty(),
            "{id} must have a user_facing_name"
        );
        // Must have custom_properties (even if empty list for clientOpenFileDialog)
        assert!(
            entry.get("custom_properties").is_some(),
            "{id} must have custom_properties"
        );
        // Must have a non-empty documentation_path
        let doc_path = entry
            .get("documentation_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(!doc_path.is_empty(), "{id} must have documentation_path");
        let full_doc = root.join(doc_path);
        assert!(
            full_doc.exists(),
            "{id} per-API doc must exist at {doc_path}"
        );
    }

    // Verify Markdown package exports are documented in packages/markdown/docs/index.md
    let markdown_docs = root.join("packages/markdown/docs/index.md");
    assert!(
        markdown_docs.exists(),
        "packages/markdown/docs/index.md must exist"
    );
    let markdown_text = fs::read_to_string(&markdown_docs).expect("read markdown package docs");
    // Package-owned load-path exports documented in package docs.
    // markdownPackageManifest is an internal helper, not a documented end-user surface.
    for export_name in ["markdownLoadMode", "loadMarkdownPackage"] {
        assert!(
            markdown_text.contains(export_name),
            "packages/markdown/docs/index.md must document {export_name}"
        );
    }
}

#[test]
fn phase18_5_public_rust_surfaces_have_clay_js_mapping_or_internal_visibility() {
    // Phase 18.5 Task 7: verifies that no new public Rust server-side function
    // bypasses Clay JS API mapping. Phase 18.5-relevant server modules must
    // either have pub(crate) visibility for internal functions or have a Clay JS
    // API inventory entry mapping the public surface.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let inventory_text =
        fs::read_to_string(root.join("docs/reference/clay-js-api/api-inventory.toml"))
            .expect("read api inventory");

    // Phase 18.5-relevant server source files that handle package UI contributions,
    // layout overrides, configuration, and package loading.
    let server_sources = [
        "src/server/ui.rs",
        "src/server/ops/ui.rs",
        "src/server/ops/configuration.rs",
        "src/server/ops/packages.rs",
        "src/packages/record.rs",
    ];

    // src/packages/service.rs has known pub functions (PackageService::new,
    // install, install_from_value, enable, disable, remove, list, inspect,
    // enabled_records) that are internal server infrastructure, not Clay JS APIs.
    // They are behind server-side ops and validated through Clay JS facades.
    // They share the crate with main but are not user-facing APIs; the test
    // verifies that no new pub fn in these Phase 18.5-relevant files bypasses
    // the Clay JS API boundary.
    let service_source =
        fs::read_to_string(root.join("src/packages/service.rs")).expect("read packages/service.rs");
    let service_known_pub_fns = [
        "new",
        "install",
        "install_from_value",
        "enable",
        "disable",
        "remove",
        "list",
        "inspect",
        "enabled_records",
    ];
    for known_fn in &service_known_pub_fns {
        assert!(
            service_source.contains(&format!("pub fn {known_fn}"))
                || service_source.contains(&format!("pub async fn {known_fn}")),
            "PackageService::{known_fn} must remain a known internal server function"
        );
    }
    // PackageService pub fns are internal server infrastructure behind Clay JS ops;
    // they must not be user-facing Clay JS APIs.
    assert!(
        !inventory_text.contains("PackageService::install"),
        "PackageService::install must not be a Clay JS API; package installation is a separate authority boundary"
    );
    assert!(
        !inventory_text.contains("PackageService::enable"),
        "PackageService::enable must not be a Clay JS API; package enable/disable is package-service authority"
    );

    for source_path in &server_sources {
        let full_path = root.join(source_path);
        let source =
            fs::read_to_string(&full_path).unwrap_or_else(|_| panic!("read {source_path}"));
        // Verify there are no leaked `pub fn` or `pub async fn` that would
        // create public server-side surfaces without a Clay JS API entry.
        // `pub(crate) fn` is acceptable.
        for line in source.lines() {
            let trimmed = line.trim();
            // Skip comment lines
            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            // Detect pub fn / pub async fn that is NOT pub(crate)
            if let Some(fn_decl) = trimmed
                .strip_prefix("pub async fn ")
                .or_else(|| trimmed.strip_prefix("pub fn "))
            {
                let fn_name = fn_decl.split('(').next().unwrap_or("").trim();
                // Check if this function is mapped in the Clay JS API inventory
                // (as backing_rust, current_rust_owner, or deno_op_path)
                let mapped = inventory_text.contains(fn_name);
                assert!(
                    mapped,
                    "{} has pub fn {} without Clay JS API inventory mapping; make it pub(crate) or add a Clay JS API entry",
                    source_path, fn_name
                );
            }
        }
    }

    // Verify that the package-service public surface (PackageService) is internal
    // or mapped, and that server ops for Phase 18.5 APIs are mapped.
    let service_source =
        fs::read_to_string(root.join("src/packages/service.rs")).expect("read packages/service.rs");
    // PackageService is pub but package-installation/enable is not a Clay JS API
    // surface; it is internal server infrastructure behind serverLoadPackage ops.
    assert!(
        service_source.contains("pub struct PackageService"),
        "PackageService must exist in packages/service.rs"
    );

    // The op files must not have public fn declarations that bypass the facade.
    // Ops are registered through deno_core macro invocations, not pub fn.
    for ops_path in &[
        "src/server/ops/ui.rs",
        "src/server/ops/configuration.rs",
        "src/server/ops/packages.rs",
    ] {
        let ops_source =
            fs::read_to_string(root.join(ops_path)).unwrap_or_else(|_| panic!("read {ops_path}"));
        // Ops use #[deno_core::op2] or similar macros; they must not have pub fn.
        let has_pub_fn = ops_source.lines().any(|line| {
            let trimmed = line.trim();
            (trimmed.starts_with("pub fn ") || trimmed.starts_with("pub async fn "))
                && !trimmed.starts_with("pub(crate)")
        });
        assert!(
            !has_pub_fn,
            "{ops_path} must not have pub fn; ops are registered through deno_core macros"
        );
    }
}

#[test]
fn phase18_5_configuration_surfaces_are_documented_or_planned() {
    // Phase 18.5 task 8: closes the Markdown end-user loading configuration audit.
    // Every behavior-changing Markdown configuration surface is either a runtime-backed
    // Clay JS API (with docs/index/registry/custom_properties/security notes) or an
    // explicitly planned/unavailable inventory entry. No undocumented keys are introduced.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let registry_links = docs_index_registry_links();
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");

    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing inventory entry {id}"))
    };

    // Implemented runtime-backed Markdown-relevant configuration surfaces.
    // Each must be runtime-backed, registry-public, have per-API docs linked
    // from docs/index.md, have non-empty custom_properties, deny prohibited
    // authorities, and export the function through its facade.
    let implemented_markdown_surfaces = [
        (
            "clay.configuration.setPackageOption",
            "docs/reference/clay-js-api/configuration/set-package-option.md",
        ),
        (
            "clay.ui.serverSetLayoutOverride",
            "docs/reference/clay-js-api/ui/server-set-layout-override.md",
        ),
        (
            "clay.ui.serverRegisterThemeToken",
            "docs/reference/clay-js-api/ui/server-register-theme-token.md",
        ),
        (
            "clay.ui.serverRegisterPanelContribution",
            "docs/reference/clay-js-api/ui/server-register-panel-contribution.md",
        ),
        (
            "clay.ui.serverRegisterInputContribution",
            "docs/reference/clay-js-api/ui/server-register-input-contribution.md",
        ),
        (
            "clay.ui.serverRegisterUiStateScope",
            "docs/reference/clay-js-api/ui/server-register-ui-state-scope.md",
        ),
    ];

    for (id, doc_path) in implemented_markdown_surfaces {
        let entry = entry_by_id(id);
        let status = entry.get("status");
        assert!(
            status == "runtime-backed" || status == "runtime-backed-command",
            "Markdown-relevant surface {id} must be runtime-backed, got {status}"
        );
        assert_eq!(
            entry.get("registry_public"),
            "true",
            "Markdown-relevant surface {id} must be registry public"
        );
        let full_doc = root.join(doc_path);
        assert!(
            full_doc.exists(),
            "Markdown-relevant surface {id} per-API doc must exist at {doc_path}"
        );
        assert!(
            registry_links.contains(doc_path),
            "Markdown-relevant surface {id} must be linked from docs/index.md registry section"
        );
        let custom_props = inventory_custom_property_names(entry.get("custom_properties"));
        assert!(
            !custom_props.is_empty(),
            "Markdown-relevant surface {id} must list non-empty custom_properties"
        );
        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
        ] {
            assert!(
                entry.get("security_notes").contains(denied),
                "Markdown-relevant surface {id} security_notes must deny {denied}"
            );
        }
        assert!(
            facade_exports_function(entry.get("facade_path"), entry.get("js_export")),
            "Markdown-relevant surface {id} facade must export the function"
        );
    }

    // Planned/unavailable Markdown-relevant configuration surfaces must remain planned.
    for planned_id in [
        "clay.configuration.setModePreference",
        "clay.configuration.setDecorationTheme",
        "clay.configuration.setParsePolicy",
    ] {
        let entry = entry_by_id(planned_id);
        assert_eq!(
            entry.get("status"),
            "planned",
            "Markdown-relevant deferred surface {planned_id} must remain planned"
        );
        assert_eq!(
            entry.get("registry_public"),
            "false",
            "Markdown-relevant deferred surface {planned_id} must not be registry public"
        );
    }

    // configuration.md must document the Phase 18.5 audit table mapping Markdown
    // needs to generic Clay JS APIs.
    for phrase in [
        "Phase 18.5 Markdown end-user loading configuration audit",
        "Markdown need",
        "defaultVisibility: \"hidden\"",
        "clay.configuration.setPackageOption",
        "clay.ui.serverSetLayoutOverride",
        "clay.ui.serverRegisterThemeToken",
        "clay.ui.serverRegisterPanelContribution",
        "clay.configuration.setModePreference",
        "clay.configuration.setDecorationTheme",
        "clay.configuration.setParsePolicy",
        "clay.packages.loadPackage",
    ] {
        assert!(
            configuration_doc.contains(phrase),
            "configuration.md Phase 18.5 audit must reference {phrase}"
        );
    }
}

#[test]
fn phase18_5_configuration_apis_have_custom_properties() {
    // Phase 18.5 task 8: behavior-changing Markdown-relevant configuration APIs
    // have typed custom_properties with type/default/allowed-value metadata in
    // their per-API docs and the generated registry.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let entry_by_id = |id: &str| {
        entries
            .iter()
            .find(|entry| entry.get("id") == id)
            .unwrap_or_else(|| panic!("missing inventory entry {id}"))
    };

    // For each Markdown-relevant runtime-backed API, verify the per-API doc has
    // YAML frontmatter custom_properties with name+type+default for each entry.
    for (id, doc_path, required_props) in [
        (
            "clay.configuration.setPackageOption",
            "docs/reference/clay-js-api/configuration/set-package-option.md",
            vec!["packagePrefix", "option", "value", "source"],
        ),
        (
            "clay.ui.serverSetLayoutOverride",
            "docs/reference/clay-js-api/ui/server-set-layout-override.md",
            vec!["targetId", "property", "value", "source"],
        ),
        (
            "clay.ui.serverRegisterThemeToken",
            "docs/reference/clay-js-api/ui/server-register-theme-token.md",
            vec!["token", "type", "fallback", "description", "source"],
        ),
        (
            "clay.ui.serverRegisterPanelContribution",
            "docs/reference/clay-js-api/ui/server-register-panel-contribution.md",
            vec!["id", "slot", "kind", "defaultVisibility", "component"],
        ),
    ] {
        let entry = entry_by_id(id);
        let inventory_props = inventory_custom_property_names(entry.get("custom_properties"));
        for prop in &required_props {
            assert!(
                inventory_props.iter().any(|name| name == prop),
                "inventory for {id} must list custom_property {prop}"
            );
        }
        let doc_text = fs::read_to_string(root.join(doc_path))
            .unwrap_or_else(|err| panic!("read {doc_path}: {err}"));
        // Parse the YAML frontmatter custom_properties block as lines so the
        // test works for both LF and CRLF line endings.
        let doc_lines: Vec<&str> = doc_text.lines().collect();
        for prop in &required_props {
            let header = format!("  - name: {prop}");
            let start = doc_lines
                .iter()
                .position(|line| line.trim_end() == header)
                .unwrap_or_else(|| {
                    panic!("{id} per-API doc {doc_path} must declare frontmatter custom_property entry for {prop}")
                });
            // Take lines until the next `  - name:` (next property), a blank line,
            // or the frontmatter close `---`.
            let mut block: Vec<&str> = Vec::new();
            for line in doc_lines.iter().skip(start + 1) {
                let trimmed = line.trim_end();
                if trimmed.starts_with("  - name:")
                    || trimmed == "---"
                    || trimmed.is_empty()
                    || (!trimmed.starts_with(' ') && !trimmed.is_empty())
                {
                    break;
                }
                block.push(line);
            }
            let block_text = block.join("\n");
            assert!(
                block_text.contains("type:"),
                "{id} custom_property {prop} must declare a type"
            );
            assert!(
                block_text.contains("default:"),
                "{id} custom_property {prop} must declare a default"
            );
        }
    }

    // serverRegisterPanelContribution must declare defaultVisibility allowed
    // values (visible|hidden|collapsed) so the Markdown preview can be hidden
    // by default through this documented typed field, not a hidden key.
    let panel_doc = fs::read_to_string(
        root.join("docs/reference/clay-js-api/ui/server-register-panel-contribution.md"),
    )
    .expect("read panel contribution doc");
    assert!(
        panel_doc.contains("`visible`, `hidden`, or `collapsed`"),
        "serverRegisterPanelContribution must document defaultVisibility allowed values"
    );
}

#[test]
fn phase18_5_docs_reject_hidden_markdown_config_keys() {
    // Phase 18.5 task 8: hidden/ad hoc Markdown configuration keys remain
    // rejected by policy and are explicitly identified in configuration docs.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let configuration_wiki =
        fs::read_to_string(root.join("docs/wiki/modules/configuration-runtime.md"))
            .expect("read configuration runtime wiki");

    // Markdown-specific hidden keys that must be identified as rejected or
    // planned-only in the Phase 18.5 audit and the configuration wiki.
    let markdown_hidden_keys = [
        "preview.position",
        "preview.defaultVisibility",
        "layout.preview.defaultSlot",
        "layout.preview.defaultVisibility",
        "theme.markdown.heading.1",
        "theme.markdown.preview.background",
        "markdown.sidebar.width",
    ];

    for hidden_key in markdown_hidden_keys {
        assert!(
            configuration_doc.contains(hidden_key),
            "configuration.md must identify Markdown hidden key `{hidden_key}` as rejected or planned-only"
        );
    }

    // A representative subset must also appear in the configuration runtime wiki
    // so internal-vs-public boundary docs stay aligned.
    for wiki_hidden_key in [
        "preview.position",
        "layout.preview.defaultSlot",
        "theme.markdown.heading.1",
        "theme.markdown.preview.background",
        "markdown.sidebar.width",
    ] {
        assert!(
            configuration_wiki.contains(wiki_hidden_key),
            "configuration-runtime.md must identify Markdown hidden key `{wiki_hidden_key}` as rejected"
        );
    }

    // The per-API docs for setPackageOption and serverSetLayoutOverride must
    // explicitly reject hidden keys (covered for general hidden-key rejection
    // in Phase 18.4; this Phase 18.5 test confirms Markdown-relevant coverage).
    let set_option_doc = fs::read_to_string(
        root.join("docs/reference/clay-js-api/configuration/set-package-option.md"),
    )
    .expect("read set-package-option.md");
    let layout_override_doc = fs::read_to_string(
        root.join("docs/reference/clay-js-api/ui/server-set-layout-override.md"),
    )
    .expect("read server-set-layout-override.md");
    for phrase in ["hidden", "raw ops", "client-side JavaScript"] {
        assert!(
            set_option_doc.contains(phrase),
            "set-package-option.md must reject {phrase}"
        );
        assert!(
            layout_override_doc.contains(phrase),
            "server-set-layout-override.md must reject {phrase}"
        );
    }

    // The setPackageOption doc must reject hidden option keys specifically.
    assert!(
        set_option_doc.contains("hidden option keys"),
        "set-package-option.md must reject hidden option keys"
    );

    // The serverSetLayoutOverride doc must reject hidden layout keys specifically.
    assert!(
        layout_override_doc.contains("hidden layout"),
        "server-set-layout-override.md must reject hidden layout keys"
    );

    // The Phase 18.5 audit section must reject Markdown-specific hidden/ad hoc keys.
    for phrase in [
        "Markdown-specific hidden/ad hoc configuration keys",
        "rejected by policy",
        "package-owned prefix",
    ] {
        assert!(
            configuration_doc.contains(phrase),
            "configuration.md Phase 18.5 audit must state {phrase}"
        );
    }
}

#[test]
fn phase20_markdown_configuration_audit_documents_end_user_contract() {
    // Phase 20 task 7: Verify that the Phase 18.5 Markdown end-user loading
    // configuration audit section accurately documents the Phase 20 contract:
    // one-line loadPackage, explicit bindKey, and setPackageOption/serverSetLayoutOverride
    // for the optional preview. Also verify no hardcoded Markdown config keys exist
    // in Rust code outside of test code that validates rejection.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");

    // Verify the Phase 18.5 audit section documents loadPackage as implemented by Plan 029
    assert!(
        configuration_doc.contains("One-line package loading")
            && configuration_doc.contains("clay.packages.loadPackage")
            && configuration_doc.contains("implemented (Plan 029, Phase 18.6)"),
        "Phase 18.5 audit must document loadPackage as implemented by Plan 029"
    );

    // Verify the Phase 18.5 audit section documents bindKey for the file-dialog key binding
    assert!(
        configuration_doc.contains("Markdown file-dialog key binding")
            && configuration_doc.contains("clay.keybindings.bindKey"),
        "Phase 18.5 audit must document bindKey for the file-dialog key binding"
    );

    // Verify the Phase 18.5 audit section documents setPackageOption for Markdown package options
    assert!(
        configuration_doc.contains("Markdown package options")
            && configuration_doc.contains("clay.configuration.setPackageOption")
            && configuration_doc.contains("runtime-backed"),
        "Phase 18.5 audit must document setPackageOption for Markdown package options"
    );

    // Verify the Phase 18.5 audit section documents serverSetLayoutOverride for Markdown layout overrides
    assert!(
        configuration_doc.contains("Markdown layout overrides")
            && configuration_doc.contains("clay.ui.serverSetLayoutOverride"),
        "Phase 18.5 audit must document serverSetLayoutOverride for Markdown layout overrides"
    );

    // Verify no hardcoded Markdown-specific hidden config keys exist in the
    // non-test Rust sources. Only configuration.rs test code (which validates
    // rejection) should reference hidden key names.
    let configuration_rs = fs::read_to_string(root.join("src/server/configuration.rs"))
        .expect("read configuration.rs");
    // The non-test portion of configuration.rs must not declare or accept
    // hidden Markdown config keys. Test code that validates rejection is
    // guarded by #[cfg(test)] and is excluded from this check.
    let non_test_section = configuration_rs
        .find("#[cfg(test)]")
        .map(|idx| &configuration_rs[..idx])
        .unwrap_or(&configuration_rs);
    for pattern in [
        "\"markdown.preview.position\"",
        "\"markdown.preview.defaultVisibility\"",
        "\"markdown.layout.preview.defaultSlot\"",
        "\"markdown.sidebar.width\"",
    ] {
        assert!(
            !non_test_section.contains(pattern),
            "configuration.rs non-test code must not contain hardcoded Markdown hidden key {pattern}"
        );
    }
}

#[test]
fn phase20_loadpackage_api_documentation_is_complete() {
    // Phase 20 task 8: Verify that the loadPackage API has complete documentation
    // with all required sections: examples, errors, permissions, security notes, etc.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Verify loadPackage docs exist and have required sections
    let load_package_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/packages/load-package.md"))
            .expect("read load-package.md");

    // Check for required documentation sections
    let required_sections = [
        "## Example",
        "## Errors",
        "## Permissions and security",
        "## Agent guidance",
        "## Return and async behavior",
        "## When to use",
    ];

    for section in required_sections {
        assert!(
            load_package_doc.contains(section),
            "loadPackage docs must include {section} section"
        );
    }

    // Verify security notes are present
    assert!(
        load_package_doc.contains("does not grant") && load_package_doc.contains("deny-by-default"),
        "loadPackage docs must include security constraints"
    );

    // Verify package docs reference the API correctly
    let package_doc = fs::read_to_string(root.join("packages/markdown/docs/index.md"))
        .expect("read packages/markdown/docs/index.md");

    assert!(
        package_doc.contains("loadPackage") && package_doc.contains("@clay/markdown"),
        "Package docs must reference loadPackage API"
    );

    // Verify the reference docs have the end-user baseline
    let ref_doc = fs::read_to_string(root.join("docs/reference/packages/markdown.md"))
        .expect("read docs/reference/packages/markdown.md");

    assert!(
        ref_doc.contains("End-User UX Baseline") || ref_doc.contains("loadPackage"),
        "Reference docs must document end-user setup"
    );
}

#[test]
fn phase20_rust_public_functions_have_api_mappings_or_internal_visibility() {
    // Phase 20 task 8: Verify that no public Rust function in the package loading
    // path lacks a Clay JS API mapping or explicit internal visibility.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let inventory = fs::read_to_string(root.join("docs/reference/clay-js-api/api-inventory.toml"))
        .expect("read api-inventory.toml");

    // Check key files in the package loading path
    let files_to_check = [
        "src/server/ops/packages.rs",
        "src/server/js_runtime.rs",
        "src/server/ops/ui.rs",
        "src/server/ops/configuration.rs",
    ];

    for file_path in files_to_check {
        let full_path = root.join(file_path);
        if !full_path.exists() {
            continue;
        }

        let content =
            fs::read_to_string(&full_path).unwrap_or_else(|_| panic!("read {}", file_path));

        // Find all pub fn declarations (excluding pub(crate))
        let lines: Vec<&str> = content.lines().collect();
        for (line_num, line) in lines.iter().enumerate() {
            if line.contains("pub fn") && !line.contains("pub(crate)") {
                // Extract function name
                if let Some(fn_name) = line.split("fn ").nth(1).and_then(|s| s.split('(').next()) {
                    let fn_name = fn_name.trim();

                    // Check if this function has a corresponding entry in the API inventory
                    // We look for the function name in backing_rust or deno_op_path fields
                    let has_api_mapping = inventory.contains(fn_name);

                    // If no mapping found, this is a violation
                    assert!(
                        has_api_mapping,
                        "Public function `{}` in {} at line {} lacks a Clay JS API mapping in api-inventory.toml. \
                         Either add a mapping or change visibility to pub(crate) if it's internal.",
                        fn_name,
                        file_path,
                        line_num + 1
                    );
                }
            }
        }
    }
}

/// Plan 030 task "Create or verify Clay configuration APIs": the security
/// budgets introduced by Plan 030 (JS runtime timeout, openable file size,
/// runtime SDUI budgets, lifecycle-script suppression, file-open capability
/// gate, IPC endpoint permissions) are server-side security boundaries, not
/// user-tunable `init.js` options. They must NOT appear as configurable
/// `clay:configuration` inventory entries (raising them from user JavaScript
/// would defeat the boundary — e.g. lift the timeout to bypass the watchdog).
/// This test pins that they remain documented as intentionally non-configurable
/// and absent from the runtime-backed configuration inventory.
#[test]
fn plan_030_security_budgets_are_intentionally_non_configurable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let budgets =
        fs::read_to_string(root.join("src/perf/budgets.rs")).expect("read src/perf/budgets.rs");
    let entries = inventory_entries();

    // Each Plan 030 security budget is present as a compiled constant...
    for constant in [
        "JS_RUNTIME_EVALUATION_TIMEOUT_MS",
        "MAX_OPENABLE_FILE_BYTES",
        "RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES",
        "RUNTIME_SDUI_TREE_MAX_NODES",
        "RUNTIME_SDUI_TREE_MAX_DEPTH",
        "RUNTIME_SDUI_TREE_MAX_NODE_TEXT_CHARS",
        "LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB",
    ] {
        assert!(
            budgets.contains(constant),
            "src/perf/budgets.rs must define Plan 030 budget `{constant}`"
        );
    }

    // ...and is documented in the configuration overview as non-configurable.
    assert!(
        configuration_doc.contains("Plan 030 security budgets are intentionally not Clay JS APIs"),
        "configuration overview must document Plan 030 budgets as non-configurable"
    );
    for budget in [
        "JS_RUNTIME_EVALUATION_TIMEOUT_MS",
        "MAX_OPENABLE_FILE_BYTES",
        "RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES",
        "LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB",
    ] {
        assert!(
            configuration_doc.contains(budget),
            "configuration overview must mention budget `{budget}`"
        );
    }
    assert!(
        configuration_doc.contains("--ignore-scripts")
            && configuration_doc.contains("CLAY_ALLOW_LIFECYCLE_SCRIPTS")
            && configuration_doc.contains("--allow-scripts"),
        "configuration overview must document lifecycle-script suppression as a CLI/env control, not an init.js API"
    );

    // None of these budgets is exposed as a configurable Clay configuration API.
    // `setPackageOption` / `setModePreference` / `setDecorationTheme` /
    // `setParsePolicy` / `loadConfigurationModule` / `getConfigurationState` are
    // the configuration inventory entries; none accepts a timeout/file-size/
    // SDUI-budget parameter.
    let config_api_ids: BTreeSet<&str> = entries
        .iter()
        .filter(|entry| entry.get("id").starts_with("clay.configuration."))
        .map(|entry| entry.get("id"))
        .collect();
    for forbidden_id in [
        "clay.configuration.setJsRuntimeTimeout",
        "clay.configuration.setMaxOpenableFileSize",
        "clay.configuration.setSduiBudget",
        "clay.configuration.setRuntimeBudget",
        "clay.configuration.allowLifecycleScripts",
        "clay.configuration.setRuntimeHeapLimit",
        "clay.configuration.setV8HeapLimit",
        "clay.configuration.disableRuntimeHeapLimit",
    ] {
        assert!(
            !config_api_ids.contains(forbidden_id),
            "Plan 030 security budgets must not be exposed as configurable Clay JS APIs: \
             `{forbidden_id}` must not exist in the inventory"
        );
    }
}

/// Phase 18.7 task "Create or verify Clay configuration APIs": persistent
/// runtime and token-backed parse handlers do not add a new user-tunable
/// configuration API. Users load packages with `loadPackage`; package authors
/// declare parse budgets on `serverRegisterParseHandler`; runtime timeouts
/// surface as diagnostics, not mutable `init.js` settings.
#[test]
fn phase18_7_persistent_runtime_does_not_add_hidden_configuration_knobs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let configuration_doc =
        fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    let parse_doc = fs::read_to_string(
        root.join("docs/reference/clay-js-api/parse/server-register-parse-handler.md"),
    )
    .expect("read parse API docs");
    let entries = inventory_entries();

    for required in [
        "Phase 18.7 persistent runtime and parse bridge configuration review",
        "does **not** promote a new user-tunable configuration API",
        "await loadPackage(\"@clay/markdown\")",
        "serverActivateClassifiedMode",
        "ParseCoordinator",
        "clay.runtime.timeout",
        "not a callable `clay:configuration` API",
        "setParsePolicy` facade remains unavailable",
        "do not execute user configuration JavaScript",
    ] {
        assert!(
            configuration_doc.contains(required),
            "configuration overview must document Phase 18.7 config boundary phrase `{required}`"
        );
    }

    for required in [
        "timeoutMs",
        "maxWindowBytes",
        "guardBytes",
        "memoryBudgetBytes",
        "INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SYNTAX_CACHE_BUDGET_BYTES",
    ] {
        assert!(
            configuration_doc.contains(required) || parse_doc.contains(required),
            "Phase 18.7 parse budget `{required}` must be documented without hidden config keys"
        );
    }

    let set_parse_policy = entries
        .iter()
        .find(|entry| entry.get("id") == "clay.configuration.setParsePolicy")
        .expect("setParsePolicy inventory entry remains as planned surface");
    assert_eq!(set_parse_policy.get("status"), "planned");
    assert_eq!(set_parse_policy.get("registry_public"), "false");
    assert!(set_parse_policy.get("runtime_path").contains("planned"));

    let config_api_ids: BTreeSet<&str> = entries
        .iter()
        .filter(|entry| entry.get("id").starts_with("clay.configuration."))
        .map(|entry| entry.get("id"))
        .collect();
    for forbidden_id in [
        "clay.configuration.setRuntimeTimeout",
        "clay.configuration.setJsRuntimeTimeout",
        "clay.configuration.setParseHandlerTimeout",
        "clay.configuration.setRuntimeHeapLimit",
        "clay.configuration.setSandboxDisabled",
        "clay.configuration.enableThirdPartyPackages",
        "clay.configuration.setParseWindowBytes",
        "clay.configuration.setSyntaxCacheBudget",
        "clay.configuration.setDecorationPayloadBudget",
    ] {
        assert!(
            !config_api_ids.contains(forbidden_id),
            "Phase 18.7 must not expose hidden/tunable security budget API `{forbidden_id}`"
        );
    }
}

/// Plan 030 task "Create or verify Clay JS APIs for public programmatic
/// surfaces": Plan 030 is security-hardening work, so it must NOT introduce new
/// deno_core ops or new Clay JS API facades (a new programmatic JS capability
/// would be an authority expansion, the opposite of hardening). Every new
/// server-side helper added by Plan 030 must be `pub(crate)`, `pub(super)`, or
/// private — never a bare `pub fn` exposed as a programmatic surface without an
/// op wrapper + inventory entry. This test pins both invariants for the files
/// Plan 030 touched.
#[test]
fn plan_030_introduces_no_new_js_api_or_public_programmatic_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let inventory = fs::read_to_string(root.join("docs/reference/clay-js-api/api-inventory.toml"))
        .expect("read api-inventory.toml");

    // (1) No bare `pub fn`/`pub async fn` in Plan 030-touched server files
    // that carried NO pre-existing public surface (so any new `pub fn` there
    // is genuinely new programmatic surface Plan 030 would be introducing).
    // Files like `src/server/mod.rs`, `connection.rs`, and `packages/*.rs`
    // carry legitimate pre-existing pub API for the `clay-server` binary and
    // the package backend trait used by tests, so they are excluded; the
    // specific Plan 030 helpers in `workspace.rs` are pinned by name below.
    let plan_030_new_surface_files = [
        "src/perf/budgets.rs",
        "src/server/workspace.rs",
        "src/server/js_runtime.rs",
        "src/server/ops/sdui.rs",
        "src/server/ui.rs",
        "src/server/behavior.rs",
    ];
    for file in plan_030_new_surface_files {
        let path = root.join(file);
        if !path.exists() {
            continue;
        }
        let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {file}: {e}"));
        for (i, line) in src.lines().enumerate() {
            let t = line.trim_start();
            // Allow `pub(crate)`, `pub(super)`, `pub(crate) async`, etc.
            if t.starts_with("pub fn ") || t.starts_with("pub async fn ") {
                panic!(
                    "Plan 030 file {file}: line {} introduces a bare `pub fn`/`pub async fn` \
                     (`{}`). New programmatic surface must use pub(crate)/pub(super) or route \
                     through an existing deno_core op + Clay JS facade mapped in \
                     api-inventory.toml.",
                    i + 1,
                    t.trim()
                );
            }
        }
    }

    // (2) The specific Plan 030 server-internal helpers are restricted-visibility
    // (pub(crate) or private), never bare `pub`.
    let workspace_src =
        fs::read_to_string(root.join("src/server/workspace.rs")).expect("read workspace.rs");
    for restricted in [
        "pub(crate) async fn open_existing_file_unlocked",
        "pub(crate) async fn open_selected_file_unlocked",
        "pub(crate) async fn save_document_unlocked",
        "pub(crate) async fn reload_document_unlocked",
        "fn check_openable_size",
        "async fn open_io",
        "async fn save_io",
        "async fn reload_io",
        "fn atomic_temp_path",
        "async fn atomic_write_file",
    ] {
        assert!(
            workspace_src.contains(restricted),
            "Plan 030 workspace helper must keep restricted visibility: `{restricted}`"
        );
    }

    // (3) Plan 030 internal-only helpers must NOT be mapped as Clay JS APIs.
    // `install_command_args` is intentionally a pub test helper on PnpmBackend
    // (a backend type, not a deno_core op); it is not a programmatic JS surface.
    for forbidden_inventory_substring in [
        "open_existing_file_unlocked",
        "open_selected_file_unlocked",
        "save_document_unlocked",
        "reload_document_unlocked",
        "with_timeout",
        "install_command_args",
        "atomic_write_file",
        "check_openable_size",
    ] {
        // These names must not appear as a deno_op mapping target.
        assert!(
            !inventory.contains(&format!("deno_op = \"op_{forbidden_inventory_substring}\"")),
            "Plan 030 internal helper `{forbidden_inventory_substring}` must not be exposed as a deno_core op"
        );
    }

    // (4) No Plan 030 budget became a Clay JS API id. The configuration task
    // already pins the configuration namespace; this pins the broader Clay JS
    // API namespace against the same budget surfaces.
    for forbidden_id in [
        "clay.runtime.setTimeout",
        "clay.runtime.setEvaluationBudget",
        "clay.documents.setMaxOpenableSize",
        "clay.sdui.setTreeBudget",
        "clay.packages.allowLifecycleScripts",
    ] {
        assert!(
            inventory_entries()
                .iter()
                .all(|entry| entry.get("id") != forbidden_id),
            "Plan 030 budget must not be exposed as Clay JS API `{forbidden_id}`"
        );
    }
}
