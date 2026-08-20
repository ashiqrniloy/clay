// Auto-extracted from surface.rs (Plan 090 task 5). Private submodule: command.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorSelectDirection {
    Down,
    Up,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand<'a> {
    Insert(&'a str),
    Newline,
    Backspace,
    DeleteForward,
    MoveLeft,
    MoveRight,
    SelectLeft,
    SelectRight,
    MoveUp,
    MoveDown,
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
    MoveWordStart {
        forward: bool,
        long: bool,
        extend: bool,
    },
    MoveWordEnd {
        forward: bool,
        long: bool,
        extend: bool,
    },
    MoveSubWord {
        forward: bool,
        extend: bool,
    },
    MoveParagraph {
        forward: bool,
        to_end: bool,
        extend: bool,
    },
    MoveFirstNonWhitespace {
        extend: bool,
    },
    MoveLastNonWhitespace {
        extend: bool,
    },
    MoveMatchingPair {
        extend: bool,
    },
    SelectWord,
    SelectLine,
    SelectParagraph,
    /// Add a collapsed caret one visual line below/above the primary caret at
    /// the same scalar column (Plan 071 task 9, VSCode insertCursorBelow/Above).
    AddCursor {
        direction: CursorSelectDirection,
    },
    /// Column/box selection: Down/Up adds a caret one line below/above the
    /// primary (growing the box); Left/Right moves every caret one scalar
    /// (Plan 071 task 9, VSCode cursorColumnSelect*).
    ColumnSelect {
        direction: CursorSelectDirection,
    },
    /// Select the next occurrence of the current selection/word as a new
    /// primary selection (VSCode addSelectionToNextFindMatch, Ctrl+D).
    SelectNextMatch,
    /// Symmetric backwards variant of [`SelectNextMatch`].
    SelectPrevMatch,
    /// Replace the selection set with every occurrence of the current
    /// selection/word (VSCode selectHighlights, Ctrl+Shift+L).
    SelectAllMatches,
    /// Collapse the selection set to the primary caret (Escape).
    CancelMultipleSelections,
    /// Keep only the primary selection (Helix keep_primary_selection).
    KeepSelection,
    /// Remove the primary selection, keeping the rest (Helix remove_primary_selection).
    RemoveSelection,
    /// Restore the previous selection set from the cursor-undo stack (Ctrl+U,
    /// VSCode cursorUndo). Cursor movements only; edits have their own history.
    UndoCursorMove,
    /// Toggle `CommentContinuationRule.line_prefix` on lines touching carets.
    ToggleComment,
    /// Toggle the first `EnterRule::ContinueLineMarkers` marker.
    ToggleListMarker,
    /// Rotate `heading_prefixes` on lines touching carets.
    RotateHeading,
    /// Toggle the fold range containing the caret.
    ToggleFold,
    /// Hide or show inlay overlays without refetching.
    ToggleInlayHints,
}

