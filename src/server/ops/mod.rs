mod behavior;
mod commands;
mod configuration;
mod decorations;
mod documents;
mod keybindings;
mod modes;
mod packages;
mod parse;
mod planned;
mod sdui;
mod ui;
mod workspace;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use deno_core::{OpState, extension, op2};
use deno_error::JsErrorBox;

use crate::{
    protocol::{
        BehaviorManifest, BehaviorScope, CommandDeclaration, DecorationSet, EditorBehaviorRules,
        KeyBindingContext, KeyBindingRule, KeyStroke,
    },
    server::{behavior::ActiveBehaviorManifest, decorations::SyntaxChunkCache},
};

use self::{
    behavior::{op_clay_behavior_get_active_manifest, op_clay_behavior_list_routes},
    commands::{op_clay_commands_list_commands, op_clay_commands_register_command},
    configuration::{
        op_clay_configuration_get_state, op_clay_configuration_load_module,
        op_clay_configuration_set_package_option,
    },
    decorations::op_clay_decorations_publish_decorations,
    documents::{
        op_clay_documents_get_document_status, op_clay_documents_list_documents,
        op_clay_documents_open_document, op_clay_documents_reload_document,
        op_clay_documents_save_document,
    },
    keybindings::{
        op_clay_keybindings_bind_key, op_clay_keybindings_list_key_bindings,
        op_clay_keybindings_unbind_key,
    },
    modes::{
        op_clay_modes_activate_major_mode, op_clay_modes_classify_document,
        op_clay_modes_register_pattern,
    },
    packages::{
        op_clay_packages_list_first_party_specifiers, op_clay_packages_load_package,
        op_clay_packages_load_package_by_specifier, op_clay_packages_validate_manifest,
        op_clay_packages_validate_permissions,
    },
    parse::{op_clay_parse_register_parse_handler, op_clay_parse_store_update},
    planned::op_clay_runtime_unavailable,
    sdui::{op_clay_sdui_define_node, op_clay_sdui_publish_tree},
    ui::{
        op_clay_ui_register_component_contribution, op_clay_ui_register_input_contribution,
        op_clay_ui_register_panel_contribution, op_clay_ui_register_theme_token,
        op_clay_ui_register_transient_overlay_contribution, op_clay_ui_register_ui_state_scope,
        op_clay_ui_set_layout_override,
    },
    workspace::op_clay_workspace_list_roots,
};

pub(crate) use self::packages::FirstPartyLoadEntryAllowlist;

/// Server-owned state shared with explicit Clay JavaScript ops.
struct ClayRuntimeContext {
    workspace: Arc<tokio::sync::Mutex<crate::server::workspace::WorkspaceState>>,
    runtime_document_id: crate::protocol::DocumentId,
}

pub(crate) struct ClayOpState {
    runtime_records: Mutex<Vec<String>>,
    published_sdui_tree: Mutex<Option<crate::protocol::SduiTree>>,
    published_decoration_set: Mutex<Option<DecorationSet>>,
    decoration_cache: Mutex<SyntaxChunkCache>,
    parse_handlers: Mutex<Vec<crate::server::parse_coordinator::ParseHandlerMeta>>,
    js_parse_handlers: Mutex<Vec<crate::server::parse_coordinator::JsParseHandlerRegistration>>,
    last_parse_update_json: Mutex<Option<String>>,
    behavior: Mutex<ActiveBehaviorManifest>,
    modes: Mutex<crate::packages::modes::ModeRegistry>,
    commands: Mutex<crate::packages::commands::CommandRegistry>,
    ui: Mutex<crate::server::ui::PackageUiRegistry>,
    runtime_context: Mutex<ClayRuntimeContext>,
    // ponytail: PackageService reuse for the first-party resolver op. The store
    // root and FakeBackend are never used by the resolver (it reads first-party
    // packages from CARGO_MANIFEST_DIR/packages); only the validate/enable path
    // (assemble_package_record + check_enabled_packages) is exercised. Upgrade
    // path: a real on-disk registry/installer when non-`@clay/*` packages land.
    first_party_packages: Mutex<crate::packages::service::PackageService>,
    load_entry_allowlist: Arc<FirstPartyLoadEntryAllowlist>,
}

