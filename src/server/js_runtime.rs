use std::{
    error::Error,
    fmt,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use deno_core::{
    JsRuntime, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
    ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, ResolutionKind, RuntimeOptions,
    error::ModuleLoaderError, v8,
};
use deno_error::JsErrorBox;
use tokio::{sync::oneshot, task};

use crate::perf::budgets::{JS_RUNTIME_EVALUATION_TIMEOUT_MS, JS_RUNTIME_HEAP_LIMIT_BYTES};
use crate::perf::metrics::global_recorder;
use crate::protocol::{
    DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, IncrementalParseUpdate,
    ParseByteRange, ParseEditNotification, RuntimeDiagnostic,
};

use super::{
    configuration::{ConfigurationError, ConfigurationRuntime},
    ops::{ClayOpState, PackageLoadEntryAllowlist, init_runtime_extension},
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
        "clay:git" => Some(CLAY_FACADE_GIT),
        "clay:keybindings" => Some(CLAY_FACADE_KEYBINDINGS),
        "clay:behavior" => Some(CLAY_FACADE_BEHAVIOR),
        "clay:packages" => Some(CLAY_FACADE_PACKAGES),
        "clay:modes" => Some(CLAY_FACADE_MODES),
        "clay:commands" => Some(CLAY_FACADE_COMMANDS),
        "clay:decorations" => Some(CLAY_FACADE_DECORATIONS),
        "clay:parse" => Some(CLAY_FACADE_PARSE),
        "clay:syntax" => Some(CLAY_FACADE_SYNTAX),
        "clay:completion" => Some(CLAY_FACADE_COMPLETION),
        "clay:application" => Some(CLAY_FACADE_APPLICATION),
        "clay:editor" => Some(CLAY_FACADE_EDITOR),
        "clay:theme" => Some(CLAY_FACADE_THEME),
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
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export async function serverListWorkspaceRoots() { return parse(await ops.op_clay_workspace_list_roots()); }
export async function serverAddWorkspaceRoot(path) {
  return parse(await ops.op_clay_workspace_add_root(path)).workspaceRootId;
}
export async function serverDiscoverWorkspaceRootForPath(path) {
  return parse(await ops.op_clay_workspace_discover_root_for_path(path));
}
export async function serverListDirectory(options) {
  const request = {
    rootId: options?.rootId,
    relativePath: options?.relativePath ?? "",
    maxDepth: options?.maxDepth,
    maxEntries: options?.maxEntries,
  };
  return parse(await ops.op_clay_workspace_list_directory(JSON.stringify(request), options?.cancelTokenId));
}
export async function serverCreateListingCancelToken() {
  return await ops.op_clay_workspace_create_listing_cancel_token();
}
export async function serverCancelListing(tokenId) {
  return await ops.op_clay_workspace_cancel_listing(tokenId);
}
export function clientOpenFolderDialog() {
  return "clay.workspace.clientOpenFolderDialog";
}
"#;

const CLAY_FACADE_GIT: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export async function serverListGitStatuses() {
  return parse(await ops.op_clay_git_list_statuses());
}
export async function serverRefreshGitStatus(options) {
  return parse(await ops.op_clay_git_refresh_status(JSON.stringify(options ?? null)));
}
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
export function buildCodeEditingManifest(options) {
  const pairs = (options.pairs ?? [
    { open: "(", close: ")" },
    { open: "[", close: "]" },
    { open: "{", close: "}" },
    { open: '"', close: '"' },
    { open: "'", close: "'" }
  ]).filter((pair) => pair.open.length > 0 && pair.close.length > 0);
  const comments = [];
  if (options.lineComment && options.lineComment.length > 0) {
    comments.push({ linePrefix: options.lineComment, continuePrefix: `${options.lineComment} ` });
  }
  const electricCharacters = [];
  for (const character of options.electricOutdentCharacters ?? []) {
    if (character === "}") {
      electricCharacters.push({ trigger: character, effect: "outdent-one-level" });
    }
  }
  const autocompleteTriggers = [];
  for (const trigger of options.autocompleteTriggers ?? []) {
    if (trigger.length > 0) {
      autocompleteTriggers.push({ trigger });
    }
  }
  return {
    enter: { kind: "preserveLeadingWhitespace" },
    pairs,
    comments,
    tabSpaces: options.indentSize,
    electricCharacters,
    autocompleteTriggers
  };
}
"#;

const CLAY_FACADE_PACKAGES: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
// Per-runtime-generation cache. Hot reload invalidates it by swapping to a
// fresh ClayJsRuntimeService, not by mutating globals/module cache in place.
const loadedPackages = globalThis.__clayLoadedPackages ??= Object.create(null);
export function serverValidatePackageManifest(manifest) {
  return parse(ops.op_clay_packages_validate_manifest(JSON.stringify(manifest ?? null)));
}
export function serverValidatePackagePermissions(permissions) {
  return parse(ops.op_clay_packages_validate_permissions(JSON.stringify(permissions ?? null)));
}
export function serverLoadPackage(packageJson) {
  return parse(ops.op_clay_packages_load_package(JSON.stringify(packageJson ?? null)));
}
export function serverListFirstPartyPackageSpecifiers() {
  return parse(ops.op_clay_packages_list_first_party_specifiers()).specifiers;
}
/** Load and activate an installed, user-authorized package by specifier.
 *
 * One-line default end-user loader from ~/.config/clay/init.js for both
 * bundled and user-installed packages:
 * await loadPackage("@clay/markdown"), await loadPackage("@vendor/foo"),
 * or await loadPackage("github:user/repo"). init.js grants no capabilities
 * on its own; every powerful capability is a separate user-approved grant. */
export async function loadPackage(specifier) {
  if (typeof specifier !== "string") {
    throw new Error("clay.packages.invalid_specifier: loadPackage requires a string specifier");
  }
  if (loadedPackages[specifier]) {
    return loadedPackages[specifier];
  }
  const result = parse(ops.op_clay_packages_load_package_by_specifier(JSON.stringify({ specifier })));
  const loadEntry = await import(result.loadEntrySpecifier);
  if (typeof loadEntry.default === "function") {
    await loadEntry.default();
  }
  loadedPackages[specifier] = result;
  return result;
}
"#;

const CLAY_FACADE_MODES: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
const activationRegistry = globalThis.__clayModeActivations ??= Object.create(null);
const activationKey = (apiPrefix, modeId) => `${apiPrefix}:${modeId}`;
export function serverRegisterModePattern(packageManifest, declaration) {
  const result = parse(ops.op_clay_modes_register_pattern(JSON.stringify(packageManifest ?? null), JSON.stringify(declaration ?? null)));
  if (packageManifest?.clay?.apiPrefix && declaration?.modeId) {
    activationRegistry[activationKey(packageManifest.clay.apiPrefix, declaration.modeId)] = {
      packageManifest,
      editorRules: declaration.editorRules,
      commands: declaration.commands,
      keymaps: declaration.keymaps,
    };
  }
  return result;
}
export function serverClassifyDocument(input) {
  return parse(ops.op_clay_modes_classify_document(JSON.stringify(input ?? null)));
}
export function serverActivateMajorMode(packageManifest, input) {
  return parse(ops.op_clay_modes_activate_major_mode(JSON.stringify(packageManifest ?? null), JSON.stringify(input ?? null)));
}
export function serverActivateClassifiedMode(classification, input = {}) {
  const activation = activationRegistry[activationKey(classification?.apiPrefix, classification?.modeId)];
  if (!activation) {
    throw new Error("clay.modes.activation_failed: classified mode has no registered activation metadata");
  }
  return serverActivateMajorMode(activation.packageManifest, {
    ...input,
    documentId: classification.documentId,
    modeId: classification.modeId,
    editorRules: activation.editorRules,
    commands: activation.commands,
    keymaps: activation.keymaps,
  });
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
export async function serverExecuteCommand(commandId, args = {}, target = { global: {} }) {
  return parse(await ops.op_clay_commands_execute_command(JSON.stringify({
    commandId,
    arguments: args ?? {},
    target: target ?? { global: {} },
    expectedPermissions: [],
  })));
}
export async function serverOpenFile(args) {
  const result = await serverExecuteCommand("clay.workspace.openFile", args ?? {});
  if (result.status?.kind !== "workspace" || result.status?.action !== "opened") {
    throw new Error(`clay.commands.open_failed: expected opened status, got ${JSON.stringify(result.status)}`);
  }
  return { documentId: String(result.status.documentId), version: Number(result.status.version), path: String(result.status.path ?? "") };
}
export async function serverOpenDirectory(args) {
  const result = await serverExecuteCommand("clay.workspace.openDirectory", args ?? {});
  if (result.status?.kind !== "workspace" || result.status?.action !== "navigated") {
    throw new Error(`clay.commands.open_directory_failed: expected navigated status, got ${JSON.stringify(result.status)}`);
  }
  return { workspaceRootId: String(result.status.workspaceRootId), relativePath: String(result.status.relativePath ?? "") };
}
export async function serverRevealInTree(args) {
  const result = await serverExecuteCommand("clay.workspace.revealInTree", args ?? {});
  if (result.status?.kind !== "workspace" || result.status?.action !== "revealed") {
    throw new Error(`clay.commands.reveal_failed: expected revealed status, got ${JSON.stringify(result.status)}`);
  }
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
  for (const key of ["handler", "callback", "onParse", "function"]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(`clay.parse.invalid_handler: executable ${key} callbacks are not accepted by the public registration contract`);
    }
  }
  const { module, exportName = "default", ...opOptions } = options ?? {};
  const registration = parse(ops.op_clay_parse_register_parse_handler(JSON.stringify({ ...(opOptions ?? {}), runtimeBridge: module !== undefined })));
  if (module !== undefined) {
    const handler = module?.[exportName];
    if (typeof handler !== "function") {
      throw new Error(`clay.parse.invalid_handler: module export ${exportName} must be a function`);
    }
    globalThis.__clayParseHandlers ??= Object.create(null);
    globalThis.__clayParseHandlers[registration.token] = handler;
  }
  return registration;
}
"#;

const CLAY_FACADE_SYNTAX: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function setSyntaxEnginePreference(target, tier) {
  return parse(ops.op_clay_syntax_set_engine_preference(String(target ?? ""), String(tier ?? "")));
}
export function serverRegisterSyntaxGrammar(options) {
  for (const key of ["handler", "callback", "onParse", "function", "clientJavaScript", "nativeHandle", "rawOps"]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(`clay.syntax.invalid_grammar: executable or raw authority field ${key} is not accepted by the public registration contract`);
    }
  }
  return parse(ops.op_clay_syntax_register_syntax_grammar(JSON.stringify(options ?? null)));
}
"#;

const CLAY_FACADE_COMPLETION: &str = r#"
const ops = Deno.core.ops;
const parse = (json) => JSON.parse(json);
export function serverRegisterCompletionProvider(options) {
  for (const key of ["handler", "callback", "complete", "function", "clientJavaScript", "nativeHandle", "rawOps", "module"]) {
    if (Object.prototype.hasOwnProperty.call(options ?? {}, key)) {
      throw new Error(`clay.completion.invalid_provider: executable or raw authority field ${key} is not accepted by the public registration contract`);
    }
  }
  return parse(ops.op_clay_completion_register_completion_provider(JSON.stringify(options ?? null)));
}
export function serverListCompletionProvidersForTrigger(options) {
  const trigger = (options ?? {}).trigger;
  if (typeof trigger !== "string" || trigger.length === 0) {
    throw new Error("clay.completion.invalid_trigger: trigger must be a non-empty string");
  }
  return parse(ops.op_clay_completion_providers_for_trigger(trigger));
}
export function completionTriggerCharactersFromEditorRules(editorRules) {
  const triggers = editorRules?.autocompleteTriggers ?? [];
  const characters = [];
  for (const trigger of triggers) {
    const value = trigger?.trigger;
    if (typeof value === "string" && value.length > 0) {
      characters.push(value);
    }
  }
  return characters;
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
export function clientCopySelection() { return "clay.editor.clientCopySelection"; }
"#;

const CLAY_FACADE_THEME: &str = r#"
export function setTheme(options) {
  const specifier = typeof options === "string" ? options : options?.specifier;
  if (typeof specifier !== "string" || specifier.length === 0) {
    throw new Error("clay.theme.invalid_request: setTheme requires a theme specifier");
  }
  return JSON.parse(Deno.core.ops.op_clay_theme_set_theme(JSON.stringify({ specifier })));
}
"#;

/// Isolated server-side Clay JavaScript runtime boundary.
#[derive(Debug, Clone)]
pub(crate) struct ClayJsRuntimeService {
    evaluations: Arc<AtomicU64>,
    timeout: Duration,
    heap_limit_bytes: usize,
    poisoned: Arc<std::sync::atomic::AtomicBool>,
    worker: Arc<std::sync::Mutex<Arc<RuntimeWorker>>>,
}

impl Default for ClayJsRuntimeService {
    fn default() -> Self {
        Self::new(Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS))
    }
}

impl ClayJsRuntimeService {
    fn new(timeout: Duration) -> Self {
        Self::new_with_heap_limit(timeout, JS_RUNTIME_HEAP_LIMIT_BYTES)
    }

    fn new_with_heap_limit(timeout: Duration, heap_limit_bytes: usize) -> Self {
        Self {
            evaluations: Arc::new(AtomicU64::new(0)),
            timeout,
            heap_limit_bytes,
            poisoned: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            worker: Arc::new(std::sync::Mutex::new(start_runtime_worker(
                timeout,
                heap_limit_bytes,
            ))),
        }
    }

    /// Sets a custom evaluation timeout. The default is
    /// [`JS_RUNTIME_EVALUATION_TIMEOUT_MS`]; tests use a short timeout to
    /// exercise the termination path quickly.
    #[cfg(test)]
    pub(crate) fn with_timeout(timeout: Duration) -> Self {
        Self::new(timeout)
    }

    #[cfg(test)]
    pub(crate) fn with_timeout_and_heap_limit(timeout: Duration, heap_limit_bytes: usize) -> Self {
        Self::new_with_heap_limit(timeout, heap_limit_bytes)
    }

    /// Evaluates a controlled server-owned ES module on the persistent runtime worker.
    pub(crate) async fn evaluate_controlled_module(
        &self,
        source: impl Into<String> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ControlledSource(source.into()),
            None,
            1,
            "runtime.evaluate_controlled_module",
        )
        .await
    }

    pub(crate) async fn load_configuration_from_root(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            None,
            1,
            "runtime.load_configuration",
        )
        .await
    }

    pub(crate) async fn load_configuration_from_root_with_workspace(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        workspace: Arc<tokio::sync::Mutex<WorkspaceState>>,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            Some(workspace),
            1,
            "runtime.load_configuration_with_workspace",
        )
        .await
    }

    pub(crate) async fn load_configuration_from_root_for_document(
        &self,
        config_root: impl Into<PathBuf> + Send + 'static,
        runtime_document_id: crate::protocol::DocumentId,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        self.evaluate_entry(
            RuntimeEntry::ConfigurationRoot(config_root.into()),
            None,
            runtime_document_id,
            "runtime.load_configuration_for_document",
        )
        .await
    }

    async fn evaluate_entry(
        &self,
        entry: RuntimeEntry,
        workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
        runtime_document_id: crate::protocol::DocumentId,
        metric: &'static str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        if self.poisoned.swap(false, Ordering::Relaxed) {
            self.replace_worker();
        }
        let (response, receiver) = oneshot::channel();
        let command = RuntimeCommand::Evaluate {
            entry,
            workspace,
            runtime_document_id,
            metric,
            response,
        };
        if let Err(error) = self.worker().sender.send(command) {
            self.replace_worker();
            self.worker().sender.send(error.0).map_err(|_| {
                ClayRuntimeError::Runtime(
                    "persistent JavaScript runtime worker stopped".to_string(),
                )
            })?;
        }
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("persistent JavaScript runtime worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            self.poisoned.store(true, Ordering::Relaxed);
        } else if result.is_ok() {
            self.evaluations.fetch_add(1, Ordering::Relaxed);
        }
        result
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

    pub(crate) fn register_parse_handlers(
        &self,
        coordinator: &crate::server::parse_coordinator::ParseCoordinator,
        generation_id: u64,
        evaluation: &ClayRuntimeEvaluation,
    ) -> Result<
        Vec<crate::server::parse_coordinator::ParseHandlerMeta>,
        crate::server::parse_coordinator::ParseCoordinatorError,
    > {
        let mut registered = Vec::new();
        for registration in &evaluation.js_parse_handlers {
            match coordinator.register_handler_meta_for_generation(
                generation_id,
                registration.meta.clone(),
                JsParseHandler {
                    runtime: self.clone(),
                    registration: registration.clone(),
                },
            ) {
                Ok(meta) => registered.push(meta),
                Err(crate::server::parse_coordinator::ParseCoordinatorError::HandlerAlreadyRegistered { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(registered)
    }

    async fn invoke_parse_handler(
        &self,
        registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
        notification: ParseEditNotification,
    ) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
        if self.poisoned.load(Ordering::Relaxed) {
            return Err(ClayRuntimeError::Runtime(
                "persistent JavaScript runtime worker stopped".to_string(),
            ));
        }
        let (response, receiver) = oneshot::channel();
        self.worker()
            .sender
            .send(RuntimeCommand::Parse {
                registration,
                notification,
                response,
            })
            .map_err(|_| {
                ClayRuntimeError::Runtime(
                    "persistent JavaScript runtime worker stopped".to_string(),
                )
            })?;
        let result = receiver.await.map_err(|_| {
            ClayRuntimeError::Runtime("persistent JavaScript runtime worker stopped".to_string())
        })?;
        if matches!(
            result,
            Err(ClayRuntimeError::Timeout | ClayRuntimeError::HeapLimit)
        ) {
            self.poisoned.store(true, Ordering::Relaxed);
        }
        result
    }

    fn worker(&self) -> Arc<RuntimeWorker> {
        Arc::clone(
            &self
                .worker
                .lock()
                .expect("Clay runtime service worker mutex poisoned"),
        )
    }

    fn replace_worker(&self) {
        *self
            .worker
            .lock()
            .expect("Clay runtime service worker mutex poisoned") =
            start_runtime_worker(self.timeout, self.heap_limit_bytes);
    }

    #[cfg(test)]
    pub(crate) fn evaluation_count(&self) -> u64 {
        self.evaluations.load(Ordering::Relaxed)
    }
}

struct JsParseHandler {
    runtime: ClayJsRuntimeService,
    registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
}

impl crate::server::parse_coordinator::ParseHandler for JsParseHandler {
    fn parse(
        &self,
        notification: ParseEditNotification,
    ) -> crate::server::parse_coordinator::ParseHandlerFuture {
        let runtime = self.runtime.clone();
        let registration = self.registration.clone();
        Box::pin(async move {
            runtime
                .invoke_parse_handler(registration, notification)
                .await
                .map_err(|error| {
                    crate::server::parse_coordinator::ParseCoordinatorError::HandlerFailed(
                        error.to_string(),
                    )
                })
        })
    }
}

/// Result of one JavaScript evaluation returned across the Rust boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClayRuntimeEvaluation {
    pub(crate) op_records: Vec<String>,
    pub(crate) published_sdui_tree: Option<crate::protocol::SduiTree>,
    pub(crate) published_decoration_set: Option<crate::protocol::DecorationSet>,
    pub(crate) parse_handlers: Vec<crate::server::parse_coordinator::ParseHandlerMeta>,
    pub(crate) js_parse_handlers: Vec<crate::server::parse_coordinator::JsParseHandlerRegistration>,
    pub(crate) behavior_manifest: Option<crate::protocol::BehaviorManifest>,
    pub(crate) ui_contributions: crate::server::ui::PackageUiRegistrySnapshot,
    pub(crate) syntax_grammars: Vec<crate::server::syntax::SyntaxGrammarContribution>,
    pub(crate) completion_providers: Vec<crate::server::completion::CompletionProviderMeta>,
    /// Resolved active theme snapshot from `setTheme` (`clay:theme` facade). `None`
    /// when `init.js` did not select a theme (Clay default applies). Applied to
    /// the shared server slot at load/reload so the welcome handshake ships it.
    pub(crate) active_theme: Option<crate::protocol::ActiveTheme>,
}

#[derive(Debug)]
pub(crate) enum ClayRuntimeError {
    Configuration(ConfigurationError),
    InvalidMainSpecifier(String),
    Runtime(String),
    Timeout,
    HeapLimit,
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
            Self::Timeout => write!(
                formatter,
                "JavaScript runtime evaluation exceeded the configured timeout"
            ),
            Self::HeapLimit => write!(formatter, "JavaScript runtime exceeded the heap limit"),
            Self::Join(error) => write!(formatter, "JavaScript runtime task failed: {error}"),
        }
    }
}

