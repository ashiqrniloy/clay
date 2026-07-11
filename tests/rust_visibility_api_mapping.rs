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
                }) && let Some(name) = tokens.get(kind_index + 1)
                {
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
        "src/server/mod.rs::IpcServer::trigger_developer_hot_reload",
        "src/server/mod.rs::ReloadedDocumentRefresh",
        "src/server/mod.rs::RuntimeReloadOutcome",
        "src/server/mod.rs::ServerConfig::new",
        "src/server/decorations.rs::DecorationValidationError",
        "src/server/decorations.rs::validate_decoration_publication",
        "src/server/decorations.rs::validate_decoration_set",
        "src/server/parse_coordinator.rs::ParseCoordinator::cancel_generation",
        "src/server/parse_coordinator.rs::ParseCoordinator::cancel_package",
        "src/server/parse_coordinator.rs::ParseCoordinator::new",
        "src/server/parse_coordinator.rs::ParseCoordinator::next_diagnostic",
        "src/server/parse_coordinator.rs::ParseCoordinator::next_update",
        "src/server/parse_coordinator.rs::ParseCoordinator::register_handler",
        "src/server/parse_coordinator.rs::ParseCoordinator::register_handler_for_generation",
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
        "src/server/runtime_sandbox.rs::RuntimeSandboxError",
        "src/server/runtime_sandbox.rs::RuntimeSandboxSupervisor",
        "src/server/runtime_sandbox.rs::RuntimeSandboxSupervisor::evaluate",
        "src/server/runtime_sandbox.rs::RuntimeSandboxSupervisor::shutdown",
        "src/server/runtime_sandbox.rs::RuntimeSandboxSupervisor::spawn",
        "src/server/runtime_sandbox.rs::SandboxEvaluation",
        "src/server/syntax.rs::ActiveSyntaxGrammar",
        "src/server/syntax.rs::NativeGrammarDescriptor",
        "src/server/syntax.rs::SyntaxCapture",
        "src/server/syntax.rs::SyntaxEngineTier",
        "src/server/syntax.rs::SyntaxGrammarContribution",
        "src/server/syntax.rs::SyntaxGrammarContribution::provenance",
        "src/server/syntax.rs::SyntaxGrammarContribution::web_tree_sitter_artifact_contract",
        "src/server/syntax.rs::SyntaxGrammarPatternKind",
        "src/server/syntax.rs::SyntaxGrammarSelection",
        "src/server/syntax.rs::SyntaxVocabularySpan",
        "src/server/syntax.rs::TreeSitterSyntaxError",
        "src/server/syntax.rs::TreeSitterSyntaxHandler",
        "src/server/syntax.rs::TreeSitterSyntaxHandler::cached_tree_version",
        "src/server/syntax.rs::TreeSitterSyntaxHandler::new",
        "src/server/syntax.rs::TreeSitterSyntaxHandler::parse_sync",
        "src/server/syntax.rs::SyntaxGrammarRegistry",
        "src/server/syntax.rs::SyntaxGrammarRegistry::active_selection",
        "src/server/syntax.rs::SyntaxGrammarRegistry::find_for_extension",
        "src/server/syntax.rs::SyntaxGrammarRegistry::first_party_native_descriptors",
        "src/server/syntax.rs::SyntaxGrammarRegistry::find_for_file_name",
        "src/server/syntax.rs::SyntaxGrammarRegistry::get",
        "src/server/syntax.rs::SyntaxGrammarRegistry::list",
        "src/server/syntax.rs::SyntaxGrammarRegistry::native_language",
        "src/server/syntax.rs::SyntaxGrammarRegistry::new",
        "src/server/syntax.rs::SyntaxGrammarRegistry::register_first_party_native_grammars",
        "src/server/syntax.rs::SyntaxGrammarRegistry::register_package",
        "src/server/syntax.rs::SyntaxGrammarRegistry::register_package_with_explicit_tier2_override",
        "src/server/syntax.rs::SyntaxGrammarRegistry::select_for_document",
        "src/server/syntax.rs::SyntaxGrammarRegistry::with_first_party_native",
        "src/server/syntax.rs::SyntaxGrammarRegistryError",
        "src/server/syntax.rs::TreeSitterSyntaxHandler::parser_cache_id",
        "src/server/syntax.rs::WebTreeSitterArtifactContract",
        "src/server/syntax.rs::WebTreeSitterArtifactError",
        "src/server/syntax.rs::map_capture_to_vocabulary",
        "src/server/command_execution.rs::CommandExecutionDiagnostic",
        "src/server/command_execution.rs::CommandExecutionProvenance",
        "src/server/command_execution.rs::CommandExecutionRequest",
        "src/server/command_execution.rs::CommandExecutionResult",
        "src/server/command_execution.rs::CommandExecutionRule",
        "src/server/command_execution.rs::CommandExecutionStatus",
        "src/server/command_execution.rs::CommandExecutionTarget",
        // Git command result structs are server-internal command payloads;
        // public Clay JS API docs cover the command facades, not these Rust
        // transport helper types.
        "src/server/command_execution.rs::GitCommandResult",
        "src/server/git.rs::GitCachedStatus",
        "src/server/command_execution.rs::DiscoveryResult",
        "src/server/command_execution.rs::CommandExecutor::execute",
        "src/server/command_execution.rs::CommandExecutor::execute_discovery",
        "src/server/command_execution.rs::CommandExecutor::execute_registered",
        "src/server/command_execution.rs::CommandExecutor::new",
        "src/server/command_execution.rs::CommandExecutor;",
        "src/server/command_execution.rs::builtin_server_command",
        "src/server/command_execution.rs::builtin_server_command_ids",
        // Phase 18.11 completion provider framework is server-internal
        // infrastructure; the only public Clay JS API is
        // clay.completion.serverRegisterCompletionProvider, backed by
        // src/server/ops/completion.rs::op_clay_completion_register_completion_provider.
        "src/server/completion.rs::BufferWordCompletionProvider",
        "src/server/completion.rs::BufferWordCompletionProvider;",
        "src/server/completion.rs::BufferWordCompletionProvider::meta",
        "src/server/completion.rs::CompletionCoordinator",
        "src/server/completion.rs::CompletionCoordinator::bump_generation",
        "src/server/completion.rs::CompletionCoordinator::cancel_generation",
        "src/server/completion.rs::CompletionCoordinator::cancel_package",
        "src/server/completion.rs::CompletionCoordinator::new",
        "src/server/completion.rs::CompletionCoordinator::next_result",
        "src/server/completion.rs::CompletionCoordinator::providers",
        "src/server/completion.rs::CompletionCoordinator::register_builtin",
        "src/server/completion.rs::CompletionCoordinator::register_builtin_buffer_words",
        "src/server/completion.rs::CompletionCoordinator::register_package",
        "src/server/completion.rs::CompletionCoordinator::schedule_completion",
        "src/server/completion.rs::CompletionCoordinator::stats",
        "src/server/completion.rs::CompletionCoordinator::validate_result",
        "src/server/completion.rs::CompletionCoordinatorError",
        "src/server/completion.rs::CompletionCoordinatorStats",
        "src/server/completion.rs::CompletionDocumentWindow",
        "src/server/completion.rs::CompletionDocumentWindow::byte_range",
        "src/server/completion.rs::CompletionDocumentWindow::text_len_bytes",
        "src/server/completion.rs::CompletionProviderError",
        "src/server/completion.rs::CompletionProviderFuture",
        "src/server/completion.rs::CompletionProviderMeta::builtin_core",
        "src/server/completion.rs::CompletionProviderRegistry",
        "src/server/completion.rs::CompletionProviderRegistry::get",
        "src/server/completion.rs::CompletionProviderRegistry::is_empty",
        "src/server/completion.rs::CompletionProviderRegistry::len",
        "src/server/completion.rs::CompletionProviderRegistry::list_ordered",
        "src/server/completion.rs::CompletionProviderRegistry::new",
        "src/server/completion.rs::CompletionProviderRegistry::providers_for_trigger_character",
        "src/server/completion.rs::CompletionProviderRegistry::register_builtin",
        "src/server/completion.rs::CompletionProviderRegistry::register_package",
        "src/server/completion.rs::CompletionProviderRegistry::remove_older_generations",
        "src/server/completion.rs::CompletionProviderRegistry::remove_package",
        "src/server/completion.rs::CompletionProviderRegistry::unregister",
        "src/server/completion.rs::CompletionProviderRegistryError",
        "src/server/completion.rs::CompletionTriggerMetadata",
        "src/server/completion.rs::ID",
        "src/server/completion.rs::JsCompletionProviderRegistration",
        "src/server/completion.rs::WordBoundaryRule",
        "src/server/completion.rs::WordBoundaryRule::default_buffer_word",
        "src/server/completion.rs::WordBoundaryRule::new",
        "src/server/command_execution.rs::WorkspaceActionResult",
        "src/server/workspace.rs::OpenDocumentSnapshot",
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
fn phase18_16_public_rust_surfaces_have_js_api_or_are_crate_private() {
    let inventory_text = inventory_rust_mapping_text();
    let public_items: BTreeSet<_> = public_server_items().into_iter().collect();

    for (rust_path, op, facade) in [
        (
            "src/server/syntax.rs::SyntaxGrammarRegistry::register_package",
            "op_clay_syntax_register_syntax_grammar",
            "runtime/js/syntax.ts::serverRegisterSyntaxGrammar",
        ),
        (
            "src/server/syntax.rs::SyntaxGrammarRegistry::set_engine_preference",
            "op_clay_syntax_set_engine_preference",
            "runtime/js/syntax.ts::setSyntaxEnginePreference",
        ),
    ] {
        assert!(
            public_items.contains(rust_path),
            "Phase 18.16 public Rust capability should be inventory-visible: {rust_path}"
        );
        assert!(
            inventory_text.contains(rust_path),
            "Phase 18.16 public Rust capability must map to Clay JS API inventory: {rust_path}"
        );
        assert!(
            inventory_text.contains(op),
            "Phase 18.16 public Rust capability {rust_path} must have op wrapper {op}"
        );
        assert!(
            inventory_text.contains(facade),
            "Phase 18.16 public Rust capability {rust_path} must have facade {facade}"
        );
    }

    for internal_surface in [
        "src/server/parse_coordinator.rs::ParseCoordinator::next_diagnostic",
        "src/server/syntax.rs::SyntaxGrammarRegistry::register_first_party_native_grammars",
        "src/server/syntax.rs::SyntaxGrammarRegistry::register_package_with_explicit_tier2_override",
        "src/server/syntax.rs::SyntaxGrammarRegistry::native_language",
        "src/server/syntax.rs::TreeSitterSyntaxHandler::parser_cache_id",
        "src/server/syntax.rs::map_capture_to_vocabulary",
    ] {
        assert!(
            public_items.contains(internal_surface),
            "Phase 18.16 internal server primitive should remain explicitly reviewed: {internal_surface}"
        );
        assert!(
            !inventory_text.contains(&format!("backing_rust = \"{internal_surface}\"")),
            "internal Phase 18.16 primitive {internal_surface} must not be promoted as a direct Clay JS API backing surface"
        );
    }
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

/// Every `unsafe` block in the COM file-dialog module must carry a `// SAFETY:`
/// comment stating the invariant that makes it safe (Plan 030 P3-2).
#[test]
fn file_dialog_unsafe_blocks_have_safety_comments() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("src/client/file_dialog.rs"))
        .expect("read src/client/file_dialog.rs");

    let mut missing: Vec<usize> = Vec::new();
    for (lineno, line) in source.lines().enumerate() {
        if !line.contains("unsafe") || line.trim_start().starts_with("//") {
            continue;
        }
        // Look back up to 8 lines for a `// SAFETY:` comment, skipping blanks
        // and other comments, so multi-line comments still count.
        let start = lineno.saturating_sub(8);
        let preceding = &source.lines().collect::<Vec<_>>()[start..lineno];
        let has_safety = preceding.iter().any(|prev| prev.contains("SAFETY:"));
        if !has_safety {
            missing.push(lineno + 1);
        }
    }

    assert!(
        missing.is_empty(),
        "each `unsafe` block in src/client/file_dialog.rs must have a preceding
         `// SAFETY:` comment stating its invariant (Plan 030 P3-2); missing at
         lines: {missing:?}"
    );
}

