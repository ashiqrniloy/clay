//! Document family: edit/resync/decorations/open/save/reload/close/status/list,
//! selection queries, parse-window scheduling. Plan 090 task 2 extraction.

use std::sync::Arc;

use tokio::{io::AsyncWrite, sync::Mutex};

use crate::{
    perf::{
        budgets::{DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES, INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES},
        metrics::{MetricMetadata, SERVER_EDIT_ACK, global_recorder},
    },
    protocol::{
        ClientId, DocumentId, DocumentMetadata, DocumentVersion, ParseByteRange, ParseInputEdit,
        ParsePolicy, ParseWindowSnapshot, ProtocolErrorCode, RuntimeDiagnostic,
        SelectionQueryRange, SelectionQueryResult, ServerMessage, WorkspaceRootId,
        codec::{Codec, CodecError},
    },
};

use crate::server::connection::{file_operation_failed, teardown_closed_document};

/// Upper bound on parse windows scheduled per viewport request. A tall or
/// zoomed-out viewport is covered by consecutive bounded windows instead of a
/// single clamped one; anything past this waits for the next scroll request.
// One job per request: the native handler parses a single window, and the
// per-document session dedups later windows that share the same request id.
// Asking for 24 windows used to increment `remaining` without scheduling 24
// jobs, so the atomic patch never left the server.
const MAX_VIEWPORT_PARSE_WINDOWS: usize = 1;
use crate::server::{
    RuntimeGenerationStore,
    behavior::{ActiveBehaviorManifest, BehaviorVersionDecision},
    document::DocumentState,
    document_analysis::DocumentAnalysisCoordinator,
    js_runtime::ClayJsRuntimeService,
    language_intelligence::LanguageIntelligenceCoordinator,
    parse_coordinator::{ParseCoordinator, ParseCoordinatorError, ParseScheduleRequest},
    sdui::StaticSduiState,
    workspace::{
        WorkspaceError, WorkspaceState, open_existing_file_unlocked, reload_document_unlocked,
        save_document_unlocked,
    },
};

#[allow(
    clippy::too_many_arguments,
    reason = "open-document follow-up carries every server-owned state handle explicitly"
)]
pub(super) async fn write_document_open_response<S>(
    codec: &Codec,
    stream: &mut S,
    response: ServerMessage,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    parse_coordinator: &ParseCoordinator,
    document_analysis: &crate::server::document_analysis::DocumentAnalysisCoordinator,
    client_id: ClientId,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    codec.write_server_message(stream, &response).await?;
    let ServerMessage::DocumentOpened { metadata, .. } = &response else {
        return Ok(());
    };
    parse_coordinator.subscribe_document(metadata.document_id, client_id);
    document_analysis.subscribe_document(metadata.document_id, client_id);
    let Some(document) = workspace.lock().await.document_handle(metadata.document_id) else {
        return Ok(());
    };
    let runtime = runtime_generation.current().await;
    for message in open_document_followup_messages(
        metadata,
        &document,
        behavior,
        sdui,
        runtime.id,
        &runtime.service,
        parse_coordinator,
    )
    .await
    {
        codec.write_server_message(stream, &message).await?;
    }
    for message in start_document_analysis(
        document_analysis,
        workspace,
        behavior,
        runtime.id,
        metadata,
        &document,
    )
    .await
    {
        codec.write_server_message(stream, &message).await?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared edit/intent dispatch keeps server-owned state explicit instead of hiding authority in a context bag"
)]
pub(super) async fn dispatch_edit_operation<S>(
    codec: Codec,
    stream: &mut S,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    completion: &crate::server::completion::CompletionCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
    document_analysis: &crate::server::document_analysis::DocumentAnalysisCoordinator,
    parse_coordinator: &ParseCoordinator,
    client_id: ClientId,
    document_id: DocumentId,
    lease_id: Option<crate::protocol::LeaseId>,
    base_version: crate::protocol::DocumentVersion,
    behavior_version: crate::protocol::BehaviorVersion,
    transaction_id: crate::protocol::TransactionId,
    operation: crate::protocol::EditOperation,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let ack_scope = global_recorder().scope_with_metadata(
        SERVER_EDIT_ACK,
        MetricMetadata::transaction(document_id, client_id, transaction_id, base_version),
    );
    let Some(target_document) =
        document_for_message(document_id, client_id, document, workspace).await
    else {
        codec
            .write_server_message(
                stream,
                &ServerMessage::EditRejected {
                    document_id,
                    transaction_id,
                    reason: crate::protocol::EditRejection::InvalidDocument { document_id },
                },
            )
            .await?;
        ack_scope.finish();
        return Ok(());
    };

    let behavior_decision = match validate_edit_behavior_version(
        behavior,
        runtime_generation,
        client_id,
        document_id,
        transaction_id,
        behavior_version,
    )
    .await
    {
        Ok(decision) => decision,
        Err(response) => {
            reject_invalid_behavior_version(
                &codec,
                stream,
                runtime_generation,
                client_id,
                response,
            )
            .await?;
            ack_scope.finish();
            return Ok(());
        }
    };
    let analysis_delta = document_analysis_delta(&operation);
    let (response, parse_input) = {
        let mut document = target_document.lock().await;
        document.apply_edit_with_parse_input(
            document_id,
            client_id,
            lease_id,
            base_version,
            transaction_id,
            operation,
        )
    };
    codec.write_server_message(stream, &response).await?;
    ack_scope.finish();
    if let (
        ServerMessage::EditAck {
            confirmed_version, ..
        },
        Some(parse_input),
    ) = (response, parse_input)
    {
        if matches!(
            behavior_decision,
            BehaviorVersionDecision::PreviousWithinGrace
        ) {
            let _ = runtime_generation
                .behavior_grace()
                .record_previous_accepted(std::time::Instant::now())
                .await;
        }
        completion.document_changed(document_id, confirmed_version);
        language_intelligence.document_changed(document_id, confirmed_version);
        let (byte_start, byte_end, inserted_text) = analysis_delta;
        if document_analysis.change_document(
            document_id,
            base_version,
            confirmed_version,
            byte_start,
            byte_end,
            inserted_text,
        ) {
            let text = target_document.lock().await.text();
            document_analysis.reset_document(document_id, confirmed_version, text);
        }
        if let Err(diagnostic) = refresh_native_syntax_after_edit(
            workspace,
            behavior,
            runtime_generation,
            parse_coordinator,
            client_id,
            document_id,
            parse_input,
            global_recorder().is_enabled().then_some(transaction_id),
        )
        .await
        {
            codec
                .write_server_message(stream, &ServerMessage::RuntimeDiagnostic(diagnostic))
                .await?;
        }
    }
    Ok(())
}

