use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct InventoryEntry {
    fields: BTreeMap<String, String>,
}

impl InventoryEntry {
    fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }
}

fn inventory_entries() -> Vec<InventoryEntry> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/reference/clay-js-api/api-inventory.toml"
    );
    let text = std::fs::read_to_string(path).expect("read api inventory");
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
        current
            .as_mut()
            .expect("inventory key/value appears inside an [[api]] table")
            .insert(key.to_string(), value.trim().trim_matches('"').to_string());
    }

    if let Some(fields) = current {
        entries.push(InventoryEntry { fields });
    }

    entries
}

fn inventory_rust_mapping_text() -> String {
    inventory_entries()
        .into_iter()
        .map(|entry| {
            format!(
                "{}\n{}\n{}\n{}",
                entry.get("backing_rust"),
                entry.get("current_rust_owner"),
                entry.get("deno_op"),
                entry.get("facade_path")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn public_items_in_dir(relative_dir: &str) -> Vec<String> {
    let source_dir = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), relative_dir);
    let mut items = Vec::new();

    for entry in
        std::fs::read_dir(&source_dir).unwrap_or_else(|err| panic!("read {relative_dir}: {err}"))
    {
        let entry = entry.expect("source dir entry");
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 source file name")
            .to_string();
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let mut current_impl: Option<String> = None;
        let mut impl_brace_depth = 0isize;

        for line in source.lines() {
            let trimmed = line.trim();
            let starts_impl = trimmed.starts_with("impl ");
            if let Some(rest) = trimmed.strip_prefix("impl ") {
                current_impl = rest
                    .split(|character: char| character == '{' || character.is_whitespace())
                    .next()
                    .filter(|name| !name.is_empty())
                    .map(str::to_string);
                impl_brace_depth = 0;
            }

            if trimmed.starts_with("pub ") {
                let tokens: Vec<_> = trimmed
                    .split(|character: char| character.is_whitespace() || character == '(')
                    .filter(|token| !token.is_empty())
                    .collect();
                if let Some(kind_index) = tokens.iter().position(|token| {
                    matches!(
                        *token,
                        "struct" | "enum" | "trait" | "type" | "const" | "static" | "fn"
                    )
                }) {
                    if let Some(name) = tokens.get(kind_index + 1) {
                        let name = name.trim_end_matches(':');
                        let rust_path = match tokens[kind_index] {
                            "fn" => match &current_impl {
                                Some(owner) => {
                                    format!("{relative_dir}/{file_name}::{owner}::{name}")
                                }
                                None => format!("{relative_dir}/{file_name}::{name}"),
                            },
                            _ => format!("{relative_dir}/{file_name}::{name}"),
                        };
                        items.push(rust_path);
                    }
                }
            }

            if current_impl.is_some() {
                impl_brace_depth += trimmed.matches('{').count() as isize;
                impl_brace_depth -= trimmed.matches('}').count() as isize;
                if !starts_impl && impl_brace_depth <= 0 {
                    current_impl = None;
                }
            }
        }
    }

    items.sort();
    items
}

fn public_server_items() -> Vec<String> {
    public_items_in_dir("src/server")
}

fn public_docs_items() -> Vec<String> {
    public_items_in_dir("src/docs")
}

#[test]
fn server_public_items_have_api_inventory_entries_or_are_allowlisted() {
    let inventory_text = inventory_rust_mapping_text();
    let allowlisted_infrastructure: BTreeSet<&str> = [
        "src/server/mod.rs::IpcServer::new",
        "src/server/mod.rs::IpcServer::try_new",
        "src/server/mod.rs::IpcServer::run",
        "src/server/mod.rs::ServerConfig::new",
        "src/server/decorations.rs::DecorationValidationError",
        "src/server/decorations.rs::validate_decoration_publication",
        "src/server/decorations.rs::validate_decoration_set",
        "src/server/parse_coordinator.rs::ParseCoordinator::new",
        "src/server/parse_coordinator.rs::ParseCoordinator::next_update",
        "src/server/parse_coordinator.rs::ParseCoordinator::register_handler",
        "src/server/parse_coordinator.rs::ParseCoordinator::schedule_parse",
        "src/server/parse_coordinator.rs::ParseCoordinator::schedule_parse_with_windows",
        "src/server/parse_coordinator.rs::ParseCoordinator::stats",
        "src/server/parse_coordinator.rs::ParseCoordinator::validate_update",
        "src/server/parse_coordinator.rs::ParseCoordinatorError",
        "src/server/parse_coordinator.rs::ParseCoordinatorStats",
        "src/server/parse_coordinator.rs::ParseHandler",
        "src/server/parse_coordinator.rs::ParseHandlerFuture",
        "src/server/parse_coordinator.rs::ParseHandlerMeta",
        "src/server/parse_coordinator.rs::ParseScheduleRequest",
    ]
    .into_iter()
    .collect();

    let unmapped: Vec<_> = public_server_items()
        .into_iter()
        .filter(|item| !allowlisted_infrastructure.contains(item.as_str()))
        .filter(|item| !inventory_text.contains(item))
        .collect();

    assert!(
        unmapped.is_empty(),
        "public server Rust items must be either mapped in docs/reference/clay-js-api/api-inventory.toml or explicitly allowlisted as non-JS server infrastructure: {unmapped:?}"
    );
}

#[test]
fn server_public_functions_are_private_or_facade_backed() {
    let inventory_text = inventory_rust_mapping_text();
    let internal_large_file_primitives: BTreeSet<&str> = [
        "src/server/decorations.rs::validate_decoration_set",
        "src/server/parse_coordinator.rs::ParseCoordinator::schedule_parse_with_windows",
        "src/server/parse_coordinator.rs::ParseCoordinator::stats",
        "src/server/parse_coordinator.rs::ParseCoordinator::validate_update",
    ]
    .into_iter()
    .collect();

    for facade_backed in [
        "src/server/decorations.rs::validate_decoration_publication",
        "src/server/parse_coordinator.rs::ParseCoordinator::register_handler",
        "op_clay_decorations_publish_decorations",
        "op_clay_parse_register_parse_handler",
        "runtime/js/decorations.ts::serverPublishDecorations",
        "runtime/js/parse.ts::serverRegisterParseHandler",
    ] {
        assert!(
            inventory_text.contains(facade_backed),
            "large-file public capability {facade_backed} must be mapped through Clay JS API inventory"
        );
    }

    for internal in internal_large_file_primitives {
        assert!(
            public_server_items().iter().any(|item| item == internal),
            "{internal} is expected to remain classified as internal server infrastructure while public to the crate"
        );
        assert!(
            !inventory_text.contains(internal),
            "{internal} must not become a user-facing API without a dedicated Clay JS facade and docs"
        );
    }
}

#[test]
fn rust_visibility_mapping_has_no_unmapped_public_primitive_functions() {
    let inventory_text = inventory_rust_mapping_text();
    for mapped in [
        "src/packages/manifest.rs::validate_manifest_value",
        "src/packages/permissions.rs::parse_permission",
        "src/packages/record.rs::assemble_package_record",
        "src/packages/modes.rs::ModeRegistry::register_mode",
        "src/packages/modes.rs::ModeRegistry::classify",
        "src/packages/modes.rs::ModeRegistry::activate_major_mode",
        "src/packages/modes.rs::ModeRegistry::select_behavior_manifest_for_document",
        "src/packages/commands.rs::CommandRegistry::register_command",
        "src/packages/commands.rs::CommandRegistry::list",
        "op_clay_packages_validate_manifest",
        "op_clay_packages_validate_permissions",
        "op_clay_packages_load_package",
        "op_clay_modes_register_pattern",
        "op_clay_modes_classify_document",
        "op_clay_modes_activate_major_mode",
        "op_clay_commands_register_command",
        "op_clay_commands_list_commands",
        "runtime/js/packages.ts::serverValidatePackageManifest",
        "runtime/js/packages.ts::serverLoadPackage",
        "runtime/js/modes.ts::serverActivateMajorMode",
        "runtime/js/commands.ts::serverRegisterCommand",
    ] {
        assert!(
            inventory_text.contains(mapped),
            "primitive gate public Rust/op/facade capability {mapped} must be mapped in api-inventory.toml"
        );
    }
}

#[test]
fn phase18_1_shell_layout_has_no_unmapped_runtime_or_rust_surfaces() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let shell_layout_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.get("id").starts_with("clay.ui."))
        .collect();

    assert_eq!(
        shell_layout_entries.len(),
        10,
        "Phase 18.4 should have ten shell/layout clay:ui surfaces after input contribution promotion"
    );
    assert!(
        root.join("runtime/js/ui.ts").exists(),
        "Phase 18.3 adds a runtime clay:ui contribution facade and public docs for contribution APIs"
    );

    let lib_source = std::fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let shell_strategy =
        std::fs::read_to_string(root.join("docs/reference/primitives/shell-layout-strategy.md"))
            .expect("read shell layout strategy");
    assert!(
        root.join("src/shell").exists(),
        "Phase 18.2 should have internal shell runtime Rust modules"
    );
    assert!(
        lib_source.contains("pub(crate) mod shell"),
        "Phase 18.2 shell runtime must stay crate-private until public clay:ui APIs ship"
    );
    for required in [
        "**Still planned/package-facing after Phase 18.3:** public callable working-area, pane-split, and pane-slot layout mutation/default APIs",
        "Planned-only `clay.ui.*` inventory entries remain `status = \"planned\"`, `registry_public = false`",
        "the four Phase 18.3 contribution entries are `status = \"runtime-backed\"`, `registry_public = true`",
        "both Phase 18.4 entries are `status = \"runtime-backed\"`, `registry_public = true`",
    ] {
        assert!(
            shell_strategy.contains(required),
            "shell layout docs must explain crate-private runtime vs planned public API status: {required}"
        );
    }

    for entry in shell_layout_entries {
        let id = entry.get("id");
        assert!(
            entry.get("facade_path").starts_with("runtime/js/ui.ts::"),
            "{id} keeps the clay:ui facade namespace"
        );
        if matches!(
            id,
            "clay.ui.serverRegisterPanelContribution"
                | "clay.ui.serverRegisterComponentContribution"
                | "clay.ui.serverRegisterTransientOverlayContribution"
                | "clay.ui.serverRegisterInputContribution"
                | "clay.ui.serverRegisterUiStateScope"
                | "clay.ui.serverSetLayoutOverride"
                | "clay.ui.serverRegisterThemeToken"
        ) {
            assert_eq!(
                entry.get("registry_public"),
                "true",
                "{id} is generated after per-API Markdown docs are linked"
            );
            assert_eq!(
                entry.get("status"),
                "runtime-backed",
                "{id} is runtime-backed after its implementation phase"
            );
            assert!(entry.get("deno_op").starts_with("op_clay_ui_"));
            assert!(
                entry
                    .get("backing_rust")
                    .starts_with("src/server/ui.rs::PackageUiRegistry")
            );
        } else {
            assert_eq!(
                entry.get("registry_public"),
                "false",
                "{id} must not be generated before implementation and per-API Markdown docs are linked"
            );
            assert_eq!(entry.get("status"), "planned", "{id} remains planned-only");
            assert_eq!(
                entry.get("deno_op"),
                "op_clay_runtime_unavailable",
                "{id} must not claim an implemented op"
            );
            assert!(
                entry.get("backing_rust").starts_with("planned:"),
                "{id} backing Rust must stay marked planned until a runtime surface ships"
            );
        }
    }

    let server_public_items = public_server_items().join("\n");
    for forbidden in [
        "WorkingAreaLayout",
        "PaneSplitTree",
        "PaneSlotLayout",
        "PanelContribution",
    ] {
        assert!(
            !server_public_items.contains(forbidden),
            "Phase 18.2 must not introduce public server-side Rust shell/layout primitive {forbidden} without a runtime-backed Clay JS API mapping"
        );
    }
}