impl Error for ClayRuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(error) => Some(error),
            Self::Join(error) => Some(error),
            Self::InvalidMainSpecifier(_) | Self::Runtime(_) | Self::Timeout | Self::HeapLimit => {
                None
            }
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
            Self::Timeout => RuntimeDiagnostic::error(
                "clay.runtime.timeout",
                "JavaScript runtime evaluation timed out and was terminated.",
            ),
            Self::HeapLimit => RuntimeDiagnostic::error(
                "clay.runtime.heap_limit",
                "JavaScript runtime exceeded its heap budget and was terminated.",
            ),
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

struct RuntimeWorker {
    sender: mpsc::Sender<RuntimeCommand>,
    join: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl fmt::Debug for RuntimeWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeWorker")
            .finish_non_exhaustive()
    }
}

impl Drop for RuntimeWorker {
    fn drop(&mut self) {
        let _ = self.sender.send(RuntimeCommand::Shutdown);
        if let Some(join) = self
            .join
            .lock()
            .expect("Clay runtime worker join mutex poisoned")
            .take()
        {
            let _ = join.join();
        }
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "runtime worker commands stay on a single internal channel; boxing parse payloads is unnecessary until profiling says otherwise"
)]
enum RuntimeCommand {
    Evaluate {
        entry: RuntimeEntry,
        workspace: Option<Arc<tokio::sync::Mutex<WorkspaceState>>>,
        runtime_document_id: crate::protocol::DocumentId,
        metric: &'static str,
        response: oneshot::Sender<Result<ClayRuntimeEvaluation, ClayRuntimeError>>,
    },
    Parse {
        registration: crate::server::parse_coordinator::JsParseHandlerRegistration,
        notification: ParseEditNotification,
        response: oneshot::Sender<Result<IncrementalParseUpdate, ClayRuntimeError>>,
    },
    Shutdown,
}

fn start_runtime_worker(timeout: Duration, heap_limit_bytes: usize) -> Arc<RuntimeWorker> {
    let (sender, receiver) = mpsc::channel();
    let join = std::thread::Builder::new()
        .name("clay-js-runtime".to_string())
        .spawn(move || run_runtime_worker(receiver, timeout, heap_limit_bytes))
        .expect("failed to spawn persistent JS runtime worker");
    Arc::new(RuntimeWorker {
        sender,
        join: std::sync::Mutex::new(Some(join)),
    })
}

