pub mod codec;
pub mod completion;
pub mod decorations;
pub mod diagnostics;
pub mod parse;
pub mod sdui;

pub use completion::*;
pub use decorations::*;
pub use diagnostics::*;
pub use parse::*;
pub use sdui::*;

/// Current wire protocol version for the local Clay IPC boundary.
///
/// Version 2 adds `DecorationViewportRequest` to `ClientMessage`; version 1
/// peers must not decode the changed enum discriminants.
pub const PROTOCOL_VERSION: u32 = 2;

pub type ClientId = u64;
pub type DocumentId = u64;
pub type DocumentVersion = u64;
pub type BehaviorVersion = u64;
pub type TransactionId = u64;
pub type LeaseId = u64;
pub type RegionLockId = u64;
pub type WorkspaceRootId = u64;

/// Closed semantic profile selected by Clay-owned typography configuration.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontRole {
    Monospace,
    Proportional,
    Ui,
}

/// Document-only role used for a document default or syntax/semantic override.
/// `Inherit` leaves a decoration on its document default; UI is deliberately
/// absent because diagnostics/search and document syntax cannot select it.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentFontRole {
    Inherit,
    Monospace,
    Proportional,
}

impl DocumentFontRole {
    pub const fn font_role(self) -> Option<FontRole> {
        match self {
            Self::Inherit => None,
            Self::Monospace => Some(FontRole::Monospace),
            Self::Proportional => Some(FontRole::Proportional),
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "inherit" => Some(Self::Inherit),
            "monospace" => Some(Self::Monospace),
            "proportional" => Some(Self::Proportional),
            _ => None,
        }
    }
}

