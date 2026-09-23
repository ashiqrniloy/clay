//! Client/server wire envelopes and their rejection payloads.

use super::*;

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct RegionLockConflict {
    pub lock_id: RegionLockId,
    pub start: u64,
    pub end: u64,
    pub owner: LockOwner,
    pub created_at_version: DocumentVersion,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LockOwner {
    Server,
    Client { client_id: ClientId },
    Extension { extension_id: String },
    AiAgent { agent_id: String },
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EditRejection {
    StaleVersion {
        client_base_version: DocumentVersion,
        server_version: DocumentVersion,
    },
    FutureVersion {
        client_base_version: DocumentVersion,
        server_version: DocumentVersion,
    },
    LeaseRequired,
    LeaseExpired {
        lease_id: LeaseId,
    },
    ReadOnlyDocument,
    RegionLocked {
        conflict: RegionLockConflict,
    },
    InvalidDocument {
        document_id: DocumentId,
    },
    InvalidRange {
        message: String,
    },
    InvalidBehaviorVersion {
        behavior_version: BehaviorVersion,
        server_behavior_version: BehaviorVersion,
    },
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(
    tag = "family",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ClientMessage {
    Hello {
        protocol_version: u32,
        client_name: String,
    },
    Edit {
        document_id: DocumentId,
        client_id: ClientId,
        lease_id: Option<LeaseId>,
        base_version: DocumentVersion,
        behavior_version: BehaviorVersion,
        transaction_id: TransactionId,
        operation: EditOperation,
    },
    EditorIntent {
        document_id: DocumentId,
        client_id: ClientId,
        lease_id: Option<LeaseId>,
        base_version: DocumentVersion,
        behavior_version: BehaviorVersion,
        transaction_id: TransactionId,
        intent: EditorIntent,
    },
    RequestResync {
        document_id: DocumentId,
        client_id: ClientId,
        known_version: DocumentVersion,
    },
    DocumentChunkRequest {
        client_id: ClientId,
        document_id: DocumentId,
        document_version: DocumentVersion,
        offset: u64,
        max_bytes: u32,
    },
    ViewportRenderRequest {
        client_id: ClientId,
        document_id: DocumentId,
        document_version: DocumentVersion,
        /// Monotonic per-connection request identity; the server answers with
        /// exactly one `ViewportRenderPatch` (complete, empty, or rejected)
        /// carrying this id. Stale ids are dropped by the client.
        request_id: ViewportRequestId,
        byte_start: u64,
        byte_end: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trace_id: Option<PerformanceTraceId>,
    },
    OpenDocument {
        client_id: ClientId,
        workspace_root_id: WorkspaceRootId,
        path: String,
    },
    OpenSelectedFile {
        client_id: ClientId,
        /// Server-issued single-use selected-path capability token. Required so
        /// the server authorizes selected-file opens rather than honoring raw paths.
        capability: String,
        selected_path: String,
    },
    /// Agent settings page (plan 117): list the daemon-delivered config files
    /// (SYSTEM.md + seeded skills/<name>/SKILL.md). Paths are built server-side
    /// from the configuration root; the client supplies no path.
    ListAgentSettingsFiles {
        client_id: ClientId,
    },
    /// Agent settings page (plan 117): open one listed file into the normal
    /// document pipeline by server-validated name (no webview-supplied paths).
    OpenAgentSettingsFile {
        client_id: ClientId,
        name: String,
    },
    /// Launcher (plan 118 Part D): the start surface's server-resolved entries
    /// — recent workspaces and configured agent types. Read-only display data
    /// with no path authority.
    ListLauncherEntries {
        client_id: ClientId,
    },
    /// Launcher: drop one recent workspace by its index in the server's own
    /// list. The webview never sends a path back.
    RemoveLauncherRecent {
        client_id: ClientId,
        index: u32,
    },
    AddSelectedWorkspaceRoot {
        client_id: ClientId,
        /// Server-issued single-use selected-path capability token. Required so
        /// the server authorizes selected-folder roots rather than honoring raw paths.
        capability: String,
        selected_path: String,
    },
    SaveDocument {
        client_id: ClientId,
        document_id: DocumentId,
        known_version: DocumentVersion,
    },
    ReloadDocument {
        client_id: ClientId,
        document_id: DocumentId,
        known_version: DocumentVersion,
        force: bool,
    },
    GetDocumentStatus {
        client_id: ClientId,
        document_id: DocumentId,
    },
    ListDocuments {
        client_id: ClientId,
    },
    SduiAction {
        client_id: ClientId,
        ui_version: SduiVersion,
        intent: SduiActionIntent,
    },
    CommandIntent {
        client_id: ClientId,
        document_id: DocumentId,
        behavior_version: BehaviorVersion,
        command_id: String,
    },
    /// Phase 18.11 completion request. Enqueued after a local-first edit that
    /// hit a behavior-manifest autocomplete trigger, or after a manual
    /// `completion.trigger` command. Carries typed request metadata only (no
    /// document text); bounded accepted-completion recency hints are inert
    /// ranking data. The server-side provider lane stale-drops older requests
    /// against `document_version`/`behavior_version`/`provider_generation`.
    CompletionRequest {
        request: CompletionRequest,
    },
    /// Phase 18.20 engine-neutral language-intelligence request. Enqueued after a
    /// local-first hover/definition/code-action/signature-help intent captures
    /// the current document/version/cursor byte offset. Carries typed request
    /// metadata only (no document text); the server-side provider lane
    /// stale-drops older requests against `document_version`/
    /// `behavior_version`/`provider_generation`. Canonical positions are UTF-8
    /// byte offsets; LSP line/character/URI conversion lives in Phase 18.21
    /// package adapters.
    LanguageIntelligenceRequest {
        request: LanguageIntelligenceRequest,
    },
    /// Plan 071 task 10: UI-reactive tree-sitter text-object/smart-select
    /// request. The client captures its selection set + document/behavior
    /// versions locally; the server runs the active grammar's read-only query
    /// and answers with `ServerMessage::SelectionQueryResult`.
    SelectionQueryRequest {
        request: SelectionQueryRequest,
    },
    /// Phase 19 acknowledgement that the client validated and atomically
    /// installed `RuntimeStateSnapshot` for the named runtime generation.
    /// Controls stale-edit grace eligibility only; the server never waits on
    /// this message during commit.
    RuntimeGenerationInstalled {
        client_id: ClientId,
        runtime_generation_id: RuntimeGenerationId,
    },
    /// Plan 060 T6: explicit document close. The server releases the client's
    /// access; when the last holder leaves, all document-scoped state (trees,
    /// versions, analysis routes, leases) is torn down. A dirty document
    /// requires `force` so close intent is explicit about discarding unsaved
    /// editor state.
    CloseDocument {
        client_id: ClientId,
        document_id: DocumentId,
        force: bool,
    },
    /// Phase 22.3: server-authoritative tab lifecycle. Each tab is a real
    /// separate client connection; the server holds the tab registry (order,
    /// active tab, per-tab workspace + client binding) so tab structure
    /// survives client reconnects. `client_id` is validated against the
    /// connection's handshake identity like every other client message.
    TabCommand {
        client_id: ClientId,
        command: TabCommand,
    },
    /// Phase 24.1: interactive intents for a server-owned transient menu
    /// session. The server is authoritative for query, items, and selection;
    /// the client renders snapshots and forwards keystrokes only. `session_id`
    /// is an opaque server-allocated handle (high bit set); unknown/stale ids
    /// are dropped server-side with a bounded diagnostic, never an error.
    MenuQueryUpdate {
        client_id: ClientId,
        #[serde(with = "menu_session_id_serde")]
        session_id: u64,
        query: String,
        /// Plan 124: the palette's scope chip selection (`All` = absent/`None`,
        /// else one of the server's closed scope words). It rides the filter
        /// update because it *is* one: the session filters and selects over the
        /// scoped item set, so the client never hides rows it did not filter.
        /// Sessions whose items carry no scope ignore it.
        #[serde(default)]
        scope: Option<String>,
    },
    /// Generic semantic Backspace (Phase 24.3): the server session decides
    /// whether Backspace deletes query text or ascends (path mode). The
    /// client no longer pops the mirrored query locally; it only mirrors
    /// full-value replacements for the bounded character send path and
    /// resyncs from every snapshot.
    MenuBackspace {
        client_id: ClientId,
        #[serde(with = "menu_session_id_serde")]
        session_id: u64,
    },
    /// Relative selection movement (arrow keys); the server clamps.
    MenuSelectionMove {
        client_id: ClientId,
        #[serde(with = "menu_session_id_serde")]
        session_id: u64,
        delta: i64,
    },
    /// Activate the session's currently selected item. The server holds the
    /// item's action; the client never supplies command payloads. `kind`
    /// distinguishes primary (Enter/Tab) from secondary (Alt+Enter)
    /// activation; kind semantics are interpreted by the session kind.
    MenuActivate {
        client_id: ClientId,
        #[serde(with = "menu_session_id_serde")]
        session_id: u64,
        kind: TransientMenuActivationData,
    },
    /// Dismiss the session. The server drops it and answers
    /// `ServerMessage::TransientMenuClosed`.
    MenuCancel {
        client_id: ClientId,
        #[serde(with = "menu_session_id_serde")]
        session_id: u64,
    },
    /// Phase 25: client agent intent. Boxed so the union floor stays small.
    /// Composer keystrokes never wait on this message.
    Agent {
        client_id: ClientId,
        command: Box<AgentClientCommand>,
    },
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
)]
#[serde(
    tag = "family",
    content = "payload",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServerMessage {
    Welcome {
        client_id: ClientId,
        protocol_version: u32,
    },
    InitialDocument {
        document_id: DocumentId,
        version: DocumentVersion,
        head: DocumentTextHead,
        access: DocumentAccess,
        lease_id: Option<LeaseId>,
        /// The workspace root path the initial document belongs to (server
        /// truth; the client uses it to register its initial tab with
        /// `TabCommand::New`). Phase 22.3.
        workspace_root: String,
    },
    BehaviorManifest(Box<BehaviorManifest>),
    SduiSnapshot {
        client_id: ClientId,
        tree: SduiTree,
    },
    /// Server-issued single-use capability token authorizing one subsequent
    /// selected-path request (`OpenSelectedFile` or `AddSelectedWorkspaceRoot`).
    /// Issued once after the Hello handshake and re-issued after every attempt
    /// so the client always has one pending token. Structural authority gate for
    /// selected file/folder paths.
    FileOpenCapabilityIssued {
        token: String,
    },
    SduiUpdate {
        update: SduiTreeUpdate,
    },
    DecorationSet(DecorationSet),
    /// Validated folding ranges for one document version. Collapse state is
    /// client-local and is not part of this message. Payload-capped by
    /// `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`.
    FoldingRangeSet(FoldingRangeSet),
    /// All authority chunks produced by one parse update, in viewport-key
    /// order. Clients apply chunks in order; the batch shares the single-set
    /// validation and staleness semantics per chunk.
    DecorationBatch(Vec<DecorationSet>),
    DiagnosticSet(DiagnosticSet),
    /// Protocol v29 atomic viewport answer: exactly one per
    /// `ViewportRenderRequest` id, carrying ordered members and a terminal
    /// complete/empty/rejected status.
    ViewportRenderPatch(ViewportRenderPatch),
    EditAck {
        document_id: DocumentId,
        confirmed_version: DocumentVersion,
        transaction_id: TransactionId,
    },
    EditRejected {
        document_id: DocumentId,
        transaction_id: TransactionId,
        reason: EditRejection,
    },
    EditTransaction {
        document_id: DocumentId,
        version: DocumentVersion,
        transaction_id: TransactionId,
        operations: Vec<EditOperation>,
    },
    ResyncSnapshot {
        document_id: DocumentId,
        version: DocumentVersion,
        head: DocumentTextHead,
        access: DocumentAccess,
        lease_id: Option<LeaseId>,
    },
    DocumentChunk {
        document_id: DocumentId,
        document_version: DocumentVersion,
        offset: u64,
        text: String,
    },
    DocumentChunkRejected {
        document_id: DocumentId,
        document_version: DocumentVersion,
        offset: u64,
        reason: DocumentChunkRejection,
    },
    DocumentOpened {
        metadata: DocumentMetadata,
        head: DocumentTextHead,
    },
    DocumentSaved {
        document_id: DocumentId,
        version: DocumentVersion,
        dirty: bool,
    },
    DocumentReloaded {
        metadata: DocumentMetadata,
        head: DocumentTextHead,
    },
    DocumentStatus {
        metadata: DocumentMetadata,
    },
    DocumentList {
        documents: Vec<DocumentMetadata>,
    },
    FileOperationFailed {
        code: FileErrorCode,
        message: String,
        workspace_root_id: Option<WorkspaceRootId>,
        document_id: Option<DocumentId>,
    },
    /// Agent settings page (plan 117): the bounded listing for
    /// ListAgentSettingsFiles. Empty when no config root exists.
    AgentSettingsFiles {
        client_id: ClientId,
        files: Vec<AgentSettingsFileInfo>,
    },
    /// Reply to `ListLauncherEntries` (and after a recent is removed): the
    /// launcher's server-resolved rows. `pruned` entries were dropped because
    /// their folder is gone.
    LauncherEntries {
        client_id: ClientId,
        entries: Box<LauncherEntries>,
    },
    RuntimeDiagnostic(RuntimeDiagnostic),
    /// Phase 18.11 completion result set. Bounded, versioned, provenance-bearing
    /// completion items published to the client after the server-side provider
    /// lane validates the result against the current document/behavior version
    /// and provider generation. Items are inert text-replacement data only.
    CompletionResult {
        result: CompletionResultSet,
    },
    /// Phase 18.11 completion result rejection. Published in place of a result
    /// when a result set fails validation (stale version/generation, invalid
    /// range, payload/item/field budget) before client publication.
    CompletionRejected {
        request_id: CompletionRequestId,
        reason: CompletionRejection,
    },
    /// Phase 18.20 language-intelligence result. Bounded, versioned,
    /// provenance-bearing, feature-tagged result payload published after
    /// server-side validation. Inert data only; code-action edits are inert
    /// previews and command-backed actions execute later through
    /// `CommandExecution`.
    LanguageIntelligenceResult {
        result: LanguageIntelligenceResult,
    },
    /// Phase 18.20 language-intelligence result rejection. Published in place of
    /// a result when validation fails (stale version/generation, invalid byte
    /// range/path/command, payload/count/field budget) before client
    /// publication.
    LanguageIntelligenceRejected {
        request_id: LanguageIntelligenceRequestId,
        reason: LanguageIntelligenceRejection,
    },
    /// Plan 071 task 10: read-only tree-sitter text-object/smart-select byte
    /// ranges aligned index-for-index with the request's selections. Inert
    /// data only; the client applies ranges as selections.
    SelectionQueryResult {
        result: SelectionQueryResult,
    },
    /// Plan 071 follow-up round (`editor-control`): gated programmatic
    /// execution of one known editor command ID. Boxed so the variant's
    /// inline size never inflates small payloads. Advisory: the client
    /// re-parses the command ID deny-by-default and drops unknown IDs.
    EditorCommandRequest(Box<EditorCommandRequest>),
    /// Runtime caret appearance override from `clientSetCursorStyle`
    /// (editor-control gated). `None` clears the override so the effective
    /// style falls back to the per-mode manifest then the theme default.
    CaretStyleOverride(Option<CaretStyle>),
    /// Phase 26 user-owned editor wrap-policy override from `setEditorLayout`
    /// (trusted-domain configuration only; packages cannot forge it). `None`
    /// clears the override so the effective wrap falls back to the per-mode
    /// manifest `editorRules.layout.wrap` then `WrapPolicy::from_font_role`.
    EditorLayoutOverride(Option<WrapPolicy>),
    /// Phase 22.1 shell-level user preferences from `setPaneFocusPolicy`
    /// (configuration-time). Sent on initial connect and whenever the
    /// preference changes during `init.js` evaluation/reload.
    ShellPreferences(ShellPreferences),
    /// Phase 22.3: server-authoritative tab registry snapshot. Broadcast to
    /// every connection on any registry mutation and replayed on handshake so
    /// each tab's `Driver` sees the same tab order/active tab. Inert data only;
    /// the client applies it to its tab bar and per-tab connection map.
    TabRegistry(TabRegistrySnapshot),
    /// Phase 18.15 (Plan 046) resolved active theme snapshot. Sent once after
    /// the welcome `BehaviorManifest` when `setTheme("...")` ran in `init.js`;
    /// absent when no theme is selected (Clay default theme applies). The
    /// client reconstructs the `StyleRegistry` from the inert overrides.
    ActiveTheme(ActiveTheme),
    /// User-owned typography snapshot. It is independently revisioned because
    /// family/size changes affect shaping and geometry, unlike theme colors.
    ActiveTypography(ActiveTypography),
    /// Phase 19 complete runtime-generation snapshot for atomic client install.
    /// Sent after a successful generation commit (and on lag recovery) instead
    /// of independent Behavior/Theme/Typography/SDUI messages for live reload.
    /// Boxed because the complete snapshot is substantially larger than other
    /// server-message variants.
    RuntimeStateSnapshot(Box<RuntimeStateSnapshot>),
    Error {
        code: ProtocolErrorCode,
        message: String,
    },
    /// Plan 060 T6 acknowledgement that `CloseDocument` released the client's
    /// access; `closed` is true when this was the final holder and the server
    /// tore down all document-scoped state.
    DocumentClosed {
        document_id: DocumentId,
        closed: bool,
    },
    /// Phase 24.1: bounded inert snapshot of a server-owned transient menu
    /// session. Boxed so the variant's inline size never inflates the union
    /// floor that small payloads like `EditAck` pay. The client renders this
    /// through the existing `TransientMenuSession` overlay projection.
    TransientMenuSnapshot(Box<TransientMenuSnapshotData>),
    /// Phase 24.1: a server-owned transient menu session ended (cancelled,
    /// replaced, or swept on disconnect). The client clears the overlay for
    /// this session id. No `Cancelled` status crosses the wire; this message
    /// IS the terminal state.
    TransientMenuClosed {
        session_id: u64,
    },
    /// Phase 24.2: a menu-activated shell command approved by the server.
    /// The id comes only from the server-held session catalogue (the shell
    /// surface); the client re-parses it deny-by-default through
    /// `ShellClientCommand::from_command_id` and drops unknown ids with no
    /// state mutation. No generic arbitrary client-command channel exists.
    ShellClientCommandRequest {
        command_id: String,
    },
    /// Phase 25: server-authoritative agent snapshot/event/inventory.
    /// Boxed. Never carries credential secrets.
    Agent(Box<AgentServerMessage>),
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ProtocolErrorCode {
    UnsupportedProtocolVersion,
    InvalidMessage,
    AccessDenied,
    InternalError,
}
