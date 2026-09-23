//! Wire types for tree-sitter text objects and smart selection (Plan 071
//! task 10 / pillar E.5).
//!
//! Selection queries are UI-reactive: the client captures its current
//! selection set plus document/behavior versions locally, the server runs the
//! active grammar's `textobjects.scm` (or walks the parsed tree for smart
//! select) read-only, and the client applies the returned byte ranges as
//! selections. No document mutation crosses this boundary.

use super::{BehaviorVersion, ClientId, DocumentId, DocumentVersion};

/// Maximum number of selection cursors one request may carry so server work
/// stays bounded regardless of caret count.
pub const MAX_SELECTION_QUERY_CURSORS: usize = 256;

/// Text object kinds a grammar's `textobjects.scm` may define. Capture names
/// follow Helix-style `textobject.<kind>.<inner|around>` naming (e.g.
/// `textobject.function.around`). Unknown kinds are rejected deny-by-default.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TextobjectKind {
    Function,
    Class,
    Argument,
    Comment,
    Loop,
    Conditional,
    Call,
    Statement,
}

impl TextobjectKind {
    /// All kinds in command-ID/vocabulary order.
    pub const ALL: [TextobjectKind; 8] = [
        TextobjectKind::Function,
        TextobjectKind::Class,
        TextobjectKind::Argument,
        TextobjectKind::Comment,
        TextobjectKind::Loop,
        TextobjectKind::Conditional,
        TextobjectKind::Call,
        TextobjectKind::Statement,
    ];

    /// The capture-name segment for this kind (`textobject.<segment>.<scope>`).
    pub fn as_str(self) -> &'static str {
        match self {
            TextobjectKind::Function => "function",
            TextobjectKind::Class => "class",
            TextobjectKind::Argument => "argument",
            TextobjectKind::Comment => "comment",
            TextobjectKind::Loop => "loop",
            TextobjectKind::Conditional => "conditional",
            TextobjectKind::Call => "call",
            TextobjectKind::Statement => "statement",
        }
    }

    pub fn parse(segment: &str) -> Option<TextobjectKind> {
        TextobjectKind::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == segment)
    }
}

/// Which occurrence of the object to select relative to the caret.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TextobjectDirection {
    /// The innermost object containing the caret.
    Current,
    /// The next object strictly after the caret (does not wrap).
    Next,
    /// The nearest object ending at or before the caret (does not wrap).
    Previous,
}

impl TextobjectDirection {
    pub fn parse(segment: &str) -> Option<TextobjectDirection> {
        match segment {
            "current" => Some(TextobjectDirection::Current),
            "next" => Some(TextobjectDirection::Next),
            "previous" => Some(TextobjectDirection::Previous),
            _ => None,
        }
    }
}

/// Smart-select walks the AST: expand grows the selection to the smallest
/// enclosing node range, shrink returns to the largest node range inside it.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SmartSelectAction {
    Expand,
    Shrink,
}

impl SmartSelectAction {
    pub fn parse(segment: &str) -> Option<SmartSelectAction> {
        match segment {
            "expand" => Some(SmartSelectAction::Expand),
            "shrink" => Some(SmartSelectAction::Shrink),
            _ => None,
        }
    }
}

/// The selection query carried by one request.
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
pub enum SelectionQuery {
    Textobject {
        kind: TextobjectKind,
        /// `true` selects the `around` capture, `false` the `inner` capture
        /// (falling back to `around` when the grammar defines no inner).
        around: bool,
        direction: TextobjectDirection,
    },
    SmartSelect {
        action: SmartSelectAction,
    },
}

/// One client selection captured at request time (byte offsets).
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
pub struct SelectionQueryCursor {
    pub anchor: u64,
    pub focus: u64,
}

/// A typed, versioned selection query enqueued after a UI-reactive command
/// captures the current document/version/selection state locally.
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
pub struct SelectionQueryRequest {
    pub request_id: u64,
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub query: SelectionQuery,
    /// One entry per client selection, in set order (bounded by
    /// [`MAX_SELECTION_QUERY_CURSORS`]).
    pub selections: Vec<SelectionQueryCursor>,
}

/// Validation failure for a [`SelectionQueryRequest`] before any server work.
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
pub enum SelectionQueryRequestRejection {
    TooManySelections,
}

impl SelectionQueryRequest {
    pub fn validate(&self) -> Result<(), SelectionQueryRequestRejection> {
        if self.selections.len() > MAX_SELECTION_QUERY_CURSORS {
            return Err(SelectionQueryRequestRejection::TooManySelections);
        }
        Ok(())
    }
}

/// One resulting byte range for a selection.
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
pub struct SelectionQueryRange {
    pub start: u64,
    pub end: u64,
}

/// Server-to-client result envelope for one selection query. `ranges` aligns
/// index-for-index with the request's `selections`; `None` entries mean "no
/// object found for this caret — leave that selection unchanged".
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
pub struct SelectionQueryResult {
    pub request_id: u64,
    pub client_id: ClientId,
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub ranges: Vec<Option<SelectionQueryRange>>,
}

/// Stable command-ID prefix for textobject selection commands.
pub const TEXTOBJECT_COMMAND_PREFIX: &str = "editor.clientSelectTextobject.";
/// Stable command-ID prefix for smart-select commands.
pub const SMART_SELECT_COMMAND_PREFIX: &str = "editor.clientSmartSelect.";

