pub mod codec;
pub mod completion;
pub mod decorations;
pub mod diagnostics;
pub mod editor_control;
pub mod folding;
pub mod language_intelligence;
pub mod menu;
pub mod parse;
pub mod runtime;
pub mod sdui;
pub mod textobjects;

pub use completion::*;
pub use decorations::*;
pub use diagnostics::*;
pub use editor_control::*;
pub use folding::*;
pub use language_intelligence::*;
pub use menu::*;
pub use parse::*;
pub use runtime::*;
pub use sdui::*;
pub use textobjects::*;

/// Current wire protocol version for the local Clay IPC boundary.
///
/// Version 2 added `DecorationViewportRequest`; version 3 adds grouped native
/// decoration chunks and removes grammar-recovery diagnostics. Version 5 adds
/// `DecorationBatch` so one parse update's chunks ship in a single frame.
/// Version 7 adds `SelectionQueryRequest`/`SelectionQueryResult` for
/// tree-sitter text objects and smart select (Plan 071 task 10).
/// Version 8 adds `EditorCommandRequest` for the gated `editor-control`
/// programmatic execution channel (Plan 071 follow-up round).
/// Version 9 adds `CaretStyleOverride` so the gated `clientSetCursorStyle`
/// runtime override reaches the client (Plan 071 caret-transport fix).
/// Version 10 adds `ShellPreferences` so the `setPaneFocusPolicy` configuration
/// option reaches the client shell widget (Phase 22.1).
/// Version 11 adds the server-authoritative tab registry: `TabCommand`
/// (new/open-workspace/close/activate/reclaim) and the `TabRegistry` snapshot
/// broadcast so each tab's connection sees the same tab order/active tab
/// (Phase 22.3).
/// Version 13 adds tab reorder commands (`MoveLeft`/`MoveRight`/`MoveTo`)
/// so registry order becomes server-authoritative and reorderable via the
/// Phase 22.4 keyboard tab commands.
/// Version 15 defers `InitialDocument` and the initial SDUI/file-browser
/// snapshot until the connection binds a tab with `TabCommand::New` or
/// `TabCommand::Reclaim`.
/// Version 16 (Phase 24.3) adds the generic semantic `MenuBackspace` intent
/// and the `MenuActivate` activation kind (`Primary`/`Secondary`).
/// Version 17 (Phase 24.4) adds `TransientMenuOriginData::Centered` so
/// command/path mode snapshots can select the window-centered Command Centre
/// surface (client-side layout/presentation only).
/// Version 18 (Phase 26) adds `EditorLayoutOverride` so the user-owned
/// `setEditorLayout` wrap-policy override (init.js / configuration reload)
/// reaches every client editor surface, beating the per-mode manifest.
/// Version 19 (Phase 28.2) adds `EditorBehaviorRules.heading_prefixes` so
/// heading rotate is package data (no ATX literals in Rust).
/// Version 20 (Phase 28.3) adds `FoldingRangeSet` so validated folds reach
/// the client; collapse state stays client-local.
/// Version 21 (Phase 28.4) adds `DecorationKind::Link` plus optional
/// `DecorationTarget` on `DecorationSpan`.
/// Version 22 (Phase 28.5) adds `DecorationKind::InlayHint`, inlay payload,
/// and `EditorChrome.inlay_hints`.
/// Version 23 (Phase 28.6) adds bounded completion recency hints to
/// `CompletionRequest`; the ring is process-local and never persisted.
/// Older server processes must not retain the previous wire semantics.
pub const PROTOCOL_VERSION: u32 = 23;