/// Shared open path for the two built-in Command Centre sessions. Both the
/// command-intent lane (`CommandIntent` from a keybinding or the JS op) and
/// Control Center activation (selecting "Browse Filesystem" from the
/// catalogue) land here, so the one-active-session invariant has a single
/// enforcement point: the helper replaces any active server session and
/// returns its id for the caller to report as `TransientMenuClosed` before
/// pushing the snapshot.
///
/// Runs one bounded user-browse relist for Path Browser navigation (plan 083
/// task 8) and installs the result back into the session, returning the
/// re-projected snapshot. The listing runs on the blocking pool with no
/// workspace/tab/menu lock held; a failed listing keeps the session open in
/// its sticky error state (recoverable input) rather than failing the
/// intent. Returns `None` only when the session vanished while listing.
pub(super) async fn validate_edit_behavior_version(
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    client_id: ClientId,
    document_id: DocumentId,
    transaction_id: crate::protocol::TransactionId,
    behavior_version: crate::protocol::BehaviorVersion,
) -> Result<BehaviorVersionDecision, ServerMessage> {
    let current = behavior.lock().await.clone();
    let current_runtime_generation = runtime_generation.generation_id().await;
    let acknowledged_generation = runtime_generation
        .acknowledged_runtime_generation(client_id)
        .await;
    runtime_generation
        .behavior_grace()
        .validate_edit_version(
            &current,
            client_id,
            document_id,
            transaction_id,
            behavior_version,
            current_runtime_generation,
            acknowledged_generation,
            std::time::Instant::now(),
        )
        .await
}

pub(super) async fn reject_invalid_behavior_version<S>(
    codec: &Codec,
    stream: &mut S,
    runtime_generation: &RuntimeGenerationStore,
    client_id: ClientId,
    rejection: ServerMessage,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    codec.write_server_message(stream, &rejection).await?;
    if let Some(snapshot) = runtime_generation
        .latest_runtime_snapshot_for(client_id)
        .await
    {
        codec
            .write_server_message(
                stream,
                &ServerMessage::RuntimeStateSnapshot(Box::new(snapshot)),
            )
            .await?;
    }
    Ok(())
}

pub(super) struct DocumentChunkRequestParams {
    pub(super) client_id: ClientId,
    pub(super) document_id: DocumentId,
    pub(super) document_version: DocumentVersion,
    pub(super) offset: u64,
    pub(super) max_bytes: u32,
}

pub(super) async fn handle_document_chunk_request<S>(
    codec: Codec,
    stream: &mut S,
    default_document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    request: DocumentChunkRequestParams,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let message = match document_for_message(
        request.document_id,
        request.client_id,
        default_document,
        workspace,
    )
    .await
    {
        Some(document) => document.lock().await.document_chunk_message(
            request.document_version,
            request.offset,
            request.max_bytes,
        ),
        None => ServerMessage::DocumentChunkRejected {
            document_id: request.document_id,
            document_version: request.document_version,
            offset: request.offset,
            reason: crate::protocol::DocumentChunkRejection::UnknownDocument,
        },
    };
    codec.write_server_message(stream, &message).await
}

// Resolve only an explicitly authorized document. Unknown IDs must not fall
// through to welcome text: globally unique IDs make that fallback an
// information leak for completion, language, and edit requests.
pub(super) async fn document_for_message(
    document_id: DocumentId,
    client_id: ClientId,
    default_document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
) -> Option<Arc<Mutex<DocumentState>>> {
    let is_authorized_default = {
        let default_document = default_document.lock().await;
        default_document.document_id() == document_id && default_document.has_access(client_id)
    };
    if is_authorized_default {
        return Some(Arc::clone(default_document));
    }

    let document = workspace.lock().await.document_handle(document_id)?;
    let authorized = document.lock().await.has_access(client_id);
    authorized.then_some(document)
}

pub(super) fn document_analysis_delta(
    operation: &crate::protocol::EditOperation,
) -> (u64, u64, String) {
    match operation {
        crate::protocol::EditOperation::Insert { byte_offset, text } => {
            (*byte_offset, *byte_offset, text.clone())
        }
        crate::protocol::EditOperation::Delete { start, end } => (*start, *end, String::new()),
        crate::protocol::EditOperation::Replace { start, end, text } => {
            (*start, *end, text.clone())
        }
    }
}

