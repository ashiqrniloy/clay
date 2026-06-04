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
    ops::{ClayOpState, init_runtime_extension},
    workspace::WorkspaceState,
};

const CONTROLLED_MAIN_SPECIFIER: &str = "clay://runtime/main.js";

fn clay_facade_source(specifier: &str) -> Option<&'static str> {
    match specifier {
        "clay:configuration" => Some(CLAY_FACADE_CONFIGURATION),
        "clay:sdui" => Some(CLAY_FACADE_SDUI),
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

export function setPackageOption(options) { void options; unavailable("clay.configuration.setPackageOption"); }
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
            evaluate_module_on_runtime(RuntimeEntry::ControlledSource(source), None)
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
            evaluate_module_on_runtime(RuntimeEntry::ConfigurationRoot(config_root), None)
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
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    let op_state =
        Arc::new(ClayOpState::new(workspace.unwrap_or_else(|| {
            Arc::new(tokio::sync::Mutex::new(WorkspaceState::new()))
        })));
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
}

impl ClayModuleLoader {
    fn new(
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
    ) -> Self {
        Self {
            main_specifier,
            main_source,
            configuration,
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use tokio::sync::Mutex;

    use super::{ClayJsRuntimeService, ClayRuntimeError};
    use crate::protocol::DiagnosticSeverity;
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
    async fn markdown_config_fixture_opens_workspace_and_publishes_status_sdui() {
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

        let tree = result.published_sdui_tree.expect("published SDUI tree");
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Panel { title, .. } if title == "Markdown Preview"
        )));
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Label { text } if text == "Mode: markdown"
        )));
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Label { text } if text == "Parse: markdown-it registered"
        )));
        assert!(tree.nodes.iter().any(|node| matches!(
            &node.kind,
            crate::protocol::SduiNodeKind::Label { text } if text == "Decorations: published"
        )));
        let toggle_intent = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                crate::protocol::SduiNodeKind::Button { action, .. }
                    if action.command_id == "markdown.togglePreview" =>
                {
                    Some(action.clone())
                }
                _ => None,
            })
            .expect("markdown toggle action must be present");
        let mut sdui_state = crate::server::sdui::StaticSduiState::for_document(1, 1);
        sdui_state
            .replace_with_runtime_tree(tree)
            .expect("runtime Markdown SDUI tree must validate");
        sdui_state
            .validate_action(&toggle_intent)
            .expect("registered package command action must validate");
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
    async fn primitive_configuration_facades_remain_explicitly_planned() {
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { setPackageOption, setModePreference, setDecorationTheme, setParsePolicy } from "clay:configuration";
                if ([setPackageOption, setModePreference, setDecorationTheme, setParsePolicy].some((api) => typeof api !== "function")) {
                  throw new Error("configuration primitive facade export missing");
                }
                setPackageOption({ packagePrefix: "markdown", option: "preview", value: true });
                "#,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error
                .to_string()
                .contains("clay.configuration.setPackageOption is planned")
        );
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
}