pub type ClientId = u64;
pub type DocumentId = u64;
pub type DocumentVersion = u64;
/// Phase 22.3: stable server-assigned tab identity. Tabs are real separate
/// client connections; the registry binds a `TabId` to a `ClientId` and a
/// workspace root. Survives client reconnects (the binding is re-pointed at the
/// reconnecting connection's `ClientId`).
pub type TabId = u64;
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

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
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
            manifest_id: "default.text".to_string(),
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
            manifest_id: "default.code".to_string(),
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
    let mut rules = vec![
        KeyBindingRule::single("text.insert_newline", KeyCode::Enter),
        KeyBindingRule::single("text.insert_tab", KeyCode::Tab),
        KeyBindingRule {
            command_id: "editor.toggleComment".to_string(),
            sequence: vec![ctrl_key(KeyCode::Character("/".to_string()))],
            context: KeyBindingContext::EditorTextFocus,
            routing_policy: RoutingPolicy::ClientFirstPredictable,
        },
        KeyBindingRule::default_reload_configuration(),
        // Phase 24.5: the Command Centre opens on the Emacs-like `Ctrl+X
        // Ctrl+P` chord (P = palette), routed through the same server-intent
        // lane as the Phase 24.2 single-stroke default. Global scope fires
        // outside editor text focus; overridable via bindKey/unbindKey like
        // every default.
        KeyBindingRule::global_server_first_sequence(
            "controlCenter.open",
            vec![
                ctrl_key(KeyCode::Character("x".to_string())),
                ctrl_key(KeyCode::Character("p".to_string())),
            ],
        ),
        // Phase 24.5: Path Mode's default is the Emacs-like `Ctrl+X Ctrl+F`
        // chord (find-file family: filesystem browsing), divergent from the
        // Command Centre chord at the second stroke so neither shadows the
        // other. Same command id, context, and ServerFirst routing as the
        // Phase 24.3 single-stroke default; fully rebindable/removable via
        // bindKey/unbindKey like every default.
        KeyBindingRule::global_server_first_sequence(
            "controlCenter.openPath",
            vec![
                ctrl_key(KeyCode::Character("x".to_string())),
                ctrl_key(KeyCode::Character("f".to_string())),
            ],
        ),
        // Phase 22.1: shell pane-management defaults (all overridable via bindKey
        // in init.js with { scope: "global" }). "vertical" = side by side,
        // "horizontal" = stacked (vim-style vsplit / split).
        KeyBindingRule::global_client_ui(
            "shell.clientSplitPaneVertical",
            ctrl_key(KeyCode::Character("\\".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientSplitPaneHorizontal",
            ctrl_key(KeyCode::Character("-".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientAddEqualPane",
            ctrl_shift_key(KeyCode::Character("\\".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientClosePane",
            ctrl_alt_key(KeyCode::Character("w".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientFocusPanePrev",
            ctrl_alt_key(KeyCode::ArrowLeft),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientFocusPaneNext",
            ctrl_alt_key(KeyCode::ArrowRight),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientResizePaneLeft",
            ctrl_alt_shift_key(KeyCode::ArrowLeft),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientResizePaneRight",
            ctrl_alt_shift_key(KeyCode::ArrowRight),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientResizePaneUp",
            ctrl_alt_shift_key(KeyCode::ArrowUp),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientResizePaneDown",
            ctrl_alt_shift_key(KeyCode::ArrowDown),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientMovePanePrev",
            ctrl_alt_key(KeyCode::Character("[".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientMovePaneNext",
            ctrl_alt_key(KeyCode::Character("]".to_string())),
        ),
        // Phase 22.4: shell tab-management defaults (all overridable via
        // bindKey in init.js with { scope: "global" }). Policies: next/prev
        // wrap around the tab order; activation by number is 1-based
        // (Ctrl+<N>); move left/right are boundary no-ops (no wraparound) and
        // use the bracket family (Ctrl+Shift+[ / ] — Ctrl+Alt+[ / ] are the
        // pane moves); move-to-position uses Ctrl+Shift+<N>; numbered families
        // exist for 1..=9 only — "beyond 9" is not a command ID. Chords are
        // the parseable set (single characters + Tab + arrows; the chord
        // parser has no PageUp/PageDown/F-keys).
        KeyBindingRule::global_client_ui("shell.clientTabNext", ctrl_key(KeyCode::Tab)),
        KeyBindingRule::global_client_ui("shell.clientTabPrev", ctrl_shift_key(KeyCode::Tab)),
        KeyBindingRule::global_client_ui(
            "shell.clientTabNew",
            ctrl_key(KeyCode::Character("t".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientTabClose",
            ctrl_shift_key(KeyCode::Character("w".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientTabMoveLeft",
            ctrl_shift_key(KeyCode::Character("[".to_string())),
        ),
        KeyBindingRule::global_client_ui(
            "shell.clientTabMoveRight",
            ctrl_shift_key(KeyCode::Character("]".to_string())),
        ),
    ];
    for n in 1..=9 {
        rules.push(KeyBindingRule::global_client_ui(
            format!("shell.clientTabActivate.{n}"),
            ctrl_key(KeyCode::Character(n.to_string())),
        ));
        rules.push(KeyBindingRule::global_client_ui(
            format!("shell.clientTabMoveTo.{n}"),
            ctrl_shift_key(KeyCode::Character(n.to_string())),
        ));
    }
    rules
}

/// `Ctrl+<key>` stroke for default keymaps.
fn ctrl_key(key: KeyCode) -> KeyStroke {
    KeyStroke {
        key,
        modifiers: KeyModifiers {
            control: true,
            ..KeyModifiers::NONE
        },
    }
}

/// `Ctrl+Shift+<key>` stroke for default keymaps.
fn ctrl_shift_key(key: KeyCode) -> KeyStroke {
    KeyStroke {
        key,
        modifiers: KeyModifiers {
            control: true,
            shift: true,
            ..KeyModifiers::NONE
        },
    }
}

/// `Ctrl+Alt+<key>` stroke for default keymaps.
fn ctrl_alt_key(key: KeyCode) -> KeyStroke {
    KeyStroke {
        key,
        modifiers: KeyModifiers {
            control: true,
            alt: true,
            ..KeyModifiers::NONE
        },
    }
}

/// `Ctrl+Alt+Shift+<key>` stroke for default keymaps.
fn ctrl_alt_shift_key(key: KeyCode) -> KeyStroke {
    KeyStroke {
        key,
        modifiers: KeyModifiers {
            control: true,
            alt: true,
            shift: true,
            ..KeyModifiers::NONE
        },
    }
}

fn default_commands() -> Vec<CommandDeclaration> {
    let mut commands = vec![
        CommandDeclaration::client_edit("text.insert", "Insert Text"),
        CommandDeclaration::client_edit("text.delete", "Delete Text"),
        CommandDeclaration::client_edit("text.replace", "Replace Text"),
        CommandDeclaration::client_edit("text.insert_newline", "Insert Newline"),
        CommandDeclaration::client_edit("text.insert_tab", "Insert Tab"),
        CommandDeclaration::client_edit("editor.toggleComment", "Toggle Comment"),
        CommandDeclaration::client_edit("editor.toggleListMarker", "Toggle List Marker"),
        CommandDeclaration::client_edit("editor.rotateHeading", "Rotate Heading"),
        CommandDeclaration::client_ui("editor.clientToggleFold", "Toggle Fold"),
        CommandDeclaration::client_ui("editor.toggleInlayHints", "Toggle Inlay Hints"),
        CommandDeclaration {
            command_id: "runtime.reloadConfiguration".to_string(),
            display_name: "Reload Configuration and Packages".to_string(),
            routing_policy: RoutingPolicy::ServerFirstWithLock {
                lock_scope: LockScope::Behavior,
            },
            authority: CommandAuthority::ServerIntent,
        },
        // Phase 24.2: the Control Center opens via the command-intent lane
        // (server-owned menu session); declared like any built-in server
        // intent so the default Global `Ctrl+X Ctrl+P` chord routes.
        CommandDeclaration::server_intent("controlCenter.open", "Open Control Center"),
        // Phase 24.3: Path Mode (dired-style filesystem browsing) opens via
        // the same command-intent lane; default Global `Ctrl+X Ctrl+F` chord
        // (Phase 24.5), same command id as the temporary single-stroke
        // default.
        CommandDeclaration::server_intent("controlCenter.openPath", "Browse Filesystem"),
        CommandDeclaration::ui_reactive("completion.trigger", "Trigger Completion"),
        // Phase 18.20: discoverable language-intelligence commands with empty
        // default key bindings. Client captures cursor/version locally and
        // enqueues LanguageIntelligenceRequest (UI-reactive, like completion).
        CommandDeclaration::ui_reactive("language.hover", "Hover"),
        CommandDeclaration::ui_reactive("language.goToDefinition", "Go to Definition"),
        CommandDeclaration::ui_reactive("language.codeActions", "Code Actions"),
        CommandDeclaration::ui_reactive("language.signatureHelp", "Signature Help"),
        // Phase 22.1: shell pane-management commands (ClientUi authority).
        CommandDeclaration::client_ui("shell.clientSplitPaneVertical", "Split Pane Vertical"),
        CommandDeclaration::client_ui("shell.clientSplitPaneHorizontal", "Split Pane Horizontal"),
        CommandDeclaration::client_ui("shell.clientAddEqualPane", "Add Equal Pane"),
        CommandDeclaration::client_ui("shell.clientClosePane", "Close Pane"),
        CommandDeclaration::client_ui("shell.clientFocusPaneNext", "Focus Next Pane"),
        CommandDeclaration::client_ui("shell.clientFocusPanePrev", "Focus Previous Pane"),
        CommandDeclaration::client_ui("shell.clientResizePaneLeft", "Resize Pane Left"),
        CommandDeclaration::client_ui("shell.clientResizePaneRight", "Resize Pane Right"),
        CommandDeclaration::client_ui("shell.clientResizePaneUp", "Resize Pane Up"),
        CommandDeclaration::client_ui("shell.clientResizePaneDown", "Resize Pane Down"),
        CommandDeclaration::client_ui("shell.clientMovePaneNext", "Move Pane Next"),
        CommandDeclaration::client_ui("shell.clientMovePanePrev", "Move Pane Previous"),
        // Phase 22.4: shell tab-management commands (ClientUi authority; Global
        // keybindings in default_keymaps). Numbered families are 1-based
        // positions in the current tab order; only 1..=9 exist.
        CommandDeclaration::client_ui("shell.clientTabNext", "Next Tab"),
        CommandDeclaration::client_ui("shell.clientTabPrev", "Previous Tab"),
        CommandDeclaration::client_ui("shell.clientTabNew", "New Tab"),
        CommandDeclaration::client_ui("shell.clientTabClose", "Close Tab"),
        CommandDeclaration::client_ui("shell.clientTabMoveLeft", "Move Tab Left"),
        CommandDeclaration::client_ui("shell.clientTabMoveRight", "Move Tab Right"),
    ];
    for n in 1..=9 {
        commands.push(CommandDeclaration::client_ui(
            format!("shell.clientTabActivate.{n}"),
            format!("Activate Tab {n}"),
        ));
        commands.push(CommandDeclaration::client_ui(
            format!("shell.clientTabMoveTo.{n}"),
            format!("Move Tab to Position {n}"),
        ));
    }
    commands
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

    /// Phase 23: the built-in global configuration reload binding. It uses
    /// the same behavior lock as the command's server-side routing policy.
    pub(crate) fn default_reload_configuration() -> Self {
        Self {
            command_id: "runtime.reloadConfiguration".to_string(),
            sequence: vec![KeyStroke {
                key: KeyCode::Character("r".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::NONE
                },
            }],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirstWithLock {
                lock_scope: LockScope::Behavior,
            },
        }
    }

    /// Phase 22.1: a Global-scope, ClientUiCommand-routed binding (shell pane
    /// commands). Fires even outside editor text focus; overridable via `bindKey`.
    pub fn global_client_ui(command_id: impl Into<String>, stroke: KeyStroke) -> Self {
        Self {
            command_id: command_id.into(),
            sequence: vec![stroke],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ClientUiCommand,
        }
    }

    /// Phase 24.2: a Global-scope, ServerFirst-routed binding (server-intent
    /// commands like `controlCenter.open`). Fires even outside editor text
    /// focus and emits the existing inert `CommandIntent`; overridable via
    /// `bindKey`.
    pub fn global_server_first(command_id: impl Into<String>, stroke: KeyStroke) -> Self {
        Self {
            command_id: command_id.into(),
            sequence: vec![stroke],
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
        }
    }

    /// Phase 24.5: multi-stroke variant of [`Self::global_server_first`]
    /// (Emacs-style chord defaults). The pending-chord matcher resolves the
    /// first stroke and dispatches on the completing stroke.
    pub fn global_server_first_sequence(
        command_id: impl Into<String>,
        sequence: Vec<KeyStroke>,
    ) -> Self {
        Self {
            command_id: command_id.into(),
            sequence,
            context: KeyBindingContext::Global,
            routing_policy: RoutingPolicy::ServerFirst,
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

impl RoutingPolicy {
    /// Parse a package/JSON routing-policy string. Accepts kebab-case and
    /// PascalCase aliases. `ServerFirstWithLock` and `ClientUiCommand` are
    /// host-constructed only (need a lock scope / are not package-declarable).
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "client-first-predictable" | "ClientFirstPredictable" => {
                Ok(Self::ClientFirstPredictable)
            }
            "client-first-requires-ack" | "ClientFirstRequiresAck" => {
                Ok(Self::ClientFirstRequiresAck)
            }
            "server-first" | "ServerFirst" => Ok(Self::ServerFirst),
            "ui-reactive-priority" | "UiReactivePriority" => Ok(Self::UiReactivePriority),
            "background" | "Background" => Ok(Self::Background),
            other => Err(format!("unsupported routingPolicy '{other}'")),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LockScope {
    Range,
    Document,
    Behavior,
    Workspace,
}

/// Word-boundary policy consumed by movement, selection, and completion so
/// they share one classifier. `Code` with `treat_underscore_as_word = true`
/// reproduces the historical `is_completion_word_character` classifier
/// (`_` || Unicode alphanumeric) exactly.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum WordSeparatorPolicy {
    /// Word characters are Unicode alphanumeric; underscore is a word
    /// character iff `treat_underscore_as_word` is true. Punctuation and
    /// whitespace are separators. The code-editing default.
    Code,
    /// Word characters are Unicode alphanumeric; underscore and all punctuation
    /// are separators. Use `Custom` for prose-specific boundaries (e.g.
    /// contractions).
    Prose,
    /// Explicit separator set; a character is a word character iff it is not in
    /// `separators` and not Unicode whitespace. `treat_underscore_as_word` is
    /// ignored.
    Custom(Vec<char>),
}

impl WordSeparatorPolicy {
    /// Classify a character as a word character (true) or separator (false).
    pub fn is_word_char(&self, character: char, treat_underscore_as_word: bool) -> bool {
        match self {
            WordSeparatorPolicy::Code => {
                character.is_alphanumeric() || (treat_underscore_as_word && character == '_')
            }
            WordSeparatorPolicy::Prose => character.is_alphanumeric(),
            WordSeparatorPolicy::Custom(separators) => {
                !character.is_whitespace() && !separators.contains(&character)
            }
        }
    }
}

/// Paragraph boundary style for vertical paragraph motion.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParagraphStyle {
    /// Paragraphs are separated by a truly empty line.
    BlankLine,
    /// Paragraphs are separated by any line that is empty or whitespace-only.
    BlankLineOrWhitespace,
}

/// Logical-line vs wrapped-visual-line vertical motion. `ScreenLine` falls
/// back to `Character` behaviour today; full wrapped-line motion is a future
/// phase that consults the laid-out text. Kept as a named variant so the
/// configuration vocabulary is complete.
/// `ponytail:` ScreenLine behaves as Character until visual-line data is wired.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMovementStyle {
    Character,
    ScreenLine,
}

/// Movement configuration shipped in [`EditorBehaviorRules`]. Language-
/// agnostic; packages override via manifest data, never per-language Rust.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MovementRules {
    pub word_separators: WordSeparatorPolicy,
    pub treat_underscore_as_word: bool,
    pub camel_case_sub_word: bool,
    pub paragraph_style: ParagraphStyle,
    /// When true, forward word-end motion stops at end of line (no cross-line).
    pub stop_at_eol_word_end: bool,
    pub line_movement: LineMovementStyle,
    /// When false, vertical motion moves to the start of the target line instead
    /// of preserving the caret column.
    pub sticky_column: bool,
}

impl MovementRules {
    /// Code-editing default: underscore is a word character, camelCase
    /// sub-word motion is on, whitespace-blank-line paragraphs, sticky column.
    pub fn default_code() -> Self {
        Self {
            word_separators: WordSeparatorPolicy::Code,
            treat_underscore_as_word: true,
            camel_case_sub_word: true,
            paragraph_style: ParagraphStyle::BlankLineOrWhitespace,
            stop_at_eol_word_end: false,
            line_movement: LineMovementStyle::Character,
            sticky_column: true,
        }
    }

    /// Plain-text default: movement is language-agnostic, so the code default
    /// keeps word-jump behaviour predictable across modes.
    pub fn default_text() -> Self {
        Self::default_code()
    }
}

impl Default for MovementRules {
    fn default() -> Self {
        Self::default_code()
    }
}

/// Caret glyph shape. `Bar`/`Line` are a thin vertical stroke, `Block` covers
/// the character cell, `Underline` is a horizontal stroke at the line baseline.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretShape {
    Bar,
    Line,
    Block,
    Underline,
}

/// Caret blink behaviour. `Solid` never hides (the reduced-motion-friendly
/// default). `Blink` is discrete on/off with an initial `wait_ms` idle delay.
/// `Phase`/`Smooth` are named for a future alpha-ramp; today they render with
/// discrete on/off timing derived from `period_ms`.
/// `ponytail:` Phase/Smooth use discrete timing until per-frame alpha is wired.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlinkStyle {
    Solid,
    Blink {
        on_ms: u32,
        off_ms: u32,
        wait_ms: u32,
    },
    Phase {
        period_ms: u32,
    },
    Smooth {
        period_ms: u32,
    },
}

impl BlinkStyle {
    /// True when the caret should animate (anything but `Solid`).
    pub fn animates(&self) -> bool {
        !matches!(self, BlinkStyle::Solid)
    }

    /// The idle delay before the first off-phase, in milliseconds.
    pub fn wait_ms(&self) -> u32 {
        match self {
            BlinkStyle::Solid => 0,
            BlinkStyle::Blink { wait_ms, .. } => *wait_ms,
            BlinkStyle::Phase { .. } | BlinkStyle::Smooth { .. } => 0,
        }
    }

    /// The visible (on) phase duration, in milliseconds.
    pub fn on_ms(&self) -> u32 {
        match self {
            BlinkStyle::Solid => 0,
            BlinkStyle::Blink { on_ms, .. } => *on_ms,
            BlinkStyle::Phase { period_ms } | BlinkStyle::Smooth { period_ms } => period_ms / 2,
        }
    }

    /// The hidden (off) phase duration, in milliseconds.
    pub fn off_ms(&self) -> u32 {
        match self {
            BlinkStyle::Solid => 0,
            BlinkStyle::Blink { off_ms, .. } => *off_ms,
            BlinkStyle::Phase { period_ms } | BlinkStyle::Smooth { period_ms } => period_ms / 2,
        }
    }
}

/// Caret appearance + blink policy shipped in [`EditorBehaviorRules`] and held
/// as the editor-chrome default in the editor `StyleRegistry`. Colour stays
/// theme-owned (`BaseUiColors::caret`); this struct owns shape + blink only so
/// it never carries raw colour. Language-agnostic; packages override via
/// manifest data, never per-language Rust.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct CaretStyle {
    pub shape: CaretShape,
    /// Stroke thickness for Bar/Line/Underline, in pixels.
    pub width_px: f32,
    /// Caret height as a fraction of the line height (1.0 = full line).
    pub height_pct: f32,
    /// When true, `Block` renders an outline instead of a solid fill.
    pub hollow: bool,
    pub blink: BlinkStyle,
    /// Reserved smooth-caret travel time; 0 disables (no travel animation).
    pub smooth_animation_ms: u32,
    /// When true, typing resets the blink to visible and restarts the wait.
    pub stop_blink_on_typing: bool,
}

impl CaretStyle {
    /// Clay default: a solid (non-blinking) 1.5px bar at full line height —
    /// reproduces the historical caret and is the reduced-motion-safe default.
    pub const fn default_bar() -> Self {
        Self {
            shape: CaretShape::Bar,
            width_px: 1.5,
            height_pct: 1.0,
            hollow: false,
            blink: BlinkStyle::Solid,
            smooth_animation_ms: 0,
            stop_blink_on_typing: true,
        }
    }

    /// Wire-validation bounds for untrusted transports: geometry must be
    /// finite and within sane caret ranges, and animating blink phases must
    /// not be degenerate (a zero period would flicker every frame).
    pub fn validate(&self) -> Result<(), CaretStyleValidationError> {
        let finite = self.width_px.is_finite()
            && self.height_pct.is_finite()
            && self.smooth_animation_ms <= MAX_CARET_SMOOTH_ANIMATION_MS;
        let geometry = self.width_px > 0.0
            && self.width_px <= MAX_CARET_WIDTH_PX
            && self.height_pct > 0.0
            && self.height_pct <= MAX_CARET_HEIGHT_PCT;
        let blink = match self.blink {
            BlinkStyle::Solid => true,
            BlinkStyle::Blink {
                on_ms,
                off_ms,
                wait_ms,
            } => {
                on_ms <= MAX_CARET_BLINK_PHASE_MS
                    && off_ms <= MAX_CARET_BLINK_PHASE_MS
                    && wait_ms <= MAX_CARET_BLINK_PHASE_MS
            }
            BlinkStyle::Phase { period_ms } | BlinkStyle::Smooth { period_ms } => {
                (2..=MAX_CARET_BLINK_PHASE_MS).contains(&period_ms)
            }
        };
        if finite && geometry && blink {
            Ok(())
        } else {
            Err(CaretStyleValidationError::OutOfBounds)
        }
    }
}

/// Upper bounds enforced by [`CaretStyle::validate`] on wire payloads.
pub const MAX_CARET_WIDTH_PX: f32 = 64.0;
pub const MAX_CARET_HEIGHT_PCT: f32 = 4.0;
pub const MAX_CARET_BLINK_PHASE_MS: u32 = 60_000;
pub const MAX_CARET_SMOOTH_ANIMATION_MS: u32 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretStyleValidationError {
    OutOfBounds,
}

impl Default for CaretStyle {
    fn default() -> Self {
        Self::default_bar()
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct EditorBehaviorRules {
    pub text_edits: Vec<TextEditCapability>,
    pub enter: EnterRule,
    pub tab: TabRule,
    pub pairs: Vec<PairRule>,
    pub comments: Vec<CommentContinuationRule>,
    /// ATX/setext-style heading prefixes rotated by `editor.rotateHeading`.
    /// Package data only (e.g. `"# "`…`"###### "`); empty means the command
    /// no-ops. No heading literals live in the transform engine.
    pub heading_prefixes: Vec<String>,
    /// Generic electric-character rules. Each rule reflows the current line
    /// locally when its trigger character is typed, e.g. outdenting a line so a
    /// closing `}` aligns with its opener. Any future language package can
    /// declare its own trigger/effect parameters; no language-specific Rust
    /// branch is consulted.
    pub electric_characters: Vec<ElectricCharacterRule>,
    pub autocomplete_triggers: Vec<AutocompleteTrigger>,
    /// Movement policy (word/paragraph/sub-word/non-blank/matching-pair motion).
    /// Defaults reproduce the historical code-editing classifier; existing modes
    /// gain new motion primitives with no behaviour change.
    pub movement: MovementRules,
    /// Per-mode caret appearance/blink override. `None` defers to the editor
    /// `StyleRegistry` default; `clientSetCursorStyle` overrides both at runtime.
    pub caret_style: Option<CaretStyle>,
    /// Per-mode editor chrome. `None` derives from `document_font_role`
    /// (monospace → on, proportional → off).
    pub chrome: Option<EditorChrome>,
    /// Per-mode wrap/column policy. `None` derives from `document_font_role`
    /// (monospace → no wrap, proportional → 72-column measure).
    pub layout: Option<EditorLayoutRules>,
}

/// Wrap + measure. Packages declare this; users can override it client-side.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorLayoutRules {
    pub wrap: WrapPolicy,
}

/// How document text wraps inside the pane.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapPolicy {
    /// No wrap; horizontal scroll. Code default.
    None,
    /// Wrap to the pane content width. Historical default.
    Viewport,
    /// Wrap to `min(pane, column * average-advance)`.
    Column(u16),
}

impl WrapPolicy {
    pub const DEFAULT_COLUMN: u16 = 72;
    pub const MIN_COLUMN: u16 = 16;
    pub const MAX_COLUMN: u16 = 240;

    pub const fn from_font_role(role: DocumentFontRole) -> Self {
        match role {
            DocumentFontRole::Monospace => Self::None,
            DocumentFontRole::Proportional => Self::Column(Self::DEFAULT_COLUMN),
            DocumentFontRole::Inherit => Self::Viewport,
        }
    }

    pub fn clamp_column(cols: u16) -> u16 {
        cols.clamp(Self::MIN_COLUMN, Self::MAX_COLUMN)
    }
}

/// Generic editor chrome toggles. Any mode can declare them; paint is client-side.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorChrome {
    pub gutter: bool,
    pub active_line: bool,
    pub indent_guides: bool,
    pub bracket_match: bool,
    pub inlay_hints: bool,
}

impl EditorChrome {
    pub const fn prose() -> Self {
        Self {
            gutter: false,
            active_line: false,
            indent_guides: false,
            bracket_match: false,
            inlay_hints: false,
        }
    }

    pub const fn code() -> Self {
        Self {
            gutter: true,
            active_line: true,
            indent_guides: true,
            bracket_match: true,
            inlay_hints: true,
        }
    }

    pub const fn from_font_role(role: DocumentFontRole) -> Self {
        match role {
            DocumentFontRole::Monospace => Self::code(),
            DocumentFontRole::Proportional | DocumentFontRole::Inherit => Self::prose(),
        }
    }
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
            heading_prefixes: Vec::new(),
            electric_characters: Vec::new(),
            autocomplete_triggers: vec![AutocompleteTrigger {
                trigger: ".".to_string(),
                routing_policy: RoutingPolicy::UiReactivePriority,
            }],
            movement: MovementRules::default_text(),
            caret_style: None,
            chrome: None,
            layout: None,
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
        session_id: u64,
        query: String,
    },
    /// Generic semantic Backspace (Phase 24.3): the server session decides
    /// whether Backspace deletes query text or ascends (path mode). The
    /// client no longer pops the mirrored query locally; it only mirrors
    /// full-value replacements for the bounded character send path and
    /// resyncs from every snapshot.
    MenuBackspace {
        client_id: ClientId,
        session_id: u64,
    },
    /// Relative selection movement (arrow keys); the server clamps.
    MenuSelectionMove {
        client_id: ClientId,
        session_id: u64,
        delta: i64,
    },
    /// Activate the session's currently selected item. The server holds the
    /// item's action; the client never supplies command payloads. `kind`
    /// distinguishes primary (Enter/Tab) from secondary (Alt+Enter)
    /// activation; kind semantics are interpreted by the session kind.
    MenuActivate {
        client_id: ClientId,
        session_id: u64,
        kind: TransientMenuActivationData,
    },
    /// Dismiss the session. The server drops it and answers
    /// `ServerMessage::TransientMenuClosed`.
    MenuCancel {
        client_id: ClientId,
        session_id: u64,
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

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
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
    /// Optional fill override (`#rrggbbaa`). Theme-resolved only; never a
    /// decoration-span wire field.
    pub background: Option<[u8; 4]>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// Size-ladder thousandths (`1500` = 1.5). Theme-owned; never a span field.
    pub scale: Option<u16>,
    /// Owning theme package api prefix (provenance).
    pub provenance: String,
}

/// A bounded ordered font-family fallback stack and logical-pixel size.
pub const MAX_FONT_FAMILIES_PER_PROFILE: usize = 8;
pub const MAX_FONT_FAMILY_BYTES: usize = 128;
pub const MIN_FONT_SIZE: f32 = 6.0;
pub const MAX_FONT_SIZE: f32 = 96.0;

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Default)]
pub struct FontProfile {
    pub families: Vec<String>,
    pub size: f32,
    /// Semantic OpenType feature policy for this profile. Boxed so growth of
    /// `LigaturePolicy` (it carries `Vec<String>` feature lists) does not
    /// inflate the `ServerMessage` union floor that small payloads like
    /// `EditAck` pay. User-owned typography data; packages declare semantic
    /// policy via behavior manifests, never concrete families/sizes.
    pub ligatures: Box<LigaturePolicy>,
}

/// Bounded, semantic OpenType feature policy. Carries no concrete family or
/// size data (those stay user-owned per `typography-role-ownership`), only
/// feature toggles a package or user may declare. Resolved client-side into a
/// `parley` `FontSettings<FontFeature>` list at typography install time.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct LigaturePolicy {
    /// Enable standard ligatures (`liga`, `clig`). Default `true` keeps the
    /// historical ligature-on shaping Clay relied on implicitly.
    pub enable_standard: bool,
    /// Enable contextual ligatures (`calt`). Default `true`.
    pub enable_contextual: bool,
    /// Additional discretionary feature tags to enable (value 1), e.g. `ss01`.
    pub discretionary_features: Vec<String>,
    /// Raw CSS feature-settings source passthrough, e.g. `"'calt' 1, 'liga' 0"`.
    /// Parsed by `swash` `Setting::parse_list`; applied before `disable_features`.
    pub raw_features: Option<String>,
    /// Feature tags to force off (value 0), applied last so they override the
    /// semantic toggles and `raw_features`.
    pub disable_features: Vec<String>,
}

impl Default for LigaturePolicy {
    /// Ligatures on by default reproduces the implicit shaping Clay relied on
    /// before feature control was exposed; users or packages opt out by setting
    /// `enable_standard`/`enable_contextual` false or listing `disable_features`.
    fn default() -> Self {
        Self {
            enable_standard: true,
            enable_contextual: true,
            discretionary_features: Vec::new(),
            raw_features: None,
            disable_features: Vec::new(),
        }
    }
}

/// Upper bound on the number of feature tags per kind (`discretionary_features`
/// and `disable_features`). OpenType fonts expose a handful of named features;
/// 32 is generous and keeps the archived payload bounded.
pub const MAX_LIGATURE_FEATURES_PER_KIND: usize = 32;
/// Upper bound on the raw CSS feature-settings source string length.
pub const MAX_LIGATURE_RAW_FEATURE_BYTES: usize = 256;
/// OpenType feature tags are exactly four ASCII bytes; shorter tags are
/// space-padded by `swash::tag_from_str_lossy`.
pub const MAX_LIGATURE_FEATURE_NAME_BYTES: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontProfileValidationError {
    EmptyFamilyStack,
    TooManyFamilies,
    EmptyFamily,
    FamilyTooLong,
    ControlCharacter,
    MissingGenericFallback,
    InvalidSize,
    TooManyDiscretionaryFeatures,
    TooManyDisabledFeatures,
    RawFeaturesTooLong,
    InvalidFeatureName,
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
        self.ligatures.validate()
    }
}

impl LigaturePolicy {
    /// Validate bounds and feature-tag shape. Packages supply these strings, so
    /// they are a trust boundary: counts and lengths are capped and each tag
    /// must be 1..=4 ASCII bytes (no control characters) so it maps to a real
    /// OpenType tag without carrying arbitrary payload.
    pub fn validate(&self) -> Result<(), FontProfileValidationError> {
        if self.discretionary_features.len() > MAX_LIGATURE_FEATURES_PER_KIND {
            return Err(FontProfileValidationError::TooManyDiscretionaryFeatures);
        }
        if self.disable_features.len() > MAX_LIGATURE_FEATURES_PER_KIND {
            return Err(FontProfileValidationError::TooManyDisabledFeatures);
        }
        if let Some(raw) = &self.raw_features
            && raw.len() > MAX_LIGATURE_RAW_FEATURE_BYTES
        {
            return Err(FontProfileValidationError::RawFeaturesTooLong);
        }
        for feature in self
            .discretionary_features
            .iter()
            .chain(&self.disable_features)
        {
            if feature.is_empty()
                || feature.len() > MAX_LIGATURE_FEATURE_NAME_BYTES
                || !feature
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b' ' || byte == b'-')
            {
                return Err(FontProfileValidationError::InvalidFeatureName);
            }
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
    /// Phase 20.1: user-owned bounded hierarchy of UI variant scale ratios
    /// carried atomically with the typography snapshot. Packages/components
    /// select a semantic role plus variant only; concrete scales stay
    /// user-owned here. Defaults preserve the legacy `Title = 14/12`,
    /// `Body/Status = 1`, `Detail = 10/12` behavior.
    pub hierarchy: UiTypographyHierarchy,
}

/// Bounded hierarchy of UI text-variant scale ratios. Each scale multiplies
/// the selected role's base size; packages cannot supply these values. All
/// scales must be finite and within `(0, HIERARCHY_SCALE_MAX]`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct UiTypographyHierarchy {
    pub display: f32,
    pub title: f32,
    pub section: f32,
    pub body: f32,
    pub status: f32,
    pub detail: f32,
    pub caption: f32,
}

/// Inclusive upper bound for any hierarchy scale ratio. Generous enough for
/// large display headings while keeping cached geometry within sane layout
/// bounds. Lower bound is exclusive-zero (scales must be strictly positive).
pub const HIERARCHY_SCALE_MIN: f32 = 0.0;
pub const HIERARCHY_SCALE_MAX: f32 = 4.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiTypographyHierarchyValidationError {
    InvalidScale { field: &'static str },
}

impl UiTypographyHierarchy {
    /// Default hierarchy preserving legacy variant metrics plus restrained
    /// defaults for the three new Phase 20.1 variants.
    pub const DEFAULT: Self = Self {
        display: 1.5,
        title: 14.0 / 12.0,
        section: 13.0 / 12.0,
        body: 1.0,
        status: 1.0,
        detail: 10.0 / 12.0,
        caption: 0.75,
    };

    pub fn validate(&self) -> Result<(), UiTypographyHierarchyValidationError> {
        let scales: [(&'static str, f32); 7] = [
            ("display", self.display),
            ("title", self.title),
            ("section", self.section),
            ("body", self.body),
            ("status", self.status),
            ("detail", self.detail),
            ("caption", self.caption),
        ];
        for (field, scale) in scales {
            if !scale.is_finite() || scale <= HIERARCHY_SCALE_MIN || scale > HIERARCHY_SCALE_MAX {
                return Err(UiTypographyHierarchyValidationError::InvalidScale { field });
            }
        }
        Ok(())
    }
}

impl Default for UiTypographyHierarchy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveTypographyValidationError {
    InvalidProfile {
        role: FontRole,
        source: FontProfileValidationError,
    },
    InvalidHierarchy {
        source: UiTypographyHierarchyValidationError,
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
        self.hierarchy
            .validate()
            .map_err(|source| ActiveTypographyValidationError::InvalidHierarchy { source })?;
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
                ligatures: Box::new(LigaturePolicy::default()),
            },
            proportional: FontProfile {
                families: vec!["sans-serif".to_string()],
                size: 20.0,
                ligatures: Box::new(LigaturePolicy::default()),
            },
            ui: FontProfile {
                families: vec!["system-ui".to_string()],
                size: 12.0,
                ligatures: Box::new(LigaturePolicy::default()),
            },
            hierarchy: UiTypographyHierarchy::DEFAULT,
        }
    }
}

/// Typed override value for a UI design token. The variant present must match
/// the core token's type (validated before install). Levels travel as
/// validated names so the protocol stays independent of shell-side level enums.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub enum WireDesignTokenValue {
    /// `color-role` override as RGBA bytes.
    Color([u8; 4]),
    /// `spacing`/`radius`/`dimension`/`motion-duration` override as a finite,
    /// non-negative, bounded scalar.
    Scalar(f64),
    /// `opacity` override as a finite `[0, 1]` scalar.
    Opacity(f32),
    /// `elevation`/`z-level`/`density` override as a validated level name.
    Level(String),
}

/// Bounded inert typed UI design-token override shipped within [`ActiveTheme`].
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct UiDesignTokenOverride {
    /// Core Clay token name being overridden (e.g. `surface.hover`).
    pub token: String,
    /// Typed override value; the variant must match the core token's type.
    pub value: WireDesignTokenValue,
    /// Owning theme package api prefix (provenance).
    pub provenance: String,
}

/// Phase 22.1 shell-level user preferences transported from the server (which
/// evaluates `init.js`) to the client (which owns the `ClayShellWidget`).
/// Currently carries only the pane-focus policy (`"click"` or `"cursor"`).
/// Pure inert data: no callbacks, no JS execution, no native handles.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShellPreferences {
    /// Pane-focus policy: `"click"` (default) or `"cursor"` (focus follows
    /// pointer hover). Validated server-side; the client maps the string to
    /// its `PaneFocusPolicy` enum.
    pub pane_focus_policy: String,
}

/// Phase 22.3: one tab in the server-authoritative tab registry. A tab is a
/// real separate client connection bound to a workspace root; the registry
/// holds order, the active tab, and the per-tab workspace + client binding so
/// tab structure survives client reconnects.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TabEntry {
    pub tab_id: TabId,
    pub workspace_root_id: WorkspaceRootId,
    pub client_id: ClientId,
    /// Workspace root path for this tab (validated by `add_root` at
    /// `TabCommand::New`/`OpenWorkspace` time). The client displays the path's
    /// final segment as the tab card label. Phase 22.3.
    pub workspace_root: String,
}

/// Phase 22.3: full tab registry snapshot. Broadcast to every connection on
/// any mutation and replayed on handshake; the client applies it to its tab
/// bar and per-tab connection map. Inert data only.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TabRegistrySnapshot {
    /// Tab order (server-authoritative; reorderable since Phase 22.4 via
    /// `TabCommand::MoveLeft`/`MoveRight`/`MoveTo`).
    pub tabs: Vec<TabEntry>,
    pub active: Option<TabId>,
    /// Monotonic registry generation: bumped on every mutation. Relays from
    /// different connections can interleave out of order (a connection's
    /// handshake replay races the broadcast of its own pending tab command),
    /// so the client applies a snapshot only when its revision advances.
    pub revision: u64,
}

/// Phase 22.3: server-authoritative tab lifecycle command. `New` opens a tab
/// bound to the connection's `ClientId` and the given workspace root (resolved
/// through the validated `WorkspaceState::add_root` path); `OpenWorkspace`
/// rebinds a tab's workspace; `Close` removes the tab and triggers the bound
/// connection's close; `Activate` sets the active tab; `Reclaim` re-points a
/// tab's `ClientId` binding at the reconnecting connection (local single-client
/// reclaim in 22.3; multi-client reclaim needs client-instance identity, Phase
/// 21).
/// Phase 22.4: `MoveLeft`/`MoveRight` move a tab one position toward the
/// front/back (boundary no-ops — no wraparound); `MoveTo` moves a tab to a
/// 1-based position (`position` outside `1..=tab_count` is rejected). Moves
/// preserve the active tab's status (the registry tracks `active` by `TabId`).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TabCommand {
    New { workspace_root: String },
    OpenWorkspace { tab_id: TabId, root: String },
    Close { tab_id: TabId },
    Activate { tab_id: TabId },
    Reclaim { tab_id: TabId },
    MoveLeft { tab_id: TabId },
    MoveRight { tab_id: TabId },
    MoveTo { tab_id: TabId, position: u32 },
}