pub(crate) async fn start_document_analysis(
    coordinator: &crate::server::document_analysis::DocumentAnalysisCoordinator,
    workspace: &Arc<Mutex<WorkspaceState>>,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    generation: u64,
    metadata: &DocumentMetadata,
    document: &Arc<Mutex<DocumentState>>,
) -> Vec<ServerMessage> {
    let canonical_root = workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .find(|root| root.workspace_root_id == metadata.workspace_root_id)
        .map(|root| root.canonical_path);
    let Some(canonical_root) = canonical_root else {
        return Vec::new();
    };
    let text = {
        let document = document.lock().await;
        if document.byte_len() > DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES {
            return vec![ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
                "analysis.document_too_large",
                "Document exceeds the package analysis limit; baseline language support remains active.",
            ))];
        }
        document.text()
    };
    let manifest_id = behavior
        .lock()
        .await
        .manifest_for(metadata.document_id)
        .manifest_id
        .clone();
    let active_mode = manifest_id.rsplit('.').next().unwrap_or(&manifest_id);
    coordinator
        .open_document(
            Arc::clone(workspace),
            generation,
            metadata,
            active_mode,
            canonical_root,
            text,
        )
        .into_iter()
        .map(ServerMessage::RuntimeDiagnostic)
        .collect()
}

/// Release every access grant the connection holds (disconnect) and tear down
/// document-scoped coordinator state for documents whose final holder left
/// (Plan 060 T6, P1-4). Documents still held by other connections keep their
/// analysis routes, versions, and provider state.
#[allow(
    clippy::too_many_arguments,
    reason = "disconnect teardown needs every document-scoped coordinator explicitly"
)]
pub(super) async fn open_document_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    workspace_root_id: WorkspaceRootId,
    path: String,
    client_id: ClientId,
) -> ServerMessage {
    let opened =
        match open_existing_file_unlocked(workspace, workspace_root_id, &path, client_id).await {
            Ok(opened) => opened,
            Err(error) => return file_operation_failed(error, Some(workspace_root_id), None),
        };

    let document = opened.document.lock().await;
    let metadata = DocumentMetadata {
        document_id: opened.document_id,
        version: document.version(),
        lease_id: opened.access.lease_id(),
        access: opened.access,
        dirty: document.is_dirty(),
        workspace_root_id,
        path: opened.file_state.display_path(),
    };
    let head = document.document_text_head();
    ServerMessage::DocumentOpened { metadata, head }
}

/// Shared bound-tab workspace open (plan 083 task 10): validates and
/// adds/gets the canonical directory root in the tab's workspace, rebinds
/// the tab through `TabRegistry::open_workspace` (bound-client-only),
/// broadcasts the reconciled snapshot, and returns the file-browser refresh
/// message (or the bounded failure) the caller must write. Used by the
/// `TabCommand::OpenWorkspace` branch and path-mode secondary activation —
/// path mode has no client-supplied tab id, so the bound tab is the only
/// target and a missing binding is an authorization failure. The caller
/// already validated any client-supplied tab id against the bound id.
#[allow(clippy::too_many_arguments)]
pub(super) async fn save_document_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
    known_version: crate::protocol::DocumentVersion,
) -> ServerMessage {
    match save_document_unlocked(workspace, document_id, client_id, known_version).await {
        Ok(outcome) => ServerMessage::DocumentSaved {
            document_id: outcome.document_id,
            version: outcome.version,
            dirty: outcome.dirty,
        },
        Err(error) => file_operation_failed(error, None, Some(document_id)),
    }
}

pub(super) async fn reload_document_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
    force: bool,
) -> ServerMessage {
    if let Err(error) = reload_document_unlocked(workspace, document_id, client_id, force).await {
        return file_operation_failed(error, None, Some(document_id));
    }
    let workspace = workspace.lock().await;
    match workspace.document_metadata(document_id, client_id).await {
        Ok(metadata) => {
            let Some(document) = workspace.document_handle(document_id) else {
                return file_operation_failed(
                    WorkspaceError::UnknownDocument { document_id },
                    None,
                    Some(document_id),
                );
            };
            let head = document.lock().await.document_text_head();
            ServerMessage::DocumentReloaded { metadata, head }
        }
        Err(error) => file_operation_failed(error, None, Some(document_id)),
    }
}

pub(super) async fn document_status_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
) -> ServerMessage {
    match workspace
        .lock()
        .await
        .document_metadata(document_id, client_id)
        .await
    {
        Ok(metadata) => ServerMessage::DocumentStatus { metadata },
        Err(error) => file_operation_failed(error, None, Some(document_id)),
    }
}

pub(super) async fn document_list_response(
    workspace: &Arc<Mutex<WorkspaceState>>,
    client_id: ClientId,
) -> ServerMessage {
    match workspace.lock().await.list_documents(client_id).await {
        Ok(documents) => ServerMessage::DocumentList { documents },
        Err(error) => file_operation_failed(error, None, None),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "shared open-document/reload follow-up primitive keeps server-owned state explicit"
)]
pub(crate) async fn open_document_followup_messages(
    metadata: &DocumentMetadata,
    document: &Arc<Mutex<DocumentState>>,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    generation_id: u64,
    js_runtime: &ClayJsRuntimeService,
    parse_coordinator: &ParseCoordinator,
) -> Vec<ServerMessage> {
    let probe_text = {
        let document = document.lock().await;
        document.bounded_prefix(crate::packages::modes::MAX_LEADING_CONTENT_BYTES)
    };
    let Some(activation) = classify_open_document(
        generation_id,
        js_runtime,
        parse_coordinator,
        metadata,
        &probe_text,
        behavior,
        sdui,
    )
    .await
    else {
        return vec![behavior.lock().await.manifest_message()];
    };

    let behavior_guard = behavior.lock().await;
    // Phase 22.2: the just-classified document's own mode layer (when the
    // open published one) precedes the connection-wide manifest. Other
    // documents' layers were already delivered when they opened; re-sending
    // them here is redundant chatter.
    let mut messages = Vec::new();
    if let Some(layer) = behavior_guard.document_layer(metadata.document_id).cloned() {
        messages.push(ServerMessage::BehaviorManifest(Box::new(layer)));
    }
    messages.push(behavior_guard.manifest_message());
    drop(behavior_guard);
    let parse_followup = {
        let document = document.lock().await;
        schedule_open_parse(
            parse_coordinator,
            metadata,
            &document,
            behavior,
            &activation,
        )
        .await
    };
    match parse_followup {
        Ok(Some(set)) => messages.push(ServerMessage::DecorationSet(set)),
        Ok(None) => {}
        Err(diagnostic) => messages.push(ServerMessage::RuntimeDiagnostic(diagnostic)),
    }

    messages
}