fn run_runtime_worker(
    receiver: mpsc::Receiver<RuntimeCommand>,
    timeout: Duration,
    heap_limit_bytes: usize,
) {
    let default_workspace = Arc::new(tokio::sync::Mutex::new(WorkspaceState::new()));
    let op_state = Arc::new(ClayOpState::new_for_document(
        Arc::clone(&default_workspace),
        1,
    ));
    let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER)
        .expect("controlled runtime specifier must parse");
    let loader = Rc::new(ClayModuleLoader::new(
        main_specifier,
        None,
        None,
        op_state.load_entry_allowlist(),
    ));
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("persistent JS runtime tokio runtime must build");
    let (mut runtime, heap_limit_hit) =
        create_js_runtime(Arc::clone(&op_state), Rc::clone(&loader), heap_limit_bytes);
    let mut controlled_evaluation_id = 0_u64;
    let mut main_module_loaded = false;

    for command in receiver {
        match command {
            RuntimeCommand::Evaluate {
                entry,
                workspace,
                runtime_document_id,
                metric,
                response,
            } => {
                controlled_evaluation_id = controlled_evaluation_id.saturating_add(1);
                let result = prepare_runtime_entry(entry, controlled_evaluation_id).and_then(
                    |loaded_entry| {
                        op_state.set_runtime_context(
                            workspace.unwrap_or_else(|| Arc::clone(&default_workspace)),
                            runtime_document_id,
                        );
                        op_state.begin_evaluation();
                        heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
                        loader.set_entry(
                            loaded_entry.main_specifier.clone(),
                            loaded_entry.main_source.clone(),
                            loaded_entry.configuration.clone(),
                        );
                        let recorder = global_recorder();
                        let _scope = recorder.scope(metric);
                        let result = tokio_runtime.block_on(evaluate_loaded_module(
                            &mut runtime,
                            &op_state,
                            loaded_entry,
                            timeout,
                            !main_module_loaded,
                            &heap_limit_hit,
                        ));
                        main_module_loaded = true;
                        result
                    },
                );
                let timed_out = matches!(result, Err(ClayRuntimeError::Timeout));
                let heap_limited = matches!(result, Err(ClayRuntimeError::HeapLimit));
                let _ = response.send(result);
                if timed_out || heap_limited {
                    break;
                }
            }
            RuntimeCommand::Parse {
                registration,
                notification,
                response,
            } => {
                op_state.begin_evaluation();
                heap_limit_hit.store(false, std::sync::atomic::Ordering::Relaxed);
                let result = tokio_runtime.block_on(evaluate_js_parse_handler(
                    &mut runtime,
                    &op_state,
                    &loader,
                    &registration,
                    notification,
                    timeout.min(Duration::from_millis(registration.timeout_ms)),
                    &heap_limit_hit,
                ));
                let timed_out = matches!(result, Err(ClayRuntimeError::Timeout));
                let heap_limited = matches!(result, Err(ClayRuntimeError::HeapLimit));
                let _ = response.send(result);
                if timed_out || heap_limited {
                    break;
                }
            }
            RuntimeCommand::Shutdown => break,
        }
    }
}

fn create_js_runtime(
    op_state: Arc<ClayOpState>,
    loader: Rc<ClayModuleLoader>,
    heap_limit_bytes: usize,
) -> (JsRuntime, Arc<std::sync::atomic::AtomicBool>) {
    let create_params = v8::Isolate::create_params().heap_limits(0, heap_limit_bytes);
    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(loader),
        extensions: vec![init_runtime_extension()],
        create_params: Some(create_params),
        ..Default::default()
    });
    let heap_limit_hit = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callback_flag = Arc::clone(&heap_limit_hit);
    let terminate_handle = runtime.v8_isolate().thread_safe_handle();
    runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
        callback_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        terminate_handle.terminate_execution();
        current_limit.saturating_mul(2)
    });
    runtime.op_state().borrow_mut().put(op_state);
    (runtime, heap_limit_hit)
}

fn prepare_runtime_entry(
    entry: RuntimeEntry,
    controlled_evaluation_id: u64,
) -> Result<LoadedRuntimeEntry, ClayRuntimeError> {
    match entry {
        RuntimeEntry::ControlledSource(source) => Ok(LoadedRuntimeEntry {
            main_specifier: ModuleSpecifier::parse(&format!(
                "clay://runtime/main-{controlled_evaluation_id}.js"
            ))
            .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
            main_source: Some(source),
            configuration: None,
        }),
        RuntimeEntry::ConfigurationRoot(config_root) => {
            let configuration = Arc::new(
                ConfigurationRuntime::from_config_root(config_root)
                    .map_err(ClayRuntimeError::Configuration)?,
            );
            Ok(LoadedRuntimeEntry {
                main_specifier: configuration
                    .entry_specifier()
                    .map_err(ClayRuntimeError::Configuration)?,
                main_source: None,
                configuration: Some(configuration),
            })
        }
    }
}

async fn evaluate_loaded_module(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loaded_configuration: LoadedRuntimeEntry,
    timeout: Duration,
    use_main_module: bool,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
    if let Some(configuration) = &loaded_configuration.configuration {
        runtime
            .op_state()
            .borrow_mut()
            .put(Arc::clone(configuration));
    }
    let terminate_handle = runtime.v8_isolate().thread_safe_handle();
    let timer = TerminationTimer::start(timeout, terminate_handle);

    let evaluation_result: Result<ClayRuntimeEvaluation, ClayRuntimeError> = async {
        let module_id = if use_main_module {
            if let Some(source) = loaded_configuration.main_source {
                runtime
                    .load_main_es_module_from_code(&loaded_configuration.main_specifier, source)
                    .await
            } else {
                runtime
                    .load_main_es_module(&loaded_configuration.main_specifier)
                    .await
            }
        } else if let Some(source) = loaded_configuration.main_source {
            runtime
                .load_side_es_module_from_code(&loaded_configuration.main_specifier, source)
                .await
        } else {
            runtime
                .load_side_es_module(&loaded_configuration.main_specifier)
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
            js_parse_handlers: op_state.js_parse_handlers(),
            behavior_manifest: (behavior_manifest.behavior_version > 1)
                .then_some(behavior_manifest),
            ui_contributions: op_state.ui_contributions(),
            syntax_grammars: op_state.syntax_grammars(),
            completion_providers: op_state.completion_providers(),
            active_theme: op_state.active_theme(),
        })
    }
    .await;

    if timer.did_fire() {
        let _ = runtime.v8_isolate().cancel_terminate_execution();
        return Err(ClayRuntimeError::Timeout);
    }
    if heap_limit_hit.load(std::sync::atomic::Ordering::Relaxed) {
        let _ = runtime.v8_isolate().cancel_terminate_execution();
        return Err(ClayRuntimeError::HeapLimit);
    }
    evaluation_result
}

async fn evaluate_js_parse_handler(
    runtime: &mut JsRuntime,
    op_state: &Arc<ClayOpState>,
    loader: &Rc<ClayModuleLoader>,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    notification: ParseEditNotification,
    timeout: Duration,
    heap_limit_hit: &std::sync::atomic::AtomicBool,
) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
    let source = format!(
        r#"
const registry = globalThis.__clayParseHandlers ?? Object.create(null);
const handler = registry[{token:?}];
if (typeof handler !== "function") {{
  throw new Error("clay.parse.handler_missing: registered parse handler is unavailable");
}}
const notification = {notification};
const update = await handler(notification);
Deno.core.ops.op_clay_parse_store_update(JSON.stringify(update ?? null));
"#,
        token = registration.token,
        notification = parse_notification_json(&notification),
    );
    let loaded = LoadedRuntimeEntry {
        main_specifier: ModuleSpecifier::parse(&format!(
            "clay://runtime/parse-{}.js",
            registration.token.replace(':', "-")
        ))
        .map_err(|error| ClayRuntimeError::InvalidMainSpecifier(error.to_string()))?,
        main_source: Some(source),
        configuration: None,
    };
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(runtime, op_state, loaded, timeout, false, heap_limit_hit).await?;
    let update_json = op_state.take_parse_update_json().ok_or_else(|| {
        ClayRuntimeError::Runtime(
            "clay.parse.invalid_update: handler produced no update".to_string(),
        )
    })?;
    parse_update_json(&update_json, registration, notification)
}

fn parse_notification_json(notification: &ParseEditNotification) -> String {
    serde_json::json!({
        "documentId": notification.document_id,
        "documentVersion": notification.document_version,
        "behaviorVersion": notification.behavior_version,
        "packagePrefix": notification.package_prefix,
        "mode": notification.mode_id,
        "viewport": range_json(notification.viewport),
        "invalidatedRanges": notification.invalidated_ranges.iter().map(|range| range_json(*range)).collect::<Vec<_>>(),
        "parseWindows": notification.parse_windows.iter().map(|window| serde_json::json!({
            "documentId": window.document_id,
            "documentVersion": window.document_version,
            "packagePrefix": window.package_prefix,
            "mode": window.mode_id,
            "byteStart": window.byte_start,
            "byteEnd": window.byte_end,
            "baseLine": window.base_line,
            "text": window.text,
        })).collect::<Vec<_>>(),
        "memoryBudget": notification.memory_budget.map(|budget| serde_json::json!({
            "budgetBytes": budget.budget_bytes,
            "retainedBytes": budget.retained_bytes,
        })),
    })
    .to_string()
}

fn range_json(range: ParseByteRange) -> serde_json::Value {
    serde_json::json!({ "byteStart": range.start, "byteEnd": range.end })
}

fn parse_update_json(
    update_json: &str,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
    fallback: ParseEditNotification,
) -> Result<IncrementalParseUpdate, ClayRuntimeError> {
    let value: serde_json::Value = serde_json::from_str(update_json).map_err(|error| {
        ClayRuntimeError::Runtime(format!("clay.parse.invalid_update: {error}"))
    })?;
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("clay.parse.invalid_update: update must be an object".to_string())
    })?;
    let viewport = object
        .get("viewport")
        .and_then(parse_range_value)
        .unwrap_or(fallback.viewport);
    let spans = object
        .get("spans")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| span_from_value(value, registration))
                .collect()
        })
        .transpose()?;
    Ok(IncrementalParseUpdate {
        document_id: object
            .get("documentId")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.document_id),
        document_version: object
            .get("documentVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.document_version),
        behavior_version: object
            .get("behaviorVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(fallback.behavior_version),
        package_prefix: object
            .get("packagePrefix")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&registration.meta.package_prefix)
            .to_string(),
        mode_id: object
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&registration.meta.mode_id)
            .to_string(),
        parse_unit: registration.parse_unit,
        viewport,
        invalidated_ranges: fallback.invalidated_ranges,
        syntax_tree_delta: object
            .get("syntaxTreeDelta")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        decoration_update: spans.map(|spans| DecorationSet {
            document_id: fallback.document_id,
            document_version: fallback.document_version,
            viewport_byte_start: viewport.start,
            viewport_byte_end: viewport.end,
            spans,
        }),
    })
}

fn parse_range_value(value: &serde_json::Value) -> Option<ParseByteRange> {
    let object = value.as_object()?;
    Some(ParseByteRange::new(
        object
            .get("byteStart")
            .or_else(|| object.get("start"))?
            .as_u64()?,
        object
            .get("byteEnd")
            .or_else(|| object.get("end"))?
            .as_u64()?,
    ))
}

