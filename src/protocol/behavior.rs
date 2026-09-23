//! Clay behavior-manifest family: manifests, key bindings, command
//! declarations, and routing/lock policy.

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
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
        // Plan 124: the agent lane toggles on the Emacs-like `Ctrl+X Ctrl+P`
        // chord (P, for the panel), which the Command Centre yielded to it:
        // the lane is the shell's bottom section, so its toggle is the chord a
        // user reaches for. Global + ServerFirst for the same reason as the
        // Command Centre chord below — the editor keymap owns it inside
        // `.cm-editor`, the shell matcher outside it. The declaration is
        // ClientUi (the shell flips its own per-tab layout state); the server
        // answers the intent with `ShellClientCommandRequest`.
        KeyBindingRule::global_server_first_sequence(
            "shell.toggleAgentLane",
            vec![
                ctrl_key(KeyCode::Character("x".to_string())),
                ctrl_key(KeyCode::Character("p".to_string())),
            ],
        ),
        // Phase 24.5: the Command Centre opened on the Emacs-like `Ctrl+X
        // Ctrl+P` chord (P = palette); plan 124 moved it to `Ctrl+X Ctrl+O`
        // (O = open commands) so the lane toggle can take the P stroke. Same
        // server-intent lane, same Global scope, same ServerFirst routing,
        // still overridable via bindKey/unbindKey like every default — and
        // still the entry point the titlebar's Control Center trigger
        // dispatches.
        KeyBindingRule::global_server_first_sequence(
            "controlCenter.open",
            vec![
                ctrl_key(KeyCode::Character("x".to_string())),
                ctrl_key(KeyCode::Character("o".to_string())),
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
        // Plan 109 I6: the left workspace tab (file browser) toggles on the
        // documented `Ctrl+B` chord, Global scope so it fires wherever
        // focus sits (the coding-agent surface, the tree itself) — the
        // workspace tree is chrome, not editor text. Rebindable via
        // bindKey/unbindKey like every default; `Ctrl+B` collides with no
        // shipped default and no editor-internal binding (the CodeMirror
        // emacs keymap is not installed).
        KeyBindingRule::global_server_first(
            "workspace.toggleFileBrowser",
            ctrl_key(KeyCode::Character("b".to_string())),
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
        // intent so the default Global `Ctrl+X Ctrl+O` chord routes (plan 124
        // moved it off the P stroke, which now toggles the agent lane).
        CommandDeclaration::server_intent("controlCenter.open", "Open Control Center"),
        // Phase 24.3: Path Mode (dired-style filesystem browsing) opens via
        // the same command-intent lane; default Global `Ctrl+X Ctrl+F` chord
        // (Phase 24.5), same command id as the temporary single-stroke
        // default.
        CommandDeclaration::server_intent("controlCenter.openPath", "Browse Filesystem"),
        // Plan 109 I6: the workspace file-browser toggle is a built-in
        // server intent (visibility is server-owned per tab); declared so
        // the default Global `Ctrl+B` chord routes like the Control Center
        // family.
        CommandDeclaration::server_intent("workspace.toggleFileBrowser", "Toggle File Browser"),
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
        // Plan 124: the agent lane's visibility is client-local per-tab layout
        // state (like the workspace rail and the inspector), declared here so
        // the default `Ctrl+X Ctrl+P` chord resolves against the manifest command
        // set, `bindKey` accepts it, and the palette reaches it; the server
        // answers the ServerFirst intent with `ShellClientCommandRequest`.
        CommandDeclaration::client_ui("shell.toggleAgentLane", "Toggle Agent Lane"),
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
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum BehaviorScope {
    GlobalDefault,
    Document { document_id: DocumentId },
    Language { language_id: String },
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
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum KeyBindingContext {
    EditorTextFocus,
    CompletionMenu,
    Global,
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
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum CommandAuthority {
    BuiltInClientEdit,
    ServerIntent,
    ClientUi,
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
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum LockScope {
    Range,
    Document,
    Behavior,
    Workspace,
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
        // Exactly one default route: Global, ServerFirst, two-stroke `Ctrl+X
        // Ctrl+O` chord (plan 124 moved it off the P stroke for the lane
        // toggle).
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
                    key: KeyCode::Character("o".to_string()),
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
    fn default_keymaps_contain_agent_lane_toggle_binding() {
        // Plan 124: `Ctrl+X Ctrl+P` toggles the agent lane (Global,
        // ServerFirst) — the chord the Control Centre used to own.
        let rules: Vec<_> = default_keymaps()
            .into_iter()
            .filter(|rule| rule.command_id == "shell.toggleAgentLane")
            .collect();
        assert_eq!(rules.len(), 1, "exactly one default lane toggle route");
        assert_eq!(
            rules[0].sequence,
            vec![
                ctrl_key(KeyCode::Character("x".to_string())),
                ctrl_key(KeyCode::Character("p".to_string())),
            ]
        );
        assert_eq!(rules[0].context, KeyBindingContext::Global);
        assert_eq!(rules[0].routing_policy, RoutingPolicy::ServerFirst);
    }

    #[test]
    fn default_keymaps_ctrl_x_family_keeps_distinct_second_strokes() {
        // Plan 124: `Ctrl+X` is the shell's chord prefix. Lane (P), palette
        // (O) and path mode (F) must keep distinct second strokes: a shared
        // one is a duplicate (context, sequence) pair and `validate_manifest`
        // would reject the default manifest outright.
        let mut seconds: Vec<String> = default_keymaps()
            .into_iter()
            .filter(|rule| {
                rule.sequence.len() == 2
                    && rule.sequence[0] == ctrl_key(KeyCode::Character("x".to_string()))
            })
            .map(|rule| match &rule.sequence[1].key {
                KeyCode::Character(character) => character.clone(),
                other => format!("{other:?}"),
            })
            .collect();
        seconds.sort();
        assert_eq!(
            seconds,
            vec!["f".to_string(), "o".to_string(), "p".to_string()]
        );
    }

    #[test]
    fn default_keymaps_contain_workspace_file_browser_toggle_binding() {
        // Plan 109 I6: the documented `Ctrl+B` default ships for
        // `workspace.toggleFileBrowser` (Global, ServerFirst), keeping the
        // left workspace tab reachable without a user init.js.
        let rule = default_keymaps()
            .into_iter()
            .find(|rule| rule.command_id == "workspace.toggleFileBrowser")
            .expect("default keymaps missing workspace.toggleFileBrowser");
        assert_eq!(
            rule.sequence,
            vec![ctrl_key(KeyCode::Character("b".to_string()))]
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
    fn default_commands_declare_workspace_file_browser_toggle_as_server_intent() {
        // Plan 109 I6: the toggle is declared so the default `Ctrl+B`
        // keymap rule resolves against the manifest command set.
        let commands = default_commands();
        let command = commands
            .iter()
            .find(|command| command.command_id == "workspace.toggleFileBrowser")
            .expect("default commands missing workspace.toggleFileBrowser");
        assert_eq!(command.display_name, "Toggle File Browser");
        assert_eq!(command.authority, CommandAuthority::ServerIntent);
        assert_eq!(command.routing_policy, RoutingPolicy::ServerFirst);
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
    fn default_commands_declare_agent_lane_toggle_as_client_ui() {
        // Plan 124: the lane toggle is declared so the `Ctrl+X Ctrl+P` keymap
        // rule resolves against the manifest command set; the authority is
        // ClientUi (the shell owns per-tab layout state), which is what makes
        // the server answer the intent with a ShellClientCommandRequest.
        let commands = default_commands();
        let command = commands
            .iter()
            .find(|command| command.command_id == "shell.toggleAgentLane")
            .expect("default commands missing shell.toggleAgentLane");
        assert_eq!(command.display_name, "Toggle Agent Lane");
        assert_eq!(command.authority, CommandAuthority::ClientUi);
        assert_eq!(command.routing_policy, RoutingPolicy::ClientUiCommand);
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