#[derive(Debug, Clone)]
pub(super) struct OpenModeActivation {
    pub(crate) package_prefix: String,
    pub(crate) mode_id: String,
    pub(crate) parse_handler_mode_id: String,
    pub(crate) native_parse_policy: Option<ParsePolicy>,
}

/// Cache key for a completed document mode activation (Plan 099). Captures
/// every classification input a mode can observe: runtime generation, path
/// identity (extension or full file name), shebang line, and a hash of the
/// bounded leading-content probe. Two opens with equal keys classify and
/// activate identically, so the cached manifest may be republished from
/// Rust without a generated module evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ModeActivationKey {
    generation_id: u64,
    path_key: String,
    shebang: Option<String>,
    leading_content_hash: u64,
}

/// A cached activation: the classification identity plus the behavior
/// manifest the V8 activation published. The manifest is inert validated
/// protocol data (rules/commands/keymaps), never executable content.
#[derive(Debug, Clone)]
pub(crate) struct CachedModeActivation {
    activation: OpenModeActivation,
    behavior_manifest: Option<crate::protocol::BehaviorManifest>,
}

/// Classification-observable identity of a path: the extension when present,
/// otherwise the full file name (Makefile-style probes match by name).
fn path_classification_key(path: &str) -> String {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    match file_name.rsplit_once('.') {
        Some((_, extension)) if !extension.is_empty() => extension.to_ascii_lowercase(),
        _ => file_name.to_string(),
    }
}

fn mode_activation_key(
    generation_id: u64,
    path: &str,
    shebang: &Option<String>,
    leading_content: &str,
) -> ModeActivationKey {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bounded_utf8_prefix(leading_content, 64).0.hash(&mut hasher);
    ModeActivationKey {
        generation_id,
        path_key: path_classification_key(path),
        shebang: shebang.clone(),
        leading_content_hash: hasher.finish(),
    }
}

pub(super) async fn classify_open_document(
    generation_id: u64,
    js_runtime: &ClayJsRuntimeService,
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    text: &str,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    _sdui: &Arc<Mutex<StaticSduiState>>,
) -> Option<OpenModeActivation> {
    // Supply the open path's bounded leading-content slice and shebang line so
    // server-owned classification probes can route scripts (shebang) and
    // magic-prefixed files (content probes). The slice is bounded to
    // MAX_LEADING_CONTENT_BYTES; `ModeRegistry::classify` rejects anything
    // larger, so probes never read unbounded content and no new filesystem
    // authority is introduced beyond the already-open document text.
    let shebang = text
        .lines()
        .next()
        .filter(|line| line.starts_with("#!"))
        .map(str::to_string);
    let leading_content =
        bounded_utf8_prefix(text, crate::packages::modes::MAX_LEADING_CONTENT_BYTES)
            .0
            .to_string();

    // Plan 099 registry fast path: a repeat open whose classification inputs
    // match a cached activation and whose native grammar is still registered
    // republishes the cached manifest from Rust — no generated module runs.
    let activation_key =
        mode_activation_key(generation_id, &metadata.path, &shebang, &leading_content);
    if let Some(cached) = js_runtime.cached_mode_activation(&activation_key)
        && js_runtime
            .registered_native_syntax_handler(generation_id, &metadata.path)
            .is_some()
    {
        // Re-scope the cached manifest to THIS document: the cached copy is
        // scoped to the document that produced it, and the behavior store
        // keys per-document layers by that scope.
        if let Some(mut manifest) = cached.behavior_manifest {
            manifest.scope = crate::protocol::BehaviorScope::Document {
                document_id: metadata.document_id,
            };
            behavior
                .lock()
                .await
                .publish_replacement(manifest)
                .map_err(|_| ())
                .ok()?;
        }
        return Some(cached.activation);
    }

    let shebang_json = serde_json::to_string(&shebang).unwrap_or_else(|_| "null".to_string());
    let leading_json =
        serde_json::to_string(&leading_content).unwrap_or_else(|_| "null".to_string());
    let source = format!(
        r#"
import {{ serverActivateClassifiedMode, serverClassifyDocument }} from "clay:modes";
import {{ loadPackage, serverListFirstPartyPackageSpecifiers }} from "clay:packages";
const input = {{ documentId: {}, path: {}, shebang: {}, leadingContent: {} }};
let classification = null;
try {{ classification = serverClassifyDocument(input); }} catch {{}}
// Built-in fallback modes (apiPrefix "core", e.g. core.text/core.code) are a
// last resort. Discard a built-in-only match so first-party packages still
// load and win precedence over the fallback, then only activate a real
// (non-built-in) classification below.
if (classification && classification.apiPrefix === "core") {{
  classification = null;
}}
if (!classification) {{
  for (const specifier of serverListFirstPartyPackageSpecifiers()) {{
    try {{
      await loadPackage(specifier);
      classification = serverClassifyDocument(input);
      if (classification && classification.apiPrefix !== "core") break;
    }} catch {{}}
  }}
}}
if (classification && classification.apiPrefix === "core") {{
  classification = null;
}}
if (classification) {{
  serverActivateClassifiedMode(classification, input);
}}
Deno.core.ops.op_clay_runtime_record(JSON.stringify(classification));
"#,
        metadata.document_id,
        serde_json::to_string(&metadata.path).ok()?,
        shebang_json,
        leading_json,
    );
    js_runtime
        .open_activation_evaluations
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let evaluation = js_runtime
        .evaluate_controlled_module_for_document(source, metadata.document_id)
        .await
        .ok()?;
    crate::server::apply_runtime_outputs_without_sdui(&evaluation, behavior).await;
    let record = evaluation.op_records.last()?;
    let value: serde_json::Value = serde_json::from_str(record).ok()?;
    let mut activation = OpenModeActivation {
        package_prefix: value.get("apiPrefix")?.as_str()?.to_string(),
        mode_id: value.get("modeId")?.as_str()?.to_string(),
        parse_handler_mode_id: value.get("modeId")?.as_str()?.to_string(),
        native_parse_policy: None,
    };
    if let Some((meta, policy)) = js_runtime
        .register_native_syntax_handler(
            parse_coordinator,
            generation_id,
            &evaluation,
            &metadata.path,
            &activation.package_prefix,
            &activation.mode_id,
        )
        .ok()
        .flatten()
    {
        activation.parse_handler_mode_id = meta.mode_id;
        activation.native_parse_policy = Some(policy);
    }
    // Tier 1 registers first. A same-generation JS handler remains available
    // only when no selected native handler owns this package/mode key.
    let _ = js_runtime.register_parse_handlers(parse_coordinator, generation_id, &evaluation);
    js_runtime.cache_mode_activation(
        activation_key,
        crate::server::connection::CachedModeActivation {
            activation: activation.clone(),
            behavior_manifest: evaluation.behavior_manifest.clone(),
        },
    );
    Some(activation)
}

