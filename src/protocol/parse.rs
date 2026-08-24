use crate::protocol::{BehaviorVersion, DecorationSet, DiagnosticSet, DocumentId, DocumentVersion};

/// Byte range metadata used by incremental parse notifications and results.
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
pub struct ParseByteRange {
    pub start: u64,
    pub end: u64,
}

impl ParseByteRange {
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    pub const fn is_valid(self) -> bool {
        self.start <= self.end
    }

    pub const fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.start < other.end && self.end > other.start
    }
}

/// Zero-based parser-neutral row and byte-column position.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ParsePoint {
    pub row: u64,
    pub column: u64,
}

impl ParsePoint {
    pub const fn new(row: u64, column: u64) -> Self {
        Self { row, column }
    }

    fn relative_to(self, base: Self) -> Option<Self> {
        let row = self.row.checked_sub(base.row)?;
        let column = if row == 0 {
            self.column.checked_sub(base.column)?
        } else {
            self.column
        };
        Some(Self { row, column })
    }
}

/// Exact server-accepted edit coordinates between consecutive document versions.
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
pub struct ParseInputEdit {
    pub base_document_version: DocumentVersion,
    pub document_version: DocumentVersion,
    pub start_byte: u64,
    pub old_end_byte: u64,
    pub new_end_byte: u64,
    pub start_position: ParsePoint,
    pub old_end_position: ParsePoint,
    pub new_end_position: ParsePoint,
}

impl ParseInputEdit {
    pub fn is_valid(self) -> bool {
        self.base_document_version.checked_add(1) == Some(self.document_version)
            && self.start_byte <= self.old_end_byte
            && self.start_byte <= self.new_end_byte
            && self.start_position <= self.old_end_position
            && self.start_position <= self.new_end_position
    }

    pub fn relative_to_window(self, window: &ParseWindowSnapshot) -> Option<Self> {
        let base = window.base_point();
        Some(Self {
            start_byte: self.start_byte.checked_sub(window.byte_start)?,
            old_end_byte: self.old_end_byte.checked_sub(window.byte_start)?,
            new_end_byte: self.new_end_byte.checked_sub(window.byte_start)?,
            start_position: self.start_position.relative_to(base)?,
            old_end_position: self.old_end_position.relative_to(base)?,
            new_end_position: self.new_end_position.relative_to(base)?,
            ..self
        })
    }
}

/// Server-prepared, bounded document text supplied to package parsers.
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
pub struct ParseWindowSnapshot {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub package_prefix: String,
    pub mode_id: String,
    /// Stable identity retained across adjacent edits; currently the aligned byte anchor.
    pub window_id: u64,
    pub byte_start: u64,
    pub byte_end: u64,
    pub base_line: u64,
    pub base_column: u64,
    /// True only when `ParseEditNotification::accepted_edit` is representable
    /// against the retained previous-version window.
    pub incremental_edit: bool,
    pub text: String,
}

impl ParseWindowSnapshot {
    pub fn byte_range(&self) -> ParseByteRange {
        ParseByteRange::new(self.byte_start, self.byte_end)
    }

    pub const fn base_point(&self) -> ParsePoint {
        ParsePoint::new(self.base_line, self.base_column)
    }

    pub fn text_len_bytes(&self) -> usize {
        self.text.len()
    }
}

/// Bounded parse-input policy used by generic large-file schedulers.
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
pub struct ParsePolicy {
    pub max_window_bytes: u64,
    pub guard_bytes: u64,
    pub memory_budget_bytes: u64,
    pub timeout_ms: u64,
}

impl ParsePolicy {
    pub const fn new(
        max_window_bytes: u64,
        guard_bytes: u64,
        memory_budget_bytes: u64,
        timeout_ms: u64,
    ) -> Self {
        Self {
            max_window_bytes,
            guard_bytes,
            memory_budget_bytes,
            timeout_ms,
        }
    }
}

/// A package/mode parse-window request before text has been materialized.
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
pub struct ParseWindowRequest {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub package_prefix: String,
    pub mode_id: String,
    pub requested_ranges: Vec<ParseByteRange>,
    pub viewport: ParseByteRange,
    pub policy: ParsePolicy,
}