impl std::fmt::Debug for ClayOpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClayOpState").finish_non_exhaustive()
    }
}

impl Default for ClayOpState {
    fn default() -> Self {
        Self::new(Arc::new(tokio::sync::Mutex::new(
            crate::server::workspace::WorkspaceState::new(),
        )))
    }
}

impl ClayOpState {
    pub(crate) fn new(
        workspace: Arc<tokio::sync::Mutex<crate::server::workspace::WorkspaceState>>,
    ) -> Self {
        Self::new_for_document(workspace, 1)
    }

    pub(crate) fn new_for_document(
        workspace: Arc<tokio::sync::Mutex<crate::server::workspace::WorkspaceState>>,
        runtime_document_id: crate::protocol::DocumentId,
    ) -> Self {
        Self {
            runtime_records: Mutex::new(Vec::new()),
            published_sdui_tree: Mutex::new(None),
            published_decoration_set: Mutex::new(None),
            decoration_cache: Mutex::new(SyntaxChunkCache::default()),
            parse_handlers: Mutex::new(Vec::new()),
            js_parse_handlers: Mutex::new(Vec::new()),
            last_parse_update_json: Mutex::new(None),
            behavior: Mutex::new(ActiveBehaviorManifest::default()),
            modes: Mutex::new(crate::packages::modes::ModeRegistry::new()),
            commands: Mutex::new(crate::packages::commands::CommandRegistry::new()),
            ui: Mutex::new(crate::server::ui::PackageUiRegistry::new()),
            runtime_context: Mutex::new(ClayRuntimeContext {
                workspace,
                runtime_document_id,
            }),
            first_party_packages: Mutex::new(crate::packages::service::PackageService::new(
                PathBuf::new(),
                Box::new(crate::packages::manager::FakeBackend::new()),
            )),
            load_entry_allowlist: Arc::new(FirstPartyLoadEntryAllowlist::default()),
        }
    }

    pub(crate) fn workspace(
        &self,
    ) -> Arc<tokio::sync::Mutex<crate::server::workspace::WorkspaceState>> {
        Arc::clone(
            &self
                .runtime_context
                .lock()
                .expect("Clay runtime op state mutex poisoned")
                .workspace,
        )
    }

    pub(super) fn runtime_document_id(&self) -> crate::protocol::DocumentId {
        self.runtime_context
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .runtime_document_id
    }

    pub(crate) fn set_runtime_context(
        &self,
        workspace: Arc<tokio::sync::Mutex<crate::server::workspace::WorkspaceState>>,
        runtime_document_id: crate::protocol::DocumentId,
    ) {
        *self
            .runtime_context
            .lock()
            .expect("Clay runtime op state mutex poisoned") = ClayRuntimeContext {
            workspace,
            runtime_document_id,
        };
    }

    pub(crate) fn begin_evaluation(&self) {
        self.runtime_records
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clear();
        *self
            .last_parse_update_json
            .lock()
            .expect("Clay runtime op state mutex poisoned") = None;
        *self
            .published_sdui_tree
            .lock()
            .expect("Clay runtime op state mutex poisoned") = None;
        *self
            .published_decoration_set
            .lock()
            .expect("Clay runtime op state mutex poisoned") = None;
    }

    /// Handle to the shared `PackageService` used by the first-party resolver
    /// op for validation/enable/conflict checks.
    pub(super) fn first_party_packages(&self) -> &Mutex<crate::packages::service::PackageService> {
        &self.first_party_packages
    }

    /// Handle to the validated first-party `loadEntry` allowlist shared with
    /// `ClayModuleLoader`.
    pub(crate) fn load_entry_allowlist(&self) -> Arc<FirstPartyLoadEntryAllowlist> {
        Arc::clone(&self.load_entry_allowlist)
    }

    pub(crate) fn records(&self) -> Vec<String> {
        self.runtime_records
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clone()
    }

    pub(crate) fn published_sdui_tree(&self) -> Option<crate::protocol::SduiTree> {
        self.published_sdui_tree
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clone()
    }