#[expect(
    clippy::too_many_arguments,
    reason = "syntax refresh keeps server-owned document and runtime handles explicit"
)]
pub(super) async fn refresh_native_syntax_after_edit(
    workspace: &Arc<Mutex<WorkspaceState>>,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    parse_coordinator: &ParseCoordinator,
    client_id: ClientId,
    document_id: DocumentId,
    accepted_edit: ParseInputEdit,
    trace_id: Option<crate::protocol::PerformanceTraceId>,
) -> Result<(), RuntimeDiagnostic> {
    let (metadata, document) = {
        let workspace = workspace.lock().await;
        let Ok(metadata) = workspace.document_metadata(document_id, client_id).await else {
            return Ok(());
        };
        let Some(document) = workspace.document_handle(document_id) else {
            return Ok(());
        };
        (metadata, document)
    };
    let runtime = runtime_generation.current().await;
    let Some((meta, policy)) = runtime
        .service
        .registered_native_syntax_handler(runtime.id, &metadata.path)
    else {
        return Ok(());
    };
    let window = document
        .lock()
        .await
        .parse_window_after_edit(&meta.package_prefix, &meta.mode_id, policy, accepted_edit)
        .map_err(|message| {
            RuntimeDiagnostic::error(
                "parse.window_failed",
                format!("Parse window failed: {message}"),
            )
        })?;
    let Some(window) = window else {
        return Ok(());
    };
    parse_coordinator.record_native_edit_accepted_with_trace(
        metadata.document_id,
        metadata.version,
        trace_id,
    );
    let viewport = window.byte_range();
    schedule_parse_snapshot(
        parse_coordinator,
        &metadata,
        behavior.lock().await.version(),
        policy,
        window,
        viewport,
        Some(accepted_edit),
        trace_id,
        None,
        None,
    )?;
    Ok(())
}