impl EditorCommand<'_> {
    /// True for commands that move the caret or reshape the selection set
    /// without editing text. These snapshot the selection set for cursor-undo
    /// (Plan 071 task 9). Edits and `UndoCursorMove` itself do not snapshot.
    pub fn is_selection_changing(&self) -> bool {
        matches!(
            self,
            EditorCommand::MoveLeft
                | EditorCommand::MoveRight
                | EditorCommand::SelectLeft
                | EditorCommand::SelectRight
                | EditorCommand::MoveUp
                | EditorCommand::MoveDown
                | EditorCommand::LineStart
                | EditorCommand::LineEnd
                | EditorCommand::DocumentStart
                | EditorCommand::DocumentEnd
                | EditorCommand::MoveWordStart { .. }
                | EditorCommand::MoveWordEnd { .. }
                | EditorCommand::MoveSubWord { .. }
                | EditorCommand::MoveParagraph { .. }
                | EditorCommand::MoveFirstNonWhitespace { .. }
                | EditorCommand::MoveLastNonWhitespace { .. }
                | EditorCommand::MoveMatchingPair { .. }
                | EditorCommand::SelectWord
                | EditorCommand::SelectLine
                | EditorCommand::SelectParagraph
                | EditorCommand::AddCursor { .. }
                | EditorCommand::ColumnSelect { .. }
                | EditorCommand::SelectNextMatch
                | EditorCommand::SelectPrevMatch
                | EditorCommand::SelectAllMatches
                | EditorCommand::CancelMultipleSelections
                | EditorCommand::KeepSelection
                | EditorCommand::RemoveSelection
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorEditEvent {
    pub document_id: DocumentId,
    pub base_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub operation: EditOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommandOutcome {
    pub changed: bool,
    /// First edit event (single-caret edits and test readers). Multi-cursor
    /// edits expose every per-caret event through `edit_events`.
    pub edit_event: Option<EditorEditEvent>,
    /// Every edit event produced by the command, right-to-left for
    /// multi-cursor edits. The connection layer stamps each with an ascending
    /// optimistic base version, so the server applies them in order.
    pub edit_events: Vec<EditorEditEvent>,
    /// Status copy when a transform no-ops (missing manifest rule).
    pub diagnostic: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorKeyOutcome {
    pub(crate) command_outcome: EditorCommandOutcome,
    pub(crate) server_intent: Option<ServerIntentRoute>,
    pub(crate) client_ui_command: Option<ClientUiCommandRoute>,
    pub(crate) completion_request: Option<EditorCompletionRequestEvent>,
    pub(crate) language_intelligence_request: Option<EditorLanguageIntelligenceRequestEvent>,
    /// Phase 24.5: the key was consumed by chord bookkeeping (a pending stroke)
    /// with no dispatchable side effect; the pane must mark it handled so it
    /// neither inserts text nor bubbles to shell-level handlers.
    pub(crate) consumed: bool,
}

/// Phase 24.5: an in-progress multi-stroke chord. Owned by `EditorSurface`
/// (mutable routing state that must survive across keystrokes); holds only
/// already-validated `KeyStroke` values from the incoming event stream.
#[derive(Debug)]
pub(crate) struct PendingChord {
    pub(super) strokes: Vec<KeyStroke>,
    pub(super) started_at: std::time::Instant,
}

impl EditorKeyOutcome {
    pub(super) fn client(command_outcome: EditorCommandOutcome) -> Self {
        Self {
            command_outcome,
            server_intent: None,
            client_ui_command: None,
            completion_request: None,
            language_intelligence_request: None,
            consumed: false,
        }
    }

    pub(super) fn consumed() -> Self {
        Self {
            command_outcome: EditorCommandOutcome::unchanged(),
            server_intent: None,
            client_ui_command: None,
            completion_request: None,
            language_intelligence_request: None,
            consumed: true,
        }
    }

    pub(super) fn server(server_intent: ServerIntentRoute) -> Self {
        Self {
            command_outcome: EditorCommandOutcome::unchanged(),
            server_intent: Some(server_intent),
            client_ui_command: None,
            completion_request: None,
            language_intelligence_request: None,
            consumed: false,
        }
    }

    pub(super) fn client_ui(client_ui_command: ClientUiCommandRoute) -> Self {
        Self {
            command_outcome: EditorCommandOutcome::unchanged(),
            server_intent: None,
            client_ui_command: Some(client_ui_command),
            completion_request: None,
            language_intelligence_request: None,
            consumed: false,
        }
    }

    pub(super) fn completion(completion_request: EditorCompletionRequestEvent) -> Self {
        Self {
            command_outcome: EditorCommandOutcome::unchanged(),
            server_intent: None,
            client_ui_command: None,
            completion_request: Some(completion_request),
            language_intelligence_request: None,
            consumed: false,
        }
    }

    pub(super) fn language_intelligence(
        language_intelligence_request: EditorLanguageIntelligenceRequestEvent,
    ) -> Self {
        Self {
            command_outcome: EditorCommandOutcome::unchanged(),
            server_intent: None,
            client_ui_command: None,
            completion_request: None,
            language_intelligence_request: Some(language_intelligence_request),
            consumed: false,
        }
    }

    pub(super) fn with_completion(
        mut self,
        completion_request: Option<EditorCompletionRequestEvent>,
    ) -> Self {
        self.completion_request = completion_request;
        self
    }

    pub(super) fn unhandled() -> Self {
        Self::client(EditorCommandOutcome::unchanged())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorCompletionRequestEvent {
    pub(crate) document_id: DocumentId,
    pub(crate) document_version: DocumentVersion,
    pub(crate) behavior_version: BehaviorVersion,
    pub(crate) cursor_byte_offset: u64,
    pub(crate) replacement_range: CompletionReplacementRange,
    pub(crate) trigger: CompletionTrigger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorLanguageIntelligenceRequestEvent {
    pub(crate) document_id: DocumentId,
    pub(crate) document_version: DocumentVersion,
    pub(crate) behavior_version: BehaviorVersion,
    pub(crate) cursor_byte_offset: u64,
    pub(crate) feature: crate::protocol::LanguageIntelligenceFeature,
}

/// Plan 071 task 10: captured selection-query context (document/behavior
/// versions + the whole selection set) for a tree-sitter text-object or
/// smart-select request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorSelectionQueryRequestEvent {
    pub(crate) document_id: DocumentId,
    pub(crate) document_version: DocumentVersion,
    pub(crate) behavior_version: BehaviorVersion,
    pub(crate) query: crate::protocol::SelectionQuery,
    pub(crate) selections: Vec<crate::protocol::SelectionQueryCursor>,
}

impl EditorCommandOutcome {
    pub(super) fn unchanged() -> Self {
        Self {
            changed: false,
            edit_event: None,
            edit_events: Vec::new(),
            diagnostic: None,
        }
    }

    pub(super) fn unchanged_with(diagnostic: &'static str) -> Self {
        Self {
            changed: false,
            edit_event: None,
            edit_events: Vec::new(),
            diagnostic: Some(diagnostic),
        }
    }

    pub(super) fn changed(edit_event: Option<EditorEditEvent>) -> Self {
        Self {
            changed: true,
            edit_events: edit_event.iter().cloned().collect(),
            edit_event,
            diagnostic: None,
        }
    }

    /// Multi-cursor edit outcome: one event per caret (Plan 071 task 9).
    pub(super) fn changed_multi(edit_events: Vec<EditorEditEvent>) -> Self {
        Self {
            changed: true,
            edit_event: edit_events.first().cloned(),
            edit_events,
            diagnostic: None,
        }
    }

    pub(super) fn from_changed(changed: bool) -> Self {
        Self {
            changed,
            edit_event: None,
            edit_events: Vec::new(),
            diagnostic: None,
        }
    }
}