#[test]
fn plan_035_unified_package_authority_public_surfaces_are_mapped_or_internal() {
    // Plan 035 introduces package-source provenance, user authorization,
    // package graph relations, conflict resolution, and package-scoped
    // revocation. Public server/package Rust primitives that are intended as
    // future Clay JS APIs must be mapped in the inventory; internal primitives
    // must be explicitly allowlisted.
    let inventory_text = inventory_rust_mapping_text();

    // Public package-management capabilities planned as Clay JS APIs.
    for mapped in [
        "op_clay_packages_install",
        "op_clay_packages_enable",
        "op_clay_packages_disable",
        "op_clay_packages_inspect",
        "op_clay_packages_list",
        "op_clay_packages_authorize",
        "op_clay_packages_set_conflict_override",
        "runtime/js/packages.ts::install",
        "runtime/js/packages.ts::enable",
        "runtime/js/packages.ts::disable",
        "runtime/js/packages.ts::inspect",
        "runtime/js/packages.ts::list",
        "runtime/js/packages.ts::authorize",
        "runtime/js/packages.ts::setConflictOverride",
        "src/packages/conflict.rs::PackageConflictResolutionPolicy",
        "src/packages/conflict.rs::PackageConflictResolutionDiagnostic",
        "src/packages/conflict.rs::PackageConflictResolutionReason",
    ] {
        assert!(
            inventory_text.contains(mapped),
            "Plan 035 intended public programmatic surface {mapped} must be mapped in api-inventory.toml"
        );
    }

    // Internal revocation/primitive helpers that are not user-facing APIs.
    let internal_primitives: std::collections::BTreeSet<&str> = [
        "src/server/parse_coordinator.rs::ParseCoordinator::cancel_package",
        "src/server/js_runtime.rs::PackageLoadEntryAllowlist::revoke_package",
    ]
    .into_iter()
    .collect();
    for internal in internal_primitives {
        assert!(
            !inventory_text.contains(internal),
            "{internal} is an internal primitive helper and must not be mapped as a user-facing Clay JS API"
        );
    }
}

