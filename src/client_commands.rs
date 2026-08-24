//! Renderer-neutral allowlists for client-local editor, pane, and tab commands.
//!
//! Server validation and command catalogues depend on these IDs. React owns
//! execution; no native widget type is exposed.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorClientCommand {
    MoveWordStartForward,
    MoveWordStartBackward,
    MoveParagraphForward,
    MoveParagraphBackward,
    SelectWord,
    SelectLine,
    AddCursorBelow,
    AddCursorAbove,
    ColumnSelectDown,
    ColumnSelectUp,
    ColumnSelectLeft,
    ColumnSelectRight,
    SelectNextMatch,
    SelectPrevMatch,
    SelectAllMatches,
    CancelMultipleSelections,
    KeepSelection,
    RemoveSelection,
    UndoCursorMove,
    ToggleComment,
    ToggleListMarker,
    RotateHeading,
    ToggleFold,
    ToggleInlayHints,
}

impl EditorClientCommand {
    pub fn from_command_id(command_id: &str) -> Option<Self> {
        match command_id {
            "editor.clientMoveCursor.nextWordStart" => Some(Self::MoveWordStartForward),
            "editor.clientMoveCursor.prevWordStart" => Some(Self::MoveWordStartBackward),
            "editor.clientMoveCursor.nextParagraph" => Some(Self::MoveParagraphForward),
            "editor.clientMoveCursor.prevParagraph" => Some(Self::MoveParagraphBackward),
            "editor.clientSetSelection.selectWord" => Some(Self::SelectWord),
            "editor.clientSetSelection.selectLine" => Some(Self::SelectLine),
            "editor.clientAddCursor.below" => Some(Self::AddCursorBelow),
            "editor.clientAddCursor.above" => Some(Self::AddCursorAbove),
            "editor.clientColumnSelect.down" => Some(Self::ColumnSelectDown),
            "editor.clientColumnSelect.up" => Some(Self::ColumnSelectUp),
            "editor.clientColumnSelect.left" => Some(Self::ColumnSelectLeft),
            "editor.clientColumnSelect.right" => Some(Self::ColumnSelectRight),
            "editor.clientSelectNextMatch" => Some(Self::SelectNextMatch),
            "editor.clientSelectPrevMatch" => Some(Self::SelectPrevMatch),
            "editor.clientSelectAllMatches" => Some(Self::SelectAllMatches),
            "editor.clientCancelMultipleSelections" => Some(Self::CancelMultipleSelections),
            "editor.clientKeepSelection" => Some(Self::KeepSelection),
            "editor.clientRemoveSelection" => Some(Self::RemoveSelection),
            "editor.clientUndoCursorMove" => Some(Self::UndoCursorMove),
            "editor.toggleComment"
            | "rust.toggleLineComment"
            | "typescript.toggleLineComment"
            | "javascript.toggleLineComment"
            | "markdown.toggleComment" => Some(Self::ToggleComment),
            "editor.toggleListMarker" | "markdown.toggleList" => Some(Self::ToggleListMarker),
            "editor.rotateHeading" | "markdown.insertHeading" => Some(Self::RotateHeading),
            "editor.clientToggleFold" => Some(Self::ToggleFold),
            "editor.toggleInlayHints" => Some(Self::ToggleInlayHints),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellClientCommand {
    SplitPaneVertical,
    SplitPaneHorizontal,
    AddEqualPane,
    ClosePane,
    FocusPaneNext,
    FocusPanePrev,
    ResizePaneLeft,
    ResizePaneRight,
    ResizePaneUp,
    ResizePaneDown,
    MovePaneNext,
    MovePanePrev,
    TabNext,
    TabPrev,
    TabNew,
    TabClose,
    TabMoveLeft,
    TabMoveRight,
    TabActivate(u32),
    TabMoveTo(u32),
}

impl ShellClientCommand {
    pub fn from_command_id(command_id: &str) -> Option<Self> {
        match command_id {
            "shell.clientSplitPaneVertical" | "shell.clientSplitPaneRight" => {
                Some(Self::SplitPaneVertical)
            }
            "shell.clientSplitPaneHorizontal" | "shell.clientSplitPaneDown" => {
                Some(Self::SplitPaneHorizontal)
            }
            "shell.clientAddEqualPane" => Some(Self::AddEqualPane),
            "shell.clientClosePane" => Some(Self::ClosePane),
            "shell.clientFocusPaneNext" => Some(Self::FocusPaneNext),
            "shell.clientFocusPanePrev" => Some(Self::FocusPanePrev),
            "shell.clientResizePaneLeft" => Some(Self::ResizePaneLeft),
            "shell.clientResizePaneRight" => Some(Self::ResizePaneRight),
            "shell.clientResizePaneUp" => Some(Self::ResizePaneUp),
            "shell.clientResizePaneDown" => Some(Self::ResizePaneDown),
            "shell.clientMovePaneNext" => Some(Self::MovePaneNext),
            "shell.clientMovePanePrev" => Some(Self::MovePanePrev),
            "shell.clientTabNext" => Some(Self::TabNext),
            "shell.clientTabPrev" => Some(Self::TabPrev),
            "shell.clientTabNew" => Some(Self::TabNew),
            "shell.clientTabClose" => Some(Self::TabClose),
            "shell.clientTabMoveLeft" => Some(Self::TabMoveLeft),
            "shell.clientTabMoveRight" => Some(Self::TabMoveRight),
            value => numbered_tab_command(value),
        }
    }
}

fn numbered_tab_command(command_id: &str) -> Option<ShellClientCommand> {
    for (prefix, constructor) in [
        (
            "shell.clientTabActivate.",
            ShellClientCommand::TabActivate as fn(u32) -> ShellClientCommand,
        ),
        (
            "shell.clientTabMoveTo.",
            ShellClientCommand::TabMoveTo as fn(u32) -> ShellClientCommand,
        ),
    ] {
        if let Some(position) = command_id
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.parse::<u32>().ok())
            .filter(|position| (1..=9).contains(position))
        {
            return Some(constructor(position));
        }
    }
    None
}

pub const SHELL_CLIENT_COMMAND_CATALOGUE: &[(&str, &str)] = &[
    ("shell.clientSplitPaneVertical", "Split Pane Vertical"),
    ("shell.clientSplitPaneHorizontal", "Split Pane Horizontal"),
    ("shell.clientSplitPaneRight", "Split Pane Right"),
    ("shell.clientSplitPaneDown", "Split Pane Down"),
    ("shell.clientAddEqualPane", "Add Equal Pane"),
    ("shell.clientClosePane", "Close Pane"),
    ("shell.clientFocusPaneNext", "Focus Next Pane"),
    ("shell.clientFocusPanePrev", "Focus Previous Pane"),
    ("shell.clientResizePaneLeft", "Resize Pane Left"),
    ("shell.clientResizePaneRight", "Resize Pane Right"),
    ("shell.clientResizePaneUp", "Resize Pane Up"),
    ("shell.clientResizePaneDown", "Resize Pane Down"),
    ("shell.clientMovePaneNext", "Move Pane Next"),
    ("shell.clientMovePanePrev", "Move Pane Previous"),
    ("shell.clientTabNext", "Next Tab"),
    ("shell.clientTabPrev", "Previous Tab"),
    ("shell.clientTabNew", "New Tab"),
    ("shell.clientTabClose", "Close Tab"),
    ("shell.clientTabMoveLeft", "Move Tab Left"),
    ("shell.clientTabMoveRight", "Move Tab Right"),
    ("shell.clientTabActivate.1", "Activate Tab 1"),
    ("shell.clientTabActivate.2", "Activate Tab 2"),
    ("shell.clientTabActivate.3", "Activate Tab 3"),
    ("shell.clientTabActivate.4", "Activate Tab 4"),
    ("shell.clientTabActivate.5", "Activate Tab 5"),
    ("shell.clientTabActivate.6", "Activate Tab 6"),
    ("shell.clientTabActivate.7", "Activate Tab 7"),
    ("shell.clientTabActivate.8", "Activate Tab 8"),
    ("shell.clientTabActivate.9", "Activate Tab 9"),
    ("shell.clientTabMoveTo.1", "Move Tab to Position 1"),
    ("shell.clientTabMoveTo.2", "Move Tab to Position 2"),
    ("shell.clientTabMoveTo.3", "Move Tab to Position 3"),
    ("shell.clientTabMoveTo.4", "Move Tab to Position 4"),
    ("shell.clientTabMoveTo.5", "Move Tab to Position 5"),
    ("shell.clientTabMoveTo.6", "Move Tab to Position 6"),
    ("shell.clientTabMoveTo.7", "Move Tab to Position 7"),
    ("shell.clientTabMoveTo.8", "Move Tab to Position 8"),
    ("shell.clientTabMoveTo.9", "Move Tab to Position 9"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_allowlists_fail_closed() {
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientTabActivate.9"),
            Some(ShellClientCommand::TabActivate(9))
        );
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientTabActivate.10"),
            None
        );
        assert_eq!(EditorClientCommand::from_command_id("Deno.core.ops"), None);
    }
}
