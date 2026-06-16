use std::{
    error::Error,
    fmt,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use deno_core::{
    JsRuntime, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind, RuntimeOptions,
    error::ModuleLoaderError,
};
use deno_error::JsErrorBox;
use tokio::task;

use crate::perf::metrics::global_recorder;
use crate::protocol::RuntimeDiagnostic;

use super::{
    configuration::{ConfigurationError, ConfigurationRuntime},
    ops::{ClayOpState, FirstPartyLoadEntryAllowlist, init_runtime_extension},
    workspace::WorkspaceState,
};

const CONTROLLED_MAIN_SPECIFIER: &str = "clay://runtime/main.js";
const MARKDOWN_IT_MODULE_SPECIFIER: &str = "clay://vendor/markdown-it.js";

fn clay_facade_source(specifier: &str) -> Option<&'static str> {
    match specifier {
        "clay:configuration" => Some(CLAY_FACADE_CONFIGURATION),
        "clay:sdui" => Some(CLAY_FACADE_SDUI),
        "clay:ui" => Some(CLAY_FACADE_UI),
        "clay:documents" => Some(CLAY_FACADE_DOCUMENTS),
        "clay:workspace" => Some(CLAY_FACADE_WORKSPACE),
        "clay:keybindings" => Some(CLAY_FACADE_KEYBINDINGS),
        "clay:behavior" => Some(CLAY_FACADE_BEHAVIOR),
        "clay:packages" => Some(CLAY_FACADE_PACKAGES),
        "clay:modes" => Some(CLAY_FACADE_MODES),
        "clay:commands" => Some(CLAY_FACADE_COMMANDS),
        "clay:decorations" => Some(CLAY_FACADE_DECORATIONS),
        "clay:parse" => Some(CLAY_FACADE_PARSE),
        "clay:application" => Some(CLAY_FACADE_APPLICATION),
        "clay:editor" => Some(CLAY_FACADE_EDITOR),
        _ => None,
    }
}

const CLAY_FACADE_CONFIGURATION: &str = r#"
const ops = Deno.core.ops;
const unavailable = (api) => { ops.op_clay_runtime_unavailable(api); };

export async function loadConfigurationModule(options) {
  if (options === null || typeof options !== "object" || typeof options.path !== "string") {
    throw new Error("clay.configuration.invalid_module: loadConfigurationModule requires { path: string }");
  }
  const path = ops.op_clay_configuration_load_module(options.path);
  await import(path);
}

export function getConfigurationState() {
  return JSON.parse(ops.op_clay_configuration_get_state());
}

export function setPackageOption(options) {
  return JSON.parse(ops.op_clay_configuration_set_package_option(JSON.stringify(options ?? null)));
}
export function setModePreference(options) { void options; unavailable("clay.configuration.setModePreference"); }
export function setDecorationTheme(options) { void options; unavailable("clay.configuration.setDecorationTheme"); }
export function setParsePolicy(options) { void options; unavailable("clay.configuration.setParsePolicy"); }
"#;

const CLAY_FACADE_SDUI: &str = r#"
const ops = Deno.core.ops;

function defineNode(kind, options) {
  return JSON.parse(ops.op_clay_sdui_define_node(kind, JSON.stringify(options ?? {})));
}

export function definePanel(options) { return defineNode("panel", options); }
export function defineLabel(options) { return defineNode("label", options); }
export function defineButton(options) { return defineNode("button", options); }
export function defineList(options) { return defineNode("list", options); }
export function defineEditorView(options) { return defineNode("editorView", options); }
export function defineFlex(options) { return defineNode("flex", options); }
export function defineStack(options) { return defineNode("stack", options); }
export async function publishTree(tree) {
  ops.op_clay_sdui_publish_tree(JSON.stringify(tree ?? null));
}
"#;

const CLAY_FACADE_UI: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
const encode = (value) => JSON.stringify(value ?? null);
export function serverRegisterPanelContribution(packageManifest, declaration) {
  return parse(ops.op_clay_ui_register_panel_contribution(encode(packageManifest), encode(declaration)));
}
export function serverRegisterComponentContribution(packageManifest, declaration) {
  return parse(ops.op_clay_ui_register_component_contribution(encode(packageManifest), encode(declaration)));
}
export function serverRegisterTransientOverlayContribution(packageManifest, declaration) {
  return parse(ops.op_clay_ui_register_transient_overlay_contribution(encode(packageManifest), encode(declaration)));
}
export function serverRegisterInputContribution(packageManifest, declaration) {
  return parse(ops.op_clay_ui_register_input_contribution(encode(packageManifest), encode(declaration)));
}
export function serverRegisterUiStateScope(packageManifest, declaration) {
  return parse(ops.op_clay_ui_register_ui_state_scope(encode(packageManifest), encode(declaration)));
}
export function serverSetLayoutOverride(declaration) {
  return parse(ops.op_clay_ui_set_layout_override(encode(declaration)));
}
export function serverRegisterThemeToken(packageManifest, declaration) {
  return parse(ops.op_clay_ui_register_theme_token(encode(packageManifest), encode(declaration)));
}
"#;

const CLAY_FACADE_DOCUMENTS: &str = r#"
const ops = Deno.core.ops;
const unavailable = (api) => { ops.op_clay_runtime_unavailable(api); };
const parse = (json) => JSON.parse(json);
export async function serverGetDocumentSnapshot(documentId) { void documentId; unavailable("clay.documents.serverGetDocumentSnapshot"); }
export async function serverGetDocumentLease(documentId) { void documentId; unavailable("clay.documents.serverGetDocumentLease"); }
export async function serverOpenDocument(options) { return parse(await ops.op_clay_documents_open_document(JSON.stringify(options ?? null))); }
export async function serverSaveDocument(options) { return parse(await ops.op_clay_documents_save_document(JSON.stringify(options ?? null))); }
export async function serverReloadDocument(options) { return parse(await ops.op_clay_documents_reload_document(JSON.stringify(options ?? null))); }
export async function serverGetDocumentStatus(documentId) { return parse(await ops.op_clay_documents_get_document_status(JSON.stringify(documentId))); }
export async function serverListDocuments() { return parse(await ops.op_clay_documents_list_documents()); }
"#;

const CLAY_FACADE_WORKSPACE: &str = r#"
export async function serverListWorkspaceRoots() { return JSON.parse(await Deno.core.ops.op_clay_workspace_list_roots()); }
"#;

const CLAY_FACADE_KEYBINDINGS: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function bindKey(key, command, options = {}) {
  if (typeof key !== "string" || typeof command !== "string") {
    throw new Error("clay.keybindings.invalid_bind: bindKey requires (key: string, command: string)");
  }
  return parse(ops.op_clay_keybindings_bind_key(key, command, JSON.stringify(options ?? {})));
}
export function unbindKey(key, options = {}) {
  if (typeof key !== "string") {
    throw new Error("clay.keybindings.invalid_unbind: unbindKey requires (key: string)");
  }
  ops.op_clay_keybindings_unbind_key(key, JSON.stringify(options ?? {}));
}
export function listKeyBindings(scope = "all") {
  return parse(ops.op_clay_keybindings_list_key_bindings(scope ?? "all"));
}
"#;

const CLAY_FACADE_BEHAVIOR: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function getActiveBehaviorManifest(documentId) {
  return parse(ops.op_clay_behavior_get_active_manifest(JSON.stringify(documentId ?? null)));
}
export function listBehaviorRoutes(documentId) {
  return parse(ops.op_clay_behavior_list_routes(JSON.stringify(documentId ?? null)));
}
"#;

const CLAY_FACADE_PACKAGES: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function serverValidatePackageManifest(manifest) {
  return parse(ops.op_clay_packages_validate_manifest(JSON.stringify(manifest ?? null)));
}
export function serverValidatePackagePermissions(permissions) {
  return parse(ops.op_clay_packages_validate_permissions(JSON.stringify(permissions ?? null)));
}
export function serverLoadPackage(packageJson) {
  return parse(ops.op_clay_packages_load_package(JSON.stringify(packageJson ?? null)));
}
export async function loadPackage(specifier) {
  if (typeof specifier !== "string") {
    throw new Error("clay.packages.invalid_specifier: loadPackage requires a string specifier");
  }
  const result = parse(ops.op_clay_packages_load_package_by_specifier(JSON.stringify({ specifier })));
  const loadEntry = await import(result.loadEntrySpecifier);
  if (typeof loadEntry.default === "function") {
    await loadEntry.default();
  }
  return result;
}
"#;

const CLAY_FACADE_MODES: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function serverRegisterModePattern(packageManifest, declaration) {
  return parse(ops.op_clay_modes_register_pattern(JSON.stringify(packageManifest ?? null), JSON.stringify(declaration ?? null)));
}
export function serverClassifyDocument(input) {
  return parse(ops.op_clay_modes_classify_document(JSON.stringify(input ?? null)));
}
export function serverActivateMajorMode(packageManifest, input) {
  return parse(ops.op_clay_modes_activate_major_mode(JSON.stringify(packageManifest ?? null), JSON.stringify(input ?? null)));
}
export function serverSelectDocumentManifest(options) { void options; ops.op_clay_runtime_unavailable("clay.modes.serverSelectDocumentManifest"); }
export function serverRegisterDecorationProvider(options) { void options; ops.op_clay_runtime_unavailable("clay.modes.serverRegisterDecorationProvider"); }
export function serverRegisterParseProvider(options) { void options; ops.op_clay_runtime_unavailable("clay.modes.serverRegisterParseProvider"); }
export function serverRegisterFoldingProvider(options) { void options; ops.op_clay_runtime_unavailable("clay.modes.serverRegisterFoldingProvider"); }
"#;

const CLAY_FACADE_COMMANDS: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function serverRegisterCommand(packageManifest, declaration) {
  return parse(ops.op_clay_commands_register_command(JSON.stringify(packageManifest ?? null), JSON.stringify(declaration ?? null)));
}
export function serverListCommands() {
  return parse(ops.op_clay_commands_list_commands());
}
"#;

const CLAY_FACADE_DECORATIONS: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function serverPublishDecorations(options) {
  return parse(ops.op_clay_decorations_publish_decorations(JSON.stringify(options ?? null)));
}
"#;

const CLAY_FACADE_PARSE: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function serverRegisterParseHandler(options) {
  return parse(ops.op_clay_parse_register_parse_handler(JSON.stringify(options ?? null)));
}
"#;

const CLAY_FACADE_APPLICATION: &str = r#"
export function quit() { Deno.core.ops.op_clay_runtime_unavailable("clay.application.quit"); }
"#;

const CLAY_FACADE_EDITOR: &str = r#"
const unavailable = (api) => { Deno.core.ops.op_clay_runtime_unavailable(api); };
export async function serverInsertText(options) { void options; unavailable("clay.editor.serverInsertText"); }
export async function serverDeleteRange(options) { void options; unavailable("clay.editor.serverDeleteRange"); }
export async function serverInsertNewline(options) { void options; unavailable("clay.editor.serverInsertNewline"); }
export function clientMoveCursor(options) { void options; unavailable("clay.editor.clientMoveCursor"); }
export function clientSetSelection(options) { void options; unavailable("clay.editor.clientSetSelection"); }
export function clientScrollTo(options) { void options; unavailable("clay.editor.clientScrollTo"); }
export function clientSetCursorStyle(options) { void options; unavailable("clay.editor.clientSetCursorStyle"); }
export function clientSetViewport(options) { void options; unavailable("clay.editor.clientSetViewport"); }
"#;

/// Isolated server-side Clay JavaScript runtime boundary.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClayJsRuntimeService {
    evaluations: Arc<AtomicU64>,
}

impl ClayJsRuntimeService {
    /// Evaluates a controlled server-owned ES module on a blocking runtime worker.
    pub(crate) async fn evaluate_controlled_module(
        &self,
        source: impl Into<String> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let source = source.into();
        let evaluations = Arc::clone(&self.evaluations);
        task::spawn_blocking(move || {
            let recorder = global_recorder();
            let _scope = recorder.scope("runtime.evaluate_controlled_module");
            evaluate_module_on_runtime(RuntimeEntry::ControlledSource(source), None, 1)
        })
        .await
        .map_err(ClayRuntimeError::Join)?
        .inspect(|_| {
            evaluations.fetch_add(1, Ordering::Relaxed);
        })
    }