/// Resolved active theme snapshot shipped from the server (which owns package
/// records) to the client (which owns the editor `StyleRegistry`). Sent once
/// during the welcome handshake when `setTheme("...")` ran in `init.js`; the
/// client reconstructs and installs the registry before/at startup paint.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq)]
pub struct ActiveTheme {
    /// Selected package specifier (e.g. `@clay/theme-gruvbox-material-dark`).
    pub specifier: String,
    /// Inert text-style + base-UI-color overrides for the selected theme.
    pub overrides: Vec<TextThemeOverride>,
    /// Phase 20.1: bounded inert typed UI design-token overrides declared by the
    /// theme package via `clay.contributions.designTokens`. Clay validates each
    /// token name, value type, and bounds against the core fallback catalog
    /// before install. Themes that omit the contribution ship an empty vector
    /// and resolve every UI value from core fallbacks unchanged. Pure data: no
    /// CSS, callbacks, JS execution, or native handles.
    pub design_tokens: Vec<UiDesignTokenOverride>,
}

/// Bounded user appearance preference (Phase 20.6). Selects the canonical
/// default theme only when the user has not explicitly called `setTheme`.
/// `System` follows the observable OS color-scheme signal; when no signal is
/// available it resolves to dark (Modus Vivendi). An explicit `setTheme`
/// specifier always wins over appearance-derived selection.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default,
)]
pub enum Appearance {
    Light,
    Dark,
    #[default]
    System,
}