fn span_from_value(
    value: &serde_json::Value,
    registration: &crate::server::parse_coordinator::JsParseHandlerRegistration,
) -> Result<DecorationSpan, ClayRuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        ClayRuntimeError::Runtime("clay.parse.invalid_update: span must be an object".to_string())
    })?;
    let kind = match object
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("syntax")
    {
        "syntax" | "Syntax" => DecorationKind::Syntax,
        "semantic" | "Semantic" => DecorationKind::Semantic,
        "diagnostic" | "Diagnostic" => DecorationKind::Diagnostic,
        "search-match" | "searchMatch" | "SearchMatch" => DecorationKind::SearchMatch,
        other => {
            return Err(ClayRuntimeError::Runtime(format!(
                "clay.parse.invalid_update: unsupported decoration kind `{other}`"
            )));
        }
    };
    let style_token = object
        .get("styleToken")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("markup.plain");
    Ok(DecorationSpan::from_style_token(
        object
            .get("byteStart")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        object
            .get("byteEnd")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        kind,
        style_token,
        object
            .get("priority")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u16,
        DecorationProvenance {
            package_name: registration.package.manifest.name.clone(),
            package_version: registration.package.manifest.version.clone(),
            package_prefix: registration.package.manifest.clay.api_prefix.clone(),
        },
    ))
}

/// Watchdog that terminates a V8 isolate when an evaluation exceeds a budget.
///
/// Spawns a lightweight OS thread that sleeps in 10 ms ticks until either the
/// timeout elapses (then calls `terminate_execution`) or [`did_fire`] cancels
/// it. `did_fire` is called on the happy path after evaluation completes and
/// atomically reports whether the watchdog already fired.
///
/// ponytail: one thread per evaluation. Ceiling: evaluations are infrequent
/// (startup config load, per-document loadEntry) so a polling thread per
/// evaluation is cheap; if evaluation frequency rises, switch to a shared
/// timer wheel or `tokio::time::timeout` on a `LocalSet`-spawned task.
struct TerminationTimer {
    fired: Arc<std::sync::atomic::AtomicBool>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl TerminationTimer {
    fn start(timeout: Duration, handle: deno_core::v8::IsolateHandle) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};

        let fired = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        let (fired_clone, cancel_clone) = (Arc::clone(&fired), Arc::clone(&cancel));
        let join = std::thread::Builder::new()
            .name("clay-js-runtime-timeout".to_string())
            .spawn(move || {
                let start = std::time::Instant::now();
                while start.elapsed() < timeout {
                    if cancel_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                // Timeout elapsed before cancellation: terminate the isolate so
                // the blocked evaluation returns control.
                fired_clone.store(true, Ordering::Relaxed);
                handle.terminate_execution();
            })
            .expect("failed to spawn JS runtime timeout watchdog thread");
        Self {
            fired,
            cancel,
            join: Some(join),
        }
    }

    /// Cancels the watchdog and returns whether it had already fired.
    fn did_fire(mut self) -> bool {
        use std::sync::atomic::Ordering;

        self.cancel.store(true, Ordering::Relaxed);
        let fired = self.fired.load(Ordering::Relaxed);
        // Detach rather than join: the thread observes `cancel` and exits within
        // a 10 ms tick. Joining is safe (terminate is non-blocking) but detaching
        // keeps the happy path off any thread-synchronization latency.
        self.join.take();
        fired
    }
}

struct LoadedRuntimeEntry {
    main_specifier: ModuleSpecifier,
    main_source: Option<String>,
    configuration: Option<Arc<ConfigurationRuntime>>,
}

#[derive(Debug)]
struct ClayModuleLoader {
    state: std::sync::Mutex<ClayModuleLoaderState>,
    // Shared validated package loadEntry gate. Populated by
    // `op_clay_packages_load_package_by_specifier`, checked in resolve/load.
    // Ceiling: one entry per loaded package module.
    package_load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
}

#[derive(Debug)]
struct ClayModuleLoaderState {
    main_specifier: ModuleSpecifier,
    main_source: Option<String>,
    configuration: Option<Arc<ConfigurationRuntime>>,
}

impl ClayModuleLoader {
    fn new(
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
        package_load_entry_allowlist: Arc<PackageLoadEntryAllowlist>,
    ) -> Self {
        Self {
            state: std::sync::Mutex::new(ClayModuleLoaderState {
                main_specifier,
                main_source,
                configuration,
            }),
            package_load_entry_allowlist,
        }
    }

    fn set_entry(
        &self,
        main_specifier: ModuleSpecifier,
        main_source: Option<String>,
        configuration: Option<Arc<ConfigurationRuntime>>,
    ) {
        *self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned") = ClayModuleLoaderState {
            main_specifier,
            main_source,
            configuration,
        };
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
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
        let state = self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned");
        if specifier == state.main_specifier.as_str() {
            return Ok(state.main_specifier.clone());
        }
        if clay_facade_source(specifier).is_some() {
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        if specifier == "markdown-it" {
            return ModuleSpecifier::parse(MARKDOWN_IT_MODULE_SPECIFIER)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        // Validated package `loadEntry`: opaque `clay://packages/...`
        // specifiers recorded by `op_clay_packages_load_package_by_specifier`.
        // This branch sits BEFORE the config-root branch because
        // `reject_non_local_specifier` would otherwise deny `clay://` URLs; the
        // shared allowlist is the single gate, so only a package module the
        // resolver op validated and recorded ever resolves here. Every other
        // `clay://packages/...` URL falls through to config-root confinement
        // (which rejects non-local specifiers) and the deny fallback.
        if self
            .package_load_entry_allowlist
            .absolute_path(specifier)
            .is_some()
        {
            return ModuleSpecifier::parse(specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        // Transitive relative imports from a validated package loadEntry are
        // confined to the validated package root by the allowlist and recorded
        // on first resolution. This lets a loadEntry import its own sibling
        // modules (e.g. `./index.js`) without weakening the config-root
        // boundary for any non-package specifier. ponytail: ceiling is the
        // validated package root; `resolve_relative` denies anything escaping it.
        if (specifier.starts_with("./") || specifier.starts_with("../"))
            && let Some(new_specifier) = self
                .package_load_entry_allowlist
                .resolve_relative(referrer, specifier)
        {
            return ModuleSpecifier::parse(&new_specifier)
                .map_err(|error| Self::denied(&error.to_string()));
        }
        if let Some(configuration) = &state.configuration {
            return configuration
                .resolve_module(specifier, referrer)
                .map_err(|error| error.to_js_error());
        }

        Err(Self::denied(&format!("{specifier} from {referrer}")))
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleLoadReferrer>,
        _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
        let state = self
            .state
            .lock()
            .expect("Clay module loader state mutex poisoned");
        if module_specifier == &state.main_specifier
            && let Some(source) = &state.main_source
        {
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(source.clone().into()),
                module_specifier,
                None,
            )));
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
        // Validated package `loadEntry`: read the on-disk source the resolver op
        // recorded for this exact opaque specifier. Single gate, same allowlist
        // as `resolve`; no path outside the validated package root is ever read.
        if let Some(absolute_path) = self
            .package_load_entry_allowlist
            .absolute_path(module_specifier.as_str())
        {
            return ModuleLoadResponse::Sync(
                std::fs::read_to_string(&absolute_path)
                    .map_err(|error| {
                        Self::denied(&format!(
                            "package loadEntry {module_specifier} could not be loaded ({error})"
                        ))
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
        if let Some(configuration) = &state.configuration {
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
                    .map_err(|error| error.to_js_error()),
            );
        }

        ModuleLoadResponse::Sync(Err(Self::denied(module_specifier.as_str())))
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
        path::Path,
        path::PathBuf,
        rc::Rc,
        sync::Arc,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use tokio::sync::Mutex;

    use deno_core::{
        ModuleLoadOptions, ModuleLoadResponse, ModuleLoader, ModuleSpecifier, ModuleType,
        RequestedModuleType, ResolutionKind,
    };

    use super::{
        CONTROLLED_MAIN_SPECIFIER, ClayJsRuntimeService, ClayModuleLoader, ClayRuntimeError,
        ClayRuntimeEvaluation, PackageLoadEntryAllowlist, RuntimeEntry, create_js_runtime,
        evaluate_loaded_module, prepare_runtime_entry,
    };
    use crate::perf::budgets::{JS_RUNTIME_EVALUATION_TIMEOUT_MS, JS_RUNTIME_HEAP_LIMIT_BYTES};
    use crate::protocol::{
        BehaviorVersion, DiagnosticSeverity, ParseByteRange, ParseEditNotification, ParsePolicy,
        ParseWindowSnapshot,
    };
    use crate::server::configuration::ConfigurationRuntime;
    use crate::server::parse_coordinator::{ParseCoordinator, ParseScheduleRequest};
    use crate::server::workspace::WorkspaceState;

    fn init_git_repo(root: &Path) {
        git(root, ["init", "-b", "main"]);
        git(root, ["config", "user.email", "clay@example.invalid"]);
        git(root, ["config", "user.name", "Clay Test"]);
    }

    fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .status()
            .unwrap();
        assert!(status.success(), "git command failed: {args:?}");
    }

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
    async fn persistent_js_runtime_retains_global_state_between_evaluations() {
        let service = ClayJsRuntimeService::default();
        service
            .evaluate_controlled_module(r#"globalThis.__clayPersistentRuntime = 41;"#)
            .await
            .unwrap();
        let result = service
            .evaluate_controlled_module(
                r#"
                if (globalThis.__clayPersistentRuntime !== 41) {
                    throw new Error("persistent runtime state missing");
                }
                Deno.core.ops.op_clay_runtime_record("persistent");
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["persistent"]);
        assert_eq!(service.evaluation_count(), 2);
    }

    #[tokio::test]
    async fn js_parse_handler_bridge_runs_registered_markdown_handler() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                await loadPackage("@clay/markdown");
                "#,
            )
            .await
            .unwrap();
        assert_eq!(evaluation.js_parse_handlers.len(), 1);

        let coordinator = ParseCoordinator::new();
        service
            .register_parse_handlers(&coordinator, 1, &evaluation)
            .unwrap();

        let text = "# Title\n";
        let request = ParseScheduleRequest {
            document_id: 1,
            document_version: 1,
            behavior_version: 1 as BehaviorVersion,
            package_prefix: "markdown".to_string(),
            mode_id: "markdown".to_string(),
            viewport: ParseByteRange::new(0, text.len() as u64),
            invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        };
        let windows = vec![ParseWindowSnapshot {
            document_id: 1,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            mode_id: "markdown".to_string(),
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            text: text.to_string(),
        }];
        coordinator
            .schedule_parse_with_windows(
                request,
                windows,
                Some(ParsePolicy::new(
                    64 * 1024,
                    4 * 1024,
                    30 * 1024 * 1024,
                    5_000,
                )),
            )
            .unwrap();

        let update = tokio::time::timeout(Duration::from_secs(6), coordinator.next_update())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(update.package_prefix, "markdown");
        assert!(
            update
                .decoration_update
                .as_ref()
                .is_some_and(|set| !set.spans.is_empty()),
            "markdown parser produced syntax decorations"
        );
    }

    #[tokio::test]
    async fn parse_registration_rejects_executable_callbacks_and_missing_permissions() {
        let service = ClayJsRuntimeService::default();
        for source in [
            r#"
            import { serverRegisterParseHandler } from "clay:parse";
            serverRegisterParseHandler({
              packageName: "@clay/evil",
              packageVersion: "0.1.0",
              packagePrefix: "evil",
              permissions: ["parse-document"],
              mode: "evil",
              handler() {}
            });
            "#,
            r#"
            import { serverRegisterParseHandler } from "clay:parse";
            serverRegisterParseHandler({
              packageName: "@clay/no-parse",
              packageVersion: "0.1.0",
              packagePrefix: "noparse",
              permissions: [],
              mode: "noparse"
            });
            "#,
        ] {
            let error = service
                .evaluate_controlled_module(source)
                .await
                .unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("clay.parse.invalid_handler")
                    || message.contains("clay.packages.missing_permission"),
                "unexpected registration error: {message}"
            );
        }
    }

    #[tokio::test]
    async fn syntax_facade_registers_grammar_metadata_without_raw_ops() {
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterSyntaxGrammar } from "clay:syntax";
                const result = serverRegisterSyntaxGrammar({
                  packageName: "@clay/rust",
                  packageVersion: "0.1.0",
                  packagePrefix: "rust",
                  permissions: ["parse-document", "render-decorations"],
                  syntaxGrammar: {
                    languageId: "rust",
                    filePatterns: { extensions: ["rs"] },
                    grammar: { kind: "tree-sitter-wasm", path: "./grammars/rust.wasm" },
                    queries: { highlights: "./queries/highlights.scm" },
                    styleMap: {
                      keyword: "keyword.control",
                      string: "string.quoted",
                      comment: "comment.line",
                      punctuation: "punctuation.definition"
                    },
                    budgets: { timeoutMs: 5000, maxWindowBytes: 4096 }
                  }
                });
                Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.languages[0]}:${result.registeredGrammarCount}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(evaluation.op_records, vec!["rust:rust:0"]);
        assert!(evaluation.syntax_grammars.iter().any(|grammar| {
            grammar.language_id == "rust"
                && grammar.engine_tier == crate::server::syntax::SyntaxEngineTier::Native
        }));
    }

