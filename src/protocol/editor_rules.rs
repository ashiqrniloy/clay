//! Editor behavior/layout rule shapes carried by [`BehaviorManifest`].

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
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum EditOperation {
    Insert { byte_offset: u64, text: String },
    Delete { start: u64, end: u64 },
    Replace { start: u64, end: u64, text: String },
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
pub enum EditorIntent {
    InsertText { byte_offset: u64, text: String },
    DeleteRange { start: u64, end: u64 },
}

/// Word-boundary policy consumed by movement, selection, and completion so
/// they share one classifier. `Code` with `treat_underscore_as_word = true`
/// reproduces the historical `is_completion_word_character` classifier
/// (`_` || Unicode alphanumeric) exactly.
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
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum LineMovementStyle {
    Character,
    ScreenLine,
}

/// Movement configuration shipped in [`EditorBehaviorRules`]. Language-
/// agnostic; packages override via manifest data, never per-language Rust.
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
pub struct EditorLayoutRules {
    pub wrap: WrapPolicy,
}

/// How document text wraps inside the pane.
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
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
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
pub enum TextEditCapability {
    Insert,
    Delete,
    Replace,
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
pub struct TabRule {
    pub mode: TabMode,
    pub spaces_per_tab: u8,
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
pub enum TabMode {
    InsertSpaces,
    InsertTabCharacter,
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
pub enum PairRuleContext {
    CaretOrSelection,
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
pub struct CommentContinuationRule {
    pub line_prefix: String,
    pub continue_prefix: String,
}

/// Deterministic local reflow applied when an electric-character trigger is
/// typed. Executed entirely by Rust-known transform engines on the client from
/// manifest data; no callbacks, JavaScript, or IPC are involved.
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
pub struct AutocompleteTrigger {
    pub trigger: String,
    pub routing_policy: RoutingPolicy,
}