    pub(crate) fn published_decoration_set(&self) -> Option<DecorationSet> {
        self.published_decoration_set
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clone()
    }

    pub(crate) fn parse_handlers(&self) -> Vec<crate::server::parse_coordinator::ParseHandlerMeta> {
        self.parse_handlers
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clone()
    }

    pub(crate) fn js_parse_handlers(
        &self,
    ) -> Vec<crate::server::parse_coordinator::JsParseHandlerRegistration> {
        self.js_parse_handlers
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clone()
    }

    pub(crate) fn take_parse_update_json(&self) -> Option<String> {
        self.last_parse_update_json
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .take()
    }

    pub(crate) fn behavior_manifest(&self) -> BehaviorManifest {
        self.behavior
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .manifest()
            .clone()
    }

    pub(super) fn bind_key(
        &self,
        rule: KeyBindingRule,
    ) -> Result<BehaviorManifest, crate::behavior::manifest::ManifestValidationError> {
        let mut behavior = self
            .behavior
            .lock()
            .expect("Clay runtime op state mutex poisoned");
        let mut replacement = behavior.manifest().clone();
        if !replacement
            .commands
            .iter()
            .any(|command| command.command_id == rule.command_id)
        {
            replacement.commands.push(command_for_rule(&rule));
        }
        replacement.keymaps.retain(|existing| {
            existing.context != rule.context || existing.sequence != rule.sequence
        });
        replacement.keymaps.push(rule);
        replacement.manifest_id = "clay.runtime.configuration".to_string();
        behavior.publish_replacement(replacement)
    }

    pub(super) fn unbind_key(
        &self,
        stroke: &KeyStroke,
        context: &KeyBindingContext,
    ) -> Result<BehaviorManifest, crate::behavior::manifest::ManifestValidationError> {
        let mut behavior = self
            .behavior
            .lock()
            .expect("Clay runtime op state mutex poisoned");
        let mut replacement = behavior.manifest().clone();
        replacement.keymaps.retain(|existing| {
            existing.context != *context || existing.sequence != vec![stroke.clone()]
        });
        replacement.manifest_id = "clay.runtime.configuration".to_string();
        behavior.publish_replacement(replacement)
    }

    pub(super) fn publish_decoration_set(&self, set: DecorationSet) {
        if let Some(package_prefix) = set.package_prefix() {
            let _ = self
                .decoration_cache
                .lock()
                .expect("Clay runtime op state mutex poisoned")
                .insert_validated_set(package_prefix, set.clone());
        }
        *self
            .published_decoration_set
            .lock()
            .expect("Clay runtime op state mutex poisoned") = Some(set);
    }

    pub(super) fn register_parse_handler_meta(
        &self,
        meta: crate::server::parse_coordinator::ParseHandlerMeta,
    ) {
        self.parse_handlers
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .push(meta);
    }

    pub(super) fn register_js_parse_handler(
        &self,
        registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
    ) {
        self.register_parse_handler_meta(registration.meta.clone());
        self.js_parse_handlers
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .push(registration);
    }

    pub(super) fn store_parse_update_json(&self, json: String) {
        *self
            .last_parse_update_json
            .lock()
            .expect("Clay runtime op state mutex poisoned") = Some(json);
    }

    pub(super) fn publish_sdui_tree(&self, tree: crate::protocol::SduiTree) {
        *self
            .published_sdui_tree
            .lock()
            .expect("Clay runtime op state mutex poisoned") = Some(tree);
    }

    pub(super) fn register_mode(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: crate::packages::modes::ModeDeclaration,
    ) -> Result<(), crate::packages::modes::ModeDiagnostic> {
        self.modes
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_mode(package, declaration)
    }

    pub(super) fn classify_document(
        &self,
        input: &crate::packages::modes::DocumentClassificationInput,
    ) -> Result<crate::packages::modes::ModeClassification, crate::packages::modes::ModeDiagnostic>
    {
        self.modes
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .classify(input)
    }

