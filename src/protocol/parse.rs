use crate::protocol::{BehaviorVersion, DecorationSet, DiagnosticSet, DocumentId, DocumentVersion};

/// Byte range metadata used by incremental parse notifications and results.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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

/// Server-prepared, bounded document text supplied to package parsers.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ParseWindowSnapshot {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub package_prefix: String,
    pub mode_id: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub base_line: u64,
    pub text: String,
}

impl ParseWindowSnapshot {
    pub fn byte_range(&self) -> ParseByteRange {
        ParseByteRange::new(self.byte_start, self.byte_end)
    }

    pub fn text_len_bytes(&self) -> usize {
        self.text.len()
    }
}

/// Bounded parse-input policy used by generic large-file schedulers.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
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
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseUnit {
    File,
    Region,
    LineGroup,
}

/// Compact, versioned server-side notification sent to a package parse handler.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ParseEditNotification {
    pub document_id: DocumentId,
    pub document_version: DocumentVersion,
    pub behavior_version: BehaviorVersion,
    pub package_prefix: String,
    pub mode_id: String,
    pub viewport: ParseByteRange,
    pub invalidated_ranges: Vec<ParseByteRange>,
    pub parse_windows: Vec<ParseWindowSnapshot>,
    pub memory_budget: Option<SyntaxMemoryBudget>,
}

/// Engine-neutral parser recovery capture before document-range translation.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxDiagnosticCapture {
    pub byte_start: u64,
    pub byte_end: u64,
    pub kind: SyntaxDiagnosticKind,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxDiagnosticKind {
    Error,
    Missing,
}

/// Inert incremental parse update produced by a server-side package parser.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
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
    /// Optional parse-produced decorations after handler-side shaping. Decoration
    /// validation still runs before client publication.
    pub decoration_update: Option<DecorationSet>,
    /// Optional source-associated diagnostics validated atomically with this update.
    pub diagnostic_update: Option<DiagnosticSet>,
}