pub(super) async fn schedule_open_parse(
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    document: &DocumentState,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    activation: &OpenModeActivation,
) -> Result<Option<crate::protocol::DecorationSet>, RuntimeDiagnostic> {
    let policy = activation.native_parse_policy.unwrap_or(ParsePolicy::new(
        64 * 1024,
        4 * 1024,
        30 * 1024 * 1024,
        5_000,
    ));
    let output_budget = policy
        .max_window_bytes
        .min(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES as u64);
    let end = document.bounded_byte_end(output_budget as usize) as u64;
    if end == 0 {
        return Ok(None);
    }
    let window = document
        .parse_window_snapshot(
            &activation.package_prefix,
            &activation.parse_handler_mode_id,
            ParseByteRange::new(0, end),
            policy.max_window_bytes,
        )
        .map_err(|message| {
            RuntimeDiagnostic::error(
                "parse.window_failed",
                format!("Parse window failed: {message}"),
            )
        })?;
    let viewport = window.byte_range();
    schedule_parse_snapshot(
        parse_coordinator,
        metadata,
        behavior.lock().await.version(),
        policy,
        window,
        viewport,
        None,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "string-window helper remains for connection parse tests"
    )
)]
pub(super) fn schedule_parse_window(
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    text: &str,
    behavior_version: u64,
    package_prefix: &str,
    mode_id: &str,
    policy: ParsePolicy,
    requested: ParseByteRange,
) -> Result<Option<crate::protocol::DecorationSet>, RuntimeDiagnostic> {
    let text_len = text.len();
    let viewport_start = floor_char_boundary(text, requested.start.min(text_len as u64) as usize);
    let output_budget = policy
        .max_window_bytes
        .min(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES as u64);
    let viewport_end = floor_char_boundary(
        text,
        requested
            .end
            .max(viewport_start as u64)
            .min(text_len as u64)
            .min((viewport_start as u64).saturating_add(output_budget)) as usize,
    );
    if viewport_start >= viewport_end {
        return Ok(None);
    }
    let viewport = ParseByteRange::new(viewport_start as u64, viewport_end as u64);

    let (window_start, window_end) = if text_len as u64 <= policy.max_window_bytes {
        (0, text_len)
    } else {
        let guard_budget = policy.max_window_bytes.saturating_sub(viewport.len());
        let before = policy.guard_bytes.min(guard_budget / 2);
        let after = policy.guard_bytes.min(guard_budget.saturating_sub(before));
        let start = floor_char_boundary(text, viewport_start.saturating_sub(before as usize));
        let mut end = ceil_char_boundary(
            text,
            viewport_end.saturating_add(after as usize).min(text_len),
        );
        if end.saturating_sub(start) > policy.max_window_bytes as usize {
            end = floor_char_boundary(text, start.saturating_add(policy.max_window_bytes as usize));
        }
        (start, end)
    };

    let prefix = &text[..window_start];
    let base_line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u64;
    let base_column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, trailing)| trailing.len()) as u64;
    let window = ParseWindowSnapshot {
        document_id: metadata.document_id,
        document_version: metadata.version,
        package_prefix: package_prefix.to_string(),
        mode_id: mode_id.to_string(),
        window_id: window_start as u64,
        byte_start: window_start as u64,
        byte_end: window_end as u64,
        base_line,
        base_column,
        incremental_edit: false,
        text: text[window_start..window_end].to_string(),
    };

    schedule_parse_snapshot(
        parse_coordinator,
        metadata,
        behavior_version,
        policy,
        window,
        viewport,
        None,
        None,
        None,
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "parse scheduling keeps validated document, policy, window, and trace context explicit"
)]
pub(super) fn schedule_parse_snapshot(
    parse_coordinator: &ParseCoordinator,
    metadata: &DocumentMetadata,
    behavior_version: u64,
    policy: ParsePolicy,
    window: ParseWindowSnapshot,
    viewport: ParseByteRange,
    accepted_edit: Option<ParseInputEdit>,
    trace_id: Option<crate::protocol::PerformanceTraceId>,
    request_id: Option<crate::protocol::ViewportRequestId>,
    client_id: Option<ClientId>,
) -> Result<Option<crate::protocol::DecorationSet>, RuntimeDiagnostic> {
    let invalidated_ranges =
        accepted_edit.map_or_else(|| vec![viewport], |edit| vec![edited_range(edit, viewport)]);
    let request = ParseScheduleRequest {
        document_id: metadata.document_id,
        document_version: metadata.version,
        behavior_version,
        package_prefix: window.package_prefix.clone(),
        mode_id: window.mode_id.clone(),
        viewport,
        invalidated_ranges,
        accepted_edit,
        trace_id,
        request_id,
        client_id,
    };
    match parse_coordinator.schedule_parse_with_windows(request, vec![window], Some(policy)) {
        Ok(_) | Err(ParseCoordinatorError::HandlerNotRegistered { .. }) => Ok(None),
        Err(error) => Err(RuntimeDiagnostic::error(
            "parse.viewport_activation_failed",
            format!("Viewport parse scheduling failed: {error:?}"),
        )),
    }
}

pub(super) fn floor_char_boundary(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset = offset.saturating_sub(1);
    }
    offset
}

pub(super) fn ceil_char_boundary(text: &str, mut offset: usize) -> usize {
    while offset < text.len() && !text.is_char_boundary(offset) {
        offset += 1;
    }
    offset
}

pub(super) fn edited_range(edit: ParseInputEdit, window: ParseByteRange) -> ParseByteRange {
    let start = edit.start_byte.clamp(window.start, window.end);
    let mut end = edit.new_end_byte.clamp(start, window.end);
    if start == end {
        if end < window.end {
            end += 1;
        } else if start > window.start {
            return ParseByteRange::new(start - 1, start);
        }
    }
    ParseByteRange::new(start, end)
}

pub(super) fn bounded_utf8_prefix(text: &str, max_bytes: usize) -> (&str, u64) {
    if text.len() <= max_bytes {
        return (text, text.len() as u64);
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], end as u64)
}