    pub(super) fn activate_major_mode(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        input: &crate::packages::modes::DocumentClassificationInput,
    ) -> Result<crate::packages::modes::MajorModeActivation, crate::packages::modes::ModeDiagnostic>
    {
        let mut modes = self
            .modes
            .lock()
            .expect("Clay runtime op state mutex poisoned");
        let classification = modes.classify(input)?;
        modes.activate_major_mode(package, classification)
    }

    /// Publish a new behavior manifest shaped by package-supplied editor rules.
    ///
    /// Called by `op_clay_modes_activate_major_mode` after the package JS
    /// passes `editorRules` in the activation payload.  The method is fully
    /// mode-agnostic: it builds a manifest from the default commands plus any
    /// extra commands and keymaps the package passed, installs the
    /// `EditorBehaviorRules` the package chose, validates, and publishes.
    pub(super) fn publish_mode_behavior_manifest(
        &self,
        behavior_version: crate::protocol::BehaviorVersion,
        scope: BehaviorScope,
        manifest_id: String,
        editor_rules: EditorBehaviorRules,
        extra_commands: Vec<CommandDeclaration>,
        extra_keymaps: Vec<KeyBindingRule>,
    ) -> Result<BehaviorManifest, crate::behavior::manifest::ManifestValidationError> {
        let mut manifest = BehaviorManifest::minimal_text_editing(behavior_version);
        manifest.manifest_id = manifest_id;
        manifest.scope = scope;
        manifest.editor_rules = editor_rules;
        manifest.commands.extend(extra_commands);
        manifest.keymaps.extend(extra_keymaps);
        self.behavior
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .publish_replacement(manifest)
    }

    pub(super) fn register_command(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: crate::packages::commands::PackageCommandDeclaration,
    ) -> Result<
        crate::packages::commands::RegisteredCommand,
        crate::packages::commands::CommandDiagnostic,
    > {
        self.commands
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_command(package, declaration)
    }

    pub(super) fn list_package_commands(&self) -> Vec<serde_json::Value> {
        self.commands
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .list()
            .map(|command| {
                serde_json::json!({
                    "packageName": command.package_name,
                    "packageVersion": command.package_version,
                    "apiPrefix": command.api_prefix,
                    "commandId": command.command_id,
                    "displayName": command.display_name,
                })
            })
            .collect()
    }

    pub(super) fn registered_command_ids(&self) -> Vec<String> {
        self.commands
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .list()
            .map(|command| command.command_id.clone())
            .collect()
    }

    pub(crate) fn ui_contributions(&self) -> crate::server::ui::PackageUiRegistrySnapshot {
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .snapshot()
    }

    pub(super) fn register_panel_contribution(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: &serde_json::Value,
    ) -> Result<
        crate::server::ui::RegisteredPanelContribution,
        crate::server::ui::UiContributionDiagnostic,
    > {
        let command_ids = self.registered_command_ids();
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_panel(package, declaration, &command_ids)
    }

    pub(super) fn register_component_contribution(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: &serde_json::Value,
    ) -> Result<
        crate::server::ui::RegisteredComponentContribution,
        crate::server::ui::UiContributionDiagnostic,
    > {
        let command_ids = self.registered_command_ids();
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_component(package, declaration, &command_ids)
    }

    pub(super) fn register_transient_overlay_contribution(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: &serde_json::Value,
    ) -> Result<
        crate::server::ui::RegisteredTransientOverlayContribution,
        crate::server::ui::UiContributionDiagnostic,
    > {
        let command_ids = self.registered_command_ids();
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_overlay(package, declaration, &command_ids)
    }

    pub(super) fn register_input_contribution(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: &serde_json::Value,
    ) -> Result<
        crate::server::ui::RegisteredPackageInputContribution,
        crate::server::ui::UiContributionDiagnostic,
    > {
        let command_ids = self.registered_command_ids();
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_input(package, declaration, &command_ids)
    }

    pub(super) fn register_ui_state_scope(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: &serde_json::Value,
    ) -> Result<
        crate::server::ui::RegisteredPackageUiStateScope,
        crate::server::ui::UiContributionDiagnostic,
    > {
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_ui_state_scope(package, declaration)
    }

