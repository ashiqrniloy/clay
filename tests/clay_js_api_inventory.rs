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

    for id in ["clay.folding.serverPublishFoldingRanges"] {
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