impl Appearance {
    /// Parse a bounded appearance value from its JSON string form. Unknown
    /// values are rejected so a future field never silently round-trips.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" => Some(Self::System),
            _ => None,
        }
    }

    /// Lowercase JSON string form used on the wire and in config.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    /// Resolve `System` to a concrete light/dark choice given the observed OS
    /// color-scheme signal. `os_dark = true` means the OS reports a dark
    /// preference; `false` means light or unobservable. `System` with no signal
    /// falls back to dark per the Phase 20.6 pinned semantics.
    pub fn resolve(self, os_dark: bool) -> ResolvedAppearance {
        match self {
            Self::Light => ResolvedAppearance::Light,
            Self::Dark => ResolvedAppearance::Dark,
            Self::System => {
                if os_dark {
                    ResolvedAppearance::Dark
                } else {
                    ResolvedAppearance::Light
                }
            }
        }
    }
}

/// Concrete light/dark choice after resolving `Appearance::System`.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedAppearance {
    Light,
    Dark,
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
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolErrorCode {
    UnsupportedProtocolVersion,
    InvalidMessage,
    AccessDenied,
    InternalError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keymaps_contain_editor_comment_toggle_binding() {
        let rule = default_keymaps()
            .into_iter()
            .find(|rule| rule.command_id == "editor.toggleComment")
            .expect("default keymap missing editor comment toggle");

        assert_eq!(
            rule.sequence,
            vec![KeyStroke {
                key: KeyCode::Character("/".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    ..KeyModifiers::NONE
                },
            }]
        );
        assert_eq!(rule.context, KeyBindingContext::EditorTextFocus);
        assert_eq!(rule.routing_policy, RoutingPolicy::ClientFirstPredictable);
    }

    #[test]
    fn default_commands_declare_phase28_editor_configuration_commands() {
        let commands = default_commands();
        for (id, authority, routing_policy) in [
            (
                "editor.toggleComment",
                CommandAuthority::BuiltInClientEdit,
                RoutingPolicy::ClientFirstPredictable,
            ),
            (
                "editor.toggleListMarker",
                CommandAuthority::BuiltInClientEdit,
                RoutingPolicy::ClientFirstPredictable,
            ),
            (
                "editor.rotateHeading",
                CommandAuthority::BuiltInClientEdit,
                RoutingPolicy::ClientFirstPredictable,
            ),
            (
                "editor.clientToggleFold",
                CommandAuthority::ClientUi,
                RoutingPolicy::ClientUiCommand,
            ),
            (
                "editor.toggleInlayHints",
                CommandAuthority::ClientUi,
                RoutingPolicy::ClientUiCommand,
            ),
        ] {
            let command = commands
                .iter()
                .find(|command| command.command_id == id)
                .unwrap_or_else(|| panic!("default commands missing {id}"));
            assert_eq!(command.authority, authority, "{id} authority drifted");
            assert_eq!(
                command.routing_policy, routing_policy,
                "{id} routing policy drifted"
            );
        }
    }

    #[test]
    fn default_keymaps_contain_configuration_reload_binding() {
        let rule = default_keymaps()
            .into_iter()
            .find(|rule| rule.command_id == "runtime.reloadConfiguration")
            .expect("default keymap missing configuration reload");

        assert_eq!(
            rule.sequence,
            vec![KeyStroke {
                key: KeyCode::Character("r".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::NONE
                },
            }]
        );
        assert_eq!(rule.context, KeyBindingContext::Global);
        assert_eq!(
            rule.routing_policy,
            RoutingPolicy::ServerFirstWithLock {
                lock_scope: LockScope::Behavior,
            }
        );
    }

    #[test]
    fn default_keymaps_contain_control_center_open_binding() {
        let keymaps = default_keymaps();
        let rules: Vec<_> = keymaps
            .iter()
            .filter(|rule| rule.command_id == "controlCenter.open")
            .collect();
        // Exactly one default route (Phase 24.5): Global, ServerFirst,
        // two-stroke `Ctrl+X Ctrl+P` chord.
        assert_eq!(
            rules.len(),
            1,
            "exactly one default controlCenter.open route"
        );
        let rule = rules[0];
        assert_eq!(
            rule.sequence,
            vec![
                KeyStroke {
                    key: KeyCode::Character("x".to_string()),
                    modifiers: KeyModifiers {
                        control: true,
                        ..KeyModifiers::NONE
                    },
                },
                KeyStroke {
                    key: KeyCode::Character("p".to_string()),
                    modifiers: KeyModifiers {
                        control: true,
                        ..KeyModifiers::NONE
                    },
                },
            ]
        );
        assert_eq!(rule.context, KeyBindingContext::Global);
        assert_eq!(rule.routing_policy, RoutingPolicy::ServerFirst);
    }

    #[test]
    fn default_keymaps_contain_path_browser_open_binding() {
        let keymaps = default_keymaps();
        let rules: Vec<_> = keymaps
            .iter()
            .filter(|rule| rule.command_id == "controlCenter.openPath")
            .collect();
        // Exactly one default route (Phase 24.5): Global, ServerFirst,
        // two-stroke `Ctrl+X Ctrl+F` chord (command id unchanged from Phase
        // 24.3's temporary single-stroke default).
        assert_eq!(rules.len(), 1, "exactly one default openPath route");
        let rule = rules[0];
        assert_eq!(
            rule.sequence,
            vec![
                KeyStroke {
                    key: KeyCode::Character("x".to_string()),
                    modifiers: KeyModifiers {
                        control: true,
                        ..KeyModifiers::NONE
                    },
                },
                KeyStroke {
                    key: KeyCode::Character("f".to_string()),
                    modifiers: KeyModifiers {
                        control: true,
                        ..KeyModifiers::NONE
                    },
                },
            ]
        );
        assert_eq!(rule.context, KeyBindingContext::Global);
        assert_eq!(rule.routing_policy, RoutingPolicy::ServerFirst);
    }

    #[test]
    fn default_keymaps_are_prefix_collision_free() {
        // Phase 24.5: the full default keymap (including the new chord
        // defaults) must pass the task-5 prefix-collision validation.
        crate::behavior::manifest::validate_manifest(&BehaviorManifest::minimal_text_editing(1))
            .expect("default keymap must be prefix-collision free");
        crate::behavior::manifest::validate_manifest(&BehaviorManifest::core_code_editing(1))
            .expect("core.code keymap must be prefix-collision free");
    }

    #[test]
    fn default_commands_declare_control_center_open_as_server_intent() {
        let commands = default_commands();
        let command = commands
            .iter()
            .find(|command| command.command_id == "controlCenter.open")
            .expect("default commands missing controlCenter.open");
        assert_eq!(command.display_name, "Open Control Center");
        assert_eq!(command.authority, CommandAuthority::ServerIntent);
        assert_eq!(command.routing_policy, RoutingPolicy::ServerFirst);
    }

    #[test]
    fn default_keymaps_contain_phase_22_1_shell_defaults() {
        let keymaps = default_keymaps();
        // Each shell command has a Global-scope, ClientUiCommand-routed default.
        let shell_ids = [
            "shell.clientSplitPaneVertical",
            "shell.clientSplitPaneHorizontal",
            "shell.clientAddEqualPane",
            "shell.clientClosePane",
            "shell.clientFocusPaneNext",
            "shell.clientFocusPanePrev",
            "shell.clientResizePaneLeft",
            "shell.clientResizePaneRight",
            "shell.clientResizePaneUp",
            "shell.clientResizePaneDown",
            "shell.clientMovePaneNext",
            "shell.clientMovePanePrev",
        ];
        for id in shell_ids {
            let rule = keymaps
                .iter()
                .find(|r| r.command_id == id)
                .unwrap_or_else(|| panic!("default keymap missing {id}"));
            assert_eq!(
                rule.context,
                KeyBindingContext::Global,
                "{id} should be Global"
            );
            assert_eq!(
                rule.routing_policy,
                RoutingPolicy::ClientUiCommand,
                "{id} should be ClientUiCommand"
            );
        }
    }

    #[test]
    fn default_commands_contain_phase_22_1_shell_commands() {
        let commands = default_commands();
        let shell_ids = [
            "shell.clientSplitPaneVertical",
            "shell.clientSplitPaneHorizontal",
            "shell.clientAddEqualPane",
            "shell.clientClosePane",
            "shell.clientFocusPaneNext",
            "shell.clientFocusPanePrev",
            "shell.clientResizePaneLeft",
            "shell.clientResizePaneRight",
            "shell.clientResizePaneUp",
            "shell.clientResizePaneDown",
            "shell.clientMovePaneNext",
            "shell.clientMovePanePrev",
        ];
        for id in shell_ids {
            let cmd = commands
                .iter()
                .find(|c| c.command_id == id)
                .unwrap_or_else(|| panic!("default commands missing {id}"));
            assert_eq!(
                cmd.authority,
                CommandAuthority::ClientUi,
                "{id} should be ClientUi"
            );
        }
    }

    #[test]
    fn default_keymaps_contain_phase_22_4_tab_defaults() {
        let keymaps = default_keymaps();
        let mut tab_ids: Vec<String> = [
            "shell.clientTabNext",
            "shell.clientTabPrev",
            "shell.clientTabNew",
            "shell.clientTabClose",
            "shell.clientTabMoveLeft",
            "shell.clientTabMoveRight",
        ]
        .iter()
        .map(|id| id.to_string())
        .collect();
        for n in 1..=9 {
            tab_ids.push(format!("shell.clientTabActivate.{n}"));
            tab_ids.push(format!("shell.clientTabMoveTo.{n}"));
        }
        for id in tab_ids {
            let rule = keymaps
                .iter()
                .find(|r| r.command_id == id)
                .unwrap_or_else(|| panic!("default keymap missing {id}"));
            assert_eq!(
                rule.context,
                KeyBindingContext::Global,
                "{id} should be Global"
            );
            assert_eq!(
                rule.routing_policy,
                RoutingPolicy::ClientUiCommand,
                "{id} should be ClientUiCommand"
            );
        }
        // Numbered families use the declared chords: Ctrl+<N> activates,
        // Ctrl+Shift+<N> moves to position.
        for n in 1..=9 {
            let activate = keymaps
                .iter()
                .find(|r| r.command_id == format!("shell.clientTabActivate.{n}"))
                .unwrap();
            assert!(activate.sequence[0].modifiers.control);
            assert!(!activate.sequence[0].modifiers.shift);
            let move_to = keymaps
                .iter()
                .find(|r| r.command_id == format!("shell.clientTabMoveTo.{n}"))
                .unwrap();
            assert!(move_to.sequence[0].modifiers.control);
            assert!(move_to.sequence[0].modifiers.shift);
            assert_eq!(
                activate.sequence[0].key,
                KeyCode::Character(n.to_string()),
                "Ctrl+{n} activates tab {n}"
            );
        }
        // All default chords are mutually distinct (no chord binds two
        // commands); the manifest ambiguity guard rejects collisions.
        for (i, a) in keymaps.iter().enumerate() {
            for b in keymaps.iter().skip(i + 1) {
                assert_ne!(
                    a.sequence, b.sequence,
                    "default chord collision between {} and {}",
                    a.command_id, b.command_id
                );
            }
        }
    }

    #[test]
    fn default_commands_contain_phase_22_4_tab_commands() {
        let commands = default_commands();
        let mut tab_ids: Vec<String> = [
            "shell.clientTabNext",
            "shell.clientTabPrev",
            "shell.clientTabNew",
            "shell.clientTabClose",
            "shell.clientTabMoveLeft",
            "shell.clientTabMoveRight",
        ]
        .iter()
        .map(|id| id.to_string())
        .collect();
        for n in 1..=9 {
            tab_ids.push(format!("shell.clientTabActivate.{n}"));
            tab_ids.push(format!("shell.clientTabMoveTo.{n}"));
        }
        for id in tab_ids {
            let cmd = commands
                .iter()
                .find(|c| c.command_id == id)
                .unwrap_or_else(|| panic!("default commands missing {id}"));
            assert_eq!(
                cmd.authority,
                CommandAuthority::ClientUi,
                "{id} should be ClientUi"
            );
        }
    }

    #[test]
    fn routing_policy_parse_accepts_kebab_and_pascal() {
        for (input, expected) in [
            (
                "client-first-predictable",
                RoutingPolicy::ClientFirstPredictable,
            ),
            (
                "ClientFirstPredictable",
                RoutingPolicy::ClientFirstPredictable,
            ),
            (
                "client-first-requires-ack",
                RoutingPolicy::ClientFirstRequiresAck,
            ),
            (
                "ClientFirstRequiresAck",
                RoutingPolicy::ClientFirstRequiresAck,
            ),
            ("server-first", RoutingPolicy::ServerFirst),
            ("ServerFirst", RoutingPolicy::ServerFirst),
            ("ui-reactive-priority", RoutingPolicy::UiReactivePriority),
            ("UiReactivePriority", RoutingPolicy::UiReactivePriority),
            ("background", RoutingPolicy::Background),
            ("Background", RoutingPolicy::Background),
        ] {
            assert_eq!(RoutingPolicy::parse(input).unwrap(), expected, "{input}");
        }
        assert!(RoutingPolicy::parse("").is_err());
        assert!(RoutingPolicy::parse("server-first-with-lock").is_err());
        assert!(RoutingPolicy::parse("ClientUiCommand").is_err());
        assert!(RoutingPolicy::parse("not-a-policy").is_err());
    }
}