    pub(super) fn set_layout_override(
        &self,
        declaration: &serde_json::Value,
    ) -> Result<
        crate::server::ui::RegisteredPackageLayoutOverride,
        crate::server::ui::UiContributionDiagnostic,
    > {
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .set_layout_override(declaration)
    }

    pub(super) fn register_theme_token(
        &self,
        package: &crate::packages::manifest::ClayPackageManifest,
        declaration: &serde_json::Value,
    ) -> Result<
        crate::server::ui::RegisteredPackageThemeTokenDeclaration,
        crate::server::ui::UiContributionDiagnostic,
    > {
        self.ui
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_theme_token(package, declaration)
    }

    fn record(&self, value: String) {
        self.runtime_records
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .push(value);
    }
}

#[op2]
#[string]
fn op_clay_runtime_ping() -> Result<String, JsErrorBox> {
    Ok("clay-runtime-ready".to_string())
}

#[op2(fast)]
fn op_clay_runtime_record(state: &mut OpState, #[string] value: String) -> Result<(), JsErrorBox> {
    if value.trim().is_empty() {
        return Err(JsErrorBox::generic(
            "clay.runtime.invalid_record: value must not be empty",
        ));
    }

    state.borrow::<Arc<ClayOpState>>().record(value);
    Ok(())
}

extension!(
    clay_runtime_extension,
    ops = [
        op_clay_runtime_ping,
        op_clay_runtime_record,
        op_clay_configuration_load_module,
        op_clay_configuration_get_state,
        op_clay_configuration_set_package_option,
        op_clay_sdui_define_node,
        op_clay_sdui_publish_tree,
        op_clay_ui_register_panel_contribution,
        op_clay_ui_register_component_contribution,
        op_clay_ui_register_transient_overlay_contribution,
        op_clay_ui_register_theme_token,
        op_clay_ui_register_input_contribution,
        op_clay_ui_register_ui_state_scope,
        op_clay_ui_set_layout_override,
        op_clay_documents_open_document,
        op_clay_documents_save_document,
        op_clay_documents_reload_document,
        op_clay_documents_get_document_status,
        op_clay_documents_list_documents,
        op_clay_workspace_list_roots,
        op_clay_keybindings_bind_key,
        op_clay_keybindings_unbind_key,
        op_clay_keybindings_list_key_bindings,
        op_clay_behavior_get_active_manifest,
        op_clay_behavior_list_routes,
        op_clay_packages_validate_manifest,
        op_clay_packages_validate_permissions,
        op_clay_packages_load_package,
        op_clay_packages_load_package_by_specifier,
        op_clay_packages_list_first_party_specifiers,
        op_clay_modes_register_pattern,
        op_clay_modes_classify_document,
        op_clay_modes_activate_major_mode,
        op_clay_commands_register_command,
        op_clay_commands_list_commands,
        op_clay_decorations_publish_decorations,
        op_clay_parse_register_parse_handler,
        op_clay_parse_store_update,
        op_clay_runtime_unavailable,
    ],
);

fn command_for_rule(rule: &KeyBindingRule) -> CommandDeclaration {
    match rule.routing_policy {
        crate::protocol::RoutingPolicy::ClientFirstPredictable
        | crate::protocol::RoutingPolicy::ClientFirstRequiresAck => {
            CommandDeclaration::client_edit(rule.command_id.clone(), display_name(&rule.command_id))
        }
        crate::protocol::RoutingPolicy::UiReactivePriority => {
            CommandDeclaration::ui_reactive(rule.command_id.clone(), display_name(&rule.command_id))
        }
        crate::protocol::RoutingPolicy::ClientUiCommand => {
            CommandDeclaration::client_ui(rule.command_id.clone(), display_name(&rule.command_id))
        }
        _ => CommandDeclaration::server_intent(
            rule.command_id.clone(),
            display_name(&rule.command_id),
        ),
    }
}

fn display_name(command_id: &str) -> String {
    command_id
        .split('.')
        .next_back()
        .unwrap_or(command_id)
        .replace('-', " ")
}

pub(crate) fn init_runtime_extension() -> deno_core::Extension {
    clay_runtime_extension::init()
}