#[test]
fn phase18_4_public_rust_surfaces_have_clay_js_mapping_or_internal_visibility() {
    let inventory_text = inventory_rust_mapping_text();

    for required_mapping in [
        "src/server/ui.rs::PackageUiRegistry::register_input",
        "src/server/ui.rs::PackageUiRegistry::register_ui_state_scope",
        "src/server/ui.rs::PackageUiRegistry::set_layout_override",
        "src/server/configuration.rs::ConfigurationRuntime::set_package_option",
        "op_clay_ui_register_input_contribution",
        "op_clay_ui_register_ui_state_scope",
        "op_clay_ui_set_layout_override",
        "op_clay_configuration_set_package_option",
        "runtime/js/ui.ts::serverRegisterInputContribution",
        "runtime/js/ui.ts::serverRegisterUiStateScope",
        "runtime/js/ui.ts::serverSetLayoutOverride",
        "runtime/js/configuration.ts::setPackageOption",
    ] {
        assert!(
            inventory_text.contains(required_mapping),
            "Phase 18.4 public programmatic surface must be mapped through Clay JS API inventory: {required_mapping}"
        );
    }

    let server_public_items = public_server_items().join("\n");
    for internal_type in [
        "PackageInputContribution",
        "PackageUiStateScope",
        "PackageLayoutOverride",
        "PackageOwnedConfiguration",
    ] {
        assert!(
            !server_public_items.contains(internal_type),
            "{internal_type} must not become a raw public server Rust API outside the Clay JS facade/registry contract"
        );
    }
}

