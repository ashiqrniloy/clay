mod behavior;
mod commands;
mod completion;
mod configuration;
mod decorations;
mod documents;
mod git;
mod keybindings;
mod modes;
mod packages;
mod parse;
mod planned;
mod sdui;
mod syntax;
mod theme;
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
    commands::{
        op_clay_commands_execute_command, op_clay_commands_list_commands,
        op_clay_commands_register_command,
    },
    completion::{
        op_clay_completion_providers_for_trigger, op_clay_completion_register_completion_provider,
    },
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
    git::{op_clay_git_list_statuses, op_clay_git_refresh_status},
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
    syntax::{op_clay_syntax_register_syntax_grammar, op_clay_syntax_set_engine_preference},
    theme::op_clay_theme_set_theme,
    ui::{
        op_clay_ui_register_component_contribution, op_clay_ui_register_input_contribution,
        op_clay_ui_register_panel_contribution, op_clay_ui_register_theme_token,
        op_clay_ui_register_transient_overlay_contribution, op_clay_ui_register_ui_state_scope,
        op_clay_ui_set_layout_override,
    },
    workspace::{
        op_clay_workspace_add_root, op_clay_workspace_cancel_listing,
        op_clay_workspace_create_listing_cancel_token, op_clay_workspace_discover_root_for_path,
        op_clay_workspace_list_directory, op_clay_workspace_list_roots,
    },
};

