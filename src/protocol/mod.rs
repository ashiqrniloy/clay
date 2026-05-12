pub mod codec;

/// Current wire protocol version for the local Clay IPC boundary.
pub const PROTOCOL_VERSION: u32 = 1;

pub type ClientId = u64;
pub type DocumentId = u64;
pub type DocumentVersion = u64;
pub type BehaviorVersion = u64;
pub type TransactionId = u64;
pub type LeaseId = u64;
pub type RegionLockId = u64;

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
            keymaps: default_keymaps(),
            commands: default_commands(),
            editor_rules: EditorBehaviorRules::default_text(),
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
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CommandAuthority {
    BuiltInClientEdit,
    ServerIntent,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RoutingPolicy {
    ClientFirstPredictable,
    ClientFirstRequiresAck,
    ServerFirst,
    ServerFirstWithLock { lock_scope: LockScope },
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
    pub autocomplete_triggers: Vec<AutocompleteTrigger>,
}

impl EditorBehaviorRules {
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
            autocomplete_triggers: vec![AutocompleteTrigger {
                trigger: ".".to_string(),
                routing_policy: RoutingPolicy::UiReactivePriority,
            }],
        }
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
    PreserveLeadingWhitespace,
    InsertNewlineOnly,
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
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
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