#[test]
fn phase18_2_shell_native_public_items_are_binary_only_or_crate_private() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_public_items = public_items_in_dir("src");
    let shell_native_public: BTreeSet<_> = root_public_items
        .into_iter()
        .filter(|item| item.contains("masonry_shell.rs"))
        .collect();
    let expected_shell_native_public = BTreeSet::from([
        "src/masonry_shell.rs::ClayShellWidget".to_string(),
        "src/masonry_shell.rs::ClayShellWidget::editor_widget_id".to_string(),
        "src/masonry_shell.rs::ClayShellWidget::focus_fallback_widget_id".to_string(),
        "src/masonry_shell.rs::ClayShellWidget::single_editor".to_string(),
    ]);

    assert_eq!(
        shell_native_public, expected_shell_native_public,
        "Phase 18.2 may expose only the doc-hidden native shell constructor/focus accessors needed by the package binary boundary"
    );

    let inventory_text = inventory_rust_mapping_text();
    for binary_only in &expected_shell_native_public {
        assert!(
            !inventory_text.contains(binary_only),
            "{binary_only} is a native binary-boundary helper, not a Clay JS API mapping"
        );
    }

    let lib_source = std::fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let shell_source =
        std::fs::read_to_string(root.join("src/masonry_shell.rs")).expect("read shell source");
    let shell_layout_source =
        std::fs::read_to_string(root.join("src/shell/layout.rs")).expect("read shell layout");
    let strategy =
        std::fs::read_to_string(root.join("docs/reference/primitives/shell-layout-strategy.md"))
            .expect("read shell layout strategy");

    assert!(lib_source.contains("#[doc(hidden)]"));
    assert!(lib_source.contains("pub mod masonry_shell;"));
    assert!(shell_source.contains("#[doc(hidden)]"));
    assert!(shell_source.contains("pub struct ClayShellWidget"));
    for internal_surface in [
        "pub(crate) fn apply_layout_update",
        "pub(crate) fn observable_snapshot",
        "pub(crate) struct ShellObservableSnapshot",
        "pub(crate) struct WorkingAreaLayout",
        "pub(crate) struct WorkingAreaLayoutUpdate",
        "pub(crate) struct WorkingAreaLayoutObservation",
        "pub(crate) struct PaneSplitTree",
        "pub(crate) struct PaneSlotLayout",
    ] {
        assert!(
            shell_source.contains(internal_surface)
                || shell_layout_source.contains(internal_surface),
            "expected internal shell surface {internal_surface} to stay crate-private"
        );
    }
    for forbidden_public in [
        "pub fn apply_layout_update",
        "pub fn observable_snapshot",
        "pub struct WorkingAreaLayout",
        "pub struct WorkingAreaLayoutUpdate",
        "pub struct WorkingAreaLayoutObservation",
        "pub struct PaneSplitTree",
        "pub struct PaneSlotLayout",
    ] {
        assert!(
            !shell_source.contains(forbidden_public)
                && !shell_layout_source.contains(forbidden_public),
            "{forbidden_public} must not bypass Clay JS API docs/facade/registry coverage"
        );
    }
    for required in [
        "Rust visibility audit",
        "introduces no new public server-side Rust shell/layout functions",
        "binary/library boundary",
        "not package-extensibility APIs",
        "remain `pub(crate)`",
    ] {
        assert!(
            strategy.contains(required),
            "shell strategy must document native Rust visibility rationale: {required}"
        );
    }
}