#[test]
fn phase18_8_command_execution_and_transient_menu_surfaces_are_internal() {
    // Phase 18.8 added CommandExecutor, TransientMenuSession, and ControlCenter.
    // Phase 18.9 added mode-discovery resolution (CommandExecutor::execute_discovery,
    // DiscoveryResult, and the builtin_server_command(_ids) lookup that surfaces
    // clay.modes.listActiveModes / clay.modes.explainActiveMode as ServerFirst
    // commands). Command execution, transient menu sessions, the Control Center,
    // and mode discovery are NOT public Clay JS APIs: packages register/list
    // commands through existing clay:commands facades, reach the Control Center
    // via bindKey to the built-in clay.controlCenter.open command, and reach
    // mode discovery through the Control Center. The internal Rust surfaces must
    // either be explicitly allowlisted as non-JS server infrastructure or kept
    // pub(crate); no Clay JS facade/op/inventory entry may claim a public
    // execute-command, open-transient-menu, or mode-discovery API.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = inventory_entries();
    let server_public_items = public_items_in_dir("src/server");
    let inventory_text = inventory_rust_mapping_text();

    // CommandExecutor and its public-to-crate Rust types back no Clay JS API;
    // they are allowlisted as non-JS server infrastructure.
    let command_execution_infrastructure: BTreeSet<&str> = [
        "src/server/command_execution.rs::CommandExecutionDiagnostic",
        "src/server/command_execution.rs::CommandExecutionProvenance",
        "src/server/command_execution.rs::CommandExecutionRequest",
        "src/server/command_execution.rs::CommandExecutionResult",
        "src/server/command_execution.rs::CommandExecutionRule",
        "src/server/command_execution.rs::CommandExecutionStatus",
        "src/server/command_execution.rs::CommandExecutionTarget",
        "src/server/command_execution.rs::DiscoveryResult",
        "src/server/command_execution.rs::CommandExecutor::execute",
        "src/server/command_execution.rs::CommandExecutor::execute_discovery",
        "src/server/command_execution.rs::CommandExecutor::execute_registered",
        "src/server/command_execution.rs::CommandExecutor::new",
        "src/server/command_execution.rs::CommandExecutor;",
        "src/server/command_execution.rs::builtin_server_command",
        "src/server/command_execution.rs::builtin_server_command_ids",
    ]
    .into_iter()
    .collect();
    for item in command_execution_infrastructure {
        assert!(
            server_public_items.iter().any(|s| s == item),
            "CommandExecutor surface {item} must remain public to the crate (integration tests rely on it)"
        );
        let exact_inventory_mapping = format!("backing_rust = \"{item}\"");
        assert!(
            !inventory_text.contains(&exact_inventory_mapping),
            "CommandExecutor surface {item} must not be mapped as a user-facing Clay JS API"
        );
    }

    // Transient menu session state is crate-private (pub(crate)); it must not
    // appear as a public Rust surface and must not be mapped as a Clay JS API.
    let transient_menu_source = std::fs::read_to_string(root.join("src/shell/transient_menu.rs"))
        .expect("read transient menu source");
    let shell_mod_source =
        std::fs::read_to_string(root.join("src/shell/mod.rs")).expect("read shell module");
    for internal_surface in [
        "pub(crate) struct TransientMenuSessionId",
        "pub(crate) struct TransientMenuSession",
        "pub(crate) struct TransientMenuItem",
        "pub(crate) struct TransientMenuAction",
        "pub(crate) enum TransientMenuItemProvenance",
        "pub(crate) enum TransientMenuFocusPolicy",
        "pub(crate) enum TransientMenuStatus",
    ] {
        assert!(
            transient_menu_source.contains(internal_surface),
            "Phase 18.8 transient menu surface {internal_surface} must stay pub(crate)"
        );
    }
    for forbidden_public in [
        "pub struct TransientMenuSession",
        "pub struct TransientMenuItem",
        "pub struct TransientMenuAction",
        "pub enum TransientMenuStatus",
    ] {
        assert!(
            !transient_menu_source.contains(forbidden_public),
            "{forbidden_public} must not bypass the pub(crate) boundary and become a user-facing Rust API"
        );
    }
    assert!(
        shell_mod_source.contains("pub(crate) mod transient_menu")
            && shell_mod_source.contains("pub(crate) use transient_menu::TransientMenuSession"),
        "shell module must re-export TransientMenuSession as pub(crate)"
    );

    // ControlCenter is pub(crate) and backs no Clay JS API.
    let control_center_source = std::fs::read_to_string(root.join("src/server/control_center.rs"))
        .expect("read control center source");
    assert!(
        control_center_source.contains("pub(crate) struct ControlCenter"),
        "ControlCenter must stay pub(crate)"
    );
    assert!(
        !control_center_source.contains("pub struct ControlCenter"),
        "ControlCenter must not become a public user-facing Rust API"
    );

    // No inventory entry, facade, or op may claim public transient-menu/control-center APIs.
    let mut forbidden_ids = Vec::new();
    for entry in &entries {
        let id = entry.get("id");
        for forbidden in ["clay.ui.serverOpenTransientMenu", "clay.controlCenter.open"] {
            if id == forbidden {
                forbidden_ids.push(id.to_string());
            }
        }
    }
    assert!(
        forbidden_ids.is_empty(),
        "Phase 18.8 must not ship public Clay JS API inventory entries for {forbidden_ids:?}; \
         command execution/transient menu/Control Center are server-internal"
    );
    for forbidden_facade_or_op in [
        "runtime/js/ui.ts::serverOpenTransientMenu",
        "op_clay_ui_open_transient_menu",
    ] {
        assert!(
            !inventory_text.contains(forbidden_facade_or_op),
            "Phase 18.8 must not wire a public facade/op for {forbidden_facade_or_op}; \
             packages reach command execution only through server-owned CommandExecutor"
        );
    }
    assert!(
        !std::fs::read_to_string(root.join("runtime/js/ui.ts"))
            .expect("read ui facade")
            .contains("serverOpenTransientMenu"),
        "ui facade must not export a public serverOpenTransientMenu function"
    );
}