/// Retained syntax/cache memory budget metadata.
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
pub struct SyntaxMemoryBudget {
    pub budget_bytes: u64,
    pub retained_bytes: u64,
}

impl SyntaxMemoryBudget {
    pub const fn new(budget_bytes: u64, retained_bytes: u64) -> Self {
        Self {
            budget_bytes,
            retained_bytes,
        }
    }

    pub const fn remaining_bytes(self) -> u64 {
        self.budget_bytes.saturating_sub(self.retained_bytes)
    }

    pub const fn is_exceeded(self) -> bool {
        self.retained_bytes > self.budget_bytes
    }
}

/// Coarsest unit a package parser can update incrementally.
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
pub enum ParseUnit {
    File,
    Region,
    LineGroup,
}

/// Compact, versioned server-side notification sent to a package parse handler.
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
pub struct ParseEditNotification {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub package_prefix: String,
    pub mode_id: String,
    pub viewport: ParseByteRange,
    pub invalidated_ranges: Vec<ParseByteRange>,
    /// Exact canonical edit for accepted edit notifications; absent for open,
    /// resync, and viewport-only work.
    pub accepted_edit: Option<ParseInputEdit>,
    pub parse_windows: Vec<ParseWindowSnapshot>,
    pub memory_budget: Option<SyntaxMemoryBudget>,
}

/// Engine-neutral parser recovery capture before document-range translation.
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
pub struct SyntaxDiagnosticCapture {
    pub byte_start: u64,
    pub byte_end: u64,
    pub kind: SyntaxDiagnosticKind,
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
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SyntaxDiagnosticKind {
    Error,
    Missing,
}

/// Inert incremental parse update produced by a server-side package parser.
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
pub struct IncrementalParseUpdate {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub package_prefix: String,
    pub mode_id: String,
    pub parse_unit: ParseUnit,
    pub viewport: ParseByteRange,
    pub invalidated_ranges: Vec<ParseByteRange>,
    /// Bounded inert parser/cache metadata. The Rust client does not execute it.
    pub syntax_tree_delta: Option<String>,
    /// Parse-produced decoration chunks shaped by one handler invocation.
    /// Every member is independently bounded and validated before publication.
    pub decoration_updates: Vec<DecorationSet>,
    /// Optional source-associated diagnostics validated atomically with this update.
    pub diagnostic_update: Option<DiagnosticSet>,
    /// Optional folding ranges produced with this accepted syntax tree or
    /// package publish harvested during the same parse invocation.
    /// Payload-capped by `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES` before publish.
    pub folding_update: Option<crate::protocol::FoldingRangeSet>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_edit_and_stable_window_round_trip() {
        let notification = ParseEditNotification {
            document_id: 7,
            document_version: 2,
            behavior_version: 3,
            package_prefix: "rust".to_string(),
            mode_id: "rust.rust".to_string(),
            viewport: ParseByteRange::new(10, 17),
            invalidated_ranges: vec![ParseByteRange::new(11, 12)],
            accepted_edit: Some(ParseInputEdit {
                base_document_version: 1,
                document_version: 2,
                start_byte: 11,
                old_end_byte: 11,
                new_end_byte: 12,
                start_position: ParsePoint::new(2, 5),
                old_end_position: ParsePoint::new(2, 5),
                new_end_position: ParsePoint::new(2, 6),
            }),
            parse_windows: vec![ParseWindowSnapshot {
                document_id: 7,
                document_version: 2,
                package_prefix: "rust".to_string(),
                mode_id: "rust.rust".to_string(),
                window_id: 10,
                byte_start: 10,
                byte_end: 17,
                base_line: 2,
                base_column: 4,
                incremental_edit: true,
                text: "aZb\néx".to_string(),
            }],
            memory_budget: None,
        };

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&notification).unwrap();
        let decoded =
            rkyv::from_bytes::<ParseEditNotification, rkyv::rancor::Error>(&bytes).unwrap();

        assert_eq!(decoded, notification);
    }
}