    pub(crate) async fn load_configuration_from_root(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let config_root = config_root.into();
        let evaluations = Arc::clone(&self.evaluations);
        task::spawn_blocking(move || {
            let recorder = global_recorder();
            let _scope = recorder.scope("runtime.load_configuration");
            evaluate_module_on_runtime(RuntimeEntry::ConfigurationRoot(config_root), None, 1)
        })
        .await
        .map_err(ClayRuntimeError::Join)?
        .inspect(|_| {
            evaluations.fetch_add(1, Ordering::Relaxed);
        })
    }

    pub(crate) async fn load_configuration_from_root_with_workspace(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let config_root = config_root.into();
        let evaluations = Arc::clone(&self.evaluations);
        task::spawn_blocking(move || {
            let recorder = global_recorder();
            let _scope = recorder.scope("runtime.load_configuration_with_workspace");
            evaluate_module_on_runtime(
                RuntimeEntry::ConfigurationRoot(config_root),
                Some(workspace),
                1,
            )
        })
        .await
        .map_err(ClayRuntimeError::Join)?
        .inspect(|_| {
            evaluations.fetch_add(1, Ordering::Relaxed);
        })
    }

    pub(crate) async fn load_configuration_from_root_for_document(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        runtime_document_id: crate::protocol::DocumentId,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        let config_root = config_root.into();
        let evaluations = Arc::clone(&self.evaluations);
        task::spawn_blocking(move || {
            let recorder = global_recorder();
            let _scope = recorder.scope("runtime.load_configuration_for_document");
            evaluate_module_on_runtime(
                RuntimeEntry::ConfigurationRoot(config_root),
                None,
                runtime_document_id,
            )
        })
        .await
        .map_err(ClayRuntimeError::Join)?
        .inspect(|_| {
            evaluations.fetch_add(1, Ordering::Relaxed);
        })
    }

    pub(crate) async fn load_default_configuration(
        &self,
    ) -> Result<Option<ClayRuntimeEvaluation>, ClayRuntimeError> {
        let Some(config_root) = ConfigurationRuntime::default_config_root() else {
            return Ok(None);
        };
        if !config_root.join("init.js").is_file() {
            return Ok(None);
        }
        self.load_configuration_from_root(config_root)
            .await
            .map(Some)
    }

    pub(crate) async fn load_default_configuration_with_workspace(
        &self,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    ) -> Result<Option<ClayRuntimeEvaluation>, ClayRuntimeError> {
        let Some(config_root) = ConfigurationRuntime::default_config_root() else {
            return Ok(None);
        };
        if !config_root.join("init.js").is_file() {
            return Ok(None);
        }
        self.load_configuration_from_root_with_workspace(config_root, workspace)
            .await
            .map(Some)
    }

    #[cfg(test)]
    pub(crate) fn evaluation_count(&self) -> u64 {
        self.evaluations.load(Ordering::Relaxed)
    }
}

/// Result of one JavaScript evaluation returned across the Rust boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClayRuntimeEvaluation {
    pub(crate) op_records: Vec<String>,
    pub(crate) published_sdui_tree: Option<crate::protocol::SduiTree>,
    pub(crate) published_decoration_set: Option<crate::protocol::DecorationSet>,
    pub(crate) parse_handlers: Vec<crate::server::parse_coordinator::ParseHandlerMeta>,
    pub(crate) behavior_manifest: Option<crate::protocol::BehaviorManifest>,
    pub(crate) ui_contributions: crate::server::ui::PackageUiRegistrySnapshot,
}

#[derive(Debug)]
pub(crate) enum ClayRuntimeError {
    Configuration(ConfigurationError),
    InvalidMainSpecifier(String),
    Runtime(String),
    Join(task::JoinError),
}

impl fmt::Display for ClayRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(error) => write!(formatter, "configuration error: {error}"),
            Self::InvalidMainSpecifier(message) => {
                write!(formatter, "invalid main module: {message}")
            }
            Self::Runtime(message) => write!(formatter, "JavaScript runtime error: {message}"),
            Self::Join(error) => write!(formatter, "JavaScript runtime task failed: {error}"),
        }
    }
}

impl Error for ClayRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::Join(error) => Some(error),
            Self::InvalidMainSpecifier(_) | Self::Runtime(_) => None,
        }
    }
}

impl ClayRuntimeError {
    pub(crate) fn diagnostic(&self) -> RuntimeDiagnostic {
        match self {
            Self::Configuration(error) => RuntimeDiagnostic::error(
                "clay.configuration.invalid_module",
                configuration_diagnostic_message(&error.to_string()),
            ),
            Self::InvalidMainSpecifier(_) => RuntimeDiagnostic::error(
                "clay.runtime.invalid_main",
                "Runtime configuration entry point could not be parsed.",
            ),
            Self::Runtime(message) => runtime_error_diagnostic(message),
            Self::Join(_) => RuntimeDiagnostic::error(
                "clay.runtime.task_failed",
                "JavaScript runtime worker failed before configuration completed.",
            ),
        }
    }
}

fn runtime_error_diagnostic(message: &str) -> RuntimeDiagnostic {
    let code = extract_clay_error_code(message).unwrap_or_else(|| {
        if message.contains("SyntaxError") {
            "clay.runtime.syntax_error".to_string()
        } else {
            "clay.runtime.exception".to_string()
        }
    });
    let detail = match code.as_str() {
        "clay.runtime.invalid_import" => {
            "Only clay:* facades and relative local configuration modules are allowed."
        }
        "clay.configuration.invalid_module" => {
            "Configuration modules must be explicit relative .js files under the configuration root."
        }
        "clay.runtime.syntax_error" => {
            "JavaScript syntax error while evaluating server-side configuration."
        }
        "clay.runtime.invalid_record" => "Runtime op validation rejected an empty record.",
        "clay.sdui.invalid_tree" => "Published SDUI tree failed server validation.",
        "clay.sdui.invalid_action" => {
            "Published SDUI action contains unsupported command authority."
        }
        "clay.keybindings.unknown_command" => {
            "Key binding references an unknown or unsupported command."
        }
        code if code.starts_with("clay.ui.") => {
            "Package UI contribution registration failed server validation."
        }
        code if code.starts_with("clay.documents.") => {
            "Document/workspace operation failed server validation."
        }
        code if code.starts_with("clay.workspace.") => {
            "Workspace operation failed server validation."
        }
        _ => "JavaScript runtime evaluation failed.",
    };

    RuntimeDiagnostic::error(code, detail)
}

fn configuration_diagnostic_message(message: &str) -> &'static str {
    if message.contains("init.js") {
        "Configuration entry point init.js could not be loaded."
    } else {
        "Configuration module could not be loaded."
    }
}

fn extract_clay_error_code(message: &str) -> Option<String> {
    message
        .split(|character: char| character.is_whitespace() || character == ':' || character == '`')
        .find(|part| part.starts_with("clay.") && part.chars().all(is_error_code_character))
        .map(ToOwned::to_owned)
}

fn is_error_code_character(character: char) -> bool {
    character.is_ascii_lowercase()
        || character.is_ascii_digit()
        || character == '.'
        || character == '_'
}

enum RuntimeEntry {
    ControlledSource(String),
    ConfigurationRoot(PathBuf),
}

fn evaluate_module_on_runtime(
    entry: RuntimeEntry,
    workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
    runtime_document_id: crate::protocol::DocumentId,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    let op_state = Arc::new(ClayOpState::new_for_document(
        workspace.unwrap_or_else(|| Arc::new(tokio::sync::Mutex::new(WorkspaceState::new()))),
        runtime_document_id,
    ));
    let loaded_configuration = match entry {
        RuntimeEntry::ControlledSource(source) => LoadedRuntimeEntry {
            main_specifier: ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER)
                .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
            main_source: Some(source),
            configuration: None,
        },
        RuntimeEntry::ConfigurationRoot(config_root) => {
            let configuration = Arc::new(
                ConfigurationRuntime::from_config_root(config_root)
                    .map_err(ClayRuntimeError::Configuration)?,
            );
            LoadedRuntimeEntry {
                main_specifier: configuration
                    .entry_specifier()
                    .map_err(ClayRuntimeError::Configuration)?,
                main_source: None,
                configuration: Some(configuration),
            }
        }
    };

    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(ClayModuleLoader::new(
            loaded_configuration.main_specifier.clone(),
            loaded_configuration.main_source.clone(),
            loaded_configuration.configuration.clone(),
            op_state.load_entry_allowlist(),
        ))),
        extensions: vec![init_runtime_extension()],
        ..Default::default()
    });

    runtime.op_state().borrow_mut().put(Arc::clone(&op_state));
    if let Some(configuration) = &loaded_configuration.configuration {
        runtime
            .op_state()
            .borrow_mut()
            .put(Arc::clone(configuration));
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?
        .block_on(async move {
            let module_id = if let Some(source) = loaded_configuration.main_source {
                runtime
                    .load_main_es_module_from_code(&loaded_configuration.main_specifier, source)
                    .await
            } else {
                runtime
                    .load_main_es_module(&loaded_configuration.main_specifier)
                    .await
            }
            .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
            let result = runtime.mod_evaluate(module_id);
            runtime
                .run_event_loop(Default::default())
                .await
                .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
            result
                .await
                .map_err(|error| ClayRuntimeError::Runtime(error.to_string()))?;
            let behavior_manifest = op_state.behavior_manifest();
            Ok(ClayRuntimeEvaluation {
                op_records: op_state.records(),
                published_sdui_tree: op_state.published_sdui_tree(),
                published_decoration_set: op_state.published_decoration_set(),
                parse_handlers: op_state.parse_handlers(),
                behavior_manifest: (behavior_manifest.behavior_version > 1)
                    .then_some(behavior_manifest),
                ui_contributions: op_state.ui_contributions(),
            })
        })
}

struct LoadedRuntimeEntry {
    main_specifier: ModuleSpecifier,
    main_source: Option<String>,
    configuration: Option<Arc<ConfigurationRuntime>>,
}

#[derive(Debug)]
struct ClayModuleLoader {
    main_specifier: ModuleSpecifier,
    main_source: Option<String>,
    configuration: Option<Arc<ConfigurationRuntime>>,
    // ponytail: shared validated first-party loadEntry gate. Populated by
    // `op_clay_packages_load_package_by_specifier`, checked in resolve/load.
    // The loader branches that consume it are added in Phase 18.6 task 4;
    // until then it is threaded but unused, so the deny-by-default boundary
    // is unchanged. Ceiling: one entry per loaded first-party package.
    first_party_load_entry_allowlist: Arc<FirstPartyLoadEntryAllowlist>,
}

impl ClayModuleLoader {
    fn new(
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
        first_party_load_entry_allowlist: Arc<FirstPartyLoadEntryAllowlist>,
    ) -> Self {
        Self {
            main_specifier,
            main_source,
            configuration,
            first_party_load_entry_allowlist,
        }
    }

    fn denied(specifier: &str) -> JsErrorBox {
        JsErrorBox::generic(format!(
            "clay.runtime.invalid_import: module specifier `{specifier}` is not allowed in the server runtime boundary"
        ))
    }
}