#[test]
fn open_dialog_internal_helpers_are_private_or_inventory_mapped() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let inventory_text = inventory_rust_mapping_text();
    let file_dialog_source = std::fs::read_to_string(root.join("src/client/file_dialog.rs"))
        .expect("read client file dialog source");
    let main_source = std::fs::read_to_string(root.join("src/main.rs")).expect("read main source");
    let workspace_source = std::fs::read_to_string(root.join("src/server/workspace.rs"))
        .expect("read workspace source");
    let connection_source = std::fs::read_to_string(root.join("src/server/connection.rs"))
        .expect("read connection source");

    for mapped in [
        "runtime/js/documents.ts::clientOpenFileDialog",
        "src/client/file_dialog.rs::FileDialogResult",
        "src/client/file_dialog.rs::open_markdown_file_dialog",
        "src/main.rs::handle_client_ui_command",
        "op_clay_keybindings_bind_key",
    ] {
        assert!(
            inventory_text.contains(mapped),
            "open-dialog public command/Rust boundary {mapped} must be mapped in the Clay JS API inventory"
        );
    }

    for private_helper in [
        "fn show_file_open_dialog",
        "fn set_markdown_filters",
        "fn wide_null",
        "fn is_cancelled",
    ] {
        assert!(
            file_dialog_source.contains(private_helper),
            "expected helper {private_helper} in file dialog source"
        );
        assert!(
            !file_dialog_source.contains(&format!("pub {private_helper}")),
            "{private_helper} must remain private implementation detail"
        );
    }
    assert!(main_source.contains("fn handle_client_ui_command"));
    assert!(
        !main_source.contains("pub fn handle_client_ui_command"),
        "native command dispatch must not become a public Rust API"
    );
    assert!(workspace_source.contains("pub(crate) async fn open_selected_file"));
    assert!(workspace_source.contains("enum WorkspaceAuthority"));
    assert!(!workspace_source.contains("pub enum WorkspaceAuthority"));
    assert!(connection_source.contains("async fn open_selected_file_response"));
    assert!(!connection_source.contains("pub async fn open_selected_file_response"));
}