impl SelectionQuery {
    /// The stable, direction-specific command ID for this query (the
    /// keybinding execution surface; mirrors the task-5 movement pattern).
    pub fn command_id(self) -> String {
        match self {
            SelectionQuery::Textobject {
                kind,
                around,
                direction,
            } => {
                let scope = if around { "around" } else { "inner" };
                match direction {
                    TextobjectDirection::Current => {
                        format!(
                            "{TEXTOBJECT_COMMAND_PREFIX}{kind}.{scope}",
                            kind = kind.as_str()
                        )
                    }
                    TextobjectDirection::Next => format!(
                        "{TEXTOBJECT_COMMAND_PREFIX}{kind}.{scope}.next",
                        kind = kind.as_str()
                    ),
                    TextobjectDirection::Previous => format!(
                        "{TEXTOBJECT_COMMAND_PREFIX}{kind}.{scope}.previous",
                        kind = kind.as_str()
                    ),
                }
            }
            SelectionQuery::SmartSelect { action } => match action {
                SmartSelectAction::Expand => {
                    format!("{SMART_SELECT_COMMAND_PREFIX}expand")
                }
                SmartSelectAction::Shrink => {
                    format!("{SMART_SELECT_COMMAND_PREFIX}shrink")
                }
            },
        }
    }

    /// Parses a stable command ID back into the query it executes, if any.
    pub fn from_command_id(command_id: &str) -> Option<SelectionQuery> {
        if let Some(rest) = command_id.strip_prefix(SMART_SELECT_COMMAND_PREFIX) {
            return SmartSelectAction::parse(rest)
                .map(|action| SelectionQuery::SmartSelect { action });
        }
        let rest = command_id.strip_prefix(TEXTOBJECT_COMMAND_PREFIX)?;
        let (kind_segment, rest) = rest.split_once('.')?;
        let kind = TextobjectKind::parse(kind_segment)?;
        // `<scope>` or `<scope>.<direction>` (default: current).
        let (scope, direction) = match rest.split_once('.') {
            Some((scope, direction_segment)) => {
                (scope, TextobjectDirection::parse(direction_segment)?)
            }
            None => (rest, TextobjectDirection::Current),
        };
        let around = match scope {
            "around" => true,
            "inner" => false,
            _ => return None,
        };
        Some(SelectionQuery::Textobject {
            kind,
            around,
            direction,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SELECTION_QUERY_CURSORS, SelectionQuery, SelectionQueryCursor, SelectionQueryRequest,
        SelectionQueryRequestRejection, SmartSelectAction, TextobjectDirection, TextobjectKind,
    };

    #[test]
    fn textobject_command_ids_round_trip() {
        for kind in TextobjectKind::ALL {
            for around in [false, true] {
                for direction in [
                    TextobjectDirection::Current,
                    TextobjectDirection::Next,
                    TextobjectDirection::Previous,
                ] {
                    let query = SelectionQuery::Textobject {
                        kind,
                        around,
                        direction,
                    };
                    let parsed = SelectionQuery::from_command_id(&query.command_id());
                    assert_eq!(parsed, Some(query), "round trip failed for {query:?}");
                }
            }
        }
    }

    #[test]
    fn smart_select_command_ids_round_trip() {
        for action in [SmartSelectAction::Expand, SmartSelectAction::Shrink] {
            let query = SelectionQuery::SmartSelect { action };
            assert_eq!(
                SelectionQuery::from_command_id(&query.command_id()),
                Some(query)
            );
        }
    }

    #[test]
    fn unknown_textobject_command_ids_reject_deny_by_default() {
        assert_eq!(
            SelectionQuery::from_command_id("editor.clientSelectTextobject.function.side"),
            None
        );
        assert_eq!(
            SelectionQuery::from_command_id("editor.clientSelectTextobject.widget.inner"),
            None
        );
        assert_eq!(
            SelectionQuery::from_command_id("editor.clientSelectTextobject.function"),
            None
        );
        assert_eq!(
            SelectionQuery::from_command_id("editor.clientSmartSelect.grow"),
            None
        );
        assert_eq!(SelectionQuery::from_command_id("editor.clientUndo"), None);
    }

    #[test]
    fn textobject_command_id_shape_matches_registry_contract() {
        assert_eq!(
            SelectionQuery::Textobject {
                kind: TextobjectKind::Function,
                around: false,
                direction: TextobjectDirection::Current,
            }
            .command_id(),
            "editor.clientSelectTextobject.function.inner"
        );
        assert_eq!(
            SelectionQuery::Textobject {
                kind: TextobjectKind::Class,
                around: true,
                direction: TextobjectDirection::Next,
            }
            .command_id(),
            "editor.clientSelectTextobject.class.around.next"
        );
        assert_eq!(
            SelectionQuery::SmartSelect {
                action: SmartSelectAction::Expand,
            }
            .command_id(),
            "editor.clientSmartSelect.expand"
        );
    }

    fn query_request_with(selections: usize) -> SelectionQueryRequest {
        SelectionQueryRequest {
            request_id: 1,
            client_id: 1,
            document_id: 1,
            document_version: 1,
            behavior_version: 1,
            query: SelectionQuery::SmartSelect {
                action: SmartSelectAction::Expand,
            },
            selections: vec![
                SelectionQueryCursor {
                    anchor: 0,
                    focus: 0
                };
                selections
            ],
        }
    }

    #[test]
    fn selection_query_request_validate_bounds_cursors_deny_by_default() {
        // Plan 071 task 15: the advisory wire path is bounded so a hostile or
        // buggy client cannot force unbounded server-side query work.
        assert!(
            query_request_with(MAX_SELECTION_QUERY_CURSORS)
                .validate()
                .is_ok()
        );
        assert_eq!(
            query_request_with(MAX_SELECTION_QUERY_CURSORS + 1)
                .validate()
                .unwrap_err(),
            SelectionQueryRequestRejection::TooManySelections
        );
    }
}