impl ModuleLoader for ClayModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        if matches!(kind, ResolutionKind::MainModule) && specifier == self.main_specifier.as_str() {
            return Ok(self.main_specifier.clone());
        }
        if clay_facade_source(specifier).is_some() {
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()).into());
        }
        if specifier == "markdown-it" {
            return ModuleSpecifier::parse(MARKDOWN_IT_MODULE_SPECIFIER)
                .map_err(|error| Self::denied(&error.to_string()).into());
        }
        // First-party validated `loadEntry`: opaque `clay://packages/...`
        // specifiers recorded by `op_clay_packages_load_package_by_specifier`.
        // This branch sits BEFORE the config-root branch because
        // `reject_non_local_specifier` would otherwise deny `clay://` URLs; the
        // shared allowlist is the single gate, so only a specifier the resolver
        // op validated and recorded ever resolves here. Every other
        // `clay://packages/...` URL falls through to config-root confinement
        // (which rejects non-local specifiers) and the deny fallback.
        // ponytail: authority ceiling is first-party `@clay/*` loadEntry only.
        // Upgrade path: non-`@clay/*` registry packages are deferred to Phase 23
        // ecosystem hardening and would widen the resolver, not this branch.
        if self
            .first_party_load_entry_allowlist
            .absolute_path(specifier)
            .is_some()
        {
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()).into());
        }
        // Transitive relative imports from a validated package loadEntry are
        // confined to the validated package root by the allowlist and recorded
        // on first resolution. This lets a loadEntry import its own sibling
        // modules (e.g. `./index.js`) without weakening the config-root
        // boundary for any non-package specifier. ponytail: ceiling is the
        // validated package root; `resolve_relative` denies anything escaping it.
        if specifier.starts_with("./") || specifier.starts_with("../") {
            if let Some(new_specifier) = self
                .first_party_load_entry_allowlist
                .resolve_relative(referrer, specifier)
            {
                return ModuleSpecifier::parse(&new_specifier)
                    .map_err(|error| Self::denied(&error.to_string()).into());
            }
        }
        if let Some(configuration) = &self.configuration {
            return configuration
                .resolve_module(specifier, referrer)
                .map_err(|error| error.to_js_error().into());
        }

        Err(Self::denied(&format!("{specifier} from {referrer}")).into())
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        if module_specifier == &self.main_specifier {
            if let Some(source) = &self.main_source {
                return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                    ModuleType::JavaScript,
                    ModuleSourceCode::String(source.clone().into()),
                    module_specifier,
                    None,
                )));
            }
        }

        if let Some(source) = clay_facade_source(module_specifier.as_str()) {
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(source.to_string().into()),
                module_specifier,
                None,
            )));
        }

        if module_specifier.as_str() == MARKDOWN_IT_MODULE_SPECIFIER {
            return ModuleLoadResponse::Sync(markdown_it_module_source().map(|source| {
                ModuleSource::new(
                    ModuleType::JavaScript,
                    ModuleSourceCode::String(source.into()),
                    module_specifier,
                    None,
                )
            }));
        }
        // First-party validated `loadEntry`: read the on-disk source the
        // resolver op recorded for this exact opaque specifier. Single gate,
        // same allowlist as `resolve`; no path outside the validated loadEntry
        // is ever read.
        if let Some(absolute_path) = self
            .first_party_load_entry_allowlist
            .absolute_path(module_specifier.as_str())
        {
            return ModuleLoadResponse::Sync(
                std::fs::read_to_string(&absolute_path)
                    .map_err(|error| {
                        Self::denied(&format!(
                            "first-party loadEntry {module_specifier} could not be loaded ({error})"
                        ))
                        .into()
                    })
                    .map(|source| {
                        ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(source.into()),
                            module_specifier,
                            None,
                        )
                    }),
            );
        }
        if let Some(configuration) = &self.configuration {
            return ModuleLoadResponse::Sync(
                configuration
                    .load_module_source(module_specifier)
                    .map(|source| {
                        ModuleSource::new(
                            ModuleType::JavaScript,
                            ModuleSourceCode::String(source.into()),
                            module_specifier,
                            None,
                        )
                    })
                    .map_err(|error| error.to_js_error().into()),
            );
        }

        ModuleLoadResponse::Sync(Err(Self::denied(module_specifier.as_str()).into()))
    }
}