impl FontRole {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "monospace" => Some(Self::Monospace),
            "proportional" => Some(Self::Proportional),
            "ui" => Some(Self::Ui),
            _ => None,
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub document_id: DocumentId,
    pub version: DocumentVersion,
    pub access: DocumentAccess,
    pub lease_id: Option<LeaseId>,
    pub dirty: bool,
    pub workspace_root_id: WorkspaceRootId,
    pub path: String,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FileErrorCode {
    UnknownWorkspaceRoot,
    UnknownDocument,
    NotFound,
    AccessDenied,
    OutsideRoot,
    InvalidUtf8,
    PermissionDenied,
    UnsupportedFileType,
    DirectoryOpen,
    DirtyDocument,
    StaleFileMetadata,
    FileTooLarge,
    WorkspaceLimitExceeded,
    InternalError,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DocumentAccess {
    ReadOnly,
    Editable { lease_id: LeaseId },
}

impl DocumentAccess {
    pub const fn lease_id(&self) -> Option<LeaseId> {
        match self {
            Self::ReadOnly => None,
            Self::Editable { lease_id } => Some(*lease_id),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum EditOperation {
    Insert { byte_offset: u64, text: String },
    Delete { start: u64, end: u64 },
    Replace { start: u64, end: u64, text: String },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum EditorIntent {
    InsertText { byte_offset: u64, text: String },
    DeleteRange { start: u64, end: u64 },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BehaviorManifest {
    pub manifest_id: String,
    pub behavior_version: BehaviorVersion,
    pub scope: BehaviorScope,
    /// Clay-owned default typography role for this document. Decorations may
    /// replace it only with a validated syntax/semantic document role.
    pub document_font_role: DocumentFontRole,
    pub keymaps: Vec<KeyBindingRule>,
    pub commands: Vec<CommandDeclaration>,
    pub editor_rules: EditorBehaviorRules,
}

impl BehaviorManifest {
    pub fn minimal_text_editing(behavior_version: BehaviorVersion) -> Self {
        Self {
            manifest_id: "clay.default.text".to_string(),
            behavior_version,
            scope: BehaviorScope::GlobalDefault,
            document_font_role: DocumentFontRole::Proportional,
            keymaps: default_keymaps(),
            commands: default_commands(),
            editor_rules: EditorBehaviorRules::default_text(),
        }
    }

    /// Default manifest shipped by the always-on built-in `core.code` fallback
    /// major mode. Same keybindings and commands as
    /// [`Self::minimal_text_editing`], but with the code-oriented editor rules
    /// ([`EditorBehaviorRules::default_code`], including electric-character
    /// reflow) so generic code editing works with no package loaded.
    pub fn core_code_editing(behavior_version: BehaviorVersion) -> Self {
        Self {
            manifest_id: "clay.default.code".to_string(),
            behavior_version,
            scope: BehaviorScope::GlobalDefault,
            document_font_role: DocumentFontRole::Monospace,
            keymaps: default_keymaps(),
            commands: default_commands(),
            editor_rules: EditorBehaviorRules::default_code(),
        }
    }

    pub fn allows_client_first_edit(&self, operation: &EditOperation) -> bool {
        self.editor_rules.text_edits.iter().any(|capability| {
            matches!(
                (operation, capability),
                (EditOperation::Insert { .. }, TextEditCapability::Insert)
                    | (EditOperation::Delete { .. }, TextEditCapability::Delete)
                    | (EditOperation::Replace { .. }, TextEditCapability::Replace)
            )
        })
    }
}

fn default_keymaps() -> Vec<KeyBindingRule> {
    vec![
        KeyBindingRule::single("text.insert_newline", KeyCode::Enter),
        KeyBindingRule::single("text.insert_tab", KeyCode::Tab),
    ]
}

fn default_commands() -> Vec<CommandDeclaration> {
    vec![
        CommandDeclaration::client_edit("text.insert", "Insert Text"),
        CommandDeclaration::client_edit("text.delete", "Delete Text"),
        CommandDeclaration::client_edit("text.replace", "Replace Text"),
        CommandDeclaration::client_edit("text.insert_newline", "Insert Newline"),
        CommandDeclaration::client_edit("text.insert_tab", "Insert Tab"),
        CommandDeclaration::ui_reactive("completion.trigger", "Trigger Completion"),
    ]
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BehaviorScope {
    GlobalDefault,
    Document { document_id: DocumentId },
    Language { language_id: String },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct KeyBindingRule {
    pub command_id: String,
    pub sequence: Vec<KeyStroke>,
    pub context: KeyBindingContext,
    pub routing_policy: RoutingPolicy,
}

impl KeyBindingRule {
    pub fn single(command_id: impl Into<String>, key: KeyCode) -> Self {
        Self {
            command_id: command_id.into(),
            sequence: vec![KeyStroke::new(key)],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientFirstPredictable,
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct KeyStroke {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyStroke {
    pub const fn new(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::NONE,
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum KeyCode {
    Character(String),
    Enter,
    Tab,
    Backspace,
    Delete,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alt: false,
        super_key: false,
    };
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum KeyBindingContext {
    EditorTextFocus,
    CompletionMenu,
    Global,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CommandDeclaration {
    pub command_id: String,
    pub display_name: String,
    pub routing_policy: RoutingPolicy,
    pub authority: CommandAuthority,
}

impl CommandDeclaration {
    pub fn client_edit(command_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            display_name: display_name.into(),
            routing_policy: RoutingPolicy::ClientFirstPredictable,
            authority: CommandAuthority::BuiltInClientEdit,
        }
    }

    pub fn server_intent(command_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            display_name: display_name.into(),
            routing_policy: RoutingPolicy::ServerFirst,
            authority: CommandAuthority::ServerIntent,
        }
    }

    pub fn ui_reactive(command_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            display_name: display_name.into(),
            routing_policy: RoutingPolicy::UiReactivePriority,
            authority: CommandAuthority::ServerIntent,
        }
    }

    pub fn client_ui(command_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            display_name: display_name.into(),
            routing_policy: RoutingPolicy::ClientUiCommand,
            authority: CommandAuthority::ClientUi,
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CommandAuthority {
    BuiltInClientEdit,
    ServerIntent,
    ClientUi,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RoutingPolicy {
    ClientFirstPredictable,
    ClientFirstRequiresAck,
    ServerFirst,
    ServerFirstWithLock { lock_scope: LockScope },
    ClientUiCommand,
    UiReactivePriority,
    Background,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LockScope {
    Range,
    Document,
    Behavior,
    Workspace,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EditorBehaviorRules {
    pub text_edits: Vec<TextEditCapability>,
    pub enter: EnterRule,
    pub tab: TabRule,
    pub pairs: Vec<PairRule>,
    pub comments: Vec<CommentContinuationRule>,
    /// Generic electric-character rules. Each rule reflows the current line
    /// locally when its trigger character is typed, e.g. outdenting a line so a
    /// closing `}` aligns with its opener. Any future language package can
    /// declare its own trigger/effect parameters; no language-specific Rust
    /// branch is consulted.
    pub electric_characters: Vec<ElectricCharacterRule>,
    pub autocomplete_triggers: Vec<AutocompleteTrigger>,
}

impl EditorBehaviorRules {
    /// Generic plain-text rule set shipped by the always-on built-in
    /// [`crate::packages::modes::core_text_mode`] fallback. No electric
    /// characters: plain text has no block structure to reflow.
    pub fn default_text() -> Self {
        Self {
            text_edits: vec![
                TextEditCapability::Insert,
                TextEditCapability::Delete,
                TextEditCapability::Replace,
            ],
            enter: EnterRule::PreserveLeadingWhitespace,
            tab: TabRule {
                mode: TabMode::InsertSpaces,
                spaces_per_tab: 4,
            },
            pairs: vec![
                PairRule::new("(", ")"),
                PairRule::new("[", "]"),
                PairRule::new("{", "}"),
                PairRule::new("\"", "\""),
                PairRule::new("'", "'"),
            ],
            comments: vec![CommentContinuationRule {
                line_prefix: "//".to_string(),
                continue_prefix: "// ".to_string(),
            }],
            electric_characters: Vec::new(),
            autocomplete_triggers: vec![AutocompleteTrigger {
                trigger: ".".to_string(),
                routing_policy: RoutingPolicy::UiReactivePriority,
            }],
        }
    }

    /// Generic code-oriented rule set shipped by the always-on built-in
    /// [`crate::packages::modes::core_code_mode`] fallback. Identical to
    /// [`Self::default_text`] for indentation, pairs, and comment
    /// continuation, plus electric-character reflow for the common closing
    /// brackets so a typed `}`/`)`/`]` aligns with its opener without a server
    /// round trip. Language packages extend or override these parameters via
    /// manifest data.
    pub fn default_code() -> Self {
        let mut rules = Self::default_text();
        rules.electric_characters = vec![
            ElectricCharacterRule::outdent("}"),
            ElectricCharacterRule::outdent(")"),
            ElectricCharacterRule::outdent("]"),
        ];
        rules
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TextEditCapability {
    Insert,
    Delete,
    Replace,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum EnterRule {
    /// Copy the indentation of the previous line only.
    PreserveLeadingWhitespace,
    /// Insert a bare newline with no indentation.
    InsertNewlineOnly,
    /// After a line beginning with one of the given `markers`, insert a new
    /// line that repeats the same marker and indentation.  When the current
    /// item body is empty and `exit_on_empty_item` is true, remove the marker
    /// instead ("exit" the list).  Any mode whose syntax has list-like
    /// continuation (Markdown, AsciiDoc, Org-mode, RST, …) uses this variant
    /// by declaring its own marker strings — no mode-specific Rust code needed.
    ContinueLineMarkers {
        /// Prefix strings that trigger continuation, e.g. `["-", "*", "+"]`
        /// or `["1.", "2."]` for ordered lists.  The special token
        /// `"ordered-dot"` signals the engine to increment the numeric prefix.
        markers: Vec<String>,
        /// Remove the marker rather than repeating it when the current item
        /// body is empty.
        exit_on_empty_item: bool,
    },
    /// Inside a fenced block opened by one of `fence_markers` (e.g. `"```"`,
    /// `"~~~"`), copy the indentation of the first non-fence body line instead
    /// of the leading whitespace of the fence line.  Any mode with fenced
    /// constructs (Markdown, RST, AsciiDoc code blocks, …) can use this
    /// variant by declaring its own fence delimiter strings.
    PreserveFenceBodyIndent {
        /// Opening/closing fence delimiter strings, e.g. `["```", "~~~"]`.
        fence_markers: Vec<String>,
    },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TabRule {
    pub mode: TabMode,
    pub spaces_per_tab: u8,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TabMode {
    InsertSpaces,
    InsertTabCharacter,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PairRule {
    pub open: String,
    pub close: String,
    pub when: PairRuleContext,
}

impl PairRule {
    pub fn new(open: impl Into<String>, close: impl Into<String>) -> Self {
        Self {
            open: open.into(),
            close: close.into(),
            when: PairRuleContext::CaretOrSelection,
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PairRuleContext {
    CaretOrSelection,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CommentContinuationRule {
    pub line_prefix: String,
    pub continue_prefix: String,
}

/// Deterministic local reflow applied when an electric-character trigger is
/// typed. Executed entirely by Rust-known transform engines on the client from
/// manifest data; no callbacks, JavaScript, or IPC are involved.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ElectricEffect {
    /// Outdent the current line by one indentation unit when the trigger is
    /// typed as the first non-whitespace character on an over-indented line,
    /// so a closing bracket aligns with the block opener.
    OutdentOneLevel,
}

/// A generic electric-character rule. The trigger is a single character (e.g.
/// `}`); the effect is a declarative reflow. Any language package can declare
/// its own rules; the Rust client executes only [`ElectricEffect`] variants it
/// knows, so packages contribute rule parameters only.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ElectricCharacterRule {
    pub trigger: String,
    pub effect: ElectricEffect,
}

impl ElectricCharacterRule {
    /// Convenience constructor for the common outdent-on-close case.
    pub fn outdent(trigger: impl Into<String>) -> Self {
        Self {
            trigger: trigger.into(),
            effect: ElectricEffect::OutdentOneLevel,
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AutocompleteTrigger {
    pub trigger: String,
    pub routing_policy: RoutingPolicy,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RegionLockConflict {
    pub lock_id: RegionLockId,
    pub start: u64,
    pub end: u64,
    pub owner: LockOwner,
    pub created_at_version: DocumentVersion,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LockOwner {
    Server,
    Client { client_id: ClientId },
    Extension { extension_id: String },
    AiAgent { agent_id: String },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
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

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
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
    DecorationViewportRequest {
        client_id: ClientId,
        document_id: DocumentId,
        document_version: DocumentVersion,
        byte_start: u64,
        byte_end: u64,
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
    /// document text); the server-side provider lane stale-drops older
    /// requests against `document_version`/`behavior_version`/
    /// `provider_generation`.
    CompletionRequest {
        request: CompletionRequest,
    },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}

impl RuntimeDiagnostic {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Wire form of one inert text-style override declared by a theme package
/// (`clay.contributions.textStyles`). Colors travel as RGBA bytes so the
/// protocol never depends on a peniko `Color`; the client reconstructs a
/// [`crate::editor::theme::StyleRegistry`] at the point the active theme is
/// applied. This is pure style data: no code, ops, widgets, or CSS (Plan 046,
/// decision 2026-07-09-0352).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TextThemeOverride {
    /// Override target: a [`TokenType`] variant name or a base-UI color key
    /// (e.g. `Keyword`, `panelBg`).
    pub token: String,
    /// RGBA override, present only when the entry declares a color.
    pub color: Option<[u8; 4]>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// Owning theme package api prefix (provenance).
    pub provenance: String,
}

/// A bounded ordered font-family fallback stack and logical-pixel size.
pub const MAX_FONT_FAMILIES_PER_PROFILE: usize = 8;
pub const MAX_FONT_FAMILY_BYTES: usize = 128;
pub const MIN_FONT_SIZE: f32 = 6.0;
pub const MAX_FONT_SIZE: f32 = 96.0;

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct FontProfile {
    pub families: Vec<String>,
    pub size: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontProfileValidationError {
    EmptyFamilyStack,
    TooManyFamilies,
    EmptyFamily,
    FamilyTooLong,
    ControlCharacter,
    MissingGenericFallback,
    InvalidSize,
}

impl FontProfile {
    pub fn validate(&self) -> Result<(), FontProfileValidationError> {
        if self.families.is_empty() {
            return Err(FontProfileValidationError::EmptyFamilyStack);
        }
        if self.families.len() > MAX_FONT_FAMILIES_PER_PROFILE {
            return Err(FontProfileValidationError::TooManyFamilies);
        }
        for family in &self.families {
            if family.trim().is_empty() {
                return Err(FontProfileValidationError::EmptyFamily);
            }
            if family.len() > MAX_FONT_FAMILY_BYTES {
                return Err(FontProfileValidationError::FamilyTooLong);
            }
            if family.chars().any(char::is_control) {
                return Err(FontProfileValidationError::ControlCharacter);
            }
        }
        if !self
            .families
            .last()
            .is_some_and(|family| is_generic_font_family(family))
        {
            return Err(FontProfileValidationError::MissingGenericFallback);
        }
        if !self.size.is_finite() || !(MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&self.size) {
            return Err(FontProfileValidationError::InvalidSize);
        }
        Ok(())
    }
}

fn is_generic_font_family(family: &str) -> bool {
    matches!(
        family,
        "system-ui" | "serif" | "sans-serif" | "monospace" | "cursive" | "fantasy"
    )
}

/// Complete typography snapshot transported separately from [`ActiveTheme`].
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct ActiveTypography {
    pub revision: u64,
    pub monospace: FontProfile,
    pub proportional: FontProfile,
    pub ui: FontProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveTypographyValidationError {
    InvalidProfile {
        role: FontRole,
        source: FontProfileValidationError,
    },
}

impl ActiveTypography {
    pub fn profile(&self, role: FontRole) -> &FontProfile {
        match role {
            FontRole::Monospace => &self.monospace,
            FontRole::Proportional => &self.proportional,
            FontRole::Ui => &self.ui,
        }
    }

    pub fn validate(&self) -> Result<(), ActiveTypographyValidationError> {
        for role in [FontRole::Monospace, FontRole::Proportional, FontRole::Ui] {
            self.profile(role).validate().map_err(|source| {
                ActiveTypographyValidationError::InvalidProfile { role, source }
            })?;
        }
        Ok(())
    }
}

impl Default for ActiveTypography {
    fn default() -> Self {
        Self {
            revision: 0,
            monospace: FontProfile {
                families: vec!["monospace".to_string()],
                size: 20.0,
            },
            proportional: FontProfile {
                families: vec!["sans-serif".to_string()],
                size: 20.0,
            },
            ui: FontProfile {
                families: vec!["system-ui".to_string()],
                size: 12.0,
            },
        }
    }
}

/// Resolved active theme snapshot shipped from the server (which owns package
/// records) to the client (which owns the editor `StyleRegistry`). Sent once
/// during the welcome handshake when `setTheme("...")` ran in `init.js`; the
/// client reconstructs and installs the registry before/at startup paint.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ActiveTheme {
    /// Selected package specifier (e.g. `@clay/theme-gruvbox-material-dark`).
    pub specifier: String,
    /// Inert text-style + base-UI-color overrides for the selected theme.
    pub overrides: Vec<TextThemeOverride>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub enum ServerMessage {
    Welcome {
        client_id: ClientId,
        protocol_version: u32,
    },
    InitialDocument {
        document_id: DocumentId,
        version: DocumentVersion,
        text: String,
        access: DocumentAccess,
        lease_id: Option<LeaseId>,
    },
    BehaviorManifest(BehaviorManifest),
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
    DiagnosticSet(DiagnosticSet),
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
        text: String,
        access: DocumentAccess,
        lease_id: Option<LeaseId>,
    },
    DocumentOpened {
        metadata: DocumentMetadata,
        text: String,
    },
    DocumentSaved {
        document_id: DocumentId,
        version: DocumentVersion,
        dirty: bool,
    },
    DocumentReloaded {
        metadata: DocumentMetadata,
        text: String,
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
    /// Phase 18.15 (Plan 046) resolved active theme snapshot. Sent once after
    /// the welcome `BehaviorManifest` when `setTheme("...")` ran in `init.js`;
    /// absent when no theme is selected (Clay default theme applies). The
    /// client reconstructs the `StyleRegistry` from the inert overrides.
    ActiveTheme(ActiveTheme),
    /// User-owned typography snapshot. It is independently revisioned because
    /// family/size changes affect shaping and geometry, unlike theme colors.
    ActiveTypography(ActiveTypography),
    Error {
        code: ProtocolErrorCode,
        message: String,
    },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolErrorCode {
    UnsupportedProtocolVersion,
    InvalidMessage,
    AccessDenied,
    InternalError,
}