    #[tokio::test]
    async fn syntax_facade_engine_preference_allows_explicit_wasm_override() {
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { setSyntaxEnginePreference, serverRegisterSyntaxGrammar } from "clay:syntax";
                setSyntaxEnginePreference("rust", "wasm");
                const result = serverRegisterSyntaxGrammar({
                  packageName: "@clay/rust",
                  packageVersion: "0.1.0",
                  packagePrefix: "rust",
                  permissions: ["parse-document", "render-decorations"],
                  syntaxGrammar: {
                    languageId: "rust",
                    filePatterns: { extensions: ["rs"] },
                    grammar: { kind: "tree-sitter-wasm", path: "./grammars/rust.wasm" },
                    queries: { highlights: "./queries/highlights.scm" },
                    styleMap: { keyword: "keyword.control" }
                  }
                });
                Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.registeredGrammarCount}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(evaluation.op_records, vec!["rust:1"]);
        assert!(evaluation.syntax_grammars.iter().any(|grammar| {
            grammar.language_id == "rust"
                && grammar.engine_tier == crate::server::syntax::SyntaxEngineTier::Wasm
        }));
    }

    #[tokio::test]
    async fn syntax_facade_rejects_raw_authority_and_third_party_grammars() {
        let service = ClayJsRuntimeService::default();
        for source in [
            r#"
            import { serverRegisterSyntaxGrammar } from "clay:syntax";
            serverRegisterSyntaxGrammar({
              packageName: "@clay/rust",
              packagePrefix: "rust",
              permissions: ["parse-document", "render-decorations"],
              rawOps: true
            });
            "#,
            r#"
            import { serverRegisterSyntaxGrammar } from "clay:syntax";
            serverRegisterSyntaxGrammar({
              packageName: "@vendor/rust",
              packagePrefix: "vendor-rust",
              permissions: ["parse-document", "render-decorations"],
              syntaxGrammar: {
                languageId: "rust",
                filePatterns: { extensions: ["rs"] },
                grammar: { kind: "tree-sitter-wasm", path: "./grammars/rust.wasm" },
                queries: { highlights: "./queries/highlights.scm" },
                styleMap: { keyword: "keyword.control" }
              }
            });
            "#,
        ] {
            let error = service
                .evaluate_controlled_module(source)
                .await
                .unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("clay.syntax.invalid_grammar")
                    || message.contains("first-party-only"),
                "unexpected syntax registration error: {message}"
            );
        }
    }

    #[tokio::test]
    async fn completion_facade_registers_provider_metadata_without_raw_ops() {
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                const result = serverRegisterCompletionProvider({
                  packageName: "@vendor/words",
                  packageVersion: "0.1.0",
                  packagePrefix: "words",
                  permissions: ["completion-provider"],
                  providerId: "words.buffer",
                  triggerCharacters: ["."],
                  wordBoundaryChars: [".", ","],
                  priority: 2,
                  timeoutMs: 50,
                  maxItems: 20
                });
                Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.providers[0]}:${result.registeredProviderCount}:${result.runtimeBridge}`);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(evaluation.op_records, vec!["words:words.buffer:1:false"]);
        assert_eq!(evaluation.completion_providers.len(), 1);
        assert_eq!(evaluation.completion_providers[0].id, "words.buffer");
        assert_eq!(
            evaluation.completion_providers[0].provenance.package_prefix,
            "words"
        );
    }

    #[tokio::test]
    async fn completion_facade_rejects_callbacks_missing_permission_and_bad_prefix() {
        let service = ClayJsRuntimeService::default();
        for source in [
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              packageName: "@vendor/evil",
              packagePrefix: "evil",
              permissions: ["completion-provider"],
              providerId: "evil.words",
              handler() {}
            });
            "#,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              packageName: "@vendor/nope",
              packagePrefix: "nope",
              permissions: [],
              providerId: "nope.words"
            });
            "#,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              packageName: "@vendor/bad",
              packagePrefix: "bad",
              permissions: ["completion-provider"],
              providerId: "other.words"
            });
            "#,
        ] {
            let error = service
                .evaluate_controlled_module(source)
                .await
                .unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("clay.completion.invalid_provider")
                    || message.contains("completion-provider"),
                "unexpected completion registration error: {message}"
            );
        }
    }

    #[tokio::test]
    async fn language_package_completion_trigger_metadata_is_queryable() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverListCompletionProvidersForTrigger } from "clay:completion";

                await loadPackage("@clay/rust");
                await loadPackage("@clay/typescript");
                await loadPackage("@clay/javascript");

                const dotProviders = serverListCompletionProvidersForTrigger({ trigger: "." });
                const rustScopeProviders = serverListCompletionProvidersForTrigger({ trigger: "::" });
                const noProviders = serverListCompletionProvidersForTrigger({ trigger: "?" });

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                  dotIds: dotProviders.providers.map((p) => p.id).sort(),
                  rustScopeIds: rustScopeProviders.providers.map((p) => p.id).sort(),
                  noCount: noProviders.providers.length,
                  rustTriggerCharacters: dotProviders.providers.find((p) => p.id === "rust.keywords")?.triggerCharacters ?? [],
                  typescriptTriggerCharacters: dotProviders.providers.find((p) => p.id === "typescript.keywords")?.triggerCharacters ?? [],
                }));
                "#,
            )
            .await
            .unwrap();

        let record = evaluation
            .op_records
            .into_iter()
            .next()
            .expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(
            parsed["dotIds"],
            serde_json::json!([
                "javascript.keywords",
                "rust.keywords",
                "typescript.keywords"
            ])
        );
        assert_eq!(parsed["rustScopeIds"], serde_json::json!(["rust.keywords"]));
        assert_eq!(parsed["noCount"], 0);
        assert_eq!(
            parsed["rustTriggerCharacters"],
            serde_json::json!([".", "::"])
        );
        assert_eq!(
            parsed["typescriptTriggerCharacters"],
            serde_json::json!(["."])
        );
    }

    #[tokio::test]
    async fn load_package_registers_first_party_syntax_grammars() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                await loadPackage("@clay/rust");
                await loadPackage("@clay/typescript");
                await loadPackage("@clay/javascript");
                "#,
            )
            .await
            .unwrap();

        let languages = evaluation
            .syntax_grammars
            .iter()
            .map(|grammar| (grammar.language_id.as_str(), grammar.engine_tier))
            .collect::<Vec<_>>();
        assert_eq!(
            languages,
            vec![
                (
                    "javascript",
                    crate::server::syntax::SyntaxEngineTier::Native
                ),
                ("markdown", crate::server::syntax::SyntaxEngineTier::Native),
                ("rust", crate::server::syntax::SyntaxEngineTier::Native),
                ("tsx", crate::server::syntax::SyntaxEngineTier::Native),
                (
                    "typescript",
                    crate::server::syntax::SyntaxEngineTier::Native
                ),
            ]
        );
    }

    #[tokio::test]
    async fn js_parse_handler_timeout_uses_registered_budget() {
        let service = ClayJsRuntimeService::default();
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { serverRegisterParseHandler } from "clay:parse";
                const parser = { parse() { while (true) {} } };
                serverRegisterParseHandler({
                  packageName: "@clay/loop",
                  packageVersion: "0.1.0",
                  packagePrefix: "loop",
                  permissions: ["parse-document"],
                  mode: "loop",
                  parseUnit: "line-group",
                  timeoutMs: 50,
                  module: parser,
                  exportName: "parse"
                });
                "#,
            )
            .await
            .expect("malicious handler registration itself should be bounded metadata work");
        let registration = evaluation
            .js_parse_handlers
            .first()
            .expect("handler registered")
            .clone();
        let notification = ParseEditNotification {
            document_id: 1,
            document_version: 1,
            behavior_version: 1,
            package_prefix: "loop".to_string(),
            mode_id: "loop".to_string(),
            viewport: ParseByteRange::new(0, 4),
            invalidated_ranges: vec![ParseByteRange::new(0, 4)],
            parse_windows: Vec::new(),
            memory_budget: None,
        };
        let started = std::time::Instant::now();
        let error = service
            .invoke_parse_handler(registration, notification)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Timeout));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "registered handler timeout budget should beat global 5s guard"
        );
        assert_eq!(error.diagnostic().code, "clay.runtime.timeout");
    }

    #[tokio::test]
    async fn runtime_boundary_does_not_expose_platform_authorities() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                const exposed = [
                  ["fetch", typeof fetch],
                  ["WebSocket", typeof WebSocket],
                  ["Worker", typeof Worker],
                  ["process", typeof process],
                  ["require", typeof require],
                  ["Deno.readTextFile", typeof Deno.readTextFile],
                  ["Deno.Command", typeof Deno.Command],
                ].filter(([, type]) => type !== "undefined");
                Deno.core.ops.op_clay_runtime_record(JSON.stringify(exposed));
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["[]"]);
    }

    #[tokio::test]
    async fn js_runtime_infinite_loop_is_terminated_with_timeout() {
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
        let start = std::time::Instant::now();
        let error = service
            .evaluate_controlled_module(r#"while (true) {}"#)
            .await
            .unwrap_err();
        let elapsed = start.elapsed();

        assert!(
            matches!(error, ClayRuntimeError::Timeout),
            "expected Timeout, got {error:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1000),
            "timeout test should finish quickly, took {elapsed:?}"
        );
        assert_eq!(
            error.diagnostic().code,
            "clay.runtime.timeout",
            "timeout should surface the clay.runtime.timeout diagnostic"
        );
        // Timed-out evaluations are not counted as successful completions.
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn js_runtime_timeout_recovery_uses_fresh_worker() {
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
        let error = service
            .evaluate_controlled_module(
                r#"
                globalThis.__clayRecoveryMarker = "stale";
                while (true) {}
                "#,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ClayRuntimeError::Timeout));

        let result = service
            .evaluate_controlled_module(
                r#"
                Deno.core.ops.op_clay_runtime_record(typeof globalThis.__clayRecoveryMarker);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["undefined"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic() {
        let service = ClayJsRuntimeService::with_timeout_and_heap_limit(
            Duration::from_secs(3),
            8 * 1024 * 1024,
        );
        let error = service
            .evaluate_controlled_module(
                r#"
                const values = [];
                while (true) {
                  values.push({ text: "Hello", number: values.length });
                }
                "#,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(error, ClayRuntimeError::HeapLimit),
            "expected heap limit, got {error:?}"
        );
        assert_eq!(error.diagnostic().code, "clay.runtime.heap_limit");
        assert_eq!(service.evaluation_count(), 0);
    }

    #[tokio::test]
    async fn js_runtime_heap_limit_recovery_uses_fresh_worker() {
        let service = ClayJsRuntimeService::with_timeout_and_heap_limit(
            Duration::from_secs(3),
            8 * 1024 * 1024,
        );
        let error = service
            .evaluate_controlled_module(
                r#"
                globalThis.__clayRecoveryMarker = "stale";
                const values = [];
                while (true) {
                  values.push({ text: "Hello", number: values.length });
                }
                "#,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ClayRuntimeError::HeapLimit));

        let result = service
            .evaluate_controlled_module(
                r#"
                Deno.core.ops.op_clay_runtime_record(typeof globalThis.__clayRecoveryMarker);
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["undefined"]);
        assert_eq!(service.evaluation_count(), 1);
    }

    #[tokio::test]
    async fn js_runtime_short_timeout_does_not_break_fast_evaluation() {
        let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
        let result = service
            .evaluate_controlled_module(
                r#"
                Deno.core.ops.op_clay_runtime_record("fast");
                "#,
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["fast"]);
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
    async fn language_packages_config_fixture_loads_and_registers_all_contributions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("language-packages");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        let provider_ids: Vec<_> = result
            .completion_providers
            .iter()
            .map(|provider| provider.id.clone())
            .collect();
        assert!(
            provider_ids.iter().any(|id| id == "rust.keywords"),
            "fixture must register rust.keywords completion provider"
        );
        assert!(
            provider_ids.iter().any(|id| id == "typescript.keywords"),
            "fixture must register typescript.keywords completion provider"
        );
        assert!(
            provider_ids.iter().any(|id| id == "javascript.keywords"),
            "fixture must register javascript.keywords completion provider"
        );

        let component_ids: Vec<_> = result
            .ui_contributions
            .components
            .iter()
            .map(|component| component.id.clone())
            .collect();
        assert!(
            component_ids.iter().any(|id| id == "rust.status.mode"),
            "fixture must register rust.status.mode status item"
        );
        assert!(
            component_ids
                .iter()
                .any(|id| id == "typescript.status.mode"),
            "fixture must register typescript.status.mode status item"
        );
        assert!(
            component_ids
                .iter()
                .any(|id| id == "javascript.status.mode"),
            "fixture must register javascript.status.mode status item"
        );

        let grammar_ids: Vec<_> = result
            .syntax_grammars
            .iter()
            .map(|grammar| grammar.language_id.clone())
            .collect();
        assert!(
            grammar_ids.iter().any(|id| id == "rust"),
            "fixture must register rust syntax grammar"
        );
        assert!(
            grammar_ids.iter().any(|id| id == "typescript"),
            "fixture must register typescript syntax grammar"
        );
        assert!(
            grammar_ids.iter().any(|id| id == "javascript"),
            "fixture must register javascript syntax grammar"
        );
    }

    #[tokio::test]
    async fn file_browser_workflow_config_fixture_loads_packages_and_bindings() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("configuration")
            .join("file-browser-workflow");

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("fixture must publish configured keybindings");

        for provider_id in [
            "rust.keywords",
            "typescript.keywords",
            "javascript.keywords",
        ] {
            assert!(
                result
                    .completion_providers
                    .iter()
                    .any(|provider| provider.id == provider_id),
                "fixture must load completion provider {provider_id}"
            );
        }
        for command_id in [
            "clay.workspace.clientOpenFolderDialog",
            "clay.workspace.openFuzzyFile",
            "clay.workspace.toggleFileBrowser",
            "clay.editor.clientCopySelection",
        ] {
            assert!(
                manifest
                    .keymaps
                    .iter()
                    .any(|rule| rule.command_id == command_id),
                "fixture must bind {command_id}"
            );
        }
        for command_id in [
            "clay.workspace.clientOpenFolderDialog",
            "clay.editor.clientCopySelection",
        ] {
            assert!(manifest.commands.iter().any(|command| {
                command.command_id == command_id
                    && command.authority == crate::protocol::CommandAuthority::ClientUi
            }));
        }
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
    async fn git_facade_lists_refreshes_and_commands_statuses() {
        let config_root = config_fixture("git-facade");
        let repo_root = config_root.join("repo");
        let plain_root = config_root.join("plain");
        fs::create_dir(&repo_root).unwrap();
        fs::create_dir(&plain_root).unwrap();
        init_git_repo(&repo_root);
        fs::write(repo_root.join("tracked.txt"), "base").unwrap();
        git(&repo_root, ["add", "."]);
        git(&repo_root, ["commit", "-m", "initial"]);
        fs::write(repo_root.join("tracked.txt"), "changed").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { serverListGitStatuses, serverRefreshGitStatus } from "clay:git";
            import { serverExecuteCommand } from "clay:commands";
            const cold = await serverListGitStatuses();
            const repo = await serverRefreshGitStatus({ workspaceRootId: cold[0].workspaceRootId });
            const plain = await serverRefreshGitStatus({ workspaceRootId: cold[1].workspaceRootId });
            const listed = await serverExecuteCommand("clay.git.listStatuses");
            const refreshed = await serverExecuteCommand("clay.git.refreshStatus", { workspaceRootId: cold[0].workspaceRootId });
            Deno.core.ops.op_clay_runtime_record(`${cold.length}:${cold[0].refreshState.kind}:${repo.snapshot.head.kind}:${repo.snapshot.dirty}:${plain.snapshot.lastRefresh.kind}:${listed.status.kind}:${listed.status.statuses.length}:${refreshed.status.action}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&repo_root).unwrap();
        workspace.add_root(&plain_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec!["2:idle:branch:true:non-repository:git:2:refreshed"]
        );
    }

    #[tokio::test]
    async fn git_package_loads_and_publishes_read_only_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let config_root = config_fixture("git-package-load");
        let repo_root = config_root.join("repo");
        let plain_root = config_root.join("plain");
        fs::create_dir(&repo_root).unwrap();
        fs::create_dir(&plain_root).unwrap();
        init_git_repo(&repo_root);
        fs::write(repo_root.join("tracked.txt"), "base").unwrap();
        git(&repo_root, ["add", "."]);
        git(&repo_root, ["commit", "-m", "initial"]);
        fs::write(repo_root.join("tracked.txt"), "changed").unwrap();
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            import { serverListGitStatuses, serverRefreshGitStatus } from "clay:git";

            // Warm the cache first so the package status panel renders branch state.
            const cold = await serverListGitStatuses();
            await serverRefreshGitStatus({ workspaceRootId: cold[0].workspaceRootId });
            // `loadPackage("@clay/git")` runs the load entry, which publishes a
            // read-only status tree from cached clay:git data. No throw => the
            // status data path works against a repo + plain root.
            const summary = await loadPackage("@clay/git");
            const warm = await serverListGitStatuses();
            Deno.core.ops.op_clay_runtime_record(`${summary.name}:${summary.apiPrefix}:${summary.permissions.length}:${summary.contributions.sdui}:${warm.length}:${warm[0].snapshot.head.kind}:${warm[0].snapshot.dirty}`);
            "#,
        )
        .unwrap();
        let mut workspace = WorkspaceState::new();
        workspace.add_root(&repo_root).unwrap();
        workspace.add_root(&plain_root).unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        assert_eq!(result.op_records, vec!["@clay/git:git:0:1:2:branch:true"]);
    }

    #[tokio::test]
    async fn git_package_declares_no_mutation_or_network_authority() {
        // Phase 18.13: prove @clay/git is read-only. It declares no permissions
        // (no network/shell/filesystem/mutation), registers no package commands,
        // and exposes no configuration/package options (fixed safe defaults).
        // Mutating Git operations and config knobs are intentionally out of scope.
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let config_root = config_fixture("git-package-authority");
        fs::write(
            config_root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";

            const summary = await loadPackage("@clay/git");
            const perms = summary.permissions.join(",");
            const mutating = ["filesystem", "network", "shell", "wasm", "ai-tools",
              "workspace-mutation", "native-ui", "client-runtime", "raw-ops",
              "package-control", "package-import"];
            const leaked = mutating.filter((m) => perms.includes(m)).join(",");
            Deno.core.ops.op_clay_runtime_record(
              `${perms.length}:${summary.contributions.commands}:` +
              `${summary.contributions.configuration}:${summary.contributions.packageOptions}:${leaked}`
            );
            "#,
        )
        .unwrap();
        let workspace = WorkspaceState::new();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root_with_workspace(
                config_root,
                Arc::new(Mutex::new(workspace)),
            )
            .await
            .unwrap();

        // perms:commands:configuration:packageOptions:leaked — all zero/empty
        assert_eq!(result.op_records, vec!["0:0:0:0:"]);
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
    async fn configuration_binds_client_ui_file_folder_and_copy_commands() {
        let service = ClayJsRuntimeService::default();
        let result = service
            .evaluate_controlled_module(
                r#"
                import { bindKey, listKeyBindings } from "clay:keybindings";
                import { listBehaviorRoutes } from "clay:behavior";
                import { clientOpenFolderDialog } from "clay:workspace";
                import { clientCopySelection } from "clay:editor";
                const file = bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
                const folder = bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
                const copy = bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
                const bindings = listKeyBindings("editor");
                const routes = await listBehaviorRoutes();
                const fileRoute = routes.find((candidate) => candidate.apiId === "clay.documents.clientOpenFileDialog");
                const folderRoute = routes.find((candidate) => candidate.apiId === "clay.workspace.clientOpenFolderDialog");
                const copyRoute = routes.find((candidate) => candidate.apiId === "clay.editor.clientCopySelection");
                Deno.core.ops.op_clay_runtime_record(`${file.key}:${file.command}:${folder.key}:${folder.command}:${copy.key}:${copy.command}:${bindings.length}:${fileRoute.runtimePath}:${fileRoute.authority}:${folderRoute.runtimePath}:${folderRoute.authority}:${copyRoute.runtimePath}:${copyRoute.authority}`);
                "#,
            )
            .await
            .unwrap();
        let manifest = result
            .behavior_manifest
            .expect("published behavior manifest");

        assert_eq!(
            result.op_records,
            vec![
                "Ctrl+O:clay.documents.clientOpenFileDialog:Ctrl+Shift+O:clay.workspace.clientOpenFolderDialog:Ctrl+Shift+C:clay.editor.clientCopySelection:5:client-ui-command:client-ui:client-ui-command:client-ui:client-ui-command:client-ui"
            ]
        );
        for command_id in [
            "clay.documents.clientOpenFileDialog",
            "clay.workspace.clientOpenFolderDialog",
            "clay.editor.clientCopySelection",
        ] {
            assert!(manifest.keymaps.iter().any(|rule| {
                rule.command_id == command_id
                    && rule.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
            }));
            assert!(manifest.commands.iter().any(|command| {
                command.command_id == command_id
                    && command.authority == crate::protocol::CommandAuthority::ClientUi
            }));
        }
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
    async fn raw_clipboard_and_dialog_command_bindings_are_rejected() {
        for command_id in [
            "clay.clipboard.writeText",
            "clay.dialog.openRawPath",
            "Deno.core.ops.op_clipboard_write",
        ] {
            let source = format!(
                r#"
                import {{ bindKey }} from "clay:keybindings";
                bindKey("Ctrl+Alt+C", {command_id:?});
                "#
            );
            let error = ClayJsRuntimeService::default()
                .evaluate_controlled_module(source)
                .await
                .unwrap_err();

            assert!(matches!(error, ClayRuntimeError::Runtime(_)));
            assert!(
                error
                    .to_string()
                    .contains("clay.keybindings.unknown_command"),
                "{command_id} must stay rejected: {error}"
            );
        }
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
    async fn syntax_grammar_packages_default_load_from_init_js() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let root = config_fixture("syntax-grammar-init-load");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";

            const loaded = [];
            for (const specifier of ["@clay/rust", "@clay/typescript", "@clay/javascript"]) {
              const summary = await loadPackage(specifier);
              loaded.push(`${summary.name}:${summary.apiPrefix}:${summary.modes.length}:${summary.permissions.join("+")}:${summary.contributions.syntaxGrammars}`);
            }
            Deno.core.ops.op_clay_runtime_record(loaded.join("|"));
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap();

        assert_eq!(
            result.op_records,
            vec![
                "@clay/rust:rust:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:1|@clay/typescript:typescript:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:1|@clay/javascript:javascript:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:1"
            ]
        );
    }

    #[tokio::test]
    async fn rust_package_expansion_registers_mode_command_completion_and_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";

                const summary = await loadPackage("@clay/rust");
                const classification = serverClassifyDocument({ documentId: 42, path: "src/main.rs" });
                const commands = serverListCommands();
                const rustCommand = commands.find((command) => command.commandId === "rust.toggleLineComment");

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    apiPrefix: summary.apiPrefix,
                    modes: summary.modes,
                    commands: summary.contributions.commands,
                    uiComponents: summary.contributions.uiComponents,
                    classification,
                    rustCommandRegistered: Boolean(rustCommand)
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["apiPrefix"], "rust");
        assert_eq!(parsed["modes"], serde_json::json!(["rust"]));
        assert_eq!(parsed["classification"]["modeId"], "rust");
        assert_eq!(parsed["classification"]["apiPrefix"], "rust");
        assert!(parsed["rustCommandRegistered"].as_bool().unwrap());
        assert_eq!(parsed["commands"], 1);
        assert_eq!(parsed["uiComponents"], 1);
    }

    #[tokio::test]
    async fn typescript_package_expansion_registers_mode_command_completion_and_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";

                const summary = await loadPackage("@clay/typescript");
                const classification = serverClassifyDocument({ documentId: 42, path: "src/index.ts" });
                const commands = serverListCommands();
                const tsCommand = commands.find((command) => command.commandId === "typescript.toggleLineComment");

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    apiPrefix: summary.apiPrefix,
                    modes: summary.modes,
                    commands: summary.contributions.commands,
                    uiComponents: summary.contributions.uiComponents,
                    classification,
                    tsCommandRegistered: Boolean(tsCommand)
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["apiPrefix"], "typescript");
        assert_eq!(parsed["modes"], serde_json::json!(["typescript"]));
        assert_eq!(parsed["classification"]["modeId"], "typescript");
        assert_eq!(parsed["classification"]["apiPrefix"], "typescript");
        assert!(parsed["tsCommandRegistered"].as_bool().unwrap());
        assert_eq!(parsed["commands"], 1);
        assert_eq!(parsed["uiComponents"], 1);
    }

    #[tokio::test]
    async fn javascript_package_expansion_registers_mode_command_completion_and_status() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";

                const summary = await loadPackage("@clay/javascript");
                const classification = serverClassifyDocument({ documentId: 42, path: "src/index.js" });
                const commands = serverListCommands();
                const jsCommand = commands.find((command) => command.commandId === "javascript.toggleLineComment");

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                    apiPrefix: summary.apiPrefix,
                    modes: summary.modes,
                    commands: summary.contributions.commands,
                    uiComponents: summary.contributions.uiComponents,
                    classification,
                    jsCommandRegistered: Boolean(jsCommand)
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["apiPrefix"], "javascript");
        assert_eq!(parsed["modes"], serde_json::json!(["javascript"]));
        assert_eq!(parsed["classification"]["modeId"], "javascript");
        assert_eq!(parsed["classification"]["apiPrefix"], "javascript");
        assert!(parsed["jsCommandRegistered"].as_bool().unwrap());
        assert_eq!(parsed["commands"], 1);
        assert_eq!(parsed["uiComponents"], 1);
    }

    #[tokio::test]
    async fn build_code_editing_manifest_produces_valid_editor_rules() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { buildCodeEditingManifest } from "clay:behavior";
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";

                // @clay/javascript now uses buildCodeEditingManifest for its editor rules.
                // Loading the package exercises the manifest validator; classifying a
                // matching document proves the mode pattern (built from helper output)
                // was registered successfully.
                const summary = await loadPackage("@clay/javascript");
                const classification = serverClassifyDocument({ documentId: 7, path: "src/index.js" });

                const rules = buildCodeEditingManifest({
                  indentSize: 4,
                  lineComment: "//",
                  pairs: [{ open: "(", close: ")" }],
                  electricOutdentCharacters: ["}"],
                  autocompleteTriggers: [".", "::"]
                });

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                  modeId: classification.modeId,
                  apiPrefix: classification.apiPrefix,
                  packageName: classification.packageName,
                  packageVersion: classification.packageVersion,
                  summaryModes: summary.modes,
                  rulesEnterKind: rules.enter.kind,
                  rulesTabSpaces: rules.tabSpaces,
                  rulesPairCount: rules.pairs.length,
                  rulesCommentCount: rules.comments.length,
                  rulesElectricCount: rules.electricCharacters.length,
                  rulesAutocompleteCount: rules.autocompleteTriggers.length
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["modeId"], "javascript");
        assert_eq!(parsed["apiPrefix"], "javascript");
        assert_eq!(parsed["packageName"], "@clay/javascript");
        assert_eq!(parsed["packageVersion"], "0.1.0");
        assert_eq!(parsed["rulesEnterKind"], "preserveLeadingWhitespace");
        assert_eq!(parsed["rulesTabSpaces"], 4);
        assert_eq!(parsed["rulesPairCount"], 1);
        assert_eq!(parsed["rulesCommentCount"], 1);
        assert_eq!(parsed["rulesElectricCount"], 1);
        assert_eq!(parsed["rulesAutocompleteCount"], 2);
    }

    #[tokio::test]
    async fn language_packages_classify_with_core_fallbacks_and_no_conflicts() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { loadPackage } from "clay:packages";
                import { serverClassifyDocument } from "clay:modes";
                import { serverListCommands } from "clay:commands";
                import { serverListCompletionProvidersForTrigger } from "clay:completion";

                await loadPackage("@clay/rust");
                await loadPackage("@clay/typescript");
                await loadPackage("@clay/javascript");

                const classifications = {
                  rust: serverClassifyDocument({ documentId: 1, path: "src/main.rs" }),
                  typescript: serverClassifyDocument({ documentId: 2, path: "src/index.ts" }),
                  javascript: serverClassifyDocument({ documentId: 3, path: "src/index.js" }),
                  plainText: serverClassifyDocument({ documentId: 4, path: "README.txt" }),
                  unknownCode: serverClassifyDocument({ documentId: 5, path: "prog.py" }),
                };

                const commands = serverListCommands();
                const commandIds = commands.map((command) => command.commandId).sort();
                const dotProviders = serverListCompletionProvidersForTrigger({ trigger: "." });
                const providerIds = dotProviders.providers.map((provider) => provider.id).sort();

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                  classifications,
                  commandIds,
                  providerIds,
                  commandCount: commands.length,
                  providerCount: dotProviders.providers.length
                }));
                "#,
            )
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");

        // Package-declared modes win over core.code for known extensions.
        assert_eq!(parsed["classifications"]["rust"]["modeId"], "rust");
        assert_eq!(
            parsed["classifications"]["typescript"]["modeId"],
            "typescript"
        );
        assert_eq!(
            parsed["classifications"]["javascript"]["modeId"],
            "javascript"
        );

        // Plain text falls back to core.text; unmatched code-like extension falls back to core.code.
        assert_eq!(
            parsed["classifications"]["plainText"]["modeId"],
            "core.text"
        );
        assert_eq!(
            parsed["classifications"]["unknownCode"]["modeId"],
            "core.code"
        );

        // No duplicate command or provider IDs across packages.
        assert_eq!(parsed["commandCount"], 3);
        assert_eq!(
            parsed["commandIds"],
            serde_json::json!([
                "javascript.toggleLineComment",
                "rust.toggleLineComment",
                "typescript.toggleLineComment"
            ])
        );
        assert_eq!(parsed["providerCount"], 3);
        assert_eq!(
            parsed["providerIds"],
            serde_json::json!([
                "javascript.keywords",
                "rust.keywords",
                "typescript.keywords"
            ])
        );
    }

    #[tokio::test]
    async fn language_package_classification_is_deterministic_across_load_orders() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;

        for (first, second, third) in [
            ("@clay/rust", "@clay/typescript", "@clay/javascript"),
            ("@clay/javascript", "@clay/rust", "@clay/typescript"),
            ("@clay/typescript", "@clay/javascript", "@clay/rust"),
        ] {
            let source = format!(
                r#"
                import {{ loadPackage }} from "clay:packages";
                import {{ serverClassifyDocument }} from "clay:modes";

                await loadPackage("{}");
                await loadPackage("{}");
                await loadPackage("{}");

                const rust = serverClassifyDocument({{ documentId: 10, path: "lib.rs" }});
                const ts = serverClassifyDocument({{ documentId: 11, path: "app.ts" }});
                const js = serverClassifyDocument({{ documentId: 12, path: "app.js" }});

                Deno.core.ops.op_clay_runtime_record(JSON.stringify({{
                  rust: rust.modeId,
                  typescript: ts.modeId,
                  javascript: js.modeId
                }}));
                "#,
                first, second, third
            );
            let result = ClayJsRuntimeService::default()
                .evaluate_controlled_module(source)
                .await
                .unwrap();

            let record = result.op_records.into_iter().next().expect("one record");
            let parsed: serde_json::Value =
                serde_json::from_str(&record).expect("valid JSON record");
            assert_eq!(parsed["rust"], "rust");
            assert_eq!(parsed["typescript"], "typescript");
            assert_eq!(parsed["javascript"], "javascript");
        }
    }

    #[tokio::test]
    async fn language_package_rejects_unauthorized_completion_provider() {
        let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module(
                r#"
                import { serverRegisterCompletionProvider } from "clay:completion";

                serverRegisterCompletionProvider({
                  packageName: "@evil/lang",
                  packageVersion: "0.0.0",
                  packagePrefix: "evil",
                  permissions: ["parse-document"],
                  providerId: "evil.keywords",
                  triggerCharacters: ["."]
                });
                "#,
            )
            .await
            .unwrap_err();

        let message = error.to_string();
        assert!(
            message.contains("completion-provider"),
            "expected missing completion-provider permission error, got: {message}"
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

    fn loadable_package_fixture(name: &str, api_prefix: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "version": "0.1.0",
            "type": "module",
            "clay": {
                "apiPrefix": api_prefix,
                "entry": "./dist/index.js",
                "loadEntry": "./dist/load.js",
                "capabilities": [],
                "modes": [],
                "docs": "./docs/index.md",
                "apiDependencies": [],
                "performance": {
                    "estimatedManifestBytes": 256,
                    "hotPathPolicy": "no hot-path JS on keypress/paint"
                },
                "contributions": {}
            }
        })
    }

    fn write_loadable_package(root: &Path, load_source: &str) {
        fs::create_dir_all(root.join("dist")).expect("create package dist directory");
        fs::create_dir_all(root.join("docs")).expect("create package docs directory");
        fs::write(root.join("dist/index.js"), "export {};\n").expect("write package entry");
        fs::write(root.join("dist/load.js"), load_source).expect("write package loadEntry");
        fs::write(
            root.join("dist/helper.js"),
            "Deno.core.ops.op_clay_runtime_record(\"helper loaded\"); export {};\n",
        )
        .expect("write package helper");
        fs::write(root.join("docs/index.md"), "# Fixture\n").expect("write package docs");
    }

    async fn evaluate_with_seeded_package(
        specifier: &str,
        package_name: &str,
        api_prefix: &str,
        package_root: PathBuf,
        load_source: &str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        write_loadable_package(&package_root, load_source);
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::new(Mutex::new(WorkspaceState::new())),
            1,
        ));
        let package_json = loadable_package_fixture(package_name, api_prefix);
        {
            let mut service = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            service
                .install_from_value_at_root_with_spec(package_json, package_root, specifier)
                .expect("seed package install succeeds");
            service
                .authorize_package(
                    package_name,
                    Vec::new(),
                    crate::packages::authorization::RuntimeProfile::NativeTrust,
                    "test-user",
                )
                .expect("seed package authorization succeeds");
        }
        let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            main_specifier,
            None,
            None,
            op_state.load_entry_allowlist(),
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
        );
        let source = format!(
            r#"
            import {{ loadPackage }} from "clay:packages";
            await loadPackage({specifier:?});
            "#
        );
        let loaded = prepare_runtime_entry(RuntimeEntry::ControlledSource(source), 1).unwrap();
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
    }

    /// Plan 035 task 8: prove the one-line `init.js` default loads an installed,
    /// authorized, *user-installed* (non-`@clay/*`) package the same way it
    /// loads `@clay/markdown`. Mirrors [`evaluate_with_seeded_package`] but
    /// evaluates a real `~/.config/clay/init.js`-shaped config root instead of
    /// a controlled module source, so the loadEntry import + default-export
    /// invocation is exercised through the configuration runtime path.
    async fn evaluate_init_js_with_seeded_package(
        config_root: PathBuf,
        specifier: &str,
        package_name: &str,
        api_prefix: &str,
        package_root: PathBuf,
        load_source: &str,
    ) -> Result<ClayRuntimeEvaluation, ClayRuntimeError> {
        write_loadable_package(&package_root, load_source);
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::new(Mutex::new(WorkspaceState::new())),
            1,
        ));
        let package_json = loadable_package_fixture(package_name, api_prefix);
        {
            let mut service = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            service
                .install_from_value_at_root_with_spec(package_json, package_root, specifier)
                .expect("seed package install succeeds");
            service
                .authorize_package(
                    package_name,
                    Vec::new(),
                    crate::packages::authorization::RuntimeProfile::NativeTrust,
                    "test-user",
                )
                .expect("seed package authorization succeeds");
        }
        let loaded =
            prepare_runtime_entry(RuntimeEntry::ConfigurationRoot(config_root), 1).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
            op_state.load_entry_allowlist(),
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
        );
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
    }

    #[tokio::test]
    async fn load_package_user_installed_default_loads_from_init_js() {
        // Plan 035 task 8: the one-line end-user default loads an installed,
        // authorized, user-installed package from a genuine `init.js` config
        // root. No inline manifest, no per-primitive registration, and no
        // manual facade plumbing in user config — `loadPackage` owns all of it.
        let config_root = config_fixture("init-js-user-package");
        let package_root = config_root
            .join("node_modules")
            .join("@vendor")
            .join("mode");
        let init_js = r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("github:vendor/mode");
            "#;
        fs::write(config_root.join("init.js"), init_js).unwrap();

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
                "default init.js must not carry `{forbidden}` for a user-installed package"
            );
        }

        let result = evaluate_init_js_with_seeded_package(
            config_root.clone(),
            "github:vendor/mode",
            "@vendor/mode",
            "vendormode",
            package_root.clone(),
            r#"Deno.core.ops.op_clay_runtime_record("user-installed init.js load"); export default function load() {}"#,
        )
        .await
        .expect("one-line init.js load must succeed for installed user package");

        // The package loadEntry default export ran (it recorded an op), proving
        // activation went through the shared resolver + enable + authorize +
        // loadEntry import path from a real init.js file.
        assert_eq!(result.op_records, vec!["user-installed init.js load"]);
        let _ = fs::remove_dir_all(config_root);
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_rejects_uninstalled_specifier() {
        // Source-aware loading no longer categorically rejects npm/GitHub/local
        // shapes. They must still exist in the package service's installed and
        // authorized registry before runtime loading can proceed.
        for denied in [
            "left-pad",
            "github:user/mode",
            "./local-package",
            "../escape",
            "/absolute/package",
        ] {
            let err = resolve_by_specifier(denied).await.unwrap_err();
            assert!(
                err.contains("clay.packages.not_installed"),
                "uninstalled specifier `{denied}` must be not_installed, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn op_clay_packages_load_package_by_specifier_rejects_invalid_bundled_specifier() {
        for denied in [
            "@clay/",
            "@clay/../escape",
            "@clay/foo/bar",
            "@clay/markdown?tag=latest",
            "@clay/markdown#hash",
        ] {
            let err = resolve_by_specifier(denied).await.unwrap_err();
            assert!(
                err.contains("clay.packages.invalid_specifier"),
                "invalid bundled specifier `{denied}` must be invalid_specifier, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn load_package_loads_authorized_npm_style_fixture() {
        let root = config_fixture("npm-package-load")
            .join("node_modules")
            .join("left-pad");
        let result = evaluate_with_seeded_package(
            "left-pad",
            "left-pad",
            "leftpad",
            root.clone(),
            r#"Deno.core.ops.op_clay_runtime_record("npm fixture loaded"); export default function load() {}"#,
        )
        .await
        .expect("authorized npm-style package must load through shared package path");

        assert_eq!(result.op_records, vec!["npm fixture loaded"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_loads_authorized_github_requested_spec_fixture() {
        let root = config_fixture("github-package-load")
            .join("node_modules")
            .join("@vendor")
            .join("mode");
        let result = evaluate_with_seeded_package(
            "github:vendor/mode",
            "@vendor/mode",
            "vendormode",
            root.clone(),
            r#"import "./helper.js"; export default function load() {}"#,
        )
        .await
        .expect("authorized scoped package must load through shared package path");

        assert_eq!(result.op_records, vec!["helper loaded"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_completion_provider_fixture_registers_metadata() {
        let root = config_fixture("completion-provider-package-load").join("completion-provider");
        write_loadable_package(
            &root,
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            export default function load() {
              serverRegisterCompletionProvider({
                packageName: "completion-provider",
                packageVersion: "0.1.0",
                packagePrefix: "completionprovider",
                permissions: ["completion-provider"],
                providerId: "completionprovider.words",
                triggerCharacters: ["."],
                timeoutMs: 50,
                maxItems: 20
              });
            }
            "#,
        );
        let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
            Arc::new(Mutex::new(WorkspaceState::new())),
            1,
        ));
        let mut package_json =
            loadable_package_fixture("completion-provider", "completionprovider");
        package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
        {
            let mut service = op_state
                .package_service()
                .lock()
                .expect("package service mutex poisoned");
            service
                .install_from_value_at_root_with_spec(
                    package_json,
                    root.clone(),
                    "completion-provider",
                )
                .expect("seed completion package install succeeds");
            service
                .authorize_package(
                    "completion-provider",
                    vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                    crate::packages::authorization::RuntimeProfile::NativeTrust,
                    "test-user",
                )
                .expect("seed completion package authorization succeeds");
        }
        let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap();
        let loader = Rc::new(ClayModuleLoader::new(
            main_specifier,
            None,
            None,
            op_state.load_entry_allowlist(),
        ));
        let (mut runtime, heap_limit_hit) = create_js_runtime(
            Arc::clone(&op_state),
            Rc::clone(&loader),
            JS_RUNTIME_HEAP_LIMIT_BYTES,
        );
        let loaded = prepare_runtime_entry(
            RuntimeEntry::ControlledSource(
                r#"
                import { loadPackage } from "clay:packages";
                await loadPackage("completion-provider");
                "#
                .to_string(),
            ),
            1,
        )
        .unwrap();
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        let result = evaluate_loaded_module(
            &mut runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            true,
            &heap_limit_hit,
        )
        .await
        .expect("completion provider loadPackage path succeeds");

        assert_eq!(result.completion_providers.len(), 1);
        assert_eq!(
            result.completion_providers[0].id,
            "completionprovider.words"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_loads_authorized_local_requested_spec_fixture() {
        let root = config_fixture("local-package-load").join("local-package");
        let result = evaluate_with_seeded_package(
            "./local-package",
            "local-package",
            "localpackage",
            root.clone(),
            r#"Deno.core.ops.op_clay_runtime_record("local fixture loaded"); export default function load() {}"#,
        )
        .await
        .expect("authorized local package spec must load through shared package path");

        assert_eq!(result.op_records, vec!["local fixture loaded"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_package_rejects_escaping_relative_import_from_package_root() {
        let root = config_fixture("escaping-package-load").join("evil-mode");
        fs::create_dir_all(root.parent().unwrap()).expect("create parent fixture root");
        fs::write(root.parent().unwrap().join("escape.js"), "export {};\n")
            .expect("write outside escape module");
        let err = evaluate_with_seeded_package(
            "evil-mode",
            "evil-mode",
            "evilmode",
            root.clone(),
            r#"import "../escape.js"; export default function load() {}"#,
        )
        .await
        .unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains("clay.runtime.invalid_import"),
            "escaping relative import must fail at module loader boundary, got: {message}"
        );
        let _ = fs::remove_dir_all(root.parent().unwrap());
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
        let allowlist = Arc::new(PackageLoadEntryAllowlist::default());
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
    fn package_load_entry_allowlist_revokes_owned_entries() {
        let root = config_fixture("loader-revoke-package");
        let loadentry_path = root.join("load.js");
        let helper_path = root.join("helper.js");
        fs::write(&loadentry_path, "import './helper.js';\n").unwrap();
        fs::write(&helper_path, "export const helper = true;\n").unwrap();
        let allowlist = PackageLoadEntryAllowlist::default();
        let opaque = "clay://packages/@vendor/example/dist/load.js";
        let canonical_root = std::fs::canonicalize(&root).unwrap();
        let canonical_loadentry = std::fs::canonicalize(&loadentry_path).unwrap();
        allowlist.record_for_package(
            opaque,
            canonical_loadentry,
            canonical_root,
            Some("@vendor/example"),
        );
        let helper = allowlist
            .resolve_relative(opaque, "./helper.js")
            .expect("relative helper import is recorded with same owner");

        assert_eq!(allowlist.revoke_package("@vendor/example"), 2);
        assert!(allowlist.absolute_path(opaque).is_none());
        assert!(allowlist.absolute_path(&helper).is_none());
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
        // still rejected, while an allowlisted package loadEntry still loads.
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
        // Phase 18.6 task 7 security boundary: a validated package loadEntry
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
        let allowlist = Arc::new(PackageLoadEntryAllowlist::default());
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
    async fn set_theme_resolves_first_party_gruvbox_theme() {
        let root = config_fixture("set-theme-e2e");
        fs::write(
            root.join("init.js"),
            r#"
            import { setTheme } from "clay:theme";
            const summary = setTheme("@clay/theme-gruvbox-material-dark");
            Deno.core.ops.op_clay_runtime_record(
              `theme:${summary.specifier}:overrides:${summary.overrideCount}`
            );
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("setTheme('@clay/theme-gruvbox-material-dark') must succeed");

        let theme = result.active_theme.expect("active theme snapshot emitted");
        assert_eq!(theme.specifier, "@clay/theme-gruvbox-material-dark");
        assert_eq!(theme.overrides.len(), 45);
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "theme:@clay/theme-gruvbox-material-dark:overrides:45"),
            "setTheme summary must reach init.js"
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
    async fn load_package_is_idempotent_per_persistent_runtime() {
        let root = config_fixture("loadpackage-idempotent");
        fs::write(
            root.join("init.js"),
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            await loadPackage("@clay/markdown");
            Deno.core.ops.op_clay_runtime_record("loaded-once");
            "#,
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("repeated loadPackage calls must reuse the already-loaded package");

        assert!(
            result
                .op_records
                .iter()
                .any(|record| record == "loaded-once")
        );
        assert_eq!(result.js_parse_handlers.len(), 1);
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