fn markdown_it_module_source() -> Result<String, ModuleLoaderError> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages")
        .join("markdown")
        .join("node_modules")
        .join("markdown-it")
        .join("dist")
        .join("markdown-it.js");
    let bundled = std::fs::read_to_string(&path).map_err(|error| {
        ClayModuleLoader::denied(&format!(
            "markdown-it bundle could not be loaded from {} ({error})",
            path.display()
        ))
    })?;
    Ok(format!(
        "{bundled}\nconst MarkdownIt = globalThis.markdownit;\nexport default MarkdownIt;\nexport {{ MarkdownIt }};\n"
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use tokio::sync::Mutex;

    use deno_core::{
        ModuleLoadOptions, ModuleLoadResponse, ModuleLoader, ModuleSpecifier, ModuleType,
        RequestedModuleType, ResolutionKind,
    };

    use super::{
        ClayJsRuntimeService, ClayModuleLoader, ClayRuntimeError, FirstPartyLoadEntryAllowlist,
    };
    use crate::protocol::DiagnosticSeverity;
    use crate::server::configuration::ConfigurationRuntime;
    use crate::server::workspace::WorkspaceState;

    fn config_fixture(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("clay-{name}-{suffix}"));
        fs::create_dir_all(&root).expect("create configuration fixture root");
        root
    }

    #[tokio::test]
    async fn js_runtime_evaluates_controlled_module() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                const ping = Deno.core.ops.op_clay_runtime_ping();
                if (ping !== "clay-runtime-ready") {
                    throw new Error(`unexpected ping: ${ping}`);
                }
                Deno.core.ops.op_clay_runtime_record("configured");
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["configured"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn js_runtime_rejects_unsafe_or_unknown_imports() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(r#"import "https://example.invalid/module.js";"#)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("clay.runtime.invalid_import"));
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn configuration_runtime_loads_init_js_fixture() {
        let root = config_fixture("init");
        fs::write(
            root.join("init.js"),
            r#"Deno.core.ops.op_clay_runtime_record("init-loaded");"#,
        )
        .unwrap();

        let service = ClayJsRuntimeService::default();
        let result = service.load_configuration_from_root(root).await.unwrap();

        assert_eq!(result.op_records, vec!["init-loaded"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn configuration_runtime_loads_relative_module() {
        let root = config_fixture("relative");
        fs::write(
            root.join("init.js"),
            r#"
            import { getConfigurationState, loadConfigurationModule } from "clay:configuration";
            await loadConfigurationModule({ path: "./ui.js" });
            const state = getConfigurationState();
            Deno.core.ops.op_clay_runtime_record(state.entryPoint);
            Deno.core.ops.op_clay_runtime_record(state.loadedModules.join(","));
            "#,
        )
        .unwrap();
        fs::write(
            root.join("ui.js"),
            r#"Deno.core.ops.op_clay_runtime_record("ui-loaded");"#,
        )
        .unwrap();

        let service = ClayJsRuntimeService::default();
        let result = service.load_configuration_from_root(root).await.unwrap();

        assert_eq!(result.op_records, vec!["ui-loaded", "./init.js", "./ui.js"]);
    }

    #[tokio::test]
    async fn runtime_imports_clay_sdui_facade() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { definePanel } from "clay:sdui";
                const panel = definePanel({ id: "root", title: "Runtime", children: [] });
                Deno.core.ops.op_clay_runtime_record(`${panel.kind}:${panel.title}:${panel.id}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["panel:Runtime:root"]);
    }

    #[tokio::test]
    async fn runtime_imports_clay_ui_facade_and_registers_contributions() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterCommand } from "clay:commands";
                import {
                  serverRegisterComponentContribution,
                  serverRegisterPanelContribution,
                  serverRegisterThemeToken,
                  serverRegisterTransientOverlayContribution,
                } from "clay:ui";

                const manifest = {
                  name: "@clay/markdown",
                  version: "0.1.0",
                  clay: {
                    apiPrefix: "markdown",
                    entry: "./dist/index.js",
                    permissions: ["command-registration"],
                    modes: ["markdown"],
                  },
                };
                serverRegisterCommand(manifest, {
                  commandId: "markdown.togglePreview",
                  displayName: "Toggle Markdown Preview",
                  routingPolicy: "server-first",
                });
                const token = serverRegisterThemeToken(manifest, {
                  token: "markdown.preview.background",
                  type: "color-role",
                  fallback: "surface.panel",
                  description: "Markdown preview background",
                });
                const component = serverRegisterComponentContribution(manifest, {
                  kind: "label",
                  id: "markdown.preview.empty",
                  text: "Preview unavailable",
                });
                const panel = serverRegisterPanelContribution(manifest, {
                  id: "markdown.preview",
                  slot: "right",
                  kind: "fixed",
                  defaultVisibility: "hidden",
                  actionTargets: ["markdown.togglePreview"],
                  component: {
                    kind: "panel",
                    id: "markdown.preview.root",
                    title: "Preview",
                    children: [{
                      kind: "button",
                      id: "markdown.preview.toggle",
                      label: "Toggle",
                      action: { commandId: "markdown.togglePreview" },
                    }],
                  },
                });
                const overlay = serverRegisterTransientOverlayContribution(manifest, {
                  id: "markdown.preview.overlay",
                  anchor: "working-area",
                  focusPolicy: "restore",
                  dismissalPolicy: "escape",
                  component: { kind: "panel", id: "markdown.preview.overlay.root", title: "Overlay", children: [] },
                });
                Deno.core.ops.op_clay_runtime_record(`${panel.slot}:${component.rootKind}:${overlay.focusPolicy}:${token.type}:${panel.provenance.apiPrefix}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["right:label:restore:color-role:markdown"]
        );
        assert_eq!(result.ui_contributions.panels.len(), 1);
        assert_eq!(result.ui_contributions.components.len(), 1);
        assert_eq!(result.ui_contributions.overlays.len(), 1);
        assert_eq!(result.ui_contributions.theme_tokens.len(), 1);
        assert_eq!(
            result.ui_contributions.panels[0].provenance.package_name,
            "@clay/markdown"
        );
    }

    #[tokio::test]
    async fn runtime_clay_ui_rejects_invalid_prefix_unregistered_action_and_raw_css() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterPanelContribution } from "clay:ui";
                const manifest = {
                  name: "@clay/markdown",
                  version: "0.1.0",
                  clay: {
                    apiPrefix: "markdown",
                    entry: "./dist/index.js",
                    permissions: ["command-registration"],
                    modes: ["markdown"],
                  },
                };
                serverRegisterPanelContribution(manifest, {
                  id: "other.preview",
                  slot: "right",
                  rawCss: "color: red",
                  component: { kind: "button", id: "other.preview.button", label: "Run", action: { commandId: "markdown.missing" } },
                });
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("clay.ui.registration_failed"));
    }

    #[tokio::test]
    async fn runtime_facades_do_not_require_raw_ops() {
        let root = config_fixture("facade-no-raw-ops");
        fs::write(
            root.join("init.js"),
            r#"
            import { defineLabel } from "clay:sdui";
            import { getConfigurationState } from "clay:configuration";
            const label = defineLabel({ text: "Ready" });
            const state = getConfigurationState();
            if (label.kind !== "label" || state.entryPoint !== "./init.js") {
              throw new Error("facade import failed");
            }
            "#,
        )
        .unwrap();

        ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn unsupported_facade_returns_planned_error() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { serverGetDocumentSnapshot } from "clay:documents";
                await serverGetDocumentSnapshot("1");
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error
                .to_string()
                .contains("clay.documents.serverGetDocumentSnapshot is planned")
        );
    }

    #[tokio::test]
    async fn facade_op_mapping_matches_inventory() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { loadConfigurationModule } from "clay:configuration";
                import { defineStack } from "clay:sdui";
                const stack = defineStack({ children: [] });
                Deno.core.ops.op_clay_runtime_record(`${typeof loadConfigurationModule}:${stack.kind}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["function:stack"]);
    }

    #[tokio::test]
    async fn smoke_config_fixture_publishes_runtime_sdui_snapshot() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("runtime-sdui");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
        let tree = result.published_sdui_tree.expect("published SDUI tree");

        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Panel { title, .. } if title == "Runtime Smoke Workspace"
        )));
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::EditorView { binding }
                if binding.document_id == 1 && binding.expected_version == Some(1)
        )));
    }

    #[tokio::test]
    async fn markdown_config_fixture_opens_workspace_without_default_panel() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("markdown-mode");
        let workspace_root = root.join("workspace");
        let mut workspace = WorkspaceState::new();
        workspace
            .add_root(&workspace_root)
            .expect("markdown workspace fixture root must register");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        // Phase 20 task 4: the fixture uses the default load path and publishes
        // NO default side panel — only behavior/decorations state. The optional
        // preview is a package PanelContribution, validated separately by
        // `markdown_optional_preview_is_valid_panel_contribution`.
        assert!(
            result.published_sdui_tree.is_none(),
            "markdown-mode fixture must not publish a default side panel SDUI tree"
        );
        assert_eq!(result.parse_handlers.len(), 1);
        assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
        assert!(result.published_decoration_set.is_some());
        let manifest = result
            .behavior_manifest
            .expect("markdown behavior manifest");
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview")
        );
    }

    #[tokio::test]
    async fn windows_markdown_open_config_fixture_loads_without_default_panel() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("windows-markdown-open");
        let workspace_root = root.join("workspace");
        let mut workspace = WorkspaceState::new();
        workspace
            .add_root(&workspace_root)
            .expect("Windows Markdown open fixture root must register");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
            .await
            .unwrap();

        // Phase 20 task 4: the fixture uses the default load path and publishes
        // NO default side panel — only behavior/decorations state.
        assert!(
            result.published_sdui_tree.is_none(),
            "windows-markdown-open fixture must not publish a default side panel SDUI tree"
        );
        assert_eq!(result.parse_handlers.len(), 1);
        assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
        assert!(result.published_decoration_set.is_some());
        let manifest = result
            .behavior_manifest
            .expect("Windows Markdown open behavior manifest");
        assert!(manifest.keymaps.iter().any(|rule| {
            rule.sequence
                == vec![crate::protocol::KeyStroke {
                    key: crate::protocol::KeyCode::Character("o".to_string()),
                    modifiers: crate::protocol::KeyModifiers {
                        control: true,
                        ..crate::protocol::KeyModifiers::NONE
                    },
                }]
                && rule.command_id == "clay.documents.clientOpenFileDialog"
                && rule.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
        }));
        assert!(manifest.commands.iter().any(|command| {
            command.command_id == "clay.documents.clientOpenFileDialog"
                && command.authority == crate::protocol::CommandAuthority::ClientUi
        }));
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview")
        );
    }

    #[tokio::test]
    async fn markdown_package_runtime_loads_markdown_it_workflow() {
        let root = config_fixture("markdown-package-runtime");
        for file_name in ["index.js", "load.js", "parser.js", "sdui.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r##"
            import * as commands from "clay:commands";
            import * as decorations from "clay:decorations";
            import * as modes from "clay:modes";
            import * as packages from "clay:packages";
            import * as parse from "clay:parse";
            import * as sdui from "clay:sdui";
            import { loadMarkdownPackage } from "./load.js";
            import { publishMarkdownDecorations } from "./parser.js";
            import { publishMarkdownPreviewStatus } from "./sdui.js";

            const clay = { commands, decorations, modes, packages, parse, sdui };
            const contract = await loadMarkdownPackage(clay, {
              documentId: 1,
              path: "sample.md",
            });

            const text = "# Runtime package\n\n- item\n";
            const tokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Runtime package", children: [{ type: "text", content: "Runtime package" }] },
              { type: "heading_close" },
              { type: "bullet_list_open", map: [2, 3] },
              { type: "list_item_open", map: [2, 3] },
              { type: "inline", map: [2, 3], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "list_item_close" },
              { type: "bullet_list_close" },
            ];
            const update = await publishMarkdownDecorations(clay, {
              text,
              tokens,
              documentId: 1,
              documentVersion: 1,
              behaviorVersion: 2,
              viewport: { byteStart: 0, byteEnd: 64 },
            });
            await publishMarkdownPreviewStatus(clay, {
              documentId: 1,
              documentVersion: 1,
              documentPath: "sample.md",
            });
            Deno.core.ops.op_clay_runtime_record(`${contract.parse.adapter}:${contract.sdui.adapter}:${update.publishedSpanCount}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["./dist/parser.js:./dist/sdui.js:2"]);
        assert_eq!(result.parse_handlers.len(), 1);
        assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
        assert!(result.published_decoration_set.is_some());
        let tree = result.published_sdui_tree.expect("Markdown SDUI tree");
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Label { text } if text == "Parse: markdown-it registered"
        )));
        let manifest = result
            .behavior_manifest
            .expect("Markdown behavior manifest");
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview")
        );
    }

    #[tokio::test]
    async fn configuration_can_publish_sdui_snapshot() {
        let root = config_fixture("sdui-publish");
        fs::write(
            root.join("init.js"),
            r#"
            import {
              defineButton,
              defineEditorView,
              defineFlex,
              defineLabel,
              defineList,
              definePanel,
              defineStack,
              publishTree,
            } from "clay:sdui";

            const tree = defineFlex({
              id: "root",
              direction: "row",
              children: [
                definePanel({
                  id: "panel",
                  title: "Runtime Workspace",
                  children: [defineStack({
                    id: "stack",
                    children: [
                      defineLabel({ id: "label", text: "Ready" }),
                      defineButton({
                        id: "refresh",
                        label: "Refresh",
                        action: { commandId: "workspace.refresh", arguments: { force: true } },
                      }),
                      defineList({
                        id: "documents",
                        items: [{
                          id: "active",
                          label: "Document 1",
                          detail: "Runtime generated",
                          action: { commandId: "document.open_recent" },
                        }],
                      }),
                    ],
                  })],
                }),
                defineEditorView({ id: "editor", documentId: 1, expectedVersion: 1 }),
              ],
            });
            await publishTree(tree);
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
        let tree = result.published_sdui_tree.expect("published SDUI tree");

        assert_eq!(tree.ui_version, 1);
        assert_eq!(tree.nodes.len(), 7);
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Panel { title, .. } if title == "Runtime Workspace"
        )));
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::EditorView { binding }
                if binding.document_id == 1 && binding.expected_version == Some(1)
        )));
    }

    #[tokio::test]
    async fn js_generated_sdui_rejects_unknown_document_binding() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { defineEditorView, publishTree } from "clay:sdui";
                await publishTree(defineEditorView({ documentId: 999 }));
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("clay.sdui.invalid_tree"));
    }

    #[tokio::test]
    async fn js_generated_sdui_rejects_executable_action_payloads() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(
                r#"
                import { defineButton, publishTree } from "clay:sdui";
                await publishTree(defineButton({
                  label: "Run",
                  action: { commandId: "shell.run", arguments: { code: "rm -rf /" } },
                }));
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("clay.sdui.invalid_action"));
    }

    #[tokio::test]
    async fn document_facade_open_status_list_round_trip() {
        let config_root = config_fixture("document-facade");
        let workspace_root = config_root.join("workspace");
        fs::create_dir(&workspace_root).unwrap();
        fs::write(workspace_root.join("note.txt"), "hello").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import {
              serverGetDocumentStatus,
              serverListDocuments,
              serverOpenDocument,
              serverReloadDocument,
              serverSaveDocument,
            } from "clay:documents";

            const opened = await serverOpenDocument({ workspaceRootId: "1", path: "note.txt" });
            const status = await serverGetDocumentStatus(opened.metadata.documentId);
            const saved = await serverSaveDocument({ documentId: opened.metadata.documentId });
            const reloaded = await serverReloadDocument({ documentId: opened.metadata.documentId });
            const documents = await serverListDocuments();
            Deno.core.ops.op_clay_runtime_record(`${opened.text}:${status.path}:${saved.dirty}:${reloaded.text}:${documents.length}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&workspace_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["hello:note.txt:false:hello:1"]);
    }

    #[tokio::test]
    async fn workspace_roots_facade_reports_authorized_roots() {
        let config_root = config_fixture("workspace-facade");
        let workspace_root = config_root.join("project");
        fs::create_dir(&workspace_root).unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { serverListWorkspaceRoots } from "clay:workspace";
            const roots = await serverListWorkspaceRoots();
            Deno.core.ops.op_clay_runtime_record(`${roots.length}:${roots[0].workspaceRootId}:${roots[0].displayName}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&workspace_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["1:1:project"]);
    }

    #[tokio::test]
    async fn document_facade_rejects_unauthorized_paths() {
        let parent = config_fixture("document-facade-reject");
        let config_root = parent.join("config");
        let workspace_root = parent.join("workspace");
        fs::create_dir(&config_root).unwrap();
        fs::create_dir(&workspace_root).unwrap();
        fs::write(parent.join("outside.txt"), "secret").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { serverOpenDocument } from "clay:documents";
            await serverOpenDocument({ workspaceRootId: "1", path: "../outside.txt" });
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&workspace_root).unwrap();

        let error = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("clay.documents.open_failed"));
        assert!(error.to_string().contains("outside the authorized root"));
    }

    #[tokio::test]
    async fn configuration_runtime_rejects_traversal_and_urls() {
        for rejected_path in [
            "../outside.js",
            "https://example.invalid/config.js",
            "npm:pkg",
            "package",
        ] {
            let root = config_fixture("reject");
            fs::write(
                root.join("init.js"),
                format!(
                    r#"
                    import {{ loadConfigurationModule }} from "clay:configuration";
                    await loadConfigurationModule({{ path: "{rejected_path}" }});
                    "#
                ),
            )
            .unwrap();
            let error = ClayJsRuntimeService::default()
                .load_configuration_from_root(root)
                .await
                .unwrap_err();

            assert!(matches!(error, ClayRuntimeError::Runtime(_)));
            assert!(
                error
                    .to_string()
                    .contains("clay.configuration.invalid_module")
            );
        }
    }

    #[tokio::test]
    async fn configuration_bind_key_updates_behavior_manifest() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings } from "clay:keybindings";
                import { getActiveBehaviorManifest, listBehaviorRoutes } from "clay:behavior";
                const bound = bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
                const bindings = listKeyBindings("editor");
                const manifest = await getActiveBehaviorManifest();
                const routes = await listBehaviorRoutes();
                Deno.core.ops.op_clay_runtime_record(`${bound.key}:${bound.command}:${manifest.version}:${bindings.length}:${routes.some((route) => route.apiId === "clay.documents.serverSaveDocument")}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(
            result.op_records,
            vec!["Ctrl+S:clay.documents.serverSaveDocument:2:3:true"]
        );
        assert_eq!(manifest.behavior_version, 2);
        assert!(manifest.keymaps.iter().any(|rule| {
            rule.command_id == "clay.documents.serverSaveDocument"
                && rule.routing_policy == crate::protocol::RoutingPolicy::ServerFirst
        }));
    }

    #[tokio::test]
    async fn configuration_bind_ctrl_o_to_client_open_file_dialog() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings } from "clay:keybindings";
                import { listBehaviorRoutes } from "clay:behavior";
                const bound = bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
                const bindings = listKeyBindings("editor");
                const routes = await listBehaviorRoutes();
                const route = routes.find((candidate) => candidate.apiId === "clay.documents.clientOpenFileDialog");
                Deno.core.ops.op_clay_runtime_record(`${bound.key}:${bound.command}:${bindings.length}:${route.runtimePath}:${route.authority}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(
            result.op_records,
            vec!["Ctrl+O:clay.documents.clientOpenFileDialog:3:client-ui-command:client-ui"]
        );
        assert!(manifest.keymaps.iter().any(|rule| {
            rule.command_id == "clay.documents.clientOpenFileDialog"
                && rule.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
        }));
        assert!(manifest.commands.iter().any(|command| {
            command.command_id == "clay.documents.clientOpenFileDialog"
                && command.authority == crate::protocol::CommandAuthority::ClientUi
        }));
    }

    #[tokio::test]
    async fn configuration_unbind_key_updates_behavior_manifest() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey, unbindKey, listKeyBindings } from "clay:keybindings";
                bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
                unbindKey("Ctrl+S", { scope: "editor" });
                Deno.core.ops.op_clay_runtime_record(`${listKeyBindings("editor").some((binding) => binding.key === "Ctrl+S")}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(result.op_records, vec!["false"]);
        assert_eq!(manifest.behavior_version, 3);
        assert!(
            !manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "clay.documents.serverSaveDocument")
        );
    }

    #[tokio::test]
    async fn unknown_command_binding_is_rejected() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { bindKey } from "clay:keybindings";
                bindKey("Ctrl+X", "shell.run");
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error
                .to_string()
                .contains("clay.keybindings.unknown_command")
        );
    }

    #[tokio::test]
    async fn runtime_imports_modes_commands_and_packages_facades() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterModePattern, serverActivateMajorMode } from "clay:modes";
                import { serverRegisterCommand, serverListCommands } from "clay:commands";
                import { serverLoadPackage, serverValidatePackagePermissions } from "clay:packages";
                import { serverPublishDecorations } from "clay:decorations";
                import { serverRegisterParseHandler } from "clay:parse";

                if (typeof serverPublishDecorations !== "function" || typeof serverRegisterParseHandler !== "function") {
                  throw new Error("decoration/parse facade export missing");
                }
                const manifest = {
                  name: "@clay/markdown",
                  version: "0.1.0",
                  clay: {
                    apiPrefix: "markdown",
                    permissions: ["mode-registration", "mode-activation", "command-registration", "parse-document", "package-configuration"],
                    modes: ["markdown"],
                    entry: "./dist/index.js",
                    loadEntry: "./dist/load.js",
                    docs: "./docs/index.md",
                    performance: { estimatedManifestBytes: 2048 },
                    apiDependencies: ["clay.modes.serverRegisterModePattern", "clay.commands.serverRegisterCommand"],
                    contributions: {
                      commands: [{ id: "markdown.togglePreview", displayName: "Toggle Markdown Preview", routingPolicy: "server-first" }],
                      configuration: [{ key: "markdown.preview.enabled", type: "boolean", default: false }]
                    }
                  }
                };
                const loaded = serverLoadPackage(manifest);
                const permissions = serverValidatePackagePermissions(manifest.clay.permissions);
                serverRegisterModePattern(manifest, {
                  modeId: "markdown",
                  displayName: "Markdown",
                  extensions: ["md"],
                  mimeTypes: ["text/markdown"]
                });
                const activation = serverActivateMajorMode(manifest, { documentId: 5, path: "README.md" });
                const command = serverRegisterCommand(manifest, {
                  commandId: "markdown.togglePreview",
                  displayName: "Toggle Markdown Preview",
                  permissions: ["parse-document"]
                });
                const commands = serverListCommands();
                Deno.core.ops.op_clay_runtime_record(`${loaded.contributions.commands}:${permissions.permissions.length}:${activation.modeId}:${activation.behaviorVersion}:${command.commandId}:${commands.length}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["1:5:markdown:1:markdown.togglePreview:1"]
        );
    }

    #[tokio::test]
    async fn primitive_facades_return_actionable_validation_errors() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverValidatePackagePermissions } from "clay:packages";
                serverValidatePackagePermissions(["network"]);
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error
                .to_string()
                .contains("clay.packages.prohibited_authority")
        );
        assert!(error.to_string().contains("network"));
    }

    #[tokio::test]
    async fn primitive_configuration_facades_promote_package_options_only() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { setPackageOption, setModePreference, setDecorationTheme, setParsePolicy } from "clay:configuration";
                if ([setPackageOption, setModePreference, setDecorationTheme, setParsePolicy].some((api) => typeof api !== "function")) {
                  throw new Error("configuration primitive facade export missing");
                }
                setModePreference({ modeId: "markdown", source: "init-js" });
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error
                .to_string()
                .contains("clay.configuration.setModePreference is planned")
        );
    }

    #[tokio::test]
    async fn markdown_large_file_parse_policy_rejects_unsafe_values() {
        for (name, policy_fields, expected) in [
            (
                "zero timeout",
                "timeoutMs: 0, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 30 * 1024 * 1024",
                "timeoutMs must be between 1 and 5000",
            ),
            (
                "oversized timeout",
                "timeoutMs: 5001, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 30 * 1024 * 1024",
                "timeoutMs must be between 1 and 5000",
            ),
            (
                "zero cache budget",
                "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 0",
                "window and memory budgets must be non-zero",
            ),
            (
                "window larger than cache budget",
                "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 1024",
                "window and memory budgets must be non-zero",
            ),
            (
                "unbounded cache budget",
                "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 64 * 1024 * 1024",
                "window and memory budgets must be non-zero",
            ),
        ] {
            let source = format!(
                r#"
                import {{ serverRegisterParseHandler }} from "clay:parse";
                const manifest = {{
                  name: "@clay/markdown",
                  version: "0.1.0",
                  type: "module",
                  exports: {{ ".": "./dist/index.js" }},
                  clay: {{
                    apiPrefix: "markdown",
                    entry: "./dist/index.js",
                    permissions: ["parse-document"],
                    modes: ["markdown"],
                    docs: "./docs/index.md"
                  }}
                }};
                serverRegisterParseHandler({{
                  packageManifest: manifest,
                  mode: "markdown",
                  parseUnit: "line-group",
                  viewportPriority: true,
                  {policy_fields}
                }});
                "#
            );
            let error = ClayJsRuntimeService::default()
                .evaluate_controlled_module(source)
                .await
                .unwrap_err();

            assert!(
                matches!(error, ClayRuntimeError::Runtime(_)),
                "{name} should fail in the runtime"
            );
            assert!(
                error.to_string().contains(expected),
                "{name} should reject unsafe parse policy with `{expected}`, got {error}"
            );
        }
    }

    #[tokio::test]
    async fn phase18_parse_and_decoration_facades_are_runtime_backed() {
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverPublishDecorations } from "clay:decorations";
                import { serverRegisterParseHandler } from "clay:parse";
                const manifest = {
                  name: "@clay/markdown",
                  version: "0.1.0",
                  type: "module",
                  exports: { ".": "./dist/index.js" },
                  clay: {
                    apiPrefix: "markdown",
                    entry: "./dist/index.js",
                    permissions: ["parse-document", "render-decorations"],
                    modes: ["markdown"],
                    docs: "./docs/index.md"
                  }
                };
                const handler = serverRegisterParseHandler({
                  packageManifest: manifest,
                  mode: "markdown",
                  parseUnit: "line-group",
                  viewportPriority: true,
                });
                const decorations = serverPublishDecorations({
                  packageManifest: manifest,
                  documentId: 1,
                  documentVersion: 1,
                  behaviorVersion: 1,
                  viewport: { byteStart: 0, byteEnd: 12 },
                  spans: [{ byteStart: 0, byteEnd: 5, kind: "syntax", styleToken: "markup.heading.1", priority: 10 }],
                });
                Deno.core.ops.op_clay_runtime_record(`${handler.mode}:${decorations.publishedSpanCount}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["markdown:1"]);
        assert_eq!(result.parse_handlers.len(), 1);
        assert!(result.published_decoration_set.is_some());
    }

    #[tokio::test]
    async fn markdown_parser_adapter_publishes_viewport_bounded_decorations() {
        let root = config_fixture("markdown-parser-adapter");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { serverPublishDecorations } from "clay:decorations";
            import { parseMarkdownDecorations, publishMarkdownDecorations } from "./parser.js";

            const text = "# Hé 🦀\n\nSome **bold** and *em* and `code`.\n\n```js\nx\n```\n\n1. item\n";
            const markdownTokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Hé 🦀", children: [] },
              { type: "heading_close" },
              { type: "paragraph_open", map: [2, 3] },
              {
                type: "inline",
                map: [2, 3],
                content: "Some **bold** and *em* and `code`.",
                children: [
                  { type: "text", content: "Some " },
                  { type: "strong_open", markup: "**" },
                  { type: "text", content: "bold" },
                  { type: "strong_close", markup: "**" },
                  { type: "text", content: " and " },
                  { type: "em_open", markup: "*" },
                  { type: "text", content: "em" },
                  { type: "em_close", markup: "*" },
                  { type: "text", content: " and " },
                  { type: "code_inline", markup: "`", content: "code" }
                ]
              },
              { type: "paragraph_close" },
              { type: "fence", tag: "code", map: [4, 7], markup: "```", info: "js" },
              { type: "ordered_list_open", map: [8, 9] },
              { type: "list_item_open", map: [8, 9] },
              { type: "paragraph_open", map: [8, 9] },
              { type: "inline", map: [8, 9], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "paragraph_close" },
              { type: "list_item_close" },
              { type: "ordered_list_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            function byteRangeFor(needle, from = 0) {
              const codeUnitStart = text.indexOf(needle, from);
              if (codeUnitStart < 0) throw new Error(`missing fixture needle: ${needle}`);
              return {
                byteStart: utf8ByteLength(text.slice(0, codeUnitStart)),
                byteEnd: utf8ByteLength(text.slice(0, codeUnitStart + needle.length)),
                codeUnitEnd: codeUnitStart + needle.length,
              };
            }
            function requireSpan(styleToken) {
              const span = spans.find((candidate) => candidate.styleToken === styleToken);
              if (!span) throw new Error(`missing span ${styleToken} in ${JSON.stringify(spans)}`);
              return span;
            }
            function assertSpan(styleToken, expected) {
              const span = requireSpan(styleToken);
              if (span.byteStart !== expected.byteStart || span.byteEnd !== expected.byteEnd) {
                throw new Error(`${styleToken} expected ${expected.byteStart}:${expected.byteEnd}, got ${span.byteStart}:${span.byteEnd}`);
              }
            }

            const fullViewport = { byteStart: 0, byteEnd: utf8ByteLength(text) };
            const spans = await parseMarkdownDecorations({ text, tokens: markdownTokens, viewport: fullViewport });
            assertSpan("markup.heading.1", { byteStart: 0, byteEnd: utf8ByteLength("# Hé 🦀") });
            assertSpan("markup.strong", byteRangeFor("**bold**"));
            assertSpan("markup.emphasis", byteRangeFor("*em*"));
            assertSpan("markup.inline-code", byteRangeFor("`code`"));
            assertSpan("markup.list-marker", byteRangeFor("1."));
            const fenceStart = byteRangeFor("```js");
            const fenceTerminator = byteRangeFor("\n\n1. item");
            assertSpan("markup.code-block", { byteStart: fenceStart.byteStart, byteEnd: utf8ByteLength(text.slice(0, fenceTerminator.codeUnitEnd - "\n1. item".length)) });

            const listMarker = requireSpan("markup.list-marker");
            const viewportOnlyList = await parseMarkdownDecorations({
              text,
              tokens: markdownTokens,
              viewport: { byteStart: listMarker.byteStart, byteEnd: listMarker.byteEnd },
            });
            if (viewportOnlyList.length !== 1 || viewportOnlyList[0].styleToken !== "markup.list-marker") {
              throw new Error(`viewport filter leaked spans: ${JSON.stringify(viewportOnlyList)}`);
            }

            let parseCalls = 0;
            const fakeMarkdownIt = {
              parse(source, env) {
                parseCalls += 1;
                if (source !== text || !env) throw new Error("parse received unexpected arguments");
                return markdownTokens;
              },
              render() {
                throw new Error("adapter must not render HTML");
              }
            };
            await parseMarkdownDecorations({ text, markdownIt: fakeMarkdownIt, viewport: fullViewport });
            if (parseCalls !== 1) throw new Error(`expected one markdown-it parse call, got ${parseCalls}`);

            const tokens = spans.map((span) => span.styleToken).sort().join(",");
            const heading = requireSpan("markup.heading.1");
            const published = await publishMarkdownDecorations({ decorations: { serverPublishDecorations } }, {
              text,
              tokens: markdownTokens,
              documentId: 7,
              documentVersion: 3,
              behaviorVersion: 2,
              viewport: fullViewport,
            });
            Deno.core.ops.op_clay_runtime_record(tokens);
            Deno.core.ops.op_clay_runtime_record(`${heading.byteStart}:${heading.byteEnd}:${published.publishedSpanCount}:parseCalls=${parseCalls}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        let tokens = &result.op_records[0];
        for expected in [
            "markup.heading.1",
            "markup.strong",
            "markup.emphasis",
            "markup.inline-code",
            "markup.code-block",
            "markup.list-marker",
        ] {
            assert!(tokens.contains(expected), "missing {expected} in {tokens}");
        }
        assert_eq!(result.op_records[1], "0:10:6:parseCalls=1");
        assert_eq!(result.published_decoration_set.unwrap().spans.len(), 6);
    }

    #[tokio::test]
    async fn markdown_windowed_adapter_offsets_ranges_to_absolute_document_bytes() {
        let root = config_fixture("markdown-windowed-absolute-ranges");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorations } from "./parser.js";

            const windowText = "# Hé 🦀\n\nParagraph **dé** and `cø`.\n";
            const absoluteByteStart = 4096;
            const tokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Hé 🦀", children: [{ type: "text", content: "Hé 🦀" }] },
              { type: "heading_close" },
              { type: "paragraph_open", map: [2, 3] },
              {
                type: "inline",
                map: [2, 3],
                content: "Paragraph **dé** and `cø`.",
                children: [
                  { type: "text", content: "Paragraph " },
                  { type: "strong_open", markup: "**" },
                  { type: "text", content: "dé" },
                  { type: "strong_close", markup: "**" },
                  { type: "text", content: " and " },
                  { type: "code_inline", markup: "`", content: "cø" },
                  { type: "text", content: "." }
                ]
              },
              { type: "paragraph_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            function absoluteRangeFor(needle, from = 0) {
              const start = windowText.indexOf(needle, from);
              if (start < 0) throw new Error(`missing ${needle}`);
              return {
                byteStart: absoluteByteStart + utf8ByteLength(windowText.slice(0, start)),
                byteEnd: absoluteByteStart + utf8ByteLength(windowText.slice(0, start + needle.length))
              };
            }
            function span(styleToken) {
              const found = spans.find((candidate) => candidate.styleToken === styleToken);
              if (!found) throw new Error(`missing ${styleToken} in ${JSON.stringify(spans)}`);
              return found;
            }
            function assertRange(styleToken, range) {
              const found = span(styleToken);
              if (found.byteStart !== range.byteStart || found.byteEnd !== range.byteEnd) {
                throw new Error(`${styleToken} expected ${range.byteStart}:${range.byteEnd}, got ${found.byteStart}:${found.byteEnd}`);
              }
            }

            let parseCalls = 0;
            const fakeMarkdownIt = {
              parse(source, env) {
                parseCalls += 1;
                if (source !== windowText || !env) throw new Error("markdown-it must receive only window text");
                return tokens;
              },
              render() {
                throw new Error("windowed adapter must not render HTML");
              }
            };
            const spans = await parseMarkdownDecorations({
              text: windowText,
              absoluteByteStart,
              baseLine: 120,
              parseWindow: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText), baseLine: 120 },
              viewport: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
              markdownIt: fakeMarkdownIt
            });

            assertRange("markup.heading.1", { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength("# Hé 🦀") });
            assertRange("markup.strong", absoluteRangeFor("**dé**"));
            assertRange("markup.inline-code", absoluteRangeFor("`cø`"));
            Deno.core.ops.op_clay_runtime_record(`${spans.length}:parseCalls=${parseCalls}:${span("markup.strong").byteStart}:${span("markup.inline-code").byteEnd}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["3:parseCalls=1:4118:4135"]);
    }

    #[tokio::test]
    async fn markdown_windowed_adapter_does_not_parse_full_large_document() {
        let root = config_fixture("markdown-windowed-no-full-doc");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorationUpdate } from "./parser.js";

            const windowText = "# Visible\n\n- item\n";
            const absoluteByteStart = 8 * 1024 * 1024;
            const largeDocumentSentinel = "x".repeat(16 * 1024 * 1024);
            const tokens = [
              { type: "heading_open", tag: "h1", map: [0, 1] },
              { type: "inline", map: [0, 1], content: "Visible", children: [{ type: "text", content: "Visible" }] },
              { type: "heading_close" },
              { type: "bullet_list_open", map: [2, 3] },
              { type: "list_item_open", map: [2, 3] },
              { type: "paragraph_open", map: [2, 3] },
              { type: "inline", map: [2, 3], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "paragraph_close" },
              { type: "list_item_close" },
              { type: "bullet_list_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            let parseCalls = 0;
            const fakeMarkdownIt = {
              parse(source) {
                parseCalls += 1;
                if (source === largeDocumentSentinel || source.length !== windowText.length) {
                  throw new Error(`received unbounded source length ${source.length}`);
                }
                return tokens;
              }
            };
            const update = await parseMarkdownDecorationUpdate({
              documentId: 7,
              documentVersion: 3,
              behaviorVersion: 2,
              viewport: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
              parseWindows: [{
                text: windowText,
                byteStart: absoluteByteStart,
                byteEnd: absoluteByteStart + utf8ByteLength(windowText),
                baseLine: 900
              }],
              markdownIt: fakeMarkdownIt
            });
            if (update.spans.length !== 2) throw new Error(`expected heading and list marker spans, got ${JSON.stringify(update.spans)}`);
            Deno.core.ops.op_clay_runtime_record(`${update.viewport.byteStart}:${update.viewport.byteEnd}:${update.spans.length}:parseCalls=${parseCalls}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["8388608:8388626:2:parseCalls=1"]);
    }

    #[tokio::test]
    async fn markdown_windowed_adapter_preserves_fence_and_list_context() {
        let root = config_fixture("markdown-windowed-fence-list-context");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorations } from "./parser.js";

            const windowText = "```js\nconst visible = 1;\n```\n\n- item\n";
            const absoluteByteStart = 2048;
            const tokens = [
              { type: "fence", tag: "code", map: [0, 3], markup: "```", info: "js" },
              { type: "bullet_list_open", map: [4, 5] },
              { type: "list_item_open", map: [4, 5] },
              { type: "paragraph_open", map: [4, 5] },
              { type: "inline", map: [4, 5], content: "item", children: [{ type: "text", content: "item" }] },
              { type: "paragraph_close" },
              { type: "list_item_close" },
              { type: "bullet_list_close" }
            ];
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            const visibleStart = absoluteByteStart + utf8ByteLength("```js\n");
            const visibleEnd = absoluteByteStart + utf8ByteLength(windowText.slice(0, windowText.indexOf(" item")));
            const spans = await parseMarkdownDecorations({
              text: windowText,
              tokens,
              absoluteByteStart,
              parseWindow: { byteStart: absoluteByteStart, byteEnd: absoluteByteStart + utf8ByteLength(windowText) },
              viewport: { byteStart: visibleStart, byteEnd: visibleEnd }
            });
            const fence = spans.find((span) => span.styleToken === "markup.code-block");
            const list = spans.find((span) => span.styleToken === "markup.list-marker");
            if (!fence || fence.byteStart !== visibleStart || fence.byteEnd > visibleEnd) {
              throw new Error(`fence span was not clipped to the visible viewport: ${JSON.stringify(spans)}`);
            }
            if (!list || list.byteStart !== visibleEnd - 1 || list.byteEnd !== visibleEnd) {
              throw new Error(`list marker did not survive guard-window parsing: ${JSON.stringify(spans)}`);
            }
            Deno.core.ops.op_clay_runtime_record(`${spans.length}:${fence.byteStart}:${fence.byteEnd}:${list.byteStart}:${list.byteEnd}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["2:2054:2077:2078:2079"]);
    }

    #[tokio::test]
    async fn markdown_large_file_status_reports_windowed_highlighting() {
        let root = config_fixture("markdown-large-file-windowed-status");
        // index.js re-exports from ./load.js (markdownLoadMode fallback entry),
        // so the whole dist module graph must be copied for sdui.js to load.
        for file_name in ["index.js", "sdui.js", "load.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r##"
            import { markdownPreviewStatusModel } from "./sdui.js";

            const model = markdownPreviewStatusModel({
              documentByteLength: 16 * 1024 * 1024,
              documentPath: "C:/Users/alice/work/large.md"
            });
            if (model.status.highlightingState !== "windowed") throw new Error(JSON.stringify(model));
            if (model.status.fileTier !== "large") throw new Error(JSON.stringify(model));
            Deno.core.ops.op_clay_runtime_record(`${model.documentPath}:${model.status.parse}:${model.status.decorations}:${model.status.highlightingState}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec![
                "large.md:windowed visible syntax current:visible and near-viewport chunks current:windowed"
            ]
        );
    }

    #[tokio::test]
    async fn markdown_large_file_budget_exhaustion_falls_back_to_plain_text() {
        let root = config_fixture("markdown-large-file-plain-text-fallback");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorationUpdate } from "./parser.js";

            const windowText = "# Visible\n\n- item\n";
            const fakeMarkdownIt = {
              parse() {
                throw new Error("plain-text fallback must not invoke markdown-it");
              }
            };
            const update = await parseMarkdownDecorationUpdate({
              documentId: 9,
              documentVersion: 4,
              behaviorVersion: 2,
              viewport: { byteStart: 0, byteEnd: 18 },
              parseWindows: [{ text: windowText, byteStart: 0, byteEnd: 18, baseLine: 0 }],
              memoryBudgetBytes: 1,
              markdownIt: fakeMarkdownIt
            });
            if (update.spans.length !== 0) throw new Error(`fallback must clear spans: ${JSON.stringify(update.spans)}`);
            if (update.status.highlightingState !== "plain-text-fallback") throw new Error(JSON.stringify(update.status));
            Deno.core.ops.op_clay_runtime_record(`${update.spans.length}:${update.status.highlightingState}:${update.status.reason}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["0:plain-text-fallback:budget-exceeded"]
        );
    }

    #[tokio::test]
    async fn markdown_degraded_status_contains_no_document_text_or_paths() {
        let root = config_fixture("markdown-degraded-status-sanitized");
        for file_name in ["index.js", "sdui.js", "load.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r##"
            import { markdownPreviewStatusModel } from "./sdui.js";

            const model = markdownPreviewStatusModel({
              documentByteLength: 6 * 1024 * 1024,
              parserTimedOut: true,
              documentPath: "C:/Users/alice/secrets/project.md",
              diagnostic: "C:/Users/alice/secrets/project.md first line SECRET_DOCUMENT_TEXT"
            });
            const encoded = JSON.stringify(model);
            for (const forbidden of ["C:/", "Users/alice", "secrets/project.md", "SECRET_DOCUMENT_TEXT"]) {
              if (encoded.includes(forbidden)) throw new Error(`unsanitized status: ${encoded}`);
            }
            if (model.status.highlightingState !== "degraded") throw new Error(encoded);
            Deno.core.ops.op_clay_runtime_record(`${model.documentPath}:${model.status.parse}:${model.status.highlightingState}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["project.md:degraded; visible syntax refresh delayed:degraded"]
        );
    }

    #[tokio::test]
    async fn markdown_it_adapter_large_fixture_span_counts_are_stable() {
        let root = config_fixture("markdown-adapter-large-counts");
        let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
            .expect("markdown parser adapter must exist");
        fs::write(root.join("parser.js"), parser_source).unwrap();
        fs::write(
            root.join("init.js"),
            r##"
            import { parseMarkdownDecorations } from "./parser.js";

            const blockCount = 192;
            let text = "";
            const tokens = [];
            for (let index = 0; index < blockCount; index += 1) {
              const startLine = text.split("\n").length - 1;
              text += `# Heading ${index}\n\n`;
              text += `Paragraph ${index} has **strong**, *emphasis*, and \`code\`.\n\n`;
              text += "```js\nconst value = 1;\n```\n\n";
              text += `- bullet ${index}\n1. ordered ${index}\n\n`;
              tokens.push(
                { type: "heading_open", tag: "h1", map: [startLine, startLine + 1] },
                { type: "inline", map: [startLine, startLine + 1], content: `Heading ${index}`, children: [{ type: "text", content: `Heading ${index}` }] },
                { type: "heading_close" },
                { type: "paragraph_open", map: [startLine + 2, startLine + 3] },
                {
                  type: "inline",
                  map: [startLine + 2, startLine + 3],
                  content: `Paragraph ${index} has **strong**, *emphasis*, and \`code\`.`,
                  children: [
                    { type: "text", content: `Paragraph ${index} has ` },
                    { type: "strong_open", markup: "**" },
                    { type: "text", content: "strong" },
                    { type: "strong_close", markup: "**" },
                    { type: "text", content: ", " },
                    { type: "em_open", markup: "*" },
                    { type: "text", content: "emphasis" },
                    { type: "em_close", markup: "*" },
                    { type: "text", content: ", and " },
                    { type: "code_inline", markup: "`", content: "code" },
                    { type: "text", content: "." }
                  ]
                },
                { type: "paragraph_close" },
                { type: "fence", tag: "code", map: [startLine + 4, startLine + 7], markup: "```", info: "js" },
                { type: "bullet_list_open", map: [startLine + 8, startLine + 9] },
                { type: "list_item_open", map: [startLine + 8, startLine + 9] },
                { type: "paragraph_open", map: [startLine + 8, startLine + 9] },
                { type: "inline", map: [startLine + 8, startLine + 9], content: `bullet ${index}`, children: [{ type: "text", content: `bullet ${index}` }] },
                { type: "paragraph_close" },
                { type: "list_item_close" },
                { type: "bullet_list_close" },
                { type: "ordered_list_open", map: [startLine + 9, startLine + 10] },
                { type: "list_item_open", map: [startLine + 9, startLine + 10] },
                { type: "paragraph_open", map: [startLine + 9, startLine + 10] },
                { type: "inline", map: [startLine + 9, startLine + 10], content: `ordered ${index}`, children: [{ type: "text", content: `ordered ${index}` }] },
                { type: "paragraph_close" },
                { type: "list_item_close" },
                { type: "ordered_list_close" }
              );
            }
            function utf8ByteLength(value) {
              let bytes = 0;
              for (const character of value) {
                const codePoint = character.codePointAt(0);
                bytes += codePoint <= 0x7f ? 1 : codePoint <= 0x7ff ? 2 : codePoint <= 0xffff ? 3 : 4;
              }
              return bytes;
            }
            const viewport = { byteStart: 0, byteEnd: utf8ByteLength(text) };
            const first = await parseMarkdownDecorations({ text, tokens, viewport });
            const second = await parseMarkdownDecorations({ text, tokens, viewport });
            if (first.length !== second.length) throw new Error(`unstable span counts: ${first.length} != ${second.length}`);
            if (first.length !== blockCount * 7) throw new Error(`expected ${blockCount * 7} spans, got ${first.length}`);
            const byToken = new Map();
            for (const span of first) byToken.set(span.styleToken, (byToken.get(span.styleToken) ?? 0) + 1);
            for (const [token, expected] of [
              ["markup.heading.1", blockCount],
              ["markup.strong", blockCount],
              ["markup.emphasis", blockCount],
              ["markup.inline-code", blockCount],
              ["markup.code-block", blockCount],
              ["markup.list-marker", blockCount * 2],
            ]) {
              if (byToken.get(token) !== expected) throw new Error(`${token} expected ${expected}, got ${byToken.get(token)}`);
            }
            Deno.core.ops.op_clay_runtime_record(`${first.length}:${byToken.get("markup.list-marker")}`);
            "##,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["1344:384"]);
    }

    #[test]
    fn keypress_routing_uses_manifest_not_js() {
        let manifest = {
            let service = ClayJsRuntimeService::default();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime
                .block_on(service.evaluate_controlled_module(
                    r#"
                    import { bindKey } from "clay:keybindings";
                    bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
                    "#,
                ))
                .unwrap()
                .behavior_manifest
                .unwrap()
        };
        let state = crate::client::behavior::ClientBehaviorState::new(manifest).unwrap();
        let routed = state.route_key(&crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("s".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            crate::client::behavior::RoutedBehavior::ServerIntent(
                crate::client::behavior::ServerIntentRoute {
                    command_id: "clay.documents.serverSaveDocument".to_string(),
                    routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
                }
            )
        );
    }

    #[test]
    fn keypress_routing_can_reach_client_ui_command_without_js() {
        let manifest = {
            let service = ClayJsRuntimeService::default();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime
                .block_on(service.evaluate_controlled_module(
                    r#"
                    import { bindKey } from "clay:keybindings";
                    bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
                    "#,
                ))
                .unwrap()
                .behavior_manifest
                .unwrap()
        };
        let state = crate::client::behavior::ClientBehaviorState::new(manifest).unwrap();
        let routed = state.route_key(&crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("o".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        });

        assert_eq!(
            routed,
            crate::client::behavior::RoutedBehavior::ClientUiCommand(
                crate::client::behavior::ClientUiCommandRoute {
                    command_id: "clay.documents.clientOpenFileDialog".to_string(),
                    routing_policy: crate::protocol::RoutingPolicy::ClientUiCommand,
                }
            )
        );
    }

    #[test]
    fn ordinary_typing_does_not_enter_js_runtime() {
        let service = ClayJsRuntimeService::default();

        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn js_runtime_errors_are_typed_not_panics() {
        let service = ClayJsRuntimeService::default();
        let error = service
            .evaluate_controlled_module(r#"Deno.core.ops.op_clay_runtime_record("");"#)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("clay.runtime.invalid_record"));
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn runtime_syntax_error_reports_diagnostic() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(r#"const broken = ;"#)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
        assert_eq!(diagnostic.code, "clay.runtime.syntax_error");
        assert_eq!(
            diagnostic.message,
            "JavaScript syntax error while evaluating server-side configuration."
        );
    }

    #[tokio::test]
    async fn runtime_permission_error_reports_sanitized_diagnostic() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(r#"import "file:///home/example/.config/clay/secret.js";"#)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, "clay.runtime.invalid_import");
        assert!(!diagnostic.message.contains("/home/example"));
        assert!(!diagnostic.message.contains("secret.js"));
    }

    #[tokio::test]
    async fn runtime_op_validation_error_reports_diagnostic() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(r#"Deno.core.ops.op_clay_runtime_record("");"#)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, "clay.runtime.invalid_record");
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    }

    /// Helper: call the raw resolver op from a controlled module. The public
    /// `loadPackage` facade is wired in Phase 18.6 task 5; these op-level
    /// tests exercise the resolver directly so the security boundary is
    /// covered before the facade lands.
    async fn resolve_by_specifier(specifier: &str) -> Result<String, String> {
        let source = format!(
            r#"
            const result = Deno.core.ops.op_clay_packages_load_package_by_specifier(
              JSON.stringify({{ specifier: {specifier:?} }})
            );
            globalThis.__clay_result = result;
            "#
        );
        match ClayJsRuntimeService::default()
            .evaluate_controlled_module(source)
            .await
        {
            Ok(_) => Ok("ok".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_rejects_non_first_party_specifier() {
        // Non-`@clay/*` specifiers are denied before the package service is
        // touched. Third-party registry specs, bare names, and `npm:` specs.
        for denied in [
            "lodash",
            "npm:foo",
            "markdown",
            "react",
            "@types/node",
            "@clay/",
            "@clay/../escape",
            "@clay/foo/bar",
        ] {
            let err = resolve_by_specifier(denied).await.unwrap_err();
            assert!(
                err.contains("clay.packages.invalid_specifier"),
                "specifier `{denied}` must be denied with invalid_specifier, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_rejects_unknown_package() {
        // `@clay/*` shape but no installed package on disk.
        let err = resolve_by_specifier("@clay/does-not-exist")
            .await
            .unwrap_err();
        assert!(
            err.contains("clay.packages.not_installed"),
            "unknown first-party package must be not_installed, got: {err}"
        );
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_resolves_and_enables_first_party_markdown()
    {
        // The real shipped `@clay/markdown` package validates/enables through
        // PackageService and returns an opaque loadEntrySpecifier. The module
        // import itself is task 4/5; here we prove resolve + enable works and
        // the opaque specifier is recorded in the allowlist via the returned
        // summary shape.
        let source = r#"
            const raw = Deno.core.ops.op_clay_packages_load_package_by_specifier(
              JSON.stringify({ specifier: "@clay/markdown" })
            );
            const summary = JSON.parse(raw);
            globalThis.__clay_summary = summary;
        "#;
        let evaluation = ClayJsRuntimeService::default()
            .evaluate_controlled_module(source)
            .await
            .expect("@clay/markdown must resolve and enable");

        // The op returns the typed summary as a JSON string; we cannot read
        // `globalThis` after the runtime tears down, so we assert the op ran
        // without error and that subsequent resolver calls for the same
        // package succeed (idempotent enable via AlreadyEnabled fallback).
        assert!(evaluation.behavior_manifest.is_none());
        let second = resolve_by_specifier("@clay/markdown").await;
        assert!(
            second.is_ok(),
            "resolving an already-enabled package must be idempotent, got: {second:?}"
        );
    }

    /// Build an isolated `ClayModuleLoader` with a manually-populated allowlist
    /// (no resolver op, no real runtime) so the resolve/load gate is tested in
    /// isolation. `configuration` mirrors the runtime's config-root branch.
    fn loader_with_allowlist(
        entries: &[(&str, PathBuf, PathBuf)],
        configuration: Option<Arc<ConfigurationRuntime>>,
    ) -> ClayModuleLoader {
        let allowlist = Arc::new(FirstPartyLoadEntryAllowlist::default());
        for (specifier, path, package_root) in entries {
            allowlist.record(specifier, path.clone(), package_root.clone());
        }
        let main_specifier = ModuleSpecifier::parse("clay://runtime/main.js").unwrap();
        ClayModuleLoader::new(main_specifier, None, configuration, allowlist)
    }

    fn default_load_options() -> ModuleLoadOptions {
        ModuleLoadOptions {
            is_dynamic_import: false,
            is_synchronous: false,
            requested_module_type: RequestedModuleType::None,
        }
    }

    #[test]
    fn clay_module_loader_loads_allowlisted_first_party_load_entry() {
        // A real on-disk loadEntry OUTSIDE any config root, recorded in the
        // allowlist (what the resolver op does), must resolve and load.
        let outside_root = config_fixture("loader-loadentry");
        let loadentry_path = outside_root.join("load.js");
        fs::write(&loadentry_path, "export const clayLoadedEntry = true;\n").unwrap();

        let opaque = "clay://packages/@clay/example/dist/load.js";
        let loader = loader_with_allowlist(&[(opaque, loadentry_path, outside_root)], None);

        let resolved = loader
            .resolve(opaque, "clay://runtime/main.js", ResolutionKind::Import)
            .expect("allowlisted loadEntry must resolve");
        assert_eq!(resolved.as_str(), opaque);

        let source = match loader.load(&resolved, None, default_load_options()) {
            ModuleLoadResponse::Sync(Ok(source)) => source,
            ModuleLoadResponse::Sync(Err(error)) => panic!("load failed: {error:?}"),
            _ => panic!("expected sync response, got async"),
        };
        assert_eq!(source.module_type, ModuleType::JavaScript);
        assert!(
            std::str::from_utf8(source.code.as_bytes())
                .unwrap()
                .contains("clayLoadedEntry"),
            "load must return the recorded on-disk loadEntry source"
        );
    }

    #[test]
    fn clay_module_loader_denies_unallowlisted_first_party_url() {
        // Empty allowlist: every `clay://packages/...` URL is denied exactly
        // like any other untrusted specifier, even loadEntry-shaped ones.
        let loader = loader_with_allowlist(&[], None);
        for url in [
            "clay://packages/@clay/markdown/dist/load.js",
            "clay://packages/@clay/evil/x.js",
            "clay://packages/anything",
        ] {
            let error = loader
                .resolve(url, "clay://runtime/main.js", ResolutionKind::Import)
                .expect_err("unallowlisted package URL must be denied");
            assert!(
                error.to_string().contains("clay.runtime.invalid_import"),
                "unallowlisted `{url}` must be denied, got: {error:?}"
            );
        }
    }

    #[test]
    fn clay_module_loader_preserves_config_root_confinement_for_non_package_imports() {
        // A real config root exercises the configuration branch. The allowlist
        // addition must NOT relax config-root confinement: escaping imports are
        // still rejected, while an allowlisted first-party loadEntry still loads.
        let parent = config_fixture("loader-configroot-parent");
        let root = parent.join("config");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("init.js"), "export const ready = true;\n").unwrap();
        // `escape.js` lives in the parent (a real file OUTSIDE config root) so
        // `canonicalize` succeeds and the `starts_with(config_root)` check is
        // the thing that rejects it.
        fs::write(parent.join("escape.js"), "export const escape = true;\n").unwrap();
        let configuration = Arc::new(ConfigurationRuntime::from_config_root(&root).unwrap());

        // Allowlisted loadEntry lives OUTSIDE the config root but still loads.
        let outside = config_fixture("loader-configroot-loadentry");
        let loadentry_path = outside.join("load.js");
        fs::write(&loadentry_path, "export const ok = true;\n").unwrap();
        let opaque = "clay://packages/@clay/example/dist/load.js";
        let loader = loader_with_allowlist(
            &[(opaque, loadentry_path, outside.clone())],
            Some(configuration),
        );

        let resolved = loader
            .resolve(opaque, "clay:configuration", ResolutionKind::Import)
            .expect("allowlisted loadEntry loads even with a config root present");

        // Escaping relative imports (not validated loadEntries) stay confined.
        let escape_err = loader
            .resolve("../escape.js", "clay:configuration", ResolutionKind::Import)
            .expect_err("escaping import must be denied by config-root confinement");
        assert!(
            escape_err.to_string().contains("configuration directory"),
            "config-root confinement must reject escaping imports, got: {escape_err:?}"
        );

        // And the allowlisted entry still returns its on-disk source alongside.
        let source = match loader.load(&resolved, None, default_load_options()) {
            ModuleLoadResponse::Sync(Ok(source)) => source,
            ModuleLoadResponse::Sync(Err(error)) => panic!("load failed: {error:?}"),
            _ => panic!("expected sync response, got async"),
        };
        assert!(
            std::str::from_utf8(source.code.as_bytes())
                .unwrap()
                .contains("ok = true"),
            "allowlisted loadEntry must load alongside config-root confinement"
        );
    }

    #[test]
    fn clay_module_loader_denies_arbitrary_file_url_or_https_specifier() {
        // `file://`, `https://`, `http://`, bare, and scheme-bearing specifiers
        // that are not curated facades or allowlisted loadEntries stay denied.
        let loader = loader_with_allowlist(&[], None);
        for specifier in [
            "file:///etc/passwd",
            "https://example.com/evil.js",
            "http://example.com/x.js",
            "react",
            "node:fs",
            "npm:lodash",
        ] {
            let error = loader
                .resolve(specifier, "clay://runtime/main.js", ResolutionKind::Import)
                .expect_err("non-allowlisted specifier must be denied");
            assert!(
                error.to_string().contains("clay.runtime.invalid_import"),
                "specifier `{specifier}` must be denied, got: {error:?}"
            );
        }
    }

    #[test]
    fn clay_module_loader_denies_load_entry_imports_outside_package_root() {
        // Phase 18.6 task 7 security boundary: a validated first-party loadEntry
        // may import its own sibling modules (e.g. `./index.js`) — those are
        // confined to the validated package root by `resolve_relative`. But an
        // import that ESCAPES the package root (e.g. `../escape.js` landing
        // outside it) must be denied so a package cannot read arbitrary files
        // outside its validated root. This is the transitive-load confinement
        // gate added in task 5.
        let outside = config_fixture("pkg-escape-root");
        let package_root = outside.join("pkg");
        let dist = package_root.join("dist");
        fs::create_dir_all(&dist).unwrap();
        let load_entry = dist.join("load.js");
        fs::write(&load_entry, "// loadEntry").unwrap();
        // A legitimate sibling inside the package root.
        let sibling = dist.join("index.js");
        fs::write(&sibling, "// sibling").unwrap();
        // An escape file OUTSIDE the package root (in the fixture parent).
        let escape = outside.join("escape.js");
        fs::write(&escape, "// secret").unwrap();

        let opaque = "clay://packages/@clay/example/dist/load.js";
        let allowlist = Arc::new(FirstPartyLoadEntryAllowlist::default());
        allowlist.record(
            opaque,
            load_entry.canonicalize().unwrap(),
            package_root.canonicalize().unwrap(),
        );

        // Legitimate sibling import inside the package root resolves.
        let ok = allowlist.resolve_relative(opaque, "./index.js");
        assert!(
            ok.is_some(),
            "a sibling import inside the validated package root must resolve"
        );
        // An import that escapes the package root is denied (returns None).
        assert_eq!(
            allowlist.resolve_relative(opaque, "../escape.js"),
            None,
            "an import escaping the validated package root must be denied"
        );
        // A deep escape attempt is also denied.
        assert_eq!(
            allowlist.resolve_relative(opaque, "../../escape.js"),
            None,
            "a deep-escape import must be denied"
        );
        // A relative import from an unknown referrer (not in the allowlist) is
        // denied — the confinement gate only fires for validated package modules.
        assert_eq!(
            allowlist.resolve_relative("clay://packages/@clay/unknown/dist/x.js", "./y.js"),
            None,
            "a relative import from a non-validated referrer must be denied"
        );
    }

    #[tokio::test]
    async fn load_package_resolves_and_activates_first_party_markdown_end_to_end() {
        // The one-line default end-user path: a configuration module that does
        // `await loadPackage("@clay/markdown")` activates the package — the
        // loadEntry imports curated clay:* facades and registers its mode,
        // commands, and parse handler under Clay's authority.
        let root = config_fixture("loadpackage-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            const summary = await loadPackage("@clay/markdown");
            Deno.core.ops.op_clay_runtime_record(
              `loaded:${summary.name}:modes:${summary.modes.join(",")}`
            );
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage('@clay/markdown') must succeed end-to-end");

        // The resolver summary reaches the caller.
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "loaded:@clay/markdown:modes:markdown"),
            "loadPackage must return the typed summary with name + modes, got {:?}",
            result.op_records
        );
        // The loadEntry's default activation registered a parse handler.
        assert!(
            !result.parse_handlers.is_empty(),
            "loadPackage must activate the markdown parse handler, got none"
        );
        // Modes/commands/keymaps surfaced through the behavior manifest.
        assert!(
            result.behavior_manifest.is_some(),
            "loadPackage must register mode/commands/keymaps into the behavior manifest"
        );
    }

    #[tokio::test]
    async fn load_package_rejects_non_string_specifier() {
        // The facade validates the specifier type before touching the op,
        // mirroring bindKey/serverLoadPackage validation.
        for invalid in ["loadPackage(123)", "loadPackage()", "loadPackage(null)"] {
            let root = config_fixture("loadpackage-invalid");
            fs::write(
                root.join("init.js"),
                format!(
                    r#"
                    import {{ loadPackage }} from "clay:packages";
                    try {{
                      await {invalid};
                      Deno.core.ops.op_clay_runtime_record("no-throw");
                    }} catch (error) {{
                      Deno.core.ops.op_clay_runtime_record(String(error));
                    }}
                    "#
                ),
            )
            .unwrap();
            let result = ClayJsRuntimeService::default()
                .load_configuration_from_root(root)
                .await
                .expect("the invalid-specifier facade call must not crash the runtime");
            assert!(
                result
                    .op_records
                    .iter()
                    .any(|record| record.contains("clay.packages.invalid_specifier")),
                "`{invalid}` must throw clay.packages.invalid_specifier, got {:?}",
                result.op_records
            );
            assert!(
                !result.op_records.iter().any(|record| record == "no-throw"),
                "`{invalid}` must throw, not return normally"
            );
        }
    }

    #[tokio::test]
    async fn markdown_optional_preview_is_valid_panel_contribution() {
        // Phase 20 task 4: the optional Markdown preview helper registers a
        // valid clay:ui PanelContribution (hidden right slot, toggle action
        // target, package provenance) — but ONLY when called explicitly. The
        // default load path never invokes it (guarded separately by the
        // `load_package_markdown_default_activates_full_mode_from_init_js`
        // test, which asserts no panel contribution is published by default).
        let root = config_fixture("markdown-optional-preview-panel");
        // load.js imports the `clay:ui` facade and `markdownPackageManifest`
        // from index.js, so the dist module graph must be copied.
        for file_name in ["index.js", "load.js"] {
            fs::write(
                root.join(file_name),
                fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                    .expect("first-party Markdown runtime module must exist"),
            )
            .unwrap();
        }
        fs::write(
            root.join("init.js"),
            r#"
            import * as commands from "clay:commands";
            import * as decorations from "clay:decorations";
            import * as modes from "clay:modes";
            import * as packages from "clay:packages";
            import * as parse from "clay:parse";
            import { loadMarkdownPackage, registerMarkdownPreview } from "./load.js";

            // Realistic opt-in order: load the package first (registers the
            // markdown.togglePreview command), THEN publish the optional panel.
            const clay = { commands, decorations, modes, packages, parse };
            await loadMarkdownPackage(clay, { documentId: 1, path: "sample.md" });
            const panel = registerMarkdownPreview();
            Deno.core.ops.op_clay_runtime_record(`${panel.id}:${panel.slot}:${panel.defaultVisibility}`);
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("registerMarkdownPreview must succeed");

        // The returned declaration reached the caller with the contract shape.
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "markdown.preview:right:hidden"),
            "registerMarkdownPreview must return the hidden right-slot panel, got {:?}",
            result.op_records
        );
        // The server-side PackageUiRegistry validated and recorded it with
        // package provenance.
        let panel = result
            .ui_contributions
            .panels
            .iter()
            .find(|panel| panel.id == "markdown.preview")
            .expect("the optional preview must register as a validated PanelContribution");
        assert_eq!(panel.slot, "right");
        assert_eq!(panel.default_visibility, "hidden");
        assert_eq!(panel.provenance.api_prefix, "markdown");
        assert!(
            panel
                .action_targets
                .iter()
                .any(|target| target == "markdown.togglePreview"),
            "preview panel must target the toggle command, got {:?}",
            panel.action_targets
        );
    }

    #[tokio::test]
    async fn load_package_markdown_default_activates_full_mode_from_init_js() {
        // Phase 18.6 task 6: the one-line default end-user path activates the
        // FULL markdown setup (parse handler + commands + mode) from a genuinely
        // minimal init.js — no inline manifest, no per-primitive registration,
        // no manual clay facade plumbing in user config.
        let root = config_fixture("loadpackage-default");
        let init_js = r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#;
        fs::write(root.join("init.js"), init_js).unwrap();

        // The user config carries no manifest object and no per-primitive
        // registration calls — loadPackage does all of it.
        for forbidden in [
            "contributions",
            "modePattern",
            "serverRegisterCommand",
            "serverRegisterParseHandler",
            "serverActivateMajorMode",
            "markdownPackageManifest",
        ] {
            assert!(
                !init_js.contains(forbidden),
                "default init.js must not carry `{forbidden}` — loadPackage owns activation"
            );
        }

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage('@clay/markdown') default must succeed");

        // The markdown parse handler registered (mode_id `markdown`).
        assert!(
            result
                .parse_handlers
                .iter()
                .any(|handler| handler.mode_id == "markdown"),
            "default load must register the markdown parse handler, got {:?}",
            result.parse_handlers
        );
        // The markdown commands surfaced into the behavior manifest.
        let manifest = result
            .behavior_manifest
            .as_ref()
            .expect("default load must activate the major mode into the behavior manifest");
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == "markdown.togglePreview"),
            "default load must register the markdown.togglePreview command, got {:?}",
            manifest
                .commands
                .iter()
                .map(|c| &c.command_id)
                .collect::<Vec<_>>()
        );
        // The markdown keymap surfaced into the behavior manifest (distinct from
        // any Ctrl+O file-open binding, which loadPackage must NOT install).
        assert!(
            manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "markdown.togglePreview"),
            "default load must register the markdown togglePreview keymap, got {:?}",
            manifest
                .keymaps
                .iter()
                .map(|k| &k.command_id)
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn default_loading_preserves_explicit_ctrl_o_separation() {
        // Phase 18.6 task 6: loadPackage must NOT install the Ctrl+O file-open
        // binding. That binding stays a separate explicit bindKey call so the
        // package never owns a global file-open key. This test verifies both
        // halves: loadPackage alone installs no clientOpenFileDialog keymap, and
        // adding the documented separate bindKey call does install it.
        let root = config_fixture("loadpackage-no-ctrlo");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#,
        )
        .unwrap();
        let without_binding = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage-only config must load");
        let manifest = without_binding
            .behavior_manifest
            .as_ref()
            .expect("loadPackage must still produce a behavior manifest");
        assert!(
            !manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "clay.documents.clientOpenFileDialog"),
            "loadPackage must NOT install the Ctrl+O file-open keymap; it stays a separate bindKey call, got {:?}",
            manifest
                .keymaps
                .iter()
                .map(|k| &k.command_id)
                .collect::<Vec<_>>()
        );

        // The documented default adds the Ctrl+O binding as a separate explicit
        // bindKey call after loadPackage, and it lands in the manifest.
        let root = config_fixture("loadpackage-with-ctrlo");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            import { bindKey } from "clay:keybindings";
            await loadPackage("@clay/markdown");
            bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
            "#,
        )
        .unwrap();
        let with_binding = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("loadPackage + bindKey config must load");
        let manifest = with_binding
            .behavior_manifest
            .as_ref()
            .expect("config with bindKey must produce a behavior manifest");
        assert!(
            manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == "clay.documents.clientOpenFileDialog"),
            "the separate bindKey call must install the Ctrl+O file-open keymap, got {:?}",
            manifest
                .keymaps
                .iter()
                .map(|k| &k.command_id)
                .collect::<Vec<_>>()
        );
    }
}
