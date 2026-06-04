use crate::protocol::{BehaviorVersion, DecorationSet, DocumentId, DocumentVersion};

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

    pub const fn intersects(self, other: Self) -> bool {
        self.start < other.end && self.end > other.start
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
}