#[test]
fn docs_public_items_are_internal_registry_infrastructure() {
    let allowlisted_docs_infrastructure: BTreeSet<&str> = [
        "src/docs/registry.rs::GENERATED_REGISTRY_PATH",
        "src/docs/registry.rs::UPDATE_COMMAND",
        "src/docs/registry.rs::CustomProperty",
        "src/docs/registry.rs::RegistryEntry",
        "src/docs/registry.rs::ClayJsApiRegistry",
        "src/docs/registry.rs::RegistryError",
        "src/docs/registry.rs::RegistryResult<T>",
        "src/docs/registry.rs::ClayJsApiRegistry::from_docs",
        "src/docs/registry.rs::ClayJsApiRegistry::from_generated",
        "src/docs/registry.rs::ClayJsApiRegistry::from_generated_json",
        "src/docs/registry.rs::ClayJsApiRegistry::by_id",
        "src/docs/registry.rs::ClayJsApiRegistry::by_js_export",
        "src/docs/registry.rs::ClayJsApiRegistry::by_user_facing_name",
        "src/docs/registry.rs::ClayJsApiRegistry::by_kind_owner",
        "src/docs/registry.rs::ClayJsApiRegistry::by_lookup_tag",
        "src/docs/registry.rs::ClayJsApiRegistry::by_key_binding",
        "src/docs/registry.rs::ClayJsApiRegistry::by_custom_property",
        "src/docs/registry.rs::ClayJsApiRegistry::to_generated_json",
        "src/docs/registry.rs::repository_root",
        "src/docs/registry.rs::expected_generated_registry",
        "src/docs/registry.rs::update_generated_registry",
        "src/docs/registry.rs::check_generated_registry_current",
        "src/docs/registry.rs::registry_source_paths",
    ]
    .into_iter()
    .collect();

    let unclassified: Vec<_> = public_docs_items()
        .into_iter()
        .filter(|item| !allowlisted_docs_infrastructure.contains(item.as_str()))
        .collect();

    assert!(
        unclassified.is_empty(),
        "public src/docs Rust items must be classified as internal documentation-registry infrastructure or promoted through Clay JS API docs/inventory before becoming user-facing APIs: {unclassified:?}"
    );
}