pub(crate) use self::packages::PackageLoadEntryAllowlist;

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
    git_status_cache: crate::server::git::GitStatusCache,
    syntax_grammars: Mutex<crate::server::syntax::SyntaxGrammarRegistry>,
    completion_providers: Mutex<Vec<crate::server::completion::CompletionProviderMeta>>,
    /// Resolved active theme snapshot set by the `setTheme` Clay JS op during
    /// `init.js`. Carried out in [`crate::server::js_runtime::ClayRuntimeEvaluation`]
    /// and applied to the shared server slot at load/reload so the welcome
    /// handshake ships it to the client. `None` = Clay default theme.
    active_theme: Mutex<Option<crate::protocol::ActiveTheme>>,
    runtime_context: Mutex<ClayRuntimeContext>,
    // Shared PackageService for loadPackage resolution. Bundled packages are
    // seeded from CARGO_MANIFEST_DIR/packages; user-installed packages are
    // resolved from this service's installed/source registry. The resolver uses
    // the same validate/authorize/enable path as CLI and package UI code.
    package_service: Mutex<crate::packages::service::PackageService>,
    load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
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
            git_status_cache: crate::server::git::GitStatusCache::default(),
            syntax_grammars: Mutex::new(
                crate::server::syntax::SyntaxGrammarRegistry::with_first_party_native(),
            ),
            completion_providers: Mutex::new(Vec::new()),
            active_theme: Mutex::new(None),
            runtime_context: Mutex::new(ClayRuntimeContext {
                workspace,
                runtime_document_id,
            }),
            package_service: Mutex::new(crate::packages::service::PackageService::new(
                PathBuf::new(),
                Box::new(crate::packages::manager::FakeBackend::new()),
            )),
            load_entry_allowlist: Arc::new(PackageLoadEntryAllowlist::default()),
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

    /// Handle to the shared `PackageService` used by the resolver op for
    /// validation/authorization/enable/conflict checks.
    pub(super) fn package_service(&self) -> &Mutex<crate::packages::service::PackageService> {
        &self.package_service
    }

    /// Handle to the validated package `loadEntry` allowlist shared with
    /// `ClayModuleLoader`.
    pub(crate) fn load_entry_allowlist(&self) -> Arc<PackageLoadEntryAllowlist> {
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

    pub(super) fn git_status_cache(&self) -> crate::server::git::GitStatusCache {
        self.git_status_cache.clone()
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

    pub(super) async fn execute_command(
        &self,
        request: crate::server::command_execution::CommandExecutionRequest,
    ) -> Result<
        crate::server::command_execution::CommandExecutionResult,
        crate::server::command_execution::CommandExecutionDiagnostic,
    > {
        let executor = crate::server::command_execution::CommandExecutor::new();
        // Phase 18.9 mode-discovery commands resolve their payload by reading
        // installed `ModeRegistry` state (read-only; no filesystem scan, package
        // evaluation, or other authority).
        if crate::server::command_execution::is_mode_discovery_command(&request.command_id) {
            return executor.execute_discovery(
                &self
                    .modes
                    .lock()
                    .expect("Clay runtime op state mutex poisoned"),
                request,
            );
        }
        // Phase 18.13 Git commands read server-owned workspace roots and Git
        // cache state. They expose branch/status metadata only: no shell,
        // network, or mutating Git authority.
        if crate::server::command_execution::is_git_command(&request.command_id) {
            let workspace = self.workspace();
            let workspace = workspace.lock().await;
            return executor
                .execute_git(&workspace, self.git_status_cache(), request)
                .await;
        }
        // Phase 18.12 file-browser commands open/reveal files through
        // server-authoritative workspace APIs and selected-file grants.
        if crate::server::command_execution::is_workspace_command(&request.command_id) {
            let registry = self
                .commands
                .lock()
                .expect("Clay runtime op state mutex poisoned")
                .clone();
            let workspace = self.workspace();
            let mut workspace = workspace.lock().await;
            return executor
                .execute_workspace(&registry, &mut workspace, request)
                .await;
        }
        // Package commands and other built-in commands go through the standard
        // validation-only execution path.
        executor.execute(
            &self
                .commands
                .lock()
                .expect("Clay runtime op state mutex poisoned"),
            request,
        )
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

    pub(crate) fn syntax_grammars(&self) -> Vec<crate::server::syntax::SyntaxGrammarContribution> {
        self.syntax_grammars
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .list()
            .cloned()
            .collect()
    }

    /// Record the active theme resolved by the `setTheme` Clay JS op.
    pub(super) fn set_active_theme(&self, theme: crate::protocol::ActiveTheme) {
        *self
            .active_theme
            .lock()
            .expect("Clay runtime op state mutex poisoned") = Some(theme);
    }

    /// Take the active theme snapshot out of this evaluation (cloned; the worker
    /// next evaluation resets it before reuse).
    pub(crate) fn active_theme(&self) -> Option<crate::protocol::ActiveTheme> {
        self.active_theme
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clone()
    }

    pub(crate) fn completion_providers(
        &self,
    ) -> Vec<crate::server::completion::CompletionProviderMeta> {
        self.completion_providers
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .clone()
    }

    pub(crate) fn completion_providers_for_trigger(
        &self,
        trigger: &str,
    ) -> Vec<crate::server::completion::CompletionProviderMeta> {
        let providers = self
            .completion_providers
            .lock()
            .expect("Clay runtime op state mutex poisoned");
        let mut matched: Vec<_> = providers
            .iter()
            .filter(|meta| {
                meta.trigger_metadata
                    .trigger_characters
                    .iter()
                    .any(|character| character == trigger)
            })
            .cloned()
            .collect();
        matched.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        matched
    }

    pub(super) fn register_completion_provider_metadata(
        &self,
        metas: Vec<crate::server::completion::CompletionProviderMeta>,
    ) -> Result<Vec<crate::server::completion::CompletionProviderMeta>, String> {
        let mut providers = self
            .completion_providers
            .lock()
            .expect("Clay runtime op state mutex poisoned");
        for meta in &metas {
            if providers.iter().any(|existing| existing.id == meta.id) {
                return Err(format!("provider `{}` is already registered", meta.id));
            }
        }
        providers.extend(metas.clone());
        Ok(metas)
    }

    pub(super) fn register_syntax_grammar_package(
        &self,
        package: &crate::packages::record::PackageRecord,
    ) -> Result<usize, crate::server::syntax::SyntaxGrammarRegistryError> {
        self.syntax_grammars
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .register_package(package)
    }

    pub(super) fn set_syntax_engine_preference(
        &self,
        target: &str,
        tier: crate::server::syntax::SyntaxEngineTier,
    ) -> Result<(), crate::server::syntax::SyntaxGrammarRegistryError> {
        self.syntax_grammars
            .lock()
            .expect("Clay runtime op state mutex poisoned")
            .set_engine_preference(target, tier)
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
        op_clay_theme_set_theme,
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
        op_clay_workspace_add_root,
        op_clay_workspace_discover_root_for_path,
        op_clay_workspace_list_directory,
        op_clay_workspace_create_listing_cancel_token,
        op_clay_workspace_cancel_listing,
        op_clay_git_list_statuses,
        op_clay_git_refresh_status,
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
        op_clay_commands_execute_command,
        op_clay_decorations_publish_decorations,
        op_clay_parse_register_parse_handler,
        op_clay_parse_store_update,
        op_clay_syntax_register_syntax_grammar,
        op_clay_syntax_set_engine_preference,
        op_clay_completion_register_completion_provider,
        op_clay_completion_providers_for_trigger,
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