// ---------- coordinator loop handlers (Plan 090 task 2 extraction) ----------

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_request_resync<S>(
    codec: Codec,
    stream: &mut S,
    document: &Arc<Mutex<DocumentState>>,
    workspace: &Arc<Mutex<WorkspaceState>>,
    client_id: ClientId,
    document_id: DocumentId,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let Some(target_document) =
        document_for_message(document_id, client_id, document, workspace).await
    else {
        let response = file_operation_failed(
            WorkspaceError::UnknownDocument { document_id },
            None,
            Some(document_id),
        );
        codec.write_server_message(stream, &response).await?;
        return Ok(());
    };
    let response = {
        let document = target_document.lock().await;
        document.resync_snapshot_message_for_client(document_id, client_id)
    };
    codec.write_server_message(stream, &response).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_viewport_render_request<S>(
    codec: Codec,
    stream: &mut S,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    workspace: &Arc<Mutex<WorkspaceState>>,
    parse_coordinator: &ParseCoordinator,
    client_id: ClientId,
    document_id: DocumentId,
    document_version: DocumentVersion,
    request_id: crate::protocol::ViewportRequestId,
    byte_start: u64,
    byte_end: u64,
    trace_id: Option<crate::protocol::PerformanceTraceId>,
) -> Result<usize, CodecError>
where
    S: AsyncWrite + Unpin,
{
    use crate::protocol::{ViewportRenderPatch, ViewportRenderStatus};

    if byte_start > byte_end {
        write_viewport_rejection(
            &codec,
            stream,
            request_id,
            document_id,
            document_version,
            "invalidRange",
        )
        .await?;
        return Ok(0);
    }
    let (metadata, target_document) = {
        let workspace = workspace.lock().await;
        let Ok(metadata) = workspace.document_metadata(document_id, client_id).await else {
            write_viewport_rejection(
                &codec,
                stream,
                request_id,
                document_id,
                document_version,
                "unknownDocument",
            )
            .await?;
            return Ok(0);
        };
        let Some(target_document) = workspace.document_handle(document_id) else {
            write_viewport_rejection(
                &codec,
                stream,
                request_id,
                document_id,
                document_version,
                "unknownDocument",
            )
            .await?;
            return Ok(0);
        };
        (metadata, target_document)
    };
    if metadata.version != document_version {
        write_viewport_rejection(
            &codec,
            stream,
            request_id,
            document_id,
            document_version,
            "staleVersion",
        )
        .await?;
        return Ok(0);
    }
    let runtime = runtime_generation.current().await;
    let Some((meta, policy)) = runtime
        .service
        .registered_native_syntax_handler(runtime.id, &metadata.path)
    else {
        // Valid request with no renderable output: explicit empty completion
        // frees the client's request slot without a heuristic timer.
        codec
            .write_server_message(
                stream,
                &ServerMessage::ViewportRenderPatch(ViewportRenderPatch {
                    request_id,
                    document_id,
                    document_version,
                    status: ViewportRenderStatus::Empty,
                    reason: None,
                    covered_ranges: Vec::new(),
                    decorations: Vec::new(),
                    diagnostics: Vec::new(),
                    folds: Vec::new(),
                    trace_id,
                }),
            )
            .await?;
        return Ok(0);
    };
    let behavior_version = behavior.lock().await.version();
    // Rope-sliced windows covering the WHOLE requested viewport, clamped to
    // the document's total bytes before any window snapshot is allocated.
    // The parse context range (windows, possibly wider for grammar context)
    // stays separate from the authoritative output coverage claimed by the
    // eventual patch members.
    let windows = {
        let document = target_document.lock().await;
        let total_bytes = document.byte_len() as u64;
        document.parse_windows_covering(
            &meta.package_prefix,
            &meta.mode_id,
            ParseByteRange::new(byte_start.min(total_bytes), byte_end.min(total_bytes)),
            policy,
            MAX_VIEWPORT_PARSE_WINDOWS,
        )
    };
    let windows = match windows {
        Ok(windows) => windows,
        Err(reason) => {
            write_viewport_rejection(
                &codec,
                stream,
                request_id,
                document_id,
                document_version,
                "activationFailed",
            )
            .await?;
            let _ = reason;
            return Ok(0);
        }
    };
    let Some(window) = windows.into_iter().next() else {
        codec
            .write_server_message(
                stream,
                &ServerMessage::ViewportRenderPatch(ViewportRenderPatch {
                    request_id,
                    document_id,
                    document_version,
                    status: ViewportRenderStatus::Empty,
                    reason: None,
                    covered_ranges: Vec::new(),
                    decorations: Vec::new(),
                    diagnostics: Vec::new(),
                    folds: Vec::new(),
                    trace_id,
                }),
            )
            .await?;
        return Ok(0);
    };
    let piece_viewport = ParseByteRange::new(window.byte_start, window.byte_end);
    if let Err(diagnostic) = schedule_parse_snapshot(
        parse_coordinator,
        &metadata,
        behavior_version,
        policy,
        window,
        piece_viewport,
        None,
        trace_id,
        Some(request_id),
        Some(client_id),
    ) {
        codec
            .write_server_message(stream, &ServerMessage::RuntimeDiagnostic(diagnostic))
            .await?;
        write_viewport_rejection(
            &codec,
            stream,
            request_id,
            document_id,
            document_version,
            "activationFailed",
        )
        .await?;
        return Ok(0);
    }
    Ok(1)
}

async fn write_viewport_rejection<S>(
    codec: &Codec,
    stream: &mut S,
    request_id: crate::protocol::ViewportRequestId,
    document_id: DocumentId,
    document_version: DocumentVersion,
    reason: &str,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    use crate::protocol::ViewportRenderPatch;

    codec
        .write_server_message(
            stream,
            &ServerMessage::ViewportRenderPatch(ViewportRenderPatch::rejected(
                request_id,
                document_id,
                document_version,
                reason,
            )),
        )
        .await
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_open_document<S>(
    codec: Codec,
    stream: &mut S,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: &RuntimeGenerationStore,
    workspace: &Arc<Mutex<WorkspaceState>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
    parse_coordinator: &ParseCoordinator,
    document_analysis: &DocumentAnalysisCoordinator,
    client_id: ClientId,
    workspace_root_id: WorkspaceRootId,
    path: String,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let response = open_document_response(workspace, workspace_root_id, path, client_id).await;
    write_document_open_response(
        &codec,
        stream,
        response,
        behavior,
        runtime_generation,
        workspace,
        sdui,
        parse_coordinator,
        document_analysis,
        client_id,
    )
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_save_document<S>(
    codec: Codec,
    stream: &mut S,
    workspace: &Arc<Mutex<WorkspaceState>>,
    client_id: ClientId,
    document_id: DocumentId,
    known_version: DocumentVersion,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let response = save_document_response(workspace, document_id, client_id, known_version).await;
    codec.write_server_message(stream, &response).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_reload_document<S>(
    codec: Codec,
    stream: &mut S,
    workspace: &Arc<Mutex<WorkspaceState>>,
    completion: &crate::server::completion::CompletionCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
    document_analysis: &DocumentAnalysisCoordinator,
    client_id: ClientId,
    document_id: DocumentId,
    _known_version: DocumentVersion,
    force: bool,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let response = reload_document_response(workspace, document_id, client_id, force).await;
    codec.write_server_message(stream, &response).await?;
    if let ServerMessage::DocumentReloaded { metadata, head } = response {
        completion.document_changed(document_id, metadata.version);
        language_intelligence.document_changed(document_id, metadata.version);
        document_analysis.reset_document(document_id, metadata.version, head.first_chunk);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_close_document<S>(
    codec: Codec,
    stream: &mut S,
    workspace: &Arc<Mutex<WorkspaceState>>,
    parse_coordinator: &ParseCoordinator,
    completion: &crate::server::completion::CompletionCoordinator,
    language_intelligence: &LanguageIntelligenceCoordinator,
    document_analysis: &DocumentAnalysisCoordinator,
    client_id: ClientId,
    document_id: DocumentId,
    force: bool,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let outcome = {
        let mut workspace = workspace.lock().await;
        workspace
            .close_document(document_id, client_id, force)
            .await
    };
    match outcome {
        Ok(outcome) => {
            // This connection's subscriptions end immediately; the document
            // may stay alive for other connections.
            parse_coordinator.unsubscribe_document(document_id, client_id);
            document_analysis.unsubscribe_document(document_id, client_id);
            if outcome.closed {
                teardown_closed_document(
                    document_id,
                    outcome.version,
                    parse_coordinator,
                    completion,
                    language_intelligence,
                    document_analysis,
                );
            }
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::DocumentClosed {
                        document_id,
                        closed: outcome.closed,
                    },
                )
                .await?;
        }
        Err(error) => {
            let response = file_operation_failed(error, None, Some(document_id));
            codec.write_server_message(stream, &response).await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_get_document_status<S>(
    codec: Codec,
    stream: &mut S,
    workspace: &Arc<Mutex<WorkspaceState>>,
    client_id: ClientId,
    document_id: DocumentId,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let response = document_status_response(workspace, document_id, client_id).await;
    codec.write_server_message(stream, &response).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_list_documents<S>(
    codec: Codec,
    stream: &mut S,
    workspace: &Arc<Mutex<WorkspaceState>>,
    client_id: ClientId,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    let response = document_list_response(workspace, client_id).await;
    codec.write_server_message(stream, &response).await?;
    Ok(())
}

/// Plan 071 task 10: read-only tree-sitter text-object/smart-select ranges.
/// Every miss (validation, no grammar, no parse handler, timed-out parse)
/// degrades to empty ranges so an advisory selection query can never block
/// editing.
#[allow(clippy::too_many_arguments)] // mirrors the connection loop's context handles
pub(super) async fn handle_selection_query_request<S>(
    codec: Codec,
    stream: &mut S,
    workspace: &Arc<Mutex<WorkspaceState>>,
    document: &Arc<Mutex<DocumentState>>,
    parse_coordinator: &ParseCoordinator,
    runtime_generation: &RuntimeGenerationStore,
    client_id: ClientId,
    request: &crate::protocol::SelectionQueryRequest,
) -> Result<(), CodecError>
where
    S: AsyncWrite + Unpin,
{
    if let Err(rejection) = request.validate() {
        codec
            .write_server_message(
                stream,
                &ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    message: format!("selection query request rejected: {rejection:?}"),
                },
            )
            .await?;
        return Ok(());
    }
    let metadata = workspace
        .lock()
        .await
        .document_metadata(request.document_id, client_id)
        .await
        .ok();
    let mut ranges: Vec<Option<SelectionQueryRange>> = vec![None; request.selections.len()];
    if let Some(metadata) = metadata {
        let Some(target_document) =
            document_for_message(request.document_id, client_id, document, workspace).await
        else {
            codec
                .write_server_message(
                    stream,
                    &ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message: "selection-query document is not authorized for this connection"
                            .to_string(),
                    },
                )
                .await?;
            return Ok(());
        };
        let document_text = target_document.lock().await.text();
        let runtime = runtime_generation.current().await;
        if let Some((meta, _policy)) = runtime
            .service
            .registered_native_syntax_handler(runtime.id, &metadata.path)
            && let Some(handler) =
                parse_coordinator.handler_for(&meta.package_prefix, &meta.mode_id)
            && let Some(query_ranges) = handler.selection_query_ranges(
                request.document_id,
                request.document_version,
                &document_text,
                request.query,
                &request.selections,
            )
        {
            ranges = query_ranges;
        }
    }
    codec
        .write_server_message(
            stream,
            &ServerMessage::SelectionQueryResult {
                result: SelectionQueryResult {
                    request_id: request.request_id,
                    client_id,
                    document_id: request.document_id,
                    document_version: request.document_version,
                    behavior_version: request.behavior_version,
                    ranges,
                },
            },
        )
        .await?;
    Ok(())
}
